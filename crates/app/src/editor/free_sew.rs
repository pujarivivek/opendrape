//! The Free Sew tool (F): a seam between any two points of an outline, as a sleeve cap is sewn
//! into an armhole. Click where the first side starts on a piece, then where it ends; then
//! where the second side starts and ends, which makes the seam (its start meets the first
//! side's start). A side runs the shorter way round between its ends; Shift-click its end to
//! take the long way. Points snap to the corners, the middle and the notches of the edge under
//! the pointer. Esc cancels; clicking a seam's line selects the seam.

use super::{PatternEditor, Selection};
use crate::tr;
use egui::Response;
use opendrape_core::{
    Half, MIN_SIDE_MM, ModelError, OutlinePos, PieceId, Point2, Project, SeamSide,
};
use opendrape_geom as geom;

/// A point on a shape's outline, as the Free Sew tool picks it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct OnOutline {
    pub shape: PieceId,
    pub half: Half,
    /// Where it is on the stored piece.
    pub pos: OutlinePos,
    /// Where it is on the shape (mm).
    pub at: Point2,
    /// How many edges the stored piece had: a point added or removed since makes `pos` name
    /// another place.
    pub outline_edges: usize,
}

/// A seam being sewn with the Free Sew tool.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct FreeDraft {
    /// Where the first side starts.
    pub start: OnOutline,
    /// The first side, once its end is picked.
    pub a: Option<SeamSide>,
    /// Where the second side starts, once picked.
    pub b_start: Option<OnOutline>,
}

/// The point of an outline under `w` within `tol` mm: on the nearest edge of any shape, snapped
/// to that edge's start, end, middle or a notch on it when one is within `tol` of `w`.
pub(crate) fn point_under(project: &Project, w: Point2, tol: f64) -> Option<OnOutline> {
    let shapes = geom::shapes(project);
    let (shape, j, t) = shapes
        .iter()
        .filter_map(|s| {
            let (j, t, d) = geom::nearest_edge(&s.piece, w)?;
            (d <= tol).then_some((s, j, t, d))
        })
        .min_by(|a, b| a.3.total_cmp(&b.3))
        .map(|(s, j, t, _)| (s, j, t))?;
    let piece = &shape.piece;
    let len = geom::edge_length(piece, j);
    let notches = piece
        .notches
        .iter()
        .filter(|n| n.edge == j)
        .map(|n| n.distance);
    let snapped = [0.0, len, len / 2.0]
        .into_iter()
        .chain(notches)
        .map(|d| (d, geom::point_at_distance(piece, j, d).distance(w)))
        .filter(|(_, gap)| *gap <= tol)
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(d, _)| d);
    let along = snapped.unwrap_or_else(|| geom::distance_along(piece, j, t));
    let (half, pos) = shape.outline_pos(j, along);
    Some(OnOutline {
        shape: shape.id,
        half,
        pos,
        at: geom::point_at_distance(piece, j, along),
        outline_edges: shape.stored_len(),
    })
}

/// Why a side can't be made between two points.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum NoSide {
    /// The end is on another shape, or the other half of a fold.
    OtherPiece,
    /// The way chosen round the outline crosses the fold line.
    Fold,
    /// It would be 1 mm long or less.
    TooShort,
    /// Part of it is sewn already.
    Sewn,
}

/// The side from `start` to `end` on the same shape and half: the shorter way round, or the
/// longer with `long`. It must cross no fold, be longer than [`MIN_SIDE_MM`], and share no more
/// than a point with a seam sewn already (mirror images included) or with `besides`.
pub(crate) fn side_between(
    project: &Project,
    start: &OnOutline,
    end: &OnOutline,
    long: bool,
    besides: Option<&SeamSide>,
) -> Result<SeamSide, NoSide> {
    if (start.shape, start.half) != (end.shape, end.half) {
        return Err(NoSide::OtherPiece);
    }
    // The same point twice is no side (not the whole outline round to it).
    if start.at.distance(end.at) <= 1e-6 {
        return Err(NoSide::TooShort);
    }
    let shape = geom::shape_of(project, start.shape).ok_or(NoSide::OtherPiece)?;
    let n = shape.stored_len();
    let way = |forward| {
        SeamSide {
            shape: start.shape,
            half: start.half,
            from: start.pos,
            to: end.pos,
            forward,
        }
        .tidy(n)
    };
    // Measured round the stored outline, fold edge and all, so that the shorter way is the
    // shorter even when it would cross the fold (and so be refused).
    let length = |side: &SeamSide| project.side_length(side).unwrap_or(0.0);
    let (forward, backward) = (way(true), way(false));
    let side = if (length(&forward) <= length(&backward)) != long {
        forward
    } else {
        backward
    };
    if side.spans(n).is_empty() || project.side_length(&side).is_some_and(|l| l <= MIN_SIDE_MM) {
        return Err(NoSide::TooShort);
    }
    if geom::side_runs(&shape, &side).is_none() {
        return Err(NoSide::Fold);
    }
    let sewn = project
        .all_seams()
        .into_iter()
        .flat_map(|(s, _)| [s.a, s.b])
        .chain(besides.copied());
    let len_of = |id| project.owner(id).map_or(0, |(p, _)| p.len());
    if sewn
        .into_iter()
        .any(|other| other.overlaps(&side, len_of(other.shape)))
    {
        return Err(NoSide::Sewn);
    }
    Ok(side)
}

