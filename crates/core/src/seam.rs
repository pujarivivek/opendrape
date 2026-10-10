//! Seams: which stretches of outline are sewn to which. A side runs between two points on one
//! shape's outline, which may be anywhere along an edge ("free" sewing); a whole-edge seam is a
//! free seam whose ends are corners. Only the seams the student sewed are stored; the mirror
//! image of a seam on a cut-on-fold piece or a mirrored pair is worked out when it is needed
//! (see `Project::mirror_of`).

use crate::PieceId;
use serde::{Deserialize, Serialize};

/// Most seams a project may hold (their mirror images are not counted).
pub const MAX_SEAMS: usize = 2_000;

/// Highest seam id a project may hold. Far above anything a student sews (ids are one more than
/// the highest in use); it only stops a corrupt file from putting an id at the end of its range.
pub const MAX_SEAM_ID: u32 = 1_000_000;

/// A seam side must be longer than this (mm); an edit that leaves one this short or shorter
/// deletes its seam.
pub const MIN_SIDE_MM: f64 = 1.0;

/// Two stretches of outline may meet at a point; they overlap only when they share more than
/// this much of an edge (as a fraction of its length), so rounding never counts as overlap.
const OVERLAP_SLACK: f64 = 1e-9;

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

/// A point on a stored piece's outline: stored edge `edge`, a fraction `t` (0 at the edge's
/// start, 1 at its end) of the way along it by arc length.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct OutlinePos {
    pub edge: usize,
    pub t: f64,
}

impl OutlinePos {
    pub const fn new(edge: usize, t: f64) -> Self {
        Self { edge, t }
    }
}

/// The part of one stored edge a side covers: from fraction `t0` to `t1` of the way along it,
/// `t0 < t1`, whichever way the side runs.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Span {
    pub edge: usize,
    pub t0: f64,
    pub t1: f64,
}

/// One side of a seam: the outline of one shape from `from` to `to`.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct SeamSide {
    /// A piece's id or a twin's id.
    pub shape: PieceId,
    #[serde(default)]
    pub half: Half,
    /// Where the side starts: it meets the other side's start.
    pub from: OutlinePos,
    /// Where the side ends.
    pub to: OutlinePos,
    /// True when the side runs from `from` to `to` the way the stored outline runs; false when
    /// it runs the other way. It may pass corners and wrap past the last edge.
    pub forward: bool,
}

impl SeamSide {
    /// A side of whole stored edges: `first` to `last` along the stored outline (wrapping past
    /// the last edge), starting at `first`'s start when `forward`, at `last`'s end otherwise.
    pub fn edges(shape: PieceId, half: Half, first: usize, last: usize, forward: bool) -> Self {
        let (low, high) = (OutlinePos::new(first, 0.0), OutlinePos::new(last, 1.0));
        let (from, to) = if forward { (low, high) } else { (high, low) };
        Self {
            shape,
            half,
            from,
            to,
            forward,
        }
    }

    /// The same side with an end that is at a corner named on the edge the side covers there
    /// (a start at the very end of an edge is the start of the next one; an end at the very
    /// start of an edge is the end of the one before), on an outline of `n` edges. Sides that
    /// cover the same stretch the same way then compare equal.
    pub fn tidy(&self, n: usize) -> Self {
        if n == 0 {
            return *self;
        }
        let (next, prev) = (|e: usize| (e + 1) % n, |e: usize| (e + n - 1) % n);
        let (mut from, mut to) = (self.from, self.to);
        // The edge a side runs into from a corner: the next one running forward, the one
        // before running backward.
        if self.forward {
            if from.t >= 1.0 {
                from = OutlinePos::new(next(from.edge), 0.0);
            }
            if to.t <= 0.0 {
                to = OutlinePos::new(prev(to.edge), 1.0);
            }
        } else {
            if from.t <= 0.0 {
                from = OutlinePos::new(prev(from.edge), 1.0);
            }
            if to.t >= 1.0 {
                to = OutlinePos::new(next(to.edge), 0.0);
            }
        }
        Self { from, to, ..*self }
    }

    /// The same stretch of outline, run the other way.
    pub fn flipped(&self) -> Self {
        Self {
            from: self.to,
            to: self.from,
            forward: !self.forward,
            ..*self
        }
    }

