//! The studio 3D view: a soft grey photo studio lit by a baked studio HDRI and one soft key
//! light, with colour-accurate output (Khronos PBR Neutral, exact sRGB).

pub mod environment;
mod environment_data;
mod frame;
pub mod look;
mod mesh;
mod output;
pub mod quality;
mod targets;

use mesh::MeshGpu;
pub use mesh::{Material, StudioMesh};

use crate::camera::OrbitCamera;
use crate::colour::srgb8_to_linear;
use crate::mesh::Vertex;
use crate::target::RenderTarget;
use frame::FrameUniforms;
use glam::{Mat4, Vec2, Vec3};
use output::OutputPass;
use quality::Quality;
use targets::{DEPTH, Targets, texture};

/// Test switches: force an effect on or off whatever the quality says, or the LDR path.
#[doc(hidden)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Overrides {
    pub ao: Option<bool>,
    pub key_shadows: Option<bool>,
    pub contact: Option<bool>,
    pub force_ldr: bool,
}

/// What a call to [`StudioRenderer::render`] did.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rendered {
    /// The target was drawn this time (false: the still image there is already finished).
    pub drew: bool,
    /// The view is still and its image finished: nothing to draw until something changes.
    pub still_done: bool,
}

/// The format the lit scene is drawn in: half-float HDR where the GPU can draw into it.
const HDR: wgpu::TextureFormat = wgpu::TextureFormat::Rgba16Float;
const LDR: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;

struct Pipelines {
    format: wgpu::TextureFormat,
    mesh: wgpu::RenderPipeline,
    backdrop: wgpu::RenderPipeline,
}

pub struct StudioRenderer {
    hdr_ok: bool,
    quality: Quality,
    overrides: Overrides,
    shader: wgpu::ShaderModule,
    draw_layout: wgpu::BindGroupLayout,
    pipeline_layout: wgpu::PipelineLayout,
    frame_uniforms: wgpu::Buffer,
    frame_bind_group: wgpu::BindGroup,
    textures_bind_group: wgpu::BindGroup,
    pipelines: Option<Pipelines>,
    targets: Option<Targets>,
    output: OutputPass,
    /// Made on the first draw (it needs the queue).
    floor: Option<StudioMesh>,
    next_mesh_id: u64,
    /// Goes up whenever any mesh's vertices or triangles change.
    geometry_epoch: u64,
}

fn uniform_entry(binding: u32) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
        ty: wgpu::BindingType::Buffer {
            ty: wgpu::BufferBindingType::Uniform,
            has_dynamic_offset: false,
            min_binding_size: None,
        },
        count: None,
    }
}

fn texture_entry(binding: u32, sample_type: wgpu::TextureSampleType) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::FRAGMENT,
        ty: wgpu::BindingType::Texture {
            sample_type,
            view_dimension: wgpu::TextureViewDimension::D2,
            multisampled: false,
        },
        count: None,
    }
}

fn sampler_entry(binding: u32, kind: wgpu::SamplerBindingType) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::FRAGMENT,
        ty: wgpu::BindingType::Sampler(kind),
        count: None,
    }
}

