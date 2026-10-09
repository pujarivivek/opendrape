use glam::DVec3;

/// A contact plane for one particle: the closest point on the body and the outward normal there.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Plane {
    pub normal: DVec3,
    pub point: DVec3,
}

/// Something cloth collides with. Queried once per frame for every particle; `None` means
/// the particle is farther than `margin` from it (and outside).
pub trait Collider: Sync {
    fn contact_planes(&self, x: &[DVec3], margin: f64) -> Vec<Option<Plane>>;
}

use parry3d::query::{PointQuery, PointQueryWithLocation, Ray, RayCast};
use parry3d::shape::{TriMesh, TriMeshFlags};
use rayon::prelude::*;

#[derive(Debug)]
pub struct ColliderError(pub String);

/// Exact collision against a closed, consistently wound triangle mesh (the body).
pub struct BodyCollider {
    mesh: TriMesh,
}

impl BodyCollider {
    pub fn new(positions: &[glam::Vec3], triangles: &[[u32; 3]]) -> Result<Self, ColliderError> {
        let mut mesh = TriMesh::new(positions.to_vec(), triangles.to_vec())
            .map_err(|e| ColliderError(format!("{e:?}")))?;
        // ORIENTED is required for is_inside; `with_flags` would silently drop errors.
        mesh.set_flags(TriMeshFlags::ORIENTED | TriMeshFlags::MERGE_DUPLICATE_VERTICES)
            .map_err(|e| ColliderError(format!("{e:?}")))?;
        Ok(Self { mesh })
    }
    /// Distance to the surface, negative inside.
    pub fn signed_distance(&self, p: DVec3) -> f64 {
        f64::from(self.mesh.distance_to_local_point(p.as_vec3(), false))
    }
    /// Distance from `origin` (inside the body) along unit `dir` to where the ray leaves it.
    pub fn ray_exit(&self, origin: DVec3, dir: DVec3, max: f64) -> Option<f64> {
        let ray = Ray::new(origin.as_vec3(), dir.as_vec3());
        self.mesh
            .cast_local_ray(&ray, max as f32, false)
            .map(f64::from)
    }
}

impl Collider for BodyCollider {
    fn contact_planes(&self, x: &[DVec3], margin: f64) -> Vec<Option<Plane>> {
        x.par_iter()
            .map(|p| {
                let (proj, (tri, _)) = self
                    .mesh
                    .project_local_point_and_get_location(p.as_vec3(), false);
                let point = proj.point.as_dvec3();
                let d = *p - point;
                let dist = d.length();
                if !proj.is_inside && dist > margin {
                    return None;
                }
                let normal = if dist > 1e-7 {
                    if proj.is_inside { -d / dist } else { d / dist }
                } else {
                    self.mesh
                        .triangle(tri)
                        .normal()
                        .unwrap_or(glam::Vec3::Y)
                        .as_dvec3()
                };
                Some(Plane { normal, point })
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ClothBuilder, Panel, Params, Solver};
    use glam::{DVec2, Vec3};

    fn cube() -> BodyCollider {
        let p = [
            [-0.5, -0.5, -0.5],
            [0.5, -0.5, -0.5],
            [0.5, 0.5, -0.5],
            [-0.5, 0.5, -0.5],
            [-0.5, -0.5, 0.5],
            [0.5, -0.5, 0.5],
            [0.5, 0.5, 0.5],
            [-0.5, 0.5, 0.5],
        ]
        .map(Vec3::from_array);
        let t = [
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
        BodyCollider::new(&p, &t).expect("closed cube")
    }

    #[test]
    fn planes_point_outward_inside_and_out() {
        let c = cube();
        let planes = c.contact_planes(
            &[
                DVec3::new(0.0, 0.0, 0.52),
                DVec3::new(0.0, 0.0, 0.3),
                DVec3::new(0.0, 0.0, 5.0),
            ],
            0.05,
        );
        let outside = planes[0].expect("near the +Z face");
        assert!(
            outside.normal.abs_diff_eq(DVec3::Z, 1e-6)
                && outside.point.abs_diff_eq(DVec3::new(0.0, 0.0, 0.5), 1e-6)
        );
        let inside = planes[1].expect("inside always gets a plane");
        assert!(
            inside.normal.abs_diff_eq(DVec3::Z, 1e-6),
            "outward even from inside: {:?}",
            inside.normal
        );
        assert_eq!(planes[2], None, "far away");
    }

    #[test]
    fn signed_distance_and_ray_exit() {
        let c = cube();
        assert!((c.signed_distance(DVec3::new(0.0, 0.0, 0.3)) + 0.2).abs() < 1e-6);
        assert!((c.signed_distance(DVec3::new(0.0, 0.0, 0.8)) - 0.3).abs() < 1e-6);
        assert!((c.ray_exit(DVec3::ZERO, DVec3::X, 2.0).unwrap() - 0.5).abs() < 1e-6);
    }

    fn falling_triangle(y: f64) -> Solver {
        let flat = vec![
            DVec2::new(-0.05, -0.05),
            DVec2::new(0.05, -0.05),
            DVec2::new(0.0, 0.05),
        ];
        let panel = Panel {
            positions: flat.iter().map(|p| DVec3::new(p.x, y, p.y)).collect(),
            flat: Some(flat),
            triangles: vec![[0, 1, 2]],
        };
        let mut b = ClothBuilder::new(0.15);
        b.add_panel(&panel, 1.0);
        Solver::new(
            b.build(),
            Params {
                gravity_delay: 0.0,
                ..Params::default()
            },
        )
    }

    #[test]
    fn cloth_lands_on_the_body_and_stays_outside() {
        let c = cube();
        let mut s = falling_triangle(0.6);
        for _ in 0..120 {
            s.step(Some(&c));
        }
        for p in s.cloth().positions() {
            assert!(
                p.y >= 0.5 + 0.003 - 1e-4 && p.y < 0.52,
                "resting on top: {p}"
            );
        }
    }

    #[test]
    fn cloth_that_starts_inside_is_pushed_out() {
        let c = cube();
        let mut s = falling_triangle(0.47);
        s.step(Some(&c));
        assert!(
            s.cloth()
                .positions()
                .iter()
                .all(|p| c.signed_distance(*p) > 0.0)
        );
    }
}
