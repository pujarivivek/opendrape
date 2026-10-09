# Dress forms design: a library of dress forms replacing the human avatar

## Context

OpenDrape drapes garments on a CC0 MakeHuman female body today (M1). The user finds designing on a
realistic human "a very weird way to design", and the body's anatomical detail was already an open
product problem for schools. Designers and fashion schools drape on **dress forms** (tailor's
dummies): idealised, symmetrical, headless torsos with tape lines, chosen to suit the garment. This
plan replaces the avatar with a library of dress forms modelled by us in Blender. We own every mesh
(Blender's FAQ says output is the user's sole property) and release them CC0.

### Decisions confirmed with the user (2026-10-09)
| Topic | Decision |
|---|---|
| Avatar | Dress forms **replace** the human avatar. MakeHuman body retired; old M5 "Avatars" (skin tones, poses, body import) is dropped |
| Sizing | Pick a size from a **size chart**, or **Custom**: type measurements and the form reshapes |
| Proportions | **Two charts per form**: "Classic form" (school/industry forms: smaller waist, higher bust) and "Everyday body" (closer to real customers). Chosen per project |
| First forms | **Women's full torso** and **Men's full torso** (neck to upper thigh) |
| Arms | Optional **soft draping arm**, attachable left/right, off by default |
| Modelling | Shaped in **Blender** (5.2 LTS installed; live MCP session for shaping, headless runs for export) |
| Scheduling | Forms built **in parallel** with M2b on a separate branch, merged after M2b |

### Research findings that shape the plan
- **Commercial tools ship human avatars**, resized by measurement (CLO, Browzwear, Style3D,
  Marvelous). Browzwear lists a female and a male "Dress Form", and CLO sells Alvanon forms
  separately. No tool is built around forms the way schools work, so this is a real gap.
- **Real adjustable forms** (iDummy, Euveka, Fits.me) resize with **4–8 girth stations plus a few
  lengths**. That is the parameter set to copy. Industry and research **morph one good hand-shaped
  base**, because pure maths lofting looks like CAD.
- **Trade form types:**
  - Half-body: neck to crotch, for bodices, dresses and skirts.
  - 3/4 body: legs to the knee, for trousers and swim.
  - Full body: to the ankle, for trousers and long coats.
  - Also half-scale (school practice), plus, petite, men's (by chest 34–52 in), children's,
    maternity (a belly only), and soft forms for lingerie/knits.
  - Add-ons: arms, legs, neck cap, shoulder caps, pads. Wolf Forms sells its range by garment type,
    a good template for our picker.
- **Tape lines on forms:** CF, CB, side seams, neckline, shoulder seam (through the armhole-plate
  screw), armhole, bust (level, apex to apex), under-bust, waist (narrowest point), high hip
  (5–7.5 cm below waist), hip (18–23 cm below waist), front princess (mid-shoulder over apex),
  back princess (over the shoulder blade), shoulder-blade line (¼ of nape-to-waist).
- **Sizing data:**
  - A classic professional form table is published (US 0–26: bust, waist, hip, neck, front/back
    length, shoulder length, apex-to-apex, back width), and a men's table (chest 34–52).
  - Kennett & Lindsell UK forms run several cm smaller than UK clothing charts.
  - ASTM D5585 (US misses) has curvy/straight variants. ISO 8559-1 defines the measurements and is
    aimed at mannequin makers.
  - **INDIAsize** (NIFT, 26,000 scans, 27 charts) is **paid (₹20,000 per chart) with no published
    licence, so it can't ship without NIFT's permission.**
  - African surveys exist (South Africa, Nigeria, Ghana, Kenya), but there is no national standard.
- **Collision bodies** must be closed with rounded rims, because a sharp bottom edge snags hanging
  cloth. 10–30k triangles is plenty.
- **Placement model:** Style3D gives arrangement points as **(around 0→1, up 0→1)** on the body,
  and pins are a surface point the cloth sticks to. Both inform M4.

### How the body works in the code today
- `crates/body/src/lib.rs`:
  - `BodyMesh { positions, triangles }`, in metres, Y up, facing +Z, with the lowest y = 0.
  - One `female_average.odb`, loaded with `include_bytes!`.
  - `girth_at` takes a convex-hull slice girth (`crates/body/src/measure.rs:30-74`).
  - `boundary_edge_count` checks the mesh is closed.
- `crates/sim/src/collide.rs`:
  - One `Collider` trait; `BodyCollider` wraps a parry3d ORIENTED `TriMesh` and builds exact
    contact planes each frame.
  - It needs a **closed, outward-wound, welded** mesh.
  - `Solver::step` takes one collider; there is no floor.
