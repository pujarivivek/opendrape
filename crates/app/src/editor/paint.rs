//! Drawing the pattern table: grid, pieces, the selection, and drafts in progress.

use super::{PatternEditor, Selection, Tool, canvas::pen_piece};
use egui::{Align2, Color32, FontId, Painter, Pos2, Rect, Shape, Stroke, StrokeKind, vec2};
use opendrape_core::{Edge, Piece, Point2};
use opendrape_geom as geom;

struct Palette {
    table: Color32,
    minor: Color32,
    major: Color32,
    ink: Color32,
    fill: Color32,
    selected: Color32,
    handle: Color32,
    label: Color32,
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
            }
        }
    }
}

impl PatternEditor {
    pub(super) fn paint(&self, painter: &Painter, rect: Rect, dark: bool) {
        let c = Palette::new(dark);
        painter.rect_filled(rect, 0.0, c.table);
        self.paint_grid(painter, rect, &c);
        for piece in &self.doc.project().pieces {
            self.paint_piece(painter, rect, piece, &c);
        }
        self.paint_selection(painter, rect, &c);
        self.paint_drafts(painter, rect, &c);
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

    fn paint_piece(&self, painter: &Painter, rect: Rect, piece: &Piece, c: &Palette) {
        let v = self.view;
        let outline = self.screen_points(rect, geom::outline_points(piece, v.mm(0.25)));
        let selected = self.selection.piece() == Some(piece.id);
        let ink = if selected { c.selected } else { c.ink };
        if outline.len() >= 3 {
            painter.add(fill_polygon(&outline, c.fill));
        }
        painter.add(Shape::closed_line(
            outline,
            Stroke::new(if selected { 2.0 } else { 1.5 }, ink),
        ));
        for vertex in &piece.vertices {
            painter.circle_filled(v.to_screen(rect, vertex.pos), 3.0, ink);
        }
        // Grainline: a double-headed arrow through the middle, with the name beside it.
        let centre = v.to_screen(rect, geom::centroid(piece));
        let r = piece.grain_deg.to_radians();
        let along = vec2(r.cos() as f32, -(r.sin() as f32)) * 40.0;
        let grain = Stroke::new(1.0, c.label);
        painter.arrow(centre, along, grain);
        painter.arrow(centre, -along, grain);
        painter.text(
            centre + vec2(6.0, -4.0),
            Align2::LEFT_BOTTOM,
            &piece.name,
            FontId::proportional(13.0),
            c.label,
        );
        if self.show_lengths {
            let units = self.doc.project().units;
            for i in 0..piece.len() {
                let mid = v.to_screen(rect, geom::point_on_edge(piece, i, 0.5));
                let text = units.format(geom::edge_length(piece, i));
                painter.text(
                    mid,
                    Align2::CENTER_CENTER,
                    text,
                    FontId::proportional(11.0),
                    c.label,
                );
            }
        }
    }

    fn paint_selection(&self, painter: &Painter, rect: Rect, c: &Palette) {
        let Some(piece) = self
            .selection
            .piece()
            .and_then(|id| self.doc.project().piece(id))
        else {
            return;
        };
        let v = self.view;
        let handle = Stroke::new(1.0, c.handle);
        for (i, edge) in piece.edges.iter().enumerate() {
            if let Edge::Curve { c1, c2 } = *edge {
                let (a, b) = piece.edge_ends(i);
                for (end, h) in [(a, c1), (b, c2)] {
                    let (end, h) = (v.to_screen(rect, end), v.to_screen(rect, h));
                    painter.line_segment([end, h], handle);
                    painter.circle_stroke(h, 4.0, handle);
                }
            }
        }
        match self.selection {
            Selection::Edge(_, i) => {
                let pts = self.screen_points(rect, geom::edge_points(piece, i, v.mm(0.25)));
                painter.add(Shape::line(pts, Stroke::new(3.5, c.selected)));
            }
            Selection::Vertex(_, i) => {
                let p = v.to_screen(rect, piece.vertices[i].pos);
                painter.circle_filled(p, 5.5, c.selected);
                painter.circle_stroke(p, 5.5, Stroke::new(1.5, c.table));
            }
            Selection::Piece(_) | Selection::None => {}
        }
    }

    fn paint_drafts(&self, painter: &Painter, rect: Rect, c: &Palette) {
        let v = self.view;
        let ink = Stroke::new(1.5, c.selected);
        let pen = &self.canvas.pen;
        if let (Some(first), Some(last)) = (pen.first(), pen.last()) {
            if pen.len() >= 2 {
                let draft = pen_piece(pen);
                for i in 0..pen.len() - 1 {
                    painter.add(Shape::line(
                        self.screen_points(rect, geom::edge_points(&draft, i, v.mm(0.25))),
                        ink,
                    ));
                }
            }
            for p in pen {
                let at = v.to_screen(rect, p.pos);
                painter.circle_filled(at, 3.5, c.selected);
                if let Some(h) = p.handle {
                    let mirrored = p.pos - (h - p.pos);
                    let handle = Stroke::new(1.0, c.handle);
                    painter
                        .line_segment([v.to_screen(rect, mirrored), v.to_screen(rect, h)], handle);
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
        if let (Some(start), Some(end)) = (self.canvas.rect_start, self.canvas.cursor) {
            let r = Rect::from_two_pos(v.to_screen(rect, start), v.to_screen(rect, end));
            painter.rect_stroke(r, 0.0, ink, StrokeKind::Middle);
        }
        if self.tool == Tool::AddPoint
            && let Some(p) = self.canvas.preview
        {
            painter.circle_stroke(v.to_screen(rect, p), 4.5, ink);
        }
    }
}

/// epaint fills only convex shapes, so concave pieces are triangulated (earcut) into a mesh.
fn fill_polygon(points: &[Pos2], fill: Color32) -> Shape {
    let mut triangles: Vec<u32> = Vec::new();
    earcut::Earcut::new().earcut(
        points.iter().map(|p| [f64::from(p.x), f64::from(p.y)]),
        &[] as &[u32],
        &mut triangles,
    );
    let mut mesh = egui::Mesh::default();
    for p in points {
        mesh.colored_vertex(*p, fill);
    }
    for &[a, b, c] in triangles.as_chunks::<3>().0 {
        mesh.add_triangle(a, b, c);
    }
    Shape::mesh(mesh)
}
