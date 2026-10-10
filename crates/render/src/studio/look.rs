//! How the studio looks: its colours and sizes, in one place.

/// The dress form: matte linen beige, as sRGB.
pub const FORM_SRGB: [u8; 3] = [188, 168, 153];
/// The dress form's tape lines: a dark brown tape, as sRGB.
pub const TAPE_SRGB: [u8; 3] = [72, 58, 50];
/// The dress form's stand (neck cap, pole and base): charcoal, as sRGB.
pub const STAND_SRGB: [u8; 3] = [64, 64, 68];

/// The backdrop: light, airy grey at the horizon, a little darker above (sRGB, as displayed).
pub(crate) const HORIZON_SRGB: [u8; 3] = [218, 218, 220];
pub(crate) const TOP_SRGB: [u8; 3] = [198, 199, 203];

/// How the studio balances its key light against its soft fill: a softbox's even light, or
/// a stronger key and less fill so shapes and folds read more clearly. Each is calibrated so a
/// fabric facing the camera still shows its own colour.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Lighting {
    Soft,
    Balanced,
    #[default]
    Sculpted,
}

/// What a lighting choice does: the key, fill and rim light scaled, and how dark the key's
/// shadow makes the floor.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LightScale {
    pub key: f32,
    pub fill: f32,
    pub rim: f32,
    pub floor_shadow: f32,
}

impl Lighting {
    pub const ALL: [Self; 3] = [Self::Soft, Self::Balanced, Self::Sculpted];

    pub fn scale(self) -> LightScale {
        match self {
            Self::Soft => LightScale {
                key: 1.0,
                fill: 1.0,
                rim: 1.0,
                floor_shadow: 0.25,
            },
            Self::Balanced => LightScale {
                key: 1.5,
                fill: 0.8,
                rim: 1.15,
                floor_shadow: 0.32,
            },
            Self::Sculpted => LightScale {
                key: 2.4,
                fill: 0.5,
                rim: 1.3,
                floor_shadow: 0.4,
            },
        }
    }
}

/// The rim light: opposite the key (behind, to the left), lower, and softer; it outlines the
/// figure against the backdrop and lights the fabric's sheen at its edges. No shadows.
pub(crate) const RIM_AZIMUTH_DEG: f32 = 220.0;
pub(crate) const RIM_ELEVATION_DEG: f32 = 35.0;
/// Its strength, as a share of the key light's.
pub(crate) const RIM_SHARE: f32 = 0.35;

/// The floor grid: lines every 10 cm and stronger ones every metre (metres), how much each
/// darkens the floor, and how far out (from the centre) it fades away.
pub(crate) const GRID: [f32; 4] = [0.1, 1.0, 0.05, 0.12];
pub(crate) const GRID_FADE: (f32, f32) = (2.5, 5.0);
/// The floor is a disc this big (metres). It shows the backdrop, darkened by shadows, and
/// the shadows fade out between these distances from the centre.
pub(crate) const FLOOR_RADIUS: f32 = 8.0;
pub(crate) const FLOOR_FADE: (f32, f32) = (3.0, 7.0);
