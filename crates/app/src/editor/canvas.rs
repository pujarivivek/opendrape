//! Pointer and keyboard handling on the pattern table, one tool at a time.

use super::length_box::{BoxKind, LengthBox, Outcome};
use super::{EMPTY_TABLE, HIT_PX, PatternEditor, Selection, Tool, project_bounds};
use crate::tr;
use egui::{Event, Key, PointerButton, Response, Sense, vec2};
use opendrape_core::{Edge, Piece, PieceId, Point2, Units, VertexKind};
use opendrape_geom as geom;

/// How close (mm) a typed pen point may land to an existing pen point and still count as being
/// on it. Typed lengths are exact, so there is no screen-point tolerance to borrow from clicks.
const TYPED_SNAP_MM: f64 = 1.0;

/// A point placed with the pen.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PenPoint {
    pub pos: Point2,
    /// The curve handle pulled out by dragging while placing the point: the edge leaving the
    /// point bends towards it, and the edge arriving is mirrored. `None` for a corner.
    pub handle: Option<Point2>,
}

#[derive(Default)]
pub(super) struct CanvasState {
    pub pen: Vec<PenPoint>,
    /// The last pen point is being dragged out into a curve point.
    pub pen_dragging: bool,
    /// Rectangle tool: the corner where the drag started.
    pub rect_start: Option<Point2>,
    pub length_box: Option<LengthBox>,
    /// The pointer on the pattern (snapped, for the pen and rectangle tools).
    pub cursor: Option<Point2>,
    /// Add-point tool: where a click would add the point.
    pub preview: Option<Point2>,
}

impl PatternEditor {
    pub(super) fn canvas_ui(&mut self, ui: &mut egui::Ui, keys_free: bool) {
        let (response, painter) = ui.allocate_painter(ui.available_size(), Sense::click_and_drag());
        let rect = response.rect;
        self.canvas_rect = rect;
        if rect.width() < 1.0 || rect.height() < 1.0 {
            return; // the window is too small to draw in
        }
        if std::mem::take(&mut self.fit_pending) {
            let (min, max) = project_bounds(self.doc.project()).unwrap_or(EMPTY_TABLE);
            self.view.fit(rect, min, max);
        }
        self.pan_and_zoom(ui, &response);

        let tol = self.view.mm(HIT_PX);
        let shift = ui.input(|i| i.modifiers.shift);
        let hover = response.hover_pos().map(|p| self.view.to_world(rect, p));
        let cursor = hover.map(|w| match self.tool {
            Tool::Pen => self.snap(w, tol, shift),
            Tool::Rectangle => self.snap(w, tol, false),
            Tool::Edit | Tool::AddPoint => w,
        });
        self.canvas.cursor = cursor;
        if response.clicked() || response.drag_started() {
            self.notice = None;
        }
        let press = ui
            .input(|i| i.pointer.press_origin())
            .map(|p| self.view.to_world(rect, p));
        let pointer = response
            .interact_pointer_pos()
            .map(|p| self.view.to_world(rect, p));
        let latest = ui
            .input(|i| i.pointer.latest_pos())
            .map(|p| self.view.to_world(rect, p));
        match self.tool {
            Tool::Pen => self.pen_tool(&response, press, pointer, tol, shift),
            Tool::Rectangle => self.rectangle_tool(&response, press, pointer, latest, tol),
            Tool::Edit | Tool::AddPoint => {} // Task 6
        }
        if keys_free {
            self.canvas_keys(ui, &response);
        }
        if let Some(mut number_box) = self.canvas.length_box.take() {
            match number_box.show(ui.ctx(), self.doc.project().units) {
                None => self.canvas.length_box = Some(number_box),
                Some(Outcome::Commit) => self.commit_box(&number_box),
                Some(Outcome::Cancel) => {}
            }
        }
        self.selection = self.selection.validated(self.doc.project());
        self.paint(&painter, rect, ui.visuals().dark_mode);
    }

