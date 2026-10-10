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
        rim: None,
        grid: Some(false),
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

/// A box `size` m wide spanning heights `y0`..`y1`, centred over the origin, flat-shaded
/// (separate vertices per face), wound outwards.
fn box_mesh(size: f32, y0: f32, y1: f32) -> (Vec<Vec3>, Vec<[u32; 3]>) {
    let (h, cy, hy) = (size / 2.0, (y0 + y1) / 2.0, (y1 - y0) / 2.0);
    let faces = [
        (Vec3::X, Vec3::Y, Vec3::Z),
        (Vec3::NEG_X, Vec3::Y, Vec3::NEG_Z),
        (Vec3::Y, Vec3::Z, Vec3::X),
        (Vec3::NEG_Y, Vec3::NEG_Z, Vec3::X),
        (Vec3::Z, Vec3::Y, Vec3::NEG_X),
        (Vec3::NEG_Z, Vec3::Y, Vec3::X),
    ];
    let scale = Vec3::new(h, hy, h);
    let (mut p, mut t) = (vec![], vec![]);
    for (n, up, side) in faces {
        let base = p.len() as u32;
        for (a, b) in [(-1.0, -1.0), (-1.0, 1.0), (1.0, 1.0), (1.0, -1.0)] {
            p.push((n + up * a + side * b) * scale + Vec3::new(0.0, cy, 0.0));
        }
        t.extend([[base, base + 2, base + 1], [base, base + 3, base + 2]]);
    }
    (p, t)
}

/// Where world point `p` lands in a `w`×`h` image from `camera`, in pixels.
fn pixel_of(camera: &OrbitCamera, (w, h): (u32, u32), p: Vec3) -> (u32, u32) {
    let clip = camera.view_proj(w as f32 / h as f32) * p.extend(1.0);
    let ndc = clip.truncate() / clip.w;
    let x = (ndc.x * 0.5 + 0.5) * w as f32;
    let y = (0.5 - ndc.y * 0.5) * h as f32;
    (x as u32, y as u32)
}

/// Mean luminance of the 3×3 pixels round world point `p`.
fn luminance_at(img: &image::RgbaImage, camera: &OrbitCamera, p: Vec3) -> f32 {
    let (x, y) = pixel_of(camera, (img.width(), img.height()), p);
    let mut sum = 0.0;
    for dy in 0..3 {
        for dx in 0..3 {
            sum += luminance(img.get_pixel(x + dx - 1, y + dy - 1));
        }
    }
    sum / 9.0
}

/// The floor right in front of something standing low over it is darker than open floor,
/// and the shadow fades out softly (no hard edge).
#[test]
fn the_floor_under_a_low_box_is_darker_with_a_soft_edge() {
    let g = gpu();
    let mut r = StudioRenderer::new(&g.device, &g.adapter, Quality::Medium);
    r.set_overrides(Overrides {
        contact: Some(true),
        ..plain()
    });
    let target = RenderTarget::new(&g.device, 400, 400);
    let camera = OrbitCamera {
        target: Vec3::new(0.0, 0.2, 0.0),
        yaw: 0.0,
        pitch: 0.5,
        distance: 2.2,
        fov_y: 35f32.to_radians(),
    };
    let b = mesh(
        &mut r,
        &g,
        box_mesh(0.3, 0.05, 0.35),
        [0.5; 3],
        Material::Form,
    );
    let img = render_still(&mut r, &g, &target, &camera, &[&b]);
    img.save(format!(
        "{}/studio_contact.png",
        env!("CARGO_TARGET_TMPDIR")
    ))
    .ok();
    let open = luminance_at(&img, &camera, Vec3::new(0.55, 0.0, 0.16));
    // Right at the box's base, 1 cm out (the floor under it is hidden from this camera).
    let near = luminance_at(&img, &camera, Vec3::new(0.0, 0.0, 0.16));
    // In light, not in display values (which are compressed): at least 20 % less.
    let light = |display: f32| opendrape_render::colour::srgb_to_linear(display / 255.0);
    assert!(
        light(near) <= 0.8 * light(open),
        "in front of the box {near}, open floor {open} (display values)"
    );
    // Walking out from the box: from 10 % to 90 % of the way back to open floor takes ≥ 6 px.
    let walk: Vec<(u32, f32)> = (0..90)
        .map(|i| {
            let p = Vec3::new(0.0, 0.0, 0.17 + 0.005 * i as f32);
            let (_, y) = pixel_of(&camera, (400, 400), p);
            (y, luminance_at(&img, &camera, p))
        })
        .collect();
    let darkest = walk.iter().map(|w| w.1).fold(f32::INFINITY, f32::min);
    let reach = |share: f32| {
        walk.iter()
            .find(|w| w.1 >= darkest + share * (open - darkest))
            .map(|w| w.0)
            .expect("the floor gets back to open-floor brightness")
    };
    let (from, to) = (reach(0.1), reach(0.9));
    assert!(
        from.abs_diff(to) >= 6,
        "the edge takes {} px",
        from.abs_diff(to)
    );
}

