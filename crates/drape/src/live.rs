//! Carrying a drape on after the pattern changes while it drapes: the new fabric starts where
//! the old fabric had got to. Each new point finds the old triangle that held the same spot of
//! the same piece and starts where that spot was; a point the old fabric didn't have (new
//! fabric, as when a sleeve is made longer) carries on from the nearest old triangle, as if
//! that triangle's surface went on flat. Velocities start at zero; seams that were closed stay
//! closed.

use crate::{Drape, FabricPanel};
use glam::{DVec2, DVec3};
use opendrape_mesh::PanelMesh;
use opendrape_sim::{Cloth, Params};

/// Stitches whose two ends start this close (m) are already sewn. When every stitch of the
/// new cloth is, they are all welded at once.
pub const SEWN_GAP_M: f64 = 0.001;
/// The old fabric's triangles are sorted into square cells this wide (m) to find a point's
/// triangle quickly.
const CELL_M: f64 = 0.03;
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
    cols: usize,
    cells: Vec<Vec<usize>>,
}

impl<'a> Index<'a> {
    fn new(panel: &'a FabricPanel) -> Self {
        let pts: Vec<DVec2> = panel.flat.iter().map(|f| DVec2::from_array(*f)).collect();
        let min = pts.iter().fold(DVec2::splat(f64::MAX), |m, p| m.min(*p));
        let max = pts.iter().fold(DVec2::splat(f64::MIN), |m, p| m.max(*p));
        let size = ((max - min) / CELL_M).ceil().max(DVec2::ONE);
        let (cols, rows) = (size.x as usize, size.y as usize);
        let mut cells = vec![Vec::new(); cols * rows];
        let cell = |p: DVec2| {
            let c = ((p - min) / CELL_M).floor();
            (
                (c.x.max(0.0) as usize).min(cols - 1),
                (c.y.max(0.0) as usize).min(rows - 1),
            )
        };
        for j in 0..panel.triangles.len() {
            let t = flat_triangle(panel, j);
            let (lo, hi) = (t[0].min(t[1]).min(t[2]), t[0].max(t[1]).max(t[2]));
            let ((c0, r0), (c1, r1)) = (cell(lo), cell(hi));
            for r in r0..=r1 {
                for c in c0..=c1 {
                    cells[r * cols + c].push(j);
                }
            }
        }
        Self {
            panel,
            min,
            cols,
            cells,
        }
    }

    /// The triangle `p` is in (from its cell), or else the nearest triangle of the panel.
    fn find(&self, p: DVec2) -> Option<(usize, [f64; 3])> {
        let c = ((p - self.min) / CELL_M).floor();
        let rows = self.cells.len() / self.cols;
        if c.x >= 0.0 && c.y >= 0.0 && (c.x as usize) < self.cols && (c.y as usize) < rows {
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

/// Where the points of `panel` (a panel of the new fabric) start, carrying on from drape `old`:
/// each point where the same spot of the same piece was in the old cloth, found through the old
/// triangle that held it (welding renumbers triangles in place, so this reaches the live
/// particles); a point outside every old triangle carries on from the nearest one. None when
/// the old drape had no panel for the shape (it starts at its placement).
pub(crate) fn warm_positions(old: &Drape, panel: &PanelMesh) -> Option<Vec<DVec3>> {
    let was = old.fabric.panel(panel.shape)?;
    let index = Index::new(was);
    let cloth = old.solver.cloth();
    let (x, triangles) = (cloth.positions(), cloth.triangles());
    panel
        .flat
        .iter()
        .map(|f| {
            let (j, b) = index.find(DVec2::from_array(*f))?;
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
    use opendrape_core::{Half, Piece, PieceId, Pin, Placement, Point2, Project, SeamSide};
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
}
