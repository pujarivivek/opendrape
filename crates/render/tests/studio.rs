//! The studio renderer, drawn headless (a real GPU, or WARP / lavapipe / llvmpipe in CI) and
//! judged by numbers: colours a student can trust, a seamless studio, shadows, soft darkening
//! in folds, and a view that stops drawing once still.

use glam::Vec3;
use opendrape_render::colour::{delta_e2000, srgb8_to_linear};
use opendrape_render::studio::quality::Quality;
use opendrape_render::studio::{Material, Overrides, StudioMesh, StudioRenderer};
use opendrape_render::{HeadlessGpu, OrbitCamera, RenderTarget, headless_device, read_back};

fn gpu() -> HeadlessGpu {
    headless_device().expect("a GPU or software adapter")
}

/// A `size` m square in the plane z = `z`, centred on (0, `y`, `z`), facing +Z.
fn card(y: f32, z: f32, size: f32) -> (Vec<Vec3>, Vec<[u32; 3]>) {
    let h = size / 2.0;
    let p = vec![
        Vec3::new(-h, y - h, z),
        Vec3::new(h, y - h, z),
        Vec3::new(h, y + h, z),
        Vec3::new(-h, y + h, z),
    ];
    (p, vec![[0, 1, 2], [0, 2, 3]])
}

/// Looking straight at (0, 1, 0) from the front.
fn front_camera(distance: f32) -> OrbitCamera {
    OrbitCamera {
        target: Vec3::new(0.0, 1.0, 0.0),
        yaw: 0.0,
        pitch: 0.0,
        distance,
        fov_y: 35f32.to_radians(),
    }
}

/// Everything but the light and colour off: no soft darkening, no shadows.
fn plain() -> Overrides {
    Overrides {
        ao: Some(false),
        key_shadows: Some(false),
        contact: Some(false),
        force_ldr: false,
    }
}

fn mesh(
    r: &mut StudioRenderer,
    g: &HeadlessGpu,
    (p, t): (Vec<Vec3>, Vec<[u32; 3]>),
    colour: [f32; 3],
    material: Material,
) -> StudioMesh {
    r.create_mesh(&g.device, &g.queue, &p, &t, colour, material)
}

/// Renders until the view is still and finished (at most 100 frames), and reads it back.
fn render_still(
    r: &mut StudioRenderer,
    g: &HeadlessGpu,
    target: &RenderTarget,
    camera: &OrbitCamera,
    meshes: &[&StudioMesh],
) -> image::RgbaImage {
    for _ in 0..100 {
        if r.render(&g.device, &g.queue, target, camera, meshes)
            .still_done
        {
            break;
        }
    }
    read_back(&g.device, &g.queue, target)
}

/// The mean colour of the `n`×`n` pixels in the middle of `img`.
fn centre_mean(img: &image::RgbaImage, n: u32) -> [u8; 3] {
    let (cx, cy) = (img.width() / 2 - n / 2, img.height() / 2 - n / 2);
    let mut sum = [0u32; 3];
    for y in cy..cy + n {
        for x in cx..cx + n {
            let p = img.get_pixel(x, y);
            for c in 0..3 {
                sum[c] += u32::from(p[c]);
            }
        }
    }
    sum.map(|s| ((s as f32) / (n * n) as f32).round() as u8)
}

fn luminance(p: &image::Rgba<u8>) -> f32 {
    0.2126 * f32::from(p[0]) + 0.7152 * f32::from(p[1]) + 0.0722 * f32::from(p[2])
}

/// The colours the accuracy check uses, as a colour picker gives them (sRGB): 18 % grey,
/// cotton red, grass green, navy, skin beige and mustard.
const SWATCHES: [[u8; 3]; 6] = [
    [118, 118, 118],
    [180, 40, 45],
    [60, 140, 60],
    [30, 40, 90],
    [225, 190, 160],
    [205, 160, 40],
];

/// A swatch, the colour it showed as, and the difference (CIEDE2000).
struct Shown {
    want: [u8; 3],
    got: [u8; 3],
    delta_e: f64,
}

