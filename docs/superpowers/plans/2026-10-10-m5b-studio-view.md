# M5b Studio View Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the 3D view's flat renderer with a studio renderer:
- soft grey studio, with a CC0 HDRI baked to SH9 plus a soft key light;
- colour-accurate output (PBR Neutral tone mapping, exact sRGB);
- cloth sheen and wrap;
- soft key and floor contact shadows;
- SSAO;
- FXAA while moving and jittered supersampling when still;
- quality levels, chosen automatically and settable in View → 3D quality.

**Architecture:**
- **`StudioRenderer`:** a new module in `crates/render/src/studio/`. It owns its meshes (`StudioMesh` with a `Material`), intermediate targets and passes. It renders into the existing `RenderTarget` (`Rgba8Unorm`, display-encoded) that egui shows.
- **Moving or still:** it decides this itself, by comparing a frame signature. It skips GPU work once a still image has finished accumulating.
- **The environment:** baked offline by `cargo xtask studio` into a committed Rust file.

**Tech Stack:** Rust 2024, wgpu 30 (WebGL2-class downlevel limits, fragment passes only), WGSL, egui 0.36, `image` (with `hdr` in xtask only).

**Spec:** `docs/superpowers/specs/2026-10-10-m5b-studio-view-design.md`

**Execution (user preference):** native, in this session, with one fresh whole-branch review at the end. Commit before any mutation check. Ask before launching the app and before any push or merge. The HDRI download was approved (2026-10-10).

## Global Constraints
- **GPU limits:** stay within `wgpu::Limits::downlevel_webgl2_defaults()`:
  - no compute, no storage buffers or textures;
  - ≤ 2 colour attachments per pass;
  - textures ≤ 2048;
  - ≤ 4 bind groups.
- **No MSAA.**
- **Formats:**
  - **HDR targets:** `Rgba16Float` only when `adapter.get_texture_format_features(Rgba16Float).allowed_usages` contains `RENDER_ATTACHMENT | TEXTURE_BINDING`. Otherwise the LDR path (`Rgba8Unorm`).
  - **Depth:** `Depth32Float` (sampled with `textureLoad`, or with a comparison sampler for shadows).
  - **AO:** `Rgba8Unorm` (r = AO; g, b = 16-bit packed linear depth).
  - **Floor contact map:** `R8Unorm`.
- **Final output:** the existing `RenderTarget.color` (`Rgba8Unorm`, display-encoded sRGB).
- **Colour:** mesh colours passed to the renderer are linear RGB. Tone mapping is Khronos PBR Neutral; sRGB encoding uses the exact piecewise curve.
- **The studio HDRI:** Poly Haven "Studio Small 08" 1k, from
  `https://dl.polyhaven.org/file/ph-assets/HDRIs/hdr/1k/studio_small_08_1k.hdr` (1,508,872 bytes, MD5
  `de3ba64222895aca876b1d1c2e0cf81a`). Never committed; only the baked `environment_data.rs` is.
- **The baked environment:**
  - **Key-light share:** 40 % of the L0 irradiance.
  - **White balance:** L0 is made neutral grey.
  - **Key direction:** the key is turned to azimuth +40° (from +Z towards +X), with the HDRI's own elevation clamped to 25°–60°.
- **Quality levels:**

  | Level | AO while moving | Key shadows while moving | FXAA while moving | Render cap | Still frames |
  |---|---|---|---|---|---|
  | **Basic** | none | none | none | 1 Mpx | 16 |
  | **Medium** | half resolution, 8 samples | 1024, 8 taps | yes | 2 Mpx | 16 |
  | **High** | half resolution, 12 samples | 2048, 12 taps | yes | 4 Mpx | 32 |

  - When still, AO and key shadows are always on: Basic gets half-resolution AO with 8 samples and 1024 shadows; High gets full-resolution AO.
  - The floor contact map is on everywhere: 256² on Basic, 512² otherwise.
