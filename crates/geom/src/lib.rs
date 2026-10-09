//! Geometry on pattern pieces (lengths, areas, hit-testing, splitting and resizing edges) using
//! kurbo's exact curve maths. Everything is in millimetres.

use kurbo::{
    BezPath, CubicBez, Line, ParamCurve, ParamCurveArclen, ParamCurveNearest, PathEl, PathSeg,
    Point, Shape,
};
use opendrape_core::{Edge, Piece, Point2, Vertex};

/// Accuracy (mm) of curve lengths and nearest-point searches.
const ACCURACY: f64 = 1e-4;
/// Longest edge a student can type (10 m), so a mistyped number can't create absurd pieces.
pub const MAX_EDGE_MM: f64 = 10_000.0;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Anchor {
    #[default]
    Start,
    End,
}

fn kp(p: Point2) -> Point {
    Point::new(p.x, p.y)
}

fn cp(p: Point) -> Point2 {
    Point2::new(p.x, p.y)
}

fn edge_seg(piece: &Piece, i: usize) -> PathSeg {
    let (a, b) = piece.edge_ends(i);
    match piece.edges[i] {
        Edge::Line => PathSeg::Line(Line::new(kp(a), kp(b))),
        Edge::Curve { c1, c2 } => PathSeg::Cubic(CubicBez::new(kp(a), kp(c1), kp(c2), kp(b))),
    }
}

fn bez_path(piece: &Piece) -> BezPath {
    let mut path = BezPath::new();
    path.move_to(kp(piece.vertices[0].pos));
    for i in 0..piece.len() {
        let b = kp(piece.edge_ends(i).1);
        match piece.edges[i] {
            Edge::Line => path.line_to(b),
            Edge::Curve { c1, c2 } => path.curve_to(kp(c1), kp(c2), b),
        }
    }
    path.close_path();
    path
}

pub fn edge_length(piece: &Piece, i: usize) -> f64 {
    edge_seg(piece, i).arclen(ACCURACY)
}

pub fn perimeter(piece: &Piece) -> f64 {
    (0..piece.len()).map(|i| edge_length(piece, i)).sum()
}

pub fn area(piece: &Piece) -> f64 {
    bez_path(piece).area().abs()
}

pub fn contains(piece: &Piece, p: Point2) -> bool {
    bez_path(piece).contains(kp(p))
}

pub fn point_on_edge(piece: &Piece, i: usize, t: f64) -> Point2 {
    cp(edge_seg(piece, i).eval(t))
}

/// The edge nearest to `p`: (edge index, curve parameter t, distance in mm).
pub fn nearest_edge(piece: &Piece, p: Point2) -> Option<(usize, f64, f64)> {
    (0..piece.len())
        .map(|i| {
            let n = edge_seg(piece, i).nearest(kp(p), ACCURACY);
            (i, n.t, n.distance_sq.sqrt())
        })
        .min_by(|a, b| a.2.total_cmp(&b.2))
}

fn flatten(path: &BezPath, tolerance: f64) -> Vec<Point2> {
    let mut out = Vec::new();
    kurbo::flatten(path.iter(), tolerance, |el| {
        if let PathEl::MoveTo(p) | PathEl::LineTo(p) = el {
            out.push(cp(p));
        }
    });
    out
}

/// The outline as points no further than `tolerance` mm from the true curve (closed; the first
/// point is not repeated at the end).
pub fn outline_points(piece: &Piece, tolerance: f64) -> Vec<Point2> {
    let mut pts = flatten(&bez_path(piece), tolerance);
    if pts.len() > 1 && pts.first() == pts.last() {
        pts.pop();
    }
    pts
}

/// Edge `i` as points within `tolerance` mm, from its start to its end.
pub fn edge_points(piece: &Piece, i: usize, tolerance: f64) -> Vec<Point2> {
    let mut path = BezPath::new();
    let seg = edge_seg(piece, i);
    path.move_to(seg.start());
    match seg {
        PathSeg::Line(l) => path.line_to(l.p1),
        PathSeg::Cubic(c) => path.curve_to(c.p1, c.p2, c.p3),
        PathSeg::Quad(q) => path.quad_to(q.p1, q.p2),
    }
    flatten(&path, tolerance)
}

