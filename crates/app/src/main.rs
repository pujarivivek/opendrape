fn main() -> eframe::Result {
    eframe::run_native(
        "OpenDrape",
        eframe::NativeOptions::default(),
        Box::new(|cc| Ok(Box::new(opendrape::OpenDrapeApp::new(cc)))),
    )
}
