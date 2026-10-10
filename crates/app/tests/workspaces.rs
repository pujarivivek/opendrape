//! The five workspaces along the top: switching between them, what each shows, and the keys
//! that must not switch them while something else owns the keyboard or the mouse.

use egui::{Event, Key, Modifiers, PointerButton, accesskit::Role};
use egui_kittest::{
    Harness,
    kittest::{NodeT, Queryable},
};
use opendrape::editor::{Selection, Tool};
use opendrape::gpu::{Decision, GpuChoice, GpuState, Reason, StateStore};
use opendrape::workspace::Workspace;
use opendrape::{FileDialogs, OpenDrapeApp, Recovery, SharedState, Startup};
use opendrape_core::{Piece, PieceId, Point2};
use std::path::Path;

/// Harness steps one `run()` may take: the 3D view finishes its still image one frame at a
/// time (up to 32 frames) after anything changes.
const MAX_STEPS: u64 = 48;

type App = Harness<'static, OpenDrapeApp>;

const TABS: [&str; 5] = [
    "Modeling",
    "Finishing",
    "Texturing",
    "Rendering",
    "Animation",
];

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

fn cmd_shift(h: &mut App, k: Key) {
    h.key_press_modifiers(
        Modifiers {
            shift: true,
            ..Modifiers::COMMAND
        },
        k,
    );
    h.run();
}

fn add_piece(h: &mut App) -> PieceId {
    let id = h.state_mut().editor_mut().doc.edit(|p| {
        p.add_piece(Piece::rectangle(
            PieceId(0),
            "Front",
            Point2::new(0.0, 0.0),
            300.0,
            500.0,
        ))
    });
    h.run();
    id
}

fn pieces(h: &App) -> usize {
    h.state().editor().doc.project().pieces.len()
}

#[test]
fn five_tabs_and_modeling_first() {
    let dir = tempfile::tempdir().unwrap();
    let h = harness(dir.path());
    for tab in TABS {
        h.get_by_role_and_label(Role::Tab, tab);
    }
    assert_eq!(h.state().workspace(), Workspace::Modeling);
    h.get_by_label("Pen (H)");
}

#[test]
fn cmd_3_opens_texturing_with_its_coming_soon_page() {
    let dir = tempfile::tempdir().unwrap();
    let mut h = harness(dir.path());
    cmd(&mut h, Key::Num3);
    assert_eq!(h.state().workspace(), Workspace::Texturing);
    h.get_by_label("Coming in a later update.");
    h.get_by_label("Fabrics, colours and prints for your pieces.");
    assert!(
        h.query_by_label("Pen (H)").is_none(),
        "the pattern tools stay in Modeling"
    );
    h.get_by_label("Play"); // the 3D view is in every workspace
}

#[test]
fn every_shortcut_opens_its_workspace() {
    let dir = tempfile::tempdir().unwrap();
    let mut h = harness(dir.path());
    let keys = [Key::Num1, Key::Num2, Key::Num3, Key::Num4, Key::Num5];
    for (k, ws) in keys.into_iter().zip(Workspace::ALL).rev() {
        cmd(&mut h, k);
        assert_eq!(h.state().workspace(), ws);
    }
}

#[test]
fn clicking_a_tab_switches() {
    let dir = tempfile::tempdir().unwrap();
    let mut h = harness(dir.path());
    h.get_by_role_and_label(Role::Tab, "Rendering").click();
    h.run();
    assert_eq!(h.state().workspace(), Workspace::Rendering);
    h.get_by_role_and_label(Role::Tab, "Modeling").click();
    h.run();
    assert_eq!(h.state().workspace(), Workspace::Modeling);
    h.get_by_label("Pen (H)");
}

#[test]
fn tool_letters_do_nothing_outside_modeling() {
    let dir = tempfile::tempdir().unwrap();
    let mut h = harness(dir.path());
    cmd(&mut h, Key::Num3);
    h.key_press(Key::H);
    h.run();
    cmd(&mut h, Key::Num1);
    assert_eq!(h.state().editor().tool, Tool::Edit);
}

