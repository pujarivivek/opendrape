use crate::ModelError;
use serde::{Deserialize, Serialize};
use std::ops::{Add, Mul, Sub};

/// A position on the pattern table in millimetres, y up. Also used as a 2D vector.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Point2 {
    pub x: f64,
    pub y: f64,
}

impl Point2 {
    pub const fn new(x: f64, y: f64) -> Self {
        Self { x, y }
    }
    pub fn distance(self, other: Self) -> f64 {
        (self.x - other.x).hypot(self.y - other.y)
    }
    pub fn lerp(self, other: Self, t: f64) -> Self {
        self + (other - self) * t
    }
    pub fn length(self) -> f64 {
        self.x.hypot(self.y)
    }
    pub fn is_finite(self) -> bool {
        self.x.is_finite() && self.y.is_finite()
    }
}

impl Add for Point2 {
    type Output = Self;
    fn add(self, o: Self) -> Self {
        Self::new(self.x + o.x, self.y + o.y)
    }
}

impl Sub for Point2 {
    type Output = Self;
    fn sub(self, o: Self) -> Self {
        Self::new(self.x - o.x, self.y - o.y)
    }
}

impl Mul<f64> for Point2 {
    type Output = Self;
    fn mul(self, k: f64) -> Self {
        Self::new(self.x * k, self.y * k)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct PieceId(pub u32);

/// A corner's two curve handles move independently; a smooth point keeps them in line.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum VertexKind {
    #[default]
    Corner,
    Smooth,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Vertex {
    pub pos: Point2,
    #[serde(default)]
    pub kind: VertexKind,
}

impl Vertex {
    pub fn corner(pos: Point2) -> Self {
        Self {
            pos,
            kind: VertexKind::Corner,
        }
    }
    pub fn smooth(pos: Point2) -> Self {
        Self {
            pos,
            kind: VertexKind::Smooth,
        }
    }
}

/// Edge `i` of a piece runs from vertex `i` to vertex `(i + 1) % n`. A curve is a cubic Bézier
/// with control points `c1` (near the start) and `c2` (near the end).
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum Edge {
    #[default]
    Line,
    Curve {
        c1: Point2,
        c2: Point2,
    },
}

/// Which end of an edge a control point belongs to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HandleEnd {
    Start,
    End,
}

/// Largest distance (mm) a point may lie from the pattern origin on either axis: 1 km. Anything
/// further only comes from a corrupt or hostile file, and would overwhelm the curve maths.
pub const MAX_COORDINATE_MM: f64 = 1_000_000.0;

/// Most points one piece may have. Real pattern pieces have a few dozen; the limit keeps a
/// corrupt or hostile file from making the window take minutes to draw one piece.
pub const MAX_VERTICES_PER_PIECE: usize = 2_000;

/// Longest piece name, in characters.
pub const MAX_NAME_CHARS: usize = 200;

/// One closed pattern piece.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Piece {
    pub id: PieceId,
    pub name: String,
    pub vertices: Vec<Vertex>,
    pub edges: Vec<Edge>,
    /// Grain direction in degrees anticlockwise from +x; 90 runs straight up the piece.
    #[serde(default = "default_grain")]
    pub grain_deg: f64,
}

fn default_grain() -> f64 {
    90.0
}

