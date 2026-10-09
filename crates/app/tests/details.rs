//! M2b on the pattern table: cut-on-fold pieces, mirrored pairs, seam allowance, notches and
//! internal lines, driven the way a student would.

mod common;
use common::*;
use egui::{Key, Modifiers, accesskit::Role, vec2};
use egui_kittest::kittest::Queryable;
use opendrape::editor::Selection;
use opendrape_core::{Piece, PieceId, Point2, Units};
use opendrape_geom as geom;

/// A 150 × 300 mm half piece at (300,100), folded on its left edge (x = 300): its pale half
/// covers x = 150..300.
fn with_half(h: &mut H) -> PieceId {
    let id = h.state_mut().doc.edit(|p| {
        let mut half =
            Piece::rectangle(PieceId(0), "Front", Point2::new(300.0, 100.0), 150.0, 300.0);
        half.fold = Some(3);
        p.add_piece(half)
    });
    h.run();
    id
}

/// The `with_rectangle` piece (100,100)–(400,500), paired with a twin at x = 450..750.
fn with_pair(h: &mut H) -> (PieceId, PieceId) {
    let id = with_rectangle(h);
    let twin = h
        .state_mut()
        .doc
        .edit(|p| p.add_twin(id, "Front (mirror)".into(), Point2::new(850.0, 0.0)))
        .unwrap();
    h.run();
    (id, twin)
}

fn twin_vertex(h: &H, id: PieceId, k: usize) -> Point2 {
    h.state()
        .doc
        .project()
        .piece(id)
        .unwrap()
        .twin_shape()
        .unwrap()
        .vertices[k]
        .pos
}

#[test]
fn a_folded_piece_is_drawn_whole_and_its_pale_half_selects_the_piece() {
    let mut h = harness();
    let id = with_half(&mut h);
    click(&mut h, 225.0, 250.0); // the pale half
    assert_eq!(h.state().selection, Selection::Piece(id));
    drag(&mut h, (450.0, 100.0), (470.0, 100.0)); // the drawn half's outer corner
    let full = geom::unfolded(&piece_of(&h, id));
    close(full.vertices[5].pos, Point2::new(130.0, 100.0)); // its mirror moved too
    assert_eq!(h.state().selection, Selection::Vertex(id, 1));
}

#[test]
fn dragging_the_pale_half_moves_the_whole_piece() {
    let mut h = harness();
    let id = with_half(&mut h);
    drag(&mut h, (225.0, 250.0), (235.0, 250.0));
    close(piece_of(&h, id).vertices[0].pos, Point2::new(310.0, 100.0));
}

#[test]
fn a_twin_moves_on_its_own_and_its_edits_reach_the_piece() {
    let mut h = harness();
    let (id, twin) = with_pair(&mut h);
    close(twin_vertex(&h, id, 1), Point2::new(450.0, 100.0)); // (400,100) mirrored: 850 - 400
    drag(&mut h, (450.0, 100.0), (470.0, 120.0)); // a twin corner
    close(piece_of(&h, id).vertices[1].pos, Point2::new(380.0, 120.0)); // mirrored back
    assert_eq!(h.state().selection, Selection::Vertex(twin, 1));
    let before = piece_of(&h, id);
    drag(&mut h, (600.0, 300.0), (620.0, 310.0)); // inside the twin
    assert_eq!(
        piece_of(&h, id).vertices,
        before.vertices,
        "the piece stays"
    );
    close(twin_vertex(&h, id, 0), Point2::new(770.0, 110.0)); // offset now (870, 10)
    let twin_before = twin_vertex(&h, id, 0);
    drag(&mut h, (200.0, 300.0), (210.0, 300.0)); // inside the piece
    close(twin_vertex(&h, id, 0), twin_before);
    cmd(&mut h, Key::Z);
    cmd(&mut h, Key::Z);
    cmd(&mut h, Key::Z);
    close(piece_of(&h, id).vertices[1].pos, Point2::new(400.0, 100.0));
}

