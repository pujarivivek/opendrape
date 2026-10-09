# OpenDrape M1 (Drape Spike) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** The app shows a real CC0 human body. A hard-coded two-panel A-line skirt falls onto it, its side seams pull shut and weld, and it settles at the waist without passing through the body. A second scene drapes a tight, body-shaped "fitted tube" as a close-fit collision test. Play/Pause, Reset, a garment switch, an orbit camera and an FPS overlay come with it.

**Architecture:** four new pieces.
- **`crates/body`** holds the avatar mesh. Its compact `.odb` file format is generated from pinned CC0 MakeHuman sources by `cargo xtask body`. The crate also measures height, girth and closedness.
- **`crates/sim`** is XPBD cloth in f64. It handles stretch, bending, stitches that close and then weld, and collision against the body.
  - **Collision:** the body collider is parry3d's exact closest-point query. It runs once per frame and gives each particle a contact plane, which is enforced every substep with friction.
- **`crates/testkit`** builds the two demo garments on the bundled body. It also measures drape quality (penetration, seam gap, strain, kinetic energy), and its acceptance tests gate the physics.
- **The app** runs the simulation on a worker thread (`SimRunner`), publishing frames via `arc-swap`. A new `MeshRenderer` in `crates/render` draws the body and cloth.

**Tech Stack:**
- Already in use: Rust 1.99, eframe/egui/egui-wgpu/egui_kittest 0.36.2, wgpu 30.0.1, glam 0.33.
- New: **parry3d 0.31.1** (Apache-2.0; uses glam 0.33 natively), rayon 1.12, crossbeam-channel 0.5, arc-swap 1.

**Spec:** `docs/specs/2026-10-09-opendrape-design.md` (milestone M1, plus the Architecture and Verification sections).

**Evidence behind every number in this plan** (throwaway prototype `scratchpad/drape-probe`, run on 2026-10-09, M4 Max):

| Run | Penetration | Seam gap | Strain mean / p99 / max | KE at end | Time per frame (serial) |
|---|---|---|---|---|---|
| Skirt, 20 substeps × 2 iterations, 71 cm waist, weld at 0.8 s | **0.00 mm** | **0** | 0.53% / 7.6% / 13.5% | 2e-7 J | 11.7 ms (4,794 particles) |
| Fitted tube, top ring pinned, 3% pre-tension | **0.00 mm** | — | 0.24% / 3.1% / 4.4% | 2e-8 J | 3.5 ms (1,440 particles) |

**What the prototype ruled out:**
- 1 iteration: p99 strain 15–24%.
- Solving stitches last: 50–78% strain beside the seams.
- Over-relaxation: unstable.
- Friction off: the skirt slides down.
- A 68 cm waist: the ring stretches over the hips.

## Global Constraints

Carried over from M0 (unchanged): Rust 1.99.0 pinned; egui family `=0.36.2`; `wgpu =30.0.1`; `glam 0.33`; GPL-3.0-or-later with `cargo deny check` passing; never "CLO"; every user-visible string goes through `tr!`; shaders stay within `downlevel_webgl2_defaults`; egui textures are `Rgba8Unorm` with sRGB encoded in the shader; 3D is in metres with Y up and the body facing +Z.

New in M1:
- **Body assets:** only CC0 MakeHuman *data* files (base mesh and targets), pinned to makehuman tag v1.3.0, commit `1f508f6083b2f823dab15de924b3bde72e08d77c`, and SHA-256 verified. Never MakeHuman code (AGPL), never MPFB code or config, never SMPL. Record every asset in `ASSETS.md`.
- **Physics state is f64** (`glam::DVec3`). f32 jittered in the prototype research. Convert to f32 only for rendering and parry queries.
- **Default `Params`**, exactly as the prototype validated:
  - steps: substeps 20, iterations 2;
  - gravity: −9.81, delayed 0.6 s, ramped over 0.3 s;
  - seams: stitches close over 0.5 s, weld at 0.8 s;
  - fabric: stretch compliance 1e-6, bend compliance 1.0;
  - motion: damping 1.0/s, friction 0.4, max speed 2.0 m/s;
  - collision: thickness 3 mm, collision margin 5 cm;
  - fabric weight 0.15 kg/m², edge length 1.2 cm.
- **Solve order in each iteration:** stitches, then bending, then stretch, so fabric constraints have the last word.
- **Determinism:**
  - no `HashMap` iteration order may affect results (collect keys, then sort);
  - the rayon work is only per-particle collision queries, each independent;
  - the same inputs give bit-identical positions on the same machine.
- **The simulation never runs on the UI thread** in the app.

**Spec deviations, on purpose (the M1 rulings):**
1. **No baked SDF in M1.** parry3d's exact closest-point query costs about 0.5 ms per frame for 8k particles with rayon, and it is exact. The SDF comes back only if low-end profiling needs it.
2. **Constraint solving is serial Gauss–Seidel.** Graph-coloured parallel batches move to the M4 performance pass.
3. **Seams weld after they close.** Merging the stitched vertices (as GarmentCodeData does) gives a seam gap of exactly 0 and avoids stitch-vs-fabric fighting.
4. **The demo garments live in `testkit`**, and the app uses them for its two hard-coded scenes until patterns arrive in M2/M3.
5. **The skirt waist is 71 cm** (35.5 cm per panel) for the 65 cm body waist. That is realistic ease; 68 cm stretched the ring over the hips.

## Review Focus

1. **The simulation is slower than real time on a low-end CPU.** The window must stay responsive: the sim runs on its own thread, the UI only reads the latest frame, and pacing never busy-waits. Test: Task 7 `the_simulation_runs_without_the_window_drawing`.
2. **Reset or garment switches clicked rapidly or mid-step.** No deadlock or panic, and the last request wins. Test: Task 7 `rapid_resets_end_on_the_last_choice`.
3. **Degenerate geometry** (a zero-area triangle, or a vertex used by no triangle). It must not produce NaN or a division by zero. Test: Task 3 `degenerate_triangle_does_not_produce_nan`.
4. **Closing the window while the simulation runs.** The worker thread must stop promptly (Drop joins it). Test: Task 7 `dropping_the_runner_stops_its_thread`.
5. **A corrupt or mismatched body asset.** `read_odb` rejects bad magic, truncated data and out-of-range indices, and the bundled asset is checked for counts and closedness. Tests: Task 1 format tests; Task 2 asset tests.

---

## File Structure

```
Cargo.toml                         + members body/sim/testkit; deps parry3d, rayon, crossbeam-channel, arc-swap; dev opt-level 3 for sim/body/testkit
scripts/fetch-makehuman.sh         downloads the 4 pinned CC0 files into target/makehuman/, verifies SHA-256
assets/body/female_average.odb     generated body (13,380 vertices, 26,756 triangles, about 0.48 MB), committed
crates/body/  Cargo.toml, src/lib.rs (BodyMesh, female_average), src/format.rs (.odb read/write), src/measure.rs (height, girth, boundary edges), tests/asset.rs
crates/sim/   Cargo.toml, src/lib.rs, src/cloth.rs (Panel, ClothBuilder, Cloth, weld), src/solver.rs (Params, Solver, FRAME_DT), src/collide.rs (Plane, Collider, BodyCollider)
crates/testkit/ Cargo.toml, src/lib.rs, src/garments.rs (Garment, Scene, skirt, bodice_proxy, body/collider caches), src/metrics.rs (DrapeReport, measure, position_hash, run), tests/drape.rs, examples/drape_bench.rs
crates/render/ src/mesh.rs + src/mesh.wgsl (MeshRenderer, GpuMesh, vertex_normals); cube.rs/cube.wgsl removed in Task 7
crates/app/   src/sim_runner.rs (SimRunner, SimFrame), src/viewport.rs (body + cloth), src/app.rs (toolbar, overlay), i18n strings, tests/ui.rs
xtask/        src/makehuman.rs (OBJ + target parsing, body conversion), src/main.rs (`body` subcommand)
.github/workflows/ci.yml           + assets job: rebuild body from sources, byte-compare
docs/testing/M1-checklist.md, ASSETS.md, docs/specs/... status
```

All commands assume `source ~/.cargo/env` and the repo root as the working directory. Work on branch `m1-drape-spike`, created from `main`.

---

### Task 1: `opendrape-body`: mesh type, `.odb` format, measurements

**Files:**
- Create: `crates/body/Cargo.toml`, `crates/body/src/{lib.rs,format.rs,measure.rs}`
- Modify: root `Cargo.toml`

**Interfaces:**
- Produces:
  - `BodyMesh { positions: Vec<glam::Vec3>, triangles: Vec<[u32;3]> }`
  - `write_odb(&BodyMesh) -> Vec<u8>`
  - `read_odb(&[u8]) -> Result<BodyMesh, OdbError>` (`OdbError::{BadMagic, Truncated, IndexOutOfRange}`)
  - `height(&BodyMesh) -> f32`
  - `boundary_edge_count(&BodyMesh) -> usize`
  - `girth_at(&BodyMesh, y: f32, max_abs_x: f32) -> f32`, the perimeter of the convex hull of the slice, in metres.

- [ ] **Step 1: Workspace entries and crate skeleton**

In the root `Cargo.toml`:
- set `members = ["crates/app", "crates/render", "crates/body", "crates/sim", "crates/testkit", "xtask"]`. Create the `sim` and `testkit` folders in Tasks 3 and 5; until then list only the crates that exist.
- add to `[workspace.dependencies]`:
```toml
opendrape-body = { path = "crates/body" }
opendrape-sim = { path = "crates/sim" }
opendrape-testkit = { path = "crates/testkit" }
parry3d = "=0.31.1"
rayon = "1.12"
crossbeam-channel = "0.5"
arc-swap = "1"
```
- add, after the existing profiles:
```toml
# Physics and its tests are unusably slow unoptimised.
[profile.dev.package.opendrape-body]
opt-level = 3
[profile.dev.package.opendrape-sim]
opt-level = 3
[profile.dev.package.opendrape-testkit]
opt-level = 3
```

`crates/body/Cargo.toml`:
```toml
[package]
name = "opendrape-body"
description = "OpenDrape avatar bodies and body measurements"
version.workspace = true
edition.workspace = true
rust-version.workspace = true
license.workspace = true
repository.workspace = true
publish.workspace = true

[dependencies]
glam.workspace = true

[lints]
workspace = true
```

`crates/body/src/lib.rs`:
```rust
//! The 3D body (avatar): meshes derived from CC0 MakeHuman data, a compact file format,
//! and tape-measure style body measurements.

mod format;
mod measure;

pub use format::{OdbError, read_odb, write_odb};
pub use measure::{boundary_edge_count, girth_at, height};

/// A static triangle mesh in metres, Y up, facing +Z, feet at y = 0.
#[derive(Clone, Debug, PartialEq)]
pub struct BodyMesh {
    pub positions: Vec<glam::Vec3>,
    pub triangles: Vec<[u32; 3]>,
}
```

- [ ] **Step 2: Failing tests for the format and measurements**

`crates/body/src/format.rs`:
```rust
use crate::BodyMesh;

#[derive(Debug, PartialEq, Eq)]
pub enum OdbError {
    BadMagic,
    Truncated,
    IndexOutOfRange,
}

pub fn write_odb(_mesh: &BodyMesh) -> Vec<u8> {
    todo!()
}

pub fn read_odb(_bytes: &[u8]) -> Result<BodyMesh, OdbError> {
    todo!()
}

#[cfg(test)]
mod tests {
    use super::*;
    use glam::Vec3;

    fn tiny() -> BodyMesh {
        BodyMesh { positions: vec![Vec3::ZERO, Vec3::X, Vec3::Y], triangles: vec![[0, 1, 2]] }
    }

    #[test]
    fn round_trips() {
        assert_eq!(read_odb(&write_odb(&tiny())), Ok(tiny()));
    }

    #[test]
    fn rejects_bad_magic_truncation_and_bad_indices() {
        assert_eq!(read_odb(b"not a body file at all"), Err(OdbError::BadMagic));
        let bytes = write_odb(&tiny());
        assert_eq!(read_odb(&bytes[..bytes.len() - 1]), Err(OdbError::Truncated));
        let bad = BodyMesh { triangles: vec![[0, 1, 3]], ..tiny() };
        assert_eq!(read_odb(&write_odb(&bad)), Err(OdbError::IndexOutOfRange));
    }
}
```

`crates/body/src/measure.rs`:
```rust
use crate::BodyMesh;

/// Height from the lowest to the highest vertex.
pub fn height(_mesh: &BodyMesh) -> f32 {
    todo!()
}

/// Edges used by exactly one triangle: 0 for a closed mesh.
pub fn boundary_edge_count(_mesh: &BodyMesh) -> usize {
    todo!()
}

/// Tape-measure girth at height `y`: perimeter of the convex hull of the mesh's
/// cross-section, keeping only points with |x| < `max_abs_x` (to leave out the arms).
pub fn girth_at(_mesh: &BodyMesh, _y: f32, _max_abs_x: f32) -> f32 {
    todo!()
}

#[cfg(test)]
mod tests {
    use super::*;
    use glam::Vec3;

    /// Closed unit cube centred on the origin, triangles counter-clockwise from outside.
    fn cube() -> BodyMesh {
        let positions = [
            [-0.5, -0.5, -0.5], [0.5, -0.5, -0.5], [0.5, 0.5, -0.5], [-0.5, 0.5, -0.5],
            [-0.5, -0.5, 0.5], [0.5, -0.5, 0.5], [0.5, 0.5, 0.5], [-0.5, 0.5, 0.5],
        ]
        .map(Vec3::from_array)
        .to_vec();
        let triangles = vec![
            [0, 3, 2], [0, 2, 1], [4, 5, 6], [4, 6, 7], [0, 4, 7], [0, 7, 3],
            [1, 2, 6], [1, 6, 5], [0, 1, 5], [0, 5, 4], [3, 7, 6], [3, 6, 2],
        ];
        BodyMesh { positions, triangles }
    }

    #[test]
    fn cube_height_and_girth() {
        assert!((height(&cube()) - 1.0).abs() < 1e-6);
        assert!((girth_at(&cube(), 0.0, 10.0) - 4.0).abs() < 1e-5);
    }

    #[test]
    fn girth_ignores_points_outside_the_x_limit() {
        // At y = 0 the slice points are the 4 corners (|x| = 0.5), the ±X face-diagonal
        // crossings (|x| = 0.5) and the ±Z face-diagonal crossings (x = 0). With |x| < 0.4
        // only the two x = 0 points remain: no area, so no girth.
        assert_eq!(girth_at(&cube(), 0.0, 0.4), 0.0);
    }

    #[test]
    fn closed_and_open_meshes() {
        assert_eq!(boundary_edge_count(&cube()), 0);
        let mut open = cube();
        open.triangles.truncate(10); // remove the top face (2 triangles)
        assert_eq!(boundary_edge_count(&open), 4);
    }
}
```

- [ ] **Step 3: Run and watch them fail**

Run: `cargo nextest run -p opendrape-body`
Expected: 5 FAIL with `not yet implemented`.

