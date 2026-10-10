//! Resizes a form's rings to target measurements in three moves:
//! 1. vertical stretches for lengths (above and below the waist);
//! 2. a sideways widening of the shoulders for the shoulder length;
//! 3. an exact uniform scale of every girth station about its centre, blended between
//!    stations by a monotone cubic so the surface stays smooth.
//!
//! The moves interact a little, so lengths are solved again a few times. Girths come out exact.
//!
//! A size is refused, naming one measurement and the range it could take here, when two
//! neighbouring girths would need scales more than `MAX_RATIO` apart (checked first: it also
//! drags the lengths about), or when a length would need a stretch beyond `MAX_RATIO`.

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
///
/// # Panics
/// If `measurement` is not one of the form's lengths, or `file` has not passed
/// `FormFile::check` (which sees to it that every tape and station the length needs is there).
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

/// Monotone cubic through (xs, ys) (Fritsch–Carlson), flat beyond the ends. `xs` must rise
/// strictly (`FormFile::check` sees to it: no two inputs share a ring). The slope is 0 at both
/// ends, where the value turns flat, so the curve has no kink there.
fn pchip(xs: Vec<f64>, ys: Vec<f64>) -> impl Fn(f64) -> f64 {
    let n = xs.len();
    let h: Vec<f64> = xs.windows(2).map(|w| w[1] - w[0]).collect();
    let d: Vec<f64> = (0..n.saturating_sub(1))
        .map(|i| (ys[i + 1] - ys[i]) / h[i])
        .collect();
    // Zero at the ends and wherever the data turns round.
    let mut m = vec![0.0; n];
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
fn solve(f: &impl Fn(f64) -> f64, target: f64, x0: f64) -> Option<f64> {
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

/// The range a measurement may take: `reach` (what the form can do here) within `declared` (what
/// the form file allows). If the two don't overlap, just `reach`, so the range is never inverted.
fn narrowed(declared: [f64; 2], reach: (f64, f64)) -> (f64, f64) {
    let (lo, hi) = (declared[0].max(reach.0), declared[1].min(reach.1));
    if lo <= hi { (lo, hi) } else { reach }
}

/// One resizing job: the form, the sizes asked for, and the inputs sorted into girths and lengths.
struct Job<'a> {
    file: &'a FormFile,
    base: &'a Rings,
    size: &'a Measurements,
    /// (station ring, input name) of each girth input, by ring.
    girths: Vec<(usize, &'a str)>,
    lengths: Vec<&'a str>,
}

impl Job<'_> {
    fn refuse(&self, m: &str, reach: (f64, f64)) -> SizeError {
        let (min_mm, max_mm) = narrowed(self.file.ranges[m], reach);
        SizeError {
            measurement: m.to_string(),
            min_mm,
            max_mm,
        }
    }

    /// Metres.
    fn target(&self, m: &str) -> f64 {
        self.size[m] / 1000.0
    }

    /// `p`'s lengths and widening, without its girth scales.
    fn unscaled(&self, p: &Params) -> Rings {
        let flat = Params {
            girth: vec![],
            ..p.clone()
        };
        apply(self.file, self.base, &flat)
    }

    /// Sets the girth scales of `p` so every girth input hits its target, given the rest of `p`.
    fn fit_girths(&self, p: &mut Params) {
        let unscaled = self.unscaled(p);
        for (g, &(i, m)) in p.girth.iter_mut().zip(&self.girths) {
            g.1 = self.target(m) / unscaled.girth(i);
        }
    }

    /// The shortest and longest (mm) that length `m` can be made by its own stretch within
    /// `MAX_RATIO`, with every other move as in `p` and the girths still on target.
    fn reach(&self, p: &Params, m: &str) -> (f64, f64) {
        let k = knob(m);
        let at = |x: f64| {
            let mut q = p.clone();
            *q.slot(k) = x;
            self.fit_girths(&mut q);
            1000.0 * length(self.file, &apply(self.file, self.base, &q), m)
        };
        let (a, b) = (at(1.0 / MAX_RATIO), at(MAX_RATIO));
        (a.min(b), a.max(b))
    }

    /// The girth stations' scales must change gently from one to the next. If two neighbours
    /// fight, the refusal blames whichever strays further from the form's own proportions.
    fn check_girth_neighbours(&self, p: &Params) -> Result<(), SizeError> {
        let unscaled = self.unscaled(p);
        for w in 1..p.girth.len() {
            let (c0, c1) = (p.girth[w - 1].1, p.girth[w].1);
            if (1.0 / MAX_RATIO..=MAX_RATIO).contains(&(c1 / c0)) {
                continue;
            }
            let (j, k) = if c1.ln().abs() >= c0.ln().abs() {
                (w, w - 1)
            } else {
                (w - 1, w)
            };
            // j's girth, with k's scale, must stay within `MAX_RATIO` of it on either side.
            let own = unscaled.girth(self.girths[j].0) * 1000.0;
            let ck = p.girth[k].1;
            let reach = (ck / MAX_RATIO * own, ck * MAX_RATIO * own);
            return Err(self.refuse(self.girths[j].1, reach));
        }
        Ok(())
    }
}

/// `base` resized so every input of `file` matches `size` (mm), with the moves that did it; or
/// the measurement that can't be met.
fn solved(
    file: &FormFile,
    base: &Rings,
    size: &Measurements,
) -> Result<(Params, Rings), SizeError> {
    for m in &file.inputs {
        let [lo, hi] = file.ranges[m];
        match size.get(m) {
            Some(v) if v.is_finite() && (lo..=hi).contains(v) => {}
            _ => {
                return Err(SizeError {
                    measurement: m.clone(),
                    min_mm: lo,
                    max_mm: hi,
                });
            }
        }
    }
    let mut girths: Vec<(usize, &str)> = file
        .inputs
        .iter()
        .filter_map(|m| file.stations.get(m).map(|&i| (i, m.as_str())))
        .collect();
    girths.sort_unstable();
    let job = Job {
        file,
        base,
        size,
        lengths: file
            .inputs
            .iter()
            .map(String::as_str)
            .filter(|m| !file.stations.contains_key(*m))
            .collect(),
        girths,
    };
    let mut p = Params {
        above: 1.0,
        below: 1.0,
        shoulder: 1.0,
        girth: job.girths.iter().map(|&(i, _)| (i, 1.0)).collect(),
    };
    for _ in 0..PASSES {
        for &m in &job.lengths {
            let k = knob(m);
            let f = |x: f64| {
                let mut q = p.clone();
                *q.slot(k) = x;
                length(file, &apply(file, base, &q), m)
            };
            let (target, start) = (job.target(m), p.value(k));
            // A stretch that can't be found lies beyond the plausible range, towards the target.
            let x = solve(&f, target, start).unwrap_or_else(|| {
                if f(start) > target {
                    0.0
                } else {
                    f64::INFINITY
                }
            });
            // The stretch never leaves the plausible range; a length it can't reach that way is
            // refused below, with the range it can.
            *p.slot(k) = x.clamp(1.0 / MAX_RATIO, MAX_RATIO);
        }
        job.fit_girths(&mut p);
        let sized = apply(file, base, &p);
        if job
            .lengths
            .iter()
            .all(|m| (length(file, &sized, m) - job.target(m)).abs() <= 10.0 * SOLVE_TOL)
        {
            break;
        }
    }
    // Girths first: a girth that fights its neighbour also pulls the lengths off target, and the
    // lengths are then not the cause.
    job.check_girth_neighbours(&p)?;
    let sized = apply(file, base, &p);
    for &m in &job.lengths {
        let off = (length(file, &sized, m) - job.target(m)).abs();
        // A NaN length is no match either.
        if off.is_nan() || off > ACCEPT_TOL {
            return Err(job.refuse(m, job.reach(&p, m)));
        }
    }
    Ok((p, sized))
}

/// `base` resized so every input of `file` matches `size` (mm), or the measurement that can't be.
/// `file` must have passed `FormFile::check`.
pub fn resize(file: &FormFile, base: &Rings, size: &Measurements) -> Result<Rings, SizeError> {
    solved(file, base, size).map(|(_, rings)| rings)
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

    /// Every stretch and widening within `MAX_RATIO` of 1, and every girth station's scale within
    /// `MAX_RATIO` of the next one's.
    fn assert_plausible(p: &Params) {
        let ok = 1.0 / MAX_RATIO - 1e-9..=MAX_RATIO + 1e-9;
        for k in [p.above, p.below, p.shoulder] {
            assert!(ok.contains(&k), "a stretch of {k}");
        }
        for w in p.girth.windows(2) {
            let ratio = w[1].1 / w[0].1;
            assert!(ok.contains(&ratio), "scales {} and {}", w[0].1, w[1].1);
        }
    }

    #[test]
    fn any_targets_in_range_give_a_valid_shape_or_a_refusal() {
        let mut seed = 0x2545_f491_4f6c_dd1d_u64;
        let mut next = move || {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            (seed >> 11) as f64 / (1u64 << 53) as f64
        };
        for (f, r) in [base(), real(WOMEN), real(MEN)] {
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
                    Err(e) => {
                        assert!(f.inputs.contains(&e.measurement), "{e:?}");
                        // The range named is one the form can take, so it never holds the value
                        // that was asked for (but for the last digit of rounding).
                        let asked = t[&e.measurement];
                        assert!(e.min_mm <= e.max_mm, "{}: {e:?}", f.id);
                        assert!(
                            !(e.min_mm + 1e-6..=e.max_mm - 1e-6).contains(&asked),
                            "{}: {e}, yet {asked:.1} was asked",
                            f.id
                        );
                    }
                    Ok(out) => {
                        sized += 1;
                        assert!(out.y.windows(2).all(|w| w[1] > w[0]), "rings stay in order");
                        assert!(out.r.iter().flatten().all(|x| x.is_finite() && *x > 0.0));
                        assert_fits(&f, &out, &t);
                        assert_plausible(&solved(&f, &r, &t).expect("sized above").0);
                    }
                }
            }
            // About one draw in eight to ten is plausible; the rest are refused. Both paths ran.
            assert!(
                (1..200).contains(&sized),
                "{}: {sized} of 200 draws were sized",
                f.id
            );
        }
    }

    /// With every other input at the form's own value, ask for `asked`; the refusal's range must
    /// leave that value out, and a value just inside the range must be met.
    fn assert_refused_with_a_range_that_can_be_met(
        (f, r): &(FormFile, Rings),
        measurement: &str,
        asked: f64,
    ) -> SizeError {
        let own = measured(f, r);
        let mut t = own.clone();
        t.insert(measurement.into(), asked);
        let e = resize(f, r, &t).expect_err(&format!("{}: {measurement} {asked}", f.id));
        assert!(
            !(e.min_mm + 1e-6..=e.max_mm - 1e-6).contains(&asked) && e.min_mm < e.max_mm,
            "{}: {measurement} {asked} was refused with {e}",
            f.id
        );
        let inside = if asked > e.max_mm {
            e.max_mm - 1.0
        } else {
            e.min_mm + 1.0
        };
        t.insert(e.measurement.clone(), inside);
        let out = resize(f, r, &t)
            .unwrap_or_else(|again| panic!("{}: {e}, yet {inside:.0} is refused: {again}", f.id));
        assert_fits(f, &out, &t);
        e
    }

    #[test]
    fn a_girth_that_fights_its_neighbour_is_named_and_not_the_length_it_drags_along() {
        let women = real(WOMEN);
        // A bust of 1400 lengthens the shoulder seam too, but the bust is what cannot be.
        let e = assert_refused_with_a_range_that_can_be_met(&women, "bust", 1400.0);
        assert_eq!(e.measurement, "bust");
        assert!(e.max_mm < 1250.0, "{e}");
        let e = assert_refused_with_a_range_that_can_be_met(&women, "hip", 1500.0);
        assert_eq!(e.measurement, "hip");
        assert!(e.max_mm < 1250.0, "{e}");
        // The same at the far end of the men's chart.
        let men = real(MEN);
        let e = assert_refused_with_a_range_that_can_be_met(&men, "chest", 1500.0);
        assert_eq!(e.measurement, "chest");
        assert!(e.max_mm < 1400.0, "{e}");
    }

    #[test]
    fn a_length_out_of_reach_is_refused_with_the_range_it_can_take() {
        let women = real(WOMEN);
        for (m, asked) in [
            ("shoulder_length", 190.0),
            ("shoulder_length", 90.0),
            ("waist_to_hip", 280.0),
            ("waist_to_hip", 150.0),
        ] {
            let e = assert_refused_with_a_range_that_can_be_met(&women, m, asked);
            assert_eq!(e.measurement, m);
        }
        let men = real(MEN);
        let e = assert_refused_with_a_range_that_can_be_met(&men, "waist_to_hip", 150.0);
        assert_eq!(e.measurement, "waist_to_hip");
        let e = assert_refused_with_a_range_that_can_be_met(&men, "shoulder_length", 110.0);
        assert_eq!(e.measurement, "shoulder_length");
    }

    #[test]
    fn every_end_of_every_range_on_the_shipped_forms_is_met_or_refused_honestly() {
        for form in [real(WOMEN), real(MEN)] {
            let own = measured(&form.0, &form.1);
            for m in &form.0.inputs {
                for asked in form.0.ranges[m] {
                    let mut t = own.clone();
                    t.insert(m.clone(), asked);
                    match resize(&form.0, &form.1, &t) {
                        Ok(out) => assert_fits(&form.0, &out, &t),
                        Err(_) => {
                            let _ = assert_refused_with_a_range_that_can_be_met(&form, m, asked);
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn the_mens_form_refuses_a_chest_and_waist_that_fight_with_a_range_that_leaves_them_out() {
        let (f, r) = real(MEN);
        let mut t = measured(&f, &r);
        t.insert("chest".into(), 1500.0);
        t.insert("waist".into(), 600.0);
        let e = resize(&f, &r, &t).unwrap_err();
        assert!(["chest", "waist"].contains(&e.measurement.as_str()), "{e}");
        let asked = t[&e.measurement];
        assert!(
            e.min_mm < e.max_mm && !(e.min_mm..=e.max_mm).contains(&asked),
            "{e} but {asked} was asked"
        );
    }

    #[test]
    fn a_far_off_chest_on_the_mens_form_names_a_range_without_it() {
        let (f, r) = real(MEN);
        for asked in [750.0, 1500.0] {
            let mut t = measured(&f, &r);
            t.insert("chest".into(), asked);
            let e = resize(&f, &r, &t).unwrap_err();
            assert_eq!(e.measurement, "chest");
            assert!(
                e.min_mm < e.max_mm && !(e.min_mm..=e.max_mm).contains(&asked),
                "{e} but {asked} was asked"
            );
        }
    }

    #[test]
    fn the_girth_scale_is_flat_into_both_ends_and_stays_between_its_data() {
        let scale = pchip(vec![10.0, 30.0, 45.0, 60.0], vec![1.0, 1.2, 1.1, 1.3]);
        for (x, y) in [(10.0, 1.0), (30.0, 1.2), (45.0, 1.1), (60.0, 1.3)] {
            assert!((scale(x) - y).abs() < 1e-12, "at {x}");
        }
        // Flat beyond the ends, and no kink there: the slope just inside is 0 too.
        let slope = |a: f64, b: f64| (scale(b) - scale(a)) / (b - a);
        assert_eq!(slope(0.0, 9.0), 0.0);
        assert_eq!(slope(61.0, 70.0), 0.0);
        assert!(slope(10.0, 10.0 + 1e-4).abs() < 1e-4);
        assert!(slope(60.0 - 1e-4, 60.0).abs() < 1e-4);
        // Rising data never dips or overshoots.
        let rising = pchip(vec![0.0, 10.0, 20.0, 30.0], vec![1.0, 1.1, 1.5, 1.6]);
        let mut last = rising(0.0);
        for k in 1..=300 {
            let y = rising(f64::from(k) / 10.0);
            assert!(y >= last - 1e-12 && (1.0..=1.6).contains(&y), "at {k}: {y}");
            last = y;
        }
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
        let (waist, hip) = (f.stations["waist"], f.stations["hip"]);
        assert!(a.y[hip] < a.y[waist], "the hip stays below the waist");
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
