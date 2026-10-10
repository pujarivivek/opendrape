//! M4b's drape gate: a T-shirt drafted the way a student would draft it, draped on the bundled
//! body through the same code the app runs (`opendrape-drape`: its `Stage` for the form, its
//! arms and its rays, and its `Drape` for the fabric, the placements and the stitches).
//! - The pieces: a front and a back cut on the fold, and a mirrored pair of sleeves with a
//!   notch at the top of the cap.
//! - The seams: the shoulders, the sides and the sleeve's underarm sewn whole-edge (W); each
//!   cap sewn into its armhole in two free seams (F) that meet at the cap notch. The mirror
//!   images sew the other side.
//! - The placing: the bodice moved up to the neck (typed), Place at front and back, and Place
//!   at → Left arm for the sleeve (its twin goes on the right arm).

use opendrape_core::{PieceId, Point2};
use opendrape_drape::{Drape, Stage};
use opendrape_geom as geom;
use opendrape_sim::{BodyCollider, FRAME_DT, Solver};
use opendrape_testkit::drafted::t_shirt;
use opendrape_testkit::metrics::{cloth_crossings, measure, position_hash};
use std::sync::Arc;

fn p(x: f64, y: f64) -> Point2 {
    Point2::new(x, y)
}

/// The form alone, for measuring how far the cloth is inside it.
fn body(stage: &Stage) -> BodyCollider {
    let (positions, triangles) = stage.render_mesh();
    BodyCollider::new(positions, triangles).expect("the bundled body is closed")
}

/// The largest and mean distance (mm) between the particles the solver stitches.
fn seam_gaps(solver: &Solver) -> (f64, f64) {
    let x = solver.cloth().positions();
    let d: Vec<f64> = solver
        .cloth()
        .stitch_pairs()
        .map(|(a, b)| (x[a] - x[b]).length() * 1000.0)
        .collect();
    (
        d.iter().copied().fold(0.0, f64::max),
        d.iter().sum::<f64>() / d.len() as f64,
    )
}

/// A triangle corner of the cloth that holds the spot of shape `shape` at `at` (mm): welding
/// renumbers the cloth's triangles in place, so that corner always holds the live particle the
/// spot has become.
fn corner_at(drape: &Drape, shape: PieceId, at: Point2) -> (usize, usize) {
    let panel = drape.fabric.panel(shape).unwrap();
    let mm = |i: usize| p(panel.flat[i][0] * 1000.0, panel.flat[i][1] * 1000.0);
    let v = (0..panel.flat.len())
        .min_by(|&a, &b| mm(a).distance(at).total_cmp(&mm(b).distance(at)))
        .unwrap();
    assert!(mm(v).distance(at) < 1e-6, "a point of the fabric is there");
    panel
        .triangles
        .iter()
        .enumerate()
        .find_map(|(t, tri)| {
            let corner = tri.iter().position(|&c| c as usize == v)?;
            Some((panel.first_triangle + t, corner))
        })
        .unwrap()
}

