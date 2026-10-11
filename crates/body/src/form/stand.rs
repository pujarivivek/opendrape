//! The stand a form sits on, drawn but never collided: a neck cap that hugs the slanted cut, a
//! rod and knob above it, a pole, and a round base on the floor (y = 0). Every piece is a
//! closed, outward-wound mesh of its own; `stand` appends them into one.
//!
//! The cap follows the one in the approved Blender pictures (`neck_cap` in
//! `scripts/forms/render_views.py`): an outline of the cut seen from above, a little larger than
//! the cut and the neck's wall; at each point of it a top, parallel to the cut and `CAP_ABOVE` higher, and a collar
//! foot `COLLAR_BELOW` lower, both measured straight up, the sides between them vertical.

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
/// The cap's top stands this far above the cut (the re-cut sees to it that nothing of the
/// torso is higher than the cut); its collar foot hangs `COLLAR_BELOW` below it, both straight
/// up and down.
const CAP_ABOVE: f64 = 0.004;
const COLLAR_BELOW: f64 = 0.022;
/// The cap's top edge is rounded this much (m)...
const CAP_ROUND: f64 = 0.004;
/// ...in this many steps.
const CAP_ROUND_STEPS: usize = 6;
/// The cap's outline is the cut seen from above, pushed out by this much all round.
const CAP_MARGIN: f64 = 0.0025;
/// Directions the margin is rounded with at each corner of the cut's outline (its clearance is
/// at least `CAP_MARGIN`·cos(π/`MARGIN_STEPS`)).
const MARGIN_STEPS: usize = 32;
/// The cap's outline is a smooth curve with a point this often (m)...
const CAP_OUTLINE_STEP: f64 = 0.0015;
/// ...smoothed at least this many times (more for a coarser hull, up to four times as many).
const CAP_SMOOTHING: usize = 12;
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

/// The cap: a closed solid on `outline` (counter-clockwise seen along x then z, as `convex_hull`
/// makes it), its top `above` the plane and its foot `below` it, the sides vertical. Its top
/// edge is rounded, a quarter circle `round` in radius in [`CAP_ROUND_STEPS`] steps (none when
/// `round` is 0). Vertices: the outline's rings from the top down (the flat top's edge first,
/// then round the curve, then the foot), then the top and foot centres.
fn cap_solid(plane: &Plane, outline: &[DVec2], above: f64, below: f64, round: f64) -> BodyMesh {
    let n = outline.len() as u32;
    let at = |c: DVec2, height: f64| DVec3::new(c.x, plane.y_at(c.x, c.y) + height, c.y).as_vec3();
    let centre = outline.iter().sum::<DVec2>() / f64::from(n);
    // Each corner's way out: the mean of its two sides' outward normals.
    let out: Vec<DVec2> = (0..outline.len())
        .map(|j| {
            let len = outline.len();
            let (prev, here, next) = (
                outline[(j + len - 1) % len],
                outline[j],
                outline[(j + 1) % len],
            );
            let side = |a: DVec2, b: DVec2| {
                let perp = (b - a).perp().normalize_or_zero();
                if perp.dot(here - centre) < 0.0 {
                    -perp
                } else {
                    perp
                }
            };
            (side(prev, here) + side(here, next)).normalize_or_zero()
        })
        .collect();
    // The rings from the top down: (how far in from the outline, height).
    let mut rings: Vec<(f64, f64)> = if round > 0.0 {
        (0..=CAP_ROUND_STEPS)
            .rev()
            .map(|k| {
                let phi = std::f64::consts::FRAC_PI_2 * k as f64 / CAP_ROUND_STEPS as f64;
                (round * (1.0 - phi.cos()), above - round + round * phi.sin())
            })
            .collect()
    } else {
        vec![(0.0, above)]
    };
    rings.push((0.0, -below));
    let mut m = BodyMesh {
        positions: vec![],
        triangles: vec![],
    };
    for &(inset, height) in &rings {
        m.positions.extend(
            outline
                .iter()
                .zip(&out)
                .map(|(&c, &o)| at(c - o * inset, height)),
        );
    }
    let count = rings.len() as u32;
    m.positions.push(at(centre, above));
    m.positions.push(at(centre, -below));
    let (top, foot) = (count * n, count * n + 1);
    let ring = |r: u32, j: u32| r * n + j;
    for j in 0..n {
        let k = (j + 1) % n;
        let last = count - 1;
        m.triangles.extend([
            [top, ring(0, k), ring(0, j)],
            [foot, ring(last, j), ring(last, k)],
        ]);
        for r in 0..last {
            let (u, l) = (r, r + 1);
            m.triangles.extend([
                [ring(u, j), ring(l, k), ring(l, j)],
                [ring(u, j), ring(u, k), ring(l, k)],
            ]);
        }
    }
    m
}

