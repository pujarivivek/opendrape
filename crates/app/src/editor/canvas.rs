//! Pointer and keyboard handling on the pattern table, one tool at a time.

use super::length_box::{BoxKind, LengthBox, Outcome};
use super::{EMPTY_TABLE, HIT_PX, PatternEditor, Selection, Tool, project_bounds};
use crate::tr;
use egui::{Event, Key, PointerButton, Response, Sense, vec2};
use opendrape_core::{Edge, HandleEnd, Piece, PieceId, Point2, Project, Units, VertexKind};
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
    /// Edit tool: what is being dragged.
    pub drag: Option<Drag>,
}

/// What placing a pen point did.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Placed {
    /// A point was added to the draft.
    Added,
    /// The point closed the draft into a new piece.
    Finished,
    /// The point should have closed the draft, but the piece was refused (a notice says why).
    /// The draft is kept.
    Kept,
    /// The spot is already a point of the draft: nothing was added.
    Duplicate,
}

/// Something under the pointer, in the order the edit tool prefers them.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Hit {
    /// A curve handle of an edge of the selected piece.
    Handle(PieceId, usize, HandleEnd),
    Vertex(PieceId, usize),
    Edge(PieceId, usize),
    Inside(PieceId),
    /// A notch's mark: the shape and the stored notch's index.
    Notch(PieceId, usize),
}

impl Hit {
    fn piece(self) -> PieceId {
        match self {
            Self::Handle(id, ..)
            | Self::Vertex(id, _)
            | Self::Edge(id, _)
            | Self::Inside(id)
            | Self::Notch(id, _) => id,
        }
    }

    fn selection(self) -> Selection {
        match self {
            Self::Handle(id, i, _) | Self::Edge(id, i) => Selection::Edge(id, i),
            Self::Vertex(id, i) => Selection::Vertex(id, i),
            Self::Inside(id) => Selection::Piece(id),
            Self::Notch(id, k) => Selection::Notch(id, k),
        }
    }
}

/// An edit-tool drag. Every frame recomputes the piece from how it was when the drag began
/// plus the pointer's offset from where it grabbed, so no movement is lost between frames.
pub(super) struct Drag {
    /// The stored piece as it was when the drag began.
    original: Piece,
    hit: Hit,
    grab: Point2,
    /// The dragged shape's kind: a twin's movements are mirrored back onto the stored piece.
    kind: geom::ShapeKind,
    /// A refused move of this drag has been reported already: one notice per drag, not one per
    /// frame.
    refusal_noted: bool,
}

