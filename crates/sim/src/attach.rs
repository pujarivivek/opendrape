//! Attachments: a point of the cloth (a barycentric point in one of its triangles) pulled to a
//! target, with a stiffness. A grab is a stiff spring to a target that follows the pointer; a pin
//! holds its point at the target. A point is named by its triangle, not its particles: welding
//! renumbers the triangles' particles in place, so the point stays on the same fabric.

use crate::cloth::Cloth;
use glam::DVec3;

/// An attachment of the cloth it was made on; it stays valid until it is removed.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct AttachmentId(u32);

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Attachment {
    /// An index into the cloth's triangles.
    triangle: usize,
    bary: [f64; 3],
    target: DVec3,
    /// XPBD compliance (m/N): 0 holds the point exactly at its target.
    compliance: f64,
}

impl Cloth {
    /// Pulls the point at barycentric coordinates `bary` (summing to 1) of triangle `triangle`
    /// (an index into [`Cloth::triangles`]) to `target`, as a spring of `compliance` (m/N; 0
    /// holds it exactly there). None when there is no such triangle.
    pub fn attach(
        &mut self,
        triangle: usize,
        bary: [f64; 3],
        target: DVec3,
        compliance: f64,
    ) -> Option<AttachmentId> {
        (triangle < self.triangles.len()).then_some(())?;
        let id = AttachmentId(self.attachments.len() as u32);
        self.attachments.push(Some(Attachment {
            triangle,
            bary,
            target,
            compliance,
        }));
        Some(id)
    }

    /// Moves an attachment's target; false when it has been removed.
    pub fn move_attachment(&mut self, id: AttachmentId, target: DVec3) -> bool {
        match self.attachments.get_mut(id.0 as usize) {
            Some(Some(a)) => {
                a.target = target;
                true
            }
            _ => false,
        }
    }

    /// Lets go of an attachment; false when it was already removed.
    pub fn detach(&mut self, id: AttachmentId) -> bool {
        self.attachments
            .get_mut(id.0 as usize)
            .is_some_and(|a| a.take().is_some())
    }

    /// Where the attached point of the cloth is now; None once it is removed.
    pub fn attached_point(&self, id: AttachmentId) -> Option<DVec3> {
        let a = (*self.attachments.get(id.0 as usize)?)?;
        let t = self.triangles[a.triangle];
        Some((0..3).map(|k| self.x[t[k] as usize] * a.bary[k]).sum())
    }
}

