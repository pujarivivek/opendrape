# OpenDrape M2a (Pattern Editor Core) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Students draft pattern pieces from scratch in a 2D pattern window beside the 3D view, CLO-style:
- draw with a pen tool (click for corners, drag for curves);
- type exact lengths and angles while drawing;
- draw rectangles;
- add points on edges;
- edit points, curve handles and whole pieces;
- see and change measurements in a properties panel;
- switch cm/inch;
- undo/redo;
- save and open `.odp` project files.

**Architecture:**
- **New crates**, with no GPU, so each is testable on its own:
  - **`crates/core`** (`opendrape-core`): the document model. `Project` holds `Piece`s; a piece is closed vertices plus straight/cubic edges, in mm with y up. It also owns `Units` and the schema version.
  - **`crates/geom`** (`opendrape-geom`): curve maths on that model with **kurbo**: lengths, area, hit-testing, splitting edges, setting an edge's length.
  - **`crates/io`** (`opendrape-io`): `.odp` = zip containing `project.json` (**zip 8.6**, deflate only), with version checks and atomic saves.
- **The app** gains an `editor` module:
  - `Document`: undo/redo snapshots, merging and drag gestures.
  - `View`: y-up pan/zoom transform.
  - The canvas with four tools.
  - A floating length box.
  - The properties panel.
  - Painting, with concave fills via **earcut**.
- **App wiring:** CLO-style split layout (3D left, 2D right), File/Edit menus, native file dialogs (rfd async, swapped for a scripted fake in tests), and an unsaved-changes guard.

**Tech Stack:**
- Existing: Rust 1.99, eframe/egui/egui_kittest 0.36.2.
- New: kurbo 0.13.1 (already in the lockfile via epaint), earcut 0.4.11, zip 8.6.0 (`default-features = false, features = ["deflate-flate2-zlib-rs"]`; zip 9.0.0 is too new), rfd 0.17 (already a dependency).

**Spec:** `docs/specs/2026-10-09-opendrape-design.md` (M2 row; data model; file format). **M2 is split** into M2a (this plan) and M2b (seam allowance, notches, internal lines, symmetric/linked-mirror pieces); the user chose this on 2026-10-09.

**User decisions (2026-10-09):**
- CLO-style split layout, with 3D on the left and 2D on the right.
- Measurements are typed while drawing and edited later in a panel.
- Two releases (M2a, then M2b).

**Evidence:** every egui, kurbo, zip and rfd API used here was compiled and tested headlessly on 2026-10-09 in `scratchpad/m2-probe`: 19 tests. They cover clicking and dragging on a custom canvas, the floating number box (Tab between Length and Angle, Enter, Escape), pan/zoom at the cursor, shortcuts, nested panels, panel text fields that apply on Enter or click-away, a dialog box, and cancelling a window close.

**UX reference:** CLO's 2D tools, researched 2026-10-09. We keep CLO's key letters and drag-to-curve so its users feel at home, and fix its documented beginner traps:
- **One curve model:** Bézier only. CLO has two incompatible curve systems.
- **Typed length *and* angle:** the box opens as soon as you type a number. CLO hides it behind right-click and has no angle.
- **Undo while drawing:** Cmd/Ctrl+Z or Backspace removes only the last point. In CLO it wipes the whole shape.
- **Units:** cm/inch switches without a restart.

## Global Constraints

Carried over from M0/M1:
- pinned toolchain, egui family `=0.36.2`, GPL-3.0-or-later, `cargo deny check` passes;
- never use the name "CLO";
- every user-visible string goes through `tr!`;
- **never launch the GUI app or anything that opens a window or dialog on the user's Mac without asking first.** Use headless egui_kittest and unit tests. Native file dialogs are never shown in tests.

New in M2a:
- **Pattern coordinates:** f64 millimetres, **y up**. Edge `i` runs from vertex `i` to vertex `(i+1) % n`. A curve edge is a cubic Bézier with control points `c1` (start end) and `c2` (end end). Pieces have ≥ 3 vertices, and `edges.len() == vertices.len()`.
- **Units:**
  - Stored in `Project::units` (cm by default) and only change how numbers are shown and typed.
  - Display: cm with 1 decimal, inch with 2.
  - Typed input accepts `,` or `.` as the decimal separator.
- **Angles:** degrees, anticlockwise from +x, y up. Grain defaults to 90° (straight up the piece).
- **Undo:**
  - Every user change is exactly one undo step: a click, a whole drag gesture, a panel field applied with Enter (or by clicking elsewhere), a closed pen piece, or a units switch.
  - Undo keeps up to 200 steps.
  - While a pen piece is being drawn, Cmd/Ctrl+Z and Backspace remove the last point instead.
- **Keyboard shortcuts:**
  - Tool keys: Z Edit, H Pen, S Rectangle, X Add point, F fit view. They only act when no text field has focus and the length box is closed.
  - Redo (Shift+Cmd/Ctrl+Z, or Cmd/Ctrl+Y) is checked before undo (Cmd/Ctrl+Z), because `consume_shortcut` ignores extra Shift. The same rule puts Save As (Shift+Cmd+S) before Save.
- **Hit radius:** 8 screen points, converted to mm by the current zoom.
- **`.odp` file format:**
  - A zip with a single `project.json` entry, deflated, with no timestamps, so the bytes are reproducible.
  - Loading refuses files with a newer `schema_version` (with a message to update) and anything that fails `Project::check()`.
  - Saving writes `<file>.odp.tmp`, then renames it over the target.
- **egui rules learned in the probe:**
  - Fill concave pieces via earcut into an `egui::Mesh` (epaint fills only convex shapes).
  - Drags are absolute: `original + (now − grab)`, never a sum of `drag_delta()`.
  - Hit-test at `pointer.press_origin()` when a drag starts.
  - Escape clears focus before our code runs, so while the length box is open the canvas ignores Enter and Escape.

## Review Focus

1. **Typing exact numbers:** comma decimals (`34,5`), empty fields, `0`, negative or absurd values (`1e9`), and non-numbers must never create a degenerate edge or crash. Invalid input leaves the drawing unchanged. Tested in Task 1 (`parses_typed_numbers`), Task 2 (`set_edge_length_rejects_nonsense`), Task 5 (`typed_length_rejects_nonsense`, `a_flat_or_zero_size_rectangle_is_refused`) and Task 7 (`panel_refuses_nonsense_lengths`).
2. **Stale selection after undo, redo or delete:** a selected point or edge whose piece or index no longer exists must not panic in the panel, painting or Delete. Tested in Task 6 (`selection_survives_undo_of_its_piece`).
3. **Degenerate geometry:**
   - Clicking the same spot twice (double-click), closing with fewer than 3 points, zero-size rectangles, splitting an edge right at a vertex, deleting down to 3 points.
   - All are refused or ignored, never stored.
   - Tested in Tasks 1, 2, 5 and 6.
4. **Bad or hostile project files:** not a zip, a zip without `project.json`, malformed JSON, a newer version, invalid geometry (NaN, 2 vertices), or a huge entry. Each gives a clear error; the current project is never replaced. Tested in Task 3 and Task 8 (`opening_a_bad_file_keeps_the_current_project`).
5. **Losing work:** New, Open and closing the window with unsaved changes ask first, and Cancel keeps everything. Saving to a path without `.odp` adds it. Tested in Task 8.

---

## File Structure

```
Cargo.toml                       + members core/geom/io; deps kurbo, earcut, zip
crates/core/   Cargo.toml, src/{lib.rs, piece.rs, project.rs, units.rs}
crates/geom/   Cargo.toml, src/lib.rs
crates/io/     Cargo.toml, src/lib.rs
crates/app/src/editor/
  mod.rs        PatternEditor: layout (toolbar, properties, status, canvas), keys, tools, selection
  document.rs   Document: project + undo/redo/merge/gesture + dirty/path
  view.rs       View: y-up pan/zoom transform, fit
  canvas.rs     pointer interaction per tool, pen draft, drags
  length_box.rs floating number box (pen length/angle, rectangle width/height)
  paint.rs      grid, pieces (earcut fill), drafts, handles, labels, grain arrow
  panel.rs      properties panel
crates/app/src/file_dialogs.rs   FileDialogs::{Native, Scripted}
crates/app/src/app.rs            split layout, File/Edit menus, unsaved-changes guard, title
crates/app/i18n/en-US/opendrape.ftl   new strings
crates/app/tests/editor.rs       kittest: pen, edit, rectangle, add point, panel
crates/app/tests/ui.rs           + file flows (save as, open, guards)
docs/testing/M2a-checklist.md
```
All commands assume `source ~/.cargo/env` and the repo root. Work on branch `m2a-pattern-editor`, created from `main`.

---

### Task 1: `opendrape-core`: the pattern model

**Files:**
- Create: `crates/core/Cargo.toml`, `crates/core/src/{lib.rs,piece.rs,project.rs,units.rs}`
- Modify: root `Cargo.toml`

**Interfaces:**
- Produces:
  - `Point2 { x, y }` (Copy, serde) with `new`, `distance`, `lerp`, `length`, `is_finite`, and `Add`/`Sub`/`Mul<f64>`.
  - `PieceId(pub u32)`, `VertexKind {Corner, Smooth}`, `Vertex { pos, kind }` with `corner`/`smooth`.
  - `Edge {Line, Curve{c1,c2}}` (serde tag `type`, lowercase) and `HandleEnd {Start, End}`.
  - `Piece { id, name, vertices, edges, grain_deg }` with:
    - constructors: `polygon`, `rectangle`;
    - navigation: `len`, `is_empty`, `next`, `prev`, `edge_ends`;
    - edits: `move_vertex`, `translate`, `set_curved`, `set_handle`, `set_vertex_kind`, `remove_vertex -> bool`;
    - checking: `check() -> Result<(), ModelError>`.
  - `Project { schema_version, units, pieces }` with `new`, `add_piece -> PieceId`, `piece`, `piece_mut`, `remove_piece`, `next_piece_name(prefix)`, `check`.
  - `SCHEMA_VERSION = 1` and `ModelError`.
  - `Units {Cm, Inch}` with `mm_per_unit`, `from_mm`, `to_mm`, `suffix`, `format_number` ("34.5", the text shown in number fields), `format` ("34.5 cm"), `format_area`, `parse`.

- [ ] **Step 1: Workspace entries and crate skeleton**

Root `Cargo.toml`:
- `members` becomes `["crates/app", "crates/render", "crates/body", "crates/sim", "crates/testkit", "crates/core", "xtask"]`. Add `crates/geom` and `crates/io` in Tasks 2 and 3.
- Add to `[workspace.dependencies]`:
```toml
opendrape-core = { path = "crates/core" }
opendrape-geom = { path = "crates/geom" }
opendrape-io = { path = "crates/io" }
kurbo = "0.13.1"
earcut = "0.4.11"
zip = { version = "8.6.0", default-features = false, features = ["deflate-flate2-zlib-rs"] }
```

`crates/core/Cargo.toml`:
```toml
[package]
name = "opendrape-core"
description = "OpenDrape document model: pattern pieces and projects"
version.workspace = true
edition.workspace = true
rust-version.workspace = true
license.workspace = true
repository.workspace = true
publish.workspace = true

[dependencies]
serde.workspace = true

[dev-dependencies]
serde_json.workspace = true

[lints]
workspace = true
```

`crates/core/src/lib.rs`:
```rust
//! OpenDrape's document model: pattern pieces made of straight and curved edges, in
//! millimetres with y up. Pure data with no geometry or GPU dependencies, so it is easy to
//! save, compare and test.

mod piece;
mod project;
mod units;

pub use piece::{Edge, HandleEnd, Piece, PieceId, Point2, Vertex, VertexKind};
pub use project::{ModelError, Project, SCHEMA_VERSION};
pub use units::Units;
```

- [ ] **Step 2: Write the types with `todo!()` bodies and failing tests**

`crates/core/src/units.rs`:
```rust
use serde::{Deserialize, Serialize};

/// How lengths are shown and typed. Stored values are always millimetres.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Units {
    #[default]
    Cm,
    Inch,
}

impl Units {
    pub fn mm_per_unit(self) -> f64 {
        todo!()
    }
    pub fn from_mm(self, _mm: f64) -> f64 {
        todo!()
    }
    pub fn to_mm(self, _value: f64) -> f64 {
        todo!()
    }
    pub fn suffix(self) -> &'static str {
        todo!()
    }
    /// The number alone, as shown in a text field: "34.5" (cm, 1 decimal) or "13.58" (inch, 2).
    pub fn format_number(self, _mm: f64) -> String {
        todo!()
    }
    /// "34.5 cm" or "13.58 in".
    pub fn format(self, _mm: f64) -> String {
        todo!()
    }
    /// "120.0 cm²" or "18.60 in²".
    pub fn format_area(self, _mm2: f64) -> String {
        todo!()
    }
    /// A typed number: accepts "34.5", "34,5" and surrounding spaces; `None` unless finite.
    pub fn parse(_text: &str) -> Option<f64> {
        todo!()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn converts_and_formats() {
        assert_eq!(Units::Cm.to_mm(34.5), 345.0);
        assert_eq!(Units::Inch.from_mm(254.0), 10.0);
        assert_eq!(Units::Cm.format_number(345.0), "34.5");
        assert_eq!(Units::Inch.format_number(100.0), "3.94");
        assert_eq!(Units::Cm.format(345.0), "34.5 cm");
        assert_eq!(Units::Inch.format(345.0), "13.58 in");
        assert_eq!(Units::Cm.format_area(12_000.0), "120.0 cm²");
        assert_eq!(Units::Inch.format_area(12_000.0), "18.60 in²");
        assert_eq!(Units::Cm.suffix(), "cm");
    }

    #[test]
    fn parses_typed_numbers() {
        assert_eq!(Units::parse("34.5"), Some(34.5));
        assert_eq!(Units::parse(" 34,5 "), Some(34.5));
        assert_eq!(Units::parse("-2"), Some(-2.0));
        assert_eq!(Units::parse(""), None);
        assert_eq!(Units::parse("abc"), None);
        assert_eq!(Units::parse("NaN"), None);
        assert_eq!(Units::parse("inf"), None);
    }
}
```

`crates/core/src/piece.rs`:
```rust
use crate::ModelError;
use serde::{Deserialize, Serialize};
use std::ops::{Add, Mul, Sub};

/// A position on the pattern table in millimetres, y up. Also used as a 2D vector.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Point2 {
    pub x: f64,
    pub y: f64,
}

impl Point2 {
    pub const fn new(x: f64, y: f64) -> Self {
        Self { x, y }
    }
    pub fn distance(self, other: Self) -> f64 {
        (self.x - other.x).hypot(self.y - other.y)
    }
    pub fn lerp(self, other: Self, t: f64) -> Self {
        self + (other - self) * t
    }
    pub fn length(self) -> f64 {
        self.x.hypot(self.y)
    }
    pub fn is_finite(self) -> bool {
        self.x.is_finite() && self.y.is_finite()
    }
}

impl Add for Point2 {
    type Output = Self;
    fn add(self, o: Self) -> Self {
        Self::new(self.x + o.x, self.y + o.y)
    }
}

impl Sub for Point2 {
    type Output = Self;
    fn sub(self, o: Self) -> Self {
        Self::new(self.x - o.x, self.y - o.y)
    }
}

impl Mul<f64> for Point2 {
    type Output = Self;
    fn mul(self, k: f64) -> Self {
        Self::new(self.x * k, self.y * k)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct PieceId(pub u32);

/// A corner's two curve handles move independently; a smooth point keeps them in line.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum VertexKind {
    #[default]
    Corner,
    Smooth,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Vertex {
    pub pos: Point2,
    #[serde(default)]
    pub kind: VertexKind,
}

impl Vertex {
    pub fn corner(pos: Point2) -> Self {
        Self { pos, kind: VertexKind::Corner }
    }
    pub fn smooth(pos: Point2) -> Self {
        Self { pos, kind: VertexKind::Smooth }
    }
}

/// Edge `i` of a piece runs from vertex `i` to vertex `(i + 1) % n`. A curve is a cubic Bézier
/// with control points `c1` (near the start) and `c2` (near the end).
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum Edge {
    #[default]
    Line,
    Curve { c1: Point2, c2: Point2 },
}

/// Which end of an edge a control point belongs to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HandleEnd {
    Start,
    End,
}

/// One closed pattern piece.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Piece {
    pub id: PieceId,
    pub name: String,
    pub vertices: Vec<Vertex>,
    pub edges: Vec<Edge>,
    /// Grain direction in degrees anticlockwise from +x; 90 runs straight up the piece.
    #[serde(default = "default_grain")]
    pub grain_deg: f64,
}

fn default_grain() -> f64 {
    90.0
}

impl Piece {
    /// A closed piece through `corners`, all edges straight.
    pub fn polygon(_id: PieceId, _name: impl Into<String>, _corners: &[Point2]) -> Self {
        todo!()
    }
    /// Counter-clockwise rectangle with its lower-left corner at `min`.
    pub fn rectangle(_id: PieceId, _name: impl Into<String>, _min: Point2, _width: f64, _height: f64) -> Self {
        todo!()
    }
    pub fn len(&self) -> usize {
        self.vertices.len()
    }
    pub fn is_empty(&self) -> bool {
        self.vertices.is_empty()
    }
    pub fn next(&self, i: usize) -> usize {
        (i + 1) % self.len()
    }
    pub fn prev(&self, i: usize) -> usize {
        (i + self.len() - 1) % self.len()
    }
    /// Start and end point of edge `i`.
    pub fn edge_ends(&self, i: usize) -> (Point2, Point2) {
        (self.vertices[i].pos, self.vertices[self.next(i)].pos)
    }
    /// Moves vertex `i` to `to`. The control points next to it move along, so the curves keep
    /// their shape near the point.
    pub fn move_vertex(&mut self, _i: usize, _to: Point2) {
        todo!()
    }
    pub fn translate(&mut self, _d: Point2) {
        todo!()
    }
    /// Turns edge `i` into a curve (control points at its thirds) or back into a straight line.
    pub fn set_curved(&mut self, _i: usize, _curved: bool) {
        todo!()
    }
    /// Moves one control point of curve edge `i`. At a smooth vertex, the control point on the
    /// other side turns to stay in line (keeping its own length).
    pub fn set_handle(&mut self, _i: usize, _end: HandleEnd, _to: Point2) {
        todo!()
    }
    /// Makes vertex `i` smooth (its handles brought in line) or a corner.
    pub fn set_vertex_kind(&mut self, _i: usize, _kind: VertexKind) {
        todo!()
    }
    /// Removes vertex `i`, joining its two edges into one. Refused (false) below 4 vertices.
    pub fn remove_vertex(&mut self, _i: usize) -> bool {
        todo!()
    }
    /// At least 3 vertices, one edge per vertex, and only finite numbers.
    pub fn check(&self) -> Result<(), ModelError> {
        todo!()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(x: f64, y: f64) -> Point2 {
        Point2::new(x, y)
    }

    fn square() -> Piece {
        Piece::rectangle(PieceId(1), "Front", p(0.0, 0.0), 100.0, 100.0)
    }

    /// Thirds (1/3, 2/3) are not exact in binary, so compare points with a tolerance.
    fn close(a: Point2, b: Point2) {
        assert!(a.distance(b) < 1e-9, "{a:?} vs {b:?}");
    }

    #[test]
    fn rectangle_is_counter_clockwise_with_straight_edges() {
        let s = square();
        let corners: Vec<Point2> = s.vertices.iter().map(|v| v.pos).collect();
        assert_eq!(corners, vec![p(0.0, 0.0), p(100.0, 0.0), p(100.0, 100.0), p(0.0, 100.0)]);
        assert!(s.edges.iter().all(|e| *e == Edge::Line));
        assert_eq!(s.grain_deg, 90.0);
        assert_eq!(s.edge_ends(3), (p(0.0, 100.0), p(0.0, 0.0)));
    }

    #[test]
    fn moving_a_vertex_carries_its_curve_handles() {
        let mut s = square();
        s.set_curved(0, true); // edge 0: (0,0) → (100,0)
        s.set_curved(3, true); // edge 3: (0,100) → (0,0)
        s.move_vertex(0, p(10.0, 5.0));
        assert_eq!(s.vertices[0].pos, p(10.0, 5.0));
        let Edge::Curve { c1, .. } = s.edges[0] else { panic!() };
        close(c1, p(100.0 / 3.0 + 10.0, 5.0));
        let Edge::Curve { c2, .. } = s.edges[3] else { panic!() };
        close(c2, p(10.0, 100.0 / 3.0 + 5.0));
    }

    #[test]
    fn edges_switch_between_straight_and_curved() {
        let mut s = square();
        s.set_curved(1, true);
        let Edge::Curve { c1, c2 } = s.edges[1] else { panic!("edge 1 should be curved") };
        close(c1, p(100.0, 100.0 / 3.0));
        close(c2, p(100.0, 200.0 / 3.0));
        s.set_curved(1, false);
        assert_eq!(s.edges[1], Edge::Line);
    }

    #[test]
    fn smooth_points_keep_their_handles_in_line() {
        let mut s = square();
        s.set_curved(0, true);
        s.set_curved(1, true);
        s.set_vertex_kind(1, VertexKind::Smooth); // the vertex at (100,0) between edges 0 and 1
        s.set_handle(0, HandleEnd::End, p(80.0, -20.0)); // edge 0's control point near (100,0)
        let Edge::Curve { c1: other, .. } = s.edges[1] else { panic!() };
        let (a, b) = (p(80.0, -20.0) - p(100.0, 0.0), other - p(100.0, 0.0));
        assert!((a.x * b.y - a.y * b.x).abs() < 1e-9, "collinear");
        assert!(a.x * b.x + a.y * b.y < 0.0, "on opposite sides");
        assert!((b.length() - 100.0 / 3.0).abs() < 1e-9, "kept its own length");
    }

    #[test]
    fn removing_a_vertex_joins_its_edges() {
        let mut s = square();
        s.vertices.push(Vertex::corner(p(-20.0, 50.0)));
        s.edges.push(Edge::Line); // pentagon
        assert!(s.remove_vertex(4));
        assert_eq!(s.len(), 4);
        assert_eq!(s.edges.len(), 4);
        assert_eq!(s.edge_ends(3), (p(0.0, 100.0), p(0.0, 0.0)));
        assert_eq!(s.edges[3], Edge::Line);
    }

    #[test]
    fn a_piece_keeps_at_least_three_vertices() {
        let mut t = Piece::polygon(PieceId(1), "T", &[p(0.0, 0.0), p(10.0, 0.0), p(0.0, 10.0)]);
        assert!(!t.remove_vertex(1));
        assert_eq!(t.len(), 3);
    }

    #[test]
    fn removing_a_vertex_between_curves_keeps_the_outer_handles() {
        let mut s = square();
        s.set_curved(0, true);
        s.set_curved(1, true);
        let Edge::Curve { c1: keep1, .. } = s.edges[0] else { panic!() };
        let Edge::Curve { c2: keep2, .. } = s.edges[1] else { panic!() };
        assert!(s.remove_vertex(1));
        assert_eq!(s.edges[0], Edge::Curve { c1: keep1, c2: keep2 });
    }

    #[test]
    fn check_rejects_broken_pieces() {
        assert_eq!(square().check(), Ok(()));
        let mut two = square();
        two.vertices.truncate(2);
        two.edges.truncate(2);
        assert_eq!(two.check(), Err(ModelError::TooFewVertices(PieceId(1))));
        let mut mismatch = square();
        mismatch.edges.pop();
        assert_eq!(mismatch.check(), Err(ModelError::EdgeCountMismatch(PieceId(1))));
        let mut nan = square();
        nan.vertices[2].pos.x = f64::NAN;
        assert_eq!(nan.check(), Err(ModelError::NotFinite(PieceId(1))));
    }

    #[test]
    fn serialises_edges_with_a_type_tag() {
        let mut s = square();
        s.set_curved(0, true);
        let json = serde_json::to_string(&s).unwrap();
        assert!(json.contains(r#""type":"curve""#) && json.contains(r#""type":"line""#), "{json}");
        let back: Piece = serde_json::from_str(&json).unwrap();
        assert_eq!(back, s);
    }
}
```

