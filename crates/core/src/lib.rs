//! OpenDrape's document model: pattern pieces made of straight and curved edges, in
//! millimetres with y up. Pure data with no geometry or GPU dependencies, so it is easy to
//! save, compare and test.

mod piece;
mod project;
mod units;

pub use piece::{Edge, HandleEnd, Piece, PieceId, Point2, Vertex, VertexKind};
pub use project::{ModelError, Project, SCHEMA_VERSION};
pub use units::Units;