#[test]
fn undo_and_redo_work_in_every_workspace() {
    let dir = tempfile::tempdir().unwrap();
    let mut h = harness(dir.path());
    let id = add_piece(&mut h);
    h.state_mut().editor_mut().selection = Selection::Piece(id);
    h.run();
    cmd(&mut h, Key::Num4);
    cmd(&mut h, Key::Z);
    assert_eq!(pieces(&h), 0);
    assert_eq!(
        h.state().editor().selection,
        Selection::None,
        "nothing stays selected that has gone"
    );
    cmd_shift(&mut h, Key::Z);
    assert_eq!(pieces(&h), 1);
    // And from the Edit menu.
    h.get_by_label("Edit").click();
    h.run();
    h.get_by_label("Undo").click();
    h.run();
    assert_eq!(pieces(&h), 0);
}

#[test]
fn switching_never_marks_the_project_changed() {
    let dir = tempfile::tempdir().unwrap();
    let mut h = harness(dir.path());
    for k in [Key::Num2, Key::Num3, Key::Num4, Key::Num5, Key::Num1] {
        cmd(&mut h, k);
    }
    h.get_by_role_and_label(Role::Tab, "Animation").click();
    h.run();
    assert!(!h.state().editor().doc.is_dirty());
    assert!(
        !h.state().window_title().contains('•'),
        "{}",
        h.state().window_title()
    );
}

#[test]
fn new_and_open_keep_the_workspace() {
    let dir = tempfile::tempdir().unwrap();
    let mut h = harness(dir.path());
    cmd(&mut h, Key::Num5);
    h.get_by_label("File").click();
    h.run();
    h.get_by_label("New").click();
    h.run();
    assert_eq!(h.state().workspace(), Workspace::Animation);
}

#[test]
fn workspace_keys_wait_for_the_save_question() {
    let dir = tempfile::tempdir().unwrap();
    let mut h = harness(dir.path());
    add_piece(&mut h);
    cmd(&mut h, Key::N);
    h.get_by_label("Save your changes?");
    cmd(&mut h, Key::Num3);
    assert_eq!(h.state().workspace(), Workspace::Modeling);
    h.get_by_label("Cancel").click();
    h.run();
    cmd(&mut h, Key::Num3);
    assert_eq!(h.state().workspace(), Workspace::Texturing);
}

#[test]
fn workspace_keys_wait_while_a_field_is_typed_in() {
    let dir = tempfile::tempdir().unwrap();
    let mut h = harness(dir.path());
    let id = add_piece(&mut h);
    h.state_mut().editor_mut().selection = Selection::Piece(id);
    h.run();
    h.get_by_role_and_label(Role::TextInput, "Name").click();
    h.run();
    cmd(&mut h, Key::Num3);
    assert_eq!(h.state().workspace(), Workspace::Modeling);
}

#[test]
fn workspace_keys_wait_while_the_mouse_is_held() {
    let dir = tempfile::tempdir().unwrap();
    let mut h = harness(dir.path());
    let canvas = h.state().editor().canvas_rect;
    let pos = canvas.center();
    h.hover_at(pos);
    let button = |pressed| Event::PointerButton {
        pos,
        button: PointerButton::Primary,
        pressed,
        modifiers: Modifiers::NONE,
    };
    h.event(button(true));
    h.run();
    cmd(&mut h, Key::Num3);
    assert_eq!(h.state().workspace(), Workspace::Modeling);
    h.event(button(false));
    h.run();
    cmd(&mut h, Key::Num3);
    assert_eq!(h.state().workspace(), Workspace::Texturing);
}

#[test]
fn view_menu_lists_and_switches_workspaces() {
    let dir = tempfile::tempdir().unwrap();
    let mut h = harness(dir.path());
    h.get_by_label("View").click();
    h.run();
    for tab in TABS {
        h.get_by_role_and_label(Role::Button, tab);
    }
    h.get_by_role_and_label(Role::Button, "Animation").click();
    h.run();
    assert_eq!(h.state().workspace(), Workspace::Animation);
}

#[test]
fn small_windows_show_every_workspace_without_crashing() {
    for size in [egui::vec2(640.0, 480.0), egui::vec2(300.0, 200.0)] {
        let dir = tempfile::tempdir().unwrap();
        let mut h = harness_sized(dir.path(), size);
        let id = add_piece(&mut h);
        h.state_mut().editor_mut().selection = Selection::Piece(id);
        for ws in Workspace::ALL {
            h.state_mut().set_workspace(ws);
            h.run();
        }
    }
}

