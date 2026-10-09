//! What each stored piece shows on the pattern table. A cut-on-fold piece is stored as half
//! and shown whole; a paired piece also shows its mirror-image twin. Each [`Shape`] is a
//! concrete piece, and remembers how its points and indices map back to what is stored,
//! because edits always go to the stored piece.

use crate::edge_length;
use opendrape_core::{Edge, EdgeProps, Notch, Piece, PieceId, Point2, Project, Vertex};

/// One thing drawn on the pattern table.
#[derive(Clone, Debug, PartialEq)]
pub struct Shape {
    /// The id this shape is selected by: the piece's own, or its twin's.
    pub id: PieceId,
    /// The stored piece it comes from.
    pub source: PieceId,
    pub kind: ShapeKind,
    /// The full outline as an ordinary piece (unfolded, or reflected and moved), with its
    /// edges' allowances, its notches and its internal lines in place.
    pub piece: Piece,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ShapeKind {
    /// The stored piece as it is.
    Plain,
    /// A cut-on-fold piece. Outline vertices `0..drawn` are the stored half's, starting at
    /// stored vertex `first` (the fold's far end); the rest are its pale mirror image. `fold`
    /// is the fold line, from its near end to its far end.
    Folded {
        first: usize,
        drawn: usize,
        fold: (Point2, Point2),
    },
    /// A twin: the stored piece reflected left to right, then moved by `offset`.
    Twin { offset: Point2 },
}

impl Shape {
    /// The stored piece's vertex for outline vertex `k`; `None` on the pale half of a fold.
    pub fn stored_vertex(&self, k: usize) -> Option<usize> {
        match self.kind {
            ShapeKind::Plain | ShapeKind::Twin { .. } => Some(k),
            ShapeKind::Folded { first, drawn, .. } => (k < drawn).then_some((first + k) % drawn),
        }
    }
    /// The stored piece's edge for outline edge `j`; `None` on the pale half of a fold.
    pub fn stored_edge(&self, j: usize) -> Option<usize> {
        match self.kind {
            ShapeKind::Plain | ShapeKind::Twin { .. } => Some(j),
            ShapeKind::Folded { first, drawn, .. } => {
                (j + 1 < drawn).then_some((first + j) % drawn)
            }
        }
    }
    /// The outline vertex showing stored vertex `i`.
    pub fn shape_vertex(&self, i: usize) -> usize {
        match self.kind {
            ShapeKind::Plain | ShapeKind::Twin { .. } => i,
            ShapeKind::Folded { first, drawn, .. } => (i + drawn - first) % drawn,
        }
    }
    /// The outline edge showing stored edge `i` (for a fold, `i` must not be the fold edge).
    pub fn shape_edge(&self, i: usize) -> usize {
        self.shape_vertex(i)
    }
    /// A point of this shape in the stored piece's coordinates.
    pub fn to_stored(&self, p: Point2) -> Point2 {
        match self.kind {
            ShapeKind::Twin { offset } => Point2::new(offset.x - p.x, p.y - offset.y),
            _ => p,
        }
    }
    /// A point of the stored piece where this shape shows it.
    pub fn from_stored(&self, p: Point2) -> Point2 {
        match self.kind {
            ShapeKind::Twin { offset } => Point2::new(offset.x - p.x, p.y + offset.y),
            _ => p,
        }
    }
    /// A movement on this shape as a movement of the stored piece.
    pub fn to_stored_delta(&self, d: Point2) -> Point2 {
        match self.kind {
            ShapeKind::Twin { .. } => Point2::new(-d.x, d.y),
            _ => d,
        }
    }
}

/// Every shape on the table, in drawing order: each stored piece, then its twin.
pub fn shapes(project: &Project) -> Vec<Shape> {
    let mut out = Vec::new();
    for piece in &project.pieces {
        out.push(main_shape(piece));
        if let (Some(t), Some(twin)) = (&piece.twin, piece.twin_shape()) {
            out.push(Shape {
                id: t.id,
                source: piece.id,
                kind: ShapeKind::Twin { offset: t.offset },
                piece: twin,
            });
        }
    }
    out
}

/// The shape selected by `id` (a piece's or a twin's).
pub fn shape_of(project: &Project, id: PieceId) -> Option<Shape> {
    shapes(project).into_iter().find(|s| s.id == id)
}

/// The whole piece a cut-on-fold half makes (a copy of `piece` when it has no fold).
pub fn unfolded(piece: &Piece) -> Piece {
    main_shape(piece).piece
}

fn main_shape(piece: &Piece) -> Shape {
    let (full, kind) = match piece.fold {
        Some(f) if f < piece.len() => unfold(piece, f),
        _ => (piece.clone(), ShapeKind::Plain),
    };
    Shape {
        id: piece.id,
        source: piece.id,
        kind,
        piece: full,
    }
}

/// Mirror image of `p` across the line through `a` and `b`.
fn reflect_across(p: Point2, a: Point2, b: Point2) -> Point2 {
    let d = b - a;
    let t = ((p.x - a.x) * d.x + (p.y - a.y) * d.y) / (d.x * d.x + d.y * d.y);
    let foot = a + d * t;
    foot * 2.0 - p
}

fn unfold(piece: &Piece, f: usize) -> (Piece, ShapeKind) {
    let n = piece.len();
    let first = (f + 1) % n;
    let (near, far) = piece.edge_ends(f);
    let mirror = |p: Point2| reflect_across(p, near, far);
    let u = |k: usize| (first + k) % n;

    let mut vertices: Vec<Vertex> = (0..n).map(|k| piece.vertices[u(k)]).collect();
    let mut edges: Vec<Edge> = (0..n - 1).map(|m| piece.edges[u(m)]).collect();
    let mut props: Vec<EdgeProps> = (0..n - 1).map(|m| piece.edge_props[u(m)]).collect();
    for k in (1..n - 1).rev() {
        let v = piece.vertices[u(k)];
        vertices.push(Vertex {
            pos: mirror(v.pos),
            kind: v.kind,
        });
    }
    for m in (0..n - 1).rev() {
        edges.push(match piece.edges[u(m)] {
            Edge::Line => Edge::Line,
            Edge::Curve { c1, c2 } => Edge::Curve {
                c1: mirror(c2),
                c2: mirror(c1),
            },
        });
        props.push(piece.edge_props[u(m)]);
    }
    let drawn_index = |stored: usize| (stored + n - first) % n;
    let mut notches = Vec::new();
    for notch in &piece.notches {
        let m = drawn_index(notch.edge);
        notches.push(Notch { edge: m, ..*notch });
    }
    for notch in &piece.notches {
        let m = drawn_index(notch.edge);
        let len = edge_length(piece, notch.edge);
        notches.push(Notch {
            edge: 2 * n - 3 - m,
            distance: (len - notch.distance).max(0.0),
            ..*notch
        });
    }
    let mut lines = piece.lines.clone();
    lines.extend(piece.lines.iter().map(|l| l.mapped(mirror)));
    let full = Piece {
        id: piece.id,
        name: piece.name.clone(),
        vertices,
        edges,
        grain_deg: piece.grain_deg,
        allowance: piece.allowance,
        edge_props: props,
        notches,
        lines,
        fold: None,
        twin: None,
    };
    (
        full,
        ShapeKind::Folded {
            first,
            drawn: n,
            fold: (near, far),
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use opendrape_core::{Edge, InternalLine, Notch, PieceId, Point2, Project};

    fn p(x: f64, y: f64) -> Point2 {
        Point2::new(x, y)
    }

    /// The right half of a 200 × 200 square, folded on its left edge (edge 3: (0,200)→(0,0)).
    fn half() -> Piece {
        let mut s = Piece::rectangle(PieceId(1), "Front", p(0.0, 0.0), 100.0, 200.0);
        s.fold = Some(3);
        s
    }

    fn close(a: Point2, b: Point2) {
        assert!(a.distance(b) < 1e-9, "{a:?} vs {b:?}");
    }

    #[test]
    fn unfolding_mirrors_the_half_across_the_fold() {
        let full = unfolded(&half());
        let corners: Vec<Point2> = full.vertices.iter().map(|v| v.pos).collect();
        let expected = [
            p(0.0, 0.0),
            p(100.0, 0.0),
            p(100.0, 200.0),
            p(0.0, 200.0),
            p(-100.0, 200.0),
            p(-100.0, 0.0),
        ];
        assert_eq!(corners.len(), expected.len());
        for (a, b) in corners.iter().zip(expected) {
            close(*a, b);
        }
        assert_eq!(full.edges.len(), 6);
        assert_eq!(full.edge_props.len(), 6);
        assert_eq!((full.fold, full.check()), (None, Ok(())));
        assert!((crate::area(&full) - 40_000.0).abs() < 1e-6);
    }

    #[test]
    fn unfolding_maps_indices_and_reverses_curves() {
        let mut h = half();
        h.set_curved(1, true); // right edge (100,0)→(100,200)
        h.notches = vec![Notch::new(0, 30.0)];
        h.lines = vec![InternalLine::open(&[p(20.0, 50.0), p(60.0, 50.0)])];
        let s = shape_of(
            &{
                let mut pr = Project::new();
                pr.add_piece(h.clone());
                pr
            },
            PieceId(1),
        )
        .unwrap();
        let ShapeKind::Folded { first, drawn, fold } = s.kind else {
            panic!("folded")
        };
        assert_eq!((first, drawn), (0, 4));
        close(fold.0, p(0.0, 200.0));
        close(fold.1, p(0.0, 0.0));
        assert_eq!((s.stored_vertex(2), s.stored_vertex(4)), (Some(2), None));
        assert_eq!((s.stored_edge(2), s.stored_edge(3)), (Some(2), None));
        assert_eq!((s.shape_vertex(1), s.shape_edge(1)), (1, 1));
        // Mirror of drawn edge 1 is outline edge 2n-3-1 = 4: from (-100,200) to (-100,0).
        let Edge::Curve { c1, c2 } = s.piece.edges[4] else {
            panic!("curved")
        };
        let Edge::Curve { c1: o1, c2: o2 } = h.edges[1] else {
            panic!()
        };
        close(c1, p(-o2.x, o2.y));
        close(c2, p(-o1.x, o1.y));
        // The bottom notch at 30 mm appears on both halves, the mirrored copy measured from
        // the mirrored edge's start (-100,0): 100 - 30 = 70 mm along.
        assert_eq!(
            s.piece.notches,
            vec![Notch::new(0, 30.0), Notch::new(5, 70.0)]
        );
        assert_eq!(s.piece.lines.len(), 2);
        close(s.piece.lines[1].vertices[1].pos, p(-60.0, 50.0));
    }

    #[test]
    fn folds_on_any_edge_map_back() {
        let mut s = Piece::rectangle(PieceId(1), "Back", p(0.0, 0.0), 100.0, 200.0);
        s.fold = Some(1); // right edge (100,0)→(100,200)
        let mut pr = Project::new();
        pr.add_piece(s);
        let shape = shape_of(&pr, PieceId(1)).unwrap();
        let ShapeKind::Folded { first, .. } = shape.kind else {
            panic!()
        };
        assert_eq!(first, 2);
        assert_eq!(shape.stored_vertex(0), Some(2)); // outline starts at (100,200)
        close(shape.piece.vertices[0].pos, p(100.0, 200.0));
        assert_eq!(shape.shape_vertex(2), 0);
        assert_eq!(shape.shape_edge(3), 1);
        close(shape.piece.vertices[4].pos, p(200.0, 0.0));
    }

    #[test]
    fn twins_follow_their_piece_and_map_back() {
        let mut pr = Project::new();
        let a = pr.add_piece(Piece::rectangle(
            PieceId(0),
            "Back",
            p(0.0, 0.0),
            100.0,
            200.0,
        ));
        let t = pr
            .add_twin(a, "Back (mirror)".into(), p(300.0, 0.0))
            .unwrap();
        let all = shapes(&pr);
        assert_eq!(all.len(), 2);
        assert_eq!((all[0].id, all[0].kind), (a, ShapeKind::Plain));
        let twin = &all[1];
        assert_eq!((twin.id, twin.source), (t, a));
        assert_eq!(
            twin.kind,
            ShapeKind::Twin {
                offset: p(300.0, 0.0)
            }
        );
        close(twin.piece.vertices[1].pos, p(200.0, 0.0));
        let q = p(123.0, 45.0);
        close(twin.to_stored(twin.from_stored(q)), q);
        close(twin.to_stored_delta(p(5.0, 7.0)), p(-5.0, 7.0));
        assert_eq!((twin.stored_vertex(3), twin.shape_edge(2)), (Some(3), 2));
    }
}
