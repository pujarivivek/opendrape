//! Helpers the form tests share: the shipped forms, the sizes they are tried at, and mesh
//! geometry that does not depend on the code under test.

use super::{Form, FormFile, Measurements, Quality, Rings};
use crate::BodyMesh;
use glam::{DVec2, DVec3};

pub const WOMEN: &str = include_str!("../../../../assets/forms/women-torso.form.json");
pub const MEN: &str = include_str!("../../../../assets/forms/men-torso.form.json");

pub const WOMEN_EXTREME: [(&str, f64); 8] = [
    ("bust", 1050.0),
    ("under_bust", 900.0),
    ("waist", 820.0),
    ("hip", 1080.0),
    ("neck", 390.0),
    ("shoulder_length", 140.0),
    ("back_waist_length", 440.0),
    ("waist_to_hip", 210.0),
];
pub const MEN_EXTREME: [(&str, f64); 7] = [
    ("chest", 1120.0),
    ("waist", 940.0),
    ("hip", 1120.0),
    ("neck", 420.0),
    ("shoulder_length", 165.0),
    ("back_waist_length", 500.0),
    ("waist_to_hip", 220.0),
];

/// A form at one size: the unsized rings and the sized, re-cut ones, as `Form::build` makes them.
pub struct Sized {
    pub file: FormFile,
    pub size: Measurements,
    pub base: Rings,
    pub rings: Rings,
}

impl Sized {
    pub fn torso(&self) -> BodyMesh {
        self.rings.mesh()
    }
}

/// `file` at its own size with `changes` (mm) applied, or `None` if the resize refuses it.
pub fn try_sized(file: &FormFile, changes: &[(&str, f64)]) -> Option<Sized> {
    let form = Form::new(file.clone()).unwrap();
    let mut size = form.base_measurements(Quality::Standard);
    for &(m, mm) in changes {
        size.insert(m.to_string(), mm);
    }
    let (base, rings) = form.rings(&size, Quality::Standard).ok()?;
    Some(Sized {
        file: file.clone(),
        size,
        base,
        rings,
    })
}

pub fn sized(file: &FormFile, changes: &[(&str, f64)]) -> Sized {
    try_sized(file, changes).expect("the resize takes this size")
}

/// The smallest and largest neck (mm) the resize takes, the form's other inputs at its own: the
/// ends of the neck's range, or the nearest whole millimetre inside them that is accepted.
pub fn neck_extremes(file: &FormFile) -> (f64, f64) {
    let [lo, hi] = file.ranges["neck"];
    let steps = (hi - lo) as usize;
    let taken = |mm: &f64| try_sized(file, &[("neck", *mm)]).is_some();
    let smallest = (0..=steps).map(|k| lo + k as f64).find(taken);
    let largest = (0..=steps).map(|k| hi - k as f64).find(taken);
    (smallest.unwrap(), largest.unwrap())
}

/// A real form at its own size, at an extreme target, and with the smallest and largest neck the
/// resize takes.
pub fn real_cases(json: &str, extreme: &[(&str, f64)]) -> Vec<(String, Sized)> {
    let file = FormFile::from_json(json).unwrap();
    let (small, large) = neck_extremes(&file);
    vec![
        ("own size".to_string(), sized(&file, &[])),
        ("extreme".to_string(), sized(&file, extreme)),
        (format!("neck {small} mm"), sized(&file, &[("neck", small)])),
        (format!("neck {large} mm"), sized(&file, &[("neck", large)])),
    ]
}

