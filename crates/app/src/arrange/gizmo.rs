//! The gizmo's maths, kept apart from egui so it can be tested on its own: the 3D view as seen
//! on screen, picking with a ray, the handles and where they are, and what dragging each one
//! means in metres or radians.

use glam::{DMat4, DVec2, DVec3, DVec4};
use opendrape_core::MAX_PLACEMENT_M;
use opendrape_render::OrbitCamera;

/// The 3D view as it appears on screen: world points (m) ↔ screen points, for a camera drawn
/// into a rectangle at `min` of size `size` (screen points).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ScreenCamera {
    view_proj: DMat4,
    inverse: DMat4,
    min: DVec2,
    size: DVec2,
    eye: DVec3,
    forward: DVec3,
    fov_y: f64,
}

impl ScreenCamera {
    pub fn new(camera: &OrbitCamera, min: DVec2, size: DVec2) -> Self {
        let size = size.max(DVec2::ONE);
        let view_proj = camera.view_proj((size.x / size.y) as f32).as_dmat4();
        let eye = camera.eye().as_dvec3();
        Self {
            view_proj,
            inverse: view_proj.inverse(),
            min,
            size,
            eye,
            forward: (camera.target.as_dvec3() - eye).normalize_or(DVec3::NEG_Z),
            fov_y: f64::from(camera.fov_y),
        }
    }
    pub fn eye(&self) -> DVec3 {
        self.eye
    }
    /// The top-left corner of the view on screen, and its size (screen points).
    pub fn rect(&self) -> (DVec2, DVec2) {
        (self.min, self.size)
    }
    /// Where `p` shows on screen; None when it is behind the camera.
    pub fn project(&self, p: DVec3) -> Option<DVec2> {
        let c = self.view_proj * p.extend(1.0);
        if c.w <= 1e-9 {
            return None;
        }
        let ndc = c.truncate() / c.w;
        Some(self.min + DVec2::new((ndc.x + 1.0) * 0.5, (1.0 - ndc.y) * 0.5) * self.size)
    }
    /// The ray through screen point `s`: its start on the near plane and its unit direction.
    pub fn ray(&self, s: DVec2) -> (DVec3, DVec3) {
        let n = (s - self.min) / self.size;
        let (x, y) = (n.x * 2.0 - 1.0, 1.0 - n.y * 2.0);
        let at = |depth: f64| {
            let p: DVec4 = self.inverse * DVec4::new(x, y, depth, 1.0);
            p.truncate() / p.w
        };
        let (near, far) = (at(0.0), at(1.0));
        (near, (far - near).normalize_or(self.forward))
    }
    /// How many metres one screen point covers at the depth of `at`.
    pub fn metres_per_point(&self, at: DVec3) -> f64 {
        let depth = (at - self.eye).dot(self.forward).max(1e-3);
        2.0 * depth * (self.fov_y / 2.0).tan() / self.size.y
    }
}

/// Where the lines p + s·u and q + t·v (unit directions) come closest: (s, t). None when they
/// are nearly parallel.
fn closest_points(p: DVec3, u: DVec3, q: DVec3, v: DVec3) -> Option<(f64, f64)> {
    let w = p - q;
    let b = u.dot(v);
    let denom = 1.0 - b * b;
    if denom < 1e-6 {
        return None;
    }
    let (d, e) = (u.dot(w), v.dot(w));
    let t = (e - b * d) / denom;
    Some((b * t - d, t))
}

/// The on-screen unit direction of `axis` at `centre`, and how many screen points one metre
/// along it covers there.
fn screen_axis(cam: &ScreenCamera, centre: DVec3, axis: DVec3) -> Option<(DVec2, f64)> {
    let a = cam.project(centre)?;
    let b = cam.project(centre + axis * 0.01)?;
    let d = b - a;
    let len = d.length();
    (len > 1e-6).then(|| (d / len, len / 0.01))
}