/// The notice for a side that can't be made.
fn refusal(why: NoSide) -> String {
    match why {
        NoSide::OtherPiece => tr!("notice-free-same-piece"),
        NoSide::Fold => tr!("notice-sew-fold"),
        NoSide::TooShort => tr!("notice-free-short"),
        NoSide::Sewn => tr!("notice-free-sewn"),
    }
}

/// Whether `point` still names the place it was picked at in `project`.
fn still_there(project: &Project, point: &OnOutline) -> bool {
    geom::shape_of(project, point.shape).is_some_and(|s| {
        s.stored_len() == point.outline_edges && s.point_at(point.half, point.pos).is_some()
    })
}

impl PatternEditor {
    /// Drops the Free Sew draft if what it points at has changed (an undo, a redo, a deleted
    /// piece, a point added or removed).
    pub(super) fn drop_stale_free_sew(&mut self) {
        let project = self.doc.project();
        self.canvas.free = self.canvas.free.filter(|d| {
            still_there(project, &d.start) && d.b_start.is_none_or(|b| still_there(project, &b))
        });
    }

    pub(super) fn free_sew_tool(
        &mut self,
        response: &Response,
        pointer: Option<Point2>,
        tol: f64,
        shift: bool,
    ) {
        self.drop_stale_free_sew();
        if !response.clicked() {
            return;
        }
        let Some(w) = pointer else { return };
        // A click on a seam's line selects that seam (unless a seam is being sewn).
        if self.canvas.free.is_none()
            && let Some(seam) = self.seam_at(w, tol)
        {
            self.selection = Selection::Seam(seam);
            return;
        }
        let project = self.doc.project();
        let Some(hit) = point_under(project, w, tol) else {
            if super::sew_tool::on_a_fold_line(project, w, tol) {
                self.notice = Some(tr!("notice-sew-fold"));
            }
            return;
        };
        match self.canvas.free {
            None => {
                self.canvas.free = Some(FreeDraft {
                    start: hit,
                    a: None,
                    b_start: None,
                });
            }
            Some(d @ FreeDraft { a: None, .. }) => {
                match side_between(project, &d.start, &hit, shift, None) {
                    Ok(a) => {
                        self.canvas.free = Some(FreeDraft { a: Some(a), ..d });
                    }
                    Err(why) => self.notice = Some(refusal(why)),
                }
            }
            Some(d @ FreeDraft { b_start: None, .. }) => {
                self.canvas.free = Some(FreeDraft {
                    b_start: Some(hit),
                    ..d
                });
            }
            Some(FreeDraft {
                a: Some(a),
                b_start: Some(b_start),
                ..
            }) => match side_between(project, &b_start, &hit, shift, Some(&a)) {
                Ok(b) => self.make_free_seam(a, b),
                Err(why) => self.notice = Some(refusal(why)),
            },
        }
    }

    /// Sews `a` to `b`, as one undo step.
    fn make_free_seam(&mut self, a: SeamSide, b: SeamSide) {
        let id = self.doc.edit(|p| p.add_seam(a, b));
        if self.doc.last_change_refused() {
            // Both sides are free (they were checked), so what refuses a seam is that its
            // mirror image would sew outline that is sewn already; or, at the very limit, there
            // are too many seams.
            self.notice = Some(match self.doc.last_refusal() {
                Some(ModelError::BadSeam(_)) => tr!("notice-mirror-sewn"),
                _ => tr!("notice-refused"),
            });
            return;
        }
        self.canvas.free = None;
        self.selection = Selection::Seam(id);
    }