/// Play and Reset are icons now: they keep their names, and hovering says what they do.
#[test]
fn the_3d_toolbar_explains_its_icons() {
    let dir = tempfile::tempdir().unwrap();
    let mut h = harness(dir.path());
    h.get_by_label("Play").hover();
    h.run_steps(30);
    h.get_by_label("Play\nDrape the pieces on the form.");
    h.get_by_label("Reset").hover();
    h.run_steps(30);
    h.get_by_label("Reset\nStop draping and go back to arranging the pieces.");
}

/// Clicks a tab the way a mouse does (pointer events, not an accessibility action), so the
/// pattern table sees it as a click elsewhere, as it would any other.
fn click_tab_with_mouse(h: &mut App, tab: &str) {
    let pos = h.get_by_role_and_label(Role::Tab, tab).rect().center();
    h.hover_at(pos);
    h.run();
    for pressed in [true, false] {
        h.event(Event::PointerButton {
            pos,
            button: PointerButton::Primary,
            pressed,
            modifiers: Modifiers::NONE,
        });
        h.run();
    }
}

#[test]
fn a_name_typed_then_a_tab_clicked_is_kept() {
    let dir = tempfile::tempdir().unwrap();
    let mut h = harness(dir.path());
    let id = add_piece(&mut h);
    h.state_mut().editor_mut().selection = Selection::Piece(id);
    h.run();
    h.get_by_role_and_label(Role::TextInput, "Name").click();
    h.run();
    h.get_by_role_and_label(Role::TextInput, "Name")
        .type_text("Sleeve");
    h.run();
    click_tab_with_mouse(&mut h, "Texturing");
    assert_eq!(h.state().workspace(), Workspace::Texturing);
    assert_eq!(h.state().editor().doc.project().name_of(id), Some("Sleeve"));
}

#[test]
fn the_number_box_closes_when_a_tab_is_clicked() {
    let dir = tempfile::tempdir().unwrap();
    let mut h = harness(dir.path());
    h.state_mut().editor_mut().set_tool(Tool::Pen);
    h.run();
    let pos = h.state().editor().canvas_rect.center();
    h.hover_at(pos);
    h.run();
    for pressed in [true, false] {
        h.event(Event::PointerButton {
            pos,
            button: PointerButton::Primary,
            pressed,
            modifiers: Modifiers::NONE,
        });
        h.run();
    }
    h.event(Event::Text("5".into()));
    h.run();
    assert!(
        h.state().editor().length_box_open(),
        "typing a digit opens the box"
    );
    click_tab_with_mouse(&mut h, "Texturing");
    assert_eq!(h.state().workspace(), Workspace::Texturing);
    assert!(
        !h.state().editor().length_box_open(),
        "a click elsewhere cancels the box"
    );
    cmd(&mut h, Key::Num1);
    assert_eq!(h.state().workspace(), Workspace::Modeling);
}

/// Play and Reset sit in a strip down the 3D view's right edge, next to the pattern tools'
/// strip: together, yet each under its own caption.
#[test]
fn the_3d_tools_sit_beside_the_pattern_tools() {
    let dir = tempfile::tempdir().unwrap();
    let h = harness(dir.path());
    let play = h.get_by_label("Play").rect();
    let reset = h.get_by_label("Reset").rect();
    let pen = h.get_by_label("Pen (H)").rect();
    assert!(
        play.max.x <= pen.min.x && pen.min.x - play.max.x < 40.0,
        "Play {play:?} next to Pen {pen:?}"
    );
    assert!(
        reset.min.y > play.max.y,
        "Reset under Play: {play:?} {reset:?}"
    );
    assert!(
        (reset.center().x - play.center().x).abs() < 1.0,
        "one column"
    );
    let (caption_3d, caption_2d) = (h.get_by_label("3D").rect(), h.get_by_label("2D").rect());
    assert!(caption_3d.max.y <= play.min.y && caption_2d.max.y <= pen.min.y);
}

const VIEWS: [&str; 4] = ["Front", "Back", "Left side", "Right side"];

/// The camera views are four small pictures in the 3D view's top-right corner.
#[test]
fn camera_views_are_compact_picture_buttons_in_the_3d_views_top_right() {
    let dir = tempfile::tempdir().unwrap();
    let h = harness(dir.path());
    let play = h.get_by_label("Play").rect();
    let rects = VIEWS.map(|v| h.get_by_label(v).rect());
    for (v, r) in VIEWS.iter().zip(&rects) {
        assert!(r.max.x < play.min.x, "{v} is inside the 3D view: {r:?}");
        assert!(r.min.y < 90.0, "{v} is at the top: {r:?}");
        assert!(
            r.width() <= 24.0 && r.height() <= 26.0,
            "{v} is small: {r:?}"
        );
    }
    assert!(
        rects[3].max.x - rects[0].min.x < 100.0,
        "a compact row: {rects:?}"
    );
    assert!(
        play.min.x - rects[3].max.x < 30.0,
        "in the top-right corner"
    );
    assert!(h.query_by_label("Left side").is_some());
}

