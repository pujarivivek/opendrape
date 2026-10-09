use egui_kittest::{
    Harness,
    kittest::{NodeT, Queryable},
};
use opendrape::editor::{Selection, Tool};
use opendrape::gpu::{Decision, GpuChoice, GpuState, Reason, StateStore};
use opendrape::{FileDialogs, OpenDrapeApp, Shared, SharedState, Startup};
use opendrape_core::{Piece, PieceId, Point2, Project};
use std::{path::Path, rc::Rc};

const SAVED_AUTO: Decision = Decision {
    choice: GpuChoice::Auto,
    reason: Reason::Saved,
};

fn harness(config_dir: &Path, shared: SharedState) -> Harness<'static, OpenDrapeApp> {
    harness_with(config_dir, shared, FileDialogs::always_cancel())
}

fn harness_with(
    config_dir: &Path,
    shared: SharedState,
    file_dialogs: FileDialogs,
) -> Harness<'static, OpenDrapeApp> {
    let startup = Startup {
        decision: SAVED_AUTO,
        previous: GpuState::default(),
        store: StateStore::new(Some(config_dir)),
        smoke_test: false,
        autoplay: false,
        file_dialogs,
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
        file_dialogs: FileDialogs::always_cancel(),
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
        file_dialogs: FileDialogs::always_cancel(),
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
        OpenDrapeApp::stats_text(Some(59.6), 11.73, 4794),
        "60 fps · simulation 11.7 ms per step · 4794 points"
    );
    // Paused: the window only redraws on input, so a frame rate would be misleading.
    assert_eq!(
        OpenDrapeApp::stats_text(None, 11.73, 4794),
        "simulation 11.7 ms per step · 4794 points"
    );
}

type App = Harness<'static, OpenDrapeApp>;

/// A piece drawn in the pattern window, leaving unsaved changes.
fn add_piece(h: &mut App) {
    h.state_mut().editor_mut().doc.edit(|p| {
        p.add_piece(Piece::rectangle(
            PieceId(0),
            "Front",
            Point2::new(0.0, 0.0),
            300.0,
            500.0,
        ))
    });
    h.run();
}

fn file_menu(h: &mut App, item: &str) {
    h.get_by_label("File").click();
    h.run();
    h.get_by_label(item).click();
    h.run();
}

fn pieces(h: &App) -> usize {
    h.state().editor().doc.project().pieces.len()
}

#[test]
fn pattern_window_sits_beside_the_3d_view() {
    let dir = tempfile::tempdir().unwrap();
    let mut h = harness(dir.path(), SharedState::default());
    h.run();
    let canvas = h.state().editor().canvas_rect;
    assert!(
        canvas.width() > 200.0 && canvas.left() > 300.0,
        "{canvas:?}"
    );
    h.get_by_label("Play"); // the 3D controls are still there
    h.get_by_label("Pen (H)");
    assert_eq!(h.state().window_title(), "Untitled — OpenDrape");
}

#[test]
fn save_as_writes_a_project_file() {
    let dir = tempfile::tempdir().unwrap();
    let chosen = dir.path().join("skirt"); // no extension: OpenDrape adds .odp
    let mut h = harness_with(
        dir.path(),
        SharedState::default(),
        FileDialogs::scripted(vec![Some(chosen)]),
    );
    h.run();
    add_piece(&mut h);
    assert_eq!(h.state().window_title(), "• Untitled — OpenDrape");
    file_menu(&mut h, "Save As…");
    let saved = dir.path().join("skirt.odp");
    assert_eq!(
        opendrape_io::load(&saved).unwrap(),
        *h.state().editor().doc.project()
    );
    assert_eq!(h.state().window_title(), "skirt.odp — OpenDrape");
}

#[test]
fn save_shortcut_saves_again_without_asking() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("skirt.odp");
    let mut h = harness_with(
        dir.path(),
        SharedState::default(),
        FileDialogs::scripted(vec![Some(file.clone())]),
    );
    h.run();
    add_piece(&mut h);
    h.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::S);
    h.run();
    assert_eq!(opendrape_io::load(&file).unwrap().pieces.len(), 1);
    add_piece(&mut h);
    h.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::S);
    h.run();
    assert_eq!(
        opendrape_io::load(&file).unwrap().pieces.len(),
        2,
        "same file, no second dialog"
    );
}

#[test]
fn cancelling_the_save_dialog_changes_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let mut h = harness(dir.path(), SharedState::default()); // every dialog is cancelled
    h.run();
    add_piece(&mut h);
    file_menu(&mut h, "Save As…");
    assert!(h.state().editor().doc.is_dirty());
    assert_eq!(h.state().window_title(), "• Untitled — OpenDrape");
}

