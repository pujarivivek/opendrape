# OpenDrape M0 (Foundations) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A native OpenDrape desktop app (macOS universal + Windows x64) that installs without admin rights, opens a window with a shaded 3D cube in a wgpu viewport, survives bad graphics drivers via automatic backend fallback, shows an About box with copyable diagnostics, and is built, tested and published by CI as downloadable installers.

**Architecture:** Cargo workspace with three members.
- `crates/render` draws into offscreen `Rgba8Unorm` textures. It works headless for tests and future PNG export.
- `crates/app` is the eframe/egui application. It shows those textures with `egui_wgpu::Renderer::register_native_texture`. It also owns graphics-backend selection, the crash sentinel (`gpu.json`), Fluent strings and the About/diagnostics UI.
- `xtask` generates the app icons.

Packaging and release:
- `cargo-packager` builds the `.app` and the NSIS installer.
- Plain `hdiutil` builds the DMG. The packager's own DMG step scripts Finder and timed out locally.
- GitHub Actions publishes a rolling `nightly` pre-release.

**Tech Stack:**
- Rust 1.99.0; eframe / egui / egui-wgpu / egui_kittest 0.36.2; wgpu 30.0.1; glam 0.33.
- i18n-embed 0.16 + i18n-embed-fl 0.10 (Fluent); directories 6; rfd 0.17; winresource 0.1.
- cargo-packager 0.11.8; cargo-nextest; cargo-deny.

**Spec:** `docs/specs/2026-10-09-opendrape-design.md` (milestone M0)

**Verified before writing:** every API this plan uses was checked against the downloaded crate sources and compiled in a throwaway probe on 2026-10-09:
- the cube renderer, readback and kittest-with-wgpu harness;
- the universal `.app` and DMG (12 MB);
- the cargo-deny allowlist.

## Global Constraints

- Toolchain: `rust-toolchain.toml` pins `1.99.0` with rustfmt and clippy. `rust-version = "1.95"` (egui 0.36's MSRV).
- Exact pins:
  - `eframe`, `egui`, `egui-wgpu` and `egui_kittest` at `=0.36.2`; `wgpu` at `=30.0.1`.
  - `glam = "0.33"`, never 0.34: the future `parry3d` 0.31 needs glam 0.33.
- License: workspace `license = "GPL-3.0-or-later"`.
  - `cargo deny check` must pass the allowlist in `deny.toml`.
  - Never add SMPL/SMPL-X data, MakeHuman *code* (AGPL), or Shewchuk's Triangle.
- Naming:
  - Product name "OpenDrape"; never use "CLO" anywhere.
  - Bundle identifier `org.opendrape.OpenDrape`; binary name `opendrape`.
  - Crates are `opendrape` (app) and `opendrape-render`.
- GPU rules:
  - Textures shown in egui must be `wgpu::TextureFormat::Rgba8Unorm`, so shaders encode sRGB themselves.
  - Shaders must stay inside `wgpu::Limits::downlevel_webgl2_defaults()`: no compute, no storage buffers.
  - Request devices with `downlevel_webgl2_defaults().using_resolution(adapter.limits())`.
- 3D units: metres, Y up.
- Strings:
  - Every user-visible string goes through `tr!` (Fluent, `en-US`).
  - The diagnostics text is English on purpose, because it's for bug reports.
  - Turn off Fluent's Unicode isolation marks (`set_use_isolating(false)`): egui would draw them as boxes.
- Platforms:
  - macOS minimum 11.0, shipped as a universal binary (arm64 + x86_64), ad-hoc signed.
  - Windows x64: NSIS installer in `currentUser` mode (no admin) plus a portable ZIP.
  - Linux is CI-only until M8.
- Robustness:
  - Never panic because the config directory is missing, unwritable or holds a corrupt file. The app must still start on a locked-down lab PC.
  - Release builds on Windows use the `windows` subsystem, so no console window appears.
- Deviations from the spec, on purpose:
  1. i18n lives in the app crate as `crates/app/src/i18n.rs`, not in a separate `crates/i18n`. `fl!` checks message IDs against the `i18n.toml` of the crate being compiled, so a wrapper crate would lose that compile-time check.
  2. M0 keeps egui's bundled default fonts, with a cargo-deny exception for `epaint_default_fonts` (OFL-1.1, Ubuntu-font-1.0). Noto Sans arrives with Hindi support after 1.0.
  3. Golden-image tests start in M1 with `testkit`. M0's render tests assert coverage and shading numerically.

## Review Focus

The five input conditions most likely to hurt a real user that no feature test naturally exercises. Each line has a pinning test in the task named.

1. **Zero, negative or NaN viewport size** (minimised window, collapsed panel): no texture is created and nothing panics. Task 2 `target_size` tests; Task 5 viewport early return.
2. **HiDPI and very large displays** (pixels-per-point 1.25 or 2.0, 5K screens, GL adapters capped at 2048 px textures): the render target is in physical pixels and scaled down to fit the GPU limit. Task 2 `target_size` tests.
3. **Corrupt or half-written `gpu.json`** (power cut during a save, hand edits): falls back to defaults. Saves are atomic (write a temp file, then rename). Task 3.
4. **Missing or unwritable config directory** (lab PCs, portable use): the app runs and nothing is remembered. Task 3.
5. **Crash loops**:
   - a graphics mode that keeps failing must step down the fallback chain and then stop with a message, never relaunch forever;
   - a `--gpu=` override never triggers a relaunch;
   - unknown launch arguments (for example macOS `-psn_…`) are ignored.

   Task 3 `decide` / `should_relaunch_after_error` tests and Task 5 `Cli::parse` tests.

---

## File Structure

```
Cargo.toml                      workspace: members, shared deps, profiles
rust-toolchain.toml             pins Rust 1.99.0
.cargo/config.toml              `cargo xtask` alias
deny.toml                       license/source policy (GPL-3.0 compatible allowlist)
LICENSE                         GPL-3.0 text
README.md                       what/why, download + "Open Anyway" help, build-from-source
ASSETS.md                       provenance ledger for every non-code asset
assets/icon@2x.png, icon.ico    generated by `cargo xtask icons`
crates/render/
  Cargo.toml
  src/lib.rs                    re-exports
  src/camera.rs                 OrbitCamera (pure math)
  src/target.rs                 target_size(), RenderTarget, formats, CLEAR_COLOR
  src/cube.rs + cube.wgsl       CubeRenderer
  src/headless.rs               HeadlessGpu, headless_device(), read_back()
  tests/cube_render.rs          GPU render tests
crates/app/
  Cargo.toml                    + [package.metadata.packager]
  build.rs                      git sha env var; Windows .exe icon
  i18n.toml, i18n/en-US/opendrape.ftl
  src/lib.rs                    module wiring, Startup, Shared
  src/main.rs                   startup flow: CLI → gpu decision → run → relaunch/error
  src/cli.rs                    Cli::parse
  src/i18n.rs                   LOADER + tr! macro
  src/gpu/mod.rs                re-exports
  src/gpu/choice.rs             Os, GpuChoice, fallback chains, pick_adapter
  src/gpu/state.rs              GpuState, Decision, decide(), StateStore, relaunch rules
  src/gpu/setup.rs              native_options(), show_startup_error()
  src/diagnostics.rs            Diagnostics::collect/to_text
  src/viewport.rs               3D panel (native texture, orbit input)
  src/app.rs                    OpenDrapeApp: menus, About, graphics switch, smoke test
  tests/ui.rs                   egui_kittest flows
xtask/Cargo.toml, xtask/src/main.rs   icon generator
scripts/package-macos.sh        universal .app + DMG
scripts/package-windows.ps1     NSIS installer + portable ZIP
.github/workflows/ci.yml        fmt, clippy, deny, nextest on Linux/Windows/macOS
.github/workflows/release.yml   build installers, smoke-test, publish nightly / tagged releases
docs/PORTABLE.txt               read-me inside the portable ZIP
docs/testing/M0-checklist.md    plain-language test steps for the user
docs/testing/nightly-notes.md   release notes for nightly builds
```

All shell commands assume `source ~/.cargo/env` has been run (rustup was installed without editing the shell profile) and the working directory is the repo root `/Users/vivekpuajri/Developer/vibe-clo3d`.

---

### Task 1: Workspace skeleton, license, CI lint/test pipeline

**Files:**
- Create: `Cargo.toml`, `rust-toolchain.toml`, `.cargo/config.toml`, `deny.toml`, `LICENSE`, `README.md`, `ASSETS.md`
- Create: `crates/app/Cargo.toml`, `crates/app/src/lib.rs`, `crates/app/src/main.rs`, `crates/app/tests/ui.rs`
- Create: `.github/workflows/ci.yml`
- Modify: `.gitignore`

**Interfaces:**
- Produces: workspace with `[workspace.dependencies]` that later tasks reference with `x.workspace = true`; `opendrape::OpenDrapeApp` (rewritten in Task 5).

- [ ] **Step 1: Write the workspace manifest and toolchain pin**

`Cargo.toml`:
```toml
[workspace]
resolver = "3"
members = ["crates/app"]

[workspace.package]
version = "0.1.0"
edition = "2024"
rust-version = "1.95"
license = "GPL-3.0-or-later"
repository = "https://github.com/pujarivivek/opendrape"
publish = false

[workspace.dependencies]
opendrape-render = { path = "crates/render" }
eframe = { version = "=0.36.2", default-features = false, features = ["wgpu", "accesskit", "default_fonts", "wayland", "x11"] }
egui = "=0.36.2"
egui-wgpu = "=0.36.2"
egui_kittest = { version = "=0.36.2", features = ["wgpu", "eframe"] }
wgpu = "=30.0.1"
glam = { version = "0.33", features = ["bytemuck"] }
bytemuck = { version = "1", features = ["derive"] }
image = { version = "0.25", default-features = false, features = ["png", "ico"] }
pollster = "0.4"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
directories = "6"
rfd = { version = "0.17", default-features = false }
i18n-embed = { version = "0.16", features = ["fluent-system"] }
i18n-embed-fl = "0.10"
rust-embed = "8"
tempfile = "3"

[workspace.lints.clippy]
all = { level = "warn", priority = -1 }

[profile.release]
lto = true
codegen-units = 1

# Optimise dependencies in debug builds so the 3D view is usable while developing.
[profile.dev.package."*"]
opt-level = 2
```

`rust-toolchain.toml`:
```toml
[toolchain]
channel = "1.99.0"
components = ["rustfmt", "clippy"]
```

`.cargo/config.toml`:
```toml
[alias]
xtask = "run --quiet -p xtask --"
```

Append to `.gitignore`:
```
/target
/dist
```
(The existing file already has `/target`; only add `/dist` if missing.)

- [ ] **Step 2: Write the failing UI test for the app crate**

`crates/app/Cargo.toml`:
```toml
[package]
name = "opendrape"
description = "Free, open-source 3D garment design for students"
version.workspace = true
edition.workspace = true
rust-version.workspace = true
license.workspace = true
repository.workspace = true
publish.workspace = true

[dependencies]
eframe.workspace = true
egui.workspace = true

[dev-dependencies]
egui_kittest.workspace = true

[lints]
workspace = true
```

`crates/app/tests/ui.rs`:
```rust
use egui_kittest::{Harness, kittest::Queryable};
use opendrape::OpenDrapeApp;

#[test]
fn window_shows_app_name() {
    let mut harness = Harness::builder()
        .with_size(egui::vec2(800.0, 600.0))
        .build_eframe(|cc| OpenDrapeApp::new(cc));
    harness.run();
    harness.get_by_label("OpenDrape");
}
```

`crates/app/src/lib.rs` (empty crate so the test fails to compile):
```rust
//! OpenDrape desktop application.
```

- [ ] **Step 3: Run the test and watch it fail**

Run: `cargo test -p opendrape --test ui`
Expected: compile error `unresolved import opendrape::OpenDrapeApp`.

- [ ] **Step 4: Minimal app**

`crates/app/src/lib.rs`:
```rust
//! OpenDrape desktop application.

pub struct OpenDrapeApp;

impl OpenDrapeApp {
    pub fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        Self
    }
}

impl eframe::App for OpenDrapeApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ui, |ui| {
            ui.heading("OpenDrape");
        });
    }
}
```

`crates/app/src/main.rs`:
```rust
fn main() -> eframe::Result {
    eframe::run_native(
        "OpenDrape",
        eframe::NativeOptions::default(),
        Box::new(|cc| Ok(Box::new(opendrape::OpenDrapeApp::new(cc)))),
    )
}
```

- [ ] **Step 5: Run the test and see it pass**

Run: `cargo test -p opendrape --test ui`
Expected: `test window_shows_app_name ... ok`

- [ ] **Step 6: License policy, licence text, docs**

`deny.toml` (allowlist verified against the real dependency tree on 2026-10-09):
```toml
[graph]
all-features = true

[licenses]
# Every license here is compatible with distributing OpenDrape under GPL-3.0-or-later.
allow = [
    "GPL-3.0-or-later",
    "MIT",
    "Apache-2.0",
    "Apache-2.0 WITH LLVM-exception",
    "BSD-2-Clause",
    "BSD-3-Clause",
    "0BSD",
    "ISC",
    "Zlib",
    "BSL-1.0",
    "Unicode-3.0",
    "Unlicense",
    "MPL-2.0",
]
confidence-threshold = 0.9
exceptions = [
    # egui's bundled UI fonts (Ubuntu Light, Hack, Noto Emoji, emoji-icon-font).
    { allow = ["OFL-1.1", "Ubuntu-font-1.0"], crate = "epaint_default_fonts" },
]

[bans]
multiple-versions = "allow"
wildcards = "deny"
allow-wildcard-paths = true

[sources]
unknown-registry = "deny"
unknown-git = "deny"

[advisories]
ignore = []
```

Download the GPL text: `curl -fsSL https://www.gnu.org/licenses/gpl-3.0.txt -o LICENSE` and check that `head -3 LICENSE` shows "GNU GENERAL PUBLIC LICENSE / Version 3, 29 June 2007".

`README.md`:
```markdown
# OpenDrape

Free, open-source 3D garment design for fashion students: draw 2D patterns, sew them
onto a 3D body, and see how the garment drapes. It runs on everyday laptops (4 GB RAM,
built-in graphics) on Windows and macOS, fully offline.

OpenDrape exists because commercial 3D fashion tools cost hundreds of dollars a year
and need gaming-class graphics cards, which shuts out most students in India, Africa
and elsewhere. Digital samples also replace muslin toiles, so less fabric is wasted.

**Status:** early development (milestone M0: foundations). There is no garment tool yet.
See `docs/specs/2026-10-09-opendrape-design.md` for the full plan.

## Download

Get the latest build from the [Releases page](https://github.com/pujarivivek/opendrape/releases)
("Nightly build").

- **Mac:** download the `.dmg`, open it, drag OpenDrape to Applications. The first time,
  macOS blocks it because it is not signed with a paid Apple certificate yet: open
  **System Settings → Privacy & Security**, scroll down, click **Open Anyway**.
- **Windows:** run the `-setup.exe` (no administrator rights needed). If Windows says
  "Windows protected your PC", click **More info → Run anyway**. Or download the
  portable `.zip`, unzip it anywhere and run `OpenDrape.exe`.

If the 3D view does not appear, choose **Help → Graphics → Software (safe mode, slow)**
or start OpenDrape with `--gpu=safe`.

## Build from source

Install Rust from https://rustup.rs, then:

    cargo run -p opendrape

## License

OpenDrape is free software under the GNU General Public License v3 or later (see `LICENSE`).
Third-party assets and their licenses are listed in `ASSETS.md`.
```

`ASSETS.md`:
```markdown
# Asset provenance

Every non-code asset shipped with OpenDrape, where it came from, and its license.
Add a row before committing any new asset.

| Asset | Source | License |
|---|---|---|
| `assets/icon@2x.png`, `assets/icon.ico` | Generated by `cargo xtask icons` (this repository) | GPL-3.0-or-later |
| egui default UI fonts (embedded via `epaint_default_fonts`) | egui project | OFL-1.1, Ubuntu Font License 1.0 |
```

- [ ] **Step 7: CI workflow**

`.github/workflows/ci.yml`:
```yaml
name: CI

on:
  push:
    branches: [main]
  pull_request:

permissions:
  contents: read

env:
  CARGO_TERM_COLOR: always

jobs:
  lint:
    runs-on: ubuntu-24.04
    steps:
      - uses: actions/checkout@v5
      - run: rustup show
      - uses: Swatinem/rust-cache@v2
      - uses: taiki-e/install-action@v2
        with:
          tool: cargo-deny
      - name: Linux libraries
        run: sudo apt-get update && sudo apt-get install -y libxkbcommon-dev libwayland-dev
      - run: cargo fmt --all --check
      - run: cargo clippy --workspace --all-targets -- -D warnings
      - run: cargo deny check

  test:
    strategy:
      fail-fast: false
      matrix:
        os: [ubuntu-24.04, windows-2025, macos-15]
    runs-on: ${{ matrix.os }}
    steps:
      - uses: actions/checkout@v5
      - run: rustup show
      - uses: Swatinem/rust-cache@v2
      - uses: taiki-e/install-action@v2
        with:
          tool: cargo-nextest
      - name: Linux libraries and software Vulkan (lavapipe)
        if: runner.os == 'Linux'
        run: sudo apt-get update && sudo apt-get install -y libxkbcommon-dev libwayland-dev libvulkan1 mesa-vulkan-drivers
      - run: cargo nextest run --workspace --no-fail-fast
```

- [ ] **Step 8: Verify locally**

Run: `cargo fmt --all --check && cargo clippy --workspace --all-targets -- -D warnings && cargo deny check && cargo nextest run --workspace`
Expected: no formatting diff, no clippy warnings, `advisories ok, bans ok, licenses ok, sources ok`, 1 test passed.

- [ ] **Step 9: Commit**

```bash
git add Cargo.toml Cargo.lock rust-toolchain.toml .cargo deny.toml LICENSE README.md ASSETS.md .gitignore crates .github
git commit -m "build: workspace skeleton, GPL-3.0 license, CI

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 2: `opendrape-render`, offscreen cube renderer

**Files:**
- Create: `crates/render/Cargo.toml`, `crates/render/src/{lib.rs,camera.rs,target.rs,cube.rs,cube.wgsl,headless.rs}`, `crates/render/tests/cube_render.rs`
- Modify: `Cargo.toml` (`members`)

**Interfaces:**
- Produces:
  - `OrbitCamera { target: Vec3, yaw: f32, pitch: f32, distance: f32, fov_y: f32 }` with `eye()`, `drag(dx, dy)`, `zoom(scroll)`, `view_proj(aspect) -> Mat4`, and consts `MIN_PITCH`, `MAX_PITCH`, `MIN_DISTANCE`, `MAX_DISTANCE`.
  - `target_size(width_pts: f32, height_pts: f32, pixels_per_point: f32, max_dim: u32) -> Option<(u32, u32)>`
  - `RenderTarget::new(&wgpu::Device, u32, u32)` with pub fields `width`, `height`, `color: wgpu::Texture`, `color_view: wgpu::TextureView`.
  - `CubeRenderer::new(&wgpu::Device)` and `CubeRenderer::render(&self, &Device, &Queue, &RenderTarget, Mat4)`.
  - `HeadlessGpu { device, queue, info: wgpu::AdapterInfo }`, `headless_device() -> Option<HeadlessGpu>`, `read_back(&Device, &Queue, &RenderTarget) -> image::RgbaImage`.
  - Consts `COLOR_FORMAT`, `DEPTH_FORMAT`, `CLEAR_COLOR: [f64; 4]`.

- [ ] **Step 1: Crate manifest and failing camera/size tests**

Add `"crates/render"` to `members` in the root `Cargo.toml`.

`crates/render/Cargo.toml`:
```toml
[package]
name = "opendrape-render"
description = "OpenDrape offscreen 3D renderer"
version.workspace = true
edition.workspace = true
rust-version.workspace = true
license.workspace = true
repository.workspace = true
publish.workspace = true

[dependencies]
wgpu.workspace = true
glam.workspace = true
bytemuck.workspace = true
image.workspace = true
pollster.workspace = true

[lints]
workspace = true
```

`crates/render/src/lib.rs`:
```rust
//! OpenDrape 3D rendering. Scenes are drawn into offscreen textures that the app
//! shows in its 3D panel, and that tests (and later PNG export) read back.

mod camera;
mod target;

pub use camera::OrbitCamera;
pub use target::target_size;
```

`crates/render/src/camera.rs`, tests first and the struct body stubbed:
```rust
use glam::{Mat4, Vec3};

/// A camera orbiting a target point: drag to rotate, scroll to zoom.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct OrbitCamera {
    pub target: Vec3,
    /// Radians around the Y axis; 0 looks at the target from +Z.
    pub yaw: f32,
    /// Radians above the horizon.
    pub pitch: f32,
    /// Metres from the target.
    pub distance: f32,
    pub fov_y: f32,
}

