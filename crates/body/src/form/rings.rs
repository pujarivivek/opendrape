//! A form's surface as horizontal rings, and the closed triangle mesh built from them.

use super::file::FormFile;
use crate::BodyMesh;
use glam::{DVec2, DVec3};
use std::f64::consts::TAU;

/// Ring `i` is at height `y[i]`, centred on (0, `zc[i]`), with radius `r[i][k]` (metres) at angle
/// k·π/(half − 1) from centre front (+z) towards the form's left (+x). Angles past π mirror onto
/// −x, so the surface is exactly symmetric.
#[derive(Clone, Debug, PartialEq)]
pub struct Rings {
    pub y: Vec<f64>,
    pub zc: Vec<f64>,
    pub r: Vec<Vec<f64>>,
}

impl Rings {
    pub fn from_file(f: &FormFile) -> Self {
        Self {
            y: f.rings.iter().map(|r| r.y).collect(),
            zc: f.rings.iter().map(|r| r.zc).collect(),
            r: f.rings
                .iter()
                .map(|r| r.r.iter().map(|mm| mm / 1000.0).collect())
                .collect(),
        }
    }

    pub fn len(&self) -> usize {
        self.y.len()
    }

    pub fn is_empty(&self) -> bool {
        self.y.is_empty()
    }

    /// Radii per side, centre front to centre back inclusive.
    pub fn half(&self) -> usize {
        self.r[0].len()
    }

    /// Samples around the full ring.
    pub fn around(&self) -> usize {
        2 * (self.half() - 1)
    }

    /// Ring `i`, sample `j` of `around()` (wraps), counted from centre front towards +x.
    pub fn vertex(&self, i: usize, j: usize) -> DVec3 {
        let (k, m) = (self.half() - 1, self.around());
        let j = j % m;
        if j > k {
            let p = self.vertex(i, m - j);
            return DVec3::new(-p.x, p.y, p.z);
        }
        let a = TAU * j as f64 / m as f64;
        let r = self.r[i][j];
        let x = if j == k { 0.0 } else { r * a.sin() };
        DVec3::new(x, self.y[i], self.zc[i] + r * a.cos())
    }

    /// Closed mesh, counter-clockwise seen from outside. Ring vertices first (`i·around + j`),
    /// then the bottom and top centre. Quads on the left side are split along one diagonal and
    /// their mirror images on the right along the other, so the mesh is exactly symmetric.
    pub fn mesh(&self) -> BodyMesh {
        let (n, m) = (self.len(), self.around());
        let mut positions: Vec<glam::Vec3> = (0..n)
            .flat_map(|i| (0..m).map(move |j| self.vertex(i, j).as_vec3()))
            .collect();
        let bottom = positions.len() as u32;
        positions.push(DVec3::new(0.0, self.y[0], self.zc[0]).as_vec3());
        positions.push(DVec3::new(0.0, self.y[n - 1], self.zc[n - 1]).as_vec3());
        let top = bottom + 1;
        let id = |i: usize, j: usize| (i * m + j % m) as u32;
        let mut triangles = Vec::with_capacity(2 * m * n);
        for i in 0..n - 1 {
            for j in 0..m {
                let (a, b, c, d) = (id(i, j), id(i, j + 1), id(i + 1, j + 1), id(i + 1, j));
                if j < m / 2 {
                    triangles.extend([[a, b, c], [a, c, d]]);
                } else {
                    triangles.extend([[a, b, d], [b, c, d]]);
                }
            }
        }
        for j in 0..m {
            triangles.push([bottom, id(0, j + 1), id(0, j)]);
            triangles.push([top, id(n - 1, j), id(n - 1, j + 1)]);
        }
        BodyMesh {
            positions,
            triangles,
        }
    }

