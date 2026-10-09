//! The form garments drape on, and the glue from a project to cloth on it. Shared by the app's
//! simulation thread and the tests, so what the tests drape is what the student sees: the
//! [`Stage`] (the body, its frame and its collider with a floor) and [`build_drape`] (the
//! project's fabric at its placements, sewn by its seams). No GPU, no windows.

mod build;
pub mod stage;

pub use build::{DENSITY_KG_M2, DrapeNote, build_drape};
pub use stage::{BodyAndFloor, Stage};