#[test]
fn open_replaces_the_project() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("bodice.odp");
    let mut project = Project::new();
    project.add_piece(Piece::rectangle(
        PieceId(0),
        "Bodice",
        Point2::new(0.0, 0.0),
        200.0,
        400.0,
    ));
    opendrape_io::save(&project, &file).unwrap();
    let mut h = harness_with(
        dir.path(),
        SharedState::default(),
        FileDialogs::scripted(vec![Some(file)]),
    );
    h.run();
    file_menu(&mut h, "Open…");
    assert_eq!(*h.state().editor().doc.project(), project);
    assert_eq!(h.state().window_title(), "bodice.odp — OpenDrape");
    assert!(!h.state().editor().can_undo(), "a fresh history");
}

#[test]
fn opening_a_bad_file_keeps_the_current_project() {
    let dir = tempfile::tempdir().unwrap();
    let bad = dir.path().join("bad.odp");
    std::fs::write(&bad, b"not a zip file").unwrap();
    let mut h = harness_with(
        dir.path(),
        SharedState::default(),
        FileDialogs::scripted(vec![Some(bad)]),
    );
    h.run();
    add_piece(&mut h);
    let before = h.state().editor().doc.project().clone();
    file_menu(&mut h, "Open…");
    h.get_by_label("Don't save").click(); // there are unsaved changes: asked first
    h.run();
    h.get_by_label_contains("not an OpenDrape project file");
    assert_eq!(*h.state().editor().doc.project(), before);
    h.get_by_label("OK").click();
    h.run();
    assert!(
        h.query_by_label_contains("not an OpenDrape project file")
            .is_none()
    );
}

#[test]
fn new_with_unsaved_changes_asks_first() {
    let dir = tempfile::tempdir().unwrap();
    let mut h = harness(dir.path(), SharedState::default());
    h.run();
    add_piece(&mut h);
    file_menu(&mut h, "New");
    h.get_by_label("Save your changes?");
    h.get_by_label("Cancel").click();
    h.run();
    assert_eq!(pieces(&h), 1, "Cancel keeps the work");
    file_menu(&mut h, "New");
    h.get_by_label("Don't save").click();
    h.run();
    assert_eq!(pieces(&h), 0);
    assert!(!h.state().editor().doc.is_dirty());
}

#[test]
fn saving_from_the_question_then_starts_the_new_project() {
    let dir = tempfile::tempdir().unwrap();
    let chosen = dir.path().join("draft.odp");
    let mut h = harness_with(
        dir.path(),
        SharedState::default(),
        FileDialogs::scripted(vec![Some(chosen.clone())]),
    );
    h.run();
    add_piece(&mut h);
    file_menu(&mut h, "New");
    h.get_by_label("Save").click();
    h.run();
    assert_eq!(opendrape_io::load(&chosen).unwrap().pieces.len(), 1);
    assert_eq!(pieces(&h), 0);
    assert_eq!(h.state().window_title(), "Untitled — OpenDrape");
}

#[test]
fn closing_with_unsaved_changes_asks_first() {
    let dir = tempfile::tempdir().unwrap();
    let mut h = harness(dir.path(), SharedState::default());
    h.run();
    add_piece(&mut h);
    h.input_mut()
        .viewports
        .entry(egui::ViewportId::ROOT)
        .or_default()
        .events
        .push(egui::ViewportEvent::Close);
    h.step();
    let cancelled = h
        .output()
        .viewport_output
        .get(&egui::ViewportId::ROOT)
        .is_some_and(|v| {
            v.commands
                .iter()
                .any(|c| matches!(c, egui::ViewportCommand::CancelClose))
        });
    assert!(cancelled, "the window must stay open");
    h.run();
    assert!(!h.state().is_closing());
    h.get_by_label("Don't save").click();
    h.run();
    assert!(h.state().is_closing());
}

#[test]
fn edit_menu_undoes_and_redoes() {
    let dir = tempfile::tempdir().unwrap();
    let mut h = harness(dir.path(), SharedState::default());
    h.run();
    add_piece(&mut h);
    h.get_by_label("Edit").click();
    h.run();
    h.get_by_label("Undo").click();
    h.run();
    assert_eq!(pieces(&h), 0);
    h.get_by_label("Edit").click();
    h.run();
    h.get_by_label("Redo").click();
    h.run();
    assert_eq!(pieces(&h), 1);
}

