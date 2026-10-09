//! OpenDrape desktop application.

pub mod gpu;
pub mod i18n;

pub struct OpenDrapeApp;

impl OpenDrapeApp {
    pub fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        Self
    }
}

impl eframe::App for OpenDrapeApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ui, |ui| {
            ui.heading("OpenDrape");
        });
    }
}
