//! Drawing the pattern table: grid, pieces, the selection, and drafts in progress.

use super::{
    PatternEditor, Selection, Tool,
    cache::Drawn,
    canvas::{PenPoint, pen_piece},
};
use crate::tr;
use egui::{Align2, Color32, FontId, Painter, Pos2, Rect, Shape, Stroke, StrokeKind, vec2};
use opendrape_core::{Edge, LineKind, Point2};
use opendrape_geom as geom;
use std::rc::Rc;

/// How far (screen points) beyond the canvas a dashed line is still drawn: more than a dash,
/// so a dash cut off at the edge of the canvas still reaches it.
const CLIP_MARGIN: f32 = 20.0;

/// A pin's marker, on the pattern table and in the 3D view.
pub(crate) const PIN_COLOUR: Color32 = Color32::from_rgb(200, 30, 60);

struct Palette {
    table: Color32,
    minor: Color32,
    major: Color32,
    ink: Color32,
    fill: Color32,
    selected: Color32,
    handle: Color32,
    label: Color32,
    /// The seam allowance band.
    band: Color32,
    /// The cut line.
    cut: Color32,
    /// The outline and fill of the pale half of a cut-on-fold piece.
    pale: Color32,
    pale_fill: Color32,
}

impl Palette {
    fn new(dark: bool) -> Self {
        if dark {
            Self {
                table: Color32::from_gray(30),
                minor: Color32::from_gray(42),
                major: Color32::from_gray(64),
                ink: Color32::from_gray(220),
                fill: Color32::from_rgba_unmultiplied(120, 160, 230, 46),
                selected: Color32::from_rgb(255, 150, 90),
                handle: Color32::from_rgb(120, 170, 255),
                label: Color32::from_gray(190),
                band: Color32::from_rgba_unmultiplied(120, 160, 230, 20),
                cut: Color32::from_gray(120),
                pale: Color32::from_gray(110),
                pale_fill: Color32::from_rgba_unmultiplied(120, 160, 230, 22),
            }
        } else {
            Self {
                table: Color32::from_gray(250),
                minor: Color32::from_gray(232),
                major: Color32::from_gray(205),
                ink: Color32::from_rgb(40, 40, 60),
                fill: Color32::from_rgba_unmultiplied(70, 110, 200, 40),
                selected: Color32::from_rgb(220, 90, 30),
                handle: Color32::from_rgb(50, 110, 220),
                label: Color32::from_gray(70),
                band: Color32::from_rgba_unmultiplied(70, 110, 200, 18),
                cut: Color32::from_gray(150),
                pale: Color32::from_gray(165),
                pale_fill: Color32::from_rgba_unmultiplied(70, 110, 200, 20),
            }
        }
    }
}

impl PatternEditor {
    pub(super) fn paint(&self, painter: &Painter, rect: Rect, dark: bool, drawn: &[Rc<Drawn>]) {
        let c = Palette::new(dark);
        painter.rect_filled(rect, 0.0, c.table);
        self.paint_grid(painter, rect, &c);
        for d in drawn {
            self.paint_shape(painter, rect, d, &c);
        }
        if let Some(d) = drawn
            .iter()
            .find(|d| Some(d.shape.id) == self.selection.piece())
        {
            self.paint_selection(painter, rect, d, &c);
        }
        self.paint_seams(painter, rect);
        self.paint_pins(painter, rect, &c);
        self.paint_drafts(painter, rect, &c);
    }

    /// Every seam and mirror image: a line in its colour just inside each side, with the seam's
    /// number halfway along. The selected seam's lines are thicker, and thin guides join its
    /// two starts and its two ends: they cross when the seam is sewn twisted.
    fn paint_seams(&self, painter: &Painter, rect: Rect) {
        for line in self.seam_lines() {
            let selected = self.selection == Selection::Seam(line.id);
            let width = if selected { 4.5 } else { 2.5 };
            for side in &line.sides {
                painter.add(Shape::line(
                    self.screen_points(rect, side.clone()),
                    Stroke::new(width, line.colour),
                ));
                if let Some(middle) = side.get(side.len() / 2) {
                    let at = self.view.to_screen(rect, *middle);
                    painter.circle_filled(at, 7.5, line.colour);
                    painter.text(
                        at,
                        Align2::CENTER_CENTER,
                        line.id.0.to_string(),
                        FontId::proportional(10.0),
                        Color32::WHITE,
                    );
                }
            }
            if selected {
                let [a, b] = &line.sides;
                for (p, q) in [(a.first(), b.first()), (a.last(), b.last())] {
                    if let (Some(p), Some(q)) = (p, q) {
                        painter.line_segment(
                            [self.view.to_screen(rect, *p), self.view.to_screen(rect, *q)],
                            Stroke::new(1.0, line.colour),
                        );
                    }
                }
            }
        }
    }

