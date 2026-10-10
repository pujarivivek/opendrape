//! `cargo xtask studio <in.hdr> <out.rs>`: bakes a studio HDRI into what the 3D view lights
//! with: nine spherical-harmonic coefficients of its diffuse light (white-balanced to neutral
//! grey) and one soft key light taken out of it, turned to come from the front-right, above.

use glam::DVec3;
use std::f64::consts::PI;
use std::fmt::Write as _;

/// The most of the diffuse light (its L0 band) that becomes the key light, which casts the
/// shadows. Less is taken when the light left over would otherwise go darker than
/// [`FLOOR_SHARE`] of its average somewhere (a room lit by one small, bright softbox).
const KEY_SHARE: f64 = 0.4;
const FLOOR_SHARE: f64 = 0.1;
/// Where the key light comes from: this many degrees round from the front (+Z) towards +X,
/// and this high, like a photo studio's softbox (shadows about as long as the figure is tall).
/// The HDRI's own brightest light is lower (bounce off a bright floor), which threw long
/// shadows.
const KEY_AZIMUTH_DEG: f64 = 40.0;
const KEY_ELEVATION_DEG: f64 = 50.0;

const Y00: f64 = 0.282_095;
const Y1: f64 = 0.488_603;
const Y2A: f64 = 1.092_548;
const Y2B: f64 = 0.315_392;
const Y2C: f64 = 0.546_274;

/// The 9 real SH basis functions (bands 0 to 2) at unit direction `d`, in the order the
/// renderer evaluates them.
fn basis(d: DVec3) -> [f64; 9] {
    [
        Y00,
        Y1 * d.y,
        Y1 * d.z,
        Y1 * d.x,
        Y2A * d.x * d.y,
        Y2A * d.y * d.z,
        Y2B * (3.0 * d.z * d.z - 1.0),
        Y2A * d.x * d.z,
        Y2C * (d.x * d.x - d.y * d.y),
    ]
}

/// The band each coefficient belongs to.
const BAND: [usize; 9] = [0, 1, 1, 1, 2, 2, 2, 2, 2];

/// Direction of pixel (x, y) of a `w`×`h` equirectangular image (row 0 at the top, +Y up),
/// turned `yaw` radians about +Y. Azimuth is measured from +Z towards +X.
fn direction(x: usize, y: usize, w: usize, h: usize, yaw: f64) -> DVec3 {
    let theta = PI * (y as f64 + 0.5) / h as f64;
    let phi = 2.0 * PI * (x as f64 + 0.5) / w as f64 + yaw;
    DVec3::new(
        theta.sin() * phi.sin(),
        theta.cos(),
        theta.sin() * phi.cos(),
    )
}

/// Projects an equirectangular radiance image (turned `yaw` about +Y) onto the 9 SH basis
/// functions, weighting each pixel by its solid angle.
fn project_sh9(w: usize, h: usize, pixels: &[[f32; 3]], yaw: f64) -> [[f64; 3]; 9] {
    let mut c = [[0.0; 3]; 9];
    let cell = (2.0 * PI / w as f64) * (PI / h as f64);
    for y in 0..h {
        let weight = cell * (PI * (y as f64 + 0.5) / h as f64).sin();
        for x in 0..w {
            let radiance = pixels[y * w + x];
            for (ci, yi) in c.iter_mut().zip(basis(direction(x, y, w, h, yaw))) {
                for (v, l) in ci.iter_mut().zip(radiance) {
                    *v += f64::from(l) * yi * weight;
                }
            }
        }
    }
    c
}

/// Radiance coefficients to irradiance/π: what a white matte surface sends back, per normal.
fn convolve(mut c: [[f64; 3]; 9]) -> [[f64; 3]; 9] {
    let factor = [1.0, 2.0 / 3.0, 0.25]; // Â_l / π, with Â = (π, 2π/3, π/4)
    for (ci, band) in c.iter_mut().zip(BAND) {
        for v in ci.iter_mut() {
            *v *= factor[band];
        }
    }
    c
}

/// Scales each colour channel so the overall (L0) light is neutral grey.
fn white_balance(mut c: [[f64; 3]; 9]) -> [[f64; 3]; 9] {
    let grey = c[0].iter().sum::<f64>() / 3.0;
    let scale = c[0].map(|v| if v > 0.0 { grey / v } else { 1.0 });
    for ci in &mut c {
        for (v, s) in ci.iter_mut().zip(scale) {
            *v *= s;
        }
    }
    c
}

fn luminance(c: [f64; 3]) -> f64 {
    0.2126 * c[0] + 0.7152 * c[1] + 0.0722 * c[2]
}

/// The direction most of the light comes from: the L1 band as a vector.
fn dominant(c: &[[f64; 3]; 9]) -> DVec3 {
    DVec3::new(luminance(c[3]), luminance(c[1]), luminance(c[2])).normalize_or(DVec3::Y)
}

