use egui_kittest::{
    Harness,
    kittest::{NodeT, Queryable},
};
use opendrape::editor::{Selection, Tool};
use opendrape::gpu::{Decision, GpuChoice, GpuState, Reason, StateStore};
use opendrape::{FileDialogs, OpenDrapeApp, Recovery, Shared, SharedState, Startup};
use opendrape_core::{
    Edge, EdgeProps, Half, InternalLine, LineKind, Notch, NotchStyle, Piece, PieceId, Point2,
    Project, SeamSide, Vertex,
};
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
        file_dialogs,
        recovery: Recovery::new(None),
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
        file_dialogs: FileDialogs::always_cancel(),
        recovery: Recovery::new(None),
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
        file_dialogs: FileDialogs::always_cancel(),
        recovery: Recovery::new(None),
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

/// Two rectangles sewn along one side, drawn in the pattern window.
fn add_sewn_pieces(h: &mut App) {
    h.state_mut().editor_mut().doc.edit(|p| {
        let a = p.add_piece(Piece::rectangle(
            PieceId(0),
            "Front",
            Point2::new(0.0, 0.0),
            200.0,
            300.0,
        ));
        let b = p.add_piece(Piece::rectangle(
            PieceId(0),
            "Back",
            Point2::new(300.0, 0.0),
            200.0,
            300.0,
        ));
        p.add_seam(
            SeamSide::new(a, Half::Drawn, 1, 1, true),
            SeamSide::new(b, Half::Drawn, 3, 1, false),
        );
    });
    h.run();
}

#[test]
fn play_drapes_the_pattern_and_reset_returns_to_arranging() {
    let dir = tempfile::tempdir().unwrap();
    let mut h = harness(dir.path(), SharedState::default());
    h.run();
    add_sewn_pieces(&mut h);
    assert!(
        h.state().sim_frame().is_none() && !h.state().is_draping(),
        "arranging"
    );
    h.get_by_label("Play").click();
    h.run_steps(2);
    wait_until(&mut h, "the drape to advance", |a| {
        a.sim_frame().is_some_and(|f| f.time > 0.05)
    });
    h.get_by_label("Press Reset to move pieces.");
    h.get_by_label("Pause").click();
    h.run_steps(3);
    h.get_by_label("Play");
    h.get_by_label("Reset").click();
    h.run_steps(2);
    wait_until(&mut h, "arranging again", |a| a.sim_frame().is_none());
    assert!(!h.state().is_draping());
    assert!(h.query_by_label("Press Reset to move pieces.").is_none());
}

#[test]
fn editing_the_pattern_while_draped_returns_to_arranging() {
    let dir = tempfile::tempdir().unwrap();
    let mut h = harness(dir.path(), SharedState::default());
    h.run();
    add_sewn_pieces(&mut h);
    h.get_by_label("Play").click();
    h.run_steps(2);
    wait_until(&mut h, "the drape", |a| a.sim_frame().is_some());
    h.state_mut()
        .editor_mut()
        .doc
        .edit(|p| p.pieces[0].name = "Front left".into());
    h.run_steps(2);
    wait_until(&mut h, "arranging again", |a| a.sim_frame().is_none());
    assert!(!h.state().is_draping());
}

#[test]
fn a_piece_that_cannot_be_made_into_fabric_is_named() {
    let dir = tempfile::tempdir().unwrap();
    let mut h = harness(dir.path(), SharedState::default());
    h.run();
    h.state_mut().editor_mut().doc.edit(|p| {
        p.add_piece(Piece::polygon(
            PieceId(0),
            "Front",
            &[
                Point2::new(0.0, 0.0),
                Point2::new(200.0, 200.0),
                Point2::new(200.0, 0.0),
                Point2::new(0.0, 200.0),
            ],
        ))
    });
    h.run();
    h.get_by_label("Play").click();
    h.run_steps(2);
    wait_until(&mut h, "the drape", |a| a.sim_frame().is_some());
    h.run_steps(1);
    h.get_by_label("Front couldn't be made into fabric: its outline crosses itself.");
}