`crates/core/src/project.rs`:
```rust
use crate::{Piece, PieceId, Units};
use serde::{Deserialize, Serialize};

/// Version of the project format written by this build. Bump it when the format changes, and
/// add a migration step in `opendrape-io`.
pub const SCHEMA_VERSION: u32 = 1;

/// Everything a student saves: their pattern pieces and settings.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Project {
    pub schema_version: u32,
    #[serde(default)]
    pub units: Units,
    #[serde(default)]
    pub pieces: Vec<Piece>,
    #[serde(default = "first_id")]
    next_piece_id: u32,
}

fn first_id() -> u32 {
    1
}

impl Default for Project {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ModelError {
    TooFewVertices(PieceId),
    EdgeCountMismatch(PieceId),
    NotFinite(PieceId),
    DuplicateId(PieceId),
    IdCounterBehind(PieceId),
}

impl std::fmt::Display for ModelError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::TooFewVertices(id) => write!(f, "piece {} has fewer than 3 points", id.0),
            Self::EdgeCountMismatch(id) => write!(f, "piece {} has the wrong number of edges", id.0),
            Self::NotFinite(id) => write!(f, "piece {} contains an invalid number", id.0),
            Self::DuplicateId(id) => write!(f, "piece id {} is used twice", id.0),
            Self::IdCounterBehind(id) => write!(f, "piece id {} is ahead of the id counter", id.0),
        }
    }
}

impl std::error::Error for ModelError {}

impl Project {
    pub fn new() -> Self {
        todo!()
    }
    /// Adds `piece` under a fresh id (replacing its own) and returns that id.
    pub fn add_piece(&mut self, _piece: Piece) -> PieceId {
        todo!()
    }
    pub fn piece(&self, _id: PieceId) -> Option<&Piece> {
        todo!()
    }
    pub fn piece_mut(&mut self, _id: PieceId) -> Option<&mut Piece> {
        todo!()
    }
    pub fn remove_piece(&mut self, _id: PieceId) -> Option<Piece> {
        todo!()
    }
    /// Default name for the next new piece: "<prefix> <number>".
    pub fn next_piece_name(&self, _prefix: &str) -> String {
        todo!()
    }
    /// Every piece valid, ids unique and below the id counter.
    pub fn check(&self) -> Result<(), ModelError> {
        todo!()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Point2;

    fn tri() -> Piece {
        Piece::polygon(PieceId(0), "T", &[Point2::new(0.0, 0.0), Point2::new(10.0, 0.0), Point2::new(0.0, 10.0)])
    }

    #[test]
    fn pieces_get_fresh_ids_and_names() {
        let mut pr = Project::new();
        assert_eq!(pr.next_piece_name("Piece"), "Piece 1");
        let a = pr.add_piece(tri());
        let b = pr.add_piece(tri());
        assert_eq!((a, b), (PieceId(1), PieceId(2)));
        assert_eq!(pr.piece(b).unwrap().id, b);
        assert_eq!(pr.next_piece_name("Piece"), "Piece 3");
        assert!(pr.remove_piece(a).is_some());
        assert!(pr.piece(a).is_none());
        assert_eq!(pr.add_piece(tri()), PieceId(3), "ids are never reused");
        assert_eq!(pr.check(), Ok(()));
    }

    #[test]
    fn check_finds_duplicate_and_runaway_ids() {
        let mut pr = Project::new();
        pr.add_piece(tri());
        let mut dup = pr.clone();
        dup.pieces.push(dup.pieces[0].clone());
        assert_eq!(dup.check(), Err(ModelError::DuplicateId(PieceId(1))));
        let mut ahead = pr.clone();
        ahead.pieces[0].id = PieceId(99);
        assert_eq!(ahead.check(), Err(ModelError::IdCounterBehind(PieceId(99))));
    }

    #[test]
    fn missing_optional_fields_get_defaults() {
        let pr: Project = serde_json::from_str(r#"{"schema_version":1}"#).unwrap();
        assert_eq!(pr, Project::new());
    }
}
```

- [ ] **Step 3: Run the tests and watch them fail**

Run: `cargo nextest run -p opendrape-core`
Expected: 14 FAIL, all with `not yet implemented`.

- [ ] **Step 4: Implement**

`units.rs`:
```rust
    pub fn mm_per_unit(self) -> f64 {
        match self {
            Self::Cm => 10.0,
            Self::Inch => 25.4,
        }
    }
    pub fn from_mm(self, mm: f64) -> f64 {
        mm / self.mm_per_unit()
    }
    pub fn to_mm(self, value: f64) -> f64 {
        value * self.mm_per_unit()
    }
    pub fn suffix(self) -> &'static str {
        match self {
            Self::Cm => "cm",
            Self::Inch => "in",
        }
    }
    pub fn format_number(self, mm: f64) -> String {
        match self {
            Self::Cm => format!("{:.1}", self.from_mm(mm)),
            Self::Inch => format!("{:.2}", self.from_mm(mm)),
        }
    }
    pub fn format(self, mm: f64) -> String {
        format!("{} {}", self.format_number(mm), self.suffix())
    }
    pub fn format_area(self, mm2: f64) -> String {
        let per = self.mm_per_unit() * self.mm_per_unit();
        match self {
            Self::Cm => format!("{:.1} cm²", mm2 / per),
            Self::Inch => format!("{:.2} in²", mm2 / per),
        }
    }
    pub fn parse(text: &str) -> Option<f64> {
        text.trim().replace(',', ".").parse::<f64>().ok().filter(|v| v.is_finite())
    }
```

`piece.rs`:
```rust
    pub fn polygon(id: PieceId, name: impl Into<String>, corners: &[Point2]) -> Self {
        Self {
            id,
            name: name.into(),
            vertices: corners.iter().map(|&c| Vertex::corner(c)).collect(),
            edges: vec![Edge::Line; corners.len()],
            grain_deg: default_grain(),
        }
    }
    pub fn rectangle(id: PieceId, name: impl Into<String>, min: Point2, width: f64, height: f64) -> Self {
        let corners = [min, min + Point2::new(width, 0.0), min + Point2::new(width, height), min + Point2::new(0.0, height)];
        Self::polygon(id, name, &corners)
    }
    pub fn move_vertex(&mut self, i: usize, to: Point2) {
        let d = to - self.vertices[i].pos;
        self.vertices[i].pos = to;
        let p = self.prev(i);
        if let Edge::Curve { c2, .. } = &mut self.edges[p] {
            *c2 = *c2 + d;
        }
        if let Edge::Curve { c1, .. } = &mut self.edges[i] {
            *c1 = *c1 + d;
        }
    }
    pub fn translate(&mut self, d: Point2) {
        for v in &mut self.vertices {
            v.pos = v.pos + d;
        }
        for e in &mut self.edges {
            if let Edge::Curve { c1, c2 } = e {
                *c1 = *c1 + d;
                *c2 = *c2 + d;
            }
        }
    }
    pub fn set_curved(&mut self, i: usize, curved: bool) {
        let (a, b) = self.edge_ends(i);
        self.edges[i] = match (curved, self.edges[i]) {
            (true, Edge::Line) => Edge::Curve { c1: a.lerp(b, 1.0 / 3.0), c2: a.lerp(b, 2.0 / 3.0) },
            (true, curve) => curve,
            (false, _) => Edge::Line,
        };
    }
    pub fn set_handle(&mut self, i: usize, end: HandleEnd, to: Point2) {
        let Edge::Curve { c1, c2 } = &mut self.edges[i] else { return };
        match end {
            HandleEnd::Start => *c1 = to,
            HandleEnd::End => *c2 = to,
        }
        let (v, other_edge, other_end) = match end {
            HandleEnd::Start => (i, self.prev(i), HandleEnd::End),
            HandleEnd::End => (self.next(i), self.next(i), HandleEnd::Start),
        };
        if self.vertices[v].kind != VertexKind::Smooth {
            return;
        }
        let pivot = self.vertices[v].pos;
        let away = pivot - to;
        let len = away.length();
        if len < 1e-9 {
            return;
        }
        if let Edge::Curve { c1, c2 } = &mut self.edges[other_edge] {
            let other = match other_end {
                HandleEnd::Start => c1,
                HandleEnd::End => c2,
            };
            let keep = (*other - pivot).length();
            *other = pivot + away * (keep / len);
        }
    }
    pub fn set_vertex_kind(&mut self, i: usize, kind: VertexKind) {
        self.vertices[i].kind = kind;
        if kind == VertexKind::Smooth
            && let Edge::Curve { c1, .. } = self.edges[i]
        {
            self.set_handle(i, HandleEnd::Start, c1);
        }
    }
    pub fn remove_vertex(&mut self, i: usize) -> bool {
        let n = self.len();
        if n <= 3 {
            return false;
        }
        let prev = self.prev(i);
        let (a, b) = (self.vertices[prev].pos, self.vertices[self.next(i)].pos);
        let merged = match (self.edges[prev], self.edges[i]) {
            (Edge::Line, Edge::Line) => Edge::Line,
            (first, second) => Edge::Curve {
                c1: match first {
                    Edge::Curve { c1, .. } => c1,
                    Edge::Line => a.lerp(b, 1.0 / 3.0),
                },
                c2: match second {
                    Edge::Curve { c2, .. } => c2,
                    Edge::Line => a.lerp(b, 2.0 / 3.0),
                },
            },
        };
        self.edges[prev] = merged;
        self.vertices.remove(i);
        self.edges.remove(i);
        true
    }
    pub fn check(&self) -> Result<(), ModelError> {
        if self.vertices.len() < 3 {
            return Err(ModelError::TooFewVertices(self.id));
        }
        if self.edges.len() != self.vertices.len() {
            return Err(ModelError::EdgeCountMismatch(self.id));
        }
        let finite = self.grain_deg.is_finite()
            && self.vertices.iter().all(|v| v.pos.is_finite())
            && self.edges.iter().all(|e| match e {
                Edge::Line => true,
                Edge::Curve { c1, c2 } => c1.is_finite() && c2.is_finite(),
            });
        if finite { Ok(()) } else { Err(ModelError::NotFinite(self.id)) }
    }
```
Note on `removing_a_vertex_joins_its_edges`: removing vertex 4 from the pentagon merges edge 3 `(0,100)→(-20,50)` with edge 4 `(-20,50)→(0,0)` into one straight edge `(0,100)→(0,0)`.

`project.rs`:
```rust
    pub fn new() -> Self {
        Self { schema_version: SCHEMA_VERSION, units: Units::Cm, pieces: Vec::new(), next_piece_id: first_id() }
    }
    pub fn add_piece(&mut self, mut piece: Piece) -> PieceId {
        let id = PieceId(self.next_piece_id);
        self.next_piece_id += 1;
        piece.id = id;
        self.pieces.push(piece);
        id
    }
    pub fn piece(&self, id: PieceId) -> Option<&Piece> {
        self.pieces.iter().find(|p| p.id == id)
    }
    pub fn piece_mut(&mut self, id: PieceId) -> Option<&mut Piece> {
        self.pieces.iter_mut().find(|p| p.id == id)
    }
    pub fn remove_piece(&mut self, id: PieceId) -> Option<Piece> {
        let at = self.pieces.iter().position(|p| p.id == id)?;
        Some(self.pieces.remove(at))
    }
    pub fn next_piece_name(&self, prefix: &str) -> String {
        format!("{prefix} {}", self.next_piece_id)
    }
    pub fn check(&self) -> Result<(), ModelError> {
        let mut seen = std::collections::BTreeSet::new();
        for p in &self.pieces {
            p.check()?;
            if !seen.insert(p.id) {
                return Err(ModelError::DuplicateId(p.id));
            }
            if p.id.0 >= self.next_piece_id {
                return Err(ModelError::IdCounterBehind(p.id));
            }
        }
        Ok(())
    }
```

- [ ] **Step 5: Run the tests and see them pass**

Run: `cargo nextest run -p opendrape-core`
Expected: 14 passed.

- [ ] **Step 6: Commit**

```bash
cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings
git add Cargo.toml Cargo.lock crates/core
git commit -m "feat(core): pattern piece and project model with units

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 2: `opendrape-geom`: curve maths on pieces

**Files:**
- Create: `crates/geom/Cargo.toml`, `crates/geom/src/lib.rs`
- Modify: root `Cargo.toml` (members)

**Interfaces:**
- Consumes: `Piece`, `Edge`, `Point2`, `Vertex` (Task 1).
- Produces:
  - `edge_length(&Piece, i) -> f64`, `perimeter(&Piece) -> f64`, `area(&Piece) -> f64` (absolute, mm²).
  - `contains(&Piece, Point2) -> bool`, `point_on_edge(&Piece, i, t) -> Point2`, `nearest_edge(&Piece, Point2) -> Option<(usize, f64, f64)>` (edge, t, distance mm).
  - `outline_points(&Piece, tolerance_mm) -> Vec<Point2>` (closed, first point not repeated), `edge_points(&Piece, i, tolerance) -> Vec<Point2>`, `centroid(&Piece) -> Point2`.
  - `split_edge(&mut Piece, i, t) -> Option<usize>` (the new vertex's index; `None` if `t` is within 2% of an end).
  - `enum Anchor { Start, End }` and `set_edge_length(&mut Piece, i, length_mm, Anchor) -> bool` (false, unchanged, for a non-finite or out-of-range length or a zero-length edge).
  - `pub const MAX_EDGE_MM: f64 = 10_000.0`.

- [ ] **Step 1: Crate and failing tests**

Add `"crates/geom"` to `members`. `crates/geom/Cargo.toml`:
```toml
[package]
name = "opendrape-geom"
description = "OpenDrape pattern geometry: lengths, areas, hit-testing and edits on curves"
version.workspace = true
edition.workspace = true
rust-version.workspace = true
license.workspace = true
repository.workspace = true
publish.workspace = true

[dependencies]
opendrape-core.workspace = true
kurbo.workspace = true

[lints]
workspace = true
```

`crates/geom/src/lib.rs`:
```rust
//! Geometry on pattern pieces (lengths, areas, hit-testing, splitting and resizing edges) using
//! kurbo's exact curve maths. Everything is in millimetres.

use kurbo::{BezPath, CubicBez, Line, ParamCurve, ParamCurveArclen, ParamCurveNearest, PathEl, PathSeg, Point, Shape};
use opendrape_core::{Edge, Piece, Point2, Vertex};

/// Accuracy (mm) of curve lengths and nearest-point searches.
const ACCURACY: f64 = 1e-4;
/// Longest edge a student can type (10 m), so a mistyped number can't create absurd pieces.
pub const MAX_EDGE_MM: f64 = 10_000.0;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Anchor {
    #[default]
    Start,
    End,
}

fn kp(p: Point2) -> Point {
    Point::new(p.x, p.y)
}

fn cp(p: Point) -> Point2 {
    Point2::new(p.x, p.y)
}

fn edge_seg(piece: &Piece, i: usize) -> PathSeg {
    let (a, b) = piece.edge_ends(i);
    match piece.edges[i] {
        Edge::Line => PathSeg::Line(Line::new(kp(a), kp(b))),
        Edge::Curve { c1, c2 } => PathSeg::Cubic(CubicBez::new(kp(a), kp(c1), kp(c2), kp(b))),
    }
}

fn bez_path(piece: &Piece) -> BezPath {
    let mut path = BezPath::new();
    path.move_to(kp(piece.vertices[0].pos));
    for i in 0..piece.len() {
        let b = kp(piece.edge_ends(i).1);
        match piece.edges[i] {
            Edge::Line => path.line_to(b),
            Edge::Curve { c1, c2 } => path.curve_to(kp(c1), kp(c2), b),
        }
    }
    path.close_path();
    path
}

pub fn edge_length(_piece: &Piece, _i: usize) -> f64 {
    todo!()
}
pub fn perimeter(_piece: &Piece) -> f64 {
    todo!()
}
pub fn area(_piece: &Piece) -> f64 {
    todo!()
}
pub fn contains(_piece: &Piece, _p: Point2) -> bool {
    todo!()
}
pub fn point_on_edge(_piece: &Piece, _i: usize, _t: f64) -> Point2 {
    todo!()
}
/// The edge nearest to `p`: (edge index, curve parameter t, distance in mm).
pub fn nearest_edge(_piece: &Piece, _p: Point2) -> Option<(usize, f64, f64)> {
    todo!()
}
/// The outline as points no further than `tolerance` mm from the true curve (closed; the first
/// point is not repeated at the end).
pub fn outline_points(_piece: &Piece, _tolerance: f64) -> Vec<Point2> {
    todo!()
}
/// Edge `i` as points within `tolerance` mm, from its start to its end.
pub fn edge_points(_piece: &Piece, _i: usize, _tolerance: f64) -> Vec<Point2> {
    todo!()
}
/// Area centroid of the piece (where its name and grainline are drawn).
pub fn centroid(_piece: &Piece) -> Point2 {
    todo!()
}
/// Adds a vertex on edge `i` at curve parameter `t`; curves are split exactly. Returns the new
/// vertex's index, or `None` when `t` is within 2% of either end (that would duplicate a vertex).
pub fn split_edge(_piece: &mut Piece, _i: usize, _t: f64) -> Option<usize> {
    todo!()
}
/// Changes edge `i` to `length` mm, keeping its `anchor` end fixed. A straight edge keeps its
/// direction; a curve is scaled about the anchor, so its shape is kept. Returns false (piece
/// unchanged) for a length that is not finite, ≤ 0 or above [`MAX_EDGE_MM`], or a zero-length edge.
pub fn set_edge_length(_piece: &mut Piece, _i: usize, _length: f64, _anchor: Anchor) -> bool {
    todo!()
}

#[cfg(test)]
mod tests {
    use super::*;
    use opendrape_core::PieceId;

    fn p(x: f64, y: f64) -> Point2 {
        Point2::new(x, y)
    }

    fn square() -> Piece {
        Piece::rectangle(PieceId(1), "S", p(0.0, 0.0), 100.0, 100.0)
    }

    fn close(a: Point2, b: Point2) {
        assert!(a.distance(b) < 1e-9, "{a:?} vs {b:?}");
    }

    #[test]
    fn measures_a_square() {
        let s = square();
        assert!((edge_length(&s, 0) - 100.0).abs() < 1e-9);
        assert!((perimeter(&s) - 400.0).abs() < 1e-9);
        assert!((area(&s) - 10_000.0).abs() < 1e-6);
        assert!(contains(&s, p(50.0, 50.0)) && !contains(&s, p(150.0, 50.0)));
        assert_eq!(centroid(&s), p(50.0, 50.0));
        assert_eq!(point_on_edge(&s, 1, 0.5), p(100.0, 50.0));
    }

    #[test]
    fn measures_a_quarter_circle_curve() {
        // Edge 0 bent into a quarter circle of radius 100 (standard Bézier constant 0.5523).
        let mut s = Piece::polygon(PieceId(1), "Q", &[p(100.0, 0.0), p(0.0, 100.0), p(0.0, 0.0)]);
        let k = 0.552_284_75 * 100.0;
        s.edges[0] = Edge::Curve { c1: p(100.0, k), c2: p(k, 100.0) };
        let expected = std::f64::consts::FRAC_PI_2 * 100.0;
        assert!((edge_length(&s, 0) - expected).abs() / expected < 1e-3, "{}", edge_length(&s, 0));
    }

    #[test]
    fn finds_the_nearest_edge() {
        let (i, t, d) = nearest_edge(&square(), p(40.0, -3.0)).unwrap();
        assert_eq!(i, 0);
        assert!((t - 0.4).abs() < 1e-6 && (d - 3.0).abs() < 1e-6);
    }

    #[test]
    fn flattens_the_outline() {
        let pts = outline_points(&square(), 0.1);
        assert_eq!(pts, vec![p(0.0, 0.0), p(100.0, 0.0), p(100.0, 100.0), p(0.0, 100.0)]);
        let mut s = square();
        s.set_curved(0, true);
        s.set_handle(0, opendrape_core::HandleEnd::Start, p(30.0, -40.0));
        let edge = edge_points(&s, 0, 0.1);
        assert!(edge.len() > 3, "a curve flattens to several points");
        close(edge[0], p(0.0, 0.0));
        close(*edge.last().unwrap(), p(100.0, 0.0));
    }

    #[test]
    fn splitting_a_line_inserts_a_corner() {
        let mut s = square();
        assert_eq!(split_edge(&mut s, 0, 0.25), Some(1));
        assert_eq!(s.len(), 5);
        assert_eq!(s.vertices[1], Vertex::corner(p(25.0, 0.0)));
        assert!((perimeter(&s) - 400.0).abs() < 1e-9);
        assert_eq!(s.check(), Ok(()));
    }

    #[test]
    fn splitting_a_curve_keeps_its_shape() {
        let mut s = square();
        s.set_curved(0, true);
        s.set_handle(0, opendrape_core::HandleEnd::Start, p(30.0, -40.0));
        let before = edge_length(&s, 0);
        let mid = point_on_edge(&s, 0, 0.5);
        assert_eq!(split_edge(&mut s, 0, 0.5), Some(1));
        assert!((edge_length(&s, 0) + edge_length(&s, 1) - before).abs() < 1e-6);
        assert!(s.vertices[1].pos.distance(mid) < 1e-9);
        assert_eq!(s.vertices[1].kind, opendrape_core::VertexKind::Smooth);
    }

    #[test]
    fn refuses_to_split_at_a_vertex() {
        let mut s = square();
        assert_eq!(split_edge(&mut s, 0, 0.005), None);
        assert_eq!(split_edge(&mut s, 0, 0.999), None);
        assert_eq!(s.len(), 4);
    }

    #[test]
    fn setting_a_straight_edge_length_moves_the_free_end() {
        let mut s = square();
        assert!(set_edge_length(&mut s, 0, 150.0, Anchor::Start));
        close(s.vertices[1].pos, p(150.0, 0.0));
        let mut s = square();
        assert!(set_edge_length(&mut s, 0, 60.0, Anchor::End));
        close(s.vertices[0].pos, p(40.0, 0.0));
    }

    #[test]
    fn setting_a_curve_length_scales_it() {
        let mut s = square();
        s.set_curved(0, true);
        s.set_handle(0, opendrape_core::HandleEnd::Start, p(30.0, -40.0));
        let target = edge_length(&s, 0) * 1.5;
        assert!(set_edge_length(&mut s, 0, target, Anchor::Start));
        assert!((edge_length(&s, 0) - target).abs() < 1e-3, "lengths are measured to 0.0001 mm");
        assert_eq!(s.vertices[0].pos, p(0.0, 0.0), "anchor fixed");
    }

    #[test]
    fn set_edge_length_rejects_nonsense() {
        for bad in [0.0, -5.0, f64::NAN, f64::INFINITY, MAX_EDGE_MM + 1.0] {
            let mut s = square();
            assert!(!set_edge_length(&mut s, 0, bad, Anchor::Start), "{bad}");
            assert_eq!(s, square());
        }
    }
}
```

- [ ] **Step 2: Run the tests and watch them fail**

Run: `cargo nextest run -p opendrape-geom`
Expected: 10 FAIL, all with `not yet implemented`.

- [ ] **Step 3: Implement** (kurbo signatures verified in the probe)

```rust
pub fn edge_length(piece: &Piece, i: usize) -> f64 {
    edge_seg(piece, i).arclen(ACCURACY)
}

pub fn perimeter(piece: &Piece) -> f64 {
    (0..piece.len()).map(|i| edge_length(piece, i)).sum()
}

pub fn area(piece: &Piece) -> f64 {
    bez_path(piece).area().abs()
}

pub fn contains(piece: &Piece, p: Point2) -> bool {
    bez_path(piece).contains(kp(p))
}

pub fn point_on_edge(piece: &Piece, i: usize, t: f64) -> Point2 {
    cp(edge_seg(piece, i).eval(t))
}

pub fn nearest_edge(piece: &Piece, p: Point2) -> Option<(usize, f64, f64)> {
    (0..piece.len())
        .map(|i| {
            let n = edge_seg(piece, i).nearest(kp(p), ACCURACY);
            (i, n.t, n.distance_sq.sqrt())
        })
        .min_by(|a, b| a.2.total_cmp(&b.2))
}

fn flatten(path: &BezPath, tolerance: f64) -> Vec<Point2> {
    let mut out = Vec::new();
    kurbo::flatten(path.iter(), tolerance, |el| {
        if let PathEl::MoveTo(p) | PathEl::LineTo(p) = el {
            out.push(cp(p));
        }
    });
    out
}

pub fn outline_points(piece: &Piece, tolerance: f64) -> Vec<Point2> {
    let mut pts = flatten(&bez_path(piece), tolerance);
    if pts.len() > 1 && pts.first() == pts.last() {
        pts.pop();
    }
    pts
}

pub fn edge_points(piece: &Piece, i: usize, tolerance: f64) -> Vec<Point2> {
    let mut path = BezPath::new();
    let seg = edge_seg(piece, i);
    path.move_to(seg.start());
    match seg {
        PathSeg::Line(l) => path.line_to(l.p1),
        PathSeg::Cubic(c) => path.curve_to(c.p1, c.p2, c.p3),
        PathSeg::Quad(q) => path.quad_to(q.p1, q.p2),
    }
    flatten(&path, tolerance)
}

pub fn centroid(piece: &Piece) -> Point2 {
    let pts = outline_points(piece, 0.5);
    let (mut a, mut cx, mut cy) = (0.0, 0.0, 0.0);
    for (k, p) in pts.iter().enumerate() {
        let q = pts[(k + 1) % pts.len()];
        let cross = p.x * q.y - q.x * p.y;
        a += cross;
        cx += (p.x + q.x) * cross;
        cy += (p.y + q.y) * cross;
    }
    if a.abs() < 1e-12 {
        let n = pts.len().max(1) as f64;
        return Point2::new(pts.iter().map(|p| p.x).sum::<f64>() / n, pts.iter().map(|p| p.y).sum::<f64>() / n);
    }
    Point2::new(cx / (3.0 * a), cy / (3.0 * a))
}

