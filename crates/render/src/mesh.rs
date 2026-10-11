use crate::target::{CLEAR_COLOR, COLOR_FORMAT, DEPTH_FORMAT, RenderTarget};
use glam::{Mat4, Vec3};

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct Vertex {
    pub(crate) position: [f32; 3],
    pub(crate) normal: [f32; 3],
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct Uniforms {
    view_proj: [[f32; 4]; 4],
    color: [f32; 4],
}

/// Area-weighted vertex normals; vertices used by no triangle get +Y. The triangles must be
/// wound consistently (see [`orient_consistently`]), or the two sides of a seam between
/// pieces facing opposite ways cancel into a dark line.
pub fn vertex_normals(positions: &[Vec3], triangles: &[[u32; 3]]) -> Vec<Vec3> {
    let mut n = vec![Vec3::ZERO; positions.len()];
    for t in triangles {
        let [a, b, c] = t.map(|k| k as usize);
        let face = (positions[b] - positions[a]).cross(positions[c] - positions[a]);
        n[a] += face;
        n[b] += face;
        n[c] += face;
    }
    n.into_iter()
        .map(|v| v.try_normalize().unwrap_or(Vec3::Y))
        .collect()
}

/// The triangles wound the same way as their neighbours across every shared edge, so that a
/// piece sewn to its mirror image placed without a turn (whose triangles face the other way)
/// lights as one surface. Each connected run of triangles is walked from its first; a
/// triangle reached across an edge that its neighbour goes round the same way is turned.
/// The lighting is two-sided, so which way a run ends up facing does not matter.
pub fn orient_consistently(triangles: &[[u32; 3]]) -> Vec<[u32; 3]> {
    use std::collections::HashMap;
    let mut out = triangles.to_vec();
    let mut by_edge: HashMap<(u32, u32), Vec<usize>> = HashMap::new();
    for (t, tri) in triangles.iter().enumerate() {
        for k in 0..3 {
            let (a, b) = (tri[k], tri[(k + 1) % 3]);
            by_edge.entry((a.min(b), a.max(b))).or_default().push(t);
        }
    }
    let mut seen = vec![false; out.len()];
    let mut stack = Vec::new();
    for start in 0..out.len() {
        if seen[start] {
            continue;
        }
        seen[start] = true;
        stack.push(start);
        while let Some(t) = stack.pop() {
            let tri = out[t];
            for k in 0..3 {
                // This triangle goes a → b along the edge; a neighbour should go b → a.
                let (a, b) = (tri[k], tri[(k + 1) % 3]);
                for &n in &by_edge[&(a.min(b), a.max(b))] {
                    if seen[n] {
                        continue;
                    }
                    seen[n] = true;
                    let m = out[n];
                    if (0..3).any(|j| m[j] == a && m[(j + 1) % 3] == b) {
                        out[n] = [m[0], m[2], m[1]];
                    }
                    stack.push(n);
                }
            }
        }
    }
    out
}

/// A triangle mesh on the GPU whose vertices (and triangles) can be replaced every frame.
pub struct GpuMesh {
    vertices: wgpu::Buffer,
    indices: wgpu::Buffer,
    vertex_capacity: usize,
    index_capacity: usize,
    index_count: u32,
    triangles: Vec<[u32; 3]>,
    uniforms: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
    color: [f32; 3],
}

impl GpuMesh {
    /// Draws the mesh in `color` from the next frame on.
    pub fn set_color(&mut self, color: [f32; 3]) {
        self.color = color;
    }
}

pub struct MeshRenderer {
    pipeline: wgpu::RenderPipeline,
    layout: wgpu::BindGroupLayout,
}

impl MeshRenderer {
    pub fn new(device: &wgpu::Device) -> Self {
        let shader = device.create_shader_module(wgpu::include_wgsl!("mesh.wgsl"));
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("mesh uniforms layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("mesh pipeline layout"),
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("mesh pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: &[Some(wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<Vertex>() as u64,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x3],
                })],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                compilation_options: Default::default(),
                targets: &[Some(COLOR_FORMAT.into())],
            }),
            primitive: wgpu::PrimitiveState {
                cull_mode: None,
                ..Default::default()
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: DEPTH_FORMAT,
                depth_write_enabled: Some(true),
                depth_compare: Some(wgpu::CompareFunction::Less),
                stencil: Default::default(),
                bias: Default::default(),
            }),
            multisample: Default::default(),
            multiview_mask: None,
            cache: None,
        });
        Self { pipeline, layout }
    }

    pub fn create_mesh(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        positions: &[Vec3],
        triangles: &[[u32; 3]],
        color: [f32; 3],
    ) -> GpuMesh {
        let buffer = |label: &str, size: usize, usage: wgpu::BufferUsages| {
            device.create_buffer(&wgpu::BufferDescriptor {
                label: Some(label),
                size: size.max(16) as u64,
                usage,
                mapped_at_creation: false,
            })
        };
        let vertices = buffer(
            "mesh vertices",
            positions.len() * std::mem::size_of::<Vertex>(),
            wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
        );
        let indices = buffer(
            "mesh indices",
            triangles.len() * 12,
            wgpu::BufferUsages::INDEX | wgpu::BufferUsages::COPY_DST,
        );
        let uniforms = buffer(
            "mesh uniforms",
            std::mem::size_of::<Uniforms>(),
            wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        );
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("mesh uniforms"),
            layout: &self.layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniforms.as_entire_binding(),
            }],
        });
        let mut mesh = GpuMesh {
            vertices,
            indices,
            vertex_capacity: positions.len(),
            index_capacity: triangles.len() * 3,
            index_count: 0,
            triangles: vec![],
            uniforms,
            bind_group,
            color,
        };
        self.upload(queue, &mut mesh, positions, Some(triangles));
        mesh
    }

    /// Replaces vertex positions (normals are recomputed) and optionally the triangles.
    pub fn update_mesh(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        mesh: &mut GpuMesh,
        positions: &[Vec3],
        triangles: Option<&[[u32; 3]]>,
    ) {
        let tri_len = triangles.map_or(mesh.triangles.len(), <[_]>::len);
        if positions.len() > mesh.vertex_capacity || tri_len * 3 > mesh.index_capacity {
            let tris = triangles.map_or_else(|| mesh.triangles.clone(), <[_]>::to_vec);
            *mesh = self.create_mesh(device, queue, positions, &tris, mesh.color);
        } else {
            self.upload(queue, mesh, positions, triangles);
        }
    }

    fn upload(
        &self,
        queue: &wgpu::Queue,
        mesh: &mut GpuMesh,
        positions: &[Vec3],
        triangles: Option<&[[u32; 3]]>,
    ) {
        if let Some(t) = triangles {
            mesh.triangles = orient_consistently(t);
            queue.write_buffer(&mesh.indices, 0, bytemuck::cast_slice(&mesh.triangles));
            mesh.index_count = (mesh.triangles.len() * 3) as u32;
        }
        let normals = vertex_normals(positions, &mesh.triangles);
        let verts: Vec<Vertex> = positions
            .iter()
            .zip(&normals)
            .map(|(p, n)| Vertex {
                position: p.to_array(),
                normal: n.to_array(),
            })
            .collect();
        queue.write_buffer(&mesh.vertices, 0, bytemuck::cast_slice(&verts));
    }

    /// Clears `target` and draws `meshes`. Submits its own command buffer.
    pub fn render(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        target: &RenderTarget,
        view_proj: Mat4,
        meshes: &[&GpuMesh],
    ) {
        for m in meshes {
            let u = Uniforms {
                view_proj: view_proj.to_cols_array_2d(),
                color: [m.color[0], m.color[1], m.color[2], 1.0],
            };
            queue.write_buffer(&m.uniforms, 0, bytemuck::bytes_of(&u));
        }
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("meshes"),
        });
        {
            let [r, g, b, a] = CLEAR_COLOR;
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("mesh pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &target.color_view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color { r, g, b, a }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &target.depth_view,
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
            pass.set_pipeline(&self.pipeline);
            for m in meshes.iter().filter(|m| m.index_count > 0) {
                pass.set_bind_group(0, &m.bind_group, &[]);
                pass.set_vertex_buffer(0, m.vertices.slice(..));
                pass.set_index_buffer(m.indices.slice(..), wgpu::IndexFormat::Uint32);
                pass.draw_indexed(0..m.index_count, 0, 0..1);
            }
        }
        queue.submit([encoder.finish()]);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_seam_between_pieces_wound_opposite_ways_still_gets_a_normal() {
        // Two flat squares side by side sharing an edge, the second wound the other way.
        let positions = [
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::new(1.0, 0.0, 0.0),
            Vec3::new(1.0, 1.0, 0.0),
            Vec3::new(0.0, 1.0, 0.0),
            Vec3::new(2.0, 0.0, 0.0),
            Vec3::new(2.0, 1.0, 0.0),
        ];
        let triangles = [[0, 1, 2], [0, 2, 3], [1, 5, 4], [1, 2, 5]];
        // Summed as they come, the normals disagree from vertex to vertex, so the shading
        // between them passes through nothing.
        let raw = vertex_normals(&positions, &triangles);
        assert!(
            raw.iter().any(|v| v.z > 0.5) && raw.iter().any(|v| v.z < -0.5),
            "{raw:?}"
        );
        // Oriented first, every face points the same way and every normal is clean.
        let oriented = orient_consistently(&triangles);
        let face = |t: &[u32; 3]| {
            let [a, b, c] = t.map(|k| k as usize);
            (positions[b] - positions[a]).cross(positions[c] - positions[a])
        };
        assert!(oriented.iter().all(|t| face(t).z > 0.0), "{oriented:?}");
        assert_eq!(
            &oriented[..2],
            &triangles[..2],
            "the first run's way is kept"
        );
        for (k, v) in vertex_normals(&positions, &oriented).iter().enumerate() {
            assert!(
                (v.length() - 1.0).abs() < 1e-6 && v.z > 0.999,
                "vertex {k}: {v}"
            );
        }
        // Two separate runs are each oriented on their own, and nothing is lost.
        let apart = [[0, 1, 2], [3, 5, 4]];
        assert_eq!(orient_consistently(&apart), apart);
    }
}
