use crate::seam::spans_overlap;
use crate::{
    Half, MAX_PINS, MAX_PLACEMENT_M, MAX_SEAM_ID, MAX_SEAMS, MIN_SIDE_MM, OutlinePos, PIN_SLACK_MM,
    Piece, PieceId, Pin, Placement, Point2, Seam, SeamId, SeamSide, Side, Span, Units, measure,
};
use serde::{Deserialize, Serialize};

/// Version of the project format written by this build. Bump it when the format changes, and
/// add a migration step in `opendrape-io`. Version 2 added seam allowances, notches, internal
/// lines, folds and twins; version 3 added seams and 3D placements (2026-10-09); version 4 made
/// seam sides run between any two points of an outline (2026-10-10).
pub const SCHEMA_VERSION: u32 = 4;

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
    /// Spots of fabric held in place while it drapes.
    #[serde(default)]
    pub pins: Vec<Pin>,
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
    /// The pin at this index in [`Project::pins`].
    BadPin(usize),
    TooManyPins,
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
            Self::BadPin(k) => write!(f, "pin {} is invalid", k + 1),
            Self::TooManyPins => write!(f, "the project has too many pins"),
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
            pins: Vec::new(),
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
    /// A twin without a placement of its own was showing its piece's placement mirrored: it
    /// keeps that as its own, so it stays where it was. Its pins stay on the same spots: they
    /// were kept where its piece shows them, and are now kept where it shows them itself.
    pub fn break_twin(&mut self, master: PieceId) -> Option<PieceId> {
        let piece = self.piece_mut(master)?;
        let offset = piece.twin.as_ref()?.offset;
        let mut twin = piece.twin_shape()?;
        twin.placement = twin
            .placement
            .or_else(|| piece.placement.map(|p| p.mirrored()));
        piece.twin = None;
        let id = twin.id;
        self.pieces.push(twin);
        for pin in self.pins.iter_mut().filter(|p| p.shape == id) {
            pin.at = Point2::new(offset.x - pin.at.x, pin.at.y + offset.y);
        }
        Some(id)
    }
    /// Removes the piece or twin with this id and returns its shape, with every seam sewn to it
    /// and every pin on it.
    /// Removing a piece that has a twin keeps the twin (and its seams), as an ordinary piece
    /// that stays where it was (see [`Self::break_twin`]).
    pub fn remove_piece(&mut self, id: PieceId) -> Option<Piece> {
        self.owner(id)?;
        self.seams.retain(|s| !s.touches(id));
        self.pins.retain(|p| p.shape != id);
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
    /// A project whose highest id is already at the end of the range (`check` refuses any above
    /// [`MAX_SEAM_ID`]) takes the lowest id not in use instead, so adding never overflows or
    /// repeats an id.
    pub fn add_seam(&mut self, a: SeamSide, b: SeamSide) -> SeamId {
        let highest = self.seams.iter().map(|s| s.id.0).max().unwrap_or(0);
        let id = match highest.checked_add(1) {
            Some(next) if next <= MAX_SEAM_ID => SeamId(next),
            _ => {
                let used: std::collections::BTreeSet<u32> =
                    self.seams.iter().map(|s| s.id.0).collect();
                SeamId((1..).find(|n| !used.contains(n)).unwrap_or(1))
            }
        };
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
        if a.same_part(&seam.b) && b.same_part(&seam.a) {
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

    /// The seam (stored, or the stored seam whose mirror image it is) that sews any part of
    /// stored edge `edge` of `half` of shape `shape`.
    pub fn seam_on(&self, shape: PieceId, half: Half, edge: usize) -> Option<SeamId> {
        let n = self.owner(shape)?.0.len();
        self.all_seams().into_iter().find_map(|(s, _)| {
            [s.a, s.b]
                .iter()
                .any(|side| side.shape == shape && side.half == half && side.covers(n, edge))
                .then_some(s.id)
        })
    }

    /// How long (mm) a side is, as [`Self::check`] measures it; None when its shape is missing
    /// or it covers nothing.
    pub fn side_length(&self, side: &SeamSide) -> Option<f64> {
        let (piece, _) = self.owner(side.shape)?;
        let spans = side.spans(piece.len());
        (!spans.is_empty()).then(|| {
            spans
                .iter()
                .map(|s| (s.t1 - s.t0) * measure::edge_length(piece, s.edge))
                .sum()
        })
    }

    /// Keeps the seams right after stored edge `i` of piece `id` was split in two (the new edge
    /// is `i + 1`) at fraction `s` of its length: a point of a side on the piece or its twin
    /// that was on edge `i` is now on the part it lies in, and every later edge number moves up
    /// by one. A side end exactly at the split stays on the part the side covers.
    pub fn seams_after_split(&mut self, id: PieceId, i: usize, s: f64) {
        let Some(piece) = self.piece(id) else { return };
        let s = s.clamp(1e-9, 1.0 - 1e-9);
        let shapes = [Some(id), piece.twin.as_ref().map(|t| t.id)];
        // `below`: the side covers the stretch just before the point (its end, running the
        // stored way; its start, running the other way).
        let moved = |p: OutlinePos, below: bool| {
            if p.edge > i {
                OutlinePos::new(p.edge + 1, p.t)
            } else if p.edge < i {
                p
            } else if p.t < s || (p.t == s && below) {
                OutlinePos::new(i, p.t / s)
            } else {
                OutlinePos::new(i + 1, (p.t - s) / (1.0 - s))
            }
        };
        for seam in &mut self.seams {
            for side in [&mut seam.a, &mut seam.b] {
                if shapes.contains(&Some(side.shape)) {
                    side.from = moved(side.from, !side.forward);
                    side.to = moved(side.to, side.forward);
                }
            }
        }
        self.drop_broken();
    }

    /// Keeps the seams right after vertex `i` of piece `id` was removed from an outline of `n`
    /// edges: its edges `i - 1` and `i` became one, the first `f` of its length being the old
    /// edge `i - 1`. A point of a side on either is put where that part of the joined edge is,
    /// and later edge numbers move down by one. If the piece lost its fold (the vertex was an
    /// end of the fold edge), every seam on its pale half goes. A side left 1 mm long or less
    /// deletes its seam.
    pub fn seams_after_removal(&mut self, id: PieceId, i: usize, n: usize, f: f64) {
        let Some(piece) = self.piece(id) else { return };
        let folded = piece.fold.is_some();
        let shapes = [Some(id), piece.twin.as_ref().map(|t| t.id)];
        let prev = (i + n - 1) % n;
        let joined = |p: OutlinePos| {
            let (edge, t) = if p.edge == prev {
                (prev, p.t * f)
            } else if p.edge == i {
                (prev, if p.t == 1.0 { 1.0 } else { f + p.t * (1.0 - f) })
            } else {
                (p.edge, p.t)
            };
            OutlinePos::new(if edge > i { edge - 1 } else { edge }, t)
        };
        self.seams.retain_mut(|seam| {
            let mut keep = true;
            for side in [&mut seam.a, &mut seam.b] {
                if !shapes.contains(&Some(side.shape)) {
                    continue;
                }
                keep &= side.half == Half::Drawn || folded;
                side.from = joined(side.from);
                side.to = joined(side.to);
            }
            keep
        });
        self.drop_broken();
    }

    /// Deletes what an edit left unusable: every seam with a side 1 mm long or less
    /// ([`MIN_SIDE_MM`]), or covering nothing (a side whose ends are not on its shape's edges is
    /// left for [`Self::check`] to refuse); and every pin on a shape or half that is gone, or
    /// more than [`PIN_SLACK_MM`] outside its piece.
    pub fn drop_broken(&mut self) {
        let pins = std::mem::take(&mut self.pins);
        self.pins = pins
            .into_iter()
            .filter(|pin| {
                self.owner(pin.shape).is_some_and(|(piece, _)| {
                    (pin.half == Half::Drawn || piece.fold.is_some())
                        && measure::distance_outside(piece, pin.at) <= PIN_SLACK_MM
                })
            })
            .collect();
        let too_short = |side: &SeamSide| {
            self.owner(side.shape).is_some_and(|(piece, _)| {
                let n = piece.len();
                [side.from, side.to]
                    .iter()
                    .all(|end| end.edge < n && end.t.is_finite())
                    && self.side_length(side).unwrap_or(0.0) <= MIN_SIDE_MM
            })
        };
        let short: Vec<SeamId> = self
            .seams
            .iter()
            .filter(|s| too_short(&s.a) || too_short(&s.b))
            .map(|s| s.id)
            .collect();
        self.seams.retain(|s| !short.contains(&s.id));
    }

    /// Unfolds cut-on-fold piece `id` into `full`, its whole outline (`geom::unfolded`), and
    /// keeps its seams: the mirror images of seams on the piece become stored seams (on the
    /// pale half they were drawn on), and every side on the piece is renumbered for the whole
    /// outline. Pins on the pale half move to where the whole piece has them. False (and
    /// nothing changed) when there is no such folded piece.
    pub fn unfold_piece(&mut self, id: PieceId, full: Piece) -> bool {
        let Some(piece) = self.piece(id) else {
            return false;
        };
        let Some(fold) = piece.fold else {
            return false;
        };
        let n = piece.len();
        let first = (fold + 1) % n;
        let (near, far) = piece.edge_ends(fold);
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
        // Stored edge e is whole-piece edge m on the drawn half, and 2n - 3 - m (running the
        // other way) on the pale half.
        let renumber = |p: OutlinePos, half: Half| {
            let m = (p.edge + n - first) % n;
            match half {
                Half::Drawn => OutlinePos::new(m, p.t),
                Half::Pale => OutlinePos::new(2 * n - 3 - m, 1.0 - p.t),
            }
        };
        for seam in &mut self.seams {
            for side in [&mut seam.a, &mut seam.b] {
                if side.shape != id {
                    continue;
                }
                side.from = renumber(side.from, side.half);
                side.to = renumber(side.to, side.half);
                if side.half == Half::Pale {
                    side.forward = !side.forward;
                    side.half = Half::Drawn;
                }
            }
        }
        // A pin on the pale half was kept as its mirror image: the whole piece has the spot.
        for pin in self.pins.iter_mut() {
            if pin.shape == id && pin.half == Half::Pale {
                pin.at = reflect_across(pin.at, near, far);
                pin.half = Half::Drawn;
            }
        }
        if let Some(stored) = self.piece_mut(id) {
            *stored = Piece { id, ..full };
        }
        true
    }

    /// Takes the fold off piece `id`: its pale half goes, and so does every seam and pin on it.
    /// The mirror images of its other seams simply disappear. False when it has no fold.
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
        self.pins
            .retain(|p| !(p.shape == id && p.half == Half::Pale));
        true
    }

    /// Moves the pins on stored piece `id` and on its twin by `d` (mm), for when the piece has
    /// been moved by `d` on the pattern table: its twin stays where it is, but its pins are
    /// kept where the piece shows them, so they move with it too.
    pub fn move_pins(&mut self, id: PieceId, d: Point2) {
        let twin = self.piece(id).and_then(|p| p.twin.as_ref()).map(|t| t.id);
        for pin in &mut self.pins {
            if pin.shape == id || Some(pin.shape) == twin {
                pin.at = pin.at + d;
            }
        }
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
        self.check_seams()?;
        self.check_pins()
    }

    /// At most [`MAX_PINS`] pins, each on a shape (and half) that exists, within
    /// [`PIN_SLACK_MM`] of its piece, and held at a target of finite numbers within
    /// [`MAX_PLACEMENT_M`] of the origin.
    fn check_pins(&self) -> Result<(), ModelError> {
        if self.pins.len() > MAX_PINS {
            return Err(ModelError::TooManyPins);
        }
        for (k, pin) in self.pins.iter().enumerate() {
            let on_piece = self.owner(pin.shape).is_some_and(|(piece, _)| {
                (pin.half == Half::Drawn || piece.fold.is_some())
                    && pin.at.is_finite()
                    && measure::distance_outside(piece, pin.at) <= PIN_SLACK_MM
            });
            let target_ok = pin.target.iter().all(|v| v.is_finite())
                && pin.target.iter().map(|v| v * v).sum::<f64>().sqrt() <= MAX_PLACEMENT_M;
            if !on_piece || !target_ok {
                return Err(ModelError::BadPin(k));
            }
        }
        Ok(())
    }

    /// At most [`MAX_SEAMS`] seams with unique ids of at most [`MAX_SEAM_ID`]; every side fits
    /// its shape (see [`Self::side_fits`]); and no two sides, mirror images included, share more
    /// than a point of outline.
    fn check_seams(&self) -> Result<(), ModelError> {
        if self.seams.len() > MAX_SEAMS {
            return Err(ModelError::TooManySeams);
        }
        let mut ids = std::collections::BTreeSet::new();
        for s in &self.seams {
            if s.id.0 > MAX_SEAM_ID
                || !ids.insert(s.id)
                || !self.side_fits(&s.a)
                || !self.side_fits(&s.b)
            {
                return Err(ModelError::BadSeam(s.id));
            }
        }
        let mut sewn: std::collections::BTreeMap<(PieceId, bool, usize), Vec<Span>> =
            std::collections::BTreeMap::new();
        for (s, _) in self.all_seams() {
            for side in [s.a, s.b] {
                let n = self.owner(side.shape).map_or(0, |(p, _)| p.len());
                for span in side.spans(n) {
                    let on = sewn
                        .entry((side.shape, side.half == Half::Pale, span.edge))
                        .or_default();
                    if spans_overlap(on, &[span]) {
                        return Err(ModelError::BadSeam(s.id));
                    }
                    on.push(span);
                }
            }
        }
        Ok(())
    }

    /// A side fits its shape when the shape exists, both ends are on its edges (`t` within
    /// 0..=1), it covers part of the outline, it is on the pale half only of a folded piece, it
    /// covers none of the fold edge, and it is longer than [`MIN_SIDE_MM`].
    fn side_fits(&self, side: &SeamSide) -> bool {
        let Some((piece, _)) = self.owner(side.shape) else {
            return false;
        };
        let n = piece.len();
        let on_outline =
            |p: OutlinePos| p.edge < n && p.t.is_finite() && (0.0..=1.0).contains(&p.t);
        if !on_outline(side.from)
            || !on_outline(side.to)
            || (side.half == Half::Pale && piece.fold.is_none())
        {
            return false;
        }
        let spans = side.spans(n);
        !spans.is_empty()
            && piece.fold.is_none_or(|f| spans.iter().all(|s| s.edge != f))
            && self.side_length(side).is_some_and(|l| l > MIN_SIDE_MM)
    }
}

/// Mirror image of `p` across the line through `a` and `b`.
fn reflect_across(p: Point2, a: Point2, b: Point2) -> Point2 {
    let d = b - a;
    let t = ((p.x - a.x) * d.x + (p.y - a.y) * d.y) / (d.x * d.x + d.y * d.y);
    (a + d * t) * 2.0 - p
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

    /// Whole stored edges `first` to `last` (wrapping) of shape `shape`.
    fn side(shape: u32, half: Half, first: usize, last: usize, forward: bool) -> SeamSide {
        SeamSide::edges(PieceId(shape), half, first, last, forward)
    }

    #[test]
    fn mirrors_are_derived_for_folds_and_pairs() {
        let mut pr = sewing_room();
        // Front's right edge to the back's left edge (edge 3 of a rectangle).
        let side_seam = pr.add_seam(
            side(1, Half::Drawn, 1, 1, true),
            side(2, Half::Drawn, 3, 3, false),
        );
        // The back's right edge to its twin's: its own mirror image.
        let centre_back = pr.add_seam(
            side(2, Half::Drawn, 1, 1, true),
            side(3, Half::Drawn, 1, 1, true),
        );
        // The pocket has no mirror image, so neither has its seam.
        let pocket = pr.add_seam(
            side(4, Half::Drawn, 0, 0, true),
            side(1, Half::Drawn, 0, 0, true),
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
                    b: side(3, Half::Drawn, 3, 3, false)
                },
                true
            )
        );
        assert_eq!(
            (all[2].0.id, all[2].1, all[3].0.id),
            (centre_back, false, pocket)
        );
        // Sewn the other way round, the centre back is still its own mirror image.
        let twisted = pr.seam(centre_back).unwrap().b.flipped();
        pr.seam_mut(centre_back).unwrap().b = twisted;
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
        let ok = side(4, Half::Drawn, 0, 0, true);
        assert_eq!(
            bad(side(9, Half::Drawn, 0, 0, true), ok),
            Err(ModelError::BadSeam(SeamId(1))),
            "no such shape"
        );
        let empty = SeamSide {
            to: OutlinePos::new(0, 0.0),
            ..side(2, Half::Drawn, 0, 0, true)
        };
        assert_eq!(
            bad(empty, ok),
            Err(ModelError::BadSeam(SeamId(1))),
            "ends where it starts"
        );
        assert_eq!(
            bad(side(2, Half::Drawn, 0, 4, true), ok),
            Err(ModelError::BadSeam(SeamId(1))),
            "ends past the last edge"
        );
        assert_eq!(
            bad(side(2, Half::Drawn, 4, 4, true), ok),
            Err(ModelError::BadSeam(SeamId(1))),
            "no such edge"
        );
        assert_eq!(
            bad(side(2, Half::Pale, 0, 0, true), ok),
            Err(ModelError::BadSeam(SeamId(1))),
            "pale half of an unfolded piece"
        );
        assert_eq!(
            bad(side(1, Half::Drawn, 2, 3, true), ok),
            Err(ModelError::BadSeam(SeamId(1))),
            "across the fold"
        );
        assert_eq!(
            bad(side(4, Half::Drawn, 3, 0, true), ok),
            Err(ModelError::BadSeam(SeamId(1))),
            "edge 0 twice"
        );
        // The first seam's mirror image already sews the pale half's edge 1: the second seam,
        // which sews it again, is the one refused.
        let mut pr = base.clone();
        pr.add_seam(
            side(1, Half::Drawn, 1, 1, true),
            side(2, Half::Drawn, 3, 3, false),
        );
        pr.add_seam(
            side(1, Half::Pale, 1, 1, true),
            side(4, Half::Drawn, 0, 0, true),
        );
        assert_eq!(pr.check(), Err(ModelError::BadSeam(SeamId(2))));
        let mut twice = base.clone();
        twice.add_seam(
            side(4, Half::Drawn, 0, 0, true),
            side(4, Half::Drawn, 2, 2, true),
        );
        twice.seams.push(Seam {
            id: SeamId(1),
            ..twice.seams[0]
        });
        twice.seams[1].a = side(4, Half::Drawn, 1, 1, true);
        twice.seams[1].b = side(4, Half::Drawn, 3, 3, true);
        assert_eq!(
            twice.check(),
            Err(ModelError::BadSeam(SeamId(1))),
            "an id used twice"
        );
        let mut many = base.clone();
        for k in 0..=MAX_SEAMS {
            many.seams.push(Seam {
                id: SeamId(k as u32 + 1),
                a: side(4, Half::Drawn, 0, 0, true),
                b: side(4, Half::Drawn, 1, 1, true),
            });
        }
        assert_eq!(many.check(), Err(ModelError::TooManySeams));
        assert_eq!(
            ModelError::BadSeam(SeamId(7)).to_string(),
            "seam 7 is invalid"
        );
    }

    /// The part of stored edge `edge` of shape `shape` from fraction `t0` to `t1`.
    fn part(shape: u32, half: Half, edge: usize, t0: f64, t1: f64) -> SeamSide {
        SeamSide {
            shape: PieceId(shape),
            half,
            from: OutlinePos::new(edge, t0),
            to: OutlinePos::new(edge, t1),
            forward: t1 >= t0,
        }
    }

    #[test]
    fn free_sides_are_checked() {
        let base = sewing_room();
        let with = |seams: &[(SeamSide, SeamSide)]| {
            let mut pr = base.clone();
            for (a, b) in seams {
                pr.add_seam(*a, *b);
            }
            pr.check()
        };
        // The pocket (id 4) is 80 mm square; the back (id 2) is 100 mm wide.
        let ok = part(2, Half::Drawn, 0, 0.0, 0.5);
        for (bad, why) in [
            (
                part(4, Half::Drawn, 0, 0.5, 1.5),
                "past the end of its edge",
            ),
            (part(4, Half::Drawn, 0, f64::NAN, 0.5), "not a number"),
            (part(4, Half::Drawn, 0, 0.5, 0.5125), "1 mm long"),
        ] {
            assert_eq!(
                with(&[(bad, ok)]),
                Err(ModelError::BadSeam(SeamId(1))),
                "{why}"
            );
        }
        assert_eq!(
            with(&[(part(4, Half::Drawn, 0, 0.5, 0.52), ok)]),
            Ok(()),
            "1.6 mm"
        );
        // Two seams on one edge may meet, not overlap.
        let (left, right) = (
            part(4, Half::Drawn, 0, 0.0, 0.5),
            part(4, Half::Drawn, 0, 0.5, 1.0),
        );
        let other = part(2, Half::Drawn, 0, 0.5, 1.0);
        assert_eq!(with(&[(left, ok), (right, other)]), Ok(()));
        let overlapping = part(4, Half::Drawn, 0, 0.4, 1.0);
        assert_eq!(
            with(&[(left, ok), (overlapping, other)]),
            Err(ModelError::BadSeam(SeamId(2)))
        );
        // The mirror image of half the front's right edge sews half of the pale one.
        let front_half = part(1, Half::Drawn, 1, 0.0, 0.5);
        let pale = part(1, Half::Pale, 1, 0.25, 0.75);
        assert_eq!(
            with(&[(front_half, ok), (pale, other)]),
            Err(ModelError::BadSeam(SeamId(2)))
        );
        assert_eq!(
            with(&[(front_half, ok), (part(1, Half::Pale, 1, 0.5, 1.0), other)]),
            Ok(())
        );
    }

    #[test]
    fn splitting_an_edge_moves_free_ends_onto_the_part_they_are_on() {
        let mut pr = sewing_room();
        let middle = pr.add_seam(
            part(4, Half::Drawn, 0, 0.25, 0.75),
            part(2, Half::Drawn, 0, 0.0, 0.5),
        );
        // Two sides meeting exactly where the edge is split, one of them running backwards.
        let below = pr.add_seam(
            part(2, Half::Drawn, 2, 0.5, 0.0),
            part(3, Half::Drawn, 2, 0.0, 0.5),
        );
        let above = pr.add_seam(
            part(2, Half::Drawn, 2, 0.5, 1.0),
            part(3, Half::Drawn, 2, 0.5, 1.0),
        );
        pr.piece_mut(PieceId(4)).unwrap().split_edge_at(
            0,
            crate::Vertex::corner(Point2::new(40.0, 400.0)),
            crate::Edge::Line,
            crate::Edge::Line,
            40.0,
        );
        pr.seams_after_split(PieceId(4), 0, 0.5);
        let p = OutlinePos::new;
        let a = pr.seam(middle).unwrap().a;
        assert_eq!(
            (a.from, a.to),
            (p(0, 0.5), p(1, 0.5)),
            "round the new corner"
        );
        pr.piece_mut(PieceId(2)).unwrap().split_edge_at(
            2,
            crate::Vertex::corner(Point2::new(350.0, 200.0)),
            crate::Edge::Line,
            crate::Edge::Line,
            50.0,
        );
        pr.seams_after_split(PieceId(2), 2, 0.5);
        let (b, c) = (pr.seam(below).unwrap().a, pr.seam(above).unwrap().a);
        assert_eq!((b.from, b.to), (p(2, 1.0), p(2, 0.0)), "the first part");
        assert_eq!((c.from, c.to), (p(3, 0.0), p(3, 1.0)), "the second part");
        // The twin's sides, on the same stored edge, moved with them.
        let (bt, ct) = (pr.seam(below).unwrap().b, pr.seam(above).unwrap().b);
        assert_eq!(
            (bt.from, bt.to, ct.from, ct.to),
            (p(2, 0.0), p(2, 1.0), p(3, 0.0), p(3, 1.0))
        );
        assert_eq!(pr.check(), Ok(()));
    }

    #[test]
    fn an_edit_that_leaves_a_side_1_mm_long_or_less_deletes_its_seam() {
        let mut pr = sewing_room();
        let short = pr.add_seam(
            part(4, Half::Drawn, 0, 0.1, 0.2),
            part(2, Half::Drawn, 0, 0.0, 0.5),
        );
        let kept = pr.add_seam(
            part(4, Half::Drawn, 2, 0.0, 1.0),
            part(2, Half::Drawn, 2, 0.0, 1.0),
        );
        assert_eq!(pr.side_length(&pr.seam(short).unwrap().a), Some(8.0));
        // The pocket's bottom edge pulled in to 5 mm: the first side would be 0.5 mm long.
        pr.piece_mut(PieceId(4))
            .unwrap()
            .move_vertex(1, Point2::new(5.0, 400.0));
        assert_eq!(
            pr.check(),
            Err(ModelError::BadSeam(short)),
            "refused if kept"
        );
        pr.drop_broken();
        assert!(pr.seam(short).is_none() && pr.seam(kept).is_some());
        assert_eq!(pr.check(), Ok(()));
    }

    #[test]
    fn a_side_that_covers_nothing_goes_with_its_seam() {
        let mut pr = sewing_room();
        let free = |edge, t, to_edge, to_t, forward| SeamSide {
            shape: PieceId(4),
            half: Half::Drawn,
            from: OutlinePos::new(edge, t),
            to: OutlinePos::new(to_edge, to_t),
            forward,
        };
        let other = side(2, Half::Drawn, 0, 0, true);
        // The corner between edges 1 and 2, named twice: no stretch at all, not the whole outline.
        let empty = pr.add_seam(free(1, 1.0, 2, 0.0, true), other);
        let kept = pr.add_seam(
            side(4, Half::Drawn, 3, 3, true),
            side(2, Half::Drawn, 2, 2, true),
        );
        assert_eq!(pr.check(), Err(ModelError::BadSeam(empty)));
        pr.drop_broken();
        assert!(pr.seam(empty).is_none() && pr.seam(kept).is_some());
        assert_eq!(pr.check(), Ok(()));
        // Nor does tidying one bring it back as a whole-outline side.
        let tidied = free(1, 1.0, 2, 0.0, true).tidy(4);
        assert_eq!(tidied.spans(4), vec![]);
    }

    #[test]
    fn splitting_a_sewn_edge_keeps_both_parts_sewn() {
        let mut pr = sewing_room();
        // The back's edges 3 and 0 (wrapping) to the pocket's edge 1; the twin's edge 2 to the
        // pocket's edge 2.
        let wrap = pr.add_seam(
            side(2, Half::Drawn, 3, 0, true),
            side(4, Half::Drawn, 1, 1, true),
        );
        let twin = pr.add_seam(
            side(3, Half::Drawn, 2, 2, false),
            side(4, Half::Drawn, 2, 2, true),
        );
        let back = pr.piece_mut(PieceId(2)).unwrap();
        back.split_edge_at(
            0,
            crate::Vertex::corner(Point2::new(350.0, 0.0)),
            crate::Edge::Line,
            crate::Edge::Line,
            50.0,
        );
        pr.seams_after_split(PieceId(2), 0, 0.5);
        assert_eq!(
            pr.seam(wrap).unwrap().a,
            side(2, Half::Drawn, 4, 1, true),
            "edges 4, 0 and 1 now"
        );
        assert_eq!(
            pr.seam(twin).unwrap().a,
            side(3, Half::Drawn, 3, 3, false),
            "moved up, not grown"
        );
        assert_eq!(pr.check(), Ok(()));
    }

    /// A regular hexagon (every edge 100 mm long) and a copy of it.
    fn two_hexagons() -> (Project, PieceId) {
        let corners: Vec<Point2> = (0..6)
            .map(|k| {
                let a = k as f64 / 6.0 * std::f64::consts::TAU;
                Point2::new(100.0 * a.cos(), 100.0 * a.sin())
            })
            .collect();
        let mut pr = Project::new();
        let hex = pr.add_piece(Piece::polygon(PieceId(0), "Hex", &corners));
        pr.add_piece(Piece::polygon(PieceId(0), "Other", &corners));
        (pr, hex)
    }

    #[test]
    fn removing_points_puts_side_ends_on_the_joined_edge() {
        let (mut pr, hex) = two_hexagons();
        let s1 = pr.add_seam(
            side(1, Half::Drawn, 0, 2, true),
            side(2, Half::Drawn, 0, 0, true),
        );
        let s2 = pr.add_seam(
            side(1, Half::Drawn, 3, 3, true),
            side(2, Half::Drawn, 3, 3, true),
        );
        let s3 = pr.add_seam(
            side(1, Half::Drawn, 4, 4, false),
            side(2, Half::Drawn, 4, 4, true),
        );
        let remove = |pr: &mut Project, i: usize| {
            let n = pr.piece(hex).unwrap().len();
            assert!(pr.piece_mut(hex).unwrap().remove_vertex(i, 100.0));
            pr.seams_after_removal(hex, i, n, 0.5);
        };
        // Vertex 1 lies inside the first side: it still runs from (the old) edge 0's start to
        // edge 2's end, now edges 0 and 1.
        remove(&mut pr, 1);
        assert_eq!(pr.seam(s1).unwrap().a, side(1, Half::Drawn, 0, 1, true));
        assert_eq!(pr.seam(s2).unwrap().a, side(1, Half::Drawn, 2, 2, true));
        assert_eq!(pr.seam(s3).unwrap().a, side(1, Half::Drawn, 3, 3, false));
        // Vertex 2 ends the first side and starts the second: each keeps its half of the joined
        // edge 1, and they meet halfway along it.
        remove(&mut pr, 2);
        let p = OutlinePos::new;
        let (a1, a2) = (pr.seam(s1).unwrap().a, pr.seam(s2).unwrap().a);
        assert_eq!((a1.from, a1.to), (p(0, 0.0), p(1, 0.5)));
        assert_eq!((a2.from, a2.to), (p(1, 0.5), p(1, 1.0)));
        assert_eq!(pr.seam(s3).unwrap().a, side(1, Half::Drawn, 2, 2, false));
        assert_eq!(pr.check(), Ok(()));
    }

    #[test]
    fn removing_a_fold_end_drops_the_pale_seams() {
        let mut pr = sewing_room();
        let drawn = pr.add_seam(
            side(1, Half::Drawn, 1, 1, true),
            side(4, Half::Drawn, 0, 0, true),
        );
        let pale = pr.add_seam(
            side(1, Half::Pale, 0, 0, true),
            side(4, Half::Drawn, 2, 2, true),
        );
        let front = pr.piece_mut(PieceId(1)).unwrap();
        front.split_edge_at(
            1,
            crate::Vertex::corner(Point2::new(100.0, 100.0)),
            crate::Edge::Line,
            crate::Edge::Line,
            100.0,
        );
        pr.seams_after_split(PieceId(1), 1, 0.5);
        assert_eq!(pr.seam(drawn).unwrap().a, side(1, Half::Drawn, 1, 2, true));
        // Vertex 4 (0,200) is an end of the fold edge: the fold goes, and the pale seam too.
        assert!(pr.piece_mut(PieceId(1)).unwrap().remove_vertex(4, 100.0));
        assert_eq!(pr.piece(PieceId(1)).unwrap().fold, None);
        pr.seams_after_removal(PieceId(1), 4, 5, 1.0 / 3.0);
        assert!(pr.seam(pale).is_none());
        assert!(pr.seam(drawn).is_some());
        assert_eq!(pr.check(), Ok(()));
    }

    #[test]
    fn deleting_a_piece_or_twin_deletes_its_seams() {
        let mut pr = sewing_room();
        pr.add_seam(
            side(3, Half::Drawn, 0, 0, true),
            side(4, Half::Drawn, 0, 0, true),
        );
        pr.add_seam(
            side(2, Half::Drawn, 0, 0, true),
            side(4, Half::Drawn, 1, 1, true),
        );
        let kept = pr.add_seam(
            side(1, Half::Drawn, 0, 0, true),
            side(4, Half::Drawn, 2, 2, true),
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
            side(2, Half::Drawn, 3, 3, false),
        );
        let pale = pr.add_seam(
            side(1, Half::Pale, 0, 0, true),
            side(4, Half::Drawn, 0, 1, true),
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
        assert_eq!(pr.seam(pale).unwrap().a, side(1, Half::Drawn, 5, 5, false));
        // The side seam's mirror image is a seam of its own now, on edge 4.
        let stored = *pr.seams.last().unwrap();
        assert_eq!(
            (stored.id, stored.a, stored.b),
            (
                SeamId(3),
                side(1, Half::Drawn, 4, 4, false),
                side(3, Half::Drawn, 3, 3, false)
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
            side(2, Half::Drawn, 3, 3, false),
        );
        let pale = pr.add_seam(
            side(1, Half::Pale, 0, 0, true),
            side(4, Half::Drawn, 0, 0, true),
        );
        let on_twin = pr.add_seam(
            side(3, Half::Drawn, 0, 0, true),
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
        assert_eq!(SCHEMA_VERSION, 4);
    }

    #[test]
    fn seam_ids_are_bounded_and_adding_a_seam_never_overflows() {
        let base = sewing_room();
        let with_id = |id: u32| {
            let mut pr = base.clone();
            pr.add_seam(
                side(4, Half::Drawn, 0, 0, true),
                side(4, Half::Drawn, 2, 2, true),
            );
            pr.seams[0].id = SeamId(id);
            pr
        };
        assert_eq!(with_id(MAX_SEAM_ID).check(), Ok(()), "the highest allowed");
        for id in [MAX_SEAM_ID + 1, u32::MAX - 1, u32::MAX] {
            assert_eq!(
                with_id(id).check(),
                Err(ModelError::BadSeam(SeamId(id))),
                "id {id}"
            );
        }
        // Adding to a project at the end of the id range (as a corrupt file could leave it)
        // neither panics (debug) nor wraps round to a used id (release): it takes a free one.
        for id in [MAX_SEAM_ID, u32::MAX] {
            let mut pr = with_id(id);
            let next = pr.add_seam(
                side(4, Half::Drawn, 1, 1, true),
                side(4, Half::Drawn, 3, 3, true),
            );
            assert_eq!(next, SeamId(1), "the lowest id not in use");
            let ids: Vec<u32> = pr.seams.iter().map(|s| s.id.0).collect();
            assert_eq!(ids, vec![id, 1]);
            if id == MAX_SEAM_ID {
                assert_eq!(pr.check(), Ok(()), "and sewing still works there");
            }
        }
        // Exactly the most seams is fine: 2,000 edges of one 2,000-gon sewn to another's.
        let ring = |name: &str| {
            let corners: Vec<Point2> = (0..MAX_SEAMS)
                .map(|k| {
                    let a = k as f64 / MAX_SEAMS as f64 * std::f64::consts::TAU;
                    Point2::new(1000.0 * a.cos(), 1000.0 * a.sin())
                })
                .collect();
            Piece::polygon(PieceId(0), name, &corners)
        };
        let mut full = Project::new();
        let (a, b) = (full.add_piece(ring("A")), full.add_piece(ring("B")));
        for k in 0..MAX_SEAMS {
            full.add_seam(
                SeamSide::edges(a, Half::Drawn, k, k, true),
                SeamSide::edges(b, Half::Drawn, k, k, true),
            );
        }
        assert_eq!(full.seams.len(), MAX_SEAMS);
        assert_eq!(full.check(), Ok(()));
    }

    #[test]
    fn a_pale_side_on_a_twin_is_refused() {
        let mut pr = sewing_room();
        pr.add_seam(
            side(3, Half::Pale, 0, 0, true),
            side(4, Half::Drawn, 0, 0, true),
        );
        assert_eq!(pr.check(), Err(ModelError::BadSeam(SeamId(1))));
    }

    #[test]
    fn breaking_a_pair_or_deleting_its_piece_leaves_the_twin_where_it_was() {
        let placed = Placement {
            position: [0.1, 0.8, -0.2],
            rotation: [0.0, 0.6, 0.0, 0.8],
            curve: Some(0.2),
        };
        // The twin has none of its own: it was showing its piece's, mirrored.
        let mut broken = sewing_room();
        broken.set_placement(PieceId(2), Some(placed));
        assert_eq!(broken.placement_of(PieceId(3)), None);
        assert_eq!(broken.break_twin(PieceId(2)), Some(PieceId(3)));
        assert_eq!(broken.placement_of(PieceId(3)), Some(placed.mirrored()));
        assert_eq!(
            broken.placement_of(PieceId(2)),
            Some(placed),
            "the piece stays"
        );
        assert_eq!(broken.check(), Ok(()));
        let mut deleted = sewing_room();
        deleted.set_placement(PieceId(2), Some(placed));
        assert!(deleted.remove_piece(PieceId(2)).is_some());
        assert_eq!(deleted.placement_of(PieceId(3)), Some(placed.mirrored()));
        assert_eq!(deleted.check(), Ok(()));
        // Its own placement wins; with neither, there is still none.
        let own = Placement::at([0.0, 0.5, 1.0]);
        let mut owned = sewing_room();
        owned.set_placement(PieceId(2), Some(placed));
        owned.set_placement(PieceId(3), Some(own));
        owned.break_twin(PieceId(2));
        assert_eq!(owned.placement_of(PieceId(3)), Some(own));
        let mut neither = sewing_room();
        neither.break_twin(PieceId(2));
        assert_eq!(neither.placement_of(PieceId(3)), None);
    }

    #[test]
    fn splitting_an_edge_grows_sides_on_the_b_side_the_twin_and_the_pale_half() {
        let mut pr = sewing_room();
        // The pocket (4) is side a throughout; the edited pieces are side b.
        // b on the back, wrapping over edges 3 and 0.
        let wrap = pr.add_seam(
            side(4, Half::Drawn, 1, 1, true),
            side(2, Half::Drawn, 3, 0, true),
        );
        // b on the twin, covering edge 0: splitting the back's edge 0 splits the twin's too.
        let twin = pr.add_seam(
            side(4, Half::Drawn, 2, 2, true),
            side(3, Half::Drawn, 0, 0, false),
        );
        // b on the pale half of the front, covering edge 1.
        let pale = pr.add_seam(
            side(4, Half::Drawn, 3, 3, true),
            side(1, Half::Pale, 1, 1, true),
        );
        assert_eq!(pr.check(), Ok(()));
        for (id, at) in [(PieceId(2), 0), (PieceId(1), 1)] {
            pr.piece_mut(id).unwrap().split_edge_at(
                at,
                crate::Vertex::corner(Point2::new(
                    if id == PieceId(2) { 350.0 } else { 100.0 },
                    if id == PieceId(2) { 0.0 } else { 50.0 },
                )),
                crate::Edge::Line,
                crate::Edge::Line,
                50.0,
            );
            // 50 mm along the back's 100 mm edge 0, or the front's 200 mm edge 1.
            pr.seams_after_split(id, at, if id == PieceId(2) { 0.5 } else { 0.25 });
        }
        assert_eq!(
            pr.seam(wrap).unwrap().b,
            side(2, Half::Drawn, 4, 1, true),
            "edges 4, 0 and 1 now"
        );
        assert_eq!(
            pr.seam(twin).unwrap().b,
            side(3, Half::Drawn, 0, 1, false),
            "the twin's side grows too"
        );
        assert_eq!(
            pr.seam(pale).unwrap().b,
            side(1, Half::Pale, 1, 2, true),
            "so does one on the pale half"
        );
        assert_eq!(pr.check(), Ok(()));
    }

    #[test]
    fn removing_points_moves_side_ends_on_the_b_side_too() {
        let (mut pr, hex) = two_hexagons();
        // The same seams as `removing_points_puts_side_ends_on_the_joined_edge`, with the
        // edited hexagon (id 1) as side b.
        let s1 = pr.add_seam(
            side(2, Half::Drawn, 0, 0, true),
            side(1, Half::Drawn, 0, 2, true),
        );
        let s2 = pr.add_seam(
            side(2, Half::Drawn, 3, 3, true),
            side(1, Half::Drawn, 3, 3, true),
        );
        let s3 = pr.add_seam(
            side(2, Half::Drawn, 4, 4, true),
            side(1, Half::Drawn, 4, 4, false),
        );
        let remove = |pr: &mut Project, i: usize| {
            let n = pr.piece(hex).unwrap().len();
            assert!(pr.piece_mut(hex).unwrap().remove_vertex(i, 100.0));
            pr.seams_after_removal(hex, i, n, 0.5);
        };
        remove(&mut pr, 1);
        assert_eq!(pr.seam(s1).unwrap().b, side(1, Half::Drawn, 0, 1, true));
        assert_eq!(pr.seam(s2).unwrap().b, side(1, Half::Drawn, 2, 2, true));
        assert_eq!(pr.seam(s3).unwrap().b, side(1, Half::Drawn, 3, 3, false));
        remove(&mut pr, 2);
        let p = OutlinePos::new;
        let (b1, b2) = (pr.seam(s1).unwrap().b, pr.seam(s2).unwrap().b);
        assert_eq!((b1.from, b1.to), (p(0, 0.0), p(1, 0.5)));
        assert_eq!(
            (b2.from, b2.to),
            (p(1, 0.5), p(1, 1.0)),
            "half the joined edge"
        );
        assert_eq!(pr.seam(s3).unwrap().b, side(1, Half::Drawn, 2, 2, false));
        assert_eq!(pr.check(), Ok(()));
    }

    #[test]
    fn unfolding_renumbers_sides_on_the_b_side_too() {
        let mut pr = sewing_room();
        // The pocket (4) is side a; the front (1, folded on edge 3) is side b: its drawn edge 1
        // (with the back as a), and its pale edge 0 and pale edges 1-2 (the pocket).
        let drawn = pr.add_seam(
            side(2, Half::Drawn, 3, 3, false),
            side(1, Half::Drawn, 1, 1, true),
        );
        let pale = pr.add_seam(
            side(4, Half::Drawn, 0, 1, true),
            side(1, Half::Pale, 0, 0, true),
        );
        assert_eq!(pr.check(), Ok(()));
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
        assert_eq!(pr.seam(drawn).unwrap().b, side(1, Half::Drawn, 1, 1, true));
        assert_eq!(
            pr.seam(pale).unwrap().b,
            side(1, Half::Drawn, 5, 5, false),
            "the pale image of edge 0 is the whole piece's edge 5, run backwards"
        );
        // The drawn seam's mirror image (back's twin against the pale edge 1) is stored now.
        let stored = *pr.seams.last().unwrap();
        assert_eq!(
            (stored.a, stored.b),
            (
                side(3, Half::Drawn, 3, 3, false),
                side(1, Half::Drawn, 4, 4, false)
            )
        );
        assert_eq!(pr.check(), Ok(()));
    }

    fn pin(shape: u32, half: Half, x: f64, y: f64) -> Pin {
        Pin {
            shape: PieceId(shape),
            half,
            at: Point2::new(x, y),
            target: [0.1, 1.2, 0.3],
        }
    }

    #[test]
    fn pins_are_checked() {
        let base = sewing_room();
        let with = |pins: Vec<Pin>| {
            let mut pr = base.clone();
            pr.pins = pins;
            pr.check()
        };
        // The front half covers (0,0)–(100,200); the back (300,0)–(400,200).
        assert_eq!(
            with(vec![
                pin(1, Half::Drawn, 50.0, 50.0),
                pin(1, Half::Pale, 99.5, 0.0),
                pin(3, Half::Drawn, 400.9, 100.0),
            ]),
            Ok(()),
            "inside, on the outline, and within 1 mm of it"
        );
        let held = |target: [f64; 3]| Pin {
            target,
            ..pin(2, Half::Drawn, 350.0, 50.0)
        };
        for (bad, why) in [
            (pin(9, Half::Drawn, 50.0, 50.0), "no such shape"),
            (pin(2, Half::Pale, 350.0, 50.0), "the back isn't folded"),
            (pin(2, Half::Drawn, 401.5, 50.0), "1.5 mm outside"),
            (pin(2, Half::Drawn, f64::NAN, 50.0), "not a number"),
            (held([0.0, 11.0, 0.0]), "held 11 m away"),
            (held([0.0, 1.0, f64::NAN]), "held at no number"),
        ] {
            assert_eq!(
                with(vec![pin(1, Half::Drawn, 50.0, 50.0), bad]),
                Err(ModelError::BadPin(1)),
                "{why}"
            );
        }
        assert_eq!(
            with(vec![pin(1, Half::Drawn, 50.0, 50.0); MAX_PINS]),
            Ok(())
        );
        assert_eq!(
            with(vec![pin(1, Half::Drawn, 50.0, 50.0); MAX_PINS + 1]),
            Err(ModelError::TooManyPins)
        );
        assert_eq!(ModelError::BadPin(1).to_string(), "pin 2 is invalid");
    }

    #[test]
    fn pins_stay_on_their_spot_of_fabric_through_edits() {
        let mut pr = sewing_room();
        pr.pins = vec![
            pin(1, Half::Pale, 20.0, 30.0),
            pin(1, Half::Drawn, 60.0, 30.0),
            pin(3, Half::Drawn, 320.0, 10.0),
            pin(4, Half::Drawn, 40.0, 440.0),
        ];
        assert_eq!(pr.check(), Ok(()));
        // The twin shows stored point (x, y) at (900 - x, y): its pin is at (580, 10) there.
        let mut broken = pr.clone();
        broken.break_twin(PieceId(2));
        assert_eq!(
            broken.pins[2].at,
            Point2::new(580.0, 10.0),
            "kept where it shows"
        );
        assert_eq!(broken.check(), Ok(()));
        // Moving the back moves the twin's pin with it (the twin stays where it is, but its pin
        // is kept where the back shows it).
        let mut moved = pr.clone();
        moved
            .piece_mut(PieceId(2))
            .unwrap()
            .translate(Point2::new(10.0, 5.0));
        moved.move_pins(PieceId(2), Point2::new(10.0, 5.0));
        assert_eq!(moved.pins[2].at, Point2::new(330.0, 15.0));
        assert_eq!(
            moved.pins[0].at,
            Point2::new(20.0, 30.0),
            "the front's stay"
        );
        let twin = moved.pieces[1].twin_shape().unwrap();
        let shown = |offset: Point2, at: Point2| Point2::new(offset.x - at.x, at.y + offset.y);
        assert_eq!(
            shown(
                moved.pieces[1].twin.as_ref().unwrap().offset,
                moved.pins[2].at
            ),
            shown(pr.pieces[1].twin.as_ref().unwrap().offset, pr.pins[2].at),
            "the twin's pin is on the same spot of the twin"
        );
        assert!(twin.check().is_ok());
        // Unfolding: the pale pin's spot is (-20, 30) on the whole piece.
        let mut unfolded = pr.clone();
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
        assert!(unfolded.unfold_piece(PieceId(1), full));
        assert_eq!(unfolded.pins[0], pin(1, Half::Drawn, -20.0, 30.0));
        assert_eq!(unfolded.check(), Ok(()));
        // Removing the fold takes its pale half's pin; deleting a piece takes its pins.
        let mut flat = pr.clone();
        flat.remove_fold(PieceId(1));
        assert_eq!(flat.pins.len(), 3);
        flat.remove_piece(PieceId(4));
        assert_eq!(
            flat.pins.iter().filter(|p| p.shape == PieceId(4)).count(),
            0
        );
        // The pocket made narrower than where its pin is (10 mm outside it now): the edit
        // deletes the pin.
        let mut shrunk = pr.clone();
        let pocket = shrunk.piece_mut(PieceId(4)).unwrap();
        pocket.move_vertex(1, Point2::new(30.0, 400.0));
        pocket.move_vertex(2, Point2::new(30.0, 480.0));
        assert_eq!(
            shrunk.check(),
            Err(ModelError::BadPin(3)),
            "refused if kept"
        );
        shrunk.drop_broken();
        assert_eq!(shrunk.pins.len(), 3);
        assert_eq!(shrunk.check(), Ok(()));
    }
}
