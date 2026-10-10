//! The studio's shadows: a depth map from the key light (soft shadows on the form, the cloth
//! and the floor), and the floor's contact map (a soft darkening under whatever is close above
//! the floor). Both depend only on the geometry, so they are drawn again only when it changes.

use super::environment;
use super::mesh::StudioMesh;
use super::targets::{DEPTH, texture};
use crate::mesh::Vertex;
use glam::{Mat4, Vec3};

/// The key light's shadow map covers this square (metres), centred on the form (shade.wgsl
/// has the same numbers).
const KEY_BOX: f32 = 2.6;
const KEY_CENTRE: Vec3 = Vec3::new(0.0, 0.95, 0.0);
const KEY_DEPTH: f32 = 6.0;
/// The contact map covers this square of floor (metres), and things up to this height.
const CONTACT_BOX: f32 = 2.4;
const CONTACT_HEIGHT: f32 = 0.6;
/// Floor distance between blur taps (metres): two rounds of 9-tap passes each way spread the
/// contact shadow into a soft halo of about 10 cm round whatever is close above the floor.
/// The map is coarse enough (1–2 cm a texel) that the taps never skip texels, which would
/// stamp ghost copies instead of blurring.
const BLUR_STEP_M: f32 = 0.019;

const CONTACT_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::R8Unorm;

/// World to the key light's shadow map.
pub(crate) fn key_view_proj() -> Mat4 {
    let eye = KEY_CENTRE + environment::key_dir() * (KEY_DEPTH / 2.0);
    let view = glam::camera::rh::view::look_at_mat4(eye, KEY_CENTRE, Vec3::Y);
    let h = KEY_BOX / 2.0;
    glam::camera::rh::proj::directx::orthographic(-h, h, -h, h, 0.0, KEY_DEPTH) * view
}

/// The size of one key shadow-map texel on the ground, in metres.
pub(crate) fn key_texel(size: u32) -> f32 {
    KEY_BOX / size as f32
}

/// World to the floor's contact map (seen from just under the floor, looking up).
pub(crate) fn contact_view_proj() -> Mat4 {
    let eye = Vec3::new(0.0, -0.02, 0.0);
    let view = glam::camera::rh::view::look_at_mat4(eye, Vec3::Y, Vec3::NEG_Z);
    let h = CONTACT_BOX / 2.0;
    glam::camera::rh::proj::directx::orthographic(-h, h, -h, h, 0.0, CONTACT_HEIGHT + 0.02) * view
}

/// What the maps hold: drawn again when any of it changes.
#[derive(Clone, PartialEq, Eq)]
struct Drawn {
    epoch: u64,
    meshes: Vec<u64>,
    key: bool,
}

struct Maps {
    key_size: u32,
    contact_size: u32,
    key: wgpu::TextureView,
    contact: wgpu::TextureView,
    contact_depth: wgpu::TextureView,
    /// Blur passes: contact → spare (across), spare → contact (down).
    across: wgpu::BindGroup,
    down: wgpu::BindGroup,
    spare: wgpu::TextureView,
}

pub(crate) struct Shadows {
    key_pipeline: wgpu::RenderPipeline,
    contact_pipeline: wgpu::RenderPipeline,
    blur_pipeline: wgpu::RenderPipeline,
    blur_layout: wgpu::BindGroupLayout,
    key_light: wgpu::BindGroup,
    contact_light: wgpu::BindGroup,
    across_step: wgpu::Buffer,
    down_step: wgpu::Buffer,
    sampler: wgpu::Sampler,
    maps: Option<Maps>,
    drawn: Option<Drawn>,
    /// How many times the maps have been drawn.
    pub redraws: u64,
}

fn uniform_buffer(device: &wgpu::Device, label: &str, bytes: &[u8]) -> wgpu::Buffer {
    use wgpu::util::DeviceExt;
    device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some(label),
        contents: bytes,
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
    })
}

