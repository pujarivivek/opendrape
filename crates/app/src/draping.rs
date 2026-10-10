//! What the pointer does in the 3D view while the garment drapes:
//! - press on the fabric and drag to pull the point under the pointer, in the plane facing the
//!   viewer at the depth it was grabbed (a grab is no edit, and no undo step);
//! - right-click the fabric for **Pin here**, which pins that spot where it is now;
//! - drag a pin's marker to move where it holds the fabric (one undo step);
//! - click a marker to select its pin, or right-click it for **Remove pin**.
//!
//! It works on the drape's latest frame and a camera, with no egui in it, so the tests drive it
//! with a fixed camera and no window. What a grab asks of the simulation it hands back as a
//! [`Pull`] for the caller to pass on.

use crate::arrange::ScreenCamera;
use crate::arrange::gizmo::{GRAB_PT, plane_drag};
use crate::editor::{Document, Selection};
use crate::sim_runner::SimFrame;
use glam::{DVec2, DVec3};
use opendrape_core::{Pin, Project};
use opendrape_drape::Fabric;
use opendrape_geom as geom;
use std::sync::Arc;

/// A point of the drape's cloth: a triangle of the frame's cloth, the point's barycentric
/// coordinates in it, and where it is (m).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FabricHit {
    pub triangle: usize,
    pub bary: [f64; 3],
    pub point: DVec3,
}

/// What a grab asks of the simulation.
#[derive(Clone, Debug, PartialEq)]
pub enum Pull {
    /// Pull this point of the cloth made from `fabric` to `target`.
    Grab {
        fabric: Arc<Fabric>,
        triangle: usize,
        bary: [f64; 3],
        target: DVec3,
    },
    /// The grabbed point's target moves.
    To(DVec3),
    /// Let go.
    Release,
}

/// What a right-click in the 3D view was on, for its menu.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum MenuAt {
    /// The fabric: the pin **Pin here** adds.
    Fabric(Pin),
    /// A pin's marker: its index in the project's pins.
    Pin(usize),
}

enum Drag {
    /// Pulling the fabric: the point grabbed, and where the press was.
    Grab { point: DVec3, start: DVec2 },
    /// Moving pin `index`, whose target was `target` when the press was at `start`.
    Pin {
        index: usize,
        target: DVec3,
        start: DVec2,
    },
}

/// What the pointer does in the 3D view while draping.
#[derive(Default)]
pub struct Draper {
    drag: Option<Drag>,
    /// What the last right-click was on: the 3D view's menu is for it.
    pub menu: Option<MenuAt>,
}

/// Where the ray from `origin` along unit `dir` meets triangle `t`: its distance, and the
/// barycentric coordinates of the point it meets.
fn ray_hit(origin: DVec3, dir: DVec3, t: [DVec3; 3]) -> Option<(f64, [f64; 3])> {
    let (e1, e2) = (t[1] - t[0], t[2] - t[0]);
    let p = dir.cross(e2);
    let det = e1.dot(p);
    if det.abs() < 1e-12 {
        return None;
    }
    let inv = 1.0 / det;
    let s = origin - t[0];
    let u = s.dot(p) * inv;
    let q = s.cross(e1);
    let v = dir.dot(q) * inv;
    if !(0.0..=1.0).contains(&u) || v < 0.0 || u + v > 1.0 {
        return None;
    }
    let d = e2.dot(q) * inv;
    (d > 1e-9).then_some((d, [1.0 - u - v, u, v]))
}

/// The point of `frame`'s cloth that the ray from `origin` along unit `dir` meets first.
pub fn pick_fabric(frame: &SimFrame, origin: DVec3, dir: DVec3) -> Option<FabricHit> {
    let at = |k: u32| frame.positions[k as usize].as_dvec3();
    frame
        .triangles
        .iter()
        .enumerate()
        .filter_map(|(triangle, t)| {
            let corners = t.map(at);
            let (d, bary) = ray_hit(origin, dir, corners)?;
            Some((d, triangle, bary))
        })
        .min_by(|a, b| a.0.total_cmp(&b.0))
        .map(|(d, triangle, bary)| FabricHit {
            triangle,
            bary,
            point: origin + dir * d,
        })
}