impl Drag {
    /// The stored piece after the pointer moved `d` (in the dragged shape's coordinates).
    fn moved(&self, d: Point2) -> Piece {
        let o = &self.original;
        let mut p = o.clone();
        let twin = matches!(self.kind, geom::ShapeKind::Twin { .. });
        let ds = self.kind.to_stored_delta(d);
        match self.hit {
            Hit::Vertex(_, i) => p.move_vertex(i, o.vertices[i].pos + ds),
            Hit::Handle(_, i, end) => {
                if let Edge::Curve { c1, c2 } = o.edges[i] {
                    let from = match end {
                        HandleEnd::Start => c1,
                        HandleEnd::End => c2,
                    };
                    p.set_handle(i, end, from + ds);
                }
            }
            Hit::Edge(_, i) => {
                let j = o.next(i);
                p.move_vertex(i, o.vertices[i].pos + ds);
                p.move_vertex(j, o.vertices[j].pos + ds);
            }
            // A twin moves on its own (its offset); the stored piece moves without its twin.
            Hit::Inside(_) if twin => {
                if let Some(t) = &mut p.twin {
                    t.offset = t.offset + d;
                }
            }
            Hit::Inside(_) => p.translate(d),
            // Notches are not dragged: no drag starts on one.
            Hit::Notch(..) => {}
        }
        p
    }
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
            Tool::Edit | Tool::AddPoint | Tool::Notch => w,
        });
        self.canvas.cursor = cursor;
        // A press on the canvas dismisses the last notice, unless a property field owned the
        // keys this frame: that press is only the click-away that applied (or refused) the text.
        if keys_free && response.contains_pointer() && ui.input(|i| i.pointer.any_pressed()) {
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
            Tool::Edit => self.edit_tool(&response, press, pointer, tol),
            Tool::AddPoint => self.add_point_tool(&response, hover, pointer, tol),
            Tool::Notch => self.notch_tool(&response, hover, pointer, tol),
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
        let drawn = self.cache.shapes(self.doc.project(), self.view.zoom);
        self.paint(&painter, rect, ui.visuals().dark_mode, &drawn);
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
            self.canvas.pen_dragging = self.pen_place(at, tol) == Placed::Added;
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
    fn pen_place(&mut self, at: Point2, tol: f64) -> Placed {
        let pen = &self.canvas.pen;
        let on_first = pen.first().is_some_and(|p| p.pos.distance(at) <= tol);
        let on_last = pen.last().is_some_and(|p| p.pos.distance(at) <= tol);
        if pen.len() >= 3 && (on_first || on_last) {
            return if self.finish_pen() {
                Placed::Finished
            } else {
                Placed::Kept
            };
        }
        if pen.iter().any(|p| p.pos.distance(at) <= tol) {
            return Placed::Duplicate;
        }
        self.canvas.pen.push(PenPoint {
            pos: at,
            handle: None,
        });
        Placed::Added
    }

    /// Turns the pen points into a piece. Returns false, with a notice and the points kept,
    /// when that isn't possible (fewer than 3 points, or the pattern would become too large).
    fn finish_pen(&mut self) -> bool {
        if self.canvas.pen.len() < 3 {
            self.notice = Some(tr!("notice-need-three-points"));
            return false;
        }
        let mut piece = pen_piece(&self.canvas.pen);
        let id = self.doc.edit(|p| {
            piece.name = p.next_piece_name(&tr!("piece-default-name"));
            p.add_piece(piece)
        });
        if self.note_if_refused() {
            return false; // keep the draft: the student's points are not lost
        }
        self.canvas.pen.clear();
        self.canvas.length_box = None;
        self.selection = Selection::Piece(id);
        true
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
        let shapes = geom::shapes(self.doc.project());
        let vertices = shapes
            .iter()
            .flat_map(|s| s.piece.vertices.iter().map(|v| v.pos));
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
            Tool::Notch => self.notch_box_on_digits(ui, response),
            Tool::Edit => {
                if pressed(Key::Escape) {
                    self.selection = Selection::None;
                }
                if pressed(Key::Delete) || pressed(Key::Backspace) {
                    self.delete_selection();
                }
            }
            _ => {}
        }
    }

    /// A digit typed while drawing opens the number box with that digit in it.
    fn open_box_on_digits(&mut self, ui: &egui::Ui, response: &Response) {
        let typed = take_typed_digits(ui);
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
                        // the piece, and landing on any other pen point is refused.
                        if self.pen_place(pos, TYPED_SNAP_MM) == Placed::Duplicate {
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
            BoxKind::NotchDistance {
                shape,
                source,
                edge,
                from_end,
            } => {
                let len = self
                    .doc
                    .project()
                    .piece(source)
                    .map_or(0.0, |p| geom::edge_length(p, edge));
                let d = Units::parse(&number_box.first)
                    .map(|v| units.to_mm(v))
                    .filter(|d| d.is_finite() && (0.0..=len).contains(d));
                match d {
                    Some(d) => {
                        self.add_notch(shape, source, edge, if from_end { len - d } else { d })
                    }
                    None => self.notice = Some(tr!("notice-bad-notch")),
                }
            }
        }
    }

    fn edit_tool(
        &mut self,
        response: &Response,
        press: Option<Point2>,
        pointer: Option<Point2>,
        tol: f64,
    ) {
        if response.drag_started_by(PointerButton::Primary)
            && let Some(grab) = press
        {
            let shapes = self.shapes();
            if let Some(hit) = self.hit(&shapes, grab, tol)
                && !matches!(hit, Hit::Notch(..))
                && let Some(shape) = shapes.iter().find(|s| s.id == hit.piece())
                && let Some(original) = self.doc.project().piece(shape.source).cloned()
            {
                self.selection = hit.selection();
                self.doc.begin_gesture();
                self.canvas.drag = Some(Drag {
                    original,
                    hit,
                    grab,
                    kind: shape.kind,
                    refusal_noted: false,
                });
            }
        }
        if response.dragged_by(PointerButton::Primary)
            && let (Some(drag), Some(now)) = (&self.canvas.drag, pointer)
        {
            let moved = drag.moved(now - drag.grab);
            let id = moved.id;
            self.doc.gesture_edit(|p| {
                if let Some(piece) = p.piece_mut(id) {
                    *piece = moved;
                }
            });
            // The move was refused (a point dragged over a fold line, or too large a piece):
            // say so, as a typed change would, but only the first time in this drag.
            if self.doc.last_change_refused()
                && let Some(drag) = &mut self.canvas.drag
                && !std::mem::replace(&mut drag.refusal_noted, true)
            {
                self.note_if_refused();
            }
        }
        if response.drag_stopped() && self.canvas.drag.take().is_some() {
            self.doc.end_gesture();
        }
        if response.clicked()
            && let Some(at) = pointer
        {
            self.selection = self
                .hit(&self.shapes(), at, tol)
                .map_or(Selection::None, Hit::selection);
        }
    }

    /// What the edit tool picks at `w`: the selected shape's curve handles first, then notch
    /// marks, then points, edges and insides, topmost shape first. Points and edges of a fold's pale half
    /// are not editable: they pick the piece.
    fn hit(&self, shapes: &[geom::Shape], w: Point2, tol: f64) -> Option<Hit> {
        if let Some(sel) = self.selection.piece()
            && let Some(s) = shapes.iter().find(|s| s.id == sel)
        {
            for (k, edge) in s.piece.edges.iter().enumerate() {
                let (Some(i), Edge::Curve { c1, c2 }) = (s.stored_edge(k), *edge) else {
                    continue;
                };
                if c1.distance(w) <= tol {
                    return Some(Hit::Handle(s.id, i, HandleEnd::Start));
                }
                if c2.distance(w) <= tol {
                    return Some(Hit::Handle(s.id, i, HandleEnd::End));
                }
            }
        }
        // Notch marks sit out on the cut line, so they never hide a point or an edge.
        for s in shapes.iter().rev() {
            let stored = self
                .doc
                .project()
                .owner(s.id)
                .map_or(0, |(p, _)| p.notches.len());
            for (j, notch) in s.piece.notches.iter().enumerate() {
                let near = geom::notch_marks(&s.piece, notch)
                    .iter()
                    .any(|[a, b]| segment_distance(w, *a, *b) <= tol);
                if near && stored > 0 {
                    // A fold's pale half repeats the stored notches after them, in order.
                    return Some(Hit::Notch(s.id, j % stored));
                }
            }
        }
        let topmost = || shapes.iter().rev();
        topmost()
            .find_map(|s| {
                (0..s.piece.len()).find_map(|k| {
                    let i = s.stored_vertex(k)?;
                    (s.piece.vertices[k].pos.distance(w) <= tol).then_some(Hit::Vertex(s.id, i))
                })
            })
            .or_else(|| {
                topmost().find_map(|s| {
                    geom::nearest_edge(&s.piece, w)
                        .filter(|e| e.2 <= tol)
                        .map(|(k, _, _)| match s.stored_edge(k) {
                            Some(i) => Hit::Edge(s.id, i),
                            None => Hit::Inside(s.id),
                        })
                })
            })
            .or_else(|| {
                topmost()
                    .find(|s| geom::contains(&s.piece, w))
                    .map(|s| Hit::Inside(s.id))
            })
    }

    /// Deletes the selected point or piece. A piece keeps at least 3 points.
    pub(super) fn delete_selection(&mut self) {
        match self.selection {
            Selection::Piece(id) => {
                self.doc.edit(|p| p.remove_piece(id));
                if !self.note_if_refused() {
                    self.selection = Selection::None;
                }
            }
            Selection::Vertex(id, i) => {
                let removed = self.doc.edit(|p| {
                    p.owner_mut(id)
                        .is_some_and(|(piece, _)| geom::remove_vertex(piece, i))
                });
                if self.note_if_refused() {
                    // The notice says why.
                } else if removed {
                    self.selection = Selection::Piece(id);
                } else {
                    self.notice = Some(tr!("notice-min-points"));
                }
            }
            Selection::Notch(id, k) => {
                self.doc.edit(|p| {
                    if let Some((pc, _)) = p.owner_mut(id)
                        && k < pc.notches.len()
                    {
                        pc.notches.remove(k);
                    }
                });
                if !self.note_if_refused() {
                    self.selection = Selection::Piece(id);
                }
            }
            Selection::Edge(..) | Selection::None => {}
        }
    }

    fn add_point_tool(
        &mut self,
        response: &Response,
        hover: Option<Point2>,
        pointer: Option<Point2>,
        tol: f64,
    ) {
        self.canvas.preview = hover
            .and_then(|w| nearest_edge(self.doc.project(), w, tol))
            .map(|hit| hit.4);
        if response.clicked()
            && let Some(at) = pointer
            && let Some((shape, source, i, t, _)) = nearest_edge(self.doc.project(), at, tol)
        {
            // A twin keeps its edges' direction, so `t` is the same on the stored piece.
            let split = self.doc.edit(|p| {
                p.piece_mut(source)
                    .and_then(|piece| geom::split_edge(piece, i, t))
            });
            if !self.note_if_refused() {
                match split {
                    Some(v) => self.selection = Selection::Vertex(shape, v),
                    None => self.notice = Some(tr!("notice-too-close")),
                }
            }
        }
    }

    pub(super) fn add_rectangle(&mut self, min: Point2, width: f64, height: f64) {
        let id = self.doc.edit(|p| {
            let name = p.next_piece_name(&tr!("piece-default-name"));
            p.add_piece(Piece::rectangle(PieceId(0), name, min, width, height))
        });
        if !self.note_if_refused() {
            self.selection = Selection::Piece(id);
        }
    }
}

