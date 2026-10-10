//! Resizes a form's rings to target measurements in three moves:
//! 1. vertical stretches for lengths (above and below the waist);
//! 2. a sideways widening of the shoulders for the shoulder length;
//! 3. an exact uniform scale of every girth station about its centre, blended between
//!    stations by a monotone cubic so the surface stays smooth.
//!
//! The moves interact a little, so lengths are solved again a few times. Girths come out exact.

use super::file::{FormFile, Kind};
use super::rings::Rings;
use super::tape;
use super::{Measurements, SizeError};
use std::f64::consts::PI;

/// The largest stretch or widening, and the largest change of girth scale between neighbouring
/// stations; its inverse is the smallest. Keeps every shape plausible.
pub const MAX_RATIO: f64 = 1.33;
/// Lengths are solved to 0.05 mm and accepted within 2 mm.
const SOLVE_TOL: f64 = 0.000_05;
const ACCEPT_TOL: f64 = 0.002;
const PASSES: usize = 6;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Knob {
    Above,
    Below,
    Shoulder,
}

fn knob(measurement: &str) -> Knob {
    match measurement {
        "back_waist_length" => Knob::Above,
        "waist_to_hip" => Knob::Below,
        "shoulder_length" => Knob::Shoulder,
        other => unreachable!("FormFile::check allows no other adjustable length: {other}"),
    }
}

#[derive(Clone, Debug)]
struct Params {
    above: f64,
    below: f64,
    shoulder: f64,
    /// (station ring index, scale), sorted by ring.
    girth: Vec<(usize, f64)>,
}

impl Params {
    fn value(&self, k: Knob) -> f64 {
        match k {
            Knob::Above => self.above,
            Knob::Below => self.below,
            Knob::Shoulder => self.shoulder,
        }
    }
    fn slot(&mut self, k: Knob) -> &mut f64 {
        match k {
            Knob::Above => &mut self.above,
            Knob::Below => &mut self.below,
            Knob::Shoulder => &mut self.shoulder,
        }
    }
}

/// A length measurement (adjustable or read-only) on `rings`, metres: along its tape between the
/// two places `TORSO_LENGTHS` names, where a station is the height of its ring on `rings`.
pub fn length(file: &FormFile, rings: &Rings, measurement: &str) -> f64 {
    let &(_, tape_name, from, to) = file
        .lengths()
        .iter()
        .find(|l| l.0 == measurement)
        .expect("a known length");
    let span = |place: &str| {
        tape::place(file, rings, place)
            .expect("FormFile::check found every station a length is measured from or to")
    };
    tape::tape(file, rings, tape_name)
        .and_then(|t| t.length_between(span(from), span(to)))
        .expect("FormFile::check found each length's sampled tape, which crosses its stations")
}

/// Monotone cubic through (xs, ys) (Fritsch–Carlson), flat beyond the ends.
fn pchip(xs: Vec<f64>, ys: Vec<f64>) -> impl Fn(f64) -> f64 {
    let n = xs.len();
    let h: Vec<f64> = xs.windows(2).map(|w| w[1] - w[0]).collect();
    let d: Vec<f64> = (0..n.saturating_sub(1))
        .map(|i| (ys[i + 1] - ys[i]) / h[i])
        .collect();
    let mut m = vec![0.0; n];
    if n > 1 {
        m[0] = d[0];
        m[n - 1] = d[n - 2];
    }
    for i in 1..n.saturating_sub(1) {
        if d[i - 1] * d[i] > 0.0 {
            let (w1, w2) = (2.0 * h[i] + h[i - 1], h[i] + 2.0 * h[i - 1]);
            m[i] = (w1 + w2) / (w1 / d[i - 1] + w2 / d[i]);
        }
    }
    move |x: f64| {
        if n == 1 || x <= xs[0] {
            return ys[0];
        }
        if x >= xs[n - 1] {
            return ys[n - 1];
        }
        let i = xs.partition_point(|&a| a <= x) - 1;
        let t = (x - xs[i]) / h[i];
        let (t2, t3) = (t * t, t * t * t);
        (2.0 * t3 - 3.0 * t2 + 1.0) * ys[i]
            + (t3 - 2.0 * t2 + t) * h[i] * m[i]
            + (3.0 * t2 - 2.0 * t3) * ys[i + 1]
            + (t3 - t2) * h[i] * m[i + 1]
    }
}