#[test]
fn file_menu_items_announce_their_shortcuts() {
    let dir = tempfile::tempdir().unwrap();
    let mut h = harness(dir.path(), SharedState::default());
    h.run();
    h.get_by_label("File").click();
    h.run();
    // The shortcut is the item's keyboard shortcut, not part of its name.
    let shortcut = h
        .get_by_label("Save As…")
        .accesskit_node()
        .data()
        .keyboard_shortcut()
        .map(String::from);
    assert!(shortcut.is_some_and(|s| s.contains('S')));
}

#[test]
fn edit_menu_greys_out_what_cannot_be_done() {
    let dir = tempfile::tempdir().unwrap();
    let mut h = harness(dir.path(), SharedState::default());
    h.run();
    h.get_by_label("Edit").click();
    h.run();
    assert!(
        h.get_by_label("Undo").accesskit_node().is_disabled(),
        "nothing to undo yet"
    );
    assert!(
        h.get_by_label("Redo").accesskit_node().is_disabled(),
        "nothing to redo yet"
    );
    // Escape closes the menu; draw a piece, then look again.
    h.key_press(egui::Key::Escape);
    h.run();
    add_piece(&mut h);
    h.get_by_label("Edit").click();
    h.run();
    assert!(!h.get_by_label("Undo").accesskit_node().is_disabled());
    assert!(h.get_by_label("Redo").accesskit_node().is_disabled());
}

#[test]
fn switching_graphics_with_unsaved_work_and_cancelling_changes_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let shared = SharedState::default();
    let mut h = harness(dir.path(), shared.clone());
    h.run();
    add_piece(&mut h);
    h.state_mut().choose_graphics(GpuChoice::Software);
    h.run();
    h.get_by_label("Save your changes?");
    h.get_by_label("Cancel").click();
    h.run();
    assert_eq!(
        StateStore::new(Some(dir.path())).load(),
        GpuState::default()
    );
    assert_eq!(shared.restart_with.get(), None);
    assert!(!h.state().is_closing());
    assert_eq!(pieces(&h), 1);
}

#[test]
fn switching_graphics_then_cancelling_the_save_dialog_does_not_restart() {
    let dir = tempfile::tempdir().unwrap();
    let shared = SharedState::default();
    let mut h = harness(dir.path(), shared.clone()); // every dialog is cancelled
    h.run();
    add_piece(&mut h);
    h.state_mut().choose_graphics(GpuChoice::Software);
    h.run();
    h.get_by_label("Save").click();
    h.run();
    assert_eq!(
        StateStore::new(Some(dir.path())).load(),
        GpuState::default()
    );
    assert_eq!(shared.restart_with.get(), None);
    assert!(!h.state().is_closing());
    assert!(h.state().editor().doc.is_dirty());
}

#[test]
fn switching_graphics_on_a_clean_project_restarts() {
    let dir = tempfile::tempdir().unwrap();
    let shared = SharedState::default();
    let mut h = harness(dir.path(), shared.clone());
    h.run();
    h.state_mut().choose_graphics(GpuChoice::Software);
    h.run();
    assert_eq!(shared.restart_with.get(), Some(GpuChoice::Software));
    assert_eq!(
        StateStore::new(Some(dir.path())).load().preferred,
        GpuChoice::Software
    );
    assert!(h.state().is_closing());
}

#[test]
fn switching_graphics_and_choosing_not_to_save_restarts() {
    let dir = tempfile::tempdir().unwrap();
    let shared = SharedState::default();
    let mut h = harness(dir.path(), shared.clone());
    h.run();
    add_piece(&mut h);
    h.state_mut().choose_graphics(GpuChoice::Software);
    h.run();
    assert_eq!(
        shared.restart_with.get(),
        None,
        "nothing happens until the question is answered"
    );
    h.get_by_label("Don't save").click();
    h.run();
    assert_eq!(shared.restart_with.get(), Some(GpuChoice::Software));
    assert_eq!(
        StateStore::new(Some(dir.path())).load().preferred,
        GpuChoice::Software
    );
    assert!(h.state().is_closing());
}

#[test]
fn cmd_q_with_unsaved_changes_asks_first() {
    let dir = tempfile::tempdir().unwrap();
    let mut h = harness(dir.path(), SharedState::default());
    h.run();
    add_piece(&mut h);
    h.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::Q);
    h.run();
    h.get_by_label("Save your changes?");
    assert!(!h.state().is_closing());
    h.get_by_label("Cancel").click();
    h.run();
    assert!(!h.state().is_closing());
    assert_eq!(pieces(&h), 1, "Cancel keeps the work");
}

