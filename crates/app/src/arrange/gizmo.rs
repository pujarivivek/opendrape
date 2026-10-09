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

/// Below this |cos| between the view direction and a ring's axis, the ring is seen at a
/// grazing angle (more than 60° from face-on, a flat sliver of an ellipse): see [`ring_angle`].
pub const GRAZING: f64 = 0.5;

/// The unit direction from the eye to `centre`.
fn view_towards(cam: &ScreenCamera, centre: DVec3) -> DVec3 {
    (centre - cam.eye).normalize_or(cam.forward)
}

/// Whether the ring about `axis` through `centre` is seen at a grazing angle: see [`GRAZING`].
/// The one test for turning it by [`ring_angle`]'s grazing way, and for what [`grab_side`] and
/// [`held_ring_angle`] do.
fn is_grazing(cam: &ScreenCamera, centre: DVec3, axis: DVec3) -> bool {
    view_towards(cam, centre).dot(axis).abs() < GRAZING
}

/// The point of the ring about `axis` nearest the eye, as a unit direction from the centre, for
/// `view` the unit direction from the eye to the centre. None when the axis points at (or from)
/// the eye, which a ring seen at a grazing angle never does.
fn nearest_side(view: DVec3, axis: DVec3) -> Option<DVec3> {
    (-view - axis * (-view).dot(axis)).try_normalize()
}

/// Radians turned about unit `axis` through `centre` when the pointer goes `from` → `to`. A drag
/// that goes round further than half a turn has to be added up from the angles of its successive
/// moves, each from the last pointer position to the next (as `Arranger::drag_to` does), not
/// measured from where it began.
///
/// Seen nearly face-on (|cos| between the view direction and the axis at least [`GRAZING`]), the
/// pointer's rays meet the ring's plane and the angle between the two hits is exact, in
/// (−π, π]. None when a ray misses the plane: keep the last angle.
///
/// Seen at a grazing angle the ring is a thin ellipse, and any angle measured round its centre
/// races, or flips by half a turn when the pointer crosses it. There the ring turns with the
/// pointer's movement along the screen direction the nearest side of the ring travels in, which
/// is across the ring's projected axis, divided by the ring's on-screen radius: a steady rate
/// in degrees per point. Movement along the axis, or anywhere else, turns it by nothing. Both
/// ways agree about which way is positive for a pointer holding the nearest side (the way that
/// side goes when the ring is turned right-handedly about `axis`). A pointer holding the far
/// side follows that side instead, which goes the other way: see [`held_ring_angle`]. The way is
/// decided from the view direction at the centre, so it never changes during a drag.
pub fn ring_angle(
    cam: &ScreenCamera,
    centre: DVec3,
    axis: DVec3,
    from: DVec2,
    to: DVec2,
) -> Option<f64> {
    if is_grazing(cam, centre, axis) {
        return grazing_angle(cam, centre, axis, from, to);
    }
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
    Some(a.cross(b).dot(axis).atan2(a.dot(b)))
}

/// [`ring_angle`] for a ring seen at a grazing angle (for the nearest side of the ring).
fn grazing_angle(
    cam: &ScreenCamera,
    centre: DVec3,
    axis: DVec3,
    from: DVec2,
    to: DVec2,
) -> Option<f64> {
    // The point of the ring nearest the eye, and the way it goes when the ring is turned
    // right-handedly about `axis`. (`near` is not short: the axis is at least 60° from the view direction.)
    let near = nearest_side(view_towards(cam, centre), axis)?;
    let heading = axis.cross(near);
    // That way, and the ring's radius, on screen.
    let origin = cam.project(centre)?;
    let radius = RING_PT * cam.metres_per_point(centre);
    let along = cam.project(centre + heading * radius)? - origin;
    let radius_pt = along.length();
    (radius_pt > 1e-6).then(|| (to - from).dot(along) / (radius_pt * radius_pt))
}

/// [`ring_angle`] for a pointer that took hold of the ring at a point [`grab_side`] gave `side`
/// for: the turn that follows the point held. Only a ring seen at a grazing angle has a side to
/// take; any other is read exactly, the half held and all.
pub fn held_ring_angle(
    cam: &ScreenCamera,
    centre: DVec3,
    axis: DVec3,
    side: f64,
    from: DVec2,
    to: DVec2,
) -> Option<f64> {
    let angle = ring_angle(cam, centre, axis, from, to)?;
    Some(if is_grazing(cam, centre, axis) {
        angle * side
    } else {
        angle
    })
}

