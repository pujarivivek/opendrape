//! The demo garments, and a floor check, on dress forms.

use crate::garments::{DENSITY, EDGE, grid_panel, skirt_cut, skirt_grid, tube};
use glam::{DVec2, DVec3};
use opendrape_body::form::BuiltForm;
use opendrape_sim::{BodyCollider, ClothBuilder, CompoundCollider, Params, Solver};
use std::f64::consts::{FRAC_PI_2, TAU};

/// The torso, then the floor at y = 0.
pub fn collider(form: &BuiltForm) -> CompoundCollider {
    let torso = BodyCollider::new(&form.torso.positions, &form.torso.triangles)
        .expect("a form's torso is closed");
    CompoundCollider::new(vec![torso], Some(0.0))
}

/// z of the form's centre line at the waist.
fn axis_z(form: &BuiltForm) -> f64 {
    (form.landmarks["front_waist"].z + form.landmarks["back_waist"].z) / 2.0
}

/// Farthest torso point from the axis between `y0` and `y1`.
fn reach(form: &BuiltForm, zc: f64, y0: f64, y1: f64) -> f64 {
    form.torso
        .positions
        .iter()
        .filter(|p| (y0..y1).contains(&f64::from(p.y)))
        .map(|p| (f64::from(p.x).powi(2) + (f64::from(p.z) - zc).powi(2)).sqrt())
        .fold(0.0, f64::max)
}

/// How much bigger than the form's waist girth the skirt is at the waist (m).
const WAIST_EASE: f64 = 0.03;
/// How much bigger than the form's girth the skirt is at the high hip and at the hip (m).
const HIP_EASE: f64 = 0.06;

/// The A-line demo skirt cut for the form and held up by its waist. Each panel is half the
/// skirt's circumference wide: the waist girth plus `WAIST_EASE` at the waist, the girth plus
/// `HIP_EASE` at the high hip and at the hip (the form is too steep under the waist for one
/// straight line from waist to hip to clear the high hip), and below the hip the same straight
/// line on down to the 55 cm hem. The panels are laid round the form like the tube, standing
/// off it by what the ease makes (3 mm at least), so the side seams start closed and the top
/// ring, which is pinned, sits at the waist. Rest lengths come from the cone the skirt's
/// circumferences make, cut open: with straight rows the flat panel doesn't fit the form where
/// it flares hard just under the waist, and its seams stretch there.
pub fn skirt(form: &BuiltForm, collider: &CompoundCollider) -> Solver {
    let (waist_y, zc, length) = (form.stations["waist"], axis_z(form), 0.55);
    let (high_hip, hip) = (
        waist_y - form.stations["high_hip"],
        waist_y - form.stations["hip"],
    );
    let girth = |name: &str| form.measured[name] / 1000.0;
    let (c0, c1, c2) = (
        girth("waist") + WAIST_EASE,
        girth("high_hip") + HIP_EASE,
        girth("hip") + HIP_EASE,
    );
    // The skirt's circumference at depth `d` below the waist.
    let circumference = |d: f64| {
        if d <= high_hip {
            c0 + (c1 - c0) * d / high_hip
        } else {
            c1 + (c2 - c1) * (d - high_hip) / (hip - high_hip)
        }
    };
    let (rows, cols) = skirt_grid(length, circumference(length) / 2.0);
    let rings: Vec<[Vec<DVec3>; 2]> = (0..=rows)
        .map(|i| {
            let d = length * i as f64 / rows as f64;
            ring(&collider.parts()[0], waist_y - d, zc, circumference(d))
        })
        .collect();
    // The flat panel: the cone the skirt's radius makes, cut open. Row i is an arc of radius
    // `apex + slant[i]` whose length is half the row's circumference, so the rest lengths agree
    // with the form-hugging 3D layout even where the skirt flares hard under the waist.
    let radius = |c: f64| c / TAU;
    let mut slant = vec![0.0];
    for i in 1..=rows {
        let (d0, d1) = (
            length * (i - 1) as f64 / rows as f64,
            length * i as f64 / rows as f64,
        );
        let dr = radius(circumference(d1)) - radius(circumference(d0));
        slant.push(slant[i - 1] + (d1 - d0).hypot(dr));
    }
    let dr0 = radius(circumference(length / rows as f64)) - radius(circumference(0.0));
    let apex = radius(circumference(0.0)) * slant[1] / dr0;
    let flat = |i: usize, j: usize| {
        let rho = apex + slant[i];
        let a = (circumference(length * i as f64 / rows as f64) / 2.0 / rho)
            * (j as f64 / cols as f64 - 0.5);
        DVec2::new(rho * a.sin(), -rho * a.cos())
    };
    skirt_cut(
        rows,
        cols,
        &flat,
        true,
        &|front, i, j| along(&rings[i][usize::from(!front)], j as f64 / cols as f64),
        Params::default(),
    )
}

