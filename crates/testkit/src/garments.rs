use glam::{DVec2, DVec3};
use opendrape_body::BodyMesh;
use opendrape_body::form::BuiltForm;
use opendrape_sim::{BodyCollider, ClothBuilder, CompoundCollider, Panel, Params, Solid, Solver};
use std::sync::OnceLock;

/// Target fabric edge length (m) and fabric weight (kg/m², a light cotton).
pub const EDGE: f64 = 0.012;
pub const DENSITY: f64 = 0.15;
pub const SKIRT_PARTICLES: usize = 4794;
pub const BODICE_PARTICLES: usize = 1440;

pub fn body() -> &'static BodyMesh {
    static BODY: OnceLock<BodyMesh> = OnceLock::new();
    BODY.get_or_init(BodyMesh::female_average)
}

pub fn collider() -> &'static BodyCollider {
    static COLLIDER: OnceLock<BodyCollider> = OnceLock::new();
    COLLIDER.get_or_init(|| {
        BodyCollider::new(&body().positions, &body().triangles).expect("bundled body is closed")
    })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Garment {
    Skirt,
    BodiceProxy,
}

impl Garment {
    pub const ALL: [Garment; 2] = [Garment::Skirt, Garment::BodiceProxy];
}

pub struct Scene {
    pub garment: Garment,
    pub solver: Solver,
    contact: Contact,
}

/// What the scene's cloth collides with.
enum Contact {
    /// The bundled MakeHuman body (`collider()`).
    Bundled,
    Form(Box<CompoundCollider>),
}

impl Contact {
    fn solid(&self) -> &dyn Solid {
        match self {
            Contact::Bundled => collider(),
            Contact::Form(c) => c.as_ref(),
        }
    }
}

impl Scene {
    pub fn new(garment: Garment) -> Self {
        let solver = match garment {
            Garment::Skirt => skirt(),
            Garment::BodiceProxy => bodice_proxy(),
        };
        Self {
            garment,
            solver,
            contact: Contact::Bundled,
        }
    }

    /// The demo garment on a dress form, colliding with its torso and the floor.
    pub fn new_on(garment: Garment, form: &BuiltForm) -> Self {
        let collider = crate::forms::collider(form);
        let solver = match garment {
            Garment::Skirt => crate::forms::skirt(form, &collider),
            Garment::BodiceProxy => crate::forms::bodice_proxy(form, &collider),
        };
        Self {
            garment,
            solver,
            contact: Contact::Form(Box::new(collider)),
        }
    }

    /// What this scene's cloth collides with, for measuring the drape.
    pub fn collider(&self) -> &dyn Solid {
        self.contact.solid()
    }

    pub fn step(&mut self) {
        // Field by field: `self.collider()` would borrow the solver too.
        self.solver.step(Some(self.contact.solid()));
    }
}

/// z of the torso's centre line, measured around the hips with the arms left out.
fn torso_axis_z(body: &BodyMesh) -> f64 {
    let (lo, hi) = body
        .positions
        .iter()
        .filter(|p| p.y > 0.7 && p.y < 0.85 && p.x.abs() < 0.22)
        .fold((f32::MAX, f32::MIN), |(lo, hi), p| {
            (lo.min(p.z), hi.max(p.z))
        });
    f64::from((lo + hi) / 2.0)
}

/// `rows`×`cols` quad grid split into triangles (alternating diagonals). `wrap` joins the
/// last column to the first (a tube). `flat` gives pattern coordinates for rest lengths.
pub(crate) fn grid_panel(
    rows: usize,
    cols: usize,
    wrap: bool,
    flat: Option<&dyn Fn(usize, usize) -> DVec2>,
    place: &dyn Fn(usize, usize) -> DVec3,
) -> Panel {
    let ncols = if wrap { cols } else { cols + 1 };
    let id = |i: usize, j: usize| (i * ncols + j % ncols) as u32;
    let mut panel = Panel::default();
    let mut flat_pts = vec![];
    for i in 0..=rows {
        for j in 0..ncols {
            panel.positions.push(place(i, j));
            if let Some(f) = flat {
                flat_pts.push(f(i, j));
            }
        }
    }
    for i in 0..rows {
        for j in 0..cols {
            let (a, b, c, d) = (id(i, j), id(i, j + 1), id(i + 1, j + 1), id(i + 1, j));
            if (i + j) % 2 == 0 {
                panel.triangles.extend([[a, b, c], [a, c, d]]);
            } else {
                panel.triangles.extend([[a, b, d], [b, c, d]]);
            }
        }
    }
    if flat.is_some() {
        panel.flat = Some(flat_pts);
    }
    panel
}

pub const SKIRT_WAIST_Y: f64 = 1.03;

