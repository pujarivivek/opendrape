//! How the studio looks: its colours and sizes, in one place.

/// The dress form: matte linen beige, as sRGB.
pub const FORM_SRGB: [u8; 3] = [188, 168, 153];

/// The backdrop: light grey at the horizon, a little darker above (sRGB, as displayed).
pub(crate) const HORIZON_SRGB: [u8; 3] = [206, 206, 208];
pub(crate) const TOP_SRGB: [u8; 3] = [186, 187, 191];
/// The floor is a disc this big (metres). It shows the backdrop, darkened by shadows, and
/// the shadows fade out between these distances from the centre.
pub(crate) const FLOOR_RADIUS: f32 = 8.0;
pub(crate) const FLOOR_FADE: (f32, f32) = (3.0, 7.0);
