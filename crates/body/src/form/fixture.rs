//! A synthetic torso for unit tests: smooth elliptical rings, the 15 landmarks of the shipped
//! forms, sampled tape lines drawn straight in (phi, v), and the shipped women's meta file's
//! inputs, ranges and collision settings.

use super::file::{ANGLES, Collision, FORMAT, FormFile, Kind, NeckCut, Ring, Stand, TapeDef};
use serde::de::DeserializeOwned;
use std::collections::BTreeMap;
use std::f64::consts::{PI, TAU};

/// Samples per tape line.
const SAMPLES: usize = 40;
/// Height of the bottom ring, and the gap between rings, metres.
const Y0: f64 = 0.70;
const STEP: f64 = 0.01;
/// Index of the top ring.
const TOP: usize = 80;

/// Piecewise-linear lookup in a (y, value) table sorted by y, flat beyond its ends.
fn lerp(table: &[(f64, f64)], y: f64) -> f64 {
    let k = table
        .partition_point(|p| p.0 <= y)
        .clamp(1, table.len() - 1);
    let ((y0, a), (y1, b)) = (table[k - 1], table[k]);
    a + (b - a) * ((y - y0) / (y1 - y0)).clamp(0.0, 1.0)
}

/// An ellipse of half-width `a` (x) and depth `b` (z), metres, as ring radii in millimetres.
fn ellipse_ring(y: f64, a: f64, b: f64) -> Ring {
    let r = (0..ANGLES)
        .map(|k| {
            let t = PI * k as f64 / (ANGLES - 1) as f64;
            1000.0 * a * b / ((b * t.sin()).powi(2) + (a * t.cos()).powi(2)).sqrt()
        })
        .collect();
    Ring { y, zc: 0.0, r }
}

fn field<T: DeserializeOwned>(meta: &serde_json::Value, key: &str) -> T {
    serde_json::from_value(meta[key].clone()).expect("meta field")
}

/// The v of a height: 0 at the bottom ring, 1 at the top.
fn v_at(y: f64) -> f64 {
    (y - Y0) / (STEP * TOP as f64)
}

/// `SAMPLES` points evenly spaced along the polyline through `pts`, both ends included.
fn sample(pts: &[[f64; 2]]) -> Vec<[f64; 2]> {
    let len: Vec<f64> = pts
        .windows(2)
        .map(|w| (w[1][0] - w[0][0]).hypot(w[1][1] - w[0][1]))
        .collect();
    let total: f64 = len.iter().sum();
    (0..SAMPLES)
        .map(|k| {
            let mut d = total * k as f64 / (SAMPLES - 1) as f64;
            let mut i = 0;
            while i + 1 < len.len() && d > len[i] {
                d -= len[i];
                i += 1;
            }
            let t = if len[i] > 0.0 { d / len[i] } else { 0.0 };
            let (a, b) = (pts[i], pts[i + 1]);
            [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t]
        })
        .collect()
}

fn line(tape: &[[f64; 2]], closed: bool, mirror: bool) -> TapeDef {
    TapeDef::Samples {
        uv: tape.to_vec(),
        closed,
        mirror,
    }
}

