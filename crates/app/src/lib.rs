//! OpenDrape desktop application.

mod app;
pub mod cli;
pub mod diagnostics;
pub mod gpu;
#[doc(hidden)]
pub mod i18n;
pub mod sim_runner;
pub mod smoke_test;
pub mod startup_log;
mod viewport;

pub use app::{OpenDrapeApp, Shared, SharedState, Startup};
pub use sim_runner::SimFrame;
