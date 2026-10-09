# OpenDrape Dress Forms, Track A (forms and form engine) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build the women's torso, men's torso and soft-arm dress forms in Blender, ship them as ring files with four size charts, and give the Rust side everything needed to drape on them:
- load and validate the forms;
- resize to any chart size or custom measurement;
- build closed meshes, tape ribbons and the stand;
- attach arms;
- collide with torso, arms and floor;
- pass drape gates on every size.

**Architecture:**
- **Blender is the design tool.** `scripts/forms/loft_base.py` lofts a form from station tables into `assets-src/forms/<id>.blend`, and `render_views.py` makes review pictures. `export_form.py` samples the shape into `assets/forms/<id>.form.json`: ~100 rings × 49 radii, plus stations, landmarks and the hand-written meta.
- **Rust: `crates/body/src/form/`.**
  - Reads the JSON.
  - Resizes the rings: vertical stretches and a shoulder widening, solved by secant; then each girth station is scaled exactly, with a monotone PCHIP between stations.
  - Builds a closed, mirror-symmetric triangle mesh.
- **`crates/sim`** gains a `CompoundCollider` (torso + arms + floor).
- **`crates/testkit`** drapes the demo skirt and tube on the forms.
- **Nothing user-visible changes yet.** The app keeps the MakeHuman body until Track B.

**Tech Stack:**
- Existing: Rust 1.99.0, glam 0.33 (`DVec3`, `DQuat`), parry3d 0.31.1, rayon, serde/serde_json (already workspace deps, new to `opendrape-body`).
- New: Blender 5.2.0 LTS, headless (`-b --factory-startup`), at `/Applications/Blender.app` on the Mac. No new Rust dependencies.

**Spec:** `docs/superpowers/specs/2026-10-09-dress-forms-design.md` (sections "Approach", "What each form file carries", "Sizing", "Collision, rendering, stand", "Track A"). Track B (app, project file, retiring MakeHuman) is a separate plan after M2b merges.

**Evidence (2026-10-09, `scratchpad/blender-probe`):**
- In headless Blender 5.2, `loft_base.py`, `render_views.py` and `export_form.py` (as written below) built, rendered and exported the women's form: 101 rings, 27 landmarks, 62 KB.
- Station girths from the export: bust 89.6, under-bust 75.0, waist 69.0, hip 94.8, neck 34.7 cm. The chart base (US 8 classic) is 90 / 75 / 67.5 / 93 / 34.
- The men's form and the soft arm lofted and rendered cleanly too.
- Running Blender **without** `--factory-startup` loads the user's add-ons (BlenderKit and others go online), so every command here passes it.

**Deviations from the spec, decided while planning:**
- **Women's sizes are labelled "US 8" with "UK 12" as the alternative name. There are no letter sizes,** because letters map to two numeric sizes each. Men's sizes are labelled by chest in inches ("40"), as men's forms are sold.
- **Measurement definitions are code constants per part kind** (`TORSO_LENGTHS`, `ARM_LENGTHS`), not JSON: both torsos share them.
- **The tape lines drawn in Blender are replaced by landmark dots.** Tapes are computed from landmarks in Rust, so the review pictures show dots where the tapes will run.
- **No new CI job.** The bundled-form validation tests live in `crates/body/tests/forms.rs` and run in the existing `test` job on all three OSes.
- **`CompoundCollider` picks the nearest way out** when a particle is inside several parts. The spec's "deepest plane" would push it through the far side.

## Global Constraints

- **Never launch the OpenDrape app, or anything that opens a window or dialog, on the user's Mac without asking first.**
  - Blender runs only headless (`-b --factory-startup`), which opens no window.
  - The live Blender MCP session (Blender's GUI) is used only after the user says yes in chat.
- **Licence:** forms, meta files and charts are made in-house and released CC0 1.0. Never use MakeHuman/MPFB code or SMPL. Every new asset gets an `ASSETS.md` row in the same commit.
- Never use the name "CLO".
- Pinned toolchain `1.99.0`. Before each commit, run `cargo fmt --all`. At the end, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo deny check` and `cargo nextest run --workspace` all pass.
- **Parallel with M2b:** do not edit `crates/core`, `crates/geom`, `crates/io` or `crates/app` (including `crates/app/i18n`). Track A touches only:
  - `crates/body`, `crates/sim`, `crates/testkit`;
  - `scripts/forms/`, `assets-src/forms/`, `assets/forms/`;
  - `ASSETS.md`, `.gitignore` and `docs/`.
- **Keep every existing public API and test working unchanged:**
  - `BodyMesh::female_average`, `girth_at`, `boundary_edge_count`;
  - `garments::{body, collider, Garment, Scene::new, Scene::step, Scene.solver, SKIRT_PARTICLES, BODICE_PARTICLES}`;
  - `metrics::{measure, run, position_hash}`;
  - all 225 current tests.
- **3D coordinates:** metres, Y up, the form faces +Z, the form's **left is +X**. Blender is Z up with the front facing −Y. Convert OpenDrape `(x, y, z)` ↔ Blender `(x, −z, y)`.
- **Form angles:**
  - `phi` is in radians from centre front (+Z) towards +X (the form's left), stored 0..=π in 49 samples.
  - The right side is the mirror image.
  - A full ring is 96 samples at `Quality::Standard` and 64 at `Quality::Low`.
  - `v` = fractional ring index ÷ (rings − 1): 0 is the bottom ring, 1 the top.
- **Measurements:** `f64` millimetres in `Measurements`, charts and `SizeError`; metres everywhere else.
- **Placement:** every torso's waist station stays at its base height (≈ 1.03 m). The floor is y = 0.
- **Commits:** every commit message ends with `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`. Never push without asking the user, because pushing `main` rebuilds the public nightly. This branch is `dress-forms`.

## Review Focus

These are the inputs most likely to bite a user that no task's main tests exercise. Each has a test added to the task that owns the code.
1. **Custom measurement combinations** that pass each range but fight each other (tiny waist, huge hip) → a refusal naming a measurement, never a pinched or inside-out mesh. *Task 6: `any_targets_in_range_give_a_valid_shape_or_a_refusal`.*
2. **Non-finite input** (NaN or ∞ from a corrupted file or a bad parse) → a `SizeError`, no panic. *Task 6: `non_finite_input_is_refused`.*
3. **Arms on the smallest and largest sizes** → each arm still sinks just into its armhole plate and leaves room for a sleeve between arm and side. *Task 8: `arms_sit_on_their_plates_at_every_size`.*
4. **Low quality preset** → chart sizes still hit their girths and lengths. *Task 8: `low_quality_still_fits_the_charts`.*
5. **A hand-edited form file with a broken reference** (a tape naming a missing landmark, or a missing station) → `FormError` with a readable message. *Task 3: `broken_references_are_refused`.*

## File Structure

```
scripts/forms/
  loft_base.py        Blender: station tables → assets-src/forms/<id>.blend (Form/Arm mesh, st.* and lm.* empties)
  render_views.py     Blender: front/side/back/¾ review sheet (optionally with landmark dots)
  export_form.py      Blender: .blend + meta → assets/forms/<id>.form.json (rings, stations, landmarks)
  blender.sh          runs the three scripts headless:  loft | render | export  <ids>
  girths.py           plain Python: station girths of a .form.json (developer check)
assets-src/forms/
  <id>.blend          the editable master shapes (women-torso, men-torso, soft-arm)
  <id>.meta.json      hand-written: name, suits, tapes, inputs, ranges, collision, arm attachment
assets/forms/
  <id>.form.json      shipped form files (format 1)
  charts/<form>-classic.json, <form>-everyday.json   shipped size charts (mm)
crates/body/src/
  lib.rs              + `pub mod form;`
  measure.rs          hull in f64; girth_at also counts vertices lying on the slice plane
  form/mod.rs         Form, BuiltForm, BuiltArm, Arms, Side, Quality, SizeError, Measurements; build; bundled
  form/file.rs        FormFile & friends (serde), FormError, check(), measurement constants
  form/rings.rs       Rings: vertices, closed symmetric mesh, surface point/normal, girth, resampling
  form/tape.rs        Tape: anchors → Catmull-Rom on the surface, lengths, ribbons
  form/resize.rs      resize(): stretches, shoulder widening, exact girths, plausibility
  form/stand.rs       neck cap, pole and base (closed cylinders)
  form/chart.rs       Chart, ChartSize, bundled charts
  form/fixture.rs     #[cfg(test)] synthetic torso and arm
crates/body/tests/forms.rs     every bundled form and chart row
crates/sim/src/collide.rs      + Collider::signed_distance, CompoundCollider
crates/testkit/src/garments.rs Scene::new_on, Scene::collider; shared skirt_panels/tube/grid_panel
crates/testkit/src/forms.rs    collider(), skirt(), bodice_proxy(), sleeve(), long_hem() on a BuiltForm
crates/testkit/src/metrics.rs  measure(&dyn Collider), run_solver()
crates/testkit/tests/forms.rs  drape gates on the forms
```

---

### Task 1: Base shapes in Blender, and the user's approval

**Files:**
- Create: `scripts/forms/loft_base.py`, `scripts/forms/render_views.py`, `scripts/forms/blender.sh`
- Create: `assets-src/forms/women-torso.blend`, `assets-src/forms/men-torso.blend`, `assets-src/forms/soft-arm.blend` (generated)
- Modify: `.gitignore` (add `*.blend1`)

**Interfaces:**
- Produces, for Task 2, each `.blend` with:
  - a mesh object named `Form` (torsos) or `Arm` (soft arm), in metres, Blender axes;
  - empties `st.<station>` at each station height;
  - empties `lm.<landmark>` on the surface of the left side.
- Station names:
  - women: `bottom hip high_hip waist under_bust bust shoulder neck`;
  - men: `bottom hip high_hip waist chest shoulder neck`;
  - arm: `wrist elbow upper_arm top`.
- Landmark names: exactly the keys of `LANDMARKS` below.

- [ ] **Step 1: Write the loft script**

Create `scripts/forms/loft_base.py`:

```python
"""Builds a starting dress form from a station table, in Blender, headless.

    blender -b --factory-startup --python-exit-code 1 --python scripts/forms/loft_base.py -- <form-id> <out.blend>

The shape is a loft through horizontal sections. Each section is two half superellipses (front
depth and back depth) of one half-width, plus smooth bumps for the bust and shoulder blades.
Squarer sections (exponent n near 3) flatten the sides into armhole plates. Parameters between
stations follow a monotone cubic, so nothing overshoots. Saves the mesh ("Form", or "Arm" for
the soft arm), station empties "st.<name>" and landmark empties "lm.<name>" for the exporter.

Coordinates are OpenDrape's (cm here, y up, front +z) and are converted to Blender's
(metres, z up, front -y) only when vertices are created.
"""
import math
import sys

import bmesh
import bpy

# y, half-width w, front depth f, back depth b, side squareness n, centre z, all in cm.
STATIONS = {
    "women-torso": [
        (70.6, 12.0, 6.8, 8.0, 2.2, -1.0),
        (71.2, 14.6, 8.4, 9.9, 2.2, -1.0),
        (72.0, 15.6, 9.0, 10.6, 2.2, -1.0),
        (76.0, 17.0, 9.8, 11.6, 2.2, -1.0),
        (82.5, 18.0, 10.3, 12.3, 2.2, -1.0),
        (96.0, 16.3, 9.2, 10.8, 2.1, -0.5),
        (103.0, 13.3, 8.1, 8.7, 2.0, 0.0),
        (113.0, 14.4, 8.6, 9.1, 2.0, 0.5),
        (121.0, 16.8, 9.4, 10.2, 2.0, 0.5),
        (127.0, 16.4, 8.6, 9.6, 2.5, 0.3),
        (131.0, 16.8, 8.0, 9.6, 3.0, 0.0),
        (135.0, 16.9, 7.2, 8.4, 3.0, 0.0),
        (137.5, 16.2, 6.4, 7.4, 2.6, 0.0),
        (139.5, 13.2, 5.8, 6.8, 2.3, 0.1),
        (141.5, 9.2, 5.3, 6.2, 2.1, 0.3),
        (143.5, 5.9, 5.0, 5.8, 2.0, 0.5),
        (149.6, 5.5, 4.7, 5.4, 2.0, 1.4),
        (150.4, 4.7, 4.0, 4.6, 2.0, 1.5),
        (151.0, 2.8, 2.4, 2.8, 2.0, 1.6),
    ],
    "men-torso": [
        (70.6, 13.0, 7.8, 9.2, 2.2, -1.0),
        (71.2, 15.8, 9.4, 11.4, 2.2, -1.0),
        (72.0, 16.8, 10.0, 12.2, 2.2, -1.0),
        (76.0, 17.8, 10.6, 12.8, 2.2, -1.0),
        (83.5, 18.6, 11.0, 13.4, 2.2, -1.0),
        (96.0, 17.6, 11.0, 12.4, 2.1, -0.5),
        (103.0, 17.0, 11.4, 12.0, 2.1, 0.0),
        (114.0, 18.2, 11.4, 12.0, 2.1, 0.3),
        (124.0, 19.6, 11.6, 12.4, 2.2, 0.4),
        (129.0, 19.8, 10.8, 12.0, 2.6, 0.2),
        (134.0, 20.2, 9.6, 11.4, 3.0, 0.0),
        (139.0, 20.4, 8.4, 9.6, 3.0, 0.0),
        (142.0, 19.6, 7.4, 8.6, 2.6, 0.0),
        (144.5, 15.6, 6.6, 7.8, 2.3, 0.1),
        (147.0, 10.6, 6.2, 7.0, 2.1, 0.3),
        (149.5, 6.6, 6.0, 6.6, 2.0, 0.5),
        (156.6, 6.3, 5.6, 6.2, 2.0, 1.2),
        (157.4, 5.4, 4.8, 5.3, 2.0, 1.3),
        (158.0, 3.2, 2.8, 3.2, 2.0, 1.4),
    ],
    # Along the arm's own axis: y = 0 is the top of the arm, x points away from the body.
    "soft-arm": [
        (-58.0, 1.4, 1.6, 1.6, 2.0, 0.0),
        (-57.4, 2.2, 2.5, 2.5, 2.0, 0.0),
        (-56.5, 2.5, 2.9, 2.9, 2.0, 0.0),
        (-54.0, 2.6, 3.0, 3.0, 2.0, 0.0),
        (-42.0, 3.3, 3.8, 3.8, 2.0, 0.0),
        (-30.0, 3.6, 4.1, 4.1, 2.0, 0.0),
        (-18.0, 4.1, 4.6, 4.6, 2.0, 0.0),
        (-8.0, 4.4, 4.9, 4.9, 2.0, 0.0),
        (-2.0, 4.6, 5.0, 5.0, 2.0, 0.0),
        (1.5, 4.0, 4.4, 4.4, 2.0, 0.0),
        (3.5, 2.6, 3.0, 3.0, 2.0, 0.0),
        (4.4, 1.0, 1.2, 1.2, 2.0, 0.0),
    ],
}
# Smooth bumps added to the depth: (front?, x, y, sigma x, sigma y, amplitude), cm.
BUMPS = {
    "women-torso": [
        (True, 9.0, 121.0, 5.0, 6.5, 3.4),  # bust
        (False, 8.0, 131.0, 6.5, 6.5, 0.8),  # shoulder blades
    ],
    "men-torso": [
        (True, 9.5, 125.0, 7.0, 7.0, 0.8),  # chest
        (False, 9.0, 134.0, 7.0, 7.0, 1.0),  # shoulder blades
    ],
}
# Station heights (cm), saved as empties "st.<name>"; the exporter puts a ring exactly at each.
STATION_HEIGHTS = {
    "women-torso": {"bottom": 72.0, "hip": 82.5, "high_hip": 96.0, "waist": 103.0, "under_bust": 113.0,
                    "bust": 121.0, "shoulder": 137.5, "neck": 145.5},
    "men-torso": {"bottom": 72.0, "hip": 83.5, "high_hip": 96.0, "waist": 103.0, "chest": 124.0,
                  "shoulder": 142.0, "neck": 151.5},
    "soft-arm": {"wrist": -54.0, "elbow": -30.0, "upper_arm": -8.0, "top": 0.0},
}
# Landmarks: (degrees from centre front towards the form's left (+x), height cm), saved as
# empties "lm.<name>" on the surface. Only the left side is stored; the right is mirrored.
LANDMARKS = {
    "women-torso": {
        "front_neck": (0, 141.5), "side_neck": (92, 143.5), "back_neck": (180, 144.0),
        "shoulder_mid": (88, 140.5), "shoulder_point": (92, 137.5), "plate_centre": (92, 132.0),
        "armhole_front": (68, 131.0), "armhole_bottom": (92, 126.5), "armhole_back": (116, 131.0),
        "princess_chest": (52, 131.0), "bust_apex": (38, 121.0), "princess_waist": (30, 103.0),
        "princess_hip": (30, 82.5), "princess_bottom": (30, 72.0),
        "shoulder_blade": (140, 131.0), "back_princess_waist": (150, 103.0),
        "back_princess_hip": (150, 82.5), "back_princess_bottom": (150, 72.0),
        "side_bust": (92, 121.0), "side_waist": (92, 103.0), "side_hip": (92, 82.5), "side_bottom": (92, 72.0),
        "front_waist": (0, 103.0), "back_waist": (180, 103.0), "cf_bottom": (0, 72.0),
        "cb_blade": (180, 131.0), "cb_bottom": (180, 72.0),
    },
    "men-torso": {
        "front_neck": (0, 147.5), "side_neck": (92, 149.5), "back_neck": (180, 150.0),
        "shoulder_mid": (88, 146.0), "shoulder_point": (92, 142.0), "plate_centre": (92, 136.0),
        "armhole_front": (68, 135.0), "armhole_bottom": (92, 129.0), "armhole_back": (116, 135.0),
        "princess_chest": (52, 135.0), "chest_point": (36, 124.0), "princess_waist": (32, 103.0),
        "princess_hip": (32, 83.5), "princess_bottom": (32, 72.0),
        "shoulder_blade": (140, 134.0), "back_princess_waist": (150, 103.0),
        "back_princess_hip": (150, 83.5), "back_princess_bottom": (150, 72.0),
        "side_chest": (92, 124.0), "side_waist": (92, 103.0), "side_hip": (92, 83.5), "side_bottom": (92, 72.0),
        "front_waist": (0, 103.0), "back_waist": (180, 103.0), "cf_bottom": (0, 72.0),
        "cb_blade": (180, 134.0), "cb_bottom": (180, 72.0),
    },
    "soft-arm": {"arm_top": (90, 3.0), "arm_wrist": (90, -54.0)},
}


def pchip(xs, ys):
    """Monotone cubic (Fritsch-Carlson) through the points; returns f(x)."""
    n = len(xs)
    h = [xs[i + 1] - xs[i] for i in range(n - 1)]
    d = [(ys[i + 1] - ys[i]) / h[i] for i in range(n - 1)]
    m = [0.0] * n
    m[0], m[-1] = d[0], d[-1]
    for i in range(1, n - 1):
        if d[i - 1] * d[i] <= 0:
            m[i] = 0.0
        else:
            w1, w2 = 2 * h[i] + h[i - 1], h[i] + 2 * h[i - 1]
            m[i] = (w1 + w2) / (w1 / d[i - 1] + w2 / d[i])

    def f(x):
        x = min(max(x, xs[0]), xs[-1])
        i = max(k for k in range(n - 1) if xs[k] <= x) if x < xs[-1] else n - 2
        t = (x - xs[i]) / h[i]
        h00, h10 = 2 * t**3 - 3 * t**2 + 1, t**3 - 2 * t**2 + t
        h01, h11 = -2 * t**3 + 3 * t**2, t**3 - t**2
        return h00 * ys[i] + h10 * h[i] * m[i] + h01 * ys[i + 1] + h11 * h[i] * m[i + 1]

    return f


def section_point(form, curves, y, phi):
    """Point (x, z) in cm on the section at height y, for parameter phi from the front."""
    w, f, b, n, zc = (c(y) for c in curves)
    s, c = math.sin(phi), math.cos(phi)
    x = w * math.copysign(abs(s) ** (2 / n), s)
    z = (f if c >= 0 else b) * math.copysign(abs(c) ** (2 / n), c)
    for front, bx, by, sx, sy, amp in BUMPS.get(form, []):
        if (z >= 0) == front:
            g = math.exp(-((abs(x) - bx) / sx) ** 2 - ((y - by) / sy) ** 2)
            z += math.copysign(amp * g, z)
    return x, z + zc


def build(form, out_path, ring_step=0.5, around=128):
    rows = STATIONS[form]
    ys = [r[0] for r in rows]
    curves = [pchip(ys, [r[k] for r in rows]) for k in range(1, 6)]
    bpy.ops.wm.read_factory_settings(use_empty=True)
    mesh = bpy.data.meshes.new("Form")
    bm = bmesh.new()
    heights = [ys[0] + ring_step * i for i in range(int((ys[-1] - ys[0]) / ring_step) + 1)]
    rings = []
    for y in heights:
        ring = []
        for j in range(around):
            x, z = section_point(form, curves, y, 2 * math.pi * j / around)
            # OpenDrape (x, y, z) cm -> Blender (x, -z, y) m
            ring.append(bm.verts.new((x / 100, -z / 100, y / 100)))
        rings.append(ring)
    for i in range(len(rings) - 1):
        for j in range(around):
            k = (j + 1) % around
            bm.faces.new((rings[i][j], rings[i + 1][j], rings[i + 1][k], rings[i][k]))
    zc0, zc1 = curves[4](ys[0]), curves[4](ys[-1])
    bottom = bm.verts.new((0, -zc0 / 100, heights[0] / 100))
    top = bm.verts.new((0, -zc1 / 100, heights[-1] / 100))
    for j in range(around):
        k = (j + 1) % around
        bm.faces.new((bottom, rings[0][k], rings[0][j]))
        bm.faces.new((top, rings[-1][j], rings[-1][k]))
    bmesh.ops.recalc_face_normals(bm, faces=bm.faces)
    bm.to_mesh(mesh)
    bm.free()
    obj = bpy.data.objects.new("Arm" if form == "soft-arm" else "Form", mesh)
    bpy.context.scene.collection.objects.link(obj)
    for p in mesh.polygons:
        p.use_smooth = True
    add_markers(form, obj, curves)
    bpy.ops.wm.save_as_mainfile(filepath=out_path)


def add_markers(form, obj, curves):
    """Empties for stations (at the centre line) and landmarks (on the surface)."""
    from mathutils import Vector
    from mathutils.bvhtree import BVHTree

    bvh = BVHTree.FromObject(obj, bpy.context.evaluated_depsgraph_get())
    scene = bpy.context.scene.collection
    for name, y in STATION_HEIGHTS[form].items():
        e = bpy.data.objects.new(f"st.{name}", None)
        e.empty_display_type, e.empty_display_size = "CIRCLE", 0.2
        e.location = (0, -curves[4](y) / 100, y / 100)
        scene.objects.link(e)
    for name, (deg, y) in LANDMARKS[form].items():
        origin = Vector((0, -curves[4](y) / 100, y / 100))
        a = math.radians(deg)
        hit = bvh.ray_cast(origin, Vector((math.sin(a), -math.cos(a), 0)), 1.0)
        e = bpy.data.objects.new(f"lm.{name}", None)
        e.empty_display_type, e.empty_display_size = "SPHERE", 0.006
        e.location = hit[0] if hit[0] is not None else origin
        scene.objects.link(e)


if __name__ == "__main__":
    args = sys.argv[sys.argv.index("--") + 1 :]
    build(args[0], args[1])
```

- [ ] **Step 2: Write the review-sheet script**

Create `scripts/forms/render_views.py`:

```python
"""Renders front, side, back and three-quarter views of a form side by side into one PNG.

    blender -b --factory-startup <form.blend> --python-exit-code 1 \
        --python scripts/forms/render_views.py -- <out.png> [--dots]

Headless (Workbench engine, no window). Objects named "Form" and "Arm*" are shown; linked
copies are rotated about the vertical axis and spaced along x under one orthographic camera.
--dots adds a small dark sphere at every landmark and its mirror image.
"""
import math
import sys

import bpy

LINEN = (0.86, 0.83, 0.76, 1.0)
VIEWS = [("front", 0.0), ("side", -90.0), ("back", 180.0), ("three-quarter", -45.0)]
SPACING = 0.75


def add_dots(scene):
    """A small dark sphere at every landmark empty (and its mirror), so reviewers see the marks."""
    import bmesh

    dots = []
    for e in [o for o in scene.objects if o.name.startswith("lm.")]:
        for sx in (1, -1):
            me = bpy.data.meshes.new("Dot")
            bm = bmesh.new()
            bmesh.ops.create_uvsphere(bm, u_segments=12, v_segments=8, radius=0.005)
            bm.to_mesh(me)
            bm.free()
            dot = bpy.data.objects.new("Dot", me)
            dot.location = (e.location.x * sx, e.location.y, e.location.z)
            dot.color = (0.1, 0.1, 0.1, 1.0)
            scene.collection.objects.link(dot)
            dots.append(dot)
    return dots


def main(out_path, with_dots):
    scene = bpy.context.scene
    parts = [o for o in scene.objects if o.type == "MESH" and (o.name == "Form" or o.name.startswith("Arm"))]
    shown = parts + (add_dots(scene) if with_dots else [])
    group = bpy.data.collections.new("Views")
    scene.collection.children.link(group)
    for k, (_, deg) in enumerate(VIEWS):
        pivot = bpy.data.objects.new(f"View{k}", None)
        group.objects.link(pivot)
        pivot.location = (k * SPACING, 0, 0)
        pivot.rotation_euler = (0, 0, math.radians(deg))
        for p in shown:
            copy = p.copy() if k else p
            if k:
                group.objects.link(copy)
            copy.parent = pivot
            if p in parts:
                copy.color = LINEN
    zs = [(p.matrix_world @ v.co).z for p in parts for v in p.data.vertices]
    zmid, zspan = (min(zs) + max(zs)) / 2, max(zs) - min(zs)
    cam = bpy.data.objects.new("Cam", bpy.data.cameras.new("Cam"))
    scene.collection.objects.link(cam)
    cam.data.type = "ORTHO"
    cam.data.ortho_scale = max(SPACING * len(VIEWS), zspan * 1.15 * 1600 / 700)
    cam.location = (SPACING * (len(VIEWS) - 1) / 2, -5.0, zmid)
    cam.rotation_euler = (math.radians(90), 0, 0)
    scene.camera = cam
    scene.render.engine = "BLENDER_WORKBENCH"
    shading = scene.display.shading
    shading.light = "STUDIO"
    shading.color_type = "OBJECT"
    shading.show_cavity = True
    scene.render.resolution_x = 1600
    scene.render.resolution_y = 700
    scene.render.film_transparent = False
    scene.world = scene.world or bpy.data.worlds.new("World")
    scene.world.color = (0.92, 0.92, 0.92)
    scene.render.filepath = out_path
    bpy.ops.render.render(write_still=True)


if __name__ == "__main__":
    args = sys.argv[sys.argv.index("--") + 1 :]
    main(args[0], "--dots" in args[1:])
```

- [ ] **Step 3: Write the runner**

Create `scripts/forms/blender.sh` and make it executable (`chmod +x scripts/forms/blender.sh`). The `export` command is used from Task 2 on:

```sh
#!/bin/sh
# Runs the dress-form scripts in Blender without opening a window.
#   scripts/forms/blender.sh loft   <form-id>...  rebuild assets-src/forms/<id>.blend from the
#                                                 tables in loft_base.py (overwrites hand-shaping!)
#   scripts/forms/blender.sh render <form-id>...  target/forms/<id>-views.png and <id>-dots.png
#   scripts/forms/blender.sh export <form-id>...  assets/forms/<id>.form.json
# BLENDER overrides the Blender executable (default: the macOS app).
set -eu
B=${BLENDER:-/Applications/Blender.app/Contents/MacOS/Blender}
cd "$(dirname "$0")/../.."
root=$PWD
cmd=$1
shift
mkdir -p target/forms assets-src/forms assets/forms
for id in "$@"; do
  blend="$root/assets-src/forms/$id.blend"
  case $cmd in
    loft) "$B" -b --factory-startup --python-exit-code 1 --python scripts/forms/loft_base.py -- "$id" "$blend" ;;
    render)
      "$B" -b --factory-startup "$blend" --python-exit-code 1 --python scripts/forms/render_views.py -- "$root/target/forms/$id-views.png"
      "$B" -b --factory-startup "$blend" --python-exit-code 1 --python scripts/forms/render_views.py -- "$root/target/forms/$id-dots.png" --dots
      ;;
    export) "$B" -b --factory-startup "$blend" --python-exit-code 1 --python scripts/forms/export_form.py -- "$root/assets-src/forms/$id.meta.json" "$root/assets/forms/$id.form.json" ;;
    *) echo "usage: $0 <loft|render|export> <form-id>..." >&2; exit 2 ;;
  esac
