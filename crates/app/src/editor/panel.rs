//! The properties panel (measurements of the selected piece, edge or point, editable by
//! typing) and the status bar.

use super::{PatternEditor, Selection, Tool, select_all_on_focus};
use crate::tr;
use egui::{Id, Key};
use opendrape_core::{
    Edge, LineKind, MAX_ALLOWANCE_MM, Notch, NotchStyle, Piece, PieceId, Point2, Project, Side,
    Units, VertexKind,
};
use opendrape_geom::{self as geom, Anchor};
use std::collections::HashMap;

/// Furthest from the origin a point may be typed (100 m), so a slip can't lose a piece.
const MAX_TYPED_COORDINATE_MM: f64 = 100_000.0;
/// Gap (mm) between a piece and the twin "Make mirrored pair" puts beside it.
const PAIR_GAP_MM: f64 = 50.0;

#[derive(Default)]
pub(super) struct PanelState {
    /// The text typed into each field that has it, applied on Enter or clicking elsewhere.
    /// Kept per field, so clicking from one field straight into another loses nothing.
    editing: HashMap<Id, String>,
    /// The fields drawn this frame; entries of any other field are dropped.
    shown: Vec<Id>,
    /// Which end of an edge stays put when its length is typed.
    anchor: Anchor,
}

impl PanelState {
    /// A field is being typed in (or was, last frame), so it owns the keyboard.
    pub(super) fn is_editing(&self) -> bool {
        !self.editing.is_empty()
    }
}

impl PatternEditor {
    pub(super) fn properties(&mut self, ui: &mut egui::Ui) {
        ui.heading(tr!("panel-title"));
        ui.separator();
        self.panel.shown.clear();
        match self.selection {
            Selection::None => {
                ui.label(tr!("panel-hint"));
            }
            Selection::Piece(id) => self.piece_properties(ui, id),
            Selection::Edge(id, i) => self.edge_properties(ui, id, i),
            Selection::Vertex(id, i) => self.vertex_properties(ui, id, i),
            Selection::Notch(id, k) => self.notch_properties(ui, id, k),
            Selection::Line(id, l) => self.line_properties(ui, id, l),
        }
        let shown = &self.panel.shown;
        self.panel.editing.retain(|id, _| shown.contains(id));
    }

    fn piece_properties(&mut self, ui: &mut egui::Ui, id: PieceId) {
        let project = self.doc.project();
        let Some((piece, side)) = project.owner(id).map(|(p, s)| (p.clone(), s)) else {
            return;
        };
        let Some(shape) = geom::shape_of(project, id) else {
            return;
        };
        let units = project.units;
        let source = piece.id;
        ui.strong(tr!("panel-piece"));
        if side == Side::Twin {
            ui.label(tr!("panel-twin-of", name = piece.name.clone()));
        }
        egui::Grid::new("piece_properties")
            .num_columns(3)
            .show(ui, |ui| {
                if let Some(name) = self.field(ui, tr!("panel-name"), &shape.piece.name, "") {
                    let name = name.trim().to_owned();
                    if name.is_empty() {
                        self.notice = Some(tr!("notice-name-empty"));
                    } else {
                        self.doc.edit(|p| match p.owner_mut(id) {
                            Some((pc, Side::Master)) => pc.name = name,
                            Some((pc, Side::Twin)) => {
                                if let Some(t) = &mut pc.twin {
                                    t.name = name;
                                }
                            }
                            None => {}
                        });
                        self.note_if_refused();
                    }
                }
                let grain = self.field(
                    ui,
                    tr!("panel-grain"),
                    &format!("{:.1}", shape.piece.grain_deg),
                    "°",
                );
                self.apply_typed(grain, false, tr!("notice-bad-number"), |p, deg| {
                    p.piece_mut(source).is_some_and(|pc| {
                        // A twin shows its piece's grain mirrored: 180° − angle.
                        let stored = if side == Side::Twin { 180.0 - deg } else { deg };
                        pc.grain_deg = stored.rem_euclid(360.0);
                        true
                    })
                });
                let allowance = self.field(
                    ui,
                    tr!("panel-allowance"),
                    &units.format_number(piece.allowance),
                    units.suffix(),
                );
                self.apply_typed(allowance, true, bad_allowance(units), |p, mm| {
                    allowance_ok(mm)
                        && p.piece_mut(source).is_some_and(|pc| {
                            pc.allowance = mm;
                            true
                        })
                });
                ui.label(tr!("panel-area"));
                ui.label(units.format_area(geom::area(&shape.piece)));
                ui.end_row();
                ui.label(tr!("panel-perimeter"));
                ui.label(units.format(geom::perimeter(&shape.piece)));
                ui.end_row();
            });
        ui.add_space(6.0);
        if piece.fold.is_some() {
            ui.horizontal(|ui| {
                if ui.button(tr!("panel-unfold")).clicked() {
                    self.unfold(source);
                }
                if ui.button(tr!("panel-remove-fold")).clicked() {
                    self.remove_fold(source);
                }
            });
        } else if piece.twin.is_some() {
            if ui.button(tr!("panel-break-pair")).clicked() {
                self.break_pair(source);
            }
        } else if ui.button(tr!("panel-make-pair")).clicked() {
            self.make_pair(&piece);
        }
        if ui.button(tr!("panel-delete-piece")).clicked() {
            self.delete_selection();
        }
    }