impl StudioRenderer {
    pub fn new(device: &wgpu::Device, adapter: &wgpu::Adapter, quality: Quality) -> Self {
        use wgpu::TextureSampleType as T;
        use wgpu::TextureUsages as U;
        let wanted = U::RENDER_ATTACHMENT | U::TEXTURE_BINDING;
        let hdr_ok = adapter
            .get_texture_format_features(HDR)
            .allowed_usages
            .contains(wanted);
        let source = concat!(include_str!("common.wgsl"), include_str!("shade.wgsl"));
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("studio shade"),
            source: wgpu::ShaderSource::Wgsl(source.into()),
        });
        let frame_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("studio frame layout"),
            entries: &[uniform_entry(0)],
        });
        let draw_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("studio draw layout"),
            entries: &[uniform_entry(0)],
        });
        let textures_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("studio textures layout"),
            entries: &[
                texture_entry(0, T::Depth),
                sampler_entry(1, wgpu::SamplerBindingType::Comparison),
                texture_entry(2, T::Float { filterable: true }),
                sampler_entry(3, wgpu::SamplerBindingType::Filtering),
                texture_entry(4, T::Float { filterable: false }),
                texture_entry(5, T::Depth),
            ],
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("studio pipeline layout"),
            bind_group_layouts: &[
                Some(&frame_layout),
                Some(&draw_layout),
                Some(&textures_layout),
            ],
            immediate_size: 0,
        });
        let frame_uniforms = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("studio frame uniforms"),
            size: std::mem::size_of::<FrameUniforms>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let frame_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("studio frame"),
            layout: &frame_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: frame_uniforms.as_entire_binding(),
            }],
        });
        // Until shadows and soft darkening exist, the shader reads these 1×1 stand-ins (and
        // the flags tell it not to use them).
        let view = |t: wgpu::Texture| t.create_view(&wgpu::TextureViewDescriptor::default());
        let one = (1, 1);
        let shadow = view(texture(device, "no shadow", one, DEPTH, U::TEXTURE_BINDING));
        let contact = view(texture(
            device,
            "no contact",
            one,
            wgpu::TextureFormat::R8Unorm,
            U::TEXTURE_BINDING,
        ));
        let ao = view(texture(device, "no ao", one, LDR, U::TEXTURE_BINDING));
        let prepass = view(texture(
            device,
            "no prepass",
            one,
            DEPTH,
            U::TEXTURE_BINDING,
        ));
        let compare = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("studio shadow compare"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            compare: Some(wgpu::CompareFunction::LessEqual),
            ..Default::default()
        });
        let linear = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("studio linear"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        let textures_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("studio textures"),
            layout: &textures_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&shadow),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&compare),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(&contact),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::Sampler(&linear),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: wgpu::BindingResource::TextureView(&ao),
                },
                wgpu::BindGroupEntry {
                    binding: 5,
                    resource: wgpu::BindingResource::TextureView(&prepass),
                },
            ],
        });
        Self {
            hdr_ok,
            quality,
            overrides: Overrides::default(),
            shader,
            draw_layout,
            pipeline_layout,
            frame_uniforms,
            frame_bind_group,
            textures_bind_group,
            pipelines: None,
            targets: None,
            output: OutputPass::new(device),
            floor: None,
            next_mesh_id: 1,
            geometry_epoch: 0,
        }
    }

    /// Whether this GPU can draw into half-float textures (without them: the simpler LDR path).
    pub fn hdr_ok(&self) -> bool {
        self.hdr_ok
    }

    pub fn quality(&self) -> Quality {
        self.quality
    }

    pub fn set_quality(&mut self, quality: Quality) {
        self.quality = quality;
    }

    #[doc(hidden)]
    pub fn set_overrides(&mut self, overrides: Overrides) {
        self.overrides = overrides;
    }

    /// The size to draw a `width`×`height` view at on this quality level (it is scaled up to
    /// fill the view).
    pub fn render_size(&self, width: u32, height: u32) -> (u32, u32) {
        quality::render_size(width, height, self.quality.settings().cap_px)
    }

    pub fn create_mesh(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        positions: &[Vec3],
        triangles: &[[u32; 3]],
        colour: [f32; 3],
        material: Material,
    ) -> StudioMesh {
        let id = self.next_mesh_id;
        self.next_mesh_id += 1;
        self.geometry_epoch += 1;
        StudioMesh::new(
            MeshGpu {
                device,
                queue,
                layout: &self.draw_layout,
            },
            id,
            positions,
            triangles,
            colour,
            material,
        )
    }

    /// Replaces a mesh's vertex positions (normals are recomputed) and optionally its
    /// triangles.
    pub fn update_mesh(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        mesh: &mut StudioMesh,
        positions: &[Vec3],
        triangles: Option<&[[u32; 3]]>,
    ) {
        self.geometry_epoch += 1;
        if mesh.fits(positions.len(), triangles.map(<[_]>::len)) {
            mesh.upload(queue, positions, triangles);
        } else {
            let tris = triangles.map_or_else(|| mesh.triangles().to_vec(), <[_]>::to_vec);
            *mesh = StudioMesh::new(
                MeshGpu {
                    device,
                    queue,
                    layout: &self.draw_layout,
                },
                mesh.id,
                positions,
                &tris,
                mesh.colour,
                mesh.material,
            );
        }
    }

    fn colour_format(&self) -> wgpu::TextureFormat {
        if self.hdr_ok && !self.overrides.force_ldr {
            HDR
        } else {
            LDR
        }
    }

    fn ensure_pipelines(&mut self, device: &wgpu::Device, format: wgpu::TextureFormat) {
        if self.pipelines.as_ref().is_some_and(|p| p.format == format) {
            return;
        }
        let pipeline = |label: &str,
                        vs: &str,
                        fs: &str,
                        buffers: &[Option<wgpu::VertexBufferLayout>],
                        depth: (bool, wgpu::CompareFunction)| {
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some(label),
                layout: Some(&self.pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &self.shader,
                    entry_point: Some(vs),
                    compilation_options: Default::default(),
                    buffers,
                },
                fragment: Some(wgpu::FragmentState {
                    module: &self.shader,
                    entry_point: Some(fs),
                    compilation_options: Default::default(),
                    targets: &[Some(format.into())],
                }),
                primitive: wgpu::PrimitiveState {
                    cull_mode: None,
                    ..Default::default()
                },
                depth_stencil: Some(wgpu::DepthStencilState {
                    format: DEPTH,
                    depth_write_enabled: Some(depth.0),
                    depth_compare: Some(depth.1),
                    stencil: Default::default(),
                    bias: Default::default(),
                }),
                multisample: Default::default(),
                multiview_mask: None,
                cache: None,
            })
        };
        let vertex = wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Vertex>() as u64,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x3],
        };
        let mesh = pipeline(
            "studio meshes",
            "vs_mesh",
            "fs_mesh",
            &[Some(vertex)],
            (true, wgpu::CompareFunction::Less),
        );
        let backdrop = pipeline(
            "studio backdrop",
            "vs_backdrop",
            "fs_backdrop",
            &[],
            (false, wgpu::CompareFunction::LessEqual),
        );
        self.pipelines = Some(Pipelines {
            format,
            mesh,
            backdrop,
        });
    }

    fn ensure_targets(
        &mut self,
        device: &wgpu::Device,
        width: u32,
        height: u32,
        format: wgpu::TextureFormat,
    ) {
        if self
            .targets
            .as_ref()
            .is_some_and(|t| t.matches(width, height, format))
        {
            return;
        }
        let targets = Targets::new(device, width, height, format);
        self.output.bind(device, &targets.colour_view);
        self.targets = Some(targets);
    }

    fn frame_uniforms(
        &self,
        camera: &OrbitCamera,
        width: u32,
        height: u32,
        jitter: Vec2,
    ) -> FrameUniforms {
        let (w, h) = (width as f32, height as f32);
        // A sub-pixel shift of the whole image (for still frames), in clip space.
        let shift = Mat4::from_translation(glam::vec3(2.0 * jitter.x / w, 2.0 * jitter.y / h, 0.0));
        let proj = shift * camera.proj(w / h);
        let view = camera.view();
        let view_proj = proj * view;
        let exposure = environment::exposure();
        // The backdrop shows as `look` says: PBR Neutral takes 4 % off, and exposure scales.
        let displayed = |srgb: [u8; 3]| {
            let [r, g, b] = srgb8_to_linear(srgb).map(|c| (c + 0.04) / exposure);
            [r, g, b, 1.0]
        };
        let v4 = |v: Vec3| [v.x, v.y, v.z, 0.0];
        let ldr = self.colour_format() == LDR;
        let (fade_start, fade_end) = look::FLOOR_FADE;
        FrameUniforms {
            view_proj: view_proj.to_cols_array_2d(),
            inv_view_proj: view_proj.inverse().to_cols_array_2d(),
            view: view.to_cols_array_2d(),
            inv_proj: proj.inverse().to_cols_array_2d(),
            key_view_proj: Mat4::IDENTITY.to_cols_array_2d(),
            contact_view_proj: Mat4::IDENTITY.to_cols_array_2d(),
            camera_pos: v4(camera.eye()),
            key_dir: v4(environment::key_dir()),
            key_colour: v4(environment::key_colour()),
            sh: environment::sh().map(|[r, g, b]| [r, g, b, 0.0]),
            horizon: displayed(look::HORIZON_SRGB),
            top: displayed(look::TOP_SRGB),
            params: [exposure, 0.0, fade_start, fade_end],
            flags: [0, 0, 0, u32::from(ldr)],
            screen: [w, h, 1.0 / w, 1.0 / h],
            extra: [0.0; 4],
        }
    }

    /// Draws the studio with `meshes` (and the floor) into `target`, seen by `camera`.
    pub fn render(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        target: &RenderTarget,
        camera: &OrbitCamera,
        meshes: &[&StudioMesh],
    ) -> Rendered {
        let format = self.colour_format();
        self.ensure_pipelines(device, format);
        self.ensure_targets(device, target.width, target.height, format);
        if self.floor.is_none() {
            let (positions, triangles) = mesh::floor_disc(look::FLOOR_RADIUS);
            self.floor = Some(StudioMesh::new(
                MeshGpu {
                    device,
                    queue,
                    layout: &self.draw_layout,
                },
                0,
                &positions,
                &triangles,
                srgb8_to_linear(look::HORIZON_SRGB),
                Material::Floor,
            ));
        }
        let uniforms = self.frame_uniforms(camera, target.width, target.height, Vec2::ZERO);
        queue.write_buffer(&self.frame_uniforms, 0, bytemuck::bytes_of(&uniforms));
        let floor = self.floor.as_ref().expect("made above");
        let all: Vec<&StudioMesh> = meshes.iter().copied().chain([floor]).collect();
        for m in &all {
            queue.write_buffer(&m.uniforms, 0, bytemuck::bytes_of(&m.draw_uniforms()));
        }
        let pipelines = self.pipelines.as_ref().expect("made above");
        let targets = self.targets.as_ref().expect("made above");
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("studio"),
        });
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("studio main"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &targets.colour_view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &targets.depth_view,
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
            pass.set_bind_group(0, &self.frame_bind_group, &[]);
            pass.set_bind_group(2, &self.textures_bind_group, &[]);
            pass.set_pipeline(&pipelines.mesh);
            for m in all.iter().filter(|m| m.index_count > 0) {
                pass.set_bind_group(1, &m.bind_group, &[]);
                pass.set_vertex_buffer(0, m.vertices.slice(..));
                pass.set_index_buffer(m.indices.slice(..), wgpu::IndexFormat::Uint32);
                pass.draw_indexed(0..m.index_count, 0, 0..1);
            }
            // Behind everything, where nothing was drawn.
            pass.set_bind_group(1, &floor.bind_group, &[]);
            pass.set_pipeline(&pipelines.backdrop);
            pass.draw(0..3, 0..1);
        }
        self.output.draw(
            queue,
            &mut encoder,
            &target.color_view,
            environment::exposure(),
            format == LDR,
        );
        queue.submit([encoder.finish()]);
        Rendered {
            drew: true,
            still_done: false,
        }
    }
}
