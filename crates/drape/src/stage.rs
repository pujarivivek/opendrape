//! The form garments drape on, behind one small boundary: what the 3D view draws (plain
//! positions and triangles), what the solver collides with (with a floor), the centre line, the
//! floor's height, how far the form's surface is from its centre line, and its arm lines (a
//! dress form has no arms: an imaginary line hangs from each armhole). No GPU code lives here, so
//! the app and the tests share it.
//!
//! The stage's frame is the form's frame: metres, y up from the floor, the form faces +z and
//! its left is +x, and its centre line (the stand's pole) is x = 0, z = 0. The shoulders and the
//! waist are the form's own stations.

use crate::choice::FormProblem;
use glam::{DVec3, Vec3};
use opendrape_body::form::{BuiltForm, Measurements};
use opendrape_core::{FormChoice, PieceId, Placement, Project};
use opendrape_geom as geom;
pub use opendrape_mesh::place::Arm;
use opendrape_mesh::place::{self, PlaceAt};
use opendrape_sim::{BodyCollider, CompoundCollider};
use std::sync::{Arc, OnceLock};

/// Shoulder height as a share of standing height (the usual proportion of an adult body).
pub const SHOULDER_SHARE: f64 = 0.82;
/// Waist height as a share of standing height, for a stage made from a bare mesh.
pub const WAIST_SHARE: f64 = 0.62;
/// A dress form has no arms: each side gets an imaginary arm line for Place at → armhole. It
/// starts this far (m) out from the armhole plate's centre, at the shoulder point's height...
pub const FORM_ARM_OUT_M: f64 = 0.10;
/// ...leans out this far from straight down...
pub const FORM_ARM_LEAN_DEG: f64 = 20.0;
/// ...and is this long (m).
pub const FORM_ARM_LENGTH_M: f64 = 0.6;

/// A mesh to draw: positions and triangles.
type Mesh = (Vec<Vec3>, Vec<[u32; 3]>);

/// The form, its frame and its collider.
pub struct Stage {
    /// What the form was built for; None for a stage made from a bare mesh.
    choice: Option<FormChoice>,
    positions: Vec<Vec3>,
    triangles: Vec<[u32; 3]>,
    /// The form's tape lines and stand, to draw only (empty for a bare mesh).
    tapes: Mesh,
    stand: Mesh,
    /// The form alone: rays and the stage's own signed distance.
    torso: BodyCollider,
    /// The form and the floor, for the solver.
    collider: CompoundCollider,
    shoulder_y: f64,
    waist_y: f64,
    /// The left arm line (+x), then the right; None for a bare mesh.
    arms: Option<[Arm; 2]>,
    /// The form's measurements as built, mm (empty for a bare mesh).
    measured: Measurements,
}

impl Stage {
    /// A stage from a closed mesh already in the form's frame (metres, y up from the floor, the
    /// form facing +z with its left at +x, its centre line at x = 0, z = 0): its shoulders at
    /// [`SHOULDER_SHARE`] of its height, its waist at [`WAIST_SHARE`], no arm lines, no tapes
    /// and no stand. For tests of odd forms. None when the solver's collider refuses the mesh.
    pub fn from_mesh(positions: Vec<Vec3>, triangles: Vec<[u32; 3]>) -> Option<Self> {
        let torso = BodyCollider::new(&positions, &triangles).ok()?;
        let collider = CompoundCollider::new(
            vec![BodyCollider::new(&positions, &triangles).ok()?],
            Some(0.0),
        );
        let height = f64::from(positions.iter().map(|p| p.y).fold(0.0_f32, f32::max));
        Some(Self {
            choice: None,
            positions,
            triangles,
            tapes: Mesh::default(),
            stand: Mesh::default(),
            torso,
            collider,
            shoulder_y: SHOULDER_SHARE * height,
            waist_y: WAIST_SHARE * height,
            arms: None,
            measured: Measurements::new(),
        })
    }

