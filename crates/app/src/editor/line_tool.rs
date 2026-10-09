//! The Internal line tool (L): draws a line inside a piece the way the pen draws a piece.
//! Click points, and press and drag for curve points. Return finishes an open line; clicking
//! the first point closes a shape. Also what keeps lines inside their piece, when drawing and
//! when the edit tool drags them.

use super::canvas::{PenPoint, Placed, pen_piece};
use super::{PatternEditor, Selection};
use crate::tr;
use egui::{PointerButton, Response};
use opendrape_core::{Edge, InternalLine, LineKind, Piece, Point2};
use opendrape_geom as geom;

/// How closely (mm) a curved line is flattened to be tested.
const FLATTEN_MM: f64 = 0.5;
/// Longest gap (mm) between the points a line is tested at along its length: a run between two
/// points inside a piece can still cross a narrow slot in its outline, and the narrowest a
/// student cuts is a few millimetres.
const TEST_STEP_MM: f64 = 2.0;
/// Most points one check tests, so a very long line (a hostile file's) stays quick: a longer
/// line is tested at a wider spacing.
const MAX_TESTS: f64 = 20_000.0;

impl PatternEditor {
    pub(super) fn line_tool(
        &mut self,
        response: &Response,
        press: Option<Point2>,
        pointer: Option<Point2>,
        tol: f64,
    ) {
        if response.drag_started_by(PointerButton::Primary)
            && let Some(at) = press
        {
            self.canvas.line_dragging = self.line_place(at, tol) == Placed::Added;
        }
        if self.canvas.line_dragging
            && response.dragged_by(PointerButton::Primary)
            && let Some(now) = pointer
            && let Some(last) = self.canvas.line.last_mut()
        {
            last.handle = (now.distance(last.pos) > tol).then_some(now);
        }
        if response.drag_stopped() {
            self.canvas.line_dragging = false;
        }
        if response.clicked()
            && let Some(at) = pointer
        {
            self.line_place(at, tol);
        }
    }

    /// Adds a point to the line being drawn, or finishes it when `at` is on its first point (a
    /// closed shape) or its last (an open line). The first point decides which piece the line
    /// is in; every point must be inside that piece.
    fn line_place(&mut self, at: Point2, tol: f64) -> Placed {
        let line = &self.canvas.line;
        let count = line.len();
        let on_first = line.first().is_some_and(|p| p.pos.distance(at) <= tol);
        let on_last = line.last().is_some_and(|p| p.pos.distance(at) <= tol);
        let closing = count >= 3 && on_first;
        if closing || (count >= 2 && on_last) {
            return if self.finish_line(closing) {
                Placed::Finished
            } else {
                Placed::Kept
            };
        }
        if line.iter().any(|p| p.pos.distance(at) <= tol) {
            return Placed::Duplicate;
        }
        let shapes = self.shapes();
        let owner = match self.canvas.line_owner {
            Some(id) => shapes.iter().find(|s| s.id == id),
            None => shapes.iter().rev().find(|s| inside(&s.piece, at)),
        };
        let Some(shape) = owner.filter(|s| inside(&s.piece, at)) else {
            self.notice = Some(tr!("notice-line-outside"));
            return Placed::Duplicate;
        };
        self.canvas.line_owner = Some(shape.id);
        // A new line leaves the previous selection behind: Backspace past the draft's last
        // point must not reach the line (or notch) that was selected before.
        if self.canvas.line.is_empty()
            && matches!(self.selection, Selection::Line(..) | Selection::Notch(..))
        {
            self.selection = Selection::Piece(shape.id);
        }
        self.canvas.line.push(PenPoint {
            pos: at,
            handle: None,
        });
        Placed::Added
    }

    /// Stores the drafted line on the piece it was drawn in. Returns false, keeping the draft,
    /// when it is too short, leaves the piece, or the change is refused (a notice says why).
    pub(super) fn finish_line(&mut self, closed: bool) -> bool {
        if self.canvas.line.len() < if closed { 3 } else { 2 } {
            self.notice = Some(tr!("notice-line-short"));
            return false;
        }
        let Some(shape) = self
            .canvas
            .line_owner
            .and_then(|id| geom::shape_of(self.doc.project(), id))
        else {
            // The piece is gone: nothing to keep the draft for.
            self.canvas.line.clear();
            self.canvas.line_owner = None;
            return false;
        };
        let drawn = draft_line(&self.canvas.line, closed);
        // The whole line has to stay inside, not only the points the student clicked: a curve
        // can bulge out, and a straight run can cross a notch.
        if !stays_inside(&shape.piece, &drawn) {
            self.notice = Some(tr!("notice-line-outside"));
            return false;
        }
        let line = shape.line_to_stored(&drawn);
        let index = self.doc.edit(|p| {
            p.piece_mut(shape.source).map(|pc| {
                pc.lines.push(line);
                pc.lines.len() - 1
            })
        });
        if self.note_if_refused() {
            return false;
        }
        self.canvas.line.clear();
        self.canvas.line_owner = None;
        if let Some(l) = index {
            self.selection = Selection::Line(shape.id, l);
        }
        true
    }
}

