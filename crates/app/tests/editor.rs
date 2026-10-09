//! The pattern window, driven the way a student uses it: clicks, drags and typing.
//! Positions are given in pattern millimetres and turned into screen points with the
//! editor's own view, so the tests don't depend on the window layout.

use egui::{Event, Key, Modifiers, PointerButton, Pos2, accesskit::Role, vec2};
use egui_kittest::{Harness, kittest::Queryable};
use opendrape::editor::{PatternEditor, Selection, Tool};
use opendrape_core::{Edge, Piece, PieceId, Point2, VertexKind};

type H = Harness<'static, PatternEditor>;

fn harness() -> H {
    let mut h = Harness::builder()
        .with_size(vec2(1100.0, 750.0))
        .build_ui_state(|ui, ed: &mut PatternEditor| ed.ui(ui), PatternEditor::new());
    h.run();
    h
}

/// Screen position of the pattern point (x, y) mm.
fn at(h: &H, x: f64, y: f64) -> Pos2 {
    let ed = h.state();
    ed.view.to_screen(ed.canvas_rect, Point2::new(x, y))
}

fn button(h: &H, pos: Pos2, pressed: bool, modifiers: Modifiers) {
    h.event(Event::PointerButton {
        pos,
        button: PointerButton::Primary,
        pressed,
        modifiers,
    });
}

fn click(h: &mut H, x: f64, y: f64) {
    let p = at(h, x, y);
    h.hover_at(p);
    button(h, p, true, Modifiers::NONE);
    button(h, p, false, Modifiers::NONE);
    h.run();
}

fn shift_click(h: &mut H, x: f64, y: f64) {
    let p = at(h, x, y);
    h.hover_at(p);
    h.event(Event::ModifiersChanged(Modifiers::SHIFT));
    button(h, p, true, Modifiers::SHIFT);
    button(h, p, false, Modifiers::SHIFT);
    h.event(Event::ModifiersChanged(Modifiers::NONE));
    h.run();
}

/// Press at `from`, move there in steps, release at `to` (all in mm).
fn drag(h: &mut H, from: (f64, f64), to: (f64, f64)) {
    let (a, b) = (at(h, from.0, from.1), at(h, to.0, to.1));
    h.hover_at(a);
    button(h, a, true, Modifiers::NONE);
    for i in 1..=5 {
        h.hover_at(a + (b - a) * (i as f32 / 5.0));
    }
    button(h, b, false, Modifiers::NONE);
    h.run();
}

fn key(h: &mut H, k: Key) {
    h.key_press(k);
    h.run();
}

fn cmd(h: &mut H, k: Key) {
    h.key_press_modifiers(Modifiers::COMMAND, k);
    h.run();
}

/// Within 0.05 mm (clicks pass through f32 screen coordinates).
fn close(a: Point2, b: Point2) {
    assert!(a.distance(b) < 0.05, "{a:?} vs {b:?}");
}

/// Types `first` over the canvas, which opens the number box.
fn type_number(h: &mut H, first: &str) {
    h.event(Event::Text(first.into()));
    h.run();
}

/// Adds a 300 × 400 mm rectangle with its lower-left corner at (100, 100), as if drawn.
fn with_rectangle(h: &mut H) -> PieceId {
    let id = h.state_mut().doc.edit(|p| {
        p.add_piece(Piece::rectangle(
            PieceId(0),
            "Front",
            Point2::new(100.0, 100.0),
            300.0,
            400.0,
        ))
    });
    h.run();
    id
}

const SQUARE: [(f64, f64); 4] = [
    (100.0, 100.0),
    (400.0, 100.0),
    (400.0, 500.0),
    (100.0, 500.0),
];

