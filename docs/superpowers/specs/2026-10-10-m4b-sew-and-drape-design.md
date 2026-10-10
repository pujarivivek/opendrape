# OpenDrape M4b: Sew & Drape, part 2 (Design)

**Date:** 2026-10-10.

**Status:**
- Approved by the user on 2026-10-10. The design summary was answered with "Yes, build it". Scope was set by three choices: "Everything for a T-shirt", "Click start and end points" for free sewing, and "Saved, and editable" pins.
- Implemented on branch `m4b-sew-and-drape`. The plan's "Where the spec's assumptions met the evidence" section records where the evidence refined the spec:
  - no stored axis: the placement's own frame wraps the sleeve round the arm;
  - the sleeve is centred on the arm's outer side, measured where the arm hangs free, and lowered until clear;
  - arm lines come from cross-sections;
  - Fit is on Cmd+0;
  - the warm start welds at once only when every seam starts closed;
  - pins left off their piece go with the edit.

**Builds on:**
- M4a (Sew & drape, part 1), merged and published as `fbc4023`. M4a's spec is `docs/superpowers/specs/2026-10-09-m4a-sew-and-drape-design.md`, and everything in it still holds unless this document changes it.
- Main spec: `docs/specs/2026-10-09-opendrape-design.md`.
- Dress forms (`docs/superpowers/specs/2026-10-09-dress-forms-design.md`, branch `dress-forms`). That work hasn't started building yet, so M4b drapes on the current body. The forms will swap in behind `crates/drape`'s `Stage`.

**Behaviour reference:** CLO3D's free sewing, M:N sewing, arrangement on arms, pins and live simulation. CLO3D is a reference only and is never named in the product.

## Purpose

A student can draft a T-shirt, sew it and see it drape, with the sleeves on the arms. To get there, M4b adds:
- free sewing, so a seam can run between any two points on the outline, as for a sleeve cap into an armhole;
- notches that line up across a seam;
- sleeves placed on the arms;
- fabric that can be grabbed and pinned in 3D;
- live updates while the garment drapes.

**Success:** a tester follows the M4b checklist on a Mac with no help, and each step is also covered by a headless test.
1. Draft a T-shirt:
   - a front and a back, each cut on the fold;
   - a mirrored pair of sleeves, with a notch at the top of the cap.
2. Sew the shoulders and the sides with **W**, and the underarm seam of each sleeve.
3. Sew each sleeve cap into its armhole with **F**, in two free seams that meet at the cap notch: cap to front armhole, and cap to back armhole. The mirrored seams appear by themselves.
4. In 3D, place the pieces:
   - **Place at front** for the front;
   - **Place at back** for the back;
   - **Place at → Left arm** for one sleeve. Its twin goes on the right arm.
5. Press **Play**. The shirt drapes with the sleeves on the arms and nothing poking through.
6. While it drapes:
   - lengthen the sleeves in 2D, and the drape carries on with the new length;
   - grab the hem and pull it;
   - right-click → **Pin here**, and the pin holds that spot.
7. Save and reopen. The free seams and pins are back.

## Scope

**In M4b:**
- Free seams (key **F**).
- Whole-edge seams (**W**) become free seams that happen to start and end at corners.
- Notch matching across seams.
- Sleeves on arms: arm lines in the `Stage`, and **Place at → Left arm / Right arm**.
- Grab (temporary) and pins (saved, editable) in 3D.
- Live 2D→3D warm start while draping.
- Format v5 if dress forms has taken v4 first, otherwise v4.
- A T-shirt drape gate.

**Not in M4b:**
- Self-collision, so a sleeve can pass through the bodice. This comes with fitted garments.
- Fabric choices (M6).
- Pins while arranging. Pins act only on the draping fabric.

## 1. Free seams

### Model (`crates/core`)

```rust
pub struct OutlinePos { pub edge: usize, pub t: f64 }   // t = arc-length fraction along the stored edge, 0..=1
pub struct SeamSide {
    pub shape: PieceId,
    pub half: Half,
    pub from: OutlinePos,          // the side's start: it meets the other side's start
    pub to: OutlinePos,            // the side's end
    pub forward: bool,             // the side runs from `from` to `to` along the stored outline direction
}
```

- **What a side covers.** The side covers the outline from `from` to `to`, walking in the direction set by `forward`. It may pass corners and wrap past the last edge. It never includes the fold edge, and it never crosses the fold.
- **Upgrading old files.** A v3 side with `{first_edge, edges, forward}` becomes:
  - `forward`: from `(first_edge, 0)` to `(last, 1)`;
  - otherwise: from `(last, 1)` to `(first_edge, 0)`.
- **Mirror seams** are still derived, never stored. The rules are unchanged from M4a.
- **`Project::check()`** keeps every M4a seam rule. It adds these:
  - `t` is finite and within 0..=1;
  - a side is longer than 1 mm;
  - no part of the outline is used by two seam sides, mirrors included;
  - two sides may share only an end point.