/// The key light comes from the front-right, above: the floor behind-left of a box is in its
/// shadow, the floor on the light's side is not.
#[test]
fn key_shadows_fall_on_the_floor_away_from_the_light() {
    let g = gpu();
    let mut r = StudioRenderer::new(&g.device, &g.adapter, Quality::Medium);
    r.set_overrides(Overrides {
        key_shadows: Some(true),
        ..plain()
    });
    let target = RenderTarget::new(&g.device, 400, 400);
    let camera = OrbitCamera {
        target: Vec3::ZERO,
        yaw: 0.0,
        pitch: 1.3,
        distance: 2.5,
        fov_y: 35f32.to_radians(),
    };
    let b = mesh(
        &mut r,
        &g,
        box_mesh(0.3, 0.0, 0.35),
        [0.5; 3],
        Material::Form,
    );
    let img = render_still(&mut r, &g, &target, &camera, &[&b]);
    img.save(format!(
        "{}/studio_key_shadow.png",
        env!("CARGO_TARGET_TMPDIR")
    ))
    .ok();
    let towards_light = opendrape_render::studio::environment::key_dir();
    let away = Vec3::new(-towards_light.x, 0.0, -towards_light.z).normalize() * 0.3;
    let shadowed = luminance_at(&img, &camera, away);
    let lit = luminance_at(&img, &camera, -away);
    assert!(
        shadowed < 0.9 * lit,
        "shadow side {shadowed}, light side {lit}"
    );
}

/// Thin cloth facing the light has no stripes of shadow on itself (shadow acne).
#[test]
fn cloth_facing_the_light_has_no_shadow_stripes() {
    let g = gpu();
    let mut r = StudioRenderer::new(&g.device, &g.adapter, Quality::High);
    r.set_overrides(Overrides {
        key_shadows: Some(true),
        ..plain()
    });
    let target = RenderTarget::new(&g.device, 256, 256);
    let c = mesh(&mut r, &g, card(1.0, 0.0, 0.6), [0.6; 3], Material::Cloth);
    let img = render_still(&mut r, &g, &target, &front_camera(1.2), &[&c]);
    img.save(format!("{}/studio_acne.png", env!("CARGO_TARGET_TMPDIR")))
        .ok();
    let values: Vec<f32> = (112..144)
        .flat_map(|y| (112..144).map(move |x| (x, y)))
        .map(|(x, y)| luminance(img.get_pixel(x, y)))
        .collect();
    let mean = values.iter().sum::<f32>() / values.len() as f32;
    let spread =
        (values.iter().map(|v| (v - mean).powi(2)).sum::<f32>() / values.len() as f32).sqrt();
    assert!(
        spread < 2.0,
        "luminance varies by {spread} over flat lit cloth"
    );
}