impl OrbitCamera {
    pub const MIN_PITCH: f32 = -1.45;
    pub const MAX_PITCH: f32 = 1.45;
    pub const MIN_DISTANCE: f32 = 0.3;
    pub const MAX_DISTANCE: f32 = 20.0;

    pub fn eye(&self) -> Vec3 {
        todo!()
    }
    pub fn drag(&mut self, _dx: f32, _dy: f32) {
        todo!()
    }
    pub fn zoom(&mut self, _scroll: f32) {
        todo!()
    }
    pub fn view_proj(&self, _aspect: f32) -> Mat4 {
        todo!()
    }
}

impl Default for OrbitCamera {
    fn default() -> Self {
        Self { target: Vec3::ZERO, yaw: 0.6, pitch: 0.35, distance: 3.5, fov_y: 45f32.to_radians() }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn eye_is_in_front_at_zero_angles() {
        let cam = OrbitCamera { yaw: 0.0, pitch: 0.0, distance: 2.0, ..Default::default() };
        assert!(cam.eye().abs_diff_eq(Vec3::new(0.0, 0.0, 2.0), 1e-6), "{}", cam.eye());
    }

    #[test]
    fn drag_clamps_pitch() {
        let mut cam = OrbitCamera::default();
        cam.drag(0.0, 10_000.0);
        assert_eq!(cam.pitch, OrbitCamera::MAX_PITCH);
        cam.drag(0.0, -10_000.0);
        assert_eq!(cam.pitch, OrbitCamera::MIN_PITCH);
    }

    #[test]
    fn zoom_clamps_distance() {
        let mut cam = OrbitCamera::default();
        cam.zoom(1e6);
        assert_eq!(cam.distance, OrbitCamera::MIN_DISTANCE);
        cam.zoom(-1e6);
        assert_eq!(cam.distance, OrbitCamera::MAX_DISTANCE);
    }

    #[test]
    fn target_projects_to_screen_centre_with_wgpu_depth() {
        let cam = OrbitCamera::default();
        let clip = cam.view_proj(16.0 / 9.0) * cam.target.extend(1.0);
        let ndc = clip.truncate() / clip.w;
        assert!(ndc.x.abs() < 1e-5 && ndc.y.abs() < 1e-5, "{ndc}");
        assert!((0.0..=1.0).contains(&ndc.z), "depth must be in wgpu's 0..1 range: {}", ndc.z);
    }
}
```

`crates/render/src/target.rs`, tests first:
```rust
/// Physical-pixel size of the render target for a viewport of `width`×`height`
/// points, or `None` when it is too small to draw (minimised window, collapsed panel).
/// Scales down, keeping the aspect ratio, to fit the GPU's maximum texture size.
pub fn target_size(_width: f32, _height: f32, _pixels_per_point: f32, _max_dim: u32) -> Option<(u32, u32)> {
    todo!()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zero_negative_or_nan_sizes_are_skipped() {
        assert_eq!(target_size(0.0, 300.0, 2.0, 8192), None);
        assert_eq!(target_size(300.0, 0.2, 2.0, 8192), None);
        assert_eq!(target_size(-5.0, 10.0, 1.0, 8192), None);
        assert_eq!(target_size(f32::NAN, 10.0, 1.0, 8192), None);
    }

    #[test]
    fn hidpi_uses_physical_pixels() {
        assert_eq!(target_size(400.0, 300.0, 2.0, 8192), Some((800, 600)));
        assert_eq!(target_size(401.0, 301.0, 1.25, 8192), Some((501, 376)));
    }

    #[test]
    fn oversized_viewport_is_scaled_to_fit_gpu_limit() {
        // A 5K display at 2x on an OpenGL adapter limited to 2048 px textures.
        assert_eq!(target_size(2560.0, 1440.0, 2.0, 2048), Some((2048, 1152)));
    }
}
```

- [ ] **Step 2: Run the tests and watch them fail**

Run: `cargo nextest run -p opendrape-render`
Expected: the 7 tests FAIL with `not yet implemented`.

- [ ] **Step 3: Implement camera and sizing**

Replace the four `todo!()` bodies in `camera.rs`:
```rust
    pub fn eye(&self) -> Vec3 {
        let (sy, cy) = self.yaw.sin_cos();
        let (sp, cp) = self.pitch.sin_cos();
        self.target + Vec3::new(sy * cp, sp, cy * cp) * self.distance
    }

    /// Rotate by a mouse drag measured in screen points.
    pub fn drag(&mut self, dx: f32, dy: f32) {
        self.yaw -= dx * 0.01;
        self.pitch = (self.pitch + dy * 0.01).clamp(Self::MIN_PITCH, Self::MAX_PITCH);
    }

    /// Zoom by a scroll amount in points (positive moves closer).
    pub fn zoom(&mut self, scroll: f32) {
        self.distance =
            (self.distance * (-scroll * 0.002).exp()).clamp(Self::MIN_DISTANCE, Self::MAX_DISTANCE);
    }

