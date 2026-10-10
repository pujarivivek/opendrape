//! Seams on the pattern table: each drawn as a coloured line just inside both of its sides,
//! with a number badge, and picked by clicking that line. A mirror image is drawn like its
//! seam, in its seam's colour, and picks its seam.

use super::{HIT_PX, PatternEditor};
use egui::Color32;
use opendrape_core::{Point2, Project, SeamId, SeamSide};
use opendrape_geom::{self as geom, Shape};

/// How far inside the outline (screen points) a seam's line runs, so the outline itself can
/// still be clicked.
pub(super) const INSET_PX: f64 = 5.0;

/// Each seam's colour, in turn.
pub(super) const SEAM_COLOURS: [Color32; 8] = [
    Color32::from_rgb(0, 150, 136),
    Color32::from_rgb(156, 39, 176),
    Color32::from_rgb(33, 120, 243),
    Color32::from_rgb(67, 160, 71),
    Color32::from_rgb(216, 27, 96),
    Color32::from_rgb(121, 85, 72),
    Color32::from_rgb(63, 81, 181),
    Color32::from_rgb(190, 145, 0),
];

/// A seam or a mirror image, ready to draw and pick.
pub(super) struct SeamLine {
    /// The stored seam (a mirror image's is its seam's).
    pub id: SeamId,
    pub colour: Color32,
    /// Each side's line (mm), from its start to its end, just inside the outline.
    pub sides: [Vec<Point2>; 2],
}

/// Every seam and mirror image of `project` whose sides fit their shapes, its lines `inset` mm
/// inside the outline and within `tolerance` mm of the curves.
pub(super) fn seam_lines(
    project: &Project,
    shapes: &[Shape],
    inset: f64,
    tolerance: f64,
) -> Vec<SeamLine> {
    let mut stored = 0;
    let mut out = Vec::new();
    for (seam, mirrored) in project.all_seams() {
        if !mirrored {
            stored += 1;
        }
        let line = |side: &SeamSide| {
            let shape = shapes.iter().find(|s| s.id == side.shape)?;
            inset_side(shape, side, inset, tolerance)
        };
        if let (Some(a), Some(b)) = (line(&seam.a), line(&seam.b)) {
            out.push(SeamLine {
                id: seam.id,
                colour: SEAM_COLOURS[(stored - 1) % SEAM_COLOURS.len()],
                sides: [a, b],
            });
        }
    }
    out
}

/// A side's line moved `inset` mm into its shape, from the side's start to its end.
fn inset_side(shape: &Shape, side: &SeamSide, inset: f64, tolerance: f64) -> Option<Vec<Point2>> {
    let ccw = geom::is_counter_clockwise(&shape.piece);
    let mut out = Vec::new();
    for run in geom::side_runs(shape, side)? {
        let (j, against) = (run.edge, run.from > run.to);
        let (lo, hi) = (run.from.min(run.to), run.from.max(run.to));
        let points = geom::edge_points_between(&shape.piece, j, lo, hi, tolerance);
        let last = points.len() - 1;
        let mut moved: Vec<Point2> = (0..points.len())
            .map(|k| {
                // Inwards is to the left of the outline's direction when it runs anticlockwise.
                let d = points[(k + 1).min(last)] - points[k.saturating_sub(1)];
                let left = Point2::new(-d.y, d.x) * (1.0 / d.length().max(1e-12));
                points[k] + left * if ccw { inset } else { -inset }
            })
            .collect();
        if against {
            moved.reverse();
        }
        out.extend(moved);
    }
    Some(out)
}

/// Distance (mm) from `p` to the polyline `points`.
pub(super) fn polyline_distance(p: Point2, points: &[Point2]) -> f64 {
    points
        .windows(2)
        .map(|w| {
            let (a, b) = (w[0], w[1]);
            let ab = b - a;
            let len2 = ab.x * ab.x + ab.y * ab.y;
            let t = if len2 < 1e-18 {
                0.0
            } else {
                (((p.x - a.x) * ab.x + (p.y - a.y) * ab.y) / len2).clamp(0.0, 1.0)
            };
            p.distance(a + ab * t)
        })
        .fold(f64::INFINITY, f64::min)
}

impl PatternEditor {
    /// The seam lines as they are drawn now.
    pub(super) fn seam_lines(&self) -> Vec<SeamLine> {
        seam_lines(
            self.doc.project(),
            &self.shapes(),
            self.view.mm(INSET_PX),
            self.view.mm(0.25),
        )
    }

    /// The stored seam whose line is within `tol` mm of `w` and nearer to it than any outline
    /// edge by a screen point.
    pub(super) fn seam_at(&self, w: Point2, tol: f64) -> Option<SeamId> {
        let (id, d) = self
            .seam_lines()
            .iter()
            .flat_map(|l| l.sides.iter().map(move |s| (l.id, polyline_distance(w, s))))
            .min_by(|a, b| a.1.total_cmp(&b.1))?;
        let edge = self
            .shapes()
            .iter()
            .filter_map(|s| geom::nearest_edge(&s.piece, w).map(|e| e.2))
            .fold(f64::INFINITY, f64::min);
        (d <= tol && d + tol / HIT_PX < edge).then_some(id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use opendrape_core::{Half, OutlinePos, Piece, PieceId};

    #[test]
    fn a_free_side_is_drawn_along_just_the_stretch_it_sews() {
        let mut pr = Project::new();
        let a = pr.add_piece(Piece::rectangle(
            PieceId(0),
            "A",
            Point2::new(0.0, 0.0),
            200.0,
            100.0,
        ));
        let b = pr.add_piece(Piece::rectangle(
            PieceId(0),
            "B",
            Point2::new(300.0, 0.0),
            100.0,
            100.0,
        ));
        // A's bottom edge from 50 mm to 150 mm, run backwards; and round B's bottom-right corner.
        pr.add_seam(
            SeamSide {
                shape: a,
                half: Half::Drawn,
                from: OutlinePos::new(0, 0.75),
                to: OutlinePos::new(0, 0.25),
                forward: false,
            },
            SeamSide {
                shape: b,
                half: Half::Drawn,
                from: OutlinePos::new(0, 0.5),
                to: OutlinePos::new(1, 0.5),
                forward: true,
            },
        );
        let lines = seam_lines(&pr, &geom::shapes(&pr), 2.0, 0.1);
        assert_eq!(lines.len(), 1);
        let [side_a, side_b] = &lines[0].sides;
        // 2 mm inside the outline, from the side's start to its end.
        let close = |p: Point2, q: Point2| assert!(p.distance(q) < 1e-9, "{p:?} vs {q:?}");
        close(side_a[0], Point2::new(150.0, 2.0));
        close(*side_a.last().unwrap(), Point2::new(50.0, 2.0));
        close(side_b[0], Point2::new(350.0, 2.0));
        close(*side_b.last().unwrap(), Point2::new(398.0, 50.0));
        assert!(
            side_b.iter().any(|p| p.x > 397.0 && p.y < 3.0),
            "round the corner"
        );
    }
}
