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

    /// The contact planes of the particles `which` (indices into `x`), each with how far that
    /// particle is from the surface (negative inside): the solver asks again about a particle
    /// only once it has moved far enough to reach the surface. A collider that can't say how
    /// far a particle outside the margin is reports the margin, and is asked every frame.
    fn contacts(&self, x: &[DVec3], which: &[u32], margin: f64) -> Vec<(Option<Plane>, f64)> {
        let planes = self.contact_planes(x, margin);
        which
            .iter()
            .map(|&i| {
                let plane = planes[i as usize];
                let clearance = plane.map_or(margin, |p| (x[i as usize] - p.point).dot(p.normal));
                (plane, clearance)
            })
            .collect()
    }
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

impl BodyCollider {
    /// The contact plane for a particle at `p` (None beyond `margin`), and how far it is
    /// from the surface (negative inside).
    fn contact(&self, p: DVec3, margin: f64) -> (Option<Plane>, f64) {
        let (proj, (tri, _)) = self
            .mesh
            .project_local_point_and_get_location(p.as_vec3(), false);
        let point = proj.point.as_dvec3();
        let d = p - point;
        let dist = d.length();
        if !proj.is_inside && dist > margin {
            return (None, dist);
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
        let signed = if proj.is_inside { -dist } else { dist };
        (Some(Plane { normal, point }), signed)
    }
}

impl Collider for BodyCollider {
    fn contact_planes(&self, x: &[DVec3], margin: f64) -> Vec<Option<Plane>> {
        x.par_iter().map(|p| self.contact(*p, margin).0).collect()
    }

    fn contacts(&self, x: &[DVec3], which: &[u32], margin: f64) -> Vec<(Option<Plane>, f64)> {
        which
            .par_iter()
            .map(|&i| self.contact(x[i as usize], margin))
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
///
/// Where parts overlap, the "nearest way out" of one part can land inside another, so cloth
/// should start outside every part.
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

impl CompoundCollider {
    /// The plane to use for a particle at `p` among its parts' and the floor's, with how far
    /// it is from the nearest surface: inside anything, the nearest way out; otherwise the
    /// nearest surface.
    fn pick(
        &self,
        p: DVec3,
        margin: f64,
        parts: impl Iterator<Item = (Option<Plane>, f64)>,
    ) -> (Option<Plane>, f64) {
        let floor = self.floor.map(|f| {
            let d = p.y - f;
            let plane = (d < margin).then_some(Plane {
                normal: DVec3::Y,
                point: DVec3::new(p.x, f, p.z),
            });
            (plane, d)
        });
        let mut clearance = f64::INFINITY;
        let mut best: Option<(f64, Plane)> = None;
        for (plane, d) in parts.chain(floor) {
            clearance = clearance.min(d);
            if let Some(plane) = plane {
                let better = best.is_none_or(|(bd, _)| {
                    (d >= 0.0)
                        .cmp(&(bd >= 0.0))
                        .then(d.abs().total_cmp(&bd.abs()))
                        .is_lt()
                });
                if better {
                    best = Some((d, plane));
                }
            }
        }
        (best.map(|(_, plane)| plane), clearance)
    }
}

impl Collider for CompoundCollider {
    fn contact_planes(&self, x: &[DVec3], margin: f64) -> Vec<Option<Plane>> {
        let all: Vec<u32> = (0..x.len() as u32).collect();
        self.contacts(x, &all, margin)
            .into_iter()
            .map(|(plane, _)| plane)
            .collect()
    }

    fn contacts(&self, x: &[DVec3], which: &[u32], margin: f64) -> Vec<(Option<Plane>, f64)> {
        let per_part: Vec<Vec<(Option<Plane>, f64)>> = self
            .parts
            .iter()
            .map(|c| c.contacts(x, which, margin))
            .collect();
        which
            .par_iter()
            .enumerate()
            .map(|(k, &i)| self.pick(x[i as usize], margin, per_part.iter().map(|found| found[k])))
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
    fn inside_any_part_beats_a_nearer_surface_outside_another() {
        // Cubes overlap between x = 0.3 and 0.5. At x = 0.29 the point is inside the first
        // cube (0.21 from its +X face) and only 0.01 outside the second (its -X face).
        let c = CompoundCollider::new(vec![cube_at(0.0), cube_at(0.8)], None);
        let p = c.contact_planes(&[DVec3::new(0.29, 0.0, 0.0)], 0.05)[0]
            .expect("inside the first cube");
        assert!(
            p.normal.abs_diff_eq(DVec3::X, 1e-6) && (p.point.x - 0.5).abs() < 1e-6,
            "out through the first cube, not onto the second's near face: {p:?}"
        );
    }

    #[test]
    fn inside_a_part_beats_a_nearer_floor() {
        // The floor is only 0.02 below the particle, the cube's +Z face is 0.2 away.
        let c = CompoundCollider::new(vec![cube()], Some(-0.02));
        let p = c.contact_planes(&[DVec3::new(0.0, 0.0, 0.3)], 0.05)[0].expect("inside the cube");
        assert!(
            p.normal.abs_diff_eq(DVec3::Z, 1e-6) && (p.point.z - 0.5).abs() < 1e-6,
            "out of the cube, not up from the floor: {p:?}"
        );
    }

    #[test]
    fn an_empty_compound_collides_with_nothing() {
        let c = CompoundCollider::new(vec![], None);
        assert!(c.parts().is_empty());
        let x = [DVec3::ZERO, DVec3::new(0.0, -5.0, 0.0)];
        assert_eq!(c.contact_planes(&x, 0.05), vec![None, None]);
        assert_eq!(c.signed_distance(DVec3::ZERO), f64::INFINITY);
    }

    #[test]
    fn contacts_say_how_far_each_asked_particle_is() {
        let c = CompoundCollider::new(vec![cube()], Some(-1.0));
        let x = [
            DVec3::new(0.0, 0.0, 0.3),   // inside the cube, 0.2 from its +Z face
            DVec3::new(0.0, 0.0, 0.52),  // 0.02 outside
            DVec3::new(0.0, 3.0, 0.0),   // far from everything: 2.5 above the cube
            DVec3::new(5.0, -0.98, 0.0), // 0.02 above the floor
        ];
        let found = c.contacts(&x, &[3, 0, 2], 0.05);
        assert_eq!(found.len(), 3);
        assert!(
            found[0].0.is_some() && (found[0].1 - 0.02).abs() < 1e-6,
            "{:?}",
            found[0]
        );
        assert!(
            found[1].0.is_some() && (found[1].1 + 0.2).abs() < 1e-6,
            "{:?}",
            found[1]
        );
        assert!(
            found[2].0.is_none() && (found[2].1 - 2.5).abs() < 1e-6,
            "{:?}",
            found[2]
        );
        // A single body says the same.
        let one = cube().contacts(&x, &[1, 2], 0.05);
        assert!(one[0].0.is_some() && (one[0].1 - 0.02).abs() < 1e-6);
        assert!(one[1].0.is_none() && (one[1].1 - 2.5).abs() < 1e-6);
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