/// 81 rings 1 cm apart from y = 0.70 m; the waist (ring 33) is at 1.03 m.
pub fn torso() -> FormFile {
    const A: [(f64, f64); 7] = [
        (0.70, 0.150),
        (0.82, 0.165),
        (1.03, 0.130),
        (1.21, 0.160),
        (1.37, 0.165),
        (1.43, 0.060),
        (1.50, 0.055),
    ];
    const B: [(f64, f64); 7] = [
        (0.70, 0.100),
        (0.82, 0.110),
        (1.03, 0.085),
        (1.21, 0.100),
        (1.37, 0.070),
        (1.43, 0.055),
        (1.50, 0.050),
    ];
    let rings = (0..=TOP)
        .map(|i| {
            let y = Y0 + STEP * i as f64;
            ellipse_ring(y, lerp(&A, y), lerp(&B, y))
        })
        .collect();

    // The 15 landmarks: (name, degrees from centre front, height in metres).
    let landmarks: BTreeMap<String, [f64; 2]> = [
        ("front_neck", 0.0, 1.41),
        ("side_neck", 108.0, 1.43),
        ("back_neck", 180.0, 1.44),
        ("shoulder_point", 92.0, 1.37),
        ("plate_centre", 92.0, 1.32),
        ("armhole_front", 68.0, 1.32),
        ("armhole_bottom", 92.0, 1.26),
        ("armhole_back", 116.0, 1.32),
        ("bust_apex", 38.0, 1.21),
        ("shoulder_blade", 140.0, 1.31),
        ("cb_blade", 180.0, 1.31),
        ("front_waist", 0.0, 1.03),
        ("back_waist", 180.0, 1.03),
        ("cf_bottom", 0.0, 0.72),
        ("cb_bottom", 180.0, 0.72),
    ]
    .into_iter()
    .map(|(name, deg, y)| (name.to_string(), [f64::to_radians(deg), v_at(y)]))
    .collect();
    let lm = |name: &str| landmarks[name];

    // Every tape is a straight line (or a few) in (phi, v), sampled 40 times.
    let [front_neck, side_neck, back_neck] = ["front_neck", "side_neck", "back_neck"].map(lm);
    let [shoulder_point, plate_centre, armhole_back] =
        ["shoulder_point", "plate_centre", "armhole_back"].map(lm);
    let [armhole_bottom, bust_apex, shoulder_blade] =
        ["armhole_bottom", "bust_apex", "shoulder_blade"].map(lm);
    let [cf_bottom, cb_bottom] = ["cf_bottom", "cb_bottom"].map(lm);

    // An ellipse in (phi, v) round the plate centre through the armhole's front, bottom and back.
    // It starts at the back (theta 0) and runs over the top to the front.
    let (da, dv) = (
        armhole_back[0] - plate_centre[0],
        plate_centre[1] - armhole_bottom[1],
    );
    let armhole: Vec<[f64; 2]> = (0..SAMPLES)
        .map(|k| {
            let theta = TAU * k as f64 / SAMPLES as f64;
            [
                plate_centre[0] + da * theta.cos(),
                plate_centre[1] + dv * theta.sin(),
            ]
        })
        .collect();

    // A neckline runs `half` either side of its middle (centre front or back), at the middle's
    // height rising to the side neck's. Phi wraps round 2π: the front starts on the right.
    let neckline = |mid: f64, half: f64, mid_v: f64, side_v: f64| -> Vec<[f64; 2]> {
        (0..SAMPLES)
            .map(|k| {
                let s = half * (2.0 * k as f64 / (SAMPLES - 1) as f64 - 1.0);
                let v = mid_v + (side_v - mid_v) * s.abs() / half;
                [(mid + s).rem_euclid(TAU), v]
            })
            .collect()
    };

    // The princess seam starts halfway along the shoulder seam.
    let shoulder_mid = [
        (side_neck[0] + shoulder_point[0]) / 2.0,
        (side_neck[1] + shoulder_point[1]) / 2.0,
    ];
    let down = |phi_deg: f64, y: f64| [f64::to_radians(phi_deg), v_at(y)];

    let tapes: BTreeMap<String, TapeDef> = [
        ("cf", line(&sample(&[front_neck, cf_bottom]), false, false)),
        ("cb", line(&sample(&[back_neck, cb_bottom]), false, false)),
        (
            "side_seam",
            line(
                &sample(&[armhole_bottom, [armhole_bottom[0], 0.0]]),
                false,
                true,
            ),
        ),
        (
            "shoulder_seam",
            line(&sample(&[side_neck, shoulder_point]), false, true),
        ),
        ("armhole", line(&armhole, true, true)),
        (
            "neckline_front",
            line(
                &neckline(0.0, side_neck[0], front_neck[1], side_neck[1]),
                false,
                false,
            ),
        ),
        (
            "neckline_back",
            line(
                &neckline(PI, PI - side_neck[0], back_neck[1], side_neck[1]),
                false,
                false,
            ),
        ),
        (
            "princess_front",
            line(
                &sample(&[
                    shoulder_mid,
                    bust_apex,
                    down(30.0, 1.03),
                    down(30.0, 0.82),
                    down(30.0, 0.72),
                ]),
                false,
                true,
            ),
        ),
        (
            "princess_back",
            line(
                &sample(&[
                    shoulder_mid,
                    shoulder_blade,
                    down(150.0, 1.03),
                    down(150.0, 0.82),
                    down(150.0, 0.72),
                ]),
                false,
                true,
            ),
        ),
    ]
    .into_iter()
    .map(|(name, tape)| (name.to_string(), tape))
    .chain(["bust", "under_bust", "waist", "high_hip", "hip"].map(|s| {
        (
            s.to_string(),
            TapeDef::Ring {
                ring: s.to_string(),
            },
        )
    }))
    .collect();

    let meta: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../assets-src/forms/women-torso.meta.json"
    ))
    .expect("meta is JSON");
    FormFile {
        format: FORMAT,
        id: "test".into(),
        kind: Kind::Torso,
        name: "form-test".into(),
        suits: vec![],
        licence: "CC0-1.0".into(),
        base_size: "test".into(),
        angles: ANGLES,
        rings,
        stations: [
            ("bottom", 2),
            ("hip", 12),
            ("high_hip", 26),
            ("waist", 33),
            ("under_bust", 43),
            ("bust", 51),
            ("shoulder", 67),
            ("neck", 75),
        ]
        .map(|(s, i)| (s.to_string(), i))
        .into(),
        landmarks,
        tapes,
        stand: Stand {
            pole_xz: [0.0, 0.0],
            neck_cut: NeckCut {
                y: 1.52,
                tilt_deg: 17.0,
            },
        },
        inputs: field(&meta, "inputs"),
        ranges: field(&meta, "ranges"),
        collision: field::<Collision>(&meta, "collision"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn uv<'a>(f: &'a FormFile, tape: &str) -> &'a [[f64; 2]] {
        match &f.tapes[tape] {
            TapeDef::Samples { uv, .. } => uv,
            TapeDef::Ring { .. } => panic!("{tape} is a ring"),
        }
    }

    fn near(a: [f64; 2], b: [f64; 2]) -> bool {
        (a[0] - b[0]).abs() < 1e-9 && (a[1] - b[1]).abs() < 1e-9
    }

    #[test]
    fn it_has_the_fifteen_landmarks_of_the_shipped_forms() {
        let f = torso();
        let names: Vec<&str> = f.landmarks.keys().map(String::as_str).collect();
        assert_eq!(
            names,
            [
                "armhole_back",
                "armhole_bottom",
                "armhole_front",
                "back_neck",
                "back_waist",
                "bust_apex",
                "cb_blade",
                "cb_bottom",
                "cf_bottom",
                "front_neck",
                "front_waist",
                "plate_centre",
                "shoulder_blade",
                "shoulder_point",
                "side_neck",
            ]
        );
    }

    #[test]
    fn stations_sit_at_their_heights() {
        let f = torso();
        assert_eq!(f.rings.len(), TOP + 1);
        let y = |s: &str| f.rings[f.stations[s]].y;
        assert!((y("waist") - 1.03).abs() < 1e-9);
        assert!((y("hip") - 0.82).abs() < 1e-9);
        assert!((y("bust") - 1.21).abs() < 1e-9);
    }

    #[test]
    fn tapes_start_and_end_at_their_landmarks() {
        let f = torso();
        let lm = |n: &str| f.landmarks[n];
        for (tape, first, last) in [
            ("cf", lm("front_neck"), lm("cf_bottom")),
            ("cb", lm("back_neck"), lm("cb_bottom")),
            (
                "side_seam",
                lm("armhole_bottom"),
                [lm("armhole_bottom")[0], 0.0],
            ),
            ("shoulder_seam", lm("side_neck"), lm("shoulder_point")),
        ] {
            let t = uv(&f, tape);
            assert_eq!(t.len(), SAMPLES, "{tape}");
            assert!(near(t[0], first), "{tape} starts at {:?}", t[0]);
            assert!(
                near(t[SAMPLES - 1], last),
                "{tape} ends at {:?}",
                t[SAMPLES - 1]
            );
        }
    }

    #[test]
    fn the_armhole_is_a_closed_loop_through_its_landmarks() {
        let f = torso();
        let t = uv(&f, "armhole");
        assert_eq!(t.len(), SAMPLES);
        for name in ["armhole_front", "armhole_bottom", "armhole_back"] {
            assert!(t.iter().any(|&p| near(p, f.landmarks[name])), "{name}");
        }
    }

    #[test]
    fn necklines_meet_at_the_side_neck_and_the_front_one_wraps_past_two_pi() {
        let f = torso();
        let side = f.landmarks["side_neck"];
        let (front, back) = (uv(&f, "neckline_front"), uv(&f, "neckline_back"));
        assert!(near(front[0], [TAU - side[0], side[1]]));
        assert!(near(front[SAMPLES - 1], side));
        assert!(near(back[0], side));
        assert!(near(back[SAMPLES - 1], [TAU - side[0], side[1]]));
        // The front one crosses centre front, where phi drops from near 2π to near 0.
        assert!(front.windows(2).any(|w| w[0][0] - w[1][0] > PI));
        // The back one passes centre back (phi = π) without wrapping.
        assert!(back.windows(2).all(|w| w[1][0] > w[0][0]));
        assert!(back.iter().any(|p| (p[0] - PI).abs() < 0.1));
    }

    #[test]
    fn the_princess_seams_pass_the_bust_apex_and_the_shoulder_blade() {
        let f = torso();
        let near_a_sample = |tape: &str, target: [f64; 2]| {
            uv(&f, tape)
                .iter()
                .any(|p| (p[0] - target[0]).abs() < 0.05 && (p[1] - target[1]).abs() < 0.05)
        };
        assert!(near_a_sample("princess_front", f.landmarks["bust_apex"]));
        assert!(near_a_sample(
            "princess_back",
            f.landmarks["shoulder_blade"]
        ));
    }
}
