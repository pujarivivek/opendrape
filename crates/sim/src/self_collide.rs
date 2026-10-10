//! Cloth against cloth: every particle keeps a fabric thickness off every triangle it is not
//! part of, so layers stack instead of passing through each other and a fold can't go
//! through itself. The particle–triangle pairs near enough to matter are found through a
//! spatial hash (after Müller's Ten Minute Physics, lesson 15), and found again only when
//! something has moved far enough that the last list might miss a pair: a settled drape
//! finds them once in many frames. Each pair remembers which side of the triangle the
//! particle was on when found, so a particle that drifts across between two looks is put
//! back, not pushed on through.
//!
//! Which pairs are kept apart: a particle and a triangle of another panel, except across a
//! seam (a stitched pair and the neighbours of each end, which a weld joins); and a particle
//! and a triangle of its own panel whose corners all lie at least one and a half edge
//! lengths from it on the pattern, so that a panel's own neighbourhood, held by its links, is
//! never pushed about.

use crate::cloth::Cloth;
use glam::DVec3;

/// Same-panel particles closer than this many edge lengths on the pattern are the fabric's
/// own neighbourhood, held by its links: never pushed.
const NEIGHBOURHOOD_PER_SPACING: f64 = 1.5;

/// A particle and a triangle to keep apart, and which side of the triangle the particle is on
/// (+1 with the triangle's normal, -1 against it).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Pair {
    particle: u32,
    triangle: u32,
    side: i8,
}

/// The pairs that may come within a thickness of each other before the next look.
pub(crate) struct SelfContacts {
    /// Particles are kept at least this far off triangles (m).
    pub(crate) distance: f64,
    /// How far a particle may travel before the pairs are found again: the pairs are every
    /// particle and triangle within `distance + 2 margin` when found, so none can meet
    /// unnoticed.
    margin: f64,
    /// Where every particle was when the pairs were found.
    built_at: Vec<DVec3>,
    pairs: Vec<Pair>,
    /// How many times the pairs have been found.
    pub(crate) builds: u64,
    /// The triangles each particle is a corner of (`incident_start[i]..incident_start[i + 1]`
    /// into `incident`), for the cloth's triangles as of `topology`.
    incident_start: Vec<u32>,
    incident: Vec<u32>,
    topology: Option<u64>,
    cell_start: Vec<u32>,
    cell_entries: Vec<u32>,
    next_slot: Vec<u32>,
    /// Which particle last looked at each triangle, so one reached through two of its corners
    /// is looked at once.
    stamp: Vec<u32>,
}

impl SelfContacts {
    pub(crate) fn new(distance: f64) -> Self {
        Self {
            distance,
            margin: 0.5 * distance,
            built_at: Vec::new(),
            pairs: Vec::new(),
            builds: 0,
            incident_start: Vec::new(),
            incident: Vec::new(),
            topology: None,
            cell_start: Vec::new(),
            cell_entries: Vec::new(),
            next_slot: Vec::new(),
            stamp: Vec::new(),
        }
    }

    pub(crate) fn pairs(&self) -> usize {
        self.pairs.len()
    }

    /// Finds the pairs again if any particle has moved more than the margin since they were
    /// found (or they never were, or the triangles have changed). Returns whether it did.
    pub(crate) fn refresh(&mut self, c: &Cloth) -> bool {
        let m2 = self.margin * self.margin;
        let stale = self.topology != Some(c.topology_version)
            || self.built_at.len() != c.x.len()
            || c.x
                .iter()
                .zip(&self.built_at)
                .zip(&c.alive)
                .any(|((x, at), &alive)| alive && x.distance_squared(*at) > m2);
        if stale {
            self.rebuild(c);
        }
        stale
    }

    /// Forgets the pairs (after welding renumbers the triangles' particles).
    pub(crate) fn invalidate(&mut self) {
        self.built_at.clear();
        self.topology = None;
    }

