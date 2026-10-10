//! Where seam sides lie on the shapes that show them: which stretches of which outline edges,
//! which way, how long, as points to draw, and where their notches are; and which stored point
//! a point of a shape's outline is.

use crate::shapes::{Shape, ShapeKind};
use crate::{ACCURACY, edge_length, edge_points, edge_seg, flatten, point_at_distance};
use kurbo::{BezPath, ParamCurve, ParamCurveArclen, PathSeg};
use opendrape_core::{Half, MIN_SIDE_MM, OutlinePos, Point2, SeamSide};

impl Shape {
    /// How many edges the stored piece behind this shape has (for a fold, its half's).
    pub fn stored_len(&self) -> usize {
        match self.kind {
            ShapeKind::Folded { drawn, .. } => drawn,
            ShapeKind::Plain | ShapeKind::Twin { .. } => self.piece.len(),
        }
    }
    /// The stored edge outline edge `j` shows: its half, its index, and whether the outline
    /// edge runs against the stored edge's direction (only on the pale half of a fold). Every
    /// outline edge has one; a fold's own edge is inside the shape, not on its outline.
    pub fn sew_edge(&self, j: usize) -> (Half, usize, bool) {
        match self.kind {
            ShapeKind::Folded { first, drawn, .. } if j + 1 >= drawn => {
                // Pale edge j is the mirror image of drawn edge 2n - 3 - j.
                let m = 2 * drawn - 3 - j;
                (Half::Pale, (first + m) % drawn, true)
            }
            ShapeKind::Folded { first, drawn, .. } => (Half::Drawn, (first + j) % drawn, false),
            ShapeKind::Plain | ShapeKind::Twin { .. } => (Half::Drawn, j, false),
        }
    }
    /// The outline edge that shows stored edge `e` of `half`, and whether it runs against the
    /// stored edge. None for the fold edge, an edge the piece doesn't have, or the pale half of
    /// a shape that isn't folded.
    pub fn outline_edge(&self, half: Half, e: usize) -> Option<(usize, bool)> {
        match (self.kind, half) {
            (ShapeKind::Folded { first, drawn, .. }, half) => {
                if e >= drawn || e == (first + drawn - 1) % drawn {
                    return None;
                }
                let m = (e + drawn - first) % drawn;
                Some(match half {
                    Half::Drawn => (m, false),
                    Half::Pale => (2 * drawn - 3 - m, true),
                })
            }
            (_, Half::Pale) => None,
            (_, Half::Drawn) => (e < self.piece.len()).then_some((e, false)),
        }
    }
    /// Which stored point the point `d` mm along outline edge `j` (from its start) shows.
    pub fn outline_pos(&self, j: usize, d: f64) -> (Half, OutlinePos) {
        let (half, e, against) = self.sew_edge(j);
        let len = edge_length(&self.piece, j);
        let f = if len > 1e-12 {
            (d / len).clamp(0.0, 1.0)
        } else {
            0.0
        };
        (half, OutlinePos::new(e, if against { 1.0 - f } else { f }))
    }
    /// Where this shape shows stored point `pos` of `half` (None as for [`Self::outline_edge`]).
    pub fn point_at(&self, half: Half, pos: OutlinePos) -> Option<Point2> {
        let (j, against) = self.outline_edge(half, pos.edge)?;
        let f = if against { 1.0 - pos.t } else { pos.t };
        Some(point_at_distance(
            &self.piece,
            j,
            f * edge_length(&self.piece, j),
        ))
    }
}

/// A stretch of one outline edge a side covers: from `from` to `to` mm along the edge (from the
/// edge's start), in the order the side runs, so `from > to` when it runs against the edge.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Run {
    pub edge: usize,
    pub from: f64,
    pub to: f64,
}

impl Run {
    pub fn length(&self) -> f64 {
        (self.to - self.from).abs()
    }
}

