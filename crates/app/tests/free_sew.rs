//! M4b on the pattern table: the Free Sew tool (F), driven the way a student would.

mod common;
use common::*;
use egui::Key;
use egui_kittest::kittest::Queryable;
use opendrape::editor::{Selection, Tool};
use opendrape_core::{Edge, Half, Notch, OutlinePos, Piece, PieceId, Point2, Seam, SeamSide};

fn p(x: f64, y: f64) -> Point2 {
    Point2::new(x, y)
}

/// A sleeve: hem (20,0)–(320,0), underarm edges up to (340,130) and (0,130), and its cap (edge
/// 2, from (340,130) round to (0,130)) curved up to a notch at its middle, (170, 220). With its
/// mirror image when `paired`.
fn with_sleeve(h: &mut H, paired: bool) -> (PieceId, Option<PieceId>) {
    let sleeve = h.state_mut().doc.edit(|pr| {
        let mut s = Piece::polygon(
            PieceId(0),
            "Sleeve",
            &[p(20.0, 0.0), p(320.0, 0.0), p(340.0, 130.0), p(0.0, 130.0)],
        );
        s.edges[2] = Edge::Curve {
            c1: p(290.0, 250.0),
            c2: p(50.0, 250.0),
        };
        let cap = opendrape_geom::edge_length(&s, 2);
        s.notches = vec![Notch::new(2, cap / 2.0)];
        pr.add_piece(s)
    });
    let twin = paired.then(|| {
        h.state_mut()
            .doc
            .edit(|pr| pr.add_twin(sleeve, "Sleeve (mirror)".into(), p(1700.0, 0.0)))
            .unwrap()
    });
    h.run();
    (sleeve, twin)
}

/// A 250 × 400 mm front at (500,0): edges 0 bottom, 1 right (the armhole), 2 top, 3 left; cut
/// on the fold (its left edge, x = 500) when `folded`.
fn with_front(h: &mut H, folded: bool) -> PieceId {
    let id = h.state_mut().doc.edit(|pr| {
        let mut f = Piece::rectangle(PieceId(0), "Front", p(500.0, 0.0), 250.0, 400.0);
        f.fold = folded.then_some(3);
        pr.add_piece(f)
    });
    h.state_mut().fit();
    h.run();
    id
}

fn seams(h: &H) -> Vec<Seam> {
    h.state().doc.project().seams.clone()
}

fn notice(h: &H) -> Option<String> {
    h.state().notice.clone()
}

/// Half the cap: from the sleeve's left underarm corner (0,130), back along the cap to its
/// notch.
fn cap_half(sleeve: PieceId) -> SeamSide {
    SeamSide {
        shape: sleeve,
        half: Half::Drawn,
        from: OutlinePos::new(2, 1.0),
        to: OutlinePos::new(2, 0.5),
        forward: false,
    }
}

#[test]
fn four_clicks_sew_half_a_sleeve_cap_into_an_armhole() {
    let mut h = harness();
    let (sleeve, _) = with_sleeve(&mut h, false);
    let front = with_front(&mut h, false);
    key(&mut h, Key::F);
    assert_eq!(h.state().tool, Tool::FreeSew);
    h.get_by_label_contains("Click where the seam starts");
    click(&mut h, 0.0, 130.0); // the cap's left end
    click(&mut h, 172.0, 221.0); // near the notch: it snaps there
    assert!(seams(&h).is_empty(), "one side so far");
    click(&mut h, 750.0, 0.0); // the front's right edge, from its bottom...
    click(&mut h, 750.0, 400.0); // ...to its top: the seam
    assert_eq!(
        seams(&h),
        vec![Seam {
            id: opendrape_core::SeamId(1),
            a: cap_half(sleeve),
            b: SeamSide::edges(front, Half::Drawn, 1, 1, true),
        }]
    );
    assert_eq!(
        h.state().selection,
        Selection::Seam(opendrape_core::SeamId(1))
    );
    cmd(&mut h, Key::Z);
    assert!(seams(&h).is_empty(), "one undo step");
}