#[test]
fn clicking_and_deleting_a_twin() {
    let mut h = harness();
    let (id, twin) = with_pair(&mut h);
    click(&mut h, 600.0, 300.0);
    assert_eq!(h.state().selection, Selection::Piece(twin));
    key(&mut h, Key::Delete);
    assert!(
        piece_of(&h, id).twin.is_none(),
        "the twin goes, the piece stays"
    );
    assert_eq!(h.state().selection, Selection::None);
}

#[test]
fn undoing_a_pair_drops_a_twin_selection() {
    let mut h = harness();
    let (_, twin) = with_pair(&mut h);
    click(&mut h, 600.0, 300.0);
    assert_eq!(h.state().selection, Selection::Piece(twin));
    cmd(&mut h, Key::Z); // takes the twin away
    assert_eq!(h.state().selection, Selection::None);
}

#[test]
fn add_point_works_on_a_twin_edge() {
    let mut h = harness();
    let (id, _) = with_pair(&mut h);
    key(&mut h, Key::X);
    click(&mut h, 600.0, 101.0); // the twin's bottom edge, halfway
    assert_eq!(piece_of(&h, id).len(), 5);
    close(piece_of(&h, id).vertices[1].pos, Point2::new(250.0, 100.0));

    // Off centre, so that a mirrored curve parameter (1 - t) would land somewhere else: the
    // twin's edge runs from (750,100) to (450,100), so a quarter of the way along is x = 675,
    // and the stored edge from (100,100) to (400,100) is split a quarter along, at x = 175.
    let mut h = harness();
    let (id, _) = with_pair(&mut h);
    key(&mut h, Key::X);
    click(&mut h, 675.0, 101.0);
    assert_eq!(piece_of(&h, id).len(), 5);
    close(piece_of(&h, id).vertices[1].pos, Point2::new(175.0, 100.0));
}

#[test]
fn the_pale_half_of_a_fold_picks_and_moves_the_whole_piece() {
    // A pale corner, then a pale edge: neither is editable, so both pick the piece.
    for (x, y) in [(150.0, 100.0), (150.0, 250.0)] {
        let mut h = harness();
        let id = with_half(&mut h);
        click(&mut h, x, y);
        assert_eq!(
            h.state().selection,
            Selection::Piece(id),
            "click at {x},{y}"
        );
        let before = piece_of(&h, id);
        drag(&mut h, (x, y), (x + 10.0, y));
        let after = piece_of(&h, id);
        assert_eq!(after.vertices.len(), before.vertices.len());
        for (a, b) in after.vertices.iter().zip(&before.vertices) {
            close(a.pos, b.pos + Point2::new(10.0, 0.0));
        }
        assert_eq!(h.state().selection, Selection::Piece(id), "drag at {x},{y}");
    }
}

#[test]
fn a_twin_placed_higher_still_edits_the_piece_through_the_mirror() {
    let mut h = harness();
    let id = with_rectangle(&mut h);
    let twin = h
        .state_mut()
        .doc
        .edit(|p| p.add_twin(id, "Front (mirror)".into(), Point2::new(850.0, 40.0)))
        .unwrap();
    h.run();
    close(twin_vertex(&h, id, 1), Point2::new(450.0, 140.0)); // (400,100) mirrored and raised
    drag(&mut h, (450.0, 140.0), (470.0, 165.0)); // right 20, up 25
    close(piece_of(&h, id).vertices[1].pos, Point2::new(380.0, 125.0)); // left 20, up 25
    assert_eq!(h.state().selection, Selection::Vertex(twin, 1));
}

#[test]
fn a_refused_drag_says_so_once() {
    let mut h = harness();
    let id = with_half(&mut h);
    let before = piece_of(&h, id);
    // Pulling the drawn half's outer corner over the fold line would put the piece on both
    // sides of it, which a cut-on-fold piece can't be.
    let (from, over) = (at(&h, 450.0, 100.0), at(&h, 250.0, 100.0));
    h.hover_at(from);
    button(&h, from, true, Modifiers::NONE);
    h.step();
    h.hover_at(over);
    h.step();
    assert!(
        h.state().doc.last_change_refused(),
        "the drag crossed the fold"
    );
    assert!(refused_notice(&h), "the refusal is reported");
    assert_eq!(piece_of(&h, id), before, "and the piece stays as it was");
    h.state_mut().notice = None;
    h.hover_at(over + vec2(1.0, 0.0));
    h.step();
    assert!(!refused_notice(&h), "the same drag doesn't report it again");
    button(&h, over, false, Modifiers::NONE);
    h.run();
    assert_eq!(piece_of(&h, id), before);
}

