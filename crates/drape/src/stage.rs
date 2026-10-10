//! The form garments drape on, behind one small boundary: what the 3D view draws (plain
//! positions and triangles), what the solver collides with (with a floor), the centre line, the
//! floor's height, how far the form's surface is from its centre line, and its arms (a line
//! down each, and how far the arm's surface is from it). No GPU code lives here, so the app and
//! the tests share it.
//!
//! The stage's frame is the form's frame: metres, y up from the floor, the form faces +z and
//! its left is +x, and its centre line is x = 0, z = 0 (the body is moved there when loaded).
//! The shoulders are at 0.82 × the form's height.

use glam::{DVec3, Vec3};
use opendrape_body::BodyMesh;
pub use opendrape_mesh::place::Arm;
use opendrape_sim::{BodyCollider, Collider, Plane};
use std::sync::{Arc, OnceLock};

/// Shoulder height as a share of standing height (the usual proportion of an adult body).
pub const SHOULDER_SHARE: f64 = 0.82;
/// A ray from an arm's line looks this far (m) for the arm's surface. One that runs on into the
/// torso finds nothing this close, and so does not count.
pub const ARM_RAY_M: f64 = 0.15;
/// The form is cut across every this many metres to find its arms...
const ARM_STEP_M: f64 = 0.01;
/// ...an arm's cross-section is a loop of the cut whose middle is at least this far (m) out
/// from the centre line (the torso's own never is, below the shoulders)...
const ARM_OUT_M: f64 = 0.15;
/// ...and the arm's line is fitted through its cross-sections this far (m, in height) below the
/// armpit.
const UPPER_ARM_M: f64 = 0.10;

/// The form, its frame and its collider. Dress forms (Track B) swap the body here.
pub struct Stage {
    positions: Vec<Vec3>,
    triangles: Vec<[u32; 3]>,
    collider: BodyCollider,
    shoulder_y: f64,
    /// The left arm (+x), then the right.
    arms: [Arm; 2],
}

impl Stage {
    /// The bundled MakeHuman body, moved so its torso's centre line is at x = 0, z = 0.
    pub fn makehuman() -> Self {
        let body = BodyMesh::female_average();
        let centre = torso_centre(&body);
        let positions: Vec<Vec3> = body.positions.iter().map(|p| *p - centre).collect();
        let collider =
            BodyCollider::new(&positions, &body.triangles).expect("the bundled body is closed");
        let height = f64::from(positions.iter().map(|p| p.y).fold(0.0_f32, f32::max));
        let shoulder_y = SHOULDER_SHARE * height;
        let arms =
            [1.0, -1.0].map(|side| find_arm(&positions, &body.triangles, height, shoulder_y, side));
        Self {
            positions,
            triangles: body.triangles,
            collider,
            shoulder_y,
            arms,
        }
    }

    /// One stage for the whole app (and its tests): building the collider takes a moment.
    pub fn shared() -> Arc<Stage> {
        static STAGE: OnceLock<Arc<Stage>> = OnceLock::new();
        STAGE.get_or_init(|| Arc::new(Self::makehuman())).clone()
    }

    /// The form's triangles, to draw.
    pub fn render_mesh(&self) -> (&[Vec3], &[[u32; 3]]) {
        (&self.positions, &self.triangles)
    }

    /// The form and the floor, for the solver.
    pub fn drape_collider(&self) -> BodyAndFloor<'_> {
        BodyAndFloor {
            body: &self.collider,
            floor: self.floor_y(),
        }
    }

    /// The centre line's x and z: the form's frame puts it at the origin.
    pub fn centre_line(&self) -> (f64, f64) {
        (0.0, 0.0)
    }

    pub fn floor_y(&self) -> f64 {
        0.0
    }

    /// Where pieces start: the top of the pattern lines up with the form's shoulders.
    pub fn shoulder_y(&self) -> f64 {
        self.shoulder_y
    }

    /// How far (m) the form's surface is from its centre line at `angle` (radians from the
    /// front towards the form's left) and height `y`, if a ray from the centre line finds it.
    pub fn surface_distance(&self, angle: f64, y: f64) -> Option<f64> {
        let (x, z) = self.centre_line();
        let dir = DVec3::new(angle.sin(), 0.0, angle.cos());
        self.collider.ray_exit(DVec3::new(x, y, z), dir, 1.0)
    }

    /// Distance (m) to the form's surface, negative inside it.
    pub fn signed_distance(&self, p: DVec3) -> f64 {
        self.collider.signed_distance(p)
    }

    /// The form's arms: its left (+x), then its right.
    pub fn arms(&self) -> &[Arm; 2] {
        &self.arms
    }

    /// How far (m) the surface of arm `arm` (0 left, 1 right) is from its line, `along` metres
    /// down from the shoulder and at `angle` round it (see [`Arm::around`]), if a ray from the
    /// line finds it within [`ARM_RAY_M`].
    pub fn arm_surface_distance(&self, arm: usize, along: f64, angle: f64) -> Option<f64> {
        let a = self.arms.get(arm)?;
        self.collider
            .ray_exit(a.at(along), a.around(angle), ARM_RAY_M)
    }
}

