//! Dress forms: shapes stored as horizontal rings (see `file`), resized to a size chart or to
//! custom measurements, and built into closed meshes for collision and drawing.

mod file;
#[cfg(test)]
mod fixture;
pub mod resize;
mod rings;
pub mod tape;

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