    pub fn view_proj(&self, aspect: f32) -> Mat4 {
        let view = glam::camera::rh::view::look_at_mat4(self.eye(), self.target, Vec3::Y);
        let proj =
            glam::camera::rh::proj::directx::perspective(self.fov_y, aspect.max(1e-3), 0.05, 100.0);
        proj * view
    }
```
(`directx` = depth range 0..1, which is what wgpu uses. `Mat4::look_at_rh` / `perspective_rh` are deprecated in glam 0.33.)

Replace `target_size`:
```rust
pub fn target_size(width: f32, height: f32, pixels_per_point: f32, max_dim: u32) -> Option<(u32, u32)> {
    let w = (width * pixels_per_point).round();
    let h = (height * pixels_per_point).round();
    // Written this way so NaN fails too.
    if !(w >= 1.0 && h >= 1.0) {
        return None;
    }
    let scale = (max_dim as f32 / w.max(h)).min(1.0);
    Some((((w * scale).floor() as u32).max(1), ((h * scale).floor() as u32).max(1)))
}
```

- [ ] **Step 4: Run the tests and see them pass**

Run: `cargo nextest run -p opendrape-render`
Expected: 7 passed.

- [ ] **Step 5: Write the failing GPU render tests**

`crates/render/tests/cube_render.rs`:
```rust
use opendrape_render::{CLEAR_COLOR, CubeRenderer, OrbitCamera, RenderTarget, headless_device, read_back};

fn is_background(p: &image::Rgba<u8>) -> bool {
    let bg = [(CLEAR_COLOR[0] * 255.0) as u8, (CLEAR_COLOR[1] * 255.0) as u8];
    p[0].abs_diff(bg[0]) <= 20 && p[1].abs_diff(bg[1]) <= 20
}

fn luminance(p: &image::Rgba<u8>) -> f32 {
    0.299 * p[0] as f32 + 0.587 * p[1] as f32 + 0.114 * p[2] as f32
}

fn render(width: u32, height: u32) -> image::RgbaImage {
    let gpu = headless_device().expect("a GPU or software adapter (lavapipe/WARP in CI)");
    eprintln!("adapter: {} ({:?}, {:?})", gpu.info.name, gpu.info.backend, gpu.info.device_type);
    let target = RenderTarget::new(&gpu.device, width, height);
    let camera = OrbitCamera::default();
    CubeRenderer::new(&gpu.device).render(&gpu.device, &gpu.queue, &target, camera.view_proj(width as f32 / height as f32));
    let image = read_back(&gpu.device, &gpu.queue, &target);
    image.save(format!("{}/cube_{width}x{height}.png", env!("CARGO_TARGET_TMPDIR"))).ok();
    image
}

#[test]
fn cube_is_centred_and_covers_a_sensible_area() {
    let img = render(128, 128);
    assert!(!is_background(img.get_pixel(64, 64)), "centre pixel is background");
    let covered = img.pixels().filter(|p| !is_background(p)).count() as f32 / (128.0 * 128.0);
    assert!((0.10..0.60).contains(&covered), "cube coverage {covered}");
}

#[test]
fn visible_faces_are_shaded_differently() {
    // Guards against inverted triangle winding: back-face culling would then show the
    // three far (unlit) faces, which all come out the same flat colour.
    let img = render(128, 128);
    let lums: Vec<f32> = img.pixels().filter(|p| !is_background(p)).map(luminance).collect();
    let (lo, hi) = lums.iter().fold((255f32, 0f32), |(lo, hi), &l| (lo.min(l), hi.max(l)));
    assert!(hi - lo > 40.0, "faces are not shaded differently: luminance {lo}..{hi}");
}

#[test]
fn odd_sizes_read_back_correctly() {
    // 101 px rows are not a multiple of wgpu's 256-byte copy alignment, so readback must unpad.
    let img = render(101, 77);
    assert_eq!(img.dimensions(), (101, 77));
    assert!(!is_background(img.get_pixel(50, 38)), "centre pixel is background");
}
```

- [ ] **Step 6: Run the tests and watch them fail**

Run: `cargo nextest run -p opendrape-render --test cube_render`
Expected: compile errors (`CubeRenderer`, `RenderTarget`, `headless_device`, `read_back`, `CLEAR_COLOR` not found).

- [ ] **Step 7: Implement target, cube and headless modules**

Append to `crates/render/src/target.rs` (above `#[cfg(test)]`):
```rust
/// egui only displays native textures in this format, so shaders encode sRGB themselves.
pub const COLOR_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;
pub const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;
/// Viewport background (light grey), RGBA 0..1.
pub const CLEAR_COLOR: [f64; 4] = [0.93, 0.93, 0.95, 1.0];

/// Colour + depth textures for one viewport.
pub struct RenderTarget {
    pub width: u32,
    pub height: u32,
    pub color: wgpu::Texture,
    pub color_view: wgpu::TextureView,
    pub(crate) depth_view: wgpu::TextureView,
}

impl RenderTarget {
    pub fn new(device: &wgpu::Device, width: u32, height: u32) -> Self {
        let size = wgpu::Extent3d { width: width.max(1), height: height.max(1), depth_or_array_layers: 1 };
        let color = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("viewport color"),
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: COLOR_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                | wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let depth = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("viewport depth"),
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: DEPTH_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });
        let color_view = color.create_view(&wgpu::TextureViewDescriptor::default());
        let depth_view = depth.create_view(&wgpu::TextureViewDescriptor::default());
        Self { width: size.width, height: size.height, color, color_view, depth_view }
    }
}
```

`crates/render/src/cube.wgsl`:
```wgsl
struct Uniforms {
    view_proj: mat4x4<f32>,
    light_dir: vec4<f32>,
};
@group(0) @binding(0) var<uniform> u: Uniforms;

struct VsIn {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
};
struct VsOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) normal: vec3<f32>,
};

@vertex
fn vs_main(v: VsIn) -> VsOut {
    var out: VsOut;
    out.clip = u.view_proj * vec4<f32>(v.position, 1.0);
    out.normal = v.normal;
    return out;
}

@fragment
fn fs_main(v: VsOut) -> @location(0) vec4<f32> {
    let n = normalize(v.normal);
    let diffuse = max(dot(n, normalize(u.light_dir.xyz)), 0.0);
    let base = vec3<f32>(0.85, 0.45, 0.30);
    let linear = base * (0.25 + 0.75 * diffuse);
    // The target is Rgba8Unorm (egui requirement), so encode to sRGB here.
    return vec4<f32>(pow(linear, vec3<f32>(1.0 / 2.2)), 1.0);
}
```

`crates/render/src/cube.rs`:
```rust
use crate::target::{CLEAR_COLOR, COLOR_FORMAT, DEPTH_FORMAT, RenderTarget};
use glam::{Mat4, Vec3};
use wgpu::util::DeviceExt;

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct Vertex {
    position: [f32; 3],
    normal: [f32; 3],
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct Uniforms {
    view_proj: [[f32; 4]; 4],
    light_dir: [f32; 4],
}

/// Draws a 1 m lit cube at the origin: the M0 stand-in for the avatar and garment.
pub struct CubeRenderer {
    pipeline: wgpu::RenderPipeline,
    vertices: wgpu::Buffer,
    vertex_count: u32,
    uniforms: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
}

impl CubeRenderer {
    pub fn new(device: &wgpu::Device) -> Self {
        let shader = device.create_shader_module(wgpu::include_wgsl!("cube.wgsl"));
        let uniforms = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("cube uniforms"),
            size: std::mem::size_of::<Uniforms>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("cube uniforms layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("cube uniforms"),
            layout: &layout,
            entries: &[wgpu::BindGroupEntry { binding: 0, resource: uniforms.as_entire_binding() }],
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("cube pipeline layout"),
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("cube pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: &[Some(wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<Vertex>() as u64,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x3],
                })],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                compilation_options: Default::default(),
                targets: &[Some(COLOR_FORMAT.into())],
            }),
            primitive: wgpu::PrimitiveState { cull_mode: Some(wgpu::Face::Back), ..Default::default() },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: DEPTH_FORMAT,
                depth_write_enabled: Some(true),
                depth_compare: Some(wgpu::CompareFunction::Less),
                stencil: Default::default(),
                bias: Default::default(),
            }),
            multisample: Default::default(),
            multiview_mask: None,
            cache: None,
        });
        let verts = cube_vertices();
        let vertices = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("cube vertices"),
            contents: bytemuck::cast_slice(&verts),
            usage: wgpu::BufferUsages::VERTEX,
        });
        Self { pipeline, vertices, vertex_count: verts.len() as u32, uniforms, bind_group }
    }

    /// Clear `target` and draw the cube. Submits its own command buffer.
    pub fn render(&self, device: &wgpu::Device, queue: &wgpu::Queue, target: &RenderTarget, view_proj: Mat4) {
        let uniforms = Uniforms { view_proj: view_proj.to_cols_array_2d(), light_dir: [0.2, 1.0, 0.5, 0.0] };
        queue.write_buffer(&self.uniforms, 0, bytemuck::bytes_of(&uniforms));
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("cube") });
        {
            let [r, g, b, a] = CLEAR_COLOR;
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("cube pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &target.color_view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations { load: wgpu::LoadOp::Clear(wgpu::Color { r, g, b, a }), store: wgpu::StoreOp::Store },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &target.depth_view,
                    depth_ops: Some(wgpu::Operations { load: wgpu::LoadOp::Clear(1.0), store: wgpu::StoreOp::Store }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, &self.bind_group, &[]);
            pass.set_vertex_buffer(0, self.vertices.slice(..));
            pass.draw(0..self.vertex_count, 0..1);
        }
        queue.submit([encoder.finish()]);
    }
}

/// 36 vertices, counter-clockwise when seen from outside (front faces for back-face culling).
fn cube_vertices() -> Vec<Vertex> {
    // (outward normal, up, side) where side = normal × up.
    let faces: [(Vec3, Vec3, Vec3); 6] = [
        (Vec3::X, Vec3::Y, Vec3::Z),
        (Vec3::NEG_X, Vec3::Y, Vec3::NEG_Z),
        (Vec3::Y, Vec3::Z, Vec3::X),
        (Vec3::NEG_Y, Vec3::NEG_Z, Vec3::X),
        (Vec3::Z, Vec3::Y, Vec3::NEG_X),
        (Vec3::NEG_Z, Vec3::Y, Vec3::X),
    ];
    let mut out = Vec::with_capacity(36);
    for (n, up, side) in faces {
        let corner = |a: f32, b: f32| (n + up * a + side * b) * 0.5;
        let quad = [corner(-1.0, -1.0), corner(1.0, 1.0), corner(-1.0, 1.0), corner(-1.0, -1.0), corner(1.0, -1.0), corner(1.0, 1.0)];
        out.extend(quad.map(|p| Vertex { position: p.to_array(), normal: n.to_array() }));
    }
    out
}
```

`crates/render/src/headless.rs`:
```rust
use crate::target::RenderTarget;

/// A GPU device with no window, for tests and offscreen export.
pub struct HeadlessGpu {
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub info: wgpu::AdapterInfo,
}

/// Any working adapter: a real GPU if there is one, otherwise a software one
/// (WARP on Windows, lavapipe/llvmpipe on Linux). `None` if nothing works.
pub fn headless_device() -> Option<HeadlessGpu> {
    pollster::block_on(async {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(),
            ..wgpu::InstanceDescriptor::new_without_display_handle()
        });
        let mut adapter = None;
        for force_fallback_adapter in [false, true] {
            adapter = instance
                .request_adapter(&wgpu::RequestAdapterOptions { force_fallback_adapter, ..Default::default() })
                .await
                .ok();
            if adapter.is_some() {
                break;
            }
        }
        let adapter = adapter?;
        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some("OpenDrape headless"),
                required_limits: wgpu::Limits::downlevel_webgl2_defaults().using_resolution(adapter.limits()),
                ..Default::default()
            })
            .await
            .ok()?;
        Some(HeadlessGpu { device, queue, info: adapter.get_info() })
    })
}

/// Copy the colour texture of `target` back to the CPU. Blocks until the GPU is done.
pub fn read_back(device: &wgpu::Device, queue: &wgpu::Queue, target: &RenderTarget) -> image::RgbaImage {
    let unpadded = target.width * 4;
    let align = wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
    let padded = unpadded.div_ceil(align) * align;
    let buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("readback"),
        size: u64::from(padded) * u64::from(target.height),
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("readback") });
    encoder.copy_texture_to_buffer(
        target.color.as_image_copy(),
        wgpu::TexelCopyBufferInfo {
            buffer: &buffer,
            layout: wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(padded), rows_per_image: None },
        },
        wgpu::Extent3d { width: target.width, height: target.height, depth_or_array_layers: 1 },
    );
    queue.submit([encoder.finish()]);
    buffer.map_async(wgpu::MapMode::Read, .., |result| result.expect("map readback buffer"));
    device.poll(wgpu::PollType::wait_indefinitely()).expect("wait for GPU");
    let data = buffer.get_mapped_range(..).expect("readback buffer is mapped");
    let mut pixels = Vec::with_capacity((unpadded * target.height) as usize);
    for row in data.chunks(padded as usize) {
        pixels.extend_from_slice(&row[..unpadded as usize]);
    }
    image::RgbaImage::from_raw(target.width, target.height, pixels).expect("pixel count matches size")
}
```

Update `crates/render/src/lib.rs`:
```rust
//! OpenDrape 3D rendering. Scenes are drawn into offscreen textures that the app
//! shows in its 3D panel, and that tests (and later PNG export) read back.

mod camera;
mod cube;
mod headless;
mod target;

pub use camera::OrbitCamera;
pub use cube::CubeRenderer;
pub use headless::{HeadlessGpu, headless_device, read_back};
pub use target::{CLEAR_COLOR, COLOR_FORMAT, DEPTH_FORMAT, RenderTarget, target_size};
```

- [ ] **Step 8: Run all render tests and see them pass, then look at the image**

Run: `cargo nextest run -p opendrape-render`
Expected: 10 passed. Open `target/tmp/cube_128x128.png` with the Read tool: it shows a terracotta cube whose top face is clearly lighter than its sides.

- [ ] **Step 9: Commit**

```bash
cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings
git add Cargo.toml Cargo.lock crates/render
git commit -m "feat(render): offscreen wgpu cube renderer with orbit camera and headless readback

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 3: Graphics backend choice, fallback chain and crash sentinel

**Files:**
- Create: `crates/app/src/gpu/{mod.rs,choice.rs,state.rs,setup.rs}`
- Modify: `crates/app/Cargo.toml`, `crates/app/src/lib.rs`

**Interfaces:**
- Produces, in module `opendrape::gpu`:
  - `enum Os { Windows, MacOs, Linux }` with `Os::current()`.
  - `enum GpuChoice { Auto, Dx12, Vulkan, Metal, Gl, Software }`, serialised lowercase, with `parse(&str) -> Option<Self>`, `available(Os) -> &'static [GpuChoice]`, `next_fallback(self, Os) -> Option<GpuChoice>` and `backends(self) -> wgpu::Backends`.
  - `pick_adapter(&[wgpu::DeviceType], software: bool) -> Option<usize>`.
  - `struct GpuState { preferred: GpuChoice, pending: Option<GpuChoice> }` (Default = Auto, None).
  - `enum Reason { CommandLine, Saved, RecoveredFromCrash(GpuChoice), NoMoreFallbacks }` and `struct Decision { choice, reason }`.
  - `decide(Option<GpuChoice>, &GpuState, Os) -> Decision`.
  - `pending_marker(Decision, &GpuState) -> Option<GpuState>`.
  - `confirmed_state(Decision) -> Option<GpuState>`.
  - `should_relaunch_after_error(Decision, Os) -> bool`.
  - `StateStore` with `new(Option<&Path>)`, `default_location()`, `load() -> GpuState` and `save(&GpuState)`.
  - `native_options(GpuChoice) -> eframe::NativeOptions`.
  - `show_startup_error(&str)`.

- [ ] **Step 1: Dependencies and failing tests for choice.rs**

Add to `crates/app/Cargo.toml` `[dependencies]`:
```toml
egui-wgpu.workspace = true
wgpu.workspace = true
serde.workspace = true
serde_json.workspace = true
directories.workspace = true
```
and add `tempfile.workspace = true` to `[dev-dependencies]`. Then add a target-specific section:
```toml
[target.'cfg(any(windows, target_os = "macos"))'.dependencies]
rfd.workspace = true
```

`crates/app/src/gpu/mod.rs` (Steps 5 and 9 add the `state` and `setup` modules):
```rust
//! Which graphics backend to start, and recovery when one crashes or fails.

mod choice;

pub use choice::{GpuChoice, Os, pick_adapter};
```

Add `pub mod gpu;` to `crates/app/src/lib.rs`.

`crates/app/src/gpu/choice.rs` (signatures with `todo!()`, plus tests):
```rust
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Os {
    Windows,
    MacOs,
    Linux,
}

impl Os {
    pub fn current() -> Self {
        if cfg!(target_os = "windows") {
            Os::Windows
        } else if cfg!(target_os = "macos") {
            Os::MacOs
        } else {
            Os::Linux
        }
    }
}

/// Which graphics backend OpenDrape asks wgpu for.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum GpuChoice {
    Auto,
    Dx12,
    Vulkan,
    Metal,
    Gl,
    /// CPU rendering (WARP on Windows, llvmpipe/lavapipe on Linux): slow but works everywhere.
    Software,
}

impl GpuChoice {
    pub fn parse(_s: &str) -> Option<Self> {
        todo!()
    }
    /// Choices offered in Help → Graphics on this OS, safest last.
    pub fn available(_os: Os) -> &'static [GpuChoice] {
        todo!()
    }
    /// What to try after this choice failed to start, or `None` when nothing is left.
    pub fn next_fallback(self, _os: Os) -> Option<GpuChoice> {
        todo!()
    }
    pub fn backends(self) -> wgpu::Backends {
        todo!()
    }
}

/// Index of the adapter to use, given the device types wgpu enumerated (in order).
/// `software` picks a CPU adapter; otherwise the most capable real GPU, falling back to CPU.
pub fn pick_adapter(_types: &[wgpu::DeviceType], _software: bool) -> Option<usize> {
    todo!()
}

#[cfg(test)]
mod tests {
    use super::*;
    use wgpu::DeviceType as T;

    #[test]
    fn parses_names_and_aliases() {
        assert_eq!(GpuChoice::parse("auto"), Some(GpuChoice::Auto));
        assert_eq!(GpuChoice::parse("DX12"), Some(GpuChoice::Dx12));
        assert_eq!(GpuChoice::parse("opengl"), Some(GpuChoice::Gl));
        assert_eq!(GpuChoice::parse(" safe "), Some(GpuChoice::Software));
        assert_eq!(GpuChoice::parse("warp"), Some(GpuChoice::Software));
        assert_eq!(GpuChoice::parse("banana"), None);
        assert_eq!(GpuChoice::parse(""), None);
    }

    #[test]
    fn windows_falls_back_auto_gl_software_then_stops() {
        let os = Os::Windows;
        assert_eq!(GpuChoice::Auto.next_fallback(os), Some(GpuChoice::Gl));
        assert_eq!(GpuChoice::Gl.next_fallback(os), Some(GpuChoice::Software));
        assert_eq!(GpuChoice::Software.next_fallback(os), None);
        // An explicitly chosen backend that fails continues after Auto.
        assert_eq!(GpuChoice::Dx12.next_fallback(os), Some(GpuChoice::Gl));
    }

    #[test]
    fn macos_has_only_metal() {
        assert_eq!(GpuChoice::available(Os::MacOs), &[GpuChoice::Auto]);
        assert_eq!(GpuChoice::Auto.next_fallback(Os::MacOs), None);
        assert_eq!(GpuChoice::Metal.next_fallback(Os::MacOs), None);
    }

    #[test]
    fn every_offered_choice_reaches_the_end_of_its_chain() {
        for os in [Os::Windows, Os::MacOs, Os::Linux] {
            for &start in GpuChoice::available(os) {
                let mut steps = 0;
                let mut c = Some(start);
                while let Some(choice) = c {
                    c = choice.next_fallback(os);
                    steps += 1;
                    assert!(steps < 10, "fallback loop from {start:?} on {os:?}");
                }
            }
        }
    }

    #[test]
    fn auto_prefers_real_gpus_and_accepts_cpu_as_last_resort() {
        assert_eq!(pick_adapter(&[T::Cpu, T::IntegratedGpu, T::DiscreteGpu], false), Some(2));
        assert_eq!(pick_adapter(&[T::Cpu, T::IntegratedGpu], false), Some(1));
        assert_eq!(pick_adapter(&[T::Cpu], false), Some(0));
        assert_eq!(pick_adapter(&[], false), None);
    }

    #[test]
    fn software_only_picks_cpu_adapters() {
        assert_eq!(pick_adapter(&[T::DiscreteGpu, T::Cpu], true), Some(1));
        assert_eq!(pick_adapter(&[T::DiscreteGpu], true), None);
    }

    #[test]
    fn auto_excludes_opengl_so_it_stays_a_separate_fallback() {
        assert!(!GpuChoice::Auto.backends().contains(wgpu::Backends::GL));
        assert_eq!(GpuChoice::Gl.backends(), wgpu::Backends::GL);
    }
}
```

- [ ] **Step 2: Run the tests and watch them fail**

Run: `cargo nextest run -p opendrape gpu::choice`
Expected: 7 FAIL with `not yet implemented`.

- [ ] **Step 3: Implement choice.rs**

```rust
impl GpuChoice {
    pub fn parse(s: &str) -> Option<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "auto" => Some(Self::Auto),
            "dx12" | "d3d12" | "directx" => Some(Self::Dx12),
            "vulkan" => Some(Self::Vulkan),
            "metal" => Some(Self::Metal),
            "gl" | "opengl" => Some(Self::Gl),
            "software" | "safe" | "warp" | "cpu" => Some(Self::Software),
            _ => None,
        }
    }

    pub fn available(os: Os) -> &'static [GpuChoice] {
        match os {
            Os::Windows => &[Self::Auto, Self::Dx12, Self::Vulkan, Self::Gl, Self::Software],
            Os::MacOs => &[Self::Auto],
            Os::Linux => &[Self::Auto, Self::Vulkan, Self::Gl, Self::Software],
        }
    }

    pub fn next_fallback(self, os: Os) -> Option<GpuChoice> {
        let chain: &[GpuChoice] = match os {
            Os::Windows | Os::Linux => &[Self::Auto, Self::Gl, Self::Software],
            Os::MacOs => &[Self::Auto],
        };
        match chain.iter().position(|c| *c == self) {
            Some(i) => chain.get(i + 1).copied(),
            None => chain.get(1).copied(),
        }
    }

    pub fn backends(self) -> wgpu::Backends {
        match self {
            Self::Auto => wgpu::Backends::PRIMARY,
            Self::Dx12 => wgpu::Backends::DX12,
            Self::Vulkan => wgpu::Backends::VULKAN,
            Self::Metal => wgpu::Backends::METAL,
            Self::Gl => wgpu::Backends::GL,
            Self::Software => wgpu::Backends::all(),
        }
    }
}

pub fn pick_adapter(types: &[wgpu::DeviceType], software: bool) -> Option<usize> {
    use wgpu::DeviceType as T;
    if software {
        return types.iter().position(|t| *t == T::Cpu);
    }
    let rank = |t: &T| match t {
        T::DiscreteGpu => 0,
        T::IntegratedGpu => 1,
        T::VirtualGpu => 2,
        T::Other => 3,
        T::Cpu => 4,
    };
    (0..types.len()).min_by_key(|&i| rank(&types[i]))
}
```

- [ ] **Step 4: Run the tests and see them pass**

Run: `cargo nextest run -p opendrape gpu::choice`
Expected: 7 passed.

- [ ] **Step 5: Failing tests for state.rs**

`crates/app/src/gpu/state.rs`:
```rust
use super::choice::{GpuChoice, Os};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Remembered between launches in `<config dir>/gpu.json`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct GpuState {
    /// The mode that last started successfully, or that the user picked.
    pub preferred: GpuChoice,
    /// Set just before the GPU starts and cleared once a frame has been drawn.
    /// Still set at the next launch means that launch crashed while starting.
    pub pending: Option<GpuChoice>,
}

impl Default for GpuState {
    fn default() -> Self {
        Self { preferred: GpuChoice::Auto, pending: None }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Reason {
    CommandLine,
    Saved,
    RecoveredFromCrash(GpuChoice),
    NoMoreFallbacks,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Decision {
    pub choice: GpuChoice,
    pub reason: Reason,
}

pub fn decide(_cli: Option<GpuChoice>, _state: &GpuState, _os: Os) -> Decision {
    todo!()
}

/// State to write before starting the GPU. `--gpu=` overrides are one-offs and leave the file alone.
pub fn pending_marker(_decision: Decision, _previous: &GpuState) -> Option<GpuState> {
    todo!()
}

/// State to write once the first frame has been drawn.
pub fn confirmed_state(_decision: Decision) -> Option<GpuState> {
    todo!()
}

/// After a start-up error that did not crash the process: try the next mode in a fresh process?
pub fn should_relaunch_after_error(_decision: Decision, _os: Os) -> bool {
    todo!()
}

/// Reads and writes `gpu.json`. I/O failures are ignored on purpose: on a locked-down
/// lab PC the app must still start, it just cannot remember anything.
#[derive(Clone, Debug)]
pub struct StateStore {
    path: Option<PathBuf>,
}

impl StateStore {
    pub fn new(dir: Option<&Path>) -> Self {
        Self { path: dir.map(|d| d.join("gpu.json")) }
    }

    pub fn default_location() -> Self {
        let dirs = directories::ProjectDirs::from("org", "OpenDrape", "OpenDrape");
        Self::new(dirs.as_ref().map(|d| d.config_dir()))
    }

    pub fn load(&self) -> GpuState {
        todo!()
    }

    pub fn save(&self, _state: &GpuState) {
        todo!()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use GpuChoice::*;

    const SAVED: Decision = Decision { choice: Auto, reason: Reason::Saved };

    #[test]
    fn command_line_wins_over_everything() {
        let state = GpuState { preferred: Dx12, pending: Some(Dx12) };
        assert_eq!(decide(Some(Gl), &state, Os::Windows), Decision { choice: Gl, reason: Reason::CommandLine });
    }

    #[test]
    fn a_crashed_start_moves_to_the_next_fallback() {
        let state = GpuState { preferred: Auto, pending: Some(Auto) };
        assert_eq!(decide(None, &state, Os::Windows), Decision { choice: Gl, reason: Reason::RecoveredFromCrash(Auto) });
    }

    #[test]
    fn when_fallbacks_run_out_the_last_choice_is_kept() {
        let state = GpuState { preferred: Auto, pending: Some(Software) };
        assert_eq!(decide(None, &state, Os::Windows), Decision { choice: Software, reason: Reason::NoMoreFallbacks });
        let mac = GpuState { preferred: Auto, pending: Some(Auto) };
        assert_eq!(decide(None, &mac, Os::MacOs), Decision { choice: Auto, reason: Reason::NoMoreFallbacks });
    }

    #[test]
    fn a_clean_start_uses_the_saved_preference() {
        let state = GpuState { preferred: Vulkan, pending: None };
        assert_eq!(decide(None, &state, Os::Linux), Decision { choice: Vulkan, reason: Reason::Saved });
    }

    #[test]
    fn markers_keep_the_old_preference_until_confirmed() {
        let previous = GpuState { preferred: Dx12, pending: None };
        let d = Decision { choice: Gl, reason: Reason::RecoveredFromCrash(Dx12) };
        assert_eq!(pending_marker(d, &previous), Some(GpuState { preferred: Dx12, pending: Some(Gl) }));
        assert_eq!(confirmed_state(d), Some(GpuState { preferred: Gl, pending: None }));
    }

    #[test]
    fn command_line_overrides_never_touch_saved_state() {
        let d = Decision { choice: Gl, reason: Reason::CommandLine };
        assert_eq!(pending_marker(d, &GpuState::default()), None);
        assert_eq!(confirmed_state(d), None);
    }

    #[test]
    fn relaunch_only_when_a_fallback_exists_and_not_for_overrides() {
        assert!(should_relaunch_after_error(SAVED, Os::Windows));
        assert!(!should_relaunch_after_error(SAVED, Os::MacOs));
        let last = Decision { choice: Software, reason: Reason::NoMoreFallbacks };
        assert!(!should_relaunch_after_error(last, Os::Windows));
        let cli = Decision { choice: Auto, reason: Reason::CommandLine };
        assert!(!should_relaunch_after_error(cli, Os::Windows));
    }

    #[test]
    fn store_round_trips() {
        let dir = tempfile::tempdir().unwrap();
        let store = StateStore::new(Some(dir.path()));
        let state = GpuState { preferred: Gl, pending: Some(Software) };
        store.save(&state);
        assert_eq!(store.load(), state);
        let json = std::fs::read_to_string(dir.path().join("gpu.json")).unwrap();
        assert!(json.contains("\"software\""), "human-readable lowercase names: {json}");
    }

    #[test]
    fn missing_or_corrupt_file_gives_defaults() {
        let dir = tempfile::tempdir().unwrap();
        let store = StateStore::new(Some(dir.path()));
        assert_eq!(store.load(), GpuState::default());
        std::fs::write(dir.path().join("gpu.json"), b"{\"preferred\": \"gl\", \"pend").unwrap();
        assert_eq!(store.load(), GpuState::default());
    }

    #[test]
    fn unwritable_or_absent_location_never_panics() {
        let dir = tempfile::tempdir().unwrap();
        let blocker = dir.path().join("blocker");
        std::fs::write(&blocker, b"a file where a directory should be").unwrap();
        let store = StateStore::new(Some(&blocker.join("sub")));
        store.save(&GpuState::default());
        assert_eq!(store.load(), GpuState::default());

        let nowhere = StateStore::new(None);
        nowhere.save(&GpuState::default());
        assert_eq!(nowhere.load(), GpuState::default());
    }
}
```

Add to `gpu/mod.rs`:
```rust
mod state;

pub use state::{
    Decision, GpuState, Reason, StateStore, confirmed_state, decide, pending_marker, should_relaunch_after_error,
};
```

- [ ] **Step 6: Run the tests and watch them fail**

Run: `cargo nextest run -p opendrape gpu::state`
Expected: 10 FAIL with `not yet implemented`.

- [ ] **Step 7: Implement state.rs**

```rust
pub fn decide(cli: Option<GpuChoice>, state: &GpuState, os: Os) -> Decision {
    if let Some(choice) = cli {
        return Decision { choice, reason: Reason::CommandLine };
    }
    if let Some(failed) = state.pending {
        return match failed.next_fallback(os) {
            Some(choice) => Decision { choice, reason: Reason::RecoveredFromCrash(failed) },
            None => Decision { choice: failed, reason: Reason::NoMoreFallbacks },
        };
    }
    Decision { choice: state.preferred, reason: Reason::Saved }
}

pub fn pending_marker(decision: Decision, previous: &GpuState) -> Option<GpuState> {
    (decision.reason != Reason::CommandLine)
        .then_some(GpuState { preferred: previous.preferred, pending: Some(decision.choice) })
}

pub fn confirmed_state(decision: Decision) -> Option<GpuState> {
    (decision.reason != Reason::CommandLine).then_some(GpuState { preferred: decision.choice, pending: None })
}

pub fn should_relaunch_after_error(decision: Decision, os: Os) -> bool {
    decision.reason != Reason::CommandLine && decision.choice.next_fallback(os).is_some()
}
```
In `impl StateStore`:
```rust
    pub fn load(&self) -> GpuState {
        self.path
            .as_ref()
            .and_then(|p| std::fs::read(p).ok())
            .and_then(|bytes| serde_json::from_slice(&bytes).ok())
            .unwrap_or_default()
    }

    /// Atomic: write a temp file, then rename, so a power cut never leaves half a file.
    pub fn save(&self, state: &GpuState) {
        let Some(path) = &self.path else { return };
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let Ok(json) = serde_json::to_vec_pretty(state) else { return };
        let tmp = path.with_extension("json.tmp");
        if std::fs::write(&tmp, json).is_ok() {
            let _ = std::fs::rename(&tmp, path);
        }
    }
```

- [ ] **Step 8: Run the tests and see them pass**

Run: `cargo nextest run -p opendrape gpu`
Expected: 17 passed.

- [ ] **Step 9: Window/GPU setup (exercised end to end in Task 5 and by `--smoke-test` in Task 7)**

`crates/app/src/gpu/setup.rs`:
```rust
use super::choice::{GpuChoice, pick_adapter};
use std::sync::Arc;

/// eframe options that start wgpu with `choice`'s backends and adapter rule.
pub fn native_options(choice: GpuChoice) -> eframe::NativeOptions {
    let mut setup = egui_wgpu::WgpuSetupCreateNew::without_display_handle();
    setup.instance_descriptor.backends = choice.backends();
    let software = choice == GpuChoice::Software;
    setup.native_adapter_selector = Some(Arc::new(
        move |adapters: &[wgpu::Adapter], _surface: Option<&wgpu::Surface<'_>>| {
            let types: Vec<_> = adapters.iter().map(|a| a.get_info().device_type).collect();
            pick_adapter(&types, software)
                .map(|i| adapters[i].clone())
                .ok_or_else(|| format!("no graphics adapter for {choice:?}"))
        },
    ));
    setup.device_descriptor = Arc::new(|adapter: &wgpu::Adapter| wgpu::DeviceDescriptor {
        label: Some("OpenDrape"),
        required_limits: wgpu::Limits::downlevel_webgl2_defaults().using_resolution(adapter.limits()),
        ..Default::default()
    });

    let mut options = eframe::NativeOptions::default();
    options.wgpu_options.wgpu_setup = egui_wgpu::WgpuSetup::CreateNew(setup);
    options.viewport = egui::ViewportBuilder::default()
        .with_title("OpenDrape")
        .with_app_id("org.opendrape.OpenDrape")
        .with_inner_size([1200.0, 800.0])
        .with_min_inner_size([640.0, 480.0]);
    options
}

/// Tell the user the graphics could not start (dialog on Windows/macOS, stderr everywhere).
pub fn show_startup_error(details: &str) {
    let body = format!(
        "OpenDrape could not start its 3D graphics. Try updating your graphics driver, \
         or start OpenDrape with --gpu=safe.\n\nDetails: {details}"
    );
    eprintln!("{body}");
    #[cfg(any(windows, target_os = "macos"))]
    {
        let _ = rfd::MessageDialog::new()
            .set_title("OpenDrape")
            .set_description(body)
            .set_level(rfd::MessageLevel::Error)
            .show();
    }
}
```
Add to `gpu/mod.rs`:
```rust
mod setup;

pub use setup::{native_options, show_startup_error};
```
(Task 4 swaps these two English strings for `tr!` messages.)

Run: `cargo clippy -p opendrape --all-targets -- -D warnings`
Expected: clean.

- [ ] **Step 10: Commit**

```bash
cargo fmt --all
git add crates/app Cargo.lock
git commit -m "feat(app): graphics backend choice, fallback chain and crash sentinel

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 4: Fluent strings (`tr!`)

**Files:**
- Create: `crates/app/i18n.toml`, `crates/app/i18n/en-US/opendrape.ftl`, `crates/app/src/i18n.rs`
- Modify: `crates/app/Cargo.toml`, `crates/app/src/lib.rs`, `crates/app/src/gpu/setup.rs`

**Interfaces:**
- Produces: `opendrape::i18n::LOADER: LazyLock<FluentLanguageLoader>` and the `tr!("id")` / `tr!("id", name = value, ...)` macro, exported at the crate root and returning `String`. Message IDs are checked at compile time against `opendrape.ftl`.

- [ ] **Step 1: Strings file and config**

Add to `crates/app/Cargo.toml` `[dependencies]`:
```toml
i18n-embed.workspace = true
i18n-embed-fl.workspace = true
rust-embed.workspace = true
```

`crates/app/i18n.toml`:
```toml
fallback_language = "en-US"

[fluent]
assets_dir = "i18n"
```

`crates/app/i18n/en-US/opendrape.ftl`:
```ftl
app-name = OpenDrape
viewport-no-gpu = The 3D view is unavailable because no graphics adapter could be started.

menu-help = Help
menu-about = About OpenDrape
menu-graphics = Graphics
graphics-auto = Automatic
graphics-dx12 = DirectX 12
graphics-vulkan = Vulkan
graphics-metal = Metal
graphics-gl = OpenGL
graphics-software = Software (safe mode, slow)
graphics-restart-note = OpenDrape restarts to switch graphics mode.

about-tagline = Free 3D garment design for students everywhere.
about-version = Version { $version }
about-graphics = Graphics: { $name } ({ $backend })
about-license = OpenDrape is free software under the GNU GPL, version 3 or later.
about-copy = Copy diagnostics
about-copied = Copied. Paste it into your bug report.

startup-failed = OpenDrape could not start its 3D graphics. Try updating your graphics driver, or start OpenDrape with --gpu=safe.

