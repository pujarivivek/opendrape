//! The cut line: where the fabric is cut, outside the stitching line by each edge's seam
//! allowance. It is always worked out from the stitching line and never stored.

use crate::{edge_points, is_counter_clockwise};
use i_overlay::core::fill_rule::FillRule;
use i_overlay::float::simplify::SimplifyShape;
use opendrape_core::{Piece, Point2};

/// A corner is mitered (pointed) unless the point would land more than this many allowance
/// widths from the corner; then it is squared off.
const MITER_LIMIT: f64 = 2.5;
/// How closely (mm) curved edges are followed when working out the cut line.
const TOLERANCE_MM: f64 = 0.1;

/// How far out of order (mm) the two offset lines of a joint may meet before the corner is
/// treated as too shallow to miter or trim.
const ORDER_EPS: f64 = 1e-9;

/// The cut line: every edge pushed outwards by its seam allowance, with mitered corners
/// (squared beyond [`MITER_LIMIT`]) and, at the ends of a hem, the neighbouring seam mirrored
/// about the hem's stitching line so the hem folds up flat against it. Where the allowances
/// on either side of a shallow corner differ so much that the two offset lines would meet far
/// away or out of order, the corner is beveled (convex) or joined through the corner point
/// (concave) instead. Inward curves tighter than the allowance are bridged straight across.
/// Counter-clockwise (whatever the piece's winding), and closed; the first point is not
/// repeated. With no allowance anywhere it is the stitching line (counter-clockwise).
pub fn cut_line(piece: &Piece) -> Vec<Point2> {
    let n = piece.len();
    let edges: Vec<(Vec<Point2>, f64, bool)> = (0..n)
        .map(|i| {
            (
                edge_points(piece, i, TOLERANCE_MM),
                piece.edge_allowance(i).max(0.0),
                piece.edge_props[i].hem,
            )
        })
        .collect();
    let sew: Vec<Point2> = edges
        .iter()
        .flat_map(|(pts, _, _)| pts[..pts.len() - 1].iter().copied())
        .collect();
    let ccw = is_counter_clockwise(piece);
    // Every ring handed to i_overlay, and returned, runs counter-clockwise.
    let as_array = |pts: &[Point2]| -> Vec<[f64; 2]> {
        let mut v: Vec<[f64; 2]> = pts.iter().map(|q| [q.x, q.y]).collect();
        if !ccw {
            v.reverse();
        }
        v
    };
    let to_points = |c: Vec<[f64; 2]>| -> Vec<Point2> {
        c.into_iter().map(|[x, y]| Point2::new(x, y)).collect()
    };
    if edges.iter().all(|(_, w, _)| *w == 0.0) {
        return to_points(as_array(&sew));
    }
    let out = |d: Point2| {
        if ccw {
            Point2::new(d.y, -d.x)
        } else {
            Point2::new(-d.y, d.x)
        }
    };

    struct Seg {
        end: Point2,
        d: Point2,
        w: f64,
        edge: usize,
    }
    let mut segs: Vec<Seg> = Vec::new();
    for (edge, (pts, w, _)) in edges.iter().enumerate() {
        for s in pts.windows(2) {
            let d = s[1] - s[0];
            let len = d.length();
            if len > 1e-12 {
                segs.push(Seg {
                    end: s[1],
                    d: d * (1.0 / len),
                    w: *w,
                    edge,
                });
            }
        }
    }
    let mut raw: Vec<Point2> = Vec::new();
    let m = segs.len();
    for k in 0..m {
        let (s, t) = (&segs[k], &segs[(k + 1) % m]);
        let v = s.end;
        let pa = v + out(s.d) * s.w;
        let pb = v + out(t.d) * t.w;
        let (hem_a, hem_b) = (edges[s.edge].2, edges[t.edge].2);
        if s.edge != t.edge && hem_a != hem_b {
            // The seam's cut line, continued to the hem's stitching line and then mirrored
            // about it, so the hem folds up flat against the seam allowance. Only when the
            // seam reaches the hem from the piece's side and both points stay close.
            let (seam_p, seam_d, hem_d, hem_off, toward) = if hem_b {
                (pa, s.d, t.d, pb, s.d)
            } else {
                (pb, t.d, s.d, pa, t.d * -1.0)
            };
            let reach = MITER_LIMIT * s.w.max(t.w);
            if dot(toward, out(hem_d)) > 0.0
                && let Some(at_hem) = intersect(seam_p, seam_d, v, hem_d)
                && let mirrored = reflect(at_hem + seam_d, v, hem_d) - at_hem
                && let Some(on_cut) = intersect(at_hem, mirrored, hem_off, hem_d)
                && at_hem.distance(v) <= reach
                && on_cut.distance(v) <= reach
            {
                if hem_b {
                    raw.extend([pa, at_hem, on_cut, pb]);
                } else {
                    raw.extend([pa, on_cut, at_hem, pb]);
                }
                continue;
            }
        }
        let convex = cross(s.d, t.d) * if ccw { 1.0 } else { -1.0 } > 0.0;
        let Some(meet) = intersect(pa, s.d, pb, t.d) else {
            raw.extend([pa, pb]);
            continue;
        };
        // How far the meeting point lies ahead of each offset end, along its own line.
        let (ahead_a, ahead_b) = (dot(meet - pa, s.d), dot(meet - pb, t.d));
        if convex {
            if ahead_a < -ORDER_EPS || ahead_b > ORDER_EPS {
                raw.extend([pa, pb]);
            } else if meet.distance(v) > MITER_LIMIT * s.w.max(t.w) {
                raw.extend([pa, pa + s.d * s.w, pb - t.d * t.w, pb]);
            } else {
                raw.push(meet);
            }
        } else if ahead_a > ORDER_EPS || ahead_b < -ORDER_EPS {
            raw.extend([pa, v, pb]);
        } else {
            raw.push(meet);
        }
    }
    // The raw path crosses itself where curves are tighter than the allowance; its union with
    // the stitching polygon, by the non-zero rule, has the true cut line as its outer contour.
    let raw = as_array(&raw);
    let parts = vec![as_array(&sew), raw.clone()];
    to_points(outer_or(parts.simplify_shape(FillRule::NonZero), raw))
}