- **Defaults and sizes (pinned in `studio::look` as named constants):**
  - **Form colour:** linear of sRGB(188, 168, 153).
  - **AO:** radius 0.06 m, applied to ambient only, with GTAO multi-bounce.
  - **Lining:** cloth back faces use the flipped normal and albedo × 0.8.
  - **Floor:** disc radius 8 m; fades to the backdrop between 3 m and 7 m.
  - **Floor contact map:** covers 2.4 m × 2.4 m, height range 0.6 m, opacity 0.6.
  - **Key shadow box:** 2.6 m square centred on (0, 0.95, 0).
- All UI strings go through `tr!`. The 3D quality choice is saved as `view.json` beside `gpu.json`.

## Review Focus
1. **GL or llvmpipe without float render targets:** the LDR path must render without wgpu validation errors, and Basic must be chosen.
2. **Panel resizes during accumulation, including to 1×1:** targets are re-made and accumulation restarts, with no validation error and no stale-size reads.
3. **Selecting a piece while still:** the colour change must restart the frame, not leave the old colour on screen.
4. **A sim that auto-pauses after settling:** the view must reach "still done" and stop asking egui to repaint, so the CPU and GPU go idle.
5. **Thin two-sided cloth in the key shadow map:** no shadow acne stripes on cloth facing the light (normal-offset and slope bias).

---

### Task 1: Colour science on the CPU (`crates/render/src/colour.rs`)

**Files:** Create `crates/render/src/colour.rs`; modify `crates/render/src/lib.rs` (`pub mod colour;`).

**Produces:**
- `pub fn srgb_to_linear(c: f32) -> f32` and `pub fn linear_to_srgb(c: f32) -> f32` (exact piecewise curve).
- `pub fn srgb8_to_linear(rgb: [u8; 3]) -> [f32; 3]`.
- `pub fn pbr_neutral(rgb: [f32; 3]) -> [f32; 3]` (Khronos reference).
- `pub fn delta_e2000(a: [u8; 3], b: [u8; 3]) -> f32`: sRGB8, D65, via XYZ → Lab.

**Steps:**
- [ ] **Step 1: Failing tests.**
  - `srgb_round_trips`: for every u8, `linear_to_srgb(srgb_to_linear(x/255))*255` rounds back to x.
  - `pbr_neutral_keeps_mid_colours_and_compresses_bright_ones`:
    - `[0.5, 0.3, 0.2]` maps to itself minus 0.04 (since min ≥ 0.08), within 1e-6;
    - `[2.0, 1.0, 0.5]` stays below 1.0 and keeps its channel order;
    - black stays black.
  - `delta_e2000_matches_sharma_pairs`: Lab inputs from Sharma, Wu & Dalal (2005) pairs 1, 7 and 17 (ΔE 2.0425, 0.0 and 27.1492), via an internal `delta_e2000_lab(l1: [f64;3], l2: [f64;3])`, within 1e-3. Also `delta_e2000([118;3],[118;3]) == 0`.
- [ ] **Step 2:** run `cargo nextest run -p opendrape-render --lib colour`. Expect a compile failure.
- [ ] **Step 3: Implement.** PBR Neutral, verbatim from the KhronosGroup/ToneMapping reference:
  ```rust
  const START: f32 = 0.8 - 0.04; const DESAT: f32 = 0.15;
  let x = r.min(g).min(b); let offset = if x < 0.08 { x - 6.25 * x * x } else { 0.04 };
  let c = [r - offset, g - offset, b - offset]; let peak = c[0].max(c[1]).max(c[2]);
  if peak < START { return c; }
  let d = 1.0 - START; let new_peak = 1.0 - d * d / (peak + d - START);
  let c = c.map(|v| v * new_peak / peak); let g = 1.0 - 1.0 / (DESAT * (peak - new_peak) + 1.0);
  c.map(|v| v * (1.0 - g) + new_peak * g)
  ```
  ΔE2000 follows Sharma's formula: lightness, chroma and hue weights k = 1, with the rotation term.
