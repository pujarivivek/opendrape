//! How much work the 3D view does per frame: three levels, picked automatically from the
//! graphics chip, each with lighter settings while things move and fuller ones once still.

/// How fully the 3D view is drawn.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Quality {
    /// Software rendering and weak or unknown graphics: studio light, sheen, colour and the
    /// floor shadow while moving; shadows and soft darkening in folds only once still.
    Basic,
    /// Integrated graphics.
    Medium,
    /// Discrete graphics and Apple chips.
    High,
}

/// Ambient occlusion (soft darkening in folds): at half resolution or full, with how many
/// samples per pixel.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AoSettings {
    pub half_res: bool,
    pub samples: u32,
}

/// The key light's shadow map: its size and the samples taken to soften it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ShadowSettings {
    pub size: u32,
    pub taps: u32,
}

/// What a quality level does while things move and once they are still.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Settings {
    /// None: no soft darkening while moving.
    pub ao_moving: Option<AoSettings>,
    pub ao_still: AoSettings,
    /// None: no key-light shadows while moving.
    pub shadow_moving: Option<ShadowSettings>,
    pub shadow_still: ShadowSettings,
    /// Size of the floor's contact-shadow map (square).
    pub contact_size: u32,
    /// Smooth jagged edges while moving.
    pub fxaa_moving: bool,
    /// The most pixels the 3D image is drawn at; it is scaled up to fill the panel.
    pub cap_px: u32,
    /// Frames averaged once still, for smooth edges and shadows.
    pub still_frames: u32,
}

impl Quality {
    pub const ALL: [Self; 3] = [Self::Basic, Self::Medium, Self::High];

    pub fn settings(self) -> Settings {
        let half = |samples| AoSettings {
            half_res: true,
            samples,
        };
        let shadow = |size, taps| ShadowSettings { size, taps };
        match self {
            Self::Basic => Settings {
                ao_moving: None,
                ao_still: half(8),
                shadow_moving: None,
                shadow_still: shadow(1024, 8),
                contact_size: 256,
                fxaa_moving: false,
                cap_px: 1_000_000,
                still_frames: 16,
            },
            Self::Medium => Settings {
                ao_moving: Some(half(8)),
                ao_still: half(8),
                shadow_moving: Some(shadow(1024, 8)),
                shadow_still: shadow(1024, 8),
                contact_size: 512,
                fxaa_moving: true,
                cap_px: 2_000_000,
                still_frames: 16,
            },
            Self::High => Settings {
                ao_moving: Some(half(12)),
                ao_still: AoSettings {
                    half_res: false,
                    samples: 12,
                },
                shadow_moving: Some(shadow(2048, 12)),
                shadow_still: shadow(2048, 12),
                contact_size: 512,
                fxaa_moving: true,
                cap_px: 4_000_000,
                still_frames: 32,
            },
        }
    }
}

/// The level to start at on this graphics chip. `hdr_ok`: it can draw into half-float
/// textures (without them only Basic's simpler path works).
pub fn auto(info: &wgpu::AdapterInfo, hdr_ok: bool) -> Quality {
    use wgpu::DeviceType::*;
    if !hdr_ok {
        return Quality::Basic;
    }
    match info.device_type {
        Cpu | VirtualGpu | Other => Quality::Basic,
        DiscreteGpu => Quality::High,
        IntegratedGpu if info.name.starts_with("Apple") => Quality::High,
        IntegratedGpu => Quality::Medium,
    }
}

/// The size to draw a `w`×`h` image at: as is when it has at most `cap_px` pixels, otherwise
/// scaled down keeping its shape (each side at least 1).
pub fn render_size(w: u32, h: u32, cap_px: u32) -> (u32, u32) {
    let (w, h) = (w.max(1), h.max(1));
    let pixels = u64::from(w) * u64::from(h);
    if pixels <= u64::from(cap_px) {
        return (w, h);
    }
    let scale = (f64::from(cap_px) / pixels as f64).sqrt();
    let fit = |v: u32| ((f64::from(v) * scale).floor() as u32).max(1);
    let (mut sw, mut sh) = (fit(w), fit(h));
    // A very thin image can still be over the cap once its short side is held at 1.
    while u64::from(sw) * u64::from(sh) > u64::from(cap_px) && sw > 1 {
        sw -= 1;
    }
    while u64::from(sw) * u64::from(sh) > u64::from(cap_px) && sh > 1 {
        sh -= 1;
    }
    (sw, sh)
}

