//! Tape lines on a form: polylines sampled on the surface in (angle, v), measured along their
//! length, and turned into thin ribbons for drawing.

use super::file::{FormFile, TapeDef};
use super::rings::Rings;
use crate::BodyMesh;
use glam::{DVec2, DVec3};
use std::f64::consts::TAU;

/// Ribbons are 6 mm wide, lifted 0.5 mm off the surface so they never flicker into it.
pub const RIBBON_HALF_WIDTH: f64 = 0.003;
pub const RIBBON_LIFT: f64 = 0.0005;

/// A tape line sampled on a sized form.
#[derive(Clone, Debug, PartialEq)]
pub struct Tape {
    pub name: String,
    pub closed: bool,
    /// (angle, v) of each sample. The angle phi stays within [0, 2π): it never runs past 2π
    /// (a file may say exactly 2π, which is centre front too), it wraps back to 0 instead. Where
    /// the tape crosses centre front, phi therefore jumps by nearly 2π between neighbouring
    /// samples. Anyone interpolating along the tape must unwrap it first.
    pub uv: Vec<DVec2>,
    /// Each sample on the form's surface.
    pub points: Vec<DVec3>,
}

/// A place on an open tape: its first or last sample, or where it first reaches a height.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Span {
    Start,
    End,
    /// The first place, counted from the tape's start, where the tape crosses this height (metres).
    Height(f64),
}

impl Tape {
    /// Distance along the tape to each sample, starting at 0.
    fn distances(&self) -> Vec<f64> {
        let mut at = 0.0;
        let mut out = Vec::with_capacity(self.points.len());
        for (k, p) in self.points.iter().enumerate() {
            if k > 0 {
                at += self.points[k - 1].distance(*p);
            }
            out.push(at);
        }
        out
    }

    /// Total length of the polyline, including the closing segment of a loop.
    pub fn length(&self) -> f64 {
        let (Some(&open), Some(first), Some(last)) = (
            self.distances().last(),
            self.points.first(),
            self.points.last(),
        ) else {
            return 0.0;
        };
        if self.closed {
            open + last.distance(*first)
        } else {
            open
        }
    }

    /// Length along the tape between two places, in either order. `None` on a loop, or if the
    /// tape never reaches a height asked for.
    pub fn length_between(&self, from: Span, to: Span) -> Option<f64> {
        if self.closed || self.points.is_empty() {
            return None;
        }
        let along = self.distances();
        let at = |span: Span| match span {
            Span::Start => Some(0.0),
            Span::End => along.last().copied(),
            Span::Height(y) => self.first_crossing(&along, y),
        };
        Some((at(to)? - at(from)?).abs())
    }

    /// Distance from the start to the first place the polyline is at height `y`, interpolated
    /// linearly within its segment.
    fn first_crossing(&self, along: &[f64], y: f64) -> Option<f64> {
        if self.points[0].y == y {
            return Some(0.0);
        }
        self.points.windows(2).enumerate().find_map(|(k, w)| {
            let (a, b) = (w[0].y, w[1].y);
            let reached = (a < y && y <= b) || (a > y && y >= b);
            reached.then(|| along[k] + (along[k + 1] - along[k]) * ((y - a) / (b - a)))
        })
    }
}

/// (angle, v) of a landmark; `<name>_R` is its mirror image on the right side.
pub fn landmark_uv(file: &FormFile, name: &str) -> Option<DVec2> {
    let (base, right) = match name.strip_suffix("_R") {
        Some(b) => (b, true),
        None => (name, false),
    };
    let [phi, v] = *file.landmarks.get(base)?;
    Some(DVec2::new(if right { TAU - phi } else { phi }, v))
}

/// The place a length is measured to or from: `"start"`, `"end"`, or a station name (the height
/// of that station's ring).
pub fn place(file: &FormFile, rings: &Rings, place: &str) -> Option<Span> {
    match place {
        "start" => Some(Span::Start),
        "end" => Some(Span::End),
        station => {
            let &i = file.stations.get(station)?;
            rings.y.get(i).map(|&y| Span::Height(y))
        }
    }
}