fn card_colour_error(force_ldr: bool) -> Vec<Shown> {
    let g = gpu();
    let mut r = StudioRenderer::new(&g.device, &g.adapter, Quality::Medium);
    r.set_overrides(Overrides {
        force_ldr,
        ..plain()
    });
    let target = RenderTarget::new(&g.device, 256, 256);
    SWATCHES
        .iter()
        .map(|&swatch| {
            let m = mesh(
                &mut r,
                &g,
                card(1.0, 0.0, 0.6),
                srgb8_to_linear(swatch),
                Material::Cloth,
            );
            let img = render_still(&mut r, &g, &target, &front_camera(1.2), &[&m]);
            let got = centre_mean(&img, 16);
            Shown {
                want: swatch,
                got,
                delta_e: delta_e2000(swatch, got),
            }
        })
        .collect()
}

/// A matte fabric facing the camera shows its own colour, within a difference that is hard
/// to see (CIEDE2000 ≤ 3).
#[test]
fn a_matte_fabric_card_shows_its_true_colour() {
    for Shown { want, got, delta_e } in card_colour_error(false) {
        eprintln!("{want:?} shows as {got:?}: ΔE2000 {delta_e:.2}");
        assert!(
            delta_e <= 3.0,
            "{want:?} shows as {got:?}: ΔE2000 {delta_e:.2}"
        );
    }
}

/// Without half-float render targets (some old OpenGL drivers) colours stay close.
#[test]
fn the_ldr_path_is_close_too() {
    for Shown { want, got, delta_e } in card_colour_error(true) {
        assert!(
            delta_e <= 5.0,
            "{want:?} shows as {got:?}: ΔE2000 {delta_e:.2}"
        );
    }
}

#[test]
fn renders_at_odd_sizes_without_errors() {
    let g = gpu();
    for quality in Quality::ALL {
        for force_ldr in [false, true] {
            let mut r = StudioRenderer::new(&g.device, &g.adapter, quality);
            r.set_overrides(Overrides {
                force_ldr,
                ..Overrides::default()
            });
            let m = mesh(
                &mut r,
                &g,
                card(1.0, 0.0, 0.6),
                [0.3, 0.2, 0.6],
                Material::Cloth,
            );
            for (w, h) in [(1, 1), (37, 19), (1000, 700)] {
                let scope = g.device.push_error_scope(wgpu::ErrorFilter::Validation);
                let target = RenderTarget::new(&g.device, w, h);
                render_still(&mut r, &g, &target, &front_camera(1.5), &[&m]);
                let error = pollster::block_on(scope.pop());
                assert!(
                    error.is_none(),
                    "{quality:?} ldr {force_ldr} {w}×{h}: {error:?}"
                );
            }
        }
    }
}

/// The floor fades into the backdrop with no visible edge: down the middle of the image,
/// from the floor up into the backdrop, no row differs from the next by more than 6/255.
#[test]
fn the_backdrop_has_no_seam_at_the_floors_edge() {
    let g = gpu();
    let mut r = StudioRenderer::new(&g.device, &g.adapter, Quality::Medium);
    r.set_overrides(plain());
    let target = RenderTarget::new(&g.device, 200, 300);
    let camera = OrbitCamera {
        target: Vec3::new(0.0, 1.0, 0.0),
        yaw: 0.3,
        pitch: 0.05,
        distance: 6.0,
        fov_y: 50f32.to_radians(),
    };
    let img = render_still(&mut r, &g, &target, &camera, &[]);
    img.save(format!(
        "{}/studio_backdrop.png",
        env!("CARGO_TARGET_TMPDIR")
    ))
    .ok();
    let column: Vec<f32> = (0..img.height())
        .map(|y| luminance(img.get_pixel(img.width() / 2, y)))
        .collect();
    for (y, pair) in column.windows(2).enumerate() {
        assert!(
            (pair[0] - pair[1]).abs() <= 6.0,
            "a step of {:.1} at row {y}: {column:?}",
            pair[0] - pair[1]
        );
    }
    // And it is a light grey studio, not white or dark.
    let top = luminance(img.get_pixel(100, 5));
    assert!((150.0..235.0).contains(&top), "{top}");
}
