//! Pattern pieces into fabric. Each shape on the pattern table (a piece, a whole cut-on-fold
//! piece, a twin) becomes one panel: its stitching outline sampled into points (both sides of a
//! seam with the same count, laid out so that notches paired across the seam land on the same
//! stitch) and filled with near-equilateral triangles. Seams become pairs of stitched points.
//! Pure: no GPU, no windows.

mod boundary;
mod holes;
pub mod place;
mod triangulate;

pub use triangulate::{ANGLE_LIMIT_DEG, TriangulateError, Triangulated, triangulate};

use opendrape_core::{PieceId, Point2, Project, Seam, SeamId, SeamSide};
use opendrape_geom::{self as geom, Shape};

/// The fabric's target edge length (mm).
pub const DEFAULT_EDGE_MM: f64 = 12.0;
/// The most particles a garment's fabric may have.
pub const MAX_PARTICLES: usize = 30_000;
/// Seam sides whose lengths differ by more than this (mm) get a note.
pub const LENGTH_WARNING_MM: f64 = 3.0;
/// The fabric is made again with coarser edges at most this many times (counting the first
/// try) when it does not fit the particle budget after all.
const MAX_PASSES: usize = 8;
/// Each try after a miss makes the edges at least this much longer.
const MIN_GROWTH: f64 = 1.25;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MeshParams {
    /// Target edge length (mm).
    pub edge_mm: f64,
    /// The fabric grows coarser until its estimated particle count is at most this.
    pub max_particles: usize,
}

impl Default for MeshParams {
    fn default() -> Self {
        Self {
            edge_mm: DEFAULT_EDGE_MM,
            max_particles: MAX_PARTICLES,
        }
    }
}

/// One panel of fabric.
#[derive(Clone, Debug, PartialEq)]
pub struct PanelMesh {
    /// The shape it was made from (a piece's id, or a twin's).
    pub shape: PieceId,
    /// Flat positions (m) on the pattern table: the fabric's rest shape.
    pub flat: Vec<[f64; 2]>,
    /// Anticlockwise on the pattern table.
    pub triangles: Vec<[u32; 3]>,
    /// For each outline edge of the shape: its points from its start corner to its end corner.
    pub edges: Vec<Vec<u32>>,
    /// The middle of the shape's bounding box on the pattern table (mm): the point placements
    /// put at their position.
    pub centre: Point2,
}

/// Something the student should know about the fabric.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum MeshNote {
    /// The pattern was too large for the fabric's particle budget: edges are `edge_mm` long.
    Coarser { edge_mm: f64 },
    /// This shape's outline crosses itself, so it was left out with its seams.
    CrossesItself(PieceId),
    /// This shape could not be made into fabric for another reason; left out with its seams.
    Unmeshable(PieceId),
    /// A cut-out of this shape touches its outline or another cut-out, or lies outside the
    /// shape: it was left out (the rest of the shape is made).
    CutoutLeftOut(PieceId),
    /// The two sides of this seam differ in length by `by_mm`: the longer gathers as ease.
    LengthsDiffer { seam: SeamId, by_mm: f64 },
}

/// Two points sewn together: (panel, point) each.
pub type Stitch = ((usize, u32), (usize, u32));

/// A garment's fabric: its panels, the stitches that sew them, and notes.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct GarmentMesh {
    pub panels: Vec<PanelMesh>,
    /// Every pair of points sewn together, in seam order.
    pub stitches: Vec<Stitch>,
    /// For each stitch, the seam it sews and whether that is the seam's mirror image.
    pub stitch_seams: Vec<(SeamId, bool)>,
    pub notes: Vec<MeshNote>,
    /// The edge length (mm) the fabric was made with.
    pub edge_mm: f64,
}

impl GarmentMesh {
    /// Points in every panel together.
    pub fn particles(&self) -> usize {
        self.panels.iter().map(|p| p.flat.len()).sum()
    }
    /// The panel made from `shape`, if it could be made.
    pub fn panel_of(&self, shape: PieceId) -> Option<usize> {
        self.panels.iter().position(|p| p.shape == shape)
    }
}