/// The arm on side `side` (+1 the form's left, -1 its right) of a form `height` m tall whose
/// shoulders are at `shoulder_y`. The form is cut across at heights [`ARM_STEP_M`] apart,
/// from the shoulders down to 55% of its height; below the armpit the arm's cut is a loop of its
/// own, its middle at least [`ARM_OUT_M`] out. The armpit is the highest cut where it is: from
/// there down the arm hangs free. The line is fitted (least squares, x and z against height)
/// through the middles of the arm's cuts within [`UPPER_ARM_M`] below the armpit, from shoulder
/// height down to the lowest cut that still finds the arm.
fn find_arm(
    positions: &[Vec3],
    triangles: &[[u32; 3]],
    height: f64,
    shoulder_y: f64,
    side: f64,
) -> Arm {
    let steps = ((shoulder_y - 0.55 * height) / ARM_STEP_M).floor() as usize;
    // From the shoulders down: each cut's height, and the middle of the arm's loop if it has one.
    let cuts: Vec<(f64, Option<(f64, f64)>)> = (1..=steps)
        .map(|k| {
            let y = shoulder_y - k as f64 * ARM_STEP_M;
            let arm = cross_sections(positions, triangles, y)
                .iter()
                .map(|l| loop_middle(l))
                .filter(|(x, _)| side * x >= ARM_OUT_M)
                .max_by(|a, b| (side * a.0).total_cmp(&(side * b.0)));
            (y, arm)
        })
        .collect();
    let first = cuts.iter().position(|(_, arm)| arm.is_some()).unwrap_or(0);
    let armpit = cuts[first].0;
    let last = cuts[first..]
        .iter()
        .position(|(_, arm)| arm.is_none())
        .map_or(cuts.len(), |k| first + k)
        .saturating_sub(1);
    let fit: Vec<(f64, f64, f64)> = cuts[first..=last]
        .iter()
        .filter(|(y, _)| *y >= armpit - UPPER_ARM_M)
        .filter_map(|(y, arm)| arm.map(|(x, z)| (*y, x, z)))
        .collect();
    let n = fit.len().max(1) as f64;
    let (my, mx, mz) = fit.iter().fold((0.0, 0.0, 0.0), |(a, b, c), (y, x, z)| {
        (a + y / n, b + x / n, c + z / n)
    });
    let syy: f64 = fit.iter().map(|(y, _, _)| (y - my) * (y - my)).sum();
    let slope = |pick: fn(&(f64, f64, f64)) -> f64, mean: f64| {
        let s: f64 = fit.iter().map(|f| (f.0 - my) * (pick(f) - mean)).sum();
        if syy > 0.0 { s / syy } else { 0.0 }
    };
    // Up one metre of height, the line moves (dx, dz).
    let (dx, dz) = (slope(|f| f.1, mx), slope(|f| f.2, mz));
    let direction = DVec3::new(-dx, -1.0, -dz).normalize();
    let shoulder = DVec3::new(mx, my, mz) + DVec3::new(dx, 1.0, dz) * (shoulder_y - my);
    Arm {
        shoulder,
        direction,
        length: (shoulder_y - cuts[last].0) / -direction.y,
        free: (shoulder_y - armpit) / -direction.y,
    }
}