    /// Every pin, where its shape shows it: a ring with a dot, the selected one in the
    /// selection's colour.
    fn paint_pins(&self, painter: &Painter, rect: Rect, c: &Palette) {
        for (k, at) in self.pin_spots() {
            let colour = if self.selection == Selection::Pin(k) {
                c.selected
            } else {
                PIN_COLOUR
            };
            let p = self.view.to_screen(rect, at);
            painter.circle_filled(p, 2.5, colour);
            painter.circle_stroke(p, 6.0, Stroke::new(2.0, colour));
        }
    }

    fn mesh(&self, rect: Rect, points: &[Point2], triangles: &[u32], fill: Color32) -> Shape {
        let mut mesh = egui::Mesh::default();
        for p in points {
            mesh.colored_vertex(self.view.to_screen(rect, *p), fill);
        }
        for &[a, b, t] in triangles.as_chunks::<3>().0 {
            mesh.add_triangle(a, b, t);
        }
        Shape::mesh(mesh)
    }

    fn screen_points(&self, rect: Rect, points: Vec<Point2>) -> Vec<Pos2> {
        points
            .into_iter()
            .map(|p| self.view.to_screen(rect, p))
            .collect()
    }

    /// One line per unit (cm or inch) and a darker one every ten; the fine lines hide when
    /// they would be closer than 6 points.
    fn paint_grid(&self, painter: &Painter, rect: Rect, c: &Palette) {
        let v = self.view;
        let unit = self.doc.project().units.mm_per_unit();
        let (lo, hi) = (
            v.to_world(rect, rect.left_bottom()),
            v.to_world(rect, rect.right_top()),
        );
        for (step, color) in [(unit, c.minor), (unit * 10.0, c.major)] {
            if step * v.zoom < 6.0 {
                continue;
            }
            let mut x = (lo.x / step).floor() * step;
            while x <= hi.x {
                painter.vline(
                    v.to_screen(rect, Point2::new(x, 0.0)).x,
                    rect.y_range(),
                    Stroke::new(1.0, color),
                );
                x += step;
            }
            let mut y = (lo.y / step).floor() * step;
            while y <= hi.y {
                painter.hline(
                    rect.x_range(),
                    v.to_screen(rect, Point2::new(0.0, y)).y,
                    Stroke::new(1.0, color),
                );
                y += step;
            }
        }
    }

