//! M4a on the pattern table: seams sewn with the Sew tool, how edits keep them, and the seam
//! panel, driven the way a student would.

mod common;
use common::*;
use egui::{Key, Modifiers};
use egui_kittest::kittest::Queryable;
use opendrape::editor::{Selection, Tool};
use opendrape_core::{Half, Piece, PieceId, Point2, Seam, SeamId, SeamSide};

/// A 150 × 300 mm half piece at (300,100), folded on its left edge (x = 300): stored edges 0
/// bottom, 1 right, 2 top, 3 the fold. Its pale half covers x = 150..300.
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

/// A 300 × 400 mm "Back" at (600,100): edges 0 bottom, 1 right, 2 top, 3 left.
fn with_back(h: &mut H) -> PieceId {
    let id = h.state_mut().doc.edit(|p| {
        p.add_piece(Piece::rectangle(
            PieceId(0),
            "Back",
            Point2::new(600.0, 100.0),
            300.0,
            400.0,
        ))
    });
    h.run();
    id
}

fn side(shape: PieceId, half: Half, first_edge: usize, edges: usize, forward: bool) -> SeamSide {
    SeamSide::new(shape, half, first_edge, edges, forward)
}

fn sew(h: &mut H, a: SeamSide, b: SeamSide) -> SeamId {
    let id = h.state_mut().doc.edit(|p| p.add_seam(a, b));
    h.run();
    id
}

fn seam_of(h: &H, id: SeamId) -> Option<Seam> {
    h.state().doc.project().seam(id).copied()
}

#[test]
fn adding_and_deleting_points_keeps_seams_sewn() {
    let mut h = harness();
    let front = with_rectangle(&mut h); // (100,100)–(400,500)
    let back = with_back(&mut h);
    let seam = sew(
        &mut h,
        side(front, Half::Drawn, 1, 1, true),
        side(back, Half::Drawn, 3, 1, false),
    );
    key(&mut h, Key::X);
    click(&mut h, 401.0, 300.0); // the front's right edge, halfway
    assert_eq!(piece_of(&h, front).len(), 5);
    assert_eq!(
        seam_of(&h, seam).unwrap().a,
        side(front, Half::Drawn, 1, 2, true)
    );
    key(&mut h, Key::Z); // the Edit tool; the new point stays selected
    key(&mut h, Key::Delete);
    assert_eq!(piece_of(&h, front).len(), 4);
    assert_eq!(
        seam_of(&h, seam).unwrap().a,
        side(front, Half::Drawn, 1, 1, true)
    );
}

#[test]
fn unfolding_keeps_the_seams_on_both_halves() {
    let mut h = harness();
    let front = with_half(&mut h);
    let back = with_back(&mut h);
    let seam = sew(
        &mut h,
        side(front, Half::Pale, 1, 1, true),
        side(back, Half::Drawn, 3, 1, false),
    );
    click(&mut h, 375.0, 250.0); // inside the drawn half
    h.get_by_label("Unfold").click();
    h.run();
    assert_eq!(piece_of(&h, front).fold, None);
    // The pale image of the right edge is the whole piece's edge 4, running the other way.
    assert_eq!(
        seam_of(&h, seam).unwrap().a,
        side(front, Half::Drawn, 4, 1, false)
    );
    assert!(
        h.query_by_label("Remove fold").is_none(),
        "the fold is gone"
    );
}

#[test]
fn a_sewn_edge_cannot_become_the_fold() {
    let mut h = harness();
    let front = with_rectangle(&mut h);
    let back = with_back(&mut h);
    sew(
        &mut h,
        side(front, Half::Drawn, 3, 1, true),
        side(back, Half::Drawn, 1, 1, true),
    );
    click(&mut h, 100.0, 300.0); // the front's left edge (edge 3)
    h.get_by_label("Set as fold line").click();
    h.run();
    assert_eq!(piece_of(&h, front).fold, None);
    assert_eq!(
        h.state().notice.as_deref(),
        Some(
            "The fold line must be a straight edge with no notches or seams on it, and the whole piece (with its lines) on one side of it."
        )
    );
}

fn seams(h: &H) -> Vec<Seam> {
    h.state().doc.project().seams.clone()
}

fn notice(h: &H) -> Option<String> {
    h.state().notice.clone()
}

