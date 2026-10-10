//! The per-frame data every studio shader reads (group 0). Matrices and vec4s only, so the
//! Rust layout matches WGSL's uniform layout exactly.

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct FrameUniforms {
    pub view_proj: [[f32; 4]; 4],
    pub inv_view_proj: [[f32; 4]; 4],
    pub view: [[f32; 4]; 4],
    pub proj: [[f32; 4]; 4],
    pub inv_proj: [[f32; 4]; 4],
    pub key_view_proj: [[f32; 4]; 4],
    pub contact_view_proj: [[f32; 4]; 4],
    pub camera_pos: [f32; 4],
    pub key_dir: [f32; 4],
    pub key_colour: [f32; 4],
    pub sh: [[f32; 4]; 9],
    /// The backdrop at the horizon and above, already divided by exposure (with the 4 % PBR
    /// Neutral takes off added back), so they display as `look` says.
    pub horizon: [f32; 4],
    pub top: [f32; 4],
    /// x exposure, y frame index (noise rotation), z and w the floor's fade start and end.
    pub params: [f32; 4],
    /// x soft darkening (AO) on, y key shadows on, z floor contact shadow on, w LDR path.
    pub flags: [u32; 4],
    /// Width, height, 1/width, 1/height of the image drawn.
    pub screen: [f32; 4],
    /// x shadow taps, y AO samples, z shadow-map texel size (world metres), w contact opacity.
    pub extra: [f32; 4],
    /// The key shadow map's square (x, metres) and depth range (y, metres).
    pub key_box: [f32; 4],
    /// Unit vector towards the rim light, and its colour (irradiance/π, 0 when off).
    pub rim_dir: [f32; 4],
    pub rim_colour: [f32; 4],
    /// The floor grid: x and y line spacings (metres), z and w how much they darken; and
    /// where it fades out (x start, y end, metres from the centre).
    pub grid: [f32; 4],
    pub grid_fade: [f32; 4],
}

#[cfg(test)]
mod tests {
    use super::*;

    /// WGSL lays the struct out with 16-byte alignment; this must be a whole number of vec4s.
    #[test]
    fn size_is_whole_vec4s() {
        assert_eq!(std::mem::size_of::<FrameUniforms>() % 16, 0);
        assert_eq!(
            std::mem::size_of::<FrameUniforms>(),
            7 * 64 + 3 * 16 + 9 * 16 + 11 * 16
        );
    }
}
