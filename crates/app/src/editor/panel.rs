//! The properties panel (measurements of the selected piece, edge or point, editable by
//! typing) and the status bar.

use super::{PatternEditor, Selection, Tool, select_all_on_focus};
use crate::tr;
use egui::{Id, Key};
use opendrape_core::{Edge, PieceId, Point2, Project, Units, VertexKind};
use opendrape_geom::{self as geom, Anchor};
use std::collections::HashMap;

/// Furthest from the origin a point may be typed (100 m), so a slip can't lose a piece.
const MAX_COORDINATE_MM: f64 = 100_000.0;

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
        }
        let shown = &self.panel.shown;
        self.panel.editing.retain(|id, _| shown.contains(id));
    }

    fn piece_properties(&mut self, ui: &mut egui::Ui, id: PieceId) {
        let Some(piece) = self.doc.project().piece(id).cloned() else {
            return;
        };
        let units = self.doc.project().units;
        ui.strong(tr!("panel-piece"));
        egui::Grid::new("piece_properties")
            .num_columns(3)
            .show(ui, |ui| {
                if let Some(name) = self.field(ui, tr!("panel-name"), &piece.name, "") {
                    let name = name.trim().to_owned();
                    if name.is_empty() {
                        self.notice = Some(tr!("notice-name-empty"));
                    } else {
                        self.doc.edit(|p| {
                            if let Some(pc) = p.piece_mut(id) {
                                pc.name = name;
                            }
                        });
                        self.note_if_refused();
                    }
                }
                let grain = self.field(
                    ui,
                    tr!("panel-grain"),
                    &format!("{:.1}", piece.grain_deg),
                    "°",
                );
                self.apply_typed(grain, false, |p, deg| {
                    p.piece_mut(id).is_some_and(|pc| {
                        pc.grain_deg = deg.rem_euclid(360.0);
                        true
                    })
                });
                ui.label(tr!("panel-area"));
                ui.label(units.format_area(geom::area(&piece)));
                ui.end_row();
                ui.label(tr!("panel-perimeter"));
                ui.label(units.format(geom::perimeter(&piece)));
                ui.end_row();
            });
        ui.add_space(6.0);
        if ui.button(tr!("panel-delete-piece")).clicked() {
            self.delete_selection();
        }
    }

    fn edge_properties(&mut self, ui: &mut egui::Ui, id: PieceId, i: usize) {
        let Some(piece) = self.doc.project().piece(id).cloned() else {
            return;
        };
        let units = self.doc.project().units;
        let anchor = self.panel.anchor;
        ui.strong(tr!("panel-edge"));
        egui::Grid::new("edge_properties")
            .num_columns(3)
            .show(ui, |ui| {
                let length = units.format_number(geom::edge_length(&piece, i));
                let typed = self.field(ui, tr!("panel-length"), &length, units.suffix());
                self.apply_typed(typed, true, |p, mm| {
                    p.piece_mut(id)
                        .is_some_and(|pc| geom::set_edge_length(pc, i, mm, anchor))
                });
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
        let mut curved = matches!(piece.edges[i], Edge::Curve { .. });
        if ui.checkbox(&mut curved, tr!("panel-curved")).changed() {
            self.doc.edit(|p| {
                if let Some(pc) = p.piece_mut(id) {
                    pc.set_curved(i, curved);
                }
            });
            self.note_if_refused();
        }
    }

    fn vertex_properties(&mut self, ui: &mut egui::Ui, id: PieceId, i: usize) {
        let Some(piece) = self.doc.project().piece(id).cloned() else {
            return;
        };
        let units = self.doc.project().units;
        let pos = piece.vertices[i].pos;
        let move_to = move |p: &mut Project, to: Point2| {
            to.x.abs() <= MAX_COORDINATE_MM
                && to.y.abs() <= MAX_COORDINATE_MM
                && p.piece_mut(id).is_some_and(|pc| {
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
                    &units.format_number(pos.x),
                    units.suffix(),
                );
                self.apply_typed(x, true, |p, mm| move_to(p, Point2::new(mm, pos.y)));
                let y = self.field(
                    ui,
                    tr!("panel-y"),
                    &units.format_number(pos.y),
                    units.suffix(),
                );
                self.apply_typed(y, true, |p, mm| move_to(p, Point2::new(pos.x, mm)));
            });
        let mut smooth = piece.vertices[i].kind == VertexKind::Smooth;
        if ui.checkbox(&mut smooth, tr!("panel-smooth")).changed() {
            let kind = if smooth {
                VertexKind::Smooth
            } else {
                VertexKind::Corner
            };
            self.doc.edit(|p| {
                if let Some(pc) = p.piece_mut(id) {
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
    /// says why.
    fn apply_typed(
        &mut self,
        typed: Option<String>,
        is_length: bool,
        apply: impl FnOnce(&mut Project, f64) -> bool,
    ) {
        let Some(text) = typed else { return };
        let units = self.doc.project().units;
        let value = Units::parse(&text).map(|v| if is_length { units.to_mm(v) } else { v });
        let Some(value) = value else {
            self.notice = Some(tr!("notice-bad-number"));
            return;
        };
        let applied = self.doc.edit(|p| apply(p, value));
        if !self.note_if_refused() && !applied {
            self.notice = Some(tr!("notice-bad-number"));
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
        }
    }
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
    if r.has_focus() {
        editing.insert(id, text);
    } else {
        editing.remove(&id);
    }
    None
}
