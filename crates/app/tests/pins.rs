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