/// An axis points (nearly) at the viewer when an arrow along it, [`ARROW_PT`] long if it were
/// side-on, shows shorter than this many screen points. Such an arrow is neither drawn nor
/// grabbed, and an axis like that cannot be dragged along: the pointer would move the piece
/// metres for every point.
pub const END_ON_PT: f64 = 0.15 * ARROW_PT;

/// Whether `axis` through `centre` points (nearly) at the viewer: see [`END_ON_PT`]. The one rule
/// for showing an arrow ([`Gizmo::arrow_shown`]) and for dragging along it ([`axis_drag`]).
fn is_end_on(cam: &ScreenCamera, centre: DVec3, axis: DVec3) -> bool {
    let tip = centre + axis * (ARROW_PT * cam.metres_per_point(centre));
    match (cam.project(centre), cam.project(tip)) {
        (Some(a), Some(b)) => a.distance(b) < END_ON_PT,
        _ => true,
    }
}

/// Metres moved along unit `axis` (through `centre`) when the pointer goes `from` → `to`. The
/// pointer's movement along the axis's on-screen direction is taken back onto the 3D axis
/// exactly (the pointer's ray against the axis line), so the piece stays under the pointer
/// however the axis is foreshortened. None when the axis points at the viewer, and when either
/// ray meets the axis behind the camera (the pointer is past the axis's vanishing point). Near
/// the vanishing point the exact answer runs off to infinity, so the move is held to
/// [`MAX_PLACEMENT_M`], the farthest a piece may be placed.
pub fn axis_drag(
    cam: &ScreenCamera,
    centre: DVec3,
    axis: DVec3,
    from: DVec2,
    to: DVec2,
) -> Option<f64> {
    if is_end_on(cam, centre, axis) {
        return None;
    }
    let (dir, _) = screen_axis(cam, centre, axis)?;
    let origin = cam.project(centre)?;
    let along = |s: DVec2| {
        let on_axis = origin + dir * (s - origin).dot(dir);
        let (o, r) = cam.ray(on_axis);
        let (ahead, t) = closest_points(o, r, centre, axis)?;
        (ahead > 0.0).then_some(t)
    };
    let moved = along(to)? - along(from)?;
    moved
        .is_finite()
        .then(|| moved.clamp(-MAX_PLACEMENT_M, MAX_PLACEMENT_M))
}

/// The move (m) in the plane facing the viewer through `centre` when the pointer goes
/// `from` → `to`: the centre square's drag.
pub fn plane_drag(cam: &ScreenCamera, centre: DVec3, from: DVec2, to: DVec2) -> Option<DVec3> {
    let normal = cam.forward;
    let hit = |s: DVec2| {
        let (o, r) = cam.ray(s);
        let denom = r.dot(normal);
        (denom.abs() > 1e-9).then(|| o + r * ((centre - o).dot(normal) / denom))
    };
    Some(hit(to)? - hit(from)?)
}

/// Below this |cos| between the view direction and a ring's axis, the ring is seen edge-on.
pub const EDGE_ON: f64 = 0.15;

