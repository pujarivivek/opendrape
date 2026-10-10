//! Soft darkening in folds (ambient occlusion): a prepass of depth and view-space normals,
//! a hemisphere search at half (or full) resolution, and a depth-aware blur across then down.

use super::mesh::StudioMesh;
use super::targets::{DEPTH, texture};
use crate::mesh::Vertex;

/// The prepass's normals and distance from the camera (24 bits packed in RGB), and the AO
/// result: 8-bit, drawable and readable everywhere, OpenGL included.
pub(crate) const NORMALS: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;
pub(crate) const DISTANCE: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;
const AO: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;

/// The AO textures at one resolution, and the bind groups that read them.
pub(crate) struct AoTarget {
    pub half: bool,
    /// The result (after the blur goes a → b → a).
    pub a: wgpu::TextureView,
    b: wgpu::TextureView,
    input: wgpu::BindGroup,
    across: wgpu::BindGroup,
    down: wgpu::BindGroup,
}

pub(crate) struct AoPass {
    prepass: wgpu::RenderPipeline,
    search: wgpu::RenderPipeline,
    blur: wgpu::RenderPipeline,
    input_layout: wgpu::BindGroupLayout,
    blur_layout: wgpu::BindGroupLayout,
    across: wgpu::Buffer,
    down: wgpu::Buffer,
    half_scale: wgpu::Buffer,
    full_scale: wgpu::Buffer,
}

fn ints(device: &wgpu::Device, label: &str, v: [i32; 4]) -> wgpu::Buffer {
    use wgpu::util::DeviceExt;
    device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some(label),
        contents: bytemuck::cast_slice(&v),
        usage: wgpu::BufferUsages::UNIFORM,
    })
}

fn colour_pass<'a>(
    encoder: &'a mut wgpu::CommandEncoder,
    label: &str,
    view: &'a wgpu::TextureView,
    depth: Option<&'a wgpu::TextureView>,
) -> wgpu::RenderPass<'a> {
    encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: Some(label),
        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
            view,
            depth_slice: None,
            resolve_target: None,
            ops: wgpu::Operations {
                load: wgpu::LoadOp::Clear(wgpu::Color::WHITE),
                store: wgpu::StoreOp::Store,
            },
        })],
        depth_stencil_attachment: depth.map(|view| wgpu::RenderPassDepthStencilAttachment {
            view,
            depth_ops: Some(wgpu::Operations {
                load: wgpu::LoadOp::Clear(1.0),
                store: wgpu::StoreOp::Store,
            }),
            stencil_ops: None,
        }),
        timestamp_writes: None,
        occlusion_query_set: None,
        multiview_mask: None,
    })
}

