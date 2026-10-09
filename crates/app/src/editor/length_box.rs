//! The small box that appears by the pointer for typing exact numbers: an edge's length and
//! angle while drawing with the pen, a rectangle's width and height, or a notch's distance.

use super::select_all_on_focus;
use crate::tr;
use egui::{Id, Key, Pos2};
use opendrape_core::{PieceId, Point2, Units};

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum BoxKind {
    /// The pen's next edge: length and angle from its last point.
    PenSegment,
    /// A rectangle with its lower-left corner here: width and height.
    Rectangle(Point2),
    /// A notch on stored edge `edge` of `source`, placed from the shape `shape`: its distance,
    /// counted from the edge's end rather than its start when `from_end`.
    NotchDistance {
        shape: PieceId,
        source: PieceId,
        edge: usize,
        from_end: bool,
    },
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

    /// Shows the box. Enter in either field commits; Tab moves between the fields (and back);
    /// Escape or clicking elsewhere cancels.
    pub fn show(&mut self, ctx: &egui::Context, units: Units) -> Option<Outcome> {
        let (first_label, second) = match self.kind {
            BoxKind::PenSegment => (tr!("box-length"), Some((tr!("box-angle"), "°"))),
            BoxKind::Rectangle(_) => (tr!("box-width"), Some((tr!("box-height"), units.suffix()))),
            BoxKind::NotchDistance { .. } => (tr!("box-distance"), None),
        };
        let focus = std::mem::take(&mut self.focus_first);
        // The fields keep their focus on Tab (lock_focus), and the box moves it itself, so Tab
        // cycles between the two fields instead of leaving. This happens before they are drawn:
        // a field that gains focus then shows as focused that frame, which is what
        // `select_all_on_focus` needs to select a pre-filled number.
        let (first_id, second_id) = (Id::new("length_box_first"), Id::new("length_box_second"));
        if ctx.input(|i| i.key_pressed(Key::Tab)) {
            match ctx.memory(|m| m.focused()) {
                Some(id) if id == first_id && second.is_some() => {
                    ctx.memory_mut(|m| m.request_focus(second_id));
                }
                Some(id) if id == second_id => ctx.memory_mut(|m| m.request_focus(first_id)),
                _ => {}
            }
        }
        let (mut enter, mut lost, mut any_focus) = (false, false, false);
        egui::Area::new(Id::new("pattern_number_box"))
            .order(egui::Order::Foreground)
            .fixed_pos(self.pos)
            .show(ctx, |ui| {
                egui::Frame::popup(ui.style()).show(ui, |ui| {
                    ui.horizontal(|ui| {
                        let la = ui.label(first_label);
                        let a = ui
                            .add(
                                egui::TextEdit::singleline(&mut self.first)
                                    .id(first_id)
                                    .desired_width(56.0)
                                    .lock_focus(true),
                            )
                            .labelled_by(la.id);
                        ui.label(units.suffix());
                        let b = second.map(|(label, unit)| {
                            let lb = ui.label(label);
                            let mut out = egui::TextEdit::singleline(&mut self.second)
                                .id(second_id)
                                .desired_width(48.0)
                                .lock_focus(true)
                                .show(ui);
                            // The first field is opened by a typed digit and keeps its cursor
                            // at the end; the second arrives pre-filled (an angle), so Tab
                            // selects it.
                            select_all_on_focus(ui.ctx(), &mut out, self.second.chars().count());
                            let b = out.response.response.labelled_by(lb.id);
                            ui.label(unit);
                            b
                        });
                        if focus {
                            a.request_focus();
                        }
                        for r in std::iter::once(&a).chain(b.as_ref()) {
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