- `crates/render/src/mesh.rs`: `MeshRenderer` draws one flat colour per mesh with smooth normals.
  The body colour `SKIN` and the camera are hard-coded in `crates/app/src/viewport.rs:6,30-36`.
- `crates/testkit/src/garments.rs`:
  - Global `body()`/`collider()`.
  - Skirt waist hard-coded at y = 1.03, tube at y 0.99–1.19, and `|x| < 0.2` arm filters.
  - `metrics::measure` takes `&BodyCollider`.
  - The drape gates are in `crates/testkit/tests/drape.rs`.
- Project file and CI:
  - `crates/core/src/project.rs` has no body field; `SCHEMA_VERSION = 1`, and M2b moves it to 2.
  - CI byte-compares the MakeHuman asset (`.github/workflows/ci.yml:48-58`).

## Approach: one hand-shaped base form per type, stored as rings; the app resizes it

A dress form is a stack of cross-sections. Each form is stored as **rings**. At each of about 100
heights, the file holds the radius at 48 fixed angles from centre front (0°) to centre back (180°)
on one side, mirrored, so the form is exactly symmetric. Every point on the form has a stable
address, **(angle, height)**, normalised 0→1 like Style3D.

```
Blender: base form hand-shaped at the middle size ─► assets-src/forms/<form>.blend
  └─ scripts/forms/export_form.py (blender -b, headless): samples rings, reads named
     landmark empties and tape curves ─► assets/forms/<form>.form.json  (~60 KB, committed, shipped)
App: rings ─► resize to chosen size ─► closed triangle mesh (~19k tris) + tape ribbons + stand
     ─► CompoundCollider (sim) + MeshRenderer (render)
```

- **Hand-shaped character, exact sizes.** Bust, shoulder blades and the neck-to-shoulder curve are
  modelled in Blender. Scaling a ring about its centre scales its convex-hull perimeter by the
  same factor, and the existing `girth_at` slice at a station height returns exactly that ring. So
  every size hits its girths with no fitting error.
- **Landmarks, tape lines, future arrangement points and pins move with the form**, because they
  are stored as (angle, height).
- **Small and simple:** no subdivision, no shape keys, no binary converter. The JSON is the shipped
  asset. Quality presets resample fewer rings/angles for weak laptops.
- **Blender is the design tool, not a build step.** CI never needs Blender; tests validate the JSON.
- **Accepted limit:** one closed surface per part. Arms are separate parts with rings along their
  own axis. Legs (trouser forms, later) will need a new part type.

## The library

| Garment | Form | When |
|---|---|---|
| Dresses, bodices, tops, blouses, saree blouse, choli | Women's torso (+ arms for sleeves) | **v1** |
| Skirts, lehenga | Women's torso; long hems hang below and rest on the floor | **v1** (skirt-only form later) |
| Kurta, kameez, kaftan, dashiki | Women's or men's torso + arms | **v1** |
| Shirts, waistcoats, simple jackets | Men's torso + arms | **v1** |
| Trousers, salwar, churidar, shorts | 3/4 form with legs | later (trousers are v1.1) |
| Half-scale practice | Any form at 0.5× | later (cheap: a scale factor) |
| Children's, maternity, plus-shape, soft knit form | Own base forms or pad-style shape adjustments | later |

The picker groups forms by garment type, like Wolf Forms' catalogue. Each form says what it is
for.

## What each form file carries (`assets/forms/<id>.form.json`, `"format": 1`)
- **Identity:** `id` (e.g. `women-torso`), display-name key, the garments it suits, base size,
  licence CC0.
- **Rings:** heights (m), per-ring centre z, 48 half-side radii (mm, 0.1 mm rounding), bottom-rim
  bevel, neck-cap dome. The armhole plates are flat ovals shaped in Blender.
  - Rings are spaced by surface length along the side profile, not evenly in height. They are
    denser over the shoulders, where the surface is nearly flat, so triangles stay even-sized.
- **Stations:** `neck`, `shoulder`, `bust` (men: `chest`), `under_bust` (women), `waist`,
  `high_hip`, `hip`, `bottom`.
- **Landmarks** (angle, height): front/back neck, side neck L/R, shoulder points L/R, bust apex
  L/R, armhole-plate centre ("screw")/top/bottom L/R, shoulder-blade points, CF/CB/side points at
  bust, waist and hip.
- **Tape lines** (polylines in angle/height): every line in the research list above.
- **Measurement definitions** (which station or tape each value is read from), **allowed ranges**,
  and suggested collision `thickness`/`friction`.
