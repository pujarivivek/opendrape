use crate::gpu::{Decision, Reason};

/// Facts for bug reports. Deliberately English, not translated.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Diagnostics {
    pub app_version: String,
    pub git_sha: String,
    pub os: String,
    pub adapter: String,
    pub backend: String,
    pub device_type: String,
    pub driver: String,
    pub graphics_mode: String,
}

impl Diagnostics {
    pub fn collect(info: Option<&wgpu::AdapterInfo>, decision: Decision) -> Self {
        let mut d = Self {
            app_version: env!("CARGO_PKG_VERSION").to_owned(),
            git_sha: env!("OPENDRAPE_GIT_SHA").to_owned(),
            os: format!("{} {}", std::env::consts::OS, std::env::consts::ARCH),
            graphics_mode: describe(decision),
            ..Default::default()
        };
        if let Some(info) = info {
            d.adapter = info.name.clone();
            d.backend = format!("{:?}", info.backend);
            d.device_type = format!("{:?}", info.device_type);
            d.driver = format!("{} {}", info.driver, info.driver_info)
                .trim()
                .to_owned();
        }
        d
    }

    pub fn to_text(&self) -> String {
        format!(
            "OpenDrape {} ({})\nOS: {}\nGraphics: {} | {} | {}\nDriver: {}\nGraphics mode: {}\n",
            self.app_version,
            self.git_sha,
            self.os,
            self.adapter,
            self.backend,
            self.device_type,
            self.driver,
            self.graphics_mode,
        )
    }
}

fn describe(d: Decision) -> String {
    let why = match d.reason {
        Reason::CommandLine => "set on the command line".to_owned(),
        Reason::Saved => "saved setting".to_owned(),
        Reason::RecoveredFromCrash(failed) => format!("switched after {failed:?} failed to start"),
        Reason::NoMoreFallbacks => "last resort".to_owned(),
    };
    format!("{:?} ({why})", d.choice)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gpu::GpuChoice;

    #[test]
    fn text_contains_every_fact_a_bug_report_needs() {
        let d = Diagnostics {
            app_version: "0.1.0".into(),
            git_sha: "abc1234".into(),
            os: "windows x86_64".into(),
            adapter: "Intel(R) HD Graphics 520".into(),
            backend: "Gl".into(),
            device_type: "IntegratedGpu".into(),
            driver: "Intel 31.0.101".into(),
            graphics_mode: "Gl (switched after Auto failed to start)".into(),
        };
        let text = d.to_text();
        for needle in [
            "0.1.0",
            "abc1234",
            "windows x86_64",
            "HD Graphics 520",
            "Gl",
            "IntegratedGpu",
            "31.0.101",
            "switched after Auto",
        ] {
            assert!(text.contains(needle), "missing {needle:?} in:\n{text}");
        }
    }

    #[test]
    fn collect_without_a_gpu_still_reports_version_and_mode() {
        let d = Diagnostics::collect(
            None,
            Decision {
                choice: GpuChoice::Auto,
                reason: Reason::Saved,
            },
        );
        assert_eq!(d.app_version, env!("CARGO_PKG_VERSION"));
        assert!(!d.git_sha.is_empty());
        assert_eq!(d.graphics_mode, "Auto (saved setting)");
        assert!(d.adapter.is_empty());
    }
}