/// Area centroid of the piece (where its name and grainline are drawn).
pub fn centroid(piece: &Piece) -> Point2 {
    let pts = outline_points(piece, 0.5);
    let (mut a, mut cx, mut cy) = (0.0, 0.0, 0.0);
    for (k, p) in pts.iter().enumerate() {
        let q = pts[(k + 1) % pts.len()];
        let cross = p.x * q.y - q.x * p.y;
        a += cross;
        cx += (p.x + q.x) * cross;
        cy += (p.y + q.y) * cross;
    }
    if a.abs() < 1e-12 {
        let n = pts.len().max(1) as f64;
        return Point2::new(
            pts.iter().map(|p| p.x).sum::<f64>() / n,
            pts.iter().map(|p| p.y).sum::<f64>() / n,
        );
    }
    Point2::new(cx / (3.0 * a), cy / (3.0 * a))
}

/// Adds a vertex on edge `i` at curve parameter `t`; curves are split exactly. Returns the new
/// vertex's index, or `None` when `t` is within 2% of either end (that would duplicate a vertex).
pub fn split_edge(piece: &mut Piece, i: usize, t: f64) -> Option<usize> {
    if !(0.02..=0.98).contains(&t) {
        return None;
    }
    let seg = edge_seg(piece, i);
    let at = cp(seg.eval(t));
    let (first, second, vertex) = match seg {
        PathSeg::Cubic(c) => {
            let (a, b) = (c.subsegment(0.0..t), c.subsegment(t..1.0));
            (
                Edge::Curve {
                    c1: cp(a.p1),
                    c2: cp(a.p2),
                },
                Edge::Curve {
                    c1: cp(b.p1),
                    c2: cp(b.p2),
                },
                Vertex::smooth(at),
            )
        }
        _ => (Edge::Line, Edge::Line, Vertex::corner(at)),
    };
    piece.edges[i] = first;
    piece.vertices.insert(i + 1, vertex);
    piece.edges.insert(i + 1, second);
    Some(i + 1)
}