/// Where the plane at height `y` cuts the form's surface: each closed loop of the cut, as its
/// (x, z) points in order. The form is closed, with every edge shared by two triangles, so every
/// edge the plane crosses joins two triangles' cuts.
fn cross_sections(positions: &[Vec3], triangles: &[[u32; 3]], y: f64) -> Vec<Vec<(f64, f64)>> {
    use std::collections::{BTreeMap, BTreeSet};
    let above = |i: u32| f64::from(positions[i as usize].y) >= y;
    // Each crossed edge, and the two other crossed edges of its triangles.
    let mut links: BTreeMap<(u32, u32), Vec<(u32, u32)>> = BTreeMap::new();
    for t in triangles {
        let crossed: Vec<(u32, u32)> = (0..3)
            .map(|k| (t[k], t[(k + 1) % 3]))
            .filter(|&(a, b)| above(a) != above(b))
            .map(|(a, b)| (a.min(b), a.max(b)))
            .collect();
        if let [e, f] = crossed[..] {
            links.entry(e).or_default().push(f);
            links.entry(f).or_default().push(e);
        }
    }
    let point = |(a, b): (u32, u32)| {
        let (p, q) = (
            positions[a as usize].as_dvec3(),
            positions[b as usize].as_dvec3(),
        );
        let t = (y - p.y) / (q.y - p.y);
        (p.x + t * (q.x - p.x), p.z + t * (q.z - p.z))
    };
    let mut seen = BTreeSet::new();
    let mut loops = Vec::new();
    for &start in links.keys() {
        if seen.contains(&start) {
            continue;
        }
        let mut cut = Vec::new();
        let mut at = Some(start);
        while let Some(edge) = at {
            seen.insert(edge);
            cut.push(point(edge));
            at = links[&edge].iter().copied().find(|e| !seen.contains(e));
        }
        loops.push(cut);
    }
    loops
}

/// The middle (x, z) of a closed loop: its area centroid, or the mean of its points when it
/// has no area.
fn loop_middle(points: &[(f64, f64)]) -> (f64, f64) {
    let n = points.len();
    let (mut a, mut cx, mut cz) = (0.0, 0.0, 0.0);
    for k in 0..n {
        let ((x0, z0), (x1, z1)) = (points[k], points[(k + 1) % n]);
        let cross = x0 * z1 - x1 * z0;
        a += cross;
        cx += (x0 + x1) * cross;
        cz += (z0 + z1) * cross;
    }
    if a.abs() < 1e-12 {
        let m = n.max(1) as f64;
        return (
            points.iter().map(|p| p.0).sum::<f64>() / m,
            points.iter().map(|p| p.1).sum::<f64>() / m,
        );
    }
    (cx / (3.0 * a), cz / (3.0 * a))
}

/// The torso's centre between the hips (from 0.7 to 0.85 m up, arms left out), with y = 0.
fn torso_centre(body: &BodyMesh) -> Vec3 {
    let (lo, hi) = body
        .positions
        .iter()
        .filter(|p| p.y > 0.7 && p.y < 0.85 && p.x.abs() < 0.22)
        .fold((Vec3::MAX, Vec3::MIN), |(lo, hi), p| {
            (lo.min(*p), hi.max(*p))
        });
    let mid = (lo + hi) / 2.0;
    Vec3::new(mid.x, 0.0, mid.z)
}

/// The form and a floor as one collider. A particle inside either takes the nearest way out;
/// otherwise the nearest surface within the margin. When the dress forms' `CompoundCollider`
/// (which has a floor) arrives, it replaces this.
pub struct BodyAndFloor<'a> {
    body: &'a BodyCollider,
    floor: f64,
}

impl BodyAndFloor<'_> {
    /// Distance to the nearer of the form and the floor, negative inside either.
    pub fn signed_distance(&self, p: DVec3) -> f64 {
        self.body.signed_distance(p).min(p.y - self.floor)
    }
}