- **Arm attachment** (torso only): anchor landmark and hang angle for each side.
- The **soft arm** is its own `soft-arm.form.json`. Its rings run along the arm's axis, with
  `upper_arm` and `arm_length` stations and a padded closed end (no hand).

## Sizing
- **Charts** live in `assets/forms/charts/<form>-classic.json` and `<form>-everyday.json`.
  - **We author them ourselves:** metric, round numbers, written grade rules. Each is checked to
    within ±2 cm against public references: the published classic form table, Kennett & Lindsell,
    and ASTM D5585 as reported publicly. Their sources are listed in `ASSETS.md`.
  - Each size is shown as a letter, the bust (or chest) in cm/in, and the US/UK number, e.g.
    "M · bust 90 cm · US 8 / UK 12".
  - Women: about 10 sizes, bust 80–125 cm. Men: chest 86–132 cm.
  - An Indian chart is added only if NIFT grants permission (see Open follow-ups).
- **Custom** is pre-filled from the selected size, so students change only what they need.
  - Women's inputs: bust, under-bust, waist, hip, neck, shoulder length, back waist length (nape to
    waist), waist-to-hip.
  - Men's inputs: chest, waist, hip, neck, shoulder length, back waist length, waist-to-hip.
  - Arms add upper-arm girth and arm length.
  - Read-only "measured" values: front waist length, apex-to-apex, back width, high hip.
- **Resize algorithm** (pure function in `crates/body`):
  1. **Lengths:** stretch ring spacing above and below the waist until back waist length (along the
     CB tape) and waist-to-hip (along the side seam) match. Each is a one-variable secant solve.
  2. **Shoulders:** scale x of the rings between underarm and neck base until the shoulder length
     (along the shoulder tape) matches.
  3. **Girths:** scale each station ring about its centre to the exact target. Between stations,
     the scale follows a monotone PCHIP curve, so the surface stays smooth with no bulges.
  4. **Repeat** steps 1–3 until lengths are within ±2 mm, usually 2–3 passes. Girths are exact
     after step 3.
- **Plausibility:**
  - Each measurement has a range per form, and neighbouring station scale factors must stay within
    0.75–1.33 of each other.
  - Requests outside that are refused with a message naming the measurement ("Waist can be 55–120
    cm on this form"), as M2a already does for edits.
- **Saved projects** store the form id, the size choice (chart + label, or Custom), the resolved
  measurements in mm, and arms left/right. A reopened file never silently changes its form.

## Collision, rendering, stand
- **`crates/sim`:**
  - Add `CompoundCollider`: several `BodyCollider` parts plus an optional floor plane at y = 0.
    Each particle takes the deepest plane if it is inside any part, otherwise the nearest one
    within the margin.
  - Torso + arms + floor make one collider, so `Solver::step` is unchanged. Long hems (lehenga,
    anarkali, gowns) rest on the floor.
  - `signed_distance` becomes a `Collider` trait method so `metrics::measure` can take
    `&dyn Collider`.
- **Rendering** reuses `MeshRenderer`:
  - the form in linen off-white;
  - tape lines as 6 mm black ribbons lifted 0.5 mm off the surface;
  - the neck cap, pole and base in wood/metal. The pole is drawn but not collided in v1.
- **Camera** frames the form's bounds. The form stands so that its waist sits near today's 1.03 m,
  which keeps the demo garments working and the drape gates comparable.

## Track A — now, in parallel with M2b (branch `dress-forms`, own git worktree)
M2b touches only `core`, `geom`, `io` and `app` (checked against its plan). Track A touches none of
them, and it adds new APIs beside the old ones, so `main` keeps working with the MakeHuman body.
1. **Base shapes in Blender.**
   - A scripted loft of two-ellipse-arc stations at the base size: women's classic M (bust about
     90 cm); men's chest 100 cm.
   - Then hand-shape them in the live Blender MCP session: bust, shoulder blades, neck-to-shoulder,
     armhole plates, rounded bottom.
   - Headless Blender renders front/side/back/¾ views, with and without tapes, with no window.
   - **The user approves the women's form, the men's form and the arm from these pictures before
     export.**
   - Commit the `.blend` files to `assets-src/forms/` and the export script to
     `scripts/forms/export_form.py`.
2. **Form engine in `crates/body`.**
   - New modules: `form` (JSON types, load, validate), `rings` (rings → closed mesh, resampling),
     `resize` (the algorithm above), `tape` (ribbons, surface lengths, landmark positions).
   - API: `Form::load(id)`, `Form::build(&FormSize, Quality) -> BuiltForm { torso, arms, tapes,
     stand, landmarks, measured }`.
   - Reuse `girth_at` and `boundary_edge_count`. `BodyMesh` stays as the mesh type.