    /// The parts of stored edges it covers on an outline of `n` edges, in the order the side
    /// runs (each with `t0 < t1`). Empty when it covers nothing (it starts where it ends) or names
    /// an edge the outline doesn't have.
    pub fn spans(&self, n: usize) -> Vec<Span> {
        let (f, t) = (self.from, self.to);
        if n == 0 || f.edge >= n || t.edge >= n {
            return Vec::new();
        }
        let mut out = Vec::new();
        if self.forward {
            if f.edge == t.edge && t.t >= f.t {
                out.push(Span {
                    edge: f.edge,
                    t0: f.t,
                    t1: t.t,
                });
            } else {
                out.push(Span {
                    edge: f.edge,
                    t0: f.t,
                    t1: 1.0,
                });
                let mut e = (f.edge + 1) % n;
                while e != t.edge {
                    out.push(Span {
                        edge: e,
                        t0: 0.0,
                        t1: 1.0,
                    });
                    e = (e + 1) % n;
                }
                out.push(Span {
                    edge: t.edge,
                    t0: 0.0,
                    t1: t.t,
                });
            }
        } else if f.edge == t.edge && t.t <= f.t {
            out.push(Span {
                edge: f.edge,
                t0: t.t,
                t1: f.t,
            });
        } else {
            out.push(Span {
                edge: f.edge,
                t0: 0.0,
                t1: f.t,
            });
            let mut e = (f.edge + n - 1) % n;
            while e != t.edge {
                out.push(Span {
                    edge: e,
                    t0: 0.0,
                    t1: 1.0,
                });
                e = (e + n - 1) % n;
            }
            out.push(Span {
                edge: t.edge,
                t0: t.t,
                t1: 1.0,
            });
        }
        out.retain(|s| s.t1 > s.t0);
        out
    }

    /// Whether it covers any of stored edge `e` (of an outline of `n` edges).
    pub fn covers(&self, n: usize, e: usize) -> bool {
        self.spans(n).iter().any(|s| s.edge == e)
    }

    /// Whether it and `other` cover the same stretch of the same shape's half, whichever way
    /// each runs.
    pub fn same_part(&self, other: &SeamSide) -> bool {
        (self.shape, self.half) == (other.shape, other.half)
            && (*self == *other || *self == other.flipped())
    }

    /// Whether it shares more than a point with `other` (on an outline of `n` edges).
    pub fn overlaps(&self, other: &SeamSide, n: usize) -> bool {
        (self.shape, self.half) == (other.shape, other.half)
            && spans_overlap(&self.spans(n), &other.spans(n))
    }
}

/// Whether any span of `a` shares more than a point with any span of `b`.
pub(crate) fn spans_overlap(a: &[Span], b: &[Span]) -> bool {
    a.iter().any(|p| {
        b.iter()
            .any(|q| p.edge == q.edge && p.t0.max(q.t0) < p.t1.min(q.t1) - OVERLAP_SLACK)
    })
}