    /// The surface point at angle `phi` (radians from centre front towards +x; any value wraps)
    /// and height parameter `v` (0 bottom ring, 1 top ring), on the same triangles as `mesh`.
    pub fn point(&self, phi: f64, v: f64) -> DVec3 {
        let (n, m) = (self.len(), self.around());
        let f = v.clamp(0.0, 1.0) * (n - 1) as f64;
        let i = (f.floor() as usize).min(n - 2);
        let t = f - i as f64;
        let g = phi.rem_euclid(TAU) / TAU * m as f64;
        let j = (g.floor() as usize).min(m - 1);
        let s = g - j as f64;
        let (a, b, c, d) = (
            self.vertex(i, j),
            self.vertex(i, (j + 1) % m),
            self.vertex(i + 1, (j + 1) % m),
            self.vertex(i + 1, j),
        );
        if j < m / 2 {
            if s >= t {
                a * (1.0 - s) + b * (s - t) + c * t
            } else {
                a * (1.0 - t) + c * s + d * (t - s)
            }
        } else if s + t <= 1.0 {
            a * (1.0 - s - t) + b * s + d * t
        } else {
            b * (1.0 - t) + c * (s + t - 1.0) + d * (1.0 - s)
        }
    }

    /// Outward unit normal at (`phi`, `v`), by central differences on the surface. `phi` wraps
    /// like in `point`.
    pub fn normal(&self, phi: f64, v: f64) -> DVec3 {
        let (dphi, dv) = (1e-3, 1e-3);
        let along = self.point(phi + dphi, v) - self.point(phi - dphi, v);
        let up = self.point(phi, (v + dv).min(1.0)) - self.point(phi, (v - dv).max(0.0));
        along.cross(up).normalize_or_zero()
    }

    /// What a tape measure reads around ring `i`: the convex-hull perimeter, metres.
    pub fn girth(&self, i: usize) -> f64 {
        crate::measure::hull_perimeter(
            (0..self.around())
                .map(|j| {
                    let p = self.vertex(i, j);
                    DVec2::new(p.x, p.z)
                })
                .collect(),
        )
    }

    /// Height at fractional ring parameter `v`.
    pub fn y_at(&self, v: f64) -> f64 {
        let f = v.clamp(0.0, 1.0) * (self.len() - 1) as f64;
        let i = (f.floor() as usize).min(self.len() - 2);
        self.y[i] + (self.y[i + 1] - self.y[i]) * (f - i as f64)
    }