pub fn split_edge(piece: &mut Piece, i: usize, t: f64) -> Option<usize> {
    if !(0.02..=0.98).contains(&t) {
        return None;
    }
    let seg = edge_seg(piece, i);
    let at = cp(seg.eval(t));
    let (first, second, vertex) = match seg {
        PathSeg::Cubic(c) => {
            let (a, b) = (c.subsegment(0.0..t), c.subsegment(t..1.0));
            (
                Edge::Curve { c1: cp(a.p1), c2: cp(a.p2) },
                Edge::Curve { c1: cp(b.p1), c2: cp(b.p2) },
                Vertex::smooth(at),
            )
        }
        _ => (Edge::Line, Edge::Line, Vertex::corner(at)),
    };
    piece.edges[i] = first;
    piece.vertices.insert(i + 1, vertex);
    piece.edges.insert(i + 1, second);
    Some(i + 1)
}

pub fn set_edge_length(piece: &mut Piece, i: usize, length: f64, anchor: Anchor) -> bool {
    if !(length.is_finite() && length > 0.0 && length <= MAX_EDGE_MM) {
        return false;
    }
    let current = edge_length(piece, i);
    if current < 1e-9 {
        return false;
    }
    let factor = length / current;
    let (a, b) = piece.edge_ends(i);
    let j = piece.next(i);
    let (pivot, moving_vertex, new_pos) = match anchor {
        Anchor::Start => (a, j, a + (b - a) * factor),
        Anchor::End => (b, i, b + (a - b) * factor),
    };
    let curve = piece.edges[i];
    piece.move_vertex(moving_vertex, new_pos);
    if let Edge::Curve { c1, c2 } = curve {
        piece.edges[i] = Edge::Curve { c1: pivot + (c1 - pivot) * factor, c2: pivot + (c2 - pivot) * factor };
    }
    true
}
```

- [ ] **Step 4: Run the tests and see them pass**

Run: `cargo nextest run -p opendrape-geom`
Expected: 10 passed. (`centroid` of the square is exact because its outline points are whole numbers; if it is not, compare with `close` and note it as a ruling.)

- [ ] **Step 5: Commit**

```bash
cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings && cargo deny check
git add Cargo.toml Cargo.lock crates/geom
git commit -m "feat(geom): pattern lengths, areas, hit-testing and edge edits with kurbo

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 3: `opendrape-io`: `.odp` project files

**Files:**
- Create: `crates/io/Cargo.toml`, `crates/io/src/lib.rs`
- Modify: root `Cargo.toml` (members)

**Interfaces:**
- Consumes: `Project`, `SCHEMA_VERSION`, `ModelError` (Task 1).
- Produces:
  - `pub const EXTENSION: &str = "odp"`;
  - `enum OdpError { Io(std::io::Error), NotAProject, Corrupt(String), NewerVersion { found: u64, supported: u32 }, Invalid(ModelError), TooLarge }` (Display + Error);
  - `to_bytes(&Project) -> Result<Vec<u8>, OdpError>` and `from_bytes(&[u8]) -> Result<Project, OdpError>`;
  - `save(&Project, &Path) -> Result<(), OdpError>` (atomic) and `load(&Path) -> Result<Project, OdpError>`.

- [ ] **Step 1: Crate and failing tests**

Add `"crates/io"` to `members`. `crates/io/Cargo.toml`:
```toml
[package]
name = "opendrape-io"
description = "OpenDrape project files (.odp)"
version.workspace = true
edition.workspace = true
rust-version.workspace = true
license.workspace = true
repository.workspace = true
publish.workspace = true

[dependencies]
opendrape-core.workspace = true
serde_json.workspace = true
zip.workspace = true

[dev-dependencies]
tempfile.workspace = true

[lints]
workspace = true
```

`crates/io/src/lib.rs`:
```rust
//! OpenDrape project files: `.odp` is a zip holding one `project.json`. Loading upgrades older
//! formats, refuses newer ones and rejects invalid data, so a bad file never replaces a
//! student's work.

use opendrape_core::{ModelError, Project, SCHEMA_VERSION};
use std::io::{Cursor, Read, Seek, Write};
use std::path::Path;
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipArchive, ZipWriter};

pub const EXTENSION: &str = "odp";
const ENTRY: &str = "project.json";
const MAX_JSON_BYTES: u64 = 64 * 1024 * 1024;

#[derive(Debug)]
pub enum OdpError {
    Io(std::io::Error),
    NotAProject,
    Corrupt(String),
    NewerVersion { found: u64, supported: u32 },
    Invalid(ModelError),
    TooLarge,
}

impl std::fmt::Display for OdpError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(e) => write!(f, "could not read or write the file: {e}"),
            Self::NotAProject => write!(f, "not an OpenDrape project file"),
            Self::Corrupt(why) => write!(f, "damaged project file ({why})"),
            Self::NewerVersion { found, supported } => {
                write!(f, "made by a newer OpenDrape (format {found}; this version reads up to {supported})")
            }
            Self::Invalid(e) => write!(f, "invalid project data ({e})"),
            Self::TooLarge => write!(f, "project file too large"),
        }
    }
}

impl std::error::Error for OdpError {}

impl From<std::io::Error> for OdpError {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e)
    }
}

pub fn to_bytes(_project: &Project) -> Result<Vec<u8>, OdpError> {
    todo!()
}
pub fn from_bytes(_bytes: &[u8]) -> Result<Project, OdpError> {
    todo!()
}
/// Saves atomically: writes `<path>.odp.tmp`, then renames it over `path`.
pub fn save(_project: &Project, _path: &Path) -> Result<(), OdpError> {
    todo!()
}
pub fn load(_path: &Path) -> Result<Project, OdpError> {
    todo!()
}

#[cfg(test)]
mod tests {
    use super::*;
    use opendrape_core::{Piece, PieceId, Point2};

    fn sample() -> Project {
        let mut p = Project::new();
        let mut front = Piece::rectangle(PieceId(0), "Front", Point2::new(0.0, 0.0), 350.0, 550.0);
        front.set_curved(2, true);
        p.add_piece(front);
        p
    }

    fn zip_with(entry: &str, json: &str) -> Vec<u8> {
        let mut zip = ZipWriter::new(Cursor::new(Vec::new()));
        zip.start_file(entry, SimpleFileOptions::default()).unwrap();
        zip.write_all(json.as_bytes()).unwrap();
        zip.finish().unwrap().into_inner()
    }

    #[test]
    fn round_trips_and_is_reproducible() {
        let bytes = to_bytes(&sample()).unwrap();
        assert_eq!(from_bytes(&bytes).unwrap(), sample());
        assert_eq!(bytes, to_bytes(&sample()).unwrap(), "same project, same bytes");
    }

    #[test]
    fn refuses_a_newer_format() {
        let json = r#"{"schema_version": 99, "pieces": []}"#;
        assert!(matches!(from_bytes(&zip_with("project.json", json)), Err(OdpError::NewerVersion { found: 99, .. })));
    }

    #[test]
    fn refuses_files_that_are_not_projects() {
        assert!(matches!(from_bytes(b"hello"), Err(OdpError::NotAProject)));
        assert!(matches!(from_bytes(&zip_with("other.txt", "{}")), Err(OdpError::NotAProject)));
        assert!(matches!(from_bytes(&zip_with("project.json", "{not json")), Err(OdpError::Corrupt(_))));
        assert!(matches!(from_bytes(&zip_with("project.json", "{}")), Err(OdpError::NotAProject)));
    }

    #[test]
    fn refuses_invalid_geometry() {
        let mut bad = sample();
        bad.pieces[0].vertices.truncate(2);
        bad.pieces[0].edges.truncate(2);
        let json = serde_json::to_string(&bad).unwrap();
        assert!(matches!(from_bytes(&zip_with("project.json", &json)), Err(OdpError::Invalid(_))));
    }

    #[test]
    fn saves_atomically_and_loads_back() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("skirt.odp");
        save(&sample(), &path).unwrap();
        assert_eq!(load(&path).unwrap(), sample());
        let names: Vec<_> = std::fs::read_dir(dir.path()).unwrap().map(|e| e.unwrap().file_name()).collect();
        assert_eq!(names, vec![std::ffi::OsString::from("skirt.odp")], "no temp file left behind");
        assert!(matches!(load(&dir.path().join("missing.odp")), Err(OdpError::Io(_))));
    }
}
```

- [ ] **Step 2: Run the tests and watch them fail**

Run: `cargo nextest run -p opendrape-io`
Expected: 5 FAIL, all with `not yet implemented`.

- [ ] **Step 3: Implement** (zip 8.6 API verified in the probe)

```rust
fn zip_err(e: zip::result::ZipError) -> OdpError {
    match e {
        zip::result::ZipError::Io(e) => OdpError::Io(e),
        other => OdpError::Corrupt(other.to_string()),
    }
}

fn write_to<W: Write + Seek>(project: &Project, w: W) -> Result<(), OdpError> {
    let mut zip = ZipWriter::new(w);
    let options = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated).unix_permissions(0o644);
    zip.start_file(ENTRY, options).map_err(zip_err)?;
    serde_json::to_writer_pretty(&mut zip, project).map_err(|e| OdpError::Corrupt(e.to_string()))?;
    zip.finish().map_err(zip_err)?;
    Ok(())
}

fn read_from<R: Read + Seek>(r: R) -> Result<Project, OdpError> {
    let mut zip = ZipArchive::new(r).map_err(|_| OdpError::NotAProject)?;
    let mut entry = zip.by_name(ENTRY).map_err(|_| OdpError::NotAProject)?;
    if entry.size() > MAX_JSON_BYTES {
        return Err(OdpError::TooLarge);
    }
    let mut text = String::new();
    entry.by_ref().take(MAX_JSON_BYTES).read_to_string(&mut text).map_err(|e| OdpError::Corrupt(e.to_string()))?;
    let value: serde_json::Value = serde_json::from_str(&text).map_err(|e| OdpError::Corrupt(e.to_string()))?;
    let value = migrate(value)?;
    let project: Project = serde_json::from_value(value).map_err(|e| OdpError::Corrupt(e.to_string()))?;
    project.check().map_err(OdpError::Invalid)?;
    Ok(project)
}

/// Upgrades older project JSON to [`SCHEMA_VERSION`] one version at a time. Version 1 is the
/// first format, so there are no steps yet: add `found = 1 => { …; found = 2 }` arms here.
fn migrate(value: serde_json::Value) -> Result<serde_json::Value, OdpError> {
    let found = value.get("schema_version").and_then(serde_json::Value::as_u64).ok_or(OdpError::NotAProject)?;
    if found > u64::from(SCHEMA_VERSION) {
        return Err(OdpError::NewerVersion { found, supported: SCHEMA_VERSION });
    }
    if found == 0 {
        return Err(OdpError::Corrupt("format version 0".into()));
    }
    Ok(value)
}

pub fn to_bytes(project: &Project) -> Result<Vec<u8>, OdpError> {
    let mut out = Cursor::new(Vec::new());
    write_to(project, &mut out)?;
    Ok(out.into_inner())
}

pub fn from_bytes(bytes: &[u8]) -> Result<Project, OdpError> {
    read_from(Cursor::new(bytes))
}

pub fn save(project: &Project, path: &Path) -> Result<(), OdpError> {
    let tmp = path.with_extension(format!("{EXTENSION}.tmp"));
    let result = (|| {
        let mut w = std::io::BufWriter::new(std::fs::File::create(&tmp)?);
        write_to(project, &mut w)?;
        w.flush()?;
        drop(w);
        std::fs::rename(&tmp, path)?;
        Ok(())
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(&tmp);
    }
    result
}

pub fn load(path: &Path) -> Result<Project, OdpError> {
    read_from(std::io::BufReader::new(std::fs::File::open(path)?))
}
```

- [ ] **Step 4: Run the tests and see them pass**

Run: `cargo nextest run -p opendrape-io`
Expected: 5 passed.

- [ ] **Step 5: Commit**

```bash
cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings && cargo deny check
git add Cargo.toml Cargo.lock crates/io
git commit -m "feat(io): .odp project files with version checks and atomic saves

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 4: Undo history (`Document`) and the pattern-table camera (`View`)

**Files:**
- Create: `crates/app/src/editor/{mod.rs,document.rs,view.rs}`
- Modify: `crates/app/Cargo.toml`, `crates/app/src/lib.rs`

**Interfaces:**
- Consumes: `Project`, `Point2` (Task 1).
- Produces:
  - `editor::Document` with:
    - `new(Project, Option<PathBuf>)`, `Default`, `project()`;
    - `edit(f) -> R`: one undo step, or none if nothing changed;
    - `begin_gesture`, `gesture_edit(f) -> R`, `end_gesture`: a whole drag is one undo step;
    - `undo`, `redo -> bool`, `can_undo`, `can_redo`;
    - `is_dirty`, `mark_saved(PathBuf)`, and the public field `path: Option<PathBuf>`.
  - `UNDO_LIMIT = 200`.
  - `editor::View { pan: Vec2, zoom: f64 }` with `to_screen`, `to_world`, `mm(px)`, `zoom_at`, `fit`.

- [ ] **Step 1: Dependencies and module**

`crates/app/Cargo.toml` `[dependencies]` gains:
```toml
opendrape-core.workspace = true
opendrape-geom.workspace = true
opendrape-io.workspace = true
earcut.workspace = true
```
`crates/app/src/lib.rs`: add `pub mod editor;` (after `pub mod diagnostics;`).

`crates/app/src/editor/mod.rs`:
```rust
//! The 2D pattern window: drawing and editing pattern pieces.

mod document;
mod view;

pub use document::{Document, UNDO_LIMIT};
pub use view::View;
```

- [ ] **Step 2: Failing tests with `todo!()` bodies**

`crates/app/src/editor/document.rs`:
```rust
//! The open project and its undo history.

use opendrape_core::Project;
use std::path::PathBuf;

/// Undo steps kept; older ones are dropped.
pub const UNDO_LIMIT: usize = 200;

/// The project being edited. Undo keeps whole snapshots of the project (a pattern is small), so
/// there is no reverse-edit code to get wrong.
pub struct Document {
    project: Project,
    undo: Vec<Project>,
    redo: Vec<Project>,
    /// The project as it was when the current drag began.
    gesture: Option<Project>,
    /// The project as last saved or opened, to tell whether there are unsaved changes.
    saved: Project,
    /// Where the project was last saved or opened from.
    pub path: Option<PathBuf>,
}

impl Default for Document {
    fn default() -> Self {
        Self::new(Project::new(), None)
    }
}

impl Document {
    pub fn new(project: Project, path: Option<PathBuf>) -> Self {
        Self { saved: project.clone(), project, undo: Vec::new(), redo: Vec::new(), gesture: None, path }
    }
    pub fn project(&self) -> &Project {
        &self.project
    }
    /// Changes the project as one undo step. A change that leaves the project as it was adds
    /// no step.
    pub fn edit<R>(&mut self, _f: impl FnOnce(&mut Project) -> R) -> R {
        todo!()
    }
    /// Starts a drag: everything changed with [`Self::gesture_edit`] until [`Self::end_gesture`]
    /// is one undo step.
    pub fn begin_gesture(&mut self) {
        todo!()
    }
    /// Changes the project as part of the current drag (starting one if needed).
    pub fn gesture_edit<R>(&mut self, _f: impl FnOnce(&mut Project) -> R) -> R {
        todo!()
    }
    pub fn end_gesture(&mut self) {
        todo!()
    }
    pub fn undo(&mut self) -> bool {
        todo!()
    }
    pub fn redo(&mut self) -> bool {
        todo!()
    }
    pub fn can_undo(&self) -> bool {
        todo!()
    }
    pub fn can_redo(&self) -> bool {
        todo!()
    }
    /// The project differs from the version last saved or opened.
    pub fn is_dirty(&self) -> bool {
        todo!()
    }
    pub fn mark_saved(&mut self, _path: PathBuf) {
        todo!()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use opendrape_core::{Piece, PieceId, Point2};

    fn rect() -> Piece {
        Piece::rectangle(PieceId(0), "R", Point2::new(0.0, 0.0), 100.0, 50.0)
    }

    #[test]
    fn edits_undo_and_redo() {
        let mut doc = Document::default();
        assert!(!doc.can_undo() && !doc.can_redo());
        let id = doc.edit(|p| p.add_piece(rect()));
        assert_eq!(doc.project().pieces.len(), 1);
        assert!(doc.undo());
        assert!(doc.project().pieces.is_empty());
        assert!(!doc.undo(), "nothing left to undo");
        assert!(doc.redo());
        assert!(doc.project().piece(id).is_some());
        assert!(!doc.redo());
    }

    #[test]
    fn a_change_that_changes_nothing_adds_no_step() {
        let mut doc = Document::default();
        doc.edit(|p| p.remove_piece(PieceId(7)));
        assert!(!doc.can_undo());
    }

    #[test]
    fn a_new_edit_clears_redo() {
        let mut doc = Document::default();
        doc.edit(|p| p.add_piece(rect()));
        doc.undo();
        doc.edit(|p| p.add_piece(rect()));
        assert!(!doc.can_redo());
    }

    #[test]
    fn a_whole_drag_is_one_step() {
        let mut doc = Document::default();
        let id = doc.edit(|p| p.add_piece(rect()));
        doc.begin_gesture();
        for k in 1..=10 {
            doc.gesture_edit(|p| p.piece_mut(id).unwrap().move_vertex(0, Point2::new(-f64::from(k), 0.0)));
        }
        doc.end_gesture();
        assert_eq!(doc.project().piece(id).unwrap().vertices[0].pos, Point2::new(-10.0, 0.0));
        assert!(doc.undo());
        assert_eq!(doc.project().piece(id).unwrap().vertices[0].pos, Point2::new(0.0, 0.0));
        assert!(doc.undo(), "then the piece itself");
        assert!(!doc.can_undo());
    }

    #[test]
    fn a_drag_that_moved_nothing_adds_no_step() {
        let mut doc = Document::default();
        doc.begin_gesture();
        doc.end_gesture();
        assert!(!doc.can_undo());
    }

    #[test]
    fn history_keeps_the_last_200_steps() {
        let mut doc = Document::default();
        for _ in 0..UNDO_LIMIT + 50 {
            doc.edit(|p| p.add_piece(rect()));
        }
        let mut undone = 0;
        while doc.undo() {
            undone += 1;
        }
        assert_eq!(undone, UNDO_LIMIT);
        assert_eq!(doc.project().pieces.len(), 50);
    }

    #[test]
    fn dirty_follows_the_saved_version() {
        let mut doc = Document::default();
        assert!(!doc.is_dirty());
        doc.edit(|p| p.add_piece(rect()));
        assert!(doc.is_dirty());
        doc.undo();
        assert!(!doc.is_dirty(), "undone back to the saved version");
        doc.redo();
        doc.mark_saved(PathBuf::from("skirt.odp"));
        assert!(!doc.is_dirty());
        assert_eq!(doc.path, Some(PathBuf::from("skirt.odp")));
        doc.undo();
        assert!(doc.is_dirty());
    }
}
```

`crates/app/src/editor/view.rs`:
```rust
//! The pattern table's camera: millimetres with y up ↔ screen points with y down.

use egui::{Pos2, Rect, Vec2, vec2};
use opendrape_core::Point2;

/// Zoom limits in screen points per millimetre (a whole room to a few millimetres).
pub const MIN_ZOOM: f64 = 0.05;
pub const MAX_ZOOM: f64 = 50.0;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct View {
    /// Screen offset from the canvas's top-left corner to the pattern origin.
    pub pan: Vec2,
    /// Screen points per millimetre.
    pub zoom: f64,
}

impl Default for View {
    fn default() -> Self {
        Self { pan: vec2(40.0, 400.0), zoom: 1.0 }
    }
}

impl View {
    pub fn to_screen(&self, _rect: Rect, _p: Point2) -> Pos2 {
        todo!()
    }
    pub fn to_world(&self, _rect: Rect, _s: Pos2) -> Point2 {
        todo!()
    }
    /// Millimetres covered by `px` screen points at this zoom.
    pub fn mm(&self, _px: f64) -> f64 {
        todo!()
    }
    /// Zooms by `factor`, keeping the pattern point under `cursor` where it is.
    pub fn zoom_at(&mut self, _rect: Rect, _cursor: Pos2, _factor: f64) {
        todo!()
    }
    /// Shows the box `min`..`max` (mm) centred in `rect`, with a margin.
    pub fn fit(&mut self, _rect: Rect, _min: Point2, _max: Point2) {
        todo!()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rect() -> Rect {
        Rect::from_min_size(egui::pos2(50.0, 30.0), vec2(800.0, 600.0))
    }

    #[test]
    fn round_trips_with_y_up() {
        let v = View { pan: vec2(10.0, 500.0), zoom: 2.0 };
        let p = Point2::new(123.4, -56.7);
        let back = v.to_world(rect(), v.to_screen(rect(), p));
        assert!(back.distance(p) < 1e-3, "{back:?}");
        let low = v.to_screen(rect(), Point2::new(0.0, 0.0));
        let high = v.to_screen(rect(), Point2::new(0.0, 10.0));
        assert!(high.y < low.y, "larger y is higher on screen");
        assert_eq!(v.mm(8.0), 4.0);
    }

    #[test]
    fn zoom_keeps_the_point_under_the_cursor() {
        let mut v = View::default();
        let cursor = egui::pos2(321.0, 222.0);
        let before = v.to_world(rect(), cursor);
        v.zoom_at(rect(), cursor, 1.5);
        assert!((v.zoom - 1.5).abs() < 1e-12);
        assert!(v.to_world(rect(), cursor).distance(before) < 1e-3);
        v.zoom_at(rect(), cursor, 1e9);
        assert_eq!(v.zoom, MAX_ZOOM);
        v.zoom_at(rect(), cursor, 1e-12);
        assert_eq!(v.zoom, MIN_ZOOM);
    }

    #[test]
    fn fit_centres_the_box_inside_the_canvas() {
        let mut v = View::default();
        let (min, max) = (Point2::new(1000.0, 2000.0), Point2::new(1600.0, 2800.0));
        v.fit(rect(), min, max);
        let centre = v.to_screen(rect(), min.lerp(max, 0.5));
        assert!((centre - rect().center()).length() < 1e-3);
        assert!(rect().contains(v.to_screen(rect(), min)) && rect().contains(v.to_screen(rect(), max)));
    }
}
```

- [ ] **Step 3: Run and watch them fail**

Run: `cargo nextest run -p opendrape --lib editor`
Expected: 10 FAIL (`not yet implemented`).

- [ ] **Step 4: Implement**

`document.rs`:
```rust
    pub fn edit<R>(&mut self, f: impl FnOnce(&mut Project) -> R) -> R {
        self.end_gesture();
        let before = self.project.clone();
        let result = f(&mut self.project);
        if self.project != before {
            self.push_undo(before);
        }
        result
    }
    pub fn begin_gesture(&mut self) {
        if self.gesture.is_none() {
            self.gesture = Some(self.project.clone());
        }
    }
    pub fn gesture_edit<R>(&mut self, f: impl FnOnce(&mut Project) -> R) -> R {
        self.begin_gesture();
        f(&mut self.project)
    }
    pub fn end_gesture(&mut self) {
        if let Some(before) = self.gesture.take()
            && before != self.project
        {
            self.push_undo(before);
        }
    }
    fn push_undo(&mut self, before: Project) {
        self.undo.push(before);
        if self.undo.len() > UNDO_LIMIT {
            self.undo.remove(0);
        }
        self.redo.clear();
    }
    pub fn undo(&mut self) -> bool {
        self.end_gesture();
        let Some(previous) = self.undo.pop() else { return false };
        self.redo.push(std::mem::replace(&mut self.project, previous));
        true
    }
    pub fn redo(&mut self) -> bool {
        self.end_gesture();
        let Some(next) = self.redo.pop() else { return false };
        self.undo.push(std::mem::replace(&mut self.project, next));
        true
    }
    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }
    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }
    pub fn is_dirty(&self) -> bool {
        self.project != self.saved
    }
    pub fn mark_saved(&mut self, path: PathBuf) {
        self.saved = self.project.clone();
        self.path = Some(path);
    }
```

`view.rs`:
```rust
    pub fn to_screen(&self, rect: Rect, p: Point2) -> Pos2 {
        rect.min + self.pan + vec2((p.x * self.zoom) as f32, (-p.y * self.zoom) as f32)
    }
    pub fn to_world(&self, rect: Rect, s: Pos2) -> Point2 {
        let v = s - rect.min - self.pan;
        Point2::new(f64::from(v.x) / self.zoom, -f64::from(v.y) / self.zoom)
    }
    pub fn mm(&self, px: f64) -> f64 {
        px / self.zoom
    }
    pub fn zoom_at(&mut self, rect: Rect, cursor: Pos2, factor: f64) {
        let anchor = self.to_world(rect, cursor);
        self.zoom = (self.zoom * factor).clamp(MIN_ZOOM, MAX_ZOOM);
        self.pan += cursor - self.to_screen(rect, anchor);
    }
    pub fn fit(&mut self, rect: Rect, min: Point2, max: Point2) {
        let size = max - min;
        let (w, h) = (size.x.max(10.0), size.y.max(10.0));
        let zoom = (f64::from(rect.width()) * 0.85 / w).min(f64::from(rect.height()) * 0.85 / h);
        self.zoom = zoom.clamp(MIN_ZOOM, MAX_ZOOM);
        let c = min.lerp(max, 0.5);
        self.pan = rect.size() * 0.5 - vec2((c.x * self.zoom) as f32, (-c.y * self.zoom) as f32);
    }