#[test]
fn pen_draws_a_closed_piece() {
    let mut h = harness();
    key(&mut h, Key::H);
    assert_eq!(h.state().tool, Tool::Pen);
    for (x, y) in SQUARE {
        click(&mut h, x, y);
    }
    assert_eq!(h.state().pen().len(), 4);
    click(&mut h, 100.0, 100.0); // back on the first point closes the piece
    let ed = h.state();
    assert!(ed.pen().is_empty());
    let [piece] = &ed.doc.project().pieces[..] else {
        panic!("one piece")
    };
    assert_eq!(piece.name, "Piece 1");
    for (v, (x, y)) in piece.vertices.iter().zip(SQUARE) {
        close(v.pos, Point2::new(x, y));
    }
    assert!(piece.edges.iter().all(|e| *e == Edge::Line));
    assert_eq!(ed.selection, Selection::Piece(piece.id));
    cmd(&mut h, Key::Z);
    assert!(
        h.state().doc.project().pieces.is_empty(),
        "the whole piece is one undo step"
    );
}

#[test]
fn enter_finishes_but_needs_three_points() {
    let mut h = harness();
    key(&mut h, Key::H);
    click(&mut h, 100.0, 100.0);
    click(&mut h, 400.0, 100.0);
    key(&mut h, Key::Enter);
    assert_eq!(h.state().pen().len(), 2);
    assert!(h.state().notice.is_some());
    click(&mut h, 250.0, 400.0);
    key(&mut h, Key::Enter);
    assert_eq!(h.state().doc.project().pieces.len(), 1);
}

#[test]
fn dragging_while_placing_makes_a_curve_point() {
    let mut h = harness();
    key(&mut h, Key::H);
    click(&mut h, 100.0, 100.0);
    drag(&mut h, (400.0, 100.0), (450.0, 150.0));
    let pen = h.state().pen().to_vec();
    assert_eq!(pen.len(), 2, "a drag places one point");
    close(
        pen[1].handle.expect("handle pulled out"),
        Point2::new(450.0, 150.0),
    );
    click(&mut h, 400.0, 500.0);
    key(&mut h, Key::Enter);
    let piece = h.state().doc.project().pieces[0].clone();
    assert_eq!(piece.vertices[1].kind, VertexKind::Smooth);
    let Edge::Curve { c2, .. } = piece.edges[0] else {
        panic!("arriving edge curved")
    };
    close(c2, Point2::new(350.0, 50.0)); // mirrored through the point
    let Edge::Curve { c1, .. } = piece.edges[1] else {
        panic!("leaving edge curved")
    };
    close(c1, Point2::new(450.0, 150.0));
    assert_eq!(piece.edges[2], Edge::Line);
}

#[test]
fn typed_length_goes_toward_the_pointer() {
    let mut h = harness();
    key(&mut h, Key::H);
    click(&mut h, 100.0, 100.0);
    let p = at(&h, 300.0, 100.0);
    h.hover_at(p);
    h.run();
    type_number(&mut h, "3");
    assert!(h.state().length_box_open());
    let field = h.get_by_role_and_label(Role::TextInput, "Length");
    assert!(field.is_focused());
    field.type_text("4.5");
    h.run();
    assert_eq!(
        h.get_by_role_and_label(Role::TextInput, "Length")
            .value()
            .as_deref(),
        Some("34.5")
    );
    assert_eq!(
        h.get_by_role_and_label(Role::TextInput, "Angle")
            .value()
            .as_deref(),
        Some("0.0")
    );
    key(&mut h, Key::Enter);
    let ed = h.state();
    assert!(!ed.length_box_open());
    let [a, b] = ed.pen() else {
        panic!("two points")
    };
    assert!(
        (b.pos - a.pos).distance(Point2::new(345.0, 0.0)) < 1e-9,
        "34.5 cm to the right"
    );
    assert!(
        ed.doc.project().pieces.is_empty(),
        "Enter in the box must not also finish the piece"
    );
}

#[test]
fn typed_angle_sets_the_direction() {
    let mut h = harness();
    key(&mut h, Key::H);
    click(&mut h, 100.0, 100.0);
    type_number(&mut h, "20");
    key(&mut h, Key::Tab);
    assert!(
        h.state().length_box_open(),
        "Tab moves to Angle without closing the box"
    );
    assert!(
        h.get_by_role_and_label(Role::TextInput, "Angle")
            .is_focused()
    );
    cmd(&mut h, Key::A);
    h.get_by_role_and_label(Role::TextInput, "Angle")
        .type_text("90");
    h.run();
    key(&mut h, Key::Enter);
    let [a, b] = h.state().pen() else {
        panic!("two points")
    };
    assert!(
        (b.pos - a.pos).distance(Point2::new(0.0, 200.0)) < 1e-9,
        "20 cm straight up"
    );
}

