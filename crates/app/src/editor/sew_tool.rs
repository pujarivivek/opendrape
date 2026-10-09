//! The Sew tool (W). Click an edge, then the edge it is sewn to: two clicks make a seam.
//! Shift-click adds the next edge along the outline to the side being built: the first side,
//! or, once the seam is made, its second. The end of an edge nearer the first click on it is
//! where its side starts, and the two starts meet. Esc cancels a half-made seam; clicking away
//! from every edge ends extending.

use super::{PatternEditor, Selection};
use crate::tr;
use egui::Response;
use opendrape_core::{Half, ModelError, PieceId, Point2, Project, SeamId, SeamSide};
use opendrape_geom as geom;

/// A seam being sewn.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct SewDraft {
    /// The first side, while it is built.
    pub a: SeamSide,
    /// The seam the second click made: Shift-clicks now add edges to its second side.
    pub seam: Option<SeamId>,
    /// How many edges the first side's outline had when it was picked. The side counts its
    /// edges from the outline's first, so a change in the count (a point added or removed, an
    /// unfold) means the edge it names is not the edge that was clicked.
    pub outline_edges: usize,
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

/// Whether `w` is within `tol` mm of a fold line: the straight line inside a cut-on-fold
/// piece where its two halves meet.
fn on_a_fold_line(project: &Project, w: Point2, tol: f64) -> bool {
    geom::shapes(project).iter().any(|s| match s.kind {
        geom::ShapeKind::Folded { fold: (a, b), .. } => {
            super::canvas::segment_distance(w, a, b) <= tol
        }
        _ => false,
    })
}

/// Whether the draft still fits the project. A half-made seam needs its shape, with the same
/// number of edges as when the first side was picked, the half it was picked on, and its edges
/// not on the fold. Once the seam is made, the draft only needs the seam. Anything else (an undo,
/// a redo, a deleted piece, a point added from the panel) leaves it pointing at nothing, or
/// at another edge.
fn draft_fits(project: &Project, draft: &SewDraft) -> bool {
    if let Some(id) = draft.seam {
        return project.seam(id).is_some();
    }
    project.owner(draft.a.shape).is_some_and(|(p, _)| {
        let n = p.len();
        n == draft.outline_edges
            && draft.a.first_edge < n
            && draft.a.edges <= n
            && (draft.a.half == Half::Drawn || p.fold.is_some())
            && p.fold.is_none_or(|f| !draft.a.covers(n, f))
    })
}

impl PatternEditor {
    /// Drops the draft if the project no longer has what it points at (see [`draft_fits`]).
    pub(super) fn drop_stale_sew(&mut self) {
        let project = self.doc.project();
        self.canvas.sew = self.canvas.sew.filter(|d| draft_fits(project, d));
    }

    /// A seam with only its first side picked, which is not in the project yet.
    pub(super) fn has_half_made_seam(&self) -> bool {
        self.canvas.sew.is_some_and(|d| d.seam.is_none())
    }

    /// Drops a half-made seam; true if there was one.
    pub(super) fn cancel_half_made_seam(&mut self) -> bool {
        self.canvas.sew.take_if(|d| d.seam.is_none()).is_some()
    }

