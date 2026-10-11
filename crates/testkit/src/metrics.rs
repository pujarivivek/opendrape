use crate::garments::Scene;
use glam::DVec3;
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
    /// Fractions (0.05 = 5%) of edge stretch beyond rest length, along the warp and weft.
    pub strain_mean: f64,
    pub strain_p99: f64,
    pub strain_max: f64,
    /// The same on the bias (the cells' diagonals), which is meant to give.
    pub shear_p99: f64,
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
    let mut shear: Vec<f64> = cloth
        .shear_links()
        .map(|(a, b, r)| ((x[a] - x[b]).length() - r) / r)
        .collect();
    shear.sort_by(f64::total_cmp);
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
        shear_p99: if shear.is_empty() {
            0.0
        } else {
            pct(&shear, 0.99)
        },
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

/// How many fabric edges pass through a triangle of the fabric they share no particle with:
/// 0 for a drape that never goes through itself. Measured through a grid of triangle boxes,
/// so it is cheap enough for every test.
pub fn cloth_crossings(cloth: &Cloth) -> usize {
    cloth_crossing_pairs(cloth).len()
}

/// Each crossing [`cloth_crossings`] counts: the edge (its particles) and the triangle (its
/// index) it passes through.
pub fn cloth_crossing_pairs(cloth: &Cloth) -> Vec<((usize, usize), usize)> {
    use std::collections::{HashMap, HashSet};
    let x = cloth.positions();
    let tris = cloth.triangles();
    let edges: Vec<(usize, usize)> = cloth
        .stretch_links()
        .chain(cloth.shear_links())
        .map(|(a, b, _)| (a, b))
        .collect();
    let mut found = Vec::new();
    if edges.is_empty() || tris.is_empty() {
        return found;
    }
    let mean = cloth.stretch_links().map(|(_, _, r)| r).sum::<f64>() / edges.len() as f64;
    let cell = (2.0 * mean).max(1e-3);
    let key = |p: DVec3| {
        [
            (p.x / cell).floor() as i64,
            (p.y / cell).floor() as i64,
            (p.z / cell).floor() as i64,
        ]
    };
    let cells = |points: &[DVec3]| {
        let lo = key(points.iter().fold(DVec3::MAX, |m, p| m.min(*p)));
        let hi = key(points.iter().fold(DVec3::MIN, |m, p| m.max(*p)));
        let mut out = Vec::new();
        for i in lo[0]..=hi[0] {
            for j in lo[1]..=hi[1] {
                for k in lo[2]..=hi[2] {
                    out.push([i, j, k]);
                }
            }
        }
        out
    };
    let mut grid: HashMap<[i64; 3], Vec<u32>> = HashMap::new();
    for (t, tri) in tris.iter().enumerate() {
        let corners = tri.map(|k| x[k as usize]);
        if corners.iter().any(|p| !p.is_finite()) {
            continue;
        }
        for c in cells(&corners) {
            grid.entry(c).or_default().push(t as u32);
        }
    }
    let mut checked = HashSet::new();
    for (a, b) in edges {
        let (pa, pb) = (x[a], x[b]);
        if !(pa.is_finite() && pb.is_finite()) {
            continue;
        }
        checked.clear();
        for c in cells(&[pa, pb]) {
            for &t in grid.get(&c).map(Vec::as_slice).unwrap_or(&[]) {
                if !checked.insert(t) {
                    continue;
                }
                let tri = tris[t as usize];
                if tri.contains(&(a as u32)) || tri.contains(&(b as u32)) {
                    continue;
                }
                if segment_crosses_triangle(pa, pb, tri.map(|k| x[k as usize])) {
                    found.push(((a, b), t as usize));
                }
            }
        }
    }
    found
}

/// Whether the open segment `a`–`b` passes through the inside of the triangle (Möller–Trumbore;
/// touching an edge or an end does not count).
fn segment_crosses_triangle(a: DVec3, b: DVec3, [p, q, r]: [DVec3; 3]) -> bool {
    const EPS: f64 = 1e-9;
    let dir = b - a;
    let (e1, e2) = (q - p, r - p);
    let h = dir.cross(e2);
    let det = e1.dot(h);
    if det.abs() < EPS * EPS {
        return false;
    }
    let inv = 1.0 / det;
    let s = a - p;
    let u = s.dot(h) * inv;
    if u <= EPS || u >= 1.0 - EPS {
        return false;
    }
    let qv = s.cross(e1);
    let v = dir.dot(qv) * inv;
    if v <= EPS || u + v >= 1.0 - EPS {
        return false;
    }
    let t = e2.dot(qv) * inv;
    t > EPS && t < 1.0 - EPS
}

/// How creased the welded seams are: the mean angle (degrees) between the two triangles on
/// either side of each seam edge, 0 for seams that lie flat like continuous fabric. None
/// before anything has welded.
pub fn seam_crease_deg(cloth: &Cloth) -> Option<f64> {
    let angles = seam_fold_angles(cloth);
    (!angles.is_empty()).then(|| angles.iter().sum::<f64>() / angles.len() as f64)
}

