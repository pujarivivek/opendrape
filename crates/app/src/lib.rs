//! OpenDrape desktop application.

mod app;
pub mod arrange;
pub mod cli;
pub mod diagnostics;
pub mod draping;
pub mod editor;
pub mod file_dialogs;
pub mod gpu;
#[doc(hidden)]
pub mod i18n;
pub mod icons;
pub mod recovery;
pub mod sim_runner;
pub mod smoke_test;
pub mod startup_log;
pub mod theme;
mod viewport;
pub mod workspace;

pub use app::{OpenDrapeApp, Shared, SharedState, Startup};
pub use file_dialogs::{DialogKind, FileDialogs};
pub use opendrape_drape::stage;
pub use recovery::Recovery;
pub use sim_runner::SimFrame;