    /// The stage for a dress form built for `choice`: its torso to drape on (with the floor),
    /// its tapes and stand to draw, its shoulder and waist stations, and an imaginary arm line
    /// from each armhole (the form has no arms, so nothing is measured round them). The stand's
    /// pole is the centre line. None when the solver's collider refuses the torso, or the form
    /// lacks a shoulder or waist station.
    pub fn from_form(choice: FormChoice, built: &BuiltForm) -> Option<Self> {
        let mesh = &built.torso;
        let torso = BodyCollider::new(&mesh.positions, &mesh.triangles).ok()?;
        let parts = vec![BodyCollider::new(&mesh.positions, &mesh.triangles).ok()?];
        Some(Self {
            choice: Some(choice),
            positions: mesh.positions.clone(),
            triangles: mesh.triangles.clone(),
            tapes: (built.tapes.positions.clone(), built.tapes.triangles.clone()),
            stand: (built.stand.positions.clone(), built.stand.triangles.clone()),
            torso,
            collider: CompoundCollider::new(parts, Some(0.0)),
            shoulder_y: *built.stations.get("shoulder")?,
            waist_y: *built.stations.get("waist")?,
            arms: imaginary_arms(built),
            measured: built.measured.clone(),
        })
    }

    /// The stage for the form `choice` names, or why it can't be built.
    pub fn for_choice(choice: &FormChoice) -> Result<Self, FormProblem> {
        let built = crate::choice::build_form(choice)?;
        Ok(Self::from_form(choice.clone(), &built).expect("a built form's torso is closed"))
    }

    /// One stage of the default form (women's Classic US 8), built once: building the collider
    /// takes a moment. The app starts on it, and the tests drape on it.
    pub fn shared() -> Arc<Stage> {
        static STAGE: OnceLock<Arc<Stage>> = OnceLock::new();
        STAGE
            .get_or_init(|| {
                Arc::new(Self::for_choice(&FormChoice::default()).expect("the default form builds"))
            })
            .clone()
    }

    /// What the form was built for; None for a stage made from a bare mesh.
    pub fn choice(&self) -> Option<&FormChoice> {
        self.choice.as_ref()
    }

    /// The form's triangles, to draw.
    pub fn render_mesh(&self) -> (&[Vec3], &[[u32; 3]]) {
        (&self.positions, &self.triangles)
    }

    /// The form's tape lines, to draw (none for a bare mesh).
    pub fn tapes_mesh(&self) -> (&[Vec3], &[[u32; 3]]) {
        (&self.tapes.0, &self.tapes.1)
    }

    /// The form's stand (neck cap, pole and base), to draw (none for a bare mesh).
    pub fn stand_mesh(&self) -> (&[Vec3], &[[u32; 3]]) {
        (&self.stand.0, &self.stand.1)
    }

