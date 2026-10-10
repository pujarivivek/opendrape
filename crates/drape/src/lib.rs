//! The form garments drape on, and the glue from a project to cloth on it. Shared by the app's
//! simulation thread and the tests, so what the tests drape is what the student sees: the
//! [`Stage`] (the body, its frame, its arms and its collider with a floor) and [`Drape`] (the
//! project's fabric at its placements, sewn by its seams, its pins held; made again from an
//! edited project, carrying on from where it had got to). No GPU, no windows.

mod build;
pub mod choice;
mod live;
pub mod stage;

pub use build::{
    DENSITY_KG_M2, Drape, DrapeNote, Fabric, FabricPanel, PIN_COMPLIANCE, build_drape,
};
pub use live::SEWN_GAP_M;
pub use stage::{Arm, FORM_ARM_LEAN_DEG, FORM_ARM_LENGTH_M, FORM_ARM_OUT_M, Stage};