    /// The same surface with `half` radii per side, interpolated in angle.
    pub fn with_half_angles(&self, half: usize) -> Rings {
        let last = (self.half() - 1) as f64;
        let r = self
            .r
            .iter()
            .map(|ring| {
                (0..half)
                    .map(|k| {
                        let s = k as f64 * last / (half - 1) as f64;
                        let i = (s.floor() as usize).min(self.half() - 2);
                        let t = s - i as f64;
                        ring[i] * (1.0 - t) + ring[i + 1] * t
                    })
                    .collect()
            })
            .collect();
        Rings {
            y: self.y.clone(),
            zc: self.zc.clone(),
            r,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::form::{ANGLES, fixture};
    use crate::{boundary_edge_count, girth_at};
    use glam::Vec3;
    use std::f64::consts::{FRAC_PI_2, PI};

    fn rings() -> Rings {
        Rings::from_file(&fixture::torso())
    }

    fn signed_volume(m: &BodyMesh) -> f64 {
        m.triangles
            .iter()
            .map(|t| {
                let [a, b, c] = t.map(|i| m.positions[i as usize].as_dvec3());
                a.dot(b.cross(c)) / 6.0
            })
            .sum()
    }

    /// `p` lies on triangle (a, b, c), within 1 µm.
    fn on_triangle(p: DVec3, a: DVec3, b: DVec3, c: DVec3) -> bool {
        let n = (b - a).cross(c - a);
        let area = n.length_squared();
        let wa = (b - p).cross(c - p).dot(n) / area;
        let wb = (c - p).cross(a - p).dot(n) / area;
        let wc = 1.0 - wa - wb;
        (p - a).dot(n).abs() / n.length() < 1e-6 && wa > -1e-6 && wb > -1e-6 && wc > -1e-6
    }

    /// The mesh is closed, faces outward, and every vertex and triangle has a mirror image.
    fn assert_closed_outward_and_mirror_symmetric(r: &Rings) {
        let mesh = r.mesh();
        let m = r.around();
        assert_eq!(mesh.triangles.len(), 2 * m * r.len());
        assert_eq!(boundary_edge_count(&mesh), 0);
        assert!(signed_volume(&mesh) > 0.0, "triangles face outward");
        for i in 0..r.len() {
            for j in 1..m {
                let (p, q) = (r.vertex(i, j), r.vertex(i, m - j));
                assert_eq!((p.x, p.y, p.z), (-q.x, q.y, q.z), "ring {i} sample {j}");
            }
        }
        // The triangulation is mirrored too, so drapes have no left/right bias.
        let key = |p: Vec3| ((p.x + 0.0).to_bits(), p.y.to_bits(), p.z.to_bits());
        let sorted = |mut k: [(u32, u32, u32); 3]| {
            k.sort();
            k
        };
        let tris: std::collections::HashSet<_> = mesh
            .triangles
            .iter()
            .map(|t| sorted(t.map(|i| key(mesh.positions[i as usize]))))
            .collect();
        for t in &mesh.triangles {
            let mirrored = t.map(|i| {
                let p = mesh.positions[i as usize];
                key(Vec3::new(-p.x, p.y, p.z))
            });
            assert!(
                tris.contains(&sorted(mirrored)),
                "triangle {t:?} has no mirror image"
            );
        }
    }

    #[test]
    fn mesh_is_closed_outward_and_mirror_symmetric() {
        let r = rings();
        assert_eq!(r.around(), 96);
        assert_closed_outward_and_mirror_symmetric(&r);
    }

    #[test]
    fn the_shipped_forms_build_closed_mirror_symmetric_meshes() {
        // The real shapes end in a slanted neck cut, where the top rings' front radii shrink.
        for (name, json) in [
            (
                "women",
                include_str!("../../../../assets/forms/women-torso.form.json"),
            ),
            (
                "men",
                include_str!("../../../../assets/forms/men-torso.form.json"),
            ),
        ] {
            let file = FormFile::from_json(json).expect(name);
            let r = Rings::from_file(&file);
            assert_eq!(r.len(), file.rings.len(), "{name}");
            assert_eq!(r.half(), ANGLES, "{name}");
            assert_closed_outward_and_mirror_symmetric(&r);
            let low = r.with_half_angles(33);
            assert_closed_outward_and_mirror_symmetric(&low);
        }
    }

    #[test]
    fn station_girth_equals_the_tape_measure_slice() {
        let r = rings();
        let mesh = r.mesh();
        for i in [12, 33, 51, 75] {
            let slice = f64::from(girth_at(&mesh, r.y[i] as f32, 10.0));
            assert!(
                (slice - r.girth(i)).abs() < 1e-4,
                "ring {i}: slice {slice} vs ring {}",
                r.girth(i)
            );
        }
    }

    #[test]
    fn surface_points_lie_on_the_mesh_triangles() {
        let r = rings();
        let mesh = r.mesh();
        let m = r.around();
        for step in 0..997 {
            let phi = (step as f64 * 0.0371) % TAU;
            let v = (step as f64 * 0.618_033_988_7) % 1.0;
            let p = r.point(phi, v);
            let i = ((v * (r.len() - 1) as f64).floor() as usize).min(r.len() - 2);
            let j = ((phi / TAU * m as f64).floor() as usize).min(m - 1);
            let quad = 2 * (i * m + j);
            let on = |t: usize| {
                let [a, b, c] = mesh.triangles[t].map(|k| mesh.positions[k as usize].as_dvec3());
                on_triangle(p, a, b, c)
            };
            assert!(
                on(quad) || on(quad + 1),
                "({phi}, {v}) is off its quad's triangles"
            );
        }
    }

    #[test]
    fn grid_points_are_the_vertices_and_mirrors_are_exact() {
        let r = rings();
        assert!(
            r.point(TAU * 7.0 / 96.0, 33.0 / 80.0)
                .distance(r.vertex(33, 7))
                < 1e-12
        );
        for step in 0..500 {
            let (phi, v) = ((step as f64 * 0.0213) % PI, (step as f64 * 0.377) % 1.0);
            let (p, q) = (r.point(phi, v), r.point(TAU - phi, v));
            assert!(
                (p.x + q.x).abs() < 1e-9 && (p.y - q.y).abs() < 1e-9 && (p.z - q.z).abs() < 1e-9,
                "({phi}, {v}): {p} vs {q}"
            );
        }
    }

    #[test]
    fn any_phi_wraps_into_one_turn() {
        // Sampled tape lines cross 2π (a front neckline runs from the right side through centre
        // front), so negative and past-a-turn angles must land on the same surface point.
        let r = rings();
        for step in 0..200 {
            let (phi, v) = ((step as f64 * 0.0313) % TAU, (step as f64 * 0.149) % 1.0);
            let base = r.point(phi, v);
            for shifted in [phi + TAU, phi - TAU, phi + 3.0 * TAU, phi - 2.0 * TAU] {
                let p = r.point(shifted, v);
                assert!(
                    p.distance(base) < 1e-9,
                    "({phi} -> {shifted}, {v}): {p} vs {base}"
                );
            }
            let n = r.normal(phi, v);
            assert!(
                r.normal(phi + TAU, v).distance(n) < 1e-9,
                "normal ({phi}, {v})"
            );
            assert!(
                r.normal(phi - TAU, v).distance(n) < 1e-9,
                "normal ({phi}, {v})"
            );
        }
        for v in [0.0, 0.3, 0.75, 1.0] {
            assert!(r.point(-0.1, v).distance(r.point(TAU - 0.1, v)) < 1e-12);
            assert!(r.point(TAU, v).distance(r.point(0.0, v)) < 1e-12);
        }
        // Across centre front the surface is continuous: a step of 0.02 rad moves the point by
        // about 0.02 · radius, whichever side of 2π it starts on.
        let (a, b) = (r.point(TAU - 0.01, 0.4), r.point(TAU + 0.01, 0.4));
        assert!(a.distance(b) < 0.02 * 0.3, "{a} vs {b}");
        assert!(r.point(TAU + 0.01, 0.4).distance(r.point(0.01, 0.4)) < 1e-12);
        // A tape point just below zero is on the right side (−x), just above on the left (+x).
        assert!(r.point(-0.2, 0.4).x < 0.0 && r.point(0.2, 0.4).x > 0.0);
    }

    #[test]
    fn normals_point_outward() {
        let r = rings();
        for (phi, v) in [(0.0, 0.4), (FRAC_PI_2, 0.5), (PI, 0.3), (4.0, 0.6)] {
            let p = r.point(phi, v);
            let out = DVec3::new(p.x, 0.0, p.z - r.zc[(v * 80.0) as usize]).normalize();
            assert!(r.normal(phi, v).dot(out) > 0.8, "({phi}, {v})");
        }
    }

    #[test]
    fn low_quality_keeps_the_shape_with_fewer_triangles() {
        let r = rings();
        let low = r.with_half_angles(33);
        assert_eq!(low.around(), 64);
        assert_eq!(low.mesh().triangles.len(), 2 * 64 * r.len());
        assert!((low.girth(33) - r.girth(33)).abs() < 0.002);
        assert_eq!(low.y_at(0.5), r.y_at(0.5));
    }
}