/// Where a skirt of circumference `circ` goes round the torso at height `y`: the front half
/// and the back half of the ring, each from the right-hand side seam to the left-hand one. The
/// ring is the torso's outline offset by what makes up `circ` (3 mm at least), or a circle
/// where the torso doesn't reach.
fn ring(torso: &BodyCollider, y: f64, zc: f64, circ: f64) -> [Vec<DVec3>; 2] {
    const N: usize = 192;
    let origin = DVec3::new(0.0, y, zc);
    let dir = |k: usize| {
        let a = -FRAC_PI_2 + TAU * (k % N) as f64 / N as f64;
        DVec3::new(a.sin(), 0.0, a.cos())
    };
    let exits: Option<Vec<f64>> = (0..N)
        .map(|k| torso.ray_exit(origin, dir(k), 1.0))
        .collect();
    let radii = match exits {
        Some(exits) => {
            let outline: Vec<DVec3> = (0..N).map(|k| origin + dir(k) * exits[k]).collect();
            let girth: f64 = (0..N)
                .map(|k| outline[k].distance(outline[(k + 1) % N]))
                .sum();
            let offset = ((circ - girth) / TAU).max(0.003);
            exits.iter().map(|e| e + offset).collect()
        }
        None => vec![circ / TAU; N],
    };
    let at = |k: usize| origin + dir(k) * radii[k % N];
    [
        (0..=N / 2).map(at).collect(),
        (N / 2..=N).rev().map(at).collect(),
    ]
}

/// The point a fraction `f` of the way along `line`, by length.
fn along(line: &[DVec3], f: f64) -> DVec3 {
    let lengths: Vec<f64> = line.windows(2).map(|w| w[0].distance(w[1])).collect();
    let mut left = f * lengths.iter().sum::<f64>();
    for (w, len) in line.windows(2).zip(&lengths) {
        if left <= *len {
            return w[0].lerp(w[1], left / len);
        }
        left -= len;
    }
    line[line.len() - 1]
}

/// The close-fit tube from 4 cm below the waist to just under the armholes.
pub fn bodice_proxy(form: &BuiltForm, collider: &CompoundCollider) -> Solver {
    let waist = form.stations["waist"];
    let chest = form
        .stations
        .get("bust")
        .or(form.stations.get("chest"))
        .copied()
        .expect("a torso has a bust or chest station");
    let top = (chest + 0.04).min(form.landmarks["armhole_bottom"].y - 0.02);
    tube(waist - 0.04, top, axis_z(form), &collider.parts()[0])
}

/// A long tube pinned just above the waist, flaring out so it is longer than the drop to the
/// floor: under gravity its hem reaches the floor and pools there.
pub fn long_hem(form: &BuiltForm) -> Solver {
    let (waist, zc) = (form.stations["waist"], axis_z(form));
    let r_top = reach(form, zc, waist - 0.25, waist + 0.02) + 0.03;
    let (r_bottom, cols) = (r_top + 0.35, 120usize);
    let (y0, y1) = (0.03, waist + 0.01);
    let rows = ((y1 - y0).hypot(r_bottom - r_top) / EDGE).round() as usize;
    let place = |i: usize, j: usize| {
        let t = i as f64 / rows as f64;
        let a = std::f64::consts::TAU * j as f64 / cols as f64;
        let r = r_top + (r_bottom - r_top) * t;
        DVec3::new(r * a.sin(), y1 - (y1 - y0) * t, zc + r * a.cos())
    };
    let mut builder = ClothBuilder::new(DENSITY);
    let tube = builder.add_panel(&grid_panel(rows, cols, true, None, &place), 1.0);
    for j in 0..cols as u32 {
        builder.pin((tube, j));
    }
    Solver::new(builder.build(), Params::default())
}