/// The shadow maps are drawn again only when the geometry changes, not when the camera moves.
#[test]
fn shadow_maps_are_redrawn_only_when_geometry_changes() {
    let g = gpu();
    let mut r = StudioRenderer::new(&g.device, &g.adapter, Quality::Medium);
    r.set_overrides(Overrides {
        key_shadows: Some(true),
        contact: Some(true),
        ..plain()
    });
    let target = RenderTarget::new(&g.device, 64, 64);
    let (p, t) = box_mesh(0.3, 0.0, 0.35);
    let mut b = r.create_mesh(&g.device, &g.queue, &p, &t, [0.5; 3], Material::Form);
    let mut camera = front_camera(2.0);
    r.render(&g.device, &g.queue, &target, &camera, &[&b]);
    let first = r.stats().shadow_redraws;
    assert!(first >= 1);
    camera.yaw += 0.3;
    r.render(&g.device, &g.queue, &target, &camera, &[&b]);
    assert_eq!(r.stats().shadow_redraws, first, "only the camera moved");
    let moved: Vec<Vec3> = p.iter().map(|v| *v + Vec3::X * 0.1).collect();
    r.update_mesh(&g.device, &g.queue, &mut b, &moved, None);
    r.render(&g.device, &g.queue, &target, &camera, &[&b]);
    assert_eq!(r.stats().shadow_redraws, first + 1, "the box moved");
}

/// Two 0.5 m pages meeting in a 90° valley along the y axis, opening towards +Z (an open book
/// facing the camera), centred on (0, 1, 0).
fn crease() -> (Vec<Vec3>, Vec<[u32; 3]>) {
    let d = 0.35;
    let p = vec![
        Vec3::new(0.0, 0.75, 0.0),
        Vec3::new(0.0, 1.25, 0.0),
        Vec3::new(-d, 1.25, d),
        Vec3::new(-d, 0.75, d),
        Vec3::new(d, 0.75, d),
        Vec3::new(d, 1.25, d),
    ];
    // Both pages wound to face the camera (+Z side).
    (p, vec![[0, 1, 2], [0, 2, 3], [0, 4, 5], [0, 5, 1]])
}

/// Mean light (linear) of the pixels in `xs` × `ys`.
fn mean_light(img: &image::RgbaImage, xs: std::ops::Range<u32>, ys: std::ops::Range<u32>) -> f32 {
    let mut sum = 0.0;
    let mut n = 0.0;
    for y in ys {
        for x in xs.clone() {
            sum += opendrape_render::colour::srgb_to_linear(luminance(img.get_pixel(x, y)) / 255.0);
            n += 1.0;
        }
    }
    sum / n
}

fn render_with_ao(ao: bool, shape: (Vec<Vec3>, Vec<[u32; 3]>)) -> image::RgbaImage {
    let g = gpu();
    let mut r = StudioRenderer::new(&g.device, &g.adapter, Quality::Medium);
    r.set_overrides(Overrides {
        ao: Some(ao),
        ..plain()
    });
    let target = RenderTarget::new(&g.device, 256, 256);
    let m = mesh(&mut r, &g, shape, [0.6; 3], Material::Cloth);
    render_still(&mut r, &g, &target, &front_camera(1.3), &[&m])
}

/// Soft darkening gathers in a fold: the bottom of a crease is clearly darker with it.
#[test]
fn ao_darkens_a_crease() {
    let (with, without) = (
        render_with_ao(true, crease()),
        render_with_ao(false, crease()),
    );
    with.save(format!(
        "{}/studio_crease_ao.png",
        env!("CARGO_TARGET_TMPDIR")
    ))
    .ok();
    let valley = |img: &image::RgbaImage| mean_light(img, 126..130, 100..156);
    let (a, b) = (valley(&with), valley(&without));
    assert!(a <= 0.85 * b, "the valley with AO {a}, without {b}");
}

/// ...and leaves flat surfaces alone.
#[test]
fn ao_leaves_flat_surfaces_alone() {
    let flat = || card(1.0, 0.0, 0.6);
    let (with, without) = (render_with_ao(true, flat()), render_with_ao(false, flat()));
    let middle = |img: &image::RgbaImage| mean_light(img, 112..144, 112..144);
    let (a, b) = (middle(&with), middle(&without));
    assert!(
        (a - b).abs() <= 0.02 * b,
        "flat card with AO {a}, without {b}"
    );
}