#[test]
fn seam_allowance_can_be_hidden() {
    let mut h = harness();
    with_rectangle(&mut h);
    assert!(h.state().show_allowance);
    h.get_by_label("Show seam allowance").click();
    h.run();
    assert!(!h.state().show_allowance);
}

#[test]
fn everything_draws_without_trouble() {
    let mut h = harness();
    let (id, _) = with_pair(&mut h);
    let half = with_half(&mut h);
    h.state_mut().doc.edit(|p| {
        let piece = p.piece_mut(id).unwrap();
        piece.notches = vec![opendrape_core::Notch::new(0, 50.0)];
        piece.lines = vec![opendrape_core::InternalLine::open(&[
            Point2::new(150.0, 200.0),
            Point2::new(300.0, 200.0),
        ])];
        piece.edge_props[0].hem = true;
    });
    for sel in [
        Selection::Piece(id),
        Selection::Edge(half, 1),
        Selection::Vertex(half, 2),
    ] {
        h.state_mut().selection = sel;
        h.run();
        assert_eq!(h.state().selection, sel);
    }
}

#[test]
fn allowance_for_the_whole_piece_and_for_one_edge() {
    let mut h = harness();
    let id = with_rectangle(&mut h);
    click(&mut h, 250.0, 300.0);
    assert_eq!(field_text(&h, "Seam allowance"), "1.0");
    type_into(&mut h, "Seam allowance", "1,5");
    assert_eq!(piece_of(&h, id).allowance, 15.0);
    click(&mut h, 250.0, 100.0); // bottom edge
    assert_eq!(field_text(&h, "Seam allowance"), "1.5");
    type_into(&mut h, "Seam allowance", "2");
    assert_eq!(piece_of(&h, id).edge_props[0].allowance, Some(20.0));
    h.get_by_label("Same as piece").click();
    h.run();
    assert_eq!(piece_of(&h, id).edge_props[0].allowance, None);
}

#[test]
fn an_allowance_out_of_range_is_refused() {
    let mut h = harness();
    let id = with_rectangle(&mut h);
    click(&mut h, 250.0, 300.0);
    type_into(&mut h, "Seam allowance", "12"); // 12 cm
    assert_eq!(piece_of(&h, id).allowance, 10.0);
    assert!(
        h.state()
            .notice
            .as_deref()
            .is_some_and(|n| n.contains("between 0 and 10 cm"))
    );
}

#[test]
fn the_hem_checkbox_gives_three_centimetres() {
    let mut h = harness();
    let id = with_rectangle(&mut h);
    click(&mut h, 250.0, 100.0);
    h.get_by_label("Hem").click();
    h.run();
    assert!(piece_of(&h, id).edge_props[0].hem);
    assert_eq!(field_text(&h, "Seam allowance"), "3.0");
}

#[test]
fn set_a_fold_then_unfold_or_remove_it() {
    let mut h = harness();
    let id = with_rectangle(&mut h); // (100,100)-(400,500)
    click(&mut h, 100.0, 300.0); // left edge
    assert_eq!(h.state().selection, Selection::Edge(id, 3));
    h.get_by_label("Set as fold line").click();
    h.run();
    assert_eq!(piece_of(&h, id).fold, Some(3));
    h.get_by_label("This edge is the fold line.");
    click(&mut h, 250.0, 300.0);
    h.get_by_label("2400.0 cm²"); // the full 60 × 40 cm piece
    h.get_by_label("Unfold").click();
    h.run();
    assert_eq!((piece_of(&h, id).len(), piece_of(&h, id).fold), (6, None));
    cmd(&mut h, Key::Z);
    assert_eq!(
        (piece_of(&h, id).len(), piece_of(&h, id).fold),
        (4, Some(3))
    );
    h.get_by_label("Remove fold").click();
    h.run();
    assert_eq!((piece_of(&h, id).len(), piece_of(&h, id).fold), (4, None));
}

