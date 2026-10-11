//! Milliseconds per simulated frame, phase by phase, with the drape's quality beside them.
//!
//! `cargo run --release -p opendrape-testkit --example drape_bench -- [options]`
//!
//! - `--scene skirt|tube|drafted-skirt|drafted-tshirt|all` (default all)
//! - `--seconds 4` simulated seconds per scene
//! - `--quality draft|normal|fine` a fabric detail preset for the drafted scenes (what the
//!   app drapes at); without it, the solver's defaults and the flags below
//! - `--edge-mm 12` fabric edge length for the drafted scenes
//! - `--substeps 20`, `--iterations 2`, `--seam-compliance 0.01` for the drafted scenes
//! - `--threads 1` the rayon pool's size (1 stands in for a slow laptop)
//!
//! Each scene prints a human line, its phases, and one `BENCH` line for scripts.

use opendrape_drape::{Drape, DrapeQuality, Stage};
use opendrape_mesh::MeshParams;
use opendrape_sim::{FRAME_DT, Params, PhaseTimes, Solid, Solver};
use opendrape_testkit::drafted;
use opendrape_testkit::garments::{Garment, Scene};
use opendrape_testkit::metrics::{cloth_crossings, measure, seam_crease_deg};
use std::sync::Arc;
use std::time::Instant;

struct Opts {
    scenes: Vec<String>,
    seconds: f64,
    quality: Option<DrapeQuality>,
    edge_mm: Option<f64>,
    substeps: Option<usize>,
    iterations: Option<usize>,
    seam_compliance: Option<f64>,
    stretch_compliance: Option<f64>,
    threads: Option<usize>,
}

fn parse() -> Opts {
    let mut o = Opts {
        scenes: vec!["all".into()],
        seconds: 4.0,
        quality: None,
        edge_mm: None,
        substeps: None,
        iterations: None,
        seam_compliance: None,
        stretch_compliance: None,
        threads: None,
    };
    let mut args = std::env::args().skip(1);
    while let Some(flag) = args.next() {
        let mut value = || {
            args.next()
                .unwrap_or_else(|| panic!("{flag} needs a value"))
        };
        match flag.as_str() {
            "--scene" => o.scenes = vec![value()],
            "--seconds" => o.seconds = value().parse().expect("seconds"),
            "--quality" => {
                o.quality = Some(match value().as_str() {
                    "draft" => DrapeQuality::Draft,
                    "normal" => DrapeQuality::Normal,
                    "fine" => DrapeQuality::Fine,
                    other => panic!("unknown quality {other}"),
                })
            }
            "--edge-mm" => o.edge_mm = Some(value().parse().expect("edge mm")),
            "--substeps" => o.substeps = Some(value().parse().expect("substeps")),
            "--iterations" => o.iterations = Some(value().parse().expect("iterations")),
            "--seam-compliance" => {
                o.seam_compliance = Some(value().parse().expect("seam compliance"))
            }
            "--stretch-compliance" => {
                o.stretch_compliance = Some(value().parse().expect("stretch compliance"))
            }
            "--threads" => o.threads = Some(value().parse().expect("threads")),
            other => panic!("unknown option {other}"),
        }
    }
    o
}

/// Steps `step` for `seconds`: wall-clock ms per frame and the mean phase times.
fn drive(seconds: f64, mut step: impl FnMut() -> PhaseTimes) -> (f64, PhaseTimes) {
    let frames = (seconds / FRAME_DT).round().max(1.0) as usize;
    let mut acc = PhaseTimes::default();
    let start = Instant::now();
    for _ in 0..frames {
        acc.add(&step());
    }
    let ms = start.elapsed().as_secs_f64() * 1000.0 / frames as f64;
    (ms, acc.scaled(1.0 / frames as f64))
}

fn report(name: &str, ms: f64, avg: PhaseTimes, solver: &Solver, solid: &dyn Solid) {
    let cloth = solver.cloth();
    let r = measure(cloth, solid);
    let crossings = cloth_crossings(cloth);
    let crease = seam_crease_deg(cloth);
    let (pairs, builds) = solver.self_contact_stats();
    println!(
        "{name}: {} particles, {ms:.2} ms/frame, penetration max {:.2} mm, strain p99 {:.1}% \
         (bias {:.1}%), {crossings} crossings, seam crease {}, {pairs} contact pairs found \
         {builds} times",
        r.particles,
        r.penetration_max_mm,
        r.strain_p99 * 100.0,
        r.shear_p99 * 100.0,
        crease.map_or("n/a".to_string(), |c| format!("{c:.1}°")),
    );
    let phases: Vec<String> = PhaseTimes::NAMES
        .iter()
        .zip(avg.values())
        .filter(|(_, v)| *v > 0.0005)
        .map(|(n, v)| format!("{n} {v:.2}"))
        .collect();
    println!("  phases (ms): {}", phases.join(" | "));
    println!(
        "BENCH scene={name} particles={} ms={ms:.3} penetration_mm={:.3} strain_p99={:.4} \
         shear_p99={:.4} crossings={crossings} crease_deg={:.2} ke={:.2e}",
        r.particles,
        r.penetration_max_mm,
        r.strain_p99,
        r.shear_p99,
        crease.unwrap_or(-1.0),
        r.kinetic_energy
    );
}

fn main() {
    let o = parse();
    if let Some(n) = o.threads {
        rayon::ThreadPoolBuilder::new()
            .num_threads(n.max(1))
            .build_global()
            .expect("the rayon pool is built once");
    }
    let defaults = Params::default();
    let params = Params {
        substeps: o.substeps.unwrap_or(defaults.substeps),
        iterations: o.iterations.unwrap_or(defaults.iterations),
        seam_compliance: o.seam_compliance.unwrap_or(defaults.seam_compliance),
        stretch_compliance: o.stretch_compliance.unwrap_or(defaults.stretch_compliance),
        ..defaults
    };
    let mesh = MeshParams {
        edge_mm: o.edge_mm.unwrap_or(MeshParams::default().edge_mm),
        ..MeshParams::default()
    };
    let all = ["skirt", "tube", "drafted-skirt", "drafted-tshirt"];
    let scenes: Vec<&str> = if o.scenes.iter().any(|s| s == "all") {
        all.to_vec()
    } else {
        o.scenes.iter().map(String::as_str).collect()
    };
    for name in scenes {
        match name {
            // The demo scenes have their own params and 12 mm grids: the flags don't apply.
            "skirt" | "tube" => {
                let garment = if name == "skirt" {
                    Garment::Skirt
                } else {
                    Garment::BodiceProxy
                };
                let mut scene = Scene::new(garment);
                let (ms, avg) = drive(o.seconds, || {
                    scene.step();
                    scene.solver.phase_times()
                });
                report(name, ms, avg, &scene.solver, scene.collider());
            }
            "drafted-skirt" | "drafted-tshirt" => {
                let stage = Stage::shared();
                let project = if name == "drafted-skirt" {
                    drafted::skirt(&stage)
                } else {
                    drafted::t_shirt(&stage).project
                };
                let mut drape = match o.quality {
                    Some(q) => Drape::at_quality(Arc::new(project), &stage, q),
                    None => Drape::with(Arc::new(project), &stage, &mesh, params),
                };
                let collider = stage.drape_collider();
                let (ms, avg) = drive(o.seconds, || {
                    drape.solver.step(Some(collider));
                    drape.solver.phase_times()
                });
                report(name, ms, avg, &drape.solver, collider);
            }
            other => panic!("unknown scene {other}"),
        }
    }
}
