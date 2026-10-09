// No console window behind the app in Windows release builds.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use opendrape::cli::Cli;
use opendrape::gpu::{self, Os, StateStore};
use opendrape::{OpenDrapeApp, SharedState, Startup};
use std::process::ExitCode;

fn main() -> ExitCode {
    let os = Os::current();
    let cli = Cli::parse(std::env::args().skip(1));
    let store = StateStore::default_location();
    let previous = store.load();
    let decision = gpu::decide(cli.gpu, &previous, os);
    if let Some(marker) = gpu::pending_marker(decision, &previous) {
        store.save(&marker);
    }

    let shared = SharedState::default();
    let startup = Startup {
        decision,
        previous,
        store,
        smoke_test: cli.smoke_test,
    };
    let app_shared = shared.clone();
    let result = eframe::run_native(
        "OpenDrape",
        gpu::native_options(decision.choice),
        Box::new(move |cc| Ok(Box::new(OpenDrapeApp::new(cc, startup, app_shared)))),
    );

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
                relaunch();
            }
            ExitCode::SUCCESS
        }
        Err(err) => {
            // The GPU or window failed without crashing. The pending marker is still on
            // disk, so a fresh process moves on to the next graphics mode.
            if !cli.smoke_test && gpu::should_relaunch_after_error(decision, os) {
                relaunch();
                return ExitCode::SUCCESS;
            }
            gpu::show_startup_error(&err.to_string());
            ExitCode::FAILURE
        }
    }
}

/// Start a fresh copy of OpenDrape. `--gpu=` is dropped so the saved state decides.
fn relaunch() {
    if let Ok(exe) = std::env::current_exe() {
        let args = std::env::args()
            .skip(1)
            .filter(|a| !a.starts_with("--gpu="));
        let _ = std::process::Command::new(exe).args(args).spawn();
    }
}