- [ ] **Step 4:** run the tests and expect them to pass.
- [ ] **Step 5:** commit `feat(render): colour science on the CPU: sRGB, PBR Neutral, ΔE2000`.

### Task 2: The studio environment bake

**Files:**
- **Create:**
  - `scripts/fetch-studio-hdri.sh` (curl, then MD5 check; prints the path);
  - `xtask/src/studio.rs`;
  - `crates/render/src/studio/mod.rs` (just `pub mod environment; mod environment_data;` for now);
  - `crates/render/src/studio/environment.rs`;
  - `crates/render/src/studio/environment_data.rs` (generated).
- **Modify:**
  - `xtask/src/main.rs` (the `studio` command);
  - `xtask/Cargo.toml` (`image = { workspace = true, features = ["hdr"] }`);
  - `.github/workflows/ci.yml` (the assets job re-bakes and `cmp`s);
  - `ASSETS.md`;
  - `crates/render/src/lib.rs` (`pub mod studio;`).

**Produces:**
- **In xtask:** `pub fn project_sh9(width, height, pixels: &[[f32;3]]) -> [[f64;3];9]` (equirect, weighted by solid angle), plus `white_balance`, `extract_key` and `rotate_to_key` (yaw-rotate so the key sits at azimuth +40°).
- **In the generated file:** `SH: [[f32;3];9]` (irradiance/π convolved), `KEY_DIR: [f32;3]` (unit, towards the light), `KEY_COLOUR: [f32;3]` (I/π).
- **In `environment.rs`:**
  - `pub fn irradiance(n: Vec3) -> Vec3`, returning E/π;
  - `pub fn key_dir() -> Vec3` and `pub fn key_colour() -> Vec3`;
  - `pub fn wrap(n_dot_l: f32) -> f32 { ((n_dot_l + 0.5) / 1.5).max(0.0) }` (normalised wrap: 1 at NoL = 1);
  - `pub fn exposure() -> f32`: 1 / luminance(irradiance(+Z) + key_colour · wrap(+Z·key_dir)).
  - The same functions are mirrored in WGSL in Task 4.

**Steps:**
- [ ] **Step 1: Failing xtask unit tests** (`xtask/src/studio.rs` `#[cfg(test)]`):
  - `a_constant_environment_has_only_l0`: a 64×32 all-ones image gives L0 = π·Y00·4π-weighted, and |L1|, |L2| < 1e-3.
  - `white_balance_makes_l0_grey`.
  - `extracting_the_key_keeps_l0` (the summed L0 before and after are equal within 1e-6).
  - `the_key_ends_up_at_plus_40_degrees_azimuth`.
- [ ] **Step 2:** run `cargo nextest run -p xtask`. Expect FAIL.
- [ ] **Step 3: Implement the bake.**
  - **SH basis:** the real SH constants (Y00 = 0.282095; Y1m = 0.488603·(y, z, x); Y2 terms 1.092548, 0.315392, 0.546274).
  - **Pixel directions:** θ = π(v + 0.5)/h; φ = 2π(u + 0.5)/w; dir = (sinθ·sinφ, cosθ, sinθ·cosφ). Solid angle = (2π/w)(π/h)·sinθ.
  - **Convolution:** multiply band l by Â_l/π, with Â = (π, 2π/3, π/4).
  - **White balance:** scale each channel by mean(L0)/L0_c.
  - **Key direction:** the dominant direction is normalize(L1 luminance vector as (x, y, z)). Rotate every coefficient about Y by the angle that moves its azimuth to +40°. Re-projecting the image with rotated directions is simplest: do two passes over the image.
  - **Key colour:** clamp the elevation to 25°–60°. Then I = 0.4·L0_lum / (Â0/π·Y00) as grey, and subtract the key's SH projection (I·Â_l/π·Y_lm(d)) from SH.
  - **Output:** write the file with `{:.6}` floats, a header comment naming the source, author, licence and the command.
  - **`main.rs`:** `cargo xtask studio <in.hdr> <out.rs>`.
