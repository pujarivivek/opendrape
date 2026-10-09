use crate::{Piece, PieceId, Point2, Side, Units};
use serde::{Deserialize, Serialize};

/// Version of the project format written by this build. Bump it when the format changes, and
/// add a migration step in `opendrape-io`. Version 2 added seam allowances, notches, internal
/// lines, folds and twins (2026-10-09).
pub const SCHEMA_VERSION: u32 = 2;

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
    BadAllowance(PieceId),
    BadNotch(PieceId),
    BadLine(PieceId),
    BadFold(PieceId),
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
            Self::BadAllowance(id) => write!(f, "piece {} has an invalid seam allowance", id.0),
            Self::BadNotch(id) => write!(f, "piece {} has an invalid notch", id.0),
            Self::BadLine(id) => write!(f, "piece {} has an invalid internal line", id.0),
            Self::BadFold(id) => write!(f, "piece {} has an invalid fold line", id.0),
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
    /// The stored piece an id belongs to, and whether the id names that piece or its twin.
    pub fn owner(&self, id: PieceId) -> Option<(&Piece, Side)> {
        self.pieces.iter().find_map(|p| {
            if p.id == id {
                Some((p, Side::Master))
            } else if p.twin.as_ref().is_some_and(|t| t.id == id) {
                Some((p, Side::Twin))
            } else {
                None
            }
        })
    }
    pub fn owner_mut(&mut self, id: PieceId) -> Option<(&mut Piece, Side)> {
        self.pieces.iter_mut().find_map(|p| {
            if p.id == id {
                Some((p, Side::Master))
            } else if p.twin.as_ref().is_some_and(|t| t.id == id) {
                Some((p, Side::Twin))
            } else {
                None
            }
        })
    }
    /// The name shown for an id: the piece's, or its twin's.
    pub fn name_of(&self, id: PieceId) -> Option<&str> {
        match self.owner(id)? {
            (p, Side::Master) => Some(&p.name),
            (p, Side::Twin) => p.twin.as_ref().map(|t| t.name.as_str()),
        }
    }
    /// Gives `master` a mirror-image twin called `name`, placed by `offset` (see [`crate::Twin`]),
    /// and returns the twin's id. None when there is no such piece, or it is folded or already
    /// paired.
    pub fn add_twin(&mut self, master: PieceId, name: String, offset: Point2) -> Option<PieceId> {
        let id = PieceId(self.next_piece_id);
        let piece = self.piece_mut(master)?;
        if piece.twin.is_some() || piece.fold.is_some() {
            return None;
        }
        piece.twin = Some(crate::Twin { id, name, offset });
        self.next_piece_id = self.next_piece_id.saturating_add(1);
        Some(id)
    }
    /// Turns `master`'s twin into an ordinary piece with the twin's current shape, id and name.
    pub fn break_twin(&mut self, master: PieceId) -> Option<PieceId> {
        let piece = self.piece_mut(master)?;
        let twin = piece.twin_shape()?;
        piece.twin = None;
        let id = twin.id;
        self.pieces.push(twin);
        Some(id)
    }
    /// Removes the piece or twin with this id and returns its shape. Removing a piece that has
    /// a twin keeps the twin, as an ordinary piece.
    pub fn remove_piece(&mut self, id: PieceId) -> Option<Piece> {
        match self.owner(id)? {
            (_, Side::Twin) => {
                let (piece, _) = self.owner_mut(id)?;
                let shape = piece.twin_shape();
                piece.twin = None;
                shape
            }
            (piece, Side::Master) => {
                if piece.twin.is_some() {
                    self.break_twin(id);
                }
                let at = self.pieces.iter().position(|p| p.id == id)?;
                Some(self.pieces.remove(at))
            }
        }
    }
    /// Default name for the next new piece: "<prefix> <number>".
    pub fn next_piece_name(&self, prefix: &str) -> String {
        format!("{prefix} {}", self.next_piece_id)
    }
    /// At most [`MAX_PIECES`] pieces and [`MAX_TOTAL_VERTICES`] points in all (a twin counts
    /// as a piece with its own points), every piece valid, ids (pieces' and twins') unique and
    /// below the id counter, and the counter itself at most [`MAX_PIECE_ID`].
    pub fn check(&self) -> Result<(), ModelError> {
        let shapes = self.pieces.len() + self.pieces.iter().filter(|p| p.twin.is_some()).count();
        if shapes > MAX_PIECES {
            return Err(ModelError::TooManyPieces);
        }
        if self.next_piece_id > MAX_PIECE_ID {
            return Err(ModelError::IdCounterTooLarge);
        }
        let mut seen = std::collections::BTreeSet::new();
        let mut total_points = 0_usize;
        for p in &self.pieces {
            p.check()?;
            let copies = if p.twin.is_some() { 2 } else { 1 };
            // At most MAX_PIECES pieces of at most MAX_VERTICES_PER_PIECE points: no overflow.
            total_points += p.point_count() * copies;
            for id in std::iter::once(p.id).chain(p.twin.as_ref().map(|t| t.id)) {
                if !seen.insert(id) {
                    return Err(ModelError::DuplicateId(id));
                }
                if id.0 >= self.next_piece_id {
                    return Err(ModelError::IdCounterBehind(id));
                }
            }
        }
        if total_points > MAX_TOTAL_VERTICES {
            return Err(ModelError::TooManyPointsInProject);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{MAX_VERTICES_PER_PIECE, Point2, Side};

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
        let json = format!(r#"{{"schema_version":{SCHEMA_VERSION}}}"#);
        let pr: Project = serde_json::from_str(&json).unwrap();
        assert_eq!(pr, Project::new());
    }

    #[test]
    fn twins_get_ids_and_names_and_can_be_broken_off() {
        let mut pr = Project::new();
        let a = pr.add_piece(tri());
        let t = pr
            .add_twin(a, "T (mirror)".into(), Point2::new(50.0, 0.0))
            .unwrap();
        assert_eq!(t, PieceId(2));
        assert_eq!(
            pr.add_twin(a, "again".into(), Point2::new(0.0, 0.0)),
            None,
            "one twin each"
        );
        assert!(matches!(pr.owner(t), Some((p, Side::Twin)) if p.id == a));
        assert!(matches!(pr.owner(a), Some((_, Side::Master))));
        assert_eq!(pr.name_of(t), Some("T (mirror)"));
        assert!(pr.piece(t).is_none(), "piece() finds stored pieces only");
        assert_eq!(pr.check(), Ok(()));
        assert_eq!(pr.break_twin(a), Some(t));
        let broken = pr.piece(t).unwrap();
        assert_eq!(broken.vertices[1].pos, Point2::new(40.0, 0.0)); // (10,0) reflected, +50
        assert!(pr.piece(a).unwrap().twin.is_none());
        assert_eq!(pr.check(), Ok(()));
    }

    #[test]
    fn removing_a_paired_piece_keeps_its_twin() {
        let mut pr = Project::new();
        let a = pr.add_piece(tri());
        let t = pr.add_twin(a, "T".into(), Point2::new(50.0, 0.0)).unwrap();
        assert!(pr.remove_piece(a).is_some());
        assert!(pr.piece(t).is_some(), "the twin becomes an ordinary piece");
        let b = pr.add_piece(tri());
        let u = pr.add_twin(b, "U".into(), Point2::new(50.0, 0.0)).unwrap();
        assert!(pr.remove_piece(u).is_some());
        assert!(pr.piece(b).unwrap().twin.is_none() && pr.owner(u).is_none());
    }

    #[test]
    fn check_counts_twins_and_refuses_clashing_ids() {
        let mut pr = Project::new();
        let a = pr.add_piece(tri());
        pr.add_twin(a, "T".into(), Point2::new(50.0, 0.0));
        let mut clash = pr.clone();
        clash.pieces[0].twin.as_mut().unwrap().id = a;
        assert_eq!(clash.check(), Err(ModelError::DuplicateId(a)));
        let mut ahead = pr.clone();
        ahead.pieces[0].twin.as_mut().unwrap().id = PieceId(99);
        assert_eq!(ahead.check(), Err(ModelError::IdCounterBehind(PieceId(99))));
        let mut folded = pr.clone();
        folded.pieces[0].fold = Some(1);
        assert_eq!(folded.check(), Err(ModelError::BadFold(a)));
        assert_eq!(SCHEMA_VERSION, 2);
    }
}
