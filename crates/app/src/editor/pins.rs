//! Pins on the pattern table: where each is shown, picking one by its marker, its panel, and
//! adding and removing pins (each one undo step).

use super::{HIT_PX, PatternEditor, Selection};
use crate::tr;
use opendrape_core::{Pin, Point2};

impl PatternEditor {
    /// Each pin (its index) and where its shape shows it (mm).
    pub(super) fn pin_spots(&self) -> Vec<(usize, Point2)> {
        let shapes = self.shapes();
        self.doc
            .project()
            .pins
            .iter()
            .enumerate()
            .filter_map(|(k, pin)| {
                let shape = shapes.iter().find(|s| s.id == pin.shape)?;
                Some((k, shape.spot_shown(pin.half, pin.at)))
            })
            .collect()
    }

    /// The pin whose marker is within `tol` mm of `w`, the nearest.
    pub(super) fn pin_at(&self, w: Point2, tol: f64) -> Option<usize> {
        self.pin_spots()
            .into_iter()
            .map(|(k, at)| (k, at.distance(w)))
            .filter(|(_, d)| *d <= tol.max(self.view.mm(HIT_PX)))
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .map(|(k, _)| k)
    }

    /// Adds `pin`, as one undo step, and selects it.
    pub fn add_pin(&mut self, pin: Pin) {
        let k = self.doc.edit(|p| {
            p.pins.push(pin);
            p.pins.len() - 1
        });
        if !self.note_if_refused() {
            self.selection = Selection::Pin(k);
        }
    }

    /// Removes pin `k`, as one undo step.
    pub fn remove_pin(&mut self, k: usize) {
        self.doc.edit(|p| {
            if k < p.pins.len() {
                p.pins.remove(k);
            }
        });
        if !self.note_if_refused() && self.selection == Selection::Pin(k) {
            self.selection = Selection::None;
        }
    }

    /// A pin: the piece it is on, where it holds the fabric, and Remove pin.
    pub(super) fn pin_properties(&mut self, ui: &mut egui::Ui, k: usize) {
        let project = self.doc.project();
        let Some(pin) = project.pins.get(k).copied() else {
            return;
        };
        let units = project.units;
        let name = project.name_of(pin.shape).unwrap_or_default().to_owned();
        ui.strong(tr!("panel-pin"));
        ui.label(tr!("panel-pin-on", name = name));
        ui.label(tr!(
            "panel-pin-held",
            height = units.format(pin.target[1] * 1000.0),
            out = units.format(pin.target[2] * 1000.0)
        ));
        ui.add_space(6.0);
        if ui.button(tr!("panel-remove-pin")).clicked() {
            self.remove_pin(k);
        }
    }
}
