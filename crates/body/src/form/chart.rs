//! Size charts: named sizes for one form, each a full set of measurements in millimetres.

use super::{FormError, Measurements};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ChartSize {
    pub label: String,
    /// Another name for the same size (e.g. "UK 12"), if there is one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub alt: Option<String>,
    pub mm: Measurements,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Chart {
    pub format: u32,
    pub id: String,
    /// The id of the form this chart sizes.
    pub form: String,
    /// Fluent message id of the chart's name ("Classic form", "Everyday body").
    pub name: String,
    /// Where the numbers come from (for ASSETS.md, not shown in the app).
    pub source: String,
    pub sizes: Vec<ChartSize>,
}

const BUNDLED: [&str; 4] = [
    include_str!("../../../../assets/forms/charts/women-torso-classic.json"),
    include_str!("../../../../assets/forms/charts/women-torso-everyday.json"),
    include_str!("../../../../assets/forms/charts/men-torso-classic.json"),
    include_str!("../../../../assets/forms/charts/men-torso-everyday.json"),
];

impl Chart {
    pub fn from_json(s: &str) -> Result<Self, FormError> {
        serde_json::from_str(s).map_err(|e| FormError(format!("not a size chart: {e}")))
    }

    /// Every bundled chart, women's classic first.
    pub fn bundled() -> Vec<Chart> {
        BUNDLED
            .iter()
            .map(|s| Chart::from_json(s).expect("bundled charts are valid"))
            .collect()
    }

    /// The bundled charts that size the form `form_id`, in picker order.
    pub fn for_form(form_id: &str) -> Vec<Chart> {
        Self::bundled()
            .into_iter()
            .filter(|c| c.form == form_id)
            .collect()
    }

    pub fn size(&self, label: &str) -> Option<&ChartSize> {
        self.sizes.iter().find(|s| s.label == label)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_chart_round_trips_through_json_and_keeps_the_alternative_names() {
        let c = Chart::bundled().remove(0);
        assert_eq!(c.id, "women-torso-classic");
        assert_eq!(c.size("US 8").and_then(|s| s.alt.as_deref()), Some("UK 12"));
        let back = Chart::from_json(&serde_json::to_string(&c).unwrap()).unwrap();
        assert_eq!(back, c);
        // The men's charts have no second name for a size, and write none.
        let men = Chart::for_form("men-torso").remove(0);
        assert_eq!(men.size("40").unwrap().alt, None);
        assert!(!serde_json::to_string(&men).unwrap().contains("alt"));
    }

    #[test]
    fn json_that_is_not_a_chart_is_refused() {
        for s in ["{}", "[]", "not json", r#"{"format": 1, "sizes": []}"#] {
            let err = Chart::from_json(s).unwrap_err().0;
            assert!(err.starts_with("not a size chart"), "{err:?}");
        }
    }

    #[test]
    fn the_bundled_charts_are_in_picker_order() {
        let ids: Vec<_> = Chart::bundled().into_iter().map(|c| c.id).collect();
        assert_eq!(
            ids,
            [
                "women-torso-classic",
                "women-torso-everyday",
                "men-torso-classic",
                "men-torso-everyday"
            ]
        );
    }
}
