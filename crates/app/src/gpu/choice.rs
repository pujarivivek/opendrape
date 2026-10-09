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
        // Windows: Auto prefers DX12 (then Vulkan) on a hardware GPU, so its fallback is Vulkan.
        let chain: &[GpuChoice] = match os {
            Os::Windows => &[Self::Auto, Self::Vulkan, Self::Gl, Self::Software],
            Os::Linux => &[Self::Auto, Self::Gl, Self::Software],
            Os::MacOs => &[Self::Auto],
        };
        match chain.iter().position(|c| *c == self) {
            Some(i) => chain.get(i + 1).copied(),
            None => chain.get(1).copied(),
        }
    }

    /// The wgpu backends to load. Software mode loads only the backend that provides the
    /// CPU rasteriser, so it never touches the GPU driver that crashed.
    pub fn backends(self, os: Os) -> wgpu::Backends {
        use wgpu::Backends as B;
        match (self, os) {
            (Self::Auto, Os::Windows) => B::DX12 | B::VULKAN,
            (Self::Auto, Os::MacOs) => B::METAL,
            (Self::Auto, Os::Linux) => B::VULKAN,
            (Self::Dx12, _) => B::DX12,
            (Self::Vulkan, _) => B::VULKAN,
            (Self::Metal, _) => B::METAL,
            (Self::Gl, _) => B::GL,
            (Self::Software, Os::Windows) => B::DX12, // WARP
            (Self::Software, Os::MacOs) => B::METAL,  // no CPU adapter exists: fails cleanly
            (Self::Software, Os::Linux) => B::VULKAN | B::GL, // lavapipe / llvmpipe
        }
    }
}

/// Index of the adapter to use among the (device type, backend) pairs wgpu enumerated.
/// `software` picks a CPU adapter. Otherwise the most capable hardware GPU, preferring
/// DX12/Metal over Vulkan over GL for the same GPU; never a CPU adapter, so a hardware
/// mode fails (and the fallback chain moves on) instead of silently rendering in software.
pub fn pick_adapter(
    adapters: &[(wgpu::DeviceType, wgpu::Backend)],
    software: bool,
) -> Option<usize> {
    use wgpu::{Backend as B, DeviceType as T};
    if software {
        return adapters.iter().position(|(t, _)| *t == T::Cpu);
    }
    let rank = |&(t, b): &(T, B)| {
        let device = match t {
            T::DiscreteGpu => 0,
            T::IntegratedGpu => 1,
            T::VirtualGpu => 2,
            T::Other | T::Cpu => 3,
        };
        let backend = match b {
            B::Dx12 | B::Metal => 0,
            B::Vulkan => 1,
            _ => 2,
        };
        (device, backend)
    };
    (0..adapters.len())
        .filter(|&i| adapters[i].0 != T::Cpu)
        .min_by_key(|&i| rank(&adapters[i]))
}

#[cfg(test)]
mod tests {
    use super::*;
    use wgpu::Backend as B;
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
    fn windows_falls_back_auto_vulkan_gl_software_then_stops() {
        let os = Os::Windows;
        assert_eq!(GpuChoice::Auto.next_fallback(os), Some(GpuChoice::Vulkan));
        assert_eq!(GpuChoice::Vulkan.next_fallback(os), Some(GpuChoice::Gl));
        assert_eq!(GpuChoice::Gl.next_fallback(os), Some(GpuChoice::Software));
        assert_eq!(GpuChoice::Software.next_fallback(os), None);
        // An explicitly chosen backend that fails continues after Auto.
        assert_eq!(GpuChoice::Dx12.next_fallback(os), Some(GpuChoice::Vulkan));
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
    fn hardware_modes_prefer_discrete_then_integrated_gpus() {
        let adapters = [(T::IntegratedGpu, B::Dx12), (T::DiscreteGpu, B::Dx12)];
        assert_eq!(pick_adapter(&adapters, false), Some(1));
    }

    #[test]
    fn hardware_modes_never_settle_for_a_cpu_adapter() {
        // Haswell / Ivy Bridge laptops get no hardware DX12 adapter from wgpu, but WARP is
        // always listed. Auto must fail so the fallback chain reaches hardware OpenGL.
        assert_eq!(pick_adapter(&[(T::Cpu, B::Dx12)], false), None);
        assert_eq!(
            pick_adapter(&[(T::Cpu, B::Dx12), (T::IntegratedGpu, B::Gl)], false),
            Some(1)
        );
        assert_eq!(pick_adapter(&[], false), None);
    }

    #[test]
    fn prefers_dx12_over_vulkan_for_the_same_gpu() {
        // wgpu lists Vulkan adapters before DX12 ones.
        let same_gpu = [(T::IntegratedGpu, B::Vulkan), (T::IntegratedGpu, B::Dx12)];
        assert_eq!(pick_adapter(&same_gpu, false), Some(1));
    }

    #[test]
    fn software_mode_only_picks_cpu_adapters() {
        assert_eq!(
            pick_adapter(&[(T::DiscreteGpu, B::Dx12), (T::Cpu, B::Dx12)], true),
            Some(1)
        );
        assert_eq!(pick_adapter(&[(T::DiscreteGpu, B::Dx12)], true), None);
    }

    #[test]
    fn software_mode_does_not_load_the_drivers_that_crashed() {
        // Safe mode must not start the Vulkan loader or create an OpenGL context on Windows.
        assert_eq!(
            GpuChoice::Software.backends(Os::Windows),
            wgpu::Backends::DX12
        );
        assert!(
            !GpuChoice::Software
                .backends(Os::Linux)
                .contains(wgpu::Backends::DX12)
        );
    }

    #[test]
    fn auto_excludes_opengl_so_it_stays_a_separate_fallback() {
        for os in [Os::Windows, Os::MacOs, Os::Linux] {
            assert!(
                !GpuChoice::Auto.backends(os).contains(wgpu::Backends::GL),
                "{os:?}"
            );
        }
        assert_eq!(GpuChoice::Gl.backends(Os::Windows), wgpu::Backends::GL);
    }
}
