use glam::{DVec2, DVec3};
use std::collections::{BTreeMap, HashMap, HashSet};

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
    /// Edges along the warp or the weft: they hold their length.
    pub(crate) stretch: Vec<Link>,
    /// Hinges across structural edges: the fabric's resistance to folding.
    pub(crate) bend: Vec<Link>,
    /// Hinges across welded seams, held flat (`Params::seam_compliance`): a sewn seam is far
    /// stiffer than the fabric, and a distance link can't hold an angle without also acting
    /// along the fabric, which on a stretched garment buckles the seam into a ridge.
    pub(crate) seam_hinges: Vec<Hinge>,
    /// Edges on the bias, and the hinges across them (a cell's other diagonal): the fabric
    /// shears along these, softly.
    pub(crate) shear: Vec<Link>,
    /// `rest` = distance when the seam was made; it shrinks to 0 while the seam closes.
    pub(crate) stitches: Vec<Link>,
    /// The seam each stitch belongs to (welded together once every stitch of it is closed).
    pub(crate) stitch_group: Vec<u32>,
    /// Whether each particle has been through a weld (it lies on a welded seam).
    pub(crate) welded: Vec<bool>,
    pub(crate) topology_version: u64,
    /// Points of the cloth pulled to targets (see `attach.rs`); a removed one leaves None.
    pub(crate) attachments: Vec<Option<crate::attach::Attachment>>,
    /// Fabric edges that run along a welded seam (both ends were stitched), sorted.
    pub(crate) seam_edges: Vec<(u32, u32)>,
    /// Which panel each particle came from, and where it rests on that panel's pattern (or,
    /// without a pattern, where it started).
    pub(crate) panel: Vec<u32>,
    pub(crate) rest: Vec<DVec3>,
    /// The fabric's typical edge length (m): the mean structural rest length.
    pub(crate) spacing: f64,
    /// The longest triangle edge on the pattern (m): no point of a triangle is further than
    /// half of it from a corner.
    pub(crate) longest_edge: f64,
    /// Pairs across a seam (each stitched pair, and the neighbours of either end) that are
    /// never pushed apart by self-collision, sorted.
    pub(crate) excluded: Vec<(u32, u32)>,
}

pub struct ClothBuilder {
    cloth: Cloth,
    density: f64,
    panels: u32,
}

fn edge_key(a: u32, b: u32) -> (u32, u32) {
    (a.min(b), a.max(b))
}

/// How many rings of neighbours round each end of a stitch are never pushed apart from the
/// other end's: the fabric a weld joins. (Wider rings make no difference to how seams close,
/// measured on the drafted T-shirt.)
const SEAM_RINGS: usize = 1;

/// An edge within 22.5° of the warp or the weft is structural; the rest are on the bias.
/// cos 22.5° and sin 22.5°.
const ALONG_GRAIN_COS: f64 = 0.923_879_532_511_286_7;
const ACROSS_GRAIN_COS: f64 = 0.382_683_432_365_089_8;

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

/// An edge `u`–`v` shared by exactly two triangles, and the vertices `p` and `q` opposite it in
/// each: the fabric bends about the edge. Within a panel a distance link `p`–`q` resists it;
/// across a seam the angle itself is held.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Hinge {
    pub(crate) u: u32,
    pub(crate) v: u32,
    pub(crate) p: u32,
    pub(crate) q: u32,
}

