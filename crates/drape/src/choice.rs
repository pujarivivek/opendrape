//! The dress form a project names (`FormChoice`), made real: its charts, its sizes, and the
//! built form. The one place the app and the tests turn a choice into a form.

use opendrape_body::form::{BuiltForm, Chart, Form, Quality, SizeError};
use opendrape_core::{FormChoice, FormSize};

/// Why a choice can't be built.
#[derive(Clone, Debug, PartialEq)]
pub enum FormProblem {
    /// No bundled form has this id.
    Unknown(String),
    /// The form can't take one of the measurements.
    Size(SizeError),
}

impl std::fmt::Display for FormProblem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Unknown(id) => {
                write!(f, "there is no dress form called \"{id}\" in this version")
            }
            Self::Size(e) => e.fmt(f),
        }
    }
}

impl std::error::Error for FormProblem {}

/// The form's charts, in picker order, each with its kind (`classic`, `everyday`): the part of
/// its id after the form's.
pub fn charts(form_id: &str) -> Vec<(String, Chart)> {
    let prefix = format!("{form_id}-");
    Chart::for_form(form_id)
        .into_iter()
        .filter_map(|c| Some((c.id.strip_prefix(&prefix)?.to_string(), c)))
        .collect()
}

/// Row `label` of the form's `kind` chart.
pub fn chart_choice(form_id: &str, kind: &str, label: &str) -> Option<FormChoice> {
    let (_, chart) = charts(form_id).into_iter().find(|(k, _)| k == kind)?;
    let row = chart.size(label)?;
    Some(FormChoice {
        id: form_id.to_string(),
        size: FormSize::Chart {
            chart: kind.to_string(),
            label: label.to_string(),
        },
        measurements: row.mm.clone(),
    })
}

/// The form at the size it was shaped at, from its Classic chart.
pub fn base_choice(form_id: &str) -> Option<FormChoice> {
    let form = Form::bundled(form_id)?;
    chart_choice(form_id, "classic", &form.file().base_size)
}

/// The girth sizes are known by: the form's first input (`bust`, or `chest`).
pub fn girth_name(form_id: &str) -> Option<String> {
    Form::bundled(form_id)?
        .inputs()
        .into_iter()
        .next()
        .map(|(name, _)| name)
}

/// The row of the form's `kind` chart nearest `choice` by the girth.
pub fn nearest_in(choice: &FormChoice, kind: &str) -> Option<FormChoice> {
    let girth = girth_name(&choice.id)?;
    let now = *choice.measurements.get(&girth)?;
    let (_, chart) = charts(&choice.id).into_iter().find(|(k, _)| k == kind)?;
    let off = |mm: &opendrape_body::form::Measurements| {
        mm.get(&girth).map_or(f64::INFINITY, |v| (v - now).abs())
    };
    let row = chart
        .sizes
        .iter()
        .min_by(|a, b| off(&a.mm).total_cmp(&off(&b.mm)))?;
    chart_choice(&choice.id, kind, &row.label)
}

/// `choice` with measurement `name` set to `mm`: a custom size.
pub fn with_measurement(choice: &FormChoice, name: &str, mm: f64) -> FormChoice {
    let mut out = choice.clone();
    out.size = FormSize::Custom;
    out.measurements.insert(name.to_string(), mm);
    out
}

/// The form `choice` names, built at its measurements.
pub fn build_form(choice: &FormChoice) -> Result<BuiltForm, FormProblem> {
    let form = Form::bundled(&choice.id).ok_or_else(|| FormProblem::Unknown(choice.id.clone()))?;
    form.build(&choice.measurements, Quality::Standard)
        .map_err(FormProblem::Size)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_core_default_is_the_bundled_womens_classic_us_8() {
        assert_eq!(base_choice("women-torso"), Some(FormChoice::default()));
    }

    #[test]
    fn each_form_has_a_classic_and_an_everyday_chart() {
        for id in Form::IDS {
            let kinds: Vec<String> = charts(id).into_iter().map(|(k, _)| k).collect();
            assert_eq!(kinds, ["classic", "everyday"], "{id}");
        }
        assert_eq!(girth_name("women-torso").as_deref(), Some("bust"));
        assert_eq!(girth_name("men-torso").as_deref(), Some("chest"));
        assert_eq!(
            base_choice("men-torso").unwrap().size,
            FormSize::Chart {
                chart: "classic".into(),
                label: "40".into()
            }
        );
        assert_eq!(girth_name("child-torso"), None);
    }

    #[test]
    fn switching_chart_keeps_the_nearest_size_by_girth() {
        let us8 = FormChoice::default(); // bust 900
        let everyday = nearest_in(&us8, "everyday").unwrap();
        let bust = everyday.measurements["bust"];
        let (_, chart) = charts("women-torso")
            .into_iter()
            .find(|(k, _)| k == "everyday")
            .unwrap();
        for s in &chart.sizes {
            assert!((bust - 900.0).abs() <= (s.mm["bust"] - 900.0).abs());
        }
        assert!(matches!(everyday.size, FormSize::Chart { ref chart, .. } if chart == "everyday"));
        // A custom size is matched by its girth too.
        let custom = with_measurement(&us8, "bust", 1000.0);
        let back = nearest_in(&custom, "classic").unwrap();
        assert!(
            (back.measurements["bust"] - 1000.0).abs() < 30.0,
            "{back:?}"
        );
    }

    #[test]
    fn a_custom_measurement_keeps_the_rest_and_says_custom() {
        let c = with_measurement(&FormChoice::default(), "waist", 712.0);
        assert_eq!(c.size, FormSize::Custom);
        assert_eq!(c.measurements["waist"], 712.0);
        assert_eq!(c.measurements["bust"], 900.0);
    }

    #[test]
    fn what_cannot_be_built_says_why() {
        let c = FormChoice {
            id: "child-torso".into(),
            ..FormChoice::default()
        };
        assert_eq!(
            build_form(&c).err(),
            Some(FormProblem::Unknown("child-torso".into()))
        );
        assert!(
            FormProblem::Unknown("child-torso".into())
                .to_string()
                .contains("child-torso")
        );
        let c = with_measurement(&FormChoice::default(), "waist", 2000.0);
        assert!(
            matches!(build_form(&c), Err(FormProblem::Size(ref e)) if e.measurement == "waist")
        );
        assert!(chart_choice("women-torso", "classic", "US 99").is_none());
        assert!(chart_choice("women-torso", "nordic", "US 8").is_none());
        assert!(build_form(&FormChoice::default()).is_ok());
    }
}
