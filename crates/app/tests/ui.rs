use egui_kittest::{Harness, kittest::Queryable};
use opendrape::OpenDrapeApp;

#[test]
fn window_shows_app_name() {
    let mut harness = Harness::builder()
        .with_size(egui::vec2(800.0, 600.0))
        .build_eframe(|cc| OpenDrapeApp::new(cc));
    harness.run();
    harness.get_by_label("OpenDrape");
}
