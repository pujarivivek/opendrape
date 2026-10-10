//! The stand a form sits on, drawn but never collided: a neck cap that hugs the slanted cut, a
//! rod and knob above it, a pole, and a round base on the floor (y = 0). Every piece is a
//! closed, outward-wound mesh of its own; `stand` appends them into one.

use super::file::FormFile;
use super::rings::Rings;
use crate::BodyMesh;
use crate::measure::convex_hull;
use glam::{DVec2, DVec3};
use std::f64::consts::TAU;

// Sizes, metres.
const BASE_RADIUS: f64 = 0.164;
const BASE_HEIGHT: f64 = 0.026;
const POLE_RADIUS: f64 = 0.013;
const ROD_RADIUS: f64 = 0.006;
const ROD_HEIGHT: f64 = 0.024;
const KNOB_RADIUS: f64 = 0.016;
const KNOB_HEIGHT: f64 = 0.026;
/// The cap stands this far above the cut; its collar hangs this far below, hugging the neck.
const CAP_ABOVE: f64 = 0.004;
const COLLAR_BELOW: f64 = 0.022;
/// The cap's outline is the hull of the torso vertices this close to the cut, pushed out by
/// `CAP_MARGIN` from its centroid.
const NEAR_CUT: f64 = 0.002;
const CAP_MARGIN: f64 = 0.0025;
/// Points around the cap's outline.
const CAP_SIDES: usize = 64;
/// Sides of the round pieces.
const SIDES: u32 = 24;

/// The plane the neck is cut by, as a frame: `u` runs across the form (+x), `w` up the slope
/// (backwards and up) and `n` is the normal pointing away from the form, tilted towards the
/// front. (`u`, `w`, `n`) is right-handed, so an outline counter-clockwise in (u, w) faces `n`.
struct Plane {
    origin: DVec3,
    u: DVec3,
    w: DVec3,
    n: DVec3,
}

impl Plane {
    /// The plane through the pole at `cut_y`, lower at the front by the file's tilt: it cuts
    /// away the points with (y − cut_y) + (z − pole_z)·tan(tilt) > 0.
    fn new(file: &FormFile, cut_y: f64) -> Self {
        let [x, z] = file.stand.pole_xz;
        let (s, c) = file.stand.neck_cut.tilt_deg.to_radians().sin_cos();
        Self {
            origin: DVec3::new(x, cut_y, z),
            u: DVec3::X,
            w: DVec3::new(0.0, s, -c),
            n: DVec3::new(0.0, c, s),
        }
    }

    /// Signed distance from the plane, positive on the cut-away side.
    fn height(&self, p: DVec3) -> f64 {
        (p - self.origin).dot(self.n)
    }

    /// Where `p` lands on the plane, in (u, w).
    fn coords(&self, p: DVec3) -> DVec2 {
        let d = p - self.origin;
        DVec2::new(d.dot(self.u), d.dot(self.w))
    }

    /// The point at `c` (u, w) lifted `height` along the normal.
    fn point(&self, c: DVec2, height: f64) -> DVec3 {
        self.origin + self.u * c.x + self.w * c.y + self.n * height
    }
}

/// Height of the neck cut on the sized form. Everything above the back neck moves rigidly when a
/// form is resized, so the cut moves with the top ring.
fn cut_height(file: &FormFile, base: &Rings, rings: &Rings) -> f64 {
    let top = rings.len() - 1;
    file.stand.neck_cut.y + (rings.y[top] - base.y[top])
}

/// A closed, outward-wound cylinder standing on `base`, with `SIDES` sides.
fn cylinder(base: DVec3, radius: f64, height: f64) -> BodyMesh {
    let mut m = BodyMesh {
        positions: vec![],
        triangles: vec![],
    };
    for level in [0.0, height] {
        for j in 0..SIDES {
            let a = TAU * f64::from(j) / f64::from(SIDES);
            m.positions
                .push((base + DVec3::new(radius * a.sin(), level, radius * a.cos())).as_vec3());
        }
    }
    m.positions.push(base.as_vec3());
    m.positions.push((base + DVec3::Y * height).as_vec3());
    let (bottom, top) = (2 * SIDES, 2 * SIDES + 1);
    for j in 0..SIDES {
        let k = (j + 1) % SIDES;
        let (a, b, c, d) = (j, k, SIDES + k, SIDES + j);
        m.triangles
            .extend([[a, b, c], [a, c, d], [bottom, b, a], [top, d, c]]);
    }
    m
}

