//! The 2D pattern window: drawing and editing pattern pieces.

mod document;
mod view;

pub use document::{Document, UNDO_LIMIT};
pub use view::View;
