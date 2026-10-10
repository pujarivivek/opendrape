//! The stand a form sits on, drawn but never collided: a neck cap that hugs the slanted cut, a
//! rod and knob above it, a pole, and a round base on the floor (y = 0). Every piece is a
//! closed, outward-wound mesh of its own; `stand` appends them into one.

use super::cut::{Plane, sized_plane};
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
/// The cap stands this far above the cut (the re-cut sees to it that nothing of the torso is
/// higher than the cut); its collar hangs `COLLAR_BELOW` below, hugging the neck.
const CAP_ABOVE: f64 = 0.004;
const COLLAR_BELOW: f64 = 0.022;
/// The cap's outline is where the torso meets the cut, pushed out by `CAP_MARGIN` from its
/// centroid.
const CAP_MARGIN: f64 = 0.0025;
/// Points around the cap's outline.
const CAP_SIDES: usize = 64;
/// Sides of the round pieces.
const SIDES: u32 = 24;
/// A vertex this close to the plane (metres) is on it.
const ON_CUT: f64 = 1e-9;

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

/// Where the torso meets the plane, in (u, w): along each column of the mesh (the vertices over
/// the rings at one sample round the form), the vertices on the plane and the points where an
/// edge crosses it.
fn cut_points(plane: &Plane, rings: &Rings) -> Vec<DVec2> {
    let mut out = vec![];
    for j in 0..rings.around() {
        let mut last: Option<(DVec3, f64)> = None;
        for i in 0..rings.len() {
            let p = rings.vertex(i, j);
            let h = plane.height(p);
            if h.abs() <= ON_CUT {
                out.push(plane.coords(p));
            } else if let Some((q, g)) = last
                && g.abs() > ON_CUT
                && (g < 0.0) != (h < 0.0)
            {
                out.push(plane.coords(q + (p - q) * (g / (g - h))));
            }
            last = Some((p, h));
        }
    }
    out
}