#[test]
fn ao_runs_at_every_quality_and_size() {
    let g = gpu();
    for quality in Quality::ALL {
        let mut r = StudioRenderer::new(&g.device, &g.adapter, quality);
        r.set_overrides(Overrides {
            ao: Some(true),
            ..Overrides::default()
        });
        let m = mesh(&mut r, &g, crease(), [0.5; 3], Material::Cloth);
        for (w, h) in [(1, 1), (37, 19), (300, 200)] {
            let scope = g.device.push_error_scope(wgpu::ErrorFilter::Validation);
            let target = RenderTarget::new(&g.device, w, h);
            render_still(&mut r, &g, &target, &front_camera(1.5), &[&m]);
            let error = pollster::block_on(scope.pop());
            assert!(error.is_none(), "{quality:?} {w}×{h}: {error:?}");
        }
    }
}

fn still_frames(q: Quality) -> u32 {
    q.settings().still_frames
}

/// Once nothing changes, the view adds up its still frames and then stops drawing.
#[test]
fn a_still_view_finishes_and_then_does_nothing() {
    let g = gpu();
    let mut r = StudioRenderer::new(&g.device, &g.adapter, Quality::Medium);
    let target = RenderTarget::new(&g.device, 64, 64);
    let c = mesh(&mut r, &g, card(1.0, 0.0, 0.6), [0.5; 3], Material::Cloth);
    let camera = front_camera(1.5);
    let n = still_frames(Quality::Medium);
    // One moving frame, a short settle with nothing drawn, then n still ones.
    let last = 1 + opendrape_render::studio::SETTLE_FRAMES + n;
    for call in 1..=last {
        let done = r
            .render(&g.device, &g.queue, &target, &camera, &[&c])
            .still_done;
        assert_eq!(done, call == last, "call {call}");
    }
    let drawn = r.stats().frames_drawn;
    let again = r.render(&g.device, &g.queue, &target, &camera, &[&c]);
    assert!(!again.drew && again.still_done);
    assert_eq!(r.stats().frames_drawn, drawn, "nothing drawn once finished");
}

#[test]
fn moving_the_camera_starts_over() {
    let g = gpu();
    let mut r = StudioRenderer::new(&g.device, &g.adapter, Quality::Basic);
    let target = RenderTarget::new(&g.device, 64, 64);
    let c = mesh(&mut r, &g, card(1.0, 0.0, 0.6), [0.5; 3], Material::Cloth);
    let mut camera = front_camera(1.5);
    render_still(&mut r, &g, &target, &camera, &[&c]);
    camera.yaw += 0.01;
    let after = r.render(&g.device, &g.queue, &target, &camera, &[&c]);
    assert!(after.drew && !after.still_done);
}

/// Selecting a piece recolours it: the finished image must not keep the old colour.
#[test]
fn changing_a_colour_starts_over() {
    let g = gpu();
    let mut r = StudioRenderer::new(&g.device, &g.adapter, Quality::Basic);
    let target = RenderTarget::new(&g.device, 64, 64);
    let mut c = mesh(&mut r, &g, card(1.0, 0.0, 0.6), [0.5; 3], Material::Cloth);
    let camera = front_camera(1.5);
    render_still(&mut r, &g, &target, &camera, &[&c]);
    c.set_colour([0.9, 0.4, 0.1]);
    let after = r.render(&g.device, &g.queue, &target, &camera, &[&c]);
    assert!(after.drew && !after.still_done);
}

/// Luminance-weighted centre of what differs from the backdrop's top-left pixel.
fn centroid(img: &image::RgbaImage) -> (f32, f32) {
    let back = luminance(img.get_pixel(0, 0));
    let (mut sx, mut sy, mut sw) = (0.0, 0.0, 0.0);
    for (x, y, p) in img.enumerate_pixels() {
        let w = (luminance(p) - back).abs();
        sx += w * x as f32;
        sy += w * y as f32;
        sw += w;
    }
    (sx / sw, sy / sw)
}

