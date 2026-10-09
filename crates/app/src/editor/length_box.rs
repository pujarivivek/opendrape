//! The small box that appears by the pointer for typing exact numbers: an edge's length and
//! angle while drawing with the pen, or a rectangle's width and height.

use super::select_all_on_focus;
use crate::tr;
use egui::{Id, Key, Pos2};
use opendrape_core::{Point2, Units};

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum BoxKind {
    /// The pen's next edge: length and angle from its last point.
    PenSegment,
    /// A rectangle with its lower-left corner here: width and height.
    Rectangle(Point2),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome {
    Commit,
    Cancel,
}

#[derive(Clone, Debug)]
pub struct LengthBox {
    pub kind: BoxKind,
    pub first: String,
    pub second: String,
    pos: Pos2,
    focus_first: bool,
}

impl LengthBox {
    pub fn new(kind: BoxKind, first: String, second: String, pos: Pos2) -> Self {
        Self {
            kind,
            first,
            second,
            pos,
            focus_first: true,
        }
    }

    /// Shows the box. Enter in either field commits; Tab moves between the fields; Escape or
    /// clicking elsewhere cancels.
    pub fn show(&mut self, ctx: &egui::Context, units: Units) -> Option<Outcome> {
        let (first_label, second_label, second_unit) = match self.kind {
            BoxKind::PenSegment => (tr!("box-length"), tr!("box-angle"), "°"),
            BoxKind::Rectangle(_) => (tr!("box-width"), tr!("box-height"), units.suffix()),
        };
        let focus = std::mem::take(&mut self.focus_first);
        let (mut enter, mut lost, mut any_focus) = (false, false, false);
        egui::Area::new(Id::new("pattern_number_box"))
            .order(egui::Order::Foreground)
            .fixed_pos(self.pos)
            .show(ctx, |ui| {
                egui::Frame::popup(ui.style()).show(ui, |ui| {
                    ui.horizontal(|ui| {
                        let la = ui.label(first_label);
                        let a = ui
                            .add(egui::TextEdit::singleline(&mut self.first).desired_width(56.0))
                            .labelled_by(la.id);
                        ui.label(units.suffix());
                        let lb = ui.label(second_label);
                        let mut second_out = egui::TextEdit::singleline(&mut self.second)
                            .desired_width(48.0)
                            .show(ui);
                        // The first field is opened by a typed digit and keeps its cursor at
                        // the end; the second arrives pre-filled (an angle), so Tab selects it.
                        select_all_on_focus(ui.ctx(), &mut second_out, self.second.chars().count());
                        let b = second_out.response.response.labelled_by(lb.id);
                        ui.label(second_unit);
                        if focus {
                            a.request_focus();
                        }
                        for r in [&a, &b] {
                            any_focus |= r.has_focus();
                            if r.lost_focus() {
                                lost = true;
                                enter |= ui.input(|i| i.key_pressed(Key::Enter));
                            }
                        }
                    });
                });
            });
        let (tab, escape) = ctx.input(|i| (i.key_pressed(Key::Tab), i.key_pressed(Key::Escape)));
        if lost && enter {
            Some(Outcome::Commit)
        } else if escape || (lost && !tab && !any_focus) {
            Some(Outcome::Cancel)
        } else {
            None
        }
    }
}