    /// Lists the triangles each particle is a corner of.
    fn incidence(&mut self, c: &Cloth) {
        let n = c.x.len();
        self.incident_start.clear();
        self.incident_start.resize(n + 1, 0);
        for t in &c.triangles {
            for &k in t {
                self.incident_start[k as usize + 1] += 1;
            }
        }
        for i in 0..n {
            self.incident_start[i + 1] += self.incident_start[i];
        }
        self.incident.clear();
        self.incident.resize(self.incident_start[n] as usize, 0);
        let mut next = self.incident_start.clone();
        for (t, tri) in c.triangles.iter().enumerate() {
            for &k in tri {
                let slot = &mut next[k as usize];
                self.incident[*slot as usize] = t as u32;
                *slot += 1;
            }
        }
        self.stamp.clear();
        self.stamp.resize(c.triangles.len(), u32::MAX);
        self.topology = Some(c.topology_version);
    }

    fn rebuild(&mut self, c: &Cloth) {
        let n = c.x.len();
        self.builds += 1;
        if self.topology != Some(c.topology_version) || self.incident_start.len() != n + 1 {
            self.incidence(c);
        }
        self.built_at.clear();
        self.built_at.extend_from_slice(&c.x);
        // A triangle's nearest point may be this far from a particle before the next look,
        // and its nearest corner at most a circumradius (0.71 edge lengths) further.
        let reach = self.distance + 2.0 * self.margin;
        let corner_reach = reach + 0.75 * c.spacing;
        let cell = corner_reach;
        let table = (2 * n).max(1);
        let key = |p: DVec3| hash(coords(p, cell), table);
        // Counting sort of the live particles into the hash table's cells.
        self.cell_start.clear();
        self.cell_start.resize(table + 1, 0);
        for i in 0..n {
            if c.alive[i] {
                self.cell_start[key(c.x[i])] += 1;
            }
        }
        let mut start = 0;
        for k in 0..=table {
            let count = self.cell_start[k];
            self.cell_start[k] = start;
            start += count;
        }
        self.cell_entries.clear();
        self.cell_entries.resize(start as usize, 0);
        self.next_slot.clear();
        self.next_slot.extend_from_slice(&self.cell_start);
        for i in 0..n {
            if c.alive[i] {
                let k = key(c.x[i]);
                self.cell_entries[self.next_slot[k] as usize] = i as u32;
                self.next_slot[k] += 1;
            }
        }
        // From each particle, through the particles in the 27 cells about it, to the
        // triangles those are corners of.
        let (reach2, corner_reach2) = (reach * reach, corner_reach * corner_reach);
        let own = NEIGHBOURHOOD_PER_SPACING * c.spacing;
        let mut pairs = std::mem::take(&mut self.pairs);
        let mut stamp = std::mem::take(&mut self.stamp);
        pairs.clear();
        for i in 0..n {
            if !c.alive[i] {
                continue;
            }
            let xi = c.x[i];
            let [cx, cy, cz] = coords(xi, cell);
            for dx in -1..=1 {
                for dy in -1..=1 {
                    for dz in -1..=1 {
                        let k = hash([cx + dx, cy + dy, cz + dz], table);
                        let slots = self.cell_start[k] as usize..self.cell_start[k + 1] as usize;
                        for &j in &self.cell_entries[slots] {
                            let j = j as usize;
                            if j == i || xi.distance_squared(c.x[j]) > corner_reach2 {
                                continue;
                            }
                            // A neighbour of the same panel this close on the pattern is the
                            // fabric's own neighbourhood: it is a corner of all its triangles.
                            if c.panel[i] == c.panel[j] && c.rest[i].distance(c.rest[j]) < own {
                                continue;
                            }
                            let incident = self.incident_start[j] as usize
                                ..self.incident_start[j + 1] as usize;
                            for &t in &self.incident[incident] {
                                if stamp[t as usize] == i as u32 {
                                    continue;
                                }
                                stamp[t as usize] = i as u32;
                                let tri = &c.triangles[t as usize];
                                if tri.contains(&(i as u32)) || !kept_apart(c, i, tri) {
                                    continue;
                                }
                                let corners = tri.map(|k| c.x[k as usize]);
                                let (p, _) = closest_point(xi, corners);
                                if xi.distance_squared(p) > reach2 {
                                    continue;
                                }
                                let normal =
                                    (corners[1] - corners[0]).cross(corners[2] - corners[0]);
                                let side = if (xi - p).dot(normal) >= 0.0 { 1 } else { -1 };
                                pairs.push(Pair {
                                    particle: i as u32,
                                    triangle: t,
                                    side,
                                });
                            }
                        }
                    }
                }
            }
        }
        // In particle order, whatever order the cells were walked in.
        pairs.sort_unstable_by_key(|p| (p.particle, p.triangle));
        self.pairs = pairs;
        self.stamp = stamp;
    }

