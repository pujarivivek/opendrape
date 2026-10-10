//! The slanted plane a form's neck is cut by, and the re-cut that keeps a resized form's neck
//! top on that plane.
//!
//! The resize scales each ring about its own centre, but the plane stays where it is (it only
//! rises and falls with the top ring). A larger neck would then stand out through the plane, a
//! smaller one fall short of it, so `recut` trims the sized rings back to the plane.

use super::file::FormFile;
use super::rings::Rings;
use glam::{DVec2, DVec3};
use std::f64::consts::PI;

/// A vertex of the base form this far off the cut plane is on the neck's own surface; nearer, it
/// is on the cut face. On the shipped forms, cut-face vertices are within 0.02 mm of the plane
/// and the nearest wall vertices 0.58 mm below it (the top ring's sides): 0.1 mm keeps those
/// apart, where 0.5 mm counts the top ring's sides as cut face and moves them by up to 3 mm at
/// the form's own size.
const ON_PLANE: f64 = 0.0001;
/// No radius shrinks below this when it is trimmed.
const MIN_RADIUS: f64 = 0.0005;

/// The plane the neck is cut by: through the pole at the cut's height, lower at the front.
pub(super) struct Plane {
    pub origin: DVec3,
    /// Unit normal, pointing away from the form and tilted towards the front.
    pub n: DVec3,
}

impl Plane {
    /// The plane through the pole at `cut_y`, lower at the front by the file's tilt: it cuts
    /// away the points with (y − cut_y) + (z − pole_z)·tan(tilt) > 0.
    pub fn new(file: &FormFile, cut_y: f64) -> Self {
        let [x, z] = file.stand.pole_xz;
        let (s, c) = file.stand.neck_cut.tilt_deg.to_radians().sin_cos();
        Self {
            origin: DVec3::new(x, cut_y, z),
            n: DVec3::new(0.0, c, s),
        }
    }

    /// Signed distance from the plane, positive on the cut-away side.
    pub fn height(&self, p: DVec3) -> f64 {
        (p - self.origin).dot(self.n)
    }

    /// The plane's height at (x, z).
    pub fn y_at(&self, x: f64, z: f64) -> f64 {
        self.origin.y - ((x - self.origin.x) * self.n.x + (z - self.origin.z) * self.n.z) / self.n.y
    }
}

/// The plane on the unsized form.
pub(super) fn base_plane(file: &FormFile) -> Plane {
    Plane::new(file, file.stand.neck_cut.y)
}

/// The plane on the sized form. Everything above the back neck moves rigidly when a form is
/// resized, so the cut moves with the top ring.
pub(super) fn sized_plane(file: &FormFile, base: &Rings, rings: &Rings) -> Plane {
    let top = rings.len() - 1;
    Plane::new(file, file.stand.neck_cut.y + (rings.y[top] - base.y[top]))
}

/// A neck trimmed to the cut plane.
pub(super) struct Trim {
    pub rings: Rings,
    /// The neck wall's last true sample, seen from above as (x, z), at every sample round the
    /// form where rings were trimmed, that is where the base form has rings above the neck's own
    /// surface. Extruded straight up, these meet the plane where the true neck does, which is
    /// beyond the vertices on the plane when the rings are far apart there.
    pub wall: Vec<DVec2>,
}

