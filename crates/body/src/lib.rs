//! The 3D body (avatar): meshes derived from CC0 MakeHuman data, a compact file format,
//! and tape-measure style body measurements.

mod format;
mod measure;

pub use format::{OdbError, read_odb, write_odb};
pub use measure::{boundary_edge_count, girth_at, height};

/// A static triangle mesh in metres, Y up, facing +Z, feet at y = 0.
#[derive(Clone, Debug, PartialEq)]
pub struct BodyMesh {
    pub positions: Vec<glam::Vec3>,
    pub triangles: Vec<[u32; 3]>,
}