#[test]
fn two_clicks_make_a_seam_whose_starts_meet() {
    let mut h = harness();
    let front = with_rectangle(&mut h); // edges: 0 bottom, 1 right, 2 top, 3 left
    let back = with_back(&mut h);
    key(&mut h, Key::W);
    assert_eq!(h.state().tool, Tool::Sew);
    click(&mut h, 401.0, 150.0); // the front's right edge, near its start (400,100)
    assert!(seams(&h).is_empty(), "half made");
    click(&mut h, 599.0, 150.0); // the back's left edge, near its end (600,100)
    let made = seams(&h);
    assert_eq!(
        made,
        vec![Seam {
            id: SeamId(1),
            a: side(front, Half::Drawn, 1, 1, true),
            b: side(back, Half::Drawn, 3, 1, false),
        }]
    );
    assert_eq!(h.state().selection, Selection::Seam(SeamId(1)));
    cmd(&mut h, Key::Z);
    assert!(seams(&h).is_empty(), "one undo step");
    assert_eq!(
        h.state().selection,
        Selection::None,
        "a removed seam is not selected"
    );
}

#[test]
fn shift_clicks_add_the_next_edges_to_either_side() {
    let mut h = harness();
    let front = with_rectangle(&mut h);
    let back = with_back(&mut h);
    key(&mut h, Key::W);
    click(&mut h, 401.0, 150.0); // front edge 1, forward
    shift_click(&mut h, 250.0, 501.0); // front edge 2, after it
    click(&mut h, 599.0, 150.0); // back edge 3, backwards
    shift_click(&mut h, 750.0, 501.0); // back edge 2, before it
    assert_eq!(seams(&h)[0].a, side(front, Half::Drawn, 1, 2, true));
    assert_eq!(seams(&h)[0].b, side(back, Half::Drawn, 2, 2, false));
    cmd(&mut h, Key::Z);
    assert_eq!(
        seams(&h)[0].b,
        side(back, Half::Drawn, 3, 1, false),
        "each edge added is a step"
    );
    // Clicking away from every edge ends extending: the next Shift-click starts a new side.
    click(&mut h, 250.0, 300.0);
    shift_click(&mut h, 750.0, 501.0);
    assert_eq!(seams(&h)[0].b, side(back, Half::Drawn, 3, 1, false));
}

#[test]
fn edges_that_are_sewn_or_not_next_are_refused() {
    let mut h = harness();
    with_rectangle(&mut h);
    with_back(&mut h);
    key(&mut h, Key::W);
    click(&mut h, 401.0, 150.0);
    shift_click(&mut h, 650.0, 101.0); // the back's bottom edge: another piece
    assert_eq!(
        notice(&h).as_deref(),
        Some("Shift-click an edge right next to this side, on the same piece.")
    );
    shift_click(&mut h, 401.0, 450.0); // the first side's own edge: it is not *next* to it
    assert_eq!(
        notice(&h).as_deref(),
        Some("Shift-click an edge right next to this side, on the same piece.")
    );
    click(&mut h, 401.0, 450.0); // the first side's own edge, to sew it to
    assert_eq!(
        notice(&h).as_deref(),
        Some("Pick a different edge for the other side of the seam.")
    );
    click(&mut h, 599.0, 150.0);
    assert_eq!(seams(&h).len(), 1);
    click(&mut h, 401.0, 300.0); // sewn now
    assert_eq!(notice(&h).as_deref(), Some("This edge is already sewn."));
    assert_eq!(seams(&h).len(), 1);
}

#[test]
fn escape_cancels_a_half_made_seam() {
    let mut h = harness();
    with_rectangle(&mut h);
    with_back(&mut h);
    key(&mut h, Key::W);
    click(&mut h, 401.0, 150.0);
    key(&mut h, Key::Escape);
    click(&mut h, 599.0, 150.0); // starts a new seam instead of finishing one
    assert!(seams(&h).is_empty());
}

