use opendrape_testkit::garments::{BODICE_PARTICLES, Garment, SKIRT_PARTICLES, Scene, collider};
use opendrape_testkit::metrics::{measure, position_hash, run};

#[test]
fn skirt_drapes_without_poking_through_and_settles() {
    let mut scene = Scene::new(Garment::Skirt);
    assert_eq!(scene.solver.cloth().len(), SKIRT_PARTICLES);
    run(&mut scene, 6.0);
    let r = measure(scene.solver.cloth(), collider());
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
        r.strain_p99 <= 0.10 && r.strain_max <= 0.20,
        "fabric over-stretched"
    );
    assert!(
        r.kinetic_energy <= 1e-4,
        "still moving: {} J",
        r.kinetic_energy
    );
    assert!(
        r.lowest_y > 0.4 && r.highest_y > 1.0,
        "skirt slid down: {}..{}",
        r.lowest_y,
        r.highest_y
    );
}

#[test]
fn fitted_tube_holds_close_to_the_body_without_poke_through() {
    let mut scene = Scene::new(Garment::BodiceProxy);
    assert_eq!(scene.solver.cloth().len(), BODICE_PARTICLES);
    run(&mut scene, 4.0);
    let r = measure(scene.solver.cloth(), collider());
    eprintln!("{r:#?}");
    assert!(!r.has_nan);
    assert!(r.penetration_max_mm <= 2.0, "poke-through under tension");
    assert!(r.strain_p99 <= 0.05, "tube over-stretched");
    assert!(
        r.kinetic_energy <= 1e-6,
        "still moving: {} J",
        r.kinetic_energy
    );
}

#[test]
fn drape_is_deterministic() {
    let hash = || {
        let mut s = Scene::new(Garment::Skirt);
        run(&mut s, 1.5);
        position_hash(s.solver.cloth())
    };
    assert_eq!(hash(), hash());
}
