//! Colour on the CPU, mirroring what the shaders do: sRGB encoding, Khronos PBR Neutral tone
//! mapping, and the CIEDE2000 colour difference the colour-accuracy tests are judged by.

/// sRGB-encoded 0..1 to linear 0..1 (the exact piecewise curve).
pub fn srgb_to_linear(c: f32) -> f32 {
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

/// Linear 0..1 to sRGB-encoded 0..1 (the exact piecewise curve).
pub fn linear_to_srgb(c: f32) -> f32 {
    if c <= 0.003_130_8 {
        c * 12.92
    } else {
        1.055 * c.powf(1.0 / 2.4) - 0.055
    }
}

/// An 8-bit sRGB colour (as a colour picker gives it) in linear RGB.
pub fn srgb8_to_linear(rgb: [u8; 3]) -> [f32; 3] {
    rgb.map(|c| srgb_to_linear(f32::from(c) / 255.0))
}

/// Khronos PBR Neutral tone mapping (2024): colours below the highlights come out as they
/// went in (less the 4 % a dielectric surface reflects), and only highlights are compressed,
/// keeping their hue. Linear Rec.709 in and out.
pub fn pbr_neutral([r, g, b]: [f32; 3]) -> [f32; 3] {
    const START: f32 = 0.8 - 0.04;
    const DESATURATION: f32 = 0.15;
    let x = r.min(g).min(b);
    let offset = if x < 0.08 { x - 6.25 * x * x } else { 0.04 };
    let c = [r - offset, g - offset, b - offset];
    let peak = c[0].max(c[1]).max(c[2]);
    if peak < START {
        return c;
    }
    let d = 1.0 - START;
    let new_peak = 1.0 - d * d / (peak + d - START);
    let c = c.map(|v| v * new_peak / peak);
    let g = 1.0 - 1.0 / (DESATURATION * (peak - new_peak) + 1.0);
    c.map(|v| v * (1.0 - g) + new_peak * g)
}

/// CIEDE2000 colour difference between two 8-bit sRGB colours (D65). About 1 is the smallest
/// difference people notice side by side; under 3 is hard to see.
pub fn delta_e2000(a: [u8; 3], b: [u8; 3]) -> f64 {
    delta_e2000_lab(srgb8_to_lab(a), srgb8_to_lab(b))
}

fn srgb8_to_lab(rgb: [u8; 3]) -> [f64; 3] {
    let [r, g, b] = srgb8_to_linear(rgb).map(f64::from);
    let x = 0.412_456_4 * r + 0.357_576_1 * g + 0.180_437_5 * b;
    let y = 0.212_672_9 * r + 0.715_152_2 * g + 0.072_175_0 * b;
    let z = 0.019_333_9 * r + 0.119_192_0 * g + 0.950_304_1 * b;
    let f = |t: f64| {
        const E: f64 = 6.0 / 29.0;
        if t > E * E * E {
            t.cbrt()
        } else {
            t / (3.0 * E * E) + 4.0 / 29.0
        }
    };
    let (fx, fy, fz) = (f(x / 0.950_47), f(y), f(z / 1.088_83));
    [116.0 * fy - 16.0, 500.0 * (fx - fy), 200.0 * (fy - fz)]
}

/// CIEDE2000 between two CIE Lab colours (Sharma, Wu and Dalal 2005), with kL = kC = kH = 1.
fn delta_e2000_lab([l1, a1, b1]: [f64; 3], [l2, a2, b2]: [f64; 3]) -> f64 {
    let pow7 = |v: f64| v.powi(7);
    let c_bar = (a1.hypot(b1) + a2.hypot(b2)) / 2.0;
    let g = 0.5 * (1.0 - (pow7(c_bar) / (pow7(c_bar) + pow7(25.0))).sqrt());
    let (a1p, a2p) = ((1.0 + g) * a1, (1.0 + g) * a2);
    let (c1p, c2p) = (a1p.hypot(b1), a2p.hypot(b2));
    let hue = |b: f64, a: f64| {
        if a == 0.0 && b == 0.0 {
            0.0
        } else {
            b.atan2(a).to_degrees().rem_euclid(360.0)
        }
    };
    let (h1p, h2p) = (hue(b1, a1p), hue(b2, a2p));
    let dl = l2 - l1;
    let dc = c2p - c1p;
    let dh = if c1p * c2p == 0.0 {
        0.0
    } else {
        let d = h2p - h1p;
        if d.abs() <= 180.0 {
            d
        } else if d > 180.0 {
            d - 360.0
        } else {
            d + 360.0
        }
    };
    let dh_big = 2.0 * (c1p * c2p).sqrt() * (dh / 2.0).to_radians().sin();
    let l_bar = (l1 + l2) / 2.0;
    let cp_bar = (c1p + c2p) / 2.0;
    let h_bar = if c1p * c2p == 0.0 {
        h1p + h2p
    } else if (h1p - h2p).abs() <= 180.0 {
        (h1p + h2p) / 2.0
    } else if h1p + h2p < 360.0 {
        (h1p + h2p + 360.0) / 2.0
    } else {
        (h1p + h2p - 360.0) / 2.0
    };
    let cos = |deg: f64| deg.to_radians().cos();
    let t =
        1.0 - 0.17 * cos(h_bar - 30.0) + 0.24 * cos(2.0 * h_bar) + 0.32 * cos(3.0 * h_bar + 6.0)
            - 0.20 * cos(4.0 * h_bar - 63.0);
    let d_theta = 30.0 * (-((h_bar - 275.0) / 25.0).powi(2)).exp();
    let r_c = 2.0 * (pow7(cp_bar) / (pow7(cp_bar) + pow7(25.0))).sqrt();
    let s_l = 1.0 + 0.015 * (l_bar - 50.0).powi(2) / (20.0 + (l_bar - 50.0).powi(2)).sqrt();
    let s_c = 1.0 + 0.045 * cp_bar;
    let s_h = 1.0 + 0.015 * cp_bar * t;
    let r_t = -(2.0 * d_theta).to_radians().sin() * r_c;
    let (l, c, h) = (dl / s_l, dc / s_c, dh_big / s_h);
    (l * l + c * c + h * h + r_t * c * h).sqrt()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn srgb_round_trips() {
        for x in 0..=255u8 {
            let back = linear_to_srgb(srgb_to_linear(f32::from(x) / 255.0)) * 255.0;
            assert_eq!(back.round() as u8, x);
        }
        assert_eq!(srgb8_to_linear([255, 0, 255]), [1.0, 0.0, 1.0]);
    }

    #[test]
    fn pbr_neutral_keeps_mid_colours_and_compresses_bright_ones() {
        let mid = pbr_neutral([0.5, 0.3, 0.2]);
        for (got, want) in mid.iter().zip([0.46, 0.26, 0.16]) {
            assert!((got - want).abs() < 1e-6, "{mid:?}");
        }
        let bright = pbr_neutral([2.0, 1.0, 0.5]);
        assert!(bright.iter().all(|&c| c < 1.0), "{bright:?}");
        assert!(bright[0] > bright[1] && bright[1] > bright[2], "{bright:?}");
        assert_eq!(pbr_neutral([0.0; 3]), [0.0; 3]);
    }

    /// Reference pairs from Sharma, Wu and Dalal (2005), "The CIEDE2000 color-difference formula".
    #[test]
    fn delta_e2000_matches_sharma_pairs() {
        let pairs = [
            ([50.0, 2.6772, -79.7751], [50.0, 0.0, -82.7485], 2.0425),
            ([50.0, -1.3802, -84.2814], [50.0, 0.0, -82.7485], 1.0000),
            ([50.0, 0.0, 0.0], [50.0, -1.0, 2.0], 2.3669),
            ([50.0, 2.5, 0.0], [73.0, 25.0, -18.0], 27.1492),
        ];
        for (a, b, want) in pairs {
            let got = delta_e2000_lab(a, b);
            assert!((got - want).abs() < 1e-3, "{a:?} {b:?}: {got} vs {want}");
        }
        assert_eq!(delta_e2000([118; 3], [118; 3]), 0.0);
        assert!(
            delta_e2000([118; 3], [121; 3]) < 1.5,
            "a barely visible step"
        );
    }
}
