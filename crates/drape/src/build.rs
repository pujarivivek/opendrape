use crate::Stage;
use glam::{DVec2, DVec3};
use opendrape_core::{PieceId, Point2, Project};
use opendrape_geom as geom;
use opendrape_mesh::{MeshNote, MeshParams, place};
use opendrape_sim::{ClothBuilder, Panel, Params, Solver};

/// Fabric weight (kg/m²): one light cotton until fabrics arrive.
pub const DENSITY_KG_M2: f64 = 0.15;

/// Something the student should know about the drape.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum DrapeNote {
    /// From making the fabric (see [`MeshNote`]).
    Mesh(MeshNote),
    /// Part of this piece starts inside the form.
    StartsInside(PieceId),
}

/// The fabric and cloth for `project` on `stage`: every shape that could be meshed, at its
/// placement, sewn by its seams; and the notes about it.
pub fn build_drape(project: &Project, stage: &Stage) -> (Solver, Vec<DrapeNote>) {
    let mesh = opendrape_mesh::build(project, &MeshParams::default());
    let shapes = geom::shapes(project);
    let layout = place::layout(&shapes);
    let mut notes: Vec<DrapeNote> = mesh.notes.iter().map(|n| DrapeNote::Mesh(*n)).collect();
    let mut builder = ClothBuilder::new(DENSITY_KG_M2);
    let mut ids = Vec::with_capacity(mesh.panels.len());
    for panel in &mesh.panels {
        let shape = shapes
            .iter()
            .find(|s| s.id == panel.shape)
            .expect("every panel comes from a shape");
        let placement = place::effective(project, shape, &layout, stage.shoulder_y());
        let positions: Vec<DVec3> = panel
            .flat
            .iter()
            .map(|f| {
                place::apply(
                    &placement,
                    panel.centre,
                    Point2::new(f[0] * 1000.0, f[1] * 1000.0),
                )
            })
            .collect();
        if positions.iter().any(|p| stage.signed_distance(*p) < 0.0) {
            notes.push(DrapeNote::StartsInside(panel.shape));
        }
        ids.push(builder.add_panel(
            &Panel {
                positions,
                flat: Some(panel.flat.iter().map(|f| DVec2::from_array(*f)).collect()),
                triangles: panel.triangles.clone(),
            },
            1.0,
        ));
    }
    for &((pa, a), (pb, b)) in &mesh.stitches {
        builder.stitch((ids[pa], a), (ids[pb], b));
    }
    (Solver::new(builder.build(), Params::default()), notes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use opendrape_core::{Half, Piece, Placement, SeamSide};
    use opendrape_mesh::MeshNote;

    /// Two 200 × 300 mm panels on the pattern table, 100 mm apart, sewn along the side between
    /// them: A's right edge to B's left edge, both starting at the bottom.
    fn two_panels() -> Project {
        let mut pr = Project::new();
        let a = pr.add_piece(Piece::rectangle(
            PieceId(0),
            "A",
            Point2::new(0.0, 0.0),
            200.0,
            300.0,
        ));
        let b = pr.add_piece(Piece::rectangle(
            PieceId(0),
            "B",
            Point2::new(300.0, 0.0),
            200.0,
            300.0,
        ));
        pr.add_seam(
            SeamSide::edges(a, Half::Drawn, 1, 1, true),
            SeamSide::edges(b, Half::Drawn, 3, 3, false),
        );
        pr
    }

    #[test]
    fn notes_name_pieces_that_start_inside_or_could_not_be_made() {
        let mut pr = Project::new();
        let mut inside = Piece::rectangle(PieceId(0), "Front", Point2::new(0.0, 0.0), 100.0, 100.0);
        inside.placement = Some(Placement::at([0.0, 1.0, 0.0]));
        let inside = pr.add_piece(inside);
        let bow = pr.add_piece(Piece::polygon(
            PieceId(0),
            "Bow",
            &[
                Point2::new(0.0, 0.0),
                Point2::new(100.0, 100.0),
                Point2::new(100.0, 0.0),
                Point2::new(0.0, 100.0),
            ],
        ));
        let (_, notes) = build_drape(&pr, &Stage::shared());
        assert_eq!(
            notes,
            vec![
                DrapeNote::Mesh(MeshNote::CrossesItself(bow)),
                DrapeNote::StartsInside(inside)
            ]
        );
    }

    #[test]
    fn the_seams_become_stitches_between_the_right_particles() {
        let pr = two_panels();
        let mesh = opendrape_mesh::build(&pr, &MeshParams::default());
        let (solver, notes) = build_drape(&pr, &Stage::shared());
        assert_eq!(notes, vec![]);
        let cloth = solver.cloth();
        assert!(cloth.has_open_stitches(), "the seam is stitched");
        let pairs: Vec<(usize, usize)> = cloth.stitch_pairs().collect();
        assert_eq!(pairs.len(), mesh.stitches.len());
        // Unplaced, both pieces stand on one plane at their pattern-table places, so every
        // stitched pair is the 100 mm gap apart, level: it joins A's right edge to B's left
        // edge, bottom to bottom, whichever particles the two panels' points are numbered as.
        let x = cloth.positions();
        for (a, b) in pairs {
            let d = x[b] - x[a];
            assert!(
                (d - glam::DVec3::new(0.1, 0.0, 0.0)).length() < 1e-9,
                "{a} and {b} are {d} apart"
            );
        }
    }

    #[test]
    fn a_sewn_drape_pulls_its_seam_shut_and_welds_it() {
        let stage = Stage::shared();
        let (mut solver, _) = build_drape(&two_panels(), &stage);
        let before = (0..solver.cloth().len())
            .filter(|&i| solver.cloth().is_alive(i))
            .count();
        let stitched = solver.cloth().stitch_pairs().count();
        let collider = stage.drape_collider();
        let mut gap_at_the_end_of_closing = f64::MAX;
        for _ in 0..90 {
            if solver.cloth().has_open_stitches() {
                let x = solver.cloth().positions();
                gap_at_the_end_of_closing = solver
                    .cloth()
                    .stitch_pairs()
                    .map(|(a, b)| (x[a] - x[b]).length())
                    .fold(0.0, f64::max);
            }
            solver.step(Some(&collider));
        }
        assert!(
            gap_at_the_end_of_closing < 0.004,
            "the seam closed to {:.1} mm before it welded",
            gap_at_the_end_of_closing * 1000.0
        );
        let cloth = solver.cloth();
        assert!(!cloth.has_open_stitches(), "welded after 1.5 s");
        let after = (0..cloth.len()).filter(|&i| cloth.is_alive(i)).count();
        assert_eq!(
            before - after,
            stitched,
            "each stitched pair became one particle"
        );
    }
}