    fn paint_shape(&self, painter: &Painter, rect: Rect, d: &Drawn, c: &Palette) {
        let v = self.view;
        let piece = &d.shape.piece;
        let selected = self.selection.piece() == Some(d.shape.id);
        let ink = if selected { c.selected } else { c.ink };
        let width = if selected { 2.0 } else { 1.5 };
        if self.show_allowance
            && let Some(cut) = d.cut()
            && !cut.points.is_empty()
        {
            painter.add(self.mesh(rect, &cut.points, &cut.fill, c.band));
            painter.add(Shape::closed_line(
                self.screen_points(rect, cut.points.clone()),
                Stroke::new(1.0, c.cut),
            ));
        }
        let fold = match d.shape.kind {
            geom::ShapeKind::Folded { fold, .. } => Some(fold),
            _ => None,
        };
        match fold {
            Some(fold) => {
                // The whole piece is pale; the half that is really stored is drawn over it
                // in the normal fill and outline.
                painter.add(self.mesh(rect, &d.outline, &d.fill, c.pale_fill));
                painter.add(self.mesh(rect, &d.half, &d.half_fill, c.fill));
                painter.add(Shape::closed_line(
                    self.screen_points(rect, d.outline.clone()),
                    Stroke::new(1.0, c.pale),
                ));
                painter.add(Shape::line(
                    self.screen_points(rect, d.half.clone()),
                    Stroke::new(width, ink),
                ));
                self.paint_fold(painter, rect, fold, c);
            }
            None => {
                painter.add(self.mesh(rect, &d.outline, &d.fill, c.fill));
                painter.add(Shape::closed_line(
                    self.screen_points(rect, d.outline.clone()),
                    Stroke::new(width, ink),
                ));
            }
        }
        for (k, vertex) in piece.vertices.iter().enumerate() {
            if d.shape.stored_vertex(k).is_some() {
                painter.circle_filled(v.to_screen(rect, vertex.pos), 3.0, ink);
            }
        }
        for [a, b] in d.notches(self.show_allowance) {
            painter.line_segment(
                [v.to_screen(rect, *a), v.to_screen(rect, *b)],
                Stroke::new(1.5, ink),
            );
        }
        for (points, kind) in &d.lines {
            let pts = self.screen_points(rect, points.clone());
            match kind {
                LineKind::Marking => {
                    painter.extend(clipped_dashes(
                        &pts,
                        rect.expand(CLIP_MARGIN),
                        Stroke::new(1.0, c.label),
                        (5.0, 3.0),
                    ));
                }
                LineKind::Cutout => {
                    painter.add(Shape::line(pts, Stroke::new(1.5, ink)));
                }
            }
        }
        // Grainline: a double-headed arrow through the middle, with the name beside it.
        let centre = v.to_screen(rect, geom::centroid(piece));
        let r = piece.grain_deg.to_radians();
        let along = vec2(r.cos() as f32, -(r.sin() as f32)) * 40.0;
        let grain = Stroke::new(1.0, c.label);
        painter.arrow(centre, along, grain);
        painter.arrow(centre, -along, grain);
        let name = painter.text(
            centre + vec2(6.0, -4.0),
            Align2::LEFT_BOTTOM,
            &piece.name,
            FontId::proportional(13.0),
            c.label,
        );
        if self.is_paired(&d.shape) {
            // Link badge: two overlapping rings after the name.
            let at = name.right_center() + vec2(9.0, 0.0);
            painter.circle_stroke(at - vec2(3.0, 0.0), 4.0, grain);
            painter.circle_stroke(at + vec2(3.0, 0.0), 4.0, grain);
        }
        if self.show_lengths {
            self.paint_lengths(painter, rect, d, c);
        }
    }

    /// Whether a shape is one of a pair (a twin, or a piece that has one).
    fn is_paired(&self, shape: &geom::Shape) -> bool {
        matches!(shape.kind, geom::ShapeKind::Twin { .. })
            || self
                .doc
                .project()
                .piece(shape.source)
                .is_some_and(|p| p.twin.is_some())
    }

    /// The fold line: dashed, with a two-headed arrow across its middle and "Place on fold".
    fn paint_fold(
        &self,
        painter: &Painter,
        rect: Rect,
        (near, far): (Point2, Point2),
        c: &Palette,
    ) {
        let (a, b) = (
            self.view.to_screen(rect, near),
            self.view.to_screen(rect, far),
        );
        painter.extend(clipped_dashes(
            &[a, b],
            rect.expand(CLIP_MARGIN),
            Stroke::new(1.5, c.ink),
            (8.0, 4.0),
        ));
        let mid = a + (b - a) * 0.5;
        let across = (b - a).normalized().rot90() * 18.0;
        let stroke = Stroke::new(1.0, c.label);
        painter.arrow(mid, across, stroke);
        painter.arrow(mid, -across, stroke);
        painter.text(
            mid + across + vec2(4.0, 0.0),
            Align2::LEFT_CENTER,
            tr!("fold-label"),
            FontId::proportional(11.0),
            c.label,
        );
    }

    /// Each editable edge's length, just outside the piece (and outside the allowance band when
    /// it is shown), so no line runs through the text. Where each label goes was worked out
    /// with the shape; this only places and writes it.
    fn paint_lengths(&self, painter: &Painter, rect: Rect, d: &Drawn, c: &Palette) {
        let piece = &d.shape.piece;
        let units = self.doc.project().units;
        for label in d.labels() {
            let band = if self.show_allowance {
                piece.edge_allowance(label.edge) * self.view.zoom
            } else {
                0.0
            };
            let gap = (band + 10.0) as f32;
            let out = vec2(label.out.x as f32, -(label.out.y as f32));
            painter.text(
                self.view.to_screen(rect, label.at) + out * gap,
                Align2::CENTER_CENTER,
                units.format(label.length),
                FontId::proportional(11.0),
                c.label,
            );
        }
    }