```

- [ ] **Step 5: Run and see them pass**

Run: `cargo nextest run -p opendrape --lib editor`
Expected: 10 passed.

- [ ] **Step 6: Commit**

```bash
cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings
git add crates/app Cargo.lock
git commit -m "feat(editor): undo history with drag gestures, and the pattern-table view

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 5: The pattern window, and drawing with the pen and rectangle tools

**Files:**
- Modify: `crates/app/src/editor/mod.rs`, `crates/app/i18n/en-US/opendrape.ftl`
- Create:
  - `crates/app/src/editor/{canvas.rs,length_box.rs,paint.rs}`
  - `crates/app/tests/editor.rs`

**Interfaces:**
- Consumes: Tasks 1, 2 and 4.
- Produces:
  - `editor::{PatternEditor, Tool, Selection, PenPoint, UNDO, REDO}`.
  - `PatternEditor` public fields: `doc`, `view`, `tool`, `selection`, `show_lengths`, `canvas_rect`, `notice`.
  - `PatternEditor` methods: `new`, `ui(&mut Ui)`, `set_project(Project, Option<PathBuf>)`, `set_tool`, `undo`, `redo`, `can_undo`, `can_redo`, `fit`, `pen() -> &[PenPoint]`, `length_box_open`.
  - Task 6 fills the Edit and Add point tools into the placeholder match arm left here. Task 7 adds the properties panel and status bar.

**Pen behaviour:**
- **Click:** places a corner.
- **Press and drag:** places a curve point. The drag pulls out its handle, and the arriving edge's handle is mirrored.
- **Finish:**
  - With ≥ 3 points, clicking the first point (or the last point again, i.e. a double-click) closes the piece. Enter does too.
  - A closed piece is one undo step and becomes the selection.
- **Undo while drawing:** Backspace or Cmd/Ctrl+Z removes the last point. Esc cancels the piece.
- **Shift:** keeps the next point at 45° steps.
- **Snapping:** points snap to existing points within 8 screen points.
- **Same spot twice:** clicking it adds nothing. Double-clicks never create zero-length edges.
- **Typed numbers:** a digit typed while drawing opens the number box.
  - Length is in the current units; Angle is in degrees and is prefilled with the direction to the pointer.
  - Tab switches fields, Enter commits, and Esc or clicking elsewhere cancels.
  - A length that is not a number, ≤ 0 or > 10 m is refused and a notice is shown.

**Rectangle behaviour (S):**
- Drag from corner to corner. A drag with no height or width makes nothing.
- Or click once and type Width, Tab, Height, Enter.
- Corners snap to existing points.

- [ ] **Step 1: Strings**

Append to `crates/app/i18n/en-US/opendrape.ftl`:
```
tool-edit = Edit
tool-pen = Pen
tool-rectangle = Rectangle
tool-add-point = Add point
tool-edit-tip = Select and move points, curve handles, edges and whole pieces.
tool-pen-tip = Draw a piece point by point. Drag while placing a point to make a curve.
tool-rectangle-tip = Draw a rectangular piece.
tool-add-point-tip = Add a point on an edge.
units-cm = cm
units-inch = inch
toolbar-show-lengths = Show lengths
toolbar-fit = Fit (F)
toolbar-fit-tip = Show all pieces.
box-length = Length
box-angle = Angle
box-width = Width
box-height = Height
piece-default-name = Piece
notice-need-three-points = A piece needs at least 3 points.
notice-bad-number = Please type a valid number. Lengths must be more than 0 and at most 10 m.
notice-too-close = Too close to an existing point.
notice-min-points = A piece needs at least 3 points, so this point can't be deleted.
```

- [ ] **Step 2: Failing UI tests**

`crates/app/tests/editor.rs`:
```rust
//! The pattern window, driven the way a student uses it: clicks, drags and typing.
//! Positions are given in pattern millimetres and turned into screen points with the
//! editor's own view, so the tests don't depend on the window layout.

use egui::{Event, Key, Modifiers, PointerButton, Pos2, accesskit::Role, vec2};
use egui_kittest::{Harness, kittest::Queryable};
use opendrape::editor::{PatternEditor, Selection, Tool};
use opendrape_core::{Edge, Piece, PieceId, Point2, VertexKind};

type H = Harness<'static, PatternEditor>;

fn harness() -> H {
    let mut h = Harness::builder()
        .with_size(vec2(1100.0, 750.0))
        .build_ui_state(|ui, ed: &mut PatternEditor| ed.ui(ui), PatternEditor::new());
    h.run();
    h
}

/// Screen position of the pattern point (x, y) mm.
fn at(h: &H, x: f64, y: f64) -> Pos2 {
    let ed = h.state();
    ed.view.to_screen(ed.canvas_rect, Point2::new(x, y))
}

fn button(h: &H, pos: Pos2, pressed: bool, modifiers: Modifiers) {
    h.event(Event::PointerButton { pos, button: PointerButton::Primary, pressed, modifiers });
}

fn click(h: &mut H, x: f64, y: f64) {
    let p = at(h, x, y);
    h.hover_at(p);
    button(h, p, true, Modifiers::NONE);
    button(h, p, false, Modifiers::NONE);
    h.run();
}

fn shift_click(h: &mut H, x: f64, y: f64) {
    let p = at(h, x, y);
    h.hover_at(p);
    h.event(Event::ModifiersChanged(Modifiers::SHIFT));
    button(h, p, true, Modifiers::SHIFT);
    button(h, p, false, Modifiers::SHIFT);
    h.event(Event::ModifiersChanged(Modifiers::NONE));
    h.run();
}

/// Press at `from`, move there in steps, release at `to` (all in mm).
fn drag(h: &mut H, from: (f64, f64), to: (f64, f64)) {
    let (a, b) = (at(h, from.0, from.1), at(h, to.0, to.1));
    h.hover_at(a);
    button(h, a, true, Modifiers::NONE);
    for i in 1..=5 {
        h.hover_at(a + (b - a) * (i as f32 / 5.0));
    }
    button(h, b, false, Modifiers::NONE);
    h.run();
}

fn key(h: &mut H, k: Key) {
    h.key_press(k);
    h.run();
}

fn cmd(h: &mut H, k: Key) {
    h.key_press_modifiers(Modifiers::COMMAND, k);
    h.run();
}

/// Within 0.05 mm (clicks pass through f32 screen coordinates).
fn close(a: Point2, b: Point2) {
    assert!(a.distance(b) < 0.05, "{a:?} vs {b:?}");
}

/// Types `first` over the canvas, which opens the number box.
fn type_number(h: &mut H, first: &str) {
    h.event(Event::Text(first.into()));
    h.run();
}

/// Adds a 300 × 400 mm rectangle with its lower-left corner at (100, 100), as if drawn.
fn with_rectangle(h: &mut H) -> PieceId {
    let id = h.state_mut().doc.edit(|p| {
        p.add_piece(Piece::rectangle(PieceId(0), "Front", Point2::new(100.0, 100.0), 300.0, 400.0))
    });
    h.run();
    id
}

const SQUARE: [(f64, f64); 4] = [(100.0, 100.0), (400.0, 100.0), (400.0, 500.0), (100.0, 500.0)];

#[test]
fn pen_draws_a_closed_piece() {
    let mut h = harness();
    key(&mut h, Key::H);
    assert_eq!(h.state().tool, Tool::Pen);
    for (x, y) in SQUARE {
        click(&mut h, x, y);
    }
    assert_eq!(h.state().pen().len(), 4);
    click(&mut h, 100.0, 100.0); // back on the first point closes the piece
    let ed = h.state();
    assert!(ed.pen().is_empty());
    let [piece] = &ed.doc.project().pieces[..] else { panic!("one piece") };
    assert_eq!(piece.name, "Piece 1");
    for (v, (x, y)) in piece.vertices.iter().zip(SQUARE) {
        close(v.pos, Point2::new(x, y));
    }
    assert!(piece.edges.iter().all(|e| *e == Edge::Line));
    assert_eq!(ed.selection, Selection::Piece(piece.id));
    cmd(&mut h, Key::Z);
    assert!(h.state().doc.project().pieces.is_empty(), "the whole piece is one undo step");
}

#[test]
fn enter_finishes_but_needs_three_points() {
    let mut h = harness();
    key(&mut h, Key::H);
    click(&mut h, 100.0, 100.0);
    click(&mut h, 400.0, 100.0);
    key(&mut h, Key::Enter);
    assert_eq!(h.state().pen().len(), 2);
    assert!(h.state().notice.is_some());
    click(&mut h, 250.0, 400.0);
    key(&mut h, Key::Enter);
    assert_eq!(h.state().doc.project().pieces.len(), 1);
}

#[test]
fn dragging_while_placing_makes_a_curve_point() {
    let mut h = harness();
    key(&mut h, Key::H);
    click(&mut h, 100.0, 100.0);
    drag(&mut h, (400.0, 100.0), (450.0, 150.0));
    let pen = h.state().pen().to_vec();
    assert_eq!(pen.len(), 2, "a drag places one point");
    close(pen[1].handle.expect("handle pulled out"), Point2::new(450.0, 150.0));
    click(&mut h, 400.0, 500.0);
    key(&mut h, Key::Enter);
    let piece = h.state().doc.project().pieces[0].clone();
    assert_eq!(piece.vertices[1].kind, VertexKind::Smooth);
    let Edge::Curve { c2, .. } = piece.edges[0] else { panic!("arriving edge curved") };
    close(c2, Point2::new(350.0, 50.0)); // mirrored through the point
    let Edge::Curve { c1, .. } = piece.edges[1] else { panic!("leaving edge curved") };
    close(c1, Point2::new(450.0, 150.0));
    assert_eq!(piece.edges[2], Edge::Line);
}

#[test]
fn typed_length_goes_toward_the_pointer() {
    let mut h = harness();
    key(&mut h, Key::H);
    click(&mut h, 100.0, 100.0);
    let p = at(&h, 300.0, 100.0);
    h.hover_at(p);
    h.run();
    type_number(&mut h, "3");
    assert!(h.state().length_box_open());
    let field = h.get_by_role_and_label(Role::TextInput, "Length");
    assert!(field.is_focused());
    field.type_text("4.5");
    h.run();
    assert_eq!(h.get_by_role_and_label(Role::TextInput, "Length").value().as_deref(), Some("34.5"));
    assert_eq!(h.get_by_role_and_label(Role::TextInput, "Angle").value().as_deref(), Some("0.0"));
    key(&mut h, Key::Enter);
    let ed = h.state();
    assert!(!ed.length_box_open());
    let [a, b] = ed.pen() else { panic!("two points") };
    assert!((b.pos - a.pos).distance(Point2::new(345.0, 0.0)) < 1e-9, "34.5 cm to the right");
    assert!(ed.doc.project().pieces.is_empty(), "Enter in the box must not also finish the piece");
}

#[test]
fn typed_angle_sets_the_direction() {
    let mut h = harness();
    key(&mut h, Key::H);
    click(&mut h, 100.0, 100.0);
    type_number(&mut h, "20");
    key(&mut h, Key::Tab);
    assert!(h.state().length_box_open(), "Tab moves to Angle without closing the box");
    assert!(h.get_by_role_and_label(Role::TextInput, "Angle").is_focused());
    cmd(&mut h, Key::A);
    h.get_by_role_and_label(Role::TextInput, "Angle").type_text("90");
    h.run();
    key(&mut h, Key::Enter);
    let [a, b] = h.state().pen() else { panic!("two points") };
    assert!((b.pos - a.pos).distance(Point2::new(0.0, 200.0)) < 1e-9, "20 cm straight up");
}

#[test]
fn typed_length_rejects_nonsense() {
    for (first, rest) in [("3", "abc"), ("0", ""), ("1", "e9"), ("5", ",,")] {
        let mut h = harness();
        key(&mut h, Key::H);
        click(&mut h, 100.0, 100.0);
        type_number(&mut h, first);
        if !rest.is_empty() {
            h.get_by_role_and_label(Role::TextInput, "Length").type_text(rest);
            h.run();
        }
        key(&mut h, Key::Enter);
        assert_eq!(h.state().pen().len(), 1, "{first}{rest}");
        assert!(h.state().notice.is_some(), "{first}{rest}");
        assert!(!h.state().length_box_open());
    }
}

#[test]
fn escape_closes_the_box_then_cancels_the_piece() {
    let mut h = harness();
    key(&mut h, Key::H);
    click(&mut h, 100.0, 100.0);
    click(&mut h, 400.0, 100.0);
    type_number(&mut h, "12");
    key(&mut h, Key::Escape);
    assert!(!h.state().length_box_open());
    assert_eq!(h.state().pen().len(), 2);
    key(&mut h, Key::Escape);
    assert!(h.state().pen().is_empty());
    assert!(h.state().doc.project().pieces.is_empty());
}

#[test]
fn undo_while_drawing_removes_the_last_point() {
    let mut h = harness();
    key(&mut h, Key::H);
    for (x, y) in &SQUARE[..3] {
        click(&mut h, *x, *y);
    }
    cmd(&mut h, Key::Z);
    assert_eq!(h.state().pen().len(), 2);
    key(&mut h, Key::Backspace);
    assert_eq!(h.state().pen().len(), 1);
    assert!(!h.state().doc.can_undo(), "the project history is untouched");
}

#[test]
fn clicking_the_same_spot_twice_adds_one_point() {
    let mut h = harness();
    key(&mut h, Key::H);
    click(&mut h, 100.0, 100.0);
    click(&mut h, 100.0, 100.0);
    assert_eq!(h.state().pen().len(), 1);
}

#[test]
fn shift_keeps_45_degree_steps() {
    let mut h = harness();
    key(&mut h, Key::H);
    click(&mut h, 100.0, 100.0);
    shift_click(&mut h, 400.0, 120.0);
    let [a, b] = h.state().pen() else { panic!("two points") };
    assert!((b.pos.y - a.pos.y).abs() < 1e-9, "snapped horizontal: {a:?} {b:?}");
}

#[test]
fn snaps_to_existing_points() {
    let mut h = harness();
    with_rectangle(&mut h);
    key(&mut h, Key::H);
    click(&mut h, 403.0, 98.0);
    assert_eq!(h.state().pen()[0].pos, Point2::new(400.0, 100.0));
}

#[test]
fn pinch_zooms_around_the_pointer() {
    let mut h = harness();
    let p = at(&h, 250.0, 250.0);
    h.hover_at(p);
    h.run();
    let zoom = h.state().view.zoom;
    h.event(Event::Zoom(1.5));
    h.run();
    let ed = h.state();
    assert!((ed.view.zoom - zoom * 1.5).abs() < 1e-6);
    close(ed.view.to_world(ed.canvas_rect, p), Point2::new(250.0, 250.0));
}

#[test]
fn rectangle_by_dragging() {
    let mut h = harness();
    key(&mut h, Key::S);
    drag(&mut h, (100.0, 100.0), (400.0, 500.0));
    let ed = h.state();
    let [piece] = &ed.doc.project().pieces[..] else { panic!("one piece") };
    for (v, (x, y)) in piece.vertices.iter().zip(SQUARE) {
        close(v.pos, Point2::new(x, y));
    }
    assert_eq!(ed.selection, Selection::Piece(piece.id));
}

#[test]
fn rectangle_by_typing_its_size() {
    let mut h = harness();
    key(&mut h, Key::S);
    click(&mut h, 100.0, 100.0);
    assert!(h.state().length_box_open());
    let width = h.get_by_role_and_label(Role::TextInput, "Width");
    assert!(width.is_focused());
    width.type_text("35");
    h.run();
    key(&mut h, Key::Tab);
    h.get_by_role_and_label(Role::TextInput, "Height").type_text("60,5");
    h.run();
    key(&mut h, Key::Enter);
    let piece = h.state().doc.project().pieces[0].clone();
    close(piece.vertices[0].pos, Point2::new(100.0, 100.0));
    assert!((piece.vertices[2].pos - piece.vertices[0].pos).distance(Point2::new(350.0, 605.0)) < 1e-9);
}

#[test]
fn a_flat_or_zero_size_rectangle_is_refused() {
    let mut h = harness();
    key(&mut h, Key::S);
    drag(&mut h, (100.0, 100.0), (400.0, 100.0));
    assert!(h.state().doc.project().pieces.is_empty());
    click(&mut h, 100.0, 100.0);
    h.get_by_role_and_label(Role::TextInput, "Width").type_text("0");
    h.run();
    key(&mut h, Key::Enter);
    assert!(h.state().doc.project().pieces.is_empty());
    assert!(h.state().notice.is_some());
}

#[test]
fn tool_keys_do_nothing_while_typing_a_number() {
    let mut h = harness();
    key(&mut h, Key::H);
    click(&mut h, 100.0, 100.0);
    type_number(&mut h, "3");
    h.key_press(Key::S);
    h.event(Event::Text("s".into()));
    h.run();
    assert_eq!(h.state().tool, Tool::Pen);
    assert!(h.state().length_box_open());
}

#[test]
fn draws_concave_and_curved_pieces() {
    let mut h = harness();
    let id = h.state_mut().doc.edit(|p| {
        let l = [(0.0, 0.0), (300.0, 0.0), (300.0, 100.0), (100.0, 100.0), (100.0, 300.0), (0.0, 300.0)];
        let mut piece = Piece::polygon(PieceId(0), "L", &l.map(|(x, y)| Point2::new(x, y)));
        piece.set_curved(1, true);
        p.add_piece(piece)
    });
    h.state_mut().selection = Selection::Edge(id, 1);
    h.run();
    h.state_mut().selection = Selection::Vertex(id, 3);
    h.run();
}
```

Unit tests at the bottom of `canvas.rs` (written in Step 4 with the code):
- `pen_piece_mirrors_curve_handles`
- `constrain_45_snaps_directions`
- `angle_text_is_tidy`

Run: `cargo nextest run -p opendrape --test editor`
Expected: compile error (`editor::PatternEditor` does not exist).

