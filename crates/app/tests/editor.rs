//! The pattern window, driven the way a student uses it: clicks, drags and typing.
//! Positions are given in pattern millimetres and turned into screen points with the
//! editor's own view, so the tests don't depend on the window layout.

use egui::{Event, Key, Modifiers, Pos2, accesskit::Role, vec2};
use egui_kittest::kittest::Queryable;
use opendrape::editor::{Selection, Tool};
use opendrape_core::{Edge, Piece, PieceId, Point2, VertexKind};

mod common;
use common::*;

/// A click as a real mouse makes it: the press and the release arrive in different frames.
fn slow_click(h: &mut H, pos: Pos2) {
    h.hover_at(pos);
    button(h, pos, true, Modifiers::NONE);
    h.step();
    button(h, pos, false, Modifiers::NONE);
    h.run();
}

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

#[test]
fn clicking_from_one_field_to_another_applies_the_first() {
    let mut h = harness();
    let id = with_rectangle(&mut h);
    click(&mut h, 100.0, 100.0);
    h.get_by_role_and_label(Role::TextInput, "X").click();
    h.run();
    h.get_by_role_and_label(Role::TextInput, "X")
        .type_text("12");
    h.run();
    h.get_by_role_and_label(Role::TextInput, "Y").click();
    h.run();
    h.run();
    assert_eq!(
        piece_of(&h, id).vertices[0].pos,
        Point2::new(120.0, 100.0),
        "the text typed into X was applied"
    );
    assert_eq!(field_text(&h, "X"), "12.0");
}

#[test]
fn a_refused_value_keeps_its_notice_after_clicking_away() {
    let mut h = harness();
    let id = with_rectangle(&mut h);
    click(&mut h, 250.0, 100.0);
    h.get_by_role_and_label(Role::TextInput, "Length").click();
    h.run();
    h.get_by_role_and_label(Role::TextInput, "Length")
        .type_text("abc");
    h.run();
    click(&mut h, 700.0, 550.0); // empty canvas
    assert!(
        h.state().canvas_rect.contains(at(&h, 700.0, 550.0)),
        "the click landed on the canvas"
    );
    assert_eq!(piece_of(&h, id), untouched_rectangle(id));
    assert!(h.state().notice.is_some());
}

#[test]
fn a_blank_name_is_refused() {
    let mut h = harness();
    let id = with_rectangle(&mut h);
    click(&mut h, 250.0, 300.0);
    type_into(&mut h, "Name", "   ");
    assert_eq!(piece_of(&h, id).name, "Front");
    assert!(h.state().notice.is_some());
}

#[test]
fn escape_in_a_field_keeps_the_selection() {
    let mut h = harness();
    let id = with_rectangle(&mut h);
    click(&mut h, 250.0, 100.0);
    h.get_by_role_and_label(Role::TextInput, "Length").click();
    h.run();
    h.get_by_role_and_label(Role::TextInput, "Length")
        .type_text("45");
    h.run();
    key(&mut h, Key::Escape);
    h.run();
    assert_eq!(h.state().selection, Selection::Edge(id, 0));
    assert_eq!(piece_of(&h, id), untouched_rectangle(id));
    assert_eq!(field_text(&h, "Length"), "30.0");
}

/// A piece with the most points a piece may have, as a ring round (400, 300) mm.
fn full_ring(h: &mut H) -> PieceId {
    let n = opendrape_core::MAX_VERTICES_PER_PIECE;
    let corners: Vec<Point2> = (0..n)
        .map(|k| {
            let a = k as f64 / n as f64 * std::f64::consts::TAU;
            Point2::new(400.0 + 250.0 * a.cos(), 300.0 + 250.0 * a.sin())
        })
        .collect();
    let id = h
        .state_mut()
        .doc
        .edit(|p| p.add_piece(Piece::polygon(PieceId(0), "Ring", &corners)));
    h.run();
    id
}