    fn edge_properties(&mut self, ui: &mut egui::Ui, id: PieceId, i: usize) {
        let Some((piece, side)) = self.doc.project().owner(id).map(|(p, s)| (p.clone(), s)) else {
            return;
        };
        let units = self.doc.project().units;
        let source = piece.id;
        let anchor = self.panel.anchor;
        // Everything here is the stored piece's: edge `i` is its index, and a twin has the same
        // edges. The fold's own edge is on no outline, so it has no allowance or hem to show.
        let is_fold = piece.fold == Some(i);
        ui.strong(tr!("panel-edge"));
        egui::Grid::new("edge_properties")
            .num_columns(3)
            .show(ui, |ui| {
                let length = units.format_number(geom::edge_length(&piece, i));
                let typed = self.field(ui, tr!("panel-length"), &length, units.suffix());
                self.apply_typed(typed, true, tr!("notice-bad-number"), |p, mm| {
                    p.piece_mut(source)
                        .is_some_and(|pc| geom::set_edge_length(pc, i, mm, anchor))
                });
                if !is_fold {
                    let own = units.format_number(piece.edge_allowance(i));
                    let typed = self.field(ui, tr!("panel-allowance"), &own, units.suffix());
                    self.apply_typed(typed, true, bad_allowance(units), |p, mm| {
                        allowance_ok(mm)
                            && p.piece_mut(source).is_some_and(|pc| {
                                pc.edge_props[i].allowance = Some(mm);
                                true
                            })
                    });
                }
            });
        ui.label(tr!("panel-keep-fixed"));
        ui.horizontal(|ui| {
            ui.radio_value(
                &mut self.panel.anchor,
                Anchor::Start,
                tr!("panel-anchor-start"),
            );
            ui.radio_value(&mut self.panel.anchor, Anchor::End, tr!("panel-anchor-end"));
        });
        if is_fold {
            ui.label(tr!("panel-fold-line"));
            if ui.button(tr!("panel-remove-fold")).clicked() {
                self.remove_fold(source);
            }
            return;
        }
        if piece.edge_props[i].allowance.is_some()
            && ui.button(tr!("panel-allowance-reset")).clicked()
        {
            self.doc.edit(|p| {
                if let Some(pc) = p.piece_mut(source) {
                    pc.edge_props[i].allowance = None;
                }
            });
            self.note_if_refused();
        }
        let mut hem = piece.edge_props[i].hem;
        if ui.checkbox(&mut hem, tr!("panel-hem")).changed() {
            self.doc.edit(|p| {
                if let Some(pc) = p.piece_mut(source) {
                    pc.edge_props[i].hem = hem;
                }
            });
            self.note_if_refused();
        }
        let mut curved = matches!(piece.edges[i], Edge::Curve { .. });
        if ui.checkbox(&mut curved, tr!("panel-curved")).changed() {
            self.doc.edit(|p| {
                if let Some(pc) = p.piece_mut(source) {
                    pc.set_curved(i, curved);
                }
            });
            self.note_if_refused();
        }
        let can_fold = side == Side::Master
            && piece.twin.is_none()
            && piece.fold.is_none()
            && piece.edges[i] == Edge::Line;
        if can_fold && ui.button(tr!("panel-set-fold")).clicked() {
            self.doc.edit(|p| {
                if let Some(pc) = p.piece_mut(source) {
                    pc.fold = Some(i);
                }
            });
            if self.note_if_refused() {
                self.notice = Some(tr!("notice-fold-refused"));
            }
        }
    }