/// How far (as a share of the ring's on-screen half-height) a grab must be beyond the ring's
/// middle line, towards the far side, to count as holding the far side: nearer the middle line
/// is the ring's tips, where neither half is the one held.
const FAR_SIDE: f64 = 0.3;

/// Which way [`ring_angle`] reads a ring seen at a grazing angle for a pointer that took hold of
/// it at `grab`: 1 for the half of the ring's ellipse nearer the eye (the way `ring_angle` goes),
/// −1 for the half farther, whose points travel the other way across the screen when the ring
/// turns, so that the turn follows the point held. Decided once, when the drag begins, and kept:
/// the pointer then crosses the ring as it likes without the turn ever changing sign.
///
/// It is also 1, which keeps the nearest-side rule, where the half is not clear: near the
/// ellipse's tips (`FAR_SIDE`), and for a ring so thin that its two halves lie within
/// [`GRAB_PT`] of each other. And it is 1 for a ring that is not seen at a grazing angle, which
/// [`ring_angle`] reads exactly, the half held and all.
pub fn grab_side(cam: &ScreenCamera, centre: DVec3, axis: DVec3, grab: DVec2) -> f64 {
    if !is_grazing(cam, centre, axis) {
        return 1.0;
    }
    let Some(near) = nearest_side(view_towards(cam, centre), axis) else {
        return 1.0;
    };
    let radius = RING_PT * cam.metres_per_point(centre);
    let (Some(origin), Some(nearest)) = (cam.project(centre), cam.project(centre + near * radius))
    else {
        return 1.0;
    };
    // The nearest point of the ring shows `half_height` beyond the centre of the ellipse, the
    // far one the same before it, along the ring's projected axis.
    let towards_near = nearest - origin;
    let half_height = towards_near.length();
    if half_height < GRAB_PT / 2.0 {
        return 1.0;
    }
    let beyond_middle = (grab - origin).dot(towards_near / half_height);
    if beyond_middle < -FAR_SIDE * half_height {
        -1.0
    } else {
        1.0
    }
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
        let (mut checked, mut grazing) = (0, 0);
        for c in cameras() {
            let cam = ScreenCamera::new(&c, DVec2::ZERO, DVec2::new(800.0, 600.0));
            for axis in AXES {
                if (centre - cam.eye()).normalize().dot(axis).abs() < GRAZING {
                    grazing += 1;
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
            checked > 500 && grazing > 200,
            "{checked} checked, {grazing} rings seen at a grazing angle"
        );
    }

    /// The point of the ring about `axis` that is `phi` radians round from the point nearest the
    /// eye (on a circle, the point towards the eye), on the way that point goes when the ring is
    /// turned right-handedly, and the screen direction (unit) the point travels in then: worked
    /// out from the ring's geometry, not from anything `ring_angle` does. `phi` of 0 is the
    /// nearest point, π the farthest, ±π/2 the tips of the ring's ellipse.
    fn ring_point_and_heading(
        cam: &ScreenCamera,
        centre: DVec3,
        axis: usize,
        phi: f64,
    ) -> (DVec2, DVec2) {
        let radius = Gizmo::new(cam, centre).size * RING_PT / ARROW_PT;
        let to_eye = cam.eye() - centre;
        let near = (to_eye - AXES[axis] * to_eye.dot(AXES[axis])).normalize();
        let p = centre + (near * phi.cos() + AXES[axis].cross(near) * phi.sin()) * radius;
        let velocity = AXES[axis].cross(p - centre).normalize();
        let at = cam.project(p).unwrap();
        let heading = (cam.project(p + velocity * 0.001).unwrap() - at).normalize_or_zero();
        (at, heading)
    }

    fn nearest_point_and_heading(cam: &ScreenCamera, centre: DVec3, axis: usize) -> (DVec2, DVec2) {
        ring_point_and_heading(cam, centre, axis, 0.0)
    }

    /// Half the height of the ring's ellipse on screen: how far the nearest point of the ring
    /// shows from the centre.
    fn half_height(cam: &ScreenCamera, centre: DVec3, axis: usize) -> f64 {
        let (nearest, _) = nearest_point_and_heading(cam, centre, axis);
        nearest.distance(cam.project(centre).unwrap())
    }

    #[test]
    fn a_ring_seen_edge_on_turns_with_the_pointer_across_its_axis() {
        // Seen from the front, the y ring (axis up) and the x ring (axis across) are edge-on,
        // and the z ring faces the viewer.
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
        let turn = |axis: DVec3, from: DVec2, by: DVec2| {
            ring_angle(&cam, centre, axis, from, from + by).unwrap()
        };
        // 30 points across a ring of 60 points radius is half a radian, wherever the pointer
        // is: at the middle of the ring, or beside it. The nearest side of the y ring goes right
        // when the ring turns right-handedly about +y.
        for from in [s, s + DVec2::new(40.0, 8.0), s + DVec2::new(-55.0, -3.0)] {
            assert!((turn(DVec3::Y, from, DVec2::new(30.0, 0.0)) - 0.5).abs() < 1e-9);
            assert!((turn(DVec3::Y, from, DVec2::new(-30.0, 0.0)) + 0.5).abs() < 1e-9);
            // Along the axis, the pointer turns nothing.
            assert!(turn(DVec3::Y, from, DVec2::new(0.0, -40.0)).abs() < 1e-9);
            // The x ring's nearest side goes down on screen for a positive turn about +x.
            assert!((turn(DVec3::X, from, DVec2::new(0.0, 30.0)) - 0.5).abs() < 1e-9);
            assert!(turn(DVec3::X, from, DVec2::new(30.0, 0.0)).abs() < 1e-9);
        }
        // The z ring faces the viewer: right to up is a quarter turn anticlockwise as the
        // viewer sees it, +90° about +z.
        let (right, up) = (s + DVec2::new(50.0, 0.0), s + DVec2::new(0.0, -50.0));
        let z = ring_angle(&cam, centre, DVec3::Z, right, up).unwrap();
        assert!((z - std::f64::consts::FRAC_PI_2).abs() < 1e-6, "{z}");
        assert!((snap_angle(0.3, 15.0) - 15f64.to_radians()).abs() < 1e-12);
    }

    #[test]
    fn a_grazing_ring_turns_at_one_steady_rate_wherever_the_pointer_is() {
        // The old rule measured the angle round the centre on screen, which races near the
        // centre and flips by half a turn across it. This one cannot: the same two points of
        // pointer movement turn the ring by the same angle at every place, the centre included.
        let centre = DVec3::new(-0.05, 0.9, 0.3);
        let (mut cases, mut places) = (0, 0);
        for c in cameras() {
            let cam = ScreenCamera::new(&c, DVec2::new(10.0, 40.0), DVec2::new(800.0, 600.0));
            let view = (centre - cam.eye()).normalize();
            for (k, axis) in AXES.into_iter().enumerate() {
                if view.dot(axis).abs() >= GRAZING {
                    continue;
                }
                let (_, heading) = nearest_point_and_heading(&cam, centre, k);
                let origin = cam.project(centre).unwrap();
                let mut rates = Vec::new();
                for dx in -4..=4 {
                    for dy in -3..=3 {
                        let from = origin + DVec2::new(f64::from(dx), f64::from(dy)) * 30.0;
                        rates.push(
                            ring_angle(&cam, centre, axis, from, from + heading * 2.0).unwrap(),
                        );
                        // Across it, nearly nothing (perspective bends "across" a few degrees).
                        let aside =
                            ring_angle(&cam, centre, axis, from, from + heading.perp() * 2.0)
                                .unwrap();
                        assert!(aside.abs() < 0.2 * rates[0].abs(), "{aside} across");
                        places += 1;
                    }
                }
                // The same everywhere, and about 2 points of a 60 point radius (perspective
                // makes the radius a little more or less).
                for rate in &rates {
                    assert!((rate - rates[0]).abs() < 1e-9, "{rate} vs {}", rates[0]);
                }
                assert!(
                    (rates[0] * RING_PT / 2.0 - 1.0).abs() < 0.08,
                    "{}° for 2 points",
                    rates[0].to_degrees()
                );
                cases += 1;
            }
        }
        assert!(
            cases > 200 && places > 12000,
            "{cases} rings, {places} places"
        );
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
    fn the_two_ways_of_turning_a_ring_agree_at_the_switch() {
        // Take hold of the side of the ring nearest the eye, or the side farthest from it, and
        // pull that point along the way it goes for a right-handed turn: that is a positive
        // turn, on either side of the switch, from above and from below, for every axis. Pulled
        // the other way it is a negative one. The two ways give about the same angle too (the
        // exact one reads a straight pull as the arctangent of its length over the radius, the
        // grazing one as the length over it).
        let mut cases = 0;
        for (k, axis) in AXES.iter().enumerate() {
            for towards in [1.0, -1.0] {
                for (arc, phi) in [("near", 0.0), ("far", std::f64::consts::PI)] {
                    let side = [DVec3::Z, DVec3::Z, DVec3::X][k];
                    let mut angles = Vec::new();
                    for cos in [GRAZING - 0.02, GRAZING + 0.02] {
                        let eye = (*axis * towards * cos + side * (1.0_f64 - cos * cos).sqrt())
                            .normalize();
                        let (cam, centre) = looking_from(eye);
                        let (at, heading) = ring_point_and_heading(&cam, centre, k, phi);
                        let held = grab_side(&cam, centre, *axis, at);
                        for (pull, sign) in [(20.0, 1.0), (-20.0, -1.0)] {
                            let to = at + heading * pull;
                            let turned = held_ring_angle(&cam, centre, *axis, held, at, to)
                                .unwrap_or_else(|| panic!("axis {k}, {towards}, {cos}"));
                            assert_eq!(
                                turned.signum(),
                                sign,
                                "axis {k} {} the viewer, the {arc} arc, {cos}, pulled {pull}: {turned}",
                                if towards > 0.0 {
                                    "towards"
                                } else {
                                    "away from"
                                }
                            );
                            if pull > 0.0 {
                                angles.push(turned);
                            }
                            cases += 1;
                        }
                    }
                    assert!(
                        (angles[0] / angles[1] - 1.0).abs() < 0.2,
                        "axis {k}, {towards}, the {arc} arc: {angles:?} either side of the switch"
                    );
                }
            }
        }
        assert_eq!(cases, 48);
    }

    #[test]
    fn a_grazing_ring_taken_by_its_far_half_turns_with_the_point_held() {
        // The far half of a ring goes the other way across the screen from the near half when
        // the ring turns. Whichever half the pointer took hold of, pulling that point along its
        // own way turns the ring forwards. Round the ring's tips, and for a ring too thin to
        // tell the halves apart, the nearest-side rule stands.
        let centre = DVec3::new(-0.05, 0.9, 0.3);
        let (mut near, mut far, mut tips, mut thin, mut exact) = (0, 0, 0, 0, 0);
        for c in cameras() {
            let cam = ScreenCamera::new(&c, DVec2::new(10.0, 40.0), DVec2::new(800.0, 600.0));
            let view = (centre - cam.eye()).normalize();
            for (k, axis) in AXES.into_iter().enumerate() {
                for step in 0..24 {
                    let phi = f64::from(step) * 15f64.to_radians();
                    let (at, heading) = ring_point_and_heading(&cam, centre, k, phi);
                    let side = grab_side(&cam, centre, axis, at);
                    if view.dot(axis).abs() >= GRAZING {
                        // Read exactly, the half held and all.
                        assert_eq!(side, 1.0, "axis {k}, {step}");
                        exact += 1;
                        continue;
                    }
                    let tall = half_height(&cam, centre, k);
                    if tall < 3.5 {
                        assert_eq!(side, 1.0, "axis {k} is nearly edge-on ({tall} points)");
                        thin += 1;
                        continue;
                    }
                    if tall < 4.5 {
                        continue; // where "too thin" begins
                    }
                    let cos = phi.cos();
                    if cos <= -0.5 {
                        assert_eq!(side, -1.0, "axis {k}, the far half at {step}");
                        far += 1;
                    } else if cos >= -0.2 {
                        assert_eq!(side, 1.0, "axis {k}, the near half or a tip at {step}");
                        if cos >= 0.5 {
                            near += 1;
                        } else {
                            tips += 1;
                        }
                    }
                    if cos.abs() >= 0.5 {
                        for (pull, sign) in [(20.0, 1.0), (-20.0, -1.0)] {
                            let to = at + heading * pull;
                            let turned = held_ring_angle(&cam, centre, axis, side, at, to).unwrap();
                            assert_eq!(turned.signum(), sign, "axis {k}, {step}, pulled {pull}");
                            // About 20 points of the 60 point radius, when the point held goes
                            // the way the ring's ellipse is long (round its tips it goes
                            // across, and a pull along that is a pull partly across the ring).
                            if cos.abs() >= 0.95 {
                                assert!(
                                    (turned.abs() * RING_PT / 20.0 - 1.0).abs() < 0.1,
                                    "axis {k}, {step}: {}°",
                                    turned.to_degrees()
                                );
                            }
                        }
                    }
                }
            }
        }
        assert!(
            near > 1000 && far > 1000 && tips > 500 && thin > 500 && exact > 3000,
            "{near} near, {far} far, {tips} at the tips, {thin} too thin, {exact} read exactly"
        );
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
