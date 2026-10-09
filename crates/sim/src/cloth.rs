use glam::{DVec2, DVec3};
use std::collections::{HashMap, HashSet};

/// One piece of fabric: a triangle mesh, its initial 3D placement, and optionally its flat
/// pattern shape (metres), which defines rest lengths and mass. Without `flat`, the 3D
/// `positions` define them.
#[derive(Clone, Debug, Default)]
pub struct Panel {
    pub positions: Vec<DVec3>,
    pub flat: Option<Vec<DVec2>>,
    pub triangles: Vec<[u32; 3]>,
}

/// Index of a panel's first particle inside the cloth.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PanelId(u32);

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Link {
    pub a: u32,
    pub b: u32,
    pub rest: f64,
}

/// All fabric being simulated, as particles plus distance constraints.
#[derive(Clone, Debug, Default)]
pub struct Cloth {
    pub(crate) x: Vec<DVec3>,
    pub(crate) prev: Vec<DVec3>,
    pub(crate) v: Vec<DVec3>,
    pub(crate) inv_mass: Vec<f64>,
    pub(crate) alive: Vec<bool>,
    pub(crate) triangles: Vec<[u32; 3]>,
    pub(crate) stretch: Vec<Link>,
    pub(crate) bend: Vec<Link>,
    /// `rest` = distance when the seam was made; it shrinks to 0 while the seam closes.
    pub(crate) stitches: Vec<Link>,
    pub(crate) topology_version: u64,
}

pub struct ClothBuilder {
    cloth: Cloth,
    density: f64,
}

fn edge_key(a: u32, b: u32) -> (u32, u32) {
    (a.min(b), a.max(b))
}

/// Unique edges of a triangle list, sorted.
fn unique_edges(triangles: &[[u32; 3]]) -> Vec<(u32, u32)> {
    let mut e: Vec<_> = triangles
        .iter()
        .flat_map(|t| (0..3).map(move |k| edge_key(t[k], t[(k + 1) % 3])))
        .collect();
    e.sort_unstable();
    e.dedup();
    e
}

/// For every edge shared by exactly two triangles, the two vertices opposite it (sorted by edge).
fn bending_pairs(triangles: &[[u32; 3]]) -> Vec<(u32, u32)> {
    let mut opposite: HashMap<(u32, u32), Vec<u32>> = HashMap::new();
    for t in triangles {
        for k in 0..3 {
            opposite
                .entry(edge_key(t[k], t[(k + 1) % 3]))
                .or_default()
                .push(t[(k + 2) % 3]);
        }
    }
    let mut keys: Vec<_> = opposite.keys().copied().collect();
    keys.sort_unstable();
    keys.into_iter()
        .filter_map(|k| match opposite[&k].as_slice() {
            [p, q] => Some((*p, *q)),
            _ => None,
        })
        .collect()
}

impl ClothBuilder {
    /// `density`: fabric weight in kg/m².
    pub fn new(density: f64) -> Self {
        Self {
            cloth: Cloth::default(),
            density,
        }
    }

    /// Adds a panel; its rest lengths are multiplied by `rest_scale` (< 1 pre-tensions it).
    pub fn add_panel(&mut self, panel: &Panel, rest_scale: f64) -> PanelId {
        let base = self.cloth.x.len() as u32;
        let rest_pos = |k: u32| -> DVec3 {
            match &panel.flat {
                Some(f) => f[k as usize].extend(0.0),
                None => panel.positions[k as usize],
            }
        };
        let mut mass = vec![0.0; panel.positions.len()];
        for t in &panel.triangles {
            let [a, b, c] = t.map(rest_pos);
            let area = 0.5 * (b - a).cross(c - a).length();
            for &k in t {
                mass[k as usize] += self.density * area / 3.0;
            }
        }
        self.cloth.x.extend_from_slice(&panel.positions);
        for &m in &mass {
            // A vertex with no area (unused, or only in degenerate triangles) can't move.
            self.cloth
                .inv_mass
                .push(if m > 0.0 { 1.0 / m } else { 0.0 });
            self.cloth.alive.push(m > 0.0);
        }
        // Links to such a vertex would turn it into an invisible pin holding the cloth up.
        let has_mass = |&(a, b): &(u32, u32)| mass[a as usize] > 0.0 && mass[b as usize] > 0.0;
        let link = |(a, b): (u32, u32)| Link {
            a: a + base,
            b: b + base,
            rest: rest_pos(a).distance(rest_pos(b)) * rest_scale,
        };
        self.cloth.stretch.extend(
            unique_edges(&panel.triangles)
                .into_iter()
                .filter(has_mass)
                .map(link),
        );
        self.cloth.bend.extend(
            bending_pairs(&panel.triangles)
                .into_iter()
                .filter(has_mass)
                .map(link),
        );
        self.cloth
            .triangles
            .extend(panel.triangles.iter().map(|t| t.map(|k| k + base)));
        PanelId(base)
    }

