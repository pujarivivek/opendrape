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

use opendrape_core::{
    Edge, Half, Notch, OutlinePos, Piece, PieceId, Placement, Point2, Project, SeamSide,
};
use opendrape_drape::{Drape, Stage};
use opendrape_geom as geom;
use opendrape_mesh::place::{self, PlaceAt};
use opendrape_sim::{BodyCollider, FRAME_DT, Solver};
use opendrape_testkit::metrics::{measure, position_hash};
use std::sync::Arc;

/// Where the bodice's neck points go (m): on the base of the neck.
const NECK_Y: f64 = 1.36;

fn p(x: f64, y: f64) -> Point2 {
    Point2::new(x, y)
}

/// The T-shirt and the ids the gate needs.
struct Shirt {
    project: Project,
    front: PieceId,
    sleeve: PieceId,
    twin: PieceId,
}

fn shirt(stage: &Stage) -> Shirt {
    let mut pr = Project::new();
    // The front half, its fold the centre front (its last edge, on the left): hem 250 wide,
    // side 400 long, an armhole curving in to the shoulder point, a sloping shoulder and a
    // scooped neckline.
    let mut front = Piece::polygon(
        PieceId(0),
        "Front",
        &[
            p(0.0, 0.0),
            p(250.0, 0.0),
            p(250.0, 400.0),
            p(190.0, 590.0),
            p(75.0, 615.0),
            p(0.0, 540.0),
        ],
    );
    front.edges[2] = Edge::Curve {
        c1: p(210.0, 420.0),
        c2: p(185.0, 520.0),
    };
    front.edges[4] = Edge::Curve {
        c1: p(75.0, 570.0),
        c2: p(40.0, 540.0),
    };
    front.fold = Some(5);
    // The back half, drawn to the left of its fold (the centre back, its edge 1), with a
    // shallower neckline: placed at the back, its drawn half is on the body's left, as the
    // front's is.
    let mut back = Piece::polygon(
        PieceId(0),
        "Back",
        &[
            p(550.0, 0.0),
            p(800.0, 0.0),
            p(800.0, 595.0),
            p(725.0, 615.0),
            p(610.0, 590.0),
            p(550.0, 400.0),
        ],
    );
    back.edges[2] = Edge::Curve {
        c1: p(770.0, 595.0),
        c2: p(725.0, 600.0),
    };
    back.edges[4] = Edge::Curve {
        c1: p(615.0, 520.0),
        c2: p(590.0, 420.0),
    };
    back.fold = Some(1);
    // A sleeve: hem 300, underarm edges 130 long, a cap (edge 2, from the right underarm
    // corner round to the left) 97.5 mm high with a notch at its top.
    let mut sleeve = Piece::polygon(
        PieceId(0),
        "Sleeve",
        &[
            p(920.0, 0.0),
            p(1220.0, 0.0),
            p(1240.0, 130.0),
            p(900.0, 130.0),
        ],
    );
    sleeve.edges[2] = Edge::Curve {
        c1: p(1190.0, 260.0),
        c2: p(950.0, 260.0),
    };
    let cap = geom::edge_length(&sleeve, 2);
    sleeve.notches = vec![Notch::new(2, cap / 2.0)];
    let front = pr.add_piece(front);
    let back = pr.add_piece(back);
    let sleeve = pr.add_piece(sleeve);
    let twin = pr
        .add_twin(sleeve, "Sleeve (mirror)".into(), p(2540.0, 0.0))
        .unwrap();
    let whole = |shape, edge, forward| SeamSide::edges(shape, Half::Drawn, edge, edge, forward);
    let cap_part = |from: f64, to: f64| SeamSide {
        shape: sleeve,
        half: Half::Drawn,
        from: OutlinePos::new(2, from),
        to: OutlinePos::new(2, to),
        forward: false,
    };
    // W: the shoulders (from the shoulder points), the sides (from the hem), and the sleeve's
    // underarm (from the hem).
    pr.add_seam(whole(front, 3, true), whole(back, 3, false));
    pr.add_seam(whole(front, 1, true), whole(back, 5, false));
    pr.add_seam(whole(sleeve, 1, true), whole(sleeve, 3, false));
    // F: the cap from its left underarm corner back to the notch, into the front armhole from
    // the underarm up; then from the notch on to its right corner, into the back armhole from
    // the shoulder down.
    pr.add_seam(cap_part(1.0, 0.5), whole(front, 2, true));
    pr.add_seam(cap_part(0.5, 0.0), whole(back, 4, true));
    assert_eq!(pr.check(), Ok(()));
    assert_eq!(pr.all_seams().len(), 10, "every seam has its mirror image");
    // Typed in Properties: the bodice up, so its neck points sit on the base of the neck.
    let middle = NECK_Y - 0.615 / 2.0;
    for id in [front, back] {
        assert!(pr.set_placement(id, Some(Placement::at([0.0, middle, 0.4]))));
    }
    // Place at front and back, as the app does it.
    for (id, at) in [(front, PlaceAt::Front), (back, PlaceAt::Back)] {
        let shapes = geom::shapes(&pr);
        let shape = shapes.iter().find(|s| s.id == id).unwrap();
        let placed = place::place_at(
            &pr,
            shape,
            at,
            &place::layout(&shapes),
            stage.shoulder_y(),
            &|angle, y| stage.surface_distance(angle, y),
        );
        assert!(pr.set_placement(id, Some(placed)));
    }
    // Place at → Left arm, as the app does it: the twin, with no placement of its own, mirrors
    // it onto the right arm.
    let shapes = geom::shapes(&pr);
    let shape = shapes.iter().find(|s| s.id == sleeve).unwrap();
    let placed = place::place_at_arm(
        shape,
        &stage.arms().expect("the bundled body has arms")[0],
        &|along, angle| stage.arm_surface_distance(0, along, angle),
        &|q| stage.signed_distance(q) < 0.0,
    );
    assert!(pr.set_placement(sleeve, Some(placed)));
    assert_eq!(pr.check(), Ok(()));
    Shirt {
        project: pr,
        front,
        sleeve,
        twin,
    }
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
    let s = shirt(&stage);
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
            before_weld = Some(seam_gaps(&drape.solver));
        }
        drape.solver.step(Some(&collider));
    }
    let (gap_max, gap_mean) = before_weld.expect("the seams were open at the start");
    eprintln!("seam gaps before welding: max {gap_max:.2} mm, mean {gap_mean:.2} mm");
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
    assert!(
        gap_max <= 4.0 && gap_mean <= 1.0,
        "seams didn't close: {gap_max:.2} / {gap_mean:.2} mm"
    );
    assert!(!r.open_stitches, "welded shut");
    assert!(notch_gap <= 5.0, "the cap notch is off the shoulder seam");
    assert!(farthest <= 0.12, "a sleeve slid off its arm");
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
        let mut drape = Drape::new(Arc::new(shirt(&stage).project), &stage);
        for _ in 0..90 {
            drape.solver.step(Some(&collider));
        }
        position_hash(drape.solver.cloth())
    };
    assert_eq!(hash(), hash());
}

#[test]
fn the_sleeves_start_round_the_arms_clear_of_the_body() {
    let stage = Stage::shared();
    let s = shirt(&stage);
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
