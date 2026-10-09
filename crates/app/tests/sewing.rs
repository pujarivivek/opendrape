//! M4a on the pattern table: seams sewn with the Sew tool, how edits keep them, and the seam
//! panel, driven the way a student would.

mod common;
use common::*;
use egui::Key;
use egui_kittest::kittest::Queryable;
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