    /// Sews particle `a` to particle `b`: pulled together over `Params::stitch_close_time`.
    pub fn stitch(&mut self, a: (PanelId, u32), b: (PanelId, u32)) {
        let (i, j) = (a.0.0 + a.1, b.0.0 + b.1);
        let rest = self.cloth.x[i as usize].distance(self.cloth.x[j as usize]);
        self.cloth.stitches.push(Link { a: i, b: j, rest });
    }

    /// Fixes a particle in space.
    pub fn pin(&mut self, p: (PanelId, u32)) {
        self.cloth.inv_mass[(p.0.0 + p.1) as usize] = 0.0;
    }

    pub fn build(mut self) -> Cloth {
        self.cloth.prev = self.cloth.x.clone();
        self.cloth.v = vec![DVec3::ZERO; self.cloth.x.len()];
        self.cloth
    }
}

impl Cloth {
    pub fn positions(&self) -> &[DVec3] {
        &self.x
    }
    pub fn velocities(&self) -> &[DVec3] {
        &self.v
    }
    pub fn triangles(&self) -> &[[u32; 3]] {
        &self.triangles
    }
    pub fn len(&self) -> usize {
        self.x.len()
    }
    pub fn is_empty(&self) -> bool {
        self.x.is_empty()
    }
    /// False for particles merged away by welding (and unused vertices).
    pub fn is_alive(&self, i: usize) -> bool {
        self.alive[i]
    }
    pub fn is_pinned(&self, i: usize) -> bool {
        self.alive[i] && self.inv_mass[i] == 0.0
    }
    pub fn mass(&self, i: usize) -> f64 {
        if self.inv_mass[i] > 0.0 {
            1.0 / self.inv_mass[i]
        } else {
            f64::INFINITY
        }
    }
    /// Mass of all movable particles.
    pub fn total_mass(&self) -> f64 {
        (0..self.len())
            .filter(|&i| self.alive[i] && self.inv_mass[i] > 0.0)
            .map(|i| self.mass(i))
            .sum()
    }
    pub fn stretch_links(&self) -> impl Iterator<Item = (usize, usize, f64)> + '_ {
        self.stretch
            .iter()
            .map(|l| (l.a as usize, l.b as usize, l.rest))
    }
    pub fn bend_link_count(&self) -> usize {
        self.bend.len()
    }
    pub fn stitch_pairs(&self) -> impl Iterator<Item = (usize, usize)> + '_ {
        self.stitches.iter().map(|l| (l.a as usize, l.b as usize))
    }
    pub fn has_open_stitches(&self) -> bool {
        !self.stitches.is_empty()
    }
    /// Kinetic energy (J) of all movable particles.
    pub fn kinetic_energy(&self) -> f64 {
        (0..self.len())
            .filter(|&i| self.alive[i] && self.inv_mass[i] > 0.0)
            .map(|i| 0.5 * self.mass(i) * self.v[i].length_squared())
            .sum()
    }
    /// Increases whenever `triangles()` change (welding).
    pub fn topology_version(&self) -> u64 {
        self.topology_version
    }

    /// Merges each stitched pair into one particle and rebuilds the constraints on the welded
    /// mesh, so a closed seam behaves like continuous fabric with zero gap.
    pub fn weld_stitches(&mut self) {
        if self.stitches.is_empty() {
            return;
        }
        let mut map: Vec<u32> = (0..self.x.len() as u32).collect();
        fn root(map: &[u32], mut k: u32) -> u32 {
            while map[k as usize] != k {
                k = map[k as usize];
            }
            k
        }
        for s in std::mem::take(&mut self.stitches) {
            let (a, b) = (root(&map, s.a) as usize, root(&map, s.b) as usize);
            if a == b {
                continue;
            }
            let (wa, wb) = (self.inv_mass[a], self.inv_mass[b]);
            self.x[a] = if wa == 0.0 {
                self.x[a]
            } else if wb == 0.0 {
                self.x[b]
            } else {
                (self.x[a] + self.x[b]) * 0.5
            };
            self.v[a] = (self.v[a] + self.v[b]) * 0.5;
            self.inv_mass[a] = if wa == 0.0 || wb == 0.0 {
                0.0
            } else {
                1.0 / (1.0 / wa + 1.0 / wb)
            };
            self.prev[a] = self.x[a];
            map[b] = a as u32;
            self.alive[b] = false;
            self.inv_mass[b] = 0.0;
            self.v[b] = DVec3::ZERO;
        }
        let m = |k: u32| root(&map, k);
        for t in &mut self.triangles {
            *t = t.map(m);
        }
        let mut seen = HashSet::new();
        self.stretch = std::mem::take(&mut self.stretch)
            .into_iter()
            .map(|l| Link {
                a: m(l.a),
                b: m(l.b),
                rest: l.rest,
            })
            .filter(|l| l.a != l.b && seen.insert(edge_key(l.a, l.b)))
            .collect();
        let old: HashMap<(u32, u32), f64> = self
            .bend
            .iter()
            .map(|l| (edge_key(m(l.a), m(l.b)), l.rest))
            .collect();
        let x = &self.x;
        self.bend = bending_pairs(&self.triangles)
            .into_iter()
            .map(|(a, b)| Link {
                a,
                b,
                rest: old
                    .get(&edge_key(a, b))
                    .copied()
                    .unwrap_or_else(|| x[a as usize].distance(x[b as usize])),
            })
            .collect();
        self.topology_version += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Right triangle with 10 cm legs in the pattern, placed `scale`× larger in 3D at `offset`.
    pub(crate) fn tri_panel(scale: f64, offset: DVec3) -> Panel {
        let flat = vec![
            DVec2::new(0.0, 0.0),
            DVec2::new(0.1, 0.0),
            DVec2::new(0.0, 0.1),
        ];
        Panel {
            positions: flat
                .iter()
                .map(|p| p.extend(0.0) * scale + offset)
                .collect(),
            flat: Some(flat),
            triangles: vec![[0, 1, 2]],
        }
    }

    #[test]
    fn rest_lengths_and_mass_come_from_the_flat_pattern() {
        let mut b = ClothBuilder::new(0.15);
        b.add_panel(&tri_panel(2.0, DVec3::ZERO), 0.97);
        let c = b.build();
        let mut rests: Vec<f64> = c.stretch_links().map(|(_, _, r)| r).collect();
        rests.sort_by(f64::total_cmp);
        assert!(
            (rests[0] - 0.097).abs() < 1e-12 && (rests[2] - 0.1f64.hypot(0.1) * 0.97).abs() < 1e-12,
            "{rests:?}"
        );
        assert!((c.mass(0) - 0.15 * 0.005 / 3.0).abs() < 1e-15);
    }

    #[test]
    fn two_triangles_sharing_an_edge_get_one_bending_link() {
        let panel = Panel {
            positions: vec![DVec3::ZERO, DVec3::X, DVec3::Y, DVec3::new(1.0, 1.0, 0.0)],
            flat: None,
            triangles: vec![[0, 1, 2], [1, 3, 2]],
        };
        let mut b = ClothBuilder::new(0.15);
        b.add_panel(&panel, 1.0);
        let c = b.build();
        assert_eq!(c.stretch_links().count(), 5);
        assert_eq!(c.bend_link_count(), 1);
    }

    #[test]
    fn welding_merges_stitched_pairs_and_keeps_mass() {
        let mut b = ClothBuilder::new(0.15);
        let p = b.add_panel(&tri_panel(1.0, DVec3::ZERO), 1.0);
        let q = b.add_panel(&tri_panel(1.0, DVec3::new(0.0, 0.0, 0.2)), 1.0);
        for k in 0..2 {
            b.stitch((p, k), (q, k));
        }
        let mut c = b.build();
        let mass = c.total_mass();
        c.weld_stitches();
        assert!(!c.has_open_stitches());
        assert_eq!((0..c.len()).filter(|&i| c.is_alive(i)).count(), 4);
        assert!((c.total_mass() - mass).abs() < 1e-15);
        assert!(
            c.triangles()
                .iter()
                .flatten()
                .all(|&k| c.is_alive(k as usize)),
            "triangles only use live particles"
        );
        assert_eq!(c.topology_version(), 1);
        // The two triangles now share the welded edge, so a bending link spans it.
        assert_eq!(c.bend_link_count(), 1);
    }
}