- [ ] **Step 4: Fetch and bake (the download is approved).** Run `scripts/fetch-studio-hdri.sh`, then `cargo xtask studio target/studio/studio_small_08_1k.hdr crates/render/src/studio/environment_data.rs`.
- [ ] **Step 5: Render-side tests** (`environment.rs`):
  - `irradiance_is_never_negative`: on a 64×32 sphere grid, every channel is ≥ 0.
  - `key_comes_from_front_above`: elevation is between 25° and 60°.
  - `exposure_makes_the_front_reproduce_albedo`: exposure · luminance(E(+Z)) = 1.
- [ ] **Step 6:**
  - **CI:** the assets job gets `./scripts/fetch-studio-hdri.sh && cargo xtask studio target/studio/studio_small_08_1k.hdr target/environment_data.rs && cmp target/environment_data.rs crates/render/src/studio/environment_data.rs`.
  - **`ASSETS.md`:** add the row (baked SH; Poly Haven, Sergej Majboroda; CC0).
  - Run the tests, then commit `feat(render,xtask): bake the CC0 studio HDRI into SH9 and a soft key light`.

### Task 3: Quality levels (`studio/quality.rs`)

**Produces:**
- `pub enum Quality { Basic, Medium, High }` (Copy, Eq, Debug, serde-free).
- `pub fn auto(info: &wgpu::AdapterInfo, hdr_ok: bool) -> Quality`:
  - Basic if `!hdr_ok`, or `device_type` is Cpu, VirtualGpu or Other;
  - High if DiscreteGpu, or the name starts with "Apple";
  - otherwise Medium.
- `pub struct Settings { ao_moving: Option<AoSettings>, ao_still: AoSettings, shadow_moving: Option<ShadowSettings>, shadow_still: ShadowSettings, contact_size: u32, fxaa_moving: bool, cap_px: u32, still_frames: u32 }`, with `AoSettings { half_res: bool, samples: u32 }` and `ShadowSettings { size: u32, taps: u32 }`. Values come from the Global Constraints table.
- `pub fn render_size(w: u32, h: u32, cap_px: u32) -> (u32, u32)`: scales down, keeping the aspect ratio, so w·h ≤ cap, each side ≥ 1.

**Steps:**
- [ ] **Failing tests:**
  - `auto_picks_by_gpu_class`: build `AdapterInfo { device_type, name, .. }` for each class (Cpu or llvmpipe → Basic; Intel UHD integrated → Medium; Apple M2 integrated → High; discrete → High; no HDR → Basic).
  - `render_size_respects_the_cap` (1920×1080 at a 1 Mpx cap: ≤ 1,000,000 and aspect within 1 %; 1×1 stays 1×1).
  - `settings_follow_the_table`.
- [ ] **Run** (FAIL), **implement**, **run** (PASS).
- [ ] **Commit** `feat(render): studio quality levels and render-size caps`.

### Task 4: The main pass and output: studio, lighting, cloth, colour

**Files:**
- **Create** in `crates/render/src/studio/`:
  - `look.rs` (constants);
  - `mesh.rs` (`StudioMesh`, `Material`);
  - `targets.rs`;
  - `frame.rs` (`FrameUniforms`, std140-safe `#[repr(C)]` Pod);
  - `shade.wgsl`, `output.wgsl`, `output.rs`.
- **Fill** `studio/mod.rs` (`StudioRenderer`).
- **Create** the test `crates/render/tests/studio.rs`.
- **Modify** `crates/render/src/headless.rs`: `HeadlessGpu` gains `pub adapter: wgpu::Adapter`.

