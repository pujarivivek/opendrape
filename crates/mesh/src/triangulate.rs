//! One panel's fabric: a constrained Delaunay triangulation of its outline (and holes),
//! refined until no triangle is too big or too thin. Every boundary point is kept, in order,
//! so the points sampled along a seam are exactly the points that get stitched.

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
/// Triangles smaller than this many squared edge lengths are never split further.
const MIN_AREA_PER_H2: f64 = 0.02;

/// A triangulated panel, in the units of its input.
#[derive(Clone, Debug, PartialEq)]
pub struct Triangulated {
    /// The boundary points first, exactly as given (outline, then each hole), then the points
    /// added inside.
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
            // spade refuses coordinates below 2^-142 in size: such a number is zero here.
            let tidy = |v: f64| if v.abs() < 1e-9 { 0.0 } else { v };
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
    let params = RefinementParameters::<f64>::new()
        .with_angle_limit(AngleLimit::from_deg(ANGLE_LIMIT_DEG))
        .with_max_allowed_area(MAX_AREA_PER_H2 * h * h)
        .with_min_required_area(MIN_AREA_PER_H2 * h * h)
        .keep_constraint_edges()
        .exclude_outer_faces(true)
        .with_max_additional_vertices(max_added);
    let result = cdt.refine(params);
    let refinement_complete = result.refinement_complete;
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
        let t = triangulate(&outline, std::slice::from_ref(&hole), 12.0, 100_000).unwrap();
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
        assert_eq!(
            triangulate(&disk, &[], 12.0, 100_000),
            triangulate(&disk, &[], 12.0, 100_000)
        );
    }
}
