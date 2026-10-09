//! OpenDrape desktop application.

mod app;
pub mod cli;
pub mod diagnostics;
pub mod gpu;
#[doc(hidden)]
pub mod i18n;
mod viewport;

pub use app::{OpenDrapeApp, Shared, SharedState, Startup};