    Details: { $error }
```

- [ ] **Step 2: Failing test**

`crates/app/src/i18n.rs`:
```rust
//! User-visible strings. English (`en-US`) ships in M0; other languages are added as
//! `i18n/<lang>/opendrape.ftl` files with the same message IDs.

#[cfg(test)]
mod tests {
    #[test]
    fn placeholders_are_filled_without_bidi_isolation_marks() {
        let s = crate::tr!("about-graphics", name = "Test GPU", backend = "Metal");
        assert_eq!(s, "Graphics: Test GPU (Metal)");
        assert!(!s.contains(['\u{2068}', '\u{2069}']), "egui would draw isolation marks as boxes");
    }
}
```
Add `pub mod i18n;` to `lib.rs`.

- [ ] **Step 3: Run the test and watch it fail**

Run: `cargo nextest run -p opendrape i18n`
Expected: compile error `cannot find macro tr`.

- [ ] **Step 4: Implement the loader and macro**

Prepend to `crates/app/src/i18n.rs` (below the module doc):
```rust
use i18n_embed::fluent::{FluentLanguageLoader, fluent_language_loader};
use rust_embed::RustEmbed;
use std::sync::LazyLock;

#[derive(RustEmbed)]
#[folder = "i18n"]
struct Localizations;

pub static LOADER: LazyLock<FluentLanguageLoader> = LazyLock::new(|| {
    let loader: FluentLanguageLoader = fluent_language_loader!();
    i18n_embed::LanguageLoader::load_fallback_language(&loader, &Localizations)
        .expect("en-US strings are embedded in the binary");
    // Fluent wraps placeables in Unicode isolation marks, which egui renders as boxes.
    loader.set_use_isolating(false);
    loader
});

/// Look up a user-visible string: `tr!("menu-help")`, `tr!("about-version", version = v)`.
#[macro_export]
macro_rules! tr {
    ($id:literal) => {
        i18n_embed_fl::fl!($crate::i18n::LOADER, $id)
    };
    ($id:literal, $($args:tt)*) => {
        i18n_embed_fl::fl!($crate::i18n::LOADER, $id, $($args)*)
    };
}
```

In `gpu/setup.rs`, replace the `body` construction in `show_startup_error` with
`let body = crate::tr!("startup-failed", error = details.to_owned());` and the dialog title with `crate::tr!("app-name")`.

- [ ] **Step 5: Run the tests and see them pass**

Run: `cargo nextest run -p opendrape`
Expected: all pass (the 18 unit tests plus the Task 1 UI test).

- [ ] **Step 6: Commit**

```bash
cargo fmt --all && cargo clippy -p opendrape --all-targets -- -D warnings
git add crates/app Cargo.lock
git commit -m "feat(app): Fluent string table and tr! macro

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 5: The app: 3D viewport, menus, About and diagnostics, startup flow

**Files:**
- Create: `crates/app/build.rs`, `crates/app/src/{cli.rs,diagnostics.rs,viewport.rs,app.rs}`
- Modify: `crates/app/Cargo.toml`, `crates/app/src/lib.rs`, `crates/app/src/main.rs`, `crates/app/tests/ui.rs`

**Interfaces:**
- Consumes: `opendrape_render::{CubeRenderer, OrbitCamera, RenderTarget, target_size}` (Task 2), everything in `opendrape::gpu` (Task 3), and `tr!` (Task 4).
- Produces:
  - `Cli { gpu: Option<GpuChoice>, smoke_test: bool }` with `Cli::parse(impl IntoIterator<Item = String>)`.
  - `Diagnostics::collect(Option<&wgpu::AdapterInfo>, Decision)` and `Diagnostics::to_text()`.
  - `Startup { decision, previous: GpuState, store: StateStore, smoke_test: bool }`.
  - `Shared { restart_with: Cell<Option<GpuChoice>>, first_frame_drawn: Cell<bool> }` and `type SharedState = Rc<Shared>`.
  - `OpenDrapeApp::new(&CreationContext, Startup, SharedState)`, plus `viewport_frames() -> u64` and `request_graphics_change(GpuChoice)`.

- [ ] **Step 1: Failing unit tests for the CLI and diagnostics**

`crates/app/src/cli.rs`:
```rust
use crate::gpu::GpuChoice;

/// Command-line options. Unknown arguments are ignored: macOS may pass `-psn_…`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Cli {
    /// `--gpu=<auto|dx12|vulkan|metal|gl|safe>`: one-off graphics override.
    pub gpu: Option<GpuChoice>,
    /// `--smoke-test`: quit with success as soon as the first 3D frame is drawn.
    pub smoke_test: bool,
}

impl Cli {
    pub fn parse<I: IntoIterator<Item = String>>(_args: I) -> Self {
        todo!()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(args: &[&str]) -> Cli {
        Cli::parse(args.iter().map(|s| s.to_string()))
    }

    #[test]
    fn reads_gpu_and_smoke_test_flags() {
        assert_eq!(parse(&["--gpu=gl", "--smoke-test"]), Cli { gpu: Some(GpuChoice::Gl), smoke_test: true });
        assert_eq!(parse(&["--gpu=safe"]).gpu, Some(GpuChoice::Software));
    }

    #[test]
    fn unknown_values_and_arguments_are_ignored() {
        assert_eq!(parse(&["--gpu=banana"]), Cli::default());
        assert_eq!(parse(&["-psn_0_12345", "--verbose", "file.odp"]), Cli::default());
        assert_eq!(parse(&[]), Cli::default());
    }
}
```

`crates/app/src/diagnostics.rs`:
```rust
use crate::gpu::{Decision, Reason};

/// Facts for bug reports. Deliberately English, not translated.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Diagnostics {
    pub app_version: String,
    pub git_sha: String,
    pub os: String,
    pub adapter: String,
    pub backend: String,
    pub device_type: String,
    pub driver: String,
    pub graphics_mode: String,
}

impl Diagnostics {
    pub fn collect(_info: Option<&wgpu::AdapterInfo>, _decision: Decision) -> Self {
        todo!()
    }
    pub fn to_text(&self) -> String {
        todo!()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gpu::GpuChoice;

    #[test]
    fn text_contains_every_fact_a_bug_report_needs() {
        let d = Diagnostics {
            app_version: "0.1.0".into(),
            git_sha: "abc1234".into(),
            os: "windows x86_64".into(),
            adapter: "Intel(R) HD Graphics 520".into(),
            backend: "Gl".into(),
            device_type: "IntegratedGpu".into(),
            driver: "Intel 31.0.101".into(),
            graphics_mode: "Gl (switched after Auto failed to start)".into(),
        };
        let text = d.to_text();
        for needle in ["0.1.0", "abc1234", "windows x86_64", "HD Graphics 520", "Gl", "IntegratedGpu", "31.0.101", "switched after Auto"] {
            assert!(text.contains(needle), "missing {needle:?} in:\n{text}");
        }
    }

    #[test]
    fn collect_without_a_gpu_still_reports_version_and_mode() {
        let d = Diagnostics::collect(None, Decision { choice: GpuChoice::Auto, reason: Reason::Saved });
        assert_eq!(d.app_version, env!("CARGO_PKG_VERSION"));
        assert!(!d.git_sha.is_empty());
        assert_eq!(d.graphics_mode, "Auto (saved setting)");
        assert!(d.adapter.is_empty());
    }
}
```

`crates/app/build.rs`:
```rust
fn main() {
    // Short commit id shown in the About box and diagnostics.
    let sha = std::env::var("GITHUB_SHA")
        .ok()
        .map(|s| s.chars().take(7).collect::<String>())
        .or_else(|| {
            let out = std::process::Command::new("git").args(["rev-parse", "--short=7", "HEAD"]).output().ok()?;
            out.status.success().then(|| String::from_utf8_lossy(&out.stdout).trim().to_owned())
        })
        .unwrap_or_else(|| "unknown".to_owned());
    println!("cargo:rustc-env=OPENDRAPE_GIT_SHA={sha}");
    println!("cargo:rerun-if-env-changed=GITHUB_SHA");
    println!("cargo:rerun-if-changed=../../.git/HEAD");
}
```

Add to `lib.rs`: `pub mod cli;` and `pub mod diagnostics;`.

- [ ] **Step 2: Run the tests and watch them fail**

Run: `cargo nextest run -p opendrape cli diagnostics`
Expected: 4 FAIL with `not yet implemented`.

- [ ] **Step 3: Implement both**

`Cli::parse`:
```rust
    pub fn parse<I: IntoIterator<Item = String>>(args: I) -> Self {
        let mut cli = Self::default();
        for arg in args {
            if let Some(value) = arg.strip_prefix("--gpu=") {
                cli.gpu = GpuChoice::parse(value);
                if cli.gpu.is_none() {
                    eprintln!("OpenDrape: ignoring unknown --gpu value {value:?}");
                }
            } else if arg == "--smoke-test" {
                cli.smoke_test = true;
            }
        }
        cli
    }
```

`Diagnostics`:
```rust
    pub fn collect(info: Option<&wgpu::AdapterInfo>, decision: Decision) -> Self {
        let mut d = Self {
            app_version: env!("CARGO_PKG_VERSION").to_owned(),
            git_sha: env!("OPENDRAPE_GIT_SHA").to_owned(),
            os: format!("{} {}", std::env::consts::OS, std::env::consts::ARCH),
            graphics_mode: describe(decision),
            ..Default::default()
        };
        if let Some(info) = info {
            d.adapter = info.name.clone();
            d.backend = format!("{:?}", info.backend);
            d.device_type = format!("{:?}", info.device_type);
            d.driver = format!("{} {}", info.driver, info.driver_info).trim().to_owned();
        }
        d
    }

    pub fn to_text(&self) -> String {
        format!(
            "OpenDrape {} ({})\nOS: {}\nGraphics: {} | {} | {}\nDriver: {}\nGraphics mode: {}\n",
            self.app_version, self.git_sha, self.os, self.adapter, self.backend, self.device_type, self.driver, self.graphics_mode,
        )
    }
```
and below the impl:
```rust
fn describe(d: Decision) -> String {
    let why = match d.reason {
        Reason::CommandLine => "set on the command line".to_owned(),
        Reason::Saved => "saved setting".to_owned(),
        Reason::RecoveredFromCrash(failed) => format!("switched after {failed:?} failed to start"),
        Reason::NoMoreFallbacks => "last resort".to_owned(),
    };
    format!("{:?} ({why})", d.choice)
}
```

- [ ] **Step 4: Run the tests and see them pass**

Run: `cargo nextest run -p opendrape cli diagnostics`
Expected: 4 passed.

- [ ] **Step 5: Failing UI tests for the full app**

Add `opendrape-render.workspace = true` to `[dependencies]`.

Replace `crates/app/tests/ui.rs`:
```rust
use egui_kittest::{Harness, kittest::Queryable};
use opendrape::gpu::{Decision, GpuChoice, GpuState, Reason, StateStore};
use opendrape::{OpenDrapeApp, Shared, SharedState, Startup};
use std::{path::Path, rc::Rc};

const SAVED_AUTO: Decision = Decision { choice: GpuChoice::Auto, reason: Reason::Saved };

fn harness(config_dir: &Path, shared: SharedState) -> Harness<'static, OpenDrapeApp> {
    let startup = Startup {
        decision: SAVED_AUTO,
        previous: GpuState::default(),
        store: StateStore::new(Some(config_dir)),
        smoke_test: false,
    };
    Harness::builder()
        .with_size(egui::vec2(1000.0, 700.0))
        .wgpu()
        .build_eframe(move |cc| OpenDrapeApp::new(cc, startup, shared))
}

#[test]
fn viewport_draws_and_first_frame_clears_the_crash_marker() {
    let dir = tempfile::tempdir().unwrap();
    let store = StateStore::new(Some(dir.path()));
    // What main() writes before starting the GPU:
    store.save(&GpuState { preferred: GpuChoice::Auto, pending: Some(GpuChoice::Auto) });
    let shared = SharedState::default();
    let mut h = harness(dir.path(), shared.clone());
    h.run();
    assert!(h.state().viewport_frames() > 0, "3D viewport drew nothing");
    assert!(shared.first_frame_drawn.get());
    assert_eq!(store.load(), GpuState { preferred: GpuChoice::Auto, pending: None });
}

#[test]
fn about_box_copies_diagnostics() {
    let dir = tempfile::tempdir().unwrap();
    let mut h = harness(dir.path(), SharedState::default());
    h.run();
    h.get_by_label("Help").click();
    h.run();
    h.get_by_label("About OpenDrape").click();
    h.run();
    h.get_by_label("Copy diagnostics").click();
    h.step();
    let copied = h.output().platform_output.commands.iter().find_map(|c| match c {
        egui::OutputCommand::CopyText(text) => Some(text.clone()),
        _ => None,
    });
    let copied = copied.expect("Copy diagnostics put text on the clipboard");
    assert!(copied.contains("OpenDrape ") && copied.contains("Graphics mode: Auto"), "{copied}");
    h.run();
    h.get_by_label("Copied. Paste it into your bug report.");
}

#[test]
fn choosing_a_graphics_mode_saves_it_and_asks_for_a_restart() {
    let dir = tempfile::tempdir().unwrap();
    let shared = SharedState::default();
    let mut h = harness(dir.path(), shared.clone());
    h.run();
    h.state_mut().request_graphics_change(GpuChoice::Software);
    assert_eq!(shared.restart_with.get(), Some(GpuChoice::Software));
    assert_eq!(StateStore::new(Some(dir.path())).load(), GpuState { preferred: GpuChoice::Software, pending: None });
}

#[test]
fn tiny_window_does_not_crash() {
    let dir = tempfile::tempdir().unwrap();
    let startup = Startup {
        decision: SAVED_AUTO,
        previous: GpuState::default(),
        store: StateStore::new(Some(dir.path())),
        smoke_test: false,
    };
    let mut h = Harness::builder()
        .with_size(egui::vec2(120.0, 40.0)) // the menu bar leaves almost no room for the 3D panel
        .wgpu()
        .build_eframe(move |cc| OpenDrapeApp::new(cc, startup, Rc::new(Shared::default())));
    h.run();
}
```

- [ ] **Step 6: Run the tests and watch them fail**

Run: `cargo nextest run -p opendrape --test ui`
Expected: compile errors (`Startup`, `Shared`, `SharedState`, and the new `OpenDrapeApp::new` signature are not defined).

- [ ] **Step 7: Viewport**

`crates/app/src/viewport.rs`:
```rust
use opendrape_render::{CubeRenderer, OrbitCamera, RenderTarget, target_size};

/// The 3D panel: renders offscreen and shows the texture as an egui image.
pub struct Viewport {
    cube: CubeRenderer,
    camera: OrbitCamera,
    target: Option<(RenderTarget, egui::TextureId)>,
    pub frames_drawn: u64,
}

impl Viewport {
    pub fn new(rs: &egui_wgpu::RenderState) -> Self {
        Self { cube: CubeRenderer::new(&rs.device), camera: OrbitCamera::default(), target: None, frames_drawn: 0 }
    }

    pub fn ui(&mut self, ui: &mut egui::Ui, rs: &egui_wgpu::RenderState) {
        let size = ui.available_size();
        let max_dim = rs.device.limits().max_texture_dimension_2d;
        let Some((w, h)) = target_size(size.x, size.y, ui.pixels_per_point(), max_dim) else {
            return; // minimised or collapsed: nothing to draw
        };
        self.ensure_target(rs, w, h);
        let (target, texture_id) = self.target.as_ref().expect("ensure_target sets it");
        self.cube.render(&rs.device, &rs.queue, target, self.camera.view_proj(w as f32 / h as f32));
        self.frames_drawn += 1;

        let image = egui::Image::new(egui::load::SizedTexture::new(*texture_id, size));
        let response = ui.add(image.sense(egui::Sense::drag()));
        let drag = response.drag_delta();
        if drag != egui::Vec2::ZERO {
            self.camera.drag(drag.x, drag.y);
        }
        if response.hovered() {
            let scroll = ui.input(|i| i.smooth_scroll_delta.y);
            if scroll != 0.0 {
                self.camera.zoom(scroll);
            }
        }
    }

    /// (Re)create the render target when the panel's pixel size changes, keeping the same egui texture id.
    fn ensure_target(&mut self, rs: &egui_wgpu::RenderState, w: u32, h: u32) {
        if self.target.as_ref().is_some_and(|(t, _)| t.width == w && t.height == h) {
            return;
        }
        let target = RenderTarget::new(&rs.device, w, h);
        let mut renderer = rs.renderer.write();
        let id = match self.target.take() {
            Some((_, id)) => {
                renderer.update_egui_texture_from_wgpu_texture(&rs.device, &target.color_view, wgpu::FilterMode::Linear, id);
                id
            }
            None => renderer.register_native_texture(&rs.device, &target.color_view, wgpu::FilterMode::Linear),
        };
        self.target = Some((target, id));
    }
}
```

- [ ] **Step 8: App**

`crates/app/src/app.rs`:
```rust
use crate::diagnostics::Diagnostics;
use crate::gpu::{Decision, GpuChoice, GpuState, Os, StateStore, confirmed_state};
use crate::tr;
use crate::viewport::Viewport;
use std::{cell::Cell, rc::Rc};

/// What main() decided before the window opened.
#[derive(Clone, Debug)]
pub struct Startup {
    pub decision: Decision,
    pub previous: GpuState,
    pub store: StateStore,
    pub smoke_test: bool,
}

/// Results main() reads after the window closes.
#[derive(Debug, Default)]
pub struct Shared {
    /// The user picked another graphics mode: start a fresh process.
    pub restart_with: Cell<Option<GpuChoice>>,
    /// The GPU drew at least one 3D frame.
    pub first_frame_drawn: Cell<bool>,
}

pub type SharedState = Rc<Shared>;

pub struct OpenDrapeApp {
    viewport: Option<Viewport>,
    diagnostics: Diagnostics,
    startup: Startup,
    shared: SharedState,
    show_about: bool,
    copied: bool,
}

impl OpenDrapeApp {
    pub fn new(cc: &eframe::CreationContext<'_>, startup: Startup, shared: SharedState) -> Self {
        let render_state = cc.wgpu_render_state.as_ref();
        let info = render_state.map(|rs| rs.adapter.get_info());
        Self {
            viewport: render_state.map(Viewport::new),
            diagnostics: Diagnostics::collect(info.as_ref(), startup.decision),
            startup,
            shared,
            show_about: false,
            copied: false,
        }
    }

    pub fn viewport_frames(&self) -> u64 {
        self.viewport.as_ref().map_or(0, |v| v.frames_drawn)
    }

    /// Save `choice` as the preferred graphics mode and ask main() to restart.
    pub fn request_graphics_change(&mut self, choice: GpuChoice) {
        self.startup.store.save(&GpuState { preferred: choice, pending: None });
        self.shared.restart_with.set(Some(choice));
    }

    /// The first 3D frame is proof this graphics mode works: clear the crash marker.
    fn confirm_first_frame(&mut self, ctx: &egui::Context) {
        if self.shared.first_frame_drawn.get() || self.viewport_frames() == 0 {
            return;
        }
        self.shared.first_frame_drawn.set(true);
        if let Some(state) = confirmed_state(self.startup.decision) {
            self.startup.store.save(&state);
        }
        if self.startup.smoke_test {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
    }

    fn menu_bar(&mut self, ui: &mut egui::Ui) {
        egui::MenuBar::new().ui(ui, |ui| {
            ui.menu_button(tr!("menu-help"), |ui| {
                if ui.button(tr!("menu-about")).clicked() {
                    self.show_about = true;
                    self.copied = false;
                    ui.close();
                }
                ui.menu_button(tr!("menu-graphics"), |ui| {
                    for &choice in GpuChoice::available(Os::current()) {
                        let current = self.startup.decision.choice == choice;
                        if ui.radio(current, choice_label(choice)).clicked() && !current {
                            self.request_graphics_change(choice);
                            ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
                        }
                    }
                    ui.separator();
                    ui.label(tr!("graphics-restart-note"));
                });
            });
        });
    }

    fn about_window(&mut self, ctx: &egui::Context) {
        let mut open = self.show_about;
        egui::Window::new(tr!("menu-about"))
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .show(ctx, |ui| {
                let d = &self.diagnostics;
                ui.heading(tr!("app-name"));
                ui.label(tr!("about-tagline"));
                ui.label(tr!("about-version", version = d.app_version.clone()));
                ui.label(tr!("about-graphics", name = d.adapter.clone(), backend = d.backend.clone()));
                ui.label(tr!("about-license"));
                if ui.button(tr!("about-copy")).clicked() {
                    ctx.copy_text(d.to_text());
                    self.copied = true;
                }
                if self.copied {
                    ui.label(tr!("about-copied"));
                }
            });
        self.show_about = open;
    }
}

fn choice_label(choice: GpuChoice) -> String {
    match choice {
        GpuChoice::Auto => tr!("graphics-auto"),
        GpuChoice::Dx12 => tr!("graphics-dx12"),
        GpuChoice::Vulkan => tr!("graphics-vulkan"),
        GpuChoice::Metal => tr!("graphics-metal"),
        GpuChoice::Gl => tr!("graphics-gl"),
        GpuChoice::Software => tr!("graphics-software"),
    }
}

impl eframe::App for OpenDrapeApp {
    fn ui(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame) {
        egui::Panel::top("menu_bar").show(ui, |ui| self.menu_bar(ui));
        self.about_window(ui.ctx());
        egui::CentralPanel::default().show(ui, |ui| match (self.viewport.as_mut(), frame.wgpu_render_state()) {
            (Some(viewport), Some(rs)) => viewport.ui(ui, rs),
            _ => {
                ui.centered_and_justified(|ui| ui.label(tr!("viewport-no-gpu")));
            }
        });
        self.confirm_first_frame(ui.ctx());
    }
}
```

`crates/app/src/lib.rs`:
```rust
//! OpenDrape desktop application.

mod app;
pub mod cli;
pub mod diagnostics;
pub mod gpu;
#[doc(hidden)]
pub mod i18n;
mod viewport;

pub use app::{OpenDrapeApp, Shared, SharedState, Startup};
```

- [ ] **Step 9: Startup flow in main**

`crates/app/src/main.rs`:
```rust
// No console window behind the app in Windows release builds.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use opendrape::cli::Cli;
use opendrape::gpu::{self, Os, StateStore};
use opendrape::{OpenDrapeApp, SharedState, Startup};
use std::process::ExitCode;

fn main() -> ExitCode {
    let os = Os::current();
    let cli = Cli::parse(std::env::args().skip(1));
    let store = StateStore::default_location();
    let previous = store.load();
    let decision = gpu::decide(cli.gpu, &previous, os);
    if let Some(marker) = gpu::pending_marker(decision, &previous) {
        store.save(&marker);
    }

    let shared = SharedState::default();
    let startup = Startup { decision, previous, store, smoke_test: cli.smoke_test };
    let app_shared = shared.clone();
    let result = eframe::run_native(
        "OpenDrape",
        gpu::native_options(decision.choice),
        Box::new(move |cc| Ok(Box::new(OpenDrapeApp::new(cc, startup, app_shared)))),
    );

    match result {
        Ok(()) if cli.smoke_test => {
            if shared.first_frame_drawn.get() { ExitCode::SUCCESS } else { ExitCode::FAILURE }
        }
        Ok(()) => {
            if shared.restart_with.get().is_some() {
                relaunch();
            }
            ExitCode::SUCCESS
        }
        Err(err) => {
            // The GPU or window failed without crashing. The pending marker is still on
            // disk, so a fresh process moves on to the next graphics mode.
            if !cli.smoke_test && gpu::should_relaunch_after_error(decision, os) {
                relaunch();
                return ExitCode::SUCCESS;
            }
            gpu::show_startup_error(&err.to_string());
            ExitCode::FAILURE
        }
    }
}

/// Start a fresh copy of OpenDrape. `--gpu=` is dropped so the saved state decides.
fn relaunch() {
    if let Ok(exe) = std::env::current_exe() {
        let args = std::env::args().skip(1).filter(|a| !a.starts_with("--gpu="));
        let _ = std::process::Command::new(exe).args(args).spawn();
    }
}
```

- [ ] **Step 10: Run all tests and see them pass**

Run: `cargo nextest run --workspace`
Expected: all pass (render 10, app unit 22, app UI 4). If a compile error points at an egui 0.36 method name (`ui.close`, `smooth_scroll_delta`, `Window::show`), check it with `grep -n "pub fn <name>" ~/.cargo/registry/src/*/egui-0.36.2/src -r` and fix it. All three were confirmed to exist on 2026-10-09.

- [ ] **Step 11: Run the real app by hand and with the smoke test**

Run: `cargo run -p opendrape -- --smoke-test; echo "exit=$?"`
Expected: a window flashes up and closes, then `exit=0`.

Run: `cargo run -p opendrape` and check:
- the cube appears and dragging rotates it;
- Help → About shows "Graphics: Apple … (Metal)";
- Copy diagnostics works.

Close the window. Then `cat ~/Library/Application\ Support/org.OpenDrape.OpenDrape/gpu.json` (the macOS location from `directories`) shows `"pending": null`.

- [ ] **Step 12: Commit**

```bash
cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings
git add crates Cargo.lock
git commit -m "feat(app): 3D viewport, Help menu, About with diagnostics, graphics fallback start-up

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 6: Icons, window icon, Windows exe icon, packaging scripts

**Files:**
- Create: `xtask/Cargo.toml`, `xtask/src/main.rs`, `assets/icon@2x.png`, `assets/icon.ico`, `scripts/package-macos.sh`, `scripts/package-windows.ps1`, `docs/PORTABLE.txt`
- Modify: `Cargo.toml` (members), `crates/app/Cargo.toml` (packager metadata, build-dependency), `crates/app/build.rs`, `crates/app/src/gpu/setup.rs`

**Interfaces:**
- Consumes: the `opendrape` binary (Task 5).
- Produces:
  - `cargo xtask icons`;
  - `scripts/package-macos.sh [suffix]` → `dist/OpenDrape-<suffix>-macos-universal.dmg`;
  - `scripts/package-windows.ps1 [suffix]` → `dist/OpenDrape-<suffix>-windows-x64-setup.exe` and `…-windows-x64-portable.zip`.

- [ ] **Step 1: Icon generator**

Add `"xtask"` to workspace `members`.

`xtask/Cargo.toml`:
```toml
[package]
name = "xtask"
description = "OpenDrape developer tasks"
version.workspace = true
edition.workspace = true
license.workspace = true
publish.workspace = true

[dependencies]
image.workspace = true

[lints]
workspace = true
```

`xtask/src/main.rs`:
```rust
//! Developer tasks: `cargo xtask icons` regenerates the app icons in `assets/`.

fn main() {
    match std::env::args().nth(1).as_deref() {
        Some("icons") => icons(),
        _ => {
            eprintln!("usage: cargo xtask icons");
            std::process::exit(2);
        }
    }
}

/// A white A-line dress on a rounded terracotta square, 4×4 supersampled.
fn icons() {
    const S: u32 = 1024;
    let dress = [
        (430.0, 200.0), (470.0, 250.0), (554.0, 250.0), (594.0, 200.0),
        (640.0, 230.0), (610.0, 430.0), (780.0, 840.0), (244.0, 840.0),
        (414.0, 430.0), (384.0, 230.0),
    ];
    let img = image::RgbaImage::from_fn(S, S, |px, py| {
        let (mut bg, mut fg) = (0u32, 0u32);
        for sy in 0..4 {
            for sx in 0..4 {
                let x = px as f32 + (sx as f32 + 0.5) / 4.0;
                let y = py as f32 + (sy as f32 + 0.5) / 4.0;
                let (radius, margin) = (180.0f32, 64.0f32);
                let half = S as f32 / 2.0;
                let dx = (x - half).abs() - (half - margin - radius);
                let dy = (y - half).abs() - (half - margin - radius);
                if (dx.max(0.0).powi(2) + dy.max(0.0).powi(2)).sqrt() <= radius {
                    if inside(&dress, x, y) { fg += 1 } else { bg += 1 }
                }
            }
        }
        let t = fg as f32 / (bg + fg).max(1) as f32;
        let mix = |a: f32, b: f32| (a * (1.0 - t) + b * t) as u8;
        image::Rgba([mix(196.0, 255.0), mix(92.0, 255.0), mix(56.0, 255.0), ((bg + fg) * 255 / 16) as u8])
    });
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../assets");
    std::fs::create_dir_all(&root).expect("create assets/");
    img.save(root.join("icon@2x.png")).expect("write icon@2x.png");
    image::DynamicImage::ImageRgba8(img)
        .resize_exact(256, 256, image::imageops::FilterType::Lanczos3)
        .save(root.join("icon.ico"))
        .expect("write icon.ico");
    println!("wrote assets/icon@2x.png and assets/icon.ico");
}

/// Even-odd point-in-polygon test.
fn inside(poly: &[(f32, f32)], x: f32, y: f32) -> bool {
    let mut c = false;
    for i in 0..poly.len() {
        let (xi, yi) = poly[i];
        let (xj, yj) = poly[(i + poly.len() - 1) % poly.len()];
        if (yi > y) != (yj > y) && x < (xj - xi) * (y - yi) / (yj - yi) + xi {
            c = !c;
        }
    }
    c
}
```
(The `@2x` suffix matters: cargo-packager reads a 1024 px PNG as 512 pt @2x. A plain `icon.png` fails with "No matching IconType".)

Run: `cargo xtask icons`
Expected: `wrote assets/icon@2x.png and assets/icon.ico`. Open `assets/icon@2x.png` with the Read tool: a white dress on a terracotta rounded square.

- [ ] **Step 2: Use the icons in the app**

In `gpu/setup.rs`, add to the `ViewportBuilder` chain:
```rust
        .with_icon(
            eframe::icon_data::from_png_bytes(include_bytes!("../../../../assets/icon@2x.png"))
                .unwrap_or_default(),
        )
```

Windows `.exe` icon. In `crates/app/Cargo.toml`:
```toml
[target.'cfg(windows)'.build-dependencies]
winresource = "0.1"
```
Append to `main()` in `crates/app/build.rs`:
```rust
    println!("cargo:rerun-if-changed=../../assets/icon.ico");
    #[cfg(windows)]
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let mut res = winresource::WindowsResource::new();
        res.set_icon("../../assets/icon.ico");
        res.compile().expect("embed the Windows .exe icon");
    }
