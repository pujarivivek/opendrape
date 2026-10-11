//! The pieces as the 3D view shows them while arranging: each shape's fabric (made coarser
//! than for draping: it is only looked at and clicked) at its placement. The fabric is made
//! again only when the pattern changes; moving a piece only places it again. It is made on the
//! thread that draws the window, so a build that panics is caught here: the view keeps what it
//! had, and says so.

use super::gizmo::ray_triangle;
use crate::sim_runner::guarded;
use glam::DVec3;
use opendrape_core::{PieceId, Placement, Point2, Project};
use opendrape_geom as geom;
use opendrape_mesh::{GarmentMesh, MeshParams, place};
use std::rc::Rc;

/// Edge length (mm) of the fabric shown while arranging.
pub const VIEW_EDGE_MM: f64 = 20.0;

/// One piece as shown in 3D.
#[derive(Clone, Debug, PartialEq)]
pub struct ArrangedPanel {
    pub shape: PieceId,
    /// Where it is shown and draped from (its own placement, or the one it takes).
    pub placement: Placement,
    pub positions: Vec<DVec3>,
    pub triangles: Vec<[u32; 3]>,
    /// Each point's place on the fabric's weave (m), so the piece is drawn in its fabric.
    pub weave: Vec<[f32; 2]>,
}

/// Every piece that could be made into fabric, at its placement.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ArrangedScene {
    pub panels: Vec<ArrangedPanel>,
}

impl ArrangedScene {
    pub fn panel(&self, shape: PieceId) -> Option<&ArrangedPanel> {
        self.panels.iter().find(|p| p.shape == shape)
    }
    /// The piece the ray from `origin` along unit `dir` meets first.
    pub fn pick(&self, origin: DVec3, dir: DVec3) -> Option<PieceId> {
        self.panels
            .iter()
            .flat_map(|p| {
                p.triangles.iter().filter_map(move |t| {
                    ray_triangle(origin, dir, t.map(|k| p.positions[k as usize]))
                        .map(|d| (p.shape, d))
                })
            })
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .map(|(shape, _)| shape)
    }
}

/// The arranged scene, kept until the project changes.
#[derive(Default)]
pub struct SceneCache {
    /// The pattern the fabric was made from: the project with every placement taken out.
    pattern: Option<Project>,
    mesh: Rc<GarmentMesh>,
    /// The project and shoulder height the scene was placed for.
    placed: Option<(Project, f64)>,
    scene: Rc<ArrangedScene>,
    /// The fabric for the current pattern could not be made (making it panicked): the pieces
    /// shown are those of the last pattern that could be, or none.
    view_failed: bool,
    /// How many times the fabric has been made (tests count them).
    pub meshed: usize,
}

/// The fabric shown while arranging: `pattern` made at [`VIEW_EDGE_MM`].
fn view_mesh(pattern: &Project) -> GarmentMesh {
    opendrape_mesh::build(
        pattern,
        &MeshParams {
            edge_mm: VIEW_EDGE_MM,
            ..MeshParams::default()
        },
    )
}

impl SceneCache {
    /// The pieces of `project` at their placements, for a form whose shoulders are at
    /// `shoulder_y`. Reused while nothing changed; only re-placed when only placements did.
    pub fn scene(&mut self, project: &Project, shoulder_y: f64) -> Rc<ArrangedScene> {
        self.scene_with(project, shoulder_y, view_mesh)
    }

    /// The fabric for the current pattern could not be made: see [`Self::scene_with`].
    pub fn view_failed(&self) -> bool {
        self.view_failed
    }

    /// [`Self::scene`], with the fabric made by `build` (the one thing a test needs to change).
    ///
    /// If `build` panics, the view keeps the fabric of the last pattern it could make (none, if
    /// there was no such pattern), placed for `project`, and [`Self::view_failed`] is set until
    /// a later pattern is made. The failed pattern is not tried again every frame: only a change
    /// to the pattern tries again.
    pub(crate) fn scene_with(
        &mut self,
        project: &Project,
        shoulder_y: f64,
        build: impl FnOnce(&Project) -> GarmentMesh,
    ) -> Rc<ArrangedScene> {
        if self
            .placed
            .as_ref()
            .is_some_and(|(p, s)| p == project && *s == shoulder_y)
        {
            return self.scene.clone();
        }
        let pattern = without_placements(project);
        if self.pattern.as_ref() != Some(&pattern) {
            match guarded(|| build(&pattern)) {
                Some(mesh) => {
                    self.mesh = Rc::new(mesh);
                    self.view_failed = false;
                    self.meshed += 1;
                }
                None => self.view_failed = true,
            }
            self.pattern = Some(pattern);
        }
        let shapes = geom::shapes(project);
        let layout = place::layout(&shapes);
        let panels = self
            .mesh
            .panels
            .iter()
            .filter_map(|panel| {
                let shape = shapes.iter().find(|s| s.id == panel.shape)?;
                let placement = place::effective(project, shape, &layout, shoulder_y);
                let positions = panel
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
                Some(ArrangedPanel {
                    shape: panel.shape,
                    placement,
                    positions,
                    triangles: panel.triangles.clone(),
                    weave: opendrape_drape::weave_of(&panel.flat, panel.grain),
                })
            })
            .collect();
        self.scene = Rc::new(ArrangedScene { panels });
        self.placed = Some((project.clone(), shoulder_y));
        self.scene.clone()
    }
}