/// The line the drafted points make: the pen's piece, without its closing edge when open.
fn draft_line(points: &[PenPoint], closed: bool) -> InternalLine {
    let draft = pen_piece(points);
    let mut line = InternalLine {
        vertices: draft.vertices,
        edges: draft.edges,
        closed,
        kind: LineKind::Marking,
    };
    if !closed {
        line.edges.pop(); // the pen's closing edge
    }
    line
}

/// Inside `piece`'s outline, or on it (within 0.5 mm).
pub(super) fn inside(piece: &Piece, p: Point2) -> bool {
    geom::contains(piece, p)
        || geom::nearest_edge(piece, p).is_some_and(|e| e.2 <= geom::ON_OUTLINE_MM)
}

/// Whether all of `line` is inside `full`, the whole piece: its drawn points, and the runs
/// between them tested every [`TEST_STEP_MM`] or so.
fn stays_inside(full: &Piece, line: &InternalLine) -> bool {
    all_tested_points(line, |p| inside(full, p))
}

/// Whether `ok` holds at the points of `line` that are tested: each drawn point, and points along
/// every run between them. A very long line is tested at a wider spacing, so there are at most
/// [`MAX_TESTS`] along it, plus one for each run.
fn all_tested_points(line: &InternalLine, mut ok: impl FnMut(Point2) -> bool) -> bool {
    let points = geom::line_points(line, FLATTEN_MM);
    let length: f64 = points.windows(2).map(|run| run[0].distance(run[1])).sum();
    let step = TEST_STEP_MM.max(length / MAX_TESTS);
    let runs_ok = points.windows(2).all(|run| {
        let (a, b) = (run[0], run[1]);
        let tests = ((a.distance(b) / step).ceil() as usize).max(1);
        (0..tests).all(|i| ok(a.lerp(b, i as f64 / tests as f64)))
    });
    runs_ok && points.last().is_none_or(|p| ok(*p))
}

/// Whether every point of `piece`'s line `l` (in stored coordinates) is inside the whole piece.
pub(super) fn line_inside(piece: &Piece, l: usize) -> bool {
    stays_inside(&geom::unfolded(piece), &piece.lines[l])
}

