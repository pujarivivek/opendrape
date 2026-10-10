//! Dress forms: shapes stored as horizontal rings (see `file`), resized to a size chart or to
//! custom measurements, and built into closed meshes for collision and drawing.

mod file;
#[cfg(test)]
mod fixture;
mod rings;

pub use file::{
    ADJUSTABLE_LENGTHS, ANGLES, Collision, FORMAT, FormError, FormFile, Kind, NeckCut, Ring, Stand,
    TORSO_LENGTHS, TORSO_STATIONS, TapeDef,
};
pub use rings::Rings;