/// Two sides sewn together, matched end to end: `a`'s start meets `b`'s start.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
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

    fn span(edge: usize, t0: f64, t1: f64) -> Span {
        Span { edge, t0, t1 }
    }

    #[test]
    fn whole_edge_sides_cover_their_edges_and_wrap() {
        let s = SeamSide::edges(PieceId(1), Half::Drawn, 3, 0, true);
        assert_eq!(
            s.spans(5),
            vec![span(3, 0.0, 1.0), span(4, 0.0, 1.0), span(0, 0.0, 1.0)]
        );
        assert!(s.covers(5, 4) && s.covers(5, 0) && !s.covers(5, 1) && !s.covers(5, 2));
        assert_eq!(s.from, OutlinePos::new(3, 0.0));
        assert_eq!(s.to, OutlinePos::new(0, 1.0));
        let back = SeamSide::edges(PieceId(1), Half::Drawn, 3, 0, false);
        assert_eq!(
            (back.from, back.to),
            (OutlinePos::new(0, 1.0), OutlinePos::new(3, 0.0))
        );
        assert_eq!(
            back.spans(5),
            vec![span(0, 0.0, 1.0), span(4, 0.0, 1.0), span(3, 0.0, 1.0)],
            "listed the way it runs"
        );
        assert!(s.same_part(&back) && s != back && back == s.flipped());
        assert!(!s.same_part(&SeamSide {
            half: Half::Pale,
            ..s
        }));
        assert_eq!(Half::Pale.other(), Half::Drawn);
    }

    #[test]
    fn free_sides_cover_parts_of_edges_and_pass_corners() {
        let side = |from: OutlinePos, to: OutlinePos, forward| SeamSide {
            shape: PieceId(1),
            half: Half::Drawn,
            from,
            to,
            forward,
        };
        let p = OutlinePos::new;
        assert_eq!(
            side(p(1, 0.25), p(1, 0.75), true).spans(4),
            vec![span(1, 0.25, 0.75)]
        );
        assert_eq!(
            side(p(1, 0.75), p(1, 0.25), false).spans(4),
            vec![span(1, 0.25, 0.75)]
        );
        // Round the corner between edges 1 and 2.
        assert_eq!(
            side(p(1, 0.5), p(2, 0.5), true).spans(4),
            vec![span(1, 0.5, 1.0), span(2, 0.0, 0.5)]
        );
        // The long way round from the same two points: every other edge.
        assert_eq!(
            side(p(1, 0.5), p(2, 0.5), false).spans(4),
            vec![
                span(1, 0.0, 0.5),
                span(0, 0.0, 1.0),
                span(3, 0.0, 1.0),
                span(2, 0.5, 1.0)
            ]
        );
        // Starting further along the same edge than it ends: all the way round.
        assert_eq!(side(p(1, 0.75), p(1, 0.25), true).spans(4).len(), 5);
        // A side ending where it starts covers nothing; an edge the outline lacks, nothing.
        assert!(side(p(1, 0.5), p(1, 0.5), true).spans(4).is_empty());
        assert!(side(p(4, 0.0), p(1, 0.5), true).spans(4).is_empty());
        // A start at the very end of an edge covers nothing of that edge.
        assert_eq!(
            side(p(0, 1.0), p(1, 0.5), true).spans(4),
            vec![span(1, 0.0, 0.5)]
        );
    }

    #[test]
    fn a_side_ending_at_a_corner_is_tidied_onto_the_edge_it_covers() {
        let p = OutlinePos::new;
        let side = |from, to, forward| SeamSide {
            shape: PieceId(1),
            half: Half::Drawn,
            from,
            to,
            forward,
        };
        // Clicked at the corner between edges 0 and 1, then at the corner between 2 and 3: the
        // same as whole edges 1 and 2.
        let clicked = side(p(0, 1.0), p(3, 0.0), true);
        assert_eq!(
            clicked.tidy(4),
            SeamSide::edges(PieceId(1), Half::Drawn, 1, 2, true)
        );
        assert_eq!(clicked.tidy(4).spans(4), clicked.spans(4));
        let backwards = side(p(3, 0.0), p(0, 1.0), false);
        assert_eq!(
            backwards.tidy(4),
            SeamSide::edges(PieceId(1), Half::Drawn, 1, 2, false)
        );
        // Ends inside edges stay as they are.
        let inside = side(p(0, 0.5), p(1, 0.25), true);
        assert_eq!(inside.tidy(4), inside);
    }

    #[test]
    fn sides_overlap_only_when_they_share_more_than_a_point() {
        let side = |e: usize, t0: f64, t1: f64| SeamSide {
            shape: PieceId(1),
            half: Half::Drawn,
            from: OutlinePos::new(e, t0),
            to: OutlinePos::new(e, t1),
            forward: true,
        };
        assert!(
            !side(1, 0.0, 0.5).overlaps(&side(1, 0.5, 1.0), 4),
            "they meet"
        );
        assert!(side(1, 0.0, 0.5).overlaps(&side(1, 0.4, 1.0), 4));
        assert!(!side(1, 0.0, 0.5).overlaps(&side(2, 0.0, 0.5), 4));
        let pale = SeamSide {
            half: Half::Pale,
            ..side(1, 0.0, 0.5)
        };
        assert!(!side(1, 0.0, 0.5).overlaps(&pale, 4), "the other half");
    }

    #[test]
    fn seams_serialise_plainly() {
        let seam = Seam {
            id: SeamId(4),
            a: SeamSide::edges(PieceId(1), Half::Pale, 2, 2, true),
            b: SeamSide {
                shape: PieceId(3),
                half: Half::Drawn,
                from: OutlinePos::new(0, 0.5),
                to: OutlinePos::new(1, 0.25),
                forward: false,
            },
        };
        let json = serde_json::to_string(&seam).unwrap();
        assert_eq!(
            json,
            r#"{"id":4,"a":{"shape":1,"half":"pale","from":{"edge":2,"t":0.0},"to":{"edge":2,"t":1.0},"forward":true},"b":{"shape":3,"half":"drawn","from":{"edge":0,"t":0.5},"to":{"edge":1,"t":0.25},"forward":false}}"#
        );
        assert_eq!(serde_json::from_str::<Seam>(&json).unwrap(), seam);
    }
}