    fn paint_selection(&self, painter: &Painter, rect: Rect, d: &Drawn, c: &Palette) {
        let v = self.view;
        let piece = &d.shape.piece;
        let handle = Stroke::new(1.0, c.handle);
        for (k, edge) in piece.edges.iter().enumerate() {
            if let (Some(_), Edge::Curve { c1, c2 }) = (d.shape.stored_edge(k), *edge) {
                let (a, b) = piece.edge_ends(k);
                for (end, h) in [(a, c1), (b, c2)] {
                    let (end, h) = (v.to_screen(rect, end), v.to_screen(rect, h));
                    painter.line_segment([end, h], handle);
                    painter.circle_stroke(h, 4.0, handle);
                }
            }
        }
        match self.selection {
            Selection::Edge(_, i) => {
                // A fold's own edge is not on the outline: `shape_edge` would name its mirror
                // image's neighbour, so only an edge that maps back to `i` is drawn.
                let k = d.shape.shape_edge(i);
                if d.shape.stored_edge(k) == Some(i) {
                    let pts = self.screen_points(rect, geom::edge_points(piece, k, v.mm(0.25)));
                    painter.add(Shape::line(pts, Stroke::new(3.5, c.selected)));
                }
            }
            Selection::Vertex(_, i) => {
                let p = v.to_screen(rect, piece.vertices[d.shape.shape_vertex(i)].pos);
                painter.circle_filled(p, 5.5, c.selected);
                painter.circle_stroke(p, 5.5, Stroke::new(1.5, c.table));
            }
            Selection::Notch(_, k) => {
                if let Some(notch) = piece.notches.get(k) {
                    let marks = if self.show_allowance {
                        geom::notch_marks(piece, notch)
                    } else {
                        geom::notch_marks_on_stitching(piece, notch)
                    };
                    for [a, b] in marks {
                        painter.line_segment(
                            [v.to_screen(rect, a), v.to_screen(rect, b)],
                            Stroke::new(3.0, c.selected),
                        );
                    }
                }
            }
            Selection::Line(_, l) => {
                // The cached points of the line: thinned for very long ones, as when drawn.
                if let Some((points, _)) = d.lines.get(l) {
                    let pts = self.screen_points(rect, points.clone());
                    painter.add(Shape::line(pts, Stroke::new(3.0, c.selected)));
                }
                // Its curve handles, as a selected piece's: a ring on each, and an arm to the
                // point it belongs to.
                if let Some(line) = piece.lines.get(l) {
                    for (e, edge) in line.edges.iter().enumerate() {
                        if let Edge::Curve { c1, c2 } = *edge {
                            let (a, b) = line.edge_ends(e);
                            for (end, h) in [(a, c1), (b, c2)] {
                                let (end, h) = (v.to_screen(rect, end), v.to_screen(rect, h));
                                painter.line_segment([end, h], handle);
                                painter.circle_stroke(h, 4.0, handle);
                            }
                        }
                    }
                }
            }
            Selection::Piece(_) | Selection::Seam(_) | Selection::Pin(_) | Selection::None => {}
        }
    }

    fn paint_drafts(&self, painter: &Painter, rect: Rect, c: &Palette) {
        let v = self.view;
        let ink = Stroke::new(1.5, c.selected);
        self.paint_draft(painter, rect, &self.canvas.pen, c);
        self.paint_draft(painter, rect, &self.canvas.line, c);
        if let (Some(start), Some(end)) = (self.canvas.rect_start, self.canvas.cursor) {
            let r = Rect::from_two_pos(v.to_screen(rect, start), v.to_screen(rect, end));
            painter.rect_stroke(r, 0.0, ink, StrokeKind::Middle);
        }
        if matches!(self.tool, Tool::AddPoint | Tool::Notch)
            && let Some(p) = self.canvas.preview
        {
            painter.circle_stroke(v.to_screen(rect, p), 4.5, ink);
        }
        if self.tool == Tool::FreeSew {
            self.paint_free_sew(painter, rect, c);
        }
        // The seam side being sewn, thick, with a ring where it starts.
        if let Some(side) = self.sew_draft_side()
            && let Some(shape) = geom::shape_of(self.doc.project(), side.shape)
            && let Some(points) = geom::side_points(&shape, &side, v.mm(0.25))
            && let Some(start) = points.first().copied()
        {
            painter.add(Shape::line(
                self.screen_points(rect, points),
                Stroke::new(4.0, c.selected),
            ));
            painter.circle_stroke(v.to_screen(rect, start), 6.0, ink);
        }
    }

