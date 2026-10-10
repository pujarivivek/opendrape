//! Dress forms: shapes stored as horizontal rings (see `file`), resized to a size chart or to
//! custom measurements, and built into closed meshes for collision and drawing.

mod cut;
mod file;
#[cfg(test)]
mod fixture;
pub mod resize;
mod rings;
mod stand;
pub mod tape;
#[cfg(test)]
mod testing;

use crate::BodyMesh;
use glam::DVec3;
use std::collections::BTreeMap;
use std::f64::consts::{PI, TAU};

pub use file::{
    ADJUSTABLE_LENGTHS, ANGLES, Collision, FORMAT, FormError, FormFile, Kind, NeckCut, Ring, Stand,
    TORSO_LENGTHS, TORSO_STATIONS, TapeDef,
};
pub use rings::Rings;

/// Measurements in millimetres, by name (`bust`, `waist`, `back_waist_length`, …).
pub type Measurements = std::collections::BTreeMap<String, f64>;

/// A size the form cannot take: which measurement, and the range it may have here.
#[derive(Clone, Debug, PartialEq)]
pub struct SizeError {
    pub measurement: String,
    pub min_mm: f64,
    pub max_mm: f64,
}

impl std::fmt::Display for SizeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{} must be {:.0}–{:.0} mm on this form",
            self.measurement, self.min_mm, self.max_mm
        )
    }
}

impl std::error::Error for SizeError {}

/// Mesh density: `Standard` has 96 samples around each ring, `Low` 64 (for weak laptops).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Quality {
    Low,
    #[default]
    Standard,
}

impl Quality {
    /// Radii per side of a ring, centre front to centre back inclusive.
    fn half_angles(self) -> usize {
        match self {
            Quality::Low => 33,
            Quality::Standard => ANGLES,
        }
    }
}

/// A dress form at one size, ready to drape on and draw.
#[derive(Clone, Debug)]
pub struct BuiltForm {
    /// Closed torso mesh, for collision and drawing.
    pub torso: BodyMesh,
    /// Tape-line ribbons, for drawing only.
    pub tapes: BodyMesh,
    /// Neck cap, rod, knob, pole and base, for drawing only. Each piece is closed.
    pub stand: BodyMesh,
    /// Landmark positions, metres; off-centre landmarks also as `<name>_R` on the right side.
    pub landmarks: BTreeMap<String, DVec3>,
    /// Station heights, metres.
    pub stations: BTreeMap<String, f64>,
    /// Every measurement of this build, millimetres: all the inputs, the girth at each ring
    /// tape's station, `front_waist_length`, and `apex_to_apex` on forms with a bust.
    pub measured: Measurements,
    pub collision: Collision,
}

/// A torso form that can be built at any size it takes.
#[derive(Clone, Debug)]
pub struct Form {
    torso: FormFile,
}

impl Form {
    /// Takes a form file once it passes `FormFile::check`.
    pub fn new(torso: FormFile) -> Result<Self, FormError> {
        torso.check()?;
        Ok(Self { torso })
    }

    pub fn file(&self) -> &FormFile {
        &self.torso
    }

    /// Every measurement a user can set, with its range in mm, in the order the file lists them.
    pub fn inputs(&self) -> Vec<(String, [f64; 2])> {
        self.torso
            .inputs
            .iter()
            .map(|m| (m.clone(), self.torso.ranges[m]))
            .collect()
    }

    /// The form's own measurements before any resizing, mm.
    pub fn base_measurements(&self, quality: Quality) -> Measurements {
        let rings = Rings::from_file(&self.torso).with_half_angles(quality.half_angles());
        measure_all(&self.torso, &rings, &landmarks(&self.torso, &rings))
    }

    /// The unsized rings and the rings at `size` (mm), resized and re-cut to the neck's plane
    /// (with the neck's wall, for the stand), or the measurement the form cannot take.
    fn rings(
        &self,
        size: &Measurements,
        quality: Quality,
    ) -> Result<(Rings, cut::Trim), SizeError> {
        let base = Rings::from_file(&self.torso).with_half_angles(quality.half_angles());
        let sized = resize::resize(&self.torso, &base, size)?;
        let trim = cut::recut(&self.torso, &base, &sized);
        Ok((base, trim))
    }

