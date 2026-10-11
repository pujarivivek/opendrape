//! The 3D shapes garments drape on: dress forms (rings resized to a size chart, see [`form`]),
//! and tape-measure measurements.

pub mod form;
mod measure;

pub use measure::{boundary_edge_count, girth_at, height};

/// A static triangle mesh in metres, Y up, facing +Z, feet at y = 0.
#[derive(Clone, Debug, PartialEq)]
pub struct BodyMesh {
    pub positions: Vec<glam::Vec3>,
    pub triangles: Vec<[u32; 3]>,
}