    /// The Free Sew tool's seam so far: the piece its first side is on, outlined; the first side
    /// once made, thick; the side being picked, from its start to where the next click would
    /// end it; a ring at each start; and a dot where a click would land.
    fn paint_free_sew(&self, painter: &Painter, rect: Rect, c: &Palette) {
        let v = self.view;
        let ink = Stroke::new(1.5, c.selected);
        let project = self.doc.project();
        let side_line = |side: &opendrape_core::SeamSide, width: f32| {
            let shape = geom::shape_of(project, side.shape)?;
            let points = geom::side_points(&shape, side, v.mm(0.25))?;
            painter.add(Shape::line(
                self.screen_points(rect, points),
                Stroke::new(width, c.selected),
            ));
            Some(())
        };
        if let Some(d) = self.canvas.free {
            if let Some(shape) = geom::shape_of(project, d.start.shape) {
                let outline = geom::outline_points(&shape.piece, v.mm(0.25));
                painter.add(Shape::closed_line(
                    self.screen_points(rect, outline),
                    Stroke::new(2.5, c.selected),
                ));
            }
            for at in std::iter::once(d.start.at).chain(d.b_start.map(|b| b.at)) {
                painter.circle_stroke(v.to_screen(rect, at), 6.0, ink);
            }
        }
        let (made, so_far) = self.free_sew_drawing(self.canvas.shift);
        if let Some(a) = made {
            side_line(&a, 4.0);
        }
        if let Some(s) = so_far {
            side_line(&s, 2.0);
        }
        if let Some(w) = self.canvas.cursor
            && let Some(hit) = super::free_sew::point_under(project, w, v.mm(super::HIT_PX))
        {
            painter.circle_filled(v.to_screen(rect, hit.at), 3.5, c.selected);
        }
    }

    /// The points placed so far with the pen or the line tool: the edges between them, each
    /// point (with its curve handle, if pulled out), a ring on the first and a dashed line to
    /// the pointer with its length.
    fn paint_draft(&self, painter: &Painter, rect: Rect, points: &[PenPoint], c: &Palette) {
        let v = self.view;
        let ink = Stroke::new(1.5, c.selected);
        let (Some(first), Some(last)) = (points.first(), points.last()) else {
            return;
        };
        if points.len() >= 2 {
            let draft = pen_piece(points);
            for i in 0..points.len() - 1 {
                painter.add(Shape::line(
                    self.screen_points(rect, geom::edge_points(&draft, i, v.mm(0.25))),
                    ink,
                ));
            }
        }
        for p in points {
            let at = v.to_screen(rect, p.pos);
            painter.circle_filled(at, 3.5, c.selected);
            if let Some(h) = p.handle {
                let mirrored = p.pos - (h - p.pos);
                let handle = Stroke::new(1.0, c.handle);
                painter.line_segment([v.to_screen(rect, mirrored), v.to_screen(rect, h)], handle);
                painter.circle_stroke(v.to_screen(rect, h), 4.0, handle);
            }
        }
        // Ring around the first point: click it to finish.
        painter.circle_stroke(
            v.to_screen(rect, first.pos),
            7.0,
            Stroke::new(1.0, c.selected),
        );
        if let Some(cursor) = self.canvas.cursor
            && self.canvas.length_box.is_none()
        {
            let (a, b) = (v.to_screen(rect, last.pos), v.to_screen(rect, cursor));
            painter.extend(clipped_dashes(
                &[a, b],
                rect.expand(CLIP_MARGIN),
                Stroke::new(1.0, c.selected),
                (4.0, 3.0),
            ));
            let text = self.doc.project().units.format(last.pos.distance(cursor));
            painter.text(
                b + vec2(12.0, -12.0),
                Align2::LEFT_BOTTOM,
                text,
                FontId::proportional(12.0),
                c.label,
            );
        }
    }
}

