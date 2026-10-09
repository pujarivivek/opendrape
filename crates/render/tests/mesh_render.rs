use glam::Vec3;
use opendrape_render::{
    CLEAR_COLOR, MeshRenderer, OrbitCamera, RenderTarget, headless_device, read_back,
    vertex_normals,
};

/// Cube with separate vertices per face (flat shading), counter-clockwise from outside.
fn flat_cube() -> (Vec<Vec3>, Vec<[u32; 3]>) {
    let faces = [
        (Vec3::X, Vec3::Y, Vec3::Z),
        (Vec3::NEG_X, Vec3::Y, Vec3::NEG_Z),
        (Vec3::Y, Vec3::Z, Vec3::X),
        (Vec3::NEG_Y, Vec3::NEG_Z, Vec3::X),
        (Vec3::Z, Vec3::Y, Vec3::NEG_X),
        (Vec3::NEG_Z, Vec3::Y, Vec3::X),
    ];
    let (mut p, mut t) = (vec![], vec![]);
    for (n, up, side) in faces {
        let base = p.len() as u32;
        for (a, b) in [(-1.0, -1.0), (-1.0, 1.0), (1.0, 1.0), (1.0, -1.0)] {
            p.push((n + up * a + side * b) * 0.5);
        }
        t.extend([[base, base + 2, base + 1], [base, base + 3, base + 2]]);
    }
    (p, t)
}

fn is_background(p: &image::Rgba<u8>) -> bool {
    let bg = [
        (CLEAR_COLOR[0] * 255.0) as u8,
        (CLEAR_COLOR[1] * 255.0) as u8,
    ];
    p[0].abs_diff(bg[0]) <= 20 && p[1].abs_diff(bg[1]) <= 20
}

#[test]
fn draws_a_lit_mesh_in_the_centre() {
    let gpu = headless_device().expect("a GPU or software adapter");
    let target = RenderTarget::new(&gpu.device, 128, 128);
    let r = MeshRenderer::new(&gpu.device);
    let (p, t) = flat_cube();
    let cube = r.create_mesh(&gpu.device, &gpu.queue, &p, &t, [0.85, 0.45, 0.30]);
    r.render(
        &gpu.device,
        &gpu.queue,
        &target,
        OrbitCamera::default().view_proj(1.0),
        &[&cube],
    );
    let img = read_back(&gpu.device, &gpu.queue, &target);
    img.save(format!("{}/mesh_cube.png", env!("CARGO_TARGET_TMPDIR")))
        .ok();
    assert!(!is_background(img.get_pixel(64, 64)));
    let lum = |p: &image::Rgba<u8>| 0.299 * p[0] as f32 + 0.587 * p[1] as f32 + 0.114 * p[2] as f32;
    let lums: Vec<f32> = img
        .pixels()
        .filter(|p| !is_background(p))
        .map(lum)
        .collect();
    let (lo, hi) = lums
        .iter()
        .fold((255f32, 0f32), |(lo, hi), &l| (lo.min(l), hi.max(l)));
    assert!(
        hi - lo > 40.0,
        "faces should be shaded differently: {lo}..{hi}"
    );
}

#[test]
fn updating_positions_moves_the_mesh() {
    let gpu = headless_device().expect("a GPU or software adapter");
    let target = RenderTarget::new(&gpu.device, 96, 96);
    let r = MeshRenderer::new(&gpu.device);
    let (p, t) = flat_cube();
    let mut cube = r.create_mesh(&gpu.device, &gpu.queue, &p, &t, [0.85, 0.45, 0.30]);
    let moved: Vec<Vec3> = p.iter().map(|v| *v + Vec3::X * 50.0).collect(); // far out of view
    r.update_mesh(&gpu.device, &gpu.queue, &mut cube, &moved, None);
    r.render(
        &gpu.device,
        &gpu.queue,
        &target,
        OrbitCamera::default().view_proj(1.0),
        &[&cube],
    );
    let img = read_back(&gpu.device, &gpu.queue, &target);
    assert!(img.pixels().all(is_background), "mesh moved away");
}

#[test]
fn flat_quad_normals_point_along_its_face() {
    let p = [Vec3::ZERO, Vec3::X, Vec3::new(1.0, 1.0, 0.0), Vec3::Y];
    let n = vertex_normals(&p, &[[0, 1, 2], [0, 2, 3]]);
    assert!(n.iter().all(|v| v.abs_diff_eq(Vec3::Z, 1e-6)), "{n:?}");
    let lonely = vertex_normals(&[Vec3::ZERO], &[]);
    assert!(lonely[0].is_finite(), "unused vertices get a finite normal");
}

fn render_cube_with(
    gpu: &opendrape_render::HeadlessGpu,
    width: u32,
    height: u32,
) -> image::RgbaImage {
    let target = RenderTarget::new(&gpu.device, width, height);
    let r = MeshRenderer::new(&gpu.device);
    let (p, t) = flat_cube();
    let cube = r.create_mesh(&gpu.device, &gpu.queue, &p, &t, [0.85, 0.45, 0.30]);
    let view_proj = OrbitCamera::default().view_proj(width as f32 / height as f32);
    r.render(&gpu.device, &gpu.queue, &target, view_proj, &[&cube]);
    read_back(&gpu.device, &gpu.queue, &target)
}

#[test]
fn cube_covers_a_sensible_area() {
    let gpu = headless_device().expect("a GPU or software adapter");
    let img = render_cube_with(&gpu, 128, 128);
    let covered = img.pixels().filter(|p| !is_background(p)).count() as f32 / (128.0 * 128.0);
    assert!((0.10..0.60).contains(&covered), "cube coverage {covered}");
}

#[test]
fn odd_sizes_read_back_correctly() {
    // 101 px rows are not a multiple of wgpu's 256-byte copy alignment, so readback must unpad.
    let gpu = headless_device().expect("a GPU or software adapter");
    let img = render_cube_with(&gpu, 101, 77);
    assert_eq!(img.dimensions(), (101, 77));
    assert!(
        !is_background(img.get_pixel(50, 38)),
        "centre pixel is background"
    );
}

/// The backend every machine of this OS has (Metal on macOS, DX12 on Windows — WARP on CI —,
/// Vulkan on Linux — lavapipe on CI).
fn native_backend() -> wgpu::Backends {
    if cfg!(target_os = "macos") {
        wgpu::Backends::METAL
    } else if cfg!(windows) {
        wgpu::Backends::DX12
    } else {
        wgpu::Backends::VULKAN
    }
}

#[test]
fn explicitly_requested_backend_is_used() {
    let wanted = native_backend();
    let gpu = opendrape_render::headless_device_with(wanted)
        .expect("this OS's native backend is available");
    assert!(
        wanted.contains(wgpu::Backends::from(gpu.info.backend)),
        "got {:?}",
        gpu.info.backend
    );
    let img = render_cube_with(&gpu, 96, 96);
    assert!(
        !is_background(img.get_pixel(48, 48)),
        "{:?}: centre pixel is background",
        gpu.info.backend
    );
}

/// OpenGL is the main fallback on old Intel laptops; CI renders through Mesa llvmpipe.
#[cfg(target_os = "linux")]
#[test]
fn opengl_fallback_renders_the_cube() {
    let gpu = opendrape_render::headless_device_with(wgpu::Backends::GL)
        .expect("Mesa OpenGL (llvmpipe) is installed");
    assert_eq!(gpu.info.backend, wgpu::Backend::Gl);
    let img = render_cube_with(&gpu, 96, 96);
    assert!(
        !is_background(img.get_pixel(48, 48)),
        "centre pixel is background"
    );
}
