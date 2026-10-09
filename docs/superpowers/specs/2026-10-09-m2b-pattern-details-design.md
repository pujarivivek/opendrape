# OpenDrape M2b: Pattern Details (Design)

**Date:** 2026-10-09.

**Status:**
- Approved in conversation.
- The written document is waiting for the user's review.

**Builds on:**
- M2a (pattern editor core), merged and published as `0e74aff`.
- Main spec: `docs/specs/2026-10-09-opendrape-design.md`. Its M2 row is split into M2a and M2b.

**Research:** CLO3D (a behaviour reference only, never named in the product), Browzwear, Seamly2D, FreeSewing, the ASTM D6673 summaries and the NIMI CITS exercises. Read on 2026-10-09; the sources are listed at the end.

## Purpose

M2b finishes the 2D pattern editor so that a student's pieces carry everything a paper pattern carries:
- seam allowance;
- notches;
- internal markings;
- cut-on-fold pieces;
- left/right pairs.

M3 then sews these pieces in 3D (notches split seams, e.g. a 1:2 sleeve cap), and M7 exports them to tiled PDF and DXF-AAMA (the cut line, notches and markings each get their own layer).

**Who it's for:** fashion students in India and Africa, many new to CAD, on low-end laptops.

**Success:** a tester can follow the checklist below on a Mac with no help. Every step is also covered by a headless test.
1. Draft a skirt front on the fold with a 1 cm seam allowance and a 3 cm hem.
2. Add notches at the hip.
3. Add a mirrored pair of back panels.
4. Save, reopen, and undo.

## Decisions made with the user (2026-10-09)

| Topic | Decision |
|---|---|
| Order | M2b before M3 |
| Default seam allowance | 1 cm on every edge; an edge marked **Hem** gets 3 cm |
| M2a leftovers | Folded into M2b: label placement, Tab in the number box, Dock → Quit, crafted huge files |
| Symmetric pieces and pairs | **Store once, derive the rest.** Only the drawn half or master piece is stored; the other half, the twin and the cut line are computed |

## What the student sees

### Seam allowance
- **New pieces:** every piece gets a 1 cm allowance. It is drawn as a light band outside the stitching line, with the cut line on the band's outer edge. A toolbar checkbox, **Show seam allowance** (on by default), hides it.
- **Old files:** pieces from M2a files get the same default when opened.
- **Piece panel:** a **Seam allowance** field sets the piece default.
- **Edge panel:**
  - **Seam allowance**: an override for this edge, shown with a "same as piece" state.
  - **Hem** checkbox: sets the edge to 3 cm and changes its corners (see the next bullet).
- **Corners are automatic, with no menu:**
  - an ordinary corner is mitered (pointed);
  - a corner sharper than the miter limit is squared off;
  - at both ends of a hem edge, the neighbouring seam's allowance is mirrored about the hem stitching line, so the hem folds up flat against the side seam.
- **The fold edge never gets allowance.** The cut line runs straight into the fold.
- **Only the stitching line is ever drawn or edited.** The cut line is always computed from it, so the allowance can never be doubled.

### Notches
- **Notch tool (N):** click on an edge to add a notch at that point. Typing a number places it exactly that far, along the stitching line, from whichever end of the edge is nearer the pointer. It is stored as a distance from the edge's start point.
- **Notch panel:**
  - **Distance** from the edge's start point;
  - **Marks:** single (default), double or triple, 3 mm apart;
  - **Style:** slit (default) or V;
  - **Delete**.
- **Drawing:** on the cut line, pointing inwards, perpendicular to the stitching line. They are 5 mm deep, but never deeper than 60 % of that edge's allowance. With no allowance they sit on the stitching line.
- **Attachment:** a notch belongs to an edge and keeps its distance from the edge's start point when points move. If the edge gets shorter than that distance, the notch sits at the edge's end. Splitting or removing edges moves notches to the right edge.
- **Not allowed** on a fold edge.