impl Piece {
    /// A closed piece through `corners`, all edges straight.
    pub fn polygon(id: PieceId, name: impl Into<String>, corners: &[Point2]) -> Self {
        Self {
            id,
            name: name.into(),
            vertices: corners.iter().map(|&c| Vertex::corner(c)).collect(),
            edges: vec![Edge::Line; corners.len()],
            grain_deg: default_grain(),
        }
    }
    /// Counter-clockwise rectangle with its lower-left corner at `min`.
    pub fn rectangle(
        id: PieceId,
        name: impl Into<String>,
        min: Point2,
        width: f64,
        height: f64,
    ) -> Self {
        let corners = [
            min,
            min + Point2::new(width, 0.0),
            min + Point2::new(width, height),
            min + Point2::new(0.0, height),
        ];
        Self::polygon(id, name, &corners)
    }
    pub fn len(&self) -> usize {
        self.vertices.len()
    }
    pub fn is_empty(&self) -> bool {
        self.vertices.is_empty()
    }
    pub fn next(&self, i: usize) -> usize {
        (i + 1) % self.len()
    }
    pub fn prev(&self, i: usize) -> usize {
        (i + self.len() - 1) % self.len()
    }
    /// Start and end point of edge `i`.
    pub fn edge_ends(&self, i: usize) -> (Point2, Point2) {
        (self.vertices[i].pos, self.vertices[self.next(i)].pos)
    }
    /// Moves vertex `i` to `to`. The control points next to it move along, so the curves keep
    /// their shape near the point.
    pub fn move_vertex(&mut self, i: usize, to: Point2) {
        let d = to - self.vertices[i].pos;
        self.vertices[i].pos = to;
        let p = self.prev(i);
        if let Edge::Curve { c2, .. } = &mut self.edges[p] {
            *c2 = *c2 + d;
        }
        if let Edge::Curve { c1, .. } = &mut self.edges[i] {
            *c1 = *c1 + d;
        }
    }
    pub fn translate(&mut self, d: Point2) {
        for v in &mut self.vertices {
            v.pos = v.pos + d;
        }
        for e in &mut self.edges {
            if let Edge::Curve { c1, c2 } = e {
                *c1 = *c1 + d;
                *c2 = *c2 + d;
            }
        }
    }
    /// Turns edge `i` into a curve (control points at its thirds) or back into a straight line.
    pub fn set_curved(&mut self, i: usize, curved: bool) {
        let (a, b) = self.edge_ends(i);
        self.edges[i] = match (curved, self.edges[i]) {
            (true, Edge::Line) => Edge::Curve {
                c1: a.lerp(b, 1.0 / 3.0),
                c2: a.lerp(b, 2.0 / 3.0),
            },
            (true, curve) => curve,
            (false, _) => Edge::Line,
        };
    }
    /// Moves one control point of curve edge `i`. At a smooth vertex, the control point on the
    /// other side turns to stay in line (keeping its own length).
    pub fn set_handle(&mut self, i: usize, end: HandleEnd, to: Point2) {
        let Edge::Curve { c1, c2 } = &mut self.edges[i] else {
            return;
        };
        match end {
            HandleEnd::Start => *c1 = to,
            HandleEnd::End => *c2 = to,
        }
        let (v, other_edge, other_end) = match end {
            HandleEnd::Start => (i, self.prev(i), HandleEnd::End),
            HandleEnd::End => (self.next(i), self.next(i), HandleEnd::Start),
        };
        if self.vertices[v].kind != VertexKind::Smooth {
            return;
        }
        let pivot = self.vertices[v].pos;
        let away = pivot - to;
        let len = away.length();
        if len < 1e-9 {
            return;
        }
        if let Edge::Curve { c1, c2 } = &mut self.edges[other_edge] {
            let other = match other_end {
                HandleEnd::Start => c1,
                HandleEnd::End => c2,
            };
            let keep = (*other - pivot).length();
            *other = pivot + away * (keep / len);
        }
    }
    /// Makes vertex `i` smooth (its handles brought in line) or a corner.
    pub fn set_vertex_kind(&mut self, i: usize, kind: VertexKind) {
        self.vertices[i].kind = kind;
        if kind == VertexKind::Smooth
            && let Edge::Curve { c1, .. } = self.edges[i]
        {
            self.set_handle(i, HandleEnd::Start, c1);
        }
    }
    /// Removes vertex `i`, joining its two edges into one. Refused (false) below 4 vertices.
    pub fn remove_vertex(&mut self, i: usize) -> bool {
        let n = self.len();
        if n <= 3 {
            return false;
        }
        let prev = self.prev(i);
        let (a, b) = (self.vertices[prev].pos, self.vertices[self.next(i)].pos);
        let merged = match (self.edges[prev], self.edges[i]) {
            (Edge::Line, Edge::Line) => Edge::Line,
            (first, second) => Edge::Curve {
                c1: match first {
                    Edge::Curve { c1, .. } => c1,
                    Edge::Line => a.lerp(b, 1.0 / 3.0),
                },
                c2: match second {
                    Edge::Curve { c2, .. } => c2,
                    Edge::Line => a.lerp(b, 2.0 / 3.0),
                },
            },
        };
        self.edges[prev] = merged;
        self.vertices.remove(i);
        self.edges.remove(i);
        true
    }
    /// 3 to [`MAX_VERTICES_PER_PIECE`] vertices, one edge per vertex, a name of at most
    /// [`MAX_NAME_CHARS`] characters, only finite numbers, and every point within
    /// [`MAX_COORDINATE_MM`] of the origin.
    pub fn check(&self) -> Result<(), ModelError> {
        if self.vertices.len() > MAX_VERTICES_PER_PIECE {
            return Err(ModelError::TooManyPoints(self.id));
        }
        if self.name.chars().count() > MAX_NAME_CHARS {
            return Err(ModelError::NameTooLong(self.id));
        }
        if self.vertices.len() < 3 {
            return Err(ModelError::TooFewVertices(self.id));
        }
        if self.edges.len() != self.vertices.len() {
            return Err(ModelError::EdgeCountMismatch(self.id));
        }
        let finite = self.grain_deg.is_finite()
            && self.vertices.iter().all(|v| v.pos.is_finite())
            && self.edges.iter().all(|e| match e {
                Edge::Line => true,
                Edge::Curve { c1, c2 } => c1.is_finite() && c2.is_finite(),
            });
        if !finite {
            return Err(ModelError::NotFinite(self.id));
        }
        let in_range = self.vertices.iter().all(|v| within_range(v.pos))
            && self.edges.iter().all(|e| match e {
                Edge::Line => true,
                Edge::Curve { c1, c2 } => within_range(*c1) && within_range(*c2),
            });
        if in_range {
            Ok(())
        } else {
            Err(ModelError::OutOfRange(self.id))
        }
    }
}

