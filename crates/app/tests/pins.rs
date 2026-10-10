//! M4b pins on the pattern table: they stay on their spot of fabric when the piece moves, and
//! they are drawn, selected and removed there.

mod common;
use common::*;
use egui::Key;
use egui_kittest::kittest::Queryable;
use opendrape::editor::Selection;
use opendrape_core::{Half, Piece, PieceId, Pin, Point2};

fn pin(shape: PieceId, x: f64, y: f64) -> Pin {
    Pin {
        shape,
        half: Half::Drawn,
        at: Point2::new(x, y),
        target: [0.0, 1.0, 0.3],
    }
}

#[test]
fn moving_a_piece_takes_its_pins_and_its_twins_pins_along() {
    let mut h = harness();
    let front = with_rectangle(&mut h); // (100,100)–(400,500)
    let twin = h
        .state_mut()
        .doc
        .edit(|p| p.add_twin(front, "Front (mirror)".into(), Point2::new(1000.0, 0.0)))
        .unwrap(); // shows stored (x, y) at (1000 - x, y)
    let other = h.state_mut().doc.edit(|p| {
        p.add_piece(Piece::rectangle(
            PieceId(0),
            "Other",
            Point2::new(1100.0, 100.0),
            100.0,
            100.0,
        ))
    });
    h.state_mut().doc.edit(|p| {
        p.pins = vec![
            pin(front, 200.0, 200.0),
            pin(twin, 300.0, 400.0),
            pin(other, 1150.0, 150.0),
        ]
    });
    h.state_mut().fit();
    h.run();
    drag(&mut h, (250.0, 300.0), (350.0, 350.0)); // the front, by about (100, 50)
    // Exactly as far as the front moved.
    let d = piece_of(&h, front).vertices[0].pos - Point2::new(100.0, 100.0);
    close(d, Point2::new(100.0, 50.0));
    let pins = h.state().doc.project().pins.clone();
    assert_eq!(
        pins[0].at,
        Point2::new(200.0, 200.0) + d,
        "the same spot of the front"
    );
    assert_eq!(
        pins[1].at,
        Point2::new(300.0, 400.0) + d,
        "kept where the front shows it"
    );
    assert_eq!(
        pins[2].at,
        Point2::new(1150.0, 150.0),
        "another piece's stays"
    );
    // The twin stayed where it was, and so did its pin's spot on it: (1000 - 300, 400).
    let shapes = opendrape_geom::shapes(h.state().doc.project());
    let twin_shape = shapes.iter().find(|s| s.id == twin).unwrap();
    close(
        twin_shape.spot_shown(Half::Drawn, pins[1].at),
        Point2::new(700.0, 400.0),
    );
    cmd(&mut h, egui::Key::Z);
    assert_eq!(
        h.state().doc.project().pins[0].at,
        Point2::new(200.0, 200.0),
        "one step"
    );
}

#[test]
fn pins_show_where_their_shapes_are_and_are_selected_and_removed_there() {
    let mut h = harness();
    let front = with_rectangle(&mut h); // (100,100)–(400,500)
    let twin = h
        .state_mut()
        .doc
        .edit(|p| p.add_twin(front, "Front (mirror)".into(), Point2::new(1000.0, 0.0)))
        .unwrap();
    h.state_mut().doc.edit(|p| {
        p.pins = vec![pin(front, 200.0, 200.0), pin(twin, 300.0, 400.0)];
    });
    h.state_mut().fit();
    h.run();
    // The twin shows its pin at (1000 - 300, 400).
    click(&mut h, 700.0, 400.0);
    assert_eq!(h.state().selection, Selection::Pin(1));
    h.get_by_label("Pin");
    h.get_by_label("On Front (mirror)");
    h.get_by_label("Remove pin").click();
    h.run();
    assert_eq!(h.state().doc.project().pins.len(), 1);
    assert_eq!(h.state().selection, Selection::None);
    cmd(&mut h, Key::Z);
    assert_eq!(h.state().doc.project().pins.len(), 2, "one undo step");
    // A pin on the piece itself, inside it: its marker comes before the piece.
    click(&mut h, 200.0, 200.0);
    assert_eq!(h.state().selection, Selection::Pin(0));
    key(&mut h, Key::Delete);
    assert_eq!(h.state().doc.project().pins, vec![pin(twin, 300.0, 400.0)]);
}

