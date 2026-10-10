//! The `.form.json` file format (format 1): one part of a dress form (today a torso) as
//! horizontal rings, with its stations, landmarks, sampled tape lines, stand, measurement inputs
//! and ranges.

use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::f64::consts::{PI, TAU};

pub const FORMAT: u32 = 1;
/// Radii per ring, from centre front (0) to centre back (π) on the form's left (+x) side.
pub const ANGLES: usize = 49;

/// What part of a body the form is. Only torsos exist today; the key stays in the file so arms
/// and legs can follow.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    Torso,
}

/// One horizontal ring: height and centre-line z in metres, radii in millimetres.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Ring {
    pub y: f64,
    pub zc: f64,
    pub r: Vec<f64>,
}

/// A tape line: a full ring at a station, or a polyline sampled on the form's surface.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum TapeDef {
    Ring {
        ring: String,
    },
    Samples {
        /// [phi, v] pairs a few millimetres apart: phi in radians, 0..=2π, from centre front
        /// towards +x (it may wrap past 2π back to 0); v from 0 (bottom ring) to 1 (top ring).
        uv: Vec<[f64; 2]>,
        #[serde(default)]
        closed: bool,
        /// Also draw the mirror image (2π − phi) on the right side, named `<tape>_R`.
        #[serde(default)]
        mirror: bool,
    },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Collision {
    /// Gap kept between cloth and form, metres.
    pub thickness: f64,
    pub friction: f64,
}

/// The stand the form is mounted on: a vertical pole, and a slanted cut where the neck ends.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Stand {
    /// The pole is the vertical line through (x, z), metres.
    pub pole_xz: [f64; 2],
    pub neck_cut: NeckCut,
}