#[test]
fn play_with_nothing_drawn_shows_just_the_form() {
    let dir = tempfile::tempdir().unwrap();
    let mut h = harness(dir.path(), SharedState::default());
    h.run();
    h.get_by_label("Play").click();
    h.run_steps(2);
    wait_until(&mut h, "the empty drape", |a| a.sim_frame().is_some());
    h.run_steps(3);
    assert!(h.state().sim_frame().unwrap().positions.is_empty());
    h.get_by_label("Reset").click();
    h.run_steps(2);
    wait_until(&mut h, "arranging again", |a| a.sim_frame().is_none());
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

/// A bodice with an allowance of its own and per edge, a hem, notches of each style, an open
/// marking line and a closed curved cut-out, paired with a twin that sits higher than it, and a
/// sleeve cut on the fold.
fn detailed_project() -> Project {
    let at = Point2::new;
    let mut project = Project::new();
    let mut bodice = Piece::rectangle(PieceId(0), "Bodice", at(0.0, 0.0), 400.0, 600.0);
    bodice.allowance = 12.0;
    bodice.edge_props[0] = EdgeProps {
        allowance: Some(5.5),
        hem: false,
    };
    bodice.edge_props[1].hem = true;
    bodice.edges[3] = Edge::Curve {
        c1: at(-40.0, 450.0),
        c2: at(-30.0, 150.0),
    };
    bodice.notches = vec![
        Notch::new(0, 50.0),
        Notch {
            marks: 2,
            style: NotchStyle::V,
            ..Notch::new(0, 120.0)
        },
        Notch {
            marks: 3,
            ..Notch::new(1, 200.0)
        },
    ];
    bodice.lines = vec![
        InternalLine::open(&[at(60.0, 120.0), at(340.0, 120.0)]),
        InternalLine {
            vertices: vec![
                Vertex::corner(at(150.0, 400.0)),
                Vertex::corner(at(250.0, 400.0)),
                Vertex::corner(at(200.0, 500.0)),
            ],
            edges: vec![
                Edge::Curve {
                    c1: at(180.0, 380.0),
                    c2: at(220.0, 380.0),
                },
                Edge::Line,
                Edge::Line,
            ],
            closed: true,
            kind: LineKind::Cutout,
        },
    ];
    let id = project.add_piece(bodice);
    project.add_twin(id, "Bodice (mirror)".into(), at(905.0, 25.0));
    let mut sleeve = Piece::rectangle(PieceId(0), "Sleeve", at(1500.0, 0.0), 200.0, 500.0);
    sleeve.fold = Some(3);
    project.add_piece(sleeve);
    assert_eq!(project.check(), Ok(()));
    project
}

#[test]
fn a_detailed_project_saves_reopens_and_undoes_step_by_step() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("detailed.odp");
    let mut h = harness_with(
        dir.path(),
        SharedState::default(),
        FileDialogs::scripted(vec![Some(file.clone()), Some(file.clone())]),
    );
    h.run();
    let saved = detailed_project();
    h.state_mut().editor_mut().set_project(saved.clone(), None);
    h.run();
    file_menu(&mut h, "Save As…");
    assert_eq!(opendrape_io::load(&file).unwrap(), saved);
    assert!(!h.state().editor().doc.is_dirty());

    // More work, then Open: the question about it, and then the file as it was saved.
    h.state_mut().editor_mut().doc.edit(|p| {
        p.piece_mut(PieceId(1)).unwrap().notches.clear();
    });
    h.run();
    file_menu(&mut h, "Open…");
    h.get_by_label("Don't save").click();
    h.run();
    assert_eq!(*h.state().editor().doc.project(), saved);
    assert_eq!(h.state().window_title(), "detailed.odp — OpenDrape");
    assert!(!h.state().editor().can_undo(), "a fresh history");

    // Several steps of work on the reopened project, each its own undo step.
    let mut stages = vec![saved.clone()];
    let steps: [fn(&mut Project); 5] = [
        |p| {
            p.piece_mut(PieceId(1))
                .unwrap()
                .notches
                .push(Notch::new(2, 77.0))
        },
        |p| {
            let bodice = p.piece_mut(PieceId(1)).unwrap();
            bodice.edge_props[2].allowance = Some(0.0);
            bodice.allowance = 8.0;
        },
        |p| {
            let bodice = p.piece_mut(PieceId(1)).unwrap();
            bodice.lines.remove(0);
            bodice.notches[1].marks = 3;
        },
        |p| p.piece_mut(PieceId(3)).unwrap().fold = None,
        |p| {
            p.piece_mut(PieceId(1))
                .unwrap()
                .twin
                .as_mut()
                .unwrap()
                .offset
                .y = 90.0
        },
    ];
    for step in &steps {
        h.state_mut().editor_mut().doc.edit(step);
        h.run();
        let now = h.state().editor().doc.project().clone();
        assert_ne!(now, *stages.last().unwrap());
        stages.push(now);
    }
    for expected in stages.iter().rev().skip(1) {
        h.state_mut().editor_mut().undo();
        h.run();
        assert_eq!(h.state().editor().doc.project(), expected);
    }
    assert!(!h.state().editor().can_undo(), "back to the file as opened");
    h.state_mut().editor_mut().undo(); // nothing more to undo
    assert_eq!(*h.state().editor().doc.project(), saved);
    // The keyboard does the same, and redo comes back.
    h.state_mut().editor_mut().redo();
    h.run();
    assert_eq!(h.state().editor().doc.project(), &stages[1]);
    h.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::Z);
    h.run();
    assert_eq!(*h.state().editor().doc.project(), saved);
    for _ in 1..stages.len() {
        h.state_mut().editor_mut().redo();
    }
    h.run();
    assert_eq!(h.state().editor().doc.project(), stages.last().unwrap());
    // And the last version saves and opens again as it is.
    h.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::S);
    h.run();
    assert_eq!(&opendrape_io::load(&file).unwrap(), stages.last().unwrap());
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