    /// The form and the floor, for the solver.
    pub fn drape_collider(&self) -> &CompoundCollider {
        &self.collider
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

    /// The form's waist height (m): where the 3D view looks.
    pub fn waist_y(&self) -> f64 {
        self.waist_y
    }

    /// The form's measurements as built (mm, by name; empty for a bare mesh).
    pub fn measured(&self) -> &Measurements {
        &self.measured
    }

    /// How far (m) the form's surface is from its centre line at `angle` (radians from the
    /// front towards the form's left) and height `y`, if a ray from the centre line finds it.
    pub fn surface_distance(&self, angle: f64, y: f64) -> Option<f64> {
        let (x, z) = self.centre_line();
        let dir = DVec3::new(angle.sin(), 0.0, angle.cos());
        self.torso.ray_exit(DVec3::new(x, y, z), dir, 1.0)
    }

    /// Distance (m) to the form's surface, negative inside it.
    pub fn signed_distance(&self, p: DVec3) -> f64 {
        self.torso.signed_distance(p)
    }

    /// The form's arm lines: its left (+x), then its right. None for a bare mesh: Place at →
    /// armhole is not offered then. A dress form has no arms: these are imaginary lines from its
    /// armholes, with nothing round them to measure.
    pub fn arms(&self) -> Option<&[Arm; 2]> {
        self.arms.as_ref()
    }

    /// Every piece and twin with a placement of its own, moved straight out of this form where
    /// it would start inside it or touching it (see `place::moved_clear`). A twin that mirrors
    /// its piece follows the piece. Run after the form changes, as part of the same edit.
    pub fn reseat(&self, project: &mut Project) {
        let moved: Vec<(PieceId, Placement)> = geom::shapes(project)
            .iter()
            .filter_map(|s| {
                let own = project.placement_of(s.id)?;
                let m = place::moved_clear(s, &own, &|q| self.signed_distance(q));
                (m != own).then_some((s.id, m))
            })
            .collect();
        for (id, m) in moved {
            project.set_placement(id, Some(m));
        }
    }

    /// Place at… front, back or a side: where piece or twin `id` of `project` goes when it is
    /// wrapped round this form at `at`, at the height it has now. None when `project` has no
    /// such shape. The one place the form's rays are wired to `place::place_at`: the app's menu
    /// and the tests both ask here.
    pub fn place_at(&self, project: &Project, id: PieceId, at: PlaceAt) -> Option<Placement> {
        let shapes = geom::shapes(project);
        let shape = shapes.iter().find(|s| s.id == id)?;
        Some(place::place_at(
            project,
            shape,
            at,
            &place::layout(&shapes),
            self.shoulder_y,
            &|angle, y| self.surface_distance(angle, y),
        ))
    }

    /// Place at → Left armhole (`arm` 0) or Right armhole (1): where piece or twin `id` of
    /// `project` goes when it is wrapped round that side's arm line, moved down it until no point
    /// is inside the form (see `place::place_at_arm`). None when `project` has no such shape or
    /// this form no such arm line. Like [`Self::place_at`], the one place this is wired.
    pub fn place_at_arm(&self, project: &Project, id: PieceId, arm: usize) -> Option<Placement> {
        let on = self.arms.as_ref()?.get(arm)?;
        let shapes = geom::shapes(project);
        let shape = shapes.iter().find(|s| s.id == id)?;
        Some(place::place_at_arm(
            shape,
            on,
            // No arm round the line to measure: the sleeve takes its usual radius.
            &|_, _| None,
            &|p| self.signed_distance(p) < 0.0,
        ))
    }
}

/// The imaginary arm lines of a form without arms: from [`FORM_ARM_OUT_M`] out of each
/// armhole plate's centre, at the shoulder point's height, down and out at
/// [`FORM_ARM_LEAN_DEG`]. The left (+x) first. None when the form lacks the landmarks.
fn imaginary_arms(built: &BuiltForm) -> Option<[Arm; 2]> {
    let lean = FORM_ARM_LEAN_DEG.to_radians();
    let arm = |suffix: &str| -> Option<Arm> {
        let plate = *built.landmarks.get(&format!("plate_centre{suffix}"))?;
        let shoulder = *built.landmarks.get(&format!("shoulder_point{suffix}"))?;
        let side = plate.x.signum();
        Some(Arm {
            shoulder: DVec3::new(plate.x + side * FORM_ARM_OUT_M, shoulder.y, plate.z),
            direction: DVec3::new(side * lean.sin(), -lean.cos(), 0.0),
            length: FORM_ARM_LENGTH_M,
            free: 0.0,
        })
    };
    Some([arm("")?, arm("_R")?])
}

#[cfg(test)]
mod tests {
    use super::*;
    use glam::DVec2;
    use opendrape_sim::{ClothBuilder, Collider, Panel, Params, Plane, Solid, Solver};

    #[test]
    fn the_form_stands_on_its_stand_round_its_centre_line() {
        let stage = Stage::shared();
        let (positions, _) = stage.render_mesh();
        let low = positions.iter().map(|p| p.y).fold(f32::MAX, f32::min);
        let stand_low = stage
            .stand_mesh()
            .0
            .iter()
            .map(|p| p.y)
            .fold(f32::MAX, f32::min);
        assert!(
            stand_low.abs() < 1e-4,
            "the stand is on the floor: {stand_low}"
        );
        assert!((0.4..0.8).contains(&low), "the torso is up on it: {low}");
        // At the hips, the form is about as far out in front as behind, and left as right.
        use std::f64::consts::{FRAC_PI_2, PI};
        let hip = stage.waist_y() - 0.2;
        let d = |a: f64| stage.surface_distance(a, hip).expect("the hips");
        assert!(
            (d(0.0) - d(PI)).abs() < 0.08,
            "front {} back {}",
            d(0.0),
            d(PI)
        );
        assert!((d(FRAC_PI_2) - d(-FRAC_PI_2)).abs() < 1e-3);
        assert!(
            (1.2..1.45).contains(&stage.shoulder_y()),
            "{}",
            stage.shoulder_y()
        );
        assert!(
            stage.signed_distance(DVec3::new(0.0, stage.waist_y(), 0.0)) < 0.0,
            "inside at the waist"
        );
        assert_eq!((stage.centre_line(), stage.floor_y()), ((0.0, 0.0), 0.0));
        // The stage's own signed distance is the form's alone: below the floor is not inside it
        // (the compound collider counts the floor, and this must not).
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
            s.step(Some(collider));
        }
        for p in s.cloth().positions() {
            assert!(
                p.y >= 0.003 - 1e-4 && p.y < 0.02,
                "resting on the floor: {p}"
            );
        }
    }