#[test]
fn a_fold_that_would_cross_the_piece_is_refused() {
    let mut h = harness();
    // A U shape: the inner edge x = 300 has parts of the piece on both sides of its line.
    let id = h.state_mut().doc.edit(|p| {
        let pts = [
            (100.0, 100.0),
            (400.0, 100.0),
            (400.0, 400.0),
            (300.0, 400.0),
            (300.0, 200.0),
            (200.0, 200.0),
            (200.0, 400.0),
            (100.0, 400.0),
        ];
        p.add_piece(Piece::polygon(
            PieceId(0),
            "U",
            &pts.map(|(x, y)| Point2::new(x, y)),
        ))
    });
    h.run();
    click(&mut h, 300.0, 300.0);
    assert_eq!(h.state().selection, Selection::Edge(id, 3));
    h.get_by_label("Set as fold line").click();
    h.run();
    assert_eq!(piece_of(&h, id).fold, None);
    assert!(
        h.state()
            .notice
            .as_deref()
            .is_some_and(|n| n.contains("one side"))
    );
}

#[test]
fn make_rename_and_break_a_pair() {
    let mut h = harness();
    let id = with_rectangle(&mut h);
    click(&mut h, 250.0, 300.0);
    h.get_by_label("Make mirrored pair").click();
    h.run();
    let twin = piece_of(&h, id).twin.unwrap().id;
    assert_eq!(h.state().selection, Selection::Piece(twin));
    assert_eq!(
        piece_of(&h, id).twin.unwrap().offset,
        Point2::new(850.0, 0.0)
    );
    assert_eq!(field_text(&h, "Name"), "Front (mirror)");
    h.get_by_label("Mirror image of Front");
    type_into(&mut h, "Name", "Back right");
    assert_eq!(h.state().doc.project().name_of(twin), Some("Back right"));
    assert_eq!(piece_of(&h, id).name, "Front");
    h.get_by_label("Break pair").click();
    h.run();
    assert!(piece_of(&h, id).twin.is_none());
    assert_eq!(piece_of(&h, twin).name, "Back right");
    assert_eq!(h.state().selection, Selection::Piece(twin));
}

#[test]
fn undo_of_make_pair_drops_the_twin_selection() {
    let mut h = harness();
    with_rectangle(&mut h);
    click(&mut h, 250.0, 300.0);
    h.get_by_label("Make mirrored pair").click();
    h.run();
    cmd(&mut h, Key::Z);
    assert_eq!(h.state().selection, Selection::None);
}

#[test]
fn a_twin_point_is_shown_where_the_twin_is() {
    let mut h = harness();
    let id = with_rectangle(&mut h);
    click(&mut h, 250.0, 300.0);
    h.get_by_label("Make mirrored pair").click();
    h.run();
    let twin = piece_of(&h, id).twin.unwrap().id;
    click(&mut h, 450.0, 100.0); // the twin's image of corner 1
    assert_eq!(h.state().selection, Selection::Vertex(twin, 1));
    assert_eq!(field_text(&h, "X"), "45.0");
    type_into(&mut h, "X", "47");
    assert_eq!(piece_of(&h, id).vertices[1].pos, Point2::new(380.0, 100.0)); // 850 - 470
}

#[test]
fn a_twin_shows_and_sets_the_mirrored_grain_angle() {
    let mut h = harness();
    let (id, twin) = with_pair(&mut h);
    click(&mut h, 250.0, 300.0);
    type_into(&mut h, "Grain angle", "30");
    assert_eq!(piece_of(&h, id).grain_deg, 30.0);
    click(&mut h, 600.0, 300.0);
    assert_eq!(h.state().selection, Selection::Piece(twin));
    assert_eq!(field_text(&h, "Grain angle"), "150.0"); // 180 - 30
    type_into(&mut h, "Grain angle", "100");
    assert_eq!(piece_of(&h, id).grain_deg, 80.0); // 180 - 100
}

