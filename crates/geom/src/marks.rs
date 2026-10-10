//! Notches and internal lines as drawable geometry, and where edge labels go.

use crate::{ACCURACY, edge_length, edge_seg, flatten, kp, outline_points};
use kurbo::{BezPath, CubicBez, Line, ParamCurve, ParamCurveArclen, ParamCurveNearest, PathSeg};
use opendrape_core::{Edge, InternalLine, Notch, NotchStyle, Piece, Point2};

/// How deep a notch is cut (mm), or 60 % of a shallower allowance.
pub const NOTCH_DEPTH_MM: f64 = 5.0;
/// Gap between the marks of a double or triple notch (mm).
pub const NOTCH_SPACING_MM: f64 = 3.0;
/// Half the width of a V notch at the cut line (mm).
const V_HALF_WIDTH_MM: f64 = 1.5;

/// Whether the outline runs counter-clockwise (y up). A twin's outline runs the other way.
pub fn is_counter_clockwise(piece: &Piece) -> bool {
    let pts = outline_points(piece, 0.5);
    (0..pts.len())
        .map(|k| {
            let (a, b) = (pts[k], pts[(k + 1) % pts.len()]);
            a.x * b.y - b.x * a.y
        })
        .sum::<f64>()
        > 0.0
}

/// Unit normal pointing out of the piece for direction `d` along its outline.
fn outward(piece_ccw: bool, d: Point2) -> Point2 {
    let len = d.length().max(1e-12);
    let n = if piece_ccw {
        Point2::new(d.y, -d.x)
    } else {
        Point2::new(-d.y, d.x)
    };
    n * (1.0 / len)
}

/// Point and unit direction `distance` mm along edge `edge` (clamped to the edge).
fn along(piece: &Piece, edge: usize, distance: f64) -> (Point2, Point2) {
    let seg = edge_seg(piece, edge);
    let len = seg.arclen(ACCURACY);
    let t = if len < 1e-12 {
        0.0
    } else {
        seg.inv_arclen(distance.clamp(0.0, len), ACCURACY)
    };
    let (a, b) = (seg.eval((t - 1e-4).max(0.0)), seg.eval((t + 1e-4).min(1.0)));
    let d = Point2::new(b.x - a.x, b.y - a.y);
    let p = seg.eval(t);
    (Point2::new(p.x, p.y), d * (1.0 / d.length().max(1e-12)))
}

/// The short lines that draw `notch`: on the cut line and pointing inwards (on the stitching
/// line when the edge has no allowance), one line per mark for a slit and two for a V.
pub fn notch_marks(piece: &Piece, notch: &Notch) -> Vec<[Point2; 2]> {
    marks_of(piece, notch, is_counter_clockwise(piece), true)
}

/// [`notch_marks`] for every notch of `piece` in order, working out which way the outline
/// winds once instead of once per notch (that takes a pass over the whole outline).
pub fn all_notch_marks(piece: &Piece) -> Vec<Vec<[Point2; 2]>> {
    let ccw = is_counter_clockwise(piece);
    piece
        .notches
        .iter()
        .map(|n| marks_of(piece, n, ccw, true))
        .collect()
}

/// The lines that draw `notch` while the seam allowance is not shown: as for an edge with no
/// allowance, on the stitching line and [`NOTCH_DEPTH_MM`] deep, pointing in.
pub fn notch_marks_on_stitching(piece: &Piece, notch: &Notch) -> Vec<[Point2; 2]> {
    marks_of(piece, notch, is_counter_clockwise(piece), false)
}

/// [`notch_marks_on_stitching`] for every notch of `piece` in order, winding worked out once.
pub fn all_notch_marks_on_stitching(piece: &Piece) -> Vec<Vec<[Point2; 2]>> {
    let ccw = is_counter_clockwise(piece);
    piece
        .notches
        .iter()
        .map(|n| marks_of(piece, n, ccw, false))
        .collect()
}

