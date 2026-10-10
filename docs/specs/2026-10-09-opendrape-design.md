# Plan: OpenDrape — free, open-source 3D garment simulation (CLO3D alternative)

## Context

Fashion students and small designers in India, Africa and other low-income regions are locked out of 3D
fashion design. CLO3D costs $450/yr ($225 student), Browzwear $750+/yr, and **every commercial tool assumes
an NVIDIA GPU and 16 GB+ RAM** (Style3D and Optitex are Windows-only). The realistic student machine in
these regions is a 4–8 GB laptop with integrated graphics, often in a shared lab with costly data.
Research found **no mature free tool** that covers pattern → sew → drape: Seamly2D/Valentina/FreeSewing are
2D-only, Blender is general-purpose and hard to learn, and the few open web prototypes (seamer-studio,
patternCanvas, Clothing-CAD) are unfinished.

**Goal:** OpenDrape, a GPL-3.0 native desktop app for macOS and Windows that covers the core CLO3D loop
simply: draw 2D patterns → arrange on a 3D body → sew → drape → fabric/colour → render and export. It runs
offline on low-end hardware. Physical sampling waste is cut by replacing muslin toiles with digital ones.
Sustainability claims stay qualitative, because the 70–90% figures in circulation are vendor marketing.

### Decisions confirmed with the user (2026-10-09)
| Topic | Decision |
|---|---|
| Platform / stack | Native **Rust**: **wgpu** for 3D, **egui/eframe** for UI. Installers for macOS (universal) + Windows; Linux AppImage at the end |
| Who codes | Claude writes all code; the user tests builds → small milestones, heavy automated tests, downloadable builds |
| Name | **OpenDrape** (never use "CLO" — trademark) |
| Pattern creation | Draw from scratch, CLO-style pen tools |
| Acceptance garments | A-line skirt, basic T-shirt, fitted bodice/dress with darts |
| Outputs | PNG renders; true-scale A4/Letter tiled PDF; OBJ + glTF; DXF-AAMA |
| Avatar | Measurement sliders, diverse skin tones and body shapes, a few poses, import own body |
| License | GPL-3.0 |
| UI language | English only; every string goes through Fluent so translations can be added later |
| Windows testing | User has a Mac only → Windows is covered by CI (WARP smoke renders) plus recruited testers |

### Direction from 2026-10-10
The app is organised into five workspaces (Modeling · Finishing · Texturing · Rendering ·
Animation). AI rendering and animation are optional plug-ins the user chooses, and a connector
lets coding agents and Blender work with OpenDrape. See
`docs/superpowers/specs/2026-10-10-m5-workspaces-design.md` for the direction and the order of
the next milestones.

## Architecture

### Core technical choices (all deps GPL-3.0-compatible, versions verified 2026-10-09)
- **App shell:** `eframe`/`egui` 0.36.2 on its default wgpu renderer (`wgpu` 30).
  - The 3D viewport renders into our own offscreen texture, shown in an egui panel via `egui_wgpu::Renderer::register_native_texture`.
  - Headless PNG export and golden tests use the same code path.
- **Graphics fallback for old Intel iGPUs:** wgpu backends DX12 → Vulkan → GL 3.3 → DX12 WARP (software).
  - A crash sentinel auto-switches backend after a failed launch; there is also a "Safe graphics mode" shortcut and a `--gpu=` flag.
  - All shaders stay within `Limits::downlevel_webgl2_defaults()`: no compute shaders, textures ≤ 2048.
  - **Stated min spec:** Windows 10/11 x64, 4 GB RAM, DX12/Vulkan/OpenGL 3.3 GPU (Intel HD 4000+); macOS 11+.
- **Cloth solver:** XPBD with substeps, run on the CPU across threads (`rayon`, graph-coloured constraint batches).
  - Ported from Matthias Müller's Ten Minute Physics demos 14 and 15 (MIT, verified).
  - **Seams:** zero-rest-length stitch constraints between arc-length-matched vertices. Both sides of a seam get the same vertex count, so the longer side gathers as ease. Stitch stiffness ramps up over the first frames.
  - **Darts** are internal seams.
  - **Fabric:** CLO-like 0–99 sliders (stretch warp/weft, shear, bend, density, damping, friction, thickness), mapped to per-edge XPBD compliance by the edge's angle to the grainline.