/// The pin whose marker (drawn where it holds the fabric) is within [`GRAB_PT`] of screen
/// point `pos`, the nearest.
pub fn pin_at(cam: &ScreenCamera, project: &Project, pos: DVec2) -> Option<usize> {
    project
        .pins
        .iter()
        .enumerate()
        .filter_map(|(k, pin)| {
            let shown = cam.project(DVec3::from_array(pin.target))?;
            let d = shown.distance(pos);
            (d <= GRAB_PT).then_some((k, d))
        })
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(k, _)| k)
}

/// The pin **Pin here** makes at `hit`: on the spot of the pattern there (kept on its stored
/// piece and half, see `Shape::pin_spot`), holding it where it is now.
pub fn pin_here(project: &Project, fabric: &Fabric, hit: &FabricHit) -> Option<Pin> {
    let (shape, on_shape) = fabric.pattern_point(hit.triangle, hit.bary)?;
    let (half, at) = geom::shape_of(project, shape)?.pin_spot(on_shape);
    Some(Pin {
        shape,
        half,
        at,
        target: hit.point.to_array(),
    })
}

impl Draper {
    /// A pin is being moved, or the fabric pulled: the camera stays put.
    pub fn is_dragging(&self) -> bool {
        self.drag.is_some()
    }

    /// The primary button went down at `pos`. On a pin's marker it starts moving the pin
    /// (everything until [`Self::release`] is one undo step); on the fabric it grabs it, and
    /// hands back the grab. Anywhere else it does nothing (the drag turns the camera).
    pub fn press(
        &mut self,
        cam: &ScreenCamera,
        frame: &SimFrame,
        doc: &mut Document,
        pos: DVec2,
    ) -> Option<Pull> {
        if let Some(index) = pin_at(cam, doc.project(), pos) {
            doc.begin_gesture();
            self.drag = Some(Drag::Pin {
                index,
                target: DVec3::from_array(doc.project().pins[index].target),
                start: pos,
            });
            return None;
        }
        let (origin, dir) = cam.ray(pos);
        let hit = pick_fabric(frame, origin, dir)?;
        self.drag = Some(Drag::Grab {
            point: hit.point,
            start: pos,
        });
        Some(Pull::Grab {
            fabric: frame.fabric.clone(),
            triangle: hit.triangle,
            bary: hit.bary,
            target: hit.point,
        })
    }

    /// The pointer moved to `pos` with the button down: the grabbed point's target, or the pin
    /// being moved, follows it in the plane facing the viewer at its depth. A pin that would go
    /// where no pin may be (more than 10 m away) stays where it last could.
    pub fn drag_to(&mut self, cam: &ScreenCamera, doc: &mut Document, pos: DVec2) -> Option<Pull> {
        match self.drag.as_ref()? {
            Drag::Grab { point, start } => {
                let moved = plane_drag(cam, *point, *start, pos)?;
                Some(Pull::To(*point + moved))
            }
            Drag::Pin {
                index,
                target,
                start,
            } => {
                let to = (*target + plane_drag(cam, *target, *start, pos)?).to_array();
                let index = *index;
                doc.gesture_edit(|p| {
                    if let Some(pin) = p.pins.get_mut(index) {
                        pin.target = to;
                    }
                });
                None
            }
        }
    }

    /// The button came up: a grab lets go; a pin's move is one undo step.
    pub fn release(&mut self, doc: &mut Document) -> Option<Pull> {
        match self.drag.take()? {
            Drag::Grab { .. } => Some(Pull::Release),
            Drag::Pin { .. } => {
                doc.end_gesture();
                None
            }
        }
    }

    /// A click (no drag) at `pos`: a pin's marker selects its pin.
    pub fn click(
        &mut self,
        cam: &ScreenCamera,
        project: &Project,
        selection: &mut Selection,
        pos: DVec2,
    ) {
        if let Some(k) = pin_at(cam, project, pos) {
            *selection = Selection::Pin(k);
        }
    }

    /// A right-click at `pos`: on a pin's marker its menu offers **Remove pin**; on the fabric,
    /// **Pin here**; anywhere else there is no menu.
    pub fn secondary_click(
        &mut self,
        cam: &ScreenCamera,
        frame: &SimFrame,
        project: &Project,
        pos: DVec2,
    ) {
        self.menu = match pin_at(cam, project, pos) {
            Some(k) => Some(MenuAt::Pin(k)),
            None => {
                let (origin, dir) = cam.ray(pos);
                pick_fabric(frame, origin, dir)
                    .and_then(|hit| pin_here(project, &frame.fabric, &hit))
                    .map(MenuAt::Fabric)
            }
        };
    }
}
