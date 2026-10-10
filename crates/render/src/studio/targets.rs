//! The textures the studio draws into before the final image: recreated when the image size
//! or the colour format changes.

/// The depth format of every studio pass.
pub(crate) const DEPTH: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;

pub(crate) struct Targets {
    pub width: u32,
    pub height: u32,
    pub format: wgpu::TextureFormat,
    /// The lit scene: half-float HDR, or 8-bit already tone-mapped (the LDR path).
    pub colour_view: wgpu::TextureView,
    pub depth_view: wgpu::TextureView,
    /// The AO prepass: its depth buffer, view normals, and distance from the camera (read by
    /// the AO search and the main pass).
    pub prepass_depth: wgpu::TextureView,
    pub normals: wgpu::TextureView,
    pub distance: wgpu::TextureView,
    /// The running average of still frames, kept in turn in one and then the other.
    pub averages: [wgpu::TextureView; 2],
}

pub(crate) fn texture(
    device: &wgpu::Device,
    label: &str,
    (width, height): (u32, u32),
    format: wgpu::TextureFormat,
    usage: wgpu::TextureUsages,
) -> wgpu::Texture {
    device.create_texture(&wgpu::TextureDescriptor {
        label: Some(label),
        size: wgpu::Extent3d {
            width: width.max(1),
            height: height.max(1),
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage,
        view_formats: &[],
    })
}

impl Targets {
    pub fn new(
        device: &wgpu::Device,
        width: u32,
        height: u32,
        format: wgpu::TextureFormat,
    ) -> Self {
        use wgpu::TextureUsages as U;
        let size = (width, height);
        let view = |t: wgpu::Texture| t.create_view(&wgpu::TextureViewDescriptor::default());
        Self {
            width,
            height,
            format,
            colour_view: view(texture(
                device,
                "studio colour",
                size,
                format,
                U::RENDER_ATTACHMENT | U::TEXTURE_BINDING,
            )),
            depth_view: view(texture(
                device,
                "studio depth",
                size,
                DEPTH,
                U::RENDER_ATTACHMENT | U::TEXTURE_BINDING,
            )),
            prepass_depth: view(texture(
                device,
                "studio prepass depth",
                size,
                DEPTH,
                U::RENDER_ATTACHMENT | U::TEXTURE_BINDING,
            )),
            normals: view(texture(
                device,
                "studio normals",
                size,
                super::ao::NORMALS,
                U::RENDER_ATTACHMENT | U::TEXTURE_BINDING,
            )),
            distance: view(texture(
                device,
                "studio distance",
                size,
                super::ao::DISTANCE,
                U::RENDER_ATTACHMENT | U::TEXTURE_BINDING,
            )),
            averages: [0, 1].map(|_| {
                view(texture(
                    device,
                    "studio average",
                    size,
                    format,
                    U::RENDER_ATTACHMENT | U::TEXTURE_BINDING,
                ))
            }),
        }
    }

    pub fn matches(&self, width: u32, height: u32, format: wgpu::TextureFormat) -> bool {
        self.width == width && self.height == height && self.format == format
    }
}
