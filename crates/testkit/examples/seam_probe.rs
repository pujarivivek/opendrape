//! `cargo run --release -p opendrape-testkit --example seam_probe [-- tshirt]`: what the
//! hinges across the drafted skirt's (or T-shirt's) seams look like as a drape runs, as the
//! angle between the two triangles at each (0° is flat).
use opendrape_drape::{Drape, DrapeQuality, Stage};
use opendrape_testkit::drafted;
use std::sync::Arc;

fn main() {
    let scene = std::env::args().nth(1).unwrap_or("skirt".into());
    let stage = Stage::shared();
    let project = if scene == "tshirt" {
        drafted::t_shirt(&stage).project
    } else {
        drafted::skirt(&stage)
    };
    let mut drape = Drape::at_quality(Arc::new(project), &stage, DrapeQuality::Normal);
    let collider = stage.drape_collider();
    for f in 0..240 {
        drape.solver.step(Some(collider));
        if f == 35 || f == 60 || f == 120 || f == 239 {
            report(&drape, f);
        }
    }
}

fn report(drape: &Drape, frame: usize) {
    let c = drape.solver.cloth();
    let x = c.positions();
    // Seam edges and the two triangles either side of each.
    let seams = c.seam_edges();
    let mut sides: std::collections::HashMap<(u32, u32), Vec<usize>> =
        std::collections::HashMap::new();
    for (t, tri) in c.triangles().iter().enumerate() {
        for k in 0..3 {
            let (a, b) = (tri[k], tri[(k + 1) % 3]);
            let key = (a.min(b), a.max(b));
            if seams.binary_search(&key).is_ok() {
                sides.entry(key).or_default().push(t);
            }
        }
    }
    let normal = |t: usize| {
        let tri = c.triangles()[t];
        (x[tri[1] as usize] - x[tri[0] as usize])
            .cross(x[tri[2] as usize] - x[tri[0] as usize])
            .normalize()
    };
    let mut angles: Vec<f64> = sides
        .values()
        .filter(|v| v.len() == 2)
        .map(|v| {
            normal(v[0])
                .dot(normal(v[1]))
                .clamp(-1.0, 1.0)
                .acos()
                .to_degrees()
        })
        .collect();
    angles.sort_by(f64::total_cmp);
    let pct = |q: f64| angles[((angles.len() - 1) as f64 * q) as usize];
    println!(
        "frame {frame}: {} seam hinges ({} held); angle between triangles p50 {:.1}° p90 {:.1}° \
         max {:.1}°; open stitches {}",
        angles.len(),
        c.seam_hinge_count(),
        pct(0.5),
        pct(0.9),
        pct(1.0),
        c.has_open_stitches()
    );
}
