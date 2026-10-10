//! The `.form.json` file format (format 1): one part of a dress form (today a torso) as
//! horizontal rings, with its stations, landmarks, sampled tape lines, stand, measurement inputs
//! and ranges.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
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
                    if !uv
                        .iter()
                        .all(|&[phi, v]| (0.0..=TAU).contains(&phi) && (0.0..=1.0).contains(&v))
                    {
                        return bad(format!(
                            "tape {name} has a sample off the form (phi must be in 0..=2π, v in 0..=1)"
                        ));
                    }
                }
            }
        }
        let Stand {
            pole_xz,
            neck_cut: NeckCut { y, tilt_deg },
        } = &self.stand;
        if !pole_xz.iter().chain([y]).all(|x| x.is_finite()) {
            return bad("stand has a value that is not finite".into());
        }
        if !(*tilt_deg > 0.0 && *tilt_deg < 90.0) {
            return bad(format!(
                "stand neck cut tilts {tilt_deg} degrees, expected between 0 and 90"
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
        for (m, tape, from, to) in self.lengths() {
            if !matches!(self.tapes.get(*tape), Some(TapeDef::Samples { .. })) {
                return bad(format!("{m} needs tape {tape} to be a sampled tape"));
            }
            for place in [from, to] {
                if !matches!(*place, "start" | "end") && !self.stations.contains_key(*place) {
                    return bad(format!("{m} needs station {place}"));
                }
            }
        }
        for m in &self.inputs {
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

    #[test]
    fn fixture_torso_passes_the_checks() {
        fixture::torso().check().unwrap();
    }

    #[test]
    fn round_trips_through_json() {
        let f = fixture::torso();
        let json = serde_json::to_string(&f).unwrap();
        assert_eq!(FormFile::from_json(&json).unwrap(), f);
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
            let f = FormFile::from_json(json).unwrap();
            f.check().unwrap();
            assert_eq!(f.id, id);
            assert_eq!(f.kind, Kind::Torso);
            assert_eq!(f.base_size, base_size);
            assert_eq!(f.angles, ANGLES);
            assert_eq!(f.chest_station(), Some(chest));
            assert_eq!(f.landmarks.len(), 15);
            // The pole stands on the vertical line through the origin; the neck is cut 17 degrees.
            assert_eq!(f.stand.pole_xz, [0.0, 0.0]);
            assert_eq!(f.stand.neck_cut.tilt_deg, 17.0);
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

    #[test]
    fn broken_references_are_refused() {
        type Breaker = Box<dyn Fn(&mut FormFile)>;
        let cases: Vec<(Breaker, &str)> = vec![
            (
                Box::new(|f| {
                    let _ = f.stations.remove("waist");
                }),
                "waist",
            ),
            (
                Box::new(|f| {
                    let _ = f.ranges.remove("hip");
                }),
                "hip",
            ),
            (
                Box::new(|f| {
                    let _ = f.rings[3].r.pop();
                }),
                "ring 3",
            ),
            (Box::new(|f| f.rings[5].y = f.rings[4].y), "ring 5"),
            (Box::new(|f| f.format = 2), "format 2"),
            (Box::new(|f| f.angles = 48), "48 angles"),
            (
                Box::new(|f| {
                    let _ = f.stations.insert("hip".into(), 500);
                }),
                "station hip",
            ),
            (
                Box::new(|f| f.landmarks.get_mut("bust_apex").unwrap()[0] = 4.0),
                "bust_apex",
            ),
            // An input that is neither a station girth nor an adjustable length.
            (
                Box::new(|f| f.inputs.push("front_waist_length".into())),
                "front_waist_length",
            ),
            (Box::new(|f| f.inputs.push("knee".into())), "knee"),
            // A length needs its tape, and the tape must be a sampled one.
            (
                Box::new(|f| {
                    let _ = f.tapes.remove("cb");
                }),
                "tape cb",
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
                "tape cf",
            ),
            // Sampled tapes: not finite, phi or v off the form, too short.
            (
                Box::new(|f| samples(f, "armhole")[3][1] = f64::NAN),
                "armhole",
            ),
            (
                Box::new(|f| samples(f, "shoulder_seam")[2][0] = 7.0),
                "shoulder_seam",
            ),
            (
                Box::new(|f| samples(f, "side_seam")[2][0] = -0.1),
                "side_seam",
            ),
            (
                Box::new(|f| samples(f, "side_seam")[2][1] = 1.5),
                "side_seam",
            ),
            (Box::new(|f| samples(f, "armhole").truncate(2)), "armhole"),
            (Box::new(|f| samples(f, "cf").truncate(1)), "cf"),
            // The stand: finite, and a tilt strictly between 0 and 90 degrees.
            (Box::new(|f| f.stand.neck_cut.tilt_deg = 0.0), "stand"),
            (Box::new(|f| f.stand.neck_cut.tilt_deg = 90.0), "stand"),
            (Box::new(|f| f.stand.neck_cut.y = f64::NAN), "stand"),
            (Box::new(|f| f.stand.pole_xz[1] = f64::INFINITY), "stand"),
        ];
        for (break_it, needle) in cases {
            let mut f = fixture::torso();
            break_it(&mut f);
            let err = f.check().unwrap_err().0;
            assert!(err.contains(needle), "{err:?} should mention {needle}");
        }
    }
}
