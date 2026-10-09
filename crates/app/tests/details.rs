//! M2b on the pattern table: cut-on-fold pieces, mirrored pairs, seam allowance, notches and
//! internal lines, driven the way a student would.

mod common;
use common::*;
use egui::{Key, Modifiers, accesskit::Role, vec2};
use egui_kittest::kittest::{NodeT, Queryable};
use opendrape::editor::{Selection, Tool};
use opendrape_core::{
    Edge, InternalLine, LineKind, Notch, NotchStyle, Piece, PieceId, Point2, Units, VertexKind,
};
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

/// Most shapes a frame at full zoom may hold: the dashes that show are a few hundred. Without
/// clipping, a line kilometres long makes millions.
const FRAME_SHAPES: usize = 3_000;

#[test]
fn a_marking_line_kilometres_long_still_draws_at_full_zoom() {
    let _guard = watchdog(60);
    let mut h = harness();
    let id = with_rectangle(&mut h);
    // 800 metres of line across the pattern table: 40 million screen points at the most the
    // table zooms in, which epaint's own dashes would take gigabytes to lay out.
    h.state_mut().doc.edit(|p| {
        p.piece_mut(id).unwrap().lines.push(InternalLine::open(&[
            Point2::new(-400_000.0, 105.0),
            Point2::new(400_000.0, 105.0),
        ]))
    });
    zoom_in_on(&mut h, 250.0, 105.0);
    h.run();
    let shapes = h.output().shapes.len();
    assert!(shapes > 20, "the dashes of the line show: {shapes}");
    assert!(shapes < FRAME_SHAPES, "{shapes} shapes in a frame");
}

#[test]
fn a_fold_line_kilometres_long_still_draws_at_full_zoom() {
    let _guard = watchdog(60);
    let mut h = harness();
    let id = h.state_mut().doc.edit(|p| {
        let mut half = Piece::rectangle(
            PieceId(0),
            "Tall",
            Point2::new(300.0, -1_000_000.0),
            150.0,
            2_000_000.0,
        );
        half.fold = Some(3);
        p.add_piece(half)
    });
    h.run();
    assert!(!refused_notice(&h));
    assert_eq!(piece_of(&h, id).fold, Some(3));
    zoom_in_on(&mut h, 300.0, 0.0);
    h.run();
    let shapes = h.output().shapes.len();
    assert!(shapes > 20, "the dashes of the fold show: {shapes}");
    assert!(shapes < FRAME_SHAPES, "{shapes} shapes in a frame");
}

