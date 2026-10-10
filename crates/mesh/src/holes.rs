//! The cut-outs of one shape that can be meshed as holes. A hole must lie wholly inside the
//! stitching outline, a quarter of the target edge length (or more) clear of it and of every
//! other hole; one that doesn't is left out, and only that hole: the rest of the piece still
//! becomes fabric. (A hole outside the outline would otherwise become an island of fabric, one
//! touching the outline would split an outline edge, and one close to it would make slivers.)

use opendrape_core::{LineKind, Point2};
use opendrape_geom::{self as geom, Shape};

/// A hole must be at least this many target edge lengths from the outline and from other holes.
pub(crate) const CLEARANCE_PER_H: f64 = 0.25;
/// A hole is always made of at least this many points, so a small one keeps its shape.
const MIN_HOLE_POINTS: f64 = 16.0;

/// A shape's holes as loops of points (mm), and how many closed cut-outs were left out.
pub(crate) struct Holes {
    pub loops: Vec<Vec<[f64; 2]>>,
    pub left_out: usize,
}

/// The holes of `shape`, given its sampled stitching outline `outline` (mm) and the target edge
/// length `h` (mm).
pub(crate) fn holes(shape: &Shape, outline: &[[f64; 2]], h: f64) -> Holes {
    let mut left_out = 0;
    let mut candidates: Vec<Vec<[f64; 2]>> = Vec::new();
    let (outline_lo, outline_hi) = box_of(outline);
    for line in shape
        .piece
        .lines
        .iter()
        .filter(|l| l.closed && l.kind == LineKind::Cutout)
        // `Piece::check` refuses a cut-out with fewer than 3 points, or one whose edges don't
        // match its points; a project built without it must not panic the mesher.
        .filter(|l| l.vertices.len() >= 3 && l.edges.len() == l.edge_count())
    {
        let points = geom::line_points(line, 0.1);
        // A cut-out inside the outline is inside its bounding box. One that isn't is left out
        // before it is resampled: `Piece::check` lets a cut-out reach a kilometre from the
        // piece, and a loop of that length is many thousands of points to check against each
        // other, a stall (seconds) for a hole that was going to be left out anyway.
        let (lo, hi) = points
            .iter()
            .fold(([f64::MAX; 2], [f64::MIN; 2]), |(lo, hi), q| {
                (
                    [lo[0].min(q.x), lo[1].min(q.y)],
                    [hi[0].max(q.x), hi[1].max(q.y)],
                )
            });
        if !(lo[0] >= outline_lo[0]
            && lo[1] >= outline_lo[1]
            && hi[0] <= outline_hi[0]
            && hi[1] <= outline_hi[1])
        {
            left_out += 1;
            continue;
        }
        let sampled = resampled_loop(&points, h);
        // Too few points, or no length at all: there is nothing to cut.
        let Some(first) = sampled.first().copied() else {
            continue;
        };
        if inside(outline, first) && is_simple(&sampled) {
            candidates.push(sampled);
        } else {
            left_out += 1;
        }
    }
    let clear = CLEARANCE_PER_H * h;
    let loops: Vec<Vec<[f64; 2]>> = candidates
        .iter()
        .enumerate()
        .filter(|(i, hole)| {
            is_clear(hole, outline, clear)
                && candidates.iter().enumerate().all(|(j, other)| {
                    // Clear of every other hole, and not inside one (that would be an island).
                    j == *i || (is_clear(hole, other, clear) && !inside(other, hole[0]))
                })
        })
        .map(|(_, hole)| hole.clone())
        .collect();
    Holes {
        left_out: left_out + candidates.len() - loops.len(),
        loops,
    }
}

/// A closed polyline (its last point repeats its first) as points evenly spaced along it: about
/// `h` apart, but at least [`MIN_HOLE_POINTS`] of them, so a small cut-out keeps its shape.
/// Empty when it has fewer than 3 points or no length.
fn resampled_loop(points: &[Point2], h: f64) -> Vec<[f64; 2]> {
    let mut pts = points.to_vec();
    if pts.len() > 1 && pts.first() == pts.last() {
        pts.pop();
    }
    let n = pts.len();
    if n < 3 {
        return Vec::new();
    }
    let seg = |k: usize| pts[k].distance(pts[(k + 1) % n]);
    let total: f64 = (0..n).map(seg).sum();
    if !(total.is_finite() && total > 1e-6) {
        return Vec::new();
    }
    let step = h.min(total / MIN_HOLE_POINTS);
    let count = ((total / step).round() as usize).max(3);
    let mut out = Vec::with_capacity(count);
    let (mut k, mut start) = (0, 0.0);
    for q in 0..count {
        let s = total * q as f64 / count as f64;
        while k + 1 < n && s > start + seg(k) {
            start += seg(k);
            k += 1;
        }
        let t = if seg(k) > 0.0 {
            (s - start) / seg(k)
        } else {
            0.0
        };
        let p = pts[k].lerp(pts[(k + 1) % n], t.clamp(0.0, 1.0));
        out.push([p.x, p.y]);
    }
    out
}

