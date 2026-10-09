//! The gizmo drawn over the 3D view with egui's painter: rings to turn, arrows to move along
//! an axis, a square to move facing the viewer. The handle under the pointer, or being
//! dragged, is drawn bright.

use super::gizmo::{Gizmo, Handle, SQUARE_PT, ScreenCamera};
use egui::{Color32, Painter, Pos2, Rect, Stroke, StrokeKind, pos2, vec2};
use glam::DVec2;

const AXIS_COLOURS: [Color32; 3] = [
    Color32::from_rgb(220, 60, 60),
    Color32::from_rgb(60, 170, 70),
    Color32::from_rgb(60, 110, 235),
];
const BRIGHT: Color32 = Color32::from_rgb(255, 200, 0);

fn screen(p: DVec2) -> Pos2 {
    pos2(p.x as f32, p.y as f32)
}

/// Draws `gizmo`, with `lit` (the handle under the pointer, or being dragged) bright.
pub fn paint(painter: &Painter, cam: &ScreenCamera, gizmo: &Gizmo, lit: Option<Handle>) {
    let Some(centre) = cam.project(gizmo.centre) else {
        return;
    };
    let colour = |handle: Handle, axis: usize| {
        if lit == Some(handle) {
            (BRIGHT, 3.5)
        } else {
            (AXIS_COLOURS[axis], 2.0)
        }
    };
    for axis in 0..3 {
        let (c, w) = colour(Handle::Turn(axis), axis);
        let ring: Vec<Pos2> = gizmo
            .ring(axis)
            .into_iter()
            .filter_map(|p| cam.project(p))
            .map(screen)
            .collect();
        painter.add(egui::Shape::closed_line(ring, Stroke::new(w, c)));
    }
    for axis in 0..3 {
        if !gizmo.arrow_shown(cam, axis) {
            continue;
        }
        if let Some(tip) = cam.project(gizmo.arrow_tip(axis)) {
            let (c, w) = colour(Handle::Move(axis), axis);
            let d = tip - centre;
            painter.arrow(
                screen(centre),
                vec2(d.x as f32, d.y as f32),
                Stroke::new(w + 1.0, c),
            );
        }
    }
    let square = Rect::from_center_size(screen(centre), vec2(2.0, 2.0) * SQUARE_PT as f32);
    let fill = if lit == Some(Handle::Plane) {
        BRIGHT
    } else {
        Color32::from_white_alpha(170)
    };
    painter.rect(
        square,
        2.0,
        fill,
        Stroke::new(1.0, Color32::from_gray(60)),
        StrokeKind::Middle,
    );
}

/// Draws the drag's readout beside the pointer.
pub fn paint_readout(painter: &Painter, at: Pos2, text: String) {
    painter.text(
        at + vec2(16.0, -16.0),
        egui::Align2::LEFT_BOTTOM,
        text,
        egui::FontId::proportional(14.0),
        Color32::from_gray(30),
    );
}
