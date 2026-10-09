//! The shapes on the pattern table, with the slow parts of drawing them worked out once and
//! kept until the shape changes or the zoom passes a power of two: the fill triangles, the
//! cut line, the notch marks, the internal lines' points, the drawn half of a cut-on-fold
//! piece and where its edge-length labels go.

use opendrape_core::{LineKind, PieceId, Point2, Project};
use opendrape_geom::{self as geom, Shape};
use std::collections::HashMap;
use std::rc::Rc;

/// Most points an outline, cut line or internal line is drawn with; finer ones are thinned
/// for drawing only, so a deliberately huge piece can't make the window slow.
pub(super) const MAX_DRAWN_POINTS: usize = 4_000;

/// Where one edge's length goes, worked out once with the shape. Millimetres.
#[derive(Clone, Copy, Debug)]
pub(super) struct Label {
    /// The edge, as numbered on the shape's outline.
    pub edge: usize,
    /// The point halfway along the edge.
    pub at: Point2,
    /// The unit direction pointing out of the piece there.
    pub out: Point2,
    pub length: f64,
}

/// One shape ready to draw. All points are in pattern millimetres.
pub(super) struct Drawn {
    pub shape: Shape,
    pub outline: Vec<Point2>,
    /// Triangles (indices into `outline`) filling the piece.
    pub fill: Vec<u32>,
    /// A cut-on-fold piece's stored (drawn) half: its edges in order, from the fold's far end
    /// round to its near end. It is open; the fold line closes it, and `half_fill` fills that
    /// polygon. Both are empty for every other shape.
    pub half: Vec<Point2>,
    pub half_fill: Vec<u32>,
    /// The cut line and its fill triangles; empty when no edge has any allowance.
    pub cut: Vec<Point2>,
    pub cut_fill: Vec<u32>,
    pub notches: Vec<[Point2; 2]>,
    pub lines: Vec<(Vec<Point2>, LineKind)>,
    /// A label for every editable edge (not the pale half of a fold).
    pub labels: Vec<Label>,
}

#[derive(Default)]
pub(super) struct ShapeCache {
    entries: HashMap<PieceId, (u64, Rc<Drawn>)>,
    /// How many shapes have been worked out from scratch.
    #[cfg(test)]
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
                    #[cfg(test)]
                    {
                        self.computed += 1;
                    }
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
    let half = match shape.kind {
        geom::ShapeKind::Folded { drawn, .. } => thin_open(half_points(piece, drawn, tolerance)),
        _ => Vec::new(),
    };
    let half_fill = triangulate(&half);
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
        .map(|l| (thin_open(geom::line_points(l, tolerance)), l.kind))
        .collect();
    let labels = geom::edge_label_anchors(piece)
        .into_iter()
        .enumerate()
        .filter(|(j, _)| shape.stored_edge(*j).is_some())
        .map(|(edge, (at, out))| Label {
            edge,
            at,
            out,
            length: geom::edge_length(piece, edge),
        })
        .collect();
    Drawn {
        shape,
        outline,
        fill,
        half,
        half_fill,
        cut,
        cut_fill,
        notches,
        lines,
        labels,
    }
}

/// The first `drawn - 1` edges of an unfolded piece (the stored half's own edges) as one
/// polyline.
fn half_points(piece: &opendrape_core::Piece, drawn: usize, tolerance: f64) -> Vec<Point2> {
    let mut points = Vec::new();
    for j in 0..drawn.saturating_sub(1) {
        let edge = geom::edge_points(piece, j, tolerance);
        // Each edge starts where the one before it ended.
        points.extend(edge.into_iter().skip(usize::from(j > 0)));
    }
    points
}

/// At most [`MAX_DRAWN_POINTS`] of `points`, evenly spaced: for a closed ring, whose last
/// point is next to its first.
fn thin(points: Vec<Point2>) -> Vec<Point2> {
    if points.len() <= MAX_DRAWN_POINTS {
        return points;
    }
    let step = points.len().div_ceil(MAX_DRAWN_POINTS);
    points.into_iter().step_by(step).collect()
}

