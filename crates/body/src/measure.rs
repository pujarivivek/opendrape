use crate::BodyMesh;
use glam::DVec2;
use std::collections::HashMap;

/// Height from the lowest to the highest vertex.
pub fn height(mesh: &BodyMesh) -> f32 {
    let (lo, hi) = mesh
        .positions
        .iter()
        .fold((f32::MAX, f32::MIN), |(lo, hi), p| {
            (lo.min(p.y), hi.max(p.y))
        });
    hi - lo
}

/// Edges used by exactly one triangle: 0 for a closed mesh.
pub fn boundary_edge_count(mesh: &BodyMesh) -> usize {
    let mut uses: HashMap<(u32, u32), u32> = HashMap::new();
    for t in &mesh.triangles {
        for k in 0..3 {
            let (a, b) = (t[k], t[(k + 1) % 3]);
            *uses.entry((a.min(b), a.max(b))).or_default() += 1;
        }
    }
    uses.values().filter(|&&n| n == 1).count()
}

/// Tape-measure girth at height `y`: perimeter of the convex hull of the mesh's
/// cross-section, keeping only points with |x| < `max_abs_x` (to leave out the arms).
/// Vertices lying exactly on the plane count (a dress form has a ring at every station).
pub fn girth_at(mesh: &BodyMesh, y: f32, max_abs_x: f32) -> f32 {
    let mut pts = vec![];
    for t in &mesh.triangles {
        for k in 0..3 {
            let (a, b) = (
                mesh.positions[t[k] as usize],
                mesh.positions[t[(k + 1) % 3] as usize],
            );
            if a.y == y && a.x.abs() < max_abs_x {
                pts.push(DVec2::new(f64::from(a.x), f64::from(a.z)));
            }
            if (a.y - y) * (b.y - y) < 0.0 {
                let p = a + (b - a) * ((y - a.y) / (b.y - a.y));
                if p.x.abs() < max_abs_x {
                    pts.push(DVec2::new(f64::from(p.x), f64::from(p.z)));
                }
            }
        }
    }
    hull_perimeter(pts) as f32
}

/// Andrew's monotone chain convex hull: its corners in counter-clockwise order, or none when
/// there are fewer than 3 distinct points. Collinear points leave just the two ends.
pub(crate) fn convex_hull(mut pts: Vec<DVec2>) -> Vec<DVec2> {
    pts.sort_by(|a, b| a.x.total_cmp(&b.x).then(a.y.total_cmp(&b.y)));
    pts.dedup();
    if pts.len() < 3 {
        return vec![];
    }
    fn half(points: impl Iterator<Item = DVec2>) -> Vec<DVec2> {
        let mut h: Vec<DVec2> = vec![];
        for p in points {
            while h.len() >= 2
                && (h[h.len() - 1] - h[h.len() - 2]).perp_dot(p - h[h.len() - 2]) <= 0.0
            {
                h.pop();
            }
            h.push(p);
        }
        h.pop();
        h
    }
    let mut hull = half(pts.iter().copied());
    hull.extend(half(pts.iter().rev().copied()));
    hull
}

/// The perimeter of the convex hull of `pts`.
pub(crate) fn hull_perimeter(pts: Vec<DVec2>) -> f64 {
    let hull = convex_hull(pts);
    (0..hull.len())
        .map(|i| hull[i].distance(hull[(i + 1) % hull.len()]))
        .sum()
}

#[cfg(test)]
mod tests {
    use super::*;
    use glam::Vec3;

    /// Closed unit cube centred on the origin, triangles counter-clockwise from outside.
    fn cube() -> BodyMesh {
        let positions = [
            [-0.5, -0.5, -0.5],
            [0.5, -0.5, -0.5],
            [0.5, 0.5, -0.5],
            [-0.5, 0.5, -0.5],
            [-0.5, -0.5, 0.5],
            [0.5, -0.5, 0.5],
            [0.5, 0.5, 0.5],
            [-0.5, 0.5, 0.5],
        ]
        .map(Vec3::from_array)
        .to_vec();
        let triangles = vec![
            [0, 3, 2],
            [0, 2, 1],
            [4, 5, 6],
            [4, 6, 7],
            [0, 4, 7],
            [0, 7, 3],
            [1, 2, 6],
            [1, 6, 5],
            [0, 1, 5],
            [0, 5, 4],
            [3, 7, 6],
            [3, 6, 2],
        ];
        BodyMesh {
            positions,
            triangles,
        }
    }

    #[test]
    fn cube_height_and_girth() {
        assert!((height(&cube()) - 1.0).abs() < 1e-6);
        assert!((girth_at(&cube(), 0.0, 10.0) - 4.0).abs() < 1e-5);
    }

    #[test]
    fn girth_ignores_points_outside_the_x_limit() {
        // At y = 0 the slice points are the 4 corners (|x| = 0.5), the ±X face-diagonal
        // crossings (|x| = 0.5) and the ±Z face-diagonal crossings (x = 0). With |x| < 0.4
        // only the two x = 0 points remain: no area, so no girth.
        assert_eq!(girth_at(&cube(), 0.0, 0.4), 0.0);
    }

    #[test]
    fn girth_counts_vertices_lying_on_the_slice_plane() {
        // y = 0.5 is the cube's top face: no edge crosses it, but its four corners lie on it.
        assert!((girth_at(&cube(), 0.5, 10.0) - 4.0).abs() < 1e-5);
    }

    #[test]
    fn closed_and_open_meshes() {
        assert_eq!(boundary_edge_count(&cube()), 0);
        let mut open = cube();
        open.triangles.truncate(10); // remove the top face (2 triangles)
        assert_eq!(boundary_edge_count(&open), 4);
    }
}