/// Radians turned about unit `axis` through `centre` when the pointer goes `from` → `to`, in
/// (−π, π]. A drag that goes round further than half a turn has to be added up from the angles
/// of its successive moves, each from the last pointer position to the next (as
/// `Arranger::drag_to` does), not measured from where it began.
/// Seen at an angle, the pointer's rays meet the ring's plane and the angle between the two
/// hits is exact. Seen nearly edge-on (decided from the view direction at the centre, so the
/// method never changes during a drag), the angle the pointer turns round the centre on screen
/// is used. None when a ray misses the plane: keep the last angle.
pub fn ring_angle(
    cam: &ScreenCamera,
    centre: DVec3,
    axis: DVec3,
    from: DVec2,
    to: DVec2,
) -> Option<f64> {
    let view = (centre - cam.eye).normalize_or(cam.forward);
    if view.dot(axis).abs() >= EDGE_ON {
        let hit = |s: DVec2| {
            let (o, r) = cam.ray(s);
            let denom = r.dot(axis);
            if denom.abs() < 1e-9 {
                return None;
            }
            let t = (centre - o).dot(axis) / denom;
            (t > 0.0)
                .then(|| o + r * t - centre)
                .filter(|v| v.length() > 1e-9)
        };
        let (a, b) = (hit(from)?, hit(to)?);
        return Some(a.cross(b).dot(axis).atan2(a.dot(b)));
    }
    let c = cam.project(centre)?;
    let (a, b) = (from - c, to - c);
    if a.length() < 1e-9 || b.length() < 1e-9 {
        return None;
    }
    // Screen y points down, so negate to count anticlockwise as positive; a ring whose axis
    // points away from the viewer turns the other way.
    let screen = -(a.perp_dot(b)).atan2(a.dot(b));
    Some(if axis.dot(cam.eye - centre) >= 0.0 {
        screen
    } else {
        -screen
    })
}

/// `radians` to the nearest multiple of `step_deg` degrees.
pub fn snap_angle(radians: f64, step_deg: f64) -> f64 {
    let step = step_deg.to_radians();
    (radians / step).round() * step
}

/// Möller–Trumbore: how far along unit `dir` from `origin` the ray meets triangle `t`.
pub fn ray_triangle(origin: DVec3, dir: DVec3, t: [DVec3; 3]) -> Option<f64> {
    let (e1, e2) = (t[1] - t[0], t[2] - t[0]);
    let p = dir.cross(e2);
    let det = e1.dot(p);
    if det.abs() < 1e-12 {
        return None;
    }
    let inv = 1.0 / det;
    let s = origin - t[0];
    let u = s.dot(p) * inv;
    if !(0.0..=1.0).contains(&u) {
        return None;
    }
    let q = s.cross(e1);
    let v = dir.dot(q) * inv;
    if v < 0.0 || u + v > 1.0 {
        return None;
    }
    let d = e2.dot(q) * inv;
    (d > 1e-9).then_some(d)
}

/// The world's axes, x (red), y (green), z (blue).
pub const AXES: [DVec3; 3] = [DVec3::X, DVec3::Y, DVec3::Z];
/// Arrow length on screen (points).
pub const ARROW_PT: f64 = 80.0;
/// Ring radius on screen (points).
pub const RING_PT: f64 = 60.0;
/// Half the centre square's side (points).
pub const SQUARE_PT: f64 = 9.0;
/// How near (points) the pointer must be to grab an arrow or a ring.
pub const GRAB_PT: f64 = 8.0;
/// Points on a drawn ring.
pub const RING_STEPS: usize = 48;

/// A part of the gizmo.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Handle {
    /// The arrow along axis 0 (x), 1 (y) or 2 (z).
    Move(usize),
    /// The centre square: moves in the plane facing the viewer.
    Plane,
    /// The ring about axis 0, 1 or 2.
    Turn(usize),
}

/// The gizmo round a piece's centre, sized to look the same however far away it is.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Gizmo {
    pub centre: DVec3,
    /// World length (m) of an arrow.
    pub size: f64,
}