/// Takes the digits (and decimal marks) typed this frame out of the input, so the number box
/// created later this frame doesn't get them twice. Returns them, or "" if none were typed.
pub(super) fn take_typed_digits(ui: &egui::Ui) -> String {
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
    typed
}

/// Distance (mm) from `p` to the segment `a`–`b`.
fn segment_distance(p: Point2, a: Point2, b: Point2) -> f64 {
    let ab = b - a;
    let len2 = ab.x * ab.x + ab.y * ab.y;
    let t = if len2 < 1e-18 {
        0.0
    } else {
        (((p.x - a.x) * ab.x + (p.y - a.y) * ab.y) / len2).clamp(0.0, 1.0)
    };
    p.distance(a + ab * t)
}

/// A typed length the pattern can hold: the same range `geom::set_edge_length` accepts.
fn valid_length(mm: f64) -> bool {
    mm.is_finite() && (geom::MIN_EDGE_MM..=geom::MAX_EDGE_MM).contains(&mm)
}

/// The editable outline edge nearest to `w` within `tol` mm, over all shapes:
/// (shape, stored piece, stored edge, curve parameter, the point on the shape).
pub(super) fn nearest_edge(
    project: &Project,
    w: Point2,
    tol: f64,
) -> Option<(PieceId, PieceId, usize, f64, Point2)> {
    geom::shapes(project)
        .iter()
        .filter_map(|s| {
            let (k, t, d) = geom::nearest_edge(&s.piece, w)?;
            let i = s.stored_edge(k)?;
            (d <= tol).then(|| (s.id, s.source, i, t, geom::point_on_edge(&s.piece, k, t), d))
        })
        .min_by(|a, b| a.5.total_cmp(&b.5))
        .map(|(id, source, i, t, at, _)| (id, source, i, t, at))
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
