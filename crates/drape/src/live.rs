//! Carrying a drape on after the pattern changes while it drapes: the new fabric starts where
//! the old fabric had got to. Each new point finds the old triangle that held the same spot of
//! the same piece and starts where that spot was; a point the old fabric didn't have (new
//! fabric, as when a sleeve is made longer) carries on from the nearest old triangle, as if
//! that triangle's surface went on flat. Velocities start at zero; seams that were closed stay
//! closed.
//!
//! A spot of a piece is named by where it is on the piece, not on the pattern table: a piece
//! dragged across the table keeps its fabric, even when it is reshaped in the same rebuild (see
//! [`steps`]).

use crate::{Drape, FabricPanel};
use glam::{DVec2, DVec3};
use opendrape_core::{Edge, Point2};
use opendrape_geom::Shape;
use opendrape_mesh::PanelMesh;
use opendrape_sim::{Cloth, Params};

/// Stitches whose two ends start this close (m) are already sewn. When every stitch of the
/// new cloth is, they are all welded at once.
pub const SEWN_GAP_M: f64 = 0.001;
/// The old fabric's triangles are sorted into a grid of square cells to find a point's triangle
/// quickly. The cells are as wide as the triangles are, but the grid never has more than this
/// many cells for each triangle, however far the piece reaches.
const CELLS_PER_TRIANGLE: usize = 4;
/// The cell width (m) for a panel whose triangles have no size to go by.
const CELL_M: f64 = 0.03;
/// Two points of a shape are the same point moved when they differ by the same step within this
/// (mm).
const SAME_MM: f64 = 1e-6;
/// A point this little (in barycentric terms) outside a triangle counts as in it.
const SLACK: f64 = 1e-9;

/// The barycentric coordinates of `p` in the flat triangle `t`.
fn bary(t: [DVec2; 3], p: DVec2) -> [f64; 3] {
    let (v0, v1, v2) = (t[1] - t[0], t[2] - t[0], p - t[0]);
    let (d00, d01, d11) = (v0.dot(v0), v0.dot(v1), v1.dot(v1));
    let (d20, d21) = (v2.dot(v0), v2.dot(v1));
    let denom = d00 * d11 - d01 * d01;
    if denom.abs() < 1e-30 {
        return [1.0, 0.0, 0.0];
    }
    let v = (d11 * d20 - d01 * d21) / denom;
    let w = (d00 * d21 - d01 * d20) / denom;
    [1.0 - v - w, v, w]
}

/// How far `p` is from the flat triangle `t` (0 inside it).
fn distance(t: [DVec2; 3], p: DVec2) -> f64 {
    if bary(t, p).iter().all(|b| *b >= -SLACK) {
        return 0.0;
    }
    (0..3)
        .map(|k| {
            let (a, b) = (t[k], t[(k + 1) % 3]);
            let ab = b - a;
            let s = ((p - a).dot(ab) / ab.length_squared().max(1e-30)).clamp(0.0, 1.0);
            (p - (a + ab * s)).length()
        })
        .fold(f64::INFINITY, f64::min)
}

/// Triangle `j` of `panel`, flat.
fn flat_triangle(panel: &FabricPanel, j: usize) -> [DVec2; 3] {
    panel.triangles[j].map(|k| DVec2::from_array(panel.flat[k as usize]))
}

/// The triangle of `panel` that `p` (m) is in, or else the nearest one: its index in the panel,
/// and `p`'s barycentric coordinates in it (some negative when `p` is outside it). None for a
/// panel with no triangles. It looks at every triangle: for many points of one panel, use an
/// [`Index`].
pub(crate) fn nearest_triangle(panel: &FabricPanel, p: DVec2) -> Option<(usize, [f64; 3])> {
    let j = (0..panel.triangles.len()).min_by(|&a, &b| {
        distance(flat_triangle(panel, a), p).total_cmp(&distance(flat_triangle(panel, b), p))
    })?;
    Some((j, bary(flat_triangle(panel, j), p)))
}

/// A panel's triangles sorted into a grid of cells, each listing the triangles whose bounding
/// box reaches it.
struct Index<'a> {
    panel: &'a FabricPanel,
    min: DVec2,
    cell: f64,
    cols: usize,
    rows: usize,
    cells: Vec<Vec<usize>>,
}

/// The cell width (m), columns and rows of a grid over a box `extent` (m) across, holding
/// `triangles` triangles of mean size `mean` (m): cells as wide as the triangles, widened until
/// there are at most [`CELLS_PER_TRIANGLE`] of them for each triangle. The grid is sized by the
/// triangles, never by how far the piece reaches: a piece a kilometre wide made of a few
/// triangles gets a few cells, not billions.
fn grid(extent: DVec2, mean: f64, triangles: usize) -> (f64, usize, usize) {
    let cap = (CELLS_PER_TRIANGLE * triangles).max(1) as f64;
    let mut cell = if mean > 0.0 && mean.is_finite() {
        mean
    } else {
        CELL_M
    };
    for _ in 0..200 {
        let cols = (extent.x / cell).ceil().max(1.0);
        let rows = (extent.y / cell).ceil().max(1.0);
        if cols * rows <= cap {
            return (cell, cols as usize, rows as usize);
        }
        cell *= 1.5;
    }
    (f64::MAX, 1, 1)
}

impl<'a> Index<'a> {
    fn new(panel: &'a FabricPanel) -> Self {
        let boxes: Vec<(DVec2, DVec2)> = (0..panel.triangles.len())
            .map(|j| {
                let t = flat_triangle(panel, j);
                (t[0].min(t[1]).min(t[2]), t[0].max(t[1]).max(t[2]))
            })
            .collect();
        let min = boxes
            .iter()
            .fold(DVec2::splat(f64::MAX), |m, (lo, _)| m.min(*lo));
        let max = boxes
            .iter()
            .fold(DVec2::splat(f64::MIN), |m, (_, hi)| m.max(*hi));
        let (min, extent) = if boxes.is_empty() {
            (DVec2::ZERO, DVec2::ZERO)
        } else {
            (min, max - min)
        };
        let mean = boxes
            .iter()
            .map(|(lo, hi)| (*hi - *lo).max_element())
            .sum::<f64>()
            / boxes.len().max(1) as f64;
        let (cell, cols, rows) = grid(extent, mean, boxes.len());
        let mut cells = vec![Vec::new(); cols * rows];
        let at = |p: DVec2| {
            let c = ((p - min) / cell).floor();
            (
                (c.x.max(0.0) as usize).min(cols - 1),
                (c.y.max(0.0) as usize).min(rows - 1),
            )
        };
        for (j, (lo, hi)) in boxes.iter().enumerate() {
            let ((c0, r0), (c1, r1)) = (at(*lo), at(*hi));
            for r in r0..=r1 {
                for c in c0..=c1 {
                    cells[r * cols + c].push(j);
                }
            }
        }
        Self {
            panel,
            min,
            cell,
            cols,
            rows,
            cells,
        }
    }

    /// The triangle `p` is in (from its cell), and `p`'s barycentric coordinates in it; None
    /// when it is in none.
    fn contains(&self, p: DVec2) -> Option<(usize, [f64; 3])> {
        let c = ((p - self.min) / self.cell).floor();
        if c.x >= 0.0 && c.y >= 0.0 && (c.x as usize) < self.cols && (c.y as usize) < self.rows {
            let cell = &self.cells[c.y as usize * self.cols + c.x as usize];
            for &j in cell {
                let b = bary(flat_triangle(self.panel, j), p);
                if b.iter().all(|v| *v >= -SLACK) {
                    return Some((j, b));
                }
            }
        }
        None
    }

    /// The triangle `p` is in (from its cell), or else the nearest triangle of the panel.
    fn find(&self, p: DVec2) -> Option<(usize, [f64; 3])> {
        self.contains(p).or_else(|| {
            let j = self.nearest(p)?;
            Some((j, bary(flat_triangle(self.panel, j), p)))
        })
    }

    /// Looks at the triangles of cell (`c`, `r`), keeping the nearest to `p` so far in `best`.
    fn look(&self, c: i64, r: i64, p: DVec2, best: &mut Option<(usize, f64)>) {
        for &j in &self.cells[r as usize * self.cols + c as usize] {
            let d = distance(flat_triangle(self.panel, j), p);
            if best.is_none_or(|(_, b)| d < b) {
                *best = Some((j, d));
            }
        }
    }

