//! OpenDrape's document model: pattern pieces made of straight and curved edges, in
//! millimetres with y up. Pure data with no geometry or GPU dependencies, so it is easy to
//! save, compare and test.

mod piece;
mod project;
mod seam;
mod units;

pub use piece::{
    DEFAULT_ALLOWANCE_MM, Edge, EdgeProps, HEM_ALLOWANCE_MM, HandleEnd, InternalLine, LineKind,
    MAX_ALLOWANCE_MM, MAX_COORDINATE_MM, MAX_NAME_CHARS, MAX_VERTICES_PER_PIECE, Notch, NotchStyle,
    Piece, PieceId, Point2, Side, Twin, Vertex, VertexKind,
};
pub use project::{
    MAX_PIECE_ID, MAX_PIECES, MAX_TOTAL_VERTICES, ModelError, Project, SCHEMA_VERSION,
};
pub use seam::{Half, MAX_SEAMS, Seam, SeamId, SeamSide};
pub use units::Units;