/// Three pins on a 300 × 400 mm rectangle, at x = 150, 250 and 350 (the middle of its bottom).
fn three_pins(h: &mut H) -> (PieceId, Vec<Pin>) {
    let front = with_rectangle(h); // (100,100)–(400,500)
    let pins = vec![
        pin(front, 150.0, 150.0),
        pin(front, 250.0, 150.0),
        pin(front, 350.0, 150.0),
    ];
    h.state_mut().doc.edit(|p| p.pins = pins.clone());
    h.state_mut().fit();
    h.run();
    (front, pins)
}

#[test]
fn removing_a_pin_before_the_selected_one_keeps_the_selection_on_its_own_pin() {
    let mut h = harness();
    let (_, pins) = three_pins(&mut h);
    h.state_mut().selection = Selection::Pin(2);
    h.state_mut().remove_pin(0);
    assert_eq!(
        h.state().selection,
        Selection::Pin(1),
        "the same pin, renumbered"
    );
    key(&mut h, Key::Delete);
    assert_eq!(
        h.state().doc.project().pins,
        vec![pins[1]],
        "the selected pin went, and no other"
    );
    assert_eq!(h.state().selection, Selection::None);
}

#[test]
fn the_selection_follows_its_pin_through_undo_and_redo_of_a_removal() {
    let mut h = harness();
    let (_, pins) = three_pins(&mut h);
    h.state_mut().remove_pin(0);
    assert_eq!(h.state().doc.project().pins, pins[1..]);
    h.state_mut().selection = Selection::Pin(1); // the pin at x = 350
    // As the Edit menu does it, with no frame between: the pin at x = 150 is back as pin 0.
    h.state_mut().undo();
    assert_eq!(h.state().doc.project().pins, pins);
    assert_eq!(
        h.state().selection,
        Selection::Pin(2),
        "still the pin at x = 350"
    );
    h.state_mut().redo(); // the first pin goes again
    assert_eq!(h.state().doc.project().pins, pins[1..]);
    assert_eq!(h.state().selection, Selection::Pin(1));
    // The same from the keyboard.
    cmd(&mut h, Key::Z);
    assert_eq!(h.state().selection, Selection::Pin(2));
    cmd(&mut h, Key::Y);
    assert_eq!(h.state().selection, Selection::Pin(1));
    // The selected pin itself going leaves nothing selected.
    h.state_mut().selection = Selection::Pin(0);
    h.state_mut().remove_pin(0);
    assert_eq!(h.state().selection, Selection::None, "its own pin went");
}

#[test]
fn a_pin_added_is_the_one_selected() {
    let mut h = harness();
    let (front, pins) = three_pins(&mut h);
    h.state_mut().selection = Selection::Pin(2);
    h.state_mut().add_pin(pin(front, 200.0, 400.0));
    assert_eq!(h.state().selection, Selection::Pin(3));
    h.run();
    assert_eq!(h.state().selection, Selection::Pin(3), "and stays so");
    assert_eq!(h.state().doc.project().pins[..3], pins);
}

#[test]
fn a_pin_lost_with_its_piece_renumbers_the_rest() {
    let mut h = harness();
    let (front, pins) = three_pins(&mut h);
    let other = h.state_mut().doc.edit(|p| {
        p.add_piece(Piece::rectangle(
            PieceId(0),
            "Other",
            Point2::new(600.0, 100.0),
            100.0,
            100.0,
        ))
    });
    let mut all = pins.clone();
    all.insert(1, pin(other, 650.0, 150.0));
    h.state_mut().doc.edit(|p| p.pins = all.clone());
    h.run();
    h.state_mut().selection = Selection::Pin(3); // the front's pin at x = 350
    h.state_mut().doc.edit(|p| p.remove_piece(other));
    h.run();
    assert_eq!(h.state().doc.project().pins, pins);
    assert_eq!(h.state().selection, Selection::Pin(2), "the same pin");
    assert_eq!(h.state().doc.project().pieces[0].id, front);
}

#[test]
fn a_new_project_lets_go_of_every_pin_held_by_number() {
    let mut h = harness();
    three_pins(&mut h);
    h.state_mut().selection = Selection::Pin(1);
    h.state_mut().take_pin_shifts();
    h.state_mut()
        .set_project(opendrape_core::Project::new(), None);
    let shifts = h.state_mut().take_pin_shifts();
    assert_eq!(shifts.len(), 1);
    assert!((0..4).all(|k| shifts[0].index(k).is_none()));
    assert!(h.state_mut().take_pin_shifts().is_empty(), "taken once");
}