#[test]
fn mirrored_seams_appear_by_themselves() {
    let mut h = harness();
    let front = with_half(&mut h); // drawn half x 300..450, pale half x 150..300
    let back = with_back(&mut h);
    let twin = h
        .state_mut()
        .doc
        .edit(|p| p.add_twin(back, "Back (mirror)".into(), Point2::new(1900.0, 0.0)))
        .unwrap(); // the twin covers x 1000..1300
    h.state_mut().fit();
    h.run();
    key(&mut h, Key::W);
    click(&mut h, 451.0, 150.0); // the drawn half's right edge
    click(&mut h, 599.0, 150.0); // the back's left edge
    let all = h.state().doc.project().all_seams();
    assert_eq!(all.len(), 2);
    assert_eq!(all[1].0.a, side(front, Half::Pale, 1, 1, true));
    assert_eq!(all[1].0.b, side(twin, Half::Drawn, 3, 1, false));
    // The mirror image's edges are sewn too.
    click(&mut h, 149.0, 150.0); // the pale half's outer edge
    assert_eq!(notice(&h).as_deref(), Some("This edge is already sewn."));
    click(&mut h, 1301.0, 150.0); // the twin's matching edge
    assert_eq!(notice(&h).as_deref(), Some("This edge is already sewn."));
    // A twin's own free edge can be sewn: its bottom edge to the pale half's bottom. Each is
    // clicked a quarter of the way along, so which end is nearer is not a matter of rounding.
    // The twin's bottom edge runs from x = 1300 to x = 1000, so x = 1225 is near its start. The
    // pale half's bottom is the mirror image of the stored edge (300,100) to (450,100): it
    // runs from x = 300 to x = 150, so x = 187.5 is near its end, and the side starts there,
    // running against the stored way.
    click(&mut h, 1225.0, 99.0);
    click(&mut h, 187.5, 99.0);
    assert_eq!(seams(&h).len(), 2);
    assert_eq!(seams(&h)[1].a, side(twin, Half::Drawn, 0, 1, true));
    assert_eq!(seams(&h)[1].b, side(front, Half::Pale, 0, 1, false));
}

#[test]
fn delete_removes_the_selected_seam() {
    let mut h = harness();
    with_rectangle(&mut h);
    with_back(&mut h);
    key(&mut h, Key::W);
    click(&mut h, 401.0, 150.0);
    click(&mut h, 599.0, 150.0);
    key(&mut h, Key::Z); // the Edit tool: the seam stays selected
    key(&mut h, Key::Delete);
    assert!(seams(&h).is_empty());
    assert_eq!(h.state().selection, Selection::None);
}

#[test]
fn a_back_sewn_to_its_own_mirror_image_is_one_seam() {
    let mut h = harness();
    let back = with_back(&mut h); // 600..900; its twin at 1000..1300
    let twin = h
        .state_mut()
        .doc
        .edit(|p| p.add_twin(back, "Back (mirror)".into(), Point2::new(1900.0, 0.0)))
        .unwrap();
    h.state_mut().fit();
    h.run();
    key(&mut h, Key::W);
    click(&mut h, 901.0, 150.0); // the back's right edge, near its start (900,100)
    click(&mut h, 999.0, 450.0); // the twin's matching edge, near its far end: sewn twisted
    assert_eq!(notice(&h), None, "not refused");
    let all = h.state().doc.project().all_seams();
    assert_eq!(
        all.len(),
        1,
        "its own mirror image: drawn and stitched once"
    );
    assert_eq!(all[0].0.a, side(back, Half::Drawn, 1, 1, true));
    assert_eq!(all[0].0.b, side(twin, Half::Drawn, 1, 1, false));
}

#[test]
fn undo_and_deletion_drop_a_seam_selection_and_a_half_made_seam() {
    let mut h = harness();
    let front = with_rectangle(&mut h);
    let back = with_back(&mut h);
    key(&mut h, Key::W);
    click(&mut h, 401.0, 150.0); // front edge 1
    click(&mut h, 599.0, 150.0); // back edge 3: the seam, whose second side can now grow
    shift_click(&mut h, 750.0, 501.0); // back edge 2 joins the second side
    cmd(&mut h, Key::Z); // the extension goes
    cmd(&mut h, Key::Z); // the seam itself goes
    assert!(seams(&h).is_empty());
    assert_eq!(h.state().selection, Selection::None);
    // Nothing is left to extend: a Shift-click starts a new side (it is not refused as "not
    // next" by a seam that no longer exists), and the next click sews to it.
    shift_click(&mut h, 750.0, 501.0); // back edge 2
    assert_eq!(notice(&h), None);
    click(&mut h, 401.0, 150.0); // front edge 1
    let made = seams(&h);
    assert_eq!(made.len(), 1, "the new side was kept as a half-made seam");
    assert_eq!((made[0].a.shape, made[0].a.first_edge), (back, 2));
    assert_eq!((made[0].b.shape, made[0].b.first_edge), (front, 1));

    // A half-made seam whose piece is deleted is dropped too.
    let mut h = harness();
    let front = with_rectangle(&mut h);
    let back = with_back(&mut h);
    key(&mut h, Key::W);
    click(&mut h, 401.0, 150.0); // front edge 1: half made
    h.state_mut().doc.edit(|p| p.remove_piece(front));
    h.run();
    click(&mut h, 599.0, 150.0); // back edge 3: starts a new seam; the front is gone
    assert!(seams(&h).is_empty());
    assert_eq!(notice(&h), None, "nothing was refused: there was no draft");
    click(&mut h, 750.0, 501.0); // back edge 2: finishes it
    let made = seams(&h);
    assert_eq!(made.len(), 1);
    assert_eq!((made[0].a.shape, made[0].a.first_edge), (back, 3));
    assert_eq!((made[0].b.shape, made[0].b.first_edge), (back, 2));
}

