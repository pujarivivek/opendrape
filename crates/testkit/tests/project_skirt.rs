//! M4a's drape gate: a skirt drafted the way a student would (a front cut on the fold, a
//! mirrored pair of back panels, the side seams and the centre back sewn, each piece moved to
//! waist height and Placed at the front or back of the form) is draped on the bundled body
//! through the same code the app runs (`opendrape-drape`: its `Stage` for the form, its frame,
//! its floor and its surface-distance ray, and its `build_drape` for the fabric, the
//! placements and the stitches).

use opendrape_core::{Half, Piece, PieceId, Placement, Point2, Project, SeamSide};
use opendrape_drape::{DrapeNote, Stage, build_drape};
use opendrape_mesh::place::PlaceAt;
use opendrape_mesh::{MeshParams, build};
use opendrape_sim::{BodyCollider, FRAME_DT, Solver};
use opendrape_testkit::metrics::{measure, position_hash};

/// Where the skirt's waist goes, as for M1's demo skirt (m).
const WAIST_Y: f64 = 1.03;
/// The skirt's length (mm).
const LENGTH_MM: f64 = 550.0;

/// The skirt: a front half on the fold (hem 300, waist 177.5, 550 long, the fold its left edge),
/// a "Back left" (side seam slanted on its left, centre back straight on its right) and its
/// mirror image to its right. Side seams: the front's right edge to the back's slanted edge,
/// both starting at the hem (the mirror image sews the other side). Centre back: the back to
/// its twin. Every piece is moved down to waist height, then Placed at front or back.
fn skirt(stage: &Stage) -> Project {
    let p = Point2::new;
    let mut pr = Project::new();
    let mut front = Piece::polygon(
        PieceId(0),
        "Front",
        &[p(0.0, 0.0), p(300.0, 0.0), p(177.5, 550.0), p(0.0, 550.0)],
    );
    front.fold = Some(3);
    let front = pr.add_piece(front);
    let back = pr.add_piece(Piece::polygon(
        PieceId(0),
        "Back left",
        &[
            p(400.0, 0.0),
            p(700.0, 0.0),
            p(700.0, 550.0),
            p(522.5, 550.0),
        ],
    ));
    let twin = pr
        .add_twin(back, "Back right".into(), p(1500.0, 0.0))
        .unwrap();
    let side = |shape, edge, forward| SeamSide::edges(shape, Half::Drawn, edge, edge, forward);
    pr.add_seam(side(front, 1, true), side(back, 3, false));
    pr.add_seam(side(back, 1, true), side(twin, 1, true));
    assert_eq!(pr.check(), Ok(()));
    assert_eq!(
        pr.all_seams().len(),
        3,
        "the side seam's mirror image sews the other side"
    );
    // Typed in Properties: down to waist height (the middle of each piece).
    let middle = WAIST_Y - LENGTH_MM / 2000.0;
    for id in [front, back] {
        assert!(pr.set_placement(id, Some(Placement::at([0.0, middle, 0.4]))));
    }
    // Place at…, through the one helper the app's menu uses.
    for (id, at) in [(front, PlaceAt::Front), (back, PlaceAt::Back)] {
        let placed = stage.place_at(&pr, id, at).expect("the piece is there");
        assert!(pr.set_placement(id, Some(placed)));
    }
    assert_eq!(pr.check(), Ok(()));
    pr
}

/// The skirt's cloth, made the way the app makes it, and how many stitches the mesh has.
fn cloth(stage: &Stage) -> (Solver, usize) {
    let pr = skirt(stage);
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
    // The step that welds starts at the weld time, so that is the state measured.
    let weld = solver.params().weld_time.expect("seams weld");
    assert!(
        time >= weld && time < weld + FRAME_DT,
        "measured at {time:.4} s, the weld is at {weld} s"
    );
    eprintln!("seam gaps before welding: max {gap_max:.2} mm, mean {gap_mean:.2} mm");
    assert!(
        gap_max <= 4.0 && gap_mean <= 1.0,
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
    assert!(
        !r.open_stitches,
        "welded after {:.1} s",
        solver.params().weld_time.unwrap()
    );
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
    assert!(
        r.lowest_y > 0.4 && r.highest_y > 1.0,
        "slid down: {}..{}",
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
