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

/// The cut line: every edge pushed outwards by its seam allowance, with mitered corners
/// (squared beyond [`MITER_LIMIT`]) and, at the ends of a hem, the neighbouring seam mirrored
/// about the hem's stitching line so the hem folds up flat against it. Inward curves tighter
/// than the allowance are bridged straight across. Closed; the first point is not repeated.
/// With no allowance anywhere it is the stitching line.
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
    if edges.iter().all(|(_, w, _)| *w == 0.0) {
        return sew;
    }
    let ccw = is_counter_clockwise(piece);
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
            // about it, so the hem folds up flat against the seam allowance.
            let (seam_p, seam_d, hem_d, hem_off) = if hem_b {
                (pa, s.d, t.d, pb)
            } else {
                (pb, t.d, s.d, pa)
            };
            if let Some(at_hem) = intersect(seam_p, seam_d, v, hem_d) {
                let mirrored = reflect(at_hem + seam_d, v, hem_d) - at_hem;
                if let Some(on_cut) = intersect(at_hem, mirrored, hem_off, hem_d) {
                    if hem_b {
                        raw.extend([pa, at_hem, on_cut, pb]);
                    } else {
                        raw.extend([pa, on_cut, at_hem, pb]);
                    }
                    continue;
                }
            }
        }
        let turn = cross(s.d, t.d) * if ccw { 1.0 } else { -1.0 };
        match intersect(pa, s.d, pb, t.d) {
            None => raw.extend([pa, pb]),
            Some(miter) if turn > 0.0 && miter.distance(v) > MITER_LIMIT * s.w.max(t.w) => {
                raw.extend([pa, pa + s.d * s.w, pb - t.d * t.w, pb]);
            }
            Some(meet) => raw.push(meet),
        }
    }
    // The raw path crosses itself where curves are tighter than the allowance; its union with
    // the stitching polygon, by the non-zero rule, has the true cut line as its outer contour.
    let as_array = |pts: &[Point2]| -> Vec<[f64; 2]> {
        let mut v: Vec<[f64; 2]> = pts.iter().map(|q| [q.x, q.y]).collect();
        if !ccw {
            v.reverse();
        }
        v
    };
    let parts = vec![as_array(&sew), as_array(&raw)];
    parts
        .simplify_shape(FillRule::NonZero)
        .into_iter()
        .filter_map(|shape| shape.into_iter().next())
        .max_by(|a, b| contour_area(a).total_cmp(&contour_area(b)))
        .map(|c| c.into_iter().map(|[x, y]| Point2::new(x, y)).collect())
        .unwrap_or(sew)
}

fn cross(a: Point2, b: Point2) -> f64 {
    a.x * b.y - a.y * b.x
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

    #[test]
    fn a_rectangle_grows_by_its_allowance_in_either_winding() {
        let r = [(0.0, 0.0), (100.0, 0.0), (100.0, 200.0), (0.0, 200.0)];
        let rev: Vec<(f64, f64)> = r.iter().rev().copied().collect();
        for pts in [r.to_vec(), rev] {
            let cut = cut_line(&polygon(&pts, &[10.0; 4]));
            assert!((area(&cut) - 120.0 * 220.0).abs() < 1e-3, "{}", area(&cut));
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
        let even = cut_line(&polygon(&pts, &[10.0; 6]));
        assert!((area(&even) - 62_400.0).abs() < 1e-3, "{}", area(&even));
        let mixed = cut_line(&polygon(&pts, &[10.0, 10.0, 20.0, 5.0, 10.0, 10.0]));
        assert!(
            mixed.iter().any(|q| q.distance(p(105.0, 120.0)) < 1e-3),
            "{mixed:?}"
        );
    }

    #[test]
    fn no_allowance_leaves_the_edge_as_it_is() {
        let cut = cut_line(&polygon(
            &[(0.0, 0.0), (100.0, 0.0), (100.0, 200.0), (0.0, 200.0)],
            &[10.0, 10.0, 10.0, 0.0],
        ));
        let min_x = cut.iter().map(|q| q.x).fold(f64::MAX, f64::min);
        assert!(min_x.abs() < 1e-6, "{min_x}");
        let none = polygon(&[(0.0, 0.0), (100.0, 0.0), (0.0, 100.0)], &[0.0; 3]);
        assert_eq!(cut_line(&none).len(), 3);
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
        let reach = cut_line(&tip)
            .iter()
            .filter(|q| q.x < 0.0)
            .map(|q| -q.x)
            .fold(0.0, f64::max);
        assert!(reach <= 2.5 * 10.0, "{reach}");
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
    }

    #[test]
    fn every_cut_point_keeps_its_distance() {
        let mut piece = Piece::rectangle(PieceId(1), "R", p(0.0, 0.0), 300.0, 400.0);
        piece.set_curved(1, true);
        piece.set_handle(1, opendrape_core::HandleEnd::Start, p(380.0, 100.0));
        for q in cut_line(&piece) {
            assert!(to_outline(&piece, q) > 10.0 - 0.15, "{q:?}");
        }
    }
}