#[test]
fn undo_cancels_a_half_made_seam_before_it_undoes_anything() {
    let mut h = harness();
    let front = with_rectangle(&mut h);
    let back = with_back(&mut h);
    sew(
        &mut h,
        side(front, Half::Drawn, 1, 1, true),
        side(back, Half::Drawn, 3, 1, false),
    );
    key(&mut h, Key::W);
    click(&mut h, 250.0, 99.0); // the front's bottom edge: a half-made seam
    assert!(h.state().can_undo());
    cmd(&mut h, Key::Z);
    assert_eq!(
        seams(&h).len(),
        1,
        "the earlier seam is not what Undo takes"
    );
    // The half-made seam is gone: this click starts a seam from the back's bottom edge...
    click(&mut h, 650.0, 99.0);
    assert_eq!(seams(&h).len(), 1);
    // ... which this one finishes.
    click(&mut h, 250.0, 99.0);
    let made = seams(&h);
    assert_eq!(made.len(), 2);
    assert_eq!((made[1].a.shape, made[1].b.shape), (back, front));
    // With nothing half made, Undo takes the seam, one step.
    cmd(&mut h, Key::Z);
    assert_eq!(seams(&h).len(), 1);
}

#[test]
fn a_half_made_seam_is_something_to_undo() {
    // A project with no history, so only the half-made seam can be undone.
    let mut project = opendrape_core::Project::new();
    for x in [100.0, 600.0] {
        project.add_piece(Piece::rectangle(
            PieceId(0),
            "Piece",
            Point2::new(x, 100.0),
            300.0,
            400.0,
        ));
    }
    let mut h = harness();
    h.state_mut().set_project(project, None);
    h.state_mut().fit();
    h.run();
    key(&mut h, Key::W);
    assert!(!h.state().can_undo());
    click(&mut h, 401.0, 150.0);
    assert!(h.state().can_undo(), "the half-made seam");
    cmd(&mut h, Key::Z);
    assert!(!h.state().can_undo());
    assert!(seams(&h).is_empty());
}

#[test]
fn redo_drops_a_half_made_seam_and_redoes() {
    let mut h = harness();
    let front = with_rectangle(&mut h);
    let back = with_back(&mut h);
    let seam = sew(
        &mut h,
        side(front, Half::Drawn, 1, 1, true),
        side(back, Half::Drawn, 3, 1, false),
    );
    cmd(&mut h, Key::Z); // the Edit tool: the seam goes
    assert!(seams(&h).is_empty());
    key(&mut h, Key::W);
    click(&mut h, 250.0, 99.0); // the front's bottom edge: a half-made seam
    h.key_press_modifiers(Modifiers::COMMAND | Modifiers::SHIFT, Key::Z);
    h.run();
    assert!(seam_of(&h, seam).is_some(), "redone");
    // The half-made seam did not survive it: this click starts a seam from the back's bottom
    // edge, and the next finishes it.
    click(&mut h, 650.0, 99.0);
    assert_eq!(seams(&h).len(), 1);
    click(&mut h, 250.0, 99.0);
    let made = seams(&h);
    assert_eq!(made.len(), 2);
    assert_eq!((made[1].a.shape, made[1].b.shape), (back, front));
}

