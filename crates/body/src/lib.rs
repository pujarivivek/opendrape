//! The 3D shapes garments drape on: dress forms (rings resized to a size chart, see [`form`]),
//! the legacy CC0 MakeHuman body, a compact mesh file format and tape-measure measurements.

pub mod form;
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

impl BodyMesh {
    /// Average young adult female in A-pose, from CC0 MakeHuman data (see ASSETS.md).
    pub fn female_average() -> Self {
        read_odb(include_bytes!("../../../assets/body/female_average.odb"))
            .expect("bundled body asset is valid")
    }
}