    fn vertex_properties(&mut self, ui: &mut egui::Ui, id: PieceId, i: usize) {
        let project = self.doc.project();
        let Some((piece, _)) = project.owner(id).map(|(p, s)| (p.clone(), s)) else {
            return;
        };
        let Some(shape) = geom::shape_of(project, id) else {
            return;
        };
        let units = project.units;
        let source = piece.id;
        // Shown (and typed) where this shape shows the point: a twin's image for a twin.
        let shown = shape.from_stored(piece.vertices[i].pos);
        let move_to = move |p: &mut Project, to_shown: Point2| {
            let to = shape.to_stored(to_shown);
            to.x.abs() <= MAX_TYPED_COORDINATE_MM
                && to.y.abs() <= MAX_TYPED_COORDINATE_MM
                && p.piece_mut(source).is_some_and(|pc| {
                    pc.move_vertex(i, to);
                    true
                })
        };
        ui.strong(tr!("panel-point"));
        egui::Grid::new("vertex_properties")
            .num_columns(3)
            .show(ui, |ui| {
                let x = self.field(
                    ui,
                    tr!("panel-x"),
                    &units.format_number(shown.x),
                    units.suffix(),
                );
                self.apply_typed(x, true, tr!("notice-bad-number"), |p, mm| {
                    move_to(p, Point2::new(mm, shown.y))
                });
                let y = self.field(
                    ui,
                    tr!("panel-y"),
                    &units.format_number(shown.y),
                    units.suffix(),
                );
                self.apply_typed(y, true, tr!("notice-bad-number"), |p, mm| {
                    move_to(p, Point2::new(shown.x, mm))
                });
            });
        let mut smooth = piece.vertices[i].kind == VertexKind::Smooth;
        if ui.checkbox(&mut smooth, tr!("panel-smooth")).changed() {
            let kind = if smooth {
                VertexKind::Smooth
            } else {
                VertexKind::Corner
            };
            self.doc.edit(|p| {
                if let Some(pc) = p.piece_mut(source) {
                    pc.set_vertex_kind(i, kind);
                }
            });
            self.note_if_refused();
        }
        ui.add_space(6.0);
        if ui.button(tr!("panel-delete-point")).clicked() {
            self.delete_selection();
        }
    }

    fn notch_properties(&mut self, ui: &mut egui::Ui, id: PieceId, k: usize) {
        let Some((piece, _)) = self.doc.project().owner(id).map(|(p, s)| (p.clone(), s)) else {
            return;
        };
        let Some(notch) = piece.notches.get(k).copied() else {
            return;
        };
        let units = self.doc.project().units;
        let source = piece.id;
        let len = geom::edge_length(&piece, notch.edge);
        ui.strong(tr!("panel-notch"));
        egui::Grid::new("notch_properties")
            .num_columns(3)
            .show(ui, |ui| {
                let typed = self.field(
                    ui,
                    tr!("box-distance"),
                    &units.format_number(notch.distance),
                    units.suffix(),
                );
                self.apply_typed(typed, true, tr!("notice-bad-notch"), |p, mm| {
                    (0.0..=len).contains(&mm)
                        && p.piece_mut(source).is_some_and(|pc| {
                            pc.notches[k].distance = mm;
                            true
                        })
                });
            });
        ui.label(tr!("panel-notch-marks"));
        ui.horizontal(|ui| {
            for (marks, label) in [
                (1, tr!("panel-notch-single")),
                (2, tr!("panel-notch-double")),
                (3, tr!("panel-notch-triple")),
            ] {
                if ui.radio(notch.marks == marks, label).clicked() && notch.marks != marks {
                    self.edit_notch(source, k, |n| n.marks = marks);
                }
            }
        });
        ui.label(tr!("panel-notch-style"));
        ui.horizontal(|ui| {
            for (style, label) in [
                (NotchStyle::Slit, tr!("panel-notch-slit")),
                (NotchStyle::V, tr!("panel-notch-v")),
            ] {
                if ui.radio(notch.style == style, label).clicked() && notch.style != style {
                    self.edit_notch(source, k, |n| n.style = style);
                }
            }
        });
        ui.add_space(6.0);
        if ui.button(tr!("panel-delete-notch")).clicked() {
            self.delete_selection();
        }
    }

