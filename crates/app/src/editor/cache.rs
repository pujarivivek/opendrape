//! The shapes on the pattern table, with the slow parts of drawing them worked out once and
//! kept until they can change. Two kinds of part, kept apart:
//! - what depends on the zoom: the outline and its fill triangles, the internal lines' points
//!   and the drawn half of a cut-on-fold piece. These are redone when the shape changes or
//!   the zoom passes a power of two, because curves are flattened more finely the further in
//!   you are;
//! - what does not: where the edge-length labels go, the cut line with its band fill, and the
//!   notch marks. These are kept for as long as the shape's content is the same, whatever the
//!   zoom, and the cut line and the marks are only worked out the first time they are wanted,
//!   so hiding the seam allowance costs nothing.

use opendrape_core::{LineKind, Piece, PieceId, Point2, Project};
use opendrape_geom::{self as geom, Shape};
use std::cell::OnceCell;
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

/// The cut line and its fill triangles.
pub(super) struct Cut {
    /// Empty when no edge has any allowance.
    pub points: Vec<Point2>,
    pub fill: Vec<u32>,
}

/// The parts of a shape that don't depend on the zoom, kept while its content is unchanged.
struct Content {
    /// A label for every editable edge (not the pale half of a fold).
    labels: Vec<Label>,
    /// Worked out when the seam allowance is first shown.
    cut: OnceCell<Cut>,
    /// The notches' marks on the cut line, for when the allowance is shown.
    notches: OnceCell<Vec<[Point2; 2]>>,
    /// The notches' marks on the stitching line, for when it is hidden.
    notches_on_stitching: OnceCell<Vec<[Point2; 2]>>,
}

impl Content {
    fn new(shape: &Shape) -> Self {
        let piece = &shape.piece;
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
        Self {
            labels,
            cut: OnceCell::new(),
            notches: OnceCell::new(),
            notches_on_stitching: OnceCell::new(),
        }
    }
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
    pub lines: Vec<(Vec<Point2>, LineKind)>,
    content: Rc<Content>,
}

impl Drawn {
    /// A label for every editable edge (not the pale half of a fold).
    pub fn labels(&self) -> &[Label] {
        &self.content.labels
    }

    /// The cut line, if [`ShapeCache::shapes`] was asked to show the seam allowance.
    pub fn cut(&self) -> Option<&Cut> {
        self.content.cut.get()
    }

    /// The marks of every notch, in order: on the cut line when the seam allowance is shown,
    /// on the stitching line when it is hidden (as for no allowance). Empty if
    /// [`ShapeCache::shapes`] was not asked for that.
    pub fn notches(&self, allowance_shown: bool) -> &[[Point2; 2]] {
        let marks = if allowance_shown {
            &self.content.notches
        } else {
            &self.content.notches_on_stitching
        };
        marks.get().map_or(&[], Vec::as_slice)
    }
}

/// How many times each part has been worked out from scratch (tests count them).
#[cfg(test)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct Computed {
    /// The zoom-dependent part of a shape.
    pub detail: usize,
    /// The zoom-independent part (labels, and the holder of the rest).
    pub content: usize,
    pub cut: usize,
    /// A set of notch marks, either kind.
    pub notches: usize,
}

struct Entry {
    /// The shape's content fingerprint.
    key: u64,
    bracket: i32,
    drawn: Rc<Drawn>,
}

#[derive(Default)]
pub(super) struct ShapeCache {
    entries: HashMap<PieceId, Entry>,
    #[cfg(test)]
    pub computed: Computed,
}