- [ ] **Step 4: Implement**

`format.rs` bodies:
```rust
const MAGIC: &[u8; 8] = b"ODBODY01";

/// `ODBODY01`, vertex count (u32), triangle count (u32), then little-endian f32 xyz per
/// vertex and u32 × 3 per triangle.
pub fn write_odb(mesh: &BodyMesh) -> Vec<u8> {
    let mut out = Vec::with_capacity(16 + 12 * (mesh.positions.len() + mesh.triangles.len()));
    out.extend_from_slice(MAGIC);
    out.extend_from_slice(&(mesh.positions.len() as u32).to_le_bytes());
    out.extend_from_slice(&(mesh.triangles.len() as u32).to_le_bytes());
    for p in &mesh.positions {
        for c in p.to_array() {
            out.extend_from_slice(&c.to_le_bytes());
        }
    }
    for t in &mesh.triangles {
        for i in t {
            out.extend_from_slice(&i.to_le_bytes());
        }
    }
    out
}

pub fn read_odb(bytes: &[u8]) -> Result<BodyMesh, OdbError> {
    if bytes.len() < 16 || &bytes[..8] != MAGIC {
        return Err(OdbError::BadMagic);
    }
    let word = |o: usize| -> [u8; 4] { bytes[o..o + 4].try_into().expect("4 bytes") };
    let (nv, nt) = (u32::from_le_bytes(word(8)) as usize, u32::from_le_bytes(word(12)) as usize);
    if bytes.len() != 16 + 12 * (nv + nt) {
        return Err(OdbError::Truncated);
    }
    let f = |o: usize| f32::from_le_bytes(word(o));
    let positions = (0..nv).map(|i| glam::Vec3::new(f(16 + 12 * i), f(20 + 12 * i), f(24 + 12 * i))).collect();
    let base = 16 + 12 * nv;
    let mut triangles = Vec::with_capacity(nt);
    for i in 0..nt {
        let t = [0, 4, 8].map(|k| u32::from_le_bytes(word(base + 12 * i + k)));
        if t.iter().any(|&k| k as usize >= nv) {
            return Err(OdbError::IndexOutOfRange);
        }
        triangles.push(t);
    }
    Ok(BodyMesh { positions, triangles })
}
```

`measure.rs` bodies (and `use glam::Vec2; use std::collections::HashMap;` at the top):
```rust
pub fn height(mesh: &BodyMesh) -> f32 {
    let (lo, hi) = mesh.positions.iter().fold((f32::MAX, f32::MIN), |(lo, hi), p| (lo.min(p.y), hi.max(p.y)));
    hi - lo
}

pub fn boundary_edge_count(mesh: &BodyMesh) -> usize {
    let mut uses: HashMap<(u32, u32), u32> = HashMap::new();
    for t in &mesh.triangles {
        for k in 0..3 {
            let (a, b) = (t[k], t[(k + 1) % 3]);
            *uses.entry((a.min(b), a.max(b))).or_default() += 1;
        }
    }
    uses.values().filter(|&&n| n == 1).count()
}

pub fn girth_at(mesh: &BodyMesh, y: f32, max_abs_x: f32) -> f32 {
    let mut pts = vec![];
    for t in &mesh.triangles {
        for k in 0..3 {
            let (a, b) = (mesh.positions[t[k] as usize], mesh.positions[t[(k + 1) % 3] as usize]);
            if (a.y - y) * (b.y - y) < 0.0 {
                let p = a + (b - a) * ((y - a.y) / (b.y - a.y));
                if p.x.abs() < max_abs_x {
                    pts.push(Vec2::new(p.x, p.z));
                }
            }
        }
    }
    hull_perimeter(pts)
}

/// Andrew's monotone chain convex hull, returning its perimeter.
fn hull_perimeter(mut pts: Vec<Vec2>) -> f32 {
    pts.sort_by(|a, b| a.x.total_cmp(&b.x).then(a.y.total_cmp(&b.y)));
    pts.dedup();
    if pts.len() < 3 {
        return 0.0;
    }
    fn half(points: impl Iterator<Item = Vec2>) -> Vec<Vec2> {
        let mut h: Vec<Vec2> = vec![];
        for p in points {
            while h.len() >= 2 && (h[h.len() - 1] - h[h.len() - 2]).perp_dot(p - h[h.len() - 2]) <= 0.0 {
                h.pop();
            }
            h.push(p);
        }
        h.pop();
        h
    }
    let mut hull = half(pts.iter().copied());
    hull.extend(half(pts.iter().rev().copied()));
    (0..hull.len()).map(|i| hull[i].distance(hull[(i + 1) % hull.len()])).sum()
}
```

- [ ] **Step 5: Run and see them pass**

Run: `cargo nextest run -p opendrape-body`
Expected: 5 passed.

- [ ] **Step 6: Commit**

```bash
cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings
git add Cargo.toml Cargo.lock crates/body
git commit -m "feat(body): body mesh type, .odb file format and measurements

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 2: The CC0 body asset (fetch, convert, bundle, verify in CI)

**Files:**
- Create: `scripts/fetch-makehuman.sh`, `xtask/src/makehuman.rs`, `assets/body/female_average.odb`, `crates/body/tests/asset.rs`
- Modify: `xtask/Cargo.toml`, `xtask/src/main.rs`, `crates/body/src/lib.rs`, `ASSETS.md`, `.github/workflows/ci.yml`

**Interfaces:**
- Consumes: `BodyMesh`, `write_odb`, `read_odb`, `height`, `girth_at` and `boundary_edge_count` from Task 1.
- Produces:
  - `BodyMesh::female_average() -> BodyMesh`, the bundled asset;
  - `cargo xtask body <makehuman_dir> <out.odb>`;
  - `scripts/fetch-makehuman.sh`, which prints `target/makehuman`.

- [ ] **Step 1: Fetch script**

`scripts/fetch-makehuman.sh`:
```bash
#!/usr/bin/env bash
# Downloads the CC0 MakeHuman data files OpenDrape's bodies are built from, pinned to
# makehuman v1.3.0, and checks every file's SHA-256. Only data (mesh + targets), never code.
# License: makehuman LICENSE.md section C, "These assets have been released under CC0 1.0 Universal."
set -euo pipefail
cd "$(dirname "$0")/.."
BASE=https://raw.githubusercontent.com/makehumancommunity/makehuman/1f508f6083b2f823dab15de924b3bde72e08d77c/makehuman/data
OUT=target/makehuman
mkdir -p "$OUT"
sha256() { if command -v sha256sum >/dev/null; then sha256sum "$1" | cut -d' ' -f1; else shasum -a 256 "$1" | cut -d' ' -f1; fi; }
while read -r sum path; do
  file="$OUT/$(basename "$path")"
  [ -f "$file" ] || curl -fsSL "$BASE/$path" -o "$file"
  if [ "$(sha256 "$file")" != "$sum" ]; then echo "checksum mismatch: $path" >&2; rm -f "$file"; exit 1; fi
done <<'LIST'
8e761e6624b8f54536409135d1636da63b32486a90d4897f84e121d144f6fb4c 3dobjs/base.obj
92d61eeb3c164b421fd5a7c3537ee45e7e1a51de4d49bf312e19c3df2be1d8fc targets/macrodetails/african-female-young.target
095fe79694fa19e1fe98d93009ec116199bd524e081c640351a10eccf2cca1eb targets/macrodetails/asian-female-young.target
118379f6e8ba9266247fdb8788a20e1df40a239f97ced0b9905bcbcc74f6e820 targets/macrodetails/caucasian-female-young.target
LIST
echo "$OUT"
```
`chmod +x scripts/fetch-makehuman.sh`, then run it. Expected: it prints `target/makehuman`, and that folder holds the 4 files.

- [ ] **Step 2: Failing tests for the conversion**

`xtask/Cargo.toml`, add to `[dependencies]`: `opendrape-body.workspace = true` and `glam.workspace = true`.

`xtask/src/makehuman.rs`:
```rust
//! Builds OpenDrape bodies from CC0 MakeHuman data files (hm08 base mesh + targets).
//! Pure data parsing; no MakeHuman code is used.

use opendrape_body::BodyMesh;

/// The average young adult female: MakeHuman's three ethnicity targets blended equally
/// (their sliders sum to 1); height/proportion/breast targets are at weight 0 by default.
pub const FEMALE_AVERAGE: [(&str, f64); 3] = [
    ("african-female-young.target", 1.0 / 3.0),
    ("asian-female-young.target", 1.0 / 3.0),
    ("caucasian-female-young.target", 1.0 / 3.0),
];

/// A parsed OBJ: vertices (MakeHuman units: decimetres) and faces with their `g` group.
pub struct ObjMesh {
    pub verts: Vec<[f64; 3]>,
    pub faces: Vec<Vec<u32>>,
    pub groups: Vec<String>,
}

pub fn parse_obj(_text: &str) -> ObjMesh {
    todo!()
}

/// `v[i] += weight * (dx, dy, dz)` for each line `i dx dy dz`; returns lines applied.
pub fn apply_target(_verts: &mut [[f64; 3]], _target: &str, _weight: f64) -> usize {
    todo!()
}

/// Faces of `group` only, reindexed in original vertex order; quads split along the shorter
/// diagonal (winding kept); decimetres → metres; lowest point moved to y = 0.
pub fn body_mesh(_obj: &ObjMesh, _group: &str) -> BodyMesh {
    todo!()
}

#[cfg(test)]
mod tests {
    use super::*;

    const OBJ: &str = "# test\nv 0 0 0\nv 10 0 0\nv 10 20 0\nv 0 20 0\nv 5 5 5\nvt 0 0\ng body\nf 1/1 2/1 3/1 4/1\ng helper-tights\nf 1 2 5\n";

    #[test]
    fn parses_vertices_faces_and_groups() {
        let m = parse_obj(OBJ);
        assert_eq!(m.verts.len(), 5);
        assert_eq!(m.faces, vec![vec![0, 1, 2, 3], vec![0, 1, 4]]);
        assert_eq!(m.groups, vec!["body".to_string(), "helper-tights".to_string()]);
    }

    #[test]
    fn applies_weighted_target_offsets() {
        let mut v = vec![[0.0; 3]; 3];
        let n = apply_target(&mut v, "# comment\n\n1 1.0 2.0 -4.0\n", 0.5);
        assert_eq!(n, 1);
        assert_eq!(v[1], [0.5, 1.0, -2.0]);
        assert_eq!(v[0], [0.0; 3]);
    }

    #[test]
    fn keeps_body_group_triangulates_scales_and_grounds() {
        let b = body_mesh(&parse_obj(OBJ), "body");
        assert_eq!(b.positions.len(), 4, "helper-only vertex 5 dropped");
        // 10×20 dm quad: diagonals equal, so split along a-c: (a,b,c),(a,c,d)
        assert_eq!(b.triangles, vec![[0, 1, 2], [0, 2, 3]]);
        assert!((b.positions[2] - glam::Vec3::new(1.0, 2.0, 0.0)).length() < 1e-6, "dm → m");
        assert_eq!(b.positions.iter().map(|p| p.y).fold(f32::MAX, f32::min), 0.0);
    }

    #[test]
    fn splits_quads_along_the_shorter_diagonal() {
        // a=(0,0) b=(10,0) c=(30,10) d=(0,10): a–c is long, b–d is short → (a,b,d),(b,c,d)
        let obj = "v 0 0 0\nv 10 0 0\nv 30 10 0\nv 0 10 0\ng body\nf 1 2 3 4\n";
        assert_eq!(body_mesh(&parse_obj(obj), "body").triangles, vec![[0, 1, 3], [1, 2, 3]]);
    }
}
```
In `xtask/src/main.rs`, add `mod makehuman;` at the top.

- [ ] **Step 3: Run and watch them fail**

Run: `cargo nextest run -p xtask`
Expected: 4 FAIL with `not yet implemented`.

- [ ] **Step 4: Implement the conversion**

```rust
pub fn parse_obj(text: &str) -> ObjMesh {
    let (mut verts, mut faces, mut groups) = (vec![], vec![], vec![]);
    let mut group = String::new();
    for line in text.lines() {
        let mut parts = line.split_whitespace();
        match parts.next() {
            Some("v") => {
                let c: Vec<f64> = parts.take(3).map(|s| s.parse().expect("vertex coordinate")).collect();
                verts.push([c[0], c[1], c[2]]);
            }
            Some("f") => {
                faces.push(
                    parts
                        .map(|s| s.split('/').next().unwrap().parse::<u32>().expect("face index") - 1)
                        .collect(),
                );
                groups.push(group.clone());
            }
            Some("g") => group = parts.collect::<Vec<_>>().join(" "),
            _ => {}
        }
    }
    ObjMesh { verts, faces, groups }
}

pub fn apply_target(verts: &mut [[f64; 3]], target: &str, weight: f64) -> usize {
    let mut applied = 0;
    for line in target.lines().map(str::trim).filter(|l| !l.is_empty() && !l.starts_with('#')) {
        let p: Vec<&str> = line.split_whitespace().collect();
        let i: usize = p[0].parse().expect("target vertex index");
        for k in 0..3 {
            verts[i][k] += weight * p[k + 1].parse::<f64>().expect("target offset");
        }
        applied += 1;
    }
    applied
}

pub fn body_mesh(obj: &ObjMesh, group: &str) -> BodyMesh {
    let faces: Vec<&Vec<u32>> = obj.faces.iter().zip(&obj.groups).filter(|(_, g)| *g == group).map(|(f, _)| f).collect();
    let mut used: Vec<u32> = faces.iter().flat_map(|f| f.iter().copied()).collect();
    used.sort_unstable();
    used.dedup();
    let mut remap = vec![u32::MAX; obj.verts.len()];
    for (new, &old) in used.iter().enumerate() {
        remap[old as usize] = new as u32;
    }
    let d2 = |a: u32, b: u32| -> f64 {
        let (p, q) = (obj.verts[a as usize], obj.verts[b as usize]);
        (0..3).map(|k| (p[k] - q[k]).powi(2)).sum()
    };
    let mut triangles = vec![];
    for f in faces {
        let r = |k: u32| remap[k as usize];
        match f.as_slice() {
            &[a, b, c] => triangles.push([r(a), r(b), r(c)]),
            &[a, b, c, d] if d2(a, c) <= d2(b, d) => triangles.extend([[r(a), r(b), r(c)], [r(a), r(c), r(d)]]),
            &[a, b, c, d] => triangles.extend([[r(a), r(b), r(d)], [r(b), r(c), r(d)]]),
            poly => triangles.extend((1..poly.len() - 1).map(|k| [r(poly[0]), r(poly[k]), r(poly[k + 1])])),
        }
    }
    // Scale in f64, then convert: identical bits on every platform (CI byte-compares the asset).
    let mut positions: Vec<glam::Vec3> =
        used.iter().map(|&i| (glam::DVec3::from_array(obj.verts[i as usize]) * 0.1).as_vec3()).collect();
    let floor = positions.iter().map(|p| p.y).fold(f32::MAX, f32::min);
    for p in &mut positions {
        p.y -= floor;
    }
    BodyMesh { positions, triangles }
}
```

Add the subcommand to `xtask/src/main.rs`'s `match`:
```rust
        Some("body") => body(),
