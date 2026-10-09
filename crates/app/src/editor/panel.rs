//! The properties panel (measurements of the selected piece, edge or point, editable by
//! typing) and the status bar.

use super::{PatternEditor, Selection, Tool};
use crate::tr;
use egui::{Id, Key};
use opendrape_core::{Edge, PieceId, Point2, Project, Units, VertexKind};
use opendrape_geom::{self as geom, Anchor};

/// Furthest from the origin a point may be typed (100 m), so a slip can't lose a piece.
const MAX_COORDINATE_MM: f64 = 100_000.0;

#[derive(Default)]
pub(super) struct PanelState {
    /// The field being typed in and its text, applied on Enter or clicking elsewhere.
    editing: Option<(Id, String)>,
    /// Which end of an edge stays put when its length is typed.
    anchor: Anchor,
}

impl PatternEditor {
    pub(super) fn properties(&mut self, ui: &mut egui::Ui) {
        ui.heading(tr!("panel-title"));
        ui.separator();
        match self.selection {
            Selection::None => {
                ui.label(tr!("panel-hint"));
            }
            Selection::Piece(id) => self.piece_properties(ui, id),
            Selection::Edge(id, i) => self.edge_properties(ui, id, i),
            Selection::Vertex(id, i) => self.vertex_properties(ui, id, i),
        }
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
                    if !name.is_empty() {
                        self.doc.edit(|p| {
                            if let Some(pc) = p.piece_mut(id) {
                                pc.name = name;
                            }
                        });
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
        let typed = text_field(ui, &mut self.panel.editing, id, &label, value);
        ui.label(unit);
        ui.end_row();
        typed
    }

    /// Applies a typed number as one undo step. Lengths (`is_length`) are typed in the current
    /// units and passed on in millimetres. `apply` returns false to refuse the value: the
    /// pattern stays unchanged and a notice says why.
    fn apply_typed(
        &mut self,
        typed: Option<String>,
        is_length: bool,
        apply: impl FnOnce(&mut Project, f64) -> bool,
    ) {
        let Some(text) = typed else { return };
        let units = self.doc.project().units;
        let value = Units::parse(&text).map(|v| if is_length { units.to_mm(v) } else { v });
        if !value.is_some_and(|v| self.doc.edit(|p| apply(p, v))) {
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
    editing: &mut Option<(Id, String)>,
    id: Id,
    label: &str,
    value: &str,
) -> Option<String> {
    let mine = matches!(editing, Some((e, _)) if *e == id);
    let mut text = match editing {
        Some((e, t)) if *e == id => t.clone(),
        _ => value.to_owned(),
    };
    let l = ui.label(label);
    let r = ui
        .add(
            egui::TextEdit::singleline(&mut text)
                .id(id)
                .desired_width(80.0),
        )
        .labelled_by(l.id);
    if r.lost_focus() {
        let escaped = ui.input(|i| i.key_pressed(Key::Escape));
        if mine {
            *editing = None;
        }
        return (mine && !escaped && text != value).then_some(text);
    }
    if r.has_focus() {
        *editing = Some((id, text));
    }
    None
}