/// One side of a seam as the layout sees it: its length, its notches and the corners inside it,
/// as distances (mm) from its start.
struct SidePlan {
    length: f64,
    notches: Vec<f64>,
    corners: Vec<f64>,
}

impl SidePlan {
    fn new(shape: &Shape, side: &SeamSide) -> Option<Self> {
        let runs = geom::side_runs(shape, side)?;
        let mut corners = Vec::new();
        let mut at = 0.0;
        for run in &runs[..runs.len() - 1] {
            at += run.length();
            corners.push(at);
        }
        Some(Self {
            length: at + runs[runs.len() - 1].length(),
            notches: geom::side_notches(shape, side)?,
            corners,
        })
    }
}

/// A seam to stitch, mirror images included: the shapes of its sides (indices into the shapes)
/// and what the layout needs of each side.
struct SeamPlan {
    seam: Seam,
    shape_a: usize,
    shape_b: usize,
    a: SidePlan,
    b: SidePlan,
    mirrored: bool,
}

impl SeamPlan {
    /// Where each side's samples go at edge length `h` (mm from the side's start, the same
    /// number on both sides). When the sides have as many notches as each other, the k-th
    /// notch of one and the k-th of the other start a stretch each, and every pair of stretches
    /// gets its own step count (at least one), so paired notches land on the same sample;
    /// otherwise the whole sides are one stretch each, as before notches were matched. Each
    /// corner inside a side then takes over the sample of its stretch nearest to it.
    fn layout(&self, h: f64) -> (Vec<f64>, Vec<f64>) {
        let breaks = |side: &SidePlan, matched: bool| {
            let mut b = vec![0.0];
            if matched {
                b.extend(&side.notches);
            }
            b.push(side.length);
            b
        };
        let matched = self.a.notches.len() == self.b.notches.len();
        let (ba, bb) = (breaks(&self.a, matched), breaks(&self.b, matched));
        let steps: Vec<usize> = ba
            .windows(2)
            .zip(bb.windows(2))
            .map(|(a, b)| (((a[1] - a[0]).max(b[1] - b[0]) / h).round() as usize).max(1))
            .collect();
        (
            samples(&ba, &steps, &self.a.corners),
            samples(&bb, &steps, &self.b.corners),
        )
    }
}

/// Samples along a side cut into stretches at `breaks` (its start, notches, its end), with
/// `steps[j]` equal steps on stretch j. Each corner (a distance in `corners`) takes over the
/// sample of its stretch nearest to it, never a stretch's own ends; a corner whose sample
/// another corner took keeps none. A corner at a break (a notch on a corner inside the side)
/// already has the break's sample, so it takes none: a second sample there would be the same
/// point of fabric twice, and stitching it to two points of the other side would weld those
/// two together.
fn samples(breaks: &[f64], steps: &[usize], corners: &[f64]) -> Vec<f64> {
    let mut at = Vec::new();
    let mut first = Vec::with_capacity(steps.len());
    for (j, &n) in steps.iter().enumerate() {
        first.push(at.len());
        let (lo, hi) = (breaks[j], breaks[j + 1]);
        at.extend((0..n).map(|k| lo + (hi - lo) * k as f64 / n as f64));
    }
    at.push(breaks[breaks.len() - 1]);
    let mut taken = vec![false; at.len()];
    for &c in corners {
        if breaks.iter().any(|b| (b - c).abs() <= 1e-9 * c.max(1.0)) {
            continue;
        }
        let Some(j) = (0..steps.len()).find(|&j| c >= breaks[j] && c <= breaks[j + 1]) else {
            continue;
        };
        let (n, lo, hi) = (steps[j], breaks[j], breaks[j + 1]);
        if n < 2 || hi - lo <= 0.0 {
            continue;
        }
        let k = first[j] + (((c - lo) / (hi - lo) * n as f64).round() as usize).clamp(1, n - 1);
        if !taken[k] {
            taken[k] = true;
            at[k] = c;
        }
    }
    at
}

