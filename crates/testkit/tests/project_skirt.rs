//! M4a's drape gate: a skirt drafted the way a student would (a front cut on the fold, a
//! mirrored pair of back panels, the side seams and the centre back sewn, each piece moved to
//! waist height and Placed at the front or back of the form) is made into fabric by
//! `opendrape-mesh` and draped on the bundled body.

use glam::{DVec2, DVec3};
use opendrape_core::{Half, Piece, PieceId, Placement, Point2, Project, SeamSide};
use opendrape_geom as geom;
use opendrape_mesh::place::{self, PlaceAt};
use opendrape_mesh::{MeshParams, Stitch, build};
use opendrape_sim::{ClothBuilder, FRAME_DT, Panel, Params, Solver};
use opendrape_testkit::garments::{DENSITY, body, collider};
use opendrape_testkit::metrics::{measure, position_hash};

/// Where the skirt's waist goes, as for M1's demo skirt (m).
const WAIST_Y: f64 = 1.03;
/// The skirt's length (mm).
const LENGTH_MM: f64 = 550.0;

/// z of the torso's centre line between the hips, as the app's stage finds it (its x is 0).
fn centre_z() -> f64 {
    let (lo, hi) = body()
        .positions
        .iter()
        .filter(|p| p.y > 0.7 && p.y < 0.85 && p.x.abs() < 0.22)
        .fold((f32::MAX, f32::MIN), |(lo, hi), p| {
            (lo.min(p.z), hi.max(p.z))
        });
    f64::from((lo + hi) / 2.0)
}

/// The skirt: a front half on the fold (hem 300, waist 177.5, 550 long, the fold its left edge),
/// a "Back left" (side seam slanted on its left, centre back straight on its right) and its
/// mirror image to its right. Side seams: the front's right edge to the back's slanted edge,
/// both starting at the hem (the mirror image sews the other side). Centre back: the back to
/// its twin. Every piece is moved down to waist height, then Placed at front or back.
fn skirt() -> Project {
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
    let side =
        |shape, first_edge, forward| SeamSide::new(shape, Half::Drawn, first_edge, 1, forward);
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
        pr.set_placement(id, Some(Placement::at([0.0, middle, 0.4])));
    }
    // Place at…, measuring the form from its centre line.
    let zc = centre_z();
    let surface = |angle: f64, y: f64| {
        let dir = DVec3::new(angle.sin(), 0.0, angle.cos());
        collider().ray_exit(DVec3::new(0.0, y, zc), dir, 1.0)
    };
    for (id, at) in [(front, PlaceAt::Front), (back, PlaceAt::Back)] {
        let shapes = geom::shapes(&pr);
        let shape = shapes.iter().find(|s| s.id == id).unwrap();
        let placed = place::place_at(&pr, shape, at, &place::layout(&shapes), 1.3, &surface);
        pr.set_placement(id, Some(placed));
    }
    assert_eq!(pr.check(), Ok(()));
    pr
}

/// The skirt's cloth, its stitches as pairs of particle indices, and how many particles start
/// inside the body.
fn cloth() -> (Solver, Vec<(usize, usize)>, usize) {
    let pr = skirt();
    let mesh = build(&pr, &MeshParams::default());
    assert!(mesh.notes.is_empty(), "{:?}", mesh.notes);
    let shapes = geom::shapes(&pr);
    let layout = place::layout(&shapes);
    // The placements are in the form's frame; the bundled body's centre line is at z = zc.
    let shift = DVec3::new(0.0, 0.0, centre_z());
    let mut builder = ClothBuilder::new(DENSITY);
    let mut first = Vec::new();
    let mut count = 0;
    for panel in &mesh.panels {
        let shape = shapes.iter().find(|s| s.id == panel.shape).unwrap();
        let placement = place::effective(&pr, shape, &layout, 1.3);
        let positions = panel
            .flat
            .iter()
            .map(|f| {
                place::apply(
                    &placement,
                    panel.centre,
                    Point2::new(f[0] * 1000.0, f[1] * 1000.0),
                ) + shift
            })
            .collect();
        let id = builder.add_panel(
            &Panel {
                positions,
                flat: Some(panel.flat.iter().map(|f| DVec2::from_array(*f)).collect()),
                triangles: panel.triangles.clone(),
            },
            1.0,
        );
        first.push((id, count));
        count += panel.flat.len();
    }
    let pairs = mesh
        .stitches
        .iter()
        .map(|&((pa, a), (pb, b)): &Stitch| {
            builder.stitch((first[pa].0, a), (first[pb].0, b));
            (first[pa].1 + a as usize, first[pb].1 + b as usize)
        })
        .collect();
    let cloth = builder.build();
    let inside = cloth
        .positions()
        .iter()
        .filter(|p| collider().signed_distance(**p) < 0.0)
        .count();
    (Solver::new(cloth, Params::default()), pairs, inside)
}

/// The largest and mean distance (mm) between stitched particles.
fn seam_gaps(solver: &Solver, pairs: &[(usize, usize)]) -> (f64, f64) {
    let x = solver.cloth().positions();
    let d: Vec<f64> = pairs
        .iter()
        .map(|&(a, b)| (x[a] - x[b]).length() * 1000.0)
        .collect();
    (
        d.iter().copied().fold(0.0, f64::max),
        d.iter().sum::<f64>() / d.len() as f64,
    )
}

#[test]
fn a_drafted_skirt_drapes_on_the_form_without_poking_through_and_settles() {
    let (mut solver, pairs, inside) = cloth();
    let particles = solver.cloth().len();
    assert!(
        (4_000..30_000).contains(&particles),
        "{particles} particles"
    );
    assert_eq!(inside, 0, "Place at… leaves every piece clear of the form");
    let weld = Params::default().weld_time.unwrap();
    let frames = (6.0 / FRAME_DT).round() as usize;
    let mut before_weld = None;
    for _ in 0..frames {
        solver.step(Some(collider()));
        // The last frame before the seams weld: they have pulled shut by now.
        if before_weld.is_none() && solver.time() + FRAME_DT >= weld {
            before_weld = Some(seam_gaps(&solver, &pairs));
        }
    }
    let (gap_max, gap_mean) = before_weld.unwrap();
    eprintln!("seam gaps before welding: max {gap_max:.2} mm, mean {gap_mean:.2} mm");
    assert!(
        gap_max <= 4.0 && gap_mean <= 1.0,
        "seams didn't close: {gap_max:.2} / {gap_mean:.2} mm"
    );
    let r = measure(solver.cloth(), collider());
    eprintln!("{r:#?}");
    assert!(!r.has_nan);
    assert!(
        r.penetration_max_mm <= 2.0 && r.penetration_p99_mm <= 1.0,
        "poke-through"
    );
    assert!(
        !r.open_stitches && r.seam_gap_max_mm == 0.0,
        "seams welded shut"
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
    let hash = || {
        let (mut solver, _, _) = cloth();
        for _ in 0..90 {
            solver.step(Some(collider()));
        }
        position_hash(solver.cloth())
    };
    assert_eq!(hash(), hash());
}