/// The marks of one notch, given which way the outline winds; `on_cut_line` false puts them
/// on the stitching line, as for no allowance.
fn marks_of(piece: &Piece, notch: &Notch, ccw: bool, on_cut_line: bool) -> Vec<[Point2; 2]> {
    let w = if on_cut_line {
        piece.edge_allowance(notch.edge)
    } else {
        0.0
    };
    let depth = if w > 0.0 {
        NOTCH_DEPTH_MM.min(0.6 * w)
    } else {
        NOTCH_DEPTH_MM
    };
    let marks = notch.marks.max(1);
    let mut lines = Vec::new();
    for k in 0..marks {
        let shift = (f64::from(k) - f64::from(marks - 1) / 2.0) * NOTCH_SPACING_MM;
        let (p, d) = along(piece, notch.edge, notch.distance + shift);
        let n = outward(ccw, d);
        let base = p + n * w;
        let tip = base - n * depth;
        match notch.style {
            NotchStyle::Slit => lines.push([base, tip]),
            NotchStyle::V => {
                lines.push([base + d * V_HALF_WIDTH_MM, tip]);
                lines.push([base - d * V_HALF_WIDTH_MM, tip]);
            }
        }
    }
    lines
}

/// How far (mm) curve parameter `t` is along edge `edge` from its start.
pub fn distance_along(piece: &Piece, edge: usize, t: f64) -> f64 {
    edge_seg(piece, edge)
        .subsegment(0.0..t.clamp(0.0, 1.0))
        .arclen(ACCURACY)
}

/// The point `distance` mm along edge `edge` from its start (clamped to the edge).
pub fn point_at_distance(piece: &Piece, edge: usize, distance: f64) -> Point2 {
    along(piece, edge, distance).0
}

fn line_seg(line: &InternalLine, i: usize) -> PathSeg {
    let (a, b) = line.edge_ends(i);
    match line.edges[i] {
        Edge::Line => PathSeg::Line(Line::new(kp(a), kp(b))),
        Edge::Curve { c1, c2 } => PathSeg::Cubic(CubicBez::new(kp(a), kp(c1), kp(c2), kp(b))),
    }
}

/// Points along an internal line within `tolerance` mm: from its start to its end, and for a
/// closed line back to the start again.
pub fn line_points(line: &InternalLine, tolerance: f64) -> Vec<Point2> {
    let mut path = BezPath::new();
    path.move_to(kp(line.vertices[0].pos));
    for i in 0..line.edge_count() {
        match line_seg(line, i) {
            PathSeg::Line(l) => path.line_to(l.p1),
            PathSeg::Cubic(c) => path.curve_to(c.p1, c.p2, c.p3),
            PathSeg::Quad(q) => path.quad_to(q.p1, q.p2),
        }
    }
    flatten(&path, tolerance)
}

/// Total length (mm) of an internal line.
pub fn line_length(line: &InternalLine) -> f64 {
    (0..line.edge_count())
        .map(|i| line_seg(line, i).arclen(ACCURACY))
        .sum()
}

/// The internal line of `piece` nearest to `p`: (line, edge, curve parameter, distance mm).
pub fn nearest_line(piece: &Piece, p: Point2) -> Option<(usize, usize, f64, f64)> {
    piece
        .lines
        .iter()
        .enumerate()
        .flat_map(|(l, line)| {
            (0..line.edge_count()).map(move |i| {
                let n = line_seg(line, i).nearest(kp(p), ACCURACY);
                (l, i, n.t, n.distance_sq.sqrt())
            })
        })
        .min_by(|a, b| a.3.total_cmp(&b.3))
}

/// Edge `i`'s label anchor, given which way the outline winds.
fn label_anchor(piece: &Piece, i: usize, ccw: bool) -> (Point2, Point2) {
    let half = edge_length(piece, i) / 2.0;
    let (p, d) = along(piece, i, half);
    (p, outward(ccw, d))
}

/// Where edge `i`'s length label goes: the point halfway along the edge, and the unit normal
/// there pointing out of the piece.
pub fn edge_label_anchor(piece: &Piece, i: usize) -> (Point2, Point2) {
    label_anchor(piece, i, is_counter_clockwise(piece))
}