/// One Gauss–Seidel pass over the attachments: each pulls its point towards its target, sharing
/// the move among the triangle's particles by their weight in the point and their inverse mass
/// (XPBD, as for the cloth's own links).
pub(crate) fn solve(c: &mut Cloth, sdt: f64) {
    for a in c.attachments.iter().flatten() {
        let t = c.triangles[a.triangle].map(|k| k as usize);
        let point: DVec3 = (0..3).map(|k| c.x[t[k]] * a.bary[k]).sum();
        let d = point - a.target;
        let len = d.length();
        let wsum: f64 = (0..3)
            .map(|k| c.inv_mass[t[k]] * a.bary[k] * a.bary[k])
            .sum();
        if len < 1e-12 || wsum == 0.0 {
            continue;
        }
        let lambda = -len / (wsum + a.compliance / (sdt * sdt));
        let n = d / len;
        for (&i, &b) in t.iter().zip(&a.bary) {
            c.x[i] += n * (lambda * c.inv_mass[i] * b);
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::{ClothBuilder, Panel, Params, Solver};
    use glam::{DVec2, DVec3};

    /// A 20 cm square of two triangles, hanging flat in the xy plane at height 1 m.
    fn square() -> Solver {
        let flat = vec![
            DVec2::new(0.0, 0.0),
            DVec2::new(0.2, 0.0),
            DVec2::new(0.2, 0.2),
            DVec2::new(0.0, 0.2),
        ];
        let panel = Panel {
            positions: flat
                .iter()
                .map(|p| DVec3::new(p.x, p.y + 1.0, 0.0))
                .collect(),
            flat: Some(flat),
            triangles: vec![[0, 1, 2], [0, 2, 3]],
        };
        let mut b = ClothBuilder::new(0.15);
        b.add_panel(&panel, 1.0);
        Solver::new(
            b.build(),
            Params {
                gravity_delay: 0.0,
                ..Params::default()
            },
        )
    }

    #[test]
    fn a_held_point_stays_at_its_target_while_the_rest_falls() {
        let mut s = square();
        // The middle of the top edge: halfway between corners 2 and 3, in triangle 1.
        let target = DVec3::new(0.1, 1.2, 0.0);
        let id = s
            .cloth_mut()
            .attach(1, [0.0, 0.5, 0.5], target, 0.0)
            .unwrap();
        for _ in 0..60 {
            s.step(None);
        }
        let held = s.cloth().attached_point(id).unwrap();
        assert!((held - target).length() < 1e-3, "{held}");
        assert!(s.cloth().positions()[0].y < 1.05, "the bottom swings down");
    }

    #[test]
    fn a_held_point_follows_its_target() {
        let mut s = square();
        let id = s
            .cloth_mut()
            .attach(0, [1.0 / 3.0; 3], DVec3::new(0.13, 1.07, 0.0), 1e-4)
            .unwrap();
        let to = DVec3::new(0.13, 1.07, 0.15);
        assert!(s.cloth_mut().move_attachment(id, to));
        for _ in 0..60 {
            s.step(None);
        }
        let held = s.cloth().attached_point(id).unwrap();
        assert!((held - to).length() < 0.01, "a stiff spring: {held}");
    }

    #[test]
    fn a_removed_attachment_lets_go() {
        let mut s = square();
        let id = s
            .cloth_mut()
            .attach(1, [0.0, 0.5, 0.5], DVec3::new(0.1, 1.2, 0.0), 0.0)
            .unwrap();
        for _ in 0..30 {
            s.step(None);
        }
        assert!(s.cloth_mut().detach(id));
        assert!(!s.cloth_mut().detach(id), "once");
        assert!(!s.cloth_mut().move_attachment(id, DVec3::ZERO));
        assert_eq!(s.cloth().attached_point(id), None);
        let before = s.cloth().positions()[2].y;
        for _ in 0..30 {
            s.step(None);
        }
        assert!(s.cloth().positions()[2].y < before - 0.05, "it falls");
        assert!(
            s.cloth_mut()
                .attach(2, [1.0, 0.0, 0.0], DVec3::ZERO, 0.0)
                .is_none()
        );
    }

    #[test]
    fn a_point_on_a_seam_stays_held_after_the_seam_welds() {
        // Two squares side by side, sewn along the edge between them; the pin is on the first
        // square's corner that welds to the second's.
        let flat = vec![
            DVec2::new(0.0, 0.0),
            DVec2::new(0.2, 0.0),
            DVec2::new(0.2, 0.2),
            DVec2::new(0.0, 0.2),
        ];
        let panel = |x: f64| Panel {
            positions: flat
                .iter()
                .map(|p| DVec3::new(p.x + x, p.y + 1.0, 0.0))
                .collect(),
            flat: Some(flat.clone()),
            triangles: vec![[0, 1, 2], [0, 2, 3]],
        };
        let mut b = ClothBuilder::new(0.15);
        let (p, q) = (
            b.add_panel(&panel(0.0), 1.0),
            b.add_panel(&panel(0.25), 1.0),
        );
        b.stitch((p, 1), (q, 0));
        b.stitch((p, 2), (q, 3));
        let mut s = Solver::new(b.build(), Params::default());
        let target = DVec3::new(0.22, 1.2, 0.0);
        // Corner 2 of the first square: triangle 0, its third corner.
        let id = s
            .cloth_mut()
            .attach(0, [0.0, 0.0, 1.0], target, 0.0)
            .unwrap();
        for _ in 0..120 {
            s.step(None);
        }
        assert!(!s.cloth().has_open_stitches(), "welded");
        let held = s.cloth().attached_point(id).unwrap();
        assert!((held - target).length() < 1e-3, "{held}");
    }
}
