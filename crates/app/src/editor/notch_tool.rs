//! The Notch tool (N): click on an edge to add a notch there, or point at an edge and type a
//! distance to place it exactly that far from the nearer end.

use super::canvas::{nearest_edge, take_typed_digits};
use super::length_box::{BoxKind, LengthBox};
use super::{HIT_PX, PatternEditor, Selection};
use egui::{Response, vec2};
use opendrape_core::{Notch, PieceId, Point2};
use opendrape_geom as geom;

impl PatternEditor {
    pub(super) fn notch_tool(
        &mut self,
        response: &Response,
        hover: Option<Point2>,
        pointer: Option<Point2>,
        tol: f64,
    ) {
        let project = self.doc.project();
        self.canvas.preview = hover
            .and_then(|w| nearest_edge(project, w, tol))
            .map(|hit| hit.4);
        if response.clicked()
            && let Some(at) = pointer
            && let Some((shape, source, edge, t, _)) = nearest_edge(self.doc.project(), at, tol)
            && let Some(piece) = self.doc.project().piece(source)
        {
            let distance = geom::distance_along(piece, edge, t);
            self.add_notch(shape, source, edge, distance);
        }
    }

    /// Adds a single slit notch `distance` mm along stored edge `edge` of `source`, and selects
    /// it on `shape` (the piece, or its twin).
    pub(super) fn add_notch(
        &mut self,
        shape: PieceId,
        source: PieceId,
        edge: usize,
        distance: f64,
    ) {
        let index = self.doc.edit(|p| {
            p.piece_mut(source).map(|pc| {
                pc.notches.push(Notch::new(edge, distance));
                pc.notches.len() - 1
            })
        });
        if !self.note_if_refused()
            && let Some(k) = index
        {
            self.selection = Selection::Notch(shape, k);
        }
    }

    /// With the pointer near an edge, a typed digit opens the number box for the notch's
    /// distance from the edge's nearer end. Digits typed away from any edge are left alone.
    pub(super) fn notch_box_on_digits(&mut self, ui: &egui::Ui, response: &Response) {
        let tol = self.view.mm(HIT_PX);
        let Some(hover) = response
            .hover_pos()
            .map(|p| self.view.to_world(self.canvas_rect, p))
        else {
            return;
        };
        let Some((shape, source, edge, t, _)) = nearest_edge(self.doc.project(), hover, tol) else {
            return;
        };
        let typed = take_typed_digits(ui);
        if typed.is_empty() {
            return;
        }
        let pos = response.hover_pos().unwrap_or(response.rect.center()) + vec2(16.0, 16.0);
        let kind = BoxKind::NotchDistance {
            shape,
            source,
            edge,
            from_end: t > 0.5,
        };
        self.canvas.length_box = Some(LengthBox::new(kind, typed, String::new(), pos));
    }
}
