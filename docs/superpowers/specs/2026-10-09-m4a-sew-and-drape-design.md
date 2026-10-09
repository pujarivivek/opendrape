# OpenDrape M4a: Sew & Drape, part 1 (Design)

**Date:** 2026-10-09.

**Status:**
- Approved in conversation, section by section, and as a written document.
- Five points were corrected on 2026-10-10 after the planning probe disproved them; they are marked *(probe)*.
- Implemented on branch `m4a-sew-and-drape`. The plan's Evidence section records where the evidence refined the spec: the Place at… radius, the angle and seam-gap gates, the centre-back seam, and the arrow drag.

**Builds on:**
- M2b (pattern details), merged and published as `2f4b7cc`.
- M1's cloth simulator (`crates/sim`: panels with flat rest shapes, stitches that close and weld, auto-pause).
- Main spec: `docs/specs/2026-10-09-opendrape-design.md`.
- Dress forms (`docs/superpowers/specs/2026-10-09-dress-forms-design.md`, branch `dress-forms`). That work runs in parallel and becomes M3. With it, the main spec's "M3 Sew & drape" becomes M4, split here into M4a (this document) and M4b.

**Behaviour reference:** CLO3D's segment sewing, arrangement and gizmo. It is a reference only and is never named in the product.

## Purpose

Pieces drawn in the 2D editor become fabric in 3D. In M4a a student can:
- sew edges together in 2D;
- place the pieces around the form with a gizmo;
- press Play and watch a skirt they drafted drape on the form.

M4b adds what a T-shirt needs.

**Who it's for:** fashion students in India and Africa, many new to CAD, on low-end laptops.

**Success:** a tester can follow the M4a checklist on a Mac with no help. Every step is also covered by a headless test.
1. Draft a skirt front on the fold and a mirrored pair of back panels (M2b tools).
2. Sew the side seams and the centre back with the Sew tool. The mirrored seams appear automatically. *(probe: a skirt with an open back slides off the form.)*
3. In 3D, right-click the front → **Place at front**, the back panel → **Place at back**, then adjust with the gizmo.
4. Press **Play**. The seams close and the skirt settles on the form with no fabric poking through.
5. Save, reopen: the seams and the arrangement are back. Undo steps back through the gizmo moves.

## Scope

**In M4a:**
- Edge-to-edge sewing (one or more consecutive edges per side) with a Sew tool, a seam panel, a length-difference warning and automatic mirrored seams.
- Turning the project into fabric meshes (`crates/mesh`).
- Arranging in 3D:
  - starting positions;
  - picking;
  - a gizmo built into OpenDrape;
  - **Place at…**;
  - typed placement.
- Draping the project's garment on the current body (on the form once dress forms land), with a floor.
- Saving seams and placements, with a format version bump.

**Not in M4a (M4b or later):**
- Free sewing: any range along an edge, e.g. a 1:2 sleeve cap.
- Arms and sleeves arrangement.
- Grabbing and pinning fabric in 3D.
- Live 2D→3D updates while draping.
- Notch matching across seams.
- The T-shirt end to end.

**Later milestones:**
- Self-collision comes with fitted garments.
- Fabrics come in M6.

## 1. What the student does, and what is saved

**Flow:**
1. **Draw** pieces in 2D (M2b).
2. **Sew** with the Sew tool (key **W**; S is taken by Rectangle). See section 2.
3. **Arrange** in 3D. Pieces start flat in front of the form. Click one to show the gizmo, or right-click → **Place at…**. See section 4.
4. **Drape:** **Play** builds the fabric and runs the simulator. **Reset** returns to the arrangement. See section 5.

The 3D view shows the project's pieces. The two built-in demo garments (skirt, fitted tube) leave the app's 3D view and stay in `crates/testkit` for the automatic tests.

**Saved** (store once, derive the rest, as in M2b):
- `Project.seams: Vec<Seam>`.
- `Piece.placement: Option<Placement>` and `Twin.placement: Option<Placement>`.
  - A piece with `None` starts at its derived starting position.
  - A twin with `None` mirrors its piece's placement.
- **Undo:** sewing, every gizmo drag, **Place at…** and typed placement values are each one undo step, through `Document`. `Project::check()` refuses anything invalid.

**Format:** the schema version goes up by one. The dress-forms work also bumps it; whichever merges second takes the next number and updates its migration and fixture. Older files load with no seams and no placements. A new frozen fixture covers the new version, and older fixtures stay.