fn apply(file: &FormFile, base: &Rings, p: &Params) -> Rings {
    let mut out = base.clone();
    match file.kind {
        Kind::Torso => {
            let waist = base.y[file.stations["waist"]];
            let neck = base.y_at(file.landmarks["back_neck"][1]);
            for (y, &y0) in out.y.iter_mut().zip(&base.y) {
                *y = if y0 < waist {
                    waist + p.below * (y0 - waist)
                } else if y0 <= neck {
                    waist + p.above * (y0 - waist)
                } else {
                    waist + p.above * (neck - waist) + (y0 - neck)
                };
            }
            // Widen the sides, most at the shoulder line, fading to nothing at bust and neck.
            let chest = file.stations[file.chest_station().expect("FormFile::check found it")];
            let (shoulder, neck_ring) = (file.stations["shoulder"], file.stations["neck"]);
            let half = out.half();
            for i in chest..=neck_ring {
                let t = if i <= shoulder {
                    (i - chest) as f64 / (shoulder - chest).max(1) as f64
                } else {
                    (neck_ring - i) as f64 / (neck_ring - shoulder).max(1) as f64
                };
                let w = t * t * (3.0 - 2.0 * t);
                for k in 0..half {
                    let side = (PI * k as f64 / (half - 1) as f64).sin().powi(2);
                    out.r[i][k] *= 1.0 + (p.shoulder - 1.0) * w * side;
                }
            }
        }
    }
    if !p.girth.is_empty() {
        let scale = pchip(
            p.girth.iter().map(|g| g.0 as f64).collect(),
            p.girth.iter().map(|g| g.1).collect(),
        );
        for (i, ring) in out.r.iter_mut().enumerate() {
            let c = scale(i as f64);
            for r in ring {
                *r *= c;
            }
        }
    }
    out
}

/// Secant solve of an increasing `f(x) = target` starting at `x0`; `None` if it fails.
fn solve(f: impl Fn(f64) -> f64, target: f64, x0: f64) -> Option<f64> {
    let (mut xa, mut fa) = (x0, f(x0) - target);
    if fa.abs() <= SOLVE_TOL {
        return Some(x0);
    }
    let mut xb = x0 * if fa > 0.0 { 0.98 } else { 1.02 };
    let mut fb = f(xb) - target;
    for _ in 0..40 {
        if fb.abs() <= SOLVE_TOL {
            return Some(xb);
        }
        if fb == fa {
            return None;
        }
        let x = xb - fb * (xb - xa) / (fb - fa);
        if !(x > 0.0 && x < 3.0) {
            return None;
        }
        (xa, fa) = (xb, fb);
        xb = x;
        fb = f(xb) - target;
    }
    None
}