/// One tape by name; `<tape>_R` is the mirror copy of a sampled tape marked `mirror`.
pub fn tape(file: &FormFile, rings: &Rings, name: &str) -> Option<Tape> {
    if let Some(def) = file.tapes.get(name) {
        return match def {
            TapeDef::Ring { ring } => Some(ring_tape(name, *file.stations.get(ring)?, rings)),
            TapeDef::Samples { uv, closed, .. } => {
                Some(sampled_tape(name, uv, false, *closed, rings))
            }
        };
    }
    match file.tapes.get(name.strip_suffix("_R")?)? {
        TapeDef::Samples {
            uv,
            closed,
            mirror: true,
        } => Some(sampled_tape(name, uv, true, *closed, rings)),
        _ => None,
    }
}

/// Every tape of the form, mirror copies included.
pub fn tapes(file: &FormFile, rings: &Rings) -> Vec<Tape> {
    let mut out = vec![];
    for (name, def) in &file.tapes {
        out.extend(tape(file, rings, name));
        if matches!(def, TapeDef::Samples { mirror: true, .. }) {
            out.extend(tape(file, rings, &format!("{name}_R")));
        }
    }
    out
}

/// A tape through the file's samples, or through their mirror images (2π − angle).
fn sampled_tape(
    name: &str,
    samples: &[[f64; 2]],
    mirror: bool,
    closed: bool,
    rings: &Rings,
) -> Tape {
    let uv: Vec<DVec2> = samples
        .iter()
        .map(|&[phi, v]| DVec2::new(if mirror { TAU - phi } else { phi }, v))
        .collect();
    Tape {
        name: name.to_string(),
        closed,
        points: uv.iter().map(|p| rings.point(p.x, p.y)).collect(),
        uv,
    }
}

/// The full ring at station ring `i`, one sample per ring vertex.
fn ring_tape(name: &str, i: usize, rings: &Rings) -> Tape {
    let m = rings.around();
    let v = i as f64 / (rings.len() - 1) as f64;
    Tape {
        name: name.to_string(),
        closed: true,
        uv: (0..m)
            .map(|j| DVec2::new(TAU * j as f64 / m as f64, v))
            .collect(),
        points: (0..m).map(|j| rings.vertex(i, j)).collect(),
    }
}

