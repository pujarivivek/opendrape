use crate::cloth::{Cloth, Link};
use crate::collide::{Collider, Plane};
use crate::self_collide::SelfContacts;
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
    /// A seam welds (its stitched pairs merge) once every stitch of it is within this gap (m).
    /// A seam under tension (fabric tight over the body) keeps a small gap however long it
    /// pulls, since the fabric's own constraints have the last word in every pass: a third of
    /// an edge length is the tension a weld takes over as strain.
    pub weld_gap: f64,
    /// When seams that still haven't closed are welded anyway, with a note (None: never).
    pub weld_timeout: Option<f64>,
    /// XPBD compliance (m/N) of fabric edges along the warp and weft, of bending, and of the
    /// bias (the diagonals of the fabric's cells, and the hinges across them).
    pub stretch_compliance: f64,
    pub bend_compliance: f64,
    pub shear_compliance: f64,
    /// Velocity damping per second.
    pub damping: f64,
    pub friction: f64,
    /// Distance kept between cloth and body (m).
    pub thickness: f64,
    /// Contact planes are created for particles within this distance of the body (m).
    pub collision_margin: f64,
    pub max_speed: f64,
    /// Whether cloth keeps off cloth (layers stack, folds don't pass through themselves),
    /// from the moment every seam has welded.
    pub self_collision: bool,
    /// How far cloth particles are kept off the other layers' triangles (m); None: half the
    /// fabric's edge length.
    pub self_collision_distance: Option<f64>,
    /// Cloth against cloth is solved every this many substeps (1: every one).
    pub self_collision_every: usize,
    /// Friction between layers of cloth.
    pub cloth_friction: f64,
}

impl Default for Params {
    fn default() -> Self {
        Self {
            // Validated on the demo grids, whose every edge is rigid. Fabric on the grain,
            // whose bias gives, does better with more substeps and one pass each (see
            // `opendrape_drape::DrapeQuality` and the sweep in docs/testing/bench.md).
            substeps: 20,
            iterations: 2,
            gravity: -9.81,
            gravity_delay: 0.6,
            gravity_ramp: 0.3,
            stitch_close_time: 0.5,
            weld_gap: 0.004,
            weld_timeout: Some(3.0),
            stretch_compliance: 1e-6,
            bend_compliance: 1.0,
            shear_compliance: 0.05,
            damping: 1.0,
            friction: 0.4,
            thickness: 0.003,
            collision_margin: 0.05,
            max_speed: 2.0,
            self_collision: true,
            self_collision_distance: None,
            self_collision_every: 1,
            cloth_friction: 0.3,
        }
    }
}

/// What self-collision keeps a particle off the other layers' triangles by: the setting, or
/// half the fabric's edge length.
pub fn self_collision_distance(p: &Params, spacing: f64) -> f64 {
    p.self_collision_distance.unwrap_or(0.5 * spacing)
}

/// Something the solver wants the student to know.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SolverNote {
    /// Seam `group` had not closed by `Params::weld_timeout` and was pulled shut from a gap of
    /// `gap_mm`.
    SeamForcedShut { group: u32, gap_mm: f64 },
}

/// A particle's last answer from the body: its contact plane (None beyond the margin), how far
/// it was from the surface, and where it was when asked.
#[derive(Clone, Copy, Debug)]
struct Contact {
    plane: Option<Plane>,
    clearance: f64,
    at: DVec3,
}

/// A particle in contact is asked again once it has moved this far (m): its plane still holds
/// over less, the body being still.
const REASK_M: f64 = 0.0005;

impl Contact {
    const NEVER: Self = Self {
        plane: None,
        clearance: f64::NEG_INFINITY,
        at: DVec3::INFINITY,
    };

    /// Whether the particle, now at `x`, may have reached the surface since it was asked: it
    /// has moved further than its clearance beyond the margin allows, or, in contact, further
    /// than its plane holds for.
    fn stale(&self, x: DVec3, margin: f64) -> bool {
        x.distance_squared(self.at) > (self.clearance - margin).max(REASK_M).powi(2)
    }
}