/// `project` with no placements: what the fabric's shape depends on.
fn without_placements(project: &Project) -> Project {
    let mut p = project.clone();
    for piece in &mut p.pieces {
        piece.placement = None;
        if let Some(t) = &mut piece.twin {
            t.placement = None;
        }
    }
    p
}

#[cfg(test)]
mod tests {
    use super::*;
    use opendrape_core::Piece;

    fn two_pieces() -> Project {
        let mut pr = Project::new();
        pr.add_piece(Piece::rectangle(
            PieceId(0),
            "Front",
            Point2::new(0.0, 0.0),
            300.0,
            400.0,
        ));
        pr.add_piece(Piece::rectangle(
            PieceId(0),
            "Back",
            Point2::new(500.0, 0.0),
            300.0,
            400.0,
        ));
        pr
    }

    #[test]
    fn moving_a_piece_places_it_again_without_remaking_the_fabric() {
        let mut cache = SceneCache::default();
        let mut pr = two_pieces();
        let first = cache.scene(&pr, 1.3);
        assert_eq!(first.panels.len(), 2);
        assert!(
            Rc::ptr_eq(&first, &cache.scene(&pr, 1.3)),
            "nothing changed"
        );
        pr.set_placement(PieceId(1), Some(Placement::at([0.0, 1.0, 0.5])));
        let moved = cache.scene(&pr, 1.3);
        assert_eq!(cache.meshed, 1);
        assert_eq!(
            moved.panel(PieceId(1)).unwrap().placement,
            Placement::at([0.0, 1.0, 0.5])
        );
        pr.pieces[0].name = "Front left".into();
        cache.scene(&pr, 1.3);
        assert_eq!(cache.meshed, 2, "a pattern change makes the fabric again");
    }

    #[test]
    fn a_build_that_panics_keeps_the_last_good_view_and_says_so() {
        let mut cache = SceneCache::default();
        let mut pr = two_pieces();
        assert_eq!(cache.scene(&pr, 1.3).panels.len(), 2);
        assert!(!cache.view_failed());
        // The pattern changes, and making the fabric for it panics.
        pr.pieces[0].name = "Front left".into();
        let kept = cache.scene_with(&pr, 1.3, |_| panic!("the mesher fell over"));
        assert!(cache.view_failed());
        assert_eq!(kept.panels.len(), 2, "the pieces of the last pattern made");
        assert_eq!(cache.meshed, 1, "nothing new was made");
        // Moving a piece still works on that fabric, and the failed pattern is not tried again.
        pr.set_placement(PieceId(1), Some(Placement::at([0.0, 1.0, 0.5])));
        let mut tried = 0;
        let moved = cache.scene_with(&pr, 1.3, |_| {
            tried += 1;
            GarmentMesh::default()
        });
        assert_eq!(tried, 0, "the same pattern is not made again every frame");
        assert!(cache.view_failed());
        assert_eq!(
            moved.panel(PieceId(1)).unwrap().placement,
            Placement::at([0.0, 1.0, 0.5])
        );
        // The next change to the pattern is tried, and when it can be made the note goes.
        pr.pieces[0].name = "Front right".into();
        assert_eq!(cache.scene(&pr, 1.3).panels.len(), 2);
        assert!(!cache.view_failed());
        assert_eq!(cache.meshed, 2);
    }

    #[test]
    fn a_build_that_panics_with_nothing_made_before_gives_an_empty_view() {
        let mut cache = SceneCache::default();
        let pr = two_pieces();
        let scene = cache.scene_with(&pr, 1.3, |_| panic!("the mesher fell over"));
        assert!(cache.view_failed());
        assert!(scene.panels.is_empty());
        assert_eq!(*scene, ArrangedScene::default());
    }

    #[test]
    fn a_ray_picks_the_nearest_piece() {
        let mut pr = two_pieces();
        // The back hangs 10 cm behind the front, right behind it.
        pr.set_placement(PieceId(1), Some(Placement::at([0.0, 1.0, 0.5])));
        pr.set_placement(PieceId(2), Some(Placement::at([0.0, 1.0, 0.4])));
        let scene = SceneCache::default().scene(&pr, 1.3);
        let from_front = (DVec3::new(0.0, 1.0, 3.0), DVec3::NEG_Z);
        assert_eq!(scene.pick(from_front.0, from_front.1), Some(PieceId(1)));
        let from_behind = (DVec3::new(0.0, 1.0, -3.0), DVec3::Z);
        assert_eq!(scene.pick(from_behind.0, from_behind.1), Some(PieceId(2)));
        assert_eq!(
            scene.pick(DVec3::new(2.0, 1.0, 3.0), DVec3::NEG_Z),
            None,
            "beside them"
        );
    }
}