fn harness_recovering(config_dir: &Path, recovery_dir: &Path) -> Harness<'static, OpenDrapeApp> {
    harness_recovering_with(config_dir, recovery_dir, FileDialogs::always_cancel())
}

fn harness_recovering_with(
    config_dir: &Path,
    recovery_dir: &Path,
    file_dialogs: FileDialogs,
) -> Harness<'static, OpenDrapeApp> {
    let startup = Startup {
        decision: SAVED_AUTO,
        previous: GpuState::default(),
        store: StateStore::new(Some(config_dir)),
        smoke_test: false,
        file_dialogs,
        recovery: Recovery::new(Some(recovery_dir)),
    };
    Harness::builder()
        .with_size(egui::vec2(1000.0, 700.0))
        .wgpu()
        .build_eframe(move |cc| OpenDrapeApp::new(cc, startup, SharedState::default()))
}

#[test]
fn quitting_without_asking_keeps_a_copy_that_is_offered_next_time() {
    use eframe::App as _;
    let (config, rescue) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let mut h = harness_recovering(config.path(), rescue.path());
    h.run();
    add_piece(&mut h);
    h.state_mut().on_exit(); // what the Dock's Quit, logout and shutdown lead to
    assert!(rescue.path().join("recovery.odp").exists());
    drop(h);

    let mut h = harness_recovering(config.path(), rescue.path());
    h.run();
    h.get_by_label("Restore unsaved work?");
    h.get_by_label("Restore").click();
    h.run();
    assert_eq!(pieces(&h), 1);
    assert!(
        h.state().editor().doc.is_dirty(),
        "restored work is still unsaved"
    );
    assert!(!rescue.path().join("recovery.odp").exists());
}

#[test]
fn dont_save_leaves_no_copy() {
    use eframe::App as _;
    let (config, rescue) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let mut h = harness_recovering(config.path(), rescue.path());
    h.run();
    add_piece(&mut h);
    h.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::Q);
    h.run();
    h.get_by_label("Don't save").click();
    h.run();
    assert!(h.state().is_closing());
    h.state_mut().on_exit();
    assert!(!rescue.path().join("recovery.odp").exists());
}

#[test]
fn a_saved_project_leaves_no_copy() {
    use eframe::App as _;
    let (config, rescue) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let mut h = harness_recovering(config.path(), rescue.path());
    h.run();
    h.state_mut().on_exit(); // nothing changed
    assert!(!rescue.path().join("recovery.odp").exists());
}

#[test]
fn discarding_the_copy_deletes_it() {
    let (config, rescue) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let mut project = opendrape_core::Project::new();
    project.add_piece(Piece::rectangle(
        PieceId(0),
        "Front",
        Point2::new(0.0, 0.0),
        300.0,
        500.0,
    ));
    Recovery::new(Some(rescue.path())).write(&project, None);
    let mut h = harness_recovering(config.path(), rescue.path());
    h.run();
    h.get_by_label("Discard").click();
    h.run();
    assert_eq!(pieces(&h), 0);
    assert!(!rescue.path().join("recovery.odp").exists());
}

// The brief's tests above cover the main paths. The ones below pin down the rest of the
// behaviour it describes: the file the work came from, every other way of quitting, the pattern
// keys, and a copy that will not open.

#[test]
fn restored_work_remembers_its_file_and_save_writes_there() {
    use eframe::App as _;
    let (config, rescue) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let original = config.path().join("skirt.odp");
    let mut h = harness_recovering(config.path(), rescue.path());
    h.run();
    add_piece(&mut h);
    h.state_mut().editor_mut().doc.path = Some(original.clone());
    h.state_mut().on_exit();
    assert!(rescue.path().join("recovery-origin.txt").exists());
    drop(h);

    let mut h = harness_recovering(config.path(), rescue.path());
    h.run();
    h.get_by_label("Restore").click();
    h.run();
    assert_eq!(h.state().editor().doc.path, Some(original.clone()));
    assert_eq!(h.state().window_title(), "• skirt.odp — OpenDrape");
    assert!(!original.exists());
    h.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::S);
    h.run();
    assert_eq!(opendrape_io::load(&original).unwrap().pieces.len(), 1);
    assert!(!h.state().editor().doc.is_dirty());
}

