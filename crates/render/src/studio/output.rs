//! The last passes: averaging still frames, and the drawn image (or that average) to the
//! 8-bit sRGB texture egui shows.

use crate::target::COLOR_FORMAT;

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct OutputUniforms {
    params: [f32; 4],
    flags: [u32; 4],
}

/// What the output pass shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Source {
    /// This frame as drawn.
    Scene,
    /// The average of the still frames so far, kept in average texture 0 or 1.
    Average(usize),
}

/// How the output pass shows its source.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Show {
    pub source: Source,
    pub exposure: f32,
    /// The source is already tone-mapped and encoded (the LDR path).
    pub ldr: bool,
    /// Smooth jagged edges.
    pub fxaa: bool,
}

pub(crate) struct OutputPass {
    shader: wgpu::ShaderModule,
    output_pipeline: wgpu::RenderPipeline,
    output_layout: wgpu::BindGroupLayout,
    average_layout: wgpu::BindGroupLayout,
    average_pipeline_layout: wgpu::PipelineLayout,
    /// The averaging pipeline for the scene's colour format.
    average: Option<(wgpu::TextureFormat, wgpu::RenderPipeline)>,
    uniforms: wgpu::Buffer,
    weight: wgpu::Buffer,
    /// What the output pass can read: the scene, average 0, average 1.
    sources: Vec<wgpu::BindGroup>,
    /// Averaging into texture 0 (from 1) and into 1 (from 0), each with the scene.
    averages: Vec<wgpu::BindGroup>,
}

fn layout_entry(binding: u32, ty: wgpu::BindingType) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::FRAGMENT,
        ty,
        count: None,
    }
}

const UNIFORM: wgpu::BindingType = wgpu::BindingType::Buffer {
    ty: wgpu::BufferBindingType::Uniform,
    has_dynamic_offset: false,
    min_binding_size: None,
};

const TEXTURE: wgpu::BindingType = wgpu::BindingType::Texture {
    sample_type: wgpu::TextureSampleType::Float { filterable: false },
    view_dimension: wgpu::TextureViewDimension::D2,
    multisampled: false,
};

fn fullscreen_pipeline(
    device: &wgpu::Device,
    label: &str,
    layout: &wgpu::PipelineLayout,
    shader: &wgpu::ShaderModule,
    fs: &str,
    format: wgpu::TextureFormat,
) -> wgpu::RenderPipeline {
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some(label),
        layout: Some(layout),
        vertex: wgpu::VertexState {
            module: shader,
            entry_point: Some("vs_output"),
            compilation_options: Default::default(),
            buffers: &[],
        },
        fragment: Some(wgpu::FragmentState {
            module: shader,
            entry_point: Some(fs),
            compilation_options: Default::default(),
            targets: &[Some(format.into())],
        }),
        primitive: Default::default(),
        depth_stencil: None,
        multisample: Default::default(),
        multiview_mask: None,
        cache: None,
    })
}

fn draw_fullscreen(
    encoder: &mut wgpu::CommandEncoder,
    label: &str,
    target: &wgpu::TextureView,
    pipeline: &wgpu::RenderPipeline,
    group: &wgpu::BindGroup,
) {
    let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: Some(label),
        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
            view: target,
            depth_slice: None,
            resolve_target: None,
            ops: wgpu::Operations {
                load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                store: wgpu::StoreOp::Store,
            },
        })],
        depth_stencil_attachment: None,
        timestamp_writes: None,
        occlusion_query_set: None,
        multiview_mask: None,
    });
    pass.set_pipeline(pipeline);
    pass.set_bind_group(0, group, &[]);
    pass.draw(0..3, 0..1);
}