```

Packager config, appended to `crates/app/Cargo.toml` (paths are relative to `crates/app`, because cargo-packager switches into the manifest's directory):
```toml
[package.metadata.packager]
productName = "OpenDrape"
identifier = "org.opendrape.OpenDrape"
publisher = "OpenDrape contributors"
copyright = "OpenDrape contributors, GPL-3.0-or-later"
category = "GraphicsAndDesign"
icons = ["../../assets/icon@2x.png"]
licenseFile = "../../LICENSE"
binaries = [{ path = "opendrape", main = true }]
macos = { minimumSystemVersion = "11.0" }
nsis = { installMode = "currentUser", installerIcon = "../../assets/icon.ico" }
```

- [ ] **Step 3: macOS packaging script**

`scripts/package-macos.sh`:
```bash
#!/usr/bin/env bash
# Builds a universal (Apple Silicon + Intel) OpenDrape.app and a drag-to-install DMG.
# Usage: scripts/package-macos.sh [name-suffix]   → dist/OpenDrape-<suffix>-macos-universal.dmg
set -euo pipefail
cd "$(dirname "$0")/.."

VERSION=$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)
SUFFIX="${1:-$VERSION}"
UNIVERSAL=target/universal-apple-darwin/release

rustup target add aarch64-apple-darwin x86_64-apple-darwin
cargo build --release -p opendrape --target aarch64-apple-darwin
cargo build --release -p opendrape --target x86_64-apple-darwin
mkdir -p "$UNIVERSAL"
lipo -create -output "$UNIVERSAL/opendrape" \
  target/aarch64-apple-darwin/release/opendrape \
  target/x86_64-apple-darwin/release/opendrape

