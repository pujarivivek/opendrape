//! The studio 3D view: a soft grey photo studio lit by a baked studio HDRI and one soft key
//! light, with colour-accurate output (Khronos PBR Neutral, exact sRGB).

mod ao;
pub mod environment;
mod environment_data;
mod frame;
pub mod look;
mod mesh;
mod output;
pub mod quality;
mod shadow;
mod targets;

use mesh::MeshGpu;
pub use mesh::{Material, StudioMesh};

use crate::camera::OrbitCamera;
use crate::colour::srgb8_to_linear;
use crate::mesh::Vertex;
use crate::target::RenderTarget;
use ao::{AoPass, AoTarget};
use frame::FrameUniforms;
use glam::{Mat4, Vec2, Vec3};
use output::{OutputPass, Show, Source};
use quality::{AoSettings, Quality, ShadowSettings};
use shadow::Shadows;
use targets::{DEPTH, Targets, texture};

/// Test switches: force an effect on or off whatever the quality says, or the LDR path.
#[doc(hidden)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
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

/// How hard a frame works: what it draws, from the quality level and the test overrides.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Effects {
    ao: Option<AoSettings>,
    shadow: Option<ShadowSettings>,
    contact: bool,
}

/// Counts, for tests and the frame-rate overlay.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Stats {
    /// Times the shadow maps were drawn.
    pub shadow_redraws: u64,
    /// Times an image was drawn.
    pub frames_drawn: u64,
}

/// How dark the contact shadow is right under something touching the floor.
const CONTACT_OPACITY: f32 = 0.75;

/// An effect's settings as `now`, unless a test forces it on (then `fallback` if it was off)
/// or off.
fn forced<T>(wanted: Option<bool>, now: Option<T>, fallback: T) -> Option<T> {
    match wanted {
        Some(true) => now.or(Some(fallback)),
        Some(false) => None,
        None => now,
    }
}

/// The `i`th number (from 1) of the Halton sequence in `base`: evenly spread over 0..1.
fn halton(mut i: u32, base: u32) -> f32 {
    let (mut f, mut r) = (1.0, 0.0);
    while i > 0 {
        f /= base as f32;
        r += f * (i % base) as f32;
        i /= base;
    }
    r
}

/// The format the lit scene is drawn in: half-float HDR where the GPU can draw into it.
const HDR: wgpu::TextureFormat = wgpu::TextureFormat::Rgba16Float;
const LDR: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;