/// [`edge_label_anchor`] for every edge in order, working out which way the outline winds
/// once instead of once per edge (that takes a pass over the whole outline).
pub fn edge_label_anchors(piece: &Piece) -> Vec<(Point2, Point2)> {
    let ccw = is_counter_clockwise(piece);
    (0..piece.len())
        .map(|i| label_anchor(piece, i, ccw))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use opendrape_core::{Edge, NotchStyle, PieceId, Point2};

    fn p(x: f64, y: f64) -> Point2 {
        Point2::new(x, y)
    }

    fn square() -> Piece {
        Piece::rectangle(PieceId(1), "S", p(0.0, 0.0), 100.0, 100.0)
    }

    fn close(a: Point2, b: Point2) {
        assert!(a.distance(b) < 1e-6, "{a:?} vs {b:?}");
    }

    #[test]
    fn a_slit_sits_on_the_cut_line_and_points_in() {
        let marks = notch_marks(&square(), &Notch::new(0, 40.0));
        assert_eq!(marks.len(), 1);
        close(marks[0][0], p(40.0, -10.0)); // on the cut line, 10 mm out
        close(marks[0][1], p(40.0, -5.0)); // 5 mm deep
    }

    #[test]
    fn marks_are_spaced_and_vs_have_two_lines() {
        let double = Notch {
            marks: 2,
            ..Notch::new(0, 40.0)
        };
        let m = notch_marks(&square(), &double);
        close(m[0][0], p(38.5, -10.0));
        close(m[1][0], p(41.5, -10.0));
        let v = Notch {
            style: NotchStyle::V,
            ..Notch::new(0, 40.0)
        };
        let m = notch_marks(&square(), &v);
        assert_eq!(m.len(), 2);
        close(m[0][1], m[1][1]); // both legs meet at the tip
    }

    #[test]
    fn shallow_allowance_and_none() {
        let mut s = square();
        s.allowance = 4.0;
        let m = notch_marks(&s, &Notch::new(0, 40.0));
        close(m[0][1], p(40.0, -4.0 + 2.4)); // 60 % of 4 mm deep
        s.allowance = 0.0;
        let m = notch_marks(&s, &Notch::new(0, 40.0));
        close(m[0][0], p(40.0, 0.0));
        close(m[0][1], p(40.0, 5.0));
    }

    #[test]
    fn all_the_notches_at_once_are_the_ones_given_one_at_a_time() {
        let mut ccw = square();
        ccw.set_curved(1, true);
        ccw.set_handle(1, opendrape_core::HandleEnd::Start, p(140.0, 20.0));
        ccw.edge_props[0].allowance = Some(4.0);
        ccw.notches = vec![
            Notch::new(0, 40.0),
            Notch {
                marks: 3,
                ..Notch::new(1, 30.0)
            },
            Notch {
                style: NotchStyle::V,
                marks: 2,
                ..Notch::new(2, 60.0)
            },
            Notch::new(3, 10.0),
        ];
        let cw = ccw.reflected(p(300.0, 0.0));
        assert!(is_counter_clockwise(&ccw) && !is_counter_clockwise(&cw));
        for piece in [ccw, cw] {
            let all = all_notch_marks(&piece);
            let flat = all_notch_marks_on_stitching(&piece);
            assert_eq!((all.len(), flat.len()), (4, 4));
            for (k, notch) in piece.notches.iter().enumerate() {
                assert_eq!(all[k], notch_marks(&piece, notch), "notch {k}");
                assert_eq!(
                    flat[k],
                    notch_marks_on_stitching(&piece, notch),
                    "notch {k} on the stitching"
                );
            }
        }
        assert!(all_notch_marks(&square()).is_empty());
    }

    #[test]
    fn without_the_allowance_a_notch_sits_on_the_stitching_line_five_deep() {
        // 10 mm of allowance all round; edge 0 has 4 mm of its own.
        let mut s = square();
        s.edge_props[0].allowance = Some(4.0);
        s.notches = vec![Notch::new(0, 40.0), Notch::new(1, 40.0)];
        for notch in &s.notches {
            let on_cut = notch_marks(&s, notch);
            let on_seam = notch_marks_on_stitching(&s, notch);
            assert_ne!(on_cut, on_seam);
            let [a, b] = on_seam[0];
            assert!(((a - b).length() - NOTCH_DEPTH_MM).abs() < 1e-9);
            assert!(
                crate::nearest_edge(&s, a).unwrap().2 < 1e-9,
                "starts on the outline"
            );
            assert!(crate::contains(&s, b), "points in");
        }
        close(
            notch_marks_on_stitching(&s, &s.notches[0])[0][0],
            p(40.0, 0.0),
        );
        close(
            notch_marks_on_stitching(&s, &s.notches[0])[0][1],
            p(40.0, 5.0),
        );
        close(
            notch_marks_on_stitching(&s, &s.notches[1])[0][0],
            p(100.0, 40.0),
        );
        close(
            notch_marks_on_stitching(&s, &s.notches[1])[0][1],
            p(95.0, 40.0),
        );
    }

    #[test]
    fn a_notch_past_the_end_sits_at_the_end() {
        let m = notch_marks(&square(), &Notch::new(0, 250.0));
        close(m[0][0], p(100.0, -10.0));
    }

    #[test]
    fn distances_along_edges() {
        let s = square();
        assert!((distance_along(&s, 0, 0.25) - 25.0).abs() < 1e-6);
        close(point_at_distance(&s, 1, 30.0), p(100.0, 30.0));
    }

    #[test]
    fn internal_line_geometry() {
        let mut s = square();
        s.lines = vec![
            InternalLine::open(&[p(10.0, 20.0), p(50.0, 20.0), p(50.0, 50.0)]),
            InternalLine::polygon(&[p(60.0, 60.0), p(80.0, 60.0), p(70.0, 80.0)]),
        ];
        assert!((line_length(&s.lines[0]) - 70.0).abs() < 1e-6);
        let open = line_points(&s.lines[0], 0.1);
        close(open[0], p(10.0, 20.0));
        close(*open.last().unwrap(), p(50.0, 50.0));
        let closed = line_points(&s.lines[1], 0.1);
        close(*closed.last().unwrap(), p(60.0, 60.0)); // back to the start
        let (line, edge, t, d) = nearest_line(&s, p(30.0, 23.0)).unwrap();
        assert_eq!((line, edge), (0, 0));
        assert!((t - 0.5).abs() < 1e-6 && (d - 3.0).abs() < 1e-6);
        s.lines[0].edges[0] = Edge::Curve {
            c1: p(20.0, 40.0),
            c2: p(40.0, 40.0),
        };
        assert!(line_length(&s.lines[0]) > 70.0);
    }

    #[test]
    fn labels_go_outside_in_either_winding() {
        let s = square();
        assert!(is_counter_clockwise(&s));
        let (at, out) = edge_label_anchor(&s, 0);
        close(at, p(50.0, 0.0));
        close(out, p(0.0, -1.0));
        let twin = s.reflected(p(300.0, 0.0));
        assert!(!is_counter_clockwise(&twin));
        let (_, out) = edge_label_anchor(&twin, 0);
        close(out, p(0.0, -1.0));
    }

    #[test]
    fn all_the_anchors_at_once_are_the_ones_given_one_at_a_time() {
        let mut ccw = square();
        ccw.set_curved(1, true);
        ccw.set_handle(1, opendrape_core::HandleEnd::Start, p(140.0, 20.0));
        let cw = ccw.reflected(p(300.0, 0.0));
        assert!(is_counter_clockwise(&ccw) && !is_counter_clockwise(&cw));
        for piece in [ccw, cw] {
            let all = edge_label_anchors(&piece);
            assert_eq!(all.len(), piece.len());
            for (i, anchor) in all.into_iter().enumerate() {
                assert_eq!(anchor, edge_label_anchor(&piece, i), "edge {i}");
            }
        }
    }
}