/// What making the fabric at one edge length gave.
struct Made {
    panels: Vec<PanelMesh>,
    stitches: Vec<Stitch>,
    stitch_seams: Vec<(SeamId, bool)>,
    /// Whether each shape became a panel.
    meshed: Vec<bool>,
    /// What went wrong with shapes and their cut-outs, in shape order.
    notes: Vec<MeshNote>,
    /// Every panel's refinement ran to the end.
    complete: bool,
}

/// The fabric for every shape of `project`, and the stitches for every seam (mirror images
/// included). A shape that can't be meshed is left out with its seams, and a note says why.
/// The whole garment stays within `params.max_particles`: edges grow coarser until the
/// estimate fits, and if the real fabric still doesn't (or refinement ran out of points), it is
/// made again with coarser edges, a few times at most.
pub fn build(project: &Project, params: &MeshParams) -> GarmentMesh {
    let shapes = geom::shapes(project);
    let seams = seam_plans(project, &shapes);
    let start = edge_length_within_budget(&shapes, params);
    let (made, h) = fit(start, params.max_particles, |h| {
        let made = mesh_shapes(&shapes, &seams, h, params.max_particles);
        Tried {
            particles: made.panels.iter().map(|p| p.flat.len()).sum(),
            complete: made.complete,
            value: made,
        }
    });
    let mut notes = Vec::new();
    if h > params.edge_mm {
        notes.push(MeshNote::Coarser { edge_mm: h });
    }
    // A seam whose shape was left out is not built, so it has no length to warn about.
    for plan in &seams {
        let differ = (plan.a.length - plan.b.length).abs();
        if !plan.mirrored
            && differ > LENGTH_WARNING_MM
            && made.meshed[plan.shape_a]
            && made.meshed[plan.shape_b]
        {
            notes.push(MeshNote::LengthsDiffer {
                seam: plan.seam.id,
                by_mm: differ,
            });
        }
    }
    notes.extend(made.notes);
    GarmentMesh {
        panels: made.panels,
        stitches: made.stitches,
        stitch_seams: made.stitch_seams,
        notes,
        edge_mm: h,
    }
}

/// Every seam with both its shapes and side lengths known, mirror images included.
fn seam_plans(project: &Project, shapes: &[Shape]) -> Vec<SeamPlan> {
    let shape_of = |id| shapes.iter().position(|s: &Shape| s.id == id);
    project
        .all_seams()
        .into_iter()
        .filter_map(|(seam, mirrored)| {
            let (shape_a, shape_b) = (shape_of(seam.a.shape)?, shape_of(seam.b.shape)?);
            Some(SeamPlan {
                seam,
                shape_a,
                shape_b,
                a: SidePlan::new(&shapes[shape_a], &seam.a)?,
                b: SidePlan::new(&shapes[shape_b], &seam.b)?,
                mirrored,
            })
        })
        .collect()
}