- [ ] **Step 3: `PatternEditor`, tools and keys** (`crates/app/src/editor/mod.rs`, replacing Task 4's version)

```rust
//! The 2D pattern window: drawing and editing pattern pieces.

mod canvas;
mod document;
mod length_box;
mod paint;
mod view;

pub use canvas::PenPoint;
pub use document::{Document, UNDO_LIMIT};
pub use view::View;

use crate::tr;
use egui::{Key, KeyboardShortcut, Modifiers};
use opendrape_core::{PieceId, Point2, Project, Units};
use std::path::PathBuf;

/// Undo: Cmd+Z (Ctrl+Z on Windows).
pub const UNDO: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::Z);
/// Redo: Shift+Cmd+Z (Shift+Ctrl+Z on Windows).
pub const REDO: KeyboardShortcut =
    KeyboardShortcut::new(Modifiers { shift: true, ..Modifiers::COMMAND }, Key::Z);
/// Redo the Windows way: Ctrl+Y.
const REDO_Y: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::Y);

/// Pointer distance (screen points) that counts as touching a point, handle or edge.
const HIT_PX: f64 = 8.0;
/// What an empty pattern table shows: 80 × 60 cm.
const EMPTY_TABLE: (Point2, Point2) = (Point2::new(0.0, 0.0), Point2::new(800.0, 600.0));

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Tool {
    #[default]
    Edit,
    Pen,
    Rectangle,
    AddPoint,
}

impl Tool {
    pub const ALL: [Self; 4] = [Self::Edit, Self::Pen, Self::Rectangle, Self::AddPoint];

    /// Single-key shortcut: the letters other pattern software uses, so habits carry over.
    pub fn key(self) -> Key {
        match self {
            Self::Edit => Key::Z,
            Self::Pen => Key::H,
            Self::Rectangle => Key::S,
            Self::AddPoint => Key::X,
        }
    }

    fn label(self) -> String {
        match self {
            Self::Edit => tr!("tool-edit"),
            Self::Pen => tr!("tool-pen"),
            Self::Rectangle => tr!("tool-rectangle"),
            Self::AddPoint => tr!("tool-add-point"),
        }
    }

    fn tip(self) -> String {
        match self {
            Self::Edit => tr!("tool-edit-tip"),
            Self::Pen => tr!("tool-pen-tip"),
            Self::Rectangle => tr!("tool-rectangle-tip"),
            Self::AddPoint => tr!("tool-add-point-tip"),
        }
    }
}

/// What the properties panel shows and Delete removes.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Selection {
    #[default]
    None,
    Piece(PieceId),
    Vertex(PieceId, usize),
    Edge(PieceId, usize),
}

impl Selection {
    pub fn piece(self) -> Option<PieceId> {
        match self {
            Self::None => None,
            Self::Piece(id) | Self::Vertex(id, _) | Self::Edge(id, _) => Some(id),
        }
    }

    /// This selection if it still exists in `project` (after an undo, say); otherwise its
    /// piece, or nothing.
    pub fn validated(self, project: &Project) -> Self {
        let Some(piece) = self.piece().and_then(|id| project.piece(id)) else { return Self::None };
        match self {
            Self::Vertex(_, i) | Self::Edge(_, i) if i >= piece.len() => Self::Piece(piece.id),
            other => other,
        }
    }
}

pub struct PatternEditor {
    pub doc: Document,
    pub view: View,
    pub tool: Tool,
    pub selection: Selection,
    /// Show every edge's length on the pattern.
    pub show_lengths: bool,
    /// Where the canvas was last drawn (tests use it to turn millimetres into screen points).
    pub canvas_rect: egui::Rect,
    /// Why the last action was refused, shown in the status bar until the next click.
    pub notice: Option<String>,
    canvas: canvas::CanvasState,
    fit_pending: bool,
}

impl Default for PatternEditor {
    fn default() -> Self {
        Self::new()
    }
}

impl PatternEditor {
    pub fn new() -> Self {
        Self {
            doc: Document::default(),
            view: View::default(),
            tool: Tool::default(),
            selection: Selection::None,
            show_lengths: true,
            canvas_rect: egui::Rect::NOTHING,
            notice: None,
            canvas: canvas::CanvasState::default(),
            fit_pending: true,
        }
    }

    /// Starts over with `project` (File → New or Open): clears the history and fits the view.
    pub fn set_project(&mut self, project: Project, path: Option<PathBuf>) {
        let show_lengths = self.show_lengths;
        *self = Self { show_lengths, ..Self::new() };
        self.doc = Document::new(project, path);
    }

    pub fn set_tool(&mut self, tool: Tool) {
        if tool == self.tool {
            return;
        }
        self.canvas = canvas::CanvasState::default();
        self.doc.end_gesture();
        self.notice = None;
        self.tool = tool;
    }

    /// Undo. While a piece is being drawn with the pen, removes its last point instead.
    pub fn undo(&mut self) {
        if self.canvas.pen.pop().is_none() {
            self.doc.undo();
        }
        self.selection = self.selection.validated(self.doc.project());
    }

    pub fn redo(&mut self) {
        if self.canvas.pen.is_empty() {
            self.doc.redo();
        }
        self.selection = self.selection.validated(self.doc.project());
    }

    pub fn can_undo(&self) -> bool {
        !self.canvas.pen.is_empty() || self.doc.can_undo()
    }

    pub fn can_redo(&self) -> bool {
        self.canvas.pen.is_empty() && self.doc.can_redo()
    }

    /// Show every piece on the next frame.
    pub fn fit(&mut self) {
        self.fit_pending = true;
    }

    /// Points placed so far in the piece being drawn with the pen.
    pub fn pen(&self) -> &[PenPoint] {
        &self.canvas.pen
    }

    /// The number box (typed length and angle, or width and height) is open.
    pub fn length_box_open(&self) -> bool {
        self.canvas.length_box.is_some()
    }

    pub fn ui(&mut self, ui: &mut egui::Ui) {
        // Sampled before any text box runs this frame. Escape has already cleared focus by
        // now, so an open number box also counts as owning the keyboard.
        let keys_free = !ui.ctx().text_edit_focused() && self.canvas.length_box.is_none();
        if keys_free {
            self.shortcuts(ui);
        }
        self.selection = self.selection.validated(self.doc.project());
        egui::Panel::top("pattern_tools").show(ui, |ui| self.toolbar(ui));
        egui::CentralPanel::default().show(ui, |ui| self.canvas_ui(ui, keys_free));
    }

    fn shortcuts(&mut self, ui: &egui::Ui) {
        // Redo first: `consume_shortcut` also matches Cmd+Z while Shift is held.
        if ui.input_mut(|i| i.consume_shortcut(&REDO) || i.consume_shortcut(&REDO_Y)) {
            self.redo();
        } else if ui.input_mut(|i| i.consume_shortcut(&UNDO)) {
            self.undo();
        }
        for tool in Tool::ALL {
            if ui.input_mut(|i| i.consume_key(Modifiers::NONE, tool.key())) {
                self.set_tool(tool);
            }
        }
        if ui.input_mut(|i| i.consume_key(Modifiers::NONE, Key::F)) {
            self.fit_pending = true;
        }
    }

    fn toolbar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal_wrapped(|ui| {
            for tool in Tool::ALL {
                let label = format!("{} ({})", tool.label(), tool.key().name());
                if ui.selectable_label(self.tool == tool, label).on_hover_text(tool.tip()).clicked() {
                    self.set_tool(tool);
                }
            }
            ui.separator();
            let units = self.doc.project().units;
            for (u, label) in [(Units::Cm, tr!("units-cm")), (Units::Inch, tr!("units-inch"))] {
                if ui.selectable_label(units == u, label).clicked() && units != u {
                    self.doc.edit(|p| p.units = u);
                }
            }
            ui.separator();
            ui.checkbox(&mut self.show_lengths, tr!("toolbar-show-lengths"));
            if ui.button(tr!("toolbar-fit")).on_hover_text(tr!("toolbar-fit-tip")).clicked() {
                self.fit_pending = true;
            }
        });
    }
}

/// The smallest box (mm) holding every piece.
fn project_bounds(project: &Project) -> Option<(Point2, Point2)> {
    let mut points = project.pieces.iter().flat_map(|p| opendrape_geom::outline_points(p, 1.0));
    let first = points.next()?;
    Some(points.fold((first, first), |(lo, hi), p| {
        (Point2::new(lo.x.min(p.x), lo.y.min(p.y)), Point2::new(hi.x.max(p.x), hi.y.max(p.y)))
    }))
}
```

- [ ] **Step 4: The canvas, the number box and painting**

`crates/app/src/editor/length_box.rs`:
```rust
//! The small box that appears by the pointer for typing exact numbers: an edge's length and
//! angle while drawing with the pen, or a rectangle's width and height.

use crate::tr;
use egui::{Id, Key, Pos2};
use opendrape_core::{Point2, Units};

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum BoxKind {
    /// The pen's next edge: length and angle from its last point.
    PenSegment,
    /// A rectangle with its lower-left corner here: width and height.
    Rectangle(Point2),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome {
    Commit,
    Cancel,
}

#[derive(Clone, Debug)]
pub struct LengthBox {
    pub kind: BoxKind,
    pub first: String,
    pub second: String,
    pos: Pos2,
    focus_first: bool,
}

impl LengthBox {
    pub fn new(kind: BoxKind, first: String, second: String, pos: Pos2) -> Self {
        Self { kind, first, second, pos, focus_first: true }
    }

    /// Shows the box. Enter in either field commits; Tab moves between the fields; Escape or
    /// clicking elsewhere cancels.
    pub fn show(&mut self, ctx: &egui::Context, units: Units) -> Option<Outcome> {
        let (first_label, second_label, second_unit) = match self.kind {
            BoxKind::PenSegment => (tr!("box-length"), tr!("box-angle"), "°"),
            BoxKind::Rectangle(_) => (tr!("box-width"), tr!("box-height"), units.suffix()),
        };
        let focus = std::mem::take(&mut self.focus_first);
        let (mut enter, mut lost, mut any_focus) = (false, false, false);
        egui::Area::new(Id::new("pattern_number_box"))
            .order(egui::Order::Foreground)
            .fixed_pos(self.pos)
            .show(ctx, |ui| {
                egui::Frame::popup(ui.style()).show(ui, |ui| {
                    ui.horizontal(|ui| {
                        let la = ui.label(first_label);
                        let a = ui
                            .add(egui::TextEdit::singleline(&mut self.first).desired_width(56.0))
                            .labelled_by(la.id);
                        ui.label(units.suffix());
                        let lb = ui.label(second_label);
                        let b = ui
                            .add(egui::TextEdit::singleline(&mut self.second).desired_width(48.0))
                            .labelled_by(lb.id);
                        ui.label(second_unit);
                        if focus {
                            a.request_focus();
                        }
                        for r in [&a, &b] {
                            any_focus |= r.has_focus();
                            if r.lost_focus() {
                                lost = true;
                                enter |= ui.input(|i| i.key_pressed(Key::Enter));
                            }
                        }
                    });
                });
            });
        let (tab, escape) = ctx.input(|i| (i.key_pressed(Key::Tab), i.key_pressed(Key::Escape)));
        if lost && enter {
            Some(Outcome::Commit)
        } else if escape || (lost && !tab && !any_focus) {
            Some(Outcome::Cancel)
        } else {
            None
        }
    }
}
```

`crates/app/src/editor/canvas.rs`:
```rust
//! Pointer and keyboard handling on the pattern table, one tool at a time.

use super::length_box::{BoxKind, LengthBox, Outcome};
use super::{EMPTY_TABLE, HIT_PX, PatternEditor, Selection, Tool, project_bounds};
use crate::tr;
use egui::{Event, Key, PointerButton, Response, Sense, vec2};
use opendrape_core::{Edge, Piece, PieceId, Point2, Units, VertexKind};
use opendrape_geom as geom;

/// A point placed with the pen.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PenPoint {
    pub pos: Point2,
    /// The curve handle pulled out by dragging while placing the point: the edge leaving the
    /// point bends towards it, and the edge arriving is mirrored. `None` for a corner.
    pub handle: Option<Point2>,
}

#[derive(Default)]
pub(super) struct CanvasState {
    pub pen: Vec<PenPoint>,
    /// The last pen point is being dragged out into a curve point.
    pub pen_dragging: bool,
    /// Rectangle tool: the corner where the drag started.
    pub rect_start: Option<Point2>,
    pub length_box: Option<LengthBox>,
    /// The pointer on the pattern (snapped, for the pen and rectangle tools).
    pub cursor: Option<Point2>,
    /// Add-point tool: where a click would add the point.
    pub preview: Option<Point2>,
}

impl PatternEditor {
    pub(super) fn canvas_ui(&mut self, ui: &mut egui::Ui, keys_free: bool) {
        let (response, painter) = ui.allocate_painter(ui.available_size(), Sense::click_and_drag());
        let rect = response.rect;
        self.canvas_rect = rect;
        if rect.width() < 1.0 || rect.height() < 1.0 {
            return; // the window is too small to draw in
        }
        if std::mem::take(&mut self.fit_pending) {
            let (min, max) = project_bounds(self.doc.project()).unwrap_or(EMPTY_TABLE);
            self.view.fit(rect, min, max);
        }
        self.pan_and_zoom(ui, &response);

        let tol = self.view.mm(HIT_PX);
        let shift = ui.input(|i| i.modifiers.shift);
        let hover = response.hover_pos().map(|p| self.view.to_world(rect, p));
        let cursor = hover.map(|w| match self.tool {
            Tool::Pen => self.snap(w, tol, shift),
            Tool::Rectangle => self.snap(w, tol, false),
            Tool::Edit | Tool::AddPoint => w,
        });
        self.canvas.cursor = cursor;
        if response.clicked() || response.drag_started() {
            self.notice = None;
        }
        let press = ui.input(|i| i.pointer.press_origin()).map(|p| self.view.to_world(rect, p));
        let pointer = response.interact_pointer_pos().map(|p| self.view.to_world(rect, p));
        let latest = ui.input(|i| i.pointer.latest_pos()).map(|p| self.view.to_world(rect, p));
        match self.tool {
            Tool::Pen => self.pen_tool(&response, press, pointer, tol, shift),
            Tool::Rectangle => self.rectangle_tool(&response, press, pointer, latest, tol),
            Tool::Edit | Tool::AddPoint => {} // Task 6
        }
        if keys_free {
            self.canvas_keys(ui, &response);
        }
        if let Some(mut number_box) = self.canvas.length_box.take() {
            match number_box.show(ui.ctx(), self.doc.project().units) {
                None => self.canvas.length_box = Some(number_box),
                Some(Outcome::Commit) => self.commit_box(&number_box),
                Some(Outcome::Cancel) => {}
            }
        }
        self.selection = self.selection.validated(self.doc.project());
        self.paint(&painter, rect, ui.visuals().dark_mode);
    }

    fn pan_and_zoom(&mut self, ui: &egui::Ui, response: &Response) {
        let rect = response.rect;
        if response.contains_pointer() {
            let (zoom, scroll) = ui.input(|i| (i.zoom_delta(), i.smooth_scroll_delta()));
            if zoom != 1.0
                && let Some(p) = response.hover_pos()
            {
                self.view.zoom_at(rect, p, f64::from(zoom));
            }
            // Two-finger scroll or the mouse wheel pans; pinch or Ctrl/Cmd + wheel zooms.
            self.view.pan += scroll;
        }
        if response.dragged_by(PointerButton::Middle) || response.dragged_by(PointerButton::Secondary) {
            self.view.pan += response.drag_delta();
        }
    }

    fn pen_tool(&mut self, response: &Response, press: Option<Point2>, pointer: Option<Point2>, tol: f64, shift: bool) {
        if response.drag_started_by(PointerButton::Primary)
            && let Some(at) = press
        {
            let at = self.snap(at, tol, shift);
            self.canvas.pen_dragging = self.pen_place(at, tol);
        }
        if self.canvas.pen_dragging
            && response.dragged_by(PointerButton::Primary)
            && let Some(now) = pointer
            && let Some(last) = self.canvas.pen.last_mut()
        {
            last.handle = (now.distance(last.pos) > tol).then_some(now);
        }
        if response.drag_stopped() {
            self.canvas.pen_dragging = false;
        }
        if response.clicked()
            && let Some(at) = pointer
        {
            let at = self.snap(at, tol, shift);
            self.pen_place(at, tol);
        }
    }

    /// Adds a pen point at `at`, or finishes the piece when `at` is on its first or last point.
    /// Returns whether a point was added.
    fn pen_place(&mut self, at: Point2, tol: f64) -> bool {
        let pen = &self.canvas.pen;
        let on_first = pen.first().is_some_and(|p| p.pos.distance(at) <= tol);
        let on_last = pen.last().is_some_and(|p| p.pos.distance(at) <= tol);
        if pen.len() >= 3 && (on_first || on_last) {
            self.finish_pen();
            return false;
        }
        if pen.iter().any(|p| p.pos.distance(at) <= tol) {
            return false; // the same spot again: nothing to add
        }
        self.canvas.pen.push(PenPoint { pos: at, handle: None });
        true
    }

    fn finish_pen(&mut self) {
        if self.canvas.pen.len() < 3 {
            self.notice = Some(tr!("notice-need-three-points"));
            return;
        }
        let mut piece = pen_piece(&self.canvas.pen);
        self.canvas.pen.clear();
        self.canvas.length_box = None;
        let id = self.doc.edit(|p| {
            piece.name = p.next_piece_name(&tr!("piece-default-name"));
            p.add_piece(piece)
        });
        self.selection = Selection::Piece(id);
    }

    fn rectangle_tool(
        &mut self,
        response: &Response,
        press: Option<Point2>,
        pointer: Option<Point2>,
        latest: Option<Point2>,
        tol: f64,
    ) {
        if response.drag_started_by(PointerButton::Primary)
            && let Some(at) = press
        {
            self.canvas.rect_start = Some(self.snap(at, tol, false));
        }
        if response.drag_stopped()
            && let Some(start) = self.canvas.rect_start.take()
            && let Some(end) = latest
        {
            let end = self.snap(end, tol, false);
            let size = Point2::new((end.x - start.x).abs(), (end.y - start.y).abs());
            // A drag along a straight line has no area: no piece.
            if size.x > tol && size.y > tol {
                self.add_rectangle(Point2::new(start.x.min(end.x), start.y.min(end.y)), size.x, size.y);
            }
        }
        if response.clicked()
            && let Some(at) = pointer
        {
            let corner = self.snap(at, tol, false);
            let pos = self.view.to_screen(self.canvas_rect, at) + vec2(16.0, 16.0);
            self.canvas.length_box =
                Some(LengthBox::new(BoxKind::Rectangle(corner), String::new(), String::new(), pos));
        }
    }

    /// Where a pen or rectangle point at `w` lands. With Shift the pen keeps 45° steps from
    /// its last point; otherwise points snap to a nearby existing point.
    pub(super) fn snap(&self, w: Point2, tol: f64, shift: bool) -> Point2 {
        let pen = &self.canvas.pen;
        if shift && let Some(last) = pen.last() {
            return constrain_45(last.pos, w);
        }
        let first = pen.first().map(|p| p.pos);
        let vertices = self.doc.project().pieces.iter().flat_map(|p| p.vertices.iter().map(|v| v.pos));
        first
            .into_iter()
            .chain(vertices)
            .filter(|p| p.distance(w) <= tol)
            .min_by(|a, b| a.distance(w).total_cmp(&b.distance(w)))
            .unwrap_or(w)
    }

    fn canvas_keys(&mut self, ui: &egui::Ui, response: &Response) {
        let pressed = |k: Key| ui.input(|i| i.key_pressed(k));
        match self.tool {
            Tool::Pen if !self.canvas.pen.is_empty() => {
                self.open_box_on_digits(ui, response);
                if pressed(Key::Enter) {
                    self.finish_pen();
                }
                if pressed(Key::Escape) {
                    self.canvas.pen.clear();
                }
                if pressed(Key::Backspace) || pressed(Key::Delete) {
                    self.canvas.pen.pop();
                }
            }
            Tool::Rectangle if pressed(Key::Escape) => self.canvas.rect_start = None,
            _ => {} // Task 6 adds the edit-tool keys
        }
    }

    /// A digit typed while drawing opens the number box with that digit in it. The text events
    /// are taken out of the input so the box (created later this frame) doesn't get them twice.
    fn open_box_on_digits(&mut self, ui: &egui::Ui, response: &Response) {
        let mut typed = String::new();
        ui.input_mut(|i| {
            i.events.retain(|e| match e {
                Event::Text(t) if !t.is_empty() && t.chars().all(|c| c.is_ascii_digit() || c == '.' || c == ',') => {
                    typed.push_str(t);
                    false
                }
                _ => true,
            })
        });
        let Some(last) = self.canvas.pen.last() else { return };
        if typed.is_empty() {
            return;
        }
        let towards = self.canvas.cursor.map(|c| c - last.pos).filter(|d| d.length() > 1e-9);
        let degrees = towards.map_or(0.0, |d| d.y.atan2(d.x).to_degrees());
        let pos = response.hover_pos().unwrap_or(response.rect.center()) + vec2(16.0, 16.0);
        self.canvas.length_box = Some(LengthBox::new(BoxKind::PenSegment, typed, angle_text(degrees), pos));
    }

    fn commit_box(&mut self, number_box: &LengthBox) {
        let units = self.doc.project().units;
        let length = |text: &str| Units::parse(text).map(|v| units.to_mm(v)).filter(|mm| valid_length(*mm));
        match number_box.kind {
            BoxKind::PenSegment => {
                let angle = Units::parse(&number_box.second);
                match (length(&number_box.first), angle, self.canvas.pen.last()) {
                    (Some(mm), Some(degrees), Some(last)) => {
                        let r = degrees.to_radians();
                        let pos = last.pos + Point2::new(r.cos(), r.sin()) * mm;
                        self.canvas.pen.push(PenPoint { pos, handle: None });
                    }
                    _ => self.notice = Some(tr!("notice-bad-number")),
                }
            }
            BoxKind::Rectangle(at) => match (length(&number_box.first), length(&number_box.second)) {
                (Some(w), Some(h)) => self.add_rectangle(at, w, h),
                _ => self.notice = Some(tr!("notice-bad-number")),
            },
        }
    }

    pub(super) fn add_rectangle(&mut self, min: Point2, width: f64, height: f64) {
        let id = self.doc.edit(|p| {
            let name = p.next_piece_name(&tr!("piece-default-name"));
            p.add_piece(Piece::rectangle(PieceId(0), name, min, width, height))
        });
        self.selection = Selection::Piece(id);
    }
}

fn valid_length(mm: f64) -> bool {
    mm.is_finite() && mm > 0.0 && mm <= geom::MAX_EDGE_MM
}

/// The closed piece the pen points make. A point with a handle is smooth: the edge leaving it
/// starts towards the handle and the edge arriving ends mirrored through the point.
pub(super) fn pen_piece(points: &[PenPoint]) -> Piece {
    let corners: Vec<Point2> = points.iter().map(|p| p.pos).collect();
    let mut piece = Piece::polygon(PieceId(0), "", &corners);
    let n = points.len();
    for (i, a) in points.iter().enumerate() {
        if a.handle.is_some() {
            piece.vertices[i].kind = VertexKind::Smooth;
        }
        let b = points[(i + 1) % n];
        if a.handle.is_none() && b.handle.is_none() {
            continue;
        }
        let c1 = a.handle.unwrap_or_else(|| a.pos.lerp(b.pos, 1.0 / 3.0));
        let c2 = b.handle.map_or_else(|| a.pos.lerp(b.pos, 2.0 / 3.0), |h| b.pos - (h - b.pos));
        piece.edges[i] = Edge::Curve { c1, c2 };
    }
    piece
}

/// `to`, turned about `from` to the nearest multiple of 45°.
fn constrain_45(from: Point2, to: Point2) -> Point2 {
    let d = to - from;
    let step = std::f64::consts::FRAC_PI_4;
    let a = (d.y.atan2(d.x) / step).round() * step;
    from + Point2::new(a.cos(), a.sin()) * d.length()
}

/// An angle as the number box shows it: 0.0 to 359.9, one decimal, never "-0.0" or "360.0".
fn angle_text(degrees: f64) -> String {
    let d = ((degrees * 10.0).round() / 10.0).rem_euclid(360.0) + 0.0;
    format!("{d:.1}")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(x: f64, y: f64) -> Point2 {
        Point2::new(x, y)
    }

    #[test]
    fn pen_piece_mirrors_curve_handles() {
        let pts = [
            PenPoint { pos: p(0.0, 0.0), handle: None },
            PenPoint { pos: p(100.0, 0.0), handle: Some(p(130.0, 20.0)) },
            PenPoint { pos: p(50.0, 80.0), handle: None },
        ];
        let piece = pen_piece(&pts);
        assert_eq!(piece.vertices[1].kind, VertexKind::Smooth);
        let Edge::Curve { c2, .. } = piece.edges[0] else { panic!() };
        assert_eq!(c2, p(70.0, -20.0));
        let Edge::Curve { c1, .. } = piece.edges[1] else { panic!() };
        assert_eq!(c1, p(130.0, 20.0));
        assert_eq!(piece.edges[2], Edge::Line);
    }

    #[test]
    fn constrain_45_snaps_directions() {
        let q = constrain_45(p(0.0, 0.0), p(100.0, 10.0));
        assert_eq!(q.y, 0.0);
        let d = constrain_45(p(0.0, 0.0), p(50.0, 60.0));
        assert!((d.x - d.y).abs() < 1e-9 && (d.length() - p(50.0, 60.0).length()).abs() < 1e-9);
    }

    #[test]
    fn angle_text_is_tidy() {
        assert_eq!(angle_text(-0.000_01), "0.0");
        assert_eq!(angle_text(359.97), "0.0");
        assert_eq!(angle_text(-90.0), "270.0");
        assert_eq!(angle_text(45.04), "45.0");
    }
}
```

`crates/app/src/editor/paint.rs`:
```rust
//! Drawing the pattern table: grid, pieces, the selection, and drafts in progress.

use super::{PatternEditor, Selection, Tool, canvas::pen_piece};
use egui::{Align2, Color32, FontId, Painter, Pos2, Rect, Shape, Stroke, StrokeKind, vec2};
use opendrape_core::{Edge, Piece, Point2};
use opendrape_geom as geom;

struct Palette {
    table: Color32,
    minor: Color32,
    major: Color32,
    ink: Color32,
    fill: Color32,
    selected: Color32,
    handle: Color32,
    label: Color32,
}

impl Palette {
    fn new(dark: bool) -> Self {
        if dark {
            Self {
                table: Color32::from_gray(30),
                minor: Color32::from_gray(42),
                major: Color32::from_gray(64),
                ink: Color32::from_gray(220),
                fill: Color32::from_rgba_unmultiplied(120, 160, 230, 46),
                selected: Color32::from_rgb(255, 150, 90),
                handle: Color32::from_rgb(120, 170, 255),
                label: Color32::from_gray(190),
            }
        } else {
            Self {
                table: Color32::from_gray(250),
                minor: Color32::from_gray(232),
                major: Color32::from_gray(205),
                ink: Color32::from_rgb(40, 40, 60),
                fill: Color32::from_rgba_unmultiplied(70, 110, 200, 40),
                selected: Color32::from_rgb(220, 90, 30),
                handle: Color32::from_rgb(50, 110, 220),
                label: Color32::from_gray(70),
            }
        }
    }
}

impl PatternEditor {
    pub(super) fn paint(&self, painter: &Painter, rect: Rect, dark: bool) {
        let c = Palette::new(dark);
        painter.rect_filled(rect, 0.0, c.table);
        self.paint_grid(painter, rect, &c);
        for piece in &self.doc.project().pieces {
            self.paint_piece(painter, rect, piece, &c);
        }
        self.paint_selection(painter, rect, &c);
        self.paint_drafts(painter, rect, &c);
    }

    fn screen_points(&self, rect: Rect, points: Vec<Point2>) -> Vec<Pos2> {
        points.into_iter().map(|p| self.view.to_screen(rect, p)).collect()
    }

    /// One line per unit (cm or inch) and a darker one every ten; the fine lines hide when
    /// they would be closer than 6 points.
    fn paint_grid(&self, painter: &Painter, rect: Rect, c: &Palette) {
        let v = self.view;
        let unit = self.doc.project().units.mm_per_unit();
        let (lo, hi) = (v.to_world(rect, rect.left_bottom()), v.to_world(rect, rect.right_top()));
        for (step, color) in [(unit, c.minor), (unit * 10.0, c.major)] {
            if step * v.zoom < 6.0 {
                continue;
            }
            let mut x = (lo.x / step).floor() * step;
            while x <= hi.x {
                painter.vline(v.to_screen(rect, Point2::new(x, 0.0)).x, rect.y_range(), Stroke::new(1.0, color));
                x += step;
            }
            let mut y = (lo.y / step).floor() * step;
            while y <= hi.y {
                painter.hline(rect.x_range(), v.to_screen(rect, Point2::new(0.0, y)).y, Stroke::new(1.0, color));
                y += step;
            }
        }
    }

    fn paint_piece(&self, painter: &Painter, rect: Rect, piece: &Piece, c: &Palette) {
        let v = self.view;
        let outline = self.screen_points(rect, geom::outline_points(piece, v.mm(0.25)));
        let selected = self.selection.piece() == Some(piece.id);
        let ink = if selected { c.selected } else { c.ink };
        if outline.len() >= 3 {
            painter.add(fill_polygon(&outline, c.fill));
        }
        painter.add(Shape::closed_line(outline, Stroke::new(if selected { 2.0 } else { 1.5 }, ink)));
        for vertex in &piece.vertices {
            painter.circle_filled(v.to_screen(rect, vertex.pos), 3.0, ink);
        }
        // Grainline: a double-headed arrow through the middle, with the name beside it.
        let centre = v.to_screen(rect, geom::centroid(piece));
        let r = piece.grain_deg.to_radians();
        let along = vec2(r.cos() as f32, -(r.sin() as f32)) * 40.0;
        let grain = Stroke::new(1.0, c.label);
        painter.arrow(centre, along, grain);
        painter.arrow(centre, -along, grain);
        painter.text(centre + vec2(6.0, -4.0), Align2::LEFT_BOTTOM, &piece.name, FontId::proportional(13.0), c.label);
        if self.show_lengths {
            let units = self.doc.project().units;
            for i in 0..piece.len() {
                let mid = v.to_screen(rect, geom::point_on_edge(piece, i, 0.5));
                let text = units.format(geom::edge_length(piece, i));
                painter.text(mid, Align2::CENTER_CENTER, text, FontId::proportional(11.0), c.label);
            }
        }
    }

    fn paint_selection(&self, painter: &Painter, rect: Rect, c: &Palette) {
        let Some(piece) = self.selection.piece().and_then(|id| self.doc.project().piece(id)) else { return };
        let v = self.view;
        let handle = Stroke::new(1.0, c.handle);
        for (i, edge) in piece.edges.iter().enumerate() {
            if let Edge::Curve { c1, c2 } = *edge {
                let (a, b) = piece.edge_ends(i);
                for (end, h) in [(a, c1), (b, c2)] {
                    let (end, h) = (v.to_screen(rect, end), v.to_screen(rect, h));
                    painter.line_segment([end, h], handle);
                    painter.circle_stroke(h, 4.0, handle);
                }
            }
        }
        match self.selection {
            Selection::Edge(_, i) => {
                let pts = self.screen_points(rect, geom::edge_points(piece, i, v.mm(0.25)));
                painter.add(Shape::line(pts, Stroke::new(3.5, c.selected)));
            }
            Selection::Vertex(_, i) => {
                let p = v.to_screen(rect, piece.vertices[i].pos);
                painter.circle_filled(p, 5.5, c.selected);
                painter.circle_stroke(p, 5.5, Stroke::new(1.5, c.table));
            }
            Selection::Piece(_) | Selection::None => {}
        }
    }

    fn paint_drafts(&self, painter: &Painter, rect: Rect, c: &Palette) {
        let v = self.view;
        let ink = Stroke::new(1.5, c.selected);
        let pen = &self.canvas.pen;
        if let (Some(first), Some(last)) = (pen.first(), pen.last()) {
            if pen.len() >= 2 {
                let draft = pen_piece(pen);
                for i in 0..pen.len() - 1 {
                    painter.add(Shape::line(self.screen_points(rect, geom::edge_points(&draft, i, v.mm(0.25))), ink));
                }
            }
            for p in pen {
                let at = v.to_screen(rect, p.pos);
                painter.circle_filled(at, 3.5, c.selected);
                if let Some(h) = p.handle {
                    let mirrored = p.pos - (h - p.pos);
                    let handle = Stroke::new(1.0, c.handle);
                    painter.line_segment([v.to_screen(rect, mirrored), v.to_screen(rect, h)], handle);
                    painter.circle_stroke(v.to_screen(rect, h), 4.0, handle);
                }
            }
            // Ring around the first point: click it to finish.
            painter.circle_stroke(v.to_screen(rect, first.pos), 7.0, Stroke::new(1.0, c.selected));
            if let Some(cursor) = self.canvas.cursor
                && self.canvas.length_box.is_none()
            {
                let (a, b) = (v.to_screen(rect, last.pos), v.to_screen(rect, cursor));
                painter.extend(Shape::dashed_line(&[a, b], Stroke::new(1.0, c.selected), 4.0, 3.0));
                let text = self.doc.project().units.format(last.pos.distance(cursor));
                painter.text(b + vec2(12.0, -12.0), Align2::LEFT_BOTTOM, text, FontId::proportional(12.0), c.label);
            }
        }
        if let (Some(start), Some(end)) = (self.canvas.rect_start, self.canvas.cursor) {
            let r = Rect::from_two_pos(v.to_screen(rect, start), v.to_screen(rect, end));
            painter.rect_stroke(r, 0.0, ink, StrokeKind::Middle);
        }
        if self.tool == Tool::AddPoint
            && let Some(p) = self.canvas.preview
        {
            painter.circle_stroke(v.to_screen(rect, p), 4.5, ink);
        }
    }
}

/// epaint fills only convex shapes, so concave pieces are triangulated (earcut) into a mesh.
fn fill_polygon(points: &[Pos2], fill: Color32) -> Shape {
    let mut triangles: Vec<u32> = Vec::new();
    earcut::Earcut::new().earcut(points.iter().map(|p| [f64::from(p.x), f64::from(p.y)]), &[] as &[u32], &mut triangles);
    let mut mesh = egui::Mesh::default();
    for p in points {
        mesh.colored_vertex(*p, fill);
    }
    for &[a, b, c] in triangles.as_chunks::<3>().0 {
        mesh.add_triangle(a, b, c);
    }
    Shape::mesh(mesh)
}
```

- [ ] **Step 5: Run the tests and see them pass**

Run: `cargo nextest run -p opendrape --test editor && cargo nextest run -p opendrape --lib editor`
Expected:
- `tests/editor.rs`: 17 passed.
- Library: 13 passed (10 from Task 4 plus 3 canvas unit tests).
- The M0/M1 `tests/ui.rs` are untouched; run `cargo nextest run -p opendrape` to confirm.

- [ ] **Step 6: Commit**

```bash
cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings
git add crates/app
git commit -m "feat(editor): pattern window with pen and rectangle tools, typed lengths and angles

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 6: Edit and Add point tools

**Files:**
- Modify: `crates/app/src/editor/canvas.rs`, `crates/app/tests/editor.rs`

**Interfaces:**
- Consumes: Task 5's canvas.
- Produces:
  - `PatternEditor::delete_selection` (`pub(super)`, also used by Task 7's panel).
  - The free function `canvas::nearest_edge`.
  - Full behaviour for `Tool::Edit` and `Tool::AddPoint`.

**Behaviour:**
- **Edit (Z):**
  - Click selects, in this order:
    - a curve handle of the selected piece (which selects that edge);
    - a point;
    - an edge;
    - the inside of a piece;
    - nothing.
  - Dragging moves:
    - a point, carrying its handles;
    - a handle, with smooth points mirroring;
    - an edge, moving both of its ends;
    - a whole piece.
  - Each drag is one undo step.
  - Delete or Backspace removes the selected point (a piece keeps ≥ 3) or the selected piece. Esc deselects.
- **Add point (X):** click on an edge to add a point there. Curves are split exactly. A click within 2% of an existing point is refused with a notice.

- [ ] **Step 1: Failing tests** (append to `crates/app/tests/editor.rs`)

```rust
#[test]
fn clicking_selects_points_edges_and_pieces() {
    let mut h = harness();
    let id = with_rectangle(&mut h);
    for ((x, y), expected) in [
        ((400.0, 100.0), Selection::Vertex(id, 1)),
        ((250.0, 100.0), Selection::Edge(id, 0)),
        ((250.0, 300.0), Selection::Piece(id)),
        ((700.0, 550.0), Selection::None),
    ] {
        click(&mut h, x, y);
        assert_eq!(h.state().selection, expected, "click at {x},{y}");
    }
}

#[test]
fn dragging_a_point_is_one_undo_step() {
    let mut h = harness();
    let id = with_rectangle(&mut h);
    drag(&mut h, (100.0, 100.0), (150.0, 120.0));
    let piece = h.state().doc.project().piece(id).unwrap().clone();
    close(piece.vertices[0].pos, Point2::new(150.0, 120.0));
    assert_eq!(piece.vertices[1].pos, Point2::new(400.0, 100.0));
    assert_eq!(h.state().selection, Selection::Vertex(id, 0));
    cmd(&mut h, Key::Z);
    assert_eq!(h.state().doc.project().piece(id).unwrap().vertices[0].pos, Point2::new(100.0, 100.0));
}

#[test]
fn dragging_an_edge_moves_both_ends() {
    let mut h = harness();
    let id = with_rectangle(&mut h);
    drag(&mut h, (250.0, 100.0), (250.0, 60.0));
    let piece = h.state().doc.project().piece(id).unwrap().clone();
    close(piece.vertices[0].pos, Point2::new(100.0, 60.0));
    close(piece.vertices[1].pos, Point2::new(400.0, 60.0));
    assert_eq!(piece.vertices[2].pos, Point2::new(400.0, 500.0));
}

#[test]
fn dragging_inside_moves_the_whole_piece() {
    let mut h = harness();
    let id = with_rectangle(&mut h);
    drag(&mut h, (250.0, 300.0), (300.0, 350.0));
    let piece = h.state().doc.project().piece(id).unwrap().clone();
    for (v, (x, y)) in piece.vertices.iter().zip(SQUARE) {
        close(v.pos, Point2::new(x + 50.0, y + 50.0));
    }
}

#[test]
fn dragging_a_curve_handle_bends_its_edge() {
    let mut h = harness();
    let id = with_rectangle(&mut h);
    h.state_mut().doc.edit(|p| p.piece_mut(id).unwrap().set_curved(0, true));
    click(&mut h, 250.0, 300.0); // select the piece so its handles show
    drag(&mut h, (200.0, 100.0), (200.0, 40.0));
    let Edge::Curve { c1, c2 } = h.state().doc.project().piece(id).unwrap().edges[0] else { panic!("curved") };
    close(c1, Point2::new(200.0, 40.0));
    close(c2, Point2::new(300.0, 100.0));
    assert_eq!(h.state().selection, Selection::Edge(id, 0));
}

#[test]
fn add_point_splits_the_edge_under_the_pointer() {
    let mut h = harness();
    let id = with_rectangle(&mut h);
    key(&mut h, Key::X);
    click(&mut h, 250.0, 101.0);
    let piece = h.state().doc.project().piece(id).unwrap().clone();
    assert_eq!(piece.len(), 5);
    close(piece.vertices[1].pos, Point2::new(250.0, 100.0));
    assert_eq!(h.state().selection, Selection::Vertex(id, 1));
    click(&mut h, 102.0, 100.0); // right next to a corner: refused
    assert_eq!(h.state().doc.project().piece(id).unwrap().len(), 5);
    assert!(h.state().notice.is_some());
}

#[test]
fn delete_removes_points_then_pieces() {
    let mut h = harness();
    let id = with_rectangle(&mut h);
    click(&mut h, 400.0, 100.0);
    key(&mut h, Key::Delete);
    assert_eq!(h.state().doc.project().piece(id).unwrap().len(), 3);
    assert_eq!(h.state().selection, Selection::Piece(id));
    click(&mut h, 100.0, 100.0);
    key(&mut h, Key::Backspace);
    assert_eq!(h.state().doc.project().piece(id).unwrap().len(), 3, "a triangle keeps its points");
    assert!(h.state().notice.is_some());
    click(&mut h, 150.0, 300.0); // inside the triangle
    key(&mut h, Key::Delete);
    assert!(h.state().doc.project().pieces.is_empty());
}

#[test]
fn selection_survives_undo_of_its_piece() {
    let mut h = harness();
    key(&mut h, Key::S);
    drag(&mut h, (100.0, 100.0), (400.0, 500.0));
    key(&mut h, Key::Z);
    click(&mut h, 400.0, 500.0);
    assert!(matches!(h.state().selection, Selection::Vertex(_, 2)));
    cmd(&mut h, Key::Z); // takes the rectangle away again
    assert_eq!(h.state().selection, Selection::None);
    key(&mut h, Key::Delete); // nothing selected: nothing happens, nothing panics
    h.key_press_modifiers(Modifiers::COMMAND | Modifiers::SHIFT, Key::Z);
    h.run();
    assert_eq!(h.state().doc.project().pieces.len(), 1);
}

```

Run: `cargo nextest run -p opendrape --test editor`
Expected: the 8 new tests FAIL, because the tools do nothing yet. The 17 from Task 5 still pass.

- [ ] **Step 2: Implement** (`crates/app/src/editor/canvas.rs`)

1. The core import becomes `use opendrape_core::{Edge, HandleEnd, Piece, PieceId, Point2, Project, Units, VertexKind};`.
2. Add the field below to `CanvasState`, and put the `Hit`/`Drag` code after the `CanvasState` struct:
```rust
    /// Edit tool: what is being dragged.
    pub drag: Option<Drag>,
```
```rust
/// Something under the pointer, in the order the edit tool prefers them.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Hit {
    /// A curve handle of an edge of the selected piece.
    Handle(PieceId, usize, HandleEnd),
    Vertex(PieceId, usize),
    Edge(PieceId, usize),
    Inside(PieceId),
}