cargo packager --release -p opendrape --target universal-apple-darwin --formats app
APP="$UNIVERSAL/OpenDrape.app"
# Ad-hoc signature: without any signature, Apple Silicon reports a downloaded app as
# "damaged" instead of offering "Open Anyway".
codesign --force --deep --sign - "$APP"

# Plain hdiutil instead of the packager's DMG step, which scripts Finder and can time out.
STAGE=$(mktemp -d)
cp -R "$APP" "$STAGE/"
ln -s /Applications "$STAGE/Applications"
mkdir -p dist
DMG="dist/OpenDrape-$SUFFIX-macos-universal.dmg"
hdiutil create -quiet -volname OpenDrape -srcfolder "$STAGE" -ov -format UDZO "$DMG"
rm -rf "$STAGE"
echo "$DMG"
```
`chmod +x scripts/package-macos.sh`

- [ ] **Step 4: Windows packaging script**

`scripts/package-windows.ps1`:
```powershell
# Builds the OpenDrape Windows installer (per-user, no admin rights) and a portable ZIP.
# Usage: scripts/package-windows.ps1 [name-suffix]
$ErrorActionPreference = 'Stop'
Set-Location (Join-Path $PSScriptRoot '..')

$version = (Select-String -Path Cargo.toml -Pattern '^version = "(.*)"' | Select-Object -First 1).Matches[0].Groups[1].Value
$suffix = if ($args.Count -gt 0) { $args[0] } else { $version }