#[test]
fn a_drafted_t_shirt_drapes_with_its_sleeves_on_the_arms() {
    let stage = Stage::shared();
    let s = t_shirt(&stage);
    let mut drape = Drape::new(Arc::new(s.project.clone()), &stage);
    assert_eq!(drape.notes, vec![], "nothing to tell the student");
    let particles = drape.solver.cloth().len();
    assert!(
        (4_000..30_000).contains(&particles),
        "{particles} particles"
    );
    // The cap notch, and the front's shoulder point (where the shoulder seam ends).
    let shapes = geom::shapes(&s.project);
    let sleeve = &shapes.iter().find(|x| x.id == s.sleeve).unwrap().piece;
    let notch = geom::point_at_distance(sleeve, 2, sleeve.notches[0].distance);
    let notch_corner = corner_at(&drape, s.sleeve, notch);
    let shoulder_corner = corner_at(&drape, s.front, p(190.0, 590.0));
    let collider = stage.drape_collider();
    let frames = (6.0 / FRAME_DT).round() as usize;
    // The seams as they are just before they weld.
    let mut before_weld = None;
    for _ in 0..frames {
        if drape.solver.cloth().has_open_stitches() {
            before_weld = Some((drape.solver.time(), seam_gaps(&drape.solver)));
        }
        drape.solver.step(Some(collider));
    }
    let (time, (gap_max, gap_mean)) = before_weld.expect("the seams were open at the start");
    eprintln!(
        "seam gaps before welding: max {gap_max:.2} mm, mean {gap_mean:.2} mm at {time:.3} s"
    );
    assert!(
        time < drape.solver.params().weld_timeout.unwrap(),
        "every seam closed on its own"
    );
    let cloth = drape.solver.cloth();
    let r = measure(cloth, &body(&stage));
    eprintln!("{r:#?}");
    let x = cloth.positions();
    let held = |(t, c): (usize, usize)| x[cloth.triangles()[t][c] as usize];
    let notch_gap = (held(notch_corner) - held(shoulder_corner)).length() * 1000.0;
    eprintln!("the cap notch is {notch_gap:.2} mm from the shoulder seam's end");
    // Every live particle of each sleeve, and how far it is from its arm's line.
    let mut farthest: f64 = 0.0;
    for (shape, arm) in [(s.sleeve, 0), (s.twin, 1)] {
        let panel = drape.fabric.panel(shape).unwrap();
        let line = stage.arms().expect("the bundled body has arms")[arm];
        let range = panel.first_particle..panel.first_particle + panel.flat.len();
        for (i, q) in x.iter().enumerate().take(range.end).skip(range.start) {
            if cloth.is_alive(i) {
                farthest = farthest.max(line.distance(*q));
            }
        }
    }
    eprintln!(
        "the farthest sleeve point is {:.1} cm from its arm's line",
        farthest * 100.0
    );
    assert!(!r.has_nan);
    assert!(
        r.penetration_max_mm <= 2.0 && r.penetration_p99_mm <= 1.0,
        "poke-through"
    );
    // Each seam welds once it is within the weld gap, so the last open seams are the tight
    // ones (over the shoulders and under the arms), measured just before they weld.
    assert!(
        gap_max <= 4.0 && gap_mean <= 2.0,
        "seams didn't close: {gap_max:.2} / {gap_mean:.2} mm"
    );
    assert!(!r.open_stitches, "welded shut");
    assert!(notch_gap <= 5.0, "the cap notch is off the shoulder seam");
    assert!(farthest <= 0.12, "a sleeve slid off its arm");
    // The sleeves rest on the bodice instead of passing through it. What is left is a patch
    // about 3 cm across at the top of one cap, where three seams meet at a saddle and the
    // fabric folded through itself while the seams pulled shut (self-collision begins once
    // they have welded); a cloth-against-triangle contact for the Fine preset is the fix.
    let crossings = cloth_crossings(cloth);
    eprintln!("the fabric crosses itself {crossings} times");
    assert!(
        crossings <= 30,
        "sleeve through the bodice: {crossings} crossings"
    );
    assert!(
        r.kinetic_energy <= 1e-4,
        "still moving: {} J",
        r.kinetic_energy
    );
    assert!(
        r.strain_p99 <= 0.10,
        "fabric over-stretched: {}",
        r.strain_p99
    );
}

#[test]
fn the_drafted_t_shirt_drapes_the_same_every_time() {
    let stage = Stage::shared();
    let collider = stage.drape_collider();
    let hash = || {
        let mut drape = Drape::new(Arc::new(t_shirt(&stage).project), &stage);
        for _ in 0..90 {
            drape.solver.step(Some(collider));
        }
        position_hash(drape.solver.cloth())
    };
    assert_eq!(hash(), hash());
}

#[test]
fn the_sleeves_start_round_the_arms_clear_of_the_body() {
    let stage = Stage::shared();
    let s = t_shirt(&stage);
    let drape = Drape::new(Arc::new(s.project), &stage);
    let x = drape.solver.cloth().positions();
    for (shape, arm) in [(s.sleeve, 0), (s.twin, 1)] {
        let panel = drape.fabric.panel(shape).unwrap();
        let line = stage.arms().expect("the bundled body has arms")[arm];
        let start = &x[panel.first_particle..panel.first_particle + panel.flat.len()];
        assert!(start.iter().all(|q| stage.signed_distance(*q) > 0.0));
        // Wrapped round its arm, its top a little way down from the shoulder.
        let top = start
            .iter()
            .map(|q| (*q - line.shoulder).dot(line.direction))
            .fold(f64::MAX, f64::min);
        assert!((0.0..0.1).contains(&top), "the top {top:.3} m down the arm");
    }
}
