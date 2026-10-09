//! Which graphics backend to start, and recovery when one crashes or fails.

mod choice;

pub use choice::{GpuChoice, Os, pick_adapter};

mod state;

pub use state::{
    Decision, GpuState, Reason, StateStore, confirmed_state, decide, pending_marker,
    should_relaunch_after_error,
};

mod setup;

pub use setup::{native_options, show_startup_error};