**Produces:**
- `pub enum Material { Cloth, Form, Floor }`.
- `pub struct StudioMesh`, with `pub fn set_colour(&mut self, linear: [f32;3])`. Colour and material are part of the frame signature.
- `StudioRenderer::new(device, adapter: &wgpu::Adapter, quality: Quality) -> Self`. It reads `hdr_ok` from the adapter and builds the floor disc mesh internally.
- `create_mesh(&mut self, device, queue, positions: &[Vec3], tris: &[[u32;3]], colour: [f32;3], material: Material) -> StudioMesh`.
- `update_mesh(&mut self, device, queue, mesh: &mut StudioMesh, positions, Option<tris>)`, which bumps `geometry_epoch`.
- `render(&mut self, device, queue, target: &RenderTarget, camera: &OrbitCamera, meshes: &[&StudioMesh]) -> Rendered`, with `pub struct Rendered { pub drew: bool, pub still_done: bool }`. In this task it always draws one "moving" frame with no AO or shadows.
- `pub fn hdr_ok(&self) -> bool`, `pub fn quality(&self) -> Quality`, `pub fn set_quality(&mut self, q)`.
- `#[doc(hidden)] pub struct Overrides { pub ao: Option<bool>, pub key_shadows: Option<bool>, pub contact: Option<bool>, pub force_ldr: bool }` and `pub fn set_overrides(&mut self, o: Overrides)`, for tests.

**Design:**
- **Group 0 (frame uniforms, about 600 B):**
  - matrices: `view_proj`, `view`, `inv_proj`, `key_view_proj`, `contact_view_proj`;
  - camera: `camera_pos`;
  - lighting: `key_dir`, `key_colour`, `sh[9]` (vec4 each);
  - output: `exposure`, `jitter`, `frame_index`, `screen_size`;
  - `flags` (ao, key shadows, contact, ldr).
- **Group 1 (draw uniforms):** `colour`, `material`.
- **Group 2:** the textures (shadow map + comparison sampler, contact map + linear sampler, AO + prepass depth). Until Tasks 5 and 6 they are bound to 1×1 placeholders.
- **Main pass** (`shade.wgsl`, own depth attachment `Less`, cull none):
  - **Backdrop:** a full-screen triangle at depth 1 with `LessEqual` and no writes. View-ray elevation e: `mix(HORIZON, TOP, smoothstep(0.0, 0.6, e))`, and HORIZON below 0. The colour is divided by `exposure` so that it displays as specified after tone mapping.
  - **Cloth:**
    - diffuse: `albedo*(sh(n)*mb(ao) + key*wrap(NoL)*shadow)`;
    - dielectric term: `0.04*F*(sh(n) + key*max(NoL,0)*shadow)` with Schlick F(NoV), normalised so F(1) = 1;
    - sheen: Charlie D (α = 0.5) with Neubelt V, colour `0.5*albedo + 0.03`, key only, times shadow;
    - the base is scaled by `1 - max3(sheen)*0.12*(1-NoV)^4`;
    - back faces: n = −n and albedo × 0.8.
  - **Form:** Lambert diffuse plus the same 0.04 dielectric term, no sheen.
  - **Floor:** Lambert plus 0.04, times contact (on ambient only) and shadow on key. It fades into the backdrop colour of its view ray with `smoothstep(3, 7, |xz|)`.
  - **Output:** HDR linear, or on the LDR path `encode(pbr_neutral(c*exposure))`.
- **Output pass** (`output.wgsl`, full-screen into `target.color_view`): `encode_srgb(pbr_neutral(textureLoad(hdr)*exposure))`; identity on the LDR path.
- **Targets** (`targets.rs`): `hdr` colour plus `main_depth` at the render size from `quality::render_size(target.w, target.h, cap)`. The output pass samples `hdr` with a linear sampler to fill the full-size target, so `target` may be bigger than the render size.