/// A-line skirt: two trapezoid panels (35.5 cm waist, 60 cm hem, 55 cm long each), wrapped
/// around the body on a cylinder clear of the hips, side seams stitched.
fn skirt() -> Solver {
    let body = body();
    let zc = torso_axis_z(body);
    let (waist_w, hem_w, length) = (0.355, 0.60, 0.55);
    let r = body
        .positions
        .iter()
        .filter(|p| {
            let y = f64::from(p.y);
            y < SKIRT_WAIST_Y + 0.02 && y > SKIRT_WAIST_Y - length && p.x.abs() < 0.2
        })
        .map(|p| (f64::from(p.x).powi(2) + (f64::from(p.z) - zc).powi(2)).sqrt())
        .fold(0.0, f64::max)
        + 0.03;
    skirt_panels(SKIRT_WAIST_Y, zc, r, waist_w, hem_w, length)
}

/// Two trapezoid panels (`waist_w` at the waist, `hem_w` at the hem, `length` long, each)
/// wrapped on a cylinder of radius `r` around the vertical axis through z = `zc`, side seams
/// stitched.
pub(crate) fn skirt_panels(
    waist_y: f64,
    zc: f64,
    r: f64,
    waist_w: f64,
    hem_w: f64,
    length: f64,
) -> Solver {
    let (rows, cols) = skirt_grid(length, hem_w);
    let flat = move |i: usize, j: usize| {
        let t = i as f64 / rows as f64;
        let half = (waist_w + (hem_w - waist_w) * t) / 2.0;
        DVec2::new(-half + 2.0 * half * j as f64 / cols as f64, -t * length)
    };
    skirt_cut(
        rows,
        cols,
        &flat,
        false,
        &|front, i, j| {
            let p = flat(i, j);
            let phi = if front {
                p.x / r
            } else {
                std::f64::consts::PI - p.x / r
            };
            DVec3::new(r * phi.sin(), waist_y + p.y, zc + r * phi.cos())
        },
        Params::default(),
    )
}

/// Rows and columns of a skirt panel `length` long and `hem_w` wide at the hem.
pub(crate) fn skirt_grid(length: f64, hem_w: f64) -> (usize, usize) {
    (
        (length / EDGE).round() as usize,
        (hem_w / EDGE).round() as usize,
    )
}

/// Two panels on a `rows`×`cols` grid, side seams stitched. `flat(i, j)` is grid point (`i`, `j`)
/// in the flat panel (it sets the rest lengths); `place(front, i, j)` puts it in space on the front
/// or the back panel. `pin_rim` holds the top row where `place` puts it.
pub(crate) fn skirt_cut(
    rows: usize,
    cols: usize,
    flat: &dyn Fn(usize, usize) -> DVec2,
    pin_rim: bool,
    place: &dyn Fn(bool, usize, usize) -> DVec3,
    params: Params,
) -> Solver {
    let mut builder = ClothBuilder::new(DENSITY);
    let mut panels = vec![];
    for front in [true, false] {
        let place_here = |i: usize, j: usize| place(front, i, j);
        let mut panel = grid_panel(rows, cols, false, Some(flat), &place_here);
        if !front {
            // The back panel is placed mirrored; flip its winding so both panels face outward
            // and the welded seams get consistent normals.
            for t in &mut panel.triangles {
                t.swap(1, 2);
            }
        }
        let id = builder.add_panel(&panel, 1.0);
        if pin_rim {
            for j in 0..=cols as u32 {
                builder.pin((id, j));
            }
        }
        panels.push(id);
    }
    for i in 0..=rows {
        for j in [0, cols] {
            let k = (i * (cols + 1) + j) as u32;
            builder.stitch((panels[0], k), (panels[1], k));
        }
    }
    Solver::new(builder.build(), params)
}

/// Close-fit collision test: a tube shaped to the torso (1 cm ease) from waist to below the
/// armpits, rest lengths 3% short so it hugs the body, top ring held as if by shoulder straps.
fn bodice_proxy() -> Solver {
    tube(0.99, 1.19, torso_axis_z(body()), collider())
}

/// A tube from `y0` up to `y1` around the vertical axis through z = `zc`. Each point sits 1 cm
/// outside where a ray from the axis leaves `body`; rest lengths are 3% short; the top ring is
/// pinned.
pub(crate) fn tube(y0: f64, y1: f64, zc: f64, body: &BodyCollider) -> Solver {
    let cols = 80usize;
    let rows = ((y1 - y0) / EDGE).round() as usize;
    let place = |i: usize, j: usize| {
        let y = y1 - (y1 - y0) * i as f64 / rows as f64;
        let a = std::f64::consts::TAU * j as f64 / cols as f64;
        let dir = DVec3::new(a.sin(), 0.0, a.cos());
        let origin = DVec3::new(0.0, y, zc);
        origin + dir * (body.ray_exit(origin, dir, 1.0).unwrap_or(0.12) + 0.01)
    };
    let mut builder = ClothBuilder::new(DENSITY);
    let tube = builder.add_panel(&grid_panel(rows, cols, true, None, &place), 0.97);
    for j in 0..cols as u32 {
        builder.pin((tube, j));
    }
    Solver::new(
        builder.build(),
        Params {
            gravity_delay: 0.0,
            weld_timeout: None,
            ..Params::default()
        },
    )
}