impl Collider for BodyAndFloor<'_> {
    fn contact_planes(&self, x: &[DVec3], margin: f64) -> Vec<Option<Plane>> {
        let body = self.body.contact_planes(x, margin);
        x.iter()
            .zip(body)
            .map(|(p, on_body)| {
                let on_floor = (p.y - self.floor < margin).then(|| Plane {
                    normal: DVec3::Y,
                    point: DVec3::new(p.x, self.floor, p.z),
                });
                on_body
                    .into_iter()
                    .chain(on_floor)
                    .map(|plane| ((*p - plane.point).dot(plane.normal), plane))
                    // Inside anything: the nearest way out. Otherwise: the nearest surface.
                    .min_by(|(a, _), (b, _)| {
                        (*a >= 0.0)
                            .cmp(&(*b >= 0.0))
                            .then(a.abs().total_cmp(&b.abs()))
                    })
                    .map(|(_, plane)| plane)
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use glam::DVec2;
    use opendrape_sim::{ClothBuilder, Panel, Params, Solver};

    #[test]
    fn the_form_stands_on_the_floor_round_its_centre_line() {
        let stage = Stage::shared();
        let (positions, _) = stage.render_mesh();
        let low = positions.iter().map(|p| p.y).fold(f32::MAX, f32::min);
        assert!(low.abs() < 0.01, "feet at the floor: {low}");
        // Between the hips, the form is about as far out in front as behind, and left as right.
        use std::f64::consts::{FRAC_PI_2, PI};
        let d = |a: f64| stage.surface_distance(a, 0.80).expect("the hips");
        assert!(
            (d(0.0) - d(PI)).abs() < 0.08,
            "front {} back {}",
            d(0.0),
            d(PI)
        );
        assert!((d(FRAC_PI_2) - d(-FRAC_PI_2)).abs() < 0.01);
        assert!(
            (1.2..1.4).contains(&stage.shoulder_y()),
            "{}",
            stage.shoulder_y()
        );
        let height = positions.iter().map(|p| f64::from(p.y)).fold(0.0, f64::max);
        assert!(
            (stage.shoulder_y() - 0.82 * height).abs() < 1e-6,
            "shoulders at 0.82 of the height"
        );
        assert!(
            stage.signed_distance(DVec3::new(0.0, 1.0, 0.0)) < 0.0,
            "inside at the waist"
        );
        assert_eq!((stage.centre_line(), stage.floor_y()), ((0.0, 0.0), 0.0));
        // The stage's own signed distance is the form's alone: below the floor is not inside it
        // (the dress forms' compound collider counts the floor, and this must not).
        assert!(stage.signed_distance(DVec3::new(2.0, -0.1, 2.0)) > 0.0);
    }

    #[test]
    fn the_floor_holds_particles_up_and_the_form_still_counts() {
        let stage = Stage::shared();
        let both = stage.drape_collider();
        let planes = both.contact_planes(
            &[
                DVec3::new(2.0, 0.01, 2.0),
                DVec3::new(2.0, -0.1, 2.0),
                DVec3::new(2.0, 1.0, 2.0),
                DVec3::new(0.0, 1.0, 0.0),
            ],
            0.05,
        );
        let floor = Some(Plane {
            normal: DVec3::Y,
            point: DVec3::new(2.0, 0.0, 2.0),
        });
        assert_eq!(planes[0], floor);
        assert_eq!(planes[1], floor, "below the floor is pushed back up");
        assert_eq!(planes[2], None, "far from both");
        let inside = planes[3].expect("inside the form");
        assert!(
            inside.normal.y.abs() < 0.9,
            "out through the form, not the floor"
        );
        assert!((both.signed_distance(DVec3::new(2.0, -0.1, 2.0)) + 0.1).abs() < 1e-9);
    }

    #[test]
    fn a_dropped_piece_comes_to_rest_on_the_floor() {
        let stage = Stage::shared();
        let flat = vec![
            DVec2::new(-0.05, -0.05),
            DVec2::new(0.05, -0.05),
            DVec2::new(0.0, 0.05),
        ];
        let panel = Panel {
            positions: flat
                .iter()
                .map(|p| DVec3::new(p.x + 1.0, 0.2, p.y))
                .collect(),
            flat: Some(flat),
            triangles: vec![[0, 1, 2]],
        };
        let mut b = ClothBuilder::new(0.15);
        b.add_panel(&panel, 1.0);
        let mut s = Solver::new(
            b.build(),
            Params {
                gravity_delay: 0.0,
                ..Params::default()
            },
        );
        let collider = stage.drape_collider();
        for _ in 0..180 {
            s.step(Some(&collider));
        }
        for p in s.cloth().positions() {
            assert!(
                p.y >= 0.003 - 1e-4 && p.y < 0.02,
                "resting on the floor: {p}"
            );
        }
    }

    /// A closed box as a form.
    fn slab(lo: Vec3, hi: Vec3) -> BodyCollider {
        let p = [
            [lo.x, lo.y, lo.z],
            [hi.x, lo.y, lo.z],
            [hi.x, hi.y, lo.z],
            [lo.x, hi.y, lo.z],
            [lo.x, lo.y, hi.z],
            [hi.x, lo.y, hi.z],
            [hi.x, hi.y, hi.z],
            [lo.x, hi.y, hi.z],
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
        BodyCollider::new(&p, &t).expect("a closed box")
    }

    #[test]
    fn a_particle_takes_the_nearest_way_out_of_the_form_and_the_floor() {
        let margin = 0.05;
        // A form that reaches 1 cm below the floor.
        let body = slab(Vec3::new(-0.5, -0.01, -0.5), Vec3::new(0.5, 1.0, 0.5));
        let both = BodyAndFloor {
            body: &body,
            floor: 0.0,
        };
        let plane = |both: &BodyAndFloor, p: DVec3| both.contact_planes(&[p], margin)[0].unwrap();
        // Inside the form and above the floor: out through the form (down, 1.4 cm), though the
        // floor is nearer (0.4 cm) it is not something the particle is inside.
        let inside = plane(&both, DVec3::new(0.0, 0.004, 0.0));
        assert!(
            inside.normal.abs_diff_eq(DVec3::NEG_Y, 1e-6) && (inside.point.y + 0.01).abs() < 1e-6,
            "the form's plane, not the floor's: {inside:?}"
        );
        // Outside both, within the margin: the nearer surface wins either way.
        let by_the_floor = plane(&both, DVec3::new(0.52, 0.01, 0.0));
        assert_eq!(
            by_the_floor.normal,
            DVec3::Y,
            "1 cm from the floor, 2 from the form"
        );
        let by_the_form = plane(&both, DVec3::new(0.505, 0.03, 0.0));
        assert!(
            by_the_form.normal.abs_diff_eq(DVec3::X, 1e-6),
            "0.5 cm from the form, 3 from the floor: {:?}",
            by_the_form.normal
        );
        // Inside both: the smaller depth wins. 1 cm below the floor is 49 cm inside a big form.
        let deep = slab(Vec3::splat(-0.5), Vec3::splat(0.5));
        let both = BodyAndFloor {
            body: &deep,
            floor: 0.0,
        };
        let shallow = plane(&both, DVec3::new(0.0, -0.01, 0.0));
        assert_eq!(shallow.normal, DVec3::Y, "up through the floor");
    }

    #[test]
    fn the_arms_hang_down_and_out_from_the_shoulders_and_mirror_each_other() {
        let stage = Stage::shared();
        let [left, right] = *stage.arms();
        let mirror = |v: DVec3| DVec3::new(-v.x, v.y, v.z);
        assert!((right.shoulder - mirror(left.shoulder)).length() < 1e-9);
        assert!((right.direction - mirror(left.direction)).length() < 1e-9);
        assert!((right.length - left.length).abs() < 1e-9 && (right.free - left.free).abs() < 1e-9);
        // Down and out, towards the form's left, at the shoulders.
        let tilt = left.direction.y.abs().acos().to_degrees();
        assert!(
            left.direction.x > 0.0 && (30.0..55.0).contains(&tilt),
            "{tilt}°"
        );
        assert!((left.shoulder.y - stage.shoulder_y()).abs() < 1e-9);
        assert!((0.08..0.25).contains(&left.shoulder.x), "{}", left.shoulder);
        assert!((0.1..0.2).contains(&left.free), "free from {}", left.free);
        assert!((0.4..0.8).contains(&left.length), "{}", left.length);
        // Where the arm hangs free, its line runs inside it: rays find its surface 2.5 to 6 cm
        // away all round, on both arms.
        for along in [left.free, left.free + 0.05, left.free + 0.1] {
            assert!(
                stage.signed_distance(left.at(along)) < 0.0,
                "inside the arm {along}"
            );
            for k in 0..12 {
                let angle = f64::from(k) * 30f64.to_radians();
                for arm in [0, 1] {
                    let d = stage.arm_surface_distance(arm, along, angle);
                    assert!(
                        d.is_some_and(|d| (0.025..0.06).contains(&d)),
                        "arm {arm}, {along:.2} m down, {angle:.2} round: {d:?}"
                    );
                }
            }
        }
    }
}
