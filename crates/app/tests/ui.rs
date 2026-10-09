use egui_kittest::{Harness, kittest::Queryable};
use opendrape::gpu::{Decision, GpuChoice, GpuState, Reason, StateStore};
use opendrape::{OpenDrapeApp, Shared, SharedState, Startup};
use std::{path::Path, rc::Rc};

const SAVED_AUTO: Decision = Decision {
    choice: GpuChoice::Auto,
    reason: Reason::Saved,
};

fn harness(config_dir: &Path, shared: SharedState) -> Harness<'static, OpenDrapeApp> {
    let startup = Startup {
        decision: SAVED_AUTO,
        previous: GpuState::default(),
        store: StateStore::new(Some(config_dir)),
        smoke_test: false,
    };
    Harness::builder()
        .with_size(egui::vec2(1000.0, 700.0))
        .wgpu()
        .build_eframe(move |cc| OpenDrapeApp::new(cc, startup, shared))
}

#[test]
fn viewport_draws_and_first_frame_clears_the_crash_marker() {
    let dir = tempfile::tempdir().unwrap();
    let store = StateStore::new(Some(dir.path()));
    // What main() writes before starting the GPU:
    store.save(&GpuState {
        preferred: GpuChoice::Auto,
        pending: Some(GpuChoice::Auto),
    });
    let shared = SharedState::default();
    let mut h = harness(dir.path(), shared.clone());
    h.run();
    assert!(h.state().viewport_frames() > 0, "3D viewport drew nothing");
    assert!(shared.first_frame_drawn.get());
    assert_eq!(
        store.load(),
        GpuState {
            preferred: GpuChoice::Auto,
            pending: None
        }
    );
}

#[test]
fn about_box_copies_diagnostics() {
    let dir = tempfile::tempdir().unwrap();
    let mut h = harness(dir.path(), SharedState::default());
    h.run();
    h.get_by_label("Help").click();
    h.run();
    h.get_by_label("About OpenDrape").click();
    h.run();
    h.get_by_label("Copy diagnostics").click();
    h.step();
    let copied = h
        .output()
        .platform_output
        .commands
        .iter()
        .find_map(|c| match c {
            egui::OutputCommand::CopyText(text) => Some(text.clone()),
            _ => None,
        });
    let copied = copied.expect("Copy diagnostics put text on the clipboard");
    assert!(
        copied.contains("OpenDrape ") && copied.contains("Graphics mode: Auto"),
        "{copied}"
    );
    h.run();
    h.get_by_label("Copied. Paste it into your bug report.");
}

#[test]
fn choosing_a_graphics_mode_saves_it_and_asks_for_a_restart() {
    let dir = tempfile::tempdir().unwrap();
    let shared = SharedState::default();
    let mut h = harness(dir.path(), shared.clone());
    h.run();
    h.state_mut().request_graphics_change(GpuChoice::Software);
    assert_eq!(shared.restart_with.get(), Some(GpuChoice::Software));
    assert_eq!(
        StateStore::new(Some(dir.path())).load(),
        GpuState {
            preferred: GpuChoice::Software,
            pending: None
        }
    );
}

#[test]
fn tiny_window_does_not_crash() {
    let dir = tempfile::tempdir().unwrap();
    let startup = Startup {
        decision: SAVED_AUTO,
        previous: GpuState::default(),
        store: StateStore::new(Some(dir.path())),
        smoke_test: false,
    };
    let mut h = Harness::builder()
        .with_size(egui::vec2(120.0, 40.0)) // the menu bar leaves almost no room for the 3D panel
        .wgpu()
        .build_eframe(move |cc| OpenDrapeApp::new(cc, startup, Rc::new(Shared::default())));
    h.run();
}