#[test]
fn typed_length_rejects_nonsense() {
    for (first, rest) in [
        ("3", "abc"),
        ("0", ""),
        ("1", "e9"),
        ("5", ",,"),
        ("0", ".001"),
    ] {
        let mut h = harness();
        key(&mut h, Key::H);
        click(&mut h, 100.0, 100.0);
        type_number(&mut h, first);
        if !rest.is_empty() {
            h.get_by_role_and_label(Role::TextInput, "Length")
                .type_text(rest);
            h.run();
        }
        key(&mut h, Key::Enter);
        assert_eq!(h.state().pen().len(), 1, "{first}{rest}");
        assert!(h.state().notice.is_some(), "{first}{rest}");
        assert!(!h.state().length_box_open());
    }
}

/// With the pen on a point, types `length` (in the current units) at `degrees` and presses Enter.
fn type_segment(h: &mut H, length: &str, degrees: &str) {
    type_number(h, length);
    key(h, Key::Tab);
    cmd(h, Key::A);
    h.get_by_role_and_label(Role::TextInput, "Angle")
        .type_text(degrees);
    h.run();
    key(h, Key::Enter);
}

#[test]
fn typing_the_closing_edge_finishes_the_piece() {
    let mut h = harness();
    key(&mut h, Key::H);
    for (x, y) in [
        (100.0, 100.0),
        (500.0, 100.0),
        (500.0, 300.0),
        (100.0, 300.0),
    ] {
        click(&mut h, x, y);
    }
    type_segment(&mut h, "20", "270"); // 20 cm straight down: back onto the first point
    let ed = h.state();
    assert!(!ed.length_box_open());
    assert!(ed.pen().is_empty());
    let [piece] = &ed.doc.project().pieces[..] else {
        panic!("one piece")
    };
    assert_eq!(
        piece.vertices.len(),
        4,
        "no extra point on top of the first"
    );
    assert_eq!(ed.selection, Selection::Piece(piece.id));
}

#[test]
fn typed_point_on_an_earlier_point_is_refused() {
    let mut h = harness();
    key(&mut h, Key::H);
    for (x, y) in [(100.0, 100.0), (300.0, 100.0), (300.0, 300.0)] {
        click(&mut h, x, y);
    }
    type_segment(&mut h, "20", "270"); // lands on (300, 100): neither the first nor the last point
    let ed = h.state();
    assert!(!ed.length_box_open());
    assert_eq!(ed.pen().len(), 3);
    assert!(ed.notice.is_some());
    assert!(ed.doc.project().pieces.is_empty());
}

#[test]
fn escape_closes_the_box_then_cancels_the_piece() {
    let mut h = harness();
    key(&mut h, Key::H);
    click(&mut h, 100.0, 100.0);
    click(&mut h, 400.0, 100.0);
    type_number(&mut h, "12");
    key(&mut h, Key::Escape);
    assert!(!h.state().length_box_open());
    assert_eq!(h.state().pen().len(), 2);
    key(&mut h, Key::Escape);
    assert!(h.state().pen().is_empty());
    assert!(h.state().doc.project().pieces.is_empty());
}

#[test]
fn undo_while_drawing_removes_the_last_point() {
    let mut h = harness();
    key(&mut h, Key::H);
    for (x, y) in &SQUARE[..3] {
        click(&mut h, *x, *y);
    }
    cmd(&mut h, Key::Z);
    assert_eq!(h.state().pen().len(), 2);
    key(&mut h, Key::Backspace);
    assert_eq!(h.state().pen().len(), 1);
    assert!(
        !h.state().doc.can_undo(),
        "the project history is untouched"
    );
}

#[test]
fn clicking_the_same_spot_twice_adds_one_point() {
    let mut h = harness();
    key(&mut h, Key::H);
    click(&mut h, 100.0, 100.0);
    click(&mut h, 100.0, 100.0);
    assert_eq!(h.state().pen().len(), 1);
}