#[test]
fn restored_untitled_work_has_no_file() {
    use eframe::App as _;
    let (config, rescue) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let mut h = harness_recovering(config.path(), rescue.path());
    h.run();
    add_piece(&mut h);
    h.state_mut().on_exit();
    drop(h);

    let mut h = harness_recovering(config.path(), rescue.path());
    h.run();
    h.get_by_label("Restore").click();
    h.run();
    assert_eq!(h.state().editor().doc.path, None);
    assert_eq!(h.state().window_title(), "• Untitled — OpenDrape");
}

#[test]
fn a_cancelled_quit_still_keeps_a_copy_when_the_app_is_ended_anyway() {
    use eframe::App as _;
    let (config, rescue) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let mut h = harness_recovering(config.path(), rescue.path());
    h.run();
    add_piece(&mut h);
    h.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::Q);
    h.run();
    h.get_by_label("Cancel").click();
    h.run();
    assert!(!h.state().is_closing());
    h.state_mut().on_exit();
    assert!(rescue.path().join("recovery.odp").exists());
}

#[test]
fn saving_then_quitting_leaves_no_copy() {
    use eframe::App as _;
    let (config, rescue) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let mut h = harness_recovering(config.path(), rescue.path());
    h.run();
    add_piece(&mut h);
    let file = config.path().join("skirt.odp");
    h.state_mut().editor_mut().doc.path = Some(file.clone());
    h.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::Q);
    h.run();
    h.get_by_label("Save").click();
    h.run();
    assert!(h.state().is_closing());
    assert!(file.exists());
    h.state_mut().on_exit();
    assert!(!rescue.path().join("recovery.odp").exists());
}

#[test]
fn switching_graphics_without_saving_leaves_no_copy() {
    use eframe::App as _;
    let (config, rescue) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let mut h = harness_recovering(config.path(), rescue.path());
    h.run();
    add_piece(&mut h);
    h.state_mut().choose_graphics(GpuChoice::Software);
    h.run();
    h.get_by_label("Don't save").click();
    h.run();
    assert!(h.state().is_closing());
    h.state_mut().on_exit();
    assert!(!rescue.path().join("recovery.odp").exists());
}

#[test]
fn the_pattern_ignores_keys_while_the_restore_question_is_shown() {
    let (config, rescue) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let mut project = opendrape_core::Project::new();
    project.add_piece(Piece::rectangle(
        PieceId(0),
        "Back",
        Point2::new(0.0, 0.0),
        300.0,
        500.0,
    ));
    Recovery::new(Some(rescue.path())).write(&project, None);
    let mut h = harness_recovering(config.path(), rescue.path());
    h.run();
    add_piece(&mut h);
    h.state_mut().editor_mut().selection = Selection::Piece(PieceId(1));
    h.get_by_label("Restore unsaved work?");
    h.key_press(egui::Key::Delete);
    h.run();
    assert_eq!(
        pieces(&h),
        1,
        "Delete reached the pattern behind the question"
    );
    h.get_by_label("Discard").click();
    h.run();
    h.key_press(egui::Key::Delete);
    h.run();
    assert_eq!(pieces(&h), 0);
}

#[test]
fn a_copy_that_will_not_open_is_dropped_without_a_question() {
    let (config, rescue) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    std::fs::write(rescue.path().join("recovery.odp"), b"not a zip file").unwrap();
    std::fs::write(rescue.path().join("recovery-origin.txt"), "/work/skirt.odp").unwrap();
    let mut h = harness_recovering(config.path(), rescue.path());
    h.run();
    assert!(h.query_by_label("Restore unsaved work?").is_none());
    assert!(!rescue.path().join("recovery.odp").exists());
    assert!(!rescue.path().join("recovery-origin.txt").exists());
    assert_eq!(pieces(&h), 0);
}

#[test]
fn quitting_with_the_restore_question_unanswered_keeps_the_copy() {
    use eframe::App as _;
    let (config, rescue) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let mut project = opendrape_core::Project::new();
    project.add_piece(Piece::rectangle(
        PieceId(0),
        "Front",
        Point2::new(0.0, 0.0),
        300.0,
        500.0,
    ));
    Recovery::new(Some(rescue.path())).write(&project, None);
    let mut h = harness_recovering(config.path(), rescue.path());
    h.run();
    h.get_by_label("Restore unsaved work?");
    h.state_mut().on_exit(); // the student never answered, and nothing in the editor is unsaved
    assert!(
        rescue.path().join("recovery.odp").exists(),
        "the waiting copy must survive"
    );
}

