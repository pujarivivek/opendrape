use crate::Stage;
use crate::live;
use crate::quality::DrapeQuality;
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
    /// This seam hadn't closed when the drape stopped waiting for it, so it was pulled shut.
    SeamDidNotClose(opendrape_core::SeamId),
}

/// One panel of a drape's fabric: the shape it was made from, its flat points (m, on the
/// pattern table) and its own triangles, and where its particles and triangles start in the
/// cloth.
#[derive(Clone, Debug, PartialEq)]
pub struct FabricPanel {
    pub shape: PieceId,
    pub flat: Vec<[f64; 2]>,
    pub triangles: Vec<[u32; 3]>,
    /// The grain as a unit direction on the pattern table: the fabric's warp.
    pub grain: [f64; 2],
    pub first_particle: usize,
    pub first_triangle: usize,
    /// How far (mm, on the pattern) each point is from the nearest sewn stretch of the
    /// panel's outline: 0 along a seam, so the view can draw the stitch lines.
    /// [`NO_SEAM_MM`] everywhere on a panel nothing is sewn to.
    pub seam_mm: Vec<f32>,
}

/// The seam distance of a point nowhere near a seam (mm).
pub const NO_SEAM_MM: f32 = 1.0e4;

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
            .enumerate()
            .map(|(k, p)| {
                let panel = FabricPanel {
                    shape: p.shape,
                    flat: p.flat.clone(),
                    triangles: p.triangles.clone(),
                    grain: p.grain,
                    first_particle: particle,
                    first_triangle: triangle,
                    seam_mm: seam_distances(mesh, k),
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

    /// Every particle's distance (mm) from the nearest sewn seam on its piece, in cloth
    /// order, for the view to draw the stitch lines by.
    pub fn seam_mm(&self) -> Vec<f32> {
        self.panels
            .iter()
            .flat_map(|p| p.seam_mm.iter().copied())
            .collect()
    }

    /// Every particle's place on its fabric's weave (m), in cloth order: across the grain,
    /// then along it, so that a texture laid over them runs with the weft and the warp of
    /// every panel however it was cut.
    pub fn weave_m(&self) -> Vec<[f32; 2]> {
        self.panels
            .iter()
            .flat_map(|p| weave_of(&p.flat, p.grain))
            .collect()
    }

    /// The cloth's triangles as the fabric was cut, over the particles each panel started
    /// with (before any weld joined them), in cloth order.
    pub fn cut_triangles(&self) -> Vec<[u32; 3]> {
        self.panels
            .iter()
            .flat_map(|p| {
                let first = p.first_particle as u32;
                p.triangles.iter().map(move |t| t.map(|k| k + first))
            })
            .collect()
    }

    /// The live particle each of the fabric's particles has become after welding, read off
    /// `triangles` (the cloth's, which welding renumbers in place): itself until a weld joins
    /// it to another.
    pub fn live_map(&self, triangles: &[[u32; 3]]) -> Vec<u32> {
        let n: usize = self.panels.iter().map(|p| p.flat.len()).sum();
        let mut live: Vec<u32> = (0..n as u32).collect();
        for p in &self.panels {
            for (j, cut) in p.triangles.iter().enumerate() {
                let Some(now) = triangles.get(p.first_triangle + j) else {
                    continue;
                };
                for k in 0..3 {
                    live[p.first_particle + cut[k] as usize] = now[k];
                }
            }
        }
        live
    }
}

/// Where each of a panel's flat points (m, on the pattern table) lies on its fabric's weave
/// (m): across the grain, then along it, for `grain` the panel's warp direction.
pub fn weave_of(flat: &[[f64; 2]], grain: [f64; 2]) -> Vec<[f32; 2]> {
    let warp = DVec2::from_array(grain).normalize_or_zero();
    let warp = if warp == DVec2::ZERO { DVec2::Y } else { warp };
    let weft = DVec2::new(warp.y, -warp.x);
    flat.iter()
        .map(|f| {
            let f = DVec2::from_array(*f);
            [f.dot(weft) as f32, f.dot(warp) as f32]
        })
        .collect()
}

/// How far (mm) each point of panel `k` of `mesh` is from the nearest stretch of its outline
/// that a seam sews: two outline points in a row that are both stitched make such a stretch.
fn seam_distances(mesh: &GarmentMesh, k: usize) -> Vec<f32> {
    let panel = &mesh.panels[k];
    let mut stitched = vec![false; panel.flat.len()];
    for &((pa, a), (pb, b)) in &mesh.stitches {
        if pa == k {
            stitched[a as usize] = true;
        }
        if pb == k {
            stitched[b as usize] = true;
        }
    }
    let outline: Vec<u32> = panel.edges.iter().flatten().copied().collect();
    let mut segments: Vec<([f64; 2], [f64; 2])> = Vec::new();
    // Each edge runs from its start corner to its end corner and the next edge starts at
    // that corner, so the flattened outline repeats every corner and comes back round to the
    // first: every stretch between points in a row is covered, the repeats skipped.
    for w in outline.windows(2) {
        let (a, b) = (w[0] as usize, w[1] as usize);
        if a != b && stitched[a] && stitched[b] {
            segments.push((panel.flat[a], panel.flat[b]));
        }
    }
    if segments.is_empty() {
        return vec![NO_SEAM_MM; panel.flat.len()];
    }
    panel
        .flat
        .iter()
        .map(|p| {
            let p = DVec2::from_array(*p);
            let nearest = segments
                .iter()
                .map(|&(a, b)| {
                    let (a, b) = (DVec2::from_array(a), DVec2::from_array(b));
                    let ab = b - a;
                    let t = if ab.length_squared() > 0.0 {
                        ((p - a).dot(ab) / ab.length_squared()).clamp(0.0, 1.0)
                    } else {
                        0.0
                    };
                    p.distance(a + ab * t)
                })
                .fold(f64::MAX, f64::min);
            (nearest * 1000.0) as f32
        })
        .collect()
}

impl Fabric {
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
    /// The seam each of the solver's stitch groups sews (a mirror image shares its original's
    /// id).
    seams: Vec<opendrape_core::SeamId>,
    /// How the fabric was made and how it is simulated at Play, kept for a rebuild.
    mesh_params: MeshParams,
    params: Params,
}

impl Drape {
    /// The drape of `project` on `stage` at Normal detail: every shape that could be meshed,
    /// at its placement, sewn by its seams, its pins held at their targets.
    pub fn new(project: Arc<Project>, stage: &Stage) -> Self {
        Self::at_quality(project, stage, DrapeQuality::default())
    }

    /// As [`Drape::new`], at the detail of `quality`.
    pub fn at_quality(project: Arc<Project>, stage: &Stage, quality: DrapeQuality) -> Self {
        Self::with(project, stage, &quality.mesh_params(), quality.params())
    }

    /// As [`Drape::new`], with the fabric meshed by `mesh` and simulated with `params` (for
    /// benchmarks).
    pub fn with(project: Arc<Project>, stage: &Stage, mesh: &MeshParams, params: Params) -> Self {
        Self::make(project, stage, None, mesh, params)
    }

    /// What the solver has found since the last call: a seam that would not close by itself
    /// and was pulled shut.
    pub fn take_notes(&mut self) -> Vec<DrapeNote> {
        let mut out = Vec::new();
        for note in self.solver.take_notes() {
            let opendrape_sim::SolverNote::SeamForcedShut { group, .. } = note;
            if let Some(&seam) = self.seams.get(group as usize) {
                let n = DrapeNote::SeamDidNotClose(seam);
                if !out.contains(&n) {
                    out.push(n);
                }
            }
        }
        out
    }

    /// The drape of `project`, an edit of this drape's project, carrying on from where this one
    /// has got to (see `live`): a shape that was already draped starts where it was, a new one
    /// at its placement.
    pub fn rebuilt(&self, project: Arc<Project>, stage: &Stage) -> Self {
        Self::make(project, stage, Some(self), &self.mesh_params, self.params)
    }

    fn make(
        project: Arc<Project>,
        stage: &Stage,
        from: Option<&Drape>,
        mesh_params: &MeshParams,
        params: Params,
    ) -> Self {
        let mesh = opendrape_mesh::build(&project, mesh_params);
        let fabric = Fabric::of(&mesh);
        let shapes = geom::shapes(&project);
        let was = from.map(|old| geom::shapes(&old.project));
        let layout = place::layout(&shapes);
        let mut notes: Vec<DrapeNote> = mesh.notes.iter().map(|n| DrapeNote::Mesh(*n)).collect();
        let mut builder = ClothBuilder::new(DENSITY_KG_M2);
        let mut ids = Vec::with_capacity(mesh.panels.len());
        for panel in &mesh.panels {
            let shape = shapes
                .iter()
                .find(|s| s.id == panel.shape)
                .expect("every panel comes from a shape");
            // A piece dragged across the pattern table since: its fabric goes with it.
            let steps = was
                .iter()
                .flatten()
                .find(|s| s.id == panel.shape)
                .map_or_else(|| vec![DVec2::ZERO], |was| live::steps(was, shape));
            let warm = from.and_then(|old| live::warm_positions(old, panel, &steps));
            let positions = warm.unwrap_or_else(|| {
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
            ids.push(builder.add_grain_panel(
                &Panel {
                    positions,
                    flat: Some(panel.flat.iter().map(|f| DVec2::from_array(*f)).collect()),
                    triangles: panel.triangles.clone(),
                },
                1.0,
                DVec2::from_array(panel.grain),
            ));
        }
        // Each seam (and each mirror image) is a stitch group of its own: it welds on its own.
        let mut seams: Vec<(opendrape_core::SeamId, bool)> = Vec::new();
        for (&((pa, a), (pb, b)), &seam) in mesh.stitches.iter().zip(&mesh.stitch_seams) {
            let group = seams.iter().position(|s| *s == seam).unwrap_or_else(|| {
                seams.push(seam);
                seams.len() - 1
            });
            builder.stitch_in((ids[pa], a), (ids[pb], b), group as u32);
        }
        let cloth = builder.build();
        let params = match from {
            None => params,
            Some(_) => live::warm_params(params),
        };
        let mut drape = Self {
            solver: Solver::new(cloth, params),
            notes,
            fabric: Arc::new(fabric),
            project,
            pins: Vec::new(),
            seams: seams.into_iter().map(|(id, _)| id).collect(),
            mesh_params: *mesh_params,
            params,
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

    /// How many pins are held in the cloth: the project's, less any on a piece that was left out
    /// of the fabric.
    pub fn held_pins(&self) -> usize {
        self.pins.len()
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
    #[test]
    fn every_point_knows_how_far_the_nearest_seam_is() {
        let stage = Stage::shared();
        let drape = Drape::new(Arc::new(two_panels()), &stage);
        let seams = drape.fabric.seam_mm();
        assert_eq!(seams.len(), drape.solver.cloth().len());
        let stitched: Vec<usize> = drape
            .solver
            .cloth()
            .stitch_pairs()
            .flat_map(|(a, b)| [a, b])
            .collect();
        assert!(!stitched.is_empty());
        assert!(
            stitched.iter().all(|&i| seams[i] < 1e-3),
            "stitched points are on the seam"
        );
        let far = seams.iter().copied().fold(0.0, f32::max);
        assert!(
            far > 50.0 && far < NO_SEAM_MM,
            "the far side of a sewn panel: {far}"
        );
        // A panel nothing is sewn to is nowhere near a seam.
        let mut pr = Project::new();
        pr.add_piece(opendrape_core::Piece::rectangle(
            PieceId(0),
            "Alone",
            Point2::new(0.0, 0.0),
            200.0,
            200.0,
        ));
        let alone = Drape::new(Arc::new(pr), &stage);
        assert!(alone.fabric.seam_mm().iter().all(|&d| d == NO_SEAM_MM));
    }

    #[test]
    fn the_weave_runs_with_each_panels_grain_and_welds_map_onto_live_particles() {
        let stage = Stage::shared();
        let mut drape = Drape::new(Arc::new(two_panels()), &stage);
        let fabric = drape.fabric.clone();
        let n = drape.solver.cloth().len();
        // On the straight grain (up the pattern), the weave is the pattern itself.
        let weave = fabric.weave_m();
        assert_eq!(weave.len(), n);
        for p in &fabric.panels {
            assert!(
                p.grain[0].abs() < 1e-9 && (p.grain[1] - 1.0).abs() < 1e-9,
                "{:?}",
                p.grain
            );
            for (k, f) in p.flat.iter().enumerate() {
                let w = weave[p.first_particle + k];
                assert!((w[0] as f64 - f[0]).abs() < 1e-6 && (w[1] as f64 - f[1]).abs() < 1e-6);
            }
        }
        // Before any weld the cut triangles are the cloth's and every particle is its own.
        let cut = fabric.cut_triangles();
        assert_eq!(cut, drape.solver.cloth().triangles());
        let live = fabric.live_map(drape.solver.cloth().triangles());
        assert!(live.iter().enumerate().all(|(i, &l)| l as usize == i));
        // Welded, each stitched particle maps onto a live one, and the cut triangles still
        // describe the same fabric.
        let stitched: Vec<(usize, usize)> = drape.solver.cloth().stitch_pairs().collect();
        drape.solver.cloth_mut().weld_stitches();
        let cloth = drape.solver.cloth();
        let live = fabric.live_map(cloth.triangles());
        assert_eq!(live.len(), n);
        for &(a, b) in &stitched {
            assert_eq!(live[a], live[b], "sewn together");
            assert!(cloth.is_alive(live[a] as usize));
        }
        assert!(
            live.iter().any(|&l| l as usize != 0)
                && live.iter().enumerate().any(|(i, &l)| l as usize != i)
        );
        assert_eq!(cut.len(), cloth.triangles().len());
    }

    #[test]
    fn draft_makes_fewer_particles_and_a_rebuild_keeps_the_detail() {
        let stage = Stage::shared();
        let pr = Arc::new(two_panels());
        let normal = Drape::new(pr.clone(), &stage);
        let draft = Drape::at_quality(pr.clone(), &stage, DrapeQuality::Draft);
        assert!(
            draft.solver.cloth().len() * 2 < normal.solver.cloth().len(),
            "{} draft vs {} normal",
            draft.solver.cloth().len(),
            normal.solver.cloth().len()
        );
        assert_eq!(draft.solver.params(), &DrapeQuality::Draft.params());
        let again = draft.rebuilt(pr, &stage);
        assert_eq!(again.solver.cloth().len(), draft.solver.cloth().len());
        assert_eq!(again.solver.params().self_collision_every, 2);
        assert_eq!(again.solver.params().gravity_delay, 0.0, "warm");
    }

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
            solver.step(Some(collider));
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
