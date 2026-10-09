//! Which graphics backend to start, and recovery when one crashes or fails.

mod choice;
mod setup;
mod state;

pub use choice::{GpuChoice, Os, pick_adapter};
pub use setup::{native_options, show_notice, show_startup_error};
pub use state::{
    Decision, GpuState, LockOutcome, MAX_RELAUNCHES, Reason, StateStore, confirmed_state, decide,
    effective_state, pending_marker, should_relaunch_after_error,
};