/// The panels for `shapes` at edge length `h`, stitched along `seams`. Each panel may use
/// what is left of the `max_particles` budget.
fn mesh_shapes(shapes: &[Shape], seams: &[SeamPlan], h: f64, max_particles: usize) -> Made {
    let mut panels = Vec::new();
    let mut notes = Vec::new();
    let mut complete = true;
    // For each shape: its panel's index, and where each of its sides' samples landed.
    let mut made: Vec<Option<(usize, Vec<Vec<u32>>)>> = Vec::with_capacity(shapes.len());
    let mut used = 0;
    let layouts: Vec<(Vec<f64>, Vec<f64>)> = seams.iter().map(|plan| plan.layout(h)).collect();
    for (index, shape) in shapes.iter().enumerate() {
        let sides: Vec<(SeamSide, Vec<f64>)> = seams
            .iter()
            .zip(&layouts)
            .flat_map(|(plan, (at_a, at_b))| {
                [
                    (plan.seam.a, plan.shape_a, at_a),
                    (plan.seam.b, plan.shape_b, at_b),
                ]
                .into_iter()
                .filter(|(_, s, _)| *s == index)
                .map(|(side, _, at)| (side, at.clone()))
            })
            .collect();
        match panel(shape, &sides, h, max_particles.saturating_sub(used)) {
            Ok(done) => {
                used += done.mesh.flat.len();
                complete &= done.complete;
                if done.cutouts_left_out > 0 {
                    notes.push(MeshNote::CutoutLeftOut(shape.id));
                }
                made.push(Some((panels.len(), done.sides)));
                panels.push(done.mesh);
            }
            Err(TriangulateError::CrossesItself) => {
                notes.push(MeshNote::CrossesItself(shape.id));
                made.push(None);
            }
            Err(_) => {
                notes.push(MeshNote::Unmeshable(shape.id));
                made.push(None);
            }
        }
    }
    // Stitch side samples in step: the k-th of side a to the k-th of side b. The order of
    // sides within each shape matches the order they were handed to `panel` above.
    let mut next_side = vec![0usize; shapes.len()];
    let mut stitches = Vec::new();
    let mut stitch_seams = Vec::new();
    for (plan, (at_a, _)) in seams.iter().zip(&layouts) {
        let (sa, sb) = (plan.shape_a, plan.shape_b);
        let side_a = next_side[sa];
        next_side[sa] += 1;
        let side_b = next_side[sb];
        next_side[sb] += 1;
        if let (Some((pa, a)), Some((pb, b))) = (&made[sa], &made[sb]) {
            for k in 0..at_a.len() {
                stitches.push(((*pa, a[side_a][k]), (*pb, b[side_b][k])));
                stitch_seams.push((plan.seam.id, plan.mirrored));
            }
        }
    }
    Made {
        panels,
        stitches,
        stitch_seams,
        meshed: made.iter().map(Option::is_some).collect(),
        notes,
        complete,
    }
}

/// One try at making the fabric: what it is, how many particles it has, and whether it was
/// made properly.
struct Tried<T> {
    value: T,
    particles: usize,
    complete: bool,
}

/// `make(h)` at edge length `h`, and again at longer edges while the result has more than
/// `max_particles` particles or was not made properly, at most [`MAX_PASSES`] tries in all (the
/// last try stands whatever it is). Returns the result and the edge length it was made with.
fn fit<T>(mut h: f64, max_particles: usize, mut make: impl FnMut(f64) -> Tried<T>) -> (T, f64) {
    let mut pass = 1;
    loop {
        let tried = make(h);
        if pass >= MAX_PASSES || (tried.complete && tried.particles <= max_particles) {
            return (tried.value, h);
        }
        let over = (tried.particles as f64 / max_particles.max(1) as f64).sqrt() * 1.05;
        h *= over.max(MIN_GROWTH);
        pass += 1;
    }
}

/// `params.edge_mm`, or a longer edge if the fabric would otherwise have more than
/// `params.max_particles` particles. Estimated from the shapes' areas and perimeters (about
/// 1.6 points per h² of area, measured on real panels, plus the outline's own points).
fn edge_length_within_budget(shapes: &[Shape], params: &MeshParams) -> f64 {
    let area: f64 = shapes.iter().map(|s| geom::area(&s.piece)).sum();
    let perimeter: f64 = shapes.iter().map(|s| geom::perimeter(&s.piece)).sum();
    let estimate = |h: f64| 1.7 * area / (h * h) + perimeter / h;
    let budget = params.max_particles as f64;
    let mut h = params.edge_mm;
    while estimate(h) > budget && h.is_finite() {
        h *= (estimate(h) / budget).sqrt() * 1.05;
    }
    h
}