    /// A closed tube (a cylinder with lids), facing outwards: radius `r` round (`cx`, `cz`),
    /// from `y0` to `y1`.
    fn tube(cx: f32, cz: f32, r: f32, y0: f32, y1: f32) -> (Vec<Vec3>, Vec<[u32; 3]>) {
        const N: u32 = 16;
        let mut p = Vec::new();
        for y in [y0, y1] {
            for i in 0..N {
                let a = std::f32::consts::TAU * i as f32 / N as f32;
                p.push(Vec3::new(cx + r * a.cos(), y, cz + r * a.sin()));
            }
        }
        p.push(Vec3::new(cx, y0, cz));
        p.push(Vec3::new(cx, y1, cz));
        let mut t = Vec::new();
        for i in 0..N {
            let j = (i + 1) % N;
            t.push([i, j, N + j]);
            t.push([i, N + j, N + i]);
            t.push([2 * N, j, i]);
            t.push([2 * N + 1, N + i, N + j]);
        }
        // Outwards: a closed surface with a negative volume is inside out.
        let volume: f32 = t
            .iter()
            .map(|&[a, b, c]| p[a as usize].dot(p[b as usize].cross(p[c as usize])))
            .sum();
        if volume < 0.0 {
            for tri in &mut t {
                tri.swap(1, 2);
            }
        }
        (p, t)
    }

    #[test]
    fn a_bare_mesh_has_no_arm_lines() {
        // A bare torso: every cut is one loop in the middle.
        let (positions, triangles) = tube(0.0, 0.0, 0.17, 0.0, 1.6);
        let stage = Stage::from_mesh(positions, triangles).expect("a closed form");
        assert_eq!(stage.arms(), None);
        assert!(
            (stage.shoulder_y() - 1.312).abs() < 1e-6,
            "still has shoulders"
        );
        // No triangles at all is no stage.
        assert!(Stage::from_mesh(vec![], vec![]).is_none());
    }

    #[test]
    fn sleeves_from_ten_to_sixty_centimetres_long_are_wrapped_round_the_arm_line_alike() {
        use opendrape_core::{Piece, Point2};
        let stage = Stage::shared();
        for tall in [100.0, 250.0, 400.0, 600.0] {
            let mut pr = Project::new();
            let id = pr.add_piece(Piece::rectangle(
                PieceId(0),
                "Sleeve",
                Point2::new(0.0, 0.0),
                340.0,
                tall,
            ));
            let p = stage.place_at_arm(&pr, id, 0).unwrap();
            // Nothing round the line to measure: every sleeve takes the usual radius.
            let r = p.curve.unwrap();
            assert!(
                (r - place::ARM_FALLBACK_RADIUS_M).abs() < 1e-9,
                "{tall} mm long: {r:.3} m"
            );
        }
    }

    /// A 340 × 250 mm sleeve and its mirror-image twin, and a 300 × 450 mm front.
    fn sleeve_and_front() -> (Project, PieceId, PieceId, PieceId) {
        use opendrape_core::{Piece, Point2};
        let mut pr = Project::new();
        let sleeve = pr.add_piece(Piece::rectangle(
            PieceId(0),
            "Sleeve",
            Point2::new(0.0, 0.0),
            340.0,
            250.0,
        ));
        let twin = pr
            .add_twin(sleeve, "Sleeve (mirror)".into(), Point2::new(900.0, 0.0))
            .unwrap();
        let front = pr.add_piece(Piece::rectangle(
            PieceId(0),
            "Front",
            Point2::new(0.0, 500.0),
            300.0,
            450.0,
        ));
        (pr, sleeve, twin, front)
    }

