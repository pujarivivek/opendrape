//! The Assets section: what can be put on the stage (dress forms for now), shown where the
//! pattern usually is while the student browses.

use crate::icons::icon_button;
use crate::tr;
use egui_phosphor::regular::X;

/// The section's heading, with a ✕ that closes it on the right. Returns whether the ✕ was
/// clicked.
pub fn header(ui: &mut egui::Ui) -> bool {
    ui.horizontal(|ui| {
        ui.heading(tr!("assets-dress-forms"));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            icon_button(
                ui,
                X,
                &tr!("assets-close"),
                &tr!("assets-close-tip"),
                false,
                true,
            )
            .clicked()
        })
        .inner
    })
    .inner
}