/// One shape's panel, where each of its `sides` samples landed among its points, and how it
/// went.
struct Done {
    mesh: PanelMesh,
    sides: Vec<Vec<u32>>,
    /// Refinement ran to the end.
    complete: bool,
    /// How many closed cut-outs were left out (see [`holes`]).
    cutouts_left_out: usize,
}

/// `shape`'s panel. Refinement may add up to `budget` points less than the outline and holes
/// already have.
fn panel(
    shape: &Shape,
    sides: &[(SeamSide, Vec<f64>)],
    h: f64,
    budget: usize,
) -> Result<Done, TriangulateError> {
    let outline = boundary::outline(shape, sides, h).ok_or(TriangulateError::CrossesItself)?;
    let corners: Vec<[f64; 2]> = outline.points.iter().map(|p| [p.x, p.y]).collect();
    let holes = holes::holes(shape, &corners, h);
    let boundary_points = corners.len() + holes.loops.iter().map(Vec::len).sum::<usize>();
    let mesh = triangulate(
        &corners,
        &holes.loops,
        h,
        budget.saturating_sub(boundary_points),
    )?;
    Ok(Done {
        mesh: PanelMesh {
            shape: shape.id,
            flat: mesh
                .points
                .iter()
                .map(|p| [p[0] / 1000.0, p[1] / 1000.0])
                .collect(),
            triangles: mesh.triangles,
            edges: outline.edges,
            centre: place::centre_of(shape),
        },
        sides: outline.sides,
        complete: mesh.refinement_complete,
        cutouts_left_out: holes.left_out,
    })
}