/// The sub-pixel shifts that smooth edges when still don't move the picture.
#[test]
fn jitter_does_not_shift_the_image() {
    let g = gpu();
    let mut r = StudioRenderer::new(&g.device, &g.adapter, Quality::Medium);
    r.set_overrides(plain());
    let target = RenderTarget::new(&g.device, 128, 128);
    let c = mesh(&mut r, &g, card(1.0, 0.0, 0.4), [0.2; 3], Material::Form);
    let camera = front_camera(1.5);
    r.render(&g.device, &g.queue, &target, &camera, &[&c]);
    let moving = centroid(&read_back(&g.device, &g.queue, &target));
    let still = centroid(&render_still(&mut r, &g, &target, &camera, &[&c]));
    assert!(
        (moving.0 - still.0).abs() < 0.25 && (moving.1 - still.1).abs() < 0.25,
        "{moving:?} vs {still:?}"
    );
}

/// A card turned 20°, so its edges are slanted.
fn slanted_card() -> (Vec<Vec3>, Vec<[u32; 3]>) {
    let (p, t) = card(1.0, 0.0, 0.5);
    let turn = glam::Quat::from_rotation_z(20f32.to_radians());
    let centre = Vec3::new(0.0, 1.0, 0.0);
    (
        p.into_iter()
            .map(|v| turn * (v - centre) + centre)
            .collect(),
        t,
    )
}

/// Shades between the card and the backdrop along its slanted edges: more of them once still
/// (the edges were averaged over many sub-pixel positions).
fn edge_shades(img: &image::RgbaImage) -> usize {
    let mut shades = std::collections::BTreeSet::new();
    let (back, card) = (
        luminance(img.get_pixel(2, 2)),
        luminance(img.get_pixel(64, 64)),
    );
    let (lo, hi) = (back.min(card) + 3.0, back.max(card) - 3.0);
    for p in img.pixels() {
        let l = luminance(p);
        if l > lo && l < hi {
            shades.insert(l.round() as i32);
        }
    }
    shades.len()
}

#[test]
fn still_edges_are_smoother_than_moving_ones() {
    let g = gpu();
    let mut r = StudioRenderer::new(&g.device, &g.adapter, Quality::Basic);
    r.set_overrides(plain());
    let target = RenderTarget::new(&g.device, 128, 128);
    let c = mesh(&mut r, &g, slanted_card(), [0.1; 3], Material::Form);
    let camera = front_camera(1.5);
    r.render(&g.device, &g.queue, &target, &camera, &[&c]);
    let moving = edge_shades(&read_back(&g.device, &g.queue, &target));
    let still = edge_shades(&render_still(&mut r, &g, &target, &camera, &[&c]));
    assert!(
        still > moving + 4,
        "edge shades: moving {moving}, still {still}"
    );
}

#[test]
fn resizing_restarts_cleanly() {
    let g = gpu();
    let mut r = StudioRenderer::new(&g.device, &g.adapter, Quality::High);
    let c = mesh(&mut r, &g, card(1.0, 0.0, 0.6), [0.5; 3], Material::Cloth);
    let camera = front_camera(1.5);
    let scope = g.device.push_error_scope(wgpu::ErrorFilter::Validation);
    for (w, h) in [(200, 100), (1, 1), (300, 300)] {
        let target = RenderTarget::new(&g.device, w, h);
        let first = r.render(&g.device, &g.queue, &target, &camera, &[&c]);
        assert!(first.drew && !first.still_done, "{w}×{h} starts over");
        render_still(&mut r, &g, &target, &camera, &[&c]);
        assert!(
            r.render(&g.device, &g.queue, &target, &camera, &[&c])
                .still_done
        );
    }
    assert!(pollster::block_on(scope.pop()).is_none());
}

