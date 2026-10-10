use crate::garments::Scene;
use opendrape_sim::{Cloth, Solid, Solver};
use rayon::prelude::*;

#[derive(Clone, Debug, Default)]
pub struct DrapeReport {
    pub particles: usize,
    pub penetration_max_mm: f64,
    pub penetration_p99_mm: f64,
    pub particles_inside: usize,
    pub open_stitches: bool,
    pub seam_gap_max_mm: f64,
    /// Fractions (0.05 = 5%) of edge stretch beyond rest length.
    pub strain_mean: f64,
    pub strain_p99: f64,
    pub strain_max: f64,
    pub kinetic_energy: f64,
    pub lowest_y: f64,
    pub highest_y: f64,
    pub has_nan: bool,
}

pub fn measure(cloth: &Cloth, collider: &dyn Solid) -> DrapeReport {
    let x = cloth.positions();
    let live: Vec<usize> = (0..cloth.len()).filter(|&i| cloth.is_alive(i)).collect();
    let has_nan = live.iter().any(|&i| !x[i].is_finite());
    let pct = |v: &[f64], q: f64| v[((v.len() - 1) as f64 * q).round() as usize];
    let mut pen: Vec<f64> = if has_nan {
        vec![f64::NAN]
    } else {
        live.par_iter()
            .map(|&i| (-collider.signed_distance(x[i])).max(0.0))
            .collect()
    };
    pen.sort_by(f64::total_cmp);
    let mut strain: Vec<f64> = cloth
        .stretch_links()
        .map(|(a, b, r)| ((x[a] - x[b]).length() - r) / r)
        .collect();
    strain.sort_by(f64::total_cmp);
    let (lowest_y, highest_y) = live.iter().fold((f64::MAX, f64::MIN), |(lo, hi), &i| {
        (lo.min(x[i].y), hi.max(x[i].y))
    });
    DrapeReport {
        particles: live.len(),
        penetration_max_mm: pen[pen.len() - 1] * 1000.0,
        penetration_p99_mm: pct(&pen, 0.99) * 1000.0,
        particles_inside: pen.iter().filter(|d| **d > 0.0).count(),
        open_stitches: cloth.has_open_stitches(),
        seam_gap_max_mm: cloth
            .stitch_pairs()
            .map(|(a, b)| (x[a] - x[b]).length())
            .fold(0.0, f64::max)
            * 1000.0,
        strain_mean: strain.iter().sum::<f64>() / strain.len() as f64,
        strain_p99: pct(&strain, 0.99),
        strain_max: strain[strain.len() - 1],
        kinetic_energy: cloth.kinetic_energy(),
        lowest_y,
        highest_y,
        has_nan,
    }
}

/// FNV-1a over the bits of every live particle position.
pub fn position_hash(cloth: &Cloth) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for (i, p) in cloth.positions().iter().enumerate() {
        if cloth.is_alive(i) {
            for c in p.to_array() {
                h = (h ^ c.to_bits()).wrapping_mul(0x0100_0000_01b3);
            }
        }
    }
    h
}

/// Steps `scene` for `seconds` of simulated time; returns wall-clock ms per frame.
pub fn run(scene: &mut Scene, seconds: f64) -> f64 {
    let frames = (seconds / opendrape_sim::FRAME_DT).round() as usize;
    let start = std::time::Instant::now();
    for _ in 0..frames {
        scene.step();
    }
    start.elapsed().as_secs_f64() * 1000.0 / frames.max(1) as f64
}

/// Steps `solver` against `collider` for `seconds`; returns wall-clock ms per frame.
pub fn run_solver(solver: &mut Solver, collider: &dyn Solid, seconds: f64) -> f64 {
    let frames = (seconds / opendrape_sim::FRAME_DT).round() as usize;
    let start = std::time::Instant::now();
    for _ in 0..frames {
        solver.step(Some(collider));
    }
    start.elapsed().as_secs_f64() * 1000.0 / frames.max(1) as f64
}