#[test]
fn shift_keeps_45_degree_steps() {
    let mut h = harness();
    key(&mut h, Key::H);
    click(&mut h, 100.0, 100.0);
    shift_click(&mut h, 400.0, 120.0);
    let [a, b] = h.state().pen() else {
        panic!("two points")
    };
    assert!(
        (b.pos.y - a.pos.y).abs() < 1e-9,
        "snapped horizontal: {a:?} {b:?}"
    );
}

#[test]
fn snaps_to_existing_points() {
    let mut h = harness();
    with_rectangle(&mut h);
    key(&mut h, Key::H);
    click(&mut h, 403.0, 98.0);
    assert_eq!(h.state().pen()[0].pos, Point2::new(400.0, 100.0));
}

#[test]
fn pinch_zooms_around_the_pointer() {
    let mut h = harness();
    let p = at(&h, 250.0, 250.0);
    h.hover_at(p);
    h.run();
    let zoom = h.state().view.zoom;
    h.event(Event::Zoom(1.5));
    h.run();
    let ed = h.state();
    assert!((ed.view.zoom - zoom * 1.5).abs() < 1e-6);
    close(
        ed.view.to_world(ed.canvas_rect, p),
        Point2::new(250.0, 250.0),
    );
}

#[test]
fn rectangle_by_dragging() {
    let mut h = harness();
    key(&mut h, Key::S);
    drag(&mut h, (100.0, 100.0), (400.0, 500.0));
    let ed = h.state();
    let [piece] = &ed.doc.project().pieces[..] else {
        panic!("one piece")
    };
    for (v, (x, y)) in piece.vertices.iter().zip(SQUARE) {
        close(v.pos, Point2::new(x, y));
    }
    assert_eq!(ed.selection, Selection::Piece(piece.id));
}

#[test]
fn rectangle_by_typing_its_size() {
    let mut h = harness();
    key(&mut h, Key::S);
    click(&mut h, 100.0, 100.0);
    assert!(h.state().length_box_open());
    let width = h.get_by_role_and_label(Role::TextInput, "Width");
    assert!(width.is_focused());
    width.type_text("35");
    h.run();
    key(&mut h, Key::Tab);
    h.get_by_role_and_label(Role::TextInput, "Height")
        .type_text("60,5");
    h.run();
    key(&mut h, Key::Enter);
    let piece = h.state().doc.project().pieces[0].clone();
    close(piece.vertices[0].pos, Point2::new(100.0, 100.0));
    assert!(
        (piece.vertices[2].pos - piece.vertices[0].pos).distance(Point2::new(350.0, 605.0)) < 1e-9
    );
}

#[test]
fn a_flat_or_zero_size_rectangle_is_refused() {
    let mut h = harness();
    key(&mut h, Key::S);
    drag(&mut h, (100.0, 100.0), (400.0, 100.0));
    assert!(h.state().doc.project().pieces.is_empty());
    click(&mut h, 100.0, 100.0);
    h.get_by_role_and_label(Role::TextInput, "Width")
        .type_text("0");
    h.run();
    key(&mut h, Key::Tab);
    h.get_by_role_and_label(Role::TextInput, "Height")
        .type_text("60");
    h.run();
    key(&mut h, Key::Enter);
    assert!(h.state().doc.project().pieces.is_empty());
    assert!(h.state().notice.is_some());
}

#[test]
fn tool_keys_do_nothing_while_typing_a_number() {
    let mut h = harness();
    key(&mut h, Key::H);
    click(&mut h, 100.0, 100.0);
    type_number(&mut h, "3");
    h.key_press(Key::S);
    h.event(Event::Text("s".into()));
    h.run();
    assert_eq!(h.state().tool, Tool::Pen);
    assert!(h.state().length_box_open());
}