**Tests** (`crates/render/tests/studio.rs`, `headless_device()`):
- [ ] **Step 1: write them first.**
  - `a_matte_fabric_card_shows_its_true_colour`. A two-sided 0.6 m square card at z = 0, centred (0, 1, 0), facing +Z, as `Material::Cloth`. Camera `{target (0,1,0), yaw 0, pitch 0, distance 1.2, fov 35°}`, at 256×256, Overrides with AO, key shadows and contact all off. Render 2 frames, then average the central 16×16 pixels. For each colour, `delta_e2000(avg, srgb) <= 3.0`. The colours, as sRGB: (118,118,118), (180,40,45), (60,140,60), (30,40,90), (225,190,160), (205,160,40). Colours are given to `create_mesh` through `srgb8_to_linear`.
  - `the_ldr_path_is_close_too`: the same with `force_ldr: true`, ΔE ≤ 5.
  - `renders_at_odd_sizes_without_errors`: 1×1, 37×19 and 1000×700 targets, each Quality. Use `device.push_error_scope(Validation)` and `pop_error_scope` and expect None.
  - `the_backdrop_has_no_seam_at_the_floors_edge`: camera at pitch 0.05, distance 6. Down the centre column, from the floor near the camera up to the backdrop, no step between adjacent rows is greater than 6/255 in luminance.
- [ ] **Step 2:** run `cargo nextest run -p opendrape-render --test studio` and expect FAIL (compile).
- [ ] **Step 3: implement** the design above.
- [ ] **Step 4:** run and expect PASS. If the card ΔE is over 3, the ruling is: tune `sheen colour` and the wrap only, never the tolerance. Record it in the ledger.
- [ ] **Step 5:** commit `feat(render): studio renderer: studio backdrop and floor, SH studio light, cloth sheen and lining, PBR Neutral colour`.

### Task 5: Soft key shadows and the floor contact shadow

**Files:** `studio/shadow.rs`, `studio/shadow.wgsl` (depth-only key pass; contact pass writing `1 - y/H` with max blending, then a two-pass separable 9-tap Gaussian into `R8Unorm`); modify `mod.rs`, `shade.wgsl`, `targets.rs`.

**Design:**
- **Key shadow map:**
  - an orthographic box from the key direction, a 2.6 m square centred on (0, 0.95, 0), depth 0–6 m;
  - `Depth32Float`, at the size from Settings;
  - depth bias constant 2, slope scale 2.0;
  - in the shader: normal offset of 1.5 shadow texels along n, then N Poisson taps with `textureSampleCompare` within a 1.2 cm radius, the pattern rotated by `frame_index` only when still.
- **Contact map:** an orthographic camera from below over 2.4 m × 2.4 m, height range 0.6 m. Opacity 0.6 is applied in the floor's ambient term.
- **When maps are redrawn:** both only when `geometry_epoch` or the quality changed. `StudioRenderer::stats() -> Stats { shadow_redraws: u64, frames_drawn: u64 }` counts them.

**Tests:**
- [ ] **Write first:**
  - `the_floor_under_a_low_box_is_darker_with_a_soft_edge`. A 0.3 m box spanning y 0.05–0.35 at the origin; camera yaw 0, pitch 0.5, distance 2.2, target (0, 0.2, 0); key shadows off and contact on.
    - The floor pixel 4 cm in front of the box's front-bottom edge must be ≥ 15 % darker in luminance than the floor 1 m to the side.
    - Along the screen column below that point, the luminance rises from 10 % to 90 % of its range over ≥ 6 px.
  - `key_shadows_fall_on_the_floor_away_from_the_light`. Contact off, key on, the same box. A floor point 0.3 m from the box, opposite the key azimuth, is darker than its mirror point on the key side.
  - `shadow_maps_are_redrawn_only_when_geometry_changes`. Render; change only the camera and render: `shadow_redraws` is unchanged. Call `update_mesh` and render: it went up by 1.
- [ ] **Run** (FAIL), **implement**, **run** (PASS), **commit** `feat(render): soft key-light shadows and a floor contact shadow, redrawn only when the geometry changes`.

### Task 6: Ambient occlusion

**Files:** `studio/ao.rs`, `studio/ao.wgsl` (prepass, AO, blur); modify `mod.rs`, `shade.wgsl`, `targets.rs`.

