use crate::{Piece, PieceId, Units};
use serde::{Deserialize, Serialize};

/// Version of the project format written by this build. Bump it when the format changes, and
/// add a migration step in `opendrape-io`.
pub const SCHEMA_VERSION: u32 = 1;

/// Everything a student saves: their pattern pieces and settings.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Project {
    pub schema_version: u32,
    #[serde(default)]
    pub units: Units,
    #[serde(default)]
    pub pieces: Vec<Piece>,
    #[serde(default = "first_id")]
    next_piece_id: u32,
}

fn first_id() -> u32 {
    1
}

impl Default for Project {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ModelError {
    TooFewVertices(PieceId),
    EdgeCountMismatch(PieceId),
    NotFinite(PieceId),
    DuplicateId(PieceId),
    IdCounterBehind(PieceId),
}

impl std::fmt::Display for ModelError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::TooFewVertices(id) => write!(f, "piece {} has fewer than 3 points", id.0),
            Self::EdgeCountMismatch(id) => {
                write!(f, "piece {} has the wrong number of edges", id.0)
            }
            Self::NotFinite(id) => write!(f, "piece {} contains an invalid number", id.0),
            Self::DuplicateId(id) => write!(f, "piece id {} is used twice", id.0),
            Self::IdCounterBehind(id) => write!(f, "piece id {} is ahead of the id counter", id.0),
        }
    }
}

impl std::error::Error for ModelError {}

impl Project {
    pub fn new() -> Self {
        Self {
            schema_version: SCHEMA_VERSION,
            units: Units::Cm,
            pieces: Vec::new(),
            next_piece_id: first_id(),
        }
    }
    /// Adds `piece` under a fresh id (replacing its own) and returns that id.
    pub fn add_piece(&mut self, mut piece: Piece) -> PieceId {
        let id = PieceId(self.next_piece_id);
        self.next_piece_id += 1;
        piece.id = id;
        self.pieces.push(piece);
        id
    }
    pub fn piece(&self, id: PieceId) -> Option<&Piece> {
        self.pieces.iter().find(|p| p.id == id)
    }
    pub fn piece_mut(&mut self, id: PieceId) -> Option<&mut Piece> {
        self.pieces.iter_mut().find(|p| p.id == id)
    }
    pub fn remove_piece(&mut self, id: PieceId) -> Option<Piece> {
        let at = self.pieces.iter().position(|p| p.id == id)?;
        Some(self.pieces.remove(at))
    }
    /// Default name for the next new piece: "<prefix> <number>".
    pub fn next_piece_name(&self, prefix: &str) -> String {
        format!("{prefix} {}", self.next_piece_id)
    }
    /// Every piece valid, ids unique and below the id counter.
    pub fn check(&self) -> Result<(), ModelError> {
        let mut seen = std::collections::BTreeSet::new();
        for p in &self.pieces {
            p.check()?;
            if !seen.insert(p.id) {
                return Err(ModelError::DuplicateId(p.id));
            }
            if p.id.0 >= self.next_piece_id {
                return Err(ModelError::IdCounterBehind(p.id));
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Point2;

    fn tri() -> Piece {
        Piece::polygon(
            PieceId(0),
            "T",
            &[
                Point2::new(0.0, 0.0),
                Point2::new(10.0, 0.0),
                Point2::new(0.0, 10.0),
            ],
        )
    }

    #[test]
    fn pieces_get_fresh_ids_and_names() {
        let mut pr = Project::new();
        assert_eq!(pr.next_piece_name("Piece"), "Piece 1");
        let a = pr.add_piece(tri());
        let b = pr.add_piece(tri());
        assert_eq!((a, b), (PieceId(1), PieceId(2)));
        assert_eq!(pr.piece(b).unwrap().id, b);
        assert_eq!(pr.next_piece_name("Piece"), "Piece 3");
        assert!(pr.remove_piece(a).is_some());
        assert!(pr.piece(a).is_none());
        assert_eq!(pr.add_piece(tri()), PieceId(3), "ids are never reused");
        assert_eq!(pr.check(), Ok(()));
    }

    #[test]
    fn check_finds_duplicate_and_runaway_ids() {
        let mut pr = Project::new();
        pr.add_piece(tri());
        let mut dup = pr.clone();
        dup.pieces.push(dup.pieces[0].clone());
        assert_eq!(dup.check(), Err(ModelError::DuplicateId(PieceId(1))));
        let mut ahead = pr.clone();
        ahead.pieces[0].id = PieceId(99);
        assert_eq!(ahead.check(), Err(ModelError::IdCounterBehind(PieceId(99))));
    }

    #[test]
    fn missing_optional_fields_get_defaults() {
        let pr: Project = serde_json::from_str(r#"{"schema_version":1}"#).unwrap();
        assert_eq!(pr, Project::new());
    }
}