cargo build --release -p opendrape
if ($LASTEXITCODE -ne 0) { throw "cargo build failed" }
cargo packager --release -p opendrape --formats nsis
if ($LASTEXITCODE -ne 0) { throw "cargo packager failed" }

New-Item -ItemType Directory -Force dist | Out-Null
$installer = Get-ChildItem target/release -Recurse -Filter '*-setup.exe' | Sort-Object LastWriteTime | Select-Object -Last 1
Copy-Item $installer.FullName "dist/OpenDrape-$suffix-windows-x64-setup.exe"

$portable = 'target/portable/OpenDrape'
Remove-Item -Recurse -Force $portable -ErrorAction SilentlyContinue
New-Item -ItemType Directory -Force $portable | Out-Null
Copy-Item target/release/opendrape.exe "$portable/OpenDrape.exe"
Copy-Item LICENSE, docs/PORTABLE.txt $portable
Compress-Archive -Path $portable -DestinationPath "dist/OpenDrape-$suffix-windows-x64-portable.zip" -Force
Get-ChildItem dist
```

`docs/PORTABLE.txt`:
```
OpenDrape (portable)

Double-click OpenDrape.exe to start. Nothing is installed; delete the folder to remove it.
OpenDrape remembers its graphics setting in %APPDATA%\OpenDrape.