impl Gizmo {
    pub fn new(cam: &ScreenCamera, centre: DVec3) -> Self {
        Self {
            centre,
            size: ARROW_PT * cam.metres_per_point(centre),
        }
    }
    pub fn arrow_tip(&self, axis: usize) -> DVec3 {
        self.centre + AXES[axis] * self.size
    }
    /// An arrow is shown (and can be grabbed) unless it points nearly at the viewer (the same
    /// rule that stops [`axis_drag`]).
    pub fn arrow_shown(&self, cam: &ScreenCamera, axis: usize) -> bool {
        !is_end_on(cam, self.centre, AXES[axis])
    }
    /// The ring about `axis`, as [`RING_STEPS`] points.
    pub fn ring(&self, axis: usize) -> Vec<DVec3> {
        let (u, v) = AXES[axis].any_orthonormal_pair();
        let r = self.size * RING_PT / ARROW_PT;
        (0..RING_STEPS)
            .map(|k| {
                let a = k as f64 / RING_STEPS as f64 * std::f64::consts::TAU;
                self.centre + (u * a.cos() + v * a.sin()) * r
            })
            .collect()
    }
    /// The handle under screen point `pos`: the centre square first, then the nearest arrow or
    /// ring within [`GRAB_PT`] (an arrow wins a tie).
    pub fn hit(&self, cam: &ScreenCamera, pos: DVec2) -> Option<Handle> {
        let c = cam.project(self.centre)?;
        let d = (pos - c).abs();
        if d.x <= SQUARE_PT && d.y <= SQUARE_PT {
            return Some(Handle::Plane);
        }
        let mut best: Option<(Handle, f64)> = None;
        let mut consider = |handle: Handle, distance: f64| {
            if distance <= GRAB_PT && best.is_none_or(|(_, b)| distance < b) {
                best = Some((handle, distance));
            }
        };
        for axis in 0..3 {
            if self.arrow_shown(cam, axis)
                && let Some(tip) = cam.project(self.arrow_tip(axis))
            {
                consider(Handle::Move(axis), segment_distance(pos, c, tip));
            }
        }
        for axis in 0..3 {
            let ring: Vec<DVec2> = self
                .ring(axis)
                .into_iter()
                .filter_map(|p| cam.project(p))
                .collect();
            let nearest = (0..ring.len())
                .map(|k| segment_distance(pos, ring[k], ring[(k + 1) % ring.len()]))
                .fold(f64::INFINITY, f64::min);
            consider(Handle::Turn(axis), nearest);
        }
        best.map(|(h, _)| h)
    }
}

/// Distance from `p` to the segment `a`–`b` (screen points).
pub fn segment_distance(p: DVec2, a: DVec2, b: DVec2) -> f64 {
    let ab = b - a;
    let len2 = ab.length_squared();
    let t = if len2 < 1e-18 {
        0.0
    } else {
        ((p - a).dot(ab) / len2).clamp(0.0, 1.0)
    };
    p.distance(a + ab * t)
}

#[cfg(test)]
mod tests {
    use super::*;
    use glam::Vec3;

    /// 144 cameras: all round, from below and above, near and far.
    fn cameras() -> Vec<OrbitCamera> {
        let mut out = vec![];
        for yaw in [-2.8_f32, -1.6, -0.7, 0.0, 0.5, 1.2, 2.2, 3.1] {
            for pitch in [-1.2_f32, -0.5, 0.0, 0.3, 0.9, 1.4] {
                for distance in [1.0_f32, 2.6, 6.0] {
                    out.push(OrbitCamera {
                        target: Vec3::new(0.0, 0.95, 0.02),
                        yaw,
                        pitch,
                        distance,
                        fov_y: 35f32.to_radians(),
                    });
                }
            }
        }
        out
    }

    #[test]
    fn arrow_drags_move_the_dragged_distance_from_every_side() {
        let centre = DVec3::new(0.1, 1.0, 0.35);
        let (mut checked, mut skipped) = (0, 0);
        for c in cameras() {
            let cam = ScreenCamera::new(&c, DVec2::new(10.0, 40.0), DVec2::new(600.0, 520.0));
            for axis in AXES {
                for d in [0.02, 0.1, -0.25] {
                    let (Some(a), Some(b)) = (cam.project(centre), cam.project(centre + axis * d))
                    else {
                        continue;
                    };
                    let Some((dir, _)) = screen_axis(&cam, centre, axis) else {
                        continue;
                    };
                    // Wobbling 7 points sideways off the arrow changes nothing.
                    match axis_drag(&cam, centre, axis, a, b + dir.perp() * 7.0) {
                        Some(got) => {
                            checked += 1;
                            assert!((got - d).abs() < 1e-6 * d.abs().max(1.0), "{got} vs {d}");
                        }
                        None => skipped += 1,
                    }
                }
            }
        }
        assert!(
            checked > 1200 && skipped < 60,
            "{checked} checked, {skipped} skipped"
        );
    }