pub struct Solver {
    cloth: Cloth,
    params: Params,
    time: f64,
    phases: PhaseTimes,
    notes: Vec<SolverNote>,
    /// Cloth-against-cloth pairs, made on the first step that needs them.
    self_contacts: Option<SelfContacts>,
    /// Each particle's last answer from the body (empty without a body).
    contacts: Vec<Contact>,
    /// How many particles the body has been asked about so far.
    body_queries: u64,
}

impl Solver {
    pub fn new(cloth: Cloth, params: Params) -> Self {
        Self {
            cloth,
            params,
            time: 0.0,
            phases: PhaseTimes::default(),
            notes: Vec::new(),
            self_contacts: None,
            contacts: Vec::new(),
            body_queries: 0,
        }
    }
    /// How many particles the body has been asked about so far: a settled drape asks about
    /// none.
    pub fn body_queries(&self) -> u64 {
        self.body_queries
    }
    /// How many cloth-against-cloth pairs are being watched, and how many times they have
    /// been found (0, 0 without self-collision).
    pub fn self_contact_stats(&self) -> (usize, u64) {
        self.self_contacts
            .as_ref()
            .map_or((0, 0), |s| (s.pairs(), s.builds))
    }
    /// The notes made since the last call.
    pub fn take_notes(&mut self) -> Vec<SolverNote> {
        std::mem::take(&mut self.notes)
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
        if self.cloth.has_open_stitches() {
            if p.weld_timeout.is_some_and(|tw| t >= tw) {
                for (group, gap) in self.cloth.open_seam_gaps() {
                    if gap > p.weld_gap {
                        self.notes.push(SolverNote::SeamForcedShut {
                            group,
                            gap_mm: gap * 1000.0,
                        });
                    }
                }
                self.cloth.weld_stitches();
            } else {
                self.cloth.weld_closed(p.weld_gap);
            }
            // A weld renumbers the triangles' particles: any pairs found are stale.
            if let Some(sc) = &mut self.self_contacts {
                sc.invalidate();
            }
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
        // The body is asked about a particle only once it may have reached the surface since
        // it was last asked (the body being still), so a settled drape asks about nothing.
        if let Some(col) = collider {
            let n = self.cloth.x.len();
            if self.contacts.len() != n {
                self.contacts = vec![Contact::NEVER; n];
            }
            let which: Vec<u32> = (0..n)
                .filter(|&i| {
                    self.cloth.alive[i]
                        && self.cloth.inv_mass[i] > 0.0
                        && self.contacts[i].stale(self.cloth.x[i], p.collision_margin)
                })
                .map(|i| i as u32)
                .collect();
            let found = col.contacts(&self.cloth.x, &which, p.collision_margin);
            for (&i, (plane, clearance)) in which.iter().zip(found) {
                self.contacts[i as usize] = Contact {
                    plane,
                    clearance,
                    at: self.cloth.x[i as usize],
                };
            }
            self.body_queries += which.len() as u64;
        } else {
            self.contacts.clear();
        }
        lap.lap(&mut ph.body_query);
        let sdt = FRAME_DT / p.substeps as f64;
        let Solver {
            cloth: c,
            self_contacts,
            contacts,
            ..
        } = self;
        // Cloth keeps off cloth once the garment is sewn. Seams pull pieces through each other
        // on their way shut (they are arranged round the form, not sewn), and particles kept
        // a thickness apart cannot meet: with self-collision from the start, the drafted
        // T-shirt's seams only shut at the timeout.
        if p.self_collision && c.spacing > 0.0 && !c.has_open_stitches() {
            let d = self_collision_distance(&p, c.spacing);
            if self_contacts.as_ref().is_none_or(|s| s.distance != d) {
                *self_contacts = Some(SelfContacts::new(d));
            }
        } else {
            *self_contacts = None;
        }
        for sub in 0..p.substeps {
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
                    &c.shear,
                    p.shear_compliance,
                    1.0,
                    sdt,
                );
                lap.lap(&mut ph.shear);
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
            // Cloth off cloth, then the body has the last word.
            if let Some(sc) = self_contacts
                && sub % p.self_collision_every.max(1) == 0
            {
                sc.refresh(c);
                sc.solve(c, p.cloth_friction);
            }
            lap.lap(&mut ph.self_collide);
            collide(c, contacts, p.thickness, p.friction);
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
fn collide(c: &mut Cloth, contacts: &[Contact], thickness: f64, friction: f64) {
    for (i, contact) in contacts.iter().enumerate() {
        let Some(pl) = &contact.plane else { continue };
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
        for _ in 0..24 {
            s.step(None); // 0.4 s: the 10 cm seam is still closing, about 2 cm to go
        }
        assert!(s.cloth().has_open_stitches());
        let x = s.cloth().positions();
        assert!(
            s.cloth()
                .stitch_pairs()
                .all(|(a, b)| (0.01..0.03).contains(&(x[a] - x[b]).length()))
        );
        for _ in 0..12 {
            s.step(None); // 0.6 s: past stitch_close_time 0.5 s, so within weld_gap, so welded
        }
        assert!(!s.cloth().has_open_stitches());
        assert!(s.cloth().positions().iter().all(|p| p.is_finite()));
        assert_eq!(s.take_notes(), vec![], "it closed on its own");
    }

    #[test]
    fn a_seam_that_cannot_close_is_pulled_shut_at_the_timeout_with_a_note() {
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
        b.stitch_in((p, 0), (q, 0), 7);
        b.stitch_in((p, 1), (q, 1), 7);
        // Every corner pinned: the seam can't close.
        for k in 0..3 {
            b.pin((p, k));
            b.pin((q, k));
        }
        let params = Params {
            weld_timeout: Some(0.2),
            ..no_gravity()
        };
        let mut s = Solver::new(b.build(), params);
        for _ in 0..11 {
            s.step(None); // the last of these starts at 0.167 s
        }
        assert!(s.cloth().has_open_stitches());
        assert_eq!(s.take_notes(), vec![]);
        for _ in 0..3 {
            s.step(None); // the last of these starts at 0.217 s, past the timeout
        }
        assert!(!s.cloth().has_open_stitches());
        let notes = s.take_notes();
        assert_eq!(notes.len(), 1);
        let SolverNote::SeamForcedShut { group, gap_mm } = notes[0];
        assert_eq!(group, 7);
        assert!((gap_mm - 100.0).abs() < 1e-6, "{gap_mm}");
        assert_eq!(s.take_notes(), vec![], "told once");
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
    fn a_particle_at_rest_on_the_body_is_not_asked_about_again() {
        let floor = crate::CompoundCollider::new(vec![], Some(0.0));
        let params = Params {
            gravity_delay: 0.0,
            gravity_ramp: 0.0,
            ..Params::default()
        };
        let mut s = Solver::new(single(1.0, DVec3::new(0.0, 0.2, 0.0), None), params);
        for _ in 0..180 {
            s.step(Some(&floor));
        }
        let asked = s.body_queries();
        assert!(asked >= 3, "asked while falling: {asked}");
        for _ in 0..60 {
            s.step(Some(&floor));
        }
        let again = s.body_queries() - asked;
        assert!(again <= 3, "asked {again} times about a triangle at rest");
        // Standing on its bottom edge (nothing tips it out of its plane).
        let ys: Vec<f64> = s.cloth().positions().iter().map(|p| p.y).collect();
        assert!(
            ys.iter().all(|&y| y >= 0.003 - 1e-4) && ys.iter().filter(|&&y| y < 0.004).count() == 2,
            "resting on the floor: {ys:?}"
        );
        // Lifted clear of the floor, it is asked about again once it comes back down.
        for p in s.cloth_mut().x.iter_mut() {
            p.y += 0.3;
        }
        for _ in 0..120 {
            s.step(Some(&floor));
        }
        assert!(s.body_queries() > asked + 3);
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