#[test]
fn a_twin_edge_has_the_pieces_sewing_controls_but_cannot_be_folded() {
    let mut h = harness();
    let (id, twin) = with_pair(&mut h);
    click(&mut h, 600.0, 100.0); // the twin's bottom edge
    assert_eq!(h.state().selection, Selection::Edge(twin, 0));
    assert_eq!(field_text(&h, "Length"), "30.0");
    assert_eq!(field_text(&h, "Seam allowance"), "1.0");
    assert!(h.query_by_label("Set as fold line").is_none());
    type_into(&mut h, "Seam allowance", "2");
    assert_eq!(piece_of(&h, id).edge_props[0].allowance, Some(20.0));
    h.get_by_label("Hem").click();
    h.run();
    assert!(piece_of(&h, id).edge_props[0].hem);
    h.get_by_label("Same as piece").click();
    h.run();
    assert_eq!(piece_of(&h, id).edge_props[0].allowance, None);
    assert_eq!(field_text(&h, "Seam allowance"), "3.0"); // the hem's
}

#[test]
fn the_fold_edge_offers_only_removing_the_fold() {
    for fold in 0..4 {
        let mut h = harness();
        let id = with_rectangle(&mut h);
        h.state_mut()
            .doc
            .edit(|p| p.piece_mut(id).unwrap().fold = Some(fold));
        h.state_mut().selection = Selection::Edge(id, fold);
        h.run();
        assert_eq!(h.state().selection, Selection::Edge(id, fold));
        h.get_by_label("This edge is the fold line.");
        assert!(h.query_by_label("Hem").is_none(), "fold {fold}");
        assert!(
            h.query_by_label("Set as fold line").is_none(),
            "fold {fold}"
        );
        assert!(
            h.query_by_role_and_label(Role::TextInput, "Seam allowance")
                .is_none(),
            "fold {fold}"
        );
        field_text(&h, "Length");
        h.get_by_label("Remove fold").click();
        h.run();
        assert_eq!(piece_of(&h, id).fold, None, "fold {fold}");
    }
}

#[test]
fn breaking_a_pair_keeps_the_selected_piece_selected() {
    let mut h = harness();
    let (id, twin) = with_pair(&mut h);
    click(&mut h, 250.0, 300.0);
    assert_eq!(h.state().selection, Selection::Piece(id));
    h.get_by_label("Break pair").click();
    h.run();
    assert!(piece_of(&h, id).twin.is_none());
    assert!(h.state().doc.project().piece(twin).is_some());
    assert_eq!(h.state().selection, Selection::Piece(id));
}

#[test]
fn a_twin_point_placed_higher_is_shown_and_typed_with_its_height() {
    let mut h = harness();
    let id = with_rectangle(&mut h);
    let twin = h
        .state_mut()
        .doc
        .edit(|p| p.add_twin(id, "Front (mirror)".into(), Point2::new(850.0, 40.0)))
        .unwrap();
    h.run();
    click(&mut h, 450.0, 140.0); // the twin's image of corner 1: (400,100) mirrored and raised
    assert_eq!(h.state().selection, Selection::Vertex(twin, 1));
    assert_eq!(field_text(&h, "X"), "45.0");
    assert_eq!(field_text(&h, "Y"), "14.0"); // the stored 100 plus the offset's 40
    type_into(&mut h, "Y", "16"); // 160 mm where the twin is
    close(piece_of(&h, id).vertices[1].pos, Point2::new(400.0, 120.0)); // 160 - 40
}

#[test]
fn the_allowance_limit_is_named_in_the_current_units() {
    let mut h = harness();
    let id = with_rectangle(&mut h);
    click(&mut h, 250.0, 300.0);
    type_into(&mut h, "Seam allowance", "10,5");
    assert_eq!(
        h.state().notice.as_deref(),
        Some("The seam allowance must be between 0 and 10 cm.")
    );
    h.state_mut().doc.edit(|p| p.units = Units::Inch);
    h.run();
    type_into(&mut h, "Seam allowance", "3,94"); // 100.08 mm: just over
    assert_eq!(piece_of(&h, id).allowance, 10.0);
    assert!(
        h.state()
            .notice
            .as_deref()
            .is_some_and(|n| n == "The seam allowance must be between 0 and 3.93 in.")
    );
    type_into(&mut h, "Seam allowance", "3,93"); // what the message names is accepted
    assert!((piece_of(&h, id).allowance - 3.93 * 25.4).abs() < 1e-9);
}
