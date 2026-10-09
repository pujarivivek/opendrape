use serde::{Deserialize, Serialize};

/// How lengths are shown and typed. Stored values are always millimetres.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Units {
    #[default]
    Cm,
    Inch,
}

impl Units {
    pub fn mm_per_unit(self) -> f64 {
        match self {
            Self::Cm => 10.0,
            Self::Inch => 25.4,
        }
    }
    pub fn from_mm(self, mm: f64) -> f64 {
        mm / self.mm_per_unit()
    }
    pub fn to_mm(self, value: f64) -> f64 {
        value * self.mm_per_unit()
    }
    pub fn suffix(self) -> &'static str {
        match self {
            Self::Cm => "cm",
            Self::Inch => "in",
        }
    }
    /// The number alone, as shown in a text field: "34.5" (cm, 1 decimal) or "13.58" (inch, 2).
    pub fn format_number(self, mm: f64) -> String {
        match self {
            Self::Cm => format!("{:.1}", self.from_mm(mm)),
            Self::Inch => format!("{:.2}", self.from_mm(mm)),
        }
    }
    /// "34.5 cm" or "13.58 in".
    pub fn format(self, mm: f64) -> String {
        format!("{} {}", self.format_number(mm), self.suffix())
    }
    /// "120.0 cm²" or "18.60 in²".
    pub fn format_area(self, mm2: f64) -> String {
        let per = self.mm_per_unit() * self.mm_per_unit();
        match self {
            Self::Cm => format!("{:.1} cm²", mm2 / per),
            Self::Inch => format!("{:.2} in²", mm2 / per),
        }
    }
    /// A typed number: accepts "34.5", "34,5" and surrounding spaces; `None` unless finite.
    pub fn parse(text: &str) -> Option<f64> {
        text.trim()
            .replace(',', ".")
            .parse::<f64>()
            .ok()
            .filter(|v| v.is_finite())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn converts_and_formats() {
        assert_eq!(Units::Cm.to_mm(34.5), 345.0);
        assert_eq!(Units::Inch.from_mm(254.0), 10.0);
        assert_eq!(Units::Cm.format_number(345.0), "34.5");
        assert_eq!(Units::Inch.format_number(100.0), "3.94");
        assert_eq!(Units::Cm.format(345.0), "34.5 cm");
        assert_eq!(Units::Inch.format(345.0), "13.58 in");
        assert_eq!(Units::Cm.format_area(12_000.0), "120.0 cm²");
        assert_eq!(Units::Inch.format_area(12_000.0), "18.60 in²");
        assert_eq!(Units::Cm.suffix(), "cm");
    }

    #[test]
    fn parses_typed_numbers() {
        assert_eq!(Units::parse("34.5"), Some(34.5));
        assert_eq!(Units::parse(" 34,5 "), Some(34.5));
        assert_eq!(Units::parse("-2"), Some(-2.0));
        assert_eq!(Units::parse(""), None);
        assert_eq!(Units::parse("abc"), None);
        assert_eq!(Units::parse("NaN"), None);
        assert_eq!(Units::parse("inf"), None);
    }
}