    fn line_properties(&mut self, ui: &mut egui::Ui, id: PieceId, l: usize) {
        let Some((piece, _)) = self.doc.project().owner(id).map(|(p, s)| (p.clone(), s)) else {
            return;
        };
        let Some(line) = piece.lines.get(l).cloned() else {
            return;
        };
        let units = self.doc.project().units;
        let source = piece.id;
        ui.strong(tr!("panel-line"));
        ui.label(tr!(
            "panel-line-length",
            length = units.format(geom::line_length(&line))
        ));
        ui.label(tr!("panel-line-kind"));
        let mut kind = line.kind;
        ui.horizontal(|ui| {
            ui.radio_value(&mut kind, LineKind::Marking, tr!("panel-line-marking"));
            ui.add_enabled_ui(line.closed, |ui| {
                ui.radio_value(&mut kind, LineKind::Cutout, tr!("panel-line-cutout"))
            });
        });
        if kind != line.kind {
            self.doc.edit(|p| {
                if let Some(pc) = p.piece_mut(source)
                    && let Some(stored) = pc.lines.get_mut(l)
                {
                    stored.kind = kind;
                }
            });
            self.note_if_refused();
        }
        ui.add_space(6.0);
        if ui.button(tr!("panel-delete-line")).clicked() {
            self.delete_selection();
        }
    }

    /// Changes notch `k` of `source` as one undo step.
    fn edit_notch(&mut self, source: PieceId, k: usize, change: impl FnOnce(&mut Notch)) {
        self.doc.edit(|p| {
            if let Some(pc) = p.piece_mut(source)
                && let Some(n) = pc.notches.get_mut(k)
            {
                change(n);
            }
        });
        self.note_if_refused();
    }

    fn make_pair(&mut self, piece: &Piece) {
        let name = tr!("twin-name", name = piece.name.clone());
        let offset = twin_offset_beside(piece);
        let source = piece.id;
        let twin = self.doc.edit(|p| p.add_twin(source, name, offset));
        if !self.note_if_refused()
            && let Some(t) = twin
        {
            self.selection = Selection::Piece(t);
        }
    }

    /// Both pieces keep their ids, so whichever was selected stays selected.
    fn break_pair(&mut self, source: PieceId) {
        self.doc.edit(|p| p.break_twin(source));
        self.note_if_refused();
    }

    fn unfold(&mut self, source: PieceId) {
        self.doc.edit(|p| {
            if let Some(pc) = p.piece_mut(source) {
                let full = geom::unfolded(pc);
                *pc = full;
            }
        });
        self.note_if_refused();
    }

    fn remove_fold(&mut self, source: PieceId) {
        self.doc.edit(|p| {
            if let Some(pc) = p.piece_mut(source) {
                pc.fold = None;
            }
        });
        self.note_if_refused();
    }

    /// One row of a 3-column grid (label, text field, unit). Returns what the user typed once
    /// they press Enter or click elsewhere, if it changed.
    fn field(
        &mut self,
        ui: &mut egui::Ui,
        label: String,
        value: &str,
        unit: &str,
    ) -> Option<String> {
        // The selection is part of the id, so typed text can never land on another edge.
        let id = Id::new(("pattern_property", &label, self.selection));
        self.panel.shown.push(id);
        let typed = text_field(ui, &mut self.panel.editing, id, &label, value);
        ui.label(unit);
        ui.end_row();
        typed
    }

    /// Applies a typed number as one undo step. Lengths (`is_length`) are typed in the current
    /// units and passed on in millimetres. `apply` returns false to refuse the value, and the
    /// document may refuse the result: either way the pattern stays unchanged and a notice
    /// says why (`refusal` for a value that isn't a number or that `apply` refused).
    fn apply_typed(
        &mut self,
        typed: Option<String>,
        is_length: bool,
        refusal: String,
        apply: impl FnOnce(&mut Project, f64) -> bool,
    ) {
        let Some(text) = typed else { return };
        let units = self.doc.project().units;
        let value = Units::parse(&text).map(|v| if is_length { units.to_mm(v) } else { v });
        let Some(value) = value else {
            self.notice = Some(refusal);
            return;
        };
        let applied = self.doc.edit(|p| apply(p, value));
        if !self.note_if_refused() && !applied {
            self.notice = Some(refusal);
        }
    }