/// `rings` (a resized copy of `base`) with its neck trimmed to the cut plane.
///
/// At each angle the base form's rings leave the neck's own surface at some ring, `neck`: the
/// highest ring whose vertex is off the plane. Above it, the base rings lie on the cut face.
/// On the sized form those rings sit on the neck's wall, extruded straight up from the last
/// true samples (seen from above, the polygon of the vertices at `neck`), clipped where the ray
/// from the ring's centre meets the new plane when the ray heads out through it. A larger neck
/// is thus cut back to the plane, and a smaller one meets it lower down. Any other vertex left
/// on the cut-away side is moved along its ray onto the plane.
///
/// The wall is taken in plan, not as the radius of the last sample, because the rings' centres
/// slide back as they rise through the cut: one radius reused about a moving centre leans the
/// neck back and, at the form's own size, moved the top rings by up to 4 mm (1.5 mm this way).
///
/// The right side mirrors the left, so the result stays exactly symmetric.
pub(super) fn recut(file: &FormFile, base: &Rings, rings: &Rings) -> Trim {
    let (old, new) = (base_plane(file), sized_plane(file, base, rings));
    let (n, half, around) = (rings.len(), rings.half(), rings.around());
    // The last true neck ring at each angle.
    let necks: Vec<Option<usize>> = (0..half)
        .map(|k| {
            (0..n)
                .rev()
                .find(|&i| old.height(base.vertex(i, k)).abs() > ON_PLANE)
        })
        .collect();
    // The neck's outline from above, from those last true samples: extruded upwards it is the
    // wall of the neck that the plane cuts.
    let wall: Vec<DVec2> = (0..around)
        .map(|j| {
            let k = j.min(around - j);
            let p = rings.vertex(necks[k].unwrap_or(n - 1), j);
            DVec2::new(p.x, p.z)
        })
        .collect();
    let trimmed = |j: usize| matches!(necks[j.min(around - j)], Some(i) if i < n - 1);
    let wall_points = (0..around)
        .filter(|&j| trimmed(j))
        .map(|j| wall[j])
        .collect();
    let mut out = rings.clone();
    for (k, &neck) in necks.iter().enumerate() {
        let angle = PI * k as f64 / (half - 1) as f64;
        let x = if k == half - 1 { 0.0 } else { angle.sin() };
        let dir = DVec3::new(x, 0.0, angle.cos());
        // How fast the plane's height rises along the ray.
        let slope = new.n.dot(dir);
        for i in 0..n {
            let centre = DVec3::new(0.0, rings.y[i], rings.zc[i]);
            let h0 = new.height(centre);
            // Where the ray from the centre meets the plane, if it does.
            let meets = (slope.abs() > 1e-9 && -h0 / slope > 0.0).then(|| -h0 / slope);
            let mut r = rings.r[i][k];
            if let Some(neck) = neck
                && i > neck
            {
                r = wall_distance(&wall, DVec2::new(0.0, centre.z), DVec2::new(dir.x, dir.z))
                    .unwrap_or(rings.r[neck][k]);
                if slope > 0.0 {
                    r = r.min(meets.unwrap_or(0.0)).max(MIN_RADIUS);
                }
            }
            if new.height(centre + dir * r) > 0.0 {
                r = meets.unwrap_or(0.0).max(MIN_RADIUS);
            }
            out.r[i][k] = r;
        }
    }
    Trim {
        rings: out,
        wall: wall_points,
    }
}