/// Both shipped forms with their extreme targets.
pub fn both_real_forms() -> [(&'static str, &'static [(&'static str, f64)]); 2] {
    [(WOMEN, &WOMEN_EXTREME), (MEN, &MEN_EXTREME)]
}

pub fn signed_volume(m: &BodyMesh) -> f64 {
    m.triangles
        .iter()
        .map(|t| {
            let [a, b, c] = t.map(|i| m.positions[i as usize].as_dvec3());
            a.dot(b.cross(c)) / 6.0
        })
        .sum()
}

/// Every edge is used by two triangles, once in each direction: the mesh is closed and its
/// triangles agree on which way is out.
pub fn consistently_oriented(m: &BodyMesh) -> bool {
    let mut directed = std::collections::HashMap::<(u32, u32), u32>::new();
    for t in &m.triangles {
        for k in 0..3 {
            *directed.entry((t[k], t[(k + 1) % 3])).or_default() += 1;
        }
    }
    directed
        .iter()
        .all(|(&(a, b), &n)| n == 1 && directed.get(&(b, a)) == Some(&1))
}

pub fn lowest(m: &BodyMesh) -> f32 {
    m.positions.iter().map(|p| p.y).fold(f32::MAX, f32::min)
}

pub fn highest(m: &BodyMesh) -> f32 {
    m.positions.iter().map(|p| p.y).fold(f32::MIN, f32::max)
}

/// The point of triangle (a, b, c) nearest to `p`.
fn nearest_on_triangle(p: DVec3, a: DVec3, b: DVec3, c: DVec3) -> DVec3 {
    let (ab, ac, ap) = (b - a, c - a, p - a);
    let (d1, d2) = (ab.dot(ap), ac.dot(ap));
    if d1 <= 0.0 && d2 <= 0.0 {
        return a;
    }
    let bp = p - b;
    let (d3, d4) = (ab.dot(bp), ac.dot(bp));
    if d3 >= 0.0 && d4 <= d3 {
        return b;
    }
    let vc = d1 * d4 - d3 * d2;
    if vc <= 0.0 && d1 >= 0.0 && d3 <= 0.0 {
        return a + ab * (d1 / (d1 - d3));
    }
    let cp = p - c;
    let (d5, d6) = (ab.dot(cp), ac.dot(cp));
    if d6 >= 0.0 && d5 <= d6 {
        return c;
    }
    let vb = d5 * d2 - d1 * d6;
    if vb <= 0.0 && d2 >= 0.0 && d6 <= 0.0 {
        return a + ac * (d2 / (d2 - d6));
    }
    let va = d3 * d6 - d5 * d4;
    if va <= 0.0 && d4 - d3 >= 0.0 && d5 - d6 >= 0.0 {
        return b + (c - b) * ((d4 - d3) / ((d4 - d3) + (d5 - d6)));
    }
    let denom = 1.0 / (va + vb + vc);
    a + ab * (vb * denom) + ac * (vc * denom)
}

/// Distance from `p` to the nearest point of the mesh's triangles.
pub fn distance_to_mesh(p: DVec3, m: &BodyMesh) -> f64 {
    m.triangles
        .iter()
        .map(|t| {
            let [a, b, c] = t.map(|i| m.positions[i as usize].as_dvec3());
            nearest_on_triangle(p, a, b, c).distance(p)
        })
        .fold(f64::MAX, f64::min)
}

/// Whether `p` is inside the polygon (crossing-number test).
pub fn inside(poly: &[DVec2], p: DVec2) -> bool {
    let mut hit = false;
    for i in 0..poly.len() {
        let (a, b) = (poly[i], poly[(i + 1) % poly.len()]);
        if (a.y > p.y) != (b.y > p.y) && p.x < a.x + (p.y - a.y) / (b.y - a.y) * (b.x - a.x) {
            hit = !hit;
        }
    }
    hit
}

/// Distance from `p` to the polygon's edges.
pub fn distance_to_edges(poly: &[DVec2], p: DVec2) -> f64 {
    (0..poly.len())
        .map(|i| {
            let (a, b) = (poly[i], poly[(i + 1) % poly.len()]);
            let t = ((p - a).dot(b - a) / (b - a).length_squared()).clamp(0.0, 1.0);
            (a + (b - a) * t).distance(p)
        })
        .fold(f64::MAX, f64::min)
}
