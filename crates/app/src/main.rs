// No console window behind the app in Windows release builds.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use opendrape::cli::Cli;
use opendrape::gpu::{self, GpuState, LockOutcome, Os, Reason, StateStore};
use opendrape::startup_log::stage;
use opendrape::{OpenDrapeApp, SharedState, Startup, smoke_test, startup_log};
use std::process::ExitCode;

/// Counts automatic relaunches in a row, so a broken setup can never relaunch forever.
const RELAUNCH_ENV: &str = "OPENDRAPE_RELAUNCHES";

fn main() -> ExitCode {
    startup_log::init_from_env();
    let os = Os::current();
    let cli = Cli::parse(std::env::args().skip(1));
    let relaunches: u32 = std::env::var(RELAUNCH_ENV)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(0);
    stage(format_args!(
        "OpenDrape {} on {os:?}: {cli:?}, relaunch {relaunches}",
        env!("CARGO_PKG_VERSION")
    ));
    if cli.smoke_test {
        smoke_test::start_watchdog(smoke_test::TIMEOUT, || {
            eprintln!(
                "OpenDrape --smoke-test: no 3D frames after {} s, giving up",
                smoke_test::TIMEOUT.as_secs()
            );
            std::process::exit(smoke_test::TIMED_OUT_EXIT_CODE);
        });
    }

    let mut store = StateStore::default_location();
    // Held until exit. While another copy runs, its crash marker means "starting", and this
    // copy must not overwrite the shared state.
    let lock = store.acquire_instance_lock();
    let another_instance_running = matches!(lock, LockOutcome::HeldElsewhere);
    let previous = gpu::effective_state(store.load(), another_instance_running);
    if another_instance_running {
        store = StateStore::new(None);
    }

    let decision = gpu::decide(cli.gpu, &previous, os);
    if let Reason::AllModesFailed(_) = decision.reason {
        gpu::show_notice(&opendrape::tr!("graphics-starting-over"), cli.interactive());
    }
    let marker_saved =
        gpu::pending_marker(decision, &previous).is_some_and(|marker| store.save(&marker));
    stage(format_args!(
        "config {:?}, another copy running: {another_instance_running}, saved {previous:?}, \
         decided {decision:?}, crash marker saved: {marker_saved}",
        store.path()
    ));

    let shared = SharedState::default();
    let startup = Startup {
        decision,
        previous,
        store: store.clone(),
        smoke_test: cli.smoke_test,
        autoplay: true,
        file_dialogs: opendrape::FileDialogs::Native,
        // A second copy of OpenDrape keeps no recovery file, so the two never overwrite each other's.
        recovery: if another_instance_running {
            opendrape::Recovery::new(None)
        } else {
            opendrape::Recovery::default_location()
        },
    };
    let app_shared = shared.clone();
    stage("opening the window");
    let result = eframe::run_native(
        "OpenDrape",
        gpu::native_options(decision.choice),
        Box::new(move |cc| Ok(Box::new(OpenDrapeApp::new(cc, startup, app_shared)))),
    );
    drop(lock);
    stage(format_args!(
        "window closed: {result:?}, 3D frames confirmed: {}",
        shared.first_frame_drawn.get()
    ));

    match result {
        Ok(()) if cli.smoke_test => {
            if shared.first_frame_drawn.get() {
                ExitCode::SUCCESS
            } else {
                ExitCode::FAILURE
            }
        }
        Ok(()) => {
            if shared.restart_with.get().is_some() {
                relaunch(0);
            }
            ExitCode::SUCCESS
        }
        Err(err) => {
            // The GPU or window failed without crashing. The crash marker is on disk, so a
            // fresh process moves on to the next graphics mode.
            if !cli.smoke_test
                && gpu::should_relaunch_after_error(decision, os, marker_saved, relaunches)
            {
                relaunch(relaunches + 1);
                return ExitCode::SUCCESS;
            }
            // Nothing left to try: tell the user, and start from the beginning next time.
            if decision.reason != Reason::CommandLine {
                store.save(&GpuState::default());
            }
            gpu::show_startup_error(&err.to_string(), cli.interactive());
            ExitCode::FAILURE
        }
    }
}

/// Start a fresh copy of OpenDrape. `--gpu=` is dropped so the saved state decides.
fn relaunch(relaunches: u32) {
    if let Ok(exe) = std::env::current_exe() {
        let args = std::env::args()
            .skip(1)
            .filter(|a| !a.starts_with("--gpu="));
        let _ = std::process::Command::new(exe)
            .args(args)
            .env(RELAUNCH_ENV, relaunches.to_string())
            .spawn();
    }
}