/// How far from `from`, along `dir`, a ray first meets the closed polygon `outline`.
fn wall_distance(outline: &[DVec2], from: DVec2, dir: DVec2) -> Option<f64> {
    let mut best: Option<f64> = None;
    for (a, b) in outline.iter().zip(outline.iter().cycle().skip(1)) {
        let edge = *b - *a;
        let denom = dir.perp_dot(edge);
        if denom.abs() < 1e-12 {
            continue;
        }
        let t = (*a - from).perp_dot(edge) / denom;
        let s = (*a - from).perp_dot(dir) / denom;
        if t > 0.0 && (0.0..=1.0).contains(&s) && best.is_none_or(|b| t < b) {
            best = Some(t);
        }
    }
    best
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::form::testing::*;
    use crate::form::{fixture, resize};

    /// Every radius of `a` and `b`, as (ring, angle, difference in metres), largest first.
    fn changes(a: &Rings, b: &Rings) -> Vec<(usize, usize, f64)> {
        let mut out: Vec<_> = (0..a.len())
            .flat_map(|i| (0..a.half()).map(move |k| (i, k)))
            .map(|(i, k)| (i, k, (a.r[i][k] - b.r[i][k]).abs()))
            .filter(|c| c.2 > 1e-12)
            .collect();
        out.sort_by(|p, q| q.2.total_cmp(&p.2));
        out
    }

    /// The highest any vertex of `rings` is above the plane, metres.
    fn highest_above(plane: &Plane, rings: &Rings) -> f64 {
        (0..rings.len())
            .flat_map(|i| (0..rings.around()).map(move |j| (i, j)))
            .map(|(i, j)| plane.height(rings.vertex(i, j)))
            .fold(f64::MIN, f64::max)
    }

    #[test]
    fn the_plane_cuts_away_what_the_file_says() {
        let file = fixture::torso();
        let plane = Plane::new(&file, 1.52);
        let tilt = file.stand.neck_cut.tilt_deg.to_radians();
        for (y, z) in [(1.60, 0.02), (1.40, -0.03), (1.52, 0.0), (1.50, 0.07)] {
            let cut_away = (y - 1.52) + z * tilt.tan() > 0.0;
            let h = plane.height(DVec3::new(0.01, y, z));
            assert_eq!(h > 0.0, cut_away, "({y}, {z})");
            assert!((h - ((y - 1.52) * tilt.cos() + z * tilt.sin())).abs() < 1e-12);
        }
        // The normal is a unit vector tilted to the front; the plane is lower there.
        assert!((plane.n.length() - 1.0).abs() < 1e-12 && plane.n.z > 0.0);
        for (x, z) in [(0.0, 0.0), (0.04, 0.05), (-0.03, -0.06)] {
            let y = plane.y_at(x, z);
            assert!(
                plane.height(DVec3::new(x, y, z)).abs() < 1e-12,
                "({x}, {z})"
            );
            assert!((y - (1.52 - z * tilt.tan())).abs() < 1e-12);
        }
    }

    #[test]
    fn the_cut_moves_with_the_top_ring() {
        let file = FormFile::from_json(WOMEN).unwrap();
        let s = sized(&file, &[("back_waist_length", 440.0)]);
        let top = s.rings.len() - 1;
        let moved = s.rings.y[top] - s.base.y[top];
        assert!(moved.abs() > 0.01, "the top ring moved {moved}");
        let plane = sized_plane(&file, &s.base, &s.rings);
        assert!((plane.origin.y - (file.stand.neck_cut.y + moved)).abs() < 1e-12);
        let own = sized(&file, &[]);
        let plane = sized_plane(&file, &own.base, &own.rings);
        assert_eq!(plane.origin.y, file.stand.neck_cut.y);
    }

    #[test]
    fn a_form_that_stops_short_of_its_cut_is_left_alone() {
        let file = fixture::torso();
        let base = Rings::from_file(&file);
        let cut = recut(&file, &base, &base);
        assert_eq!(cut.rings, base);
        assert!(cut.wall.is_empty(), "nothing above the neck's own surface");
        let s = sized(&file, &[("neck", 330.0), ("hip", 1000.0)]);
        let raw = resize::resize(&file, &s.base, &s.size).unwrap();
        assert_ne!(raw, s.base);
        assert_eq!(s.rings, raw);
    }

    #[test]
    fn at_its_own_size_a_real_form_barely_changes() {
        for json in [WOMEN, MEN] {
            let file = FormFile::from_json(json).unwrap();
            for half in [49, 33] {
                let base = Rings::from_file(&file).with_half_angles(half);
                let cut = recut(&file, &base, &base).rings;
                let diffs = changes(&cut, &base);
                // Only the rings above the neck's station, and by about a millimetre at most
                // (the base's top rings are slivers cut by the plane, which the wall through
                // the last true samples only approximates).
                assert!(
                    diffs.iter().all(|c| c.0 > file.stations["neck"]),
                    "{} ({half}): {:?}",
                    file.id,
                    diffs[0]
                );
                assert!(
                    diffs[0].2 < 0.002,
                    "{} ({half}): ring {} angle {} moved {:.2} mm",
                    file.id,
                    diffs[0].0,
                    diffs[0].1,
                    diffs[0].2 * 1000.0
                );
            }
        }
    }

    #[test]
    fn a_trimmed_neck_lies_on_or_below_the_plane_at_any_size() {
        for (json, extreme) in both_real_forms() {
            for (case, s) in real_cases(json, extreme) {
                let id = format!("{} {case}", s.file.id);
                let plane = sized_plane(&s.file, &s.base, &s.rings);
                let raw = resize::resize(&s.file, &s.base, &s.size).unwrap();
                let (before, after) =
                    (highest_above(&plane, &raw), highest_above(&plane, &s.rings));
                assert!(
                    after <= 1e-9,
                    "{id}: {:.3} mm above the plane",
                    after * 1000.0
                );
                // Larger necks stood out through the plane before, up to a few millimetres.
                if case.starts_with("neck") && before > 0.001 {
                    assert!(
                        s.size["neck"] > s.base.girth(s.file.stations["neck"]) * 1000.0,
                        "{id}"
                    );
                }
                // The trim leaves the girths alone: nothing at or below the neck's station
                // moves, so the neck's girth, and every length, is as the resize made it.
                let moved = changes(&s.rings, &raw);
                assert!(
                    moved.iter().all(|c| c.0 > s.file.stations["neck"]),
                    "{id}: ring {} moved",
                    moved[0].0
                );
                assert_eq!(
                    s.rings.girth(s.file.stations["neck"]),
                    raw.girth(s.file.stations["neck"]),
                    "{id}"
                );
            }
        }
    }

    #[test]
    fn the_big_necks_do_stand_out_before_the_trim() {
        for (json, _) in both_real_forms() {
            let file = FormFile::from_json(json).unwrap();
            let (_, large) = neck_extremes(&file);
            let s = sized(&file, &[("neck", large)]);
            let raw = resize::resize(&file, &s.base, &s.size).unwrap();
            let above = highest_above(&sized_plane(&file, &s.base, &s.rings), &raw);
            assert!(
                above > 0.004,
                "{}: only {:.2} mm above",
                file.id,
                above * 1000.0
            );
        }
    }

    #[test]
    fn trimming_twice_changes_nothing_more() {
        for (json, extreme) in both_real_forms() {
            for (case, s) in real_cases(json, extreme) {
                let again = recut(&s.file, &s.base, &s.rings).rings;
                let worst = changes(&again, &s.rings).first().map_or(0.0, |c| c.2);
                assert!(worst < 1e-9, "{} {case}: moved {worst}", s.file.id);
            }
        }
    }

    #[test]
    fn radii_stay_positive_and_rings_stay_in_order() {
        for (json, extreme) in both_real_forms() {
            for (case, s) in real_cases(json, extreme) {
                let id = format!("{} {case}", s.file.id);
                assert!(
                    s.rings
                        .r
                        .iter()
                        .flatten()
                        .all(|r| r.is_finite() && *r >= MIN_RADIUS),
                    "{id}"
                );
                assert!(s.rings.y.windows(2).all(|w| w[1] > w[0]), "{id}");
            }
        }
    }

    #[test]
    fn the_wall_points_are_vertices_of_the_sized_neck_in_mirror_pairs() {
        for (json, extreme) in both_real_forms() {
            for (case, s) in real_cases(json, extreme) {
                let id = format!("{} {case}", s.file.id);
                let raw = resize::resize(&s.file, &s.base, &s.size).unwrap();
                let vertices: Vec<DVec3> = (0..raw.len())
                    .flat_map(|i| (0..raw.around()).map(move |j| (i, j)))
                    .map(|(i, j)| raw.vertex(i, j))
                    .collect();
                let on_a_vertex = |w: &DVec2| {
                    vertices
                        .iter()
                        .any(|v| (v.x - w.x).abs() < 1e-12 && (v.z - w.y).abs() < 1e-12)
                };
                for w in &s.wall {
                    assert!(
                        on_a_vertex(w),
                        "{id}: {w} is not a vertex of the sized neck"
                    );
                    assert!(
                        s.wall
                            .iter()
                            .any(|m| (m.x + w.x).abs() < 1e-12 && (m.y - w.y).abs() < 1e-12),
                        "{id}: {w} has no mirror image"
                    );
                }
                // The front of the neck: the angles where the base form has rings above the
                // neck's own surface, a third to a half of the samples round the form.
                let share = s.wall.len() as f64 / s.rings.around() as f64;
                assert!((0.3..0.5).contains(&share), "{id}: {share}");
            }
        }
    }

    #[test]
    fn a_form_with_nothing_to_trim_has_no_wall_points() {
        let file = fixture::torso();
        let s = sized(&file, &[("neck", 330.0)]);
        assert!(s.wall.is_empty());
    }
}