**Part 1 fabric:** one light cotton (M1's parameters). The grain angle is kept, but it has no effect until fabrics arrive.

## 2. Sewing in the 2D editor

### Model (`crates/core`)

```rust
pub struct SeamId(pub u32);
pub enum Half { Drawn, Pale }      // which half of a folded piece; Drawn for everything else
pub struct SeamSide {
    pub shape: PieceId,            // a piece's id or a twin's id
    pub half: Half,
    pub first_edge: usize,         // stored edge index
    pub edges: usize,              // number of consecutive edges, ≥ 1
    pub forward: bool,             // runs along the stored outline's direction
}
pub struct Seam { pub id: SeamId, pub a: SeamSide, pub b: SeamSide }
```

- **How the edges run:**
  - A side's edges are consecutive along the outline and may wrap past the last edge.
  - The two sides are matched end to end: a's start meets b's start.
  - On a folded piece a side never includes the fold edge and never crosses the fold.
- **Mirrored seams are derived, not stored.** A seam has a mirror when both of its sides have a mirror image:
  - the mirror of a side on a folded piece is the same edges on the other half;
  - the mirror of a side on a member of a pair is the same edges on the other member.

  The mirror is drawn, meshed and stitched like any seam. It is edited and deleted together with its seam.
- **`Project::check()` refuses:**
  - a missing shape or edge;
  - `edges` of 0 or more than the outline;
  - `Half::Pale` on a piece that isn't folded;
  - a side that includes the fold edge;
  - any edge used by two seams, mirrors included;
  - more than 2,000 seams.
- **Edits keep seams valid** (each is one undo step):

  | Edit | What happens to the seam |
  |---|---|
  | Add a point on a sewn edge | The edge splits and the side grows by one edge. Both parts stay sewn. |
  | Remove a point between two sewn edges of a side | The side shrinks by one. |
  | Remove a point at a side's end | The side shrinks. A side that would drop to 0 edges deletes its seam. |
  | Delete a piece or twin | Its seams are deleted. |
  | **Unfold** | A derived mirror seam on the piece's pale half becomes an ordinary stored seam. |
  | **Remove fold** or **Break pair** | Derived mirrors simply disappear. A stored seam with a side on the removed pale half is deleted. |

### Sew tool (key W)

- **Making a seam:**
  - Click an edge to start the first side. Shift-click adds the next edge along the outline, in either direction.
  - Click an edge for the second side to **make the seam**, so the common case is two clicks.
  - Shift-clicks straight after that extend the second side.
  - Esc cancels a half-made seam. Clicking empty space ends extending.
- **Which ends meet:** set by where each first click lands. The end of the clicked edge nearer the click is that side's start.
- **What can be sewn:**
  - Any edge of any shape, including a twin's edges and a folded piece's pale half. A fold edge itself can't be sewn.
  - An already-sewn edge is refused with "This edge is already sewn."
- **Drawing:**
  - Each seam has its own colour from a fixed palette, drawn along both sides with a number badge.
  - The selected seam is thicker and shows thin guide lines joining its two starts and its two ends, so a flipped seam shows crossed guides.
- **Seam panel** (select a seam by clicking its line with Edit or Sew):
  - each side's length;
  - the difference, in amber over 3 mm: "Lengths differ by 4 mm";
  - **Flip** (toggles `b.forward`, keeping b's edges);
  - **Delete seam**.
  - All strings go through Fluent.

## 3. Pieces into fabric (`crates/mesh`)

`crates/mesh` is a new pure crate (no GPU, no windows). It depends on `core` and `geom`, and adds `spade` = 2.15.1 (MIT OR Apache-2.0).

**What becomes fabric:**
- The area inside the **stitching line** of every shape from `geom::shapes`.
- A folded piece becomes one whole panel, with the fold inside it.
- Each twin is its own panel.
- **Cut-out** lines become holes. **Marking** lines don't affect the fabric.

**How:**
- **Edges:**
  - Every edge is sampled at a target step `h` (12 mm by default).
  - An edge that isn't sewn gets `max(1, round(len / h))` steps.
  - Both sides of a seam get the same count, `max(1, round(max(lenA, lenB) / h))`, spread at equal arc-length fractions along each side. A longer side gathers as ease.
- **Inside:** a constrained Delaunay triangulation with refinement (spade, `keep_constraint_edges`). This keeps every boundary sample, so seam counts stay matched. The angle limit is 25° and the maximum area is set from `h`.
- **Each panel carries:**
  - its flat positions in metres, which are the rest shape;
  - triangles;
  - per-edge boundary vertex lists.
- **Seam stitches:** pairs of boundary vertices, matched by index along both sides, including derived mirrors.
- **Size limit:** the whole garment stays under 30,000 particles. If the estimate is over, `h` grows until it fits, and a note says "Large pattern: using coarser fabric".
- **Placement:** turns each panel's flat positions into 3D starting positions (section 4).

**Failures:** a shape that can't be meshed (its outline crosses itself, or triangulation fails) is left out. Its seams are left out too, and a note names it: "Front couldn't be made into fabric: its outline crosses itself." The rest still drapes.

**API sketch:**

```rust
build(project, &MeshParams) -> GarmentMesh {
    panels: Vec<PanelMesh>,
    stitches: Vec<((panel, vertex), (panel, vertex))>,
    notes: Vec<MeshNote>,
}
```

The app turns this into a `sim::Cloth` with `ClothBuilder::add_panel` (with `flat`), `stitch`, then `build`.

## 4. Arranging in 3D

### Placement (`crates/core`)

`Placement { position: [f64; 3] (m), rotation: [f64; 4] (unit quaternion), curve: Option<f64> (radius, m) }`

- **Frame:** the form's.
  - Metres, with y up from the floor.
  - The form faces +z, and its left is +x.
  - x = 0, z = 0 is its centre line.
- **Applying a placement to a panel:**
  1. Centre the flat panel on its bounding-box centre.
  2. If `curve` is set, wrap it around a vertical cylinder of that radius whose axis lies behind the piece, so its sides bend back around the form.
  3. Rotate, then translate to `position`.
- **Validation:** `check()` refuses non-finite values, a rotation that isn't unit length (within 1e-6 after normalising), a curve radius that isn't between 5 cm and 2 m, and a position more than 10 m from the origin.

### Starting position (derived when `placement` is `None`)

- The whole 2D layout is drawn at real size on a vertical plane 40 cm in front of the form's centre line.
- The layout is centred left–right on the form, with its top at the form's shoulder height.

### The 3D view

**Two states:**
- **Arranging:** before Play, or after Reset. It shows the form and the pieces at their placements, and the gizmo works.
- **Draping:** after Play. The simulator runs and the gizmo hides. A hint says "Press Reset to move pieces."

**Selecting:**
- Clicking a piece selects it. A ray from the camera is tested against the pieces' triangles on the CPU.
- 2D and 3D share one selection: `Selection::Piece(shape id)`.
- Clicking empty space clears it.
- **Front / Back / Left / Right** buttons turn the camera to that side.

**The gizmo** is built into OpenDrape. It is drawn with egui's painter over the 3D image, projected with the orbit camera, and centred on the selected piece.
- **Moving:**
  - Arrows for x (red), y (green) and z (blue) move along that world axis. Pointer movement is projected onto the axis's on-screen direction, and that point is mapped back onto the 3D axis exactly. *(probe: converting at the gizmo's depth was up to 35% off for long drags close up.)*
  - A centre square moves the piece in the camera's view plane.
- **Rotating:** rings around x, y and z rotate about the piece's centre. Hold Shift to snap to 15°. A ring seen at a grazing angle (its axis more than 60° from the line of sight, as the y ring is in the default view) turns with the pointer's movement across it, divided by the ring's on-screen radius, a steady number of degrees per point that cannot race or flip, while a ring seen nearly face-on turns with the pointer's angle in the ring's own plane.
- **Feedback:**
  - The handle under the pointer is highlighted.
  - While dragging, a readout shows the move or angle in the user's units ("12 cm up", "45°").
- **Undo:** one drag is one undo step.

**Typed placement:** a **3D placement** group in Properties has Position x/y/z in the user's units and Rotation in degrees about x, y and z (applied in that order). It uses the M2a `text_field` rules.

**Place at front / back / left side / right side** (right-click menu on a piece, in 3D or 2D):
- **Where it goes:**
  - The angle around the form is 0°, 180°, +90° (form's left) or −90°.
  - The piece keeps its current height.
  - Its curve radius is the largest surface distance over the heights and angles the piece covers, plus 3 cm, or 20 cm when no ray finds the form. Distances are measured by rays outward from the centre line (`BodyCollider::ray_exit`). *(probe: one ray at the piece's own height found the gap between the legs and would wrap a skirt through the hips.)*
- **Folded piece:** centred on that angle, with its fold on the centre line.
- **A member of a pair:** placed beside its partner across the centre line. The edges that face each other in the 2D layout meet at the centre line.
- **Flat** clears the curve.

**Mirrored pairs:** a twin without its own placement mirrors its piece's placement across x = 0, or, while the piece has none either, starts from its own place in the 2D layout. Once the student moves the twin itself, it keeps its own placement.

## 5. Draping, the body, problems

**Draping:**
- **Play:**
  - Takes a snapshot of the project (`Arc<Project>`).
  - Builds the mesh and the cloth **on the simulation thread**, so the window never stalls.
  - Runs the solver with M1's parameters: stitch close time, weld, auto-pause when settled.
- **Reset:** returns to arranging.
- **Editing during a drape:** a pattern edit while draped also returns to arranging. Live warm-start is M4b.

**Body boundary:**
- The 3D view, picking, **Place at…** and the solver use the body through one small boundary in the app, a `Stage` that gives:
  - the render mesh;
  - the collider;
  - the centre line;
  - the floor height;
  - the surface distance at an angle and height.
- Today it wraps the MakeHuman body. The dress-forms Track B swaps in the chosen form behind the same boundary.

**Floor:** a plane at y = 0. If dress forms' `CompoundCollider` (with floor) has merged, use it. Otherwise add a minimal "collider plus floor" wrapper behind the `Stage`, which is removed when forms arrive. An unsewn piece falls and rests on the floor.

**Problems** (shown as notes; draping goes ahead where it can):

| Problem | What happens |
|---|---|
| Seam lengths differ | Warning only; the extra length gathers as ease. |
| A shape can't be meshed | It is left out and named (section 3). |
| A piece starts inside the form (any starting point inside the collider) | "Front starts inside the form; move it out first." It still drapes. |
| A very large pattern | Coarser fabric (section 3). |
| The simulation goes non-finite | It stops, returns to arranging, and says "The drape went wrong and was reset. Check for seams that pull pieces through the form." |

**Performance:** a drafted skirt runs at 20 frames per second or more on the CI-comparable machine. Quality presets for weak laptops come later.

## Testing (all headless; the app is never launched)

- **core/io:**
  - Seam and placement validation.
  - Each edit rule in section 2.
  - Derived mirrors.
  - Save → load round trip of every new field.
  - The new-version fixture.
  - Older fixtures still load.
- **mesh:**
  - Smallest angle ≥ 20°, except triangles touching an outline corner sharper than 20°, and ≥ 25° more than 4 h from corners sharper than 45°. *(probe: no triangle can beat a sharper corner.)*
  - Edge lengths within 0.5–1.6 h.
  - Area within 0.5% of the stitching outline.
  - Seam sides always have equal counts.
  - Holes for cut-outs.
  - A random-pattern fuzz that never panics.
  - Folded and twin panels.
- **Placement and gizmo maths:**
  - Applying placements and curves.
  - **Place at…** on a cylinder stand-in.
  - Arrow drags at many camera angles move the expected distance.
  - Ring drags rotate the expected angle.
  - Ray picking finds the right piece.
- **egui_kittest:**
  - Sew tool flows (two clicks, shift-extend, refusal, Flip, Delete, mirrors).
  - The seam panel.
  - 3D select, gizmo drag and undo, driven through the gizmo's input handler with a fixed camera.
  - **Place at…**.
  - Typed placement.
  - Play and Reset state changes.
- **testkit drape:** a skirt built the way a student would (folded front, mirrored back pair, side seams and centre back, Place at…) is built through `crates/mesh` and draped. It must meet:
  - no non-finite values;
  - max penetration ≤ 2 mm (p99 ≤ 1 mm);
  - before welding, seam gap ≤ 4 mm max and ≤ 1 mm mean, then welded shut *(probe: the worst gap before welding is 2.7 mm, over the seat; a flipped seam measures 11.5 mm, so the gate still catches mistakes)*;
  - kinetic energy settles;
  - a deterministic position hash.

## Constraints

- Ask the user before running the OpenDrape app, or anything that opens a window, on their Mac.
- Ask before pushing, merging or publishing.
- The only new dependency is `spade` 2.15.1 (MIT OR Apache-2.0). The gizmo is OpenDrape's own code. `cargo deny check` passes.
- Never use the name "CLO". Every string goes through Fluent.
- Keep the dress-forms branch's files out of M4a. Coordinate only through the `Stage` boundary and the schema version number.

## M4b (next)

- Free sewing (ranges within edges, 1:2 joins such as a sleeve cap).
- Arms and sleeve arrangement.
- Grab and pin fabric in 3D.
- Live 2D→3D warm-start.
- Notch matching across seams.
- The T-shirt end to end.