```
and:
```rust
/// `cargo xtask body <makehuman_dir> <out.odb>`: average female body from CC0 MakeHuman data.
fn body() {
    let args: Vec<String> = std::env::args().skip(2).collect();
    let [dir, out] = args.as_slice() else {
        eprintln!("usage: cargo xtask body <makehuman_dir> <out.odb>   (run scripts/fetch-makehuman.sh first)");
        std::process::exit(2);
    };
    let dir = std::path::Path::new(dir);
    let read = |name: &str| std::fs::read_to_string(dir.join(name)).unwrap_or_else(|e| panic!("{name}: {e}"));
    let mut obj = makehuman::parse_obj(&read("base.obj"));
    for (name, weight) in makehuman::FEMALE_AVERAGE {
        let n = makehuman::apply_target(&mut obj.verts, &read(name), weight);
        println!("applied {name} × {weight:.4} ({n} vertices)");
    }
    let body = makehuman::body_mesh(&obj, "body");
    std::fs::write(out, opendrape_body::write_odb(&body)).expect("write .odb");
    println!(
        "wrote {out}: {} vertices, {} triangles, height {:.3} m, waist {:.3} m, hips {:.3} m, open edges {}",
        body.positions.len(),
        body.triangles.len(),
        opendrape_body::height(&body),
        opendrape_body::girth_at(&body, 1.028, 0.2),
        opendrape_body::girth_at(&body, 0.767, 0.2),
        opendrape_body::boundary_edge_count(&body),
    );
}
```
Also update the usage message in `main` to `usage: cargo xtask <icons|body>`.

- [ ] **Step 5: Run the tests and generate the asset**

Run: `cargo nextest run -p xtask`. Expected: 4 passed.

Then run:
```bash
mkdir -p assets/body
./scripts/fetch-makehuman.sh
cargo xtask body target/makehuman assets/body/female_average.odb
```
Expected:
- three `applied …` lines;
- `wrote …: 13380 vertices, 26756 triangles, height 1.590 m`;
- waist ≈ 0.65 m and hips ≈ 0.95 m;
- `open edges 0`.

These match the research measurements: 159.0 cm, waist 65.2 cm, hips 94.6 cm.

- [ ] **Step 6: Bundle the asset, with failing tests first**

`crates/body/tests/asset.rs`:
```rust
use opendrape_body::{BodyMesh, boundary_edge_count, girth_at, height};

#[test]
fn bundled_female_body_is_the_expected_closed_mesh() {
    let b = BodyMesh::female_average();
    assert_eq!((b.positions.len(), b.triangles.len()), (13380, 26756));
    assert_eq!(boundary_edge_count(&b), 0, "must be closed for inside/outside tests");
    let h = height(&b);
    assert!((1.585..1.595).contains(&h), "height {h}");
    assert_eq!(b.positions.iter().map(|p| p.y).fold(f32::MAX, f32::min), 0.0, "feet on the floor");
}

#[test]
fn bundled_female_body_has_plausible_girths() {
    let b = BodyMesh::female_average();
    let waist = girth_at(&b, 1.028, 0.2);
    let hips = girth_at(&b, 0.767, 0.2);
    assert!((0.63..0.68).contains(&waist), "waist {waist}");
    assert!((0.92..0.97).contains(&hips), "hips {hips}");
}
```
Run `cargo nextest run -p opendrape-body --test asset`. Expected: compile error, `no function female_average`.

Add to `crates/body/src/lib.rs`:
```rust
impl BodyMesh {
    /// Average young adult female in A-pose, from CC0 MakeHuman data (see ASSETS.md).
    pub fn female_average() -> Self {
        read_odb(include_bytes!("../../../assets/body/female_average.odb")).expect("bundled body asset is valid")
    }
}
```
Run again. Expected: 2 passed.

- [ ] **Step 7: Provenance, plus a CI job that rebuilds and byte-compares the asset**

Append a row to the table in `ASSETS.md`:
```
| `assets/body/female_average.odb` | Built by `cargo xtask body` from MakeHuman v1.3.0 (commit 1f508f60) `base.obj` + `african/asian/caucasian-female-young.target` (⅓ each), fetched and SHA-256-checked by `scripts/fetch-makehuman.sh` | CC0 1.0 (makehuman LICENSE.md §C) |
```
Add a job to `.github/workflows/ci.yml` (same action pins as the existing jobs):
```yaml
  assets:
    runs-on: ubuntu-24.04
    steps:
      - uses: actions/checkout@v5
      - run: rustup show
      - uses: Swatinem/rust-cache@6323deb102c322ba6fcbdcafc7e3dddab59af2b6 # v2.9.2
      - name: Rebuild the body from its CC0 sources and compare with the committed file
        run: |
          ./scripts/fetch-makehuman.sh
          cargo xtask body target/makehuman target/female_average.odb
          cmp target/female_average.odb assets/body/female_average.odb
```
Run locally: `cargo xtask body target/makehuman target/check.odb && cmp target/check.odb assets/body/female_average.odb`. Expected: no output (identical).

- [ ] **Step 8: Commit**

```bash
cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings && cargo deny check
git add scripts/fetch-makehuman.sh xtask crates/body assets/body ASSETS.md .github/workflows/ci.yml Cargo.lock
git commit -m "feat(body): bundle a CC0 average female body built reproducibly from MakeHuman data

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 3: `opendrape-sim`: cloth building and the XPBD solver (no collision yet)

**Files:**
- Create: `crates/sim/Cargo.toml`, `crates/sim/src/{lib.rs,cloth.rs,solver.rs,collide.rs}`. In this task, `collide.rs` holds only `Plane` and `Collider`.
- Modify: root `Cargo.toml` (add `crates/sim` to members)

**Interfaces:**
- Produces:
  - `Panel { positions: Vec<DVec3>, flat: Option<Vec<DVec2>>, triangles: Vec<[u32;3]> }` (Default).
  - `ClothBuilder::new(density) -> Self`, plus `add_panel(&Panel, rest_scale: f64) -> PanelId`, `stitch((PanelId,u32),(PanelId,u32))`, `pin((PanelId,u32))` and `build() -> Cloth`.
  - `Cloth` accessors:
    - `positions() -> &[DVec3]`, `velocities() -> &[DVec3]`, `triangles() -> &[[u32;3]]`;
    - `len()`, `is_alive(i)`, `is_pinned(i)`, `mass(i) -> f64`, `total_mass() -> f64`;
    - `stretch_links() -> impl Iterator<(usize,usize,f64)>`, `bend_link_count()`, `stitch_pairs() -> impl Iterator<(usize,usize)>`;
    - `has_open_stitches()`, `topology_version() -> u64`, `weld_stitches()`.
  - `Params` (pub fields; `Default` = the Global Constraints values), `Solver::new(Cloth, Params)`, `cloth()`, `params()`, `time()`, `step(Option<&dyn Collider>)`, and `pub const FRAME_DT: f64 = 1.0/60.0`.
  - `Plane { normal: DVec3, point: DVec3 }` and `trait Collider: Sync { fn contact_planes(&self, x: &[DVec3], margin: f64) -> Vec<Option<Plane>>; }`.

- [ ] **Step 1: Crate, types and failing tests**

Add `"crates/sim"` to `members`. `crates/sim/Cargo.toml`:
```toml
[package]
name = "opendrape-sim"
description = "OpenDrape XPBD cloth simulation"
version.workspace = true
edition.workspace = true
rust-version.workspace = true
license.workspace = true
repository.workspace = true
publish.workspace = true

[dependencies]
glam.workspace = true
parry3d.workspace = true
rayon.workspace = true

[lints]
workspace = true
```

`crates/sim/src/lib.rs`:
```rust
//! XPBD cloth simulation on the CPU in f64: fabric stretch and bending, seams that pull shut
//! and then weld, and collision against a static body.

mod cloth;
mod collide;
mod solver;

pub use cloth::{Cloth, ClothBuilder, Panel, PanelId};
pub use collide::{Collider, Plane};
pub use solver::{FRAME_DT, Params, Solver};
```

`crates/sim/src/collide.rs` (Task 4 extends it):
```rust
use glam::DVec3;

/// A contact plane for one particle: the closest point on the body and the outward normal there.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Plane {
    pub normal: DVec3,
    pub point: DVec3,
}

/// Something cloth collides with. Queried once per frame for every particle; `None` means
/// the particle is farther than `margin` from it (and outside).
pub trait Collider: Sync {
    fn contact_planes(&self, x: &[DVec3], margin: f64) -> Vec<Option<Plane>>;
}
```

`crates/sim/src/cloth.rs`:
```rust
use glam::{DVec2, DVec3};
use std::collections::{HashMap, HashSet};

/// One piece of fabric: a triangle mesh, its initial 3D placement, and optionally its flat
/// pattern shape (metres), which defines rest lengths and mass. Without `flat`, the 3D
/// `positions` define them.
#[derive(Clone, Debug, Default)]
pub struct Panel {
    pub positions: Vec<DVec3>,
    pub flat: Option<Vec<DVec2>>,
    pub triangles: Vec<[u32; 3]>,
}

/// Index of a panel's first particle inside the cloth.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PanelId(u32);

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Link {
    pub a: u32,
    pub b: u32,
    pub rest: f64,
}

/// All fabric being simulated, as particles plus distance constraints.
#[derive(Clone, Debug, Default)]
pub struct Cloth {
    pub(crate) x: Vec<DVec3>,
    pub(crate) prev: Vec<DVec3>,
    pub(crate) v: Vec<DVec3>,
    pub(crate) inv_mass: Vec<f64>,
    pub(crate) alive: Vec<bool>,
    pub(crate) triangles: Vec<[u32; 3]>,
    pub(crate) stretch: Vec<Link>,
    pub(crate) bend: Vec<Link>,
    /// `rest` = distance when the seam was made; it shrinks to 0 while the seam closes.
    pub(crate) stitches: Vec<Link>,
    pub(crate) topology_version: u64,
}

pub struct ClothBuilder {
    cloth: Cloth,
    density: f64,
}

fn edge_key(a: u32, b: u32) -> (u32, u32) {
    (a.min(b), a.max(b))
}

/// Unique edges of a triangle list, sorted.
fn unique_edges(triangles: &[[u32; 3]]) -> Vec<(u32, u32)> {
    let mut e: Vec<_> = triangles.iter().flat_map(|t| (0..3).map(move |k| edge_key(t[k], t[(k + 1) % 3]))).collect();
    e.sort_unstable();
    e.dedup();
    e
}

/// For every edge shared by exactly two triangles, the two vertices opposite it (sorted by edge).
fn bending_pairs(triangles: &[[u32; 3]]) -> Vec<(u32, u32)> {
    let mut opposite: HashMap<(u32, u32), Vec<u32>> = HashMap::new();
    for t in triangles {
        for k in 0..3 {
            opposite.entry(edge_key(t[k], t[(k + 1) % 3])).or_default().push(t[(k + 2) % 3]);
        }
    }
    let mut keys: Vec<_> = opposite.keys().copied().collect();
    keys.sort_unstable();
    keys.into_iter()
        .filter_map(|k| match opposite[&k].as_slice() {
            [p, q] => Some((*p, *q)),
            _ => None,
        })
        .collect()
}

impl ClothBuilder {
    /// `density`: fabric weight in kg/m².
    pub fn new(density: f64) -> Self {
        Self { cloth: Cloth::default(), density }
    }

    /// Adds a panel; its rest lengths are multiplied by `rest_scale` (< 1 pre-tensions it).
    pub fn add_panel(&mut self, _panel: &Panel, _rest_scale: f64) -> PanelId {
        todo!()
    }

    /// Sews particle `a` to particle `b`: pulled together over `Params::stitch_close_time`.
    pub fn stitch(&mut self, _a: (PanelId, u32), _b: (PanelId, u32)) {
        todo!()
    }

    /// Fixes a particle in space.
    pub fn pin(&mut self, _p: (PanelId, u32)) {
        todo!()
    }

    pub fn build(self) -> Cloth {
        todo!()
    }
}

impl Cloth {
    pub fn positions(&self) -> &[DVec3] {
        &self.x
    }
    pub fn velocities(&self) -> &[DVec3] {
        &self.v
    }
    pub fn triangles(&self) -> &[[u32; 3]] {
        &self.triangles
    }
    pub fn len(&self) -> usize {
        self.x.len()
    }
    pub fn is_empty(&self) -> bool {
        self.x.is_empty()
    }
    /// False for particles merged away by welding (and unused vertices).
    pub fn is_alive(&self, i: usize) -> bool {
        self.alive[i]
    }
    pub fn is_pinned(&self, i: usize) -> bool {
        self.alive[i] && self.inv_mass[i] == 0.0
    }
    pub fn mass(&self, i: usize) -> f64 {
        if self.inv_mass[i] > 0.0 { 1.0 / self.inv_mass[i] } else { f64::INFINITY }
    }
    /// Mass of all movable particles.
    pub fn total_mass(&self) -> f64 {
        (0..self.len()).filter(|&i| self.alive[i] && self.inv_mass[i] > 0.0).map(|i| self.mass(i)).sum()
    }
    pub fn stretch_links(&self) -> impl Iterator<Item = (usize, usize, f64)> + '_ {
        self.stretch.iter().map(|l| (l.a as usize, l.b as usize, l.rest))
    }
    pub fn bend_link_count(&self) -> usize {
        self.bend.len()
    }
    pub fn stitch_pairs(&self) -> impl Iterator<Item = (usize, usize)> + '_ {
        self.stitches.iter().map(|l| (l.a as usize, l.b as usize))
    }
    pub fn has_open_stitches(&self) -> bool {
        !self.stitches.is_empty()
    }
    /// Increases whenever `triangles()` change (welding).
    pub fn topology_version(&self) -> u64 {
        self.topology_version
    }

    /// Merges each stitched pair into one particle and rebuilds the constraints on the welded
    /// mesh, so a closed seam behaves like continuous fabric with zero gap.
    pub fn weld_stitches(&mut self) {
        todo!()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Right triangle with 10 cm legs in the pattern, placed `scale`× larger in 3D at `offset`.
    pub(crate) fn tri_panel(scale: f64, offset: DVec3) -> Panel {
        let flat = vec![DVec2::new(0.0, 0.0), DVec2::new(0.1, 0.0), DVec2::new(0.0, 0.1)];
        Panel {
            positions: flat.iter().map(|p| p.extend(0.0) * scale + offset).collect(),
            flat: Some(flat),
            triangles: vec![[0, 1, 2]],
        }
    }

    #[test]
    fn rest_lengths_and_mass_come_from_the_flat_pattern() {
        let mut b = ClothBuilder::new(0.15);
        b.add_panel(&tri_panel(2.0, DVec3::ZERO), 0.97);
        let c = b.build();
        let mut rests: Vec<f64> = c.stretch_links().map(|(_, _, r)| r).collect();
        rests.sort_by(f64::total_cmp);
        assert!((rests[0] - 0.097).abs() < 1e-12 && (rests[2] - 0.1f64.hypot(0.1) * 0.97).abs() < 1e-12, "{rests:?}");
        assert!((c.mass(0) - 0.15 * 0.005 / 3.0).abs() < 1e-15);
    }

    #[test]
    fn two_triangles_sharing_an_edge_get_one_bending_link() {
        let panel = Panel {
            positions: vec![DVec3::ZERO, DVec3::X, DVec3::Y, DVec3::new(1.0, 1.0, 0.0)],
            flat: None,
            triangles: vec![[0, 1, 2], [1, 3, 2]],
        };
        let mut b = ClothBuilder::new(0.15);
        b.add_panel(&panel, 1.0);
        let c = b.build();
        assert_eq!(c.stretch_links().count(), 5);
        assert_eq!(c.bend_link_count(), 1);
    }

    #[test]
    fn welding_merges_stitched_pairs_and_keeps_mass() {
        let mut b = ClothBuilder::new(0.15);
        let p = b.add_panel(&tri_panel(1.0, DVec3::ZERO), 1.0);
        let q = b.add_panel(&tri_panel(1.0, DVec3::new(0.0, 0.0, 0.2)), 1.0);
        for k in 0..2 {
            b.stitch((p, k), (q, k));
        }
        let mut c = b.build();
        let mass = c.total_mass();
        c.weld_stitches();
        assert!(!c.has_open_stitches());
        assert_eq!((0..c.len()).filter(|&i| c.is_alive(i)).count(), 4);
        assert!((c.total_mass() - mass).abs() < 1e-15);
        assert!(c.triangles().iter().flatten().all(|&k| c.is_alive(k as usize)), "triangles only use live particles");
        assert_eq!(c.topology_version(), 1);
        // The two triangles now share the welded edge, so a bending link spans it.
        assert_eq!(c.bend_link_count(), 1);
    }
}
```