- **Edits keep seams valid.** Each edit below is one undo step.
  - Adding a point re-expresses any `OutlinePos` on the split edge on the right new edge, with the right `t`.
  - Removing a point re-expresses positions on the two merged edges onto the merged edge.
  - Moving points or handles leaves each `t` unchanged.
  - The M4a rules for Delete, Unfold, Remove fold and Break pair still apply.
  - If an edit would leave a side 1 mm or shorter, that seam is deleted.

### Free Sew tool (key F)

- **Making a seam:**
  - Click the start point on one piece, then its end point. The side runs the shorter way round between them. Shift-click the end point to take the long way round.
  - Then click the start and end points on the other piece. That makes the seam.
- **Snapping:** points snap to notches, corners and edge middles within 8 screen points. Otherwise the point lands where the pointer is.
- **While drawing:** the first side's pieces are highlighted, and a rubber band shows the side so far. Esc cancels.
- **Refusals:**
  - a side that would overlap an existing seam: "Part of this is already sewn.";
  - a point on the fold line: the M4b fold notice.
- **The seam panel** shows each side's length and the difference, as M4a does. It also shows the notch check (section 2).

## 2. Notches line up

- **Matching:** for each seam, the notches that lie within each side are paired in order from the side's start. The fabric steps are laid out piece by piece between paired notches, so the k-th notch on one side and the k-th notch on the other land on the same stitch.
- **Mismatched counts:** if the two sides have different numbers of notches, the seam falls back to M4a's even layout, and the panel says "Notches don't match: 2 on one side, 1 on the other."
- **Sampling:** the step count is chosen per stretch between notches, so short stretches still get at least one step.

## 3. Sleeves on arms

- **Arm lines.** `crates/drape`'s `Stage::arms()` returns `Option<&[Arm; 2]>`: one arm per side, left (+x) first. A form may have no arms, or none may be found, and then it is `None`: **Place at → arm** is greyed out, and says "This form has no arms." Each `Arm` gives:
  - a shoulder point;
  - a unit direction down the arm;
  - a length: how far down from the shoulder the straight line runs inside the arm (the elbow bends away below it);
  - `free`: how far down from the shoulder the arm starts to hang clear of the body (below the armpit);
  - the surface distance from the arm line at any point along it and angle around it (`Stage::arm_surface_distance`), measured by rays as for the torso.

  For today's body, the arm lines are worked out once from the mesh. A form made from a mesh (`Stage::from_mesh`) has them worked out the same way. For the dress forms they come from the form file later.
- **Place at → Left arm / Right arm** (the 3D and 2D right-click menu, next to front, back and sides):
  - **Orientation:** the sleeve's pattern "up" (the cap) points to the shoulder, and its length runs down the arm. Its width wraps around the arm, centred on the arm's front, 3 cm off the skin. The curve radius is the largest surface distance over the span the sleeve covers, plus 3 cm.
  - **Mirrored pairs:** placing one sleeve of a pair puts its twin on the other arm.
  - **One wiring.** The form's rays are wired to the placement maths in `Stage` only (`Stage::place_at` and `Stage::place_at_arm`). The app's menu and the drape gates both call them.
  - **Storage:** a placement keeps the cylinder axis it wraps around, so a sleeve isn't forced around the body's centre line. `Placement` gains `axis: Option<Axis>`, a point and a direction. Without it, the M4a vertical centre line is used.
- **Gizmo and typed values** work as in M4a, on the whole placement.

## 4. Grab and pin

- **Only while draping.** While arranging, the gizmo owns the 3D view.
- **Grab:**
  - Press on fabric and drag to pull the point under the pointer. It is a stiff spring to a target that moves with the pointer in the camera's view plane, at the depth where you grabbed.
  - Releasing lets go. A grab is not a project edit and adds no undo step.
- **Pin here:**
  - Right-click fabric → **Pin here** adds `Pin { shape, half, at: Point2 (stored-piece pattern mm), target: [f64; 3] (m) }` to `Project.pins`. The target is the fabric's current 3D position. This is an undo step.
  - The pinned spot of fabric is held at `target`.
  - Pins draw as small markers on the fabric in 3D and on the piece in 2D.
- **Editing pins:**
  - Drag a pin marker in 3D to move its target, as one undo step.
  - Right-click → **Remove pin**, or select the pin and press Delete.
  - A pin on a twin or on a fold's pale half is stored with that `shape` and `half`.
- **`check()` refuses:**
  - a missing shape;
  - an `at` outside the piece's outline, beyond 1 mm;
  - a non-finite or out-of-range target, beyond 10 m;
  - more than 500 pins.
- **Pins outlive mesh changes.** A pin is stored by pattern position, not by mesh vertex, so it survives re-meshing.
- **Simulator changes** (`crates/sim`) are small and additive:
  - attach a mesh point, as a barycentric point in a triangle, to a target, with a stiffness;
  - move that target;
  - remove the attachment.

  Nothing existing changes. This keeps a later merge with the dress-forms work, which also edits `crates/sim`, easy.

## 5. Live 2D→3D