impl OutputPass {
    pub fn new(device: &wgpu::Device) -> Self {
        let source = concat!(include_str!("common.wgsl"), include_str!("output.wgsl"));
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("studio output"),
            source: wgpu::ShaderSource::Wgsl(source.into()),
        });
        let output_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("studio output layout"),
            entries: &[layout_entry(0, UNIFORM), layout_entry(1, TEXTURE)],
        });
        let average_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("studio average layout"),
            entries: &[
                layout_entry(0, UNIFORM),
                layout_entry(1, TEXTURE),
                layout_entry(2, TEXTURE),
            ],
        });
        let pipeline_layout = |label, layout: &wgpu::BindGroupLayout| {
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some(label),
                bind_group_layouts: &[Some(layout)],
                immediate_size: 0,
            })
        };
        let output_pipeline_layout =
            pipeline_layout("studio output pipeline layout", &output_layout);
        let average_pipeline_layout =
            pipeline_layout("studio average pipeline layout", &average_layout);
        let output_pipeline = fullscreen_pipeline(
            device,
            "studio output",
            &output_pipeline_layout,
            &shader,
            "fs_output",
            COLOR_FORMAT,
        );
        let buffer = |label, size| {
            device.create_buffer(&wgpu::BufferDescriptor {
                label: Some(label),
                size,
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            })
        };
        Self {
            shader,
            output_pipeline,
            output_layout,
            average_layout,
            average_pipeline_layout,
            average: None,
            uniforms: buffer(
                "studio output uniforms",
                std::mem::size_of::<OutputUniforms>() as u64,
            ),
            weight: buffer("studio average weight", 16),
            sources: Vec::new(),
            averages: Vec::new(),
        }
    }

    /// Reads `scene` and the two average textures (all in `format`) from now on, after the
    /// targets were made anew.
    pub fn bind(
        &mut self,
        device: &wgpu::Device,
        scene: &wgpu::TextureView,
        averages: [&wgpu::TextureView; 2],
        format: wgpu::TextureFormat,
    ) {
        if self.average.as_ref().is_none_or(|(f, _)| *f != format) {
            let pipeline = fullscreen_pipeline(
                device,
                "studio average",
                &self.average_pipeline_layout,
                &self.shader,
                "fs_accumulate",
                format,
            );
            self.average = Some((format, pipeline));
        }
        let texture = |binding, view| wgpu::BindGroupEntry {
            binding,
            resource: wgpu::BindingResource::TextureView(view),
        };
        self.sources = [scene, averages[0], averages[1]]
            .into_iter()
            .map(|view| {
                device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some("studio output"),
                    layout: &self.output_layout,
                    entries: &[
                        wgpu::BindGroupEntry {
                            binding: 0,
                            resource: self.uniforms.as_entire_binding(),
                        },
                        texture(1, view),
                    ],
                })
            })
            .collect();
        self.averages = [averages[1], averages[0]]
            .into_iter()
            .map(|previous| {
                device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some("studio average"),
                    layout: &self.average_layout,
                    entries: &[
                        wgpu::BindGroupEntry {
                            binding: 0,
                            resource: self.weight.as_entire_binding(),
                        },
                        texture(1, previous),
                        texture(2, scene),
                    ],
                })
            })
            .collect();
    }

    /// Adds this frame's scene into average texture `into` (from the other one), weighted
    /// `weight`: 1 / (frames so far + 1).
    pub fn accumulate(
        &self,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        into: usize,
        target: &wgpu::TextureView,
        weight: f32,
    ) {
        queue.write_buffer(
            &self.weight,
            0,
            bytemuck::cast_slice(&[weight, 0.0, 0.0, 0.0]),
        );
        let (_, pipeline) = self.average.as_ref().expect("bound first");
        draw_fullscreen(
            encoder,
            "studio average",
            target,
            pipeline,
            &self.averages[into],
        );
    }

    /// Writes what `show` says to `target`.
    pub fn draw(
        &self,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        target: &wgpu::TextureView,
        Show {
            source,
            exposure,
            ldr,
            fxaa,
        }: Show,
    ) {
        let u = OutputUniforms {
            params: [exposure, 0.0, 0.0, 0.0],
            flags: [u32::from(ldr), u32::from(fxaa), 0, 0],
        };
        queue.write_buffer(&self.uniforms, 0, bytemuck::bytes_of(&u));
        let group = match source {
            Source::Scene => &self.sources[0],
            Source::Average(i) => &self.sources[1 + i],
        };
        draw_fullscreen(
            encoder,
            "studio output",
            target,
            &self.output_pipeline,
            group,
        );
    }
}