`crates/sim/src/solver.rs`:
```rust
use crate::cloth::{Cloth, Link};
use crate::collide::{Collider, Plane};
use glam::DVec3;

/// One simulation frame.
pub const FRAME_DT: f64 = 1.0 / 60.0;

/// Simulation settings. `Default` is the set validated by the M1 prototype.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Params {
    pub substeps: usize,
    pub iterations: usize,
    /// m/s² along Y.
    pub gravity: f64,
    /// Seconds without gravity at the start, so seams can close first.
    pub gravity_delay: f64,
    /// Seconds over which gravity then ramps to full.
    pub gravity_ramp: f64,
    /// Seconds over which stitched seams pull shut.
    pub stitch_close_time: f64,
    /// When to weld closed seams (None: never).
    pub weld_time: Option<f64>,
    /// XPBD compliance (m/N) of fabric edges and of bending.
    pub stretch_compliance: f64,
    pub bend_compliance: f64,
    /// Velocity damping per second.
    pub damping: f64,
    pub friction: f64,
    /// Distance kept between cloth and body (m).
    pub thickness: f64,
    /// Contact planes are created for particles within this distance of the body (m).
    pub collision_margin: f64,
    pub max_speed: f64,
}

impl Default for Params {
    fn default() -> Self {
        Self {
            substeps: 20,
            iterations: 2,
            gravity: -9.81,
            gravity_delay: 0.6,
            gravity_ramp: 0.3,
            stitch_close_time: 0.5,
            weld_time: Some(0.8),
            stretch_compliance: 1e-6,
            bend_compliance: 1.0,
            damping: 1.0,
            friction: 0.4,
            thickness: 0.003,
            collision_margin: 0.05,
            max_speed: 2.0,
        }
    }
}

pub struct Solver {
    cloth: Cloth,
    params: Params,
    time: f64,
}

impl Solver {
    pub fn new(cloth: Cloth, params: Params) -> Self {
        Self { cloth, params, time: 0.0 }
    }
    pub fn cloth(&self) -> &Cloth {
        &self.cloth
    }
    pub fn params(&self) -> &Params {
        &self.params
    }
    /// Simulated seconds so far.
    pub fn time(&self) -> f64 {
        self.time
    }

    /// Advances one [`FRAME_DT`] frame.
    pub fn step(&mut self, _collider: Option<&dyn Collider>) {
        todo!()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cloth::{ClothBuilder, Panel};
    use glam::DVec2;

    fn single(scale: f64, offset: DVec3, pin: Option<u32>) -> Cloth {
        let flat = vec![DVec2::new(0.0, 0.0), DVec2::new(0.1, 0.0), DVec2::new(0.0, 0.1)];
        let panel = Panel {
            positions: flat.iter().map(|p| p.extend(0.0) * scale + offset).collect(),
            flat: Some(flat),
            triangles: vec![[0, 1, 2]],
        };
        let mut b = ClothBuilder::new(0.15);
        let id = b.add_panel(&panel, 1.0);
        if let Some(k) = pin {
            b.pin((id, k));
        }
        b.build()
    }

    fn no_gravity() -> Params {
        Params { gravity: 0.0, ..Params::default() }
    }

    #[test]
    fn a_stretched_triangle_returns_to_its_rest_shape() {
        let mut s = Solver::new(single(1.1, DVec3::ZERO, None), no_gravity());
        for _ in 0..60 {
            s.step(None);
        }
        let x = s.cloth().positions();
        for (a, b, r) in s.cloth().stretch_links() {
            let err = ((x[a] - x[b]).length() - r).abs() / r;
            assert!(err < 0.01, "edge {a}-{b} off by {:.2}%", err * 100.0);
        }
    }

    #[test]
    fn free_fall_follows_gravity() {
        let params = Params { gravity_delay: 0.0, gravity_ramp: 0.0, damping: 0.0, max_speed: 1e9, ..Params::default() };
        let mut s = Solver::new(single(1.0, DVec3::ZERO, None), params);
        for _ in 0..30 {
            s.step(None);
        }
        let expected = -0.5 * 9.81 * 0.5f64.powi(2);
        let y = s.cloth().positions()[0].y;
        assert!((y - expected).abs() / expected.abs() < 0.01, "y {y} vs {expected}");
    }

    #[test]
    fn pinned_particles_never_move() {
        let mut s = Solver::new(single(1.0, DVec3::ZERO, Some(0)), Params::default());
        for _ in 0..90 {
            s.step(None);
        }
        assert_eq!(s.cloth().positions()[0], DVec3::ZERO);
        assert!(s.cloth().positions()[1].y < -0.01, "the rest swings down");
    }

    #[test]
    fn stitched_panels_close_then_weld() {
        let mut b = ClothBuilder::new(0.15);
        let flat = vec![DVec2::new(0.0, 0.0), DVec2::new(0.1, 0.0), DVec2::new(0.0, 0.1)];
        let panel = |z: f64| Panel {
            positions: flat.iter().map(|p| p.extend(z)).collect(),
            flat: Some(flat.clone()),
            triangles: vec![[0, 1, 2]],
        };
        let (p, q) = (b.add_panel(&panel(0.0), 1.0), b.add_panel(&panel(0.1), 1.0));
        b.stitch((p, 0), (q, 0));
        b.stitch((p, 1), (q, 1));
        let mut s = Solver::new(b.build(), no_gravity());
        for _ in 0..36 {
            s.step(None); // 0.6 s: past stitch_close_time 0.5 s, so the seam has closed
        }
        let x = s.cloth().positions();
        assert!(s.cloth().stitch_pairs().all(|(a, b)| (x[a] - x[b]).length() < 1e-3));
        for _ in 0..24 {
            s.step(None); // 1.0 s: past weld_time 0.8 s
        }
        assert!(!s.cloth().has_open_stitches());
        assert!(s.cloth().positions().iter().all(|p| p.is_finite()));
    }

    #[test]
    fn degenerate_triangle_does_not_produce_nan() {
        let panel = Panel {
            positions: vec![DVec3::ZERO, DVec3::X * 0.1, DVec3::X * 0.2, DVec3::new(0.0, 0.1, 0.0)],
            flat: None,
            triangles: vec![[0, 1, 2], [0, 1, 3]], // [0,1,2] is collinear (zero area)
        };
        let mut b = ClothBuilder::new(0.15);
        b.add_panel(&panel, 1.0);
        let mut s = Solver::new(b.build(), Params::default());
        for _ in 0..120 {
            s.step(None);
        }
        assert!(s.cloth().positions().iter().all(|p| p.is_finite()));
    }

    #[test]
    fn same_inputs_give_identical_results() {
        let run = || {
            let mut s = Solver::new(single(1.05, DVec3::ZERO, Some(0)), Params::default());
            for _ in 0..60 {
                s.step(None);
            }
            s.cloth().positions().to_vec()
        };
        assert_eq!(run(), run());
    }
}
```

- [ ] **Step 2: Run and watch them fail**

Run: `cargo nextest run -p opendrape-sim`
Expected: 9 FAIL with `not yet implemented`.

- [ ] **Step 3: Implement the builder and welding (`cloth.rs`)**

```rust
    pub fn add_panel(&mut self, panel: &Panel, rest_scale: f64) -> PanelId {
        let base = self.cloth.x.len() as u32;
        let rest_pos = |k: u32| -> DVec3 {
            match &panel.flat {
                Some(f) => f[k as usize].extend(0.0),
                None => panel.positions[k as usize],
            }
        };
        let mut mass = vec![0.0; panel.positions.len()];
        for t in &panel.triangles {
            let [a, b, c] = t.map(rest_pos);
            let area = 0.5 * (b - a).cross(c - a).length();
            for &k in t {
                mass[k as usize] += self.density * area / 3.0;
            }
        }
        self.cloth.x.extend_from_slice(&panel.positions);
        for m in mass {
            // A vertex with no area (unused, or only in degenerate triangles) can't move.
            self.cloth.inv_mass.push(if m > 0.0 { 1.0 / m } else { 0.0 });
            self.cloth.alive.push(m > 0.0);
        }
        let link = |(a, b): (u32, u32)| Link { a: a + base, b: b + base, rest: rest_pos(a).distance(rest_pos(b)) * rest_scale };
        self.cloth.stretch.extend(unique_edges(&panel.triangles).into_iter().map(link));
        self.cloth.bend.extend(bending_pairs(&panel.triangles).into_iter().map(link));
        self.cloth.triangles.extend(panel.triangles.iter().map(|t| t.map(|k| k + base)));
        PanelId(base)
    }

    pub fn stitch(&mut self, a: (PanelId, u32), b: (PanelId, u32)) {
        let (i, j) = (a.0.0 + a.1, b.0.0 + b.1);
        let rest = self.cloth.x[i as usize].distance(self.cloth.x[j as usize]);
        self.cloth.stitches.push(Link { a: i, b: j, rest });
    }

    pub fn pin(&mut self, p: (PanelId, u32)) {
        self.cloth.inv_mass[(p.0.0 + p.1) as usize] = 0.0;
    }

    pub fn build(mut self) -> Cloth {
        self.cloth.prev = self.cloth.x.clone();
        self.cloth.v = vec![DVec3::ZERO; self.cloth.x.len()];
        self.cloth
    }
```
`weld_stitches`:
```rust
    pub fn weld_stitches(&mut self) {
        if self.stitches.is_empty() {
            return;
        }
        let mut map: Vec<u32> = (0..self.x.len() as u32).collect();
        fn root(map: &[u32], mut k: u32) -> u32 {
            while map[k as usize] != k {
                k = map[k as usize];
            }
            k
        }
        for s in std::mem::take(&mut self.stitches) {
            let (a, b) = (root(&map, s.a) as usize, root(&map, s.b) as usize);
            if a == b {
                continue;
            }
            let (wa, wb) = (self.inv_mass[a], self.inv_mass[b]);
            self.x[a] = if wa == 0.0 { self.x[a] } else if wb == 0.0 { self.x[b] } else { (self.x[a] + self.x[b]) * 0.5 };
            self.v[a] = (self.v[a] + self.v[b]) * 0.5;
            self.inv_mass[a] = if wa == 0.0 || wb == 0.0 { 0.0 } else { 1.0 / (1.0 / wa + 1.0 / wb) };
            self.prev[a] = self.x[a];
            map[b] = a as u32;
            self.alive[b] = false;
            self.inv_mass[b] = 0.0;
            self.v[b] = DVec3::ZERO;
        }
        let m = |k: u32| root(&map, k);
        for t in &mut self.triangles {
            *t = t.map(m);
        }
        let mut seen = HashSet::new();
        self.stretch = std::mem::take(&mut self.stretch)
            .into_iter()
            .map(|l| Link { a: m(l.a), b: m(l.b), rest: l.rest })
            .filter(|l| l.a != l.b && seen.insert(edge_key(l.a, l.b)))
            .collect();
        let old: HashMap<(u32, u32), f64> = self.bend.iter().map(|l| (edge_key(m(l.a), m(l.b)), l.rest)).collect();
        let x = &self.x;
        self.bend = bending_pairs(&self.triangles)
            .into_iter()
            .map(|(a, b)| Link {
                a,
                b,
                rest: old.get(&edge_key(a, b)).copied().unwrap_or_else(|| x[a as usize].distance(x[b as usize])),
            })
            .collect();
        self.topology_version += 1;
    }
```

- [ ] **Step 4: Implement the solver step (`solver.rs`)**

