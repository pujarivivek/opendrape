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

// Picking the dress form: its card, its chart and size, or custom measurements.

use egui::accesskit::Role;
use opendrape_core::{FormChoice, FormSize};

fn form(h: &App) -> FormChoice {
    h.state().editor().doc.project().form.clone()
}

/// Types `text` into the measurement field `label` and presses Enter.
fn type_measurement(h: &mut App, label: &str, text: &str) {
    h.get_by_role_and_label(Role::TextInput, label).click();
    h.run();
    h.get_by_role_and_label(Role::TextInput, label)
        .type_text(text);
    h.run();
    h.key_press(Key::Enter);
    h.run();
}

#[test]
fn clicking_the_mens_card_switches_form_and_undo_switches_back() {
    let dir = tempfile::tempdir().unwrap();
    let mut h = harness(dir.path());
    open_assets(&mut h);
    h.get_by_label("Women's torso");
    h.get_by_label("Men's torso").click();
    h.run();
    assert_eq!(form(&h).id, "men-torso");
    assert_eq!(
        form(&h).size,
        FormSize::Chart {
            chart: "classic".into(),
            label: "40".into()
        }
    );
    assert!(h.state().editor().doc.is_dirty());
    h.get_by_label_contains("For shirts");
    cmd(&mut h, Key::Z);
    assert_eq!(form(&h), FormChoice::default());
    h.get_by_label_contains("For dresses");
}

#[test]
fn the_chart_and_size_change_the_form() {
    let dir = tempfile::tempdir().unwrap();
    let mut h = harness(dir.path());
    open_assets(&mut h);
    h.get_by_label("Everyday body").click();
    h.run();
    let f = form(&h);
    assert!(
        matches!(f.size, FormSize::Chart { ref chart, .. } if chart == "everyday"),
        "{f:?}"
    );
    assert!(
        (f.measurements["bust"] - 900.0).abs() < 30.0,
        "the nearest size"
    );
    h.get_by_role(Role::ComboBox).click();
    h.run();
    h.get_by_label_contains("US 14").click();
    h.run();
    let f = form(&h);
    assert!(
        matches!(f.size, FormSize::Chart { ref label, ref chart } if label == "US 14" && chart == "everyday"),
        "{f:?}"
    );
    let bust = h.state().stage().measured()["bust"];
    assert!(
        (bust - f.measurements["bust"]).abs() < 1.0,
        "the form is built at the size: {bust}"
    );
}

#[test]
fn a_custom_waist_applies_on_enter_and_an_impossible_one_is_refused() {
    let dir = tempfile::tempdir().unwrap();
    let mut h = harness(dir.path());
    open_assets(&mut h);
    h.get_by_label("Custom measurements").click();
    h.run();
    assert_eq!(
        h.get_by_role_and_label(Role::TextInput, "Waist")
            .value()
            .as_deref(),
        Some("67.5"),
        "pre-filled from the size, in the project's units"
    );
    type_measurement(&mut h, "Waist", "72");
    let f = form(&h);
    assert_eq!(f.size, FormSize::Custom);
    assert!((f.measurements["waist"] - 720.0).abs() < 1e-9);
    assert!((h.state().stage().measured()["waist"] - 720.0).abs() < 1.0);
    assert_eq!(
        h.get_by_role(Role::ComboBox).value().as_deref(),
        Some("Custom"),
        "the size list says Custom"
    );
    type_measurement(&mut h, "Waist", "200");
    h.get_by_label_contains("Waist can be");
    assert!(
        (form(&h).measurements["waist"] - 720.0).abs() < 1e-9,
        "unchanged"
    );
    assert_eq!(
        h.get_by_role_and_label(Role::TextInput, "Waist")
            .value()
            .as_deref(),
        Some("72.0"),
        "the field shows the form's waist again"
    );
}

#[test]
fn measured_values_are_shown_for_the_form() {
    let dir = tempfile::tempdir().unwrap();
    let mut h = harness(dir.path());
    open_assets(&mut h);
    h.get_by_label("Custom measurements").click();
    h.run();
    h.get_by_label_contains("Front waist length");
    h.get_by_label_contains("Apex to apex");
    h.get_by_label_contains("High hip");
}

#[test]
fn going_up_to_the_largest_size_moves_placed_pieces_out_of_the_form() {
    use opendrape_core::{Piece, PieceId, Point2};
    let dir = tempfile::tempdir().unwrap();
    let mut h = harness(dir.path());
    let id = h.state_mut().editor_mut().doc.edit(|p| {
        p.add_piece(Piece::rectangle(
            PieceId(0),
            "Front",
            Point2::new(0.0, 0.0),
            380.0,
            600.0,
        ))
    });
    h.run();
    h.state_mut()
        .editor_mut()
        .place_at(id, opendrape_mesh::place::PlaceAt::Front);
    h.run();
    let largest = opendrape_drape::choice::chart_choice("women-torso", "classic", "US 18").unwrap();
    h.state_mut().apply_form(largest).unwrap();
    h.run();
    let stage = h.state().stage().clone();
    let project = h.state().editor().doc.project().clone();
    let placement = project.placement_of(id).unwrap();
    let shape = &opendrape_geom::shapes(&project)[0];
    let centre = opendrape_mesh::place::centre_of(shape);
    for q in opendrape_geom::outline_points(&shape.piece, 0.5) {
        let p = opendrape_mesh::place::apply(&placement, centre, q);
        assert!(stage.signed_distance(p) > 0.0, "{p} is inside the form");
    }
    // One undo step takes back the size and the move together.
    cmd(&mut h, Key::Z);
    assert_eq!(form(&h), FormChoice::default());
}

#[test]
fn show_tape_lines_is_remembered_with_the_view_settings() {
    let dir = tempfile::tempdir().unwrap();
    let mut h = harness(dir.path());
    open_assets(&mut h);
    h.get_by_label("Show tape lines").click();
    h.run();
    let saved = std::fs::read_to_string(dir.path().join("view.json")).unwrap();
    assert!(saved.contains(r#""show_tapes": false"#), "{saved}");
}

#[test]
fn letters_typed_in_a_measurement_field_pick_no_pattern_tool() {
    let dir = tempfile::tempdir().unwrap();
    let mut h = harness(dir.path());
    open_assets(&mut h);
    h.get_by_label("Show the pattern").click();
    h.run();
    h.get_by_label("Custom measurements").click();
    h.run();
    h.get_by_role_and_label(Role::TextInput, "Waist").click();
    h.run();
    for k in [Key::H, Key::P, Key::R] {
        h.key_press(k);
        h.run();
    }
    assert_eq!(h.state().editor().tool, Tool::Edit);
    // Escape leaves the field as it was.
    h.key_press(Key::Escape);
    h.run();
    assert_eq!(form(&h), FormChoice::default());
}
