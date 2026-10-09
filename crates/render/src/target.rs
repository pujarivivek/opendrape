/// Physical-pixel size of the render target for a viewport of `width`×`height`
/// points, or `None` when it is too small to draw (minimised window, collapsed panel).
/// Scales down, keeping the aspect ratio, to fit the GPU's maximum texture size.
pub fn target_size(
    width: f32,
    height: f32,
    pixels_per_point: f32,
    max_dim: u32,
) -> Option<(u32, u32)> {
    let w = (width * pixels_per_point).round();
    let h = (height * pixels_per_point).round();
    // Written this way so NaN fails too.
    if !(w >= 1.0 && h >= 1.0) {
        return None;
    }
    let scale = (max_dim as f32 / w.max(h)).min(1.0);
    Some((
        ((w * scale).floor() as u32).max(1),
        ((h * scale).floor() as u32).max(1),
    ))
}

/// egui only displays native textures in this format, so shaders encode sRGB themselves.
pub const COLOR_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;
pub const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;
/// Viewport background (light grey), RGBA 0..1.
pub const CLEAR_COLOR: [f64; 4] = [0.93, 0.93, 0.95, 1.0];

/// Colour + depth textures for one viewport.
pub struct RenderTarget {
    pub width: u32,
    pub height: u32,
    pub color: wgpu::Texture,
    pub color_view: wgpu::TextureView,
    pub(crate) depth_view: wgpu::TextureView,
}

impl RenderTarget {
    pub fn new(device: &wgpu::Device, width: u32, height: u32) -> Self {
        let size = wgpu::Extent3d {
            width: width.max(1),
            height: height.max(1),
            depth_or_array_layers: 1,
        };
        let color = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("viewport color"),
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: COLOR_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                | wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let depth = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("viewport depth"),
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: DEPTH_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });
        let color_view = color.create_view(&wgpu::TextureViewDescriptor::default());
        let depth_view = depth.create_view(&wgpu::TextureViewDescriptor::default());
        Self {
            width: size.width,
            height: size.height,
            color,
            color_view,
            depth_view,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zero_negative_or_nan_sizes_are_skipped() {
        assert_eq!(target_size(0.0, 300.0, 2.0, 8192), None);
        assert_eq!(target_size(300.0, 0.2, 2.0, 8192), None);
        assert_eq!(target_size(-5.0, 10.0, 1.0, 8192), None);
        assert_eq!(target_size(f32::NAN, 10.0, 1.0, 8192), None);
    }

    #[test]
    fn hidpi_uses_physical_pixels() {
        assert_eq!(target_size(400.0, 300.0, 2.0, 8192), Some((800, 600)));
        assert_eq!(target_size(401.0, 301.0, 1.25, 8192), Some((501, 376)));
    }

    #[test]
    fn oversized_viewport_is_scaled_to_fit_gpu_limit() {
        // A 5K display at 2x on an OpenGL adapter limited to 2048 px textures.
        assert_eq!(target_size(2560.0, 1440.0, 2.0, 2048), Some((2048, 1152)));
    }
}