- **What triggers it.** Any project change while draping triggers it: pattern edits, seams, pins, and undo or redo.
- **How it continues.** The fabric is rebuilt on the simulation thread from the new project, and warm-started:
  - Each new fabric point finds where the same spot of the same piece was in the old fabric. It looks up the old triangle containing its pattern position and takes the barycentric 3D position.
  - Points with no old match (new fabric) take the nearest old point's position, offset along that point's surface frame by the pattern distance.
  - Velocities start at zero.
  - Already-closed seams stay closed: stitches whose two ends start together are welded at once.
- **Placement.** Placement changes don't apply while draping, and typed placement fields are disabled then.
- **Reset** still returns to arranging.
- **Speed.** The rebuild happens off the UI thread, and the old drape keeps showing until the new one is ready. Edits that arrive during a rebuild are coalesced, so only the latest project is rebuilt.

## 6. T-shirt drape gate

A new `crates/testkit/tests/project_tshirt.rs` drafts a T-shirt the way a student would:
- a folded front and back with neck and armhole curves;
- a mirrored sleeve pair with a cap notch;
- shoulder, side and underarm seams;
- free cap seams split at the notch;
- Place at front, back, and arms.

It then drapes the shirt through `crates/drape`, the same code the app runs. The gates:
- nothing non-finite;
- max penetration ≤ 2 mm, with p99 ≤ 1 mm;
- before welding, seam gap ≤ 4 mm max and ≤ 1 mm mean, then welded shut;
- the cap notch lands on the shoulder seam's end within 5 mm;
- the sleeves stay on the arms: every sleeve particle lies within 12 cm of its arm line after settling;
- it settles;
- the same run always gives the same result.

## Testing (all headless; the app is never launched)

- **core and io:**
  - `OutlinePos` and free-side validation, plus every edit rule;
  - the v3→v4 upgrade (whole-edge sides become free sides);
  - pin validation;
  - a round trip of every new field;
  - a frozen v4 fixture;
  - older fixtures still load.
- **mesh:**
  - free sides sampled across corners;
  - notch-matched step layout, with paired notches on equal stitch indices;
  - the fallback when notch counts differ.
- **drape:**
  - arm lines on today's body: symmetric, pointing down and out;
  - Place at arm on a cylinder stand-in;
  - the warm-start mapping, where a pure re-mesh puts every point back where it was within 1 mm.
- **sim:** attachment holds a barycentric point at its target, moves with it, and is removed cleanly.
- **egui_kittest:**
  - the Free Sew tool flows, including snapping, the long way round, refusal and the notch warning;
  - grab and pin through the 3D input handler with a fixed camera;
  - pin markers in 2D and 3D;
  - Remove pin;
  - live update while draping, with no Reset.
- **testkit:** the T-shirt gate, and the M4a skirt gate still passes.

## Constraints

- Ask the user before running the OpenDrape app, or anything that opens a window, on their Mac. Ask before pushing, merging or publishing.
- Add no new dependencies, and don't use the name "CLO". Every string goes through Fluent.
- Leave the dress-forms files untouched: `crates/body`, `crates/testkit/src/{garments,metrics}.rs`, `scripts/forms`, `assets/forms`, `ASSETS.md`. `crates/sim` gets only the small additive attachment API.
- Schema: v4. If dress-forms Track B merges first and takes v4, renumber to v5 when merging.

## Corrections from the planning probe (2026-10-10)

The planning probe built every task in a scratch copy. It found the following, and the plan follows these corrections where they differ from the sections above.

1. **No `Placement.axis`.** A placement already wraps a piece around its own axis, and Place at → arm turns the piece so that axis is the arm. Format v4 has no axis field.
2. **Sleeves are centred on the arm's outer side**, not its front. Centring on the front put the underarm corner in the chest.
3. **The arm curve radius is measured only where the arm hangs free** (new `Arm.free`). Measuring over the whole span counted rays that hit the body near the shoulder.
4. **Place at → arm steps the sleeve down the arm** in 1 cm steps, up to 20 cm, until no point starts inside the body. The cap seams pull it back up.
5. **Arm lines are found by horizontal cross-sections of the body.** Clustering the body's points tilted the line out of the arm on this low-poly body.
6. **F is Free Sew. Fit moves to Cmd+0 (Ctrl+0).**
7. **Welding after a warm start is all or nothing.** If every stitch starts within 1 mm, all stitches weld at once. Otherwise they weld at the usual time. The simulator can only weld every stitch together.
8. **An edit that leaves a pin off its piece deletes that pin** in the same undo step, as short seams are deleted. Refusing the edit would stop a student shortening a sleeve past a pin.
9. **Notches within 1 mm of a side's ends don't count** for notch matching. Otherwise the two cap seams would each count the shared cap notch.
10. **Removing a point at a side's end keeps the end on the joined edge.** M4a shortened the side by an edge instead.
11. **`Stage::arms()` is optional.** A form may have no arms (or none can be found): `arms()` returns `Option<&[Arm; 2]>`, and Place at → arm then says "This form has no arms." `Arm.length` is how far down from the shoulder the straight arm line runs inside the arm, and `Arm.free` is how far down the arm starts to hang clear of the body.
12. **The Place at wiring lives in `Stage`** (`place_at` and `place_at_arm`) and is shared by the app and the drape gates, so the T-shirt gate places pieces the way the app's menu does.
