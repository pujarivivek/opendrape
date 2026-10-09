use glam::DVec3;

/// A contact plane for one particle: the closest point on the body and the outward normal there.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Plane {
    pub normal: DVec3,
    pub point: DVec3,
}

/// Something cloth collides with. Queried once per frame for every particle; `None` means
/// the particle is farther than `margin` from it (and outside).
pub trait Collider: Sync {
    fn contact_planes(&self, x: &[DVec3], margin: f64) -> Vec<Option<Plane>>;
}