    /// Where shape `id`'s outline points are in 3D, given placement `p`.
    fn placed_points(project: &Project, id: PieceId, p: &Placement) -> Vec<DVec3> {
        let shapes = geom::shapes(project);
        let shape = shapes.iter().find(|s| s.id == id).unwrap();
        geom::outline_points(&shape.piece, 5.0)
            .into_iter()
            .map(|q| place::apply(p, place::centre_of(shape), q))
            .collect()
    }

    #[test]
    fn place_at_wraps_a_piece_round_the_form_on_the_side_asked_for() {
        let stage = Stage::shared();
        let (pr, _, _, front) = sleeve_and_front();
        let outside = |at: PlaceAt| {
            let p = stage.place_at(&pr, front, at).expect("a shape");
            let points = placed_points(&pr, front, &p);
            // Clear of the form, and the middle of the piece on the side asked for.
            assert!(
                points.iter().all(|q| stage.signed_distance(*q) > -0.002),
                "{at:?} is inside the form"
            );
            points.iter().fold(DVec3::ZERO, |a, q| a + *q) / points.len() as f64
        };
        assert!(outside(PlaceAt::Front).z > 0.05);
        assert!(outside(PlaceAt::Back).z < -0.05);
        assert!(outside(PlaceAt::LeftSide).x > 0.1);
        assert!(outside(PlaceAt::RightSide).x < -0.1);
        assert_eq!(stage.place_at(&pr, PieceId(99), PlaceAt::Front), None);
    }

    #[test]
    fn place_at_arm_puts_a_sleeve_round_the_arm_asked_for_clear_of_the_form() {
        let stage = Stage::shared();
        let arms = *stage.arms().unwrap();
        let (pr, sleeve, twin, _) = sleeve_and_front();
        for (arm, id) in [(0, sleeve), (1, twin), (1, sleeve)] {
            let p = stage
                .place_at_arm(&pr, id, arm)
                .expect("a shape and an arm");
            let r = p.curve.expect("curved round the arm");
            let points = placed_points(&pr, id, &p);
            // Every point is a sleeve's radius from this arm's line, nearer it than the other,
            // and none is inside the form.
            for q in &points {
                assert!((arms[arm].distance(*q) - r).abs() < 1e-6);
                assert!(arms[arm].distance(*q) < arms[1 - arm].distance(*q));
                assert!(stage.signed_distance(*q) > -0.002, "inside the form");
            }
        }
        assert_eq!(stage.place_at_arm(&pr, sleeve, 2), None, "no third arm");
        assert_eq!(stage.place_at_arm(&pr, PieceId(99), 0), None);
    }
    fn form_stage(choice: &FormChoice) -> Stage {
        Stage::for_choice(choice).expect("a bundled size")
    }

    #[test]
    fn a_form_stage_has_its_shoulders_waist_and_centre_line_from_the_form() {
        let choice = FormChoice::default();
        let built = crate::choice::build_form(&choice).unwrap();
        let stage = form_stage(&choice);
        assert_eq!(stage.choice(), Some(&choice));
        assert!((stage.shoulder_y() - built.stations["shoulder"]).abs() < 1e-12);
        assert!((stage.waist_y() - built.stations["waist"]).abs() < 1e-12);
        // The pole is the centre line: rays from it find the torso all round at the waist.
        use std::f64::consts::{FRAC_PI_2, PI};
        for a in [0.0, FRAC_PI_2, PI, -FRAC_PI_2] {
            let d = stage
                .surface_distance(a, stage.waist_y())
                .expect("the waist");
            assert!((0.05..0.2).contains(&d), "{a}: {d}");
        }
        assert!(stage.signed_distance(DVec3::new(0.0, stage.waist_y(), 0.0)) < 0.0);
        // Its own signed distance is the torso's alone; the collider also has the floor.
        let below = DVec3::new(2.0, -0.1, 2.0);
        assert!(stage.signed_distance(below) > 0.0);
        let with_floor = opendrape_sim::Solid::signed_distance(stage.drape_collider(), below);
        assert!((with_floor + 0.1).abs() < 1e-9, "{with_floor}");
        assert_eq!(stage.measured(), &built.measured);
        assert!(!stage.tapes_mesh().1.is_empty() && !stage.stand_mesh().1.is_empty());
        let low = stage
            .stand_mesh()
            .0
            .iter()
            .map(|p| p.y)
            .fold(f32::MAX, f32::min);
        assert!(low.abs() < 1e-4, "the stand stands on the floor: {low}");
    }

