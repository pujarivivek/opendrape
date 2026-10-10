//! The studio's light, from the baked HDRI (`environment_data.rs`, made by
//! `cargo xtask studio`): soft diffuse light as 9 spherical-harmonic coefficients, plus one
//! soft key light. The shaders evaluate the same functions; these are the CPU versions, used
//! to calibrate exposure and in tests.

use super::environment_data::{KEY_COLOUR, KEY_DIR, SH};
use glam::Vec3;

/// The 9 real SH basis functions (bands 0 to 2) at unit direction `n`, in the order the bake
/// writes the coefficients.
pub fn basis(n: Vec3) -> [f32; 9] {
    [
        0.282_095,
        0.488_603 * n.y,
        0.488_603 * n.z,
        0.488_603 * n.x,
        1.092_548 * n.x * n.y,
        1.092_548 * n.y * n.z,
        0.315_392 * (3.0 * n.z * n.z - 1.0),
        1.092_548 * n.x * n.z,
        0.546_274 * (n.x * n.x - n.y * n.y),
    ]
}

/// The soft light reaching a surface facing `n`, divided by π: a white matte surface facing
/// `n` sends back this much.
pub fn irradiance(n: Vec3) -> Vec3 {
    basis(n)
        .iter()
        .zip(SH)
        .fold(Vec3::ZERO, |e, (y, c)| e + Vec3::from_array(c) * *y)
}

/// The baked soft-light coefficients (irradiance/π, bands 0 to 2, RGB).
pub fn sh() -> [[f32; 3]; 9] {
    SH
}

/// Unit vector towards the key light.
pub fn key_dir() -> Vec3 {
    Vec3::from_array(KEY_DIR)
}

/// The key light at normal incidence, divided by π.
pub fn key_colour() -> Vec3 {
    Vec3::from_array(KEY_COLOUR)
}

/// Soft "wrapped" light for cloth: 1 facing the light, still a little just past its edge, and
/// none once the light is half a radian or so behind.
pub fn wrap(n_dot_l: f32) -> f32 {
    ((n_dot_l + 0.5) / 1.5).max(0.0)
}

pub fn luminance(c: Vec3) -> f32 {
    c.dot(Vec3::new(0.2126, 0.7152, 0.0722))
}

/// The exposure that makes the light on the front of a cloth piece seen from the front come
/// out at 1, so a matte fabric facing the camera shows its own colour.
pub fn exposure() -> f32 {
    let front = irradiance(Vec3::Z) + key_colour() * wrap(Vec3::Z.dot(key_dir()));
    1.0 / luminance(front)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sphere() -> impl Iterator<Item = Vec3> {
        let (w, h) = (64, 32);
        (0..h).flat_map(move |y| {
            (0..w).map(move |x| {
                let theta = std::f32::consts::PI * (y as f32 + 0.5) / h as f32;
                let phi = std::f32::consts::TAU * (x as f32 + 0.5) / w as f32;
                Vec3::new(
                    theta.sin() * phi.sin(),
                    theta.cos(),
                    theta.sin() * phi.cos(),
                )
            })
        })
    }

    #[test]
    fn the_soft_light_is_never_negative() {
        for n in sphere() {
            let e = irradiance(n);
            assert!(e.min_element() >= 0.0, "{n} gets {e}");
        }
    }

    #[test]
    fn the_key_light_comes_from_the_front_right_above() {
        let d = key_dir();
        assert!((d.length() - 1.0).abs() < 1e-5);
        let elevation = d.y.asin().to_degrees();
        let azimuth = d.x.atan2(d.z).to_degrees();
        // The bake clamps the height to 25°–60°; the file keeps 6 decimals.
        assert!((24.99..=60.01).contains(&elevation), "{elevation}");
        assert!((azimuth - 40.0).abs() < 0.1, "{azimuth}");
        assert!(key_colour().min_element() > 0.0);
    }

    #[test]
    fn the_light_is_neutral_grey() {
        let e = irradiance(Vec3::Z) + key_colour() * wrap(Vec3::Z.dot(key_dir()));
        assert!(
            (e.x - e.y).abs() < 0.05 * e.y && (e.z - e.y).abs() < 0.05 * e.y,
            "{e}"
        );
    }

    /// Exposure makes the light falling on the front of a matte piece, as the camera looks at
    /// it from the front, come out at 1: the piece shows its own colour.
    #[test]
    fn exposure_calibrates_the_front_to_one() {
        let e = irradiance(Vec3::Z) + key_colour() * wrap(Vec3::Z.dot(key_dir()));
        assert!((exposure() * luminance(e) - 1.0).abs() < 1e-5);
        assert!(exposure().is_finite() && exposure() > 0.0);
    }

    #[test]
    fn wrap_is_one_facing_the_light_and_zero_well_behind_it() {
        assert_eq!(wrap(1.0), 1.0);
        assert_eq!(wrap(-0.5), 0.0);
        assert_eq!(wrap(-1.0), 0.0);
        assert!(wrap(0.0) > 0.0, "light wraps a little past the edge");
    }
}