    pub(super) fn sew_tool(
        &mut self,
        response: &Response,
        pointer: Option<Point2>,
        tol: f64,
        shift: bool,
    ) {
        self.drop_stale_sew();
        if !response.clicked() {
            return;
        }
        let Some(at) = pointer else { return };
        // A click on a seam's line selects that seam.
        if let Some(seam) = self.seam_at(at, tol) {
            self.selection = Selection::Seam(seam);
            self.canvas.sew = None;
            return;
        }
        let Some(hit) = edge_under(self.doc.project(), at, tol) else {
            // A click on a fold line says why it does nothing.
            if on_a_fold_line(self.doc.project(), at, tol) {
                self.notice = Some(tr!("notice-sew-fold"));
            }
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
        // Shift-clicking an edge that is already in the side is not an edge *next* to it: that
        // is for the Shift branch below to say.
        if in_first_side && !shift {
            self.notice = Some(tr!("notice-sew-same-edge"));
            return;
        }
        if project.seam_on(hit.shape, hit.half, hit.edge).is_some() {
            self.notice = Some(tr!("notice-already-sewn"));
            return;
        }
        match (draft, shift) {
            (Some(SewDraft { seam: None, a, .. }), true) => {
                let same_outline = a.shape == hit.shape && a.half == hit.half;
                match same_outline.then(|| extended(a, hit.edge, n)).flatten() {
                    Some(a) => {
                        self.canvas.sew = draft.map(|d| SewDraft { a, ..d });
                    }
                    None => self.notice = Some(tr!("notice-sew-not-next")),
                }
            }
            (Some(SewDraft { seam: Some(id), .. }), true) => self.extend_second_side(id, hit, n),
            (Some(d @ SewDraft { seam: None, .. }), false) => self.make_seam(d, hit),
            _ => {
                self.canvas.sew = Some(SewDraft {
                    a: hit.side(),
                    seam: None,
                    outline_edges: n,
                })
            }
        }
    }

    /// Sews the draft's finished first side to the clicked edge, as one undo step.
    fn make_seam(&mut self, draft: SewDraft, hit: EdgeUnder) {
        let id = self.doc.edit(|p| p.add_seam(draft.a, hit.side()));
        if self.doc.last_change_refused() {
            // Both edges are free (they were checked), so what refuses a seam is that its
            // mirror image would sew an edge that is already sewn; or, at the very limit, there
            // are too many seams.
            self.notice = Some(match self.doc.last_refusal() {
                Some(ModelError::BadSeam(_)) => tr!("notice-mirror-sewn"),
                _ => tr!("notice-refused"),
            });
            return;
        }
        self.canvas.sew = Some(SewDraft {
            seam: Some(id),
            ..draft
        });
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
    use opendrape_core::{Edge, Piece};

    fn p(x: f64, y: f64) -> Point2 {
        Point2::new(x, y)
    }

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

    /// A project with one of each kind of shape: a plain piece with a curved edge, a piece cut
    /// on a fold (its edge 3 is the fold), and a piece with its twin.
    fn one_of_each() -> (Project, PieceId, PieceId, PieceId, PieceId) {
        let mut pr = Project::new();
        let mut plain = Piece::rectangle(PieceId(0), "Plain", p(0.0, 0.0), 300.0, 400.0);
        plain.edges[1] = Edge::Curve {
            c1: p(380.0, 100.0),
            c2: p(380.0, 300.0),
        };
        let plain = pr.add_piece(plain);
        let mut half = Piece::rectangle(PieceId(0), "Half", p(500.0, 100.0), 150.0, 300.0);
        half.fold = Some(3);
        let half = pr.add_piece(half);
        let master = pr.add_piece(Piece::rectangle(
            PieceId(0),
            "Back",
            p(800.0, 100.0),
            300.0,
            400.0,
        ));
        let twin = pr.add_twin(master, "Back (mirror)".into(), p(3000.0, 0.0));
        (pr, plain, half, master, twin.unwrap())
    }

    /// The mirror image of `q` across the line through `a` and `b`.
    fn reflect(q: Point2, a: Point2, b: Point2) -> Point2 {
        let d = b - a;
        let t = ((q.x - a.x) * d.x + (q.y - a.y) * d.y) / (d.x * d.x + d.y * d.y);
        (a + d * t) * 2.0 - q
    }

    #[test]
    fn a_side_starts_at_the_end_nearer_the_click_on_every_kind_of_shape() {
        let (pr, plain, half, master, twin) = one_of_each();
        let shapes = geom::shapes(&pr);
        let fold = shapes
            .iter()
            .find_map(|s| match s.kind {
                geom::ShapeKind::Folded { fold, .. } => Some(fold),
                _ => None,
            })
            .expect("the folded piece");
        // Every stored edge, on every shape that shows it, clicked a quarter and then three
        // quarters of the way along it (by arc length, on the stored piece, then carried to
        // where the shape shows that point). Nearer the stored start means `forward`.
        let mut clicks = 0;
        for (shape, source, halves) in [
            (plain, plain, &[Half::Drawn][..]),
            (half, half, &[Half::Drawn, Half::Pale][..]),
            (master, master, &[Half::Drawn][..]),
            (twin, master, &[Half::Drawn][..]),
        ] {
            let stored = pr.piece(source).unwrap();
            let on_shape = shapes.iter().find(|s| s.id == shape).unwrap();
            for half_of in halves {
                for edge in (0..stored.len()).filter(|e| stored.fold != Some(*e)) {
                    let length = geom::edge_length(stored, edge);
                    for along in [0.25, 0.75] {
                        let at = geom::point_at_distance(stored, edge, length * along);
                        let at = match (on_shape.kind, half_of) {
                            (geom::ShapeKind::Twin { .. }, _) => on_shape.from_stored(at),
                            (_, Half::Pale) => reflect(at, fold.0, fold.1),
                            _ => at,
                        };
                        let hit = edge_under(&pr, at, 3.0).expect("an edge under the click");
                        assert_eq!(
                            (hit.shape, hit.half, hit.edge, hit.forward),
                            (shape, *half_of, edge, along < 0.5),
                            "edge {edge} of {shape:?} ({half_of:?}) at {along}"
                        );
                        clicks += 1;
                    }
                }
            }
        }
        assert_eq!(
            clicks,
            2 * (4 + 3 + 3 + 4 + 4),
            "every edge was clicked twice"
        );
    }

    #[test]
    fn a_draft_is_kept_only_while_what_it_points_at_is_as_it_was() {
        let mut pr = Project::new();
        let a = pr.add_piece(Piece::rectangle(PieceId(0), "A", p(0.0, 0.0), 300.0, 400.0));
        let b = pr.add_piece(Piece::rectangle(
            PieceId(0),
            "B",
            p(500.0, 0.0),
            300.0,
            400.0,
        ));
        let first = SewDraft {
            a: SeamSide::new(a, Half::Drawn, 1, 1, true),
            seam: None,
            outline_edges: 4,
        };
        assert!(draft_fits(&pr, &first));

        let mut more = pr.clone();
        geom::split_edge_in(&mut more, a, 0, 0.5);
        assert!(
            !draft_fits(&more, &first),
            "an edge was added: edge 1 is another"
        );
        let mut fewer = pr.clone();
        assert!(geom::remove_vertex_in(&mut fewer, a, 0));
        assert!(!draft_fits(&fewer, &first), "an edge was taken away");
        let mut gone = pr.clone();
        gone.remove_piece(a);
        assert!(!draft_fits(&gone, &first), "its piece was deleted");
        let mut folded = pr.clone();
        folded.piece_mut(a).unwrap().fold = Some(1);
        assert!(!draft_fits(&folded, &first), "its edge became the fold");
        let pale = SewDraft {
            a: SeamSide::new(a, Half::Pale, 0, 1, true),
            ..first
        };
        assert!(!draft_fits(&pr, &pale), "no pale half without a fold");
        assert!(draft_fits(&folded, &pale));

        // Once the seam is made, the seam is what the draft needs.
        let id = pr.add_seam(
            SeamSide::new(a, Half::Drawn, 1, 1, true),
            SeamSide::new(b, Half::Drawn, 3, 1, false),
        );
        let extending = SewDraft {
            seam: Some(id),
            ..first
        };
        assert!(draft_fits(&pr, &extending));
        pr.remove_seam(id);
        assert!(!draft_fits(&pr, &extending), "the seam was undone");
    }
}
