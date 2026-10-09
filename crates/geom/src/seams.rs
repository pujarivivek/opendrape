//! Where seam sides lie on the shapes that show them: which outline edges, which way, how
//! long, and as points to draw.

use crate::shapes::{Shape, ShapeKind};
use crate::{edge_length, edge_points};
use opendrape_core::{Half, Point2, SeamSide};

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
}

/// The outline edges of `shape` that `side` covers, in the order the side runs, each with
/// whether the side runs against that outline edge's own direction. None when the side does
/// not fit the shape (a pale half on a shape that isn't folded, an edge that doesn't exist,
/// the fold edge).
pub fn side_edges(shape: &Shape, side: &SeamSide) -> Option<Vec<(usize, bool)>> {
    let n = shape.stored_len();
    if side.edges == 0 || side.edges > n || side.first_edge >= n {
        return None;
    }
    let mut runs = Vec::with_capacity(side.edges);
    for i in side.stored_edges(n) {
        let run = match (shape.kind, side.half) {
            (ShapeKind::Folded { first, drawn, .. }, half) => {
                if i == (first + drawn - 1) % drawn {
                    return None; // the fold edge
                }
                let m = (i + drawn - first) % drawn;
                match half {
                    Half::Drawn => (m, false),
                    Half::Pale => (2 * drawn - 3 - m, true),
                }
            }
            (_, Half::Pale) => return None,
            (_, Half::Drawn) => (i, false),
        };
        runs.push(run);
    }
    if !side.forward {
        runs.reverse();
        for run in &mut runs {
            run.1 = !run.1;
        }
    }
    Some(runs)
}

/// Length (mm) of a side along the stitching line.
pub fn side_length(shape: &Shape, side: &SeamSide) -> Option<f64> {
    let runs = side_edges(shape, side)?;
    Some(
        runs.iter()
            .map(|(j, _)| edge_length(&shape.piece, *j))
            .sum(),
    )
}

/// The side as points on the shape no further than `tolerance` mm from it, from its start to
/// its end.
pub fn side_points(shape: &Shape, side: &SeamSide, tolerance: f64) -> Option<Vec<Point2>> {
    let mut out: Vec<Point2> = Vec::new();
    for (j, against) in side_edges(shape, side)? {
        let mut pts = edge_points(&shape.piece, j, tolerance);
        if against {
            pts.reverse();
        }
        // Each edge starts where the one before it ended.
        let skip = usize::from(!out.is_empty());
        out.extend(pts.into_iter().skip(skip));
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shapes::shapes;
    use opendrape_core::{Piece, PieceId, Project};

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
        let pale = SeamSide::new(PieceId(1), Half::Pale, 0, 2, true);
        assert_eq!(side_edges(front, &pale), Some(vec![(5, true), (4, true)]));
        let pts = side_points(front, &pale, 0.1).unwrap();
        assert_eq!(pts.len(), 3);
        close(pts[0], p(0.0, 0.0));
        close(pts[1], p(-100.0, 0.0));
        close(pts[2], p(-100.0, 200.0));
        assert!((side_length(front, &pale).unwrap() - 300.0).abs() < 1e-9);
        let back_way = SeamSide {
            forward: false,
            ..pale
        };
        assert_eq!(
            side_edges(front, &back_way),
            Some(vec![(4, false), (5, false)])
        );
        // The fold edge (stored 3) can't be part of a side; nor can a pale half elsewhere.
        assert_eq!(
            side_edges(front, &SeamSide::new(PieceId(1), Half::Drawn, 2, 2, true)),
            None
        );
        assert_eq!(
            side_edges(&all[1], &SeamSide::new(PieceId(2), Half::Pale, 0, 1, true)),
            None
        );
        // A twin side wraps like any other.
        let twin = SeamSide::new(PieceId(3), Half::Drawn, 3, 2, false);
        assert_eq!(side_edges(&all[2], &twin), Some(vec![(0, true), (3, true)]));
    }
}
