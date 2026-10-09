use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Os {
    Windows,
    MacOs,
    Linux,
}

impl Os {
    pub fn current() -> Self {
        if cfg!(target_os = "windows") {
            Os::Windows
        } else if cfg!(target_os = "macos") {
            Os::MacOs
        } else {
            Os::Linux
        }
    }
}

/// Which graphics backend OpenDrape asks wgpu for.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum GpuChoice {
    Auto,
    Dx12,
    Vulkan,
    Metal,
    Gl,
    /// CPU rendering (WARP on Windows, llvmpipe/lavapipe on Linux): slow but works everywhere.
    Software,
}

impl GpuChoice {
    pub fn parse(s: &str) -> Option<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "auto" => Some(Self::Auto),
            "dx12" | "d3d12" | "directx" => Some(Self::Dx12),
            "vulkan" => Some(Self::Vulkan),
            "metal" => Some(Self::Metal),
            "gl" | "opengl" => Some(Self::Gl),
            "software" | "safe" | "warp" | "cpu" => Some(Self::Software),
            _ => None,
        }
    }

    /// Choices offered in Help → Graphics on this OS, safest last.
    pub fn available(os: Os) -> &'static [GpuChoice] {
        match os {
            Os::Windows => &[
                Self::Auto,
                Self::Dx12,
                Self::Vulkan,
                Self::Gl,
                Self::Software,
            ],
            Os::MacOs => &[Self::Auto],
            Os::Linux => &[Self::Auto, Self::Vulkan, Self::Gl, Self::Software],
        }
    }

    /// What to try after this choice failed to start, or `None` when nothing is left.
    pub fn next_fallback(self, os: Os) -> Option<GpuChoice> {
        let chain: &[GpuChoice] = match os {
            Os::Windows | Os::Linux => &[Self::Auto, Self::Gl, Self::Software],
            Os::MacOs => &[Self::Auto],
        };
        match chain.iter().position(|c| *c == self) {
            Some(i) => chain.get(i + 1).copied(),
            None => chain.get(1).copied(),
        }
    }

    pub fn backends(self) -> wgpu::Backends {
        match self {
            Self::Auto => wgpu::Backends::PRIMARY,
            Self::Dx12 => wgpu::Backends::DX12,
            Self::Vulkan => wgpu::Backends::VULKAN,
            Self::Metal => wgpu::Backends::METAL,
            Self::Gl => wgpu::Backends::GL,
            Self::Software => wgpu::Backends::all(),
        }
    }
}

/// Index of the adapter to use, given the device types wgpu enumerated (in order).
/// `software` picks a CPU adapter; otherwise the most capable real GPU, falling back to CPU.
pub fn pick_adapter(types: &[wgpu::DeviceType], software: bool) -> Option<usize> {
    use wgpu::DeviceType as T;
    if software {
        return types.iter().position(|t| *t == T::Cpu);
    }
    let rank = |t: &T| match t {
        T::DiscreteGpu => 0,
        T::IntegratedGpu => 1,
        T::VirtualGpu => 2,
        T::Other => 3,
        T::Cpu => 4,
    };
    (0..types.len()).min_by_key(|&i| rank(&types[i]))
}

#[cfg(test)]
mod tests {
    use super::*;
    use wgpu::DeviceType as T;

    #[test]
    fn parses_names_and_aliases() {
        assert_eq!(GpuChoice::parse("auto"), Some(GpuChoice::Auto));
        assert_eq!(GpuChoice::parse("DX12"), Some(GpuChoice::Dx12));
        assert_eq!(GpuChoice::parse("opengl"), Some(GpuChoice::Gl));
        assert_eq!(GpuChoice::parse(" safe "), Some(GpuChoice::Software));
        assert_eq!(GpuChoice::parse("warp"), Some(GpuChoice::Software));
        assert_eq!(GpuChoice::parse("banana"), None);
        assert_eq!(GpuChoice::parse(""), None);
    }

    #[test]
    fn windows_falls_back_auto_gl_software_then_stops() {
        let os = Os::Windows;
        assert_eq!(GpuChoice::Auto.next_fallback(os), Some(GpuChoice::Gl));
        assert_eq!(GpuChoice::Gl.next_fallback(os), Some(GpuChoice::Software));
        assert_eq!(GpuChoice::Software.next_fallback(os), None);
        // An explicitly chosen backend that fails continues after Auto.
        assert_eq!(GpuChoice::Dx12.next_fallback(os), Some(GpuChoice::Gl));
    }

    #[test]
    fn macos_has_only_metal() {
        assert_eq!(GpuChoice::available(Os::MacOs), &[GpuChoice::Auto]);
        assert_eq!(GpuChoice::Auto.next_fallback(Os::MacOs), None);
        assert_eq!(GpuChoice::Metal.next_fallback(Os::MacOs), None);
    }

    #[test]
    fn every_offered_choice_reaches_the_end_of_its_chain() {
        for os in [Os::Windows, Os::MacOs, Os::Linux] {
            for &start in GpuChoice::available(os) {
                let mut steps = 0;
                let mut c = Some(start);
                while let Some(choice) = c {
                    c = choice.next_fallback(os);
                    steps += 1;
                    assert!(steps < 10, "fallback loop from {start:?} on {os:?}");
                }
            }
        }
    }

    #[test]
    fn auto_prefers_real_gpus_and_accepts_cpu_as_last_resort() {
        assert_eq!(
            pick_adapter(&[T::Cpu, T::IntegratedGpu, T::DiscreteGpu], false),
            Some(2)
        );
        assert_eq!(pick_adapter(&[T::Cpu, T::IntegratedGpu], false), Some(1));
        assert_eq!(pick_adapter(&[T::Cpu], false), Some(0));
        assert_eq!(pick_adapter(&[], false), None);
    }

    #[test]
    fn software_only_picks_cpu_adapters() {
        assert_eq!(pick_adapter(&[T::DiscreteGpu, T::Cpu], true), Some(1));
        assert_eq!(pick_adapter(&[T::DiscreteGpu], true), None);
    }

    #[test]
    fn auto_excludes_opengl_so_it_stays_a_separate_fallback() {
        assert!(!GpuChoice::Auto.backends().contains(wgpu::Backends::GL));
        assert_eq!(GpuChoice::Gl.backends(), wgpu::Backends::GL);
    }
}