**Design:**
- **Prepass:** full resolution, with depth `Depth32Float` (+TEXTURE_BINDING) and view normals `Rgba8Unorm` (n·0.5 + 0.5), for every mesh including the floor.
- **AO pass:** half or full resolution, into `Rgba8Unorm`:
  - reconstruct the view position from the depth (`inv_proj`);
  - sample N points from a normal-oriented hemisphere kernel, generated in the shader with a cosine distribution and lengths scaled by `mix(0.1, 1, (i/N)^2)`, rotated per pixel by interleaved gradient noise (plus `frame_index` when still);
  - radius 0.06 m, range check `smoothstep(0, 1, R/|Δz|)`, bias 0.005 m;
  - AO = 1 − occlusion/N;
  - pack the linear depth in g and b as a 16-bit value over 0–20 m.
- **Blur:** two passes (horizontal, then vertical), 5 taps, weights `exp(-|Δdepth|/0.02 m)`.
- **Main pass:** a bilateral upsample from the 4 nearest AO texels, weighted by depth similarity to the full-resolution prepass depth (`textureLoad`). Multi-bounce: `mb(ao, albedo) = max(ao, ((ao*a + b)*ao + c)*ao)` with GTAO's a, b and c. AO multiplies the SH ambient (and the floor's ambient) only.

**Tests:**
- [ ] **Write first:**
  - `ao_darkens_a_crease`. Two 0.5 m quads meet at a 90° valley along the y axis (an open book facing +Z), centred (0, 1, 0); camera from +Z. The valley-line pixels with AO on must be ≥ 15 % darker than with `Overrides { ao: Some(false) }`.
  - `ao_leaves_flat_surfaces_alone`. A flat card at the same place: the mean of its central 32×32 with AO is within 2 % of without.
  - `ao_runs_at_every_quality` (no validation errors at 37×19).
- [ ] **Run** (FAIL), **implement**, **run** (PASS), **commit** `feat(render): ambient occlusion in folds and creases, at half resolution with a depth-aware blur`.

### Task 7: Still frames: signature, jitter accumulation, FXAA and stopping

**Files:** modify `mod.rs`, `output.rs`, `output.wgsl`, `targets.rs` (accumulation ping-pong `acc[2]` in the HDR format).

**Design:**
- **Signature:**
  - the camera fields as bits;
  - the target size and render size;
  - the quality and overrides;
  - `geometry_epoch`;
  - each mesh's (id, colour bits, material) in draw order.
- **Moving or still:** a frame is *moving* when the signature differs from the previous frame's. On moving: `acc_count = 0`, draw with the moving settings, output with FXAA when `fxaa_moving`, `still_done = false`.
- **Still:** draw with the still settings and jitter `halton(2,3)[acc_count % 32] - 0.5` in pixels at the render size, applied to the projection. Accumulate `acc_new = mix(acc_old, hdr, 1/(acc_count+1))`, then output from `acc_new` with no FXAA, and `acc_count += 1`.
- **Done:** when `acc_count == still_frames`, `still_done = true`. Later calls with the same signature return `Rendered { drew: false, still_done: true }` with no GPU work.
- **The AO and shadow noise** uses `frame_index = acc_count` when still, and 0 when moving.
- **FXAA** (in `output.wgsl`): FXAA 3.11 "quality 10": luma from tone-mapped neighbours, an edge search of 4 steps.

**Tests:**
- [ ] **Write first:**
  - `a_still_view_finishes_and_then_does_nothing`. With unchanged inputs, `still_done` becomes true after exactly `still_frames + 1` calls (one moving frame, then N still ones). The next call returns `drew == false`, and `stats().frames_drawn` doesn't change.
  - `moving_the_camera_starts_over`. After done, yaw += 0.01: `drew == true` and `still_done == false`.
  - `changing_a_colour_starts_over`. After done, `set_colour` on a mesh: drew is true.
  - `jitter_does_not_shift_the_image`. The luminance centroid of a card after accumulation is within 0.25 px of the first, unjittered frame.
  - `still_edges_are_smoother_than_moving_ones`. Across the card's slanted edge, the count of distinct intermediate luminance values is larger in the done image than in the moving image.
  - `resizing_restarts_cleanly`. Go done at 200×100, then 1×1, then 300×300: no validation errors, and done again eventually.