#[test]
fn the_rubber_band_from_a_point_kilometres_away_still_draws_at_full_zoom() {
    let _guard = watchdog(60);
    let mut h = harness();
    key(&mut h, Key::H);
    click(&mut h, 150.0, 150.0); // the pen's first point
    assert_eq!(h.state().pen().len(), 1);
    zoom_in_on(&mut h, 900_000.0, 0.0);
    h.hover_at(h.state().canvas_rect.center());
    h.run();
    let shapes = h.output().shapes.len();
    assert!(shapes > 20, "the dashes of the band show: {shapes}");
    assert!(shapes < FRAME_SHAPES, "{shapes} shapes in a frame");
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

#[test]
fn the_notch_tool_adds_a_notch_where_clicked() {
    let mut h = harness();
    let id = with_rectangle(&mut h);
    key(&mut h, Key::N);
    assert_eq!(h.state().tool, Tool::Notch);
    click(&mut h, 250.0, 101.0); // bottom edge, halfway
    let notches = piece_of(&h, id).notches;
    assert_eq!(notches.len(), 1);
    assert_eq!(notches[0].edge, 0);
    assert!(
        (notches[0].distance - 150.0).abs() < 0.5,
        "{}",
        notches[0].distance
    );
    assert_eq!(h.state().selection, Selection::Notch(id, 0));
}

#[test]
fn a_typed_notch_distance_counts_from_the_nearer_end() {
    let mut h = harness();
    let id = with_rectangle(&mut h);
    key(&mut h, Key::N);
    let p = at(&h, 380.0, 101.0); // near the bottom edge's right end
    h.hover_at(p);
    h.run();
    type_number(&mut h, "5");
    assert!(
        h.get_by_role_and_label(Role::TextInput, "Distance")
            .is_focused()
    );
    key(&mut h, Key::Enter);
    assert_eq!(piece_of(&h, id).notches, vec![Notch::new(0, 250.0)]); // 300 - 50
}

#[test]
fn a_notch_distance_past_the_edge_is_refused() {
    let mut h = harness();
    let id = with_rectangle(&mut h);
    key(&mut h, Key::N);
    let p = at(&h, 380.0, 101.0);
    h.hover_at(p);
    h.run();
    type_number(&mut h, "5");
    h.get_by_role_and_label(Role::TextInput, "Distance")
        .type_text("00"); // 500 cm
    h.run();
    key(&mut h, Key::Enter);
    assert!(piece_of(&h, id).notches.is_empty());
    assert!(h.state().notice.is_some());
}

#[test]
fn the_notch_panel_changes_marks_style_and_distance() {
    let mut h = harness();
    let id = with_rectangle(&mut h);
    h.state_mut()
        .doc
        .edit(|p| p.piece_mut(id).unwrap().notches.push(Notch::new(0, 150.0)));
    h.run();
    click(&mut h, 250.0, 92.0); // on the mark, out on the cut line
    assert_eq!(h.state().selection, Selection::Notch(id, 0));
    h.get_by_label("Double").click();
    h.run();
    h.get_by_label("V").click();
    h.run();
    type_into(&mut h, "Distance", "10");
    assert_eq!(
        piece_of(&h, id).notches,
        vec![Notch {
            edge: 0,
            distance: 100.0,
            marks: 2,
            style: NotchStyle::V
        }]
    );
}

#[test]
fn notches_on_a_twin_belong_to_its_piece() {
    let mut h = harness();
    let (id, twin) = with_pair(&mut h);
    key(&mut h, Key::N);
    click(&mut h, 600.0, 101.0); // the twin's bottom edge, halfway
    let notches = piece_of(&h, id).notches;
    assert_eq!(notches.len(), 1);
    assert!((notches[0].distance - 150.0).abs() < 0.5);
    assert_eq!(h.state().selection, Selection::Notch(twin, 0));
}

#[test]
fn selection_of_a_removed_notch_is_dropped() {
    let mut h = harness();
    let id = with_rectangle(&mut h);
    key(&mut h, Key::N);
    click(&mut h, 250.0, 101.0);
    cmd(&mut h, Key::Z);
    assert!(piece_of(&h, id).notches.is_empty());
    assert_eq!(h.state().selection, Selection::Piece(id));
}

#[test]
fn delete_removes_the_selected_notch() {
    let mut h = harness();
    let id = with_rectangle(&mut h);
    h.state_mut()
        .doc
        .edit(|p| p.piece_mut(id).unwrap().notches.push(Notch::new(0, 150.0)));
    h.run();
    click(&mut h, 250.0, 92.0);
    key(&mut h, Key::Delete);
    assert!(piece_of(&h, id).notches.is_empty());
    assert_eq!(h.state().selection, Selection::Piece(id));
}

#[test]
fn a_drag_starting_on_a_notch_moves_nothing() {
    let mut h = harness();
    let id = with_rectangle(&mut h);
    h.state_mut()
        .doc
        .edit(|p| p.piece_mut(id).unwrap().notches.push(Notch::new(0, 150.0)));
    h.run();
    let before = piece_of(&h, id);
    drag(&mut h, (250.0, 92.0), (250.0, 60.0));
    assert_eq!(piece_of(&h, id), before);
}

#[test]
fn a_drag_on_a_notch_selects_nothing() {
    let mut h = harness();
    let id = with_rectangle(&mut h);
    h.state_mut()
        .doc
        .edit(|p| p.piece_mut(id).unwrap().notches.push(Notch::new(0, 150.0)));
    h.run();
    drag(&mut h, (250.0, 92.0), (250.0, 60.0));
    assert_eq!(h.state().selection, Selection::None);
}

#[test]
fn the_delete_notch_button_removes_it() {
    let mut h = harness();
    let id = with_rectangle(&mut h);
    h.state_mut()
        .doc
        .edit(|p| p.piece_mut(id).unwrap().notches.push(Notch::new(0, 150.0)));
    h.run();
    click(&mut h, 250.0, 92.0);
    h.get_by_label("Delete notch").click();
    h.run();
    assert!(piece_of(&h, id).notches.is_empty());
    assert_eq!(h.state().selection, Selection::Piece(id));
}

#[test]
fn a_digit_away_from_every_edge_opens_no_notch_box() {
    let mut h = harness();
    with_rectangle(&mut h);
    key(&mut h, Key::N);
    let p = at(&h, 250.0, 300.0); // well inside the piece
    h.hover_at(p);
    h.run();
    type_number(&mut h, "5");
    assert!(!h.state().length_box_open());
}

#[test]
fn a_notch_distance_typed_by_the_start_of_an_edge_counts_from_the_start() {
    let mut h = harness();
    let id = with_rectangle(&mut h);
    key(&mut h, Key::N);
    let p = at(&h, 120.0, 101.0); // near the bottom edge's left end
    h.hover_at(p);
    h.run();
    type_number(&mut h, "4");
    key(&mut h, Key::Enter);
    assert_eq!(piece_of(&h, id).notches, vec![Notch::new(0, 40.0)]);
    assert_eq!(h.state().selection, Selection::Notch(id, 0));
}

#[test]
fn notches_go_on_a_folded_pieces_drawn_edges_and_never_on_the_fold() {
    let mut h = harness();
    let id = with_half(&mut h);
    key(&mut h, Key::N);
    click(&mut h, 300.0, 250.0); // the fold line, inside the whole piece
    click(&mut h, 225.0, 101.0); // the pale half's bottom edge
    assert!(piece_of(&h, id).notches.is_empty());
    click(&mut h, 375.0, 101.0); // the drawn half's bottom edge, halfway
    let notches = piece_of(&h, id).notches;
    assert_eq!(notches.len(), 1);
    assert!((notches[0].distance - 75.0).abs() < 0.5);
    // Its mirror image on the pale half picks the same stored notch.
    key(&mut h, Key::Z);
    click(&mut h, 225.0, 92.0);
    assert_eq!(h.state().selection, Selection::Notch(id, 0));
}

#[test]
fn a_notch_clicked_off_centre_counts_from_the_edges_start() {
    let mut h = harness();
    let id = with_rectangle(&mut h);
    key(&mut h, Key::N);
    click(&mut h, 175.0, 101.0); // a quarter along the bottom edge (100,100) to (400,100)
    let notches = piece_of(&h, id).notches;
    assert_eq!(notches.len(), 1);
    assert_eq!(notches[0].edge, 0);
    assert!(
        (notches[0].distance - 75.0).abs() < 0.5,
        "{}",
        notches[0].distance
    );

    // The right edge runs up from (400,100): a quarter of its 400 mm is 100 mm.
    let mut h = harness();
    let id = with_rectangle(&mut h);
    key(&mut h, Key::N);
    click(&mut h, 401.0, 200.0);
    let notches = piece_of(&h, id).notches;
    assert_eq!(notches.len(), 1);
    assert_eq!(notches[0].edge, 1);
    assert!(
        (notches[0].distance - 100.0).abs() < 0.5,
        "{}",
        notches[0].distance
    );
}

#[test]
fn a_notch_clicked_off_centre_on_a_twin_counts_from_the_twins_start() {
    // The twin's edge 0 runs from (750,100) to (450,100), like the piece's from (100,100) to
    // (400,100): a quarter of the way along the twin is x = 675, and the stored notch is 75 mm
    // from the stored edge's start (a mirrored curve parameter would say 225).
    let mut h = harness();
    let (id, twin) = with_pair(&mut h);
    key(&mut h, Key::N);
    click(&mut h, 675.0, 101.0);
    let notches = piece_of(&h, id).notches;
    assert_eq!(notches.len(), 1);
    assert_eq!(notches[0].edge, 0);
    assert!(
        (notches[0].distance - 75.0).abs() < 0.5,
        "{}",
        notches[0].distance
    );
    assert_eq!(h.state().selection, Selection::Notch(twin, 0));

    // Edge 1 climbs the twin's near side from (450,100): a quarter of 400 mm is 100 mm.
    let mut h = harness();
    let (id, twin) = with_pair(&mut h);
    key(&mut h, Key::N);
    click(&mut h, 449.0, 200.0);
    let notches = piece_of(&h, id).notches;
    assert_eq!(notches.len(), 1);
    assert_eq!(notches[0].edge, 1);
    assert!(
        (notches[0].distance - 100.0).abs() < 0.5,
        "{}",
        notches[0].distance
    );
    assert_eq!(h.state().selection, Selection::Notch(twin, 0));
}

#[test]
fn a_typed_distance_on_a_twin_counts_from_the_nearer_end_of_the_twins_edge() {
    let mut h = harness();
    let (id, _) = with_pair(&mut h);
    key(&mut h, Key::N);
    // The twin's bottom edge starts at x = 750 and ends at x = 450. Near its start, 4 cm in is
    // 40 mm from the stored start; near its end, 4 cm from the end is 300 - 40 from the start.
    let near_start = at(&h, 730.0, 101.0);
    h.hover_at(near_start);
    h.run();
    type_number(&mut h, "4");
    key(&mut h, Key::Enter);
    let near_end = at(&h, 470.0, 101.0);
    h.hover_at(near_end);
    h.run();
    type_number(&mut h, "4");
    key(&mut h, Key::Enter);
    assert_eq!(
        piece_of(&h, id).notches,
        vec![Notch::new(0, 40.0), Notch::new(0, 260.0)]
    );
}

#[test]
fn clicking_a_twins_notch_mark_selects_it_on_the_twin() {
    let mut h = harness();
    let (id, twin) = with_pair(&mut h);
    // 75 mm along: x = 175 on the piece, and x = 675 on the twin (750 - 75).
    h.state_mut()
        .doc
        .edit(|p| p.piece_mut(id).unwrap().notches.push(Notch::new(0, 75.0)));
    h.run();
    click(&mut h, 675.0, 92.0);
    assert_eq!(h.state().selection, Selection::Notch(twin, 0));
    click(&mut h, 175.0, 92.0);
    assert_eq!(h.state().selection, Selection::Notch(id, 0));
}

#[test]
fn with_the_allowance_hidden_a_notch_is_picked_on_the_stitching_line() {
    let mut h = harness();
    let id = with_rectangle(&mut h);
    // 75 mm along the bottom edge: the mark is on the cut line (y = 90..95) while the
    // allowance is shown, and on the stitching line (y = 100..105) when it is hidden.
    h.state_mut()
        .doc
        .edit(|p| p.piece_mut(id).unwrap().notches.push(Notch::new(0, 75.0)));
    h.state_mut().fit();
    h.run();
    let zoom = h.state().view.zoom;
    assert!(zoom > 1.0, "a pick reaches under 8 mm: {zoom}");
    click(&mut h, 175.0, 104.0); // 9 mm from the cut-line mark: only the edge
    assert_eq!(h.state().selection, Selection::Edge(id, 0));
    click(&mut h, 175.0, 92.0);
    assert_eq!(h.state().selection, Selection::Notch(id, 0));
    h.get_by_label("Show seam allowance").click();
    h.run();
    assert!(!h.state().show_allowance);
    click(&mut h, 175.0, 92.0); // where the mark was drawn before
    assert_eq!(h.state().selection, Selection::None);
    click(&mut h, 175.0, 104.0); // near the tip of the 5 mm mark
    assert_eq!(h.state().selection, Selection::Notch(id, 0));
    // The selected mark draws on the stitching line too, and showing the allowance again
    // brings the cut-line mark back.
    h.run();
    h.get_by_label("Show seam allowance").click();
    h.run();
    click(&mut h, 175.0, 92.0);
    assert_eq!(h.state().selection, Selection::Notch(id, 0));
}

#[test]
fn a_notch_past_the_points_limit_is_refused() {
    let mut h = harness();
    let id = with_rectangle(&mut h);
    // 4 outline points and 1,996 notches: exactly at the limit of 2,000.
    h.state_mut().doc.edit(|p| {
        p.piece_mut(id).unwrap().notches = (0..1_996)
            .map(|k| Notch::new(0, 1.0 + f64::from(k % 200)))
            .collect();
    });
    h.run();
    assert_eq!(piece_of(&h, id).notches.len(), 1_996);
    key(&mut h, Key::N);
    click(&mut h, 350.0, 101.0);
    assert_eq!(piece_of(&h, id).notches.len(), 1_996);
    assert!(refused_notice(&h), "{:?}", h.state().notice);
}

/// The 300 mm rectangle with no seam allowance and a notch right on its first corner (100,100),
/// where the mark runs from the corner 5 mm up into the piece.
fn corner_notch_without_allowance(h: &mut H) -> PieceId {
    let id = with_rectangle(h);
    h.state_mut().doc.edit(|p| {
        let piece = p.piece_mut(id).unwrap();
        piece.allowance = 0.0;
        piece.notches.push(Notch::new(0, 0.0));
    });
    h.run();
    id
}

#[test]
fn a_notch_on_a_corner_without_allowance_does_not_hide_the_corner() {
    let mut h = harness();
    let id = corner_notch_without_allowance(&mut h);
    click(&mut h, 100.0, 100.0);
    assert_eq!(h.state().selection, Selection::Vertex(id, 0));
    // A hair along the mark is still the corner: positions pass through f32 screen points.
    click(&mut h, 100.0, 100.2);
    assert_eq!(h.state().selection, Selection::Vertex(id, 0));
    // Further along the mark the notch is nearer.
    click(&mut h, 100.0, 104.0);
    assert_eq!(h.state().selection, Selection::Notch(id, 0));
    click(&mut h, 100.0, 100.0);
    assert_eq!(h.state().selection, Selection::Vertex(id, 0));

    let mut h = harness();
    let id = corner_notch_without_allowance(&mut h);
    drag(&mut h, (100.0, 100.0), (80.0, 90.0));
    close(piece_of(&h, id).vertices[0].pos, Point2::new(80.0, 90.0));
}

#[test]
fn a_notch_on_the_cut_line_beside_a_corner_does_not_hide_the_corner() {
    let mut h = harness();
    let id = with_rectangle(&mut h); // the default 10 mm allowance: the mark is at y = 90..95
    h.state_mut()
        .doc
        .edit(|p| p.piece_mut(id).unwrap().notches.push(Notch::new(0, 0.0)));
    h.run();
    click(&mut h, 100.0, 100.0);
    assert_eq!(h.state().selection, Selection::Vertex(id, 0));
    click(&mut h, 100.0, 92.0);
    assert_eq!(h.state().selection, Selection::Notch(id, 0));
    click(&mut h, 100.0, 100.0);
    assert_eq!(h.state().selection, Selection::Vertex(id, 0));
}

#[test]
fn a_notch_does_not_hide_a_curve_handle_but_still_beats_the_edge() {
    let mut h = harness();
    let id = with_rectangle(&mut h);
    h.state_mut().doc.edit(|p| {
        let piece = p.piece_mut(id).unwrap();
        // A straight-looking curve whose first handle sits at (200,100); a notch 100 mm along
        // has its mark at x = 200, y = 90..95.
        piece.edges[0] = opendrape_core::Edge::Curve {
            c1: Point2::new(200.0, 100.0),
            c2: Point2::new(300.0, 100.0),
        };
        piece.notches.push(Notch::new(0, 100.0));
    });
    h.run();
    click(&mut h, 250.0, 300.0); // select the piece, so its handles show
    assert_eq!(h.state().selection, Selection::Piece(id));
    click(&mut h, 200.0, 100.0);
    assert_eq!(h.state().selection, Selection::Edge(id, 0)); // the handle
    click(&mut h, 200.0, 92.0);
    assert_eq!(h.state().selection, Selection::Notch(id, 0));
}

#[test]
fn the_nearer_end_of_a_curved_edge_is_by_arc_length_not_curve_parameter() {
    // The bottom edge is a straight-looking curve with its handles pulled to the far end, so
    // x = 300 (200 mm along, past the middle) is at curve parameter 0.37, before the middle.
    let mut h = harness();
    let id = with_rectangle(&mut h);
    h.state_mut().doc.edit(|p| {
        p.piece_mut(id).unwrap().edges[0] = opendrape_core::Edge::Curve {
            c1: Point2::new(350.0, 100.0),
            c2: Point2::new(390.0, 100.0),
        };
    });
    h.run();
    key(&mut h, Key::N);
    let p = at(&h, 300.0, 101.0);
    h.hover_at(p);
    h.run();
    type_number(&mut h, "4");
    key(&mut h, Key::Enter);
    let piece = piece_of(&h, id);
    assert_eq!(piece.notches.len(), 1);
    let len = geom::edge_length(&piece, 0);
    assert!(
        (piece.notches[0].distance - (len - 40.0)).abs() < 0.01,
        "{} of {len}",
        piece.notches[0].distance
    );
}

#[test]
fn delete_removes_a_notch_in_the_notch_tool_too() {
    let mut h = harness();
    let id = with_rectangle(&mut h);
    key(&mut h, Key::N);
    click(&mut h, 250.0, 101.0);
    assert_eq!(h.state().selection, Selection::Notch(id, 0));
    key(&mut h, Key::Delete);
    assert!(piece_of(&h, id).notches.is_empty());
    assert_eq!(h.state().selection, Selection::Piece(id));
}

#[test]
fn delete_in_the_notch_tool_leaves_a_selected_piece_alone() {
    let mut h = harness();
    let id = with_rectangle(&mut h);
    click(&mut h, 250.0, 300.0); // select the piece with the Edit tool
    key(&mut h, Key::N);
    key(&mut h, Key::Delete);
    assert!(h.state().doc.project().piece(id).is_some());
    assert_eq!(h.state().selection, Selection::Piece(id));
}

/// Puts `line` on piece `id`, as if drawn.
fn with_line(h: &mut H, id: PieceId, line: InternalLine) {
    h.state_mut()
        .doc
        .edit(|p| p.piece_mut(id).unwrap().lines.push(line));
    h.run();
}

/// An L-shaped piece: a 300 × 100 mm foot along the bottom and a 100 × 400 mm arm up the left,
/// from (100,100). Its inner corner is (200,200).
fn with_l_shape(h: &mut H) -> PieceId {
    let id = h.state_mut().doc.edit(|p| {
        let corners = [(100.0, 100.0), (400.0, 100.0), (400.0, 200.0)]
            .into_iter()
            .chain([(200.0, 200.0), (200.0, 500.0), (100.0, 500.0)])
            .map(|(x, y)| Point2::new(x, y))
            .collect::<Vec<_>>();
        p.add_piece(Piece::polygon(PieceId(0), "L", &corners))
    });
    h.run();
    id
}

fn notice_is(h: &H, text: &str) -> bool {
    h.state().notice.as_deref() == Some(text)
}

const OUTSIDE: &str = "Internal lines must stay inside their piece.";

#[test]
fn the_line_tool_draws_an_open_line() {
    let mut h = harness();
    let id = with_rectangle(&mut h);
    key(&mut h, Key::L);
    assert_eq!(h.state().tool, Tool::Line);
    click(&mut h, 150.0, 200.0);
    click(&mut h, 300.0, 200.0);
    key(&mut h, Key::Enter);
    let lines = piece_of(&h, id).lines;
    assert_eq!(lines.len(), 1);
    assert!(!lines[0].closed);
    close(lines[0].vertices[1].pos, Point2::new(300.0, 200.0));
    assert_eq!(h.state().selection, Selection::Line(id, 0));
}

#[test]
fn clicking_the_first_point_closes_a_shape_that_can_be_cut_out() {
    let mut h = harness();
    let id = with_rectangle(&mut h);
    key(&mut h, Key::L);
    for (x, y) in [
        (150.0, 200.0),
        (300.0, 200.0),
        (220.0, 350.0),
        (150.0, 200.0),
    ] {
        click(&mut h, x, y);
    }
    assert!(piece_of(&h, id).lines[0].closed);
    h.get_by_label("Cut-out").click();
    h.run();
    assert_eq!(piece_of(&h, id).lines[0].kind, LineKind::Cutout);
}

#[test]
fn points_outside_the_piece_are_refused() {
    let mut h = harness();
    with_rectangle(&mut h);
    key(&mut h, Key::L);
    click(&mut h, 150.0, 200.0);
    click(&mut h, 600.0, 200.0); // outside the piece
    assert_eq!(h.state().line_draft().len(), 1);
    assert!(h.state().notice.is_some());
}

#[test]
fn undo_while_drawing_a_line_removes_its_last_point() {
    let mut h = harness();
    let id = with_rectangle(&mut h);
    key(&mut h, Key::L);
    click(&mut h, 150.0, 200.0);
    click(&mut h, 300.0, 200.0);
    cmd(&mut h, Key::Z);
    assert_eq!(h.state().line_draft().len(), 1);
    assert!(piece_of(&h, id).lines.is_empty());
}

#[test]
fn the_edit_tool_moves_lines_and_their_points_but_keeps_them_inside() {
    let mut h = harness();
    let id = with_rectangle(&mut h);
    h.state_mut().doc.edit(|p| {
        p.piece_mut(id).unwrap().lines.push(InternalLine::open(&[
            Point2::new(150.0, 200.0),
            Point2::new(300.0, 200.0),
        ]))
    });
    h.run();
    drag(&mut h, (225.0, 200.0), (225.0, 250.0)); // the whole line
    close(
        piece_of(&h, id).lines[0].vertices[0].pos,
        Point2::new(150.0, 250.0),
    );
    assert_eq!(h.state().selection, Selection::Line(id, 0));
    drag(&mut h, (150.0, 250.0), (160.0, 260.0)); // one point
    close(
        piece_of(&h, id).lines[0].vertices[0].pos,
        Point2::new(160.0, 260.0),
    );
    drag(&mut h, (160.0, 260.0), (700.0, 260.0)); // out of the piece: not applied
    close(
        piece_of(&h, id).lines[0].vertices[0].pos,
        Point2::new(160.0, 260.0),
    );
}

#[test]
fn a_line_on_a_folded_piece_shows_on_both_halves() {
    let mut h = harness();
    let id = with_half(&mut h);
    key(&mut h, Key::L);
    click(&mut h, 350.0, 200.0);
    click(&mut h, 420.0, 200.0);
    key(&mut h, Key::Enter);
    assert_eq!(piece_of(&h, id).lines.len(), 1);
    let shape = geom::shape_of(h.state().doc.project(), id).unwrap();
    assert_eq!(shape.piece.lines.len(), 2);
}

#[test]
fn selection_of_a_removed_line_is_dropped() {
    let mut h = harness();
    let id = with_rectangle(&mut h);
    key(&mut h, Key::L);
    click(&mut h, 150.0, 200.0);
    click(&mut h, 300.0, 200.0);
    key(&mut h, Key::Enter);
    cmd(&mut h, Key::Z);
    assert!(piece_of(&h, id).lines.is_empty());
    assert_eq!(h.state().selection, Selection::Piece(id));
}

#[test]
fn delete_removes_the_selected_line() {
    let mut h = harness();
    let id = with_rectangle(&mut h);
    key(&mut h, Key::L);
    click(&mut h, 150.0, 200.0);
    click(&mut h, 300.0, 200.0);
    key(&mut h, Key::Enter);
    key(&mut h, Key::Z); // edit tool
    key(&mut h, Key::Delete);
    assert!(piece_of(&h, id).lines.is_empty());
}

#[test]
fn a_line_dragged_out_of_its_piece_is_held_back_and_says_why() {
    let mut h = harness();
    let id = with_rectangle(&mut h);
    with_line(
        &mut h,
        id,
        InternalLine::open(&[Point2::new(150.0, 200.0), Point2::new(300.0, 200.0)]),
    );
    let before = piece_of(&h, id);
    drag(&mut h, (225.0, 200.0), (225.0, 600.0)); // the whole line, up past the top edge
    assert_eq!(piece_of(&h, id), before);
    assert!(notice_is(&h, OUTSIDE), "{:?}", h.state().notice);
    cmd(&mut h, Key::Z); // the held-back drag left no step: this undoes the line itself
    assert!(piece_of(&h, id).lines.is_empty());
}

#[test]
fn line_points_next_to_a_corner_do_not_hide_it_and_the_nearer_one_wins() {
    let mut h = harness();
    let id = with_rectangle(&mut h);
    let tol = h.state().view.mm(8.0);
    // A line point 0.7 tol from the corner (100,100), the pointer on it: the point is nearer.
    let near = 100.0 + 0.5 * tol;
    with_line(
        &mut h,
        id,
        InternalLine::open(&[Point2::new(near, near), Point2::new(300.0, 300.0)]),
    );
    click(&mut h, near, near);
    assert_eq!(h.state().selection, Selection::Line(id, 0));
    // Pointer on the corner: the line point is within reach too, but not nearer.
    click(&mut h, 100.0, 100.0);
    assert_eq!(h.state().selection, Selection::Vertex(id, 0));
}

#[test]
fn a_line_starting_on_a_corner_leaves_the_corner_grabbable() {
    let mut h = harness();
    let id = with_rectangle(&mut h);
    with_line(
        &mut h,
        id,
        InternalLine::open(&[Point2::new(100.0, 100.0), Point2::new(250.0, 250.0)]),
    );
    drag(&mut h, (100.0, 100.0), (90.0, 90.0)); // a tie: the piece's own point wins
    let piece = piece_of(&h, id);
    close(piece.vertices[0].pos, Point2::new(90.0, 90.0));
    close(piece.lines[0].vertices[0].pos, Point2::new(100.0, 100.0));
    assert_eq!(h.state().selection, Selection::Vertex(id, 0));
}

#[test]
fn a_line_point_on_a_curve_handle_leaves_the_handle_grabbable() {
    let mut h = harness();
    let id = with_rectangle(&mut h);
    h.state_mut().doc.edit(|p| {
        p.piece_mut(id).unwrap().edges[0] = Edge::Curve {
            c1: Point2::new(200.0, 130.0),
            c2: Point2::new(300.0, 130.0),
        };
    });
    with_line(
        &mut h,
        id,
        InternalLine::open(&[Point2::new(200.0, 130.0), Point2::new(250.0, 250.0)]),
    );
    click(&mut h, 250.0, 400.0); // select the piece, so its handles show
    assert_eq!(h.state().selection, Selection::Piece(id));
    click(&mut h, 200.0, 130.0);
    assert_eq!(h.state().selection, Selection::Edge(id, 0)); // the handle, not the line point
}

#[test]
fn a_line_on_a_twin_is_stored_on_its_piece_the_mirrored_way() {
    // The twin shows the stored point (x, y) at (850 - x, y + 40): its offset has y ≠ 0 and
    // the points are off centre, so a missed mirror or a flipped y sign lands somewhere else.
    let mut h = harness();
    let id = with_rectangle(&mut h);
    let twin = h
        .state_mut()
        .doc
        .edit(|p| p.add_twin(id, "Front (mirror)".into(), Point2::new(850.0, 40.0)))
        .unwrap();
    h.run();
    key(&mut h, Key::L);
    click(&mut h, 500.0, 200.0);
    click(&mut h, 700.0, 260.0);
    key(&mut h, Key::Enter);
    let line = piece_of(&h, id).lines.remove(0);
    close(line.vertices[0].pos, Point2::new(350.0, 160.0));
    close(line.vertices[1].pos, Point2::new(150.0, 220.0));
    assert_eq!(h.state().selection, Selection::Line(twin, 0));
    let shape = geom::shape_of(h.state().doc.project(), twin).unwrap();
    close(
        shape.piece.lines[0].vertices[0].pos,
        Point2::new(500.0, 200.0),
    );
    close(
        shape.piece.lines[0].vertices[1].pos,
        Point2::new(700.0, 260.0),
    );
}

#[test]
fn dragging_a_line_on_a_twin_moves_the_stored_line_the_mirrored_way() {
    let mut h = harness();
    let id = with_rectangle(&mut h);
    let twin = h
        .state_mut()
        .doc
        .edit(|p| p.add_twin(id, "Front (mirror)".into(), Point2::new(850.0, 40.0)))
        .unwrap();
    // Stored (350,160)–(150,220), shown on the twin at (500,200)–(700,260).
    with_line(
        &mut h,
        id,
        InternalLine::open(&[Point2::new(350.0, 160.0), Point2::new(150.0, 220.0)]),
    );
    drag(&mut h, (500.0, 200.0), (520.0, 230.0)); // the twin's point: right 20, up 30
    let line = piece_of(&h, id).lines.remove(0);
    close(line.vertices[0].pos, Point2::new(330.0, 190.0)); // stored: left 20, up 30
    close(line.vertices[1].pos, Point2::new(150.0, 220.0));
    assert_eq!(h.state().selection, Selection::Line(twin, 0));
    // Now shown from (520,230) to (700,260): its middle is (610,245). Right 10, down 20.
    drag(&mut h, (610.0, 245.0), (620.0, 225.0));
    let line = piece_of(&h, id).lines.remove(0);
    close(line.vertices[0].pos, Point2::new(320.0, 170.0)); // stored: left 10, down 20
    close(line.vertices[1].pos, Point2::new(140.0, 200.0));
    // Dragged out past the twin's right edge (x = 750), which is the stored piece's left edge.
    drag(&mut h, (530.0, 210.0), (800.0, 210.0));
    let held = piece_of(&h, id).lines.remove(0);
    close(held.vertices[0].pos, Point2::new(320.0, 170.0));
    assert!(notice_is(&h, OUTSIDE));
    assert_eq!(
        piece_of(&h, id).twin.map(|t| t.offset),
        Some(Point2::new(850.0, 40.0))
    );
}

#[test]
fn a_line_drawn_on_the_pale_half_is_stored_mirrored_on_the_drawn_half() {
    // The pale half (x 150..300) is not stored, and a stored line on the wrong side of the
    // fold would make the piece invalid: it is stored as its mirror image across the fold,
    // which the pale half shows again where the line was drawn.
    let mut h = harness();
    let id = with_half(&mut h);
    key(&mut h, Key::L);
    click(&mut h, 200.0, 200.0);
    click(&mut h, 250.0, 300.0);
    key(&mut h, Key::Enter);
    assert!(h.state().notice.is_none(), "{:?}", h.state().notice);
    let line = piece_of(&h, id).lines.remove(0);
    close(line.vertices[0].pos, Point2::new(400.0, 200.0));
    close(line.vertices[1].pos, Point2::new(350.0, 300.0));
    let shape = geom::shape_of(h.state().doc.project(), id).unwrap();
    assert_eq!(shape.piece.lines.len(), 2);
    close(
        shape.piece.lines[1].vertices[0].pos,
        Point2::new(200.0, 200.0),
    );
    close(
        shape.piece.lines[1].vertices[1].pos,
        Point2::new(250.0, 300.0),
    );
    assert_eq!(h.state().selection, Selection::Line(id, 0));
}

#[test]
fn the_mirror_image_of_a_line_on_a_fold_is_not_editable() {
    let mut h = harness();
    let id = with_half(&mut h);
    with_line(
        &mut h,
        id,
        InternalLine::open(&[Point2::new(350.0, 200.0), Point2::new(420.0, 200.0)]),
    );
    click(&mut h, 385.0, 200.0);
    assert_eq!(h.state().selection, Selection::Line(id, 0)); // the stored line
    // Its mirror image on the pale half is (250,200)–(180,200): it picks the piece, and a
    // drag from it moves the whole piece.
    click(&mut h, 215.0, 200.0);
    assert_eq!(h.state().selection, Selection::Piece(id));
    drag(&mut h, (215.0, 200.0), (215.0, 230.0));
    let piece = piece_of(&h, id);
    close(piece.vertices[0].pos, Point2::new(300.0, 130.0));
    close(piece.lines[0].vertices[0].pos, Point2::new(350.0, 230.0));
    // Nor are its points: the mirror image of the stored point (350,200), now at (350,230),
    // is at (250,230).
    click(&mut h, 250.0, 230.0);
    assert_eq!(h.state().selection, Selection::Piece(id));
    drag(&mut h, (250.0, 230.0), (250.0, 250.0));
    let piece = piece_of(&h, id);
    close(piece.vertices[0].pos, Point2::new(300.0, 150.0));
    close(piece.lines[0].vertices[0].pos, Point2::new(350.0, 250.0));
    close(piece.lines[0].vertices[1].pos, Point2::new(420.0, 250.0));
}

#[test]
fn the_first_point_decides_the_piece() {
    let mut h = harness();
    let a = with_rectangle(&mut h);
    let b = h.state_mut().doc.edit(|p| {
        p.add_piece(Piece::rectangle(
            PieceId(0),
            "Back",
            Point2::new(500.0, 100.0),
            200.0,
            400.0,
        ))
    });
    h.run();
    key(&mut h, Key::L);
    click(&mut h, 150.0, 200.0);
    click(&mut h, 600.0, 200.0); // inside the other piece
    assert_eq!(h.state().line_draft().len(), 1);
    assert!(notice_is(&h, OUTSIDE));
    click(&mut h, 300.0, 300.0);
    key(&mut h, Key::Enter);
    assert_eq!(piece_of(&h, a).lines.len(), 1);
    assert!(piece_of(&h, b).lines.is_empty());
}

#[test]
fn a_line_may_start_on_the_outline_but_not_beside_it() {
    let mut h = harness();
    with_rectangle(&mut h);
    key(&mut h, Key::L);
    click(&mut h, 98.0, 300.0); // 2 mm outside the left edge
    assert!(h.state().line_draft().is_empty());
    assert!(notice_is(&h, OUTSIDE));
    click(&mut h, 99.8, 300.0); // 0.2 mm outside: on the outline
    assert_eq!(h.state().line_draft().len(), 1);
}

#[test]
fn a_line_may_not_cut_across_a_notch_in_the_piece() {
    // Both ends are inside the L, but the straight line between them leaves it.
    let mut h = harness();
    let id = with_l_shape(&mut h);
    key(&mut h, Key::L);
    click(&mut h, 150.0, 450.0); // up the arm
    click(&mut h, 350.0, 150.0); // along the foot
    assert_eq!(h.state().line_draft().len(), 2);
    key(&mut h, Key::Enter);
    assert!(piece_of(&h, id).lines.is_empty());
    assert_eq!(h.state().line_draft().len(), 2, "the draft is kept");
    assert!(notice_is(&h, OUTSIDE), "{:?}", h.state().notice);
    // Round the corner instead, and it is fine.
    key(&mut h, Key::Backspace);
    click(&mut h, 150.0, 150.0);
    click(&mut h, 350.0, 150.0);
    key(&mut h, Key::Enter);
    assert_eq!(piece_of(&h, id).lines.len(), 1);
    assert_eq!(piece_of(&h, id).lines[0].vertices.len(), 3);
}

#[test]
fn dragging_a_line_point_across_a_notch_in_the_piece_is_refused() {
    let mut h = harness();
    let id = with_l_shape(&mut h);
    with_line(
        &mut h,
        id,
        InternalLine::open(&[Point2::new(150.0, 450.0), Point2::new(150.0, 150.0)]),
    );
    // The new end (350,150) is inside the foot, but the line to it crosses the notch.
    drag(&mut h, (150.0, 150.0), (350.0, 150.0));
    close(
        piece_of(&h, id).lines[0].vertices[1].pos,
        Point2::new(150.0, 150.0),
    );
    assert!(notice_is(&h, OUTSIDE));
}

#[test]
fn escape_cancels_a_line_and_backspace_removes_its_last_point() {
    let mut h = harness();
    let id = with_rectangle(&mut h);
    key(&mut h, Key::L);
    click(&mut h, 150.0, 200.0);
    click(&mut h, 300.0, 200.0);
    key(&mut h, Key::Backspace);
    assert_eq!(h.state().line_draft().len(), 1);
    key(&mut h, Key::Escape);
    assert!(h.state().line_draft().is_empty());
    assert!(piece_of(&h, id).lines.is_empty());
}

#[test]
fn a_line_needs_two_points() {
    let mut h = harness();
    let id = with_rectangle(&mut h);
    key(&mut h, Key::L);
    click(&mut h, 150.0, 200.0);
    key(&mut h, Key::Enter);
    assert!(piece_of(&h, id).lines.is_empty());
    assert_eq!(h.state().line_draft().len(), 1);
    assert!(notice_is(
        &h,
        "A line needs at least 2 points, and a closed shape 3."
    ));
}

#[test]
fn clicking_the_last_point_again_finishes_an_open_line() {
    let mut h = harness();
    let id = with_rectangle(&mut h);
    key(&mut h, Key::L);
    for (x, y) in [
        (150.0, 200.0),
        (300.0, 200.0),
        (300.0, 350.0),
        (300.0, 350.0),
    ] {
        click(&mut h, x, y);
    }
    let lines = piece_of(&h, id).lines;
    assert_eq!(lines.len(), 1);
    assert!(!lines[0].closed);
    assert_eq!(lines[0].vertices.len(), 3);
    assert_eq!(lines[0].edges.len(), 2);
    assert!(h.state().line_draft().is_empty());
}

#[test]
fn clicking_the_first_point_of_a_two_point_line_does_not_close_it() {
    let mut h = harness();
    let id = with_rectangle(&mut h);
    key(&mut h, Key::L);
    click(&mut h, 150.0, 200.0);
    click(&mut h, 300.0, 200.0);
    click(&mut h, 150.0, 200.0); // a closed shape needs 3 points: nothing happens
    assert_eq!(h.state().line_draft().len(), 2);
    assert!(piece_of(&h, id).lines.is_empty());
    assert!(h.state().notice.is_none(), "{:?}", h.state().notice);
}

#[test]
fn pressing_and_dragging_makes_a_curve_point() {
    let mut h = harness();
    let id = with_rectangle(&mut h);
    key(&mut h, Key::L);
    drag(&mut h, (150.0, 200.0), (200.0, 260.0));
    assert!(h.state().line_draft()[0].handle.is_some());
    click(&mut h, 350.0, 200.0);
    key(&mut h, Key::Enter);
    let line = piece_of(&h, id).lines.remove(0);
    assert_eq!(line.vertices[0].kind, VertexKind::Smooth);
    let Edge::Curve { c1, .. } = line.edges[0] else {
        panic!("a curve: {:?}", line.edges)
    };
    close(c1, Point2::new(200.0, 260.0));
    assert_eq!(line.edges.len(), 1, "an open line has no closing edge");
}

#[test]
fn dragging_a_line_point_carries_its_curve_handles() {
    let mut h = harness();
    let id = with_rectangle(&mut h);
    let mut line = InternalLine::open(&[
        Point2::new(150.0, 200.0),
        Point2::new(250.0, 300.0),
        Point2::new(350.0, 200.0),
    ]);
    line.edges = vec![
        Edge::Curve {
            c1: Point2::new(170.0, 260.0),
            c2: Point2::new(220.0, 300.0),
        },
        Edge::Curve {
            c1: Point2::new(280.0, 300.0),
            c2: Point2::new(330.0, 260.0),
        },
    ];
    with_line(&mut h, id, line);
    drag(&mut h, (250.0, 300.0), (250.0, 320.0));
    let line = piece_of(&h, id).lines.remove(0);
    close(line.vertices[1].pos, Point2::new(250.0, 320.0));
    let (Edge::Curve { c1: a1, c2: a2 }, Edge::Curve { c1: b1, c2: b2 }) =
        (line.edges[0], line.edges[1])
    else {
        panic!("curves")
    };
    close(a2, Point2::new(220.0, 320.0)); // the handles beside the point went with it
    close(b1, Point2::new(280.0, 320.0));
    close(a1, Point2::new(170.0, 260.0)); // the far ones stayed
    close(b2, Point2::new(330.0, 260.0));
}

#[test]
fn the_line_panel_shows_the_length_and_only_a_closed_line_can_be_cut_out() {
    let mut h = harness();
    let id = with_rectangle(&mut h);
    with_line(
        &mut h,
        id,
        InternalLine::open(&[Point2::new(150.0, 200.0), Point2::new(300.0, 200.0)]),
    );
    click(&mut h, 225.0, 200.0);
    assert_eq!(h.state().selection, Selection::Line(id, 0));
    h.get_by_label("Length: 15.0 cm");
    assert!(h.get_by_label("Cut-out").accesskit_node().is_disabled());
    assert!(!h.get_by_label("Marking").accesskit_node().is_disabled());
    with_line(
        &mut h,
        id,
        InternalLine::polygon(&[
            Point2::new(150.0, 300.0),
            Point2::new(250.0, 300.0),
            Point2::new(200.0, 400.0),
        ]),
    );
    click(&mut h, 200.0, 300.0); // the triangle's bottom edge
    assert_eq!(h.state().selection, Selection::Line(id, 1));
    assert!(!h.get_by_label("Cut-out").accesskit_node().is_disabled());
    h.get_by_label("Cut-out").click();
    h.run();
    assert_eq!(piece_of(&h, id).lines[1].kind, LineKind::Cutout);
    h.get_by_label("Marking").click();
    h.run();
    assert_eq!(piece_of(&h, id).lines[1].kind, LineKind::Marking);
    h.get_by_label("Delete line").click();
    h.run();
    assert_eq!(piece_of(&h, id).lines.len(), 1);
    assert_eq!(h.state().selection, Selection::Piece(id));
}

#[test]
fn redo_waits_while_a_line_is_being_drawn() {
    let mut h = harness();
    let id = with_rectangle(&mut h);
    key(&mut h, Key::L);
    click(&mut h, 150.0, 200.0);
    click(&mut h, 300.0, 200.0);
    key(&mut h, Key::Enter);
    cmd(&mut h, Key::Z); // takes the line away
    assert!(h.state().can_redo());
    click(&mut h, 200.0, 300.0); // starts another line
    assert!(h.state().can_undo() && !h.state().can_redo());
    h.key_press_modifiers(Modifiers::COMMAND | Modifiers::SHIFT, Key::Z);
    h.run();
    assert!(piece_of(&h, id).lines.is_empty());
    key(&mut h, Key::Escape);
    assert!(h.state().can_redo());
    h.key_press_modifiers(Modifiers::COMMAND | Modifiers::SHIFT, Key::Z);
    h.run();
    assert_eq!(piece_of(&h, id).lines.len(), 1);
}

#[test]
fn a_line_that_sticks_out_can_be_brought_back_in_one_point_at_a_time() {
    // Reshaping a piece can leave a line outside it. Held to "stay inside", neither end could
    // ever move, because the other would still be out.
    let mut h = harness();
    let id = with_rectangle(&mut h);
    with_line(
        &mut h,
        id,
        InternalLine::open(&[Point2::new(50.0, 200.0), Point2::new(450.0, 200.0)]),
    );
    drag(&mut h, (450.0, 200.0), (350.0, 200.0));
    close(
        piece_of(&h, id).lines[0].vertices[1].pos,
        Point2::new(350.0, 200.0),
    );
    drag(&mut h, (50.0, 200.0), (150.0, 200.0)); // now it is all inside
    close(
        piece_of(&h, id).lines[0].vertices[0].pos,
        Point2::new(150.0, 200.0),
    );
    assert!(h.state().notice.is_none(), "{:?}", h.state().notice);
    drag(&mut h, (150.0, 200.0), (50.0, 200.0)); // and from here on it is held
    close(
        piece_of(&h, id).lines[0].vertices[0].pos,
        Point2::new(150.0, 200.0),
    );
    assert!(notice_is(&h, OUTSIDE));
}

#[test]
fn a_line_started_a_fraction_of_a_millimetre_either_side_of_the_fold_is_stored() {
    // The fold is x = 300. A click that close to it is on the fold line, and the line is stored
    // with that point on it: not refused for lying on both sides, whichever side it landed on.
    let mut h = harness();
    let id = with_half(&mut h);
    key(&mut h, Key::L);
    for (first, second, stored_second) in [
        ((299.7, 200.0), (380.0, 300.0), (380.0, 300.0)), // pale side, the rest on the drawn half
        ((300.3, 150.0), (380.0, 250.0), (380.0, 250.0)), // drawn side, the rest on the drawn half
        ((299.7, 250.0), (250.0, 300.0), (350.0, 300.0)), // pale side, the rest on the pale half
        ((300.3, 120.0), (250.0, 160.0), (350.0, 160.0)), // drawn side, the rest on the pale half
    ] {
        click(&mut h, first.0, first.1);
        click(&mut h, second.0, second.1);
        key(&mut h, Key::Enter);
        assert!(
            h.state().notice.is_none(),
            "{first:?}: {:?}",
            h.state().notice
        );
        let line = piece_of(&h, id).lines.pop().unwrap();
        assert!(
            (line.vertices[0].pos.x - 300.0).abs() < 1e-9,
            "{first:?}: {line:?}"
        );
        close(line.vertices[0].pos, Point2::new(300.0, first.1));
        close(
            line.vertices[1].pos,
            Point2::new(stored_second.0, stored_second.1),
        );
    }
    assert_eq!(piece_of(&h, id).lines.len(), 4);
}

/// A 1200 × 100 mm piece with an 8 mm wide, 40 mm deep slot cut in from the top at
/// x = 602..610, shown whole.
fn with_slotted(h: &mut H) -> PieceId {
    let id = h.state_mut().doc.edit(|p| {
        let corners = [
            (0.0, 0.0),
            (1200.0, 0.0),
            (1200.0, 100.0),
            (610.0, 100.0),
            (610.0, 60.0),
            (602.0, 60.0),
            (602.0, 100.0),
            (0.0, 100.0),
        ]
        .map(|(x, y)| Point2::new(x, y));
        p.add_piece(Piece::polygon(PieceId(0), "Slotted", &corners))
    });
    h.state_mut().fit();
    h.run();
    id
}

#[test]
fn a_metre_long_line_may_not_cross_a_narrow_slot() {
    let mut h = harness();
    let id = with_slotted(&mut h);
    key(&mut h, Key::L);
    click(&mut h, 50.0, 80.0);
    click(&mut h, 1150.0, 80.0); // both ends are inside the piece
    key(&mut h, Key::Enter);
    assert!(piece_of(&h, id).lines.is_empty());
    assert_eq!(h.state().line_draft().len(), 2, "the draft is kept");
    assert!(notice_is(&h, OUTSIDE), "{:?}", h.state().notice);
    // Up to the slot is fine.
    key(&mut h, Key::Backspace);
    click(&mut h, 590.0, 80.0);
    key(&mut h, Key::Enter);
    assert_eq!(piece_of(&h, id).lines.len(), 1);
}

#[test]
fn a_notch_does_not_hide_a_line_point_that_starts_at_it() {
    let mut h = harness();
    let id = with_rectangle(&mut h);
    h.state_mut().doc.edit(|p| {
        let piece = p.piece_mut(id).unwrap();
        piece.allowance = 0.0; // the notch mark runs 5 mm in from the stitching line
        piece.notches.push(Notch::new(0, 150.0)); // at (250,100), up to (250,105)
        piece.lines.push(InternalLine::open(&[
            Point2::new(250.0, 100.0),
            Point2::new(250.0, 300.0),
        ]));
    });
    h.run();
    assert!(
        h.state().view.mm(8.0) > 6.0,
        "both are within reach of each other"
    );
    click(&mut h, 250.0, 105.0); // the mark's inner end
    assert_eq!(h.state().selection, Selection::Notch(id, 0));
    click(&mut h, 250.0, 100.0); // the line's first point, where the mark starts
    assert_eq!(h.state().selection, Selection::Line(id, 0));
    drag(&mut h, (250.0, 100.0), (270.0, 150.0));
    close(
        piece_of(&h, id).lines[0].vertices[0].pos,
        Point2::new(270.0, 150.0),
    );
    assert_eq!(piece_of(&h, id).notches, vec![Notch::new(0, 150.0)]);
}

#[test]
fn delete_removes_a_selected_line_in_the_line_tool_too() {
    for removing in [Key::Delete, Key::Backspace] {
        let mut h = harness();
        let id = with_rectangle(&mut h);
        key(&mut h, Key::L);
        click(&mut h, 150.0, 200.0);
        click(&mut h, 300.0, 200.0);
        key(&mut h, Key::Enter);
        assert_eq!(h.state().selection, Selection::Line(id, 0));
        key(&mut h, removing);
        assert!(piece_of(&h, id).lines.is_empty(), "{removing:?}");
        assert_eq!(h.state().selection, Selection::Piece(id));
        assert_eq!(h.state().tool, Tool::Line);
    }
}

#[test]
fn delete_in_the_line_tool_leaves_a_selected_piece_alone() {
    let mut h = harness();
    let id = with_rectangle(&mut h);
    click(&mut h, 250.0, 300.0); // select the piece with the Edit tool
    key(&mut h, Key::L);
    key(&mut h, Key::Delete);
    assert!(h.state().doc.project().piece(id).is_some());
    assert_eq!(h.state().selection, Selection::Piece(id));
}

#[test]
fn delete_while_drawing_a_line_removes_draft_points_not_the_selected_line() {
    let mut h = harness();
    let id = with_rectangle(&mut h);
    key(&mut h, Key::L);
    click(&mut h, 150.0, 200.0);
    click(&mut h, 300.0, 200.0);
    key(&mut h, Key::Enter);
    click(&mut h, 150.0, 400.0);
    click(&mut h, 300.0, 400.0);
    key(&mut h, Key::Delete);
    assert_eq!(h.state().line_draft().len(), 1);
    assert_eq!(piece_of(&h, id).lines.len(), 1, "the finished line stays");
}

#[test]
fn starting_a_new_line_drops_the_selection_of_the_old_one() {
    let mut h = harness();
    let id = with_rectangle(&mut h);
    key(&mut h, Key::L);
    click(&mut h, 150.0, 200.0);
    click(&mut h, 300.0, 200.0);
    key(&mut h, Key::Enter);
    assert_eq!(h.state().selection, Selection::Line(id, 0)); // a finished line stays selected
    click(&mut h, 150.0, 400.0);
    assert_eq!(h.state().line_draft().len(), 1);
    assert_eq!(h.state().selection, Selection::Piece(id));
    key(&mut h, Key::Backspace); // the draft's only point
    key(&mut h, Key::Backspace); // nothing left to remove, and no line selected to delete
    assert!(h.state().line_draft().is_empty());
    assert_eq!(piece_of(&h, id).lines.len(), 1);
}

#[test]
fn starting_a_line_drops_a_notch_selection_too() {
    let mut h = harness();
    let id = with_rectangle(&mut h);
    key(&mut h, Key::N);
    click(&mut h, 250.0, 101.0);
    assert_eq!(h.state().selection, Selection::Notch(id, 0));
    key(&mut h, Key::L);
    click(&mut h, 150.0, 200.0);
    assert_eq!(h.state().selection, Selection::Piece(id));
    key(&mut h, Key::Backspace);
    key(&mut h, Key::Backspace);
    assert_eq!(piece_of(&h, id).notches.len(), 1);
}

#[test]
fn tab_cycles_between_the_two_box_fields() {
    let mut h = harness();
    key(&mut h, Key::H);
    click(&mut h, 100.0, 100.0);
    type_number(&mut h, "3");
    key(&mut h, Key::Tab);
    assert!(
        h.get_by_role_and_label(Role::TextInput, "Angle")
            .is_focused()
    );
    key(&mut h, Key::Tab);
    assert!(h.state().length_box_open(), "Tab never closes the box");
    assert!(
        h.get_by_role_and_label(Role::TextInput, "Length")
            .is_focused()
    );
    key(&mut h, Key::Tab);
    assert!(
        h.get_by_role_and_label(Role::TextInput, "Angle")
            .is_focused()
    );
}

#[test]
fn tab_in_a_one_field_box_keeps_it_open() {
    let mut h = harness();
    with_rectangle(&mut h);
    key(&mut h, Key::N);
    let p = at(&h, 380.0, 101.0);
    h.hover_at(p);
    h.run();
    type_number(&mut h, "5");
    key(&mut h, Key::Tab);
    assert!(h.state().length_box_open());
    assert!(
        h.get_by_role_and_label(Role::TextInput, "Distance")
            .is_focused()
    );
}