    /// The triangle nearest `p`, found in rings of cells round the cell of the grid nearest `p`:
    /// ring 0 is that cell, ring 1 the cells touching it, and so on. It stops once the next ring
    /// is further off than the best triangle so far, so a point just off the fabric looks at a
    /// handful of cells, not at every triangle. None for a panel with no triangles.
    fn nearest(&self, p: DVec2) -> Option<usize> {
        if self.panel.triangles.is_empty() {
            return None;
        }
        let (cols, rows) = (self.cols as i64, self.rows as i64);
        // `as i64` saturates, so a wild point lands on the edge of the grid, not out of range.
        let at = ((p - self.min) / self.cell).floor();
        let (c0, r0) = (
            (at.x as i64).clamp(0, cols - 1),
            (at.y as i64).clamp(0, rows - 1),
        );
        // How far `p` is off the grid along each axis: every cell is at least that far from it.
        let size = DVec2::new(self.cols as f64, self.rows as f64) * self.cell;
        let out = (self.min - p).max(p - (self.min + size)).max(DVec2::ZERO);
        let mut best: Option<(usize, f64)> = None;
        for k in 0..cols.max(rows) {
            let (x0, x1, y0, y1) = (c0 - k, c0 + k, r0 - k, r0 + k);
            for r in y0.max(0)..=y1.min(rows - 1) {
                if r == y0 || r == y1 {
                    for c in x0.max(0)..=x1.min(cols - 1) {
                        self.look(c, r, p, &mut best);
                    }
                } else {
                    if x0 >= 0 {
                        self.look(x0, r, p, &mut best);
                    }
                    if x1 < cols {
                        self.look(x1, r, p, &mut best);
                    }
                }
            }
            // A triangle not seen yet has no point in rings 0..=k, so every point of it is more
            // than k cells away from the grid cell nearest `p` along one axis (and at least as
            // far as `out` along the other).
            let reach = k as f64 * self.cell;
            let next = (out + DVec2::new(reach, 0.0))
                .length()
                .min((out + DVec2::new(0.0, reach)).length());
            if best.is_some_and(|(_, d)| d <= next) {
                break;
            }
        }
        best.map(|(j, _)| j)
    }
}

/// The most steps tried for one piece besides none at all: the biggest groups of corners that
/// took the same step.
const MAX_STEPS: usize = 8;
/// Two steps explain a panel equally well when the points one explains and the other doesn't
/// are no more than this share of its points.
const TIE: f64 = 0.05;

/// The steps (m) that piece `now` may have been dragged across the pattern table by since `was`,
/// the same piece as it was before, nearest zero first. [`best_step`] chooses among them by how
/// much of the new fabric each explains. The evidence is the step each corner and curve handle
/// that kept its index took.
/// - **One step for every point** (a piece dragged and nothing else, or not at all): that step
///   alone. This is exact.
/// - **Otherwise** the piece was reshaped, and may have been dragged as well (edits made in
///   quick succession are made once, as is fast Undo over a drag and a reshape): no step, and
///   each step that corners share and that at least two more corners took than stayed put.
///
/// The fabric can't be left to choose alone. A hem made longer by `h` fits the old fabric about
/// as well as "the piece moved down by `h` and its top shortened by `h`" (a rectangle's scores
/// differ by under 1%), and a triangle's base made lower fits better still shifted. So a step
/// is tried only when it is the likelier edit: it then takes fewer corners to say that the
/// piece was dragged and the rest reshaped than that the corners that moved were reshaped. Two
/// hem corners of four, or a triangle's two base corners, are not a drag. Handles don't count:
/// they move with their corner, so a curved hem has two more points that moved than it has
/// corners.
pub(crate) fn steps(was: &Shape, now: &Shape) -> Vec<DVec2> {
    let (a, b) = (&was.piece, &now.piece);
    let step = |from: Point2, to: Point2| DVec2::new(to.x - from.x, to.y - from.y);
    let mut corners: Vec<DVec2> = a
        .vertices
        .iter()
        .zip(&b.vertices)
        .map(|(v, w)| step(v.pos, w.pos))
        .collect();
    let mut handles = Vec::new();
    let mut same_edges = a.vertices.len() == b.vertices.len() && a.edges.len() == b.edges.len();
    for pair in a.edges.iter().zip(&b.edges) {
        match pair {
            (Edge::Line, Edge::Line) => {}
            (Edge::Curve { c1, c2 }, Edge::Curve { c1: d1, c2: d2 }) => {
                handles.extend([step(*c1, *d1), step(*c2, *d2)]);
            }
            _ => same_edges = false,
        }
    }
    corners.retain(|s| s.is_finite());
    handles.retain(|s| s.is_finite());
    let Some(&first) = corners.first().or(handles.first()) else {
        return vec![DVec2::ZERO];
    };
    let same = |s: DVec2, t: DVec2| (s - t).abs().max_element() <= SAME_MM;
    if same_edges && corners.iter().chain(&handles).all(|s| same(*s, first)) {
        return vec![first / 1000.0];
    }
    let stayed = corners.iter().filter(|s| same(**s, DVec2::ZERO)).count();
    let mut shared = shared_steps(corners);
    shared.retain(|(n, s)| *n >= stayed + 2 && !same(*s, DVec2::ZERO));
    shared.sort_by(|x, y| y.0.cmp(&x.0).then(x.1.length().total_cmp(&y.1.length())));
    shared.truncate(MAX_STEPS);
    let mut steps: Vec<DVec2> = shared.into_iter().map(|(_, s)| s / 1000.0).collect();
    steps.push(DVec2::ZERO);
    steps.sort_by(|x, y| x.length().total_cmp(&y.length()));
    steps
}

/// Each step (mm) that two or more of `steps` share (within [`SAME_MM`] on both axes), with how
/// many do.
fn shared_steps(mut steps: Vec<DVec2>) -> Vec<(usize, DVec2)> {
    steps.sort_by(|a, b| a.x.total_cmp(&b.x));
    let mut shared = Vec::new();
    let mut i = 0;
    while i < steps.len() {
        let n = steps[i..]
            .iter()
            .take_while(|s| s.x - steps[i].x <= SAME_MM)
            .count();
        let column = &mut steps[i..i + n];
        column.sort_by(|a, b| a.y.total_cmp(&b.y));
        let mut j = 0;
        while j < column.len() {
            let m = column[j..]
                .iter()
                .take_while(|s| s.y - column[j].y <= SAME_MM)
                .count();
            if m >= 2 {
                shared.push((m, column[j + m / 2]));
            }
            j += m;
        }
        i += n;
    }
    shared
}

/// The step of `steps` (m) that explains most of `panel`, a panel of the new fabric: the one
/// for which most of its points, looked up where the step puts them, are in a triangle of `old`,
/// the old fabric's grid. A step takes over from an earlier one (they come nearest zero first)
/// only by explaining more than [`TIE`] of the points more: a near tie goes to the step nearest
/// zero. Fabric that fits about as well shifted as not is a tie, however the meshes happen to
/// fall.
fn best_step(old: &Index, panel: &PanelMesh, steps: &[DVec2]) -> DVec2 {
    let Some(&first) = steps.first() else {
        return DVec2::ZERO;
    };
    if steps.len() == 1 {
        return first;
    }
    let explained = |step: DVec2| {
        panel
            .flat
            .iter()
            .filter(|f| old.contains(DVec2::from_array(**f) - step).is_some())
            .count()
    };
    let tie = (panel.flat.len() as f64 * TIE) as usize;
    let mut best = (first, explained(first));
    for &step in &steps[1..] {
        let score = explained(step);
        if score > best.1 + tie {
            best = (step, score);
        }
    }
    best.0
}