/// The outer contour of the largest of `shapes` (each a list of contours, the outer one
/// first), or `fallback` when there are none.
fn outer_or(shapes: Vec<Vec<Vec<[f64; 2]>>>, fallback: Vec<[f64; 2]>) -> Vec<[f64; 2]> {
    shapes
        .into_iter()
        .filter_map(|shape| shape.into_iter().next())
        .max_by(|a, b| contour_area(a).total_cmp(&contour_area(b)))
        .unwrap_or(fallback)
}

fn cross(a: Point2, b: Point2) -> f64 {
    a.x * b.y - a.y * b.x
}

fn dot(a: Point2, b: Point2) -> f64 {
    a.x * b.x + a.y * b.y
}

/// Where the line through `p` along `d` meets the line through `q` along `e`.
fn intersect(p: Point2, d: Point2, q: Point2, e: Point2) -> Option<Point2> {
    let den = cross(d, e);
    if den.abs() < 1e-12 {
        return None;
    }
    let t = cross(q - p, e) / den;
    Some(p + d * t)
}

/// Mirror image of `x` across the line through `o` along unit direction `u`.
fn reflect(x: Point2, o: Point2, u: Point2) -> Point2 {
    let v = x - o;
    let along = u * (v.x * u.x + v.y * u.y);
    o + along * 2.0 - v
}

fn contour_area(c: &[[f64; 2]]) -> f64 {
    (0..c.len())
        .map(|k| {
            let (a, b) = (c[k], c[(k + 1) % c.len()]);
            a[0] * b[1] - b[0] * a[1]
        })
        .sum::<f64>()
        .abs()
}

#[cfg(test)]
mod tests {
    use super::*;
    use opendrape_core::{PieceId, Point2};

    fn p(x: f64, y: f64) -> Point2 {
        Point2::new(x, y)
    }

    fn polygon(pts: &[(f64, f64)], widths: &[f64]) -> Piece {
        let corners: Vec<Point2> = pts.iter().map(|&(x, y)| p(x, y)).collect();
        let mut piece = Piece::polygon(PieceId(1), "P", &corners);
        for (props, &w) in piece.edge_props.iter_mut().zip(widths) {
            props.allowance = Some(w);
        }
        piece
    }

