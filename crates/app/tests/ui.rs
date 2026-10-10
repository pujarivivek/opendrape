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
            SeamSide::edges(a, Half::Drawn, 1, 1, true),
            SeamSide::edges(b, Half::Drawn, 3, 3, false),
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
fn editing_the_pattern_while_draped_carries_the_drape_on() {
    let dir = tempfile::tempdir().unwrap();
    let mut h = harness(dir.path(), SharedState::default());
    h.run();
    add_sewn_pieces(&mut h);
    h.get_by_label("Play").click();
    h.run_steps(2);
    wait_until(&mut h, "the drape", |a| {
        a.sim_frame().is_some_and(|f| f.time > 0.2)
    });
    let before = h.state().sim_frame().unwrap();
    // The back made 60 mm longer at its hem, as in the pattern window.
    h.state_mut().editor_mut().doc.edit(|p| {
        for v in 0..2 {
            let at = p.pieces[1].vertices[v].pos;
            p.pieces[1].move_vertex(v, at - Point2::new(0.0, 60.0));
        }
    });
    h.run_steps(2);
    wait_until(&mut h, "the longer drape", |a| {
        a.sim_frame()
            .is_some_and(|f| f.positions.len() > before.positions.len())
    });
    let after = h.state().sim_frame().unwrap();
    assert!(h.state().is_draping(), "still draping: no Reset");
    assert_eq!(after.drape, before.drape, "the same drape, carried on");
    h.get_by_label("Press Reset to move pieces.");
    // Undo is an edit too: the drape carries on with the shorter back again.
    h.state_mut().editor_mut().undo();
    h.run_steps(2);
    wait_until(&mut h, "the shorter drape", |a| {
        a.sim_frame()
            .is_some_and(|f| f.positions.len() == before.positions.len())
    });
    assert!(h.state().is_draping());
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
fn only_five_drape_notes_are_listed_and_the_rest_are_counted() {
    let dir = tempfile::tempdir().unwrap();
    let mut h = harness(dir.path(), SharedState::default());
    h.run();
    // Eight outlines that cross themselves: eight notes.
    h.state_mut().editor_mut().doc.edit(|p| {
        for k in 0..8 {
            p.add_piece(Piece::polygon(
                PieceId(0),
                format!("Bow {}", k + 1),
                &[
                    Point2::new(0.0, 0.0),
                    Point2::new(100.0, 100.0),
                    Point2::new(100.0, 0.0),
                    Point2::new(0.0, 100.0),
                ],
            ));
        }
    });
    h.run();
    h.get_by_label("Play").click();
    h.run_steps(2);
    wait_until(&mut h, "the drape", |a| a.sim_frame().is_some());
    h.run_steps(2);
    for k in 1..=5 {
        h.get_by_label(&format!(
            "Bow {k} couldn't be made into fabric: its outline crosses itself."
        ));
    }
    assert!(
        h.query_by_label("Bow 6 couldn't be made into fabric: its outline crosses itself.")
            .is_none()
    );
    h.get_by_label("…and 3 more");
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

#[test]
fn right_clicking_a_piece_in_3d_offers_place_at() {
    let dir = tempfile::tempdir().unwrap();
    let mut h = harness(dir.path(), SharedState::default());
    h.run();
    add_piece(&mut h);
    let camera = h.state().view_camera().expect("the 3D view was drawn");
    let centre = h.state_mut().arranged_scene().panels[0].placement.position;
    let p = camera.project(glam::DVec3::from_array(centre)).unwrap();
    let p = egui::pos2(p.x as f32, p.y as f32);
    h.hover_at(p);
    for pressed in [true, false] {
        h.event(egui::Event::PointerButton {
            pos: p,
            button: egui::PointerButton::Secondary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        });
    }
    h.run();
    assert_eq!(h.state().editor().selection, Selection::Piece(PieceId(1)));
    h.get_by_label("Place at front").click();
    h.run();
    let placed = h.state().editor().doc.project().placement_of(PieceId(1));
    assert!(placed.is_some_and(|p| p.curve.is_some()), "{placed:?}");
}

#[test]
fn right_clicking_the_draping_fabric_pins_it_there() {
    let dir = tempfile::tempdir().unwrap();
    let mut h = harness(dir.path(), SharedState::default());
    h.run();
    // A piece hanging upright in front of the form, facing the camera.
    h.state_mut().editor_mut().doc.edit(|p| {
        let id = p.add_piece(Piece::rectangle(
            PieceId(0),
            "Front",
            Point2::new(0.0, 0.0),
            300.0,
            400.0,
        ));
        p.set_placement(id, Some(opendrape_core::Placement::at([0.0, 1.0, 0.5])));
    });
    h.run();
    h.get_by_label("Play").click();
    h.run_steps(2);
    wait_until(&mut h, "the drape", |a| a.sim_frame().is_some());
    h.run_steps(1);
    h.get_by_label(
        "Drag the fabric to pull it. Right-click it to pin it there; drag a pin to move it.",
    );
    // Gravity waits a moment at the start: the piece is still where it was placed.
    let camera = h.state().view_camera().expect("the 3D view was drawn");
    let p = camera.project(glam::DVec3::new(0.0, 1.0, 0.5)).unwrap();
    let p = egui::pos2(p.x as f32, p.y as f32);
    h.hover_at(p);
    for pressed in [true, false] {
        h.event(egui::Event::PointerButton {
            pos: p,
            button: egui::PointerButton::Secondary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        });
    }
    h.run_steps(2);
    h.get_by_label("Pin here").click();
    h.run_steps(2);
    let pins = h.state().editor().doc.project().pins.clone();
    assert_eq!(pins.len(), 1, "pinned");
    assert_eq!(pins[0].shape, PieceId(1));
    assert!(
        pins[0].at.distance(Point2::new(150.0, 200.0)) < 10.0,
        "{:?}",
        pins[0].at
    );
    assert_eq!(h.state().editor().selection, Selection::Pin(0));
    assert!(h.state().is_draping(), "pinning carries the drape on");
}

// The gizmo in the 3D view of the real app (drawn off-screen): who gets a press, when it is
// live, and what the camera does while a handle is held. The maths and the undo steps are
// tested without a window in `tests/arrange.rs`.

use glam::DVec2;
use opendrape::arrange::gizmo::{ARROW_PT, AXES, GRAZING, Gizmo, Handle, RING_PT, ring_angle};

fn screen(p: DVec2) -> egui::Pos2 {
    egui::pos2(p.x as f32, p.y as f32)
}

fn pointer_button(h: &mut App, at: DVec2, pressed: bool) {
    h.event(egui::Event::PointerButton {
        pos: screen(at),
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: egui::Modifiers::NONE,
    });
}

/// Presses at `from` and moves to `to` in four frames, holding the button.
fn grab_and_pull(h: &mut App, from: DVec2, to: DVec2) {
    h.hover_at(screen(from));
    h.step();
    pointer_button(h, from, true);
    h.step();
    for k in 1..=4 {
        h.hover_at(screen(from.lerp(to, f64::from(k) / 4.0)));
        h.step();
    }
}

fn let_go(h: &mut App, at: DVec2) {
    pointer_button(h, at, false);
    h.step();
}

/// A piece in the pattern window, selected, with the 3D view drawn: the view's camera and the
/// piece's gizmo.
fn piece_with_gizmo(h: &mut App) -> (opendrape::arrange::ScreenCamera, Gizmo) {
    add_piece(h);
    h.state_mut().editor_mut().selection = Selection::Piece(PieceId(1));
    h.run();
    let cam = h.state().view_camera().expect("the 3D view was drawn");
    let scene = h.state_mut().arranged_scene();
    let g = opendrape::arrange::Arranger::gizmo(&cam, &scene, &Selection::Piece(PieceId(1)))
        .expect("a selected piece has a gizmo");
    (cam, g)
}

fn near_tip(cam: &opendrape::arrange::ScreenCamera, g: &Gizmo, axis: usize) -> DVec2 {
    cam.project(g.arrow_tip(axis) - AXES[axis] * g.size * 0.1)
        .unwrap()
}

fn own_place(h: &App) -> Option<opendrape_core::Placement> {
    h.state().editor().doc.project().placement_of(PieceId(1))
}

#[test]
fn a_click_on_the_gizmo_keeps_the_piece_selected() {
    let dir = tempfile::tempdir().unwrap();
    let mut h = harness(dir.path(), SharedState::default());
    h.run();
    let (cam, g) = piece_with_gizmo(&mut h);
    // The tip of the x arrow, the middle of the y arrow and the centre square: nothing of the
    // piece is under the tips, and a press and release there is a click, not a drag.
    for at in [
        near_tip(&cam, &g, 0),
        near_tip(&cam, &g, 1),
        cam.project(g.centre).unwrap(),
    ] {
        assert!(g.hit(&cam, at).is_some());
        click_at(&mut h, at);
        assert_eq!(
            h.state().editor().selection,
            Selection::Piece(PieceId(1)),
            "a click on the gizmo at {at}"
        );
    }
    assert_eq!(own_place(&h), None, "and it moved nothing");
    // A click on nothing still clears it.
    let (corner, _) = cam.rect();
    click_at(&mut h, corner + DVec2::new(8.0, 8.0));
    assert_eq!(h.state().editor().selection, Selection::None);
}

#[test]
fn the_handle_under_the_pointer_is_lit_until_the_pointer_leaves_the_view() {
    let dir = tempfile::tempdir().unwrap();
    let mut h = harness(dir.path(), SharedState::default());
    h.run();
    let (cam, g) = piece_with_gizmo(&mut h);
    h.hover_at(screen(near_tip(&cam, &g, 2)));
    h.step();
    assert_eq!(
        h.state().arranger().hovered,
        Some(opendrape::arrange::gizmo::Handle::Move(2))
    );
    // Over the pattern window on the right.
    h.hover_at(egui::pos2(900.0, 300.0));
    h.step();
    assert_eq!(h.state().arranger().hovered, None);
    // Back on the handle, and then the selection goes while the pointer stays there.
    h.hover_at(screen(near_tip(&cam, &g, 2)));
    h.step();
    assert!(h.state().arranger().hovered.is_some());
    h.state_mut().editor_mut().selection = Selection::None;
    h.step();
    assert_eq!(h.state().arranger().hovered, None);
}

#[test]
fn the_camera_stays_put_while_a_handle_is_held_scroll_zoom_included() {
    let dir = tempfile::tempdir().unwrap();
    let mut h = harness(dir.path(), SharedState::default());
    h.run();
    let (cam, g) = piece_with_gizmo(&mut h);
    let before = h.state().orbit_camera().unwrap();
    let from = near_tip(&cam, &g, 1);
    grab_and_pull(&mut h, from, from + DVec2::new(0.0, -30.0));
    assert!(h.state().arranger().is_dragging());
    for _ in 0..3 {
        h.event(egui::Event::MouseWheel {
            unit: egui::MouseWheelUnit::Point,
            delta: egui::vec2(0.0, 60.0),
            phase: egui::TouchPhase::Move,
            modifiers: egui::Modifiers::NONE,
        });
        h.step();
    }
    let held = h.state().orbit_camera().unwrap();
    assert_eq!(
        (held.yaw, held.pitch, held.distance),
        (before.yaw, before.pitch, before.distance),
        "no orbit and no zoom while a handle is held"
    );
    let_go(&mut h, from + DVec2::new(0.0, -30.0));
    assert!(!h.state().arranger().is_dragging());
    assert!(own_place(&h).is_some(), "the piece moved");
    // The scroll wheel does zoom when nothing is held.
    for _ in 0..3 {
        h.event(egui::Event::MouseWheel {
            unit: egui::MouseWheelUnit::Point,
            delta: egui::vec2(0.0, 60.0),
            phase: egui::TouchPhase::Move,
            modifiers: egui::Modifiers::NONE,
        });
        h.step();
    }
    assert!(h.state().orbit_camera().unwrap().distance < before.distance);
}

#[test]
fn escape_gives_a_gizmo_drag_up_and_the_piece_goes_back_with_no_undo_step() {
    let dir = tempfile::tempdir().unwrap();
    let mut h = harness(dir.path(), SharedState::default());
    h.run();
    let (cam, g) = piece_with_gizmo(&mut h);
    let before = h.state().editor().doc.project().clone();
    let from = near_tip(&cam, &g, 1);
    grab_and_pull(&mut h, from, from + DVec2::new(0.0, -40.0));
    assert!(own_place(&h).is_some(), "it followed the pointer");
    h.key_press(egui::Key::Escape);
    h.step();
    assert!(!h.state().arranger().is_dragging());
    assert_eq!(
        *h.state().editor().doc.project(),
        before,
        "back where it was"
    );
    assert_eq!(
        h.state().editor().selection,
        Selection::Piece(PieceId(1)),
        "Escape was the drag's, not the pattern window's"
    );
    // The pointer going on moving, and letting go, change nothing.
    h.hover_at(screen(from + DVec2::new(0.0, -60.0)));
    h.step();
    let_go(&mut h, from + DVec2::new(0.0, -60.0));
    assert_eq!(*h.state().editor().doc.project(), before);
    assert_eq!(
        h.state().editor().selection,
        Selection::Piece(PieceId(1)),
        "letting go after Escape is not a click on the background"
    );
    // The only step in the history is the piece being drawn.
    h.state_mut().editor_mut().undo();
    assert_eq!(pieces(&h), 0);
    assert!(!h.state().editor().can_undo());
}

#[test]
fn a_refused_gizmo_move_shows_the_notice_once_per_drag() {
    let dir = tempfile::tempdir().unwrap();
    let mut h = harness(dir.path(), SharedState::default());
    h.run();
    // A project that is already invalid (an internal line of one point) refuses every change.
    let mut project = Project::new();
    let mut piece = Piece::rectangle(PieceId(0), "Front", Point2::new(0.0, 0.0), 300.0, 500.0);
    piece.lines = vec![InternalLine::open(&[Point2::new(10.0, 10.0)])];
    project.add_piece(piece);
    assert!(project.check().is_err());
    h.state_mut().editor_mut().set_project(project, None);
    h.state_mut().editor_mut().selection = Selection::Piece(PieceId(1));
    h.run();
    let cam = h.state().view_camera().expect("the 3D view was drawn");
    let scene = h.state_mut().arranged_scene();
    let g = opendrape::arrange::Arranger::gizmo(&cam, &scene, &Selection::Piece(PieceId(1)))
        .expect("a gizmo");
    let from = cam.project(g.centre).unwrap();
    h.hover_at(screen(from));
    h.step();
    pointer_button(&mut h, from, true);
    h.step();
    h.hover_at(screen(from + DVec2::new(20.0, 0.0)));
    h.step();
    h.hover_at(screen(from + DVec2::new(30.0, 0.0)));
    h.step();
    let refused = "That change can't be made: the pattern would become too large or invalid.";
    assert_eq!(h.state().editor().notice.as_deref(), Some(refused));
    // Dismissed, it does not come back with the next move of the same drag.
    h.state_mut().editor_mut().notice = None;
    h.hover_at(screen(from + DVec2::new(40.0, 0.0)));
    h.step();
    h.hover_at(screen(from + DVec2::new(50.0, 0.0)));
    h.step();
    assert_eq!(h.state().editor().notice, None);
    let_go(&mut h, from + DVec2::new(50.0, 0.0));
    assert_eq!(own_place(&h), None, "nothing was written");
}

#[test]
fn the_gizmo_is_not_live_from_the_moment_play_is_pressed() {
    let dir = tempfile::tempdir().unwrap();
    let mut h = harness(dir.path(), SharedState::default());
    h.run();
    // Big, so that making its fabric takes the simulation thread longer than this test needs
    // frames to press and pull at a handle.
    h.state_mut().editor_mut().doc.edit(|p| {
        p.add_piece(Piece::rectangle(
            PieceId(0),
            "Front",
            Point2::new(0.0, 0.0),
            2400.0,
            2400.0,
        ))
    });
    h.state_mut().editor_mut().selection = Selection::Piece(PieceId(1));
    h.run();
    let cam = h.state().view_camera().expect("the 3D view was drawn");
    let scene = h.state_mut().arranged_scene();
    let g = opendrape::arrange::Arranger::gizmo(&cam, &scene, &Selection::Piece(PieceId(1)))
        .expect("a gizmo");
    let from = near_tip(&cam, &g, 1);
    let project = h.state().editor().doc.project().clone();

    // Between Play and the first frame of the drape, the pointer goes to the y arrow, presses,
    // pulls, and lets go. If the computer was so slow that the first frame came before that was
    // over, the attempt means nothing and is made again.
    let mut waited = 0;
    for _attempt in 0..10 {
        h.get_by_label("Play").click();
        h.step();
        assert!(h.state().is_draping(), "Play was pressed");
        waited = 0;
        let mut watch = |h: &mut App| {
            h.step();
            let app = h.state();
            if app.is_draping() && app.sim_frame().is_none() {
                waited += 1;
            }
            assert!(
                !app.arranger().is_dragging() && app.arranger().hovered.is_none(),
                "the gizmo is not live"
            );
        };
        h.hover_at(screen(from));
        watch(&mut h);
        pointer_button(&mut h, from, true);
        watch(&mut h);
        for k in 1..=4 {
            h.hover_at(screen(from + DVec2::new(0.0, -10.0 * f64::from(k))));
            watch(&mut h);
        }
        pointer_button(&mut h, from, false);
        watch(&mut h);
        assert_eq!(
            *h.state().editor().doc.project(),
            project,
            "the piece was not moved"
        );
        assert!(
            h.state().is_draping(),
            "so the drape was not reset by an edit"
        );
        if waited >= 7 {
            break;
        }
        h.get_by_label("Reset").click();
        h.run_steps(2);
        wait_until(&mut h, "arranging again", |a| !a.is_draping());
    }
    assert!(waited >= 7, "the drape was always ahead of the test");
}

#[test]
fn a_gizmo_drag_in_the_real_view_is_one_undo_step() {
    let dir = tempfile::tempdir().unwrap();
    let mut h = harness(dir.path(), SharedState::default());
    h.run();
    let (cam, g) = piece_with_gizmo(&mut h);
    let from = near_tip(&cam, &g, 1);
    let to = from + DVec2::new(0.0, -40.0);
    grab_and_pull(&mut h, from, to);
    let_go(&mut h, to);
    let moved = own_place(&h).expect("it moved");
    assert!(moved.position[1] > g.centre.y, "up the screen is up");
    h.state_mut().editor_mut().undo();
    assert_eq!(own_place(&h), None, "one step undoes the whole drag");
    assert_eq!(pieces(&h), 1);
}

#[test]
fn a_gizmo_drag_still_held_when_play_is_pressed_ends_where_it_is() {
    let dir = tempfile::tempdir().unwrap();
    let mut h = harness(dir.path(), SharedState::default());
    h.run();
    h.state_mut().editor_mut().doc.edit(|p| {
        p.add_piece(Piece::rectangle(
            PieceId(0),
            "Front",
            Point2::new(0.0, 0.0),
            2400.0,
            2400.0,
        ))
    });
    h.state_mut().editor_mut().selection = Selection::Piece(PieceId(1));
    h.run();
    let cam = h.state().view_camera().expect("the 3D view was drawn");
    let scene = h.state_mut().arranged_scene();
    let g = opendrape::arrange::Arranger::gizmo(&cam, &scene, &Selection::Piece(PieceId(1)))
        .expect("a gizmo");
    let from = near_tip(&cam, &g, 1);
    grab_and_pull(&mut h, from, from + DVec2::new(0.0, -30.0));
    assert!(h.state().arranger().is_dragging());
    // Play is pressed from the keyboard, with the handle still held.
    h.get_by_label("Play").focus();
    h.key_press(egui::Key::Enter);
    h.step();
    assert!(h.state().is_draping(), "Play was pressed");
    let snapshot = h.state().editor().doc.project().clone();
    h.hover_at(screen(from + DVec2::new(0.0, -50.0)));
    h.step();
    h.hover_at(screen(from + DVec2::new(0.0, -70.0)));
    h.step();
    assert!(
        !h.state().arranger().is_dragging(),
        "the drag ended with Play"
    );
    assert_eq!(
        *h.state().editor().doc.project(),
        snapshot,
        "and moved nothing since"
    );
    assert!(
        h.state().is_draping(),
        "so the drape was not reset by an edit"
    );
    let_go(&mut h, from + DVec2::new(0.0, -70.0));
    // The history has the piece and the drag, as separate steps.
    h.state_mut().editor_mut().undo();
    assert_eq!(own_place(&h), None);
    assert_eq!(pieces(&h), 1);
}

#[test]
fn a_press_on_a_handle_let_go_beyond_its_reach_is_still_the_gizmos_click() {
    let dir = tempfile::tempdir().unwrap();
    let mut h = harness(dir.path(), SharedState::default());
    h.run();
    let (cam, g) = piece_with_gizmo(&mut h);
    // Pressed 7 points off the x arrow (it reaches 8), let go 10 points off it: less than the
    // 6 points that make a drag of it, so it is a click.
    let on_arrow = near_tip(&cam, &g, 0);
    let tip = cam.project(g.arrow_tip(0)).unwrap();
    let side = (tip - cam.project(g.centre).unwrap()).normalize().perp();
    let (pressed, released) = (on_arrow + side * 7.0, on_arrow + side * 10.0);
    assert_eq!(g.hit(&cam, pressed), Some(Handle::Move(0)));
    assert_eq!(g.hit(&cam, released), None);

    // A click that begins and ends there is a click on the background.
    click_at(&mut h, released);
    assert_eq!(h.state().editor().selection, Selection::None);
    h.state_mut().editor_mut().selection = Selection::Piece(PieceId(1));
    h.run();

    // One that begins on the arrow is not, wherever the button comes up.
    h.hover_at(screen(pressed));
    h.step();
    pointer_button(&mut h, pressed, true);
    h.step();
    h.hover_at(screen(released));
    h.step();
    pointer_button(&mut h, released, false);
    h.step();
    assert_eq!(
        h.state().editor().selection,
        Selection::Piece(PieceId(1)),
        "the piece is still selected"
    );
    assert!(!h.state().arranger().is_dragging());
    assert_eq!(own_place(&h), None, "and nothing moved");
    // The next click is its own again, even one that comes down and up in a single frame (the
    // press is then not seen on its own: the release stands for it).
    let one_frame = |h: &mut App, at: DVec2| {
        let p = screen(at);
        h.input_mut().events.push(egui::Event::PointerMoved(p));
        for pressed in [true, false] {
            h.input_mut().events.push(egui::Event::PointerButton {
                pos: p,
                button: egui::PointerButton::Primary,
                pressed,
                modifiers: egui::Modifiers::NONE,
            });
        }
        h.step();
    };
    one_frame(&mut h, pressed);
    assert_eq!(
        h.state().editor().selection,
        Selection::Piece(PieceId(1)),
        "a quick click on the arrow"
    );
    one_frame(&mut h, released);
    assert_eq!(
        h.state().editor().selection,
        Selection::None,
        "a quick click beside it is judged by itself, not by the press before"
    );
}

// The rings in the views the student gets without touching the camera: as the app opens, and
// from each of the four view buttons, with the piece where it starts and lower down.

/// Where in the app a ring was tried.
#[derive(Clone, Copy, Debug)]
struct Where {
    view: &'static str,
    lower: bool,
    axis: usize,
}

/// Calls `try_ring` for every ring seen at a grazing angle in every one of those views.
fn in_each_view(mut try_ring: impl FnMut(&mut App, Where)) -> Vec<Where> {
    let dir = tempfile::tempdir().unwrap();
    let mut tried = Vec::new();
    for lower in [false, true] {
        let mut h = harness(dir.path(), SharedState::default());
        h.run();
        piece_with_gizmo(&mut h);
        if lower {
            // Wrapped round the form at the front, and lowered to the hips.
            h.state_mut()
                .editor_mut()
                .place_at(PieceId(1), opendrape_mesh::place::PlaceAt::Front);
            h.state_mut().editor_mut().doc.edit(|p| {
                let mut at = p.placement_of(PieceId(1)).expect("placed");
                at.position[1] = 0.8;
                p.set_placement(PieceId(1), Some(at));
            });
            h.run();
        }
        for view in ["Default", "Front", "Back", "Left side", "Right side"] {
            if view != "Default" {
                h.get_by_label(view).click();
                h.run();
            }
            let cam = h.state().view_camera().expect("the 3D view was drawn");
            let scene = h.state_mut().arranged_scene();
            let centre =
                glam::DVec3::from_array(scene.panel(PieceId(1)).unwrap().placement.position);
            let looking = (centre - cam.eye()).normalize();
            for (axis, direction) in AXES.into_iter().enumerate() {
                if looking.dot(direction).abs() < GRAZING {
                    let at = Where { view, lower, axis };
                    try_ring(&mut h, at);
                    tried.push(at);
                }
            }
        }
    }
    tried
}

/// The screen direction across ring `axis` of the piece's gizmo: square to the ring's axis.
fn across_the_ring(
    cam: &opendrape::arrange::ScreenCamera,
    centre: glam::DVec3,
    axis: usize,
) -> DVec2 {
    let along = cam.project(centre + AXES[axis] * 0.01).unwrap() - cam.project(centre).unwrap();
    along.normalize().perp()
}

/// Takes hold of the selected piece's ring about `axis`, pulls the pointer 60 points across it
/// in steps of 2 (`way` is 1 or -1), and gives up the drag. The turn (radians, about `axis`)
/// after each step.
fn pull_ring(h: &mut App, axis: usize, way: f64) -> Vec<f64> {
    let cam = h.state().view_camera().expect("the 3D view was drawn");
    let scene = h.state_mut().arranged_scene();
    let selection = Selection::Piece(PieceId(1));
    let g = opendrape::arrange::Arranger::gizmo(&cam, &scene, &selection).expect("a gizmo");
    let before = scene.panel(PieceId(1)).unwrap().placement;
    let grab = g
        .ring(axis)
        .into_iter()
        .filter_map(|p| cam.project(p))
        .find(|p| g.hit(&cam, *p) == Some(Handle::Turn(axis)))
        .expect("a point that grabs the ring");
    let across = across_the_ring(&cam, g.centre, axis) * way;
    let doc = &mut h.state_mut().editor_mut().doc;
    let mut arranger = opendrape::arrange::Arranger::default();
    assert!(arranger.press(&cam, &scene, &selection, doc, grab));
    let mut turns = Vec::new();
    for step in 1..=30 {
        arranger.drag_to(&cam, doc, grab + across * (2.0 * f64::from(step)), false);
        let now = doc.project().placement_of(PieceId(1)).unwrap_or(before);
        let q = glam::DQuat::from_array(now.rotation)
            * glam::DQuat::from_array(before.rotation).inverse();
        turns.push(2.0 * q.xyz().dot(AXES[axis]).atan2(q.w));
    }
    arranger.cancel(doc);
    turns
}

#[test]
fn a_ring_seen_at_a_grazing_angle_turns_steadily_in_every_view_the_app_gives() {
    let tried = in_each_view(|h, at| {
        let (forward, backward) = (pull_ring(h, at.axis, 1.0), pull_ring(h, at.axis, -1.0));
        for (way, turns) in [(1.0, &forward), (-1.0, &backward)] {
            let mut last = 0.0;
            for (step, turned) in turns.iter().enumerate() {
                let by = (turned - last).to_degrees();
                assert!(
                    by.abs() <= 4.0,
                    "{at:?}: step {step} of a 2 point pull turned {by}°"
                );
                assert!(
                    by * way * forward.last().unwrap().signum() >= 0.0,
                    "{at:?}: back and forth"
                );
                last = *turned;
            }
            let total = turns.last().unwrap().to_degrees().abs();
            assert!(
                (30.0..=90.0).contains(&total),
                "{at:?}: a 60 point pull turned {total}°"
            );
        }
        assert!(
            forward.last().unwrap() * backward.last().unwrap() < 0.0,
            "{at:?}: the other way is the other way round"
        );
    });
    // The y ring, which turns a piece about the form, in every view and both heights; and the x
    // ring, which is edge-on from the front.
    for lower in [false, true] {
        for view in ["Default", "Front", "Back", "Left side", "Right side"] {
            assert!(
                tried
                    .iter()
                    .any(|t| t.axis == 1 && t.view == view && t.lower == lower),
                "the y ring in the {view} view (lower: {lower})"
            );
        }
        assert!(
            tried
                .iter()
                .any(|t| t.axis == 0 && t.view == "Front" && t.lower == lower)
        );
    }
}

#[test]
fn a_pointer_going_along_a_grazing_ring_through_its_centre_does_not_make_it_flip() {
    let tried = in_each_view(|h, at| {
        let cam = h.state().view_camera().expect("the 3D view was drawn");
        let scene = h.state_mut().arranged_scene();
        let centre = glam::DVec3::from_array(scene.panel(PieceId(1)).unwrap().placement.position);
        let (origin, across) = (
            cam.project(centre).unwrap(),
            across_the_ring(&cam, centre, at.axis),
        );
        // From one end of the ring, through the middle, to the other end, 2 points at a time.
        let mut last = origin - across * 60.0;
        let mut total = 0.0;
        for step in -29..=30 {
            let to = origin + across * (2.0 * f64::from(step));
            let by = ring_angle(&cam, centre, AXES[at.axis], last, to)
                .unwrap_or_else(|| panic!("{at:?}: no angle at step {step}"));
            assert!(
                by.to_degrees().abs() <= 10.0,
                "{at:?}: step {step} turned {by}"
            );
            total += by;
            last = to;
        }
        assert!(total.to_degrees().abs() > 60.0, "{at:?}: {total} in all");
    });
    // At least the y rings (five views, two heights), and the x rings from the front and back.
    assert!(tried.len() >= 14, "{} rings", tried.len());
}

#[test]
fn a_ring_taken_by_its_far_half_turns_with_the_point_held_in_every_view_the_app_gives() {
    // In the app's own views, with the real pointer: a grazing ring that is tall enough on
    // screen to have two halves turns the way the point held goes, whichever half that is.
    let mut far_halves = Vec::new();
    in_each_view(|h, at| {
        let cam = h.state().view_camera().expect("the 3D view was drawn");
        let scene = h.state_mut().arranged_scene();
        let selection = Selection::Piece(PieceId(1));
        let g = opendrape::arrange::Arranger::gizmo(&cam, &scene, &selection).expect("a gizmo");
        let before = scene.panel(PieceId(1)).unwrap().placement;
        // The ring about this axis: the point `phi` round from the one nearest the eye, and the
        // screen direction it travels in when the ring is turned right-handedly.
        let to_eye = cam.eye() - g.centre;
        let near = (to_eye - AXES[at.axis] * to_eye.dot(AXES[at.axis])).normalize();
        let radius = g.size * RING_PT / ARROW_PT;
        let point = |phi: f64| {
            let p = g.centre + (near * phi.cos() + AXES[at.axis].cross(near) * phi.sin()) * radius;
            let velocity = AXES[at.axis].cross(p - g.centre).normalize();
            let on_screen = cam.project(p).unwrap();
            let heading = (cam.project(p + velocity * 0.001).unwrap() - on_screen).normalize();
            (on_screen, heading)
        };
        let middle = cam.project(g.centre).unwrap();
        let tall = (point(0.0).0 - middle).length();
        if tall < 5.0 {
            return; // too flat for the halves to be told apart
        }
        for (half, sign) in [("near", 1.0), ("far", -1.0)] {
            // Of the points of that half that grab this ring, the one nearest the half's middle.
            let (_, (grab, heading)) = (0..72)
                .map(|k| f64::from(k) * 5f64.to_radians())
                .filter(|phi| phi.cos() * sign >= 0.5)
                .map(|phi| (phi, point(phi)))
                // Not where another handle is as near (where two rings cross), or the grab
                // would be a coin toss: it must grab this ring a point to every side too.
                .filter(|(_, (s, _))| {
                    [(0.0, 0.0), (1.0, 0.0), (-1.0, 0.0), (0.0, 1.0), (0.0, -1.0)]
                        .into_iter()
                        .all(|(x, y)| {
                            g.hit(&cam, *s + DVec2::new(x, y)) == Some(Handle::Turn(at.axis))
                        })
                })
                .max_by(|a, b| (a.0.cos() * sign).total_cmp(&(b.0.cos() * sign)))
                .unwrap_or_else(|| {
                    panic!("{at:?}: a point of the {half} half that grabs the ring")
                });
            let to = grab + heading * 30.0;
            grab_and_pull(h, grab, to);
            let_go(h, to);
            let now = own_place(h).expect("the piece was turned");
            let q = glam::DQuat::from_array(now.rotation)
                * glam::DQuat::from_array(before.rotation).inverse();
            let turned = 2.0 * q.xyz().dot(AXES[at.axis]).atan2(q.w);
            assert!(
                (0.25..=0.7).contains(&turned),
                "{at:?}: the {half} half, pulled the way it goes, turned {}°",
                turned.to_degrees()
            );
            // One undo step, and the next try starts from the same piece.
            assert!(h.state_mut().editor_mut().doc.undo());
            h.run();
            if half == "far" {
                far_halves.push(at);
            }
        }
    });
    // The y ring, in the app as it opens, with the piece where it starts and lowered.
    for lower in [false, true] {
        assert!(
            far_halves
                .iter()
                .any(|t| t.axis == 1 && t.view == "Default" && t.lower == lower),
            "the far half of the y ring in the default view (lower: {lower})"
        );
    }
    assert!(far_halves.len() >= 8, "{far_halves:?}");
}