#[test]
fn a_change_to_the_outline_drops_a_half_made_seam() {
    let mut h = harness();
    let front = with_rectangle(&mut h);
    let back = with_back(&mut h);
    key(&mut h, Key::W);
    click(&mut h, 401.0, 150.0); // the front's right edge, edge 1 of 4: half made
    h.get_by_label_contains("Click the edge to sew it to");
    // The bottom edge gains a point (a panel, an undo, a redo: anything that renumbers the
    // edges): edge 1 is now the right half of the bottom edge.
    h.state_mut()
        .doc
        .edit(|p| opendrape_geom::split_edge_in(p, front, 0, 0.5));
    h.step();
    assert_eq!(piece_of(&h, front).len(), 5);
    // The hint, which is drawn before the canvas, already knows.
    h.get_by_label_contains("Click an edge to start a seam");
    click(&mut h, 599.0, 150.0); // starts a new seam; it does not finish the old one
    assert!(seams(&h).is_empty(), "no seam on the wrong edge");
    click(&mut h, 401.0, 150.0); // the front's right edge, now edge 2
    let made = seams(&h);
    assert_eq!(made.len(), 1);
    assert_eq!((made[0].a.shape, made[0].a.first_edge), (back, 3));
    assert_eq!((made[0].b.shape, made[0].b.first_edge), (front, 2));
}

#[test]
fn a_seam_whose_mirror_image_would_sew_a_sewn_edge_says_so() {
    let mut h = harness();
    with_half(&mut h); // drawn x 300..450, pale 150..300; edge 3 is the fold
    let back = with_back(&mut h); // 600..900
    let twin = h
        .state_mut()
        .doc
        .edit(|p| p.add_twin(back, "Back (mirror)".into(), Point2::new(1900.0, 0.0)))
        .unwrap();
    let sleeve = h.state_mut().doc.edit(|p| {
        p.add_piece(Piece::rectangle(
            PieceId(0),
            "Sleeve",
            Point2::new(600.0, -400.0),
            300.0,
            200.0,
        ))
    });
    // The twin's bottom edge is sewn to the sleeve's top. The sleeve has no mirror image, so
    // neither has this seam: the back's own bottom edge stays free.
    let first = sew(
        &mut h,
        side(twin, Half::Drawn, 0, 1, true),
        side(sleeve, Half::Drawn, 2, 1, true),
    );
    h.state_mut().fit();
    h.run();
    assert_eq!(h.state().doc.project().all_seams().len(), 1);
    key(&mut h, Key::W);
    click(&mut h, 650.0, 99.0); // the back's bottom edge, free
    click(&mut h, 451.0, 300.0); // the front's right edge, free: but the seam's mirror image
    //                              would sew the twin's bottom edge, which is sewn
    assert_eq!(
        notice(&h).as_deref(),
        Some("That seam's mirror image would sew an edge that is already sewn.")
    );
    assert_eq!(seams(&h).len(), 1, "nothing was sewn");
    assert!(seam_of(&h, first).is_some());
}

#[test]
fn the_fold_line_cannot_be_sewn_and_says_so() {
    let mut h = harness();
    with_half(&mut h); // the fold is the line x = 300, y 100..400
    with_back(&mut h);
    key(&mut h, Key::W);
    click(&mut h, 225.0, 250.0); // inside the pale half, away from every edge
    assert_eq!(notice(&h), None);
    click(&mut h, 300.0, 250.0); // on the fold line
    assert_eq!(
        notice(&h).as_deref(),
        Some("The fold line is inside the piece and can't be sewn.")
    );
    assert!(seams(&h).is_empty());
    // The notice is for the fold line only; the next click starts a seam as usual.
    click(&mut h, 451.0, 300.0); // the drawn half's right edge
    assert_eq!(notice(&h), None);
    click(&mut h, 599.0, 150.0);
    assert_eq!(seams(&h).len(), 1);
}

/// The front's right edge (400 mm) sewn to the left edge of a back `height` mm tall.
fn sewn_pair(h: &mut H, height: f64) -> (PieceId, PieceId, SeamId) {
    let front = with_rectangle(h);
    let back = h.state_mut().doc.edit(|p| {
        p.add_piece(Piece::rectangle(
            PieceId(0),
            "Back",
            Point2::new(600.0, 100.0),
            300.0,
            height,
        ))
    });
    let seam = sew(
        h,
        side(front, Half::Drawn, 1, 1, true),
        side(back, Half::Drawn, 3, 1, false),
    );
    (front, back, seam)
}

