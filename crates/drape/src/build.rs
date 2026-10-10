use crate::Stage;
use crate::live;
use glam::{DVec2, DVec3};
use opendrape_core::{PieceId, Point2, Project};
use opendrape_geom as geom;
use opendrape_mesh::{GarmentMesh, MeshNote, MeshParams, place};
use opendrape_sim::{AttachmentId, ClothBuilder, Panel, Params, Solver};
use std::sync::Arc;

/// Fabric weight (kg/m²): one light cotton until fabrics arrive.
pub const DENSITY_KG_M2: f64 = 0.15;
/// How stiffly a pin holds its spot at its target (XPBD compliance, m/N): exactly.
pub const PIN_COMPLIANCE: f64 = 0.0;

/// Something the student should know about the drape.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum DrapeNote {
    /// From making the fabric (see [`MeshNote`]).
    Mesh(MeshNote),
    /// Part of this piece starts inside the form.
    StartsInside(PieceId),
}

/// One panel of a drape's fabric: the shape it was made from, its flat points (m, on the
/// pattern table) and its own triangles, and where its particles and triangles start in the
/// cloth.
#[derive(Clone, Debug, PartialEq)]
pub struct FabricPanel {
    pub shape: PieceId,
    pub flat: Vec<[f64; 2]>,
    pub triangles: Vec<[u32; 3]>,
    pub first_particle: usize,
    pub first_triangle: usize,
}

/// Where a drape's cloth came from on the pattern: its panels, in cloth order.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Fabric {
    pub panels: Vec<FabricPanel>,
}

impl Fabric {
    fn of(mesh: &GarmentMesh) -> Self {
        let (mut particle, mut triangle) = (0, 0);
        let panels = mesh
            .panels
            .iter()
            .map(|p| {
                let panel = FabricPanel {
                    shape: p.shape,
                    flat: p.flat.clone(),
                    triangles: p.triangles.clone(),
                    first_particle: particle,
                    first_triangle: triangle,
                };
                particle += p.flat.len();
                triangle += p.triangles.len();
                panel
            })
            .collect();
        Self { panels }
    }

    /// The panel of shape `shape`.
    pub fn panel(&self, shape: PieceId) -> Option<&FabricPanel> {
        self.panels.iter().find(|p| p.shape == shape)
    }

    /// The panel that cloth triangle `triangle` belongs to, and its index in the panel.
    fn triangle(&self, triangle: usize) -> Option<(&FabricPanel, usize)> {
        self.panels.iter().find_map(|p| {
            let j = triangle.checked_sub(p.first_triangle)?;
            (j < p.triangles.len()).then_some((p, j))
        })
    }

    /// Which shape, and where on it (mm), the point at barycentric `bary` of cloth triangle
    /// `triangle` is.
    pub fn pattern_point(&self, triangle: usize, bary: [f64; 3]) -> Option<(PieceId, Point2)> {
        let (panel, j) = self.triangle(triangle)?;
        let t = panel.triangles[j];
        let f = (0..3).fold(DVec2::ZERO, |acc, k| {
            acc + DVec2::from_array(panel.flat[t[k] as usize]) * bary[k]
        });
        Some((panel.shape, Point2::new(f.x * 1000.0, f.y * 1000.0)))
    }

    /// The cloth triangle holding point `at` (mm, on shape `shape`), with the point's
    /// barycentric coordinates in it: the triangle it is in, or, just outside the fabric, the
    /// nearest one, with the point moved onto it.
    pub fn cloth_point(&self, shape: PieceId, at: Point2) -> Option<(usize, [f64; 3])> {
        let panel = self.panel(shape)?;
        let f = DVec2::new(at.x / 1000.0, at.y / 1000.0);
        let (j, bary) = live::nearest_triangle(panel, f)?;
        let clamped = bary.map(|b| b.max(0.0));
        let sum: f64 = clamped.iter().sum();
        Some((
            panel.first_triangle + j,
            if sum > 0.0 {
                clamped.map(|b| b / sum)
            } else {
                [1.0 / 3.0; 3]
            },
        ))
    }
}

