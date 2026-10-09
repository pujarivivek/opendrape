use crate::target::RenderTarget;

/// A GPU device with no window, for tests and offscreen export.
pub struct HeadlessGpu {
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub info: wgpu::AdapterInfo,
}

/// Any working adapter: a real GPU if there is one, otherwise a software one
/// (WARP on Windows, lavapipe/llvmpipe on Linux). `None` if nothing works.
pub fn headless_device() -> Option<HeadlessGpu> {
    headless_device_with(wgpu::Backends::all())
}

/// Like [`headless_device`], restricted to `backends` (e.g. only OpenGL, to test that fallback).
pub fn headless_device_with(backends: wgpu::Backends) -> Option<HeadlessGpu> {
    pollster::block_on(async {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends,
            ..wgpu::InstanceDescriptor::new_without_display_handle()
        });
        let mut adapter = None;
        for force_fallback_adapter in [false, true] {
            adapter = instance
                .request_adapter(&wgpu::RequestAdapterOptions {
                    force_fallback_adapter,
                    ..Default::default()
                })
                .await
                .ok();
            if adapter.is_some() {
                break;
            }
        }
        let adapter = adapter?;
        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some("OpenDrape headless"),
                required_limits: wgpu::Limits::downlevel_webgl2_defaults()
                    .using_resolution(adapter.limits()),
                ..Default::default()
            })
            .await
            .ok()?;
        Some(HeadlessGpu {
            device,
            queue,
            info: adapter.get_info(),
        })
    })
}

/// Copy the colour texture of `target` back to the CPU. Blocks until the GPU is done.
pub fn read_back(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    target: &RenderTarget,
) -> image::RgbaImage {
    let unpadded = target.width * 4;
    let align = wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
    let padded = unpadded.div_ceil(align) * align;
    let buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("readback"),
        size: u64::from(padded) * u64::from(target.height),
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("readback"),
    });
    encoder.copy_texture_to_buffer(
        target.color.as_image_copy(),
        wgpu::TexelCopyBufferInfo {
            buffer: &buffer,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(padded),
                rows_per_image: None,
            },
        },
        wgpu::Extent3d {
            width: target.width,
            height: target.height,
            depth_or_array_layers: 1,
        },
    );
    queue.submit([encoder.finish()]);
    buffer.map_async(wgpu::MapMode::Read, .., |result| {
        result.expect("map readback buffer")
    });
    device
        .poll(wgpu::PollType::wait_indefinitely())
        .expect("wait for GPU");
    let data = buffer
        .get_mapped_range(..)
        .expect("readback buffer is mapped");
    let mut pixels = Vec::with_capacity((unpadded * target.height) as usize);
    for row in data.chunks(padded as usize) {
        pixels.extend_from_slice(&row[..unpadded as usize]);
    }
    image::RgbaImage::from_raw(target.width, target.height, pixels)
        .expect("pixel count matches size")
}
