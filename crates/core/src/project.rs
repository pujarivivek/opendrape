use crate::{Piece, PieceId, Units};
use serde::{Deserialize, Serialize};

/// Version of the project format written by this build. Bump it when the format changes, and
/// add a migration step in `opendrape-io`.
pub const SCHEMA_VERSION: u32 = 1;

/// Most pieces a project may hold.
pub const MAX_PIECES: usize = 500;

/// Most points a project may hold across all its pieces, so that drawing and simulating it
/// stays fast whatever a file contains.
pub const MAX_TOTAL_VERTICES: usize = 20_000;

/// Highest value the piece id counter may reach. Far above anything a student draws; it only
/// stops a corrupt file from putting the counter at the end of its range.
pub const MAX_PIECE_ID: u32 = 1_000_000;

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
    OutOfRange(PieceId),
    DuplicateId(PieceId),
    IdCounterBehind(PieceId),
    TooManyPoints(PieceId),
    NameTooLong(PieceId),
    TooManyPieces,
    TooManyPointsInProject,
    IdCounterTooLarge,
}

impl std::fmt::Display for ModelError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::TooFewVertices(id) => write!(f, "piece {} has fewer than 3 points", id.0),
            Self::EdgeCountMismatch(id) => {
                write!(f, "piece {} has the wrong number of edges", id.0)
            }
            Self::NotFinite(id) => write!(f, "piece {} contains an invalid number", id.0),
            Self::OutOfRange(id) => write!(f, "piece {} is too far from the origin", id.0),
            Self::DuplicateId(id) => write!(f, "piece id {} is used twice", id.0),
            Self::IdCounterBehind(id) => write!(f, "piece id {} is ahead of the id counter", id.0),
            Self::TooManyPoints(id) => write!(f, "piece {} has too many points", id.0),
            Self::NameTooLong(id) => write!(f, "the name of piece {} is too long", id.0),
            Self::TooManyPieces => write!(f, "the project has too many pieces"),
            Self::TooManyPointsInProject => write!(f, "the project has too many points"),
            Self::IdCounterTooLarge => write!(f, "the piece id counter is too large"),
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
        // Saturating, so a counter loaded at the end of its range can't panic here (debug) or
        // wrap round to reused ids (release). `check` refuses a counter above `MAX_PIECE_ID`
        // long before that, so a checked project never gets near the saturation point.
        self.next_piece_id = self.next_piece_id.saturating_add(1);
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
    /// At most [`MAX_PIECES`] pieces and [`MAX_TOTAL_VERTICES`] points in all, every piece
    /// valid, ids unique and below the id counter, and the counter itself at most
    /// [`MAX_PIECE_ID`].
    pub fn check(&self) -> Result<(), ModelError> {
        if self.pieces.len() > MAX_PIECES {
            return Err(ModelError::TooManyPieces);
        }
        if self.next_piece_id > MAX_PIECE_ID {
            return Err(ModelError::IdCounterTooLarge);
        }
        let mut seen = std::collections::BTreeSet::new();
        let mut total_vertices = 0_usize;
        for p in &self.pieces {
            p.check()?;
            if !seen.insert(p.id) {
                return Err(ModelError::DuplicateId(p.id));
            }
            if p.id.0 >= self.next_piece_id {
                return Err(ModelError::IdCounterBehind(p.id));
            }
            // Each piece is at most `MAX_VERTICES_PER_PIECE` long and there are at most
            // `MAX_PIECES` of them, so this cannot overflow.
            total_vertices += p.len();
        }
        if total_vertices > MAX_TOTAL_VERTICES {
            return Err(ModelError::TooManyPointsInProject);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{MAX_VERTICES_PER_PIECE, Point2};

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
    fn check_limits_the_size_of_a_project() {
        let mut full = Project::new();
        for _ in 0..MAX_PIECES {
            full.add_piece(tri());
        }
        assert_eq!(full.check(), Ok(()));
        full.add_piece(tri());
        assert_eq!(full.check(), Err(ModelError::TooManyPieces));

        // 11 pieces of 2000 points are 22000 points: over the project total, though every
        // piece alone is fine.
        let big = |id: u32| {
            let n = MAX_VERTICES_PER_PIECE;
            let corners: Vec<Point2> = (0..n)
                .map(|k| {
                    let a = k as f64 / n as f64 * std::f64::consts::TAU;
                    Point2::new(1000.0 * a.cos(), 1000.0 * a.sin())
                })
                .collect();
            Piece::polygon(PieceId(id), "Big", &corners)
        };
        let mut heavy = Project::new();
        for _ in 0..MAX_TOTAL_VERTICES / MAX_VERTICES_PER_PIECE {
            heavy.add_piece(big(0));
        }
        assert_eq!(heavy.check(), Ok(()), "exactly at the limit");
        heavy.add_piece(tri());
        assert_eq!(heavy.check(), Err(ModelError::TooManyPointsInProject));
    }

    #[test]
    fn check_limits_the_id_counter() {
        let json = |next: u64| format!(r#"{{"schema_version":1,"next_piece_id":{next}}}"#);
        let at_limit: Project = serde_json::from_str(&json(MAX_PIECE_ID.into())).unwrap();
        assert_eq!(at_limit.check(), Ok(()));
        let over: Project = serde_json::from_str(&json(u64::from(MAX_PIECE_ID) + 1)).unwrap();
        assert_eq!(over.check(), Err(ModelError::IdCounterTooLarge));
        let end: Project = serde_json::from_str(&json(u32::MAX.into())).unwrap();
        assert_eq!(end.check(), Err(ModelError::IdCounterTooLarge));
    }

    #[test]
    fn adding_a_piece_at_the_end_of_the_id_range_does_not_panic() {
        let mut pr: Project = serde_json::from_str(&format!(
            r#"{{"schema_version":1,"next_piece_id":{}}}"#,
            u32::MAX
        ))
        .unwrap();
        pr.add_piece(tri());
        assert_eq!(pr.check(), Err(ModelError::IdCounterTooLarge));
    }

    #[test]
    fn the_new_errors_say_what_is_wrong() {
        assert_eq!(
            ModelError::TooManyPoints(PieceId(3)).to_string(),
            "piece 3 has too many points"
        );
    }

    #[test]
    fn missing_optional_fields_get_defaults() {
        let pr: Project = serde_json::from_str(r#"{"schema_version":1}"#).unwrap();
        assert_eq!(pr, Project::new());
    }
}
