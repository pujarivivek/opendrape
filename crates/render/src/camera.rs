use glam::{Mat4, Vec3};

/// A camera orbiting a target point: drag to rotate, scroll to zoom.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct OrbitCamera {
    pub target: Vec3,
    /// Radians around the Y axis; 0 looks at the target from +Z.
    pub yaw: f32,
    /// Radians above the horizon.
    pub pitch: f32,
    /// Metres from the target.
    pub distance: f32,
    pub fov_y: f32,
}

impl OrbitCamera {
    pub const MIN_PITCH: f32 = -1.45;
    pub const MAX_PITCH: f32 = 1.45;
    pub const MIN_DISTANCE: f32 = 0.3;
    pub const MAX_DISTANCE: f32 = 20.0;

    pub fn eye(&self) -> Vec3 {
        let (sy, cy) = self.yaw.sin_cos();
        let (sp, cp) = self.pitch.sin_cos();
        self.target + Vec3::new(sy * cp, sp, cy * cp) * self.distance
    }

    /// Rotate by a mouse drag measured in screen points.
    pub fn drag(&mut self, dx: f32, dy: f32) {
        self.yaw -= dx * 0.01;
        self.pitch = (self.pitch + dy * 0.01).clamp(Self::MIN_PITCH, Self::MAX_PITCH);
    }

    /// Zoom by a scroll amount in points (positive moves closer).
    pub fn zoom(&mut self, scroll: f32) {
        self.distance =
            (self.distance * (-scroll * 0.002).exp()).clamp(Self::MIN_DISTANCE, Self::MAX_DISTANCE);
    }

    pub fn view_proj(&self, aspect: f32) -> Mat4 {
        self.proj(aspect) * self.view()
    }

    /// World to camera space (right-handed, looking down −Z).
    pub fn view(&self) -> Mat4 {
        glam::camera::rh::view::look_at_mat4(self.eye(), self.target, Vec3::Y)
    }

    /// Camera space to clip space, with wgpu's 0..1 depth.
    pub fn proj(&self, aspect: f32) -> Mat4 {
        glam::camera::rh::proj::directx::perspective(self.fov_y, aspect.max(1e-3), 0.05, 100.0)
    }
}

impl Default for OrbitCamera {
    fn default() -> Self {
        Self {
            target: Vec3::ZERO,
            yaw: 0.6,
            pitch: 0.35,
            distance: 3.5,
            fov_y: 45f32.to_radians(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn eye_is_in_front_at_zero_angles() {
        let cam = OrbitCamera {
            yaw: 0.0,
            pitch: 0.0,
            distance: 2.0,
            ..Default::default()
        };
        assert!(
            cam.eye().abs_diff_eq(Vec3::new(0.0, 0.0, 2.0), 1e-6),
            "{}",
            cam.eye()
        );
    }

    #[test]
    fn drag_clamps_pitch() {
        let mut cam = OrbitCamera::default();
        cam.drag(0.0, 10_000.0);
        assert_eq!(cam.pitch, OrbitCamera::MAX_PITCH);
        cam.drag(0.0, -10_000.0);
        assert_eq!(cam.pitch, OrbitCamera::MIN_PITCH);
    }

    #[test]
    fn zoom_clamps_distance() {
        let mut cam = OrbitCamera::default();
        cam.zoom(1e6);
        assert_eq!(cam.distance, OrbitCamera::MIN_DISTANCE);
        cam.zoom(-1e6);
        assert_eq!(cam.distance, OrbitCamera::MAX_DISTANCE);
    }

    #[test]
    fn target_projects_to_screen_centre_with_wgpu_depth() {
        let cam = OrbitCamera::default();
        let clip = cam.view_proj(16.0 / 9.0) * cam.target.extend(1.0);
        let ndc = clip.truncate() / clip.w;
        assert!(ndc.x.abs() < 1e-5 && ndc.y.abs() < 1e-5, "{ndc}");
        assert!(
            (0.0..=1.0).contains(&ndc.z),
            "depth must be in wgpu's 0..1 range: {}",
            ndc.z
        );
    }
}