/// The most pieces a project may hold, all far off to the right of where the tests click.
fn fill_with_pieces(h: &mut H) {
    h.state_mut().doc.edit(|p| {
        for i in 0..opendrape_core::MAX_PIECES {
            p.add_piece(Piece::rectangle(
                PieceId(0),
                format!("P{i}"),
                Point2::new(5000.0 + 30.0 * i as f64, 5000.0),
                20.0,
                20.0,
            ));
        }
    });
    h.run();
    assert!(!h.state().doc.last_change_refused());
}

fn refused_notice(h: &H) -> bool {
    h.state()
        .notice
        .as_deref()
        .is_some_and(|n| n.contains("can't be made"))
}

#[test]
fn adding_a_point_to_a_full_piece_is_refused_with_a_notice() {
    let mut h = harness();
    let id = full_ring(&mut h);
    let before = piece_of(&h, id);
    assert_eq!(before.len(), opendrape_core::MAX_VERTICES_PER_PIECE);
    key(&mut h, Key::X);
    let mid = before.vertices[0].pos.lerp(before.vertices[1].pos, 0.5);
    click(&mut h, mid.x, mid.y);
    assert_eq!(piece_of(&h, id), before, "the piece is unchanged");
    assert!(refused_notice(&h), "{:?}", h.state().notice);
    assert_eq!(h.state().selection, Selection::None);
}

#[test]
fn a_refused_piece_keeps_the_pen_draft() {
    let mut h = harness();
    fill_with_pieces(&mut h);
    key(&mut h, Key::H);
    for (x, y) in &SQUARE[..3] {
        click(&mut h, *x, *y);
    }
    key(&mut h, Key::Enter);
    assert_eq!(h.state().doc.project().pieces.len(), 500, "no 501st piece");
    assert_eq!(h.state().pen().len(), 3, "the student's points are kept");
    assert!(refused_notice(&h), "{:?}", h.state().notice);
}

#[test]
fn a_refused_typed_closing_edge_says_so_and_keeps_the_pen_draft() {
    let mut h = harness();
    fill_with_pieces(&mut h);
    key(&mut h, Key::H);
    for (x, y) in [
        (100.0, 100.0),
        (500.0, 100.0),
        (500.0, 300.0),
        (100.0, 300.0),
    ] {
        click(&mut h, x, y);
    }
    type_segment(&mut h, "20", "270"); // back onto the first point: would close the piece
    assert_eq!(h.state().pen().len(), 4);
    assert!(
        refused_notice(&h),
        "not 'too close': {:?}",
        h.state().notice
    );
}

#[test]
fn a_refused_rectangle_says_so() {
    let mut h = harness();
    fill_with_pieces(&mut h);
    key(&mut h, Key::S);
    drag(&mut h, (100.0, 100.0), (400.0, 500.0));
    assert_eq!(h.state().doc.project().pieces.len(), 500);
    assert!(refused_notice(&h), "{:?}", h.state().notice);
    assert_eq!(h.state().selection, Selection::None);
}

#[test]
fn a_refused_name_says_so() {
    let mut h = harness();
    let id = with_rectangle(&mut h);
    click(&mut h, 250.0, 300.0);
    type_into(
        &mut h,
        "Name",
        &"a".repeat(opendrape_core::MAX_NAME_CHARS + 1),
    );
    assert_eq!(piece_of(&h, id).name, "Front");
    assert!(refused_notice(&h), "{:?}", h.state().notice);
}

#[test]
fn typing_into_a_filled_field_replaces_its_number() {
    // What the tester checklist says: click Length (it shows 30.0), type 45, press Return.
    let mut h = harness();
    let id = with_rectangle(&mut h);
    click(&mut h, 250.0, 100.0);
    assert_eq!(field_text(&h, "Length"), "30.0");
    h.get_by_role_and_label(Role::TextInput, "Length").click();
    h.run();
    h.get_by_role_and_label(Role::TextInput, "Length")
        .type_text("45");
    h.run();
    assert_eq!(field_text(&h, "Length"), "45", "not 30.045");
    key(&mut h, Key::Enter);
    let p = piece_of(&h, id);
    assert!((opendrape_geom::edge_length(&p, 0) - 450.0).abs() < 1e-9);
}