/// Whether `p` is inside the closed polygon `poly` (even-odd rule). A point on the boundary
/// may count either way.
fn inside(poly: &[[f64; 2]], p: [f64; 2]) -> bool {
    let mut inside = false;
    let mut j = poly.len().wrapping_sub(1);
    for i in 0..poly.len() {
        let (a, b) = (poly[i], poly[j]);
        if (a[1] > p[1]) != (b[1] > p[1])
            && p[0] < (b[0] - a[0]) * (p[1] - a[1]) / (b[1] - a[1]) + a[0]
        {
            inside = !inside;
        }
        j = i;
    }
    inside
}

/// No two edges of the closed polygon `poly` cross or touch, apart from neighbours meeting at
/// their shared point.
fn is_simple(poly: &[[f64; 2]]) -> bool {
    let n = poly.len();
    for i in 0..n {
        for j in i + 1..n {
            let neighbours = j == i + 1 || (i == 0 && j == n - 1);
            let (a, b) = (poly[i], poly[(i + 1) % n]);
            let (c, d) = (poly[j], poly[(j + 1) % n]);
            if neighbours {
                continue;
            }
            if segment_distance(a, b, c, d) <= 0.0 {
                return false;
            }
        }
    }
    true
}

/// Whether the closed polylines `a` and `b` are at least `clear` apart everywhere.
fn is_clear(a: &[[f64; 2]], b: &[[f64; 2]], clear: f64) -> bool {
    let (lo, hi) = box_of(a);
    let (lo, hi) = (
        [lo[0] - clear, lo[1] - clear],
        [hi[0] + clear, hi[1] + clear],
    );
    for k in 0..b.len() {
        let (c, d) = (b[k], b[(k + 1) % b.len()]);
        // Only edges near `a`'s bounding box can be too close.
        if c[0].max(d[0]) < lo[0]
            || c[0].min(d[0]) > hi[0]
            || c[1].max(d[1]) < lo[1]
            || c[1].min(d[1]) > hi[1]
        {
            continue;
        }
        for i in 0..a.len() {
            if segment_distance(a[i], a[(i + 1) % a.len()], c, d) < clear {
                return false;
            }
        }
    }
    true
}

fn box_of(points: &[[f64; 2]]) -> ([f64; 2], [f64; 2]) {
    points
        .iter()
        .fold(([f64::MAX; 2], [f64::MIN; 2]), |(lo, hi), p| {
            (
                [lo[0].min(p[0]), lo[1].min(p[1])],
                [hi[0].max(p[0]), hi[1].max(p[1])],
            )
        })
}

/// The shortest distance between segments `a`–`b` and `c`–`d` (0 when they cross or touch).
fn segment_distance(a: [f64; 2], b: [f64; 2], c: [f64; 2], d: [f64; 2]) -> f64 {
    let side = |p: [f64; 2], q: [f64; 2], r: [f64; 2]| {
        (q[0] - p[0]) * (r[1] - p[1]) - (q[1] - p[1]) * (r[0] - p[0])
    };
    let (d1, d2, d3, d4) = (side(a, b, c), side(a, b, d), side(c, d, a), side(c, d, b));
    if ((d1 > 0.0 && d2 < 0.0) || (d1 < 0.0 && d2 > 0.0))
        && ((d3 > 0.0 && d4 < 0.0) || (d3 < 0.0 && d4 > 0.0))
    {
        return 0.0;
    }
    point_segment_distance(a, c, d)
        .min(point_segment_distance(b, c, d))
        .min(point_segment_distance(c, a, b))
        .min(point_segment_distance(d, a, b))
}

fn point_segment_distance(p: [f64; 2], a: [f64; 2], b: [f64; 2]) -> f64 {
    let (ab, ap) = ([b[0] - a[0], b[1] - a[1]], [p[0] - a[0], p[1] - a[1]]);
    let len2 = ab[0] * ab[0] + ab[1] * ab[1];
    let t = if len2 > 0.0 {
        ((ap[0] * ab[0] + ap[1] * ab[1]) / len2).clamp(0.0, 1.0)
    } else {
        0.0
    };
    (ap[0] - t * ab[0]).hypot(ap[1] - t * ab[1])
}