    #[test]
    fn an_arrow_pointing_at_the_viewer_cannot_be_dragged() {
        let c = OrbitCamera {
            target: Vec3::new(0.0, 1.0, 0.0),
            yaw: 0.0,
            pitch: 0.0,
            distance: 2.0,
            fov_y: 0.6,
        };
        let cam = ScreenCamera::new(&c, DVec2::ZERO, DVec2::new(800.0, 600.0));
        let centre = DVec3::new(0.0, 1.0, 0.0);
        let s = cam.project(centre).unwrap();
        assert_eq!(
            axis_drag(&cam, centre, DVec3::Z, s, s + DVec2::new(40.0, 0.0)),
            None
        );
        let gizmo = Gizmo::new(&cam, centre);
        assert!(!gizmo.arrow_shown(&cam, 2) && gizmo.arrow_shown(&cam, 0));
    }

    /// A camera whose line of sight is 10° off the x axis, and the view it draws into.
    fn nearly_along_x() -> (ScreenCamera, DVec3) {
        let c = OrbitCamera {
            target: Vec3::new(0.0, 1.0, 0.0),
            yaw: 80f32.to_radians(),
            pitch: 0.0,
            distance: 2.6,
            fov_y: 35f32.to_radians(),
        };
        let cam = ScreenCamera::new(&c, DVec2::new(10.0, 40.0), DVec2::new(600.0, 520.0));
        (cam, DVec3::new(0.0, 1.0, 0.0))
    }

    #[test]
    fn dragging_towards_an_axis_vanishing_point_stays_finite_bounded_and_monotone() {
        let (cam, centre) = nearly_along_x();
        assert!(Gizmo::new(&cam, centre).arrow_shown(&cam, 0), "grabbable");
        let origin = cam.project(centre).unwrap();
        // The x axis points at the eye here: far along -x it runs away to its vanishing point.
        let vanishing = cam.project(centre - DVec3::X * 1e6).unwrap();
        let towards = (vanishing - origin).length();
        assert!((100.0..300.0).contains(&towards), "{towards} points away");
        for sign in [1.0, -1.0] {
            // The pointer goes from the centre along the axis's screen direction, through the
            // vanishing point and out the other side.
            let mut results = Vec::new();
            for step in 0..=240 {
                let lambda = f64::from(step) / 200.0 * sign;
                let to = origin + (vanishing - origin) * lambda;
                results.push((lambda, axis_drag(&cam, centre, DVec3::X, origin, to)));
            }
            let moves: Vec<f64> = results.iter().filter_map(|(_, m)| *m).collect();
            for m in &moves {
                assert!(m.is_finite() && m.abs() <= MAX_PLACEMENT_M, "{m}");
            }
            // Monotone: the move only ever grows in the direction it started.
            let direction = moves[moves.len() / 2].signum();
            for pair in moves.windows(2) {
                assert!(
                    (pair[1] - pair[0]) * direction >= -1e-9,
                    "{pair:?} (pointer side {sign})"
                );
            }
            // Once the ray meets the axis behind the camera there is no move at all: no jump
            // to the other sign.
            let first_none = results.iter().position(|(_, m)| m.is_none());
            if let Some(i) = first_none {
                assert!(results[i..].iter().all(|(_, m)| m.is_none()), "{results:?}");
            }
            if sign > 0.0 {
                // Towards the vanishing point the move runs up to the 10 m limit, and the
                // pointer past it moves nothing.
                let (lambda, _) = results[first_none.expect("past the vanishing point")];
                assert!((0.95..=1.2).contains(&lambda), "{lambda}");
                assert!((moves.last().unwrap().abs() - MAX_PLACEMENT_M).abs() < 1e-9);
                assert!(
                    results[..first_none.unwrap()]
                        .iter()
                        .all(|(_, m)| m.is_some())
                );
            }
        }
    }