#[test]
fn tabbing_into_a_filled_field_replaces_its_number_too() {
    let mut h = harness();
    with_rectangle(&mut h);
    click(&mut h, 100.0, 100.0);
    h.get_by_role_and_label(Role::TextInput, "X").click();
    h.run();
    key(&mut h, Key::Tab);
    assert!(h.get_by_role_and_label(Role::TextInput, "Y").is_focused());
    h.get_by_role_and_label(Role::TextInput, "Y").type_text("7");
    h.run();
    assert_eq!(field_text(&h, "Y"), "7", "not 10.07");
}

#[test]
fn a_second_click_in_a_field_places_the_cursor_instead_of_selecting_all() {
    let mut h = harness();
    with_rectangle(&mut h);
    click(&mut h, 250.0, 100.0);
    h.get_by_role_and_label(Role::TextInput, "Length").click();
    h.run();
    // Already focused: clicking again must not select everything again. Clicking the field's
    // far right edge puts the cursor at the end, so typing appends.
    let field = h.get_by_role_and_label(Role::TextInput, "Length");
    let right = field.rect().right_center() - vec2(3.0, 0.0);
    h.hover_at(right);
    button(&h, right, true, Modifiers::NONE);
    button(&h, right, false, Modifiers::NONE);
    h.run();
    h.get_by_role_and_label(Role::TextInput, "Length")
        .type_text("5");
    h.run();
    assert_eq!(field_text(&h, "Length"), "30.05");
}

#[test]
fn a_real_two_frame_click_into_a_filled_field_also_selects_it() {
    let mut h = harness();
    with_rectangle(&mut h);
    click(&mut h, 250.0, 100.0);
    let centre = h
        .get_by_role_and_label(Role::TextInput, "Length")
        .rect()
        .center();
    slow_click(&mut h, centre);
    assert!(
        h.get_by_role_and_label(Role::TextInput, "Length")
            .is_focused()
    );
    h.get_by_role_and_label(Role::TextInput, "Length")
        .type_text("45");
    h.run();
    assert_eq!(field_text(&h, "Length"), "45");
}

#[test]
fn switching_units_does_not_change_how_a_pending_typed_length_is_read() {
    let mut h = harness();
    let id = with_rectangle(&mut h);
    click(&mut h, 250.0, 100.0);
    h.get_by_role_and_label(Role::TextInput, "Length").click();
    h.run();
    h.get_by_role_and_label(Role::TextInput, "Length")
        .type_text("45"); // 45 cm, typed but not yet entered
    h.run();
    // A real mouse click on "inch": the press and the release are separate frames.
    let inch = h.get_by_label("inch").rect().center();
    slow_click(&mut h, inch);
    h.run();
    let p = piece_of(&h, id);
    assert!(
        (opendrape_geom::edge_length(&p, 0) - 450.0).abs() < 1e-9,
        "45 was typed in cm: {} mm",
        opendrape_geom::edge_length(&p, 0)
    );
    assert_eq!(h.state().doc.project().units, opendrape_core::Units::Inch);
}

#[test]
fn text_typed_in_a_field_survives_the_window_losing_focus() {
    let mut h = harness();
    let id = with_rectangle(&mut h);
    click(&mut h, 250.0, 100.0);
    h.get_by_role_and_label(Role::TextInput, "Length").click();
    h.run();
    h.get_by_role_and_label(Role::TextInput, "Length")
        .type_text("45");
    h.run();
    // The student switches to another app for a moment, then comes back.
    h.input_mut().focused = false;
    h.run_steps(3);
    h.input_mut().focused = true;
    h.run_steps(3);
    assert_eq!(
        field_text(&h, "Length"),
        "45",
        "their typing is still there"
    );
    key(&mut h, Key::Enter);
    let p = piece_of(&h, id);
    assert!((opendrape_geom::edge_length(&p, 0) - 450.0).abs() < 1e-9);
}