/// A closed prism along the plane's normal, from `above` the plane down to `below` it, on an
/// outline that runs counter-clockwise in (u, w). Vertices: the top outline, the bottom outline,
/// then the top and bottom centres.
fn prism(plane: &Plane, outline: &[DVec2], above: f64, below: f64) -> BodyMesh {
    let n = outline.len() as u32;
    let mut m = BodyMesh {
        positions: vec![],
        triangles: vec![],
    };
    for height in [above, -below] {
        m.positions
            .extend(outline.iter().map(|&c| plane.point(c, height).as_vec3()));
    }
    let centre = outline.iter().sum::<DVec2>() / f64::from(n);
    m.positions.push(plane.point(centre, above).as_vec3());
    m.positions.push(plane.point(centre, -below).as_vec3());
    let (top, bottom) = (2 * n, 2 * n + 1);
    for j in 0..n {
        let k = (j + 1) % n;
        let (a, b, c, d) = (j, k, n + k, n + j);
        m.triangles
            .extend([[top, a, b], [bottom, c, d], [a, d, c], [a, c, b]]);
    }
    m
}

/// Area centroid of a counter-clockwise polygon.
fn centroid(poly: &[DVec2]) -> DVec2 {
    let o = poly[0];
    let (mut twice_area, mut sum) = (0.0, DVec2::ZERO);
    for (i, &p) in poly.iter().enumerate() {
        let (p, q) = (p - o, poly[(i + 1) % poly.len()] - o);
        let cross = p.perp_dot(q);
        twice_area += cross;
        sum += (p + q) * cross;
    }
    o + sum / (3.0 * twice_area)
}

/// `count` points evenly spread along the closed polygon's perimeter, starting at its first
/// corner. The polygon must have no repeated corners.
fn resample(poly: &[DVec2], count: usize) -> Vec<DVec2> {
    let n = poly.len();
    let edge = |i: usize| poly[i].distance(poly[(i + 1) % n]);
    let total: f64 = (0..n).map(edge).sum();
    let (mut i, mut start) = (0, 0.0);
    (0..count)
        .map(|k| {
            let at = total * k as f64 / count as f64;
            while i + 1 < n && start + edge(i) < at {
                start += edge(i);
                i += 1;
            }
            poly[i].lerp(poly[(i + 1) % n], (at - start) / edge(i))
        })
        .collect()
}

/// The cap's outline in (u, w): the hull of the torso vertices within `NEAR_CUT` of the plane,
/// pushed `CAP_MARGIN` out from its centroid, resampled to `CAP_SIDES` even points. A form whose
/// rings stop short of the plane has no such vertices: its top ring stands in for them.
fn cap_outline(plane: &Plane, rings: &Rings, torso: &BodyMesh) -> Vec<DVec2> {
    let near: Vec<DVec2> = torso
        .positions
        .iter()
        .map(|p| p.as_dvec3())
        .filter(|&p| plane.height(p).abs() <= NEAR_CUT)
        .map(|p| plane.coords(p))
        .collect();
    let mut hull = convex_hull(near);
    if hull.is_empty() {
        let top = rings.len() - 1;
        hull = convex_hull(
            (0..rings.around())
                .map(|j| plane.coords(rings.vertex(top, j)))
                .collect(),
        );
    }
    if hull.is_empty() {
        return hull;
    }
    let c = centroid(&hull);
    let pushed: Vec<DVec2> = hull
        .iter()
        .map(|&p| p + (p - c).normalize_or_zero() * CAP_MARGIN)
        .collect();
    resample(&pushed, CAP_SIDES)
}

/// The five closed pieces of the stand.
struct Parts {
    cap: BodyMesh,
    rod: BodyMesh,
    knob: BodyMesh,
    pole: BodyMesh,
    base: BodyMesh,
}