/// Where the torso meets the plane: the vertices on it, and the point where each edge of the
/// mesh crosses it (the plane's height changes sign along the edge). The mesh is `rings.mesh()`,
/// but its corners are taken from the rings in full precision.
fn section(plane: &Plane, rings: &Rings) -> Vec<DVec3> {
    let (n, m) = (rings.len(), rings.around());
    let position = |v: u32| match v as usize {
        v if v < n * m => rings.vertex(v / m, v % m),
        v if v == n * m => DVec3::new(0.0, rings.y[0], rings.zc[0]),
        _ => DVec3::new(0.0, rings.y[n - 1], rings.zc[n - 1]),
    };
    let mesh = rings.mesh();
    let mut out: Vec<DVec3> = (0..mesh.positions.len() as u32)
        .map(position)
        .filter(|&p| plane.height(p).abs() <= ON_CUT)
        .collect();
    for t in &mesh.triangles {
        for k in 0..3 {
            let (a, b) = (position(t[k]), position(t[(k + 1) % 3]));
            let (g, h) = (plane.height(a), plane.height(b));
            if g.abs() > ON_CUT && h.abs() > ON_CUT && (g < 0.0) != (h < 0.0) {
                out.push(a + (b - a) * (g / (g - h)));
            }
        }
    }
    out
}

/// The cap's outline seen from above, as (x, z): the hull of the cut and of the neck's `wall`
/// (see `cut::Trim`), pushed `CAP_MARGIN` out all round (corners are rounded, so the margin is
/// the same everywhere). The wall matters because the rings are far apart where the neck meets
/// the plane: the vertices on the cut stop short of the true rim, and the wall, extruded
/// straight up, meets the plane at it. Fewer than 3 corners (a torso that never reaches the
/// plane) fall back to the hull of the top ring.
fn cap_outline(plane: &Plane, rings: &Rings, wall: &[DVec2]) -> Vec<DVec2> {
    let from_above = |p: DVec3| DVec2::new(p.x, p.z);
    let mut cut: Vec<DVec2> = section(plane, rings).into_iter().map(from_above).collect();
    cut.extend_from_slice(wall);
    let mut hull = convex_hull(cut);
    if hull.len() < 3 {
        let top = rings.len() - 1;
        hull = convex_hull(
            (0..rings.around())
                .map(|j| from_above(rings.vertex(top, j)))
                .collect(),
        );
    }
    if hull.len() < 3 {
        return vec![];
    }
    let rounded: Vec<DVec2> = hull
        .iter()
        .flat_map(|&p| {
            (0..MARGIN_STEPS).map(move |k| {
                let a = TAU * k as f64 / MARGIN_STEPS as f64;
                p + DVec2::new(a.cos(), a.sin()) * CAP_MARGIN
            })
        })
        .collect();
    smoothed(&convex_hull(rounded))
}