#[test]
fn draws_concave_and_curved_pieces() {
    let mut h = harness();
    let id = h.state_mut().doc.edit(|p| {
        let l = [
            (0.0, 0.0),
            (300.0, 0.0),
            (300.0, 100.0),
            (100.0, 100.0),
            (100.0, 300.0),
            (0.0, 300.0),
        ];
        let mut piece = Piece::polygon(PieceId(0), "L", &l.map(|(x, y)| Point2::new(x, y)));
        piece.set_curved(1, true);
        p.add_piece(piece)
    });
    // Painting a curved, concave piece with handles and a selected edge or point must neither
    // panic nor drop the selection.
    h.state_mut().selection = Selection::Edge(id, 1);
    h.run();
    assert_eq!(h.state().selection, Selection::Edge(id, 1));
    h.state_mut().selection = Selection::Vertex(id, 3);
    h.run();
    assert_eq!(h.state().selection, Selection::Vertex(id, 3));
}

#[test]
fn clicking_selects_points_edges_and_pieces() {
    let mut h = harness();
    let id = with_rectangle(&mut h);
    for ((x, y), expected) in [
        ((400.0, 100.0), Selection::Vertex(id, 1)),
        ((250.0, 100.0), Selection::Edge(id, 0)),
        ((250.0, 300.0), Selection::Piece(id)),
        ((700.0, 550.0), Selection::None),
    ] {
        click(&mut h, x, y);
        assert_eq!(h.state().selection, expected, "click at {x},{y}");
    }
}

#[test]
fn dragging_a_point_is_one_undo_step() {
    let mut h = harness();
    let id = with_rectangle(&mut h);
    drag(&mut h, (100.0, 100.0), (150.0, 120.0));
    let piece = h.state().doc.project().piece(id).unwrap().clone();
    close(piece.vertices[0].pos, Point2::new(150.0, 120.0));
    assert_eq!(piece.vertices[1].pos, Point2::new(400.0, 100.0));
    assert_eq!(h.state().selection, Selection::Vertex(id, 0));
    cmd(&mut h, Key::Z);
    assert_eq!(
        h.state().doc.project().piece(id).unwrap().vertices[0].pos,
        Point2::new(100.0, 100.0)
    );
}

#[test]
fn dragging_an_edge_moves_both_ends() {
    let mut h = harness();
    let id = with_rectangle(&mut h);
    drag(&mut h, (250.0, 100.0), (250.0, 60.0));
    let piece = h.state().doc.project().piece(id).unwrap().clone();
    close(piece.vertices[0].pos, Point2::new(100.0, 60.0));
    close(piece.vertices[1].pos, Point2::new(400.0, 60.0));
    assert_eq!(piece.vertices[2].pos, Point2::new(400.0, 500.0));
}

#[test]
fn dragging_inside_moves_the_whole_piece() {
    let mut h = harness();
    let id = with_rectangle(&mut h);
    drag(&mut h, (250.0, 300.0), (300.0, 350.0));
    let piece = h.state().doc.project().piece(id).unwrap().clone();
    for (v, (x, y)) in piece.vertices.iter().zip(SQUARE) {
        close(v.pos, Point2::new(x + 50.0, y + 50.0));
    }
}

#[test]
fn dragging_a_curve_handle_bends_its_edge() {
    let mut h = harness();
    let id = with_rectangle(&mut h);
    h.state_mut()
        .doc
        .edit(|p| p.piece_mut(id).unwrap().set_curved(0, true));
    click(&mut h, 250.0, 300.0); // select the piece so its handles show
    drag(&mut h, (200.0, 100.0), (200.0, 40.0));
    let Edge::Curve { c1, c2 } = h.state().doc.project().piece(id).unwrap().edges[0] else {
        panic!("curved")
    };
    close(c1, Point2::new(200.0, 40.0));
    close(c2, Point2::new(300.0, 100.0));
    assert_eq!(h.state().selection, Selection::Edge(id, 0));
}

#[test]
fn add_point_splits_the_edge_under_the_pointer() {
    let mut h = harness();
    let id = with_rectangle(&mut h);
    key(&mut h, Key::X);
    click(&mut h, 250.0, 101.0);
    let piece = h.state().doc.project().piece(id).unwrap().clone();
    assert_eq!(piece.len(), 5);
    close(piece.vertices[1].pos, Point2::new(250.0, 100.0));
    assert_eq!(h.state().selection, Selection::Vertex(id, 1));
    click(&mut h, 102.0, 100.0); // right next to a corner: refused
    assert_eq!(h.state().doc.project().piece(id).unwrap().len(), 5);
    assert!(h.state().notice.is_some());
}

