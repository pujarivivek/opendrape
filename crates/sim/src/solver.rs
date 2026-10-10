use crate::cloth::{Cloth, Link};
use crate::collide::{Collider, Plane};
use crate::timing::{Lap, PhaseTimes};
use glam::DVec3;

/// One simulation frame.
pub const FRAME_DT: f64 = 1.0 / 60.0;

/// Simulation settings. `Default` is the set validated by the M1 prototype.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Params {
    pub substeps: usize,
    pub iterations: usize,
    /// m/s² along Y.
    pub gravity: f64,
    /// Seconds without gravity at the start, so seams can close first.
    pub gravity_delay: f64,
    /// Seconds over which gravity then ramps to full.
    pub gravity_ramp: f64,
    /// Seconds over which stitched seams pull shut.
    pub stitch_close_time: f64,
    /// When to weld closed seams (None: never).
    pub weld_time: Option<f64>,
    /// XPBD compliance (m/N) of fabric edges and of bending.
    pub stretch_compliance: f64,
    pub bend_compliance: f64,
    /// Velocity damping per second.
    pub damping: f64,
    pub friction: f64,
    /// Distance kept between cloth and body (m).
    pub thickness: f64,
    /// Contact planes are created for particles within this distance of the body (m).
    pub collision_margin: f64,
    pub max_speed: f64,
}

impl Default for Params {
    fn default() -> Self {
        Self {
            substeps: 20,
            iterations: 2,
            gravity: -9.81,
            gravity_delay: 0.6,
            gravity_ramp: 0.3,
            stitch_close_time: 0.5,
            weld_time: Some(0.8),
            stretch_compliance: 1e-6,
            bend_compliance: 1.0,
            damping: 1.0,
            friction: 0.4,
            thickness: 0.003,
            collision_margin: 0.05,
            max_speed: 2.0,
        }
    }
}

pub struct Solver {
    cloth: Cloth,
    params: Params,
    time: f64,
    phases: PhaseTimes,
}

impl Solver {
    pub fn new(cloth: Cloth, params: Params) -> Self {
        Self {
            cloth,
            params,
            time: 0.0,
            phases: PhaseTimes::default(),
        }
    }
    pub fn cloth(&self) -> &Cloth {
        &self.cloth
    }
    /// The cloth, to attach points of it to targets (see [`Cloth::attach`]).
    pub fn cloth_mut(&mut self) -> &mut Cloth {
        &mut self.cloth
    }
    pub fn params(&self) -> &Params {
        &self.params
    }
    /// Simulated seconds so far.
    pub fn time(&self) -> f64 {
        self.time
    }
    /// Where the last frame's time went.
    pub fn phase_times(&self) -> PhaseTimes {
        self.phases
    }

