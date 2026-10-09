use opendrape_render::{
    CLEAR_COLOR, CubeRenderer, OrbitCamera, RenderTarget, headless_device, read_back,
};

fn is_background(p: &image::Rgba<u8>) -> bool {
    let bg = [
        (CLEAR_COLOR[0] * 255.0) as u8,
        (CLEAR_COLOR[1] * 255.0) as u8,
    ];
    p[0].abs_diff(bg[0]) <= 20 && p[1].abs_diff(bg[1]) <= 20
}

fn luminance(p: &image::Rgba<u8>) -> f32 {
    0.299 * p[0] as f32 + 0.587 * p[1] as f32 + 0.114 * p[2] as f32
}

fn render(width: u32, height: u32) -> image::RgbaImage {
    let gpu = headless_device().expect("a GPU or software adapter (lavapipe/WARP in CI)");
    eprintln!(
        "adapter: {} ({:?}, {:?})",
        gpu.info.name, gpu.info.backend, gpu.info.device_type
    );
    let target = RenderTarget::new(&gpu.device, width, height);
    let camera = OrbitCamera::default();
    CubeRenderer::new(&gpu.device).render(
        &gpu.device,
        &gpu.queue,
        &target,
        camera.view_proj(width as f32 / height as f32),
    );
    let image = read_back(&gpu.device, &gpu.queue, &target);
    image
        .save(format!(
            "{}/cube_{width}x{height}.png",
            env!("CARGO_TARGET_TMPDIR")
        ))
        .ok();
    image
}

#[test]
fn cube_is_centred_and_covers_a_sensible_area() {
    let img = render(128, 128);
    assert!(
        !is_background(img.get_pixel(64, 64)),
        "centre pixel is background"
    );
    let covered = img.pixels().filter(|p| !is_background(p)).count() as f32 / (128.0 * 128.0);
    assert!((0.10..0.60).contains(&covered), "cube coverage {covered}");
}

#[test]
fn visible_faces_are_shaded_differently() {
    // Guards against inverted triangle winding: back-face culling would then show the
    // three far (unlit) faces, which all come out the same flat colour.
    let img = render(128, 128);
    let lums: Vec<f32> = img
        .pixels()
        .filter(|p| !is_background(p))
        .map(luminance)
        .collect();
    let (lo, hi) = lums
        .iter()
        .fold((255f32, 0f32), |(lo, hi), &l| (lo.min(l), hi.max(l)));
    assert!(
        hi - lo > 40.0,
        "faces are not shaded differently: luminance {lo}..{hi}"
    );
}

#[test]
fn odd_sizes_read_back_correctly() {
    // 101 px rows are not a multiple of wgpu's 256-byte copy alignment, so readback must unpad.
    let img = render(101, 77);
    assert_eq!(img.dimensions(), (101, 77));
    assert!(
        !is_background(img.get_pixel(50, 38)),
        "centre pixel is background"
    );
}