If Windows shows "Windows protected your PC", click "More info", then "Run anyway".
If the 3D view does not appear, use Help > Graphics > Software (safe mode, slow).

OpenDrape is free software under the GNU GPL v3 or later (see LICENSE).
```

- [ ] **Step 5: Package locally and verify the DMG**

Run: `./scripts/package-macos.sh dev`
Expected: ends by printing `dist/OpenDrape-dev-macos-universal.dmg`, about 12 MB.

Then verify:
```bash
lipo -archs target/universal-apple-darwin/release/OpenDrape.app/Contents/MacOS/opendrape   # → x86_64 arm64
codesign -dv target/universal-apple-darwin/release/OpenDrape.app 2>&1 | grep Signature      # → Signature=adhoc
plutil -p target/universal-apple-darwin/release/OpenDrape.app/Contents/Info.plist | grep -E 'Identifier|MinimumSystem'
target/universal-apple-darwin/release/OpenDrape.app/Contents/MacOS/opendrape --smoke-test; echo "exit=$?"   # → exit=0
```
Expected output, in order:
- `x86_64 arm64`;
- `Signature=adhoc`;
- identifier `org.opendrape.OpenDrape` and minimum system version `11.0`;
- `exit=0`.

- [ ] **Step 6: Commit**

Update the icon row in `ASSETS.md` if anything changed, then:
```bash
cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings && cargo deny check
git add Cargo.toml Cargo.lock xtask assets crates/app scripts docs/PORTABLE.txt ASSETS.md
git commit -m "build: app icons, macOS universal DMG and Windows installer/portable packaging

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 7: Release workflow and the user's test checklist

**Files:**
- Create: `.github/workflows/release.yml`, `docs/testing/M0-checklist.md`, `docs/testing/nightly-notes.md`

**Interfaces:**
- Consumes: `scripts/package-macos.sh`, `scripts/package-windows.ps1`, `--smoke-test` (Tasks 5–6).
- Produces: a GitHub pre-release `nightly` rebuilt on every push to `main`, and a release for every `v*` tag.

- [ ] **Step 1: Release workflow**

`.github/workflows/release.yml`:
```yaml
name: Release

on:
  push:
    branches: [main]
    tags: ["v*"]
  workflow_dispatch:

permissions:
  contents: write

concurrency:
  group: release-${{ github.ref }}
  cancel-in-progress: true

jobs:
  macos:
    runs-on: macos-15
    steps:
      - uses: actions/checkout@v5
      - run: rustup show
      - uses: Swatinem/rust-cache@v2
      - run: cargo install --locked cargo-packager@0.11.8
      - id: name
        run: |
          if [ "$GITHUB_REF_TYPE" = tag ]; then echo "suffix=${GITHUB_REF_NAME#v}" >> "$GITHUB_OUTPUT"
          else echo "suffix=nightly-${GITHUB_SHA::7}" >> "$GITHUB_OUTPUT"; fi
      - run: ./scripts/package-macos.sh "${{ steps.name.outputs.suffix }}"
      - name: Smoke test (opens a window, draws one 3D frame, quits)
        run: target/universal-apple-darwin/release/OpenDrape.app/Contents/MacOS/opendrape --smoke-test
        timeout-minutes: 2
      - uses: actions/upload-artifact@v4
        with:
          name: macos
          path: dist/*

  windows:
    runs-on: windows-2025
    steps:
      - uses: actions/checkout@v5
      - run: rustup show
      - uses: Swatinem/rust-cache@v2
      - run: cargo install --locked cargo-packager@0.11.8
      - id: name
        shell: bash
        run: |
          if [ "$GITHUB_REF_TYPE" = tag ]; then echo "suffix=${GITHUB_REF_NAME#v}" >> "$GITHUB_OUTPUT"
          else echo "suffix=nightly-${GITHUB_SHA::7}" >> "$GITHUB_OUTPUT"; fi
      - run: ./scripts/package-windows.ps1 "${{ steps.name.outputs.suffix }}"
        shell: pwsh
      - name: Smoke test (best effort, CI desktops may not allow windows)
        continue-on-error: true
        timeout-minutes: 2
        shell: pwsh
        run: |
          $p = Start-Process target/release/opendrape.exe -ArgumentList '--smoke-test' -PassThru -Wait
          "exit code: $($p.ExitCode)"
          exit $p.ExitCode
      - uses: actions/upload-artifact@v4
        with:
          name: windows
          path: dist/*

  publish:
    needs: [macos, windows]
    runs-on: ubuntu-24.04
    env:
      GH_TOKEN: ${{ github.token }}
    steps:
      - uses: actions/checkout@v5
      - uses: actions/download-artifact@v4
        with:
          path: dist
          merge-multiple: true
      - name: Publish nightly pre-release
        if: github.ref_type == 'branch'
        run: |
          gh release delete nightly --yes --cleanup-tag || true
          gh release create nightly dist/* --prerelease --target "$GITHUB_SHA" \
            --title "Nightly build (${GITHUB_SHA::7})" --notes-file docs/testing/nightly-notes.md
      - name: Publish tagged release
        if: github.ref_type == 'tag'
        run: |
          gh release create "$GITHUB_REF_NAME" dist/* --title "OpenDrape $GITHUB_REF_NAME" --generate-notes
```

- [ ] **Step 2: Release notes and the plain-language checklist**

`docs/testing/nightly-notes.md`:
```markdown
Automatic build of the latest OpenDrape code. It may be unstable.

- **Mac:** `…-macos-universal.dmg` (Apple Silicon and Intel, macOS 11+)
- **Windows:** `…-windows-x64-setup.exe` (installs without admin rights) or `…-windows-x64-portable.zip`

First-time install help and what to test: see `docs/testing/` in the repository.
```

`docs/testing/M0-checklist.md`:
```markdown
# OpenDrape M0: what to try

This first build only checks the foundations: the app installs, opens, and draws 3D on
your computer. There is no garment yet, only a test cube.

## Install on a Mac

1. Go to the project's **Releases** page and open **Nightly build**.
2. Download the file ending in `macos-universal.dmg` and double-click it.
3. Drag **OpenDrape** onto the **Applications** folder in the window that opens.
4. Open your Applications folder and double-click **OpenDrape**. macOS says it cannot
   check the developer. Click **Done**. (The app is not yet signed with a paid Apple
   certificate.)
5. Open **System Settings → Privacy & Security**, scroll down to the message about
   OpenDrape, click **Open Anyway**, enter your password, then click **Open**.
   You only do this once.

## Check these

- [ ] A window called **OpenDrape** opens with an orange-brown cube in a light grey area.
- [ ] Dragging on the cube turns it. Scrolling zooms in and out.
- [ ] The cube's top looks lighter than its sides (it is lit from above).
- [ ] **Help → About OpenDrape** shows your graphics, e.g. "Graphics: Apple M4 Max (Metal)".
- [ ] Click **Copy diagnostics**, then paste into Notes: you see the version, OS and graphics lines.
- [ ] **Help → Graphics** shows **Automatic** selected.
- [ ] Make the window very small, then very large: nothing breaks.
- [ ] Quit (Cmd+Q) and open it again: it opens normally.

If anything fails, write down what you did and what happened, and paste the copied
diagnostics with it.

## For Windows testers

Download `…-windows-x64-setup.exe` and run it. If Windows says "Windows protected your
PC", click **More info → Run anyway**. It installs without asking for an administrator.
Or download the `portable.zip`, unzip it anywhere, and run `OpenDrape.exe`.

Also check:
- [ ] **Help → Graphics → Software (safe mode, slow)**: OpenDrape restarts and still shows the cube.
- [ ] **Help → Graphics → Automatic**: it restarts back to normal.
```

- [ ] **Step 3: Validate the workflow files**

Run: `ruby -ryaml -e 'ARGV.each { |f| YAML.load_file(f); puts "ok #{f}" }' .github/workflows/*.yml`
Expected: `ok` for both files. (Ruby ships with macOS.) If `actionlint` is installed (`brew install actionlint`), also run `actionlint` and expect no output.

- [ ] **Step 4: Commit**

```bash
git add .github/workflows/release.yml docs/testing
git commit -m "ci: nightly and tagged releases with smoke tests; M0 tester checklist

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 8: Publish to GitHub and get CI green (needs the user's OK)

**Files:**
- Modify: whatever CI failures point to; `Cargo.toml` `repository` if the repo name changes.

**Interfaces:**
- Consumes: everything above.
- Produces: public repo `pujarivivek/opendrape`, green CI on three OSes, and a `nightly` release with the DMG, setup.exe and portable ZIP.

- [ ] **Step 1: STOP and ask the user**

Creating a public repository publishes the code. Ask the user to confirm two things:
- the name `pujarivivek/opendrape`;
- that it should be public, since GPL projects are normally public and grants such as FLOSS/fund require it.

Do not continue without a clear yes. If they choose another name, update `repository` in `Cargo.toml` and the links in `README.md`, then commit.

- [ ] **Step 2: Create the repo and push**

```bash
gh repo create pujarivivek/opendrape --public --source . --remote origin \
  --description "Free, open-source 3D garment design for fashion students (GPL-3.0)" --push
```

- [ ] **Step 3: Watch both workflows**

Run: `gh run list --limit 5` and then `gh run watch <id> --exit-status` for the CI run and the Release run (in the background).
Expected: CI green on ubuntu-24.04, windows-2025 and macos-15. Release green, with the Windows smoke test allowed to fail.

Known failure modes to check first (use superpowers:systematic-debugging):
- **Linux UI tests can't find an adapter.** Confirm `mesa-vulkan-drivers` installed, then run with `WGPU_BACKEND=vulkan`.
- **Windows UI tests need a software adapter.** If kittest's default setup finds none, give `Harness::builder()` an explicit `.wgpu_setup(...)` whose `native_adapter_selector` uses `opendrape::gpu::pick_adapter`.
- **The Windows `winresource` icon step fails.** Check that `rc.exe` from the Windows SDK is on the runner image.

- [ ] **Step 4: Verify the nightly release**

Run: `gh release view nightly --json assets --jq '.assets[].name'`
Expected: three files, ending `-macos-universal.dmg`, `-windows-x64-setup.exe` and `-windows-x64-portable.zip`.

- [ ] **Step 5: Hand over to the user**

Send the user:
- the link to the nightly release;
- `docs/testing/M0-checklist.md` in plain words.

Ask them to run the Mac checklist and report back. Then note in `docs/specs/2026-10-09-opendrape-design.md` that M0 is complete, and commit.

---

## Self-Review Notes

**Spec coverage (M0 row):**

| M0 item | Task |
|---|---|
| Rust toolchain (done 2026-10-09) | — |
| git repo and GitHub repo (GPL-3.0, README, ASSETS.md) | Tasks 1, 8 |
| Workspace | Task 1 |
| CI (fmt, clippy, nextest, cargo-deny) | Task 1 |
| Installers + portable ZIP | Task 6 |
| Shaded cube in the 3D panel | Tasks 2, 5 |
| Backend selector + crash sentinel | Tasks 3, 5 |
| About + Copy diagnostics | Task 5 |
| Fluent scaffold | Task 4 |
| Nightly releases | Task 7 |

**Spec items deliberately deferred:**
- Linux AppImage → M8.
- Golden-image tests → M1.
- Noto fonts → after 1.0.
- `macos-15-intel` nightly test job → added in M1, once there is simulation code worth testing on x86.

**Type consistency:** `Decision`, `GpuState`, `StateStore`, `Startup`, `Shared` and `SharedState` are spelled the same in Tasks 3, 5 and 8. `target_size(width, height, ppp, max_dim)` has the same argument order in Tasks 2 and 5.