- **Body collision:** a signed distance field baked from the avatar with `parry3d` 0.31 (BVH + oriented point projection), about 8 mm grid. Each substep adds an exact closest-point correction for particles within about 1 cm, which the fitted bodice needs.
- **2D geometry:**
  - `kurbo` 0.13 for Bezier curves, arc length and offsets (f64, millimetres).
  - `spade` 2.15 constrained Delaunay **with refinement** (`keep_constraint_edges`, angle limit) for uniform cloth meshes.
  - `i_overlay` 9 plus our own code for per-segment seam allowance.
  - The 2D editor draws with egui's painter (no lyon).
- **Avatar:** MakeHuman/MPFB **CC0 assets only** (base mesh, targets, skins, poses).
  - Never MakeHuman's AGPL code, and never SMPL/SMPL-X (non-commercial license).
  - Measurement sliders = linear blendshapes plus an iterative solver. Girths are measured by slicing the mesh and taking the convex hull, not with MakeHuman's AGPL ruler code.
  - An `xtask` converts the assets offline into a compact binary.
- **I/O:**
  - `serde`/`serde_json`/`zip` for `.odp` project files.
  - `gltf-json` 1.4 for GLB; a hand-written OBJ writer; `tobj` for OBJ import.
  - `krilla` 0.8 for PDF tiles.
  - Our own DXF-AAMA R12 writer, following Seamly2D's layer map and ezdxf's `gerber_D6673` rules.
- **Other:** `fluent-bundle` + `i18n-embed-fl` (`fl!()` checks message IDs at compile time); `rfd` for file dialogs; `crossbeam-channel` + `arc-swap` for threading; Noto Sans shipped as font files.
- **Packaging:** `cargo-packager` 0.11 builds a universal .dmg, an NSIS per-user .exe (no admin needed) and a portable ZIP.

### Cargo workspace (each crate has one job and is testable without a GPU where possible)
```
crates/core     data model, IDs, units, schema version + migrations (no GPU/geometry deps)
crates/geom     resolve_piece (mirror/symmetry/darts → concrete paths), seam allowance, notches, validation
crates/mesh     pattern → GarmentMesh: seam-aware boundary sampling, spade CDT + refine, UVs = pattern mm, grain angles, stitch pairs
crates/body     avatar assets, measurement + slider solver, skinning/poses, OBJ/glTF import, SDF bake
crates/sim      XPBD: stretch/bend/stitch/pin, SDF + exact collision, spatial-hash self-collision
crates/render   wgpu offscreen renderer (avatar, garment, ground), orbit camera, PNG capture, headless-capable
crates/io       .odp project zip, OBJ/GLB export, PDF tiling, DXF-AAMA writer + reader
crates/i18n     Fluent resources + fl! wrapper
crates/app      eframe app: 2D editor, 3D view, panels, Document + undo, sim orchestration
crates/testkit  programmatic acceptance garments, metrics, golden-image compare
xtask/          MakeHuman CC0 → avatar binary, golden updates, ASSETS.md / license report
```
Package names are `opendrape-<crate>`. Key pure interfaces:
- `resolve_piece(&Project, PieceId)`
- `build_garment_mesh(&Project, &MeshParams)`
- `build_sim_scene(...)`
- `Solver::step(dt)`
- `bake_sdf(&AvatarMesh, cell, band)`
- `Renderer3D::render_to_texture(...)`

### Data model (`core`)
- **Units:** 2D is f64 mm with y up; 3D is f32 metres, Y up, avatar faces +Z, feet at y=0.
- **`Project`:** schema_version, pieces, seams, fabrics, avatar spec, arrangement placements, sim settings.
- **`Piece`:**
  - points;
  - outline segments (Line | Cubic), each with its own seam-allowance spec;
  - notches, internal lines, darts (Wedge | FishEye), grainline;
  - symmetry, or a LinkedMirror (derived, never stored);
  - fabric and print placement.
- **`Seam`:** two `SeamSide{piece, start, end, reversed}` ranges along outlines. This covers CLO's segment and free sewing, and a 1:2 join (sleeve cap) becomes two seams split at a notch.
- **File:** `.odp` is a zip of `project.json` + `assets/` + `thumbnail.png`.
  - Loading runs a migration chain over JSON, with one fixture per historical version kept forever.
  - A file newer than the app is refused with a friendly message.
  - Autosave every 2 min, with crash recovery.