### Internal lines
- **Internal line tool (L):** works like the pen. Click points, press and drag for curves. **Return** finishes an open line; clicking the first point closes a shape.
- **Attachment:** the line belongs to the piece the first point is in. Points may touch the outline but cannot be outside it; a point outside is refused with a notice.
- **Line panel:**
  - **Kind:** **Marking** (default; placement, centre front, button, fold or press lines) or **Cut-out** (closed shapes only; a hole that gets cut);
  - total length;
  - **Delete**.
- **Editing:** the Edit tool drags line points, curve handles and whole lines. Lines move with their piece.

### Symmetric (cut-on-fold) pieces
- **Setting the fold:** select a **straight** edge, then **Set as fold line**.
- **Display:**
  - the whole piece is shown: the drawn half as usual and the mirrored half pale;
  - the fold line is dashed, with a two-headed "Place on fold" arrow across it;
  - edits to the drawn half (points, curves, allowance, notches, internal lines) show on the pale half at once.
- **The pale half can't be edited directly;** clicking it selects the piece.
- **Measurements:** Area and Perimeter in the piece panel describe the full unfolded piece.
- **Piece panel actions:**
  - **Unfold** replaces the half with one ordinary full piece. It is one undo step, so Cmd+Z folds it back up.
  - **Remove fold** keeps just the drawn half as an ordinary piece.

### Mirrored pairs
- **Making a pair:** the piece panel's **Make mirrored pair** creates a mirror-image twin beside the original. The twin is named "<name> (mirror)" and can be renamed.
- **Shared and separate:**
  - Shared: outline, curves, seam allowance, hems, notches and internal lines. Editing either piece updates both; edits on the twin are mapped back through the mirror.
  - Separate: name and position. Dragging a whole twin moves only the twin.
- **Display:** both pieces show a small link badge.
- **Breaking the pair:** **Break pair** makes the twin an ordinary independent piece with its current shape. It can be undone.
- **Limits:** a piece can't be both folded and paired in M2b, and a twin can't be paired again.

### Polish carried over from M2a
- **Edge-length labels** sit just outside the piece, off the line.
- **Tab** in the floating number box cycles between its two fields instead of closing the box.
- **Dock → Quit, logout and shutdown:**
  - macOS lets an app run code on the way out, but not ask a question. So when there is unsaved work and the student didn't choose "Don't save", OpenDrape writes a recovery copy to its app-data folder.
  - On the next launch it asks: *"OpenDrape closed with unsaved work. Restore it?"* with **Restore** and **Discard**.
  - The recovery copy is deleted after either answer.
- **Crafted huge files:** each piece's fill is triangulated from a cached outline with a capped number of points. A deliberately giant file can't stall drawing, and normal pieces look the same.

## How it works

### Data model and file format (version 2)

All changes are in `opendrape-core`.

**Piece** gains:
- a piece-level seam allowance in mm (default 10);
- per-edge properties: an optional allowance override and a hem flag (hem default 30 mm);
- a list of notches: edge, distance in mm from the edge's start along the stitching line, mark count 1 to 3, slit or V;
- a list of internal lines: points and straight or cubic edges like an outline, open or closed, Marking or Cut-out;
- an optional fold: the index of a straight edge;
- an optional twin: its own piece id, name and position offset. The twin's shape is never stored.

**Invariants:**
- The edit methods that insert, split or remove vertices keep the per-edge properties, notches and fold index consistent.
- `Piece::check()` enforces:
  - the fold edge is straight;
  - notch edges exist and distances are finite and non-negative;
  - internal lines have at least 2 points (3 if closed);
  - allowances lie in 0–100 mm;
  - folded and twinned are not both set;
  - the M2a size limits now count internal-line points too.

**Ids:** twin ids come from the same counter as pieces, so selections, undo and the file all treat a twin as a piece id.

**File format:**
- `SCHEMA_VERSION` becomes 2.
- The v1 → v2 step in `opendrape-io`'s migration chain adds the defaults: allowance 10 mm, no hems, no notches, no lines, no fold, no twin. That matches "M2a pieces get the 1 cm default".
- The frozen v1 fixture keeps passing through migration, and a frozen v2 fixture is added.

### Resolving a piece (`opendrape-geom`)