    #[test]
    fn a_form_has_an_imaginary_arm_line_from_each_armhole_with_no_surface() {
        let stage = form_stage(&FormChoice::default());
        let [left, right] = *stage.arms().expect("imaginary arms");
        let mirror = |v: DVec3| DVec3::new(-v.x, v.y, v.z);
        assert!((right.shoulder - mirror(left.shoulder)).length() < 1e-9);
        assert!((right.direction - mirror(left.direction)).length() < 1e-9);
        let lean = left.direction.y.abs().acos().to_degrees();
        assert!(left.direction.x > 0.0 && (lean - FORM_ARM_LEAN_DEG).abs() < 1e-9);
        // Clear of the torso by more than a sleeve's starting radius, at the shoulder's height.
        let clear = stage.signed_distance(left.shoulder);
        assert!(
            clear > opendrape_mesh::place::ARM_FALLBACK_RADIUS_M,
            "{clear} at {}",
            left.shoulder
        );
        assert!(
            (left.shoulder.y - stage.shoulder_y()).abs() < 0.08,
            "{}",
            left.shoulder
        );
    }

    #[test]
    fn a_sleeve_placed_at_an_armhole_starts_clear_of_the_form() {
        use opendrape_core::{Piece, Point2};
        let stage = form_stage(&FormChoice::default());
        let mut pr = Project::new();
        let id = pr.add_piece(Piece::rectangle(
            PieceId(0),
            "Sleeve",
            Point2::new(0.0, 0.0),
            340.0,
            200.0,
        ));
        let shape = &geom::shapes(&pr)[0];
        let outline = geom::outline_points(&shape.piece, 0.5);
        let (lo, hi) = opendrape_mesh::bounds(&outline);
        let centre = lo.lerp(hi, 0.5);
        for arm in [0, 1] {
            let placed = stage.place_at_arm(&pr, id, arm).expect("an arm");
            for q in &outline {
                let d = stage.signed_distance(place::apply(&placed, centre, *q));
                assert!(d > 0.0, "arm {arm}: {d}");
            }
        }
    }
    #[test]
    fn going_up_sizes_moves_placed_pieces_out_of_the_bigger_form() {
        use opendrape_core::{Piece, Point2};
        let small = form_stage(&FormChoice::default());
        let big =
            form_stage(&crate::choice::chart_choice("women-torso", "classic", "US 18").unwrap());
        let mut pr = Project::new();
        let front = pr.add_piece(Piece::rectangle(
            PieceId(0),
            "Front",
            Point2::new(0.0, 0.0),
            380.0,
            600.0,
        ));
        let back = pr.add_piece(Piece::rectangle(
            PieceId(0),
            "Back",
            Point2::new(500.0, 0.0),
            380.0,
            600.0,
        ));
        for (id, at) in [(front, PlaceAt::Front), (back, PlaceAt::Back)] {
            let p = small.place_at(&pr, id, at).unwrap();
            pr.set_placement(id, Some(p));
        }
        // The nearest any point of any placed piece comes to `stage` (negative: inside it).
        let nearest = |stage: &Stage, pr: &Project| -> f64 {
            geom::shapes(pr)
                .iter()
                .flat_map(|s| {
                    let p = pr.placement_of(s.id).unwrap();
                    let centre = place::centre_of(s);
                    geom::outline_points(&s.piece, 0.5)
                        .into_iter()
                        .map(move |q| stage.signed_distance(place::apply(&p, centre, q)))
                        .collect::<Vec<_>>()
                })
                .fold(f64::MAX, f64::min)
        };
        assert!(
            nearest(&big, &pr) < 0.0,
            "the test needs a piece inside the bigger form"
        );
        let before = pr.clone();
        big.reseat(&mut pr);
        assert!(nearest(&big, &pr) >= place::RESEAT_GAP_M - 1e-9);
        assert_ne!(pr, before);
        // Going back down leaves them where they are: they fall in when draped.
        let mut down = pr.clone();
        small.reseat(&mut down);
        assert_eq!(down, pr);
    }
}