```rust
    pub fn step(&mut self, collider: Option<&dyn Collider>) {
        let p = self.params;
        let t = self.time;
        if p.weld_time.is_some_and(|tw| t >= tw) && self.cloth.has_open_stitches() {
            self.cloth.weld_stitches();
        }
        let gravity = if t < p.gravity_delay {
            0.0
        } else if p.gravity_ramp <= 0.0 {
            p.gravity
        } else {
            ((t - p.gravity_delay) / p.gravity_ramp).min(1.0) * p.gravity
        };
        let stitch_scale = if p.stitch_close_time > 0.0 { (1.0 - t / p.stitch_close_time).max(0.0) } else { 0.0 };
        let planes = collider.map(|c| c.contact_planes(&self.cloth.x, p.collision_margin));
        let sdt = FRAME_DT / p.substeps as f64;
        let c = &mut self.cloth;
        for _ in 0..p.substeps {
            for i in 0..c.x.len() {
                if c.inv_mass[i] == 0.0 {
                    c.prev[i] = c.x[i];
                    continue;
                }
                let mut v = c.v[i];
                v.y += gravity * sdt;
                v *= 1.0 - p.damping * sdt;
                let speed = v.length();
                if speed > p.max_speed {
                    v *= p.max_speed / speed;
                }
                c.prev[i] = c.x[i];
                c.x[i] += v * sdt;
            }
            // Stitches first, fabric last: the fabric constraints get the final word.
            for _ in 0..p.iterations {
                solve_links(&mut c.x, &c.inv_mass, &c.stitches, 0.0, stitch_scale, sdt);
                solve_links(&mut c.x, &c.inv_mass, &c.bend, p.bend_compliance, 1.0, sdt);
                solve_links(&mut c.x, &c.inv_mass, &c.stretch, p.stretch_compliance, 1.0, sdt);
            }
            if let Some(planes) = &planes {
                collide(c, planes, p.thickness, p.friction);
            }
            for i in 0..c.x.len() {
                if c.inv_mass[i] > 0.0 {
                    c.v[i] = (c.x[i] - c.prev[i]) / sdt;
                }
            }
        }
        self.time += FRAME_DT;
    }
```
Below the `impl`:
```rust
/// One Gauss–Seidel pass of XPBD distance constraints (Macklin, Müller, Chentanez 2016).
fn solve_links(x: &mut [DVec3], w: &[f64], links: &[Link], compliance: f64, rest_scale: f64, sdt: f64) {
    let alpha = compliance / (sdt * sdt);
    for l in links {
        let (a, b) = (l.a as usize, l.b as usize);
        let wsum = w[a] + w[b];
        if wsum == 0.0 {
            continue;
        }
        let d = x[a] - x[b];
        let len = d.length();
        if len < 1e-12 {
            continue;
        }
        let lambda = -(len - l.rest * rest_scale) / (wsum + alpha);
        let corr = d * (lambda / len);
        x[a] += corr * w[a];
        x[b] -= corr * w[b];
    }
}

/// Pushes particles out to `thickness` above their contact plane, then applies Coulomb-style
/// friction to this substep's sliding (static when the slide is small).
fn collide(c: &mut Cloth, planes: &[Option<Plane>], thickness: f64, friction: f64) {
    for (i, plane) in planes.iter().enumerate() {
        let Some(pl) = plane else { continue };
        if c.inv_mass[i] == 0.0 {
            continue;
        }
        let depth = pl.normal.dot(c.x[i] - pl.point) - thickness;
        if depth >= 0.0 {
            continue;
        }
        c.x[i] -= pl.normal * depth;
        let moved = c.x[i] - c.prev[i];
        let slide = moved - pl.normal * pl.normal.dot(moved);
        let len = slide.length();
        let limit = friction * -depth;
        if len <= limit {
            c.x[i] -= slide;
        } else if len > 0.0 {
            c.x[i] -= slide * (limit / len);
        }
    }
}
```

- [ ] **Step 5: Run and see them pass**

Run: `cargo nextest run -p opendrape-sim`
Expected: 9 passed.

- [ ] **Step 6: Commit**

```bash
cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings
git add Cargo.toml Cargo.lock crates/sim
git commit -m "feat(sim): XPBD cloth with stitches that close and weld

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 4: Body collision (`BodyCollider`)

**Files:**
- Modify: `crates/sim/src/collide.rs`, `crates/sim/src/lib.rs`

**Interfaces:**
- Consumes: `Plane`, `Collider`, `Solver::step` (Task 3).
- Produces:
  - `BodyCollider::new(&[glam::Vec3], &[[u32;3]]) -> Result<BodyCollider, ColliderError>`;
  - `signed_distance(DVec3) -> f64` (negative inside);
  - `ray_exit(origin: DVec3, dir: DVec3, max: f64) -> Option<f64>`;
  - `impl Collider for BodyCollider`;
  - `ColliderError(pub String)`.

- [ ] **Step 1: Failing tests**

Append to `collide.rs`:
```rust
use parry3d::query::{PointQuery, PointQueryWithLocation, Ray, RayCast};
use parry3d::shape::{TriMesh, TriMeshFlags};
use rayon::prelude::*;

#[derive(Debug)]
pub struct ColliderError(pub String);

/// Exact collision against a closed, consistently wound triangle mesh (the body).
pub struct BodyCollider {
    mesh: TriMesh,
}

impl BodyCollider {
    pub fn new(_positions: &[glam::Vec3], _triangles: &[[u32; 3]]) -> Result<Self, ColliderError> {
        todo!()
    }
    /// Distance to the surface, negative inside.
    pub fn signed_distance(&self, _p: DVec3) -> f64 {
        todo!()
    }
    /// Distance from `origin` (inside the body) along unit `dir` to where the ray leaves it.
    pub fn ray_exit(&self, _origin: DVec3, _dir: DVec3, _max: f64) -> Option<f64> {
        todo!()
    }
}

impl Collider for BodyCollider {
    fn contact_planes(&self, _x: &[DVec3], _margin: f64) -> Vec<Option<Plane>> {
        todo!()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ClothBuilder, Panel, Params, Solver};
    use glam::{DVec2, Vec3};

    fn cube() -> BodyCollider {
        let p = [
            [-0.5, -0.5, -0.5], [0.5, -0.5, -0.5], [0.5, 0.5, -0.5], [-0.5, 0.5, -0.5],
            [-0.5, -0.5, 0.5], [0.5, -0.5, 0.5], [0.5, 0.5, 0.5], [-0.5, 0.5, 0.5],
        ]
        .map(Vec3::from_array);
        let t = [
            [0, 3, 2], [0, 2, 1], [4, 5, 6], [4, 6, 7], [0, 4, 7], [0, 7, 3],
            [1, 2, 6], [1, 6, 5], [0, 1, 5], [0, 5, 4], [3, 7, 6], [3, 6, 2],
        ];
        BodyCollider::new(&p, &t).expect("closed cube")
    }

    #[test]
    fn planes_point_outward_inside_and_out() {
        let c = cube();
        let planes = c.contact_planes(&[DVec3::new(0.0, 0.0, 0.52), DVec3::new(0.0, 0.0, 0.3), DVec3::new(0.0, 0.0, 5.0)], 0.05);
        let outside = planes[0].expect("near the +Z face");
        assert!(outside.normal.abs_diff_eq(DVec3::Z, 1e-6) && outside.point.abs_diff_eq(DVec3::new(0.0, 0.0, 0.5), 1e-6));
        let inside = planes[1].expect("inside always gets a plane");
        assert!(inside.normal.abs_diff_eq(DVec3::Z, 1e-6), "outward even from inside: {:?}", inside.normal);
        assert_eq!(planes[2], None, "far away");
    }

    #[test]
    fn signed_distance_and_ray_exit() {
        let c = cube();
        assert!((c.signed_distance(DVec3::new(0.0, 0.0, 0.3)) + 0.2).abs() < 1e-6);
        assert!((c.signed_distance(DVec3::new(0.0, 0.0, 0.8)) - 0.3).abs() < 1e-6);
        assert!((c.ray_exit(DVec3::ZERO, DVec3::X, 2.0).unwrap() - 0.5).abs() < 1e-6);
    }

    fn falling_triangle(y: f64) -> Solver {
        let flat = vec![DVec2::new(-0.05, -0.05), DVec2::new(0.05, -0.05), DVec2::new(0.0, 0.05)];
        let panel = Panel {
            positions: flat.iter().map(|p| DVec3::new(p.x, y, p.y)).collect(),
            flat: Some(flat),
            triangles: vec![[0, 1, 2]],
        };
        let mut b = ClothBuilder::new(0.15);
        b.add_panel(&panel, 1.0);
        Solver::new(b.build(), Params { gravity_delay: 0.0, ..Params::default() })
    }

    #[test]
    fn cloth_lands_on_the_body_and_stays_outside() {
        let c = cube();
        let mut s = falling_triangle(0.6);
        for _ in 0..120 {
            s.step(Some(&c));
        }
        for p in s.cloth().positions() {
            assert!(p.y >= 0.5 + 0.003 - 1e-4 && p.y < 0.52, "resting on top: {p}");
        }
    }

    #[test]
    fn cloth_that_starts_inside_is_pushed_out() {
        let c = cube();
        let mut s = falling_triangle(0.47);
        s.step(Some(&c));
        assert!(s.cloth().positions().iter().all(|p| c.signed_distance(*p) > 0.0));
    }
}
```

- [ ] **Step 2: Run and watch them fail**

Run: `cargo nextest run -p opendrape-sim collide`
Expected: 4 FAIL with `not yet implemented`.

- [ ] **Step 3: Implement** (API verified with parry3d 0.31.1 on 2026-10-09)

```rust
    pub fn new(positions: &[glam::Vec3], triangles: &[[u32; 3]]) -> Result<Self, ColliderError> {
        let mut mesh = TriMesh::new(positions.to_vec(), triangles.to_vec()).map_err(|e| ColliderError(format!("{e:?}")))?;
        // ORIENTED is required for is_inside; `with_flags` would silently drop errors.
        mesh.set_flags(TriMeshFlags::ORIENTED | TriMeshFlags::MERGE_DUPLICATE_VERTICES)
            .map_err(|e| ColliderError(format!("{e:?}")))?;
        Ok(Self { mesh })
    }

    pub fn signed_distance(&self, p: DVec3) -> f64 {
        f64::from(self.mesh.distance_to_local_point(p.as_vec3(), false))
    }

    pub fn ray_exit(&self, origin: DVec3, dir: DVec3, max: f64) -> Option<f64> {
        let ray = Ray::new(origin.as_vec3(), dir.as_vec3());
        self.mesh.cast_local_ray(&ray, max as f32, false).map(f64::from)
    }
```
```rust
    fn contact_planes(&self, x: &[DVec3], margin: f64) -> Vec<Option<Plane>> {
        x.par_iter()
            .map(|p| {
                let (proj, (tri, _)) = self.mesh.project_local_point_and_get_location(p.as_vec3(), false);
                let point = proj.point.as_dvec3();
                let d = *p - point;
                let dist = d.length();
                if !proj.is_inside && dist > margin {
                    return None;
                }
                let normal = if dist > 1e-7 {
                    if proj.is_inside { -d / dist } else { d / dist }
                } else {
                    self.mesh.triangle(tri).normal().unwrap_or(glam::Vec3::Y).as_dvec3()
                };
                Some(Plane { normal, point })
            })
            .collect()
    }
```
Export from `lib.rs`: `pub use collide::{BodyCollider, Collider, ColliderError, Plane};`

- [ ] **Step 4: Run and see them pass**

Run: `cargo nextest run -p opendrape-sim`
Expected: 13 passed.

- [ ] **Step 5: Commit**

```bash
cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings && cargo deny check
git add crates/sim Cargo.lock
git commit -m "feat(sim): exact body collision with per-frame contact planes and friction

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 5: `opendrape-testkit`: demo garments, drape metrics, acceptance gates

**Files:**
- Create: `crates/testkit/Cargo.toml`, `crates/testkit/src/{lib.rs,garments.rs,metrics.rs}`, `crates/testkit/tests/drape.rs`, `crates/testkit/examples/drape_bench.rs`
- Modify: root `Cargo.toml` (members)

**Interfaces:**
- Consumes: `BodyMesh::female_average`; everything in `opendrape_sim`.
- Produces:
  - in `garments`: `body() -> &'static BodyMesh`, `collider() -> &'static BodyCollider`, `enum Garment { Skirt, BodiceProxy }` with `Garment::ALL`, and `Scene { garment, solver }` with `Scene::new(Garment)` and `step()`;
  - `pub const SKIRT_PARTICLES: usize = 4794` and `pub const BODICE_PARTICLES: usize = 1440`;
  - in `metrics`: `DrapeReport` (pub fields), `measure(&Cloth, &BodyCollider) -> DrapeReport`, `position_hash(&Cloth) -> u64`, `run(&mut Scene, seconds) -> f64` (ms per frame).

- [ ] **Step 1: Crate and failing acceptance tests**

Add `"crates/testkit"` to `members`. `crates/testkit/Cargo.toml`:
```toml
[package]
name = "opendrape-testkit"
description = "OpenDrape demo garments and drape-quality metrics"
version.workspace = true
edition.workspace = true
rust-version.workspace = true
license.workspace = true
repository.workspace = true
publish.workspace = true

[dependencies]
opendrape-body.workspace = true
opendrape-sim.workspace = true
glam.workspace = true
rayon.workspace = true

[lints]
workspace = true
```

`crates/testkit/src/lib.rs`:
```rust
//! Programmatic garments draped on the bundled body, and the drape-quality metrics used by
//! automated tests, the app's demo scenes and benchmarks.

pub mod garments;
pub mod metrics;
```

`crates/testkit/tests/drape.rs`:
```rust
use opendrape_testkit::garments::{BODICE_PARTICLES, Garment, SKIRT_PARTICLES, Scene, collider};
use opendrape_testkit::metrics::{measure, position_hash, run};

#[test]
fn skirt_drapes_without_poking_through_and_settles() {
    let mut scene = Scene::new(Garment::Skirt);
    assert_eq!(scene.solver.cloth().len(), SKIRT_PARTICLES);
    run(&mut scene, 6.0);
    let r = measure(scene.solver.cloth(), collider());
    eprintln!("{r:#?}");
    assert!(!r.has_nan);
    assert!(r.penetration_max_mm <= 2.0 && r.penetration_p99_mm <= 1.0, "poke-through");
    assert!(!r.open_stitches && r.seam_gap_max_mm == 0.0, "seams welded shut");
    assert!(r.strain_p99 <= 0.10 && r.strain_max <= 0.20, "fabric over-stretched");
    assert!(r.kinetic_energy <= 1e-4, "still moving: {} J", r.kinetic_energy);
    assert!(r.lowest_y > 0.4 && r.highest_y > 1.0, "skirt slid down: {}..{}", r.lowest_y, r.highest_y);
}

#[test]
fn fitted_tube_holds_close_to_the_body_without_poke_through() {
    let mut scene = Scene::new(Garment::BodiceProxy);
    assert_eq!(scene.solver.cloth().len(), BODICE_PARTICLES);
    run(&mut scene, 4.0);
    let r = measure(scene.solver.cloth(), collider());
    eprintln!("{r:#?}");
    assert!(!r.has_nan);
    assert!(r.penetration_max_mm <= 2.0, "poke-through under tension");
    assert!(r.strain_p99 <= 0.05, "tube over-stretched");
    assert!(r.kinetic_energy <= 1e-6, "still moving: {} J", r.kinetic_energy);
}

#[test]
fn drape_is_deterministic() {
    let hash = || {
        let mut s = Scene::new(Garment::Skirt);
        run(&mut s, 1.5);
        position_hash(s.solver.cloth())
    };
    assert_eq!(hash(), hash());
}
```

`crates/testkit/src/garments.rs` with stubs:
```rust
use glam::{DVec2, DVec3};
use opendrape_body::BodyMesh;
use opendrape_sim::{BodyCollider, ClothBuilder, Panel, Params, Solver};
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
    COLLIDER.get_or_init(|| BodyCollider::new(&body().positions, &body().triangles).expect("bundled body is closed"))
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
}

impl Scene {
    pub fn new(_garment: Garment) -> Self {
        todo!()
    }
    pub fn step(&mut self) {
        self.solver.step(Some(collider()));
    }
}
```