/// `outline` (convex, counter-clockwise) as a smooth curve: points every [`CAP_OUTLINE_STEP`]
/// round it, each drawn towards its neighbours' middle over and over (over about the length of
/// the hull's longest flat, at least [`CAP_SMOOTHING`] times), then the curve
/// pushed out just as far as it takes to cover `outline` again. Metal shows every flat, and a
/// hull is all flats and corners.
fn smoothed(outline: &[DVec2]) -> Vec<DVec2> {
    let n = outline.len();
    if n < 3 {
        return outline.to_vec();
    }
    let side = |j: usize| outline[j].distance(outline[(j + 1) % n]);
    let perimeter: f64 = (0..n).map(side).sum();
    let count = ((perimeter / CAP_OUTLINE_STEP).round() as usize).max(12);
    let step = perimeter / count as f64;
    let (mut j, mut walked) = (0, 0.0);
    let mut points: Vec<DVec2> = (0..count)
        .map(|k| {
            let at = k as f64 * step;
            while j + 1 < n && walked + side(j) < at {
                walked += side(j);
                j += 1;
            }
            let t = ((at - walked) / side(j).max(1e-12)).clamp(0.0, 1.0);
            outline[j].lerp(outline[(j + 1) % n], t)
        })
        .collect();
    // Smoothed over about as far as the hull's longest flat, so a coarser hull gets more.
    let longest = (0..n).map(side).fold(0.0, f64::max);
    let passes = ((longest / CAP_OUTLINE_STEP).powi(2) / 4.0).round() as usize;
    for _ in 0..passes.clamp(CAP_SMOOTHING, 4 * CAP_SMOOTHING) {
        points = (0..count)
            .map(|k| {
                let middle = (points[(k + count - 1) % count] + points[(k + 1) % count]) / 2.0;
                points[k].lerp(middle, 0.5)
            })
            .collect();
    }
    // How far the farthest corner of `outline` is outside the curve (0 if none is).
    let centre = points.iter().sum::<DVec2>() / count as f64;
    let outward = |a: DVec2, b: DVec2| {
        let perp = (b - a).perp().normalize_or_zero();
        if perp.dot(a - centre) < 0.0 {
            -perp
        } else {
            perp
        }
    };
    let outside = |points: &[DVec2]| {
        outline
            .iter()
            .map(|&q| {
                (0..count)
                    .map(|k| {
                        let (a, b) = (points[k], points[(k + 1) % count]);
                        (q - a).dot(outward(a, b))
                    })
                    .fold(f64::MIN, f64::max)
            })
            .fold(0.0, f64::max)
    };
    // Each point's way out: the mean of its two sides' outward normals.
    let normals: Vec<DVec2> = (0..count)
        .map(|k| {
            let (prev, here, next) = (
                points[(k + count - 1) % count],
                points[k],
                points[(k + 1) % count],
            );
            (outward(prev, here) + outward(here, next)).normalize_or_zero()
        })
        .collect();
    for _ in 0..3 {
        let gap = outside(&points);
        if gap <= 0.0 {
            break;
        }
        for (p, n) in points.iter_mut().zip(&normals) {
            *p += *n * gap;
        }
    }
    points
}

/// The five closed pieces of the stand.
struct Parts {
    cap: BodyMesh,
    rod: BodyMesh,
    knob: BodyMesh,
    pole: BodyMesh,
    base: BodyMesh,
}

