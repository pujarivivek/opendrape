use crate::{
    Half, MAX_SEAMS, Piece, PieceId, Placement, Point2, Seam, SeamId, SeamSide, Side, Units,
};
use serde::{Deserialize, Serialize};

/// Version of the project format written by this build. Bump it when the format changes, and
/// add a migration step in `opendrape-io`. Version 2 added seam allowances, notches, internal
/// lines, folds and twins; version 3 added seams and 3D placements (2026-10-09).
pub const SCHEMA_VERSION: u32 = 3;

/// Most pieces a project may hold.
pub const MAX_PIECES: usize = 500;

/// Most points a project may hold across all its pieces (outline points, internal-line points
/// and notches), so that drawing and simulating it stays fast whatever a file contains.
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
    /// The seams the student sewed. Their mirror images are not stored (see [`Self::mirror_of`]).
    #[serde(default)]
    pub seams: Vec<Seam>,
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
    BadPlacement(PieceId),
    BadSeam(SeamId),
    TooManySeams,
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
            Self::BadPlacement(id) => write!(f, "the 3D placement of piece {} is invalid", id.0),
            Self::BadSeam(id) => write!(f, "seam {} is invalid", id.0),
            Self::TooManySeams => write!(f, "the project has too many seams"),
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
            seams: Vec::new(),
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
        piece.twin = Some(crate::Twin {
            id,
            name,
            offset,
            placement: None,
        });
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
    /// Removes the piece or twin with this id and returns its shape, with every seam sewn to it.
    /// Removing a piece that has a twin keeps the twin (and its seams), as an ordinary piece.
    pub fn remove_piece(&mut self, id: PieceId) -> Option<Piece> {
        self.owner(id)?;
        self.seams.retain(|s| !s.touches(id));
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
    /// The placement stored for a piece or twin (None when it has none of its own, or there
    /// is no such shape).
    pub fn placement_of(&self, id: PieceId) -> Option<Placement> {
        match self.owner(id)? {
            (p, Side::Master) => p.placement,
            (p, Side::Twin) => p.twin.as_ref()?.placement,
        }
    }

    /// Gives a piece or twin its own placement, or (None) takes it away. False when there is
    /// no such shape.
    pub fn set_placement(&mut self, id: PieceId, placement: Option<Placement>) -> bool {
        match self.owner_mut(id) {
            Some((p, Side::Master)) => p.placement = placement,
            Some((p, Side::Twin)) => match &mut p.twin {
                Some(t) => t.placement = placement,
                None => return false,
            },
            None => return false,
        }
        true
    }

    /// The stored seam with this id.
    pub fn seam(&self, id: SeamId) -> Option<&Seam> {
        self.seams.iter().find(|s| s.id == id)
    }

    pub fn seam_mut(&mut self, id: SeamId) -> Option<&mut Seam> {
        self.seams.iter_mut().find(|s| s.id == id)
    }

    /// Sews `a` to `b` (a's start meets b's start) under a fresh id, and returns the id. Ids are
    /// one more than the highest in use, so an id freed by deleting the last seam may come back.
    pub fn add_seam(&mut self, a: SeamSide, b: SeamSide) -> SeamId {
        let id = SeamId(self.seams.iter().map(|s| s.id.0).max().unwrap_or(0) + 1);
        self.seams.push(Seam { id, a, b });
        id
    }

    pub fn remove_seam(&mut self, id: SeamId) -> Option<Seam> {
        let at = self.seams.iter().position(|s| s.id == id)?;
        Some(self.seams.remove(at))
    }

    /// The mirror image of a seam side: the same edges on the other half of a cut-on-fold
    /// piece, or on the other member of a mirrored pair. None for any other piece.
    pub fn mirror_side(&self, side: &SeamSide) -> Option<SeamSide> {
        let (piece, owner) = self.owner(side.shape)?;
        if piece.fold.is_some() {
            Some(SeamSide {
                half: side.half.other(),
                ..*side
            })
        } else if let Some(t) = &piece.twin {
            let shape = match owner {
                Side::Master => t.id,
                Side::Twin => piece.id,
            };
            Some(SeamSide { shape, ..*side })
        } else {
            None
        }
    }

    /// The derived mirror image of `seam`, when both of its sides have one. A seam that is its
    /// own mirror image (a centre-back seam joining a piece to its twin, say) has none: its
    /// mirror would sew the very same edges.
    pub fn mirror_of(&self, seam: &Seam) -> Option<Seam> {
        let a = self.mirror_side(&seam.a)?;
        let b = self.mirror_side(&seam.b)?;
        if a.same_edges(&seam.b) && b.same_edges(&seam.a) {
            return None;
        }
        Some(Seam { id: seam.id, a, b })
    }

    /// Every seam to draw, mesh and stitch: each stored seam, followed by its mirror image when
    /// it has one (`true` marks a mirror image; it has its seam's id).
    pub fn all_seams(&self) -> Vec<(Seam, bool)> {
        let mut out = Vec::with_capacity(self.seams.len() * 2);
        for s in &self.seams {
            out.push((*s, false));
            if let Some(m) = self.mirror_of(s) {
                out.push((m, true));
            }
        }
        out
    }

    /// The seam (stored, or the stored seam whose mirror image it is) that sews stored edge
    /// `edge` of `half` of shape `shape`.
    pub fn seam_on(&self, shape: PieceId, half: Half, edge: usize) -> Option<SeamId> {
        let n = self.owner(shape)?.0.len();
        self.all_seams().into_iter().find_map(|(s, _)| {
            [s.a, s.b]
                .iter()
                .any(|side| side.shape == shape && side.half == half && side.covers(n, edge))
                .then_some(s.id)
        })
    }

    /// Keeps the seams right after stored edge `i` of piece `id` was split in two (the new
    /// edge is `i + 1`): a side on the piece or its twin that covers edge `i` now covers both
    /// parts, and every later edge number moves up by one.
    pub fn seams_after_split(&mut self, id: PieceId, i: usize) {
        let Some(piece) = self.piece(id) else { return };
        let n = piece.len() - 1; // edges before the split
        let shapes = [Some(id), piece.twin.as_ref().map(|t| t.id)];
        for seam in &mut self.seams {
            for side in [&mut seam.a, &mut seam.b] {
                if !shapes.contains(&Some(side.shape)) {
                    continue;
                }
                let had = side.covers(n, i);
                if side.first_edge > i {
                    side.first_edge += 1;
                }
                if had {
                    side.edges += 1;
                }
            }
        }
    }

    /// Keeps the seams right after vertex `i` of piece `id` was removed from an outline of
    /// `n` edges (its edges `i - 1` and `i` became one). A side covering both loses one edge; a
    /// side ending at the vertex loses its edge there, and a side left with none is deleted
    /// with its seam. If the piece lost its fold (the vertex was an end of the fold edge),
    /// every seam on its pale half goes too.
    pub fn seams_after_removal(&mut self, id: PieceId, i: usize, n: usize) {
        let Some(piece) = self.piece(id) else { return };
        let folded = piece.fold.is_some();
        let shapes = [Some(id), piece.twin.as_ref().map(|t| t.id)];
        let prev = (i + n - 1) % n;
        let shift = |e: usize| if e > i { e - 1 } else { e };
        self.seams.retain_mut(|seam| {
            let mut keep = true;
            for side in [&mut seam.a, &mut seam.b] {
                if !shapes.contains(&Some(side.shape)) {
                    continue;
                }
                if side.half == Half::Pale && !folded {
                    keep = false;
                    continue;
                }
                let (has_prev, has_i) = (side.covers(n, prev), side.covers(n, i));
                let first = match (has_prev, has_i) {
                    // A side round the whole outline starting at edge i: start at the join.
                    (true, true) if side.first_edge == i => prev,
                    // A side starting at edge i loses it: it starts at the next edge.
                    (false, true) => (i + 1) % n,
                    _ => side.first_edge,
                };
                if has_prev || has_i {
                    side.edges -= 1;
                }
                side.first_edge = shift(first);
                keep &= side.edges > 0;
            }
            keep
        });
    }

    /// Unfolds cut-on-fold piece `id` into `full`, its whole outline (`geom::unfolded`), and
    /// keeps its seams: the mirror images of seams on the piece become stored seams (on the
    /// pale half they were drawn on), and every side on the piece is renumbered for the whole
    /// outline. False (and nothing changed) when there is no such folded piece.
    pub fn unfold_piece(&mut self, id: PieceId, full: Piece) -> bool {
        let Some(piece) = self.piece(id) else {
            return false;
        };
        let Some(fold) = piece.fold else {
            return false;
        };
        let n = piece.len();
        let first = (fold + 1) % n;
        let mirrors: Vec<(SeamSide, SeamSide)> = self
            .seams
            .iter()
            .filter(|s| s.touches(id))
            .filter_map(|s| self.mirror_of(s))
            .map(|m| (m.a, m.b))
            .collect();
        for (a, b) in mirrors {
            self.add_seam(a, b);
        }
        for seam in &mut self.seams {
            for side in [&mut seam.a, &mut seam.b] {
                if side.shape != id {
                    continue;
                }
                // A side never covers the fold edge, so its edges are consecutive on the drawn
                // half: outline edges `low..low + edges` of the whole piece.
                let low = (side.first_edge + n - first) % n;
                match side.half {
                    Half::Drawn => side.first_edge = low,
                    Half::Pale => {
                        // The pale image of drawn edge m is edge 2n - 3 - m, running backwards.
                        side.first_edge = 2 * n - 3 - (low + side.edges - 1);
                        side.forward = !side.forward;
                        side.half = Half::Drawn;
                    }
                }
            }
        }
        if let Some(stored) = self.piece_mut(id) {
            *stored = Piece { id, ..full };
        }
        true
    }

    /// Takes the fold off piece `id`: its pale half goes, and so does every seam on it. The
    /// mirror images of its other seams simply disappear. False when it has no fold.
    pub fn remove_fold(&mut self, id: PieceId) -> bool {
        let Some(piece) = self.piece_mut(id) else {
            return false;
        };
        if piece.fold.take().is_none() {
            return false;
        }
        self.seams.retain(|s| {
            ![s.a, s.b]
                .iter()
                .any(|side| side.shape == id && side.half == Half::Pale)
        });
        true
    }

    /// Default name for the next new piece: "<prefix> <number>".
    pub fn next_piece_name(&self, prefix: &str) -> String {
        format!("{prefix} {}", self.next_piece_id)
    }
    /// At most [`MAX_PIECES`] pieces and [`MAX_TOTAL_VERTICES`] points in all (outline and
    /// internal-line points and notches; a twin counts as a piece with its own points), every
    /// piece valid, ids (pieces' and twins') unique and
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
        self.check_seams()
    }

    /// At most [`MAX_SEAMS`] seams with unique ids; every side on an existing shape and its
    /// edges (1 up to the outline's count, never the fold edge, the pale half only of a folded
    /// piece); and no edge sewn twice, mirror images included.
    fn check_seams(&self) -> Result<(), ModelError> {
        if self.seams.len() > MAX_SEAMS {
            return Err(ModelError::TooManySeams);
        }
        let mut ids = std::collections::BTreeSet::new();
        for s in &self.seams {
            if !ids.insert(s.id) || !self.side_fits(&s.a) || !self.side_fits(&s.b) {
                return Err(ModelError::BadSeam(s.id));
            }
        }
        let mut sewn = std::collections::BTreeSet::new();
        for (s, _) in self.all_seams() {
            for side in [s.a, s.b] {
                let n = self.owner(side.shape).map_or(0, |(p, _)| p.len());
                for e in side.stored_edges(n) {
                    if !sewn.insert((side.shape, side.half == Half::Pale, e)) {
                        return Err(ModelError::BadSeam(s.id));
                    }
                }
            }
        }
        Ok(())
    }

    fn side_fits(&self, side: &SeamSide) -> bool {
        let Some((piece, _)) = self.owner(side.shape) else {
            return false;
        };
        let n = piece.len();
        (1..=n).contains(&side.edges)
            && side.first_edge < n
            && (side.half == Half::Drawn || piece.fold.is_some())
            && piece.fold.is_none_or(|f| !side.covers(n, f))
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
    fn notches_count_toward_the_project_total() {
        let mut heavy = Project::new();
        // A piece with 1,997 notches has 2,000 points, a twin doubles that.
        let mut notched = tri();
        notched.notches = vec![crate::Notch::new(0, 1.0); MAX_VERTICES_PER_PIECE - 3];
        for _ in 0..MAX_TOTAL_VERTICES / MAX_VERTICES_PER_PIECE {
            heavy.add_piece(notched.clone());
        }
        assert_eq!(heavy.check(), Ok(()), "exactly at the limit");
        let id = heavy.add_piece(tri());
        assert_eq!(heavy.check(), Err(ModelError::TooManyPointsInProject));
        heavy.remove_piece(id);
        let first = heavy.pieces[0].id;
        heavy.pieces[0].notches.truncate(500);
        assert_eq!(heavy.check(), Ok(()));
        // A twin has the notches too: 9 pieces of 2,000 points, and one of 503 counted twice.
        heavy
            .add_twin(first, "Twin".into(), Point2::new(100.0, 0.0))
            .unwrap();
        assert_eq!(heavy.check(), Ok(()), "503 more points still fit");
        heavy.pieces[0].notches = vec![crate::Notch::new(0, 1.0); MAX_VERTICES_PER_PIECE - 3];
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
    }

    /// A folded front half (id 1: edges 0 bottom, 1 right, 2 top, 3 the fold on the left), a
    /// back (id 2) paired with its twin (id 3), and a plain pocket (id 4).
    fn sewing_room() -> Project {
        let mut pr = Project::new();
        let mut front = Piece::rectangle(PieceId(0), "Front", Point2::new(0.0, 0.0), 100.0, 200.0);
        front.fold = Some(3);
        pr.add_piece(front);
        let back = pr.add_piece(Piece::rectangle(
            PieceId(0),
            "Back",
            Point2::new(300.0, 0.0),
            100.0,
            200.0,
        ));
        pr.add_twin(back, "Back (mirror)".into(), Point2::new(900.0, 0.0))
            .unwrap();
        pr.add_piece(Piece::rectangle(
            PieceId(0),
            "Pocket",
            Point2::new(0.0, 400.0),
            80.0,
            80.0,
        ));
        assert_eq!(pr.check(), Ok(()));
        pr
    }

    fn side(shape: u32, half: Half, first_edge: usize, edges: usize, forward: bool) -> SeamSide {
        SeamSide::new(PieceId(shape), half, first_edge, edges, forward)
    }

    #[test]
    fn mirrors_are_derived_for_folds_and_pairs() {
        let mut pr = sewing_room();
        // Front's right edge to the back's left edge (edge 3 of a rectangle).
        let side_seam = pr.add_seam(
            side(1, Half::Drawn, 1, 1, true),
            side(2, Half::Drawn, 3, 1, false),
        );
        // The back's right edge to its twin's: its own mirror image.
        let centre_back = pr.add_seam(
            side(2, Half::Drawn, 1, 1, true),
            side(3, Half::Drawn, 1, 1, true),
        );
        // The pocket has no mirror image, so neither has its seam.
        let pocket = pr.add_seam(
            side(4, Half::Drawn, 0, 1, true),
            side(1, Half::Drawn, 0, 1, true),
        );
        assert_eq!(
            (side_seam, centre_back, pocket),
            (SeamId(1), SeamId(2), SeamId(3))
        );
        assert_eq!(pr.check(), Ok(()));
        let all = pr.all_seams();
        assert_eq!(all.len(), 4);
        assert_eq!(
            all[1],
            (
                Seam {
                    id: side_seam,
                    a: side(1, Half::Pale, 1, 1, true),
                    b: side(3, Half::Drawn, 3, 1, false)
                },
                true
            )
        );
        assert_eq!(
            (all[2].0.id, all[2].1, all[3].0.id),
            (centre_back, false, pocket)
        );
        // Sewn the other way round, the centre back is still its own mirror image.
        pr.seam_mut(centre_back).unwrap().b.forward = false;
        assert_eq!(pr.all_seams().len(), 4);
        assert_eq!(pr.check(), Ok(()));
        assert_eq!(
            pr.seam_on(PieceId(3), Half::Drawn, 3),
            Some(side_seam),
            "a mirror image belongs to its seam"
        );
        assert_eq!(pr.seam_on(PieceId(1), Half::Pale, 1), Some(side_seam));
        assert_eq!(pr.seam_on(PieceId(1), Half::Pale, 0), None);
    }

    #[test]
    fn seams_are_checked() {
        let base = sewing_room();
        let bad = |a: SeamSide, b: SeamSide| {
            let mut pr = base.clone();
            pr.add_seam(a, b);
            pr.check()
        };
        let ok = side(4, Half::Drawn, 0, 1, true);
        assert_eq!(
            bad(side(9, Half::Drawn, 0, 1, true), ok),
            Err(ModelError::BadSeam(SeamId(1))),
            "no such shape"
        );
        assert_eq!(
            bad(side(2, Half::Drawn, 0, 0, true), ok),
            Err(ModelError::BadSeam(SeamId(1))),
            "no edges"
        );
        assert_eq!(
            bad(side(2, Half::Drawn, 0, 5, true), ok),
            Err(ModelError::BadSeam(SeamId(1))),
            "more than the outline"
        );
        assert_eq!(
            bad(side(2, Half::Drawn, 4, 1, true), ok),
            Err(ModelError::BadSeam(SeamId(1))),
            "no such edge"
        );
        assert_eq!(
            bad(side(2, Half::Pale, 0, 1, true), ok),
            Err(ModelError::BadSeam(SeamId(1))),
            "pale half of an unfolded piece"
        );
        assert_eq!(
            bad(side(1, Half::Drawn, 2, 2, true), ok),
            Err(ModelError::BadSeam(SeamId(1))),
            "across the fold"
        );
        assert_eq!(
            bad(side(4, Half::Drawn, 3, 2, true), ok),
            Err(ModelError::BadSeam(SeamId(1))),
            "edge 0 twice"
        );
        // The first seam's mirror image already sews the pale half's edge 1: the second seam,
        // which sews it again, is the one refused.
        let mut pr = base.clone();
        pr.add_seam(
            side(1, Half::Drawn, 1, 1, true),
            side(2, Half::Drawn, 3, 1, false),
        );
        pr.add_seam(
            side(1, Half::Pale, 1, 1, true),
            side(4, Half::Drawn, 0, 1, true),
        );
        assert_eq!(pr.check(), Err(ModelError::BadSeam(SeamId(2))));
        let mut twice = base.clone();
        twice.add_seam(
            side(4, Half::Drawn, 0, 1, true),
            side(4, Half::Drawn, 2, 1, true),
        );
        twice.seams.push(Seam {
            id: SeamId(1),
            ..twice.seams[0]
        });
        twice.seams[1].a.first_edge = 1;
        twice.seams[1].b.first_edge = 3;
        assert_eq!(
            twice.check(),
            Err(ModelError::BadSeam(SeamId(1))),
            "an id used twice"
        );
        let mut many = base.clone();
        for k in 0..=MAX_SEAMS {
            many.seams.push(Seam {
                id: SeamId(k as u32 + 1),
                a: side(4, Half::Drawn, 0, 1, true),
                b: side(4, Half::Drawn, 1, 1, true),
            });
        }
        assert_eq!(many.check(), Err(ModelError::TooManySeams));
        assert_eq!(
            ModelError::BadSeam(SeamId(7)).to_string(),
            "seam 7 is invalid"
        );
    }

    #[test]
    fn splitting_a_sewn_edge_keeps_both_parts_sewn() {
        let mut pr = sewing_room();
        // The back's edges 3 and 0 (wrapping) to the pocket's edge 1; the twin's edge 2 to the
        // pocket's edge 2.
        let wrap = pr.add_seam(
            side(2, Half::Drawn, 3, 2, true),
            side(4, Half::Drawn, 1, 1, true),
        );
        let twin = pr.add_seam(
            side(3, Half::Drawn, 2, 1, false),
            side(4, Half::Drawn, 2, 1, true),
        );
        let back = pr.piece_mut(PieceId(2)).unwrap();
        back.split_edge_at(
            0,
            crate::Vertex::corner(Point2::new(350.0, 0.0)),
            crate::Edge::Line,
            crate::Edge::Line,
            50.0,
        );
        pr.seams_after_split(PieceId(2), 0);
        assert_eq!(
            pr.seam(wrap).unwrap().a,
            side(2, Half::Drawn, 4, 3, true),
            "edges 4, 0 and 1 now"
        );
        assert_eq!(
            pr.seam(twin).unwrap().a,
            side(3, Half::Drawn, 3, 1, false),
            "moved up, not grown"
        );
        assert_eq!(pr.check(), Ok(()));
    }

    #[test]
    fn removing_points_shrinks_sides_and_drops_empty_seams() {
        let corners: Vec<Point2> = (0..6)
            .map(|k| {
                let a = k as f64 / 6.0 * std::f64::consts::TAU;
                Point2::new(100.0 * a.cos(), 100.0 * a.sin())
            })
            .collect();
        let mut pr = Project::new();
        let hex = pr.add_piece(Piece::polygon(PieceId(0), "Hex", &corners));
        pr.add_piece(Piece::polygon(PieceId(0), "Other", &corners));
        let s1 = pr.add_seam(
            side(1, Half::Drawn, 0, 3, true),
            side(2, Half::Drawn, 0, 1, true),
        );
        let s2 = pr.add_seam(
            side(1, Half::Drawn, 3, 1, true),
            side(2, Half::Drawn, 3, 1, true),
        );
        let s3 = pr.add_seam(
            side(1, Half::Drawn, 4, 1, false),
            side(2, Half::Drawn, 4, 1, true),
        );
        let remove = |pr: &mut Project, i: usize| {
            let n = pr.piece(hex).unwrap().len();
            assert!(pr.piece_mut(hex).unwrap().remove_vertex(i, 1.0));
            pr.seams_after_removal(hex, i, n);
        };
        // Vertex 1 lies between edges 0 and 1 of the first seam's side: it shrinks by one.
        remove(&mut pr, 1);
        assert_eq!(pr.seam(s1).unwrap().a, side(1, Half::Drawn, 0, 2, true));
        assert_eq!(pr.seam(s2).unwrap().a, side(1, Half::Drawn, 2, 1, true));
        assert_eq!(pr.seam(s3).unwrap().a, side(1, Half::Drawn, 3, 1, false));
        // Vertex 2 ends the first side (edge 1) and starts the second (edge 2): both lose an
        // edge, and the second, left with none, goes.
        remove(&mut pr, 2);
        assert_eq!(pr.seam(s1).unwrap().a, side(1, Half::Drawn, 0, 1, true));
        assert!(pr.seam(s2).is_none());
        assert_eq!(pr.seam(s3).unwrap().a, side(1, Half::Drawn, 2, 1, false));
        assert_eq!(pr.check(), Ok(()));
    }

    #[test]
    fn removing_a_fold_end_drops_the_pale_seams() {
        let mut pr = sewing_room();
        let drawn = pr.add_seam(
            side(1, Half::Drawn, 1, 1, true),
            side(4, Half::Drawn, 0, 1, true),
        );
        let pale = pr.add_seam(
            side(1, Half::Pale, 0, 1, true),
            side(4, Half::Drawn, 2, 1, true),
        );
        let front = pr.piece_mut(PieceId(1)).unwrap();
        front.split_edge_at(
            1,
            crate::Vertex::corner(Point2::new(100.0, 100.0)),
            crate::Edge::Line,
            crate::Edge::Line,
            100.0,
        );
        pr.seams_after_split(PieceId(1), 1);
        assert_eq!(pr.seam(drawn).unwrap().a, side(1, Half::Drawn, 1, 2, true));
        // Vertex 4 (0,200) is an end of the fold edge: the fold goes, and the pale seam too.
        assert!(pr.piece_mut(PieceId(1)).unwrap().remove_vertex(4, 100.0));
        assert_eq!(pr.piece(PieceId(1)).unwrap().fold, None);
        pr.seams_after_removal(PieceId(1), 4, 5);
        assert!(pr.seam(pale).is_none());
        assert!(pr.seam(drawn).is_some());
        assert_eq!(pr.check(), Ok(()));
    }

    #[test]
    fn deleting_a_piece_or_twin_deletes_its_seams() {
        let mut pr = sewing_room();
        pr.add_seam(
            side(3, Half::Drawn, 0, 1, true),
            side(4, Half::Drawn, 0, 1, true),
        );
        pr.add_seam(
            side(2, Half::Drawn, 0, 1, true),
            side(4, Half::Drawn, 1, 1, true),
        );
        let kept = pr.add_seam(
            side(1, Half::Drawn, 0, 1, true),
            side(4, Half::Drawn, 2, 1, true),
        );
        let mut no_twin = pr.clone();
        no_twin.remove_piece(PieceId(3));
        assert_eq!(no_twin.seams.len(), 2);
        assert_eq!(no_twin.check(), Ok(()));
        // Deleting the back keeps its twin as a piece of its own, with the twin's seam.
        pr.remove_piece(PieceId(2));
        assert_eq!(
            pr.seams.iter().map(|s| s.a.shape).collect::<Vec<_>>(),
            vec![PieceId(3), PieceId(1)]
        );
        assert!(pr.seam(kept).is_some());
        assert_eq!(pr.check(), Ok(()));
    }

    #[test]
    fn unfolding_keeps_seams_and_stores_their_mirror_images() {
        let mut pr = sewing_room();
        let side_seam = pr.add_seam(
            side(1, Half::Drawn, 1, 1, true),
            side(2, Half::Drawn, 3, 1, false),
        );
        let pale = pr.add_seam(
            side(1, Half::Pale, 0, 1, true),
            side(4, Half::Drawn, 0, 2, true),
        );
        assert_eq!(pr.check(), Ok(()));
        // The whole front: (0,0) (100,0) (100,200) (0,200) (-100,200) (-100,0).
        let full = Piece::polygon(
            PieceId(1),
            "Front",
            &[
                Point2::new(0.0, 0.0),
                Point2::new(100.0, 0.0),
                Point2::new(100.0, 200.0),
                Point2::new(0.0, 200.0),
                Point2::new(-100.0, 200.0),
                Point2::new(-100.0, 0.0),
            ],
        );
        assert!(pr.unfold_piece(PieceId(1), full));
        assert_eq!(pr.piece(PieceId(1)).unwrap().fold, None);
        assert_eq!(
            pr.seam(side_seam).unwrap().a,
            side(1, Half::Drawn, 1, 1, true)
        );
        // The pale image of edge 0 is the whole piece's edge 5, which runs the other way.
        assert_eq!(pr.seam(pale).unwrap().a, side(1, Half::Drawn, 5, 1, false));
        // The side seam's mirror image is a seam of its own now, on edge 4.
        let stored = *pr.seams.last().unwrap();
        assert_eq!(
            (stored.id, stored.a, stored.b),
            (
                SeamId(3),
                side(1, Half::Drawn, 4, 1, false),
                side(3, Half::Drawn, 3, 1, false)
            )
        );
        assert_eq!(pr.all_seams().len(), 3, "and no longer derived");
        assert_eq!(pr.check(), Ok(()));
        let pocket = Piece::rectangle(PieceId(4), "x", Point2::new(0.0, 0.0), 1.0, 1.0);
        assert!(!pr.unfold_piece(PieceId(4), pocket), "not folded");
    }

    #[test]
    fn removing_a_fold_or_breaking_a_pair_drops_what_no_longer_exists() {
        let mut pr = sewing_room();
        let drawn = pr.add_seam(
            side(1, Half::Drawn, 1, 1, true),
            side(2, Half::Drawn, 3, 1, false),
        );
        let pale = pr.add_seam(
            side(1, Half::Pale, 0, 1, true),
            side(4, Half::Drawn, 0, 1, true),
        );
        let on_twin = pr.add_seam(
            side(3, Half::Drawn, 0, 1, true),
            side(4, Half::Drawn, 1, 1, true),
        );
        let mut unfolded = pr.clone();
        assert!(unfolded.remove_fold(PieceId(1)));
        assert!(unfolded.seam(pale).is_none() && unfolded.seam(drawn).is_some());
        assert_eq!(
            unfolded.all_seams().len(),
            2,
            "no mirror images without the fold"
        );
        assert_eq!(unfolded.check(), Ok(()));
        assert!(!unfolded.remove_fold(PieceId(1)));
        assert_eq!(pr.break_twin(PieceId(2)), Some(PieceId(3)));
        assert!(
            pr.seam(on_twin).is_some(),
            "the twin is a piece now, with its seam"
        );
        assert_eq!(
            pr.all_seams().len(),
            3,
            "without the pair, the side seam has no mirror image"
        );
        assert_eq!(pr.check(), Ok(()));
    }

    #[test]
    fn placements_belong_to_pieces_and_twins() {
        let mut pr = sewing_room();
        let p = Placement::at([0.0, 1.0, 0.4]);
        assert!(pr.set_placement(PieceId(2), Some(p)));
        assert!(pr.set_placement(PieceId(3), Some(Placement::at([0.0, 1.0, -0.4]))));
        assert_eq!(pr.placement_of(PieceId(2)), Some(p));
        assert_eq!(
            pr.pieces[1].twin.as_ref().unwrap().placement,
            Some(Placement::at([0.0, 1.0, -0.4]))
        );
        assert_eq!(pr.placement_of(PieceId(1)), None);
        assert!(!pr.set_placement(PieceId(99), Some(p)));
        assert_eq!(pr.check(), Ok(()));
        // Breaking the pair keeps the twin where it was placed.
        pr.break_twin(PieceId(2));
        assert_eq!(
            pr.piece(PieceId(3)).unwrap().placement,
            Some(Placement::at([0.0, 1.0, -0.4]))
        );
        assert_eq!(SCHEMA_VERSION, 3);
    }
}