    fn area(points: &[Point2]) -> f64 {
        (0..points.len())
            .map(|k| {
                let (a, b) = (points[k], points[(k + 1) % points.len()]);
                a.x * b.y - b.x * a.y
            })
            .sum::<f64>()
            .abs()
            / 2.0
    }

    /// Smallest distance from `q` to the stitching outline.
    fn to_outline(piece: &Piece, q: Point2) -> f64 {
        crate::nearest_edge(piece, q).unwrap().2
    }

    /// Every cut point is at most `max` mm from the stitching outline.
    fn assert_within(piece: &Piece, cut: &[Point2], max: f64) {
        for q in cut {
            let d = to_outline(piece, *q);
            assert!(
                d <= max + 1e-6,
                "{q:?} is {d} from the stitching, over {max}"
            );
        }
    }

    /// Whether `q` is inside the closed ring `ring` (even-odd).
    fn inside(ring: &[Point2], q: Point2) -> bool {
        let mut yes = false;
        for k in 0..ring.len() {
            let (a, b) = (ring[k], ring[(k + 1) % ring.len()]);
            if (a.y > q.y) != (b.y > q.y) && q.x < a.x + (q.y - a.y) / (b.y - a.y) * (b.x - a.x) {
                yes = !yes;
            }
        }
        yes
    }

    fn signed_area(points: &[Point2]) -> f64 {
        (0..points.len())
            .map(|k| {
                let (a, b) = (points[k], points[(k + 1) % points.len()]);
                a.x * b.y - b.x * a.y
            })
            .sum::<f64>()
            / 2.0
    }

    #[test]
    fn a_rectangle_grows_by_its_allowance_in_either_winding() {
        let r = [(0.0, 0.0), (100.0, 0.0), (100.0, 200.0), (0.0, 200.0)];
        let rev: Vec<(f64, f64)> = r.iter().rev().copied().collect();
        for pts in [r.to_vec(), rev] {
            let piece = polygon(&pts, &[10.0; 4]);
            let cut = cut_line(&piece);
            assert!((area(&cut) - 120.0 * 220.0).abs() < 1e-3, "{}", area(&cut));
            assert_within(&piece, &cut, MITER_LIMIT * 10.0);
        }
    }

    #[test]
    fn the_piece_allowance_and_hem_defaults_apply() {
        let mut piece = Piece::rectangle(PieceId(1), "R", p(0.0, 0.0), 100.0, 200.0);
        piece.edge_props[0].hem = true; // bottom: 30 mm; the others 10 mm
        let cut = cut_line(&piece);
        let min_y = cut.iter().map(|q| q.y).fold(f64::MAX, f64::min);
        assert!((min_y + 30.0).abs() < 1e-6, "{min_y}");
        assert!((area(&cut) - 120.0 * 240.0).abs() < 1e-3, "{}", area(&cut));
        assert_within(&piece, &cut, MITER_LIMIT * 30.0);
    }

    #[test]
    fn hem_corners_mirror_the_flared_side_seams() {
        let mut piece = polygon(
            &[(0.0, 0.0), (400.0, 0.0), (320.0, 600.0), (80.0, 600.0)],
            &[30.0, 10.0, 10.0, 10.0],
        );
        piece.edge_props[0].hem = true;
        let cut = cut_line(&piece);
        let bottom: Vec<&Point2> = cut.iter().filter(|q| q.y < -29.99).collect();
        let min_x = bottom.iter().map(|q| q.x).fold(f64::MAX, f64::min);
        let max_x = bottom.iter().map(|q| q.x).fold(f64::MIN, f64::max);
        // Checked by hand: the side seam's cut line meets the hem line at x = -10.0885 and,
        // mirrored about it, slopes 4 mm back in over the 30 mm hem.
        assert!((min_x + 6.0885).abs() < 0.01, "{min_x}");
        assert!((max_x - 406.0885).abs() < 0.01, "{max_x}");
        assert_within(&piece, &cut, MITER_LIMIT * 30.0);
    }