/// Where the points of `panel` (a panel of the new fabric) start, carrying on from drape `old`:
/// each point where the same spot of the same piece was in the old cloth, found through the old
/// triangle that held it (welding renumbers triangles in place, so this reaches the live
/// particles); a point outside every old triangle carries on from the nearest one. `steps` are
/// the steps the piece may have been dragged across the pattern table since (see [`steps`]): a
/// point is looked up where it was before the drag. None when the old drape had no panel for the
/// shape (it starts at its placement).
pub(crate) fn warm_positions(
    old: &Drape,
    panel: &PanelMesh,
    steps: &[DVec2],
) -> Option<Vec<DVec3>> {
    let was = old.fabric.panel(panel.shape)?;
    let index = Index::new(was);
    let moved = best_step(&index, panel, steps);
    let cloth = old.solver.cloth();
    let (x, triangles) = (cloth.positions(), cloth.triangles());
    panel
        .flat
        .iter()
        .map(|f| {
            let (j, b) = index.find(DVec2::from_array(*f) - moved)?;
            let t = triangles[was.first_triangle + j];
            Some((0..3).map(|k| x[t[k] as usize] * b[k]).sum())
        })
        .collect()
}

/// How a drape made again while draping runs: gravity at once (it was already hanging), and its
/// seams welded at once when every stitch starts closed; otherwise they close and weld as at
/// Play.
pub(crate) fn warm_params(cloth: &Cloth) -> Params {
    let x = cloth.positions();
    let closed = cloth
        .stitch_pairs()
        .all(|(a, b)| (x[a] - x[b]).length() <= SEWN_GAP_M);
    let play = Params::default();
    Params {
        gravity_delay: 0.0,
        gravity_ramp: 0.0,
        weld_time: if closed { Some(0.0) } else { play.weld_time },
        ..play
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Stage;
    use opendrape_core::{Half, Piece, PieceId, Pin, Placement, Point2, Project, SeamSide, Vertex};
    use opendrape_geom as geom;
    use std::sync::Arc;

    /// A 200 × 300 mm panel A and a panel B beside it, sewn A's right edge to B's left, B
    /// `height` mm tall, both hanging in front of the form from pins at A's and B's top corners.
    fn hanging(height: f64) -> Project {
        let mut pr = Project::new();
        let a = pr.add_piece(Piece::rectangle(
            PieceId(0),
            "A",
            Point2::new(0.0, 0.0),
            200.0,
            300.0,
        ));
        let b = pr.add_piece(Piece::rectangle(
            PieceId(0),
            "B",
            Point2::new(200.0, 300.0 - height),
            200.0,
            height,
        ));
        pr.add_seam(
            SeamSide::edges(a, Half::Drawn, 1, 1, true),
            SeamSide::edges(b, Half::Drawn, 3, 3, false),
        );
        // A plane 50 cm in front of the form, the panels' tops at 1.2 m.
        for id in [a, b] {
            pr.set_placement(id, Some(Placement::at([0.0, 0.0, 0.0])));
        }
        pr.set_placement(a, Some(Placement::at([-0.1, 1.05, 0.5])));
        pr.set_placement(b, Some(Placement::at([0.1, 1.2 - height / 2000.0, 0.5])));
        let pin = |shape, x: f64, at: [f64; 3]| Pin {
            shape,
            half: Half::Drawn,
            at: Point2::new(x, 300.0),
            target: at,
        };
        pr.pins = vec![
            pin(a, 0.0, [-0.2, 1.2, 0.5]),
            pin(b, 400.0, [0.2, 1.2, 0.5]),
        ];
        assert_eq!(pr.check(), Ok(()));
        pr
    }

    /// `drape` stepped on for `frames` frames.
    fn run(drape: &mut Drape, stage: &Stage, frames: usize) {
        let collider = stage.drape_collider();
        for _ in 0..frames {
            drape.solver.step(Some(collider));
        }
    }

    #[test]
    fn made_again_from_the_same_pattern_every_point_starts_where_it_was() {
        let stage = Stage::shared();
        let pr = Arc::new(hanging(300.0));
        let mut old = Drape::new(pr.clone(), &stage);
        run(&mut old, &stage, 60);
        assert!(!old.solver.cloth().has_open_stitches(), "welded");
        let new = old.rebuilt(pr, &stage);
        // Every corner of every triangle (the same triangles: the same pattern makes the same
        // fabric) starts where that corner of the old cloth is now.
        let (ox, ot) = (
            old.solver.cloth().positions(),
            old.solver.cloth().triangles(),
        );
        let (nx, nt) = (
            new.solver.cloth().positions(),
            new.solver.cloth().triangles(),
        );
        assert_eq!(ot.len(), nt.len());
        let mut worst: f64 = 0.0;
        for (a, b) in ot.iter().zip(nt) {
            for k in 0..3 {
                worst = worst.max((ox[a[k] as usize] - nx[b[k] as usize]).length());
            }
        }
        assert!(worst < 0.001, "{:.4} mm", worst * 1000.0);
        // From rest, with gravity on, and the seam (closed) welded at once.
        let c = new.solver.cloth();
        assert!(c.velocities().iter().all(|v| *v == DVec3::ZERO));
        let params = new.solver.params();
        assert_eq!((params.weld_time, params.gravity_delay), (Some(0.0), 0.0));
        assert_eq!(new.fabric, old.fabric, "the same fabric");
    }

    #[test]
    fn new_fabric_carries_on_from_the_fabric_next_to_it() {
        let stage = Stage::shared();
        let mut old = Drape::new(Arc::new(hanging(300.0)), &stage);
        run(&mut old, &stage, 60);
        // B made 60 mm longer at its hem: its bottom corners move down, out of the old fabric.
        let mut longer = hanging(300.0);
        let b = longer.pieces[1].id;
        for v in 0..2 {
            let at = longer.pieces[1].vertices[v].pos;
            longer.pieces[1].move_vertex(v, at - Point2::new(0.0, 60.0));
        }
        assert_eq!(longer.check(), Ok(()));
        let new = old.rebuilt(Arc::new(longer), &stage);
        let panel = new.fabric.panel(b).unwrap();
        let was = old.fabric.panel(b).unwrap();
        let x = new.solver.cloth().positions();
        // The old hem (y = 0 on the pattern) as it hangs now.
        let old_x = old.solver.cloth().positions();
        let hem: Vec<DVec3> = (0..was.flat.len())
            .filter(|&i| was.flat[i][1].abs() < 1e-9)
            .map(|i| {
                let t = was
                    .triangles
                    .iter()
                    .position(|t| t.contains(&(i as u32)))
                    .unwrap();
                let corner = was.triangles[t]
                    .iter()
                    .position(|&c| c == i as u32)
                    .unwrap();
                old_x[old.solver.cloth().triangles()[was.first_triangle + t][corner] as usize]
            })
            .collect();
        let mut checked = 0;
        for (i, f) in panel.flat.iter().enumerate() {
            let p = x[panel.first_particle + i];
            assert!(p.is_finite());
            if f[1] < -0.005 {
                // New fabric, |f.y| below the old hem on the pattern: about as far below it.
                let below = -f[1];
                let nearest = hem
                    .iter()
                    .map(|h| (*h - p).length())
                    .fold(f64::INFINITY, f64::min);
                assert!(
                    (nearest - below).abs() < 0.3 * below + 0.005,
                    "{:.1} mm below the hem on the pattern, {:.1} mm in 3D",
                    below * 1000.0,
                    nearest * 1000.0
                );
                assert!(p.y < hem.iter().map(|h| h.y).fold(f64::MAX, f64::min) + 0.005);
                checked += 1;
            }
        }
        assert!(checked >= 5, "{checked} new points");
    }

    #[test]
    fn a_seam_sewn_while_draping_closes_and_welds_as_at_play() {
        let stage = Stage::shared();
        // B hangs 10 cm to the right of A, and nothing joins them yet.
        let mut apart = hanging(300.0);
        let b = apart.pieces[1].id;
        apart.set_placement(b, Some(Placement::at([0.2, 1.05, 0.5])));
        apart.pins[1].target = [0.3, 1.2, 0.5];
        let sewn = apart.clone();
        apart.seams.clear();
        let mut old = Drape::new(Arc::new(apart), &stage);
        run(&mut old, &stage, 30);
        // The side seam is sewn now: it starts open, so it closes and welds as at Play.
        let new = old.rebuilt(Arc::new(sewn), &stage);
        let c = new.solver.cloth();
        let x = c.positions();
        let widest = c
            .stitch_pairs()
            .map(|(a, b)| (x[a] - x[b]).length())
            .fold(0.0, f64::max);
        assert!(widest > 0.05, "{widest}");
        assert_eq!(new.solver.params().weld_time, Params::default().weld_time);
    }

    #[test]
    fn pins_hold_their_spots_and_a_pin_edit_keeps_the_fabric() {
        let stage = Stage::shared();
        let pr = hanging(300.0);
        let mut drape = Drape::new(Arc::new(pr.clone()), &stage);
        run(&mut drape, &stage, 90);
        // Each pin's spot (the top corners) is at its target.
        let spot = |drape: &Drape, shape: PieceId, at: Point2| {
            let (t, b) = drape.fabric.cloth_point(shape, at).unwrap();
            let c = drape.solver.cloth();
            let tri = c.triangles()[t];
            (0..3)
                .map(|k| c.positions()[tri[k] as usize] * b[k])
                .sum::<DVec3>()
        };
        for pin in &pr.pins {
            let held = spot(&drape, pin.shape, pin.at);
            assert!(
                (held - DVec3::from_array(pin.target)).length() < 1e-3,
                "{held}"
            );
        }
        // Moving a pin's target is not a change to the fabric: the drape carries on.
        let mut moved = pr.clone();
        moved.pins[0].target = [-0.25, 1.25, 0.5];
        moved.set_placement(moved.pieces[0].id, Some(Placement::at([0.0, 0.5, 0.5])));
        assert!(drape.same_fabric(&moved));
        assert!(!drape.same_fabric(&hanging(320.0)));
        let time = drape.solver.time();
        drape.set_pins(Arc::new(moved.clone()));
        run(&mut drape, &stage, 60);
        assert!(drape.solver.time() > time, "the same drape, carrying on");
        let held = spot(&drape, moved.pins[0].shape, moved.pins[0].at);
        assert!(
            (held - DVec3::new(-0.25, 1.25, 0.5)).length() < 1e-3,
            "{held}"
        );
    }

    #[test]
    fn a_point_of_the_cloth_goes_back_to_its_spot_on_the_pattern() {
        let stage = Stage::shared();
        let drape = Drape::new(Arc::new(hanging(300.0)), &stage);
        let b = drape.project.pieces[1].id;
        for at in [Point2::new(250.0, 120.0), Point2::new(399.0, 1.0)] {
            let (t, bary) = drape.fabric.cloth_point(b, at).unwrap();
            let (shape, back) = drape.fabric.pattern_point(t, bary).unwrap();
            assert_eq!(shape, b);
            assert!(back.distance(at) < 1e-6, "{back:?}");
        }
        // Just outside the piece: the nearest point of its fabric.
        let (t, bary) = drape
            .fabric
            .cloth_point(b, Point2::new(400.5, 150.0))
            .unwrap();
        let (_, back) = drape.fabric.pattern_point(t, bary).unwrap();
        assert!(back.distance(Point2::new(400.0, 150.0)) < 1e-6, "{back:?}");
        assert!(drape.fabric.cloth_point(PieceId(99), at_origin()).is_none());
    }

    fn at_origin() -> Point2 {
        Point2::new(0.0, 0.0)
    }

    #[test]
    fn a_pin_on_a_piece_left_out_of_the_fabric_is_let_go() {
        let stage = Stage::shared();
        let mut pr = hanging(300.0);
        // A piece whose outline crosses itself: it can't be made into fabric.
        let bow = pr.add_piece(Piece::polygon(
            PieceId(0),
            "Bow",
            &[
                Point2::new(600.0, 0.0),
                Point2::new(700.0, 100.0),
                Point2::new(700.0, 0.0),
                Point2::new(600.0, 100.0),
            ],
        ));
        pr.pins.push(Pin {
            shape: bow,
            half: Half::Drawn,
            at: Point2::new(610.0, 50.0),
            target: [0.5, 1.0, 0.5],
        });
        let mut drape = Drape::new(Arc::new(pr), &stage);
        assert!(drape.fabric.panel(bow).is_none());
        assert_eq!(
            drape.held_pins(),
            2,
            "the other two are held, the bow's is let go"
        );
        run(&mut drape, &stage, 30);
        assert!(
            drape
                .solver
                .cloth()
                .positions()
                .iter()
                .all(|p| p.is_finite())
        );
    }

    /// Where the spot of `pin` is in `drape`'s cloth now.
    fn pinned_spot(drape: &Drape, pin: &Pin) -> DVec3 {
        let shapes = geom::shapes(&drape.project);
        let shape = shapes.iter().find(|s| s.id == pin.shape).unwrap();
        let spot = shape.spot_shown(pin.half, pin.at);
        let (t, b) = drape.fabric.cloth_point(pin.shape, spot).unwrap();
        let c = drape.solver.cloth();
        let tri = c.triangles()[t];
        (0..3).map(|k| c.positions()[tri[k] as usize] * b[k]).sum()
    }

    /// Where the spot `at` of piece `shape` is in `drape`'s cloth now.
    fn spot(drape: &Drape, shape: PieceId, at: Point2) -> DVec3 {
        let (t, b) = drape.fabric.cloth_point(shape, at).unwrap();
        let c = drape.solver.cloth();
        let tri = c.triangles()[t];
        (0..3).map(|k| c.positions()[tri[k] as usize] * b[k]).sum()
    }

    #[test]
    fn a_piece_dragged_across_the_pattern_table_keeps_its_fabric_where_it_was() {
        let stage = Stage::shared();
        let pr = hanging(300.0);
        let mut old = Drape::new(Arc::new(pr.clone()), &stage);
        run(&mut old, &stage, 90);
        let b = pr.pieces[1].id;
        // Piece B is dragged 150 mm right and 40 up on the table (its pin goes with it), and
        // then on by another 5 mm: the drape is made again after each.
        let mut project = pr.clone();
        let mut total = Point2::new(0.0, 0.0);
        let mut made: Vec<Drape> = Vec::new();
        for d in [Point2::new(150.0, 40.0), Point2::new(5.0, 0.0)] {
            project.piece_mut(b).unwrap().translate(d);
            project.move_pins(b, d);
            total = total + d;
            assert_eq!(project.check(), Ok(()));
            let next = made
                .last()
                .unwrap_or(&old)
                .rebuilt(Arc::new(project.clone()), &stage);
            made.push(next);
        }
        let mut drape = made.pop().unwrap();
        // Every spot of B on the pattern is where it was, a little further right on the table.
        let mut worst: f64 = 0.0;
        for i in 0..=8 {
            for j in 0..=6 {
                let at = Point2::new(200.0 + 25.0 * i as f64, 50.0 * j as f64);
                let before = spot(&old, b, at);
                let after = spot(&drape, b, at + total);
                worst = worst.max((before - after).length());
            }
        }
        assert!(worst < 0.001, "{:.1} mm", worst * 1000.0);
        // And it hangs on from there, with its pin where it was.
        let pin = project.pins[1];
        run(&mut drape, &stage, 60);
        let held = pinned_spot(&drape, &pin);
        assert!(
            (held - DVec3::from_array(pin.target)).length() < 1e-3,
            "{held}"
        );
    }

    #[test]
    fn a_twin_dragged_across_the_pattern_table_keeps_its_fabric_where_it_was() {
        let stage = Stage::shared();
        let (pr, twin) = pale_and_twin(0.0);
        let mut old = Drape::new(Arc::new(pr.clone()), &stage);
        run(&mut old, &stage, 60);
        // The twin alone is dragged: its offset changes, the stored piece stays.
        let mut moved = pr.clone();
        let master = pr.pieces[1].id;
        moved
            .piece_mut(master)
            .unwrap()
            .twin
            .as_mut()
            .unwrap()
            .offset = pr.pieces[1].twin.as_ref().unwrap().offset + Point2::new(-120.0, 35.0);
        assert_eq!(moved.check(), Ok(()));
        let new = old.rebuilt(Arc::new(moved.clone()), &stage);
        // The same stored spot of the twin: shown 120 mm to the left and 35 up on the table.
        let shapes = geom::shapes(&pr);
        let shown = shapes.iter().find(|s| s.id == twin).unwrap();
        let shapes_now = geom::shapes(&moved);
        let shown_now = shapes_now.iter().find(|s| s.id == twin).unwrap();
        let mut worst: f64 = 0.0;
        for stored in [
            Point2::new(610.0, 20.0),
            Point2::new(700.0, 150.0),
            Point2::new(790.0, 290.0),
        ] {
            let before = spot(&old, twin, shown.from_stored(stored));
            let after = spot(&new, twin, shown_now.from_stored(stored));
            worst = worst.max((before - after).length());
        }
        assert!(worst < 0.001, "{:.1} mm", worst * 1000.0);
    }

    /// A Front folded on its left edge (so its drawn half is the right half), and a Back with a
    /// twin to its right, each 200 mm wide as stored and 300 tall; the Front's pale half and the
    /// twin each hold a corner of the fabric at a target in front of the form, as does the
    /// Front's drawn half. `longer` is how much longer (mm) the hems are made. Returns the
    /// project and the twin's id.
    fn pale_and_twin(longer: f64) -> (Project, PieceId) {
        let mut pr = Project::new();
        let mut front = Piece::rectangle(PieceId(0), "Front", Point2::new(0.0, 0.0), 200.0, 300.0);
        front.fold = Some(3);
        let front = pr.add_piece(front);
        let back = pr.add_piece(Piece::rectangle(
            PieceId(0),
            "Back",
            Point2::new(600.0, 0.0),
            200.0,
            300.0,
        ));
        let twin = pr
            .add_twin(back, "Back (mirror)".into(), Point2::new(1600.0, 0.0))
            .unwrap();
        for id in [front, back] {
            for v in 0..2 {
                let p = pr.piece(id).unwrap().vertices[v].pos;
                pr.piece_mut(id)
                    .unwrap()
                    .move_vertex(v, p - Point2::new(0.0, longer));
            }
        }
        // The Front's centre (bounding box) 40 cm in front of the form's centre line, the Back
        // on its left with the twin mirrored on its right, 40 cm out.
        pr.set_placement(front, Some(Placement::at([0.0, 1.05, 0.5])));
        pr.set_placement(back, Some(Placement::at([-0.5, 1.05, 0.5])));
        let pin = |shape, half, at: Point2, target: [f64; 3]| Pin {
            shape,
            half,
            at,
            target,
        };
        pr.pins = vec![
            // The pale half's top-left corner (-200, 300) is the mirror image of stored (200, 300).
            pin(
                front,
                Half::Pale,
                Point2::new(200.0, 300.0),
                [-0.2, 1.2, 0.5],
            ),
            pin(
                front,
                Half::Drawn,
                Point2::new(200.0, 300.0),
                [0.2, 1.2, 0.5],
            ),
            // The twin shows stored (600, 300) at (1000, 300): its top-right corner.
            pin(
                twin,
                Half::Drawn,
                Point2::new(600.0, 300.0),
                [0.7, 1.2, 0.5],
            ),
        ];
        assert_eq!(pr.check(), Ok(()));
        (pr, twin)
    }

    #[test]
    fn pins_hold_the_same_spots_of_the_pattern_when_the_fabric_is_made_again() {
        let stage = Stage::shared();
        let pr = hanging(300.0);
        let mut drape = Drape::new(Arc::new(pr.clone()), &stage);
        run(&mut drape, &stage, 90);
        // B is made 60 mm longer at its hem: a different mesh, more triangles.
        let mut longer = pr.clone();
        for v in 0..2 {
            let at = longer.pieces[1].vertices[v].pos;
            longer.pieces[1].move_vertex(v, at - Point2::new(0.0, 60.0));
        }
        assert_eq!(longer.check(), Ok(()));
        let triangles = drape.solver.cloth().triangles().len();
        let mut new = drape.rebuilt(Arc::new(longer.clone()), &stage);
        assert!(
            new.solver.cloth().triangles().len() > triangles,
            "re-meshed"
        );
        assert_eq!(new.held_pins(), 2);
        run(&mut new, &stage, 60);
        for pin in &longer.pins {
            let held = pinned_spot(&new, pin);
            assert!(
                (held - DVec3::from_array(pin.target)).length() < 1e-3,
                "{held}"
            );
        }
    }

    #[test]
    fn a_pale_half_pin_and_a_twin_pin_hold_their_spots_when_the_fabric_is_made_again() {
        let stage = Stage::shared();
        let (pr, twin) = pale_and_twin(0.0);
        let mut drape = Drape::new(Arc::new(pr.clone()), &stage);
        assert_eq!(drape.held_pins(), 3);
        run(&mut drape, &stage, 60);
        let check = |drape: &Drape, project: &Project| {
            assert_eq!(drape.held_pins(), 3);
            for pin in &project.pins {
                let held = pinned_spot(drape, pin);
                assert!(
                    (held - DVec3::from_array(pin.target)).length() < 1e-3,
                    "{:?} of {:?} at {:?} is at {held}",
                    pin.half,
                    pin.shape,
                    pin.at
                );
            }
        };
        check(&drape, &pr);
        // The pale pin is held on the pale half: the left corner of the whole front.
        let corner = spot(&drape, pr.pieces[0].id, Point2::new(-200.0, 300.0));
        assert!(
            (corner - DVec3::new(-0.2, 1.2, 0.5)).length() < 1e-3,
            "{corner}"
        );
        // The twin's is held on the twin's top-right corner.
        let corner = spot(&drape, twin, Point2::new(1000.0, 300.0));
        assert!(
            (corner - DVec3::new(0.7, 1.2, 0.5)).length() < 1e-3,
            "{corner}"
        );
        // Both hems are made 50 mm longer, and the fabric is made again.
        let (longer, _) = pale_and_twin(50.0);
        let mut new = drape.rebuilt(Arc::new(longer.clone()), &stage);
        assert_ne!(new.fabric, drape.fabric, "re-meshed");
        run(&mut new, &stage, 60);
        check(&new, &longer);
    }

    #[test]
    fn a_pin_taken_off_lets_go_of_its_spot() {
        let stage = Stage::shared();
        let pr = hanging(300.0);
        let mut drape = Drape::new(Arc::new(pr.clone()), &stage);
        run(&mut drape, &stage, 90);
        // The right-hand pin is taken off: B's top corner is no longer held at 1.2 m.
        let mut one = pr.clone();
        let gone = one.pins.pop().unwrap();
        drape.set_pins(Arc::new(one));
        assert_eq!(drape.held_pins(), 1);
        run(&mut drape, &stage, 90);
        let fallen = DVec3::from_array(gone.target).y - spot(&drape, gone.shape, gone.at).y;
        assert!(fallen > 0.05, "it sank only {:.1} mm", fallen * 1000.0);
        // The left one still holds.
        let held = pinned_spot(&drape, &pr.pins[0]);
        assert!((held - DVec3::from_array(pr.pins[0].target)).length() < 1e-3);
    }

    fn corner_piece(points: &[(f64, f64)], curve: bool) -> Piece {
        let corners: Vec<Point2> = points.iter().map(|&(x, y)| Point2::new(x, y)).collect();
        let mut piece = Piece::polygon(PieceId(1), "P", &corners);
        if curve {
            piece.edges[1] = Edge::Curve {
                c1: Point2::new(210.0, 40.0),
                c2: Point2::new(190.0, 60.0),
            };
        }
        piece
    }

    /// The one step a piece took when it was only dragged (or not touched).
    fn dragged_by(was: &Shape, now: &Shape) -> DVec2 {
        let steps = steps(was, now);
        assert_eq!(steps.len(), 1, "{steps:?}");
        steps[0]
    }

    fn shape_of(project: &Project, id: PieceId) -> Shape {
        geom::shapes(project)
            .into_iter()
            .find(|s| s.id == id)
            .unwrap()
    }

    /// The steps `project`'s piece `id` may have taken since `was`, in mm.
    fn steps_mm(was: &Shape, project: &Project, id: PieceId) -> Vec<(f64, f64)> {
        steps(was, &shape_of(project, id))
            .iter()
            .map(|s| (s.x * 1000.0, s.y * 1000.0))
            .collect()
    }

    #[test]
    fn a_piece_only_dragged_took_one_step_and_a_reshaped_one_offers_the_steps_to_choose_from() {
        let square = [(0.0, 0.0), (200.0, 0.0), (200.0, 300.0), (0.0, 300.0)];
        let mut pr = Project::new();
        let id = pr.add_piece(corner_piece(&square, true));
        let was = shape_of(&pr, id);
        let close = |a: &[(f64, f64)], b: &[(f64, f64)]| {
            a.len() == b.len()
                && a.iter()
                    .zip(b)
                    .all(|(p, q)| (p.0 - q.0).abs() < 1e-9 && (p.1 - q.1).abs() < 1e-9)
        };
        let none = [(0.0, 0.0)];
        assert!(close(&steps_mm(&was, &pr, id), &none));
        // Dragged: corners and the curve's handles all step the same, exactly, and that is the
        // only step.
        let mut dragged = pr.clone();
        dragged
            .piece_mut(id)
            .unwrap()
            .translate(Point2::new(120.0, -35.5));
        let drag = [(120.0, -35.5)];
        assert!(close(&steps_mm(&was, &dragged, id), &drag));
        // Reshaped, no two points sharing a step: the piece did not move.
        let mut one = pr.clone();
        one.piece_mut(id)
            .unwrap()
            .move_vertex(2, Point2::new(210.0, 320.0));
        assert!(close(&steps_mm(&was, &one, id), &none));
        let mut handle = pr.clone();
        handle.piece_mut(id).unwrap().edges[1] = Edge::Curve {
            c1: Point2::new(215.0, 40.0),
            c2: Point2::new(190.0, 60.0),
        };
        assert!(close(&steps_mm(&was, &handle, id), &none));
        // Two hem corners down 60 mm (and the handle that goes with one of them): as many
        // corners stayed as moved, so the piece did not move.
        let lower_hem = |project: &mut Project| {
            for v in 0..2 {
                let p = project.piece(id).unwrap().vertices[v].pos;
                project
                    .piece_mut(id)
                    .unwrap()
                    .move_vertex(v, p - Point2::new(0.0, 60.0));
            }
        };
        let mut hem = pr.clone();
        lower_hem(&mut hem);
        assert!(close(&steps_mm(&was, &hem, id), &none));
        // The same with the hem itself curved: its handles go with the corners, and the points
        // that moved (6 of 8 here) outnumber the ones that stayed. Still not a move.
        let mut curved = pr.clone();
        curved.piece_mut(id).unwrap().edges[0] = Edge::Curve {
            c1: Point2::new(50.0, -25.0),
            c2: Point2::new(150.0, -25.0),
        };
        let was_curved = shape_of(&curved, id);
        let mut lowered = curved.clone();
        lower_hem(&mut lowered);
        assert!(close(&steps_mm(&was_curved, &lowered, id), &none));
        // A triangle's base made lower: two corners moved, one stayed. Not a move either.
        let mut triangle = Project::new();
        let t = triangle.add_piece(corner_piece(
            &[(0.0, 0.0), (200.0, 0.0), (100.0, 300.0)],
            false,
        ));
        let was_t = shape_of(&triangle, t);
        let mut lower_base = triangle.clone();
        for v in 0..2 {
            let p = lower_base.piece(t).unwrap().vertices[v].pos;
            lower_base
                .piece_mut(t)
                .unwrap()
                .move_vertex(v, p - Point2::new(0.0, 60.0));
        }
        assert!(close(&steps_mm(&was_t, &lower_base, t), &none));
        // Dragged, and one corner somewhere else: the drag's step is one to choose.
        let mut nudged = dragged.clone();
        nudged
            .piece_mut(id)
            .unwrap()
            .move_vertex(2, Point2::new(400.0, 320.0));
        let nudged = steps_mm(&was, &nudged, id);
        assert!(close(&nudged, &[(0.0, 0.0), (120.0, -35.5)]), "{nudged:?}");
        // Dragged and its hem lowered in one rebuild: none, and both the steps two corners
        // took (nearest zero first).
        let mut dragged_hem = dragged.clone();
        lower_hem(&mut dragged_hem);
        let dragged_hem = steps_mm(&was, &dragged_hem, id);
        assert!(
            close(&dragged_hem, &[(0.0, 0.0), (120.0, -35.5), (120.0, -95.5)]),
            "{dragged_hem:?}"
        );
        let mut straight = dragged.clone();
        straight.piece_mut(id).unwrap().edges[1] = Edge::Line;
        let straight = steps_mm(&was, &straight, id);
        assert!(
            close(&straight, &[(0.0, 0.0), (120.0, -35.5)]),
            "{straight:?}"
        );
        // A triangle with every corner somewhere else: none.
        let mut all_over = triangle.clone();
        for (v, to) in [(10.0, 20.0), (230.0, 5.0), (7.0, 340.0)]
            .iter()
            .enumerate()
        {
            all_over
                .piece_mut(t)
                .unwrap()
                .move_vertex(v, Point2::new(to.0, to.1));
        }
        assert!(close(&steps_mm(&was_t, &all_over, t), &none));
        // A vertex added: not the same outline, and the points that are still there stayed.
        let mut more = pr.clone();
        more.piece_mut(id)
            .unwrap()
            .vertices
            .push(Vertex::corner(Point2::new(0.0, 0.0)));
        more.piece_mut(id).unwrap().edges.push(Edge::Line);
        assert!(close(&steps_mm(&was, &more, id), &none));
        // However many steps a piece with a great many points offers, only the biggest groups
        // are tried (with none).
        let ring: Vec<(f64, f64)> = (0..60)
            .map(|k| {
                let a = f64::from(k) / 60.0 * std::f64::consts::TAU;
                (200.0 * a.cos(), 200.0 * a.sin())
            })
            .collect();
        let mut round = Project::new();
        let r = round.add_piece(corner_piece(&ring, false));
        let was_r = shape_of(&round, r);
        for v in 0..60 {
            let p = round.piece(r).unwrap().vertices[v].pos;
            // Pairs of corners take the same step as each other and no other's.
            let d = Point2::new(f64::from(v as u32 / 2) * 3.0 + 1.0, 0.0);
            round.piece_mut(r).unwrap().move_vertex(v, p + d);
        }
        assert_eq!(steps(&was_r, &shape_of(&round, r)).len(), MAX_STEPS + 1);
    }

    #[test]
    fn a_twin_and_a_folded_piece_are_moved_with_their_pale_halves() {
        let mut pr = Project::new();
        let mut front = Piece::rectangle(PieceId(0), "F", Point2::new(0.0, 0.0), 100.0, 200.0);
        front.fold = Some(3);
        let front = pr.add_piece(front);
        let back = pr.add_piece(Piece::rectangle(
            PieceId(0),
            "B",
            Point2::new(300.0, 0.0),
            100.0,
            200.0,
        ));
        let twin = pr
            .add_twin(back, "B2".into(), Point2::new(900.0, 0.0))
            .unwrap();
        let find = |project: &Project, id: PieceId| {
            geom::shapes(project)
                .into_iter()
                .find(|s| s.id == id)
                .unwrap()
        };
        let mut next = pr.clone();
        next.piece_mut(front)
            .unwrap()
            .translate(Point2::new(30.0, 10.0));
        let d = dragged_by(&find(&pr, front), &find(&next, front));
        assert!(
            (d.x - 0.03).abs() < 1e-12 && (d.y - 0.01).abs() < 1e-12,
            "{d}"
        );
        // The back dragged leaves its twin where it is.
        let d = dragged_by(&find(&pr, twin), &find(&next, twin));
        assert_eq!(d, DVec2::ZERO);
        // The twin dragged: its offset changes.
        let mut dragged = pr.clone();
        dragged
            .piece_mut(back)
            .unwrap()
            .twin
            .as_mut()
            .unwrap()
            .offset = Point2::new(850.0, 25.0);
        let d = dragged_by(&find(&pr, twin), &find(&dragged, twin));
        assert!(
            (d.x + 0.05).abs() < 1e-12 && (d.y - 0.025).abs() < 1e-12,
            "{d}"
        );
        assert_eq!(
            dragged_by(&find(&pr, back), &find(&dragged, back)),
            DVec2::ZERO
        );
    }

    /// A panel of flat points and triangles, as the old fabric has it.
    fn panel_of(flat: Vec<[f64; 2]>, triangles: Vec<[u32; 3]>) -> FabricPanel {
        FabricPanel {
            shape: PieceId(1),
            flat,
            triangles,
            first_particle: 0,
            first_triangle: 0,
        }
    }

    #[test]
    fn the_lookup_grid_is_sized_by_the_triangles_not_by_how_far_the_piece_reaches() {
        // Two triangles 5 km across, and a sliver of 100 triangles along a 10 km diagonal.
        let square = panel_of(
            vec![[0.0, 0.0], [5000.0, 0.0], [5000.0, 5000.0], [0.0, 5000.0]],
            vec![[0, 1, 2], [0, 2, 3]],
        );
        let (cell, cols, rows) = grid(DVec2::new(5000.0, 5000.0), 5000.0, 2);
        assert!(cols * rows <= 8 && cell > 0.0, "{cols} x {rows}");
        let index = Index::new(&square);
        assert!(index.cells.len() <= 8, "{} cells", index.cells.len());
        let (j, b) = index.find(DVec2::new(4000.0, 1000.0)).unwrap();
        assert_eq!(j, 0);
        assert!(b.iter().all(|v| *v >= 0.0));
        let (j, _) = index.find(DVec2::new(1000.0, 4000.0)).unwrap();
        assert_eq!(j, 1);

        let n = 50;
        let mut flat = Vec::new();
        for i in 0..=n {
            let t = 10_000.0 * i as f64 / n as f64;
            flat.push([t, t]);
            flat.push([t + 0.01, t]);
        }
        let triangles: Vec<[u32; 3]> = (0..n as u32)
            .flat_map(|i| {
                let k = 2 * i;
                [[k, k + 1, k + 2], [k + 1, k + 3, k + 2]]
            })
            .collect();
        let sliver = panel_of(flat, triangles);
        let index = Index::new(&sliver);
        assert!(index.cells.len() <= 4 * 100, "{} cells", index.cells.len());
        // A point on the sliver is found in its own triangle, and one far off in the nearest.
        let (j, _) = index.find(DVec2::new(5000.002, 5000.001)).unwrap();
        let t = flat_triangle(&sliver, j);
        assert!(
            bary(t, DVec2::new(5000.002, 5000.001))
                .iter()
                .all(|v| *v >= -1e-9)
        );
        let (j, _) = index.find(DVec2::new(9000.0, 100.0)).unwrap();
        assert!(j < 100);
    }

    /// A panel of `w` × `h` squares `step` (m) wide, two triangles each.
    fn lattice(w: usize, h: usize, step: f64) -> FabricPanel {
        let mut flat = Vec::new();
        for r in 0..=h {
            for c in 0..=w {
                flat.push([c as f64 * step, r as f64 * step]);
            }
        }
        let at = |c: usize, r: usize| (r * (w + 1) + c) as u32;
        let mut triangles = Vec::new();
        for r in 0..h {
            for c in 0..w {
                triangles.push([at(c, r), at(c + 1, r), at(c + 1, r + 1)]);
                triangles.push([at(c, r), at(c + 1, r + 1), at(c, r + 1)]);
            }
        }
        panel_of(flat, triangles)
    }

    #[test]
    fn a_grid_over_ordinary_fabric_has_cells_about_as_wide_as_its_triangles() {
        // A 60 x 80 cm piece of 2 cm triangles (a 40 x 30 grid of squares).
        let panel = lattice(40, 30, 0.02);
        let index = Index::new(&panel);
        assert!(index.cell > 0.01 && index.cell < 0.08, "{} m", index.cell);
        assert!(index.cells.len() <= 4 * panel.triangles.len());
        // Every point of the panel finds a triangle that holds it.
        for p in [
            DVec2::new(0.013, 0.021),
            DVec2::new(0.5, 0.37),
            DVec2::new(0.79, 0.59),
        ] {
            let (j, b) = index.find(p).unwrap();
            assert!(b.iter().all(|v| *v >= -SLACK), "{j} {b:?}");
        }
        // A panel with no triangles has a grid of one cell and finds nothing.
        let empty = panel_of(vec![], vec![]);
        let index = Index::new(&empty);
        assert_eq!(index.cells.len(), 1);
        assert!(index.find(DVec2::new(1.0, 1.0)).is_none());
    }

    /// The distance from `p` to the nearest triangle of `panel`, looking at every one.
    fn nearest_by_looking_at_all(panel: &FabricPanel, p: DVec2) -> f64 {
        (0..panel.triangles.len())
            .map(|j| distance(flat_triangle(panel, j), p))
            .fold(f64::INFINITY, f64::min)
    }

    #[test]
    fn the_nearest_triangle_through_the_grid_is_the_nearest_one() {
        // A lattice with holes in it (every triangle whose number is a multiple of 7 or 13 is
        // missing, and a whole block of squares), and a sliver panel of unequal triangles.
        let mut holed = lattice(30, 20, 0.02);
        let w = 30;
        let mut k = 0;
        holed.triangles.retain(|t| {
            k += 1;
            let (c, r) = ((t[0] as usize) % (w + 1), (t[0] as usize) / (w + 1));
            let block = (10..16).contains(&c) && (6..14).contains(&r);
            !(k % 7 == 0 || k % 13 == 0 || block)
        });
        let sliver = {
            let n = 30;
            let mut flat = Vec::new();
            for i in 0..=n {
                let t = 2.0 * i as f64 / n as f64;
                flat.extend([[t, t * 0.5], [t + 0.01, t * 0.5]]);
            }
            let triangles = (0..n as u32)
                .flat_map(|i| {
                    let k = 2 * i;
                    [[k, k + 1, k + 2], [k + 1, k + 3, k + 2]]
                })
                .collect();
            panel_of(flat, triangles)
        };
        // A fixed scatter of points: inside the holes, off the edges, and far outside.
        let mut seed = 12345_u64;
        let mut next = || {
            seed = seed
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            (seed >> 11) as f64 / (1_u64 << 53) as f64
        };
        for panel in [&holed, &sliver] {
            let index = Index::new(panel);
            let mut off = 0;
            for _ in 0..600 {
                let reach: f64 = [0.05, 1.0, 40.0][(next() * 3.0) as usize % 3];
                let p = DVec2::new(
                    (next() - 0.25) * 2.0 * reach.max(0.7),
                    (next() - 0.25) * 2.0 * reach.max(0.5),
                );
                let (j, b) = index.find(p).expect("a panel with triangles");
                let found = distance(flat_triangle(panel, j), p);
                let best = nearest_by_looking_at_all(panel, p);
                assert!(
                    (found - best).abs() <= 1e-12 * (1.0 + best),
                    "{p:?}: triangle {j} is {found} away, the nearest is {best}"
                );
                assert_eq!(b, bary(flat_triangle(panel, j), p));
                off += usize::from(best > 0.0);
            }
            assert!(off > 100, "{off} of the points were off the fabric");
        }
        // Nothing to find in a panel without triangles, whatever the point.
        assert!(
            Index::new(&panel_of(vec![], vec![]))
                .find(DVec2::new(1e300, -1e300))
                .is_none()
        );
        // A wild point is still the nearest triangle, not a panic or a hang.
        let index = Index::new(&holed);
        for p in [
            DVec2::new(f64::MAX, 0.0),
            DVec2::new(-1e300, 1e300),
            DVec2::new(f64::NAN, 0.0),
        ] {
            assert!(index.find(p).is_some());
        }
    }

    #[test]
    fn a_point_off_a_big_panel_finds_its_nearest_triangle_without_looking_at_them_all() {
        // A 75 × 60 cm panel of 9,000 triangles, and the points of a strip of new fabric below
        // it (a hem made longer), none of them in a triangle of the old fabric. Looking at
        // every triangle for every point takes about a second here; the grid, a few ms.
        let panel = lattice(75, 60, 0.01);
        assert_eq!(panel.triangles.len(), 9_000);
        let points: Vec<DVec2> = (0..40)
            .flat_map(|r| {
                (0..100).map(move |c| DVec2::new(0.0075 * c as f64, -0.0015 * (r + 1) as f64))
            })
            .collect();
        let start = std::time::Instant::now();
        let index = Index::new(&panel);
        let found: Vec<(usize, [f64; 3])> = points
            .iter()
            .map(|p| index.find(*p).expect("a triangle"))
            .collect();
        let took = start.elapsed();
        assert!(
            took < std::time::Duration::from_millis(250),
            "4,000 points off a 9,000-triangle panel took {took:?}"
        );
        // And they found the right ones: the bottom row of the lattice, directly above.
        for (p, (j, b)) in points.iter().zip(&found) {
            let t = flat_triangle(&panel, *j);
            assert!(t.iter().all(|v| v.y <= 0.01 + 1e-12), "{p:?} found {t:?}");
            assert!(
                b.iter().any(|v| *v < 0.0),
                "outside, so a negative coordinate"
            );
        }
    }

    /// `hanging`, with the hem of B (its bottom edge) curved, sagging 25 mm.
    fn hanging_curved(height: f64) -> Project {
        let mut pr = hanging(height);
        let bottom = 300.0 - height;
        pr.pieces[1].edges[0] = Edge::Curve {
            c1: Point2::new(250.0, bottom - 25.0),
            c2: Point2::new(350.0, bottom - 25.0),
        };
        assert_eq!(pr.check(), Ok(()));
        pr
    }

    /// `pr` with the hem corners of B (its first two) `by` mm lower, the handles of a curved
    /// hem going with them.
    fn lowered(pr: &Project, by: f64) -> Project {
        let mut longer = pr.clone();
        for v in 0..2 {
            let at = longer.pieces[1].vertices[v].pos;
            longer.pieces[1].move_vertex(v, at - Point2::new(0.0, by));
        }
        assert_eq!(longer.check(), Ok(()));
        longer
    }

    /// How far (m) the fabric of `shape` is from where `old` had it, over a grid of spots on
    /// the pattern within `old_box` (mm: x0, y0, x1, y1) and moved by `by` (mm) in `new`.
    fn worst_spot_error(
        old: &Drape,
        new: &Drape,
        shape: PieceId,
        old_box: (f64, f64, f64, f64),
        by: Point2,
    ) -> f64 {
        let (x0, y0, x1, y1) = old_box;
        let mut worst: f64 = 0.0;
        for i in 0..=8 {
            for j in 0..=8 {
                let at = Point2::new(
                    x0 + (x1 - x0) * f64::from(i) / 8.0,
                    y0 + (y1 - y0) * f64::from(j) / 8.0,
                );
                worst = worst.max((spot(old, shape, at) - spot(new, shape, at + by)).length());
            }
        }
        worst
    }

    #[test]
    fn a_hem_made_longer_on_a_piece_with_a_curved_hem_is_not_a_move_and_the_fabric_stays_put() {
        let stage = Stage::shared();
        // B is 200 wide and 300 tall at (200, 0) on the pattern table; its hem is straight in
        // one pattern and curved in the other.
        for pr in [hanging(300.0), hanging_curved(300.0)] {
            let mut old = Drape::new(Arc::new(pr.clone()), &stage);
            run(&mut old, &stage, 90);
            let b = pr.pieces[1].id;
            let longer = lowered(&pr, 60.0);
            // Made again while it drapes: every spot of B that the old fabric had is where it
            // was.
            let new = old.rebuilt(Arc::new(longer.clone()), &stage);
            let worst = worst_spot_error(
                &old,
                &new,
                b,
                (200.0, 0.0, 400.0, 300.0),
                Point2::new(0.0, 0.0),
            );
            assert!(worst < 0.005, "{:.1} mm", worst * 1000.0);
            assert_eq!(
                steps(&shape_of(&pr, b), &shape_of(&longer, b)),
                [DVec2::ZERO],
                "two corners of four moved: the piece did not"
            );
        }
    }

    #[test]
    fn a_triangle_with_its_base_made_lower_is_not_a_move_and_the_fabric_stays_put() {
        let stage = Stage::shared();
        let mut pr = Project::new();
        let t = pr.add_piece(Piece::polygon(
            PieceId(0),
            "T",
            &[
                Point2::new(0.0, 0.0),
                Point2::new(200.0, 0.0),
                Point2::new(100.0, 300.0),
            ],
        ));
        pr.set_placement(t, Some(Placement::at([0.0, 1.0, 0.5])));
        let mut old = Drape::new(Arc::new(pr.clone()), &stage);
        run(&mut old, &stage, 60);
        let mut longer = pr.clone();
        for v in 0..2 {
            let at = longer.pieces[0].vertices[v].pos;
            longer.pieces[0].move_vertex(v, at - Point2::new(0.0, 60.0));
        }
        assert_eq!(longer.check(), Ok(()));
        let new = old.rebuilt(Arc::new(longer.clone()), &stage);
        // The old triangle's spots, away from its edges (the edges move with the corners).
        let mut worst: f64 = 0.0;
        for (x, y) in [
            (100.0, 50.0),
            (100.0, 150.0),
            (80.0, 100.0),
            (120.0, 100.0),
            (100.0, 250.0),
        ] {
            let at = Point2::new(x, y);
            worst = worst.max((spot(&old, t, at) - spot(&new, t, at)).length());
        }
        assert!(worst < 0.005, "{:.1} mm", worst * 1000.0);
        assert_eq!(
            steps(&shape_of(&pr, t), &shape_of(&longer, t)),
            [DVec2::ZERO],
            "two corners of three moved: the triangle did not"
        );
    }

    #[test]
    fn a_piece_dragged_and_its_hem_made_longer_in_one_rebuild_keeps_the_rest_of_its_fabric() {
        let stage = Stage::shared();
        let pr = hanging_curved(300.0);
        let mut old = Drape::new(Arc::new(pr.clone()), &stage);
        run(&mut old, &stage, 90);
        let b = pr.pieces[1].id;
        // B dragged 120 mm right and 35 down (its pin goes with it), its hem then made 60 mm
        // longer, all before the fabric is made again. Two corners took each of two steps: the
        // drag's, and the drag's with the hem's. The new fabric fits the old as well one way
        // as the other, so it is a tie, and the step nearest zero is the drag's.
        let mut project = pr.clone();
        let d = Point2::new(120.0, -35.0);
        project.piece_mut(b).unwrap().translate(d);
        project.move_pins(b, d);
        let project = lowered(&project, 60.0);
        let steps = steps(&shape_of(&pr, b), &shape_of(&project, b));
        assert_eq!(steps.len(), 3, "{steps:?}");
        let new = old.rebuilt(Arc::new(project), &stage);
        // The upper part of B, clear of the hem, is where it was: the drag's step.
        let worst = worst_spot_error(&old, &new, b, (200.0, 150.0, 400.0, 300.0), d);
        assert!(worst < 0.005, "{:.1} mm", worst * 1000.0);
    }

    #[test]
    fn a_piece_dragged_and_reshaped_in_one_rebuild_keeps_its_fabric_within_a_few_mm() {
        let stage = Stage::shared();
        let pr = hanging(300.0);
        let mut old = Drape::new(Arc::new(pr.clone()), &stage);
        run(&mut old, &stage, 90);
        let b = pr.pieces[1].id;
        // Piece B is dragged 250 mm right and 40 up (its pin goes with it), and its top right
        // corner is moved out 3 mm and up 2: one rebuild after both (edits made in quick
        // succession are made once, as is fast Undo over a drag and a reshape).
        let mut project = pr.clone();
        let d = Point2::new(250.0, 40.0);
        project.piece_mut(b).unwrap().translate(d);
        project.move_pins(b, d);
        project
            .piece_mut(b)
            .unwrap()
            .move_vertex(2, Point2::new(403.0, 302.0) + d);
        assert_eq!(project.check(), Ok(()));
        let drape = old.rebuilt(Arc::new(project), &stage);
        // Every spot of B on the pattern is where it was, a little further right on the table.
        let mut worst: f64 = 0.0;
        for i in 0..=8 {
            for j in 0..=6 {
                let at = Point2::new(200.0 + 25.0 * i as f64, 50.0 * j as f64);
                worst = worst.max((spot(&old, b, at) - spot(&drape, b, at + d)).length());
            }
        }
        assert!(worst < 0.005, "{:.1} mm", worst * 1000.0);
    }
}
