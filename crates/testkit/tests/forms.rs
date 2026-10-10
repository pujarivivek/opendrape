use opendrape_body::form::{BuiltForm, Chart, Form, Quality};
use opendrape_testkit::forms;
use opendrape_testkit::garments::{Garment, Scene};
use opendrape_testkit::metrics::{DrapeReport, measure, position_hash, run, run_solver};
use std::time::Instant;

#[derive(Clone, Copy)]
enum Which {
    Smallest,
    Middle,
    Largest,
}

fn built(chart: &str, which: Which) -> BuiltForm {
    let chart = Chart::bundled()
        .into_iter()
        .find(|c| c.id == chart)
        .expect("chart");
    let size = match which {
        Which::Smallest => &chart.sizes[0],
        Which::Middle => &chart.sizes[chart.sizes.len() / 2],
        Which::Largest => chart.sizes.last().unwrap(),
    };
    Form::bundled(&chart.form)
        .expect("form")
        .build(&size.mm, Quality::Standard)
        .expect("chart sizes fit")
}

/// One line per drape, for the results table (run with `--no-capture` to see it).
fn row(name: &str, r: &DrapeReport, started: Instant) {
    eprintln!("{r:#?}");
    eprintln!(
        "ROW {name} | pen {:.3}/{:.3} mm | strain {:.4}/{:.4} | KE {:.3e} J | y {:.3}..{:.3} | {:.1} s",
        r.penetration_max_mm,
        r.penetration_p99_mm,
        r.strain_p99,
        r.strain_max,
        r.kinetic_energy,
        r.lowest_y,
        r.highest_y,
        started.elapsed().as_secs_f64()
    );
}

/// The body skirt's gates, with "stayed up" measured from this form's waist.
fn skirt_gates(chart: &str, which: Which) {
    let started = Instant::now();
    let form = built(chart, which);
    let mut scene = Scene::new_on(Garment::Skirt, &form);
    run(&mut scene, 6.0);
    let r = measure(scene.solver.cloth(), scene.collider());
    row(&format!("skirt {chart} {}", label(which)), &r, started);
    let waist = form.stations["waist"];
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
        r.highest_y > waist - 0.05 && r.lowest_y > waist - 0.65,
        "skirt slid down: {}..{}",
        r.lowest_y,
        r.highest_y
    );
}

/// The body tube's gates.
fn tube_gates(chart: &str, which: Which) {
    let started = Instant::now();
    let form = built(chart, which);
    let mut scene = Scene::new_on(Garment::BodiceProxy, &form);
    run(&mut scene, 4.0);
    let r = measure(scene.solver.cloth(), scene.collider());
    row(&format!("tube {chart} {}", label(which)), &r, started);
    assert!(!r.has_nan);
    assert!(r.penetration_max_mm <= 2.0, "poke-through under tension");
    assert!(r.strain_p99 <= 0.05, "tube over-stretched");
    assert!(
        r.kinetic_energy <= 1e-6,
        "still moving: {} J",
        r.kinetic_energy
    );
}

fn label(which: Which) -> &'static str {
    match which {
        Which::Smallest => "smallest",
        Which::Middle => "middle",
        Which::Largest => "largest",
    }
}

#[test]
fn skirt_on_women_classic_smallest() {
    skirt_gates("women-torso-classic", Which::Smallest)
}
#[test]
fn skirt_on_women_classic_middle() {
    skirt_gates("women-torso-classic", Which::Middle)
}
#[test]
fn skirt_on_women_classic_largest() {
    skirt_gates("women-torso-classic", Which::Largest)
}
#[test]
fn skirt_on_women_everyday_smallest() {
    skirt_gates("women-torso-everyday", Which::Smallest)
}
#[test]
fn skirt_on_women_everyday_middle() {
    skirt_gates("women-torso-everyday", Which::Middle)
}
#[test]
fn skirt_on_women_everyday_largest() {
    skirt_gates("women-torso-everyday", Which::Largest)
}
#[test]
fn tube_on_women_classic_smallest() {
    tube_gates("women-torso-classic", Which::Smallest)
}
#[test]
fn tube_on_women_classic_middle() {
    tube_gates("women-torso-classic", Which::Middle)
}
#[test]
fn tube_on_women_classic_largest() {
    tube_gates("women-torso-classic", Which::Largest)
}
#[test]
fn tube_on_women_everyday_smallest() {
    tube_gates("women-torso-everyday", Which::Smallest)
}
#[test]
fn tube_on_women_everyday_middle() {
    tube_gates("women-torso-everyday", Which::Middle)
}
#[test]
fn tube_on_women_everyday_largest() {
    tube_gates("women-torso-everyday", Which::Largest)
}
#[test]
fn tube_on_men_classic_smallest() {
    tube_gates("men-torso-classic", Which::Smallest)
}
#[test]
fn tube_on_men_classic_middle() {
    tube_gates("men-torso-classic", Which::Middle)
}
#[test]
fn tube_on_men_classic_largest() {
    tube_gates("men-torso-classic", Which::Largest)
}

#[test]
fn a_long_hem_rests_on_the_floor() {
    let started = Instant::now();
    let form = built("women-torso-classic", Which::Middle);
    let collider = forms::collider(&form);
    let mut solver = forms::long_hem(&form);
    run_solver(&mut solver, &collider, 5.0);
    let r = measure(solver.cloth(), &collider);
    row("long hem women-torso-classic middle", &r, started);
    assert!(!r.has_nan);
    assert!(
        r.penetration_max_mm <= 2.0,
        "poke-through into form or floor"
    );
    assert!(
        r.lowest_y >= -0.0005,
        "fell through the floor: {}",
        r.lowest_y
    );
    assert!(
        r.lowest_y <= 0.01,
        "the hem should reach the floor: {}",
        r.lowest_y
    );
}

#[test]
fn form_drape_is_deterministic() {
    let form = built("women-torso-classic", Which::Middle);
    let hash = || {
        let mut s = Scene::new_on(Garment::Skirt, &form);
        run(&mut s, 1.5);
        position_hash(s.solver.cloth())
    };
    assert_eq!(hash(), hash());
}