    #[test]
    fn an_arrow_is_grabbable_exactly_when_it_can_be_dragged() {
        // One rule for both, however far the camera is: before, an arrow could be shown (12
        // points long) that the drag refused (under 20 points per metre) from 8 m out.
        let centre = DVec3::new(0.0, 1.0, 0.0);
        let (mut shown, mut hidden) = (0, 0);
        for distance in [1.0_f32, 2.6, 6.0, 8.0, 12.0, 20.0] {
            for yaw in 0..24 {
                for pitch in [-1.3_f32, -0.6, 0.0, 0.4, 1.2] {
                    let c = OrbitCamera {
                        target: Vec3::new(0.0, 1.0, 0.0),
                        yaw: yaw as f32 * 0.2618,
                        pitch,
                        distance,
                        fov_y: 35f32.to_radians(),
                    };
                    let cam =
                        ScreenCamera::new(&c, DVec2::new(10.0, 40.0), DVec2::new(600.0, 520.0));
                    let gizmo = Gizmo::new(&cam, centre);
                    let origin = cam.project(centre).unwrap();
                    for (axis, direction) in AXES.iter().enumerate() {
                        let tip = cam.project(gizmo.arrow_tip(axis)).unwrap();
                        let along = (tip - origin).normalize_or(DVec2::X);
                        let can_drag =
                            axis_drag(&cam, centre, *direction, origin, origin + along * 5.0)
                                .is_some();
                        assert_eq!(
                            gizmo.arrow_shown(&cam, axis),
                            can_drag,
                            "axis {axis}, {distance} m, yaw {yaw}, pitch {pitch}"
                        );
                        if can_drag {
                            shown += 1;
                        } else {
                            hidden += 1;
                        }
                    }
                }
            }
        }
        assert!(shown > 300 && hidden > 10, "{shown} shown, {hidden} hidden");
    }

    #[test]
    fn ring_drags_turn_the_dragged_angle_from_every_side() {
        let centre = DVec3::new(-0.05, 0.9, 0.3);
        let (mut checked, mut edge_on) = (0, 0);
        for c in cameras() {
            let cam = ScreenCamera::new(&c, DVec2::ZERO, DVec2::new(800.0, 600.0));
            for axis in AXES {
                if (centre - cam.eye()).normalize().dot(axis).abs() < EDGE_ON {
                    edge_on += 1;
                    continue;
                }
                let (u, v) = axis.any_orthonormal_pair();
                for (a0, a1) in [(0.3_f64, 1.1_f64), (2.0, 1.2), (-0.4, 0.9)] {
                    let on_ring = |a: f64| centre + (u * a.cos() + v * a.sin()) * 0.12;
                    let (Some(s0), Some(s1)) = (cam.project(on_ring(a0)), cam.project(on_ring(a1)))
                    else {
                        continue;
                    };
                    if let Some(got) = ring_angle(&cam, centre, axis, s0, s1) {
                        checked += 1;
                        assert!((got - (a1 - a0)).abs() < 1e-6, "{got} vs {}", a1 - a0);
                    }
                }
            }
        }
        assert!(
            checked > 900 && edge_on > 0,
            "{checked} checked, {edge_on} edge-on"
        );
    }