3. **`CompoundCollider` + floor** in `crates/sim`.
4. **Testkit:**
   - Add `Scene::new_on(Garment, &BuiltForm)`. It places the skirt waist and tube from form
     landmarks instead of literals, and starts the tube rays at the form's centre instead of
     `torso_axis_z`.
   - The old `Scene::new` stays until Track B.
   - Add the form drape tests.
5. **CI and provenance:** a job that loads every form and chart and runs the validation tests.
   `ASSETS.md` gets rows for each `.blend`, `.form.json` and chart (CC0, in-house, Blender 5.2 LTS,
   chart references).

## Track B — after M2b merges: the "Dress forms" release (new M3)
1. **`crates/core` + `crates/io`:**
   - A `form` field on `Project` and a schema version bump (M2b's version + 1).
   - A migration: old files get the women's torso, classic chart, size M.
   - A frozen fixture for the new version; older fixtures stay.
2. **`crates/app`:**
   - A **Dress form** panel with:
     - a form picker grouped by garment type;
     - the chart (Classic/Everyday) and size;
     - *Custom…* fields with "measured" readouts;
     - Left/Right arm checkboxes and a "Show tape lines" toggle.
   - The viewport draws form, tapes and stand; `SKIN` goes; the camera frames the form.
   - Changing form or size resets the drape. All strings go through Fluent.
3. Switch testkit and app to forms, then delete the MakeHuman path in `Scene::new`.
4. **Retire MakeHuman:** remove `assets/body/female_average.odb`, `scripts/fetch-makehuman.sh`,
   `xtask/src/makehuman.rs` and `xtask body`, the CI byte-compare job, `crates/body/tests/asset.rs`
   and the MakeHuman `ASSETS.md` row.
5. **Docs:**
   - Update `docs/specs/2026-10-09-opendrape-design.md`: avatar rows, licence notes, the role of
     `crates/body`, and milestones (M3 Dress forms, M4 Sew & drape, M5 Fitted garments, Avatars
     removed, M6–M8 unchanged).
   - Write `docs/testing/M3-checklist.md` and update the memory notes.

## Verification
- **Asset checks (CI, every form):**
  - built mesh closed (`boundary_edge_count == 0`), outward-wound, welded;
  - mirror error ≤ 0.1 mm;
  - landmarks and tape points on the surface (≤ 0.5 mm);
  - 10–30k triangles at default quality.
- **Sizing (every chart row + proptest):**
  - girths at stations within ±1 mm, lengths within ±2 mm;
  - out-of-range input refused, never a crash;
  - deterministic;
  - landmark order preserved (apex above waist above hip).
- **Drape gates, unchanged thresholds:** penetration max ≤ 2 mm and p99 ≤ 1 mm, seams welded,
  strain p99 ≤ 10%, settles.
  - Skirt and tube on the women's form at its smallest, middle and largest size, on both charts.
  - Tube on the men's form.
  - A sleeve tube with an arm attached.
  - A long-hem tube resting on the floor at y ≥ 0.
- **Collider unit tests:** overlapping cubes, a particle inside near the joint, the floor.
- **UI (`egui_kittest`):** pick the men's form; switch chart; choose a size; type a custom waist
  and see the measured value; an out-of-range value is refused; toggle the arms. A v1/v2 project
  opens with the default form.
- **Render:** a golden image per form with tapes (lavapipe); the Windows WARP smoke test still
  passes.
- **User checklist (Mac, M3 release):**
  - Both forms look like real forms, with no anatomy.
  - S/M/L and Classic/Everyday visibly change the form.
  - A custom bust of 92 cm reads "92.0 cm".
  - The arms attach, and the demo skirt drapes on every size.

## Open follow-ups (not in this plan)
- **INDIAsize permission:** writing to NIFT to ask whether OpenDrape may ship their charts is an
  outward-facing step for the user to decide on. Until then, Indian students use Custom.
- Later forms: skirt form, 3/4 form with legs (with trousers in v1.1), half-scale, children's,
  maternity, plus-shape, soft knit form, pad-style shape adjustments (stomach, seat, bust, like
  Fabulous Fit pads), and pins and arrangement points in M4 using the form's (angle, height)
  addresses.

## Next steps on approval
1. Create the `dress-forms` worktree, save this design as
   `docs/superpowers/specs/2026-10-09-dress-forms-design.md`, and commit it there.
2. Write the Track A implementation plan (writing-plans skill) and run it. The first stop is the
   Blender pictures for the user's approval. **I'll ask before opening Blender** for the live
   session; headless renders open no window.
3. After M2b merges, write and run the Track B plan.