/// Every edge shared by exactly two triangles, sorted by edge.
fn hinges(triangles: &[[u32; 3]]) -> Vec<Hinge> {
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
        .filter_map(|(u, v)| match opposite[&(u, v)].as_slice() {
            [p, q] => Some(Hinge { u, v, p: *p, q: *q }),
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
            panels: 0,
        }
    }

    /// Adds a panel; its rest lengths are multiplied by `rest_scale` (< 1 pre-tensions it).
    /// Every edge is structural: the fabric has no bias.
    pub fn add_panel(&mut self, panel: &Panel, rest_scale: f64) -> PanelId {
        self.add(panel, rest_scale, None)
    }

    /// Adds a panel of fabric with a grain, `grain` being the warp's direction on its flat
    /// pattern. Edges along the warp or the weft (within 22.5°) are structural and hold their
    /// length; the rest run on the bias and shear softly (`Params::shear_compliance`), as do
    /// the hinges across them. Without `flat` every edge is structural, as in [`Self::add_panel`].
    pub fn add_grain_panel(&mut self, panel: &Panel, rest_scale: f64, grain: DVec2) -> PanelId {
        self.add(panel, rest_scale, Some(grain))
    }

    fn add(&mut self, panel: &Panel, rest_scale: f64, grain: Option<DVec2>) -> PanelId {
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
            self.cloth.welded.push(false);
            self.cloth.panel.push(self.panels);
        }
        self.cloth
            .rest
            .extend((0..panel.positions.len() as u32).map(rest_pos));
        self.panels += 1;
        // Links to such a vertex would turn it into an invisible pin holding the cloth up.
        let has_mass = |&(a, b): &(u32, u32)| mass[a as usize] > 0.0 && mass[b as usize] > 0.0;
        let link = |(a, b): (u32, u32)| Link {
            a: a + base,
            b: b + base,
            rest: rest_pos(a).distance(rest_pos(b)) * rest_scale,
        };
        // Whether the edge a–b runs on the bias: neither along the warp nor along the weft.
        let on_bias = |a: u32, b: u32| match (grain, &panel.flat) {
            (Some(g), Some(f)) if g.length_squared() > 0.0 => {
                let d = f[b as usize] - f[a as usize];
                let len = d.length();
                if len <= 0.0 {
                    return false;
                }
                let c = (d.dot(g) / (len * g.length())).abs();
                c < ALONG_GRAIN_COS && c > ACROSS_GRAIN_COS
            }
            _ => false,
        };
        for e in unique_edges(&panel.triangles).into_iter().filter(has_mass) {
            if on_bias(e.0, e.1) {
                self.cloth.shear.push(link(e));
            } else {
                self.cloth.stretch.push(link(e));
            }
        }
        // Whether a hinge's two triangles are one square cell of the lattice, so the link
        // between its opposite corners is the cell's other diagonal: a shear spring. The
        // irregular triangles in the band along an outline bend like any other fabric, even
        // across an edge on the bias; left soft, that band folds into a sawtooth beside every
        // seam.
        let is_cell = |h: &Hinge| {
            let Some(f) = &panel.flat else { return false };
            let d = |a: u32, b: u32| f[a as usize].distance(f[b as usize]);
            let diagonals = (d(h.u, h.v), d(h.p, h.q));
            let sides = [d(h.u, h.p), d(h.p, h.v), d(h.v, h.q), d(h.q, h.u)];
            let side = sides.iter().sum::<f64>() / 4.0;
            side > 0.0
                && sides.iter().all(|s| (s - side).abs() <= 0.05 * side)
                && (diagonals.0 - diagonals.1).abs() <= 0.05 * side
                && (diagonals.0 - side * std::f64::consts::SQRT_2).abs() <= 0.05 * side
        };
        for h in hinges(&panel.triangles) {
            if !has_mass(&(h.p, h.q)) {
                continue;
            }
            if on_bias(h.u, h.v) && is_cell(&h) {
                self.cloth.shear.push(link((h.p, h.q)));
            } else {
                self.cloth.bend.push(link((h.p, h.q)));
            }
        }
        self.cloth
            .triangles
            .extend(panel.triangles.iter().map(|t| t.map(|k| k + base)));
        PanelId(base)
    }

    /// Sews particle `a` to particle `b`: pulled together over `Params::stitch_close_time`, as
    /// part of seam 0 (see [`Self::stitch_in`]).
    pub fn stitch(&mut self, a: (PanelId, u32), b: (PanelId, u32)) {
        self.stitch_in(a, b, 0);
    }

    /// Sews particle `a` to particle `b` as part of seam `group`: the seam welds once every
    /// stitch of it has closed.
    pub fn stitch_in(&mut self, a: (PanelId, u32), b: (PanelId, u32), group: u32) {
        let (i, j) = (a.0.0 + a.1, b.0.0 + b.1);
        let rest = self.cloth.x[i as usize].distance(self.cloth.x[j as usize]);
        self.cloth.stitches.push(Link { a: i, b: j, rest });
        self.cloth.stitch_group.push(group);
    }

    /// Fixes a particle in space.
    pub fn pin(&mut self, p: (PanelId, u32)) {
        self.cloth.inv_mass[(p.0.0 + p.1) as usize] = 0.0;
    }

    pub fn build(mut self) -> Cloth {
        let c = &mut self.cloth;
        c.prev = c.x.clone();
        c.v = vec![DVec3::ZERO; c.x.len()];
        c.spacing = if c.stretch.is_empty() {
            0.0
        } else {
            c.stretch.iter().map(|l| l.rest).sum::<f64>() / c.stretch.len() as f64
        };
        let rest = &c.rest;
        c.longest_edge = c
            .triangles
            .iter()
            .flat_map(|t| (0..3).map(move |k| (t[k] as usize, t[(k + 1) % 3] as usize)))
            .map(|(a, b)| rest[a].distance(rest[b]))
            .fold(0.0, f64::max);
        // Each stitched pair, and the neighbours of either end, are never pushed apart: a weld
        // joins them.
        let mut ring: Vec<Vec<u32>> = vec![Vec::new(); c.x.len()];
        for l in c.stretch.iter().chain(&c.shear) {
            ring[l.a as usize].push(l.b);
            ring[l.b as usize].push(l.a);
        }
        let near = |k: u32| -> Vec<u32> {
            let mut seen = vec![k];
            let mut frontier = vec![k];
            for _ in 0..SEAM_RINGS {
                let mut next = Vec::new();
                for &f in &frontier {
                    for &n in &ring[f as usize] {
                        if !seen.contains(&n) {
                            seen.push(n);
                            next.push(n);
                        }
                    }
                }
                frontier = next;
            }
            seen
        };
        let mut excluded = Vec::new();
        for s in &c.stitches {
            for &a in &near(s.a) {
                for &b in &near(s.b) {
                    if a != b {
                        excluded.push(edge_key(a, b));
                    }
                }
            }
        }
        excluded.sort_unstable();
        excluded.dedup();
        c.excluded = excluded;
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
    /// How many hinges across welded seams are held flat.
    pub fn seam_hinge_count(&self) -> usize {
        self.seam_hinges.len()
    }
    /// The hinges across welded seams: the seam edge's particles, then the one opposite it in
    /// each triangle.
    pub fn seam_hinges(&self) -> impl Iterator<Item = [usize; 4]> + '_ {
        self.seam_hinges
            .iter()
            .map(|h| [h.u as usize, h.v as usize, h.p as usize, h.q as usize])
    }
    pub fn bend_links(&self) -> impl Iterator<Item = (usize, usize, f64)> + '_ {
        self.bend
            .iter()
            .map(|l| (l.a as usize, l.b as usize, l.rest))
    }
    /// The bias edges and the hinges across them.
    pub fn shear_links(&self) -> impl Iterator<Item = (usize, usize, f64)> + '_ {
        self.shear
            .iter()
            .map(|l| (l.a as usize, l.b as usize, l.rest))
    }
    /// The widest gap (m) of every seam still open, by seam, in seam order.
    pub fn open_seam_gaps(&self) -> Vec<(u32, f64)> {
        let mut worst: BTreeMap<u32, f64> = BTreeMap::new();
        for (l, &g) in self.stitches.iter().zip(&self.stitch_group) {
            let d = self.x[l.a as usize].distance(self.x[l.b as usize]);
            let w = worst.entry(g).or_insert(0.0);
            *w = w.max(d);
        }
        worst.into_iter().collect()
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
    /// The fabric edges along every welded seam, so a drape can be measured along its seams.
    pub fn seam_edges(&self) -> &[(u32, u32)] {
        &self.seam_edges
    }
    /// The fabric's typical edge length (m), 0 for a cloth with no edges.
    pub fn spacing(&self) -> f64 {
        self.spacing
    }

    /// Welds every seam whose stitches are all within `gap` (m): each stitched pair merges
    /// into one particle and the constraints are rebuilt on the welded mesh, so a closed seam
    /// behaves like continuous fabric with zero gap. Returns the seams welded.
    pub fn weld_closed(&mut self, gap: f64) -> Vec<u32> {
        let closed: Vec<u32> = self
            .open_seam_gaps()
            .into_iter()
            .filter(|&(_, d)| d <= gap)
            .map(|(g, _)| g)
            .collect();
        if closed.is_empty() {
            return closed;
        }
        let (now, later): (Vec<_>, Vec<_>) = std::mem::take(&mut self.stitches)
            .into_iter()
            .zip(std::mem::take(&mut self.stitch_group))
            .partition(|(_, g)| closed.binary_search(g).is_ok());
        (self.stitches, self.stitch_group) = later.into_iter().unzip();
        self.merge(now.into_iter().map(|(l, _)| l));
        closed
    }

    /// Welds every seam, closed or not.
    pub fn weld_stitches(&mut self) {
        self.weld_closed(f64::INFINITY);
    }

    fn merge(&mut self, stitches: impl Iterator<Item = Link>) {
        let mut map: Vec<u32> = (0..self.x.len() as u32).collect();
        fn root(map: &[u32], mut k: u32) -> u32 {
            while map[k as usize] != k {
                k = map[k as usize];
            }
            k
        }
        for s in stitches {
            let (a, b) = (root(&map, s.a) as usize, root(&map, s.b) as usize);
            self.welded[a] = true;
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
            // Momentum is kept: the merged particle moves as the two did together.
            self.v[a] = if wa == 0.0 || wb == 0.0 {
                DVec3::ZERO
            } else {
                (self.v[a] / wa + self.v[b] / wb) / (1.0 / wa + 1.0 / wb)
            };
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
        // The pairs left alone across seams follow the merged particles too, so a corner where
        // seams meet keeps every exclusion it had.
        self.excluded = std::mem::take(&mut self.excluded)
            .into_iter()
            .map(|(a, b)| edge_key(m(a), m(b)))
            .filter(|(a, b)| a != b)
            .collect();
        self.excluded.sort_unstable();
        self.excluded.dedup();
        // Seams still open follow their merged particles; a pair this weld has already joined
        // (through a seam that meets it at a corner) is done.
        let open: Vec<(Link, u32)> = std::mem::take(&mut self.stitches)
            .into_iter()
            .zip(std::mem::take(&mut self.stitch_group))
            .map(|(l, g)| {
                (
                    Link {
                        a: m(l.a),
                        b: m(l.b),
                        rest: l.rest,
                    },
                    g,
                )
            })
            .filter(|(l, _)| l.a != l.b)
            .collect();
        (self.stitches, self.stitch_group) = open.into_iter().unzip();
        for t in &mut self.triangles {
            *t = t.map(m);
        }
        // Rest lengths of the welded mesh's edges (and the links across the bias). Where the
        // two sides of a seam differ (ease), the shared edge takes their mean.
        let mut sums: HashMap<(u32, u32), (f64, f64)> = HashMap::new();
        for l in self.stretch.iter().chain(&self.shear) {
            let (a, b) = (m(l.a), m(l.b));
            if a != b {
                let e = sums.entry(edge_key(a, b)).or_insert((0.0, 0.0));
                e.0 += l.rest;
                e.1 += 1.0;
            }
        }
        let rest_of: HashMap<(u32, u32), f64> =
            sums.into_iter().map(|(k, (sum, n))| (k, sum / n)).collect();
        let mut seen = HashSet::new();
        let mut remap = |links: Vec<Link>| -> Vec<Link> {
            links
                .into_iter()
                .map(|l| (m(l.a), m(l.b)))
                .filter(|&(a, b)| a != b && seen.insert(edge_key(a, b)))
                .map(|(a, b)| Link {
                    a,
                    b,
                    rest: rest_of[&edge_key(a, b)],
                })
                .collect()
        };
        self.stretch = remap(std::mem::take(&mut self.stretch));
        self.shear = remap(std::mem::take(&mut self.shear));
        let on_bias: HashSet<(u32, u32)> = self.shear.iter().map(|l| edge_key(l.a, l.b)).collect();
        let welded = &self.welded;
        self.seam_edges = self
            .stretch
            .iter()
            .filter(|l| welded[l.a as usize] && welded[l.b as usize])
            .map(|l| edge_key(l.a, l.b))
            .collect();
        self.seam_edges.sort_unstable();
        // Hinges that were there keep their rest (those across the bias are still among the
        // shear links). A hinge across the seam rests where the pattern lays flat, not where
        // the fabric happens to be as it welds.
        let remembered = |links: &[Link]| -> HashMap<(u32, u32), f64> {
            links
                .iter()
                .map(|l| (edge_key(m(l.a), m(l.b)), l.rest))
                .collect()
        };
        let old = remembered(&self.bend);
        let (mut bend, mut seam_hinges) = (Vec::new(), Vec::new());
        for h in hinges(&self.triangles) {
            let key = edge_key(h.p, h.q);
            if on_bias.contains(&key) {
                continue;
            }
            match old.get(&key) {
                Some(&rest) => bend.push(Link {
                    a: h.p,
                    b: h.q,
                    rest,
                }),
                // Across a seam, this weld's or an earlier one's: a hinge held flat.
                None => seam_hinges.push(h),
            }
        }
        self.bend = bend;
        self.seam_hinges = seam_hinges;
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
        // The two triangles now share the welded edge, so a seam hinge spans it.
        assert_eq!(c.bend_link_count(), 0);
        let hinges: Vec<_> = c.seam_hinges().collect();
        assert_eq!(hinges.len(), 1);
        assert_eq!(&hinges[0][..2], &[0, 1], "about the welded edge");
        assert_eq!(c.seam_edges(), &[(0, 1)], "the welded edge is the seam");
    }

    /// One lattice cell: a 10 cm square split along one diagonal.
    fn cell() -> Panel {
        let flat = vec![
            DVec2::new(0.0, 0.0),
            DVec2::new(0.1, 0.0),
            DVec2::new(0.1, 0.1),
            DVec2::new(0.0, 0.1),
        ];
        Panel {
            positions: flat.iter().map(|p| p.extend(0.0)).collect(),
            flat: Some(flat),
            triangles: vec![[0, 1, 2], [0, 2, 3]],
        }
    }

    #[test]
    fn a_grain_panel_holds_the_warp_and_weft_and_shears_on_the_bias() {
        // Grain along x: the four sides are structural; the diagonal, and the hinge across
        // it (the other diagonal), shear.
        let mut b = ClothBuilder::new(0.15);
        b.add_grain_panel(&cell(), 1.0, DVec2::X);
        let c = b.build();
        assert_eq!(c.stretch_links().count(), 4);
        assert_eq!(c.bend_link_count(), 0);
        let shear: Vec<_> = c.shear_links().collect();
        assert_eq!(shear.len(), 2);
        assert!(
            shear
                .iter()
                .all(|&(_, _, r)| (r - 0.1f64.hypot(0.1)).abs() < 1e-12)
        );
        // Grain on the diagonal: the sides are on the bias, the diagonal is structural and the
        // hinge across it bends.
        let mut b = ClothBuilder::new(0.15);
        b.add_grain_panel(&cell(), 1.0, DVec2::new(1.0, 1.0));
        let c = b.build();
        assert_eq!(c.stretch_links().count(), 1);
        assert_eq!(c.shear_links().count(), 4);
        assert_eq!(c.bend_link_count(), 1);
        // No grain: everything structural, as before.
        let mut b = ClothBuilder::new(0.15);
        b.add_panel(&cell(), 1.0);
        let c = b.build();
        assert_eq!((c.stretch_links().count(), c.shear_links().count()), (5, 0));
        // Two irregular triangles (the band along an outline) sharing an edge on the bias:
        // the edge shears, but the hinge across it bends like any fabric.
        let band = Panel {
            positions: vec![
                DVec3::ZERO,
                DVec3::new(0.11, 0.07, 0.0),
                DVec3::new(0.02, 0.1, 0.0),
                DVec3::new(0.13, -0.03, 0.0),
            ],
            flat: Some(vec![
                DVec2::new(0.0, 0.0),
                DVec2::new(0.11, 0.07),
                DVec2::new(0.02, 0.1),
                DVec2::new(0.13, -0.03),
            ]),
            triangles: vec![[0, 1, 2], [0, 3, 1]],
        };
        let mut b = ClothBuilder::new(0.15);
        b.add_grain_panel(&band, 1.0, DVec2::X);
        let c = b.build();
        assert_eq!(c.bend_link_count(), 1, "the hinge bends");
        assert!(
            c.shear_links().count() >= 1,
            "the shared edge is on the bias"
        );
    }

    #[test]
    fn welding_keeps_the_bias_links_and_bends_across_the_seam() {
        // Two cells side by side on the x grain, sewn along the edge between them.
        let mut b = ClothBuilder::new(0.15);
        let p = b.add_grain_panel(&cell(), 1.0, DVec2::X);
        let moved = Panel {
            positions: cell()
                .positions
                .iter()
                .map(|q| *q + DVec3::X * 0.1)
                .collect(),
            ..cell()
        };
        let q = b.add_grain_panel(&moved, 1.0, DVec2::X);
        b.stitch((p, 1), (q, 0));
        b.stitch((p, 2), (q, 3));
        let mut c = b.build();
        c.weld_stitches();
        assert_eq!(c.stretch_links().count(), 7, "the shared edge once");
        assert_eq!(c.shear_links().count(), 4, "each cell's two diagonals");
        // The one hinge across the welded (structural) edge is held flat.
        assert_eq!(c.seam_hinge_count(), 1);
        assert_eq!(c.bend_link_count(), 0);
    }

    #[test]
    fn a_welded_seam_rests_where_the_pattern_lays_flat() {
        // Two right triangles sewn along their vertical legs (corners 0 and 2), the second
        // folded up 90° in 3D: their far corners are 14 cm apart as they weld, 20 cm apart
        // with the pattern laid flat.
        let mut b = ClothBuilder::new(0.15);
        let p = b.add_panel(&tri_panel(1.0, DVec3::ZERO), 1.0);
        let folded = Panel {
            positions: vec![
                DVec3::ZERO,
                DVec3::new(0.0, 0.0, 0.1),
                DVec3::new(0.0, 0.1, 0.0),
            ],
            ..tri_panel(1.0, DVec3::ZERO)
        };
        let q = b.add_panel(&folded, 1.0);
        b.stitch((p, 0), (q, 0));
        b.stitch((p, 2), (q, 2));
        let mut c = b.build();
        c.weld_stitches();
        // One hinge, about the welded leg, between the two far corners; no distance link.
        let hinges: Vec<_> = c.seam_hinges().collect();
        assert_eq!(hinges, vec![[0, 2, 1, 4]]);
        assert_eq!(c.bend_link_count(), 0);
    }

    #[test]
    fn seams_weld_one_at_a_time_as_each_closes_and_the_rest_follow() {
        // A and B lie on each other, sewn as seam 1; C is sewn to B as seam 2 but 5 cm away.
        // The two seams meet at corner 0.
        let mut b = ClothBuilder::new(0.15);
        let a = b.add_panel(&tri_panel(1.0, DVec3::ZERO), 1.0);
        let p = b.add_panel(&tri_panel(1.0, DVec3::ZERO), 1.0);
        let c = b.add_panel(&tri_panel(1.0, DVec3::new(0.0, 0.0, 0.05)), 1.0);
        b.stitch_in((a, 0), (p, 0), 1);
        b.stitch_in((a, 2), (p, 2), 1);
        b.stitch_in((p, 0), (c, 0), 2);
        b.stitch_in((p, 1), (c, 1), 2);
        let mut cloth = b.build();
        assert_eq!(cloth.open_seam_gaps(), vec![(1, 0.0), (2, 0.05)]);
        assert_eq!(cloth.weld_closed(0.002), vec![1]);
        assert!(cloth.has_open_stitches(), "seam 2 is still open");
        // B's corner 0 merged into A's: seam 2's first stitch now reaches A's corner.
        let pairs: Vec<_> = cloth.stitch_pairs().collect();
        assert_eq!(pairs, vec![(0, 6), (4, 7)]);
        assert_eq!(cloth.open_seam_gaps(), vec![(2, 0.05)]);
        assert_eq!(cloth.weld_closed(0.002), vec![], "not closed yet");
        // Seam 2's exclusions now reach A's corner, which took over B's.
        assert!(cloth.excluded.contains(&(0, 6)), "{:?}", cloth.excluded);
        assert!(
            cloth
                .excluded
                .iter()
                .all(|&(a, b)| a < b && cloth.is_alive(a as usize))
        );
        cloth.weld_stitches();
        assert!(!cloth.has_open_stitches());
        assert_eq!((0..cloth.len()).filter(|&i| cloth.is_alive(i)).count(), 5);
        assert_eq!(cloth.topology_version(), 2);
    }
}
