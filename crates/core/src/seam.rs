//! Seams: which outline edges are sewn to which. Only the seams the student sewed are stored;
//! the mirror image of a seam on a cut-on-fold piece or a mirrored pair is worked out when it
//! is needed (see `Project::mirror_of`).

use crate::PieceId;
use serde::{Deserialize, Serialize};

/// Most seams a project may hold (their mirror images are not counted).
pub const MAX_SEAMS: usize = 2_000;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct SeamId(pub u32);

/// Which half of a cut-on-fold piece a seam side is on.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Half {
    /// The stored half; also every piece that isn't folded, and every twin.
    #[default]
    Drawn,
    /// The pale mirror image across the fold.
    Pale,
}

impl Half {
    pub fn other(self) -> Self {
        match self {
            Self::Drawn => Self::Pale,
            Self::Pale => Self::Drawn,
        }
    }
}

/// One side of a seam: one or more consecutive outline edges of one shape.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SeamSide {
    /// A piece's id or a twin's id.
    pub shape: PieceId,
    #[serde(default)]
    pub half: Half,
    /// The lowest of its edges in the stored outline's order: the side covers stored edges
    /// `first_edge`, `first_edge + 1`, … `first_edge + edges - 1`, wrapping past the last edge.
    pub first_edge: usize,
    /// How many consecutive edges; at least 1.
    pub edges: usize,
    /// True when the side runs the way the stored outline does (it starts at the start of
    /// `first_edge`); false when it runs the other way (it starts at the end of its last edge).
    pub forward: bool,
}

impl SeamSide {
    pub fn new(shape: PieceId, half: Half, first_edge: usize, edges: usize, forward: bool) -> Self {
        Self {
            shape,
            half,
            first_edge,
            edges,
            forward,
        }
    }
    /// The stored edges it covers, lowest first (wrapping), on an outline of `n` edges.
    pub fn stored_edges(&self, n: usize) -> impl Iterator<Item = usize> + '_ {
        let n = n.max(1);
        (0..self.edges).map(move |k| (self.first_edge + k) % n)
    }
    /// Whether it covers stored edge `e` of an outline of `n` edges.
    pub fn covers(&self, n: usize, e: usize) -> bool {
        n > 0 && (e % n + n - self.first_edge % n) % n < self.edges
    }
    /// The same shape, half and edges, whichever way the two run.
    pub fn same_edges(&self, other: &SeamSide) -> bool {
        (self.shape, self.half, self.first_edge, self.edges)
            == (other.shape, other.half, other.first_edge, other.edges)
    }
}

/// Two sides sewn together, matched end to end: `a`'s start meets `b`'s start.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Seam {
    pub id: SeamId,
    pub a: SeamSide,
    pub b: SeamSide,
}

impl Seam {
    /// Whether either side is on shape `id`.
    pub fn touches(&self, id: PieceId) -> bool {
        self.a.shape == id || self.b.shape == id
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sides_cover_consecutive_edges_and_wrap() {
        let s = SeamSide::new(PieceId(1), Half::Drawn, 3, 3, true);
        assert_eq!(s.stored_edges(5).collect::<Vec<_>>(), vec![3, 4, 0]);
        assert!(s.covers(5, 4) && s.covers(5, 0) && !s.covers(5, 1) && !s.covers(5, 2));
        let flipped = SeamSide {
            forward: false,
            ..s
        };
        assert!(s.same_edges(&flipped) && s != flipped);
        assert!(!s.same_edges(&SeamSide {
            half: Half::Pale,
            ..s
        }));
        assert_eq!(Half::Pale.other(), Half::Drawn);
    }

    #[test]
    fn seams_serialise_plainly() {
        let seam = Seam {
            id: SeamId(4),
            a: SeamSide::new(PieceId(1), Half::Pale, 2, 1, true),
            b: SeamSide::new(PieceId(3), Half::Drawn, 0, 2, false),
        };
        let json = serde_json::to_string(&seam).unwrap();
        assert_eq!(
            json,
            r#"{"id":4,"a":{"shape":1,"half":"pale","first_edge":2,"edges":1,"forward":true},"b":{"shape":3,"half":"drawn","first_edge":0,"edges":2,"forward":false}}"#
        );
        assert_eq!(serde_json::from_str::<Seam>(&json).unwrap(), seam);
    }
}