/// 1×1 stand-ins for textures not drawn (yet): the flags tell the shader not to read them.
struct Placeholders {
    shadow: wgpu::TextureView,
    contact: wgpu::TextureView,
    ao: wgpu::TextureView,
    prepass: wgpu::TextureView,
}

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
    textures_layout: wgpu::BindGroupLayout,
    compare_sampler: wgpu::Sampler,
    linear_sampler: wgpu::Sampler,
    placeholders: Placeholders,
    /// The main pass's textures, one set per AO resolution (half or full); made again when a
    /// texture they bind is made anew.
    textures_bind_groups: Vec<(bool, wgpu::BindGroup)>,
    ao: AoPass,
    /// AO textures at each resolution this quality level uses.
    ao_targets: Vec<AoTarget>,
    /// The quality level the targets were made for.
    targets_quality: Option<Quality>,
    shadows: Shadows,
    frames_drawn: u64,
    /// The last frame's signature: the same again means the view is still.
    last_signature: Option<u64>,
    /// Still frames averaged so far, and whether that's all of them.
    still_count: u32,
    still_done: bool,
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
                texture_entry(5, T::Float { filterable: false }),
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
        let prepass = view(texture(device, "no prepass", one, LDR, U::TEXTURE_BINDING));
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
        Self {
            hdr_ok,
            quality,
            overrides: Overrides::default(),
            shader,
            draw_layout,
            pipeline_layout,
            frame_uniforms,
            frame_bind_group,
            textures_layout,
            compare_sampler: compare,
            linear_sampler: linear,
            placeholders: Placeholders {
                shadow,
                contact,
                ao,
                prepass,
            },
            textures_bind_groups: Vec::new(),
            ao: AoPass::new(device, &frame_layout),
            ao_targets: Vec::new(),
            targets_quality: None,
            shadows: Shadows::new(device),
            frames_drawn: 0,
            last_signature: None,
            still_count: 0,
            still_done: false,
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
            && self.targets_quality == Some(self.quality)
        {
            return;
        }
        let targets = Targets::new(device, width, height, format);
        let averages = [&targets.averages[0], &targets.averages[1]];
        self.output
            .bind(device, &targets.colour_view, averages, format);
        let s = self.quality.settings();
        let mut resolutions: Vec<bool> = [s.ao_moving, Some(s.ao_still)]
            .into_iter()
            .flatten()
            .map(|a| a.half_res)
            .collect();
        resolutions.dedup();
        self.ao_targets = resolutions
            .into_iter()
            .map(|half| {
                self.ao.target(
                    device,
                    &targets.distance,
                    &targets.normals,
                    (width, height),
                    half,
                )
            })
            .collect();
        self.targets = Some(targets);
        self.targets_quality = Some(self.quality);
        self.textures_bind_groups.clear();
    }

    /// What this frame draws: the quality level's moving or still settings, with the test
    /// overrides on top.
    fn effects(&self, still: bool) -> Effects {
        let s = self.quality.settings();
        let (ao, shadow) = if still {
            (Some(s.ao_still), Some(s.shadow_still))
        } else {
            (s.ao_moving, s.shadow_moving)
        };
        Effects {
            ao: forced(self.overrides.ao, ao, s.ao_still),
            shadow: forced(self.overrides.key_shadows, shadow, s.shadow_still),
            contact: self.overrides.contact.unwrap_or(true),
        }
    }

    /// Everything that decides what the image looks like: the same as last frame means the
    /// view is still.
    fn signature(
        &self,
        camera: &OrbitCamera,
        target: &RenderTarget,
        meshes: &[&StudioMesh],
    ) -> u64 {
        use std::hash::{Hash, Hasher};
        let mut h = std::collections::hash_map::DefaultHasher::new();
        let c = camera;
        for v in [
            c.target.x, c.target.y, c.target.z, c.yaw, c.pitch, c.distance, c.fov_y,
        ] {
            v.to_bits().hash(&mut h);
        }
        (target.width, target.height).hash(&mut h);
        (self.quality, self.overrides, self.geometry_epoch).hash(&mut h);
        for m in meshes {
            (m.id, m.colour.map(f32::to_bits), m.material).hash(&mut h);
        }
        h.finish()
    }

    pub fn stats(&self) -> Stats {
        Stats {
            shadow_redraws: self.shadows.redraws,
            frames_drawn: self.frames_drawn,
        }
    }

    /// Binds the textures the main pass reads, the real ones where they exist: one set per
    /// AO resolution.
    fn bind_textures(&mut self, device: &wgpu::Device) {
        let p = &self.placeholders;
        let shadow = self.shadows.key_view().unwrap_or(&p.shadow);
        let contact = self.shadows.contact_view().unwrap_or(&p.contact);
        let prepass = self.targets.as_ref().map_or(&p.prepass, |t| &t.distance);
        let mut sets: Vec<(bool, &wgpu::TextureView)> =
            self.ao_targets.iter().map(|t| (t.half, &t.a)).collect();
        if sets.is_empty() {
            sets.push((true, &p.ao));
        }
        self.textures_bind_groups = sets
            .into_iter()
            .map(|(half, ao)| {
                let views = [(0, shadow), (2, contact), (4, ao), (5, prepass)];
                let mut entries: Vec<wgpu::BindGroupEntry> = views
                    .iter()
                    .map(|&(binding, view)| wgpu::BindGroupEntry {
                        binding,
                        resource: wgpu::BindingResource::TextureView(view),
                    })
                    .collect();
                entries.push(wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&self.compare_sampler),
                });
                entries.push(wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::Sampler(&self.linear_sampler),
                });
                let group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some("studio textures"),
                    layout: &self.textures_layout,
                    entries: &entries,
                });
                (half, group)
            })
            .collect();
    }

    fn frame_uniforms(
        &self,
        camera: &OrbitCamera,
        (width, height): (u32, u32),
        jitter: Vec2,
        effects: Effects,
        frame_index: u32,
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
        let key_size = self.quality.settings().shadow_still.size;
        let taps = effects.shadow.map_or(0, |s| s.taps);
        let samples = effects.ao.map_or(0, |a| a.samples);
        FrameUniforms {
            view_proj: view_proj.to_cols_array_2d(),
            inv_view_proj: view_proj.inverse().to_cols_array_2d(),
            view: view.to_cols_array_2d(),
            proj: proj.to_cols_array_2d(),
            inv_proj: proj.inverse().to_cols_array_2d(),
            key_view_proj: shadow::key_view_proj().to_cols_array_2d(),
            contact_view_proj: shadow::contact_view_proj().to_cols_array_2d(),
            camera_pos: v4(camera.eye()),
            key_dir: v4(environment::key_dir()),
            key_colour: v4(environment::key_colour()),
            sh: environment::sh().map(|[r, g, b]| [r, g, b, 0.0]),
            horizon: displayed(look::HORIZON_SRGB),
            top: displayed(look::TOP_SRGB),
            params: [exposure, frame_index as f32, fade_start, fade_end],
            flags: [
                u32::from(effects.ao.is_some()),
                u32::from(effects.shadow.is_some()),
                u32::from(effects.contact),
                u32::from(ldr),
            ],
            screen: [w, h, 1.0 / w, 1.0 / h],
            extra: [
                taps as f32,
                samples as f32,
                shadow::key_texel(key_size),
                CONTACT_OPACITY,
            ],
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
        let signature = self.signature(camera, target, meshes);
        let still = self.last_signature == Some(signature);
        self.last_signature = Some(signature);
        if still && self.still_done {
            return Rendered {
                drew: false,
                still_done: true,
            };
        }
        if !still {
            self.still_count = 0;
            self.still_done = false;
        }
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
        let effects = self.effects(still);
        let settings = self.quality.settings();
        let remade = self.shadows.ensure_maps(
            device,
            queue,
            settings.shadow_still.size,
            settings.contact_size,
        );
        if remade || self.textures_bind_groups.is_empty() {
            self.bind_textures(device);
        }
        let size = (target.width, target.height);
        // Each still frame is shifted a different fraction of a pixel (Halton 2, 3) and turns
        // the shadow and AO noise, so their average has smooth edges and smooth shading.
        let (jitter, index) = if still {
            let i = self.still_count + 1;
            (Vec2::new(halton(i, 2) - 0.5, halton(i, 3) - 0.5), i)
        } else {
            (Vec2::ZERO, 0)
        };
        let uniforms = self.frame_uniforms(camera, size, jitter, effects, index);
        queue.write_buffer(&self.frame_uniforms, 0, bytemuck::bytes_of(&uniforms));
        let floor = self.floor.as_ref().expect("made above");
        for m in meshes.iter().copied().chain([floor]) {
            queue.write_buffer(&m.uniforms, 0, bytemuck::bytes_of(&m.draw_uniforms()));
        }
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("studio"),
        });
        self.shadows.draw_if_needed(
            &mut encoder,
            meshes,
            self.geometry_epoch,
            effects.shadow.is_some(),
        );
        let floor = self.floor.as_ref().expect("made above");
        let pipelines = self.pipelines.as_ref().expect("made above");
        let targets = self.targets.as_ref().expect("made above");
        let ao_half = effects.ao.map(|a| a.half_res);
        if let Some(half) = ao_half {
            let all: Vec<&StudioMesh> = meshes.iter().copied().chain([floor]).collect();
            self.ao.prepass(
                &mut encoder,
                &self.frame_bind_group,
                &targets.prepass_depth,
                [&targets.normals, &targets.distance],
                &all,
            );
            if let Some(target) = self.ao_targets.iter().find(|t| t.half == half) {
                self.ao
                    .occlusion(&mut encoder, &self.frame_bind_group, target);
            }
        }
        let textures = self
            .textures_bind_groups
            .iter()
            .find(|(half, _)| Some(*half) == ao_half)
            .or(self.textures_bind_groups.first())
            .map(|(_, group)| group)
            .expect("bound above");
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
            pass.set_bind_group(2, textures, &[]);
            pass.set_pipeline(&pipelines.mesh);
            for m in meshes
                .iter()
                .copied()
                .chain([floor])
                .filter(|m| m.index_count > 0)
            {
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
        let source = if still {
            let into = (self.still_count % 2) as usize;
            let weight = 1.0 / (self.still_count + 1) as f32;
            self.output
                .accumulate(queue, &mut encoder, into, &targets.averages[into], weight);
            self.still_count += 1;
            self.still_done = self.still_count >= settings.still_frames;
            Source::Average(into)
        } else {
            Source::Scene
        };
        let show = Show {
            source,
            exposure: environment::exposure(),
            ldr: format == LDR,
            fxaa: !still && settings.fxaa_moving,
        };
        self.output
            .draw(queue, &mut encoder, &target.color_view, show);
        queue.submit([encoder.finish()]);
        self.frames_drawn += 1;
        Rendered {
            drew: true,
            still_done: self.still_done,
        }
    }
}