#[cfg(test)]
mod tests {
    use super::*;
    use wgpu::{AdapterInfo, Backend, DeviceType};

    fn adapter(name: &str, device_type: DeviceType, backend: Backend) -> AdapterInfo {
        AdapterInfo {
            name: name.into(),
            ..AdapterInfo::new(device_type, backend)
        }
    }

    #[test]
    fn auto_picks_by_graphics_chip() {
        use DeviceType::*;
        let cases = [
            (
                "Microsoft Basic Render Driver",
                Cpu,
                Backend::Dx12,
                true,
                Quality::Basic,
            ),
            (
                "llvmpipe (LLVM 17.0.6, 256 bits)",
                Cpu,
                Backend::Gl,
                true,
                Quality::Basic,
            ),
            (
                "Virtual GPU",
                VirtualGpu,
                Backend::Vulkan,
                true,
                Quality::Basic,
            ),
            (
                "Intel(R) UHD Graphics 620",
                IntegratedGpu,
                Backend::Dx12,
                true,
                Quality::Medium,
            ),
            (
                "Apple M2",
                IntegratedGpu,
                Backend::Metal,
                true,
                Quality::High,
            ),
            (
                "NVIDIA GeForce RTX 3050",
                DiscreteGpu,
                Backend::Vulkan,
                true,
                Quality::High,
            ),
            (
                "Intel(R) HD Graphics 4000",
                IntegratedGpu,
                Backend::Gl,
                false,
                Quality::Basic,
            ),
        ];
        for (name, kind, backend, hdr_ok, want) in cases {
            assert_eq!(auto(&adapter(name, kind, backend), hdr_ok), want, "{name}");
        }
    }

    #[test]
    fn render_size_respects_the_cap() {
        let (w, h) = render_size(1920, 1080, 1_000_000);
        assert!(w * h <= 1_000_000, "{w}×{h}");
        assert!(((w as f32 / h as f32) - 1920.0 / 1080.0).abs() < 0.01 * 1920.0 / 1080.0);
        assert_eq!(render_size(1, 1, 1_000_000), (1, 1));
        assert_eq!(
            render_size(800, 600, 1_000_000),
            (800, 600),
            "under the cap: as is"
        );
        let (w, h) = render_size(5000, 3, 1_000);
        assert!(w >= 1 && h >= 1 && w * h <= 1_000, "{w}×{h}");
    }

    #[test]
    fn settings_follow_the_table() {
        let basic = Quality::Basic.settings();
        assert_eq!(basic.ao_moving, None);
        assert_eq!(basic.shadow_moving, None);
        assert!(!basic.fxaa_moving);
        assert_eq!(
            basic.ao_still,
            AoSettings {
                half_res: true,
                samples: 8
            }
        );
        assert_eq!(
            basic.shadow_still,
            ShadowSettings {
                size: 1024,
                taps: 8
            }
        );
        assert_eq!(
            (basic.contact_size, basic.cap_px, basic.still_frames),
            (256, 1_000_000, 16)
        );

        let medium = Quality::Medium.settings();
        assert_eq!(
            medium.ao_moving,
            Some(AoSettings {
                half_res: true,
                samples: 8
            })
        );
        assert_eq!(
            medium.shadow_moving,
            Some(ShadowSettings {
                size: 1024,
                taps: 8
            })
        );
        assert!(medium.fxaa_moving);
        assert_eq!(
            (medium.contact_size, medium.cap_px, medium.still_frames),
            (512, 2_000_000, 16)
        );

        let high = Quality::High.settings();
        assert_eq!(
            high.ao_moving,
            Some(AoSettings {
                half_res: true,
                samples: 12
            })
        );
        assert_eq!(
            high.ao_still,
            AoSettings {
                half_res: false,
                samples: 12
            }
        );
        assert_eq!(
            high.shadow_moving,
            Some(ShadowSettings {
                size: 2048,
                taps: 12
            })
        );
        assert_eq!(
            (high.contact_size, high.cap_px, high.still_frames),
            (512, 4_000_000, 32)
        );
    }
}
