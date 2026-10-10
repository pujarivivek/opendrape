//! One panel's fabric: a constrained Delaunay triangulation of its outline (and holes), filled
//! with a lattice of points on the fabric's grain and refined where the lattice meets the
//! outline until no triangle is too big or too thin. Every boundary point is kept, in order,
//! so the points sampled along a seam are exactly the points that get stitched. The lattice
//! points follow, row by row along the grain, so the fabric's warp and weft run along the
//! triangles' edges and the bias along their diagonals.

use spade::{
    AngleLimit, ConstrainedDelaunayTriangulation, Point2 as SPoint, RefinementParameters,
    Triangulation,
};

/// Refinement adds points until no triangle has an angle below this (degrees), except next to
/// an outline corner sharper than that, which no triangle there can beat.
pub const ANGLE_LIMIT_DEG: f64 = 25.0;
/// The largest triangle refinement leaves is this many squared edge lengths: 0.5 h² keeps edges
/// between 0.6 and 1.5 h (an equilateral triangle of side h is 0.433 h²).
const MAX_AREA_PER_H2: f64 = 0.5;
/// With a lattice inside, each cell is two right triangles of exactly 0.5 h²: refinement must
/// leave those alone and only tidy the band where the lattice meets the outline.
const MAX_AREA_PER_H2_WITH_GRID: f64 = 0.56;
/// Triangles smaller than this many squared edge lengths are never split further.
const MIN_AREA_PER_H2: f64 = 0.02;
/// Lattice points closer than this many edge lengths to the outline are left out: the band
/// between the outline's points and the first lattice row is then between half and one and a
/// half edge lengths wide, which makes well-shaped triangles.
const CLEARANCE_PER_H: f64 = 0.6;

/// A lattice of points to fill a panel with, on the fabric's grain: rows along the grain and
/// columns across it, `spacing` apart, laid out from `origin`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Grid {
    /// A point of the lattice (in the units of the outline).
    pub origin: [f64; 2],
    /// The grain: degrees anticlockwise from +x.
    pub angle_deg: f64,
    pub spacing: f64,
}

