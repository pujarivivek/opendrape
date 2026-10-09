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

/// Seam allowance a new piece gets (mm).
pub const DEFAULT_ALLOWANCE_MM: f64 = 10.0;
/// Allowance of an edge marked as a hem, unless the edge has its own (mm).
pub const HEM_ALLOWANCE_MM: f64 = 30.0;
/// Widest allowance a piece or edge may have (mm).
pub const MAX_ALLOWANCE_MM: f64 = 100.0;

/// Sewing properties of one outline edge.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct EdgeProps {
    /// This edge's own seam allowance (mm); `None` uses the hem's or the piece's.
    #[serde(default)]
    pub allowance: Option<f64>,
    /// A hem: 3 cm allowance unless the edge has its own, and corners that fold up flat.
    #[serde(default)]
    pub hem: bool,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum NotchStyle {
    /// A straight cut into the allowance.
    #[default]
    Slit,
    /// A small V-shaped cut.
    V,
}

/// A notch: a short mark on an edge showing where pieces line up when they are sewn together.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Notch {
    /// The outline edge it is on.
    pub edge: usize,
    /// Distance (mm) along the stitching line from the edge's start point.
    pub distance: f64,
    /// How many marks, 3 mm apart: 1 (front), 2 (back) or 3.
    #[serde(default = "one_mark")]
    pub marks: u8,
    #[serde(default)]
    pub style: NotchStyle,
}

fn one_mark() -> u8 {
    1
}

impl Notch {
    /// A single slit notch.
    pub fn new(edge: usize, distance: f64) -> Self {
        Self {
            edge,
            distance,
            marks: 1,
            style: NotchStyle::Slit,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LineKind {
    /// Drawn on the fabric: placement, centre front, button, fold or press lines.
    #[default]
    Marking,
    /// A hole that is cut out (closed lines only).
    Cutout,
}

/// A line drawn inside a piece.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct InternalLine {
    pub vertices: Vec<Vertex>,
    /// Edge `i` runs from vertex `i` to vertex `i + 1`; a closed line has one more edge, from
    /// its last vertex back to its first.
    pub edges: Vec<Edge>,
    #[serde(default)]
    pub closed: bool,
    #[serde(default)]
    pub kind: LineKind,
}

impl InternalLine {
    /// An open line of straight edges through `points`.
    pub fn open(points: &[Point2]) -> Self {
        Self {
            vertices: points.iter().map(|&p| Vertex::corner(p)).collect(),
            edges: vec![Edge::Line; points.len().saturating_sub(1)],
            closed: false,
            kind: LineKind::Marking,
        }
    }
    /// A closed shape of straight edges through `points`.
    pub fn polygon(points: &[Point2]) -> Self {
        Self {
            edges: vec![Edge::Line; points.len()],
            closed: true,
            ..Self::open(points)
        }
    }
    /// How many edges a line with this many vertices has.
    pub fn edge_count(&self) -> usize {
        if self.closed {
            self.vertices.len()
        } else {
            self.vertices.len().saturating_sub(1)
        }
    }
    /// Start and end point of edge `i`.
    pub fn edge_ends(&self, i: usize) -> (Point2, Point2) {
        (
            self.vertices[i].pos,
            self.vertices[(i + 1) % self.vertices.len()].pos,
        )
    }
    /// Every vertex and control point.
    pub fn points(&self) -> impl Iterator<Item = Point2> + '_ {
        self.vertices
            .iter()
            .map(|v| v.pos)
            .chain(self.edges.iter().flat_map(|e| match *e {
                Edge::Line => Vec::new(),
                Edge::Curve { c1, c2 } => vec![c1, c2],
            }))
    }
    pub fn translate(&mut self, d: Point2) {
        *self = self.mapped(|p| p + d);
    }
    /// The line with every point (vertices and control points) passed through `f`.
    pub fn mapped(&self, f: impl Fn(Point2) -> Point2) -> Self {
        Self {
            vertices: self
                .vertices
                .iter()
                .map(|v| Vertex {
                    pos: f(v.pos),
                    kind: v.kind,
                })
                .collect(),
            edges: self.edges.iter().map(|e| map_edge(*e, &f)).collect(),
            closed: self.closed,
            kind: self.kind,
        }
    }
    /// The right number of edges for its vertices (at least 2, or 3 when closed), only a
    /// closed line may be a cut-out, and every number finite and within range.
    fn is_valid(&self) -> bool {
        let enough = self.vertices.len() >= if self.closed { 3 } else { 2 };
        enough
            && self.edges.len() == self.edge_count()
            && (self.closed || self.kind == LineKind::Marking)
            && self.points().all(|p| p.is_finite() && within_range(p))
    }
}

fn map_edge(e: Edge, f: &impl Fn(Point2) -> Point2) -> Edge {
    match e {
        Edge::Line => Edge::Line,
        Edge::Curve { c1, c2 } => Edge::Curve {
            c1: f(c1),
            c2: f(c2),
        },
    }
}

/// The mirror-image twin of a piece, for a left/right pair. Its shape is never stored: it is
/// the piece reflected left to right (x → −x) and then moved by `offset`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Twin {
    pub id: PieceId,
    pub name: String,
    pub offset: Point2,
}

