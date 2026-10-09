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
        self.paint_drafts(painter, rect, &c);
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
        if self.show_allowance && !d.cut.is_empty() {
            painter.add(self.mesh(rect, &d.cut, &d.cut_fill, c.band));
            painter.add(Shape::closed_line(
                self.screen_points(rect, d.cut.clone()),
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
        for [a, b] in &d.notches {
            painter.line_segment(
                [v.to_screen(rect, *a), v.to_screen(rect, *b)],
                Stroke::new(1.5, ink),
            );
        }
        for (points, kind) in &d.lines {
            let pts = self.screen_points(rect, points.clone());
            match kind {
                LineKind::Marking => {
                    painter.extend(Shape::dashed_line(
                        &pts,
                        Stroke::new(1.0, c.label),
                        5.0,
                        3.0,
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
        painter.extend(Shape::dashed_line(
            &[a, b],
            Stroke::new(1.5, c.ink),
            8.0,
            4.0,
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
        for label in &d.labels {
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
                if k < piece.notches.len() {
                    for [a, b] in geom::notch_marks(piece, &piece.notches[k]) {
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
            }
            Selection::Piece(_) | Selection::None => {}
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
            painter.extend(Shape::dashed_line(
                &[a, b],
                Stroke::new(1.0, c.selected),
                4.0,
                3.0,
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
