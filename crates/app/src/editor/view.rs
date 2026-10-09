//! The pattern table's camera: millimetres with y up ↔ screen points with y down.

use egui::{Pos2, Rect, Vec2, vec2};
use opendrape_core::Point2;

/// Zoom limits in screen points per millimetre (a whole room to a few millimetres).
pub const MIN_ZOOM: f64 = 0.05;
pub const MAX_ZOOM: f64 = 50.0;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct View {
    /// Screen offset from the canvas's top-left corner to the pattern origin.
    pub pan: Vec2,
    /// Screen points per millimetre.
    pub zoom: f64,
}

impl Default for View {
    fn default() -> Self {
        Self {
            pan: vec2(40.0, 400.0),
            zoom: 1.0,
        }
    }
}

impl View {
    pub fn to_screen(&self, rect: Rect, p: Point2) -> Pos2 {
        rect.min + self.pan + vec2((p.x * self.zoom) as f32, (-p.y * self.zoom) as f32)
    }
    pub fn to_world(&self, rect: Rect, s: Pos2) -> Point2 {
        let v = s - rect.min - self.pan;
        Point2::new(f64::from(v.x) / self.zoom, -f64::from(v.y) / self.zoom)
    }
    /// Millimetres covered by `px` screen points at this zoom.
    pub fn mm(&self, px: f64) -> f64 {
        px / self.zoom
    }
    /// Zooms by `factor`, keeping the pattern point under `cursor` where it is.
    pub fn zoom_at(&mut self, rect: Rect, cursor: Pos2, factor: f64) {
        let anchor = self.to_world(rect, cursor);
        self.zoom = (self.zoom * factor).clamp(MIN_ZOOM, MAX_ZOOM);
        self.pan += cursor - self.to_screen(rect, anchor);
    }
    /// Shows the box `min`..`max` (mm) centred in `rect`, with a margin.
    pub fn fit(&mut self, rect: Rect, min: Point2, max: Point2) {
        let size = max - min;
        let (w, h) = (size.x.max(10.0), size.y.max(10.0));
        let zoom = (f64::from(rect.width()) * 0.85 / w).min(f64::from(rect.height()) * 0.85 / h);
        self.zoom = zoom.clamp(MIN_ZOOM, MAX_ZOOM);
        let c = min.lerp(max, 0.5);
        self.pan = rect.size() * 0.5 - vec2((c.x * self.zoom) as f32, (-c.y * self.zoom) as f32);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rect() -> Rect {
        Rect::from_min_size(egui::pos2(50.0, 30.0), vec2(800.0, 600.0))
    }

    #[test]
    fn round_trips_with_y_up() {
        let v = View {
            pan: vec2(10.0, 500.0),
            zoom: 2.0,
        };
        let p = Point2::new(123.4, -56.7);
        let back = v.to_world(rect(), v.to_screen(rect(), p));
        assert!(back.distance(p) < 1e-3, "{back:?}");
        let low = v.to_screen(rect(), Point2::new(0.0, 0.0));
        let high = v.to_screen(rect(), Point2::new(0.0, 10.0));
        assert!(high.y < low.y, "larger y is higher on screen");
        assert_eq!(v.mm(8.0), 4.0);
    }

    #[test]
    fn zoom_keeps_the_point_under_the_cursor() {
        let mut v = View::default();
        let cursor = egui::pos2(321.0, 222.0);
        let before = v.to_world(rect(), cursor);
        v.zoom_at(rect(), cursor, 1.5);
        assert!((v.zoom - 1.5).abs() < 1e-12);
        assert!(v.to_world(rect(), cursor).distance(before) < 1e-3);
        v.zoom_at(rect(), cursor, 1e9);
        assert_eq!(v.zoom, MAX_ZOOM);
        v.zoom_at(rect(), cursor, 1e-12);
        assert_eq!(v.zoom, MIN_ZOOM);
    }

    #[test]
    fn fit_centres_the_box_inside_the_canvas() {
        let mut v = View::default();
        let (min, max) = (Point2::new(1000.0, 2000.0), Point2::new(1600.0, 2800.0));
        v.fit(rect(), min, max);
        let centre = v.to_screen(rect(), min.lerp(max, 0.5));
        assert!((centre - rect().center()).length() < 1e-3);
        assert!(
            rect().contains(v.to_screen(rect(), min)) && rect().contains(v.to_screen(rect(), max))
        );
    }
}