/// Moves vertex `k` of `line` to `to`, carrying the curve handles next to it.
pub(super) fn move_line_vertex(line: &mut InternalLine, k: usize, to: Point2) {
    let d = to - line.vertices[k].pos;
    line.vertices[k].pos = to;
    let n = line.vertices.len();
    let before = if k > 0 {
        Some(k - 1)
    } else if line.closed {
        Some(n - 1)
    } else {
        None
    };
    if let Some(e) = before
        && let Edge::Curve { c2, .. } = &mut line.edges[e]
    {
        *c2 = *c2 + d;
    }
    if k < line.edges.len()
        && let Edge::Curve { c1, .. } = &mut line.edges[k]
    {
        *c1 = *c1 + d;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(x: f64, y: f64) -> Point2 {
        Point2::new(x, y)
    }

    fn curve(c1: Point2, c2: Point2) -> Edge {
        Edge::Curve { c1, c2 }
    }

    #[test]
    fn moving_a_point_carries_the_handles_on_either_side() {
        let mut line = InternalLine::open(&[p(0.0, 0.0), p(10.0, 0.0), p(20.0, 0.0)]);
        line.edges = vec![
            curve(p(1.0, 1.0), p(9.0, 1.0)),
            curve(p(11.0, 1.0), p(19.0, 1.0)),
        ];
        move_line_vertex(&mut line, 1, p(10.0, 5.0));
        assert_eq!(line.vertices[1].pos, p(10.0, 5.0));
        assert_eq!(line.edges[0], curve(p(1.0, 1.0), p(9.0, 6.0)));
        assert_eq!(line.edges[1], curve(p(11.0, 6.0), p(19.0, 1.0)));
    }

    #[test]
    fn the_ends_of_an_open_line_have_one_handle_and_a_closed_line_wraps_round() {
        let mut open = InternalLine::open(&[p(0.0, 0.0), p(10.0, 0.0), p(20.0, 0.0)]);
        open.edges = vec![
            curve(p(1.0, 1.0), p(9.0, 1.0)),
            curve(p(11.0, 1.0), p(19.0, 1.0)),
        ];
        move_line_vertex(&mut open, 0, p(0.0, 2.0));
        assert_eq!(open.edges[0], curve(p(1.0, 3.0), p(9.0, 1.0)));
        move_line_vertex(&mut open, 2, p(20.0, 2.0));
        assert_eq!(open.edges[1], curve(p(11.0, 1.0), p(19.0, 3.0)));

        let mut ring = InternalLine::polygon(&[p(0.0, 0.0), p(10.0, 0.0), p(5.0, 10.0)]);
        ring.edges = vec![
            curve(p(2.0, -1.0), p(8.0, -1.0)),
            Edge::Line,
            curve(p(2.0, 6.0), p(1.0, 2.0)),
        ];
        move_line_vertex(&mut ring, 0, p(0.0, 1.0));
        assert_eq!(ring.edges[0], curve(p(2.0, 0.0), p(8.0, -1.0)));
        assert_eq!(ring.edges[2], curve(p(2.0, 6.0), p(1.0, 3.0)));
    }

    #[test]
    fn a_straight_run_across_a_notch_is_not_inside() {
        let l = Piece::polygon(
            opendrape_core::PieceId(1),
            "L",
            &[
                p(0.0, 0.0),
                p(300.0, 0.0),
                p(300.0, 100.0),
                p(100.0, 100.0),
                p(100.0, 400.0),
                p(0.0, 400.0),
            ],
        );
        let across = InternalLine::open(&[p(50.0, 350.0), p(250.0, 50.0)]);
        assert!(!stays_inside(&l, &across));
        let round = InternalLine::open(&[p(50.0, 350.0), p(50.0, 50.0), p(250.0, 50.0)]);
        assert!(stays_inside(&l, &round));
        let outside = InternalLine::open(&[p(50.0, 50.0), p(350.0, 50.0)]);
        assert!(!stays_inside(&l, &outside));
        // On the outline counts as inside.
        let along = InternalLine::open(&[p(0.0, 10.0), p(0.0, 390.0)]);
        assert!(stays_inside(&l, &along));
    }

    /// 1200 × 100 mm with an 8 mm wide, 40 mm deep slot cut in from the top at x = 602..610.
    fn slotted() -> Piece {
        Piece::polygon(
            opendrape_core::PieceId(1),
            "Slotted",
            &[
                p(0.0, 0.0),
                p(1200.0, 0.0),
                p(1200.0, 100.0),
                p(610.0, 100.0),
                p(610.0, 60.0),
                p(602.0, 60.0),
                p(602.0, 100.0),
                p(0.0, 100.0),
            ],
        )
    }

    #[test]
    fn a_long_line_is_tested_finely_enough_to_find_a_narrow_slot() {
        let piece = slotted();
        let across = InternalLine::open(&[p(50.0, 80.0), p(1150.0, 80.0)]);
        assert!(!stays_inside(&piece, &across));
        let short = InternalLine::open(&[p(50.0, 80.0), p(590.0, 80.0)]);
        assert!(stays_inside(&piece, &short));
        let beneath = InternalLine::open(&[p(50.0, 40.0), p(1150.0, 40.0)]);
        assert!(stays_inside(&piece, &beneath));
    }

    #[test]
    fn a_huge_line_is_tested_at_a_bounded_number_of_points() {
        // 2,000 runs of 900 m, in a 1 km square (the biggest the model allows): every 2 mm
        // would be 900 million tests, so the spacing widens.
        let mut zigzag = vec![p(10.0, 10.0)];
        for k in 0..2_000 {
            zigzag.push(p(
                10.0 + f64::from(k % 2) * 900_000.0,
                10.0 + f64::from(k) * 400.0,
            ));
        }
        let mut tested = 0_usize;
        assert!(all_tested_points(&InternalLine::open(&zigzag), |_| {
            tested += 1;
            true
        }));
        assert!(tested <= 20_000 + 2_001, "{tested}"); // the cap, and one for each run
        // And an ordinary line is still tested every 2 mm.
        let mut tested = 0_usize;
        all_tested_points(&InternalLine::open(&[p(0.0, 0.0), p(1000.0, 0.0)]), |_| {
            tested += 1;
            true
        });
        assert_eq!(tested, 501);
    }
}
