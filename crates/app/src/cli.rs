use crate::gpu::GpuChoice;

/// Command-line options. Unknown arguments are ignored: macOS may pass `-psn_…`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Cli {
    /// `--gpu=<auto|dx12|vulkan|metal|gl|safe>`: one-off graphics override.
    pub gpu: Option<GpuChoice>,
    /// `--smoke-test`: quit with success as soon as the first 3D frame is drawn.
    pub smoke_test: bool,
}

impl Cli {
    pub fn parse<I: IntoIterator<Item = String>>(args: I) -> Self {
        let mut cli = Self::default();
        for arg in args {
            if let Some(value) = arg.strip_prefix("--gpu=") {
                cli.gpu = GpuChoice::parse(value);
                if cli.gpu.is_none() {
                    eprintln!("OpenDrape: ignoring unknown --gpu value {value:?}");
                }
            } else if arg == "--smoke-test" {
                cli.smoke_test = true;
            }
        }
        cli
    }

    /// Whether someone may be there to answer a dialog. Not in `--smoke-test` runs: on a CI
    /// machine a dialog waits for a click forever.
    pub fn interactive(&self) -> bool {
        !self.smoke_test
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(args: &[&str]) -> Cli {
        Cli::parse(args.iter().map(|s| s.to_string()))
    }

    #[test]
    fn reads_gpu_and_smoke_test_flags() {
        assert_eq!(
            parse(&["--gpu=gl", "--smoke-test"]),
            Cli {
                gpu: Some(GpuChoice::Gl),
                smoke_test: true
            }
        );
        assert_eq!(parse(&["--gpu=safe"]).gpu, Some(GpuChoice::Software));
    }

    #[test]
    fn a_smoke_test_never_waits_for_someone_to_click_a_dialog() {
        // On a CI machine nobody is there to press OK: the run would hang until it timed out.
        assert!(!parse(&["--smoke-test"]).interactive());
        assert!(parse(&["--gpu=gl"]).interactive());
    }

    #[test]
    fn unknown_values_and_arguments_are_ignored() {
        assert_eq!(parse(&["--gpu=banana"]), Cli::default());
        assert_eq!(
            parse(&["-psn_0_12345", "--verbose", "file.odp"]),
            Cli::default()
        );
        assert_eq!(parse(&[]), Cli::default());
    }
}