/// Where a seam's line runs inside the front's right edge (x = 400): 5 screen points in.
fn on_seam_line(h: &H) -> f64 {
    400.0 - 5.0 / h.state().view.zoom
}

#[test]
fn clicking_a_seam_line_selects_the_seam_and_shows_both_lengths() {
    let mut h = harness();
    let (front, _, seam) = sewn_pair(&mut h, 400.0);
    let x = on_seam_line(&h);
    click(&mut h, x, 300.0);
    assert_eq!(h.state().selection, Selection::Seam(seam));
    h.get_by_label("Seam 1");
    h.get_by_label("Side 1: 40.0 cm");
    h.get_by_label("Side 2: 40.0 cm");
    assert!(h.query_by_label_contains("Lengths differ").is_none());
    // On the outline itself, the edge wins.
    click(&mut h, 400.0, 300.0);
    assert_eq!(h.state().selection, Selection::Edge(front, 1));
}

#[test]
fn sides_more_than_3_mm_apart_in_length_are_flagged() {
    let mut h = harness();
    let (_, _, seam) = sewn_pair(&mut h, 440.0);
    h.state_mut().selection = Selection::Seam(seam);
    h.run();
    h.get_by_label("Lengths differ by 4.0 cm");
    let mut close = harness();
    let (_, _, seam) = sewn_pair(&mut close, 402.0);
    close.state_mut().selection = Selection::Seam(seam);
    close.run();
    assert!(
        close.query_by_label_contains("Lengths differ").is_none(),
        "2 mm is fine"
    );
}

#[test]
fn flip_turns_the_second_side_round_as_one_step() {
    let mut h = harness();
    let (_, back, seam) = sewn_pair(&mut h, 400.0);
    h.state_mut().selection = Selection::Seam(seam);
    h.run();
    h.get_by_label("Flip").click();
    h.run();
    assert_eq!(
        seam_of(&h, seam).unwrap().b,
        side(back, Half::Drawn, 3, 1, true)
    );
    cmd(&mut h, Key::Z);
    assert_eq!(
        seam_of(&h, seam).unwrap().b,
        side(back, Half::Drawn, 3, 1, false)
    );
}

#[test]
fn delete_seam_takes_its_mirror_image_too() {
    let mut h = harness();
    let front = with_half(&mut h);
    let back = with_back(&mut h);
    h.state_mut()
        .doc
        .edit(|p| p.add_twin(back, "Back (mirror)".into(), Point2::new(1900.0, 0.0)));
    let seam = sew(
        &mut h,
        side(front, Half::Drawn, 1, 1, true),
        side(back, Half::Drawn, 3, 1, false),
    );
    assert_eq!(h.state().doc.project().all_seams().len(), 2);
    h.state_mut().selection = Selection::Seam(seam);
    h.run();
    h.get_by_label("Delete seam").click();
    h.run();
    assert!(h.state().doc.project().all_seams().is_empty());
    assert_eq!(h.state().selection, Selection::None);
}

#[test]
fn the_sew_tool_picks_a_seam_by_its_line_and_delete_removes_it() {
    let mut h = harness();
    let (_, _, seam) = sewn_pair(&mut h, 400.0);
    key(&mut h, Key::W);
    let x = on_seam_line(&h);
    click(&mut h, x, 300.0);
    assert_eq!(h.state().selection, Selection::Seam(seam));
    key(&mut h, Key::Delete);
    assert!(seams(&h).is_empty());
}

#[test]
fn seams_draw_on_halves_twins_and_round_corners() {
    let mut h = harness();
    let front = with_half(&mut h);
    let back = with_back(&mut h);
    let twin = h
        .state_mut()
        .doc
        .edit(|p| p.add_twin(back, "Back (mirror)".into(), Point2::new(1900.0, 0.0)))
        .unwrap();
    // Two edges round a corner of the pale half, and a twin's edges wrapping past its last.
    let seam = sew(
        &mut h,
        side(front, Half::Pale, 0, 2, false),
        side(twin, Half::Drawn, 3, 2, true),
    );
    for selection in [Selection::Seam(seam), Selection::None] {
        h.state_mut().selection = selection;
        h.run();
    }
    assert_eq!(h.state().doc.project().all_seams().len(), 2);
}