impl ShapeCache {
    /// Every shape of `project`, ready to draw at `zoom` (screen points per mm). A shape whose
    /// content and zoom bracket haven't changed since the last call is reused as it was, and
    /// one whose content is the same at a new zoom keeps its cut line, marks and labels. The
    /// cut line and the cut-line marks are worked out only when `allowance` (the seam
    /// allowance is shown); the stitching-line marks only when it is not.
    pub fn shapes(&mut self, project: &Project, zoom: f64, allowance: bool) -> Vec<Rc<Drawn>> {
        let bracket = zoom.max(1e-9).log2().floor() as i32;
        // 0.25 to 0.5 screen points: smooth curves at every zoom.
        let tolerance = 0.25 / 2f64.powi(bracket);
        let mut kept = HashMap::new();
        let mut out = Vec::new();
        for shape in geom::shapes(project) {
            let key = fingerprint(&shape);
            let id = shape.id;
            let same = self.entries.remove(&id).filter(|e| e.key == key);
            let (content, reusable) = match same {
                Some(e) => (
                    e.drawn.content.clone(),
                    Some(e).filter(|e| e.bracket == bracket),
                ),
                None => {
                    #[cfg(test)]
                    {
                        self.computed.content += 1;
                    }
                    (Rc::new(Content::new(&shape)), None)
                }
            };
            let piece = &shape.piece;
            if allowance {
                content.cut.get_or_init(|| {
                    #[cfg(test)]
                    {
                        self.computed.cut += 1;
                    }
                    cut_of(piece)
                });
                content.notches.get_or_init(|| {
                    #[cfg(test)]
                    {
                        self.computed.notches += 1;
                    }
                    flat(geom::all_notch_marks(piece))
                });
            } else {
                content.notches_on_stitching.get_or_init(|| {
                    #[cfg(test)]
                    {
                        self.computed.notches += 1;
                    }
                    flat(geom::all_notch_marks_on_stitching(piece))
                });
            }
            let drawn = match reusable {
                Some(e) => e.drawn,
                None => {
                    #[cfg(test)]
                    {
                        self.computed.detail += 1;
                    }
                    Rc::new(prepare(shape, tolerance, content))
                }
            };
            kept.insert(
                id,
                Entry {
                    key,
                    bracket,
                    drawn: drawn.clone(),
                },
            );
            out.push(drawn);
        }
        self.entries = kept;
        out
    }
}

/// Identifies a shape's content: its kind and every field of its piece.
fn fingerprint(shape: &Shape) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    format!("{:?}", shape.kind).hash(&mut h);
    serde_json::to_vec(&shape.piece)
        .unwrap_or_default()
        .hash(&mut h);
    h.finish()
}

/// The zoom-dependent part of a shape, at flattening `tolerance` mm.
fn prepare(shape: Shape, tolerance: f64, content: Rc<Content>) -> Drawn {
    let piece = &shape.piece;
    let outline = thin(geom::outline_points(piece, tolerance));
    let fill = triangulate(&outline);
    let half = match shape.kind {
        geom::ShapeKind::Folded { drawn, .. } => thin_open(half_points(piece, drawn, tolerance)),
        _ => Vec::new(),
    };
    let half_fill = triangulate(&half);
    let lines = piece
        .lines
        .iter()
        .map(|l| (thin_open(geom::line_points(l, tolerance)), l.kind))
        .collect();
    Drawn {
        shape,
        outline,
        fill,
        half,
        half_fill,
        lines,
        content,
    }
}

/// The cut line of `piece` and its fill; empty when no edge has any allowance.
fn cut_of(piece: &Piece) -> Cut {
    let any_allowance = (0..piece.len()).any(|i| piece.edge_allowance(i) > 0.0);
    let points = if any_allowance {
        thin(geom::cut_line(piece))
    } else {
        Vec::new()
    };
    let fill = triangulate(&points);
    Cut { points, fill }
}

/// Every mark of every notch, in order.
fn flat(marks: Vec<Vec<[Point2; 2]>>) -> Vec<[Point2; 2]> {
    marks.into_iter().flatten().collect()
}

