//! `cargo run --release -p opendrape-testkit --example drape_bench`: ms per simulated frame, on the default dress form.
use opendrape_testkit::garments::{Garment, Scene};
use opendrape_testkit::metrics::{measure, run};

fn main() {
    for g in Garment::ALL {
        let mut scene = Scene::new(g);
        let ms = run(&mut scene, 4.0);
        let r = measure(scene.solver.cloth(), scene.collider());
        println!(
            "{g:?}: {} particles, {ms:.2} ms/frame, penetration max {:.2} mm, strain p99 {:.1}%",
            r.particles,
            r.penetration_max_mm,
            r.strain_p99 * 100.0
        );
    }
}
