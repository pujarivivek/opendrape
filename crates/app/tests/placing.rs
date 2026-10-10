//! M4a in the pattern window: placing pieces in 3D with Place at… and by typing, each one undo
//! step.

mod common;
use common::*;
use egui::Key;
use egui_kittest::kittest::{NodeT, Queryable};
use glam::{DQuat, DVec3};
use opendrape::editor::Selection;
use opendrape::stage::Stage;
use opendrape_core::{Piece, PieceId, Placement, Point2};
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

/// A 340 × 220 mm sleeve at (100,100) with its twin to its right, at 600..940.
fn with_sleeves(h: &mut H) -> (PieceId, PieceId) {
    let (sleeve, twin) = h.state_mut().doc.edit(|p| {
        let id = p.add_piece(Piece::rectangle(
            PieceId(0),
            "Sleeve",
            Point2::new(100.0, 100.0),
            340.0,
            220.0,
        ));
        let twin = p
            .add_twin(id, "Sleeve (mirror)".into(), Point2::new(1040.0, 0.0))
            .unwrap();
        (id, twin)
    });
    h.state_mut().fit();
    h.run();
    (sleeve, twin)
}

/// How far each point of shape `id`'s outline is from arm `arm`'s line, in 3D: (nearest,
/// farthest).
fn round_arm(h: &H, id: PieceId, arm: usize) -> (f64, f64) {
    let shapes = opendrape_geom::shapes(h.state().doc.project());
    let shape = shapes.iter().find(|s| s.id == id).unwrap();
    let p = h.state().placement(id).unwrap();
    let line = Stage::shared().arms().expect("the bundled body has arms")[arm];
    opendrape_geom::outline_points(&shape.piece, 0.5)
        .into_iter()
        .map(|q| line.distance(place::apply(&p, place::centre_of(shape), q)))
        .fold((f64::MAX, f64::MIN), |(lo, hi), d| (lo.min(d), hi.max(d)))
}

#[test]
fn place_at_left_arm_puts_a_sleeve_round_it_and_its_twin_round_the_other_as_one_step() {
    let mut h = harness_with_form();
    let (sleeve, twin) = with_sleeves(&mut h);
    right_click(&mut h, 250.0, 200.0);
    h.get_by_label("Place at left arm").click();
    h.run();
    let p = placement(&h, sleeve).expect("placed");
    let r = p.curve.expect("curved round the arm");
    let (near, far) = round_arm(&h, sleeve, 0);
    assert!(
        (near - r).abs() < 1e-6 && (far - r).abs() < 1e-6,
        "{near}..{far} vs {r}"
    );
    assert_eq!(
        placement(&h, twin),
        None,
        "the twin takes the sleeve's, mirrored"
    );
    let (near, far) = round_arm(&h, twin, 1);
    assert!(
        (near - r).abs() < 1e-6 && (far - r).abs() < 1e-6,
        "{near}..{far}"
    );
    cmd(&mut h, Key::Z);
    assert_eq!((placement(&h, sleeve), placement(&h, twin)), (None, None));
}

#[test]
fn placing_a_twin_at_an_arm_puts_its_piece_round_the_other() {
    let mut h = harness_with_form();
    let (sleeve, twin) = with_sleeves(&mut h);
    h.state_mut().place_at_arm(twin, 1);
    h.run();
    let (near, far) = round_arm(&h, twin, 1);
    let r = placement(&h, twin).unwrap().curve.unwrap();
    assert!((near - r).abs() < 1e-6 && (far - r).abs() < 1e-6);
    let (near, far) = round_arm(&h, sleeve, 0);
    assert!(
        (near - r).abs() < 1e-6 && (far - r).abs() < 1e-6,
        "{near}..{far}"
    );
}

#[test]
fn while_draping_placements_are_not_offered() {
    let mut h = harness_with_form();
    let id = with_rectangle(&mut h);
    h.state_mut().draping = true;
    click(&mut h, 250.0, 300.0);
    h.get_by_label("Placements apply after Reset.");
    type_into(&mut h, "Position Y", "100");
    assert_eq!(placement(&h, id), None, "the field is greyed out");
    right_click(&mut h, 250.0, 300.0);
    h.get_by_label("Place at left arm").click();
    h.run();
    assert_eq!(placement(&h, id), None, "and so is Place at…");
}

/// A form with no arms: a closed box 34 cm wide, 1.6 m tall and 24 cm deep, centred on the
/// centre line.
fn armless_form() -> std::sync::Arc<Stage> {
    let positions: Vec<glam::Vec3> = (0..8)
        .map(|k| {
            glam::Vec3::new(
                if k & 1 == 0 { -0.17 } else { 0.17 },
                if k & 2 == 0 { 0.0 } else { 1.6 },
                if k & 4 == 0 { -0.12 } else { 0.12 },
            )
        })
        .collect();
    let mut triangles: Vec<[u32; 3]> = [
        [0, 2, 3],
        [0, 3, 1],
        [4, 5, 7],
        [4, 7, 6],
        [0, 4, 6],
        [0, 6, 2],
        [1, 3, 7],
        [1, 7, 5],
        [0, 1, 5],
        [0, 5, 4],
        [2, 6, 7],
        [2, 7, 3],
    ]
    .to_vec();
    // Outwards: a closed surface with a negative volume is inside out.
    let volume: f32 = triangles
        .iter()
        .map(|t| {
            let [a, b, c] = t.map(|i| positions[i as usize]);
            a.dot(b.cross(c))
        })
        .sum();
    if volume < 0.0 {
        triangles.iter_mut().for_each(|t| t.swap(1, 2));
    }
    std::sync::Arc::new(Stage::from_mesh(positions, triangles).expect("a closed box"))
}

#[test]
fn on_a_form_without_arms_place_at_arm_is_greyed_out_and_says_why() {
    let mut h = harness();
    let form = armless_form();
    assert!(form.arms().is_none());
    h.state_mut().stage = Some(form);
    h.run();
    let (sleeve, twin) = with_sleeves(&mut h);
    right_click(&mut h, 250.0, 200.0);
    for label in ["Place at left arm", "Place at right arm"] {
        assert!(
            h.get_by_label(label).accesskit_node().is_disabled(),
            "{label}"
        );
    }
    assert!(
        !h.get_by_label("Place at front")
            .accesskit_node()
            .is_disabled()
    );
    h.get_by_label("Place at left arm").click();
    h.run();
    assert_eq!((placement(&h, sleeve), placement(&h, twin)), (None, None));
    // Asked for anyway, it changes nothing and the notice says why.
    let undo_before = h.state().doc.can_undo();
    h.state_mut().place_at_arm(sleeve, 1);
    h.run();
    assert_eq!((placement(&h, sleeve), placement(&h, twin)), (None, None));
    assert_eq!(h.state().doc.can_undo(), undo_before, "not an undo step");
    assert_eq!(h.state().notice.as_deref(), Some("This form has no arms."));
    // The other places still work on it.
    h.state_mut().place_at(sleeve, place::PlaceAt::Front);
    assert!(placement(&h, sleeve).is_some_and(|p| p.curve.is_some()));
}
