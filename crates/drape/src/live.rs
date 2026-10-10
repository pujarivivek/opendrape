//! Carrying a drape on after the pattern changes while it drapes: the new fabric starts where
//! the old fabric had got to. Each new point finds the old triangle that held the same spot of
//! the same piece and starts where that spot was; a point the old fabric didn't have (new
//! fabric, as when a sleeve is made longer) carries on from the nearest old triangle, as if
//! that triangle's surface went on flat. Velocities start at zero; seams that were closed stay
//! closed.
//!
//! A spot of a piece is named by where it is on the piece, not on the pattern table: a piece
//! dragged across the table keeps its fabric (see [`moved`]).

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
/// panel with no triangles.
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

    /// The triangle `p` is in (from its cell), or else the nearest triangle of the panel.
    fn find(&self, p: DVec2) -> Option<(usize, [f64; 3])> {
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
        nearest_triangle(self.panel, p)
    }
}

/// How far `now` has been moved on the pattern table (m) from `was`, the same piece as it was
/// before: the one step every point of its outline (corners and curve handles) has taken. A
/// piece dragged across the table is exactly that. Zero when the outline itself changed, moved
/// or not: the shape of the piece is then not the one the old fabric was made for, and a point
/// carries on from the nearest triangle of the old fabric.
pub(crate) fn moved(was: &Shape, now: &Shape) -> DVec2 {
    let (a, b) = (&was.piece, &now.piece);
    let (Some(first), Some(first_now)) = (a.vertices.first(), b.vertices.first()) else {
        return DVec2::ZERO;
    };
    let step = first_now.pos - first.pos;
    let same = |was: Point2, now: Point2| (now - was - step).length() <= SAME_MM;
    let one_step = a.vertices.len() == b.vertices.len()
        && a.edges.len() == b.edges.len()
        && step.is_finite()
        && a.vertices
            .iter()
            .zip(&b.vertices)
            .all(|(v, w)| same(v.pos, w.pos))
        && a.edges.iter().zip(&b.edges).all(|pair| match pair {
            (Edge::Line, Edge::Line) => true,
            (Edge::Curve { c1, c2 }, Edge::Curve { c1: d1, c2: d2 }) => {
                same(*c1, *d1) && same(*c2, *d2)
            }
            _ => false,
        });
    if one_step {
        DVec2::new(step.x / 1000.0, step.y / 1000.0)
    } else {
        DVec2::ZERO
    }
}

/// Where the points of `panel` (a panel of the new fabric) start, carrying on from drape `old`:
/// each point where the same spot of the same piece was in the old cloth, found through the old
/// triangle that held it (welding renumbers triangles in place, so this reaches the live
/// particles); a point outside every old triangle carries on from the nearest one. `moved` (m)
/// is how far the piece has been dragged across the pattern table since (see [`moved`]): a point
/// is looked up where it was before the drag. None when the old drape had no panel for the
/// shape (it starts at its placement).
pub(crate) fn warm_positions(old: &Drape, panel: &PanelMesh, moved: DVec2) -> Option<Vec<DVec3>> {
    let was = old.fabric.panel(panel.shape)?;
    let index = Index::new(was);
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
            drape.solver.step(Some(&collider));
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

    #[test]
    fn a_piece_moved_is_the_step_all_its_points_took_and_a_changed_one_is_not() {
        let square = [(0.0, 0.0), (200.0, 0.0), (200.0, 300.0), (0.0, 300.0)];
        let shape_of = |project: &Project, id: PieceId| {
            geom::shapes(project)
                .into_iter()
                .find(|s| s.id == id)
                .unwrap()
        };
        let mut pr = Project::new();
        let id = pr.add_piece(corner_piece(&square, true));
        let was = shape_of(&pr, id);
        let step = |p: &Project| {
            let d = moved(&was, &shape_of(p, id));
            (d.x * 1000.0, d.y * 1000.0)
        };
        assert_eq!(step(&pr), (0.0, 0.0));
        // Dragged: corners and the curve's handles all step the same.
        let mut dragged = pr.clone();
        dragged
            .piece_mut(id)
            .unwrap()
            .translate(Point2::new(120.0, -35.5));
        let (x, y) = step(&dragged);
        assert!(
            (x - 120.0).abs() < 1e-9 && (y + 35.5).abs() < 1e-9,
            "{x} {y}"
        );
        // One corner moved: the piece changed. So did two (the hem made longer), and so did a
        // handle on its own; and a piece dragged and changed.
        let mut one = pr.clone();
        one.piece_mut(id)
            .unwrap()
            .move_vertex(2, Point2::new(210.0, 320.0));
        assert_eq!(step(&one), (0.0, 0.0));
        let mut hem = pr.clone();
        for v in 0..2 {
            let p = hem.piece(id).unwrap().vertices[v].pos;
            hem.piece_mut(id)
                .unwrap()
                .move_vertex(v, p - Point2::new(0.0, 60.0));
        }
        assert_eq!(step(&hem), (0.0, 0.0));
        let mut handle = pr.clone();
        handle.piece_mut(id).unwrap().edges[1] = Edge::Curve {
            c1: Point2::new(215.0, 40.0),
            c2: Point2::new(190.0, 60.0),
        };
        assert_eq!(step(&handle), (0.0, 0.0));
        let mut both = dragged.clone();
        both.piece_mut(id)
            .unwrap()
            .move_vertex(2, Point2::new(400.0, 320.0));
        assert_eq!(step(&both), (0.0, 0.0));
        let mut straight = dragged.clone();
        straight.piece_mut(id).unwrap().edges[1] = Edge::Line;
        assert_eq!(step(&straight), (0.0, 0.0));
        // A vertex added: not the same outline.
        let mut more = pr.clone();
        more.piece_mut(id)
            .unwrap()
            .vertices
            .push(Vertex::corner(Point2::new(0.0, 0.0)));
        more.piece_mut(id).unwrap().edges.push(Edge::Line);
        assert_eq!(step(&more), (0.0, 0.0));
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
        let d = moved(&find(&pr, front), &find(&next, front));
        assert!(
            (d.x - 0.03).abs() < 1e-12 && (d.y - 0.01).abs() < 1e-12,
            "{d}"
        );
        // The back dragged leaves its twin where it is.
        let d = moved(&find(&pr, twin), &find(&next, twin));
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
        let d = moved(&find(&pr, twin), &find(&dragged, twin));
        assert!(
            (d.x + 0.05).abs() < 1e-12 && (d.y - 0.025).abs() < 1e-12,
            "{d}"
        );
        assert_eq!(moved(&find(&pr, back), &find(&dragged, back)), DVec2::ZERO);
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

    #[test]
    fn a_grid_over_ordinary_fabric_has_cells_about_as_wide_as_its_triangles() {
        // A 60 x 80 cm piece of 2 cm triangles (a 40 x 30 grid of squares).
        let (w, h, step) = (40, 30, 0.02);
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
        let panel = panel_of(flat, triangles);
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
}
