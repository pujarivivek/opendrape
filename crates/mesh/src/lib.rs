//! Pattern pieces into fabric. Each shape on the pattern table (a piece, a whole cut-on-fold
//! piece, a twin) becomes one panel: its stitching outline sampled into points (both sides of a
//! seam with the same count) and filled with near-equilateral triangles. Seams become pairs of
//! stitched points. Pure: no GPU, no windows.

mod boundary;
pub mod place;
mod triangulate;

pub use triangulate::{ANGLE_LIMIT_DEG, TriangulateError, Triangulated, triangulate};

use opendrape_core::{LineKind, PieceId, Point2, Project, SeamId};
use opendrape_geom::{self as geom, Shape};

/// The fabric's target edge length (mm).
pub const DEFAULT_EDGE_MM: f64 = 12.0;
/// The most particles a garment's fabric may have.
pub const MAX_PARTICLES: usize = 30_000;
/// Seam sides whose lengths differ by more than this (mm) get a note.
pub const LENGTH_WARNING_MM: f64 = 3.0;

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

/// The fabric for every shape of `project`, and the stitches for every seam (mirror images
/// included). A shape that can't be meshed is left out with its seams, and a note says why.
pub fn build(project: &Project, params: &MeshParams) -> GarmentMesh {
    let shapes = geom::shapes(project);
    let mut notes = Vec::new();
    let h = edge_length_within_budget(&shapes, params);
    if h > params.edge_mm {
        notes.push(MeshNote::Coarser { edge_mm: h });
    }
    // Every seam, mirror images too: its sides' shapes and its step count.
    let mut seams = Vec::new();
    for (seam, mirrored) in project.all_seams() {
        let shape_of = |id| shapes.iter().position(|s: &Shape| s.id == id);
        let (Some(sa), Some(sb)) = (shape_of(seam.a.shape), shape_of(seam.b.shape)) else {
            continue;
        };
        let (Some(la), Some(lb)) = (
            geom::side_length(&shapes[sa], &seam.a),
            geom::side_length(&shapes[sb], &seam.b),
        ) else {
            continue;
        };
        if !mirrored && (la - lb).abs() > LENGTH_WARNING_MM {
            notes.push(MeshNote::LengthsDiffer {
                seam: seam.id,
                by_mm: (la - lb).abs(),
            });
        }
        let steps = ((la.max(lb) / h).round() as usize).max(1);
        seams.push((seam, sa, sb, steps));
    }
    let mut panels = Vec::new();
    // For each shape: its panel's index, and where each of its sides' samples landed.
    let mut made: Vec<Option<(usize, Vec<Vec<u32>>)>> = Vec::with_capacity(shapes.len());
    for (index, shape) in shapes.iter().enumerate() {
        let sides: Vec<_> = seams
            .iter()
            .flat_map(|(seam, sa, sb, steps)| {
                [(seam.a, *sa), (seam.b, *sb)]
                    .into_iter()
                    .filter(|(_, s)| *s == index)
                    .map(|(side, _)| (side, *steps))
            })
            .collect();
        match panel(shape, &sides, h, params.max_particles) {
            Ok((mesh, side_points)) => {
                made.push(Some((panels.len(), side_points)));
                panels.push(mesh);
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
    for (_, sa, sb, steps) in &seams {
        let side_a = next_side[*sa];
        next_side[*sa] += 1;
        let side_b = next_side[*sb];
        next_side[*sb] += 1;
        if let (Some((pa, a)), Some((pb, b))) = (&made[*sa], &made[*sb]) {
            for k in 0..=*steps {
                stitches.push(((*pa, a[side_a][k]), (*pb, b[side_b][k])));
            }
        }
    }
    GarmentMesh {
        panels,
        stitches,
        notes,
        edge_mm: h,
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

/// One shape's panel, and where each of its `sides` samples landed among its points.
fn panel(
    shape: &Shape,
    sides: &[(opendrape_core::SeamSide, usize)],
    h: f64,
    max_points: usize,
) -> Result<(PanelMesh, Vec<Vec<u32>>), TriangulateError> {
    let outline = boundary::outline(shape, sides, h).ok_or(TriangulateError::CrossesItself)?;
    let corners: Vec<[f64; 2]> = outline.points.iter().map(|p| [p.x, p.y]).collect();
    let holes: Vec<Vec<[f64; 2]>> = shape
        .piece
        .lines
        .iter()
        .filter(|l| l.closed && l.kind == LineKind::Cutout)
        .map(|l| resampled_loop(&geom::line_points(l, 0.1), h))
        .collect();
    let mesh = triangulate(&corners, &holes, h, max_points)?;
    Ok((
        PanelMesh {
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
        outline.sides,
    ))
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

/// A closed polyline (its last point repeats its first) as points about `h` apart along it.
fn resampled_loop(points: &[Point2], h: f64) -> Vec<[f64; 2]> {
    let mut pts = points.to_vec();
    if pts.len() > 1 && pts.first() == pts.last() {
        pts.pop();
    }
    let n = pts.len();
    if n < 3 {
        return Vec::new();
    }
    let seg = |k: usize| pts[k].distance(pts[(k + 1) % n]);
    let total: f64 = (0..n).map(seg).sum();
    let count = ((total / h).round() as usize).max(3);
    let mut out = Vec::with_capacity(count);
    let (mut k, mut start) = (0, 0.0);
    for q in 0..count {
        let s = total * q as f64 / count as f64;
        while k + 1 < n && s > start + seg(k) {
            start += seg(k);
            k += 1;
        }
        let t = if seg(k) > 0.0 {
            (s - start) / seg(k)
        } else {
            0.0
        };
        let p = pts[k].lerp(pts[(k + 1) % n], t.clamp(0.0, 1.0));
        out.push([p.x, p.y]);
    }
    out
}