/// The OpenGL fallback (Linux without Vulkan, Windows' third choice) draws the studio at every
/// level without errors. Linux only: that is where CI has an OpenGL driver (llvmpipe).
#[cfg(target_os = "linux")]
#[test]
fn the_studio_draws_on_opengl() {
    let Some(g) = opendrape_render::headless_device_with(wgpu::Backends::GL) else {
        panic!("no OpenGL adapter (llvmpipe should be there in CI)");
    };
    for quality in Quality::ALL {
        let mut r = StudioRenderer::new(&g.device, &g.adapter, quality);
        r.set_overrides(Overrides {
            ao: Some(true),
            key_shadows: Some(true),
            ..Overrides::default()
        });
        let m = mesh(&mut r, &g, crease(), [0.5; 3], Material::Cloth);
        let scope = g.device.push_error_scope(wgpu::ErrorFilter::Validation);
        let target = RenderTarget::new(&g.device, 96, 64);
        render_still(&mut r, &g, &target, &front_camera(1.5), &[&m]);
        let error = pollster::block_on(scope.pop());
        assert!(error.is_none(), "{quality:?} on GL: {error:?}");
    }
}

/// While the drape plays or a drag is held, frames where nothing new arrived are not "still":
/// no still frames start (they would flicker against the moving ones).
#[test]
fn held_motion_never_starts_still_frames() {
    let g = gpu();
    let mut r = StudioRenderer::new(&g.device, &g.adapter, Quality::Basic);
    let target = RenderTarget::new(&g.device, 64, 64);
    let (p, t) = crease();
    let mut cloth = r.create_mesh(&g.device, &g.queue, &p, &t, [0.5; 3], Material::Cloth);
    let camera = front_camera(1.5);
    r.set_moving(true);
    for call in 0..20 {
        if call % 2 == 0 {
            let moved: Vec<Vec3> = p
                .iter()
                .map(|v| *v + Vec3::Y * 0.001 * call as f32)
                .collect();
            r.update_mesh(&g.device, &g.queue, &mut cloth, &moved, None);
        }
        let frame = r.render(&g.device, &g.queue, &target, &camera, &[&cloth]);
        assert!(!frame.still_done);
    }
    assert_eq!(r.stats().still_frames, 0, "no still frames while held");
    r.set_moving(false);
    render_still(&mut r, &g, &target, &camera, &[&cloth]);
    assert!(r.stats().still_frames > 0, "still frames once let go");
}

/// A gap of a frame between two drape frames doesn't start still frames either.
#[test]
fn a_short_pause_does_not_start_still_frames() {
    let g = gpu();
    let mut r = StudioRenderer::new(&g.device, &g.adapter, Quality::Basic);
    let target = RenderTarget::new(&g.device, 64, 64);
    let mut camera = front_camera(1.5);
    let c = mesh(&mut r, &g, card(1.0, 0.0, 0.6), [0.5; 3], Material::Cloth);
    for _ in 0..10 {
        camera.yaw += 0.01;
        r.render(&g.device, &g.queue, &target, &camera, &[&c]);
        let pause = r.render(&g.device, &g.queue, &target, &camera, &[&c]);
        assert!(!pause.drew, "nothing new to draw");
    }
    assert_eq!(r.stats().still_frames, 0);
}

/// A box `size` m square and `height` tall standing on the floor at (`x`, 0, `z`).
fn box_at(x: f32, z: f32, size: f32, height: f32) -> (Vec<Vec3>, Vec<[u32; 3]>) {
    let (p, t) = box_mesh(size, 0.0, height);
    (p.into_iter().map(|v| v + Vec3::new(x, 0.0, z)).collect(), t)
}

