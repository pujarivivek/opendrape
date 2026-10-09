//! The shapes on the pattern table, with the slow parts of drawing them worked out once and
//! kept until the shape changes or the zoom passes a power of two: the fill triangles, the
//! cut line, the notch marks and the internal lines' points.

use opendrape_core::{LineKind, PieceId, Point2, Project};
use opendrape_geom::{self as geom, Shape};
use std::collections::HashMap;
use std::rc::Rc;

/// Most points an outline, cut line or internal line is drawn with; finer ones are thinned
/// for drawing only, so a deliberately huge piece can't make the window slow.
pub(super) const MAX_DRAWN_POINTS: usize = 4_000;

/// One shape ready to draw. All points are in pattern millimetres.
pub(super) struct Drawn {
    pub shape: Shape,
    pub outline: Vec<Point2>,
    /// Triangles (indices into `outline`) filling the piece.
    pub fill: Vec<u32>,
    /// The cut line and its fill triangles; empty when no edge has any allowance.
    pub cut: Vec<Point2>,
    pub cut_fill: Vec<u32>,
    pub notches: Vec<[Point2; 2]>,
    pub lines: Vec<(Vec<Point2>, LineKind)>,
}

#[derive(Default)]
pub(super) struct ShapeCache {
    entries: HashMap<PieceId, (u64, Rc<Drawn>)>,
    /// How many shapes have been worked out from scratch (for tests).
    pub computed: usize,
}

impl ShapeCache {
    /// Every shape of `project`, ready to draw at `zoom` (screen points per mm). A shape whose
    /// content and zoom bracket haven't changed since the last call is reused as it was.
    pub fn shapes(&mut self, project: &Project, zoom: f64) -> Vec<Rc<Drawn>> {
        let bracket = zoom.max(1e-9).log2().floor() as i32;
        // 0.25 to 0.5 screen points: smooth curves at every zoom.
        let tolerance = 0.25 / 2f64.powi(bracket);
        let mut kept = HashMap::new();
        let mut out = Vec::new();
        for shape in geom::shapes(project) {
            let key = fingerprint(&shape, bracket);
            let id = shape.id;
            let drawn = match self.entries.get(&id) {
                Some((k, d)) if *k == key => d.clone(),
                _ => {
                    self.computed += 1;
                    Rc::new(prepare(shape, tolerance))
                }
            };
            kept.insert(id, (key, drawn.clone()));
            out.push(drawn);
        }
        self.entries = kept;
        out
    }
}

/// Identifies a shape's content: its kind and every field of its piece, plus the zoom bracket.
fn fingerprint(shape: &Shape, bracket: i32) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    bracket.hash(&mut h);
    format!("{:?}", shape.kind).hash(&mut h);
    serde_json::to_vec(&shape.piece)
        .unwrap_or_default()
        .hash(&mut h);
    h.finish()
}

fn prepare(shape: Shape, tolerance: f64) -> Drawn {
    let piece = &shape.piece;
    let outline = thin(geom::outline_points(piece, tolerance));
    let fill = triangulate(&outline);
    let any_allowance = (0..piece.len()).any(|i| piece.edge_allowance(i) > 0.0);
    let cut = if any_allowance {
        thin(geom::cut_line(piece))
    } else {
        Vec::new()
    };
    let cut_fill = triangulate(&cut);
    let notches = piece
        .notches
        .iter()
        .flat_map(|n| geom::notch_marks(piece, n))
        .collect();
    let lines = piece
        .lines
        .iter()
        .map(|l| (thin(geom::line_points(l, tolerance)), l.kind))
        .collect();
    Drawn {
        shape,
        outline,
        fill,
        cut,
        cut_fill,
        notches,
        lines,
    }
}

/// At most [`MAX_DRAWN_POINTS`] of `points`, evenly spaced.
fn thin(points: Vec<Point2>) -> Vec<Point2> {
    if points.len() <= MAX_DRAWN_POINTS {
        return points;
    }
    let step = points.len().div_ceil(MAX_DRAWN_POINTS);
    points.into_iter().step_by(step).collect()
}

/// epaint fills only convex shapes, so pieces are triangulated (earcut) once here.
fn triangulate(points: &[Point2]) -> Vec<u32> {
    let mut triangles = Vec::new();
    if points.len() >= 3 {
        earcut::Earcut::new().earcut(
            points.iter().map(|p| [p.x, p.y]),
            &[] as &[u32],
            &mut triangles,
        );
    }
    triangles
}

#[cfg(test)]
mod tests {
    use super::*;
    use opendrape_core::{Edge, Piece, PieceId, Point2, Project};

    fn project_with(piece: Piece) -> Project {
        let mut p = Project::new();
        p.add_piece(piece);
        p
    }

    #[test]
    fn shapes_are_reused_until_they_change() {
        let mut cache = ShapeCache::default();
        let mut project = project_with(Piece::rectangle(
            PieceId(0),
            "R",
            Point2::new(0.0, 0.0),
            100.0,
            100.0,
        ));
        cache.shapes(&project, 1.0);
        cache.shapes(&project, 1.3); // same power-of-two bracket
        assert_eq!(cache.computed, 1);
        project.pieces[0].translate(Point2::new(1.0, 0.0));
        cache.shapes(&project, 1.3);
        assert_eq!(cache.computed, 2);
        cache.shapes(&project, 2.5); // a new bracket: finer outlines
        assert_eq!(cache.computed, 3);
    }

    #[test]
    fn huge_shapes_are_cached_and_capped() {
        // A within-limits but deliberately heavy piece: 2,000 wildly curved edges.
        let corners: Vec<Point2> = (0..2000)
            .map(|k| {
                let a = k as f64 / 2000.0 * std::f64::consts::TAU;
                Point2::new(5000.0 * a.cos(), 5000.0 * a.sin())
            })
            .collect();
        let mut piece = Piece::polygon(PieceId(0), "Heavy", &corners);
        for i in 0..piece.len() {
            let (a, b) = piece.edge_ends(i);
            piece.edges[i] = Edge::Curve {
                c1: a + Point2::new(900.0, -900.0),
                c2: b + Point2::new(-900.0, 900.0),
            };
        }
        let project = project_with(piece);
        let mut cache = ShapeCache::default();
        let first = cache.shapes(&project, 50.0);
        assert!(
            first[0].outline.len() <= MAX_DRAWN_POINTS && first[0].cut.len() <= MAX_DRAWN_POINTS
        );
        let started = std::time::Instant::now();
        let again = cache.shapes(&project, 50.0);
        assert!(Rc::ptr_eq(&first[0], &again[0]), "not recomputed");
        assert!(
            started.elapsed() < std::time::Duration::from_secs(1),
            "{:?}",
            started.elapsed()
        );
    }
}