/// The dashes of the polyline `path` (screen points) that lie inside `within`, `(dash, gap)`
/// points long. epaint works out dashes in f32 by walking the whole line, so a line millions of
/// points long (a very long line, zoomed right in) would take forever, or never finish once the
/// distance walked is too large for a gap to be added to it. Only the parts inside `within` are
/// walked, and each keeps the place it would have in the dash pattern of the whole line, so the
/// dashes don't crawl along as the view is panned.
fn clipped_dashes(
    path: &[Pos2],
    within: Rect,
    stroke: Stroke,
    (dash, gap): (f32, f32),
) -> Vec<Shape> {
    let mut shapes = Vec::new();
    let mut run: Vec<Pos2> = Vec::new();
    let mut run_starts_at = 0.0_f64;
    let mut walked = 0.0_f64;
    let flush = |run: &mut Vec<Pos2>, starts_at: f64, shapes: &mut Vec<Shape>| {
        if run.len() >= 2 {
            let offset = dash_offset(starts_at, dash, gap);
            shapes.extend(Shape::dashed_line_with_offset(
                run,
                stroke,
                &[dash],
                &[gap],
                offset,
            ));
        }
        run.clear();
    };
    for w in path.windows(2) {
        let (a, b) = (w[0], w[1]);
        let length = f64::from(a.x - b.x).hypot(f64::from(a.y - b.y));
        match clip_params(a, b, within) {
            Some((t0, t1)) => {
                let at = |t: f64| {
                    if t <= 0.0 {
                        a
                    } else if t >= 1.0 {
                        b
                    } else {
                        Pos2::new(
                            (f64::from(a.x) + f64::from(b.x - a.x) * t) as f32,
                            (f64::from(a.y) + f64::from(b.y - a.y) * t) as f32,
                        )
                    }
                };
                if t0 > 0.0 {
                    // Entering from outside: a new run (the last one ended on leaving).
                    flush(&mut run, run_starts_at, &mut shapes);
                }
                if run.is_empty() {
                    run_starts_at = walked + t0 * length;
                    run.push(at(t0));
                }
                let end = at(t1);
                // Runs shorter than a thousandth of a point add nothing but a zero-length
                // segment, which epaint cannot step along.
                if run.last().is_some_and(|last| last.distance(end) > 1e-3) {
                    run.push(end);
                }
                if t1 < 1.0 {
                    flush(&mut run, run_starts_at, &mut shapes);
                }
            }
            None => flush(&mut run, run_starts_at, &mut shapes),
        }
        walked += length;
    }
    flush(&mut run, run_starts_at, &mut shapes);
    shapes
}

/// Where epaint should start its first dash on a piece of line that begins `walked` points
/// along the whole line, for the pattern to carry on as it would have: a negative offset starts
/// the dash before the piece, when it begins in the middle of one.
fn dash_offset(walked: f64, dash: f32, gap: f32) -> f32 {
    let period = f64::from(dash) + f64::from(gap);
    let phase = walked.rem_euclid(period);
    if phase == 0.0 {
        0.0
    } else if phase < f64::from(dash) {
        -phase as f32
    } else {
        (period - phase) as f32
    }
}