    /// The form at `size` (mm), or the measurement it cannot take.
    pub fn build(&self, size: &Measurements, quality: Quality) -> Result<BuiltForm, SizeError> {
        let (base, trim) = self.rings(size, quality)?;
        let rings = &trim.rings;
        let landmarks = landmarks(&self.torso, rings);
        let tapes = tape::tapes(&self.torso, rings);
        Ok(BuiltForm {
            torso: rings.mesh(),
            tapes: tape::ribbons(&tapes, rings),
            stand: stand::stand(&self.torso, &base, rings, &trim.wall),
            stations: self
                .torso
                .stations
                .iter()
                .map(|(name, &i)| (name.clone(), rings.y[i]))
                .collect(),
            measured: measure_all(&self.torso, rings, &landmarks),
            landmarks,
            collision: self.torso.collision.clone(),
        })
    }
}

/// Landmarks within this many radians of centre front or back count as on the centre line: the
/// shipped files keep 5 decimals, so their centre back reads 3.14159 and not π.
const CENTRE_LINE: f64 = 1e-4;

/// Each landmark on the surface; those off the centre line also as `<name>_R`, mirrored.
fn landmarks(file: &FormFile, rings: &Rings) -> BTreeMap<String, DVec3> {
    let mut out = BTreeMap::new();
    for (name, &[phi, v]) in &file.landmarks {
        out.insert(name.clone(), rings.point(phi, v));
        if phi > CENTRE_LINE && phi < PI - CENTRE_LINE {
            out.insert(format!("{name}_R"), rings.point(TAU - phi, v));
        }
    }
    out
}