impl Hit {
    fn piece(self) -> PieceId {
        match self {
            Self::Handle(id, ..) | Self::Vertex(id, _) | Self::Edge(id, _) | Self::Inside(id) => id,
        }
    }

    fn selection(self) -> Selection {
        match self {
            Self::Handle(id, i, _) | Self::Edge(id, i) => Selection::Edge(id, i),
            Self::Vertex(id, i) => Selection::Vertex(id, i),
            Self::Inside(id) => Selection::Piece(id),
        }
    }
}

/// An edit-tool drag. Every frame recomputes the piece from how it was when the drag began
/// plus the pointer's offset from where it grabbed, so no movement is lost between frames.
pub(super) struct Drag {
    original: Piece,
    hit: Hit,
    grab: Point2,
}

impl Drag {
    fn moved(&self, d: Point2) -> Piece {
        let o = &self.original;
        let mut p = o.clone();
        match self.hit {
            Hit::Vertex(_, i) => p.move_vertex(i, o.vertices[i].pos + d),
            Hit::Handle(_, i, end) => {
                if let Edge::Curve { c1, c2 } = o.edges[i] {
                    let from = match end {
                        HandleEnd::Start => c1,
                        HandleEnd::End => c2,
                    };
                    p.set_handle(i, end, from + d);
                }
            }
            Hit::Edge(_, i) => {
                let j = o.next(i);
                p.move_vertex(i, o.vertices[i].pos + d);
                p.move_vertex(j, o.vertices[j].pos + d);
            }
            Hit::Inside(_) => p.translate(d),
        }
        p
    }
}
```
3. In `canvas_ui`, replace the tool `match` with:
```rust
        match self.tool {
            Tool::Edit => self.edit_tool(&response, press, pointer, tol),
            Tool::Pen => self.pen_tool(&response, press, pointer, tol, shift),
            Tool::Rectangle => self.rectangle_tool(&response, press, pointer, latest, tol),
            Tool::AddPoint => self.add_point_tool(&response, hover, pointer, tol),
        }
```
4. In `canvas_keys`, replace `_ => {} // Task 6 adds the edit-tool keys` with:
```rust
            Tool::Edit => {
                if pressed(Key::Escape) {
                    self.selection = Selection::None;
                }
                if pressed(Key::Delete) || pressed(Key::Backspace) {
                    self.delete_selection();
                }
            }
            _ => {}
```
5. Add to `impl PatternEditor`:
```rust
    fn edit_tool(&mut self, response: &Response, press: Option<Point2>, pointer: Option<Point2>, tol: f64) {
        if response.drag_started_by(PointerButton::Primary)
            && let Some(grab) = press
            && let Some(hit) = self.hit(grab, tol)
            && let Some(original) = self.doc.project().piece(hit.piece()).cloned()
        {
            self.selection = hit.selection();
            self.doc.begin_gesture();
            self.canvas.drag = Some(Drag { original, hit, grab });
        }
        if response.dragged_by(PointerButton::Primary)
            && let (Some(drag), Some(now)) = (&self.canvas.drag, pointer)
        {
            let moved = drag.moved(now - drag.grab);
            let id = moved.id;
            self.doc.gesture_edit(|p| {
                if let Some(piece) = p.piece_mut(id) {
                    *piece = moved;
                }
            });
        }
        if response.drag_stopped() && self.canvas.drag.take().is_some() {
            self.doc.end_gesture();
        }
        if response.clicked()
            && let Some(at) = pointer
        {
            self.selection = self.hit(at, tol).map_or(Selection::None, Hit::selection);
        }
    }

    /// What the edit tool picks at `w`: the selected piece's curve handles first, then points,
    /// edges and piece insides, topmost piece first.
    fn hit(&self, w: Point2, tol: f64) -> Option<Hit> {
        let project = self.doc.project();
        if let Some(piece) = self.selection.piece().and_then(|id| project.piece(id)) {
            for (i, edge) in piece.edges.iter().enumerate() {
                if let Edge::Curve { c1, c2 } = *edge {
                    if c1.distance(w) <= tol {
                        return Some(Hit::Handle(piece.id, i, HandleEnd::Start));
                    }
                    if c2.distance(w) <= tol {
                        return Some(Hit::Handle(piece.id, i, HandleEnd::End));
                    }
                }
            }
        }
        let pieces = || project.pieces.iter().rev();
        pieces()
            .find_map(|p| p.vertices.iter().position(|v| v.pos.distance(w) <= tol).map(|i| Hit::Vertex(p.id, i)))
            .or_else(|| {
                pieces().find_map(|p| geom::nearest_edge(p, w).filter(|e| e.2 <= tol).map(|(i, _, _)| Hit::Edge(p.id, i)))
            })
            .or_else(|| pieces().find(|p| geom::contains(p, w)).map(|p| Hit::Inside(p.id)))
    }

    /// Deletes the selected point or piece. A piece keeps at least 3 points.
    pub(super) fn delete_selection(&mut self) {
        match self.selection {
            Selection::Piece(id) => {
                self.doc.edit(|p| p.remove_piece(id));
                self.selection = Selection::None;
            }
            Selection::Vertex(id, i) => {
                if self.doc.edit(|p| p.piece_mut(id).is_some_and(|piece| piece.remove_vertex(i))) {
                    self.selection = Selection::Piece(id);
                } else {
                    self.notice = Some(tr!("notice-min-points"));
                }
            }
            Selection::Edge(..) | Selection::None => {}
        }
    }

    fn add_point_tool(&mut self, response: &Response, hover: Option<Point2>, pointer: Option<Point2>, tol: f64) {
        let project = self.doc.project();
        self.canvas.preview = hover
            .and_then(|w| nearest_edge(project, w, tol))
            .and_then(|(id, i, t)| Some(geom::point_on_edge(project.piece(id)?, i, t)));
        if response.clicked()
            && let Some(at) = pointer
            && let Some((id, i, t)) = nearest_edge(self.doc.project(), at, tol)
        {
            match self.doc.edit(|p| p.piece_mut(id).and_then(|piece| geom::split_edge(piece, i, t))) {
                Some(v) => self.selection = Selection::Vertex(id, v),
                None => self.notice = Some(tr!("notice-too-close")),
            }
        }
    }
```
6. In `mod.rs`, `undo` and `redo` drop any edit drag before changing the history. Add `self.canvas.drag = None;` just before `self.doc.undo();` and before `self.doc.redo();`.
7. Add the free function (beside `valid_length`):
```rust
/// The edge nearest to `w` within `tol` mm over all pieces: (piece, edge, curve parameter).
pub(super) fn nearest_edge(project: &Project, w: Point2, tol: f64) -> Option<(PieceId, usize, f64)> {
    project
        .pieces
        .iter()
        .filter_map(|p| geom::nearest_edge(p, w).map(|(i, t, d)| (p.id, i, t, d)))
        .filter(|hit| hit.3 <= tol)
        .min_by(|a, b| a.3.total_cmp(&b.3))
        .map(|(id, i, t, _)| (id, i, t))
}
```

- [ ] **Step 3: Run and see them pass**

Run: `cargo nextest run -p opendrape --test editor`
Expected: 25 passed.

- [ ] **Step 4: Commit**

```bash
cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings
git add crates/app
git commit -m "feat(editor): edit and add-point tools with one undo step per drag

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 7: Properties panel and status bar

**Files:**
- Create: `crates/app/src/editor/panel.rs`
- Modify: `crates/app/src/editor/mod.rs`, `crates/app/i18n/en-US/opendrape.ftl`, `crates/app/tests/editor.rs`

**Interfaces:**
- Consumes: Tasks 2, 5 and 6 (`set_edge_length`, `Anchor`, `delete_selection`).
- Produces:
  - A right-hand **Properties** panel:
    - piece: Name, Grain angle, Area, Perimeter, Delete piece;
    - edge: Length, the "keep fixed" start/end choice, and Curved;
    - point: X, Y, Smooth curve point, Delete point.
  - A bottom status bar showing any notice, a hint for the current tool, and the pointer position.

**Field rules (verified in the probe on 2026-10-09):**
- Typing edits a private copy of the text. Enter or clicking elsewhere applies it as one undo step, and only if the text changed. Escape throws it away.
- The selection is part of each field's id, so typed text can never land on a different edge.
- Numbers accept `,` or `.` as the decimal separator.
- Refused values leave the pattern unchanged and show a notice:
  - text that is not a number;
  - lengths ≤ 0 or > 10 m;
  - coordinates beyond 100 m.

- [ ] **Step 1: Strings** (append to the `.ftl`)

```
panel-title = Properties
panel-hint = Select a piece, a point or an edge to see and change its measurements.
panel-piece = Piece
panel-name = Name
panel-grain = Grain angle
panel-area = Area
panel-perimeter = Perimeter
panel-delete-piece = Delete piece
panel-edge = Edge
panel-length = Length
panel-keep-fixed = When the length changes, keep fixed:
panel-anchor-start = Start point
panel-anchor-end = End point
panel-curved = Curved
panel-point = Point
panel-x = X
panel-y = Y
panel-smooth = Smooth curve point
panel-delete-point = Delete point
hint-edit = Click to select. Drag points, curve handles, edges or whole pieces. Delete removes the selection.
hint-pen-start = Click to place corners, or press and drag to make a curve point.
hint-pen-drawing = Type a number for an exact length. Click the first point or press Enter to finish; Backspace removes the last point; Esc cancels.
hint-rectangle = Drag to draw a rectangle, or click and type its width and height.
hint-add-point = Click on an edge to add a point there.
status-cursor = x { $x }   y { $y }
```

- [ ] **Step 2: Failing tests** (append to `crates/app/tests/editor.rs`)

```rust
fn piece_of(h: &H, id: PieceId) -> Piece {
    h.state().doc.project().piece(id).unwrap().clone()
}

fn field_text(h: &H, label: &str) -> String {
    h.get_by_role_and_label(Role::TextInput, label).value().unwrap_or_default()
}

/// Clicks the property field `label`, replaces its text with `text` and presses Enter.
fn type_into(h: &mut H, label: &str, text: &str) {
    h.get_by_role_and_label(Role::TextInput, label).click();
    h.run();
    cmd(h, Key::A);
    h.get_by_role_and_label(Role::TextInput, label).type_text(text);
    h.run();
    key(h, Key::Enter);
}

fn untouched_rectangle(id: PieceId) -> Piece {
    Piece::rectangle(id, "Front", Point2::new(100.0, 100.0), 300.0, 400.0)
}

#[test]
fn panel_sets_an_edge_length() {
    let mut h = harness();
    let id = with_rectangle(&mut h);
    click(&mut h, 250.0, 100.0);
    assert_eq!(h.state().selection, Selection::Edge(id, 0));
    assert_eq!(field_text(&h, "Length"), "30.0");
    type_into(&mut h, "Length", "45");
    let p = piece_of(&h, id);
    assert!((opendrape_geom::edge_length(&p, 0) - 450.0).abs() < 1e-9);
    assert_eq!(p.vertices[0].pos, Point2::new(100.0, 100.0), "the start point stays");
    assert_eq!(field_text(&h, "Length"), "45.0");
    cmd(&mut h, Key::Z);
    assert_eq!(piece_of(&h, id), untouched_rectangle(id), "one undo step");
}

#[test]
fn panel_refuses_nonsense_lengths() {
    for text in ["abc", "-5", "0", "99999"] {
        let mut h = harness();
        let id = with_rectangle(&mut h);
        click(&mut h, 250.0, 100.0);
        type_into(&mut h, "Length", text);
        assert_eq!(piece_of(&h, id), untouched_rectangle(id), "{text}");
        assert!(h.state().notice.is_some(), "{text}");
    }
}

#[test]
fn keeping_the_end_point_fixed() {
    let mut h = harness();
    let id = with_rectangle(&mut h);
    click(&mut h, 250.0, 100.0);
    h.get_by_label("End point").click();
    h.run();
    type_into(&mut h, "Length", "45");
    let p = piece_of(&h, id);
    assert_eq!(p.vertices[1].pos, Point2::new(400.0, 100.0));
    assert_eq!(p.vertices[0].pos, Point2::new(-50.0, 100.0));
}

#[test]
fn curved_checkbox_bends_the_edge() {
    let mut h = harness();
    let id = with_rectangle(&mut h);
    click(&mut h, 250.0, 100.0);
    h.get_by_label("Curved").click();
    h.run();
    assert!(matches!(piece_of(&h, id).edges[0], Edge::Curve { .. }));
    cmd(&mut h, Key::Z);
    assert_eq!(piece_of(&h, id).edges[0], Edge::Line);
}

#[test]
fn renaming_a_piece_and_setting_its_grain() {
    let mut h = harness();
    let id = with_rectangle(&mut h);
    click(&mut h, 250.0, 300.0);
    assert_eq!(field_text(&h, "Name"), "Front");
    h.get_by_label("1200.0 cm²"); // 30 × 40 cm
    h.get_by_label("140.0 cm");
    type_into(&mut h, "Name", "Front skirt");
    type_into(&mut h, "Grain angle", "45");
    let p = piece_of(&h, id);
    assert_eq!((p.name.as_str(), p.grain_deg), ("Front skirt", 45.0));
}

#[test]
fn moving_a_point_by_typing_its_position() {
    let mut h = harness();
    let id = with_rectangle(&mut h);
    click(&mut h, 100.0, 100.0);
    assert_eq!(field_text(&h, "X"), "10.0");
    type_into(&mut h, "X", "12");
    type_into(&mut h, "Y", "-3,5");
    assert_eq!(piece_of(&h, id).vertices[0].pos, Point2::new(120.0, -35.0));
    assert_eq!(h.state().selection, Selection::Vertex(id, 0));
}

#[test]
fn inches_change_what_is_shown_not_the_pattern() {
    let mut h = harness();
    let id = with_rectangle(&mut h);
    click(&mut h, 250.0, 100.0);
    h.get_by_label("inch").click();
    h.run();
    assert_eq!(field_text(&h, "Length"), "11.81");
    assert_eq!(piece_of(&h, id), untouched_rectangle(id));
    type_into(&mut h, "Length", "12");
    assert!((opendrape_geom::edge_length(&piece_of(&h, id), 0) - 304.8).abs() < 1e-9);
}

#[test]
fn fit_shows_every_piece() {
    let mut h = harness();
    with_rectangle(&mut h);
    h.state_mut().doc.edit(|p| {
        p.add_piece(Piece::rectangle(PieceId(0), "Far", Point2::new(5000.0, 3000.0), 200.0, 200.0))
    });
    h.run();
    key(&mut h, Key::F);
    let ed = h.state();
    for p in [Point2::new(100.0, 100.0), Point2::new(5200.0, 3200.0)] {
        assert!(ed.canvas_rect.contains(ed.view.to_screen(ed.canvas_rect, p)), "{p:?}");
    }
}