/// The part of the segment `a`–`b` inside `rect`, as the parameters `(t0, t1)` along it (0 at
/// `a`, 1 at `b`), or `None` when it lies wholly outside. Liang–Barsky, in f64 so that
/// coordinates in the millions don't lose their precision.
fn clip_params(a: Pos2, b: Pos2, rect: Rect) -> Option<(f64, f64)> {
    let (a, d) = (
        [f64::from(a.x), f64::from(a.y)],
        [
            f64::from(b.x) - f64::from(a.x),
            f64::from(b.y) - f64::from(a.y),
        ],
    );
    let lo = [f64::from(rect.min.x), f64::from(rect.min.y)];
    let hi = [f64::from(rect.max.x), f64::from(rect.max.y)];
    let (mut t0, mut t1) = (0.0_f64, 1.0_f64);
    for axis in 0..2 {
        if d[axis] == 0.0 {
            if a[axis] < lo[axis] || a[axis] > hi[axis] {
                return None; // parallel to this side, and outside it
            }
            continue;
        }
        // Where the segment crosses the two sides of this axis, entering first.
        let (mut enter, mut leave) = (
            (lo[axis] - a[axis]) / d[axis],
            (hi[axis] - a[axis]) / d[axis],
        );
        if enter > leave {
            std::mem::swap(&mut enter, &mut leave);
        }
        t0 = t0.max(enter);
        t1 = t1.min(leave);
        if t0 > t1 {
            return None;
        }
    }
    Some((t0, t1))
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui::pos2;

    /// 100 × 50 points.
    fn canvas() -> Rect {
        Rect::from_min_max(pos2(0.0, 0.0), pos2(100.0, 50.0))
    }

    /// The segment `a`–`b` clipped to `canvas()`, as points.
    fn clip(a: Pos2, b: Pos2) -> Option<[Pos2; 2]> {
        clip_params(a, b, canvas()).map(|(t0, t1)| {
            let at = |t: f64| {
                pos2(
                    (f64::from(a.x) + f64::from(b.x - a.x) * t) as f32,
                    (f64::from(a.y) + f64::from(b.y - a.y) * t) as f32,
                )
            };
            [at(t0), at(t1)]
        })
    }

    fn near(a: Pos2, b: Pos2) -> bool {
        a.distance(b) < 1e-3
    }

    #[test]
    fn a_segment_inside_the_rectangle_is_kept_whole() {
        assert_eq!(
            clip_params(pos2(10.0, 10.0), pos2(90.0, 40.0), canvas()),
            Some((0.0, 1.0))
        );
        // On its edge counts as inside.
        assert_eq!(
            clip_params(pos2(0.0, 0.0), pos2(100.0, 0.0), canvas()),
            Some((0.0, 1.0))
        );
    }

    #[test]
    fn a_segment_outside_the_rectangle_is_dropped() {
        for (a, b) in [
            (pos2(200.0, 10.0), pos2(300.0, 40.0)),  // beside it
            (pos2(-50.0, -10.0), pos2(-10.0, -5.0)), // beyond a corner
            (pos2(-10.0, 70.0), pos2(30.0, 120.0)),  // below
            (pos2(-10.0, -5.0), pos2(110.0, -5.0)),  // along the top side, outside
            (pos2(120.0, -10.0), pos2(120.0, 90.0)), // along the right side, outside
            (pos2(-60.0, 30.0), pos2(30.0, 90.0)),   // slants past the bottom-left corner
        ] {
            assert_eq!(clip_params(a, b, canvas()), None, "{a:?} {b:?}");
            assert_eq!(clip_params(b, a, canvas()), None, "{b:?} {a:?}");
        }
    }

    #[test]
    fn a_segment_crossing_the_rectangle_is_cut_at_its_sides() {
        let [a, b] = clip(pos2(-50.0, 25.0), pos2(150.0, 25.0)).unwrap();
        assert!(
            near(a, pos2(0.0, 25.0)) && near(b, pos2(100.0, 25.0)),
            "{a:?} {b:?}"
        );
        // One end inside, one out; either way round.
        let [a, b] = clip(pos2(40.0, 10.0), pos2(40.0, 500.0)).unwrap();
        assert!(
            near(a, pos2(40.0, 10.0)) && near(b, pos2(40.0, 50.0)),
            "{a:?} {b:?}"
        );
        let [a, b] = clip(pos2(40.0, 500.0), pos2(40.0, 10.0)).unwrap();
        assert!(
            near(a, pos2(40.0, 50.0)) && near(b, pos2(40.0, 10.0)),
            "{a:?} {b:?}"
        );
        // Across a corner.
        let [a, b] = clip(pos2(-10.0, 40.0), pos2(40.0, -10.0)).unwrap();
        assert!(
            near(a, pos2(0.0, 30.0)) && near(b, pos2(30.0, 0.0)),
            "{a:?} {b:?}"
        );
    }

    #[test]
    fn a_segment_a_billion_points_long_is_cut_to_the_rectangle() {
        let [a, b] = clip(pos2(-1e9, 25.0), pos2(1e9, 25.0)).unwrap();
        assert!(
            near(a, pos2(0.0, 25.0)) && near(b, pos2(100.0, 25.0)),
            "{a:?} {b:?}"
        );
        // Corner to corner of a huge square, through the rectangle's corner (0, 0).
        let [a, b] = clip(pos2(-1e9, -1e9), pos2(1e9, 1e9)).unwrap();
        assert!(
            near(a, pos2(0.0, 0.0)) && near(b, pos2(50.0, 50.0)),
            "{a:?} {b:?}"
        );
        // And one that misses it.
        assert_eq!(clip_params(pos2(-1e9, 1e9), pos2(1e9, 1e9), canvas()), None);
    }

    /// The (start, end) of every dash of `shapes`.
    fn dashes(shapes: &[Shape]) -> Vec<[Pos2; 2]> {
        shapes
            .iter()
            .map(|s| match s {
                Shape::LineSegment { points, .. } => *points,
                other => panic!("{other:?}"),
            })
            .collect()
    }

    #[test]
    fn a_line_a_billion_points_long_makes_only_the_dashes_that_show() {
        let stroke = Stroke::new(1.0, Color32::BLACK);
        let long = [pos2(-1e9, 25.0), pos2(1e9, 25.0)];
        let shapes = clipped_dashes(&long, canvas().expand(CLIP_MARGIN), stroke, (5.0, 3.0));
        // 140 points of line at one dash per 8: about 18 dashes.
        assert!((10..=25).contains(&shapes.len()), "{} dashes", shapes.len());
        for [a, b] in dashes(&shapes) {
            for end in [a, b] {
                assert!(canvas().expand(CLIP_MARGIN + 5.0).contains(end), "{end:?}");
            }
        }
        // Nothing at all for one that never comes near.
        let far = [pos2(-1e9, 1e9), pos2(1e9, 1e9)];
        assert!(clipped_dashes(&far, canvas().expand(CLIP_MARGIN), stroke, (5.0, 3.0)).is_empty());
        assert!(clipped_dashes(&[], canvas(), stroke, (5.0, 3.0)).is_empty());
        assert!(clipped_dashes(&far[..1], canvas(), stroke, (5.0, 3.0)).is_empty());
    }

    #[test]
    fn the_dashes_that_show_are_those_of_the_whole_line() {
        // Clipping must not move the dashes: they stay where epaint puts them on the whole line,
        // whether the line begins inside a dash or inside a gap, and across its vertices.
        let stroke = Stroke::new(1.0, Color32::BLACK);
        let within = canvas().expand(CLIP_MARGIN);
        for start in [-1003.0, -1006.5, -1000.0, -1004.9] {
            let path = [
                pos2(start, 25.0),
                pos2(-400.0, 25.0),
                pos2(-3.5, 25.0),
                pos2(60.0, 25.0),
                pos2(1200.0, 25.0),
            ];
            let whole = dashes(&Shape::dashed_line(&path, stroke, 5.0, 3.0));
            let clipped = dashes(&clipped_dashes(&path, within, stroke, (5.0, 3.0)));
            // Dashes that start well inside the clip rectangle (a cut one is not the same).
            let inner = |all: &[[Pos2; 2]]| -> Vec<f32> {
                all.iter()
                    .filter(|[a, b]| a.x > -10.0 && b.x < 110.0)
                    .map(|[a, _]| a.x)
                    .collect()
            };
            let (want, got) = (inner(&whole), inner(&clipped));
            assert!(want.len() > 10, "{start}: {want:?}");
            assert_eq!(want.len(), got.len(), "{start}: {want:?} vs {got:?}");
            for (w, g) in want.iter().zip(&got) {
                assert!((w - g).abs() < 1e-2, "{start}: {w} vs {g}");
            }
        }
    }

    #[test]
    fn a_line_that_leaves_and_comes_back_is_drawn_in_both_parts() {
        let stroke = Stroke::new(1.0, Color32::BLACK);
        let within = canvas().expand(CLIP_MARGIN);
        // Down the middle, out of the left side, and back in lower down.
        let path = [
            pos2(50.0, 0.0),
            pos2(50.0, 40.0),
            pos2(-900.0, 40.0),
            pos2(-900.0, 45.0),
            pos2(60.0, 45.0),
        ];
        let shapes = clipped_dashes(&path, within, stroke, (5.0, 3.0));
        let d = dashes(&shapes);
        assert!(
            d.iter()
                .any(|[a, _]| (a.x - 50.0).abs() < 1e-3 && a.y < 40.0)
        );
        assert!(
            d.iter()
                .any(|[a, _]| (a.y - 45.0).abs() < 1e-3 && a.x > 0.0)
        );
        // None of it in the long way round, past the margin (and a dash's overshoot).
        let reach = within.expand(6.0);
        assert!(
            d.iter()
                .all(|[a, b]| reach.contains(*a) && reach.contains(*b)),
            "{d:?}"
        );
    }
}
