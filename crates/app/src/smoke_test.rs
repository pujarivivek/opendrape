//! `--smoke-test` must end with an answer, never a hang: CI cannot see the screen.

use std::time::Duration;

/// Long enough for software rendering (WARP) on a slow CI machine to draw its first frames,
/// short enough that the run ends before CI kills it and the start-up log gets printed.
pub const TIMEOUT: Duration = Duration::from_secs(60);

/// Exit code when the smoke test gave up waiting (1 means the graphics failed to start).
pub const TIMED_OUT_EXIT_CODE: i32 = 2;

/// Run `on_timeout` on a background thread once `after` has passed. The thread is never
/// cancelled: a smoke test that passes exits the process before it fires.
pub fn start_watchdog(after: Duration, on_timeout: impl FnOnce() + Send + 'static) {
    let _ = std::thread::Builder::new()
        .name("smoke-test watchdog".into())
        .spawn(move || {
            std::thread::sleep(after);
            on_timeout();
        });
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;
    use std::time::Instant;

    #[test]
    fn the_watchdog_fires_once_the_time_is_up() {
        let (tx, rx) = mpsc::channel();
        let started = Instant::now();
        start_watchdog(Duration::from_millis(50), move || tx.send(()).unwrap());
        assert!(rx.recv_timeout(Duration::from_secs(10)).is_ok());
        assert!(started.elapsed() >= Duration::from_millis(50));
    }

    #[test]
    fn gives_up_before_the_ci_step_is_killed_so_the_log_is_printed() {
        // release.yml waits 90 s for the smoke test before killing it.
        assert!(TIMEOUT < Duration::from_secs(90));
    }
}