#[test]
fn shift_takes_the_long_way_round() {
    let mut h = harness();
    let (sleeve, _) = with_sleeve(&mut h, false);
    with_front(&mut h, false);
    key(&mut h, Key::F);
    click(&mut h, 0.0, 130.0);
    shift_click(&mut h, 172.0, 221.0);
    click(&mut h, 750.0, 0.0);
    click(&mut h, 750.0, 400.0);
    // From the left underarm corner down the left underarm, along the hem, up the right one
    // and round the cap's right half to the notch.
    let a = seams(&h)[0].a;
    assert_eq!(
        (a.from, a.to, a.forward),
        (OutlinePos::new(3, 0.0), OutlinePos::new(2, 0.5), true)
    );
    assert_eq!(a.shape, sleeve);
}

#[test]
fn the_second_half_of_the_cap_meets_the_first_at_the_notch() {
    let mut h = harness();
    let (sleeve, _) = with_sleeve(&mut h, false);
    let front = with_front(&mut h, false);
    let back = h.state_mut().doc.edit(|pr| {
        pr.add_piece(Piece::rectangle(
            PieceId(0),
            "Back",
            p(900.0, 0.0),
            250.0,
            400.0,
        ))
    });
    h.state_mut().fit();
    h.run();
    key(&mut h, Key::F);
    for (x, y) in [(0.0, 130.0), (170.0, 220.0), (750.0, 0.0), (750.0, 400.0)] {
        click(&mut h, x, y);
    }
    // On from the notch to the cap's right end, to the back's left edge from its top down.
    for (x, y) in [(170.0, 220.0), (340.0, 130.0), (900.0, 400.0), (900.0, 0.0)] {
        click(&mut h, x, y);
    }
    assert_eq!(notice(&h), None, "they meet at the notch: no overlap");
    let made = seams(&h);
    assert_eq!(made.len(), 2);
    assert_eq!(made[1].a.from, OutlinePos::new(2, 0.5), "from the notch");
    assert_eq!(
        made[1].a.to,
        OutlinePos::new(2, 0.0),
        "to the cap's right end"
    );
    assert_eq!(made[1].b, SeamSide::edges(back, Half::Drawn, 3, 3, true));
    assert_eq!(made[0].b.shape, front);
    assert_eq!(made[1].a.shape, sleeve);
}

#[test]
fn a_side_over_a_sewn_stretch_or_across_the_fold_is_refused() {
    let mut h = harness();
    with_sleeve(&mut h, false);
    with_front(&mut h, true); // folded on its left edge: its pale half covers x 250..500
    key(&mut h, Key::F);
    for (x, y) in [(0.0, 130.0), (170.0, 220.0), (750.0, 0.0), (750.0, 400.0)] {
        click(&mut h, x, y);
    }
    assert_eq!(seams(&h).len(), 1);
    // Part of the cap again (from three quarters of the way along it to its left end):
    // refused.
    let on_cap = {
        let sleeve = &h.state().doc.project().pieces[0];
        let cap = opendrape_geom::edge_length(sleeve, 2);
        opendrape_geom::point_at_distance(sleeve, 2, 0.75 * cap)
    };
    click(&mut h, on_cap.x, on_cap.y);
    click(&mut h, 0.0, 130.0);
    assert_eq!(notice(&h).as_deref(), Some("Part of this is already sewn."));
    key(&mut h, Key::Escape);
    // The fold line itself.
    click(&mut h, 500.0, 200.0);
    assert_eq!(
        notice(&h).as_deref(),
        Some("The fold line is inside the piece and can't be sewn.")
    );
    // A side that ends on another piece.
    click(&mut h, 20.0, 0.0);
    click(&mut h, 600.0, 0.0);
    assert_eq!(
        notice(&h).as_deref(),
        Some("End the side on the same piece (and the same half) it started on.")
    );
    assert_eq!(seams(&h).len(), 1);
}