/// The smallest box holding `points`: (min, max).
pub fn bounds(points: &[Point2]) -> (Point2, Point2) {
    let first = points.first().copied().unwrap_or_default();
    points.iter().fold((first, first), |(lo, hi), p| {
        (
            Point2::new(lo.x.min(p.x), lo.y.min(p.y)),
            Point2::new(hi.x.max(p.x), hi.y.max(p.y)),
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn even(from: f64, to: f64, steps: usize) -> Vec<f64> {
        (0..=steps)
            .map(|k| from + (to - from) * k as f64 / steps as f64)
            .collect()
    }

    #[test]
    fn a_corner_takes_the_nearest_sample_of_its_stretch_and_nothing_else() {
        let rest = |mut samples: Vec<f64>, k: usize, value: f64| {
            samples[k] = value;
            samples
        };
        // 10 steps of 10 mm, a corner 37 mm along: the sample at 40 moves to it.
        assert_eq!(
            samples(&[0.0, 100.0], &[10], &[37.0]),
            rest(even(0.0, 100.0, 10), 4, 37.0)
        );
        // Never a stretch's own ends: a corner near the start or the end takes the first or
        // the last sample inside it.
        assert_eq!(
            samples(&[0.0, 100.0], &[10], &[1.0]),
            rest(even(0.0, 100.0, 10), 1, 1.0)
        );
        assert_eq!(
            samples(&[0.0, 100.0], &[10], &[99.0]),
            rest(even(0.0, 100.0, 10), 9, 99.0)
        );
        // A stretch of one step has no sample inside it for a corner to take.
        assert_eq!(
            samples(&[0.0, 10.0, 100.0], &[1, 9], &[4.0]),
            [vec![0.0], even(10.0, 100.0, 9)].concat()
        );
        // Two corners nearest the same sample: the first has it, the second none.
        assert_eq!(
            samples(&[0.0, 100.0], &[10], &[36.0, 38.0]),
            rest(even(0.0, 100.0, 10), 4, 36.0)
        );
        // A corner in a later stretch is found there: 75 mm is 1.5 steps into the second one.
        assert_eq!(
            samples(&[0.0, 60.0, 100.0], &[6, 4], &[75.0]),
            vec![
                0.0, 10.0, 20.0, 30.0, 40.0, 50.0, 60.0, 70.0, 75.0, 90.0, 100.0
            ]
        );
    }

    #[test]
    fn a_corner_on_a_notch_has_the_notch_sample_and_takes_no_other() {
        // A break at 60 mm is a notch; the corner is the same place, however it was measured.
        for c in [60.0, 60.0 + 1e-12, 60.0 - 1e-12] {
            let got = samples(&[0.0, 60.0, 100.0], &[6, 4], &[c]);
            assert_eq!(got.len(), 11);
            assert_eq!(got[6], 60.0, "the notch's own sample");
            for w in got.windows(2) {
                assert!(w[1] - w[0] > 9.0, "{c}: a sample twice: {got:?}");
            }
        }
        // The ends of a side are breaks too.
        let got = samples(&[0.0, 100.0], &[10], &[1e-12, 100.0 - 1e-12]);
        assert_eq!(got, even(0.0, 100.0, 10));
    }

    /// A stand-in for making the fabric: 2,000,000 / h² particles.
    fn tried(h: f64) -> Tried<f64> {
        Tried {
            value: h,
            particles: (2_000_000.0 / (h * h)) as usize,
            complete: true,
        }
    }

    #[test]
    fn a_panel_uses_no_more_points_than_the_budget_leaves_and_says_so() {
        let mut pr = Project::new();
        pr.add_piece(opendrape_core::Piece::rectangle(
            PieceId(0),
            "Square",
            Point2::new(0.0, 0.0),
            300.0,
            300.0,
        ));
        let shapes = geom::shapes(&pr);
        let plenty = panel(&shapes[0], &[], 12.0, 100_000).unwrap();
        assert!(plenty.complete && plenty.mesh.flat.len() > 600);
        // The square's outline alone has 100 points: 400 leaves 300 for refinement, which
        // needs more than that.
        let tight = panel(&shapes[0], &[], 12.0, 400).unwrap();
        assert!(!tight.complete, "refinement ran out of points");
        assert!(tight.mesh.flat.len() <= 400, "{}", tight.mesh.flat.len());
        // A budget below the outline's own points leaves refinement none.
        let none = panel(&shapes[0], &[], 12.0, 10).unwrap();
        assert!(!none.complete && none.mesh.flat.len() == 100);
    }

    #[test]
    fn a_try_that_fits_stands() {
        let mut calls = 0;
        let (value, h) = fit(12.0, 20_000, |h| {
            calls += 1;
            tried(h)
        });
        assert_eq!((calls, value, h), (1, 12.0, 12.0));
    }

    #[test]
    fn a_try_that_does_not_fit_is_made_again_with_coarser_edges() {
        let mut edges = Vec::new();
        let (value, h) = fit(12.0, 5_000, |h| {
            edges.push(h);
            tried(h)
        });
        assert!(edges.len() > 1 && edges.len() < MAX_PASSES, "{edges:?}");
        for pair in edges.windows(2) {
            assert!(pair[1] >= pair[0] * MIN_GROWTH, "{edges:?}");
        }
        assert_eq!(value, h);
        assert!(
            tried(h).particles <= 5_000,
            "{h} mm → {}",
            tried(h).particles
        );
    }

    #[test]
    fn a_try_whose_refinement_ran_out_of_points_is_made_again_too() {
        let mut edges = Vec::new();
        let (_, h) = fit(12.0, 1_000_000, |h| {
            edges.push(h);
            Tried {
                complete: h > 20.0,
                ..tried(h)
            }
        });
        assert!(h > 20.0 && edges.len() > 1, "{edges:?}");
    }

    #[test]
    fn making_the_fabric_again_stops_after_a_few_tries() {
        // Always 100 particles, whatever the edge length: no budget below that is ever met.
        let mut calls = 0;
        let (_, h) = fit(12.0, 50, |h| {
            calls += 1;
            Tried {
                value: h,
                particles: 100,
                complete: true,
            }
        });
        assert_eq!(calls, MAX_PASSES);
        assert!(
            h > 12.0 * MIN_GROWTH.powi(MAX_PASSES as i32 - 1) * 0.99,
            "{h}"
        );
    }
}