/// The cap's outline in (u, w): the hull of where the torso meets the cut and of every vertex
/// from there down to the collar's depth, all seen along the plane's normal, pushed
/// `CAP_MARGIN` out from its centroid and resampled to `CAP_SIDES` even points. The vertices
/// matter because the rings are coarse where the neck meets the plane (a facet runs up from the
/// last wall vertex to the cut face), and because the collar must go round the neck all the
/// way down. Fewer than 3 corners (a torso that never reaches the plane) fall back to the hull
/// of the top ring.
fn cap_outline(plane: &Plane, rings: &Rings) -> Vec<DVec2> {
    let mut points = cut_points(plane, rings);
    for i in 0..rings.len() {
        for j in 0..rings.around() {
            let p = rings.vertex(i, j);
            if plane.height(p) >= -COLLAR_BELOW {
                points.push(plane.coords(p));
            }
        }
    }
    let mut hull = convex_hull(points);
    if hull.len() < 3 {
        let top = rings.len() - 1;
        hull = convex_hull(
            (0..rings.around())
                .map(|j| plane.coords(rings.vertex(top, j)))
                .collect(),
        );
    }
    if hull.len() < 3 {
        return vec![];
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

fn parts(file: &FormFile, base: &Rings, rings: &Rings) -> Parts {
    let [px, pz] = file.stand.pole_xz;
    let plane = sized_plane(file, base, rings);
    let outline = cap_outline(&plane, rings);
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

/// The stand for `rings`, a resized and re-cut copy of `base`: the pieces appended into one
/// mesh, each of them closed.
pub(super) fn stand(file: &FormFile, base: &Rings, rings: &Rings) -> BodyMesh {
    let p = parts(file, base, rings);
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
    use crate::form::cut::base_plane;
    use crate::form::testing::*;
    use crate::form::{FormFile, fixture};

    fn assert_closed_and_outward(name: &str, m: &BodyMesh) {
        assert!(!m.triangles.is_empty(), "{name} is empty");
        assert_eq!(boundary_edge_count(m), 0, "{name} is open");
        assert!(signed_volume(m) > 0.0, "{name} faces inward");
    }

    /// The outline of a cap built by `prism`, in (u, w): its first `CAP_SIDES` vertices.
    fn top_outline(plane: &Plane, cap: &BodyMesh) -> Vec<DVec2> {
        cap.positions[..CAP_SIDES]
            .iter()
            .map(|p| plane.coords(p.as_dvec3()))
            .collect()
    }

    /// The checks of the neck and its cap that do not go through the cap's own choices: they
    /// look at the torso mesh and the finished cap mesh only.
    fn assert_neck_and_cap(id: &str, s: &Sized) {
        let plane = sized_plane(&s.file, &s.base, &s.rings);
        let torso = s.torso();
        let p = parts(&s.file, &s.base, &s.rings);
        // The torso is closed and faces outward, and nothing of it is on the cut-away side.
        assert_closed_and_outward(&format!("{id} torso"), &torso);
        let worst = torso
            .positions
            .iter()
            .map(|v| plane.height(v.as_dvec3()))
            .fold(f64::MIN, f64::max);
        assert!(
            worst <= 1e-4,
            "{id}: a vertex is {:.3} mm above the plane",
            worst * 1000.0
        );
        // Every vertex down to the collar's depth is inside the cap's outline, or within 1 mm.
        let outline = top_outline(&plane, &p.cap);
        for v in torso.positions.iter().map(|v| v.as_dvec3()) {
            if plane.height(v) >= -COLLAR_BELOW {
                let c = plane.coords(v);
                assert!(
                    inside(&outline, c) || distance_to_edges(&outline, c) <= 1e-3,
                    "{id}: {v} is {:.1} mm outside the cap",
                    distance_to_edges(&outline, c) * 1000.0
                );
            }
        }
        // The pole axis passes through the cap, a knob's radius clear of its edge.
        let [px, pz] = s.file.stand.pole_xz;
        let axis = plane.coords(DVec3::new(px, plane.origin.y, pz));
        assert!(inside(&outline, axis), "{id}: the pole axis misses the cap");
        let clear = distance_to_edges(&outline, axis);
        assert!(
            clear >= KNOB_RADIUS,
            "{id}: the axis is {:.1} mm from the edge",
            clear * 1000.0
        );
        // Each piece of the stand is closed and faces outward; together they stand on the floor.
        for (name, m) in [
            ("cap", &p.cap),
            ("rod", &p.rod),
            ("knob", &p.knob),
            ("pole", &p.pole),
            ("base", &p.base),
        ] {
            assert_closed_and_outward(&format!("{id} {name}"), m);
        }
        let all = stand(&s.file, &s.base, &s.rings);
        assert_eq!(boundary_edge_count(&all), 0, "{id}");
        assert_eq!(lowest(&all), 0.0, "{id}");
    }

    #[test]
    fn the_cap_covers_the_neck_of_the_real_forms_at_every_neck_size() {
        for (json, extreme) in both_real_forms() {
            for (case, s) in real_cases(json, extreme) {
                assert_neck_and_cap(&format!("{} {case}", s.file.id), &s);
            }
        }
    }

    #[test]
    fn the_cap_covers_the_neck_at_any_size_a_form_takes() {
        let mut seed = 0x9e37_79b9_7f4a_7c15_u64;
        let mut next = move || {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            (seed >> 11) as f64 / (1u64 << 53) as f64
        };
        for json in [WOMEN, MEN] {
            let file = FormFile::from_json(json).unwrap();
            let mut built = 0;
            for _ in 0..400 {
                let draw: Vec<(&str, f64)> = file
                    .inputs
                    .iter()
                    .map(|m| {
                        let [lo, hi] = file.ranges[m];
                        (m.as_str(), lo + (hi - lo) * next())
                    })
                    .collect();
                let Some(s) = try_sized(&file, &draw) else {
                    continue;
                };
                built += 1;
                assert_neck_and_cap(&format!("{} draw {built}", file.id), &s);
            }
            assert!(built >= 5, "{}: only {built} sizes were taken", file.id);
        }
    }

    #[test]
    fn the_cap_hugs_the_cut_and_lies_in_front_at_its_lowest() {
        for (json, extreme) in both_real_forms() {
            for (case, s) in real_cases(json, extreme) {
                let id = format!("{} {case}", s.file.id);
                let plane = sized_plane(&s.file, &s.base, &s.rings);
                let cap = parts(&s.file, &s.base, &s.rings).cap;
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
            }
        }
    }

    #[test]
    fn the_rod_and_knob_stand_on_the_pole_axis_above_the_cap() {
        for (json, extreme) in both_real_forms() {
            for (case, s) in real_cases(json, extreme) {
                let id = format!("{} {case}", s.file.id);
                let [px, pz] = s.file.stand.pole_xz;
                let tilt = s.file.stand.neck_cut.tilt_deg.to_radians();
                let plane = sized_plane(&s.file, &s.base, &s.rings);
                let p = parts(&s.file, &s.base, &s.rings);
                // Where the axis crosses the cap's top face.
                let crossing = plane.origin.y + CAP_ABOVE / tilt.cos();
                let on_top_face = plane.height(DVec3::new(px, crossing, pz));
                assert!((on_top_face - CAP_ABOVE).abs() < 1e-9, "{id}");
                assert!(
                    (highest(&p.rod) as f64 - (crossing + ROD_HEIGHT)).abs() < 1e-6,
                    "{id}"
                );
                assert!(
                    (highest(&p.knob) as f64 - (crossing + ROD_HEIGHT + KNOB_HEIGHT)).abs() < 1e-6,
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
                // The rod's foot is level with the cap's top face at the front of the rod and
                // under it everywhere else, so no gap shows.
                for v in &p.rod.positions[..SIDES as usize] {
                    let h = plane.height(v.as_dvec3());
                    assert!(h < CAP_ABOVE + 1e-6, "{id}: rod foot shows");
                }
            }
        }
    }

    #[test]
    fn the_pole_runs_from_the_base_to_the_bottom_ring_and_the_base_is_round() {
        for (json, extreme) in both_real_forms() {
            for (case, s) in real_cases(json, extreme) {
                let id = format!("{} {case}", s.file.id);
                let p = parts(&s.file, &s.base, &s.rings);
                let (lo, hi) = (f64::from(lowest(&p.pole)), f64::from(highest(&p.pole)));
                assert!((lo - BASE_HEIGHT).abs() < 1e-6, "{id}: pole starts at {lo}");
                assert!((hi - s.rings.y[0]).abs() < 1e-3, "{id}: pole ends at {hi}");
                assert_eq!(lowest(&p.base), 0.0, "{id}");
                assert_eq!(highest(&p.base), BASE_HEIGHT as f32, "{id}");
                for v in &p.base.positions[..2 * SIDES as usize] {
                    let r = f64::from(v.x).hypot(f64::from(v.z));
                    assert!((r - BASE_RADIUS).abs() < 1e-6, "{id}: base radius {r}");
                }
            }
        }
    }

    #[test]
    fn cut_points_lie_on_the_plane_and_on_the_surface() {
        // Lower the fixture's cut so the plane slices through its neck, rings left as they are.
        let mut file = fixture::torso();
        file.stand.neck_cut.y = 1.46;
        let rings = Rings::from_file(&file);
        let plane = base_plane(&file);
        let torso = rings.mesh();
        let points = cut_points(&plane, &rings);
        assert!(points.len() >= rings.around(), "{} points", points.len());
        for c in points {
            let p = plane.point(c, 0.0);
            assert!(distance_to_mesh(p, &torso) < 1e-6, "{p} is off the surface");
        }
        // The hull of the section is the neck as the plane slices it: wider than deep.
        let hull = convex_hull(cut_points(&plane, &rings));
        let (u, w): (Vec<f64>, Vec<f64>) = hull.iter().map(|p| (p.x.abs(), p.y)).unzip();
        let span = |v: &[f64]| v.iter().copied().fold(f64::MIN, f64::max);
        assert!(
            span(&u) > 0.05 && span(&u) < 0.065,
            "half width {}",
            span(&u)
        );
        assert!(span(&w) > 0.04, "{}", span(&w));
    }

    #[test]
    fn the_fixtures_stand_is_closed_and_on_the_floor() {
        // The fixture's rings stop short of its cut: the cap is built on the vertices within
        // the collar's depth, which is the neck's top.
        let file = fixture::torso();
        let base = Rings::from_file(&file);
        let plane = base_plane(&file);
        assert!(cut_points(&plane, &base).is_empty());
        let p = parts(&file, &base, &base);
        for (name, m) in [
            ("cap", &p.cap),
            ("rod", &p.rod),
            ("knob", &p.knob),
            ("pole", &p.pole),
            ("base", &p.base),
        ] {
            assert_closed_and_outward(name, m);
        }
        let all = stand(&file, &base, &base);
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
    fn a_torso_that_never_reaches_the_cut_gets_a_cap_on_its_top_ring() {
        // Hang the cut a metre above the fixture: nothing is within the collar's depth.
        let mut file = fixture::torso();
        file.stand.neck_cut.y = 2.5;
        let base = Rings::from_file(&file);
        let plane = base_plane(&file);
        let outline = cap_outline(&plane, &base);
        assert_eq!(outline.len(), CAP_SIDES);
        let top = base.len() - 1;
        for j in 0..base.around() {
            assert!(
                inside(&outline, plane.coords(base.vertex(top, j))),
                "sample {j}"
            );
        }
        assert_closed_and_outward("cap", &parts(&file, &base, &base).cap);
    }

    #[test]
    fn the_prism_is_closed_outward_and_exactly_the_height_asked() {
        let plane = base_plane(&fixture::torso());
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
        assert!((highest(&m) - 0.8).abs() < 1e-6);
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
    fn a_pole_cannot_have_negative_length() {
        // A form hung so low that its bottom ring is inside the base gets no pole.
        let mut file = fixture::torso();
        for r in &mut file.rings {
            r.y -= 0.69;
        }
        let base = Rings::from_file(&file);
        let p = parts(&file, &base, &base);
        assert!(p.pole.triangles.is_empty());
        assert_eq!(boundary_edge_count(&stand(&file, &base, &base)), 0);
    }
}