    fn pan_and_zoom(&mut self, ui: &egui::Ui, response: &Response) {
        let rect = response.rect;
        if response.contains_pointer() {
            let (zoom, scroll) = ui.input(|i| (i.zoom_delta(), i.smooth_scroll_delta()));
            if zoom != 1.0
                && let Some(p) = response.hover_pos()
            {
                self.view.zoom_at(rect, p, f64::from(zoom));
            }
            // Two-finger scroll or the mouse wheel pans; pinch or Ctrl/Cmd + wheel zooms.
            self.view.pan += scroll;
        }
        if response.dragged_by(PointerButton::Middle)
            || response.dragged_by(PointerButton::Secondary)
        {
            self.view.pan += response.drag_delta();
        }
    }

    fn pen_tool(
        &mut self,
        response: &Response,
        press: Option<Point2>,
        pointer: Option<Point2>,
        tol: f64,
        shift: bool,
    ) {
        if response.drag_started_by(PointerButton::Primary)
            && let Some(at) = press
        {
            let at = self.snap(at, tol, shift);
            self.canvas.pen_dragging = self.pen_place(at, tol);
        }
        if self.canvas.pen_dragging
            && response.dragged_by(PointerButton::Primary)
            && let Some(now) = pointer
            && let Some(last) = self.canvas.pen.last_mut()
        {
            last.handle = (now.distance(last.pos) > tol).then_some(now);
        }
        if response.drag_stopped() {
            self.canvas.pen_dragging = false;
        }
        if response.clicked()
            && let Some(at) = pointer
        {
            let at = self.snap(at, tol, shift);
            self.pen_place(at, tol);
        }
    }

    /// Adds a pen point at `at`, or finishes the piece when `at` is on its first or last point.
    /// Returns whether a point was added.
    fn pen_place(&mut self, at: Point2, tol: f64) -> bool {
        let pen = &self.canvas.pen;
        let on_first = pen.first().is_some_and(|p| p.pos.distance(at) <= tol);
        let on_last = pen.last().is_some_and(|p| p.pos.distance(at) <= tol);
        if pen.len() >= 3 && (on_first || on_last) {
            self.finish_pen();
            return false;
        }
        if pen.iter().any(|p| p.pos.distance(at) <= tol) {
            return false; // the same spot again: nothing to add
        }
        self.canvas.pen.push(PenPoint {
            pos: at,
            handle: None,
        });
        true
    }

    fn finish_pen(&mut self) {
        if self.canvas.pen.len() < 3 {
            self.notice = Some(tr!("notice-need-three-points"));
            return;
        }
        let mut piece = pen_piece(&self.canvas.pen);
        self.canvas.pen.clear();
        self.canvas.length_box = None;
        let id = self.doc.edit(|p| {
            piece.name = p.next_piece_name(&tr!("piece-default-name"));
            p.add_piece(piece)
        });
        self.selection = Selection::Piece(id);
    }

    fn rectangle_tool(
        &mut self,
        response: &Response,
        press: Option<Point2>,
        pointer: Option<Point2>,
        latest: Option<Point2>,
        tol: f64,
    ) {
        if response.drag_started_by(PointerButton::Primary)
            && let Some(at) = press
        {
            self.canvas.rect_start = Some(self.snap(at, tol, false));
        }
        if response.drag_stopped()
            && let Some(start) = self.canvas.rect_start.take()
            && let Some(end) = latest
        {
            let end = self.snap(end, tol, false);
            let size = Point2::new((end.x - start.x).abs(), (end.y - start.y).abs());
            // A drag along a straight line has no area: no piece.
            if size.x > tol && size.y > tol {
                self.add_rectangle(
                    Point2::new(start.x.min(end.x), start.y.min(end.y)),
                    size.x,
                    size.y,
                );
            }
        }
        if response.clicked()
            && let Some(at) = pointer
        {
            let corner = self.snap(at, tol, false);
            let pos = self.view.to_screen(self.canvas_rect, at) + vec2(16.0, 16.0);
            self.canvas.length_box = Some(LengthBox::new(
                BoxKind::Rectangle(corner),
                String::new(),
                String::new(),
                pos,
            ));
        }
    }

