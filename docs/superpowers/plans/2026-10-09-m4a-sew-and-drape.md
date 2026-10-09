# OpenDrape M4a (Sew & Drape, part 1) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** a student sews the edges of the pieces they drew, arranges the pieces round the form in 3D (a gizmo, **Place at…**, typed values), presses **Play** and watches the skirt they drafted drape. Seams and placements are saved in project format 3.

**Architecture:** "store once, derive the rest", as in M2b.
- **`opendrape-core`** stores only what the student did:
  - `Project.seams` (two sides of consecutive stored edges each);
  - `Piece.placement` and `Twin.placement` (position, unit quaternion, optional curve radius).

  Mirror-image seams are derived (`Project::mirror_of`). The edits that renumber edges keep seams valid. `Project::check()` refuses anything invalid.
- **`opendrape-geom`** maps a seam side onto the shape that shows it: which outline edges, which way, how long, as points.
- **`opendrape-mesh`** (new, pure) does two jobs:
  - it turns every shape into a fabric panel: its stitching outline sampled, both sides of a seam with the same count, and filled by spade's refined constrained Delaunay triangulation; seams become stitched point pairs;
  - `place` turns placements into 3D positions (with curve wrap and starting places), and also holds **Place at…** and the typed angles.
- **The app** gets:
  - the Sew tool (W), seam drawing and the seam panel;
  - a `Stage` boundary around the body (centred, with a floor);
  - a project-driven `SimRunner` that meshes and drapes on its own thread;
  - the 3D arranging view: a cached scene, ray picking, a shared selection, and an `Arranger` driving OpenDrape's own gizmo, whose maths is pure and tested and drawn by a thin egui overlay;
  - **Place at…** in both views, and typed placement in Properties.

**Tech Stack:** as M2b (Rust 1.99, eframe/egui/egui_kittest 0.36.2, wgpu 30.0.1, winit 0.30.13, kurbo 0.13.1, i_overlay 9.0.1), plus **spade =2.15.1** (MIT OR Apache-2.0). spade 2.15.1 is already in `Cargo.lock`, pulled in by parry3d 0.31.1, so the build gains no new crates. `glam` 0.33 (already a workspace dependency) is used by `opendrape-mesh` for the 3D maths.

**Spec:** `docs/superpowers/specs/2026-10-09-m4a-sew-and-drape-design.md` (approved by the user, section by section). Main spec: `docs/specs/2026-10-09-opendrape-design.md`. Parallel work: `docs/superpowers/specs/2026-10-09-dress-forms-design.md` on branch `dress-forms`.

**Evidence:**
- **The probe, `scratchpad/m4a-probe`.**
  - spade's API checked by compiling and running these calls (verified signatures):
    - `ConstrainedDelaunayTriangulation::<spade::Point2<f64>>::new()`, `Triangulation::insert(&mut self, V) -> Result<FixedVertexHandle, InsertionError>` and `Triangulation::num_vertices(&self) -> usize`. Inserting a point equal to an earlier one returns the earlier handle and adds nothing, which is how the plan detects repeated points.
    - `try_add_constraint(&mut self, from: FixedVertexHandle, to: FixedVertexHandle) -> Vec<FixedDirectedEdgeHandle>`: empty when the edge would cross a constraint, and more than one edge when it runs through a vertex. `add_constraint` and `add_constraint_edges` panic on a crossing, so the plan never calls them.
    - `RefinementParameters::<f64>::new()` with `.with_angle_limit(AngleLimit::from_deg(25.0))`, `.with_max_allowed_area(0.5·h²)`, `.with_min_required_area(0.02·h²)`, `.keep_constraint_edges()`, `.exclude_outer_faces(true)` and `.with_max_additional_vertices(n)`.
    - `refine(&mut self, RefinementParameters<f64>) -> RefinementResult { excluded_faces: Vec<FixedFaceHandle<InnerTag>>, refinement_complete: bool }`.
    - Reading back: `vertices()` (`.position()` gives `x`, `y`; `fix().index()` is insertion order, so boundary points stay `0..n` in the order given) and `inner_faces()` (`.fix()`, `.vertices() -> [VertexHandle; 3]`, anticlockwise).
    - spade refuses coordinates below 2⁻¹⁴²; the plan rounds |v| < 1e-9 mm to 0.
  - **Meshing results:**
    - boundary points are kept exactly and first;
    - holes (either winding) are left empty;
    - at 0.5 h² every edge is 0.62–1.47 h, and every angle is ≥ 26.1° on trapezoids, disks, L-shapes and holed squares;
    - crossings, repeated points and collinear outlines are refused without a panic (3,000 random outlines, 2,485 refused);
    - the output is the same on every run;
    - a 2,900-point panel takes 3.6 ms (opt-level 3).
  - **Gizmo maths at 144 camera poses** (8 yaws × 6 pitches × 3 distances):
    - arrow drags over 1,269 drags: exact to < 1e-6, even with the pointer 7 points off the arrow;
    - ring drags over 1,288 drags: exact; 440 edge-on cases fall back to the on-screen angle;
    - Euler x→y→z round trips;
    - a mirrored placement shows the exact mirror image;
    - Place-at arcs keep every point on the cylinder.
  - **The bundled body:**
    - torso centre line at z = 0.020;
    - height 1.59 m, shoulders ≈ 1.30 m;
    - surface distances from the centre line in the table below.
  - **A spade-meshed, stitched skirt draped for 6 s:**
    - 5,824 particles, 13.6 ms/frame;
    - 0.00 mm penetration, strain p99 6.3%, kinetic energy 1.9e-7 J;
    - it stays on the hips (0.47–1.03 m), and drapes the same on every run.
- **The prototype, `scratchpad/proto`:** a copy of `main` at `547362f` in which every task below was implemented first. After each task, `cargo fmt`, `cargo clippy --workspace --all-targets -- -D warnings` and `cargo nextest run --workspace` passed. At the end: 525 tests, `cargo deny check` (advisories, bans, licenses and sources ok) and `cargo about generate` (spade listed). The code in this plan is copied from it.

| Height (m) | front 0° | 45° | left 90° | 135° | back 180° |
|---|---|---|---|---|---|
| 0.75 | 0.068 | 0.150 | 0.182 | 0.124 | 0.039 |
| 0.80 | 0.094 | 0.137 | 0.175 | 0.140 | 0.076 |
| 0.90 | 0.119 | 0.137 | 0.147 | 0.103 | 0.072 |
| 1.03 | 0.123 | 0.131 | 0.108 | 0.057 | 0.034 |
| 1.30 | 0.056 | 0.070 | 0.181 | 0.103 | 0.060 |

**Where the spec's assumptions met the evidence (decided here; listed for the user):**
1. **Place at…'s curve radius:** the spec measures one ray, "at that angle and height".
   - At a skirt's middle height (0.755 m) that ray finds the front 7.5 cm out (between the legs) and the back 4.4 cm out (the seat). Those radii would wrap a 30 cm panel two-thirds of the way round the body, through the hips.
   - The plan takes the largest distance over the heights the piece spans and the angles it covers (iterated three times, because the span depends on the radius), plus 3 cm. That gives 21.4 cm and 20.7 cm, and the skirt drapes with nothing inside the form.
   - A form a ray can't find (a piece above the head) gets a 20 cm curve.
2. **Smallest angle:**
   - "≥ 20° everywhere" holds for triangles that don't touch an outline corner sharper than 20°; a sharper corner is itself the triangle's angle.
   - "≥ 25° away from sharper corners" holds more than 4 h from corners sharper than 45°. Near 30–40° corners, the probe's worst was 24.0°.
3. **Seam gap:**
   - Before welding (0.78 s) the closed seams are at most 2.7 mm apart (mean 0.49). The worst is the centre back crossing the hollow of the seat, so the spec's 1.5 mm maximum fails.
   - The gate is ≤ 4 mm and a mean ≤ 1 mm before the weld (a flipped seam measures 11.5 / 2.2 mm), and **welded shut** after it, as M1's gate.
4. **The drape test sews the centre back too.** With side seams only, the open-backed skirt slides off the form onto the floor (the probe's lowest point was 3 mm). The M4a checklist says to sew it.
5. **Arrow drags:** "converted to metres at the gizmo's depth" was up to 35% off for long drags seen close up. The plan keeps the spec's projection onto the arrow's on-screen direction, then takes that screen point back onto the 3D axis exactly (pointer ray against the axis line).
6. **A seam that is its own mirror image:**
   - Examples: a back sewn to its own twin (centre back), or a folded sleeve's drawn edge sewn to its pale image.
   - It has no separate mirror. Otherwise the mirror would sew the same edges twice and the seam would be refused.
7. **Shoulder height** (where pieces start) is 0.82 × the form's height (1.30 m for the bundled body). The dress forms will give their own.

---

## Global Constraints

Carried from M0–M2b:
- Pinned toolchain `1.99.0`. The egui family is `=0.36.2`. GPL-3.0-or-later. These pass: `cargo deny check`, and `mkdir -p target/licenses && cargo about generate -m crates/app/Cargo.toml about.hbs -o target/licenses/THIRD_PARTY_LICENSES.html`.
- Never use the name "CLO" in the product.
- Every user-visible string goes through `tr!` (`crates/app/i18n/en-US/opendrape.ftl`). The `fl!` macro checks message ids when it compiles, so add a task's strings before its code.
- **Never launch the GUI app, `--smoke-test`, or anything that opens a window or dialog on the user's Mac.** Use headless unit and egui_kittest tests only. The app-level kittest tests use `Harness::builder().wgpu()`, as `crates/app/tests/ui.rs` already does: an offscreen device, no window.
- Commit messages end with exactly `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`.
- Work on branch `m4a-sew-and-drape`, created from `main`. Never push.
- Pattern coordinates are f64 millimetres, y up. Outline edge `i` runs from vertex `i` to vertex `(i+1) % n`.
- Every user change is one undo step. `Document` refuses any change that makes `Project::check()` fail; call sites show `notice-refused` via `note_if_refused()`.
- **Keyboard rules from M2a are unchanged:**
  - shortcuts and tool keys act only when no text field has focus, the number box is closed and no modal is open;
  - redo is checked before undo.

New in M4a (from the spec):
- **Dependencies:**
  - The only new dependency is `spade = "=2.15.1"` (MIT OR Apache-2.0), pinned exactly.
  - The gizmo is OpenDrape's own code: no gizmo crate.
  - The new crate `opendrape-mesh` is pure (no GPU, no windows). It depends on `core`, `geom`, `spade` and `glam`.
- **Do not touch the dress-forms branch's files.** These belong to it:
  - `crates/body/**`, `crates/sim/**`;
  - `crates/testkit/src/garments.rs`, `crates/testkit/src/metrics.rs`;
  - `scripts/forms/**`, `assets/forms/**`, `ASSETS.md`.

  M4a only *uses* their public APIs. Its drape test is a new file, `crates/testkit/tests/project_skirt.rs`.
  - `crates/testkit/Cargo.toml` gains dev-dependencies. If `dress-forms` also edits that file, merge both by hand.
  - When dress-forms Track A merges, the `Collider` trait gains a required `fn signed_distance(&self, p: DVec3) -> f64`. `BodyAndFloor` (Task 7) already has an inherent method of that name, so its trait impl gains three lines then. Track A's `CompoundCollider` (with a floor) replaces `BodyAndFloor` behind the `Stage`.
- **Schema version:** `SCHEMA_VERSION` goes from 2 to 3, with a v2→v3 upgrade and a frozen `crates/io/tests/fixtures/v3/project.json`. v1 and v2 fixtures stay and keep loading. **If dress-forms Track B merges first and takes 3, renumber M4a's version, migration and fixture folder to 4 at merge time.**
- **Seams:**
  - **Model:**
    - `SeamSide { shape, half, first_edge, edges ≥ 1, forward }`. A side covers stored edges `first_edge, first_edge + 1, …` (wrapping). `forward` means it runs the stored way.
    - A seam's `a` start meets its `b` start.
    - At most 2,000 seams (`MAX_SEAMS`).
  - **Mirror images are derived, never stored.** A side's mirror is the same edges on a fold's other half, or on the other member of a pair. A seam whose two sides both have mirrors has a mirror seam, unless that mirror covers the same edges as the seam itself.
  - **`check()` refuses:**
    - a missing shape or edge;
    - 0 edges, or more than the outline has;
    - `Pale` on a piece that isn't folded;
    - a side covering the fold edge;
    - any edge sewn twice (mirror images included);
    - duplicate seam ids.
  - **Edits:**
    - adding a point on a sewn edge grows the side;
    - removing a point shrinks a side, and a side left with 0 edges deletes its seam;
    - deleting a piece or twin deletes its seams;
    - Unfold stores the mirror images on the piece's pale half;
    - Remove fold deletes seams on the pale half;
    - Break pair: mirror images vanish, and the twin's own seams stay.
- **Sew tool:**
  - Key **W** (S is taken by Rectangle).
  - The end of a clicked edge nearer the first click is the side's start. Shift-click extends the side being built at either end.
  - Refusal notices:
    - "This edge is already sewn.";
    - "Shift-click an edge right next to this side, on the same piece.";
    - "Pick a different edge for the other side of the seam."
  - The seam panel shows each side's length, and in amber "Lengths differ by …" when they differ by more than 3 mm (`LENGTH_WARNING_MM`). It has **Flip** (toggles `b.forward`) and **Delete seam**.
- **Fabric:**
  - Target edge `h` = 12 mm (`DEFAULT_EDGE_MM`).
  - A free edge gets `max(1, round(len/h))` steps. Both sides of a seam get `max(1, round(max(lenA, lenB)/h))` steps at equal arc-length fractions, and a corner inside a side takes over its nearest sample.
  - spade's refinement runs with an angle limit of 25°, a maximum area of 0.5 h², `keep_constraint_edges`, and outer faces and holes excluded.
  - The whole garment stays under 30,000 particles (`MAX_PARTICLES`); `h` grows (note "Large pattern: using coarser fabric") until the estimate 1.7·area/h² + perimeter/h fits.
  - A shape that can't be meshed is left out with its seams, with a note.
  - The arranging view uses 20 mm fabric (`VIEW_EDGE_MM`).
- **Placement:**
  - It is in the form's frame: metres, y up from the floor (y = 0), the form faces +z and its left is +x, and its centre line is x = 0, z = 0.
  - `Placement { position: [f64; 3], rotation: [f64; 4] (x, y, z, w; unit within 1e-6), curve: Option<f64> (radius 0.05–2 m) }`. The position is within 10 m of the origin.
  - Applying it centres the flat piece on its bounding-box middle, wraps it round a vertical cylinder whose axis lies behind it, rotates it, then translates it.
  - A piece with no placement starts at real size on a vertical plane 40 cm in front of the centre line, centred left–right, with the pattern's top at shoulder height.
  - A twin with no placement mirrors its piece's (position x → −x, rotation (x, y, z, w) → (x, −y, −z, w)). Its own placement wins once it is moved.
- **Place at:**
  - Angles: front 0, back π, left side +π/2, right side −π/2, measured from the front towards the form's left.
  - The piece keeps its height.
  - The curve radius is the largest surface distance over the piece's heights and covered angles, plus 3 cm (`PLACE_GAP_M`).
  - A member of a pair puts the side that faces its partner on the pattern table on the centre line. **Flat** clears the curve.
- **Gizmo:**
  - Red x, green y and blue z arrows; a centre square that moves in the plane facing the viewer; rings about x, y and z.
  - It stays a constant 80 points across.
  - Shift snaps rings to 15°.
  - A readout shows the move ("12.0 cm up") or the angle ("45°").
  - One drag is one undo step.
- **Typed placement:** the **3D placement** group in Properties has Position X/Y/Z in the student's units and Rotation X/Y/Z in degrees. The rotation is applied about x, then y, then z, all world axes.
- **Draping:**
  - One light cotton, 0.15 kg/m² (`DENSITY_KG_M2`), with M1's `Params::default()`.
  - **Play** takes a snapshot of the project. Meshing and cloth building happen on the simulation thread.
  - **Reset** returns to arranging, and any project change while draped does too.
  - Non-finite values stop the drape, reset it, and show "The drape went wrong and was reset. Check for seams that pull pieces through the form."
  - The floor is a plane at y = 0, in the `Stage`'s `BodyAndFloor`.
- **Determinism:** no `HashMap`/`HashSet` iteration may decide the fabric's output order. spade's own sets only answer membership, and its queues are ordered.

## Review Focus

These are the inputs most likely to bite a student that the spec's own tests don't exercise, most likely first. Each one is pinned by a test in the task that owns the code.
1. **Pressing Play with nothing drawn, or with no seams.** A student will try it first. Expected: the form stays, an empty drape settles at once, and Reset works; no crash, no "went wrong". Tests:
   - Task 7: `play_with_nothing_drawn_is_harmless` (runner) and `play_with_nothing_drawn_shows_just_the_form` (app).
2. **Sewing a piece to its own mirror image** (the centre back of a mirrored pair, or either way round). Expected: accepted, drawn and stitched once, and the skirt drapes as a closed tube. Tests:
   - Task 1: `mirrors_are_derived_for_folds_and_pairs` (flipped too);
   - Task 3: `a_folded_piece_sewn_to_itself_is_one_tube`;
   - Task 5: `a_back_sewn_to_its_own_mirror_image_is_one_seam`;
   - Task 11: the drape test sews it.
3. **Place at… where the form is uneven or missing:** the centre line between the legs, the seat, a piece above the head. Expected: the piece ends up clear of the form, never inside it, never with a NaN. Tests:
   - Task 4: `with_no_form_in_reach_place_at_uses_a_fallback_curve`, `place_at_wraps_a_piece_round_the_form_at_its_height`;
   - Task 10: `place_at_front_wraps_the_piece_round_the_form_as_one_step` (no corner inside the real body).
4. **A gizmo handle seen end-on** (an arrow pointing at the viewer, a ring seen edge-on). Expected: no wild jumps. The arrow can't be grabbed, and a ring seen at a grazing angle (|cos| between the view and its axis under 0.5, as the y ring is in the app's default and view-button cameras) turns with the pointer's movement across it divided by its on-screen radius, so it neither races nor flips (not with the pointer's angle round the centre, as first planned). Tests:
   - Task 4: `an_arrow_pointing_at_the_viewer_cannot_be_dragged`, `a_ring_seen_edge_on_turns_with_the_pointer_across_its_axis`;
   - final fix wave: `a_ring_seen_at_a_grazing_angle_turns_steadily_in_every_view_the_app_gives`, `a_pointer_going_along_a_grazing_ring_through_its_centre_does_not_make_it_flip` (the real app's cameras).
5. **Undo, redo or a deleted piece taking away what the Sew tool or the seam selection points at.** Expected: the selection and a half-made seam are dropped quietly; no panic in painting, the panel or Delete. Tests:
   - Task 5: `two_clicks_make_a_seam_whose_starts_meet` (undo), `undo_and_deletion_drop_a_seam_selection_and_a_half_made_seam`.

---

## File Structure

```
Cargo.toml                                   + crates/mesh member, opendrape-mesh and spade deps, opt-level for mesh
crates/core/src/seam.rs                      SeamId, Half, SeamSide, Seam, MAX_SEAMS
crates/core/src/placement.rs                 Placement and its limits
crates/core/src/project.rs                   seams, mirrors, edit rules, placement access, check, SCHEMA_VERSION 3
crates/core/src/piece.rs                     Piece.placement, Twin.placement, twin shapes carry the twin's
crates/core/src/lib.rs                       exports
crates/geom/src/seams.rs                     Shape::sew_edge/stored_len, side_edges, side_length, side_points
crates/geom/src/lib.rs                       split_edge_in, remove_vertex_in (edits that keep seams)
crates/geom/src/shapes.rs                    an unfolded piece keeps its placement
crates/io/src/lib.rs                         every older version becomes the current one on load
crates/io/tests/fixtures/v3/project.json     new frozen fixture (+ README line)
crates/io/tests/{fixtures.rs,roundtrip.rs}   v3 expectations; seams and placements round-trip
crates/mesh/Cargo.toml                       new crate
crates/mesh/src/lib.rs                       MeshParams, PanelMesh, GarmentMesh, MeshNote, Stitch, build()
crates/mesh/src/triangulate.rs               spade CDT + refinement of one panel
crates/mesh/src/boundary.rs                  a shape's outline sampled, seam sides in step
crates/mesh/src/place.rs                     placements in 3D, start, mirror, Place at…, Euler angles
crates/mesh/tests/fabric.rs                  fabric quality, seams, holes, folds, twins, fuzz
crates/render/src/mesh.rs                    GpuMesh::set_color (+ a render test)
crates/app/Cargo.toml                        + body, mesh; testkit no longer a dependency
crates/app/src/stage.rs                      Stage (centred body, collider, floor, shoulders, surface distance), BodyAndFloor
crates/app/src/sim_runner.rs                 project-driven runner: build_drape, DrapeNote, Play/Reset/went wrong
crates/app/src/arrange/mod.rs                Arranger (click, hover, gizmo press/drag/release, readout)
crates/app/src/arrange/gizmo.rs              ScreenCamera, axis/plane/ring drags, ray_triangle, Gizmo handles
crates/app/src/arrange/scene.rs              ArrangedScene, SceneCache, pick
crates/app/src/arrange/overlay.rs            the gizmo and readout drawn with egui's painter
crates/app/src/viewport.rs                   shows pieces or the drape; returns response + camera
crates/app/src/app.rs                        Play/Pause/Reset, notes, view buttons, arranging, 3D Place at…
crates/app/src/editor/sew_tool.rs            the Sew tool (W)
crates/app/src/editor/seams.rs               seam lines (inset), colours, picking
crates/app/src/editor/placing.rs             Place at…, Flat, typed 3D placement
crates/app/src/editor/{mod.rs,canvas.rs,panel.rs,paint.rs}   Tool::Sew, Selection::Seam, menus, drawing, seam panel
crates/app/src/{lib.rs,main.rs}              modules; no autoplay
crates/app/i18n/en-US/opendrape.ftl          strings
crates/app/tests/{sewing.rs,placing.rs,arrange.rs,ui.rs,details.rs,common/mod.rs}   tests
crates/testkit/Cargo.toml                    dev-deps core, geom, mesh
crates/testkit/tests/project_skirt.rs        the drafted-skirt drape gate
docs/testing/M4a-checklist.md, README.md, docs/specs/2026-10-09-opendrape-design.md, the M4a spec   docs
```

All commands assume `source ~/.cargo/env` and the repo root. Before Task 1: `git checkout -b m4a-sew-and-drape`.

---
### Task 1: Seams in the model, the edits that keep them, and where sides lie on shapes (review: full)

**Files:**
- Create:
  - `crates/core/src/seam.rs`
  - `crates/geom/src/seams.rs`
  - `crates/app/tests/sewing.rs`
- Modify:
  - `crates/core/src/{lib.rs,project.rs}`
  - `crates/geom/src/lib.rs`
  - `crates/app/src/editor/{canvas.rs,panel.rs}` (the edits go through the project)
  - `crates/app/i18n/en-US/opendrape.ftl` (one changed string)
  - `crates/app/tests/details.rs` (that string's constant)

**Interfaces:**
- Consumes: M2b's `Project::{owner, owner_mut, piece, piece_mut, break_twin, remove_piece}`, `Piece::{split_edge_at, remove_vertex}`, `geom::{split_edge, remove_vertex, unfolded, shapes, shape_of, edge_points, edge_length}`, `Shape`/`ShapeKind`.
- Produces (core, exported from `opendrape_core`):
  - `pub const MAX_SEAMS: usize = 2_000;`
  - `SeamId(pub u32)`: Copy, Ord, Hash, serde transparent.
  - `enum Half { Drawn, Pale }`: Default `Drawn`, serde lowercase, with `fn other(self) -> Half`.
  - `SeamSide { shape: PieceId, half: Half, first_edge: usize, edges: usize, forward: bool }`: Copy, Eq, Hash. Methods: `new(shape, half, first_edge, edges, forward)`, `stored_edges(&self, n) -> impl Iterator<Item = usize>`, `covers(&self, n, e) -> bool`, `same_edges(&self, &SeamSide) -> bool`.
  - `Seam { id: SeamId, a: SeamSide, b: SeamSide }`: Copy, Eq. Method: `touches(&self, PieceId) -> bool`.
  - `Project.seams: Vec<Seam>` (`#[serde(default)]`).
  - New `Project` methods:
    - `seam(id) -> Option<&Seam>`, `seam_mut(id) -> Option<&mut Seam>`;
    - `add_seam(a, b) -> SeamId`, `remove_seam(id) -> Option<Seam>`;
    - `mirror_side(&SeamSide) -> Option<SeamSide>`, `mirror_of(&Seam) -> Option<Seam>`;
    - `all_seams() -> Vec<(Seam, bool)>` (true = derived mirror, with its seam's id);
    - `seam_on(shape, half, edge) -> Option<SeamId>`;
    - `seams_after_split(id, i)`, `seams_after_removal(id, i, n_before)`;
    - `unfold_piece(id, full: Piece) -> bool`, `remove_fold(id) -> bool`.
  - Changed: `remove_piece(id)` also deletes the seams on that shape.
  - New `ModelError` variants: `BadSeam(SeamId)` ("seam 7 is invalid") and `TooManySeams`.
- Produces (geom):
  - `Shape::stored_len(&self) -> usize` and `Shape::sew_edge(&self, j) -> (Half, usize, bool)` (half, stored edge, runs against the stored direction).
  - `side_edges(&Shape, &SeamSide) -> Option<Vec<(usize, bool)>>`, `side_length(&Shape, &SeamSide) -> Option<f64>`, `side_points(&Shape, &SeamSide, tolerance) -> Option<Vec<Point2>>`.
  - `split_edge_in(&mut Project, PieceId, i, t) -> Option<usize>` and `remove_vertex_in(&mut Project, PieceId, i) -> bool`.
- Later tasks rely on all of these names.

**Behaviour:**
- **Mirror images:**
  - A fold's mirror side is the same edges on the other half; a pair member's is the same edges on the other member.
  - `mirror_of` is None unless both sides have mirrors.
  - It is also None when the mirror covers the same edges as the seam itself (a back sewn to its own twin, either way round).
- **Edit rules:** see Global Constraints. `seams_after_*` touch sides on the stored piece and on its twin, which share stored edge numbers.

- [ ] **Step 1: Failing tests**

Create `crates/core/src/seam.rs` with only its tests for now:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sides_cover_consecutive_edges_and_wrap() {
        let s = SeamSide::new(PieceId(1), Half::Drawn, 3, 3, true);
        assert_eq!(s.stored_edges(5).collect::<Vec<_>>(), vec![3, 4, 0]);
        assert!(s.covers(5, 4) && s.covers(5, 0) && !s.covers(5, 1) && !s.covers(5, 2));
        let flipped = SeamSide {
            forward: false,
            ..s
        };
        assert!(s.same_edges(&flipped) && s != flipped);
        assert!(!s.same_edges(&SeamSide {
            half: Half::Pale,
            ..s
        }));
        assert_eq!(Half::Pale.other(), Half::Drawn);
    }

    #[test]
    fn seams_serialise_plainly() {
        let seam = Seam {
            id: SeamId(4),
            a: SeamSide::new(PieceId(1), Half::Pale, 2, 1, true),
            b: SeamSide::new(PieceId(3), Half::Drawn, 0, 2, false),
        };
        let json = serde_json::to_string(&seam).unwrap();
        assert_eq!(
            json,
            r#"{"id":4,"a":{"shape":1,"half":"pale","first_edge":2,"edges":1,"forward":true},"b":{"shape":3,"half":"drawn","first_edge":0,"edges":2,"forward":false}}"#
        );
        assert_eq!(serde_json::from_str::<Seam>(&json).unwrap(), seam);
    }
}
```

Append to the `mod tests` of `crates/core/src/project.rs` (it already has `use super::*;`):

```rust
    /// A folded front half (id 1: edges 0 bottom, 1 right, 2 top, 3 the fold on the left), a
    /// back (id 2) paired with its twin (id 3), and a plain pocket (id 4).
    fn sewing_room() -> Project {
        let mut pr = Project::new();
        let mut front = Piece::rectangle(PieceId(0), "Front", Point2::new(0.0, 0.0), 100.0, 200.0);
        front.fold = Some(3);
        pr.add_piece(front);
        let back = pr.add_piece(Piece::rectangle(
            PieceId(0),
            "Back",
            Point2::new(300.0, 0.0),
            100.0,
            200.0,
        ));
        pr.add_twin(back, "Back (mirror)".into(), Point2::new(900.0, 0.0))
            .unwrap();
        pr.add_piece(Piece::rectangle(
            PieceId(0),
            "Pocket",
            Point2::new(0.0, 400.0),
            80.0,
            80.0,
        ));
        assert_eq!(pr.check(), Ok(()));
        pr
    }

    fn side(shape: u32, half: Half, first_edge: usize, edges: usize, forward: bool) -> SeamSide {
        SeamSide::new(PieceId(shape), half, first_edge, edges, forward)
    }

    #[test]
    fn mirrors_are_derived_for_folds_and_pairs() {
        let mut pr = sewing_room();
        // Front's right edge to the back's left edge (edge 3 of a rectangle).
        let side_seam = pr.add_seam(
            side(1, Half::Drawn, 1, 1, true),
            side(2, Half::Drawn, 3, 1, false),
        );
        // The back's right edge to its twin's: its own mirror image.
        let centre_back = pr.add_seam(
            side(2, Half::Drawn, 1, 1, true),
            side(3, Half::Drawn, 1, 1, true),
        );
        // The pocket has no mirror image, so neither has its seam.
        let pocket = pr.add_seam(
            side(4, Half::Drawn, 0, 1, true),
            side(1, Half::Drawn, 0, 1, true),
        );
        assert_eq!(
            (side_seam, centre_back, pocket),
            (SeamId(1), SeamId(2), SeamId(3))
        );
        assert_eq!(pr.check(), Ok(()));
        let all = pr.all_seams();
        assert_eq!(all.len(), 4);
        assert_eq!(
            all[1],
            (
                Seam {
                    id: side_seam,
                    a: side(1, Half::Pale, 1, 1, true),
                    b: side(3, Half::Drawn, 3, 1, false)
                },
                true
            )
        );
        assert_eq!(
            (all[2].0.id, all[2].1, all[3].0.id),
            (centre_back, false, pocket)
        );
        // Sewn the other way round, the centre back is still its own mirror image.
        pr.seam_mut(centre_back).unwrap().b.forward = false;
        assert_eq!(pr.all_seams().len(), 4);
        assert_eq!(pr.check(), Ok(()));
        assert_eq!(
            pr.seam_on(PieceId(3), Half::Drawn, 3),
            Some(side_seam),
            "a mirror image belongs to its seam"
        );
        assert_eq!(pr.seam_on(PieceId(1), Half::Pale, 1), Some(side_seam));
        assert_eq!(pr.seam_on(PieceId(1), Half::Pale, 0), None);
    }

    #[test]
    fn seams_are_checked() {
        let base = sewing_room();
        let bad = |a: SeamSide, b: SeamSide| {
            let mut pr = base.clone();
            pr.add_seam(a, b);
            pr.check()
        };
        let ok = side(4, Half::Drawn, 0, 1, true);
        assert_eq!(
            bad(side(9, Half::Drawn, 0, 1, true), ok),
            Err(ModelError::BadSeam(SeamId(1))),
            "no such shape"
        );
        assert_eq!(
            bad(side(2, Half::Drawn, 0, 0, true), ok),
            Err(ModelError::BadSeam(SeamId(1))),
            "no edges"
        );
        assert_eq!(
            bad(side(2, Half::Drawn, 0, 5, true), ok),
            Err(ModelError::BadSeam(SeamId(1))),
            "more than the outline"
        );
        assert_eq!(
            bad(side(2, Half::Drawn, 4, 1, true), ok),
            Err(ModelError::BadSeam(SeamId(1))),
            "no such edge"
        );
        assert_eq!(
            bad(side(2, Half::Pale, 0, 1, true), ok),
            Err(ModelError::BadSeam(SeamId(1))),
            "pale half of an unfolded piece"
        );
        assert_eq!(
            bad(side(1, Half::Drawn, 2, 2, true), ok),
            Err(ModelError::BadSeam(SeamId(1))),
            "across the fold"
        );
        assert_eq!(
            bad(side(4, Half::Drawn, 3, 2, true), ok),
            Err(ModelError::BadSeam(SeamId(1))),
            "edge 0 twice"
        );
        // The first seam's mirror image already sews the pale half's edge 1: the second seam,
        // which sews it again, is the one refused.
        let mut pr = base.clone();
        pr.add_seam(
            side(1, Half::Drawn, 1, 1, true),
            side(2, Half::Drawn, 3, 1, false),
        );
        pr.add_seam(
            side(1, Half::Pale, 1, 1, true),
            side(4, Half::Drawn, 0, 1, true),
        );
        assert_eq!(pr.check(), Err(ModelError::BadSeam(SeamId(2))));
        let mut twice = base.clone();
        twice.add_seam(
            side(4, Half::Drawn, 0, 1, true),
            side(4, Half::Drawn, 2, 1, true),
        );
        twice.seams.push(Seam {
            id: SeamId(1),
            ..twice.seams[0]
        });
        twice.seams[1].a.first_edge = 1;
        twice.seams[1].b.first_edge = 3;
        assert_eq!(
            twice.check(),
            Err(ModelError::BadSeam(SeamId(1))),
            "an id used twice"
        );
        let mut many = base.clone();
        for k in 0..=MAX_SEAMS {
            many.seams.push(Seam {
                id: SeamId(k as u32 + 1),
                a: side(4, Half::Drawn, 0, 1, true),
                b: side(4, Half::Drawn, 1, 1, true),
            });
        }
        assert_eq!(many.check(), Err(ModelError::TooManySeams));
        assert_eq!(
            ModelError::BadSeam(SeamId(7)).to_string(),
            "seam 7 is invalid"
        );
    }

    #[test]
    fn splitting_a_sewn_edge_keeps_both_parts_sewn() {
        let mut pr = sewing_room();
        // The back's edges 3 and 0 (wrapping) to the pocket's edge 1; the twin's edge 2 to the
        // pocket's edge 2.
        let wrap = pr.add_seam(
            side(2, Half::Drawn, 3, 2, true),
            side(4, Half::Drawn, 1, 1, true),
        );
        let twin = pr.add_seam(
            side(3, Half::Drawn, 2, 1, false),
            side(4, Half::Drawn, 2, 1, true),
        );
        let back = pr.piece_mut(PieceId(2)).unwrap();
        back.split_edge_at(
            0,
            crate::Vertex::corner(Point2::new(350.0, 0.0)),
            crate::Edge::Line,
            crate::Edge::Line,
            50.0,
        );
        pr.seams_after_split(PieceId(2), 0);
        assert_eq!(
            pr.seam(wrap).unwrap().a,
            side(2, Half::Drawn, 4, 3, true),
            "edges 4, 0 and 1 now"
        );
        assert_eq!(
            pr.seam(twin).unwrap().a,
            side(3, Half::Drawn, 3, 1, false),
            "moved up, not grown"
        );
        assert_eq!(pr.check(), Ok(()));
    }

    #[test]
    fn removing_points_shrinks_sides_and_drops_empty_seams() {
        let corners: Vec<Point2> = (0..6)
            .map(|k| {
                let a = k as f64 / 6.0 * std::f64::consts::TAU;
                Point2::new(100.0 * a.cos(), 100.0 * a.sin())
            })
            .collect();
        let mut pr = Project::new();
        let hex = pr.add_piece(Piece::polygon(PieceId(0), "Hex", &corners));
        pr.add_piece(Piece::polygon(PieceId(0), "Other", &corners));
        let s1 = pr.add_seam(
            side(1, Half::Drawn, 0, 3, true),
            side(2, Half::Drawn, 0, 1, true),
        );
        let s2 = pr.add_seam(
            side(1, Half::Drawn, 3, 1, true),
            side(2, Half::Drawn, 3, 1, true),
        );
        let s3 = pr.add_seam(
            side(1, Half::Drawn, 4, 1, false),
            side(2, Half::Drawn, 4, 1, true),
        );
        let remove = |pr: &mut Project, i: usize| {
            let n = pr.piece(hex).unwrap().len();
            assert!(pr.piece_mut(hex).unwrap().remove_vertex(i, 1.0));
            pr.seams_after_removal(hex, i, n);
        };
        // Vertex 1 lies between edges 0 and 1 of the first seam's side: it shrinks by one.
        remove(&mut pr, 1);
        assert_eq!(pr.seam(s1).unwrap().a, side(1, Half::Drawn, 0, 2, true));
        assert_eq!(pr.seam(s2).unwrap().a, side(1, Half::Drawn, 2, 1, true));
        assert_eq!(pr.seam(s3).unwrap().a, side(1, Half::Drawn, 3, 1, false));
        // Vertex 2 ends the first side (edge 1) and starts the second (edge 2): both lose an
        // edge, and the second, left with none, goes.
        remove(&mut pr, 2);
        assert_eq!(pr.seam(s1).unwrap().a, side(1, Half::Drawn, 0, 1, true));
        assert!(pr.seam(s2).is_none());
        assert_eq!(pr.seam(s3).unwrap().a, side(1, Half::Drawn, 2, 1, false));
        assert_eq!(pr.check(), Ok(()));
    }

    #[test]
    fn removing_a_fold_end_drops_the_pale_seams() {
        let mut pr = sewing_room();
        let drawn = pr.add_seam(
            side(1, Half::Drawn, 1, 1, true),
            side(4, Half::Drawn, 0, 1, true),
        );
        let pale = pr.add_seam(
            side(1, Half::Pale, 0, 1, true),
            side(4, Half::Drawn, 2, 1, true),
        );
        let front = pr.piece_mut(PieceId(1)).unwrap();
        front.split_edge_at(
            1,
            crate::Vertex::corner(Point2::new(100.0, 100.0)),
            crate::Edge::Line,
            crate::Edge::Line,
            100.0,
        );
        pr.seams_after_split(PieceId(1), 1);
        assert_eq!(pr.seam(drawn).unwrap().a, side(1, Half::Drawn, 1, 2, true));
        // Vertex 4 (0,200) is an end of the fold edge: the fold goes, and the pale seam too.
        assert!(pr.piece_mut(PieceId(1)).unwrap().remove_vertex(4, 100.0));
        assert_eq!(pr.piece(PieceId(1)).unwrap().fold, None);
        pr.seams_after_removal(PieceId(1), 4, 5);
        assert!(pr.seam(pale).is_none());
        assert!(pr.seam(drawn).is_some());
        assert_eq!(pr.check(), Ok(()));
    }

    #[test]
    fn deleting_a_piece_or_twin_deletes_its_seams() {
        let mut pr = sewing_room();
        pr.add_seam(
            side(3, Half::Drawn, 0, 1, true),
            side(4, Half::Drawn, 0, 1, true),
        );
        pr.add_seam(
            side(2, Half::Drawn, 0, 1, true),
            side(4, Half::Drawn, 1, 1, true),
        );
        let kept = pr.add_seam(
            side(1, Half::Drawn, 0, 1, true),
            side(4, Half::Drawn, 2, 1, true),
        );
        let mut no_twin = pr.clone();
        no_twin.remove_piece(PieceId(3));
        assert_eq!(no_twin.seams.len(), 2);
        assert_eq!(no_twin.check(), Ok(()));
        // Deleting the back keeps its twin as a piece of its own, with the twin's seam.
        pr.remove_piece(PieceId(2));
        assert_eq!(
            pr.seams.iter().map(|s| s.a.shape).collect::<Vec<_>>(),
            vec![PieceId(3), PieceId(1)]
        );
        assert!(pr.seam(kept).is_some());
        assert_eq!(pr.check(), Ok(()));
    }

    #[test]
    fn unfolding_keeps_seams_and_stores_their_mirror_images() {
        let mut pr = sewing_room();
        let side_seam = pr.add_seam(
            side(1, Half::Drawn, 1, 1, true),
            side(2, Half::Drawn, 3, 1, false),
        );
        let pale = pr.add_seam(
            side(1, Half::Pale, 0, 1, true),
            side(4, Half::Drawn, 0, 2, true),
        );
        assert_eq!(pr.check(), Ok(()));
        // The whole front: (0,0) (100,0) (100,200) (0,200) (-100,200) (-100,0).
        let full = Piece::polygon(
            PieceId(1),
            "Front",
            &[
                Point2::new(0.0, 0.0),
                Point2::new(100.0, 0.0),
                Point2::new(100.0, 200.0),
                Point2::new(0.0, 200.0),
                Point2::new(-100.0, 200.0),
                Point2::new(-100.0, 0.0),
            ],
        );
        assert!(pr.unfold_piece(PieceId(1), full));
        assert_eq!(pr.piece(PieceId(1)).unwrap().fold, None);
        assert_eq!(
            pr.seam(side_seam).unwrap().a,
            side(1, Half::Drawn, 1, 1, true)
        );
        // The pale image of edge 0 is the whole piece's edge 5, which runs the other way.
        assert_eq!(pr.seam(pale).unwrap().a, side(1, Half::Drawn, 5, 1, false));
        // The side seam's mirror image is a seam of its own now, on edge 4.
        let stored = *pr.seams.last().unwrap();
        assert_eq!(
            (stored.id, stored.a, stored.b),
            (
                SeamId(3),
                side(1, Half::Drawn, 4, 1, false),
                side(3, Half::Drawn, 3, 1, false)
            )
        );
        assert_eq!(pr.all_seams().len(), 3, "and no longer derived");
        assert_eq!(pr.check(), Ok(()));
        let pocket = Piece::rectangle(PieceId(4), "x", Point2::new(0.0, 0.0), 1.0, 1.0);
        assert!(!pr.unfold_piece(PieceId(4), pocket), "not folded");
    }

    #[test]
    fn removing_a_fold_or_breaking_a_pair_drops_what_no_longer_exists() {
        let mut pr = sewing_room();
        let drawn = pr.add_seam(
            side(1, Half::Drawn, 1, 1, true),
            side(2, Half::Drawn, 3, 1, false),
        );
        let pale = pr.add_seam(
            side(1, Half::Pale, 0, 1, true),
            side(4, Half::Drawn, 0, 1, true),
        );
        let on_twin = pr.add_seam(
            side(3, Half::Drawn, 0, 1, true),
            side(4, Half::Drawn, 1, 1, true),
        );
        let mut unfolded = pr.clone();
        assert!(unfolded.remove_fold(PieceId(1)));
        assert!(unfolded.seam(pale).is_none() && unfolded.seam(drawn).is_some());
        assert_eq!(
            unfolded.all_seams().len(),
            2,
            "no mirror images without the fold"
        );
        assert_eq!(unfolded.check(), Ok(()));
        assert!(!unfolded.remove_fold(PieceId(1)));
        assert_eq!(pr.break_twin(PieceId(2)), Some(PieceId(3)));
        assert!(
            pr.seam(on_twin).is_some(),
            "the twin is a piece now, with its seam"
        );
        assert_eq!(
            pr.all_seams().len(),
            3,
            "without the pair, the side seam has no mirror image"
        );
        assert_eq!(pr.check(), Ok(()));
    }
```

Run: `cargo nextest run -p opendrape-core`
Expected: compile errors (`seam` is not a module yet; no `SeamSide`, `add_seam`, …).

- [ ] **Step 2: The seam types** (`crates/core/src/seam.rs`, above the tests)

```rust
//! Seams: which outline edges are sewn to which. Only the seams the student sewed are stored;
//! the mirror image of a seam on a cut-on-fold piece or a mirrored pair is worked out when it
//! is needed (see `Project::mirror_of`).

use crate::PieceId;
use serde::{Deserialize, Serialize};

/// Most seams a project may hold (their mirror images are not counted).
pub const MAX_SEAMS: usize = 2_000;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct SeamId(pub u32);

/// Which half of a cut-on-fold piece a seam side is on.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Half {
    /// The stored half; also every piece that isn't folded, and every twin.
    #[default]
    Drawn,
    /// The pale mirror image across the fold.
    Pale,
}

impl Half {
    pub fn other(self) -> Self {
        match self {
            Self::Drawn => Self::Pale,
            Self::Pale => Self::Drawn,
        }
    }
}

/// One side of a seam: one or more consecutive outline edges of one shape.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SeamSide {
    /// A piece's id or a twin's id.
    pub shape: PieceId,
    #[serde(default)]
    pub half: Half,
    /// The lowest of its edges in the stored outline's order: the side covers stored edges
    /// `first_edge`, `first_edge + 1`, … `first_edge + edges - 1`, wrapping past the last edge.
    pub first_edge: usize,
    /// How many consecutive edges; at least 1.
    pub edges: usize,
    /// True when the side runs the way the stored outline does (it starts at the start of
    /// `first_edge`); false when it runs the other way (it starts at the end of its last edge).
    pub forward: bool,
}

impl SeamSide {
    pub fn new(shape: PieceId, half: Half, first_edge: usize, edges: usize, forward: bool) -> Self {
        Self {
            shape,
            half,
            first_edge,
            edges,
            forward,
        }
    }
    /// The stored edges it covers, lowest first (wrapping), on an outline of `n` edges.
    pub fn stored_edges(&self, n: usize) -> impl Iterator<Item = usize> + '_ {
        let n = n.max(1);
        (0..self.edges).map(move |k| (self.first_edge + k) % n)
    }
    /// Whether it covers stored edge `e` of an outline of `n` edges.
    pub fn covers(&self, n: usize, e: usize) -> bool {
        n > 0 && (e % n + n - self.first_edge % n) % n < self.edges
    }
    /// The same shape, half and edges, whichever way the two run.
    pub fn same_edges(&self, other: &SeamSide) -> bool {
        (self.shape, self.half, self.first_edge, self.edges)
            == (other.shape, other.half, other.first_edge, other.edges)
    }
}

/// Two sides sewn together, matched end to end: `a`'s start meets `b`'s start.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Seam {
    pub id: SeamId,
    pub a: SeamSide,
    pub b: SeamSide,
}

impl Seam {
    /// Whether either side is on shape `id`.
    pub fn touches(&self, id: PieceId) -> bool {
        self.a.shape == id || self.b.shape == id
    }
}
```

In `crates/core/src/lib.rs`, add `mod seam;` after `mod project;`. After the `pub use project::…;` line, add:

```rust
pub use seam::{Half, MAX_SEAMS, Seam, SeamId, SeamSide};
```

- [ ] **Step 3: Seams in the project** (`crates/core/src/project.rs`)

Replace the first line with:

```rust
use crate::{Half, MAX_SEAMS, Piece, PieceId, Point2, Seam, SeamId, SeamSide, Side, Units};
```

In `pub struct Project`, after `pub pieces: Vec<Piece>,`:

```rust
    /// The seams the student sewed. Their mirror images are not stored (see [`Self::mirror_of`]).
    #[serde(default)]
    pub seams: Vec<Seam>,
```

In `ModelError`, after `BadFold(PieceId),` add `BadSeam(SeamId),` and `TooManySeams,`. In its `Display`, after the `BadFold` arm:

```rust
            Self::BadSeam(id) => write!(f, "seam {} is invalid", id.0),
            Self::TooManySeams => write!(f, "the project has too many seams"),
```

In `Project::new()`, after `pieces: Vec::new(),` add `seams: Vec::new(),`.

Replace the doc comment and first line of `remove_piece`'s body:

```rust
    /// Removes the piece or twin with this id and returns its shape, with every seam sewn to it.
    /// Removing a piece that has a twin keeps the twin (and its seams), as an ordinary piece.
    pub fn remove_piece(&mut self, id: PieceId) -> Option<Piece> {
        self.owner(id)?;
        self.seams.retain(|s| !s.touches(id));
        match self.owner(id)? {
```

(The rest of `remove_piece` is unchanged.) Add these methods before `pub fn next_piece_name`:

```rust
    /// The stored seam with this id.
    pub fn seam(&self, id: SeamId) -> Option<&Seam> {
        self.seams.iter().find(|s| s.id == id)
    }

    pub fn seam_mut(&mut self, id: SeamId) -> Option<&mut Seam> {
        self.seams.iter_mut().find(|s| s.id == id)
    }

    /// Sews `a` to `b` (a's start meets b's start) under a fresh id, and returns the id. Ids are
    /// one more than the highest in use, so an id freed by deleting the last seam may come back.
    pub fn add_seam(&mut self, a: SeamSide, b: SeamSide) -> SeamId {
        let id = SeamId(self.seams.iter().map(|s| s.id.0).max().unwrap_or(0) + 1);
        self.seams.push(Seam { id, a, b });
        id
    }

    pub fn remove_seam(&mut self, id: SeamId) -> Option<Seam> {
        let at = self.seams.iter().position(|s| s.id == id)?;
        Some(self.seams.remove(at))
    }

    /// The mirror image of a seam side: the same edges on the other half of a cut-on-fold
    /// piece, or on the other member of a mirrored pair. None for any other piece.
    pub fn mirror_side(&self, side: &SeamSide) -> Option<SeamSide> {
        let (piece, owner) = self.owner(side.shape)?;
        if piece.fold.is_some() {
            Some(SeamSide {
                half: side.half.other(),
                ..*side
            })
        } else if let Some(t) = &piece.twin {
            let shape = match owner {
                Side::Master => t.id,
                Side::Twin => piece.id,
            };
            Some(SeamSide { shape, ..*side })
        } else {
            None
        }
    }

    /// The derived mirror image of `seam`, when both of its sides have one. A seam that is its
    /// own mirror image (a centre-back seam joining a piece to its twin, say) has none: its
    /// mirror would sew the very same edges.
    pub fn mirror_of(&self, seam: &Seam) -> Option<Seam> {
        let a = self.mirror_side(&seam.a)?;
        let b = self.mirror_side(&seam.b)?;
        if a.same_edges(&seam.b) && b.same_edges(&seam.a) {
            return None;
        }
        Some(Seam { id: seam.id, a, b })
    }

    /// Every seam to draw, mesh and stitch: each stored seam, followed by its mirror image when
    /// it has one (`true` marks a mirror image; it has its seam's id).
    pub fn all_seams(&self) -> Vec<(Seam, bool)> {
        let mut out = Vec::with_capacity(self.seams.len() * 2);
        for s in &self.seams {
            out.push((*s, false));
            if let Some(m) = self.mirror_of(s) {
                out.push((m, true));
            }
        }
        out
    }

    /// The seam (stored, or the stored seam whose mirror image it is) that sews stored edge
    /// `edge` of `half` of shape `shape`.
    pub fn seam_on(&self, shape: PieceId, half: Half, edge: usize) -> Option<SeamId> {
        let n = self.owner(shape)?.0.len();
        self.all_seams().into_iter().find_map(|(s, _)| {
            [s.a, s.b]
                .iter()
                .any(|side| side.shape == shape && side.half == half && side.covers(n, edge))
                .then_some(s.id)
        })
    }

    /// Keeps the seams right after stored edge `i` of piece `id` was split in two (the new
    /// edge is `i + 1`): a side on the piece or its twin that covers edge `i` now covers both
    /// parts, and every later edge number moves up by one.
    pub fn seams_after_split(&mut self, id: PieceId, i: usize) {
        let Some(piece) = self.piece(id) else { return };
        let n = piece.len() - 1; // edges before the split
        let shapes = [Some(id), piece.twin.as_ref().map(|t| t.id)];
        for seam in &mut self.seams {
            for side in [&mut seam.a, &mut seam.b] {
                if !shapes.contains(&Some(side.shape)) {
                    continue;
                }
                let had = side.covers(n, i);
                if side.first_edge > i {
                    side.first_edge += 1;
                }
                if had {
                    side.edges += 1;
                }
            }
        }
    }

    /// Keeps the seams right after vertex `i` of piece `id` was removed from an outline of
    /// `n` edges (its edges `i - 1` and `i` became one). A side covering both loses one edge; a
    /// side ending at the vertex loses its edge there, and a side left with none is deleted
    /// with its seam. If the piece lost its fold (the vertex was an end of the fold edge),
    /// every seam on its pale half goes too.
    pub fn seams_after_removal(&mut self, id: PieceId, i: usize, n: usize) {
        let Some(piece) = self.piece(id) else { return };
        let folded = piece.fold.is_some();
        let shapes = [Some(id), piece.twin.as_ref().map(|t| t.id)];
        let prev = (i + n - 1) % n;
        let shift = |e: usize| if e > i { e - 1 } else { e };
        self.seams.retain_mut(|seam| {
            let mut keep = true;
            for side in [&mut seam.a, &mut seam.b] {
                if !shapes.contains(&Some(side.shape)) {
                    continue;
                }
                if side.half == Half::Pale && !folded {
                    keep = false;
                    continue;
                }
                let (has_prev, has_i) = (side.covers(n, prev), side.covers(n, i));
                let first = match (has_prev, has_i) {
                    // A side round the whole outline starting at edge i: start at the join.
                    (true, true) if side.first_edge == i => prev,
                    // A side starting at edge i loses it: it starts at the next edge.
                    (false, true) => (i + 1) % n,
                    _ => side.first_edge,
                };
                if has_prev || has_i {
                    side.edges -= 1;
                }
                side.first_edge = shift(first);
                keep &= side.edges > 0;
            }
            keep
        });
    }

    /// Unfolds cut-on-fold piece `id` into `full`, its whole outline (`geom::unfolded`), and
    /// keeps its seams: the mirror images of seams on the piece become stored seams (on the
    /// pale half they were drawn on), and every side on the piece is renumbered for the whole
    /// outline. False (and nothing changed) when there is no such folded piece.
    pub fn unfold_piece(&mut self, id: PieceId, full: Piece) -> bool {
        let Some(piece) = self.piece(id) else {
            return false;
        };
        let Some(fold) = piece.fold else {
            return false;
        };
        let n = piece.len();
        let first = (fold + 1) % n;
        let mirrors: Vec<(SeamSide, SeamSide)> = self
            .seams
            .iter()
            .filter(|s| s.touches(id))
            .filter_map(|s| self.mirror_of(s))
            .map(|m| (m.a, m.b))
            .collect();
        for (a, b) in mirrors {
            self.add_seam(a, b);
        }
        for seam in &mut self.seams {
            for side in [&mut seam.a, &mut seam.b] {
                if side.shape != id {
                    continue;
                }
                // A side never covers the fold edge, so its edges are consecutive on the drawn
                // half: outline edges `low..low + edges` of the whole piece.
                let low = (side.first_edge + n - first) % n;
                match side.half {
                    Half::Drawn => side.first_edge = low,
                    Half::Pale => {
                        // The pale image of drawn edge m is edge 2n - 3 - m, running backwards.
                        side.first_edge = 2 * n - 3 - (low + side.edges - 1);
                        side.forward = !side.forward;
                        side.half = Half::Drawn;
                    }
                }
            }
        }
        if let Some(stored) = self.piece_mut(id) {
            *stored = Piece { id, ..full };
        }
        true
    }

    /// Takes the fold off piece `id`: its pale half goes, and so does every seam on it. The
    /// mirror images of its other seams simply disappear. False when it has no fold.
    pub fn remove_fold(&mut self, id: PieceId) -> bool {
        let Some(piece) = self.piece_mut(id) else {
            return false;
        };
        if piece.fold.take().is_none() {
            return false;
        }
        self.seams.retain(|s| {
            ![s.a, s.b]
                .iter()
                .any(|side| side.shape == id && side.half == Half::Pale)
        });
        true
    }
```

At the end of `check`, replace the final `Ok(())` with `self.check_seams()`. Then add after `check`, still inside `impl Project`:

```rust
    /// At most [`MAX_SEAMS`] seams with unique ids; every side on an existing shape and its
    /// edges (1 up to the outline's count, never the fold edge, the pale half only of a folded
    /// piece); and no edge sewn twice, mirror images included.
    fn check_seams(&self) -> Result<(), ModelError> {
        if self.seams.len() > MAX_SEAMS {
            return Err(ModelError::TooManySeams);
        }
        let mut ids = std::collections::BTreeSet::new();
        for s in &self.seams {
            if !ids.insert(s.id) || !self.side_fits(&s.a) || !self.side_fits(&s.b) {
                return Err(ModelError::BadSeam(s.id));
            }
        }
        let mut sewn = std::collections::BTreeSet::new();
        for (s, _) in self.all_seams() {
            for side in [s.a, s.b] {
                let n = self.owner(side.shape).map_or(0, |(p, _)| p.len());
                for e in side.stored_edges(n) {
                    if !sewn.insert((side.shape, side.half == Half::Pale, e)) {
                        return Err(ModelError::BadSeam(s.id));
                    }
                }
            }
        }
        Ok(())
    }

    fn side_fits(&self, side: &SeamSide) -> bool {
        let Some((piece, _)) = self.owner(side.shape) else {
            return false;
        };
        let n = piece.len();
        (1..=n).contains(&side.edges)
            && side.first_edge < n
            && (side.half == Half::Drawn || piece.fold.is_some())
            && piece.fold.is_none_or(|f| !side.covers(n, f))
    }
```

Run: `cargo nextest run -p opendrape-core`
Expected: all pass (the 2 seam tests, the 8 new project tests and every earlier one).

- [ ] **Step 4: Failing geom tests**

Create `crates/geom/src/seams.rs` with only its tests:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::shapes::shapes;
    use opendrape_core::{Piece, PieceId, Project};

    fn p(x: f64, y: f64) -> Point2 {
        Point2::new(x, y)
    }

    fn close(a: Point2, b: Point2) {
        assert!(a.distance(b) < 1e-9, "{a:?} vs {b:?}");
    }

    /// A front half folded on its left edge (id 1), and a back (id 2) with its twin (id 3).
    fn project() -> Project {
        let mut pr = Project::new();
        let mut front = Piece::rectangle(PieceId(0), "Front", p(0.0, 0.0), 100.0, 200.0);
        front.fold = Some(3);
        pr.add_piece(front);
        let back = pr.add_piece(Piece::rectangle(
            PieceId(0),
            "Back",
            p(300.0, 0.0),
            100.0,
            200.0,
        ));
        pr.add_twin(back, "Back (mirror)".into(), p(900.0, 0.0))
            .unwrap();
        pr
    }

    #[test]
    fn every_outline_edge_maps_to_a_stored_edge() {
        let all = shapes(&project());
        let front = &all[0];
        // The whole front's edges: 0 bottom, 1 right, 2 top, then the pale images of the top,
        // the right and the bottom, each running backwards.
        let mapped: Vec<_> = (0..6).map(|j| front.sew_edge(j)).collect();
        assert_eq!(
            mapped,
            vec![
                (Half::Drawn, 0, false),
                (Half::Drawn, 1, false),
                (Half::Drawn, 2, false),
                (Half::Pale, 2, true),
                (Half::Pale, 1, true),
                (Half::Pale, 0, true),
            ]
        );
        assert_eq!((front.stored_len(), all[2].stored_len()), (4, 4));
        assert_eq!(
            all[2].sew_edge(3),
            (Half::Drawn, 3, false),
            "a twin keeps its edges' direction"
        );
    }

    #[test]
    fn sides_run_from_their_start() {
        let all = shapes(&project());
        let front = &all[0];
        // Pale edges 0 and 1, run the stored way: from the mirror of (0,0) to the mirror of
        // (100,200), along whole-piece edges 5 then 4, each against its direction.
        let pale = SeamSide::new(PieceId(1), Half::Pale, 0, 2, true);
        assert_eq!(side_edges(front, &pale), Some(vec![(5, true), (4, true)]));
        let pts = side_points(front, &pale, 0.1).unwrap();
        assert_eq!(pts.len(), 3);
        close(pts[0], p(0.0, 0.0));
        close(pts[1], p(-100.0, 0.0));
        close(pts[2], p(-100.0, 200.0));
        assert!((side_length(front, &pale).unwrap() - 300.0).abs() < 1e-9);
        let back_way = SeamSide {
            forward: false,
            ..pale
        };
        assert_eq!(
            side_edges(front, &back_way),
            Some(vec![(4, false), (5, false)])
        );
        // The fold edge (stored 3) can't be part of a side; nor can a pale half elsewhere.
        assert_eq!(
            side_edges(front, &SeamSide::new(PieceId(1), Half::Drawn, 2, 2, true)),
            None
        );
        assert_eq!(
            side_edges(&all[1], &SeamSide::new(PieceId(2), Half::Pale, 0, 1, true)),
            None
        );
        // A twin side wraps like any other.
        let twin = SeamSide::new(PieceId(3), Half::Drawn, 3, 2, false);
        assert_eq!(side_edges(&all[2], &twin), Some(vec![(0, true), (3, true)]));
    }
}
```

In `crates/geom/src/lib.rs`:
- change the core import to `use opendrape_core::{Edge, Piece, PieceId, Point2, Project, Vertex};`;
- add `mod seams;` after `mod marks;`;
- add `pub use seams::{side_edges, side_length, side_points};` before the `pub use shapes::…` line;
- append to its `mod tests`:

```rust
    #[test]
    fn edits_in_a_project_keep_its_seams() {
        let mut pr = Project::new();
        let a = pr.add_piece(square());
        let b = pr.add_piece(square());
        let side = |shape, first_edge, edges| {
            opendrape_core::SeamSide::new(
                shape,
                opendrape_core::Half::Drawn,
                first_edge,
                edges,
                true,
            )
        };
        let seam = pr.add_seam(side(a, 1, 1), side(b, 3, 1));
        assert_eq!(split_edge_in(&mut pr, a, 1, 0.5), Some(2));
        assert_eq!(pr.seam(seam).unwrap().a, side(a, 1, 2));
        assert_eq!(split_edge_in(&mut pr, PieceId(9), 0, 0.5), None);
        assert!(remove_vertex_in(&mut pr, a, 2));
        assert_eq!(pr.seam(seam).unwrap().a, side(a, 1, 1));
        assert_eq!(pr.check(), Ok(()));
        assert!(!remove_vertex_in(&mut pr, PieceId(9), 0));
    }
```

Run: `cargo nextest run -p opendrape-geom`
Expected: compile errors (`side_edges`, `sew_edge`, `split_edge_in` don't exist yet).

- [ ] **Step 5: Implement** (`crates/geom/src/seams.rs` above its tests, and `lib.rs`)

```rust
//! Where seam sides lie on the shapes that show them: which outline edges, which way, how
//! long, and as points to draw.

use crate::shapes::{Shape, ShapeKind};
use crate::{edge_length, edge_points};
use opendrape_core::{Half, Point2, SeamSide};

impl Shape {
    /// How many edges the stored piece behind this shape has (for a fold, its half's).
    pub fn stored_len(&self) -> usize {
        match self.kind {
            ShapeKind::Folded { drawn, .. } => drawn,
            ShapeKind::Plain | ShapeKind::Twin { .. } => self.piece.len(),
        }
    }
    /// The stored edge outline edge `j` shows: its half, its index, and whether the outline
    /// edge runs against the stored edge's direction (only on the pale half of a fold). Every
    /// outline edge has one; a fold's own edge is inside the shape, not on its outline.
    pub fn sew_edge(&self, j: usize) -> (Half, usize, bool) {
        match self.kind {
            ShapeKind::Folded { first, drawn, .. } if j + 1 >= drawn => {
                // Pale edge j is the mirror image of drawn edge 2n - 3 - j.
                let m = 2 * drawn - 3 - j;
                (Half::Pale, (first + m) % drawn, true)
            }
            ShapeKind::Folded { first, drawn, .. } => (Half::Drawn, (first + j) % drawn, false),
            ShapeKind::Plain | ShapeKind::Twin { .. } => (Half::Drawn, j, false),
        }
    }
}

/// The outline edges of `shape` that `side` covers, in the order the side runs, each with
/// whether the side runs against that outline edge's own direction. None when the side does
/// not fit the shape (a pale half on a shape that isn't folded, an edge that doesn't exist,
/// the fold edge).
pub fn side_edges(shape: &Shape, side: &SeamSide) -> Option<Vec<(usize, bool)>> {
    let n = shape.stored_len();
    if side.edges == 0 || side.edges > n || side.first_edge >= n {
        return None;
    }
    let mut runs = Vec::with_capacity(side.edges);
    for i in side.stored_edges(n) {
        let run = match (shape.kind, side.half) {
            (ShapeKind::Folded { first, drawn, .. }, half) => {
                if i == (first + drawn - 1) % drawn {
                    return None; // the fold edge
                }
                let m = (i + drawn - first) % drawn;
                match half {
                    Half::Drawn => (m, false),
                    Half::Pale => (2 * drawn - 3 - m, true),
                }
            }
            (_, Half::Pale) => return None,
            (_, Half::Drawn) => (i, false),
        };
        runs.push(run);
    }
    if !side.forward {
        runs.reverse();
        for run in &mut runs {
            run.1 = !run.1;
        }
    }
    Some(runs)
}

/// Length (mm) of a side along the stitching line.
pub fn side_length(shape: &Shape, side: &SeamSide) -> Option<f64> {
    let runs = side_edges(shape, side)?;
    Some(
        runs.iter()
            .map(|(j, _)| edge_length(&shape.piece, *j))
            .sum(),
    )
}

/// The side as points on the shape no further than `tolerance` mm from it, from its start to
/// its end.
pub fn side_points(shape: &Shape, side: &SeamSide, tolerance: f64) -> Option<Vec<Point2>> {
    let mut out: Vec<Point2> = Vec::new();
    for (j, against) in side_edges(shape, side)? {
        let mut pts = edge_points(&shape.piece, j, tolerance);
        if against {
            pts.reverse();
        }
        // Each edge starts where the one before it ended.
        let skip = usize::from(!out.is_empty());
        out.extend(pts.into_iter().skip(skip));
    }
    Some(out)
}
```

In `crates/geom/src/lib.rs`, after `pub fn remove_vertex`:

```rust
/// [`split_edge`] on stored piece `id` of `project`, keeping its seams sewn: a side on the
/// split edge covers both parts (see `Project::seams_after_split`).
pub fn split_edge_in(project: &mut Project, id: PieceId, i: usize, t: f64) -> Option<usize> {
    let v = split_edge(project.piece_mut(id)?, i, t)?;
    project.seams_after_split(id, i);
    Some(v)
}

/// [`remove_vertex`] on stored piece `id` of `project`, keeping its seams valid (see
/// `Project::seams_after_removal`).
pub fn remove_vertex_in(project: &mut Project, id: PieceId, i: usize) -> bool {
    let Some(piece) = project.piece_mut(id) else {
        return false;
    };
    let n = piece.len();
    if !remove_vertex(piece, i) {
        return false;
    }
    project.seams_after_removal(id, i, n);
    true
}
```

Run: `cargo nextest run -p opendrape-geom`
Expected: all pass.

- [ ] **Step 6: The editor's edits keep seams** (`crates/app`)

`crates/app/src/editor/canvas.rs`, in `delete_selection`, the `Selection::Vertex` arm's edit becomes:

```rust
                let removed = self.doc.edit(|p| {
                    let source = p.owner(id).map(|(piece, _)| piece.id);
                    source.is_some_and(|source| geom::remove_vertex_in(p, source, i))
                });
```

In `add_point_tool`, the split becomes:

```rust
            let split = self.doc.edit(|p| geom::split_edge_in(p, source, i, t));
```

`crates/app/src/editor/panel.rs`: replace `unfold` and `remove_fold` with:

```rust
    /// The whole piece replaces the half; its seams stay sewn (see `Project::unfold_piece`).
    fn unfold(&mut self, source: PieceId) {
        self.doc.edit(|p| {
            if let Some(full) = p.piece(source).map(geom::unfolded) {
                p.unfold_piece(source, full);
            }
        });
        self.note_if_refused();
    }

    /// Seams on the pale half go with it (see `Project::remove_fold`).
    fn remove_fold(&mut self, source: PieceId) {
        self.doc.edit(|p| p.remove_fold(source));
        self.note_if_refused();
    }
```

`crates/app/i18n/en-US/opendrape.ftl`: `notice-fold-refused` now mentions seams (a fold on a sewn edge is refused by `check`):

```
notice-fold-refused = The fold line must be a straight edge with no notches or seams on it, and the whole piece (with its lines) on one side of it.
```

`crates/app/tests/details.rs`: in `const FOLD_REFUSED`, change "with no notches on it" to "with no notches or seams on it".

Create `crates/app/tests/sewing.rs`:

```rust
//! M4a on the pattern table: seams sewn with the Sew tool, how edits keep them, and the seam
//! panel, driven the way a student would.

mod common;
use common::*;
use egui::Key;
use egui_kittest::kittest::Queryable;
use opendrape_core::{Half, Piece, PieceId, Point2, Seam, SeamId, SeamSide};

/// A 150 × 300 mm half piece at (300,100), folded on its left edge (x = 300): stored edges 0
/// bottom, 1 right, 2 top, 3 the fold. Its pale half covers x = 150..300.
fn with_half(h: &mut H) -> PieceId {
    let id = h.state_mut().doc.edit(|p| {
        let mut half =
            Piece::rectangle(PieceId(0), "Front", Point2::new(300.0, 100.0), 150.0, 300.0);
        half.fold = Some(3);
        p.add_piece(half)
    });
    h.run();
    id
}

/// A 300 × 400 mm "Back" at (600,100): edges 0 bottom, 1 right, 2 top, 3 left.
fn with_back(h: &mut H) -> PieceId {
    let id = h.state_mut().doc.edit(|p| {
        p.add_piece(Piece::rectangle(
            PieceId(0),
            "Back",
            Point2::new(600.0, 100.0),
            300.0,
            400.0,
        ))
    });
    h.run();
    id
}

fn side(shape: PieceId, half: Half, first_edge: usize, edges: usize, forward: bool) -> SeamSide {
    SeamSide::new(shape, half, first_edge, edges, forward)
}

fn sew(h: &mut H, a: SeamSide, b: SeamSide) -> SeamId {
    let id = h.state_mut().doc.edit(|p| p.add_seam(a, b));
    h.run();
    id
}

fn seam_of(h: &H, id: SeamId) -> Option<Seam> {
    h.state().doc.project().seam(id).copied()
}

#[test]
fn adding_and_deleting_points_keeps_seams_sewn() {
    let mut h = harness();
    let front = with_rectangle(&mut h); // (100,100)–(400,500)
    let back = with_back(&mut h);
    let seam = sew(
        &mut h,
        side(front, Half::Drawn, 1, 1, true),
        side(back, Half::Drawn, 3, 1, false),
    );
    key(&mut h, Key::X);
    click(&mut h, 401.0, 300.0); // the front's right edge, halfway
    assert_eq!(piece_of(&h, front).len(), 5);
    assert_eq!(
        seam_of(&h, seam).unwrap().a,
        side(front, Half::Drawn, 1, 2, true)
    );
    key(&mut h, Key::Z); // the Edit tool; the new point stays selected
    key(&mut h, Key::Delete);
    assert_eq!(piece_of(&h, front).len(), 4);
    assert_eq!(
        seam_of(&h, seam).unwrap().a,
        side(front, Half::Drawn, 1, 1, true)
    );
}

#[test]
fn unfolding_keeps_the_seams_on_both_halves() {
    let mut h = harness();
    let front = with_half(&mut h);
    let back = with_back(&mut h);
    let seam = sew(
        &mut h,
        side(front, Half::Pale, 1, 1, true),
        side(back, Half::Drawn, 3, 1, false),
    );
    click(&mut h, 375.0, 250.0); // inside the drawn half
    h.get_by_label("Unfold").click();
    h.run();
    assert_eq!(piece_of(&h, front).fold, None);
    // The pale image of the right edge is the whole piece's edge 4, running the other way.
    assert_eq!(
        seam_of(&h, seam).unwrap().a,
        side(front, Half::Drawn, 4, 1, false)
    );
    assert!(
        h.query_by_label("Remove fold").is_none(),
        "the fold is gone"
    );
}

#[test]
fn a_sewn_edge_cannot_become_the_fold() {
    let mut h = harness();
    let front = with_rectangle(&mut h);
    let back = with_back(&mut h);
    sew(
        &mut h,
        side(front, Half::Drawn, 3, 1, true),
        side(back, Half::Drawn, 1, 1, true),
    );
    click(&mut h, 100.0, 300.0); // the front's left edge (edge 3)
    h.get_by_label("Set as fold line").click();
    h.run();
    assert_eq!(piece_of(&h, front).fold, None);
    assert_eq!(
        h.state().notice.as_deref(),
        Some(
            "The fold line must be a straight edge with no notches or seams on it, and the whole piece (with its lines) on one side of it."
        )
    );
}
```

- [ ] **Step 7: Run and see everything pass**

Run: `cargo nextest run --workspace`
Expected: all pass, including the 3 new app tests and the updated `details` fold test.

- [ ] **Step 8: Commit**

```bash
cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings
git add crates/core crates/geom crates/app
git commit -m "feat(core): seams, their mirror images, and edits that keep them sewn

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 2: 3D placements in the model, and project format 3 (review: full)

**Files:**
- Create:
  - `crates/core/src/placement.rs`
  - `crates/io/tests/fixtures/v3/project.json`
- Modify:
  - `crates/core/src/{lib.rs,piece.rs,project.rs}`
  - `crates/geom/src/shapes.rs`
  - `crates/io/src/lib.rs`
  - `crates/io/tests/{fixtures.rs,roundtrip.rs,fixtures/README.md}`

**Interfaces:**
- Consumes: Task 1 (`Project::{seams, add_seam, all_seams}`, `SeamSide`, `Half`, `Seam`, `SeamId`).
- Produces (core, exported):
  - `pub const MIN_CURVE_M: f64 = 0.05; pub const MAX_CURVE_M: f64 = 2.0; pub const MAX_PLACEMENT_M: f64 = 10.0;`
  - `Placement { position: [f64; 3], rotation: [f64; 4] /* x, y, z, w */, curve: Option<f64> }`: Copy, PartialEq, serde. Members: `Placement::NO_ROTATION`, `Placement::at([f64; 3])`, `is_valid(&self) -> bool`.
  - `Piece.placement: Option<Placement>` and `Twin.placement: Option<Placement>`, both `#[serde(default)]`. `Piece::twin_shape()` carries the twin's placement; `reflected()` has none.
  - `Project::placement_of(id) -> Option<Placement>` (the stored one, for a piece or a twin) and `Project::set_placement(id, Option<Placement>) -> bool`.
  - `ModelError::BadPlacement(PieceId)` ("the 3D placement of piece 1 is invalid"), reported with the twin's id for a twin's placement.
  - `SCHEMA_VERSION = 3`.
- Produces (geom): `geom::unfolded` keeps the piece's placement.
- Produces (io): v1 and v2 files load with no seams or placements and `schema_version` 3.

- [ ] **Step 1: Failing tests**

Create `crates/core/src/placement.rs` with only its tests:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn placements_are_checked() {
        let good = Placement {
            position: [0.1, 1.0, 0.3],
            rotation: [0.0, 0.6, 0.0, 0.8],
            curve: Some(0.2),
        };
        assert!(good.is_valid() && Placement::at([0.0, 0.0, 0.0]).is_valid());
        let bad = [
            Placement {
                position: [f64::NAN, 0.0, 0.0],
                ..good
            },
            Placement {
                rotation: [0.0, 0.0, 0.0, 1.001],
                ..good
            },
            Placement {
                rotation: [0.0; 4],
                ..good
            },
            Placement {
                rotation: [0.0, 0.0, f64::INFINITY, 1.0],
                ..good
            },
            Placement {
                curve: Some(0.049),
                ..good
            },
            Placement {
                curve: Some(2.01),
                ..good
            },
            Placement {
                curve: Some(f64::NAN),
                ..good
            },
            Placement {
                position: [8.0, 0.0, 6.1],
                ..good
            },
        ];
        for p in bad {
            assert!(!p.is_valid(), "{p:?}");
        }
        assert!(
            Placement {
                rotation: [0.0, 0.0, 0.0, 1.0 + 9e-7],
                ..good
            }
            .is_valid()
        );
    }

    #[test]
    fn a_flat_placement_saves_without_a_curve() {
        let json = r#"{"position":[0.0,1.2,0.4],"rotation":[0.0,0.0,0.0,1.0]}"#;
        let p: Placement = serde_json::from_str(json).unwrap();
        assert_eq!(p, Placement::at([0.0, 1.2, 0.4]));
    }
}
```

Append to `crates/core/src/piece.rs` `mod tests`:

```rust
    #[test]
    fn placements_are_checked_and_follow_the_twin() {
        let mut s = square();
        let placed = Placement {
            position: [0.1, 1.0, 0.3],
            rotation: [0.0, 0.6, 0.0, 0.8],
            curve: Some(0.2),
        };
        s.placement = Some(placed);
        assert_eq!(s.check(), Ok(()));
        s.placement = Some(Placement {
            curve: Some(0.01),
            ..placed
        });
        assert_eq!(s.check(), Err(ModelError::BadPlacement(PieceId(1))));
        s.placement = Some(placed);
        let mut own = placed;
        own.position[0] = -0.1;
        s.twin = Some(Twin {
            id: PieceId(2),
            name: "B".into(),
            offset: p(300.0, 0.0),
            placement: Some(own),
        });
        assert_eq!(
            s.twin_shape().unwrap().placement,
            Some(own),
            "the twin shape has the twin's"
        );
        assert_eq!(s.reflected(p(0.0, 0.0)).placement, None);
        s.twin.as_mut().unwrap().placement = Some(Placement {
            rotation: [0.0; 4],
            ..own
        });
        assert_eq!(
            s.check(),
            Err(ModelError::BadPlacement(PieceId(2))),
            "named by the twin's id"
        );
    }
```

In the same module, `old_files_get_defaults_for_the_new_fields` now also checks the placement: its last line becomes

```rust
        assert_eq!(
            (piece.fold, piece.twin, piece.placement),
            (None, None, None)
        );
```

and the three `Twin { … }` literals in this test module each gain `placement: None,`.

Append to `crates/core/src/project.rs` `mod tests` (it uses Task 1's `sewing_room`):

```rust
    #[test]
    fn placements_belong_to_pieces_and_twins() {
        let mut pr = sewing_room();
        let p = Placement::at([0.0, 1.0, 0.4]);
        assert!(pr.set_placement(PieceId(2), Some(p)));
        assert!(pr.set_placement(PieceId(3), Some(Placement::at([0.0, 1.0, -0.4]))));
        assert_eq!(pr.placement_of(PieceId(2)), Some(p));
        assert_eq!(
            pr.pieces[1].twin.as_ref().unwrap().placement,
            Some(Placement::at([0.0, 1.0, -0.4]))
        );
        assert_eq!(pr.placement_of(PieceId(1)), None);
        assert!(!pr.set_placement(PieceId(99), Some(p)));
        assert_eq!(pr.check(), Ok(()));
        // Breaking the pair keeps the twin where it was placed.
        pr.break_twin(PieceId(2));
        assert_eq!(
            pr.piece(PieceId(3)).unwrap().placement,
            Some(Placement::at([0.0, 1.0, -0.4]))
        );
        assert_eq!(SCHEMA_VERSION, 3);
    }
```

and delete the line `assert_eq!(SCHEMA_VERSION, 2);` from `check_counts_twins_and_refuses_clashing_ids`.

Run: `cargo nextest run -p opendrape-core`
Expected: compile errors (`placement` module, `Piece::placement`, `set_placement`).

- [ ] **Step 2: Implement core**

`crates/core/src/placement.rs`, above the tests:

```rust
//! Where a piece sits in 3D. Placements are in the form's frame: metres, y up from the floor,
//! the form faces +z and its left is +x, and x = 0, z = 0 is its centre line.

use serde::{Deserialize, Serialize};

/// The tightest curve a piece may be wrapped round (m).
pub const MIN_CURVE_M: f64 = 0.05;
/// The widest curve a piece may be wrapped round (m).
pub const MAX_CURVE_M: f64 = 2.0;
/// The furthest from the origin a piece may be placed (m).
pub const MAX_PLACEMENT_M: f64 = 10.0;

/// A piece's place in 3D. Applying it to the flat piece: centre the piece on the middle of its
/// bounding box; wrap it round a vertical cylinder of radius `curve` whose axis lies behind it
/// (when `curve` is set); turn it by `rotation`; move it to `position`.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Placement {
    /// Where the middle of the piece goes (m).
    pub position: [f64; 3],
    /// A unit quaternion, stored x, y, z, w.
    pub rotation: [f64; 4],
    /// The radius (m) of the cylinder the piece is wrapped round; None for a flat piece.
    #[serde(default)]
    pub curve: Option<f64>,
}

impl Placement {
    /// No turn at all.
    pub const NO_ROTATION: [f64; 4] = [0.0, 0.0, 0.0, 1.0];

    /// Flat and unturned, with its middle at `position`.
    pub fn at(position: [f64; 3]) -> Self {
        Self {
            position,
            rotation: Self::NO_ROTATION,
            curve: None,
        }
    }

    /// Every number finite; the rotation of unit length (within 1e-6); the curve, if any,
    /// between [`MIN_CURVE_M`] and [`MAX_CURVE_M`]; the position within [`MAX_PLACEMENT_M`] of
    /// the origin.
    pub fn is_valid(&self) -> bool {
        let finite = self
            .position
            .iter()
            .chain(&self.rotation)
            .all(|v| v.is_finite());
        let length = self.rotation.iter().map(|v| v * v).sum::<f64>().sqrt();
        let distance = self.position.iter().map(|v| v * v).sum::<f64>().sqrt();
        finite
            && (length - 1.0).abs() <= 1e-6
            && distance <= MAX_PLACEMENT_M
            && self
                .curve
                .is_none_or(|r| r.is_finite() && (MIN_CURVE_M..=MAX_CURVE_M).contains(&r))
    }
}
```

`crates/core/src/lib.rs`: add `mod placement;` after `mod piece;`, and before the `pub use project::…` line:

```rust
pub use placement::{MAX_CURVE_M, MAX_PLACEMENT_M, MIN_CURVE_M, Placement};
```

`crates/core/src/piece.rs`:
- First line: `use crate::{ModelError, Placement};`
- `pub struct Twin` gains, after `offset`:

```rust
    /// The twin's own place in 3D. None: it mirrors its piece's placement across x = 0 (or,
    /// while the piece has none either, starts from its own place on the pattern table).
    #[serde(default)]
    pub placement: Option<Placement>,
```

- `pub struct Piece` gains, after `twin`:

```rust
    /// Where the piece sits in 3D. None: at its starting place, in front of the form.
    #[serde(default)]
    pub placement: Option<Placement>,
```

- `Piece::polygon` and `Piece::reflected` set `placement: None` (after `twin: None`). In `reflected`'s doc comment, "The result has no fold and no twin." becomes "The result has no fold, twin or placement."
- `twin_shape` carries the twin's placement:

```rust
    /// The twin as an ordinary piece (with the twin's id, name and placement), if there is one.
    pub fn twin_shape(&self) -> Option<Piece> {
        let t = self.twin.as_ref()?;
        let mut shape = self.reflected(t.offset);
        shape.id = t.id;
        shape.name = t.name.clone();
        shape.placement = t.placement;
        Some(shape)
    }
```

- In `Piece::check`, after the fold check:

```rust
        if !self.placement.is_none_or(|p| p.is_valid()) {
            return Err(ModelError::BadPlacement(self.id));
        }
```

  In its doc list, replace "- a twin that is itself a valid piece." with "- a valid placement, if it has one;" and "- a twin that is itself a valid piece (with its own placement)."

`crates/core/src/project.rs`:
- Its doc and constant become:

```rust
/// Version of the project format written by this build. Bump it when the format changes, and
/// add a migration step in `opendrape-io`. Version 2 added seam allowances, notches, internal
/// lines, folds and twins; version 3 added seams and 3D placements (2026-10-09).
pub const SCHEMA_VERSION: u32 = 3;
```

- The import line becomes:

```rust
use crate::{
    Half, MAX_SEAMS, Piece, PieceId, Placement, Point2, Seam, SeamId, SeamSide, Side, Units,
};
```

- `ModelError` gains `BadPlacement(PieceId),` (before `BadSeam`), and its `Display` arm:

```rust
            Self::BadPlacement(id) => write!(f, "the 3D placement of piece {} is invalid", id.0),
```

- In `add_twin`, the twin literal gains `placement: None,`.
- Add before `pub fn seam`:

```rust
    /// The placement stored for a piece or twin (None when it has none of its own, or there
    /// is no such shape).
    pub fn placement_of(&self, id: PieceId) -> Option<Placement> {
        match self.owner(id)? {
            (p, Side::Master) => p.placement,
            (p, Side::Twin) => p.twin.as_ref()?.placement,
        }
    }

    /// Gives a piece or twin its own placement, or (None) takes it away. False when there is
    /// no such shape.
    pub fn set_placement(&mut self, id: PieceId, placement: Option<Placement>) -> bool {
        match self.owner_mut(id) {
            Some((p, Side::Master)) => p.placement = placement,
            Some((p, Side::Twin)) => match &mut p.twin {
                Some(t) => t.placement = placement,
                None => return false,
            },
            None => return false,
        }
        true
    }
```

`crates/geom/src/shapes.rs`: in `unfold`, the `full` piece literal gains `placement: piece.placement,` after `twin: None,`.

Run: `cargo nextest run -p opendrape-core -p opendrape-geom`
Expected: all pass.

- [ ] **Step 3: The frozen v3 file**

Create `crates/io/tests/fixtures/v3/project.json`. Never edit it afterwards.

```json
{
  "schema_version": 3,
  "units": "inch",
  "pieces": [
    {
      "id": 1,
      "name": "Skirt front",
      "vertices": [
        { "pos": { "x": 0.0, "y": 0.0 }, "kind": "corner" },
        { "pos": { "x": 300.0, "y": 0.0 }, "kind": "corner" },
        { "pos": { "x": 177.5, "y": 550.0 }, "kind": "corner" },
        { "pos": { "x": 0.0, "y": 550.0 }, "kind": "corner" }
      ],
      "edges": [{ "type": "line" }, { "type": "line" }, { "type": "line" }, { "type": "line" }],
      "grain_deg": 90.0,
      "allowance": 10.0,
      "edge_props": [
        { "allowance": null, "hem": true },
        { "allowance": null, "hem": false },
        { "allowance": null, "hem": false },
        { "allowance": null, "hem": false }
      ],
      "notches": [],
      "lines": [],
      "fold": 3,
      "twin": null,
      "placement": { "position": [0.0, 0.755, 0.214], "rotation": [0.0, 0.0, 0.0, 1.0], "curve": 0.214 }
    },
    {
      "id": 2,
      "name": "Back left",
      "vertices": [
        { "pos": { "x": 400.0, "y": 0.0 }, "kind": "corner" },
        { "pos": { "x": 700.0, "y": 0.0 }, "kind": "corner" },
        { "pos": { "x": 700.0, "y": 550.0 }, "kind": "corner" },
        { "pos": { "x": 522.5, "y": 550.0 }, "kind": "corner" }
      ],
      "edges": [{ "type": "line" }, { "type": "line" }, { "type": "line" }, { "type": "line" }],
      "grain_deg": 90.0,
      "allowance": 10.0,
      "edge_props": [
        { "allowance": null, "hem": true },
        { "allowance": null, "hem": false },
        { "allowance": null, "hem": false },
        { "allowance": null, "hem": false }
      ],
      "notches": [],
      "lines": [],
      "fold": null,
      "twin": {
        "id": 3,
        "name": "Back right",
        "offset": { "x": 1500.0, "y": 0.0 },
        "placement": { "position": [-0.1, 0.755, -0.2], "rotation": [0.0, 1.0, 0.0, 0.0], "curve": null }
      },
      "placement": null
    },
    {
      "id": 4,
      "name": "Pocket",
      "vertices": [
        { "pos": { "x": 0.0, "y": 700.0 }, "kind": "corner" },
        { "pos": { "x": 150.0, "y": 700.0 }, "kind": "corner" },
        { "pos": { "x": 150.0, "y": 850.0 }, "kind": "corner" },
        { "pos": { "x": 0.0, "y": 850.0 }, "kind": "corner" }
      ],
      "edges": [{ "type": "line" }, { "type": "line" }, { "type": "line" }, { "type": "line" }],
      "grain_deg": 90.0,
      "allowance": 10.0,
      "edge_props": [
        { "allowance": null, "hem": false },
        { "allowance": null, "hem": false },
        { "allowance": null, "hem": false },
        { "allowance": null, "hem": false }
      ],
      "notches": [],
      "lines": [],
      "fold": null,
      "twin": null
    }
  ],
  "seams": [
    {
      "id": 1,
      "a": { "shape": 1, "half": "drawn", "first_edge": 1, "edges": 1, "forward": true },
      "b": { "shape": 2, "half": "drawn", "first_edge": 3, "edges": 1, "forward": false }
    },
    {
      "id": 2,
      "a": { "shape": 2, "half": "drawn", "first_edge": 1, "edges": 1, "forward": true },
      "b": { "shape": 3, "half": "drawn", "first_edge": 1, "edges": 1, "forward": true }
    },
    {
      "id": 5,
      "a": { "shape": 4, "half": "drawn", "first_edge": 3, "edges": 2, "forward": true },
      "b": { "shape": 1, "half": "pale", "first_edge": 2, "edges": 1, "forward": false }
    }
  ],
  "next_piece_id": 5
}
```

In `crates/io/tests/fixtures/README.md`, the last line becomes:

```
Folders: v1 (M2a), v2 (M2b: seam allowance, notches, internal lines, fold, twin), v3 (M4a:
seams, 3D placements).
```

- [ ] **Step 4: Failing io tests** (`crates/io/tests/fixtures.rs`)

- The import becomes:

```rust
use opendrape_core::{
    Edge, EdgeProps, Half, InternalLine, LineKind, Notch, NotchStyle, Piece, PieceId, Placement,
    Point2, Project, Seam, SeamId, SeamSide, Twin, Units, Vertex, VertexKind,
};
```

- Every `Piece { … }` literal gains `placement: None,` (four of them), and the `Twin { … }` literal too.
- `format_v1_still_opens`: `assert_eq!(loaded.schema_version, 3, "upgraded on load");`
- `format_v2_still_opens`: its version line becomes these two:

```rust
    assert_eq!(loaded.schema_version, 3, "upgraded on load");
    assert!(loaded.seams.is_empty());
```

- Append:

```rust
#[test]
fn format_v3_still_opens() {
    let loaded = opendrape_io::from_bytes(&odp(include_str!("fixtures/v3/project.json")))
        .expect("the frozen v3 project opens");
    let lines = || vec![Edge::Line; 4];
    let props = |hem: bool| {
        let mut p = vec![EdgeProps::default(); 4];
        p[0].hem = hem;
        p
    };
    let front = Piece {
        id: PieceId(1),
        name: "Skirt front".into(),
        vertices: vec![
            corner(0.0, 0.0),
            corner(300.0, 0.0),
            corner(177.5, 550.0),
            corner(0.0, 550.0),
        ],
        edges: lines(),
        grain_deg: 90.0,
        allowance: 10.0,
        edge_props: props(true),
        notches: vec![],
        lines: vec![],
        fold: Some(3),
        twin: None,
        placement: Some(Placement {
            position: [0.0, 0.755, 0.214],
            rotation: [0.0, 0.0, 0.0, 1.0],
            curve: Some(0.214),
        }),
    };
    let back = Piece {
        id: PieceId(2),
        name: "Back left".into(),
        vertices: vec![
            corner(400.0, 0.0),
            corner(700.0, 0.0),
            corner(700.0, 550.0),
            corner(522.5, 550.0),
        ],
        edges: lines(),
        grain_deg: 90.0,
        allowance: 10.0,
        edge_props: props(true),
        notches: vec![],
        lines: vec![],
        fold: None,
        twin: None, // set below, once the ids are taken
        placement: None,
    };
    let pocket = Piece {
        id: PieceId(4),
        name: "Pocket".into(),
        vertices: vec![
            corner(0.0, 700.0),
            corner(150.0, 700.0),
            corner(150.0, 850.0),
            corner(0.0, 850.0),
        ],
        edges: lines(),
        grain_deg: 90.0,
        allowance: 10.0,
        edge_props: props(false),
        notches: vec![],
        lines: vec![],
        fold: None,
        twin: None,
        placement: None,
    };
    let mut expected = Project::new();
    expected.units = Units::Inch;
    expected.add_piece(front);
    expected.add_piece(back);
    // The twin has id 3: take it with a throwaway piece (the only way to move the counter
    // from outside the model), then give the back its twin.
    let spare = expected.add_piece(Piece::polygon(
        PieceId(0),
        "x",
        &[at(0.0, 0.0), at(1.0, 0.0), at(0.0, 1.0)],
    ));
    expected.remove_piece(spare);
    expected.add_piece(pocket);
    expected.pieces[1].twin = Some(Twin {
        id: PieceId(3),
        name: "Back right".into(),
        offset: at(1500.0, 0.0),
        placement: Some(Placement {
            position: [-0.1, 0.755, -0.2],
            rotation: [0.0, 1.0, 0.0, 0.0],
            curve: None,
        }),
    });
    let side = |shape: u32, half: Half, first_edge: usize, edges: usize, forward: bool| SeamSide {
        shape: PieceId(shape),
        half,
        first_edge,
        edges,
        forward,
    };
    expected.seams = vec![
        Seam {
            id: SeamId(1),
            a: side(1, Half::Drawn, 1, 1, true),
            b: side(2, Half::Drawn, 3, 1, false),
        },
        Seam {
            id: SeamId(2),
            a: side(2, Half::Drawn, 1, 1, true),
            b: side(3, Half::Drawn, 1, 1, true),
        },
        Seam {
            id: SeamId(5),
            a: side(4, Half::Drawn, 3, 2, true),
            b: side(1, Half::Pale, 2, 1, false),
        },
    ];
    assert_eq!(loaded, expected);
    assert_eq!(loaded.schema_version, 3);
    assert_eq!(loaded.next_piece_name("Piece"), "Piece 5");
    // The first seam has a mirror image (front's pale half to the back's twin); the centre
    // back is its own; the pocket has none.
    assert_eq!(loaded.all_seams().len(), 4);
}

#[test]
fn refuses_invalid_v3_details() {
    let good = include_str!("fixtures/v3/project.json");
    for (from, to) in [
        (r#""curve": 0.214"#, r#""curve": 0.01"#), // too tight a curve
        (
            r#""rotation": [0.0, 1.0, 0.0, 0.0]"#,
            r#""rotation": [0.0, 2.0, 0.0, 0.0]"#,
        ), // not unit length
        (
            r#""first_edge": 3, "edges": 1, "forward": false"#,
            r#""first_edge": 3, "edges": 9, "forward": false"#,
        ), // more edges than the outline
        (
            r#""shape": 4, "half": "drawn""#,
            r#""shape": 4, "half": "pale""#,
        ), // the pocket isn't folded
        (
            r#""shape": 1, "half": "pale", "first_edge": 2"#,
            r#""shape": 1, "half": "pale", "first_edge": 3"#,
        ), // the fold edge
        (
            r#""shape": 3, "half": "drawn", "first_edge": 1"#,
            r#""shape": 3, "half": "drawn", "first_edge": 3"#,
        ), // the twin's edge 3 is sewn by seam 1's mirror
    ] {
        assert!(good.contains(from), "{from}");
        let bad = good.replacen(from, to, 1);
        assert!(
            matches!(
                opendrape_io::from_bytes(&odp(&bad)),
                Err(opendrape_io::OdpError::Invalid(_))
            ),
            "{to}"
        );
    }
}
```

`crates/io/tests/roundtrip.rs`: the sample now uses every M4a field too. Replace its header and imports with:

```rust
//! Everything M2b and M4a added to a project (seam allowances, hems, notches, internal lines,
//! folds, mirrored pairs, seams and 3D placements) must survive saving and opening exactly. The
//! project is built field by field here, with no default left where a value could be lost
//! without anyone noticing.

use opendrape_core::{
    Edge, EdgeProps, Half, InternalLine, LineKind, Notch, NotchStyle, Piece, PieceId, Placement,
    Point2, Project, SeamSide, Units, Vertex, VertexKind,
};
```

In `detailed_project()`, replace `project.add_piece(sleeve);` with this, and keep the final check and return:

```rust
    sleeve.placement = Some(Placement {
        position: [0.31, 1.12, -0.05],
        rotation: [0.0, 0.707_106_781_186_547_5, 0.0, 0.707_106_781_186_547_5],
        curve: Some(0.075),
    });
    let sleeve_id = project.add_piece(sleeve);

    // The bodice's right edge to the sleeve's pale half, run backwards; and the bodice's left
    // and bottom edges (wrapping past the last edge) to the sleeve's drawn bottom edge.
    project.add_seam(
        SeamSide::new(bodice_id, Half::Drawn, 1, 1, true),
        SeamSide::new(sleeve_id, Half::Pale, 1, 1, false),
    );
    project.add_seam(
        SeamSide::new(bodice_id, Half::Drawn, 3, 2, true),
        SeamSide::new(sleeve_id, Half::Drawn, 0, 1, true),
    );
    project.set_placement(bodice_id, Some(Placement::at([0.0, 1.25, 0.4])));
    project.set_placement(
        twin,
        Some(Placement {
            position: [-0.12, 1.25, 0.15],
            rotation: [0.0, 0.0, 0.258_819_045_102_520_74, 0.965_925_826_289_068_3],
            curve: None,
        }),
    );
```

Rename `every_m2b_field_survives_a_save_and_open` to `every_m2b_and_m4a_field_survives_a_save_and_open`. At the end of `the_sample_really_uses_every_new_feature`, add:

```rust
    // M4a: seams on both halves, running both ways, one wrapping past the last edge, and
    // placements on a piece, a twin and a curved piece.
    let sides: Vec<SeamSide> = project.seams.iter().flat_map(|s| [s.a, s.b]).collect();
    assert!(sides.iter().any(|s| s.half == Half::Pale) && sides.iter().any(|s| !s.forward));
    assert!(sides.iter().any(|s| s.first_edge + s.edges > 4), "wraps");
    assert!(bodice.placement.is_some() && bodice.twin.as_ref().unwrap().placement.is_some());
    assert!(
        project.pieces[1]
            .placement
            .is_some_and(|p| p.curve.is_some())
    );
    assert_eq!(
        project.all_seams().len(),
        4,
        "both seams have mirror images"
    );
```

Run: `cargo nextest run -p opendrape-io`
Expected: `format_v2_still_opens` fails (a version-2 file still says 2; version 1 is already upgraded by `upgrade_from_v1`); the rest pass.

- [ ] **Step 5: Upgrade older versions on load** (`crates/io/src/lib.rs`)

In `read_from`, after the `if found == 1 { … }` block:

```rust
    // Version 2 (M2b) had no seams or 3D placements; serde's defaults give an older file none.
    // Whatever version it was, it is the current one now.
    project.schema_version = SCHEMA_VERSION;
```

Remove `project.schema_version = SCHEMA_VERSION;` from `upgrade_from_v1`, and end its doc comment at "…which needs the edge count; add those." In `check_version`'s doc, "Versions 1 and 2 parse directly into `Project` (version 1's missing fields take their defaults; …)" becomes "Versions 1 to 3 parse directly into `Project` (the fields older versions lack take their defaults; …)".

- [ ] **Step 6: Run and see everything pass**

Run: `cargo nextest run --workspace`
Expected: all pass.

- [ ] **Step 7: Commit**

```bash
cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings
git add crates/core crates/geom crates/io
git commit -m "feat(core): 3D placements for pieces and twins; project format 3 with seams

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 3: `opendrape-mesh`: pieces into fabric, seams into stitches (review: full)

**Files:**
- Create:
  - `crates/mesh/Cargo.toml`
  - `crates/mesh/src/{lib.rs,triangulate.rs,boundary.rs}`
  - `crates/mesh/tests/fabric.rs`
- Modify: `Cargo.toml` (workspace member, dependencies, dev profile)

**Interfaces:**
- Consumes:
  - Task 1: `Project::all_seams`, `SeamSide`, `geom::{side_edges, side_length}`, `Shape::stored_len`;
  - M2b: `geom::{shapes, edge_length, point_at_distance, outline_points, line_points, area, perimeter}`, `LineKind`.
- Produces (`opendrape_mesh`):
  - Constants: `DEFAULT_EDGE_MM = 12.0`, `MAX_PARTICLES = 30_000`, `LENGTH_WARNING_MM = 3.0`.
  - `MeshParams { edge_mm: f64, max_particles: usize }`, with `Default` giving 12 mm and 30,000.
  - `PanelMesh { shape: PieceId, flat: Vec<[f64; 2]> /* m */, triangles: Vec<[u32; 3]>, edges: Vec<Vec<u32>>, centre: Point2 /* mm */ }`.
  - `pub type Stitch = ((usize, u32), (usize, u32));`
  - `GarmentMesh { panels, stitches: Vec<Stitch>, notes: Vec<MeshNote>, edge_mm: f64 }`, with `particles()` and `panel_of(PieceId) -> Option<usize>`.
  - `MeshNote::{Coarser { edge_mm }, CrossesItself(PieceId), Unmeshable(PieceId), LengthsDiffer { seam: SeamId, by_mm: f64 }}`.
  - `pub fn build(&Project, &MeshParams) -> GarmentMesh`.
  - `pub fn bounds(&[Point2]) -> (Point2, Point2)`.
  - `pub fn triangulate(outline: &[[f64; 2]], holes: &[Vec<[f64; 2]>], h: f64, max_added: usize) -> Result<Triangulated, TriangulateError>`.
  - `Triangulated { points: Vec<[f64; 2]>, triangles: Vec<[u32; 3]> }`; `TriangulateError::{TooFewPoints, NotFinite, CrossesItself}`; `ANGLE_LIMIT_DEG = 25.0`.
- Later tasks rely on:
  - `PanelMesh.centre` being the middle of `geom::outline_points(&shape.piece, 0.5)`'s bounding box (Task 4's `place::centre_of` computes the same);
  - panels coming in `geom::shapes` order;
  - each panel's boundary points coming first, in outline order.

**How the fabric is made** (proven in the probe; see Evidence):
1. **One panel per shape:** the whole outline of a folded piece, a twin separately. Cut-out lines become holes; markings are ignored.
2. **Seams:** every seam and mirror image gets `steps = max(1, round(max(lenA, lenB)/h))`. Each side is sampled at `k·len/steps`. A corner inside a side takes over its nearest sample (no sliver next to a corner). Stored seams whose sides differ by more than 3 mm get a `LengthsDiffer` note.
3. **The outline:** every corner, the samples of the sides on it, and free edges in `max(1, round(len/h))` steps. Positions come from `geom::point_at_distance`, exact along curves.
4. **spade:**
   - inserting a point that is already there, or a constraint that crosses or runs through another, means the outline crosses itself: the panel is left out with a note, and so are its stitches;
   - refinement keeps every boundary point.
5. **Stitches** pair the k-th sample of side a with the k-th of side b, mirror images included.
6. **The particle budget:** the estimate `1.7·area/h² + perimeter/h` (measured: about 1.6 points per h² plus the outline) must fit 30,000; `h` grows until it does.

- [ ] **Step 1: The crate, with failing tests**

`Cargo.toml` (workspace):
- add `"crates/mesh"` to `members` (after `"crates/io"`);
- under `[workspace.dependencies]`, after `opendrape-io = …`, add `opendrape-mesh = { path = "crates/mesh" }`, and after `i_overlay = "9.0.1"` add `spade = "=2.15.1"`;
- after the `[profile.dev.package.opendrape-geom]` block, add:

```toml
# The fabric mesh is rebuilt whenever the pattern changes while the 3D view shows the pieces.
[profile.dev.package.opendrape-mesh]
opt-level = 2
```

Create `crates/mesh/Cargo.toml`:

```toml
[package]
name = "opendrape-mesh"
description = "OpenDrape: pattern pieces into fabric meshes, seams into stitches, and 3D placement"
version.workspace = true
edition.workspace = true
rust-version.workspace = true
license.workspace = true
repository.workspace = true
publish.workspace = true

[dependencies]
opendrape-core.workspace = true
opendrape-geom.workspace = true
spade.workspace = true

[lints]
workspace = true
```

Create `crates/mesh/src/triangulate.rs` with only its tests:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    /// Points at most `h` apart round the polygon `corners`, corners included.
    fn sampled(corners: &[[f64; 2]], h: f64) -> Vec<[f64; 2]> {
        let mut out = Vec::new();
        for k in 0..corners.len() {
            let (a, b) = (corners[k], corners[(k + 1) % corners.len()]);
            let steps = (((b[0] - a[0]).hypot(b[1] - a[1]) / h).round() as usize).max(1);
            for s in 0..steps {
                let t = s as f64 / steps as f64;
                out.push([a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t]);
            }
        }
        out
    }

    #[test]
    fn keeps_the_boundary_and_leaves_holes_empty() {
        let outline = sampled(
            &[[0.0, 0.0], [400.0, 0.0], [400.0, 400.0], [0.0, 400.0]],
            12.0,
        );
        let hole: Vec<[f64; 2]> = (0..31)
            .map(|k| {
                let a = -(k as f64) / 31.0 * std::f64::consts::TAU; // clockwise
                [200.0 + 60.0 * a.cos(), 200.0 + 60.0 * a.sin()]
            })
            .collect();
        let t = triangulate(&outline, std::slice::from_ref(&hole), 12.0, 100_000).unwrap();
        let given: Vec<[f64; 2]> = outline.iter().chain(&hole).copied().collect();
        assert_eq!(
            &t.points[..given.len()],
            &given[..],
            "boundary first and unchanged"
        );
        for tri in &t.triangles {
            let c = tri.map(|k| t.points[k as usize]);
            let (x, y) = (
                (c[0][0] + c[1][0] + c[2][0]) / 3.0,
                (c[0][1] + c[1][1] + c[2][1]) / 3.0,
            );
            assert!(
                (x - 200.0).hypot(y - 200.0) > 58.0,
                "a triangle in the hole"
            );
            let twice_area = (c[1][0] - c[0][0]) * (c[2][1] - c[0][1])
                - (c[2][0] - c[0][0]) * (c[1][1] - c[0][1]);
            assert!(twice_area > 0.0, "anticlockwise");
        }
    }

    #[test]
    fn crossings_and_repeated_points_are_refused() {
        let bow = [[0.0, 0.0], [100.0, 100.0], [100.0, 0.0], [0.0, 100.0]];
        assert_eq!(
            triangulate(&bow, &[], 12.0, 1000),
            Err(TriangulateError::CrossesItself)
        );
        let repeated = [
            [0.0, 0.0],
            [100.0, 0.0],
            [100.0, 100.0],
            [100.0, 0.0],
            [0.0, 100.0],
        ];
        assert_eq!(
            triangulate(&repeated, &[], 12.0, 1000),
            Err(TriangulateError::CrossesItself)
        );
        let flat = [[0.0, 0.0], [1.0, 0.0], [2.0, 0.0]];
        assert_eq!(
            triangulate(&flat, &[], 12.0, 1000),
            Err(TriangulateError::CrossesItself)
        );
        let nan = [[0.0, 0.0], [f64::NAN, 0.0], [0.0, 1.0]];
        assert_eq!(
            triangulate(&nan, &[], 12.0, 1000),
            Err(TriangulateError::NotFinite)
        );
        assert_eq!(
            triangulate(&nan[..2], &[], 12.0, 1000),
            Err(TriangulateError::TooFewPoints)
        );
        let square = sampled(
            &[[0.0, 0.0], [100.0, 0.0], [100.0, 100.0], [0.0, 100.0]],
            12.0,
        );
        let across: Vec<[f64; 2]> = (0..12)
            .map(|k| {
                let a = k as f64 / 12.0 * std::f64::consts::TAU;
                [100.0 + 30.0 * a.cos(), 50.0 + 30.0 * a.sin()]
            })
            .collect();
        assert_eq!(
            triangulate(&square, &[across], 12.0, 1000),
            Err(TriangulateError::CrossesItself)
        );
    }

    #[test]
    fn the_same_outline_gives_the_same_triangles() {
        let disk: Vec<[f64; 2]> = (0..79)
            .map(|k| {
                let a = k as f64 / 79.0 * std::f64::consts::TAU;
                [3.0 + 150.0 * a.cos(), 7.0 + 150.0 * a.sin()]
            })
            .collect();
        assert_eq!(
            triangulate(&disk, &[], 12.0, 100_000),
            triangulate(&disk, &[], 12.0, 100_000)
        );
    }
}
```

Create `crates/mesh/src/lib.rs` holding only `mod triangulate;`, and `crates/mesh/tests/fabric.rs`:

```rust
//! The fabric `opendrape_mesh::build` makes from a student's pattern: well-shaped triangles,
//! the whole stitching outline covered, holes where cut-outs are, and both sides of every seam
//! sampled with the same number of points.

use opendrape_core::{
    Edge, Half, InternalLine, LineKind, Piece, PieceId, Point2, Project, SeamSide,
};
use opendrape_geom as geom;
use opendrape_mesh::{MeshNote, MeshParams, PanelMesh, Stitch, build};

const H: f64 = 12.0;

fn p(x: f64, y: f64) -> Point2 {
    Point2::new(x, y)
}

fn side(shape: PieceId, half: Half, first_edge: usize, edges: usize, forward: bool) -> SeamSide {
    SeamSide::new(shape, half, first_edge, edges, forward)
}

/// The skirt a student drafts: a front on the fold (half: hem 300, waist 177.5, 550 long, with
/// a curved waist and hem), and a back left with its mirrored twin. Side seams sewn (the
/// front's right edge to the back's slanted edge) and the centre back (the back to its twin).
/// Returns the project and the ids of front, back and twin.
fn skirt() -> (Project, [PieceId; 3]) {
    let mut pr = Project::new();
    let mut front = Piece::polygon(
        PieceId(0),
        "Front",
        &[p(0.0, 0.0), p(300.0, 0.0), p(177.5, 550.0), p(0.0, 550.0)],
    );
    front.edges[0] = Edge::Curve {
        c1: p(100.0, -12.0),
        c2: p(200.0, -10.0),
    };
    front.edges[2] = Edge::Curve {
        c1: p(120.0, 556.0),
        c2: p(60.0, 558.0),
    };
    front.fold = Some(3);
    let front = pr.add_piece(front);
    let back = pr.add_piece(Piece::polygon(
        PieceId(0),
        "Back",
        &[
            p(400.0, 0.0),
            p(700.0, 0.0),
            p(700.0, 550.0),
            p(522.5, 550.0),
        ],
    ));
    let twin = pr
        .add_twin(back, "Back (mirror)".into(), p(1500.0, 0.0))
        .unwrap();
    pr.add_seam(
        side(front, Half::Drawn, 1, 1, true),
        side(back, Half::Drawn, 3, 1, false),
    );
    pr.add_seam(
        side(back, Half::Drawn, 1, 1, true),
        side(twin, Half::Drawn, 1, 1, true),
    );
    assert_eq!(pr.check(), Ok(()));
    (pr, [front, back, twin])
}

/// A bodice-like piece: an armhole curve, a neckline curve and a round cut-out.
fn bodice() -> Project {
    let mut pr = Project::new();
    let mut piece = Piece::polygon(
        PieceId(0),
        "Bodice",
        &[
            p(0.0, 0.0),
            p(220.0, 0.0),
            p(230.0, 260.0),
            p(150.0, 400.0),
            p(60.0, 420.0),
            p(0.0, 380.0),
        ],
    );
    piece.edges[2] = Edge::Curve {
        c1: p(180.0, 280.0),
        c2: p(170.0, 360.0),
    };
    piece.edges[4] = Edge::Curve {
        c1: p(40.0, 380.0),
        c2: p(10.0, 370.0),
    };
    let ring: Vec<Point2> = (0..24)
        .map(|k| {
            let a = k as f64 / 24.0 * std::f64::consts::TAU;
            p(100.0 + 30.0 * a.cos(), 150.0 + 30.0 * a.sin())
        })
        .collect();
    piece.lines = vec![InternalLine {
        kind: LineKind::Cutout,
        ..InternalLine::polygon(&ring)
    }];
    pr.add_piece(piece);
    assert_eq!(pr.check(), Ok(()));
    pr
}

/// Each triangle of `panel` as three points in millimetres.
fn triangles_mm(panel: &PanelMesh) -> Vec<[Point2; 3]> {
    panel
        .triangles
        .iter()
        .map(|t| {
            t.map(|k| {
                p(
                    panel.flat[k as usize][0] * 1000.0,
                    panel.flat[k as usize][1] * 1000.0,
                )
            })
        })
        .collect()
}

fn angles(t: [Point2; 3]) -> [f64; 3] {
    [0, 1, 2].map(|i| {
        let (a, b, c) = (t[i], t[(i + 1) % 3], t[(i + 2) % 3]);
        let (u, w) = (b - a, c - a);
        ((u.x * w.x + u.y * w.y) / (u.length() * w.length()))
            .clamp(-1.0, 1.0)
            .acos()
            .to_degrees()
    })
}

fn area(t: [Point2; 3]) -> f64 {
    0.5 * ((t[1].x - t[0].x) * (t[2].y - t[0].y) - (t[2].x - t[0].x) * (t[1].y - t[0].y))
}

#[test]
fn a_skirt_becomes_three_panels_sewn_at_matching_points() {
    let (pr, [front, back, twin]) = skirt();
    let mesh = build(&pr, &MeshParams::default());
    assert_eq!(mesh.notes, vec![]);
    assert_eq!(
        mesh.panels.iter().map(|p| p.shape).collect::<Vec<_>>(),
        vec![front, back, twin]
    );
    // The side seam (563.5 mm), its mirror image and the centre back (550 mm): each side gets
    // round(length / 12) + 1 points.
    let side_seam = (563.5_f64 / H).round() as usize + 1;
    let centre_back = (550.0 / H).round() as usize + 1;
    assert_eq!(mesh.stitches.len(), 2 * side_seam + centre_back);
    // The front's side seam runs up its right edge from the hem; the back's runs down its
    // slanted edge backwards, so it starts at the hem too: each pair starts level.
    for &((pa, a), (pb, b)) in &mesh.stitches[..side_seam] {
        let (fa, fb) = (
            mesh.panels[pa].flat[a as usize],
            mesh.panels[pb].flat[b as usize],
        );
        assert!((fa[1] - fb[1]).abs() < 1e-9, "level: {fa:?} {fb:?}");
    }
    // Every stitched point is on its panel's outline, and no point is sewn twice by a seam.
    for &((pa, a), (pb, b)) in &mesh.stitches {
        assert!(mesh.panels[pa].edges.iter().flatten().any(|&k| k == a));
        assert!(mesh.panels[pb].edges.iter().flatten().any(|&k| k == b));
    }
    // The whole front is one panel with the fold inside it: twice the half's area.
    let half = geom::area(pr.piece(front).unwrap());
    let front_area: f64 = triangles_mm(&mesh.panels[0]).into_iter().map(area).sum();
    assert!(
        (front_area / (2.0 * half) - 1.0).abs() < 0.005,
        "{front_area} vs {}",
        2.0 * half
    );
    // The twin is its own panel, the back's mirror image.
    let back_area: f64 = triangles_mm(&mesh.panels[1]).into_iter().map(area).sum();
    let twin_area: f64 = triangles_mm(&mesh.panels[2]).into_iter().map(area).sum();
    assert!((back_area - twin_area).abs() < 1e-6 * back_area);
}

#[test]
fn triangles_are_well_shaped_and_close_to_the_target_size() {
    let (skirt, _) = skirt();
    for pr in [skirt, bodice()] {
        let mesh = build(&pr, &MeshParams::default());
        for panel in &mesh.panels {
            for t in triangles_mm(panel) {
                let smallest = angles(t).into_iter().fold(180.0, f64::min);
                assert!(smallest >= 25.0, "{smallest:.2}° in {:?}", panel.shape);
                for i in 0..3 {
                    let e = t[i].distance(t[(i + 1) % 3]) / H;
                    assert!(
                        (0.5..=1.6).contains(&e),
                        "an edge of {e:.2} h in {:?}",
                        panel.shape
                    );
                }
            }
        }
    }
}

#[test]
fn sharp_corners_only_spoil_the_triangles_next_to_them() {
    // Corners of 24° and 36°: the triangles touching them can't beat those angles, but every
    // triangle is at least 20° unless a corner sharper than that is part of it, and every
    // triangle more than 4 h from a corner sharper than 45° is at least 25°.
    let mut pr = Project::new();
    let corners = [p(0.0, 0.0), p(500.0, 0.0), p(150.0, 210.0), p(0.0, 300.0)];
    let piece = pr.add_piece(Piece::polygon(PieceId(0), "Spike", &corners));
    let mesh = build(&pr, &MeshParams::default());
    let panel = &mesh.panels[mesh.panel_of(piece).unwrap()];
    let corner_angles: Vec<f64> = (0..4)
        .map(|k| {
            let (a, o, b) = (corners[(k + 3) % 4], corners[k], corners[(k + 1) % 4]);
            angles([o, a, b])[0]
        })
        .collect();
    assert!(corner_angles.iter().any(|a| *a < 45.0), "{corner_angles:?}");
    for t in triangles_mm(panel) {
        let smallest = angles(t).into_iter().fold(180.0, f64::min);
        let near_sharp = corners
            .iter()
            .zip(&corner_angles)
            .any(|(c, a)| *a < 45.0 && t.iter().any(|q| q.distance(*c) < 4.0 * H));
        if !near_sharp {
            assert!(
                smallest >= 25.0,
                "{smallest:.2}° away from the sharp corners"
            );
        }
        let touches_sharper_than_20 = corners
            .iter()
            .zip(&corner_angles)
            .any(|(c, a)| *a < 20.0 && t.iter().any(|q| q.distance(*c) < 1e-9));
        if !touches_sharper_than_20 {
            assert!(smallest >= 20.0, "{smallest:.2}°");
        }
    }
}

#[test]
fn the_fabric_covers_the_stitching_outline_except_cut_outs() {
    let pr = bodice();
    let mesh = build(&pr, &MeshParams::default());
    let panel = &mesh.panels[0];
    let piece = &pr.pieces[0];
    let hole = 30.0 * 30.0 * std::f64::consts::PI;
    let want = geom::area(piece) - hole;
    let got: f64 = triangles_mm(panel).into_iter().map(area).sum();
    assert!((got / want - 1.0).abs() < 0.005, "{got} vs {want}");
    for t in triangles_mm(panel) {
        let c = p(
            (t[0].x + t[1].x + t[2].x) / 3.0,
            (t[0].y + t[1].y + t[2].y) / 3.0,
        );
        assert!(
            c.distance(p(100.0, 150.0)) > 29.0,
            "a triangle in the cut-out at {c:?}"
        );
    }
}

#[test]
fn seam_sides_always_get_the_same_count() {
    let mut pr = Project::new();
    let short = pr.add_piece(Piece::rectangle(
        PieceId(0),
        "Short",
        p(0.0, 0.0),
        100.0,
        200.0,
    ));
    let long = pr.add_piece(Piece::rectangle(
        PieceId(0),
        "Long",
        p(300.0, 0.0),
        100.0,
        236.0,
    ));
    // The short piece's right edge (200 mm) to the long one's left (236 mm), and the short
    // piece's top and left edges (300 mm, two edges) to the long one's bottom and right (336).
    let ease = pr.add_seam(
        side(short, Half::Drawn, 1, 1, true),
        side(long, Half::Drawn, 3, 1, false),
    );
    pr.add_seam(
        side(short, Half::Drawn, 2, 2, true),
        side(long, Half::Drawn, 0, 2, false),
    );
    let mesh = build(&pr, &MeshParams::default());
    assert!(mesh.notes.iter().any(|n| matches!(
        n,
        MeshNote::LengthsDiffer { seam, by_mm } if *seam == ease && (by_mm - 36.0).abs() < 1e-6
    )));
    let first = (236.0_f64 / H).round() as usize + 1;
    let second = (336.0_f64 / H).round() as usize + 1;
    assert_eq!(mesh.stitches.len(), first + second);
    // Each side's points are all different points.
    for range in [0..first, first..first + second] {
        let distinct = |pick: fn(&Stitch) -> u32| {
            let mut points: Vec<u32> = mesh.stitches[range.clone()].iter().map(pick).collect();
            points.sort_unstable();
            points.dedup();
            points.len()
        };
        assert_eq!(distinct(|s| s.0.1), range.len());
        assert_eq!(distinct(|s| s.1.1), range.len());
    }
    // The two-edge side keeps its corner: the short piece's corner (100,200) is stitched.
    let short_panel = &mesh.panels[mesh.panel_of(short).unwrap()];
    let corner = short_panel.edges[2][0];
    assert!(mesh.stitches[first..].iter().any(|s| s.0 == (0, corner)));
}

#[test]
fn a_folded_piece_sewn_to_itself_is_one_tube() {
    let mut pr = Project::new();
    let mut half = Piece::rectangle(PieceId(0), "Sleeve", p(0.0, 0.0), 150.0, 400.0);
    half.fold = Some(3);
    let id = pr.add_piece(half);
    // The drawn half's right edge to the pale half's: the sleeve's underarm seam.
    pr.add_seam(
        side(id, Half::Drawn, 1, 1, true),
        side(id, Half::Pale, 1, 1, true),
    );
    assert_eq!(pr.all_seams().len(), 1, "its own mirror image");
    let mesh = build(&pr, &MeshParams::default());
    assert_eq!(mesh.panels.len(), 1);
    assert_eq!(mesh.stitches.len(), (400.0_f64 / H).round() as usize + 1);
    for &((_, a), (_, b)) in &mesh.stitches {
        let (fa, fb) = (
            mesh.panels[0].flat[a as usize],
            mesh.panels[0].flat[b as usize],
        );
        assert!(
            (fa[0] + fb[0]).abs() < 1e-9 && (fa[1] - fb[1]).abs() < 1e-9,
            "mirror points"
        );
    }
}

#[test]
fn an_outline_that_crosses_itself_is_left_out_with_its_seams() {
    let mut pr = Project::new();
    let bow = pr.add_piece(Piece::polygon(
        PieceId(0),
        "Front",
        &[p(0.0, 0.0), p(200.0, 200.0), p(200.0, 0.0), p(0.0, 200.0)],
    ));
    let other = pr.add_piece(Piece::rectangle(
        PieceId(0),
        "Back",
        p(400.0, 0.0),
        200.0,
        200.0,
    ));
    // The bow's edge 1, (200,200) to (200,0), is as long as the back's bottom edge.
    pr.add_seam(
        side(bow, Half::Drawn, 1, 1, true),
        side(other, Half::Drawn, 0, 1, true),
    );
    let mesh = build(&pr, &MeshParams::default());
    assert_eq!(mesh.notes, vec![MeshNote::CrossesItself(bow)]);
    assert_eq!(mesh.panels.len(), 1);
    assert_eq!(mesh.panels[0].shape, other);
    assert!(mesh.stitches.is_empty());
}

#[test]
fn a_huge_pattern_gets_coarser_fabric() {
    let mut pr = Project::new();
    for k in 0..40 {
        pr.add_piece(Piece::rectangle(
            PieceId(0),
            "Panel",
            p(k as f64 * 1100.0, 0.0),
            1000.0,
            1000.0,
        ));
    }
    let mesh = build(&pr, &MeshParams::default());
    assert!(mesh.edge_mm > H);
    assert_eq!(
        mesh.notes,
        vec![MeshNote::Coarser {
            edge_mm: mesh.edge_mm
        }]
    );
    assert!(
        mesh.particles() < opendrape_mesh::MAX_PARTICLES,
        "{}",
        mesh.particles()
    );
}

#[test]
fn the_same_project_gives_the_same_fabric() {
    let (pr, _) = skirt();
    assert_eq!(
        build(&pr, &MeshParams::default()),
        build(&pr, &MeshParams::default())
    );
}

#[test]
fn random_patterns_never_panic() {
    let mut seed: u64 = 0x2545_f491_4f6c_dd1d;
    let mut rnd = move || {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        (seed >> 11) as f64 / (1u64 << 53) as f64
    };
    let (mut meshed, mut left_out) = (0, 0);
    for _ in 0..150 {
        let mut pr = Project::new();
        let pieces = 1 + (rnd() * 3.0) as usize;
        for k in 0..pieces {
            let n = 3 + (rnd() * 6.0) as usize;
            let corners: Vec<Point2> = (0..n)
                .map(|_| p(k as f64 * 700.0 + rnd() * 500.0, rnd() * 500.0))
                .collect();
            let mut piece = Piece::polygon(PieceId(0), "Random", &corners);
            if rnd() < 0.3 {
                piece.set_curved(0, true);
            }
            let id = pr.add_piece(piece);
            if rnd() < 0.3 {
                pr.add_twin(id, "Twin".into(), p(k as f64 * 700.0 + 2000.0, 0.0));
            }
        }
        let ids: Vec<(PieceId, usize)> = geom::shapes(&pr)
            .iter()
            .map(|s| (s.id, s.stored_len()))
            .collect();
        for _ in 0..3 {
            let (a, na) = ids[(rnd() * ids.len() as f64) as usize];
            let (b, nb) = ids[(rnd() * ids.len() as f64) as usize];
            let mut tried = pr.clone();
            tried.add_seam(
                side(a, Half::Drawn, (rnd() * na as f64) as usize, 1, rnd() < 0.5),
                side(b, Half::Drawn, (rnd() * nb as f64) as usize, 1, rnd() < 0.5),
            );
            if tried.check().is_ok() {
                pr = tried;
            }
        }
        let mesh = build(&pr, &MeshParams::default());
        for panel in &mesh.panels {
            assert!(
                panel
                    .triangles
                    .iter()
                    .flatten()
                    .all(|&k| (k as usize) < panel.flat.len())
            );
        }
        for &((pa, a), (pb, b)) in &mesh.stitches {
            assert!(
                (a as usize) < mesh.panels[pa].flat.len()
                    && (b as usize) < mesh.panels[pb].flat.len()
            );
        }
        meshed += mesh.panels.len();
        left_out += mesh
            .notes
            .iter()
            .filter(|n| matches!(n, MeshNote::CrossesItself(_)))
            .count();
    }
    assert!(
        meshed > 0 && left_out > 0,
        "{meshed} meshed, {left_out} left out"
    );
}
```

Run: `cargo nextest run -p opendrape-mesh`
Expected: compile errors (`triangulate`, `build`, `MeshParams`, … don't exist yet).

- [ ] **Step 2: Triangulating one panel** (`crates/mesh/src/triangulate.rs`, above its tests)

```rust
//! One panel's fabric: a constrained Delaunay triangulation of its outline (and holes),
//! refined until no triangle is too big or too thin. Every boundary point is kept, in order,
//! so the points sampled along a seam are exactly the points that get stitched.

use spade::{
    AngleLimit, ConstrainedDelaunayTriangulation, Point2 as SPoint, RefinementParameters,
    Triangulation,
};

/// Refinement adds points until no triangle has an angle below this (degrees), except next to
/// an outline corner sharper than that, which no triangle there can beat.
pub const ANGLE_LIMIT_DEG: f64 = 25.0;
/// The largest triangle refinement leaves is this many squared edge lengths: 0.5 h² keeps edges
/// between 0.6 and 1.5 h (an equilateral triangle of side h is 0.433 h²).
const MAX_AREA_PER_H2: f64 = 0.5;
/// Triangles smaller than this many squared edge lengths are never split further.
const MIN_AREA_PER_H2: f64 = 0.02;

/// A triangulated panel, in the units of its input.
#[derive(Clone, Debug, PartialEq)]
pub struct Triangulated {
    /// The boundary points first, exactly as given (outline, then each hole), then the points
    /// added inside.
    pub points: Vec<[f64; 2]>,
    /// Anticlockwise triangles (indices into `points`) covering the inside of the outline and
    /// none of its holes.
    pub triangles: Vec<[u32; 3]>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TriangulateError {
    /// The outline has fewer than 3 points.
    TooFewPoints,
    /// A point is not a finite number.
    NotFinite,
    /// Two boundary points coincide, or the outline (or a hole) crosses or touches itself or
    /// another loop.
    CrossesItself,
}

/// Triangulates the area inside `outline` and outside every hole (closed loops; the first
/// point is not repeated; either winding), aiming at edges `h` long. Refinement adds at most
/// `max_added` points.
pub fn triangulate(
    outline: &[[f64; 2]],
    holes: &[Vec<[f64; 2]>],
    h: f64,
    max_added: usize,
) -> Result<Triangulated, TriangulateError> {
    if outline.len() < 3 {
        return Err(TriangulateError::TooFewPoints);
    }
    let loops: Vec<&[[f64; 2]]> = std::iter::once(outline)
        .chain(holes.iter().map(Vec::as_slice).filter(|l| l.len() >= 3))
        .collect();
    let mut cdt: ConstrainedDelaunayTriangulation<SPoint<f64>> =
        ConstrainedDelaunayTriangulation::new();
    let mut handles = Vec::with_capacity(loops.len());
    for points in &loops {
        let mut loop_handles = Vec::with_capacity(points.len());
        for p in points.iter() {
            if !(p[0].is_finite() && p[1].is_finite()) {
                return Err(TriangulateError::NotFinite);
            }
            // spade refuses coordinates below 2^-142 in size: such a number is zero here.
            let tidy = |v: f64| if v.abs() < 1e-9 { 0.0 } else { v };
            let before = cdt.num_vertices();
            let v = cdt
                .insert(SPoint::new(tidy(p[0]), tidy(p[1])))
                .map_err(|_| TriangulateError::NotFinite)?;
            if cdt.num_vertices() == before {
                // spade merged it with an earlier point at the same place.
                return Err(TriangulateError::CrossesItself);
            }
            loop_handles.push(v);
        }
        handles.push(loop_handles);
    }
    for loop_handles in &handles {
        for k in 0..loop_handles.len() {
            let (a, b) = (loop_handles[k], loop_handles[(k + 1) % loop_handles.len()]);
            // Exactly one new constraint edge: anything else means it crossed another
            // boundary edge (none added) or ran through another boundary point (split).
            if cdt.try_add_constraint(a, b).len() != 1 {
                return Err(TriangulateError::CrossesItself);
            }
        }
    }
    let boundary = cdt.num_vertices();
    let params = RefinementParameters::<f64>::new()
        .with_angle_limit(AngleLimit::from_deg(ANGLE_LIMIT_DEG))
        .with_max_allowed_area(MAX_AREA_PER_H2 * h * h)
        .with_min_required_area(MIN_AREA_PER_H2 * h * h)
        .keep_constraint_edges()
        .exclude_outer_faces(true)
        .with_max_additional_vertices(max_added);
    let result = cdt.refine(params);
    let outside: std::collections::HashSet<_> = result.excluded_faces.into_iter().collect();
    let mut used = vec![false; cdt.num_vertices()];
    let mut faces = Vec::new();
    // spade lists its faces in a fixed order, so the result is the same on every run.
    for face in cdt.inner_faces() {
        if outside.contains(&face.fix()) {
            continue;
        }
        let t = face.vertices().map(|v| v.fix().index());
        for &k in &t {
            used[k] = true;
        }
        faces.push(t);
    }
    // Boundary points stay, in order; points refinement added outside the outline are dropped.
    let mut new_index = vec![u32::MAX; used.len()];
    let mut points = Vec::with_capacity(used.len());
    for (k, v) in cdt.vertices().enumerate() {
        if k < boundary || used[k] {
            new_index[k] = points.len() as u32;
            let p = v.position();
            points.push([p.x, p.y]);
        }
    }
    let triangles = faces.iter().map(|t| t.map(|k| new_index[k])).collect();
    Ok(Triangulated { points, triangles })
}
```

- [ ] **Step 3: The outline with seam sides in step** (`crates/mesh/src/boundary.rs`)

```rust
//! The points round one shape's outline that become the fabric's edge: every corner, points
//! about `h` apart along free edges, and along each seam side the side's own count of points
//! at equal steps, so both sides of a seam have the same number.

use opendrape_core::{Point2, SeamSide};
use opendrape_geom::{self as geom, Shape};

/// A shape's outline as points (mm), and where things are among them.
pub(crate) struct Outline {
    pub points: Vec<Point2>,
    /// For each outline edge: its start corner, the points along it, its end corner.
    pub edges: Vec<Vec<u32>>,
    /// For each side asked for: the point of each of its `steps + 1` samples, from its start.
    pub sides: Vec<Vec<u32>>,
}

/// A point along an outline edge: its distance (mm) from the edge's start, and the side
/// sample it is (side, sample), if any.
type Along = (f64, Option<(usize, usize)>);

/// One sample of a side: on outline edge `edge`, `along` mm from that edge's start.
#[derive(Clone, Copy, Debug)]
struct At {
    edge: usize,
    along: f64,
}

/// The outline of `shape` with each of `sides` (a side on this shape, and its step count)
/// sampled at equal steps. None when a side does not fit the shape.
pub(crate) fn outline(shape: &Shape, sides: &[(SeamSide, usize)], h: f64) -> Option<Outline> {
    let piece = &shape.piece;
    let m = piece.len();
    let lens: Vec<f64> = (0..m).map(|j| geom::edge_length(piece, j)).collect();
    let mut sewn = vec![false; m];
    // Per outline edge, the points along it.
    let mut along: Vec<Vec<Along>> = vec![Vec::new(); m];
    // Side samples that land on a corner: (side, sample, corner).
    let mut on_corner = Vec::new();
    for (s, (side, steps)) in sides.iter().enumerate() {
        let runs = geom::side_edges(shape, side)?;
        for (j, _) in &runs {
            sewn[*j] = true;
        }
        for (k, at) in side_samples(&runs, &lens, *steps).into_iter().enumerate() {
            let len = lens[at.edge];
            let tiny = 1e-9 * len.max(1.0);
            if at.along <= tiny {
                on_corner.push((s, k, at.edge));
            } else if at.along >= len - tiny {
                on_corner.push((s, k, (at.edge + 1) % m));
            } else {
                along[at.edge].push((at.along, Some((s, k))));
            }
        }
    }
    for j in 0..m {
        if !sewn[j] {
            let steps = ((lens[j] / h).round() as usize).max(1);
            along[j].extend((1..steps).map(|q| (lens[j] * q as f64 / steps as f64, None)));
        }
        along[j].sort_by(|a, b| a.0.total_cmp(&b.0));
    }
    let mut points = Vec::new();
    let mut corners = Vec::with_capacity(m);
    let mut inner: Vec<Vec<u32>> = vec![Vec::new(); m];
    let mut side_points: Vec<Vec<u32>> = sides.iter().map(|(_, n)| vec![0; n + 1]).collect();
    for j in 0..m {
        corners.push(points.len() as u32);
        points.push(piece.vertices[j].pos);
        for &(d, sample) in &along[j] {
            let index = points.len() as u32;
            points.push(geom::point_at_distance(piece, j, d));
            inner[j].push(index);
            if let Some((s, k)) = sample {
                side_points[s][k] = index;
            }
        }
    }
    for (s, k, corner) in on_corner {
        side_points[s][k] = corners[corner];
    }
    let edges = (0..m)
        .map(|j| {
            let mut e = vec![corners[j]];
            e.extend(&inner[j]);
            e.push(corners[(j + 1) % m]);
            e
        })
        .collect();
    Some(Outline {
        points,
        edges,
        sides: side_points,
    })
}

/// The `steps + 1` samples of a side made of `runs` (outline edge, and whether the side runs
/// against it), at equal steps of arc length from the side's start. Each corner inside the
/// side takes over the sample nearest to it, so no sample lies a sliver away from a corner;
/// a corner whose nearest sample another corner took keeps no sample.
fn side_samples(runs: &[(usize, bool)], lens: &[f64], steps: usize) -> Vec<At> {
    let total: f64 = runs.iter().map(|(j, _)| lens[*j]).sum();
    let step = total / steps as f64;
    let mut at: Vec<f64> = (0..=steps).map(|k| k as f64 * step).collect();
    at[steps] = total;
    let mut taken = vec![false; steps + 1];
    let mut start = 0.0;
    for (j, _) in &runs[..runs.len() - 1] {
        start += lens[*j];
        if steps >= 2 {
            let k = ((start / step).round() as usize).clamp(1, steps - 1);
            if !taken[k] {
                taken[k] = true;
                at[k] = start;
            }
        }
    }
    at.into_iter()
        .map(|s| {
            // The run this sample is on: the last one starting at or before it.
            let mut start = 0.0;
            let mut r = 0;
            while r + 1 < runs.len() && s >= start + lens[runs[r].0] {
                start += lens[runs[r].0];
                r += 1;
            }
            let (edge, against) = runs[r];
            let local = (s - start).clamp(0.0, lens[edge]);
            At {
                edge,
                along: if against { lens[edge] - local } else { local },
            }
        })
        .collect()
}
```

- [ ] **Step 4: Building the garment** (replace `crates/mesh/src/lib.rs`)

```rust
//! Pattern pieces into fabric. Each shape on the pattern table (a piece, a whole cut-on-fold
//! piece, a twin) becomes one panel: its stitching outline sampled into points (both sides of a
//! seam with the same count) and filled with near-equilateral triangles. Seams become pairs of
//! stitched points. Pure: no GPU, no windows.

mod boundary;
mod triangulate;

pub use triangulate::{ANGLE_LIMIT_DEG, TriangulateError, Triangulated, triangulate};

use opendrape_core::{LineKind, PieceId, Point2, Project, SeamId};
use opendrape_geom::{self as geom, Shape};

/// The fabric's target edge length (mm).
pub const DEFAULT_EDGE_MM: f64 = 12.0;
/// The most particles a garment's fabric may have.
pub const MAX_PARTICLES: usize = 30_000;
/// Seam sides whose lengths differ by more than this (mm) get a note.
pub const LENGTH_WARNING_MM: f64 = 3.0;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MeshParams {
    /// Target edge length (mm).
    pub edge_mm: f64,
    /// The fabric grows coarser until its estimated particle count is at most this.
    pub max_particles: usize,
}

impl Default for MeshParams {
    fn default() -> Self {
        Self {
            edge_mm: DEFAULT_EDGE_MM,
            max_particles: MAX_PARTICLES,
        }
    }
}

/// One panel of fabric.
#[derive(Clone, Debug, PartialEq)]
pub struct PanelMesh {
    /// The shape it was made from (a piece's id, or a twin's).
    pub shape: PieceId,
    /// Flat positions (m) on the pattern table: the fabric's rest shape.
    pub flat: Vec<[f64; 2]>,
    /// Anticlockwise on the pattern table.
    pub triangles: Vec<[u32; 3]>,
    /// For each outline edge of the shape: its points from its start corner to its end corner.
    pub edges: Vec<Vec<u32>>,
    /// The middle of the shape's bounding box on the pattern table (mm): the point placements
    /// put at their position.
    pub centre: Point2,
}

/// Something the student should know about the fabric.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum MeshNote {
    /// The pattern was too large for the fabric's particle budget: edges are `edge_mm` long.
    Coarser { edge_mm: f64 },
    /// This shape's outline crosses itself, so it was left out with its seams.
    CrossesItself(PieceId),
    /// This shape could not be made into fabric for another reason; left out with its seams.
    Unmeshable(PieceId),
    /// The two sides of this seam differ in length by `by_mm`: the longer gathers as ease.
    LengthsDiffer { seam: SeamId, by_mm: f64 },
}

/// Two points sewn together: (panel, point) each.
pub type Stitch = ((usize, u32), (usize, u32));

/// A garment's fabric: its panels, the stitches that sew them, and notes.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct GarmentMesh {
    pub panels: Vec<PanelMesh>,
    /// Every pair of points sewn together, in seam order.
    pub stitches: Vec<Stitch>,
    pub notes: Vec<MeshNote>,
    /// The edge length (mm) the fabric was made with.
    pub edge_mm: f64,
}

impl GarmentMesh {
    /// Points in every panel together.
    pub fn particles(&self) -> usize {
        self.panels.iter().map(|p| p.flat.len()).sum()
    }
    /// The panel made from `shape`, if it could be made.
    pub fn panel_of(&self, shape: PieceId) -> Option<usize> {
        self.panels.iter().position(|p| p.shape == shape)
    }
}

/// The fabric for every shape of `project`, and the stitches for every seam (mirror images
/// included). A shape that can't be meshed is left out with its seams, and a note says why.
pub fn build(project: &Project, params: &MeshParams) -> GarmentMesh {
    let shapes = geom::shapes(project);
    let mut notes = Vec::new();
    let h = edge_length_within_budget(&shapes, params);
    if h > params.edge_mm {
        notes.push(MeshNote::Coarser { edge_mm: h });
    }
    // Every seam, mirror images too: its sides' shapes and its step count.
    let mut seams = Vec::new();
    for (seam, mirrored) in project.all_seams() {
        let shape_of = |id| shapes.iter().position(|s: &Shape| s.id == id);
        let (Some(sa), Some(sb)) = (shape_of(seam.a.shape), shape_of(seam.b.shape)) else {
            continue;
        };
        let (Some(la), Some(lb)) = (
            geom::side_length(&shapes[sa], &seam.a),
            geom::side_length(&shapes[sb], &seam.b),
        ) else {
            continue;
        };
        if !mirrored && (la - lb).abs() > LENGTH_WARNING_MM {
            notes.push(MeshNote::LengthsDiffer {
                seam: seam.id,
                by_mm: (la - lb).abs(),
            });
        }
        let steps = ((la.max(lb) / h).round() as usize).max(1);
        seams.push((seam, sa, sb, steps));
    }
    let mut panels = Vec::new();
    // For each shape: its panel's index, and where each of its sides' samples landed.
    let mut made: Vec<Option<(usize, Vec<Vec<u32>>)>> = Vec::with_capacity(shapes.len());
    for (index, shape) in shapes.iter().enumerate() {
        let sides: Vec<_> = seams
            .iter()
            .flat_map(|(seam, sa, sb, steps)| {
                [(seam.a, *sa), (seam.b, *sb)]
                    .into_iter()
                    .filter(|(_, s)| *s == index)
                    .map(|(side, _)| (side, *steps))
            })
            .collect();
        match panel(shape, &sides, h, params.max_particles) {
            Ok((mesh, side_points)) => {
                made.push(Some((panels.len(), side_points)));
                panels.push(mesh);
            }
            Err(TriangulateError::CrossesItself) => {
                notes.push(MeshNote::CrossesItself(shape.id));
                made.push(None);
            }
            Err(_) => {
                notes.push(MeshNote::Unmeshable(shape.id));
                made.push(None);
            }
        }
    }
    // Stitch side samples in step: the k-th of side a to the k-th of side b. The order of
    // sides within each shape matches the order they were handed to `panel` above.
    let mut next_side = vec![0usize; shapes.len()];
    let mut stitches = Vec::new();
    for (_, sa, sb, steps) in &seams {
        let side_a = next_side[*sa];
        next_side[*sa] += 1;
        let side_b = next_side[*sb];
        next_side[*sb] += 1;
        if let (Some((pa, a)), Some((pb, b))) = (&made[*sa], &made[*sb]) {
            for k in 0..=*steps {
                stitches.push(((*pa, a[side_a][k]), (*pb, b[side_b][k])));
            }
        }
    }
    GarmentMesh {
        panels,
        stitches,
        notes,
        edge_mm: h,
    }
}

/// `params.edge_mm`, or a longer edge if the fabric would otherwise have more than
/// `params.max_particles` particles. Estimated from the shapes' areas and perimeters (about
/// 1.6 points per h² of area, measured on real panels, plus the outline's own points).
fn edge_length_within_budget(shapes: &[Shape], params: &MeshParams) -> f64 {
    let area: f64 = shapes.iter().map(|s| geom::area(&s.piece)).sum();
    let perimeter: f64 = shapes.iter().map(|s| geom::perimeter(&s.piece)).sum();
    let estimate = |h: f64| 1.7 * area / (h * h) + perimeter / h;
    let budget = params.max_particles as f64;
    let mut h = params.edge_mm;
    while estimate(h) > budget && h.is_finite() {
        h *= (estimate(h) / budget).sqrt() * 1.05;
    }
    h
}

/// One shape's panel, and where each of its `sides` samples landed among its points.
fn panel(
    shape: &Shape,
    sides: &[(opendrape_core::SeamSide, usize)],
    h: f64,
    max_points: usize,
) -> Result<(PanelMesh, Vec<Vec<u32>>), TriangulateError> {
    let outline = boundary::outline(shape, sides, h).ok_or(TriangulateError::CrossesItself)?;
    let corners: Vec<[f64; 2]> = outline.points.iter().map(|p| [p.x, p.y]).collect();
    let holes: Vec<Vec<[f64; 2]>> = shape
        .piece
        .lines
        .iter()
        .filter(|l| l.closed && l.kind == LineKind::Cutout)
        .map(|l| resampled_loop(&geom::line_points(l, 0.1), h))
        .collect();
    let mesh = triangulate(&corners, &holes, h, max_points)?;
    let (lo, hi) = bounds(&geom::outline_points(&shape.piece, 0.5));
    Ok((
        PanelMesh {
            shape: shape.id,
            flat: mesh
                .points
                .iter()
                .map(|p| [p[0] / 1000.0, p[1] / 1000.0])
                .collect(),
            triangles: mesh.triangles,
            edges: outline.edges,
            centre: lo.lerp(hi, 0.5),
        },
        outline.sides,
    ))
}

/// The smallest box holding `points`: (min, max).
pub fn bounds(points: &[Point2]) -> (Point2, Point2) {
    let first = points.first().copied().unwrap_or_default();
    points.iter().fold((first, first), |(lo, hi), p| {
        (
            Point2::new(lo.x.min(p.x), lo.y.min(p.y)),
            Point2::new(hi.x.max(p.x), hi.y.max(p.y)),
        )
    })
}

/// A closed polyline (its last point repeats its first) as points about `h` apart along it.
fn resampled_loop(points: &[Point2], h: f64) -> Vec<[f64; 2]> {
    let mut pts = points.to_vec();
    if pts.len() > 1 && pts.first() == pts.last() {
        pts.pop();
    }
    let n = pts.len();
    if n < 3 {
        return Vec::new();
    }
    let seg = |k: usize| pts[k].distance(pts[(k + 1) % n]);
    let total: f64 = (0..n).map(seg).sum();
    let count = ((total / h).round() as usize).max(3);
    let mut out = Vec::with_capacity(count);
    let (mut k, mut start) = (0, 0.0);
    for q in 0..count {
        let s = total * q as f64 / count as f64;
        while k + 1 < n && s > start + seg(k) {
            start += seg(k);
            k += 1;
        }
        let t = if seg(k) > 0.0 {
            (s - start) / seg(k)
        } else {
            0.0
        };
        let p = pts[k].lerp(pts[(k + 1) % n], t.clamp(0.0, 1.0));
        out.push([p.x, p.y]);
    }
    out
}
```

- [ ] **Step 5: Run and see them pass**

Run: `cargo nextest run -p opendrape-mesh`
Expected: the 3 triangulation tests and the 10 fabric tests pass. `random_patterns_never_panic` meshes some panels and leaves out some crossing ones. The skirt's and the bodice's triangles all have angles ≥ 25° and edges of 0.5–1.6 h.

If an angle or edge-length assertion fails, change nothing in the test thresholds. Check that `MAX_AREA_PER_H2` is 0.5 and the angle limit 25°: the probe measured 0.62–1.47 h and ≥ 26.1° with exactly these.

- [ ] **Step 6: Licences**

Run: `cargo deny check`
Expected: `advisories ok, bans ok, licenses ok, sources ok`. spade 2.15.1 and its dependencies (hashbrown, num-traits, robust, smallvec) are already in `Cargo.lock` through parry3d.

- [ ] **Step 7: Commit**

```bash
cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings
git add Cargo.toml Cargo.lock crates/mesh
git commit -m "feat(mesh): pattern pieces into refined fabric meshes, seams into matched stitches

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 4: The maths of placing and of the gizmo (review: full)

**Files:**
- Create:
  - `crates/mesh/src/place.rs`
  - `crates/app/src/arrange/mod.rs`
  - `crates/app/src/arrange/gizmo.rs`
- Modify:
  - `crates/mesh/Cargo.toml`, `crates/mesh/src/lib.rs`
  - `crates/app/Cargo.toml`, `crates/app/src/lib.rs`

**Interfaces:**
- Consumes:
  - Task 2: `Placement`, `Project::placement_of`, `MIN_CURVE_M`, `MAX_CURVE_M`;
  - Task 3: `opendrape_mesh::bounds`;
  - `opendrape_render::OrbitCamera` (pub fields `target, yaw, pitch, distance, fov_y`; `eye()`, `view_proj(aspect)`).
- Produces (`opendrape_mesh::place`):
  - Constants: `START_DISTANCE_M = 0.40`, `PLACE_GAP_M = 0.03`, `FALLBACK_RADIUS_M = 0.2`.
  - Applying placements:
    - `centre_of(&Shape) -> Point2`;
    - `rotation(&Placement) -> DQuat`, `position(&Placement) -> DVec3`;
    - `apply(&Placement, centre: Point2, flat: Point2) -> DVec3` (inputs in mm, output in m);
    - `mirrored(&Placement) -> Placement`.
  - Starting places:
    - `Layout { min: Point2, max: Point2 }` and `layout(&[Shape]) -> Layout`;
    - `start(&Layout, centre: Point2, shoulder_y: f64) -> Placement`;
    - `effective(&Project, &Shape, &Layout, shoulder_y) -> Placement`.
  - Place at…:
    - `enum PlaceAt { Front, Back, LeftSide, RightSide }`, with `ALL` and `angle(self) -> f64`;
    - `place_at(&Project, &Shape, PlaceAt, &Layout, shoulder_y, surface: &dyn Fn(f64, f64) -> Option<f64>) -> Placement`.
  - Typed angles: `euler_xyz_deg([f64; 4]) -> [f64; 3]` and `rotation_from_euler_xyz_deg([f64; 3]) -> [f64; 4]`.
- Produces (`opendrape::arrange::gizmo`, pure, no egui):
  - `ScreenCamera`:
    - `new(&OrbitCamera, min: DVec2, size: DVec2)`;
    - `eye()`, `rect() -> (DVec2, DVec2)`;
    - `project(DVec3) -> Option<DVec2>`, `ray(DVec2) -> (DVec3, DVec3)`, `metres_per_point(DVec3) -> f64`.
  - Drags:
    - `axis_drag(&ScreenCamera, centre, axis, from, to) -> Option<f64>`;
    - `plane_drag(&ScreenCamera, centre, from, to) -> Option<DVec3>`;
    - `ring_angle(&ScreenCamera, centre, axis, from, to) -> Option<f64>`;
    - `snap_angle(radians, step_deg) -> f64`.
  - Picking and handle geometry:
    - `ray_triangle(origin, dir, [DVec3; 3]) -> Option<f64>`;
    - `segment_distance(DVec2, DVec2, DVec2) -> f64`.
  - Constants: `AXES`, `ARROW_PT = 80`, `RING_PT = 60`, `SQUARE_PT = 9`, `GRAB_PT = 8`, `RING_STEPS = 48`, `EDGE_ON = 0.15`.
  - `enum Handle { Move(usize), Plane, Turn(usize) }`.
  - `Gizmo { centre: DVec3, size: f64 }`, with `new(&ScreenCamera, centre)`, `arrow_tip(axis)`, `arrow_shown(&ScreenCamera, axis)`, `ring(axis) -> Vec<DVec3>` and `hit(&ScreenCamera, DVec2) -> Option<Handle>`.

**The maths** (all proven in the probe at 144 camera poses):
- **Placing a flat point:** `local = (x, y)` in metres from the bounding-box middle. With a curve `r`, wrap it: `(r·sin(x/r), y, r·cos(x/r) − r)`. Then `rotation · local + position`.
- **Place at θ with radius R and local x `f` on the centre line:** `φ = θ − f/R`, `position = (R·sin φ, y, R·cos φ)`, `rotation = rot_y(φ)`. Every local x then lands at angle `φ + x/R` on the radius-R cylinder round the centre line.
- **Twin mirror:** position (x, y, z) → (−x, y, z); quaternion (x, y, z, w) → (x, −y, −z, w). Together with the twin's mirrored flat shape this gives the exact mirror image.
- **Arrow drag:** project the pointer onto the arrow's on-screen direction, then intersect that screen point's ray with the 3D axis line (closest approach). It is exact for any foreshortening. An axis covering fewer than 20 points per metre (pointing at the viewer) can't be dragged.
- **Ring drag:** with the ring seen at an angle, it uses the angle between the two pointer rays' hits on the ring's plane. Edge-on (|view · axis| < 0.15, decided at the centre so it never switches during a drag), it uses the pointer's angle round the projected centre, with the axis's facing setting the sign.

- [ ] **Step 1: Failing tests**

`crates/mesh/Cargo.toml`: under `[dependencies]` add `glam.workspace = true`. `crates/mesh/src/lib.rs`: add `pub mod place;` after `mod boundary;`.

Create `crates/mesh/src/place.rs` with only its tests:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use glam::DVec2;
    use opendrape_core::{Piece, PieceId};

    fn p(x: f64, y: f64) -> Point2 {
        Point2::new(x, y)
    }

    fn near(a: DVec3, b: DVec3) {
        assert!((a - b).length() < 1e-9, "{a} vs {b}");
    }

    #[test]
    fn a_placement_centres_wraps_turns_and_moves() {
        let flat = Placement::at([0.1, 1.0, 0.4]);
        near(
            apply(&flat, p(500.0, 300.0), p(500.0, 300.0)),
            DVec3::new(0.1, 1.0, 0.4),
        );
        near(
            apply(&flat, p(500.0, 300.0), p(600.0, 250.0)),
            DVec3::new(0.2, 0.95, 0.4),
        );
        let turned = Placement {
            rotation: DQuat::from_rotation_y(std::f64::consts::FRAC_PI_2).to_array(),
            ..flat
        };
        near(
            apply(&turned, p(0.0, 0.0), p(100.0, 0.0)),
            DVec3::new(0.1, 1.0, 0.3),
        );
        // Wrapped round a 0.2 m cylinder whose axis is 0.2 m behind the piece: every point
        // stays on it, the arc keeps the flat length, and the middle doesn't move.
        let r = 0.2;
        let curved = Placement {
            curve: Some(r),
            ..Placement::at([0.0, 0.0, 0.0])
        };
        let axis = DVec3::new(0.0, 0.0, -r);
        for x in [-300.0, -50.0, 0.0, 120.0] {
            let q = apply(&curved, p(0.0, 0.0), p(x, 10.0));
            assert!((DVec2::new(q.x - axis.x, q.z - axis.z).length() - r).abs() < 1e-12);
            let arc = (q.x - axis.x).atan2(q.z - axis.z) * r;
            assert!((arc - x / 1000.0).abs() < 1e-12, "{arc}");
        }
    }

    #[test]
    fn a_mirrored_placement_shows_the_mirror_image() {
        let placement = Placement {
            position: [0.13, 0.8, -0.17],
            rotation: DQuat::from_euler(glam::EulerRot::XYZ, 0.3, 2.1, -0.4).to_array(),
            curve: Some(0.21),
        };
        let m = mirrored(&placement);
        for q in [p(100.0, 50.0), p(-200.0, 300.0), p(0.0, 0.0)] {
            let piece = apply(&placement, p(0.0, 0.0), q);
            let twin = apply(&m, p(0.0, 0.0), p(-q.x, q.y));
            near(twin, DVec3::new(-piece.x, piece.y, piece.z));
        }
        assert!(m.is_valid());
    }

    /// A front (id 1, 0..400 × 0..600), a back (id 2, 600..900 × 0..550) whose twin (id 3)
    /// sits to its right at 1000..1300, all unplaced.
    fn pattern() -> Project {
        let mut pr = Project::new();
        pr.add_piece(Piece::rectangle(
            PieceId(0),
            "Front",
            p(0.0, 0.0),
            400.0,
            600.0,
        ));
        let back = pr.add_piece(Piece::rectangle(
            PieceId(0),
            "Back",
            p(600.0, 0.0),
            300.0,
            550.0,
        ));
        pr.add_twin(back, "Back (mirror)".into(), p(1900.0, 0.0))
            .unwrap();
        pr
    }

    #[test]
    fn unplaced_pieces_start_in_front_of_the_form() {
        let pr = pattern();
        let shapes = geom::shapes(&pr);
        let layout = layout(&shapes);
        assert_eq!((layout.min, layout.max), (p(0.0, 0.0), p(1300.0, 600.0)));
        // The front's middle (200, 300) is 450 mm left of the layout's middle and 300 mm below
        // its top.
        let front = effective(&pr, &shapes[0], &layout, 1.3);
        assert_eq!(front, Placement::at([-0.45, 1.0, START_DISTANCE_M]));
        // A twin with no placement while its piece has none starts at its own place.
        let twin = effective(&pr, &shapes[2], &layout, 1.3);
        assert!((twin.position[0] - (1150.0 - 650.0) / 1000.0).abs() < 1e-12);
    }

    #[test]
    fn a_twin_mirrors_its_piece_until_it_has_its_own_placement() {
        let mut pr = pattern();
        let placed = Placement {
            position: [0.1, 0.8, -0.2],
            rotation: DQuat::from_rotation_y(2.8).to_array(),
            curve: Some(0.2),
        };
        pr.set_placement(PieceId(2), Some(placed));
        let shapes = geom::shapes(&pr);
        let layout = layout(&shapes);
        assert_eq!(effective(&pr, &shapes[2], &layout, 1.3), mirrored(&placed));
        let own = Placement::at([0.0, 0.5, 1.0]);
        pr.set_placement(PieceId(3), Some(own));
        let shapes = geom::shapes(&pr);
        assert_eq!(effective(&pr, &shapes[2], &layout, 1.3), own);
    }

    /// A form stand-in: a cylinder of radius 0.15 m round the centre line, 0.5 to 1.5 m up.
    fn cylinder(_angle: f64, y: f64) -> Option<f64> {
        (0.5..=1.5).contains(&y).then_some(0.15)
    }

    #[test]
    fn place_at_wraps_a_piece_round_the_form_at_its_height() {
        let pr = pattern();
        let shapes = geom::shapes(&pr);
        let layout = layout(&shapes);
        let r = 0.15 + PLACE_GAP_M;
        let height = effective(&pr, &shapes[0], &layout, 1.3).position[1];
        let front = place_at(&pr, &shapes[0], PlaceAt::Front, &layout, 1.3, &cylinder);
        assert_eq!(front.curve, Some(r));
        near(position(&front), DVec3::new(0.0, height, r));
        near(rotation(&front) * DVec3::Z, DVec3::Z);
        let left = place_at(&pr, &shapes[0], PlaceAt::LeftSide, &layout, 1.3, &cylinder);
        near(position(&left), DVec3::new(r, height, 0.0));
        near(rotation(&left) * DVec3::Z, DVec3::X);
        let right = place_at(&pr, &shapes[0], PlaceAt::RightSide, &layout, 1.3, &cylinder);
        near(position(&right), DVec3::new(-r, height, 0.0));
        // Every point of the placed front is the gap clear of the cylinder.
        for x in [0.0, 100.0, 400.0] {
            let q = apply(&front, centre_of(&shapes[0]), p(x, 300.0));
            assert!((DVec2::new(q.x, q.z).length() - r).abs() < 1e-12);
        }
    }

    #[test]
    fn a_pair_meets_at_the_centre_line() {
        let pr = pattern();
        let shapes = geom::shapes(&pr);
        let layout = layout(&shapes);
        let r = 0.15 + PLACE_GAP_M;
        // The back's right side faces its twin on the table: it goes on the back centre line.
        let back = place_at(&pr, &shapes[1], PlaceAt::Back, &layout, 1.3, &cylinder);
        let corner = apply(&back, centre_of(&shapes[1]), p(900.0, 100.0));
        assert!(
            corner.x.abs() < 1e-12 && (corner.z + r).abs() < 1e-12,
            "{corner}"
        );
        // Its other side is round towards the form's left (+x).
        let other = apply(&back, centre_of(&shapes[1]), p(600.0, 100.0));
        assert!(other.x > 0.0);
        // Its twin, unplaced, mirrors it: the two meet at the centre line.
        let mut placed = pr.clone();
        placed.set_placement(PieceId(2), Some(back));
        let twin = effective(&placed, &geom::shapes(&placed)[2], &layout, 1.3);
        let twin_corner = apply(&twin, centre_of(&shapes[2]), p(1000.0, 100.0));
        near(twin_corner, corner);
    }

    #[test]
    fn a_folded_piece_is_centred_with_its_fold_on_the_centre_line() {
        let mut pr = Project::new();
        let mut half = Piece::rectangle(PieceId(0), "Front", p(100.0, 0.0), 250.0, 550.0);
        half.fold = Some(3); // the left edge, x = 100
        pr.add_piece(half);
        let shapes = geom::shapes(&pr);
        let layout = layout(&shapes);
        let placed = place_at(&pr, &shapes[0], PlaceAt::Front, &layout, 1.3, &cylinder);
        let on_fold = apply(&placed, centre_of(&shapes[0]), p(100.0, 200.0));
        assert!(on_fold.x.abs() < 1e-12 && on_fold.z > 0.0, "{on_fold}");
    }

    #[test]
    fn with_no_form_in_reach_place_at_uses_a_fallback_curve() {
        let pr = pattern();
        let shapes = geom::shapes(&pr);
        let layout = layout(&shapes);
        let placed = place_at(&pr, &shapes[0], PlaceAt::Front, &layout, 1.3, &|_, _| None);
        assert_eq!(placed.curve, Some(FALLBACK_RADIUS_M));
        assert!(placed.is_valid());
    }

    #[test]
    fn typed_angles_turn_about_x_then_y_then_z() {
        let q = DQuat::from_array(rotation_from_euler_xyz_deg([90.0, 90.0, 0.0]));
        near(q * DVec3::Y, DVec3::X); // y turned about x is z; z turned about y is x
        for degrees in [
            [10.0, 20.0, 30.0],
            [-45.0, 80.0, 170.0],
            [0.0, 0.0, -90.0],
            [0.0, 90.0, 0.0],
            [30.0, -90.0, 10.0],
        ] {
            let q = rotation_from_euler_xyz_deg(degrees);
            let back = rotation_from_euler_xyz_deg(euler_xyz_deg(q));
            let dot: f64 = q.iter().zip(&back).map(|(a, b)| a * b).sum();
            assert!(
                dot.abs() > 1.0 - 1e-9,
                "{degrees:?} → {:?}",
                euler_xyz_deg(q)
            );
        }
        let e = euler_xyz_deg(rotation_from_euler_xyz_deg([10.0, 20.0, 30.0]));
        for (a, b) in e.iter().zip([10.0, 20.0, 30.0]) {
            assert!((a - b).abs() < 1e-9, "{e:?}");
        }
    }
}
```

`crates/app/Cargo.toml`: under `[dependencies]`, after `opendrape-io.workspace = true`, add `opendrape-mesh.workspace = true`. `crates/app/src/lib.rs`: add `pub mod arrange;` after `mod app;`.

Create `crates/app/src/arrange/mod.rs`:

```rust
//! Arranging pieces in 3D before draping: the view's maths (`gizmo`).

pub mod gizmo;
```

Create `crates/app/src/arrange/gizmo.rs` with only its tests:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use glam::Vec3;

    /// 144 cameras: all round, from below and above, near and far.
    fn cameras() -> Vec<OrbitCamera> {
        let mut out = vec![];
        for yaw in [-2.8_f32, -1.6, -0.7, 0.0, 0.5, 1.2, 2.2, 3.1] {
            for pitch in [-1.2_f32, -0.5, 0.0, 0.3, 0.9, 1.4] {
                for distance in [1.0_f32, 2.6, 6.0] {
                    out.push(OrbitCamera {
                        target: Vec3::new(0.0, 0.95, 0.02),
                        yaw,
                        pitch,
                        distance,
                        fov_y: 35f32.to_radians(),
                    });
                }
            }
        }
        out
    }

    #[test]
    fn arrow_drags_move_the_dragged_distance_from_every_side() {
        let centre = DVec3::new(0.1, 1.0, 0.35);
        let (mut checked, mut skipped) = (0, 0);
        for c in cameras() {
            let cam = ScreenCamera::new(&c, DVec2::new(10.0, 40.0), DVec2::new(600.0, 520.0));
            for axis in AXES {
                for d in [0.02, 0.1, -0.25] {
                    let (Some(a), Some(b)) = (cam.project(centre), cam.project(centre + axis * d))
                    else {
                        continue;
                    };
                    let Some((dir, _)) = screen_axis(&cam, centre, axis) else {
                        continue;
                    };
                    // Wobbling 7 points sideways off the arrow changes nothing.
                    match axis_drag(&cam, centre, axis, a, b + dir.perp() * 7.0) {
                        Some(got) => {
                            checked += 1;
                            assert!((got - d).abs() < 1e-6 * d.abs().max(1.0), "{got} vs {d}");
                        }
                        None => skipped += 1,
                    }
                }
            }
        }
        assert!(
            checked > 1200 && skipped < 40,
            "{checked} checked, {skipped} skipped"
        );
    }

    #[test]
    fn an_arrow_pointing_at_the_viewer_cannot_be_dragged() {
        let c = OrbitCamera {
            target: Vec3::new(0.0, 1.0, 0.0),
            yaw: 0.0,
            pitch: 0.0,
            distance: 2.0,
            fov_y: 0.6,
        };
        let cam = ScreenCamera::new(&c, DVec2::ZERO, DVec2::new(800.0, 600.0));
        let centre = DVec3::new(0.0, 1.0, 0.0);
        let s = cam.project(centre).unwrap();
        assert_eq!(
            axis_drag(&cam, centre, DVec3::Z, s, s + DVec2::new(40.0, 0.0)),
            None
        );
        let gizmo = Gizmo::new(&cam, centre);
        assert!(!gizmo.arrow_shown(&cam, 2) && gizmo.arrow_shown(&cam, 0));
    }

    #[test]
    fn ring_drags_turn_the_dragged_angle_from_every_side() {
        let centre = DVec3::new(-0.05, 0.9, 0.3);
        let (mut checked, mut edge_on) = (0, 0);
        for c in cameras() {
            let cam = ScreenCamera::new(&c, DVec2::ZERO, DVec2::new(800.0, 600.0));
            for axis in AXES {
                if (centre - cam.eye()).normalize().dot(axis).abs() < EDGE_ON {
                    edge_on += 1;
                    continue;
                }
                let (u, v) = axis.any_orthonormal_pair();
                for (a0, a1) in [(0.3_f64, 1.1_f64), (2.0, 1.2), (-0.4, 0.9)] {
                    let on_ring = |a: f64| centre + (u * a.cos() + v * a.sin()) * 0.12;
                    let (Some(s0), Some(s1)) = (cam.project(on_ring(a0)), cam.project(on_ring(a1)))
                    else {
                        continue;
                    };
                    if let Some(got) = ring_angle(&cam, centre, axis, s0, s1) {
                        checked += 1;
                        assert!((got - (a1 - a0)).abs() < 1e-6, "{got} vs {}", a1 - a0);
                    }
                }
            }
        }
        assert!(
            checked > 900 && edge_on > 0,
            "{checked} checked, {edge_on} edge-on"
        );
    }

    #[test]
    fn an_edge_on_ring_turns_with_the_pointer_round_the_centre() {
        // Seen from the front, the z ring faces the viewer and the y ring is edge-on.
        let c = OrbitCamera {
            target: Vec3::new(0.0, 1.0, 0.0),
            yaw: 0.0,
            pitch: 0.0,
            distance: 2.0,
            fov_y: 0.6,
        };
        let cam = ScreenCamera::new(&c, DVec2::ZERO, DVec2::new(800.0, 600.0));
        let centre = DVec3::new(0.0, 1.0, 0.0);
        let s = cam.project(centre).unwrap();
        let (right, up) = (s + DVec2::new(50.0, 0.0), s + DVec2::new(0.0, -50.0));
        // Right to up is a quarter turn anticlockwise as the viewer sees it: +90° about +z.
        let z = ring_angle(&cam, centre, DVec3::Z, right, up).unwrap();
        assert!((z - std::f64::consts::FRAC_PI_2).abs() < 1e-6, "{z}");
        let y = ring_angle(&cam, centre, DVec3::Y, right, up).unwrap();
        assert!((y.abs() - std::f64::consts::FRAC_PI_2).abs() < 1e-6, "{y}");
        assert!((snap_angle(0.3, 15.0) - 15f64.to_radians()).abs() < 1e-12);
    }

    #[test]
    fn the_centre_square_moves_in_the_plane_facing_the_viewer() {
        let c = OrbitCamera {
            target: Vec3::new(0.0, 1.0, 0.0),
            yaw: 0.7,
            pitch: 0.3,
            distance: 2.5,
            fov_y: 0.6,
        };
        let cam = ScreenCamera::new(&c, DVec2::ZERO, DVec2::new(800.0, 600.0));
        let centre = DVec3::new(0.1, 1.1, 0.2);
        let target = centre + DVec3::new(0.05, -0.08, 0.0);
        let (from, to) = (cam.project(centre).unwrap(), cam.project(target).unwrap());
        let moved = plane_drag(&cam, centre, from, to).unwrap();
        // The new point is under the pointer, and the move is square to the view.
        let back = cam.project(centre + moved).unwrap();
        assert!(back.distance(to) < 1e-6, "{back} vs {to}");
        assert!(moved.dot(cam.forward).abs() < 1e-9);
    }

    #[test]
    fn a_point_projects_where_its_ray_comes_from() {
        let c = OrbitCamera::default();
        let cam = ScreenCamera::new(&c, DVec2::new(5.0, 5.0), DVec2::new(640.0, 480.0));
        let p = DVec3::new(0.2, 0.4, -0.3);
        let (o, d) = cam.ray(cam.project(p).unwrap());
        assert!((o + d * (p - o).length() - p).length() < 1e-6);
        // At the middle of the view, 100 points across is 100 × metres_per_point.
        let target = c.target.as_dvec3();
        let right = cam.forward.cross(DVec3::Y).normalize();
        let side = target + right * 100.0 * cam.metres_per_point(target);
        let gap = cam
            .project(side)
            .unwrap()
            .distance(cam.project(target).unwrap());
        assert!((gap - 100.0).abs() < 0.5, "{gap}");
    }

    #[test]
    fn a_ray_finds_the_triangle_in_front_of_it() {
        let tri = |z: f64| {
            [
                DVec3::new(-1.0, -1.0, z),
                DVec3::new(1.0, -1.0, z),
                DVec3::new(0.0, 1.0, z),
            ]
        };
        let o = DVec3::new(0.0, 0.0, 5.0);
        assert_eq!(ray_triangle(o, DVec3::NEG_Z, tri(0.0)), Some(5.0));
        assert_eq!(ray_triangle(o, DVec3::NEG_Z, tri(6.0)), None, "behind");
        assert_eq!(
            ray_triangle(DVec3::new(3.0, 0.0, 5.0), DVec3::NEG_Z, tri(0.0)),
            None
        );
    }

    #[test]
    fn every_handle_is_grabbed_where_it_is_drawn() {
        let c = OrbitCamera {
            target: Vec3::new(0.0, 1.0, 0.0),
            yaw: 0.6,
            pitch: 0.35,
            distance: 2.6,
            fov_y: 35f32.to_radians(),
        };
        let cam = ScreenCamera::new(&c, DVec2::new(0.0, 30.0), DVec2::new(700.0, 600.0));
        let gizmo = Gizmo::new(&cam, DVec3::new(0.0, 1.0, 0.3));
        let c2 = cam.project(gizmo.centre).unwrap();
        assert_eq!(
            gizmo.hit(&cam, c2 + DVec2::new(3.0, -2.0)),
            Some(Handle::Plane)
        );
        for axis in 0..3 {
            let tip = cam.project(gizmo.arrow_tip(axis)).unwrap();
            assert_eq!(
                gizmo.hit(&cam, tip),
                Some(Handle::Move(axis)),
                "arrow {axis}"
            );
        }
        for axis in 0..3 {
            let grabbed = gizmo
                .ring(axis)
                .into_iter()
                .filter_map(|p| cam.project(p))
                .filter(|s| gizmo.hit(&cam, *s) == Some(Handle::Turn(axis)))
                .count();
            // Most of the ring: only where an arrow or another ring is nearer is it not.
            assert!(grabbed > RING_STEPS / 3, "ring {axis}: {grabbed}");
        }
        assert_eq!(gizmo.hit(&cam, c2 + DVec2::new(200.0, 200.0)), None);
    }
}
```

Run: `cargo nextest run -p opendrape-mesh place && cargo nextest run -p opendrape --lib arrange`
Expected: compile errors.

- [ ] **Step 2: Placing** (`crates/mesh/src/place.rs`, above its tests)

```rust
//! Pieces in 3D, in the form's frame (metres, y up, the form faces +z, its left is +x, its
//! centre line at x = 0, z = 0): where a placement puts each flat point, where a piece starts
//! when nobody placed it, the mirror image a twin takes, Place at…, and the angles typed in
//! Properties.

use glam::{DMat3, DQuat, DVec3};
use opendrape_core::{MAX_CURVE_M, MIN_CURVE_M, Placement, Point2, Project};
use opendrape_geom::{self as geom, Shape, ShapeKind};

/// How far in front of the form's centre line the pattern starts (m).
pub const START_DISTANCE_M: f64 = 0.40;
/// The gap Place at… leaves between the form and a piece (m).
pub const PLACE_GAP_M: f64 = 0.03;
/// Place at… curves a piece this much (m) when no ray finds the form near it.
pub const FALLBACK_RADIUS_M: f64 = 0.2;

/// The middle of a shape's bounding box on the pattern table (mm): the point a placement puts
/// at its position.
pub fn centre_of(shape: &Shape) -> Point2 {
    let (lo, hi) = crate::bounds(&geom::outline_points(&shape.piece, 0.5));
    lo.lerp(hi, 0.5)
}

pub fn rotation(p: &Placement) -> DQuat {
    DQuat::from_array(p.rotation)
}

pub fn position(p: &Placement) -> DVec3 {
    DVec3::from_array(p.position)
}

/// Where flat point `flat` (mm) of a piece centred on `centre` (mm) goes: centred, wrapped
/// round the placement's cylinder (whose axis lies behind the piece, so its sides bend back),
/// turned, then moved to the placement's position.
pub fn apply(p: &Placement, centre: Point2, flat: Point2) -> DVec3 {
    let (x, y) = ((flat.x - centre.x) / 1000.0, (flat.y - centre.y) / 1000.0);
    let local = match p.curve {
        Some(r) => DVec3::new(r * (x / r).sin(), y, r * (x / r).cos() - r),
        None => DVec3::new(x, y, 0.0),
    };
    rotation(p) * local + position(p)
}

/// The placement that shows a piece's mirror image across x = 0, for its twin: the twin's flat
/// shape is the piece's mirror image, so mirroring the placement too keeps the pair mirrored.
pub fn mirrored(p: &Placement) -> Placement {
    let [x, y, z, w] = p.rotation;
    Placement {
        position: [-p.position[0], p.position[1], p.position[2]],
        rotation: [x, -y, -z, w],
        curve: p.curve,
    }
}

/// The whole pattern's bounding box on the table (mm).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Layout {
    pub min: Point2,
    pub max: Point2,
}

/// The bounding box of every shape's outline.
pub fn layout(shapes: &[Shape]) -> Layout {
    let points: Vec<Point2> = shapes
        .iter()
        .flat_map(|s| geom::outline_points(&s.piece, 0.5))
        .collect();
    let (min, max) = crate::bounds(&points);
    Layout { min, max }
}

/// Where a shape centred on `centre` (mm) starts when nobody has placed it: the pattern table
/// stood up at real size on a vertical plane 40 cm in front of the form, centred left to right
/// on it, with its top at `shoulder_y`.
pub fn start(layout: &Layout, centre: Point2, shoulder_y: f64) -> Placement {
    let middle = (layout.min.x + layout.max.x) / 2.0;
    Placement::at([
        (centre.x - middle) / 1000.0,
        shoulder_y - (layout.max.y - centre.y) / 1000.0,
        START_DISTANCE_M,
    ])
}

/// The placement `shape` is shown and draped with: its own; for a twin with none, its piece's
/// mirrored; otherwise its starting place.
pub fn effective(project: &Project, shape: &Shape, layout: &Layout, shoulder_y: f64) -> Placement {
    if let Some(own) = shape.piece.placement {
        return own;
    }
    if matches!(shape.kind, ShapeKind::Twin { .. })
        && let Some(piece) = project.placement_of(shape.source)
    {
        return mirrored(&piece);
    }
    start(layout, centre_of(shape), shoulder_y)
}

/// The four sides of the form Place at… knows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlaceAt {
    Front,
    Back,
    LeftSide,
    RightSide,
}

impl PlaceAt {
    pub const ALL: [Self; 4] = [Self::Front, Self::Back, Self::LeftSide, Self::RightSide];

    /// Radians round the form from its front towards its left (+x).
    pub fn angle(self) -> f64 {
        use std::f64::consts::{FRAC_PI_2, PI};
        match self {
            Self::Front => 0.0,
            Self::Back => PI,
            Self::LeftSide => FRAC_PI_2,
            Self::RightSide => -FRAC_PI_2,
        }
    }
}

/// Place at…: `shape` wrapped round the form at `at`, at the height it has now.
/// - It curves round a cylinder about the form's centre line, 3 cm clear of the form: the
///   largest surface distance (`surface(angle, y)`, from the centre line, if a ray finds the
///   form there) over the heights the piece spans and the angles it covers, plus the gap.
/// - A cut-on-fold piece (or any unpaired piece) is centred on that angle; a fold drawn
///   upright then lies on the centre line.
/// - A member of a mirrored pair goes beside its partner: its side that faces the partner on
///   the pattern table lies on the centre line at that angle.
pub fn place_at(
    project: &Project,
    shape: &Shape,
    at: PlaceAt,
    layout: &Layout,
    shoulder_y: f64,
    surface: &dyn Fn(f64, f64) -> Option<f64>,
) -> Placement {
    let height = effective(project, shape, layout, shoulder_y).position[1];
    let (lo, hi) = crate::bounds(&geom::outline_points(&shape.piece, 0.5));
    let width = (hi.x - lo.x) / 1000.0;
    let tall = (hi.y - lo.y) / 1000.0;
    let centre = lo.lerp(hi, 0.5);
    // The local x (m) that goes on the centre line: the side facing the partner, or the middle.
    let facing = partner_centre(project, shape).map_or(0.0, |partner| {
        if partner.x > centre.x {
            width / 2.0
        } else {
            -width / 2.0
        }
    });
    let theta = at.angle();
    let rows = ((tall / 0.02).ceil() as usize).max(1);
    let mut radius = FALLBACK_RADIUS_M;
    for _ in 0..3 {
        let middle = theta - facing / radius;
        let half_span = width / 2.0 / radius;
        let mut farthest: Option<f64> = None;
        for i in 0..=24 {
            let angle = middle - half_span + 2.0 * half_span * i as f64 / 24.0;
            for j in 0..=rows {
                let y = height - tall / 2.0 + tall * j as f64 / rows as f64;
                if let Some(d) = surface(angle, y).filter(|d| d.is_finite()) {
                    farthest = Some(farthest.map_or(d, |f: f64| f.max(d)));
                }
            }
        }
        radius = farthest
            .map_or(FALLBACK_RADIUS_M, |d| d + PLACE_GAP_M)
            .clamp(MIN_CURVE_M, MAX_CURVE_M);
    }
    let phi = theta - facing / radius;
    Placement {
        position: [radius * phi.sin(), height, radius * phi.cos()],
        rotation: DQuat::from_rotation_y(phi).to_array(),
        curve: Some(radius),
    }
}

/// The pattern-table centre of `shape`'s partner in a mirrored pair, if it is in one.
fn partner_centre(project: &Project, shape: &Shape) -> Option<Point2> {
    let partner = match shape.kind {
        ShapeKind::Twin { .. } => shape.source,
        _ => project.piece(shape.source)?.twin.as_ref()?.id,
    };
    geom::shape_of(project, partner).map(|s| centre_of(&s))
}

/// The rotation as angles (degrees) about x, then y, then z, as Properties shows them.
pub fn euler_xyz_deg(rotation: [f64; 4]) -> [f64; 3] {
    let m = DMat3::from_quat(DQuat::from_array(rotation).normalize());
    // m = Rz · Ry · Rx; its element in row r, column c is m.col(c)[r].
    let r20 = m.col(0).z;
    let (x, y, z) = if r20.abs() < 1.0 - 1e-9 {
        (
            m.col(1).z.atan2(m.col(2).z),
            (-r20).asin(),
            m.col(0).y.atan2(m.col(0).x),
        )
    } else {
        // Turned a quarter turn about y: x and z turn about the same axis; keep it all in z.
        let y = if r20 < 0.0 {
            std::f64::consts::FRAC_PI_2
        } else {
            -std::f64::consts::FRAC_PI_2
        };
        (0.0, y, (-m.col(1).x).atan2(m.col(1).y))
    };
    [x, y, z].map(f64::to_degrees)
}

/// The rotation that turns `degrees[0]` about x, then `degrees[1]` about y, then `degrees[2]`
/// about z (the world's axes), stored x, y, z, w.
pub fn rotation_from_euler_xyz_deg(degrees: [f64; 3]) -> [f64; 4] {
    let [x, y, z] = degrees.map(f64::to_radians);
    (DQuat::from_rotation_z(z) * DQuat::from_rotation_y(y) * DQuat::from_rotation_x(x)).to_array()
}
```

In `crates/mesh/src/lib.rs`'s `panel`, use the same middle as the placements: delete the line `let (lo, hi) = bounds(&geom::outline_points(&shape.piece, 0.5));` and set `centre: place::centre_of(shape),`.

- [ ] **Step 3: The gizmo's maths** (`crates/app/src/arrange/gizmo.rs`, above its tests)

```rust
//! The gizmo's maths, kept apart from egui so it can be tested on its own: the 3D view as seen
//! on screen, picking with a ray, the handles and where they are, and what dragging each one
//! means in metres or radians.

use glam::{DMat4, DVec2, DVec3, DVec4};
use opendrape_render::OrbitCamera;

/// The 3D view as it appears on screen: world points (m) ↔ screen points, for a camera drawn
/// into a rectangle at `min` of size `size` (screen points).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ScreenCamera {
    view_proj: DMat4,
    inverse: DMat4,
    min: DVec2,
    size: DVec2,
    eye: DVec3,
    forward: DVec3,
    fov_y: f64,
}

impl ScreenCamera {
    pub fn new(camera: &OrbitCamera, min: DVec2, size: DVec2) -> Self {
        let size = size.max(DVec2::ONE);
        let view_proj = camera.view_proj((size.x / size.y) as f32).as_dmat4();
        let eye = camera.eye().as_dvec3();
        Self {
            view_proj,
            inverse: view_proj.inverse(),
            min,
            size,
            eye,
            forward: (camera.target.as_dvec3() - eye).normalize_or(DVec3::NEG_Z),
            fov_y: f64::from(camera.fov_y),
        }
    }
    pub fn eye(&self) -> DVec3 {
        self.eye
    }
    /// The top-left corner of the view on screen, and its size (screen points).
    pub fn rect(&self) -> (DVec2, DVec2) {
        (self.min, self.size)
    }
    /// Where `p` shows on screen; None when it is behind the camera.
    pub fn project(&self, p: DVec3) -> Option<DVec2> {
        let c = self.view_proj * p.extend(1.0);
        if c.w <= 1e-9 {
            return None;
        }
        let ndc = c.truncate() / c.w;
        Some(self.min + DVec2::new((ndc.x + 1.0) * 0.5, (1.0 - ndc.y) * 0.5) * self.size)
    }
    /// The ray through screen point `s`: its start on the near plane and its unit direction.
    pub fn ray(&self, s: DVec2) -> (DVec3, DVec3) {
        let n = (s - self.min) / self.size;
        let (x, y) = (n.x * 2.0 - 1.0, 1.0 - n.y * 2.0);
        let at = |depth: f64| {
            let p: DVec4 = self.inverse * DVec4::new(x, y, depth, 1.0);
            p.truncate() / p.w
        };
        let (near, far) = (at(0.0), at(1.0));
        (near, (far - near).normalize_or(self.forward))
    }
    /// How many metres one screen point covers at the depth of `at`.
    pub fn metres_per_point(&self, at: DVec3) -> f64 {
        let depth = (at - self.eye).dot(self.forward).max(1e-3);
        2.0 * depth * (self.fov_y / 2.0).tan() / self.size.y
    }
}

/// Where the lines p + s·u and q + t·v (unit directions) come closest: the t on the second.
/// None when they are nearly parallel.
fn closest_on_second(p: DVec3, u: DVec3, q: DVec3, v: DVec3) -> Option<f64> {
    let w = p - q;
    let b = u.dot(v);
    let denom = 1.0 - b * b;
    if denom < 1e-6 {
        return None;
    }
    Some((v.dot(w) - b * u.dot(w)) / denom)
}

/// The on-screen unit direction of `axis` at `centre`, and how many screen points one metre
/// along it covers there.
fn screen_axis(cam: &ScreenCamera, centre: DVec3, axis: DVec3) -> Option<(DVec2, f64)> {
    let a = cam.project(centre)?;
    let b = cam.project(centre + axis * 0.01)?;
    let d = b - a;
    let len = d.length();
    (len > 1e-6).then(|| (d / len, len / 0.01))
}

/// Below this many screen points per metre an axis points (almost) straight at the viewer and
/// can't be dragged along: a centimetre would be under a fifth of a point.
const MIN_POINTS_PER_METRE: f64 = 20.0;

/// Metres moved along unit `axis` (through `centre`) when the pointer goes `from` → `to`. The
/// pointer's movement along the axis's on-screen direction is taken back onto the 3D axis
/// exactly (the pointer's ray against the axis line), so the piece stays under the pointer
/// however the axis is foreshortened. None when the axis points at the viewer.
pub fn axis_drag(
    cam: &ScreenCamera,
    centre: DVec3,
    axis: DVec3,
    from: DVec2,
    to: DVec2,
) -> Option<f64> {
    let (dir, per_metre) = screen_axis(cam, centre, axis)?;
    if per_metre < MIN_POINTS_PER_METRE {
        return None;
    }
    let origin = cam.project(centre)?;
    let along = |s: DVec2| {
        let on_axis = origin + dir * (s - origin).dot(dir);
        let (o, r) = cam.ray(on_axis);
        closest_on_second(o, r, centre, axis)
    };
    Some(along(to)? - along(from)?)
}

/// The move (m) in the plane facing the viewer through `centre` when the pointer goes
/// `from` → `to`: the centre square's drag.
pub fn plane_drag(cam: &ScreenCamera, centre: DVec3, from: DVec2, to: DVec2) -> Option<DVec3> {
    let normal = cam.forward;
    let hit = |s: DVec2| {
        let (o, r) = cam.ray(s);
        let denom = r.dot(normal);
        (denom.abs() > 1e-9).then(|| o + r * ((centre - o).dot(normal) / denom))
    };
    Some(hit(to)? - hit(from)?)
}

/// Below this |cos| between the view direction and a ring's axis, the ring is seen edge-on.
pub const EDGE_ON: f64 = 0.15;

/// Radians turned about unit `axis` through `centre` when the pointer goes `from` → `to`.
/// Seen at an angle, the pointer's rays meet the ring's plane and the angle between the two
/// hits is exact. Seen nearly edge-on (decided from the view direction at the centre, so the
/// method never changes during a drag), the angle the pointer turns round the centre on screen
/// is used. None when a ray misses the plane: keep the last angle.
pub fn ring_angle(
    cam: &ScreenCamera,
    centre: DVec3,
    axis: DVec3,
    from: DVec2,
    to: DVec2,
) -> Option<f64> {
    let view = (centre - cam.eye).normalize_or(cam.forward);
    if view.dot(axis).abs() >= EDGE_ON {
        let hit = |s: DVec2| {
            let (o, r) = cam.ray(s);
            let denom = r.dot(axis);
            if denom.abs() < 1e-9 {
                return None;
            }
            let t = (centre - o).dot(axis) / denom;
            (t > 0.0)
                .then(|| o + r * t - centre)
                .filter(|v| v.length() > 1e-9)
        };
        let (a, b) = (hit(from)?, hit(to)?);
        return Some(a.cross(b).dot(axis).atan2(a.dot(b)));
    }
    let c = cam.project(centre)?;
    let (a, b) = (from - c, to - c);
    if a.length() < 1e-9 || b.length() < 1e-9 {
        return None;
    }
    // Screen y points down, so negate to count anticlockwise as positive; a ring whose axis
    // points away from the viewer turns the other way.
    let screen = -(a.perp_dot(b)).atan2(a.dot(b));
    Some(if axis.dot(cam.eye - centre) >= 0.0 {
        screen
    } else {
        -screen
    })
}

/// `radians` to the nearest multiple of `step_deg` degrees.
pub fn snap_angle(radians: f64, step_deg: f64) -> f64 {
    let step = step_deg.to_radians();
    (radians / step).round() * step
}

/// Möller–Trumbore: how far along unit `dir` from `origin` the ray meets triangle `t`.
pub fn ray_triangle(origin: DVec3, dir: DVec3, t: [DVec3; 3]) -> Option<f64> {
    let (e1, e2) = (t[1] - t[0], t[2] - t[0]);
    let p = dir.cross(e2);
    let det = e1.dot(p);
    if det.abs() < 1e-12 {
        return None;
    }
    let inv = 1.0 / det;
    let s = origin - t[0];
    let u = s.dot(p) * inv;
    if !(0.0..=1.0).contains(&u) {
        return None;
    }
    let q = s.cross(e1);
    let v = dir.dot(q) * inv;
    if v < 0.0 || u + v > 1.0 {
        return None;
    }
    let d = e2.dot(q) * inv;
    (d > 1e-9).then_some(d)
}

/// The world's axes, x (red), y (green), z (blue).
pub const AXES: [DVec3; 3] = [DVec3::X, DVec3::Y, DVec3::Z];
/// Arrow length on screen (points).
pub const ARROW_PT: f64 = 80.0;
/// Ring radius on screen (points).
pub const RING_PT: f64 = 60.0;
/// Half the centre square's side (points).
pub const SQUARE_PT: f64 = 9.0;
/// How near (points) the pointer must be to grab an arrow or a ring.
pub const GRAB_PT: f64 = 8.0;
/// Points on a drawn ring.
pub const RING_STEPS: usize = 48;

/// A part of the gizmo.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Handle {
    /// The arrow along axis 0 (x), 1 (y) or 2 (z).
    Move(usize),
    /// The centre square: moves in the plane facing the viewer.
    Plane,
    /// The ring about axis 0, 1 or 2.
    Turn(usize),
}

/// The gizmo round a piece's centre, sized to look the same however far away it is.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Gizmo {
    pub centre: DVec3,
    /// World length (m) of an arrow.
    pub size: f64,
}

impl Gizmo {
    pub fn new(cam: &ScreenCamera, centre: DVec3) -> Self {
        Self {
            centre,
            size: ARROW_PT * cam.metres_per_point(centre),
        }
    }
    pub fn arrow_tip(&self, axis: usize) -> DVec3 {
        self.centre + AXES[axis] * self.size
    }
    /// An arrow is shown (and can be grabbed) unless it points nearly at the viewer.
    pub fn arrow_shown(&self, cam: &ScreenCamera, axis: usize) -> bool {
        match (cam.project(self.centre), cam.project(self.arrow_tip(axis))) {
            (Some(a), Some(b)) => a.distance(b) >= 0.15 * ARROW_PT,
            _ => false,
        }
    }
    /// The ring about `axis`, as [`RING_STEPS`] points.
    pub fn ring(&self, axis: usize) -> Vec<DVec3> {
        let (u, v) = AXES[axis].any_orthonormal_pair();
        let r = self.size * RING_PT / ARROW_PT;
        (0..RING_STEPS)
            .map(|k| {
                let a = k as f64 / RING_STEPS as f64 * std::f64::consts::TAU;
                self.centre + (u * a.cos() + v * a.sin()) * r
            })
            .collect()
    }
    /// The handle under screen point `pos`: the centre square first, then the nearest arrow or
    /// ring within [`GRAB_PT`] (an arrow wins a tie).
    pub fn hit(&self, cam: &ScreenCamera, pos: DVec2) -> Option<Handle> {
        let c = cam.project(self.centre)?;
        let d = (pos - c).abs();
        if d.x <= SQUARE_PT && d.y <= SQUARE_PT {
            return Some(Handle::Plane);
        }
        let mut best: Option<(Handle, f64)> = None;
        let mut consider = |handle: Handle, distance: f64| {
            if distance <= GRAB_PT && best.is_none_or(|(_, b)| distance < b) {
                best = Some((handle, distance));
            }
        };
        for axis in 0..3 {
            if self.arrow_shown(cam, axis)
                && let Some(tip) = cam.project(self.arrow_tip(axis))
            {
                consider(Handle::Move(axis), segment_distance(pos, c, tip));
            }
        }
        for axis in 0..3 {
            let ring: Vec<DVec2> = self
                .ring(axis)
                .into_iter()
                .filter_map(|p| cam.project(p))
                .collect();
            let nearest = (0..ring.len())
                .map(|k| segment_distance(pos, ring[k], ring[(k + 1) % ring.len()]))
                .fold(f64::INFINITY, f64::min);
            consider(Handle::Turn(axis), nearest);
        }
        best.map(|(h, _)| h)
    }
}

/// Distance from `p` to the segment `a`–`b` (screen points).
pub fn segment_distance(p: DVec2, a: DVec2, b: DVec2) -> f64 {
    let ab = b - a;
    let len2 = ab.length_squared();
    let t = if len2 < 1e-18 {
        0.0
    } else {
        ((p - a).dot(ab) / len2).clamp(0.0, 1.0)
    };
    p.distance(a + ab * t)
}
```

- [ ] **Step 4: Run and see them pass**

Run: `cargo nextest run -p opendrape-mesh && cargo nextest run -p opendrape --lib arrange`
Expected: 22 mesh tests and 8 gizmo tests pass. `arrow_drags_move_the_dragged_distance_from_every_side` checks more than 1,200 drags to 1e-6. `ring_drags_turn_the_dragged_angle_from_every_side` checks more than 900.

- [ ] **Step 5: Commit**

```bash
cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings
git add Cargo.lock crates/mesh crates/app
git commit -m "feat: placement, Place at and gizmo maths, exact from every camera angle

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 5: The Sew tool (review: full)

**Files:**
- Create: `crates/app/src/editor/sew_tool.rs`
- Modify:
  - `crates/app/src/editor/{mod.rs,canvas.rs,panel.rs,paint.rs}`
  - `crates/app/i18n/en-US/opendrape.ftl`
  - `crates/app/tests/sewing.rs`

**Interfaces:**
- Consumes:
  - Task 1: `Project::{add_seam, seam, seam_mut, remove_seam, seam_on, all_seams, owner}`, `SeamSide`, `Half`, `Shape::sew_edge`, `geom::{side_points, shapes, shape_of}`;
  - M2b: `geom::{nearest_edge, distance_along, edge_length}`, `Document::edit`, `note_if_refused`.
- Produces:
  - `Tool::Sew` (key **W**), last in `Tool::ALL` (now 7 tools).
  - `Selection::Seam(SeamId)`: its `piece()` is None, and `validated` keeps it only while the seam exists.
  - `CanvasState.sew: Option<SewDraft>` (private to the editor).
  - In `sew_tool.rs`:
    - `pub(super) struct SewDraft { a: SeamSide, seam: Option<SeamId> }`;
    - `pub(super) struct EdgeUnder { shape, half, edge, forward, distance }`;
    - `pub(super) fn edge_under(&Project, Point2, tol) -> Option<EdgeUnder>`;
    - `pub(super) fn extended(SeamSide, edge, n) -> Option<SeamSide>`;
    - `PatternEditor::sew_tool(&mut self, &Response, Option<Point2>, tol, shift)`;
    - `PatternEditor::sew_draft_side(&self) -> Option<SeamSide>`.
  - `delete_selection` deletes a selected seam (its mirror image goes with it).

**Behaviour:**
- **Clicks:**
  - The first click on a free edge starts side a. It runs away from the end nearer the click; on the pale half, where the outline runs against the stored edge, that flips `forward`.
  - Shift-click adds the next edge at either end of the side being built: side a while the seam is half made, then the seam's side b.
  - A plain click on another free edge makes the seam (one undo step) and selects it. A refused seam (its mirror image would sew an edge twice) shows `notice-refused`.
- **Ending:** Esc drops the draft. A click away from every edge ends extending; a half-made seam waits.
- **Refusals:**
  - an edge already sewn (mirror images included): "This edge is already sewn.";
  - a Shift-click not next to the side: "Shift-click an edge right next to this side, on the same piece.";
  - an edge of side a clicked as side b: "Pick a different edge for the other side of the seam."
- **Drawing:** the side being built is drawn thick in the selection colour, with a ring at its start.
- **Undo or a deleted piece:** a draft whose piece, edges or seam are gone is dropped (checked every frame).

- [ ] **Step 1: Strings** (append to the `.ftl`)

```
tool-sew = Sew
tool-sew-tip = Sew edges together: click an edge, then the edge it is sewn to.
hint-sew-start = Click an edge to start a seam. Shift-click the next edges along the outline to add them.
hint-sew-second = Click the edge to sew it to. Shift-click to add more edges to this side; Esc cancels.
hint-sew-extend = Shift-click to add edges to the second side, or click an edge to start another seam.
notice-already-sewn = This edge is already sewn.
notice-sew-not-next = Shift-click an edge right next to this side, on the same piece.
notice-sew-same-edge = Pick a different edge for the other side of the seam.
```

- [ ] **Step 2: Failing tests**

In `crates/app/tests/sewing.rs`, add `use opendrape::editor::{Selection, Tool};` to the imports and append:

```rust
fn seams(h: &H) -> Vec<Seam> {
    h.state().doc.project().seams.clone()
}

fn notice(h: &H) -> Option<String> {
    h.state().notice.clone()
}

#[test]
fn two_clicks_make_a_seam_whose_starts_meet() {
    let mut h = harness();
    let front = with_rectangle(&mut h); // edges: 0 bottom, 1 right, 2 top, 3 left
    let back = with_back(&mut h);
    key(&mut h, Key::W);
    assert_eq!(h.state().tool, Tool::Sew);
    click(&mut h, 401.0, 150.0); // the front's right edge, near its start (400,100)
    assert!(seams(&h).is_empty(), "half made");
    click(&mut h, 599.0, 150.0); // the back's left edge, near its end (600,100)
    let made = seams(&h);
    assert_eq!(
        made,
        vec![Seam {
            id: SeamId(1),
            a: side(front, Half::Drawn, 1, 1, true),
            b: side(back, Half::Drawn, 3, 1, false),
        }]
    );
    assert_eq!(h.state().selection, Selection::Seam(SeamId(1)));
    cmd(&mut h, Key::Z);
    assert!(seams(&h).is_empty(), "one undo step");
    assert_eq!(
        h.state().selection,
        Selection::None,
        "a removed seam is not selected"
    );
}

#[test]
fn shift_clicks_add_the_next_edges_to_either_side() {
    let mut h = harness();
    let front = with_rectangle(&mut h);
    let back = with_back(&mut h);
    key(&mut h, Key::W);
    click(&mut h, 401.0, 150.0); // front edge 1, forward
    shift_click(&mut h, 250.0, 501.0); // front edge 2, after it
    click(&mut h, 599.0, 150.0); // back edge 3, backwards
    shift_click(&mut h, 750.0, 501.0); // back edge 2, before it
    assert_eq!(seams(&h)[0].a, side(front, Half::Drawn, 1, 2, true));
    assert_eq!(seams(&h)[0].b, side(back, Half::Drawn, 2, 2, false));
    cmd(&mut h, Key::Z);
    assert_eq!(
        seams(&h)[0].b,
        side(back, Half::Drawn, 3, 1, false),
        "each edge added is a step"
    );
    // Clicking away from every edge ends extending: the next Shift-click starts a new side.
    click(&mut h, 250.0, 300.0);
    shift_click(&mut h, 750.0, 501.0);
    assert_eq!(seams(&h)[0].b, side(back, Half::Drawn, 3, 1, false));
}

#[test]
fn edges_that_are_sewn_or_not_next_are_refused() {
    let mut h = harness();
    with_rectangle(&mut h);
    with_back(&mut h);
    key(&mut h, Key::W);
    click(&mut h, 401.0, 150.0);
    shift_click(&mut h, 650.0, 101.0); // the back's bottom edge: another piece
    assert_eq!(
        notice(&h).as_deref(),
        Some("Shift-click an edge right next to this side, on the same piece.")
    );
    click(&mut h, 401.0, 450.0); // the first side's own edge
    assert_eq!(
        notice(&h).as_deref(),
        Some("Pick a different edge for the other side of the seam.")
    );
    click(&mut h, 599.0, 150.0);
    assert_eq!(seams(&h).len(), 1);
    click(&mut h, 401.0, 300.0); // sewn now
    assert_eq!(notice(&h).as_deref(), Some("This edge is already sewn."));
    assert_eq!(seams(&h).len(), 1);
}

#[test]
fn escape_cancels_a_half_made_seam() {
    let mut h = harness();
    with_rectangle(&mut h);
    with_back(&mut h);
    key(&mut h, Key::W);
    click(&mut h, 401.0, 150.0);
    key(&mut h, Key::Escape);
    click(&mut h, 599.0, 150.0); // starts a new seam instead of finishing one
    assert!(seams(&h).is_empty());
}

#[test]
fn mirrored_seams_appear_by_themselves() {
    let mut h = harness();
    let front = with_half(&mut h); // drawn half x 300..450, pale half x 150..300
    let back = with_back(&mut h);
    let twin = h
        .state_mut()
        .doc
        .edit(|p| p.add_twin(back, "Back (mirror)".into(), Point2::new(1900.0, 0.0)))
        .unwrap(); // the twin covers x 1000..1300
    h.state_mut().fit();
    h.run();
    key(&mut h, Key::W);
    click(&mut h, 451.0, 150.0); // the drawn half's right edge
    click(&mut h, 599.0, 150.0); // the back's left edge
    let all = h.state().doc.project().all_seams();
    assert_eq!(all.len(), 2);
    assert_eq!(all[1].0.a, side(front, Half::Pale, 1, 1, true));
    assert_eq!(all[1].0.b, side(twin, Half::Drawn, 3, 1, false));
    // The mirror image's edges are sewn too.
    click(&mut h, 149.0, 150.0); // the pale half's outer edge
    assert_eq!(notice(&h).as_deref(), Some("This edge is already sewn."));
    click(&mut h, 1301.0, 150.0); // the twin's matching edge
    assert_eq!(notice(&h).as_deref(), Some("This edge is already sewn."));
    // A twin's own free edge can be sewn: its bottom edge to the pale half's bottom.
    click(&mut h, 1150.0, 99.0);
    click(&mut h, 225.0, 99.0);
    assert_eq!(seams(&h).len(), 2);
    assert_eq!(seams(&h)[1].a.shape, twin);
    assert_eq!(seams(&h)[1].b, side(front, Half::Pale, 0, 1, false));
}

#[test]
fn delete_removes_the_selected_seam() {
    let mut h = harness();
    with_rectangle(&mut h);
    with_back(&mut h);
    key(&mut h, Key::W);
    click(&mut h, 401.0, 150.0);
    click(&mut h, 599.0, 150.0);
    key(&mut h, Key::Z); // the Edit tool: the seam stays selected
    key(&mut h, Key::Delete);
    assert!(seams(&h).is_empty());
    assert_eq!(h.state().selection, Selection::None);
}

#[test]
fn a_back_sewn_to_its_own_mirror_image_is_one_seam() {
    let mut h = harness();
    let back = with_back(&mut h); // 600..900; its twin at 1000..1300
    let twin = h
        .state_mut()
        .doc
        .edit(|p| p.add_twin(back, "Back (mirror)".into(), Point2::new(1900.0, 0.0)))
        .unwrap();
    h.state_mut().fit();
    h.run();
    key(&mut h, Key::W);
    click(&mut h, 901.0, 150.0); // the back's right edge, near its start (900,100)
    click(&mut h, 999.0, 450.0); // the twin's matching edge, near its far end: sewn twisted
    assert_eq!(notice(&h), None, "not refused");
    let all = h.state().doc.project().all_seams();
    assert_eq!(
        all.len(),
        1,
        "its own mirror image: drawn and stitched once"
    );
    assert_eq!(all[0].0.a, side(back, Half::Drawn, 1, 1, true));
    assert_eq!(all[0].0.b, side(twin, Half::Drawn, 1, 1, false));
}

#[test]
fn undo_and_deletion_drop_a_seam_selection_and_a_half_made_seam() {
    let mut h = harness();
    let front = with_rectangle(&mut h);
    with_back(&mut h);
    key(&mut h, Key::W);
    click(&mut h, 401.0, 150.0);
    click(&mut h, 599.0, 150.0);
    shift_click(&mut h, 750.0, 501.0); // extending the second side
    cmd(&mut h, Key::Z);
    cmd(&mut h, Key::Z); // the seam itself goes
    assert_eq!(h.state().selection, Selection::None);
    shift_click(&mut h, 750.0, 501.0); // nothing left to extend: starts a new side instead
    assert!(seams(&h).is_empty());
    // A half-made seam whose piece is deleted is dropped too.
    key(&mut h, Key::Escape);
    click(&mut h, 401.0, 150.0);
    h.state_mut().doc.edit(|p| p.remove_piece(front));
    h.run();
    click(&mut h, 599.0, 150.0); // starts a new seam: the old first side is gone
    assert!(seams(&h).is_empty());
}
```

Create `crates/app/src/editor/sew_tool.rs` with only its tests:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_side_grows_at_either_end_and_wraps() {
        let side = SeamSide::new(PieceId(1), Half::Drawn, 0, 1, true);
        assert_eq!(extended(side, 1, 4), Some(SeamSide { edges: 2, ..side }));
        assert_eq!(
            extended(side, 3, 4),
            Some(SeamSide {
                first_edge: 3,
                edges: 2,
                ..side
            })
        );
        assert_eq!(extended(side, 2, 4), None, "not next to it");
        let all = SeamSide::new(PieceId(1), Half::Drawn, 0, 4, true);
        assert_eq!(extended(all, 0, 4), None, "already the whole outline");
    }
}
```

Run: `cargo nextest run -p opendrape --test sewing`
Expected: compile errors (`Tool::Sew` and `Selection::Seam` don't exist yet).

- [ ] **Step 3: The Sew tool** (`crates/app/src/editor/sew_tool.rs`, above its tests)

```rust
//! The Sew tool (W). Click an edge, then the edge it is sewn to: two clicks make a seam.
//! Shift-click adds the next edge along the outline to the side being built: the first side,
//! or, once the seam is made, its second. The end of an edge nearer the first click on it is
//! where its side starts, and the two starts meet. Esc cancels a half-made seam; clicking away
//! from every edge ends extending.

use super::{PatternEditor, Selection};
use crate::tr;
use egui::Response;
use opendrape_core::{Half, PieceId, Point2, Project, SeamId, SeamSide};
use opendrape_geom as geom;

/// A seam being sewn.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct SewDraft {
    /// The first side, while it is built.
    pub a: SeamSide,
    /// The seam the second click made: Shift-clicks now add edges to its second side.
    pub seam: Option<SeamId>,
}

/// The outline edge nearest the pointer, as the Sew tool sees it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct EdgeUnder {
    pub shape: PieceId,
    pub half: Half,
    /// The stored edge.
    pub edge: usize,
    /// The pointer is nearer the stored edge's start: a side started here runs forward.
    pub forward: bool,
    /// How far (mm) the pointer is from the edge.
    pub distance: f64,
}

impl EdgeUnder {
    /// A side of just this edge, starting at its end nearer the pointer.
    fn side(&self) -> SeamSide {
        SeamSide::new(self.shape, self.half, self.edge, 1, self.forward)
    }
}

/// The outline edge of any shape nearest to `w`, within `tol` mm: the pale half of a fold
/// included (a fold's own edge is inside its shape, so it is never found).
pub(super) fn edge_under(project: &Project, w: Point2, tol: f64) -> Option<EdgeUnder> {
    geom::shapes(project)
        .iter()
        .filter_map(|s| {
            let (j, t, d) = geom::nearest_edge(&s.piece, w)?;
            if d > tol {
                return None;
            }
            let (half, edge, against) = s.sew_edge(j);
            let near_start =
                geom::distance_along(&s.piece, j, t) <= geom::edge_length(&s.piece, j) / 2.0;
            Some(EdgeUnder {
                shape: s.id,
                half,
                edge,
                forward: near_start != against,
                distance: d,
            })
        })
        .min_by(|a, b| a.distance.total_cmp(&b.distance))
}

/// `side` (on an outline of `n` edges) with stored edge `edge` added at whichever of its ends
/// the edge is next to; None when it is next to neither, or the side is the whole outline.
pub(super) fn extended(side: SeamSide, edge: usize, n: usize) -> Option<SeamSide> {
    if side.edges >= n {
        return None;
    }
    if edge == (side.first_edge + side.edges) % n {
        Some(SeamSide {
            edges: side.edges + 1,
            ..side
        })
    } else if (edge + 1) % n == side.first_edge {
        Some(SeamSide {
            first_edge: edge,
            edges: side.edges + 1,
            ..side
        })
    } else {
        None
    }
}

/// Whether the draft still fits the project (an undo or a deleted piece may have taken its
/// edges or its seam away).
fn draft_fits(project: &Project, draft: &SewDraft) -> bool {
    let fits = project.owner(draft.a.shape).is_some_and(|(p, _)| {
        draft.a.first_edge < p.len()
            && draft.a.edges <= p.len()
            && (draft.a.half == Half::Drawn || p.fold.is_some())
    });
    fits && draft.seam.is_none_or(|id| project.seam(id).is_some())
}

impl PatternEditor {
    pub(super) fn sew_tool(
        &mut self,
        response: &Response,
        pointer: Option<Point2>,
        tol: f64,
        shift: bool,
    ) {
        let project = self.doc.project();
        self.canvas.sew = self.canvas.sew.filter(|d| draft_fits(project, d));
        if !response.clicked() {
            return;
        }
        let Some(at) = pointer else { return };
        let Some(hit) = edge_under(self.doc.project(), at, tol) else {
            // A click away from every edge ends extending; a half-made seam waits for its
            // second edge.
            if self.canvas.sew.is_some_and(|d| d.seam.is_some()) {
                self.canvas.sew = None;
            }
            return;
        };
        let project = self.doc.project();
        let n = project.owner(hit.shape).map_or(0, |(p, _)| p.len());
        let draft = self.canvas.sew;
        let in_first_side = draft.is_some_and(|d| {
            d.seam.is_none()
                && d.a.shape == hit.shape
                && d.a.half == hit.half
                && d.a.covers(n, hit.edge)
        });
        if in_first_side {
            self.notice = Some(tr!("notice-sew-same-edge"));
            return;
        }
        if project.seam_on(hit.shape, hit.half, hit.edge).is_some() {
            self.notice = Some(tr!("notice-already-sewn"));
            return;
        }
        match (draft, shift) {
            (Some(SewDraft { a, seam: None }), true) => {
                let same_outline = a.shape == hit.shape && a.half == hit.half;
                match same_outline.then(|| extended(a, hit.edge, n)).flatten() {
                    Some(a) => self.canvas.sew = Some(SewDraft { a, seam: None }),
                    None => self.notice = Some(tr!("notice-sew-not-next")),
                }
            }
            (Some(SewDraft { seam: Some(id), .. }), true) => self.extend_second_side(id, hit, n),
            (Some(SewDraft { a, seam: None }), false) => self.make_seam(a, hit),
            _ => {
                self.canvas.sew = Some(SewDraft {
                    a: hit.side(),
                    seam: None,
                })
            }
        }
    }

    /// Sews the finished first side `a` to the clicked edge, as one undo step.
    fn make_seam(&mut self, a: SeamSide, hit: EdgeUnder) {
        let id = self.doc.edit(|p| p.add_seam(a, hit.side()));
        // Refused when the seam's mirror image would sew an edge that is already sewn.
        if self.note_if_refused() {
            return;
        }
        self.canvas.sew = Some(SewDraft { a, seam: Some(id) });
        self.selection = Selection::Seam(id);
    }

    /// Adds the clicked edge to seam `id`'s second side, as one undo step.
    fn extend_second_side(&mut self, id: SeamId, hit: EdgeUnder, n: usize) {
        let grown = self.doc.edit(|p| {
            let seam = p.seam_mut(id)?;
            let same_outline = seam.b.shape == hit.shape && seam.b.half == hit.half;
            seam.b = same_outline
                .then(|| extended(seam.b, hit.edge, n))
                .flatten()?;
            Some(())
        });
        if !self.note_if_refused() && grown.is_none() {
            self.notice = Some(tr!("notice-sew-not-next"));
        }
    }

    /// The side being built, to draw: the first while the seam is half made, then the second.
    pub(super) fn sew_draft_side(&self) -> Option<SeamSide> {
        let draft = self.canvas.sew?;
        match draft.seam {
            None => Some(draft.a),
            Some(id) => self.doc.project().seam(id).map(|s| s.b),
        }
    }
}
```

- [ ] **Step 4: Wire it in**

`crates/app/src/editor/mod.rs`:
- add `mod sew_tool;` after `mod panel;`;
- the core import becomes `use opendrape_core::{PieceId, Point2, Project, SeamId, Units};`;
- `enum Tool` gains `Sew` after `Line`; `Tool::ALL` becomes `[Self; 7]` with `Self::Sew` last;
- `key()` gets the arm `// S is the Rectangle's.` followed by `Self::Sew => Key::W,`;
- `label()` gets `Self::Sew => tr!("tool-sew"),` and `tip()` gets `Self::Sew => tr!("tool-sew-tip"),`;
- `enum Selection` gains, after `Line`:

```rust
    /// A stored seam (clicking a seam's mirror image selects the seam).
    Seam(SeamId),
```

- in `Selection::piece`, the first arm becomes `Self::None | Self::Seam(_) => None,`;
- `Selection::validated` starts with:

```rust
        if let Self::Seam(id) = self {
            return if project.seam(id).is_some() {
                self
            } else {
                Self::None
            };
        }
```

`crates/app/src/editor/canvas.rs`:
- `CanvasState` gains, after `line_dragging`:

```rust
    /// Sew tool: the seam being sewn.
    pub sew: Option<super::sew_tool::SewDraft>,
```

- in `canvas_ui`, the cursor match's last arm becomes `Tool::Edit | Tool::AddPoint | Tool::Notch | Tool::Line | Tool::Sew => w,`, and the tool match gains `Tool::Sew => self.sew_tool(&response, pointer, tol, shift),`;
- in `canvas_keys`, before `_ => {}`: `Tool::Sew if pressed(Key::Escape) => self.canvas.sew = None,`;
- `delete_selection`'s doc becomes "Deletes the selected point, notch, line, seam or piece. A piece keeps at least 3 points.", and it gets, before the `Selection::Edge(..) | Selection::None => {}` arm:

```rust
            Selection::Seam(id) => {
                // Its mirror image goes with it.
                self.doc.edit(|p| p.remove_seam(id));
                if !self.note_if_refused() {
                    self.selection = Selection::None;
                }
            }
```

`crates/app/src/editor/panel.rs`:
- `properties()` routes `Selection::Seam(_) => {}`, with the comment `// The seam panel comes with seam drawing.` (Task 6 fills it in);
- `hint()` gets:

```rust
            Tool::Sew => match self.canvas.sew {
                None => tr!("hint-sew-start"),
                Some(draft) if draft.seam.is_none() => tr!("hint-sew-second"),
                Some(_) => tr!("hint-sew-extend"),
            },
```

`crates/app/src/editor/paint.rs`:
- in `paint_selection`, the last arm becomes `Selection::Piece(_) | Selection::Seam(_) | Selection::None => {}`;
- at the end of `paint_drafts`:

```rust
        // The seam side being sewn, thick, with a ring where it starts.
        if let Some(side) = self.sew_draft_side()
            && let Some(shape) = geom::shape_of(self.doc.project(), side.shape)
            && let Some(points) = geom::side_points(&shape, &side, v.mm(0.25))
            && let Some(start) = points.first().copied()
        {
            painter.add(Shape::line(
                self.screen_points(rect, points),
                Stroke::new(4.0, c.selected),
            ));
            painter.circle_stroke(v.to_screen(rect, start), 6.0, ink);
        }
```

- [ ] **Step 5: Run and see them pass**

Run: `cargo nextest run -p opendrape`
Expected: the 8 new sewing tests and the unit test pass, plus all earlier ones.

- [ ] **Step 6: Commit**

```bash
cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings
git add crates/app
git commit -m "feat(editor): Sew tool: two clicks make a seam, Shift-click extends, mirror seams appear

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 6: Seams drawn on the pattern, picked by their line, and the seam panel (review: light)

**Files:**
- Create: `crates/app/src/editor/seams.rs`
- Modify:
  - `crates/app/src/editor/{mod.rs,canvas.rs,sew_tool.rs,paint.rs,panel.rs}`
  - `crates/app/i18n/en-US/opendrape.ftl`
  - `crates/app/tests/sewing.rs`

**Interfaces:**
- Consumes:
  - Task 1: `Project::{all_seams, seam, seam_mut}`, `geom::{side_edges, side_length}`;
  - Task 3: `opendrape_mesh::LENGTH_WARNING_MM`;
  - Task 5: `Selection::Seam`, `delete_selection` for seams, `SewDraft`.
- Produces:
  - In `editor/seams.rs`:
    - `pub(super) const INSET_PX: f64 = 5.0` and `pub(super) const SEAM_COLOURS: [Color32; 8]`;
    - `pub(super) struct SeamLine { id: SeamId, colour: Color32, sides: [Vec<Point2>; 2] }`;
    - `pub(super) fn seam_lines(&Project, &[Shape], inset_mm, tolerance) -> Vec<SeamLine>`;
    - `pub(super) fn polyline_distance(Point2, &[Point2]) -> f64`.
  - `PatternEditor::seam_lines(&self) -> Vec<SeamLine>` and `PatternEditor::seam_at(&self, w, tol) -> Option<SeamId>`.

**Behaviour:**
- **Drawing:**
  - Each seam and its mirror image is drawn in the seam's colour (the palette in turn, by position among the stored seams), as a 2.5-point line 5 points inside each side.
  - The seam's number (its id) sits on a filled circle halfway along each side.
  - The selected seam is drawn 4.5 points wide, with thin guides joining a's start to b's start and a's end to b's end; a twisted seam shows crossed guides.
- **Picking:**
  - In the Edit tool, a point, handle or notch under the pointer still wins.
  - Otherwise a seam line within 8 points that is nearer than any outline edge by a screen point selects the seam. A click on the outline itself still selects the edge.
  - In the Sew tool, a click on a seam line selects the seam and ends any draft.
- **The seam panel** (Selection::Seam):
  - "Seam 1", "Side 1: 40.0 cm", "Side 2: 40.0 cm";
  - in amber, "Lengths differ by 4.0 cm" when the sides differ by more than 3 mm;
  - **Flip** (one undo step: `b.forward` toggled) and **Delete seam**.
- **Delete:** the key deletes a selected seam in the Sew tool too.

- [ ] **Step 1: Strings** (append to the `.ftl`)

```
panel-seam = Seam { $number }
panel-seam-side = Side { $side }: { $length }
panel-seam-differ = Lengths differ by { $difference }
panel-seam-flip = Flip
panel-delete-seam = Delete seam
```

- [ ] **Step 2: Failing tests** (append to `crates/app/tests/sewing.rs`)

```rust
/// The front's right edge (400 mm) sewn to the left edge of a back `height` mm tall.
fn sewn_pair(h: &mut H, height: f64) -> (PieceId, PieceId, SeamId) {
    let front = with_rectangle(h);
    let back = h.state_mut().doc.edit(|p| {
        p.add_piece(Piece::rectangle(
            PieceId(0),
            "Back",
            Point2::new(600.0, 100.0),
            300.0,
            height,
        ))
    });
    let seam = sew(
        h,
        side(front, Half::Drawn, 1, 1, true),
        side(back, Half::Drawn, 3, 1, false),
    );
    (front, back, seam)
}

/// Where a seam's line runs inside the front's right edge (x = 400): 5 screen points in.
fn on_seam_line(h: &H) -> f64 {
    400.0 - 5.0 / h.state().view.zoom
}

#[test]
fn clicking_a_seam_line_selects_the_seam_and_shows_both_lengths() {
    let mut h = harness();
    let (front, _, seam) = sewn_pair(&mut h, 400.0);
    let x = on_seam_line(&h);
    click(&mut h, x, 300.0);
    assert_eq!(h.state().selection, Selection::Seam(seam));
    h.get_by_label("Seam 1");
    h.get_by_label("Side 1: 40.0 cm");
    h.get_by_label("Side 2: 40.0 cm");
    assert!(h.query_by_label_contains("Lengths differ").is_none());
    // On the outline itself, the edge wins.
    click(&mut h, 400.0, 300.0);
    assert_eq!(h.state().selection, Selection::Edge(front, 1));
}

#[test]
fn sides_more_than_3_mm_apart_in_length_are_flagged() {
    let mut h = harness();
    let (_, _, seam) = sewn_pair(&mut h, 440.0);
    h.state_mut().selection = Selection::Seam(seam);
    h.run();
    h.get_by_label("Lengths differ by 4.0 cm");
    let mut close = harness();
    let (_, _, seam) = sewn_pair(&mut close, 402.0);
    close.state_mut().selection = Selection::Seam(seam);
    close.run();
    assert!(
        close.query_by_label_contains("Lengths differ").is_none(),
        "2 mm is fine"
    );
}

#[test]
fn flip_turns_the_second_side_round_as_one_step() {
    let mut h = harness();
    let (_, back, seam) = sewn_pair(&mut h, 400.0);
    h.state_mut().selection = Selection::Seam(seam);
    h.run();
    h.get_by_label("Flip").click();
    h.run();
    assert_eq!(
        seam_of(&h, seam).unwrap().b,
        side(back, Half::Drawn, 3, 1, true)
    );
    cmd(&mut h, Key::Z);
    assert_eq!(
        seam_of(&h, seam).unwrap().b,
        side(back, Half::Drawn, 3, 1, false)
    );
}

#[test]
fn delete_seam_takes_its_mirror_image_too() {
    let mut h = harness();
    let front = with_half(&mut h);
    let back = with_back(&mut h);
    h.state_mut()
        .doc
        .edit(|p| p.add_twin(back, "Back (mirror)".into(), Point2::new(1900.0, 0.0)));
    let seam = sew(
        &mut h,
        side(front, Half::Drawn, 1, 1, true),
        side(back, Half::Drawn, 3, 1, false),
    );
    assert_eq!(h.state().doc.project().all_seams().len(), 2);
    h.state_mut().selection = Selection::Seam(seam);
    h.run();
    h.get_by_label("Delete seam").click();
    h.run();
    assert!(h.state().doc.project().all_seams().is_empty());
    assert_eq!(h.state().selection, Selection::None);
}

#[test]
fn the_sew_tool_picks_a_seam_by_its_line_and_delete_removes_it() {
    let mut h = harness();
    let (_, _, seam) = sewn_pair(&mut h, 400.0);
    key(&mut h, Key::W);
    let x = on_seam_line(&h);
    click(&mut h, x, 300.0);
    assert_eq!(h.state().selection, Selection::Seam(seam));
    key(&mut h, Key::Delete);
    assert!(seams(&h).is_empty());
}

#[test]
fn seams_draw_on_halves_twins_and_round_corners() {
    let mut h = harness();
    let front = with_half(&mut h);
    let back = with_back(&mut h);
    let twin = h
        .state_mut()
        .doc
        .edit(|p| p.add_twin(back, "Back (mirror)".into(), Point2::new(1900.0, 0.0)))
        .unwrap();
    // Two edges round a corner of the pale half, and a twin's edges wrapping past its last.
    let seam = sew(
        &mut h,
        side(front, Half::Pale, 0, 2, false),
        side(twin, Half::Drawn, 3, 2, true),
    );
    for selection in [Selection::Seam(seam), Selection::None] {
        h.state_mut().selection = selection;
        h.run();
    }
    assert_eq!(h.state().doc.project().all_seams().len(), 2);
}
```

Run: `cargo nextest run -p opendrape --test sewing`
Expected: failures (no seam lines to click, and no seam panel).

- [ ] **Step 3: Seam lines** (`crates/app/src/editor/seams.rs`)

```rust
//! Seams on the pattern table: each drawn as a coloured line just inside both of its sides,
//! with a number badge, and picked by clicking that line. A mirror image is drawn like its
//! seam, in its seam's colour, and picks its seam.

use super::{HIT_PX, PatternEditor};
use egui::Color32;
use opendrape_core::{Point2, Project, SeamId, SeamSide};
use opendrape_geom::{self as geom, Shape};

/// How far inside the outline (screen points) a seam's line runs, so the outline itself can
/// still be clicked.
pub(super) const INSET_PX: f64 = 5.0;

/// Each seam's colour, in turn.
pub(super) const SEAM_COLOURS: [Color32; 8] = [
    Color32::from_rgb(0, 150, 136),
    Color32::from_rgb(156, 39, 176),
    Color32::from_rgb(33, 120, 243),
    Color32::from_rgb(67, 160, 71),
    Color32::from_rgb(216, 27, 96),
    Color32::from_rgb(121, 85, 72),
    Color32::from_rgb(63, 81, 181),
    Color32::from_rgb(190, 145, 0),
];

/// A seam or a mirror image, ready to draw and pick.
pub(super) struct SeamLine {
    /// The stored seam (a mirror image's is its seam's).
    pub id: SeamId,
    pub colour: Color32,
    /// Each side's line (mm), from its start to its end, just inside the outline.
    pub sides: [Vec<Point2>; 2],
}

/// Every seam and mirror image of `project` whose sides fit their shapes, its lines `inset` mm
/// inside the outline and within `tolerance` mm of the curves.
pub(super) fn seam_lines(
    project: &Project,
    shapes: &[Shape],
    inset: f64,
    tolerance: f64,
) -> Vec<SeamLine> {
    let mut stored = 0;
    let mut out = Vec::new();
    for (seam, mirrored) in project.all_seams() {
        if !mirrored {
            stored += 1;
        }
        let line = |side: &SeamSide| {
            let shape = shapes.iter().find(|s| s.id == side.shape)?;
            inset_side(shape, side, inset, tolerance)
        };
        if let (Some(a), Some(b)) = (line(&seam.a), line(&seam.b)) {
            out.push(SeamLine {
                id: seam.id,
                colour: SEAM_COLOURS[(stored - 1) % SEAM_COLOURS.len()],
                sides: [a, b],
            });
        }
    }
    out
}

/// A side's line moved `inset` mm into its shape, from the side's start to its end.
fn inset_side(shape: &Shape, side: &SeamSide, inset: f64, tolerance: f64) -> Option<Vec<Point2>> {
    let ccw = geom::is_counter_clockwise(&shape.piece);
    let mut out = Vec::new();
    for (j, against) in geom::side_edges(shape, side)? {
        let points = geom::edge_points(&shape.piece, j, tolerance);
        let last = points.len() - 1;
        let mut moved: Vec<Point2> = (0..points.len())
            .map(|k| {
                // Inwards is to the left of the outline's direction when it runs anticlockwise.
                let d = points[(k + 1).min(last)] - points[k.saturating_sub(1)];
                let left = Point2::new(-d.y, d.x) * (1.0 / d.length().max(1e-12));
                points[k] + left * if ccw { inset } else { -inset }
            })
            .collect();
        if against {
            moved.reverse();
        }
        out.extend(moved);
    }
    Some(out)
}

/// Distance (mm) from `p` to the polyline `points`.
pub(super) fn polyline_distance(p: Point2, points: &[Point2]) -> f64 {
    points
        .windows(2)
        .map(|w| {
            let (a, b) = (w[0], w[1]);
            let ab = b - a;
            let len2 = ab.x * ab.x + ab.y * ab.y;
            let t = if len2 < 1e-18 {
                0.0
            } else {
                (((p.x - a.x) * ab.x + (p.y - a.y) * ab.y) / len2).clamp(0.0, 1.0)
            };
            p.distance(a + ab * t)
        })
        .fold(f64::INFINITY, f64::min)
}

impl PatternEditor {
    /// The seam lines as they are drawn now.
    pub(super) fn seam_lines(&self) -> Vec<SeamLine> {
        seam_lines(
            self.doc.project(),
            &self.shapes(),
            self.view.mm(INSET_PX),
            self.view.mm(0.25),
        )
    }

    /// The stored seam whose line is within `tol` mm of `w` and nearer to it than any outline
    /// edge by a screen point.
    pub(super) fn seam_at(&self, w: Point2, tol: f64) -> Option<SeamId> {
        let (id, d) = self
            .seam_lines()
            .iter()
            .flat_map(|l| l.sides.iter().map(move |s| (l.id, polyline_distance(w, s))))
            .min_by(|a, b| a.1.total_cmp(&b.1))?;
        let edge = self
            .shapes()
            .iter()
            .filter_map(|s| geom::nearest_edge(&s.piece, w).map(|e| e.2))
            .fold(f64::INFINITY, f64::min);
        (d <= tol && d + tol / HIT_PX < edge).then_some(id)
    }
}
```

Add `mod seams;` after `mod panel;` in `crates/app/src/editor/mod.rs`.

- [ ] **Step 4: Picking seams**

`crates/app/src/editor/canvas.rs`: in `edit_tool`, replace the final `if response.clicked() … { self.selection = self.hit(…)…; }` block with:

```rust
        if response.clicked()
            && let Some(at) = pointer
        {
            let hit = self.hit(&self.shapes(), at, tol);
            // A point, handle or notch under the pointer comes first; then a seam's line
            // (drawn just inside the outline), when it is nearer than the outline itself.
            let on_a_point = matches!(
                hit,
                Some(
                    Hit::Vertex(..)
                        | Hit::Handle(..)
                        | Hit::Notch(..)
                        | Hit::LineVertex(..)
                        | Hit::LineHandle(..)
                )
            );
            self.selection = match self.seam_at(at, tol) {
                Some(seam) if !on_a_point => Selection::Seam(seam),
                _ => hit.map_or(Selection::None, Hit::selection),
            };
        }
```

In `canvas_keys`, replace the arm `Tool::Sew if pressed(Key::Escape) => self.canvas.sew = None,` with:

```rust
            Tool::Sew => {
                if pressed(Key::Escape) {
                    self.canvas.sew = None;
                }
                // Only a seam: a piece or point selected earlier is the Edit tool's to delete.
                if matches!(self.selection, Selection::Seam(_)) && delete_pressed() {
                    self.delete_selection();
                }
            }
```

`crates/app/src/editor/sew_tool.rs`: in `sew_tool`, right after `let Some(at) = pointer else { return };`, add:

```rust
        // A click on a seam's line selects that seam.
        if let Some(seam) = self.seam_at(at, tol) {
            self.selection = Selection::Seam(seam);
            self.canvas.sew = None;
            return;
        }
```

- [ ] **Step 5: Drawing seams** (`crates/app/src/editor/paint.rs`)

In `paint`, call `self.paint_seams(painter, rect);` right before `self.paint_drafts(painter, rect, &c);`, and add to `impl PatternEditor`:

```rust
    /// Every seam and mirror image: a line in its colour just inside each side, with the seam's
    /// number halfway along. The selected seam's lines are thicker, and thin guides join its
    /// two starts and its two ends: they cross when the seam is sewn twisted.
    fn paint_seams(&self, painter: &Painter, rect: Rect) {
        for line in self.seam_lines() {
            let selected = self.selection == Selection::Seam(line.id);
            let width = if selected { 4.5 } else { 2.5 };
            for side in &line.sides {
                painter.add(Shape::line(
                    self.screen_points(rect, side.clone()),
                    Stroke::new(width, line.colour),
                ));
                if let Some(middle) = side.get(side.len() / 2) {
                    let at = self.view.to_screen(rect, *middle);
                    painter.circle_filled(at, 7.5, line.colour);
                    painter.text(
                        at,
                        Align2::CENTER_CENTER,
                        line.id.0.to_string(),
                        FontId::proportional(10.0),
                        Color32::WHITE,
                    );
                }
            }
            if selected {
                let [a, b] = &line.sides;
                for (p, q) in [(a.first(), b.first()), (a.last(), b.last())] {
                    if let (Some(p), Some(q)) = (p, q) {
                        painter.line_segment(
                            [self.view.to_screen(rect, *p), self.view.to_screen(rect, *q)],
                            Stroke::new(1.0, line.colour),
                        );
                    }
                }
            }
        }
    }
```

- [ ] **Step 6: The seam panel** (`crates/app/src/editor/panel.rs`)

- The core import gains `SeamId` and `SeamSide`.
- `properties()` routes `Selection::Seam(id) => self.seam_properties(ui, id),` in place of Task 5's `Selection::Seam(_) => {}`.
- Add:

```rust
    /// A seam: each side's length, a warning when they differ by more than 3 mm, Flip and
    /// Delete.
    fn seam_properties(&mut self, ui: &mut egui::Ui, id: SeamId) {
        let project = self.doc.project();
        let Some(seam) = project.seam(id).copied() else {
            return;
        };
        let units = project.units;
        let length = |side: &SeamSide| {
            geom::shape_of(project, side.shape).and_then(|s| geom::side_length(&s, side))
        };
        let (Some(a), Some(b)) = (length(&seam.a), length(&seam.b)) else {
            return;
        };
        ui.strong(tr!("panel-seam", number = id.0));
        ui.label(tr!("panel-seam-side", side = 1, length = units.format(a)));
        ui.label(tr!("panel-seam-side", side = 2, length = units.format(b)));
        let difference = (a - b).abs();
        if difference > opendrape_mesh::LENGTH_WARNING_MM {
            ui.colored_label(
                ui.visuals().warn_fg_color,
                tr!("panel-seam-differ", difference = units.format(difference)),
            );
        }
        ui.add_space(6.0);
        if ui.button(tr!("panel-seam-flip")).clicked() {
            self.doc.edit(|p| {
                if let Some(s) = p.seam_mut(id) {
                    s.b.forward = !s.b.forward;
                }
            });
            self.note_if_refused();
        }
        if ui.button(tr!("panel-delete-seam")).clicked() {
            self.delete_selection();
        }
    }
```

- [ ] **Step 7: Run and see them pass**

Run: `cargo nextest run -p opendrape`
Expected: the 6 new tests pass, plus all earlier ones. `a_sewn_edge_cannot_become_the_fold` still selects the edge, because it clicks on the outline itself.

- [ ] **Step 8: Commit**

```bash
cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings
git add crates/app
git commit -m "feat(editor): seams drawn in colour with numbers, picked by their line; the seam panel

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 7: The `Stage` boundary, and draping the student's project (review: full)

**Files:**
- Create: `crates/app/src/stage.rs`
- Replace: `crates/app/src/sim_runner.rs`
- Modify:
  - `crates/app/src/{app.rs,viewport.rs,lib.rs,main.rs}`
  - `crates/app/Cargo.toml`
  - `crates/app/i18n/en-US/opendrape.ftl`
  - `crates/app/tests/ui.rs`

**Interfaces:**
- Consumes:
  - Task 3: `opendrape_mesh::{build, MeshParams, MeshNote}`, `PanelMesh.{flat, triangles, centre}`, `GarmentMesh.stitches`;
  - Task 4: `place::{layout, effective, apply}`;
  - read-only from the dress-forms-owned crates: `opendrape_body::BodyMesh::female_average`, `opendrape_sim::{BodyCollider, Collider, Plane, ClothBuilder, Panel, Params, Solver, FRAME_DT}` and `BodyCollider::{new, ray_exit, signed_distance, contact_planes}`.
- Produces:
  - `opendrape::stage::Stage`:
    - `makehuman()`, `shared() -> Arc<Stage>` (built once per process);
    - `render_mesh() -> (&[Vec3], &[[u32; 3]])`, `collider() -> &BodyCollider`, `drape_collider() -> BodyAndFloor<'_>`;
    - `centre_line() -> (f64, f64)` (always (0, 0): the frame is the form's), `floor_y() -> f64` (0), `shoulder_y() -> f64`;
    - `surface_distance(angle, y) -> Option<f64>`, `signed_distance(DVec3) -> f64`.
  - `opendrape::stage::BodyAndFloor<'a>`: implements `opendrape_sim::Collider`, and has an inherent `signed_distance`.
  - `opendrape::sim_runner`:
    - `DENSITY_KG_M2 = 0.15`;
    - `enum DrapeNote { Mesh(MeshNote), StartsInside(PieceId) }`;
    - `SimFrame { seq, time, positions, triangles, step_ms, notes: Arc<Vec<DrapeNote>> }`;
    - `SimRunner`: `start(Arc<Stage>, on_frame)`, `play(Arc<Project>)`, `reset()`, `latest() -> Option<Arc<SimFrame>>`, `is_draping()`, `is_playing()`, `is_idle()`, `set_playing(bool)`, `went_wrong() -> bool`;
    - `build_drape(&Project, &Stage) -> (Solver, Vec<DrapeNote>)`.
  - `OpenDrapeApp`:
    - `sim_frame() -> Option<Arc<SimFrame>>` (None while arranging), `is_draping()`, `stage() -> &Arc<Stage>`;
    - `Startup` loses `autoplay`;
    - `Viewport::new(rs, &Stage)`.
  - The app no longer depends on `opendrape-testkit` (`Garment` and the demo scenes are gone from the 3D view). It depends on `opendrape-body` instead.

**Behaviour:**
- **Stage:**
  - The bundled body, moved so its torso's centre between the hips is at x = 0, z = 0.
  - Shoulders at 0.82 × height.
  - Surface distance: a ray from the centre line (`BodyCollider::ray_exit`).
  - `BodyAndFloor`: a particle inside the form or below the floor takes the nearest way out; otherwise the nearest surface within the margin. This matches dress-forms' `CompoundCollider`.
- **The runner:**
  - It starts with nothing to drape.
  - `play(project)`: on the simulation thread, `build_drape` meshes the project (12 mm), places every panel, builds the cloth (0.15 kg/m², flat rest shape), stitches its seams and notes pieces that start inside the form. The first frame (time 0) is then published and the run starts.
  - Each step uses `stage.drape_collider()`.
  - A non-finite live particle drops the drape and sets `went_wrong`.
  - A settled drape pauses itself, as in M1.
  - `reset()` drops the drape.
- **The app:**
  - **Play** snapshots the project and plays it. Then the button reads **Pause**, and **Play** again resumes.
  - **Reset** is enabled while draping. Any change to the project while draped resets, compared against the snapshot.
  - Under the toolbar: "The drape went wrong…" (until the next Play), "Press Reset to move pieces." while draping, and the drape's notes, naming pieces as the project names them now.

- [ ] **Step 1: Strings**

Remove `garment-skirt` and `garment-bodice-proxy` from the `.ftl`, and append:

```
hint-draping = Press Reset to move pieces.
note-coarser = Large pattern: using coarser fabric
note-crosses-itself = { $name } couldn't be made into fabric: its outline crosses itself.
note-unmeshable = { $name } couldn't be made into fabric.
note-lengths-differ = Seam { $number }: the sides' lengths differ by { $difference }; the longer one gathers as ease.
note-starts-inside = { $name } starts inside the form; move it out first.
note-went-wrong = The drape went wrong and was reset. Check for seams that pull pieces through the form.
```

- [ ] **Step 2: The stage, test first**

`crates/app/Cargo.toml`:
- under `[dependencies]`, add `opendrape-body.workspace = true` (first) and remove `opendrape-testkit.workspace = true`;
- under `[dev-dependencies]`, remove `opendrape-testkit.workspace = true` too.

`crates/app/src/lib.rs`: add `pub mod stage;` after `pub mod smoke_test;`.

Create `crates/app/src/stage.rs` with only its tests:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use glam::DVec2;
    use opendrape_sim::{ClothBuilder, Panel, Params, Solver};

    #[test]
    fn the_form_stands_on_the_floor_round_its_centre_line() {
        let stage = Stage::shared();
        let (positions, _) = stage.render_mesh();
        let low = positions.iter().map(|p| p.y).fold(f32::MAX, f32::min);
        assert!(low.abs() < 0.01, "feet at the floor: {low}");
        // Between the hips, the form is about as far out in front as behind, and left as right.
        use std::f64::consts::{FRAC_PI_2, PI};
        let d = |a: f64| stage.surface_distance(a, 0.80).expect("the hips");
        assert!(
            (d(0.0) - d(PI)).abs() < 0.08,
            "front {} back {}",
            d(0.0),
            d(PI)
        );
        assert!((d(FRAC_PI_2) - d(-FRAC_PI_2)).abs() < 0.01);
        assert!(
            (1.2..1.4).contains(&stage.shoulder_y()),
            "{}",
            stage.shoulder_y()
        );
        assert!(
            stage.signed_distance(DVec3::new(0.0, 1.0, 0.0)) < 0.0,
            "inside at the waist"
        );
        assert_eq!((stage.centre_line(), stage.floor_y()), ((0.0, 0.0), 0.0));
    }

    #[test]
    fn the_floor_holds_particles_up_and_the_form_still_counts() {
        let stage = Stage::shared();
        let both = stage.drape_collider();
        let planes = both.contact_planes(
            &[
                DVec3::new(2.0, 0.01, 2.0),
                DVec3::new(2.0, -0.1, 2.0),
                DVec3::new(2.0, 1.0, 2.0),
                DVec3::new(0.0, 1.0, 0.0),
            ],
            0.05,
        );
        let floor = Some(Plane {
            normal: DVec3::Y,
            point: DVec3::new(2.0, 0.0, 2.0),
        });
        assert_eq!(planes[0], floor);
        assert_eq!(planes[1], floor, "below the floor is pushed back up");
        assert_eq!(planes[2], None, "far from both");
        let inside = planes[3].expect("inside the form");
        assert!(
            inside.normal.y.abs() < 0.9,
            "out through the form, not the floor"
        );
        assert!((both.signed_distance(DVec3::new(2.0, -0.1, 2.0)) + 0.1).abs() < 1e-9);
    }

    #[test]
    fn a_dropped_piece_comes_to_rest_on_the_floor() {
        let stage = Stage::shared();
        let flat = vec![
            DVec2::new(-0.05, -0.05),
            DVec2::new(0.05, -0.05),
            DVec2::new(0.0, 0.05),
        ];
        let panel = Panel {
            positions: flat
                .iter()
                .map(|p| DVec3::new(p.x + 1.0, 0.2, p.y))
                .collect(),
            flat: Some(flat),
            triangles: vec![[0, 1, 2]],
        };
        let mut b = ClothBuilder::new(0.15);
        b.add_panel(&panel, 1.0);
        let mut s = Solver::new(
            b.build(),
            Params {
                gravity_delay: 0.0,
                ..Params::default()
            },
        );
        let collider = stage.drape_collider();
        for _ in 0..180 {
            s.step(Some(&collider));
        }
        for p in s.cloth().positions() {
            assert!(
                p.y >= 0.003 - 1e-4 && p.y < 0.02,
                "resting on the floor: {p}"
            );
        }
    }
}
```

Run: `cargo nextest run -p opendrape --lib stage`
Expected: compile errors (`Stage` doesn't exist yet). The old `sim_runner` and `viewport` also fail to compile without testkit; Step 4 replaces them.

- [ ] **Step 3: Implement the stage** (above its tests)

```rust
//! The form garments drape on, behind one small boundary: what the 3D view draws, what the
//! solver collides with (with a floor), the centre line, the floor's height, and how far the
//! form's surface is from its centre line. Today it wraps the MakeHuman body; the dress forms
//! swap in behind the same methods.
//!
//! The stage's frame is the form's frame: metres, y up from the floor, the form faces +z and
//! its left is +x, and its centre line is x = 0, z = 0 (the body is moved there when loaded).

use glam::{DVec3, Vec3};
use opendrape_body::BodyMesh;
use opendrape_sim::{BodyCollider, Collider, Plane};
use std::sync::{Arc, OnceLock};

/// Shoulder height as a share of standing height (the usual proportion of an adult body).
const SHOULDER_SHARE: f64 = 0.82;

pub struct Stage {
    positions: Vec<Vec3>,
    triangles: Vec<[u32; 3]>,
    collider: BodyCollider,
    shoulder_y: f64,
}

impl Stage {
    /// The bundled MakeHuman body, moved so its torso's centre line is at x = 0, z = 0.
    pub fn makehuman() -> Self {
        let body = BodyMesh::female_average();
        let centre = torso_centre(&body);
        let positions: Vec<Vec3> = body.positions.iter().map(|p| *p - centre).collect();
        let collider =
            BodyCollider::new(&positions, &body.triangles).expect("the bundled body is closed");
        let height = positions.iter().map(|p| p.y).fold(0.0_f32, f32::max);
        Self {
            positions,
            triangles: body.triangles,
            collider,
            shoulder_y: SHOULDER_SHARE * f64::from(height),
        }
    }

    /// One stage for the whole app (and its tests): building the collider takes a moment.
    pub fn shared() -> Arc<Stage> {
        static STAGE: OnceLock<Arc<Stage>> = OnceLock::new();
        STAGE.get_or_init(|| Arc::new(Self::makehuman())).clone()
    }

    /// The form's triangles, to draw.
    pub fn render_mesh(&self) -> (&[Vec3], &[[u32; 3]]) {
        (&self.positions, &self.triangles)
    }

    pub fn collider(&self) -> &BodyCollider {
        &self.collider
    }

    /// The form and the floor, for the solver.
    pub fn drape_collider(&self) -> BodyAndFloor<'_> {
        BodyAndFloor {
            body: &self.collider,
            floor: self.floor_y(),
        }
    }

    /// The centre line's x and z: the form's frame puts it at the origin.
    pub fn centre_line(&self) -> (f64, f64) {
        (0.0, 0.0)
    }

    pub fn floor_y(&self) -> f64 {
        0.0
    }

    /// Where pieces start: the top of the pattern lines up with the form's shoulders.
    pub fn shoulder_y(&self) -> f64 {
        self.shoulder_y
    }

    /// How far (m) the form's surface is from its centre line at `angle` (radians from the
    /// front towards the form's left) and height `y`, if a ray from the centre line finds it.
    pub fn surface_distance(&self, angle: f64, y: f64) -> Option<f64> {
        let (x, z) = self.centre_line();
        let dir = DVec3::new(angle.sin(), 0.0, angle.cos());
        self.collider.ray_exit(DVec3::new(x, y, z), dir, 1.0)
    }

    /// Distance (m) to the form's surface, negative inside it.
    pub fn signed_distance(&self, p: DVec3) -> f64 {
        self.collider.signed_distance(p)
    }
}

/// The torso's centre between the hips (from 0.7 to 0.85 m up, arms left out), with y = 0.
fn torso_centre(body: &BodyMesh) -> Vec3 {
    let (lo, hi) = body
        .positions
        .iter()
        .filter(|p| p.y > 0.7 && p.y < 0.85 && p.x.abs() < 0.22)
        .fold((Vec3::MAX, Vec3::MIN), |(lo, hi), p| {
            (lo.min(*p), hi.max(*p))
        });
    let mid = (lo + hi) / 2.0;
    Vec3::new(mid.x, 0.0, mid.z)
}

/// The form and a floor as one collider. A particle inside either takes the nearest way out;
/// otherwise the nearest surface within the margin. When the dress forms' `CompoundCollider`
/// (which has a floor) arrives, it replaces this.
pub struct BodyAndFloor<'a> {
    body: &'a BodyCollider,
    floor: f64,
}

impl BodyAndFloor<'_> {
    /// Distance to the nearer of the form and the floor, negative inside either.
    pub fn signed_distance(&self, p: DVec3) -> f64 {
        self.body.signed_distance(p).min(p.y - self.floor)
    }
}

impl Collider for BodyAndFloor<'_> {
    fn contact_planes(&self, x: &[DVec3], margin: f64) -> Vec<Option<Plane>> {
        let body = self.body.contact_planes(x, margin);
        x.iter()
            .zip(body)
            .map(|(p, on_body)| {
                let on_floor = (p.y - self.floor < margin).then(|| Plane {
                    normal: DVec3::Y,
                    point: DVec3::new(p.x, self.floor, p.z),
                });
                on_body
                    .into_iter()
                    .chain(on_floor)
                    .map(|plane| ((*p - plane.point).dot(plane.normal), plane))
                    // Inside anything: the nearest way out. Otherwise: the nearest surface.
                    .min_by(|(a, _), (b, _)| {
                        (*a >= 0.0)
                            .cmp(&(*b >= 0.0))
                            .then(a.abs().total_cmp(&b.abs()))
                    })
                    .map(|(_, plane)| plane)
            })
            .collect()
    }
}
```

- [ ] **Step 4: The runner, replaced** (`crates/app/src/sim_runner.rs`, whole file)

```rust
//! Drapes the student's garment on its own thread, so the window stays responsive even when a
//! slow computer simulates slower than real time. Play hands the thread a snapshot of the
//! project; the thread builds the fabric (meshing takes a moment) and the cloth, then steps
//! the solver. Reset drops the drape. The UI only reads the latest frame.

use crate::stage::Stage;
use arc_swap::ArcSwapOption;
use crossbeam_channel::{Receiver, Sender, unbounded};
use glam::{DVec2, DVec3, Vec3};
use opendrape_core::{PieceId, Point2, Project};
use opendrape_geom as geom;
use opendrape_mesh::{MeshNote, MeshParams, place};
use opendrape_sim::{ClothBuilder, Panel, Params, Solver};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

/// Fabric weight (kg/m²): one light cotton until fabrics arrive.
pub const DENSITY_KG_M2: f64 = 0.15;

/// Something the student should know about the drape.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum DrapeNote {
    /// From making the fabric (see [`MeshNote`]).
    Mesh(MeshNote),
    /// Part of this piece starts inside the form.
    StartsInside(PieceId),
}

/// One published simulation frame.
#[derive(Debug)]
pub struct SimFrame {
    /// Increases with every published frame, across drapes.
    pub seq: u64,
    /// Simulated seconds since Play.
    pub time: f64,
    pub positions: Vec<Vec3>,
    /// Shared until the topology changes (seams weld), so the renderer can skip re-uploads.
    pub triangles: Arc<Vec<[u32; 3]>>,
    /// Wall-clock milliseconds the last step took.
    pub step_ms: f64,
    /// What making this drape found.
    pub notes: Arc<Vec<DrapeNote>>,
}

enum Command {
    Play(Arc<Project>),
    Reset,
    Wake,
    Shutdown,
    /// Tests: treat the next step as having gone wrong.
    #[cfg(test)]
    Spoil,
}

pub struct SimRunner {
    tx: Sender<Command>,
    latest: Arc<ArcSwapOption<SimFrame>>,
    /// From Play until Reset (or until the drape goes wrong): the 3D view shows the drape.
    draping: Arc<AtomicBool>,
    playing: Arc<AtomicBool>,
    idle: Arc<AtomicBool>,
    went_wrong: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

/// Below this kinetic energy (J) for [`SETTLE_FRAMES`] frames, a welded drape counts as settled
/// and the simulation pauses itself, so a finished drape doesn't keep a laptop core busy.
const SETTLED_ENERGY: f64 = 1e-6;
const SETTLE_FRAMES: u32 = 60;

struct Status {
    latest: Arc<ArcSwapOption<SimFrame>>,
    draping: Arc<AtomicBool>,
    playing: Arc<AtomicBool>,
    idle: Arc<AtomicBool>,
    went_wrong: Arc<AtomicBool>,
}

impl SimRunner {
    /// Starts the simulation thread, with nothing to drape. `on_frame` runs after every
    /// published frame (the app asks for a repaint).
    pub fn start(stage: Arc<Stage>, on_frame: impl Fn() + Send + 'static) -> Self {
        let (tx, rx) = unbounded();
        let status = Status {
            latest: Arc::new(ArcSwapOption::empty()),
            draping: Arc::new(AtomicBool::new(false)),
            playing: Arc::new(AtomicBool::new(false)),
            idle: Arc::new(AtomicBool::new(false)),
            went_wrong: Arc::new(AtomicBool::new(false)),
        };
        let (latest, draping, playing, idle, went_wrong) = (
            status.latest.clone(),
            status.draping.clone(),
            status.playing.clone(),
            status.idle.clone(),
            status.went_wrong.clone(),
        );
        let thread = std::thread::Builder::new()
            .name("opendrape-sim".into())
            .spawn(move || run(&stage, &rx, &status, &on_frame))
            .expect("spawn the simulation thread");
        Self {
            tx,
            latest,
            draping,
            playing,
            idle,
            went_wrong,
            thread: Some(thread),
        }
    }
    /// Drapes `project`: the fabric is made on the simulation thread, then it runs.
    pub fn play(&self, project: Arc<Project>) {
        self.went_wrong.store(false, Ordering::Relaxed);
        self.draping.store(true, Ordering::Relaxed);
        self.playing.store(true, Ordering::Relaxed);
        let _ = self.tx.send(Command::Play(project));
    }
    /// Back to arranging: the drape is dropped.
    pub fn reset(&self) {
        self.draping.store(false, Ordering::Relaxed);
        self.playing.store(false, Ordering::Relaxed);
        let _ = self.tx.send(Command::Reset);
    }
    /// The latest frame of the drape; None while arranging (and while the fabric is made).
    pub fn latest(&self) -> Option<Arc<SimFrame>> {
        self.latest.load_full()
    }
    /// Between Play and Reset.
    pub fn is_draping(&self) -> bool {
        self.draping.load(Ordering::Relaxed)
    }
    pub fn is_playing(&self) -> bool {
        self.playing.load(Ordering::Relaxed)
    }
    /// True while the worker waits for a command: no step is in flight.
    pub fn is_idle(&self) -> bool {
        self.idle.load(Ordering::Acquire)
    }
    /// Pauses or resumes the drape.
    pub fn set_playing(&self, playing: bool) {
        self.playing.store(playing, Ordering::Relaxed);
        let _ = self.tx.send(Command::Wake);
    }
    /// Whether the last drape went wrong (a number stopped being finite) and was dropped.
    pub fn went_wrong(&self) -> bool {
        self.went_wrong.load(Ordering::Relaxed)
    }
    #[cfg(test)]
    fn spoil(&self) {
        let _ = self.tx.send(Command::Spoil);
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

/// A drape in progress.
struct Drape {
    solver: Solver,
    notes: Arc<Vec<DrapeNote>>,
    topology: u64,
    triangles: Arc<Vec<[u32; 3]>>,
}

impl Drape {
    fn frame(&mut self, seq: u64, step_ms: f64) -> SimFrame {
        let c = self.solver.cloth();
        if c.topology_version() != self.topology {
            self.topology = c.topology_version();
            self.triangles = Arc::new(c.triangles().to_vec());
        }
        SimFrame {
            seq,
            time: self.solver.time(),
            positions: c.positions().iter().map(|p| p.as_vec3()).collect(),
            triangles: self.triangles.clone(),
            step_ms,
            notes: self.notes.clone(),
        }
    }
    /// Every live particle is a finite number.
    fn is_finite(&self) -> bool {
        let c = self.solver.cloth();
        c.positions()
            .iter()
            .enumerate()
            .all(|(i, p)| !c.is_alive(i) || p.is_finite())
    }
}

/// The fabric and cloth for `project` on `stage`: every shape that could be meshed, at its
/// placement, sewn by its seams; and the notes about it.
pub fn build_drape(project: &Project, stage: &Stage) -> (Solver, Vec<DrapeNote>) {
    let mesh = opendrape_mesh::build(project, &MeshParams::default());
    let shapes = geom::shapes(project);
    let layout = place::layout(&shapes);
    let mut notes: Vec<DrapeNote> = mesh.notes.iter().map(|n| DrapeNote::Mesh(*n)).collect();
    let mut builder = ClothBuilder::new(DENSITY_KG_M2);
    let mut ids = Vec::with_capacity(mesh.panels.len());
    for panel in &mesh.panels {
        let shape = shapes
            .iter()
            .find(|s| s.id == panel.shape)
            .expect("every panel comes from a shape");
        let placement = place::effective(project, shape, &layout, stage.shoulder_y());
        let positions: Vec<DVec3> = panel
            .flat
            .iter()
            .map(|f| {
                place::apply(
                    &placement,
                    panel.centre,
                    Point2::new(f[0] * 1000.0, f[1] * 1000.0),
                )
            })
            .collect();
        if positions.iter().any(|p| stage.signed_distance(*p) < 0.0) {
            notes.push(DrapeNote::StartsInside(panel.shape));
        }
        ids.push(builder.add_panel(
            &Panel {
                positions,
                flat: Some(panel.flat.iter().map(|f| DVec2::from_array(*f)).collect()),
                triangles: panel.triangles.clone(),
            },
            1.0,
        ));
    }
    for &((pa, a), (pb, b)) in &mesh.stitches {
        builder.stitch((ids[pa], a), (ids[pb], b));
    }
    (Solver::new(builder.build(), Params::default()), notes)
}

fn run(stage: &Stage, rx: &Receiver<Command>, status: &Status, on_frame: &dyn Fn()) {
    let mut drape: Option<Drape> = None;
    let mut seq = 0;
    let mut next = Instant::now();
    let mut settled_frames = 0;
    let mut spoiled = false;
    loop {
        // Draping and playing: just look for a command. Otherwise sleep until one arrives.
        let busy = drape.is_some() && status.playing.load(Ordering::Relaxed);
        let cmd = if busy {
            rx.try_recv().ok()
        } else {
            status.idle.store(true, Ordering::Release);
            let cmd = rx.recv().unwrap_or(Command::Shutdown);
            status.idle.store(false, Ordering::Release);
            Some(cmd)
        };
        match cmd {
            Some(Command::Shutdown) => return,
            Some(Command::Play(project)) => {
                let (solver, notes) = build_drape(&project, stage);
                let c = solver.cloth();
                let mut d = Drape {
                    topology: c.topology_version(),
                    triangles: Arc::new(c.triangles().to_vec()),
                    solver,
                    notes: Arc::new(notes),
                };
                seq += 1;
                status.latest.store(Some(Arc::new(d.frame(seq, 0.0))));
                drape = Some(d);
                settled_frames = 0;
                on_frame();
                next = Instant::now();
                continue;
            }
            Some(Command::Reset) => {
                drape = None;
                status.latest.store(None);
                on_frame();
                continue;
            }
            Some(Command::Wake) => {
                next = Instant::now();
                continue;
            }
            #[cfg(test)]
            Some(Command::Spoil) => {
                spoiled = true;
                continue;
            }
            None => {}
        }
        let Some(d) = &mut drape else { continue };
        let started = Instant::now();
        let collider = stage.drape_collider();
        d.solver.step(Some(&collider));
        if std::mem::take(&mut spoiled) || !d.is_finite() {
            // Something pulled the cloth apart: drop the drape and say so.
            drape = None;
            status.latest.store(None);
            status.draping.store(false, Ordering::Relaxed);
            status.playing.store(false, Ordering::Relaxed);
            status.went_wrong.store(true, Ordering::Relaxed);
            on_frame();
            continue;
        }
        let cloth = d.solver.cloth();
        settled_frames = if !cloth.has_open_stitches() && cloth.kinetic_energy() < SETTLED_ENERGY {
            settled_frames + 1
        } else {
            0
        };
        if settled_frames >= SETTLE_FRAMES {
            status.playing.store(false, Ordering::Relaxed);
            settled_frames = 0;
        }
        seq += 1;
        let ms = started.elapsed().as_secs_f64() * 1000.0;
        status.latest.store(Some(Arc::new(d.frame(seq, ms))));
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

#[cfg(test)]
mod tests {
    use super::*;
    use opendrape_core::{Half, Piece, Placement, SeamSide};

    fn wait_for(what: &str, mut cond: impl FnMut() -> bool) {
        let start = Instant::now();
        while !cond() {
            assert!(
                start.elapsed() < Duration::from_secs(20),
                "timed out waiting for {what}"
            );
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    /// Two 200 × 300 mm panels sewn along one side, hanging in front of the form.
    fn two_panels() -> Arc<Project> {
        let mut pr = Project::new();
        let a = pr.add_piece(Piece::rectangle(
            PieceId(0),
            "A",
            Point2::new(0.0, 0.0),
            200.0,
            300.0,
        ));
        let b = pr.add_piece(Piece::rectangle(
            PieceId(0),
            "B",
            Point2::new(300.0, 0.0),
            200.0,
            300.0,
        ));
        pr.add_seam(
            SeamSide::new(a, Half::Drawn, 1, 1, true),
            SeamSide::new(b, Half::Drawn, 3, 1, false),
        );
        Arc::new(pr)
    }

    /// A small square lying flat 3 cm above the floor, beside the form: it drops and settles.
    fn on_the_floor() -> Arc<Project> {
        let mut pr = Project::new();
        let mut piece = Piece::rectangle(PieceId(0), "Square", Point2::new(0.0, 0.0), 100.0, 100.0);
        piece.placement = Some(Placement {
            position: [0.6, 0.03, 0.0],
            rotation: glam::DQuat::from_rotation_x(-std::f64::consts::FRAC_PI_2).to_array(),
            curve: None,
        });
        pr.add_piece(piece);
        Arc::new(pr)
    }

    #[test]
    fn play_builds_the_fabric_on_the_simulation_thread_and_runs_it() {
        let r = SimRunner::start(Stage::shared(), || {});
        assert!(
            r.latest().is_none() && !r.is_draping(),
            "nothing to drape yet"
        );
        let project = two_panels();
        let mesh = opendrape_mesh::build(&project, &MeshParams::default());
        r.play(project);
        assert!(r.is_draping());
        wait_for("simulated time to advance", || {
            r.latest().is_some_and(|f| f.time > 0.1)
        });
        assert_eq!(r.latest().unwrap().positions.len(), mesh.particles());
        assert!(r.latest().unwrap().notes.is_empty());
    }

    #[test]
    fn pausing_stops_time_and_reset_drops_the_drape() {
        let r = SimRunner::start(Stage::shared(), || {});
        r.play(two_panels());
        wait_for("time > 0.1", || r.latest().is_some_and(|f| f.time > 0.1));
        r.set_playing(false);
        wait_for("the worker to go idle", || r.is_idle());
        let t = r.latest().unwrap().time;
        std::thread::sleep(Duration::from_millis(150));
        assert_eq!(r.latest().unwrap().time, t, "paused");
        r.reset();
        wait_for("the drape to go", || r.latest().is_none());
        assert!(!r.is_draping());
    }

    #[test]
    fn a_settled_drape_pauses_itself() {
        let r = SimRunner::start(Stage::shared(), || {});
        r.play(on_the_floor());
        wait_for("auto-pause once settled", || !r.is_playing() && r.is_idle());
        let f = r.latest().expect("still showing the drape");
        assert!(
            f.positions.iter().all(|p| p.y < 0.02 && p.y > 0.0),
            "on the floor"
        );
        std::thread::sleep(Duration::from_millis(150));
        assert_eq!(r.latest().unwrap().time, f.time);
    }

    #[test]
    fn a_drape_that_goes_wrong_is_dropped_and_reported() {
        let r = SimRunner::start(Stage::shared(), || {});
        r.play(two_panels());
        wait_for("the drape", || r.latest().is_some());
        r.spoil();
        wait_for("the drape to be dropped", || r.went_wrong());
        assert!(r.latest().is_none() && !r.is_draping() && !r.is_playing());
        r.play(two_panels());
        assert!(!r.went_wrong(), "a new Play starts afresh");
    }

    #[test]
    fn notes_name_pieces_that_start_inside_or_could_not_be_made() {
        let mut pr = Project::new();
        let mut inside = Piece::rectangle(PieceId(0), "Front", Point2::new(0.0, 0.0), 100.0, 100.0);
        inside.placement = Some(Placement::at([0.0, 1.0, 0.0]));
        let inside = pr.add_piece(inside);
        let bow = pr.add_piece(Piece::polygon(
            PieceId(0),
            "Bow",
            &[
                Point2::new(0.0, 0.0),
                Point2::new(100.0, 100.0),
                Point2::new(100.0, 0.0),
                Point2::new(0.0, 100.0),
            ],
        ));
        let (_, notes) = build_drape(&pr, &Stage::shared());
        assert_eq!(
            notes,
            vec![
                DrapeNote::Mesh(MeshNote::CrossesItself(bow)),
                DrapeNote::StartsInside(inside)
            ]
        );
    }

    #[test]
    fn play_with_nothing_drawn_is_harmless() {
        let r = SimRunner::start(Stage::shared(), || {});
        r.play(Arc::new(Project::new()));
        wait_for("an empty drape that settles at once", || {
            r.latest().is_some() && !r.is_playing() && r.is_idle()
        });
        assert!(r.latest().unwrap().positions.is_empty());
        assert!(!r.went_wrong());
    }

    #[test]
    fn dropping_the_runner_stops_its_thread() {
        let r = SimRunner::start(Stage::shared(), || {});
        r.play(two_panels());
        wait_for("running", || r.latest().is_some_and(|f| f.time > 0.0));
        let start = Instant::now();
        drop(r);
        assert!(
            start.elapsed() < Duration::from_secs(2),
            "thread joined promptly"
        );
    }
}
```

`crates/app/src/viewport.rs`:
- add `use crate::stage::Stage;`;
- `new` takes the stage and draws its body:

```rust
    pub fn new(rs: &egui_wgpu::RenderState, stage: &Stage) -> Self {
        let renderer = MeshRenderer::new(&rs.device);
        let (positions, triangles) = stage.render_mesh();
        let body = renderer.create_mesh(&rs.device, &rs.queue, positions, triangles, SKIN);
```

- the camera's `target` becomes `glam::Vec3::new(0.0, 0.95, 0.0)`;
- in `ui`, a missing frame now clears the cloth:

```rust
        match frame {
            Some(f) => self.sync_cloth(rs, f),
            None => self.cloth = None,
        }
```

- [ ] **Step 5: The app** (`crates/app/src/app.rs`)

- Imports:
  - `use crate::sim_runner::{DrapeNote, SimFrame, SimRunner};`
  - `use crate::stage::Stage;`
  - `use opendrape_core::{PieceId, Project};`
  - `use opendrape_mesh::MeshNote;`
  - remove the `opendrape_testkit` import.
- `Startup` loses the `autoplay` field (and its doc comment).
- `OpenDrapeApp` loses `garment` and gains:

```rust
    /// The form, shared with the simulation thread.
    stage: Arc<Stage>,
    /// The project as it was when Play was pressed: any change to it returns to arranging.
    draped: Option<Arc<Project>>,
```

- In `OpenDrapeApp::new`, the runner becomes:

```rust
        let stage = Stage::shared();
        let runner = render_state.map(|_| {
            let ctx = cc.egui_ctx.clone();
            SimRunner::start(stage.clone(), move || ctx.request_repaint())
        });
```

  The struct literal gets `viewport: render_state.map(|rs| Viewport::new(rs, &stage)),`, `stage,` and `draped: None,`, and drops `garment: Garment::Skirt,`.
- Replace `sim_frame` with these three:

```rust
    /// The latest frame of the drape; None while arranging.
    pub fn sim_frame(&self) -> Option<Arc<SimFrame>> {
        self.runner.as_ref().and_then(SimRunner::latest)
    }

    /// Between Play and Reset.
    pub fn is_draping(&self) -> bool {
        self.runner.as_ref().is_some_and(SimRunner::is_draping)
    }

    /// The form garments are arranged round and draped on.
    pub fn stage(&self) -> &Arc<Stage> {
        &self.stage
    }
```

- Replace `toolbar` and `view_3d`, and add `reset_if_edited` and `notes`:

```rust
    fn toolbar(&mut self, ui: &mut egui::Ui) {
        let Some(runner) = &self.runner else { return };
        let (draping, playing) = (runner.is_draping(), runner.is_playing());
        let mut clicked = None;
        ui.horizontal_wrapped(|ui| {
            let label = if draping && playing {
                tr!("toolbar-pause")
            } else {
                tr!("toolbar-play")
            };
            if ui.button(label).clicked() {
                clicked = Some(match (draping, playing) {
                    (false, _) => Toolbar::Play,
                    (true, true) => Toolbar::Pause,
                    (true, false) => Toolbar::Resume,
                });
            }
            if ui
                .add_enabled(draping, egui::Button::new(tr!("toolbar-reset")))
                .clicked()
            {
                clicked = Some(Toolbar::Reset);
            }
        });
        match clicked {
            Some(Toolbar::Play) => {
                let snapshot = Arc::new(self.editor.doc.project().clone());
                runner.play(snapshot.clone());
                self.draped = Some(snapshot);
            }
            Some(Toolbar::Pause) => runner.set_playing(false),
            Some(Toolbar::Resume) => runner.set_playing(true),
            Some(Toolbar::Reset) => {
                runner.reset();
                self.draped = None;
            }
            None => {}
        }
    }

    /// A pattern edit while draped returns to arranging (live updates come later).
    fn reset_if_edited(&mut self) {
        let Some(runner) = &self.runner else { return };
        let edited = self
            .draped
            .as_ref()
            .is_some_and(|d| **d != *self.editor.doc.project());
        if edited {
            runner.reset();
        }
        if edited || !runner.is_draping() {
            self.draped = None;
        }
    }

    /// The hint while draping, and what the student should know about the drape.
    fn notes(&self, ui: &mut egui::Ui) {
        let Some(runner) = &self.runner else { return };
        let warn = ui.visuals().warn_fg_color;
        if runner.went_wrong() {
            ui.colored_label(warn, tr!("note-went-wrong"));
        }
        if runner.is_draping() {
            ui.label(tr!("hint-draping"));
        }
        if let Some(frame) = runner.latest() {
            for note in frame.notes.iter() {
                ui.colored_label(warn, note_text(note, self.editor.doc.project()));
            }
        }
    }

    /// The 3D view: its toolbar and notes, the form and the drape, and the speed overlay.
    fn view_3d(&mut self, ui: &mut egui::Ui, frame: &eframe::Frame) {
        self.reset_if_edited();
        self.toolbar(ui);
        self.notes(ui);
        ui.separator();
        let sim = self.sim_frame();
        let fps = self
            .runner
            .as_ref()
            .is_some_and(SimRunner::is_playing)
            .then_some(self.fps);
        match (self.viewport.as_mut(), frame.wgpu_render_state()) {
            (Some(viewport), Some(rs)) => {
                let rect = ui.available_rect_before_wrap();
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
        }
    }
```

- Replace `fn garment_label` with:

```rust
/// The toolbar buttons of the 3D view.
#[derive(Clone, Copy)]
enum Toolbar {
    Play,
    Pause,
    Resume,
    Reset,
}

/// A drape note as the student reads it, naming pieces as the project does now.
fn note_text(note: &DrapeNote, project: &Project) -> String {
    let name = |id: PieceId| project.name_of(id).unwrap_or_default().to_owned();
    match *note {
        DrapeNote::Mesh(MeshNote::Coarser { .. }) => tr!("note-coarser"),
        DrapeNote::Mesh(MeshNote::CrossesItself(id)) => tr!("note-crosses-itself", name = name(id)),
        DrapeNote::Mesh(MeshNote::Unmeshable(id)) => tr!("note-unmeshable", name = name(id)),
        DrapeNote::Mesh(MeshNote::LengthsDiffer { seam, by_mm }) => tr!(
            "note-lengths-differ",
            number = seam.0,
            difference = project.units.format(by_mm)
        ),
        DrapeNote::StartsInside(id) => tr!("note-starts-inside", name = name(id)),
    }
}
```

`crates/app/src/main.rs`: remove the line `autoplay: true,`.

- [ ] **Step 6: App tests** (`crates/app/tests/ui.rs`)

- The core import gains `Half` and `SeamSide`.
- Remove every `autoplay: false,` line (three `Startup` literals).
- Replace `the_skirt_drapes_and_can_be_paused` and `reset_and_garment_switch_reload_the_scene` (from `/// Two rectangles…`) with:

```rust
/// Two rectangles sewn along one side, drawn in the pattern window.
fn add_sewn_pieces(h: &mut App) {
    h.state_mut().editor_mut().doc.edit(|p| {
        let a = p.add_piece(Piece::rectangle(
            PieceId(0),
            "Front",
            Point2::new(0.0, 0.0),
            200.0,
            300.0,
        ));
        let b = p.add_piece(Piece::rectangle(
            PieceId(0),
            "Back",
            Point2::new(300.0, 0.0),
            200.0,
            300.0,
        ));
        p.add_seam(
            SeamSide::new(a, Half::Drawn, 1, 1, true),
            SeamSide::new(b, Half::Drawn, 3, 1, false),
        );
    });
    h.run();
}

#[test]
fn play_drapes_the_pattern_and_reset_returns_to_arranging() {
    let dir = tempfile::tempdir().unwrap();
    let mut h = harness(dir.path(), SharedState::default());
    h.run();
    add_sewn_pieces(&mut h);
    assert!(
        h.state().sim_frame().is_none() && !h.state().is_draping(),
        "arranging"
    );
    h.get_by_label("Play").click();
    h.run_steps(2);
    wait_until(&mut h, "the drape to advance", |a| {
        a.sim_frame().is_some_and(|f| f.time > 0.05)
    });
    h.get_by_label("Press Reset to move pieces.");
    h.get_by_label("Pause").click();
    h.run_steps(3);
    h.get_by_label("Play");
    h.get_by_label("Reset").click();
    h.run_steps(2);
    wait_until(&mut h, "arranging again", |a| a.sim_frame().is_none());
    assert!(!h.state().is_draping());
    assert!(h.query_by_label("Press Reset to move pieces.").is_none());
}

#[test]
fn editing_the_pattern_while_draped_returns_to_arranging() {
    let dir = tempfile::tempdir().unwrap();
    let mut h = harness(dir.path(), SharedState::default());
    h.run();
    add_sewn_pieces(&mut h);
    h.get_by_label("Play").click();
    h.run_steps(2);
    wait_until(&mut h, "the drape", |a| a.sim_frame().is_some());
    h.state_mut()
        .editor_mut()
        .doc
        .edit(|p| p.pieces[0].name = "Front left".into());
    h.run_steps(2);
    wait_until(&mut h, "arranging again", |a| a.sim_frame().is_none());
    assert!(!h.state().is_draping());
}

#[test]
fn a_piece_that_cannot_be_made_into_fabric_is_named() {
    let dir = tempfile::tempdir().unwrap();
    let mut h = harness(dir.path(), SharedState::default());
    h.run();
    h.state_mut().editor_mut().doc.edit(|p| {
        p.add_piece(Piece::polygon(
            PieceId(0),
            "Front",
            &[
                Point2::new(0.0, 0.0),
                Point2::new(200.0, 200.0),
                Point2::new(200.0, 0.0),
                Point2::new(0.0, 200.0),
            ],
        ))
    });
    h.run();
    h.get_by_label("Play").click();
    h.run_steps(2);
    wait_until(&mut h, "the drape", |a| a.sim_frame().is_some());
    h.run_steps(1);
    h.get_by_label("Front couldn't be made into fabric: its outline crosses itself.");
}

#[test]
fn play_with_nothing_drawn_shows_just_the_form() {
    let dir = tempfile::tempdir().unwrap();
    let mut h = harness(dir.path(), SharedState::default());
    h.run();
    h.get_by_label("Play").click();
    h.run_steps(2);
    wait_until(&mut h, "the empty drape", |a| a.sim_frame().is_some());
    h.run_steps(3);
    assert!(h.state().sim_frame().unwrap().positions.is_empty());
    h.get_by_label("Reset").click();
    h.run_steps(2);
    wait_until(&mut h, "arranging again", |a| a.sim_frame().is_none());
}
```

- [ ] **Step 7: Run and see them pass**

Run: `cargo nextest run --workspace`
Expected: all pass, including:
- the 3 stage tests;
- the 7 runner tests (among them `play_with_nothing_drawn_is_harmless` and `a_drape_that_goes_wrong_is_dropped_and_reported`);
- the 4 app tests.

The testkit's M1 drape tests are unchanged and still pass.

- [ ] **Step 8: Commit**

```bash
cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings
git add Cargo.lock crates/app
git commit -m "feat(app): drape the student's own pattern: Stage with a floor, project-driven runner, notes

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 8: The pieces in 3D, clicking them, and one selection for both views (review: light)

**Files:**
- Create: `crates/app/src/arrange/scene.rs`
- Replace: `crates/app/src/arrange/mod.rs`, `crates/app/src/viewport.rs`
- Modify:
  - `crates/render/src/mesh.rs`, `crates/render/tests/mesh_render.rs`
  - `crates/app/src/app.rs`
  - `crates/app/i18n/en-US/opendrape.ftl`
  - `crates/app/tests/ui.rs`

**Interfaces:**
- Consumes:
  - Task 3: `opendrape_mesh::{build, MeshParams, GarmentMesh}`;
  - Task 4: `place::{layout, effective, apply, PlaceAt}`, `gizmo::{ScreenCamera, ray_triangle}`;
  - Task 7: `Stage::shoulder_y`, `SimRunner::latest`, `Viewport::new(rs, &Stage)`;
  - the editor's `pub selection: Selection` (the one selection both views share).
- Produces:
  - `opendrape_render::GpuMesh::set_color(&mut self, [f32; 3])`.
  - `opendrape::arrange::scene`:
    - `VIEW_EDGE_MM = 20.0`;
    - `ArrangedPanel { shape: PieceId, placement: Placement, positions: Vec<DVec3>, triangles: Vec<[u32; 3]> }`;
    - `ArrangedScene { panels }`, with `panel(PieceId) -> Option<&ArrangedPanel>` and `pick(origin, dir) -> Option<PieceId>`;
    - `SceneCache`: `scene(&mut self, &Project, shoulder_y) -> Rc<ArrangedScene>`; `pub meshed: usize` counts how often the fabric was made.
  - `opendrape::arrange::{Arranger, ArrangedPanel, ArrangedScene, SceneCache, ScreenCamera}` re-exported. `Arranger::click(&mut self, &ScreenCamera, &ArrangedScene, &mut Selection, DVec2)`.
  - `viewport`:
    - `enum Show<'a> { Drape(&SimFrame), Pieces { scene: &Rc<ArrangedScene>, selected: Option<PieceId> } }`;
    - `struct Drawn { response: egui::Response, camera: ScreenCamera }`;
    - `Viewport::{ui(&mut self, ui, rs, Show) -> Option<Drawn>, camera(), camera_mut(), look_from(angle)}`.
  - `OpenDrapeApp::{arranged_scene(&mut self) -> Rc<ArrangedScene>, view_camera() -> Option<ScreenCamera>, orbit_camera() -> Option<OrbitCamera>}`.
  - A private `fn arrange(&mut self, response, cam, scene)` on the app: Task 9 replaces it, and Task 10 extends it.

**Behaviour:**
- **The 3D view while arranging** shows each piece's fabric at its placement:
  - made at 20 mm, again only when the pattern (not just a placement) changes;
  - placed again when a placement changes;
  - the selected piece is orange.
- **While draping** it shows the cloth, as before.
- **Clicking:** a click picks the nearest piece under the pointer (CPU ray against its triangles) and selects it in both views. A click on nothing clears the selection.
- **Turning the view:** a drag turns the camera. **Front**, **Back**, **Left side** and **Right side** turn the camera to look from that side of the form (yaw 0, π, π/2, −π/2).

- [ ] **Step 1: Strings** (append to the `.ftl`)

```
view-front = Front
view-back = Back
view-left = Left side
view-right = Right side
```

- [ ] **Step 2: Failing tests**

Append to `crates/render/tests/mesh_render.rs`:

```rust
#[test]
fn a_mesh_can_change_colour() {
    let gpu = headless_device().expect("a GPU or software adapter");
    let target = RenderTarget::new(&gpu.device, 64, 64);
    let r = MeshRenderer::new(&gpu.device);
    let (p, t) = flat_cube();
    let mut cube = r.create_mesh(&gpu.device, &gpu.queue, &p, &t, [0.9, 0.1, 0.1]);
    let view_proj = OrbitCamera::default().view_proj(1.0);
    r.render(&gpu.device, &gpu.queue, &target, view_proj, &[&cube]);
    let red = *read_back(&gpu.device, &gpu.queue, &target).get_pixel(32, 32);
    cube.set_color([0.1, 0.1, 0.9]);
    r.render(&gpu.device, &gpu.queue, &target, view_proj, &[&cube]);
    let blue = *read_back(&gpu.device, &gpu.queue, &target).get_pixel(32, 32);
    assert!(
        red[0] > red[2] && blue[2] > blue[0],
        "{red:?} then {blue:?}"
    );
}
```

Create `crates/app/src/arrange/scene.rs` with only its tests:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use opendrape_core::Piece;

    fn two_pieces() -> Project {
        let mut pr = Project::new();
        pr.add_piece(Piece::rectangle(
            PieceId(0),
            "Front",
            Point2::new(0.0, 0.0),
            300.0,
            400.0,
        ));
        pr.add_piece(Piece::rectangle(
            PieceId(0),
            "Back",
            Point2::new(500.0, 0.0),
            300.0,
            400.0,
        ));
        pr
    }

    #[test]
    fn moving_a_piece_places_it_again_without_remaking_the_fabric() {
        let mut cache = SceneCache::default();
        let mut pr = two_pieces();
        let first = cache.scene(&pr, 1.3);
        assert_eq!(first.panels.len(), 2);
        assert!(
            Rc::ptr_eq(&first, &cache.scene(&pr, 1.3)),
            "nothing changed"
        );
        pr.set_placement(PieceId(1), Some(Placement::at([0.0, 1.0, 0.5])));
        let moved = cache.scene(&pr, 1.3);
        assert_eq!(cache.meshed, 1);
        assert_eq!(
            moved.panel(PieceId(1)).unwrap().placement,
            Placement::at([0.0, 1.0, 0.5])
        );
        pr.pieces[0].name = "Front left".into();
        cache.scene(&pr, 1.3);
        assert_eq!(cache.meshed, 2, "a pattern change makes the fabric again");
    }

    #[test]
    fn a_ray_picks_the_nearest_piece() {
        let mut pr = two_pieces();
        // The back hangs 10 cm behind the front, right behind it.
        pr.set_placement(PieceId(1), Some(Placement::at([0.0, 1.0, 0.5])));
        pr.set_placement(PieceId(2), Some(Placement::at([0.0, 1.0, 0.4])));
        let scene = SceneCache::default().scene(&pr, 1.3);
        let from_front = (DVec3::new(0.0, 1.0, 3.0), DVec3::NEG_Z);
        assert_eq!(scene.pick(from_front.0, from_front.1), Some(PieceId(1)));
        let from_behind = (DVec3::new(0.0, 1.0, -3.0), DVec3::Z);
        assert_eq!(scene.pick(from_behind.0, from_behind.1), Some(PieceId(2)));
        assert_eq!(
            scene.pick(DVec3::new(2.0, 1.0, 3.0), DVec3::NEG_Z),
            None,
            "beside them"
        );
    }
}
```

Append to `crates/app/tests/ui.rs`:

```rust
/// Presses and releases the primary button at screen point `pos`.
fn click_at(h: &mut App, pos: glam::DVec2) {
    let p = egui::pos2(pos.x as f32, pos.y as f32);
    h.hover_at(p);
    for pressed in [true, false] {
        h.event(egui::Event::PointerButton {
            pos: p,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        });
    }
    h.run();
}

#[test]
fn clicking_a_piece_in_3d_selects_it_in_the_pattern_window() {
    let dir = tempfile::tempdir().unwrap();
    let mut h = harness(dir.path(), SharedState::default());
    h.run();
    add_piece(&mut h);
    let camera = h.state().view_camera().expect("the 3D view was drawn");
    let centre = h.state_mut().arranged_scene().panels[0].placement.position;
    let on_piece = camera.project(glam::DVec3::from_array(centre)).unwrap();
    click_at(&mut h, on_piece);
    assert_eq!(h.state().editor().selection, Selection::Piece(PieceId(1)));
    // The view's top-left corner: only background there.
    let (corner, _) = camera.rect();
    click_at(&mut h, corner + glam::DVec2::new(8.0, 8.0));
    assert_eq!(h.state().editor().selection, Selection::None);
}

#[test]
fn the_view_buttons_turn_the_camera_to_each_side() {
    use std::f32::consts::{FRAC_PI_2, PI};
    let dir = tempfile::tempdir().unwrap();
    let mut h = harness(dir.path(), SharedState::default());
    h.run();
    for (label, yaw) in [
        ("Back", PI),
        ("Left side", FRAC_PI_2),
        ("Right side", -FRAC_PI_2),
        ("Front", 0.0),
    ] {
        h.get_by_label(label).click();
        h.run();
        let camera = h.state().orbit_camera().unwrap();
        assert!((camera.yaw - yaw).abs() < 1e-6, "{label}: {}", camera.yaw);
    }
}
```

Run: `cargo nextest run -p opendrape-render && cargo nextest run -p opendrape`
Expected: compile errors (`set_color`, `scene`, `view_camera`, …).

- [ ] **Step 3: A mesh's colour** (`crates/render/src/mesh.rs`, after `pub struct GpuMesh { … }`)

```rust
impl GpuMesh {
    /// Draws the mesh in `color` from the next frame on.
    pub fn set_color(&mut self, color: [f32; 3]) {
        self.color = color;
    }
}
```

- [ ] **Step 4: The arranged scene** (`crates/app/src/arrange/scene.rs`, above its tests)

```rust
//! The pieces as the 3D view shows them while arranging: each shape's fabric (made coarser
//! than for draping: it is only looked at and clicked) at its placement. The fabric is made
//! again only when the pattern changes; moving a piece only places it again.

use super::gizmo::ray_triangle;
use glam::DVec3;
use opendrape_core::{PieceId, Placement, Point2, Project};
use opendrape_geom as geom;
use opendrape_mesh::{GarmentMesh, MeshParams, place};
use std::rc::Rc;

/// Edge length (mm) of the fabric shown while arranging.
pub const VIEW_EDGE_MM: f64 = 20.0;

/// One piece as shown in 3D.
#[derive(Clone, Debug, PartialEq)]
pub struct ArrangedPanel {
    pub shape: PieceId,
    /// Where it is shown and draped from (its own placement, or the one it takes).
    pub placement: Placement,
    pub positions: Vec<DVec3>,
    pub triangles: Vec<[u32; 3]>,
}

/// Every piece that could be made into fabric, at its placement.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ArrangedScene {
    pub panels: Vec<ArrangedPanel>,
}

impl ArrangedScene {
    pub fn panel(&self, shape: PieceId) -> Option<&ArrangedPanel> {
        self.panels.iter().find(|p| p.shape == shape)
    }
    /// The piece the ray from `origin` along unit `dir` meets first.
    pub fn pick(&self, origin: DVec3, dir: DVec3) -> Option<PieceId> {
        self.panels
            .iter()
            .flat_map(|p| {
                p.triangles.iter().filter_map(move |t| {
                    ray_triangle(origin, dir, t.map(|k| p.positions[k as usize]))
                        .map(|d| (p.shape, d))
                })
            })
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .map(|(shape, _)| shape)
    }
}

/// The arranged scene, kept until the project changes.
#[derive(Default)]
pub struct SceneCache {
    /// The pattern the fabric was made from: the project with every placement taken out.
    pattern: Option<Project>,
    mesh: Rc<GarmentMesh>,
    /// The project and shoulder height the scene was placed for.
    placed: Option<(Project, f64)>,
    scene: Rc<ArrangedScene>,
    /// How many times the fabric has been made (tests count them).
    pub meshed: usize,
}

impl SceneCache {
    /// The pieces of `project` at their placements, for a form whose shoulders are at
    /// `shoulder_y`. Reused while nothing changed; only re-placed when only placements did.
    pub fn scene(&mut self, project: &Project, shoulder_y: f64) -> Rc<ArrangedScene> {
        if self
            .placed
            .as_ref()
            .is_some_and(|(p, s)| p == project && *s == shoulder_y)
        {
            return self.scene.clone();
        }
        let pattern = without_placements(project);
        if self.pattern.as_ref() != Some(&pattern) {
            self.mesh = Rc::new(opendrape_mesh::build(
                &pattern,
                &MeshParams {
                    edge_mm: VIEW_EDGE_MM,
                    ..MeshParams::default()
                },
            ));
            self.pattern = Some(pattern);
            self.meshed += 1;
        }
        let shapes = geom::shapes(project);
        let layout = place::layout(&shapes);
        let panels = self
            .mesh
            .panels
            .iter()
            .filter_map(|panel| {
                let shape = shapes.iter().find(|s| s.id == panel.shape)?;
                let placement = place::effective(project, shape, &layout, shoulder_y);
                let positions = panel
                    .flat
                    .iter()
                    .map(|f| {
                        place::apply(
                            &placement,
                            panel.centre,
                            Point2::new(f[0] * 1000.0, f[1] * 1000.0),
                        )
                    })
                    .collect();
                Some(ArrangedPanel {
                    shape: panel.shape,
                    placement,
                    positions,
                    triangles: panel.triangles.clone(),
                })
            })
            .collect();
        self.scene = Rc::new(ArrangedScene { panels });
        self.placed = Some((project.clone(), shoulder_y));
        self.scene.clone()
    }
}

/// `project` with no placements: what the fabric's shape depends on.
fn without_placements(project: &Project) -> Project {
    let mut p = project.clone();
    for piece in &mut p.pieces {
        piece.placement = None;
        if let Some(t) = &mut piece.twin {
            t.placement = None;
        }
    }
    p
}
```

Replace `crates/app/src/arrange/mod.rs` with:

```rust
//! Arranging pieces in 3D before draping: the view's maths (`gizmo`), the pieces as shown
//! (`scene`), and what the pointer does to them (`Arranger`).

pub mod gizmo;
pub mod scene;

pub use gizmo::ScreenCamera;
pub use scene::{ArrangedPanel, ArrangedScene, SceneCache};

use crate::editor::Selection;
use glam::DVec2;

/// What the pointer does in the 3D view while arranging.
#[derive(Default)]
pub struct Arranger {}

impl Arranger {
    /// A click: the piece under the pointer becomes the selection (in the pattern window too);
    /// a click on nothing clears it.
    pub fn click(
        &mut self,
        cam: &ScreenCamera,
        scene: &ArrangedScene,
        selection: &mut Selection,
        pos: DVec2,
    ) {
        let (origin, dir) = cam.ray(pos);
        *selection = scene
            .pick(origin, dir)
            .map_or(Selection::None, Selection::Piece);
    }
}
```

- [ ] **Step 5: The viewport shows pieces or the drape** (replace `crates/app/src/viewport.rs`)

```rust
use crate::arrange::{ArrangedScene, ScreenCamera};
use crate::sim_runner::SimFrame;
use crate::stage::Stage;
use glam::DVec2;
use opendrape_core::PieceId;
use opendrape_render::{GpuMesh, MeshRenderer, OrbitCamera, RenderTarget, target_size};
use std::rc::Rc;
use std::sync::Arc;

/// Mid-brown skin tone, a cotton blue, and the selected piece's warmer blue.
const SKIN: [f32; 3] = [0.62, 0.45, 0.36];
const FABRIC: [f32; 3] = [0.17, 0.36, 0.70];
const SELECTED_FABRIC: [f32; 3] = [0.95, 0.55, 0.25];

struct ClothOnGpu {
    mesh: GpuMesh,
    seq: u64,
    triangles: Arc<Vec<[u32; 3]>>,
}

/// What the 3D view shows besides the form.
pub enum Show<'a> {
    /// The drape, frame by frame.
    Drape(&'a SimFrame),
    /// The pieces being arranged, the selected one highlighted.
    Pieces {
        scene: &'a Rc<ArrangedScene>,
        selected: Option<PieceId>,
    },
}

/// The 3D image as drawn this frame: its response to the pointer, and the camera that drew it.
pub struct Drawn {
    pub response: egui::Response,
    pub camera: ScreenCamera,
}

/// The 3D panel: renders offscreen and shows the texture as an egui image.
pub struct Viewport {
    renderer: MeshRenderer,
    camera: OrbitCamera,
    target: Option<(RenderTarget, egui::TextureId)>,
    body: GpuMesh,
    cloth: Option<ClothOnGpu>,
    /// The arranged pieces on the GPU, and the scene they were made from.
    pieces: Vec<(PieceId, GpuMesh)>,
    pieces_of: Option<Rc<ArrangedScene>>,
    pub frames_drawn: u64,
}

impl Viewport {
    pub fn new(rs: &egui_wgpu::RenderState, stage: &Stage) -> Self {
        let renderer = MeshRenderer::new(&rs.device);
        let (positions, triangles) = stage.render_mesh();
        let body = renderer.create_mesh(&rs.device, &rs.queue, positions, triangles, SKIN);
        let camera = OrbitCamera {
            target: glam::Vec3::new(0.0, 0.95, 0.0),
            yaw: 0.5,
            pitch: 0.12,
            distance: 2.6,
            fov_y: 35f32.to_radians(),
        };
        Self {
            renderer,
            camera,
            target: None,
            body,
            cloth: None,
            pieces: Vec::new(),
            pieces_of: None,
            frames_drawn: 0,
        }
    }

    pub fn camera(&self) -> &OrbitCamera {
        &self.camera
    }

    pub fn camera_mut(&mut self) -> &mut OrbitCamera {
        &mut self.camera
    }

    /// Turns the camera round to look at the form from `angle` (radians from its front towards
    /// its left), a little from above.
    pub fn look_from(&mut self, angle: f64) {
        self.camera.yaw = angle as f32;
        self.camera.pitch = 0.1;
    }

    /// Draws the form and `show`, and returns the image's response with the camera that drew
    /// it; None when the panel is too small to draw in.
    pub fn ui(
        &mut self,
        ui: &mut egui::Ui,
        rs: &egui_wgpu::RenderState,
        show: Show,
    ) -> Option<Drawn> {
        let size = ui.available_size();
        let max_dim = rs.device.limits().max_texture_dimension_2d;
        let (w, h) = target_size(size.x, size.y, ui.pixels_per_point(), max_dim)?;
        match show {
            Show::Drape(f) => {
                self.pieces.clear();
                self.pieces_of = None;
                self.sync_cloth(rs, f);
            }
            Show::Pieces { scene, selected } => {
                self.cloth = None;
                self.sync_pieces(rs, scene, selected);
            }
        }
        self.ensure_target(rs, w, h);
        let (target, texture_id) = self.target.as_ref().expect("ensure_target sets it");
        let mut meshes = vec![&self.body];
        meshes.extend(self.cloth.as_ref().map(|c| &c.mesh));
        meshes.extend(self.pieces.iter().map(|(_, m)| m));
        self.renderer.render(
            &rs.device,
            &rs.queue,
            target,
            self.camera.view_proj(w as f32 / h as f32),
            &meshes,
        );
        let texture_id = *texture_id;
        self.frames_drawn += 1;
        let image = egui::Image::new(egui::load::SizedTexture::new(texture_id, size));
        let response = ui.add(image.sense(egui::Sense::click_and_drag()));
        let rect = response.rect;
        let camera = ScreenCamera::new(
            &self.camera,
            DVec2::new(f64::from(rect.min.x), f64::from(rect.min.y)),
            DVec2::new(f64::from(rect.width()), f64::from(rect.height())),
        );
        Some(Drawn { response, camera })
    }

    /// Uploads the arranged pieces when the scene changed, and colours the selected one.
    fn sync_pieces(
        &mut self,
        rs: &egui_wgpu::RenderState,
        scene: &Rc<ArrangedScene>,
        selected: Option<PieceId>,
    ) {
        if !self
            .pieces_of
            .as_ref()
            .is_some_and(|s| Rc::ptr_eq(s, scene))
        {
            self.pieces = scene
                .panels
                .iter()
                .map(|p| {
                    let positions: Vec<glam::Vec3> =
                        p.positions.iter().map(|q| q.as_vec3()).collect();
                    let mesh = self.renderer.create_mesh(
                        &rs.device,
                        &rs.queue,
                        &positions,
                        &p.triangles,
                        FABRIC,
                    );
                    (p.shape, mesh)
                })
                .collect();
            self.pieces_of = Some(scene.clone());
        }
        for (shape, mesh) in &mut self.pieces {
            mesh.set_color(if Some(*shape) == selected {
                SELECTED_FABRIC
            } else {
                FABRIC
            });
        }
    }

    /// Uploads a new simulation frame (and new triangles after welding) once per frame number.
    fn sync_cloth(&mut self, rs: &egui_wgpu::RenderState, f: &SimFrame) {
        match &mut self.cloth {
            Some(c) if c.seq == f.seq => {}
            Some(c) => {
                let new_tris =
                    (!Arc::ptr_eq(&c.triangles, &f.triangles)).then_some(f.triangles.as_slice());
                self.renderer.update_mesh(
                    &rs.device,
                    &rs.queue,
                    &mut c.mesh,
                    &f.positions,
                    new_tris,
                );
                c.seq = f.seq;
                c.triangles = f.triangles.clone();
            }
            None => {
                let mesh = self.renderer.create_mesh(
                    &rs.device,
                    &rs.queue,
                    &f.positions,
                    &f.triangles,
                    FABRIC,
                );
                self.cloth = Some(ClothOnGpu {
                    mesh,
                    seq: f.seq,
                    triangles: f.triangles.clone(),
                });
            }
        }
    }

    /// (Re)create the render target when the panel's pixel size changes, keeping the same egui texture id.
    fn ensure_target(&mut self, rs: &egui_wgpu::RenderState, w: u32, h: u32) {
        if self
            .target
            .as_ref()
            .is_some_and(|(t, _)| t.width == w && t.height == h)
        {
            return;
        }
        let target = RenderTarget::new(&rs.device, w, h);
        let mut renderer = rs.renderer.write();
        let id = match self.target.take() {
            Some((_, id)) => {
                renderer.update_egui_texture_from_wgpu_texture(
                    &rs.device,
                    &target.color_view,
                    wgpu::FilterMode::Linear,
                    id,
                );
                id
            }
            None => renderer.register_native_texture(
                &rs.device,
                &target.color_view,
                wgpu::FilterMode::Linear,
            ),
        };
        self.target = Some((target, id));
    }
}
```

- [ ] **Step 6: The app** (`crates/app/src/app.rs`)

- Imports: add `use crate::arrange::{ArrangedScene, Arranger, SceneCache, ScreenCamera};`, `use opendrape_mesh::place::PlaceAt;` and `use opendrape_render::OrbitCamera;`, and change the viewport import to `use crate::viewport::{Show, Viewport};`.
- `OpenDrapeApp` gains, after `draped`:

```rust
    /// The pieces as the 3D view shows them while arranging.
    arranged: SceneCache,
    /// What the pointer does in the 3D view while arranging.
    arranger: Arranger,
    /// The 3D view's camera as last drawn.
    view_camera: Option<ScreenCamera>,
```

  with `arranged: SceneCache::default(), arranger: Arranger::default(), view_camera: None,` in `new`.
- After `stage()`, add:

```rust
    /// The pieces as the 3D view shows them while arranging.
    pub fn arranged_scene(&mut self) -> Rc<ArrangedScene> {
        self.arranged
            .scene(self.editor.doc.project(), self.stage.shoulder_y())
    }

    /// The 3D view's camera and rectangle as last drawn.
    pub fn view_camera(&self) -> Option<ScreenCamera> {
        self.view_camera
    }

    /// The 3D view's orbit camera.
    pub fn orbit_camera(&self) -> Option<OrbitCamera> {
        self.viewport.as_ref().map(|v| *v.camera())
    }
```

- Replace `toolbar` (it gains the four view buttons) and the `Toolbar` enum:

```rust
    fn toolbar(&mut self, ui: &mut egui::Ui) {
        let Some(runner) = &self.runner else { return };
        let (draping, playing) = (runner.is_draping(), runner.is_playing());
        let mut clicked = None;
        ui.horizontal_wrapped(|ui| {
            let label = if draping && playing {
                tr!("toolbar-pause")
            } else {
                tr!("toolbar-play")
            };
            if ui.button(label).clicked() {
                clicked = Some(match (draping, playing) {
                    (false, _) => Toolbar::Play,
                    (true, true) => Toolbar::Pause,
                    (true, false) => Toolbar::Resume,
                });
            }
            if ui
                .add_enabled(draping, egui::Button::new(tr!("toolbar-reset")))
                .clicked()
            {
                clicked = Some(Toolbar::Reset);
            }
            ui.separator();
            for (side, label) in [
                (PlaceAt::Front, tr!("view-front")),
                (PlaceAt::Back, tr!("view-back")),
                (PlaceAt::LeftSide, tr!("view-left")),
                (PlaceAt::RightSide, tr!("view-right")),
            ] {
                if ui.button(label).clicked() {
                    clicked = Some(Toolbar::Look(side));
                }
            }
        });
        match clicked {
            Some(Toolbar::Play) => {
                let snapshot = Arc::new(self.editor.doc.project().clone());
                runner.play(snapshot.clone());
                self.draped = Some(snapshot);
            }
            Some(Toolbar::Pause) => runner.set_playing(false),
            Some(Toolbar::Resume) => runner.set_playing(true),
            Some(Toolbar::Reset) => {
                runner.reset();
                self.draped = None;
            }
            Some(Toolbar::Look(side)) => {
                if let Some(v) = &mut self.viewport {
                    v.look_from(side.angle());
                }
            }
            None => {}
        }
    }
```

```rust
/// The toolbar buttons of the 3D view.
#[derive(Clone, Copy)]
enum Toolbar {
    Play,
    Pause,
    Resume,
    Reset,
    /// Turn the camera to look from this side of the form.
    Look(PlaceAt),
}
```

- Replace `view_3d` and add `arrange` after it:

```rust
    /// The 3D view: its toolbar and notes, the form with the pieces being arranged or the
    /// drape, and the speed overlay.
    fn view_3d(&mut self, ui: &mut egui::Ui, frame: &eframe::Frame) {
        self.reset_if_edited();
        self.toolbar(ui);
        self.notes(ui);
        ui.separator();
        let (Some(viewport), Some(rs)) = (self.viewport.as_mut(), frame.wgpu_render_state()) else {
            ui.centered_and_justified(|ui| ui.label(tr!("viewport-no-gpu")));
            return;
        };
        let sim = self.runner.as_ref().and_then(SimRunner::latest);
        let fps = self
            .runner
            .as_ref()
            .is_some_and(SimRunner::is_playing)
            .then_some(self.fps);
        // Kept up to date while draping too: the project doesn't change then, so it costs
        // nothing, and Reset shows the pieces at once.
        let scene = self
            .arranged
            .scene(self.editor.doc.project(), self.stage.shoulder_y());
        let rect = ui.available_rect_before_wrap();
        // Arranging until the drape's first frame arrives.
        let show = match &sim {
            Some(f) => Show::Drape(f),
            None => Show::Pieces {
                scene: &scene,
                selected: self.editor.selection.piece(),
            },
        };
        if let Some(drawn) = viewport.ui(ui, rs, show) {
            self.view_camera = Some(drawn.camera);
            if sim.is_none() {
                self.arrange(&drawn.response, &drawn.camera, &scene);
            }
            if let Some(viewport) = self.viewport.as_mut() {
                // A drag turns the camera.
                let drag = drawn.response.drag_delta();
                if drag != egui::Vec2::ZERO {
                    viewport.camera_mut().drag(drag.x, drag.y);
                }
                if drawn.response.hovered() {
                    let scroll = ui.input(|i| i.smooth_scroll_delta.y);
                    if scroll != 0.0 {
                        viewport.camera_mut().zoom(scroll);
                    }
                }
            }
        }
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

    /// The pointer in the 3D view while arranging: a click picks a piece (or clears the
    /// selection).
    fn arrange(&mut self, response: &egui::Response, cam: &ScreenCamera, scene: &ArrangedScene) {
        if response.clicked()
            && let Some(p) = response.interact_pointer_pos()
        {
            let at = glam::DVec2::new(f64::from(p.x), f64::from(p.y));
            self.arranger
                .click(cam, scene, &mut self.editor.selection, at);
        }
    }
```

- [ ] **Step 7: Run and see them pass**

Run: `cargo nextest run -p opendrape-render && cargo nextest run -p opendrape`
Expected: `a_mesh_can_change_colour`, the 2 scene tests and the 2 app tests pass, plus all earlier ones.

- [ ] **Step 8: Commit**

```bash
cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings
git add crates/render crates/app
git commit -m "feat(app): the pieces in 3D at their placements; click to select in both views; view buttons

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 9: The gizmo: drawn over the 3D view, dragged, undone in one step (review: full)

**Files:**
- Create:
  - `crates/app/src/arrange/overlay.rs`
  - `crates/app/tests/arrange.rs`
- Replace: `crates/app/src/arrange/mod.rs`
- Modify:
  - `crates/app/src/app.rs`
  - `crates/app/i18n/en-US/opendrape.ftl`

**Interfaces:**
- Consumes:
  - Task 4: `gizmo::{AXES, Gizmo, Handle, axis_drag, plane_drag, ring_angle, snap_angle, SQUARE_PT, ScreenCamera}`;
  - Task 8: `ArrangedScene::panel`, `ArrangedPanel.placement`, `SceneCache`, `Arranger::click`;
  - `Document::{begin_gesture, gesture_edit, end_gesture}`, `Project::set_placement`.
- Produces (`opendrape::arrange`):
  - `pub const SNAP_DEG: f64 = 15.0`;
  - `enum Moved { Along(usize, f64), Across(DVec3), Turned(f64) }`;
  - `Arranger`:
    - `pub hovered: Option<Handle>`;
    - `gizmo(&ScreenCamera, &ArrangedScene, &Selection) -> Option<Gizmo>` (associated);
    - `click(…)`, `hover(&mut self, cam, scene, &Selection, DVec2)`;
    - `press(&mut self, cam, scene, &Selection, &mut Document, DVec2) -> bool`, `drag_to(&mut self, cam, &mut Document, DVec2, shift: bool)`, `release(&mut self, &mut Document)`;
    - `is_dragging()`, `active() -> Option<Handle>`, `readout(Units) -> Option<String>`.
  - `overlay::{paint(&Painter, &ScreenCamera, &Gizmo, lit: Option<Handle>), paint_readout(&Painter, Pos2, String)}`.

**Behaviour:**
- **The gizmo** is drawn round the selected piece's centre (its placement's position), 80 points across whatever the zoom:
  - rings first, then arrows (an arrow pointing at the viewer is hidden), then the centre square;
  - the handle under the pointer, or being dragged, is drawn bright yellow.
- **Dragging:**
  - A press on a handle grabs it and begins a gesture; the camera doesn't turn.
  - Every move recomputes the placement from the one the drag began with: along the axis, in the view plane, or turned about a world axis through the piece's centre (Shift snaps to 15°).
  - Release ends the gesture: one undo step. A twin with no placement follows its piece; a twin that is dragged gets its own placement.
- **The readout** follows the pointer: "12.0 cm up", "12.0 cm down", "… to the form's left", "… to the form's right", "… forward", "… back", "12.0 cm" for the centre square, or "45°".

- [ ] **Step 1: Strings** (append to the `.ftl`)

```
gizmo-up = { $distance } up
gizmo-down = { $distance } down
gizmo-left = { $distance } to the form's left
gizmo-right = { $distance } to the form's right
gizmo-forward = { $distance } forward
gizmo-back = { $distance } back
gizmo-moved = { $distance }
gizmo-turned = { $angle }°
```

- [ ] **Step 2: Failing tests** (`crates/app/tests/arrange.rs`: headless, a fixed camera and rectangle, no window, no GPU)

```rust
//! Arranging pieces in 3D, driven through the gizmo's input handler with a fixed camera: no
//! window and no graphics card needed.

use glam::{DQuat, DVec2, DVec3, Vec3};
use opendrape::arrange::gizmo::{ARROW_PT, AXES, Gizmo, Handle, RING_PT};
use opendrape::arrange::{Arranger, SceneCache, ScreenCamera};
use opendrape::editor::{Document, Selection};
use opendrape_core::{Piece, PieceId, Point2, Units};
use opendrape_mesh::place;
use opendrape_render::OrbitCamera;

const SHOULDER: f64 = 1.3;

/// The view the tests look through: from the front left, a little from above.
fn camera() -> ScreenCamera {
    let orbit = OrbitCamera {
        target: Vec3::new(0.0, 1.0, 0.0),
        yaw: 0.6,
        pitch: 0.3,
        distance: 2.6,
        fov_y: 35f32.to_radians(),
    };
    ScreenCamera::new(&orbit, DVec2::new(0.0, 30.0), DVec2::new(700.0, 600.0))
}

/// A document with one 300 × 400 mm piece, selected.
fn one_piece() -> (Document, PieceId, Selection) {
    let mut doc = Document::default();
    let id = doc.edit(|p| {
        p.add_piece(Piece::rectangle(
            PieceId(0),
            "Front",
            Point2::new(0.0, 0.0),
            300.0,
            400.0,
        ))
    });
    (doc, id, Selection::Piece(id))
}

fn position(doc: &Document, id: PieceId, cache: &mut SceneCache) -> DVec3 {
    let scene = cache.scene(doc.project(), SHOULDER);
    DVec3::from_array(scene.panel(id).unwrap().placement.position)
}

fn gizmo(doc: &Document, cache: &mut SceneCache, sel: &Selection) -> Gizmo {
    let scene = cache.scene(doc.project(), SHOULDER);
    Arranger::gizmo(&camera(), &scene, sel).expect("a selected piece has a gizmo")
}

/// Presses on `from`, moves to `to`, releases (screen points).
fn drag(
    arranger: &mut Arranger,
    doc: &mut Document,
    cache: &mut SceneCache,
    sel: &Selection,
    from: DVec2,
    to: DVec2,
    shift: bool,
) -> bool {
    let cam = camera();
    let scene = cache.scene(doc.project(), SHOULDER);
    if !arranger.press(&cam, &scene, sel, doc, from) {
        return false;
    }
    arranger.drag_to(&cam, doc, from.lerp(to, 0.5), shift);
    arranger.drag_to(&cam, doc, to, shift);
    arranger.release(doc);
    true
}

#[test]
fn dragging_an_arrow_moves_the_piece_along_it_as_one_undo_step() {
    let (mut doc, id, sel) = one_piece();
    let mut cache = SceneCache::default();
    let mut arranger = Arranger::default();
    let start = position(&doc, id, &mut cache);
    let g = gizmo(&doc, &mut cache, &sel);
    let cam = camera();
    // Grab the up arrow near its tip and pull it up by the screen length of 10 cm.
    let tip = g.arrow_tip(1) - AXES[1] * g.size * 0.1;
    let (from, to) = (
        cam.project(tip).unwrap(),
        cam.project(tip + DVec3::Y * 0.1).unwrap(),
    );
    assert!(drag(
        &mut arranger,
        &mut doc,
        &mut cache,
        &sel,
        from,
        to,
        false
    ));
    let moved = position(&doc, id, &mut cache);
    assert!(
        (moved - (start + DVec3::Y * 0.1)).length() < 1e-6,
        "{start} → {moved}"
    );
    assert!(doc.undo(), "one step");
    assert_eq!(
        doc.project().pieces[0].placement,
        None,
        "back at its starting place"
    );
}

#[test]
fn dragging_a_ring_turns_the_piece_and_shift_snaps_to_15_degrees() {
    let (mut doc, _, sel) = one_piece();
    let mut cache = SceneCache::default();
    let mut arranger = Arranger::default();
    let g = gizmo(&doc, &mut cache, &sel);
    let cam = camera();
    // A point of the y ring that grabs it, and the point 40° further round.
    let (u, v) = AXES[1].any_orthonormal_pair();
    let r = g.size * RING_PT / ARROW_PT;
    let on_ring = |a: f64| g.centre + (u * a.cos() + v * a.sin()) * r;
    let a0 = (0..36)
        .map(|k| f64::from(k) * 10f64.to_radians())
        .find(|a| g.hit(&cam, cam.project(on_ring(*a)).unwrap()) == Some(Handle::Turn(1)))
        .expect("a point that grabs the y ring");
    let turn = 40f64.to_radians();
    let (from, to) = (
        cam.project(on_ring(a0)).unwrap(),
        cam.project(on_ring(a0 + turn)).unwrap(),
    );
    assert!(drag(
        &mut arranger,
        &mut doc,
        &mut cache,
        &sel,
        from,
        to,
        false
    ));
    let q = DQuat::from_array(doc.project().pieces[0].placement.unwrap().rotation);
    let (axis, angle) = q.to_axis_angle();
    assert!(
        (angle - turn).abs() < 1e-6 && (axis - u.cross(v)).length() < 1e-6,
        "{axis} {angle}"
    );
    doc.undo();
    assert!(drag(
        &mut arranger,
        &mut doc,
        &mut cache,
        &sel,
        from,
        to,
        true
    ));
    let q = DQuat::from_array(doc.project().pieces[0].placement.unwrap().rotation);
    assert!(
        (q.to_axis_angle().1 - 45f64.to_radians()).abs() < 1e-6,
        "snapped to 45°"
    );
}

#[test]
fn the_centre_square_moves_the_piece_under_the_pointer() {
    let (mut doc, id, sel) = one_piece();
    let mut cache = SceneCache::default();
    let mut arranger = Arranger::default();
    let start = position(&doc, id, &mut cache);
    let cam = camera();
    let from = cam.project(start).unwrap();
    let to = from + DVec2::new(40.0, 25.0);
    assert!(drag(
        &mut arranger,
        &mut doc,
        &mut cache,
        &sel,
        from,
        to,
        false
    ));
    let moved = position(&doc, id, &mut cache);
    assert!(cam.project(moved).unwrap().distance(to) < 1e-6);
}

#[test]
fn a_press_away_from_the_gizmo_leaves_the_drag_to_the_camera() {
    let (mut doc, _, sel) = one_piece();
    let mut cache = SceneCache::default();
    let mut arranger = Arranger::default();
    let far = DVec2::new(5.0, 40.0);
    assert!(!drag(
        &mut arranger,
        &mut doc,
        &mut cache,
        &sel,
        far,
        far + DVec2::new(30.0, 0.0),
        false
    ));
    assert!(!arranger.is_dragging());
    assert!(!drag(
        &mut arranger,
        &mut doc,
        &mut cache,
        &Selection::None,
        far,
        far,
        false
    ));
    assert_eq!(doc.project().pieces[0].placement, None);
}

#[test]
fn moving_a_piece_moves_its_unplaced_twin_and_a_moved_twin_keeps_its_place() {
    let (mut doc, id, sel) = one_piece();
    let twin = doc
        .edit(|p| p.add_twin(id, "Front (mirror)".into(), Point2::new(700.0, 0.0)))
        .unwrap();
    let mut cache = SceneCache::default();
    let mut arranger = Arranger::default();
    let cam = camera();
    let start = position(&doc, id, &mut cache);
    let to = cam.project(start + DVec3::new(0.0, 0.05, 0.0)).unwrap();
    assert!(drag(
        &mut arranger,
        &mut doc,
        &mut cache,
        &sel,
        cam.project(start).unwrap(),
        to,
        false
    ));
    let placed = doc.project().pieces[0].placement.unwrap();
    let scene = cache.scene(doc.project(), SHOULDER);
    assert_eq!(
        scene.panel(twin).unwrap().placement,
        place::mirrored(&placed),
        "mirrors it"
    );
    // Now move the twin itself: it keeps its own place from then on.
    let twin_sel = Selection::Piece(twin);
    let at = position(&doc, twin, &mut cache);
    let to = cam.project(at + DVec3::new(0.0, -0.05, 0.0)).unwrap();
    assert!(drag(
        &mut arranger,
        &mut doc,
        &mut cache,
        &twin_sel,
        cam.project(at).unwrap(),
        to,
        false
    ));
    let own = doc.project().placement_of(twin).expect("its own placement");
    let at = cam.project(position(&doc, id, &mut cache)).unwrap();
    drag(
        &mut arranger,
        &mut doc,
        &mut cache,
        &sel,
        at,
        at + DVec2::new(20.0, 0.0),
        false,
    );
    assert_eq!(
        doc.project().placement_of(twin),
        Some(own),
        "the twin stays put"
    );
}

#[test]
fn the_readout_says_how_far_and_which_way() {
    let (mut doc, _, sel) = one_piece();
    let mut cache = SceneCache::default();
    let mut arranger = Arranger::default();
    let g = gizmo(&doc, &mut cache, &sel);
    let cam = camera();
    let scene = cache.scene(doc.project(), SHOULDER);
    let tip = g.arrow_tip(1) - AXES[1] * g.size * 0.1;
    assert!(arranger.press(&cam, &scene, &sel, &mut doc, cam.project(tip).unwrap()));
    assert_eq!(arranger.readout(Units::Cm), None, "nothing moved yet");
    arranger.drag_to(
        &cam,
        &mut doc,
        cam.project(tip - DVec3::Y * 0.12).unwrap(),
        false,
    );
    assert_eq!(arranger.readout(Units::Cm).as_deref(), Some("12.0 cm down"));
    arranger.release(&mut doc);
    assert_eq!(arranger.readout(Units::Cm), None);
}

#[test]
fn the_handle_under_the_pointer_is_noted_for_drawing() {
    let (doc, _, sel) = one_piece();
    let mut cache = SceneCache::default();
    let mut arranger = Arranger::default();
    let g = gizmo(&doc, &mut cache, &sel);
    let cam = camera();
    let scene = cache.scene(doc.project(), SHOULDER);
    arranger.hover(&cam, &scene, &sel, cam.project(g.centre).unwrap());
    assert_eq!(arranger.hovered, Some(Handle::Plane));
    arranger.hover(
        &cam,
        &scene,
        &Selection::None,
        cam.project(g.centre).unwrap(),
    );
    assert_eq!(arranger.hovered, None, "no gizmo without a selected piece");
}
```

Run: `cargo nextest run -p opendrape --test arrange`
Expected: compile errors (`press`, `drag_to`, `readout`, `hovered` don't exist yet).

- [ ] **Step 3: The arranger** (replace `crates/app/src/arrange/mod.rs`)

```rust
//! Arranging pieces in 3D before draping: the view's maths (`gizmo`), the pieces as shown
//! (`scene`), what the pointer does to them (`Arranger`), and the gizmo drawn over the view
//! (`overlay`).

pub mod gizmo;
pub mod overlay;
pub mod scene;

pub use gizmo::ScreenCamera;
pub use scene::{ArrangedPanel, ArrangedScene, SceneCache};

use crate::editor::{Document, Selection};
use crate::tr;
use gizmo::{AXES, Gizmo, Handle, axis_drag, plane_drag, ring_angle, snap_angle};
use glam::{DQuat, DVec2, DVec3};
use opendrape_core::{PieceId, Placement, Units};

/// Shift snaps a turn to steps of this many degrees.
pub const SNAP_DEG: f64 = 15.0;

/// What a gizmo drag has done so far.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Moved {
    /// Metres along axis 0 (x), 1 (y) or 2 (z).
    Along(usize, f64),
    /// Metres in the plane facing the viewer.
    Across(DVec3),
    /// Radians about an axis.
    Turned(f64),
}

struct GizmoDrag {
    handle: Handle,
    shape: PieceId,
    /// The piece's placement when the drag began.
    original: Placement,
    start: DVec2,
    moved: Option<Moved>,
}

/// What the pointer does in the 3D view while arranging: clicking picks a piece, and the
/// selected piece's gizmo moves and turns it, one undo step per drag.
#[derive(Default)]
pub struct Arranger {
    /// The gizmo handle under the pointer, drawn highlighted.
    pub hovered: Option<Handle>,
    drag: Option<GizmoDrag>,
}

impl Arranger {
    /// The selected piece's gizmo, when a piece shown in `scene` is selected.
    pub fn gizmo(
        cam: &ScreenCamera,
        scene: &ArrangedScene,
        selection: &Selection,
    ) -> Option<Gizmo> {
        let Selection::Piece(id) = *selection else {
            return None;
        };
        let panel = scene.panel(id)?;
        Some(Gizmo::new(cam, DVec3::from_array(panel.placement.position)))
    }

    /// A click: the piece under the pointer becomes the selection (in the pattern window too);
    /// a click on nothing clears it.
    pub fn click(
        &mut self,
        cam: &ScreenCamera,
        scene: &ArrangedScene,
        selection: &mut Selection,
        pos: DVec2,
    ) {
        let (origin, dir) = cam.ray(pos);
        *selection = scene
            .pick(origin, dir)
            .map_or(Selection::None, Selection::Piece);
    }

    /// The pointer moved with no button down: note the handle under it.
    pub fn hover(
        &mut self,
        cam: &ScreenCamera,
        scene: &ArrangedScene,
        selection: &Selection,
        pos: DVec2,
    ) {
        self.hovered = Self::gizmo(cam, scene, selection).and_then(|g| g.hit(cam, pos));
    }

    /// A drag starts at `pos`: on a handle of the selected piece's gizmo, it grabs it and
    /// returns true (the drag moves the piece); anywhere else it returns false (the drag turns
    /// the camera).
    pub fn press(
        &mut self,
        cam: &ScreenCamera,
        scene: &ArrangedScene,
        selection: &Selection,
        doc: &mut Document,
        pos: DVec2,
    ) -> bool {
        let Selection::Piece(shape) = *selection else {
            return false;
        };
        let (Some(gizmo), Some(panel)) = (Self::gizmo(cam, scene, selection), scene.panel(shape))
        else {
            return false;
        };
        let Some(handle) = gizmo.hit(cam, pos) else {
            return false;
        };
        doc.begin_gesture();
        self.drag = Some(GizmoDrag {
            handle,
            shape,
            original: panel.placement,
            start: pos,
            moved: None,
        });
        true
    }

    /// The pointer moved to `pos` during a gizmo drag: the piece follows. Shift snaps a turn
    /// to 15° steps.
    pub fn drag_to(&mut self, cam: &ScreenCamera, doc: &mut Document, pos: DVec2, shift: bool) {
        let Some(d) = &mut self.drag else { return };
        let o = d.original;
        let centre = DVec3::from_array(o.position);
        let (moved, placement) = match d.handle {
            Handle::Move(k) => {
                let Some(m) = axis_drag(cam, centre, AXES[k], d.start, pos) else {
                    return;
                };
                let to = centre + AXES[k] * m;
                (
                    Moved::Along(k, m),
                    Placement {
                        position: to.to_array(),
                        ..o
                    },
                )
            }
            Handle::Plane => {
                let Some(v) = plane_drag(cam, centre, d.start, pos) else {
                    return;
                };
                (
                    Moved::Across(v),
                    Placement {
                        position: (centre + v).to_array(),
                        ..o
                    },
                )
            }
            Handle::Turn(k) => {
                let Some(mut a) = ring_angle(cam, centre, AXES[k], d.start, pos) else {
                    return;
                };
                if shift {
                    a = snap_angle(a, SNAP_DEG);
                }
                let turned = DQuat::from_axis_angle(AXES[k], a) * DQuat::from_array(o.rotation);
                (
                    Moved::Turned(a),
                    Placement {
                        rotation: turned.normalize().to_array(),
                        ..o
                    },
                )
            }
        };
        d.moved = Some(moved);
        let shape = d.shape;
        doc.gesture_edit(|p| p.set_placement(shape, Some(placement)));
    }

    /// The drag ended: everything it did is one undo step.
    pub fn release(&mut self, doc: &mut Document) {
        if self.drag.take().is_some() {
            doc.end_gesture();
        }
    }

    /// A gizmo drag is under way.
    pub fn is_dragging(&self) -> bool {
        self.drag.is_some()
    }

    /// The handle being dragged.
    pub fn active(&self) -> Option<Handle> {
        self.drag.as_ref().map(|d| d.handle)
    }

    /// What the drag has done, as the student reads it: "12.0 cm up", "45°".
    pub fn readout(&self, units: Units) -> Option<String> {
        let moved = self.drag.as_ref()?.moved?;
        let length = |m: f64| units.format(m.abs() * 1000.0);
        Some(match moved {
            Moved::Along(0, m) if m >= 0.0 => tr!("gizmo-left", distance = length(m)),
            Moved::Along(0, m) => tr!("gizmo-right", distance = length(m)),
            Moved::Along(1, m) if m >= 0.0 => tr!("gizmo-up", distance = length(m)),
            Moved::Along(1, m) => tr!("gizmo-down", distance = length(m)),
            Moved::Along(_, m) if m >= 0.0 => tr!("gizmo-forward", distance = length(m)),
            Moved::Along(_, m) => tr!("gizmo-back", distance = length(m)),
            Moved::Across(v) => tr!("gizmo-moved", distance = length(v.length())),
            Moved::Turned(a) => tr!("gizmo-turned", angle = format!("{:.0}", a.to_degrees())),
        })
    }
}
```

- [ ] **Step 4: The overlay** (`crates/app/src/arrange/overlay.rs`)

```rust
//! The gizmo drawn over the 3D view with egui's painter: rings to turn, arrows to move along
//! an axis, a square to move facing the viewer. The handle under the pointer, or being
//! dragged, is drawn bright.

use super::gizmo::{Gizmo, Handle, SQUARE_PT, ScreenCamera};
use egui::{Color32, Painter, Pos2, Rect, Stroke, StrokeKind, pos2, vec2};
use glam::DVec2;

const AXIS_COLOURS: [Color32; 3] = [
    Color32::from_rgb(220, 60, 60),
    Color32::from_rgb(60, 170, 70),
    Color32::from_rgb(60, 110, 235),
];
const BRIGHT: Color32 = Color32::from_rgb(255, 200, 0);

fn screen(p: DVec2) -> Pos2 {
    pos2(p.x as f32, p.y as f32)
}

/// Draws `gizmo`, with `lit` (the handle under the pointer, or being dragged) bright.
pub fn paint(painter: &Painter, cam: &ScreenCamera, gizmo: &Gizmo, lit: Option<Handle>) {
    let Some(centre) = cam.project(gizmo.centre) else {
        return;
    };
    let colour = |handle: Handle, axis: usize| {
        if lit == Some(handle) {
            (BRIGHT, 3.5)
        } else {
            (AXIS_COLOURS[axis], 2.0)
        }
    };
    for axis in 0..3 {
        let (c, w) = colour(Handle::Turn(axis), axis);
        let ring: Vec<Pos2> = gizmo
            .ring(axis)
            .into_iter()
            .filter_map(|p| cam.project(p))
            .map(screen)
            .collect();
        painter.add(egui::Shape::closed_line(ring, Stroke::new(w, c)));
    }
    for axis in 0..3 {
        if !gizmo.arrow_shown(cam, axis) {
            continue;
        }
        if let Some(tip) = cam.project(gizmo.arrow_tip(axis)) {
            let (c, w) = colour(Handle::Move(axis), axis);
            let d = tip - centre;
            painter.arrow(
                screen(centre),
                vec2(d.x as f32, d.y as f32),
                Stroke::new(w + 1.0, c),
            );
        }
    }
    let square = Rect::from_center_size(screen(centre), vec2(2.0, 2.0) * SQUARE_PT as f32);
    let fill = if lit == Some(Handle::Plane) {
        BRIGHT
    } else {
        Color32::from_white_alpha(170)
    };
    painter.rect(
        square,
        2.0,
        fill,
        Stroke::new(1.0, Color32::from_gray(60)),
        StrokeKind::Middle,
    );
}

/// Draws the drag's readout beside the pointer.
pub fn paint_readout(painter: &Painter, at: Pos2, text: String) {
    painter.text(
        at + vec2(16.0, -16.0),
        egui::Align2::LEFT_BOTTOM,
        text,
        egui::FontId::proportional(14.0),
        Color32::from_gray(30),
    );
}
```

- [ ] **Step 5: Wire it into the 3D view** (`crates/app/src/app.rs`)

Replace `arrange` with:

```rust
    /// The pointer in the 3D view while arranging: clicks pick pieces, the selected piece's
    /// gizmo moves and turns it, and the gizmo is drawn over the view.
    fn arrange(
        &mut self,
        ui: &egui::Ui,
        response: &egui::Response,
        cam: &ScreenCamera,
        scene: &ArrangedScene,
    ) {
        let at = |p: egui::Pos2| glam::DVec2::new(f64::from(p.x), f64::from(p.y));
        let shift = ui.input(|i| i.modifiers.shift);
        let doc = &mut self.editor.doc;
        let selection = &mut self.editor.selection;
        if response.drag_started_by(egui::PointerButton::Primary)
            && let Some(p) = ui.input(|i| i.pointer.press_origin())
        {
            self.arranger.press(cam, scene, selection, doc, at(p));
        }
        if self.arranger.is_dragging()
            && let Some(p) = response.interact_pointer_pos()
        {
            self.arranger.drag_to(cam, doc, at(p), shift);
        }
        if response.drag_stopped() {
            self.arranger.release(doc);
        }
        if response.clicked()
            && let Some(p) = response.interact_pointer_pos()
        {
            self.arranger.click(cam, scene, selection, at(p));
        }
        if !self.arranger.is_dragging()
            && let Some(p) = response.hover_pos()
        {
            self.arranger.hover(cam, scene, selection, at(p));
        }
        // Drawn where the piece is now, after this frame's drag.
        let scene = self
            .arranged
            .scene(self.editor.doc.project(), self.stage.shoulder_y());
        let painter = ui.painter_at(response.rect);
        if let Some(gizmo) = Arranger::gizmo(cam, &scene, &self.editor.selection) {
            let lit = self.arranger.active().or(self.arranger.hovered);
            crate::arrange::overlay::paint(&painter, cam, &gizmo, lit);
        }
        if let (Some(text), Some(p)) = (
            self.arranger.readout(self.editor.doc.project().units),
            response.interact_pointer_pos(),
        ) {
            crate::arrange::overlay::paint_readout(&painter, p, text);
        }
    }
```

In `view_3d`, the call becomes `self.arrange(ui, &drawn.response, &drawn.camera, &scene);`, and the camera only turns when the gizmo isn't being dragged:

```rust
                // A drag that didn't grab the gizmo turns the camera.
                let drag = drawn.response.drag_delta();
                if drag != egui::Vec2::ZERO && !self.arranger.is_dragging() {
```

- [ ] **Step 6: Run and see them pass**

Run: `cargo nextest run -p opendrape`
Expected: the 7 arrange tests pass, plus all earlier ones.

- [ ] **Step 7: Commit**

```bash
cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings
git add crates/app
git commit -m "feat(app): an in-house gizmo to move and turn pieces in 3D, one undo step per drag

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 10: Place at…, Flat, and typed placement (review: light)

**Files:**
- Create:
  - `crates/app/src/editor/placing.rs`
  - `crates/app/tests/placing.rs`
- Modify:
  - `crates/app/src/editor/{mod.rs,canvas.rs,panel.rs}`
  - `crates/app/src/app.rs`
  - `crates/app/i18n/en-US/opendrape.ftl`
  - `crates/app/tests/{common/mod.rs,ui.rs}`

**Interfaces:**
- Consumes:
  - Task 4: `place::{place_at, effective, layout, PlaceAt, euler_xyz_deg, rotation_from_euler_xyz_deg}`;
  - Task 7: `Stage::{shared, shoulder_y, surface_distance}`;
  - Task 9: the app's `arrange`;
  - M2a's panel `field` / `apply_typed`, which become `pub(super)`.
- Produces:
  - `PatternEditor.stage: Option<Arc<Stage>>` (pub). It is kept by `set_project`, and the app sets it when there is a 3D view.
  - `editor::DEFAULT_SHOULDER_M = 1.3` (used when there is no stage).
  - `PatternEditor` methods:
    - `placement(&self, PieceId) -> Option<Placement>` (the effective one);
    - `place_at(&mut self, PieceId, PlaceAt)`, `flatten(&mut self, PieceId)`;
    - `place_menu(&mut self, &mut egui::Ui, PieceId)`.
  - `CanvasState.menu_for: Option<PieceId>`.
  - Test helper `common::right_click(h, x, y)`.

**Behaviour:**
- **The menu:** a right-click on a piece, in the pattern window or in 3D, selects it and opens:
  - **Place at front**, **Place at back**, **Place at left side**, **Place at right side**;
  - **Flat** (keeps the position and rotation, drops the curve).

  Each is one undo step and gives the shape (piece or twin) its own placement. Without a form (no 3D view) there is no menu.
- **Typed placement:** Properties shows a **3D placement** group for a selected piece or twin:
  - Position X/Y/Z in the student's units, and Rotation X/Y/Z in degrees (x, then y, then z, about the world's axes);
  - each starts from the effective placement;
  - each typed value is one undo step, and an invalid result (over 10 m away) is refused with `notice-refused`.

- [ ] **Step 1: Strings** (append to the `.ftl`)

```
place-front = Place at front
place-back = Place at back
place-left = Place at left side
place-right = Place at right side
place-flat = Flat
panel-placement = 3D placement
panel-position-x = Position X
panel-position-y = Position Y
panel-position-z = Position Z
panel-rotation-x = Rotation X
panel-rotation-y = Rotation Y
panel-rotation-z = Rotation Z
```

- [ ] **Step 2: Failing tests**

In `crates/app/tests/common/mod.rs`, before the `drag` helper (its doc comment starts "Press at"), add:

```rust
/// A right-click (secondary button) at pattern point (x, y) mm.
pub fn right_click(h: &mut H, x: f64, y: f64) {
    let p = at(h, x, y);
    h.hover_at(p);
    for pressed in [true, false] {
        h.event(Event::PointerButton {
            pos: p,
            button: PointerButton::Secondary,
            pressed,
            modifiers: Modifiers::NONE,
        });
    }
    h.run();
}
```

Create `crates/app/tests/placing.rs`:

```rust
//! M4a in the pattern window: placing pieces in 3D with Place at… and by typing, each one undo
//! step.

mod common;
use common::*;
use egui::Key;
use egui_kittest::kittest::Queryable;
use glam::{DQuat, DVec3};
use opendrape::editor::Selection;
use opendrape::stage::Stage;
use opendrape_core::{PieceId, Placement, Point2};
use opendrape_mesh::place;

/// The pattern window with the form to place pieces round.
fn harness_with_form() -> H {
    let mut h = harness();
    h.state_mut().stage = Some(Stage::shared());
    h.run();
    h
}

fn placement(h: &H, id: PieceId) -> Option<Placement> {
    h.state().doc.project().placement_of(id)
}

#[test]
fn place_at_front_wraps_the_piece_round_the_form_as_one_step() {
    let mut h = harness_with_form();
    let id = with_rectangle(&mut h);
    let height = h.state().placement(id).unwrap().position[1];
    right_click(&mut h, 250.0, 300.0);
    assert_eq!(h.state().selection, Selection::Piece(id));
    h.get_by_label("Place at front").click();
    h.run();
    let p = placement(&h, id).expect("placed");
    let r = p.curve.expect("curved round the form");
    assert!(
        p.position[0].abs() < 1e-9 && (p.position[2] - r).abs() < 1e-9,
        "{p:?}"
    );
    assert!((p.position[1] - height).abs() < 1e-9, "keeps its height");
    // Clear of the form: no point of the piece is inside it.
    let stage = Stage::shared();
    let centre = place::centre_of(&opendrape_geom::shapes(h.state().doc.project())[0]);
    for (x, y) in SQUARE {
        let q = place::apply(&p, centre, Point2::new(x, y));
        assert!(stage.signed_distance(q) > 0.0, "{q} is inside the form");
    }
    cmd(&mut h, Key::Z);
    assert_eq!(placement(&h, id), None);
}

#[test]
fn flat_takes_the_curve_away_and_leaves_the_piece_where_it_is() {
    let mut h = harness_with_form();
    let id = with_rectangle(&mut h);
    h.state_mut().place_at(id, place::PlaceAt::Back);
    let curved = placement(&h, id).unwrap();
    right_click(&mut h, 250.0, 300.0);
    h.get_by_label("Flat").click();
    h.run();
    assert_eq!(
        placement(&h, id),
        Some(Placement {
            curve: None,
            ..curved
        })
    );
}

#[test]
fn a_pair_placed_at_the_back_meets_at_the_centre_line() {
    let mut h = harness_with_form();
    let id = with_rectangle(&mut h); // 100..400; its twin will sit to its right
    let twin = h
        .state_mut()
        .doc
        .edit(|p| p.add_twin(id, "Front (mirror)".into(), Point2::new(950.0, 0.0)))
        .unwrap();
    h.state_mut().place_at(id, place::PlaceAt::Back);
    let p = placement(&h, id).unwrap();
    let shapes = opendrape_geom::shapes(h.state().doc.project());
    // The piece's right side faces its twin: it lies on the back centre line.
    let corner = place::apply(&p, place::centre_of(&shapes[0]), Point2::new(400.0, 300.0));
    assert!(corner.x.abs() < 1e-9 && corner.z < 0.0, "{corner}");
    // The twin has no placement of its own: it mirrors the piece and meets it there.
    let t = h.state().placement(twin).unwrap();
    let twin_corner = place::apply(&t, place::centre_of(&shapes[1]), Point2::new(550.0, 300.0));
    assert!(
        (twin_corner - corner).length() < 1e-9,
        "{twin_corner} vs {corner}"
    );
}

#[test]
fn typed_position_and_rotation_move_the_piece_one_step_each() {
    let mut h = harness_with_form();
    let id = with_rectangle(&mut h);
    click(&mut h, 250.0, 300.0);
    type_into(&mut h, "Position Y", "100");
    assert!((placement(&h, id).unwrap().position[1] - 1.0).abs() < 1e-9);
    type_into(&mut h, "Rotation Y", "90");
    let q = DQuat::from_array(placement(&h, id).unwrap().rotation);
    assert!(
        (q * DVec3::Z - DVec3::X).length() < 1e-9,
        "turned to face the form's left"
    );
    assert_eq!(field_text(&h, "Rotation Y"), "90.0");
    cmd(&mut h, Key::Z);
    assert_eq!(placement(&h, id).unwrap().rotation, Placement::NO_ROTATION);
    cmd(&mut h, Key::Z);
    assert_eq!(placement(&h, id), None);
}

#[test]
fn a_typed_twin_gets_its_own_placement() {
    let mut h = harness_with_form();
    let id = with_rectangle(&mut h);
    let twin = h
        .state_mut()
        .doc
        .edit(|p| p.add_twin(id, "Front (mirror)".into(), Point2::new(950.0, 0.0)))
        .unwrap();
    h.state_mut().selection = Selection::Piece(twin);
    h.run();
    type_into(&mut h, "Position Z", "-30");
    let own = placement(&h, twin).expect("its own now");
    assert!((own.position[2] + 0.3).abs() < 1e-9);
    assert_eq!(placement(&h, id), None, "the piece stays unplaced");
}

#[test]
fn without_a_form_there_is_no_place_at_menu() {
    let mut h = harness();
    with_rectangle(&mut h);
    right_click(&mut h, 250.0, 300.0);
    assert!(h.query_by_label("Place at front").is_none());
}
```

Append to `crates/app/tests/ui.rs`:

```rust
#[test]
fn right_clicking_a_piece_in_3d_offers_place_at() {
    let dir = tempfile::tempdir().unwrap();
    let mut h = harness(dir.path(), SharedState::default());
    h.run();
    add_piece(&mut h);
    let camera = h.state().view_camera().expect("the 3D view was drawn");
    let centre = h.state_mut().arranged_scene().panels[0].placement.position;
    let p = camera.project(glam::DVec3::from_array(centre)).unwrap();
    let p = egui::pos2(p.x as f32, p.y as f32);
    h.hover_at(p);
    for pressed in [true, false] {
        h.event(egui::Event::PointerButton {
            pos: p,
            button: egui::PointerButton::Secondary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        });
    }
    h.run();
    assert_eq!(h.state().editor().selection, Selection::Piece(PieceId(1)));
    h.get_by_label("Place at front").click();
    h.run();
    let placed = h.state().editor().doc.project().placement_of(PieceId(1));
    assert!(placed.is_some_and(|p| p.curve.is_some()), "{placed:?}");
}
```

Run: `cargo nextest run -p opendrape --test placing --test ui`
Expected: compile errors (`stage`, `placement`, `place_at` on the editor).

- [ ] **Step 3: Placing from the editor** (`crates/app/src/editor/placing.rs`)

```rust
//! Placing pieces in 3D from the pattern window and Properties: Place at… (wrapped round the
//! form at its front, back or sides), Flat, and typed positions and angles. Each is one undo
//! step, and works on a twin as on any piece (the twin then keeps a placement of its own).

use super::{PatternEditor, Selection};
use crate::tr;
use opendrape_core::{PieceId, Placement};
use opendrape_geom as geom;
use opendrape_mesh::place::{self, PlaceAt};

/// Where pieces start when there is no form to measure (the bundled body's shoulders, m).
pub const DEFAULT_SHOULDER_M: f64 = 1.3;

impl PatternEditor {
    /// The form's shoulder height: where the top of the pattern starts.
    pub(super) fn shoulder_y(&self) -> f64 {
        self.stage
            .as_ref()
            .map_or(DEFAULT_SHOULDER_M, |s| s.shoulder_y())
    }

    /// Where piece or twin `id` is in 3D now: its own placement, or the one it takes.
    pub fn placement(&self, id: PieceId) -> Option<Placement> {
        let project = self.doc.project();
        let shapes = geom::shapes(project);
        let shape = shapes.iter().find(|s| s.id == id)?;
        Some(place::effective(
            project,
            shape,
            &place::layout(&shapes),
            self.shoulder_y(),
        ))
    }

    /// Gives `id` this placement, as one undo step.
    pub(super) fn set_placement(&mut self, id: PieceId, placement: Placement) {
        self.doc.edit(|p| p.set_placement(id, Some(placement)));
        if !self.note_if_refused() {
            self.selection = Selection::Piece(id);
        }
    }

    /// Place at…: wraps `id` round the form at `at`, at the height it has now. Needs a form.
    pub fn place_at(&mut self, id: PieceId, at: PlaceAt) {
        let Some(stage) = self.stage.clone() else {
            return;
        };
        let project = self.doc.project();
        let shapes = geom::shapes(project);
        let Some(shape) = shapes.iter().find(|s| s.id == id) else {
            return;
        };
        let placement = place::place_at(
            project,
            shape,
            at,
            &place::layout(&shapes),
            stage.shoulder_y(),
            &|angle, y| stage.surface_distance(angle, y),
        );
        self.set_placement(id, placement);
    }

    /// Flat: takes away the curve, leaving the piece where it is.
    pub fn flatten(&mut self, id: PieceId) {
        if let Some(p) = self.placement(id) {
            self.set_placement(id, Placement { curve: None, ..p });
        }
    }

    /// The Place at… menu for `id` (right-click on a piece, in 2D or 3D).
    pub fn place_menu(&mut self, ui: &mut egui::Ui, id: PieceId) {
        for (at, label) in [
            (PlaceAt::Front, tr!("place-front")),
            (PlaceAt::Back, tr!("place-back")),
            (PlaceAt::LeftSide, tr!("place-left")),
            (PlaceAt::RightSide, tr!("place-right")),
        ] {
            if ui.button(label).clicked() {
                self.place_at(id, at);
                ui.close();
            }
        }
        ui.separator();
        if ui.button(tr!("place-flat")).clicked() {
            self.flatten(id);
            ui.close();
        }
    }

    /// The "3D placement" group of a piece's properties: its position (in the student's units)
    /// and its rotation in degrees about x, then y, then z.
    pub(super) fn placement_properties(&mut self, ui: &mut egui::Ui, id: PieceId) {
        let Some(placement) = self.placement(id) else {
            return;
        };
        let units = self.doc.project().units;
        ui.add_space(6.0);
        ui.strong(tr!("panel-placement"));
        egui::Grid::new("placement_properties")
            .num_columns(3)
            .show(ui, |ui| {
                let labels = [
                    tr!("panel-position-x"),
                    tr!("panel-position-y"),
                    tr!("panel-position-z"),
                ];
                for (k, label) in labels.into_iter().enumerate() {
                    let shown = units.format_number(placement.position[k] * 1000.0);
                    let typed = self.field(ui, label, &shown, units.suffix());
                    self.apply_typed(typed, true, tr!("notice-bad-number"), |p, mm| {
                        let mut moved = placement;
                        moved.position[k] = mm / 1000.0;
                        p.set_placement(id, Some(moved))
                    });
                }
                let angles = place::euler_xyz_deg(placement.rotation);
                let labels = [
                    tr!("panel-rotation-x"),
                    tr!("panel-rotation-y"),
                    tr!("panel-rotation-z"),
                ];
                for (k, label) in labels.into_iter().enumerate() {
                    let typed = self.field(ui, label, &format!("{:.1}", angles[k]), "°");
                    self.apply_typed(typed, false, tr!("notice-bad-number"), |p, degrees| {
                        let mut turned = angles;
                        turned[k] = degrees;
                        let rotation = place::rotation_from_euler_xyz_deg(turned);
                        p.set_placement(
                            id,
                            Some(Placement {
                                rotation,
                                ..placement
                            }),
                        )
                    });
                }
            });
    }
}
```

`crates/app/src/editor/mod.rs`:
- add `mod placing;` after `mod panel;`, and `pub use placing::DEFAULT_SHOULDER_M;` after the `document` re-export;
- add `use crate::stage::Stage;` and `use std::sync::Arc;`;
- `PatternEditor` gains, after `notice`:

```rust
    /// The form pieces are placed round (Place at… needs it); None without a 3D view.
    pub stage: Option<Arc<Stage>>,
```

  with `stage: None,` in `new()`.
- `set_project` keeps it:

```rust
    pub fn set_project(&mut self, project: Project, path: Option<PathBuf>) {
        let (show_lengths, show_allowance) = (self.show_lengths, self.show_allowance);
        let stage = self.stage.take();
        *self = Self {
            show_lengths,
            show_allowance,
            stage,
            ..Self::new()
        };
        self.doc = Document::new(project, path);
    }
```

`crates/app/src/editor/panel.rs`: `field` and `apply_typed` become `pub(super) fn`. At the end of `piece_properties` (after the **Delete piece** button), add `self.placement_properties(ui, id);`.

`crates/app/src/editor/canvas.rs`:
- `CanvasState` gains, after `sew`:

```rust
    /// The shape the Place at… menu was opened on (right-click).
    pub menu_for: Option<PieceId>,
```

- in `canvas_ui`, right after the tool `match`, call `self.place_menu_on(&response, tol);`;
- add before `fn pan_and_zoom`:

```rust
    /// A right-click on a shape selects it and opens Place at… for it (when there is a form
    /// to place it round).
    fn place_menu_on(&mut self, response: &Response, tol: f64) {
        if self.stage.is_none() {
            return;
        }
        if response.secondary_clicked()
            && let Some(at) = response
                .interact_pointer_pos()
                .map(|p| self.view.to_world(self.canvas_rect, p))
        {
            self.canvas.menu_for = self
                .shapes()
                .iter()
                .rev()
                .find(|s| {
                    geom::contains(&s.piece, at)
                        || geom::nearest_edge(&s.piece, at).is_some_and(|e| e.2 <= tol)
                })
                .map(|s| s.id);
            if let Some(id) = self.canvas.menu_for {
                self.selection = Selection::Piece(id);
            }
        }
        if let Some(id) = self.canvas.menu_for {
            response.context_menu(|ui| self.place_menu(ui, id));
        }
    }
```

- [ ] **Step 4: Place at… in 3D** (`crates/app/src/app.rs`)

- `OpenDrapeApp` gains, after `view_camera`:

```rust
    /// The piece the 3D view's Place at… menu was opened on.
    menu_for: Option<PieceId>,
```

  with `menu_for: None,` in `new`.
- In `new`, before the comment that starts "Read before":

```rust
        // Place at… needs the form; there is none without a 3D view.
        let mut editor = PatternEditor::new();
        editor.stage = render_state.map(|_| stage.clone());
```

  and the struct literal's `editor: PatternEditor::new(),` becomes `editor,`.
- Replace `arrange` (it now borrows the editor once, and handles right-clicks):

```rust
    /// The pointer in the 3D view while arranging: clicks pick pieces, the selected piece's
    /// gizmo moves and turns it, and the gizmo is drawn over the view.
    fn arrange(
        &mut self,
        ui: &egui::Ui,
        response: &egui::Response,
        cam: &ScreenCamera,
        scene: &ArrangedScene,
    ) {
        let at = |p: egui::Pos2| glam::DVec2::new(f64::from(p.x), f64::from(p.y));
        let shift = ui.input(|i| i.modifiers.shift);
        let editor = &mut self.editor;
        if response.drag_started_by(egui::PointerButton::Primary)
            && let Some(p) = ui.input(|i| i.pointer.press_origin())
        {
            self.arranger
                .press(cam, scene, &editor.selection, &mut editor.doc, at(p));
        }
        if self.arranger.is_dragging()
            && let Some(p) = response.interact_pointer_pos()
        {
            self.arranger.drag_to(cam, &mut editor.doc, at(p), shift);
        }
        if response.drag_stopped() {
            self.arranger.release(&mut editor.doc);
        }
        if response.clicked()
            && let Some(p) = response.interact_pointer_pos()
        {
            self.arranger
                .click(cam, scene, &mut editor.selection, at(p));
        }
        // A right-click on a piece selects it and opens Place at… for it.
        if response.secondary_clicked()
            && let Some(p) = response.interact_pointer_pos()
        {
            self.arranger
                .click(cam, scene, &mut editor.selection, at(p));
            self.menu_for = editor.selection.piece();
        }
        if let Some(id) = self.menu_for {
            response.context_menu(|ui| editor.place_menu(ui, id));
        }
        if !self.arranger.is_dragging()
            && let Some(p) = response.hover_pos()
        {
            self.arranger.hover(cam, scene, &editor.selection, at(p));
        }
        // Drawn where the piece is now, after this frame's drag.
        let scene = self
            .arranged
            .scene(self.editor.doc.project(), self.stage.shoulder_y());
        let painter = ui.painter_at(response.rect);
        if let Some(gizmo) = Arranger::gizmo(cam, &scene, &self.editor.selection) {
            let lit = self.arranger.active().or(self.arranger.hovered);
            crate::arrange::overlay::paint(&painter, cam, &gizmo, lit);
        }
        if let (Some(text), Some(p)) = (
            self.arranger.readout(self.editor.doc.project().units),
            response.interact_pointer_pos(),
        ) {
            crate::arrange::overlay::paint_readout(&painter, p, text);
        }
    }
```

- [ ] **Step 5: Run and see them pass**

Run: `cargo nextest run -p opendrape`
Expected: the 6 placing tests and the 3D right-click test pass, plus all earlier ones.

- [ ] **Step 6: Commit**

```bash
cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings
git add crates/app
git commit -m "feat(app): Place at front, back and sides; Flat; typed 3D position and rotation

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 11: The drape gate: a skirt drafted the way a student would (review: full)

**Files:**
- Create: `crates/testkit/tests/project_skirt.rs`
- Modify: `crates/testkit/Cargo.toml` (dev-dependencies only)

Do **not** touch `crates/testkit/src/garments.rs` or `metrics.rs`: they belong to the dress-forms branch.

**Interfaces:**
- Consumes:
  - Tasks 1–4: `Project::{add_seam, add_twin, set_placement, all_seams, check}`, `opendrape_mesh::{build, MeshParams, Stitch}`, `place::{place_at, layout, effective, apply, PlaceAt}`;
  - the existing public testkit API: `garments::{DENSITY, body, collider}` and `metrics::{measure, position_hash}`;
  - sim: `ClothBuilder`, `Panel`, `Params`, `Solver`, `FRAME_DT`.
- Produces: the M4a drape gate, and nothing other code uses.

**What it drafts:**
- **The pieces:**
  - a front half on the fold (hem 300 mm, waist 177.5 mm, 550 mm long);
  - a "Back left" with its slanted side seam on its left and a straight centre back on its right;
  - its twin "Back right" to its right.
- **The seams:**
  - the side seam: the front's right edge running up from the hem, to the back's slanted edge running down from the waist (`forward: false`), so both start at the hem. Its mirror image sews the other side;
  - the centre back: the back to its twin. It is its own mirror image.
- **The placing:**
  - each piece is moved (as if typed) to put the waist at 1.03 m, M1's skirt waist;
  - then **Place at front** and **Place at back**, measuring the body from its centre line;
  - the body's centre line is at z = 0.020 (computed here the way `Stage` computes it), so the cloth is shifted by that.

**Gates:**
- Everything the spec asks: no NaN; penetration max ≤ 2 mm and p99 ≤ 1 mm; kinetic energy settles (≤ 1e-4 J after 6 s); a deterministic position hash.
- **The seams** (see Evidence, item 3):
  - just before they weld, every stitched pair is within 4 mm and the mean within 1 mm;
  - after welding, they are shut, as M1's gate.
- **Added for safety:**
  - no particle starts inside the body;
  - strain p99 ≤ 10%;
  - the skirt stays on the hips (lowest point > 0.4 m, highest > 1.0 m);
  - 4,000–30,000 particles.
- **The prototype measured:**
  - 5,670 live particles;
  - gaps of 2.72 mm max and 0.49 mm mean before the weld;
  - 0.00 mm penetration, strain p99 6.3%, kinetic energy 1.0e-7 J;
  - y from 0.47 to 1.03 m;
  - 4.9 s for 6 s simulated in the test profile on the user's Mac.

- [ ] **Step 1: Dev-dependencies**

Append to `crates/testkit/Cargo.toml`, before `[lints]`:

```toml
[dev-dependencies]
opendrape-core.workspace = true
opendrape-geom.workspace = true
opendrape-mesh.workspace = true
```

- [ ] **Step 2: The test** (`crates/testkit/tests/project_skirt.rs`)

```rust
//! M4a's drape gate: a skirt drafted the way a student would (a front cut on the fold, a
//! mirrored pair of back panels, the side seams and the centre back sewn, each piece moved to
//! waist height and Placed at the front or back of the form) is made into fabric by
//! `opendrape-mesh` and draped on the bundled body.

use glam::{DVec2, DVec3};
use opendrape_core::{Half, Piece, PieceId, Placement, Point2, Project, SeamSide};
use opendrape_geom as geom;
use opendrape_mesh::place::{self, PlaceAt};
use opendrape_mesh::{MeshParams, Stitch, build};
use opendrape_sim::{ClothBuilder, FRAME_DT, Panel, Params, Solver};
use opendrape_testkit::garments::{DENSITY, body, collider};
use opendrape_testkit::metrics::{measure, position_hash};

/// Where the skirt's waist goes, as for M1's demo skirt (m).
const WAIST_Y: f64 = 1.03;
/// The skirt's length (mm).
const LENGTH_MM: f64 = 550.0;

/// z of the torso's centre line between the hips, as the app's stage finds it (its x is 0).
fn centre_z() -> f64 {
    let (lo, hi) = body()
        .positions
        .iter()
        .filter(|p| p.y > 0.7 && p.y < 0.85 && p.x.abs() < 0.22)
        .fold((f32::MAX, f32::MIN), |(lo, hi), p| {
            (lo.min(p.z), hi.max(p.z))
        });
    f64::from((lo + hi) / 2.0)
}

/// The skirt: a front half on the fold (hem 300, waist 177.5, 550 long, the fold its left edge),
/// a "Back left" (side seam slanted on its left, centre back straight on its right) and its
/// mirror image to its right. Side seams: the front's right edge to the back's slanted edge,
/// both starting at the hem (the mirror image sews the other side). Centre back: the back to
/// its twin. Every piece is moved down to waist height, then Placed at front or back.
fn skirt() -> Project {
    let p = Point2::new;
    let mut pr = Project::new();
    let mut front = Piece::polygon(
        PieceId(0),
        "Front",
        &[p(0.0, 0.0), p(300.0, 0.0), p(177.5, 550.0), p(0.0, 550.0)],
    );
    front.fold = Some(3);
    let front = pr.add_piece(front);
    let back = pr.add_piece(Piece::polygon(
        PieceId(0),
        "Back left",
        &[
            p(400.0, 0.0),
            p(700.0, 0.0),
            p(700.0, 550.0),
            p(522.5, 550.0),
        ],
    ));
    let twin = pr
        .add_twin(back, "Back right".into(), p(1500.0, 0.0))
        .unwrap();
    let side =
        |shape, first_edge, forward| SeamSide::new(shape, Half::Drawn, first_edge, 1, forward);
    pr.add_seam(side(front, 1, true), side(back, 3, false));
    pr.add_seam(side(back, 1, true), side(twin, 1, true));
    assert_eq!(pr.check(), Ok(()));
    assert_eq!(
        pr.all_seams().len(),
        3,
        "the side seam's mirror image sews the other side"
    );
    // Typed in Properties: down to waist height (the middle of each piece).
    let middle = WAIST_Y - LENGTH_MM / 2000.0;
    for id in [front, back] {
        pr.set_placement(id, Some(Placement::at([0.0, middle, 0.4])));
    }
    // Place at…, measuring the form from its centre line.
    let zc = centre_z();
    let surface = |angle: f64, y: f64| {
        let dir = DVec3::new(angle.sin(), 0.0, angle.cos());
        collider().ray_exit(DVec3::new(0.0, y, zc), dir, 1.0)
    };
    for (id, at) in [(front, PlaceAt::Front), (back, PlaceAt::Back)] {
        let shapes = geom::shapes(&pr);
        let shape = shapes.iter().find(|s| s.id == id).unwrap();
        let placed = place::place_at(&pr, shape, at, &place::layout(&shapes), 1.3, &surface);
        pr.set_placement(id, Some(placed));
    }
    assert_eq!(pr.check(), Ok(()));
    pr
}

/// The skirt's cloth, its stitches as pairs of particle indices, and how many particles start
/// inside the body.
fn cloth() -> (Solver, Vec<(usize, usize)>, usize) {
    let pr = skirt();
    let mesh = build(&pr, &MeshParams::default());
    assert!(mesh.notes.is_empty(), "{:?}", mesh.notes);
    let shapes = geom::shapes(&pr);
    let layout = place::layout(&shapes);
    // The placements are in the form's frame; the bundled body's centre line is at z = zc.
    let shift = DVec3::new(0.0, 0.0, centre_z());
    let mut builder = ClothBuilder::new(DENSITY);
    let mut first = Vec::new();
    let mut count = 0;
    for panel in &mesh.panels {
        let shape = shapes.iter().find(|s| s.id == panel.shape).unwrap();
        let placement = place::effective(&pr, shape, &layout, 1.3);
        let positions = panel
            .flat
            .iter()
            .map(|f| {
                place::apply(
                    &placement,
                    panel.centre,
                    Point2::new(f[0] * 1000.0, f[1] * 1000.0),
                ) + shift
            })
            .collect();
        let id = builder.add_panel(
            &Panel {
                positions,
                flat: Some(panel.flat.iter().map(|f| DVec2::from_array(*f)).collect()),
                triangles: panel.triangles.clone(),
            },
            1.0,
        );
        first.push((id, count));
        count += panel.flat.len();
    }
    let pairs = mesh
        .stitches
        .iter()
        .map(|&((pa, a), (pb, b)): &Stitch| {
            builder.stitch((first[pa].0, a), (first[pb].0, b));
            (first[pa].1 + a as usize, first[pb].1 + b as usize)
        })
        .collect();
    let cloth = builder.build();
    let inside = cloth
        .positions()
        .iter()
        .filter(|p| collider().signed_distance(**p) < 0.0)
        .count();
    (Solver::new(cloth, Params::default()), pairs, inside)
}

/// The largest and mean distance (mm) between stitched particles.
fn seam_gaps(solver: &Solver, pairs: &[(usize, usize)]) -> (f64, f64) {
    let x = solver.cloth().positions();
    let d: Vec<f64> = pairs
        .iter()
        .map(|&(a, b)| (x[a] - x[b]).length() * 1000.0)
        .collect();
    (
        d.iter().copied().fold(0.0, f64::max),
        d.iter().sum::<f64>() / d.len() as f64,
    )
}

#[test]
fn a_drafted_skirt_drapes_on_the_form_without_poking_through_and_settles() {
    let (mut solver, pairs, inside) = cloth();
    let particles = solver.cloth().len();
    assert!(
        (4_000..30_000).contains(&particles),
        "{particles} particles"
    );
    assert_eq!(inside, 0, "Place at… leaves every piece clear of the form");
    let weld = Params::default().weld_time.unwrap();
    let frames = (6.0 / FRAME_DT).round() as usize;
    let mut before_weld = None;
    for _ in 0..frames {
        solver.step(Some(collider()));
        // The last frame before the seams weld: they have pulled shut by now.
        if before_weld.is_none() && solver.time() + FRAME_DT >= weld {
            before_weld = Some(seam_gaps(&solver, &pairs));
        }
    }
    let (gap_max, gap_mean) = before_weld.unwrap();
    eprintln!("seam gaps before welding: max {gap_max:.2} mm, mean {gap_mean:.2} mm");
    assert!(
        gap_max <= 4.0 && gap_mean <= 1.0,
        "seams didn't close: {gap_max:.2} / {gap_mean:.2} mm"
    );
    let r = measure(solver.cloth(), collider());
    eprintln!("{r:#?}");
    assert!(!r.has_nan);
    assert!(
        r.penetration_max_mm <= 2.0 && r.penetration_p99_mm <= 1.0,
        "poke-through"
    );
    assert!(
        !r.open_stitches && r.seam_gap_max_mm == 0.0,
        "seams welded shut"
    );
    assert!(
        r.strain_p99 <= 0.10,
        "fabric over-stretched: {}",
        r.strain_p99
    );
    assert!(
        r.kinetic_energy <= 1e-4,
        "still moving: {} J",
        r.kinetic_energy
    );
    assert!(
        r.lowest_y > 0.4 && r.highest_y > 1.0,
        "slid down: {}..{}",
        r.lowest_y,
        r.highest_y
    );
}

#[test]
fn the_drafted_skirt_drapes_the_same_every_time() {
    let hash = || {
        let (mut solver, _, _) = cloth();
        for _ in 0..90 {
            solver.step(Some(collider()));
        }
        position_hash(solver.cloth())
    };
    assert_eq!(hash(), hash());
}
```

- [ ] **Step 3: Run it**

Run: `cargo nextest run -p opendrape-testkit --test project_skirt --no-capture`
Expected: both tests pass. The first prints the gaps and the drape report, with numbers close to those above.

If a gate fails, don't loosen it. Debug with superpowers:systematic-debugging, starting from what the report shows:
- **Penetration:** check that Place at… used the band of heights. With a single ray, the panels wrap through the hips.
- **Seam gaps:** check the side seam's `forward: false`. A flipped seam measures about 11 mm.
- **`lowest_y` near 0:** the centre back isn't sewn, so the skirt fell to the floor.

- [ ] **Step 4: Commit**

```bash
cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings
git add crates/testkit/Cargo.toml crates/testkit/tests/project_skirt.rs Cargo.lock
git commit -m "test(testkit): a skirt drafted, sewn and placed like a student's drapes on the body

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 12: Checklist, docs, and the full local CI run (review: light)

**Files:**
- Create: `docs/testing/M4a-checklist.md`
- Modify:
  - `README.md`
  - `docs/specs/2026-10-09-opendrape-design.md` (Status only)
  - `docs/superpowers/specs/2026-10-09-m4a-sew-and-drape-design.md` (Status only)

Leave the main spec's milestone table alone: dress forms Track B renumbers it.

- [ ] **Step 1: Tester checklist** (`docs/testing/M4a-checklist.md`)

```markdown
# OpenDrape M4a: what to try

This build sews pattern pieces together and drapes them on the body. You draw a skirt, sew
it, place the pieces round the body in 3D, and press **Play**. Sleeves, pinning fabric in 3D
and the T-shirt come in M4b.

## Check these

- [ ] **Draft a skirt front on the fold.** With **Rectangle (S)**, draw a piece about 30 cm
      wide and 55 cm tall. With **Edit (Z)**, drag its top-right corner about 12 cm to the
      left, so the waist is narrower than the hem. Click its left edge, then **Set as fold
      line**.
- [ ] **Draft a back.** Draw another rectangle, 30 × 55 cm, to the right of the front. Drag
      its top-left corner about 12 cm to the right. Select it and click **Make mirrored
      pair**.
- [ ] **Sew the side seam.** Press **W** (Sew). Click the front's slanted right edge near the
      hem, then the back's slanted left edge near the hem. A coloured line with a number
      appears just inside both edges. The same seam appears by itself between the front's
      pale half and the mirrored back.
- [ ] **Sew the centre back.** Still in Sew, click the back's straight right edge near the
      hem, then the mirror's straight left edge near the hem. Without it, the skirt is open at
      the back and slides off the body.
- [ ] Click a sewn edge again in Sew: "This edge is already sewn."
- [ ] Click a seam's coloured line. **Properties** shows each side's length. Click **Flip**:
      the thin lines joining the seam's ends now cross. Click **Flip** again.
- [ ] In the 3D view, the pieces stand in front of the body. Right-click the front, then
      **Place at front**: it curves round the body. Right-click the back, then **Place at
      back**: the back and its mirror meet at the centre back.
- [ ] Click the front in 3D: it turns orange, and it is selected in the pattern window too.
      Drag the green arrow down until the top of the skirt is at the waist; the label says
      how far ("27.0 cm down"). Drag a ring to turn the piece; hold **Shift** for 15° steps.
      **Cmd+Z** undoes a whole drag.
- [ ] Do the same for the back (its mirror follows it).
- [ ] Press **Play**. The seams pull shut and the skirt settles on the body, with nothing
      poking through. "Press Reset to move pieces." shows while it drapes. **Front**,
      **Back**, **Left side** and **Right side** turn the view.
- [ ] Press **Reset**: the pieces are back where you placed them.
- [ ] In **Properties**, under **3D placement**, type **75** in **Position Y**: the piece
      moves to 75 cm up.
- [ ] **File → Save As…**, quit, reopen and **File → Open…**: the seams and the arrangement
      are back. **Cmd+Z** steps back through your gizmo moves.
- [ ] Your M2b files still open, with no seams yet.
- [ ] Draw a piece with the pen whose outline crosses itself (a figure of eight), and press
      **Play**: a note names it ("… couldn't be made into fabric: its outline crosses
      itself.") and the rest still drapes.

## Known limits in this build

- Cmd+H (hide OpenDrape) does not work in this build.
- Seams join whole edges. Sewing part of an edge (a sleeve cap) comes in M4b.
- Changing the pattern while it drapes returns to arranging. Live updates come in M4b.
- Pieces you never place hang in front of the body, and fall to the floor when you press
  **Play**.

If anything looks wrong, take a screenshot, then choose Help → About OpenDrape and
click **Copy diagnostics**.
```

- [ ] **Step 2: README and spec status**

README, the **Status** line:
> **Status:** early development (milestone M4a: sew and drape, part 1: sew pattern edges together, arrange the pieces round the body in 3D, and drape the skirt you drafted). Next: dress forms (M3), and M4b: sleeves, sewing part of an edge, pinning fabric and the T-shirt.

`docs/specs/2026-10-09-opendrape-design.md`, `## Status`: add after the M2b bullet (use the day the branch is finished if it is later than 2026-10-10):
> - **M4a Sew & drape, part 1: complete (2026-10-10).**
>   - **Sewing:** the Sew tool (W) joins whole edges. Mirrored seams are derived for cut-on-fold pieces and pairs. A seam panel shows the lengths, a warning over 3 mm, and Flip. Edits keep seams sewn.
>   - **Fabric:** spade's refined triangulation (12 mm, at most 30,000 particles).
>   - **Arranging:** a Stage around the body with a floor; in 3D, click-to-select, an in-house gizmo, Place at front/back/sides and typed placement.
>   - **Draping:** Play/Reset drapes the student's own pattern on the simulation thread, with notes for problems.
>   - **Files:** project format v3 (v1 and v2 files upgrade).
>   - **The drafted-skirt gate:** 0.00 mm penetration, seams welded, strain p99 6.3%.

`docs/superpowers/specs/2026-10-09-m4a-sew-and-drape-design.md`, the `**Status:**` line becomes:
> **Status:**
> - Approved in conversation, section by section, and as a written document.
> - Implemented on branch `m4a-sew-and-drape`. The plan's Evidence section records where the evidence refined the spec: the Place at… radius, the angle and seam-gap gates, the centre-back seam, and the arrow drag.

- [ ] **Step 3: Full local CI**

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo nextest run --workspace
cargo deny check
mkdir -p target/licenses
cargo about generate -m crates/app/Cargo.toml about.hbs -o target/licenses/THIRD_PARTY_LICENSES.html
grep -c spade target/licenses/THIRD_PARTY_LICENSES.html
```

Expected:
- all green;
- `cargo deny check` prints `advisories ok, bans ok, licenses ok, sources ok`;
- the last command prints at least 1, so the release packaging lists spade's licence.

The prototype ran 525 tests in about 10 s.

- [ ] **Step 4: Commit**

```bash
git add docs README.md
git commit -m "docs: M4a tester checklist and status

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

The controller does the final whole-branch review, the merge and any publishing afterwards. Ask the user before merging or pushing. **Never run the app on the user's Mac.**

---

## Self-review

**1. Spec coverage**

| Spec | Task |
|---|---|
| §1 Saved: `Project.seams`, `Piece/Twin.placement`; undo through `Document`; `check()` refuses invalid | 1, 2 |
| §1 Format: version bump, older files load without seams/placements, new frozen fixture, older fixtures stay | 2 |
| §1 The demo garments leave the app's 3D view and stay in testkit | 7 |
| §2 Model: `SeamId`, `Half`, `SeamSide`, `Seam`; how edges run; derived mirrors | 1 |
| §2 `check()` refusals, incl. more than 2,000 seams | 1 |
| §2 Edit table: add point, remove point (between / at an end / to 0), delete piece or twin, Unfold, Remove fold, Break pair | 1 |
| §2 Sew tool: W, two clicks, Shift-extend both sides, Esc, click away, which ends meet, pale half and twins, fold edge not sewable, "already sewn" | 5 |
| §2 Drawing: colours, number badge, selected thicker with guides | 6 |
| §2 Seam panel: lengths, amber difference over 3 mm, Flip, Delete; select by clicking the line with Edit or Sew | 6 |
| §3 `crates/mesh`: stitching outline of every shape, fold as one panel, twins separate, cut-outs as holes, markings ignored | 3 |
| §3 Edge sampling, equal counts per seam, ease; CDT with refinement (25°, area from h, keep constraint edges); per-panel data; stitches incl. mirrors; 30,000 limit and note; failures noted and left out | 3 |
| §3 API `build(project, &MeshParams) -> GarmentMesh { panels, stitches, notes }`; the app builds a `Cloth` with `add_panel`/`stitch`/`build` | 3, 7 |
| §4 Placement struct, frame, applying (centre, curve wrap, rotate, translate), validation | 2, 4 |
| §4 Starting position; twins mirror until placed themselves | 4 |
| §4 3D view: arranging/draping states, hint, picking by CPU ray, shared selection, clicking empty clears, Front/Back/Left/Right | 7, 8 |
| §4 Gizmo: arrows, centre square, rings, Shift 15°, highlight, readout, one undo step per drag | 4, 9 |
| §4 Typed placement group with the M2a text_field rules | 10 |
| §4 Place at front/back/left/right in 3D and 2D, height kept, radius +3 cm via `ray_exit`, folded centred, pairs meet at the centre line, Flat | 4, 10 |
| §5 Play (snapshot, mesh and cloth on the simulation thread, M1 params, auto-pause), Reset, edits while draped reset | 7 |
| §5 Stage boundary (render mesh, collider, centre line, floor height, surface distance) | 7 |
| §5 Floor at y = 0 behind the Stage | 7 |
| §5 Problems: lengths differ (panel and note), unmeshable named, starts inside the form, coarser fabric, non-finite reset | 3, 6, 7 |
| §5 Performance: ≥ 20 fps for a drafted skirt | 11 (13.6 ms/frame measured for 5,824 particles) |
| Testing: core/io, mesh, maths, kittest flows, testkit drape | 1–11 |
| Constraints: never launch the app, spade only, no "CLO", Fluent, dress-forms files untouched | Global Constraints; every task |

**2. Placeholders:** none. Every code step carries the code, copied from the prototype where it compiled and passed.

**3. Type consistency:** every task's Interfaces block was written from the prototype, where all tasks compiled and ran together (525 tests, clippy `-D warnings`). These shapes are kept in step:
- Task 8's `view_3d` and its click-only `arrange`, plus Task 9's two edits, equal the prototype's Task 9 file exactly (checked by diff).
- The two Review Focus tests added to Tasks 5 and 7 were run at those tasks' states.

**4. Review Focus:** five items, each with its test in the owning task (see the list at the top).