    #[test]
    fn an_edge_on_ring_turns_with_the_pointer_round_the_centre() {
        // Seen from the front, the z ring faces the viewer and the y ring is edge-on.
        let c = OrbitCamera {
            target: Vec3::new(0.0, 1.0, 0.0),
            yaw: 0.0,
            pitch: 0.0,
            distance: 2.0,
            fov_y: 0.6,
        };
        let cam = ScreenCamera::new(&c, DVec2::ZERO, DVec2::new(800.0, 600.0));
        let centre = DVec3::new(0.0, 1.0, 0.0);
        let s = cam.project(centre).unwrap();
        let (right, up) = (s + DVec2::new(50.0, 0.0), s + DVec2::new(0.0, -50.0));
        // Right to up is a quarter turn anticlockwise as the viewer sees it: +90° about +z.
        let z = ring_angle(&cam, centre, DVec3::Z, right, up).unwrap();
        assert!((z - std::f64::consts::FRAC_PI_2).abs() < 1e-6, "{z}");
        let y = ring_angle(&cam, centre, DVec3::Y, right, up).unwrap();
        assert!((y.abs() - std::f64::consts::FRAC_PI_2).abs() < 1e-6, "{y}");
        assert!((snap_angle(0.3, 15.0) - 15f64.to_radians()).abs() < 1e-12);
    }

    /// A view of (0, 1, 0) from the direction `eye` (unit), and the point looked at.
    fn looking_from(eye: DVec3) -> (ScreenCamera, DVec3) {
        let c = OrbitCamera {
            target: Vec3::new(0.0, 1.0, 0.0),
            yaw: eye.x.atan2(eye.z) as f32,
            pitch: eye.y.asin() as f32,
            distance: 2.6,
            fov_y: 35f32.to_radians(),
        };
        let cam = ScreenCamera::new(&c, DVec2::new(0.0, 30.0), DVec2::new(700.0, 600.0));
        (cam, DVec3::new(0.0, 1.0, 0.0))
    }

    #[test]
    fn an_edge_on_ring_turns_the_way_the_exact_method_does_just_outside_the_limit() {
        // Anticlockwise on screen, a quarter turn about the centre, 50 points out. Seen from
        // the axis's positive side (the axis towards the viewer) that is a positive turn about
        // it, and from the other side a negative one: for every axis, from above and below,
        // 0.14 (read from the pointer's angle on screen) and 0.16 (read exactly from the ring's
        // plane) from edge-on must agree on the sign, and the edge-on one is a true quarter.
        let mut cases = 0;
        for (k, axis) in AXES.iter().enumerate() {
            for towards in [1.0, -1.0] {
                let mut signs = Vec::new();
                for cos in [0.14, 0.16] {
                    let side = [DVec3::Z, DVec3::Z, DVec3::X][k];
                    let eye =
                        (*axis * towards * cos + side * (1.0_f64 - cos * cos).sqrt()).normalize();
                    let (cam, centre) = looking_from(eye);
                    let c = cam.project(centre).unwrap();
                    let turned = ring_angle(
                        &cam,
                        centre,
                        *axis,
                        c + DVec2::new(50.0, 0.0),
                        c + DVec2::new(0.0, -50.0),
                    )
                    .unwrap_or_else(|| panic!("axis {k}, {towards}, {cos}"));
                    assert!(turned.abs() < std::f64::consts::PI, "{turned}");
                    assert_eq!(
                        turned.signum(),
                        towards,
                        "axis {k} {} the viewer, {cos} from edge-on: {turned}",
                        if towards > 0.0 {
                            "towards"
                        } else {
                            "away from"
                        }
                    );
                    if cos < EDGE_ON {
                        assert!(
                            (turned.abs() - std::f64::consts::FRAC_PI_2).abs() < 1e-6,
                            "{turned}"
                        );
                    }
                    signs.push(turned.signum());
                    cases += 1;
                }
                assert_eq!(signs[0], signs[1]);
            }
        }
        assert_eq!(cases, 12);
    }