    /// Advances one [`FRAME_DT`] frame.
    pub fn step(&mut self, collider: Option<&dyn Collider>) {
        let p = self.params;
        let t = self.time;
        let mut ph = PhaseTimes::default();
        let mut lap = Lap::start();
        if p.weld_time.is_some_and(|tw| t >= tw) && self.cloth.has_open_stitches() {
            self.cloth.weld_stitches();
        }
        lap.lap(&mut ph.weld);
        let gravity = if t < p.gravity_delay {
            0.0
        } else if p.gravity_ramp <= 0.0 {
            p.gravity
        } else {
            ((t - p.gravity_delay) / p.gravity_ramp).min(1.0) * p.gravity
        };
        let stitch_scale = if p.stitch_close_time > 0.0 {
            (1.0 - t / p.stitch_close_time).max(0.0)
        } else {
            0.0
        };
        let planes = collider.map(|c| c.contact_planes(&self.cloth.x, p.collision_margin));
        lap.lap(&mut ph.body_query);
        let sdt = FRAME_DT / p.substeps as f64;
        let c = &mut self.cloth;
        for _ in 0..p.substeps {
            for i in 0..c.x.len() {
                if c.inv_mass[i] == 0.0 {
                    c.prev[i] = c.x[i];
                    continue;
                }
                let mut v = c.v[i];
                v.y += gravity * sdt;
                v *= 1.0 - p.damping * sdt;
                let speed = v.length();
                if speed > p.max_speed {
                    v *= p.max_speed / speed;
                }
                c.prev[i] = c.x[i];
                c.x[i] += v * sdt;
            }
            lap.lap(&mut ph.predict);
            // Stitches first, fabric last: the fabric constraints get the final word.
            for _ in 0..p.iterations {
                solve_links(&mut c.x, &c.inv_mass, &c.stitches, 0.0, stitch_scale, sdt);
                lap.lap(&mut ph.stitches);
                solve_links(&mut c.x, &c.inv_mass, &c.bend, p.bend_compliance, 1.0, sdt);
                lap.lap(&mut ph.bend);
                solve_links(
                    &mut c.x,
                    &c.inv_mass,
                    &c.stretch,
                    p.stretch_compliance,
                    1.0,
                    sdt,
                );
                lap.lap(&mut ph.stretch);
                // Held points last of all, so a pin holds exactly.
                crate::attach::solve(c, sdt);
                lap.lap(&mut ph.attach);
            }
            if let Some(planes) = &planes {
                collide(c, planes, p.thickness, p.friction);
            }
            lap.lap(&mut ph.collide);
            for i in 0..c.x.len() {
                if c.inv_mass[i] > 0.0 {
                    c.v[i] = (c.x[i] - c.prev[i]) / sdt;
                }
            }
            lap.lap(&mut ph.velocity);
        }
        self.time += FRAME_DT;
        self.phases = ph;
    }
}

/// One Gauss–Seidel pass of XPBD distance constraints (Macklin, Müller, Chentanez 2016).
fn solve_links(
    x: &mut [DVec3],
    w: &[f64],
    links: &[Link],
    compliance: f64,
    rest_scale: f64,
    sdt: f64,
) {
    let alpha = compliance / (sdt * sdt);
    for l in links {
        let (a, b) = (l.a as usize, l.b as usize);
        let wsum = w[a] + w[b];
        if wsum == 0.0 {
            continue;
        }
        let d = x[a] - x[b];
        let len = d.length();
        if len < 1e-12 {
            continue;
        }
        let lambda = -(len - l.rest * rest_scale) / (wsum + alpha);
        let corr = d * (lambda / len);
        x[a] += corr * w[a];
        x[b] -= corr * w[b];
    }
}