#[test]
fn clicking_away_applies_the_typed_length_to_the_right_edge() {
    let mut h = harness();
    let id = with_rectangle(&mut h);
    click(&mut h, 250.0, 100.0);
    h.get_by_role_and_label(Role::TextInput, "Length").click();
    h.run();
    cmd(&mut h, Key::A);
    h.get_by_role_and_label(Role::TextInput, "Length").type_text("45");
    h.run();
    click(&mut h, 100.0, 300.0); // the left edge
    let p = piece_of(&h, id);
    assert!((opendrape_geom::edge_length(&p, 0) - 450.0).abs() < 1e-9, "typed into the bottom edge");
    assert!((opendrape_geom::edge_length(&p, 3) - 400.0).abs() < 1e-9, "left edge untouched");
    assert_eq!(h.state().selection, Selection::Edge(id, 3));
}

#[test]
fn status_bar_explains_the_current_tool() {
    let mut h = harness();
    h.get_by_label_contains("Click to select");
    key(&mut h, Key::H);
    h.get_by_label_contains("press and drag to make a curve point");
    click(&mut h, 100.0, 100.0);
    h.get_by_label_contains("Type a number for an exact length");
}
```

Run: `cargo nextest run -p opendrape --test editor`
Expected: the 10 new tests FAIL (no panel yet); the other 25 pass.

- [ ] **Step 3: Implement**

`crates/app/src/editor/panel.rs`:
```rust
//! The properties panel (measurements of the selected piece, edge or point, editable by
//! typing) and the status bar.

use super::{PatternEditor, Selection, Tool};
use crate::tr;
use egui::{Id, Key};
use opendrape_core::{Edge, PieceId, Point2, Project, Units, VertexKind};
use opendrape_geom::{self as geom, Anchor};

/// Furthest from the origin a point may be typed (100 m), so a slip can't lose a piece.
const MAX_COORDINATE_MM: f64 = 100_000.0;

#[derive(Default)]
pub(super) struct PanelState {
    /// The field being typed in and its text, applied on Enter or clicking elsewhere.
    editing: Option<(Id, String)>,
    /// Which end of an edge stays put when its length is typed.
    anchor: Anchor,
}

impl PatternEditor {
    pub(super) fn properties(&mut self, ui: &mut egui::Ui) {
        ui.heading(tr!("panel-title"));
        ui.separator();
        match self.selection {
            Selection::None => {
                ui.label(tr!("panel-hint"));
            }
            Selection::Piece(id) => self.piece_properties(ui, id),
            Selection::Edge(id, i) => self.edge_properties(ui, id, i),
            Selection::Vertex(id, i) => self.vertex_properties(ui, id, i),
        }
    }

    fn piece_properties(&mut self, ui: &mut egui::Ui, id: PieceId) {
        let Some(piece) = self.doc.project().piece(id).cloned() else { return };
        let units = self.doc.project().units;
        ui.strong(tr!("panel-piece"));
        egui::Grid::new("piece_properties").num_columns(3).show(ui, |ui| {
            if let Some(name) = self.field(ui, tr!("panel-name"), &piece.name, "") {
                let name = name.trim().to_owned();
                if !name.is_empty() {
                    self.doc.edit(|p| {
                        if let Some(pc) = p.piece_mut(id) {
                            pc.name = name;
                        }
                    });
                }
            }
            let grain = self.field(ui, tr!("panel-grain"), &format!("{:.1}", piece.grain_deg), "°");
            self.apply_typed(grain, false, |p, deg| {
                p.piece_mut(id).is_some_and(|pc| {
                    pc.grain_deg = deg.rem_euclid(360.0);
                    true
                })
            });
            ui.label(tr!("panel-area"));
            ui.label(units.format_area(geom::area(&piece)));
            ui.end_row();
            ui.label(tr!("panel-perimeter"));
            ui.label(units.format(geom::perimeter(&piece)));
            ui.end_row();
        });
        ui.add_space(6.0);
        if ui.button(tr!("panel-delete-piece")).clicked() {
            self.delete_selection();
        }
    }

    fn edge_properties(&mut self, ui: &mut egui::Ui, id: PieceId, i: usize) {
        let Some(piece) = self.doc.project().piece(id).cloned() else { return };
        let units = self.doc.project().units;
        let anchor = self.panel.anchor;
        ui.strong(tr!("panel-edge"));
        egui::Grid::new("edge_properties").num_columns(3).show(ui, |ui| {
            let length = units.format_number(geom::edge_length(&piece, i));
            let typed = self.field(ui, tr!("panel-length"), &length, units.suffix());
            self.apply_typed(typed, true, |p, mm| {
                p.piece_mut(id).is_some_and(|pc| geom::set_edge_length(pc, i, mm, anchor))
            });
        });
        ui.label(tr!("panel-keep-fixed"));
        ui.horizontal(|ui| {
            ui.radio_value(&mut self.panel.anchor, Anchor::Start, tr!("panel-anchor-start"));
            ui.radio_value(&mut self.panel.anchor, Anchor::End, tr!("panel-anchor-end"));
        });
        let mut curved = matches!(piece.edges[i], Edge::Curve { .. });
        if ui.checkbox(&mut curved, tr!("panel-curved")).changed() {
            self.doc.edit(|p| {
                if let Some(pc) = p.piece_mut(id) {
                    pc.set_curved(i, curved);
                }
            });
        }
    }

    fn vertex_properties(&mut self, ui: &mut egui::Ui, id: PieceId, i: usize) {
        let Some(piece) = self.doc.project().piece(id).cloned() else { return };
        let units = self.doc.project().units;
        let pos = piece.vertices[i].pos;
        let move_to = move |p: &mut Project, to: Point2| {
            to.x.abs() <= MAX_COORDINATE_MM
                && to.y.abs() <= MAX_COORDINATE_MM
                && p.piece_mut(id).is_some_and(|pc| {
                    pc.move_vertex(i, to);
                    true
                })
        };
        ui.strong(tr!("panel-point"));
        egui::Grid::new("vertex_properties").num_columns(3).show(ui, |ui| {
            let x = self.field(ui, tr!("panel-x"), &units.format_number(pos.x), units.suffix());
            self.apply_typed(x, true, |p, mm| move_to(p, Point2::new(mm, pos.y)));
            let y = self.field(ui, tr!("panel-y"), &units.format_number(pos.y), units.suffix());
            self.apply_typed(y, true, |p, mm| move_to(p, Point2::new(pos.x, mm)));
        });
        let mut smooth = piece.vertices[i].kind == VertexKind::Smooth;
        if ui.checkbox(&mut smooth, tr!("panel-smooth")).changed() {
            let kind = if smooth { VertexKind::Smooth } else { VertexKind::Corner };
            self.doc.edit(|p| {
                if let Some(pc) = p.piece_mut(id) {
                    pc.set_vertex_kind(i, kind);
                }
            });
        }
        ui.add_space(6.0);
        if ui.button(tr!("panel-delete-point")).clicked() {
            self.delete_selection();
        }
    }

    /// One row of a 3-column grid (label, text field, unit). Returns what the user typed once
    /// they press Enter or click elsewhere, if it changed.
    fn field(&mut self, ui: &mut egui::Ui, label: String, value: &str, unit: &str) -> Option<String> {
        // The selection is part of the id, so typed text can never land on another edge.
        let id = Id::new(("pattern_property", &label, self.selection));
        let typed = text_field(ui, &mut self.panel.editing, id, &label, value);
        ui.label(unit);
        ui.end_row();
        typed
    }

    /// Applies a typed number as one undo step. Lengths (`is_length`) are typed in the current
    /// units and passed on in millimetres. `apply` returns false to refuse the value: the
    /// pattern stays unchanged and a notice says why.
    fn apply_typed(&mut self, typed: Option<String>, is_length: bool, apply: impl FnOnce(&mut Project, f64) -> bool) {
        let Some(text) = typed else { return };
        let units = self.doc.project().units;
        let value = Units::parse(&text).map(|v| if is_length { units.to_mm(v) } else { v });
        if !value.is_some_and(|v| self.doc.edit(|p| apply(p, v))) {
            self.notice = Some(tr!("notice-bad-number"));
        }
    }

    pub(super) fn status_bar(&self, ui: &mut egui::Ui) {
        ui.horizontal_wrapped(|ui| {
            if let Some(notice) = &self.notice {
                ui.colored_label(ui.visuals().warn_fg_color, notice);
                ui.separator();
            }
            ui.label(self.hint());
            if let Some(c) = self.canvas.cursor {
                let units = self.doc.project().units;
                ui.separator();
                ui.label(tr!("status-cursor", x = units.format(c.x), y = units.format(c.y)));
            }
        });
    }

    fn hint(&self) -> String {
        match self.tool {
            Tool::Edit => tr!("hint-edit"),
            Tool::Pen if self.canvas.pen.is_empty() => tr!("hint-pen-start"),
            Tool::Pen => tr!("hint-pen-drawing"),
            Tool::Rectangle => tr!("hint-rectangle"),
            Tool::AddPoint => tr!("hint-add-point"),
        }
    }
}

/// A one-line text box for a property. Typing edits a private copy; Enter or clicking
/// elsewhere returns it (when it differs from `value`); Escape throws it away.
fn text_field(ui: &mut egui::Ui, editing: &mut Option<(Id, String)>, id: Id, label: &str, value: &str) -> Option<String> {
    let mine = matches!(editing, Some((e, _)) if *e == id);
    let mut text = match editing {
        Some((e, t)) if *e == id => t.clone(),
        _ => value.to_owned(),
    };
    let l = ui.label(label);
    let r = ui.add(egui::TextEdit::singleline(&mut text).id(id).desired_width(80.0)).labelled_by(l.id);
    if r.lost_focus() {
        let escaped = ui.input(|i| i.key_pressed(Key::Escape));
        if mine {
            *editing = None;
        }
        return (mine && !escaped && text != value).then_some(text);
    }
    if r.has_focus() {
        *editing = Some((id, text));
    }
    None
}
```

`crates/app/src/editor/mod.rs`:
- Add `mod panel;`.
- Add the field `panel: panel::PanelState` to `PatternEditor`, set to `panel::PanelState::default()` in `new()`.
- `ui()` lays out the two new panels before the canvas:
```rust
        egui::Panel::top("pattern_tools").show(ui, |ui| self.toolbar(ui));
        egui::Panel::bottom("pattern_status").show(ui, |ui| self.status_bar(ui));
        egui::Panel::right("pattern_properties")
            .resizable(false)
            .exact_size(240.0)
            .show(ui, |ui| self.properties(ui));
        egui::CentralPanel::default().show(ui, |ui| self.canvas_ui(ui, keys_free));
```

- [ ] **Step 4: Run and see them pass**

Run: `cargo nextest run -p opendrape --test editor && cargo nextest run -p opendrape --lib`
Expected: 35 editor tests passed; library tests pass.

- [ ] **Step 5: Commit**

```bash
cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings
git add crates/app
git commit -m "feat(editor): properties panel with typed measurements, and a status bar

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 8: Into the app: side-by-side layout, File and Edit menus, dialogs, unsaved-changes guard

**Files:**
- Create: `crates/app/src/file_dialogs.rs`
- Modify:
  - `crates/app/src/{app.rs,lib.rs,main.rs}`, `crates/app/Cargo.toml`;
  - `crates/app/i18n/en-US/opendrape.ftl`, `crates/app/tests/ui.rs`.

**Interfaces:**
- Consumes: `PatternEditor` (Tasks 5–7), `opendrape_io::{save, load, EXTENSION}` (Task 3).
- Produces:
  - `FileDialogs::{Native, Scripted}` with `scripted(Vec<Option<PathBuf>>)` and `always_cancel()`, plus `DialogKind`.
  - `Startup::file_dialogs`.
  - On `OpenDrapeApp`: `editor()`, `editor_mut()`, `window_title()`, `is_closing()`.

**Behaviour:**
- **Layout:** the 3D view (with its own toolbar) is a resizable left panel, and the pattern window fills the rest.
- **File menu:**
  - New ⌘N, Open… ⌘O, Save ⌘S, Save As… ⇧⌘S. (Ctrl on Windows.)
  - Save asks for a file name only the first time.
  - A chosen name without `.odp` gets it added.
- **Edit menu:** Undo and Redo, greyed out when there is nothing to undo or redo.
- **Unsaved changes:**
  - New, Open, closing the window, and switching graphics mode (which restarts) all ask "Save your changes?" first, with Save, Don't save and Cancel.
  - Cancel keeps everything as it was.
  - Save that fails, or whose dialog is cancelled, does not go on to the next step.
- **Errors:** a file that fails to open or save shows a message box. The project on screen is never replaced by a bad file.
- **Window title:** `skirt.odp — OpenDrape`, with a leading `•` while there are unsaved changes.
- **No dialogs in tests:** tests script the dialogs' answers, so no window ever opens. The native dialogs open as sheets on the OpenDrape window (macOS) without freezing it.

- [ ] **Step 1: Strings** (append to the `.ftl`)

```
menu-file = File
menu-new = New
menu-open = Open…
menu-save = Save
menu-save-as = Save As…
menu-edit = Edit
menu-undo = Undo
menu-redo = Redo
file-type-project = OpenDrape project
file-untitled = Untitled
window-title = { $name } — OpenDrape
window-title-unsaved = • { $name } — OpenDrape
unsaved-title = Save your changes?
unsaved-body = “{ $name }” has changes that are not saved yet. If you don't save, they will be lost.
unsaved-save = Save
unsaved-discard = Don't save
unsaved-cancel = Cancel
error-title = Something went wrong
error-open = This file could not be opened: { $error }.
error-save = The project could not be saved: { $error }.
error-ok = OK
```

- [ ] **Step 2: File dialogs**

`crates/app/Cargo.toml`: add `pollster.workspace = true` to `[target.'cfg(any(windows, target_os = "macos"))'.dependencies]` (beside `rfd`).

`crates/app/src/file_dialogs.rs`:
```rust
//! Open and Save dialogs. The app uses the system's own dialogs; tests script the answers, so
//! no window ever opens during testing.

use std::cell::RefCell;
use std::collections::VecDeque;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::mpsc::{Receiver, Sender, channel};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DialogKind {
    Open,
    Save,
}

#[derive(Clone, Debug)]
pub enum FileDialogs {
    /// The system's dialogs (macOS and Windows), shown without freezing the window.
    Native,
    /// Tests: each dialog takes the next answer; `None`, or no answers left, means Cancel.
    Scripted(Rc<RefCell<VecDeque<Option<PathBuf>>>>),
}

impl FileDialogs {
    pub fn scripted(answers: Vec<Option<PathBuf>>) -> Self {
        Self::Scripted(Rc::new(RefCell::new(answers.into())))
    }

    /// Every dialog is cancelled.
    pub fn always_cancel() -> Self {
        Self::Scripted(Rc::default())
    }

    /// Opens a dialog. The chosen path (or `None` for Cancel) arrives on the returned channel,
    /// with a repaint request.
    pub fn ask(
        &self,
        kind: DialogKind,
        suggested_name: &str,
        frame: &eframe::Frame,
        ctx: &egui::Context,
    ) -> Receiver<Option<PathBuf>> {
        let (tx, rx) = channel();
        match self {
            Self::Scripted(answers) => {
                let _ = tx.send(answers.borrow_mut().pop_front().flatten());
            }
            Self::Native => native(kind, suggested_name, frame, ctx.clone(), tx),
        }
        rx
    }
}

#[cfg(any(windows, target_os = "macos"))]
fn native(kind: DialogKind, suggested_name: &str, frame: &eframe::Frame, ctx: egui::Context, tx: Sender<Option<PathBuf>>) {
    use std::future::Future;
    use std::pin::Pin;
    let dialog = rfd::AsyncFileDialog::new()
        .add_filter(crate::tr!("file-type-project"), &[opendrape_io::EXTENSION])
        .set_parent(frame);
    // The dialog is created here, on the main thread; only the waiting happens elsewhere.
    let answer: Pin<Box<dyn Future<Output = Option<rfd::FileHandle>> + Send>> = match kind {
        DialogKind::Open => Box::pin(dialog.pick_file()),
        DialogKind::Save => Box::pin(dialog.set_file_name(suggested_name).save_file()),
    };
    std::thread::spawn(move || {
        let path = pollster::block_on(answer).map(|f| f.path().to_path_buf());
        let _ = tx.send(path);
        ctx.request_repaint();
    });
}

/// Linux builds have no file dialogs yet: every dialog is cancelled.
#[cfg(not(any(windows, target_os = "macos")))]
fn native(_: DialogKind, _: &str, _: &eframe::Frame, _: egui::Context, tx: Sender<Option<PathBuf>>) {
    let _ = tx.send(None);
}
```

`crates/app/src/lib.rs`: add `pub mod file_dialogs;` and `pub use file_dialogs::{DialogKind, FileDialogs};`.

`crates/app/src/main.rs`: the `Startup { … }` literal gains `file_dialogs: opendrape::FileDialogs::Native,`.

- [ ] **Step 3: Failing tests** (`crates/app/tests/ui.rs`)

Change the existing `harness` helper into:
```rust
fn harness(config_dir: &Path, shared: SharedState) -> Harness<'static, OpenDrapeApp> {
    harness_with(config_dir, shared, FileDialogs::always_cancel())
}

fn harness_with(config_dir: &Path, shared: SharedState, file_dialogs: FileDialogs) -> Harness<'static, OpenDrapeApp> {
    let startup = Startup {
        decision: SAVED_AUTO,
        previous: GpuState::default(),
        store: StateStore::new(Some(config_dir)),
        smoke_test: false,
        autoplay: false,
        file_dialogs,
    };
    Harness::builder()
        .with_size(egui::vec2(1000.0, 700.0))
        .wgpu()
        .build_eframe(move |cc| OpenDrapeApp::new(cc, startup, shared))
}
```
Also:
- add `file_dialogs: FileDialogs::always_cancel(),` to the `Startup` in `tiny_window_does_not_crash`;
- add `use opendrape::FileDialogs;` and `use opendrape_core::{Piece, PieceId, Point2, Project};` at the top.

Then append:
```rust
type App = Harness<'static, OpenDrapeApp>;

/// A piece drawn in the pattern window, leaving unsaved changes.
fn add_piece(h: &mut App) {
    h.state_mut().editor_mut().doc.edit(|p| {
        p.add_piece(Piece::rectangle(PieceId(0), "Front", Point2::new(0.0, 0.0), 300.0, 500.0))
    });
    h.run();
}

fn file_menu(h: &mut App, item: &str) {
    h.get_by_label("File").click();
    h.run();
    h.get_by_label(item).click();
    h.run();
}

fn pieces(h: &App) -> usize {
    h.state().editor().doc.project().pieces.len()
}

#[test]
fn pattern_window_sits_beside_the_3d_view() {
    let dir = tempfile::tempdir().unwrap();
    let mut h = harness(dir.path(), SharedState::default());
    h.run();
    let canvas = h.state().editor().canvas_rect;
    assert!(canvas.width() > 200.0 && canvas.left() > 300.0, "{canvas:?}");
    h.get_by_label("Play"); // the 3D controls are still there
    h.get_by_label("Pen (H)");
    assert_eq!(h.state().window_title(), "Untitled — OpenDrape");
}

#[test]
fn save_as_writes_a_project_file() {
    let dir = tempfile::tempdir().unwrap();
    let chosen = dir.path().join("skirt"); // no extension: OpenDrape adds .odp
    let mut h = harness_with(dir.path(), SharedState::default(), FileDialogs::scripted(vec![Some(chosen)]));
    h.run();
    add_piece(&mut h);
    assert_eq!(h.state().window_title(), "• Untitled — OpenDrape");
    file_menu(&mut h, "Save As…");
    let saved = dir.path().join("skirt.odp");
    assert_eq!(opendrape_io::load(&saved).unwrap(), *h.state().editor().doc.project());
    assert_eq!(h.state().window_title(), "skirt.odp — OpenDrape");
}

#[test]
fn save_shortcut_saves_again_without_asking() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("skirt.odp");
    let mut h = harness_with(dir.path(), SharedState::default(), FileDialogs::scripted(vec![Some(file.clone())]));
    h.run();
    add_piece(&mut h);
    h.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::S);
    h.run();
    assert_eq!(opendrape_io::load(&file).unwrap().pieces.len(), 1);
    add_piece(&mut h);
    h.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::S);
    h.run();
    assert_eq!(opendrape_io::load(&file).unwrap().pieces.len(), 2, "same file, no second dialog");
}

#[test]
fn cancelling_the_save_dialog_changes_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let mut h = harness(dir.path(), SharedState::default()); // every dialog is cancelled
    h.run();
    add_piece(&mut h);
    file_menu(&mut h, "Save As…");
    assert!(h.state().editor().doc.is_dirty());
    assert_eq!(h.state().window_title(), "• Untitled — OpenDrape");
}

#[test]
fn open_replaces_the_project() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("bodice.odp");
    let mut project = Project::new();
    project.add_piece(Piece::rectangle(PieceId(0), "Bodice", Point2::new(0.0, 0.0), 200.0, 400.0));
    opendrape_io::save(&project, &file).unwrap();
    let mut h = harness_with(dir.path(), SharedState::default(), FileDialogs::scripted(vec![Some(file)]));
    h.run();
    file_menu(&mut h, "Open…");
    assert_eq!(*h.state().editor().doc.project(), project);
    assert_eq!(h.state().window_title(), "bodice.odp — OpenDrape");
    assert!(!h.state().editor().can_undo(), "a fresh history");
}

#[test]
fn opening_a_bad_file_keeps_the_current_project() {
    let dir = tempfile::tempdir().unwrap();
    let bad = dir.path().join("bad.odp");
    std::fs::write(&bad, b"not a zip file").unwrap();
    let mut h = harness_with(dir.path(), SharedState::default(), FileDialogs::scripted(vec![Some(bad)]));
    h.run();
    add_piece(&mut h);
    let before = h.state().editor().doc.project().clone();
    file_menu(&mut h, "Open…");
    h.get_by_label("Don't save").click(); // there are unsaved changes: asked first
    h.run();
    h.get_by_label_contains("not an OpenDrape project file");
    assert_eq!(*h.state().editor().doc.project(), before);
    h.get_by_label("OK").click();
    h.run();
    assert!(h.query_by_label_contains("not an OpenDrape project file").is_none());
}

#[test]
fn new_with_unsaved_changes_asks_first() {
    let dir = tempfile::tempdir().unwrap();
    let mut h = harness(dir.path(), SharedState::default());
    h.run();
    add_piece(&mut h);
    file_menu(&mut h, "New");
    h.get_by_label("Save your changes?");
    h.get_by_label("Cancel").click();
    h.run();
    assert_eq!(pieces(&h), 1, "Cancel keeps the work");
    file_menu(&mut h, "New");
    h.get_by_label("Don't save").click();
    h.run();
    assert_eq!(pieces(&h), 0);
    assert!(!h.state().editor().doc.is_dirty());
}

#[test]
fn saving_from_the_question_then_starts_the_new_project() {
    let dir = tempfile::tempdir().unwrap();
    let chosen = dir.path().join("draft.odp");
    let mut h = harness_with(dir.path(), SharedState::default(), FileDialogs::scripted(vec![Some(chosen.clone())]));
    h.run();
    add_piece(&mut h);
    file_menu(&mut h, "New");
    h.get_by_label("Save").click();
    h.run();
    assert_eq!(opendrape_io::load(&chosen).unwrap().pieces.len(), 1);
    assert_eq!(pieces(&h), 0);
    assert_eq!(h.state().window_title(), "Untitled — OpenDrape");
}

#[test]
fn closing_with_unsaved_changes_asks_first() {
    let dir = tempfile::tempdir().unwrap();
    let mut h = harness(dir.path(), SharedState::default());
    h.run();
    add_piece(&mut h);
    h.input_mut().viewports.entry(egui::ViewportId::ROOT).or_default().events.push(egui::ViewportEvent::Close);
    h.step();
    let cancelled = h.output().viewport_output.get(&egui::ViewportId::ROOT).is_some_and(|v| {
        v.commands.iter().any(|c| matches!(c, egui::ViewportCommand::CancelClose))
    });
    assert!(cancelled, "the window must stay open");
    h.run();
    assert!(!h.state().is_closing());
    h.get_by_label("Don't save").click();
    h.run();
    assert!(h.state().is_closing());
}

#[test]
fn edit_menu_undoes_and_redoes() {
    let dir = tempfile::tempdir().unwrap();
    let mut h = harness(dir.path(), SharedState::default());
    h.run();
    add_piece(&mut h);
    h.get_by_label("Edit").click();
    h.run();
    h.get_by_label("Undo").click();
    h.run();
    assert_eq!(pieces(&h), 0);
    h.get_by_label("Edit").click();
    h.run();
    h.get_by_label("Redo").click();
    h.run();
    assert_eq!(pieces(&h), 1);
}
```

