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
        autoplay: false,
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
        autoplay: false,
    };
    let mut h = Harness::builder()
        .with_size(egui::vec2(120.0, 40.0)) // the menu bar leaves almost no room for the 3D panel
        .wgpu()
        .build_eframe(move |cc| OpenDrapeApp::new(cc, startup, Rc::new(Shared::default())));
    h.run();
}

#[test]
fn crash_marker_is_cleared_only_after_frames_were_presented() {
    // A driver that crashes on its first present must still count as a crash, so the
    // marker has to survive the first frame and only clear after several.
    let dir = tempfile::tempdir().unwrap();
    let store = StateStore::new(Some(dir.path()));
    store.save(&GpuState {
        preferred: GpuChoice::Auto,
        pending: Some(GpuChoice::Auto),
    });
    let shared = SharedState::default();
    let startup = Startup {
        decision: SAVED_AUTO,
        previous: GpuState::default(),
        store: StateStore::new(Some(dir.path())),
        smoke_test: false,
        autoplay: false,
    };
    let app_shared = shared.clone();
    // The harness draws one frame plus at most `max_steps` more while it is being built.
    let mut h = Harness::builder()
        .with_size(egui::vec2(1000.0, 700.0))
        .with_max_steps(1)
        .wgpu()
        .build_eframe(move |cc| OpenDrapeApp::new(cc, startup, app_shared));
    assert!(h.state().viewport_frames() >= 1);
    assert!(
        !shared.first_frame_drawn.get(),
        "confirmed before any frame was presented"
    );
    assert_eq!(store.load().pending, Some(GpuChoice::Auto));
    h.run_steps(5);
    assert!(shared.first_frame_drawn.get());
    assert_eq!(store.load().pending, None);
}

use std::time::{Duration, Instant};

fn wait_until(
    h: &mut Harness<'static, OpenDrapeApp>,
    what: &str,
    mut cond: impl FnMut(&OpenDrapeApp) -> bool,
) {
    let start = Instant::now();
    while !cond(h.state()) {
        assert!(
            start.elapsed() < Duration::from_secs(20),
            "timed out waiting for {what}"
        );
        std::thread::sleep(Duration::from_millis(20));
        h.step();
    }
}

// While the simulation plays it keeps requesting repaints, so these tests step explicitly
// (`run_steps`, `wait_until`) instead of `run()`, which waits for the UI to settle.

#[test]
fn the_skirt_drapes_and_can_be_paused() {
    let dir = tempfile::tempdir().unwrap();
    let mut h = harness(dir.path(), SharedState::default()); // starts paused (autoplay: false)
    h.run();
    h.get_by_label("A-line skirt");
    h.get_by_label("Play").click();
    h.run_steps(2);
    wait_until(&mut h, "the simulation to advance", |a| {
        a.sim_frame().is_some_and(|f| f.time > 0.05)
    });
    h.get_by_label("Pause").click();
    h.run_steps(3);
    h.get_by_label("Play");
}

#[test]
fn reset_and_garment_switch_reload_the_scene() {
    let dir = tempfile::tempdir().unwrap();
    let mut h = harness(dir.path(), SharedState::default());
    h.run();
    h.get_by_label("Play").click();
    h.run_steps(2);
    wait_until(&mut h, "time > 0.1", |a| {
        a.sim_frame().is_some_and(|f| f.time > 0.1)
    });
    h.get_by_label("Pause").click();
    h.run_steps(2);
    h.get_by_label("Reset").click();
    h.run_steps(2);
    wait_until(&mut h, "time back to 0", |a| {
        a.sim_frame().is_some_and(|f| f.time == 0.0)
    });
    h.get_by_label("Fitted tube (collision test)").click();
    h.run_steps(2);
    wait_until(&mut h, "the tube", |a| {
        a.sim_frame()
            .is_some_and(|f| f.positions.len() == opendrape_testkit::garments::BODICE_PARTICLES)
    });
}

#[test]
fn stats_text_is_readable() {
    assert_eq!(
        OpenDrapeApp::stats_text(59.6, 11.73, 4794),
        "60 fps · simulation 11.7 ms per step · 4794 points"
    );
}