`crates/testkit/src/metrics.rs` with stubs:
```rust
use crate::garments::{Scene, collider};
use opendrape_sim::{BodyCollider, Cloth};
use rayon::prelude::*;

#[derive(Clone, Debug, Default)]
pub struct DrapeReport {
    pub particles: usize,
    pub penetration_max_mm: f64,
    pub penetration_p99_mm: f64,
    pub particles_inside: usize,
    pub open_stitches: bool,
    pub seam_gap_max_mm: f64,
    /// Fractions (0.05 = 5%) of edge stretch beyond rest length.
    pub strain_mean: f64,
    pub strain_p99: f64,
    pub strain_max: f64,
    pub kinetic_energy: f64,
    pub lowest_y: f64,
    pub highest_y: f64,
    pub has_nan: bool,
}

pub fn measure(_cloth: &Cloth, _collider: &BodyCollider) -> DrapeReport {
    todo!()
}

/// FNV-1a over the bits of every live particle position.
pub fn position_hash(_cloth: &Cloth) -> u64 {
    todo!()
}

/// Steps `scene` for `seconds` of simulated time; returns wall-clock ms per frame.
pub fn run(_scene: &mut Scene, _seconds: f64) -> f64 {
    todo!()
}
```

- [ ] **Step 2: Run and watch them fail**

Run: `cargo nextest run -p opendrape-testkit`
Expected: 3 FAIL with `not yet implemented`.

- [ ] **Step 3: Implement the garments**

In `garments.rs`:
```rust
impl Scene {
    pub fn new(garment: Garment) -> Self {
        let solver = match garment {
            Garment::Skirt => skirt(),
            Garment::BodiceProxy => bodice_proxy(),
        };
        Self { garment, solver }
    }
    // step() as above
}

/// z of the torso's centre line, measured around the hips with the arms left out.
fn torso_axis_z(body: &BodyMesh) -> f64 {
    let (lo, hi) = body
        .positions
        .iter()
        .filter(|p| p.y > 0.7 && p.y < 0.85 && p.x.abs() < 0.22)
        .fold((f32::MAX, f32::MIN), |(lo, hi), p| (lo.min(p.z), hi.max(p.z)));
    f64::from((lo + hi) / 2.0)
}

/// `rows`×`cols` quad grid split into triangles (alternating diagonals). `wrap` joins the
/// last column to the first (a tube). `flat` gives pattern coordinates for rest lengths.
fn grid_panel(rows: usize, cols: usize, wrap: bool, flat: Option<&dyn Fn(usize, usize) -> DVec2>, place: &dyn Fn(usize, usize) -> DVec3) -> Panel {
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
    let rows = (length / EDGE).round() as usize;
    let cols = (hem_w / EDGE).round() as usize;
    let flat = move |i: usize, j: usize| {
        let t = i as f64 / rows as f64;
        let half = (waist_w + (hem_w - waist_w) * t) / 2.0;
        DVec2::new(-half + 2.0 * half * j as f64 / cols as f64, -t * length)
    };
    let mut builder = ClothBuilder::new(DENSITY);
    let mut panels = vec![];
    for front in [true, false] {
        let place = |i: usize, j: usize| {
            let p = flat(i, j);
            let phi = if front { p.x / r } else { std::f64::consts::PI - p.x / r };
            DVec3::new(r * phi.sin(), SKIRT_WAIST_Y + p.y, zc + r * phi.cos())
        };
        panels.push(builder.add_panel(&grid_panel(rows, cols, false, Some(&flat), &place), 1.0));
    }
    for i in 0..=rows {
        for j in [0, cols] {
            let k = (i * (cols + 1) + j) as u32;
            builder.stitch((panels[0], k), (panels[1], k));
        }
    }
    Solver::new(builder.build(), Params::default())
}

/// Close-fit collision test: a tube shaped to the torso (1 cm ease) from waist to below the
/// armpits, rest lengths 3% short so it hugs the body, top ring held as if by shoulder straps.
fn bodice_proxy() -> Solver {
    let zc = torso_axis_z(body());
    let collider = collider();
    let (y0, y1, cols) = (0.99, 1.19, 80usize);
    let rows = ((y1 - y0) / EDGE).round() as usize;
    let place = |i: usize, j: usize| {
        let y = y1 - (y1 - y0) * i as f64 / rows as f64;
        let a = std::f64::consts::TAU * j as f64 / cols as f64;
        let dir = DVec3::new(a.sin(), 0.0, a.cos());
        let origin = DVec3::new(0.0, y, zc);
        origin + dir * (collider.ray_exit(origin, dir, 1.0).unwrap_or(0.12) + 0.01)
    };
    let mut builder = ClothBuilder::new(DENSITY);
    let tube = builder.add_panel(&grid_panel(rows, cols, true, None, &place), 0.97);
    for j in 0..cols as u32 {
        builder.pin((tube, j));
    }
    Solver::new(builder.build(), Params { gravity_delay: 0.0, weld_time: None, ..Params::default() })
}
```

- [ ] **Step 4: Implement the metrics**

```rust
pub fn measure(cloth: &Cloth, collider: &BodyCollider) -> DrapeReport {
    let x = cloth.positions();
    let live: Vec<usize> = (0..cloth.len()).filter(|&i| cloth.is_alive(i)).collect();
    let has_nan = live.iter().any(|&i| !x[i].is_finite());
    let pct = |v: &[f64], q: f64| v[((v.len() - 1) as f64 * q).round() as usize];
    let mut pen: Vec<f64> = if has_nan {
        vec![f64::NAN]
    } else {
        live.par_iter().map(|&i| (-collider.signed_distance(x[i])).max(0.0)).collect()
    };
    pen.sort_by(f64::total_cmp);
    let mut strain: Vec<f64> = cloth.stretch_links().map(|(a, b, r)| ((x[a] - x[b]).length() - r) / r).collect();
    strain.sort_by(f64::total_cmp);
    let (lowest_y, highest_y) = live.iter().fold((f64::MAX, f64::MIN), |(lo, hi), &i| (lo.min(x[i].y), hi.max(x[i].y)));
    DrapeReport {
        particles: live.len(),
        penetration_max_mm: pen[pen.len() - 1] * 1000.0,
        penetration_p99_mm: pct(&pen, 0.99) * 1000.0,
        particles_inside: pen.iter().filter(|d| **d > 0.0).count(),
        open_stitches: cloth.has_open_stitches(),
        seam_gap_max_mm: cloth.stitch_pairs().map(|(a, b)| (x[a] - x[b]).length()).fold(0.0, f64::max) * 1000.0,
        strain_mean: strain.iter().sum::<f64>() / strain.len() as f64,
        strain_p99: pct(&strain, 0.99),
        strain_max: strain[strain.len() - 1],
        kinetic_energy: live
            .iter()
            .filter(|&&i| !cloth.is_pinned(i))
            .map(|&i| 0.5 * cloth.mass(i) * cloth.velocities()[i].length_squared())
            .sum(),
        lowest_y,
        highest_y,
        has_nan,
    }
}

pub fn position_hash(cloth: &Cloth) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for (i, p) in cloth.positions().iter().enumerate() {
        if cloth.is_alive(i) {
            for c in p.to_array() {
                h = (h ^ c.to_bits()).wrapping_mul(0x0100_0000_01b3);
            }
        }
    }
    h
}

pub fn run(scene: &mut Scene, seconds: f64) -> f64 {
    let frames = (seconds / opendrape_sim::FRAME_DT).round() as usize;
    let start = std::time::Instant::now();
    for _ in 0..frames {
        scene.solver.step(Some(collider()));
    }
    start.elapsed().as_secs_f64() * 1000.0 / frames.max(1) as f64
}
```
(Remove the now-unused `use crate::garments::Scene` warning, if any, by keeping both imports in use as above.)

- [ ] **Step 5: Run and see them pass, and read the reports**

Run: `cargo nextest run -p opendrape-testkit --no-capture 2>&1 | tail -60`
Expected: 3 passed. The printed reports should be close to the prototype's:
- **skirt:** 0 mm penetration, no open stitches, strain p99 ≈ 0.076, KE ≈ 2e-7;
- **tube:** 0 mm, strain p99 ≈ 0.031.

If a gate fails, debug with superpowers:systematic-debugging against the prototype numbers in this plan's header. Don't loosen the gate.

- [ ] **Step 6: Benchmark example**

`crates/testkit/examples/drape_bench.rs`:
```rust
//! `cargo run --release -p opendrape-testkit --example drape_bench`: ms per simulated frame.
use opendrape_testkit::garments::{Garment, Scene, collider};
use opendrape_testkit::metrics::{measure, run};

fn main() {
    for g in Garment::ALL {
        let mut scene = Scene::new(g);
        let ms = run(&mut scene, 4.0);
        let r = measure(scene.solver.cloth(), collider());
        println!("{g:?}: {} particles, {ms:.2} ms/frame, penetration max {:.2} mm, strain p99 {:.1}%", r.particles, r.penetration_max_mm, r.strain_p99 * 100.0);
    }
}
```
Run it. Expected on the M4 Max: skirt ≈ 12 ms/frame and tube ≈ 3.5 ms/frame. Record the numbers in the ledger.

- [ ] **Step 7: Commit**

```bash
cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings
git add Cargo.toml Cargo.lock crates/testkit
git commit -m "feat(testkit): demo skirt and fitted tube on the body, drape metrics and acceptance gates

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 6: `MeshRenderer`: draw the body and the cloth

**Files:**
- Create: `crates/render/src/mesh.rs`, `crates/render/src/mesh.wgsl`, `crates/render/tests/mesh_render.rs`
- Modify: `crates/render/src/lib.rs`

**Interfaces:**
- Consumes: `RenderTarget`, `COLOR_FORMAT`, `DEPTH_FORMAT`, `CLEAR_COLOR`, `headless_device`, `read_back` (M0).
- Produces:
  - `MeshRenderer::new(&Device)`;
  - `create_mesh(&Device, &Queue, positions: &[Vec3], triangles: &[[u32;3]], color: [f32;3]) -> GpuMesh`;
  - `update_mesh(&Device, &Queue, &mut GpuMesh, positions: &[Vec3], triangles: Option<&[[u32;3]]>)`;
  - `render(&Device, &Queue, &RenderTarget, view_proj: Mat4, meshes: &[&GpuMesh])`;
  - `vertex_normals(&[Vec3], &[[u32;3]]) -> Vec<Vec3>`.

- [ ] **Step 1: Failing tests**

`crates/render/tests/mesh_render.rs`:
```rust
use glam::Vec3;
use opendrape_render::{CLEAR_COLOR, MeshRenderer, OrbitCamera, RenderTarget, headless_device, read_back, vertex_normals};

/// Cube with separate vertices per face (flat shading), counter-clockwise from outside.
fn flat_cube() -> (Vec<Vec3>, Vec<[u32; 3]>) {
    let faces = [(Vec3::X, Vec3::Y, Vec3::Z), (Vec3::NEG_X, Vec3::Y, Vec3::NEG_Z), (Vec3::Y, Vec3::Z, Vec3::X), (Vec3::NEG_Y, Vec3::NEG_Z, Vec3::X), (Vec3::Z, Vec3::Y, Vec3::NEG_X), (Vec3::NEG_Z, Vec3::Y, Vec3::X)];
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
    let bg = [(CLEAR_COLOR[0] * 255.0) as u8, (CLEAR_COLOR[1] * 255.0) as u8];
    p[0].abs_diff(bg[0]) <= 20 && p[1].abs_diff(bg[1]) <= 20
}