#[test]
fn delete_removes_points_then_pieces() {
    let mut h = harness();
    let id = with_rectangle(&mut h);
    click(&mut h, 400.0, 100.0);
    key(&mut h, Key::Delete);
    assert_eq!(h.state().doc.project().piece(id).unwrap().len(), 3);
    assert_eq!(h.state().selection, Selection::Piece(id));
    click(&mut h, 100.0, 100.0);
    key(&mut h, Key::Backspace);
    assert_eq!(
        h.state().doc.project().piece(id).unwrap().len(),
        3,
        "a triangle keeps its points"
    );
    assert!(h.state().notice.is_some());
    click(&mut h, 150.0, 300.0); // inside the triangle
    key(&mut h, Key::Delete);
    assert!(h.state().doc.project().pieces.is_empty());
}

#[test]
fn selection_survives_undo_of_its_piece() {
    let mut h = harness();
    key(&mut h, Key::S);
    drag(&mut h, (100.0, 100.0), (400.0, 500.0));
    key(&mut h, Key::Z);
    click(&mut h, 400.0, 500.0);
    assert!(matches!(h.state().selection, Selection::Vertex(_, 2)));
    cmd(&mut h, Key::Z); // takes the rectangle away again
    assert_eq!(h.state().selection, Selection::None);
    key(&mut h, Key::Delete); // nothing selected: nothing happens, nothing panics
    h.key_press_modifiers(Modifiers::COMMAND | Modifiers::SHIFT, Key::Z);
    h.run();
    assert_eq!(h.state().doc.project().pieces.len(), 1);
}

fn piece_of(h: &H, id: PieceId) -> Piece {
    h.state().doc.project().piece(id).unwrap().clone()
}

fn field_text(h: &H, label: &str) -> String {
    h.get_by_role_and_label(Role::TextInput, label)
        .value()
        .unwrap_or_default()
}

/// Clicks the property field `label`, replaces its text with `text` and presses Enter.
fn type_into(h: &mut H, label: &str, text: &str) {
    h.get_by_role_and_label(Role::TextInput, label).click();
    h.run();
    cmd(h, Key::A);
    h.get_by_role_and_label(Role::TextInput, label)
        .type_text(text);
    h.run();
    key(h, Key::Enter);
}

fn untouched_rectangle(id: PieceId) -> Piece {
    Piece::rectangle(id, "Front", Point2::new(100.0, 100.0), 300.0, 400.0)
}

#[test]
fn panel_sets_an_edge_length() {
    let mut h = harness();
    let id = with_rectangle(&mut h);
    click(&mut h, 250.0, 100.0);
    assert_eq!(h.state().selection, Selection::Edge(id, 0));
    assert_eq!(field_text(&h, "Length"), "30.0");
    type_into(&mut h, "Length", "45");
    let p = piece_of(&h, id);
    assert!((opendrape_geom::edge_length(&p, 0) - 450.0).abs() < 1e-9);
    assert_eq!(
        p.vertices[0].pos,
        Point2::new(100.0, 100.0),
        "the start point stays"
    );
    assert_eq!(field_text(&h, "Length"), "45.0");
    cmd(&mut h, Key::Z);
    assert_eq!(piece_of(&h, id), untouched_rectangle(id), "one undo step");
}

#[test]
fn panel_refuses_nonsense_lengths() {
    for text in ["abc", "-5", "0", "99999"] {
        let mut h = harness();
        let id = with_rectangle(&mut h);
        click(&mut h, 250.0, 100.0);
        type_into(&mut h, "Length", text);
        assert_eq!(piece_of(&h, id), untouched_rectangle(id), "{text}");
        assert!(h.state().notice.is_some(), "{text}");
    }
}

#[test]
fn keeping_the_end_point_fixed() {
    let mut h = harness();
    let id = with_rectangle(&mut h);
    click(&mut h, 250.0, 100.0);
    h.get_by_label("End point").click();
    h.run();
    type_into(&mut h, "Length", "45");
    let p = piece_of(&h, id);
    assert_eq!(p.vertices[1].pos, Point2::new(400.0, 100.0));
    assert_eq!(p.vertices[0].pos, Point2::new(-50.0, 100.0));
}