done
```

Append to `.gitignore`:

```
*.blend1
```

- [ ] **Step 4: Loft and render all three parts**

Run: `sh scripts/forms/blender.sh loft women-torso men-torso soft-arm`
Expected: three `Info: Saved as "<id>.blend"` lines and `Blender quit`, with no Python traceback.

Run: `sh scripts/forms/blender.sh render women-torso men-torso soft-arm`
Expected: six `Saved: '.../target/forms/<id>-views.png'` or `-dots.png` lines.

Open every PNG with the Read tool and check it against this list:
- Front and back are mirror-symmetric.
- There is no anatomy. The bust is one smooth idealised shape, as on a school form.
- The surface is smooth, with no steps or creases.
- The armhole plates are flat below sloping, rounded shoulders.
- The neck stub is short with a flat top.
- The bottom is a rounded rim with a flat base.
- The dots sit on the neck base, shoulder, apex, waist, hip and side lines.

If any item fails, adjust the station table (`STATIONS`, `BUMPS`, `LANDMARKS`) and loft and render again.

- [ ] **Step 5: STOP: the user approves the shapes**

Send all six PNGs with `SendUserFile` (`status: "proactive"`). Use plain words, for example:

> "Here are the three dress forms as they stand now: women's torso, men's torso, and the soft arm. Each sheet shows front, side, back and ¾. The 'dots' sheets mark where the tape lines will run. Do they look like the forms you'd drape on? Tell me anything to change: shoulders, bust, hips, neck, overall proportions."

**Wait for the user's answer.** Do not start Task 2 until they approve.

For each change requested:
- **Prefer editing the tables** in `loft_base.py`, which keeps the shape reproducible. Then loft, render and send again.
- **Only if a table edit can't express the change,** ask the user in chat whether you may open Blender (its window will appear on their Mac). Use the Blender MCP session to shape the mesh, then save over `assets-src/forms/<id>.blend`.
  - After that, **never run `loft` for that form again.**
  - Note the hand-shaping in Task 2's `ASSETS.md` row.
  - Hand-shaping must keep the mesh mirror-symmetric (a Mirror modifier) and every horizontal section star-shaped around the centre line, or the exporter will refuse it.

- [ ] **Step 6: Commit**

```bash
git add scripts/forms/loft_base.py scripts/forms/render_views.py scripts/forms/blender.sh .gitignore assets-src/forms/*.blend
git commit -m "feat(forms): women's and men's torso and soft arm shaped in Blender

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 2: Export the forms to ring files

**Files:**
- Create: `scripts/forms/export_form.py`, `scripts/forms/girths.py`
- Create: `assets-src/forms/women-torso.meta.json`, `assets-src/forms/men-torso.meta.json`, `assets-src/forms/soft-arm.meta.json`
- Create: `assets/forms/women-torso.form.json`, `assets/forms/men-torso.form.json`, `assets/forms/soft-arm.form.json` (generated)
- Modify: `ASSETS.md`

**Interfaces:**
- Consumes: the `.blend` files and marker names from Task 1.
- Produces `assets/forms/<id>.form.json` in format 1. Keys:
  - `format`, `id`, `kind` (`torso` | `arm`), `name`, `suits`, `licence`, `base_size`;
  - `angles` (49);
  - `rings`: `[{y, zc, r[49]}]`, with `y` and `zc` in metres and `r` in mm;
  - `stations`: `{name: ring index}`;
  - `landmarks`: `{name: [phi, v]}`;
  - `tapes`: `{name: {ring: station} | {anchors: [...], closed?, mirror?}}`;
  - `inputs`: `[name]`;
  - `ranges`: `{name: [lo_mm, hi_mm]}`;
  - `collision`: `{thickness, friction}`;
  - `arm` (torso only): `{form, anchor, hang_deg, overlap}`.

- [ ] **Step 1: Write the exporter**

Create `scripts/forms/export_form.py`:

```python
"""Samples a shaped form in a .blend into OpenDrape's ring format and writes <id>.form.json.

    blender -b --factory-startup <form.blend> --python-exit-code 1 \
        --python scripts/forms/export_form.py -- <meta.json> <out.form.json>

The mesh object "Form" (or "Arm") is read with its modifiers applied. At each ring height, rays
from the centre line at 49 angles (centre front 0 deg to centre back 180 deg, the form's left
side) give the radii; the right side is the mirror image. Rings sit exactly at every station
("st.<name>" empties) and are spread between stations by surface length, so flat areas such as
the shoulder tops get more rings. Landmarks ("lm.<name>" empties) are stored as (angle, v), where
v runs 0 (bottom ring) to 1 (top ring); a landmark named "<x>_<station>" sits exactly on that
station's ring. Everything else (name, tapes, ranges...) comes from the hand-written meta file.
Exits non-zero, naming the height, if the shape is not star-shaped around its centre line (a ray
leaves the surface and meets it again).
"""
import json
import math
import sys

import bpy
from mathutils import Vector
from mathutils.bvhtree import BVHTree

ANGLES = 49  # samples from 0 to 180 degrees inclusive
RINGS = 100  # rings in the exported file (rounding may add one)
SCAN_STEP = 0.002  # m between scan slices when measuring surface length


def od_to_blender(x, y, z):
    return Vector((x, -z, y))


def blender_to_od(v):
    return v.x, v.z, -v.y


class Sampler:
    def __init__(self, obj):
        dg = bpy.context.evaluated_depsgraph_get()
        self.bvh = BVHTree.FromObject(obj, dg)
        ys = [blender_to_od(obj.matrix_world @ v.co)[1] for v in obj.evaluated_get(dg).data.vertices]
        self.y_min, self.y_max = min(ys), max(ys)

    def cast(self, origin, d):
        hit = self.bvh.ray_cast(origin, d, 2.0)
        if hit[0] is None:
            return None
        again = self.bvh.ray_cast(hit[0] + d * 1e-4, d, 2.0)
        if again[0] is not None:
            y = blender_to_od(hit[0])[1]
            raise SystemExit(f"not star-shaped at y={y:.4f} m: a ray meets the surface twice")
        return hit[3]

    def centre_z(self, y):
        """Midpoint of the front and back surface along x = 0."""
        guess = 0.0
        for _ in range(2):
            o = od_to_blender(0, y, guess)
            front = self.cast(o, od_to_blender(0, 0, 1) - od_to_blender(0, 0, 0))
            back = self.cast(o, od_to_blender(0, 0, -1) - od_to_blender(0, 0, 0))
            if front is None or back is None:
                raise SystemExit(f"no surface in front of or behind the centre line at y={y:.4f} m")
            guess += (front - back) / 2
        return guess

    def ring(self, y):
        zc = self.centre_z(y)
        o = od_to_blender(0, y, zc)
        radii = []
        for k in range(ANGLES):
            a = math.pi * k / (ANGLES - 1)
            d = od_to_blender(math.sin(a), 0, math.cos(a)) - od_to_blender(0, 0, 0)
            r = self.cast(o, d)
            if r is None:
                raise SystemExit(f"ray missed the form at y={y:.4f} m, angle {math.degrees(a):.1f} deg")
            radii.append(r)
        return zc, radii


def ring_heights(s, stations):
    """About RINGS heights: one exactly at each station, the rest spread by surface length."""
    lo, hi = s.y_min + 0.001, s.y_max - 0.001
    steps = int((hi - lo) / SCAN_STEP)
    ys = [lo + (hi - lo) * i / steps for i in range(steps + 1)]
    rings = [s.ring(y) for y in ys]
    arc = [0.0]
    for i in range(1, len(ys)):
        dr = max(abs(a - b) for a, b in zip(rings[i][1], rings[i - 1][1]))
        arc.append(arc[-1] + math.hypot(ys[i] - ys[i - 1], dr))

    def arc_at(y):
        i = min(max(0, int((y - lo) / (hi - lo) * (len(ys) - 1))), len(ys) - 2)
        t = (y - ys[i]) / (ys[i + 1] - ys[i])
        return arc[i] + t * (arc[i + 1] - arc[i])

    def y_at(a):
        i = next(k for k in range(1, len(arc)) if arc[k] >= a) if a < arc[-1] else len(arc) - 1
        t = (a - arc[i - 1]) / max(arc[i] - arc[i - 1], 1e-12)
        return ys[i - 1] + t * (ys[i] - ys[i - 1])

    fixed = sorted({lo, hi, *stations.values()})
    per = (RINGS - 1) / arc[-1]
    out = [fixed[0]]
    for a, b in zip(fixed, fixed[1:]):
        n = max(1, round((arc_at(b) - arc_at(a)) * per))
        out += [y_at(arc_at(a) + (arc_at(b) - arc_at(a)) * j / n) for j in range(1, n)] + [b]
    return out


def main(meta_path, out_path):
    meta = json.load(open(meta_path))
    obj = bpy.data.objects.get("Form") or bpy.data.objects["Arm"]
    s = Sampler(obj)
    stations = {o.name[3:]: blender_to_od(o.location)[1] for o in bpy.data.objects if o.name.startswith("st.")}
    heights = ring_heights(s, stations)
    rings = [s.ring(y) for y in heights]
    n = len(heights)
    station_index = {k: min(range(n), key=lambda i: abs(heights[i] - y)) for k, y in stations.items()}

    def v_of(y):
        i = min(max(0, max(k for k in range(n) if heights[k] <= y) if y >= heights[0] else 0), n - 2)
        return (i + (y - heights[i]) / (heights[i + 1] - heights[i])) / (n - 1)

    landmarks = {}
    for o in bpy.data.objects:
        if not o.name.startswith("lm."):
            continue
        name = o.name[3:]
        x, y, z = blender_to_od(o.location)
        zc = rings[min(range(n), key=lambda i: abs(heights[i] - y))][0]
        phi = math.atan2(abs(x), z - zc)
        v = v_of(y)
        for st, i in station_index.items():
            if name.endswith("_" + st):
                v = i / (n - 1)
        landmarks[name] = [round(phi, 5), round(v, 6)]
    form = {
        "format": 1,
        **{k: meta[k] for k in ("id", "kind", "name", "suits", "licence", "base_size")},
        "angles": ANGLES,
        "rings": [{"y": round(y, 5), "zc": round(zc, 5), "r": [round(r * 1000, 1) for r in radii]}
                  for y, (zc, radii) in zip(heights, rings)],
        "stations": dict(sorted(station_index.items(), key=lambda kv: kv[1])),
        "landmarks": dict(sorted(landmarks.items())),
        **{k: meta[k] for k in ("tapes", "inputs", "ranges", "collision") if k in meta},
    }
    if "arm" in meta:
        form["arm"] = meta["arm"]
    with open(out_path, "w") as f:
        json.dump(form, f, indent=1)
        f.write("\n")
    print(f"wrote {out_path}: {n} rings, {len(landmarks)} landmarks, stations {form['stations']}")


if __name__ == "__main__":
    args = sys.argv[sys.argv.index("--") + 1 :]
    main(args[0], args[1])
```

Create `scripts/forms/girths.py` (plain Python, a developer check):

```python
"""Prints the station girths of a .form.json in cm (convex hull of each station ring).

    python3 -I scripts/forms/girths.py assets/forms/women-torso.form.json
"""
import json
import math
import sys

form = json.load(open(sys.argv[1]))
K = form["angles"]


def ring_points(ring):
    m = 2 * (K - 1)
    pts = []
    for j in range(m):
        k = j if j < K else m - j
        a = 2 * math.pi * j / m
        r = ring["r"][k] / 1000
        pts.append((r * math.sin(a), ring["zc"] + r * math.cos(a)))
    return pts


def hull_perimeter(pts):
    pts = sorted(set(pts))

    def cross(o, a, b):
        return (a[0] - o[0]) * (b[1] - o[1]) - (a[1] - o[1]) * (b[0] - o[0])

    lower, upper = [], []
    for p in pts:
        while len(lower) >= 2 and cross(lower[-2], lower[-1], p) <= 0:
            lower.pop()
        lower.append(p)
    for p in reversed(pts):
        while len(upper) >= 2 and cross(upper[-2], upper[-1], p) <= 0:
            upper.pop()
        upper.append(p)
    hull = lower[:-1] + upper[:-1]
    return sum(math.dist(hull[i], hull[(i + 1) % len(hull)]) for i in range(len(hull)))


for name, i in form["stations"].items():
    ring = form["rings"][i]
    print(f"{name:12s} y={ring['y']:.3f}  girth {hull_perimeter(ring_points(ring)) * 100:6.1f} cm")
```

- [ ] **Step 2: Write the three meta files**

Create `assets-src/forms/women-torso.meta.json`:

```json
{
  "id": "women-torso",
  "kind": "torso",
  "name": "form-women-torso",
  "suits": ["dresses", "tops", "blouses", "skirts", "kurtas"],
  "licence": "CC0-1.0",
  "base_size": "US 8",
  "tapes": {
    "cf": {"anchors": ["front_neck", "front_waist", "cf_bottom"]},
    "cb": {"anchors": ["back_neck", "cb_blade", "back_waist", "cb_bottom"]},
    "neckline": {"anchors": ["front_neck", "side_neck", "back_neck", "side_neck_R"], "closed": true},
    "shoulder_seam": {"anchors": ["side_neck", "shoulder_point"], "mirror": true},
    "armhole": {"anchors": ["shoulder_point", "armhole_front", "armhole_bottom", "armhole_back"], "closed": true, "mirror": true},
    "side_seam": {"anchors": ["armhole_bottom", "side_bust", "side_waist", "side_hip", "side_bottom"], "mirror": true},
    "princess_front": {"anchors": ["shoulder_mid", "princess_chest", "bust_apex", "princess_waist", "princess_hip", "princess_bottom"], "mirror": true},
    "princess_back": {"anchors": ["shoulder_mid", "shoulder_blade", "back_princess_waist", "back_princess_hip", "back_princess_bottom"], "mirror": true},
    "back_width": {"anchors": ["armhole_back_R", "cb_blade", "armhole_back"]},
    "bust": {"ring": "bust"},
    "under_bust": {"ring": "under_bust"},
    "waist": {"ring": "waist"},
    "high_hip": {"ring": "high_hip"},
    "hip": {"ring": "hip"}
  },
  "inputs": ["bust", "under_bust", "waist", "hip", "neck", "shoulder_length", "back_waist_length", "waist_to_hip"],
  "ranges": {
    "bust": [700, 1400], "under_bust": [600, 1250], "waist": [500, 1300], "hip": [750, 1500],
    "neck": [280, 480], "shoulder_length": [90, 190], "back_waist_length": [330, 520], "waist_to_hip": [150, 280]
  },
  "collision": {"thickness": 0.003, "friction": 0.4},
  "arm": {"form": "soft-arm", "anchor": "plate_centre", "hang_deg": 12.0, "overlap": 0.01}
}
```

Create `assets-src/forms/men-torso.meta.json`:

```json
{
  "id": "men-torso",
  "kind": "torso",
  "name": "form-men-torso",
  "suits": ["shirts", "kurtas", "waistcoats", "jackets"],
  "licence": "CC0-1.0",
  "base_size": "40",
  "tapes": {
    "cf": {"anchors": ["front_neck", "front_waist", "cf_bottom"]},
    "cb": {"anchors": ["back_neck", "cb_blade", "back_waist", "cb_bottom"]},
    "neckline": {"anchors": ["front_neck", "side_neck", "back_neck", "side_neck_R"], "closed": true},
    "shoulder_seam": {"anchors": ["side_neck", "shoulder_point"], "mirror": true},
    "armhole": {"anchors": ["shoulder_point", "armhole_front", "armhole_bottom", "armhole_back"], "closed": true, "mirror": true},
    "side_seam": {"anchors": ["armhole_bottom", "side_chest", "side_waist", "side_hip", "side_bottom"], "mirror": true},
    "princess_front": {"anchors": ["shoulder_mid", "princess_chest", "chest_point", "princess_waist", "princess_hip", "princess_bottom"], "mirror": true},
    "princess_back": {"anchors": ["shoulder_mid", "shoulder_blade", "back_princess_waist", "back_princess_hip", "back_princess_bottom"], "mirror": true},
    "back_width": {"anchors": ["armhole_back_R", "cb_blade", "armhole_back"]},
    "chest": {"ring": "chest"},
    "waist": {"ring": "waist"},
    "high_hip": {"ring": "high_hip"},
    "hip": {"ring": "hip"}
  },
  "inputs": ["chest", "waist", "hip", "neck", "shoulder_length", "back_waist_length", "waist_to_hip"],
  "ranges": {
    "chest": [750, 1500], "waist": [600, 1450], "hip": [750, 1500], "neck": [300, 520],
    "shoulder_length": [110, 210], "back_waist_length": [380, 560], "waist_to_hip": [150, 280]
  },
  "collision": {"thickness": 0.003, "friction": 0.4},
  "arm": {"form": "soft-arm", "anchor": "plate_centre", "hang_deg": 12.0, "overlap": 0.01}
}
```

Create `assets-src/forms/soft-arm.meta.json`:

```json
{
  "id": "soft-arm",
  "kind": "arm",
  "name": "form-soft-arm",
  "suits": [],
  "licence": "CC0-1.0",
  "base_size": "US 8",
  "tapes": {
    "outer": {"anchors": ["arm_top", "arm_wrist"]},
    "upper_arm": {"ring": "upper_arm"},
    "wrist": {"ring": "wrist"}
  },
  "inputs": ["upper_arm", "arm_length"],
  "ranges": {"upper_arm": [200, 480], "arm_length": [450, 700]},
  "collision": {"thickness": 0.003, "friction": 0.4}
}
```

- [ ] **Step 3: Export and check the base sizes**

Run: `sh scripts/forms/blender.sh export women-torso men-torso soft-arm`
Expected: three lines like `wrote .../women-torso.form.json: 101 rings, 27 landmarks, stations {...}` (men: 27 landmarks; arm: 2), with no `not star-shaped` or `ray missed` exit.

Run: `python3 -I scripts/forms/girths.py assets/forms/women-torso.form.json`, then the same for `men-torso` and `soft-arm`.
Expected: each girth within ±3 cm of its chart's base row:
- **women (US 8 classic):** bust 90, under-bust 75, waist 67.5, hip 93, neck 34;
- **men (40 classic):** chest 101.5, waist 91, hip 98.5, neck 39.5;
- **arm:** upper_arm 28.

If a girth is further off, change that station's half-width and depths in `loft_base.py` by the same factor. Loft, render, glance at the pictures and export again. Tell the user if the change is visible. (On a hand-shaped `.blend`, scale that region in Blender instead.)

Each `.form.json` should be 40–80 KB.

- [ ] **Step 4: Record provenance**

Append to the table in `ASSETS.md`. If Task 1 involved hand-shaping, say which form in the first row.

```markdown
| `assets-src/forms/women-torso.blend`, `men-torso.blend`, `soft-arm.blend` | Made in-house in Blender 5.2.0 LTS by `scripts/forms/loft_base.py` (its station tables are the design) | CC0 1.0 |
| `assets-src/forms/*.meta.json` | Written in-house: names, tape lines, measurement inputs and ranges | CC0 1.0 |
| `assets/forms/*.form.json` | Exported from the `.blend` files by `scripts/forms/export_form.py` (`scripts/forms/blender.sh export`) | CC0 1.0 |
```

- [ ] **Step 5: Commit**

```bash
git add scripts/forms/export_form.py scripts/forms/girths.py assets-src/forms/*.meta.json assets/forms/*.form.json ASSETS.md
git commit -m "feat(forms): export the dress forms as ring files with landmarks and tape lines

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 3: The form file in Rust: types, checks, test fixture

**Files:**
- Modify: `crates/body/Cargo.toml`, `crates/body/src/lib.rs`
- Create: `crates/body/src/form/mod.rs`, `crates/body/src/form/file.rs`, `crates/body/src/form/fixture.rs`

**Interfaces:**
- Consumes: the format from Task 2, and `assets-src/forms/women-torso.meta.json` and `soft-arm.meta.json` (the fixture reuses their tapes, inputs and ranges).
- Produces:
  - `opendrape_body::form::{FormFile, Ring, TapeDef, Collision, ArmAttach, Kind, FormError}`;
  - constants `FORMAT = 1`, `ANGLES = 49`, `TORSO_LENGTHS`, `ARM_LENGTHS`, `ADJUSTABLE_LENGTHS`, `TORSO_STATIONS`, `ARM_STATIONS`;
  - `FormFile::from_json(&str) -> Result<FormFile, FormError>`, `FormFile::check(&self) -> Result<(), FormError>`;
  - `FormFile::lengths(&self) -> &'static [(&str, &str, &str, &str)]` (measurement, tape, from anchor, to anchor);
  - `FormFile::chest_station(&self) -> Option<&str>`;
  - test-only `fixture::torso() -> FormFile` (81 rings at y = 0.70 + 0.01·i m; stations bottom 2, hip 12, high_hip 26, waist 33, under_bust 43, bust 51, shoulder 67, neck 75) and `fixture::arm() -> FormFile` (32 rings at y = −0.58 + 0.02·i; stations wrist 2, elbow 14, upper_arm 25, top 29).

- [ ] **Step 1: Add the dependencies and module**

In `crates/body/Cargo.toml`, change the description to `"OpenDrape dress forms, bodies and body measurements"` and add to `[dependencies]`:

```toml
serde.workspace = true
serde_json.workspace = true
```

In `crates/body/src/lib.rs`, replace the crate doc and module list with:

```rust
//! The 3D shapes garments drape on: dress forms (rings resized to a size chart, see [`form`]),
//! the legacy CC0 MakeHuman body, a compact mesh file format and tape-measure measurements.

pub mod form;
mod format;
mod measure;
```

(The `pub use` lines, `BodyMesh` and `female_average` stay as they are.)

Create `crates/body/src/form/mod.rs`:

```rust
//! Dress forms: shapes stored as horizontal rings (see `file`), resized to a size chart or to
//! custom measurements, and built into closed meshes for collision and drawing.

mod file;
#[cfg(test)]
mod fixture;

pub use file::{
    ADJUSTABLE_LENGTHS, ANGLES, ARM_LENGTHS, ARM_STATIONS, ArmAttach, Collision, FORMAT, FormError,
    FormFile, Kind, Ring, TORSO_LENGTHS, TORSO_STATIONS, TapeDef,
};
```

- [ ] **Step 2: Write the failing tests**

Create `crates/body/src/form/file.rs` with only the test module for now:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::form::fixture;

    #[test]
    fn fixture_torso_and_arm_pass_the_checks() {
        fixture::torso().check().unwrap();
        fixture::arm().check().unwrap();
    }

    #[test]
    fn round_trips_through_json() {
        let f = fixture::torso();
        let json = serde_json::to_string(&f).unwrap();
        assert_eq!(FormFile::from_json(&json).unwrap(), f);
    }

    #[test]
    fn json_that_is_not_a_form_is_refused() {
        assert!(
            FormFile::from_json("{}")
                .unwrap_err()
                .0
                .starts_with("not a form file")
        );
    }

    #[test]
    fn broken_references_are_refused() {
        type Breaker = Box<dyn Fn(&mut FormFile)>;
        let cases: Vec<(Breaker, &str)> = vec![
            (Box::new(|f| { let _ = f.landmarks.remove("side_hip"); }), "side_hip"),
            (Box::new(|f| { let _ = f.stations.remove("waist"); }), "waist"),
            (Box::new(|f| { let _ = f.ranges.remove("hip"); }), "hip"),
            (Box::new(|f| { let _ = f.rings[3].r.pop(); }), "ring 3"),
            (Box::new(|f| f.rings[5].y = f.rings[4].y), "ring 5"),
            (Box::new(|f| f.format = 2), "format 2"),
            (Box::new(|f| f.inputs.push("front_waist_length".into())), "front_waist_length"),
            (Box::new(|f| f.arm.as_mut().unwrap().anchor = "elbow".into()), "elbow"),
        ];
        for (break_it, needle) in cases {
            let mut f = fixture::torso();
            break_it(&mut f);
            let err = f.check().unwrap_err().0;
            assert!(err.contains(needle), "{err:?} should mention {needle}");
        }
    }
}
```

Create `crates/body/src/form/fixture.rs`:

```rust
//! A synthetic torso and arm for unit tests: smooth elliptical rings, the shipped meta files'
//! tape lines, inputs and ranges, and landmarks at the same angles as the shipped forms.

use super::file::{ANGLES, FORMAT, FormFile, Kind, Ring};
use serde::de::DeserializeOwned;
use std::f64::consts::PI;

/// Piecewise-linear lookup in a (y, value) table sorted by y, flat beyond its ends.
fn lerp(table: &[(f64, f64)], y: f64) -> f64 {
    let k = table.partition_point(|p| p.0 <= y).clamp(1, table.len() - 1);
    let ((y0, a), (y1, b)) = (table[k - 1], table[k]);
    a + (b - a) * ((y - y0) / (y1 - y0)).clamp(0.0, 1.0)
}

/// An ellipse of half-width `a` (x) and depth `b` (z), metres, as ring radii in millimetres.
fn ellipse_ring(y: f64, a: f64, b: f64) -> Ring {
    let r = (0..ANGLES)
        .map(|k| {
            let t = PI * k as f64 / (ANGLES - 1) as f64;
            1000.0 * a * b / ((b * t.sin()).powi(2) + (a * t.cos()).powi(2)).sqrt()
        })
        .collect();
    Ring { y, zc: 0.0, r }
}

fn field<T: DeserializeOwned>(meta: &serde_json::Value, key: &str) -> T {
    serde_json::from_value(meta[key].clone()).expect("meta field")
}

/// `landmarks` are (name, degrees from centre front, fractional ring index).
fn from_meta(
    meta: &str,
    kind: Kind,
    rings: Vec<Ring>,
    stations: &[(&str, usize)],
    landmarks: &[(&str, f64, f64)],
) -> FormFile {
    let meta: serde_json::Value = serde_json::from_str(meta).expect("meta is JSON");
    let top = (rings.len() - 1) as f64;
    FormFile {
        format: FORMAT,
        id: "test".into(),
        kind,
        name: "form-test".into(),
        suits: vec![],
        licence: "CC0-1.0".into(),
        base_size: "test".into(),
        angles: ANGLES,
        rings,
        stations: stations.iter().map(|&(s, i)| (s.to_string(), i)).collect(),
        landmarks: landmarks
            .iter()
            .map(|&(l, deg, i)| (l.to_string(), [deg.to_radians(), i / top]))
            .collect(),
        tapes: field(&meta, "tapes"),
        inputs: field(&meta, "inputs"),
        ranges: field(&meta, "ranges"),
        collision: field(&meta, "collision"),
        arm: meta.get("arm").map(|_| field(&meta, "arm")),
    }
}

/// 81 rings 1 cm apart from y = 0.70 m; the waist (ring 33) is at 1.03 m.
pub fn torso() -> FormFile {
    const A: [(f64, f64); 7] = [
        (0.70, 0.150),
        (0.82, 0.165),
        (1.03, 0.130),
        (1.21, 0.160),
        (1.37, 0.165),
        (1.43, 0.060),
        (1.50, 0.055),
    ];
    const B: [(f64, f64); 7] = [
        (0.70, 0.100),
        (0.82, 0.110),
        (1.03, 0.085),
        (1.21, 0.100),
        (1.37, 0.070),
        (1.43, 0.055),
        (1.50, 0.050),
    ];
    let rings = (0..=80)
        .map(|i| {
            let y = 0.70 + 0.01 * i as f64;
            ellipse_ring(y, lerp(&A, y), lerp(&B, y))
        })
        .collect();
    let at = |y: f64| (y - 0.70) / 0.01;
    from_meta(
        include_str!("../../../../assets-src/forms/women-torso.meta.json"),
        Kind::Torso,
        rings,
        &[
            ("bottom", 2),
            ("hip", 12),
            ("high_hip", 26),
            ("waist", 33),
            ("under_bust", 43),
            ("bust", 51),
            ("shoulder", 67),
            ("neck", 75),
        ],
        &[
            ("front_neck", 0.0, at(1.41)),
            ("side_neck", 92.0, at(1.43)),
            ("back_neck", 180.0, at(1.44)),
            ("shoulder_mid", 88.0, at(1.40)),
            ("shoulder_point", 92.0, at(1.37)),
            ("plate_centre", 92.0, at(1.32)),
            ("armhole_front", 68.0, at(1.31)),
            ("armhole_bottom", 92.0, at(1.26)),
            ("armhole_back", 116.0, at(1.31)),
            ("princess_chest", 52.0, at(1.31)),
            ("bust_apex", 38.0, at(1.21)),
            ("princess_waist", 30.0, at(1.03)),
            ("princess_hip", 30.0, at(0.82)),
            ("princess_bottom", 30.0, at(0.72)),
            ("shoulder_blade", 140.0, at(1.31)),
            ("back_princess_waist", 150.0, at(1.03)),
            ("back_princess_hip", 150.0, at(0.82)),
            ("back_princess_bottom", 150.0, at(0.72)),
            ("side_bust", 92.0, at(1.21)),
            ("side_waist", 92.0, at(1.03)),
            ("side_hip", 92.0, at(0.82)),
            ("side_bottom", 92.0, at(0.72)),
            ("front_waist", 0.0, at(1.03)),
            ("back_waist", 180.0, at(1.03)),
            ("cf_bottom", 0.0, at(0.72)),
            ("cb_blade", 180.0, at(1.31)),
            ("cb_bottom", 180.0, at(0.72)),
        ],
    )
}

/// 32 rings 2 cm apart from y = −0.58 m; the arm's top station is ring 29 (y = 0).
pub fn arm() -> FormFile {
    const R: [(f64, f64); 7] = [
        (-0.58, 0.015),
        (-0.56, 0.026),
        (-0.30, 0.038),
        (-0.08, 0.046),
        (0.0, 0.046),
        (0.03, 0.030),
        (0.04, 0.012),
    ];
    let rings = (0..32)
        .map(|i| {
            let y = -0.58 + 0.02 * i as f64;
            let r = lerp(&R, y);
            ellipse_ring(y, r, r)
        })
        .collect();
    from_meta(
        include_str!("../../../../assets-src/forms/soft-arm.meta.json"),
        Kind::Arm,
        rings,
        &[("wrist", 2), ("elbow", 14), ("upper_arm", 25), ("top", 29)],
        &[("arm_top", 90.0, 30.0), ("arm_wrist", 90.0, 2.0)],
    )
}
```

- [ ] **Step 3: Run the tests to see them fail**

Run: `cargo nextest run -p opendrape-body form::file`
Expected: compile errors: `FormFile`, `Ring`, `Kind`, `ANGLES`, `FORMAT` not found.

- [ ] **Step 4: Implement the types and checks**

Put this above the test module in `crates/body/src/form/file.rs`:

```rust
//! The `.form.json` file format (format 1): one part of a dress form (a torso or a soft arm) as
//! horizontal rings, with its stations, landmarks, tape lines, measurement inputs and ranges.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::f64::consts::PI;

pub const FORMAT: u32 = 1;
/// Radii per ring, from centre front (0) to centre back (π) on the form's left (+x) side.
pub const ANGLES: usize = 49;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    Torso,
    Arm,
}

/// One horizontal ring: height and centre-line z in metres, radii in millimetres.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Ring {
    pub y: f64,
    pub zc: f64,
    pub r: Vec<f64>,
}

/// A tape line: a full ring at a station, or a smooth path through landmarks. An anchor named
/// `<landmark>_R` is that landmark's mirror image on the form's right side.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum TapeDef {
    Ring {
        ring: String,
    },
    Path {
        anchors: Vec<String>,
        #[serde(default)]
        closed: bool,
        /// Also draw the mirror image on the right side, named `<tape>_R`.
        #[serde(default)]
        mirror: bool,
    },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Collision {
    /// Gap kept between cloth and form, metres.
    pub thickness: f64,
    pub friction: f64,
}

/// Where a soft arm attaches: sunk `overlap` metres into the armhole plate at landmark `anchor`,
/// hanging `hang_deg` away from the body.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ArmAttach {
    pub form: String,
    pub anchor: String,
    pub hang_deg: f64,
    pub overlap: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FormFile {
    pub format: u32,
    pub id: String,
    pub kind: Kind,
    /// Fluent message id of the display name.
    pub name: String,
    /// Garment types this form is for.
    pub suits: Vec<String>,
    pub licence: String,
    pub base_size: String,
    pub angles: usize,
    /// Bottom to top.
    pub rings: Vec<Ring>,
    /// Station name → ring index.
    pub stations: BTreeMap<String, usize>,
    /// Left-side landmark → [angle from centre front (radians, 0..=π), v (0 bottom ring, 1 top)].
    pub landmarks: BTreeMap<String, [f64; 2]>,
    pub tapes: BTreeMap<String, TapeDef>,
    /// Measurements a user can set, in the order the app shows them.
    pub inputs: Vec<String>,
    /// Allowed range of each input, millimetres.
    pub ranges: BTreeMap<String, [f64; 2]>,
    pub collision: Collision,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub arm: Option<ArmAttach>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FormError(pub String);

impl std::fmt::Display for FormError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for FormError {}

/// Lengths measured along tape lines: (measurement, tape, from anchor, to anchor).
pub const TORSO_LENGTHS: [(&str, &str, &str, &str); 5] = [
    ("back_waist_length", "cb", "back_neck", "back_waist"),
    ("waist_to_hip", "side_seam", "side_waist", "side_hip"),
    ("shoulder_length", "shoulder_seam", "side_neck", "shoulder_point"),
    ("front_waist_length", "cf", "front_neck", "front_waist"),
    ("back_width", "back_width", "armhole_back_R", "armhole_back"),
];
pub const ARM_LENGTHS: [(&str, &str, &str, &str); 1] =
    [("arm_length", "outer", "arm_top", "arm_wrist")];
/// Lengths a user can set; the others are read-only measurements.
pub const ADJUSTABLE_LENGTHS: [&str; 4] =
    ["back_waist_length", "waist_to_hip", "shoulder_length", "arm_length"];
/// Stations the resizing needs (plus `bust` or `chest` on a torso).
pub const TORSO_STATIONS: [&str; 4] = ["waist", "hip", "shoulder", "neck"];
pub const ARM_STATIONS: [&str; 2] = ["top", "upper_arm"];

impl FormFile {
    pub fn from_json(s: &str) -> Result<Self, FormError> {
        let f: FormFile =
            serde_json::from_str(s).map_err(|e| FormError(format!("not a form file: {e}")))?;
        f.check()?;
        Ok(f)
    }

    pub fn lengths(&self) -> &'static [(&'static str, &'static str, &'static str, &'static str)] {
        match self.kind {
            Kind::Torso => &TORSO_LENGTHS,
            Kind::Arm => &ARM_LENGTHS,
        }
    }

    /// Where the shoulder widening starts: `bust` on women's forms, `chest` on men's.
    pub fn chest_station(&self) -> Option<&str> {
        ["bust", "chest"]
            .into_iter()
            .find(|s| self.stations.contains_key(*s))
    }

    pub fn check(&self) -> Result<(), FormError> {
        let bad = |m: String| Err(FormError(format!("form {}: {m}", self.id)));
        if self.format != FORMAT {
            return bad(format!("format {} is not {FORMAT}", self.format));
        }
        if self.angles != ANGLES {
            return bad(format!("{} angles, expected {ANGLES}", self.angles));
        }
        if self.rings.len() < 8 {
            return bad("fewer than 8 rings".into());
        }
        for (i, r) in self.rings.iter().enumerate() {
            let ok = r.r.len() == ANGLES
                && r.r.iter().all(|x| x.is_finite() && *x > 0.0)
                && r.y.is_finite()
                && r.zc.is_finite();
            if !ok {
                return bad(format!("ring {i} is malformed"));
            }
            if i > 0 && r.y <= self.rings[i - 1].y {
                return bad(format!("ring {i} is not above ring {}", i - 1));
            }
        }
        for (name, &i) in &self.stations {
            if i >= self.rings.len() {
                return bad(format!("station {name} has no ring {i}"));
            }
        }
        for (name, &[phi, v]) in &self.landmarks {
            if !(0.0..=PI).contains(&phi) || !(0.0..=1.0).contains(&v) {
                return bad(format!("landmark {name} is off the form"));
            }
        }
        for (name, tape) in &self.tapes {
            match tape {
                TapeDef::Ring { ring } => {
                    if !self.stations.contains_key(ring) {
                        return bad(format!("tape {name} needs station {ring}"));
                    }
                }
                TapeDef::Path {
                    anchors, closed, ..
                } => {
                    if anchors.len() < if *closed { 3 } else { 2 } {
                        return bad(format!("tape {name} is too short"));
                    }
                    let missing = anchors
                        .iter()
                        .find(|a| {
                            !self
                                .landmarks
                                .contains_key(a.strip_suffix("_R").unwrap_or(a.as_str()))
                        });
                    if let Some(a) = missing {
                        return bad(format!("tape {name} needs landmark {a}"));
                    }
                }
            }
        }
        let needed: &[&str] = match self.kind {
            Kind::Torso => &TORSO_STATIONS,
            Kind::Arm => &ARM_STATIONS,
        };
        if let Some(s) = needed.iter().find(|s| !self.stations.contains_key(**s)) {
            return bad(format!("needs station {s}"));
        }
        if self.kind == Kind::Torso && self.chest_station().is_none() {
            return bad("needs a bust or chest station".into());
        }
        for (m, tape, from, to) in self.lengths() {
            let Some(TapeDef::Path { anchors, .. }) = self.tapes.get(*tape) else {
                return bad(format!("{m} needs tape {tape}"));
            };
            if !anchors.iter().any(|a| a == from) || !anchors.iter().any(|a| a == to) {
                return bad(format!("{m}: tape {tape} must pass {from} and {to}"));
            }
        }
        for m in &self.inputs {
            let adjustable_length = ADJUSTABLE_LENGTHS.contains(&m.as_str())
                && self.lengths().iter().any(|l| l.0 == m);
            if !self.stations.contains_key(m) && !adjustable_length {
                return bad(format!(
                    "input {m} is neither a station girth nor an adjustable length"
                ));
            }
            match self.ranges.get(m) {
                Some(&[lo, hi]) if lo > 0.0 && lo < hi => {}
                _ => return bad(format!("input {m} has no valid range")),
            }
        }
        if let Some(arm) = &self.arm
            && !self.landmarks.contains_key(&arm.anchor)
        {
            return bad(format!("arm anchor {} is not a landmark", arm.anchor));
        }
        Ok(())
    }
}
```

- [ ] **Step 5: Run the tests to see them pass**

Run: `cargo nextest run -p opendrape-body form::file`
Expected: PASS, 4 tests.

- [ ] **Step 6: Commit**

```bash
cargo fmt --all
git add crates/body
git commit -m "feat(body): dress-form file format with reference checks and a test fixture

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 4: Rings: a closed, mirror-symmetric mesh and exact surface points

**Files:**
- Create: `crates/body/src/form/rings.rs`
- Modify: `crates/body/src/form/mod.rs` (add `mod rings; pub use rings::Rings;`), `crates/body/src/measure.rs`

**Interfaces:**
- Consumes: `FormFile` from Task 3.
- Produces:
  - `pub struct Rings { pub y: Vec<f64>, pub zc: Vec<f64>, pub r: Vec<Vec<f64>> }`: metres; `r[i]` holds the half-side radii from 0 to π.
  - Methods:
    - `Rings::from_file(&FormFile) -> Rings`;
    - `len() -> usize`, `half() -> usize` (radii per side), `around() -> usize` (= 2·(half − 1));
    - `vertex(i, j) -> DVec3` for ring `i` and full-circle sample `j` (wraps);
    - `mesh() -> BodyMesh`: the vertex index of `(i, j)` is `i·around + j`; then the bottom and top centre vertices; the triangles for quad `(i, j)` are at `2·(i·around + j)` and `+1`;
    - `point(phi, v) -> DVec3` and `normal(phi, v) -> DVec3`;
    - `girth(i) -> f64`, `y_at(v) -> f64`;
    - `with_half_angles(half) -> Rings`.
  - `crate::measure::hull_perimeter(Vec<DVec2>) -> f64` (`pub(crate)`).
  - `girth_at` now also counts vertices lying exactly on the slice plane.

- [ ] **Step 1: Write the failing tests**

Add to the `tests` module in `crates/body/src/measure.rs`:

```rust
    #[test]
    fn girth_counts_vertices_lying_on_the_slice_plane() {
        // y = 0.5 is the cube's top face: no edge crosses it, but its four corners lie on it.
        assert!((girth_at(&cube(), 0.5, 10.0) - 4.0).abs() < 1e-5);
    }
```

Create `crates/body/src/form/rings.rs` with only its tests for now:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::form::fixture;
    use crate::{boundary_edge_count, girth_at};
    use glam::Vec3;
    use std::f64::consts::{FRAC_PI_2, PI};

    fn rings() -> Rings {
        Rings::from_file(&fixture::torso())
    }

    fn signed_volume(m: &BodyMesh) -> f64 {
        m.triangles
            .iter()
            .map(|t| {
                let [a, b, c] = t.map(|i| m.positions[i as usize].as_dvec3());
                a.dot(b.cross(c)) / 6.0
            })
            .sum()
    }

    /// `p` lies on triangle (a, b, c), within 1 µm.
    fn on_triangle(p: DVec3, a: DVec3, b: DVec3, c: DVec3) -> bool {
        let n = (b - a).cross(c - a);
        let area = n.length_squared();
        let wa = (b - p).cross(c - p).dot(n) / area;
        let wb = (c - p).cross(a - p).dot(n) / area;
        let wc = 1.0 - wa - wb;
        (p - a).dot(n).abs() / n.length() < 1e-6 && wa > -1e-6 && wb > -1e-6 && wc > -1e-6
    }

    #[test]
    fn mesh_is_closed_outward_and_mirror_symmetric() {
        let r = rings();
        let mesh = r.mesh();
        assert_eq!(mesh.triangles.len(), 2 * 96 * r.len());
        assert_eq!(boundary_edge_count(&mesh), 0);
        assert!(signed_volume(&mesh) > 0.0, "triangles face outward");
        for i in 0..r.len() {
            for j in 1..96 {
                let (p, q) = (r.vertex(i, j), r.vertex(i, 96 - j));
                assert_eq!((p.x, p.y, p.z), (-q.x, q.y, q.z), "ring {i} sample {j}");
            }
        }
        // The triangulation is mirrored too, so drapes have no left/right bias.
        let key = |p: Vec3| ((p.x + 0.0).to_bits(), p.y.to_bits(), p.z.to_bits());
        let sorted = |mut k: [(u32, u32, u32); 3]| {
            k.sort();
            k
        };
        let tris: std::collections::HashSet<_> = mesh
            .triangles
            .iter()
            .map(|t| sorted(t.map(|i| key(mesh.positions[i as usize]))))
            .collect();
        for t in &mesh.triangles {
            let mirrored = t.map(|i| {
                let p = mesh.positions[i as usize];
                key(Vec3::new(-p.x, p.y, p.z))
            });
            assert!(tris.contains(&sorted(mirrored)), "triangle {t:?} has no mirror image");
        }
    }

    #[test]
    fn station_girth_equals_the_tape_measure_slice() {
        let r = rings();
        let mesh = r.mesh();
        for i in [12, 33, 51, 75] {
            let slice = f64::from(girth_at(&mesh, r.y[i] as f32, 10.0));
            assert!(
                (slice - r.girth(i)).abs() < 1e-4,
                "ring {i}: slice {slice} vs ring {}",
                r.girth(i)
            );
        }
    }

    #[test]
    fn surface_points_lie_on_the_mesh_triangles() {
        let r = rings();
        let mesh = r.mesh();
        let m = r.around();
        for step in 0..997 {
            let phi = (step as f64 * 0.0371) % TAU;
            let v = (step as f64 * 0.618_033_988_7) % 1.0;
            let p = r.point(phi, v);
            let i = ((v * (r.len() - 1) as f64).floor() as usize).min(r.len() - 2);
            let j = ((phi / TAU * m as f64).floor() as usize).min(m - 1);
            let quad = 2 * (i * m + j);
            let on = |t: usize| {
                let [a, b, c] = mesh.triangles[t].map(|k| mesh.positions[k as usize].as_dvec3());
                on_triangle(p, a, b, c)
            };
            assert!(on(quad) || on(quad + 1), "({phi}, {v}) is off its quad's triangles");
        }
    }

    #[test]
    fn grid_points_are_the_vertices_and_mirrors_are_exact() {
        let r = rings();
        assert!(r.point(TAU * 7.0 / 96.0, 33.0 / 80.0).distance(r.vertex(33, 7)) < 1e-12);
        for step in 0..500 {
            let (phi, v) = ((step as f64 * 0.0213) % PI, (step as f64 * 0.377) % 1.0);
            let (p, q) = (r.point(phi, v), r.point(TAU - phi, v));
            assert!(
                (p.x + q.x).abs() < 1e-9 && (p.y - q.y).abs() < 1e-9 && (p.z - q.z).abs() < 1e-9,
                "({phi}, {v}): {p} vs {q}"
            );
        }
    }

    #[test]
    fn normals_point_outward() {
        let r = rings();
        for (phi, v) in [(0.0, 0.4), (FRAC_PI_2, 0.5), (PI, 0.3), (4.0, 0.6)] {
            let p = r.point(phi, v);
            let out = DVec3::new(p.x, 0.0, p.z - r.zc[(v * 80.0) as usize]).normalize();
            assert!(r.normal(phi, v).dot(out) > 0.8, "({phi}, {v})");
        }
    }

    #[test]
    fn low_quality_keeps_the_shape_with_fewer_triangles() {
        let r = rings();
        let low = r.with_half_angles(33);
        assert_eq!(low.around(), 64);
        assert_eq!(low.mesh().triangles.len(), 2 * 64 * r.len());
        assert!((low.girth(33) - r.girth(33)).abs() < 0.002);
        assert_eq!(low.y_at(0.5), r.y_at(0.5));
    }
}
```

Add `mod rings;` and `pub use rings::Rings;` to `crates/body/src/form/mod.rs`.

- [ ] **Step 2: Run the tests to see them fail**

Run: `cargo nextest run -p opendrape-body measure:: form::rings`
Expected: compile errors in `rings.rs` (`Rings` not found). Once it compiles, `girth_counts_vertices_lying_on_the_slice_plane` fails with a girth of 0.

- [ ] **Step 3: Implement**

In `crates/body/src/measure.rs`, change `use glam::Vec2;` to `use glam::DVec2;`. Replace `girth_at` and `hull_perimeter` with:

```rust
/// Tape-measure girth at height `y`: perimeter of the convex hull of the mesh's
/// cross-section, keeping only points with |x| < `max_abs_x` (to leave out the arms).
/// Vertices lying exactly on the plane count (a dress form has a ring at every station).
pub fn girth_at(mesh: &BodyMesh, y: f32, max_abs_x: f32) -> f32 {
    let mut pts = vec![];
    for t in &mesh.triangles {
        for k in 0..3 {
            let (a, b) = (
                mesh.positions[t[k] as usize],
                mesh.positions[t[(k + 1) % 3] as usize],
            );
            if a.y == y && a.x.abs() < max_abs_x {
                pts.push(DVec2::new(f64::from(a.x), f64::from(a.z)));
            }
            if (a.y - y) * (b.y - y) < 0.0 {
                let p = a + (b - a) * ((y - a.y) / (b.y - a.y));
                if p.x.abs() < max_abs_x {
                    pts.push(DVec2::new(f64::from(p.x), f64::from(p.z)));
                }
            }
        }
    }
    hull_perimeter(pts) as f32
}

/// Andrew's monotone chain convex hull, returning its perimeter.
pub(crate) fn hull_perimeter(mut pts: Vec<DVec2>) -> f64 {
    pts.sort_by(|a, b| a.x.total_cmp(&b.x).then(a.y.total_cmp(&b.y)));
    pts.dedup();
    if pts.len() < 3 {
        return 0.0;
    }
    fn half(points: impl Iterator<Item = DVec2>) -> Vec<DVec2> {
        let mut h: Vec<DVec2> = vec![];
        for p in points {
            while h.len() >= 2
                && (h[h.len() - 1] - h[h.len() - 2]).perp_dot(p - h[h.len() - 2]) <= 0.0
            {
                h.pop();
            }
            h.push(p);
        }
        h.pop();
        h
    }
    let mut hull = half(pts.iter().copied());
    hull.extend(half(pts.iter().rev().copied()));
    (0..hull.len())
        .map(|i| hull[i].distance(hull[(i + 1) % hull.len()]))
        .sum()
}
```

Put this above the tests in `crates/body/src/form/rings.rs`:

```rust
//! A form's surface as horizontal rings, and the closed triangle mesh built from them.

use super::file::FormFile;
use crate::BodyMesh;
use glam::{DVec2, DVec3};
use std::f64::consts::TAU;

/// Ring `i` is at height `y[i]`, centred on (0, `zc[i]`), with radius `r[i][k]` (metres) at angle
/// k·π/(half − 1) from centre front (+z) towards the form's left (+x). Angles past π mirror onto
/// −x, so the surface is exactly symmetric.
#[derive(Clone, Debug, PartialEq)]
pub struct Rings {
    pub y: Vec<f64>,
    pub zc: Vec<f64>,
    pub r: Vec<Vec<f64>>,
}

impl Rings {
    pub fn from_file(f: &FormFile) -> Self {
        Self {
            y: f.rings.iter().map(|r| r.y).collect(),
            zc: f.rings.iter().map(|r| r.zc).collect(),
            r: f
                .rings
                .iter()
                .map(|r| r.r.iter().map(|mm| mm / 1000.0).collect())
                .collect(),
        }
    }

    pub fn len(&self) -> usize {
        self.y.len()
    }

    pub fn is_empty(&self) -> bool {
        self.y.is_empty()
    }

    /// Radii per side, centre front to centre back inclusive.
    pub fn half(&self) -> usize {
        self.r[0].len()
    }

    /// Samples around the full ring.
    pub fn around(&self) -> usize {
        2 * (self.half() - 1)
    }

    /// Ring `i`, sample `j` of `around()` (wraps), counted from centre front towards +x.
    pub fn vertex(&self, i: usize, j: usize) -> DVec3 {
        let (k, m) = (self.half() - 1, self.around());
        let j = j % m;
        if j > k {
            let p = self.vertex(i, m - j);
            return DVec3::new(-p.x, p.y, p.z);
        }
        let a = TAU * j as f64 / m as f64;
        let r = self.r[i][j];
        let x = if j == k { 0.0 } else { r * a.sin() };
        DVec3::new(x, self.y[i], self.zc[i] + r * a.cos())
    }

    /// Closed mesh, counter-clockwise seen from outside. Ring vertices first (`i·around + j`),
    /// then the bottom and top centre. Quads on the left side are split along one diagonal and
    /// their mirror images on the right along the other, so the mesh is exactly symmetric.
    pub fn mesh(&self) -> BodyMesh {
        let (n, m) = (self.len(), self.around());
        let mut positions: Vec<glam::Vec3> = (0..n)
            .flat_map(|i| (0..m).map(move |j| self.vertex(i, j).as_vec3()))
            .collect();
        let bottom = positions.len() as u32;
        positions.push(DVec3::new(0.0, self.y[0], self.zc[0]).as_vec3());
        positions.push(DVec3::new(0.0, self.y[n - 1], self.zc[n - 1]).as_vec3());
        let top = bottom + 1;
        let id = |i: usize, j: usize| (i * m + j % m) as u32;
        let mut triangles = Vec::with_capacity(2 * m * n);
        for i in 0..n - 1 {
            for j in 0..m {
                let (a, b, c, d) = (id(i, j), id(i, j + 1), id(i + 1, j + 1), id(i + 1, j));
                if j < m / 2 {
                    triangles.extend([[a, b, c], [a, c, d]]);
                } else {
                    triangles.extend([[a, b, d], [b, c, d]]);
                }
            }
        }
        for j in 0..m {
            triangles.push([bottom, id(0, j + 1), id(0, j)]);
            triangles.push([top, id(n - 1, j), id(n - 1, j + 1)]);
        }
        BodyMesh {
            positions,
            triangles,
        }
    }

    /// The surface point at angle `phi` (radians from centre front towards +x; any value wraps)
    /// and height parameter `v` (0 bottom ring, 1 top ring), on the same triangles as `mesh`.
    pub fn point(&self, phi: f64, v: f64) -> DVec3 {
        let (n, m) = (self.len(), self.around());
        let f = v.clamp(0.0, 1.0) * (n - 1) as f64;
        let i = (f.floor() as usize).min(n - 2);
        let t = f - i as f64;
        let g = phi.rem_euclid(TAU) / TAU * m as f64;
        let j = (g.floor() as usize).min(m - 1);
        let s = g - j as f64;
        let (a, b, c, d) = (
            self.vertex(i, j),
            self.vertex(i, (j + 1) % m),
            self.vertex(i + 1, (j + 1) % m),
            self.vertex(i + 1, j),
        );
        if j < m / 2 {
            if s >= t {
                a * (1.0 - s) + b * (s - t) + c * t
            } else {
                a * (1.0 - t) + c * s + d * (t - s)
            }
        } else if s + t <= 1.0 {
            a * (1.0 - s - t) + b * s + d * t
        } else {
            b * (1.0 - t) + c * (s + t - 1.0) + d * (1.0 - s)
        }
    }

    /// Outward unit normal at (`phi`, `v`), by central differences on the surface.
    pub fn normal(&self, phi: f64, v: f64) -> DVec3 {
        let (dphi, dv) = (1e-3, 1e-3);
        let along = self.point(phi + dphi, v) - self.point(phi - dphi, v);
        let up = self.point(phi, (v + dv).min(1.0)) - self.point(phi, (v - dv).max(0.0));
        along.cross(up).normalize_or_zero()
    }

    /// What a tape measure reads around ring `i`: the convex-hull perimeter, metres.
    pub fn girth(&self, i: usize) -> f64 {
        crate::measure::hull_perimeter(
            (0..self.around())
                .map(|j| {
                    let p = self.vertex(i, j);
                    DVec2::new(p.x, p.z)
                })
                .collect(),
        )
    }

    /// Height at fractional ring parameter `v`.
    pub fn y_at(&self, v: f64) -> f64 {
        let f = v.clamp(0.0, 1.0) * (self.len() - 1) as f64;
        let i = (f.floor() as usize).min(self.len() - 2);
        self.y[i] + (self.y[i + 1] - self.y[i]) * (f - i as f64)
    }

    /// The same surface with `half` radii per side, interpolated in angle.
    pub fn with_half_angles(&self, half: usize) -> Rings {
        let last = (self.half() - 1) as f64;
        let r = self
            .r
            .iter()
            .map(|ring| {
                (0..half)
                    .map(|k| {
                        let s = k as f64 * last / (half - 1) as f64;
                        let i = (s.floor() as usize).min(self.half() - 2);
                        let t = s - i as f64;
                        ring[i] * (1.0 - t) + ring[i + 1] * t
                    })
                    .collect()
            })
            .collect();
        Rings {
            y: self.y.clone(),
            zc: self.zc.clone(),
            r,
        }
    }
}
```

- [ ] **Step 4: Run the tests to see them pass**

Run: `cargo nextest run -p opendrape-body`
Expected: PASS. That includes the 6 new rings tests, the new measure test, and the unchanged MakeHuman asset tests: the waist and hip girths stay within their ranges.

- [ ] **Step 5: Commit**

```bash
cargo fmt --all
git add crates/body
git commit -m "feat(body): rings build a closed, mirror-symmetric form mesh with exact surface points

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 5: Tape lines: smooth paths through landmarks, lengths, ribbons

**Files:**
- Create: `crates/body/src/form/tape.rs`
- Modify: `crates/body/src/form/mod.rs` (add `pub mod tape;`)

**Interfaces:**
- Consumes: `FormFile`, `TapeDef` and `Rings` (`point`, `normal`, `vertex`, `around`).
- Produces:
  - `pub struct Tape { pub name: String, pub closed: bool, pub uv: Vec<DVec2>, pub points: Vec<DVec3>, pub anchors: Vec<(String, usize)> }`;
  - `tape::landmark_uv(&FormFile, &str) -> Option<DVec2>` (`_R` mirrors);
  - `tape::tape(&FormFile, &Rings, name) -> Option<Tape>` (`<tape>_R` gives the mirror copy of a mirrored tape);
  - `tape::tapes(&FormFile, &Rings) -> Vec<Tape>` (all tapes, mirrors included);
  - `Tape::length() -> f64` and `Tape::length_between(from, to) -> Option<f64>`;
  - `tape::ribbons(&[Tape], &Rings) -> BodyMesh`;
  - constants `RIBBON_HALF_WIDTH = 0.003`, `RIBBON_LIFT = 0.0005`.

- [ ] **Step 1: Write the failing tests**

Create `crates/body/src/form/tape.rs` with only its tests:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::form::fixture;

    fn setup() -> (FormFile, Rings) {
        let f = fixture::torso();
        let r = Rings::from_file(&f);
        (f, r)
    }

    #[test]
    fn every_tape_and_its_mirror_is_built() {
        let (f, r) = setup();
        let names: Vec<String> = tapes(&f, &r).into_iter().map(|t| t.name).collect();
        for n in [
            "cf", "cb", "neckline", "shoulder_seam", "shoulder_seam_R", "armhole", "armhole_R",
            "side_seam", "side_seam_R", "princess_front", "princess_front_R", "princess_back",
            "princess_back_R", "back_width", "bust", "under_bust", "waist", "high_hip", "hip",
        ] {
            assert!(names.iter().any(|x| x == n), "missing {n}");
        }
        assert!(tape(&f, &r, "cf_R").is_none(), "cf is not mirrored");
    }

    #[test]
    fn centre_front_runs_down_x_zero() {
        let (f, r) = setup();
        let cf = tape(&f, &r, "cf").unwrap();
        assert!(cf.points.iter().all(|p| p.x == 0.0));
        let l = cf.length_between("front_neck", "front_waist").unwrap();
        assert!((0.38..0.46).contains(&l), "{l}");
        assert_eq!(cf.length_between("front_waist", "front_neck"), Some(l));
        assert_eq!(cf.length_between("front_neck", "nowhere"), None);
    }

    #[test]
    fn ring_tape_is_the_station_ring() {
        let (f, r) = setup();
        let waist = tape(&f, &r, "waist").unwrap();
        assert!(waist.closed && waist.points.len() == r.around());
        assert!(
            (waist.length() - r.girth(33)).abs() < 1e-9,
            "a convex ring's perimeter is its hull's"
        );
    }

    #[test]
    fn mirrored_tapes_are_mirror_images() {
        let (f, r) = setup();
        let p = tape(&f, &r, "princess_front").unwrap();
        let q = tape(&f, &r, "princess_front_R").unwrap();
        assert_eq!(p.points.len(), q.points.len());
        for (a, b) in p.points.iter().zip(&q.points) {
            assert!((a.x + b.x).abs() < 1e-9 && (a.y - b.y).abs() < 1e-9 && (a.z - b.z).abs() < 1e-9);
        }
    }

    #[test]
    fn closed_loops_cross_centre_front_without_a_jump() {
        let (f, r) = setup();
        let neck = tape(&f, &r, "neckline").unwrap();
        let n = neck.points.len();
        let longest = (0..n)
            .map(|k| neck.points[k].distance(neck.points[(k + 1) % n]))
            .fold(0.0, f64::max);
        assert!(longest < 0.02, "a {longest} m jump");
        assert!((0.2..0.6).contains(&neck.length()), "{}", neck.length());
    }

    #[test]
    fn ribbons_sit_just_outside_the_surface() {
        let (f, r) = setup();
        let t = vec![tape(&f, &r, "side_seam").unwrap()];
        let mesh = ribbons(&t, &r);
        assert_eq!(mesh.positions.len(), 2 * t[0].points.len());
        assert_eq!(mesh.triangles.len(), 2 * (t[0].points.len() - 1));
        for (k, p) in t[0].points.iter().enumerate() {
            let (a, b) = (
                mesh.positions[2 * k].as_dvec3(),
                mesh.positions[2 * k + 1].as_dvec3(),
            );
            assert!((a.distance(b) - 2.0 * RIBBON_HALF_WIDTH).abs() < 1e-5);
            let mid = (a + b) / 2.0;
            assert!((mid.distance(*p) - RIBBON_LIFT).abs() < 1e-5);
            assert!((mid - *p).dot(r.normal(t[0].uv[k].x, t[0].uv[k].y)) > 0.0, "lifted outward");
        }
    }
}
```

Add `pub mod tape;` to `crates/body/src/form/mod.rs`.

- [ ] **Step 2: Run the tests to see them fail**

Run: `cargo nextest run -p opendrape-body form::tape`
Expected: compile errors: `Tape`, `tape`, `tapes`, `ribbons` not found.

- [ ] **Step 3: Implement**

Put this above the tests in `crates/body/src/form/tape.rs`:

```rust
//! Tape lines on a form: resolved from landmarks, sampled on the surface as smooth curves,
//! measured, and turned into thin ribbons for drawing.

use super::file::{FormFile, TapeDef};
use super::rings::Rings;
use crate::BodyMesh;
use glam::{DVec2, DVec3};
use std::f64::consts::{PI, TAU};

/// Samples per span between consecutive anchors.
const SPAN: usize = 24;
/// Ribbons are 6 mm wide, lifted 0.5 mm off the surface so they never flicker into it.
pub const RIBBON_HALF_WIDTH: f64 = 0.003;
pub const RIBBON_LIFT: f64 = 0.0005;

/// A tape line sampled on a sized form.
#[derive(Clone, Debug, PartialEq)]
pub struct Tape {
    pub name: String,
    pub closed: bool,
    /// (angle, v) of each sample.
    pub uv: Vec<DVec2>,
    pub points: Vec<DVec3>,
    /// Sample index of each anchor.
    pub anchors: Vec<(String, usize)>,
}

impl Tape {
    /// Total length, including the closing segment of a loop.
    pub fn length(&self) -> f64 {
        let n = self.points.len();
        let open: f64 = self.points.windows(2).map(|w| w[0].distance(w[1])).sum();
        if self.closed && n > 1 {
            open + self.points[n - 1].distance(self.points[0])
        } else {
            open
        }
    }

    /// Surface length between two anchors along the tape (either order).
    pub fn length_between(&self, from: &str, to: &str) -> Option<f64> {
        let at = |name: &str| self.anchors.iter().find(|(a, _)| a == name).map(|(_, i)| *i);
        let (a, b) = (at(from)?, at(to)?);
        let (a, b) = (a.min(b), a.max(b));
        Some(self.points[a..=b].windows(2).map(|w| w[0].distance(w[1])).sum())
    }
}

/// (angle, v) of a landmark; `<name>_R` is its mirror image on the right side.
pub fn landmark_uv(file: &FormFile, name: &str) -> Option<DVec2> {
    let (base, right) = match name.strip_suffix("_R") {
        Some(b) => (b, true),
        None => (name, false),
    };
    let [phi, v] = *file.landmarks.get(base)?;
    Some(DVec2::new(if right { TAU - phi } else { phi }, v))
}

fn mirror_name(name: &str) -> String {
    match name.strip_suffix("_R") {
        Some(b) => b.to_string(),
        None => format!("{name}_R"),
    }
}

/// One tape by name; `<tape>_R` is the mirror copy of a tape marked `mirror`.
pub fn tape(file: &FormFile, rings: &Rings, name: &str) -> Option<Tape> {
    if let Some(def) = file.tapes.get(name) {
        return Some(match def {
            TapeDef::Ring { ring } => ring_tape(name, file.stations[ring], rings),
            TapeDef::Path {
                anchors, closed, ..
            } => path_tape(name, file, anchors, *closed, rings),
        });
    }
    let base = name.strip_suffix("_R")?;
    match file.tapes.get(base)? {
        TapeDef::Path {
            anchors,
            closed,
            mirror: true,
        } => {
            let mirrored: Vec<String> = anchors.iter().map(|a| mirror_name(a)).collect();
            Some(path_tape(name, file, &mirrored, *closed, rings))
        }
        _ => None,
    }
}

/// Every tape of the form, mirror copies included.
pub fn tapes(file: &FormFile, rings: &Rings) -> Vec<Tape> {
    let mut out = vec![];
    for (name, def) in &file.tapes {
        out.extend(tape(file, rings, name));
        if let TapeDef::Path { mirror: true, .. } = def {
            out.extend(tape(file, rings, &format!("{name}_R")));
        }
    }
    out
}

fn ring_tape(name: &str, i: usize, rings: &Rings) -> Tape {
    let m = rings.around();
    let v = i as f64 / (rings.len() - 1) as f64;
    Tape {
        name: name.to_string(),
        closed: true,
        uv: (0..m)
            .map(|j| DVec2::new(TAU * j as f64 / m as f64, v))
            .collect(),
        points: (0..m).map(|j| rings.vertex(i, j)).collect(),
        anchors: vec![],
    }
}

/// Uniform Catmull-Rom through the anchors in (angle, v), sampled onto the surface.
fn path_tape(name: &str, file: &FormFile, anchors: &[String], closed: bool, rings: &Rings) -> Tape {
    let mut ctrl: Vec<DVec2> = anchors
        .iter()
        .map(|a| landmark_uv(file, a).expect("FormFile::check found every anchor"))
        .collect();
    // Unwrap angles so each step goes the short way round.
    for k in 1..ctrl.len() {
        while ctrl[k].x - ctrl[k - 1].x > PI {
            ctrl[k].x -= TAU;
        }
        while ctrl[k].x - ctrl[k - 1].x < -PI {
            ctrl[k].x += TAU;
        }
    }
    let n = ctrl.len();
    // A loop closes onto its first anchor shifted by whole turns, the short way round.
    let turn = DVec2::new(TAU * ((ctrl[n - 1].x - ctrl[0].x) / TAU).round(), 0.0);
    let get = |k: isize| -> DVec2 {
        if closed {
            let wraps = k.div_euclid(n as isize) as f64;
            ctrl[k.rem_euclid(n as isize) as usize] + turn * wraps
        } else {
            ctrl[k.clamp(0, n as isize - 1) as usize]
        }
    };
    let spans = if closed { n } else { n - 1 };
    let mut uv = Vec::with_capacity(spans * SPAN + 1);
    for s in 0..spans as isize {
        let (p0, p1, p2, p3) = (get(s - 1), get(s), get(s + 1), get(s + 2));
        for q in 0..SPAN {
            let t = q as f64 / SPAN as f64;
            let (t2, t3) = (t * t, t * t * t);
            uv.push(
                0.5 * (2.0 * p1
                    + (p2 - p0) * t
                    + (2.0 * p0 - 5.0 * p1 + 4.0 * p2 - p3) * t2
                    + (3.0 * p1 - p0 - 3.0 * p2 + p3) * t3),
            );
        }
    }
    if !closed {
        uv.push(ctrl[n - 1]);
    }
    Tape {
        name: name.to_string(),
        closed,
        points: uv.iter().map(|p| rings.point(p.x, p.y)).collect(),
        uv,
        anchors: anchors
            .iter()
            .enumerate()
            .map(|(k, a)| (a.clone(), k * SPAN))
            .collect(),
    }
}

/// Thin ribbons along every tape, lifted off the surface, for drawing only (open, not collided).
pub fn ribbons(tapes: &[Tape], rings: &Rings) -> BodyMesh {
    let mut mesh = BodyMesh {
        positions: vec![],
        triangles: vec![],
    };
    for t in tapes {
        let n = t.points.len();
        let base = mesh.positions.len() as u32;
        for k in 0..n {
            let prev = if k > 0 {
                k - 1
            } else if t.closed {
                n - 1
            } else {
                0
            };
            let next = if k + 1 < n {
                k + 1
            } else if t.closed {
                0
            } else {
                n - 1
            };
            let tangent = (t.points[next] - t.points[prev]).normalize_or_zero();
            let normal = rings.normal(t.uv[k].x, t.uv[k].y);
            let side = normal.cross(tangent).normalize_or_zero() * RIBBON_HALF_WIDTH;
            let p = t.points[k] + normal * RIBBON_LIFT;
            mesh.positions.push((p - side).as_vec3());
            mesh.positions.push((p + side).as_vec3());
        }
        let segments = if t.closed { n } else { n - 1 };
        for k in 0..segments {
            let (a, b) = (base + 2 * k as u32, base + 2 * ((k + 1) % n) as u32);
            mesh.triangles.push([a, b, a + 1]);
            mesh.triangles.push([a + 1, b, b + 1]);
        }
    }
    mesh
}
```

- [ ] **Step 4: Run the tests to see them pass**

Run: `cargo nextest run -p opendrape-body form::tape`
Expected: PASS, 6 tests.

- [ ] **Step 5: Commit**

```bash
cargo fmt --all
git add crates/body
git commit -m "feat(body): tape lines through form landmarks, with lengths and ribbons

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 6: Resize a form to target measurements

**Files:**
- Create: `crates/body/src/form/resize.rs`
- Modify: `crates/body/src/form/mod.rs` (add `pub mod resize;`, `pub type Measurements`, `SizeError`)

**Interfaces:**
- Consumes: `FormFile` and its constants; `Rings`; `tape::{tape, landmark_uv}`.
- Produces:
  - `pub type Measurements = std::collections::BTreeMap<String, f64>` (millimetres);
  - `pub struct SizeError { pub measurement: String, pub min_mm: f64, pub max_mm: f64 }` (`Display` and `Error`): the measurement that cannot be met, and the range it could take;
  - `resize::resize(&FormFile, base: &Rings, &Measurements) -> Result<Rings, SizeError>` (a public module, so it never warns as unused before Task 7 calls it);
  - `resize::length(&FormFile, &Rings, measurement) -> f64` (metres);
  - `pub const MAX_RATIO: f64 = 1.33`.

- [ ] **Step 1: Write the failing tests**

Add to `crates/body/src/form/mod.rs`:

```rust
pub mod resize;

/// Measurements in millimetres, by name (`bust`, `waist`, `back_waist_length`, …).
pub type Measurements = std::collections::BTreeMap<String, f64>;

/// A size the form cannot take: which measurement, and the range it may have here.
#[derive(Clone, Debug, PartialEq)]
pub struct SizeError {
    pub measurement: String,
    pub min_mm: f64,
    pub max_mm: f64,
}

impl std::fmt::Display for SizeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{} must be {:.0}–{:.0} mm on this form",
            self.measurement, self.min_mm, self.max_mm
        )
    }
}

impl std::error::Error for SizeError {}
```

Create `crates/body/src/form/resize.rs` with only its tests:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::form::{ANGLES, fixture};
    use crate::girth_at;

    fn base() -> (FormFile, Rings) {
        let f = fixture::torso();
        let r = Rings::from_file(&f);
        (f, r)
    }

    /// The form's own value for each input, mm.
    fn measured(f: &FormFile, r: &Rings) -> Measurements {
        f.inputs
            .iter()
            .map(|m| {
                let metres = match f.stations.get(m) {
                    Some(&i) => r.girth(i),
                    None => length(f, r, m),
                };
                (m.clone(), metres * 1000.0)
            })
            .collect()
    }

    /// Girths (sliced from the mesh, like a tape measure) within 1 mm; lengths within 2 mm.
    fn assert_fits(f: &FormFile, r: &Rings, target: &Measurements) {
        let mesh = r.mesh();
        for m in &f.inputs {
            let (got, tol) = match f.stations.get(m) {
                Some(&i) => (1000.0 * f64::from(girth_at(&mesh, r.y[i] as f32, 10.0)), 1.0),
                None => (1000.0 * length(f, r, m), 2.0),
            };
            assert!((got - target[m]).abs() <= tol, "{m}: {got:.2} mm, wanted {:.2}", target[m]);
        }
    }

    fn scaled(m: &Measurements, by: &[(&str, f64)]) -> Measurements {
        let mut out = m.clone();
        for (name, k) in by {
            *out.get_mut(*name).expect("known input") *= k;
        }
        out
    }

    #[test]
    fn its_own_measurements_give_back_the_same_shape() {
        let (f, r) = base();
        let out = resize(&f, &r, &measured(&f, &r)).unwrap();
        for i in 0..r.len() {
            assert!((out.y[i] - r.y[i]).abs() < 1e-6, "ring {i} moved");
            for k in 0..ANGLES {
                assert!((out.r[i][k] - r.r[i][k]).abs() < 1e-6, "ring {i} radius {k} changed");
            }
        }
    }

    #[test]
    fn larger_and_smaller_targets_are_hit() {
        let (f, r) = base();
        let m = measured(&f, &r);
        for k in [1.08, 0.93] {
            let t = scaled(
                &m,
                &[
                    ("bust", k),
                    ("under_bust", k),
                    ("waist", k * k),
                    ("hip", k),
                    ("neck", k),
                    ("shoulder_length", k),
                    ("back_waist_length", k.sqrt()),
                    ("waist_to_hip", k),
                ],
            );
            assert_fits(&f, &resize(&f, &r, &t).unwrap(), &t);
        }
    }

    #[test]
    fn out_of_range_and_missing_inputs_are_refused_with_the_range() {
        let (f, r) = base();
        let mut t = measured(&f, &r);
        t.insert("waist".into(), 400.0);
        assert_eq!(
            resize(&f, &r, &t).unwrap_err(),
            SizeError {
                measurement: "waist".into(),
                min_mm: 500.0,
                max_mm: 1300.0
            }
        );
        let mut t = measured(&f, &r);
        t.remove("hip");
        assert_eq!(resize(&f, &r, &t).unwrap_err().measurement, "hip");
    }

    #[test]
    fn non_finite_input_is_refused() {
        let (f, r) = base();
        for bad in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            let mut t = measured(&f, &r);
            t.insert("bust".into(), bad);
            assert_eq!(resize(&f, &r, &t).unwrap_err().measurement, "bust");
        }
    }

    #[test]
    fn neighbours_that_fight_are_refused_naming_the_odd_one_out() {
        let (f, r) = base();
        let m = measured(&f, &r);
        let e = resize(&f, &r, &scaled(&m, &[("waist", 1.6)])).unwrap_err();
        assert_eq!(e.measurement, "waist");
        assert!(e.max_mm <= m["waist"] * 1.34 && e.min_mm < e.max_mm, "{e:?}");
        let e = resize(&f, &r, &scaled(&m, &[("hip", 1.5)])).unwrap_err();
        assert_eq!(e.measurement, "hip");
    }

    #[test]
    fn any_targets_in_range_give_a_valid_shape_or_a_refusal() {
        let (f, r) = base();
        let mut seed = 0x2545_f491_4f6c_dd1d_u64;
        let mut next = move || {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            (seed >> 11) as f64 / (1u64 << 53) as f64
        };
        for _ in 0..200 {
            let t: Measurements = f
                .inputs
                .iter()
                .map(|m| {
                    let [lo, hi] = f.ranges[m];
                    (m.clone(), lo + (hi - lo) * next())
                })
                .collect();
            match resize(&f, &r, &t) {
                Err(e) => assert!(f.inputs.contains(&e.measurement), "{e:?}"),
                Ok(out) => {
                    assert!(out.y.windows(2).all(|w| w[1] > w[0]), "rings stay in order");
                    assert!(out.r.iter().flatten().all(|x| x.is_finite() && *x > 0.0));
                    assert_fits(&f, &out, &t);
                }
            }
        }
    }

    #[test]
    fn resizing_is_deterministic_keeps_landmark_order_and_the_waist() {
        let (f, r) = base();
        let t = scaled(
            &measured(&f, &r),
            &[("bust", 1.1), ("under_bust", 1.1), ("waist", 1.1), ("hip", 1.1), ("back_waist_length", 1.05), ("waist_to_hip", 0.95)],
        );
        let (a, b) = (resize(&f, &r, &t).unwrap(), resize(&f, &r, &t).unwrap());
        assert_eq!(a, b);
        let y = |l: &str| a.y_at(f.landmarks[l][1]);
        assert!(y("back_neck") > y("bust_apex") && y("bust_apex") > y("front_waist"));
        assert!(y("front_waist") > y("side_hip"));
        assert_eq!(a.y[33], r.y[33], "the waist stays at its height");
    }

    #[test]
    fn arm_hits_its_girth_and_length() {
        let f = fixture::arm();
        let r = Rings::from_file(&f);
        let t: Measurements = [
            ("upper_arm".to_string(), 320.0),
            ("arm_length".to_string(), 600.0),
        ]
        .into();
        assert_fits(&f, &resize(&f, &r, &t).unwrap(), &t);
    }
}
```

- [ ] **Step 2: Run the tests to see them fail**

Run: `cargo nextest run -p opendrape-body form::resize`
Expected: compile errors: `resize`, `length` not found.

- [ ] **Step 3: Implement**

Put this above the tests in `crates/body/src/form/resize.rs`:

```rust
//! Resizes a form's rings to target measurements in three moves:
//! 1. vertical stretches for lengths (torso: above and below the waist; arm: below its top);
//! 2. a sideways widening of the shoulders for the shoulder length;
//! 3. an exact uniform scale of every girth station about its centre, blended between
//!    stations by a monotone cubic so the surface stays smooth.
//!
//! The moves interact a little, so lengths are solved again a few times. Girths come out exact.

use super::file::{FormFile, Kind};
use super::rings::Rings;
use super::tape;
use super::{Measurements, SizeError};
use std::f64::consts::PI;

/// The largest stretch or widening, and the largest change of girth scale between neighbouring
/// stations; its inverse is the smallest. Keeps every shape plausible.
pub const MAX_RATIO: f64 = 1.33;
/// Lengths are solved to 0.05 mm and accepted within 2 mm.
const SOLVE_TOL: f64 = 0.000_05;
const ACCEPT_TOL: f64 = 0.002;
const PASSES: usize = 6;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Knob {
    Above,
    Below,
    Shoulder,
}

fn knob(measurement: &str) -> Knob {
    match measurement {
        "back_waist_length" => Knob::Above,
        "waist_to_hip" | "arm_length" => Knob::Below,
        "shoulder_length" => Knob::Shoulder,
        other => unreachable!("FormFile::check allows no other adjustable length: {other}"),
    }
}

#[derive(Clone, Debug)]
struct Params {
    above: f64,
    below: f64,
    shoulder: f64,
    /// (station ring index, scale), sorted by ring.
    girth: Vec<(usize, f64)>,
}

impl Params {
    fn value(&self, k: Knob) -> f64 {
        match k {
            Knob::Above => self.above,
            Knob::Below => self.below,
            Knob::Shoulder => self.shoulder,
        }
    }
    fn slot(&mut self, k: Knob) -> &mut f64 {
        match k {
            Knob::Above => &mut self.above,
            Knob::Below => &mut self.below,
            Knob::Shoulder => &mut self.shoulder,
        }
    }
}

/// A length measurement (adjustable or read-only) on `rings`, metres.
pub fn length(file: &FormFile, rings: &Rings, measurement: &str) -> f64 {
    let &(_, tape_name, from, to) = file
        .lengths()
        .iter()
        .find(|l| l.0 == measurement)
        .expect("a known length");
    tape::tape(file, rings, tape_name)
        .and_then(|t| t.length_between(from, to))
        .expect("FormFile::check found the tape and its anchors")
}

/// Monotone cubic through (xs, ys) (Fritsch–Carlson), flat beyond the ends.
fn pchip(xs: Vec<f64>, ys: Vec<f64>) -> impl Fn(f64) -> f64 {
    let n = xs.len();
    let h: Vec<f64> = xs.windows(2).map(|w| w[1] - w[0]).collect();
    let d: Vec<f64> = (0..n.saturating_sub(1))
        .map(|i| (ys[i + 1] - ys[i]) / h[i])
        .collect();
    let mut m = vec![0.0; n];
    if n > 1 {
        m[0] = d[0];
        m[n - 1] = d[n - 2];
    }
    for i in 1..n.saturating_sub(1) {
        if d[i - 1] * d[i] > 0.0 {
            let (w1, w2) = (2.0 * h[i] + h[i - 1], h[i] + 2.0 * h[i - 1]);
            m[i] = (w1 + w2) / (w1 / d[i - 1] + w2 / d[i]);
        }
    }
    move |x: f64| {
        if n == 1 || x <= xs[0] {
            return ys[0];
        }
        if x >= xs[n - 1] {
            return ys[n - 1];
        }
        let i = xs.partition_point(|&a| a <= x) - 1;
        let t = (x - xs[i]) / h[i];
        let (t2, t3) = (t * t, t * t * t);
        (2.0 * t3 - 3.0 * t2 + 1.0) * ys[i]
            + (t3 - 2.0 * t2 + t) * h[i] * m[i]
            + (3.0 * t2 - 2.0 * t3) * ys[i + 1]
            + (t3 - t2) * h[i] * m[i + 1]
    }
}

fn apply(file: &FormFile, base: &Rings, p: &Params) -> Rings {
    let mut out = base.clone();
    match file.kind {
        Kind::Torso => {
            let waist = base.y[file.stations["waist"]];
            let neck = base.y_at(file.landmarks["back_neck"][1]);
            for (y, &y0) in out.y.iter_mut().zip(&base.y) {
                *y = if y0 < waist {
                    waist + p.below * (y0 - waist)
                } else if y0 <= neck {
                    waist + p.above * (y0 - waist)
                } else {
                    waist + p.above * (neck - waist) + (y0 - neck)
                };
            }
            // Widen the sides, most at the shoulder line, fading to nothing at bust and neck.
            let chest = file.stations[file.chest_station().expect("checked")];
            let (shoulder, neck_ring) = (file.stations["shoulder"], file.stations["neck"]);
            let half = out.half();
            for i in chest..=neck_ring {
                let t = if i <= shoulder {
                    (i - chest) as f64 / (shoulder - chest).max(1) as f64
                } else {
                    (neck_ring - i) as f64 / (neck_ring - shoulder).max(1) as f64
                };
                let w = t * t * (3.0 - 2.0 * t);
                for k in 0..half {
                    let side = (PI * k as f64 / (half - 1) as f64).sin().powi(2);
                    out.r[i][k] *= 1.0 + (p.shoulder - 1.0) * w * side;
                }
            }
        }
        Kind::Arm => {
            let top = base.y[file.stations["top"]];
            for (y, &y0) in out.y.iter_mut().zip(&base.y) {
                if y0 < top {
                    *y = top + p.below * (y0 - top);
                }
            }
        }
    }
    if !p.girth.is_empty() {
        let scale = pchip(
            p.girth.iter().map(|g| g.0 as f64).collect(),
            p.girth.iter().map(|g| g.1).collect(),
        );
        for (i, ring) in out.r.iter_mut().enumerate() {
            let c = scale(i as f64);
            for r in ring {
                *r *= c;
            }
        }
    }
    out
}

/// Secant solve of an increasing `f(x) = target` starting at `x0`; `None` if it fails.
fn solve(f: impl Fn(f64) -> f64, target: f64, x0: f64) -> Option<f64> {
    let (mut xa, mut fa) = (x0, f(x0) - target);
    if fa.abs() <= SOLVE_TOL {
        return Some(x0);
    }
    let mut xb = x0 * if fa > 0.0 { 0.98 } else { 1.02 };
    let mut fb = f(xb) - target;
    for _ in 0..40 {
        if fb.abs() <= SOLVE_TOL {
            return Some(xb);
        }
        if fb == fa {
            return None;
        }
        let x = xb - fb * (xb - xa) / (fb - fa);
        if !(x > 0.0 && x < 3.0) {
            return None;
        }
        (xa, fa) = (xb, fb);
        xb = x;
        fb = f(xb) - target;
    }
    None
}

/// `base` resized so every input of `file` matches `size` (mm), or the measurement that can't be.
pub fn resize(file: &FormFile, base: &Rings, size: &Measurements) -> Result<Rings, SizeError> {
    let range = |m: &str| {
        let [lo, hi] = file.ranges[m];
        (lo, hi)
    };
    let refuse = |m: &str, lo: f64, hi: f64| SizeError {
        measurement: m.to_string(),
        min_mm: lo,
        max_mm: hi,
    };
    for m in &file.inputs {
        let (lo, hi) = range(m);
        match size.get(m) {
            Some(v) if v.is_finite() && (lo..=hi).contains(v) => {}
            _ => return Err(refuse(m, lo, hi)),
        }
    }
    let target = |m: &str| size[m] / 1000.0;
    let mut girths: Vec<(usize, &str)> = file
        .inputs
        .iter()
        .filter_map(|m| file.stations.get(m).map(|&i| (i, m.as_str())))
        .collect();
    girths.sort_unstable();
    let lengths: Vec<&str> = file
        .inputs
        .iter()
        .map(String::as_str)
        .filter(|m| !file.stations.contains_key(*m))
        .collect();
    let mut p = Params {
        above: 1.0,
        below: 1.0,
        shoulder: 1.0,
        girth: girths.iter().map(|&(i, _)| (i, 1.0)).collect(),
    };
    let without_girths = |p: &Params| {
        apply(
            file,
            base,
            &Params {
                girth: vec![],
                ..p.clone()
            },
        )
    };
    for _ in 0..PASSES {
        for &m in &lengths {
            let k = knob(m);
            let f = |x: f64| {
                let mut q = p.clone();
                *q.slot(k) = x;
                length(file, &apply(file, base, &q), m)
            };
            let Some(x) = solve(f, target(m), p.value(k)) else {
                let (lo, hi) = range(m);
                return Err(refuse(m, lo, hi));
            };
            *p.slot(k) = x;
        }
        let unscaled = without_girths(&p);
        for (g, &(i, m)) in p.girth.iter_mut().zip(&girths) {
            g.1 = target(m) / unscaled.girth(i);
        }
        let sized = apply(file, base, &p);
        if lengths
            .iter()
            .all(|m| (length(file, &sized, m) - target(m)).abs() <= 10.0 * SOLVE_TOL)
        {
            break;
        }
    }
    let unscaled = without_girths(&p);
    let plausible = |x: f64| (1.0 / MAX_RATIO..=MAX_RATIO).contains(&x);
    for &m in &lengths {
        if !plausible(p.value(knob(m))) {
            let (lo, hi) = range(m);
            let own = length(file, base, m) * 1000.0;
            return Err(refuse(m, lo.max(own / MAX_RATIO), hi.min(own * MAX_RATIO)));
        }
    }
    for w in 1..p.girth.len() {
        let (c0, c1) = (p.girth[w - 1].1, p.girth[w].1);
        if !plausible(c1 / c0) {
            // Blame whichever of the two strays further from the form's own proportions.
            let (j, k) = if c1.ln().abs() >= c0.ln().abs() {
                (w, w - 1)
            } else {
                (w - 1, w)
            };
            let m = girths[j].1;
            let (lo, hi) = range(m);
            let own = unscaled.girth(girths[j].0) * 1000.0;
            let ck = p.girth[k].1;
            return Err(refuse(m, lo.max(ck / MAX_RATIO * own), hi.min(ck * MAX_RATIO * own)));
        }
    }
    let sized = apply(file, base, &p);
    for &m in &lengths {
        if (length(file, &sized, m) - target(m)).abs() > ACCEPT_TOL {
            let (lo, hi) = range(m);
            return Err(refuse(m, lo, hi));
        }
    }
    Ok(sized)
}
```

- [ ] **Step 4: Run the tests to see them pass**

Run: `cargo nextest run -p opendrape-body form::resize`
Expected: PASS, 8 tests.

If `larger_and_smaller_targets_are_hit` misses a length by a little more than 2 mm, raise `PASSES` (the lengths and girths need more rounds to settle). Never widen the 1 mm / 2 mm tolerances.

- [ ] **Step 5: Commit**

```bash
cargo fmt --all
git add crates/body
git commit -m "feat(body): resize a form to target measurements with exact girths and plausibility checks

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 7: Build a sized form with arms, tapes and stand

**Files:**
- Create: `crates/body/src/form/stand.rs`
- Modify: `crates/body/src/form/mod.rs`

**Interfaces:**
- Consumes: `FormFile`, `Rings`, `tape::{tapes, ribbons}`, `resize::{resize, length}`, `Measurements`, `SizeError`.
- Produces:
  - `pub struct Arms { pub left: bool, pub right: bool }` (`Default`: no arms);
  - `pub enum Side { Left, Right }`;
  - `pub enum Quality { Low, Standard }` (`Default`: `Standard`);
  - `pub struct BuiltArm { pub side: Side, pub mesh: BodyMesh, pub top: DVec3, pub down: DVec3 }`;
  - `pub struct BuiltForm { pub torso: BodyMesh, pub arms: Vec<BuiltArm>, pub tapes: BodyMesh, pub stand: BodyMesh, pub landmarks: BTreeMap<String, DVec3>, pub stations: BTreeMap<String, f64>, pub measured: Measurements, pub collision: Collision }`;
  - `pub struct Form`, with:
    - `Form::new(torso: FormFile, arm: Option<FormFile>) -> Result<Form, FormError>`;
    - `file(&self) -> &FormFile`;
    - `inputs(&self) -> Vec<(String, [f64; 2])>`;
    - `base_measurements(&self, Quality) -> Measurements`;
    - `build(&self, &Measurements, Arms, Quality) -> Result<BuiltForm, SizeError>`.
  - The `measured` keys: every input, every ring-tape station (`high_hip`, `wrist`), `front_waist_length`, `back_width`, `apex_to_apex` (when `bust_apex` exists), and `upper_arm` / `arm_length` when arms are attached.

- [ ] **Step 1: Write the failing tests**

Add at the end of `crates/body/src/form/mod.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::boundary_edge_count;

    const BOTH: Arms = Arms {
        left: true,
        right: true,
    };

    fn form() -> Form {
        Form::new(fixture::torso(), Some(fixture::arm())).unwrap()
    }

    fn own_size(f: &Form) -> Measurements {
        f.base_measurements(Quality::Standard)
    }

    #[test]
    fn builds_a_closed_torso_with_tapes_landmarks_and_stand() {
        let f = form();
        let b = f.build(&own_size(&f), Arms::default(), Quality::Standard).unwrap();
        assert_eq!(boundary_edge_count(&b.torso), 0);
        assert_eq!(b.torso.triangles.len(), 2 * 96 * 81);
        assert!(b.arms.is_empty());
        assert!(!b.tapes.triangles.is_empty());
        assert_eq!(boundary_edge_count(&b.stand), 0);
        assert_eq!(b.stand.positions.iter().map(|p| p.y).fold(f32::MAX, f32::min), 0.0);
        let (l, r) = (b.landmarks["bust_apex"], b.landmarks["bust_apex_R"]);
        assert!((l.x + r.x).abs() < 1e-9 && l.x > 0.0);
        assert!(!b.landmarks.contains_key("front_neck_R"), "centre-line marks have no mirror");
        assert!((b.stations["waist"] - 1.03).abs() < 1e-9);
    }

    #[test]
    fn measured_has_every_input_and_the_read_only_values() {
        let f = form();
        let b = f.build(&own_size(&f), BOTH, Quality::Standard).unwrap();
        let extra = ["high_hip", "front_waist_length", "back_width", "apex_to_apex", "upper_arm", "arm_length"];
        for m in f.file().inputs.iter().map(String::as_str).chain(extra) {
            assert!(b.measured.get(m).is_some_and(|v| v.is_finite() && *v > 0.0), "{m}");
        }
        assert_eq!(f.inputs().len(), f.file().inputs.len() + 2, "torso inputs, then the arm's");
    }

    #[test]
    fn arms_attach_into_the_plates_and_mirror_each_other() {
        let f = form();
        let b = f.build(&own_size(&f), BOTH, Quality::Standard).unwrap();
        assert_eq!(b.arms.len(), 2);
        let (l, r) = (&b.arms[0], &b.arms[1]);
        assert_eq!((l.side, r.side), (Side::Left, Side::Right));
        for a in [l, r] {
            assert_eq!(boundary_edge_count(&a.mesh), 0);
        }
        for (p, q) in l.mesh.positions.iter().zip(&r.mesh.positions) {
            assert_eq!((p.x, p.y, p.z), (-q.x, q.y, q.z));
        }
        assert!(l.down.x > 0.0 && l.down.y < -0.9, "hangs down and a little out: {:?}", l.down);
        let plate = b.landmarks["plate_centre"];
        let inner = l.mesh.positions.iter().map(|p| f64::from(p.x)).fold(f64::MAX, f64::min);
        assert!((inner - (plate.x - 0.01)).abs() < 0.003, "sinks 1 cm into the plate: {inner} vs {}", plate.x);
    }

    #[test]
    fn missing_arm_sizes_fall_back_to_the_arms_own() {
        let f = form();
        let mut size = own_size(&f);
        size.remove("upper_arm");
        size.remove("arm_length");
        assert_eq!(f.build(&size, BOTH, Quality::Standard).unwrap().arms.len(), 2);
    }

    #[test]
    fn low_quality_has_fewer_triangles() {
        let f = form();
        let b = f.build(&f.base_measurements(Quality::Low), Arms::default(), Quality::Low).unwrap();
        assert_eq!(b.torso.triangles.len(), 2 * 64 * 81);
    }

    #[test]
    fn size_errors_come_back_and_parts_must_be_the_right_kind() {
        let f = form();
        let mut size = own_size(&f);
        size.insert("waist".into(), 400.0);
        assert_eq!(f.build(&size, Arms::default(), Quality::Standard).unwrap_err().measurement, "waist");
        assert!(Form::new(fixture::arm(), None).is_err());
        assert!(Form::new(fixture::torso(), Some(fixture::torso())).is_err());
    }
}
```

- [ ] **Step 2: Run the tests to see them fail**

Run: `cargo nextest run -p opendrape-body form::tests`
Expected: compile errors: `Form`, `Arms`, `Quality`, `Side` not found.

- [ ] **Step 3: Implement the stand**

Create `crates/body/src/form/stand.rs`:

```rust
//! The stand a form sits on, drawn but not collided: a neck cap with a knob, a pole, and a
//! base disc on the floor (y = 0).

use super::file::FormFile;
use super::rings::Rings;
use crate::BodyMesh;
use glam::DVec3;
use std::f64::consts::TAU;

/// A closed 24-sided cylinder standing on `base`, counter-clockwise seen from outside.
fn cylinder(m: &mut BodyMesh, base: DVec3, radius: f64, height: f64) {
    const SIDES: u32 = 24;
    let o = m.positions.len() as u32;
    for level in [0.0, height] {
        for j in 0..SIDES {
            let a = TAU * f64::from(j) / f64::from(SIDES);
            m.positions
                .push((base + DVec3::new(radius * a.sin(), level, radius * a.cos())).as_vec3());
        }
    }
    m.positions.push(base.as_vec3());
    m.positions.push((base + DVec3::Y * height).as_vec3());
    let (bottom, top) = (o + 2 * SIDES, o + 2 * SIDES + 1);
    for j in 0..SIDES {
        let k = (j + 1) % SIDES;
        let (a, b, c, d) = (o + j, o + k, o + SIDES + k, o + SIDES + j);
        m.triangles
            .extend([[a, b, c], [a, c, d], [bottom, b, a], [top, d, c]]);
    }
}

pub fn stand(file: &FormFile, rings: &Rings) -> BodyMesh {
    let n = rings.len();
    let neck = rings.r[file.stations["neck"]]
        .iter()
        .copied()
        .fold(f64::MAX, f64::min);
    let top = DVec3::new(0.0, rings.y[n - 1], rings.zc[n - 1]);
    let bottom = DVec3::new(0.0, rings.y[0], rings.zc[0]);
    let mut m = BodyMesh {
        positions: vec![],
        triangles: vec![],
    };
    cylinder(&mut m, top - DVec3::Y * 0.01, 0.9 * neck, 0.025);
    cylinder(&mut m, top + DVec3::Y * 0.015, 0.012, 0.02);
    cylinder(&mut m, DVec3::new(0.0, 0.02, bottom.z), 0.0125, bottom.y - 0.01);
    cylinder(&mut m, DVec3::new(0.0, 0.0, bottom.z), 0.22, 0.02);
    m
}
```

- [ ] **Step 4: Implement the form API**

In `crates/body/src/form/mod.rs`, add `mod stand;` and `pub use rings::Rings;` (if not already there), plus this code above the tests:

```rust
use crate::BodyMesh;
use glam::{DQuat, DVec3};
use std::collections::BTreeMap;
use std::f64::consts::{PI, TAU};

/// Which soft arms to attach.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Arms {
    pub left: bool,
    pub right: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Side {
    Left,
    Right,
}

/// Mesh density: `Standard` has 96 samples around each ring, `Low` 64 (for weak laptops).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Quality {
    Low,
    #[default]
    Standard,
}

impl Quality {
    fn half_angles(self) -> usize {
        match self {
            Quality::Low => 33,
            Quality::Standard => ANGLES,
        }
    }
}

/// A soft arm placed on the form.
#[derive(Clone, Debug)]
pub struct BuiltArm {
    pub side: Side,
    /// Closed, ready for collision.
    pub mesh: BodyMesh,
    /// Top of the arm's axis, inside the arm, level with the armhole plate's centre.
    pub top: DVec3,
    /// Unit vector down the arm.
    pub down: DVec3,
}

/// A dress form at one size, ready to drape on and draw.
#[derive(Clone, Debug)]
pub struct BuiltForm {
    /// Closed torso mesh, for collision and drawing.
    pub torso: BodyMesh,
    pub arms: Vec<BuiltArm>,
    /// Tape-line ribbons, for drawing only.
    pub tapes: BodyMesh,
    /// Neck cap, pole and base, for drawing only.
    pub stand: BodyMesh,
    /// Landmark positions; off-centre landmarks also as `<name>_R` on the right side.
    pub landmarks: BTreeMap<String, DVec3>,
    /// Station heights, metres.
    pub stations: BTreeMap<String, f64>,
    /// Every measurement of this build, millimetres.
    pub measured: Measurements,
    pub collision: Collision,
}

/// A torso form and the soft arm it takes.
#[derive(Clone, Debug)]
pub struct Form {
    torso: FormFile,
    arm: Option<FormFile>,
}

impl Form {
    pub fn new(torso: FormFile, arm: Option<FormFile>) -> Result<Self, FormError> {
        torso.check()?;
        if torso.kind != Kind::Torso {
            return Err(FormError(format!("form {} is not a torso", torso.id)));
        }
        if let Some(a) = &arm {
            a.check()?;
            if a.kind != Kind::Arm {
                return Err(FormError(format!("form {} is not an arm", a.id)));
            }
        }
        Ok(Self { torso, arm })
    }

    pub fn file(&self) -> &FormFile {
        &self.torso
    }

    fn parts(&self) -> impl Iterator<Item = &FormFile> {
        std::iter::once(&self.torso).chain(&self.arm)
    }

    /// Every measurement a user can set (the torso's, then the arm's), with its range in mm.
    pub fn inputs(&self) -> Vec<(String, [f64; 2])> {
        self.parts()
            .flat_map(|f| f.inputs.iter().map(|m| (m.clone(), f.ranges[m])))
            .collect()
    }

    /// The form's own measurements before any resizing, mm.
    pub fn base_measurements(&self, quality: Quality) -> Measurements {
        let mut out = Measurements::new();
        for f in self.parts() {
            let rings = Rings::from_file(f).with_half_angles(quality.half_angles());
            out.extend(measure_all(f, &rings, &BTreeMap::new()));
        }
        out
    }

    /// The form at `size` (mm). Arm sizes missing from `size` fall back to the arm's own.
    pub fn build(
        &self,
        size: &Measurements,
        arms: Arms,
        quality: Quality,
    ) -> Result<BuiltForm, SizeError> {
        let half = quality.half_angles();
        let base = Rings::from_file(&self.torso).with_half_angles(half);
        let rings = resize::resize(&self.torso, &base, size)?;
        let mut landmarks = BTreeMap::new();
        for (name, &[phi, v]) in &self.torso.landmarks {
            landmarks.insert(name.clone(), rings.point(phi, v));
            if phi > 1e-9 && phi < PI - 1e-9 {
                landmarks.insert(format!("{name}_R"), rings.point(TAU - phi, v));
            }
        }
        let mut measured = measure_all(&self.torso, &rings, &landmarks);
        let mut built_arms = vec![];
        if let (Some(arm), Some(attach)) = (&self.arm, &self.torso.arm)
            && (arms.left || arms.right)
        {
            let arm_base = Rings::from_file(arm).with_half_angles(half);
            let own = measure_all(arm, &arm_base, &BTreeMap::new());
            let arm_size: Measurements = arm
                .inputs
                .iter()
                .map(|m| (m.clone(), size.get(m).copied().unwrap_or(own[m])))
                .collect();
            let arm_rings = resize::resize(arm, &arm_base, &arm_size)?;
            measured.extend(measure_all(arm, &arm_rings, &BTreeMap::new()));
            let anchor = landmarks[&attach.anchor];
            for (side, wanted) in [(Side::Left, arms.left), (Side::Right, arms.right)] {
                if wanted {
                    built_arms.push(place_arm(arm, &arm_rings, attach, anchor, side));
                }
            }
        }
        let tapes = tape::tapes(&self.torso, &rings);
        Ok(BuiltForm {
            torso: rings.mesh(),
            arms: built_arms,
            tapes: tape::ribbons(&tapes, &rings),
            stand: stand::stand(&self.torso, &rings),
            stations: self
                .torso
                .stations
                .iter()
                .map(|(n, &i)| (n.clone(), rings.y[i]))
                .collect(),
            landmarks,
            measured,
            collision: self.torso.collision.clone(),
        })
    }
}

/// Every girth and length this part defines (and apex to apex when both apexes exist), mm.
fn measure_all(
    file: &FormFile,
    rings: &Rings,
    landmarks: &BTreeMap<String, DVec3>,
) -> Measurements {
    let mut out = Measurements::new();
    let ring_tapes = file.tapes.values().filter_map(|t| match t {
        TapeDef::Ring { ring } => Some(ring),
        TapeDef::Path { .. } => None,
    });
    let girths = file
        .inputs
        .iter()
        .filter(|m| file.stations.contains_key(*m))
        .chain(ring_tapes);
    for station in girths {
        out.insert(station.clone(), rings.girth(file.stations[station]) * 1000.0);
    }
    for &(m, ..) in file.lengths() {
        out.insert(m.to_string(), resize::length(file, rings, m) * 1000.0);
    }
    if let (Some(a), Some(b)) = (landmarks.get("bust_apex"), landmarks.get("bust_apex_R")) {
        out.insert("apex_to_apex".into(), a.distance(*b) * 1000.0);
    }
    out
}

/// The arm hangs from its top station, sunk `overlap` into the plate at `anchor` (a left-side
/// landmark) and tilted `hang_deg` outward. The right arm is the left one mirrored.
fn place_arm(
    file: &FormFile,
    rings: &Rings,
    attach: &ArmAttach,
    anchor: DVec3,
    side: Side,
) -> BuiltArm {
    let top = file.stations["top"];
    // Radius towards the body (by symmetry, the same as the radius at 90°).
    let inner = rings.r[top][rings.half() / 2];
    let pivot = anchor + DVec3::X * (inner - attach.overlap);
    let tilt = DQuat::from_rotation_z(attach.hang_deg.to_radians());
    let lift = DVec3::new(0.0, rings.y[top], 0.0);
    let mut mesh = rings.mesh();
    for p in &mut mesh.positions {
        *p = (pivot + tilt * (p.as_dvec3() - lift)).as_vec3();
    }
    let mut arm = BuiltArm {
        side,
        mesh,
        top: pivot,
        down: tilt * DVec3::NEG_Y,
    };
    if side == Side::Right {
        for p in &mut arm.mesh.positions {
            p.x = -p.x;
        }
        for t in &mut arm.mesh.triangles {
            t.swap(1, 2);
        }
        arm.top.x = -arm.top.x;
        arm.down.x = -arm.down.x;
    }
    arm
}
```

- [ ] **Step 5: Run the tests to see them pass**

Run: `cargo nextest run -p opendrape-body`
Expected: PASS: all body tests, including the 6 new `form::tests`.

- [ ] **Step 6: Commit**

```bash
cargo fmt --all
git add crates/body
git commit -m "feat(body): build a sized dress form with soft arms, tape ribbons and a stand

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 8: Size charts, bundled forms, and whole-library checks

**Files:**
- Create: `crates/body/src/form/chart.rs`, `crates/body/tests/forms.rs`
- Create: `assets/forms/charts/women-torso-classic.json`, `women-torso-everyday.json`, `men-torso-classic.json`, `men-torso-everyday.json`
- Modify: `crates/body/src/form/mod.rs` (add `pub mod chart; pub use chart::{Chart, ChartSize};`, `FORMS`, `Form::IDS`, `Form::bundled`), `ASSETS.md`

**Interfaces:**
- Consumes: everything from Tasks 2–7.
- Produces:
  - `Form::IDS: [&str; 2] = ["women-torso", "men-torso"]`, `Form::bundled(id) -> Option<Form>`;
  - `pub struct Chart { pub format: u32, pub id: String, pub form: String, pub name: String, pub source: String, pub sizes: Vec<ChartSize> }`;
  - `pub struct ChartSize { pub label: String, pub alt: Option<String>, pub mm: Measurements }`;
  - `Chart::from_json(&str) -> Result<Chart, FormError>`, `Chart::bundled() -> Vec<Chart>`, `Chart::for_form(&str) -> Vec<Chart>`, `Chart::size(&self, label) -> Option<&ChartSize>`.

- [ ] **Step 1: Add the four charts**

The numbers are OpenDrape's own grading, checked to within 2 cm against public references. Classic is the professional dress-form table and Kennett & Lindsell. Everyday is ASTM D5585 (straight) as reported publicly, plus an ASTM-regular soft-form table. Men's Everyday is the classic men's chart with the waist 5 cm and the hip 2 cm fuller.

Create `assets/forms/charts/women-torso-classic.json`:

```json
{
 "format": 1,
 "id": "women-torso-classic",
 "form": "women-torso",
 "name": "chart-classic",
 "source": "OpenDrape grading, checked within 2 cm against published professional dress form size tables (theshopcompany.com, moodfabrics.com) and Kennett & Lindsell UK forms",
 "sizes": [
  {"label": "US 0", "alt": "UK 4", "mm": {"bust": 815, "under_bust": 665, "waist": 610, "hip": 865, "neck": 310, "shoulder_length": 115, "back_waist_length": 395, "waist_to_hip": 195, "upper_arm": 250, "arm_length": 560}},
  {"label": "US 2", "alt": "UK 6", "mm": {"bust": 825, "under_bust": 675, "waist": 625, "hip": 880, "neck": 320, "shoulder_length": 120, "back_waist_length": 400, "waist_to_hip": 195, "upper_arm": 255, "arm_length": 565}},
  {"label": "US 4", "alt": "UK 8", "mm": {"bust": 850, "under_bust": 700, "waist": 640, "hip": 895, "neck": 325, "shoulder_length": 125, "back_waist_length": 405, "waist_to_hip": 200, "upper_arm": 260, "arm_length": 570}},
  {"label": "US 6", "alt": "UK 10", "mm": {"bust": 875, "under_bust": 725, "waist": 655, "hip": 910, "neck": 330, "shoulder_length": 125, "back_waist_length": 410, "waist_to_hip": 200, "upper_arm": 270, "arm_length": 575}},
  {"label": "US 8", "alt": "UK 12", "mm": {"bust": 900, "under_bust": 750, "waist": 675, "hip": 930, "neck": 340, "shoulder_length": 130, "back_waist_length": 420, "waist_to_hip": 205, "upper_arm": 280, "arm_length": 580}},
  {"label": "US 10", "alt": "UK 14", "mm": {"bust": 925, "under_bust": 775, "waist": 700, "hip": 955, "neck": 350, "shoulder_length": 135, "back_waist_length": 425, "waist_to_hip": 205, "upper_arm": 290, "arm_length": 585}},
  {"label": "US 12", "alt": "UK 16", "mm": {"bust": 965, "under_bust": 815, "waist": 735, "hip": 995, "neck": 365, "shoulder_length": 140, "back_waist_length": 430, "waist_to_hip": 210, "upper_arm": 305, "arm_length": 590}},
  {"label": "US 14", "alt": "UK 18", "mm": {"bust": 1005, "under_bust": 855, "waist": 775, "hip": 1035, "neck": 380, "shoulder_length": 145, "back_waist_length": 440, "waist_to_hip": 210, "upper_arm": 320, "arm_length": 595}},
  {"label": "US 16", "alt": "UK 20", "mm": {"bust": 1045, "under_bust": 895, "waist": 815, "hip": 1075, "neck": 395, "shoulder_length": 150, "back_waist_length": 445, "waist_to_hip": 215, "upper_arm": 335, "arm_length": 600}},
  {"label": "US 18", "alt": "UK 22", "mm": {"bust": 1105, "under_bust": 955, "waist": 875, "hip": 1145, "neck": 415, "shoulder_length": 155, "back_waist_length": 450, "waist_to_hip": 215, "upper_arm": 355, "arm_length": 605}}
 ]
}
```

Create `assets/forms/charts/women-torso-everyday.json`:

```json
{
 "format": 1,
 "id": "women-torso-everyday",
 "form": "women-torso",
 "name": "chart-everyday",
 "source": "OpenDrape grading, checked within 2 cm against ASTM D5585 (straight) figures as reported publicly (Wikipedia, US standard clothing size) and an ASTM-regular soft form table",
 "sizes": [
  {"label": "US 2", "alt": "UK 6", "mm": {"bust": 850, "under_bust": 720, "waist": 675, "hip": 900, "neck": 330, "shoulder_length": 120, "back_waist_length": 400, "waist_to_hip": 200, "upper_arm": 265, "arm_length": 565}},
  {"label": "US 4", "alt": "UK 8", "mm": {"bust": 875, "under_bust": 745, "waist": 700, "hip": 925, "neck": 335, "shoulder_length": 125, "back_waist_length": 405, "waist_to_hip": 200, "upper_arm": 275, "arm_length": 570}},
  {"label": "US 6", "alt": "UK 10", "mm": {"bust": 900, "under_bust": 770, "waist": 725, "hip": 950, "neck": 340, "shoulder_length": 125, "back_waist_length": 410, "waist_to_hip": 205, "upper_arm": 285, "arm_length": 575}},
  {"label": "US 8", "alt": "UK 12", "mm": {"bust": 925, "under_bust": 795, "waist": 750, "hip": 975, "neck": 345, "shoulder_length": 130, "back_waist_length": 415, "waist_to_hip": 205, "upper_arm": 295, "arm_length": 580}},
  {"label": "US 10", "alt": "UK 14", "mm": {"bust": 950, "under_bust": 820, "waist": 775, "hip": 1000, "neck": 350, "shoulder_length": 130, "back_waist_length": 420, "waist_to_hip": 210, "upper_arm": 305, "arm_length": 585}},
  {"label": "US 12", "alt": "UK 16", "mm": {"bust": 985, "under_bust": 855, "waist": 810, "hip": 1035, "neck": 360, "shoulder_length": 135, "back_waist_length": 425, "waist_to_hip": 210, "upper_arm": 320, "arm_length": 590}},
  {"label": "US 14", "alt": "UK 18", "mm": {"bust": 1025, "under_bust": 895, "waist": 850, "hip": 1075, "neck": 370, "shoulder_length": 140, "back_waist_length": 430, "waist_to_hip": 215, "upper_arm": 335, "arm_length": 595}},
  {"label": "US 16", "alt": "UK 20", "mm": {"bust": 1065, "under_bust": 935, "waist": 890, "hip": 1115, "neck": 380, "shoulder_length": 145, "back_waist_length": 435, "waist_to_hip": 215, "upper_arm": 350, "arm_length": 600}},
  {"label": "US 18", "alt": "UK 22", "mm": {"bust": 1115, "under_bust": 985, "waist": 940, "hip": 1165, "neck": 395, "shoulder_length": 150, "back_waist_length": 440, "waist_to_hip": 220, "upper_arm": 370, "arm_length": 605}},
  {"label": "US 20", "alt": "UK 24", "mm": {"bust": 1165, "under_bust": 1035, "waist": 990, "hip": 1215, "neck": 410, "shoulder_length": 155, "back_waist_length": 445, "waist_to_hip": 220, "upper_arm": 390, "arm_length": 610}}
 ]
}
```

Create `assets/forms/charts/men-torso-classic.json`:

```json
{
 "format": 1,
 "id": "men-torso-classic",
 "form": "men-torso",
 "name": "chart-classic",
 "source": "OpenDrape grading, checked within 2 cm against published professional men's dress form size tables (theshopcompany.com)",
 "sizes": [
  {"label": "34", "mm": {"chest": 865, "waist": 750, "hip": 850, "neck": 355, "shoulder_length": 150, "back_waist_length": 440, "waist_to_hip": 185, "upper_arm": 280, "arm_length": 610}},
  {"label": "36", "mm": {"chest": 915, "waist": 805, "hip": 895, "neck": 370, "shoulder_length": 155, "back_waist_length": 445, "waist_to_hip": 190, "upper_arm": 295, "arm_length": 615}},
  {"label": "38", "mm": {"chest": 965, "waist": 855, "hip": 940, "neck": 380, "shoulder_length": 155, "back_waist_length": 450, "waist_to_hip": 190, "upper_arm": 310, "arm_length": 620}},
  {"label": "40", "mm": {"chest": 1015, "waist": 910, "hip": 985, "neck": 395, "shoulder_length": 160, "back_waist_length": 455, "waist_to_hip": 195, "upper_arm": 325, "arm_length": 625}},
  {"label": "42", "mm": {"chest": 1065, "waist": 965, "hip": 1030, "neck": 405, "shoulder_length": 165, "back_waist_length": 460, "waist_to_hip": 195, "upper_arm": 340, "arm_length": 630}},
  {"label": "44", "mm": {"chest": 1120, "waist": 1015, "hip": 1070, "neck": 420, "shoulder_length": 165, "back_waist_length": 465, "waist_to_hip": 200, "upper_arm": 355, "arm_length": 635}},
  {"label": "46", "mm": {"chest": 1170, "waist": 1070, "hip": 1115, "neck": 430, "shoulder_length": 170, "back_waist_length": 470, "waist_to_hip": 200, "upper_arm": 370, "arm_length": 640}},
  {"label": "48", "mm": {"chest": 1220, "waist": 1125, "hip": 1160, "neck": 440, "shoulder_length": 175, "back_waist_length": 475, "waist_to_hip": 205, "upper_arm": 385, "arm_length": 645}},
  {"label": "50", "mm": {"chest": 1270, "waist": 1180, "hip": 1205, "neck": 450, "shoulder_length": 175, "back_waist_length": 480, "waist_to_hip": 205, "upper_arm": 400, "arm_length": 650}},
  {"label": "52", "mm": {"chest": 1320, "waist": 1230, "hip": 1245, "neck": 460, "shoulder_length": 180, "back_waist_length": 485, "waist_to_hip": 210, "upper_arm": 415, "arm_length": 655}}
 ]
}
```

Create `assets/forms/charts/men-torso-everyday.json`:

```json
{
 "format": 1,
 "id": "men-torso-everyday",
 "form": "men-torso",
 "name": "chart-everyday",
 "source": "OpenDrape grading: the classic men's chart with a 5 cm fuller waist and 2 cm fuller hip, checked against ASTM D6240 (men's regular) drops as reported publicly",
 "sizes": [
  {"label": "34", "mm": {"chest": 865, "waist": 800, "hip": 870, "neck": 355, "shoulder_length": 150, "back_waist_length": 440, "waist_to_hip": 185, "upper_arm": 280, "arm_length": 610}},
  {"label": "36", "mm": {"chest": 915, "waist": 855, "hip": 915, "neck": 370, "shoulder_length": 155, "back_waist_length": 445, "waist_to_hip": 190, "upper_arm": 295, "arm_length": 615}},
  {"label": "38", "mm": {"chest": 965, "waist": 905, "hip": 960, "neck": 380, "shoulder_length": 155, "back_waist_length": 450, "waist_to_hip": 190, "upper_arm": 310, "arm_length": 620}},
  {"label": "40", "mm": {"chest": 1015, "waist": 960, "hip": 1005, "neck": 395, "shoulder_length": 160, "back_waist_length": 455, "waist_to_hip": 195, "upper_arm": 325, "arm_length": 625}},
  {"label": "42", "mm": {"chest": 1065, "waist": 1015, "hip": 1050, "neck": 405, "shoulder_length": 165, "back_waist_length": 460, "waist_to_hip": 195, "upper_arm": 340, "arm_length": 630}},
  {"label": "44", "mm": {"chest": 1120, "waist": 1065, "hip": 1090, "neck": 420, "shoulder_length": 165, "back_waist_length": 465, "waist_to_hip": 200, "upper_arm": 355, "arm_length": 635}},
  {"label": "46", "mm": {"chest": 1170, "waist": 1120, "hip": 1135, "neck": 430, "shoulder_length": 170, "back_waist_length": 470, "waist_to_hip": 200, "upper_arm": 370, "arm_length": 640}},
  {"label": "48", "mm": {"chest": 1220, "waist": 1175, "hip": 1180, "neck": 440, "shoulder_length": 175, "back_waist_length": 475, "waist_to_hip": 205, "upper_arm": 385, "arm_length": 645}},
  {"label": "50", "mm": {"chest": 1270, "waist": 1230, "hip": 1225, "neck": 450, "shoulder_length": 175, "back_waist_length": 480, "waist_to_hip": 205, "upper_arm": 400, "arm_length": 650}},
  {"label": "52", "mm": {"chest": 1320, "waist": 1280, "hip": 1265, "neck": 460, "shoulder_length": 180, "back_waist_length": 485, "waist_to_hip": 210, "upper_arm": 415, "arm_length": 655}}
 ]
}
```

Append to `ASSETS.md`:

```markdown
| `assets/forms/charts/*.json` | OpenDrape's own size charts (graded in-house). Each file's `source` field names the public references it was checked against, within 2 cm: published professional dress-form tables (theshopcompany.com, moodfabrics.com), Kennett & Lindsell UK forms, ASTM D5585 / D6240 figures as reported publicly. No table is copied | CC0 1.0 |
```

- [ ] **Step 2: Write the failing tests**

Create `crates/body/tests/forms.rs`:

```rust
use opendrape_body::form::{Arms, Chart, Form, Measurements, Quality, Side, SizeError};
use opendrape_body::{boundary_edge_count, girth_at};
use std::collections::HashSet;

const BOTH: Arms = Arms {
    left: true,
    right: true,
};

#[test]
fn every_bundled_form_builds_closed_symmetric_and_in_budget() {
    for id in Form::IDS {
        let form = Form::bundled(id).expect(id);
        let built = form
            .build(&form.base_measurements(Quality::Standard), BOTH, Quality::Standard)
            .expect("its own size fits");
        let t = built.torso.triangles.len();
        assert!((10_000..=30_000).contains(&t), "{id}: {t} triangles");
        assert_eq!(boundary_edge_count(&built.torso), 0, "{id} torso is closed");
        assert_eq!(built.arms.len(), 2);
        for arm in &built.arms {
            assert_eq!(boundary_edge_count(&arm.mesh), 0, "{id} arm is closed");
        }
        assert_eq!(boundary_edge_count(&built.stand), 0);
        let key = |x: f32, y: f32, z: f32| {
            ((x * 1e5).round() as i64, (y * 1e5).round() as i64, (z * 1e5).round() as i64)
        };
        let set: HashSet<_> = built.torso.positions.iter().map(|p| key(p.x, p.y, p.z)).collect();
        assert!(
            built.torso.positions.iter().all(|p| set.contains(&key(-p.x, p.y, p.z))),
            "{id} is mirror-symmetric"
        );
        let waist = built.stations["waist"];
        assert!((1.01..1.05).contains(&waist), "{id} waist at {waist} m");
        let lowest = built.stand.positions.iter().map(|p| p.y).fold(f32::MAX, f32::min);
        assert_eq!(lowest, 0.0, "{id} stands on the floor");
    }
    assert!(Form::bundled("soft-arm").is_none(), "an arm is not a torso form");
    assert!(Form::bundled("nonsense").is_none());
}

/// Girths sliced from the mesh (like a tape measure) within 1 mm, lengths within 2 mm.
fn check_fit(form: &Form, size: &Measurements, quality: Quality, what: &str) {
    let built = form
        .build(size, BOTH, quality)
        .unwrap_or_else(|e| panic!("{what}: {e}"));
    let f = form.file();
    for m in &f.inputs {
        let (got, tol) = if f.stations.contains_key(m) {
            let slice = girth_at(&built.torso, built.stations[m] as f32, 10.0);
            (1000.0 * f64::from(slice), 1.0)
        } else {
            (built.measured[m], 2.0)
        };
        assert!((got - size[m]).abs() <= tol, "{what} {m}: {got:.1} mm, chart says {}", size[m]);
    }
    for (m, tol) in [("upper_arm", 1.0), ("arm_length", 2.0)] {
        let got = built.measured[m];
        assert!((got - size[m]).abs() <= tol, "{what} {m}: {got:.1} mm, chart says {}", size[m]);
    }
}

#[test]
fn every_chart_row_fits_its_form() {
    for chart in Chart::bundled() {
        let form = Form::bundled(&chart.form).expect("the chart's form is bundled");
        for size in &chart.sizes {
            check_fit(&form, &size.mm, Quality::Standard, &format!("{} {}", chart.id, size.label));
        }
    }
}

#[test]
fn low_quality_still_fits_the_charts() {
    for chart in Chart::bundled() {
        let form = Form::bundled(&chart.form).unwrap();
        for size in [&chart.sizes[0], chart.sizes.last().unwrap()] {
            check_fit(&form, &size.mm, Quality::Low, &format!("{} {} (low)", chart.id, size.label));
        }
    }
}

#[test]
fn charts_cover_their_forms() {
    let charts = Chart::bundled();
    assert_eq!(charts.len(), 4);
    for c in &charts {
        let form = Form::bundled(&c.form).unwrap();
        let first = form.file().inputs[0].clone();
        let mut labels = HashSet::new();
        for s in &c.sizes {
            assert!(labels.insert(&s.label), "{} repeats {}", c.id, s.label);
            for (m, _) in form.inputs() {
                assert!(s.mm.contains_key(&m), "{} {} lacks {m}", c.id, s.label);
            }
        }
        assert!(
            c.sizes.windows(2).all(|w| w[0].mm[&first] < w[1].mm[&first]),
            "{} grows size by size",
            c.id
        );
        assert!(c.size(&c.sizes[3].label).is_some());
    }
    assert_eq!(Chart::for_form("women-torso").len(), 2);
    assert_eq!(Chart::for_form("men-torso").len(), 2);
}

#[test]
fn refusals_name_the_measurement_and_its_range() {
    let form = Form::bundled("women-torso").unwrap();
    let mut size = Chart::for_form("women-torso")[0].sizes[4].mm.clone();
    size.insert("waist".into(), 400.0);
    let err = form.build(&size, Arms::default(), Quality::Standard).unwrap_err();
    assert_eq!(
        err,
        SizeError {
            measurement: "waist".into(),
            min_mm: 500.0,
            max_mm: 1300.0
        }
    );
}

#[test]
fn arms_sit_on_their_plates_at_every_size() {
    for chart in Chart::bundled() {
        let form = Form::bundled(&chart.form).unwrap();
        for size in [&chart.sizes[0], chart.sizes.last().unwrap()] {
            let what = format!("{} {}", chart.id, size.label);
            let built = form.build(&size.mm, BOTH, Quality::Standard).unwrap();
            let plate = built.landmarks["plate_centre"];
            let left = built.arms.iter().find(|a| a.side == Side::Left).unwrap();
            let xs = |m: &opendrape_body::BodyMesh, y: f64| {
                m.positions
                    .iter()
                    .filter(move |p| (f64::from(p.y) - y).abs() < 0.01)
                    .map(|p| f64::from(p.x))
                    .collect::<Vec<_>>()
            };
            let inner = left.mesh.positions.iter().map(|p| f64::from(p.x)).fold(f64::MAX, f64::min);
            assert!(inner < plate.x && inner > plate.x - 0.02, "{what}: arm reaches x = {inner}, plate at {}", plate.x);
            // A sleeve fits between arm and side: 25 cm down the arm there is a 2 cm gap.
            let y = left.top.y - 0.25;
            let side = xs(&built.torso, y).into_iter().fold(f64::MIN, f64::max);
            let arm = xs(&left.mesh, y).into_iter().fold(f64::MAX, f64::min);
            assert!(arm - side > 0.02, "{what}: only {:.3} m between arm and side", arm - side);
        }
    }
}
```

- [ ] **Step 3: Run the tests to see them fail**

Run: `cargo nextest run -p opendrape-body --test forms`
Expected: compile errors: `Chart`, `Form::IDS`, `Form::bundled` not found.

- [ ] **Step 4: Implement charts and the bundled registry**

Create `crates/body/src/form/chart.rs`:

```rust
//! Size charts: named sizes for one form, each a full set of measurements in millimetres.

use super::{FormError, Measurements};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ChartSize {
    pub label: String,
    /// Another name for the same size (e.g. "UK 12"), if there is one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub alt: Option<String>,
    pub mm: Measurements,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Chart {
    pub format: u32,
    pub id: String,
    /// The id of the form this chart sizes.
    pub form: String,
    /// Fluent message id of the chart's name ("Classic form", "Everyday body").
    pub name: String,
    /// Where the numbers come from (for ASSETS.md, not shown in the app).
    pub source: String,
    pub sizes: Vec<ChartSize>,
}

const BUNDLED: [&str; 4] = [
    include_str!("../../../../assets/forms/charts/women-torso-classic.json"),
    include_str!("../../../../assets/forms/charts/women-torso-everyday.json"),
    include_str!("../../../../assets/forms/charts/men-torso-classic.json"),
    include_str!("../../../../assets/forms/charts/men-torso-everyday.json"),
];

impl Chart {
    pub fn from_json(s: &str) -> Result<Self, FormError> {
        serde_json::from_str(s).map_err(|e| FormError(format!("not a size chart: {e}")))
    }

    /// Every bundled chart, women's classic first.
    pub fn bundled() -> Vec<Chart> {
        BUNDLED
            .iter()
            .map(|s| Chart::from_json(s).expect("bundled charts are valid"))
            .collect()
    }

    pub fn for_form(form_id: &str) -> Vec<Chart> {
        Self::bundled()
            .into_iter()
            .filter(|c| c.form == form_id)
            .collect()
    }

    pub fn size(&self, label: &str) -> Option<&ChartSize> {
        self.sizes.iter().find(|s| s.label == label)
    }
}
```

In `crates/body/src/form/mod.rs`, add `pub mod chart;` and `pub use chart::{Chart, ChartSize};`. Then add:

```rust
const FORMS: [(&str, &str); 3] = [
    ("women-torso", include_str!("../../../../assets/forms/women-torso.form.json")),
    ("men-torso", include_str!("../../../../assets/forms/men-torso.form.json")),
    ("soft-arm", include_str!("../../../../assets/forms/soft-arm.form.json")),
];

impl Form {
    /// The bundled torso forms, in picker order.
    pub const IDS: [&'static str; 2] = ["women-torso", "men-torso"];

    /// A bundled torso form with its soft arm, or `None` for an id that isn't a bundled torso.
    pub fn bundled(id: &str) -> Option<Form> {
        let file = |id: &str| {
            FORMS
                .iter()
                .find(|f| f.0 == id)
                .map(|f| FormFile::from_json(f.1).expect("bundled form files are valid"))
        };
        let torso = file(id).filter(|f| f.kind == Kind::Torso)?;
        let arm = torso.arm.as_ref().and_then(|a| file(&a.form));
        Some(Form::new(torso, arm).expect("bundled forms are valid"))
    }
}
```

- [ ] **Step 5: Run the tests to see them pass**

Run: `cargo nextest run -p opendrape-body`
Expected: PASS, including the 6 tests in `tests/forms.rs`.

If a chart row fails, read the message: it names the measurement.
- **Out of range:** the chart is wrong, or the meta range is too tight. Fix the chart or meta, re-export if the meta changed, and tell the user about any chart change.
- **Neighbour ratio:** the base form is too far from that size. Make the base form closer to the middle size (Task 2, Step 3).
- **Arm gap below 2 cm:** increase `hang_deg` in both torso meta files (14–16°), re-export, and re-run.

- [ ] **Step 6: Commit**

```bash
cargo fmt --all
git add crates/body assets/forms/charts ASSETS.md
git commit -m "feat(body): bundled forms and Classic/Everyday size charts that every form size fits

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 9: One collider for torso, arms and floor

**Files:**
- Modify: `crates/sim/src/collide.rs`, `crates/sim/src/lib.rs`

**Interfaces:**
- Consumes: nothing new.
- Produces:
  - the `Collider` trait gains `fn signed_distance(&self, p: DVec3) -> f64` (negative inside);
  - `pub struct CompoundCollider`, with `CompoundCollider::new(parts: Vec<BodyCollider>, floor: Option<f64>)` and `parts(&self) -> &[BodyCollider]`;
  - `impl Collider for CompoundCollider`;
  - `opendrape_sim::CompoundCollider` is exported.

- [ ] **Step 1: Write the failing tests**

In the `tests` module of `crates/sim/src/collide.rs`, replace `fn cube() -> BodyCollider { ... }` with a shifted version, keeping `cube()`:

```rust
    /// Unit cube centred on (dx, 0, 0).
    fn cube_at(dx: f32) -> BodyCollider {
        let p = [
            [-0.5, -0.5, -0.5],
            [0.5, -0.5, -0.5],
            [0.5, 0.5, -0.5],
            [-0.5, 0.5, -0.5],
            [-0.5, -0.5, 0.5],
            [0.5, -0.5, 0.5],
            [0.5, 0.5, 0.5],
            [-0.5, 0.5, 0.5],
        ]
        .map(|c| Vec3::from_array(c) + Vec3::X * dx);
        let t = [
            [0, 3, 2],
            [0, 2, 1],
            [4, 5, 6],
            [4, 6, 7],
            [0, 4, 7],
            [0, 7, 3],
            [1, 2, 6],
            [1, 6, 5],
            [0, 1, 5],
            [0, 5, 4],
            [3, 7, 6],
            [3, 6, 2],
        ];
        BodyCollider::new(&p, &t).expect("closed cube")
    }

    fn cube() -> BodyCollider {
        cube_at(0.0)
    }
```

Then add:

```rust
    #[test]
    fn compound_takes_the_nearest_way_out_then_the_nearest_surface() {
        // Two cubes overlapping between x = 0.3 and 0.5.
        let c = CompoundCollider::new(vec![cube_at(0.0), cube_at(0.8)], None);
        let planes = c.contact_planes(
            &[
                DVec3::new(0.0, 0.0, 0.45),
                DVec3::new(0.9, 0.0, 0.0),
                DVec3::new(0.0, 0.53, 0.0),
                DVec3::new(0.0, 3.0, 0.0),
                DVec3::new(0.45, 0.0, 0.0),
            ],
            0.05,
        );
        let p = planes[0].expect("inside the first cube");
        assert!(p.normal.abs_diff_eq(DVec3::Z, 1e-6), "{p:?}");
        let p = planes[1].expect("inside the second cube");
        assert!(
            p.normal.abs_diff_eq(DVec3::X, 1e-6) && p.point.abs_diff_eq(DVec3::new(1.3, 0.0, 0.0), 1e-6),
            "{p:?}"
        );
        let p = planes[2].expect("just above the first cube");
        assert!(p.normal.abs_diff_eq(DVec3::Y, 1e-6), "{p:?}");
        assert_eq!(planes[3], None, "far from everything");
        let p = planes[4].expect("inside both");
        assert!(
            p.normal.abs_diff_eq(DVec3::X, 1e-6) && (p.point.x - 0.5).abs() < 1e-6,
            "nearest way out, not the far side: {p:?}"
        );
    }

    #[test]
    fn floor_holds_particles_above_it() {
        let c = CompoundCollider::new(vec![], Some(0.0));
        let planes = c.contact_planes(
            &[
                DVec3::new(5.0, 0.01, 5.0),
                DVec3::new(5.0, -0.1, 5.0),
                DVec3::new(5.0, 1.0, 5.0),
            ],
            0.05,
        );
        let on_floor = Some(Plane {
            normal: DVec3::Y,
            point: DVec3::new(5.0, 0.0, 5.0),
        });
        assert_eq!(planes[0], on_floor);
        assert_eq!(planes[1], on_floor, "below the floor is pushed back up");
        assert_eq!(planes[2], None);
    }

    #[test]
    fn signed_distance_is_the_union_of_parts_and_floor() {
        let c = CompoundCollider::new(vec![cube_at(0.0), cube_at(0.8)], Some(-0.6));
        assert!((c.signed_distance(DVec3::new(0.0, 0.0, 0.3)) + 0.2).abs() < 1e-6);
        assert!((c.signed_distance(DVec3::new(0.9, 0.0, 0.0)) + 0.4).abs() < 1e-6);
        assert!((c.signed_distance(DVec3::new(3.0, -0.7, 0.0)) + 0.1).abs() < 1e-6, "below the floor");
        let single: &dyn Collider = &cube();
        assert!((single.signed_distance(DVec3::new(0.0, 0.0, 0.8)) - 0.3).abs() < 1e-6);
    }

    #[test]
    fn cloth_settles_on_the_floor() {
        let c = CompoundCollider::new(vec![], Some(0.0));
        let mut s = falling_triangle(0.2);
        for _ in 0..120 {
            s.step(Some(&c));
        }
        for p in s.cloth().positions() {
            assert!(p.y >= 0.003 - 1e-4 && p.y < 0.02, "resting on the floor: {p}");
        }
    }
```

- [ ] **Step 2: Run the tests to see them fail**

Run: `cargo nextest run -p opendrape-sim collide`
Expected: compile errors: `CompoundCollider` not found; no method `signed_distance` on `&dyn Collider`.

- [ ] **Step 3: Implement**

In `crates/sim/src/collide.rs`, extend the trait:

```rust
/// Something cloth collides with. Queried once per frame for every particle; `None` means
/// the particle is farther than `margin` from it (and outside).
pub trait Collider: Sync {
    fn contact_planes(&self, x: &[DVec3], margin: f64) -> Vec<Option<Plane>>;
    /// Distance to the surface, negative inside.
    fn signed_distance(&self, p: DVec3) -> f64;
}
```

In `impl Collider for BodyCollider`, add:

```rust
    fn signed_distance(&self, p: DVec3) -> f64 {
        // The inherent method of the same name.
        BodyCollider::signed_distance(self, p)
    }
```

Add, after the `BodyCollider` impls:

```rust
/// Several closed bodies (a dress form's torso and arms) and an optional floor at height
/// `floor`, as one collider.
pub struct CompoundCollider {
    parts: Vec<BodyCollider>,
    floor: Option<f64>,
}

impl CompoundCollider {
    pub fn new(parts: Vec<BodyCollider>, floor: Option<f64>) -> Self {
        Self { parts, floor }
    }

    pub fn parts(&self) -> &[BodyCollider] {
        &self.parts
    }
}

impl Collider for CompoundCollider {
    fn contact_planes(&self, x: &[DVec3], margin: f64) -> Vec<Option<Plane>> {
        let per_part: Vec<Vec<Option<Plane>>> = self
            .parts
            .iter()
            .map(|c| c.contact_planes(x, margin))
            .collect();
        (0..x.len())
            .into_par_iter()
            .map(|i| {
                let floor = self
                    .floor
                    .filter(|f| x[i].y - f < margin)
                    .map(|f| Plane {
                        normal: DVec3::Y,
                        point: DVec3::new(x[i].x, f, x[i].z),
                    });
                per_part
                    .iter()
                    .filter_map(|planes| planes[i])
                    .chain(floor)
                    .map(|p| ((x[i] - p.point).dot(p.normal), p))
                    // Inside anything: the nearest way out. Otherwise: the nearest surface.
                    .min_by(|(a, _), (b, _)| {
                        (*a >= 0.0)
                            .cmp(&(*b >= 0.0))
                            .then(a.abs().total_cmp(&b.abs()))
                    })
                    .map(|(_, p)| p)
            })
            .collect()
    }

    fn signed_distance(&self, p: DVec3) -> f64 {
        let parts = self
            .parts
            .iter()
            .map(|c| c.signed_distance(p))
            .fold(f64::INFINITY, f64::min);
        self.floor.map_or(parts, |f| parts.min(p.y - f))
    }
}
```

In `crates/sim/src/lib.rs`, export it:

```rust
pub use collide::{BodyCollider, Collider, ColliderError, CompoundCollider, Plane};
```

- [ ] **Step 4: Run the tests to see them pass**

Run: `cargo nextest run -p opendrape-sim`
Expected: PASS, with the 4 new tests and every old one.

- [ ] **Step 5: Commit**

```bash
cargo fmt --all
git add crates/sim
git commit -m "feat(sim): one collider for a form's torso, arms and the floor

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 10: Drape the demo garments on the forms

**Files:**
- Create: `crates/testkit/src/forms.rs`, `crates/testkit/tests/forms.rs`
- Modify: `crates/testkit/src/lib.rs`, `crates/testkit/src/garments.rs`, `crates/testkit/src/metrics.rs`

**Interfaces:**
- Consumes:
  - `opendrape_body::form::{BuiltForm, Side, Form, Chart, Arms, Quality}`;
  - `opendrape_sim::{CompoundCollider, Collider, BodyCollider}`.
- Produces:
  - `Scene::new_on(Garment, &BuiltForm) -> Scene` and `Scene::collider(&self) -> &dyn Collider`;
  - `forms::{collider, skirt, bodice_proxy, sleeve, long_hem}`;
  - `metrics::measure(&Cloth, &dyn Collider)` (was `&BodyCollider`);
  - `metrics::run_solver(&mut Solver, &dyn Collider, seconds) -> f64`.
  - `Scene::new`, `Scene::step`, `metrics::run` and the particle constants behave exactly as before, so the app needs no change.

- [ ] **Step 1: Share the garment builders**

In `crates/testkit/src/garments.rs`:
- Make `grid_panel` `pub(crate)`.
- Split `skirt()` and `bodice_proxy()` into the body-specific numbers and shared builders. **The arithmetic must stay identical**, so `drape_is_deterministic` and the particle counts don't move.

```rust
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
            let phi = if front {
                p.x / r
            } else {
                std::f64::consts::PI - p.x / r
            };
            DVec3::new(r * phi.sin(), waist_y + p.y, zc + r * phi.cos())
        };
        let mut panel = grid_panel(rows, cols, false, Some(&flat), &place);
        if !front {
            // The back panel is placed mirrored; flip its winding so both panels face outward
            // and the welded seams get consistent normals.
            for t in &mut panel.triangles {
                t.swap(1, 2);
            }
        }
        panels.push(builder.add_panel(&panel, 1.0));
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
            weld_time: None,
            ..Params::default()
        },
    )
}
```

Give `Scene` its own collider:

```rust
use opendrape_body::form::BuiltForm;
use opendrape_sim::{BodyCollider, ClothBuilder, Collider, CompoundCollider, Panel, Params, Solver};

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

    /// The demo garment on a dress form, colliding with its torso, arms and the floor.
    pub fn new_on(garment: Garment, form: &BuiltForm) -> Self {
        let collider = crate::forms::collider(form);
        let solver = match garment {
            Garment::Skirt => crate::forms::skirt(form),
            Garment::BodiceProxy => crate::forms::bodice_proxy(form, &collider),
        };
        Self {
            garment,
            solver,
            contact: Contact::Form(Box::new(collider)),
        }
    }

    pub fn collider(&self) -> &dyn Collider {
        match &self.contact {
            Contact::Bundled => collider(),
            Contact::Form(c) => c.as_ref(),
        }
    }

    pub fn step(&mut self) {
        let c: &dyn Collider = match &self.contact {
            Contact::Bundled => collider(),
            Contact::Form(c) => c.as_ref(),
        };
        self.solver.step(Some(c));
    }
}
```

In `crates/testkit/src/metrics.rs`:

```rust
use crate::garments::Scene;
use opendrape_sim::{Cloth, Collider, Solver};
```

Change the signature to `pub fn measure(cloth: &Cloth, collider: &dyn Collider) -> DrapeReport`; the body stays the same. Replace `run` with:

```rust
/// Steps `scene` for `seconds` of simulated time; returns wall-clock ms per frame.
pub fn run(scene: &mut Scene, seconds: f64) -> f64 {
    let frames = (seconds / opendrape_sim::FRAME_DT).round() as usize;
    let start = std::time::Instant::now();
    for _ in 0..frames {
        scene.step();
    }
    start.elapsed().as_secs_f64() * 1000.0 / frames.max(1) as f64
}

/// Steps `solver` against `collider` for `seconds`; returns wall-clock ms per frame.
pub fn run_solver(solver: &mut Solver, collider: &dyn Collider, seconds: f64) -> f64 {
    let frames = (seconds / opendrape_sim::FRAME_DT).round() as usize;
    let start = std::time::Instant::now();
    for _ in 0..frames {
        solver.step(Some(collider));
    }
    start.elapsed().as_secs_f64() * 1000.0 / frames.max(1) as f64
}
```

Run: `cargo nextest run -p opendrape-testkit -p opendrape-app`
Expected: still fails to compile until `forms.rs` exists (Step 3). After Step 3, every old test passes unchanged.

- [ ] **Step 2: Write the failing form tests**

Create `crates/testkit/tests/forms.rs`:

```rust
use opendrape_body::form::{Arms, BuiltForm, Chart, Form, Quality, Side};
use opendrape_testkit::forms;
use opendrape_testkit::garments::{Garment, Scene};
use opendrape_testkit::metrics::{measure, position_hash, run, run_solver};

#[derive(Clone, Copy)]
enum Which {
    Smallest,
    Middle,
    Largest,
}

fn built(chart: &str, which: Which, arms: Arms) -> BuiltForm {
    let chart = Chart::bundled().into_iter().find(|c| c.id == chart).expect("chart");
    let size = match which {
        Which::Smallest => &chart.sizes[0],
        Which::Middle => &chart.sizes[chart.sizes.len() / 2],
        Which::Largest => chart.sizes.last().unwrap(),
    };
    Form::bundled(&chart.form)
        .expect("form")
        .build(&size.mm, arms, Quality::Standard)
        .expect("chart sizes fit")
}

/// The body skirt's gates, with "stayed up" measured from this form's waist.
fn skirt_gates(chart: &str, which: Which) {
    let form = built(chart, which, Arms::default());
    let mut scene = Scene::new_on(Garment::Skirt, &form);
    run(&mut scene, 6.0);
    let r = measure(scene.solver.cloth(), scene.collider());
    eprintln!("{r:#?}");
    let waist = form.stations["waist"];
    assert!(!r.has_nan);
    assert!(r.penetration_max_mm <= 2.0 && r.penetration_p99_mm <= 1.0, "poke-through");
    assert!(!r.open_stitches && r.seam_gap_max_mm == 0.0, "seams welded shut");
    assert!(r.strain_p99 <= 0.10 && r.strain_max <= 0.20, "fabric over-stretched");
    assert!(r.kinetic_energy <= 1e-4, "still moving: {} J", r.kinetic_energy);
    assert!(
        r.highest_y > waist - 0.05 && r.lowest_y > waist - 0.65,
        "skirt slid down: {}..{}",
        r.lowest_y,
        r.highest_y
    );
}

/// The body tube's gates.
fn tube_gates(chart: &str, which: Which) {
    let form = built(chart, which, Arms::default());
    let mut scene = Scene::new_on(Garment::BodiceProxy, &form);
    run(&mut scene, 4.0);
    let r = measure(scene.solver.cloth(), scene.collider());
    eprintln!("{r:#?}");
    assert!(!r.has_nan);
    assert!(r.penetration_max_mm <= 2.0, "poke-through under tension");
    assert!(r.strain_p99 <= 0.05, "tube over-stretched");
    assert!(r.kinetic_energy <= 1e-6, "still moving: {} J", r.kinetic_energy);
}

#[test]
fn skirt_on_women_classic_smallest() {
    skirt_gates("women-torso-classic", Which::Smallest)
}
#[test]
fn skirt_on_women_classic_middle() {
    skirt_gates("women-torso-classic", Which::Middle)
}
#[test]
fn skirt_on_women_classic_largest() {
    skirt_gates("women-torso-classic", Which::Largest)
}
#[test]
fn skirt_on_women_everyday_smallest() {
    skirt_gates("women-torso-everyday", Which::Smallest)
}
#[test]
fn skirt_on_women_everyday_middle() {
    skirt_gates("women-torso-everyday", Which::Middle)
}
#[test]
fn skirt_on_women_everyday_largest() {
    skirt_gates("women-torso-everyday", Which::Largest)
}
#[test]
fn tube_on_women_classic_smallest() {
    tube_gates("women-torso-classic", Which::Smallest)
}
#[test]
fn tube_on_women_classic_middle() {
    tube_gates("women-torso-classic", Which::Middle)
}
#[test]
fn tube_on_women_classic_largest() {
    tube_gates("women-torso-classic", Which::Largest)
}
#[test]
fn tube_on_women_everyday_smallest() {
    tube_gates("women-torso-everyday", Which::Smallest)
}
#[test]
fn tube_on_women_everyday_middle() {
    tube_gates("women-torso-everyday", Which::Middle)
}
#[test]
fn tube_on_women_everyday_largest() {
    tube_gates("women-torso-everyday", Which::Largest)
}
#[test]
fn tube_on_men_classic_middle() {
    tube_gates("men-torso-classic", Which::Middle)
}

#[test]
fn sleeve_tube_holds_close_to_an_attached_arm() {
    let form = built(
        "women-torso-classic",
        Which::Middle,
        Arms {
            left: true,
            right: false,
        },
    );
    let collider = forms::collider(&form);
    let mut solver = forms::sleeve(&form, Side::Left, &collider);
    run_solver(&mut solver, &collider, 4.0);
    let r = measure(solver.cloth(), &collider);
    eprintln!("{r:#?}");
    assert!(!r.has_nan);
    assert!(r.penetration_max_mm <= 2.0, "poke-through into the arm");
    assert!(r.strain_p99 <= 0.05, "sleeve over-stretched");
    assert!(r.kinetic_energy <= 1e-6, "still moving: {} J", r.kinetic_energy);
}

#[test]
fn a_long_hem_rests_on_the_floor() {
    let form = built("women-torso-classic", Which::Middle, Arms::default());
    let collider = forms::collider(&form);
    let mut solver = forms::long_hem(&form);
    run_solver(&mut solver, &collider, 5.0);
    let r = measure(solver.cloth(), &collider);
    eprintln!("{r:#?}");
    assert!(!r.has_nan);
    assert!(r.penetration_max_mm <= 2.0, "poke-through into form or floor");
    assert!(r.lowest_y >= -0.0005, "fell through the floor: {}", r.lowest_y);
    assert!(r.lowest_y <= 0.01, "the hem should reach the floor: {}", r.lowest_y);
}

#[test]
fn form_drape_is_deterministic() {
    let form = built("women-torso-classic", Which::Middle, Arms::default());
    let hash = || {
        let mut s = Scene::new_on(Garment::Skirt, &form);
        run(&mut s, 1.5);
        position_hash(s.solver.cloth())
    };
    assert_eq!(hash(), hash());
}
```

- [ ] **Step 3: Implement the form scenes**

Create `crates/testkit/src/forms.rs`:

```rust
//! The demo garments, and collision checks for arms and the floor, on dress forms.

use crate::garments::{DENSITY, EDGE, grid_panel, skirt_panels, tube};
use glam::DVec3;
use opendrape_body::BodyMesh;
use opendrape_body::form::{BuiltForm, Side};
use opendrape_sim::{BodyCollider, ClothBuilder, CompoundCollider, Params, Solver};

/// Torso first, then the arms in `form.arms` order, then the floor at y = 0.
pub fn collider(form: &BuiltForm) -> CompoundCollider {
    let part = |m: &BodyMesh| {
        BodyCollider::new(&m.positions, &m.triangles).expect("form parts are closed")
    };
    let mut parts = vec![part(&form.torso)];
    parts.extend(form.arms.iter().map(|a| part(&a.mesh)));
    CompoundCollider::new(parts, Some(0.0))
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

/// The A-line demo skirt sized to the form: 6 cm waist ease, the body skirt's flare and length.
pub fn skirt(form: &BuiltForm) -> Solver {
    let (waist_y, zc, length) = (form.stations["waist"], axis_z(form), 0.55);
    let waist_w = (form.measured["waist"] / 1000.0 + 0.06) / 2.0;
    let r = reach(form, zc, waist_y - length, waist_y + 0.02) + 0.03;
    skirt_panels(waist_y, zc, r, waist_w, waist_w + 0.245, length)
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

/// A close-fit sleeve tube (1 cm ease, 3% short) around the `side` arm, 14 to 34 cm below its
/// top, clear of the armpit where arm and side nearly touch; top ring pinned.
pub fn sleeve(form: &BuiltForm, side: Side, collider: &CompoundCollider) -> Solver {
    let k = form
        .arms
        .iter()
        .position(|a| a.side == side)
        .expect("that arm is attached");
    let (arm, body) = (&form.arms[k], &collider.parts()[1 + k]);
    let across = arm.down.cross(DVec3::Z).normalize();
    let fwd = across.cross(arm.down).normalize();
    let (s0, s1, cols) = (0.14, 0.34, 48usize);
    let rows = ((s1 - s0) / EDGE).round() as usize;
    let place = |i: usize, j: usize| {
        let s = s0 + (s1 - s0) * i as f64 / rows as f64;
        let a = std::f64::consts::TAU * j as f64 / cols as f64;
        let dir = fwd * a.cos() + across * a.sin();
        let origin = arm.top + arm.down * s;
        origin + dir * (body.ray_exit(origin, dir, 0.3).unwrap_or(0.05) + 0.01)
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
            weld_time: None,
            ..Params::default()
        },
    )
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
```

In `crates/testkit/src/lib.rs`, add `pub mod forms;` and update the crate doc:

```rust
//! Programmatic garments draped on the bundled body or on a dress form, and the drape-quality
//! metrics used by automated tests, the app's demo scenes and benchmarks.

pub mod forms;
pub mod garments;
pub mod metrics;
```

- [ ] **Step 4: Run all the drape tests**

Run: `cargo nextest run -p opendrape-testkit -p opendrape-app`
Expected: PASS. That covers the 16 new form tests and every old test (skirt, tube, determinism, winding, the app's sim-runner and UI tests).

**If a gate fails, fix the scene or the form, never the gate:**
- **Penetration at a sharp rim:** smooth that region of the form, then re-export and re-run.
- **Skirt slides down:** the ease is too big; lower the 0.06 waist ease to 0.04.
- **Not settled:** increase the simulated seconds (6 → 8 for the skirt, 5 → 7 for the long hem).

Report each change in the task's summary.

- [ ] **Step 5: Commit**

```bash
cargo fmt --all
git add crates/testkit
git commit -m "test(testkit): drape the demo skirt, tube, a sleeve and a long hem on the dress forms

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 11: Final checks and the hand-off to the user

**Files:**
- Modify: `docs/superpowers/specs/2026-10-09-dress-forms-design.md` (status line)

- [ ] **Step 1: Full verification**

Run each command and read its output:
- `cargo fmt --all --check`: no output.
- `cargo clippy --workspace --all-targets -- -D warnings`: no warnings.
- `cargo deny check`: `advisories ok, bans ok, licenses ok, sources ok`.
- `cargo nextest run --workspace --no-fail-fast`: every test passes. That is 225 old tests plus the new ones (about 60).
- `cargo run --release -p opendrape-testkit --example drape_bench`: still prints the two body scenes. It only prints to the terminal and opens no window.

- [ ] **Step 2: Record the status**

Append to the end of `docs/superpowers/specs/2026-10-09-dress-forms-design.md`, writing the day's date where it says `<date>`:

```markdown
## Status

- **Track A: complete (<date>)** on branch `dress-forms`.
  - Three CC0 parts are built in Blender and exported as ring files: women's torso, men's torso and the soft arm.
  - Four size charts ship: Classic and Everyday, for women and men.
  - The form engine in `crates/body` resizes forms, builds tapes, the stand and arms; `CompoundCollider` adds the floor.
  - Drape gates pass on every chart extreme.
- **Track B** (app panel, project field, retiring MakeHuman) starts after M2b merges.
```

Commit:

```bash
git add docs/superpowers/specs/2026-10-09-dress-forms-design.md
git commit -m "docs: dress forms track A complete

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

- [ ] **Step 3: Tell the user, in plain words**

Re-render the review sheets (`sh scripts/forms/blender.sh render women-torso men-torso soft-arm`) and send them with `SendUserFile`. Also send a short summary:
- what now exists, and that nothing changes in the app yet;
- the size chart numbers as a readable table (bust/chest, waist, hip per size), so they can sanity-check them as a designer;
- that Track B waits for M2b;
- a question: may the `dress-forms` branch be pushed to GitHub? Pushing a branch doesn't touch the nightly, which is only rebuilt from `main`. Push only on a yes.

Update the memory note `opendrape-project.md`: Track A is done, and where things live.