#[test]
fn a_camera_view_button_turns_the_camera_and_shows_it_is_current() {
    let dir = tempfile::tempdir().unwrap();
    let mut h = harness(dir.path());
    h.get_by_label("Back").click();
    h.run();
    let yaw = h.state().orbit_camera().unwrap().yaw;
    assert!((yaw - std::f32::consts::PI).abs() < 1e-4, "{yaw}");
    let selected = |h: &App, v: &str| h.get_by_label(v).accesskit_node().is_selected();
    assert_eq!(selected(&h, "Back"), Some(true));
    assert_ne!(selected(&h, "Front"), Some(true));
    h.get_by_label("Right side").click();
    h.run();
    assert_eq!(selected(&h, "Right side"), Some(true));
    assert_ne!(selected(&h, "Back"), Some(true));
    // Hovering says what it does, without repeating the name alone.
    h.get_by_label("Front").hover();
    h.run_steps(30);
    h.get_by_label("Look from the front");
    h.get_by_label("Front");
}

/// While draping, the 3D view shows no instruction lines (only warnings, when there are any).
#[test]
fn draping_shows_no_instruction_lines() {
    let dir = tempfile::tempdir().unwrap();
    let mut h = harness(dir.path());
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
    let start = std::time::Instant::now();
    while h.state().sim_frame().is_none() {
        assert!(start.elapsed().as_secs() < 20, "the drape never started");
        std::thread::sleep(std::time::Duration::from_millis(20));
        h.step();
    }
    h.run_steps(2);
    assert!(h.state().is_draping());
    assert!(h.query_by_label_contains("Press Reset").is_none());
    assert!(h.query_by_label_contains("Drag the fabric").is_none());
}

/// View → 3D quality switches how the 3D view is drawn, and the choice is there next launch.
#[test]
fn the_3d_quality_menu_switches_and_remembers() {
    use opendrape_render::studio::quality::Quality;
    let dir = tempfile::tempdir().unwrap();
    let mut h = harness(dir.path());
    h.get_by_label("View").click();
    h.run();
    // A submenu: egui adds an arrow to its name.
    h.get_by_label_contains("3D quality").click();
    h.run();
    h.get_by_label("Basic").click();
    h.run();
    assert_eq!(h.state().viewport_quality(), Some(Quality::Basic));
    let saved = std::fs::read_to_string(dir.path().join("view.json")).unwrap();
    assert!(saved.contains("basic"), "{saved}");
    drop(h);
    let again = harness(dir.path());
    assert_eq!(again.state().viewport_quality(), Some(Quality::Basic));
}

/// Once nothing moves, the 3D view finishes its image and stops drawing; turning it starts
/// again.
#[test]
fn the_3d_view_stops_drawing_when_still() {
    let dir = tempfile::tempdir().unwrap();
    let mut h = harness(dir.path());
    for _ in 0..80 {
        h.step();
    }
    let settled = h.state().viewport_frames();
    for _ in 0..5 {
        h.step();
    }
    assert_eq!(
        h.state().viewport_frames(),
        settled,
        "nothing drawn once still"
    );
    h.get_by_label("Back").click();
    h.run();
    assert!(
        h.state().viewport_frames() > settled,
        "turning the view draws again"
    );
}

/// View → Lighting switches between soft and sculpted studio light, and is remembered.
#[test]
fn the_lighting_menu_switches_and_remembers() {
    use opendrape_render::studio::Lighting;
    let dir = tempfile::tempdir().unwrap();
    let mut h = harness(dir.path());
    assert_eq!(h.state().viewport_lighting(), Some(Lighting::Sculpted));
    h.get_by_label("View").click();
    h.run();
    h.get_by_label_contains("Lighting").click();
    h.run();
    h.get_by_label("Soft").click();
    h.run();
    assert_eq!(h.state().viewport_lighting(), Some(Lighting::Soft));
    drop(h);
    let again = harness(dir.path());
    assert_eq!(again.state().viewport_lighting(), Some(Lighting::Soft));
}
