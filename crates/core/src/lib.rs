//! OpenDrape's document model: pattern pieces made of straight and curved edges, in
//! millimetres with y up. Pure data with no geometry or GPU dependencies, so it is easy to
//! save, compare and test.

mod measure;
mod piece;
mod placement;
mod project;
mod seam;
mod units;

pub use piece::{
    DEFAULT_ALLOWANCE_MM, Edge, EdgeProps, HEM_ALLOWANCE_MM, HandleEnd, InternalLine, LineKind,
    MAX_ALLOWANCE_MM, MAX_COORDINATE_MM, MAX_NAME_CHARS, MAX_VERTICES_PER_PIECE, Notch, NotchStyle,
    Piece, PieceId, Point2, Side, Twin, Vertex, VertexKind,
};
pub use placement::{MAX_CURVE_M, MAX_PLACEMENT_M, MIN_CURVE_M, Placement};
pub use project::{
    MAX_PIECE_ID, MAX_PIECES, MAX_TOTAL_VERTICES, ModelError, Project, SCHEMA_VERSION,
};
pub use seam::{
    Half, MAX_SEAM_ID, MAX_SEAMS, MIN_SIDE_MM, OutlinePos, Seam, SeamId, SeamSide, Span,
};
pub use units::Units;