fn parts(file: &FormFile, base: &Rings, rings: &Rings, torso: &BodyMesh) -> Parts {
    let [px, pz] = file.stand.pole_xz;
    let plane = Plane::new(file, cut_height(file, base, rings));
    let outline = cap_outline(&plane, rings, torso);
    let cap = if outline.len() < 3 {
        BodyMesh {
            positions: vec![],
            triangles: vec![],
        }
    } else {
        prism(&plane, &outline, CAP_ABOVE, COLLAR_BELOW)
    };
    // Where the pole axis crosses the cap's top face. The face is slanted, so the rod's flat
    // end is sunk by the slope across its radius to meet it all round; the rod still stands
    // `ROD_HEIGHT` above the crossing.
    let tilt = file.stand.neck_cut.tilt_deg.to_radians();
    let crossing = plane.origin.y + CAP_ABOVE / tilt.cos();
    let sunk = ROD_RADIUS * tilt.tan();
    let rod = cylinder(
        DVec3::new(px, crossing - sunk, pz),
        ROD_RADIUS,
        ROD_HEIGHT + sunk,
    );
    let knob = cylinder(
        DVec3::new(px, crossing + ROD_HEIGHT, pz),
        KNOB_RADIUS,
        KNOB_HEIGHT,
    );
    let pole_height = rings.y[0] - BASE_HEIGHT;
    let pole = if pole_height > 0.0 {
        cylinder(DVec3::new(px, BASE_HEIGHT, pz), POLE_RADIUS, pole_height)
    } else {
        BodyMesh {
            positions: vec![],
            triangles: vec![],
        }
    };
    Parts {
        cap,
        rod,
        knob,
        pole,
        base: cylinder(DVec3::new(px, 0.0, pz), BASE_RADIUS, BASE_HEIGHT),
    }
}

