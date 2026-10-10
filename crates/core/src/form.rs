//! Which dress form a project drapes on, and at what size: plain data. Whether a form with that
//! id exists and takes those measurements is for `opendrape-drape` to say (it builds it).

use crate::ModelError;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Longest form id, chart name or size label a project may hold.
pub const MAX_FORM_NAME: usize = 64;
/// Most measurements a form choice may hold.
pub const MAX_FORM_MEASUREMENTS: usize = 32;
/// Every measurement is within this range (mm): far wider than any form's, it only stops a
/// damaged file.
pub const FORM_MM: std::ops::RangeInclusive<f64> = 1.0..=5000.0;

/// The dress form a project drapes on: which one, the size picked, and the measurements it is
/// built at.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FormChoice {
    /// A bundled form: `women-torso` or `men-torso`.
    pub id: String,
    pub size: FormSize,
    /// What the form is built at (mm, by name: `bust`, `waist`, …). Kept in the file, so a
    /// later change to a chart never reshapes a saved project's form.
    pub measurements: BTreeMap<String, f64>,
}

/// How the size was picked.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum FormSize {
    /// A row of one of the form's charts: `classic` or `everyday`, and its label (`US 8`).
    Chart { chart: String, label: String },
    /// Measurements the student typed.
    Custom,
}

impl Default for FormChoice {
    /// The women's torso, Classic chart, US 8: the size it was shaped at. `opendrape-drape`
    /// checks that this matches the bundled chart.
    fn default() -> Self {
        let mm = [
            ("back_waist_length", 420.0),
            ("bust", 900.0),
            ("hip", 930.0),
            ("neck", 340.0),
            ("shoulder_length", 130.0),
            ("under_bust", 750.0),
            ("waist", 675.0),
            ("waist_to_hip", 205.0),
        ];
        Self {
            id: "women-torso".into(),
            size: FormSize::Chart {
                chart: "classic".into(),
                label: "US 8".into(),
            },
            measurements: mm.into_iter().map(|(k, v)| (k.to_string(), v)).collect(),
        }
    }
}

impl FormChoice {
    /// Well formed: its names present and not too long, one to [`MAX_FORM_MEASUREMENTS`]
    /// measurements, each a number in [`FORM_MM`].
    pub fn check(&self) -> Result<(), ModelError> {
        let name_ok = |s: &str| !s.is_empty() && s.len() <= MAX_FORM_NAME;
        let size_ok = match &self.size {
            FormSize::Chart { chart, label } => name_ok(chart) && name_ok(label),
            FormSize::Custom => true,
        };
        let measurements_ok = (1..=MAX_FORM_MEASUREMENTS).contains(&self.measurements.len())
            && self
                .measurements
                .iter()
                .all(|(k, v)| name_ok(k) && FORM_MM.contains(v));
        if name_ok(&self.id) && size_ok && measurements_ok {
            Ok(())
        } else {
            Err(ModelError::BadForm)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ModelError, Project};

    #[test]
    fn the_default_is_the_womens_classic_us_8() {
        let d = FormChoice::default();
        assert_eq!(d.id, "women-torso");
        assert_eq!(
            d.size,
            FormSize::Chart {
                chart: "classic".into(),
                label: "US 8".into()
            }
        );
        assert_eq!(d.measurements["bust"], 900.0);
        assert_eq!(d.measurements["waist_to_hip"], 205.0);
        assert_eq!(d.measurements.len(), 8);
        assert_eq!(d.check(), Ok(()));
    }

    #[test]
    fn a_choice_round_trips_through_json_with_its_size_kind() {
        let mut c = FormChoice {
            size: FormSize::Custom,
            ..FormChoice::default()
        };
        c.measurements.insert("waist".into(), 712.5);
        let json = serde_json::to_string(&c).unwrap();
        assert!(json.contains(r#""kind":"custom""#), "{json}");
        assert_eq!(serde_json::from_str::<FormChoice>(&json).unwrap(), c);
    }

    #[test]
    fn malformed_choices_are_refused() {
        let bad = |f: &dyn Fn(&mut FormChoice)| {
            let mut c = FormChoice::default();
            f(&mut c);
            c.check()
        };
        assert_eq!(bad(&|c| c.id.clear()), Err(ModelError::BadForm));
        assert_eq!(bad(&|c| c.id = "x".repeat(65)), Err(ModelError::BadForm));
        assert_eq!(
            bad(&|c| {
                c.size = FormSize::Chart {
                    chart: String::new(),
                    label: "US 8".into(),
                }
            }),
            Err(ModelError::BadForm)
        );
        assert_eq!(bad(&|c| c.measurements.clear()), Err(ModelError::BadForm));
        assert_eq!(
            bad(&|c| {
                c.measurements.insert("waist".into(), f64::NAN);
            }),
            Err(ModelError::BadForm)
        );
        assert_eq!(
            bad(&|c| {
                c.measurements.insert("waist".into(), 0.5);
            }),
            Err(ModelError::BadForm)
        );
        assert_eq!(
            bad(&|c| {
                for k in 0..40 {
                    c.measurements.insert(format!("m{k}"), 100.0);
                }
            }),
            Err(ModelError::BadForm)
        );
        assert_eq!(
            bad(&|c| {
                c.measurements.insert(String::new(), 100.0);
            }),
            Err(ModelError::BadForm)
        );
    }

    #[test]
    fn a_project_checks_its_form_and_an_older_file_gets_the_default() {
        let mut p = Project::new();
        assert_eq!(p.form, FormChoice::default());
        p.form.id.clear();
        assert_eq!(p.check(), Err(ModelError::BadForm));
        let old: Project = serde_json::from_str(r#"{"schema_version":4}"#).unwrap();
        assert_eq!(old.form, FormChoice::default());
    }
}