One function, `resolve_piece`, turns a stored piece into what everything else uses:
- **The full stitching outline:** for a folded piece, the drawn half mirrored across the fold line and joined. The fold edge itself is interior and is dropped.
- **The twin's outline:** reflected left–right, then offset.
- **The cut line:** computed on the full outline. The fold contributes zero allowance.
- **Notch marks:** positions and directions on the cut line.
- **Internal lines:** mirrored where needed.

Painting, hit-testing, measurements and the later exporters all use this one result, so they always agree. Edits always apply to the stored half or master. A click on a twin is mapped back through the reflection; the pale half is read-only.

**Cut-line algorithm:**
1. Offset each stitching edge outward by its width.
2. Join neighbours at their corners: mitered, squared beyond the miter limit, or mirrored at hem ends.
3. Remove self-intersections from tight curves.

The plan starts with a short probe comparing kurbo's curve offsets with offsetting the flattened polyline (0.1 mm tolerance), and the `i_overlay` cleanup named in the main spec. The plan picks one based on that probe and records the choice.

### Editor

- **Toolbar:**
  - new tools **Notch (N)** and **Internal line (L)**;
  - a **Show seam allowance** checkbox;
  - the existing tools are unchanged.
- **Properties panel:**
  - new Notch and Line sections;
  - new fields and buttons on the Piece and Edge sections (allowance, Hem, Set as fold line, Unfold, Remove fold, Make mirrored pair, Break pair).
- **Behaviour carried over from M2a:**
  - Selection gains notch and internal-line targets, validated after undo, redo and delete like today.
  - Every change is one undo step.
  - Changes that would make the project invalid are refused with a notice.

### Recovery copy (app)

- **When it's written:** `eframe::App::on_exit` writes `recovery.odp` into OpenDrape's app-data folder (the same place as the graphics settings) when the document has unsaved changes and the student didn't choose "Don't save".
- **On start-up:** if the file exists and loads, a modal offers **Restore** or **Discard**. Restore opens it as unsaved work; if it came from a saved file, it remembers that file, so **Save** writes back to it.
- **Afterwards:** the file is deleted after either answer. If it can't be read, it is discarded silently and the event is written to the start-up log.

## Testing

All tests are headless; nothing opens a window on the user's Mac.

- **Property tests:**
  - the cut line contains the stitching line;
  - every cut-line point lies at least the edge's allowance away from the stitching line, except at squared corners;
  - mirroring is exact, to 1e-9 mm;
  - fold edges get zero allowance;
  - notches keep their distance when points move, and move with split and removed edges.
- **Pattern-window tests** (egui_kittest), following the checklist:
  - set a fold and see the full piece;
  - place notches by click and by typed distance;
  - draw internal lines;
  - make and break a pair;
  - edit a twin and check the master changes;
  - use the Hem checkbox;
  - check allowance overrides;
  - undo after each.
- **File tests:**
  - v1 → v2 migration of the frozen v1 fixture;
  - a frozen v2 fixture;
  - a round trip of every new field.
- **App tests:**
  - the recovery copy is written when the student hasn't chosen "Don't save" and skipped after "Don't save";
  - a scripted start-up offers Restore and Discard.
- **Polish:**
  - labels sit outside the outline;
  - Tab cycles between the box fields;
  - a hostile-file timing test stays under budget.

## Out of scope for M2b

- **Darts:** M4.
- **Sewing and arranging pieces in 3D:** M3.
- **PDF and DXF export:** M7. The data needed for them is stored now.
- **Corner-type menus.**
- **Notch styles beyond slit and V:** T, castle and U wait for the DXF exporter.
- **A "split edge into N notches" helper.**
- **Re-folding an unfolded piece** other than by undo.
- **Pieces that are both folded and paired.**
- **Autosave on a timer:** the recovery copy only covers quitting.

## Sources

The full research notes were kept outside the repo, in the session scratchpad. The main sources were:
- CLO3D help-centre articles on seam allowance, notches, symmetric editing and linked editing;
- Seamly wiki and forum (seam allowance, fold, notches);
- FreeSewing notation and seam-allowance docs;
- Browzwear symmetry articles;
- Patro's DXF-ASTM summary and the DH Patterns ePattern ASTM paper;
- NIMI CITS Fashion Design Technology exercises;
- Wikipedia "Seam allowance".