    /// Pushes each particle that is within the distance of its triangle, on the side it was
    /// found on, back out to the distance, sharing the move with the triangle's corners by
    /// inverse mass (as a pin shares with its triangle); then holds back this substep's
    /// sliding between them by Coulomb-style friction (as `collide` does against the body).
    pub(crate) fn solve(&self, c: &mut Cloth, friction: f64) {
        let d = self.distance;
        for pair in &self.pairs {
            let i = pair.particle as usize;
            let tri = c.triangles[pair.triangle as usize].map(|k| k as usize);
            if !c.alive[i] || tri.iter().any(|&k| !c.alive[k]) {
                continue;
            }
            let corners = tri.map(|k| c.x[k]);
            let (p, bary) = closest_point(c.x[i], corners);
            let delta = c.x[i] - p;
            // Over the triangle, how far the particle stands off it on its side: one that has
            // drifted through reads negative and comes back. Beside it (nearest to an edge or
            // a corner), plainly how far it is, with no side to it: flat fabric two rings
            // across a welded seam lies in the triangle's plane and must not be pushed.
            let over = bary.iter().all(|&w| w > 1e-9);
            let (n, standoff) = if over {
                let normal = (corners[1] - corners[0])
                    .cross(corners[2] - corners[0])
                    .try_normalize();
                let Some(normal) = normal else { continue };
                let n = normal * f64::from(pair.side);
                (n, delta.dot(n))
            } else {
                let dist = delta.length();
                if dist < 1e-12 {
                    continue;
                }
                (delta / dist, dist)
            };
            if standoff >= d {
                continue;
            }
            let wi = c.inv_mass[i];
            let wsum: f64 = wi
                + (0..3)
                    .map(|k| c.inv_mass[tri[k]] * bary[k] * bary[k])
                    .sum::<f64>();
            if wsum == 0.0 {
                continue;
            }
            let push = d - standoff;
            let lambda = push / wsum;
            c.x[i] += n * (lambda * wi);
            for k in 0..3 {
                c.x[tri[k]] -= n * (lambda * c.inv_mass[tri[k]] * bary[k]);
            }
            // Friction on the sliding between the particle and the spot of the triangle.
            let moved_tri: DVec3 = (0..3)
                .map(|k| (c.x[tri[k]] - c.prev[tri[k]]) * bary[k])
                .sum();
            let rel = (c.x[i] - c.prev[i]) - moved_tri;
            let slide = rel - n * n.dot(rel);
            let len = slide.length();
            if len <= 0.0 {
                continue;
            }
            let limit = friction * push;
            let f = if len <= limit { 1.0 } else { limit / len };
            let hold = slide * (f / wsum);
            c.x[i] -= hold * wi;
            for k in 0..3 {
                c.x[tri[k]] += hold * (c.inv_mass[tri[k]] * bary[k]);
            }
        }
    }
}

/// Whether particle `i` is kept a thickness off triangle `tri` (see the module doc).
fn kept_apart(c: &Cloth, i: usize, tri: &[u32; 3]) -> bool {
    tri.iter().all(|&k| {
        let k = k as usize;
        if c.panel[i] == c.panel[k] {
            c.rest[i].distance(c.rest[k]) >= NEIGHBOURHOOD_PER_SPACING * c.spacing
        } else {
            let key = (i.min(k) as u32, i.max(k) as u32);
            c.excluded.binary_search(&key).is_err()
        }
    })
}