    /// Where a pen or rectangle point at `w` lands. With Shift the pen keeps 45° steps from
    /// its last point; otherwise points snap to a nearby existing point.
    pub(super) fn snap(&self, w: Point2, tol: f64, shift: bool) -> Point2 {
        let pen = &self.canvas.pen;
        if shift && let Some(last) = pen.last() {
            return constrain_45(last.pos, w);
        }
        let first = pen.first().map(|p| p.pos);
        let vertices = self
            .doc
            .project()
            .pieces
            .iter()
            .flat_map(|p| p.vertices.iter().map(|v| v.pos));
        first
            .into_iter()
            .chain(vertices)
            .filter(|p| p.distance(w) <= tol)
            .min_by(|a, b| a.distance(w).total_cmp(&b.distance(w)))
            .unwrap_or(w)
    }

    fn canvas_keys(&mut self, ui: &egui::Ui, response: &Response) {
        let pressed = |k: Key| ui.input(|i| i.key_pressed(k));
        match self.tool {
            Tool::Pen if !self.canvas.pen.is_empty() => {
                self.open_box_on_digits(ui, response);
                if pressed(Key::Enter) {
                    self.finish_pen();
                }
                if pressed(Key::Escape) {
                    self.canvas.pen.clear();
                }
                if pressed(Key::Backspace) || pressed(Key::Delete) {
                    self.canvas.pen.pop();
                }
            }
            Tool::Rectangle if pressed(Key::Escape) => self.canvas.rect_start = None,
            _ => {} // Task 6 adds the edit-tool keys
        }
    }

    /// A digit typed while drawing opens the number box with that digit in it. The text events
    /// are taken out of the input so the box (created later this frame) doesn't get them twice.
    fn open_box_on_digits(&mut self, ui: &egui::Ui, response: &Response) {
        let mut typed = String::new();
        ui.input_mut(|i| {
            i.events.retain(|e| match e {
                Event::Text(t)
                    if !t.is_empty()
                        && t.chars()
                            .all(|c| c.is_ascii_digit() || c == '.' || c == ',') =>
                {
                    typed.push_str(t);
                    false
                }
                _ => true,
            })
        });
        let Some(last) = self.canvas.pen.last() else {
            return;
        };
        if typed.is_empty() {
            return;
        }
        let towards = self
            .canvas
            .cursor
            .map(|c| c - last.pos)
            .filter(|d| d.length() > 1e-9);
        let degrees = towards.map_or(0.0, |d| d.y.atan2(d.x).to_degrees());
        let pos = response.hover_pos().unwrap_or(response.rect.center()) + vec2(16.0, 16.0);
        self.canvas.length_box = Some(LengthBox::new(
            BoxKind::PenSegment,
            typed,
            angle_text(degrees),
            pos,
        ));
    }

    fn commit_box(&mut self, number_box: &LengthBox) {
        let units = self.doc.project().units;
        let length = |text: &str| {
            Units::parse(text)
                .map(|v| units.to_mm(v))
                .filter(|mm| valid_length(*mm))
        };
        match number_box.kind {
            BoxKind::PenSegment => {
                let angle = Units::parse(&number_box.second);
                match (length(&number_box.first), angle, self.canvas.pen.last()) {
                    (Some(mm), Some(degrees), Some(last)) => {
                        let r = degrees.to_radians();
                        let pos = last.pos + Point2::new(r.cos(), r.sin()) * mm;
                        // Accepted like a click: closing on the first or last point finishes
                        // the piece, and landing on any other pen point is refused. A finished
                        // piece empties the pen, so a non-empty pen that did not grow was refused.
                        if !self.pen_place(pos, TYPED_SNAP_MM) && !self.canvas.pen.is_empty() {
                            self.notice = Some(tr!("notice-too-close"));
                        }
                    }
                    _ => self.notice = Some(tr!("notice-bad-number")),
                }
            }
            BoxKind::Rectangle(at) => match (length(&number_box.first), length(&number_box.second))
            {
                (Some(w), Some(h)) => self.add_rectangle(at, w, h),
                _ => self.notice = Some(tr!("notice-bad-number")),
            },
        }
    }