    #[test]
    fn the_centre_square_moves_in_the_plane_facing_the_viewer() {
        let c = OrbitCamera {
            target: Vec3::new(0.0, 1.0, 0.0),
            yaw: 0.7,
            pitch: 0.3,
            distance: 2.5,
            fov_y: 0.6,
        };
        let cam = ScreenCamera::new(&c, DVec2::ZERO, DVec2::new(800.0, 600.0));
        let centre = DVec3::new(0.1, 1.1, 0.2);
        let target = centre + DVec3::new(0.05, -0.08, 0.0);
        let (from, to) = (cam.project(centre).unwrap(), cam.project(target).unwrap());
        let moved = plane_drag(&cam, centre, from, to).unwrap();
        // The new point is under the pointer, and the move is square to the view.
        let back = cam.project(centre + moved).unwrap();
        assert!(back.distance(to) < 1e-6, "{back} vs {to}");
        assert!(moved.dot(cam.forward).abs() < 1e-9);
    }

    #[test]
    fn a_point_projects_where_its_ray_comes_from() {
        let c = OrbitCamera::default();
        let cam = ScreenCamera::new(&c, DVec2::new(5.0, 5.0), DVec2::new(640.0, 480.0));
        let p = DVec3::new(0.2, 0.4, -0.3);
        let (o, d) = cam.ray(cam.project(p).unwrap());
        assert!((o + d * (p - o).length() - p).length() < 1e-6);
        // At the middle of the view, 100 points across is 100 × metres_per_point.
        let target = c.target.as_dvec3();
        let right = cam.forward.cross(DVec3::Y).normalize();
        let side = target + right * 100.0 * cam.metres_per_point(target);
        let gap = cam
            .project(side)
            .unwrap()
            .distance(cam.project(target).unwrap());
        assert!((gap - 100.0).abs() < 0.5, "{gap}");
    }

    #[test]
    fn a_ray_finds_the_triangle_in_front_of_it() {
        let tri = |z: f64| {
            [
                DVec3::new(-1.0, -1.0, z),
                DVec3::new(1.0, -1.0, z),
                DVec3::new(0.0, 1.0, z),
            ]
        };
        let o = DVec3::new(0.0, 0.0, 5.0);
        assert_eq!(ray_triangle(o, DVec3::NEG_Z, tri(0.0)), Some(5.0));
        assert_eq!(ray_triangle(o, DVec3::NEG_Z, tri(6.0)), None, "behind");
        assert_eq!(
            ray_triangle(DVec3::new(3.0, 0.0, 5.0), DVec3::NEG_Z, tri(0.0)),
            None
        );
    }

    #[test]
    fn every_handle_is_grabbed_where_it_is_drawn() {
        let c = OrbitCamera {
            target: Vec3::new(0.0, 1.0, 0.0),
            yaw: 0.6,
            pitch: 0.35,
            distance: 2.6,
            fov_y: 35f32.to_radians(),
        };
        let cam = ScreenCamera::new(&c, DVec2::new(0.0, 30.0), DVec2::new(700.0, 600.0));
        let gizmo = Gizmo::new(&cam, DVec3::new(0.0, 1.0, 0.3));
        let c2 = cam.project(gizmo.centre).unwrap();
        assert_eq!(
            gizmo.hit(&cam, c2 + DVec2::new(3.0, -2.0)),
            Some(Handle::Plane)
        );
        for axis in 0..3 {
            let tip = cam.project(gizmo.arrow_tip(axis)).unwrap();
            assert_eq!(
                gizmo.hit(&cam, tip),
                Some(Handle::Move(axis)),
                "arrow {axis}"
            );
        }
        for axis in 0..3 {
            let grabbed = gizmo
                .ring(axis)
                .into_iter()
                .filter_map(|p| cam.project(p))
                .filter(|s| gizmo.hit(&cam, *s) == Some(Handle::Turn(axis)))
                .count();
            // Most of the ring: only where an arrow or another ring is nearer is it not.
            assert!(grabbed > RING_STEPS / 3, "ring {axis}: {grabbed}");
        }
        assert_eq!(gizmo.hit(&cam, c2 + DVec2::new(200.0, 200.0)), None);
    }
}
