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

/// A collider that can also say how far a point is from its surface (negative inside),
/// for measuring a drape after the fact.
pub trait Solid: Collider {
    fn signed_distance(&self, p: DVec3) -> f64;
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

impl Solid for BodyCollider {
    fn signed_distance(&self, p: DVec3) -> f64 {
        // The inherent method of the same name.
        BodyCollider::signed_distance(self, p)
    }
}

/// Several closed bodies (a dress form's torso, and later its arms or legs) and an optional
/// floor at height `floor`, as one collider.
pub struct CompoundCollider {
    parts: Vec<BodyCollider>,
    floor: Option<f64>,
}

impl CompoundCollider {
    pub fn new(parts: Vec<BodyCollider>, floor: Option<f64>) -> Self {
        Self { parts, floor }
    }

    pub fn parts(&self) -> &[BodyCollider] {
        &self.parts
    }
}

impl Collider for CompoundCollider {
    fn contact_planes(&self, x: &[DVec3], margin: f64) -> Vec<Option<Plane>> {
        let per_part: Vec<Vec<Option<Plane>>> = self
            .parts
            .iter()
            .map(|c| c.contact_planes(x, margin))
            .collect();
        (0..x.len())
            .into_par_iter()
            .map(|i| {
                let floor = self.floor.filter(|f| x[i].y - f < margin).map(|f| Plane {
                    normal: DVec3::Y,
                    point: DVec3::new(x[i].x, f, x[i].z),
                });
                per_part
                    .iter()
                    .filter_map(|planes| planes[i])
                    .chain(floor)
                    .map(|p| ((x[i] - p.point).dot(p.normal), p))
                    // Inside anything: the nearest way out. Otherwise: the nearest surface.
                    .min_by(|(a, _), (b, _)| {
                        (*a >= 0.0)
                            .cmp(&(*b >= 0.0))
                            .then(a.abs().total_cmp(&b.abs()))
                    })
                    .map(|(_, p)| p)
            })
            .collect()
    }
}

impl Solid for CompoundCollider {
    fn signed_distance(&self, p: DVec3) -> f64 {
        let parts = self
            .parts
            .iter()
            .map(|c| c.signed_distance(p))
            .fold(f64::INFINITY, f64::min);
        self.floor.map_or(parts, |f| parts.min(p.y - f))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ClothBuilder, Panel, Params, Solver};
    use glam::{DVec2, Vec3};

    /// Unit cube centred on (dx, 0, 0).
    fn cube_at(dx: f32) -> BodyCollider {
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
        .map(|c| Vec3::from_array(c) + Vec3::X * dx);
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

    fn cube() -> BodyCollider {
        cube_at(0.0)
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

    #[test]
    fn compound_takes_the_nearest_way_out_then_the_nearest_surface() {
        // Two cubes overlapping between x = 0.3 and 0.5.
        let c = CompoundCollider::new(vec![cube_at(0.0), cube_at(0.8)], None);
        let planes = c.contact_planes(
            &[
                DVec3::new(0.0, 0.0, 0.45),
                DVec3::new(0.9, 0.0, 0.0),
                DVec3::new(0.0, 0.53, 0.0),
                DVec3::new(0.0, 3.0, 0.0),
                DVec3::new(0.45, 0.0, 0.0),
            ],
            0.05,
        );
        let p = planes[0].expect("inside the first cube");
        assert!(p.normal.abs_diff_eq(DVec3::Z, 1e-6), "{p:?}");
        let p = planes[1].expect("inside the second cube");
        assert!(
            p.normal.abs_diff_eq(DVec3::X, 1e-6)
                && p.point.abs_diff_eq(DVec3::new(1.3, 0.0, 0.0), 1e-6),
            "{p:?}"
        );
        let p = planes[2].expect("just above the first cube");
        assert!(p.normal.abs_diff_eq(DVec3::Y, 1e-6), "{p:?}");
        assert_eq!(planes[3], None, "far from everything");
        let p = planes[4].expect("inside both");
        assert!(
            p.normal.abs_diff_eq(DVec3::X, 1e-6) && (p.point.x - 0.5).abs() < 1e-6,
            "nearest way out, not the far side: {p:?}"
        );
    }

    #[test]
    fn floor_holds_particles_above_it() {
        let c = CompoundCollider::new(vec![], Some(0.0));
        let planes = c.contact_planes(
            &[
                DVec3::new(5.0, 0.01, 5.0),
                DVec3::new(5.0, -0.1, 5.0),
                DVec3::new(5.0, 1.0, 5.0),
            ],
            0.05,
        );
        let on_floor = Some(Plane {
            normal: DVec3::Y,
            point: DVec3::new(5.0, 0.0, 5.0),
        });
        assert_eq!(planes[0], on_floor);
        assert_eq!(planes[1], on_floor, "below the floor is pushed back up");
        assert_eq!(planes[2], None);
    }

    #[test]
    fn signed_distance_is_the_union_of_parts_and_floor() {
        let c = CompoundCollider::new(vec![cube_at(0.0), cube_at(0.8)], Some(-0.6));
        assert!((c.signed_distance(DVec3::new(0.0, 0.0, 0.3)) + 0.2).abs() < 1e-6);
        assert!((c.signed_distance(DVec3::new(0.9, 0.0, 0.0)) + 0.4).abs() < 1e-6);
        assert!(
            (c.signed_distance(DVec3::new(3.0, -0.7, 0.0)) + 0.1).abs() < 1e-6,
            "below the floor"
        );
        let single: &dyn Solid = &cube();
        assert!((single.signed_distance(DVec3::new(0.0, 0.0, 0.8)) - 0.3).abs() < 1e-6);
    }

    #[test]
    fn a_compound_collider_is_both_a_collider_and_a_solid() {
        let c = CompoundCollider::new(vec![cube()], Some(-1.0));
        let as_collider: &dyn Collider = &c;
        let as_solid: &dyn Solid = &c;
        let p = DVec3::new(0.0, 0.0, 0.3);
        assert!(as_collider.contact_planes(&[p], 0.05)[0].is_some());
        assert!((as_solid.signed_distance(p) + 0.2).abs() < 1e-6);
        // A `Solid` is a `Collider`, so a drape measured through `&dyn Solid` can also be
        // simulated against.
        let upcast: &dyn Collider = as_solid;
        assert!(upcast.contact_planes(&[p], 0.05)[0].is_some());
    }

    #[test]
    fn cloth_settles_on_the_floor() {
        let c = CompoundCollider::new(vec![], Some(0.0));
        let mut s = falling_triangle(0.2);
        for _ in 0..120 {
            s.step(Some(&c));
        }
        for p in s.cloth().positions() {
            assert!(
                p.y >= 0.003 - 1e-4 && p.y < 0.02,
                "resting on the floor: {p}"
            );
        }
    }
}
