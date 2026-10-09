//! M4a in the pattern window: placing pieces in 3D with Place at… and by typing, each one undo
//! step.

mod common;
use common::*;
use egui::Key;
use egui_kittest::kittest::Queryable;
use glam::{DQuat, DVec3};
use opendrape::editor::Selection;
use opendrape::stage::Stage;
use opendrape_core::{PieceId, Placement, Point2};
use opendrape_mesh::place;

/// The pattern window with the form to place pieces round.
fn harness_with_form() -> H {
    let mut h = harness();
    h.state_mut().stage = Some(Stage::shared());
    h.run();
    h
}

fn placement(h: &H, id: PieceId) -> Option<Placement> {
    h.state().doc.project().placement_of(id)
}

#[test]
fn place_at_front_wraps_the_piece_round_the_form_as_one_step() {
    let mut h = harness_with_form();
    let id = with_rectangle(&mut h);
    let height = h.state().placement(id).unwrap().position[1];
    right_click(&mut h, 250.0, 300.0);
    assert_eq!(h.state().selection, Selection::Piece(id));
    h.get_by_label("Place at front").click();
    h.run();
    let p = placement(&h, id).expect("placed");
    let r = p.curve.expect("curved round the form");
    assert!(
        p.position[0].abs() < 1e-9 && (p.position[2] - r).abs() < 1e-9,
        "{p:?}"
    );
    assert!((p.position[1] - height).abs() < 1e-9, "keeps its height");
    // Clear of the form: no point of the piece is inside it.
    let stage = Stage::shared();
    let centre = place::centre_of(&opendrape_geom::shapes(h.state().doc.project())[0]);
    for (x, y) in SQUARE {
        let q = place::apply(&p, centre, Point2::new(x, y));
        assert!(stage.signed_distance(q) > 0.0, "{q} is inside the form");
    }
    cmd(&mut h, Key::Z);
    assert_eq!(placement(&h, id), None);
}

#[test]
fn flat_takes_the_curve_away_and_leaves_the_piece_where_it_is() {
    let mut h = harness_with_form();
    let id = with_rectangle(&mut h);
    h.state_mut().place_at(id, place::PlaceAt::Back);
    let curved = placement(&h, id).unwrap();
    right_click(&mut h, 250.0, 300.0);
    h.get_by_label("Flat").click();
    h.run();
    assert_eq!(
        placement(&h, id),
        Some(Placement {
            curve: None,
            ..curved
        })
    );
}

#[test]
fn a_pair_placed_at_the_back_meets_at_the_centre_line() {
    let mut h = harness_with_form();
    let id = with_rectangle(&mut h); // 100..400; its twin will sit to its right
    let twin = h
        .state_mut()
        .doc
        .edit(|p| p.add_twin(id, "Front (mirror)".into(), Point2::new(950.0, 0.0)))
        .unwrap();
    h.state_mut().place_at(id, place::PlaceAt::Back);
    let p = placement(&h, id).unwrap();
    let shapes = opendrape_geom::shapes(h.state().doc.project());
    // The piece's right side faces its twin: it lies on the back centre line.
    let corner = place::apply(&p, place::centre_of(&shapes[0]), Point2::new(400.0, 300.0));
    assert!(corner.x.abs() < 1e-9 && corner.z < 0.0, "{corner}");
    // The twin has no placement of its own: it mirrors the piece and meets it there.
    let t = h.state().placement(twin).unwrap();
    let twin_corner = place::apply(&t, place::centre_of(&shapes[1]), Point2::new(550.0, 300.0));
    assert!(
        (twin_corner - corner).length() < 1e-9,
        "{twin_corner} vs {corner}"
    );
}

#[test]
fn typed_position_and_rotation_move_the_piece_one_step_each() {
    let mut h = harness_with_form();
    let id = with_rectangle(&mut h);
    click(&mut h, 250.0, 300.0);
    type_into(&mut h, "Position Y", "100");
    assert!((placement(&h, id).unwrap().position[1] - 1.0).abs() < 1e-9);
    type_into(&mut h, "Rotation Y", "90");
    let q = DQuat::from_array(placement(&h, id).unwrap().rotation);
    assert!(
        (q * DVec3::Z - DVec3::X).length() < 1e-9,
        "turned to face the form's left"
    );
    assert_eq!(field_text(&h, "Rotation Y"), "90.0");
    cmd(&mut h, Key::Z);
    assert_eq!(placement(&h, id).unwrap().rotation, Placement::NO_ROTATION);
    cmd(&mut h, Key::Z);
    assert_eq!(placement(&h, id), None);
}

#[test]
fn a_typed_twin_gets_its_own_placement() {
    let mut h = harness_with_form();
    let id = with_rectangle(&mut h);
    let twin = h
        .state_mut()
        .doc
        .edit(|p| p.add_twin(id, "Front (mirror)".into(), Point2::new(950.0, 0.0)))
        .unwrap();
    h.state_mut().selection = Selection::Piece(twin);
    h.run();
    type_into(&mut h, "Position Z", "-30");
    let own = placement(&h, twin).expect("its own now");
    assert!((own.position[2] + 0.3).abs() < 1e-9);
    assert_eq!(placement(&h, id), None, "the piece stays unplaced");
}

#[test]
fn without_a_form_there_is_no_place_at_menu() {
    let mut h = harness();
    with_rectangle(&mut h);
    right_click(&mut h, 250.0, 300.0);
    assert!(h.query_by_label("Place at front").is_none());
}