/// `base` resized so every input of `file` matches `size` (mm), or the measurement that can't be.
pub fn resize(file: &FormFile, base: &Rings, size: &Measurements) -> Result<Rings, SizeError> {
    let range = |m: &str| {
        let [lo, hi] = file.ranges[m];
        (lo, hi)
    };
    let refuse = |m: &str, lo: f64, hi: f64| SizeError {
        measurement: m.to_string(),
        min_mm: lo,
        max_mm: hi,
    };
    for m in &file.inputs {
        let (lo, hi) = range(m);
        match size.get(m) {
            Some(v) if v.is_finite() && (lo..=hi).contains(v) => {}
            _ => return Err(refuse(m, lo, hi)),
        }
    }
    let target = |m: &str| size[m] / 1000.0;
    let mut girths: Vec<(usize, &str)> = file
        .inputs
        .iter()
        .filter_map(|m| file.stations.get(m).map(|&i| (i, m.as_str())))
        .collect();
    girths.sort_unstable();
    let lengths: Vec<&str> = file
        .inputs
        .iter()
        .map(String::as_str)
        .filter(|m| !file.stations.contains_key(*m))
        .collect();
    let mut p = Params {
        above: 1.0,
        below: 1.0,
        shoulder: 1.0,
        girth: girths.iter().map(|&(i, _)| (i, 1.0)).collect(),
    };
    let without_girths = |p: &Params| {
        apply(
            file,
            base,
            &Params {
                girth: vec![],
                ..p.clone()
            },
        )
    };
    for _ in 0..PASSES {
        for &m in &lengths {
            let k = knob(m);
            let f = |x: f64| {
                let mut q = p.clone();
                *q.slot(k) = x;
                length(file, &apply(file, base, &q), m)
            };
            let Some(x) = solve(f, target(m), p.value(k)) else {
                let (lo, hi) = range(m);
                return Err(refuse(m, lo, hi));
            };
            *p.slot(k) = x;
        }
        let unscaled = without_girths(&p);
        for (g, &(i, m)) in p.girth.iter_mut().zip(&girths) {
            g.1 = target(m) / unscaled.girth(i);
        }
        let sized = apply(file, base, &p);
        if lengths
            .iter()
            .all(|m| (length(file, &sized, m) - target(m)).abs() <= 10.0 * SOLVE_TOL)
        {
            break;
        }
    }
    let unscaled = without_girths(&p);
    let plausible = |x: f64| (1.0 / MAX_RATIO..=MAX_RATIO).contains(&x);
    for &m in &lengths {
        if !plausible(p.value(knob(m))) {
            let (lo, hi) = range(m);
            let own = length(file, base, m) * 1000.0;
            return Err(refuse(m, lo.max(own / MAX_RATIO), hi.min(own * MAX_RATIO)));
        }
    }
    for w in 1..p.girth.len() {
        let (c0, c1) = (p.girth[w - 1].1, p.girth[w].1);
        if !plausible(c1 / c0) {
            // Blame whichever of the two strays further from the form's own proportions.
            let (j, k) = if c1.ln().abs() >= c0.ln().abs() {
                (w, w - 1)
            } else {
                (w - 1, w)
            };
            let m = girths[j].1;
            let (lo, hi) = range(m);
            let own = unscaled.girth(girths[j].0) * 1000.0;
            let ck = p.girth[k].1;
            return Err(refuse(
                m,
                lo.max(ck / MAX_RATIO * own),
                hi.min(ck * MAX_RATIO * own),
            ));
        }
    }
    let sized = apply(file, base, &p);
    for &m in &lengths {
        if (length(file, &sized, m) - target(m)).abs() > ACCEPT_TOL {
            let (lo, hi) = range(m);
            return Err(refuse(m, lo, hi));
        }
    }
    Ok(sized)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::form::{ANGLES, fixture};
    use crate::girth_at;

    const WOMEN: &str = include_str!("../../../../assets/forms/women-torso.form.json");
    const MEN: &str = include_str!("../../../../assets/forms/men-torso.form.json");

    fn base() -> (FormFile, Rings) {
        let f = fixture::torso();
        let r = Rings::from_file(&f);
        (f, r)
    }

    fn real(json: &str) -> (FormFile, Rings) {
        let f = FormFile::from_json(json).expect("a shipped form");
        let r = Rings::from_file(&f);
        (f, r)
    }

    /// The form's own value for each input, mm.
    fn measured(f: &FormFile, r: &Rings) -> Measurements {
        f.inputs
            .iter()
            .map(|m| {
                let metres = match f.stations.get(m) {
                    Some(&i) => r.girth(i),
                    None => length(f, r, m),
                };
                (m.clone(), metres * 1000.0)
            })
            .collect()
    }

    /// Girths (sliced from the mesh, like a tape measure) within 1 mm; lengths within 2 mm.
    fn assert_fits(f: &FormFile, r: &Rings, target: &Measurements) {
        let mesh = r.mesh();
        for m in &f.inputs {
            let (got, tol) = match f.stations.get(m) {
                Some(&i) => (
                    1000.0 * f64::from(girth_at(&mesh, r.y[i] as f32, 10.0)),
                    1.0,
                ),
                None => (1000.0 * length(f, r, m), 2.0),
            };
            assert!(
                (got - target[m]).abs() <= tol,
                "{}: {m}: {got:.2} mm, wanted {:.2}",
                f.id,
                target[m]
            );
        }
    }

    fn scaled(m: &Measurements, by: &[(&str, f64)]) -> Measurements {
        let mut out = m.clone();
        for (name, k) in by {
            *out.get_mut(*name).expect("known input") *= k;
        }
        out
    }

    fn named(sizes: &[(&str, f64)]) -> Measurements {
        sizes.iter().map(|(m, v)| (m.to_string(), *v)).collect()
    }

    fn assert_same_shape(out: &Rings, r: &Rings) {
        for i in 0..r.len() {
            assert!((out.y[i] - r.y[i]).abs() < 1e-6, "ring {i} moved");
            for k in 0..ANGLES {
                assert!(
                    (out.r[i][k] - r.r[i][k]).abs() < 1e-6,
                    "ring {i} radius {k} changed"
                );
            }
        }
    }

    #[test]
    fn its_own_measurements_give_back_the_same_shape() {
        let (f, r) = base();
        let out = resize(&f, &r, &measured(&f, &r)).unwrap();
        assert_same_shape(&out, &r);
    }

    #[test]
    fn larger_and_smaller_targets_are_hit() {
        let (f, r) = base();
        let m = measured(&f, &r);
        for k in [1.08, 0.93] {
            let t = scaled(
                &m,
                &[
                    ("bust", k),
                    ("under_bust", k),
                    ("waist", k * k),
                    ("hip", k),
                    ("neck", k),
                    ("shoulder_length", k),
                    ("back_waist_length", k.sqrt()),
                    ("waist_to_hip", k),
                ],
            );
            assert_fits(&f, &resize(&f, &r, &t).unwrap(), &t);
        }
    }

    #[test]
    fn out_of_range_and_missing_inputs_are_refused_with_the_range() {
        let (f, r) = base();
        let mut t = measured(&f, &r);
        t.insert("waist".into(), 400.0);
        assert_eq!(
            resize(&f, &r, &t).unwrap_err(),
            SizeError {
                measurement: "waist".into(),
                min_mm: 500.0,
                max_mm: 1300.0
            }
        );
        let mut t = measured(&f, &r);
        t.remove("hip");
        assert_eq!(resize(&f, &r, &t).unwrap_err().measurement, "hip");
    }

    #[test]
    fn non_finite_input_is_refused() {
        let (f, r) = base();
        for bad in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            let mut t = measured(&f, &r);
            t.insert("bust".into(), bad);
            assert_eq!(resize(&f, &r, &t).unwrap_err().measurement, "bust");
        }
    }

    #[test]
    fn neighbours_that_fight_are_refused_naming_the_odd_one_out() {
        let (f, r) = base();
        let m = measured(&f, &r);
        let e = resize(&f, &r, &scaled(&m, &[("waist", 1.6)])).unwrap_err();
        assert_eq!(e.measurement, "waist");
        assert!(
            e.max_mm <= m["waist"] * 1.34 && e.min_mm < e.max_mm,
            "{e:?}"
        );
        let e = resize(&f, &r, &scaled(&m, &[("hip", 1.5)])).unwrap_err();
        assert_eq!(e.measurement, "hip");
    }

    #[test]
    fn any_targets_in_range_give_a_valid_shape_or_a_refusal() {
        let (f, r) = base();
        let mut seed = 0x2545_f491_4f6c_dd1d_u64;
        let mut next = move || {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            (seed >> 11) as f64 / (1u64 << 53) as f64
        };
        let mut sized = 0;
        for _ in 0..200 {
            let t: Measurements = f
                .inputs
                .iter()
                .map(|m| {
                    let [lo, hi] = f.ranges[m];
                    (m.clone(), lo + (hi - lo) * next())
                })
                .collect();
            match resize(&f, &r, &t) {
                Err(e) => assert!(f.inputs.contains(&e.measurement), "{e:?}"),
                Ok(out) => {
                    sized += 1;
                    assert!(out.y.windows(2).all(|w| w[1] > w[0]), "rings stay in order");
                    assert!(out.r.iter().flatten().all(|x| x.is_finite() && *x > 0.0));
                    assert_fits(&f, &out, &t);
                }
            }
        }
        // About one draw in eight is plausible; the rest are refused. Both paths ran.
        assert!((1..200).contains(&sized), "{sized} of 200 draws were sized");
    }

    #[test]
    fn resizing_is_deterministic_keeps_landmark_order_and_the_waist() {
        let (f, r) = base();
        let t = scaled(
            &measured(&f, &r),
            &[
                ("bust", 1.1),
                ("under_bust", 1.1),
                ("waist", 1.1),
                ("hip", 1.1),
                ("back_waist_length", 1.05),
                ("waist_to_hip", 0.95),
            ],
        );
        let (a, b) = (resize(&f, &r, &t).unwrap(), resize(&f, &r, &t).unwrap());
        assert_eq!(a, b);
        let y = |l: &str| a.y_at(f.landmarks[l][1]);
        assert!(y("back_neck") > y("bust_apex") && y("bust_apex") > y("front_waist"));
        assert!(y("front_waist") > y("cf_bottom"));
        let waist = f.stations["waist"];
        assert_eq!(a.y[waist], r.y[waist], "the waist stays at its height");
    }

    #[test]
    fn the_shipped_forms_give_back_their_own_shape_from_their_own_measurements() {
        for json in [WOMEN, MEN] {
            let (f, r) = real(json);
            let out = resize(&f, &r, &measured(&f, &r)).unwrap();
            assert_same_shape(&out, &r);
        }
    }

    #[test]
    fn the_womens_form_hits_targets_near_both_ends_of_its_charts() {
        let (f, r) = real(WOMEN);
        for t in [
            named(&[
                ("bust", 1050.0),
                ("under_bust", 900.0),
                ("waist", 820.0),
                ("hip", 1080.0),
                ("neck", 390.0),
                ("shoulder_length", 140.0),
                ("back_waist_length", 440.0),
                ("waist_to_hip", 210.0),
            ]),
            named(&[
                ("bust", 815.0),
                ("under_bust", 665.0),
                ("waist", 610.0),
                ("hip", 865.0),
                ("neck", 315.0),
                ("shoulder_length", 118.0),
                ("back_waist_length", 410.0),
                ("waist_to_hip", 195.0),
            ]),
        ] {
            let out = resize(&f, &r, &t).unwrap_or_else(|e| panic!("{e}"));
            assert_fits(&f, &out, &t);
        }
    }

    #[test]
    fn the_mens_form_hits_targets_near_both_ends_of_its_charts() {
        let (f, r) = real(MEN);
        for t in [
            named(&[
                ("chest", 1120.0),
                ("waist", 940.0),
                ("hip", 1120.0),
                ("neck", 420.0),
                ("shoulder_length", 165.0),
                ("back_waist_length", 500.0),
                ("waist_to_hip", 220.0),
            ]),
            named(&[
                ("chest", 920.0),
                ("waist", 740.0),
                ("hip", 920.0),
                ("neck", 370.0),
                ("shoulder_length", 150.0),
                ("back_waist_length", 470.0),
                ("waist_to_hip", 205.0),
            ]),
        ] {
            let out = resize(&f, &r, &t).unwrap_or_else(|e| panic!("{e}"));
            assert_fits(&f, &out, &t);
        }
    }
}