/// Something standing well away from the centre still casts its shadow: the key light's
/// shadow map follows what is in the scene.
#[test]
fn shadows_follow_things_away_from_the_centre() {
    let g = gpu();
    let mut r = StudioRenderer::new(&g.device, &g.adapter, Quality::Medium);
    r.set_overrides(Overrides {
        key_shadows: Some(true),
        ..plain()
    });
    let target = RenderTarget::new(&g.device, 400, 400);
    let camera = OrbitCamera {
        target: Vec3::new(1.6, 0.0, 0.0),
        yaw: 0.0,
        pitch: 1.3,
        distance: 3.0,
        fov_y: 35f32.to_radians(),
    };
    let b = mesh(
        &mut r,
        &g,
        box_at(1.6, 0.0, 0.3, 1.0),
        [0.5; 3],
        Material::Form,
    );
    let img = render_still(&mut r, &g, &target, &camera, &[&b]);
    let towards_light = opendrape_render::studio::environment::key_dir();
    let away = Vec3::new(-towards_light.x, 0.0, -towards_light.z).normalize() * 0.5;
    let base = Vec3::new(1.6, 0.0, 0.0);
    let shadowed = luminance_at(&img, &camera, base + away);
    let lit = luminance_at(&img, &camera, base - away);
    assert!(
        shadowed < 0.9 * lit,
        "shadow side {shadowed}, light side {lit}"
    );
}

/// A soft rim light from behind-left (opposite the key) outlines the figure: surfaces facing
/// it are brighter with it.
#[test]
fn the_rim_light_lights_what_faces_it() {
    let rim = |on: bool| {
        let g = gpu();
        let mut r = StudioRenderer::new(&g.device, &g.adapter, Quality::Medium);
        r.set_overrides(Overrides {
            rim: Some(on),
            ..plain()
        });
        let target = RenderTarget::new(&g.device, 128, 128);
        // A card turned to face back-left (azimuth 220°), seen from that side.
        let turn = glam::Quat::from_rotation_y(220f32.to_radians());
        let centre = Vec3::new(0.0, 1.0, 0.0);
        let (p, t) = card(1.0, 0.0, 0.6);
        let p = p
            .into_iter()
            .map(|v| turn * (v - centre) + centre)
            .collect();
        let c = mesh(&mut r, &g, (p, t), [0.5; 3], Material::Cloth);
        let camera = OrbitCamera {
            yaw: 220f32.to_radians(),
            ..front_camera(1.5)
        };
        mean_light(
            &render_still(&mut r, &g, &target, &camera, &[&c]),
            56..72,
            56..72,
        )
    };
    let (with, without) = (rim(true), rim(false));
    assert!(with >= 1.1 * without, "with rim {with}, without {without}");
}

/// Mean light (linear) of a column of pixels `x`, rows `ys`.
fn column_light(img: &image::RgbaImage, x: u32, ys: std::ops::Range<u32>) -> f32 {
    mean_light(img, x..x + 1, ys)
}

/// The floor shows a grid: faint lines every 10 cm, stronger ones every metre.
#[test]
fn the_floor_shows_a_grid() {
    let g = gpu();
    let mut r = StudioRenderer::new(&g.device, &g.adapter, Quality::Medium);
    r.set_overrides(Overrides {
        grid: Some(true),
        ..plain()
    });
    let target = RenderTarget::new(&g.device, 600, 600);
    let camera = OrbitCamera {
        target: Vec3::new(0.5, 0.0, 0.0),
        yaw: 0.0,
        pitch: 1.5,
        distance: 2.0,
        fov_y: 35f32.to_radians(),
    };
    let img = render_still(&mut r, &g, &target, &camera, &[]);
    img.save(format!("{}/studio_grid.png", env!("CARGO_TARGET_TMPDIR")))
        .ok();
    let size = (img.width(), img.height());
    let x_of = |x: f32| pixel_of(&camera, size, Vec3::new(x, 0.0, 0.0)).0;
    let rows = 250..350;
    let between = column_light(&img, x_of(0.25), rows.clone());
    let minor = (x_of(0.2)..=x_of(0.2) + 1)
        .map(|x| column_light(&img, x, rows.clone()))
        .fold(f32::INFINITY, f32::min);
    let major = (x_of(1.0)..=x_of(1.0) + 1)
        .map(|x| column_light(&img, x, rows.clone()))
        .fold(f32::INFINITY, f32::min);
    assert!(
        minor < 0.985 * between,
        "10 cm line {minor} vs between {between}"
    );
    assert!(major < minor, "1 m line {major} vs 10 cm line {minor}");
}