impl Shadows {
    pub fn new(device: &wgpu::Device) -> Self {
        let source = concat!(include_str!("common.wgsl"), include_str!("shadow.wgsl"));
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("studio shadows"),
            source: wgpu::ShaderSource::Wgsl(source.into()),
        });
        let uniform = |binding| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Uniform,
                has_dynamic_offset: false,
                min_binding_size: None,
            },
            count: None,
        };
        let light_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("studio light layout"),
            entries: &[uniform(0)],
        });
        let blur_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("studio blur layout"),
            entries: &[
                uniform(0),
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });
        let layout = |label, group: &wgpu::BindGroupLayout| {
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some(label),
                bind_group_layouts: &[Some(group)],
                immediate_size: 0,
            })
        };
        let light_pipeline_layout = layout("studio light pipeline layout", &light_layout);
        let blur_pipeline_layout = layout("studio blur pipeline layout", &blur_layout);
        let vertex = wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Vertex>() as u64,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x3],
        };
        let primitive = wgpu::PrimitiveState {
            cull_mode: None,
            ..Default::default()
        };
        let key_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("studio key shadow"),
            layout: Some(&light_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_depth"),
                compilation_options: Default::default(),
                buffers: &[Some(vertex.clone())],
            },
            fragment: None,
            primitive,
            depth_stencil: Some(wgpu::DepthStencilState {
                format: DEPTH,
                depth_write_enabled: Some(true),
                depth_compare: Some(wgpu::CompareFunction::Less),
                stencil: Default::default(),
                // Thin two-sided cloth must not shadow itself in stripes.
                bias: wgpu::DepthBiasState {
                    constant: 2,
                    slope_scale: 2.0,
                    clamp: 0.0,
                },
            }),
            multisample: Default::default(),
            multiview_mask: None,
            cache: None,
        });
        let contact_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("studio contact"),
            layout: Some(&light_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_depth"),
                compilation_options: Default::default(),
                buffers: &[Some(vertex)],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_contact"),
                compilation_options: Default::default(),
                targets: &[Some(CONTACT_FORMAT.into())],
            }),
            primitive,
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
        let blur_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("studio contact blur"),
            layout: Some(&blur_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_blur"),
                compilation_options: Default::default(),
                buffers: &[],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_blur"),
                compilation_options: Default::default(),
                targets: &[Some(CONTACT_FORMAT.into())],
            }),
            primitive: Default::default(),
            depth_stencil: None,
            multisample: Default::default(),
            multiview_mask: None,
            cache: None,
        });
        let light = |label, matrix: Mat4| {
            let buffer = uniform_buffer(
                device,
                label,
                bytemuck::bytes_of(&matrix.to_cols_array_2d()),
            );
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some(label),
                layout: &light_layout,
                entries: &[wgpu::BindGroupEntry {
                    binding: 0,
                    resource: buffer.as_entire_binding(),
                }],
            })
        };
        Self {
            key_light: light("studio key light", key_view_proj()),
            contact_light: light("studio contact light", contact_view_proj()),
            across_step: uniform_buffer(device, "studio blur across", &[0; 16]),
            down_step: uniform_buffer(device, "studio blur down", &[0; 16]),
            sampler: device.create_sampler(&wgpu::SamplerDescriptor {
                label: Some("studio blur"),
                mag_filter: wgpu::FilterMode::Linear,
                min_filter: wgpu::FilterMode::Linear,
                ..Default::default()
            }),
            key_pipeline,
            contact_pipeline,
            blur_pipeline,
            blur_layout,
            maps: None,
            drawn: None,
            redraws: 0,
        }
    }

    /// Makes the maps at these sizes if they aren't already; true when they were made anew
    /// (and the shader's textures must be bound again).
    pub fn ensure_maps(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        key_size: u32,
        contact_size: u32,
    ) -> bool {
        if self
            .maps
            .as_ref()
            .is_some_and(|m| m.key_size == key_size && m.contact_size == contact_size)
        {
            return false;
        }
        use wgpu::TextureUsages as U;
        let view = |t: wgpu::Texture| t.create_view(&wgpu::TextureViewDescriptor::default());
        let target = U::RENDER_ATTACHMENT | U::TEXTURE_BINDING;
        let contact_px = (contact_size, contact_size);
        let key = view(texture(
            device,
            "studio key shadow",
            (key_size, key_size),
            DEPTH,
            target,
        ));
        let contact = view(texture(
            device,
            "studio contact",
            contact_px,
            CONTACT_FORMAT,
            target,
        ));
        let spare = view(texture(
            device,
            "studio contact blur",
            contact_px,
            CONTACT_FORMAT,
            target,
        ));
        let contact_depth = view(texture(
            device,
            "studio contact depth",
            contact_px,
            DEPTH,
            target,
        ));
        let step = BLUR_STEP_M / CONTACT_BOX;
        queue.write_buffer(
            &self.across_step,
            0,
            bytemuck::cast_slice(&[step, 0.0, 0.0, 0.0]),
        );
        queue.write_buffer(
            &self.down_step,
            0,
            bytemuck::cast_slice(&[0.0, step, 0.0, 0.0]),
        );
        let blur = |label, step: &wgpu::Buffer, source: &wgpu::TextureView| {
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some(label),
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
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: wgpu::BindingResource::Sampler(&self.sampler),
                    },
                ],
            })
        };
        let across = blur("studio blur across", &self.across_step, &contact);
        let down = blur("studio blur down", &self.down_step, &spare);
        self.maps = Some(Maps {
            key_size,
            contact_size,
            key,
            contact,
            contact_depth,
            across,
            down,
            spare,
        });
        self.drawn = None;
        true
    }

    pub fn key_view(&self) -> Option<&wgpu::TextureView> {
        self.maps.as_ref().map(|m| &m.key)
    }

    pub fn contact_view(&self) -> Option<&wgpu::TextureView> {
        self.maps.as_ref().map(|m| &m.contact)
    }

    /// Draws the maps for `meshes` (not the floor) into `encoder` if what they hold is out of
    /// date: the geometry (`epoch`) changed, other meshes are shown, or the key map is now
    /// wanted (`key`) and wasn't drawn.
    pub fn draw_if_needed(
        &mut self,
        encoder: &mut wgpu::CommandEncoder,
        meshes: &[&StudioMesh],
        epoch: u64,
        key: bool,
    ) {
        let Some(maps) = &self.maps else { return };
        let now = Drawn {
            epoch,
            meshes: meshes.iter().map(|m| m.id).collect(),
            key,
        };
        // A key map drawn already serves a frame that doesn't need it.
        if self
            .drawn
            .as_ref()
            .is_some_and(|d| d.epoch == now.epoch && d.meshes == now.meshes && (d.key || !key))
        {
            return;
        }
        let draw_meshes = |pass: &mut wgpu::RenderPass| {
            for m in meshes.iter().filter(|m| m.index_count > 0) {
                pass.set_vertex_buffer(0, m.vertices.slice(..));
                pass.set_index_buffer(m.indices.slice(..), wgpu::IndexFormat::Uint32);
                pass.draw_indexed(0..m.index_count, 0, 0..1);
            }
        };
        if key {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("studio key shadow"),
                color_attachments: &[],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &maps.key,
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
            pass.set_pipeline(&self.key_pipeline);
            pass.set_bind_group(0, &self.key_light, &[]);
            draw_meshes(&mut pass);
        }
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("studio contact"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &maps.contact,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &maps.contact_depth,
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
            pass.set_pipeline(&self.contact_pipeline);
            pass.set_bind_group(0, &self.contact_light, &[]);
            draw_meshes(&mut pass);
        }
        // Two rounds of across-then-down: soft, with no blocky edge.
        for _ in 0..2 {
            for (target, bind) in [(&maps.spare, &maps.across), (&maps.contact, &maps.down)] {
                let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("studio contact blur"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: target,
                        depth_slice: None,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                            store: wgpu::StoreOp::Store,
                        },
                    })],
                    depth_stencil_attachment: None,
                    timestamp_writes: None,
                    occlusion_query_set: None,
                    multiview_mask: None,
                });
                pass.set_pipeline(&self.blur_pipeline);
                pass.set_bind_group(0, bind, &[]);
                pass.draw(0..3, 0..1);
            }
        }
        self.drawn = Some(now);
        self.redraws += 1;
    }
}