#[test]
fn curved_checkbox_bends_the_edge() {
    let mut h = harness();
    let id = with_rectangle(&mut h);
    click(&mut h, 250.0, 100.0);
    h.get_by_label("Curved").click();
    h.run();
    assert!(matches!(piece_of(&h, id).edges[0], Edge::Curve { .. }));
    cmd(&mut h, Key::Z);
    assert_eq!(piece_of(&h, id).edges[0], Edge::Line);
}

#[test]
fn renaming_a_piece_and_setting_its_grain() {
    let mut h = harness();
    let id = with_rectangle(&mut h);
    click(&mut h, 250.0, 300.0);
    assert_eq!(field_text(&h, "Name"), "Front");
    h.get_by_label("1200.0 cm²"); // 30 × 40 cm
    h.get_by_label("140.0 cm");
    type_into(&mut h, "Name", "Front skirt");
    type_into(&mut h, "Grain angle", "45");
    let p = piece_of(&h, id);
    assert_eq!((p.name.as_str(), p.grain_deg), ("Front skirt", 45.0));
}

#[test]
fn moving_a_point_by_typing_its_position() {
    let mut h = harness();
    let id = with_rectangle(&mut h);
    click(&mut h, 100.0, 100.0);
    assert_eq!(field_text(&h, "X"), "10.0");
    type_into(&mut h, "X", "12");
    type_into(&mut h, "Y", "-3,5");
    assert_eq!(piece_of(&h, id).vertices[0].pos, Point2::new(120.0, -35.0));
    assert_eq!(h.state().selection, Selection::Vertex(id, 0));
}

#[test]
fn inches_change_what_is_shown_not_the_pattern() {
    let mut h = harness();
    let id = with_rectangle(&mut h);
    click(&mut h, 250.0, 100.0);
    h.get_by_label("inch").click();
    h.run();
    assert_eq!(field_text(&h, "Length"), "11.81");
    assert_eq!(piece_of(&h, id), untouched_rectangle(id));
    type_into(&mut h, "Length", "12");
    assert!((opendrape_geom::edge_length(&piece_of(&h, id), 0) - 304.8).abs() < 1e-9);
}

#[test]
fn fit_shows_every_piece() {
    let mut h = harness();
    with_rectangle(&mut h);
    h.state_mut().doc.edit(|p| {
        p.add_piece(Piece::rectangle(
            PieceId(0),
            "Far",
            Point2::new(5000.0, 3000.0),
            200.0,
            200.0,
        ))
    });
    h.run();
    key(&mut h, Key::F);
    let ed = h.state();
    for p in [Point2::new(100.0, 100.0), Point2::new(5200.0, 3200.0)] {
        assert!(
            ed.canvas_rect
                .contains(ed.view.to_screen(ed.canvas_rect, p)),
            "{p:?}"
        );
    }
}

#[test]
fn clicking_away_applies_the_typed_length_to_the_right_edge() {
    let mut h = harness();
    let id = with_rectangle(&mut h);
    click(&mut h, 250.0, 100.0);
    h.get_by_role_and_label(Role::TextInput, "Length").click();
    h.run();
    cmd(&mut h, Key::A);
    h.get_by_role_and_label(Role::TextInput, "Length")
        .type_text("45");
    h.run();
    click(&mut h, 100.0, 300.0); // the left edge
    let p = piece_of(&h, id);
    assert!(
        (opendrape_geom::edge_length(&p, 0) - 450.0).abs() < 1e-9,
        "typed into the bottom edge"
    );
    assert!(
        (opendrape_geom::edge_length(&p, 3) - 400.0).abs() < 1e-9,
        "left edge untouched"
    );
    assert_eq!(h.state().selection, Selection::Edge(id, 3));
}

#[test]
fn status_bar_explains_the_current_tool() {
    let mut h = harness();
    h.get_by_label_contains("Click to select");
    key(&mut h, Key::H);
    h.get_by_label_contains("press and drag to make a curve point");
    click(&mut h, 100.0, 100.0);
    h.get_by_label_contains("Type a number for an exact length");
}