#[test]
fn mirror_images_of_free_seams_appear_by_themselves() {
    let mut h = harness();
    let (sleeve, twin) = with_sleeve(&mut h, true);
    let front = with_front(&mut h, true);
    key(&mut h, Key::F);
    for (x, y) in [(0.0, 130.0), (170.0, 220.0), (750.0, 0.0), (750.0, 400.0)] {
        click(&mut h, x, y);
    }
    let all = h.state().doc.project().all_seams();
    assert_eq!(all.len(), 2);
    let (mirror, derived) = all[1];
    assert!(derived);
    assert_eq!(
        mirror.a,
        SeamSide {
            shape: twin.unwrap(),
            ..cap_half(sleeve)
        }
    );
    assert_eq!(mirror.b, SeamSide::edges(front, Half::Pale, 1, 1, true));
    // The twin's cap half is sewn now: picking it again is refused.
    let twin_left_end = 1700.0; // the twin shows stored (x, y) at (1700 - x, y)
    click(&mut h, twin_left_end, 130.0);
    click(&mut h, 1700.0 - 172.0, 221.0);
    assert_eq!(notice(&h).as_deref(), Some("Part of this is already sewn."));
}

#[test]
fn escape_and_undo_cancel_a_half_made_free_seam() {
    let mut h = harness();
    with_sleeve(&mut h, false);
    with_front(&mut h, false);
    key(&mut h, Key::F);
    click(&mut h, 0.0, 130.0);
    click(&mut h, 170.0, 220.0);
    key(&mut h, Key::Escape);
    h.get_by_label_contains("Click where the seam starts");
    click(&mut h, 0.0, 130.0);
    assert!(h.state().can_undo(), "the half-made seam");
    cmd(&mut h, Key::Z);
    h.get_by_label_contains("Click where the seam starts");
    // A point added to the sleeve renumbers its edges: a half-made seam on it is dropped.
    click(&mut h, 0.0, 130.0);
    let sleeve = h.state().doc.project().pieces[0].id;
    h.state_mut()
        .doc
        .edit(|pr| opendrape_geom::split_edge_in(pr, sleeve, 0, 0.5));
    h.run();
    h.get_by_label_contains("Click where the seam starts");
}

#[test]
fn a_seam_line_is_picked_with_free_sew_and_delete_removes_it() {
    let mut h = harness();
    with_sleeve(&mut h, false);
    with_front(&mut h, false);
    key(&mut h, Key::F);
    for (x, y) in [(0.0, 130.0), (170.0, 220.0), (750.0, 0.0), (750.0, 400.0)] {
        click(&mut h, x, y);
    }
    h.state_mut().selection = Selection::None;
    h.run();
    // The seam's line runs 5 screen points inside the front's right edge.
    let x = 750.0 - 5.0 / h.state().view.zoom;
    click(&mut h, x, 200.0);
    assert!(matches!(h.state().selection, Selection::Seam(_)));
    key(&mut h, Key::Delete);
    assert!(seams(&h).is_empty());
}

#[test]
fn the_same_spot_clicked_for_both_ends_is_refused_not_sewn_all_the_way_round() {
    let mut h = harness();
    with_sleeve(&mut h, false);
    with_front(&mut h, false);
    key(&mut h, Key::F);
    // A corner twice (it is the end of one edge and the start of the next), and a notch twice.
    for (x, y) in [(340.0, 130.0), (170.0, 220.0)] {
        click(&mut h, x, y);
        click(&mut h, x, y);
        assert_eq!(
            notice(&h).as_deref(),
            Some("That side is too short to sew: pick points more than 1 mm apart.")
        );
        key(&mut h, Key::Escape);
    }
    assert!(seams(&h).is_empty());
}