/// Pushes particles out to `thickness` above their contact plane, then applies Coulomb-style
/// friction to this substep's sliding (static when the slide is small).
fn collide(c: &mut Cloth, planes: &[Option<Plane>], thickness: f64, friction: f64) {
    for (i, plane) in planes.iter().enumerate() {
        let Some(pl) = plane else { continue };
        if c.inv_mass[i] == 0.0 {
            continue;
        }
        let depth = pl.normal.dot(c.x[i] - pl.point) - thickness;
        if depth >= 0.0 {
            continue;
        }
        c.x[i] -= pl.normal * depth;
        let moved = c.x[i] - c.prev[i];
        let slide = moved - pl.normal * pl.normal.dot(moved);
        let len = slide.length();
        let limit = friction * -depth;
        if len <= limit {
            c.x[i] -= slide;
        } else if len > 0.0 {
            c.x[i] -= slide * (limit / len);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cloth::{ClothBuilder, Panel};
    use glam::DVec2;

    fn single(scale: f64, offset: DVec3, pin: Option<u32>) -> Cloth {
        let flat = vec![
            DVec2::new(0.0, 0.0),
            DVec2::new(0.1, 0.0),
            DVec2::new(0.0, 0.1),
        ];
        let panel = Panel {
            positions: flat
                .iter()
                .map(|p| p.extend(0.0) * scale + offset)
                .collect(),
            flat: Some(flat),
            triangles: vec![[0, 1, 2]],
        };
        let mut b = ClothBuilder::new(0.15);
        let id = b.add_panel(&panel, 1.0);
        if let Some(k) = pin {
            b.pin((id, k));
        }
        b.build()
    }

    fn no_gravity() -> Params {
        Params {
            gravity: 0.0,
            ..Params::default()
        }
    }

    #[test]
    fn a_stretched_triangle_returns_to_its_rest_shape() {
        let mut s = Solver::new(single(1.1, DVec3::ZERO, None), no_gravity());
        for _ in 0..60 {
            s.step(None);
        }
        let x = s.cloth().positions();
        for (a, b, r) in s.cloth().stretch_links() {
            let err = ((x[a] - x[b]).length() - r).abs() / r;
            assert!(err < 0.01, "edge {a}-{b} off by {:.2}%", err * 100.0);
        }
    }

    #[test]
    fn free_fall_follows_gravity() {
        let params = Params {
            gravity_delay: 0.0,
            gravity_ramp: 0.0,
            damping: 0.0,
            max_speed: 1e9,
            ..Params::default()
        };
        let mut s = Solver::new(single(1.0, DVec3::ZERO, None), params);
        for _ in 0..30 {
            s.step(None);
        }
        let expected = -0.5 * 9.81 * 0.5f64.powi(2);
        let y = s.cloth().positions()[0].y;
        assert!(
            (y - expected).abs() / expected.abs() < 0.01,
            "y {y} vs {expected}"
        );
    }

    #[test]
    fn pinned_particles_never_move() {
        let mut s = Solver::new(single(1.0, DVec3::ZERO, Some(0)), Params::default());
        for _ in 0..90 {
            s.step(None);
        }
        assert_eq!(s.cloth().positions()[0], DVec3::ZERO);
        assert!(s.cloth().positions()[1].y < -0.01, "the rest swings down");
    }

    #[test]
    fn stitched_panels_close_then_weld() {
        let mut b = ClothBuilder::new(0.15);
        let flat = vec![
            DVec2::new(0.0, 0.0),
            DVec2::new(0.1, 0.0),
            DVec2::new(0.0, 0.1),
        ];
        let panel = |z: f64| Panel {
            positions: flat.iter().map(|p| p.extend(z)).collect(),
            flat: Some(flat.clone()),
            triangles: vec![[0, 1, 2]],
        };
        let (p, q) = (b.add_panel(&panel(0.0), 1.0), b.add_panel(&panel(0.1), 1.0));
        b.stitch((p, 0), (q, 0));
        b.stitch((p, 1), (q, 1));
        let mut s = Solver::new(b.build(), no_gravity());
        for _ in 0..36 {
            s.step(None); // 0.6 s: past stitch_close_time 0.5 s, so the seam has closed
        }
        let x = s.cloth().positions();
        assert!(
            s.cloth()
                .stitch_pairs()
                .all(|(a, b)| (x[a] - x[b]).length() < 1e-3)
        );
        for _ in 0..24 {
            s.step(None); // 1.0 s: past weld_time 0.8 s
        }
        assert!(!s.cloth().has_open_stitches());
        assert!(s.cloth().positions().iter().all(|p| p.is_finite()));
    }

    #[test]
    fn degenerate_triangle_does_not_produce_nan() {
        let panel = Panel {
            positions: vec![
                DVec3::ZERO,
                DVec3::X * 0.1,
                DVec3::X * 0.2,
                DVec3::new(0.0, 0.1, 0.0),
            ],
            flat: None,
            triangles: vec![[0, 1, 2], [0, 1, 3]], // [0,1,2] is collinear (zero area)
        };
        let mut b = ClothBuilder::new(0.15);
        b.add_panel(&panel, 1.0);
        let mut s = Solver::new(b.build(), Params::default());
        for _ in 0..120 {
            s.step(None);
        }
        assert!(s.cloth().positions().iter().all(|p| p.is_finite()));
        // Vertex 2 only belongs to the zero-area triangle; it must not hold the cloth up.
        assert!(
            s.cloth().positions()[3].y < -0.5,
            "live cloth fell: {}",
            s.cloth().positions()[3]
        );
    }

    #[test]
    fn same_inputs_give_identical_results() {
        let run = || {
            let mut s = Solver::new(single(1.05, DVec3::ZERO, Some(0)), Params::default());
            for _ in 0..60 {
                s.step(None);
            }
            s.cloth().positions().to_vec()
        };
        assert_eq!(run(), run());
    }
}
