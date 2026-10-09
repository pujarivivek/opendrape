//! The Sew tool (W). Click an edge, then the edge it is sewn to: two clicks make a seam.
//! Shift-click adds the next edge along the outline to the side being built: the first side,
//! or, once the seam is made, its second. The end of an edge nearer the first click on it is
//! where its side starts, and the two starts meet. Esc cancels a half-made seam; clicking away
//! from every edge ends extending.

use super::{PatternEditor, Selection};
use crate::tr;
use egui::Response;
use opendrape_core::{Half, PieceId, Point2, Project, SeamId, SeamSide};
use opendrape_geom as geom;

/// A seam being sewn.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct SewDraft {
    /// The first side, while it is built.
    pub a: SeamSide,
    /// The seam the second click made: Shift-clicks now add edges to its second side.
    pub seam: Option<SeamId>,
}

/// The outline edge nearest the pointer, as the Sew tool sees it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct EdgeUnder {
    pub shape: PieceId,
    pub half: Half,
    /// The stored edge.
    pub edge: usize,
    /// The pointer is nearer the stored edge's start: a side started here runs forward.
    pub forward: bool,
    /// How far (mm) the pointer is from the edge.
    pub distance: f64,
}

impl EdgeUnder {
    /// A side of just this edge, starting at its end nearer the pointer.
    fn side(&self) -> SeamSide {
        SeamSide::new(self.shape, self.half, self.edge, 1, self.forward)
    }
}

/// The outline edge of any shape nearest to `w`, within `tol` mm: the pale half of a fold
/// included (a fold's own edge is inside its shape, so it is never found).
pub(super) fn edge_under(project: &Project, w: Point2, tol: f64) -> Option<EdgeUnder> {
    geom::shapes(project)
        .iter()
        .filter_map(|s| {
            let (j, t, d) = geom::nearest_edge(&s.piece, w)?;
            if d > tol {
                return None;
            }
            let (half, edge, against) = s.sew_edge(j);
            let near_start =
                geom::distance_along(&s.piece, j, t) <= geom::edge_length(&s.piece, j) / 2.0;
            Some(EdgeUnder {
                shape: s.id,
                half,
                edge,
                forward: near_start != against,
                distance: d,
            })
        })
        .min_by(|a, b| a.distance.total_cmp(&b.distance))
}

/// `side` (on an outline of `n` edges) with stored edge `edge` added at whichever of its ends
/// the edge is next to; None when it is next to neither, or the side is the whole outline.
pub(super) fn extended(side: SeamSide, edge: usize, n: usize) -> Option<SeamSide> {
    if side.edges >= n {
        return None;
    }
    if edge == (side.first_edge + side.edges) % n {
        Some(SeamSide {
            edges: side.edges + 1,
            ..side
        })
    } else if (edge + 1) % n == side.first_edge {
        Some(SeamSide {
            first_edge: edge,
            edges: side.edges + 1,
            ..side
        })
    } else {
        None
    }
}

/// Whether the draft still fits the project (an undo or a deleted piece may have taken its
/// edges or its seam away).
fn draft_fits(project: &Project, draft: &SewDraft) -> bool {
    let fits = project.owner(draft.a.shape).is_some_and(|(p, _)| {
        draft.a.first_edge < p.len()
            && draft.a.edges <= p.len()
            && (draft.a.half == Half::Drawn || p.fold.is_some())
    });
    fits && draft.seam.is_none_or(|id| project.seam(id).is_some())
}

impl PatternEditor {
    pub(super) fn sew_tool(
        &mut self,
        response: &Response,
        pointer: Option<Point2>,
        tol: f64,
        shift: bool,
    ) {
        let project = self.doc.project();
        self.canvas.sew = self.canvas.sew.filter(|d| draft_fits(project, d));
        if !response.clicked() {
            return;
        }
        let Some(at) = pointer else { return };
        let Some(hit) = edge_under(self.doc.project(), at, tol) else {
            // A click away from every edge ends extending; a half-made seam waits for its
            // second edge.
            if self.canvas.sew.is_some_and(|d| d.seam.is_some()) {
                self.canvas.sew = None;
            }
            return;
        };
        let project = self.doc.project();
        let n = project.owner(hit.shape).map_or(0, |(p, _)| p.len());
        let draft = self.canvas.sew;
        let in_first_side = draft.is_some_and(|d| {
            d.seam.is_none()
                && d.a.shape == hit.shape
                && d.a.half == hit.half
                && d.a.covers(n, hit.edge)
        });
        if in_first_side {
            self.notice = Some(tr!("notice-sew-same-edge"));
            return;
        }
        if project.seam_on(hit.shape, hit.half, hit.edge).is_some() {
            self.notice = Some(tr!("notice-already-sewn"));
            return;
        }
        match (draft, shift) {
            (Some(SewDraft { a, seam: None }), true) => {
                let same_outline = a.shape == hit.shape && a.half == hit.half;
                match same_outline.then(|| extended(a, hit.edge, n)).flatten() {
                    Some(a) => self.canvas.sew = Some(SewDraft { a, seam: None }),
                    None => self.notice = Some(tr!("notice-sew-not-next")),
                }
            }
            (Some(SewDraft { seam: Some(id), .. }), true) => self.extend_second_side(id, hit, n),
            (Some(SewDraft { a, seam: None }), false) => self.make_seam(a, hit),
            _ => {
                self.canvas.sew = Some(SewDraft {
                    a: hit.side(),
                    seam: None,
                })
            }
        }
    }

    /// Sews the finished first side `a` to the clicked edge, as one undo step.
    fn make_seam(&mut self, a: SeamSide, hit: EdgeUnder) {
        let id = self.doc.edit(|p| p.add_seam(a, hit.side()));
        // Refused when the seam's mirror image would sew an edge that is already sewn.
        if self.note_if_refused() {
            return;
        }
        self.canvas.sew = Some(SewDraft { a, seam: Some(id) });
        self.selection = Selection::Seam(id);
    }

    /// Adds the clicked edge to seam `id`'s second side, as one undo step.
    fn extend_second_side(&mut self, id: SeamId, hit: EdgeUnder, n: usize) {
        let grown = self.doc.edit(|p| {
            let seam = p.seam_mut(id)?;
            let same_outline = seam.b.shape == hit.shape && seam.b.half == hit.half;
            seam.b = same_outline
                .then(|| extended(seam.b, hit.edge, n))
                .flatten()?;
            Some(())
        });
        if !self.note_if_refused() && grown.is_none() {
            self.notice = Some(tr!("notice-sew-not-next"));
        }
    }

    /// The side being built, to draw: the first while the seam is half made, then the second.
    pub(super) fn sew_draft_side(&self) -> Option<SeamSide> {
        let draft = self.canvas.sew?;
        match draft.seam {
            None => Some(draft.a),
            Some(id) => self.doc.project().seam(id).map(|s| s.b),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_side_grows_at_either_end_and_wraps() {
        let side = SeamSide::new(PieceId(1), Half::Drawn, 0, 1, true);
        assert_eq!(extended(side, 1, 4), Some(SeamSide { edges: 2, ..side }));
        assert_eq!(
            extended(side, 3, 4),
            Some(SeamSide {
                first_edge: 3,
                edges: 2,
                ..side
            })
        );
        assert_eq!(extended(side, 2, 4), None, "not next to it");
        let all = SeamSide::new(PieceId(1), Half::Drawn, 0, 4, true);
        assert_eq!(extended(all, 0, 4), None, "already the whole outline");
    }
}