/// Changes edge `i` to `length` mm, keeping its `anchor` end fixed. A straight edge keeps its
/// direction; a curve is scaled about the anchor, so its shape is kept. Returns false (piece
/// unchanged) for a length that is not finite, ≤ 0 or above [`MAX_EDGE_MM`], or a zero-length edge.
pub fn set_edge_length(piece: &mut Piece, i: usize, length: f64, anchor: Anchor) -> bool {
    if !(length.is_finite() && length > 0.0 && length <= MAX_EDGE_MM) {
        return false;
    }
    let current = edge_length(piece, i);
    if current < 1e-9 {
        return false;
    }
    let factor = length / current;
    let (a, b) = piece.edge_ends(i);
    let j = piece.next(i);
    let (pivot, moving_vertex, new_pos) = match anchor {
        Anchor::Start => (a, j, a + (b - a) * factor),
        Anchor::End => (b, i, b + (a - b) * factor),
    };
    let curve = piece.edges[i];
    piece.move_vertex(moving_vertex, new_pos);
    if let Edge::Curve { c1, c2 } = curve {
        piece.edges[i] = Edge::Curve {
            c1: pivot + (c1 - pivot) * factor,
            c2: pivot + (c2 - pivot) * factor,
        };
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use opendrape_core::PieceId;

    fn p(x: f64, y: f64) -> Point2 {
        Point2::new(x, y)
    }

    fn square() -> Piece {
        Piece::rectangle(PieceId(1), "S", p(0.0, 0.0), 100.0, 100.0)
    }

    fn close(a: Point2, b: Point2) {
        assert!(a.distance(b) < 1e-9, "{a:?} vs {b:?}");
    }

    #[test]
    fn measures_a_square() {
        let s = square();
        assert!((edge_length(&s, 0) - 100.0).abs() < 1e-9);
        assert!((perimeter(&s) - 400.0).abs() < 1e-9);
        assert!((area(&s) - 10_000.0).abs() < 1e-6);
        assert!(contains(&s, p(50.0, 50.0)) && !contains(&s, p(150.0, 50.0)));
        assert_eq!(centroid(&s), p(50.0, 50.0));
        assert_eq!(point_on_edge(&s, 1, 0.5), p(100.0, 50.0));
    }

    #[test]
    fn measures_a_quarter_circle_curve() {
        // Edge 0 bent into a quarter circle of radius 100 (standard Bézier constant 0.5523).
        let mut s = Piece::polygon(
            PieceId(1),
            "Q",
            &[p(100.0, 0.0), p(0.0, 100.0), p(0.0, 0.0)],
        );
        let k = 0.552_284_75 * 100.0;
        s.edges[0] = Edge::Curve {
            c1: p(100.0, k),
            c2: p(k, 100.0),
        };
        let expected = std::f64::consts::FRAC_PI_2 * 100.0;
        assert!(
            (edge_length(&s, 0) - expected).abs() / expected < 1e-3,
            "{}",
            edge_length(&s, 0)
        );
    }

    #[test]
    fn finds_the_nearest_edge() {
        let (i, t, d) = nearest_edge(&square(), p(40.0, -3.0)).unwrap();
        assert_eq!(i, 0);
        assert!((t - 0.4).abs() < 1e-6 && (d - 3.0).abs() < 1e-6);
    }

    #[test]
    fn flattens_the_outline() {
        let pts = outline_points(&square(), 0.1);
        assert_eq!(
            pts,
            vec![p(0.0, 0.0), p(100.0, 0.0), p(100.0, 100.0), p(0.0, 100.0)]
        );
        let mut s = square();
        s.set_curved(0, true);
        s.set_handle(0, opendrape_core::HandleEnd::Start, p(30.0, -40.0));
        let edge = edge_points(&s, 0, 0.1);
        assert!(edge.len() > 3, "a curve flattens to several points");
        close(edge[0], p(0.0, 0.0));
        close(*edge.last().unwrap(), p(100.0, 0.0));
    }

    #[test]
    fn splitting_a_line_inserts_a_corner() {
        let mut s = square();
        assert_eq!(split_edge(&mut s, 0, 0.25), Some(1));
        assert_eq!(s.len(), 5);
        assert_eq!(s.vertices[1], Vertex::corner(p(25.0, 0.0)));
        assert!((perimeter(&s) - 400.0).abs() < 1e-9);
        assert_eq!(s.check(), Ok(()));
    }

    #[test]
    fn splitting_a_curve_keeps_its_shape() {
        let mut s = square();
        s.set_curved(0, true);
        s.set_handle(0, opendrape_core::HandleEnd::Start, p(30.0, -40.0));
        let before = edge_length(&s, 0);
        let mid = point_on_edge(&s, 0, 0.5);
        assert_eq!(split_edge(&mut s, 0, 0.5), Some(1));
        assert!((edge_length(&s, 0) + edge_length(&s, 1) - before).abs() < 1e-6);
        assert!(s.vertices[1].pos.distance(mid) < 1e-9);
        assert_eq!(s.vertices[1].kind, opendrape_core::VertexKind::Smooth);
    }

    #[test]
    fn refuses_to_split_at_a_vertex() {
        let mut s = square();
        assert_eq!(split_edge(&mut s, 0, 0.005), None);
        assert_eq!(split_edge(&mut s, 0, 0.999), None);
        assert_eq!(s.len(), 4);
    }

    #[test]
    fn setting_a_straight_edge_length_moves_the_free_end() {
        let mut s = square();
        assert!(set_edge_length(&mut s, 0, 150.0, Anchor::Start));
        close(s.vertices[1].pos, p(150.0, 0.0));
        let mut s = square();
        assert!(set_edge_length(&mut s, 0, 60.0, Anchor::End));
        close(s.vertices[0].pos, p(40.0, 0.0));
    }

    #[test]
    fn setting_a_curve_length_scales_it() {
        let mut s = square();
        s.set_curved(0, true);
        s.set_handle(0, opendrape_core::HandleEnd::Start, p(30.0, -40.0));
        let target = edge_length(&s, 0) * 1.5;
        assert!(set_edge_length(&mut s, 0, target, Anchor::Start));
        assert!(
            (edge_length(&s, 0) - target).abs() < 1e-3,
            "lengths are measured to 0.0001 mm"
        );
        assert_eq!(s.vertices[0].pos, p(0.0, 0.0), "anchor fixed");
    }

    #[test]
    fn set_edge_length_rejects_nonsense() {
        for bad in [0.0, -5.0, f64::NAN, f64::INFINITY, MAX_EDGE_MM + 1.0] {
            let mut s = square();
            assert!(!set_edge_length(&mut s, 0, bad, Anchor::Start), "{bad}");
            assert_eq!(s, square());
        }
    }
}
