//! The five workspaces along the top: switching between them, what each shows, and the keys
//! that must not switch them while something else owns the keyboard or the mouse.

use egui::{Event, Key, Modifiers, PointerButton, accesskit::Role};
use egui_kittest::{Harness, kittest::Queryable};
use opendrape::editor::{Selection, Tool};
use opendrape::gpu::{Decision, GpuChoice, GpuState, Reason, StateStore};
use opendrape::workspace::Workspace;
use opendrape::{FileDialogs, OpenDrapeApp, Recovery, SharedState, Startup};
use opendrape_core::{Piece, PieceId, Point2};
use std::path::Path;

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