### Threading and undo
- **UI thread:** owns the `Document`.
- **Sim thread:** owns the `Solver` and receives `SimCmd` over a channel.
  - After each step it publishes the latest frame with `arc-swap` and requests a repaint; the GPU upload happens only when the frame changes.
  - The rayon pool uses cores − 1.
  - When the pattern is edited, the drape warm-starts by mapping old positions through pattern-space barycentrics.
- **Undo:** snapshots of `Arc<Project>` with copy-on-write pieces. A whole drag is one undo step. 500 levels are cheap, and there is no inverse-command logic to get wrong. Sim state is never in undo.

## Milestones

Each milestone ends with a **GitHub Release** (installers + portable ZIP) and a short checklist with screenshots for the user. A rolling "nightly" pre-release is published between milestones. Estimated total: ~18–24 weeks.

| # | Deliverable | User's test checklist (examples) |
|---|---|---|
| **M0 Foundations** (1–2 wk) | Install the Rust toolchain; git repo plus public GitHub repo (GPL-3.0, README, ASSETS.md); workspace; CI (fmt, clippy, nextest, cargo-deny license allowlist); installers; shaded cube in the 3D panel; backend selector + crash sentinel; About box with a "Copy diagnostics" button; Fluent scaffold | Installs on the Mac without admin rights; shows the GPU name; Safe graphics mode works |
| **M1 Drape spike** (2–3 wk) — *retires the biggest risk first* | MakeHuman CC0 body in A-pose; SDF + exact collision; XPBD; a hard-coded 2-panel skirt **and a fitted-tube "bodice proxy"**; play/pause/reset; orbit camera; FPS overlay | Skirt falls and settles; no visible poke-through; ≥ 20 FPS |
| **M2 Pattern editor** (2–3 wk) | Rectangle/polygon/point tools; line↔curve with Bezier handles; numeric entry in cm or in; segment lengths; notches; internal lines; grainline; seam allowance; symmetric and linked-mirror pieces; undo/redo; basic save/open | Draft a skirt panel from scratch; save, reopen, undo 10 steps |
| **M3 Sew & drape** (2–3 wk) | Arrangement slots around the body; segment and free sewing (flip, warning when lengths mismatch); drag/pin in 3D. **Skirt + T-shirt work end to end** | Follow the in-app guide to draw, sew and drape a T-shirt |
| **M4 Fitted garments** (2–3 wk) | Darts (wedge, fish-eye); self-collision; stitch ramp; strain/fit map; quality presets; live 2D→3D updates. **Fitted bodice/dress** | A bodice with 4 darts drapes with no poke-through |
| **M5 Avatars** (2–3 wk) | Female + male bodies; ~10 measurement sliders + solver; 10+ skin tones; shape presets; A-pose + 3 portfolio poses (garment follows, then re-settles); OBJ/glTF body import with a units dialog | Enter a size chart; measured girths match within ±5 mm |
| **M6 Fabrics & renders** (1–2 wk) | Fabric presets (cotton poplin, jersey, denim, silk, linen, chiffon, khadi, viscose); colours; prints aligned to the grain; PNG render dialog (resolution, transparent background, front/back/side/¾ views) | Make 3 colourways and export 4 views |
| **M7 Exports** (2–3 wk) | OBJ/MTL; GLB; tiled A4/Letter PDF (10 cm test square, overlap marks, page grid) + single-sheet A0 for plotter shops; DXF-AAMA; save/load hardening | Printed test square measures 10 cm; DXF opens in Seamly2D |
| **M8 Release 1.0** (2 wk) | Onboarding tutorial + sample projects; pseudo-locale pass; accessibility (AccessKit); performance pass; Linux AppImage; signing (SignPath for Windows; Apple notarization if funded) | A new user drapes a T-shirt from the tutorial in under 30 min |

**Out of scope for v1:** trousers (v1.1), grading, buttons/zippers/trims, elastic, pleats, more than 2 layers, animation, GPU compute simulation, DXF *import*, auto block drafting, regional draped garments (saree, wrapper). Kurta and kaftan/dashiki are flat-pattern garments and the natural first additions after 1.0, together with Hindi and French.

