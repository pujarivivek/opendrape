//! Start-up stages on stderr, for a window that never appears or never draws.
//!
//! Off unless `OPENDRAPE_STARTUP_LOG=1` is set. Windows release builds have no console, so
//! redirect stderr to read it: `opendrape.exe 2> startup.txt` in a command prompt.

use std::ffi::OsStr;
use std::fmt::Display;
use std::sync::OnceLock;
use std::time::{Duration, Instant};

pub const ENV: &str = "OPENDRAPE_STARTUP_LOG";

/// When the log was turned on; unset while it is off.
static STARTED: OnceLock<Instant> = OnceLock::new();

/// Turn the log on if the environment asks for it. Call first thing in `main()`.
pub fn init_from_env() {
    if enabled_by(std::env::var_os(ENV).as_deref()) {
        let _ = STARTED.set(Instant::now());
    }
}

/// Write one start-up stage to stderr, if the log is on.
pub fn stage(what: impl Display) {
    if let Some(started) = STARTED.get() {
        eprintln!("{}", line(started.elapsed(), what));
    }
}

fn enabled_by(value: Option<&OsStr>) -> bool {
    value.is_some_and(|v| !v.is_empty() && v != "0")
}

fn line(elapsed: Duration, what: impl Display) -> String {
    format!("OpenDrape start-up {:>6} ms: {what}", elapsed.as_millis())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn off_unless_the_variable_is_set_to_something_other_than_0() {
        assert!(!enabled_by(None));
        assert!(!enabled_by(Some(OsStr::new(""))));
        assert!(!enabled_by(Some(OsStr::new("0"))));
        assert!(enabled_by(Some(OsStr::new("1"))));
        assert!(enabled_by(Some(OsStr::new("yes"))));
    }

    #[test]
    fn each_line_says_how_long_after_launch_the_stage_was_reached() {
        // A slow stage (WARP compiling shaders) and a stuck one look different in the log.
        assert_eq!(
            line(Duration::from_millis(1234), "window open"),
            "OpenDrape start-up   1234 ms: window open"
        );
    }
}