impl AoPass {
    pub fn new(device: &wgpu::Device, frame_layout: &wgpu::BindGroupLayout) -> Self {
        let source = concat!(include_str!("common.wgsl"), include_str!("ao.wgsl"));
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("studio ao"),
            source: wgpu::ShaderSource::Wgsl(source.into()),
        });
        let texture = |binding, sample_type| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Texture {
                sample_type,
                view_dimension: wgpu::TextureViewDimension::D2,
                multisampled: false,
            },
            count: None,
        };
        let uniform = |binding| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Uniform,
                has_dynamic_offset: false,
                min_binding_size: None,
            },
            count: None,
        };
        let unfiltered = wgpu::TextureSampleType::Float { filterable: false };
        let input_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("studio ao input layout"),
            entries: &[texture(0, unfiltered), texture(1, unfiltered), uniform(2)],
        });
        let blur_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("studio ao blur layout"),
            entries: &[uniform(0), texture(1, unfiltered)],
        });
        let layout = |label, groups: &[Option<&wgpu::BindGroupLayout>]| {
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some(label),
                bind_group_layouts: groups,
                immediate_size: 0,
            })
        };
        let prepass_layout = layout("studio prepass layout", &[Some(frame_layout)]);
        let search_layout = layout(
            "studio ao search layout",
            &[Some(frame_layout), Some(&input_layout)],
        );
        let blur_pipeline_layout = layout(
            "studio ao blur layout",
            &[Some(frame_layout), Some(&blur_layout)],
        );
        let fullscreen = |label, layout: &wgpu::PipelineLayout, fs| {
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some(label),
                layout: Some(layout),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: Some("vs_fullscreen"),
                    compilation_options: Default::default(),
                    buffers: &[],
                },
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: Some(fs),
                    compilation_options: Default::default(),
                    targets: &[Some(AO.into())],
                }),
                primitive: Default::default(),
                depth_stencil: None,
                multisample: Default::default(),
                multiview_mask: None,
                cache: None,
            })
        };
        let prepass = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("studio prepass"),
            layout: Some(&prepass_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_prepass"),
                compilation_options: Default::default(),
                buffers: &[Some(wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<Vertex>() as u64,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x3],
                })],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_prepass"),
                compilation_options: Default::default(),
                targets: &[Some(NORMALS.into()), Some(DISTANCE.into())],
            }),
            primitive: wgpu::PrimitiveState {
                cull_mode: None,
                ..Default::default()
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: DEPTH,
                depth_write_enabled: Some(true),
                depth_compare: Some(wgpu::CompareFunction::Less),
                stencil: Default::default(),
                bias: Default::default(),
            }),
            multisample: Default::default(),
            multiview_mask: None,
            cache: None,
        });
        Self {
            prepass,
            search: fullscreen("studio ao search", &search_layout, "fs_ao"),
            blur: fullscreen("studio ao blur", &blur_pipeline_layout, "fs_blur"),
            input_layout,
            blur_layout,
            across: ints(device, "studio ao across", [1, 0, 0, 0]),
            down: ints(device, "studio ao down", [0, 1, 0, 0]),
            half_scale: ints(device, "studio ao half", [2, 0, 0, 0]),
            full_scale: ints(device, "studio ao full", [1, 0, 0, 0]),
        }
    }

    /// AO textures for a `full`-sized image, at half its resolution or full, reading the
    /// prepass's `distance` and `normals`.
    pub fn target(
        &self,
        device: &wgpu::Device,
        distance: &wgpu::TextureView,
        normals: &wgpu::TextureView,
        (w, h): (u32, u32),
        half: bool,
    ) -> AoTarget {
        use wgpu::TextureUsages as U;
        let size = if half {
            (w.div_ceil(2), h.div_ceil(2))
        } else {
            (w, h)
        };
        let usage = U::RENDER_ATTACHMENT | U::TEXTURE_BINDING;
        let view = |t: wgpu::Texture| t.create_view(&wgpu::TextureViewDescriptor::default());
        let a = view(texture(device, "studio ao", size, AO, usage));
        let b = view(texture(device, "studio ao blur", size, AO, usage));
        let input = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("studio ao input"),
            layout: &self.input_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(distance),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(normals),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: if half {
                        self.half_scale.as_entire_binding()
                    } else {
                        self.full_scale.as_entire_binding()
                    },
                },
            ],
        });
        let blur = |step: &wgpu::Buffer, source: &wgpu::TextureView| {
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("studio ao blur"),
                layout: &self.blur_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: step.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::TextureView(source),
                    },
                ],
            })
        };
        let across = blur(&self.across, &a);
        let down = blur(&self.down, &b);
        AoTarget {
            half,
            a,
            b,
            input,
            across,
            down,
        }
    }

    /// Draws view-space normals and distance from the camera of `meshes` (the floor
    /// included), depth-tested against `depth`.
    pub fn prepass(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        frame: &wgpu::BindGroup,
        depth: &wgpu::TextureView,
        [normals, distance]: [&wgpu::TextureView; 2],
        meshes: &[&StudioMesh],
    ) {
        let attachment = |view| {
            Some(wgpu::RenderPassColorAttachment {
                view,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::WHITE),
                    store: wgpu::StoreOp::Store,
                },
            })
        };
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("studio prepass"),
            color_attachments: &[attachment(normals), attachment(distance)],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: depth,
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Clear(1.0),
                    store: wgpu::StoreOp::Store,
                }),
                stencil_ops: None,
            }),
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        pass.set_pipeline(&self.prepass);
        pass.set_bind_group(0, frame, &[]);
        for m in meshes.iter().filter(|m| m.index_count > 0) {
            pass.set_vertex_buffer(0, m.vertices.slice(..));
            pass.set_index_buffer(m.indices.slice(..), wgpu::IndexFormat::Uint32);
            pass.draw_indexed(0..m.index_count, 0, 0..1);
        }
    }

    /// Searches for occlusion into `target.a`, then blurs it across (into b) and down (back
    /// into a).
    pub fn occlusion(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        frame: &wgpu::BindGroup,
        target: &AoTarget,
    ) {
        let steps = [
            (&self.search, &target.a, &target.input),
            (&self.blur, &target.b, &target.across),
            (&self.blur, &target.a, &target.down),
        ];
        for (pipeline, view, group) in steps {
            let mut pass = colour_pass(encoder, "studio ao", view, None);
            pass.set_pipeline(pipeline);
            pass.set_bind_group(0, frame, &[]);
            pass.set_bind_group(1, group, &[]);
            pass.draw(0..3, 0..1);
        }
    }
}