    /// What to draw while a seam is sewn with the Free Sew tool: the first side once made, and
    /// the side so far (its start, to the point under the pointer the way the next click would
    /// take it).
    pub(super) fn free_sew_drawing(&self, shift: bool) -> (Option<SeamSide>, Option<SeamSide>) {
        let Some(d) = self.canvas.free else {
            return (None, None);
        };
        let project = self.doc.project();
        let tol = self.view.mm(super::HIT_PX);
        let under = self
            .canvas
            .cursor
            .and_then(|w| point_under(project, w, tol));
        let start = if d.a.is_none() {
            Some(d.start)
        } else {
            d.b_start
        };
        let so_far = start
            .zip(under)
            .and_then(|(s, e)| side_between(project, &s, &e, shift, d.a.as_ref()).ok());
        (d.a, so_far)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use opendrape_core::{Notch, Piece};

    fn p(x: f64, y: f64) -> Point2 {
        Point2::new(x, y)
    }

    /// A 300 × 200 piece at the origin with a notch 100 mm along its top edge (which runs from
    /// (300,200) to (0,200)).
    fn piece() -> Project {
        let mut pr = Project::new();
        let mut a = Piece::rectangle(PieceId(0), "A", p(0.0, 0.0), 300.0, 200.0);
        a.notches = vec![Notch::new(2, 100.0)];
        pr.add_piece(a);
        pr
    }

    #[test]
    fn points_snap_to_corners_middles_and_notches() {
        let pr = piece();
        let at = |x, y| point_under(&pr, p(x, y), 5.0).unwrap();
        assert_eq!(at(2.0, -1.0).at, p(0.0, 0.0), "a corner");
        assert_eq!(at(148.0, 1.0).at, p(150.0, 0.0), "the bottom edge's middle");
        assert_eq!(at(203.0, 199.0).at, p(200.0, 200.0), "the notch");
        assert_eq!(at(203.0, 199.0).pos, OutlinePos::new(2, 1.0 / 3.0));
        assert_eq!(
            at(60.0, 1.0).at,
            p(60.0, 0.0),
            "anywhere else: under the pointer"
        );
        assert!(
            point_under(&pr, p(150.0, 100.0), 5.0).is_none(),
            "inside, off the outline"
        );
    }

    #[test]
    fn a_side_runs_the_shorter_way_round_unless_shift_asks_for_the_longer() {
        let pr = piece();
        let at = |x, y| point_under(&pr, p(x, y), 1.0).unwrap();
        // From the bottom-right corner to halfway up the right edge: 100 mm one way, 900 the
        // other.
        let (start, end) = (at(300.0, 0.0), at(300.0, 100.0));
        let short = side_between(&pr, &start, &end, false, None).unwrap();
        assert!((pr.side_length(&short).unwrap() - 100.0).abs() < 0.01);
        assert!(short.forward);
        let long = side_between(&pr, &start, &end, true, None).unwrap();
        assert!((pr.side_length(&long).unwrap() - 900.0).abs() < 0.01);
        assert!(!long.forward);
        assert_eq!(
            side_between(&pr, &start, &start, false, None),
            Err(NoSide::TooShort)
        );
    }

    #[test]
    fn a_side_over_sewn_outline_is_refused() {
        let mut pr = piece();
        let b = pr.add_piece(Piece::rectangle(
            PieceId(0),
            "B",
            p(500.0, 0.0),
            200.0,
            200.0,
        ));
        let a = pr.pieces[0].id;
        pr.add_seam(
            SeamSide {
                to: OutlinePos::new(0, 0.5),
                ..SeamSide::edges(a, Half::Drawn, 0, 0, true)
            },
            SeamSide::edges(b, Half::Drawn, 3, 3, false),
        );
        let at = |x, y| point_under(&pr, p(x, y), 1.0).unwrap();
        // The bottom edge is sewn from x = 0 to 150: from 100 to 200 overlaps it, from 150 on
        // only meets it.
        assert_eq!(
            side_between(&pr, &at(100.0, 0.0), &at(200.0, 0.0), false, None),
            Err(NoSide::Sewn)
        );
        assert!(side_between(&pr, &at(150.0, 0.0), &at(250.0, 0.0), false, None).is_ok());
        assert_eq!(
            side_between(&pr, &at(150.0, 0.0), &at(600.0, 0.0), false, None),
            Err(NoSide::OtherPiece)
        );
    }
}