- [ ] **Run** (FAIL), **implement**, **run** (PASS), **commit** `feat(render): still frames build up supersampled edges and soft shadows, then stop drawing; FXAA while moving`.

### Task 8: The app uses the studio renderer; View → 3D quality

**Files:**
- **Modify:**
  - `crates/app/src/viewport.rs`: `StudioRenderer` and `StudioMesh` replace `MeshRenderer` and `GpuMesh`. The body uses `Material::Form` in `theme::FORM`, the cloth and pieces `Material::Cloth`. It requests a repaint while `!still_done`, and `frames_drawn` counts only frames with `drew`.
  - `crates/app/src/theme.rs`: `SKIN` becomes `FORM` = linear of sRGB(188, 168, 153); update its value test.
  - `crates/app/src/app.rs`: a View → "3D quality" submenu with four radio items, applied with `viewport.set_quality`, saved.
  - `crates/app/i18n/en-US/opendrape.ftl`.
- **Create:** `crates/app/src/view_settings.rs`.

**Produces:**
- `pub enum QualityChoice { Auto, Basic, Medium, High }` (serde), and `ViewSettings { quality: QualityChoice }` with `load(dir) / save(dir)` (atomic like `StateStore`), file `view.json`.
- `StateStore::dir() -> Option<&Path>`.
- `Viewport::new(rs, stage, choice)`. `Viewport::quality() -> Quality` gives the effective level, and `Viewport::auto_quality() -> Quality` the label for "Auto (Medium)".

**Strings:**
- `menu-quality = 3D quality`
- `quality-auto = Auto ({ $level })`
- `quality-basic = Basic`
- `quality-medium = Medium`
- `quality-high = High`
- `quality-tip = How the 3D view looks. Lower levels stay smooth on slower computers.`

**Tests:**
- [ ] **Write first:**
  - `view_settings.rs` unit tests: round trip; a missing or bad file loads Auto.
  - `crates/app/tests/workspaces.rs`:
    - `the_3d_quality_menu_switches_and_remembers`: click View → 3D quality → Basic. Then `state().viewport_quality() == Quality::Basic`, and `view.json` in the harness config dir says Basic. A fresh harness on the same dir starts in Basic.
    - `the_3d_view_stops_drawing_when_still`: run until the frame count settles. Two further `run()` calls must not raise `viewport_frames()`. A camera view click (Back) raises it again.
  - Keep every existing test green. `ui.rs` tests rely only on `viewport_frames() > 0`.
- [ ] **Run** (FAIL), **implement**, **run** the full workspace (PASS), **commit** `feat(app): the 3D view uses the studio renderer; View → 3D quality, remembered between launches`.

### Task 9: Docs, checks, review

- [ ] **`docs/testing/M5b-checklist.md`** (plain language):
  - the studio look;
  - folds darker and soft;
  - the floor shadow;
  - colours match: compare a piece coloured with a known swatch;
  - the view sharpens about a second after you stop, then the fans quieten;
  - View → 3D quality: Basic, Medium, High and Auto;
  - turning the view stays smooth;
  - the T-shirt drapes as before.
- [ ] **README status, spec Status (M5b), `ASSETS.md`:** check them.
- [ ] **Full checks:** `cargo fmt --check && cargo clippy --workspace --all-targets -- -D warnings && cargo nextest run --workspace && cargo deny check licenses`.
- [ ] **Commit** `docs: M5b tester checklist and status`.
- [ ] **Final review:** one fresh reviewer on the most capable model, given the Review Focus. Fix the confirmed Critical and Important findings with TDD, then re-run the suite.
- [ ] **Hand back:** ask before launching the app, and before merging or pushing.