#[test]
fn cmd_q_on_a_clean_project_quits() {
    let dir = tempfile::tempdir().unwrap();
    let mut h = harness(dir.path(), SharedState::default());
    h.run();
    h.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::Q);
    h.run();
    assert!(h.state().is_closing());
}

#[test]
fn file_menu_quit_asks_first() {
    let dir = tempfile::tempdir().unwrap();
    let mut h = harness(dir.path(), SharedState::default());
    h.run();
    add_piece(&mut h);
    file_menu(&mut h, "Quit OpenDrape");
    h.get_by_label("Save your changes?");
    assert!(!h.state().is_closing());
}

/// Clicks the pattern table at (x, y) mm, as a mouse does.
fn canvas_click(h: &mut App, x: f64, y: f64) {
    let ed = h.state().editor();
    let pos = ed.view.to_screen(ed.canvas_rect, Point2::new(x, y));
    h.hover_at(pos);
    for pressed in [true, false] {
        h.event(egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        });
    }
    h.run();
}

/// Starts a pen draft of three points and leaves unsaved changes (a drawn piece).
fn pen_draft_with_unsaved_changes(h: &mut App) {
    h.state_mut().editor_mut().set_tool(Tool::Pen);
    h.run();
    for (x, y) in [(100.0, 100.0), (400.0, 100.0), (400.0, 300.0)] {
        canvas_click(h, x, y);
    }
    assert_eq!(h.state().editor().pen().len(), 3);
    add_piece(h);
}

#[test]
fn escape_closes_the_question_and_leaves_the_pen_draft_alone() {
    let dir = tempfile::tempdir().unwrap();
    let mut h = harness(dir.path(), SharedState::default());
    h.run();
    pen_draft_with_unsaved_changes(&mut h);
    h.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::Q);
    h.run();
    h.get_by_label("Save your changes?");
    h.key_press(egui::Key::Escape);
    h.run();
    assert!(
        h.query_by_label("Save your changes?").is_none(),
        "Escape answers the question"
    );
    assert_eq!(
        h.state().editor().pen().len(),
        3,
        "and must not also cancel the piece being drawn"
    );
}

#[test]
fn delete_does_nothing_to_the_pattern_while_the_question_is_open() {
    let dir = tempfile::tempdir().unwrap();
    let mut h = harness(dir.path(), SharedState::default());
    h.run();
    add_piece(&mut h);
    h.state_mut().editor_mut().selection = Selection::Piece(PieceId(1));
    h.run();
    h.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::Q);
    h.run();
    h.get_by_label("Save your changes?");
    h.key_press(egui::Key::Delete);
    h.run();
    assert_eq!(pieces(&h), 1, "the piece is still there");
    h.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::Z);
    h.run();
    assert_eq!(pieces(&h), 1, "and Cmd+Z did not undo it");
    h.get_by_label("Cancel").click();
    h.run();
    h.key_press(egui::Key::Delete);
    h.run();
    assert_eq!(
        pieces(&h),
        0,
        "once the question is gone, Delete works again"
    );
}

#[test]
fn the_pattern_ignores_keys_while_an_error_is_shown() {
    let dir = tempfile::tempdir().unwrap();
    let bad = dir.path().join("bad.odp");
    std::fs::write(&bad, b"not a zip file").unwrap();
    let mut h = harness_with(
        dir.path(),
        SharedState::default(),
        FileDialogs::scripted(vec![Some(bad)]),
    );
    h.run();
    add_piece(&mut h);
    h.state_mut().editor_mut().selection = Selection::Piece(PieceId(1));
    file_menu(&mut h, "Open…");
    h.get_by_label("Don't save").click();
    h.run();
    h.get_by_label_contains("not an OpenDrape project file");
    h.key_press(egui::Key::Delete);
    h.run();
    assert_eq!(pieces(&h), 1);
    h.get_by_label("OK").click();
    h.run();
    h.key_press(egui::Key::Delete);
    h.run();
    assert_eq!(pieces(&h), 0);
}

#[test]
fn cmd_q_works_while_a_text_field_has_focus() {
    let dir = tempfile::tempdir().unwrap();
    let mut h = harness(dir.path(), SharedState::default());
    h.run();
    add_piece(&mut h);
    h.state_mut().editor_mut().selection = Selection::Piece(PieceId(1));
    h.run();
    h.get_by_role_and_label(egui::accesskit::Role::TextInput, "Name")
        .click();
    h.run();
    assert!(
        h.get_by_role_and_label(egui::accesskit::Role::TextInput, "Name")
            .is_focused()
    );
    h.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::Q);
    h.run();
    h.get_by_label("Save your changes?");
    assert!(!h.state().is_closing());
}