/// The point of the triangle `[a, b, c]` nearest to `p`, and its barycentric coordinates
/// (Ericson, Real-Time Collision Detection, 5.1.5).
fn closest_point(p: DVec3, [a, b, c]: [DVec3; 3]) -> (DVec3, [f64; 3]) {
    let (ab, ac, ap) = (b - a, c - a, p - a);
    let (d1, d2) = (ab.dot(ap), ac.dot(ap));
    if d1 <= 0.0 && d2 <= 0.0 {
        return (a, [1.0, 0.0, 0.0]);
    }
    let bp = p - b;
    let (d3, d4) = (ab.dot(bp), ac.dot(bp));
    if d3 >= 0.0 && d4 <= d3 {
        return (b, [0.0, 1.0, 0.0]);
    }
    let vc = d1 * d4 - d3 * d2;
    if vc <= 0.0 && d1 >= 0.0 && d3 <= 0.0 {
        let v = if d1 - d3 != 0.0 { d1 / (d1 - d3) } else { 0.0 };
        return (a + ab * v, [1.0 - v, v, 0.0]);
    }
    let cp = p - c;
    let (d5, d6) = (ab.dot(cp), ac.dot(cp));
    if d6 >= 0.0 && d5 <= d6 {
        return (c, [0.0, 0.0, 1.0]);
    }
    let vb = d5 * d2 - d1 * d6;
    if vb <= 0.0 && d2 >= 0.0 && d6 <= 0.0 {
        let w = if d2 - d6 != 0.0 { d2 / (d2 - d6) } else { 0.0 };
        return (a + ac * w, [1.0 - w, 0.0, w]);
    }
    let va = d3 * d6 - d5 * d4;
    if va <= 0.0 && (d4 - d3) >= 0.0 && (d5 - d6) >= 0.0 {
        let denom = (d4 - d3) + (d5 - d6);
        let w = if denom != 0.0 { (d4 - d3) / denom } else { 0.0 };
        return (b + (c - b) * w, [0.0, 1.0 - w, w]);
    }
    let denom = va + vb + vc;
    let (v, w) = if denom != 0.0 {
        (vb / denom, vc / denom)
    } else {
        (1.0 / 3.0, 1.0 / 3.0)
    };
    (a + ab * v + ac * w, [1.0 - v - w, v, w])
}

fn coords(p: DVec3, cell: f64) -> [i64; 3] {
    [
        (p.x / cell).floor() as i64,
        (p.y / cell).floor() as i64,
        (p.z / cell).floor() as i64,
    ]
}

