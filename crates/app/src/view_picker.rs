//! The camera views in the 3D view's top-right corner: four small pictures of the form, seen
//! from the front, the back, its left and its right. Clicking one turns the camera to look from
//! there; the one the camera is looking from now is highlighted.

use crate::tr;
use egui::{Pos2, Rect, Shape, Stroke, pos2, vec2};
use opendrape_mesh::place::PlaceAt;

/// The views, left to right.
const VIEWS: [PlaceAt; 4] = [
    PlaceAt::Front,
    PlaceAt::Back,
    PlaceAt::LeftSide,
    PlaceAt::RightSide,
];
/// One button (screen points), the gap between buttons, and the margin from the view's corner.
const BUTTON: egui::Vec2 = vec2(20.0, 24.0);
const GAP: f32 = 2.0;
const MARGIN: f32 = 6.0;

fn name(side: PlaceAt) -> String {
    match side {
        PlaceAt::Front => tr!("view-front"),
        PlaceAt::Back => tr!("view-back"),
        PlaceAt::LeftSide => tr!("view-left"),
        PlaceAt::RightSide => tr!("view-right"),
    }
}

fn tip(side: PlaceAt) -> String {
    match side {
        PlaceAt::Front => tr!("view-front-tip"),
        PlaceAt::Back => tr!("view-back-tip"),
        PlaceAt::LeftSide => tr!("view-left-tip"),
        PlaceAt::RightSide => tr!("view-right-tip"),
    }
}

/// The camera, turned to `yaw`, looks from `side`.
fn looks_from(yaw: f32, side: PlaceAt) -> bool {
    let off = (f64::from(yaw) - side.angle()).rem_euclid(std::f64::consts::TAU);
    off.min(std::f64::consts::TAU - off) < 1e-3
}

/// Draws the four view buttons in the top-right corner of `view` (the 3D image) for a camera
/// turned to `yaw`, and returns the one clicked.
pub fn view_picker(ui: &mut egui::Ui, view: Rect, yaw: f32) -> Option<PlaceAt> {
    let width = VIEWS.len() as f32 * (BUTTON.x + GAP) - GAP;
    let mut at = pos2(view.right() - MARGIN - width, view.top() + MARGIN);
    let mut clicked = None;
    for side in VIEWS {
        let rect = Rect::from_min_size(at, BUTTON);
        at.x += BUTTON.x + GAP;
        let selected = looks_from(yaw, side);
        let response = ui.put(
            rect,
            egui::Button::selectable(selected, "").min_size(BUTTON),
        );
        let label = name(side);
        ui.ctx().accesskit_node_builder(response.id, |node| {
            node.set_label(label);
            node.set_selected(selected);
        });
        let visuals = ui.style().interact_selectable(&response, selected);
        paint_form(
            ui.painter(),
            rect.shrink2(vec2(3.0, 3.0)),
            side,
            visuals.fg_stroke.color,
        );
        if response.on_hover_text(tip(side)).clicked() {
            clicked = Some(side);
        }
    }
    clicked
}

/// A small outline of the dress form in `rect`, as seen from `side`: from the front with a V
/// neckline, from the back with its centre-back seam, from a side in profile with the bust
/// towards where the form faces.
fn paint_form(painter: &egui::Painter, rect: Rect, side: PlaceAt, colour: egui::Color32) {
    let at = |u: f32, v: f32| -> Pos2 { rect.min + vec2(u * rect.width(), v * rect.height()) };
    let stroke = Stroke::new(1.2, colour);
    // The neck cap.
    painter.rect_filled(Rect::from_min_max(at(0.4, 0.0), at(0.6, 0.12)), 1.0, colour);
    // The pole under the form.
    painter.line_segment([at(0.5, 0.9), at(0.5, 1.0)], stroke);
    let outline: Vec<Pos2> = match side {
        PlaceAt::Front | PlaceAt::Back => [
            (0.38, 0.14),
            (0.12, 0.24),
            (0.1, 0.4),
            (0.24, 0.62),
            (0.14, 0.9),
            (0.86, 0.9),
            (0.76, 0.62),
            (0.9, 0.4),
            (0.88, 0.24),
            (0.62, 0.14),
        ]
        .map(|(u, v)| at(u, v))
        .to_vec(),
        PlaceAt::LeftSide | PlaceAt::RightSide => {
            // In profile facing left; mirrored for the right side.
            let flip = side == PlaceAt::RightSide;
            [
                (0.42, 0.14),
                (0.3, 0.3),
                (0.16, 0.42),
                (0.32, 0.62),
                (0.24, 0.9),
                (0.72, 0.9),
                (0.66, 0.62),
                (0.7, 0.3),
                (0.58, 0.14),
            ]
            .map(|(u, v)| if flip { at(1.0 - u, v) } else { at(u, v) })
            .to_vec()
        }
    };
    painter.add(Shape::closed_line(outline, stroke));
    match side {
        PlaceAt::Front => {
            let v = [at(0.38, 0.14), at(0.5, 0.3), at(0.62, 0.14)];
            painter.add(Shape::line(v.to_vec(), stroke));
        }
        PlaceAt::Back => {
            painter.line_segment([at(0.5, 0.14), at(0.5, 0.9)], stroke);
        }
        PlaceAt::LeftSide | PlaceAt::RightSide => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_the_view_the_camera_looks_from_is_current() {
        let yaw = std::f64::consts::PI as f32;
        assert!(looks_from(yaw, PlaceAt::Back));
        assert!(
            looks_from(-yaw, PlaceAt::Back),
            "a whole turn apart is the same view"
        );
        assert!(!looks_from(yaw, PlaceAt::Front));
        assert!(
            !looks_from(0.3, PlaceAt::Front),
            "an orbited camera looks from none"
        );
        assert!(looks_from(0.0, PlaceAt::Front));
    }
}