#[test]
fn draws_a_lit_mesh_in_the_centre() {
    let gpu = headless_device().expect("a GPU or software adapter");
    let target = RenderTarget::new(&gpu.device, 128, 128);
    let r = MeshRenderer::new(&gpu.device);
    let (p, t) = flat_cube();
    let cube = r.create_mesh(&gpu.device, &gpu.queue, &p, &t, [0.85, 0.45, 0.30]);
    r.render(&gpu.device, &gpu.queue, &target, OrbitCamera::default().view_proj(1.0), &[&cube]);
    let img = read_back(&gpu.device, &gpu.queue, &target);
    img.save(format!("{}/mesh_cube.png", env!("CARGO_TARGET_TMPDIR"))).ok();
    assert!(!is_background(img.get_pixel(64, 64)));
    let lum = |p: &image::Rgba<u8>| 0.299 * p[0] as f32 + 0.587 * p[1] as f32 + 0.114 * p[2] as f32;
    let lums: Vec<f32> = img.pixels().filter(|p| !is_background(p)).map(lum).collect();
    let (lo, hi) = lums.iter().fold((255f32, 0f32), |(lo, hi), &l| (lo.min(l), hi.max(l)));
    assert!(hi - lo > 40.0, "faces should be shaded differently: {lo}..{hi}");
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
    r.render(&gpu.device, &gpu.queue, &target, OrbitCamera::default().view_proj(1.0), &[&cube]);
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
```
> The cube winding: in `flat_cube`, the four corners go around the face. `[base, base+2, base+1]` is counter-clockwise from outside for this corner order. Lighting doesn't depend on it, because the shader is two-sided and culling is off.

- [ ] **Step 2: Run and watch them fail**

Run: `cargo nextest run -p opendrape-render --test mesh_render`
Expected: compile errors (`MeshRenderer`, `vertex_normals` not found).

- [ ] **Step 3: Implement**

`crates/render/src/mesh.wgsl`:
```wgsl
struct Uniforms {
    view_proj: mat4x4<f32>,
    color: vec4<f32>,
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
fn fs_main(v: VsOut, @builtin(front_facing) front: bool) -> @location(0) vec4<f32> {
    // Two-sided: cloth is seen from inside too.
    var n = normalize(v.normal);
    if (!front) {
        n = -n;
    }
    let key = max(dot(n, normalize(vec3<f32>(0.3, 0.8, 0.6))), 0.0);
    let fill = max(dot(n, normalize(vec3<f32>(-0.5, 0.2, -0.4))), 0.0);
    let sky = 0.5 + 0.5 * n.y;
    let light = 0.18 + 0.22 * sky + 0.6 * key + 0.15 * fill;
    let linear = u.color.rgb * light;
    // Target is Rgba8Unorm (egui requirement): encode to sRGB here.
    return vec4<f32>(pow(linear, vec3<f32>(1.0 / 2.2)), 1.0);
}
```

`crates/render/src/mesh.rs`:
```rust
use crate::target::{CLEAR_COLOR, COLOR_FORMAT, DEPTH_FORMAT, RenderTarget};
use glam::{Mat4, Vec3};

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
    color: [f32; 4],
}

/// Area-weighted vertex normals; vertices used by no triangle get +Y.
pub fn vertex_normals(positions: &[Vec3], triangles: &[[u32; 3]]) -> Vec<Vec3> {
    let mut n = vec![Vec3::ZERO; positions.len()];
    for t in triangles {
        let [a, b, c] = t.map(|k| k as usize);
        let face = (positions[b] - positions[a]).cross(positions[c] - positions[a]);
        n[a] += face;
        n[b] += face;
        n[c] += face;
    }
    n.into_iter().map(|v| v.try_normalize().unwrap_or(Vec3::Y)).collect()
}

/// A triangle mesh on the GPU whose vertices (and triangles) can be replaced every frame.
pub struct GpuMesh {
    vertices: wgpu::Buffer,
    indices: wgpu::Buffer,
    vertex_capacity: usize,
    index_capacity: usize,
    index_count: u32,
    triangles: Vec<[u32; 3]>,
    uniforms: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
    color: [f32; 3],
}

pub struct MeshRenderer {
    pipeline: wgpu::RenderPipeline,
    layout: wgpu::BindGroupLayout,
}

impl MeshRenderer {
    pub fn new(device: &wgpu::Device) -> Self {
        let shader = device.create_shader_module(wgpu::include_wgsl!("mesh.wgsl"));
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("mesh uniforms layout"),
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
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("mesh pipeline layout"),
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("mesh pipeline"),
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
            primitive: wgpu::PrimitiveState { cull_mode: None, ..Default::default() },
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
        Self { pipeline, layout }
    }

    pub fn create_mesh(&self, device: &wgpu::Device, queue: &wgpu::Queue, positions: &[Vec3], triangles: &[[u32; 3]], color: [f32; 3]) -> GpuMesh {
        let buffer = |label: &str, size: usize, usage: wgpu::BufferUsages| {
            device.create_buffer(&wgpu::BufferDescriptor { label: Some(label), size: size.max(16) as u64, usage, mapped_at_creation: false })
        };
        let vertices = buffer("mesh vertices", positions.len() * std::mem::size_of::<Vertex>(), wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST);
        let indices = buffer("mesh indices", triangles.len() * 12, wgpu::BufferUsages::INDEX | wgpu::BufferUsages::COPY_DST);
        let uniforms = buffer("mesh uniforms", std::mem::size_of::<Uniforms>(), wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST);
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("mesh uniforms"),
            layout: &self.layout,
            entries: &[wgpu::BindGroupEntry { binding: 0, resource: uniforms.as_entire_binding() }],
        });
        let mut mesh = GpuMesh {
            vertices,
            indices,
            vertex_capacity: positions.len(),
            index_capacity: triangles.len() * 3,
            index_count: 0,
            triangles: vec![],
            uniforms,
            bind_group,
            color,
        };
        self.upload(queue, &mut mesh, positions, Some(triangles));
        mesh
    }

    /// Replaces vertex positions (normals are recomputed) and optionally the triangles.
    pub fn update_mesh(&self, device: &wgpu::Device, queue: &wgpu::Queue, mesh: &mut GpuMesh, positions: &[Vec3], triangles: Option<&[[u32; 3]]>) {
        let tri_len = triangles.map_or(mesh.triangles.len(), <[_]>::len);
        if positions.len() > mesh.vertex_capacity || tri_len * 3 > mesh.index_capacity {
            let tris = triangles.map_or_else(|| mesh.triangles.clone(), <[_]>::to_vec);
            *mesh = self.create_mesh(device, queue, positions, &tris, mesh.color);
        } else {
            self.upload(queue, mesh, positions, triangles);
        }
    }

    fn upload(&self, queue: &wgpu::Queue, mesh: &mut GpuMesh, positions: &[Vec3], triangles: Option<&[[u32; 3]]>) {
        if let Some(t) = triangles {
            mesh.triangles = t.to_vec();
            queue.write_buffer(&mesh.indices, 0, bytemuck::cast_slice(t));
            mesh.index_count = (t.len() * 3) as u32;
        }
        let normals = vertex_normals(positions, &mesh.triangles);
        let verts: Vec<Vertex> = positions.iter().zip(&normals).map(|(p, n)| Vertex { position: p.to_array(), normal: n.to_array() }).collect();
        queue.write_buffer(&mesh.vertices, 0, bytemuck::cast_slice(&verts));
    }

    /// Clears `target` and draws `meshes`. Submits its own command buffer.
    pub fn render(&self, device: &wgpu::Device, queue: &wgpu::Queue, target: &RenderTarget, view_proj: Mat4, meshes: &[&GpuMesh]) {
        for m in meshes {
            let u = Uniforms { view_proj: view_proj.to_cols_array_2d(), color: [m.color[0], m.color[1], m.color[2], 1.0] };
            queue.write_buffer(&m.uniforms, 0, bytemuck::bytes_of(&u));
        }
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("meshes") });
        {
            let [r, g, b, a] = CLEAR_COLOR;
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("mesh pass"),
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
            for m in meshes.iter().filter(|m| m.index_count > 0) {
                pass.set_bind_group(0, &m.bind_group, &[]);
                pass.set_vertex_buffer(0, m.vertices.slice(..));
                pass.set_index_buffer(m.indices.slice(..), wgpu::IndexFormat::Uint32);
                pass.draw_indexed(0..m.index_count, 0, 0..1);
            }
        }
        queue.submit([encoder.finish()]);
    }
}
```
In `lib.rs`, add `mod mesh;` and `pub use mesh::{GpuMesh, MeshRenderer, vertex_normals};`. Keep `CubeRenderer` until Task 7.

- [ ] **Step 4: Run and see them pass; look at the image**

Run: `cargo nextest run -p opendrape-render`
Expected: all pass: the 10 existing tests plus 3 new ones. Open `target/tmp/mesh_cube.png` with Read: a shaded cube.

- [ ] **Step 5: Commit**

```bash
cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings
git add crates/render
git commit -m "feat(render): two-sided mesh renderer with updatable vertices

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 7: The app: simulation thread, body + cloth view, toolbar, FPS overlay

**Files:**
- Create: `crates/app/src/sim_runner.rs`
- Modify: `crates/app/Cargo.toml`, `crates/app/src/{lib.rs,app.rs,viewport.rs,main.rs}`, `crates/app/i18n/en-US/opendrape.ftl`, `crates/app/tests/ui.rs`
- Delete: `crates/render/src/cube.rs`, `crates/render/src/cube.wgsl`, `crates/render/tests/cube_render.rs`. Before deleting, move its `odd_sizes_read_back_correctly`, `explicitly_requested_backend_is_used` and Linux `opengl_fallback_renders_the_cube` tests into `mesh_render.rs`, drawing `flat_cube()` with `MeshRenderer`.
- Modify: `crates/render/src/lib.rs` (drop the `CubeRenderer` export)

**Interfaces:**
- Consumes: `Scene`, `Garment`, `body()` (Task 5); `MeshRenderer`, `GpuMesh` (Task 6); `FRAME_DT` (Task 3).
- Produces:
  - `SimFrame { seq, time, positions: Vec<Vec3>, triangles: Arc<Vec<[u32;3]>>, step_ms }`;
  - `SimRunner::start(Garment, playing: bool, on_frame: impl Fn()+Send+'static)`, with `latest() -> Arc<SimFrame>`, `is_playing()`, `set_playing(bool)`, `reset(Garment)`, and `Drop` joining the thread;
  - `OpenDrapeApp::sim_frame() -> Option<Arc<SimFrame>>` and `OpenDrapeApp::stats_text(fps, ms, points) -> String`.

- [ ] **Step 1: Dependencies and failing tests for the runner**

`crates/app/Cargo.toml` `[dependencies]` add:
```toml
opendrape-sim.workspace = true
opendrape-testkit.workspace = true
glam.workspace = true
arc-swap.workspace = true
crossbeam-channel.workspace = true
```

`crates/app/src/sim_runner.rs`, with types, stubs and unit tests:
```rust
//! Runs the cloth simulation on its own thread so the window stays responsive even when a
//! slow computer simulates slower than real time. The UI only reads the latest frame.

use arc_swap::ArcSwap;
use crossbeam_channel::{Receiver, Sender, unbounded};
use glam::Vec3;
use opendrape_testkit::garments::{Garment, Scene};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

/// One published simulation frame.
#[derive(Debug)]
pub struct SimFrame {
    /// Increases with every published frame, across resets.
    pub seq: u64,
    /// Simulated seconds since the scene started.
    pub time: f64,
    pub positions: Vec<Vec3>,
    /// Shared until the topology changes (seams weld), so the renderer can skip re-uploads.
    pub triangles: Arc<Vec<[u32; 3]>>,
    /// Wall-clock milliseconds the last step took.
    pub step_ms: f64,
}

enum Command {
    Reset(Garment),
    Wake,
    Shutdown,
}

pub struct SimRunner {
    tx: Sender<Command>,
    latest: Arc<ArcSwap<SimFrame>>,
    playing: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl SimRunner {
    pub fn start(_garment: Garment, _playing: bool, _on_frame: impl Fn() + Send + 'static) -> Self {
        todo!()
    }
    pub fn latest(&self) -> Arc<SimFrame> {
        self.latest.load_full()
    }
    pub fn is_playing(&self) -> bool {
        self.playing.load(Ordering::Relaxed)
    }
    pub fn set_playing(&self, _playing: bool) {
        todo!()
    }
    pub fn reset(&self, _garment: Garment) {
        todo!()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use opendrape_testkit::garments::{BODICE_PARTICLES, SKIRT_PARTICLES};

    fn wait_for(what: &str, mut cond: impl FnMut() -> bool) {
        let start = Instant::now();
        while !cond() {
            assert!(start.elapsed() < Duration::from_secs(20), "timed out waiting for {what}");
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    #[test]
    fn the_simulation_runs_without_the_window_drawing() {
        let r = SimRunner::start(Garment::Skirt, true, || {});
        assert_eq!(r.latest().positions.len(), SKIRT_PARTICLES);
        wait_for("simulated time to advance", || r.latest().time > 0.1);
    }

    #[test]
    fn pausing_stops_time_and_reset_rewinds_it() {
        let r = SimRunner::start(Garment::Skirt, true, || {});
        wait_for("time > 0.1", || r.latest().time > 0.1);
        r.set_playing(false);
        std::thread::sleep(Duration::from_millis(100)); // let an in-flight step finish
        let t = r.latest().time;
        std::thread::sleep(Duration::from_millis(150));
        assert_eq!(r.latest().time, t, "paused");
        r.reset(Garment::Skirt);
        wait_for("reset to t = 0", || r.latest().time == 0.0);
    }

    #[test]
    fn rapid_resets_end_on_the_last_choice() {
        let r = SimRunner::start(Garment::Skirt, true, || {});
        for g in [Garment::BodiceProxy, Garment::Skirt, Garment::BodiceProxy] {
            r.reset(g);
        }
        wait_for("the fitted tube", || r.latest().positions.len() == BODICE_PARTICLES);
    }

    #[test]
    fn dropping_the_runner_stops_its_thread() {
        let r = SimRunner::start(Garment::Skirt, true, || {});
        wait_for("running", || r.latest().time > 0.0);
        let start = Instant::now();
        drop(r);
        assert!(start.elapsed() < Duration::from_secs(2), "thread joined promptly");
    }
}
```
In `lib.rs`, add `pub mod sim_runner;` and `pub use sim_runner::SimFrame;`.

- [ ] **Step 2: Run and watch them fail**

Run: `cargo nextest run -p opendrape sim_runner`
Expected: 4 FAIL with `not yet implemented`.

- [ ] **Step 3: Implement the runner**

```rust
impl SimRunner {
    pub fn start(garment: Garment, playing: bool, on_frame: impl Fn() + Send + 'static) -> Self {
        let (tx, rx) = unbounded();
        let scene = Scene::new(garment);
        let mut publisher = Publisher::new(&scene, 0);
        let latest = Arc::new(ArcSwap::from_pointee(publisher.frame(&scene, 0.0)));
        let playing = Arc::new(AtomicBool::new(playing));
        let thread = {
            let (latest, playing) = (latest.clone(), playing.clone());
            std::thread::Builder::new()
                .name("opendrape-sim".into())
                .spawn(move || run(scene, publisher, &rx, &latest, &playing, &on_frame))
                .expect("spawn the simulation thread")
        };
        Self { tx, latest, playing, thread: Some(thread) }
    }
    pub fn set_playing(&self, playing: bool) {
        self.playing.store(playing, Ordering::Relaxed);
        let _ = self.tx.send(Command::Wake);
    }
    pub fn reset(&self, garment: Garment) {
        let _ = self.tx.send(Command::Reset(garment));
    }
}

impl Drop for SimRunner {
    fn drop(&mut self) {
        let _ = self.tx.send(Command::Shutdown);
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

struct Publisher {
    seq: u64,
    topology: u64,
    triangles: Arc<Vec<[u32; 3]>>,
}

impl Publisher {
    fn new(scene: &Scene, seq: u64) -> Self {
        let c = scene.solver.cloth();
        Self { seq, topology: c.topology_version(), triangles: Arc::new(c.triangles().to_vec()) }
    }
    fn frame(&mut self, scene: &Scene, step_ms: f64) -> SimFrame {
        let c = scene.solver.cloth();
        if c.topology_version() != self.topology {
            self.topology = c.topology_version();
            self.triangles = Arc::new(c.triangles().to_vec());
        }
        self.seq += 1;
        SimFrame {
            seq: self.seq,
            time: scene.solver.time(),
            positions: c.positions().iter().map(|p| p.as_vec3()).collect(),
            triangles: self.triangles.clone(),
            step_ms,
        }
    }
}

fn run(mut scene: Scene, mut publisher: Publisher, rx: &Receiver<Command>, latest: &ArcSwap<SimFrame>, playing: &AtomicBool, on_frame: &dyn Fn()) {
    let mut next = Instant::now();
    loop {
        // Paused: sleep until a command arrives. Playing: just look.
        let cmd = if playing.load(Ordering::Relaxed) { rx.try_recv().ok() } else { Some(rx.recv().unwrap_or(Command::Shutdown)) };
        match cmd {
            Some(Command::Shutdown) => return,
            Some(Command::Reset(g)) => {
                scene = Scene::new(g);
                publisher = Publisher::new(&scene, publisher.seq);
                latest.store(Arc::new(publisher.frame(&scene, 0.0)));
                on_frame();
                next = Instant::now();
                continue;
            }
            Some(Command::Wake) => {
                next = Instant::now();
                continue;
            }
            None => {}
        }
        let started = Instant::now();
        scene.step();
        latest.store(Arc::new(publisher.frame(&scene, started.elapsed().as_secs_f64() * 1000.0)));
        on_frame();
        // Real time at most; a slow computer simply runs slower.
        next += Duration::from_secs_f64(opendrape_sim::FRAME_DT);
        let now = Instant::now();
        if next > now {
            std::thread::sleep(next - now);
        } else {
            next = now;
        }
    }
}
```

- [ ] **Step 4: Runner tests pass**

Run: `cargo nextest run -p opendrape sim_runner`
Expected: 4 passed.

- [ ] **Step 5: Viewport draws the body and cloth, and the cube goes away**

