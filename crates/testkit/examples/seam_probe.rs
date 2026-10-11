//! `cargo run --release -p opendrape-testkit --example seam_probe [-- tshirt | path.odp]`:
//! what the hinges across a garment's seams look like as a drape runs (the drafted skirt, the
//! drafted T-shirt, or a saved project), as the angle between the two triangles at each (0°
//! is flat), and how far the fabric steps up at the seam compared with the fabric either
//! side of it.
use opendrape_drape::{Drape, DrapeQuality, Stage};
use opendrape_sim::Solid;
use opendrape_testkit::drafted;
use opendrape_testkit::metrics::{cloth_crossings, seam_fold_angles};
use std::sync::Arc;

fn main() {
    let what = std::env::args().nth(1).unwrap_or("skirt".into());
    let stage = Stage::shared();
    let project = match what.as_str() {
        "tshirt" => drafted::t_shirt(&stage).project,
        "skirt" => drafted::skirt(&stage),
        path => opendrape_io::load(std::path::Path::new(path)).expect("a saved project"),
    };
    println!(
        "{} pieces, {} seams, {} pins",
        project.pieces.len(),
        project.seams.len(),
        project.pins.len()
    );
    // Optional further arguments: the fabric's stretch compliance, e.g. `1e-4`, and the
    // friction against the body, e.g. `0.1`.
    let mut params = DrapeQuality::Normal.params();
    if let Some(c) = std::env::args().nth(2) {
        params.stretch_compliance = c.parse().expect("a stretch compliance");
    }
    if let Some(f) = std::env::args().nth(3) {
        params.friction = f.parse().expect("a friction");
    }
    if let Some(s) = std::env::args().nth(4) {
        params.seam_compliance = s.parse().expect("a seam compliance");
    }
    let mut drape = Drape::with(
        Arc::new(project),
        &stage,
        &DrapeQuality::Normal.mesh_params(),
        params,
    );
    println!(
        "{} particles, notes {:?}",
        drape.solver.cloth().len(),
        drape.notes
    );
    let collider = stage.drape_collider();
    for f in 0..300 {
        drape.solver.step(Some(collider));
        let notes = drape.take_notes();
        if !notes.is_empty() {
            println!("frame {f}: {notes:?}");
        }
        if f % 15 == 14 && drape.solver.cloth().has_open_stitches() {
            let gaps: Vec<String> = drape
                .solver
                .cloth()
                .open_seam_gaps()
                .iter()
                .map(|(g, d)| format!("seam {g}: {:.1} mm", d * 1000.0))
                .collect();
            println!("frame {f}: open {}", gaps.join(", "));
        }
        if f == 35 || f == 60 || f == 120 || f == 299 {
            report(&drape, f, collider);
        }
    }
}

fn report(drape: &Drape, frame: usize, body: &dyn Solid) {
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
    let _ = sides;
    let mut angles = seam_fold_angles(c);
    angles.sort_by(f64::total_cmp);
    let pct = |v: &[f64], q: f64| {
        if v.is_empty() {
            f64::NAN
        } else {
            v[((v.len() - 1) as f64 * q) as usize]
        }
    };
    // The step at the seam: how far off the body a seam particle sits, less the mean of its
    // neighbours' that are not on the seam (positive: the seam stands proud).
    let mut on_seam = vec![false; c.len()];
    for &(a, b) in seams {
        on_seam[a as usize] = true;
        on_seam[b as usize] = true;
    }
    let mut ring: Vec<Vec<usize>> = vec![Vec::new(); c.len()];
    for (a, b, _) in c.stretch_links().chain(c.shear_links()) {
        ring[a].push(b);
        ring[b].push(a);
    }
    let mut steps: Vec<f64> = (0..c.len())
        .filter(|&i| on_seam[i] && c.is_alive(i))
        .filter_map(|i| {
            let off: Vec<f64> = ring[i]
                .iter()
                .filter(|&&j| !on_seam[j])
                .map(|&j| body.signed_distance(x[j]))
                .collect();
            (!off.is_empty()).then(|| {
                (body.signed_distance(x[i]) - off.iter().sum::<f64>() / off.len() as f64) * 1000.0
            })
        })
        .collect();
    steps.sort_by(f64::total_cmp);
    println!(
        "frame {frame}: {} seam hinges ({} held); fold p50 {:.1}° p90 {:.1}° max {:.1}°; step at the \
         seam (mm) p50 {:.2} p90 {:.2} max {:.2}; crossings {}; open stitches {}",
        angles.len(),
        c.seam_hinge_count(),
        pct(&angles, 0.5),
        pct(&angles, 0.9),
        pct(&angles, 1.0),
        if steps.is_empty() {
            0.0
        } else {
            pct(&steps, 0.5)
        },
        if steps.is_empty() {
            0.0
        } else {
            pct(&steps, 0.9)
        },
        if steps.is_empty() {
            0.0
        } else {
            pct(&steps, 1.0)
        },
        cloth_crossings(c),
        c.has_open_stitches()
    );
}