fn hash([x, y, z]: [i64; 3], table: usize) -> usize {
    let h = (x.wrapping_mul(92_837_111))
        ^ (y.wrapping_mul(689_287_499))
        ^ (z.wrapping_mul(283_923_481));
    h.rem_euclid(table as i64) as usize
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cloth::{ClothBuilder, Panel};
    use crate::solver::self_collision_distance;
    use crate::{Params, Solver};
    use glam::DVec2;

    /// A flat `nx` × `ny` cell sheet of `h` m cells, lying level at height `y`, from `x0`.
    fn sheet(nx: usize, ny: usize, h: f64, x0: f64, y: f64, z0: f64) -> Panel {
        let mut flat = Vec::new();
        let mut positions = Vec::new();
        for j in 0..=ny {
            for i in 0..=nx {
                flat.push(DVec2::new(i as f64 * h, j as f64 * h));
                positions.push(DVec3::new(x0 + i as f64 * h, y, z0 + j as f64 * h));
            }
        }
        let id = |i: usize, j: usize| (j * (nx + 1) + i) as u32;
        let mut triangles = Vec::new();
        for j in 0..ny {
            for i in 0..nx {
                triangles.push([id(i, j), id(i + 1, j), id(i + 1, j + 1)]);
                triangles.push([id(i, j), id(i + 1, j + 1), id(i, j + 1)]);
            }
        }
        Panel {
            positions,
            flat: Some(flat),
            triangles,
        }
    }

    fn still() -> Params {
        Params {
            gravity: 0.0,
            gravity_delay: 0.0,
            ..Params::default()
        }
    }

    #[test]
    fn the_closest_point_is_found_in_every_region_of_a_triangle() {
        let t = [DVec3::ZERO, DVec3::X, DVec3::Y];
        let at = |p: DVec3| closest_point(p, t);
        assert_eq!(at(DVec3::new(-1.0, -1.0, 0.0)).0, DVec3::ZERO);
        assert_eq!(at(DVec3::new(2.0, -1.0, 0.0)).0, DVec3::X);
        assert_eq!(at(DVec3::new(-1.0, 2.0, 0.0)).0, DVec3::Y);
        assert_eq!(at(DVec3::new(0.5, -1.0, 0.0)).0, DVec3::new(0.5, 0.0, 0.0));
        assert_eq!(at(DVec3::new(-1.0, 0.5, 0.0)).0, DVec3::new(0.0, 0.5, 0.0));
        let (p, w) = at(DVec3::new(1.0, 1.0, 0.0));
        assert!(p.abs_diff_eq(DVec3::new(0.5, 0.5, 0.0), 1e-12) && (w[1] - 0.5).abs() < 1e-12);
        let (p, w) = at(DVec3::new(0.25, 0.25, 3.0));
        assert!(p.abs_diff_eq(DVec3::new(0.25, 0.25, 0.0), 1e-12));
        assert!((w[0] - 0.5).abs() < 1e-12 && (w[1] - 0.25).abs() < 1e-12);
    }

    #[test]
    fn a_sheet_dropped_on_a_held_sheet_rests_a_thickness_above_it() {
        let run = || {
            let mut b = ClothBuilder::new(0.15);
            let floor = b.add_panel(&sheet(8, 8, 0.02, 0.0, 0.0, 0.0), 1.0);
            for k in 0..81 {
                b.pin((floor, k));
            }
            // A smaller sheet 3 cm up, over the middle of the held one, offset by half a
            // cell so its particles fall over the held sheet's cells, not its particles.
            b.add_panel(&sheet(4, 4, 0.02, 0.05, 0.03, 0.05), 1.0);
            let mut s = Solver::new(
                b.build(),
                Params {
                    gravity_delay: 0.0,
                    gravity_ramp: 0.0,
                    ..Params::default()
                },
            );
            for _ in 0..90 {
                s.step(None);
            }
            s
        };
        let s = run();
        let d = self_collision_distance(s.params(), s.cloth().spacing());
        let x = s.cloth().positions();
        let lowest = x[81..].iter().map(|p| p.y).fold(f64::MAX, f64::min);
        assert!(lowest >= d - 1e-4, "{lowest} vs {d}");
        assert!(lowest < 2.0 * d, "it fell onto the held sheet: {lowest}");
        assert!(
            s.cloth().kinetic_energy() < 1e-7,
            "{}",
            s.cloth().kinetic_energy()
        );
        assert_eq!(
            s.cloth().positions(),
            run().cloth().positions(),
            "deterministic"
        );
    }

    #[test]
    fn a_strip_folded_onto_itself_opens_to_a_thickness() {
        // A 10 × 1 cell strip folded in half at cell 5: the two halves lie 2 mm apart.
        let h = 0.01;
        let mut panel = sheet(10, 1, h, 0.0, 0.0, 0.0);
        for (k, p) in panel.positions.iter_mut().enumerate() {
            let i = k % 11;
            if i > 5 {
                *p = DVec3::new((10 - i) as f64 * h, 0.002, p.z);
            }
        }
        let mut b = ClothBuilder::new(0.15);
        b.add_panel(&panel, 1.0);
        let mut s = Solver::new(b.build(), still());
        for _ in 0..60 {
            s.step(None);
        }
        let c = s.cloth();
        let (d, x) = (
            self_collision_distance(s.params(), c.spacing()),
            c.positions(),
        );
        for k in 0..22 {
            for l in 0..22 {
                let (i, j) = (k % 11, l % 11);
                // Lying on each other across the fold, far enough along the strip to count.
                if i < 5 && j > 5 && (i as i64 - (10 - j) as i64).abs() <= 1 && (5 - i) >= 2 {
                    let gap = x[k].distance(x[l]);
                    assert!(gap >= 0.9 * d, "{k} and {l} are {gap} apart, not {d}");
                }
            }
        }
        assert!(x.iter().all(|p| p.is_finite()));
    }

    #[test]
    fn a_flat_sheet_is_left_alone() {
        let mut b = ClothBuilder::new(0.15);
        b.add_panel(&sheet(6, 6, 0.012, 0.0, 1.0, 0.0), 1.0);
        let before: Vec<DVec3> = b.build().positions().to_vec();
        let mut b = ClothBuilder::new(0.15);
        b.add_panel(&sheet(6, 6, 0.012, 0.0, 1.0, 0.0), 1.0);
        let mut s = Solver::new(b.build(), still());
        for _ in 0..30 {
            s.step(None);
        }
        for (a, b) in s.cloth().positions().iter().zip(&before) {
            assert!(a.distance(*b) < 1e-9, "{a} moved from {b}");
        }
        assert_eq!(s.self_contact_stats().0, 0, "no pairs in a flat sheet");
    }

    #[test]
    fn stitched_panels_still_close_and_weld() {
        // Two sheets side by side with a 10 cm gap, sewn along their facing edges.
        let mut b = ClothBuilder::new(0.15);
        let p = b.add_panel(&sheet(3, 3, 0.012, 0.0, 1.0, 0.0), 1.0);
        let q = b.add_panel(&sheet(3, 3, 0.012, 0.136, 1.0, 0.0), 1.0);
        for j in 0..4 {
            b.stitch((p, j * 4 + 3), (q, j * 4));
        }
        let mut s = Solver::new(b.build(), still());
        for _ in 0..36 {
            s.step(None);
        }
        assert!(!s.cloth().has_open_stitches(), "welded");
        assert_eq!(s.take_notes(), vec![]);
        assert!(s.cloth().positions().iter().all(|p| p.is_finite()));
    }

    #[test]
    fn pairs_are_found_again_only_when_something_has_moved() {
        let mut b = ClothBuilder::new(0.15);
        b.add_panel(&sheet(4, 4, 0.02, 0.0, 0.0, 0.0), 1.0);
        b.add_panel(&sheet(4, 4, 0.02, 0.0, 0.01, 0.0), 1.0);
        let mut c = b.build();
        let mut sc = SelfContacts::new(0.015);
        assert!(sc.refresh(&c));
        assert!(sc.pairs() >= 25, "{}", sc.pairs());
        assert!(!sc.refresh(&c), "nothing moved");
        c.x[0].x += 0.001;
        assert!(!sc.refresh(&c), "within the margin");
        c.x[0].x += 0.01;
        assert!(sc.refresh(&c));
        assert_eq!(sc.builds, 2);
        sc.invalidate();
        assert!(sc.refresh(&c), "forgotten");
    }

    #[test]
    fn a_particle_that_drifted_through_a_layer_is_put_back_on_its_side() {
        // Two level sheets 1 cm apart; the upper's middle particle is then moved to 2 mm
        // below the lower sheet, closer to it than when the pairs were found.
        let mut b = ClothBuilder::new(0.15);
        b.add_panel(&sheet(4, 4, 0.02, 0.0, 0.0, 0.0), 1.0);
        b.add_panel(&sheet(4, 4, 0.02, 0.01, 0.01, 0.01), 1.0);
        let mut c = b.build();
        let mut sc = SelfContacts::new(0.006);
        sc.refresh(&c);
        let middle = 25 + 12;
        c.x[middle].y = -0.002;
        c.prev[middle] = c.x[middle];
        sc.solve(&mut c, 0.0);
        assert!(
            c.x[middle].y > 0.0,
            "back above the lower sheet: {}",
            c.x[middle].y
        );
    }
}