/// Whether both coordinates of `p` are within [`MAX_COORDINATE_MM`] of the origin.
fn within_range(p: Point2) -> bool {
    p.x.abs() <= MAX_COORDINATE_MM && p.y.abs() <= MAX_COORDINATE_MM
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(x: f64, y: f64) -> Point2 {
        Point2::new(x, y)
    }

    fn square() -> Piece {
        Piece::rectangle(PieceId(1), "Front", p(0.0, 0.0), 100.0, 100.0)
    }

    /// Thirds (1/3, 2/3) are not exact in binary, so compare points with a tolerance.
    fn close(a: Point2, b: Point2) {
        assert!(a.distance(b) < 1e-9, "{a:?} vs {b:?}");
    }

    #[test]
    fn rectangle_is_counter_clockwise_with_straight_edges() {
        let s = square();
        let corners: Vec<Point2> = s.vertices.iter().map(|v| v.pos).collect();
        assert_eq!(
            corners,
            vec![p(0.0, 0.0), p(100.0, 0.0), p(100.0, 100.0), p(0.0, 100.0)]
        );
        assert!(s.edges.iter().all(|e| *e == Edge::Line));
        assert_eq!(s.grain_deg, 90.0);
        assert_eq!(s.edge_ends(3), (p(0.0, 100.0), p(0.0, 0.0)));
    }

    #[test]
    fn moving_a_vertex_carries_its_curve_handles() {
        let mut s = square();
        s.set_curved(0, true); // edge 0: (0,0) → (100,0)
        s.set_curved(3, true); // edge 3: (0,100) → (0,0)
        s.move_vertex(0, p(10.0, 5.0));
        assert_eq!(s.vertices[0].pos, p(10.0, 5.0));
        let Edge::Curve { c1, .. } = s.edges[0] else {
            panic!()
        };
        close(c1, p(100.0 / 3.0 + 10.0, 5.0));
        let Edge::Curve { c2, .. } = s.edges[3] else {
            panic!()
        };
        close(c2, p(10.0, 100.0 / 3.0 + 5.0));
    }

    #[test]
    fn edges_switch_between_straight_and_curved() {
        let mut s = square();
        s.set_curved(1, true);
        let Edge::Curve { c1, c2 } = s.edges[1] else {
            panic!("edge 1 should be curved")
        };
        close(c1, p(100.0, 100.0 / 3.0));
        close(c2, p(100.0, 200.0 / 3.0));
        s.set_curved(1, false);
        assert_eq!(s.edges[1], Edge::Line);
    }

    #[test]
    fn smooth_points_keep_their_handles_in_line() {
        let mut s = square();
        s.set_curved(0, true);
        s.set_curved(1, true);
        s.set_vertex_kind(1, VertexKind::Smooth); // the vertex at (100,0) between edges 0 and 1
        s.set_handle(0, HandleEnd::End, p(80.0, -20.0)); // edge 0's control point near (100,0)
        let Edge::Curve { c1: other, .. } = s.edges[1] else {
            panic!()
        };
        let (a, b) = (p(80.0, -20.0) - p(100.0, 0.0), other - p(100.0, 0.0));
        assert!((a.x * b.y - a.y * b.x).abs() < 1e-9, "collinear");
        assert!(a.x * b.x + a.y * b.y < 0.0, "on opposite sides");
        assert!(
            (b.length() - 100.0 / 3.0).abs() < 1e-9,
            "kept its own length"
        );
    }

    #[test]
    fn removing_a_vertex_joins_its_edges() {
        let mut s = square();
        s.vertices.push(Vertex::corner(p(-20.0, 50.0)));
        s.edges.push(Edge::Line); // pentagon
        assert!(s.remove_vertex(4));
        assert_eq!(s.len(), 4);
        assert_eq!(s.edges.len(), 4);
        assert_eq!(s.edge_ends(3), (p(0.0, 100.0), p(0.0, 0.0)));
        assert_eq!(s.edges[3], Edge::Line);
    }

    #[test]
    fn a_piece_keeps_at_least_three_vertices() {
        let mut t = Piece::polygon(PieceId(1), "T", &[p(0.0, 0.0), p(10.0, 0.0), p(0.0, 10.0)]);
        assert!(!t.remove_vertex(1));
        assert_eq!(t.len(), 3);
    }

    #[test]
    fn removing_a_vertex_between_curves_keeps_the_outer_handles() {
        let mut s = square();
        s.set_curved(0, true);
        s.set_curved(1, true);
        let Edge::Curve { c1: keep1, .. } = s.edges[0] else {
            panic!()
        };
        let Edge::Curve { c2: keep2, .. } = s.edges[1] else {
            panic!()
        };
        assert!(s.remove_vertex(1));
        assert_eq!(
            s.edges[0],
            Edge::Curve {
                c1: keep1,
                c2: keep2
            }
        );
    }

    #[test]
    fn check_rejects_broken_pieces() {
        assert_eq!(square().check(), Ok(()));
        let mut two = square();
        two.vertices.truncate(2);
        two.edges.truncate(2);
        assert_eq!(two.check(), Err(ModelError::TooFewVertices(PieceId(1))));
        let mut mismatch = square();
        mismatch.edges.pop();
        assert_eq!(
            mismatch.check(),
            Err(ModelError::EdgeCountMismatch(PieceId(1)))
        );
        let mut nan = square();
        nan.vertices[2].pos.x = f64::NAN;
        assert_eq!(nan.check(), Err(ModelError::NotFinite(PieceId(1))));
        let mut far = square();
        far.vertices[2].pos.x = 2e6;
        assert_eq!(far.check(), Err(ModelError::OutOfRange(PieceId(1))));
        let mut far_handle = square();
        far_handle.set_curved(0, true);
        far_handle.set_handle(0, HandleEnd::Start, p(-2e6, 0.0));
        assert_eq!(far_handle.check(), Err(ModelError::OutOfRange(PieceId(1))));
    }

    #[test]
    fn check_rejects_oversized_pieces() {
        let ring = |n: usize| {
            let corners: Vec<Point2> = (0..n)
                .map(|k| {
                    let a = k as f64 / n as f64 * std::f64::consts::TAU;
                    p(1000.0 * a.cos(), 1000.0 * a.sin())
                })
                .collect();
            Piece::polygon(PieceId(1), "Ring", &corners)
        };
        assert_eq!(ring(MAX_VERTICES_PER_PIECE).check(), Ok(()));
        assert_eq!(
            ring(MAX_VERTICES_PER_PIECE + 1).check(),
            Err(ModelError::TooManyPoints(PieceId(1)))
        );
        let mut named = square();
        named.name = "n".repeat(MAX_NAME_CHARS);
        assert_eq!(named.check(), Ok(()));
        named.name.push('n');
        assert_eq!(named.check(), Err(ModelError::NameTooLong(PieceId(1))));
        // Characters, not bytes: 200 two-byte letters are fine.
        named.name = "é".repeat(MAX_NAME_CHARS);
        assert_eq!(named.check(), Ok(()));
    }

    #[test]
    fn serialises_edges_with_a_type_tag() {
        let mut s = square();
        s.set_curved(0, true);
        let json = serde_json::to_string(&s).unwrap();
        assert!(
            json.contains(r#""type":"curve""#) && json.contains(r#""type":"line""#),
            "{json}"
        );
        let back: Piece = serde_json::from_str(&json).unwrap();
        assert_eq!(back, s);
    }
}