/// The first `drawn - 1` edges of an unfolded piece (the stored half's own edges) as one
/// polyline.
fn half_points(piece: &Piece, drawn: usize, tolerance: f64) -> Vec<Point2> {
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
        cache.shapes(&project, 1.0, true);
        cache.shapes(&project, 1.3, true); // same power-of-two bracket
        assert_eq!(cache.computed.detail, 1);
        project.pieces[0].translate(Point2::new(1.0, 0.0));
        cache.shapes(&project, 1.3, true);
        assert_eq!(cache.computed.detail, 2);
        cache.shapes(&project, 2.5, true); // a new bracket: finer outlines
        assert_eq!(cache.computed.detail, 3);
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
        let drawn = ShapeCache::default().shapes(&project, 1.0, true);
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
        let drawn = ShapeCache::default().shapes(&plain, 1.0, true);
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
        let drawn = ShapeCache::default().shapes(&project, 1.0, true);
        for d in &drawn {
            let piece = &d.shape.piece;
            for label in d.labels() {
                assert!(d.shape.stored_edge(label.edge).is_some());
                let (at, out) = geom::edge_label_anchor(piece, label.edge);
                assert_eq!((label.at, label.out), (at, out));
                assert_eq!(label.length, geom::edge_length(piece, label.edge));
            }
        }
        // A fold's pale half has no labels: 3 of the 6 outline edges are editable.
        let folded = drawn.iter().find(|d| d.shape.id == id).unwrap();
        assert_eq!(
            folded.labels().iter().map(|l| l.edge).collect::<Vec<_>>(),
            [0, 1, 2]
        );
        // A twin's outline runs the other way round: its labels still point out of it.
        let twin = drawn
            .iter()
            .find(|d| matches!(d.shape.kind, geom::ShapeKind::Twin { .. }))
            .unwrap();
        assert_eq!(twin.labels().len(), 4);
        assert_eq!(twin.labels()[0].out, Point2::new(0.0, -1.0)); // below the bottom edge
        assert_eq!(twin.labels()[0].length, 100.0);
    }

    #[test]
    fn crossing_a_zoom_bracket_redoes_the_zoom_dependent_part_only() {
        let mut cache = ShapeCache::default();
        let mut piece = Piece::rectangle(PieceId(0), "R", Point2::new(0.0, 0.0), 100.0, 100.0);
        piece.notches = vec![opendrape_core::Notch::new(0, 40.0)];
        let project = project_with(piece);
        let before = cache.shapes(&project, 1.0, true);
        let first_cut = before[0].cut().unwrap() as *const Cut;
        assert_eq!(
            cache.computed,
            Computed {
                detail: 1,
                content: 1,
                cut: 1,
                notches: 1
            }
        );
        // The same bracket, then a finer one and a coarser one: the outline is flattened again
        // for the new brackets, and the cut line, the marks and the labels are the very ones
        // from the first call.
        for zoom in [1.5, 4.0, 0.3] {
            let after = cache.shapes(&project, zoom, true);
            assert!(std::ptr::eq(first_cut, after[0].cut().unwrap()), "{zoom}");
            assert_eq!(after[0].notches(true), before[0].notches(true));
            assert_eq!(after[0].labels().len(), 4);
        }
        assert_eq!(
            cache.computed,
            Computed {
                detail: 3,
                content: 1,
                cut: 1,
                notches: 1
            }
        );
    }

    #[test]
    fn with_the_allowance_hidden_the_cut_line_is_never_computed() {
        let mut cache = ShapeCache::default();
        let mut project = project_with(Piece::rectangle(
            PieceId(0),
            "R",
            Point2::new(0.0, 0.0),
            100.0,
            100.0,
        ));
        project.pieces[0].notches = vec![opendrape_core::Notch::new(0, 40.0)];
        for zoom in [1.0, 4.0, 0.3] {
            let drawn = cache.shapes(&project, zoom, false);
            assert!(drawn[0].cut().is_none(), "{zoom}");
        }
        project.pieces[0].translate(Point2::new(5.0, 0.0));
        assert!(cache.shapes(&project, 1.0, false)[0].cut().is_none());
        assert_eq!(cache.computed.cut, 0);
        // Only the notches' stitching-line marks were wanted, once per content.
        assert_eq!(cache.computed.notches, 2);
        // Showing it works the cut line out once, and hiding and showing it again does not
        // work it out again.
        let shown = cache.shapes(&project, 1.0, true);
        assert!(!shown[0].cut().unwrap().points.is_empty());
        assert_eq!(cache.computed.cut, 1);
        for allowance in [false, true, false, true] {
            cache.shapes(&project, 1.0, allowance);
            cache.shapes(&project, 8.0, allowance);
        }
        assert_eq!(cache.computed.cut, 1);
        assert_eq!(cache.computed.notches, 3, "the cut-line marks, once");
        // A change to the shape does work it out again, when shown.
        project.pieces[0].translate(Point2::new(5.0, 0.0));
        cache.shapes(&project, 1.0, false);
        assert_eq!(cache.computed.cut, 1);
        let moved = cache.shapes(&project, 1.0, true);
        assert_eq!(cache.computed.cut, 2);
        // The piece now spans x = 10..110, so its cut line starts 10 mm further out.
        let left = moved[0].cut().unwrap().points.iter().map(|p| p.x);
        assert!((left.fold(f64::MAX, f64::min) - 0.0).abs() < 1e-9);
    }

    #[test]
    fn notches_are_drawn_on_the_cut_line_or_the_stitching_line() {
        let mut piece = Piece::rectangle(PieceId(0), "R", Point2::new(0.0, 0.0), 100.0, 100.0);
        piece.notches = vec![
            opendrape_core::Notch::new(0, 40.0),
            opendrape_core::Notch {
                marks: 2,
                ..opendrape_core::Notch::new(1, 40.0)
            },
        ];
        let project = project_with(piece);
        let mut cache = ShapeCache::default();
        let shown = cache.shapes(&project, 1.0, true);
        let hidden = cache.shapes(&project, 1.0, false);
        assert_eq!(shown[0].notches(true).len(), 3);
        assert_eq!(hidden[0].notches(false).len(), 3);
        // 10 mm of allowance: the first mark starts on the cut line, or on the stitching line.
        let near = |a: [Point2; 2], b: [Point2; 2]| {
            a[0].distance(b[0]) < 1e-6 && a[1].distance(b[1]) < 1e-6
        };
        assert!(near(
            shown[0].notches(true)[0],
            [Point2::new(40.0, -10.0), Point2::new(40.0, -5.0)]
        ));
        assert!(near(
            hidden[0].notches(false)[0],
            [Point2::new(40.0, 0.0), Point2::new(40.0, 5.0)]
        ));
        // A set is only there when it was asked for.
        let only_hidden = ShapeCache::default().shapes(&project, 1.0, false);
        assert!(only_hidden[0].notches(true).is_empty());
        let only_shown = ShapeCache::default().shapes(&project, 1.0, true);
        assert!(only_shown[0].notches(false).is_empty());
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
        // A within-limits but deliberately heavy piece: up to 2,000 wildly curved edges. 1,990
        // used to crash (a NaN point from the curve flattening reached the cut line), where
        // 2,000 happened not to.
        for count in [2000, 1990] {
            let corners: Vec<Point2> = (0..count)
                .map(|k| {
                    let a = k as f64 / count as f64 * std::f64::consts::TAU;
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
            let first = cache.shapes(&project, 50.0, true);
            assert!(
                first[0].outline.len() <= MAX_DRAWN_POINTS
                    && first[0].cut().unwrap().points.len() <= MAX_DRAWN_POINTS,
                "{count} points"
            );
            let started = std::time::Instant::now();
            let again = cache.shapes(&project, 50.0, true);
            assert!(Rc::ptr_eq(&first[0], &again[0]), "not recomputed");
            assert!(
                started.elapsed() < std::time::Duration::from_secs(1),
                "{:?}",
                started.elapsed()
            );
        }
    }
}
