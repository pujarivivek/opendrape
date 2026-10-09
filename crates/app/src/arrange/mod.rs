//! Arranging pieces in 3D before draping: the view's maths (`gizmo`), the pieces as shown
//! (`scene`), what the pointer does to them (`Arranger`), and the gizmo drawn over the view
//! (`overlay`).

pub mod gizmo;
pub mod overlay;
pub mod scene;

pub use gizmo::ScreenCamera;
pub use scene::{ArrangedPanel, ArrangedScene, SceneCache};

use crate::editor::{Document, Selection};
use crate::tr;
use gizmo::{AXES, Gizmo, Handle, axis_drag, plane_drag, ring_angle, snap_angle};
use glam::{DQuat, DVec2, DVec3};
use opendrape_core::{PieceId, Placement, Units};

/// Shift snaps a turn to steps of this many degrees.
pub const SNAP_DEG: f64 = 15.0;

/// What a gizmo drag has done so far.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Moved {
    /// Metres along axis 0 (x), 1 (y) or 2 (z).
    Along(usize, f64),
    /// Metres in the plane facing the viewer.
    Across(DVec3),
    /// Radians about an axis.
    Turned(f64),
}

struct GizmoDrag {
    handle: Handle,
    shape: PieceId,
    /// The piece's placement when the drag began.
    original: Placement,
    start: DVec2,
    moved: Option<Moved>,
}

/// What the pointer does in the 3D view while arranging: clicking picks a piece, and the
/// selected piece's gizmo moves and turns it, one undo step per drag.
#[derive(Default)]
pub struct Arranger {
    /// The gizmo handle under the pointer, drawn highlighted.
    pub hovered: Option<Handle>,
    drag: Option<GizmoDrag>,
}

impl Arranger {
    /// The selected piece's gizmo, when a piece shown in `scene` is selected.
    pub fn gizmo(
        cam: &ScreenCamera,
        scene: &ArrangedScene,
        selection: &Selection,
    ) -> Option<Gizmo> {
        let Selection::Piece(id) = *selection else {
            return None;
        };
        let panel = scene.panel(id)?;
        Some(Gizmo::new(cam, DVec3::from_array(panel.placement.position)))
    }

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

    /// The pointer moved with no button down: note the handle under it.
    pub fn hover(
        &mut self,
        cam: &ScreenCamera,
        scene: &ArrangedScene,
        selection: &Selection,
        pos: DVec2,
    ) {
        self.hovered = Self::gizmo(cam, scene, selection).and_then(|g| g.hit(cam, pos));
    }

    /// A drag starts at `pos`: on a handle of the selected piece's gizmo, it grabs it and
    /// returns true (the drag moves the piece); anywhere else it returns false (the drag turns
    /// the camera).
    pub fn press(
        &mut self,
        cam: &ScreenCamera,
        scene: &ArrangedScene,
        selection: &Selection,
        doc: &mut Document,
        pos: DVec2,
    ) -> bool {
        let Selection::Piece(shape) = *selection else {
            return false;
        };
        let (Some(gizmo), Some(panel)) = (Self::gizmo(cam, scene, selection), scene.panel(shape))
        else {
            return false;
        };
        let Some(handle) = gizmo.hit(cam, pos) else {
            return false;
        };
        doc.begin_gesture();
        self.drag = Some(GizmoDrag {
            handle,
            shape,
            original: panel.placement,
            start: pos,
            moved: None,
        });
        true
    }

    /// The pointer moved to `pos` during a gizmo drag: the piece follows. Shift snaps a turn
    /// to 15° steps.
    pub fn drag_to(&mut self, cam: &ScreenCamera, doc: &mut Document, pos: DVec2, shift: bool) {
        let Some(d) = &mut self.drag else { return };
        let o = d.original;
        let centre = DVec3::from_array(o.position);
        let (moved, placement) = match d.handle {
            Handle::Move(k) => {
                let Some(m) = axis_drag(cam, centre, AXES[k], d.start, pos) else {
                    return;
                };
                let to = centre + AXES[k] * m;
                (
                    Moved::Along(k, m),
                    Placement {
                        position: to.to_array(),
                        ..o
                    },
                )
            }
            Handle::Plane => {
                let Some(v) = plane_drag(cam, centre, d.start, pos) else {
                    return;
                };
                (
                    Moved::Across(v),
                    Placement {
                        position: (centre + v).to_array(),
                        ..o
                    },
                )
            }
            Handle::Turn(k) => {
                let Some(mut a) = ring_angle(cam, centre, AXES[k], d.start, pos) else {
                    return;
                };
                if shift {
                    a = snap_angle(a, SNAP_DEG);
                }
                let turned = DQuat::from_axis_angle(AXES[k], a) * DQuat::from_array(o.rotation);
                (
                    Moved::Turned(a),
                    Placement {
                        rotation: turned.normalize().to_array(),
                        ..o
                    },
                )
            }
        };
        d.moved = Some(moved);
        let shape = d.shape;
        doc.gesture_edit(|p| p.set_placement(shape, Some(placement)));
    }

    /// The drag ended: everything it did is one undo step.
    pub fn release(&mut self, doc: &mut Document) {
        if self.drag.take().is_some() {
            doc.end_gesture();
        }
    }

    /// A gizmo drag is under way.
    pub fn is_dragging(&self) -> bool {
        self.drag.is_some()
    }

    /// The handle being dragged.
    pub fn active(&self) -> Option<Handle> {
        self.drag.as_ref().map(|d| d.handle)
    }

    /// What the drag has done, as the student reads it: "12.0 cm up", "45°".
    pub fn readout(&self, units: Units) -> Option<String> {
        let moved = self.drag.as_ref()?.moved?;
        let length = |m: f64| units.format(m.abs() * 1000.0);
        Some(match moved {
            Moved::Along(0, m) if m >= 0.0 => tr!("gizmo-left", distance = length(m)),
            Moved::Along(0, m) => tr!("gizmo-right", distance = length(m)),
            Moved::Along(1, m) if m >= 0.0 => tr!("gizmo-up", distance = length(m)),
            Moved::Along(1, m) => tr!("gizmo-down", distance = length(m)),
            Moved::Along(_, m) if m >= 0.0 => tr!("gizmo-forward", distance = length(m)),
            Moved::Along(_, m) => tr!("gizmo-back", distance = length(m)),
            Moved::Across(v) => tr!("gizmo-moved", distance = length(v.length())),
            Moved::Turned(a) => tr!("gizmo-turned", angle = format!("{:.0}", a.to_degrees())),
        })
    }
}