/// The neck is cut by a plane through the pole at height `y` (metres), lower at the front by
/// `tilt_deg`: points with `z > (y_cut − y) / tan(tilt)` are cut away.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct NeckCut {
    pub y: f64,
    pub tilt_deg: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FormFile {
    pub format: u32,
    pub id: String,
    pub kind: Kind,
    /// Fluent message id of the display name.
    pub name: String,
    /// Garment types this form is for.
    pub suits: Vec<String>,
    pub licence: String,
    pub base_size: String,
    pub angles: usize,
    /// Bottom to top.
    pub rings: Vec<Ring>,
    /// Station name → ring index.
    pub stations: BTreeMap<String, usize>,
    /// Left-side landmark → [angle from centre front (radians, 0..=π), v (0 bottom ring, 1 top)].
    pub landmarks: BTreeMap<String, [f64; 2]>,
    pub tapes: BTreeMap<String, TapeDef>,
    pub stand: Stand,
    /// Measurements a user can set, in the order the app shows them.
    pub inputs: Vec<String>,
    /// Allowed range of each input, millimetres.
    pub ranges: BTreeMap<String, [f64; 2]>,
    pub collision: Collision,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FormError(pub String);

impl std::fmt::Display for FormError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for FormError {}

/// Lengths measured along a sampled tape: (measurement, tape, from, to). `from` and `to` are
/// `"start"` or `"end"` (the tape's first and last samples) or a station name (where the tape
/// crosses that station's ring height).
pub const TORSO_LENGTHS: [(&str, &str, &str, &str); 4] = [
    ("back_waist_length", "cb", "start", "waist"),
    ("waist_to_hip", "side_seam", "waist", "hip"),
    ("shoulder_length", "shoulder_seam", "start", "end"),
    ("front_waist_length", "cf", "start", "waist"),
];
/// Lengths a user can set; the others are read-only measurements.
pub const ADJUSTABLE_LENGTHS: [&str; 3] = ["back_waist_length", "waist_to_hip", "shoulder_length"];
/// How far (in v, where the whole form is 1) a length's tape must run past a station it is
/// measured from or to.
const REACH_MARGIN: f64 = 1e-9;
/// Stations the resizing needs (plus `bust` or `chest` on a torso).
pub const TORSO_STATIONS: [&str; 4] = ["waist", "hip", "shoulder", "neck"];

impl FormFile {
    pub fn from_json(s: &str) -> Result<Self, FormError> {
        let f: FormFile =
            serde_json::from_str(s).map_err(|e| FormError(format!("not a form file: {e}")))?;
        f.check()?;
        Ok(f)
    }

    pub fn lengths(&self) -> &'static [(&'static str, &'static str, &'static str, &'static str)] {
        match self.kind {
            Kind::Torso => &TORSO_LENGTHS,
        }
    }

    /// Where the shoulder widening starts: `bust` on women's forms, `chest` on men's.
    pub fn chest_station(&self) -> Option<&str> {
        ["bust", "chest"]
            .into_iter()
            .find(|s| self.stations.contains_key(*s))
    }

    pub fn check(&self) -> Result<(), FormError> {
        let bad = |m: String| Err(FormError(format!("form {}: {m}", self.id)));
        if self.format != FORMAT {
            return bad(format!("format {} is not {FORMAT}", self.format));
        }
        if self.angles != ANGLES {
            return bad(format!("{} angles, expected {ANGLES}", self.angles));
        }
        if self.rings.len() < 8 {
            return bad("fewer than 8 rings".into());
        }
        for (i, r) in self.rings.iter().enumerate() {
            let ok = r.r.len() == ANGLES
                && r.r.iter().all(|x| x.is_finite() && *x > 0.0)
                && r.y.is_finite()
                && r.zc.is_finite();
            if !ok {
                return bad(format!("ring {i} is malformed"));
            }
            if i > 0 && r.y <= self.rings[i - 1].y {
                return bad(format!("ring {i} is not above ring {}", i - 1));
            }
        }
        for (name, &i) in &self.stations {
            if i >= self.rings.len() {
                return bad(format!("station {name} has no ring {i}"));
            }
        }
        for (name, &[phi, v]) in &self.landmarks {
            if !(0.0..=PI).contains(&phi) || !(0.0..=1.0).contains(&v) {
                return bad(format!("landmark {name} is off the form"));
            }
        }
        for (name, tape) in &self.tapes {
            match tape {
                TapeDef::Ring { ring } => {
                    if !self.stations.contains_key(ring) {
                        return bad(format!("tape {name} needs station {ring}"));
                    }
                }
                TapeDef::Samples { uv, closed, .. } => {
                    if uv.len() < if *closed { 3 } else { 2 } {
                        return bad(format!("tape {name} is too short"));
                    }
                    if !uv.iter().flatten().all(|x| x.is_finite()) {
                        return bad(format!("tape {name} has a sample that is not finite"));
                    }
                    if let Some(&[phi, _]) = uv.iter().find(|p| !(0.0..=TAU).contains(&p[0])) {
                        return bad(format!("tape {name} has phi {phi} outside 0..=2π"));
                    }
                    if let Some(&[_, v]) = uv.iter().find(|p| !(0.0..=1.0).contains(&p[1])) {
                        return bad(format!("tape {name} has v {v} outside 0..=1"));
                    }
                }
            }
        }
        let Stand {
            pole_xz,
            neck_cut: NeckCut { y, tilt_deg },
        } = &self.stand;
        if !pole_xz.iter().all(|x| x.is_finite()) {
            return bad("stand pole is not finite".into());
        }
        if !y.is_finite() {
            return bad("stand neck cut height is not finite".into());
        }
        if !(*tilt_deg > 0.0 && *tilt_deg < 90.0) {
            return bad(format!(
                "stand neck cut tilts {tilt_deg} degrees, expected between 0 and 90"
            ));
        }
        let Collision {
            thickness,
            friction,
        } = &self.collision;
        if !(thickness.is_finite() && *thickness > 0.0) {
            return bad(format!(
                "collision thickness {thickness} must be finite and above 0"
            ));
        }
        if !(friction.is_finite() && *friction >= 0.0) {
            return bad(format!(
                "collision friction {friction} must be finite and 0 or more"
            ));
        }
        let needed: &[&str] = match self.kind {
            Kind::Torso => &TORSO_STATIONS,
        };
        if let Some(s) = needed.iter().find(|s| !self.stations.contains_key(**s)) {
            return bad(format!("needs station {s}"));
        }
        if self.chest_station().is_none() {
            return bad("needs a bust or chest station".into());
        }
        if !self.landmarks.contains_key("back_neck") {
            return bad(
                "needs landmark back_neck (the resizing finds the neck's height from it)".into(),
            );
        }
        // The resizing stretches the form between the waist and the back neck (that is how it
        // sets the back waist length), which needs the back neck to lie above the waist.
        let [_, back_neck_v] = self.landmarks["back_neck"];
        let waist = self.stations["waist"];
        let waist_v = waist as f64 / (self.rings.len() - 1) as f64;
        if back_neck_v <= waist_v {
            return bad(format!(
                "landmark back_neck (at v {back_neck_v:.4}) must lie above the waist station \
                 (ring {waist}, at v {waist_v:.4})"
            ));
        }
        for (m, tape, from, to) in self.lengths() {
            let Some(TapeDef::Samples { uv, closed, .. }) = self.tapes.get(*tape) else {
                return bad(format!("{m} needs tape {tape} to be a sampled tape"));
            };
            if *closed {
                return bad(format!("{m} needs tape {tape} to be open, not a loop"));
            }
            // Ring heights rise with v, so a tape reaches a station's height exactly when the v
            // of the station lies within the v of its samples. Reaching it by less than
            // `REACH_MARGIN` is refused: rounding could then miss the crossing.
            let (v_lo, v_hi) = uv
                .iter()
                .fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), p| {
                    (lo.min(p[1]), hi.max(p[1]))
                });
            for place in [from, to] {
                if matches!(*place, "start" | "end") {
                    continue;
                }
                let Some(&ring) = self.stations.get(*place) else {
                    return bad(format!("{m} needs station {place}"));
                };
                let v = ring as f64 / (self.rings.len() - 1) as f64;
                if !(v_lo + REACH_MARGIN <= v && v <= v_hi - REACH_MARGIN) {
                    return bad(format!(
                        "{m} needs tape {tape} to cross station {place} (at v {v:.4}), \
                         but the tape only spans v {v_lo:.4} to {v_hi:.4}"
                    ));
                }
            }
        }
        let mut listed = BTreeSet::new();
        let mut ring_of: BTreeMap<usize, &str> = BTreeMap::new();
        for m in &self.inputs {
            if !listed.insert(m) {
                return bad(format!("input {m} is listed twice"));
            }
            // Two girth inputs on one ring could not both be met, and would leave the resizing
            // with two stations at the same place.
            if let Some(&ring) = self.stations.get(m)
                && let Some(other) = ring_of.insert(ring, m)
            {
                return bad(format!(
                    "inputs {other} and {m} are both girths at ring {ring}"
                ));
            }
            let adjustable_length =
                ADJUSTABLE_LENGTHS.contains(&m.as_str()) && self.lengths().iter().any(|l| l.0 == m);
            if !self.stations.contains_key(m) && !adjustable_length {
                return bad(format!(
                    "input {m} is neither a station girth nor an adjustable length"
                ));
            }
            match self.ranges.get(m) {
                Some(&[lo, hi]) if lo > 0.0 && lo < hi => {}
                _ => return bad(format!("input {m} has no valid range")),
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::form::fixture;

    const WOMEN: &str = include_str!("../../../../assets/forms/women-torso.form.json");
    const MEN: &str = include_str!("../../../../assets/forms/men-torso.form.json");

    /// The sample list of a `Samples` tape, for tests that break one.
    fn samples<'a>(f: &'a mut FormFile, tape: &str) -> &'a mut Vec<[f64; 2]> {
        match f.tapes.get_mut(tape) {
            Some(TapeDef::Samples { uv, .. }) => uv,
            _ => panic!("{tape} is not a sampled tape"),
        }
    }

    fn close(a: &[f64], b: &[f64], tol: f64) -> bool {
        a.len() == b.len() && a.iter().zip(b).all(|(x, y)| (x - y).abs() <= tol)
    }

    /// The first thing that differs between two forms, or `None`. Text, counts, flags and names
    /// must be equal; lengths and angles may differ by the last digit JSON parsing loses
    /// (rings 1e-12 m and 1e-9 mm, everything else 1e-12).
    fn first_difference(a: &FormFile, b: &FormFile) -> Option<String> {
        let same = (a.format, &a.id, a.kind, &a.name, &a.suits, &a.licence)
            == (b.format, &b.id, b.kind, &b.name, &b.suits, &b.licence);
        if !same || a.base_size != b.base_size || a.angles != b.angles {
            return Some("header".into());
        }
        if a.rings.len() != b.rings.len() {
            return Some("ring count".into());
        }
        for (i, (p, q)) in a.rings.iter().zip(&b.rings).enumerate() {
            if !close(&[p.y, p.zc], &[q.y, q.zc], 1e-12) || !close(&p.r, &q.r, 1e-9) {
                return Some(format!("ring {i}"));
            }
        }
        if a.stations != b.stations || a.inputs != b.inputs {
            return Some("stations or inputs".into());
        }
        let names = |m: &BTreeMap<String, [f64; 2]>| m.keys().cloned().collect::<Vec<_>>();
        if names(&a.landmarks) != names(&b.landmarks)
            || a.landmarks
                .iter()
                .any(|(n, p)| !close(p, &b.landmarks[n], 1e-12))
        {
            return Some("landmarks".into());
        }
        if a.tapes.len() != b.tapes.len() {
            return Some("tape count".into());
        }
        for (name, t) in &a.tapes {
            let same = match (t, b.tapes.get(name)) {
                (TapeDef::Ring { ring: x }, Some(TapeDef::Ring { ring: y })) => x == y,
                (
                    TapeDef::Samples {
                        uv: p,
                        closed: c,
                        mirror: m,
                    },
                    Some(TapeDef::Samples {
                        uv: q,
                        closed: d,
                        mirror: n,
                    }),
                ) => {
                    c == d
                        && m == n
                        && p.len() == q.len()
                        && p.iter().zip(q).all(|(x, y)| close(x, y, 1e-12))
                }
                _ => false,
            };
            if !same {
                return Some(format!("tape {name}"));
            }
        }
        let (s, t) = (&a.stand, &b.stand);
        let stand = [
            s.pole_xz[0],
            s.pole_xz[1],
            s.neck_cut.y,
            s.neck_cut.tilt_deg,
        ];
        let other = [
            t.pole_xz[0],
            t.pole_xz[1],
            t.neck_cut.y,
            t.neck_cut.tilt_deg,
        ];
        if !close(&stand, &other, 1e-12) {
            return Some("stand".into());
        }
        let ranges = |f: &FormFile| f.ranges.values().flatten().copied().collect::<Vec<_>>();
        if a.ranges.keys().ne(b.ranges.keys()) || !close(&ranges(a), &ranges(b), 1e-9) {
            return Some("ranges".into());
        }
        let c = [a.collision.thickness, a.collision.friction];
        let d = [b.collision.thickness, b.collision.friction];
        (!close(&c, &d, 1e-12)).then(|| "collision".into())
    }

    #[test]
    fn fixture_torso_passes_the_checks() {
        fixture::torso().check().unwrap();
    }

    #[test]
    fn round_trips_through_json() {
        let f = fixture::torso();
        let json = serde_json::to_string(&f).unwrap();
        let back = FormFile::from_json(&json).unwrap();
        assert_eq!(first_difference(&f, &back), None);
    }

    #[test]
    fn the_round_trip_comparison_notices_real_differences() {
        let f = fixture::torso();
        for (break_it, what) in [
            (
                Box::new(|f: &mut FormFile| f.rings[7].r[3] += 1e-6) as Box<dyn Fn(&mut FormFile)>,
                "ring 7",
            ),
            (Box::new(|f| samples(f, "cf")[5][1] += 1e-6), "tape cf"),
            (
                Box::new(|f| f.landmarks.get_mut("bust_apex").unwrap()[0] += 1e-6),
                "landmarks",
            ),
            (Box::new(|f| f.stand.neck_cut.tilt_deg += 1e-6), "stand"),
            (Box::new(|f| f.collision.friction += 1e-6), "collision"),
            (Box::new(|f| f.base_size = "other".into()), "header"),
            (Box::new(|f| f.inputs.reverse()), "stations or inputs"),
        ] {
            let mut g = f.clone();
            break_it(&mut g);
            assert_eq!(first_difference(&f, &g).as_deref(), Some(what));
        }
    }

    #[test]
    fn json_that_is_not_a_form_is_refused() {
        assert!(
            FormFile::from_json("{}")
                .unwrap_err()
                .0
                .starts_with("not a form file")
        );
    }

    #[test]
    fn from_json_runs_the_checks() {
        let mut f = fixture::torso();
        f.format = 2;
        let json = serde_json::to_string(&f).unwrap();
        let err = FormFile::from_json(&json).unwrap_err().0;
        assert!(err.contains("format 2"), "{err:?}");
    }

    #[test]
    fn both_shipped_forms_parse_and_pass_the_checks() {
        for (json, id, chest, base_size) in [
            (WOMEN, "women-torso", "bust", "US 8"),
            (MEN, "men-torso", "chest", "40"),
        ] {
            // `from_json` runs `check()`.
            let f = FormFile::from_json(json).unwrap();
            assert_eq!(f.id, id);
            assert_eq!(f.kind, Kind::Torso);
            assert_eq!(f.base_size, base_size);
            assert_eq!(f.angles, ANGLES);
            assert_eq!(f.chest_station(), Some(chest));
            assert_eq!(f.landmarks.len(), 15);
            // The neck is cut near the top ring.
            assert!(f.stand.neck_cut.y > f.rings.last().unwrap().y - 0.1);
            // The four lengths all measure along a sampled tape.
            for (_, tape, _, _) in f.lengths() {
                assert!(matches!(f.tapes[*tape], TapeDef::Samples { .. }), "{tape}");
            }
        }
    }

    #[test]
    fn the_fixture_has_the_tapes_the_lengths_need() {
        let f = fixture::torso();
        assert_eq!(f.lengths(), &TORSO_LENGTHS);
        for (m, tape, _, _) in f.lengths() {
            let Some(TapeDef::Samples { uv, .. }) = f.tapes.get(*tape) else {
                panic!("{m} needs tape {tape}");
            };
            assert_eq!(uv.len(), 40, "{tape}");
        }
        for m in ADJUSTABLE_LENGTHS {
            assert!(f.lengths().iter().any(|l| l.0 == m), "{m}");
        }
    }

    #[test]
    fn chest_station_is_bust_or_chest() {
        let mut f = fixture::torso();
        assert_eq!(f.chest_station(), Some("bust"));
        let ring = f.stations.remove("bust").unwrap();
        assert_eq!(f.chest_station(), None);
        f.stations.insert("chest".into(), ring);
        assert_eq!(f.chest_station(), Some("chest"));
    }

    /// Each case breaks the fixture one way; the error must carry that case's own message, so
    /// the needles are specific enough that no two causes can pass for each other.
    #[test]
    fn broken_references_are_refused() {
        type Breaker = Box<dyn Fn(&mut FormFile)>;
        let cases: Vec<(Breaker, &str)> = vec![
            // The header and the rings.
            (Box::new(|f| f.format = 2), "format 2 is not 1"),
            (Box::new(|f| f.angles = 48), "48 angles, expected 49"),
            (
                Box::new(|f| {
                    let _ = f.rings[3].r.pop();
                }),
                "ring 3 is malformed",
            ),
            (
                Box::new(|f| f.rings[5].y = f.rings[4].y),
                "ring 5 is not above ring 4",
            ),
            // Stations and landmarks.
            (
                Box::new(|f| {
                    let _ = f.stations.insert("hip".into(), 500);
                }),
                "station hip has no ring 500",
            ),
            (
                Box::new(|f| f.landmarks.get_mut("bust_apex").unwrap()[0] = 4.0),
                "landmark bust_apex is off the form",
            ),
            // A ring tape needs its station.
            (
                Box::new(|f| {
                    let _ = f.stations.remove("waist");
                }),
                "tape waist needs station waist",
            ),
            // A station the resizing needs, with no tape that names it.
            (
                Box::new(|f| {
                    let _ = f.stations.remove("neck");
                }),
                "needs station neck",
            ),
            // No bust or chest station (and no ring tape naming bust).
            (
                Box::new(|f| {
                    let _ = f.stations.remove("bust");
                    let _ = f.tapes.remove("bust");
                }),
                "needs a bust or chest station",
            ),
            // Inputs.
            (
                Box::new(|f| {
                    let _ = f.ranges.remove("hip");
                }),
                "input hip has no valid range",
            ),
            (
                Box::new(|f| f.inputs.push("front_waist_length".into())),
                "input front_waist_length is neither",
            ),
            (
                Box::new(|f| f.inputs.push("knee".into())),
                "input knee is neither",
            ),
            // Two girth inputs on one ring, or one input listed twice.
            (
                Box::new(|f| {
                    let ring = f.stations["bust"];
                    let _ = f.stations.insert("under_bust".into(), ring);
                }),
                "inputs bust and under_bust are both girths at ring 51",
            ),
            (
                Box::new(|f| f.inputs.push("hip".into())),
                "input hip is listed twice",
            ),
            (
                Box::new(|f| f.inputs.push("waist_to_hip".into())),
                "input waist_to_hip is listed twice",
            ),
            // A length needs its tape, and the tape must be a sampled one.
            (
                Box::new(|f| {
                    let _ = f.tapes.remove("cb");
                }),
                "back_waist_length needs tape cb to be a sampled tape",
            ),
            (
                Box::new(|f| {
                    f.tapes.insert(
                        "cf".into(),
                        TapeDef::Ring {
                            ring: "waist".into(),
                        },
                    );
                }),
                "front_waist_length needs tape cf to be a sampled tape",
            ),
            // A length's tape must be open, and cross every station its length is measured to.
            (
                Box::new(|f| {
                    if let Some(TapeDef::Samples { closed, .. }) = f.tapes.get_mut("cb") {
                        *closed = true;
                    }
                }),
                "back_waist_length needs tape cb to be open, not a loop",
            ),
            (
                Box::new(|f| samples(f, "cb").truncate(5)),
                "back_waist_length needs tape cb to cross station waist (at v 0.4125)",
            ),
            (
                Box::new(|f| samples(f, "side_seam").truncate(25)),
                "waist_to_hip needs tape side_seam to cross station hip (at v 0.1500)",
            ),
            // The resizing finds the neck's height from the back_neck landmark.
            (
                Box::new(|f| {
                    let _ = f.landmarks.remove("back_neck");
                }),
                "needs landmark back_neck",
            ),
            // The back of the neck is above the waist.
            (
                Box::new(|f| f.landmarks.get_mut("back_neck").unwrap()[1] = 0.3),
                "back_neck (at v 0.3000) must lie above the waist station (ring 33, at v 0.4125)",
            ),
            // Sampled tapes: too short, not finite, phi or v off the form.
            (
                Box::new(|f| samples(f, "armhole").truncate(2)),
                "tape armhole is too short",
            ),
            (
                Box::new(|f| samples(f, "cf").truncate(1)),
                "tape cf is too short",
            ),
            (
                Box::new(|f| samples(f, "armhole")[3][1] = f64::NAN),
                "tape armhole has a sample that is not finite",
            ),
            (
                Box::new(|f| samples(f, "shoulder_seam")[2][0] = 7.0),
                "tape shoulder_seam has phi 7 outside 0..=2π",
            ),
            (
                Box::new(|f| samples(f, "side_seam")[2][0] = -0.1),
                "tape side_seam has phi -0.1 outside 0..=2π",
            ),
            (
                Box::new(|f| samples(f, "side_seam")[2][1] = 1.5),
                "tape side_seam has v 1.5 outside 0..=1",
            ),
            // The stand: finite, and a tilt strictly between 0 and 90 degrees.
            (
                Box::new(|f| f.stand.pole_xz[1] = f64::INFINITY),
                "stand pole is not finite",
            ),
            (
                Box::new(|f| f.stand.neck_cut.y = f64::NAN),
                "stand neck cut height is not finite",
            ),
            (
                Box::new(|f| f.stand.neck_cut.tilt_deg = 0.0),
                "tilts 0 degrees, expected between 0 and 90",
            ),
            (
                Box::new(|f| f.stand.neck_cut.tilt_deg = 90.0),
                "tilts 90 degrees, expected between 0 and 90",
            ),
            (
                Box::new(|f| f.stand.neck_cut.tilt_deg = f64::NAN),
                "tilts NaN degrees, expected between 0 and 90",
            ),
            // Collision: a gap above 0, a friction of 0 or more, both finite.
            (
                Box::new(|f| f.collision.thickness = 0.0),
                "collision thickness 0 must be finite and above 0",
            ),
            (
                Box::new(|f| f.collision.thickness = f64::NAN),
                "collision thickness NaN must be finite and above 0",
            ),
            (
                Box::new(|f| f.collision.friction = -0.1),
                "collision friction -0.1 must be finite and 0 or more",
            ),
            (
                Box::new(|f| f.collision.friction = f64::INFINITY),
                "collision friction inf must be finite and 0 or more",
            ),
        ];
        for (break_it, needle) in cases {
            let mut f = fixture::torso();
            break_it(&mut f);
            let err = f.check().unwrap_err().0;
            assert!(err.contains(needle), "{err:?} should mention {needle:?}");
        }
    }

    /// The resizing finds the neck from `back_neck` and the waist from its station, so a form
    /// whose back neck is level with, or below, its waist is refused, naming both.
    #[test]
    fn a_back_neck_at_or_below_the_waist_is_refused() {
        let mut f = fixture::torso();
        let waist = f.stations["waist"];
        let v = waist as f64 / (f.rings.len() - 1) as f64;
        for back_neck in [0.0, v / 2.0, v] {
            f.landmarks.get_mut("back_neck").unwrap()[1] = back_neck;
            let err = f.check().unwrap_err().0;
            assert!(
                err.contains("back_neck") && err.contains(&format!("waist station (ring {waist}")),
                "{err:?}"
            );
        }
        // Just above the waist is fine, and so are the shipped forms.
        f.landmarks.get_mut("back_neck").unwrap()[1] = v + 0.01;
        f.check().unwrap();
        for json in [WOMEN, MEN] {
            FormFile::from_json(json).unwrap();
        }
    }

    /// A friction of exactly 0 is allowed (a frictionless form).
    #[test]
    fn zero_friction_is_allowed() {
        let mut f = fixture::torso();
        f.collision.friction = 0.0;
        f.check().unwrap();
    }
}