/// The stand for `rings`, a resized copy of `base`, whose mesh is `torso`: the pieces appended
/// into one mesh, each of them closed.
pub(super) fn stand(file: &FormFile, base: &Rings, rings: &Rings, torso: &BodyMesh) -> BodyMesh {
    let p = parts(file, base, rings, torso);
    let mut out = BodyMesh {
        positions: vec![],
        triangles: vec![],
    };
    for part in [p.cap, p.rod, p.knob, p.pole, p.base] {
        let offset = out.positions.len() as u32;
        out.positions.extend(part.positions);
        out.triangles
            .extend(part.triangles.into_iter().map(|t| t.map(|i| i + offset)));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::boundary_edge_count;
    use crate::form::{Form, Measurements, Quality, fixture, resize};

    const WOMEN: &str = include_str!("../../../../assets/forms/women-torso.form.json");
    const MEN: &str = include_str!("../../../../assets/forms/men-torso.form.json");

    const WOMEN_EXTREME: [(&str, f64); 8] = [
        ("bust", 1050.0),
        ("under_bust", 900.0),
        ("waist", 820.0),
        ("hip", 1080.0),
        ("neck", 390.0),
        ("shoulder_length", 140.0),
        ("back_waist_length", 440.0),
        ("waist_to_hip", 210.0),
    ];
    const MEN_EXTREME: [(&str, f64); 7] = [
        ("chest", 1120.0),
        ("waist", 940.0),
        ("hip", 1120.0),
        ("neck", 420.0),
        ("shoulder_length", 165.0),
        ("back_waist_length", 500.0),
        ("waist_to_hip", 220.0),
    ];

    /// A form at one size: the base rings, the sized rings and the sized torso mesh.
    struct Sized {
        file: FormFile,
        base: Rings,
        rings: Rings,
        torso: BodyMesh,
    }

    /// `file` at its own size with `changes` (mm) applied.
    fn sized(file: FormFile, changes: &[(&str, f64)]) -> Sized {
        let mut size = Form::new(file.clone())
            .unwrap()
            .base_measurements(Quality::Standard);
        for &(m, mm) in changes {
            size.insert(m.to_string(), mm);
        }
        let base = Rings::from_file(&file);
        let rings = resize::resize(&file, &base, &size).unwrap();
        let torso = rings.mesh();
        Sized {
            file,
            base,
            rings,
            torso,
        }
    }

    fn real(json: &str, extreme: &[(&str, f64)]) -> Vec<(&'static str, Sized)> {
        let file = FormFile::from_json(json).unwrap();
        vec![
            ("own size", sized(file.clone(), &[])),
            ("extreme", sized(file, extreme)),
        ]
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

    fn assert_closed_and_outward(name: &str, m: &BodyMesh) {
        assert!(!m.triangles.is_empty(), "{name} is empty");
        assert_eq!(boundary_edge_count(m), 0, "{name} is open");
        assert!(signed_volume(m) > 0.0, "{name} faces inward");
    }

    fn lowest(m: &BodyMesh) -> f32 {
        m.positions.iter().map(|p| p.y).fold(f32::MAX, f32::min)
    }

    /// Whether `p` is inside the polygon (crossing-number test).
    fn inside(poly: &[DVec2], p: DVec2) -> bool {
        let mut hit = false;
        for i in 0..poly.len() {
            let (a, b) = (poly[i], poly[(i + 1) % poly.len()]);
            if (a.y > p.y) != (b.y > p.y) && p.x < a.x + (p.y - a.y) / (b.y - a.y) * (b.x - a.x) {
                hit = !hit;
            }
        }
        hit
    }

    /// The outline of a cap built by `prism`, in (u, w): its first `CAP_SIDES` vertices.
    fn top_outline(plane: &Plane, cap: &BodyMesh) -> Vec<DVec2> {
        cap.positions[..CAP_SIDES]
            .iter()
            .map(|p| plane.coords(p.as_dvec3()))
            .collect()
    }

    #[test]
    fn the_plane_cuts_away_what_the_file_says() {
        let file = fixture::torso();
        let plane = Plane::new(&file, 1.52);
        let tilt = 17.0_f64.to_radians();
        for (y, z) in [(1.60, 0.02), (1.40, -0.03), (1.52, 0.0), (1.50, 0.07)] {
            let cut_away = (y - 1.52) + z * tilt.tan() > 0.0;
            let h = plane.height(DVec3::new(0.01, y, z));
            assert_eq!(h > 0.0, cut_away, "({y}, {z})");
            assert!((h - ((y - 1.52) * tilt.cos() + z * tilt.sin())).abs() < 1e-12);
        }
        // The frame is orthonormal and right-handed, and lower at the front.
        assert!((plane.u.cross(plane.w) - plane.n).length() < 1e-12);
        assert!(plane.w.dot(plane.n).abs() < 1e-12 && plane.w.y > 0.0 && plane.w.z < 0.0);
        assert!(plane.height(plane.point(DVec2::new(0.3, -0.2), 0.0)).abs() < 1e-12);
        assert!(plane.point(DVec2::ZERO, 0.0).distance(plane.origin) < 1e-12);
    }

    #[test]
    fn the_cut_moves_with_the_top_ring() {
        let s = sized(
            FormFile::from_json(WOMEN).unwrap(),
            &[("back_waist_length", 440.0)],
        );
        let top = s.rings.len() - 1;
        let moved = s.rings.y[top] - s.base.y[top];
        assert!(moved.abs() > 0.01, "the top ring moved {moved}");
        assert!(
            (cut_height(&s.file, &s.base, &s.rings) - (s.file.stand.neck_cut.y + moved)).abs()
                < 1e-12
        );
        let own = sized(FormFile::from_json(WOMEN).unwrap(), &[]);
        assert_eq!(
            cut_height(&own.file, &own.base, &own.rings),
            own.file.stand.neck_cut.y
        );
    }

    #[test]
    fn the_prism_is_closed_outward_and_exactly_the_height_asked() {
        let plane = Plane::new(&fixture::torso(), 1.5);
        let outline: Vec<DVec2> = (0..CAP_SIDES)
            .map(|k| {
                let a = TAU * k as f64 / CAP_SIDES as f64;
                DVec2::new(0.05 * a.cos(), 0.03 * a.sin())
            })
            .collect();
        let m = prism(&plane, &outline, 0.004, 0.022);
        assert_closed_and_outward("prism", &m);
        assert_eq!(m.triangles.len(), 4 * CAP_SIDES);
        for (k, p) in m.positions.iter().enumerate() {
            let h = plane.height(p.as_dvec3());
            let want = if k < CAP_SIDES || k == 2 * CAP_SIDES {
                0.004
            } else {
                -0.022
            };
            assert!((h - want).abs() < 1e-6, "vertex {k}: {h}");
        }
    }

    #[test]
    fn a_cylinder_is_closed_outward_and_stands_on_its_base() {
        let m = cylinder(DVec3::new(0.1, 0.5, -0.2), 0.02, 0.3);
        assert_closed_and_outward("cylinder", &m);
        assert_eq!(lowest(&m), 0.5);
        let top = m.positions.iter().map(|p| p.y).fold(f32::MIN, f32::max);
        assert!((top - 0.8).abs() < 1e-6);
        for p in &m.positions[..2 * SIDES as usize] {
            let r = f64::from(p.x - 0.1).hypot(f64::from(p.z + 0.2));
            assert!((r - 0.02).abs() < 1e-6);
        }
    }

    #[test]
    fn resampling_spreads_points_evenly_along_the_outline() {
        let square = [
            DVec2::new(0.0, 0.0),
            DVec2::new(2.0, 0.0),
            DVec2::new(2.0, 2.0),
            DVec2::new(0.0, 2.0),
        ];
        let pts = resample(&square, 16);
        assert_eq!(pts.len(), 16);
        assert_eq!(pts[0], square[0]);
        assert!((pts[1] - DVec2::new(0.5, 0.0)).length() < 1e-12);
        assert!((pts[4] - DVec2::new(2.0, 0.0)).length() < 1e-12);
        assert!((pts[15] - DVec2::new(0.0, 0.5)).length() < 1e-12);
        assert!((centroid(&square) - DVec2::new(1.0, 1.0)).length() < 1e-12);
    }

    #[test]
    fn the_fixtures_stand_is_closed_and_on_the_floor() {
        // The fixture's rings stop short of its cut, so its cap is built on the top ring.
        let file = fixture::torso();
        let base = Rings::from_file(&file);
        let torso = base.mesh();
        let plane = Plane::new(&file, file.stand.neck_cut.y);
        assert!(
            !torso
                .positions
                .iter()
                .any(|p| plane.height(p.as_dvec3()).abs() <= NEAR_CUT),
            "the fixture's top ring is beyond 2 mm of its cut"
        );
        let p = parts(&file, &base, &base, &torso);
        for (name, m) in [
            ("cap", &p.cap),
            ("rod", &p.rod),
            ("knob", &p.knob),
            ("pole", &p.pole),
            ("base", &p.base),
        ] {
            assert_closed_and_outward(name, m);
        }
        let outline = top_outline(&plane, &p.cap);
        let top = base.len() - 1;
        for j in 0..base.around() {
            let c = plane.coords(base.vertex(top, j));
            assert!(
                inside(&outline, c),
                "top ring sample {j} is outside the cap"
            );
        }
        let all = stand(&file, &base, &base, &torso);
        assert_eq!(boundary_edge_count(&all), 0);
        assert_eq!(lowest(&all), 0.0);
        assert_eq!(
            all.triangles.len(),
            [&p.cap, &p.rod, &p.knob, &p.pole, &p.base]
                .iter()
                .map(|m| m.triangles.len())
                .sum::<usize>()
        );
    }

    #[test]
    fn every_piece_of_a_real_stand_is_closed_and_outward() {
        for (json, extreme) in [(WOMEN, &WOMEN_EXTREME[..]), (MEN, &MEN_EXTREME[..])] {
            for (case, s) in real(json, extreme) {
                let p = parts(&s.file, &s.base, &s.rings, &s.torso);
                for (name, m) in [
                    ("cap", &p.cap),
                    ("rod", &p.rod),
                    ("knob", &p.knob),
                    ("pole", &p.pole),
                    ("base", &p.base),
                ] {
                    assert_closed_and_outward(&format!("{} {case} {name}", s.file.id), m);
                }
                let all = stand(&s.file, &s.base, &s.rings, &s.torso);
                assert_eq!(boundary_edge_count(&all), 0, "{} {case}", s.file.id);
                assert_eq!(lowest(&all), 0.0, "{} {case}", s.file.id);
            }
        }
    }

    #[test]
    fn the_cap_hugs_the_cut_and_encloses_the_neck_there() {
        for (json, extreme) in [(WOMEN, &WOMEN_EXTREME[..]), (MEN, &MEN_EXTREME[..])] {
            for (case, s) in real(json, extreme) {
                let id = format!("{} {case}", s.file.id);
                let plane = Plane::new(&s.file, cut_height(&s.file, &s.base, &s.rings));
                let cap = parts(&s.file, &s.base, &s.rings, &s.torso).cap;
                assert_eq!(cap.positions.len(), 2 * CAP_SIDES + 2, "{id}");
                // 4 mm above the cut at the top, a 22 mm collar below it.
                for (k, p) in cap.positions.iter().enumerate() {
                    let h = plane.height(p.as_dvec3());
                    let want = if k < CAP_SIDES || k == 2 * CAP_SIDES {
                        CAP_ABOVE
                    } else {
                        -COLLAR_BELOW
                    };
                    assert!((h - want).abs() < 1e-6, "{id}: vertex {k} at {h}");
                }
                // The lowest point is at the front, below the cut; the highest at the back,
                // above it.
                let at = |best: fn(&&glam::Vec3, &&glam::Vec3) -> std::cmp::Ordering| {
                    *cap.positions.iter().min_by(best).unwrap()
                };
                let low = at(|a, b| a.y.total_cmp(&b.y));
                let high = at(|a, b| b.y.total_cmp(&a.y));
                let cut = plane.origin.y as f32;
                assert!(low.y < cut - 0.02, "{id}: lowest {} vs cut {cut}", low.y);
                assert!(high.y > cut, "{id}: highest {} vs cut {cut}", high.y);
                assert!(
                    f64::from(low.z) > plane.origin.z,
                    "{id}: lowest is at the front"
                );
                assert!(low.z > high.z, "{id}: lowest is in front of the highest");
                assert_cap_encloses_the_cut(&id, &s);
            }
        }
    }

    /// Every torso vertex within 2 mm of the cut is inside the cap's outline.
    fn assert_cap_encloses_the_cut(id: &str, s: &Sized) {
        let plane = Plane::new(&s.file, cut_height(&s.file, &s.base, &s.rings));
        let cap = parts(&s.file, &s.base, &s.rings, &s.torso).cap;
        let outline = top_outline(&plane, &cap);
        let near: Vec<_> = s
            .torso
            .positions
            .iter()
            .map(|p| p.as_dvec3())
            .filter(|&p| plane.height(p).abs() <= NEAR_CUT)
            .collect();
        assert!(
            near.len() > 50,
            "{id}: only {} vertices near the cut",
            near.len()
        );
        for p in near {
            assert!(inside(&outline, plane.coords(p)), "{id}: {p} is outside");
        }
    }

    #[test]
    fn the_cap_encloses_the_neck_at_any_size_a_form_takes() {
        let mut seed = 0x9e37_79b9_7f4a_7c15_u64;
        let mut next = move || {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            (seed >> 11) as f64 / (1u64 << 53) as f64
        };
        for json in [WOMEN, MEN] {
            let file = FormFile::from_json(json).unwrap();
            let base = Rings::from_file(&file);
            let mut built = 0;
            for _ in 0..400 {
                let size: Measurements = file
                    .inputs
                    .iter()
                    .map(|m| {
                        let [lo, hi] = file.ranges[m];
                        (m.clone(), lo + (hi - lo) * next())
                    })
                    .collect();
                let Ok(rings) = resize::resize(&file, &base, &size) else {
                    continue;
                };
                built += 1;
                let torso = rings.mesh();
                let all = stand(&file, &base, &rings, &torso);
                assert_eq!(boundary_edge_count(&all), 0, "{}", file.id);
                assert_eq!(lowest(&all), 0.0, "{}", file.id);
                let s = Sized {
                    file: file.clone(),
                    base: base.clone(),
                    rings,
                    torso,
                };
                assert_cap_encloses_the_cut(&format!("{} draw {built}", file.id), &s);
            }
            assert!(built >= 5, "{}: only {built} sizes were taken", file.id);
        }
    }

    #[test]
    fn the_rod_and_knob_stand_on_the_pole_axis_above_the_cap() {
        for (json, extreme) in [(WOMEN, &WOMEN_EXTREME[..]), (MEN, &MEN_EXTREME[..])] {
            for (case, s) in real(json, extreme) {
                let id = format!("{} {case}", s.file.id);
                let [px, pz] = s.file.stand.pole_xz;
                let plane = Plane::new(&s.file, cut_height(&s.file, &s.base, &s.rings));
                let p = parts(&s.file, &s.base, &s.rings, &s.torso);
                let crossing = plane.origin.y + CAP_ABOVE / 17.0_f64.to_radians().cos();
                let top = |m: &BodyMesh| {
                    f64::from(m.positions.iter().map(|p| p.y).fold(f32::MIN, f32::max))
                };
                assert!((top(&p.rod) - (crossing + ROD_HEIGHT)).abs() < 1e-6, "{id}");
                assert!(
                    (top(&p.knob) - (crossing + ROD_HEIGHT + KNOB_HEIGHT)).abs() < 1e-6,
                    "{id}"
                );
                for (m, radius) in [
                    (&p.rod, ROD_RADIUS),
                    (&p.knob, KNOB_RADIUS),
                    (&p.pole, POLE_RADIUS),
                ] {
                    for v in &m.positions[..2 * SIDES as usize] {
                        let r = (f64::from(v.x) - px).hypot(f64::from(v.z) - pz);
                        assert!((r - radius).abs() < 1e-6, "{id}: radius {r}");
                    }
                }
                // The rod's foot is level with the cap's top face at the front of the rod and under
                // it everywhere else, so no gap shows.
                for v in &p.rod.positions[..SIDES as usize] {
                    let h = plane.height(v.as_dvec3());
                    assert!(h < CAP_ABOVE + 1e-6, "{id}: rod foot shows");
                }
                // The axis passes through the cap's outline.
                let outline = top_outline(&plane, &p.cap);
                assert!(
                    inside(&outline, plane.coords(DVec3::new(px, crossing, pz))),
                    "{id}: the pole axis misses the cap"
                );
            }
        }
    }

    #[test]
    fn the_pole_runs_from_the_base_to_the_bottom_ring_and_the_base_is_round() {
        for (json, extreme) in [(WOMEN, &WOMEN_EXTREME[..]), (MEN, &MEN_EXTREME[..])] {
            for (case, s) in real(json, extreme) {
                let id = format!("{} {case}", s.file.id);
                let p = parts(&s.file, &s.base, &s.rings, &s.torso);
                let ys = |m: &BodyMesh| {
                    let (lo, hi) = m
                        .positions
                        .iter()
                        .fold((f32::MAX, f32::MIN), |(lo, hi), v| {
                            (lo.min(v.y), hi.max(v.y))
                        });
                    (f64::from(lo), f64::from(hi))
                };
                let (lo, hi) = ys(&p.pole);
                assert!((lo - BASE_HEIGHT).abs() < 1e-6, "{id}: pole starts at {lo}");
                assert!((hi - s.rings.y[0]).abs() < 1e-3, "{id}: pole ends at {hi}");
                let (lo, hi) = ys(&p.base);
                assert_eq!((lo, hi as f32), (0.0, BASE_HEIGHT as f32), "{id}");
                for v in &p.base.positions[..2 * SIDES as usize] {
                    let r = f64::from(v.x).hypot(f64::from(v.z));
                    assert!((r - BASE_RADIUS).abs() < 1e-6, "{id}: base radius {r}");
                }
            }
        }
    }

    #[test]
    fn a_pole_cannot_have_negative_length() {
        // A form hung so low that its bottom ring is inside the base gets no pole.
        let mut file = fixture::torso();
        for r in &mut file.rings {
            r.y -= 0.69;
        }
        let base = Rings::from_file(&file);
        let torso = base.mesh();
        let p = parts(&file, &base, &base, &torso);
        assert!(p.pole.triangles.is_empty());
        assert_eq!(boundary_edge_count(&stand(&file, &base, &base, &torso)), 0);
    }
}
