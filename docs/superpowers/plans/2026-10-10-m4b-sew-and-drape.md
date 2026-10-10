# OpenDrape M4b (Sew & Drape, part 2) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** a student drafts a T-shirt and sews it: the shoulders, sides and underarms whole-edge with **W**, and each sleeve cap into its armhole with **F** in two free seams that meet at the cap notch. They place the pieces (**Place at → Left arm** for a sleeve, whose twin goes on the right arm), press **Play**, and while it drapes they lengthen the sleeves, pull the hem and pin it. Free seams and pins are saved in project format 4.

**Architecture:** "store once, derive the rest", as in M2b and M4a.
- **`opendrape-core`** stores what the student did:
  - a seam side runs between two points of an outline: `SeamSide { shape, half, from: OutlinePos, to: OutlinePos, forward }`, where `OutlinePos { edge, t }` is a fraction of a stored edge's arc length. A whole-edge side is `SeamSide::edges(first, last)`.
  - `Project.pins: Vec<Pin>`, each a spot of a stored piece held at a 3D target.

  Edits re-express side ends on the edges they now lie on. `Project::drop_broken` deletes a seam with a side 1 mm or shorter, and a pin left off its piece; `Document` runs it on every edit. `check()` measures side lengths with core's own polyline lengths (`measure.rs`).
- **`opendrape-geom`** maps a free side onto the shape that shows it (`side_runs`: stretches of outline edges). It also maps a point of an outline to a stored position and back, finds the notches inside a side, and maps a pin's stored spot to where its shape shows it.
- **`opendrape-io`** rewrites version 3's whole-edge sides as free sides before parsing (on a `serde_json::Value`).
- **`opendrape-mesh`**:
  - samples free sides across corners, leaves the rest of each edge at about `h`, and gives two sides that meet part-way along an edge one shared point;
  - lays out a seam's steps stretch by stretch between notches paired in order, so paired notches land on the same stitch (with an even layout when the counts differ);
  - in `place`, defines `Arm` and Place at → arm.
- **`opendrape-sim`** gains one file (`attach.rs`) and ten lines: a barycentric point of a triangle pulled to a target, named by its triangle, so welding never loses it.
- **`opendrape-drape`**:
  - finds each arm from cross-sections of the body;
  - `Drape` holds the cloth, a `Fabric` map from cloth triangles back to the pattern, and the pins;
  - `Drape::rebuilt` warm-starts a new fabric from the old one.
- **The app**:
  - the Free Sew tool (F; Fit moves to Cmd+0) and the notch warning in the seam panel;
  - a runner that carries the drape on after every edit (coalesced, off the UI thread) and pulls grabbed points;
  - a `Draper` for grab and pin in 3D, with pin markers in 2D and 3D, Pin here and Remove pin;
  - Place at → Left arm / Right arm, with placements greyed out while draping.

**Tech Stack:** as M4a: Rust 1.99, eframe/egui/egui_kittest 0.36.2, wgpu 30.0.1, winit 0.30.13, kurbo 0.13.1, i_overlay 9.0.1, spade 2.15.1, glam 0.33 and serde_json. **No new dependencies:** `Cargo.lock` does not change. `Cargo.toml` gains only a dev-profile `opt-level` for `opendrape-drape`.

**Spec:** `docs/superpowers/specs/2026-10-10-m4b-sew-and-drape-design.md` (approved by the user on 2026-10-10). It builds on `docs/superpowers/specs/2026-10-09-m4a-sew-and-drape-design.md`, which still holds unless M4b changes it. Main spec: `docs/specs/2026-10-09-opendrape-design.md`. Parallel work: `docs/superpowers/specs/2026-10-09-dress-forms-design.md` (branch `dress-forms`).

**Evidence:**
- **The probe, `scratchpad/m4b-probe`:** a detached `git worktree` of `main` at `d62b0a6` (removed once this plan was written), with every task below implemented in order and checked after each task:
  - `cargo fmt`, `cargo clippy --workspace --all-targets -- -D warnings` and `cargo nextest run --workspace` all passed;
  - at the end: 694 tests (616 before), `cargo deny check` (advisories, bans, licenses and sources ok) and `cargo about generate`.

  The code in this plan is copied from it by a script, not retyped.
- **Free sides and notches (Task 3):**
  - Sides run across corners and wrap past the last edge. A corner inside a side takes over its nearest sample, as in M4a.
  - The stretches of an edge no side covers get points 0.5–1.6 h apart.
  - Two sides that meet at a cap notch share one point, stitched to both armholes.
  - Notches 100/250 mm along a 300 mm side and 50/150 mm along a 200 mm side land on stitches 8 and 21 of 26 on both sides. With 2 against 1 the layout is even.
  - A fuzz of 120 patterns with random free seams and notches never panics.
  - Every M4a fabric test passes unchanged in meaning.
- **The body's arms (Task 4):** cut across every centimetre below the shoulders, each arm is a loop of its own from the armpit (y ≈ 1.19 m) down.
  - The fitted line runs 42.0° from straight down, from the shoulder point (±0.113, 1.304, −0.004) m.
  - It hangs free of the body 0.148 m down the line; its length is 0.565 m.
  - The two arms are exact mirror images (1e-9).
  - Where the arm hangs free, rays from the line find its surface 3.0–5.5 cm away all round.
- **Place at → arm on a cylinder stand-in (Task 4):** every point of the sleeve lies exactly at the curve radius from the arm line (1e-9), with its top at the shoulder and its pattern-left half towards the front. Its twin, mirrored, lies round the other arm. Moved 5 cm and turned 10°, it is still round the moved and turned line. It takes 3.5 ms for a sleeve and 85 ms for a 2 m square (dev profile).
- **The attachment API (Task 5):**
  - A pinned barycentric point stays within 1 mm of its target while the rest falls.
  - A grab spring (1e-4 m/N) follows its target within 1 cm in 1 s.
  - Detaching lets go.
  - A point on a seam stays held after the seam welds.
  - Existing sim files gain 10 lines; `weld_stitches` is untouched.
- **The warm start (Task 6):**
  - A pure re-mesh puts every triangle corner back exactly (worst 0.000000 mm; the gate is 1 mm). Its closed seams then weld at once, and it starts from rest with gravity on.
  - A hem made 60 mm longer carries on 60 mm below the old hem, within 30%.
  - A T-shirt rebuild (7,934 particles before welding) takes 48 ms with `opendrape-drape` at opt-level 2 in the dev profile, against 382 ms unoptimised; meshing alone takes 7.8 ms.
  - Six edits sent in a row are made at most twice, ending with the newest.
- **The T-shirt gate (Task 11):**
  - 7,407 live particles, and no notes (each cap half is 1.2 mm longer than its 206.7 mm armhole);
  - before welding, the seams are at most 2.86 mm apart (mean 0.42 mm), then welded shut;
  - 0.00 mm penetration (max and p99), strain p99 4.5%, kinetic energy 2.2e-8 J after 6 s;
  - the cap notch is 0.00 mm from the shoulder seam's end, and every sleeve point is within 7.1 cm of its arm's line;
  - y runs from 0.77 to 1.36 m, and the run is deterministic.

  It took 12–18 s on a heavily loaded machine; the M4a skirt gate took 13 s under the same load.

**Where the spec's assumptions met the evidence (decided here; listed for the user):**
1. **No `Placement.axis`.** M4a's placement already wraps a piece round an axis set by its own position and turn: the piece's middle is `radius` out from it along the turned z, parallel to the turned y. Place at → arm turns the piece so that axis is the arm's line.
   - The probe shows every sleeve point exactly on that cylinder. A gizmo move or turn carries the axis with the piece, so a sleeve is never forced round the body's centre line.
   - A stored axis would say the same thing a second time, and drift from the placement as soon as the gizmo moved the piece. Format v4 has no `axis`.
2. **A sleeve is centred on the arm's outer side, not its front.** Centred on the front, its underarm edges meet behind the arm, and its front underarm corner starts in the chest.
   - The sleeve had to go 10 cm down the arm to start clear. At 3 cm down, 76 points started inside and a seam was 5.49 mm open before welding (the gate is 4).
   - Centred on the outer side, the cap's middle (the notch) faces the shoulder seam and the underarm edges face the body. It starts clear 3 cm down, and every gate passes.
3. **Place at → arm measures the arm only where it hangs free (below the armpit).** "The largest surface distance over the span the sleeve covers" also counts rays from near the shoulder that run on into the torso. That gave a 10.8 cm curve.
   - Across that gap the sleeve's underarm seam closes through the arm: it was still 86 mm open when it welded, and strain reached 613%.
   - Radii from 5.5 to 8 cm pass, and 9 cm fails. The free arm gives 7.4 cm.
   - `Arm` gains `free`, how far down its line the arm hangs free.
4. **The sleeve starts a little down the arm.** At the shoulder, 8 points of the cap's front underarm corner start up to 13 mm inside the chest, and the student would see "Sleeve starts inside the form".
   - Place at → arm moves the piece down in 1 cm steps until no point of it is inside the form, at most 20 cm (3 cm for the T-shirt).
   - The cap seams pull it up into the armhole: the notch lands on the shoulder seam's end.
5. **The arm lines come from cross-sections of the mesh, not from groups of its points.** The body is low-poly: in 1 cm horizontal bands its points leave gaps over 2 cm up to the shoulder, so "the outermost group" found an arm everywhere. The fitted line then tilted 13° and left the arm 20 cm down.
   - The plane cuts the mesh into closed loops instead (every edge is shared by exactly two triangles).
6. **Fit moves from F to Cmd+0 (Ctrl+0).** F has been Fit since M2a, and the spec gives F to Free Sew. The toolbar shows the new shortcut, and the one M2a test that pressed F now presses Cmd+0.
7. **"Already-closed seams stay closed: stitches whose two ends start together are welded at once."** The sim may not change, and it can only weld all stitches at once.
   - When every stitch of the new cloth starts within 1 mm, it welds at once (`weld_time = 0`).
   - When the edit added a seam that starts open, every stitch welds at the usual 0.8 s, and the closed ones are held shut by their stitches until then.
   - A remade drape starts with full gravity (no 0.6 s delay).
8. **Seam sides 1 mm or shorter, and pins off their piece, go with the edit that did it.** The spec says this for seams. Refusing the edit instead would stop a student shortening a sleeve past a pin, so pins more than 1 mm outside their piece go the same way.
   - `Document` runs `drop_broken` on every edit and drag, so it happens in one place and in the same undo step.
9. **A side ending at a notch doesn't count it.** Notches within 1 mm of a side's ends mark where it meets another seam, not a point inside it. Without this the two cap seams, split at the notch, would each count it and the panel would warn.
10. **Removing a point where a side ends no longer shortens the side by an edge.** The end keeps its place on the joined edge, as the spec's edit rules say. Two M4a tests that asserted the old rule are replaced.

**Spec points this plan interprets (where the spec is silent or loose):**
- **Free Sew:**
  - The second side may be on the same piece as the first (a sleeve's underarm is); the spec says "the other piece".
  - "The M4b fold notice" is M4a's existing "The fold line is inside the piece and can't be sewn." (`notice-sew-fold`).
  - Two refusals the spec doesn't list get notices of their own: an end on another piece or half, and a side 1 mm or shorter (including the same spot clicked twice).
  - "The first side's pieces are highlighted" is the piece outlined in the selection colour.
- **The notch check:**
  - Notches within 1 mm of a side's ends don't count (item 9 above).
  - The mismatch is shown in the seam panel only, as the spec says; the drape gives no note.
- **The 1 mm rule:** `check()` measures with core's own polylines (64 steps a curve), so validity never depends on `opendrape-geom`. On a curve the two lengths can differ by a few hundredths of a millimetre; core's decides.
- **Upgrading v3:** the upgrade rewrites the JSON document before parsing. A side naming edges its piece lacks becomes one that `check()` refuses, as v3's check did.
- **Place at → arm on a pair:**
  - Placing a piece clears its twin's own placement, so the twin shows the mirror image on the other arm.
  - Placing a twin gives its piece the mirror image.
- **Placements while draping:** the spec disables the typed fields. Place at… and Flat are greyed out too, since none of them would move the cloth.
- **Pins:**
  - Pins have no ids: a pin is selected by its index in `Project.pins`.
  - A pin on a twin is kept where its piece shows that spot (the spec's "stored-piece pattern mm").
  - Dragging a piece on the pattern table moves its pins with it, so they stay on the same spot of fabric.
  - "Out of range" for a target is more than 10 m from the origin (M4a's placement limit). A pin dragged further stops where it last could.
  - A pin whose piece isn't in the fabric (it can't be meshed) is skipped while draping and kept in the project.
- **In 3D while draping:**
  - A press on the fabric grabs it, and a drag that starts off the fabric turns the camera.
  - A pin's marker is dragged in the plane facing the viewer, as a grab is.
  - The grab is a 1e-4 m/N spring. The drape never pauses itself while a point is held.
  - A grab or an edit plays a paused drape on.
- **The warm start:**
  - "Offset along that point's surface frame by the pattern distance" is done by continuing the nearest old triangle flat (barycentric extrapolation). That triangle is the surface frame, with its stretch included.
  - A piece the old fabric didn't have starts at its placement.
  - A change to pins or placements only keeps the same cloth and moves its pins.
  - A command found while skipping to the newest edit is handled next, not dropped.
- **The gate:** "every sleeve particle" means every live one (welding retires one particle of each stitched pair), checked after 6 s with the drape settled (kinetic energy ≤ 1e-4 J, as M4a's gate).
- **"No new dependencies":** `Cargo.lock` doesn't change. A dev-profile `opt-level` for `opendrape-drape` is a build setting, added because the warm start runs on every edit.

---

## Global Constraints

Carried from M0–M4a:
- Pinned toolchain `1.99.0`; the egui family is `=0.36.2`; GPL-3.0-or-later. These pass: `cargo deny check`, and `mkdir -p target/licenses && cargo about generate -m crates/app/Cargo.toml about.hbs -o target/licenses/THIRD_PARTY_LICENSES.html`.
- Never use the name "CLO" in the product or the code.
- Every user-visible string goes through `tr!` (`crates/app/i18n/en-US/opendrape.ftl`). The `fl!` macro checks message ids when it compiles, so add a task's strings before its code.
- **Never launch the GUI app, `--smoke-test`, or anything that opens a window or dialog on the user's Mac.** Use only headless unit tests and egui_kittest tests. App-level kittests use `Harness::builder().wgpu()`, as `crates/app/tests/ui.rs` does: an offscreen device, no window.
- Commit messages end with exactly `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`.
- Work on branch `m4b-sew-and-drape`, created from `main`. Never push.
- Pattern coordinates are f64 millimetres, y up. Outline edge `i` runs from vertex `i` to vertex `(i+1) % n`. 3D is the form's frame: metres, y up from the floor, the form faces +z and its left is +x.
- Every user change is one undo step through `Document`, which refuses any change that makes `Project::check()` fail. Call sites show `notice-refused` via `note_if_refused()`.
- The keyboard rules from M2a are unchanged: shortcuts and tool keys act only when no text field has focus, the number box is closed and no modal is open, and redo is checked before undo.
- **Determinism:** no `HashMap`/`HashSet` iteration may decide an order the fabric, the cloth or a test depends on. Use `BTreeMap`/`BTreeSet`, or sort.

New in M4b (from the spec):
- **Dependencies:** none new. `crates/sim` gets only the additive attachment API: a new `attach.rs`, one field on `Cloth`, `Solver::cloth_mut`, and one solver line. Nothing existing in `crates/sim` changes behaviour.
- **Do not touch the dress-forms branch's files:**
  - `crates/body/**`;
  - `crates/testkit/src/garments.rs`, `crates/testkit/src/metrics.rs`;
  - `scripts/forms/**`, `assets/forms/**`, `ASSETS.md`.

  The T-shirt gate is a new file, `crates/testkit/tests/project_tshirt.rs`.
- **Schema version:** `SCHEMA_VERSION` goes from 3 to 4, with a v3→v4 upgrade (whole-edge sides become free sides from the first edge's start to the last edge's end) and a frozen `crates/io/tests/fixtures/v4/project.json`. The v1, v2 and v3 fixtures stay and keep loading. **If dress-forms Track B merges first and takes 4, renumber M4b's version, upgrade and fixture folder to 5 at merge time.**
- **Seam sides:**
  - **Model:**
    - `SeamSide { shape, half, from: OutlinePos { edge, t }, to: OutlinePos, forward }`, where `t` is an arc-length fraction of a stored edge.
    - A side covers the outline from `from` to `to`, walking the stored way when `forward` (it may pass corners and wrap). `SeamSide::edges(shape, half, first, last, forward)` is whole edges `first..=last`.
  - **`check()` keeps every M4a rule and adds these:**
    - each `t` is finite and within 0..=1;
    - each side is longer than 1 mm (`MIN_SIDE_MM`, measured by `opendrape_core`'s own 64-step polylines);
    - no two sides share more than a point of outline, mirror images included.
  - **Edits:**
    - adding a point re-expresses ends on the split edge on the right part (`Project::seams_after_split(id, i, s)`, `s` the split's arc-length fraction);
    - removing a point maps ends on the two joined edges onto the joined edge by their share of its length (`seams_after_removal(id, i, n, f)`);
    - moving points or handles leaves each `t` alone;
    - a side left 1 mm or shorter deletes its seam (`Project::drop_broken`, run by `Document` on every edit);
    - the M4a rules for Delete, Unfold, Remove fold and Break pair still hold.
- **Whole-edge sewing (W) keeps working:** it makes free sides that start and end at corners, through a private `EdgeRun`.
- **Free Sew tool:**
  - **Key F.** Fit moves to Cmd+0 / Ctrl+0 (`editor::FIT`).
  - **Clicks:** click a side's start, then its end, on the same shape and half. The side runs the shorter way round, or the longer with Shift. Then the second side's start and end make the seam (one undo step).
  - **Snapping:** a point snaps to the corners, the middle or a notch of the edge under the pointer, within 8 screen points.
  - **Refusal notices:**
    - "Part of this is already sewn.";
    - the fold notice ("The fold line is inside the piece and can't be sewn.");
    - "End the side on the same piece (and the same half) it started on.";
    - "That side is too short to sew: pick points more than 1 mm apart.";
    - the M4a mirror notice.
- **Notches:** a side's notches are those more than 1 mm from both of its ends. The k-th of each side pair up; with different counts the layout is even, and the seam panel says "Notches don't match: 2 on one side, 1 on the other."
- **Arms:**
  - `Stage::arms() -> &[Arm; 2]` (left +x first) and `Stage::arm_surface_distance(arm, along, angle)`, with rays up to 0.15 m.
  - `Arm { shoulder, direction, length, free }`. `Arm::around(angle)`: 0 is the arm's front, π/2 its outer side, the same on both arms.
  - **Place at → Left arm / Right arm:** the sleeve centred on its arm's outer side, curved 3 cm clear of the free arm, and moved down the arm (1 cm steps, at most 20 cm) until no point is inside the form. Its pair partner goes on the other arm.
- **Pins:**
  - `Pin { shape, half, at: Point2 (stored-piece mm), target: [f64; 3] (m) }`, at most 500 (`MAX_PINS`).
  - **`check()` refuses:** a missing shape or half, an `at` more than 1 mm outside its stored piece (`PIN_SLACK_MM`), and a non-finite target or one more than 10 m out.
  - Pins move with their piece when it is dragged on the pattern table.
  - Break pair, Unfold, Remove fold and Delete keep them on their spot or remove them.
  - A pin holds exactly (compliance 0); a grab is a 1e-4 m/N spring to a target in the plane facing the viewer.
- **Live updates:**
  - Any project change while draping is sent to the runner (`SimRunner::update`), which plays on.
  - A change only to pins or placements moves the pins on the same cloth. Anything else makes the fabric again on the simulation thread, warm-started, and the last frame keeps showing until then.
  - Edits waiting in the channel are made once, the newest.
  - Placements don't apply while draping: the typed placement fields and Place at… are greyed out, with "Placements apply after Reset."
- **Grab and pin act only while draping.** While arranging, the gizmo owns the 3D view.

## Review Focus

These are the inputs most likely to bite a student that the spec's own tests don't exercise, most likely first. Each one is pinned by a test in the task that owns the code.
1. **Clicking the same spot for both ends of a side with Free Sew** (a corner, a notch, or a double-click). Expected: refused as "too short", never sewn as the whole outline round to the same point. Tests:
   - Task 7: `side_between` unit test `a_side_runs_the_shorter_way_round_unless_shift_asks_for_the_longer` (start and end the same point);
   - Task 7: kittest `the_same_spot_clicked_for_both_ends_is_refused_not_sewn_all_the_way_round`.
2. **An edit that shortens a piece past a pin, or past a free side's end.** Expected: the edit is made, and the pin or seam goes in the same undo step (never a refused edit, never a pin floating off its piece). Tests:
   - Task 1: `document::tests::a_drag_that_leaves_a_seam_side_too_short_deletes_the_seam_in_the_same_step`, and core's `an_edit_that_leaves_a_side_1_mm_long_or_less_deletes_its_seam`;
   - Task 2: core's `pins_stay_on_their_spot_of_fabric_through_edits` (the narrowed pocket).
3. **Dragging a point in 2D while the drape runs** (dozens of edits a second). Expected: the drape carries on with the newest pattern, never resets, and the fabric is not remade once per edit. Tests:
   - Task 6: `edits_that_arrive_while_the_fabric_is_made_again_are_made_once_the_newest`, `editing_the_pattern_while_draped_carries_the_drape_on`.
4. **Pinning or grabbing fabric on a seam after it has welded** (a pin at the shoulder seam, say). Expected: the spot stays held; welding never makes a pin hold a dead particle. Tests:
   - Task 5: `a_point_on_a_seam_stays_held_after_the_seam_welds`;
   - Task 6: `a_pin_on_a_piece_left_out_of_the_fabric_is_let_go` (a pin whose piece is not in the fabric is skipped, not a panic).
5. **Place at → arm on a piece that isn't a sleeve** (a whole front, or a huge piece chosen by mistake). Expected: a valid placement at once, never a frozen window. Tests:
   - Task 4: `place_at_arm_on_a_huge_piece_checks_a_bounded_number_of_points`, `with_no_arm_in_reach_place_at_arm_uses_a_fallback_curve`.

---

## File Structure

```
Cargo.toml                                    dev-profile opt-level 2 for opendrape-drape (Task 6)
crates/core/src/seam.rs                       OutlinePos, Span, SeamSide (free), MIN_SIDE_MM, overlap
crates/core/src/measure.rs                    new: edge lengths and distance outside a piece, for check()
crates/core/src/pin.rs                        new: Pin, MAX_PINS, PIN_SLACK_MM
crates/core/src/project.rs                    free-side checks and edits, drop_broken, pins, SCHEMA_VERSION 4
crates/core/src/lib.rs                        exports
crates/geom/src/seams.rs                      side_runs (Run), outline_pos/point_at, edge_points_between, side_notches
crates/geom/src/lib.rs                        split_edge_in / remove_vertex_in pass fractions; exports
crates/geom/src/shapes.rs                     Shape::pin_spot / spot_shown
crates/io/src/lib.rs                          v3 → v4: whole-edge sides become free sides
crates/io/tests/{fixtures.rs,roundtrip.rs}    v4 fixture test; free sides and pins round-trip
crates/io/tests/fixtures/v4/project.json      new frozen fixture (+ README line)
crates/mesh/src/boundary.rs                   outlines with free sides, gaps sampled, shared points
crates/mesh/src/lib.rs                        SidePlan, notch-matched layout, samples()
crates/mesh/src/place.rs                      Arm, place_at_arm, along_outline
crates/mesh/tests/fabric.rs                   free sides, notches, cap notch shared, fuzz
crates/sim/src/attach.rs                      new: AttachmentId, Cloth::{attach, move_attachment, detach, attached_point}
crates/sim/src/{cloth.rs,lib.rs,solver.rs}    one field, the export, Solver::cloth_mut and one solve call
crates/drape/src/stage.rs                     arms from cross-sections, arm_surface_distance
crates/drape/src/build.rs                     Drape (fabric, pins, rebuilt), Fabric, FabricPanel; build_drape kept
crates/drape/src/live.rs                      new: warm start, warm params, flat triangle index
crates/drape/src/lib.rs                       exports
crates/app/src/sim_runner.rs                  Update (coalesced), Grab/Pull/Release, frames carry their Fabric
crates/app/src/app.rs                         live updates, drape input, pin markers, editor.draping
crates/app/src/draping.rs                     new: Draper (grab, pin, move and remove pins in 3D), pick_fabric
crates/app/src/arrange/overlay.rs             paint_pins
crates/app/src/editor/free_sew.rs             new: the Free Sew tool (F)
crates/app/src/editor/pins.rs                 new: pins on the pattern table, the pin panel
crates/app/src/editor/{mod.rs,canvas.rs,sew_tool.rs,panel.rs,paint.rs,placing.rs,seams.rs,document.rs}
crates/app/src/lib.rs                         pub mod draping
crates/app/i18n/en-US/opendrape.ftl           strings
crates/app/tests/{free_sew.rs,draping.rs,pins.rs}   new kittest files
crates/app/tests/{sewing.rs,ui.rs,editor.rs,placing.rs}   updated / added tests
crates/testkit/tests/project_tshirt.rs        new: the T-shirt drape gate
docs/testing/M4b-checklist.md, README.md, docs/specs/2026-10-09-opendrape-design.md, the M4b spec   docs
```

All commands assume `source ~/.cargo/env` and the repo root. Before Task 1: `git checkout -b m4b-sew-and-drape`.

**M4a tests the model change forces to change** (each listed again in its task):
- **Task 1:**
  - **core:**
    - `seam.rs`'s two tests;
    - in `project.rs`, the `side()` test helper, which now takes the first and last edge (a call naming one edge reads the same);
    - every multi-edge `side(…)` call;
    - `seams_are_checked` ("no edges" and "more than the outline" are not expressible: they become "ends where it starts" and "ends past the last edge");
    - the split and removal tests, which pass the new fraction arguments, and the two "removing points shrinks sides" tests, replaced because a removed point no longer shortens a side by an edge;
    - the twisted centre back in `mirrors_are_derived_for_folds_and_pairs`, which uses `flipped()`;
    - `SCHEMA_VERSION == 4`.
  - **geom:** `sides_run_from_their_start`, the two whole-outline loops, and `edits_in_a_project_keep_its_seams` (`side_edges` became `side_runs`).
  - **io:**
    - `format_v1/v2_still_opens` (version 4);
    - `format_v3_still_opens` (the upgraded sides);
    - `roundtrip.rs` (`SeamSide::edges`, the "wraps" check).
  - **mesh:** `fabric.rs`'s helper and multi-edge calls, `multi_edge_and_wrapping_sides_pair_start_to_start`, and `random_patterns_never_panic` (same random sequence).
  - **app:**
    - `sew_tool.rs` unit tests (`EdgeRun`);
    - `tests/sewing.rs` (helper, multi-edge calls, `.first_edge` → `.from.edge`);
    - `tests/ui.rs::add_sewn_pieces`;
    - `sim_runner.rs` tests.
  - **drape and testkit:** `drape/build.rs` tests and `testkit/tests/project_skirt.rs` (`SeamSide::new` → `SeamSide::edges`).
- **Task 6:**
  - `tests/ui.rs::editing_the_pattern_while_draped_returns_to_arranging` is replaced by `editing_the_pattern_while_draped_carries_the_drape_on`: the spec makes edits carry the drape on;
  - three `sim_runner.rs` tests build the runner's `Drape` from an `opendrape_drape::Drape`.
- **Task 7:** `tests/editor.rs::fit_shows_every_piece` presses Cmd+0, not F.

---

### Task 1: Free seams in the model, the edits that keep them, and format 4 (review: full)

**Files:**
- Create: `crates/core/src/measure.rs`
- Modify:
  - `crates/core/src/{seam.rs,project.rs,lib.rs}`
  - `crates/geom/src/{seams.rs,lib.rs}`
  - `crates/io/src/lib.rs`, `crates/io/tests/{fixtures.rs,roundtrip.rs}`
  - `crates/mesh/src/boundary.rs` (a one-line bridge until Task 3), `crates/mesh/tests/fabric.rs`
  - `crates/drape/src/build.rs` (a test helper)
  - `crates/app/src/editor/{sew_tool.rs,seams.rs,panel.rs,document.rs}`, `crates/app/src/sim_runner.rs` (a test helper)
  - `crates/app/tests/{sewing.rs,ui.rs}`
  - `crates/testkit/tests/project_skirt.rs`

**Interfaces:**
- Consumes: M4a's `Project::{owner, piece, piece_mut, add_seam, seam, seam_mut, all_seams, mirror_side, mirror_of, unfold_piece}`, `Piece::{edge_ends, prev, len}`, `geom::{Shape, ShapeKind, edge_length, edge_points, distance_along, point_at_distance, flatten, edge_seg, ACCURACY}`.
- Produces (core, exported from `opendrape_core`):
  - `pub const MIN_SIDE_MM: f64 = 1.0;`
  - `OutlinePos { pub edge: usize, pub t: f64 }`: Copy, PartialEq, serde; `pub const fn new(edge, t)`.
  - `Span { pub edge: usize, pub t0: f64, pub t1: f64 }`: Copy, PartialEq (`t0 < t1`).
  - `SeamSide { shape: PieceId, half: Half, from: OutlinePos, to: OutlinePos, forward: bool }`: Copy, PartialEq, serde (no longer `Eq`/`Hash`). Methods:
    - `edges(shape, half, first, last, forward) -> Self` (whole edges `first..=last`, wrapping);
    - `tidy(&self, n) -> Self` (an end at a corner named on the edge the side covers there);
    - `flipped(&self) -> Self`, `spans(&self, n) -> Vec<Span>`, `covers(&self, n, e) -> bool`;
    - `same_part(&self, &SeamSide) -> bool`, `overlaps(&self, &SeamSide, n) -> bool`.
  - Removed: `SeamSide::new`, `stored_edges`, `same_edges` and the fields `first_edge`/`edges`.
  - `Project::side_length(&SeamSide) -> Option<f64>` (core's polyline lengths) and `Project::drop_broken(&mut self)`.
  - Changed: `seams_after_split(id, i, s: f64)` and `seams_after_removal(id, i, n, f: f64)`; `SCHEMA_VERSION = 4`.
- Produces (geom, exported from `opendrape_geom`):
  - `Shape::outline_edge(&self, Half, e) -> Option<(usize, bool)>`, `Shape::outline_pos(&self, j, d) -> (Half, OutlinePos)`, `Shape::point_at(&self, Half, OutlinePos) -> Option<Point2>`;
  - `Run { pub edge: usize, pub from: f64, pub to: f64 }` with `length()`, and `side_runs(&Shape, &SeamSide) -> Option<Vec<Run>>` (replaces `side_edges`);
  - `edge_points_between(&Piece, j, d0, d1, tolerance) -> Vec<Point2>`;
  - `side_notches(&Shape, &SeamSide) -> Option<Vec<f64>>`;
  - `side_length`/`side_points` keep their signatures.
- Produces (app): `editor::sew_tool::EdgeRun` (pub(super): `side(n)`, `of(&SeamSide, n)`); `SewDraft.a: EdgeRun`. `Document::{edit, gesture_edit}` run `Project::drop_broken` before checking.
- Later tasks rely on all of these names.

**Behaviour:**
- **Spans:**
  - A side's spans run from `from` to `to`, the stored way when `forward`, passing corners and wrapping. A forward side whose `to` is behind its `from` on the same edge goes all the way round.
  - A side from a point to the same point covers nothing.
- **`check()`:** see Global Constraints. Overlap is by span, mirror images included. Two sides that meet at a point are fine.
- **Edits:**
  - Split at fraction `s`: an end with `t < s` goes to `(i, t/s)`, one with `t > s` to `(i+1, (t−s)/(1−s))`. An end exactly at `s` stays on the part its side covers.
  - Removing vertex `i`, where the old edge `i−1` is the first `f` of the joined edge: ends on `i−1` go to `t·f`, ends on `i` to `f + t(1−f)` (and `t = 1` stays 1), and later edges shift down.
  - Unfold maps a drawn end `(e, t)` to `(m, t)` and a pale one to `(2n−3−m, 1−t)`, where `m = (e − first) mod n` is the edge's place on the whole piece; a pale side's `forward` flips.
  - `drop_broken` runs after every split and removal, and in `Document` after every edit.
- **Format 4:** a v3 file's side `{first_edge, edges, forward}` becomes `from: {edge: first, t: 0}`, `to: {edge: last, t: 1}` (swapped when it ran backwards). A side naming edges its piece lacks ends past the last edge, so the check still refuses it.

- [ ] **Step 1: Failing core tests**

Replace the whole of `crates/core/src/seam.rs` with only its tests for now (the model goes above them in Step 3):

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn span(edge: usize, t0: f64, t1: f64) -> Span {
        Span { edge, t0, t1 }
    }

    #[test]
    fn whole_edge_sides_cover_their_edges_and_wrap() {
        let s = SeamSide::edges(PieceId(1), Half::Drawn, 3, 0, true);
        assert_eq!(
            s.spans(5),
            vec![span(3, 0.0, 1.0), span(4, 0.0, 1.0), span(0, 0.0, 1.0)]
        );
        assert!(s.covers(5, 4) && s.covers(5, 0) && !s.covers(5, 1) && !s.covers(5, 2));
        assert_eq!(s.from, OutlinePos::new(3, 0.0));
        assert_eq!(s.to, OutlinePos::new(0, 1.0));
        let back = SeamSide::edges(PieceId(1), Half::Drawn, 3, 0, false);
        assert_eq!(
            (back.from, back.to),
            (OutlinePos::new(0, 1.0), OutlinePos::new(3, 0.0))
        );
        assert_eq!(
            back.spans(5),
            vec![span(0, 0.0, 1.0), span(4, 0.0, 1.0), span(3, 0.0, 1.0)],
            "listed the way it runs"
        );
        assert!(s.same_part(&back) && s != back && back == s.flipped());
        assert!(!s.same_part(&SeamSide {
            half: Half::Pale,
            ..s
        }));
        assert_eq!(Half::Pale.other(), Half::Drawn);
    }

    #[test]
    fn free_sides_cover_parts_of_edges_and_pass_corners() {
        let side = |from: OutlinePos, to: OutlinePos, forward| SeamSide {
            shape: PieceId(1),
            half: Half::Drawn,
            from,
            to,
            forward,
        };
        let p = OutlinePos::new;
        assert_eq!(
            side(p(1, 0.25), p(1, 0.75), true).spans(4),
            vec![span(1, 0.25, 0.75)]
        );
        assert_eq!(
            side(p(1, 0.75), p(1, 0.25), false).spans(4),
            vec![span(1, 0.25, 0.75)]
        );
        // Round the corner between edges 1 and 2.
        assert_eq!(
            side(p(1, 0.5), p(2, 0.5), true).spans(4),
            vec![span(1, 0.5, 1.0), span(2, 0.0, 0.5)]
        );
        // The long way round from the same two points: every other edge.
        assert_eq!(
            side(p(1, 0.5), p(2, 0.5), false).spans(4),
            vec![
                span(1, 0.0, 0.5),
                span(0, 0.0, 1.0),
                span(3, 0.0, 1.0),
                span(2, 0.5, 1.0)
            ]
        );
        // Starting further along the same edge than it ends: all the way round.
        assert_eq!(side(p(1, 0.75), p(1, 0.25), true).spans(4).len(), 5);
        // A side ending where it starts covers nothing; an edge the outline lacks, nothing.
        assert!(side(p(1, 0.5), p(1, 0.5), true).spans(4).is_empty());
        assert!(side(p(4, 0.0), p(1, 0.5), true).spans(4).is_empty());
        // A start at the very end of an edge covers nothing of that edge.
        assert_eq!(
            side(p(0, 1.0), p(1, 0.5), true).spans(4),
            vec![span(1, 0.0, 0.5)]
        );
    }

    #[test]
    fn a_side_ending_at_a_corner_is_tidied_onto_the_edge_it_covers() {
        let p = OutlinePos::new;
        let side = |from, to, forward| SeamSide {
            shape: PieceId(1),
            half: Half::Drawn,
            from,
            to,
            forward,
        };
        // Clicked at the corner between edges 0 and 1, then at the corner between 2 and 3: the
        // same as whole edges 1 and 2.
        let clicked = side(p(0, 1.0), p(3, 0.0), true);
        assert_eq!(
            clicked.tidy(4),
            SeamSide::edges(PieceId(1), Half::Drawn, 1, 2, true)
        );
        assert_eq!(clicked.tidy(4).spans(4), clicked.spans(4));
        let backwards = side(p(3, 0.0), p(0, 1.0), false);
        assert_eq!(
            backwards.tidy(4),
            SeamSide::edges(PieceId(1), Half::Drawn, 1, 2, false)
        );
        // Ends inside edges stay as they are.
        let inside = side(p(0, 0.5), p(1, 0.25), true);
        assert_eq!(inside.tidy(4), inside);
    }

    #[test]
    fn sides_overlap_only_when_they_share_more_than_a_point() {
        let side = |e: usize, t0: f64, t1: f64| SeamSide {
            shape: PieceId(1),
            half: Half::Drawn,
            from: OutlinePos::new(e, t0),
            to: OutlinePos::new(e, t1),
            forward: true,
        };
        assert!(
            !side(1, 0.0, 0.5).overlaps(&side(1, 0.5, 1.0), 4),
            "they meet"
        );
        assert!(side(1, 0.0, 0.5).overlaps(&side(1, 0.4, 1.0), 4));
        assert!(!side(1, 0.0, 0.5).overlaps(&side(2, 0.0, 0.5), 4));
        let pale = SeamSide {
            half: Half::Pale,
            ..side(1, 0.0, 0.5)
        };
        assert!(!side(1, 0.0, 0.5).overlaps(&pale, 4), "the other half");
    }

    #[test]
    fn seams_serialise_plainly() {
        let seam = Seam {
            id: SeamId(4),
            a: SeamSide::edges(PieceId(1), Half::Pale, 2, 2, true),
            b: SeamSide {
                shape: PieceId(3),
                half: Half::Drawn,
                from: OutlinePos::new(0, 0.5),
                to: OutlinePos::new(1, 0.25),
                forward: false,
            },
        };
        let json = serde_json::to_string(&seam).unwrap();
        assert_eq!(
            json,
            r#"{"id":4,"a":{"shape":1,"half":"pale","from":{"edge":2,"t":0.0},"to":{"edge":2,"t":1.0},"forward":true},"b":{"shape":3,"half":"drawn","from":{"edge":0,"t":0.5},"to":{"edge":1,"t":0.25},"forward":false}}"#
        );
        assert_eq!(serde_json::from_str::<Seam>(&json).unwrap(), seam);
    }
}
```

Create `crates/core/src/measure.rs` with only its tests for now:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::PieceId;

    #[test]
    fn edges_are_measured_closely_enough_for_the_checks() {
        let mut square = Piece::rectangle(PieceId(1), "S", Point2::new(0.0, 0.0), 100.0, 100.0);
        assert_eq!(edge_length(&square, 0), 100.0);
        // A quarter circle of radius 300 (control points at the usual 0.5523 of the radius).
        let k = 0.552_284_75 * 300.0;
        square.vertices[1].pos = Point2::new(300.0, 0.0);
        square.vertices[2].pos = Point2::new(0.0, 300.0);
        square.edges[1] = Edge::Curve {
            c1: Point2::new(300.0, k),
            c2: Point2::new(k, 300.0),
        };
        let arc = std::f64::consts::FRAC_PI_2 * 300.0;
        assert!(
            (edge_length(&square, 1) - arc).abs() < 0.1,
            "{}",
            edge_length(&square, 1)
        );
    }
}
```

In `crates/core/src/project.rs`'s `mod tests`, replace everything from the `/// A folded front half` comment above `fn sewing_room` to the end of the file with the code below. It keeps every M4a test, rewritten for whole-edge sides named first and last edge (`side(1, Half::Drawn, 3, 0, true)` is edges 3 and 0). It adds `free_sides_are_checked`, `splitting_an_edge_moves_free_ends_onto_the_part_they_are_on` and `an_edit_that_leaves_a_side_1_mm_long_or_less_deletes_its_seam`. It replaces the two "removing points shrinks sides" tests, because a removed point no longer shortens a side by an edge.

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

    /// Whole stored edges `first` to `last` (wrapping) of shape `shape`.
    fn side(shape: u32, half: Half, first: usize, last: usize, forward: bool) -> SeamSide {
        SeamSide::edges(PieceId(shape), half, first, last, forward)
    }

    #[test]
    fn mirrors_are_derived_for_folds_and_pairs() {
        let mut pr = sewing_room();
        // Front's right edge to the back's left edge (edge 3 of a rectangle).
        let side_seam = pr.add_seam(
            side(1, Half::Drawn, 1, 1, true),
            side(2, Half::Drawn, 3, 3, false),
        );
        // The back's right edge to its twin's: its own mirror image.
        let centre_back = pr.add_seam(
            side(2, Half::Drawn, 1, 1, true),
            side(3, Half::Drawn, 1, 1, true),
        );
        // The pocket has no mirror image, so neither has its seam.
        let pocket = pr.add_seam(
            side(4, Half::Drawn, 0, 0, true),
            side(1, Half::Drawn, 0, 0, true),
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
                    b: side(3, Half::Drawn, 3, 3, false)
                },
                true
            )
        );
        assert_eq!(
            (all[2].0.id, all[2].1, all[3].0.id),
            (centre_back, false, pocket)
        );
        // Sewn the other way round, the centre back is still its own mirror image.
        let twisted = pr.seam(centre_back).unwrap().b.flipped();
        pr.seam_mut(centre_back).unwrap().b = twisted;
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
        let ok = side(4, Half::Drawn, 0, 0, true);
        assert_eq!(
            bad(side(9, Half::Drawn, 0, 0, true), ok),
            Err(ModelError::BadSeam(SeamId(1))),
            "no such shape"
        );
        let empty = SeamSide {
            to: OutlinePos::new(0, 0.0),
            ..side(2, Half::Drawn, 0, 0, true)
        };
        assert_eq!(
            bad(empty, ok),
            Err(ModelError::BadSeam(SeamId(1))),
            "ends where it starts"
        );
        assert_eq!(
            bad(side(2, Half::Drawn, 0, 4, true), ok),
            Err(ModelError::BadSeam(SeamId(1))),
            "ends past the last edge"
        );
        assert_eq!(
            bad(side(2, Half::Drawn, 4, 4, true), ok),
            Err(ModelError::BadSeam(SeamId(1))),
            "no such edge"
        );
        assert_eq!(
            bad(side(2, Half::Pale, 0, 0, true), ok),
            Err(ModelError::BadSeam(SeamId(1))),
            "pale half of an unfolded piece"
        );
        assert_eq!(
            bad(side(1, Half::Drawn, 2, 3, true), ok),
            Err(ModelError::BadSeam(SeamId(1))),
            "across the fold"
        );
        assert_eq!(
            bad(side(4, Half::Drawn, 3, 0, true), ok),
            Err(ModelError::BadSeam(SeamId(1))),
            "edge 0 twice"
        );
        // The first seam's mirror image already sews the pale half's edge 1: the second seam,
        // which sews it again, is the one refused.
        let mut pr = base.clone();
        pr.add_seam(
            side(1, Half::Drawn, 1, 1, true),
            side(2, Half::Drawn, 3, 3, false),
        );
        pr.add_seam(
            side(1, Half::Pale, 1, 1, true),
            side(4, Half::Drawn, 0, 0, true),
        );
        assert_eq!(pr.check(), Err(ModelError::BadSeam(SeamId(2))));
        let mut twice = base.clone();
        twice.add_seam(
            side(4, Half::Drawn, 0, 0, true),
            side(4, Half::Drawn, 2, 2, true),
        );
        twice.seams.push(Seam {
            id: SeamId(1),
            ..twice.seams[0]
        });
        twice.seams[1].a = side(4, Half::Drawn, 1, 1, true);
        twice.seams[1].b = side(4, Half::Drawn, 3, 3, true);
        assert_eq!(
            twice.check(),
            Err(ModelError::BadSeam(SeamId(1))),
            "an id used twice"
        );
        let mut many = base.clone();
        for k in 0..=MAX_SEAMS {
            many.seams.push(Seam {
                id: SeamId(k as u32 + 1),
                a: side(4, Half::Drawn, 0, 0, true),
                b: side(4, Half::Drawn, 1, 1, true),
            });
        }
        assert_eq!(many.check(), Err(ModelError::TooManySeams));
        assert_eq!(
            ModelError::BadSeam(SeamId(7)).to_string(),
            "seam 7 is invalid"
        );
    }

    /// The part of stored edge `edge` of shape `shape` from fraction `t0` to `t1`.
    fn part(shape: u32, half: Half, edge: usize, t0: f64, t1: f64) -> SeamSide {
        SeamSide {
            shape: PieceId(shape),
            half,
            from: OutlinePos::new(edge, t0),
            to: OutlinePos::new(edge, t1),
            forward: t1 >= t0,
        }
    }

    #[test]
    fn free_sides_are_checked() {
        let base = sewing_room();
        let with = |seams: &[(SeamSide, SeamSide)]| {
            let mut pr = base.clone();
            for (a, b) in seams {
                pr.add_seam(*a, *b);
            }
            pr.check()
        };
        // The pocket (id 4) is 80 mm square; the back (id 2) is 100 mm wide.
        let ok = part(2, Half::Drawn, 0, 0.0, 0.5);
        for (bad, why) in [
            (
                part(4, Half::Drawn, 0, 0.5, 1.5),
                "past the end of its edge",
            ),
            (part(4, Half::Drawn, 0, f64::NAN, 0.5), "not a number"),
            (part(4, Half::Drawn, 0, 0.5, 0.5125), "1 mm long"),
        ] {
            assert_eq!(
                with(&[(bad, ok)]),
                Err(ModelError::BadSeam(SeamId(1))),
                "{why}"
            );
        }
        assert_eq!(
            with(&[(part(4, Half::Drawn, 0, 0.5, 0.52), ok)]),
            Ok(()),
            "1.6 mm"
        );
        // Two seams on one edge may meet, not overlap.
        let (left, right) = (
            part(4, Half::Drawn, 0, 0.0, 0.5),
            part(4, Half::Drawn, 0, 0.5, 1.0),
        );
        let other = part(2, Half::Drawn, 0, 0.5, 1.0);
        assert_eq!(with(&[(left, ok), (right, other)]), Ok(()));
        let overlapping = part(4, Half::Drawn, 0, 0.4, 1.0);
        assert_eq!(
            with(&[(left, ok), (overlapping, other)]),
            Err(ModelError::BadSeam(SeamId(2)))
        );
        // The mirror image of half the front's right edge sews half of the pale one.
        let front_half = part(1, Half::Drawn, 1, 0.0, 0.5);
        let pale = part(1, Half::Pale, 1, 0.25, 0.75);
        assert_eq!(
            with(&[(front_half, ok), (pale, other)]),
            Err(ModelError::BadSeam(SeamId(2)))
        );
        assert_eq!(
            with(&[(front_half, ok), (part(1, Half::Pale, 1, 0.5, 1.0), other)]),
            Ok(())
        );
    }

    #[test]
    fn splitting_an_edge_moves_free_ends_onto_the_part_they_are_on() {
        let mut pr = sewing_room();
        let middle = pr.add_seam(
            part(4, Half::Drawn, 0, 0.25, 0.75),
            part(2, Half::Drawn, 0, 0.0, 0.5),
        );
        // Two sides meeting exactly where the edge is split, one of them running backwards.
        let below = pr.add_seam(
            part(2, Half::Drawn, 2, 0.5, 0.0),
            part(3, Half::Drawn, 2, 0.0, 0.5),
        );
        let above = pr.add_seam(
            part(2, Half::Drawn, 2, 0.5, 1.0),
            part(3, Half::Drawn, 2, 0.5, 1.0),
        );
        pr.piece_mut(PieceId(4)).unwrap().split_edge_at(
            0,
            crate::Vertex::corner(Point2::new(40.0, 400.0)),
            crate::Edge::Line,
            crate::Edge::Line,
            40.0,
        );
        pr.seams_after_split(PieceId(4), 0, 0.5);
        let p = OutlinePos::new;
        let a = pr.seam(middle).unwrap().a;
        assert_eq!(
            (a.from, a.to),
            (p(0, 0.5), p(1, 0.5)),
            "round the new corner"
        );
        pr.piece_mut(PieceId(2)).unwrap().split_edge_at(
            2,
            crate::Vertex::corner(Point2::new(350.0, 200.0)),
            crate::Edge::Line,
            crate::Edge::Line,
            50.0,
        );
        pr.seams_after_split(PieceId(2), 2, 0.5);
        let (b, c) = (pr.seam(below).unwrap().a, pr.seam(above).unwrap().a);
        assert_eq!((b.from, b.to), (p(2, 1.0), p(2, 0.0)), "the first part");
        assert_eq!((c.from, c.to), (p(3, 0.0), p(3, 1.0)), "the second part");
        // The twin's sides, on the same stored edge, moved with them.
        let (bt, ct) = (pr.seam(below).unwrap().b, pr.seam(above).unwrap().b);
        assert_eq!(
            (bt.from, bt.to, ct.from, ct.to),
            (p(2, 0.0), p(2, 1.0), p(3, 0.0), p(3, 1.0))
        );
        assert_eq!(pr.check(), Ok(()));
    }

    #[test]
    fn an_edit_that_leaves_a_side_1_mm_long_or_less_deletes_its_seam() {
        let mut pr = sewing_room();
        let short = pr.add_seam(
            part(4, Half::Drawn, 0, 0.1, 0.2),
            part(2, Half::Drawn, 0, 0.0, 0.5),
        );
        let kept = pr.add_seam(
            part(4, Half::Drawn, 2, 0.0, 1.0),
            part(2, Half::Drawn, 2, 0.0, 1.0),
        );
        assert_eq!(pr.side_length(&pr.seam(short).unwrap().a), Some(8.0));
        // The pocket's bottom edge pulled in to 5 mm: the first side would be 0.5 mm long.
        pr.piece_mut(PieceId(4))
            .unwrap()
            .move_vertex(1, Point2::new(5.0, 400.0));
        assert_eq!(
            pr.check(),
            Err(ModelError::BadSeam(short)),
            "refused if kept"
        );
        pr.drop_broken();
        assert!(pr.seam(short).is_none() && pr.seam(kept).is_some());
        assert_eq!(pr.check(), Ok(()));
    }

    #[test]
    fn splitting_a_sewn_edge_keeps_both_parts_sewn() {
        let mut pr = sewing_room();
        // The back's edges 3 and 0 (wrapping) to the pocket's edge 1; the twin's edge 2 to the
        // pocket's edge 2.
        let wrap = pr.add_seam(
            side(2, Half::Drawn, 3, 0, true),
            side(4, Half::Drawn, 1, 1, true),
        );
        let twin = pr.add_seam(
            side(3, Half::Drawn, 2, 2, false),
            side(4, Half::Drawn, 2, 2, true),
        );
        let back = pr.piece_mut(PieceId(2)).unwrap();
        back.split_edge_at(
            0,
            crate::Vertex::corner(Point2::new(350.0, 0.0)),
            crate::Edge::Line,
            crate::Edge::Line,
            50.0,
        );
        pr.seams_after_split(PieceId(2), 0, 0.5);
        assert_eq!(
            pr.seam(wrap).unwrap().a,
            side(2, Half::Drawn, 4, 1, true),
            "edges 4, 0 and 1 now"
        );
        assert_eq!(
            pr.seam(twin).unwrap().a,
            side(3, Half::Drawn, 3, 3, false),
            "moved up, not grown"
        );
        assert_eq!(pr.check(), Ok(()));
    }

    /// A regular hexagon (every edge 100 mm long) and a copy of it.
    fn two_hexagons() -> (Project, PieceId) {
        let corners: Vec<Point2> = (0..6)
            .map(|k| {
                let a = k as f64 / 6.0 * std::f64::consts::TAU;
                Point2::new(100.0 * a.cos(), 100.0 * a.sin())
            })
            .collect();
        let mut pr = Project::new();
        let hex = pr.add_piece(Piece::polygon(PieceId(0), "Hex", &corners));
        pr.add_piece(Piece::polygon(PieceId(0), "Other", &corners));
        (pr, hex)
    }

    #[test]
    fn removing_points_puts_side_ends_on_the_joined_edge() {
        let (mut pr, hex) = two_hexagons();
        let s1 = pr.add_seam(
            side(1, Half::Drawn, 0, 2, true),
            side(2, Half::Drawn, 0, 0, true),
        );
        let s2 = pr.add_seam(
            side(1, Half::Drawn, 3, 3, true),
            side(2, Half::Drawn, 3, 3, true),
        );
        let s3 = pr.add_seam(
            side(1, Half::Drawn, 4, 4, false),
            side(2, Half::Drawn, 4, 4, true),
        );
        let remove = |pr: &mut Project, i: usize| {
            let n = pr.piece(hex).unwrap().len();
            assert!(pr.piece_mut(hex).unwrap().remove_vertex(i, 100.0));
            pr.seams_after_removal(hex, i, n, 0.5);
        };
        // Vertex 1 lies inside the first side: it still runs from (the old) edge 0's start to
        // edge 2's end, now edges 0 and 1.
        remove(&mut pr, 1);
        assert_eq!(pr.seam(s1).unwrap().a, side(1, Half::Drawn, 0, 1, true));
        assert_eq!(pr.seam(s2).unwrap().a, side(1, Half::Drawn, 2, 2, true));
        assert_eq!(pr.seam(s3).unwrap().a, side(1, Half::Drawn, 3, 3, false));
        // Vertex 2 ends the first side and starts the second: each keeps its half of the joined
        // edge 1, and they meet halfway along it.
        remove(&mut pr, 2);
        let p = OutlinePos::new;
        let (a1, a2) = (pr.seam(s1).unwrap().a, pr.seam(s2).unwrap().a);
        assert_eq!((a1.from, a1.to), (p(0, 0.0), p(1, 0.5)));
        assert_eq!((a2.from, a2.to), (p(1, 0.5), p(1, 1.0)));
        assert_eq!(pr.seam(s3).unwrap().a, side(1, Half::Drawn, 2, 2, false));
        assert_eq!(pr.check(), Ok(()));
    }

    #[test]
    fn removing_a_fold_end_drops_the_pale_seams() {
        let mut pr = sewing_room();
        let drawn = pr.add_seam(
            side(1, Half::Drawn, 1, 1, true),
            side(4, Half::Drawn, 0, 0, true),
        );
        let pale = pr.add_seam(
            side(1, Half::Pale, 0, 0, true),
            side(4, Half::Drawn, 2, 2, true),
        );
        let front = pr.piece_mut(PieceId(1)).unwrap();
        front.split_edge_at(
            1,
            crate::Vertex::corner(Point2::new(100.0, 100.0)),
            crate::Edge::Line,
            crate::Edge::Line,
            100.0,
        );
        pr.seams_after_split(PieceId(1), 1, 0.5);
        assert_eq!(pr.seam(drawn).unwrap().a, side(1, Half::Drawn, 1, 2, true));
        // Vertex 4 (0,200) is an end of the fold edge: the fold goes, and the pale seam too.
        assert!(pr.piece_mut(PieceId(1)).unwrap().remove_vertex(4, 100.0));
        assert_eq!(pr.piece(PieceId(1)).unwrap().fold, None);
        pr.seams_after_removal(PieceId(1), 4, 5, 1.0 / 3.0);
        assert!(pr.seam(pale).is_none());
        assert!(pr.seam(drawn).is_some());
        assert_eq!(pr.check(), Ok(()));
    }

    #[test]
    fn deleting_a_piece_or_twin_deletes_its_seams() {
        let mut pr = sewing_room();
        pr.add_seam(
            side(3, Half::Drawn, 0, 0, true),
            side(4, Half::Drawn, 0, 0, true),
        );
        pr.add_seam(
            side(2, Half::Drawn, 0, 0, true),
            side(4, Half::Drawn, 1, 1, true),
        );
        let kept = pr.add_seam(
            side(1, Half::Drawn, 0, 0, true),
            side(4, Half::Drawn, 2, 2, true),
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
            side(2, Half::Drawn, 3, 3, false),
        );
        let pale = pr.add_seam(
            side(1, Half::Pale, 0, 0, true),
            side(4, Half::Drawn, 0, 1, true),
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
        assert_eq!(pr.seam(pale).unwrap().a, side(1, Half::Drawn, 5, 5, false));
        // The side seam's mirror image is a seam of its own now, on edge 4.
        let stored = *pr.seams.last().unwrap();
        assert_eq!(
            (stored.id, stored.a, stored.b),
            (
                SeamId(3),
                side(1, Half::Drawn, 4, 4, false),
                side(3, Half::Drawn, 3, 3, false)
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
            side(2, Half::Drawn, 3, 3, false),
        );
        let pale = pr.add_seam(
            side(1, Half::Pale, 0, 0, true),
            side(4, Half::Drawn, 0, 0, true),
        );
        let on_twin = pr.add_seam(
            side(3, Half::Drawn, 0, 0, true),
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
        assert_eq!(SCHEMA_VERSION, 4);
    }

    #[test]
    fn seam_ids_are_bounded_and_adding_a_seam_never_overflows() {
        let base = sewing_room();
        let with_id = |id: u32| {
            let mut pr = base.clone();
            pr.add_seam(
                side(4, Half::Drawn, 0, 0, true),
                side(4, Half::Drawn, 2, 2, true),
            );
            pr.seams[0].id = SeamId(id);
            pr
        };
        assert_eq!(with_id(MAX_SEAM_ID).check(), Ok(()), "the highest allowed");
        for id in [MAX_SEAM_ID + 1, u32::MAX - 1, u32::MAX] {
            assert_eq!(
                with_id(id).check(),
                Err(ModelError::BadSeam(SeamId(id))),
                "id {id}"
            );
        }
        // Adding to a project at the end of the id range (as a corrupt file could leave it)
        // neither panics (debug) nor wraps round to a used id (release): it takes a free one.
        for id in [MAX_SEAM_ID, u32::MAX] {
            let mut pr = with_id(id);
            let next = pr.add_seam(
                side(4, Half::Drawn, 1, 1, true),
                side(4, Half::Drawn, 3, 3, true),
            );
            assert_eq!(next, SeamId(1), "the lowest id not in use");
            let ids: Vec<u32> = pr.seams.iter().map(|s| s.id.0).collect();
            assert_eq!(ids, vec![id, 1]);
            if id == MAX_SEAM_ID {
                assert_eq!(pr.check(), Ok(()), "and sewing still works there");
            }
        }
        // Exactly the most seams is fine: 2,000 edges of one 2,000-gon sewn to another's.
        let ring = |name: &str| {
            let corners: Vec<Point2> = (0..MAX_SEAMS)
                .map(|k| {
                    let a = k as f64 / MAX_SEAMS as f64 * std::f64::consts::TAU;
                    Point2::new(1000.0 * a.cos(), 1000.0 * a.sin())
                })
                .collect();
            Piece::polygon(PieceId(0), name, &corners)
        };
        let mut full = Project::new();
        let (a, b) = (full.add_piece(ring("A")), full.add_piece(ring("B")));
        for k in 0..MAX_SEAMS {
            full.add_seam(
                SeamSide::edges(a, Half::Drawn, k, k, true),
                SeamSide::edges(b, Half::Drawn, k, k, true),
            );
        }
        assert_eq!(full.seams.len(), MAX_SEAMS);
        assert_eq!(full.check(), Ok(()));
    }

    #[test]
    fn a_pale_side_on_a_twin_is_refused() {
        let mut pr = sewing_room();
        pr.add_seam(
            side(3, Half::Pale, 0, 0, true),
            side(4, Half::Drawn, 0, 0, true),
        );
        assert_eq!(pr.check(), Err(ModelError::BadSeam(SeamId(1))));
    }

    #[test]
    fn breaking_a_pair_or_deleting_its_piece_leaves_the_twin_where_it_was() {
        let placed = Placement {
            position: [0.1, 0.8, -0.2],
            rotation: [0.0, 0.6, 0.0, 0.8],
            curve: Some(0.2),
        };
        // The twin has none of its own: it was showing its piece's, mirrored.
        let mut broken = sewing_room();
        broken.set_placement(PieceId(2), Some(placed));
        assert_eq!(broken.placement_of(PieceId(3)), None);
        assert_eq!(broken.break_twin(PieceId(2)), Some(PieceId(3)));
        assert_eq!(broken.placement_of(PieceId(3)), Some(placed.mirrored()));
        assert_eq!(
            broken.placement_of(PieceId(2)),
            Some(placed),
            "the piece stays"
        );
        assert_eq!(broken.check(), Ok(()));
        let mut deleted = sewing_room();
        deleted.set_placement(PieceId(2), Some(placed));
        assert!(deleted.remove_piece(PieceId(2)).is_some());
        assert_eq!(deleted.placement_of(PieceId(3)), Some(placed.mirrored()));
        assert_eq!(deleted.check(), Ok(()));
        // Its own placement wins; with neither, there is still none.
        let own = Placement::at([0.0, 0.5, 1.0]);
        let mut owned = sewing_room();
        owned.set_placement(PieceId(2), Some(placed));
        owned.set_placement(PieceId(3), Some(own));
        owned.break_twin(PieceId(2));
        assert_eq!(owned.placement_of(PieceId(3)), Some(own));
        let mut neither = sewing_room();
        neither.break_twin(PieceId(2));
        assert_eq!(neither.placement_of(PieceId(3)), None);
    }

    #[test]
    fn splitting_an_edge_grows_sides_on_the_b_side_the_twin_and_the_pale_half() {
        let mut pr = sewing_room();
        // The pocket (4) is side a throughout; the edited pieces are side b.
        // b on the back, wrapping over edges 3 and 0.
        let wrap = pr.add_seam(
            side(4, Half::Drawn, 1, 1, true),
            side(2, Half::Drawn, 3, 0, true),
        );
        // b on the twin, covering edge 0: splitting the back's edge 0 splits the twin's too.
        let twin = pr.add_seam(
            side(4, Half::Drawn, 2, 2, true),
            side(3, Half::Drawn, 0, 0, false),
        );
        // b on the pale half of the front, covering edge 1.
        let pale = pr.add_seam(
            side(4, Half::Drawn, 3, 3, true),
            side(1, Half::Pale, 1, 1, true),
        );
        assert_eq!(pr.check(), Ok(()));
        for (id, at) in [(PieceId(2), 0), (PieceId(1), 1)] {
            pr.piece_mut(id).unwrap().split_edge_at(
                at,
                crate::Vertex::corner(Point2::new(
                    if id == PieceId(2) { 350.0 } else { 100.0 },
                    if id == PieceId(2) { 0.0 } else { 50.0 },
                )),
                crate::Edge::Line,
                crate::Edge::Line,
                50.0,
            );
            // 50 mm along the back's 100 mm edge 0, or the front's 200 mm edge 1.
            pr.seams_after_split(id, at, if id == PieceId(2) { 0.5 } else { 0.25 });
        }
        assert_eq!(
            pr.seam(wrap).unwrap().b,
            side(2, Half::Drawn, 4, 1, true),
            "edges 4, 0 and 1 now"
        );
        assert_eq!(
            pr.seam(twin).unwrap().b,
            side(3, Half::Drawn, 0, 1, false),
            "the twin's side grows too"
        );
        assert_eq!(
            pr.seam(pale).unwrap().b,
            side(1, Half::Pale, 1, 2, true),
            "so does one on the pale half"
        );
        assert_eq!(pr.check(), Ok(()));
    }

    #[test]
    fn removing_points_moves_side_ends_on_the_b_side_too() {
        let (mut pr, hex) = two_hexagons();
        // The same seams as `removing_points_puts_side_ends_on_the_joined_edge`, with the
        // edited hexagon (id 1) as side b.
        let s1 = pr.add_seam(
            side(2, Half::Drawn, 0, 0, true),
            side(1, Half::Drawn, 0, 2, true),
        );
        let s2 = pr.add_seam(
            side(2, Half::Drawn, 3, 3, true),
            side(1, Half::Drawn, 3, 3, true),
        );
        let s3 = pr.add_seam(
            side(2, Half::Drawn, 4, 4, true),
            side(1, Half::Drawn, 4, 4, false),
        );
        let remove = |pr: &mut Project, i: usize| {
            let n = pr.piece(hex).unwrap().len();
            assert!(pr.piece_mut(hex).unwrap().remove_vertex(i, 100.0));
            pr.seams_after_removal(hex, i, n, 0.5);
        };
        remove(&mut pr, 1);
        assert_eq!(pr.seam(s1).unwrap().b, side(1, Half::Drawn, 0, 1, true));
        assert_eq!(pr.seam(s2).unwrap().b, side(1, Half::Drawn, 2, 2, true));
        assert_eq!(pr.seam(s3).unwrap().b, side(1, Half::Drawn, 3, 3, false));
        remove(&mut pr, 2);
        let p = OutlinePos::new;
        let (b1, b2) = (pr.seam(s1).unwrap().b, pr.seam(s2).unwrap().b);
        assert_eq!((b1.from, b1.to), (p(0, 0.0), p(1, 0.5)));
        assert_eq!(
            (b2.from, b2.to),
            (p(1, 0.5), p(1, 1.0)),
            "half the joined edge"
        );
        assert_eq!(pr.seam(s3).unwrap().b, side(1, Half::Drawn, 2, 2, false));
        assert_eq!(pr.check(), Ok(()));
    }

    #[test]
    fn unfolding_renumbers_sides_on_the_b_side_too() {
        let mut pr = sewing_room();
        // The pocket (4) is side a; the front (1, folded on edge 3) is side b: its drawn edge 1
        // (with the back as a), and its pale edge 0 and pale edges 1-2 (the pocket).
        let drawn = pr.add_seam(
            side(2, Half::Drawn, 3, 3, false),
            side(1, Half::Drawn, 1, 1, true),
        );
        let pale = pr.add_seam(
            side(4, Half::Drawn, 0, 1, true),
            side(1, Half::Pale, 0, 0, true),
        );
        assert_eq!(pr.check(), Ok(()));
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
        assert_eq!(pr.seam(drawn).unwrap().b, side(1, Half::Drawn, 1, 1, true));
        assert_eq!(
            pr.seam(pale).unwrap().b,
            side(1, Half::Drawn, 5, 5, false),
            "the pale image of edge 0 is the whole piece's edge 5, run backwards"
        );
        // The drawn seam's mirror image (back's twin against the pale edge 1) is stored now.
        let stored = *pr.seams.last().unwrap();
        assert_eq!(
            (stored.a, stored.b),
            (
                side(3, Half::Drawn, 3, 3, false),
                side(1, Half::Drawn, 4, 4, false)
            )
        );
        assert_eq!(pr.check(), Ok(()));
    }
}
```

- [ ] **Step 2: Run them**

Run: `cargo nextest run -p opendrape-core`
Expected: compile errors (no `OutlinePos`, `SeamSide::edges`, `drop_broken`, `measure`…).

- [ ] **Step 3: The model**

Put the model above the tests in `crates/core/src/seam.rs`:

```rust
//! Seams: which stretches of outline are sewn to which. A side runs between two points on one
//! shape's outline, which may be anywhere along an edge ("free" sewing); a whole-edge seam is a
//! free seam whose ends are corners. Only the seams the student sewed are stored; the mirror
//! image of a seam on a cut-on-fold piece or a mirrored pair is worked out when it is needed
//! (see `Project::mirror_of`).

use crate::PieceId;
use serde::{Deserialize, Serialize};

/// Most seams a project may hold (their mirror images are not counted).
pub const MAX_SEAMS: usize = 2_000;

/// Highest seam id a project may hold. Far above anything a student sews (ids are one more than
/// the highest in use); it only stops a corrupt file from putting an id at the end of its range.
pub const MAX_SEAM_ID: u32 = 1_000_000;

/// A seam side must be longer than this (mm); an edit that leaves one this short or shorter
/// deletes its seam.
pub const MIN_SIDE_MM: f64 = 1.0;

/// Two stretches of outline may meet at a point; they overlap only when they share more than
/// this much of an edge (as a fraction of its length), so rounding never counts as overlap.
const OVERLAP_SLACK: f64 = 1e-9;

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

/// A point on a stored piece's outline: stored edge `edge`, a fraction `t` (0 at the edge's
/// start, 1 at its end) of the way along it by arc length.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct OutlinePos {
    pub edge: usize,
    pub t: f64,
}

impl OutlinePos {
    pub const fn new(edge: usize, t: f64) -> Self {
        Self { edge, t }
    }
}

/// The part of one stored edge a side covers: from fraction `t0` to `t1` of the way along it,
/// `t0 < t1`, whichever way the side runs.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Span {
    pub edge: usize,
    pub t0: f64,
    pub t1: f64,
}

/// One side of a seam: the outline of one shape from `from` to `to`.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct SeamSide {
    /// A piece's id or a twin's id.
    pub shape: PieceId,
    #[serde(default)]
    pub half: Half,
    /// Where the side starts: it meets the other side's start.
    pub from: OutlinePos,
    /// Where the side ends.
    pub to: OutlinePos,
    /// True when the side runs from `from` to `to` the way the stored outline runs; false when
    /// it runs the other way. It may pass corners and wrap past the last edge.
    pub forward: bool,
}

impl SeamSide {
    /// A side of whole stored edges: `first` to `last` along the stored outline (wrapping past
    /// the last edge), starting at `first`'s start when `forward`, at `last`'s end otherwise.
    pub fn edges(shape: PieceId, half: Half, first: usize, last: usize, forward: bool) -> Self {
        let (low, high) = (OutlinePos::new(first, 0.0), OutlinePos::new(last, 1.0));
        let (from, to) = if forward { (low, high) } else { (high, low) };
        Self {
            shape,
            half,
            from,
            to,
            forward,
        }
    }

    /// The same side with an end that is at a corner named on the edge the side covers there
    /// (a start at the very end of an edge is the start of the next one; an end at the very
    /// start of an edge is the end of the one before), on an outline of `n` edges. Sides that
    /// cover the same stretch the same way then compare equal.
    pub fn tidy(&self, n: usize) -> Self {
        if n == 0 {
            return *self;
        }
        let (next, prev) = (|e: usize| (e + 1) % n, |e: usize| (e + n - 1) % n);
        let (mut from, mut to) = (self.from, self.to);
        // The edge a side runs into from a corner: the next one running forward, the one
        // before running backward.
        if self.forward {
            if from.t >= 1.0 {
                from = OutlinePos::new(next(from.edge), 0.0);
            }
            if to.t <= 0.0 {
                to = OutlinePos::new(prev(to.edge), 1.0);
            }
        } else {
            if from.t <= 0.0 {
                from = OutlinePos::new(prev(from.edge), 1.0);
            }
            if to.t >= 1.0 {
                to = OutlinePos::new(next(to.edge), 0.0);
            }
        }
        Self { from, to, ..*self }
    }

    /// The same stretch of outline, run the other way.
    pub fn flipped(&self) -> Self {
        Self {
            from: self.to,
            to: self.from,
            forward: !self.forward,
            ..*self
        }
    }

    /// The parts of stored edges it covers on an outline of `n` edges, in the order the side
    /// runs (each with `t0 < t1`). Empty when it covers nothing (it starts where it ends) or names
    /// an edge the outline doesn't have.
    pub fn spans(&self, n: usize) -> Vec<Span> {
        let (f, t) = (self.from, self.to);
        if n == 0 || f.edge >= n || t.edge >= n {
            return Vec::new();
        }
        let mut out = Vec::new();
        if self.forward {
            if f.edge == t.edge && t.t >= f.t {
                out.push(Span {
                    edge: f.edge,
                    t0: f.t,
                    t1: t.t,
                });
            } else {
                out.push(Span {
                    edge: f.edge,
                    t0: f.t,
                    t1: 1.0,
                });
                let mut e = (f.edge + 1) % n;
                while e != t.edge {
                    out.push(Span {
                        edge: e,
                        t0: 0.0,
                        t1: 1.0,
                    });
                    e = (e + 1) % n;
                }
                out.push(Span {
                    edge: t.edge,
                    t0: 0.0,
                    t1: t.t,
                });
            }
        } else if f.edge == t.edge && t.t <= f.t {
            out.push(Span {
                edge: f.edge,
                t0: t.t,
                t1: f.t,
            });
        } else {
            out.push(Span {
                edge: f.edge,
                t0: 0.0,
                t1: f.t,
            });
            let mut e = (f.edge + n - 1) % n;
            while e != t.edge {
                out.push(Span {
                    edge: e,
                    t0: 0.0,
                    t1: 1.0,
                });
                e = (e + n - 1) % n;
            }
            out.push(Span {
                edge: t.edge,
                t0: t.t,
                t1: 1.0,
            });
        }
        out.retain(|s| s.t1 > s.t0);
        out
    }

    /// Whether it covers any of stored edge `e` (of an outline of `n` edges).
    pub fn covers(&self, n: usize, e: usize) -> bool {
        self.spans(n).iter().any(|s| s.edge == e)
    }

    /// Whether it and `other` cover the same stretch of the same shape's half, whichever way
    /// each runs.
    pub fn same_part(&self, other: &SeamSide) -> bool {
        (self.shape, self.half) == (other.shape, other.half)
            && (*self == *other || *self == other.flipped())
    }

    /// Whether it shares more than a point with `other` (on an outline of `n` edges).
    pub fn overlaps(&self, other: &SeamSide, n: usize) -> bool {
        (self.shape, self.half) == (other.shape, other.half)
            && spans_overlap(&self.spans(n), &other.spans(n))
    }
}

/// Whether any span of `a` shares more than a point with any span of `b`.
pub(crate) fn spans_overlap(a: &[Span], b: &[Span]) -> bool {
    a.iter().any(|p| {
        b.iter()
            .any(|q| p.edge == q.edge && p.t0.max(q.t0) < p.t1.min(q.t1) - OVERLAP_SLACK)
    })
}

/// Two sides sewn together, matched end to end: `a`'s start meets `b`'s start.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
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

Put the measuring above the tests in `crates/core/src/measure.rs`:

```rust
//! The little geometry `Project::check` needs without a curve library: how long an edge is, and
//! how far a point lies outside a piece. Curves are followed as polylines of [`CURVE_STEPS`]
//! steps, which is well within the millimetre the checks allow (a 30 cm curve bent through a
//! right angle comes out about 0.02 mm short). `opendrape-geom` measures exactly for drawing and
//! meshing; these numbers only ever decide validity, so every rule that uses them uses them alone.

use crate::{Edge, Piece, Point2};

/// Steps a curved edge is followed in.
const CURVE_STEPS: usize = 64;

/// Point at parameter `s` of edge `i` (0 at its start, 1 at its end).
fn edge_point(piece: &Piece, i: usize, s: f64) -> Point2 {
    let (a, b) = piece.edge_ends(i);
    match piece.edges[i] {
        Edge::Line => a.lerp(b, s),
        Edge::Curve { c1, c2 } => {
            let u = 1.0 - s;
            a * (u * u * u) + c1 * (3.0 * u * u * s) + c2 * (3.0 * u * s * s) + b * (s * s * s)
        }
    }
}

/// Edge `i` as points from its start to its end.
fn edge_polyline(piece: &Piece, i: usize) -> Vec<Point2> {
    let steps = match piece.edges[i] {
        Edge::Line => 1,
        Edge::Curve { .. } => CURVE_STEPS,
    };
    (0..=steps)
        .map(|k| edge_point(piece, i, k as f64 / steps as f64))
        .collect()
}

/// Length (mm) of edge `i`, as the checks measure it.
pub(crate) fn edge_length(piece: &Piece, i: usize) -> f64 {
    edge_polyline(piece, i)
        .windows(2)
        .map(|w| w[0].distance(w[1]))
        .sum()
}
```

In `crates/core/src/lib.rs`, add `mod measure;` above `mod piece;`, and replace the `pub use seam::…` line with:

```rust
pub use seam::{
    Half, MAX_SEAM_ID, MAX_SEAMS, MIN_SIDE_MM, OutlinePos, Seam, SeamId, SeamSide, Span,
};
```

In `crates/core/src/project.rs`:
- Replace the `use crate::{…}` block and the `SCHEMA_VERSION` comment and constant with:

```rust
use crate::seam::spans_overlap;
use crate::{
    Half, MAX_SEAM_ID, MAX_SEAMS, MIN_SIDE_MM, OutlinePos, Piece, PieceId, Placement, Point2, Seam,
    SeamId, SeamSide, Side, Span, Units, measure,
};
use serde::{Deserialize, Serialize};

/// Version of the project format written by this build. Bump it when the format changes, and
/// add a migration step in `opendrape-io`. Version 2 added seam allowances, notches, internal
/// lines, folds and twins; version 3 added seams and 3D placements (2026-10-09); version 4 made
/// seam sides run between any two points of an outline (2026-10-10).
pub const SCHEMA_VERSION: u32 = 4;
```

- In `mirror_of`, `a.same_edges(&seam.b) && b.same_edges(&seam.a)` becomes `a.same_part(&seam.b) && b.same_part(&seam.a)`.
- Replace `seam_on`'s doc comment (its body is unchanged: `SeamSide::covers` keeps its meaning):

```rust
    /// The seam (stored, or the stored seam whose mirror image it is) that sews any part of
    /// stored edge `edge` of `half` of shape `shape`.
    pub fn seam_on(&self, shape: PieceId, half: Half, edge: usize) -> Option<SeamId> {
        let n = self.owner(shape)?.0.len();
        self.all_seams().into_iter().find_map(|(s, _)| {
            [s.a, s.b]
                .iter()
                .any(|side| side.shape == shape && side.half == half && side.covers(n, edge))
                .then_some(s.id)
        })
    }
```

- Replace `seams_after_split` and `seams_after_removal` with these four methods:

```rust
    /// How long (mm) a side is, as [`Self::check`] measures it; None when its shape is missing
    /// or it covers nothing.
    pub fn side_length(&self, side: &SeamSide) -> Option<f64> {
        let (piece, _) = self.owner(side.shape)?;
        let spans = side.spans(piece.len());
        (!spans.is_empty()).then(|| {
            spans
                .iter()
                .map(|s| (s.t1 - s.t0) * measure::edge_length(piece, s.edge))
                .sum()
        })
    }

    /// Keeps the seams right after stored edge `i` of piece `id` was split in two (the new edge
    /// is `i + 1`) at fraction `s` of its length: a point of a side on the piece or its twin
    /// that was on edge `i` is now on the part it lies in, and every later edge number moves up
    /// by one. A side end exactly at the split stays on the part the side covers.
    pub fn seams_after_split(&mut self, id: PieceId, i: usize, s: f64) {
        let Some(piece) = self.piece(id) else { return };
        let s = s.clamp(1e-9, 1.0 - 1e-9);
        let shapes = [Some(id), piece.twin.as_ref().map(|t| t.id)];
        // `below`: the side covers the stretch just before the point (its end, running the
        // stored way; its start, running the other way).
        let moved = |p: OutlinePos, below: bool| {
            if p.edge > i {
                OutlinePos::new(p.edge + 1, p.t)
            } else if p.edge < i {
                p
            } else if p.t < s || (p.t == s && below) {
                OutlinePos::new(i, p.t / s)
            } else {
                OutlinePos::new(i + 1, (p.t - s) / (1.0 - s))
            }
        };
        for seam in &mut self.seams {
            for side in [&mut seam.a, &mut seam.b] {
                if shapes.contains(&Some(side.shape)) {
                    side.from = moved(side.from, !side.forward);
                    side.to = moved(side.to, side.forward);
                }
            }
        }
        self.drop_broken();
    }

    /// Keeps the seams right after vertex `i` of piece `id` was removed from an outline of `n`
    /// edges: its edges `i - 1` and `i` became one, the first `f` of its length being the old
    /// edge `i - 1`. A point of a side on either is put where that part of the joined edge is,
    /// and later edge numbers move down by one. If the piece lost its fold (the vertex was an
    /// end of the fold edge), every seam on its pale half goes. A side left 1 mm long or less
    /// deletes its seam.
    pub fn seams_after_removal(&mut self, id: PieceId, i: usize, n: usize, f: f64) {
        let Some(piece) = self.piece(id) else { return };
        let folded = piece.fold.is_some();
        let shapes = [Some(id), piece.twin.as_ref().map(|t| t.id)];
        let prev = (i + n - 1) % n;
        let joined = |p: OutlinePos| {
            let (edge, t) = if p.edge == prev {
                (prev, p.t * f)
            } else if p.edge == i {
                (prev, if p.t == 1.0 { 1.0 } else { f + p.t * (1.0 - f) })
            } else {
                (p.edge, p.t)
            };
            OutlinePos::new(if edge > i { edge - 1 } else { edge }, t)
        };
        self.seams.retain_mut(|seam| {
            let mut keep = true;
            for side in [&mut seam.a, &mut seam.b] {
                if !shapes.contains(&Some(side.shape)) {
                    continue;
                }
                keep &= side.half == Half::Drawn || folded;
                side.from = joined(side.from);
                side.to = joined(side.to);
            }
            keep
        });
        self.drop_broken();
    }

    /// Deletes what an edit left unusable: every seam with a side 1 mm long or less
    /// ([`MIN_SIDE_MM`]), or covering nothing. A side whose ends are not on its shape's edges
    /// is left for [`Self::check`] to refuse.
    pub fn drop_broken(&mut self) {
        let too_short = |side: &SeamSide| {
            self.owner(side.shape).is_some_and(|(piece, _)| {
                let n = piece.len();
                [side.from, side.to]
                    .iter()
                    .all(|end| end.edge < n && end.t.is_finite())
                    && self.side_length(side).unwrap_or(0.0) <= MIN_SIDE_MM
            })
        };
        let short: Vec<SeamId> = self
            .seams
            .iter()
            .filter(|s| too_short(&s.a) || too_short(&s.b))
            .map(|s| s.id)
            .collect();
        self.seams.retain(|s| !short.contains(&s.id));
    }
```

- Replace `unfold_piece` (the renumbering now moves both ends):

```rust
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
        // Stored edge e is whole-piece edge m on the drawn half, and 2n - 3 - m (running the
        // other way) on the pale half.
        let renumber = |p: OutlinePos, half: Half| {
            let m = (p.edge + n - first) % n;
            match half {
                Half::Drawn => OutlinePos::new(m, p.t),
                Half::Pale => OutlinePos::new(2 * n - 3 - m, 1.0 - p.t),
            }
        };
        for seam in &mut self.seams {
            for side in [&mut seam.a, &mut seam.b] {
                if side.shape != id {
                    continue;
                }
                side.from = renumber(side.from, side.half);
                side.to = renumber(side.to, side.half);
                if side.half == Half::Pale {
                    side.forward = !side.forward;
                    side.half = Half::Drawn;
                }
            }
        }
        if let Some(stored) = self.piece_mut(id) {
            *stored = Piece { id, ..full };
        }
        true
    }
```

- Replace `check_seams` and `side_fits`:

```rust
    /// At most [`MAX_SEAMS`] seams with unique ids of at most [`MAX_SEAM_ID`]; every side fits
    /// its shape (see [`Self::side_fits`]); and no two sides, mirror images included, share more
    /// than a point of outline.
    fn check_seams(&self) -> Result<(), ModelError> {
        if self.seams.len() > MAX_SEAMS {
            return Err(ModelError::TooManySeams);
        }
        let mut ids = std::collections::BTreeSet::new();
        for s in &self.seams {
            if s.id.0 > MAX_SEAM_ID
                || !ids.insert(s.id)
                || !self.side_fits(&s.a)
                || !self.side_fits(&s.b)
            {
                return Err(ModelError::BadSeam(s.id));
            }
        }
        let mut sewn: std::collections::BTreeMap<(PieceId, bool, usize), Vec<Span>> =
            std::collections::BTreeMap::new();
        for (s, _) in self.all_seams() {
            for side in [s.a, s.b] {
                let n = self.owner(side.shape).map_or(0, |(p, _)| p.len());
                for span in side.spans(n) {
                    let on = sewn
                        .entry((side.shape, side.half == Half::Pale, span.edge))
                        .or_default();
                    if spans_overlap(on, &[span]) {
                        return Err(ModelError::BadSeam(s.id));
                    }
                    on.push(span);
                }
            }
        }
        Ok(())
    }

    /// A side fits its shape when the shape exists, both ends are on its edges (`t` within
    /// 0..=1), it covers part of the outline, it is on the pale half only of a folded piece, it
    /// covers none of the fold edge, and it is longer than [`MIN_SIDE_MM`].
    fn side_fits(&self, side: &SeamSide) -> bool {
        let Some((piece, _)) = self.owner(side.shape) else {
            return false;
        };
        let n = piece.len();
        let on_outline =
            |p: OutlinePos| p.edge < n && p.t.is_finite() && (0.0..=1.0).contains(&p.t);
        if !on_outline(side.from)
            || !on_outline(side.to)
            || (side.half == Half::Pale && piece.fold.is_none())
        {
            return false;
        }
        let spans = side.spans(n);
        !spans.is_empty()
            && piece.fold.is_none_or(|f| spans.iter().all(|s| s.edge != f))
            && self.side_length(side).is_some_and(|l| l > MIN_SIDE_MM)
    }
```

- [ ] **Step 4: Run the core tests**

Run: `cargo nextest run -p opendrape-core`
Expected: all pass.

- [ ] **Step 5: Where free sides lie on shapes (geom)**

Replace the whole of `crates/geom/src/seams.rs`. `side_edges` becomes `side_runs`, which gives stretches of outline edges in millimetres. The new functions map outline points to stored positions and back, give points between two distances along an edge, and find a side's notches. The M4a tests are kept, rewritten for `side_runs` and first/last edges, and two new tests sweep free sides over every kind of shape.

```rust
//! Where seam sides lie on the shapes that show them: which stretches of which outline edges,
//! which way, how long, as points to draw, and where their notches are; and which stored point
//! a point of a shape's outline is.

use crate::shapes::{Shape, ShapeKind};
use crate::{ACCURACY, edge_length, edge_points, edge_seg, flatten, point_at_distance};
use kurbo::{BezPath, ParamCurve, ParamCurveArclen, PathSeg};
use opendrape_core::{Half, MIN_SIDE_MM, OutlinePos, Point2, SeamSide};

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
    /// The outline edge that shows stored edge `e` of `half`, and whether it runs against the
    /// stored edge. None for the fold edge, an edge the piece doesn't have, or the pale half of
    /// a shape that isn't folded.
    pub fn outline_edge(&self, half: Half, e: usize) -> Option<(usize, bool)> {
        match (self.kind, half) {
            (ShapeKind::Folded { first, drawn, .. }, half) => {
                if e >= drawn || e == (first + drawn - 1) % drawn {
                    return None;
                }
                let m = (e + drawn - first) % drawn;
                Some(match half {
                    Half::Drawn => (m, false),
                    Half::Pale => (2 * drawn - 3 - m, true),
                })
            }
            (_, Half::Pale) => None,
            (_, Half::Drawn) => (e < self.piece.len()).then_some((e, false)),
        }
    }
    /// Which stored point the point `d` mm along outline edge `j` (from its start) shows.
    pub fn outline_pos(&self, j: usize, d: f64) -> (Half, OutlinePos) {
        let (half, e, against) = self.sew_edge(j);
        let len = edge_length(&self.piece, j);
        let f = if len > 1e-12 {
            (d / len).clamp(0.0, 1.0)
        } else {
            0.0
        };
        (half, OutlinePos::new(e, if against { 1.0 - f } else { f }))
    }
    /// Where this shape shows stored point `pos` of `half` (None as for [`Self::outline_edge`]).
    pub fn point_at(&self, half: Half, pos: OutlinePos) -> Option<Point2> {
        let (j, against) = self.outline_edge(half, pos.edge)?;
        let f = if against { 1.0 - pos.t } else { pos.t };
        Some(point_at_distance(
            &self.piece,
            j,
            f * edge_length(&self.piece, j),
        ))
    }
}

/// A stretch of one outline edge a side covers: from `from` to `to` mm along the edge (from the
/// edge's start), in the order the side runs, so `from > to` when it runs against the edge.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Run {
    pub edge: usize,
    pub from: f64,
    pub to: f64,
}

impl Run {
    pub fn length(&self) -> f64 {
        (self.to - self.from).abs()
    }
}

/// The stretches of outline `side` covers on `shape`, in the order it runs. None when the side
/// does not fit the shape (a pale half on a shape that isn't folded, an edge that doesn't
/// exist, the fold edge, or nothing covered).
pub fn side_runs(shape: &Shape, side: &SeamSide) -> Option<Vec<Run>> {
    let spans = side.spans(shape.stored_len());
    if spans.is_empty() {
        return None;
    }
    spans
        .iter()
        .map(|s| {
            let (j, against) = shape.outline_edge(side.half, s.edge)?;
            let len = edge_length(&shape.piece, j);
            // The span's ends along the outline edge, nearer its start first.
            let (near, far) = if against {
                ((1.0 - s.t1) * len, (1.0 - s.t0) * len)
            } else {
                (s.t0 * len, s.t1 * len)
            };
            // A side running the stored way runs the outline edge's way unless that is reversed.
            Some(if side.forward != against {
                Run {
                    edge: j,
                    from: near,
                    to: far,
                }
            } else {
                Run {
                    edge: j,
                    from: far,
                    to: near,
                }
            })
        })
        .collect()
}

/// Length (mm) of a side along the stitching line.
pub fn side_length(shape: &Shape, side: &SeamSide) -> Option<f64> {
    Some(side_runs(shape, side)?.iter().map(Run::length).sum())
}

/// The side as points on the shape no further than `tolerance` mm from it, from its start to
/// its end.
pub fn side_points(shape: &Shape, side: &SeamSide, tolerance: f64) -> Option<Vec<Point2>> {
    let mut out: Vec<Point2> = Vec::new();
    for run in side_runs(shape, side)? {
        let mut pts = edge_points_between(
            &shape.piece,
            run.edge,
            run.from.min(run.to),
            run.from.max(run.to),
            tolerance,
        );
        if run.from > run.to {
            pts.reverse();
        }
        // Each stretch starts where the one before it ended.
        let skip = usize::from(!out.is_empty());
        out.extend(pts.into_iter().skip(skip));
    }
    Some(out)
}

/// Edge `j` from `d0` to `d1` mm along it (from its start), as points within `tolerance` mm.
pub fn edge_points_between(
    piece: &opendrape_core::Piece,
    j: usize,
    d0: f64,
    d1: f64,
    tolerance: f64,
) -> Vec<Point2> {
    let seg = edge_seg(piece, j);
    let len = seg.arclen(ACCURACY);
    // The ends of the edge are exact, so a stretch to a corner ends on the corner.
    let param = |d: f64| {
        if d <= 0.0 || len < 1e-12 {
            0.0
        } else if d >= len {
            1.0
        } else {
            seg.inv_arclen(d, ACCURACY)
        }
    };
    let (t0, t1) = (param(d0), param(d1));
    if t0 == 0.0 && t1 == 1.0 {
        return edge_points(piece, j, tolerance);
    }
    let sub = seg.subsegment(t0..t1);
    let mut path = BezPath::new();
    path.move_to(sub.start());
    match sub {
        PathSeg::Line(l) => path.line_to(l.p1),
        PathSeg::Cubic(c) => path.curve_to(c.p1, c.p2, c.p3),
        PathSeg::Quad(q) => path.quad_to(q.p1, q.p2),
    }
    flatten(&path, tolerance)
}

/// Where the shape's notches are along `side` (mm from its start, in order): those more than
/// [`MIN_SIDE_MM`] from both of its ends. A notch at either end marks where it meets another
/// seam, not a point inside it.
pub fn side_notches(shape: &Shape, side: &SeamSide) -> Option<Vec<f64>> {
    let runs = side_runs(shape, side)?;
    let total: f64 = runs.iter().map(Run::length).sum();
    let mut out = Vec::new();
    let mut start = 0.0;
    for run in &runs {
        let (lo, hi) = (run.from.min(run.to), run.from.max(run.to));
        for notch in shape.piece.notches.iter().filter(|n| n.edge == run.edge) {
            if (lo..=hi).contains(&notch.distance) {
                let at = start + (notch.distance - run.from).abs();
                if at > MIN_SIDE_MM && at < total - MIN_SIDE_MM {
                    out.push(at);
                }
            }
        }
        start += run.length();
    }
    out.sort_by(f64::total_cmp);
    // A notch on a corner inside the side is at the end of one stretch and the start of the next.
    out.dedup_by(|a, b| (*a - *b).abs() < 1e-9);
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shapes::shapes;
    use crate::unfolded;
    use opendrape_core::{Half, Notch, Piece, PieceId, Project, SeamSide};

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
        let pale = SeamSide::edges(PieceId(1), Half::Pale, 0, 1, true);
        let run = |edge, from, to| Run { edge, from, to };
        assert_eq!(
            side_runs(front, &pale),
            Some(vec![run(5, 100.0, 0.0), run(4, 200.0, 0.0)])
        );
        let pts = side_points(front, &pale, 0.1).unwrap();
        assert_eq!(pts.len(), 3);
        close(pts[0], p(0.0, 0.0));
        close(pts[1], p(-100.0, 0.0));
        close(pts[2], p(-100.0, 200.0));
        assert!((side_length(front, &pale).unwrap() - 300.0).abs() < 1e-9);
        assert_eq!(
            side_runs(front, &pale.flipped()),
            Some(vec![run(4, 0.0, 200.0), run(5, 0.0, 100.0)])
        );
        // The fold edge (stored 3) can't be part of a side; nor can a pale half elsewhere.
        assert_eq!(
            side_runs(front, &SeamSide::edges(PieceId(1), Half::Drawn, 2, 3, true)),
            None
        );
        assert_eq!(
            side_runs(
                &all[1],
                &SeamSide::edges(PieceId(2), Half::Pale, 0, 0, true)
            ),
            None
        );
        // A twin side wraps like any other.
        let twin = SeamSide::edges(PieceId(3), Half::Drawn, 3, 0, false);
        assert_eq!(
            side_runs(&all[2], &twin),
            Some(vec![run(0, 100.0, 0.0), run(3, 200.0, 0.0)])
        );
    }

    /// A convex piece with six uneven edges, folded on edge `fold` (so a fold on any edge is
    /// valid), with a plain copy of it (id 2) to sew sides to.
    fn folded_hexagon(fold: usize) -> Project {
        let corners = [
            p(0.0, 0.0),
            p(120.0, -10.0),
            p(190.0, 60.0),
            p(160.0, 150.0),
            p(70.0, 190.0),
            p(-20.0, 100.0),
        ];
        let mut pr = Project::new();
        let mut cut = Piece::polygon(PieceId(0), "Cut", &corners);
        cut.fold = Some(fold);
        pr.add_piece(cut);
        pr.add_piece(Piece::polygon(PieceId(0), "Plain", &corners));
        assert_eq!(pr.check(), Ok(()), "fold on edge {fold}");
        pr
    }

    fn reflected_across(q: Point2, a: Point2, b: Point2) -> Point2 {
        let d = b - a;
        let t = ((q.x - a.x) * d.x + (q.y - a.y) * d.y) / (d.x * d.x + d.y * d.y);
        (a + d * t) * 2.0 - q
    }

    /// Every side the folded half can have, on every fold edge: its start and end points are
    /// the stored vertices it names (or their mirror images across the fold, on the pale half),
    /// run the way the side runs. Binds `side_edges` and `sew_edge` to the geometry, so a
    /// mapping that only works when the fold is the last edge cannot pass.
    #[test]
    fn sides_sit_on_the_stored_edges_whichever_edge_is_the_fold() {
        let n = 6;
        let mut checked = 0;
        for fold in 0..n {
            let pr = folded_hexagon(fold);
            let stored = &pr.pieces[0];
            let all = shapes(&pr);
            let shape = &all[0];
            let (near, far) = stored.edge_ends(fold);
            let vertex = |i: usize, half: Half| {
                let v = stored.vertices[i % n].pos;
                match half {
                    Half::Drawn => v,
                    Half::Pale => reflected_across(v, near, far),
                }
            };
            for half in [Half::Drawn, Half::Pale] {
                for first in 0..n {
                    for edges in 1..=n {
                        for forward in [true, false] {
                            let last = (first + edges - 1) % n;
                            let side = SeamSide::edges(PieceId(1), half, first, last, forward);
                            let got = side_points(shape, &side, 0.1);
                            if side.covers(n, fold) {
                                assert_eq!(got, None, "{side:?} crosses the fold edge {fold}");
                                continue;
                            }
                            let pts = got.unwrap_or_else(|| panic!("{side:?}, fold {fold}"));
                            let (lo, hi) = (vertex(first, half), vertex(first + edges, half));
                            let (start, end) = if forward { (lo, hi) } else { (hi, lo) };
                            close(pts[0], start);
                            close(pts[pts.len() - 1], end);
                            checked += 1;
                        }
                    }
                }
            }
            // One outline edge at a time: sew_edge names the stored edge, and the side built
            // from it covers exactly that outline edge, against its direction on the pale half.
            for j in 0..shape.piece.len() {
                let (half, i, against) = shape.sew_edge(j);
                let side = SeamSide::edges(PieceId(1), half, i, i, true);
                let len = crate::edge_length(&shape.piece, j);
                let (from, to) = if against { (len, 0.0) } else { (0.0, len) };
                assert_eq!(
                    side_runs(shape, &side),
                    Some(vec![Run { edge: j, from, to }]),
                    "outline edge {j}, fold {fold}"
                );
            }
        }
        assert!(checked > 300, "{checked} sides checked");
    }

    /// Unfolding renumbers every side and stores the mirror images; each side must end up on
    /// the same points as before, whichever edge was the fold.
    #[test]
    fn unfolding_leaves_every_side_where_it_was_whichever_edge_is_the_fold() {
        let n = 6;
        let ends = |pr: &Project| {
            let all = shapes(pr);
            let mut out: Vec<Vec<i64>> = pr
                .all_seams()
                .iter()
                .map(|(seam, _)| {
                    let mut key = Vec::new();
                    for side in [seam.a, seam.b] {
                        let shape = all.iter().find(|s| s.id == side.shape).unwrap();
                        for q in side_points(shape, &side, 0.1).unwrap() {
                            key.push((q.x * 1e6).round() as i64);
                            key.push((q.y * 1e6).round() as i64);
                        }
                        key.push(i64::MIN);
                    }
                    key
                })
                .collect();
            out.sort();
            out
        };
        let mut checked = 0;
        for fold in 0..n {
            for half in [Half::Drawn, Half::Pale] {
                for first in 0..n {
                    for edges in 1..n {
                        for forward in [true, false] {
                            // The other side: a plain piece's edge, or (below) the folded
                            // piece's own, so both sides are renumbered.
                            let last = (first + edges - 1) % n;
                            let a = SeamSide::edges(PieceId(1), half, first, last, forward);
                            let other =
                                |half, e, forward| SeamSide::edges(PieceId(1), half, e, e, forward);
                            let others = [
                                SeamSide::edges(PieceId(2), Half::Drawn, 1, 1, forward),
                                other(half.other(), (first + 3) % n, true),
                                other(half, (first + n - 1) % n, false),
                            ];
                            for b in others {
                                let mut pr = folded_hexagon(fold);
                                pr.add_seam(a, b);
                                if pr.check().is_err() {
                                    continue;
                                }
                                let before = ends(&pr);
                                let full = unfolded(&pr.pieces[0]);
                                assert!(pr.unfold_piece(PieceId(1), full));
                                assert_eq!(pr.check(), Ok(()), "{a:?} / {b:?}, fold {fold}");
                                assert_eq!(ends(&pr), before, "{a:?} / {b:?}, fold {fold}");
                                checked += 1;
                            }
                        }
                    }
                }
            }
        }
        assert!(checked > 500, "{checked} seams checked");
    }

    /// Fractions along stored edges, chosen to land inside, at and next to corners.
    const FRACTIONS: [f64; 5] = [0.0, 0.1, 0.5, 0.93, 1.0];

    #[test]
    fn free_sides_run_between_the_points_their_ends_name_on_every_kind_of_shape() {
        let n = 6;
        let mut checked = 0;
        for fold in [None, Some(2)] {
            let mut pr = folded_hexagon(fold.unwrap_or(0));
            if fold.is_none() {
                pr.pieces[0].fold = None;
                pr.add_twin(PieceId(2), "Twin".into(), p(600.0, 0.0))
                    .unwrap();
            }
            let all = shapes(&pr);
            for (shape, halves) in [
                (
                    &all[0],
                    if fold.is_some() {
                        &[Half::Drawn, Half::Pale][..]
                    } else {
                        &[Half::Drawn][..]
                    },
                ),
                (&all[all.len() - 1], &[Half::Drawn][..]),
            ] {
                for &half in halves {
                    for (e0, e1) in [(0, 0), (0, 1), (1, 4), (4, 1), (5, 0)] {
                        for &t0 in &FRACTIONS {
                            for &t1 in &FRACTIONS {
                                for forward in [true, false] {
                                    let side = SeamSide {
                                        shape: shape.id,
                                        half,
                                        from: opendrape_core::OutlinePos::new(e0, t0),
                                        to: opendrape_core::OutlinePos::new(e1, t1),
                                        forward,
                                    };
                                    let Some(points) = side_points(shape, &side, 0.1) else {
                                        let covers = |e| side.covers(n, e);
                                        assert!(
                                            side.spans(n).is_empty() || fold.is_some_and(covers),
                                            "{side:?} on {:?}",
                                            shape.kind
                                        );
                                        continue;
                                    };
                                    close(points[0], shape.point_at(half, side.from).unwrap());
                                    close(
                                        *points.last().unwrap(),
                                        shape.point_at(half, side.to).unwrap(),
                                    );
                                    let length: f64 = side
                                        .spans(n)
                                        .iter()
                                        .map(|s| {
                                            (s.t1 - s.t0)
                                                * crate::edge_length(&pr.pieces[0], s.edge)
                                        })
                                        .sum();
                                    assert!(
                                        (side_length(shape, &side).unwrap() - length).abs() < 1e-6,
                                        "{side:?}"
                                    );
                                    checked += 1;
                                }
                            }
                        }
                    }
                }
            }
        }
        assert!(checked > 400, "{checked} sides checked");
    }

    #[test]
    fn a_point_on_the_outline_names_the_stored_point_it_shows() {
        let mut pr = project();
        pr.pieces[1].edges[1] = opendrape_core::Edge::Curve {
            c1: p(450.0, 50.0),
            c2: p(380.0, 150.0),
        };
        let all = shapes(&pr);
        for shape in &all {
            for j in 0..shape.piece.len() {
                let len = crate::edge_length(&shape.piece, j);
                for f in [0.0, 0.3, 1.0] {
                    let (half, pos) = shape.outline_pos(j, f * len);
                    let on_shape = crate::point_at_distance(&shape.piece, j, f * len);
                    close(shape.point_at(half, pos).unwrap(), on_shape);
                }
            }
        }
        // The pale half's edges run against the stored ones.
        let (half, pos) = all[0].outline_pos(5, 25.0);
        assert_eq!((half, pos.edge), (Half::Pale, 0));
        assert!((pos.t - 0.75).abs() < 1e-12);
    }

    #[test]
    fn a_side_knows_the_notches_inside_it() {
        let mut pr = project();
        // The back's left edge (stored 3, running down from (300,200) to (300,0)) has notches 50
        // and 100 mm from its start, and its top edge one at 99.5 mm (0.5 mm from its end).
        pr.pieces[1].notches = vec![
            Notch::new(3, 50.0),
            Notch::new(3, 100.0),
            Notch::new(2, 99.5),
        ];
        let all = shapes(&pr);
        let back = &all[1];
        // Up the left edge, then on along the top: the notches at 150 and 100 mm from the
        // bottom-left corner; the one 0.5 mm from the top-left corner is inside too.
        let up = SeamSide::edges(PieceId(2), Half::Drawn, 2, 3, false);
        let got = side_notches(back, &up).unwrap();
        assert_eq!(got.len(), 3, "{got:?}");
        assert!((got[0] - 100.0).abs() < 1e-6 && (got[1] - 150.0).abs() < 1e-6);
        assert!((got[2] - 200.5).abs() < 1e-6);
        // A notch less than 1 mm from an end is where that side meets another, not inside it.
        let top = SeamSide::edges(PieceId(2), Half::Drawn, 2, 2, true);
        assert_eq!(side_notches(back, &top), Some(vec![]));
        // The twin has them too, at the same places along the same side.
        let twin_up = SeamSide {
            shape: PieceId(3),
            ..up
        };
        assert_eq!(side_notches(&all[2], &twin_up), Some(got));
    }

    #[test]
    fn unfolding_leaves_free_sides_where_they_were() {
        let mut checked = 0;
        for fold in 0..6 {
            for half in [Half::Drawn, Half::Pale] {
                for (t0, t1) in [(0.2, 0.7), (0.9, 0.3)] {
                    let mut pr = folded_hexagon(fold);
                    // From part-way along the edge after the fold to part-way along the one
                    // after that, sewn to part of the plain copy's edge 1.
                    let e = (fold + 1) % 6;
                    let a = SeamSide {
                        shape: PieceId(1),
                        half,
                        from: opendrape_core::OutlinePos::new(e, t0),
                        to: opendrape_core::OutlinePos::new((e + 1) % 6, t1),
                        forward: true,
                    };
                    let b = SeamSide {
                        from: opendrape_core::OutlinePos::new(1, 0.25),
                        to: opendrape_core::OutlinePos::new(1, 0.75),
                        ..SeamSide::edges(PieceId(2), Half::Drawn, 1, 1, true)
                    };
                    pr.add_seam(a, b);
                    assert_eq!(pr.check(), Ok(()));
                    let ends = |pr: &Project| {
                        let all = shapes(pr);
                        let side = pr.seams[0].a;
                        let pts = side_points(&all[0], &side, 0.1).unwrap();
                        (pts[0], *pts.last().unwrap())
                    };
                    let before = ends(&pr);
                    let full = unfolded(&pr.pieces[0]);
                    assert!(pr.unfold_piece(PieceId(1), full));
                    assert_eq!(pr.check(), Ok(()));
                    let after = ends(&pr);
                    close(before.0, after.0);
                    close(before.1, after.1);
                    checked += 1;
                }
            }
        }
        assert_eq!(checked, 24);
    }
}
```

In `crates/geom/src/lib.rs`:
- Replace `pub use seams::{side_edges, side_length, side_points};` with:

```rust
pub use seams::{Run, edge_points_between, side_length, side_notches, side_points, side_runs};
```

- Replace `split_edge_in` and `remove_vertex_in` (they pass the split's and the join's arc-length fractions on):

```rust
/// [`split_edge`] on stored piece `id` of `project`, keeping its seams sewn: a side's ends stay
/// on the same points of the outline (see `Project::seams_after_split`).
pub fn split_edge_in(project: &mut Project, id: PieceId, i: usize, t: f64) -> Option<usize> {
    let piece = project.piece_mut(id)?;
    let total = edge_length(piece, i);
    let first = distance_along(piece, i, t);
    let v = split_edge(piece, i, t)?;
    project.seams_after_split(id, i, if total > 1e-12 { first / total } else { 0.5 });
    Some(v)
}

/// [`remove_vertex`] on stored piece `id` of `project`, keeping its seams valid: side ends on the
/// two joined edges keep their share of the joined edge's length (see
/// `Project::seams_after_removal`).
pub fn remove_vertex_in(project: &mut Project, id: PieceId, i: usize) -> bool {
    let Some(piece) = project.piece_mut(id) else {
        return false;
    };
    let n = piece.len();
    if i >= n {
        return false;
    }
    let (before, after) = (edge_length(piece, piece.prev(i)), edge_length(piece, i));
    if !remove_vertex(piece, i) {
        return false;
    }
    let f = if before + after > 1e-12 {
        before / (before + after)
    } else {
        0.5
    };
    project.seams_after_removal(id, i, n, f);
    true
}
```

- In the test that calls `split_edge_in`, the `side` closure becomes:

```rust
        let side = |shape, first, last| {
            opendrape_core::SeamSide::edges(shape, opendrape_core::Half::Drawn, first, last, true)
        };
        let seam = pr.add_seam(side(a, 1, 1), side(b, 3, 3));
```

- Append to its `mod tests`:

```rust
    #[test]
    fn splitting_a_curve_keeps_free_side_ends_on_the_same_points() {
        let mut pr = Project::new();
        let mut a = square();
        a.set_curved(1, true);
        a.set_handle(1, opendrape_core::HandleEnd::Start, p(160.0, 20.0));
        let a = pr.add_piece(a);
        let b = pr.add_piece(Piece::rectangle(
            PieceId(0),
            "B",
            p(300.0, 0.0),
            100.0,
            100.0,
        ));
        let free = |shape, t0, t1| opendrape_core::SeamSide {
            shape,
            half: opendrape_core::Half::Drawn,
            from: opendrape_core::OutlinePos::new(1, t0),
            to: opendrape_core::OutlinePos::new(1, t1),
            forward: t1 > t0,
        };
        // Either side of where the curve is split (curve parameter 0.5 is not its middle by
        // length), and one end exactly at the point the split lands on.
        let seams = [
            pr.add_seam(free(a, 0.1, 0.3), free(b, 0.0, 0.2)),
            pr.add_seam(free(a, 0.8, 0.4), free(b, 0.3, 0.8)),
        ];
        let ends = |pr: &Project| -> Vec<Point2> {
            let all = shapes(pr);
            seams
                .iter()
                .flat_map(|id| {
                    let side = pr.seam(*id).unwrap().a;
                    let pts = side_points(&all[0], &side, 0.01).unwrap();
                    [pts[0], *pts.last().unwrap()]
                })
                .collect()
        };
        let before = ends(&pr);
        assert_eq!(split_edge_in(&mut pr, a, 1, 0.5), Some(2));
        assert_eq!(pr.check(), Ok(()));
        // Lengths are measured to 0.0001 mm.
        for (p, q) in before.iter().zip(ends(&pr)) {
            assert!(p.distance(q) < 1e-3, "{p:?} moved to {q:?}");
        }
        // The second seam now runs round the new point, over both parts.
        let side = pr.seam(seams[1]).unwrap().a;
        assert_eq!((side.from.edge, side.to.edge), (2, 1));
    }
```

Run: `cargo nextest run -p opendrape-geom`
Expected: all pass.

- [ ] **Step 6: Format 4 (io)**

In `crates/io/src/lib.rs`'s `from_bytes`, replace the `let mut project: Project = serde_json::from_str(…)…;` statement with:

```rust
    let mut document: serde_json::Value =
        serde_json::from_str(text).map_err(|e| OdpError::Corrupt(e.to_string()))?;
    if found <= 3 {
        upgrade_sides_from_v3(&mut document);
    }
    let mut project: Project =
        serde_json::from_value(document).map_err(|e| OdpError::Corrupt(e.to_string()))?;
```

Add after `from_bytes`:

```rust
/// Version 3 (M4a) stored a seam side as whole edges: `first_edge`, `edges` and `forward`. From
/// version 4 a side runs between two points of the outline, so each becomes the free side from
/// the start of its first edge to the end of its last (from the last's end to the first's start
/// when it runs the other way). The edge count comes from the piece the side is on (a twin has
/// its piece's). A side that names edges its piece doesn't have, or none, ends on an edge past
/// the last, so the check that follows refuses it as before.
fn upgrade_sides_from_v3(document: &mut serde_json::Value) {
    use serde_json::{Value, json};
    let mut edge_counts = std::collections::BTreeMap::new();
    for piece in document["pieces"].as_array().into_iter().flatten() {
        let n = piece["vertices"].as_array().map_or(0, Vec::len) as u64;
        edge_counts.extend(piece["id"].as_u64().map(|id| (id, n)));
        edge_counts.extend(piece["twin"]["id"].as_u64().map(|id| (id, n)));
    }
    let Some(seams) = document.get_mut("seams").and_then(Value::as_array_mut) else {
        return;
    };
    for seam in seams {
        for key in ["a", "b"] {
            let Some(side) = seam.get_mut(key).and_then(Value::as_object_mut) else {
                continue;
            };
            let (Some(first), Some(edges), Some(forward)) = (
                side.get("first_edge").and_then(Value::as_u64),
                side.get("edges").and_then(Value::as_u64),
                side.get("forward").and_then(Value::as_bool),
            ) else {
                continue;
            };
            let n = side
                .get("shape")
                .and_then(Value::as_u64)
                .and_then(|id| edge_counts.get(&id).copied())
                .unwrap_or(0);
            let last = if first < n && (1..=n).contains(&edges) {
                (first + edges - 1) % n
            } else {
                n.max(first + 1)
            };
            let (start, end) = (
                json!({"edge": first, "t": 0.0}),
                json!({"edge": last, "t": 1.0}),
            );
            let (from, to) = if forward { (start, end) } else { (end, start) };
            side.remove("first_edge");
            side.remove("edges");
            side.insert("from".into(), from);
            side.insert("to".into(), to);
        }
    }
}
```

In `check_version`'s doc comment, replace the paragraph starting "Versions 1 to 3 parse directly" with:

```rust
/// Versions 1 to 3 are read as a `serde_json::Value` first, so the seam sides of version 3 can be
/// rewritten ([`upgrade_sides_from_v3`]); the fields older versions lack take their defaults (see
/// [`upgrade_from_v1`]).
```

In `crates/io/tests/fixtures.rs`:
- Add `OutlinePos` to the `opendrape_core` import.
- In `format_v1_still_opens` and `format_v2_still_opens`, `assert_eq!(loaded.schema_version, 3, "upgraded on load");` becomes `assert_eq!(loaded.schema_version, 4, "upgraded on load");`.
- In `format_v3_still_opens`, replace the `side` closure and the expected seams with:

```rust
    // Version 3 sewed whole edges: each side is now the free side from its first edge's start
    // to its last edge's end (the other way round when it ran backwards).
    let side = |shape: u32, half: Half, first: usize, last: usize, forward: bool| {
        let (start, end) = (OutlinePos::new(first, 0.0), OutlinePos::new(last, 1.0));
        let (from, to) = if forward { (start, end) } else { (end, start) };
        SeamSide {
            shape: PieceId(shape),
            half,
            from,
            to,
            forward,
        }
    };
    expected.seams = vec![
        Seam {
            id: SeamId(1),
            a: side(1, Half::Drawn, 1, 1, true),
            b: side(2, Half::Drawn, 3, 3, false),
        },
        Seam {
            id: SeamId(2),
            a: side(2, Half::Drawn, 1, 1, true),
            b: side(3, Half::Drawn, 1, 1, true),
        },
        Seam {
            id: SeamId(5),
            a: side(4, Half::Drawn, 3, 0, true),
            b: side(1, Half::Pale, 2, 2, false),
        },
    ];
    assert_eq!(loaded, expected);
    assert_eq!(loaded.schema_version, 4, "upgraded on load");
```

In `crates/io/tests/roundtrip.rs`:
- `SeamSide::new(bodice_id, Half::Drawn, 1, 1, true)` becomes `SeamSide::edges(bodice_id, Half::Drawn, 1, 1, true)`; likewise `SeamSide::new(sleeve_id, Half::Pale, 1, 1, false)` and `SeamSide::new(sleeve_id, Half::Drawn, 0, 1, true)` become `SeamSide::edges(sleeve_id, Half::Pale, 1, 1, false)` and `SeamSide::edges(sleeve_id, Half::Drawn, 0, 0, true)`.
- `SeamSide::new(bodice_id, Half::Drawn, 3, 2, true)` becomes `SeamSide::edges(bodice_id, Half::Drawn, 3, 0, true)`.
- The "wraps" check becomes `assert!(sides.iter().any(|s| s.to.edge < s.from.edge), "wraps");`.

Run: `cargo nextest run -p opendrape-io`
Expected: all pass. The v3 fixture's sides come back as free sides between corners.

- [ ] **Step 7: Every caller of the old side**

`crates/mesh/src/boundary.rs` keeps sampling whole edges until Task 3. Replace its `let runs = geom::side_edges(shape, side)?;` with:

```rust
        // Whole edges only, as the Sew tool makes them (free sides come with their own layout).
        let runs: Vec<(usize, bool)> = geom::side_runs(shape, side)?
            .iter()
            .map(|r| (r.edge, r.from > r.to))
            .collect();
```

`crates/app/src/editor/seams.rs`, in `inset_side`, replace the head of the loop (`for (j, against) in geom::side_edges(shape, side)? {` and the `let points = …` line after it) with:

```rust
    for run in geom::side_runs(shape, side)? {
        let (j, against) = (run.edge, run.from > run.to);
        let (lo, hi) = (run.from.min(run.to), run.from.max(run.to));
        let points = geom::edge_points_between(&shape.piece, j, lo, hi, tolerance);
```

`crates/app/src/editor/panel.rs`, in the seam panel's Flip button: `s.b.forward = !s.b.forward;` becomes `s.b = s.b.flipped();` (a free side flipped keeps covering the same stretch).

`crates/app/src/editor/sew_tool.rs`: the Sew tool keeps building whole edges, as an `EdgeRun` it turns into a side.
- At the end of the module doc comment, after "from every edge ends extending.", add "Its sides are whole edges: free sides that start and end at corners."
- Add after the imports:

```rust
/// Whole stored edges of one shape: `edges` of them from `first_edge` (wrapping), running the
/// stored way when `forward`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct EdgeRun {
    pub shape: PieceId,
    pub half: Half,
    pub first_edge: usize,
    pub edges: usize,
    pub forward: bool,
}

impl EdgeRun {
    /// The seam side these edges make, on an outline of `n` edges.
    pub fn side(&self, n: usize) -> SeamSide {
        let last = (self.first_edge + self.edges - 1) % n.max(1);
        SeamSide::edges(self.shape, self.half, self.first_edge, last, self.forward)
    }

    /// The whole edges `side` covers (on an outline of `n` edges); None when it covers only
    /// part of one.
    pub fn of(side: &SeamSide, n: usize) -> Option<Self> {
        let spans = side.spans(n);
        if spans.is_empty() || spans.iter().any(|s| s.t0 != 0.0 || s.t1 != 1.0) {
            return None;
        }
        let first = if side.forward {
            spans[0].edge
        } else {
            spans[spans.len() - 1].edge
        };
        Some(Self {
            shape: side.shape,
            half: side.half,
            first_edge: first,
            edges: spans.len(),
            forward: side.forward,
        })
    }

    /// Whether it covers stored edge `e` of an outline of `n` edges.
    fn covers(&self, n: usize, e: usize) -> bool {
        n > 0 && (e % n + n - self.first_edge % n) % n < self.edges
    }
}
```

- In `SewDraft`, `pub a: SeamSide,` becomes `pub a: EdgeRun,`.
- Replace `EdgeUnder::side` with:

```rust
    /// Just this edge, starting at its end nearer the pointer.
    fn run(&self) -> EdgeRun {
        EdgeRun {
            shape: self.shape,
            half: self.half,
            first_edge: self.edge,
            edges: 1,
            forward: self.forward,
        }
    }
```

- Replace `extended`:

```rust
/// `side` (on an outline of `n` edges) with stored edge `edge` added at whichever of its ends
/// the edge is next to; None when it is next to neither, or the side is the whole outline.
pub(super) fn extended(side: EdgeRun, edge: usize, n: usize) -> Option<EdgeRun> {
    if side.edges >= n {
        return None;
    }
    if edge == (side.first_edge + side.edges) % n {
        Some(EdgeRun {
            edges: side.edges + 1,
            ..side
        })
    } else if (edge + 1) % n == side.first_edge {
        Some(EdgeRun {
            first_edge: edge,
            edges: side.edges + 1,
            ..side
        })
    } else {
        None
    }
}
```

- In `sew_tool`, where a new draft starts, `a: hit.side(),` becomes `a: hit.run(),`.
- Replace `make_seam`, `extend_second_side` and `sew_draft_side`:

```rust
    /// Sews the draft's finished first side to the clicked edge, as one undo step.
    fn make_seam(&mut self, draft: SewDraft, hit: EdgeUnder) {
        let n = |p: &Project, id| p.owner(id).map_or(0, |(piece, _)| piece.len());
        let id = self.doc.edit(|p| {
            let (a, b) = (
                draft.a.side(n(p, draft.a.shape)),
                hit.run().side(n(p, hit.shape)),
            );
            p.add_seam(a, b)
        });
        if self.doc.last_change_refused() {
            // Both edges are free (they were checked), so what refuses a seam is that its
            // mirror image would sew an edge that is already sewn; or, at the very limit, there
            // are too many seams.
            self.notice = Some(match self.doc.last_refusal() {
                Some(ModelError::BadSeam(_)) => tr!("notice-mirror-sewn"),
                _ => tr!("notice-refused"),
            });
            return;
        }
        self.canvas.sew = Some(SewDraft {
            seam: Some(id),
            ..draft
        });
        self.selection = Selection::Seam(id);
    }

    /// Adds the clicked edge to seam `id`'s second side, as one undo step.
    fn extend_second_side(&mut self, id: SeamId, hit: EdgeUnder, n: usize) {
        let grown = self.doc.edit(|p| {
            let seam = p.seam_mut(id)?;
            let same_outline = seam.b.shape == hit.shape && seam.b.half == hit.half;
            let run = EdgeRun::of(&seam.b, n)?;
            seam.b = same_outline
                .then(|| extended(run, hit.edge, n))
                .flatten()?
                .side(n);
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
            None => Some(draft.a.side(draft.outline_edges)),
            Some(id) => self.doc.project().seam(id).map(|s| s.b),
        }
    }
```

- In its `mod tests`, replace `a_side_grows_at_either_end_and_wraps` with:

```rust
    fn run(first_edge: usize, edges: usize) -> EdgeRun {
        EdgeRun {
            shape: PieceId(1),
            half: Half::Drawn,
            first_edge,
            edges,
            forward: true,
        }
    }

    #[test]
    fn a_side_grows_at_either_end_and_wraps() {
        let side = run(0, 1);
        assert_eq!(extended(side, 1, 4), Some(run(0, 2)));
        assert_eq!(extended(side, 3, 4), Some(run(3, 2)));
        assert_eq!(extended(side, 2, 4), None, "not next to it");
        assert_eq!(extended(run(0, 4), 0, 4), None, "already the whole outline");
        // As a seam side it runs from the start of its first edge to the end of its last.
        assert_eq!(
            run(3, 2).side(4),
            SeamSide::edges(PieceId(1), Half::Drawn, 3, 0, true)
        );
        assert_eq!(EdgeRun::of(&run(3, 2).side(4), 4), Some(run(3, 2)));
        let backwards = EdgeRun {
            forward: false,
            ..run(3, 2)
        };
        assert_eq!(EdgeRun::of(&backwards.side(4), 4), Some(backwards));
        let part = SeamSide {
            to: opendrape_core::OutlinePos::new(0, 0.5),
            ..run(3, 2).side(4)
        };
        assert_eq!(EdgeRun::of(&part, 4), None, "not whole edges");
    }
```

- …and replace `a_draft_is_kept_only_while_what_it_points_at_is_as_it_was` with:

```rust
    #[test]
    fn a_draft_is_kept_only_while_what_it_points_at_is_as_it_was() {
        let mut pr = Project::new();
        let a = pr.add_piece(Piece::rectangle(PieceId(0), "A", p(0.0, 0.0), 300.0, 400.0));
        let b = pr.add_piece(Piece::rectangle(
            PieceId(0),
            "B",
            p(500.0, 0.0),
            300.0,
            400.0,
        ));
        let first = SewDraft {
            a: EdgeRun {
                shape: a,
                ..run(1, 1)
            },
            seam: None,
            outline_edges: 4,
        };
        assert!(draft_fits(&pr, &first));

        let mut more = pr.clone();
        geom::split_edge_in(&mut more, a, 0, 0.5);
        assert!(
            !draft_fits(&more, &first),
            "an edge was added: edge 1 is another"
        );
        let mut fewer = pr.clone();
        assert!(geom::remove_vertex_in(&mut fewer, a, 0));
        assert!(!draft_fits(&fewer, &first), "an edge was taken away");
        let mut gone = pr.clone();
        gone.remove_piece(a);
        assert!(!draft_fits(&gone, &first), "its piece was deleted");
        let mut folded = pr.clone();
        folded.piece_mut(a).unwrap().fold = Some(1);
        assert!(!draft_fits(&folded, &first), "its edge became the fold");
        let pale = SewDraft {
            a: EdgeRun {
                shape: a,
                half: Half::Pale,
                ..run(0, 1)
            },
            ..first
        };
        assert!(!draft_fits(&pr, &pale), "no pale half without a fold");
        assert!(draft_fits(&folded, &pale));

        // Once the seam is made, the seam is what the draft needs.
        let id = pr.add_seam(
            SeamSide::edges(a, Half::Drawn, 1, 1, true),
            SeamSide::edges(b, Half::Drawn, 3, 3, false),
        );
        let extending = SewDraft {
            seam: Some(id),
            ..first
        };
        assert!(draft_fits(&pr, &extending));
        pr.remove_seam(id);
        assert!(!draft_fits(&pr, &extending), "the seam was undone");
    }
```

`crates/app/src/editor/document.rs`: an edit that leaves a seam side 1 mm long or less takes the seam with it, in the same undo step.
- In `Document::edit`, after `let result = f(&mut self.project);` add `self.project.drop_broken();`, and replace its doc comment's first sentence with "Changes the project as one undo step. What the change leaves unusable goes with it (a seam side 1 mm long or less: see [`Project::drop_broken`])."
- In `gesture_edit`, after `let result = f(&mut self.project);` add `self.project.drop_broken();`, and make its doc comment "Changes the project as part of the current drag (starting one if needed). What it leaves unusable goes, and an invalid result is refused, as in [`Self::edit`]."
- Append to its `mod tests`:

```rust
    #[test]
    fn a_drag_that_leaves_a_seam_side_too_short_deletes_the_seam_in_the_same_step() {
        use opendrape_core::{Half, OutlinePos, SeamSide};
        let mut doc = Document::default();
        let (a, b) = doc.edit(|p| (p.add_piece(rect()), p.add_piece(rect())));
        // A's bottom edge (100 mm) from 10 to 20 mm, to B's left edge.
        let part = SeamSide {
            from: OutlinePos::new(0, 0.1),
            to: OutlinePos::new(0, 0.2),
            ..SeamSide::edges(a, Half::Drawn, 0, 0, true)
        };
        doc.edit(|p| p.add_seam(part, SeamSide::edges(b, Half::Drawn, 3, 3, false)));
        let steps = |doc: &mut Document| {
            let mut n = 0;
            while doc.undo() {
                n += 1;
            }
            for _ in 0..n {
                doc.redo();
            }
            n
        };
        assert_eq!(steps(&mut doc), 2);
        // Pull A's bottom-right corner in to 5 mm: the side would be 0.5 mm long.
        doc.gesture_edit(|p| {
            p.piece_mut(a)
                .unwrap()
                .move_vertex(1, Point2::new(5.0, 0.0))
        });
        doc.end_gesture();
        assert!(!doc.last_change_refused(), "the edit is made");
        assert!(doc.project().seams.is_empty(), "and the seam goes with it");
        assert_eq!(steps(&mut doc), 3, "in the same step");
        doc.undo();
        assert_eq!(doc.project().seams.len(), 1, "undo brings both back");
    }
```

The test helpers that name whole edges (M4a tests that this change forces, listed in the File Structure). In the repo root:

```bash
perl -pi -e 's/\bside\((\w+), (Half::\w+), (\d+), 1, (true|false)\)/side($1, $2, $3, $3, $4)/g; s/SeamSide::new\((\w+), (Half::\w+), (\w+), 1, (true|false)\)/SeamSide::edges($1, $2, $3, $3, $4)/g; s/\.first_edge\)/.from.edge)/g' \
  crates/app/tests/sewing.rs crates/app/tests/ui.rs crates/app/src/sim_runner.rs \
  crates/drape/src/build.rs crates/mesh/tests/fabric.rs
```

That rewrites every single-edge side (`…, 3, 1, false)` becomes `…, 3, 3, false)`), and `.first_edge)` becomes `.from.edge)` in three sewing tests. Then by hand:
- `crates/app/tests/sewing.rs` and `crates/mesh/tests/fabric.rs`: replace the `side` helper with

```rust
/// Whole stored edges `first` to `last` of a shape, as the Sew tool makes them.
fn side(shape: PieceId, half: Half, first: usize, last: usize, forward: bool) -> SeamSide {
    SeamSide::edges(shape, half, first, last, forward)
}
```

  (in `fabric.rs` its comment is `/// Whole stored edges `first` to `last` of a shape.`).
- `crates/app/tests/sewing.rs`: in `shift_clicks_add_the_next_edges_to_either_side`, `side(back, Half::Drawn, 2, 2, false)` becomes `side(back, Half::Drawn, 2, 3, false)`; in `seams_draw_on_halves_twins_and_round_corners`, `side(front, Half::Pale, 0, 2, false)` and `side(twin, Half::Drawn, 3, 2, true)` become `side(front, Half::Pale, 0, 1, false)` and `side(twin, Half::Drawn, 3, 0, true)`.
- `crates/mesh/tests/fabric.rs`:
  - in `seam_sides_always_get_the_same_count`, `side(short, Half::Drawn, 2, 2, true)` and `side(long, Half::Drawn, 0, 2, false)` become `side(short, Half::Drawn, 2, 3, true)` and `side(long, Half::Drawn, 0, 1, false)`;
  - in `multi_edge_and_wrapping_sides_pair_start_to_start`, `side(a, Half::Drawn, first_a, 2, true)` and `side(b, Half::Drawn, first_b, 2, forward_b)` become `side(a, Half::Drawn, first_a, (first_a + 1) % 4, true)` and `side(b, Half::Drawn, first_b, (first_b + 1) % 4, forward_b)`;
  - in `random_patterns_never_panic`, the `tried.add_seam(…)` call becomes the lines below (the same random numbers in the same order):

```rust
            let (ea, fa) = ((rnd() * na as f64) as usize, rnd() < 0.5);
            let (eb, fb) = ((rnd() * nb as f64) as usize, rnd() < 0.5);
            tried.add_seam(
                side(a, Half::Drawn, ea, ea, fa),
                side(b, Half::Drawn, eb, eb, fb),
            );
```

- `crates/testkit/tests/project_skirt.rs`: the `side` closure becomes

```rust
    let side = |shape, edge, forward| SeamSide::edges(shape, Half::Drawn, edge, edge, forward);
```

- [ ] **Step 8: Run everything**

Run: `cargo nextest run --workspace`
Expected: all pass.

- [ ] **Step 9: Commit**

```bash
cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings
git add crates
git commit -m "feat(core): seam sides run between any two points of an outline (format 4)

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 2: Pins in the model, and the frozen v4 file (review: full)

**Files:**
- Create:
  - `crates/core/src/pin.rs`
  - `crates/io/tests/fixtures/v4/project.json`
  - `crates/app/tests/pins.rs`
- Modify:
  - `crates/core/src/{project.rs,measure.rs,lib.rs}`
  - `crates/geom/src/shapes.rs`
  - `crates/io/tests/{fixtures.rs,roundtrip.rs,fixtures/README.md}`
  - `crates/app/src/editor/{canvas.rs,document.rs}`

**Interfaces:**
- Consumes: Task 1's `measure` module and `Project::drop_broken`; M2b's `Piece::twin_shape`, `Twin.offset`, `Project::{break_twin, remove_piece, unfold_piece, remove_fold}`; `geom::Shape::{to_stored, from_stored}`; `MAX_PLACEMENT_M` (10 m).
- Produces (core, exported from `opendrape_core`):
  - `pub const MAX_PINS: usize = 500;`, `pub const PIN_SLACK_MM: f64 = 1.0;`
  - `Pin { pub shape: PieceId, pub half: Half, pub at: Point2, pub target: [f64; 3] }`: Copy, PartialEq, serde (`half` defaults to `Drawn`).
  - `Project.pins: Vec<Pin>` (`#[serde(default)]`).
  - `ModelError::BadPin(usize)` ("pin 2 is invalid" for index 1) and `ModelError::TooManyPins`.
  - `Project::move_pins(&mut self, id: PieceId, d: Point2)`.
  - `pub(crate) fn measure::distance_outside(&Piece, Point2) -> f64`.
- Produces (geom): `Shape::pin_spot(&self, Point2) -> (Half, Point2)` and `Shape::spot_shown(&self, Half, Point2) -> Point2`.
- Changed: `break_twin`, `remove_piece`, `unfold_piece`, `remove_fold`, `drop_broken` and `check` also handle pins.

**Behaviour:**
- **Where a pin is kept:** `at` is on the stored piece, in mm.
  - A pin on a twin is kept where its piece shows that spot (as the twin's outline is), so it moves with the piece.
  - A pin on a fold's pale half is kept as the mirror image, across the fold, of the spot pinned.
- **Edits:**
  - Break pair keeps a twin's pins where the twin shows them now: `(offset.x − x, y + offset.y)`.
  - Unfold moves pale pins to their spot on the whole piece.
  - Remove fold drops pale pins, and Delete drops a shape's pins.
  - `drop_broken` drops pins whose shape or half is gone, or that lie more than 1 mm outside their piece.
- **Dragging a whole piece** on the pattern table moves its pins and its twin's pins by the same amount, in the same gesture. Each gesture step starts from the pins as they were when the drag began, so a held-back move never leaves them behind.

- [ ] **Step 1: Failing tests**

Create `crates/core/src/pin.rs` with only its tests for now:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pins_serialise_plainly() {
        let pin = Pin {
            shape: PieceId(3),
            half: Half::Pale,
            at: Point2::new(12.5, 40.0),
            target: [0.1, 1.2, -0.05],
        };
        let json = serde_json::to_string(&pin).unwrap();
        assert_eq!(
            json,
            r#"{"shape":3,"half":"pale","at":{"x":12.5,"y":40.0},"target":[0.1,1.2,-0.05]}"#
        );
        assert_eq!(serde_json::from_str::<Pin>(&json).unwrap(), pin);
        let drawn = r#"{"shape":3,"at":{"x":12.5,"y":40.0},"target":[0.1,1.2,-0.05]}"#;
        assert_eq!(
            serde_json::from_str::<Pin>(drawn).unwrap().half,
            Half::Drawn
        );
    }
}
```

Append to `crates/core/src/measure.rs`'s `mod tests`:

```rust
    #[test]
    fn a_point_inside_is_no_distance_outside() {
        let square = Piece::rectangle(PieceId(1), "S", Point2::new(0.0, 0.0), 100.0, 100.0);
        assert_eq!(distance_outside(&square, Point2::new(50.0, 50.0)), 0.0);
        assert!((distance_outside(&square, Point2::new(103.0, 50.0)) - 3.0).abs() < 1e-12);
        assert!((distance_outside(&square, Point2::new(-0.5, 100.0)) - 0.5).abs() < 1e-12);
    }
```

Append to `crates/core/src/project.rs`'s `mod tests`:

```rust
    fn pin(shape: u32, half: Half, x: f64, y: f64) -> Pin {
        Pin {
            shape: PieceId(shape),
            half,
            at: Point2::new(x, y),
            target: [0.1, 1.2, 0.3],
        }
    }

    #[test]
    fn pins_are_checked() {
        let base = sewing_room();
        let with = |pins: Vec<Pin>| {
            let mut pr = base.clone();
            pr.pins = pins;
            pr.check()
        };
        // The front half covers (0,0)–(100,200); the back (300,0)–(400,200).
        assert_eq!(
            with(vec![
                pin(1, Half::Drawn, 50.0, 50.0),
                pin(1, Half::Pale, 99.5, 0.0),
                pin(3, Half::Drawn, 400.9, 100.0),
            ]),
            Ok(()),
            "inside, on the outline, and within 1 mm of it"
        );
        let held = |target: [f64; 3]| Pin {
            target,
            ..pin(2, Half::Drawn, 350.0, 50.0)
        };
        for (bad, why) in [
            (pin(9, Half::Drawn, 50.0, 50.0), "no such shape"),
            (pin(2, Half::Pale, 350.0, 50.0), "the back isn't folded"),
            (pin(2, Half::Drawn, 401.5, 50.0), "1.5 mm outside"),
            (pin(2, Half::Drawn, f64::NAN, 50.0), "not a number"),
            (held([0.0, 11.0, 0.0]), "held 11 m away"),
            (held([0.0, 1.0, f64::NAN]), "held at no number"),
        ] {
            assert_eq!(
                with(vec![pin(1, Half::Drawn, 50.0, 50.0), bad]),
                Err(ModelError::BadPin(1)),
                "{why}"
            );
        }
        assert_eq!(
            with(vec![pin(1, Half::Drawn, 50.0, 50.0); MAX_PINS]),
            Ok(())
        );
        assert_eq!(
            with(vec![pin(1, Half::Drawn, 50.0, 50.0); MAX_PINS + 1]),
            Err(ModelError::TooManyPins)
        );
        assert_eq!(ModelError::BadPin(1).to_string(), "pin 2 is invalid");
    }

    #[test]
    fn pins_stay_on_their_spot_of_fabric_through_edits() {
        let mut pr = sewing_room();
        pr.pins = vec![
            pin(1, Half::Pale, 20.0, 30.0),
            pin(1, Half::Drawn, 60.0, 30.0),
            pin(3, Half::Drawn, 320.0, 10.0),
            pin(4, Half::Drawn, 40.0, 440.0),
        ];
        assert_eq!(pr.check(), Ok(()));
        // The twin shows stored point (x, y) at (900 - x, y): its pin is at (580, 10) there.
        let mut broken = pr.clone();
        broken.break_twin(PieceId(2));
        assert_eq!(
            broken.pins[2].at,
            Point2::new(580.0, 10.0),
            "kept where it shows"
        );
        assert_eq!(broken.check(), Ok(()));
        // Moving the back moves the twin's pin with it (the twin stays where it is, but its pin
        // is kept where the back shows it).
        let mut moved = pr.clone();
        moved
            .piece_mut(PieceId(2))
            .unwrap()
            .translate(Point2::new(10.0, 5.0));
        moved.move_pins(PieceId(2), Point2::new(10.0, 5.0));
        assert_eq!(moved.pins[2].at, Point2::new(330.0, 15.0));
        assert_eq!(
            moved.pins[0].at,
            Point2::new(20.0, 30.0),
            "the front's stay"
        );
        let twin = moved.pieces[1].twin_shape().unwrap();
        let shown = |offset: Point2, at: Point2| Point2::new(offset.x - at.x, at.y + offset.y);
        assert_eq!(
            shown(
                moved.pieces[1].twin.as_ref().unwrap().offset,
                moved.pins[2].at
            ),
            shown(pr.pieces[1].twin.as_ref().unwrap().offset, pr.pins[2].at),
            "the twin's pin is on the same spot of the twin"
        );
        assert!(twin.check().is_ok());
        // Unfolding: the pale pin's spot is (-20, 30) on the whole piece.
        let mut unfolded = pr.clone();
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
        assert!(unfolded.unfold_piece(PieceId(1), full));
        assert_eq!(unfolded.pins[0], pin(1, Half::Drawn, -20.0, 30.0));
        assert_eq!(unfolded.check(), Ok(()));
        // Removing the fold takes its pale half's pin; deleting a piece takes its pins.
        let mut flat = pr.clone();
        flat.remove_fold(PieceId(1));
        assert_eq!(flat.pins.len(), 3);
        flat.remove_piece(PieceId(4));
        assert_eq!(
            flat.pins.iter().filter(|p| p.shape == PieceId(4)).count(),
            0
        );
        // The pocket made narrower than where its pin is (10 mm outside it now): the edit
        // deletes the pin.
        let mut shrunk = pr.clone();
        let pocket = shrunk.piece_mut(PieceId(4)).unwrap();
        pocket.move_vertex(1, Point2::new(30.0, 400.0));
        pocket.move_vertex(2, Point2::new(30.0, 480.0));
        assert_eq!(
            shrunk.check(),
            Err(ModelError::BadPin(3)),
            "refused if kept"
        );
        shrunk.drop_broken();
        assert_eq!(shrunk.pins.len(), 3);
        assert_eq!(shrunk.check(), Ok(()));
    }
```

Append to `crates/geom/src/shapes.rs`'s `mod tests`:

```rust
    #[test]
    fn a_pin_spot_is_kept_on_the_stored_piece_and_shown_where_it_was() {
        let mut pr = Project::new();
        pr.add_piece(half());
        let back = pr.add_piece(Piece::rectangle(
            PieceId(0),
            "Back",
            p(300.0, 0.0),
            100.0,
            200.0,
        ));
        pr.add_twin(back, "Back (mirror)".into(), p(900.0, 0.0))
            .unwrap();
        let all = shapes(&pr);
        // The front: its drawn half is x 0..100, its pale half x -100..0.
        assert_eq!(all[0].pin_spot(p(30.0, 50.0)), (Half::Drawn, p(30.0, 50.0)));
        assert_eq!(all[0].pin_spot(p(-30.0, 50.0)), (Half::Pale, p(30.0, 50.0)));
        // The twin shows stored (x, y) at (900 - x, y).
        assert_eq!(
            all[2].pin_spot(p(580.0, 10.0)),
            (Half::Drawn, p(320.0, 10.0))
        );
        for (shape, q) in [
            (&all[0], p(30.0, 50.0)),
            (&all[0], p(-30.0, 50.0)),
            (&all[2], p(580.0, 10.0)),
        ] {
            let (half, at) = shape.pin_spot(q);
            close(shape.spot_shown(half, at), q);
        }
    }
```

Run: `cargo nextest run -p opendrape-core -p opendrape-geom`
Expected: compile errors (no `Pin`, `pins`, `distance_outside`, `pin_spot`…).

- [ ] **Step 2: Pins in core**

Put the model above the tests in `crates/core/src/pin.rs`:

```rust
//! Pins: a spot of fabric held at a point in 3D while it drapes. A pin is stored by where it is
//! on the pattern, not by a point of the fabric mesh, so it outlives any change to the mesh.

use crate::{Half, PieceId, Point2};
use serde::{Deserialize, Serialize};

/// Most pins a project may hold.
pub const MAX_PINS: usize = 500;

/// How far (mm) outside its piece's outline a pin may be and still count as on it.
pub const PIN_SLACK_MM: f64 = 1.0;

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Pin {
    /// The piece or twin it is on.
    pub shape: PieceId,
    /// The half of a cut-on-fold piece it is on; `Drawn` for every other shape.
    #[serde(default)]
    pub half: Half,
    /// Where it is on the stored piece (mm): for a twin, where its piece shows that spot; on
    /// the pale half of a fold, the mirror image across the fold of the spot pinned.
    pub at: Point2,
    /// Where that spot of fabric is held (m, in the form's frame).
    pub target: [f64; 3],
}
```

In `crates/core/src/lib.rs`, add `mod pin;` after `mod piece;`, and after the `pub use piece::{…};` block:

```rust
pub use pin::{MAX_PINS, PIN_SLACK_MM, Pin};
```

In `crates/core/src/measure.rs`, after `edge_length`:

```rust
/// How far (mm) `p` lies outside the piece's outline: 0 inside it.
pub(crate) fn distance_outside(piece: &Piece, p: Point2) -> f64 {
    let outline: Vec<Point2> = (0..piece.len())
        .flat_map(|i| {
            let mut pts = edge_polyline(piece, i);
            pts.pop(); // the next edge starts there
            pts
        })
        .collect();
    let n = outline.len();
    let mut inside = false;
    let mut nearest = f64::INFINITY;
    for k in 0..n {
        let (a, b) = (outline[k], outline[(k + 1) % n]);
        if (a.y > p.y) != (b.y > p.y) && p.x < a.x + (p.y - a.y) / (b.y - a.y) * (b.x - a.x) {
            inside = !inside;
        }
        let ab = b - a;
        let len2 = ab.x * ab.x + ab.y * ab.y;
        let s = if len2 < 1e-18 {
            0.0
        } else {
            (((p.x - a.x) * ab.x + (p.y - a.y) * ab.y) / len2).clamp(0.0, 1.0)
        };
        nearest = nearest.min(p.distance(a + ab * s));
    }
    if inside { 0.0 } else { nearest }
}
```

In `crates/core/src/project.rs`:
- The `use crate::{…}` block becomes:

```rust
use crate::{
    Half, MAX_PINS, MAX_PLACEMENT_M, MAX_SEAM_ID, MAX_SEAMS, MIN_SIDE_MM, OutlinePos, PIN_SLACK_MM,
    Piece, PieceId, Pin, Placement, Point2, Seam, SeamId, SeamSide, Side, Span, Units, measure,
};
```

- In `struct Project`, after `pub seams: Vec<Seam>,`:

```rust
    /// Spots of fabric held in place while it drapes.
    #[serde(default)]
    pub pins: Vec<Pin>,
```

- In `Project::new`, after `seams: Vec::new(),` add `pins: Vec::new(),`.
- In `enum ModelError`, after `TooManySeams,`:

```rust
    /// The pin at this index in [`Project::pins`].
    BadPin(usize),
    TooManyPins,
```

- In its `Display`, after the `TooManySeams` arm:

```rust
            Self::BadPin(k) => write!(f, "pin {} is invalid", k + 1),
            Self::TooManyPins => write!(f, "the project has too many pins"),
```

- Replace `break_twin`:

```rust
    /// Turns `master`'s twin into an ordinary piece with the twin's current shape, id and name.
    /// A twin without a placement of its own was showing its piece's placement mirrored: it
    /// keeps that as its own, so it stays where it was. Its pins stay on the same spots: they
    /// were kept where its piece shows them, and are now kept where it shows them itself.
    pub fn break_twin(&mut self, master: PieceId) -> Option<PieceId> {
        let piece = self.piece_mut(master)?;
        let offset = piece.twin.as_ref()?.offset;
        let mut twin = piece.twin_shape()?;
        twin.placement = twin
            .placement
            .or_else(|| piece.placement.map(|p| p.mirrored()));
        piece.twin = None;
        let id = twin.id;
        self.pieces.push(twin);
        for pin in self.pins.iter_mut().filter(|p| p.shape == id) {
            pin.at = Point2::new(offset.x - pin.at.x, pin.at.y + offset.y);
        }
        Some(id)
    }
```

- In `remove_piece`, after `self.seams.retain(|s| !s.touches(id));` add `self.pins.retain(|p| p.shape != id);`, and its doc comment's first sentence becomes "Removes the piece or twin with this id and returns its shape, with every seam sewn to it and every pin on it."
- Replace `drop_broken`, `unfold_piece` and `remove_fold`, and add `move_pins` after `remove_fold`:

```rust
    /// Deletes what an edit left unusable: every seam with a side 1 mm long or less
    /// ([`MIN_SIDE_MM`]), or covering nothing (a side whose ends are not on its shape's edges is
    /// left for [`Self::check`] to refuse); and every pin on a shape or half that is gone, or
    /// more than [`PIN_SLACK_MM`] outside its piece.
    pub fn drop_broken(&mut self) {
        let pins = std::mem::take(&mut self.pins);
        self.pins = pins
            .into_iter()
            .filter(|pin| {
                self.owner(pin.shape).is_some_and(|(piece, _)| {
                    (pin.half == Half::Drawn || piece.fold.is_some())
                        && measure::distance_outside(piece, pin.at) <= PIN_SLACK_MM
                })
            })
            .collect();
        let too_short = |side: &SeamSide| {
            self.owner(side.shape).is_some_and(|(piece, _)| {
                let n = piece.len();
                [side.from, side.to]
                    .iter()
                    .all(|end| end.edge < n && end.t.is_finite())
                    && self.side_length(side).unwrap_or(0.0) <= MIN_SIDE_MM
            })
        };
        let short: Vec<SeamId> = self
            .seams
            .iter()
            .filter(|s| too_short(&s.a) || too_short(&s.b))
            .map(|s| s.id)
            .collect();
        self.seams.retain(|s| !short.contains(&s.id));
    }

    /// Unfolds cut-on-fold piece `id` into `full`, its whole outline (`geom::unfolded`), and
    /// keeps its seams: the mirror images of seams on the piece become stored seams (on the
    /// pale half they were drawn on), and every side on the piece is renumbered for the whole
    /// outline. Pins on the pale half move to where the whole piece has them. False (and
    /// nothing changed) when there is no such folded piece.
    pub fn unfold_piece(&mut self, id: PieceId, full: Piece) -> bool {
        let Some(piece) = self.piece(id) else {
            return false;
        };
        let Some(fold) = piece.fold else {
            return false;
        };
        let n = piece.len();
        let first = (fold + 1) % n;
        let (near, far) = piece.edge_ends(fold);
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
        // Stored edge e is whole-piece edge m on the drawn half, and 2n - 3 - m (running the
        // other way) on the pale half.
        let renumber = |p: OutlinePos, half: Half| {
            let m = (p.edge + n - first) % n;
            match half {
                Half::Drawn => OutlinePos::new(m, p.t),
                Half::Pale => OutlinePos::new(2 * n - 3 - m, 1.0 - p.t),
            }
        };
        for seam in &mut self.seams {
            for side in [&mut seam.a, &mut seam.b] {
                if side.shape != id {
                    continue;
                }
                side.from = renumber(side.from, side.half);
                side.to = renumber(side.to, side.half);
                if side.half == Half::Pale {
                    side.forward = !side.forward;
                    side.half = Half::Drawn;
                }
            }
        }
        // A pin on the pale half was kept as its mirror image: the whole piece has the spot.
        for pin in self.pins.iter_mut() {
            if pin.shape == id && pin.half == Half::Pale {
                pin.at = reflect_across(pin.at, near, far);
                pin.half = Half::Drawn;
            }
        }
        if let Some(stored) = self.piece_mut(id) {
            *stored = Piece { id, ..full };
        }
        true
    }

    /// Takes the fold off piece `id`: its pale half goes, and so does every seam and pin on it.
    /// The mirror images of its other seams simply disappear. False when it has no fold.
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
        self.pins
            .retain(|p| !(p.shape == id && p.half == Half::Pale));
        true
    }

    /// Moves the pins on stored piece `id` and on its twin by `d` (mm), for when the piece has
    /// been moved by `d` on the pattern table: its twin stays where it is, but its pins are
    /// kept where the piece shows them, so they move with it too.
    pub fn move_pins(&mut self, id: PieceId, d: Point2) {
        let twin = self.piece(id).and_then(|p| p.twin.as_ref()).map(|t| t.id);
        for pin in &mut self.pins {
            if pin.shape == id || Some(pin.shape) == twin {
                pin.at = pin.at + d;
            }
        }
    }
```

- At the end of `check`, `self.check_seams()` becomes `self.check_seams()?;` followed by `self.check_pins()`, and add before `check_seams`:

```rust
    /// At most [`MAX_PINS`] pins, each on a shape (and half) that exists, within
    /// [`PIN_SLACK_MM`] of its piece, and held at a target of finite numbers within
    /// [`MAX_PLACEMENT_M`] of the origin.
    fn check_pins(&self) -> Result<(), ModelError> {
        if self.pins.len() > MAX_PINS {
            return Err(ModelError::TooManyPins);
        }
        for (k, pin) in self.pins.iter().enumerate() {
            let on_piece = self.owner(pin.shape).is_some_and(|(piece, _)| {
                (pin.half == Half::Drawn || piece.fold.is_some())
                    && pin.at.is_finite()
                    && measure::distance_outside(piece, pin.at) <= PIN_SLACK_MM
            });
            let target_ok = pin.target.iter().all(|v| v.is_finite())
                && pin.target.iter().map(|v| v * v).sum::<f64>().sqrt() <= MAX_PLACEMENT_M;
            if !on_piece || !target_ok {
                return Err(ModelError::BadPin(k));
            }
        }
        Ok(())
    }
```

- After `impl Project { … }` (before `#[cfg(test)]`):

```rust
/// Mirror image of `p` across the line through `a` and `b`.
fn reflect_across(p: Point2, a: Point2, b: Point2) -> Point2 {
    let d = b - a;
    let t = ((p.x - a.x) * d.x + (p.y - a.y) * d.y) / (d.x * d.x + d.y * d.y);
    (a + d * t) * 2.0 - p
}
```

In `crates/geom/src/shapes.rs`, add `Half` to the `opendrape_core` import, and after `Shape::from_stored`:

```rust
    /// The stored spot that point `p` of this shape shows, and the half it is on: where a pin
    /// at `p` is kept. A point on the pale side of a fold line is kept as its mirror image.
    pub fn pin_spot(&self, p: Point2) -> (Half, Point2) {
        match self.kind {
            ShapeKind::Folded {
                drawn,
                fold: (near, far),
                ..
            } => {
                let d = far - near;
                let side = |q: Point2| d.x * (q.y - near.y) - d.y * (q.x - near.x);
                let stored = self.piece.vertices[..drawn]
                    .iter()
                    .map(|v| side(v.pos))
                    .max_by(|a, b| a.abs().total_cmp(&b.abs()))
                    .unwrap_or(0.0);
                if side(p) * stored < 0.0 {
                    (Half::Pale, reflect_across(p, near, far))
                } else {
                    (Half::Drawn, p)
                }
            }
            _ => (Half::Drawn, self.to_stored(p)),
        }
    }

    /// Where this shape shows stored spot `at` of `half` (see [`Self::pin_spot`]).
    pub fn spot_shown(&self, half: Half, at: Point2) -> Point2 {
        match (self.kind, half) {
            (
                ShapeKind::Folded {
                    fold: (near, far), ..
                },
                Half::Pale,
            ) => reflect_across(at, near, far),
            _ => self.from_stored(at),
        }
    }
```

Run: `cargo nextest run -p opendrape-core -p opendrape-geom`
Expected: all pass.

- [ ] **Step 3: The frozen v4 file**

Create `crates/io/tests/fixtures/v4/project.json`. It has a folded front with a curved armhole, a sleeve with a curved cap and its twin, a whole-edge seam, two free seams (half the cap to the armhole, part of the front's pale hem to the twin), and pins on a drawn half, a pale half and a twin:

```json
{
  "schema_version": 4,
  "units": "cm",
  "pieces": [
    {
      "id": 1,
      "name": "Front",
      "vertices": [
        { "pos": { "x": 0.0, "y": 0.0 }, "kind": "corner" },
        { "pos": { "x": 200.0, "y": 0.0 }, "kind": "corner" },
        { "pos": { "x": 200.0, "y": 350.0 }, "kind": "corner" },
        { "pos": { "x": 150.0, "y": 450.0 }, "kind": "corner" },
        { "pos": { "x": 0.0, "y": 450.0 }, "kind": "corner" }
      ],
      "edges": [
        { "type": "line" },
        { "type": "line" },
        { "type": "curve", "c1": { "x": 190.0, "y": 400.0 }, "c2": { "x": 170.0, "y": 440.0 } },
        { "type": "line" },
        { "type": "line" }
      ],
      "grain_deg": 90.0,
      "allowance": 10.0,
      "edge_props": [
        { "allowance": null, "hem": true },
        { "allowance": null, "hem": false },
        { "allowance": null, "hem": false },
        { "allowance": null, "hem": false },
        { "allowance": null, "hem": false }
      ],
      "notches": [{ "edge": 2, "distance": 30.0, "marks": 1, "style": "slit" }],
      "lines": [],
      "fold": 4,
      "twin": null,
      "placement": { "position": [0.0, 1.1, 0.2], "rotation": [0.0, 0.0, 0.0, 1.0], "curve": 0.2 }
    },
    {
      "id": 2,
      "name": "Sleeve",
      "vertices": [
        { "pos": { "x": 400.0, "y": 0.0 }, "kind": "corner" },
        { "pos": { "x": 700.0, "y": 0.0 }, "kind": "corner" },
        { "pos": { "x": 700.0, "y": 200.0 }, "kind": "corner" },
        { "pos": { "x": 400.0, "y": 200.0 }, "kind": "corner" }
      ],
      "edges": [
        { "type": "line" },
        { "type": "line" },
        { "type": "curve", "c1": { "x": 650.0, "y": 320.0 }, "c2": { "x": 450.0, "y": 320.0 } },
        { "type": "line" }
      ],
      "grain_deg": 90.0,
      "allowance": 10.0,
      "edge_props": [
        { "allowance": null, "hem": true },
        { "allowance": null, "hem": false },
        { "allowance": null, "hem": false },
        { "allowance": null, "hem": false }
      ],
      "notches": [{ "edge": 2, "distance": 180.0, "marks": 1, "style": "slit" }],
      "lines": [],
      "fold": null,
      "twin": {
        "id": 3,
        "name": "Sleeve (mirror)",
        "offset": { "x": 1500.0, "y": 0.0 },
        "placement": null
      },
      "placement": {
        "position": [0.25, 1.15, 0.0],
        "rotation": [0.0, 0.0, 0.3826834323650898, 0.9238795325112867],
        "curve": 0.08
      }
    }
  ],
  "seams": [
    {
      "id": 1,
      "a": { "shape": 1, "half": "drawn", "from": { "edge": 1, "t": 0.0 }, "to": { "edge": 1, "t": 1.0 }, "forward": true },
      "b": { "shape": 2, "half": "drawn", "from": { "edge": 1, "t": 1.0 }, "to": { "edge": 1, "t": 0.0 }, "forward": false }
    },
    {
      "id": 2,
      "a": { "shape": 2, "half": "drawn", "from": { "edge": 2, "t": 0.0 }, "to": { "edge": 2, "t": 0.5 }, "forward": true },
      "b": { "shape": 1, "half": "drawn", "from": { "edge": 2, "t": 0.0 }, "to": { "edge": 2, "t": 1.0 }, "forward": true }
    },
    {
      "id": 3,
      "a": { "shape": 1, "half": "pale", "from": { "edge": 0, "t": 0.25 }, "to": { "edge": 0, "t": 0.75 }, "forward": true },
      "b": { "shape": 3, "half": "drawn", "from": { "edge": 0, "t": 1.0 }, "to": { "edge": 0, "t": 0.0 }, "forward": false }
    }
  ],
  "pins": [
    { "shape": 1, "half": "drawn", "at": { "x": 100.0, "y": 100.0 }, "target": [0.05, 1.0, 0.25] },
    { "shape": 1, "half": "pale", "at": { "x": 50.0, "y": 200.0 }, "target": [-0.05, 1.0, 0.25] },
    { "shape": 3, "at": { "x": 600.0, "y": 100.0 }, "target": [-0.3, 1.1, 0.0] }
  ],
  "next_piece_id": 4
}
```

In `crates/io/tests/fixtures/README.md`, the folder list becomes:

```markdown
Folders: v1 (M2a), v2 (M2b: seam allowance, notches, internal lines, fold, twin), v3 (M4a:
seams, 3D placements), v4 (M4b: seam sides between any two points of an outline, pins).
```

In `crates/io/tests/fixtures.rs`, add `Pin` to the `opendrape_core` import, and append:

```rust
#[test]
fn format_v4_still_opens() {
    let loaded = opendrape_io::from_bytes(&odp(include_str!("fixtures/v4/project.json")))
        .expect("the frozen v4 project opens");
    let props = |hem: bool, n: usize| {
        let mut p = vec![EdgeProps::default(); n];
        p[0].hem = hem;
        p
    };
    let front = Piece {
        id: PieceId(1),
        name: "Front".into(),
        vertices: vec![
            corner(0.0, 0.0),
            corner(200.0, 0.0),
            corner(200.0, 350.0),
            corner(150.0, 450.0),
            corner(0.0, 450.0),
        ],
        edges: vec![
            Edge::Line,
            Edge::Line,
            Edge::Curve {
                c1: at(190.0, 400.0),
                c2: at(170.0, 440.0),
            },
            Edge::Line,
            Edge::Line,
        ],
        grain_deg: 90.0,
        allowance: 10.0,
        edge_props: props(true, 5),
        notches: vec![Notch::new(2, 30.0)],
        lines: vec![],
        fold: Some(4),
        twin: None,
        placement: Some(Placement {
            position: [0.0, 1.1, 0.2],
            rotation: [0.0, 0.0, 0.0, 1.0],
            curve: Some(0.2),
        }),
    };
    let sleeve = Piece {
        id: PieceId(2),
        name: "Sleeve".into(),
        vertices: vec![
            corner(400.0, 0.0),
            corner(700.0, 0.0),
            corner(700.0, 200.0),
            corner(400.0, 200.0),
        ],
        edges: vec![
            Edge::Line,
            Edge::Line,
            Edge::Curve {
                c1: at(650.0, 320.0),
                c2: at(450.0, 320.0),
            },
            Edge::Line,
        ],
        grain_deg: 90.0,
        allowance: 10.0,
        edge_props: props(true, 4),
        notches: vec![Notch::new(2, 180.0)],
        lines: vec![],
        fold: None,
        twin: None, // set below, once its id is taken
        placement: Some(Placement {
            position: [0.25, 1.15, 0.0],
            rotation: [0.0, 0.0, 0.382_683_432_365_089_8, 0.923_879_532_511_286_7],
            curve: Some(0.08),
        }),
    };
    let mut expected = Project::new();
    expected.add_piece(front);
    let sleeve = expected.add_piece(sleeve);
    expected
        .add_twin(sleeve, "Sleeve (mirror)".into(), at(1500.0, 0.0))
        .unwrap();
    let side =
        |shape: u32, half: Half, from: (usize, f64), to: (usize, f64), forward: bool| SeamSide {
            shape: PieceId(shape),
            half,
            from: OutlinePos::new(from.0, from.1),
            to: OutlinePos::new(to.0, to.1),
            forward,
        };
    expected.seams = vec![
        Seam {
            id: SeamId(1),
            a: side(1, Half::Drawn, (1, 0.0), (1, 1.0), true),
            b: side(2, Half::Drawn, (1, 1.0), (1, 0.0), false),
        },
        Seam {
            id: SeamId(2),
            a: side(2, Half::Drawn, (2, 0.0), (2, 0.5), true),
            b: side(1, Half::Drawn, (2, 0.0), (2, 1.0), true),
        },
        Seam {
            id: SeamId(3),
            a: side(1, Half::Pale, (0, 0.25), (0, 0.75), true),
            b: side(3, Half::Drawn, (0, 1.0), (0, 0.0), false),
        },
    ];
    let pin = |shape: u32, half: Half, x: f64, y: f64, target: [f64; 3]| Pin {
        shape: PieceId(shape),
        half,
        at: at(x, y),
        target,
    };
    expected.pins = vec![
        pin(1, Half::Drawn, 100.0, 100.0, [0.05, 1.0, 0.25]),
        pin(1, Half::Pale, 50.0, 200.0, [-0.05, 1.0, 0.25]),
        pin(3, Half::Drawn, 600.0, 100.0, [-0.3, 1.1, 0.0]),
    ];
    assert_eq!(loaded, expected);
    assert_eq!(loaded.schema_version, 4);
    // Each of the three seams has a mirror image.
    assert_eq!(loaded.all_seams().len(), 6);
}

#[test]
fn refuses_invalid_v4_details() {
    let good = include_str!("fixtures/v4/project.json");
    for (from, to) in [
        (
            r#""to": { "edge": 2, "t": 0.5 }"#,
            r#""to": { "edge": 2, "t": 1.5 }"#,
        ), // past the end of the cap
        (
            r#""from": { "edge": 0, "t": 0.25 }, "to": { "edge": 0, "t": 0.75 }"#,
            r#""from": { "edge": 0, "t": 0.25 }, "to": { "edge": 0, "t": 0.251 }"#,
        ), // 0.05 mm long
        (
            r#""shape": 1, "half": "pale", "from": { "edge": 0, "t": 0.25 }"#,
            r#""shape": 1, "half": "pale", "from": { "edge": 1, "t": 0.25 }"#,
        ), // round to the pale side edge, which the first seam's mirror image sews
        (
            r#""at": { "x": 600.0, "y": 100.0 }"#,
            r#""at": { "x": 720.0, "y": 100.0 }"#,
        ), // 20 mm outside the sleeve
        (
            r#""target": [-0.3, 1.1, 0.0]"#,
            r#""target": [-0.3, 11.1, 0.0]"#,
        ), // held 11 m up
        (r#""shape": 3, "at""#, r#""shape": 3, "half": "pale", "at""#), // the twin has no pale half
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

In `crates/io/tests/roundtrip.rs`:
- The module comment and import become:

```rust
//! Everything M2b, M4a and M4b added to a project (seam allowances, hems, notches, internal
//! lines, folds, mirrored pairs, seams, 3D placements, free seam sides and pins) must survive
//! saving and opening exactly. The
//! project is built field by field here, with no default left where a value could be lost
//! without anyone noticing.

use opendrape_core::{
    Edge, EdgeProps, Half, InternalLine, LineKind, Notch, NotchStyle, OutlinePos, Piece, PieceId,
    Pin, Placement, Point2, Project, SeamSide, Units, Vertex, VertexKind,
};
```

- In `detailed_project`, after the second `project.add_seam(…);`:

```rust
    // Part of the bodice's top, run backwards, to most of the sleeve's top.
    project.add_seam(
        SeamSide {
            from: OutlinePos::new(2, 0.7),
            to: OutlinePos::new(2, 0.2),
            ..SeamSide::edges(bodice_id, Half::Drawn, 2, 2, false)
        },
        SeamSide {
            from: OutlinePos::new(2, 0.1),
            to: OutlinePos::new(2, 0.9),
            ..SeamSide::edges(sleeve_id, Half::Drawn, 2, 2, true)
        },
    );
    // Pins on the bodice, the sleeve's pale half and the bodice's mirror image.
    project.pins = vec![
        Pin {
            shape: bodice_id,
            half: Half::Drawn,
            at: at(200.5, 300.25),
            target: [0.12, 1.31, 0.42],
        },
        Pin {
            shape: sleeve_id,
            half: Half::Pale,
            at: at(1550.0, 250.0),
            target: [0.4, 1.1, -0.05],
        },
        Pin {
            shape: twin,
            half: Half::Drawn,
            at: at(100.0, 500.0),
            target: [-0.2, 1.2, 0.3],
        },
    ];
```

- Rename `every_m2b_and_m4a_field_survives_a_save_and_open` to `every_m2b_m4a_and_m4b_field_survives_a_save_and_open`, and replace its last assertion (`project.all_seams().len()` is 4), to the end of the test, with:

```rust
    assert_eq!(
        project.all_seams().len(),
        6,
        "every seam has a mirror image"
    );
    // M4b: a side ending part-way along an edge, and pins on a piece, a pale half and a twin.
    assert!(
        sides
            .iter()
            .any(|s| ![0.0, 1.0].contains(&s.from.t) && ![0.0, 1.0].contains(&s.to.t))
    );
    let pinned: Vec<(PieceId, Half)> = project.pins.iter().map(|p| (p.shape, p.half)).collect();
    assert_eq!(
        pinned,
        vec![
            (PieceId(1), Half::Drawn),
            (PieceId(3), Half::Pale),
            (PieceId(2), Half::Drawn)
        ]
    );
}
```

Run: `cargo nextest run -p opendrape-io`
Expected: all pass. Every refusal in `refuses_invalid_v4_details` is `Invalid`.

- [ ] **Step 4: Pins move with a dragged piece**

Create `crates/app/tests/pins.rs`:

```rust
//! M4b pins on the pattern table: they stay on their spot of fabric when the piece moves, and
//! they are drawn, selected and removed there.

mod common;
use common::*;
use opendrape_core::{Half, Piece, PieceId, Pin, Point2};

fn pin(shape: PieceId, x: f64, y: f64) -> Pin {
    Pin {
        shape,
        half: Half::Drawn,
        at: Point2::new(x, y),
        target: [0.0, 1.0, 0.3],
    }
}

#[test]
fn moving_a_piece_takes_its_pins_and_its_twins_pins_along() {
    let mut h = harness();
    let front = with_rectangle(&mut h); // (100,100)–(400,500)
    let twin = h
        .state_mut()
        .doc
        .edit(|p| p.add_twin(front, "Front (mirror)".into(), Point2::new(1000.0, 0.0)))
        .unwrap(); // shows stored (x, y) at (1000 - x, y)
    let other = h.state_mut().doc.edit(|p| {
        p.add_piece(Piece::rectangle(
            PieceId(0),
            "Other",
            Point2::new(1100.0, 100.0),
            100.0,
            100.0,
        ))
    });
    h.state_mut().doc.edit(|p| {
        p.pins = vec![
            pin(front, 200.0, 200.0),
            pin(twin, 300.0, 400.0),
            pin(other, 1150.0, 150.0),
        ]
    });
    h.state_mut().fit();
    h.run();
    drag(&mut h, (250.0, 300.0), (350.0, 350.0)); // the front, by about (100, 50)
    // Exactly as far as the front moved.
    let d = piece_of(&h, front).vertices[0].pos - Point2::new(100.0, 100.0);
    close(d, Point2::new(100.0, 50.0));
    let pins = h.state().doc.project().pins.clone();
    assert_eq!(
        pins[0].at,
        Point2::new(200.0, 200.0) + d,
        "the same spot of the front"
    );
    assert_eq!(
        pins[1].at,
        Point2::new(300.0, 400.0) + d,
        "kept where the front shows it"
    );
    assert_eq!(
        pins[2].at,
        Point2::new(1150.0, 150.0),
        "another piece's stays"
    );
    // The twin stayed where it was, and so did its pin's spot on it: (1000 - 300, 400).
    let shapes = opendrape_geom::shapes(h.state().doc.project());
    let twin_shape = shapes.iter().find(|s| s.id == twin).unwrap();
    close(
        twin_shape.spot_shown(Half::Drawn, pins[1].at),
        Point2::new(700.0, 400.0),
    );
    cmd(&mut h, egui::Key::Z);
    assert_eq!(
        h.state().doc.project().pins[0].at,
        Point2::new(200.0, 200.0),
        "one step"
    );
}
```

Run: `cargo nextest run -p opendrape --test pins`
Expected: FAIL (the pins stay where they were).

In `crates/app/src/editor/canvas.rs`:
- Add `Pin` to the `opendrape_core` import.
- In `struct Drag`, after `last_accepted: Piece,`:

```rust
    /// The project's pins when the drag began: a whole piece that moves takes its pins along.
    pins: Vec<Pin>,
```

- At the top of `impl Drag`:

```rust
    /// Whether the drag moves a whole stored piece (not a twin, which moves on its own).
    fn moves_whole_piece(&self) -> bool {
        matches!(self.hit, Hit::Inside(_)) && !matches!(self.kind, geom::ShapeKind::Twin { .. })
    }
```

- Where `edit_tool` starts a drag (`self.canvas.drag = Some(Drag { … })`), after `line_was_inside,` add `pins: self.doc.project().pins.clone(),`.
- Where it applies the drag, replace from `let accepted = moved.clone();` to the end of the `self.doc.gesture_edit(…);` call with:

```rust
            let accepted = moved.clone();
            let pins = drag
                .moves_whole_piece()
                .then(|| (drag.pins.clone(), now - drag.grab));
            self.doc.gesture_edit(|p| {
                if let Some(piece) = p.piece_mut(id) {
                    *piece = moved;
                }
                if let Some((pins, d)) = pins {
                    p.pins = pins;
                    p.move_pins(id, d);
                }
            });
```

In `crates/app/src/editor/document.rs`, `Document::edit`'s doc comment now names pins too: "(a seam side 1 mm long or less, a pin off its piece: see [`Project::drop_broken`])".

Run: `cargo nextest run -p opendrape --test pins`
Expected: PASS.

- [ ] **Step 5: Run everything and commit**

```bash
cargo nextest run --workspace
cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings
git add crates
git commit -m "feat(core): pins hold spots of fabric; format 4 frozen

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 3: Free sides and notches in the fabric (review: full)

**Files:**
- Modify:
  - `crates/mesh/src/boundary.rs` (rewritten)
  - `crates/mesh/src/lib.rs`
  - `crates/mesh/tests/fabric.rs`

**Interfaces:**
- Consumes: Task 1's `geom::{side_runs, side_notches, Run}`, `SeamSide::spans`; M4a's `boundary::outline`, `panel`, `mesh_shapes`, `seam_plans`.
- Produces (crate-private; `build(&Project, &MeshParams) -> GarmentMesh` is unchanged):
  - `boundary::outline(shape: &Shape, sides: &[(SeamSide, Vec<f64>)], h: f64) -> Option<Outline>`: each side comes with its samples as distances from its start. It replaces the step count.
  - `boundary::{locate(&[Run], d) -> (usize, f64), gaps(&mut [(f64, f64)], len, tiny) -> Vec<(f64, f64)>}`.
  - `SidePlan { length, notches, corners }` and `SeamPlan { seam, shape_a, shape_b, a: SidePlan, b: SidePlan, mirrored }`, with `SeamPlan::layout(&self, h) -> (Vec<f64>, Vec<f64>)`.
  - `fn samples(breaks: &[f64], steps: &[usize], corners: &[f64]) -> Vec<f64>`.

**Behaviour:**
- **Sampling free sides:**
  - A side's samples go where its layout says, across corners and past the last edge.
  - A corner inside a side takes over the nearest sample of its stretch (never a stretch's ends), as in M4a.
  - Each stretch of an edge that no side covers gets points about `h` apart.
  - Samples within 1e-9 of the edge length of each other are one point, so two sides that meet part-way along an edge share the point where they meet.
- **Notch-matched layout:**
  - When both sides have as many notches (from `side_notches`), the k-th notch of each starts a stretch. Each pair of stretches gets `max(len_a, len_b) / h` steps (rounded, at least 1).
  - Otherwise each side is one stretch: M4a's even layout.
  - The number of stitches is the sum of the steps plus one.
- **The M4a fabric tests keep their meaning:** a whole-edge side is a free side between corners, laid out as before.

- [ ] **Step 1: Failing tests**

In `crates/mesh/tests/fabric.rs`, the imports become:

```rust
use opendrape_core::{
    Edge, Half, InternalLine, LineKind, Notch, OutlinePos, Piece, PieceId, Point2, Project,
    SeamSide,
};
use opendrape_geom as geom;
use opendrape_mesh::{GarmentMesh, MeshNote, MeshParams, PanelMesh, Stitch, build};
```

and append:

```rust
/// The stretch of a shape's outline from fraction `from.1` of stored edge `from.0` to fraction
/// `to.1` of edge `to.0`.
fn part(shape: PieceId, from: (usize, f64), to: (usize, f64), forward: bool) -> SeamSide {
    SeamSide {
        shape,
        half: Half::Drawn,
        from: OutlinePos::new(from.0, from.1),
        to: OutlinePos::new(to.0, to.1),
        forward,
    }
}

/// Where a stitched point is on the pattern table (mm).
fn at_mm(mesh: &GarmentMesh, (panel, i): (usize, u32)) -> Point2 {
    let f = mesh.panels[panel].flat[i as usize];
    p(f[0] * 1000.0, f[1] * 1000.0)
}

#[test]
fn a_free_side_is_sampled_across_a_corner_and_the_rest_of_its_edges_too() {
    let mut pr = Project::new();
    let a = pr.add_piece(Piece::rectangle(PieceId(0), "A", p(0.0, 0.0), 200.0, 200.0));
    let b = pr.add_piece(Piece::rectangle(
        PieceId(0),
        "B",
        p(400.0, 0.0),
        200.0,
        200.0,
    ));
    // From halfway along A's bottom, round its bottom-right corner, halfway up its right edge;
    // to B's left edge, running up from its bottom.
    pr.add_seam(
        part(a, (0, 0.5), (1, 0.5), true),
        side(b, Half::Drawn, 3, 3, false),
    );
    assert_eq!(pr.check(), Ok(()));
    let mesh = build(&pr, &MeshParams::default());
    assert_eq!(mesh.notes, vec![]);
    let steps = (200.0_f64 / H).round() as usize;
    assert_eq!(mesh.stitches.len(), steps + 1);
    let side_a = [p(100.0, 0.0), p(200.0, 0.0), p(200.0, 100.0)];
    let side_b = [p(400.0, 0.0), p(400.0, 200.0)];
    let mut corner_taken = false;
    for (k, &(sa, sb)) in mesh.stitches.iter().enumerate() {
        let (qa, qb) = (at_mm(&mesh, sa), at_mm(&mesh, sb));
        let want = 200.0 * k as f64 / steps as f64;
        let half_step = 0.5 * 200.0 / steps as f64;
        assert!(
            (arc_along(&side_a, qa) - want).abs() <= half_step + 1e-6,
            "{k}: {qa:?}"
        );
        assert!((arc_along(&side_b, qb) - want).abs() < 1e-6, "{k}: {qb:?}");
        corner_taken |= qa.distance(p(200.0, 0.0)) < 1e-9;
    }
    assert!(
        corner_taken,
        "the corner inside the side is one of its samples"
    );
    // The halves of A's bottom and right edges that nobody sews get points about h apart too:
    // the bottom edge from x = 0 to 100, the right edge from y = 100 to 200.
    let panel = &mesh.panels[mesh.panel_of(a).unwrap()];
    let mm = |i: u32| {
        p(
            panel.flat[i as usize][0] * 1000.0,
            panel.flat[i as usize][1] * 1000.0,
        )
    };
    for (edge, free) in [(0, 0.0..=100.0), (1, 100.0..=200.0)] {
        let points: Vec<Point2> = panel.edges[edge]
            .iter()
            .map(|&i| mm(i))
            .filter(|q| free.contains(&if edge == 0 { q.x } else { q.y }))
            .collect();
        assert!(points.len() >= 9, "edge {edge}: {points:?}");
        for w in points.windows(2) {
            let d = w[0].distance(w[1]);
            assert!(
                d > 0.5 * H && d < 1.6 * H,
                "edge {edge}: {d} mm between points"
            );
        }
    }
    let area: f64 = triangles_mm(panel).into_iter().map(area).sum();
    assert!((area - 40_000.0).abs() < 1.0, "{area}");
}

/// A 300 mm bottom edge with notches `on_a` (mm from its start) sewn, start to start, to a
/// 200 mm bottom edge with notches `on_b`.
fn notched_seam(on_a: &[f64], on_b: &[f64]) -> (Project, GarmentMesh) {
    let mut pr = Project::new();
    let mut a = Piece::rectangle(PieceId(0), "A", p(0.0, 0.0), 300.0, 100.0);
    a.notches = on_a.iter().map(|d| Notch::new(0, *d)).collect();
    let mut b = Piece::rectangle(PieceId(0), "B", p(400.0, 0.0), 200.0, 100.0);
    b.notches = on_b.iter().map(|d| Notch::new(0, *d)).collect();
    let (a, b) = (pr.add_piece(a), pr.add_piece(b));
    pr.add_seam(
        side(a, Half::Drawn, 0, 0, true),
        side(b, Half::Drawn, 0, 0, true),
    );
    assert_eq!(pr.check(), Ok(()));
    let mesh = build(&pr, &MeshParams::default());
    (pr, mesh)
}

#[test]
fn notches_paired_across_a_seam_land_on_the_same_stitch() {
    let (_, mesh) = notched_seam(&[100.0, 250.0], &[50.0, 150.0]);
    // Stretches of 100/50, 150/100 and 50/50 mm: 8, 13 and 4 steps.
    assert_eq!(mesh.stitches.len(), 8 + 13 + 4 + 1);
    let x = |k: usize| {
        (
            at_mm(&mesh, mesh.stitches[k].0).x,
            at_mm(&mesh, mesh.stitches[k].1).x,
        )
    };
    assert_eq!(x(8), (100.0, 450.0), "the first notches meet");
    assert_eq!(x(21), (250.0, 550.0), "and the second");
    // Even steps within each stretch.
    for k in 0..=25 {
        let (xa, xb) = x(k);
        let (want_a, want_b) = match k {
            0..=8 => (100.0 * k as f64 / 8.0, 400.0 + 50.0 * k as f64 / 8.0),
            9..=21 => (
                100.0 + 150.0 * (k - 8) as f64 / 13.0,
                450.0 + 100.0 * (k - 8) as f64 / 13.0,
            ),
            _ => (
                250.0 + 50.0 * (k - 21) as f64 / 4.0,
                550.0 + 50.0 * (k - 21) as f64 / 4.0,
            ),
        };
        assert!(
            (xa - want_a).abs() < 1e-6 && (xb - want_b).abs() < 1e-6,
            "{k}"
        );
    }
}

#[test]
fn notch_counts_that_differ_fall_back_to_an_even_layout() {
    let (pr, mesh) = notched_seam(&[100.0, 250.0], &[50.0]);
    let steps = (300.0_f64 / H).round() as usize;
    assert_eq!(mesh.stitches.len(), steps + 1);
    for (k, &(sa, sb)) in mesh.stitches.iter().enumerate() {
        let f = k as f64 / steps as f64;
        assert!((at_mm(&mesh, sa).x - 300.0 * f).abs() < 1e-6);
        assert!((at_mm(&mesh, sb).x - (400.0 + 200.0 * f)).abs() < 1e-6);
    }
    // What the seam panel counts to say so.
    let shapes = geom::shapes(&pr);
    let seam = pr.seams[0];
    let count = |s: &geom::Shape, side| geom::side_notches(s, side).unwrap().len();
    assert_eq!(
        (count(&shapes[0], &seam.a), count(&shapes[1], &seam.b)),
        (2, 1)
    );
}

#[test]
fn a_stretch_between_two_close_notches_still_gets_a_step() {
    let (_, mesh) = notched_seam(&[100.0, 103.0], &[50.0, 52.0]);
    let a: Vec<f64> = mesh.stitches.iter().map(|s| at_mm(&mesh, s.0).x).collect();
    let k = a.iter().position(|x| (x - 100.0).abs() < 1e-9).unwrap();
    assert!(
        (a[k + 1] - 103.0).abs() < 1e-9,
        "the next sample is the next notch"
    );
    assert!((at_mm(&mesh, mesh.stitches[k + 1].1).x - 452.0).abs() < 1e-9);
}

#[test]
fn two_free_seams_meeting_at_a_cap_notch_share_its_point() {
    let mut pr = Project::new();
    // A sleeve whose cap (its top edge, curved) has a notch halfway along it.
    let mut sleeve = Piece::rectangle(PieceId(0), "Sleeve", p(0.0, 0.0), 300.0, 100.0);
    sleeve.edges[2] = Edge::Curve {
        c1: p(250.0, 220.0),
        c2: p(50.0, 220.0),
    };
    let cap = geom::edge_length(&sleeve, 2);
    sleeve.notches = vec![Notch::new(2, cap / 2.0)];
    let sleeve = pr.add_piece(sleeve);
    let front = pr.add_piece(Piece::rectangle(
        PieceId(0),
        "Front",
        p(500.0, 0.0),
        100.0,
        200.0,
    ));
    let back = pr.add_piece(Piece::rectangle(
        PieceId(0),
        "Back",
        p(800.0, 0.0),
        100.0,
        200.0,
    ));
    // Cap to the front's right edge as far as the notch; on from the notch to the back's left.
    pr.add_seam(
        part(sleeve, (2, 0.0), (2, 0.5), true),
        side(front, Half::Drawn, 1, 1, true),
    );
    pr.add_seam(
        part(sleeve, (2, 0.5), (2, 1.0), true),
        side(back, Half::Drawn, 3, 3, true),
    );
    assert_eq!(pr.check(), Ok(()));
    let mesh = build(&pr, &MeshParams::default());
    // Each cap half is 14.5 mm shorter than its armhole edge (ease); nothing else to say.
    assert!(
        mesh.notes
            .iter()
            .all(|n| matches!(n, MeshNote::LengthsDiffer { .. })),
        "{:?}",
        mesh.notes
    );
    // The first seam's steps: its longer side is the front's 200 mm edge.
    let first = (200.0_f64.max(cap / 2.0) / H).round() as usize;
    let end_of_first = mesh.stitches[first];
    let start_of_second = mesh.stitches[first + 1];
    assert_eq!(end_of_first.0, start_of_second.0, "one point of the sleeve");
    let notch = geom::point_at_distance(&geom::shapes(&pr)[0].piece, 2, cap / 2.0);
    assert!(at_mm(&mesh, end_of_first.0).distance(notch) < 1e-6);
    // ...stitched to the top of the front's right edge and the top of the back's left edge.
    assert_eq!(at_mm(&mesh, end_of_first.1), p(600.0, 200.0));
    assert_eq!(at_mm(&mesh, start_of_second.1), p(800.0, 200.0));
    // Every other sleeve point of the two seams is a point of its own.
    let mut sleeve_points: Vec<u32> = mesh.stitches.iter().map(|s| s.0.1).collect();
    sleeve_points.sort_unstable();
    sleeve_points.dedup();
    assert_eq!(sleeve_points.len(), mesh.stitches.len() - 1);
}

#[test]
fn random_free_seams_with_notches_never_panic() {
    let mut seed: u64 = 0x9e37_79b9_7f4a_7c15;
    let mut rnd = move || {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        (seed >> 11) as f64 / (1u64 << 53) as f64
    };
    let mut sewn = 0;
    for _ in 0..120 {
        let mut pr = Project::new();
        for k in 0..2 {
            let n = 3 + (rnd() * 4.0) as usize;
            // A convex polygon, so it always meshes: corners round an ellipse.
            let corners: Vec<Point2> = (0..n)
                .map(|i| {
                    let a = (i as f64 + 0.3 * rnd()) / n as f64 * std::f64::consts::TAU;
                    p(
                        k as f64 * 700.0 + 250.0 + 200.0 * a.cos(),
                        250.0 + 150.0 * a.sin(),
                    )
                })
                .collect();
            let mut piece = Piece::polygon(PieceId(0), "Random", &corners);
            for _ in 0..(rnd() * 4.0) as usize {
                let edge = (rnd() * n as f64) as usize;
                let len = geom::edge_length(&piece, edge);
                piece.notches.push(Notch::new(edge, rnd() * len));
            }
            pr.add_piece(piece);
        }
        let n: Vec<usize> = pr.pieces.iter().map(Piece::len).collect();
        for _ in 0..4 {
            let mut free = |shape: usize| {
                let (e0, t0, e1, t1) = (
                    (rnd() * n[shape] as f64) as usize,
                    rnd(),
                    (rnd() * n[shape] as f64) as usize,
                    rnd(),
                );
                part(pr.pieces[shape].id, (e0, t0), (e1, t1), rnd() < 0.5)
            };
            let (a, b) = (free(0), free(1));
            let mut tried = pr.clone();
            tried.add_seam(a, b);
            if tried.check().is_ok() {
                pr = tried;
            }
        }
        sewn += pr.seams.len();
        let mesh = build(&pr, &MeshParams::default());
        assert_eq!(mesh.panels.len(), 2, "{:?}", mesh.notes);
        for &((pa, a), (pb, b)) in &mesh.stitches {
            assert!(
                (a as usize) < mesh.panels[pa].flat.len()
                    && (b as usize) < mesh.panels[pb].flat.len()
            );
        }
        for panel in &mesh.panels {
            assert!(panel.flat.iter().flatten().all(|v| v.is_finite()));
        }
    }
    assert!(sewn > 100, "{sewn} free seams sewn");
}
```

Run: `cargo nextest run -p opendrape-mesh --test fabric`
Expected: the new tests FAIL. Before this task a free side is sampled as if it were its whole edges, so the stitches land in the wrong places, the cap notch isn't shared, and notches aren't matched.

- [ ] **Step 2: Outlines with free sides**

Replace the whole of `crates/mesh/src/boundary.rs`:

```rust
//! The points round one shape's outline that become the fabric's edge: every corner; along each
//! seam side, the side's own samples (so both sides of a seam have the same number); and along
//! the stretches no side covers, points about `h` apart. Two sides that meet part-way along an
//! edge share the point where they meet.

use opendrape_core::{Point2, SeamSide};
use opendrape_geom::{self as geom, Run, Shape};

/// A shape's outline as points (mm), and where things are among them.
pub(crate) struct Outline {
    pub points: Vec<Point2>,
    /// For each outline edge: its start corner, the points along it, its end corner.
    pub edges: Vec<Vec<u32>>,
    /// For each side asked for: the point of each of its samples, from its start.
    pub sides: Vec<Vec<u32>>,
}

/// A point along an outline edge: its distance (mm) from the edge's start, and the side
/// sample it is (side, sample), if any.
type Along = (f64, Option<(usize, usize)>);

/// The outline of `shape` with each of `sides` (a side on this shape, and its samples as
/// distances along it from its start) in place. None when a side does not fit the shape.
pub(crate) fn outline(shape: &Shape, sides: &[(SeamSide, Vec<f64>)], h: f64) -> Option<Outline> {
    let piece = &shape.piece;
    let m = piece.len();
    let lens: Vec<f64> = (0..m).map(|j| geom::edge_length(piece, j)).collect();
    let tiny = |j: usize| 1e-9 * lens[j].max(1.0);
    // Per outline edge, the stretches sides cover, and the points along it.
    let mut covered: Vec<Vec<(f64, f64)>> = vec![Vec::new(); m];
    let mut along: Vec<Vec<Along>> = vec![Vec::new(); m];
    // Side samples that land on a corner: (side, sample, corner).
    let mut on_corner = Vec::new();
    for (s, (side, at)) in sides.iter().enumerate() {
        let runs = geom::side_runs(shape, side)?;
        for r in &runs {
            covered[r.edge].push((r.from.min(r.to), r.from.max(r.to)));
        }
        for (k, d) in at.iter().enumerate() {
            let (edge, x) = locate(&runs, *d);
            if x <= tiny(edge) {
                on_corner.push((s, k, edge));
            } else if x >= lens[edge] - tiny(edge) {
                on_corner.push((s, k, (edge + 1) % m));
            } else {
                along[edge].push((x, Some((s, k))));
            }
        }
    }
    for j in 0..m {
        for (g0, g1) in gaps(&mut covered[j], lens[j], tiny(j)) {
            let steps = (((g1 - g0) / h).round() as usize).max(1);
            along[j].extend((1..steps).map(|q| (g0 + (g1 - g0) * q as f64 / steps as f64, None)));
        }
        along[j].sort_by(|a, b| a.0.total_cmp(&b.0));
    }
    let mut points = Vec::new();
    let mut corners = Vec::with_capacity(m);
    let mut inner: Vec<Vec<u32>> = vec![Vec::new(); m];
    let mut side_points: Vec<Vec<u32>> = sides.iter().map(|(_, at)| vec![0; at.len()]).collect();
    for j in 0..m {
        corners.push(points.len() as u32);
        points.push(piece.vertices[j].pos);
        // Points at the same place (where two sides meet) are one point.
        let mut last: Option<(f64, u32)> = None;
        for &(d, sample) in &along[j] {
            let index = match last {
                Some((at, index)) if d - at <= tiny(j) => index,
                _ => {
                    let index = points.len() as u32;
                    points.push(geom::point_at_distance(piece, j, d));
                    inner[j].push(index);
                    index
                }
            };
            last = Some((d, index));
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

/// Where distance `d` along a side made of `runs` is: the outline edge, and how far along it
/// from its start. A distance at the end of one run is the start of the next (the same corner).
fn locate(runs: &[Run], d: f64) -> (usize, f64) {
    let mut start = 0.0;
    let mut r = 0;
    while r + 1 < runs.len() && d >= start + runs[r].length() {
        start += runs[r].length();
        r += 1;
    }
    let run = runs[r];
    let local = (d - start).clamp(0.0, run.length());
    let x = if run.to >= run.from {
        run.from + local
    } else {
        run.from - local
    };
    (run.edge, x)
}

/// The stretches of an edge `len` mm long that none of `covered` covers, longer than `tiny`.
fn gaps(covered: &mut [(f64, f64)], len: f64, tiny: f64) -> Vec<(f64, f64)> {
    covered.sort_by(|a, b| a.0.total_cmp(&b.0));
    let mut out = Vec::new();
    let mut at = 0.0;
    for &(lo, hi) in covered.iter() {
        if lo - at > tiny {
            out.push((at, lo));
        }
        at = f64::max(at, hi);
    }
    if len - at > tiny {
        out.push((at, len));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_stretches_nobody_sews_are_what_is_left_of_the_edge() {
        let mut covered = vec![(60.0, 80.0), (10.0, 30.0), (30.0, 40.0)];
        assert_eq!(
            gaps(&mut covered, 100.0, 1e-9),
            vec![(0.0, 10.0), (40.0, 60.0), (80.0, 100.0)]
        );
        assert_eq!(gaps(&mut [(0.0, 100.0)], 100.0, 1e-9), vec![]);
        assert_eq!(gaps(&mut [], 100.0, 1e-9), vec![(0.0, 100.0)]);
    }

    #[test]
    fn a_distance_along_a_side_is_found_on_its_runs() {
        let runs = [
            Run {
                edge: 1,
                from: 30.0,
                to: 100.0,
            },
            Run {
                edge: 2,
                from: 50.0,
                to: 0.0,
            },
        ];
        assert_eq!(locate(&runs, 0.0), (1, 30.0));
        assert_eq!(locate(&runs, 69.0), (1, 99.0));
        assert_eq!(
            locate(&runs, 70.0),
            (2, 50.0),
            "the corner, on the next run"
        );
        assert_eq!(locate(&runs, 100.0), (2, 20.0), "running backwards");
        assert_eq!(locate(&runs, 120.0), (2, 0.0));
    }
}
```

- [ ] **Step 3: The layout**

In `crates/mesh/src/lib.rs`:
- The module comment becomes:

```rust
//! Pattern pieces into fabric. Each shape on the pattern table (a piece, a whole cut-on-fold
//! piece, a twin) becomes one panel: its stitching outline sampled into points (both sides of a
//! seam with the same count, laid out so that notches paired across the seam land on the same
//! stitch) and filled with near-equilateral triangles. Seams become pairs of stitched points.
//! Pure: no GPU, no windows.
```

- Replace `struct SeamPlan` and `impl SeamPlan` with:

```rust
/// One side of a seam as the layout sees it: its length, its notches and the corners inside it,
/// as distances (mm) from its start.
struct SidePlan {
    length: f64,
    notches: Vec<f64>,
    corners: Vec<f64>,
}

impl SidePlan {
    fn new(shape: &Shape, side: &SeamSide) -> Option<Self> {
        let runs = geom::side_runs(shape, side)?;
        let mut corners = Vec::new();
        let mut at = 0.0;
        for run in &runs[..runs.len() - 1] {
            at += run.length();
            corners.push(at);
        }
        Some(Self {
            length: at + runs[runs.len() - 1].length(),
            notches: geom::side_notches(shape, side)?,
            corners,
        })
    }
}

/// A seam to stitch, mirror images included: the shapes of its sides (indices into the shapes)
/// and what the layout needs of each side.
struct SeamPlan {
    seam: Seam,
    shape_a: usize,
    shape_b: usize,
    a: SidePlan,
    b: SidePlan,
    mirrored: bool,
}

impl SeamPlan {
    /// Where each side's samples go at edge length `h` (mm from the side's start, the same
    /// number on both sides). When the sides have as many notches as each other, the k-th
    /// notch of one and the k-th of the other start a stretch each, and every pair of stretches
    /// gets its own step count (at least one), so paired notches land on the same sample;
    /// otherwise the whole sides are one stretch each, as before notches were matched. Each
    /// corner inside a side then takes over the sample of its stretch nearest to it.
    fn layout(&self, h: f64) -> (Vec<f64>, Vec<f64>) {
        let breaks = |side: &SidePlan, matched: bool| {
            let mut b = vec![0.0];
            if matched {
                b.extend(&side.notches);
            }
            b.push(side.length);
            b
        };
        let matched = self.a.notches.len() == self.b.notches.len();
        let (ba, bb) = (breaks(&self.a, matched), breaks(&self.b, matched));
        let steps: Vec<usize> = ba
            .windows(2)
            .zip(bb.windows(2))
            .map(|(a, b)| (((a[1] - a[0]).max(b[1] - b[0]) / h).round() as usize).max(1))
            .collect();
        (
            samples(&ba, &steps, &self.a.corners),
            samples(&bb, &steps, &self.b.corners),
        )
    }
}

/// Samples along a side cut into stretches at `breaks` (its start, notches, its end), with
/// `steps[j]` equal steps on stretch j. Each corner (a distance in `corners`) takes over the
/// sample of its stretch nearest to it, never a stretch's own ends; a corner whose sample
/// another corner took keeps none.
fn samples(breaks: &[f64], steps: &[usize], corners: &[f64]) -> Vec<f64> {
    let mut at = Vec::new();
    let mut first = Vec::with_capacity(steps.len());
    for (j, &n) in steps.iter().enumerate() {
        first.push(at.len());
        let (lo, hi) = (breaks[j], breaks[j + 1]);
        at.extend((0..n).map(|k| lo + (hi - lo) * k as f64 / n as f64));
    }
    at.push(breaks[breaks.len() - 1]);
    let mut taken = vec![false; at.len()];
    for &c in corners {
        let Some(j) = (0..steps.len()).find(|&j| c >= breaks[j] && c <= breaks[j + 1]) else {
            continue;
        };
        let (n, lo, hi) = (steps[j], breaks[j], breaks[j + 1]);
        if n < 2 || hi - lo <= 0.0 {
            continue;
        }
        let k = first[j] + (((c - lo) / (hi - lo) * n as f64).round() as usize).clamp(1, n - 1);
        if !taken[k] {
            taken[k] = true;
            at[k] = c;
        }
    }
    at
}
```

- In `build`, `let differ = (plan.length_a - plan.length_b).abs();` becomes `let differ = (plan.a.length - plan.b.length).abs();`.
- Replace `seam_plans` and `mesh_shapes`:

```rust
/// Every seam with both its shapes and side lengths known, mirror images included.
fn seam_plans(project: &Project, shapes: &[Shape]) -> Vec<SeamPlan> {
    let shape_of = |id| shapes.iter().position(|s: &Shape| s.id == id);
    project
        .all_seams()
        .into_iter()
        .filter_map(|(seam, mirrored)| {
            let (shape_a, shape_b) = (shape_of(seam.a.shape)?, shape_of(seam.b.shape)?);
            Some(SeamPlan {
                seam,
                shape_a,
                shape_b,
                a: SidePlan::new(&shapes[shape_a], &seam.a)?,
                b: SidePlan::new(&shapes[shape_b], &seam.b)?,
                mirrored,
            })
        })
        .collect()
}

/// The panels for `shapes` at edge length `h`, stitched along `seams`. Each panel may use
/// what is left of the `max_particles` budget.
fn mesh_shapes(shapes: &[Shape], seams: &[SeamPlan], h: f64, max_particles: usize) -> Made {
    let mut panels = Vec::new();
    let mut notes = Vec::new();
    let mut complete = true;
    // For each shape: its panel's index, and where each of its sides' samples landed.
    let mut made: Vec<Option<(usize, Vec<Vec<u32>>)>> = Vec::with_capacity(shapes.len());
    let mut used = 0;
    let layouts: Vec<(Vec<f64>, Vec<f64>)> = seams.iter().map(|plan| plan.layout(h)).collect();
    for (index, shape) in shapes.iter().enumerate() {
        let sides: Vec<(SeamSide, Vec<f64>)> = seams
            .iter()
            .zip(&layouts)
            .flat_map(|(plan, (at_a, at_b))| {
                [
                    (plan.seam.a, plan.shape_a, at_a),
                    (plan.seam.b, plan.shape_b, at_b),
                ]
                .into_iter()
                .filter(|(_, s, _)| *s == index)
                .map(|(side, _, at)| (side, at.clone()))
            })
            .collect();
        match panel(shape, &sides, h, max_particles.saturating_sub(used)) {
            Ok(done) => {
                used += done.mesh.flat.len();
                complete &= done.complete;
                if done.cutouts_left_out > 0 {
                    notes.push(MeshNote::CutoutLeftOut(shape.id));
                }
                made.push(Some((panels.len(), done.sides)));
                panels.push(done.mesh);
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
    for (plan, (at_a, _)) in seams.iter().zip(&layouts) {
        let (sa, sb) = (plan.shape_a, plan.shape_b);
        let side_a = next_side[sa];
        next_side[sa] += 1;
        let side_b = next_side[sb];
        next_side[sb] += 1;
        if let (Some((pa, a)), Some((pb, b))) = (&made[sa], &made[sb]) {
            for k in 0..at_a.len() {
                stitches.push(((*pa, a[side_a][k]), (*pb, b[side_b][k])));
            }
        }
    }
    Made {
        panels,
        stitches,
        meshed: made.iter().map(Option::is_some).collect(),
        notes,
        complete,
    }
}
```

- In `panel`'s signature, `sides: &[(SeamSide, usize)],` becomes `sides: &[(SeamSide, Vec<f64>)],`.

- [ ] **Step 4: Run them**

Run: `cargo nextest run -p opendrape-mesh`
Expected: all pass, the M4a tests included. In the probe, the paired notches landed on stitches 8 and 21 of 26.

- [ ] **Step 5: Commit**

```bash
cargo nextest run --workspace
cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings
git add crates/mesh
git commit -m "feat(mesh): free seam sides sampled across corners, notches matched across seams

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 4: Arms on the form, and Place at → arm (review: full)

**Files:**
- Modify:
  - `crates/mesh/src/place.rs`
  - `crates/drape/src/{stage.rs,lib.rs}`

**Interfaces:**
- Consumes:
  - M4a's `place::{apply, centre_of, position, rotation, effective, layout, PLACE_GAP_M, ANGLE_STEPS, ROW_SPACING_M, RADIUS_TRIES}`, `geom::{outline_points, contains}`, `MIN_CURVE_M`/`MAX_CURVE_M`;
  - `BodyCollider::ray_exit(origin, dir, max) -> Option<f64>` and `Stage::{shared, shoulder_y, signed_distance}`.
- Produces (mesh, `opendrape_mesh::place`):
  - `pub const ARM_FALLBACK_RADIUS_M: f64 = 0.08;`, `pub const SLEEVE_ANGLE: f64 = FRAC_PI_2;`
  - `Arm { pub shoulder: DVec3, pub direction: DVec3, pub length: f64, pub free: f64 }`: Copy, PartialEq. Methods: `around(&self, angle) -> DVec3`, `at(&self, along) -> DVec3`, `distance(&self, DVec3) -> f64`.
  - `place_at_arm(shape: &Shape, arm: &Arm, surface: &dyn Fn(f64, f64) -> Option<f64>, inside: &dyn Fn(DVec3) -> bool) -> Placement`. `surface(along, angle)` is the arm's surface distance from its line.
  - `along_outline(&[Point2], spacing) -> Vec<Point2>`.
- Produces (drape):
  - `pub const ARM_RAY_M: f64 = 0.15;`
  - `Stage::arms(&self) -> &[Arm; 2]` (left, +x, first) and `Stage::arm_surface_distance(&self, arm: usize, along, angle) -> Option<f64>`;
  - `opendrape_drape::Arm` (re-exported).

**Behaviour:**
- **Finding the arms** (once, when the Stage loads):
  - The body is cut across every 1 cm from the shoulders down to 55% of its height.
  - Each cut's closed loops are followed edge to edge. A loop whose area centroid is at least 15 cm from the centre line, on that side, is the arm's.
  - The highest such cut is the armpit: `free` is how far down the line that is.
  - The line is a least-squares fit of x and z against height through the loop middles within 10 cm below the armpit, extended up to shoulder height.
  - `length` runs to the lowest cut that still finds the arm.
- **`Arm::around(angle)`:** 0 is the arm's front (+z), π/2 its outer side, and mirror images give the same angle on both arms.
- **Place at → arm:**
  - The piece's top (pattern +y) goes towards the shoulder, and its width wraps round the arm's line, centred at `SLEEVE_ANGLE`.
  - The radius is the largest surface distance found over the angles and the free stretch of arm it covers, plus 3 cm, tried again three times (as M4a's Place at…). With no ray hit it is 8 cm.
  - Its top is at the shoulder, or 1 cm steps further down (at most 20 cm, and no more than the piece is long) until none of its points (outline and a grid at least 1 cm apart, at most 40 points across) is inside the form.

- [ ] **Step 1: Failing tests**

In `crates/mesh/src/place.rs`, replace the `}` that closes `mod tests` (the file's last line) with:

```rust
    /// An arm stand-in on the form's left: from (0.18, 1.3, 0) down and out, a little forward.
    fn left_arm() -> Arm {
        Arm {
            shoulder: DVec3::new(0.18, 1.3, 0.0),
            direction: DVec3::new(0.5, -0.85, 0.1).normalize(),
            length: 0.6,
            free: 0.0,
        }
    }

    /// A sleeve: 340 mm wide, 220 mm long, with its twin.
    fn sleeves() -> Project {
        let mut pr = Project::new();
        let id = pr.add_piece(Piece::rectangle(
            PieceId(0),
            "Sleeve",
            p(0.0, 0.0),
            340.0,
            220.0,
        ));
        pr.add_twin(id, "Sleeve (mirror)".into(), p(800.0, 0.0))
            .unwrap();
        pr
    }

    #[test]
    fn an_arm_has_a_front_and_an_outer_side_the_same_way_round_on_both() {
        let left = left_arm();
        let right = Arm {
            shoulder: left.shoulder * DVec3::new(-1.0, 1.0, 1.0),
            direction: left.direction * DVec3::new(-1.0, 1.0, 1.0),
            ..left
        };
        for arm in [left, right] {
            let (front, out) = (arm.around(0.0), arm.around(std::f64::consts::FRAC_PI_2));
            assert!(front.z > 0.9, "the front faces forward: {front}");
            assert!(
                out.x * arm.direction.x > 0.0 && out.y > 0.0,
                "out and up: {out}"
            );
            for v in [front, out] {
                assert!(v.dot(arm.direction).abs() < 1e-12 && (v.length() - 1.0).abs() < 1e-12);
            }
        }
        let mirror = |v: DVec3| v * DVec3::new(-1.0, 1.0, 1.0);
        for angle in [0.0, 0.7, 2.0, -1.2] {
            near(right.around(angle), mirror(left.around(angle)));
        }
        // 5 cm in front of the shoulder; and past the far end, measured from the end.
        assert!((left.distance(left.shoulder + left.around(0.0) * 0.05) - 0.05).abs() < 1e-12);
        assert!((left.distance(left.at(0.7)) - 0.1).abs() < 1e-12);
    }

    #[test]
    fn place_at_arm_wraps_a_sleeve_round_the_arm_line_with_its_top_at_the_shoulder() {
        let pr = sleeves();
        let shapes = geom::shapes(&pr);
        let arm = left_arm();
        // An arm 4.5 cm thick all the way down.
        let thick = |along: f64, _angle: f64| (0.0..=0.6).contains(&along).then_some(0.045);
        let placed = place_at_arm(&shapes[0], &arm, &thick, &|_| false);
        let r = 0.045 + PLACE_GAP_M;
        assert_eq!(placed.curve, Some(r));
        assert!(placed.is_valid());
        let centre = centre_of(&shapes[0]);
        for x in [0.0, 85.0, 170.0, 300.0, 340.0] {
            for y in [0.0, 110.0, 220.0] {
                let q = apply(&placed, centre, p(x, y));
                let along = (q - arm.shoulder).dot(arm.direction);
                assert!(
                    (arm.distance(q) - r).abs() < 1e-9,
                    "({x}, {y}) is round the arm"
                );
                // The top of the sleeve is level with the shoulder, its hem 220 mm down.
                assert!(
                    (along - (220.0 - y) / 1000.0).abs() < 1e-9,
                    "({x}, {y}): {along}"
                );
            }
        }
        // The middle of its width is on the arm's outer side.
        let middle = apply(&placed, centre, p(170.0, 110.0));
        near(middle, arm.at(0.11) + arm.around(SLEEVE_ANGLE) * r);
        // Its left half (on the pattern) is towards the front of the arm.
        assert!(apply(&placed, centre, p(85.0, 110.0)).z > middle.z);
    }

    #[test]
    fn the_twin_of_a_sleeve_on_one_arm_is_on_the_other() {
        let mut pr = sleeves();
        let arm = left_arm();
        let shapes = geom::shapes(&pr);
        let placed = place_at_arm(&shapes[0], &arm, &|_, _| Some(0.045), &|_| false);
        pr.set_placement(PieceId(1), Some(placed));
        let shapes = geom::shapes(&pr);
        let twin = effective(&pr, &shapes[1], &layout(&shapes), 1.3);
        let right = Arm {
            shoulder: arm.shoulder * DVec3::new(-1.0, 1.0, 1.0),
            direction: arm.direction * DVec3::new(-1.0, 1.0, 1.0),
            ..arm
        };
        for q in geom::outline_points(&shapes[1].piece, 1.0) {
            let at = apply(&twin, centre_of(&shapes[1]), q);
            assert!((right.distance(at) - 0.075).abs() < 1e-9, "{at}");
        }
    }

    #[test]
    fn a_sleeve_moved_after_placing_takes_its_curve_with_it() {
        let pr = sleeves();
        let shapes = geom::shapes(&pr);
        let arm = left_arm();
        let placed = place_at_arm(&shapes[0], &arm, &|_, _| Some(0.045), &|_| false);
        // Moved 5 cm forward and turned 10° about y, as with the gizmo: still wrapped round
        // the arm's line moved and turned the same way, not round the body's centre line.
        let turn = DQuat::from_rotation_y(10f64.to_radians());
        let shift = DVec3::new(0.0, 0.0, 0.05);
        let centre_before = position(&placed);
        let moved = Placement {
            position: (centre_before + shift).to_array(),
            rotation: (turn * rotation(&placed)).to_array(),
            ..placed
        };
        let moved_arm = Arm {
            shoulder: centre_before + shift + turn * (arm.shoulder - centre_before),
            direction: turn * arm.direction,
            ..arm
        };
        for q in geom::outline_points(&shapes[0].piece, 1.0) {
            let at = apply(&moved, centre_of(&shapes[0]), q);
            assert!((moved_arm.distance(at) - 0.075).abs() < 1e-9, "{at}");
        }
    }

    #[test]
    fn with_no_arm_in_reach_place_at_arm_uses_a_fallback_curve() {
        let pr = sleeves();
        let shapes = geom::shapes(&pr);
        let placed = place_at_arm(&shapes[0], &left_arm(), &|_, _| None, &|_| false);
        assert_eq!(placed.curve, Some(ARM_FALLBACK_RADIUS_M));
        assert!(placed.is_valid());
    }

    #[test]
    fn place_at_arm_measures_the_arm_only_where_it_hangs_free() {
        let pr = sleeves();
        let shapes = geom::shapes(&pr);
        // Down to 10 cm the arm's rays run into the body, 15 cm out; below, it is 4.5 cm thick.
        let arm = Arm {
            free: 0.1,
            ..left_arm()
        };
        let joined = |along: f64, _: f64| Some(if along < 0.1 { 0.15 } else { 0.045 });
        let placed = place_at_arm(&shapes[0], &arm, &joined, &|_| false);
        assert_eq!(placed.curve, Some(0.045 + PLACE_GAP_M));
    }

    #[test]
    fn place_at_arm_moves_a_piece_down_the_arm_until_it_is_clear_of_the_form() {
        let pr = sleeves();
        let shapes = geom::shapes(&pr);
        let arm = left_arm();
        // A form whose shoulder fills everything above 1.27 m.
        let shoulder = |q: DVec3| q.y > 1.27;
        let surface = |_: f64, _: f64| Some(0.045);
        let placed = place_at_arm(&shapes[0], &arm, &surface, &shoulder);
        let centre = centre_of(&shapes[0]);
        let outline = along_outline(&geom::outline_points(&shapes[0].piece, 0.5), 1.0);
        let highest = |p: &Placement| {
            outline
                .iter()
                .map(|q| apply(p, centre, *q).y)
                .fold(f64::MIN, f64::max)
        };
        assert!(highest(&placed) <= 1.27, "clear: {}", highest(&placed));
        // And no further down than it had to go: a centimetre higher, it is not clear.
        let higher = Placement {
            position: (position(&placed) - arm.direction * 0.01).to_array(),
            ..placed
        };
        assert!(highest(&higher) > 1.27, "{}", highest(&higher));
        // Still wrapped round the arm, its top somewhere down from the shoulder.
        let top = (position(&placed) - arm.shoulder).dot(arm.direction) - 0.11;
        assert!(top > 0.0 && top <= 0.22, "{top}");
        // With no clear place on the arm at all, it stays at the shoulder.
        let everywhere = place_at_arm(&shapes[0], &arm, &surface, &|_| true);
        near(
            position(&everywhere),
            arm.at(0.11) + arm.around(SLEEVE_ANGLE) * 0.075,
        );
    }

    #[test]
    fn place_at_arm_on_a_huge_piece_checks_a_bounded_number_of_points() {
        // A 2 m square (no sleeve; chosen by mistake), on an arm where nothing is ever clear:
        // the search gives up after 20 cm, checking at most a few thousand points each step.
        let mut pr = Project::new();
        pr.add_piece(Piece::rectangle(
            PieceId(0),
            "Huge",
            p(0.0, 0.0),
            2000.0,
            2000.0,
        ));
        let shapes = geom::shapes(&pr);
        let checked = std::cell::Cell::new(0_usize);
        let placed = place_at_arm(&shapes[0], &left_arm(), &|_, _| Some(0.045), &|_| {
            checked.set(checked.get() + 1);
            true
        });
        assert!(placed.is_valid());
        assert!(
            checked.get() < 21 * 2_000,
            "{} points checked",
            checked.get()
        );
        // It stays at the shoulder: its top level with it.
        let top = (position(&placed) - left_arm().shoulder).dot(left_arm().direction) - 1.0;
        assert!(top.abs() < 1e-9, "{top}");
    }
}
```

Append to `crates/drape/src/stage.rs`'s `mod tests`:

```rust
    #[test]
    fn the_arms_hang_down_and_out_from_the_shoulders_and_mirror_each_other() {
        let stage = Stage::shared();
        let [left, right] = *stage.arms();
        let mirror = |v: DVec3| DVec3::new(-v.x, v.y, v.z);
        assert!((right.shoulder - mirror(left.shoulder)).length() < 1e-9);
        assert!((right.direction - mirror(left.direction)).length() < 1e-9);
        assert!((right.length - left.length).abs() < 1e-9 && (right.free - left.free).abs() < 1e-9);
        // Down and out, towards the form's left, at the shoulders.
        let tilt = left.direction.y.abs().acos().to_degrees();
        assert!(
            left.direction.x > 0.0 && (30.0..55.0).contains(&tilt),
            "{tilt}°"
        );
        assert!((left.shoulder.y - stage.shoulder_y()).abs() < 1e-9);
        assert!((0.08..0.25).contains(&left.shoulder.x), "{}", left.shoulder);
        assert!((0.1..0.2).contains(&left.free), "free from {}", left.free);
        assert!((0.4..0.8).contains(&left.length), "{}", left.length);
        // Where the arm hangs free, its line runs inside it: rays find its surface 2.5 to 6 cm
        // away all round, on both arms.
        for along in [left.free, left.free + 0.05, left.free + 0.1] {
            assert!(
                stage.signed_distance(left.at(along)) < 0.0,
                "inside the arm {along}"
            );
            for k in 0..12 {
                let angle = f64::from(k) * 30f64.to_radians();
                for arm in [0, 1] {
                    let d = stage.arm_surface_distance(arm, along, angle);
                    assert!(
                        d.is_some_and(|d| (0.025..0.06).contains(&d)),
                        "arm {arm}, {along:.2} m down, {angle:.2} round: {d:?}"
                    );
                }
            }
        }
    }
```

Run: `cargo nextest run -p opendrape-mesh -p opendrape-drape`
Expected: compile errors (no `Arm`, `place_at_arm`, `arms()`).

- [ ] **Step 2: Arm and Place at → arm**

In `crates/mesh/src/place.rs`:
- The module comment's end becomes "the mirror image a twin takes, Place at… (round the body, or round an arm), and the angles typed in Properties."
- After `const RADIUS_TRIES: usize = 3;`:

```rust
/// Place at → arm curves a piece this much (m) when no ray finds the arm near it.
pub const ARM_FALLBACK_RADIUS_M: f64 = 0.08;
/// Where round its arm a sleeve is centred (radians from the arm's front, see [`Arm::around`]):
/// its outer side, so its two underarm edges meet under the arm, facing the body.
pub const SLEEVE_ANGLE: f64 = std::f64::consts::FRAC_PI_2;
/// Place at → arm moves a piece down its arm in steps this long (m) until it is clear of the
/// form...
const ARM_STEP_M: f64 = 0.01;
/// ...at most this far (m).
const ARM_MAX_DROP_M: f64 = 0.2;
/// It checks the piece's points this far apart (mm) for being inside the form, or further
/// apart on a piece more than 40 times as big, so a huge piece is checked as quickly.
const CLEAR_SPACING_MM: f64 = 10.0;
```

- After `place_at` (before `partner_centre`):

```rust
/// One arm of the form, as a straight line down its upper arm: from `shoulder` (at the form's
/// shoulder height) along the unit `direction` for `length` (m). From `free` (m) down the line,
/// past the armpit, the arm hangs free of the body. The arms are each other's mirror image
/// across x = 0.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Arm {
    pub shoulder: DVec3,
    pub direction: DVec3,
    pub length: f64,
    pub free: f64,
}

impl Arm {
    /// +1 for the form's left arm (+x), -1 for its right.
    fn side(&self) -> f64 {
        if self.direction.x >= 0.0 { 1.0 } else { -1.0 }
    }

    /// The unit direction square to the arm at `angle` (radians) round it: 0 is the arm's
    /// front (towards +z), π/2 its outer side (away from the body), the same way round on both
    /// arms as their mirror images.
    pub fn around(&self, angle: f64) -> DVec3 {
        let out = (DVec3::Z.cross(self.direction) * self.side()).normalize();
        let front = self.direction.cross(out) * self.side();
        front * angle.cos() + out * angle.sin()
    }

    /// The point `along` metres down the line from the shoulder.
    pub fn at(&self, along: f64) -> DVec3 {
        self.shoulder + self.direction * along
    }

    /// How far (m) `p` is from the arm's line, between its shoulder and its far end.
    pub fn distance(&self, p: DVec3) -> f64 {
        let along = (p - self.shoulder)
            .dot(self.direction)
            .clamp(0.0, self.length);
        (p - self.at(along)).length()
    }
}

/// Place at → Left arm / Right arm: `shape` wrapped round `arm`, its pattern's "up" (the
/// sleeve cap) towards the shoulder and its length down the arm, the middle of its width at
/// [`SLEEVE_ANGLE`] round it.
/// - It curves round the arm's line, [`PLACE_GAP_M`] clear of the arm where the arm hangs free:
///   the largest surface distance (`surface(along, angle)`, from the arm's line, if a ray finds
///   the arm there) over the angles the piece covers and the stretch of free arm it covers, plus
///   the gap; [`ARM_FALLBACK_RADIUS_M`] when no ray finds the arm there. (Up by the shoulder a
///   ray runs on into the body: a curve that clears that leaves a gap under the arm too wide
///   for the underarm seam to close across.)
/// - Its top goes at the shoulder, or as little further down the arm (in steps of
///   [`ARM_STEP_M`], at most [`ARM_MAX_DROP_M`] or the piece's own length) as keeps every point
///   of it out of the form (`inside(point)`); the cap seams pull it up into the armhole. When
///   no such place is found it stays at the shoulder.
///
/// The placement's own position and turn put that axis on the arm's line, so moving or turning
/// the piece later moves the axis with it, as for any placement.
pub fn place_at_arm(
    shape: &Shape,
    arm: &Arm,
    surface: &dyn Fn(f64, f64) -> Option<f64>,
    inside: &dyn Fn(DVec3) -> bool,
) -> Placement {
    let outline = geom::outline_points(&shape.piece, 0.5);
    let (lo, hi) = crate::bounds(&outline);
    let width = (hi.x - lo.x) / 1000.0;
    let tall = (hi.y - lo.y) / 1000.0;
    let rows = ((tall / ROW_SPACING_M).ceil() as usize).max(1);
    // The radius the piece needs when it is wrapped at radius `r` with its top at `top`.
    let need = |r: f64, top: f64| {
        let half_span = (width / 2.0 / r).min(std::f64::consts::PI);
        let mut farthest: Option<f64> = None;
        for i in 0..=ANGLE_STEPS {
            let angle = SLEEVE_ANGLE - half_span + 2.0 * half_span * i as f64 / ANGLE_STEPS as f64;
            for j in 0..=rows {
                let along = top + tall * j as f64 / rows as f64;
                if along < arm.free {
                    continue;
                }
                if let Some(d) = surface(along, angle).filter(|d| d.is_finite()) {
                    farthest = Some(farthest.map_or(d, |f: f64| f.max(d)));
                }
            }
        }
        farthest
            .map_or(ARM_FALLBACK_RADIUS_M, |d| d + PLACE_GAP_M)
            .clamp(MIN_CURVE_M, MAX_CURVE_M)
    };
    // The placement with the piece's top `top` down the arm and curved to radius `r`.
    let centre = lo.lerp(hi, 0.5);
    let out = arm.around(SLEEVE_ANGLE);
    let up = -arm.direction;
    let turn = DQuat::from_mat3(&DMat3::from_cols(up.cross(out), up, out)).normalize();
    let placed = |top: f64, r: f64| Placement {
        position: (arm.at(top + tall / 2.0) + out * r).to_array(),
        rotation: turn.to_array(),
        curve: Some(r),
    };
    // Its points: along the outline, and a grid inside it, about a spacing apart.
    let spacing = CLEAR_SPACING_MM.max((hi.x - lo.x).max(hi.y - lo.y) / 40.0);
    let mut points = along_outline(&outline, spacing);
    let steps = |a: f64, b: f64| ((b - a) / spacing).ceil() as usize;
    for i in 0..=steps(lo.x, hi.x) {
        for j in 0..=steps(lo.y, hi.y) {
            let q = Point2::new(lo.x + i as f64 * spacing, lo.y + j as f64 * spacing);
            if geom::contains(&shape.piece, q) {
                points.push(q);
            }
        }
    }
    let clear = |p: &Placement| !points.iter().any(|q| inside(apply(p, centre, *q)));
    // At each height, the radius is tried again as for Place at…: the larger of the last two
    // tries covers the span it reaches.
    let at = |top: f64| {
        let (mut previous, mut radius) = (ARM_FALLBACK_RADIUS_M, ARM_FALLBACK_RADIUS_M);
        for _ in 0..RADIUS_TRIES {
            previous = radius;
            radius = need(radius, top);
        }
        placed(top, radius.max(previous))
    };
    let steps = (tall.min(ARM_MAX_DROP_M) / ARM_STEP_M).floor() as usize;
    (0..=steps)
        .map(|k| at(k as f64 * ARM_STEP_M))
        .find(clear)
        .unwrap_or_else(|| at(0.0))
}

/// Points round the closed polyline `outline`, no more than `spacing` apart.
pub fn along_outline(outline: &[Point2], spacing: f64) -> Vec<Point2> {
    let n = outline.len();
    let mut out = Vec::new();
    for k in 0..n {
        let (a, b) = (outline[k], outline[(k + 1) % n]);
        let steps = (a.distance(b) / spacing).ceil().max(1.0) as usize;
        out.extend((0..steps).map(|i| a.lerp(b, i as f64 / steps as f64)));
    }
    out
}
```

Run: `cargo nextest run -p opendrape-mesh place`
Expected: all pass, the huge-piece test well under a second.

- [ ] **Step 3: The form's arms**

In `crates/drape/src/stage.rs`:
- The module comment's first paragraph becomes:

```rust
//! The form garments drape on, behind one small boundary: what the 3D view draws (plain
//! positions and triangles), what the solver collides with (with a floor), the centre line, the
//! floor's height, how far the form's surface is from its centre line, and its arms (a line
//! down each, and how far the arm's surface is from it). No GPU code lives here, so the app and
//! the tests share it.
```

- After `use opendrape_body::BodyMesh;` add `pub use opendrape_mesh::place::Arm;`, and after `SHOULDER_SHARE`:

```rust
/// A ray from an arm's line looks this far (m) for the arm's surface. One that runs on into the
/// torso finds nothing this close, and so does not count.
pub const ARM_RAY_M: f64 = 0.15;
/// The form is cut across every this many metres to find its arms...
const ARM_STEP_M: f64 = 0.01;
/// ...an arm's cross-section is a loop of the cut whose middle is at least this far (m) out
/// from the centre line (the torso's own never is, below the shoulders)...
const ARM_OUT_M: f64 = 0.15;
/// ...and the arm's line is fitted through its cross-sections this far (m, in height) below the
/// armpit.
const UPPER_ARM_M: f64 = 0.10;
```

- In `struct Stage`, after `shoulder_y: f64,`:

```rust
    /// The left arm (+x), then the right.
    arms: [Arm; 2],
```

- In the constructor, replace from `let height = …` to the end of the `Self { … }` literal with:

```rust
        let height = f64::from(positions.iter().map(|p| p.y).fold(0.0_f32, f32::max));
        let shoulder_y = SHOULDER_SHARE * height;
        let arms =
            [1.0, -1.0].map(|side| find_arm(&positions, &body.triangles, height, shoulder_y, side));
        Self {
            positions,
            triangles: body.triangles,
            collider,
            shoulder_y,
            arms,
        }
```

- After `signed_distance`, inside `impl Stage`:

```rust
    /// The form's arms: its left (+x), then its right.
    pub fn arms(&self) -> &[Arm; 2] {
        &self.arms
    }

    /// How far (m) the surface of arm `arm` (0 left, 1 right) is from its line, `along` metres
    /// down from the shoulder and at `angle` round it (see [`Arm::around`]), if a ray from the
    /// line finds it within [`ARM_RAY_M`].
    pub fn arm_surface_distance(&self, arm: usize, along: f64, angle: f64) -> Option<f64> {
        let a = self.arms.get(arm)?;
        self.collider
            .ray_exit(a.at(along), a.around(angle), ARM_RAY_M)
    }
```

- After `impl Stage { … }`:

```rust
/// The arm on side `side` (+1 the form's left, -1 its right) of a form `height` m tall whose
/// shoulders are at `shoulder_y`. The form is cut across at heights [`ARM_STEP_M`] apart,
/// from the shoulders down to 55% of its height; below the armpit the arm's cut is a loop of its
/// own, its middle at least [`ARM_OUT_M`] out. The armpit is the highest cut where it is: from
/// there down the arm hangs free. The line is fitted (least squares, x and z against height)
/// through the middles of the arm's cuts within [`UPPER_ARM_M`] below the armpit, from shoulder
/// height down to the lowest cut that still finds the arm.
fn find_arm(
    positions: &[Vec3],
    triangles: &[[u32; 3]],
    height: f64,
    shoulder_y: f64,
    side: f64,
) -> Arm {
    let steps = ((shoulder_y - 0.55 * height) / ARM_STEP_M).floor() as usize;
    // From the shoulders down: each cut's height, and the middle of the arm's loop if it has one.
    let cuts: Vec<(f64, Option<(f64, f64)>)> = (1..=steps)
        .map(|k| {
            let y = shoulder_y - k as f64 * ARM_STEP_M;
            let arm = cross_sections(positions, triangles, y)
                .iter()
                .map(|l| loop_middle(l))
                .filter(|(x, _)| side * x >= ARM_OUT_M)
                .max_by(|a, b| (side * a.0).total_cmp(&(side * b.0)));
            (y, arm)
        })
        .collect();
    let first = cuts.iter().position(|(_, arm)| arm.is_some()).unwrap_or(0);
    let armpit = cuts[first].0;
    let last = cuts[first..]
        .iter()
        .position(|(_, arm)| arm.is_none())
        .map_or(cuts.len(), |k| first + k)
        .saturating_sub(1);
    let fit: Vec<(f64, f64, f64)> = cuts[first..=last]
        .iter()
        .filter(|(y, _)| *y >= armpit - UPPER_ARM_M)
        .filter_map(|(y, arm)| arm.map(|(x, z)| (*y, x, z)))
        .collect();
    let n = fit.len().max(1) as f64;
    let (my, mx, mz) = fit.iter().fold((0.0, 0.0, 0.0), |(a, b, c), (y, x, z)| {
        (a + y / n, b + x / n, c + z / n)
    });
    let syy: f64 = fit.iter().map(|(y, _, _)| (y - my) * (y - my)).sum();
    let slope = |pick: fn(&(f64, f64, f64)) -> f64, mean: f64| {
        let s: f64 = fit.iter().map(|f| (f.0 - my) * (pick(f) - mean)).sum();
        if syy > 0.0 { s / syy } else { 0.0 }
    };
    // Up one metre of height, the line moves (dx, dz).
    let (dx, dz) = (slope(|f| f.1, mx), slope(|f| f.2, mz));
    let direction = DVec3::new(-dx, -1.0, -dz).normalize();
    let shoulder = DVec3::new(mx, my, mz) + DVec3::new(dx, 1.0, dz) * (shoulder_y - my);
    Arm {
        shoulder,
        direction,
        length: (shoulder_y - cuts[last].0) / -direction.y,
        free: (shoulder_y - armpit) / -direction.y,
    }
}

/// Where the plane at height `y` cuts the form's surface: each closed loop of the cut, as its
/// (x, z) points in order. The form is closed, with every edge shared by two triangles, so every
/// edge the plane crosses joins two triangles' cuts.
fn cross_sections(positions: &[Vec3], triangles: &[[u32; 3]], y: f64) -> Vec<Vec<(f64, f64)>> {
    use std::collections::{BTreeMap, BTreeSet};
    let above = |i: u32| f64::from(positions[i as usize].y) >= y;
    // Each crossed edge, and the two other crossed edges of its triangles.
    let mut links: BTreeMap<(u32, u32), Vec<(u32, u32)>> = BTreeMap::new();
    for t in triangles {
        let crossed: Vec<(u32, u32)> = (0..3)
            .map(|k| (t[k], t[(k + 1) % 3]))
            .filter(|&(a, b)| above(a) != above(b))
            .map(|(a, b)| (a.min(b), a.max(b)))
            .collect();
        if let [e, f] = crossed[..] {
            links.entry(e).or_default().push(f);
            links.entry(f).or_default().push(e);
        }
    }
    let point = |(a, b): (u32, u32)| {
        let (p, q) = (
            positions[a as usize].as_dvec3(),
            positions[b as usize].as_dvec3(),
        );
        let t = (y - p.y) / (q.y - p.y);
        (p.x + t * (q.x - p.x), p.z + t * (q.z - p.z))
    };
    let mut seen = BTreeSet::new();
    let mut loops = Vec::new();
    for &start in links.keys() {
        if seen.contains(&start) {
            continue;
        }
        let mut cut = Vec::new();
        let mut at = Some(start);
        while let Some(edge) = at {
            seen.insert(edge);
            cut.push(point(edge));
            at = links[&edge].iter().copied().find(|e| !seen.contains(e));
        }
        loops.push(cut);
    }
    loops
}

/// The middle (x, z) of a closed loop: its area centroid, or the mean of its points when it
/// has no area.
fn loop_middle(points: &[(f64, f64)]) -> (f64, f64) {
    let n = points.len();
    let (mut a, mut cx, mut cz) = (0.0, 0.0, 0.0);
    for k in 0..n {
        let ((x0, z0), (x1, z1)) = (points[k], points[(k + 1) % n]);
        let cross = x0 * z1 - x1 * z0;
        a += cross;
        cx += (x0 + x1) * cross;
        cz += (z0 + z1) * cross;
    }
    if a.abs() < 1e-12 {
        let m = n.max(1) as f64;
        return (
            points.iter().map(|p| p.0).sum::<f64>() / m,
            points.iter().map(|p| p.1).sum::<f64>() / m,
        );
    }
    (cx / (3.0 * a), cz / (3.0 * a))
}
```

In `crates/drape/src/lib.rs`, `pub use stage::{BodyAndFloor, Stage};` becomes `pub use stage::{Arm, BodyAndFloor, Stage};`.

- [ ] **Step 4: Run them**

Run: `cargo nextest run -p opendrape-mesh -p opendrape-drape`
Expected: all pass. In the probe, the arm test found the lines 42.0° from vertical, free from 0.148 m, and rays 3.0 to 5.5 cm all round where the arm hangs free.

- [ ] **Step 5: Commit**

```bash
cargo nextest run --workspace
cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings
git add crates/mesh crates/drape
git commit -m "feat(drape): arm lines from cross-sections of the form; Place at an arm

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 5: Holding a point of the cloth: the attachment API (review: full)

**Files:**
- Create: `crates/sim/src/attach.rs`
- Modify: `crates/sim/src/{cloth.rs,lib.rs,solver.rs}` (10 lines added, nothing existing changed)

**Interfaces:**
- Consumes: `Cloth { x, inv_mass, triangles }` (crate-visible), `ClothBuilder`, `Solver::step`, and the solver's substep loop.
- Produces (exported from `opendrape_sim`):
  - `AttachmentId`: Copy, Eq, Hash, opaque.
  - `Cloth::attach(&mut self, triangle: usize, bary: [f64; 3], target: DVec3, compliance: f64) -> Option<AttachmentId>` (None for no such triangle).
  - `Cloth::move_attachment(&mut self, AttachmentId, DVec3) -> bool`, `Cloth::detach(&mut self, AttachmentId) -> bool`, `Cloth::attached_point(&self, AttachmentId) -> Option<DVec3>`.
  - `Solver::cloth_mut(&mut self) -> &mut Cloth`.
  - Crate-private: `Cloth.attachments: Vec<Option<Attachment>>`, `attach::solve(&mut Cloth, sdt)`.

**Behaviour:**
- **The constraint:** an XPBD constraint pulls the barycentric point towards its target. The move is shared among the triangle's three particles by `w_i · b_i`, with `λ = −|d| / (Σ w_i b_i² + α/sdt²)`.
- **When it runs:** last in each solver iteration (after the stitches, bending and stretch) and before collision, so a pin (α = 0) holds exactly.
- **Ids:** an attachment is named by its triangle index. Welding renumbers the triangles' particles in place, so the point stays on the same fabric. A detached id stays dead (`None` in its slot).

- [ ] **Step 1: Failing tests**

Create `crates/sim/src/attach.rs` with only its tests for now, and add `mod attach;` above `mod cloth;` in `crates/sim/src/lib.rs`:

```rust
#[cfg(test)]
mod tests {
    use crate::{ClothBuilder, Panel, Params, Solver};
    use glam::{DVec2, DVec3};

    /// A 20 cm square of two triangles, hanging flat in the xy plane at height 1 m.
    fn square() -> Solver {
        let flat = vec![
            DVec2::new(0.0, 0.0),
            DVec2::new(0.2, 0.0),
            DVec2::new(0.2, 0.2),
            DVec2::new(0.0, 0.2),
        ];
        let panel = Panel {
            positions: flat
                .iter()
                .map(|p| DVec3::new(p.x, p.y + 1.0, 0.0))
                .collect(),
            flat: Some(flat),
            triangles: vec![[0, 1, 2], [0, 2, 3]],
        };
        let mut b = ClothBuilder::new(0.15);
        b.add_panel(&panel, 1.0);
        Solver::new(
            b.build(),
            Params {
                gravity_delay: 0.0,
                ..Params::default()
            },
        )
    }

    #[test]
    fn a_held_point_stays_at_its_target_while_the_rest_falls() {
        let mut s = square();
        // The middle of the top edge: halfway between corners 2 and 3, in triangle 1.
        let target = DVec3::new(0.1, 1.2, 0.0);
        let id = s
            .cloth_mut()
            .attach(1, [0.0, 0.5, 0.5], target, 0.0)
            .unwrap();
        for _ in 0..60 {
            s.step(None);
        }
        let held = s.cloth().attached_point(id).unwrap();
        assert!((held - target).length() < 1e-3, "{held}");
        assert!(s.cloth().positions()[0].y < 1.05, "the bottom swings down");
    }

    #[test]
    fn a_held_point_follows_its_target() {
        let mut s = square();
        let id = s
            .cloth_mut()
            .attach(0, [1.0 / 3.0; 3], DVec3::new(0.13, 1.07, 0.0), 1e-4)
            .unwrap();
        let to = DVec3::new(0.13, 1.07, 0.15);
        assert!(s.cloth_mut().move_attachment(id, to));
        for _ in 0..60 {
            s.step(None);
        }
        let held = s.cloth().attached_point(id).unwrap();
        assert!((held - to).length() < 0.01, "a stiff spring: {held}");
    }

    #[test]
    fn a_removed_attachment_lets_go() {
        let mut s = square();
        let id = s
            .cloth_mut()
            .attach(1, [0.0, 0.5, 0.5], DVec3::new(0.1, 1.2, 0.0), 0.0)
            .unwrap();
        for _ in 0..30 {
            s.step(None);
        }
        assert!(s.cloth_mut().detach(id));
        assert!(!s.cloth_mut().detach(id), "once");
        assert!(!s.cloth_mut().move_attachment(id, DVec3::ZERO));
        assert_eq!(s.cloth().attached_point(id), None);
        let before = s.cloth().positions()[2].y;
        for _ in 0..30 {
            s.step(None);
        }
        assert!(s.cloth().positions()[2].y < before - 0.05, "it falls");
        assert!(
            s.cloth_mut()
                .attach(2, [1.0, 0.0, 0.0], DVec3::ZERO, 0.0)
                .is_none()
        );
    }

    #[test]
    fn a_point_on_a_seam_stays_held_after_the_seam_welds() {
        // Two squares side by side, sewn along the edge between them; the pin is on the first
        // square's corner that welds to the second's.
        let flat = vec![
            DVec2::new(0.0, 0.0),
            DVec2::new(0.2, 0.0),
            DVec2::new(0.2, 0.2),
            DVec2::new(0.0, 0.2),
        ];
        let panel = |x: f64| Panel {
            positions: flat
                .iter()
                .map(|p| DVec3::new(p.x + x, p.y + 1.0, 0.0))
                .collect(),
            flat: Some(flat.clone()),
            triangles: vec![[0, 1, 2], [0, 2, 3]],
        };
        let mut b = ClothBuilder::new(0.15);
        let (p, q) = (
            b.add_panel(&panel(0.0), 1.0),
            b.add_panel(&panel(0.25), 1.0),
        );
        b.stitch((p, 1), (q, 0));
        b.stitch((p, 2), (q, 3));
        let mut s = Solver::new(b.build(), Params::default());
        let target = DVec3::new(0.22, 1.2, 0.0);
        // Corner 2 of the first square: triangle 0, its third corner.
        let id = s
            .cloth_mut()
            .attach(0, [0.0, 0.0, 1.0], target, 0.0)
            .unwrap();
        for _ in 0..120 {
            s.step(None);
        }
        assert!(!s.cloth().has_open_stitches(), "welded");
        let held = s.cloth().attached_point(id).unwrap();
        assert!((held - target).length() < 1e-3, "{held}");
    }
}
```

Run: `cargo nextest run -p opendrape-sim attach`
Expected: compile errors (no `cloth_mut`, `attach`…).

- [ ] **Step 2: The API**

Put the code above the tests in `crates/sim/src/attach.rs`:

```rust
//! Attachments: a point of the cloth (a barycentric point in one of its triangles) pulled to a
//! target, with a stiffness. A grab is a stiff spring to a target that follows the pointer; a pin
//! holds its point at the target. A point is named by its triangle, not its particles: welding
//! renumbers the triangles' particles in place, so the point stays on the same fabric.

use crate::cloth::Cloth;
use glam::DVec3;

/// An attachment of the cloth it was made on; it stays valid until it is removed.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct AttachmentId(u32);

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Attachment {
    /// An index into the cloth's triangles.
    triangle: usize,
    bary: [f64; 3],
    target: DVec3,
    /// XPBD compliance (m/N): 0 holds the point exactly at its target.
    compliance: f64,
}

impl Cloth {
    /// Pulls the point at barycentric coordinates `bary` (summing to 1) of triangle `triangle`
    /// (an index into [`Cloth::triangles`]) to `target`, as a spring of `compliance` (m/N; 0
    /// holds it exactly there). None when there is no such triangle.
    pub fn attach(
        &mut self,
        triangle: usize,
        bary: [f64; 3],
        target: DVec3,
        compliance: f64,
    ) -> Option<AttachmentId> {
        (triangle < self.triangles.len()).then_some(())?;
        let id = AttachmentId(self.attachments.len() as u32);
        self.attachments.push(Some(Attachment {
            triangle,
            bary,
            target,
            compliance,
        }));
        Some(id)
    }

    /// Moves an attachment's target; false when it has been removed.
    pub fn move_attachment(&mut self, id: AttachmentId, target: DVec3) -> bool {
        match self.attachments.get_mut(id.0 as usize) {
            Some(Some(a)) => {
                a.target = target;
                true
            }
            _ => false,
        }
    }

    /// Lets go of an attachment; false when it was already removed.
    pub fn detach(&mut self, id: AttachmentId) -> bool {
        self.attachments
            .get_mut(id.0 as usize)
            .is_some_and(|a| a.take().is_some())
    }

    /// Where the attached point of the cloth is now; None once it is removed.
    pub fn attached_point(&self, id: AttachmentId) -> Option<DVec3> {
        let a = (*self.attachments.get(id.0 as usize)?)?;
        let t = self.triangles[a.triangle];
        Some((0..3).map(|k| self.x[t[k] as usize] * a.bary[k]).sum())
    }
}

/// One Gauss–Seidel pass over the attachments: each pulls its point towards its target, sharing
/// the move among the triangle's particles by their weight in the point and their inverse mass
/// (XPBD, as for the cloth's own links).
pub(crate) fn solve(c: &mut Cloth, sdt: f64) {
    for a in c.attachments.iter().flatten() {
        let t = c.triangles[a.triangle].map(|k| k as usize);
        let point: DVec3 = (0..3).map(|k| c.x[t[k]] * a.bary[k]).sum();
        let d = point - a.target;
        let len = d.length();
        let wsum: f64 = (0..3)
            .map(|k| c.inv_mass[t[k]] * a.bary[k] * a.bary[k])
            .sum();
        if len < 1e-12 || wsum == 0.0 {
            continue;
        }
        let lambda = -len / (wsum + a.compliance / (sdt * sdt));
        let n = d / len;
        for (&i, &b) in t.iter().zip(&a.bary) {
            c.x[i] += n * (lambda * c.inv_mass[i] * b);
        }
    }
}
```

In `crates/sim/src/cloth.rs`, at the end of `struct Cloth` (after `topology_version`); `Cloth` derives `Default`, so `ClothBuilder::build` needs no change:

```rust
    /// Points of the cloth pulled to targets (see `attach.rs`); a removed one leaves None.
    pub(crate) attachments: Vec<Option<crate::attach::Attachment>>,
```

In `crates/sim/src/lib.rs`, add `pub use attach::AttachmentId;` before `pub use cloth::…`, so the exports read:

```rust
pub use attach::AttachmentId;
pub use cloth::{Cloth, ClothBuilder, Panel, PanelId};
pub use collide::{BodyCollider, Collider, ColliderError, Plane};
pub use solver::{FRAME_DT, Params, Solver};
```

In `crates/sim/src/solver.rs`, after `pub fn cloth(&self) -> &Cloth { … }`:

```rust
    /// The cloth, to attach points of it to targets (see [`Cloth::attach`]).
    pub fn cloth_mut(&mut self) -> &mut Cloth {
        &mut self.cloth
    }
```

and in `step`, inside the `for _ in 0..p.iterations` loop, right after the stretch solve (`solve_links(&mut c.x, &c.inv_mass, &c.stretch, …);`):

```rust
                // Held points last of all, so a pin holds exactly.
                crate::attach::solve(c, sdt);
```

- [ ] **Step 3: Run them**

Run: `cargo nextest run -p opendrape-sim`
Expected: all pass, the four new tests and every existing sim test.

Run: `git diff --stat crates/sim/src/cloth.rs crates/sim/src/lib.rs crates/sim/src/solver.rs`
Expected: 10 insertions, 0 deletions.

- [ ] **Step 4: Commit**

```bash
cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings
git add crates/sim
git commit -m "feat(sim): attach a point of the cloth to a target (pins and grabs)

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 6: Pins and grabs in the drape, and carrying the drape on after an edit (review: full)

**Files:**
- Create: `crates/drape/src/live.rs`
- Modify:
  - `Cargo.toml` (one dev-profile entry)
  - `crates/drape/src/{build.rs,lib.rs}`
  - `crates/app/src/{sim_runner.rs,app.rs}`
  - `crates/app/tests/ui.rs`

**Interfaces:**
- Consumes:
  - Task 2's `Project.pins`, `Shape::spot_shown`;
  - Task 5's `Cloth::{attach, move_attachment, detach}`, `Solver::cloth_mut`, `AttachmentId`;
  - M4a's `opendrape_mesh::{build, GarmentMesh, PanelMesh}`, `place::{effective, apply, layout}`, `Params { gravity_delay, gravity_ramp, weld_time, .. }`, `Cloth::{stitch_pairs, positions, triangles}`.
- Produces (drape, exported from `opendrape_drape`):
  - `pub const PIN_COMPLIANCE: f64 = 0.0;`, `pub const SEWN_GAP_M: f64 = 0.001;`
  - `FabricPanel { pub shape: PieceId, pub flat: Vec<[f64; 2]>, pub triangles: Vec<[u32; 3]>, pub first_particle: usize, pub first_triangle: usize }`.
  - `Fabric { pub panels: Vec<FabricPanel> }` (Default, PartialEq). Methods:
    - `panel(&self, PieceId) -> Option<&FabricPanel>`;
    - `pattern_point(&self, triangle, bary) -> Option<(PieceId, Point2)>`;
    - `cloth_point(&self, PieceId, Point2) -> Option<(usize, [f64; 3])>`.
  - `Drape { pub solver: Solver, pub notes: Vec<DrapeNote>, pub fabric: Arc<Fabric>, pub project: Arc<Project>, pins (private) }`. Methods:
    - `new(Arc<Project>, &Stage) -> Self`, `rebuilt(&self, Arc<Project>, &Stage) -> Self`;
    - `same_fabric(&self, &Project) -> bool`, `set_pins(&mut self, Arc<Project>)`.
  - `build_drape(&Project, &Stage) -> (Solver, Vec<DrapeNote>)` is kept, as a wrapper.
  - Crate-private (`live`): `nearest_triangle(&FabricPanel, DVec2) -> Option<(usize, [f64; 3])>`, `warm_positions(&Drape, &PanelMesh) -> Option<Vec<DVec3>>`, `warm_params(&Cloth) -> Params`.
- Produces (app):
  - `SimFrame.fabric: Arc<Fabric>` and `pub const GRAB_COMPLIANCE: f64 = 1e-4;`
  - `SimRunner::update(&self, Arc<Project>)`, `grab(&self, Arc<Fabric>, triangle, bary, target: DVec3)`, `pull(&self, DVec3)`, `release(&self)`, `remade(&self) -> u64`.
  - `opendrape::sim_runner` re-exports `Fabric`.
  - `App::update_if_edited` replaces `reset_if_edited`.

**Behaviour:**
- **A drape's pins:** each pin is held at its target with compliance 0, at the cloth point its spot maps to. A spot just off the fabric is moved onto the nearest triangle; a pin whose shape has no panel is skipped.
- **Warm start (`Drape::rebuilt`):**
  - Each point of a new panel finds the old panel of the same shape, then the old triangle holding the same flat spot, through a 3 cm grid. A spot outside every triangle uses the nearest one, extrapolated flat.
  - The point starts where that spot of the old cloth is now. The lookup goes through the old cloth's triangles, which welding renumbered in place.
  - A shape with no old panel starts at its placement, and may get the "starts inside the form" note.
  - Velocities are zero and gravity is on at once.
  - Seams weld at once when every stitch starts within 1 mm; otherwise at Play's 0.8 s.
- **The runner:**
  - `update` sends the project and sets playing. On the thread, any `Update`s already waiting are skipped to the newest; a different command found while skipping is handled next.
  - When only pins or placements changed (`same_fabric`), the pins are re-held on the same cloth. Otherwise the fabric is made again (`rebuilt`), its first frame checked like Play's, and `remade` counts it.
  - The last frame keeps showing until the new one is published, and the drape number stays the same.
  - A grab whose `fabric` is not the drape's current fabric (`Arc::ptr_eq`) is ignored.
  - The runner never auto-pauses while a point is grabbed.
- **The app:** each frame while draping, if the editor's project differs from the one last given to the runner, it sends a snapshot (`update`) and plays on. That covers pattern edits, seams, pins, undo and redo.

- [ ] **Step 1: Failing drape tests**

Create `crates/drape/src/live.rs` with only its tests for now, and add `mod live;` after `mod build;` in `crates/drape/src/lib.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::Stage;
    use opendrape_core::{Half, Piece, PieceId, Pin, Placement, Point2, Project, SeamSide};
    use std::sync::Arc;

    /// A 200 × 300 mm panel A and a panel B beside it, sewn A's right edge to B's left, B
    /// `height` mm tall, both hanging in front of the form from pins at A's and B's top corners.
    fn hanging(height: f64) -> Project {
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
            Point2::new(200.0, 300.0 - height),
            200.0,
            height,
        ));
        pr.add_seam(
            SeamSide::edges(a, Half::Drawn, 1, 1, true),
            SeamSide::edges(b, Half::Drawn, 3, 3, false),
        );
        // A plane 50 cm in front of the form, the panels' tops at 1.2 m.
        for id in [a, b] {
            pr.set_placement(id, Some(Placement::at([0.0, 0.0, 0.0])));
        }
        pr.set_placement(a, Some(Placement::at([-0.1, 1.05, 0.5])));
        pr.set_placement(b, Some(Placement::at([0.1, 1.2 - height / 2000.0, 0.5])));
        let pin = |shape, x: f64, at: [f64; 3]| Pin {
            shape,
            half: Half::Drawn,
            at: Point2::new(x, 300.0),
            target: at,
        };
        pr.pins = vec![
            pin(a, 0.0, [-0.2, 1.2, 0.5]),
            pin(b, 400.0, [0.2, 1.2, 0.5]),
        ];
        assert_eq!(pr.check(), Ok(()));
        pr
    }

    /// `drape` stepped on for `frames` frames.
    fn run(drape: &mut Drape, stage: &Stage, frames: usize) {
        let collider = stage.drape_collider();
        for _ in 0..frames {
            drape.solver.step(Some(&collider));
        }
    }

    #[test]
    fn made_again_from_the_same_pattern_every_point_starts_where_it_was() {
        let stage = Stage::shared();
        let pr = Arc::new(hanging(300.0));
        let mut old = Drape::new(pr.clone(), &stage);
        run(&mut old, &stage, 60);
        assert!(!old.solver.cloth().has_open_stitches(), "welded");
        let new = old.rebuilt(pr, &stage);
        // Every corner of every triangle (the same triangles: the same pattern makes the same
        // fabric) starts where that corner of the old cloth is now.
        let (ox, ot) = (
            old.solver.cloth().positions(),
            old.solver.cloth().triangles(),
        );
        let (nx, nt) = (
            new.solver.cloth().positions(),
            new.solver.cloth().triangles(),
        );
        assert_eq!(ot.len(), nt.len());
        let mut worst: f64 = 0.0;
        for (a, b) in ot.iter().zip(nt) {
            for k in 0..3 {
                worst = worst.max((ox[a[k] as usize] - nx[b[k] as usize]).length());
            }
        }
        assert!(worst < 0.001, "{:.4} mm", worst * 1000.0);
        // From rest, with gravity on, and the seam (closed) welded at once.
        let c = new.solver.cloth();
        assert!(c.velocities().iter().all(|v| *v == DVec3::ZERO));
        let params = new.solver.params();
        assert_eq!((params.weld_time, params.gravity_delay), (Some(0.0), 0.0));
        assert_eq!(new.fabric, old.fabric, "the same fabric");
    }

    #[test]
    fn new_fabric_carries_on_from_the_fabric_next_to_it() {
        let stage = Stage::shared();
        let mut old = Drape::new(Arc::new(hanging(300.0)), &stage);
        run(&mut old, &stage, 60);
        // B made 60 mm longer at its hem: its bottom corners move down, out of the old fabric.
        let mut longer = hanging(300.0);
        let b = longer.pieces[1].id;
        for v in 0..2 {
            let at = longer.pieces[1].vertices[v].pos;
            longer.pieces[1].move_vertex(v, at - Point2::new(0.0, 60.0));
        }
        assert_eq!(longer.check(), Ok(()));
        let new = old.rebuilt(Arc::new(longer), &stage);
        let panel = new.fabric.panel(b).unwrap();
        let was = old.fabric.panel(b).unwrap();
        let x = new.solver.cloth().positions();
        // The old hem (y = 0 on the pattern) as it hangs now.
        let old_x = old.solver.cloth().positions();
        let hem: Vec<DVec3> = (0..was.flat.len())
            .filter(|&i| was.flat[i][1].abs() < 1e-9)
            .map(|i| {
                let t = was
                    .triangles
                    .iter()
                    .position(|t| t.contains(&(i as u32)))
                    .unwrap();
                let corner = was.triangles[t]
                    .iter()
                    .position(|&c| c == i as u32)
                    .unwrap();
                old_x[old.solver.cloth().triangles()[was.first_triangle + t][corner] as usize]
            })
            .collect();
        let mut checked = 0;
        for (i, f) in panel.flat.iter().enumerate() {
            let p = x[panel.first_particle + i];
            assert!(p.is_finite());
            if f[1] < -0.005 {
                // New fabric, |f.y| below the old hem on the pattern: about as far below it.
                let below = -f[1];
                let nearest = hem
                    .iter()
                    .map(|h| (*h - p).length())
                    .fold(f64::INFINITY, f64::min);
                assert!(
                    (nearest - below).abs() < 0.3 * below + 0.005,
                    "{:.1} mm below the hem on the pattern, {:.1} mm in 3D",
                    below * 1000.0,
                    nearest * 1000.0
                );
                assert!(p.y < hem.iter().map(|h| h.y).fold(f64::MAX, f64::min) + 0.005);
                checked += 1;
            }
        }
        assert!(checked >= 5, "{checked} new points");
    }

    #[test]
    fn a_seam_sewn_while_draping_closes_and_welds_as_at_play() {
        let stage = Stage::shared();
        // B hangs 10 cm to the right of A, and nothing joins them yet.
        let mut apart = hanging(300.0);
        let b = apart.pieces[1].id;
        apart.set_placement(b, Some(Placement::at([0.2, 1.05, 0.5])));
        apart.pins[1].target = [0.3, 1.2, 0.5];
        let sewn = apart.clone();
        apart.seams.clear();
        let mut old = Drape::new(Arc::new(apart), &stage);
        run(&mut old, &stage, 30);
        // The side seam is sewn now: it starts open, so it closes and welds as at Play.
        let new = old.rebuilt(Arc::new(sewn), &stage);
        let c = new.solver.cloth();
        let x = c.positions();
        let widest = c
            .stitch_pairs()
            .map(|(a, b)| (x[a] - x[b]).length())
            .fold(0.0, f64::max);
        assert!(widest > 0.05, "{widest}");
        assert_eq!(new.solver.params().weld_time, Params::default().weld_time);
    }

    #[test]
    fn pins_hold_their_spots_and_a_pin_edit_keeps_the_fabric() {
        let stage = Stage::shared();
        let pr = hanging(300.0);
        let mut drape = Drape::new(Arc::new(pr.clone()), &stage);
        run(&mut drape, &stage, 90);
        // Each pin's spot (the top corners) is at its target.
        let spot = |drape: &Drape, shape: PieceId, at: Point2| {
            let (t, b) = drape.fabric.cloth_point(shape, at).unwrap();
            let c = drape.solver.cloth();
            let tri = c.triangles()[t];
            (0..3)
                .map(|k| c.positions()[tri[k] as usize] * b[k])
                .sum::<DVec3>()
        };
        for pin in &pr.pins {
            let held = spot(&drape, pin.shape, pin.at);
            assert!(
                (held - DVec3::from_array(pin.target)).length() < 1e-3,
                "{held}"
            );
        }
        // Moving a pin's target is not a change to the fabric: the drape carries on.
        let mut moved = pr.clone();
        moved.pins[0].target = [-0.25, 1.25, 0.5];
        moved.set_placement(moved.pieces[0].id, Some(Placement::at([0.0, 0.5, 0.5])));
        assert!(drape.same_fabric(&moved));
        assert!(!drape.same_fabric(&hanging(320.0)));
        let time = drape.solver.time();
        drape.set_pins(Arc::new(moved.clone()));
        run(&mut drape, &stage, 60);
        assert!(drape.solver.time() > time, "the same drape, carrying on");
        let held = spot(&drape, moved.pins[0].shape, moved.pins[0].at);
        assert!(
            (held - DVec3::new(-0.25, 1.25, 0.5)).length() < 1e-3,
            "{held}"
        );
    }

    #[test]
    fn a_point_of_the_cloth_goes_back_to_its_spot_on_the_pattern() {
        let stage = Stage::shared();
        let drape = Drape::new(Arc::new(hanging(300.0)), &stage);
        let b = drape.project.pieces[1].id;
        for at in [Point2::new(250.0, 120.0), Point2::new(399.0, 1.0)] {
            let (t, bary) = drape.fabric.cloth_point(b, at).unwrap();
            let (shape, back) = drape.fabric.pattern_point(t, bary).unwrap();
            assert_eq!(shape, b);
            assert!(back.distance(at) < 1e-6, "{back:?}");
        }
        // Just outside the piece: the nearest point of its fabric.
        let (t, bary) = drape
            .fabric
            .cloth_point(b, Point2::new(400.5, 150.0))
            .unwrap();
        let (_, back) = drape.fabric.pattern_point(t, bary).unwrap();
        assert!(back.distance(Point2::new(400.0, 150.0)) < 1e-6, "{back:?}");
        assert!(drape.fabric.cloth_point(PieceId(99), at_origin()).is_none());
    }

    fn at_origin() -> Point2 {
        Point2::new(0.0, 0.0)
    }

    #[test]
    fn a_pin_on_a_piece_left_out_of_the_fabric_is_let_go() {
        let stage = Stage::shared();
        let mut pr = hanging(300.0);
        // A piece whose outline crosses itself: it can't be made into fabric.
        let bow = pr.add_piece(Piece::polygon(
            PieceId(0),
            "Bow",
            &[
                Point2::new(600.0, 0.0),
                Point2::new(700.0, 100.0),
                Point2::new(700.0, 0.0),
                Point2::new(600.0, 100.0),
            ],
        ));
        pr.pins.push(Pin {
            shape: bow,
            half: Half::Drawn,
            at: Point2::new(610.0, 50.0),
            target: [0.5, 1.0, 0.5],
        });
        let mut drape = Drape::new(Arc::new(pr), &stage);
        assert!(drape.fabric.panel(bow).is_none());
        run(&mut drape, &stage, 30);
        assert!(
            drape
                .solver
                .cloth()
                .positions()
                .iter()
                .all(|p| p.is_finite())
        );
    }
}
```

Run: `cargo nextest run -p opendrape-drape live`
Expected: compile errors (no `Drape`, `Fabric`, `SEWN_GAP_M`…).

- [ ] **Step 2: Drape, Fabric and the warm start**

Put the code above the tests in `crates/drape/src/live.rs`:

```rust
//! Carrying a drape on after the pattern changes while it drapes: the new fabric starts where
//! the old fabric had got to. Each new point finds the old triangle that held the same spot of
//! the same piece and starts where that spot was; a point the old fabric didn't have (new
//! fabric, as when a sleeve is made longer) carries on from the nearest old triangle, as if
//! that triangle's surface went on flat. Velocities start at zero; seams that were closed stay
//! closed.

use crate::{Drape, FabricPanel};
use glam::{DVec2, DVec3};
use opendrape_mesh::PanelMesh;
use opendrape_sim::{Cloth, Params};

/// Stitches whose two ends start this close (m) are already sewn. When every stitch of the
/// new cloth is, they are all welded at once.
pub const SEWN_GAP_M: f64 = 0.001;
/// The old fabric's triangles are sorted into square cells this wide (m) to find a point's
/// triangle quickly.
const CELL_M: f64 = 0.03;
/// A point this little (in barycentric terms) outside a triangle counts as in it.
const SLACK: f64 = 1e-9;

/// The barycentric coordinates of `p` in the flat triangle `t`.
fn bary(t: [DVec2; 3], p: DVec2) -> [f64; 3] {
    let (v0, v1, v2) = (t[1] - t[0], t[2] - t[0], p - t[0]);
    let (d00, d01, d11) = (v0.dot(v0), v0.dot(v1), v1.dot(v1));
    let (d20, d21) = (v2.dot(v0), v2.dot(v1));
    let denom = d00 * d11 - d01 * d01;
    if denom.abs() < 1e-30 {
        return [1.0, 0.0, 0.0];
    }
    let v = (d11 * d20 - d01 * d21) / denom;
    let w = (d00 * d21 - d01 * d20) / denom;
    [1.0 - v - w, v, w]
}

/// How far `p` is from the flat triangle `t` (0 inside it).
fn distance(t: [DVec2; 3], p: DVec2) -> f64 {
    if bary(t, p).iter().all(|b| *b >= -SLACK) {
        return 0.0;
    }
    (0..3)
        .map(|k| {
            let (a, b) = (t[k], t[(k + 1) % 3]);
            let ab = b - a;
            let s = ((p - a).dot(ab) / ab.length_squared().max(1e-30)).clamp(0.0, 1.0);
            (p - (a + ab * s)).length()
        })
        .fold(f64::INFINITY, f64::min)
}

/// Triangle `j` of `panel`, flat.
fn flat_triangle(panel: &FabricPanel, j: usize) -> [DVec2; 3] {
    panel.triangles[j].map(|k| DVec2::from_array(panel.flat[k as usize]))
}

/// The triangle of `panel` that `p` (m) is in, or else the nearest one: its index in the panel,
/// and `p`'s barycentric coordinates in it (some negative when `p` is outside it). None for a
/// panel with no triangles.
pub(crate) fn nearest_triangle(panel: &FabricPanel, p: DVec2) -> Option<(usize, [f64; 3])> {
    let j = (0..panel.triangles.len()).min_by(|&a, &b| {
        distance(flat_triangle(panel, a), p).total_cmp(&distance(flat_triangle(panel, b), p))
    })?;
    Some((j, bary(flat_triangle(panel, j), p)))
}

/// A panel's triangles sorted into a grid of cells, each listing the triangles whose bounding
/// box reaches it.
struct Index<'a> {
    panel: &'a FabricPanel,
    min: DVec2,
    cols: usize,
    cells: Vec<Vec<usize>>,
}

impl<'a> Index<'a> {
    fn new(panel: &'a FabricPanel) -> Self {
        let pts: Vec<DVec2> = panel.flat.iter().map(|f| DVec2::from_array(*f)).collect();
        let min = pts.iter().fold(DVec2::splat(f64::MAX), |m, p| m.min(*p));
        let max = pts.iter().fold(DVec2::splat(f64::MIN), |m, p| m.max(*p));
        let size = ((max - min) / CELL_M).ceil().max(DVec2::ONE);
        let (cols, rows) = (size.x as usize, size.y as usize);
        let mut cells = vec![Vec::new(); cols * rows];
        let cell = |p: DVec2| {
            let c = ((p - min) / CELL_M).floor();
            (
                (c.x.max(0.0) as usize).min(cols - 1),
                (c.y.max(0.0) as usize).min(rows - 1),
            )
        };
        for j in 0..panel.triangles.len() {
            let t = flat_triangle(panel, j);
            let (lo, hi) = (t[0].min(t[1]).min(t[2]), t[0].max(t[1]).max(t[2]));
            let ((c0, r0), (c1, r1)) = (cell(lo), cell(hi));
            for r in r0..=r1 {
                for c in c0..=c1 {
                    cells[r * cols + c].push(j);
                }
            }
        }
        Self {
            panel,
            min,
            cols,
            cells,
        }
    }

    /// The triangle `p` is in (from its cell), or else the nearest triangle of the panel.
    fn find(&self, p: DVec2) -> Option<(usize, [f64; 3])> {
        let c = ((p - self.min) / CELL_M).floor();
        let rows = self.cells.len() / self.cols;
        if c.x >= 0.0 && c.y >= 0.0 && (c.x as usize) < self.cols && (c.y as usize) < rows {
            let cell = &self.cells[c.y as usize * self.cols + c.x as usize];
            for &j in cell {
                let b = bary(flat_triangle(self.panel, j), p);
                if b.iter().all(|v| *v >= -SLACK) {
                    return Some((j, b));
                }
            }
        }
        nearest_triangle(self.panel, p)
    }
}

/// Where the points of `panel` (a panel of the new fabric) start, carrying on from drape `old`:
/// each point where the same spot of the same piece was in the old cloth, found through the old
/// triangle that held it (welding renumbers triangles in place, so this reaches the live
/// particles); a point outside every old triangle carries on from the nearest one. None when
/// the old drape had no panel for the shape (it starts at its placement).
pub(crate) fn warm_positions(old: &Drape, panel: &PanelMesh) -> Option<Vec<DVec3>> {
    let was = old.fabric.panel(panel.shape)?;
    let index = Index::new(was);
    let cloth = old.solver.cloth();
    let (x, triangles) = (cloth.positions(), cloth.triangles());
    panel
        .flat
        .iter()
        .map(|f| {
            let (j, b) = index.find(DVec2::from_array(*f))?;
            let t = triangles[was.first_triangle + j];
            Some((0..3).map(|k| x[t[k] as usize] * b[k]).sum())
        })
        .collect()
}

/// How a drape made again while draping runs: gravity at once (it was already hanging), and its
/// seams welded at once when every stitch starts closed; otherwise they close and weld as at
/// Play.
pub(crate) fn warm_params(cloth: &Cloth) -> Params {
    let x = cloth.positions();
    let closed = cloth
        .stitch_pairs()
        .all(|(a, b)| (x[a] - x[b]).length() <= SEWN_GAP_M);
    let play = Params::default();
    Params {
        gravity_delay: 0.0,
        gravity_ramp: 0.0,
        weld_time: if closed { Some(0.0) } else { play.weld_time },
        ..play
    }
}
```

In `crates/drape/src/build.rs`, replace everything above `#[cfg(test)]` with the code below (its tests are unchanged). `build_drape` becomes a wrapper round `Drape::new`.

```rust
use crate::Stage;
use crate::live;
use glam::{DVec2, DVec3};
use opendrape_core::{PieceId, Point2, Project};
use opendrape_geom as geom;
use opendrape_mesh::{GarmentMesh, MeshNote, MeshParams, place};
use opendrape_sim::{AttachmentId, ClothBuilder, Panel, Params, Solver};
use std::sync::Arc;

/// Fabric weight (kg/m²): one light cotton until fabrics arrive.
pub const DENSITY_KG_M2: f64 = 0.15;
/// How stiffly a pin holds its spot at its target (XPBD compliance, m/N): exactly.
pub const PIN_COMPLIANCE: f64 = 0.0;

/// Something the student should know about the drape.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum DrapeNote {
    /// From making the fabric (see [`MeshNote`]).
    Mesh(MeshNote),
    /// Part of this piece starts inside the form.
    StartsInside(PieceId),
}

/// One panel of a drape's fabric: the shape it was made from, its flat points (m, on the
/// pattern table) and its own triangles, and where its particles and triangles start in the
/// cloth.
#[derive(Clone, Debug, PartialEq)]
pub struct FabricPanel {
    pub shape: PieceId,
    pub flat: Vec<[f64; 2]>,
    pub triangles: Vec<[u32; 3]>,
    pub first_particle: usize,
    pub first_triangle: usize,
}

/// Where a drape's cloth came from on the pattern: its panels, in cloth order.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Fabric {
    pub panels: Vec<FabricPanel>,
}

impl Fabric {
    fn of(mesh: &GarmentMesh) -> Self {
        let (mut particle, mut triangle) = (0, 0);
        let panels = mesh
            .panels
            .iter()
            .map(|p| {
                let panel = FabricPanel {
                    shape: p.shape,
                    flat: p.flat.clone(),
                    triangles: p.triangles.clone(),
                    first_particle: particle,
                    first_triangle: triangle,
                };
                particle += p.flat.len();
                triangle += p.triangles.len();
                panel
            })
            .collect();
        Self { panels }
    }

    /// The panel of shape `shape`.
    pub fn panel(&self, shape: PieceId) -> Option<&FabricPanel> {
        self.panels.iter().find(|p| p.shape == shape)
    }

    /// The panel that cloth triangle `triangle` belongs to, and its index in the panel.
    fn triangle(&self, triangle: usize) -> Option<(&FabricPanel, usize)> {
        self.panels.iter().find_map(|p| {
            let j = triangle.checked_sub(p.first_triangle)?;
            (j < p.triangles.len()).then_some((p, j))
        })
    }

    /// Which shape, and where on it (mm), the point at barycentric `bary` of cloth triangle
    /// `triangle` is.
    pub fn pattern_point(&self, triangle: usize, bary: [f64; 3]) -> Option<(PieceId, Point2)> {
        let (panel, j) = self.triangle(triangle)?;
        let t = panel.triangles[j];
        let f = (0..3).fold(DVec2::ZERO, |acc, k| {
            acc + DVec2::from_array(panel.flat[t[k] as usize]) * bary[k]
        });
        Some((panel.shape, Point2::new(f.x * 1000.0, f.y * 1000.0)))
    }

    /// The cloth triangle holding point `at` (mm, on shape `shape`), with the point's
    /// barycentric coordinates in it: the triangle it is in, or, just outside the fabric, the
    /// nearest one, with the point moved onto it.
    pub fn cloth_point(&self, shape: PieceId, at: Point2) -> Option<(usize, [f64; 3])> {
        let panel = self.panel(shape)?;
        let f = DVec2::new(at.x / 1000.0, at.y / 1000.0);
        let (j, bary) = live::nearest_triangle(panel, f)?;
        let clamped = bary.map(|b| b.max(0.0));
        let sum: f64 = clamped.iter().sum();
        Some((
            panel.first_triangle + j,
            if sum > 0.0 {
                clamped.map(|b| b / sum)
            } else {
                [1.0 / 3.0; 3]
            },
        ))
    }
}

/// A drape of a project: its solver, what the student should know about it, where its fabric
/// came from, the project it was made from, and its pins as they are held in the cloth.
pub struct Drape {
    pub solver: Solver,
    pub notes: Vec<DrapeNote>,
    pub fabric: Arc<Fabric>,
    pub project: Arc<Project>,
    pins: Vec<AttachmentId>,
}

impl Drape {
    /// The drape of `project` on `stage`: every shape that could be meshed, at its placement,
    /// sewn by its seams, its pins held at their targets.
    pub fn new(project: Arc<Project>, stage: &Stage) -> Self {
        Self::make(project, stage, None)
    }

    /// The drape of `project`, an edit of this drape's project, carrying on from where this one
    /// has got to (see `live`): a shape that was already draped starts where it was, a new one
    /// at its placement.
    pub fn rebuilt(&self, project: Arc<Project>, stage: &Stage) -> Self {
        Self::make(project, stage, Some(self))
    }

    fn make(project: Arc<Project>, stage: &Stage, from: Option<&Drape>) -> Self {
        let mesh = opendrape_mesh::build(&project, &MeshParams::default());
        let fabric = Fabric::of(&mesh);
        let shapes = geom::shapes(&project);
        let layout = place::layout(&shapes);
        let mut notes: Vec<DrapeNote> = mesh.notes.iter().map(|n| DrapeNote::Mesh(*n)).collect();
        let mut builder = ClothBuilder::new(DENSITY_KG_M2);
        let mut ids = Vec::with_capacity(mesh.panels.len());
        for panel in &mesh.panels {
            let warm = from.and_then(|old| live::warm_positions(old, panel));
            let positions = warm.unwrap_or_else(|| {
                let shape = shapes
                    .iter()
                    .find(|s| s.id == panel.shape)
                    .expect("every panel comes from a shape");
                let placement = place::effective(&project, shape, &layout, stage.shoulder_y());
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
                positions
            });
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
        let cloth = builder.build();
        let params = match from {
            None => Params::default(),
            Some(_) => live::warm_params(&cloth),
        };
        let mut drape = Self {
            solver: Solver::new(cloth, params),
            notes,
            fabric: Arc::new(fabric),
            project,
            pins: Vec::new(),
        };
        drape.hold_pins();
        drape
    }

    /// Whether `project` makes the same fabric as this drape's: they differ at most in pins and
    /// placements (which don't apply while draping).
    pub fn same_fabric(&self, project: &Project) -> bool {
        let pattern = |p: &Project| {
            let mut p = p.clone();
            p.pins.clear();
            for piece in &mut p.pieces {
                piece.placement = None;
                if let Some(t) = &mut piece.twin {
                    t.placement = None;
                }
            }
            p
        };
        pattern(&self.project) == pattern(project)
    }

    /// Holds `project`'s pins in place of this drape's own (its fabric is unchanged: see
    /// [`Self::same_fabric`]).
    pub fn set_pins(&mut self, project: Arc<Project>) {
        for id in std::mem::take(&mut self.pins) {
            self.solver.cloth_mut().detach(id);
        }
        self.project = project;
        self.hold_pins();
    }

    /// Attaches each of the project's pins to its spot of the fabric.
    fn hold_pins(&mut self) {
        let shapes = geom::shapes(&self.project);
        for pin in &self.project.pins {
            let Some(shape) = shapes.iter().find(|s| s.id == pin.shape) else {
                continue;
            };
            let spot = shape.spot_shown(pin.half, pin.at);
            let Some((triangle, bary)) = self.fabric.cloth_point(pin.shape, spot) else {
                continue;
            };
            let target = DVec3::from_array(pin.target);
            if let Some(id) = self
                .solver
                .cloth_mut()
                .attach(triangle, bary, target, PIN_COMPLIANCE)
            {
                self.pins.push(id);
            }
        }
    }
}

/// The fabric and cloth for `project` on `stage`: every shape that could be meshed, at its
/// placement, sewn by its seams, its pins held; and the notes about it.
pub fn build_drape(project: &Project, stage: &Stage) -> (Solver, Vec<DrapeNote>) {
    let drape = Drape::new(Arc::new(project.clone()), stage);
    (drape.solver, drape.notes)
}
```

In `crates/drape/src/lib.rs`, the module comment and exports become:

```rust
//! The form garments drape on, and the glue from a project to cloth on it. Shared by the app's
//! simulation thread and the tests, so what the tests drape is what the student sees: the
//! [`Stage`] (the body, its frame, its arms and its collider with a floor) and [`Drape`] (the
//! project's fabric at its placements, sewn by its seams, its pins held; made again from an
//! edited project, carrying on from where it had got to). No GPU, no windows.

mod build;
mod live;
pub mod stage;

pub use build::{
    DENSITY_KG_M2, Drape, DrapeNote, Fabric, FabricPanel, PIN_COMPLIANCE, build_drape,
};
pub use live::SEWN_GAP_M;
pub use stage::{Arm, BodyAndFloor, Stage};
```

In the root `Cargo.toml`, after the `[profile.dev.package.opendrape-mesh]` entry:

```toml
# The drape is made again on every edit while it drapes (the warm start).
[profile.dev.package.opendrape-drape]
opt-level = 2
```

Without it, making a T-shirt's drape again takes 382 ms in the dev profile; with it, 48 ms. This is a build setting, not a dependency: `Cargo.lock` does not change.

Run: `cargo nextest run -p opendrape-drape`
Expected: all pass, including:
- `made_again_from_the_same_pattern_every_point_starts_where_it_was` (worst 0.000 mm in the probe);
- `new_fabric_carries_on_from_the_fabric_next_to_it`;
- `a_pin_on_a_piece_left_out_of_the_fabric_is_let_go`.

- [ ] **Step 3: Failing runner tests**

In `crates/app/src/sim_runner.rs`'s `mod tests`:
- Add `use glam::DVec3;` after `use super::*;`.
- In `status()`, after `shown: Arc::new(AtomicU64::new(0)),` add `remade: Arc::new(AtomicU64::new(0)),`.
- Replace `a_frame_is_only_made_from_numbers_the_renderer_can_take`; the runner's private `Drape` now wraps an `opendrape_drape::Drape`, imported as `Made`:

```rust
    #[test]
    fn a_frame_is_only_made_from_numbers_the_renderer_can_take() {
        let stage = Stage::shared();
        for project in unusable_placements() {
            let mut drape = Drape::new(1, Made::new(project, &stage));
            assert!(drape.frame(1, 0.0).is_none(), "the first frame");
        }
        let mut drape = Drape::new(7, Made::new(two_panels(), &stage));
        let frame = drape.frame(1, 0.0).expect("an ordinary drape");
        assert_eq!(frame.drape, 7);
        assert!(frame.positions.iter().all(|p| p.is_finite()));
        // Later frames go through the same check.
        let collider = stage.drape_collider();
        for _ in 0..3 {
            drape.made.solver.step(Some(&collider));
            assert!(drape.frame(2, 1.0).is_some());
        }
    }
```

- In `a_panic_while_making_the_fabric_is_a_drape_gone_wrong_not_a_dead_thread`, the two lines `let (solver, notes) = build_drape(&two_panels(), &Stage::shared());` and `let mut drape = Drape::new(4, solver, notes);` become `let mut drape = Drape::new(4, Made::new(two_panels(), &Stage::shared()));`.
- Replace the `}` that closes `mod tests` (the file's last line) with:

```rust
    /// `two_panels` with B made `longer` mm longer at its hem.
    fn lengthened(longer: f64) -> Arc<Project> {
        let mut pr = (*two_panels()).clone();
        for v in 0..2 {
            let at = pr.pieces[1].vertices[v].pos;
            pr.pieces[1].move_vertex(v, at - Point2::new(0.0, longer));
        }
        Arc::new(pr)
    }

    #[test]
    fn an_edit_while_draping_carries_the_drape_on_with_the_new_pattern() {
        let r = SimRunner::start(Stage::shared(), || {});
        r.play(two_panels());
        wait_for("the drape to run", || {
            r.latest().is_some_and(|f| f.time > 0.2)
        });
        let before = r.latest().unwrap();
        r.update(lengthened(60.0));
        wait_for("the longer fabric", || {
            r.latest()
                .is_some_and(|f| f.positions.len() > before.positions.len())
        });
        let after = r.latest().unwrap();
        assert_eq!(after.drape, before.drape, "the same drape: no Reset");
        assert!(!Arc::ptr_eq(&after.fabric, &before.fabric));
        assert!(r.is_draping() && r.is_playing() && !r.went_wrong());
        assert_eq!(r.remade(), 1);
    }

    #[test]
    fn edits_that_arrive_while_the_fabric_is_made_again_are_made_once_the_newest() {
        let r = SimRunner::start(Stage::shared(), || {});
        r.play(two_panels());
        wait_for("the drape to run", || {
            r.latest().is_some_and(|f| f.time > 0.1)
        });
        for k in 1..=6 {
            r.update(lengthened(10.0 * f64::from(k)));
        }
        let newest = opendrape_mesh::build(&lengthened(60.0), &MeshParams::default());
        wait_for("the newest pattern", || {
            r.latest()
                .is_some_and(|f| f.positions.len() == newest.particles())
        });
        assert!(r.remade() <= 2, "made {} times", r.remade());
        std::thread::sleep(Duration::from_millis(100));
        assert_eq!(r.latest().unwrap().positions.len(), newest.particles());
    }

    #[test]
    fn moving_a_pin_while_draping_carries_the_same_cloth_on() {
        let mut pr = (*two_panels()).clone();
        let a = pr.pieces[0].id;
        let pin = opendrape_core::Pin {
            shape: a,
            half: Half::Drawn,
            at: Point2::new(0.0, 300.0),
            target: [-0.25, 1.4, 0.4],
        };
        pr.pins = vec![pin];
        let r = SimRunner::start(Stage::shared(), || {});
        r.play(Arc::new(pr.clone()));
        wait_for("the drape to run", || {
            r.latest().is_some_and(|f| f.time > 0.2)
        });
        let before = r.latest().unwrap();
        pr.pins[0].target = [-0.3, 1.45, 0.4];
        r.update(Arc::new(pr));
        wait_for("more drape", || {
            r.latest().is_some_and(|f| f.time > before.time + 0.2)
        });
        assert!(Arc::ptr_eq(&r.latest().unwrap().fabric, &before.fabric));
        assert_eq!(r.remade(), 0, "pins alone don't make the fabric again");
    }

    /// Where the point at `bary` of triangle `t` of `frame`'s cloth is.
    fn point(frame: &SimFrame, t: usize, bary: [f64; 3]) -> DVec3 {
        let tri = frame.triangles[t];
        (0..3)
            .map(|k| frame.positions[tri[k] as usize].as_dvec3() * bary[k])
            .sum()
    }

    #[test]
    fn a_grabbed_point_follows_the_pointer_and_letting_go_lets_it_fall() {
        let r = SimRunner::start(Stage::shared(), || {});
        r.play(on_the_floor());
        wait_for("it to settle", || !r.is_playing() && r.is_idle());
        let frame = r.latest().unwrap();
        let held = [1.0 / 3.0; 3];
        let start = point(&frame, 0, held);
        let up = start + DVec3::new(0.0, 0.25, 0.0);
        r.grab(frame.fabric.clone(), 0, held, start);
        assert!(r.is_playing(), "grabbing wakes the drape");
        r.pull(up);
        wait_for("the point to follow", || {
            r.latest()
                .is_some_and(|f| (point(&f, 0, held) - up).length() < 0.02)
        });
        r.release();
        wait_for("it to fall back", || {
            r.latest().is_some_and(|f| point(&f, 0, held).y < 0.05)
        });
    }

    #[test]
    fn a_grab_of_cloth_made_again_since_is_not_taken() {
        let r = SimRunner::start(Stage::shared(), || {});
        r.play(two_panels());
        wait_for("the drape to run", || {
            r.latest().is_some_and(|f| f.time > 0.1)
        });
        let old = r.latest().unwrap();
        r.update(lengthened(60.0));
        wait_for("the new fabric", || {
            r.latest()
                .is_some_and(|f| !Arc::ptr_eq(&f.fabric, &old.fabric))
        });
        let held = [1.0 / 3.0; 3];
        let start = point(&r.latest().unwrap(), 0, held);
        r.grab(
            old.fabric.clone(),
            0,
            held,
            start + DVec3::new(0.0, 1.0, 0.0),
        );
        let t = r.latest().unwrap().time;
        wait_for("time to pass", || {
            r.latest().is_some_and(|f| f.time > t + 0.3)
        });
        assert!(
            point(&r.latest().unwrap(), 0, held).y < start.y + 0.05,
            "not pulled"
        );
    }
}
```

Run: `cargo nextest run -p opendrape --lib sim_runner`
Expected: compile errors (no `update`, `grab`, `remade`, `Made`…).

- [ ] **Step 4: The runner**

Replace everything above `#[cfg(test)]` in `crates/app/src/sim_runner.rs`:

```rust
//! Drapes the student's garment on its own thread, so the window stays responsive even when a
//! slow computer simulates slower than real time. Play hands the thread a snapshot of the
//! project; the thread builds the fabric (meshing takes a moment) and the cloth, then steps
//! the solver. An edit while draping hands it the edited project: it makes the fabric again,
//! carrying on from where the drape had got to (the last frame keeps showing meanwhile, and
//! edits that arrive while it works are made once, the newest). A grab pulls a point of the
//! cloth after the pointer. Reset drops the drape. The UI only reads the latest frame.

use arc_swap::ArcSwapOption;
use crossbeam_channel::{Receiver, Sender, unbounded};
use glam::{DVec3, Vec3};
use opendrape_core::Project;
use opendrape_drape::Drape as Made;
use opendrape_drape::Stage;
pub use opendrape_drape::{DENSITY_KG_M2, DrapeNote, Fabric, build_drape};
use opendrape_sim::AttachmentId;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

/// One published simulation frame.
#[derive(Debug)]
pub struct SimFrame {
    /// Which Play made it (see [`SimRunner::latest`]).
    pub drape: u64,
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
    /// Where this frame's cloth came from on the pattern: its triangles, to grab and pin it
    /// by. A new one each time an edit makes the fabric again.
    pub fabric: Arc<Fabric>,
}

/// A grabbed point of the cloth follows the pointer as a spring this stiff (XPBD compliance,
/// m/N): firmly, but the fabric round it still has its say.
pub const GRAB_COMPLIANCE: f64 = 1e-4;

enum Command {
    /// Drape this project; its frames are numbered `drape`.
    Play(Arc<Project>, u64),
    /// The project changed while draping: carry the drape on with this one.
    Update(Arc<Project>),
    /// Pull the point at `bary` in triangle `triangle` of the cloth made from `fabric` (when
    /// that is still the cloth) towards `target`.
    Grab {
        fabric: Arc<Fabric>,
        triangle: usize,
        bary: [f64; 3],
        target: DVec3,
    },
    /// The grabbed point's target moves.
    Pull(DVec3),
    /// Let go of the grabbed point.
    Release,
    Reset,
    Wake,
    Shutdown,
}

pub struct SimRunner {
    tx: Sender<Command>,
    latest: Arc<ArcSwapOption<SimFrame>>,
    /// From Play until Reset (or until the drape goes wrong): the 3D view shows the drape.
    draping: Arc<AtomicBool>,
    playing: Arc<AtomicBool>,
    idle: Arc<AtomicBool>,
    went_wrong: Arc<AtomicBool>,
    /// The number of the Play whose frames the 3D view shows. Play and Reset both move it on, so
    /// a frame an earlier drape publishes after Reset (the thread may be in the middle of a
    /// step) is never shown.
    shown: Arc<AtomicU64>,
    /// How many times an edit has made the fabric again (the tests count them).
    remade: Arc<AtomicU64>,
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
    shown: Arc<AtomicU64>,
    remade: Arc<AtomicU64>,
}

impl Status {
    /// Drape number `drape` went wrong (a number stopped being finite, or making it panicked):
    /// drop it, stop, and say so. A drape the student has already moved on from (Reset, or a
    /// newer Play) is dropped silently.
    fn fail(&self, drape: u64) {
        if self.shown.load(Ordering::Acquire) != drape {
            return;
        }
        self.latest.store(None);
        self.draping.store(false, Ordering::Relaxed);
        self.playing.store(false, Ordering::Relaxed);
        self.went_wrong.store(true, Ordering::Release);
    }
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
            shown: Arc::new(AtomicU64::new(0)),
            remade: Arc::new(AtomicU64::new(0)),
        };
        let (latest, draping, playing, idle, went_wrong, shown, remade) = (
            status.latest.clone(),
            status.draping.clone(),
            status.playing.clone(),
            status.idle.clone(),
            status.went_wrong.clone(),
            status.shown.clone(),
            status.remade.clone(),
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
            shown,
            remade,
            thread: Some(thread),
        }
    }
    /// Drapes `project`: the fabric is made on the simulation thread, then it runs.
    pub fn play(&self, project: Arc<Project>) {
        let drape = self.shown.fetch_add(1, Ordering::AcqRel) + 1;
        self.went_wrong.store(false, Ordering::Release);
        self.draping.store(true, Ordering::Relaxed);
        self.playing.store(true, Ordering::Relaxed);
        let _ = self.tx.send(Command::Play(project, drape));
    }
    /// The project changed while draping: the drape carries on with `project` (its fabric made
    /// again from where the drape has got to, or only its pins moved when that is all that
    /// changed), and plays on if it had paused. Nothing happens while arranging.
    pub fn update(&self, project: Arc<Project>) {
        if !self.is_draping() {
            return;
        }
        self.playing.store(true, Ordering::Relaxed);
        let _ = self.tx.send(Command::Update(project));
    }
    /// Pulls the point at `bary` in triangle `triangle` of `fabric`'s cloth (as a frame showed
    /// it) towards `target`, and plays on if the drape had paused. A cloth made again since
    /// that frame is not grabbed.
    pub fn grab(&self, fabric: Arc<Fabric>, triangle: usize, bary: [f64; 3], target: DVec3) {
        self.playing.store(true, Ordering::Relaxed);
        let _ = self.tx.send(Command::Grab {
            fabric,
            triangle,
            bary,
            target,
        });
    }
    /// Moves the grabbed point's target (and plays on, if the drape had paused).
    pub fn pull(&self, target: DVec3) {
        self.playing.store(true, Ordering::Relaxed);
        let _ = self.tx.send(Command::Pull(target));
    }
    /// Lets go of the grabbed point.
    pub fn release(&self) {
        let _ = self.tx.send(Command::Release);
    }
    /// How many times an edit has made the fabric again.
    pub fn remade(&self) -> u64 {
        self.remade.load(Ordering::Acquire)
    }
    /// Back to arranging: the drape is dropped, and its last frame is gone at once.
    pub fn reset(&self) {
        self.shown.fetch_add(1, Ordering::AcqRel);
        self.draping.store(false, Ordering::Relaxed);
        self.playing.store(false, Ordering::Relaxed);
        self.latest.store(None);
        let _ = self.tx.send(Command::Reset);
    }
    /// The latest frame of the drape since the last Play; None while arranging (and while the
    /// fabric is made). A frame the thread was still working on when Reset (or a newer Play)
    /// came is not shown.
    pub fn latest(&self) -> Option<Arc<SimFrame>> {
        let shown = self.shown.load(Ordering::Acquire);
        self.latest.load_full().filter(|f| f.drape == shown)
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
    /// Whether the last drape went wrong (a number stopped being finite, or making it
    /// panicked) and was dropped.
    pub fn went_wrong(&self) -> bool {
        self.went_wrong.load(Ordering::Acquire)
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
    /// The Play it came from.
    number: u64,
    made: Made,
    notes: Arc<Vec<DrapeNote>>,
    topology: u64,
    triangles: Arc<Vec<[u32; 3]>>,
    /// The point being grabbed, if any.
    grab: Option<AttachmentId>,
}

impl Drape {
    fn new(number: u64, made: Made) -> Self {
        let c = made.solver.cloth();
        Self {
            number,
            topology: c.topology_version(),
            triangles: Arc::new(c.triangles().to_vec()),
            notes: Arc::new(made.notes.clone()),
            made,
            grab: None,
        }
    }

    /// The frame to publish, or None if any live particle is not a finite number as the
    /// renderer will get it (single precision: a double too big for one counts too). Every
    /// frame, the first included, goes through here before anyone sees it.
    fn frame(&mut self, seq: u64, step_ms: f64) -> Option<SimFrame> {
        let c = self.made.solver.cloth();
        let positions: Vec<Vec3> = c.positions().iter().map(|p| p.as_vec3()).collect();
        if positions
            .iter()
            .enumerate()
            .any(|(i, p)| c.is_alive(i) && !p.is_finite())
        {
            return None;
        }
        if c.topology_version() != self.topology {
            self.topology = c.topology_version();
            self.triangles = Arc::new(c.triangles().to_vec());
        }
        Some(SimFrame {
            drape: self.number,
            seq,
            time: self.made.solver.time(),
            positions,
            triangles: self.triangles.clone(),
            step_ms,
            notes: self.notes.clone(),
            fabric: self.made.fabric.clone(),
        })
    }
}

/// `work()`, or None if it panicked: a pattern that sends the mesher or the solver somewhere it
/// should never go must end the drape with a message, not the simulation thread without a word;
/// and the 3D view's fabric (`arrange::scene`), made on the thread that draws the window, must
/// not end the program.
pub(crate) fn guarded<T>(work: impl FnOnce() -> T) -> Option<T> {
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(work)).ok()
}

/// Makes drape `number` with `make` (its first frame too) and publishes that frame; the drape
/// goes on to be stepped. If `make` panics, or gives no drape because its first frame is not
/// made of numbers the renderer can take, the drape went wrong: it is reported (see
/// [`Status::fail`]) and there is nothing to step.
fn start_drape(
    status: &Status,
    number: u64,
    make: impl FnOnce() -> Option<(Drape, SimFrame)>,
) -> Option<Drape> {
    match guarded(make).flatten() {
        Some((drape, first)) => {
            status.latest.store(Some(Arc::new(first)));
            Some(drape)
        }
        None => {
            status.fail(number);
            None
        }
    }
}

fn run(stage: &Stage, rx: &Receiver<Command>, status: &Status, on_frame: &dyn Fn()) {
    let mut drape: Option<Drape> = None;
    let mut seq = 0;
    let mut next = Instant::now();
    let mut settled_frames = 0;
    // A command read while gathering edits together, handled next.
    let mut pending: Option<Command> = None;
    loop {
        // Draping and playing: just look for a command. Otherwise sleep until one arrives.
        let busy = drape.is_some() && status.playing.load(Ordering::Relaxed);
        let cmd = if pending.is_some() {
            pending.take()
        } else if busy {
            rx.try_recv().ok()
        } else {
            status.idle.store(true, Ordering::Release);
            let cmd = rx.recv().unwrap_or(Command::Shutdown);
            status.idle.store(false, Ordering::Release);
            Some(cmd)
        };
        match cmd {
            Some(Command::Shutdown) => return,
            Some(Command::Play(project, number)) => {
                // The old drape goes first, so its memory is free for the new one.
                drop(drape.take());
                seq += 1;
                // Making the fabric, and the first frame, are checked like every later frame.
                drape = start_drape(status, number, || {
                    let mut d = Drape::new(number, Made::new(project, stage));
                    let first = d.frame(seq, 0.0);
                    first.map(|f| (d, f))
                });
                settled_frames = 0;
                on_frame();
                next = Instant::now();
                continue;
            }
            Some(Command::Update(project)) => {
                // Edits that arrived while the last one was being made wait in the channel:
                // only the newest is made. A command after them is handled next.
                let mut project = project;
                while let Ok(next) = rx.try_recv() {
                    match next {
                        Command::Update(newer) => project = newer,
                        other => {
                            pending = Some(other);
                            break;
                        }
                    }
                }
                if let Some(mut d) = drape.take() {
                    if d.made.same_fabric(&project) {
                        // Only pins or placements changed: the cloth carries on as it is.
                        d.made.set_pins(project);
                        drape = Some(d);
                    } else {
                        seq += 1;
                        let number = d.number;
                        drape = start_drape(status, number, || {
                            let mut remade = Drape::new(number, d.made.rebuilt(project, stage));
                            let first = remade.frame(seq, 0.0);
                            first.map(|f| (remade, f))
                        });
                        status.remade.fetch_add(1, Ordering::AcqRel);
                    }
                }
                settled_frames = 0;
                on_frame();
                next = Instant::now();
                continue;
            }
            Some(Command::Grab {
                fabric,
                triangle,
                bary,
                target,
            }) => {
                if let Some(d) = &mut drape
                    && Arc::ptr_eq(&fabric, &d.made.fabric)
                {
                    let cloth = d.made.solver.cloth_mut();
                    if let Some(old) = d.grab.take() {
                        cloth.detach(old);
                    }
                    d.grab = cloth.attach(triangle, bary, target, GRAB_COMPLIANCE);
                }
                settled_frames = 0;
                next = Instant::now();
                continue;
            }
            Some(Command::Pull(target)) => {
                if let Some(d) = &mut drape
                    && let Some(id) = d.grab
                {
                    d.made.solver.cloth_mut().move_attachment(id, target);
                }
                settled_frames = 0;
                continue;
            }
            Some(Command::Release) => {
                if let Some(d) = &mut drape
                    && let Some(id) = d.grab.take()
                {
                    d.made.solver.cloth_mut().detach(id);
                }
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
            None => {}
        }
        let Some(d) = &mut drape else { continue };
        let started = Instant::now();
        let collider = stage.drape_collider();
        let stepped = guarded(|| d.made.solver.step(Some(&collider))).is_some();
        seq += 1;
        let ms = started.elapsed().as_secs_f64() * 1000.0;
        let frame = if stepped { d.frame(seq, ms) } else { None };
        let Some(frame) = frame else {
            // Something pulled the cloth apart (or the solver panicked): drop the drape and say so.
            status.fail(d.number);
            drape = None;
            on_frame();
            continue;
        };
        let cloth = d.made.solver.cloth();
        // A point held by the pointer is never settled: the pointer may pull it again.
        settled_frames = if d.grab.is_none()
            && !cloth.has_open_stitches()
            && cloth.kinetic_energy() < SETTLED_ENERGY
        {
            settled_frames + 1
        } else {
            0
        };
        if settled_frames >= SETTLE_FRAMES {
            status.playing.store(false, Ordering::Relaxed);
            settled_frames = 0;
        }
        status.latest.store(Some(Arc::new(frame)));
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

Run: `cargo nextest run -p opendrape --lib sim_runner`
Expected: all pass, including `edits_that_arrive_while_the_fabric_is_made_again_are_made_once_the_newest` (made at most twice).

- [ ] **Step 5: Edits while draping carry the drape on**

In `crates/app/tests/ui.rs`, replace `editing_the_pattern_while_draped_returns_to_arranging`: the spec makes edits carry the drape on, so the M4a test's expectation is now wrong.

```rust
#[test]
fn editing_the_pattern_while_draped_carries_the_drape_on() {
    let dir = tempfile::tempdir().unwrap();
    let mut h = harness(dir.path(), SharedState::default());
    h.run();
    add_sewn_pieces(&mut h);
    h.get_by_label("Play").click();
    h.run_steps(2);
    wait_until(&mut h, "the drape", |a| {
        a.sim_frame().is_some_and(|f| f.time > 0.2)
    });
    let before = h.state().sim_frame().unwrap();
    // The back made 60 mm longer at its hem, as in the pattern window.
    h.state_mut().editor_mut().doc.edit(|p| {
        for v in 0..2 {
            let at = p.pieces[1].vertices[v].pos;
            p.pieces[1].move_vertex(v, at - Point2::new(0.0, 60.0));
        }
    });
    h.run_steps(2);
    wait_until(&mut h, "the longer drape", |a| {
        a.sim_frame()
            .is_some_and(|f| f.positions.len() > before.positions.len())
    });
    let after = h.state().sim_frame().unwrap();
    assert!(h.state().is_draping(), "still draping: no Reset");
    assert_eq!(after.drape, before.drape, "the same drape, carried on");
    h.get_by_label("Press Reset to move pieces.");
    // Undo is an edit too: the drape carries on with the shorter back again.
    h.state_mut().editor_mut().undo();
    h.run_steps(2);
    wait_until(&mut h, "the shorter drape", |a| {
        a.sim_frame()
            .is_some_and(|f| f.positions.len() == before.positions.len())
    });
    assert!(h.state().is_draping());
}
```

Run: `cargo nextest run -p opendrape --test ui editing_the_pattern`
Expected: FAIL (the edit still returns to arranging).

In `crates/app/src/app.rs`:
- The `draped` field's comment becomes `/// The project the drape was last given (at Play, or since by an edit while draping).`
- Replace `reset_if_edited` with:

```rust
    /// A change to the project while draping (a pattern edit, a seam, a pin, an undo or a
    /// redo) carries the drape on with it: see [`SimRunner::update`].
    fn update_if_edited(&mut self) {
        let Some(runner) = &self.runner else { return };
        if !runner.is_draping() {
            self.draped = None;
            return;
        }
        let project = self.editor.doc.project();
        if self.draped.as_ref().is_some_and(|d| **d != *project) {
            let snapshot = Arc::new(project.clone());
            runner.update(snapshot.clone());
            self.draped = Some(snapshot);
        }
    }
```

- In `view_3d`, `self.reset_if_edited();` becomes `self.update_if_edited();`.

Run: `cargo nextest run -p opendrape`
Expected: all pass.

- [ ] **Step 6: Commit**

```bash
cargo nextest run --workspace
cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings
git add Cargo.toml crates/drape crates/app
git commit -m "feat(drape): pins hold the cloth; edits while draping carry the drape on

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 7: The Free Sew tool (F) (review: full)

**Files:**
- Create:
  - `crates/app/src/editor/free_sew.rs`
  - `crates/app/tests/free_sew.rs`
- Modify:
  - `crates/app/src/editor/{mod.rs,canvas.rs,sew_tool.rs,panel.rs,paint.rs}`
  - `crates/app/i18n/en-US/opendrape.ftl`
  - `crates/app/tests/editor.rs` (Fit is Cmd+0 now)

**Interfaces:**
- Consumes:
  - Task 1's `SeamSide::{tidy, spans, overlaps}`, `Project::side_length`, `MIN_SIDE_MM`, `Shape::{outline_pos, point_at, stored_len}`, `geom::{side_runs, side_points}`;
  - M4a's `sew_tool::on_a_fold_line`, `seam_at`, `Selection::Seam`, `Document::{edit, last_change_refused}`, `notice-sew-fold`/`notice-mirror-sewn`;
  - `geom::{nearest_edge, point_at_distance, edge_length, outline_points}`.
- Produces:
  - `Tool::FreeSew` (key F; `Tool::ALL` has 8) and `pub const editor::FIT: KeyboardShortcut` (Cmd+0 / Ctrl+0).
  - In `editor::free_sew`:
    - `pub(crate) struct OnOutline { shape, half, pos: OutlinePos, at: Point2, outline_edges }`;
    - `pub(super) struct FreeDraft { start: OnOutline, a: Option<SeamSide>, b_start: Option<OnOutline> }`;
    - `pub(crate) fn point_under(&Project, w, tol) -> Option<OnOutline>`;
    - `pub(crate) enum NoSide { OtherPiece, Fold, TooShort, Sewn }`;
    - `pub(crate) fn side_between(&Project, &OnOutline, &OnOutline, long: bool, besides: Option<&SeamSide>) -> Result<SeamSide, NoSide>`.
  - `PatternEditor::{free_sew_tool, drop_stale_free_sew, free_sew_drawing}`, `CanvasState.{free, shift}`.
  - `sew_tool::on_a_fold_line` becomes `pub(super)`. `has_half_made_seam`/`cancel_half_made_seam` cover a Free Sew draft too.

**Behaviour:**
- **Clicks:** start of side a, end of side a, start of side b, end of side b. The fourth click makes the seam (one undo step) and selects it.
- **Snapping:** a click within 8 screen points of an outline lands on the nearest point of the nearest edge, snapped to that edge's start, end, middle or a notch on it when one is within 8 points of the click. A click on a fold line (and no outline) shows the fold notice, and a click away from every outline does nothing.
- **Which way round:** a side runs the shorter way round its outline (measured with `Project::side_length`), or the longer with Shift held on its end click. Ends at corners are tidied onto the edge the side covers.
- **Refusals:**
  - an end on another shape or half: "End the side on the same piece (and the same half) it started on.";
  - 1 mm or less, or the same point twice: "That side is too short to sew: pick points more than 1 mm apart.";
  - over the fold: the fold notice;
  - overlapping any sewn side, mirror images included, or side a: "Part of this is already sewn.";
  - a seam whose mirror image would overlap: the M4a mirror notice.

  After a refused end, the draft keeps its start.
- **Drawing:** the first side's piece is outlined, a made side is drawn thick and the side so far thinner, with a ring at each start and a dot where a click would land.
- **Cancelling:** Esc, undo, a tool change, or a change that removes a picked point's piece or changes its number of edges drops the draft.
- **A seam line under the pointer** is picked first while no seam is being sewn (it selects the seam, as in the Sew tool), and Delete removes a selected seam.
- **Fit** is Cmd+0 / Ctrl+0. The toolbar button shows the shortcut as egui writes it for the platform ("Fit (⌘0)" on a Mac).

- [ ] **Step 1: Strings**

In `crates/app/i18n/en-US/opendrape.ftl`, `toolbar-fit = Fit (F)` becomes `toolbar-fit = Fit` (the button adds the shortcut), and after `notice-mirror-sewn = …` add:

```
tool-free-sew = Free sew
tool-free-sew-tip = Sew part of an edge, or across corners: click where each side of the seam starts and ends.
hint-free-start = Click where the seam starts on a piece's outline. Points snap to corners, edge middles and notches.
hint-free-end = Click where this side ends; it runs the shorter way round. Shift-click to go the long way round. Esc cancels.
hint-free-second = Click where the other side starts: it meets this side's start.
hint-free-second-end = Click where the other side ends to sew the seam. Shift-click to go the long way round. Esc cancels.
notice-free-sewn = Part of this is already sewn.
notice-free-same-piece = End the side on the same piece (and the same half) it started on.
notice-free-short = That side is too short to sew: pick points more than 1 mm apart.
```

- [ ] **Step 2: Failing tests**

Create `crates/app/tests/free_sew.rs`:

```rust
//! M4b on the pattern table: the Free Sew tool (F), driven the way a student would.

mod common;
use common::*;
use egui::Key;
use egui_kittest::kittest::Queryable;
use opendrape::editor::{Selection, Tool};
use opendrape_core::{Edge, Half, Notch, OutlinePos, Piece, PieceId, Point2, Seam, SeamSide};

fn p(x: f64, y: f64) -> Point2 {
    Point2::new(x, y)
}

/// A sleeve: hem (20,0)–(320,0), underarm edges up to (340,130) and (0,130), and its cap (edge
/// 2, from (340,130) round to (0,130)) curved up to a notch at its middle, (170, 220). With its
/// mirror image when `paired`.
fn with_sleeve(h: &mut H, paired: bool) -> (PieceId, Option<PieceId>) {
    let sleeve = h.state_mut().doc.edit(|pr| {
        let mut s = Piece::polygon(
            PieceId(0),
            "Sleeve",
            &[p(20.0, 0.0), p(320.0, 0.0), p(340.0, 130.0), p(0.0, 130.0)],
        );
        s.edges[2] = Edge::Curve {
            c1: p(290.0, 250.0),
            c2: p(50.0, 250.0),
        };
        let cap = opendrape_geom::edge_length(&s, 2);
        s.notches = vec![Notch::new(2, cap / 2.0)];
        pr.add_piece(s)
    });
    let twin = paired.then(|| {
        h.state_mut()
            .doc
            .edit(|pr| pr.add_twin(sleeve, "Sleeve (mirror)".into(), p(1700.0, 0.0)))
            .unwrap()
    });
    h.run();
    (sleeve, twin)
}

/// A 250 × 400 mm front at (500,0): edges 0 bottom, 1 right (the armhole), 2 top, 3 left; cut
/// on the fold (its left edge, x = 500) when `folded`.
fn with_front(h: &mut H, folded: bool) -> PieceId {
    let id = h.state_mut().doc.edit(|pr| {
        let mut f = Piece::rectangle(PieceId(0), "Front", p(500.0, 0.0), 250.0, 400.0);
        f.fold = folded.then_some(3);
        pr.add_piece(f)
    });
    h.state_mut().fit();
    h.run();
    id
}

fn seams(h: &H) -> Vec<Seam> {
    h.state().doc.project().seams.clone()
}

fn notice(h: &H) -> Option<String> {
    h.state().notice.clone()
}

/// Half the cap: from the sleeve's left underarm corner (0,130), back along the cap to its
/// notch.
fn cap_half(sleeve: PieceId) -> SeamSide {
    SeamSide {
        shape: sleeve,
        half: Half::Drawn,
        from: OutlinePos::new(2, 1.0),
        to: OutlinePos::new(2, 0.5),
        forward: false,
    }
}

#[test]
fn four_clicks_sew_half_a_sleeve_cap_into_an_armhole() {
    let mut h = harness();
    let (sleeve, _) = with_sleeve(&mut h, false);
    let front = with_front(&mut h, false);
    key(&mut h, Key::F);
    assert_eq!(h.state().tool, Tool::FreeSew);
    h.get_by_label_contains("Click where the seam starts");
    click(&mut h, 0.0, 130.0); // the cap's left end
    click(&mut h, 172.0, 221.0); // near the notch: it snaps there
    assert!(seams(&h).is_empty(), "one side so far");
    click(&mut h, 750.0, 0.0); // the front's right edge, from its bottom...
    click(&mut h, 750.0, 400.0); // ...to its top: the seam
    assert_eq!(
        seams(&h),
        vec![Seam {
            id: opendrape_core::SeamId(1),
            a: cap_half(sleeve),
            b: SeamSide::edges(front, Half::Drawn, 1, 1, true),
        }]
    );
    assert_eq!(
        h.state().selection,
        Selection::Seam(opendrape_core::SeamId(1))
    );
    cmd(&mut h, Key::Z);
    assert!(seams(&h).is_empty(), "one undo step");
}

#[test]
fn shift_takes_the_long_way_round() {
    let mut h = harness();
    let (sleeve, _) = with_sleeve(&mut h, false);
    with_front(&mut h, false);
    key(&mut h, Key::F);
    click(&mut h, 0.0, 130.0);
    shift_click(&mut h, 172.0, 221.0);
    click(&mut h, 750.0, 0.0);
    click(&mut h, 750.0, 400.0);
    // From the left underarm corner down the left underarm, along the hem, up the right one
    // and round the cap's right half to the notch.
    let a = seams(&h)[0].a;
    assert_eq!(
        (a.from, a.to, a.forward),
        (OutlinePos::new(3, 0.0), OutlinePos::new(2, 0.5), true)
    );
    assert_eq!(a.shape, sleeve);
}

#[test]
fn the_second_half_of_the_cap_meets_the_first_at_the_notch() {
    let mut h = harness();
    let (sleeve, _) = with_sleeve(&mut h, false);
    let front = with_front(&mut h, false);
    let back = h.state_mut().doc.edit(|pr| {
        pr.add_piece(Piece::rectangle(
            PieceId(0),
            "Back",
            p(900.0, 0.0),
            250.0,
            400.0,
        ))
    });
    h.state_mut().fit();
    h.run();
    key(&mut h, Key::F);
    for (x, y) in [(0.0, 130.0), (170.0, 220.0), (750.0, 0.0), (750.0, 400.0)] {
        click(&mut h, x, y);
    }
    // On from the notch to the cap's right end, to the back's left edge from its top down.
    for (x, y) in [(170.0, 220.0), (340.0, 130.0), (900.0, 400.0), (900.0, 0.0)] {
        click(&mut h, x, y);
    }
    assert_eq!(notice(&h), None, "they meet at the notch: no overlap");
    let made = seams(&h);
    assert_eq!(made.len(), 2);
    assert_eq!(made[1].a.from, OutlinePos::new(2, 0.5), "from the notch");
    assert_eq!(
        made[1].a.to,
        OutlinePos::new(2, 0.0),
        "to the cap's right end"
    );
    assert_eq!(made[1].b, SeamSide::edges(back, Half::Drawn, 3, 3, true));
    assert_eq!(made[0].b.shape, front);
    assert_eq!(made[1].a.shape, sleeve);
}

#[test]
fn a_side_over_a_sewn_stretch_or_across_the_fold_is_refused() {
    let mut h = harness();
    with_sleeve(&mut h, false);
    with_front(&mut h, true); // folded on its left edge: its pale half covers x 250..500
    key(&mut h, Key::F);
    for (x, y) in [(0.0, 130.0), (170.0, 220.0), (750.0, 0.0), (750.0, 400.0)] {
        click(&mut h, x, y);
    }
    assert_eq!(seams(&h).len(), 1);
    // Part of the cap again (from three quarters of the way along it to its left end):
    // refused.
    let on_cap = {
        let sleeve = &h.state().doc.project().pieces[0];
        let cap = opendrape_geom::edge_length(sleeve, 2);
        opendrape_geom::point_at_distance(sleeve, 2, 0.75 * cap)
    };
    click(&mut h, on_cap.x, on_cap.y);
    click(&mut h, 0.0, 130.0);
    assert_eq!(notice(&h).as_deref(), Some("Part of this is already sewn."));
    key(&mut h, Key::Escape);
    // The fold line itself.
    click(&mut h, 500.0, 200.0);
    assert_eq!(
        notice(&h).as_deref(),
        Some("The fold line is inside the piece and can't be sewn.")
    );
    // A side that ends on another piece.
    click(&mut h, 20.0, 0.0);
    click(&mut h, 600.0, 0.0);
    assert_eq!(
        notice(&h).as_deref(),
        Some("End the side on the same piece (and the same half) it started on.")
    );
    assert_eq!(seams(&h).len(), 1);
}

#[test]
fn mirror_images_of_free_seams_appear_by_themselves() {
    let mut h = harness();
    let (sleeve, twin) = with_sleeve(&mut h, true);
    let front = with_front(&mut h, true);
    key(&mut h, Key::F);
    for (x, y) in [(0.0, 130.0), (170.0, 220.0), (750.0, 0.0), (750.0, 400.0)] {
        click(&mut h, x, y);
    }
    let all = h.state().doc.project().all_seams();
    assert_eq!(all.len(), 2);
    let (mirror, derived) = all[1];
    assert!(derived);
    assert_eq!(
        mirror.a,
        SeamSide {
            shape: twin.unwrap(),
            ..cap_half(sleeve)
        }
    );
    assert_eq!(mirror.b, SeamSide::edges(front, Half::Pale, 1, 1, true));
    // The twin's cap half is sewn now: picking it again is refused.
    let twin_left_end = 1700.0; // the twin shows stored (x, y) at (1700 - x, y)
    click(&mut h, twin_left_end, 130.0);
    click(&mut h, 1700.0 - 172.0, 221.0);
    assert_eq!(notice(&h).as_deref(), Some("Part of this is already sewn."));
}

#[test]
fn escape_and_undo_cancel_a_half_made_free_seam() {
    let mut h = harness();
    with_sleeve(&mut h, false);
    with_front(&mut h, false);
    key(&mut h, Key::F);
    click(&mut h, 0.0, 130.0);
    click(&mut h, 170.0, 220.0);
    key(&mut h, Key::Escape);
    h.get_by_label_contains("Click where the seam starts");
    click(&mut h, 0.0, 130.0);
    assert!(h.state().can_undo(), "the half-made seam");
    cmd(&mut h, Key::Z);
    h.get_by_label_contains("Click where the seam starts");
    // A point added to the sleeve renumbers its edges: a half-made seam on it is dropped.
    click(&mut h, 0.0, 130.0);
    let sleeve = h.state().doc.project().pieces[0].id;
    h.state_mut()
        .doc
        .edit(|pr| opendrape_geom::split_edge_in(pr, sleeve, 0, 0.5));
    h.run();
    h.get_by_label_contains("Click where the seam starts");
}

#[test]
fn a_seam_line_is_picked_with_free_sew_and_delete_removes_it() {
    let mut h = harness();
    with_sleeve(&mut h, false);
    with_front(&mut h, false);
    key(&mut h, Key::F);
    for (x, y) in [(0.0, 130.0), (170.0, 220.0), (750.0, 0.0), (750.0, 400.0)] {
        click(&mut h, x, y);
    }
    h.state_mut().selection = Selection::None;
    h.run();
    // The seam's line runs 5 screen points inside the front's right edge.
    let x = 750.0 - 5.0 / h.state().view.zoom;
    click(&mut h, x, 200.0);
    assert!(matches!(h.state().selection, Selection::Seam(_)));
    key(&mut h, Key::Delete);
    assert!(seams(&h).is_empty());
}

#[test]
fn the_same_spot_clicked_for_both_ends_is_refused_not_sewn_all_the_way_round() {
    let mut h = harness();
    with_sleeve(&mut h, false);
    with_front(&mut h, false);
    key(&mut h, Key::F);
    // A corner twice (it is the end of one edge and the start of the next), and a notch twice.
    for (x, y) in [(340.0, 130.0), (170.0, 220.0)] {
        click(&mut h, x, y);
        click(&mut h, x, y);
        assert_eq!(
            notice(&h).as_deref(),
            Some("That side is too short to sew: pick points more than 1 mm apart.")
        );
        key(&mut h, Key::Escape);
    }
    assert!(seams(&h).is_empty());
}
```

In `crates/app/tests/editor.rs`'s `fit_shows_every_piece`, `key(&mut h, Key::F);` becomes:

```rust
    cmd(&mut h, Key::Num0); // F is the Free Sew tool's now
```

Run: `cargo nextest run -p opendrape --test free_sew --test editor`
Expected: compile errors (no `Tool::FreeSew`).

- [ ] **Step 3: The tool**

Create `crates/app/src/editor/free_sew.rs` (its unit tests include `side_between`'s same-point case):

```rust
//! The Free Sew tool (F): a seam between any two points of an outline, as a sleeve cap is sewn
//! into an armhole. Click where the first side starts on a piece, then where it ends; then
//! where the second side starts and ends, which makes the seam (its start meets the first
//! side's start). A side runs the shorter way round between its ends; Shift-click its end to
//! take the long way. Points snap to the corners, the middle and the notches of the edge under
//! the pointer. Esc cancels; clicking a seam's line selects the seam.

use super::{PatternEditor, Selection};
use crate::tr;
use egui::Response;
use opendrape_core::{
    Half, MIN_SIDE_MM, ModelError, OutlinePos, PieceId, Point2, Project, SeamSide,
};
use opendrape_geom as geom;

/// A point on a shape's outline, as the Free Sew tool picks it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct OnOutline {
    pub shape: PieceId,
    pub half: Half,
    /// Where it is on the stored piece.
    pub pos: OutlinePos,
    /// Where it is on the shape (mm).
    pub at: Point2,
    /// How many edges the stored piece had: a point added or removed since makes `pos` name
    /// another place.
    pub outline_edges: usize,
}

/// A seam being sewn with the Free Sew tool.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct FreeDraft {
    /// Where the first side starts.
    pub start: OnOutline,
    /// The first side, once its end is picked.
    pub a: Option<SeamSide>,
    /// Where the second side starts, once picked.
    pub b_start: Option<OnOutline>,
}

/// The point of an outline under `w` within `tol` mm: on the nearest edge of any shape, snapped
/// to that edge's start, end, middle or a notch on it when one is within `tol` of `w`.
pub(crate) fn point_under(project: &Project, w: Point2, tol: f64) -> Option<OnOutline> {
    let shapes = geom::shapes(project);
    let (shape, j, t) = shapes
        .iter()
        .filter_map(|s| {
            let (j, t, d) = geom::nearest_edge(&s.piece, w)?;
            (d <= tol).then_some((s, j, t, d))
        })
        .min_by(|a, b| a.3.total_cmp(&b.3))
        .map(|(s, j, t, _)| (s, j, t))?;
    let piece = &shape.piece;
    let len = geom::edge_length(piece, j);
    let notches = piece
        .notches
        .iter()
        .filter(|n| n.edge == j)
        .map(|n| n.distance);
    let snapped = [0.0, len, len / 2.0]
        .into_iter()
        .chain(notches)
        .map(|d| (d, geom::point_at_distance(piece, j, d).distance(w)))
        .filter(|(_, gap)| *gap <= tol)
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(d, _)| d);
    let along = snapped.unwrap_or_else(|| geom::distance_along(piece, j, t));
    let (half, pos) = shape.outline_pos(j, along);
    Some(OnOutline {
        shape: shape.id,
        half,
        pos,
        at: geom::point_at_distance(piece, j, along),
        outline_edges: shape.stored_len(),
    })
}

/// Why a side can't be made between two points.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum NoSide {
    /// The end is on another shape, or the other half of a fold.
    OtherPiece,
    /// The way chosen round the outline crosses the fold line.
    Fold,
    /// It would be 1 mm long or less.
    TooShort,
    /// Part of it is sewn already.
    Sewn,
}

/// The side from `start` to `end` on the same shape and half: the shorter way round, or the
/// longer with `long`. It must cross no fold, be longer than [`MIN_SIDE_MM`], and share no more
/// than a point with a seam sewn already (mirror images included) or with `besides`.
pub(crate) fn side_between(
    project: &Project,
    start: &OnOutline,
    end: &OnOutline,
    long: bool,
    besides: Option<&SeamSide>,
) -> Result<SeamSide, NoSide> {
    if (start.shape, start.half) != (end.shape, end.half) {
        return Err(NoSide::OtherPiece);
    }
    // The same point twice is no side (not the whole outline round to it).
    if start.at.distance(end.at) <= 1e-6 {
        return Err(NoSide::TooShort);
    }
    let shape = geom::shape_of(project, start.shape).ok_or(NoSide::OtherPiece)?;
    let n = shape.stored_len();
    let way = |forward| {
        SeamSide {
            shape: start.shape,
            half: start.half,
            from: start.pos,
            to: end.pos,
            forward,
        }
        .tidy(n)
    };
    // Measured round the stored outline, fold edge and all, so that the shorter way is the
    // shorter even when it would cross the fold (and so be refused).
    let length = |side: &SeamSide| project.side_length(side).unwrap_or(0.0);
    let (forward, backward) = (way(true), way(false));
    let side = if (length(&forward) <= length(&backward)) != long {
        forward
    } else {
        backward
    };
    if side.spans(n).is_empty() || project.side_length(&side).is_some_and(|l| l <= MIN_SIDE_MM) {
        return Err(NoSide::TooShort);
    }
    if geom::side_runs(&shape, &side).is_none() {
        return Err(NoSide::Fold);
    }
    let sewn = project
        .all_seams()
        .into_iter()
        .flat_map(|(s, _)| [s.a, s.b])
        .chain(besides.copied());
    let len_of = |id| project.owner(id).map_or(0, |(p, _)| p.len());
    if sewn
        .into_iter()
        .any(|other| other.overlaps(&side, len_of(other.shape)))
    {
        return Err(NoSide::Sewn);
    }
    Ok(side)
}

/// The notice for a side that can't be made.
fn refusal(why: NoSide) -> String {
    match why {
        NoSide::OtherPiece => tr!("notice-free-same-piece"),
        NoSide::Fold => tr!("notice-sew-fold"),
        NoSide::TooShort => tr!("notice-free-short"),
        NoSide::Sewn => tr!("notice-free-sewn"),
    }
}

/// Whether `point` still names the place it was picked at in `project`.
fn still_there(project: &Project, point: &OnOutline) -> bool {
    geom::shape_of(project, point.shape).is_some_and(|s| {
        s.stored_len() == point.outline_edges && s.point_at(point.half, point.pos).is_some()
    })
}

impl PatternEditor {
    /// Drops the Free Sew draft if what it points at has changed (an undo, a redo, a deleted
    /// piece, a point added or removed).
    pub(super) fn drop_stale_free_sew(&mut self) {
        let project = self.doc.project();
        self.canvas.free = self.canvas.free.filter(|d| {
            still_there(project, &d.start) && d.b_start.is_none_or(|b| still_there(project, &b))
        });
    }

    pub(super) fn free_sew_tool(
        &mut self,
        response: &Response,
        pointer: Option<Point2>,
        tol: f64,
        shift: bool,
    ) {
        self.drop_stale_free_sew();
        if !response.clicked() {
            return;
        }
        let Some(w) = pointer else { return };
        // A click on a seam's line selects that seam (unless a seam is being sewn).
        if self.canvas.free.is_none()
            && let Some(seam) = self.seam_at(w, tol)
        {
            self.selection = Selection::Seam(seam);
            return;
        }
        let project = self.doc.project();
        let Some(hit) = point_under(project, w, tol) else {
            if super::sew_tool::on_a_fold_line(project, w, tol) {
                self.notice = Some(tr!("notice-sew-fold"));
            }
            return;
        };
        match self.canvas.free {
            None => {
                self.canvas.free = Some(FreeDraft {
                    start: hit,
                    a: None,
                    b_start: None,
                });
            }
            Some(d @ FreeDraft { a: None, .. }) => {
                match side_between(project, &d.start, &hit, shift, None) {
                    Ok(a) => {
                        self.canvas.free = Some(FreeDraft { a: Some(a), ..d });
                    }
                    Err(why) => self.notice = Some(refusal(why)),
                }
            }
            Some(d @ FreeDraft { b_start: None, .. }) => {
                self.canvas.free = Some(FreeDraft {
                    b_start: Some(hit),
                    ..d
                });
            }
            Some(FreeDraft {
                a: Some(a),
                b_start: Some(b_start),
                ..
            }) => match side_between(project, &b_start, &hit, shift, Some(&a)) {
                Ok(b) => self.make_free_seam(a, b),
                Err(why) => self.notice = Some(refusal(why)),
            },
        }
    }

    /// Sews `a` to `b`, as one undo step.
    fn make_free_seam(&mut self, a: SeamSide, b: SeamSide) {
        let id = self.doc.edit(|p| p.add_seam(a, b));
        if self.doc.last_change_refused() {
            // Both sides are free (they were checked), so what refuses a seam is that its
            // mirror image would sew outline that is sewn already; or, at the very limit, there
            // are too many seams.
            self.notice = Some(match self.doc.last_refusal() {
                Some(ModelError::BadSeam(_)) => tr!("notice-mirror-sewn"),
                _ => tr!("notice-refused"),
            });
            return;
        }
        self.canvas.free = None;
        self.selection = Selection::Seam(id);
    }

    /// What to draw while a seam is sewn with the Free Sew tool: the first side once made, and
    /// the side so far (its start, to the point under the pointer the way the next click would
    /// take it).
    pub(super) fn free_sew_drawing(&self, shift: bool) -> (Option<SeamSide>, Option<SeamSide>) {
        let Some(d) = self.canvas.free else {
            return (None, None);
        };
        let project = self.doc.project();
        let tol = self.view.mm(super::HIT_PX);
        let under = self
            .canvas
            .cursor
            .and_then(|w| point_under(project, w, tol));
        let start = if d.a.is_none() {
            Some(d.start)
        } else {
            d.b_start
        };
        let so_far = start
            .zip(under)
            .and_then(|(s, e)| side_between(project, &s, &e, shift, d.a.as_ref()).ok());
        (d.a, so_far)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use opendrape_core::{Notch, Piece};

    fn p(x: f64, y: f64) -> Point2 {
        Point2::new(x, y)
    }

    /// A 300 × 200 piece at the origin with a notch 100 mm along its top edge (which runs from
    /// (300,200) to (0,200)).
    fn piece() -> Project {
        let mut pr = Project::new();
        let mut a = Piece::rectangle(PieceId(0), "A", p(0.0, 0.0), 300.0, 200.0);
        a.notches = vec![Notch::new(2, 100.0)];
        pr.add_piece(a);
        pr
    }

    #[test]
    fn points_snap_to_corners_middles_and_notches() {
        let pr = piece();
        let at = |x, y| point_under(&pr, p(x, y), 5.0).unwrap();
        assert_eq!(at(2.0, -1.0).at, p(0.0, 0.0), "a corner");
        assert_eq!(at(148.0, 1.0).at, p(150.0, 0.0), "the bottom edge's middle");
        assert_eq!(at(203.0, 199.0).at, p(200.0, 200.0), "the notch");
        assert_eq!(at(203.0, 199.0).pos, OutlinePos::new(2, 1.0 / 3.0));
        assert_eq!(
            at(60.0, 1.0).at,
            p(60.0, 0.0),
            "anywhere else: under the pointer"
        );
        assert!(
            point_under(&pr, p(150.0, 100.0), 5.0).is_none(),
            "inside, off the outline"
        );
    }

    #[test]
    fn a_side_runs_the_shorter_way_round_unless_shift_asks_for_the_longer() {
        let pr = piece();
        let at = |x, y| point_under(&pr, p(x, y), 1.0).unwrap();
        // From the bottom-right corner to halfway up the right edge: 100 mm one way, 900 the
        // other.
        let (start, end) = (at(300.0, 0.0), at(300.0, 100.0));
        let short = side_between(&pr, &start, &end, false, None).unwrap();
        assert!((pr.side_length(&short).unwrap() - 100.0).abs() < 0.01);
        assert!(short.forward);
        let long = side_between(&pr, &start, &end, true, None).unwrap();
        assert!((pr.side_length(&long).unwrap() - 900.0).abs() < 0.01);
        assert!(!long.forward);
        assert_eq!(
            side_between(&pr, &start, &start, false, None),
            Err(NoSide::TooShort)
        );
    }

    #[test]
    fn a_side_over_sewn_outline_is_refused() {
        let mut pr = piece();
        let b = pr.add_piece(Piece::rectangle(
            PieceId(0),
            "B",
            p(500.0, 0.0),
            200.0,
            200.0,
        ));
        let a = pr.pieces[0].id;
        pr.add_seam(
            SeamSide {
                to: OutlinePos::new(0, 0.5),
                ..SeamSide::edges(a, Half::Drawn, 0, 0, true)
            },
            SeamSide::edges(b, Half::Drawn, 3, 3, false),
        );
        let at = |x, y| point_under(&pr, p(x, y), 1.0).unwrap();
        // The bottom edge is sewn from x = 0 to 150: from 100 to 200 overlaps it, from 150 on
        // only meets it.
        assert_eq!(
            side_between(&pr, &at(100.0, 0.0), &at(200.0, 0.0), false, None),
            Err(NoSide::Sewn)
        );
        assert!(side_between(&pr, &at(150.0, 0.0), &at(250.0, 0.0), false, None).is_ok());
        assert_eq!(
            side_between(&pr, &at(150.0, 0.0), &at(600.0, 0.0), false, None),
            Err(NoSide::OtherPiece)
        );
    }
}
```

In `crates/app/src/editor/mod.rs`:
- Add `mod free_sew;` after `mod document;`.
- After `const REDO_Y: …;`:

```rust
/// Show every piece: Cmd+0 (Ctrl+0 on Windows). F is the Free Sew tool's.
pub const FIT: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::Num0);
```

- Add `FreeSew,` to `enum Tool` after `Sew,`; `ALL` becomes `[Self; 8]` with `Self::FreeSew,` last; `key()` gets `Self::FreeSew => Key::F,`; `label()` gets `Self::FreeSew => tr!("tool-free-sew"),`; `tip()` gets `Self::FreeSew => tr!("tool-free-sew-tip"),`.
- In `undo`, `redo` and `ui`, after each `self.drop_stale_sew();` add `self.drop_stale_free_sew();`. `undo`'s comment ends "while a seam is half made (with either Sew tool), cancels that."
- In the tool-key handling, `if ui.input_mut(|i| i.consume_key(Modifiers::NONE, Key::F)) {` becomes `if ui.input_mut(|i| i.consume_shortcut(&FIT)) {`.
- In `toolbar`, the Fit button becomes:

```rust
            let fit = format!(
                "{} ({})",
                tr!("toolbar-fit"),
                ui.ctx().format_shortcut(&FIT)
            );
            if ui
                .button(fit)
```

  (followed by the existing `.on_hover_text(tr!("toolbar-fit-tip"))`).

In `crates/app/src/editor/canvas.rs`:
- In `CanvasState`, after `pub sew: …,` and after `pub menu_for: …,` respectively:

```rust
    /// Free Sew tool: the seam being sewn.
    pub free: Option<super::free_sew::FreeDraft>,
```

```rust
    /// Shift is held (the Free Sew tool shows the long way round then).
    pub shift: bool,
```

- After `let shift = ui.input(|i| i.modifiers.shift);` add `self.canvas.shift = shift;`. In the `cursor` match, the last arm becomes `Tool::Edit | Tool::AddPoint | Tool::Notch | Tool::Line | Tool::Sew | Tool::FreeSew => w,`.
- In the tool dispatch, after the `Tool::Sew` arm: `Tool::FreeSew => self.free_sew_tool(&response, pointer, tol, shift),`.
- In the key handling, `Tool::Sew => {` becomes `Tool::Sew | Tool::FreeSew => {`, and under `if pressed(Key::Escape) {` add `self.canvas.free = None;` after `self.canvas.sew = None;`.

In `crates/app/src/editor/sew_tool.rs`:
- `fn on_a_fold_line` becomes `pub(super) fn on_a_fold_line`.
- Replace `has_half_made_seam` and `cancel_half_made_seam`:

```rust
    /// A seam with only its first side picked (or, with the Free Sew tool, any of its points),
    /// which is not in the project yet.
    pub(super) fn has_half_made_seam(&self) -> bool {
        self.canvas.sew.is_some_and(|d| d.seam.is_none()) || self.canvas.free.is_some()
    }

    /// Drops a half-made seam; true if there was one.
    pub(super) fn cancel_half_made_seam(&mut self) -> bool {
        let free = self.canvas.free.take().is_some();
        self.canvas.sew.take_if(|d| d.seam.is_none()).is_some() || free
    }
```

In `crates/app/src/editor/panel.rs`'s hint, after the `Tool::Sew` arm:

```rust
            Tool::FreeSew => match self.canvas.free {
                None => tr!("hint-free-start"),
                Some(d) if d.a.is_none() => tr!("hint-free-end"),
                Some(d) if d.b_start.is_none() => tr!("hint-free-second"),
                Some(_) => tr!("hint-free-second-end"),
            },
```

In `crates/app/src/editor/paint.rs`, before the comment `// The seam side being sewn, thick, with a ring where it starts.`:

```rust
        if self.tool == Tool::FreeSew {
            self.paint_free_sew(painter, rect, c);
        }
```

and after the function that holds it (the one drawing the sew draft):

```rust
    /// The Free Sew tool's seam so far: the piece its first side is on, outlined; the first side
    /// once made, thick; the side being picked, from its start to where the next click would
    /// end it; a ring at each start; and a dot where a click would land.
    fn paint_free_sew(&self, painter: &Painter, rect: Rect, c: &Palette) {
        let v = self.view;
        let ink = Stroke::new(1.5, c.selected);
        let project = self.doc.project();
        let side_line = |side: &opendrape_core::SeamSide, width: f32| {
            let shape = geom::shape_of(project, side.shape)?;
            let points = geom::side_points(&shape, side, v.mm(0.25))?;
            painter.add(Shape::line(
                self.screen_points(rect, points),
                Stroke::new(width, c.selected),
            ));
            Some(())
        };
        if let Some(d) = self.canvas.free {
            if let Some(shape) = geom::shape_of(project, d.start.shape) {
                let outline = geom::outline_points(&shape.piece, v.mm(0.25));
                painter.add(Shape::closed_line(
                    self.screen_points(rect, outline),
                    Stroke::new(2.5, c.selected),
                ));
            }
            for at in std::iter::once(d.start.at).chain(d.b_start.map(|b| b.at)) {
                painter.circle_stroke(v.to_screen(rect, at), 6.0, ink);
            }
        }
        let (made, so_far) = self.free_sew_drawing(self.canvas.shift);
        if let Some(a) = made {
            side_line(&a, 4.0);
        }
        if let Some(s) = so_far {
            side_line(&s, 2.0);
        }
        if let Some(w) = self.canvas.cursor
            && let Some(hit) = super::free_sew::point_under(project, w, v.mm(super::HIT_PX))
        {
            painter.circle_filled(v.to_screen(rect, hit.at), 3.5, c.selected);
        }
    }
```

- [ ] **Step 4: Run them**

Run: `cargo nextest run -p opendrape`
Expected: all pass, including:
- `four_clicks_sew_half_a_sleeve_cap_into_an_armhole`;
- `the_second_half_of_the_cap_meets_the_first_at_the_notch`;
- `the_same_spot_clicked_for_both_ends_is_refused_not_sewn_all_the_way_round`;
- `fit_shows_every_piece`.

- [ ] **Step 5: Commit**

```bash
cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings
git add crates/app
git commit -m "feat(app): the Free Sew tool sews between any two points of an outline (F; Fit is Cmd+0)

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 8: Notches in the seam panel, and free seams drawn (review: light)

**Files:**
- Modify:
  - `crates/app/src/editor/{panel.rs,seams.rs}`
  - `crates/app/i18n/en-US/opendrape.ftl`
  - `crates/app/tests/free_sew.rs`

**Interfaces:**
- Consumes: Task 1's `geom::side_notches` and the free-side `inset_side` (Task 1 already draws free sides through `side_runs`/`edge_points_between`); Task 3's matching rule (the k-th notches pair up when the counts are equal).
- Produces: the string `panel-seam-notches`. Nothing new for later tasks.

**Behaviour:**
- **The seam panel:** when a seam's sides have different numbers of notches inside them (more than 1 mm from both ends, as `side_notches` counts them), the panel shows in amber "Notches don't match: 1 on one side, 2 on the other." under the lengths.
- **The drape** lays such a seam out evenly (Task 3). There is no drape note: the spec puts the warning in the panel, and the student can see the stitches.
- **Drawing:** a free side's coloured line runs 5 points inside just the stretch it sews, round any corner inside it. The `seams.rs` test pins that.

- [ ] **Step 1: String**

In `crates/app/i18n/en-US/opendrape.ftl`, after `panel-seam-differ = …`:

```
panel-seam-notches = Notches don't match: { $one } on one side, { $other } on the other.
```

- [ ] **Step 2: Failing tests**

Append to `crates/app/tests/free_sew.rs`:

```rust
#[test]
fn the_seam_panel_says_when_the_notches_do_not_pair_up() {
    let mut h = harness();
    // The sleeve's cap has one notch, at its middle; a 400 mm edge with two notches is sewn to
    // the whole cap.
    let (sleeve, _) = with_sleeve(&mut h, false);
    let other = h.state_mut().doc.edit(|pr| {
        let mut piece = Piece::rectangle(PieceId(0), "Other", p(500.0, 0.0), 400.0, 100.0);
        piece.notches = vec![Notch::new(0, 150.0), Notch::new(0, 250.0)];
        pr.add_piece(piece)
    });
    let seam = h.state_mut().doc.edit(|pr| {
        pr.add_seam(
            SeamSide::edges(sleeve, Half::Drawn, 2, 2, true),
            SeamSide::edges(other, Half::Drawn, 0, 0, true),
        )
    });
    h.state_mut().selection = Selection::Seam(seam);
    h.run();
    h.get_by_label("Notches don't match: 1 on one side, 2 on the other.");
    // With one notch taken off the other edge, they pair up: no warning.
    h.state_mut()
        .doc
        .edit(|pr| pr.pieces[1].notches.truncate(1));
    h.run();
    assert!(h.query_by_label_contains("Notches don't match").is_none());
}
```

Append to `crates/app/src/editor/seams.rs` (it has no tests yet):

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use opendrape_core::{Half, OutlinePos, Piece, PieceId};

    #[test]
    fn a_free_side_is_drawn_along_just_the_stretch_it_sews() {
        let mut pr = Project::new();
        let a = pr.add_piece(Piece::rectangle(
            PieceId(0),
            "A",
            Point2::new(0.0, 0.0),
            200.0,
            100.0,
        ));
        let b = pr.add_piece(Piece::rectangle(
            PieceId(0),
            "B",
            Point2::new(300.0, 0.0),
            100.0,
            100.0,
        ));
        // A's bottom edge from 50 mm to 150 mm, run backwards; and round B's bottom-right corner.
        pr.add_seam(
            SeamSide {
                shape: a,
                half: Half::Drawn,
                from: OutlinePos::new(0, 0.75),
                to: OutlinePos::new(0, 0.25),
                forward: false,
            },
            SeamSide {
                shape: b,
                half: Half::Drawn,
                from: OutlinePos::new(0, 0.5),
                to: OutlinePos::new(1, 0.5),
                forward: true,
            },
        );
        let lines = seam_lines(&pr, &geom::shapes(&pr), 2.0, 0.1);
        assert_eq!(lines.len(), 1);
        let [side_a, side_b] = &lines[0].sides;
        // 2 mm inside the outline, from the side's start to its end.
        let close = |p: Point2, q: Point2| assert!(p.distance(q) < 1e-9, "{p:?} vs {q:?}");
        close(side_a[0], Point2::new(150.0, 2.0));
        close(*side_a.last().unwrap(), Point2::new(50.0, 2.0));
        close(side_b[0], Point2::new(350.0, 2.0));
        close(*side_b.last().unwrap(), Point2::new(398.0, 50.0));
        assert!(
            side_b.iter().any(|p| p.x > 397.0 && p.y < 3.0),
            "round the corner"
        );
    }
}
```

Run: `cargo nextest run -p opendrape --test free_sew notches` and `cargo nextest run -p opendrape --lib seams`
Expected: the panel test FAILS (no warning); the drawing test passes (Task 1 made the drawing follow free sides; this test pins it).

- [ ] **Step 3: The warning**

In `crates/app/src/editor/panel.rs`'s `seam_properties`:
- Its doc comment becomes "A seam: each side's length, a warning when they differ by more than 3 mm, a warning when their notches don't pair up, Flip and Delete."
- After the `if difference > … { … }` block that shows `panel-seam-differ`:

```rust
        // Notches inside each side pair up in order; with a different number on each side the
        // fabric is laid out evenly instead (see `opendrape_mesh`).
        let notches = |side: &SeamSide| {
            geom::shape_of(project, side.shape)
                .and_then(|s| geom::side_notches(&s, side))
                .map_or(0, |n| n.len())
        };
        let (na, nb) = (notches(&seam.a), notches(&seam.b));
        if na != nb {
            ui.colored_label(
                ui.visuals().warn_fg_color,
                tr!("panel-seam-notches", one = na, other = nb),
            );
        }
```

Run: `cargo nextest run -p opendrape`
Expected: all pass.

- [ ] **Step 4: Commit**

```bash
cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings
git add crates/app
git commit -m "feat(app): the seam panel warns when notches don't pair up

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 9: Grab and pin in 3D, pin markers in 2D and 3D, Remove pin (review: light)

**Files:**
- Create:
  - `crates/app/src/draping.rs`
  - `crates/app/src/editor/pins.rs`
  - `crates/app/tests/draping.rs`
- Modify:
  - `crates/app/src/{lib.rs,app.rs}`
  - `crates/app/src/arrange/overlay.rs`
  - `crates/app/src/editor/{mod.rs,canvas.rs,panel.rs,paint.rs}`
  - `crates/app/i18n/en-US/opendrape.ftl`
  - `crates/app/tests/{pins.rs,ui.rs}`

**Interfaces:**
- Consumes:
  - Task 2's `Pin`, `Shape::{pin_spot, spot_shown}`;
  - Task 6's `SimFrame.fabric`, `Fabric::pattern_point`, `SimRunner::{grab, pull, release}`;
  - M4a's `arrange::{ScreenCamera, gizmo::{GRAB_PT, plane_drag}, overlay}`, `Document::{begin_gesture, gesture_edit, end_gesture}`.
- Produces:
  - `opendrape::draping`:
    - `FabricHit { triangle, bary, point }`;
    - `enum Pull { Grab { fabric, triangle, bary, target }, To(DVec3), Release }`;
    - `enum MenuAt { Fabric(Pin), Pin(usize) }`;
    - `Draper { pub menu: Option<MenuAt>, .. }` with `is_dragging`, `press`, `drag_to`, `release`, `click`, `secondary_click`;
    - `pick_fabric(&SimFrame, origin, dir) -> Option<FabricHit>`, `pin_at(&ScreenCamera, &Project, DVec2) -> Option<usize>`, `pin_here(&Project, &Fabric, &FabricHit) -> Option<Pin>`.
  - `editor::Selection::Pin(usize)`, `pub(crate) editor::PIN_COLOUR`, `PatternEditor::{add_pin, remove_pin}` (pub) and `pin_spots`, `pin_at`, `pin_properties` (pub(super)).
  - `arrange::overlay::paint_pins(&Painter, &ScreenCamera, &Project, selected: Option<usize>)`.

**Behaviour:**
- **In the 3D view, while draping only:**
  - A press on a pin's marker (within 8 screen points, the gizmo's `GRAB_PT`) starts moving that pin: its target follows the pointer in the plane facing the viewer, as one undo step.
  - A press on the fabric grabs the point under the pointer, which follows it in that plane at the depth it was grabbed (no undo step), and lets go on release.
  - A drag that starts off the fabric and off every marker turns the camera, as before.
- **Clicks and menus:**
  - A click on a marker selects its pin. A right-click on the fabric offers **Pin here**, which pins that spot where it is now and selects the pin.
  - A right-click on a marker offers **Remove pin**.
  - "Drag the fabric to pull it. Right-click it to pin it there; drag a pin to move it." shows under "Press Reset to move pieces."
- **On the pattern table:**
  - Each pin is a ring with a dot where its shape shows it, the selected one in the selection colour.
  - A click on a marker selects the pin before anything else.
  - **Properties** shows "Pin", "On Front", "Holds the fabric 120.0 cm up, 25.0 cm in front of the form's centre line." and **Remove pin**.
  - Delete removes a selected pin, in the Edit, Sew and Free Sew tools.
- **The 3D view** draws the markers at their targets (the selected one bright).

- [ ] **Step 1: Strings**

In `crates/app/i18n/en-US/opendrape.ftl`, after `panel-delete-seam = …`:

```
panel-pin = Pin
panel-pin-on = On { $name }
panel-pin-held = Holds the fabric { $height } up, { $out } in front of the form's centre line.
panel-remove-pin = Remove pin
menu-pin-here = Pin here
hint-pinning = Drag the fabric to pull it. Right-click it to pin it there; drag a pin to move it.
```

- [ ] **Step 2: Failing tests**

Create `crates/app/tests/draping.rs`:

```rust
//! Grabbing and pinning the fabric in the 3D view while it drapes, driven through the 3D view's
//! input handler (`Draper`) with a fixed camera: no window and no graphics card needed.

use glam::{DVec2, DVec3, Vec3};
use opendrape::SimFrame;
use opendrape::arrange::ScreenCamera;
use opendrape::draping::{Draper, MenuAt, Pull};
use opendrape::editor::{Document, Selection};
use opendrape::sim_runner::SimRunner;
use opendrape_core::{Half, Piece, PieceId, Pin, Placement, Point2, Project};
use opendrape_drape::{Drape, Stage};
use opendrape_render::OrbitCamera;
use std::sync::Arc;
use std::time::{Duration, Instant};

/// The view the tests look through: from the front, at 1 m up, 1.5 m away.
fn camera() -> ScreenCamera {
    let orbit = OrbitCamera {
        target: Vec3::new(0.0, 1.0, 0.5),
        yaw: 0.0,
        pitch: 0.0,
        distance: 1.5,
        fov_y: 35f32.to_radians(),
    };
    ScreenCamera::new(&orbit, DVec2::new(0.0, 0.0), DVec2::new(800.0, 600.0))
}

/// A 300 × 400 mm front cut on the fold (its left edge: the whole piece is x -300..300), flat
/// and upright at z = 0.5 m facing the camera, its middle at 1 m up.
fn front() -> Project {
    let mut pr = Project::new();
    let mut half = Piece::rectangle(PieceId(0), "Front", Point2::new(0.0, 0.0), 300.0, 400.0);
    half.fold = Some(3);
    let id = pr.add_piece(half);
    pr.set_placement(id, Some(Placement::at([0.0, 1.0, 0.5])));
    pr
}

/// The first frame of `project`'s drape, as the 3D view would get it.
fn first_frame(project: &Project) -> SimFrame {
    let drape = Drape::new(Arc::new(project.clone()), &Stage::shared());
    let cloth = drape.solver.cloth();
    SimFrame {
        drape: 1,
        seq: 1,
        time: 0.0,
        positions: cloth.positions().iter().map(|p| p.as_vec3()).collect(),
        triangles: Arc::new(cloth.triangles().to_vec()),
        step_ms: 0.0,
        notes: Arc::new(Vec::new()),
        fabric: drape.fabric.clone(),
    }
}

/// Where the piece's spot (x, y) mm on the pattern (the whole front, centred on (0, 200)) is
/// in 3D, and on screen.
fn spot(x: f64, y: f64) -> (DVec3, DVec2) {
    let at = DVec3::new(x / 1000.0, 1.0 + (y - 200.0) / 1000.0, 0.5);
    (at, camera().project(at).unwrap())
}

#[test]
fn a_press_on_the_fabric_grabs_the_spot_under_the_pointer_and_drags_it_facing_the_viewer() {
    let pr = front();
    let frame = first_frame(&pr);
    let mut doc = Document::new(pr, None);
    let mut draper = Draper::default();
    let (at, on_screen) = spot(100.0, 300.0);
    let Some(Pull::Grab {
        fabric,
        triangle,
        bary,
        target,
    }) = draper.press(&camera(), &frame, &mut doc, on_screen)
    else {
        panic!("a grab");
    };
    assert!((target - at).length() < 1e-6, "{target}");
    let (shape, back) = fabric.pattern_point(triangle, bary).unwrap();
    assert_eq!(shape, PieceId(1));
    assert!(back.distance(Point2::new(100.0, 300.0)) < 1e-3, "{back:?}");
    assert!(draper.is_dragging());
    // 40 points right and 20 up: the target moves in the plane facing the viewer, under the
    // pointer.
    let to = on_screen + DVec2::new(40.0, -20.0);
    let Some(Pull::To(moved)) = draper.drag_to(&camera(), &mut doc, to) else {
        panic!("a pull");
    };
    assert!((moved.z - 0.5).abs() < 1e-9, "at the depth it was grabbed");
    assert!(camera().project(moved).unwrap().distance(to) < 1e-6);
    assert_eq!(draper.release(&mut doc), Some(Pull::Release));
    assert!(!doc.can_undo(), "a grab is no edit");
    // Beside the fabric, a press grabs nothing (the drag turns the camera).
    assert_eq!(
        draper.press(&camera(), &frame, &mut doc, DVec2::new(5.0, 5.0)),
        None
    );
    assert!(!draper.is_dragging());
}

#[test]
fn pin_here_pins_the_spot_under_the_pointer_where_it_is_on_either_half() {
    let pr = front();
    let frame = first_frame(&pr);
    let mut draper = Draper::default();
    // On the drawn half (x > 0) the spot is kept as it is; on the pale half, as its mirror image.
    for (x, half, kept) in [(100.0, Half::Drawn, 100.0), (-100.0, Half::Pale, 100.0)] {
        let (at, on_screen) = spot(x, 300.0);
        draper.secondary_click(&camera(), &frame, &pr, on_screen);
        let Some(MenuAt::Fabric(pin)) = draper.menu else {
            panic!("Pin here at {x}");
        };
        assert_eq!((pin.shape, pin.half), (PieceId(1), half));
        assert!(
            pin.at.distance(Point2::new(kept, 300.0)) < 1e-3,
            "{:?}",
            pin.at
        );
        assert!((DVec3::from_array(pin.target) - at).length() < 1e-6);
    }
}

#[test]
fn a_pin_marker_is_dragged_as_one_undo_step_selected_by_a_click_and_removed_from_its_menu() {
    let mut pr = front();
    let (at, on_screen) = spot(100.0, 300.0);
    pr.pins = vec![Pin {
        shape: PieceId(1),
        half: Half::Drawn,
        at: Point2::new(100.0, 300.0),
        target: at.to_array(),
    }];
    let frame = first_frame(&pr);
    let mut doc = Document::new(pr.clone(), None);
    let mut draper = Draper::default();
    assert_eq!(
        draper.press(&camera(), &frame, &mut doc, on_screen),
        None,
        "not a grab"
    );
    assert!(draper.is_dragging());
    for k in 1..=4 {
        draper.drag_to(
            &camera(),
            &mut doc,
            on_screen + DVec2::new(0.0, -10.0 * f64::from(k)),
        );
    }
    draper.release(&mut doc);
    let moved = DVec3::from_array(doc.project().pins[0].target);
    assert!(
        moved.y > at.y + 0.01 && (moved.z - 0.5).abs() < 1e-9,
        "{moved}"
    );
    assert!(doc.undo(), "one step");
    assert_eq!(doc.project().pins[0].target, at.to_array());
    assert!(!doc.undo());
    // A click on the marker selects the pin; a right-click offers Remove pin.
    let mut selection = Selection::None;
    draper.click(&camera(), doc.project(), &mut selection, on_screen);
    assert_eq!(selection, Selection::Pin(0));
    draper.secondary_click(&camera(), &frame, doc.project(), on_screen);
    assert_eq!(draper.menu, Some(MenuAt::Pin(0)));
}

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

#[test]
fn a_grab_through_the_handler_pulls_the_draping_fabric() {
    let mut pr = front();
    // Held up by two pins at its top corners, so it hangs where the camera looks.
    let corner = |x: f64| Pin {
        shape: PieceId(1),
        half: if x < 0.0 { Half::Pale } else { Half::Drawn },
        at: Point2::new(x.abs(), 400.0),
        target: spot(x, 400.0).0.to_array(),
    };
    pr.pins = vec![corner(-300.0), corner(300.0)];
    let runner = SimRunner::start(Stage::shared(), || {});
    runner.play(Arc::new(pr.clone()));
    wait_for("a frame", || runner.latest().is_some_and(|f| f.time > 0.1));
    let frame = runner.latest().unwrap();
    let mut doc = Document::new(pr, None);
    let mut draper = Draper::default();
    let (_, on_screen) = spot(0.0, 100.0);
    let to = on_screen + DVec2::new(0.0, -60.0); // pulled up
    let mut pulls: Vec<Pull> = draper
        .press(&camera(), &frame, &mut doc, on_screen)
        .into_iter()
        .collect();
    pulls.extend(draper.drag_to(&camera(), &mut doc, to));
    let Some(Pull::To(target)) = pulls.last().cloned() else {
        panic!("{pulls:?}");
    };
    let Pull::Grab {
        fabric,
        triangle,
        bary,
        ..
    } = pulls[0].clone()
    else {
        panic!("{pulls:?}");
    };
    runner.grab(fabric, triangle, bary, target);
    let held = |f: &SimFrame| -> DVec3 {
        let t = f.triangles[triangle];
        (0..3)
            .map(|k| f.positions[t[k] as usize].as_dvec3() * bary[k])
            .sum()
    };
    wait_for("the fabric to follow", || {
        runner
            .latest()
            .is_some_and(|f| (held(&f) - target).length() < 0.01)
    });
    runner.release();
}
```

In `crates/app/tests/pins.rs`, the imports become:

```rust
use egui::Key;
use egui_kittest::kittest::Queryable;
use opendrape::editor::Selection;
use opendrape_core::{Half, Piece, PieceId, Pin, Point2};
```

and append:

```rust
#[test]
fn pins_show_where_their_shapes_are_and_are_selected_and_removed_there() {
    let mut h = harness();
    let front = with_rectangle(&mut h); // (100,100)–(400,500)
    let twin = h
        .state_mut()
        .doc
        .edit(|p| p.add_twin(front, "Front (mirror)".into(), Point2::new(1000.0, 0.0)))
        .unwrap();
    h.state_mut().doc.edit(|p| {
        p.pins = vec![pin(front, 200.0, 200.0), pin(twin, 300.0, 400.0)];
    });
    h.state_mut().fit();
    h.run();
    // The twin shows its pin at (1000 - 300, 400).
    click(&mut h, 700.0, 400.0);
    assert_eq!(h.state().selection, Selection::Pin(1));
    h.get_by_label("Pin");
    h.get_by_label("On Front (mirror)");
    h.get_by_label("Remove pin").click();
    h.run();
    assert_eq!(h.state().doc.project().pins.len(), 1);
    assert_eq!(h.state().selection, Selection::None);
    cmd(&mut h, Key::Z);
    assert_eq!(h.state().doc.project().pins.len(), 2, "one undo step");
    // A pin on the piece itself, inside it: its marker comes before the piece.
    click(&mut h, 200.0, 200.0);
    assert_eq!(h.state().selection, Selection::Pin(0));
    key(&mut h, Key::Delete);
    assert_eq!(h.state().doc.project().pins, vec![pin(twin, 300.0, 400.0)]);
}
```

Add to `crates/app/tests/ui.rs`, before the comment that opens the gizmo tests (`// The gizmo in the 3D view of the real app …`):

```rust
#[test]
fn right_clicking_the_draping_fabric_pins_it_there() {
    let dir = tempfile::tempdir().unwrap();
    let mut h = harness(dir.path(), SharedState::default());
    h.run();
    // A piece hanging upright in front of the form, facing the camera.
    h.state_mut().editor_mut().doc.edit(|p| {
        let id = p.add_piece(Piece::rectangle(
            PieceId(0),
            "Front",
            Point2::new(0.0, 0.0),
            300.0,
            400.0,
        ));
        p.set_placement(id, Some(opendrape_core::Placement::at([0.0, 1.0, 0.5])));
    });
    h.run();
    h.get_by_label("Play").click();
    h.run_steps(2);
    wait_until(&mut h, "the drape", |a| a.sim_frame().is_some());
    h.run_steps(1);
    h.get_by_label(
        "Drag the fabric to pull it. Right-click it to pin it there; drag a pin to move it.",
    );
    // Gravity waits a moment at the start: the piece is still where it was placed.
    let camera = h.state().view_camera().expect("the 3D view was drawn");
    let p = camera.project(glam::DVec3::new(0.0, 1.0, 0.5)).unwrap();
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
    h.run_steps(2);
    h.get_by_label("Pin here").click();
    h.run_steps(2);
    let pins = h.state().editor().doc.project().pins.clone();
    assert_eq!(pins.len(), 1, "pinned");
    assert_eq!(pins[0].shape, PieceId(1));
    assert!(
        pins[0].at.distance(Point2::new(150.0, 200.0)) < 10.0,
        "{:?}",
        pins[0].at
    );
    assert_eq!(h.state().editor().selection, Selection::Pin(0));
    assert!(h.state().is_draping(), "pinning carries the drape on");
}
```

Run: `cargo nextest run -p opendrape --test draping --test pins --test ui`
Expected: compile errors (no `opendrape::draping`, `Selection::Pin`).

- [ ] **Step 3: Pins on the pattern table**

Create `crates/app/src/editor/pins.rs`:

```rust
//! Pins on the pattern table: where each is shown, picking one by its marker, its panel, and
//! adding and removing pins (each one undo step).

use super::{HIT_PX, PatternEditor, Selection};
use crate::tr;
use opendrape_core::{Pin, Point2};

impl PatternEditor {
    /// Each pin (its index) and where its shape shows it (mm).
    pub(super) fn pin_spots(&self) -> Vec<(usize, Point2)> {
        let shapes = self.shapes();
        self.doc
            .project()
            .pins
            .iter()
            .enumerate()
            .filter_map(|(k, pin)| {
                let shape = shapes.iter().find(|s| s.id == pin.shape)?;
                Some((k, shape.spot_shown(pin.half, pin.at)))
            })
            .collect()
    }

    /// The pin whose marker is within `tol` mm of `w`, the nearest.
    pub(super) fn pin_at(&self, w: Point2, tol: f64) -> Option<usize> {
        self.pin_spots()
            .into_iter()
            .map(|(k, at)| (k, at.distance(w)))
            .filter(|(_, d)| *d <= tol.max(self.view.mm(HIT_PX)))
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .map(|(k, _)| k)
    }

    /// Adds `pin`, as one undo step, and selects it.
    pub fn add_pin(&mut self, pin: Pin) {
        let k = self.doc.edit(|p| {
            p.pins.push(pin);
            p.pins.len() - 1
        });
        if !self.note_if_refused() {
            self.selection = Selection::Pin(k);
        }
    }

    /// Removes pin `k`, as one undo step.
    pub fn remove_pin(&mut self, k: usize) {
        self.doc.edit(|p| {
            if k < p.pins.len() {
                p.pins.remove(k);
            }
        });
        if !self.note_if_refused() && self.selection == Selection::Pin(k) {
            self.selection = Selection::None;
        }
    }

    /// A pin: the piece it is on, where it holds the fabric, and Remove pin.
    pub(super) fn pin_properties(&mut self, ui: &mut egui::Ui, k: usize) {
        let project = self.doc.project();
        let Some(pin) = project.pins.get(k).copied() else {
            return;
        };
        let units = project.units;
        let name = project.name_of(pin.shape).unwrap_or_default().to_owned();
        ui.strong(tr!("panel-pin"));
        ui.label(tr!("panel-pin-on", name = name));
        ui.label(tr!(
            "panel-pin-held",
            height = units.format(pin.target[1] * 1000.0),
            out = units.format(pin.target[2] * 1000.0)
        ));
        ui.add_space(6.0);
        if ui.button(tr!("panel-remove-pin")).clicked() {
            self.remove_pin(k);
        }
    }
}
```

In `crates/app/src/editor/mod.rs`:
- Add `mod pins;` after `mod panel;`, and `pub(crate) use paint::PIN_COLOUR;` after `pub use document::{Document, UNDO_LIMIT};`.
- In `enum Selection`, after `Seam(SeamId),`:

```rust
    /// A pin: its index in the project's pins.
    Pin(usize),
```

- In `Selection::piece`, the first arm becomes `Self::None | Self::Seam(_) | Self::Pin(_) => None,`.
- In `Selection::validated`, after the `if let Self::Seam(id) = self { … }` block:

```rust
        if let Self::Pin(k) = self {
            return if k < project.pins.len() {
                self
            } else {
                Self::None
            };
        }
```

In `crates/app/src/editor/paint.rs`:
- After `const CLIP_MARGIN: f32 = 20.0;`:

```rust
/// A pin's marker, on the pattern table and in the 3D view.
pub(crate) const PIN_COLOUR: Color32 = Color32::from_rgb(200, 30, 60);
```

- In `paint`, after `self.paint_seams(painter, rect);` add `self.paint_pins(painter, rect, &c);`, and after the function that holds it:

```rust
    /// Every pin, where its shape shows it: a ring with a dot, the selected one in the
    /// selection's colour.
    fn paint_pins(&self, painter: &Painter, rect: Rect, c: &Palette) {
        for (k, at) in self.pin_spots() {
            let colour = if self.selection == Selection::Pin(k) {
                c.selected
            } else {
                PIN_COLOUR
            };
            let p = self.view.to_screen(rect, at);
            painter.circle_filled(p, 2.5, colour);
            painter.circle_stroke(p, 6.0, Stroke::new(2.0, colour));
        }
    }
```

- In `paint_selection`'s match, `Selection::Piece(_) | Selection::Seam(_) | Selection::None => {}` becomes `Selection::Piece(_) | Selection::Seam(_) | Selection::Pin(_) | Selection::None => {}`.

In `crates/app/src/editor/panel.rs`'s properties match, after the `Selection::Seam(id)` arm: `Selection::Pin(k) => self.pin_properties(ui, k),`.

In `crates/app/src/editor/canvas.rs`:
- In the Sew/Free Sew key handling, the seam-only Delete becomes:

```rust
                // Only a seam or a pin: a piece or point selected earlier is the Edit tool's to
                // delete.
                if matches!(self.selection, Selection::Seam(_) | Selection::Pin(_))
                    && delete_pressed()
                {
                    self.delete_selection();
                }
```

- Where the Edit tool's click picks a selection (`self.selection = match self.seam_at(at, tol) { … };`), replace that statement with:

```rust
            // A pin's marker comes before everything.
            self.selection = match (self.pin_at(at, tol), self.seam_at(at, tol)) {
                (Some(k), _) => Selection::Pin(k),
                (None, Some(seam)) if !on_a_point => Selection::Seam(seam),
                _ => hit.map_or(Selection::None, Hit::selection),
            };
```

- In `delete_selection`'s match, before `Selection::Edge(..) | Selection::None => {}`: `Selection::Pin(k) => self.remove_pin(k),`.

Run: `cargo nextest run -p opendrape --test pins`
Expected: PASS.

- [ ] **Step 4: Grab and pin in 3D**

Create `crates/app/src/draping.rs`, and add `pub mod draping;` after `pub mod diagnostics;` in `crates/app/src/lib.rs`:

```rust
//! What the pointer does in the 3D view while the garment drapes:
//! - press on the fabric and drag to pull the point under the pointer, in the plane facing the
//!   viewer at the depth it was grabbed (a grab is no edit, and no undo step);
//! - right-click the fabric for **Pin here**, which pins that spot where it is now;
//! - drag a pin's marker to move where it holds the fabric (one undo step);
//! - click a marker to select its pin, or right-click it for **Remove pin**.
//!
//! It works on the drape's latest frame and a camera, with no egui in it, so the tests drive it
//! with a fixed camera and no window. What a grab asks of the simulation it hands back as a
//! [`Pull`] for the caller to pass on.

use crate::arrange::ScreenCamera;
use crate::arrange::gizmo::{GRAB_PT, plane_drag};
use crate::editor::{Document, Selection};
use crate::sim_runner::SimFrame;
use glam::{DVec2, DVec3};
use opendrape_core::{Pin, Project};
use opendrape_drape::Fabric;
use opendrape_geom as geom;
use std::sync::Arc;

/// A point of the drape's cloth: a triangle of the frame's cloth, the point's barycentric
/// coordinates in it, and where it is (m).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FabricHit {
    pub triangle: usize,
    pub bary: [f64; 3],
    pub point: DVec3,
}

/// What a grab asks of the simulation.
#[derive(Clone, Debug, PartialEq)]
pub enum Pull {
    /// Pull this point of the cloth made from `fabric` to `target`.
    Grab {
        fabric: Arc<Fabric>,
        triangle: usize,
        bary: [f64; 3],
        target: DVec3,
    },
    /// The grabbed point's target moves.
    To(DVec3),
    /// Let go.
    Release,
}

/// What a right-click in the 3D view was on, for its menu.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum MenuAt {
    /// The fabric: the pin **Pin here** adds.
    Fabric(Pin),
    /// A pin's marker: its index in the project's pins.
    Pin(usize),
}

enum Drag {
    /// Pulling the fabric: the point grabbed, and where the press was.
    Grab { point: DVec3, start: DVec2 },
    /// Moving pin `index`, whose target was `target` when the press was at `start`.
    Pin {
        index: usize,
        target: DVec3,
        start: DVec2,
    },
}

/// What the pointer does in the 3D view while draping.
#[derive(Default)]
pub struct Draper {
    drag: Option<Drag>,
    /// What the last right-click was on: the 3D view's menu is for it.
    pub menu: Option<MenuAt>,
}

/// Where the ray from `origin` along unit `dir` meets triangle `t`: its distance, and the
/// barycentric coordinates of the point it meets.
fn ray_hit(origin: DVec3, dir: DVec3, t: [DVec3; 3]) -> Option<(f64, [f64; 3])> {
    let (e1, e2) = (t[1] - t[0], t[2] - t[0]);
    let p = dir.cross(e2);
    let det = e1.dot(p);
    if det.abs() < 1e-12 {
        return None;
    }
    let inv = 1.0 / det;
    let s = origin - t[0];
    let u = s.dot(p) * inv;
    let q = s.cross(e1);
    let v = dir.dot(q) * inv;
    if !(0.0..=1.0).contains(&u) || v < 0.0 || u + v > 1.0 {
        return None;
    }
    let d = e2.dot(q) * inv;
    (d > 1e-9).then_some((d, [1.0 - u - v, u, v]))
}

/// The point of `frame`'s cloth that the ray from `origin` along unit `dir` meets first.
pub fn pick_fabric(frame: &SimFrame, origin: DVec3, dir: DVec3) -> Option<FabricHit> {
    let at = |k: u32| frame.positions[k as usize].as_dvec3();
    frame
        .triangles
        .iter()
        .enumerate()
        .filter_map(|(triangle, t)| {
            let corners = t.map(at);
            let (d, bary) = ray_hit(origin, dir, corners)?;
            Some((d, triangle, bary))
        })
        .min_by(|a, b| a.0.total_cmp(&b.0))
        .map(|(d, triangle, bary)| FabricHit {
            triangle,
            bary,
            point: origin + dir * d,
        })
}

/// The pin whose marker (drawn where it holds the fabric) is within [`GRAB_PT`] of screen
/// point `pos`, the nearest.
pub fn pin_at(cam: &ScreenCamera, project: &Project, pos: DVec2) -> Option<usize> {
    project
        .pins
        .iter()
        .enumerate()
        .filter_map(|(k, pin)| {
            let shown = cam.project(DVec3::from_array(pin.target))?;
            let d = shown.distance(pos);
            (d <= GRAB_PT).then_some((k, d))
        })
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(k, _)| k)
}

/// The pin **Pin here** makes at `hit`: on the spot of the pattern there (kept on its stored
/// piece and half, see `Shape::pin_spot`), holding it where it is now.
pub fn pin_here(project: &Project, fabric: &Fabric, hit: &FabricHit) -> Option<Pin> {
    let (shape, on_shape) = fabric.pattern_point(hit.triangle, hit.bary)?;
    let (half, at) = geom::shape_of(project, shape)?.pin_spot(on_shape);
    Some(Pin {
        shape,
        half,
        at,
        target: hit.point.to_array(),
    })
}

impl Draper {
    /// A pin is being moved, or the fabric pulled: the camera stays put.
    pub fn is_dragging(&self) -> bool {
        self.drag.is_some()
    }

    /// The primary button went down at `pos`. On a pin's marker it starts moving the pin
    /// (everything until [`Self::release`] is one undo step); on the fabric it grabs it, and
    /// hands back the grab. Anywhere else it does nothing (the drag turns the camera).
    pub fn press(
        &mut self,
        cam: &ScreenCamera,
        frame: &SimFrame,
        doc: &mut Document,
        pos: DVec2,
    ) -> Option<Pull> {
        if let Some(index) = pin_at(cam, doc.project(), pos) {
            doc.begin_gesture();
            self.drag = Some(Drag::Pin {
                index,
                target: DVec3::from_array(doc.project().pins[index].target),
                start: pos,
            });
            return None;
        }
        let (origin, dir) = cam.ray(pos);
        let hit = pick_fabric(frame, origin, dir)?;
        self.drag = Some(Drag::Grab {
            point: hit.point,
            start: pos,
        });
        Some(Pull::Grab {
            fabric: frame.fabric.clone(),
            triangle: hit.triangle,
            bary: hit.bary,
            target: hit.point,
        })
    }

    /// The pointer moved to `pos` with the button down: the grabbed point's target, or the pin
    /// being moved, follows it in the plane facing the viewer at its depth. A pin that would go
    /// where no pin may be (more than 10 m away) stays where it last could.
    pub fn drag_to(&mut self, cam: &ScreenCamera, doc: &mut Document, pos: DVec2) -> Option<Pull> {
        match self.drag.as_ref()? {
            Drag::Grab { point, start } => {
                let moved = plane_drag(cam, *point, *start, pos)?;
                Some(Pull::To(*point + moved))
            }
            Drag::Pin {
                index,
                target,
                start,
            } => {
                let to = (*target + plane_drag(cam, *target, *start, pos)?).to_array();
                let index = *index;
                doc.gesture_edit(|p| {
                    if let Some(pin) = p.pins.get_mut(index) {
                        pin.target = to;
                    }
                });
                None
            }
        }
    }

    /// The button came up: a grab lets go; a pin's move is one undo step.
    pub fn release(&mut self, doc: &mut Document) -> Option<Pull> {
        match self.drag.take()? {
            Drag::Grab { .. } => Some(Pull::Release),
            Drag::Pin { .. } => {
                doc.end_gesture();
                None
            }
        }
    }

    /// A click (no drag) at `pos`: a pin's marker selects its pin.
    pub fn click(
        &mut self,
        cam: &ScreenCamera,
        project: &Project,
        selection: &mut Selection,
        pos: DVec2,
    ) {
        if let Some(k) = pin_at(cam, project, pos) {
            *selection = Selection::Pin(k);
        }
    }

    /// A right-click at `pos`: on a pin's marker its menu offers **Remove pin**; on the fabric,
    /// **Pin here**; anywhere else there is no menu.
    pub fn secondary_click(
        &mut self,
        cam: &ScreenCamera,
        frame: &SimFrame,
        project: &Project,
        pos: DVec2,
    ) {
        self.menu = match pin_at(cam, project, pos) {
            Some(k) => Some(MenuAt::Pin(k)),
            None => {
                let (origin, dir) = cam.ray(pos);
                pick_fabric(frame, origin, dir)
                    .and_then(|hit| pin_here(project, &frame.fabric, &hit))
                    .map(MenuAt::Fabric)
            }
        };
    }
}
```

In `crates/app/src/arrange/overlay.rs`, before `paint_readout`:

```rust
/// Draws each pin's marker where it holds the fabric (the selected pin bright).
pub fn paint_pins(
    painter: &Painter,
    cam: &ScreenCamera,
    project: &opendrape_core::Project,
    selected: Option<usize>,
) {
    for (k, pin) in project.pins.iter().enumerate() {
        let Some(at) = cam.project(glam::DVec3::from_array(pin.target)) else {
            continue;
        };
        let colour = if selected == Some(k) {
            BRIGHT
        } else {
            crate::editor::PIN_COLOUR
        };
        painter.circle_filled(screen(at), 3.0, colour);
        painter.circle_stroke(screen(at), 7.0, Stroke::new(2.0, colour));
    }
}
```

In `crates/app/src/app.rs`:
- Add `use crate::draping::{Draper, MenuAt, Pull};` after `use crate::diagnostics::Diagnostics;`.
- In `struct App`, after `arranger: Arranger,`:

```rust
    /// What the pointer does in the 3D view while draping.
    draper: Draper,
```

  and in its constructor, after `arranger: Arranger::default(),` add `draper: Draper::default(),`.
- In the draping hint, after `ui.label(tr!("hint-draping"));` add `ui.label(tr!("hint-pinning"));`.
- In `view_3d`, replace the `match &drawn { … }` that calls `self.arrange(…)` with:

```rust
        match (&drawn, &sim) {
            (Some(drawn), _) if !draping => {
                self.arrange(ui, &drawn.response, &drawn.camera, &scene)
            }
            (Some(drawn), Some(frame)) => {
                self.stop_arranging();
                self.drape_input(ui, &drawn.response, &drawn.camera, frame);
            }
            _ => {
                // Nothing to arrange or pull (no view, or the drape's fabric is being made): a
                // drag still held ends where it is.
                self.stop_arranging();
                self.stop_pulling();
            }
        }
```

- In the camera drag, `let grabbed = self.arranger.is_dragging();` becomes `let grabbed = self.arranger.is_dragging() || self.draper.is_dragging();`.
- After `view_3d`:

```rust
    /// No arranging now: a gizmo drag still held ends where it is, and nothing is lit.
    fn stop_arranging(&mut self) {
        self.arranger.release(&mut self.editor.doc);
        self.arranger.released();
        self.arranger.hovered = None;
    }

    /// No pulling now: a grab lets go, and a pin being moved stays where it is (one step).
    fn stop_pulling(&mut self) {
        let pull = self.draper.release(&mut self.editor.doc);
        self.send(pull.into_iter().collect());
        self.draper.menu = None;
    }

    /// Passes what a grab asks for on to the simulation.
    fn send(&self, pulls: Vec<Pull>) {
        let Some(runner) = &self.runner else { return };
        for pull in pulls {
            match pull {
                Pull::Grab {
                    fabric,
                    triangle,
                    bary,
                    target,
                } => runner.grab(fabric, triangle, bary, target),
                Pull::To(target) => runner.pull(target),
                Pull::Release => runner.release(),
            }
        }
    }

    /// The pointer in the 3D view while draping: a press on the fabric pulls it, a pin's marker
    /// is dragged to move it, right-clicks offer Pin here and Remove pin, and the pins are drawn.
    fn drape_input(
        &mut self,
        ui: &egui::Ui,
        response: &egui::Response,
        cam: &ScreenCamera,
        frame: &SimFrame,
    ) {
        let at = |p: egui::Pos2| glam::DVec2::new(f64::from(p.x), f64::from(p.y));
        let editor = &mut self.editor;
        let mut pulls = Vec::new();
        if response.drag_started_by(egui::PointerButton::Primary)
            && let Some(p) = ui.input(|i| i.pointer.press_origin())
        {
            pulls.extend(self.draper.press(cam, frame, &mut editor.doc, at(p)));
        }
        if self.draper.is_dragging()
            && let Some(p) = response.interact_pointer_pos()
        {
            pulls.extend(self.draper.drag_to(cam, &mut editor.doc, at(p)));
        }
        if response.drag_stopped() {
            pulls.extend(self.draper.release(&mut editor.doc));
        }
        if response.clicked()
            && let Some(p) = response.interact_pointer_pos()
        {
            self.draper
                .click(cam, editor.doc.project(), &mut editor.selection, at(p));
        }
        if response.secondary_clicked()
            && let Some(p) = response.interact_pointer_pos()
        {
            self.draper
                .secondary_click(cam, frame, editor.doc.project(), at(p));
        }
        if let Some(menu) = self.draper.menu {
            response.context_menu(|ui| match menu {
                MenuAt::Fabric(pin) => {
                    if ui.button(tr!("menu-pin-here")).clicked() {
                        editor.add_pin(pin);
                        ui.close();
                    }
                }
                MenuAt::Pin(k) => {
                    if ui.button(tr!("panel-remove-pin")).clicked() {
                        editor.remove_pin(k);
                        ui.close();
                    }
                }
            });
        }
        let selected = match editor.selection {
            editor::Selection::Pin(k) => Some(k),
            _ => None,
        };
        let painter = ui.painter_at(response.rect);
        crate::arrange::overlay::paint_pins(&painter, cam, editor.doc.project(), selected);
        self.send(pulls);
    }
```

- [ ] **Step 5: Run them**

Run: `cargo nextest run -p opendrape`
Expected: all pass, including:
- the four `draping` tests;
- `pins_show_where_their_shapes_are_and_are_selected_and_removed_there`;
- `right_clicking_the_draping_fabric_pins_it_there`, which drives the real app's 3D view off-screen through `Harness::builder().wgpu()`, as the M4a gizmo tests do.

- [ ] **Step 6: Commit**

```bash
cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings
git add crates/app
git commit -m "feat(app): grab and pin the draping fabric in 3D; pins on the pattern table

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 10: Place at → arm in the menus, and placements held while draping (review: light)

**Files:**
- Modify:
  - `crates/app/src/editor/{placing.rs,mod.rs}`
  - `crates/app/src/app.rs`
  - `crates/app/i18n/en-US/opendrape.ftl`
  - `crates/app/tests/placing.rs`

**Interfaces:**
- Consumes: Task 4's `place::place_at_arm`, `Stage::{arms, arm_surface_distance, signed_distance}`, `Placement::mirrored`; M4a's `place_menu` (shared by the 2D and 3D right-click menus), `placement_properties`, `set_placement`, `Project::owner`.
- Produces: `PatternEditor::place_at_arm(&mut self, id: PieceId, arm: usize)` (pub; `arm` 0 left, 1 right) and `pub draping: bool` on `PatternEditor`, which the app sets each frame.

**Behaviour:**
- **The menu:** Place at… (right-click a piece in 2D or 3D) gains **Place at left arm** and **Place at right arm**, after the four body sides.
- **The pair partner:**
  - Placing a piece clears its twin's own placement, so the twin shows the piece's placement mirrored, round the other arm.
  - Placing a twin gives its piece the twin's placement mirrored.
  - One undo step for both; the placed piece is selected.
- **While draping:**
  - Every Place at… item and Flat is greyed out.
  - The "3D placement" fields are greyed out under "Placements apply after Reset."
  - A placement typed or chosen then would not move the draping cloth, so it is not offered.

- [ ] **Step 1: Strings**

In `crates/app/i18n/en-US/opendrape.ftl`, after `place-right = Place at right side`:

```
place-left-arm = Place at left arm
place-right-arm = Place at right arm
```

and after `place-flat = Flat`:

```
panel-placement-draping = Placements apply after Reset.
```

- [ ] **Step 2: Failing tests**

In `crates/app/tests/placing.rs`, `use opendrape_core::{PieceId, Placement, Point2};` becomes `use opendrape_core::{Piece, PieceId, Placement, Point2};`, and append:

```rust
/// A 340 × 220 mm sleeve at (100,100) with its twin to its right, at 600..940.
fn with_sleeves(h: &mut H) -> (PieceId, PieceId) {
    let (sleeve, twin) = h.state_mut().doc.edit(|p| {
        let id = p.add_piece(Piece::rectangle(
            PieceId(0),
            "Sleeve",
            Point2::new(100.0, 100.0),
            340.0,
            220.0,
        ));
        let twin = p
            .add_twin(id, "Sleeve (mirror)".into(), Point2::new(1040.0, 0.0))
            .unwrap();
        (id, twin)
    });
    h.state_mut().fit();
    h.run();
    (sleeve, twin)
}

/// How far each point of shape `id`'s outline is from arm `arm`'s line, in 3D: (nearest,
/// farthest).
fn round_arm(h: &H, id: PieceId, arm: usize) -> (f64, f64) {
    let shapes = opendrape_geom::shapes(h.state().doc.project());
    let shape = shapes.iter().find(|s| s.id == id).unwrap();
    let p = h.state().placement(id).unwrap();
    let line = Stage::shared().arms()[arm];
    opendrape_geom::outline_points(&shape.piece, 0.5)
        .into_iter()
        .map(|q| line.distance(place::apply(&p, place::centre_of(shape), q)))
        .fold((f64::MAX, f64::MIN), |(lo, hi), d| (lo.min(d), hi.max(d)))
}

#[test]
fn place_at_left_arm_puts_a_sleeve_round_it_and_its_twin_round_the_other_as_one_step() {
    let mut h = harness_with_form();
    let (sleeve, twin) = with_sleeves(&mut h);
    right_click(&mut h, 250.0, 200.0);
    h.get_by_label("Place at left arm").click();
    h.run();
    let p = placement(&h, sleeve).expect("placed");
    let r = p.curve.expect("curved round the arm");
    let (near, far) = round_arm(&h, sleeve, 0);
    assert!(
        (near - r).abs() < 1e-6 && (far - r).abs() < 1e-6,
        "{near}..{far} vs {r}"
    );
    assert_eq!(
        placement(&h, twin),
        None,
        "the twin takes the sleeve's, mirrored"
    );
    let (near, far) = round_arm(&h, twin, 1);
    assert!(
        (near - r).abs() < 1e-6 && (far - r).abs() < 1e-6,
        "{near}..{far}"
    );
    cmd(&mut h, Key::Z);
    assert_eq!((placement(&h, sleeve), placement(&h, twin)), (None, None));
}

#[test]
fn placing_a_twin_at_an_arm_puts_its_piece_round_the_other() {
    let mut h = harness_with_form();
    let (sleeve, twin) = with_sleeves(&mut h);
    h.state_mut().place_at_arm(twin, 1);
    h.run();
    let (near, far) = round_arm(&h, twin, 1);
    let r = placement(&h, twin).unwrap().curve.unwrap();
    assert!((near - r).abs() < 1e-6 && (far - r).abs() < 1e-6);
    let (near, far) = round_arm(&h, sleeve, 0);
    assert!(
        (near - r).abs() < 1e-6 && (far - r).abs() < 1e-6,
        "{near}..{far}"
    );
}

#[test]
fn while_draping_placements_are_not_offered() {
    let mut h = harness_with_form();
    let id = with_rectangle(&mut h);
    h.state_mut().draping = true;
    click(&mut h, 250.0, 300.0);
    h.get_by_label("Placements apply after Reset.");
    type_into(&mut h, "Position Y", "100");
    assert_eq!(placement(&h, id), None, "the field is greyed out");
    right_click(&mut h, 250.0, 300.0);
    h.get_by_label("Place at left arm").click();
    h.run();
    assert_eq!(placement(&h, id), None, "and so is Place at…");
}
```

Run: `cargo nextest run -p opendrape --test placing`
Expected: compile errors (no `place_at_arm`, `draping`).

- [ ] **Step 3: The menu items and the draping flag**

In `crates/app/src/editor/mod.rs`, in `PatternEditor` after `pub stage: Option<Arc<Stage>>,`:

```rust
    /// The garment is draping: placements don't apply until Reset, so they are not offered.
    pub draping: bool,
```

and in its constructor, after `stage: None,` add `draping: false,`.

In `crates/app/src/editor/placing.rs`:
- The module comment becomes:

```rust
//! Placing pieces in 3D from the pattern window and Properties: Place at… (wrapped round the
//! form at its front, back or sides, or round an arm), Flat, and typed positions and angles.
//! Each is one undo step, and works on a twin as on any piece (the twin then keeps a placement
//! of its own). While the garment drapes, placements don't apply: they are not offered then.
```

- After `place_at`:

```rust
    /// Place at → Left arm (`arm` 0) or Right arm (1): wraps `id` round that arm (see
    /// `place::place_at_arm`). Its partner in a mirrored pair goes on the other arm: a twin by
    /// taking its piece's placement mirrored, a piece by being given the twin's mirrored. One
    /// undo step. Needs a form.
    pub fn place_at_arm(&mut self, id: PieceId, arm: usize) {
        let Some(stage) = self.stage.clone() else {
            return;
        };
        let project = self.doc.project();
        let shapes = geom::shapes(project);
        let (Some(shape), Some(on)) = (shapes.iter().find(|s| s.id == id), stage.arms().get(arm))
        else {
            return;
        };
        let placement = place::place_at_arm(
            shape,
            on,
            &|along, angle| stage.arm_surface_distance(arm, along, angle),
            &|p| stage.signed_distance(p) < 0.0,
        );
        let partner = match project.owner(id) {
            Some((piece, opendrape_core::Side::Master)) => {
                piece.twin.as_ref().map(|t| (t.id, None))
            }
            Some((piece, opendrape_core::Side::Twin)) => {
                Some((piece.id, Some(placement.mirrored())))
            }
            None => None,
        };
        self.doc.edit(|p| {
            p.set_placement(id, Some(placement));
            if let Some((other, its)) = partner {
                p.set_placement(other, its);
            }
        });
        if !self.note_if_refused() {
            self.selection = Selection::Piece(id);
        }
    }
```

- Replace `place_menu`:

```rust
    /// The Place at… menu for `id` (right-click on a piece, in 2D or 3D). While the garment
    /// drapes its items are greyed out: placements apply after Reset.
    pub fn place_menu(&mut self, ui: &mut egui::Ui, id: PieceId) {
        let free = !self.draping;
        for (at, label) in [
            (PlaceAt::Front, tr!("place-front")),
            (PlaceAt::Back, tr!("place-back")),
            (PlaceAt::LeftSide, tr!("place-left")),
            (PlaceAt::RightSide, tr!("place-right")),
        ] {
            if ui.add_enabled(free, egui::Button::new(label)).clicked() {
                self.place_at(id, at);
                ui.close();
            }
        }
        for (arm, label) in [(0, tr!("place-left-arm")), (1, tr!("place-right-arm"))] {
            if ui.add_enabled(free, egui::Button::new(label)).clicked() {
                self.place_at_arm(id, arm);
                ui.close();
            }
        }
        ui.separator();
        if ui
            .add_enabled(free, egui::Button::new(tr!("place-flat")))
            .clicked()
        {
            self.flatten(id);
            ui.close();
        }
    }
```

- Replace `placement_properties` with the two functions below. The fields move, unchanged, into `placement_fields`, which is shown greyed out while draping:

```rust
    /// The "3D placement" group of a piece's properties: its position (in the student's units)
    /// and its rotation in degrees about x, then y, then z.
    pub(super) fn placement_properties(&mut self, ui: &mut egui::Ui, id: PieceId) {
        let Some(placement) = self.placement(id) else {
            return;
        };
        let units = self.doc.project().units;
        ui.add_space(6.0);
        ui.strong(tr!("panel-placement"));
        if self.draping {
            ui.label(tr!("panel-placement-draping"));
        }
        let free = !self.draping;
        ui.add_enabled_ui(free, |ui| self.placement_fields(ui, id, placement, units));
    }

    /// The position and rotation fields of the "3D placement" group.
    fn placement_fields(
        &mut self,
        ui: &mut egui::Ui,
        id: PieceId,
        placement: Placement,
        units: opendrape_core::Units,
    ) {
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
```

In `crates/app/src/app.rs`'s `view_3d`, after `let draping = self.is_draping();` add `self.editor.draping = draping;`.

- [ ] **Step 4: Run them**

Run: `cargo nextest run -p opendrape`
Expected: all pass, including:
- `place_at_left_arm_puts_a_sleeve_round_it_and_its_twin_round_the_other_as_one_step`;
- `placing_a_twin_at_an_arm_puts_its_piece_round_the_other`;
- `while_draping_placements_are_not_offered`.

- [ ] **Step 5: Commit**

```bash
cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings
git add crates/app
git commit -m "feat(app): Place at left/right arm; placements wait for Reset while draping

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 11: The T-shirt drape gate (review: full)

**Files:**
- Create: `crates/testkit/tests/project_tshirt.rs` (new file; `crates/testkit/src/{garments.rs,metrics.rs}` are only used, never changed)

**Interfaces:**
- Consumes:
  - Tasks 1–6: free `SeamSide`s, `SeamSide::edges`, `Notch`, `place::{place_at, place_at_arm, layout, PlaceAt}`, `Stage::{shared, arms, arm_surface_distance, surface_distance, signed_distance, drape_collider, shoulder_y}`;
  - `Drape::new`, `Drape.fabric.panel`, `FabricPanel.{first_particle, first_triangle, flat, triangles}`;
  - `opendrape_testkit::metrics::{measure, position_hash}`, `Cloth::{is_alive, stitch_pairs, triangles}`.
- Produces: nothing for other tasks. The testkit's `Cargo.toml` needs no change (its dev-dependencies already name core, drape, geom and mesh).

**Behaviour (the gate, from the spec):**
- **What it drapes:** a T-shirt drafted, sewn and placed as a student would, through the app's own code. The front and back are halves on the fold. A mirrored pair of sleeves has a notch at the top of the cap.
- **The seams:**
  - shoulders, sides and the sleeve's underarm whole-edge;
  - each cap half sewn free into an armhole, the two meeting at the notch.

  That makes five stored seams, and ten with their mirror images.
- **The placing:** the bodice typed up so its neck points sit at 1.36 m, then Place at front and back, then Place at → Left arm for the sleeve. Its twin mirrors onto the right arm.
- **After 6 s,** all of these must hold:
  - no notes;
  - no NaN;
  - penetration max ≤ 2 mm and p99 ≤ 1 mm;
  - the seams at most 4 mm apart (mean ≤ 1 mm) just before they weld, and welded shut;
  - the cap notch within 5 mm of the shoulder seam's end;
  - every live sleeve particle within 12 cm of its arm's line;
  - kinetic energy ≤ 1e-4 J;
  - strain p99 ≤ 10%.
- **Determinism:** two runs of 90 frames give the same position hash.
- **The start:** the sleeves start round the arms and clear of the body, their tops less than 10 cm down the arm.

- [ ] **Step 1: The gate**

Create `crates/testkit/tests/project_tshirt.rs`:

```rust
//! M4b's drape gate: a T-shirt drafted the way a student would draft it, draped on the bundled
//! body through the same code the app runs (`opendrape-drape`: its `Stage` for the form, its
//! arms and its rays, and its `Drape` for the fabric, the placements and the stitches).
//! - The pieces: a front and a back cut on the fold, and a mirrored pair of sleeves with a
//!   notch at the top of the cap.
//! - The seams: the shoulders, the sides and the sleeve's underarm sewn whole-edge (W); each
//!   cap sewn into its armhole in two free seams (F) that meet at the cap notch. The mirror
//!   images sew the other side.
//! - The placing: the bodice moved up to the neck (typed), Place at front and back, and Place
//!   at → Left arm for the sleeve (its twin goes on the right arm).

use opendrape_core::{
    Edge, Half, Notch, OutlinePos, Piece, PieceId, Placement, Point2, Project, SeamSide,
};
use opendrape_drape::{Drape, Stage};
use opendrape_geom as geom;
use opendrape_mesh::place::{self, PlaceAt};
use opendrape_sim::{BodyCollider, FRAME_DT, Solver};
use opendrape_testkit::metrics::{measure, position_hash};
use std::sync::Arc;

/// Where the bodice's neck points go (m): on the base of the neck.
const NECK_Y: f64 = 1.36;

fn p(x: f64, y: f64) -> Point2 {
    Point2::new(x, y)
}

/// The T-shirt and the ids the gate needs.
struct Shirt {
    project: Project,
    front: PieceId,
    sleeve: PieceId,
    twin: PieceId,
}

fn shirt(stage: &Stage) -> Shirt {
    let mut pr = Project::new();
    // The front half, its fold the centre front (its last edge, on the left): hem 250 wide,
    // side 400 long, an armhole curving in to the shoulder point, a sloping shoulder and a
    // scooped neckline.
    let mut front = Piece::polygon(
        PieceId(0),
        "Front",
        &[
            p(0.0, 0.0),
            p(250.0, 0.0),
            p(250.0, 400.0),
            p(190.0, 590.0),
            p(75.0, 615.0),
            p(0.0, 540.0),
        ],
    );
    front.edges[2] = Edge::Curve {
        c1: p(210.0, 420.0),
        c2: p(185.0, 520.0),
    };
    front.edges[4] = Edge::Curve {
        c1: p(75.0, 570.0),
        c2: p(40.0, 540.0),
    };
    front.fold = Some(5);
    // The back half, drawn to the left of its fold (the centre back, its edge 1), with a
    // shallower neckline: placed at the back, its drawn half is on the body's left, as the
    // front's is.
    let mut back = Piece::polygon(
        PieceId(0),
        "Back",
        &[
            p(550.0, 0.0),
            p(800.0, 0.0),
            p(800.0, 595.0),
            p(725.0, 615.0),
            p(610.0, 590.0),
            p(550.0, 400.0),
        ],
    );
    back.edges[2] = Edge::Curve {
        c1: p(770.0, 595.0),
        c2: p(725.0, 600.0),
    };
    back.edges[4] = Edge::Curve {
        c1: p(615.0, 520.0),
        c2: p(590.0, 420.0),
    };
    back.fold = Some(1);
    // A sleeve: hem 300, underarm edges 130 long, a cap (edge 2, from the right underarm
    // corner round to the left) 97.5 mm high with a notch at its top.
    let mut sleeve = Piece::polygon(
        PieceId(0),
        "Sleeve",
        &[
            p(920.0, 0.0),
            p(1220.0, 0.0),
            p(1240.0, 130.0),
            p(900.0, 130.0),
        ],
    );
    sleeve.edges[2] = Edge::Curve {
        c1: p(1190.0, 260.0),
        c2: p(950.0, 260.0),
    };
    let cap = geom::edge_length(&sleeve, 2);
    sleeve.notches = vec![Notch::new(2, cap / 2.0)];
    let front = pr.add_piece(front);
    let back = pr.add_piece(back);
    let sleeve = pr.add_piece(sleeve);
    let twin = pr
        .add_twin(sleeve, "Sleeve (mirror)".into(), p(2540.0, 0.0))
        .unwrap();
    let whole = |shape, edge, forward| SeamSide::edges(shape, Half::Drawn, edge, edge, forward);
    let cap_part = |from: f64, to: f64| SeamSide {
        shape: sleeve,
        half: Half::Drawn,
        from: OutlinePos::new(2, from),
        to: OutlinePos::new(2, to),
        forward: false,
    };
    // W: the shoulders (from the shoulder points), the sides (from the hem), and the sleeve's
    // underarm (from the hem).
    pr.add_seam(whole(front, 3, true), whole(back, 3, false));
    pr.add_seam(whole(front, 1, true), whole(back, 5, false));
    pr.add_seam(whole(sleeve, 1, true), whole(sleeve, 3, false));
    // F: the cap from its left underarm corner back to the notch, into the front armhole from
    // the underarm up; then from the notch on to its right corner, into the back armhole from
    // the shoulder down.
    pr.add_seam(cap_part(1.0, 0.5), whole(front, 2, true));
    pr.add_seam(cap_part(0.5, 0.0), whole(back, 4, true));
    assert_eq!(pr.check(), Ok(()));
    assert_eq!(pr.all_seams().len(), 10, "every seam has its mirror image");
    // Typed in Properties: the bodice up, so its neck points sit on the base of the neck.
    let middle = NECK_Y - 0.615 / 2.0;
    for id in [front, back] {
        assert!(pr.set_placement(id, Some(Placement::at([0.0, middle, 0.4]))));
    }
    // Place at front and back, as the app does it.
    for (id, at) in [(front, PlaceAt::Front), (back, PlaceAt::Back)] {
        let shapes = geom::shapes(&pr);
        let shape = shapes.iter().find(|s| s.id == id).unwrap();
        let placed = place::place_at(
            &pr,
            shape,
            at,
            &place::layout(&shapes),
            stage.shoulder_y(),
            &|angle, y| stage.surface_distance(angle, y),
        );
        assert!(pr.set_placement(id, Some(placed)));
    }
    // Place at → Left arm, as the app does it: the twin, with no placement of its own, mirrors
    // it onto the right arm.
    let shapes = geom::shapes(&pr);
    let shape = shapes.iter().find(|s| s.id == sleeve).unwrap();
    let placed = place::place_at_arm(
        shape,
        &stage.arms()[0],
        &|along, angle| stage.arm_surface_distance(0, along, angle),
        &|q| stage.signed_distance(q) < 0.0,
    );
    assert!(pr.set_placement(sleeve, Some(placed)));
    assert_eq!(pr.check(), Ok(()));
    Shirt {
        project: pr,
        front,
        sleeve,
        twin,
    }
}

/// The form alone, for measuring how far the cloth is inside it.
fn body(stage: &Stage) -> BodyCollider {
    let (positions, triangles) = stage.render_mesh();
    BodyCollider::new(positions, triangles).expect("the bundled body is closed")
}

/// The largest and mean distance (mm) between the particles the solver stitches.
fn seam_gaps(solver: &Solver) -> (f64, f64) {
    let x = solver.cloth().positions();
    let d: Vec<f64> = solver
        .cloth()
        .stitch_pairs()
        .map(|(a, b)| (x[a] - x[b]).length() * 1000.0)
        .collect();
    (
        d.iter().copied().fold(0.0, f64::max),
        d.iter().sum::<f64>() / d.len() as f64,
    )
}

/// A triangle corner of the cloth that holds the spot of shape `shape` at `at` (mm): welding
/// renumbers the cloth's triangles in place, so that corner always holds the live particle the
/// spot has become.
fn corner_at(drape: &Drape, shape: PieceId, at: Point2) -> (usize, usize) {
    let panel = drape.fabric.panel(shape).unwrap();
    let mm = |i: usize| p(panel.flat[i][0] * 1000.0, panel.flat[i][1] * 1000.0);
    let v = (0..panel.flat.len())
        .min_by(|&a, &b| mm(a).distance(at).total_cmp(&mm(b).distance(at)))
        .unwrap();
    assert!(mm(v).distance(at) < 1e-6, "a point of the fabric is there");
    panel
        .triangles
        .iter()
        .enumerate()
        .find_map(|(t, tri)| {
            let corner = tri.iter().position(|&c| c as usize == v)?;
            Some((panel.first_triangle + t, corner))
        })
        .unwrap()
}

#[test]
fn a_drafted_t_shirt_drapes_with_its_sleeves_on_the_arms() {
    let stage = Stage::shared();
    let s = shirt(&stage);
    let mut drape = Drape::new(Arc::new(s.project.clone()), &stage);
    assert_eq!(drape.notes, vec![], "nothing to tell the student");
    let particles = drape.solver.cloth().len();
    assert!(
        (4_000..30_000).contains(&particles),
        "{particles} particles"
    );
    // The cap notch, and the front's shoulder point (where the shoulder seam ends).
    let shapes = geom::shapes(&s.project);
    let sleeve = &shapes.iter().find(|x| x.id == s.sleeve).unwrap().piece;
    let notch = geom::point_at_distance(sleeve, 2, sleeve.notches[0].distance);
    let notch_corner = corner_at(&drape, s.sleeve, notch);
    let shoulder_corner = corner_at(&drape, s.front, p(190.0, 590.0));
    let collider = stage.drape_collider();
    let frames = (6.0 / FRAME_DT).round() as usize;
    // The seams as they are just before they weld.
    let mut before_weld = None;
    for _ in 0..frames {
        if drape.solver.cloth().has_open_stitches() {
            before_weld = Some(seam_gaps(&drape.solver));
        }
        drape.solver.step(Some(&collider));
    }
    let (gap_max, gap_mean) = before_weld.expect("the seams were open at the start");
    eprintln!("seam gaps before welding: max {gap_max:.2} mm, mean {gap_mean:.2} mm");
    let cloth = drape.solver.cloth();
    let r = measure(cloth, &body(&stage));
    eprintln!("{r:#?}");
    let x = cloth.positions();
    let held = |(t, c): (usize, usize)| x[cloth.triangles()[t][c] as usize];
    let notch_gap = (held(notch_corner) - held(shoulder_corner)).length() * 1000.0;
    eprintln!("the cap notch is {notch_gap:.2} mm from the shoulder seam's end");
    // Every live particle of each sleeve, and how far it is from its arm's line.
    let mut farthest: f64 = 0.0;
    for (shape, arm) in [(s.sleeve, 0), (s.twin, 1)] {
        let panel = drape.fabric.panel(shape).unwrap();
        let line = stage.arms()[arm];
        let range = panel.first_particle..panel.first_particle + panel.flat.len();
        for (i, q) in x.iter().enumerate().take(range.end).skip(range.start) {
            if cloth.is_alive(i) {
                farthest = farthest.max(line.distance(*q));
            }
        }
    }
    eprintln!(
        "the farthest sleeve point is {:.1} cm from its arm's line",
        farthest * 100.0
    );
    assert!(!r.has_nan);
    assert!(
        r.penetration_max_mm <= 2.0 && r.penetration_p99_mm <= 1.0,
        "poke-through"
    );
    assert!(
        gap_max <= 4.0 && gap_mean <= 1.0,
        "seams didn't close: {gap_max:.2} / {gap_mean:.2} mm"
    );
    assert!(!r.open_stitches, "welded shut");
    assert!(notch_gap <= 5.0, "the cap notch is off the shoulder seam");
    assert!(farthest <= 0.12, "a sleeve slid off its arm");
    assert!(
        r.kinetic_energy <= 1e-4,
        "still moving: {} J",
        r.kinetic_energy
    );
    assert!(
        r.strain_p99 <= 0.10,
        "fabric over-stretched: {}",
        r.strain_p99
    );
}

#[test]
fn the_drafted_t_shirt_drapes_the_same_every_time() {
    let stage = Stage::shared();
    let collider = stage.drape_collider();
    let hash = || {
        let mut drape = Drape::new(Arc::new(shirt(&stage).project), &stage);
        for _ in 0..90 {
            drape.solver.step(Some(&collider));
        }
        position_hash(drape.solver.cloth())
    };
    assert_eq!(hash(), hash());
}

#[test]
fn the_sleeves_start_round_the_arms_clear_of_the_body() {
    let stage = Stage::shared();
    let s = shirt(&stage);
    let drape = Drape::new(Arc::new(s.project), &stage);
    let x = drape.solver.cloth().positions();
    for (shape, arm) in [(s.sleeve, 0), (s.twin, 1)] {
        let panel = drape.fabric.panel(shape).unwrap();
        let line = stage.arms()[arm];
        let start = &x[panel.first_particle..panel.first_particle + panel.flat.len()];
        assert!(start.iter().all(|q| stage.signed_distance(*q) > 0.0));
        // Wrapped round its arm, its top a little way down from the shoulder.
        let top = start
            .iter()
            .map(|q| (*q - line.shoulder).dot(line.direction))
            .fold(f64::MAX, f64::min);
        assert!((0.0..0.1).contains(&top), "the top {top:.3} m down the arm");
    }
}
```

- [ ] **Step 2: Run it**

Run: `cargo nextest run -p opendrape-testkit --test project_tshirt --no-capture`
Expected: all three tests pass. The first prints the gaps, the report and the sleeve numbers. The probe printed:
- seam gaps before welding: max 2.86 mm, mean 0.42 mm;
- penetration 0.00 mm (max and p99), strain p99 0.045, kinetic energy 2.2e-8 J;
- the cap notch 0.00 mm from the shoulder seam's end;
- the farthest sleeve point 7.1 cm from its arm's line.

It took 12–18 s on a heavily loaded machine (the M4a skirt gate took 13 s under the same load).

If a gate fails, don't loosen it. Debug with superpowers:systematic-debugging, starting from what the report shows:
- **Seam gaps over 4 mm:**
  - check that `place_at_arm` measures the arm only where it hangs free (`along >= arm.free`). Counting the rays near the shoulder gave a 10.8 cm curve, and the underarm seam was still 86 mm open when it welded;
  - check that the sleeve is centred at `SLEEVE_ANGLE` (the outer side). Centred on the arm's front, the underarm corner starts in the chest.
- **Penetration at the start, or a "starts inside" note:** check `place_at_arm`'s drop down the arm: without it, 8 points start up to 13 mm inside the chest.
- **The notch far from the shoulder seam:** check the two cap seams' directions. The cap runs from the right underarm corner to the left, so both cap halves run backwards (`forward: false`).
- **A sleeve off its arm:** print `stage.arms()`; the lines should run 42° from vertical.

- [ ] **Step 3: Commit**

```bash
cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings
git add crates/testkit/tests/project_tshirt.rs
git commit -m "test(testkit): a T-shirt drafted, sewn and placed like a student's drapes with its sleeves on the arms

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 12: Checklist, docs, and the full local CI run (review: light)

**Files:**
- Create: `docs/testing/M4b-checklist.md`
- Modify:
  - `README.md`
  - `docs/specs/2026-10-09-opendrape-design.md` (Status only)
  - `docs/superpowers/specs/2026-10-10-m4b-sew-and-drape-design.md` (Status only)

Leave the main spec's milestone table alone: dress forms Track B renumbers it.

- [ ] **Step 1: Tester checklist** (`docs/testing/M4b-checklist.md`)

```markdown
# OpenDrape M4b: what to try

This build sews part of an edge, puts sleeves on the arms, and lets you pull and pin the
fabric while it drapes. You draft a T-shirt, sew its sleeve caps into the armholes, place the
pieces, press **Play**, and change the pattern while it drapes.

## Check these

- [ ] **Fit moved.** Press **Cmd+0** (Ctrl+0 on Windows): every piece fits the window. The
      toolbar's **Fit** button shows the new shortcut. **F** is now the Free Sew tool.
- [ ] **Draft a front on the fold.** With **Pen (H)**, draw half a T-shirt front about 25 cm
      wide and 60 cm tall: hem, side seam, an armhole curving in to the shoulder, a sloping
      shoulder, and a neckline down to the centre front. Click the centre-front edge, then
      **Set as fold line**.
- [ ] **Draft a back the same way**, to the right of the front, with a shallower neckline,
      and set its centre back as the fold line.
- [ ] **Draft a sleeve.** With **Rectangle (S)**, draw a piece 30 cm wide and 13 cm tall.
      With **Edit (Z)**, click its top edge, tick **Curved**, and drag both handles up about
      13 cm: that is the cap. With **Notch (N)**, click the top of the cap. Select the sleeve
      and click **Make mirrored pair**.
- [ ] **Sew the straight seams with W:** the front's shoulder to the back's shoulder, the
      front's side to the back's side, and the sleeve's left edge to its right edge (the
      underarm).
- [ ] **Sew the cap into the front armhole with F.** Press **F** (Free Sew). Click the cap's
      left corner (where it meets the underarm edge), then the notch: a thick line runs along
      half the cap. Click the bottom of the front's armhole, then the front's shoulder point.
      A coloured seam line appears along half the cap and the whole armhole. The mirrored
      seams (the front's pale half, the other sleeve) appear by themselves.
- [ ] **Sew the other half of the cap into the back armhole.** Click the notch, then the
      cap's right corner; then the back's shoulder point, then the bottom of the back's
      armhole.
- [ ] Still in Free Sew, click one corner twice: "That side is too short to sew: pick
      points more than 1 mm apart." Press **Esc**.
- [ ] Click two points on an edge, holding **Shift** on the second: the side goes the long
      way round the piece. Press **Esc**.
- [ ] Click two points along the front armhole, which is sewn already: "Part of this is
      already sewn." Press **Esc**.
- [ ] Click a cap seam's coloured line: **Properties** shows both lengths.
- [ ] **Notches that don't match.** Draw two rectangles. With **Notch (N)**, add a notch to
      the middle of the first one's bottom edge. With **W**, sew that edge to the second
      one's bottom edge, then click the seam's line: the panel says "Notches don't match: 1
      on one side, 0 on the other." Delete the two rectangles.
- [ ] **Place the pieces.** In the 3D view, select the front. In **Properties**, under
      **3D placement**, type **105** in **Position Y**, and the same for the back. Right-click
      the front, then **Place at front**; the back, then **Place at back**. Right-click the
      sleeve, then **Place at left arm**: it wraps round the left arm, and its mirror wraps
      round the right arm.
- [ ] Press **Play**. The T-shirt settles on the body with the sleeves on the arms. Under
      "Press Reset to move pieces." it says "Drag the fabric to pull it. Right-click it to pin
      it there; drag a pin to move it."
- [ ] **Change the pattern while it drapes.** In the pattern window, with **Edit (Z)**, drag
      the sleeve's two hem corners about 5 cm down. The drape carries on with longer sleeves,
      without going back to arranging. **Cmd+Z**: the sleeves are short again, still draping.
- [ ] **Pull the fabric.** In 3D, press on the front hem and drag: the fabric follows the
      pointer. Let go: it falls back.
- [ ] **Pin it.** Right-click the front hem, then **Pin here**. A red ring appears there in
      3D and on the front in the pattern window. Drag the ring in 3D: the fabric follows it.
      **Cmd+Z** puts the pin back where it was. Right-click the ring, then **Remove pin**.
- [ ] While it drapes, the 3D placement fields are greyed out under "Placements apply after
      Reset.", and so is **Place at…**.
- [ ] Press **Reset**: the pieces are back where you placed them.
- [ ] **File → Save As…**, quit, reopen and **File → Open…**: the free seams and the pins are
      back. Your M4a files still open, with their seams as they were.

## Known limits in this build

- Cmd+H (hide OpenDrape) does not work in this build.
- Placements don't change while the garment drapes: press **Reset** first.
- A grab pulls one point at a time, and a pin holds its spot exactly.
- Every piece is one light cotton until fabrics arrive.
- The garment drapes on the bundled body; dress forms come in M3.

If anything looks wrong, take a screenshot, then choose Help → About OpenDrape and
click **Copy diagnostics**.
```

- [ ] **Step 2: README and spec status**

README, the **Status** line:
> **Status:** early development (milestone M4b: sew and drape, part 2: sew part of an edge, put sleeves on the arms, pull and pin the fabric while it drapes, and drape the T-shirt you drafted). Next: dress forms (M3).

`docs/specs/2026-10-09-opendrape-design.md`, `## Status`: add after the M4a bullet (use the day the branch is finished if it is later than 2026-10-10):
> - **M4b Sew & drape, part 2: complete (2026-10-10).**
>   - **Sewing:** Free Sew (F) joins any two points of outlines; notches pair up across seams; whole-edge sewing (W) is unchanged; Fit moves to Cmd+0.
>   - **Arranging:** arm lines found on the form; Place at → Left arm / Right arm.
>   - **Draping:** pins (saved) and grabs in 3D; edits while draping carry the drape on, warm-started and coalesced on the simulation thread.
>   - **Files:** project format v4 (v1 to v3 files upgrade).
>   - **The drafted-T-shirt gate:** 0.00 mm penetration, seams welded, the cap notch on the shoulder seam, strain p99 4.5%.

`docs/superpowers/specs/2026-10-10-m4b-sew-and-drape-design.md`, the `**Status:**` line becomes:
> **Status:**
> - Approved by the user on 2026-10-10. The design summary was answered with "Yes, build it". Scope was set by three choices: "Everything for a T-shirt", "Click start and end points" for free sewing, and "Saved, and editable" pins.
> - Implemented on branch `m4b-sew-and-drape`. The plan's "Where the spec's assumptions met the evidence" section records where the evidence refined the spec:
>   - no stored axis: the placement's own frame wraps the sleeve round the arm;
>   - the sleeve is centred on the arm's outer side, measured where the arm hangs free, and lowered until clear;
>   - arm lines come from cross-sections;
>   - Fit is on Cmd+0;
>   - the warm start welds at once only when every seam starts closed;
>   - pins left off their piece go with the edit.

- [ ] **Step 3: Full local CI**

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo nextest run --workspace
cargo deny check
mkdir -p target/licenses
cargo about generate -m crates/app/Cargo.toml about.hbs -o target/licenses/THIRD_PARTY_LICENSES.html
git diff --quiet main -- Cargo.lock && echo "no new dependencies"
git diff --stat main -- crates/body crates/testkit/src scripts/forms assets/forms ASSETS.md
git grep -n 'CLO' -- crates
```

Expected:
- all green;
- `cargo deny check` prints `advisories ok, bans ok, licenses ok, sources ok`;
- "no new dependencies";
- the dress-forms `git diff --stat` and the `git grep` print nothing.

The probe ran 694 tests (616 before M4b).

- [ ] **Step 4: Commit**

```bash
git add docs README.md
git commit -m "docs: M4b tester checklist and status

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

The controller does the final whole-branch review, the merge and any publishing afterwards. Ask the user before merging or pushing. **Never run the app on the user's Mac.**

---

## Self-review

**1. Spec coverage**

| Spec | Task |
|---|---|
| §1 Model: `OutlinePos`, free `SeamSide` (`from`, `to`, `forward`), what a side covers, never the fold | 1 |
| §1 v3→v4 upgrade of whole-edge sides; mirror seams still derived | 1 |
| §1 `check()`: finite `t` in 0..=1, side > 1 mm, no shared outline (mirrors included), ends may meet | 1 |
| §1 Edits: add point, remove point, move (t unchanged), Delete/Unfold/Remove fold/Break pair, a side left ≤ 1 mm deletes its seam | 1 (and `Document`, Task 1) |
| §1 Free Sew tool: F, four clicks, shorter way / Shift for longer, snapping to notches, corners and middles within 8 points, highlight and rubber band, Esc | 7 |
| §1 Refusals: "Part of this is already sewn.", the fold notice | 7 |
| §1 Seam panel lengths and difference (M4a) plus the notch check | 8 |
| §2 Notches paired in order, stretches laid out between them, even fallback, the panel message, at least one step per stretch | 3, 8 |
| §3 `Stage.arms`: shoulder, direction, length, surface distance by rays; worked out once from the mesh | 4 |
| §3 Place at → Left/Right arm in the 2D and 3D menus; cap to the shoulder; width round the arm, 3 cm off; twin on the other arm | 4, 10 |
| §3 Storage: the axis kept with the placement (see "Where the spec's assumptions met the evidence", 1) | 4 |
| §3 Gizmo and typed values as in M4a | unchanged; tested in Task 4 (`a_sleeve_moved_after_placing_takes_its_curve_with_it`) |
| §4 Only while draping; grab: stiff spring in the view plane at the grab depth, no undo step | 6, 9 |
| §4 Pin here (undo step, target = current position), held at target, markers in 2D and 3D | 6, 9 |
| §4 Drag a marker (one step), Remove pin, Delete; pins on twins and pale halves | 2, 9 |
| §4 `check()` refuses missing shape, `at` > 1 mm outside, bad target > 10 m, more than 500 | 2 |
| §4 Pins outlive re-meshing (stored by pattern position) | 2, 6 |
| §4 `crates/sim`: attach, move, remove; nothing existing changes | 5 |
| §5 Any change while draping updates the drape (edits, seams, pins, undo, redo) | 6, 9 |
| §5 Warm start: same spot of the same piece through the old triangle, new fabric from the nearest, zero velocity, closed seams welded at once | 6 (see "Where the spec's assumptions met the evidence", 7) |
| §5 Placements don't apply while draping; typed fields disabled | 6, 10 |
| §5 Reset returns to arranging; rebuild off the UI thread, old drape shown meanwhile, edits coalesced | 6 |
| §6 The T-shirt gate, all seven gates | 11 |
| Testing: core/io, mesh, drape, sim, egui_kittest, testkit (the M4a skirt gate still passes) | 1–11 |
| Constraints: never launch the app, no new dependencies, no "CLO", Fluent, dress-forms files untouched, sim additive only, schema v4 (v5 if Track B takes 4 first) | Global Constraints; every task; checked in Task 12 |

**2. Placeholders:** none. Every code step carries the code, copied by a script from the probe, where every task compiled and its tests passed. Where a task's code here differs from the probe's own task boundaries, the change is a move, not new code:
- `SeamSide::tidy` and `Document`'s `drop_broken` call were written in the probe's Tasks 7 and 2. They sit in Task 1 here, where the model they belong to is made.
- The M4a fuzz test's sides are fixed in Task 1, not Task 3: Task 1's helper change gives the old calls a new meaning.
- `crates/mesh/src/place.rs` (Task 4) and `crates/drape/src/live.rs` (Task 6) are their final versions. They add the bounded clearance search (Review Focus 5) and one test each, which the probe wrote after the gate.

One constant was renamed after the probe's last full run: the warm start's closed-stitch limit is `SEWN_GAP_M`, so that no identifier spells the product name the constraints forbid. The probe was formatted, linted (`-D warnings`) and its drape tests run again after the rename.

**3. Type consistency:** every Interfaces block was written from the probe, where all twelve tasks compiled and ran together: 694 tests, clippy `-D warnings`, `cargo deny check` and `cargo about generate`, with `Cargo.lock` unchanged. The names later tasks use (`SeamSide::edges`, `Project::drop_broken`, `geom::side_runs`, `Arm`, `place_at_arm`, `Drape`, `Fabric`, `SimRunner::update`, `Selection::Pin`, `PatternEditor::place_at_arm`) are spelled as their tasks produce them.

**4. Review Focus:** five items, each with its test in the owning task (see the list at the top).