    #[test]
    fn a_concave_corner_meets_at_both_widths() {
        let pts = [
            (0.0, 0.0),
            (300.0, 0.0),
            (300.0, 100.0),
            (100.0, 100.0),
            (100.0, 300.0),
            (0.0, 300.0),
        ];
        let piece = polygon(&pts, &[10.0; 6]);
        let even = cut_line(&piece);
        assert!((area(&even) - 62_400.0).abs() < 1e-3, "{}", area(&even));
        assert_within(&piece, &even, MITER_LIMIT * 10.0);
        let piece = polygon(&pts, &[10.0, 10.0, 20.0, 5.0, 10.0, 10.0]);
        let mixed = cut_line(&piece);
        assert!(
            mixed.iter().any(|q| q.distance(p(105.0, 120.0)) < 1e-3),
            "{mixed:?}"
        );
        assert_within(&piece, &mixed, MITER_LIMIT * 20.0);
    }

    #[test]
    fn no_allowance_leaves_the_edge_as_it_is() {
        let piece = polygon(
            &[(0.0, 0.0), (100.0, 0.0), (100.0, 200.0), (0.0, 200.0)],
            &[10.0, 10.0, 10.0, 0.0],
        );
        let cut = cut_line(&piece);
        let min_x = cut.iter().map(|q| q.x).fold(f64::MAX, f64::min);
        assert!(min_x.abs() < 1e-6, "{min_x}");
        assert_within(&piece, &cut, MITER_LIMIT * 10.0);
        let none = polygon(&[(0.0, 0.0), (100.0, 0.0), (0.0, 100.0)], &[0.0; 3]);
        assert_eq!(cut_line(&none).len(), 3);
        assert_within(&none, &cut_line(&none), 0.0);
    }

    #[test]
    fn a_sharp_tip_is_squared_off() {
        let a = 5f64.to_radians();
        let tip = polygon(
            &[
                (0.0, 0.0),
                (300.0 * a.cos(), -300.0 * a.sin()),
                (300.0 * a.cos(), 300.0 * a.sin()),
            ],
            &[10.0; 3],
        );
        let cut = cut_line(&tip);
        let reach = cut
            .iter()
            .filter(|q| q.x < 0.0)
            .map(|q| -q.x)
            .fold(0.0, f64::max);
        assert!(reach <= 2.5 * 10.0, "{reach}");
        assert_within(&tip, &cut, MITER_LIMIT * 10.0);
    }

    #[test]
    fn a_tight_inward_curve_is_bridged_never_cut_into() {
        // A 100 mm square whose top edge dips 30 mm into the piece in a narrow curve.
        let mut piece = Piece::rectangle(PieceId(1), "Bite", p(0.0, 0.0), 100.0, 100.0);
        // Edge 2 runs (100,100)→(0,100); its control points pull it down into a narrow dip.
        piece.edges[2] = opendrape_core::Edge::Curve {
            c1: p(52.0, 55.0),
            c2: p(48.0, 55.0),
        };
        let cut = cut_line(&piece);
        for q in &cut {
            assert!(
                to_outline(&piece, *q) > 10.0 - 0.15,
                "{q:?} is {} from the stitching",
                to_outline(&piece, *q)
            );
            assert!(!crate::contains(&piece, *q), "{q:?} inside the piece");
        }
        assert_within(&piece, &cut, MITER_LIMIT * 10.0);
    }

    #[test]
    fn every_cut_point_keeps_its_distance() {
        let mut piece = Piece::rectangle(PieceId(1), "R", p(0.0, 0.0), 300.0, 400.0);
        piece.set_curved(1, true);
        piece.set_handle(1, opendrape_core::HandleEnd::Start, p(380.0, 100.0));
        let cut = cut_line(&piece);
        for q in &cut {
            assert!(to_outline(&piece, *q) > 10.0 - 0.15, "{q:?}");
        }
        assert_within(&piece, &cut, MITER_LIMIT * 10.0);
    }