## Verification (automated, so the user doesn't have to debug)
- **Geometry/mesh (proptest):**
  - seam allowance contains the sew line;
  - arc-length samples are evenly spaced within ±1%;
  - mirroring is exact;
  - mesh minimum angle ≥ 25°, edge length within 0.5–1.6 h, area preserved within ±0.5%;
  - boundary vertex counts match the seam plan.
- **Headless drape tests (`testkit`, 600 frames each for skirt, T-shirt and bodice):**
  - no NaNs;
  - **max body penetration ≤ 2 mm (p99 ≤ 1 mm)**;
  - **seam gap ≤ 1.5 mm max, ≤ 0.5 mm mean**;
  - kinetic energy settles;
  - deterministic position hash on rerun;
  - bodice ease at bust and waist within thickness + 8 mm;
  - drape ordering silk < cotton < denim;
  - random-pattern fuzz;
  - criterion benchmark of the T-shirt scene, with a trend chart.
- **Rendering:**
  - golden images on Linux with lavapipe;
  - **Windows DX12-WARP and Linux GL (llvmpipe) smoke renders** check that the image isn't black and coverage is within ±5%. This stands in for the user's missing Windows machine and guards the old-GPU path.
- **UI:** `egui_kittest` flows (draw → add point → undo/redo, sew two segments, fabric panel) and panel snapshots on macOS arm64.
- **File formats:**
  - project round-trip plus migration fixtures;
  - OBJ re-imported with tobj;
  - GLB checked with the Khronos glTF-Validator;
  - PDF checked with lopdf: A4 is 595.28 × 841.89 pt and the test square is 283.46 pt;
  - DXF-AAMA parsed by the `dxf` crate and ezdxf in CI (R12, 7-bit ASCII, one block per piece, closed boundaries).
- **CI matrix:**
  - every PR: ubuntu-24.04 (everything + goldens), windows-2025 (tests + WARP render), macos-15 arm64 (tests + kittest);
  - nightly adds macos-15-intel and the packaging jobs;
  - release profile uses LTO and codegen-units=1.
- **Per milestone:** the user installs the Release on the Mac and runs its checklist. Windows builds go to recruited testers (a lab laptop is the best target). Bug reports use the in-app "Copy diagnostics".

## Top risks → mitigations
1. **Fitted-bodice collision quality** → spiked in M1 on the real body: SDF + exact correction each substep, thickness offset, denser local mesh, hard metric gates.
2. **Old iGPU drivers** (Intel disabled DX12 on Haswell) → one downlevel wgpu renderer, automatic backend fallback with crash sentinel and WARP, GL smoke tests in CI.
3. **Speed on 2-core CPUs** → particle-budget presets, coloured parallel batches, self-collision every second substep, auto-pause when settled, profiling with puffin.
4. **License contamination** → CC0 MakeHuman assets only (no AGPL code, no SMPL), ASSETS.md provenance ledger, cargo-deny allowlist. Clothing-CAD (GPL-3.0, JS) is used for ideas only, with attribution.
5. **Install friction** (unsigned apps) → portable ZIP, illustrated "Open Anyway" instructions, SignPath free OSS signing after the first release, Apple fee waiver through a partner school or $99/yr.
6. **egui/wgpu breaking releases** (~quarterly) → exact version pins, upgrade once per milestone, wgpu code kept inside `render`.
7. **No Windows machine in hand** → WARP/GL CI renders; recruit 2–3 student testers in India/Africa by M3.
8. **AI-written regressions** → small PRs, CI gates, a `CONTRACTS.md` per crate, golden tests.

## Status