/// The fold at each welded seam edge (degrees, 0 lying flat): the angle between the two
/// triangles either side of it, taken about the edge itself so that it reads the same
/// whichever way round each piece's triangles are wound (a piece sewn to its mirror image
/// placed without a turn faces the other way).
pub fn seam_fold_angles(cloth: &Cloth) -> Vec<f64> {
    use std::collections::HashMap;
    let seams = cloth.seam_edges();
    if seams.is_empty() {
        return Vec::new();
    }
    let x = cloth.positions();
    // For each seam edge, the vertex opposite it in each triangle that has it.
    let mut opposite: HashMap<(u32, u32), Vec<u32>> = HashMap::new();
    for t in cloth.triangles() {
        for k in 0..3 {
            let (a, b) = (t[k], t[(k + 1) % 3]);
            let key = (a.min(b), a.max(b));
            if seams.binary_search(&key).is_ok() {
                opposite.entry(key).or_default().push(t[(k + 2) % 3]);
            }
        }
    }
    seams
        .iter()
        .filter_map(|&(u, v)| match opposite.get(&(u, v)).map(Vec::as_slice) {
            Some(&[p, q]) => {
                let e = x[v as usize] - x[u as usize];
                let np = e.cross(x[p as usize] - x[u as usize]);
                let nq = (x[q as usize] - x[u as usize]).cross(e);
                let (lp, lq) = (np.length(), nq.length());
                (lp > 0.0 && lq > 0.0).then(|| {
                    (np.dot(nq) / (lp * lq))
                        .clamp(-1.0, 1.0)
                        .acos()
                        .to_degrees()
                })
            }
            _ => None,
        })
        .collect()
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

#[cfg(test)]
mod tests {
    use super::*;
    use glam::DVec2;
    use opendrape_sim::{ClothBuilder, Panel};

    /// A 10 cm square of two triangles in the xz plane at height `y`, flat in the pattern.
    fn square(y: f64) -> Panel {
        let flat = vec![
            DVec2::new(0.0, 0.0),
            DVec2::new(0.1, 0.0),
            DVec2::new(0.1, 0.1),
            DVec2::new(0.0, 0.1),
        ];
        Panel {
            positions: flat.iter().map(|p| DVec3::new(p.x, y, p.y)).collect(),
            flat: Some(flat),
            triangles: vec![[0, 1, 2], [0, 2, 3]],
        }
    }

    #[test]
    fn stacked_squares_do_not_cross_but_a_pierced_one_does() {
        let mut b = ClothBuilder::new(0.15);
        b.add_panel(&square(0.0), 1.0);
        b.add_panel(&square(0.01), 1.0);
        assert_eq!(cloth_crossings(&b.build()), 0, "one above the other");
        let mut b = ClothBuilder::new(0.15);
        b.add_panel(&square(0.0), 1.0);
        // A vertical edge through the middle of the square's first triangle.
        b.add_panel(
            &Panel {
                positions: vec![
                    DVec3::new(0.07, -0.05, 0.03),
                    DVec3::new(0.07, 0.05, 0.03),
                    DVec3::new(0.08, 0.05, 0.03),
                ],
                flat: None,
                triangles: vec![[0, 1, 2]],
            },
            1.0,
        );
        assert!(cloth_crossings(&b.build()) >= 1, "pierced");
    }

    /// A 10 cm square in the xz plane with its left edge at `x`, flat in the pattern.
    fn side_by_side(x: f64) -> Panel {
        Panel {
            positions: vec![
                DVec3::new(x, 0.0, 0.0),
                DVec3::new(x + 0.1, 0.0, 0.0),
                DVec3::new(x + 0.1, 0.0, 0.1),
                DVec3::new(x, 0.0, 0.1),
            ],
            ..square(0.0)
        }
    }

    #[test]
    fn a_seam_welded_flat_has_no_crease_and_a_folded_one_has() {
        // Two squares side by side, sewn along the edge between them.
        let mut b = ClothBuilder::new(0.15);
        let (p, q) = (
            b.add_panel(&side_by_side(0.0), 1.0),
            b.add_panel(&side_by_side(0.1), 1.0),
        );
        b.stitch((p, 1), (q, 0));
        b.stitch((p, 2), (q, 3));
        let mut c = b.build();
        assert_eq!(seam_crease_deg(&c), None, "nothing welded yet");
        c.weld_stitches();
        assert!(seam_crease_deg(&c).unwrap() < 1e-9, "flat");
        // The second square folded straight up: a right-angle crease.
        let mut b = ClothBuilder::new(0.15);
        let p = b.add_panel(&side_by_side(0.0), 1.0);
        let q = b.add_panel(
            &Panel {
                positions: vec![
                    DVec3::new(0.1, 0.0, 0.0),
                    DVec3::new(0.1, 0.1, 0.0),
                    DVec3::new(0.1, 0.1, 0.1),
                    DVec3::new(0.1, 0.0, 0.1),
                ],
                ..square(0.0)
            },
            1.0,
        );
        b.stitch((p, 1), (q, 0));
        b.stitch((p, 2), (q, 3));
        let mut c = b.build();
        c.weld_stitches();
        let crease = seam_crease_deg(&c).unwrap();
        assert!((crease - 90.0).abs() < 1e-6, "{crease}");
    }

    #[test]
    fn a_seam_reads_flat_however_the_two_pieces_are_wound() {
        // The second square's triangles wound the other way (its mirror image, as a twin
        // placed without a turn is): flat is still 0°.
        let mut b = ClothBuilder::new(0.15);
        let p = b.add_panel(&side_by_side(0.0), 1.0);
        let q = b.add_panel(
            &Panel {
                triangles: vec![[0, 2, 1], [0, 3, 2]],
                ..side_by_side(0.1)
            },
            1.0,
        );
        b.stitch((p, 1), (q, 0));
        b.stitch((p, 2), (q, 3));
        let mut c = b.build();
        c.weld_stitches();
        assert!(seam_crease_deg(&c).unwrap() < 1e-9, "flat");
    }
}