/// A recovery copy holding one piece, waiting in `dir`.
fn leave_a_copy(dir: &Path) {
    let mut project = opendrape_core::Project::new();
    project.add_piece(Piece::rectangle(
        PieceId(0),
        "Front",
        Point2::new(0.0, 0.0),
        300.0,
        500.0,
    ));
    Recovery::new(Some(dir)).write(&project, None);
}

#[test]
fn file_shortcuts_do_nothing_while_the_restore_question_is_shown() {
    let (config, rescue) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    leave_a_copy(rescue.path());
    let dialogs = FileDialogs::scripted(vec![Some(config.path().join("other.odp"))]);
    let FileDialogs::Scripted(unused_answers) = &dialogs else {
        unreachable!("scripted")
    };
    let unused_answers = unused_answers.clone();
    let mut h = harness_recovering_with(config.path(), rescue.path(), dialogs);
    h.run();
    add_piece(&mut h); // unsaved, so New and Open would ask "Save your changes?"
    let save_as = egui::Modifiers {
        shift: true,
        ..egui::Modifiers::COMMAND
    };
    for (modifiers, key) in [
        (egui::Modifiers::COMMAND, egui::Key::N),
        (egui::Modifiers::COMMAND, egui::Key::O),
        (egui::Modifiers::COMMAND, egui::Key::S),
        (save_as, egui::Key::S),
    ] {
        h.key_press_modifiers(modifiers, key);
        h.run();
    }
    h.get_by_label("Restore unsaved work?");
    assert!(h.query_by_label("Save your changes?").is_none());
    assert_eq!(unused_answers.borrow().len(), 1, "a file dialog was opened");
    assert!(rescue.path().join("recovery.odp").exists());

    // Once the question is answered the same shortcuts work again.
    h.get_by_label("Discard").click();
    h.run();
    h.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::N);
    h.run();
    h.get_by_label("Save your changes?");
}

#[test]
fn quit_still_works_while_the_restore_question_is_shown() {
    let (config, rescue) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    leave_a_copy(rescue.path());
    let mut h = harness_recovering(config.path(), rescue.path());
    h.run();
    h.get_by_label("Restore unsaved work?");
    h.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::Q);
    h.run();
    assert!(h.state().is_closing());
    assert!(
        rescue.path().join("recovery.odp").exists(),
        "quitting without an answer keeps the copy for next time"
    );
}

/// Presses and releases the primary button at screen point `pos`.
fn click_at(h: &mut App, pos: glam::DVec2) {
    let p = egui::pos2(pos.x as f32, pos.y as f32);
    h.hover_at(p);
    for pressed in [true, false] {
        h.event(egui::Event::PointerButton {
            pos: p,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        });
    }
    h.run();
}

#[test]
fn clicking_a_piece_in_3d_selects_it_in_the_pattern_window() {
    let dir = tempfile::tempdir().unwrap();
    let mut h = harness(dir.path(), SharedState::default());
    h.run();
    add_piece(&mut h);
    let camera = h.state().view_camera().expect("the 3D view was drawn");
    let centre = h.state_mut().arranged_scene().panels[0].placement.position;
    let on_piece = camera.project(glam::DVec3::from_array(centre)).unwrap();
    click_at(&mut h, on_piece);
    assert_eq!(h.state().editor().selection, Selection::Piece(PieceId(1)));
    // The view's top-left corner: only background there.
    let (corner, _) = camera.rect();
    click_at(&mut h, corner + glam::DVec2::new(8.0, 8.0));
    assert_eq!(h.state().editor().selection, Selection::None);
}

#[test]
fn the_view_buttons_turn_the_camera_to_each_side() {
    use std::f32::consts::{FRAC_PI_2, PI};
    let dir = tempfile::tempdir().unwrap();
    let mut h = harness(dir.path(), SharedState::default());
    h.run();
    for (label, yaw) in [
        ("Back", PI),
        ("Left side", FRAC_PI_2),
        ("Right side", -FRAC_PI_2),
        ("Front", 0.0),
    ] {
        h.get_by_label(label).click();
        h.run();
        let camera = h.state().orbit_camera().unwrap();
        assert!((camera.yaw - yaw).abs() < 1e-6, "{label}: {}", camera.yaw);
    }
}