/// What the renderer lights with.
struct Bake {
    /// Irradiance/π, bands 0 to 2, RGB; white-balanced, the key light taken out.
    sh: [[f64; 3]; 9],
    /// Unit vector towards the key light.
    key_dir: DVec3,
    /// The key light's irradiance/π at normal incidence (grey).
    key_colour: [f64; 3],
}

fn bake(w: usize, h: usize, pixels: &[[f32; 3]]) -> Bake {
    // Turn the environment so its brightest side is front-right.
    let first = white_balance(convolve(project_sh9(w, h, pixels, 0.0)));
    let d = dominant(&first);
    let yaw = KEY_AZIMUTH_DEG.to_radians() - d.x.atan2(d.z);
    let mut sh = white_balance(convolve(project_sh9(w, h, pixels, yaw)));
    let elevation = KEY_ELEVATION_DEG.to_radians();
    let azimuth = KEY_AZIMUTH_DEG.to_radians();
    let key_dir = DVec3::new(
        azimuth.sin() * elevation.cos(),
        elevation.sin(),
        azimuth.cos() * elevation.cos(),
    );
    // A directional light of irradiance I has L0 = I·Y00 in these units: take up to
    // KEY_SHARE of the grey L0 into it, and its whole projection out of the SH, as much as
    // leaves the rest at least FLOOR_SHARE of its average everywhere.
    let full = KEY_SHARE * sh[0][0] / Y00;
    let without = |intensity: f64| {
        let mut rest = sh;
        let projected = convolve(basis(key_dir).map(|y| [intensity * y; 3]));
        for (ci, pi) in rest.iter_mut().zip(projected) {
            for (v, p) in ci.iter_mut().zip(pi) {
                *v -= p;
            }
        }
        rest
    };
    let enough = |rest: &[[f64; 3]; 9]| lowest_irradiance(rest) >= FLOOR_SHARE * rest[0][0] * Y00;
    let intensity = if enough(&without(full)) {
        full
    } else {
        let (mut lo, mut hi) = (0.0, full);
        for _ in 0..40 {
            let mid = (lo + hi) / 2.0;
            if enough(&without(mid)) {
                lo = mid
            } else {
                hi = mid
            }
        }
        lo
    };
    sh = without(intensity);
    Bake {
        sh,
        key_dir,
        key_colour: [intensity / PI; 3],
    }
}

/// The least irradiance/π (luminance) the coefficients give over the sphere.
fn lowest_irradiance(sh: &[[f64; 3]; 9]) -> f64 {
    let (w, h) = (64, 32);
    (0..h)
        .flat_map(|y| (0..w).map(move |x| direction(x, y, w, h, 0.0)))
        .map(|n| {
            let rgb = basis(n).iter().zip(sh).fold([0.0; 3], |acc, (y, c)| {
                [acc[0] + y * c[0], acc[1] + y * c[1], acc[2] + y * c[2]]
            });
            luminance(rgb)
        })
        .fold(f64::INFINITY, f64::min)
}

/// The Rust source the renderer includes.
fn source(b: &Bake) -> String {
    let mut s = String::from(
        "//! Generated by `cargo xtask studio` from Poly Haven \"Studio Small 08\" by Sergej\n\
         //! Majboroda (CC0 1.0, https://polyhaven.com/a/studio_small_08), 1k HDR. Do not edit.\n\n\
         /// Irradiance/π as 9 SH coefficients (bands 0 to 2, RGB), white-balanced, key removed.\n\
         #[rustfmt::skip]\npub const SH: [[f32; 3]; 9] = [\n",
    );
    for c in b.sh {
        writeln!(s, "    [{:.6}, {:.6}, {:.6}],", c[0], c[1], c[2]).unwrap();
    }
    s.push_str("];\n\n/// Unit vector towards the key light.\n#[rustfmt::skip]\n");
    let d = b.key_dir;
    writeln!(
        s,
        "pub const KEY_DIR: [f32; 3] = [{:.6}, {:.6}, {:.6}];",
        d.x, d.y, d.z
    )
    .unwrap();
    s.push_str("\n/// The key light's irradiance/π at normal incidence.\n#[rustfmt::skip]\n");
    let k = b.key_colour;
    writeln!(
        s,
        "pub const KEY_COLOUR: [f32; 3] = [{:.6}, {:.6}, {:.6}];",
        k[0], k[1], k[2]
    )
    .unwrap();
    s
}

pub fn run(args: Vec<String>) {
    let [input, output] = args.as_slice() else {
        eprintln!(
            "usage: cargo xtask studio <in.hdr> <out.rs>   (run scripts/fetch-studio-hdri.sh first)"
        );
        std::process::exit(2);
    };
    let image = image::open(input)
        .unwrap_or_else(|e| panic!("can't read {input}: {e}"))
        .into_rgb32f();
    let (w, h) = (image.width() as usize, image.height() as usize);
    let pixels: Vec<[f32; 3]> = image.pixels().map(|p| p.0).collect();
    let b = bake(w, h, &pixels);
    std::fs::write(output, source(&b)).unwrap_or_else(|e| panic!("can't write {output}: {e}"));
    println!("{output}");
}