/// At most [`MAX_DRAWN_POINTS`] of `points`, evenly spaced, for an open line: its first and
/// last points are always kept, so a thinned line still ends where it did.
fn thin_open(points: Vec<Point2>) -> Vec<Point2> {
    if points.len() <= MAX_DRAWN_POINTS {
        return points;
    }
    let last = points.len() - 1;
    let step = last.div_ceil(MAX_DRAWN_POINTS - 1);
    let mut out: Vec<Point2> = points.iter().copied().step_by(step).collect();
    if !last.is_multiple_of(step) {
        out.push(points[last]);
    }
    out
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

    /// The half piece at (300,100), 150 × 300 mm, folded on its left edge: whole it is 300 × 300.
    fn folded_half() -> Piece {
        let mut half = Piece::rectangle(PieceId(0), "H", Point2::new(300.0, 100.0), 150.0, 300.0);
        half.fold = Some(3);
        half
    }

    /// The area the triangles (indices into `points`) cover.
    fn triangles_area(points: &[Point2], triangles: &[u32]) -> f64 {
        triangles
            .chunks(3)
            .map(|t| {
                let [a, b, c] = [t[0], t[1], t[2]].map(|i| points[i as usize]);
                ((b.x - a.x) * (c.y - a.y) - (c.x - a.x) * (b.y - a.y)).abs() / 2.0
            })
            .sum()
    }

    #[test]
    fn a_folded_piece_keeps_its_drawn_half_apart_from_the_whole() {
        let project = project_with(folded_half());
        let drawn = ShapeCache::default().shapes(&project, 1.0);
        let d = &drawn[0];
        assert!((triangles_area(&d.outline, &d.fill) - 90_000.0).abs() < 1e-6);
        // The stored half: edges 0..3 from (300,100) round to (300,400), closed by the fold.
        assert!((triangles_area(&d.half, &d.half_fill) - 45_000.0).abs() < 1e-6);
        assert_eq!(d.half.first(), Some(&Point2::new(300.0, 100.0)));
        assert_eq!(d.half.last(), Some(&Point2::new(300.0, 400.0)));
        // Other shapes have no separate half.
        let plain = project_with(Piece::rectangle(
            PieceId(0),
            "R",
            Point2::new(0.0, 0.0),
            10.0,
            10.0,
        ));
        let drawn = ShapeCache::default().shapes(&plain, 1.0);
        assert!(drawn[0].half.is_empty() && drawn[0].half_fill.is_empty());
    }

    #[test]
    fn labels_are_worked_out_once_for_the_editable_edges() {
        let mut project = project_with(folded_half());
        let id = project.pieces[0].id;
        let mut plain = Piece::rectangle(PieceId(0), "R", Point2::new(0.0, 0.0), 100.0, 50.0);
        plain.allowance = 0.0;
        let master = project.add_piece(plain);
        project
            .add_twin(master, "R2".into(), Point2::new(300.0, 0.0))
            .unwrap();
        let drawn = ShapeCache::default().shapes(&project, 1.0);
        for d in &drawn {
            let piece = &d.shape.piece;
            for label in &d.labels {
                assert!(d.shape.stored_edge(label.edge).is_some());
                let (at, out) = geom::edge_label_anchor(piece, label.edge);
                assert_eq!((label.at, label.out), (at, out));
                assert_eq!(label.length, geom::edge_length(piece, label.edge));
            }
        }
        // A fold's pale half has no labels: 3 of the 6 outline edges are editable.
        let folded = drawn.iter().find(|d| d.shape.id == id).unwrap();
        assert_eq!(
            folded.labels.iter().map(|l| l.edge).collect::<Vec<_>>(),
            [0, 1, 2]
        );
        // A twin's outline runs the other way round: its labels still point out of it.
        let twin = drawn
            .iter()
            .find(|d| matches!(d.shape.kind, geom::ShapeKind::Twin { .. }))
            .unwrap();
        assert_eq!(twin.labels.len(), 4);
        assert_eq!(twin.labels[0].out, Point2::new(0.0, -1.0)); // below the bottom edge
        assert_eq!(twin.labels[0].length, 100.0);
    }

    #[test]
    fn thinning_an_open_line_keeps_its_last_point() {
        let line: Vec<Point2> = (0..10_001)
            .map(|k| Point2::new(f64::from(k), 0.0))
            .collect();
        let thinned = thin_open(line.clone());
        assert!(thinned.len() <= MAX_DRAWN_POINTS);
        assert_eq!(
            (thinned.first(), thinned.last()),
            (line.first(), line.last())
        );
        let short: Vec<Point2> = line[..100].to_vec();
        assert_eq!(thin_open(short.clone()), short);
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