/// Which member of a pair an id names.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Side {
    /// The stored piece itself.
    Master,
    /// Its mirror-image twin.
    Twin,
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
    /// Seam allowance (mm) of every edge that has none of its own.
    #[serde(default = "default_allowance")]
    pub allowance: f64,
    /// One entry per edge: its own allowance and whether it is a hem.
    #[serde(default)]
    pub edge_props: Vec<EdgeProps>,
    #[serde(default)]
    pub notches: Vec<Notch>,
    #[serde(default)]
    pub lines: Vec<InternalLine>,
    /// The straight edge that is the fold line of a cut-on-fold piece: only half the piece is
    /// stored, and the other half is its mirror image across this edge.
    #[serde(default)]
    pub fold: Option<usize>,
    /// The mirror-image twin, for a left/right pair.
    #[serde(default)]
    pub twin: Option<Twin>,
}

fn default_grain() -> f64 {
    90.0
}

fn default_allowance() -> f64 {
    DEFAULT_ALLOWANCE_MM
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
            allowance: DEFAULT_ALLOWANCE_MM,
            edge_props: vec![EdgeProps::default(); corners.len()],
            notches: Vec::new(),
            lines: Vec::new(),
            fold: None,
            twin: None,
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
    /// Moves the piece, with its internal lines. A twin stays where it is.
    pub fn translate(&mut self, d: Point2) {
        for v in &mut self.vertices {
            v.pos = v.pos + d;
        }
        for e in &mut self.edges {
            *e = map_edge(*e, &|p| p + d);
        }
        for line in &mut self.lines {
            line.translate(d);
        }
        // The twin is the reflection (x → −x) moved by its offset, so it moved by (−d.x, d.y):
        // shift the offset back.
        if let Some(t) = &mut self.twin {
            t.offset = t.offset + Point2::new(d.x, -d.y);
        }
    }
    /// Seam allowance (mm) of edge `i`: its own, else the hem's for a hem, else the piece's.
    pub fn edge_allowance(&self, i: usize) -> f64 {
        let props = self.edge_props[i];
        props.allowance.unwrap_or(if props.hem {
            HEM_ALLOWANCE_MM
        } else {
            self.allowance
        })
    }
    /// Outline points plus internal-line points: what the size limits count.
    pub fn point_count(&self) -> usize {
        self.vertices.len() + self.lines.iter().map(|l| l.vertices.len()).sum::<usize>()
    }
    /// The piece reflected left to right (x → `offset.x` − x) and moved up by `offset.y`.
    /// Vertex order and edge directions stay, so edge indices and notch distances still apply;
    /// the outline's winding is reversed. The result has no fold and no twin.
    pub fn reflected(&self, offset: Point2) -> Piece {
        let f = |p: Point2| Point2::new(offset.x - p.x, p.y + offset.y);
        Piece {
            id: self.id,
            name: self.name.clone(),
            vertices: self
                .vertices
                .iter()
                .map(|v| Vertex {
                    pos: f(v.pos),
                    kind: v.kind,
                })
                .collect(),
            edges: self.edges.iter().map(|e| map_edge(*e, &f)).collect(),
            grain_deg: (180.0 - self.grain_deg).rem_euclid(360.0),
            allowance: self.allowance,
            edge_props: self.edge_props.clone(),
            notches: self.notches.clone(),
            lines: self.lines.iter().map(|l| l.mapped(f)).collect(),
            fold: None,
            twin: None,
        }
    }
    /// The twin as an ordinary piece (with the twin's id and name), if there is one.
    pub fn twin_shape(&self) -> Option<Piece> {
        let t = self.twin.as_ref()?;
        let mut shape = self.reflected(t.offset);
        shape.id = t.id;
        shape.name = t.name.clone();
        Some(shape)
    }
    /// Splits edge `i` at `vertex` into `first` (up to the new vertex) and `second`; `first_len`
    /// is the length (mm) of the first part. Returns the new vertex's index. The new edge gets
    /// the split edge's sewing properties, and notches past the split move onto it. Where to
    /// split and the curve control points come from `opendrape-geom`.
    pub fn split_edge_at(
        &mut self,
        i: usize,
        vertex: Vertex,
        first: Edge,
        second: Edge,
        first_len: f64,
    ) -> usize {
        self.edges[i] = first;
        self.vertices.insert(i + 1, vertex);
        self.edges.insert(i + 1, second);
        let props = self.edge_props[i];
        self.edge_props.insert(i + 1, props);
        for notch in &mut self.notches {
            if notch.edge > i {
                notch.edge += 1;
            } else if notch.edge == i && notch.distance > first_len {
                notch.edge = i + 1;
                notch.distance -= first_len;
            }
        }
        if let Some(f) = &mut self.fold
            && *f > i
        {
            *f += 1;
        }
        i + 1
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
    /// Removes vertex `i`, joining its two edges into one; `prev_len` is the length (mm) of the
    /// edge before it, so its notches keep their places along the joined edge. The joined edge
    /// keeps that first edge's sewing properties. A fold that loses an end point is cleared.
    /// Refused (false) below 4 vertices.
    pub fn remove_vertex(&mut self, i: usize, prev_len: f64) -> bool {
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
        if self.fold.is_some_and(|f| f == prev || f == i) {
            self.fold = None;
        }
        for notch in &mut self.notches {
            if notch.edge == i {
                notch.edge = prev;
                notch.distance += prev_len;
            }
        }
        self.vertices.remove(i);
        self.edges.remove(i);
        self.edge_props.remove(i);
        let shift = |e: usize| if e > i { e - 1 } else { e };
        for notch in &mut self.notches {
            notch.edge = shift(notch.edge);
        }
        if let Some(f) = &mut self.fold {
            *f = shift(*f);
        }
        true
    }
    /// Everything [`crate::Project::check`] needs of one piece:
    /// - 3 to [`MAX_VERTICES_PER_PIECE`] points (outline plus internal lines), one edge and one
    ///   set of edge properties per vertex, a name of at most [`MAX_NAME_CHARS`] characters;
    /// - only finite numbers, every point within [`MAX_COORDINATE_MM`] of the origin;
    /// - allowances within 0..=[`MAX_ALLOWANCE_MM`], notches on real edges with 1–3 marks,
    ///   well-formed internal lines;
    /// - a fold only on a straight, notch-free edge of an unpaired piece that lies entirely on
    ///   one side of it;
    /// - a twin that is itself a valid piece.
    pub fn check(&self) -> Result<(), ModelError> {
        if self.point_count() > MAX_VERTICES_PER_PIECE {
            return Err(ModelError::TooManyPoints(self.id));
        }
        if self.name.chars().count() > MAX_NAME_CHARS {
            return Err(ModelError::NameTooLong(self.id));
        }
        if self.vertices.len() < 3 {
            return Err(ModelError::TooFewVertices(self.id));
        }
        if self.edges.len() != self.vertices.len() || self.edge_props.len() != self.vertices.len() {
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
        if !in_range {
            return Err(ModelError::OutOfRange(self.id));
        }
        let allowance_ok = |w: f64| w.is_finite() && (0.0..=MAX_ALLOWANCE_MM).contains(&w);
        if !allowance_ok(self.allowance)
            || !self
                .edge_props
                .iter()
                .all(|e| e.allowance.is_none_or(allowance_ok))
        {
            return Err(ModelError::BadAllowance(self.id));
        }
        let notches_ok = self.notches.iter().all(|n| {
            n.edge < self.len()
                && n.distance.is_finite()
                && n.distance >= 0.0
                && (1..=3).contains(&n.marks)
        });
        if !notches_ok {
            return Err(ModelError::BadNotch(self.id));
        }
        if !self.lines.iter().all(InternalLine::is_valid) {
            return Err(ModelError::BadLine(self.id));
        }
        if !self.fold_is_valid() {
            return Err(ModelError::BadFold(self.id));
        }
        if let Some(twin) = self.twin_shape() {
            twin.check()?;
        }
        Ok(())
    }
    /// No fold, or a fold on a straight edge with no notches, on an unpaired piece whose every
    /// point (vertices, control points, internal lines) is on the same side of the fold line.
    fn fold_is_valid(&self) -> bool {
        let Some(f) = self.fold else { return true };
        if f >= self.len() || self.edges[f] != Edge::Line || self.twin.is_some() {
            return false;
        }
        if self.notches.iter().any(|n| n.edge == f) {
            return false;
        }
        let (a, b) = self.edge_ends(f);
        let d = b - a;
        let len = d.length();
        if len < 1e-9 {
            return false;
        }
        let side = |p: Point2| (d.x * (p.y - a.y) - d.y * (p.x - a.x)) / len;
        let outline = self
            .vertices
            .iter()
            .map(|v| v.pos)
            .chain(self.edges.iter().flat_map(|e| match *e {
                Edge::Line => Vec::new(),
                Edge::Curve { c1, c2 } => vec![c1, c2],
            }));
        let lines = self.lines.iter().flat_map(InternalLine::points);
        let (mut left, mut right) = (false, false);
        for p in outline.chain(lines) {
            let s = side(p);
            left |= s > 1e-6;
            right |= s < -1e-6;
        }
        !(left && right)
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
        s.edge_props.push(EdgeProps::default());
        assert!(s.remove_vertex(4, 50.0));
        assert_eq!(s.len(), 4);
        assert_eq!(s.edges.len(), 4);
        assert_eq!(s.edge_ends(3), (p(0.0, 100.0), p(0.0, 0.0)));
        assert_eq!(s.edges[3], Edge::Line);
    }

    #[test]
    fn a_piece_keeps_at_least_three_vertices() {
        let mut t = Piece::polygon(PieceId(1), "T", &[p(0.0, 0.0), p(10.0, 0.0), p(0.0, 10.0)]);
        assert!(!t.remove_vertex(1, 10.0));
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
        assert!(s.remove_vertex(1, 100.0));
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

    #[test]
    fn new_pieces_get_a_one_centimetre_allowance() {
        let mut s = square();
        assert_eq!(s.allowance, DEFAULT_ALLOWANCE_MM);
        assert_eq!(s.edge_props.len(), 4);
        assert_eq!(s.edge_allowance(0), 10.0);
        s.edge_props[0].hem = true;
        assert_eq!(s.edge_allowance(0), HEM_ALLOWANCE_MM);
        s.edge_props[0].allowance = Some(15.0);
        assert_eq!(
            s.edge_allowance(0),
            15.0,
            "an edge's own value wins over the hem's"
        );
        s.allowance = 6.0;
        assert_eq!(s.edge_allowance(1), 6.0);
    }

    #[test]
    fn notches_follow_split_and_removed_edges() {
        let mut s = square(); // edges: 0 (0,0)→(100,0), 1 (100,0)→(100,100), 2, 3
        s.notches = vec![
            Notch::new(0, 10.0),
            Notch::new(0, 60.0),
            Notch::new(1, 40.0),
        ];
        s.edge_props[0].hem = true;
        let v = s.split_edge_at(
            0,
            Vertex::corner(p(25.0, 0.0)),
            Edge::Line,
            Edge::Line,
            25.0,
        );
        assert_eq!(v, 1);
        assert_eq!(s.len(), 5);
        assert_eq!(
            s.notches,
            vec![
                Notch::new(0, 10.0),
                Notch::new(1, 35.0),
                Notch::new(2, 40.0)
            ]
        );
        assert!(s.edge_props[1].hem, "both halves of a hem stay a hem");
        assert!(s.remove_vertex(1, 25.0));
        assert_eq!(
            s.notches,
            vec![
                Notch::new(0, 10.0),
                Notch::new(0, 60.0),
                Notch::new(1, 40.0)
            ]
        );
        assert_eq!(s.edge_props.len(), 4);
        // Removing vertex 0 merges the last edge with edge 0, which becomes the last edge.
        assert!(s.remove_vertex(0, 100.0));
        assert_eq!(
            s.notches,
            vec![
                Notch::new(2, 110.0),
                Notch::new(2, 160.0),
                Notch::new(0, 40.0)
            ]
        );
    }

    #[test]
    fn removing_a_fold_end_clears_the_fold() {
        let mut s = square();
        s.vertices.push(Vertex::corner(p(-20.0, 50.0)));
        s.edges.push(Edge::Line);
        s.edge_props.push(EdgeProps::default());
        s.fold = Some(1);
        assert!(s.remove_vertex(4, 40.0)); // not an end of the fold edge
        assert_eq!(s.fold, Some(1));
        assert!(s.remove_vertex(2, 100.0)); // (100,100) is the far end of edge 1
        assert_eq!(s.fold, None);
    }

    #[test]
    fn a_fold_must_be_straight_one_sided_and_unpaired() {
        let mut s = square();
        s.fold = Some(3); // the left edge (0,100)→(0,0): everything is on its right
        assert_eq!(s.check(), Ok(()));
        s.vertices[2].pos = p(-50.0, 100.0); // now the piece crosses the fold line
        assert_eq!(s.check(), Err(ModelError::BadFold(PieceId(1))));
        let mut curved = square();
        curved.set_curved(3, true);
        curved.fold = Some(3);
        assert_eq!(curved.check(), Err(ModelError::BadFold(PieceId(1))));
        let mut notched = square();
        notched.fold = Some(3);
        notched.notches = vec![Notch::new(3, 10.0)];
        assert_eq!(notched.check(), Err(ModelError::BadFold(PieceId(1))));
        let mut paired = square();
        paired.fold = Some(3);
        paired.twin = Some(Twin {
            id: PieceId(9),
            name: "B".into(),
            offset: p(300.0, 0.0),
        });
        assert_eq!(paired.check(), Err(ModelError::BadFold(PieceId(1))));
    }

    #[test]
    fn check_rejects_bad_details() {
        let mut a = square();
        a.allowance = -1.0;
        assert_eq!(a.check(), Err(ModelError::BadAllowance(PieceId(1))));
        let mut e = square();
        e.edge_props[2].allowance = Some(MAX_ALLOWANCE_MM + 1.0);
        assert_eq!(e.check(), Err(ModelError::BadAllowance(PieceId(1))));
        let mut props = square();
        props.edge_props.pop();
        assert_eq!(
            props.check(),
            Err(ModelError::EdgeCountMismatch(PieceId(1)))
        );
        for bad in [
            Notch::new(4, 1.0),
            Notch::new(0, -1.0),
            Notch::new(0, f64::NAN),
            Notch {
                marks: 4,
                ..Notch::new(0, 1.0)
            },
        ] {
            let mut n = square();
            n.notches = vec![bad];
            assert_eq!(n.check(), Err(ModelError::BadNotch(PieceId(1))), "{bad:?}");
        }
        let mut short = square();
        short.lines = vec![InternalLine::open(&[p(10.0, 10.0)])];
        assert_eq!(short.check(), Err(ModelError::BadLine(PieceId(1))));
        let mut closed_two = square();
        closed_two.lines = vec![InternalLine {
            closed: true,
            ..InternalLine::open(&[p(10.0, 10.0), p(20.0, 20.0)])
        }];
        assert_eq!(closed_two.check(), Err(ModelError::BadLine(PieceId(1))));
        let mut open_cutout = square();
        open_cutout.lines = vec![InternalLine {
            kind: LineKind::Cutout,
            ..InternalLine::open(&[p(10.0, 10.0), p(20.0, 20.0)])
        }];
        assert_eq!(open_cutout.check(), Err(ModelError::BadLine(PieceId(1))));
        let mut far_line = square();
        far_line.lines = vec![InternalLine::open(&[p(10.0, 10.0), p(2e6, 20.0)])];
        assert_eq!(far_line.check(), Err(ModelError::BadLine(PieceId(1))));
        let mut ok = square();
        ok.lines = vec![InternalLine::polygon(&[
            p(10.0, 10.0),
            p(30.0, 10.0),
            p(20.0, 30.0),
        ])];
        assert_eq!(ok.check(), Ok(()));
    }

    #[test]
    fn internal_line_points_count_toward_the_limit() {
        let mut s = square();
        let pts: Vec<Point2> = (0..MAX_VERTICES_PER_PIECE - 4)
            .map(|k| p(1.0 + k as f64 * 0.01, 50.0))
            .collect();
        s.lines = vec![InternalLine::open(&pts)];
        assert_eq!(s.point_count(), MAX_VERTICES_PER_PIECE);
        assert_eq!(s.check(), Ok(()));
        s.lines[0].vertices.push(Vertex::corner(p(90.0, 50.0)));
        s.lines[0].edges.push(Edge::Line);
        assert_eq!(s.check(), Err(ModelError::TooManyPoints(PieceId(1))));
    }

    #[test]
    fn a_twin_is_the_mirror_image_moved_by_its_offset() {
        let mut s = square();
        s.grain_deg = 45.0;
        s.set_curved(0, true);
        s.notches = vec![Notch::new(1, 30.0)];
        s.lines = vec![InternalLine::open(&[p(20.0, 20.0), p(40.0, 30.0)])];
        s.twin = Some(Twin {
            id: PieceId(2),
            name: "Front (mirror)".into(),
            offset: p(300.0, 10.0),
        });
        let t = s.twin_shape().unwrap();
        assert_eq!((t.id, t.name.as_str()), (PieceId(2), "Front (mirror)"));
        assert_eq!(t.vertices[1].pos, p(200.0, 10.0)); // (100,0) → (300-100, 0+10)
        let Edge::Curve { c1, .. } = t.edges[0] else {
            panic!("still curved")
        };
        close(c1, p(300.0 - 100.0 / 3.0, 10.0));
        assert_eq!(t.grain_deg, 135.0);
        assert_eq!(
            t.notches, s.notches,
            "edges keep their direction, so distances stay"
        );
        assert_eq!(t.lines[0].vertices[1].pos, p(260.0, 40.0));
        assert_eq!((t.twin.clone(), t.fold), (None, None));
    }

    #[test]
    fn moving_a_paired_piece_leaves_its_twin_in_place() {
        let mut s = square();
        s.lines = vec![InternalLine::open(&[p(20.0, 20.0), p(40.0, 30.0)])];
        s.twin = Some(Twin {
            id: PieceId(2),
            name: "B".into(),
            offset: p(300.0, 0.0),
        });
        let before = s.twin_shape().unwrap();
        s.translate(p(15.0, -7.0));
        assert_eq!(s.lines[0].vertices[0].pos, p(35.0, 13.0));
        let after = s.twin_shape().unwrap();
        for (a, b) in before.vertices.iter().zip(&after.vertices) {
            close(a.pos, b.pos);
        }
    }

    #[test]
    fn old_files_get_defaults_for_the_new_fields() {
        let json = r#"{"id":1,"name":"A","vertices":[{"pos":{"x":0,"y":0}},{"pos":{"x":1,"y":0}},{"pos":{"x":0,"y":1}}],"edges":[{"type":"line"},{"type":"line"},{"type":"line"}]}"#;
        let piece: Piece = serde_json::from_str(json).unwrap();
        assert_eq!(piece.allowance, DEFAULT_ALLOWANCE_MM);
        assert!(piece.edge_props.is_empty() && piece.notches.is_empty() && piece.lines.is_empty());
        assert_eq!((piece.fold, piece.twin), (None, None));
    }
}