#[cfg(test)]
mod tests {
    use super::*;

    /// An equirectangular image, `w`×`h`, from a radiance function of direction.
    fn image(w: usize, h: usize, radiance: impl Fn(DVec3) -> [f32; 3]) -> Vec<[f32; 3]> {
        (0..h)
            .flat_map(|y| (0..w).map(move |x| (x, y)))
            .map(|(x, y)| radiance(direction(x, y, w, h, 0.0)))
            .collect()
    }

    #[test]
    fn a_constant_environment_has_only_l0() {
        // Fine enough that the grid's own error is well under the tolerance (the real HDRI is
        // 1024×512).
        let (w, h) = (256, 128);
        let c = project_sh9(w, h, &image(w, h, |_| [1.0; 3]), 0.0);
        let l0 = 0.282_095 * 4.0 * std::f64::consts::PI;
        assert!((c[0][0] - l0).abs() / l0 < 2e-3, "{}", c[0][0]);
        for band in &c[1..] {
            assert!(band.iter().all(|v| v.abs() < 1e-3), "{c:?}");
        }
    }

    #[test]
    fn white_balance_makes_l0_grey() {
        let (w, h) = (64, 32);
        let c = project_sh9(w, h, &image(w, h, |d| [2.0 + d.y as f32, 1.0, 0.5]), 0.0);
        let wb = white_balance(convolve(c));
        assert!((wb[0][0] - wb[0][1]).abs() < 1e-9 && (wb[0][1] - wb[0][2]).abs() < 1e-9);
    }

    /// A bright patch somewhere above: the key ends up front-right, at +40° azimuth.
    fn softbox_room(d: DVec3) -> [f32; 3] {
        let towards = DVec3::new(-0.6, 0.6, -0.5).normalize();
        let lit = if d.dot(towards) > 0.9 { 40.0 } else { 0.5 };
        [lit; 3]
    }

    #[test]
    fn the_key_ends_up_front_right_above() {
        let (w, h) = (128, 64);
        let b = bake(w, h, &image(w, h, softbox_room));
        let azimuth = b.key_dir.x.atan2(b.key_dir.z).to_degrees();
        let elevation = b.key_dir.y.asin().to_degrees();
        assert!((azimuth - 40.0).abs() < 2.0, "azimuth {azimuth}");
        assert!((elevation - 50.0).abs() < 1e-6, "elevation {elevation}");
    }

    #[test]
    fn taking_the_key_out_keeps_the_light_the_same_in_total() {
        let (w, h) = (128, 64);
        let pixels = image(w, h, softbox_room);
        let before = white_balance(convolve(project_sh9(w, h, &pixels, 0.0)))[0][0];
        let b = bake(w, h, &pixels);
        // The key's own share of L0 is its intensity times Y00 (π·key colour · Y00 / π · π).
        let key_l0 = b.key_colour[0] * std::f64::consts::PI * Y00;
        assert!(((b.sh[0][0] + key_l0) - before).abs() / before < 1e-6);
        assert!(key_l0 / before <= KEY_SHARE + 1e-9 && key_l0 > 0.0);
    }

    /// The soft light left after the key is taken out is nowhere darker than a tenth of its
    /// average, even when the room's light comes from one small, bright patch (as long as the
    /// room's own diffuse light isn't that dark anywhere: three bands can't do better).
    #[test]
    fn the_light_left_over_never_goes_dark() {
        let (w, h) = (128, 64);
        let sharp_room = |d: DVec3| {
            let lit = d.dot(DVec3::new(0.3, 0.2, 0.9).normalize()) > 0.97;
            [if lit { 60.0 } else { 0.5 }; 3]
        };
        // Like the real studio, whose light partly comes up off a bright floor: the brightest
        // patch a little below the horizon, so the key (set at least 25° up) would take light
        // out of the wrong place if all of KEY_SHARE went into it.
        let low_softbox = |d: DVec3| {
            let lit = d.dot(DVec3::new(-0.5, -0.2, -0.85).normalize()) > 0.8;
            [if lit { 12.0 } else { 0.4 }; 3]
        };
        for room in [
            softbox_room as fn(DVec3) -> [f32; 3],
            sharp_room,
            low_softbox,
        ] {
            let pixels = image(w, h, room);
            let own = white_balance(convolve(project_sh9(w, h, &pixels, 0.0)));
            assert!(
                lowest_irradiance(&own) >= 0.1 * own[0][0] * Y00,
                "a fair room"
            );
            let b = bake(w, h, &pixels);
            let average = b.sh[0][0] * Y00;
            assert!(
                lowest_irradiance(&b.sh) >= 0.1 * average - 1e-9,
                "{:?}",
                b.sh
            );
        }
    }
}