    pub(super) fn status_bar(&self, ui: &mut egui::Ui) {
        ui.horizontal_wrapped(|ui| {
            if let Some(notice) = &self.notice {
                ui.colored_label(ui.visuals().warn_fg_color, notice);
                ui.separator();
            }
            ui.label(self.hint());
            if let Some(c) = self.canvas.cursor {
                let units = self.doc.project().units;
                ui.separator();
                ui.label(tr!(
                    "status-cursor",
                    x = units.format(c.x),
                    y = units.format(c.y)
                ));
            }
        });
    }

    fn hint(&self) -> String {
        match self.tool {
            Tool::Edit => tr!("hint-edit"),
            Tool::Pen if self.canvas.pen.is_empty() => tr!("hint-pen-start"),
            Tool::Pen => tr!("hint-pen-drawing"),
            Tool::Rectangle => tr!("hint-rectangle"),
            Tool::AddPoint => tr!("hint-add-point"),
            Tool::Notch => tr!("hint-notch"),
            Tool::Line if self.canvas.line.is_empty() => tr!("hint-line-start"),
            Tool::Line => tr!("hint-line-drawing"),
        }
    }
}

/// A seam allowance the pattern can hold: 0 to [`MAX_ALLOWANCE_MM`].
fn allowance_ok(mm: f64) -> bool {
    mm.is_finite() && (0.0..=MAX_ALLOWANCE_MM).contains(&mm)
}

/// "The seam allowance must be between 0 and 10 cm.", with the limit in the current units.
fn bad_allowance(units: Units) -> String {
    tr!("notice-bad-allowance", max = allowance_limit(units))
}

/// The widest seam allowance in `units`, rounded down to the precision the panel shows so that
/// a typed value up to it is accepted: "10 cm", "3.93 in".
fn allowance_limit(units: Units) -> String {
    let scale = match units {
        Units::Cm => 10.0,
        Units::Inch => 100.0,
    };
    let rounded_down = (units.from_mm(MAX_ALLOWANCE_MM) * scale + 1e-9).floor() / scale;
    format!("{rounded_down} {}", units.suffix())
}

/// The offset that puts a piece's mirror image [`PAIR_GAP_MM`] to its right, at the same
/// height (see `opendrape_core::Twin`).
fn twin_offset_beside(piece: &Piece) -> Point2 {
    let max_x = geom::outline_points(piece, 1.0)
        .iter()
        .map(|p| p.x)
        .fold(f64::MIN, f64::max);
    Point2::new(2.0 * max_x + PAIR_GAP_MM, 0.0)
}

/// A one-line text box for a property. Typing edits a private copy; Enter or clicking
/// elsewhere returns it (when it differs from `value`); Escape throws it away.
fn text_field(
    ui: &mut egui::Ui,
    editing: &mut HashMap<Id, String>,
    id: Id,
    label: &str,
    value: &str,
) -> Option<String> {
    let mut text = editing
        .get(&id)
        .cloned()
        .unwrap_or_else(|| value.to_owned());
    let l = ui.label(label);
    let mut output = egui::TextEdit::singleline(&mut text)
        .id(id)
        .desired_width(80.0)
        .show(ui);
    select_all_on_focus(ui.ctx(), &mut output, text.chars().count());
    let r = output.response.response.labelled_by(l.id);
    if r.lost_focus() {
        // egui reports the loss for two frames; only the first has an entry to apply.
        let escaped = ui.input(|i| i.key_pressed(Key::Escape));
        let had_entry = editing.remove(&id).is_some();
        return (had_entry && !escaped && text != value).then_some(text);
    }
    // Asked of the memory, not `Response::has_focus`: that is also false while the window is in
    // the background, which would throw away what was typed when the student looks at another
    // app for a moment.
    if ui.memory(|m| m.has_focus(id)) {
        editing.insert(id, text);
    } else {
        editing.remove(&id);
    }
    None
}
