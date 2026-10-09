use super::choice::{GpuChoice, Os};
use serde::{Deserialize, Serialize};
use std::fs::File;
use std::path::{Path, PathBuf};

/// Remembered between launches in `<config dir>/gpu.json`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct GpuState {
    /// The mode that last started successfully, or that the user picked.
    pub preferred: GpuChoice,
    /// Set just before the GPU starts and cleared once frames have been presented.
    /// Still set at the next launch means that launch crashed while starting.
    pub pending: Option<GpuChoice>,
}

impl Default for GpuState {
    fn default() -> Self {
        Self {
            preferred: GpuChoice::Auto,
            pending: None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Reason {
    CommandLine,
    Saved,
    RecoveredFromCrash(GpuChoice),
    /// Every mode in the fallback chain failed (the last one named): start over, and tell the user.
    AllModesFailed(GpuChoice),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Decision {
    pub choice: GpuChoice,
    pub reason: Reason,
}

/// Upper bound on automatic relaunches in a row, whatever the saved state says.
pub const MAX_RELAUNCHES: u32 = 4;

pub fn decide(cli: Option<GpuChoice>, state: &GpuState, os: Os) -> Decision {
    if let Some(choice) = cli {
        return Decision {
            choice,
            reason: Reason::CommandLine,
        };
    }
    if let Some(failed) = state.pending {
        return match failed.next_fallback(os) {
            Some(choice) => Decision {
                choice,
                reason: Reason::RecoveredFromCrash(failed),
            },
            None => Decision {
                choice: GpuChoice::Auto,
                reason: Reason::AllModesFailed(failed),
            },
        };
    }
    Decision {
        choice: state.preferred,
        reason: Reason::Saved,
    }
}

/// The saved state as this process should see it. While another copy of OpenDrape is
/// running, its `pending` marker means "starting up", not "crashed".
pub fn effective_state(previous: GpuState, another_instance_running: bool) -> GpuState {
    if another_instance_running {
        GpuState {
            pending: None,
            ..previous
        }
    } else {
        previous
    }
}

/// State to write before starting the GPU. `--gpu=` overrides are one-offs and leave the file alone.
pub fn pending_marker(decision: Decision, previous: &GpuState) -> Option<GpuState> {
    (decision.reason != Reason::CommandLine).then_some(GpuState {
        preferred: previous.preferred,
        pending: Some(decision.choice),
    })
}

/// State to write once the first frames have been presented.
pub fn confirmed_state(decision: Decision) -> Option<GpuState> {
    (decision.reason != Reason::CommandLine).then_some(GpuState {
        preferred: decision.choice,
        pending: None,
    })
}

/// After a start-up error that did not crash the process: try the next mode in a fresh process?
/// Only if the crash marker reached the disk (otherwise the new process would make the same
/// choice forever) and fewer than [`MAX_RELAUNCHES`] relaunches happened in a row.
pub fn should_relaunch_after_error(
    decision: Decision,
    os: Os,
    marker_saved: bool,
    relaunches: u32,
) -> bool {
    decision.reason != Reason::CommandLine
        && marker_saved
        && relaunches < MAX_RELAUNCHES
        && decision.choice.next_fallback(os).is_some()
}

/// Result of trying to become the only running copy of OpenDrape.
#[derive(Debug)]
pub enum LockOutcome {
    /// Keep the file open for the life of the process; the OS releases the lock on exit or crash.
    Acquired(File),
    HeldElsewhere,
    /// No config folder, or the lock file could not be opened.
    Unavailable,
}

/// Reads and writes `gpu.json`. I/O failures are ignored on purpose: on a locked-down
/// lab PC the app must still start, it just cannot remember anything.
#[derive(Clone, Debug)]
pub struct StateStore {
    path: Option<PathBuf>,
}

impl StateStore {
    pub fn new(dir: Option<&Path>) -> Self {
        Self {
            path: dir.map(|d| d.join("gpu.json")),
        }
    }

    /// The per-machine config folder (`%LOCALAPPDATA%` on Windows, not the roaming profile:
    /// a fallback forced by one lab PC's driver must not follow the student to the next PC).
    pub fn default_location() -> Self {
        let dirs = directories::ProjectDirs::from("org", "OpenDrape", "OpenDrape");
        Self::new(dirs.as_ref().map(|d| d.config_local_dir()))
    }

    pub fn path(&self) -> Option<&Path> {
        self.path.as_deref()
    }

    pub fn load(&self) -> GpuState {
        self.path
            .as_ref()
            .and_then(|p| std::fs::read(p).ok())
            .and_then(|bytes| serde_json::from_slice(&bytes).ok())
            .unwrap_or_default()
    }

    /// Atomic: write a temp file, then rename, so a power cut never leaves half a file.
    /// Returns whether the state reached the disk.
    pub fn save(&self, state: &GpuState) -> bool {
        let Some(path) = &self.path else { return false };
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let Ok(json) = serde_json::to_vec_pretty(state) else {
            return false;
        };
        let tmp = path.with_extension("json.tmp");
        std::fs::write(&tmp, json).is_ok() && std::fs::rename(&tmp, path).is_ok()
    }

    /// Take the `instance.lock` file next to `gpu.json`.
    pub fn acquire_instance_lock(&self) -> LockOutcome {
        let Some(path) = &self.path else {
            return LockOutcome::Unavailable;
        };
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let Ok(file) = File::options()
            .create(true)
            .truncate(false)
            .write(true)
            .open(path.with_file_name("instance.lock"))
        else {
            return LockOutcome::Unavailable;
        };
        match file.try_lock() {
            Ok(()) => LockOutcome::Acquired(file),
            Err(std::fs::TryLockError::WouldBlock) => LockOutcome::HeldElsewhere,
            Err(std::fs::TryLockError::Error(_)) => LockOutcome::Unavailable,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use GpuChoice::*;

    const SAVED: Decision = Decision {
        choice: Auto,
        reason: Reason::Saved,
    };

    #[test]
    fn command_line_wins_over_everything() {
        let state = GpuState {
            preferred: Dx12,
            pending: Some(Dx12),
        };
        assert_eq!(
            decide(Some(Gl), &state, Os::Windows),
            Decision {
                choice: Gl,
                reason: Reason::CommandLine
            }
        );
    }

    #[test]
    fn a_crashed_start_moves_to_the_next_fallback() {
        let state = GpuState {
            preferred: Auto,
            pending: Some(Auto),
        };
        assert_eq!(
            decide(None, &state, Os::Windows),
            Decision {
                choice: Vulkan,
                reason: Reason::RecoveredFromCrash(Auto)
            }
        );
    }

    #[test]
    fn after_every_mode_failed_start_over_from_the_beginning() {
        // Otherwise the last resort would be retried (and crash) on every launch, silently.
        let state = GpuState {
            preferred: Auto,
            pending: Some(Software),
        };
        assert_eq!(
            decide(None, &state, Os::Windows),
            Decision {
                choice: Auto,
                reason: Reason::AllModesFailed(Software)
            }
        );
        let mac = GpuState {
            preferred: Auto,
            pending: Some(Auto),
        };
        assert_eq!(
            decide(None, &mac, Os::MacOs),
            Decision {
                choice: Auto,
                reason: Reason::AllModesFailed(Auto)
            }
        );
    }

    #[test]
    fn a_clean_start_uses_the_saved_preference() {
        let state = GpuState {
            preferred: Vulkan,
            pending: None,
        };
        assert_eq!(
            decide(None, &state, Os::Linux),
            Decision {
                choice: Vulkan,
                reason: Reason::Saved
            }
        );
    }

    #[test]
    fn markers_keep_the_old_preference_until_confirmed() {
        let previous = GpuState {
            preferred: Dx12,
            pending: None,
        };
        let d = Decision {
            choice: Gl,
            reason: Reason::RecoveredFromCrash(Dx12),
        };
        assert_eq!(
            pending_marker(d, &previous),
            Some(GpuState {
                preferred: Dx12,
                pending: Some(Gl)
            })
        );
        assert_eq!(
            confirmed_state(d),
            Some(GpuState {
                preferred: Gl,
                pending: None
            })
        );
    }

    #[test]
    fn command_line_overrides_never_touch_saved_state() {
        let d = Decision {
            choice: Gl,
            reason: Reason::CommandLine,
        };
        assert_eq!(pending_marker(d, &GpuState::default()), None);
        assert_eq!(confirmed_state(d), None);
    }

    #[test]
    fn relaunch_only_when_a_fallback_exists_and_not_for_overrides() {
        assert!(should_relaunch_after_error(SAVED, Os::Windows, true, 0));
        assert!(!should_relaunch_after_error(SAVED, Os::MacOs, true, 0));
        let last = Decision {
            choice: Software,
            reason: Reason::RecoveredFromCrash(Gl),
        };
        assert!(!should_relaunch_after_error(last, Os::Windows, true, 0));
        let cli = Decision {
            choice: Auto,
            reason: Reason::CommandLine,
        };
        assert!(!should_relaunch_after_error(cli, Os::Windows, true, 0));
    }

    #[test]
    fn never_relaunch_when_the_crash_marker_could_not_be_saved() {
        // Unwritable config folder: every copy would read the same state and relaunch forever.
        let store = StateStore::new(None);
        let saved = store.save(&GpuState {
            preferred: Auto,
            pending: Some(Auto),
        });
        assert!(!should_relaunch_after_error(SAVED, Os::Windows, saved, 0));
    }

    #[test]
    fn relaunches_are_capped() {
        assert!(should_relaunch_after_error(
            SAVED,
            Os::Windows,
            true,
            MAX_RELAUNCHES - 1
        ));
        assert!(!should_relaunch_after_error(
            SAVED,
            Os::Windows,
            true,
            MAX_RELAUNCHES
        ));
    }

    #[test]
    fn a_second_running_copy_is_not_mistaken_for_a_crash() {
        let previous = GpuState {
            preferred: Auto,
            pending: Some(Auto),
        };
        assert_eq!(
            effective_state(previous, true),
            GpuState {
                preferred: Auto,
                pending: None
            }
        );
        assert_eq!(effective_state(previous, false), previous);
    }

    #[test]
    fn only_one_process_holds_the_instance_lock() {
        let dir = tempfile::tempdir().unwrap();
        let store = StateStore::new(Some(dir.path()));
        let first = store.acquire_instance_lock();
        assert!(matches!(first, LockOutcome::Acquired(_)));
        assert!(matches!(
            store.acquire_instance_lock(),
            LockOutcome::HeldElsewhere
        ));
        drop(first);
        assert!(matches!(
            store.acquire_instance_lock(),
            LockOutcome::Acquired(_)
        ));
        assert!(matches!(
            StateStore::new(None).acquire_instance_lock(),
            LockOutcome::Unavailable
        ));
    }

    #[cfg(windows)]
    #[test]
    fn graphics_state_stays_on_this_pc_not_the_roaming_profile() {
        let store = StateStore::default_location();
        let path = store.path().expect("a config folder exists on Windows");
        assert!(
            path.to_string_lossy().contains(r"AppData\Local"),
            "{path:?}"
        );
    }

    #[test]
    fn store_round_trips() {
        let dir = tempfile::tempdir().unwrap();
        let store = StateStore::new(Some(dir.path()));
        let state = GpuState {
            preferred: Gl,
            pending: Some(Software),
        };
        assert!(store.save(&state));
        assert_eq!(store.load(), state);
        let json = std::fs::read_to_string(dir.path().join("gpu.json")).unwrap();
        assert!(
            json.contains("\"software\""),
            "human-readable lowercase names: {json}"
        );
    }

    #[test]
    fn missing_or_corrupt_file_gives_defaults() {
        let dir = tempfile::tempdir().unwrap();
        let store = StateStore::new(Some(dir.path()));
        assert_eq!(store.load(), GpuState::default());
        std::fs::write(
            dir.path().join("gpu.json"),
            b"{\"preferred\": \"gl\", \"pend",
        )
        .unwrap();
        assert_eq!(store.load(), GpuState::default());
    }

    #[test]
    fn unwritable_or_absent_location_never_panics() {
        let dir = tempfile::tempdir().unwrap();
        let blocker = dir.path().join("blocker");
        std::fs::write(&blocker, b"a file where a directory should be").unwrap();
        let store = StateStore::new(Some(&blocker.join("sub")));
        assert!(!store.save(&GpuState::default()));
        assert_eq!(store.load(), GpuState::default());

        let nowhere = StateStore::new(None);
        assert!(!nowhere.save(&GpuState::default()));
        assert_eq!(nowhere.load(), GpuState::default());
    }
}
