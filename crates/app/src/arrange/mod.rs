//! Arranging pieces in 3D before draping: the view's maths (`gizmo`), the pieces as shown
//! (`scene`), and what the pointer does to them (`Arranger`).

pub mod gizmo;
pub mod scene;

pub use gizmo::ScreenCamera;
pub use scene::{ArrangedPanel, ArrangedScene, SceneCache};

use crate::editor::Selection;
use glam::DVec2;

/// What the pointer does in the 3D view while arranging.
#[derive(Default)]
pub struct Arranger {}

impl Arranger {
    /// A click: the piece under the pointer becomes the selection (in the pattern window too);
    /// a click on nothing clears it.
    pub fn click(
        &mut self,
        cam: &ScreenCamera,
        scene: &ArrangedScene,
        selection: &mut Selection,
        pos: DVec2,
    ) {
        let (origin, dir) = cam.ray(pos);
        *selection = scene
            .pick(origin, dir)
            .map_or(Selection::None, Selection::Piece);
    }
}