/// A triangulated panel, in the units of its input.
#[derive(Clone, Debug, PartialEq)]
pub struct Triangulated {
    /// The boundary points first, exactly as given (outline, then each hole), then the lattice
    /// points row by row along the grain, then the points refinement added.
    pub points: Vec<[f64; 2]>,
    /// Anticlockwise triangles (indices into `points`) covering the inside of the outline and
    /// none of its holes.
    pub triangles: Vec<[u32; 3]>,
    /// False when refinement ran out of the points it was allowed to add, so some triangles
    /// may still be too big or too thin.
    pub refinement_complete: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TriangulateError {
    /// The outline has fewer than 3 points.
    TooFewPoints,
    /// A point is not a finite number.
    NotFinite,
    /// Two boundary points coincide, or the outline (or a hole) crosses or touches itself or
    /// another loop.
    CrossesItself,
}

/// Triangulates the area inside `outline` and outside every hole (closed loops; the first
/// point is not repeated; either winding), aiming at edges `h` long. Refinement adds at most
/// `max_added` points (the result says if that was too few).
pub fn triangulate(
    outline: &[[f64; 2]],
    holes: &[Vec<[f64; 2]>],
    h: f64,
    max_added: usize,
) -> Result<Triangulated, TriangulateError> {
    triangulate_with(outline, holes, h, None, max_added)
}

/// As [`triangulate`], filled with the lattice `grid` first when there is one (its spacing
/// should be `h`), so that only the band along the outline is left to refinement.
pub fn triangulate_with(
    outline: &[[f64; 2]],
    holes: &[Vec<[f64; 2]>],
    h: f64,
    grid: Option<Grid>,
    max_added: usize,
) -> Result<Triangulated, TriangulateError> {
    if outline.len() < 3 {
        return Err(TriangulateError::TooFewPoints);
    }
    let loops: Vec<&[[f64; 2]]> = std::iter::once(outline)
        .chain(holes.iter().map(Vec::as_slice).filter(|l| l.len() >= 3))
        .collect();
    let mut cdt: ConstrainedDelaunayTriangulation<SPoint<f64>> =
        ConstrainedDelaunayTriangulation::new();
    let mut handles = Vec::with_capacity(loops.len());
    for points in &loops {
        let mut loop_handles = Vec::with_capacity(points.len());
        for p in points.iter() {
            if !(p[0].is_finite() && p[1].is_finite()) {
                return Err(TriangulateError::NotFinite);
            }
            let before = cdt.num_vertices();
            let v = cdt
                .insert(SPoint::new(tidy(p[0]), tidy(p[1])))
                .map_err(|_| TriangulateError::NotFinite)?;
            if cdt.num_vertices() == before {
                // spade merged it with an earlier point at the same place.
                return Err(TriangulateError::CrossesItself);
            }
            loop_handles.push(v);
        }
        handles.push(loop_handles);
    }
    for loop_handles in &handles {
        for k in 0..loop_handles.len() {
            let (a, b) = (loop_handles[k], loop_handles[(k + 1) % loop_handles.len()]);
            // Exactly one new constraint edge: anything else means it crossed another
            // boundary edge (none added) or ran through another boundary point (split).
            if cdt.try_add_constraint(a, b).len() != 1 {
                return Err(TriangulateError::CrossesItself);
            }
        }
    }
    let boundary = cdt.num_vertices();
    // The lattice counts against `max_added` too; one cut short leaves the fill to refinement,
    // which then has nothing left either, and the result says so.
    let mut lattice_cut_short = false;
    if let Some(g) = grid {
        // One more than fits tells whether the lattice was cut short.
        let points = lattice(&loops, &g, CLEARANCE_PER_H * h, max_added.saturating_add(1));
        for p in points {
            if cdt.num_vertices() - boundary >= max_added {
                lattice_cut_short = true;
                break;
            }
            // A lattice point on an existing point is merged away by spade: nothing lost.
            cdt.insert(SPoint::new(tidy(p[0]), tidy(p[1])))
                .map_err(|_| TriangulateError::NotFinite)?;
        }
    }
    let max_area = if grid.is_some() {
        MAX_AREA_PER_H2_WITH_GRID
    } else {
        MAX_AREA_PER_H2
    };
    let params = RefinementParameters::<f64>::new()
        .with_angle_limit(AngleLimit::from_deg(ANGLE_LIMIT_DEG))
        .with_max_allowed_area(max_area * h * h)
        .with_min_required_area(MIN_AREA_PER_H2 * h * h)
        .keep_constraint_edges()
        .exclude_outer_faces(true)
        .with_max_additional_vertices(max_added - (cdt.num_vertices() - boundary));
    let result = cdt.refine(params);
    let refinement_complete = result.refinement_complete && !lattice_cut_short;
    let outside: std::collections::HashSet<_> = result.excluded_faces.into_iter().collect();
    let mut used = vec![false; cdt.num_vertices()];
    let mut faces = Vec::new();
    // spade lists its faces in a fixed order, so the result is the same on every run.
    for face in cdt.inner_faces() {
        if outside.contains(&face.fix()) {
            continue;
        }
        let t = face.vertices().map(|v| v.fix().index());
        for &k in &t {
            used[k] = true;
        }
        faces.push(t);
    }
    // Boundary points stay, in order; points refinement added outside the outline are dropped.
    let mut new_index = vec![u32::MAX; used.len()];
    let mut points = Vec::with_capacity(used.len());
    for (k, v) in cdt.vertices().enumerate() {
        if k < boundary || used[k] {
            new_index[k] = points.len() as u32;
            let p = v.position();
            points.push([p.x, p.y]);
        }
    }
    let triangles = faces.iter().map(|t| t.map(|k| new_index[k])).collect();
    Ok(Triangulated {
        points,
        triangles,
        refinement_complete,
    })
}

/// spade refuses coordinates below 2^-142 in size: such a number is zero here.
fn tidy(v: f64) -> f64 {
    if v.abs() < 1e-9 { 0.0 } else { v }
}

/// The lattice's points inside the first of `loops` (the outline) and outside the others (the
/// holes), at least `clearance` from every loop, row by row along the grain: at most `most`
/// of them, and none at all for an outline whose box would take more than [`MOST_CELLS`]
/// lattice cells to walk (a crafted piece kilometres across, which refinement's own budget
/// then handles).
fn lattice(loops: &[&[[f64; 2]]], g: &Grid, clearance: f64, most: usize) -> Vec<[f64; 2]> {
    let (s, c) = g.angle_deg.to_radians().sin_cos();
    let (along, across) = ([c, s], [-s, c]);
    let outline = loops[0];
    // The outline's extent along and across the grain, in lattice steps from the origin.
    let (mut lo, mut hi) = ([f64::MAX; 2], [f64::MIN; 2]);
    for p in outline {
        let d = [p[0] - g.origin[0], p[1] - g.origin[1]];
        for (k, axis) in [along, across].iter().enumerate() {
            let t = (d[0] * axis[0] + d[1] * axis[1]) / g.spacing;
            lo[k] = lo[k].min(t);
            hi[k] = hi[k].max(t);
        }
    }
    let mut out = Vec::new();
    let (rows, cols) = (
        hi[1].ceil() - lo[1].floor() + 1.0,
        hi[0].ceil() - lo[0].floor() + 1.0,
    );
    let cells = rows * cols;
    if !cells.is_finite() || cells > MOST_CELLS || most == 0 {
        return out;
    }
    'rows: for j in (lo[1].floor() as i64)..=(hi[1].ceil() as i64) {
        for i in (lo[0].floor() as i64)..=(hi[0].ceil() as i64) {
            let (u, v) = (i as f64 * g.spacing, j as f64 * g.spacing);
            let p = [
                g.origin[0] + u * along[0] + v * across[0],
                g.origin[1] + u * along[1] + v * across[1],
            ];
            if inside(p, outline)
                && loops[1..].iter().all(|hole| !inside(p, hole))
                && loops.iter().all(|l| distance_to(p, l) >= clearance)
            {
                out.push(p);
                if out.len() >= most {
                    break 'rows;
                }
            }
        }
    }
    out
}

