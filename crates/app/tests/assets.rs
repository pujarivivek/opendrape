//! The Assets section: a toggle in the menu row that shows the asset list where the pattern
//! usually is, with a 3D | 2D switch on the left area while it's open in Modeling.

use egui::{Key, Modifiers};
use egui_kittest::{Harness, kittest::Queryable};
use opendrape::editor::Tool;
use opendrape::gpu::{Decision, GpuChoice, GpuState, Reason, StateStore};
use opendrape::{FileDialogs, OpenDrapeApp, Recovery, SharedState, Startup};
use std::path::Path;

/// Harness steps one `run()` may take: the 3D view finishes its still image one frame at a
/// time (up to 32 frames) after anything changes.
const MAX_STEPS: u64 = 48;

type App = Harness<'static, OpenDrapeApp>;

fn harness_sized(config_dir: &Path, size: egui::Vec2) -> App {
    let startup = Startup {
        decision: Decision {
            choice: GpuChoice::Auto,
            reason: Reason::Saved,
        },
        previous: GpuState::default(),
        store: StateStore::new(Some(config_dir)),
        smoke_test: false,
        file_dialogs: FileDialogs::always_cancel(),
        recovery: Recovery::new(None),
    };
    let mut h = Harness::builder()
        .with_size(size)
        .with_max_steps(MAX_STEPS)
        .wgpu()
        .build_eframe(move |cc| OpenDrapeApp::new(cc, startup, SharedState::default()));
    h.run();
    h
}

fn harness(config_dir: &Path) -> App {
    harness_sized(config_dir, egui::vec2(1000.0, 700.0))
}

fn cmd(h: &mut App, k: Key) {
    h.key_press_modifiers(Modifiers::COMMAND, k);
    h.run();
}

fn open_assets(h: &mut App) {
    h.get_by_label("Assets").click();
    h.run();
}

#[test]
fn assets_opens_from_the_menu_row_and_closes_three_ways() {
    let dir = tempfile::tempdir().unwrap();
    let mut h = harness(dir.path());
    assert!(!h.state().assets_open());
    open_assets(&mut h);
    assert!(h.state().assets_open());
    h.get_by_label("Dress forms");
    // Clicked again: closed.
    open_assets(&mut h);
    assert!(!h.state().assets_open());
    assert!(h.query_by_label("Dress forms").is_none());
    // Its ✕.
    open_assets(&mut h);
    h.get_by_label("Close assets").click();
    h.run();
    assert!(!h.state().assets_open());
    // A tab: choosing one shows that stage.
    open_assets(&mut h);
    cmd(&mut h, Key::Num3);
    assert!(!h.state().assets_open(), "Cmd+3 shows Texturing");
    open_assets(&mut h);
    h.get_by_label("Modeling").click();
    h.run();
    assert!(!h.state().assets_open(), "a tab click shows that tab");
}

#[test]
fn the_area_switch_shows_only_while_assets_is_open_in_modeling() {
    let dir = tempfile::tempdir().unwrap();
    let mut h = harness(dir.path());
    assert!(h.query_by_label("Show the pattern").is_none());
    open_assets(&mut h);
    h.get_by_label("Show the 3D view");
    h.get_by_label("Show the pattern");
    // In another tab, Assets opens without it: only Modeling has a pattern.
    cmd(&mut h, Key::Num2);
    open_assets(&mut h);
    assert!(h.state().assets_open());
    assert!(h.query_by_label("Show the pattern").is_none());
    cmd(&mut h, Key::Num1);
    open_assets(&mut h);
    h.get_by_label("Show the pattern").click();
    h.run();
    assert!(h.state().left_shows_pattern());
    h.get_by_label("Pen (H)");
    assert!(h.query_by_label("Play").is_none(), "the 3D view gives way");
    h.key_press(Key::H);
    h.run();
    assert_eq!(
        h.state().editor().tool,
        Tool::Pen,
        "the pattern's keys work there"
    );
    h.get_by_label("Show the 3D view").click();
    h.run();
    assert!(!h.state().left_shows_pattern());
    h.get_by_label("Play");
    // Closing Assets puts the 3D view back on the left and the pattern on the right.
    h.get_by_label("Show the pattern").click();
    h.run();
    open_assets(&mut h);
    assert!(!h.state().left_shows_pattern());
    h.get_by_label("Play");
    h.get_by_label("Pen (H)");
}

#[test]
fn undo_works_while_assets_hides_the_pattern() {
    let dir = tempfile::tempdir().unwrap();
    let mut h = harness(dir.path());
    h.state_mut().editor_mut().doc.edit(|p| {
        p.add_piece(opendrape_core::Piece::rectangle(
            opendrape_core::PieceId(0),
            "Front",
            opendrape_core::Point2::new(0.0, 0.0),
            300.0,
            500.0,
        ))
    });
    h.run();
    open_assets(&mut h);
    cmd(&mut h, Key::Z);
    assert!(h.state().editor().doc.project().pieces.is_empty());
}

#[test]
fn a_tiny_window_with_assets_open_does_not_crash() {
    let dir = tempfile::tempdir().unwrap();
    let mut h = harness_sized(dir.path(), egui::vec2(320.0, 240.0));
    h.state_mut().set_assets_open(true);
    h.run();
    h.state_mut().set_left_shows_pattern(true);
    h.run();
    assert!(h.state().left_shows_pattern());
}