/// The stretches of outline `side` covers on `shape`, in the order it runs. None when the side
/// does not fit the shape (a pale half on a shape that isn't folded, an edge that doesn't
/// exist, the fold edge, or nothing covered).
pub fn side_runs(shape: &Shape, side: &SeamSide) -> Option<Vec<Run>> {
    let spans = side.spans(shape.stored_len());
    if spans.is_empty() {
        return None;
    }
    spans
        .iter()
        .map(|s| {
            let (j, against) = shape.outline_edge(side.half, s.edge)?;
            let len = edge_length(&shape.piece, j);
            // The span's ends along the outline edge, nearer its start first.
            let (near, far) = if against {
                ((1.0 - s.t1) * len, (1.0 - s.t0) * len)
            } else {
                (s.t0 * len, s.t1 * len)
            };
            // A side running the stored way runs the outline edge's way unless that is reversed.
            Some(if side.forward != against {
                Run {
                    edge: j,
                    from: near,
                    to: far,
                }
            } else {
                Run {
                    edge: j,
                    from: far,
                    to: near,
                }
            })
        })
        .collect()
}

/// Length (mm) of a side along the stitching line.
pub fn side_length(shape: &Shape, side: &SeamSide) -> Option<f64> {
    Some(side_runs(shape, side)?.iter().map(Run::length).sum())
}

/// The side as points on the shape no further than `tolerance` mm from it, from its start to
/// its end.
pub fn side_points(shape: &Shape, side: &SeamSide, tolerance: f64) -> Option<Vec<Point2>> {
    let mut out: Vec<Point2> = Vec::new();
    for run in side_runs(shape, side)? {
        let mut pts = edge_points_between(
            &shape.piece,
            run.edge,
            run.from.min(run.to),
            run.from.max(run.to),
            tolerance,
        );
        if run.from > run.to {
            pts.reverse();
        }
        // Each stretch starts where the one before it ended.
        let skip = usize::from(!out.is_empty());
        out.extend(pts.into_iter().skip(skip));
    }
    Some(out)
}

/// Edge `j` from `d0` to `d1` mm along it (from its start), as points within `tolerance` mm.
pub fn edge_points_between(
    piece: &opendrape_core::Piece,
    j: usize,
    d0: f64,
    d1: f64,
    tolerance: f64,
) -> Vec<Point2> {
    let seg = edge_seg(piece, j);
    let len = seg.arclen(ACCURACY);
    // The ends of the edge are exact, so a stretch to a corner ends on the corner.
    let param = |d: f64| {
        if d <= 0.0 || len < 1e-12 {
            0.0
        } else if d >= len {
            1.0
        } else {
            seg.inv_arclen(d, ACCURACY)
        }
    };
    let (t0, t1) = (param(d0), param(d1));
    if t0 == 0.0 && t1 == 1.0 {
        return edge_points(piece, j, tolerance);
    }
    let sub = seg.subsegment(t0..t1);
    let mut path = BezPath::new();
    path.move_to(sub.start());
    match sub {
        PathSeg::Line(l) => path.line_to(l.p1),
        PathSeg::Cubic(c) => path.curve_to(c.p1, c.p2, c.p3),
        PathSeg::Quad(q) => path.quad_to(q.p1, q.p2),
    }
    flatten(&path, tolerance)
}

