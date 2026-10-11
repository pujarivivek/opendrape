//! M4a's drape gate: a skirt drafted the way a student would (a front cut on the fold, a
//! mirrored pair of back panels, the side seams and the centre back sewn, each piece moved to
//! waist height and Placed at the front or back of the form) is draped on the default dress form
//! through the same code the app runs (`opendrape-drape`: its `Stage` for the form, its frame,
//! its floor and its surface-distance ray, and its `build_drape` for the fabric, the
//! placements and the stitches).

use opendrape_drape::{DrapeNote, Stage, build_drape};
use opendrape_mesh::{MeshParams, build};
use opendrape_sim::{BodyCollider, FRAME_DT, Solver};
use opendrape_testkit::drafted;
use opendrape_testkit::metrics::{measure, position_hash};

/// The skirt's cloth, made the way the app makes it, and how many stitches the mesh has.
fn cloth(stage: &Stage) -> (Solver, usize) {
    let pr = drafted::skirt(stage);
    let (solver, notes) = build_drape(&pr, stage);
    let starts_inside = notes
        .iter()
        .filter(|n| matches!(n, DrapeNote::StartsInside(_)))
        .count();
    assert_eq!(
        starts_inside, 0,
        "Place at… leaves every piece clear of the form"
    );
    assert_eq!(notes, vec![], "nothing to tell the student");
    let stitches = build(&pr, &MeshParams::default()).stitches.len();
    (solver, stitches)
}

/// The form alone, for measuring how far the cloth is inside it.
fn body(stage: &Stage) -> BodyCollider {
    let (positions, triangles) = stage.render_mesh();
    BodyCollider::new(positions, triangles).expect("the form is closed")
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

#[test]
fn a_drafted_skirt_drapes_on_the_form_without_poking_through_and_settles() {
    let stage = Stage::shared();
    let (mut solver, mesh_stitches) = cloth(&stage);
    let particles = solver.cloth().len();
    assert!(
        (4_000..30_000).contains(&particles),
        "{particles} particles"
    );
    // The solver has every stitch the mesh made; these are the pairs that weld.
    let stitched: Vec<(usize, usize)> = solver.cloth().stitch_pairs().collect();
    assert_eq!(
        stitched.len(),
        mesh_stitches,
        "every stitch reached the cloth"
    );
    let collider = stage.drape_collider();
    let frames = (6.0 / FRAME_DT).round() as usize;
    // The seams as they are just before they weld: the state at the start of the last step
    // that still has open stitches (the step that welds them runs next).
    let mut before_weld = None;
    for _ in 0..frames {
        if solver.cloth().has_open_stitches() {
            before_weld = Some((solver.time(), seam_gaps(&solver)));
        }
        solver.step(Some(collider));
    }
    let (time, (gap_max, gap_mean)) = before_weld.expect("the seams were open at the start");
    eprintln!(
        "seam gaps before welding: max {gap_max:.2} mm, mean {gap_mean:.2} mm at {time:.3} s"
    );
    // Seams weld once they have closed: at the end of the stitch ramp, and well before the
    // drape would give up waiting and pull them shut.
    let close = solver.params().stitch_close_time;
    assert!(
        time >= close - 2.0 * FRAME_DT && time < solver.params().weld_timeout.unwrap(),
        "measured at {time:.4} s, the seams close over {close} s"
    );
    // Each seam welds once it is within the weld gap, so the last open seams are the tight
    // ones, measured just before they weld.
    assert!(
        gap_max <= 4.0 && gap_mean <= 2.0,
        "seams didn't close: {gap_max:.2} / {gap_mean:.2} mm"
    );
    let r = measure(solver.cloth(), &body(&stage));
    eprintln!("{r:#?}");
    assert!(!r.has_nan);
    assert!(
        r.penetration_max_mm <= 2.0 && r.penetration_p99_mm <= 1.0,
        "poke-through"
    );
    // Welding drops the stitches (so the gap measured now is always 0): the gap above is the
    // seam's quality. What this checks is that the weld happened to every stitched pair.
    assert!(!r.open_stitches, "welded within {:.1} s", solver.time());
    let cloth = solver.cloth();
    let unwelded: Vec<_> = stitched
        .iter()
        .filter(|&&(a, b)| cloth.is_alive(a) && cloth.is_alive(b))
        .collect();
    assert!(
        unwelded.is_empty(),
        "{} stitched pairs not welded",
        unwelded.len()
    );
    assert!(
        r.strain_p99 <= 0.10,
        "fabric over-stretched: {}",
        r.strain_p99
    );
    assert!(
        r.kinetic_energy <= 1e-4,
        "still moving: {} J",
        r.kinetic_energy
    );
    // Held up at the waist, and no lower at the hem than its slanted side seams reach (61 cm
    // from the waist, where the centre is 55: the hem dips at the sides, as on an A-line whose
    // hem hasn't been trued), with a little give.
    let side_seam = (drafted::SKIRT_LENGTH_MM.powi(2)
        + (drafted::SKIRT_HEM_QUARTER_MM - drafted::SKIRT_WAIST_QUARTER_MM).powi(2))
    .sqrt();
    let lowest = drafted::SKIRT_WAIST_Y - side_seam / 1000.0 - 0.04;
    assert!(
        r.lowest_y > lowest && r.highest_y > 1.0,
        "slid down: {}..{} (lowest allowed {lowest:.3})",
        r.lowest_y,
        r.highest_y
    );
}

#[test]
fn the_drafted_skirt_drapes_the_same_every_time() {
    let stage = Stage::shared();
    let collider = stage.drape_collider();
    let hash = || {
        let (mut solver, _) = cloth(&stage);
        for _ in 0..90 {
            solver.step(Some(collider));
        }
        position_hash(solver.cloth())
    };
    assert_eq!(hash(), hash());
}