/// The most lattice cells an outline's box may take to walk: a 2 m × 2 m piece at 4 mm
/// cells is 250,000; this leaves room for a long piece on the bias.
const MOST_CELLS: f64 = 4_000_000.0;

/// Whether `p` is inside the closed polygon `ring` (even–odd rule).
fn inside(p: [f64; 2], ring: &[[f64; 2]]) -> bool {
    let mut inside = false;
    for k in 0..ring.len() {
        let (a, b) = (ring[k], ring[(k + 1) % ring.len()]);
        if (a[1] > p[1]) != (b[1] > p[1]) {
            let x = a[0] + (p[1] - a[1]) / (b[1] - a[1]) * (b[0] - a[0]);
            if p[0] < x {
                inside = !inside;
            }
        }
    }
    inside
}

/// The distance from `p` to the nearest segment of the closed polygon `ring`.
fn distance_to(p: [f64; 2], ring: &[[f64; 2]]) -> f64 {
    let mut best = f64::MAX;
    for k in 0..ring.len() {
        let (a, b) = (ring[k], ring[(k + 1) % ring.len()]);
        let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
        let len2 = dx * dx + dy * dy;
        let t = if len2 > 0.0 {
            (((p[0] - a[0]) * dx + (p[1] - a[1]) * dy) / len2).clamp(0.0, 1.0)
        } else {
            0.0
        };
        let (qx, qy) = (a[0] + t * dx, a[1] + t * dy);
        best = best.min((p[0] - qx).hypot(p[1] - qy));
    }
    best
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Points at most `h` apart round the polygon `corners`, corners included.
    fn sampled(corners: &[[f64; 2]], h: f64) -> Vec<[f64; 2]> {
        let mut out = Vec::new();
        for k in 0..corners.len() {
            let (a, b) = (corners[k], corners[(k + 1) % corners.len()]);
            let steps = (((b[0] - a[0]).hypot(b[1] - a[1]) / h).round() as usize).max(1);
            for s in 0..steps {
                let t = s as f64 / steps as f64;
                out.push([a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t]);
            }
        }
        out
    }

    #[test]
    fn keeps_the_boundary_and_leaves_holes_empty() {
        let outline = sampled(
            &[[0.0, 0.0], [400.0, 0.0], [400.0, 400.0], [0.0, 400.0]],
            12.0,
        );
        let hole: Vec<[f64; 2]> = (0..31)
            .map(|k| {
                let a = -(k as f64) / 31.0 * std::f64::consts::TAU; // clockwise
                [200.0 + 60.0 * a.cos(), 200.0 + 60.0 * a.sin()]
            })
            .collect();
        let grid = Grid {
            origin: [200.0, 200.0],
            angle_deg: 90.0,
            spacing: 12.0,
        };
        for grid in [None, Some(grid)] {
            let t = triangulate_with(&outline, std::slice::from_ref(&hole), 12.0, grid, 100_000)
                .unwrap();
            let given: Vec<[f64; 2]> = outline.iter().chain(&hole).copied().collect();
            assert_eq!(
                &t.points[..given.len()],
                &given[..],
                "boundary first and unchanged"
            );
            for tri in &t.triangles {
                let c = tri.map(|k| t.points[k as usize]);
                let (x, y) = (
                    (c[0][0] + c[1][0] + c[2][0]) / 3.0,
                    (c[0][1] + c[1][1] + c[2][1]) / 3.0,
                );
                assert!(
                    (x - 200.0).hypot(y - 200.0) > 58.0,
                    "a triangle in the hole"
                );
                let twice_area = (c[1][0] - c[0][0]) * (c[2][1] - c[0][1])
                    - (c[2][0] - c[0][0]) * (c[1][1] - c[0][1]);
                assert!(twice_area > 0.0, "anticlockwise");
            }
        }
    }

    #[test]
    fn a_lattice_fills_the_inside_on_the_grain_and_keeps_clear_of_the_outline() {
        let outline = sampled(
            &[[0.0, 0.0], [240.0, 0.0], [240.0, 120.0], [0.0, 120.0]],
            12.0,
        );
        let hole: Vec<[f64; 2]> = (0..16)
            .map(|k| {
                let a = -(k as f64) / 16.0 * std::f64::consts::TAU;
                [120.0 + 24.0 * a.cos(), 60.0 + 24.0 * a.sin()]
            })
            .collect();
        let grid = Grid {
            origin: [6.0, 6.0],
            angle_deg: 0.0,
            spacing: 12.0,
        };
        let points = lattice(&[&outline, &hole], &grid, 6.0, usize::MAX);
        // Rows 6, 18, …, 114 (10 of them) by columns 6, 18, …, 234 (20), less the hole.
        assert!(points.len() < 200 && points.len() > 160, "{}", points.len());
        for p in &points {
            assert!(
                ((p[0] - 6.0) / 12.0).fract().abs() < 1e-9
                    && ((p[1] - 6.0) / 12.0).fract().abs() < 1e-9,
                "on the lattice: {p:?}"
            );
            assert!(distance_to(*p, &outline) >= 6.0 - 1e-9);
            assert!(distance_to(*p, &hole) >= 6.0 - 1e-9 && !inside(*p, &hole));
        }
        // Row by row along the grain: y never decreases, x increases within a row.
        for w in points.windows(2) {
            assert!(w[1][1] > w[0][1] - 1e-9);
            assert!(w[1][1] > w[0][1] + 1e-9 || w[1][0] > w[0][0]);
        }
        // Rotated 90°, the rows run up the piece instead, and the rows follow each other
        // across the grain (leftwards, as the grain's left-hand side is).
        let up = lattice(
            &[&outline],
            &Grid {
                angle_deg: 90.0,
                ..grid
            },
            6.0,
            usize::MAX,
        );
        assert!(up.windows(2).all(|w| w[1][0] < w[0][0] + 1e-9));
        assert!(
            up.windows(2)
                .all(|w| w[1][0] < w[0][0] - 1e-9 || w[1][1] > w[0][1])
        );
        // The triangles inside are the lattice's right isosceles cells, and the lattice's
        // points come right after the boundary's, in order.
        let t = triangulate_with(&outline, &[], 12.0, Some(grid), 100_000).unwrap();
        let only_outline = lattice(&[&outline], &grid, CLEARANCE_PER_H * 12.0, usize::MAX);
        assert_eq!(
            &t.points[outline.len()..outline.len() + only_outline.len()],
            &only_outline[..]
        );
        // Rows 18 … 102 (8) by columns 18 … 222 (18) stay clear of the outline: 7 × 17 cells.
        assert_eq!(only_outline.len(), 8 * 18);
        let cells = t
            .triangles
            .iter()
            .filter(|tri| tri.iter().all(|&k| k as usize >= outline.len()))
            .count();
        assert!(cells >= 2 * 7 * 17 - 20, "{cells} cells inside");
    }

    #[test]
    fn the_lattice_stops_at_its_budget_and_skips_a_piece_too_big_to_walk() {
        let outline = sampled(
            &[[0.0, 0.0], [240.0, 0.0], [240.0, 120.0], [0.0, 120.0]],
            12.0,
        );
        let grid = Grid {
            origin: [6.0, 6.0],
            angle_deg: 0.0,
            spacing: 12.0,
        };
        assert_eq!(lattice(&[&outline], &grid, 6.0, 5).len(), 5);
        assert_eq!(lattice(&[&outline], &grid, 6.0, 0).len(), 0);
        // A strip 3 km long on the bias: its box would be billions of cells.
        let strip = [
            [0.0, 0.0],
            [3e6, 3e6],
            [3e6 - 10.0, 3e6 + 10.0],
            [-10.0, 10.0],
        ];
        let started = std::time::Instant::now();
        assert!(lattice(&[&strip], &grid, 6.0, usize::MAX).is_empty());
        assert!(started.elapsed().as_secs_f64() < 1.0);
        let t = triangulate_with(&strip, &[], 12.0, Some(grid), 50).unwrap();
        assert!(
            t.points.len() <= 54,
            "{} points, complete: {}",
            t.points.len(),
            t.refinement_complete
        );
    }

    #[test]
    fn crossings_and_repeated_points_are_refused() {
        let bow = [[0.0, 0.0], [100.0, 100.0], [100.0, 0.0], [0.0, 100.0]];
        assert_eq!(
            triangulate(&bow, &[], 12.0, 1000),
            Err(TriangulateError::CrossesItself)
        );
        let repeated = [
            [0.0, 0.0],
            [100.0, 0.0],
            [100.0, 100.0],
            [100.0, 0.0],
            [0.0, 100.0],
        ];
        assert_eq!(
            triangulate(&repeated, &[], 12.0, 1000),
            Err(TriangulateError::CrossesItself)
        );
        let flat = [[0.0, 0.0], [1.0, 0.0], [2.0, 0.0]];
        assert_eq!(
            triangulate(&flat, &[], 12.0, 1000),
            Err(TriangulateError::CrossesItself)
        );
        let nan = [[0.0, 0.0], [f64::NAN, 0.0], [0.0, 1.0]];
        assert_eq!(
            triangulate(&nan, &[], 12.0, 1000),
            Err(TriangulateError::NotFinite)
        );
        assert_eq!(
            triangulate(&nan[..2], &[], 12.0, 1000),
            Err(TriangulateError::TooFewPoints)
        );
        let square = sampled(
            &[[0.0, 0.0], [100.0, 0.0], [100.0, 100.0], [0.0, 100.0]],
            12.0,
        );
        let across: Vec<[f64; 2]> = (0..12)
            .map(|k| {
                let a = k as f64 / 12.0 * std::f64::consts::TAU;
                [100.0 + 30.0 * a.cos(), 50.0 + 30.0 * a.sin()]
            })
            .collect();
        assert_eq!(
            triangulate(&square, &[across], 12.0, 1000),
            Err(TriangulateError::CrossesItself)
        );
    }

    #[test]
    fn refinement_says_when_it_ran_out_of_points() {
        let square = sampled(
            &[[0.0, 0.0], [400.0, 0.0], [400.0, 400.0], [0.0, 400.0]],
            12.0,
        );
        let enough = triangulate(&square, &[], 12.0, 100_000).unwrap();
        assert!(enough.refinement_complete);
        // A 400 mm square needs well over 200 more points than the 132 round its edge.
        let short = triangulate(&square, &[], 12.0, 50).unwrap();
        assert!(!short.refinement_complete);
        assert!(short.points.len() <= square.len() + 50);
        assert!(short.points.len() < enough.points.len());
    }

    #[test]
    fn the_same_outline_gives_the_same_triangles() {
        let disk: Vec<[f64; 2]> = (0..79)
            .map(|k| {
                let a = k as f64 / 79.0 * std::f64::consts::TAU;
                [3.0 + 150.0 * a.cos(), 7.0 + 150.0 * a.sin()]
            })
            .collect();
        let grid = Some(Grid {
            origin: [3.0, 7.0],
            angle_deg: 30.0,
            spacing: 12.0,
        });
        assert_eq!(
            triangulate_with(&disk, &[], 12.0, grid, 100_000),
            triangulate_with(&disk, &[], 12.0, grid, 100_000)
        );
    }
}
