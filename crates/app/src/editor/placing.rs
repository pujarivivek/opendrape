//! Placing pieces in 3D from the pattern window and Properties: Place at… (wrapped round the
//! form at its front, back or sides, or round an arm), Flat, and typed positions and angles.
//! Each is one undo step, and works on a twin as on any piece (the twin then keeps a placement
//! of its own). While the garment drapes, placements don't apply: they are not offered then.

use super::{PatternEditor, Selection};
use crate::tr;
use opendrape_core::{PieceId, Placement};
use opendrape_geom as geom;
use opendrape_mesh::place::{self, PlaceAt};

/// Where pieces start when there is no form to measure (the bundled body's shoulders, m).
pub const DEFAULT_SHOULDER_M: f64 = 1.3;

impl PatternEditor {
    /// The form's shoulder height: where the top of the pattern starts.
    pub(super) fn shoulder_y(&self) -> f64 {
        self.stage
            .as_ref()
            .map_or(DEFAULT_SHOULDER_M, |s| s.shoulder_y())
    }

    /// Where piece or twin `id` is in 3D now: its own placement, or the one it takes.
    pub fn placement(&self, id: PieceId) -> Option<Placement> {
        let project = self.doc.project();
        let shapes = geom::shapes(project);
        let shape = shapes.iter().find(|s| s.id == id)?;
        Some(place::effective(
            project,
            shape,
            &place::layout(&shapes),
            self.shoulder_y(),
        ))
    }

    /// Gives `id` this placement, as one undo step.
    pub(super) fn set_placement(&mut self, id: PieceId, placement: Placement) {
        self.doc.edit(|p| p.set_placement(id, Some(placement)));
        if !self.note_if_refused() {
            self.selection = Selection::Piece(id);
        }
    }

    /// Place at…: wraps `id` round the form at `at`, at the height it has now. Needs a form.
    pub fn place_at(&mut self, id: PieceId, at: PlaceAt) {
        let Some(stage) = self.stage.clone() else {
            return;
        };
        if let Some(placement) = stage.place_at(self.doc.project(), id, at) {
            self.set_placement(id, placement);
        }
    }

    /// Place at → Left arm (`arm` 0) or Right arm (1): wraps `id` round that arm (see
    /// `place::place_at_arm`). Its partner in a mirrored pair goes on the other arm: a twin by
    /// taking its piece's placement mirrored, a piece by being given the twin's mirrored. One
    /// undo step. Needs a form with arms: on one without, nothing changes and the notice says so.
    pub fn place_at_arm(&mut self, id: PieceId, arm: usize) {
        let Some(stage) = self.stage.clone() else {
            return;
        };
        if stage.arms().is_none() {
            self.notice = Some(tr!("notice-no-arms"));
            return;
        }
        let project = self.doc.project();
        let Some(placement) = stage.place_at_arm(project, id, arm) else {
            return;
        };
        let partner = match project.owner(id) {
            Some((piece, opendrape_core::Side::Master)) => {
                piece.twin.as_ref().map(|t| (t.id, None))
            }
            Some((piece, opendrape_core::Side::Twin)) => {
                Some((piece.id, Some(placement.mirrored())))
            }
            None => None,
        };
        self.doc.edit(|p| {
            p.set_placement(id, Some(placement));
            if let Some((other, its)) = partner {
                p.set_placement(other, its);
            }
        });
        if !self.note_if_refused() {
            self.selection = Selection::Piece(id);
        }
    }

    /// Flat: takes away the curve, leaving the piece where it is.
    pub fn flatten(&mut self, id: PieceId) {
        if let Some(p) = self.placement(id) {
            self.set_placement(id, Placement { curve: None, ..p });
        }
    }

    /// The Place at… menu for `id` (right-click on a piece, in 2D or 3D). While the garment
    /// drapes its items are greyed out: placements apply after Reset.
    pub fn place_menu(&mut self, ui: &mut egui::Ui, id: PieceId) {
        let free = !self.draping;
        for (at, label) in [
            (PlaceAt::Front, tr!("place-front")),
            (PlaceAt::Back, tr!("place-back")),
            (PlaceAt::LeftSide, tr!("place-left")),
            (PlaceAt::RightSide, tr!("place-right")),
        ] {
            if ui.add_enabled(free, egui::Button::new(label)).clicked() {
                self.place_at(id, at);
                ui.close();
            }
        }
        // A form without arms greys the arm items out and says why.
        let no_arms = self.stage.as_ref().is_some_and(|s| s.arms().is_none());
        for (arm, label) in [(0, tr!("place-left-arm")), (1, tr!("place-right-arm"))] {
            let item = ui.add_enabled(free && !no_arms, egui::Button::new(label));
            if no_arms {
                item.on_disabled_hover_text(tr!("notice-no-arms"));
            } else if item.clicked() {
                self.place_at_arm(id, arm);
                ui.close();
            }
        }
        ui.separator();
        if ui
            .add_enabled(free, egui::Button::new(tr!("place-flat")))
            .clicked()
        {
            self.flatten(id);
            ui.close();
        }
    }

    /// The "3D placement" group of a piece's properties: its position (in the student's units)
    /// and its rotation in degrees about x, then y, then z.
    pub(super) fn placement_properties(&mut self, ui: &mut egui::Ui, id: PieceId) {
        let Some(placement) = self.placement(id) else {
            return;
        };
        let units = self.doc.project().units;
        ui.add_space(6.0);
        ui.strong(tr!("panel-placement"));
        if self.draping {
            ui.label(tr!("panel-placement-draping"));
        }
        let free = !self.draping;
        ui.add_enabled_ui(free, |ui| self.placement_fields(ui, id, placement, units));
    }

    /// The position and rotation fields of the "3D placement" group.
    fn placement_fields(
        &mut self,
        ui: &mut egui::Ui,
        id: PieceId,
        placement: Placement,
        units: opendrape_core::Units,
    ) {
        egui::Grid::new("placement_properties")
            .num_columns(3)
            .show(ui, |ui| {
                let labels = [
                    tr!("panel-position-x"),
                    tr!("panel-position-y"),
                    tr!("panel-position-z"),
                ];
                for (k, label) in labels.into_iter().enumerate() {
                    let shown = units.format_number(placement.position[k] * 1000.0);
                    let typed = self.field(ui, label, &shown, units.suffix());
                    self.apply_typed(typed, true, tr!("notice-bad-number"), |p, mm| {
                        let mut moved = placement;
                        moved.position[k] = mm / 1000.0;
                        p.set_placement(id, Some(moved))
                    });
                }
                let angles = place::euler_xyz_deg(placement.rotation);
                let labels = [
                    tr!("panel-rotation-x"),
                    tr!("panel-rotation-y"),
                    tr!("panel-rotation-z"),
                ];
                for (k, label) in labels.into_iter().enumerate() {
                    let typed = self.field(ui, label, &format!("{:.1}", angles[k]), "°");
                    self.apply_typed(typed, false, tr!("notice-bad-number"), |p, degrees| {
                        let mut turned = angles;
                        turned[k] = degrees;
                        let rotation = place::rotation_from_euler_xyz_deg(turned);
                        p.set_placement(
                            id,
                            Some(Placement {
                                rotation,
                                ..placement
                            }),
                        )
                    });
                }
            });
    }
}