/// What `Form::build` reports as `measured`, mm: the girth at every input station and every ring
/// tape's station, every length, and apex to apex on forms with a bust.
fn measure_all(
    file: &FormFile,
    rings: &Rings,
    landmarks: &BTreeMap<String, DVec3>,
) -> Measurements {
    let ring_tapes = file.tapes.values().filter_map(|t| match t {
        TapeDef::Ring { ring } => Some(ring),
        TapeDef::Samples { .. } => None,
    });
    let girths = file
        .inputs
        .iter()
        .filter(|m| file.stations.contains_key(*m))
        .chain(ring_tapes);
    let mut out = Measurements::new();
    for station in girths {
        out.insert(
            station.clone(),
            rings.girth(file.stations[station]) * 1000.0,
        );
    }
    for &(m, ..) in file.lengths() {
        out.insert(m.to_string(), resize::length(file, rings, m) * 1000.0);
    }
    if file.stations.contains_key("bust")
        && let (Some(a), Some(b)) = (landmarks.get("bust_apex"), landmarks.get("bust_apex_R"))
    {
        out.insert("apex_to_apex".into(), a.distance(*b) * 1000.0);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::testing::*;
    use super::*;
    use crate::boundary_edge_count;

    fn form() -> Form {
        Form::new(fixture::torso()).unwrap()
    }

    fn own_size(f: &Form) -> Measurements {
        f.base_measurements(Quality::Standard)
    }

    #[test]
    fn builds_a_closed_torso_with_tapes_landmarks_and_stand() {
        let f = form();
        let b = f.build(&own_size(&f), Quality::Standard).unwrap();
        assert_eq!(boundary_edge_count(&b.torso), 0);
        assert_eq!(b.torso.triangles.len(), 2 * 96 * 81);
        assert!(!b.tapes.triangles.is_empty());
        assert_eq!(boundary_edge_count(&b.stand), 0);
        assert_eq!(lowest(&b.stand), 0.0);
        let (l, r) = (b.landmarks["bust_apex"], b.landmarks["bust_apex_R"]);
        assert!((l.x + r.x).abs() < 1e-9 && l.x > 0.0);
        assert!((l.y - r.y).abs() < 1e-9 && (l.z - r.z).abs() < 1e-9);
        assert!(
            !b.landmarks.contains_key("front_neck_R"),
            "centre-line marks have no mirror"
        );
        assert!(!b.landmarks.contains_key("cb_blade_R"));
        assert!((b.stations["waist"] - 1.03).abs() < 1e-9);
        assert_eq!(b.collision, f.file().collision);
    }

    /// Landmarks are the file's, placed on the built torso: each lies on its triangles (within
    /// 1 mm; in fact a micrometre) and the right-hand copies mirror the left exactly.
    fn assert_landmarks_on_the_torso(id: &str, f: &Form, b: &BuiltForm) {
        assert_eq!(
            b.landmarks.len(),
            15 + 8,
            "{id}: 15 landmarks, 8 of them off the centre line"
        );
        for (name, &[phi, _]) in &f.file().landmarks {
            let off_centre = phi > CENTRE_LINE && phi < PI - CENTRE_LINE;
            let right = format!("{name}_R");
            assert_eq!(b.landmarks.contains_key(&right), off_centre, "{id}: {name}");
            for key in [name, &right] {
                if let Some(&p) = b.landmarks.get(key) {
                    let off = distance_to_mesh(p, &b.torso);
                    assert!(
                        off < 1e-3,
                        "{id}: {key} is {:.3} mm off the torso",
                        off * 1e3
                    );
                }
            }
            if off_centre {
                let (l, r) = (b.landmarks[name], b.landmarks[&right]);
                assert!((l.x + r.x).abs() < 1e-9 && l.x > 0.0, "{id}: {name}");
                assert!(
                    (l.y - r.y).abs() < 1e-9 && (l.z - r.z).abs() < 1e-9,
                    "{id}: {name}"
                );
            }
        }
    }

    #[test]
    fn landmarks_sit_on_the_sized_torso_and_mirror_exactly() {
        let f = form();
        let mut size = own_size(&f);
        size.insert("hip".into(), size["hip"] * 1.1);
        let b = f.build(&size, Quality::Standard).unwrap();
        assert_landmarks_on_the_torso("fixture", &f, &b);
        // They follow the sizing: the same landmarks sit higher on a longer back.
        let mut longer = size.clone();
        *longer.get_mut("back_waist_length").unwrap() += 30.0;
        let c = f.build(&longer, Quality::Standard).unwrap();
        assert_landmarks_on_the_torso("fixture, longer back", &f, &c);
        assert!(c.landmarks["back_neck"].y > b.landmarks["back_neck"].y + 0.02);
        assert!((c.landmarks["cb_bottom"].y - b.landmarks["cb_bottom"].y).abs() < 1e-3);
    }

    #[test]
    fn measured_has_every_input_and_the_read_only_values() {
        let f = form();
        let size = own_size(&f);
        let b = f.build(&size, Quality::Standard).unwrap();
        let extra = ["high_hip", "front_waist_length", "apex_to_apex"];
        for m in f.file().inputs.iter().map(String::as_str).chain(extra) {
            assert!(
                b.measured.get(m).is_some_and(|v| v.is_finite() && *v > 0.0),
                "{m}"
            );
        }
        assert_eq!(b.measured.len(), f.file().inputs.len() + extra.len());
        // Built at its own size, the form reads back the sizes it was asked for.
        for (m, v) in &size {
            assert!(
                (b.measured[m] - v).abs() < 0.5,
                "{m}: {} vs {v}",
                b.measured[m]
            );
        }
    }

    #[test]
    fn inputs_list_every_input_with_its_range() {
        let f = form();
        let inputs = f.inputs();
        assert_eq!(
            inputs.iter().map(|i| i.0.as_str()).collect::<Vec<_>>(),
            f.file()
                .inputs
                .iter()
                .map(String::as_str)
                .collect::<Vec<_>>()
        );
        for (m, range) in inputs {
            assert_eq!(range, f.file().ranges[&m]);
        }
    }

    #[test]
    fn low_quality_has_fewer_triangles() {
        let f = form();
        let b = f
            .build(&f.base_measurements(Quality::Low), Quality::Low)
            .unwrap();
        assert_eq!(b.torso.triangles.len(), 2 * 64 * 81);
        assert_eq!(boundary_edge_count(&b.torso), 0);
        assert_eq!(boundary_edge_count(&b.stand), 0);
        assert_eq!(Quality::default(), Quality::Standard);
    }

    #[test]
    fn size_errors_come_back_from_build() {
        let f = form();
        let mut size = own_size(&f);
        size.insert("waist".into(), 400.0);
        let e = f.build(&size, Quality::Standard).unwrap_err();
        assert_eq!(e.measurement, "waist");
        let mut size = own_size(&f);
        size.remove("hip");
        assert_eq!(
            f.build(&size, Quality::Standard).unwrap_err().measurement,
            "hip"
        );
    }

    #[test]
    fn a_form_file_that_fails_its_checks_is_refused() {
        let mut file = fixture::torso();
        file.stations.remove("waist");
        assert!(Form::new(file).is_err());
    }

    fn real(json: &str, extreme: &[(&str, f64)]) {
        let f = Form::new(FormFile::from_json(json).unwrap()).unwrap();
        let id = f.file().id.clone();
        let women = f.file().stations.contains_key("bust");
        let with = |base: Measurements, changes: &[(&str, f64)]| {
            let mut size = base;
            for &(m, mm) in changes {
                size.insert(m.to_string(), mm);
            }
            size
        };
        let (small, large) = neck_extremes(f.file());
        let own = own_size(&f);
        let low = f.base_measurements(Quality::Low);
        for (case, size, quality) in [
            ("own size", own.clone(), Quality::Standard),
            ("extreme", with(own.clone(), extreme), Quality::Standard),
            (
                "smallest neck",
                with(own.clone(), &[("neck", small)]),
                Quality::Standard,
            ),
            (
                "largest neck",
                with(own, &[("neck", large)]),
                Quality::Standard,
            ),
            ("low quality", low.clone(), Quality::Low),
            ("low extreme", with(low, extreme), Quality::Low),
        ] {
            let case = format!("{id} {case}");
            let b = f
                .build(&size, quality)
                .unwrap_or_else(|e| panic!("{case}: {e}"));
            assert_eq!(boundary_edge_count(&b.torso), 0, "{case}");
            assert_eq!(boundary_edge_count(&b.stand), 0, "{case}");
            assert_eq!(lowest(&b.stand), 0.0, "{case}");
            assert!(!b.tapes.triangles.is_empty(), "{case}");
            assert_landmarks_on_the_torso(&case, &f, &b);
            // `measured` has every input, within what the resizing promises (girths 1 mm,
            // lengths 2 mm) even where the neck was trimmed to its plane, the ring-tape
            // girths and front waist length, and apex to apex only where there is a bust.
            for m in &f.file().inputs {
                let tol = if f.file().stations.contains_key(m) {
                    1.0
                } else {
                    2.0
                };
                assert!((b.measured[m] - size[m]).abs() <= tol, "{case}: {m}");
            }
            for m in ["high_hip", "front_waist_length"] {
                assert!(b.measured[m] > 0.0, "{case}: {m}");
            }
            assert_eq!(b.measured.contains_key("apex_to_apex"), women, "{case}");
            if women {
                let a = b.measured["apex_to_apex"];
                assert!((100.0..300.0).contains(&a), "{case}: apex to apex {a}");
            }
            assert!(b.landmarks.contains_key("bust_apex_R"), "{case}");
            // The files keep 5 decimals, so centre back reads 3.14159: still on the centre line.
            for name in [
                "back_neck",
                "back_waist",
                "cb_blade",
                "cb_bottom",
                "front_neck",
            ] {
                assert!(
                    !b.landmarks.contains_key(&format!("{name}_R")),
                    "{case}: {name}"
                );
            }
        }
    }

    #[test]
    fn the_womens_form_builds_at_its_own_size_and_extreme_ones() {
        real(WOMEN, &WOMEN_EXTREME);
    }

    #[test]
    fn the_mens_form_builds_at_its_own_size_and_extreme_ones() {
        // The men's form has a bust_apex landmark (the middle of the chest), but no bust.
        real(MEN, &MEN_EXTREME);
    }

    #[test]
    fn the_stand_rises_with_the_neck() {
        let f = Form::new(FormFile::from_json(WOMEN).unwrap()).unwrap();
        let size = own_size(&f);
        let mut longer = size.clone();
        *longer.get_mut("back_waist_length").unwrap() += 40.0;
        let (own, long) = (
            f.build(&size, Quality::Standard).unwrap(),
            f.build(&longer, Quality::Standard).unwrap(),
        );
        let rise = highest(&long.torso) - highest(&own.torso);
        assert!(rise > 0.03, "the neck rises {rise}");
        assert!(
            (highest(&long.stand) - highest(&own.stand) - rise).abs() < 1e-4,
            "the cap, rod and knob rise with the neck"
        );
    }
}