- **M0 Foundations: complete (2026-10-09).** Public repo https://github.com/pujarivivek/opendrape, CI green on Linux/Windows/macOS, nightly installers published. Windows start-up on real hardware still to be confirmed by a tester.
- **M1 Drape spike: complete (2026-10-09).** CC0 MakeHuman body; XPBD cloth with seams that close and weld; exact per-frame body collision; skirt and fitted-tube scenes with Play/Pause/Reset and auto-pause when settled. Release bench on an M4 Max: skirt 4,700 particles, 14 ms/frame, 0.00 mm penetration, strain p99 7.5%; fitted tube 1,440 particles, 4.3 ms/frame, 0.00 mm, strain p99 3.1%. The CI smoke test confirmed the Windows exe starts (WARP).
- **M2a Pattern editor core: complete (2026-10-09).** Pen, rectangle, edit and add-point tools; typed lengths and angles while drawing; a properties panel for lengths, positions, names and grain; cm/inch; undo/redo (200 steps, one per drag); `.odp` save/open with version checks; unsaved-changes guard.
- **M2b Pattern details: complete (2026-10-09).** Seam allowance per piece and edge (1 cm default, 3 cm hems with mirrored corners), notches (single/double/triple, slit/V), internal lines (markings and cut-outs), cut-on-fold pieces shown whole, mirrored left/right pairs, project format v2 (v1 files upgrade), a recovery copy when quitting can't ask, Tab cycling in the number box, labels outside pieces, cached drawing for huge files.
- **M4a Sew & drape, part 1: complete (2026-10-10).**
  - **Sewing:** the Sew tool (W) joins whole edges. Mirrored seams are derived for cut-on-fold pieces and pairs. A seam panel shows the lengths, a warning over 3 mm, and Flip. Edits keep seams sewn.
  - **Fabric:** spade's refined triangulation (12 mm, at most 30,000 particles).
  - **Arranging:** a Stage around the body with a floor; in 3D, click-to-select, an in-house gizmo, Place at front/back/sides and typed placement.
  - **Draping:** Play/Reset drapes the student's own pattern on the simulation thread, with notes for problems.
  - **Files:** project format v3 (v1 and v2 files upgrade).
  - **The drafted-skirt gate:** 0.00 mm penetration, seams welded, strain p99 6.3%.
- **M4b Sew & drape, part 2: complete (2026-10-10).**
  - **Sewing:** Free Sew (F) joins any two points of outlines; notches pair up across seams; whole-edge sewing (W) is unchanged; Fit moves to Cmd+0.
  - **Arranging:** arm lines found on the form; Place at → Left arm / Right arm.
  - **Draping:** pins (saved) and grabs in 3D; edits while draping carry the drape on, warm-started and coalesced on the simulation thread.
  - **Files:** project format v4 (v1 to v3 files upgrade).
  - **The drafted-T-shirt gate:** 0.00 mm penetration, seams welded, the cap notch on the shoulder seam, strain p99 4.5%.

- **M5 Workspaces: complete (2026-10-10).** Five workspace tabs (Cmd+1…5 and a View menu); Modeling holds everything above in a tidier layout: a tool strip of icons down the left of the pattern table, view options above it, a scrolling properties panel, and icon buttons for Play/Pause/Reset. The other four workspaces show what they will hold. One theme module holds the style and every colour; Phosphor icons (MIT).

- **M5b Studio view: complete (2026-10-10).** The 3D view is drawn by a studio renderer:
  - a soft grey studio with a shadow-catching floor;
  - the CC0 HDRI "Studio Small 08" baked to SH9 plus a soft key light;
  - Khronos PBR Neutral tone mapping, so a matte fabric facing the camera shows its colour within CIEDE2000 1.05;
  - cloth sheen and a lining shade;
  - soft key-light shadows and a floor contact shadow;
  - ambient occlusion at half resolution;
  - edge smoothing while moving, then about 16 sub-pixel-jittered frames averaged when still, after which drawing stops.

  View → 3D quality offers Auto, Basic, Medium and High; Auto follows the graphics chip, and the choice is remembered. Spec: `docs/superpowers/specs/2026-10-10-m5b-studio-view-design.md`.

## On approval: next steps
1. `git init` in `/Users/vivekpuajri/Developer/vibe-clo3d`, delete the stray `firebase-debug.log`, and save this plan as `docs/specs/2026-10-09-opendrape-design.md`.
2. Save memory notes: user is a non-developer and Claude writes all code; OpenDrape key decisions.
3. Write the detailed task-by-task M0 implementation plan (writing-plans skill), then execute M0.
   - M0 installs the Rust toolchain via rustup into the user's home directory.
   - Creating the public GitHub repo needs the user's GitHub account.
4. Optional funding track, in parallel: register a `funding.json` for FLOSS/fund (Zerodha), apply to the NLnet NGI Zero Commons Fund, and apply for a Digital Public Goods Alliance listing once 1.0 ships.