/// Thin ribbons along every tape, lifted off the surface, for drawing only (open, not collided).
pub fn ribbons(tapes: &[Tape], rings: &Rings) -> BodyMesh {
    let mut mesh = BodyMesh {
        positions: vec![],
        triangles: vec![],
    };
    for t in tapes {
        let n = t.points.len();
        let base = mesh.positions.len() as u32;
        for k in 0..n {
            let prev = if k > 0 {
                k - 1
            } else if t.closed {
                n - 1
            } else {
                0
            };
            let next = if k + 1 < n {
                k + 1
            } else if t.closed {
                0
            } else {
                n - 1
            };
            let tangent = (t.points[next] - t.points[prev]).normalize_or_zero();
            let normal = rings.normal(t.uv[k].x, t.uv[k].y);
            let side = normal.cross(tangent).normalize_or_zero() * RIBBON_HALF_WIDTH;
            let p = t.points[k] + normal * RIBBON_LIFT;
            mesh.positions.push((p - side).as_vec3());
            mesh.positions.push((p + side).as_vec3());
        }
        let segments = if t.closed { n } else { n.saturating_sub(1) };
        for k in 0..segments {
            let (a, b) = (base + 2 * k as u32, base + 2 * ((k + 1) % n) as u32);
            mesh.triangles.push([a, b, a + 1]);
            mesh.triangles.push([a + 1, b, b + 1]);
        }
    }
    mesh
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::form::{TORSO_LENGTHS, fixture};
    use std::f64::consts::PI;

    const WOMEN: &str = include_str!("../../../../assets/forms/women-torso.form.json");
    const MEN: &str = include_str!("../../../../assets/forms/men-torso.form.json");

    fn setup() -> (FormFile, Rings) {
        let f = fixture::torso();
        let r = Rings::from_file(&f);
        (f, r)
    }

    fn waist_y(f: &FormFile, r: &Rings) -> f64 {
        r.y[f.stations["waist"]]
    }

    fn assert_mirror(a: DVec3, b: DVec3) {
        assert!(
            (a.x + b.x).abs() < 1e-9 && (a.y - b.y).abs() < 1e-9 && (a.z - b.z).abs() < 1e-9,
            "{a} vs {b}"
        );
    }

    /// The median and the largest distance between consecutive samples.
    fn step_sizes(t: &Tape) -> (f64, f64) {
        let mut gaps: Vec<f64> = t.points.windows(2).map(|w| w[0].distance(w[1])).collect();
        gaps.sort_by(f64::total_cmp);
        (gaps[gaps.len() / 2], gaps[gaps.len() - 1])
    }

    /// A tape with only its points set: enough for measuring.
    fn polyline(points: &[[f64; 3]]) -> Tape {
        Tape {
            name: "hand".into(),
            closed: false,
            uv: vec![],
            points: points.iter().map(|&p| DVec3::from(p)).collect(),
        }
    }

    #[test]
    fn every_tape_and_its_mirror_is_built() {
        let (f, r) = setup();
        let all = tapes(&f, &r);
        let mut names: Vec<&str> = all.iter().map(|t| t.name.as_str()).collect();
        names.sort_unstable();
        assert_eq!(
            names,
            [
                "armhole",
                "armhole_R",
                "bust",
                "cb",
                "cf",
                "high_hip",
                "hip",
                "neckline_back",
                "neckline_front",
                "princess_back",
                "princess_back_R",
                "princess_front",
                "princess_front_R",
                "shoulder_seam",
                "shoulder_seam_R",
                "side_seam",
                "side_seam_R",
                "under_bust",
                "waist",
            ]
        );
        for t in &all {
            assert_eq!(tape(&f, &r, &t.name).as_ref(), Some(t), "{}", t.name);
            assert_eq!(t.points.len(), t.uv.len(), "{}", t.name);
            assert!(t.points.len() >= 2, "{}", t.name);
            assert!(t.points.iter().all(|p| p.is_finite()), "{}", t.name);
            for (p, uv) in t.points.iter().zip(&t.uv) {
                assert!(
                    p.distance(r.point(uv.x, uv.y)) < 1e-9,
                    "{} sits on the surface",
                    t.name
                );
            }
        }
        // Sampled tapes keep the file's samples; ring tapes take a sample per ring vertex.
        assert_eq!(tape(&f, &r, "cf").unwrap().points.len(), 40);
        assert_eq!(tape(&f, &r, "waist").unwrap().points.len(), r.around());
        assert!(tape(&f, &r, "cf_R").is_none(), "cf is not mirrored");
        assert!(tape(&f, &r, "waist_R").is_none(), "a ring has no mirror");
        assert!(tape(&f, &r, "nowhere").is_none());
        assert!(tape(&f, &r, "nowhere_R").is_none());
    }

    #[test]
    fn landmarks_resolve_and_the_right_side_mirrors() {
        let (f, _) = setup();
        let [phi, v] = f.landmarks["shoulder_point"];
        assert_eq!(landmark_uv(&f, "shoulder_point"), Some(DVec2::new(phi, v)));
        assert_eq!(
            landmark_uv(&f, "shoulder_point_R"),
            Some(DVec2::new(TAU - phi, v))
        );
        assert_eq!(landmark_uv(&f, "front_neck").unwrap().x, 0.0);
        assert_eq!(landmark_uv(&f, "nowhere"), None);
        assert_eq!(landmark_uv(&f, "nowhere_R"), None);
        assert_eq!(landmark_uv(&f, "_R"), None);
    }

    #[test]
    fn centre_front_and_back_run_down_the_middle() {
        let (f, r) = setup();
        for (name, side) in [("cf", 1.0), ("cb", -1.0)] {
            let t = tape(&f, &r, name).unwrap();
            assert!(!t.closed);
            assert!(t.points.iter().all(|p| p.x.abs() < 1e-9), "{name} on x = 0");
            assert!(
                t.points.windows(2).all(|w| w[1].y < w[0].y),
                "{name} runs down"
            );
            assert!(
                t.points.iter().all(|p| p.z * side > 0.0),
                "{name} is on its side of the form"
            );
        }
        let cf = tape(&f, &r, "cf").unwrap();
        let [phi, v] = f.landmarks["front_neck"];
        assert!(cf.points[0].distance(r.point(phi, v)) < 1e-9);
    }

    #[test]
    fn ring_tape_is_the_station_ring() {
        let (f, r) = setup();
        let waist = tape(&f, &r, "waist").unwrap();
        assert!(waist.closed && waist.points.len() == r.around());
        for (j, p) in waist.points.iter().enumerate() {
            assert_eq!(*p, r.vertex(33, j), "sample {j}");
        }
        for (p, uv) in waist.points.iter().zip(&waist.uv) {
            assert!(
                p.distance(r.point(uv.x, uv.y)) < 1e-9,
                "uv names the sample"
            );
        }
        assert!(
            (waist.length() - r.girth(33)).abs() < 1e-9,
            "a convex ring's perimeter is its hull's"
        );
        let bust = tape(&f, &r, "bust").unwrap();
        assert!(bust.points.iter().all(|p| p.y == r.y[51]));
    }

    #[test]
    fn mirrored_tapes_are_mirror_images() {
        let (f, r) = setup();
        for name in [
            "shoulder_seam",
            "side_seam",
            "armhole",
            "princess_front",
            "princess_back",
        ] {
            let p = tape(&f, &r, name).unwrap();
            let q = tape(&f, &r, &format!("{name}_R")).unwrap();
            assert_eq!(p.closed, q.closed, "{name}");
            assert_eq!(p.points.len(), q.points.len(), "{name}");
            for (a, b) in p.points.iter().zip(&q.points) {
                assert_mirror(*a, *b);
            }
            for (a, b) in p.uv.iter().zip(&q.uv) {
                assert!((a.x + b.x - TAU).abs() < 1e-12 && a.y == b.y, "{name} uv");
            }
            let reach = |t: &Tape| {
                t.points
                    .iter()
                    .map(|p| p.x)
                    .fold(0.0, |m: f64, x| m.max(x.abs()))
            };
            assert!(reach(&p) > 0.04, "{name} is off the middle");
            assert!(p.points.iter().all(|p| p.x >= 0.0), "{name} on the left");
            assert!(q.points.iter().all(|p| p.x <= 0.0), "{name}_R on the right");
        }
    }

    #[test]
    fn neckline_crosses_centre_front_without_a_jump() {
        let (f, r) = setup();
        let neck = tape(&f, &r, "neckline_front").unwrap();
        assert!(!neck.closed);
        assert!(
            neck.uv.windows(2).any(|w| (w[0].x - w[1].x).abs() > PI),
            "its samples wrap past 2π, so the test is not trivial"
        );
        let (median, longest) = step_sizes(&neck);
        assert!(longest < 3.0 * median, "{longest} m step, median {median}");
        // It passes through the middle of the front.
        assert!(neck.points.iter().any(|p| p.x.abs() < 0.005 && p.z > 0.0));
        assert!((0.15..0.45).contains(&neck.length()), "{}", neck.length());
    }

    #[test]
    fn length_between_measures_along_the_tape() {
        let (f, r) = setup();
        let cf = tape(&f, &r, "cf").unwrap();
        let waist = Span::Height(waist_y(&f, &r));
        let l = cf.length_between(Span::Start, waist).unwrap();
        assert_eq!(
            cf.length_between(waist, Span::Start),
            Some(l),
            "either order"
        );
        assert!(l > 0.0 && l < cf.length(), "{l} of {}", cf.length());

        // The fixture's cf lies on the ellipses' front tips, so its true length is the path over
        // the ring-to-ring segments from front_neck (ring 71) down to the waist (ring 33).
        let tip = |i: usize| DVec2::new(r.y[i], r.zc[i] + r.r[i][0]);
        let exact: f64 = (34..=71).map(|i| tip(i).distance(tip(i - 1))).sum();
        assert!((l - exact).abs() < 5e-4, "{l} vs {exact}");
        assert!(l > r.y[71] - r.y[33], "longer than the straight drop");

        // Two heights on the tape, either order, and lengths add up.
        let (bust, hip) = (
            Span::Height(r.y[f.stations["bust"]]),
            Span::Height(r.y[f.stations["hip"]]),
        );
        let (a, b) = (
            cf.length_between(bust, waist).unwrap(),
            cf.length_between(waist, hip).unwrap(),
        );
        assert_eq!(cf.length_between(waist, bust), Some(a));
        assert!((a + b - cf.length_between(bust, hip).unwrap()).abs() < 1e-12);
        assert!(
            (cf.length_between(Span::Start, bust).unwrap() + a - l).abs() < 1e-12,
            "start to bust to waist is start to waist"
        );

        // The ends, and heights the tape never reaches.
        assert_eq!(cf.length_between(Span::Start, Span::End), Some(cf.length()));
        assert_eq!(cf.length_between(Span::End, Span::Start), Some(cf.length()));
        assert_eq!(cf.length_between(Span::Start, Span::Start), Some(0.0));
        assert_eq!(cf.length_between(Span::Start, Span::Height(1.5)), None);
        assert_eq!(cf.length_between(Span::Height(1.5), Span::Start), None);
        assert_eq!(cf.length_between(Span::Height(0.5), Span::End), None);
        assert_eq!(cf.length_between(waist, Span::Height(f64::NAN)), None);
    }

    #[test]
    fn height_means_the_first_crossing_and_interpolates_within_the_segment() {
        // Up the 3-4-5 diagonal, down 4, then back across to the top: height 2 is crossed three
        // times; the first one is halfway up the first segment.
        let t = polyline(&[
            [0.0, 0.0, 0.0],
            [3.0, 4.0, 0.0],
            [3.0, 0.0, 0.0],
            [0.0, 4.0, 0.0],
        ]);
        let total = 5.0 + 4.0 + 5.0;
        let close = |a: Option<f64>, b: f64| a.is_some_and(|a| (a - b).abs() < 1e-12);
        assert!(close(t.length_between(Span::Start, Span::Height(2.0)), 2.5));
        assert!(close(
            t.length_between(Span::Height(2.0), Span::End),
            total - 2.5
        ));
        assert!(close(
            t.length_between(Span::Height(1.0), Span::Height(3.0)),
            2.5
        ));
        // Touching a height exactly counts; so does standing on it at the start.
        assert!(close(t.length_between(Span::Start, Span::Height(4.0)), 5.0));
        assert!(close(t.length_between(Span::Start, Span::Height(0.0)), 0.0));
        assert_eq!(t.length_between(Span::Start, Span::Height(4.5)), None);
        assert_eq!(t.length_between(Span::Start, Span::Height(-0.5)), None);
        // A tape that comes down finds the height on the way down.
        let down = polyline(&[[0.0, 10.0, 0.0], [0.0, 6.0, 0.0], [0.0, 0.0, 0.0]]);
        assert!(close(
            down.length_between(Span::Start, Span::Height(8.0)),
            2.0
        ));
        assert!(close(
            down.length_between(Span::Height(3.0), Span::Height(8.0)),
            5.0
        ));
    }

    #[test]
    fn closed_tapes_have_no_between() {
        let (f, r) = setup();
        for name in ["waist", "armhole", "armhole_R"] {
            let t = tape(&f, &r, name).unwrap();
            assert!(t.closed, "{name}");
            assert_eq!(t.length_between(Span::Start, Span::End), None, "{name}");
            let y = t.points[0].y;
            assert_eq!(
                t.length_between(Span::Start, Span::Height(y)),
                None,
                "{name}"
            );
        }
    }

    #[test]
    fn length_counts_the_closing_segment_of_a_loop_only() {
        let mut square = polyline(&[
            [0.0, 0.0, 0.0],
            [1.0, 0.0, 0.0],
            [1.0, 1.0, 0.0],
            [0.0, 1.0, 0.0],
        ]);
        assert_eq!(square.length(), 3.0);
        square.closed = true;
        assert_eq!(square.length(), 4.0);
    }

    #[test]
    fn places_resolve_to_spans() {
        let (f, r) = setup();
        assert_eq!(place(&f, &r, "start"), Some(Span::Start));
        assert_eq!(place(&f, &r, "end"), Some(Span::End));
        assert_eq!(place(&f, &r, "waist"), Some(Span::Height(1.03)));
        assert_eq!(place(&f, &r, "hip"), Some(Span::Height(r.y[12])));
        assert_eq!(place(&f, &r, "nowhere"), None);
        assert_eq!(place(&f, &r, "cf"), None, "a tape is not a place");
        assert_eq!(place(&f, &r, ""), None);
    }

    #[test]
    fn the_fixtures_lengths_are_all_measurable() {
        let (f, r) = setup();
        for (m, name, from, to) in TORSO_LENGTHS {
            let l = measure(&f, &r, name, from, to);
            assert!(l.is_some_and(|l| (0.05..0.8).contains(&l)), "{m}: {l:?} m");
        }
    }

    #[test]
    fn ribbons_sit_just_outside_the_surface() {
        let (f, r) = setup();
        let t = vec![tape(&f, &r, "side_seam").unwrap()];
        let mesh = ribbons(&t, &r);
        assert_eq!(mesh.positions.len(), 2 * t[0].points.len());
        assert_eq!(mesh.triangles.len(), 2 * (t[0].points.len() - 1));
        for (k, p) in t[0].points.iter().enumerate() {
            let (a, b) = (
                mesh.positions[2 * k].as_dvec3(),
                mesh.positions[2 * k + 1].as_dvec3(),
            );
            assert!((a.distance(b) - 2.0 * RIBBON_HALF_WIDTH).abs() < 1e-5);
            let mid = (a + b) / 2.0;
            assert!((mid.distance(*p) - RIBBON_LIFT).abs() < 1e-5);
            let n = r.normal(t[0].uv[k].x, t[0].uv[k].y);
            assert!((mid - *p).dot(n) > 0.0, "lifted outward");
            assert!(
                (b - a).dot(n).abs() < 1e-5,
                "across the tape, flat on the surface"
            );
        }
        for tri in &mesh.triangles {
            let [a, b, c] = tri.map(|i| mesh.positions[i as usize].as_dvec3());
            let k = tri.iter().map(|&i| i as usize / 2).min().unwrap();
            let n = r.normal(t[0].uv[k].x, t[0].uv[k].y);
            assert!((b - a).cross(c - a).dot(n) > 0.0, "{tri:?} faces outward");
        }
    }

    #[test]
    fn ribbons_cover_loops_and_several_tapes() {
        let (f, r) = setup();
        let ts = vec![tape(&f, &r, "waist").unwrap(), tape(&f, &r, "cf").unwrap()];
        let mesh = ribbons(&ts, &r);
        let (n0, n1) = (ts[0].points.len(), ts[1].points.len());
        assert_eq!(mesh.positions.len(), 2 * (n0 + n1));
        // A loop closes back on its start; an open tape does not.
        assert_eq!(mesh.triangles.len(), 2 * n0 + 2 * (n1 - 1));
        let (waist, cf) = mesh.triangles.split_at(2 * n0);
        assert!(waist.iter().flatten().all(|&i| (i as usize) < 2 * n0));
        assert!(cf.iter().flatten().all(|&i| (i as usize) >= 2 * n0));
        assert!(
            waist.iter().flatten().any(|&i| i == 0 || i == 1)
                && waist.iter().flatten().any(|&i| i as usize == 2 * n0 - 1),
            "the last quad of the loop meets the first"
        );
        assert!(ribbons(&[], &r).triangles.is_empty());
    }

    /// Length of `tape` between two places, as the torso's measurements will take it.
    fn measure(f: &FormFile, r: &Rings, tape_name: &str, from: &str, to: &str) -> Option<f64> {
        tape(f, r, tape_name)?.length_between(place(f, r, from)?, place(f, r, to)?)
    }

    #[test]
    fn the_shipped_forms_tapes_build_and_their_lengths_are_plausible() {
        // (measurement, plausible range in metres) for each shipped form.
        type Ranges<'a> = &'a [(&'a str, f64, f64)];
        let women: Ranges = &[
            ("back_waist_length", 0.38, 0.48),
            ("waist_to_hip", 0.18, 0.24),
            ("shoulder_length", 0.11, 0.16),
            ("front_waist_length", 0.30, 0.42),
        ];
        let men: Ranges = &[
            ("back_waist_length", 0.43, 0.53),
            ("waist_to_hip", 0.18, 0.25),
            ("shoulder_length", 0.13, 0.19),
            ("front_waist_length", 0.36, 0.48),
        ];
        for (who, json, ranges) in [("women", WOMEN, women), ("men", MEN, men)] {
            let f = FormFile::from_json(json).expect(who);
            let r = Rings::from_file(&f);
            let all = tapes(&f, &r);
            let mirrored = f
                .tapes
                .values()
                .filter(|d| matches!(d, TapeDef::Samples { mirror: true, .. }))
                .count();
            assert_eq!(all.len(), f.tapes.len() + mirrored, "{who}");
            for t in &all {
                assert!(t.points.len() >= 2, "{who} {}", t.name);
                assert_eq!(t.points.len(), t.uv.len(), "{who} {}", t.name);
                assert!(t.points.iter().all(|p| p.is_finite()), "{who} {}", t.name);
                assert!(t.length() > 0.0, "{who} {}", t.name);
            }

            // Mirror copies are exact, and the front neckline (which wraps past 2π) has no jump.
            for t in all.iter().filter(|t| t.name.ends_with("_R")) {
                let base = tape(&f, &r, t.name.trim_end_matches("_R")).unwrap();
                assert_eq!(base.points.len(), t.points.len(), "{who} {}", t.name);
                for (a, b) in base.points.iter().zip(&t.points) {
                    assert_mirror(*a, *b);
                }
            }
            let neck = tape(&f, &r, "neckline_front").unwrap();
            assert!(neck.uv.windows(2).any(|w| (w[0].x - w[1].x).abs() > PI));
            let (median, longest) = step_sizes(&neck);
            assert!(longest < 3.0 * median, "{who} neckline jumps {longest} m");

            // cb starts at the back of the neck and cf at the front, on the form's surface.
            for (name, landmark) in [("cb", "back_neck"), ("cf", "front_neck")] {
                let t = tape(&f, &r, name).unwrap();
                let uv = landmark_uv(&f, landmark).unwrap();
                let want = r.point(uv.x, uv.y);
                assert!(
                    t.points[0].distance(want) < 0.01,
                    "{who} {name} starts {} m from {landmark}",
                    t.points[0].distance(want)
                );
            }
            // The shoulder seam runs from the side of the neck to the shoulder tip.
            let seam = tape(&f, &r, "shoulder_seam").unwrap();
            for (p, landmark) in [
                (seam.points[0], "side_neck"),
                (*seam.points.last().unwrap(), "shoulder_point"),
            ] {
                let uv = landmark_uv(&f, landmark).unwrap();
                assert!(
                    p.distance(r.point(uv.x, uv.y)) < 0.01,
                    "{who} shoulder_seam ends away from {landmark}"
                );
            }

            for (m, name, from, to) in TORSO_LENGTHS {
                let l = measure(&f, &r, name, from, to)
                    .unwrap_or_else(|| panic!("{who} {m}: no length on {name} {from}..{to}"));
                let &(_, lo, hi) = ranges
                    .iter()
                    .find(|x| x.0 == m)
                    .unwrap_or_else(|| panic!("{who}: no plausible range for {m}"));
                assert!((lo..hi).contains(&l), "{who} {m} is {l} m, not {lo}..{hi}");
            }
        }
    }
}
