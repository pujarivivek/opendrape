//! Cloth keeps off cloth: a square held level by its four corners catches a square dropped on
//! it, which falls straight through to the floor without self-collision.

use opendrape_core::{Half, Piece, PieceId, Pin, Placement, Point2, Project};
use opendrape_drape::{Drape, Stage};
use opendrape_mesh::MeshParams;
use opendrape_sim::{FRAME_DT, Params};
use opendrape_testkit::metrics::{cloth_crossing_pairs, cloth_crossings, measure};
use std::sync::Arc;

/// Height (m) the held square's corners are pinned at.
const HELD_Y: f64 = 0.3;

/// Two 20 cm squares 40 cm in front of the form: one turned level (a quarter turn about x)
/// and pinned by its corners at [`HELD_Y`], and one level too, 15 cm above it.
fn held_and_dropped() -> Project {
    let mut pr = Project::new();
    let held = pr.add_piece(Piece::rectangle(
        PieceId(0),
        "Held",
        Point2::new(0.0, 0.0),
        200.0,
        200.0,
    ));
    let dropped = pr.add_piece(Piece::rectangle(
        PieceId(0),
        "Dropped",
        Point2::new(300.0, 0.0),
        200.0,
        200.0,
    ));
    let half = std::f64::consts::FRAC_1_SQRT_2;
    let level = |y: f64| Placement {
        rotation: [half, 0.0, 0.0, half],
        ..Placement::at([0.0, y, 0.4])
    };
    assert!(pr.set_placement(held, Some(level(HELD_Y))));
    assert!(pr.set_placement(dropped, Some(level(HELD_Y + 0.15))));
    // The quarter turn about x takes the piece's +y (up the table) to +z: its corners land
    // 10 cm either side of x = 0 and of z = 0.4.
    for (x, y) in [(0.0, 0.0), (200.0, 0.0), (200.0, 200.0), (0.0, 200.0)] {
        pr.pins.push(Pin {
            shape: held,
            half: Half::Drawn,
            at: Point2::new(x, y),
            target: [x / 1000.0 - 0.1, HELD_Y, 0.3 + y / 1000.0],
        });
    }
    assert_eq!(pr.check(), Ok(()));
    pr
}

/// The lowest point of the dropped square after 3 s, and whether anything crosses.
fn dropped_lands_at(self_collision: bool) -> (f64, usize) {
    let stage = Stage::shared();
    let params = Params {
        self_collision,
        gravity_delay: 0.0,
        gravity_ramp: 0.0,
        ..Params::default()
    };
    let mut drape = Drape::with(
        Arc::new(held_and_dropped()),
        &stage,
        &MeshParams::default(),
        params,
    );
    let collider = stage.drape_collider();
    for _ in 0..(3.0 / FRAME_DT).round() as usize {
        drape.solver.step(Some(collider));
    }
    let cloth = drape.solver.cloth();
    let r = measure(cloth, collider);
    assert!(!r.has_nan);
    assert!(r.penetration_max_mm <= 2.0, "through the floor");
    let panel = &drape.fabric.panels[1];
    let x = cloth.positions();
    let lowest = (panel.first_particle..panel.first_particle + panel.flat.len())
        .filter(|&i| cloth.is_alive(i))
        .map(|i| x[i].y)
        .fold(f64::MAX, f64::min);
    let panel_of = |i: usize| usize::from(i >= panel.first_particle);
    let mut by_pair = [[0usize; 2]; 2];
    for ((a, _), t) in cloth_crossing_pairs(cloth) {
        let (pa, pt) = (panel_of(a), panel_of(cloth.triangles()[t][0] as usize));
        by_pair[pa.min(pt)][pa.max(pt)] += 1;
    }
    eprintln!(
        "crossings held×held {}, held×dropped {}, dropped×dropped {}",
        by_pair[0][0], by_pair[0][1], by_pair[1][1]
    );
    (lowest, cloth_crossings(cloth))
}

#[test]
fn a_dropped_square_is_caught_by_a_held_one_instead_of_falling_through() {
    let (caught, crossings) = dropped_lands_at(true);
    let (fell, _) = dropped_lands_at(false);
    eprintln!("lowest point: {caught:.3} m caught, {fell:.3} m without self-collision");
    // The held square, pinned only at its corners, sags a few centimetres under the weight.
    assert!(caught > HELD_Y - 0.08, "it fell through: {caught}");
    assert_eq!(crossings, 0);
    assert!(fell < 0.05, "the test shows nothing: {fell}");
}