/// Where the shape's notches are along `side` (mm from its start, in order): those more than
/// [`MIN_SIDE_MM`] from both of its ends. A notch at either end marks where it meets another
/// seam, not a point inside it.
pub fn side_notches(shape: &Shape, side: &SeamSide) -> Option<Vec<f64>> {
    let runs = side_runs(shape, side)?;
    let total: f64 = runs.iter().map(Run::length).sum();
    let mut out = Vec::new();
    let mut start = 0.0;
    for run in &runs {
        let (lo, hi) = (run.from.min(run.to), run.from.max(run.to));
        for notch in shape.piece.notches.iter().filter(|n| n.edge == run.edge) {
            if (lo..=hi).contains(&notch.distance) {
                let at = start + (notch.distance - run.from).abs();
                if at > MIN_SIDE_MM && at < total - MIN_SIDE_MM {
                    out.push(at);
                }
            }
        }
        start += run.length();
    }
    out.sort_by(f64::total_cmp);
    // A notch on a corner inside the side is at the end of one stretch and the start of the next.
    out.dedup_by(|a, b| (*a - *b).abs() < 1e-9);
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shapes::shapes;
    use crate::unfolded;
    use opendrape_core::{Half, Notch, Piece, PieceId, Project, SeamSide};

    fn p(x: f64, y: f64) -> Point2 {
        Point2::new(x, y)
    }

    fn close(a: Point2, b: Point2) {
        assert!(a.distance(b) < 1e-9, "{a:?} vs {b:?}");
    }

    /// A front half folded on its left edge (id 1), and a back (id 2) with its twin (id 3).
    fn project() -> Project {
        let mut pr = Project::new();
        let mut front = Piece::rectangle(PieceId(0), "Front", p(0.0, 0.0), 100.0, 200.0);
        front.fold = Some(3);
        pr.add_piece(front);
        let back = pr.add_piece(Piece::rectangle(
            PieceId(0),
            "Back",
            p(300.0, 0.0),
            100.0,
            200.0,
        ));
        pr.add_twin(back, "Back (mirror)".into(), p(900.0, 0.0))
            .unwrap();
        pr
    }

    #[test]
    fn every_outline_edge_maps_to_a_stored_edge() {
        let all = shapes(&project());
        let front = &all[0];
        // The whole front's edges: 0 bottom, 1 right, 2 top, then the pale images of the top,
        // the right and the bottom, each running backwards.
        let mapped: Vec<_> = (0..6).map(|j| front.sew_edge(j)).collect();
        assert_eq!(
            mapped,
            vec![
                (Half::Drawn, 0, false),
                (Half::Drawn, 1, false),
                (Half::Drawn, 2, false),
                (Half::Pale, 2, true),
                (Half::Pale, 1, true),
                (Half::Pale, 0, true),
            ]
        );
        assert_eq!((front.stored_len(), all[2].stored_len()), (4, 4));
        assert_eq!(
            all[2].sew_edge(3),
            (Half::Drawn, 3, false),
            "a twin keeps its edges' direction"
        );
    }

    #[test]
    fn sides_run_from_their_start() {
        let all = shapes(&project());
        let front = &all[0];
        // Pale edges 0 and 1, run the stored way: from the mirror of (0,0) to the mirror of
        // (100,200), along whole-piece edges 5 then 4, each against its direction.
        let pale = SeamSide::edges(PieceId(1), Half::Pale, 0, 1, true);
        let run = |edge, from, to| Run { edge, from, to };
        assert_eq!(
            side_runs(front, &pale),
            Some(vec![run(5, 100.0, 0.0), run(4, 200.0, 0.0)])
        );
        let pts = side_points(front, &pale, 0.1).unwrap();
        assert_eq!(pts.len(), 3);
        close(pts[0], p(0.0, 0.0));
        close(pts[1], p(-100.0, 0.0));
        close(pts[2], p(-100.0, 200.0));
        assert!((side_length(front, &pale).unwrap() - 300.0).abs() < 1e-9);
        assert_eq!(
            side_runs(front, &pale.flipped()),
            Some(vec![run(4, 0.0, 200.0), run(5, 0.0, 100.0)])
        );
        // The fold edge (stored 3) can't be part of a side; nor can a pale half elsewhere.
        assert_eq!(
            side_runs(front, &SeamSide::edges(PieceId(1), Half::Drawn, 2, 3, true)),
            None
        );
        assert_eq!(
            side_runs(
                &all[1],
                &SeamSide::edges(PieceId(2), Half::Pale, 0, 0, true)
            ),
            None
        );
        // A twin side wraps like any other.
        let twin = SeamSide::edges(PieceId(3), Half::Drawn, 3, 0, false);
        assert_eq!(
            side_runs(&all[2], &twin),
            Some(vec![run(0, 100.0, 0.0), run(3, 200.0, 0.0)])
        );
    }

    /// A convex piece with six uneven edges, folded on edge `fold` (so a fold on any edge is
    /// valid), with a plain copy of it (id 2) to sew sides to.
    fn folded_hexagon(fold: usize) -> Project {
        let corners = [
            p(0.0, 0.0),
            p(120.0, -10.0),
            p(190.0, 60.0),
            p(160.0, 150.0),
            p(70.0, 190.0),
            p(-20.0, 100.0),
        ];
        let mut pr = Project::new();
        let mut cut = Piece::polygon(PieceId(0), "Cut", &corners);
        cut.fold = Some(fold);
        pr.add_piece(cut);
        pr.add_piece(Piece::polygon(PieceId(0), "Plain", &corners));
        assert_eq!(pr.check(), Ok(()), "fold on edge {fold}");
        pr
    }

    fn reflected_across(q: Point2, a: Point2, b: Point2) -> Point2 {
        let d = b - a;
        let t = ((q.x - a.x) * d.x + (q.y - a.y) * d.y) / (d.x * d.x + d.y * d.y);
        (a + d * t) * 2.0 - q
    }

    /// Every side the folded half can have, on every fold edge: its start and end points are
    /// the stored vertices it names (or their mirror images across the fold, on the pale half),
    /// run the way the side runs. Binds `side_edges` and `sew_edge` to the geometry, so a
    /// mapping that only works when the fold is the last edge cannot pass.
    #[test]
    fn sides_sit_on_the_stored_edges_whichever_edge_is_the_fold() {
        let n = 6;
        let mut checked = 0;
        for fold in 0..n {
            let pr = folded_hexagon(fold);
            let stored = &pr.pieces[0];
            let all = shapes(&pr);
            let shape = &all[0];
            let (near, far) = stored.edge_ends(fold);
            let vertex = |i: usize, half: Half| {
                let v = stored.vertices[i % n].pos;
                match half {
                    Half::Drawn => v,
                    Half::Pale => reflected_across(v, near, far),
                }
            };
            for half in [Half::Drawn, Half::Pale] {
                for first in 0..n {
                    for edges in 1..=n {
                        for forward in [true, false] {
                            let last = (first + edges - 1) % n;
                            let side = SeamSide::edges(PieceId(1), half, first, last, forward);
                            let got = side_points(shape, &side, 0.1);
                            if side.covers(n, fold) {
                                assert_eq!(got, None, "{side:?} crosses the fold edge {fold}");
                                continue;
                            }
                            let pts = got.unwrap_or_else(|| panic!("{side:?}, fold {fold}"));
                            let (lo, hi) = (vertex(first, half), vertex(first + edges, half));
                            let (start, end) = if forward { (lo, hi) } else { (hi, lo) };
                            close(pts[0], start);
                            close(pts[pts.len() - 1], end);
                            checked += 1;
                        }
                    }
                }
            }
            // One outline edge at a time: sew_edge names the stored edge, and the side built
            // from it covers exactly that outline edge, against its direction on the pale half.
            for j in 0..shape.piece.len() {
                let (half, i, against) = shape.sew_edge(j);
                let side = SeamSide::edges(PieceId(1), half, i, i, true);
                let len = crate::edge_length(&shape.piece, j);
                let (from, to) = if against { (len, 0.0) } else { (0.0, len) };
                assert_eq!(
                    side_runs(shape, &side),
                    Some(vec![Run { edge: j, from, to }]),
                    "outline edge {j}, fold {fold}"
                );
            }
        }
        assert!(checked > 300, "{checked} sides checked");
    }

    /// Unfolding renumbers every side and stores the mirror images; each side must end up on
    /// the same points as before, whichever edge was the fold.
    #[test]
    fn unfolding_leaves_every_side_where_it_was_whichever_edge_is_the_fold() {
        let n = 6;
        let ends = |pr: &Project| {
            let all = shapes(pr);
            let mut out: Vec<Vec<i64>> = pr
                .all_seams()
                .iter()
                .map(|(seam, _)| {
                    let mut key = Vec::new();
                    for side in [seam.a, seam.b] {
                        let shape = all.iter().find(|s| s.id == side.shape).unwrap();
                        for q in side_points(shape, &side, 0.1).unwrap() {
                            key.push((q.x * 1e6).round() as i64);
                            key.push((q.y * 1e6).round() as i64);
                        }
                        key.push(i64::MIN);
                    }
                    key
                })
                .collect();
            out.sort();
            out
        };
        let mut checked = 0;
        for fold in 0..n {
            for half in [Half::Drawn, Half::Pale] {
                for first in 0..n {
                    for edges in 1..n {
                        for forward in [true, false] {
                            // The other side: a plain piece's edge, or (below) the folded
                            // piece's own, so both sides are renumbered.
                            let last = (first + edges - 1) % n;
                            let a = SeamSide::edges(PieceId(1), half, first, last, forward);
                            let other =
                                |half, e, forward| SeamSide::edges(PieceId(1), half, e, e, forward);
                            let others = [
                                SeamSide::edges(PieceId(2), Half::Drawn, 1, 1, forward),
                                other(half.other(), (first + 3) % n, true),
                                other(half, (first + n - 1) % n, false),
                            ];
                            for b in others {
                                let mut pr = folded_hexagon(fold);
                                pr.add_seam(a, b);
                                if pr.check().is_err() {
                                    continue;
                                }
                                let before = ends(&pr);
                                let full = unfolded(&pr.pieces[0]);
                                assert!(pr.unfold_piece(PieceId(1), full));
                                assert_eq!(pr.check(), Ok(()), "{a:?} / {b:?}, fold {fold}");
                                assert_eq!(ends(&pr), before, "{a:?} / {b:?}, fold {fold}");
                                checked += 1;
                            }
                        }
                    }
                }
            }
        }
        assert!(checked > 500, "{checked} seams checked");
    }

    /// Fractions along stored edges, chosen to land inside, at and next to corners.
    const FRACTIONS: [f64; 5] = [0.0, 0.1, 0.5, 0.93, 1.0];

    #[test]
    fn free_sides_run_between_the_points_their_ends_name_on_every_kind_of_shape() {
        let n = 6;
        let mut checked = 0;
        for fold in [None, Some(2)] {
            let mut pr = folded_hexagon(fold.unwrap_or(0));
            if fold.is_none() {
                pr.pieces[0].fold = None;
                pr.add_twin(PieceId(2), "Twin".into(), p(600.0, 0.0))
                    .unwrap();
            }
            let all = shapes(&pr);
            for (shape, halves) in [
                (
                    &all[0],
                    if fold.is_some() {
                        &[Half::Drawn, Half::Pale][..]
                    } else {
                        &[Half::Drawn][..]
                    },
                ),
                (&all[all.len() - 1], &[Half::Drawn][..]),
            ] {
                for &half in halves {
                    for (e0, e1) in [(0, 0), (0, 1), (1, 4), (4, 1), (5, 0)] {
                        for &t0 in &FRACTIONS {
                            for &t1 in &FRACTIONS {
                                for forward in [true, false] {
                                    let side = SeamSide {
                                        shape: shape.id,
                                        half,
                                        from: opendrape_core::OutlinePos::new(e0, t0),
                                        to: opendrape_core::OutlinePos::new(e1, t1),
                                        forward,
                                    };
                                    let Some(points) = side_points(shape, &side, 0.1) else {
                                        let covers = |e| side.covers(n, e);
                                        assert!(
                                            side.spans(n).is_empty() || fold.is_some_and(covers),
                                            "{side:?} on {:?}",
                                            shape.kind
                                        );
                                        continue;
                                    };
                                    close(points[0], shape.point_at(half, side.from).unwrap());
                                    close(
                                        *points.last().unwrap(),
                                        shape.point_at(half, side.to).unwrap(),
                                    );
                                    let length: f64 = side
                                        .spans(n)
                                        .iter()
                                        .map(|s| {
                                            (s.t1 - s.t0)
                                                * crate::edge_length(&pr.pieces[0], s.edge)
                                        })
                                        .sum();
                                    assert!(
                                        (side_length(shape, &side).unwrap() - length).abs() < 1e-6,
                                        "{side:?}"
                                    );
                                    checked += 1;
                                }
                            }
                        }
                    }
                }
            }
        }
        assert!(checked > 400, "{checked} sides checked");
    }

    #[test]
    fn a_point_on_the_outline_names_the_stored_point_it_shows() {
        let mut pr = project();
        pr.pieces[1].edges[1] = opendrape_core::Edge::Curve {
            c1: p(450.0, 50.0),
            c2: p(380.0, 150.0),
        };
        let all = shapes(&pr);
        for shape in &all {
            for j in 0..shape.piece.len() {
                let len = crate::edge_length(&shape.piece, j);
                for f in [0.0, 0.3, 1.0] {
                    let (half, pos) = shape.outline_pos(j, f * len);
                    let on_shape = crate::point_at_distance(&shape.piece, j, f * len);
                    close(shape.point_at(half, pos).unwrap(), on_shape);
                }
            }
        }
        // The pale half's edges run against the stored ones.
        let (half, pos) = all[0].outline_pos(5, 25.0);
        assert_eq!((half, pos.edge), (Half::Pale, 0));
        assert!((pos.t - 0.75).abs() < 1e-12);
    }

    #[test]
    fn a_side_knows_the_notches_inside_it() {
        let mut pr = project();
        // The back's left edge (stored 3, running down from (300,200) to (300,0)) has notches 50
        // and 100 mm from its start, and its top edge one at 99.5 mm (0.5 mm from its end).
        pr.pieces[1].notches = vec![
            Notch::new(3, 50.0),
            Notch::new(3, 100.0),
            Notch::new(2, 99.5),
        ];
        let all = shapes(&pr);
        let back = &all[1];
        // Up the left edge, then on along the top: the notches at 150 and 100 mm from the
        // bottom-left corner; the one 0.5 mm from the top-left corner is inside too.
        let up = SeamSide::edges(PieceId(2), Half::Drawn, 2, 3, false);
        let got = side_notches(back, &up).unwrap();
        assert_eq!(got.len(), 3, "{got:?}");
        assert!((got[0] - 100.0).abs() < 1e-6 && (got[1] - 150.0).abs() < 1e-6);
        assert!((got[2] - 200.5).abs() < 1e-6);
        // A notch less than 1 mm from an end is where that side meets another, not inside it.
        let top = SeamSide::edges(PieceId(2), Half::Drawn, 2, 2, true);
        assert_eq!(side_notches(back, &top), Some(vec![]));
        // The twin has them too, at the same places along the same side.
        let twin_up = SeamSide {
            shape: PieceId(3),
            ..up
        };
        assert_eq!(side_notches(&all[2], &twin_up), Some(got));
    }

    #[test]
    fn unfolding_leaves_free_sides_where_they_were() {
        let mut checked = 0;
        for fold in 0..6 {
            for half in [Half::Drawn, Half::Pale] {
                for (t0, t1) in [(0.2, 0.7), (0.9, 0.3)] {
                    let mut pr = folded_hexagon(fold);
                    // From part-way along the edge after the fold to part-way along the one
                    // after that, sewn to part of the plain copy's edge 1.
                    let e = (fold + 1) % 6;
                    let a = SeamSide {
                        shape: PieceId(1),
                        half,
                        from: opendrape_core::OutlinePos::new(e, t0),
                        to: opendrape_core::OutlinePos::new((e + 1) % 6, t1),
                        forward: true,
                    };
                    let b = SeamSide {
                        from: opendrape_core::OutlinePos::new(1, 0.25),
                        to: opendrape_core::OutlinePos::new(1, 0.75),
                        ..SeamSide::edges(PieceId(2), Half::Drawn, 1, 1, true)
                    };
                    pr.add_seam(a, b);
                    assert_eq!(pr.check(), Ok(()));
                    let ends = |pr: &Project| {
                        let all = shapes(pr);
                        let side = pr.seams[0].a;
                        let pts = side_points(&all[0], &side, 0.1).unwrap();
                        (pts[0], *pts.last().unwrap())
                    };
                    let before = ends(&pr);
                    let full = unfolded(&pr.pieces[0]);
                    assert!(pr.unfold_piece(PieceId(1), full));
                    assert_eq!(pr.check(), Ok(()));
                    let after = ends(&pr);
                    close(before.0, after.0);
                    close(before.1, after.1);
                    checked += 1;
                }
            }
        }
        assert_eq!(checked, 24);
    }
}