/// A drape of a project: its solver, what the student should know about it, where its fabric
/// came from, the project it was made from, and its pins as they are held in the cloth.
pub struct Drape {
    pub solver: Solver,
    pub notes: Vec<DrapeNote>,
    pub fabric: Arc<Fabric>,
    pub project: Arc<Project>,
    pins: Vec<AttachmentId>,
}

impl Drape {
    /// The drape of `project` on `stage`: every shape that could be meshed, at its placement,
    /// sewn by its seams, its pins held at their targets.
    pub fn new(project: Arc<Project>, stage: &Stage) -> Self {
        Self::make(project, stage, None)
    }

    /// The drape of `project`, an edit of this drape's project, carrying on from where this one
    /// has got to (see `live`): a shape that was already draped starts where it was, a new one
    /// at its placement.
    pub fn rebuilt(&self, project: Arc<Project>, stage: &Stage) -> Self {
        Self::make(project, stage, Some(self))
    }

    fn make(project: Arc<Project>, stage: &Stage, from: Option<&Drape>) -> Self {
        let mesh = opendrape_mesh::build(&project, &MeshParams::default());
        let fabric = Fabric::of(&mesh);
        let shapes = geom::shapes(&project);
        let layout = place::layout(&shapes);
        let mut notes: Vec<DrapeNote> = mesh.notes.iter().map(|n| DrapeNote::Mesh(*n)).collect();
        let mut builder = ClothBuilder::new(DENSITY_KG_M2);
        let mut ids = Vec::with_capacity(mesh.panels.len());
        for panel in &mesh.panels {
            let warm = from.and_then(|old| live::warm_positions(old, panel));
            let positions = warm.unwrap_or_else(|| {
                let shape = shapes
                    .iter()
                    .find(|s| s.id == panel.shape)
                    .expect("every panel comes from a shape");
                let placement = place::effective(&project, shape, &layout, stage.shoulder_y());
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
                positions
            });
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
        let cloth = builder.build();
        let params = match from {
            None => Params::default(),
            Some(_) => live::warm_params(&cloth),
        };
        let mut drape = Self {
            solver: Solver::new(cloth, params),
            notes,
            fabric: Arc::new(fabric),
            project,
            pins: Vec::new(),
        };
        drape.hold_pins();
        drape
    }

    /// Whether `project` makes the same fabric as this drape's: they differ at most in pins and
    /// placements (which don't apply while draping).
    pub fn same_fabric(&self, project: &Project) -> bool {
        let pattern = |p: &Project| {
            let mut p = p.clone();
            p.pins.clear();
            for piece in &mut p.pieces {
                piece.placement = None;
                if let Some(t) = &mut piece.twin {
                    t.placement = None;
                }
            }
            p
        };
        pattern(&self.project) == pattern(project)
    }

    /// Holds `project`'s pins in place of this drape's own (its fabric is unchanged: see
    /// [`Self::same_fabric`]).
    pub fn set_pins(&mut self, project: Arc<Project>) {
        for id in std::mem::take(&mut self.pins) {
            self.solver.cloth_mut().detach(id);
        }
        self.project = project;
        self.hold_pins();
    }

    /// Attaches each of the project's pins to its spot of the fabric.
    fn hold_pins(&mut self) {
        let shapes = geom::shapes(&self.project);
        for pin in &self.project.pins {
            let Some(shape) = shapes.iter().find(|s| s.id == pin.shape) else {
                continue;
            };
            let spot = shape.spot_shown(pin.half, pin.at);
            let Some((triangle, bary)) = self.fabric.cloth_point(pin.shape, spot) else {
                continue;
            };
            let target = DVec3::from_array(pin.target);
            if let Some(id) = self
                .solver
                .cloth_mut()
                .attach(triangle, bary, target, PIN_COMPLIANCE)
            {
                self.pins.push(id);
            }
        }
    }
}

/// The fabric and cloth for `project` on `stage`: every shape that could be meshed, at its
/// placement, sewn by its seams, its pins held; and the notes about it.
pub fn build_drape(project: &Project, stage: &Stage) -> (Solver, Vec<DrapeNote>) {
    let drape = Drape::new(Arc::new(project.clone()), stage);
    (drape.solver, drape.notes)
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
