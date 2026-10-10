//! The little geometry `Project::check` needs without a curve library: how long an edge is, and
//! how far a point lies outside a piece. Curves are followed as polylines of [`CURVE_STEPS`]
//! steps, which is well within the millimetre the checks allow (a 30 cm curve bent through a
//! right angle comes out about 0.02 mm short). `opendrape-geom` measures exactly for drawing and
//! meshing; these numbers only ever decide validity, so every rule that uses them uses them alone.

use crate::{Edge, Piece, Point2};

/// Steps a curved edge is followed in.
const CURVE_STEPS: usize = 64;

/// Point at parameter `s` of edge `i` (0 at its start, 1 at its end).
fn edge_point(piece: &Piece, i: usize, s: f64) -> Point2 {
    let (a, b) = piece.edge_ends(i);
    match piece.edges[i] {
        Edge::Line => a.lerp(b, s),
        Edge::Curve { c1, c2 } => {
            let u = 1.0 - s;
            a * (u * u * u) + c1 * (3.0 * u * u * s) + c2 * (3.0 * u * s * s) + b * (s * s * s)
        }
    }
}

/// Edge `i` as points from its start to its end.
fn edge_polyline(piece: &Piece, i: usize) -> Vec<Point2> {
    let steps = match piece.edges[i] {
        Edge::Line => 1,
        Edge::Curve { .. } => CURVE_STEPS,
    };
    (0..=steps)
        .map(|k| edge_point(piece, i, k as f64 / steps as f64))
        .collect()
}

/// Length (mm) of edge `i`, as the checks measure it.
pub(crate) fn edge_length(piece: &Piece, i: usize) -> f64 {
    edge_polyline(piece, i)
        .windows(2)
        .map(|w| w[0].distance(w[1]))
        .sum()
}

/// How far (mm) `p` lies outside the piece's outline: 0 inside it.
pub(crate) fn distance_outside(piece: &Piece, p: Point2) -> f64 {
    let outline: Vec<Point2> = (0..piece.len())
        .flat_map(|i| {
            let mut pts = edge_polyline(piece, i);
            pts.pop(); // the next edge starts there
            pts
        })
        .collect();
    let n = outline.len();
    let mut inside = false;
    let mut nearest = f64::INFINITY;
    for k in 0..n {
        let (a, b) = (outline[k], outline[(k + 1) % n]);
        if (a.y > p.y) != (b.y > p.y) && p.x < a.x + (p.y - a.y) / (b.y - a.y) * (b.x - a.x) {
            inside = !inside;
        }
        let ab = b - a;
        let len2 = ab.x * ab.x + ab.y * ab.y;
        let s = if len2 < 1e-18 {
            0.0
        } else {
            (((p.x - a.x) * ab.x + (p.y - a.y) * ab.y) / len2).clamp(0.0, 1.0)
        };
        nearest = nearest.min(p.distance(a + ab * s));
    }
    if inside { 0.0 } else { nearest }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::PieceId;

    #[test]
    fn edges_are_measured_closely_enough_for_the_checks() {
        let mut square = Piece::rectangle(PieceId(1), "S", Point2::new(0.0, 0.0), 100.0, 100.0);
        assert_eq!(edge_length(&square, 0), 100.0);
        // A quarter circle of radius 300 (control points at the usual 0.5523 of the radius).
        let k = 0.552_284_75 * 300.0;
        square.vertices[1].pos = Point2::new(300.0, 0.0);
        square.vertices[2].pos = Point2::new(0.0, 300.0);
        square.edges[1] = Edge::Curve {
            c1: Point2::new(300.0, k),
            c2: Point2::new(k, 300.0),
        };
        let arc = std::f64::consts::FRAC_PI_2 * 300.0;
        assert!(
            (edge_length(&square, 1) - arc).abs() < 0.1,
            "{}",
            edge_length(&square, 1)
        );
    }

    #[test]
    fn a_point_inside_is_no_distance_outside() {
        let square = Piece::rectangle(PieceId(1), "S", Point2::new(0.0, 0.0), 100.0, 100.0);
        assert_eq!(distance_outside(&square, Point2::new(50.0, 50.0)), 0.0);
        assert!((distance_outside(&square, Point2::new(103.0, 50.0)) - 3.0).abs() < 1e-12);
        assert!((distance_outside(&square, Point2::new(-0.5, 100.0)) - 0.5).abs() < 1e-12);
    }
}
