//! OpenDrape's document model: pattern pieces made of straight and curved edges, in
//! millimetres with y up. Pure data with no geometry or GPU dependencies, so it is easy to
//! save, compare and test.

mod piece;
mod project;
mod units;

pub use piece::{
    Edge, HandleEnd, MAX_COORDINATE_MM, MAX_NAME_CHARS, MAX_VERTICES_PER_PIECE, Piece, PieceId,
    Point2, Vertex, VertexKind,
};
pub use project::{
    MAX_PIECE_ID, MAX_PIECES, MAX_TOTAL_VERTICES, ModelError, Project, SCHEMA_VERSION,
};
pub use units::Units;