    pub(super) fn add_rectangle(&mut self, min: Point2, width: f64, height: f64) {
        let id = self.doc.edit(|p| {
            let name = p.next_piece_name(&tr!("piece-default-name"));
            p.add_piece(Piece::rectangle(PieceId(0), name, min, width, height))
        });
        self.selection = Selection::Piece(id);
    }
}

/// A typed length the pattern can hold: the same range `geom::set_edge_length` accepts.
fn valid_length(mm: f64) -> bool {
    mm.is_finite() && (geom::MIN_EDGE_MM..=geom::MAX_EDGE_MM).contains(&mm)
}

/// The closed piece the pen points make. A point with a handle is smooth: the edge leaving it
/// starts towards the handle and the edge arriving ends mirrored through the point.
pub(super) fn pen_piece(points: &[PenPoint]) -> Piece {
    let corners: Vec<Point2> = points.iter().map(|p| p.pos).collect();
    let mut piece = Piece::polygon(PieceId(0), "", &corners);
    let n = points.len();
    for (i, a) in points.iter().enumerate() {
        if a.handle.is_some() {
            piece.vertices[i].kind = VertexKind::Smooth;
        }
        let b = points[(i + 1) % n];
        if a.handle.is_none() && b.handle.is_none() {
            continue;
        }
        let c1 = a.handle.unwrap_or_else(|| a.pos.lerp(b.pos, 1.0 / 3.0));
        let c2 = b
            .handle
            .map_or_else(|| a.pos.lerp(b.pos, 2.0 / 3.0), |h| b.pos - (h - b.pos));
        piece.edges[i] = Edge::Curve { c1, c2 };
    }
    piece
}

/// `to`, turned about `from` to the nearest multiple of 45°.
fn constrain_45(from: Point2, to: Point2) -> Point2 {
    let d = to - from;
    let step = std::f64::consts::FRAC_PI_4;
    let a = (d.y.atan2(d.x) / step).round() * step;
    from + Point2::new(a.cos(), a.sin()) * d.length()
}

/// An angle as the number box shows it: 0.0 to 359.9, one decimal, never "-0.0" or "360.0".
fn angle_text(degrees: f64) -> String {
    let d = ((degrees * 10.0).round() / 10.0).rem_euclid(360.0) + 0.0;
    format!("{d:.1}")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(x: f64, y: f64) -> Point2 {
        Point2::new(x, y)
    }

    #[test]
    fn pen_piece_mirrors_curve_handles() {
        let pts = [
            PenPoint {
                pos: p(0.0, 0.0),
                handle: None,
            },
            PenPoint {
                pos: p(100.0, 0.0),
                handle: Some(p(130.0, 20.0)),
            },
            PenPoint {
                pos: p(50.0, 80.0),
                handle: None,
            },
        ];
        let piece = pen_piece(&pts);
        assert_eq!(piece.vertices[1].kind, VertexKind::Smooth);
        let Edge::Curve { c2, .. } = piece.edges[0] else {
            panic!()
        };
        assert_eq!(c2, p(70.0, -20.0));
        let Edge::Curve { c1, .. } = piece.edges[1] else {
            panic!()
        };
        assert_eq!(c1, p(130.0, 20.0));
        assert_eq!(piece.edges[2], Edge::Line);
    }

    #[test]
    fn constrain_45_snaps_directions() {
        let q = constrain_45(p(0.0, 0.0), p(100.0, 10.0));
        assert_eq!(q.y, 0.0);
        let d = constrain_45(p(0.0, 0.0), p(50.0, 60.0));
        assert!((d.x - d.y).abs() < 1e-9 && (d.length() - p(50.0, 60.0).length()).abs() < 1e-9);
    }

    #[test]
    fn angle_text_is_tidy() {
        assert_eq!(angle_text(-0.000_01), "0.0");
        assert_eq!(angle_text(359.97), "0.0");
        assert_eq!(angle_text(-90.0), "270.0");
        assert_eq!(angle_text(45.04), "45.0");
    }
}