fn parts(file: &FormFile, base: &Rings, rings: &Rings, wall: &[DVec2]) -> Parts {
    let [px, pz] = file.stand.pole_xz;
    let plane = sized_plane(file, base, rings);
    let outline = cap_outline(&plane, rings, wall);
    let cap = if outline.len() < 3 {
        BodyMesh {
            positions: vec![],
            triangles: vec![],
        }
    } else {
        cap_solid(&plane, &outline, CAP_ABOVE, COLLAR_BELOW, CAP_ROUND)
    };
    // Where the pole axis crosses the cap's top face. The face is slanted, so the rod's flat
    // end is sunk by the slope across its radius to meet it all round; the rod still stands
    // `ROD_HEIGHT` above the crossing.
    let tilt = file.stand.neck_cut.tilt_deg.to_radians();
    let crossing = plane.y_at(px, pz) + CAP_ABOVE;
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

/// The stand for `rings`, a resized and re-cut copy of `base` whose neck wall is `wall`: the
/// pieces appended into one mesh, each of them closed.
pub(super) fn stand(file: &FormFile, base: &Rings, rings: &Rings, wall: &[DVec2]) -> BodyMesh {
    let p = parts(file, base, rings, wall);
    joined([p.cap, p.rod, p.knob, p.pole, p.base])
}

/// The same stand as [`stand`] in its two finishes: the metal neck cap and the rod on it, then
/// the dark knob, pole and base.
pub(super) fn stand_finishes(
    file: &FormFile,
    base: &Rings,
    rings: &Rings,
    wall: &[DVec2],
) -> (BodyMesh, BodyMesh) {
    let p = parts(file, base, rings, wall);
    (joined([p.cap, p.rod]), joined([p.knob, p.pole, p.base]))
}

/// Closed pieces appended into one mesh.
fn joined<const N: usize>(pieces: [BodyMesh; N]) -> BodyMesh {
    let mut out = BodyMesh {
        positions: vec![],
        triangles: vec![],
    };
    for part in pieces {
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

    /// How far the torso may poke out through the cap's vertical collar, mm, by form and case:
    /// what was measured, rounded up to the next millimetre, plus 1 mm. Measured (women, men):
    /// own size 2.90, 3.65; extreme target 5.82, 5.37; smallest neck 1.09, 1.07; largest neck
    /// 12.27, 11.53; the same at low quality for own size and the extreme target.
    ///
    /// The neck is padded: below the cut it flares into a front fillet and a back slope, and the
    /// collar's vertical sides sink into them by design, as in the approved Blender cap. The
    /// worst vertices are at the back, near the collar's foot, and the lean there grows with the
    /// neck's girth (the rings scale about their own centres, the cut's back edge does not).
    fn poke_bound(id: &str, case: &str) -> f64 {
        let women = id.starts_with("women");
        match (case, women) {
            ("own size" | "low quality", true) => 0.004,
            ("extreme" | "low extreme", true) => 0.007,
            (c, true) if c.starts_with("neck 280") => 0.003,
            (c, true) if c.starts_with("neck 474") => 0.014,
            ("own size" | "low quality", false) => 0.005,
            ("extreme" | "low extreme", false) => 0.007,
            (c, false) if c.starts_with("neck 301") => 0.003,
            (c, false) if c.starts_with("neck 520") => 0.013,
            _ => panic!("no bound for {id} {case}"),
        }
    }
    /// The bound for sizes in general: sweep maxima 12.37 (women) and 11.22 (men), rounded up,
    /// plus 1 mm.
    const ANY_POKE: f64 = 0.014;

    fn assert_closed_and_outward(name: &str, m: &BodyMesh) {
        assert!(!m.triangles.is_empty(), "{name} is empty");
        assert_eq!(boundary_edge_count(m), 0, "{name} is open");
        assert!(
            consistently_oriented(m),
            "{name} has triangles facing both ways"
        );
        assert!(signed_volume(m) > 0.0, "{name} faces inward");
    }

    /// Sides of a cap: its vertices are the top outline, the foot outline and two centres.
    /// Rings of a built (rounded) cap: the flat top's edge, the rest of the rounded edge down to
    /// the top of the side, and the foot.
    const CAP_RINGS: usize = CAP_ROUND_STEPS + 2;

    fn sides(cap: &BodyMesh) -> usize {
        (cap.positions.len() - 2) / CAP_RINGS
    }

    /// The cap's outline seen from above, as (x, z): its foot's vertices (its sides are
    /// vertical, and its top is rounded inside the outline).
    fn top_outline(cap: &BodyMesh) -> Vec<DVec2> {
        let n = sides(cap);
        cap.positions[(CAP_RINGS - 1) * n..CAP_RINGS * n]
            .iter()
            .map(|p| DVec2::new(f64::from(p.x), f64::from(p.z)))
            .collect()
    }

    /// Where the plane cuts the torso mesh, worked out from its triangles' edges: vertices on
    /// the plane and the crossing of every edge whose ends lie on either side of it. The mesh is
    /// in single precision, so "on the plane" is a micrometre.
    fn mesh_section(plane: &Plane, torso: &BodyMesh) -> Vec<DVec3> {
        let near = 1e-6;
        let mut out = vec![];
        for v in &torso.positions {
            if plane.height(v.as_dvec3()).abs() <= near {
                out.push(v.as_dvec3());
            }
        }
        for t in &torso.triangles {
            for k in 0..3 {
                let a = torso.positions[t[k] as usize].as_dvec3();
                let b = torso.positions[t[(k + 1) % 3] as usize].as_dvec3();
                let (g, h) = (plane.height(a), plane.height(b));
                if g.abs() > near && h.abs() > near && (g < 0.0) != (h < 0.0) {
                    out.push(a + (b - a) * (g / (g - h)));
                }
            }
        }
        out
    }

    /// The torso vertices from the plane down to the collar's depth, measured straight up and
    /// down: how far the farthest lies outside the outline seen from above, metres.
    fn poke(plane: &Plane, torso: &BodyMesh, outline: &[DVec2]) -> f64 {
        torso
            .positions
            .iter()
            .map(|v| v.as_dvec3())
            .filter(|v| (-COLLAR_BELOW..=1e-4).contains(&(v.y - plane.y_at(v.x, v.z))))
            .map(|v| DVec2::new(v.x, v.z))
            .filter(|&c| !inside(outline, c))
            .map(|c| distance_to_edges(outline, c))
            .fold(0.0, f64::max)
    }

    /// The checks of the neck and its cap that do not go through the cap's own choices: they
    /// look at the torso mesh and the finished cap mesh only.
    fn assert_neck_and_cap(id: &str, s: &Sized, poke_bound: f64) {
        let plane = sized_plane(&s.file, &s.base, &s.rings);
        let torso = s.torso();
        let p = parts(&s.file, &s.base, &s.rings, &s.wall);
        let outline = top_outline(&p.cap);
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
        // The cut, as the plane slices the mesh's edges, lies inside the outline by the margin.
        let section = mesh_section(&plane, &torso);
        assert!(
            section.len() > 100,
            "{id}: {} section points",
            section.len()
        );
        for q in section {
            let c = DVec2::new(q.x, q.z);
            let clear = distance_to_edges(&outline, c);
            assert!(
                inside(&outline, c) && clear >= 0.0024,
                "{id}: the cut at {q} is {:.2} mm from the outline's edge",
                clear * 1000.0
            );
        }
        // The neck wall's last true samples, which the trim extruded up to the plane, are inside
        // the outline by the margin too.
        assert!(!s.wall.is_empty(), "{id}: no wall points");
        for &w in &s.wall {
            let clear = distance_to_edges(&outline, w);
            assert!(
                inside(&outline, w) && clear >= 0.0024,
                "{id}: the wall at {w} is {:.2} mm from the outline's edge",
                clear * 1000.0
            );
        }
        // The neck below the cut pokes out through the collar only so far.
        let out = poke(&plane, &torso, &outline);
        assert!(
            out <= poke_bound,
            "{id}: the neck pokes {:.1} mm through the collar, bound {:.0}",
            out * 1000.0,
            poke_bound * 1000.0
        );
        // The pole axis passes through the cap, a knob's radius clear of its edge.
        let [px, pz] = s.file.stand.pole_xz;
        let axis = DVec2::new(px, pz);
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
        let all = stand(&s.file, &s.base, &s.rings, &s.wall);
        assert_eq!(boundary_edge_count(&all), 0, "{id}");
        assert_eq!(lowest(&all), 0.0, "{id}");
    }

    #[test]
    fn the_cap_covers_the_cut_of_the_real_forms_at_every_neck_size() {
        for (json, extreme) in both_real_forms() {
            for (case, s) in real_cases(json, extreme) {
                let id = format!("{} {case}", s.file.id);
                assert_neck_and_cap(&id, &s, poke_bound(&s.file.id, &case));
            }
        }
    }

    #[test]
    fn the_cap_covers_the_cut_at_any_size_a_form_takes() {
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
                assert_neck_and_cap(&format!("{} draw {built}", file.id), &s, ANY_POKE);
            }
            assert!(built >= 5, "{}: only {built} sizes were taken", file.id);
        }
    }

    #[test]
    fn the_caps_outline_is_a_smooth_evenly_spaced_curve() {
        // Metal shows every flat: the outline turns a little at every point, never a lot at one.
        for (json, extreme) in both_real_forms() {
            for (case, s) in real_cases(json, extreme) {
                let id = format!("{} {case}", s.file.id);
                let outline = top_outline(&parts(&s.file, &s.base, &s.rings, &s.wall).cap);
                let n = outline.len();
                let lengths: Vec<f64> = (0..n)
                    .map(|j| outline[j].distance(outline[(j + 1) % n]))
                    .collect();
                let mean = lengths.iter().sum::<f64>() / n as f64;
                for (j, len) in lengths.iter().enumerate() {
                    assert!(
                        (0.5 * mean..2.0 * mean).contains(len),
                        "{id}: side {j} is {len}"
                    );
                }
                // No corners: a gentle turn at every point, each much like the next.
                let turns: Vec<f64> = (0..n)
                    .map(|j| {
                        let (a, b, c) =
                            (outline[(j + n - 1) % n], outline[j], outline[(j + 1) % n]);
                        (b - a).angle_to(c - b).to_degrees().abs()
                    })
                    .collect();
                for j in 0..n {
                    let (turn, next) = (turns[j], turns[(j + 1) % n]);
                    assert!(turn < 4.0, "{id}: turns {turn:.1}° at {j}");
                    assert!(
                        (turn - next).abs() < 1.0,
                        "{id}: {turn:.1}° then {next:.1}° at {j}"
                    );
                }
            }
        }
    }

    #[test]
    fn the_cap_reaches_only_its_margin_beyond_the_cut_and_the_wall() {
        for (json, extreme) in both_real_forms() {
            for (case, s) in real_cases(json, extreme) {
                let id = format!("{} {case}", s.file.id);
                let plane = sized_plane(&s.file, &s.base, &s.rings);
                let outline = top_outline(&parts(&s.file, &s.base, &s.rings, &s.wall).cap);
                let section: Vec<f64> = mesh_section(&plane, &s.torso())
                    .iter()
                    .map(|p| p.z)
                    .collect();
                let wall: Vec<f64> = s.wall.iter().map(|p| p.y).collect();
                let outline_z: Vec<f64> = outline.iter().map(|p| p.y).collect();
                let front = |v: &[f64]| v.iter().copied().fold(f64::MIN, f64::max);
                let back = |v: &[f64]| v.iter().copied().fold(f64::MAX, f64::min);
                // Front is +z, back is −z. Past the wall and the cut together, the cap goes only
                // its margin, front and back, and at most a millimetre and a half more where its
                // outline is smoothed into a curve (the hull's flats would show on the metal;
                // measured 2.9 to 4.0 mm over every form and size here).
                let both: Vec<f64> = section.iter().chain(&wall).copied().collect();
                let past_front = front(&outline_z) - front(&both);
                let past_back = back(&both) - back(&outline_z);
                for (side, r) in [("front", past_front), ("back", past_back)] {
                    assert!(
                        (0.0023..=0.0045).contains(&r),
                        "{id}: the cap reaches {:.2} mm past the cut and the wall at the {side}",
                        r * 1000.0
                    );
                }
                // The wall carries the cap's front on beyond the cut's.
                assert!(front(&wall) >= front(&section) - 1e-6, "{id}");
                assert!(front(&outline_z) - front(&section) >= 0.0023, "{id}");
            }
        }
    }

    #[test]
    fn the_cap_hugs_the_cut_with_a_vertical_collar() {
        for (json, extreme) in both_real_forms() {
            for (case, s) in real_cases(json, extreme) {
                let id = format!("{} {case}", s.file.id);
                let plane = sized_plane(&s.file, &s.base, &s.rings);
                let cap = parts(&s.file, &s.base, &s.rings, &s.wall).cap;
                let n = sides(&cap);
                // A smooth curve with a point every 1.5 mm round a neck.
                assert!((150..=400).contains(&n), "{id}: {n} sides");
                // 4 mm above the cut at the top and a 22 mm collar below it, measured straight
                // up and down; the top edge rounds over; the sides are vertical.
                let up =
                    |p: &glam::Vec3| f64::from(p.y) - plane.y_at(f64::from(p.x), f64::from(p.z));
                let (top_ring, side_top, foot) = (0, CAP_ROUND_STEPS, CAP_RINGS - 1);
                for j in 0..n {
                    let at = |r: usize| cap.positions[r * n + j];
                    assert!(
                        (up(&at(top_ring)) - CAP_ABOVE).abs() < 1e-6,
                        "{id}: top {j}"
                    );
                    assert!(
                        (up(&at(foot)) + COLLAR_BELOW).abs() < 1e-6,
                        "{id}: foot {j}"
                    );
                    let (side, foot) = (at(side_top), at(foot));
                    assert!(
                        (side.x - foot.x).abs() < 1e-6 && (side.z - foot.z).abs() < 1e-6,
                        "{id}: side {j} leans"
                    );
                }
                for p in &cap.positions {
                    assert!((-COLLAR_BELOW - 1e-6..=CAP_ABOVE + 1e-6).contains(&up(p)));
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

    /// The approved cap is 4 mm above the cut and its collar foot 22 mm below it. Measured on a
    /// built stand, in numbers written out here and not the constants' names, so that changing a
    /// constant fails a test.
    #[test]
    fn the_built_cap_stands_4_mm_above_the_cut_and_22_mm_below_it() {
        assert_eq!((CAP_ABOVE, COLLAR_BELOW), (0.004, 0.022));
        for (json, extreme) in both_real_forms() {
            for (case, s) in real_cases(json, extreme) {
                let id = format!("{} {case}", s.file.id);
                let plane = sized_plane(&s.file, &s.base, &s.rings);
                let all = stand(&s.file, &s.base, &s.rings, &s.wall);
                // The cap comes first; four round pieces (rod, knob, pole, base) follow it.
                let round = 2 * SIDES as usize + 2;
                let cap = all.positions.len() - 4 * round;
                let (top, foot) = (all.positions[cap - 2], all.positions[cap - 1]);
                // The centre of the cap: the cut seen from above, wherever the centre is.
                assert_eq!((top.x, top.z), (foot.x, foot.z), "{id}");
                let cut = plane.y_at(f64::from(top.x), f64::from(top.z));
                assert!((f64::from(top.y) - cut - 0.004).abs() < 1e-6, "{id}: top");
                assert!((cut - f64::from(foot.y) - 0.022).abs() < 1e-6, "{id}: foot");
            }
        }
    }

    #[test]
    fn the_rod_and_knob_stand_on_the_pole_axis_above_the_cap() {
        for (json, extreme) in both_real_forms() {
            for (case, s) in real_cases(json, extreme) {
                let id = format!("{} {case}", s.file.id);
                let [px, pz] = s.file.stand.pole_xz;
                let plane = sized_plane(&s.file, &s.base, &s.rings);
                let p = parts(&s.file, &s.base, &s.rings, &s.wall);
                // Where the axis crosses the cap's top face, 4 mm above the cut.
                let crossing = plane.y_at(px, pz) + CAP_ABOVE;
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
                // under it everywhere else, so no gap shows; the face is the cut raised 4 mm.
                for v in &p.rod.positions[..SIDES as usize] {
                    let face = plane.y_at(f64::from(v.x), f64::from(v.z)) + CAP_ABOVE;
                    assert!(f64::from(v.y) <= face + 1e-6, "{id}: rod foot shows");
                }
                let front = p.rod.positions[..SIDES as usize]
                    .iter()
                    .max_by(|a, b| a.z.total_cmp(&b.z))
                    .unwrap();
                let face = plane.y_at(f64::from(front.x), f64::from(front.z)) + CAP_ABOVE;
                assert!(
                    (f64::from(front.y) - face).abs() < 1e-5,
                    "{id}: foot is sunk"
                );
            }
        }
    }

    #[test]
    fn the_pole_runs_from_the_base_to_the_bottom_ring_and_the_base_is_round() {
        for (json, extreme) in both_real_forms() {
            for (case, s) in real_cases(json, extreme) {
                let id = format!("{} {case}", s.file.id);
                let p = parts(&s.file, &s.base, &s.rings, &s.wall);
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
    fn the_section_lies_on_the_plane_and_on_the_surface() {
        // Lower the fixture's cut so the plane slices through its neck, rings left as they are.
        let mut file = fixture::torso();
        file.stand.neck_cut.y = 1.46;
        let rings = Rings::from_file(&file);
        let plane = base_plane(&file);
        let torso = rings.mesh();
        let points = section(&plane, &rings);
        assert!(points.len() >= rings.around(), "{} points", points.len());
        for p in &points {
            assert!(plane.height(*p).abs() < 1e-9, "{p} is off the plane");
            assert!(
                distance_to_mesh(*p, &torso) < 1e-6,
                "{p} is off the surface"
            );
        }
        // Seen from above, the cut is the neck's ellipse, wider than deep.
        let (x, z): (Vec<f64>, Vec<f64>) = points.iter().map(|p| (p.x.abs(), p.z)).unzip();
        let span = |v: &[f64]| v.iter().copied().fold(f64::MIN, f64::max);
        assert!(
            span(&x) > 0.05 && span(&x) < 0.065,
            "half width {}",
            span(&x)
        );
        assert!(span(&z) > 0.04, "{}", span(&z));
        // The cap's outline is that ellipse, a margin larger.
        let outline = cap_outline(&plane, &rings, &[]);
        assert!(outline.len() >= 3);
        for p in &points {
            let c = DVec2::new(p.x, p.z);
            assert!(inside(&outline, c) && distance_to_edges(&outline, c) >= 0.0024);
        }
        assert_closed_and_outward("cap", &parts(&file, &rings, &rings, &[]).cap);
    }

    #[test]
    fn a_torso_that_never_reaches_the_cut_gets_a_cap_on_its_top_ring() {
        // The fixture's rings stop short of its cut.
        let file = fixture::torso();
        let base = Rings::from_file(&file);
        let plane = base_plane(&file);
        assert!(section(&plane, &base).is_empty());
        let p = parts(&file, &base, &base, &[]);
        for (name, m) in [
            ("cap", &p.cap),
            ("rod", &p.rod),
            ("knob", &p.knob),
            ("pole", &p.pole),
            ("base", &p.base),
        ] {
            assert_closed_and_outward(name, m);
        }
        let outline = top_outline(&p.cap);
        let top = base.len() - 1;
        for j in 0..base.around() {
            let v = base.vertex(top, j);
            assert!(inside(&outline, DVec2::new(v.x, v.z)), "sample {j}");
        }
        let all = stand(&file, &base, &base, &[]);
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
    fn the_caps_top_edge_is_rounded_smoothly() {
        let plane = base_plane(&fixture::torso());
        const N: usize = 40;
        let outline: Vec<DVec2> = (0..N)
            .map(|k| {
                let a = TAU * k as f64 / N as f64;
                DVec2::new(0.05 * a.cos(), 0.03 * a.sin())
            })
            .collect();
        let round = 0.004;
        let m = cap_solid(&plane, &outline, 0.004, 0.022, round);
        assert_closed_and_outward("rounded cap", &m);
        let up = |p: &glam::Vec3| f64::from(p.y) - plane.y_at(f64::from(p.x), f64::from(p.z));
        // The flat top stops `round` short of the side all round...
        let inside = |p: &glam::Vec3| {
            let q = DVec2::new(f64::from(p.x), f64::from(p.z));
            outline
                .iter()
                .map(|o| o.distance(q))
                .fold(f64::MAX, f64::min)
        };
        let top: Vec<_> = m
            .positions
            .iter()
            .filter(|p| (up(p) - 0.004).abs() < 1e-6)
            .collect();
        assert!(top.len() > N);
        assert!(
            top.iter()
                .all(|p| inside(p) > 0.9 * round || inside(p) > 0.02)
        );
        // ...and the edge curves down to the side through heights in between.
        let between = m
            .positions
            .iter()
            .filter(|p| up(p) > 0.004 - round + 1e-6 && up(p) < 0.004 - 1e-6)
            .count();
        assert!(between >= (CAP_ROUND_STEPS - 1) * N, "{between}");
        // No two neighbouring faces round the edge turn more than a step of it.
        let step = 90.0 / CAP_ROUND_STEPS as f64 + 1.0;
        let normal = |t: &[u32; 3]| {
            let [a, b, c] = t.map(|i| m.positions[i as usize].as_dvec3());
            (b - a).cross(c - a).normalize()
        };
        for pair in m.triangles.windows(2) {
            let (a, b) = (normal(&pair[0]), normal(&pair[1]));
            let shares = pair[0].iter().filter(|v| pair[1].contains(v)).count() >= 2;
            let on_the_rim = pair
                .iter()
                .flatten()
                .all(|&v| up(&m.positions[v as usize]) > 0.004 - round - 1e-6);
            if shares && on_the_rim {
                let turn = a.dot(b).clamp(-1.0, 1.0).acos().to_degrees();
                assert!(turn <= step, "{turn}°");
            }
        }
    }

    #[test]
    fn the_cap_solid_is_closed_outward_and_exactly_the_height_asked() {
        let plane = base_plane(&fixture::torso());
        // Counter-clockwise as `convex_hull` makes it: x then z.
        const N: usize = 40;
        let outline: Vec<DVec2> = (0..N)
            .map(|k| {
                let a = TAU * k as f64 / N as f64;
                DVec2::new(0.05 * a.cos(), 0.03 * a.sin())
            })
            .collect();
        let m = cap_solid(&plane, &outline, 0.004, 0.022, 0.0);
        assert_closed_and_outward("cap", &m);
        assert_eq!(m.triangles.len(), 4 * N);
        for (k, p) in m.positions.iter().enumerate() {
            let up = f64::from(p.y) - plane.y_at(f64::from(p.x), f64::from(p.z));
            let want = if k < N || k == 2 * N { 0.004 } else { -0.022 };
            assert!((up - want).abs() < 1e-6, "vertex {k}: {up}");
        }
        // Outward means upwards on top, downwards underneath, away from the middle at the sides.
        let normal = |t: [u32; 3]| {
            let [a, b, c] = t.map(|i| m.positions[i as usize].as_dvec3());
            (b - a).cross(c - a)
        };
        let n = N as u32;
        assert!(normal([2 * n, 1, 0]).y > 0.0 && normal([2 * n + 1, n, n + 1]).y < 0.0);
        assert!(
            normal(m.triangles[3])
                .dot(m.positions[0].as_dvec3() - m.positions[2 * n as usize].as_dvec3())
                > 0.0
        );
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
    fn a_pole_cannot_have_negative_length() {
        // A form hung so low that its bottom ring is inside the base gets no pole.
        let mut file = fixture::torso();
        for r in &mut file.rings {
            r.y -= 0.69;
        }
        let base = Rings::from_file(&file);
        let p = parts(&file, &base, &base, &[]);
        assert!(p.pole.triangles.is_empty());
        assert_eq!(boundary_edge_count(&stand(&file, &base, &base, &[])), 0);
    }
}