    #[test]
    fn a_shallow_kink_with_different_widths_steps_without_a_spike() {
        // The bottom edge is split at x = 100 and bent by `k` mm, convex for -k and concave
        // for +k; the two halves have different widths, so their offset lines meet far away.
        for sign in [1.0, -1.0] {
            for k in [0.01, 0.5, 2.0] {
                let piece = polygon(
                    &[
                        (0.0, 0.0),
                        (100.0, sign * k),
                        (200.0, 0.0),
                        (200.0, 100.0),
                        (0.0, 100.0),
                    ],
                    &[10.0, 20.0, 10.0, 10.0, 10.0],
                );
                let cut = cut_line(&piece);
                assert_within(&piece, &cut, MITER_LIMIT * 20.0);
                for q in &cut {
                    let d = to_outline(&piece, *q);
                    assert!(
                        d >= 10.0 - 1e-6,
                        "k {}: {q:?} is {d} from the stitching",
                        sign * k
                    );
                }
            }
        }
    }

    #[test]
    fn a_split_curve_with_one_half_overridden_has_no_spike() {
        // Bulging out (convex at the split) and bulging in (concave at the split).
        for handle in [p(380.0, 100.0), p(220.0, 100.0)] {
            let mut piece = Piece::rectangle(PieceId(1), "R", p(0.0, 0.0), 300.0, 400.0);
            piece.set_curved(1, true);
            piece.set_handle(1, opendrape_core::HandleEnd::Start, handle);
            assert!(crate::split_edge(&mut piece, 1, 0.5).is_some());
            piece.edge_props[1].allowance = Some(20.0);
            let cut = cut_line(&piece);
            assert_within(&piece, &cut, MITER_LIMIT * 20.0);
            // The offset follows the curve flattened to TOLERANCE_MM, so it can sit a hair
            // closer to the true curve than 10 mm: the same 0.15 mm slack as the other curve
            // tests.
            for q in &cut {
                let d = to_outline(&piece, *q);
                assert!(
                    d > 10.0 - 0.15,
                    "{handle:?}: {q:?} is {d} from the stitching"
                );
            }
        }
    }

    #[test]
    fn a_hem_beside_a_shallow_or_reflex_seam_keeps_its_allowance() {
        for y0 in [-150.0, -30.0, -10.0, -3.0, -1.0, 1.0, 3.0] {
            let mut piece = polygon(
                &[
                    (-150.0, y0),
                    (0.0, 0.0),
                    (300.0, 0.0),
                    (300.0, 200.0),
                    (-150.0, 200.0),
                ],
                &[10.0; 5],
            );
            piece.edge_props[1].allowance = None;
            piece.edge_props[1].hem = true; // (0,0)→(300,0): 30 mm
            let cut = cut_line(&piece);
            for x in [50.0, 100.0, 150.0, 200.0, 250.0] {
                assert!(
                    inside(&cut, p(x, -29.9)),
                    "y0 {y0}: ({x}, -29.9) is outside"
                );
                assert!(
                    !inside(&cut, p(x, -30.1)),
                    "y0 {y0}: ({x}, -30.1) is inside"
                );
            }
            assert_within(&piece, &cut, MITER_LIMIT * 30.0);
        }
    }

    #[test]
    fn an_empty_overlay_falls_back_to_the_offset_path() {
        let path = vec![[0.0, 0.0], [5.0, 0.0], [5.0, 5.0]];
        assert_eq!(outer_or(Vec::new(), path.clone()), path);
        let big = vec![[0.0, 0.0], [9.0, 0.0], [9.0, 9.0]];
        let small = vec![[0.0, 0.0], [1.0, 0.0], [1.0, 1.0]];
        let shapes = vec![vec![small], vec![big.clone()]];
        assert_eq!(outer_or(shapes, path), big);
    }

    #[test]
    fn the_cut_line_is_counter_clockwise() {
        let cw = [(0.0, 0.0), (0.0, 200.0), (100.0, 200.0), (100.0, 0.0)];
        for w in [10.0, 0.0] {
            let cut = cut_line(&polygon(&cw, &[w; 4]));
            assert!(signed_area(&cut) > 0.0, "width {w}: {}", signed_area(&cut));
        }
        let ccw = [(0.0, 0.0), (100.0, 0.0), (100.0, 200.0), (0.0, 200.0)];
        for w in [10.0, 0.0] {
            let cut = cut_line(&polygon(&ccw, &[w; 4]));
            assert!(signed_area(&cut) > 0.0, "width {w}: {}", signed_area(&cut));
        }
    }
}