Replace `crates/app/src/viewport.rs`:
```rust
use crate::sim_runner::SimFrame;
use opendrape_render::{GpuMesh, MeshRenderer, OrbitCamera, RenderTarget, target_size};
use std::sync::Arc;

/// Mid-brown skin tone and a cotton blue.
const SKIN: [f32; 3] = [0.62, 0.45, 0.36];
const FABRIC: [f32; 3] = [0.17, 0.36, 0.70];

struct ClothOnGpu {
    mesh: GpuMesh,
    seq: u64,
    triangles: Arc<Vec<[u32; 3]>>,
}

/// The 3D panel: renders offscreen and shows the texture as an egui image.
pub struct Viewport {
    renderer: MeshRenderer,
    camera: OrbitCamera,
    target: Option<(RenderTarget, egui::TextureId)>,
    body: GpuMesh,
    cloth: Option<ClothOnGpu>,
    pub frames_drawn: u64,
}

impl Viewport {
    pub fn new(rs: &egui_wgpu::RenderState) -> Self {
        let renderer = MeshRenderer::new(&rs.device);
        let b = opendrape_testkit::garments::body();
        let body = renderer.create_mesh(&rs.device, &rs.queue, &b.positions, &b.triangles, SKIN);
        let camera = OrbitCamera {
            target: glam::Vec3::new(0.0, 0.95, 0.02),
            yaw: 0.5,
            pitch: 0.12,
            distance: 2.6,
            fov_y: 35f32.to_radians(),
        };
        Self { renderer, camera, target: None, body, cloth: None, frames_drawn: 0 }
    }

    pub fn ui(&mut self, ui: &mut egui::Ui, rs: &egui_wgpu::RenderState, frame: Option<&SimFrame>) {
        let size = ui.available_size();
        let max_dim = rs.device.limits().max_texture_dimension_2d;
        let Some((w, h)) = target_size(size.x, size.y, ui.pixels_per_point(), max_dim) else {
            return; // minimised or collapsed: nothing to draw
        };
        if let Some(f) = frame {
            self.sync_cloth(rs, f);
        }
        self.ensure_target(rs, w, h);
        let (target, texture_id) = self.target.as_ref().expect("ensure_target sets it");
        let mut meshes = vec![&self.body];
        if let Some(c) = &self.cloth {
            meshes.push(&c.mesh);
        }
        self.renderer.render(&rs.device, &rs.queue, target, self.camera.view_proj(w as f32 / h as f32), &meshes);
        let texture_id = *texture_id;
        self.frames_drawn += 1;

        let image = egui::Image::new(egui::load::SizedTexture::new(texture_id, size));
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

    /// Uploads a new simulation frame (and new triangles after welding) once per frame number.
    fn sync_cloth(&mut self, rs: &egui_wgpu::RenderState, f: &SimFrame) {
        match &mut self.cloth {
            Some(c) if c.seq == f.seq => {}
            Some(c) => {
                let new_tris = (!Arc::ptr_eq(&c.triangles, &f.triangles)).then_some(f.triangles.as_slice());
                self.renderer.update_mesh(&rs.device, &rs.queue, &mut c.mesh, &f.positions, new_tris);
                c.seq = f.seq;
                c.triangles = f.triangles.clone();
            }
            None => {
                let mesh = self.renderer.create_mesh(&rs.device, &rs.queue, &f.positions, &f.triangles, FABRIC);
                self.cloth = Some(ClothOnGpu { mesh, seq: f.seq, triangles: f.triangles.clone() });
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
Then delete `crates/render/src/cube.rs`, `crates/render/src/cube.wgsl` and `crates/render/tests/cube_render.rs`. Before deleting, port its backend tests into `mesh_render.rs`:
- `explicitly_requested_backend_is_used`;
- the Linux `opengl_fallback_renders_the_cube`;
- `odd_sizes_read_back_correctly`.

Each draws `flat_cube()` with `MeshRenderer` instead of `CubeRenderer`. Remove `mod cube;` and the `CubeRenderer` export from `crates/render/src/lib.rs`.

- [ ] **Step 6: Strings, toolbar and overlay, with failing UI tests first**

Append to `crates/app/i18n/en-US/opendrape.ftl`:
```ftl
toolbar-play = Play
toolbar-pause = Pause
toolbar-reset = Reset
garment-skirt = A-line skirt
garment-bodice-proxy = Fitted tube (collision test)
overlay-stats = { $fps } fps · simulation { $ms } ms per step · { $points } points
```

Append to `crates/app/tests/ui.rs`:
```rust
use std::time::{Duration, Instant};

fn wait_until(h: &mut Harness<'static, OpenDrapeApp>, what: &str, mut cond: impl FnMut(&OpenDrapeApp) -> bool) {
    let start = Instant::now();
    while !cond(h.state()) {
        assert!(start.elapsed() < Duration::from_secs(20), "timed out waiting for {what}");
        std::thread::sleep(Duration::from_millis(20));
        h.step();
    }
}

// While the simulation plays it keeps requesting repaints, so these tests step explicitly
// (`run_steps`, `wait_until`) instead of `run()`, which waits for the UI to settle.

#[test]
fn the_skirt_drapes_and_can_be_paused() {
    let dir = tempfile::tempdir().unwrap();
    let mut h = harness(dir.path(), SharedState::default()); // starts paused (autoplay: false)
    h.run();
    h.get_by_label("A-line skirt");
    h.get_by_label("Play").click();
    h.run_steps(2);
    wait_until(&mut h, "the simulation to advance", |a| a.sim_frame().is_some_and(|f| f.time > 0.05));
    h.get_by_label("Pause").click();
    h.run_steps(3);
    h.get_by_label("Play");
}

#[test]
fn reset_and_garment_switch_reload_the_scene() {
    let dir = tempfile::tempdir().unwrap();
    let mut h = harness(dir.path(), SharedState::default());
    h.run();
    h.get_by_label("Play").click();
    h.run_steps(2);
    wait_until(&mut h, "time > 0.1", |a| a.sim_frame().is_some_and(|f| f.time > 0.1));
    h.get_by_label("Pause").click();
    h.run_steps(2);
    h.get_by_label("Reset").click();
    h.run_steps(2);
    wait_until(&mut h, "time back to 0", |a| a.sim_frame().is_some_and(|f| f.time == 0.0));
    h.get_by_label("Fitted tube (collision test)").click();
    h.run_steps(2);
    wait_until(&mut h, "the tube", |a| {
        a.sim_frame().is_some_and(|f| f.positions.len() == opendrape_testkit::garments::BODICE_PARTICLES)
    });
}

#[test]
fn stats_text_is_readable() {
    assert_eq!(OpenDrapeApp::stats_text(59.6, 11.73, 4794), "60 fps · simulation 11.7 ms per step · 4794 points");
}
```
Add `opendrape-testkit.workspace = true` to `[dev-dependencies]` too.

Run: `cargo nextest run -p opendrape --test ui`
Expected: compile errors (`sim_frame` and `stats_text` don't exist).

- [ ] **Step 7: Wire the runner, toolbar and overlay into the app**

In `app.rs`:
- add `use crate::sim_runner::{SimFrame, SimRunner}; use opendrape_testkit::garments::Garment; use std::sync::Arc;`;
- add fields `runner: Option<SimRunner>`, `garment: Garment` and `fps: f32` to `OpenDrapeApp`;
- add `pub autoplay: bool` to `Startup`, documented as "Start simulating immediately (tests start paused, so `Harness::run` can settle)". Set it to `true` where `main.rs` builds `Startup`. Add `autoplay: false` to every `Startup { … }` in `crates/app/tests/ui.rs`: the `harness()` helper, `tiny_window_does_not_crash` and `crash_marker_is_cleared_only_after_frames_were_presented`;
- in `new`, before `Self { … }`:
```rust
        let runner = render_state.map(|_| {
            let ctx = cc.egui_ctx.clone();
            SimRunner::start(Garment::Skirt, startup.autoplay, move || ctx.request_repaint())
        });
```
  and set `runner, garment: Garment::Skirt, fps: 0.0`;
- add the methods:
```rust
    /// The latest simulation frame, if the 3D view is running.
    pub fn sim_frame(&self) -> Option<Arc<SimFrame>> {
        self.runner.as_ref().map(SimRunner::latest)
    }

    pub fn stats_text(fps: f32, step_ms: f64, points: usize) -> String {
        tr!(
            "overlay-stats",
            fps = format!("{fps:.0}"),
            ms = format!("{step_ms:.1}"),
            points = points.to_string()
        )
    }

    fn toolbar(&mut self, ui: &mut egui::Ui) {
        let Some(runner) = &self.runner else { return };
        ui.horizontal(|ui| {
            for g in Garment::ALL {
                if ui.selectable_label(self.garment == g, garment_label(g)).clicked() && self.garment != g {
                    self.garment = g;
                    runner.reset(g);
                }
            }
            ui.separator();
            let playing = runner.is_playing();
            if ui.button(if playing { tr!("toolbar-pause") } else { tr!("toolbar-play") }).clicked() {
                runner.set_playing(!playing);
            }
            if ui.button(tr!("toolbar-reset")).clicked() {
                runner.reset(self.garment);
            }
        });
    }
```
- add a free function:
```rust
fn garment_label(g: Garment) -> String {
    match g {
        Garment::Skirt => tr!("garment-skirt"),
        Garment::BodiceProxy => tr!("garment-bodice-proxy"),
    }
}
```
- in `ui()`, after the menu-bar panel, add `egui::Panel::top("toolbar").show(ui, |ui| self.toolbar(ui));`, then replace the whole `egui::CentralPanel::default().show(…)` call with:
```rust
        let dt = ui.input(|i| i.unstable_dt).max(1e-3);
        self.fps = if self.fps == 0.0 { 1.0 / dt } else { 0.9 * self.fps + 0.1 / dt };
        let sim = self.sim_frame();
        let fps = self.fps;
        egui::CentralPanel::default().show(ui, |ui| match (self.viewport.as_mut(), frame.wgpu_render_state()) {
            (Some(viewport), Some(rs)) => {
                let rect = ui.max_rect();
                viewport.ui(ui, rs, sim.as_deref());
                if let Some(f) = &sim {
                    ui.painter().text(
                        rect.left_top() + egui::vec2(10.0, 8.0),
                        egui::Align2::LEFT_TOP,
                        Self::stats_text(fps, f.step_ms, f.positions.len()),
                        egui::FontId::proportional(13.0),
                        egui::Color32::from_gray(60),
                    );
                }
            }
            _ => {
                ui.centered_and_justified(|ui| ui.label(tr!("viewport-no-gpu")));
            }
        });
```
  (`frame` here is the `eframe::Frame` parameter, as before. Rename nothing else.)

- [ ] **Step 8: Run everything**

Run: `cargo nextest run --workspace`
Expected: all pass: body, xtask, sim, testkit, render and the app's unit and UI tests. Then run `cargo run -p opendrape -- --smoke-test; echo "exit=$?"`. Expected: `exit=0`. A window shows the body briefly; ask the user before launching it while they're working.

Throwaway visual check: a temporary `tests/zz_visual_tmp.rs`, as in M0, renders the app after waiting for `time > 3.0` and saves a PNG. Read it, and expect the body in a blue A-line skirt at the waist. Delete the file.

- [ ] **Step 9: Commit**

```bash
cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings && cargo deny check
git add -A crates
git commit -m "feat(app): drape the skirt on the body in a background simulation thread

- body and cloth rendered with the new mesh renderer; cube removed
- toolbar: garment choice, Play/Pause, Reset; FPS and step-time overlay

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 8: Docs, checklist, final checks, publish (publishing needs the user's OK)

**Files:**
- Create: `docs/testing/M1-checklist.md`
- Modify: `README.md` (status line), `docs/specs/2026-10-09-opendrape-design.md` (Status), `docs/testing/nightly-notes.md` (unchanged unless wording needs M1)

- [ ] **Step 1: Plain-language checklist**

`docs/testing/M1-checklist.md`:
```markdown
# OpenDrape M1: what to try

This build shows a real 3D body and simulates fabric falling onto it. There is no pattern
drawing yet: the two garments are built in.

## Check these

- [ ] OpenDrape opens showing a woman's body in a light grey room.
- [ ] A blue skirt appears around the hips, its side seams pull together, and it falls
      into place at the waist within about 5 seconds, then stops moving.
- [ ] Turn the view by dragging and zoom by scrolling: the skirt never passes through the body
      (no skin showing through the fabric).
- [ ] The text at the top left shows a speed of at least 20 fps.
- [ ] **Pause** freezes the fabric; **Play** continues it.
- [ ] **Reset** starts the drape again from the beginning.
- [ ] Click **Fitted tube (collision test)**: a tight tube hugs the chest and waist with no
      skin poking through. Click **A-line skirt** to go back.
- [ ] Quit and reopen: it starts normally.

If anything looks wrong, take a screenshot and copy Help → About → Copy diagnostics.
```
Update `README.md`'s status line to "milestone M1: drape spike, a skirt draping on a 3D body".

- [ ] **Step 2: Final verification**

```bash
cargo fmt --all --check && cargo clippy --workspace --all-targets -- -D warnings && cargo deny check
cargo nextest run --workspace
./scripts/package-macos.sh dev
target/universal-apple-darwin/release/OpenDrape.app/Contents/MacOS/opendrape --smoke-test; echo "exit=$?"
cargo run --release -p opendrape-testkit --example drape_bench
```
Expected:
- all green;
- the package script prints the DMG path;
- the smoke test prints `exit=0`;
- the bench prints ms per frame for both garments. Record them.

- [ ] **Step 3: Whole-branch review, then publish only with the user's OK**

Run the final whole-branch review (executing-plans "Final Review"). Fix Critical and Important findings, each with a failing test first.

Then **STOP and ask the user** before merging to `main` and pushing: every push rebuilds and renames the public nightly downloads. After the push, watch CI and the Release run. When they're green, send the user the release page link and the M1 checklist.

- [ ] **Step 4: Mark M1 complete**

Add `- **M1 Drape spike: complete (<date>).**` under Status in the spec, with the bench numbers. Commit with the push, or after it if the user prefers.

---

## Self-Review Notes

**Spec coverage (M1 row):**

| M1 item | Where |
|---|---|
| MakeHuman CC0 body in A-pose | Tasks 1–2 |
| SDF + exact collision | Task 4: exact collision; SDF deferred by ruling 1 |
| XPBD | Task 3 |
| Hard-coded 2-panel skirt and fitted-tube bodice proxy | Task 5 |
| Play/pause/reset | Task 7 |
| Orbit camera | M0, reframed for the body in Task 7 |
| FPS overlay | Task 7 |
| Checklist: falls and settles / no poke-through / ≥ 20 FPS | Task 5 gates, Task 7 overlay, Task 8 checklist |

**Type consistency:**
- `Garment::{Skirt, BodiceProxy}`, `Scene::new` and `garments::{body, collider}` are used identically in Tasks 5 and 7.
- `SimFrame` fields match between the runner, viewport and tests.
- `MeshRenderer::update_mesh(device, queue, &mut GpuMesh, positions, Option<triangles>)` is called the same way in Task 7.
- `BodyCollider::ray_exit(origin, dir, max)` is the same in Tasks 4 and 5.

**Known fragility:** Task 7's UI tests wait on a real background thread. Each wait is bounded at 20 s and polls every 20 ms. If CI shows flakiness, raise the bound; don't remove the test.