Run: `cargo nextest run -p opendrape --test ui`
Expected: compile errors (`FileDialogs`, `editor()`, `window_title()` … do not exist yet).

- [ ] **Step 4: Implement `app.rs`** (complete file)

```rust
use crate::diagnostics::Diagnostics;
use crate::editor::{self, PatternEditor};
use crate::file_dialogs::{DialogKind, FileDialogs};
use crate::gpu::{Decision, GpuChoice, GpuState, Os, StateStore, confirmed_state};
use crate::sim_runner::{SimFrame, SimRunner};
use crate::tr;
use crate::viewport::Viewport;
use egui::{Key, KeyboardShortcut, Modifiers, ViewportCommand};
use opendrape_core::Project;
use opendrape_testkit::garments::Garment;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::mpsc::{Receiver, TryRecvError};
use std::{cell::Cell, rc::Rc};

/// What main() decided before the window opened.
#[derive(Clone, Debug)]
pub struct Startup {
    pub decision: Decision,
    pub previous: GpuState,
    pub store: StateStore,
    pub smoke_test: bool,
    /// Start simulating immediately (tests start paused, so `Harness::run` can settle).
    pub autoplay: bool,
    /// Where Open and Save get file names: the system dialogs, or a script in tests.
    pub file_dialogs: FileDialogs,
}

/// Results main() reads after the window closes.
#[derive(Debug, Default)]
pub struct Shared {
    /// The user picked another graphics mode: start a fresh process.
    pub restart_with: Cell<Option<GpuChoice>>,
    /// The GPU drew at least one 3D frame.
    pub first_frame_drawn: Cell<bool>,
}

pub type SharedState = Rc<Shared>;

/// Completed frames before a graphics mode counts as working (those frames have been presented by then).
const CONFIRM_AFTER_FRAMES: u64 = 3;

const NEW: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::N);
const OPEN: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::O);
const SAVE: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::S);
const SAVE_AS: KeyboardShortcut = KeyboardShortcut::new(Modifiers { shift: true, ..Modifiers::COMMAND }, Key::S);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum FileAction {
    New,
    Open,
    Save,
    SaveAs,
    /// Close the window (the graphics-mode switch restarts OpenDrape this way).
    Quit,
}

/// What to do once unsaved changes have been dealt with.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Then {
    NewProject,
    OpenFile,
    Quit,
}

#[derive(Clone, Copy, Debug)]
enum DialogFor {
    Open,
    SaveAs(Option<Then>),
}

/// A question waiting for the user; at most one at a time.
enum Pending {
    /// "Save your changes?", asked before `Then`.
    AskToSave(Then),
    /// A file dialog is open; its answer arrives on the channel.
    Dialog(DialogFor, Receiver<Option<PathBuf>>),
}

#[derive(Clone, Copy)]
enum Answer {
    Save,
    Discard,
    Cancel,
}

pub struct OpenDrapeApp {
    viewport: Option<Viewport>,
    diagnostics: Diagnostics,
    startup: Startup,
    shared: SharedState,
    show_about: bool,
    copied: bool,
    runner: Option<SimRunner>,
    garment: Garment,
    fps: f32,
    editor: PatternEditor,
    pending: Option<Pending>,
    /// Shown in a message box after a failed open or save.
    error: Option<String>,
    /// The user chose to quit without saving: let the window close.
    closing: bool,
    /// The window title last sent, so it is sent only when it changes.
    title: String,
}

impl OpenDrapeApp {
    pub fn new(cc: &eframe::CreationContext<'_>, startup: Startup, shared: SharedState) -> Self {
        let render_state = cc.wgpu_render_state.as_ref();
        let info = render_state.map(|rs| rs.adapter.get_info());
        crate::startup_log::stage(format_args!(
            "window open, graphics: {:?}",
            info.as_ref().map(|i| (&i.name, i.device_type, i.backend))
        ));
        let runner = render_state.map(|_| {
            let ctx = cc.egui_ctx.clone();
            SimRunner::start(Garment::Skirt, startup.autoplay, move || ctx.request_repaint())
        });
        Self {
            viewport: render_state.map(Viewport::new),
            diagnostics: Diagnostics::collect(info.as_ref(), startup.decision),
            startup,
            shared,
            show_about: false,
            copied: false,
            runner,
            garment: Garment::Skirt,
            fps: 0.0,
            editor: PatternEditor::new(),
            pending: None,
            error: None,
            closing: false,
            title: String::new(),
        }
    }

    pub fn editor(&self) -> &PatternEditor {
        &self.editor
    }

    pub fn editor_mut(&mut self) -> &mut PatternEditor {
        &mut self.editor
    }

    /// The user agreed to close the window (after dealing with unsaved changes).
    pub fn is_closing(&self) -> bool {
        self.closing
    }

    /// "skirt.odp — OpenDrape", with a leading "•" while there are unsaved changes.
    pub fn window_title(&self) -> String {
        let name = self.document_name();
        if self.editor.doc.is_dirty() {
            tr!("window-title-unsaved", name = name)
        } else {
            tr!("window-title", name = name)
        }
    }

    fn document_name(&self) -> String {
        self.editor
            .doc
            .path
            .as_deref()
            .and_then(Path::file_name)
            .map_or_else(|| tr!("file-untitled"), |n| n.to_string_lossy().into_owned())
    }

    /// The latest simulation frame, if the 3D view is running.
    pub fn sim_frame(&self) -> Option<Arc<SimFrame>> {
        self.runner.as_ref().map(SimRunner::latest)
    }

    /// `fps` is `None` while paused: the window then only redraws on input.
    pub fn stats_text(fps: Option<f32>, step_ms: f64, points: usize) -> String {
        let (ms, points) = (format!("{step_ms:.1}"), points.to_string());
        match fps {
            Some(fps) => tr!("overlay-stats", fps = format!("{fps:.0}"), ms = ms, points = points),
            None => tr!("overlay-stats-paused", ms = ms, points = points),
        }
    }

    fn toolbar(&mut self, ui: &mut egui::Ui) {
        let Some(runner) = &self.runner else { return };
        ui.horizontal_wrapped(|ui| {
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

    /// The 3D view: its toolbar, the body and garment, and the speed overlay.
    fn view_3d(&mut self, ui: &mut egui::Ui, frame: &eframe::Frame) {
        self.toolbar(ui);
        ui.separator();
        let sim = self.sim_frame();
        let fps = self.runner.as_ref().is_some_and(SimRunner::is_playing).then_some(self.fps);
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

    pub fn viewport_frames(&self) -> u64 {
        self.viewport.as_ref().map_or(0, |v| v.frames_drawn)
    }

    /// Save `choice` as the preferred graphics mode and ask main() to restart.
    pub fn request_graphics_change(&mut self, choice: GpuChoice) {
        self.startup.store.save(&GpuState { preferred: choice, pending: None });
        self.shared.restart_with.set(Some(choice));
    }

    /// Frames presented to the screen are proof this graphics mode works: clear the crash
    /// marker. egui presents a frame only after `ui()` returns, and some drivers crash on
    /// their first present, so wait until [`CONFIRM_AFTER_FRAMES`] frames have completed.
    /// (Counted with egui's frame number: `ui()` can run more than once per frame.)
    fn confirm_first_frame(&mut self, ctx: &egui::Context) {
        if self.shared.first_frame_drawn.get() {
            return;
        }
        crate::startup_log::stage(format_args!(
            "frame {}, 3D frames drawn: {}",
            ctx.cumulative_frame_nr(),
            self.viewport_frames()
        ));
        if self.viewport_frames() == 0 {
            return;
        }
        if ctx.cumulative_frame_nr() < CONFIRM_AFTER_FRAMES {
            ctx.request_repaint();
            return;
        }
        self.shared.first_frame_drawn.set(true);
        if let Some(state) = confirmed_state(self.startup.decision) {
            self.startup.store.save(&state);
        }
        if self.startup.smoke_test {
            ctx.send_viewport_cmd(ViewportCommand::Close);
        }
    }

    fn menu_bar(&mut self, ui: &mut egui::Ui) -> Option<FileAction> {
        let mut action = None;
        egui::MenuBar::new().ui(ui, |ui| {
            ui.menu_button(tr!("menu-file"), |ui| {
                let items = [
                    (tr!("menu-new"), NEW, FileAction::New),
                    (tr!("menu-open"), OPEN, FileAction::Open),
                    (tr!("menu-save"), SAVE, FileAction::Save),
                    (tr!("menu-save-as"), SAVE_AS, FileAction::SaveAs),
                ];
                for (label, shortcut, item) in items {
                    let button = egui::Button::new(label).shortcut_text(ui.ctx().format_shortcut(&shortcut));
                    if ui.add(button).clicked() {
                        action = Some(item);
                        ui.close();
                    }
                }
            });
            ui.menu_button(tr!("menu-edit"), |ui| {
                let undo = egui::Button::new(tr!("menu-undo")).shortcut_text(ui.ctx().format_shortcut(&editor::UNDO));
                if ui.add_enabled(self.editor.can_undo(), undo).clicked() {
                    self.editor.undo();
                    ui.close();
                }
                let redo = egui::Button::new(tr!("menu-redo")).shortcut_text(ui.ctx().format_shortcut(&editor::REDO));
                if ui.add_enabled(self.editor.can_redo(), redo).clicked() {
                    self.editor.redo();
                    ui.close();
                }
            });
            ui.menu_button(tr!("menu-help"), |ui| {
                if ui.button(tr!("menu-about")).clicked() {
                    self.show_about = true;
                    self.copied = false;
                    ui.close();
                }
                ui.menu_button(tr!("menu-graphics"), |ui| {
                    for &choice in GpuChoice::available(Os::current()) {
                        let current = self.startup.decision.choice == choice;
                        if ui.radio(current, choice_label(choice)).clicked() && !current {
                            self.request_graphics_change(choice);
                            action = Some(FileAction::Quit);
                        }
                    }
                    ui.separator();
                    ui.label(tr!("graphics-restart-note"));
                });
            });
        });
        action
    }

    /// ⌘N, ⌘O, ⌘S, ⇧⌘S (Ctrl on Windows), unless a text field is being typed in.
    fn file_shortcut(&self, ctx: &egui::Context) -> Option<FileAction> {
        if ctx.text_edit_focused() {
            return None;
        }
        // Save As first: `consume_shortcut` also matches ⌘S while Shift is held.
        [(SAVE_AS, FileAction::SaveAs), (SAVE, FileAction::Save), (NEW, FileAction::New), (OPEN, FileAction::Open)]
            .into_iter()
            .find(|(shortcut, _)| ctx.input_mut(|i| i.consume_shortcut(shortcut)))
            .map(|(_, action)| action)
    }

    fn file_action(&mut self, action: FileAction, frame: &eframe::Frame, ctx: &egui::Context) {
        if self.pending.is_some() {
            return; // a question or dialog is already open
        }
        match action {
            FileAction::New => self.after_saving_changes(Then::NewProject, frame, ctx),
            FileAction::Open => self.after_saving_changes(Then::OpenFile, frame, ctx),
            FileAction::Quit => self.after_saving_changes(Then::Quit, frame, ctx),
            FileAction::Save => self.save(None, frame, ctx),
            FileAction::SaveAs => self.ask_file(DialogKind::Save, DialogFor::SaveAs(None), frame, ctx),
        }
    }

    /// Runs `then`, first asking whether to save any unsaved changes.
    fn after_saving_changes(&mut self, then: Then, frame: &eframe::Frame, ctx: &egui::Context) {
        if self.editor.doc.is_dirty() {
            self.pending = Some(Pending::AskToSave(then));
        } else {
            self.run(then, frame, ctx);
        }
    }

    fn run(&mut self, then: Then, frame: &eframe::Frame, ctx: &egui::Context) {
        match then {
            Then::NewProject => self.editor.set_project(Project::new(), None),
            Then::OpenFile => self.ask_file(DialogKind::Open, DialogFor::Open, frame, ctx),
            Then::Quit => {
                self.closing = true;
                ctx.send_viewport_cmd(ViewportCommand::Close);
            }
        }
    }

    /// Saves to the project's file, asking for a name the first time, then runs `then`.
    fn save(&mut self, then: Option<Then>, frame: &eframe::Frame, ctx: &egui::Context) {
        match self.editor.doc.path.clone() {
            Some(path) => {
                if self.write(&path)
                    && let Some(then) = then
                {
                    self.run(then, frame, ctx);
                }
            }
            None => self.ask_file(DialogKind::Save, DialogFor::SaveAs(then), frame, ctx),
        }
    }

    fn ask_file(&mut self, kind: DialogKind, purpose: DialogFor, frame: &eframe::Frame, ctx: &egui::Context) {
        let suggested = self.editor.doc.path.as_deref().and_then(Path::file_name).map_or_else(
            || format!("{}.{}", tr!("file-untitled"), opendrape_io::EXTENSION),
            |n| n.to_string_lossy().into_owned(),
        );
        let answer = self.startup.file_dialogs.ask(kind, &suggested, frame, ctx);
        self.pending = Some(Pending::Dialog(purpose, answer));
    }

    fn write(&mut self, path: &Path) -> bool {
        match opendrape_io::save(self.editor.doc.project(), path) {
            Ok(()) => {
                self.editor.doc.mark_saved(path.to_path_buf());
                true
            }
            Err(e) => {
                self.error = Some(tr!("error-save", error = e.to_string()));
                false
            }
        }
    }

    /// Acts on a file dialog's answer once it arrives.
    fn poll_dialog(&mut self, frame: &eframe::Frame, ctx: &egui::Context) {
        let Some(Pending::Dialog(purpose, answer)) = &self.pending else { return };
        let answer = match answer.try_recv() {
            Ok(answer) => answer,
            Err(TryRecvError::Empty) => return,
            Err(TryRecvError::Disconnected) => None,
        };
        let purpose = *purpose;
        self.pending = None;
        let Some(path) = answer else { return }; // cancelled
        match purpose {
            DialogFor::Open => match opendrape_io::load(&path) {
                Ok(project) => self.editor.set_project(project, Some(path)),
                Err(e) => self.error = Some(tr!("error-open", error = e.to_string())),
            },
            DialogFor::SaveAs(then) => {
                if self.write(&with_project_extension(path))
                    && let Some(then) = then
                {
                    self.run(then, frame, ctx);
                }
            }
        }
    }

    /// Closing the window with unsaved changes asks first.
    fn guard_close(&mut self, ctx: &egui::Context) {
        if ctx.input(|i| i.viewport().close_requested()) && !self.closing && self.editor.doc.is_dirty() {
            ctx.send_viewport_cmd(ViewportCommand::CancelClose);
            if self.pending.is_none() {
                self.pending = Some(Pending::AskToSave(Then::Quit));
            }
        }
    }

    fn unsaved_changes_modal(&mut self, frame: &eframe::Frame, ctx: &egui::Context) {
        let Some(Pending::AskToSave(then)) = &self.pending else { return };
        let then = *then;
        let name = self.document_name();
        let mut answer = None;
        let modal = egui::Modal::new(egui::Id::new("unsaved_changes")).show(ctx, |ui| {
            ui.set_max_width(360.0);
            ui.heading(tr!("unsaved-title"));
            ui.label(tr!("unsaved-body", name = name));
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                if ui.button(tr!("unsaved-save")).clicked() {
                    answer = Some(Answer::Save);
                }
                if ui.button(tr!("unsaved-discard")).clicked() {
                    answer = Some(Answer::Discard);
                }
                if ui.button(tr!("unsaved-cancel")).clicked() {
                    answer = Some(Answer::Cancel);
                }
            });
        });
        if answer.is_none() && modal.should_close() {
            answer = Some(Answer::Cancel); // Escape, or a click outside the box
        }
        let Some(answer) = answer else { return };
        self.pending = None;
        match answer {
            Answer::Save => self.save(Some(then), frame, ctx),
            Answer::Discard => self.run(then, frame, ctx),
            // Not quitting after all, so don't restart into another graphics mode later.
            Answer::Cancel if then == Then::Quit => self.shared.restart_with.set(None),
            Answer::Cancel => {}
        }
    }

    fn error_modal(&mut self, ctx: &egui::Context) {
        let Some(message) = self.error.clone() else { return };
        let modal = egui::Modal::new(egui::Id::new("file_error")).show(ctx, |ui| {
            ui.set_max_width(360.0);
            ui.heading(tr!("error-title"));
            ui.label(message);
            ui.add_space(8.0);
            ui.button(tr!("error-ok")).clicked()
        });
        if modal.inner || modal.should_close() {
            self.error = None;
        }
    }

    fn update_title(&mut self, ctx: &egui::Context) {
        let title = self.window_title();
        if title != self.title {
            ctx.send_viewport_cmd(ViewportCommand::Title(title.clone()));
            self.title = title;
        }
    }

    fn about_window(&mut self, ctx: &egui::Context) {
        let mut open = self.show_about;
        egui::Window::new(tr!("menu-about"))
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .show(ctx, |ui| {
                let d = &self.diagnostics;
                ui.heading(tr!("app-name"));
                ui.label(tr!("about-tagline"));
                ui.label(tr!("about-version", version = d.app_version.clone()));
                ui.label(tr!("about-graphics", name = d.adapter.clone(), backend = d.backend.clone()));
                ui.label(tr!("about-license"));
                if ui.button(tr!("about-copy")).clicked() {
                    ctx.copy_text(d.to_text());
                    self.copied = true;
                }
                if self.copied {
                    ui.label(tr!("about-copied"));
                }
            });
        self.show_about = open;
    }
}

/// `path`, with ".odp" added unless it already ends in it ("skirt.v2" becomes "skirt.v2.odp").
fn with_project_extension(path: PathBuf) -> PathBuf {
    if path.extension().is_some_and(|e| e.eq_ignore_ascii_case(opendrape_io::EXTENSION)) {
        return path;
    }
    let mut name = path.into_os_string();
    name.push(".");
    name.push(opendrape_io::EXTENSION);
    name.into()
}

fn garment_label(g: Garment) -> String {
    match g {
        Garment::Skirt => tr!("garment-skirt"),
        Garment::BodiceProxy => tr!("garment-bodice-proxy"),
    }
}

fn choice_label(choice: GpuChoice) -> String {
    match choice {
        GpuChoice::Auto => tr!("graphics-auto"),
        GpuChoice::Dx12 => tr!("graphics-dx12"),
        GpuChoice::Vulkan => tr!("graphics-vulkan"),
        GpuChoice::Metal => tr!("graphics-metal"),
        GpuChoice::Gl => tr!("graphics-gl"),
        GpuChoice::Software => tr!("graphics-software"),
    }
}

impl eframe::App for OpenDrapeApp {
    fn ui(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        self.guard_close(&ctx);
        let shortcut = self.file_shortcut(&ctx);
        let menu = egui::Panel::top("menu_bar").show(ui, |ui| self.menu_bar(ui)).inner;
        if let Some(action) = menu.or(shortcut) {
            self.file_action(action, frame, &ctx);
        }
        self.about_window(&ctx);
        let dt = ui.input(|i| i.unstable_dt).max(1e-3);
        self.fps = if self.fps == 0.0 { 1.0 / dt } else { 0.9 * self.fps + 0.1 / dt };
        let width = ui.available_width();
        egui::Panel::left("view_3d")
            .resizable(true)
            .default_size(width * 0.42)
            .size_range(240.0..=(width - 360.0).max(240.0))
            .show(ui, |ui| self.view_3d(ui, frame));
        egui::CentralPanel::default().show(ui, |ui| self.editor.ui(ui));
        self.unsaved_changes_modal(frame, &ctx);
        self.error_modal(&ctx);
        self.poll_dialog(frame, &ctx);
        self.update_title(&ctx);
        self.confirm_first_frame(&ctx);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn project_extension_is_added_once() {
        assert_eq!(with_project_extension("a/skirt".into()), PathBuf::from("a/skirt.odp"));
        assert_eq!(with_project_extension("a/skirt.ODP".into()), PathBuf::from("a/skirt.ODP"));
        assert_eq!(with_project_extension("a/skirt.v2".into()), PathBuf::from("a/skirt.v2.odp"));
    }
}
```

Notes for the implementer:
- `cargo fmt` reflows the long lines. Keep the M1 behaviour of `confirm_first_frame`, the about box and graphics restart exactly as before; they are only moved.
- The graphics-mode switch now goes through `FileAction::Quit`, so unsaved work is protected. `choosing_a_graphics_mode_saves_it_and_asks_for_a_restart` calls `request_graphics_change` directly and is unaffected.

- [ ] **Step 5: Run everything**

Run: `cargo nextest run --workspace`
Expected: all pass, including:
- the M0/M1 UI tests and the 10 new file-flow UI tests;
- 35 editor tests;
- the core, geom and io unit tests;
- `project_extension_is_added_once`.

- [ ] **Step 6: Commit**

```bash
cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings && cargo deny check
git add crates/app
git commit -m "feat(app): pattern window beside the 3D view, File/Edit menus, save/open with unsaved-changes guard

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 9: Checklist, docs, final review, release

**Files:**
- Create: `docs/testing/M2a-checklist.md`
- Modify: `README.md`, `docs/specs/2026-10-09-opendrape-design.md`

- [ ] **Step 1: Tester checklist** (`docs/testing/M2a-checklist.md`)

```markdown
# OpenDrape M2a: what to try

This build adds the pattern window. The 3D body stays on the left; on the right is a grid,
the pattern table, where you draw pattern pieces. Pieces don't go onto the body yet:
sewing them on comes in M3.

## Check these

- [ ] OpenDrape opens with the 3D view on the left and the pattern table on the right.
      Drag the divider between them: both sides resize.
- [ ] Press **H** (or click **Pen (H)**). Click four corners, then click the first point
      again: the piece fills in and is called "Piece 1".
- [ ] With the pen, click a point, type **50** and press **Enter**: the next point is
      exactly 50 cm away, towards your mouse. Type a number, press **Tab**, type **90**,
      press **Enter**: that edge goes straight up.
- [ ] While drawing, press and drag instead of clicking: that point becomes a smooth curve.
- [ ] **Cmd+Z** while drawing removes only the last point. **Esc** cancels the piece.
- [ ] Press **S** and drag: a rectangle. Or click once and type its width, **Tab**, its
      height, **Enter**.
- [ ] Press **Z** (Edit). Drag a corner, an edge, and the middle of a piece: each moves.
      Select a curved piece and drag one of its blue handles to change the curve.
- [ ] Click an edge and type a new length on the right (for example **45**, **Enter**).
      Choose whether its start or end point stays put.
- [ ] Press **X** and click an edge: a new point appears there. Select a point and press
      **Delete** (the Backspace key on a Mac keyboard) to remove it.
- [ ] Switch between **cm** and **inch** at the top of the pattern window: the numbers
      change, the pattern doesn't.
- [ ] **Draft a skirt panel from scratch.** For example: a 45 cm waist, 60 cm side seams
      and a curved hem. Then **File → Save As…**, quit OpenDrape, reopen it and use
      **File → Open…**: everything is back.
- [ ] Undo 10 steps with **Cmd+Z**, then redo them with **Shift+Cmd+Z**.
- [ ] Change something and close the window: OpenDrape asks whether to save. **Cancel**
      keeps you working.
- [ ] The 3D skirt still drapes as it did in M1.

If anything looks wrong, take a screenshot and copy Help → About → Copy diagnostics.
```

- [ ] **Step 2: README and spec status**

README status line becomes:
> **Status:** early development (milestone M2a: pattern editor; draw pieces with exact measurements, then save and open them). Next: seam allowance, notches and mirrored pieces (M2b), then sewing pieces onto the body (M3).

Spec `## Status` gains:
> - **M2a Pattern editor core: complete (<date>).** Pen, rectangle, edit and add-point tools; typed lengths and angles while drawing; a properties panel for lengths, positions, names and grain; cm/inch; undo/redo (200 steps, one per drag); `.odp` save/open with version checks; unsaved-changes guard. M2b (seam allowance, notches, internal lines, symmetric and mirrored pieces) is next.

- [ ] **Step 3: Full local CI**

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo nextest run --workspace
cargo deny check
cargo about generate -m crates/app/Cargo.toml about.hbs -o target/licenses/THIRD_PARTY_LICENSES.html
```
Expected: all green. The `cargo about` run proves the release packaging accepts the new dependencies' licences.

- [ ] **Step 4: Final review**

Dispatch a fresh reviewer over `git diff main...m2a-pattern-editor` with this plan's **Review Focus** list and the global constraints. Fix every finding with a test. Note any deliberate deferrals in the ledger.

- [ ] **Step 5: Commit, merge, and ask before publishing**

```bash
git add docs README.md
git commit -m "docs: M2a tester checklist and status

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
git checkout main && git merge --ff-only m2a-pattern-editor
```
Then **ask the user before pushing**: a push to `main` rebuilds the nightly downloads. Once they agree:
- push;
- watch CI and the release workflow;
- give them the release page https://github.com/pujarivivek/opendrape/releases/tag/nightly and `docs/testing/M2a-checklist.md`;
- update the `opendrape-project` memory to say M2a is done and M2b is next.

**Do not run the app on the user's Mac** (not even `--smoke-test`) unless they agree first.
