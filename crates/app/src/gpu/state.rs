use super::choice::{GpuChoice, Os};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Remembered between launches in `<config dir>/gpu.json`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct GpuState {
    /// The mode that last started successfully, or that the user picked.
    pub preferred: GpuChoice,
    /// Set just before the GPU starts and cleared once a frame has been drawn.
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
    NoMoreFallbacks,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Decision {
    pub choice: GpuChoice,
    pub reason: Reason,
}

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
                choice: failed,
                reason: Reason::NoMoreFallbacks,
            },
        };
    }
    Decision {
        choice: state.preferred,
        reason: Reason::Saved,
    }
}

/// State to write before starting the GPU. `--gpu=` overrides are one-offs and leave the file alone.
pub fn pending_marker(decision: Decision, previous: &GpuState) -> Option<GpuState> {
    (decision.reason != Reason::CommandLine).then_some(GpuState {
        preferred: previous.preferred,
        pending: Some(decision.choice),
    })
}

/// State to write once the first frame has been drawn.
pub fn confirmed_state(decision: Decision) -> Option<GpuState> {
    (decision.reason != Reason::CommandLine).then_some(GpuState {
        preferred: decision.choice,
        pending: None,
    })
}

/// After a start-up error that did not crash the process: try the next mode in a fresh process?
pub fn should_relaunch_after_error(decision: Decision, os: Os) -> bool {
    decision.reason != Reason::CommandLine && decision.choice.next_fallback(os).is_some()
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

    pub fn default_location() -> Self {
        let dirs = directories::ProjectDirs::from("org", "OpenDrape", "OpenDrape");
        Self::new(dirs.as_ref().map(|d| d.config_dir()))
    }

    pub fn load(&self) -> GpuState {
        self.path
            .as_ref()
            .and_then(|p| std::fs::read(p).ok())
            .and_then(|bytes| serde_json::from_slice(&bytes).ok())
            .unwrap_or_default()
    }

    /// Atomic: write a temp file, then rename, so a power cut never leaves half a file.
    pub fn save(&self, state: &GpuState) {
        let Some(path) = &self.path else { return };
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let Ok(json) = serde_json::to_vec_pretty(state) else {
            return;
        };
        let tmp = path.with_extension("json.tmp");
        if std::fs::write(&tmp, json).is_ok() {
            let _ = std::fs::rename(&tmp, path);
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
                choice: Gl,
                reason: Reason::RecoveredFromCrash(Auto)
            }
        );
    }

    #[test]
    fn when_fallbacks_run_out_the_last_choice_is_kept() {
        let state = GpuState {
            preferred: Auto,
            pending: Some(Software),
        };
        assert_eq!(
            decide(None, &state, Os::Windows),
            Decision {
                choice: Software,
                reason: Reason::NoMoreFallbacks
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
                reason: Reason::NoMoreFallbacks
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
        assert!(should_relaunch_after_error(SAVED, Os::Windows));
        assert!(!should_relaunch_after_error(SAVED, Os::MacOs));
        let last = Decision {
            choice: Software,
            reason: Reason::NoMoreFallbacks,
        };
        assert!(!should_relaunch_after_error(last, Os::Windows));
        let cli = Decision {
            choice: Auto,
            reason: Reason::CommandLine,
        };
        assert!(!should_relaunch_after_error(cli, Os::Windows));
    }

    #[test]
    fn store_round_trips() {
        let dir = tempfile::tempdir().unwrap();
        let store = StateStore::new(Some(dir.path()));
        let state = GpuState {
            preferred: Gl,
            pending: Some(Software),
        };
        store.save(&state);
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
        store.save(&GpuState::default());
        assert_eq!(store.load(), GpuState::default());

        let nowhere = StateStore::new(None);
        nowhere.save(&GpuState::default());
        assert_eq!(nowhere.load(), GpuState::default());
    }
}
