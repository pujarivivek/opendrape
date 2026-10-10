//! Meshes the studio draws: the form, the cloth and the floor, each with its colour and kind
//! of surface.

use crate::mesh::{Vertex, vertex_normals};
use glam::Vec3;

/// What a mesh is made of, which decides how light falls on it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Material {
    /// Fabric: soft wrapped light, a sheen at grazing angles, a darker inside like a lining.
    Cloth,
    /// The dress form: matte.
    Form,
    /// The studio floor: matte, takes the contact shadow, fades into the backdrop.
    Floor,
}

impl Material {
    pub(crate) fn id(self) -> u32 {
        match self {
            Self::Cloth => 0,
            Self::Form => 1,
            Self::Floor => 2,
        }
    }
}

/// The per-draw data the shader reads (group 1).
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct DrawUniforms {
    pub colour: [f32; 4],
    pub material: [u32; 4],
}

/// What making a mesh on the GPU needs.
#[derive(Clone, Copy)]
pub(crate) struct MeshGpu<'a> {
    pub device: &'a wgpu::Device,
    pub queue: &'a wgpu::Queue,
    /// The layout of its per-draw uniforms (group 1).
    pub layout: &'a wgpu::BindGroupLayout,
}

/// A triangle mesh on the GPU whose vertices (and triangles) can be replaced every frame.
pub struct StudioMesh {
    pub(crate) id: u64,
    pub(crate) vertices: wgpu::Buffer,
    pub(crate) indices: wgpu::Buffer,
    vertex_capacity: usize,
    index_capacity: usize,
    pub(crate) index_count: u32,
    triangles: Vec<[u32; 3]>,
    pub(crate) uniforms: wgpu::Buffer,
    pub(crate) bind_group: wgpu::BindGroup,
    pub(crate) colour: [f32; 3],
    pub(crate) material: Material,
    /// The smallest box holding its vertices (min, max), for fitting the shadow map.
    pub(crate) bounds: (Vec3, Vec3),
}

impl StudioMesh {
    /// Draws the mesh in `linear` RGB from the next frame on.
    pub fn set_colour(&mut self, linear: [f32; 3]) {
        self.colour = linear;
    }

    pub fn colour(&self) -> [f32; 3] {
        self.colour
    }

    pub fn material(&self) -> Material {
        self.material
    }

    pub(crate) fn draw_uniforms(&self) -> DrawUniforms {
        let [r, g, b] = self.colour;
        DrawUniforms {
            colour: [r, g, b, 1.0],
            material: [self.material.id(), 0, 0, 0],
        }
    }

    pub(crate) fn new(
        MeshGpu {
            device,
            queue,
            layout,
        }: MeshGpu,
        id: u64,
        positions: &[Vec3],
        triangles: &[[u32; 3]],
        colour: [f32; 3],
        material: Material,
    ) -> Self {
        let buffer = |label: &str, size: usize, usage: wgpu::BufferUsages| {
            device.create_buffer(&wgpu::BufferDescriptor {
                label: Some(label),
                size: size.max(16) as u64,
                usage,
                mapped_at_creation: false,
            })
        };
        let vertices = buffer(
            "studio vertices",
            positions.len() * std::mem::size_of::<Vertex>(),
            wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
        );
        let indices = buffer(
            "studio indices",
            triangles.len() * 12,
            wgpu::BufferUsages::INDEX | wgpu::BufferUsages::COPY_DST,
        );
        let uniforms = buffer(
            "studio draw uniforms",
            std::mem::size_of::<DrawUniforms>(),
            wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        );
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("studio draw"),
            layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniforms.as_entire_binding(),
            }],
        });
        let mut mesh = Self {
            id,
            vertices,
            indices,
            vertex_capacity: positions.len(),
            index_capacity: triangles.len() * 3,
            index_count: 0,
            triangles: vec![],
            uniforms,
            bind_group,
            colour,
            material,
            bounds: (Vec3::ZERO, Vec3::ZERO),
        };
        mesh.upload(queue, positions, Some(triangles));
        mesh
    }

    /// Whether `positions` and `triangles` fit in the buffers this mesh has.
    pub(crate) fn fits(&self, positions: usize, triangles: Option<usize>) -> bool {
        let tris = triangles.unwrap_or(self.triangles.len());
        positions <= self.vertex_capacity && tris * 3 <= self.index_capacity
    }

    pub(crate) fn triangles(&self) -> &[[u32; 3]] {
        &self.triangles
    }

    /// Writes new vertex positions (normals recomputed) and, optionally, new triangles.
    pub(crate) fn upload(
        &mut self,
        queue: &wgpu::Queue,
        positions: &[Vec3],
        triangles: Option<&[[u32; 3]]>,
    ) {
        if let Some(t) = triangles {
            self.triangles = t.to_vec();
            queue.write_buffer(&self.indices, 0, bytemuck::cast_slice(t));
            self.index_count = (t.len() * 3) as u32;
        }
        self.bounds = positions.iter().fold(
            (Vec3::splat(f32::INFINITY), Vec3::splat(f32::NEG_INFINITY)),
            |(lo, hi), p| (lo.min(*p), hi.max(*p)),
        );
        let normals = vertex_normals(positions, &self.triangles);
        let verts: Vec<Vertex> = positions
            .iter()
            .zip(&normals)
            .map(|(p, n)| Vertex {
                position: p.to_array(),
                normal: n.to_array(),
            })
            .collect();
        queue.write_buffer(&self.vertices, 0, bytemuck::cast_slice(&verts));
    }
}

/// The studio floor: a flat disc of `radius` metres at y = 0, facing up.
pub(crate) fn floor_disc(radius: f32) -> (Vec<Vec3>, Vec<[u32; 3]>) {
    const SEGMENTS: u32 = 64;
    let mut positions = vec![Vec3::ZERO];
    positions.extend((0..SEGMENTS).map(|i| {
        let a = std::f32::consts::TAU * i as f32 / SEGMENTS as f32;
        Vec3::new(radius * a.cos(), 0.0, radius * a.sin())
    }));
    // Wound so the face normal points up.
    let triangles = (0..SEGMENTS)
        .map(|i| [0, 1 + (i + 1) % SEGMENTS, 1 + i])
        .collect();
    (positions, triangles)
}
