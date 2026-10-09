# OpenDrape M2b (Pattern Details) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** a student's pattern pieces carry everything a paper pattern does:
- seam allowance, with hems;
- notches;
- internal lines;
- cut-on-fold pieces;
- mirrored left/right pairs.

They are saved in format version 2, and the M2a polish (labels, Tab, Dock-quit recovery, huge files) is fixed.

**Architecture:** "store once, derive the rest".
- **`opendrape-core`** stores only what the student drew:
  - per-edge allowance and hem flags;
  - notches as (edge, distance);
  - internal lines;
  - a fold edge index;
  - a twin record (id, name, offset).
- **`opendrape-geom`** derives everything else:
  - `geom::shapes(&Project)` turns each stored piece into concrete `Shape`s: the full unfolded piece, or the reflected twin. They map back to stored indices and coordinates.
  - `geom::cut_line` computes the cut line. It offsets every flattened segment, joins neighbours by miter, trim, squared end or mirrored hem corner, and cleans loops with `i_overlay`.
- **The app's editor** draws, hit-tests and edits those shapes, and caches the expensive parts per shape.

**Tech Stack:** as M2a (Rust 1.99, eframe/egui/egui_kittest 0.36.2, kurbo 0.13.1, earcut 0.4.11, zip 8.6.0), plus **i_overlay 9.0.1** (MIT OR Apache-2.0) for cut-line cleanup.

**Spec:** `docs/superpowers/specs/2026-10-09-m2b-pattern-details-design.md` (approved by the user on 2026-10-09). Main spec: `docs/specs/2026-10-09-opendrape-design.md`.

**Evidence:** the cut-line algorithm was prototyped headlessly on 2026-10-09 in `scratchpad/m2b-probe` (8 tests). It covers:
- rectangles in both winding directions;
- an A-line hem whose mirrored corner was checked against a hand calculation (cut x = −6.088 at the hem);
- an L-shape area of 62,400 mm²;
- a zero-width (fold) edge;
- an inward curve tighter than the allowance, bridged straight across;
- a concave corner between different widths;
- a squared acute tip;
- speed: 200 curved edges in 4 ms (release).

A first "union of strips" design failed the tight-curve test (7.2 mm from the stitching line), so the plan uses the raw-offset design.

## Global Constraints

Carried from M0–M2a:
- Pinned toolchain; the egui family is `=0.36.2`; GPL-3.0-or-later; `cargo deny check` and `cargo about generate -m crates/app/Cargo.toml about.hbs -o target/licenses/THIRD_PARTY_LICENSES.html` pass.
- Never use the name "CLO" in the product.
- Every user-visible string goes through `tr!` (`crates/app/i18n/en-US/opendrape.ftl`).
- **Never launch the GUI app, `--smoke-test`, or anything that opens a window or dialog on the user's Mac. Use headless unit and egui_kittest tests only.**
- Commit messages end with exactly `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`.
- Work on branch `m2b-pattern-details`, created from `main`. Never push.
- Pattern coordinates are f64 millimetres with y up. Outline edge `i` runs from vertex `i` to vertex `(i+1) % n`.
- Every user change is one undo step. `Document` refuses any change that makes `Project::check()` fail; call sites show `notice-refused` via `note_if_refused()`.
- Keyboard ownership rules from M2a are unchanged:
  - shortcuts and tool keys act only when no text field has focus, the number box is closed, and no modal is open;
  - redo is checked before undo, and Save As before Save.

New in M2b (from the spec):
- **Seam allowance:**
  - Default 10 mm per piece (`DEFAULT_ALLOWANCE_MM`). A hem edge gets 30 mm (`HEM_ALLOWANCE_MM`) unless the edge has its own value. The allowed range is 0–100 mm (`MAX_ALLOWANCE_MM`).
  - The cut line is always derived from the stitching line and is never stored.
  - A fold edge has no allowance: it isn't part of the unfolded outline.
- **Corners:** mitered when the miter is at most 2.5 widths from the corner, otherwise squared. At the ends of a hem, the neighbouring seam's cut line is mirrored about the hem's stitching line.
- **Notches:**
  - Stored as (edge, distance in mm from the edge's start along the stitching line), with marks 1–3 at 3 mm spacing and style slit or V.
  - Drawn on the cut line pointing inwards, `min(5 mm, 0.6 × allowance)` deep; with no allowance, on the stitching line, 5 mm deep.
  - Not allowed on a fold edge.
- **Internal lines:**
  - Open lines have ≥ 2 points; closed lines have ≥ 3.
  - Kind Marking (default) or Cut-out (closed only).
  - Points must be inside the piece or on its outline.
- **Fold:** only on a straight edge; every point of the piece lies on one side of the fold line; a piece can't be both folded and paired.
- **Twin:** its shape is the stored piece reflected x → −x and then moved by `offset`. Name and position are separate from the master; everything else is shared. Dragging the master moves only the master.
- **Files:**
  - `SCHEMA_VERSION` = 2.
  - v1 files load with every new field defaulted (allowance 10 mm on every edge).
  - `crates/io/tests/fixtures/v1/project.json` must never be edited.
- **Size limits:** the M2a limits count internal-line points. A twin counts toward the piece and point limits as if it were a stored piece.

## Review Focus

1. **Undo, redo or delete removing what a selection points at**, e.g. a notch, internal line or twin selected, then Cmd+Z removes it. The panel, painting and Delete must not panic. Tests:
   - Task 6: `selection_of_a_removed_notch_is_dropped`;
   - Task 7: `selection_of_a_removed_line_is_dropped`;
   - Task 5: `undo_of_make_pair_drops_the_twin_selection`.
2. **Editing a vertex next to a notch or the fold.** Removing or splitting vertices must keep notches on the right edge at the right distance, and clear a fold whose end vertex is removed. Tests:
   - Task 1: `notches_follow_split_and_removed_edges`, `removing_a_fold_end_clears_the_fold`.
3. **Shapes that make the cut line degenerate:**
   - an inward curve tighter than the allowance;
   - an acute tip;
   - a zero allowance;
   - a twin, whose winding is reversed.
   The result must never come out inside the stitching line, and must not spike. Tests: Task 3 cut-line tests.
4. **Hostile or old files:**
   - a v1 file with no new fields;
   - a v2 file with a notch on a missing edge, a fold on a curved edge, or a twin on a folded piece;
   - a huge but within-limits piece.
   Each must load with defaults, be refused with a clear error, or draw without stalling. Tests:
   - Task 1: `format_v1_still_opens`, `format_v2_still_opens`, `refuses_invalid_v2_details`;
   - Task 4: `huge_shapes_are_cached_and_capped`.
5. **Quitting from the Dock with unsaved work.** The recovery copy is written exactly when the student didn't choose "Don't save", and is offered once on the next start. Tests: Task 9.

---

## File Structure

```
Cargo.toml                                   + i_overlay workspace dep
crates/core/src/piece.rs                     + EdgeProps, Notch, NotchStyle, InternalLine, LineKind, Twin, Side; new Piece fields and methods
crates/core/src/project.rs                   + SCHEMA_VERSION 2, owner/name_of/add_twin/break_twin, twin-aware remove_piece and check
crates/core/src/lib.rs                       exports
crates/io/src/lib.rs                         v1 → v2 migration
crates/io/tests/fixtures.rs                  v1 expectations with defaults, + v2 fixture test
crates/io/tests/fixtures/v2/project.json     new frozen fixture
crates/geom/src/lib.rs                       split_edge/remove_vertex keep notches; + module declarations
crates/geom/src/shapes.rs                    Shape, ShapeKind, shapes(), unfolded()
crates/geom/src/allowance.rs                 cut_line()
crates/geom/src/marks.rs                     notch geometry, internal-line geometry, label anchors
crates/app/src/editor/cache.rs               ShapeCache: shapes + cached fill, cut line, notch marks, lines
crates/app/src/editor/paint.rs               draw shapes, allowance, notches, lines, fold, twin badge, labels outside
crates/app/src/editor/canvas.rs              hit-testing and dragging over shapes (twins mapped back)
crates/app/src/editor/notch_tool.rs          Notch tool (N)
crates/app/src/editor/line_tool.rs           Internal line tool (L)
crates/app/src/editor/panel.rs               allowance, hem, fold, pair, notch and line sections
crates/app/src/editor/mod.rs                 Tool::Notch/Line, Selection::Notch/Line, show_allowance
crates/app/src/editor/length_box.rs          one-field mode (notch distance), Tab cycling
crates/app/src/recovery.rs                   recovery copy write/read/discard
crates/app/src/app.rs                        on_exit writes recovery; startup Restore/Discard modal
crates/app/src/main.rs                       Startup.recovery path
crates/app/tests/editor.rs, ui.rs            new UI tests
docs/testing/M2b-checklist.md, README.md, docs/specs/...  docs
```

All commands assume `source ~/.cargo/env` and the repo root. Before Task 1: `git checkout -b m2b-pattern-details`.

---
### Task 1: Pattern model and file format version 2

**Files:**
- Modify: `crates/core/src/{piece.rs,project.rs,lib.rs}`, `crates/io/src/lib.rs`, `crates/io/tests/fixtures.rs`, `crates/geom/src/lib.rs` (callers of the changed split/remove API), `crates/app/src/editor/canvas.rs` (one caller)
- Create: `crates/io/tests/fixtures/v2/project.json`

**Interfaces:**
- Produces (core):
  - **Consts:** `DEFAULT_ALLOWANCE_MM = 10.0`, `HEM_ALLOWANCE_MM = 30.0`, `MAX_ALLOWANCE_MM = 100.0`.
  - **New types:**
    - `EdgeProps { allowance: Option<f64>, hem: bool }` (Default).
    - `NotchStyle { Slit, V }` and `Notch { edge: usize, distance: f64, marks: u8, style: NotchStyle }`, with `Notch::new(edge, distance)`.
    - `LineKind { Marking, Cutout }` and `InternalLine { vertices: Vec<Vertex>, edges: Vec<Edge>, closed: bool, kind: LineKind }`. Methods: `open(&[Point2])`, `polygon(&[Point2])`, `edge_count()`, `edge_ends(i)`, `points()`, `translate(d)`, `mapped(f)`.
    - `Twin { id: PieceId, name: String, offset: Point2 }`.
    - `Side { Master, Twin }`.
  - **New `Piece` fields:** `allowance: f64`, `edge_props: Vec<EdgeProps>`, `notches: Vec<Notch>`, `lines: Vec<InternalLine>`, `fold: Option<usize>`, `twin: Option<Twin>`. All are `#[serde(default)]`, except `allowance`, whose default is 10.
  - **New `Piece` methods:**
    - `edge_allowance(i) -> f64`
    - `point_count() -> usize` (outline plus internal-line points)
    - `reflected(offset) -> Piece` (x → offset.x − x, y → y + offset.y)
    - `twin_shape() -> Option<Piece>`
    - `split_edge_at(i, vertex, first, second, first_len) -> usize`
  - **Changed `Piece` methods:**
    - `remove_vertex(i, prev_len: f64) -> bool`: the merged edge keeps the first edge's properties, notches move with their edge, and a fold losing an end is cleared.
    - `translate(d)` also moves internal lines and keeps the twin where it is.
  - **`Project`:**
    - `SCHEMA_VERSION = 2`.
    - New methods: `owner(id) -> Option<(&Piece, Side)>`, `owner_mut(id) -> Option<(&mut Piece, Side)>`, `name_of(id) -> Option<&str>`, `add_twin(master, name, offset) -> Option<PieceId>`, `break_twin(master) -> Option<PieceId>`.
    - `remove_piece(id)` works on twins: removing a master that has a twin turns the twin into an ordinary piece first.
  - **New `ModelError` variants:** `BadAllowance(PieceId)`, `BadNotch(PieceId)`, `BadLine(PieceId)`, `BadFold(PieceId)`.
- Produces (geom):
  - `split_edge` is unchanged on the outside, refuses the fold edge, and keeps notches.
  - New `geom::remove_vertex(&mut Piece, i) -> bool`.
- Produces (io): v1 files load with `edge_props` filled to one default per edge and `schema_version` set to 2.

- [ ] **Step 1: Write the failing core tests** (append to `crates/core/src/piece.rs` `mod tests`)

```rust
    #[test]
    fn new_pieces_get_a_one_centimetre_allowance() {
        let mut s = square();
        assert_eq!(s.allowance, DEFAULT_ALLOWANCE_MM);
        assert_eq!(s.edge_props.len(), 4);
        assert_eq!(s.edge_allowance(0), 10.0);
        s.edge_props[0].hem = true;
        assert_eq!(s.edge_allowance(0), HEM_ALLOWANCE_MM);
        s.edge_props[0].allowance = Some(15.0);
        assert_eq!(s.edge_allowance(0), 15.0, "an edge's own value wins over the hem's");
        s.allowance = 6.0;
        assert_eq!(s.edge_allowance(1), 6.0);
    }

    #[test]
    fn notches_follow_split_and_removed_edges() {
        let mut s = square(); // edges: 0 (0,0)→(100,0), 1 (100,0)→(100,100), 2, 3
        s.notches = vec![Notch::new(0, 10.0), Notch::new(0, 60.0), Notch::new(1, 40.0)];
        s.edge_props[0].hem = true;
        let v = s.split_edge_at(0, Vertex::corner(p(25.0, 0.0)), Edge::Line, Edge::Line, 25.0);
        assert_eq!(v, 1);
        assert_eq!(s.len(), 5);
        assert_eq!(s.notches, vec![Notch::new(0, 10.0), Notch::new(1, 35.0), Notch::new(2, 40.0)]);
        assert!(s.edge_props[1].hem, "both halves of a hem stay a hem");
        assert!(s.remove_vertex(1, 25.0));
        assert_eq!(s.notches, vec![Notch::new(0, 10.0), Notch::new(0, 60.0), Notch::new(1, 40.0)]);
        assert_eq!(s.edge_props.len(), 4);
        // Removing vertex 0 merges the last edge with edge 0, which becomes the last edge.
        assert!(s.remove_vertex(0, 100.0));
        assert_eq!(s.notches, vec![Notch::new(2, 110.0), Notch::new(2, 160.0), Notch::new(0, 40.0)]);
    }

    #[test]
    fn removing_a_fold_end_clears_the_fold() {
        let mut s = square();
        s.vertices.push(Vertex::corner(p(-20.0, 50.0)));
        s.edges.push(Edge::Line);
        s.edge_props.push(EdgeProps::default());
        s.fold = Some(1);
        assert!(s.remove_vertex(4, 40.0)); // not an end of the fold edge
        assert_eq!(s.fold, Some(1));
        assert!(s.remove_vertex(2, 100.0)); // (100,100) is the far end of edge 1
        assert_eq!(s.fold, None);
    }

    #[test]
    fn a_fold_must_be_straight_one_sided_and_unpaired() {
        let mut s = square();
        s.fold = Some(3); // the left edge (0,100)→(0,0): everything is on its right
        assert_eq!(s.check(), Ok(()));
        s.vertices[2].pos = p(-50.0, 100.0); // now the piece crosses the fold line
        assert_eq!(s.check(), Err(ModelError::BadFold(PieceId(1))));
        let mut curved = square();
        curved.set_curved(3, true);
        curved.fold = Some(3);
        assert_eq!(curved.check(), Err(ModelError::BadFold(PieceId(1))));
        let mut notched = square();
        notched.fold = Some(3);
        notched.notches = vec![Notch::new(3, 10.0)];
        assert_eq!(notched.check(), Err(ModelError::BadFold(PieceId(1))));
        let mut paired = square();
        paired.fold = Some(3);
        paired.twin = Some(Twin { id: PieceId(9), name: "B".into(), offset: p(300.0, 0.0) });
        assert_eq!(paired.check(), Err(ModelError::BadFold(PieceId(1))));
    }

    #[test]
    fn check_rejects_bad_details() {
        let mut a = square();
        a.allowance = -1.0;
        assert_eq!(a.check(), Err(ModelError::BadAllowance(PieceId(1))));
        let mut e = square();
        e.edge_props[2].allowance = Some(MAX_ALLOWANCE_MM + 1.0);
        assert_eq!(e.check(), Err(ModelError::BadAllowance(PieceId(1))));
        let mut props = square();
        props.edge_props.pop();
        assert_eq!(props.check(), Err(ModelError::EdgeCountMismatch(PieceId(1))));
        for bad in [Notch::new(4, 1.0), Notch::new(0, -1.0), Notch::new(0, f64::NAN), Notch { marks: 4, ..Notch::new(0, 1.0) }] {
            let mut n = square();
            n.notches = vec![bad];
            assert_eq!(n.check(), Err(ModelError::BadNotch(PieceId(1))), "{bad:?}");
        }
        let mut short = square();
        short.lines = vec![InternalLine::open(&[p(10.0, 10.0)])];
        assert_eq!(short.check(), Err(ModelError::BadLine(PieceId(1))));
        let mut closed_two = square();
        closed_two.lines = vec![InternalLine { closed: true, ..InternalLine::open(&[p(10.0, 10.0), p(20.0, 20.0)]) }];
        assert_eq!(closed_two.check(), Err(ModelError::BadLine(PieceId(1))));
        let mut open_cutout = square();
        open_cutout.lines = vec![InternalLine { kind: LineKind::Cutout, ..InternalLine::open(&[p(10.0, 10.0), p(20.0, 20.0)]) }];
        assert_eq!(open_cutout.check(), Err(ModelError::BadLine(PieceId(1))));
        let mut far_line = square();
        far_line.lines = vec![InternalLine::open(&[p(10.0, 10.0), p(2e6, 20.0)])];
        assert_eq!(far_line.check(), Err(ModelError::BadLine(PieceId(1))));
        let mut ok = square();
        ok.lines = vec![InternalLine::polygon(&[p(10.0, 10.0), p(30.0, 10.0), p(20.0, 30.0)])];
        assert_eq!(ok.check(), Ok(()));
    }

    #[test]
    fn internal_line_points_count_toward_the_limit() {
        let mut s = square();
        let pts: Vec<Point2> = (0..MAX_VERTICES_PER_PIECE - 4).map(|k| p(1.0 + k as f64 * 0.01, 50.0)).collect();
        s.lines = vec![InternalLine::open(&pts)];
        assert_eq!(s.point_count(), MAX_VERTICES_PER_PIECE);
        assert_eq!(s.check(), Ok(()));
        s.lines[0].vertices.push(Vertex::corner(p(90.0, 50.0)));
        s.lines[0].edges.push(Edge::Line);
        assert_eq!(s.check(), Err(ModelError::TooManyPoints(PieceId(1))));
    }

    #[test]
    fn a_twin_is_the_mirror_image_moved_by_its_offset() {
        let mut s = square();
        s.grain_deg = 45.0;
        s.set_curved(0, true);
        s.notches = vec![Notch::new(1, 30.0)];
        s.lines = vec![InternalLine::open(&[p(20.0, 20.0), p(40.0, 30.0)])];
        s.twin = Some(Twin { id: PieceId(2), name: "Front (mirror)".into(), offset: p(300.0, 10.0) });
        let t = s.twin_shape().unwrap();
        assert_eq!((t.id, t.name.as_str()), (PieceId(2), "Front (mirror)"));
        assert_eq!(t.vertices[1].pos, p(200.0, 10.0)); // (100,0) → (300-100, 0+10)
        let Edge::Curve { c1, .. } = t.edges[0] else { panic!("still curved") };
        close(c1, p(300.0 - 100.0 / 3.0, 10.0));
        assert_eq!(t.grain_deg, 135.0);
        assert_eq!(t.notches, s.notches, "edges keep their direction, so distances stay");
        assert_eq!(t.lines[0].vertices[1].pos, p(260.0, 40.0));
        assert_eq!((t.twin.clone(), t.fold), (None, None));
    }

    #[test]
    fn moving_a_paired_piece_leaves_its_twin_in_place() {
        let mut s = square();
        s.lines = vec![InternalLine::open(&[p(20.0, 20.0), p(40.0, 30.0)])];
        s.twin = Some(Twin { id: PieceId(2), name: "B".into(), offset: p(300.0, 0.0) });
        let before = s.twin_shape().unwrap();
        s.translate(p(15.0, -7.0));
        assert_eq!(s.lines[0].vertices[0].pos, p(35.0, 13.0));
        let after = s.twin_shape().unwrap();
        for (a, b) in before.vertices.iter().zip(&after.vertices) {
            close(a.pos, b.pos);
        }
    }

    #[test]
    fn old_files_get_defaults_for_the_new_fields() {
        let json = r#"{"id":1,"name":"A","vertices":[{"pos":{"x":0,"y":0}},{"pos":{"x":1,"y":0}},{"pos":{"x":0,"y":1}}],"edges":[{"type":"line"},{"type":"line"},{"type":"line"}]}"#;
        let piece: Piece = serde_json::from_str(json).unwrap();
        assert_eq!(piece.allowance, DEFAULT_ALLOWANCE_MM);
        assert!(piece.edge_props.is_empty() && piece.notches.is_empty() && piece.lines.is_empty());
        assert_eq!((piece.fold, piece.twin), (None, None));
    }
```

Also update the existing tests whose API changed:
- `removing_a_vertex_joins_its_edges`: after `s.edges.push(Edge::Line);` add `s.edge_props.push(EdgeProps::default());`. Change `s.remove_vertex(4)` to `s.remove_vertex(4, 50.0)`.
- `a_piece_keeps_at_least_three_vertices`: `t.remove_vertex(1)` becomes `t.remove_vertex(1, 10.0)`.
- `removing_a_vertex_between_curves_keeps_the_outer_handles`: `s.remove_vertex(1)` becomes `s.remove_vertex(1, 100.0)`.

Append to `crates/core/src/project.rs` `mod tests`:

```rust
    #[test]
    fn twins_get_ids_and_names_and_can_be_broken_off() {
        let mut pr = Project::new();
        let a = pr.add_piece(tri());
        let t = pr.add_twin(a, "T (mirror)".into(), Point2::new(50.0, 0.0)).unwrap();
        assert_eq!(t, PieceId(2));
        assert_eq!(pr.add_twin(a, "again".into(), Point2::new(0.0, 0.0)), None, "one twin each");
        assert!(matches!(pr.owner(t), Some((p, Side::Twin)) if p.id == a));
        assert!(matches!(pr.owner(a), Some((_, Side::Master))));
        assert_eq!(pr.name_of(t), Some("T (mirror)"));
        assert!(pr.piece(t).is_none(), "piece() finds stored pieces only");
        assert_eq!(pr.check(), Ok(()));
        assert_eq!(pr.break_twin(a), Some(t));
        let broken = pr.piece(t).unwrap();
        assert_eq!(broken.vertices[1].pos, Point2::new(40.0, 0.0)); // (10,0) reflected, +50
        assert!(pr.piece(a).unwrap().twin.is_none());
        assert_eq!(pr.check(), Ok(()));
    }

    #[test]
    fn removing_a_paired_piece_keeps_its_twin() {
        let mut pr = Project::new();
        let a = pr.add_piece(tri());
        let t = pr.add_twin(a, "T".into(), Point2::new(50.0, 0.0)).unwrap();
        assert!(pr.remove_piece(a).is_some());
        assert!(pr.piece(t).is_some(), "the twin becomes an ordinary piece");
        let b = pr.add_piece(tri());
        let u = pr.add_twin(b, "U".into(), Point2::new(50.0, 0.0)).unwrap();
        assert!(pr.remove_piece(u).is_some());
        assert!(pr.piece(b).unwrap().twin.is_none() && pr.owner(u).is_none());
    }

    #[test]
    fn check_counts_twins_and_refuses_clashing_ids() {
        let mut pr = Project::new();
        let a = pr.add_piece(tri());
        pr.add_twin(a, "T".into(), Point2::new(50.0, 0.0));
        let mut clash = pr.clone();
        clash.pieces[0].twin.as_mut().unwrap().id = a;
        assert_eq!(clash.check(), Err(ModelError::DuplicateId(a)));
        let mut ahead = pr.clone();
        ahead.pieces[0].twin.as_mut().unwrap().id = PieceId(99);
        assert_eq!(ahead.check(), Err(ModelError::IdCounterBehind(PieceId(99))));
        let mut folded = pr.clone();
        folded.pieces[0].fold = Some(1);
        assert_eq!(folded.check(), Err(ModelError::BadFold(a)));
        assert_eq!(SCHEMA_VERSION, 2);
    }
```
(Add `use crate::Side;` to that test module.)

Run: `cargo nextest run -p opendrape-core`
Expected: compile errors (the new types don't exist yet).

- [ ] **Step 2: Implement the core model** (`crates/core/src/piece.rs`)

Add after `HandleEnd`:

```rust
/// Seam allowance a new piece gets (mm).
pub const DEFAULT_ALLOWANCE_MM: f64 = 10.0;
/// Allowance of an edge marked as a hem, unless the edge has its own (mm).
pub const HEM_ALLOWANCE_MM: f64 = 30.0;
/// Widest allowance a piece or edge may have (mm).
pub const MAX_ALLOWANCE_MM: f64 = 100.0;

/// Sewing properties of one outline edge.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct EdgeProps {
    /// This edge's own seam allowance (mm); `None` uses the hem's or the piece's.
    #[serde(default)]
    pub allowance: Option<f64>,
    /// A hem: 3 cm allowance unless the edge has its own, and corners that fold up flat.
    #[serde(default)]
    pub hem: bool,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum NotchStyle {
    /// A straight cut into the allowance.
    #[default]
    Slit,
    /// A small V-shaped cut.
    V,
}

/// A notch: a short mark on an edge showing where pieces line up when they are sewn together.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Notch {
    /// The outline edge it is on.
    pub edge: usize,
    /// Distance (mm) along the stitching line from the edge's start point.
    pub distance: f64,
    /// How many marks, 3 mm apart: 1 (front), 2 (back) or 3.
    #[serde(default = "one_mark")]
    pub marks: u8,
    #[serde(default)]
    pub style: NotchStyle,
}

fn one_mark() -> u8 {
    1
}

impl Notch {
    /// A single slit notch.
    pub fn new(edge: usize, distance: f64) -> Self {
        Self { edge, distance, marks: 1, style: NotchStyle::Slit }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LineKind {
    /// Drawn on the fabric: placement, centre front, button, fold or press lines.
    #[default]
    Marking,
    /// A hole that is cut out (closed lines only).
    Cutout,
}

/// A line drawn inside a piece.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct InternalLine {
    pub vertices: Vec<Vertex>,
    /// Edge `i` runs from vertex `i` to vertex `i + 1`; a closed line has one more edge, from
    /// its last vertex back to its first.
    pub edges: Vec<Edge>,
    #[serde(default)]
    pub closed: bool,
    #[serde(default)]
    pub kind: LineKind,
}

impl InternalLine {
    /// An open line of straight edges through `points`.
    pub fn open(points: &[Point2]) -> Self {
        Self {
            vertices: points.iter().map(|&p| Vertex::corner(p)).collect(),
            edges: vec![Edge::Line; points.len().saturating_sub(1)],
            closed: false,
            kind: LineKind::Marking,
        }
    }
    /// A closed shape of straight edges through `points`.
    pub fn polygon(points: &[Point2]) -> Self {
        Self { edges: vec![Edge::Line; points.len()], closed: true, ..Self::open(points) }
    }
    /// How many edges a line with this many vertices has.
    pub fn edge_count(&self) -> usize {
        if self.closed { self.vertices.len() } else { self.vertices.len().saturating_sub(1) }
    }
    /// Start and end point of edge `i`.
    pub fn edge_ends(&self, i: usize) -> (Point2, Point2) {
        (self.vertices[i].pos, self.vertices[(i + 1) % self.vertices.len()].pos)
    }
    /// Every vertex and control point.
    pub fn points(&self) -> impl Iterator<Item = Point2> + '_ {
        self.vertices.iter().map(|v| v.pos).chain(self.edges.iter().flat_map(|e| match *e {
            Edge::Line => Vec::new(),
            Edge::Curve { c1, c2 } => vec![c1, c2],
        }))
    }
    pub fn translate(&mut self, d: Point2) {
        *self = self.mapped(|p| p + d);
    }
    /// The line with every point (vertices and control points) passed through `f`.
    pub fn mapped(&self, f: impl Fn(Point2) -> Point2) -> Self {
        Self {
            vertices: self.vertices.iter().map(|v| Vertex { pos: f(v.pos), kind: v.kind }).collect(),
            edges: self.edges.iter().map(|e| map_edge(*e, &f)).collect(),
            closed: self.closed,
            kind: self.kind,
        }
    }
    /// The right number of edges for its vertices (at least 2, or 3 when closed), only a
    /// closed line may be a cut-out, and every number finite and within range.
    fn is_valid(&self) -> bool {
        let enough = self.vertices.len() >= if self.closed { 3 } else { 2 };
        enough
            && self.edges.len() == self.edge_count()
            && (self.closed || self.kind == LineKind::Marking)
            && self.points().all(|p| p.is_finite() && within_range(p))
    }
}

fn map_edge(e: Edge, f: &impl Fn(Point2) -> Point2) -> Edge {
    match e {
        Edge::Line => Edge::Line,
        Edge::Curve { c1, c2 } => Edge::Curve { c1: f(c1), c2: f(c2) },
    }
}

/// The mirror-image twin of a piece, for a left/right pair. Its shape is never stored: it is
/// the piece reflected left to right (x → −x) and then moved by `offset`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Twin {
    pub id: PieceId,
    pub name: String,
    pub offset: Point2,
}

/// Which member of a pair an id names.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Side {
    /// The stored piece itself.
    Master,
    /// Its mirror-image twin.
    Twin,
}
```

Add the new fields to `struct Piece`, after `grain_deg`:

```rust
    /// Seam allowance (mm) of every edge that has none of its own.
    #[serde(default = "default_allowance")]
    pub allowance: f64,
    /// One entry per edge: its own allowance and whether it is a hem.
    #[serde(default)]
    pub edge_props: Vec<EdgeProps>,
    #[serde(default)]
    pub notches: Vec<Notch>,
    #[serde(default)]
    pub lines: Vec<InternalLine>,
    /// The straight edge that is the fold line of a cut-on-fold piece: only half the piece is
    /// stored, and the other half is its mirror image across this edge.
    #[serde(default)]
    pub fold: Option<usize>,
    /// The mirror-image twin, for a left/right pair.
    #[serde(default)]
    pub twin: Option<Twin>,
```

and `fn default_allowance() -> f64 { DEFAULT_ALLOWANCE_MM }` next to `default_grain`.

`Piece::polygon` fills the new fields:

```rust
    pub fn polygon(id: PieceId, name: impl Into<String>, corners: &[Point2]) -> Self {
        Self {
            id,
            name: name.into(),
            vertices: corners.iter().map(|&c| Vertex::corner(c)).collect(),
            edges: vec![Edge::Line; corners.len()],
            grain_deg: default_grain(),
            allowance: DEFAULT_ALLOWANCE_MM,
            edge_props: vec![EdgeProps::default(); corners.len()],
            notches: Vec::new(),
            lines: Vec::new(),
            fold: None,
            twin: None,
        }
    }
```

Replace `translate`, `remove_vertex` and `check`, and add the new methods, inside `impl Piece`:

```rust
    /// Moves the piece, with its internal lines. A twin stays where it is.
    pub fn translate(&mut self, d: Point2) {
        for v in &mut self.vertices {
            v.pos = v.pos + d;
        }
        for e in &mut self.edges {
            *e = map_edge(*e, &|p| p + d);
        }
        for line in &mut self.lines {
            line.translate(d);
        }
        // The twin is the reflection (x → −x) moved by its offset, so it moved by (−d.x, d.y):
        // shift the offset back.
        if let Some(t) = &mut self.twin {
            t.offset = t.offset + Point2::new(d.x, -d.y);
        }
    }
    /// Seam allowance (mm) of edge `i`: its own, else the hem's for a hem, else the piece's.
    pub fn edge_allowance(&self, i: usize) -> f64 {
        let props = self.edge_props[i];
        props.allowance.unwrap_or(if props.hem { HEM_ALLOWANCE_MM } else { self.allowance })
    }
    /// Outline points plus internal-line points: what the size limits count.
    pub fn point_count(&self) -> usize {
        self.vertices.len() + self.lines.iter().map(|l| l.vertices.len()).sum::<usize>()
    }
    /// The piece reflected left to right (x → `offset.x` − x) and moved up by `offset.y`.
    /// Vertex order and edge directions stay, so edge indices and notch distances still apply;
    /// the outline's winding is reversed. The result has no fold and no twin.
    pub fn reflected(&self, offset: Point2) -> Piece {
        let f = |p: Point2| Point2::new(offset.x - p.x, p.y + offset.y);
        Piece {
            id: self.id,
            name: self.name.clone(),
            vertices: self.vertices.iter().map(|v| Vertex { pos: f(v.pos), kind: v.kind }).collect(),
            edges: self.edges.iter().map(|e| map_edge(*e, &f)).collect(),
            grain_deg: (180.0 - self.grain_deg).rem_euclid(360.0),
            allowance: self.allowance,
            edge_props: self.edge_props.clone(),
            notches: self.notches.clone(),
            lines: self.lines.iter().map(|l| l.mapped(f)).collect(),
            fold: None,
            twin: None,
        }
    }
    /// The twin as an ordinary piece (with the twin's id and name), if there is one.
    pub fn twin_shape(&self) -> Option<Piece> {
        let t = self.twin.as_ref()?;
        let mut shape = self.reflected(t.offset);
        shape.id = t.id;
        shape.name = t.name.clone();
        Some(shape)
    }
    /// Splits edge `i` at `vertex` into `first` (up to the new vertex) and `second`; `first_len`
    /// is the length (mm) of the first part. Returns the new vertex's index. The new edge gets
    /// the split edge's sewing properties, and notches past the split move onto it. Where to
    /// split and the curve control points come from `opendrape-geom`.
    pub fn split_edge_at(&mut self, i: usize, vertex: Vertex, first: Edge, second: Edge, first_len: f64) -> usize {
        self.edges[i] = first;
        self.vertices.insert(i + 1, vertex);
        self.edges.insert(i + 1, second);
        let props = self.edge_props[i];
        self.edge_props.insert(i + 1, props);
        for notch in &mut self.notches {
            if notch.edge > i {
                notch.edge += 1;
            } else if notch.edge == i && notch.distance > first_len {
                notch.edge = i + 1;
                notch.distance -= first_len;
            }
        }
        if let Some(f) = &mut self.fold
            && *f > i
        {
            *f += 1;
        }
        i + 1
    }
    /// Removes vertex `i`, joining its two edges into one; `prev_len` is the length (mm) of the
    /// edge before it, so its notches keep their places along the joined edge. The joined edge
    /// keeps that first edge's sewing properties. A fold that loses an end point is cleared.
    /// Refused (false) below 4 vertices.
    pub fn remove_vertex(&mut self, i: usize, prev_len: f64) -> bool {
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
        if self.fold.is_some_and(|f| f == prev || f == i) {
            self.fold = None;
        }
        for notch in &mut self.notches {
            if notch.edge == i {
                notch.edge = prev;
                notch.distance += prev_len;
            }
        }
        self.vertices.remove(i);
        self.edges.remove(i);
        self.edge_props.remove(i);
        let shift = |e: usize| if e > i { e - 1 } else { e };
        for notch in &mut self.notches {
            notch.edge = shift(notch.edge);
        }
        if let Some(f) = &mut self.fold {
            *f = shift(*f);
        }
        true
    }
    /// Everything [`Project::check`] needs of one piece:
    /// - 3 to [`MAX_VERTICES_PER_PIECE`] points (outline plus internal lines), one edge and one
    ///   set of edge properties per vertex, a name of at most [`MAX_NAME_CHARS`] characters;
    /// - only finite numbers, every point within [`MAX_COORDINATE_MM`] of the origin;
    /// - allowances within 0..=[`MAX_ALLOWANCE_MM`], notches on real edges with 1–3 marks,
    ///   well-formed internal lines;
    /// - a fold only on a straight, notch-free edge of an unpaired piece that lies entirely on
    ///   one side of it;
    /// - a twin that is itself a valid piece.
    pub fn check(&self) -> Result<(), ModelError> {
        if self.point_count() > MAX_VERTICES_PER_PIECE {
            return Err(ModelError::TooManyPoints(self.id));
        }
        if self.name.chars().count() > MAX_NAME_CHARS {
            return Err(ModelError::NameTooLong(self.id));
        }
        if self.vertices.len() < 3 {
            return Err(ModelError::TooFewVertices(self.id));
        }
        if self.edges.len() != self.vertices.len() || self.edge_props.len() != self.vertices.len() {
            return Err(ModelError::EdgeCountMismatch(self.id));
        }
        let finite = self.grain_deg.is_finite()
            && self.vertices.iter().all(|v| v.pos.is_finite())
            && self.edges.iter().all(|e| match e {
                Edge::Line => true,
                Edge::Curve { c1, c2 } => c1.is_finite() && c2.is_finite(),
            });
        if !finite {
            return Err(ModelError::NotFinite(self.id));
        }
        let in_range = self.vertices.iter().all(|v| within_range(v.pos))
            && self.edges.iter().all(|e| match e {
                Edge::Line => true,
                Edge::Curve { c1, c2 } => within_range(*c1) && within_range(*c2),
            });
        if !in_range {
            return Err(ModelError::OutOfRange(self.id));
        }
        let allowance_ok = |w: f64| w.is_finite() && (0.0..=MAX_ALLOWANCE_MM).contains(&w);
        if !allowance_ok(self.allowance)
            || !self.edge_props.iter().all(|e| e.allowance.is_none_or(allowance_ok))
        {
            return Err(ModelError::BadAllowance(self.id));
        }
        let notches_ok = self.notches.iter().all(|n| {
            n.edge < self.len() && n.distance.is_finite() && n.distance >= 0.0 && (1..=3).contains(&n.marks)
        });
        if !notches_ok {
            return Err(ModelError::BadNotch(self.id));
        }
        if !self.lines.iter().all(InternalLine::is_valid) {
            return Err(ModelError::BadLine(self.id));
        }
        if !self.fold_is_valid() {
            return Err(ModelError::BadFold(self.id));
        }
        if let Some(twin) = self.twin_shape() {
            twin.check()?;
        }
        Ok(())
    }
    /// No fold, or a fold on a straight edge with no notches, on an unpaired piece whose every
    /// point (vertices, control points, internal lines) is on the same side of the fold line.
    fn fold_is_valid(&self) -> bool {
        let Some(f) = self.fold else { return true };
        if f >= self.len() || self.edges[f] != Edge::Line || self.twin.is_some() {
            return false;
        }
        if self.notches.iter().any(|n| n.edge == f) {
            return false;
        }
        let (a, b) = self.edge_ends(f);
        let d = b - a;
        let len = d.length();
        if len < 1e-9 {
            return false;
        }
        let side = |p: Point2| (d.x * (p.y - a.y) - d.y * (p.x - a.x)) / len;
        let outline = self.vertices.iter().map(|v| v.pos).chain(self.edges.iter().flat_map(|e| match *e {
            Edge::Line => Vec::new(),
            Edge::Curve { c1, c2 } => vec![c1, c2],
        }));
        let lines = self.lines.iter().flat_map(InternalLine::points);
        let (mut left, mut right) = (false, false);
        for p in outline.chain(lines) {
            let s = side(p);
            left |= s > 1e-6;
            right |= s < -1e-6;
        }
        !(left && right)
    }
```

Note: `Option::is_none_or` is stable since Rust 1.82.

- [ ] **Step 3: Implement the project changes** (`crates/core/src/project.rs`)

- `pub const SCHEMA_VERSION: u32 = 2;` Update the doc comment: "Version 2 added seam allowances, notches, internal lines, folds and twins (2026-10-09)".
- `use crate::{Piece, PieceId, Point2, Side, Units};`
- Add `ModelError` variants, with Display text:
  - `BadAllowance(PieceId)`: "piece {} has an invalid seam allowance";
  - `BadNotch(PieceId)`: "piece {} has an invalid notch";
  - `BadLine(PieceId)`: "piece {} has an invalid internal line";
  - `BadFold(PieceId)`: "piece {} has an invalid fold line".
- Replace `remove_piece` and `check`, and add the new methods:

```rust
    /// The stored piece an id belongs to, and whether the id names that piece or its twin.
    pub fn owner(&self, id: PieceId) -> Option<(&Piece, Side)> {
        self.pieces.iter().find_map(|p| {
            if p.id == id {
                Some((p, Side::Master))
            } else if p.twin.as_ref().is_some_and(|t| t.id == id) {
                Some((p, Side::Twin))
            } else {
                None
            }
        })
    }
    pub fn owner_mut(&mut self, id: PieceId) -> Option<(&mut Piece, Side)> {
        self.pieces.iter_mut().find_map(|p| {
            if p.id == id {
                Some((p, Side::Master))
            } else if p.twin.as_ref().is_some_and(|t| t.id == id) {
                Some((p, Side::Twin))
            } else {
                None
            }
        })
    }
    /// The name shown for an id: the piece's, or its twin's.
    pub fn name_of(&self, id: PieceId) -> Option<&str> {
        match self.owner(id)? {
            (p, Side::Master) => Some(&p.name),
            (p, Side::Twin) => p.twin.as_ref().map(|t| t.name.as_str()),
        }
    }
    /// Gives `master` a mirror-image twin called `name`, placed by `offset` (see [`crate::Twin`]),
    /// and returns the twin's id. None when there is no such piece, or it is folded or already
    /// paired.
    pub fn add_twin(&mut self, master: PieceId, name: String, offset: Point2) -> Option<PieceId> {
        let id = PieceId(self.next_piece_id);
        let piece = self.piece_mut(master)?;
        if piece.twin.is_some() || piece.fold.is_some() {
            return None;
        }
        piece.twin = Some(crate::Twin { id, name, offset });
        self.next_piece_id = self.next_piece_id.saturating_add(1);
        Some(id)
    }
    /// Turns `master`'s twin into an ordinary piece with the twin's current shape, id and name.
    pub fn break_twin(&mut self, master: PieceId) -> Option<PieceId> {
        let piece = self.piece_mut(master)?;
        let twin = piece.twin_shape()?;
        piece.twin = None;
        let id = twin.id;
        self.pieces.push(twin);
        Some(id)
    }
    /// Removes the piece or twin with this id and returns its shape. Removing a piece that has
    /// a twin keeps the twin, as an ordinary piece.
    pub fn remove_piece(&mut self, id: PieceId) -> Option<Piece> {
        match self.owner(id)? {
            (_, Side::Twin) => {
                let (piece, _) = self.owner_mut(id)?;
                let shape = piece.twin_shape();
                piece.twin = None;
                shape
            }
            (piece, Side::Master) => {
                if piece.twin.is_some() {
                    self.break_twin(id);
                }
                let at = self.pieces.iter().position(|p| p.id == id)?;
                Some(self.pieces.remove(at))
            }
        }
    }
    /// At most [`MAX_PIECES`] pieces and [`MAX_TOTAL_VERTICES`] points in all (a twin counts
    /// as a piece with its own points), every piece valid, ids (pieces' and twins') unique and
    /// below the id counter, and the counter itself at most [`MAX_PIECE_ID`].
    pub fn check(&self) -> Result<(), ModelError> {
        let shapes = self.pieces.len() + self.pieces.iter().filter(|p| p.twin.is_some()).count();
        if shapes > MAX_PIECES {
            return Err(ModelError::TooManyPieces);
        }
        if self.next_piece_id > MAX_PIECE_ID {
            return Err(ModelError::IdCounterTooLarge);
        }
        let mut seen = std::collections::BTreeSet::new();
        let mut total_points = 0_usize;
        for p in &self.pieces {
            p.check()?;
            let copies = if p.twin.is_some() { 2 } else { 1 };
            // At most MAX_PIECES pieces of at most MAX_VERTICES_PER_PIECE points: no overflow.
            total_points += p.point_count() * copies;
            for id in std::iter::once(p.id).chain(p.twin.as_ref().map(|t| t.id)) {
                if !seen.insert(id) {
                    return Err(ModelError::DuplicateId(id));
                }
                if id.0 >= self.next_piece_id {
                    return Err(ModelError::IdCounterBehind(id));
                }
            }
        }
        if total_points > MAX_TOTAL_VERTICES {
            return Err(ModelError::TooManyPointsInProject);
        }
        Ok(())
    }
```

`crates/core/src/lib.rs` exports:
```rust
pub use piece::{
    DEFAULT_ALLOWANCE_MM, Edge, EdgeProps, HEM_ALLOWANCE_MM, HandleEnd, InternalLine, LineKind,
    MAX_ALLOWANCE_MM, MAX_COORDINATE_MM, MAX_NAME_CHARS, MAX_VERTICES_PER_PIECE, Notch, NotchStyle,
    Piece, PieceId, Point2, Side, Twin, Vertex, VertexKind,
};
```

- [ ] **Step 4: geom keeps its callers working** (`crates/geom/src/lib.rs`)

Replace the body of `split_edge` from `let seg = ...` on, and add `remove_vertex`:

```rust
/// Adds a vertex on edge `i` at curve parameter `t`; curves are split exactly. Returns the new
/// vertex's index, or `None` when `t` is within 2% of either end (that would duplicate a vertex)
/// or the edge is the piece's fold line (it must stay one straight edge). Notches keep their
/// places.
pub fn split_edge(piece: &mut Piece, i: usize, t: f64) -> Option<usize> {
    if !(0.02..=0.98).contains(&t) || piece.fold == Some(i) {
        return None;
    }
    let seg = edge_seg(piece, i);
    let at = cp(seg.eval(t));
    let first_len = seg.subsegment(0.0..t).arclen(ACCURACY);
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
    Some(piece.split_edge_at(i, vertex, first, second, first_len))
}

/// Removes vertex `i` (see [`Piece::remove_vertex`]), measuring the edge before it so its
/// notches keep their places. Refused (false) below 4 vertices.
pub fn remove_vertex(piece: &mut Piece, i: usize) -> bool {
    if piece.len() <= 3 {
        return false;
    }
    let prev_len = edge_length(piece, piece.prev(i));
    piece.remove_vertex(i, prev_len)
}
```

Add a geom test:

```rust
    #[test]
    fn splitting_keeps_notches_and_never_splits_the_fold() {
        let mut s = square();
        s.notches = vec![opendrape_core::Notch::new(0, 60.0)];
        assert_eq!(split_edge(&mut s, 0, 0.25), Some(1));
        assert_eq!(s.notches, vec![opendrape_core::Notch::new(1, 35.0)]);
        assert!(remove_vertex(&mut s, 1));
        assert_eq!(s.notches, vec![opendrape_core::Notch::new(0, 60.0)]);
        s.fold = Some(3);
        assert_eq!(split_edge(&mut s, 3, 0.5), None);
    }
```

In `crates/app/src/editor/canvas.rs` `delete_selection`, the vertex case calls `geom::remove_vertex(piece, i)` instead of `piece.remove_vertex(i)`.

- [ ] **Step 5: io migration and fixtures** (`crates/io/src/lib.rs`)

`check_version` returns the version it found, and `read_from` upgrades version 1. Replace `check_version`'s signature and doc, and the end of `read_from`:

```rust
    let found = check_version(text)?;
    let mut project: Project =
        serde_json::from_str(text).map_err(|e| OdpError::Corrupt(e.to_string()))?;
    if found == 1 {
        upgrade_from_v1(&mut project);
    }
    project.check().map_err(OdpError::Invalid)?;
    Ok(project)
}

/// Version 1 (M2a) had no seam allowances, notches, internal lines, folds or twins. Serde's
/// field defaults already give a v1 file every new field except one set of edge properties per
/// edge, which needs the edge count; add those, and mark the project as current.
fn upgrade_from_v1(project: &mut Project) {
    for piece in &mut project.pieces {
        piece.edge_props = vec![opendrape_core::EdgeProps::default(); piece.edges.len()];
    }
    project.schema_version = SCHEMA_VERSION;
}

/// Reads only `schema_version` (other fields are skipped without building anything), so a file
/// from a newer format is reported as such even when its contents have changed shape. Returns
/// the version found.
///
/// Versions 1 and 2 parse directly into `Project` (version 1's missing fields take their
/// defaults; see [`upgrade_from_v1`]). A future version that renames or reshapes fields will
/// need a step that parses older documents as a `serde_json::Value` and rewrites them first.
fn check_version(text: &str) -> Result<u64, OdpError> {
```
…ending the function with `Ok(found)` instead of `Ok(())`.

`crates/io/tests/fixtures.rs`:
- Do **not** edit `fixtures/v1/project.json`.
- In `format_v1_still_opens`, add the new fields to both `Piece` literals:
  `allowance: 10.0, edge_props: vec![EdgeProps::default(); 4]` (3 for the pocket), `notches: vec![], lines: vec![], fold: None, twin: None`.
- Change `assert_eq!(loaded.schema_version, 1)` to `assert_eq!(loaded.schema_version, 2, "upgraded on load")`.
- Update the imports.

Then add the v2 fixture `crates/io/tests/fixtures/v2/project.json`:

```json
{
  "schema_version": 2,
  "units": "cm",
  "pieces": [
    {
      "id": 1,
      "name": "Skirt front",
      "vertices": [
        { "pos": { "x": 0.0, "y": 0.0 }, "kind": "corner" },
        { "pos": { "x": 250.0, "y": 0.0 }, "kind": "corner" },
        { "pos": { "x": 200.0, "y": 600.0 }, "kind": "corner" },
        { "pos": { "x": 0.0, "y": 600.0 }, "kind": "corner" }
      ],
      "edges": [
        { "type": "line" },
        { "type": "line" },
        { "type": "curve", "c1": { "x": 130.0, "y": 610.0 }, "c2": { "x": 60.0, "y": 605.0 } },
        { "type": "line" }
      ],
      "grain_deg": 90.0,
      "allowance": 10.0,
      "edge_props": [
        { "allowance": null, "hem": true },
        { "allowance": 15.0, "hem": false },
        { "allowance": null, "hem": false },
        { "allowance": null, "hem": false }
      ],
      "notches": [
        { "edge": 1, "distance": 180.0, "marks": 1, "style": "slit" },
        { "edge": 2, "distance": 40.0, "marks": 2, "style": "v" }
      ],
      "lines": [
        {
          "vertices": [
            { "pos": { "x": 30.0, "y": 300.0 }, "kind": "corner" },
            { "pos": { "x": 120.0, "y": 300.0 }, "kind": "corner" }
          ],
          "edges": [{ "type": "line" }],
          "closed": false,
          "kind": "marking"
        }
      ],
      "fold": 3,
      "twin": null
    },
    {
      "id": 2,
      "name": "Back left",
      "vertices": [
        { "pos": { "x": 400.0, "y": 0.0 }, "kind": "corner" },
        { "pos": { "x": 600.0, "y": 0.0 }, "kind": "corner" },
        { "pos": { "x": 580.0, "y": 600.0 }, "kind": "corner" },
        { "pos": { "x": 400.0, "y": 600.0 }, "kind": "corner" }
      ],
      "edges": [{ "type": "line" }, { "type": "line" }, { "type": "line" }, { "type": "line" }],
      "grain_deg": 90.0,
      "allowance": 12.0,
      "edge_props": [
        { "allowance": null, "hem": true },
        { "allowance": null, "hem": false },
        { "allowance": null, "hem": false },
        { "allowance": null, "hem": false }
      ],
      "notches": [],
      "lines": [],
      "fold": null,
      "twin": { "id": 3, "name": "Back right", "offset": { "x": 1300.0, "y": 0.0 } }
    }
  ],
  "next_piece_id": 4
}
```

Add a test that spells out the expected project field by field, using the same helpers as the v1 test:

```rust
#[test]
fn format_v2_still_opens() {
    let loaded = opendrape_io::from_bytes(&odp(include_str!("fixtures/v2/project.json")))
        .expect("the frozen v2 project opens");
    let props = |allowance: Option<f64>, hem: bool| EdgeProps { allowance, hem };
    let front = Piece {
        id: PieceId(1),
        name: "Skirt front".into(),
        vertices: vec![corner(0.0, 0.0), corner(250.0, 0.0), corner(200.0, 600.0), corner(0.0, 600.0)],
        edges: vec![
            Edge::Line,
            Edge::Line,
            Edge::Curve { c1: at(130.0, 610.0), c2: at(60.0, 605.0) },
            Edge::Line,
        ],
        grain_deg: 90.0,
        allowance: 10.0,
        edge_props: vec![props(None, true), props(Some(15.0), false), props(None, false), props(None, false)],
        notches: vec![
            Notch { edge: 1, distance: 180.0, marks: 1, style: NotchStyle::Slit },
            Notch { edge: 2, distance: 40.0, marks: 2, style: NotchStyle::V },
        ],
        lines: vec![InternalLine {
            vertices: vec![corner(30.0, 300.0), corner(120.0, 300.0)],
            edges: vec![Edge::Line],
            closed: false,
            kind: LineKind::Marking,
        }],
        fold: Some(3),
        twin: None,
    };
    let back = Piece {
        id: PieceId(2),
        name: "Back left".into(),
        vertices: vec![corner(400.0, 0.0), corner(600.0, 0.0), corner(580.0, 600.0), corner(400.0, 600.0)],
        edges: vec![Edge::Line; 4],
        grain_deg: 90.0,
        allowance: 12.0,
        edge_props: vec![props(None, true), props(None, false), props(None, false), props(None, false)],
        notches: vec![],
        lines: vec![],
        fold: None,
        twin: None, // set below, once the ids are taken
    };
    let mut expected = Project::new(); // units default to cm
    expected.add_piece(front);
    expected.add_piece(back);
    // The twin has id 3 and the file's counter stands at 4. Take id 3 with a throwaway piece
    // (the only way to move the counter from outside the model), then give the back its twin.
    let spare = expected.add_piece(Piece::polygon(PieceId(0), "x", &[at(0.0, 0.0), at(1.0, 0.0), at(0.0, 1.0)]));
    expected.remove_piece(spare);
    expected.pieces[1].twin = Some(Twin { id: PieceId(3), name: "Back right".into(), offset: at(1300.0, 0.0) });
    assert_eq!(loaded, expected);
    assert_eq!(loaded.schema_version, 2);
    assert_eq!(loaded.name_of(PieceId(3)), Some("Back right"));
    assert_eq!(loaded.next_piece_name("Piece"), "Piece 4");
}

#[test]
fn refuses_invalid_v2_details() {
    let good = include_str!("fixtures/v2/project.json");
    for (from, to) in [
        (r#""fold": 3"#, r#""fold": 2"#),                  // a curved edge
        (r#""edge": 1, "distance": 180.0"#, r#""edge": 9, "distance": 180.0"#), // no such edge
        (r#""twin": null"#, r#""twin": { "id": 5, "name": "x", "offset": { "x": 0, "y": 0 } }"#), // folded and paired
        (r#""allowance": 12.0"#, r#""allowance": 500.0"#),
    ] {
        assert!(good.contains(from), "{from}");
        let bad = good.replacen(from, to, 1);
        assert!(
            matches!(opendrape_io::from_bytes(&odp(&bad)), Err(opendrape_io::OdpError::Invalid(_))),
            "{to}"
        );
    }
}
```

Update `crates/io/tests/fixtures/README.md`: "Folders: v1 (M2a), v2 (M2b: seam allowance, notches, internal lines, fold, twin)."

- [ ] **Step 6: Run and see everything pass**

Run: `cargo nextest run -p opendrape-core -p opendrape-geom -p opendrape-io && cargo nextest run -p opendrape`
Expected:
- all pass, including the 10 new core tests, 3 new project tests, 1 new geom test and 2 new fixture tests;
- the editor and app UI tests still pass, because they build pieces with `Piece::rectangle`/`polygon`.

- [ ] **Step 7: Commit**

```bash
cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings
git add -A crates
git commit -m "feat(core,io): seam allowance, notches, internal lines, folds and twins; format version 2

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---
### Task 2: Shapes: unfolded halves and mirrored twins

**Files:**
- Create: `crates/geom/src/shapes.rs`
- Modify: `crates/geom/src/lib.rs` (module and exports)

**Interfaces:**
- Consumes: Task 1 (`Piece` fields, `reflected`, `twin_shape`, `InternalLine::mapped`, `Project::pieces`).
- Produces:
  - `pub struct Shape { id: PieceId, source: PieceId, kind: ShapeKind, piece: Piece }`.
  - `pub enum ShapeKind { Plain, Folded { first: usize, drawn: usize, fold: (Point2, Point2) }, Twin { offset: Point2 } }`.
  - Methods on `Shape`:
    - `stored_vertex(k) -> Option<usize>` and `stored_edge(j) -> Option<usize>` (None on the pale half);
    - `shape_vertex(i) -> usize` and `shape_edge(i) -> usize` (stored index to shape index);
    - `to_stored(p)`, `from_stored(p)`, `to_stored_delta(d)`.
  - Free functions:
    - `shapes(&Project) -> Vec<Shape>` (each stored piece, then its twin);
    - `shape_of(&Project, PieceId) -> Option<Shape>`;
    - `unfolded(&Piece) -> Piece`.

**Unfolded layout** (stored half with `n` vertices, fold edge `f` running stored vertex `f` → `f+1`):
- Let `first = (f+1) % n`, and let u(k) be stored vertex `(first+k) % n`.
- **Vertices:**
  - outline vertices `0..n` are u(0)..u(n−1), the drawn half from the fold's far end round to its near end;
  - vertices `n..2n−2` are the mirror images of u(n−2)..u(1).
- **Edges:**
  - edges `0..n−1` are the drawn edges u(m) → u(m+1);
  - edge `2n−3−m` is drawn edge `m` mirrored and reversed: control points swap ends (`c1' = mirror(c2)`, `c2' = mirror(c1)`), and notch distances become `length − d`.
- **Not included:** the fold edge itself, since it lies inside the full piece.

- [ ] **Step 1: Failing tests** (`crates/geom/src/shapes.rs`, at the bottom)

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use opendrape_core::{Edge, InternalLine, Notch, PieceId, Point2, Project};

    fn p(x: f64, y: f64) -> Point2 {
        Point2::new(x, y)
    }

    /// The right half of a 200 × 200 square, folded on its left edge (edge 3: (0,200)→(0,0)).
    fn half() -> Piece {
        let mut s = Piece::rectangle(PieceId(1), "Front", p(0.0, 0.0), 100.0, 200.0);
        s.fold = Some(3);
        s
    }

    fn close(a: Point2, b: Point2) {
        assert!(a.distance(b) < 1e-9, "{a:?} vs {b:?}");
    }

    #[test]
    fn unfolding_mirrors_the_half_across_the_fold() {
        let full = unfolded(&half());
        let corners: Vec<Point2> = full.vertices.iter().map(|v| v.pos).collect();
        let expected = [p(0.0, 0.0), p(100.0, 0.0), p(100.0, 200.0), p(0.0, 200.0), p(-100.0, 200.0), p(-100.0, 0.0)];
        assert_eq!(corners.len(), expected.len());
        for (a, b) in corners.iter().zip(expected) {
            close(*a, b);
        }
        assert_eq!(full.edges.len(), 6);
        assert_eq!(full.edge_props.len(), 6);
        assert_eq!((full.fold, full.check()), (None, Ok(())));
        assert!((crate::area(&full) - 40_000.0).abs() < 1e-6);
    }

    #[test]
    fn unfolding_maps_indices_and_reverses_curves() {
        let mut h = half();
        h.set_curved(1, true); // right edge (100,0)→(100,200)
        h.notches = vec![Notch::new(0, 30.0)];
        h.lines = vec![InternalLine::open(&[p(20.0, 50.0), p(60.0, 50.0)])];
        let s = shape_of(&{
            let mut pr = Project::new();
            pr.add_piece(h.clone());
            pr
        }, PieceId(1))
        .unwrap();
        let ShapeKind::Folded { first, drawn, fold } = s.kind else { panic!("folded") };
        assert_eq!((first, drawn), (0, 4));
        close(fold.0, p(0.0, 200.0));
        close(fold.1, p(0.0, 0.0));
        assert_eq!((s.stored_vertex(2), s.stored_vertex(4)), (Some(2), None));
        assert_eq!((s.stored_edge(2), s.stored_edge(3)), (Some(2), None));
        assert_eq!((s.shape_vertex(1), s.shape_edge(1)), (1, 1));
        // Mirror of drawn edge 1 is outline edge 2n-3-1 = 4: from (-100,200) to (-100,0).
        let Edge::Curve { c1, c2 } = s.piece.edges[4] else { panic!("curved") };
        let Edge::Curve { c1: o1, c2: o2 } = h.edges[1] else { panic!() };
        close(c1, p(-o2.x, o2.y));
        close(c2, p(-o1.x, o1.y));
        // The bottom notch at 30 mm appears on both halves, the mirrored copy measured from
        // the mirrored edge's start (-100,0): 100 - 30 = 70 mm along.
        assert_eq!(s.piece.notches, vec![Notch::new(0, 30.0), Notch::new(5, 70.0)]);
        assert_eq!(s.piece.lines.len(), 2);
        close(s.piece.lines[1].vertices[1].pos, p(-60.0, 50.0));
    }

    #[test]
    fn folds_on_any_edge_map_back() {
        let mut s = Piece::rectangle(PieceId(1), "Back", p(0.0, 0.0), 100.0, 200.0);
        s.fold = Some(1); // right edge (100,0)→(100,200)
        let mut pr = Project::new();
        pr.add_piece(s);
        let shape = shape_of(&pr, PieceId(1)).unwrap();
        let ShapeKind::Folded { first, .. } = shape.kind else { panic!() };
        assert_eq!(first, 2);
        assert_eq!(shape.stored_vertex(0), Some(2)); // outline starts at (100,200)
        close(shape.piece.vertices[0].pos, p(100.0, 200.0));
        assert_eq!(shape.shape_vertex(2), 0);
        assert_eq!(shape.shape_edge(3), 1);
        close(shape.piece.vertices[4].pos, p(200.0, 0.0));
    }

    #[test]
    fn twins_follow_their_piece_and_map_back() {
        let mut pr = Project::new();
        let a = pr.add_piece(Piece::rectangle(PieceId(0), "Back", p(0.0, 0.0), 100.0, 200.0));
        let t = pr.add_twin(a, "Back (mirror)".into(), p(300.0, 0.0)).unwrap();
        let all = shapes(&pr);
        assert_eq!(all.len(), 2);
        assert_eq!((all[0].id, all[0].kind), (a, ShapeKind::Plain));
        let twin = &all[1];
        assert_eq!((twin.id, twin.source), (t, a));
        assert_eq!(twin.kind, ShapeKind::Twin { offset: p(300.0, 0.0) });
        close(twin.piece.vertices[1].pos, p(200.0, 0.0));
        let q = p(123.0, 45.0);
        close(twin.to_stored(twin.from_stored(q)), q);
        close(twin.to_stored_delta(p(5.0, 7.0)), p(-5.0, 7.0));
        assert_eq!((twin.stored_vertex(3), twin.shape_edge(2)), (Some(3), 2));
    }
}
```

Run: `cargo nextest run -p opendrape-geom shapes`
Expected: compile errors (the module doesn't exist yet).

- [ ] **Step 2: Implement** (`crates/geom/src/shapes.rs`, above the tests)

```rust
//! What each stored piece shows on the pattern table. A cut-on-fold piece is stored as half
//! and shown whole; a paired piece also shows its mirror-image twin. Each [`Shape`] is a
//! concrete piece, and remembers how its points and indices map back to what is stored,
//! because edits always go to the stored piece.

use crate::edge_length;
use opendrape_core::{Edge, EdgeProps, Notch, Piece, PieceId, Point2, Project, Vertex};

/// One thing drawn on the pattern table.
#[derive(Clone, Debug, PartialEq)]
pub struct Shape {
    /// The id this shape is selected by: the piece's own, or its twin's.
    pub id: PieceId,
    /// The stored piece it comes from.
    pub source: PieceId,
    pub kind: ShapeKind,
    /// The full outline as an ordinary piece (unfolded, or reflected and moved), with its
    /// edges' allowances, its notches and its internal lines in place.
    pub piece: Piece,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ShapeKind {
    /// The stored piece as it is.
    Plain,
    /// A cut-on-fold piece. Outline vertices `0..drawn` are the stored half's, starting at
    /// stored vertex `first` (the fold's far end); the rest are its pale mirror image. `fold`
    /// is the fold line, from its near end to its far end.
    Folded { first: usize, drawn: usize, fold: (Point2, Point2) },
    /// A twin: the stored piece reflected left to right, then moved by `offset`.
    Twin { offset: Point2 },
}

impl Shape {
    /// The stored piece's vertex for outline vertex `k`; `None` on the pale half of a fold.
    pub fn stored_vertex(&self, k: usize) -> Option<usize> {
        match self.kind {
            ShapeKind::Plain | ShapeKind::Twin { .. } => Some(k),
            ShapeKind::Folded { first, drawn, .. } => (k < drawn).then_some((first + k) % drawn),
        }
    }
    /// The stored piece's edge for outline edge `j`; `None` on the pale half of a fold.
    pub fn stored_edge(&self, j: usize) -> Option<usize> {
        match self.kind {
            ShapeKind::Plain | ShapeKind::Twin { .. } => Some(j),
            ShapeKind::Folded { first, drawn, .. } => (j + 1 < drawn).then_some((first + j) % drawn),
        }
    }
    /// The outline vertex showing stored vertex `i`.
    pub fn shape_vertex(&self, i: usize) -> usize {
        match self.kind {
            ShapeKind::Plain | ShapeKind::Twin { .. } => i,
            ShapeKind::Folded { first, drawn, .. } => (i + drawn - first) % drawn,
        }
    }
    /// The outline edge showing stored edge `i` (for a fold, `i` must not be the fold edge).
    pub fn shape_edge(&self, i: usize) -> usize {
        self.shape_vertex(i)
    }
    /// A point of this shape in the stored piece's coordinates.
    pub fn to_stored(&self, p: Point2) -> Point2 {
        match self.kind {
            ShapeKind::Twin { offset } => Point2::new(offset.x - p.x, p.y - offset.y),
            _ => p,
        }
    }
    /// A point of the stored piece where this shape shows it.
    pub fn from_stored(&self, p: Point2) -> Point2 {
        match self.kind {
            ShapeKind::Twin { offset } => Point2::new(offset.x - p.x, p.y + offset.y),
            _ => p,
        }
    }
    /// A movement on this shape as a movement of the stored piece.
    pub fn to_stored_delta(&self, d: Point2) -> Point2 {
        match self.kind {
            ShapeKind::Twin { .. } => Point2::new(-d.x, d.y),
            _ => d,
        }
    }
}

/// Every shape on the table, in drawing order: each stored piece, then its twin.
pub fn shapes(project: &Project) -> Vec<Shape> {
    let mut out = Vec::new();
    for piece in &project.pieces {
        out.push(main_shape(piece));
        if let (Some(t), Some(twin)) = (&piece.twin, piece.twin_shape()) {
            out.push(Shape { id: t.id, source: piece.id, kind: ShapeKind::Twin { offset: t.offset }, piece: twin });
        }
    }
    out
}

/// The shape selected by `id` (a piece's or a twin's).
pub fn shape_of(project: &Project, id: PieceId) -> Option<Shape> {
    shapes(project).into_iter().find(|s| s.id == id)
}

/// The whole piece a cut-on-fold half makes (a copy of `piece` when it has no fold).
pub fn unfolded(piece: &Piece) -> Piece {
    main_shape(piece).piece
}

fn main_shape(piece: &Piece) -> Shape {
    let (full, kind) = match piece.fold {
        Some(f) if f < piece.len() => unfold(piece, f),
        _ => (piece.clone(), ShapeKind::Plain),
    };
    Shape { id: piece.id, source: piece.id, kind, piece: full }
}

/// Mirror image of `p` across the line through `a` and `b`.
fn reflect_across(p: Point2, a: Point2, b: Point2) -> Point2 {
    let d = b - a;
    let t = ((p.x - a.x) * d.x + (p.y - a.y) * d.y) / (d.x * d.x + d.y * d.y);
    let foot = a + d * t;
    foot * 2.0 - p
}

fn unfold(piece: &Piece, f: usize) -> (Piece, ShapeKind) {
    let n = piece.len();
    let first = (f + 1) % n;
    let (near, far) = piece.edge_ends(f);
    let mirror = |p: Point2| reflect_across(p, near, far);
    let u = |k: usize| (first + k) % n;

    let mut vertices: Vec<Vertex> = (0..n).map(|k| piece.vertices[u(k)]).collect();
    let mut edges: Vec<Edge> = (0..n - 1).map(|m| piece.edges[u(m)]).collect();
    let mut props: Vec<EdgeProps> = (0..n - 1).map(|m| piece.edge_props[u(m)]).collect();
    for k in (1..n - 1).rev() {
        let v = piece.vertices[u(k)];
        vertices.push(Vertex { pos: mirror(v.pos), kind: v.kind });
    }
    for m in (0..n - 1).rev() {
        edges.push(match piece.edges[u(m)] {
            Edge::Line => Edge::Line,
            Edge::Curve { c1, c2 } => Edge::Curve { c1: mirror(c2), c2: mirror(c1) },
        });
        props.push(piece.edge_props[u(m)]);
    }
    let drawn_index = |stored: usize| (stored + n - first) % n;
    let mut notches = Vec::new();
    for notch in &piece.notches {
        let m = drawn_index(notch.edge);
        notches.push(Notch { edge: m, ..*notch });
    }
    for notch in &piece.notches {
        let m = drawn_index(notch.edge);
        let len = edge_length(piece, notch.edge);
        notches.push(Notch { edge: 2 * n - 3 - m, distance: (len - notch.distance).max(0.0), ..*notch });
    }
    let mut lines = piece.lines.clone();
    lines.extend(piece.lines.iter().map(|l| l.mapped(mirror)));
    let full = Piece {
        id: piece.id,
        name: piece.name.clone(),
        vertices,
        edges,
        grain_deg: piece.grain_deg,
        allowance: piece.allowance,
        edge_props: props,
        notches,
        lines,
        fold: None,
        twin: None,
    };
    (full, ShapeKind::Folded { first, drawn: n, fold: (near, far) })
}
```

Check the mirrored notch index against the test: drawn edge `m = 0` gives `2n − 3 − 0 = 5` for n = 4 ✓.

The test lists the drawn copies first, then the mirrored copies, which is why the code uses two loops. Keep the loops in that order; the test's expected list depends on it.

In `crates/geom/src/lib.rs`, add `mod shapes;` and `pub use shapes::{Shape, ShapeKind, shape_of, shapes, unfolded};`.

- [ ] **Step 3: Run and see them pass**

Run: `cargo nextest run -p opendrape-geom`
Expected: 4 new tests pass, plus the existing ones.

- [ ] **Step 4: Commit**

```bash
cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings
git add crates/geom
git commit -m "feat(geom): shapes - unfolded cut-on-fold pieces and mirrored twins

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---
### Task 3: Cut line, notch marks, internal-line geometry, label anchors

**Files:**
- Create: `crates/geom/src/allowance.rs`, `crates/geom/src/marks.rs`
- Modify: `crates/geom/src/lib.rs`, `crates/geom/Cargo.toml`, root `Cargo.toml`

**Interfaces:**
- Consumes:
  - Task 1: `Piece::edge_allowance`, `edge_props`, `Notch`, `NotchStyle`, `InternalLine`.
  - `lib.rs`: `edge_points`, `edge_seg`, `edge_length`, `flatten`, `bez_path`.
- Produces:
  - `pub fn cut_line(piece: &Piece) -> Vec<Point2>`: closed, the first point not repeated.
  - Notch marks:
    - `pub const NOTCH_DEPTH_MM: f64 = 5.0`;
    - `pub const NOTCH_SPACING_MM: f64 = 3.0`;
    - `pub fn notch_marks(piece: &Piece, notch: &Notch) -> Vec<[Point2; 2]>`.
  - Positions along edges:
    - `pub fn distance_along(piece: &Piece, edge: usize, t: f64) -> f64`;
    - `pub fn point_at_distance(piece: &Piece, edge: usize, distance: f64) -> Point2`.
  - Internal lines:
    - `pub fn line_points(line: &InternalLine, tolerance: f64) -> Vec<Point2>`: open from start to end; closed returns to the first point at the end;
    - `pub fn line_length(line: &InternalLine) -> f64`;
    - `pub fn nearest_line(piece: &Piece, p: Point2) -> Option<(usize, usize, f64, f64)>`, returning (line, edge, t, distance).
  - Labels and winding:
    - `pub fn edge_label_anchor(piece: &Piece, i: usize) -> (Point2, Point2)`: the halfway point along the edge and the unit outward normal there;
    - `pub fn is_counter_clockwise(piece: &Piece) -> bool`.

**Algorithm (probe-verified):**
1. Flatten each edge at 0.1 mm.
2. For each flattened segment, offset it outward by that edge's allowance. Outward is the right-hand normal `(d.y, −d.x)` for a counter-clockwise outline, and the opposite for a clockwise one, which is what twins are.
3. Walk the segments in order. At each joint, emit one of these:
   - the intersection of the two offset lines: a miter at convex joints, a trim at concave ones;
   - a squared end when a convex miter would be more than 2.5 × the wider allowance from the corner: `pa, pa + d_a·w_a, pb − d_b·w_b, pb`;
   - between a hem edge and a non-hem edge, the mirrored corner: `pa, at_hem, on_cut, pb`, or `pa, on_cut, at_hem, pb` when the hem comes first;
   - both offset ends, for parallel lines.
4. This raw path may cross itself. Combine it with the stitching polygon using `i_overlay`'s `simplify_shape(FillRule::NonZero)`, and return the outer contour of the largest shape.

- [ ] **Step 1: Dependency**

Root `Cargo.toml` `[workspace.dependencies]`: `i_overlay = "9.0.1"`. `crates/geom/Cargo.toml` `[dependencies]`: `i_overlay.workspace = true`.

Run `cargo deny check`. Expected: ok (MIT OR Apache-2.0; its `i_float`, `i_shape`, `i_tree` and `i_key_sort` dependencies are MIT as well).

- [ ] **Step 2: Failing tests**

`crates/geom/src/allowance.rs` (tests at the bottom):

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use opendrape_core::{PieceId, Point2};

    fn p(x: f64, y: f64) -> Point2 {
        Point2::new(x, y)
    }

    fn polygon(pts: &[(f64, f64)], widths: &[f64]) -> Piece {
        let corners: Vec<Point2> = pts.iter().map(|&(x, y)| p(x, y)).collect();
        let mut piece = Piece::polygon(PieceId(1), "P", &corners);
        for (props, &w) in piece.edge_props.iter_mut().zip(widths) {
            props.allowance = Some(w);
        }
        piece
    }

    fn area(points: &[Point2]) -> f64 {
        (0..points.len())
            .map(|k| {
                let (a, b) = (points[k], points[(k + 1) % points.len()]);
                a.x * b.y - b.x * a.y
            })
            .sum::<f64>()
            .abs()
            / 2.0
    }

    /// Smallest distance from `q` to the stitching outline.
    fn to_outline(piece: &Piece, q: Point2) -> f64 {
        crate::nearest_edge(piece, q).unwrap().2
    }

    #[test]
    fn a_rectangle_grows_by_its_allowance_in_either_winding() {
        let r = [(0.0, 0.0), (100.0, 0.0), (100.0, 200.0), (0.0, 200.0)];
        let rev: Vec<(f64, f64)> = r.iter().rev().copied().collect();
        for pts in [r.to_vec(), rev] {
            let cut = cut_line(&polygon(&pts, &[10.0; 4]));
            assert!((area(&cut) - 120.0 * 220.0).abs() < 1e-3, "{}", area(&cut));
        }
    }

    #[test]
    fn the_piece_allowance_and_hem_defaults_apply() {
        let mut piece = Piece::rectangle(PieceId(1), "R", p(0.0, 0.0), 100.0, 200.0);
        piece.edge_props[0].hem = true; // bottom: 30 mm; the others 10 mm
        let cut = cut_line(&piece);
        let min_y = cut.iter().map(|q| q.y).fold(f64::MAX, f64::min);
        assert!((min_y + 30.0).abs() < 1e-6, "{min_y}");
        assert!((area(&cut) - 120.0 * 240.0).abs() < 1e-3, "{}", area(&cut));
    }

    #[test]
    fn hem_corners_mirror_the_flared_side_seams() {
        let mut piece = polygon(&[(0.0, 0.0), (400.0, 0.0), (320.0, 600.0), (80.0, 600.0)], &[30.0, 10.0, 10.0, 10.0]);
        piece.edge_props[0].hem = true;
        let cut = cut_line(&piece);
        let bottom: Vec<&Point2> = cut.iter().filter(|q| q.y < -29.99).collect();
        let min_x = bottom.iter().map(|q| q.x).fold(f64::MAX, f64::min);
        let max_x = bottom.iter().map(|q| q.x).fold(f64::MIN, f64::max);
        // Checked by hand: the side seam's cut line meets the hem line at x = -10.0885 and,
        // mirrored about it, slopes 4 mm back in over the 30 mm hem.
        assert!((min_x + 6.0885).abs() < 0.01, "{min_x}");
        assert!((max_x - 406.0885).abs() < 0.01, "{max_x}");
    }

    #[test]
    fn a_concave_corner_meets_at_both_widths() {
        let pts = [(0.0, 0.0), (300.0, 0.0), (300.0, 100.0), (100.0, 100.0), (100.0, 300.0), (0.0, 300.0)];
        let even = cut_line(&polygon(&pts, &[10.0; 6]));
        assert!((area(&even) - 62_400.0).abs() < 1e-3, "{}", area(&even));
        let mixed = cut_line(&polygon(&pts, &[10.0, 10.0, 20.0, 5.0, 10.0, 10.0]));
        assert!(mixed.iter().any(|q| q.distance(p(105.0, 120.0)) < 1e-3), "{mixed:?}");
    }

    #[test]
    fn no_allowance_leaves_the_edge_as_it_is() {
        let cut = cut_line(&polygon(&[(0.0, 0.0), (100.0, 0.0), (100.0, 200.0), (0.0, 200.0)], &[10.0, 10.0, 10.0, 0.0]));
        let min_x = cut.iter().map(|q| q.x).fold(f64::MAX, f64::min);
        assert!(min_x.abs() < 1e-6, "{min_x}");
        let none = polygon(&[(0.0, 0.0), (100.0, 0.0), (0.0, 100.0)], &[0.0; 3]);
        assert_eq!(cut_line(&none).len(), 3);
    }

    #[test]
    fn a_sharp_tip_is_squared_off() {
        let a = 5f64.to_radians();
        let tip = polygon(&[(0.0, 0.0), (300.0 * a.cos(), -300.0 * a.sin()), (300.0 * a.cos(), 300.0 * a.sin())], &[10.0; 3]);
        let reach = cut_line(&tip).iter().filter(|q| q.x < 0.0).map(|q| -q.x).fold(0.0, f64::max);
        assert!(reach <= 2.5 * 10.0, "{reach}");
    }

    #[test]
    fn a_tight_inward_curve_is_bridged_never_cut_into() {
        // A 100 mm square whose top edge dips 30 mm into the piece in a narrow curve.
        let mut piece = Piece::rectangle(PieceId(1), "Bite", p(0.0, 0.0), 100.0, 100.0);
        // Edge 2 runs (100,100)→(0,100); its control points pull it down into a narrow dip.
        piece.edges[2] = opendrape_core::Edge::Curve { c1: p(52.0, 55.0), c2: p(48.0, 55.0) };
        let cut = cut_line(&piece);
        for q in &cut {
            assert!(to_outline(&piece, *q) > 10.0 - 0.15, "{q:?} is {} from the stitching", to_outline(&piece, *q));
            assert!(!crate::contains(&piece, *q), "{q:?} inside the piece");
        }
    }

    #[test]
    fn every_cut_point_keeps_its_distance() {
        let mut piece = Piece::rectangle(PieceId(1), "R", p(0.0, 0.0), 300.0, 400.0);
        piece.set_curved(1, true);
        piece.set_handle(1, opendrape_core::HandleEnd::Start, p(380.0, 100.0));
        for q in cut_line(&piece) {
            assert!(to_outline(&piece, q) > 10.0 - 0.15, "{q:?}");
        }
    }
}
```

`crates/geom/src/marks.rs` (tests at the bottom):

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use opendrape_core::{Edge, NotchStyle, PieceId, Point2};

    fn p(x: f64, y: f64) -> Point2 {
        Point2::new(x, y)
    }

    fn square() -> Piece {
        Piece::rectangle(PieceId(1), "S", p(0.0, 0.0), 100.0, 100.0)
    }

    fn close(a: Point2, b: Point2) {
        assert!(a.distance(b) < 1e-6, "{a:?} vs {b:?}");
    }

    #[test]
    fn a_slit_sits_on_the_cut_line_and_points_in() {
        let marks = notch_marks(&square(), &Notch::new(0, 40.0));
        assert_eq!(marks.len(), 1);
        close(marks[0][0], p(40.0, -10.0)); // on the cut line, 10 mm out
        close(marks[0][1], p(40.0, -5.0)); // 5 mm deep
    }

    #[test]
    fn marks_are_spaced_and_vs_have_two_lines() {
        let double = Notch { marks: 2, ..Notch::new(0, 40.0) };
        let m = notch_marks(&square(), &double);
        close(m[0][0], p(38.5, -10.0));
        close(m[1][0], p(41.5, -10.0));
        let v = Notch { style: NotchStyle::V, ..Notch::new(0, 40.0) };
        let m = notch_marks(&square(), &v);
        assert_eq!(m.len(), 2);
        close(m[0][1], m[1][1]); // both legs meet at the tip
    }

    #[test]
    fn shallow_allowance_and_none() {
        let mut s = square();
        s.allowance = 4.0;
        let m = notch_marks(&s, &Notch::new(0, 40.0));
        close(m[0][1], p(40.0, -4.0 + 2.4)); // 60 % of 4 mm deep
        s.allowance = 0.0;
        let m = notch_marks(&s, &Notch::new(0, 40.0));
        close(m[0][0], p(40.0, 0.0));
        close(m[0][1], p(40.0, 5.0));
    }

    #[test]
    fn a_notch_past_the_end_sits_at_the_end() {
        let m = notch_marks(&square(), &Notch::new(0, 250.0));
        close(m[0][0], p(100.0, -10.0));
    }

    #[test]
    fn distances_along_edges() {
        let s = square();
        assert!((distance_along(&s, 0, 0.25) - 25.0).abs() < 1e-6);
        close(point_at_distance(&s, 1, 30.0), p(100.0, 30.0));
    }

    #[test]
    fn internal_line_geometry() {
        let mut s = square();
        s.lines = vec![
            InternalLine::open(&[p(10.0, 20.0), p(50.0, 20.0), p(50.0, 50.0)]),
            InternalLine::polygon(&[p(60.0, 60.0), p(80.0, 60.0), p(70.0, 80.0)]),
        ];
        assert!((line_length(&s.lines[0]) - 70.0).abs() < 1e-6);
        let open = line_points(&s.lines[0], 0.1);
        close(open[0], p(10.0, 20.0));
        close(*open.last().unwrap(), p(50.0, 50.0));
        let closed = line_points(&s.lines[1], 0.1);
        close(*closed.last().unwrap(), p(60.0, 60.0)); // back to the start
        let (line, edge, t, d) = nearest_line(&s, p(30.0, 23.0)).unwrap();
        assert_eq!((line, edge), (0, 0));
        assert!((t - 0.5).abs() < 1e-6 && (d - 3.0).abs() < 1e-6);
        s.lines[0].edges[0] = Edge::Curve { c1: p(20.0, 40.0), c2: p(40.0, 40.0) };
        assert!(line_length(&s.lines[0]) > 70.0);
    }

    #[test]
    fn labels_go_outside_in_either_winding() {
        let s = square();
        assert!(is_counter_clockwise(&s));
        let (at, out) = edge_label_anchor(&s, 0);
        close(at, p(50.0, 0.0));
        close(out, p(0.0, -1.0));
        let twin = s.reflected(p(300.0, 0.0));
        assert!(!is_counter_clockwise(&twin));
        let (_, out) = edge_label_anchor(&twin, 0);
        close(out, p(0.0, -1.0));
    }
}
```

Run: `cargo nextest run -p opendrape-geom`
Expected: compile errors (the functions don't exist yet).

- [ ] **Step 3: Implement `allowance.rs`** (above its tests)

```rust
//! The cut line: where the fabric is cut, outside the stitching line by each edge's seam
//! allowance. It is always worked out from the stitching line and never stored.

use crate::{edge_points, is_counter_clockwise};
use i_overlay::core::fill_rule::FillRule;
use i_overlay::float::simplify::SimplifyShape;
use opendrape_core::{Piece, Point2};

/// A corner is mitered (pointed) unless the point would land more than this many allowance
/// widths from the corner; then it is squared off.
const MITER_LIMIT: f64 = 2.5;
/// How closely (mm) curved edges are followed when working out the cut line.
const TOLERANCE_MM: f64 = 0.1;

/// The cut line: every edge pushed outwards by its seam allowance, with mitered corners
/// (squared beyond [`MITER_LIMIT`]) and, at the ends of a hem, the neighbouring seam mirrored
/// about the hem's stitching line so the hem folds up flat against it. Inward curves tighter
/// than the allowance are bridged straight across. Closed; the first point is not repeated.
/// With no allowance anywhere it is the stitching line.
pub fn cut_line(piece: &Piece) -> Vec<Point2> {
    let n = piece.len();
    let edges: Vec<(Vec<Point2>, f64, bool)> = (0..n)
        .map(|i| (edge_points(piece, i, TOLERANCE_MM), piece.edge_allowance(i).max(0.0), piece.edge_props[i].hem))
        .collect();
    let sew: Vec<Point2> = edges.iter().flat_map(|(pts, _, _)| pts[..pts.len() - 1].iter().copied()).collect();
    if edges.iter().all(|(_, w, _)| *w == 0.0) {
        return sew;
    }
    let ccw = is_counter_clockwise(piece);
    let out = |d: Point2| if ccw { Point2::new(d.y, -d.x) } else { Point2::new(-d.y, d.x) };

    struct Seg {
        end: Point2,
        d: Point2,
        w: f64,
        edge: usize,
    }
    let mut segs: Vec<Seg> = Vec::new();
    for (edge, (pts, w, _)) in edges.iter().enumerate() {
        for s in pts.windows(2) {
            let d = s[1] - s[0];
            let len = d.length();
            if len > 1e-12 {
                segs.push(Seg { end: s[1], d: d * (1.0 / len), w: *w, edge });
            }
        }
    }
    let mut raw: Vec<Point2> = Vec::new();
    let m = segs.len();
    for k in 0..m {
        let (s, t) = (&segs[k], &segs[(k + 1) % m]);
        let v = s.end;
        let pa = v + out(s.d) * s.w;
        let pb = v + out(t.d) * t.w;
        let (hem_a, hem_b) = (edges[s.edge].2, edges[t.edge].2);
        if s.edge != t.edge && hem_a != hem_b {
            // The seam's cut line, continued to the hem's stitching line and then mirrored
            // about it, so the hem folds up flat against the seam allowance.
            let (seam_p, seam_d, hem_d, hem_off) = if hem_b { (pa, s.d, t.d, pb) } else { (pb, t.d, s.d, pa) };
            if let Some(at_hem) = intersect(seam_p, seam_d, v, hem_d) {
                let mirrored = reflect(at_hem + seam_d, v, hem_d) - at_hem;
                if let Some(on_cut) = intersect(at_hem, mirrored, hem_off, hem_d) {
                    if hem_b {
                        raw.extend([pa, at_hem, on_cut, pb]);
                    } else {
                        raw.extend([pa, on_cut, at_hem, pb]);
                    }
                    continue;
                }
            }
        }
        let turn = cross(s.d, t.d) * if ccw { 1.0 } else { -1.0 };
        match intersect(pa, s.d, pb, t.d) {
            None => raw.extend([pa, pb]),
            Some(miter) if turn > 0.0 && miter.distance(v) > MITER_LIMIT * s.w.max(t.w) => {
                raw.extend([pa, pa + s.d * s.w, pb - t.d * t.w, pb]);
            }
            Some(meet) => raw.push(meet),
        }
    }
    // The raw path crosses itself where curves are tighter than the allowance; its union with
    // the stitching polygon, by the non-zero rule, has the true cut line as its outer contour.
    let as_array = |pts: &[Point2]| -> Vec<[f64; 2]> {
        let mut v: Vec<[f64; 2]> = pts.iter().map(|q| [q.x, q.y]).collect();
        if !ccw {
            v.reverse();
        }
        v
    };
    let parts = vec![as_array(&sew), as_array(&raw)];
    parts
        .simplify_shape(FillRule::NonZero)
        .into_iter()
        .filter_map(|shape| shape.into_iter().next())
        .max_by(|a, b| contour_area(a).total_cmp(&contour_area(b)))
        .map(|c| c.into_iter().map(|[x, y]| Point2::new(x, y)).collect())
        .unwrap_or(sew)
}

fn cross(a: Point2, b: Point2) -> f64 {
    a.x * b.y - a.y * b.x
}

/// Where the line through `p` along `d` meets the line through `q` along `e`.
fn intersect(p: Point2, d: Point2, q: Point2, e: Point2) -> Option<Point2> {
    let den = cross(d, e);
    if den.abs() < 1e-12 {
        return None;
    }
    let t = cross(q - p, e) / den;
    Some(p + d * t)
}

/// Mirror image of `x` across the line through `o` along unit direction `u`.
fn reflect(x: Point2, o: Point2, u: Point2) -> Point2 {
    let v = x - o;
    let along = u * (v.x * u.x + v.y * u.y);
    o + along * 2.0 - v
}

fn contour_area(c: &[[f64; 2]]) -> f64 {
    (0..c.len())
        .map(|k| {
            let (a, b) = (c[k], c[(k + 1) % c.len()]);
            a[0] * b[1] - b[0] * a[1]
        })
        .sum::<f64>()
        .abs()
}
```

**Precision:** `i_overlay` snaps coordinates to an integer grid, so results differ from exact values by around 1e-6 of the shape's size. The tests compare with tolerances accordingly.

- [ ] **Step 4: Implement `marks.rs`** (above its tests)

```rust
//! Notches and internal lines as drawable geometry, and where edge labels go.

use crate::{ACCURACY, edge_length, edge_seg, flatten, outline_points};
use kurbo::{BezPath, CubicBez, Line, ParamCurve, ParamCurveArclen, ParamCurveNearest, PathSeg, Point};
use opendrape_core::{Edge, InternalLine, Notch, NotchStyle, Piece, Point2};

/// How deep a notch is cut (mm), or 60 % of a shallower allowance.
pub const NOTCH_DEPTH_MM: f64 = 5.0;
/// Gap between the marks of a double or triple notch (mm).
pub const NOTCH_SPACING_MM: f64 = 3.0;
/// Half the width of a V notch at the cut line (mm).
const V_HALF_WIDTH_MM: f64 = 1.5;

/// Whether the outline runs counter-clockwise (y up). A twin's outline runs the other way.
pub fn is_counter_clockwise(piece: &Piece) -> bool {
    let pts = outline_points(piece, 0.5);
    (0..pts.len())
        .map(|k| {
            let (a, b) = (pts[k], pts[(k + 1) % pts.len()]);
            a.x * b.y - b.x * a.y
        })
        .sum::<f64>()
        > 0.0
}

/// Unit normal pointing out of the piece for direction `d` along its outline.
fn outward(piece_ccw: bool, d: Point2) -> Point2 {
    let len = d.length().max(1e-12);
    let n = if piece_ccw { Point2::new(d.y, -d.x) } else { Point2::new(-d.y, d.x) };
    n * (1.0 / len)
}

/// Point and unit direction `distance` mm along edge `edge` (clamped to the edge).
fn along(piece: &Piece, edge: usize, distance: f64) -> (Point2, Point2) {
    let seg = edge_seg(piece, edge);
    let len = seg.arclen(ACCURACY);
    let t = if len < 1e-12 { 0.0 } else { seg.inv_arclen(distance.clamp(0.0, len), ACCURACY) };
    let (a, b) = (seg.eval((t - 1e-4).max(0.0)), seg.eval((t + 1e-4).min(1.0)));
    let d = Point2::new(b.x - a.x, b.y - a.y);
    let p = seg.eval(t);
    (Point2::new(p.x, p.y), d * (1.0 / d.length().max(1e-12)))
}

/// The short lines that draw `notch`: on the cut line and pointing inwards (on the stitching
/// line when the edge has no allowance), one line per mark for a slit and two for a V.
pub fn notch_marks(piece: &Piece, notch: &Notch) -> Vec<[Point2; 2]> {
    let ccw = is_counter_clockwise(piece);
    let w = piece.edge_allowance(notch.edge);
    let depth = if w > 0.0 { NOTCH_DEPTH_MM.min(0.6 * w) } else { NOTCH_DEPTH_MM };
    let marks = notch.marks.max(1);
    let mut lines = Vec::new();
    for k in 0..marks {
        let shift = (f64::from(k) - f64::from(marks - 1) / 2.0) * NOTCH_SPACING_MM;
        let (p, d) = along(piece, notch.edge, notch.distance + shift);
        let n = outward(ccw, d);
        let base = p + n * w;
        let tip = base - n * depth;
        match notch.style {
            NotchStyle::Slit => lines.push([base, tip]),
            NotchStyle::V => {
                lines.push([base + d * V_HALF_WIDTH_MM, tip]);
                lines.push([base - d * V_HALF_WIDTH_MM, tip]);
            }
        }
    }
    lines
}

/// How far (mm) curve parameter `t` is along edge `edge` from its start.
pub fn distance_along(piece: &Piece, edge: usize, t: f64) -> f64 {
    edge_seg(piece, edge).subsegment(0.0..t.clamp(0.0, 1.0)).arclen(ACCURACY)
}

/// The point `distance` mm along edge `edge` from its start (clamped to the edge).
pub fn point_at_distance(piece: &Piece, edge: usize, distance: f64) -> Point2 {
    along(piece, edge, distance).0
}

fn kp(p: Point2) -> Point {
    Point::new(p.x, p.y)
}

fn line_seg(line: &InternalLine, i: usize) -> PathSeg {
    let (a, b) = line.edge_ends(i);
    match line.edges[i] {
        Edge::Line => PathSeg::Line(Line::new(kp(a), kp(b))),
        Edge::Curve { c1, c2 } => PathSeg::Cubic(CubicBez::new(kp(a), kp(c1), kp(c2), kp(b))),
    }
}

/// Points along an internal line within `tolerance` mm: from its start to its end, and for a
/// closed line back to the start again.
pub fn line_points(line: &InternalLine, tolerance: f64) -> Vec<Point2> {
    let mut path = BezPath::new();
    path.move_to(kp(line.vertices[0].pos));
    for i in 0..line.edge_count() {
        match line_seg(line, i) {
            PathSeg::Line(l) => path.line_to(l.p1),
            PathSeg::Cubic(c) => path.curve_to(c.p1, c.p2, c.p3),
            PathSeg::Quad(q) => path.quad_to(q.p1, q.p2),
        }
    }
    flatten(&path, tolerance)
}

/// Total length (mm) of an internal line.
pub fn line_length(line: &InternalLine) -> f64 {
    (0..line.edge_count()).map(|i| line_seg(line, i).arclen(ACCURACY)).sum()
}

/// The internal line of `piece` nearest to `p`: (line, edge, curve parameter, distance mm).
pub fn nearest_line(piece: &Piece, p: Point2) -> Option<(usize, usize, f64, f64)> {
    piece
        .lines
        .iter()
        .enumerate()
        .flat_map(|(l, line)| {
            (0..line.edge_count()).map(move |i| {
                let n = line_seg(line, i).nearest(kp(p), ACCURACY);
                (l, i, n.t, n.distance_sq.sqrt())
            })
        })
        .min_by(|a, b| a.3.total_cmp(&b.3))
}

/// Where edge `i`'s length label goes: the point halfway along the edge, and the unit normal
/// there pointing out of the piece.
pub fn edge_label_anchor(piece: &Piece, i: usize) -> (Point2, Point2) {
    let half = edge_length(piece, i) / 2.0;
    let (p, d) = along(piece, i, half);
    (p, outward(is_counter_clockwise(piece), d))
}
```

In `crates/geom/src/lib.rs`:
- make `ACCURACY`, `edge_seg` and `flatten` `pub(crate)`;
- add `mod allowance; mod marks;`;
- add these re-exports:

```rust
pub use allowance::cut_line;
pub use marks::{
    NOTCH_DEPTH_MM, NOTCH_SPACING_MM, distance_along, edge_label_anchor, is_counter_clockwise,
    line_length, line_points, nearest_line, notch_marks, point_at_distance,
};
```

- [ ] **Step 5: Run and see them pass**

Run: `cargo nextest run -p opendrape-geom`
Expected: 8 allowance and 7 marks tests pass, plus the earlier ones.

If `a_sharp_tip_is_squared_off` measures exactly `25.0000001`, treat it as `i_overlay` grid rounding: compare against `2.5 * 10.0 + 1e-4` and note it in the report.

- [ ] **Step 6: Commit**

```bash
cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings && cargo deny check
git add Cargo.toml Cargo.lock crates/geom
git commit -m "feat(geom): cut line with mitered, squared and hem-mirrored corners; notch, line and label geometry

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---
### Task 4: The pattern table draws and edits shapes: whole folds, twins, allowance, notches, lines

**Files:**
- Create:
  - `crates/app/src/editor/cache.rs`
  - `crates/app/tests/common/mod.rs` (shared helpers, moved out of `editor.rs`)
  - `crates/app/tests/details.rs`
- Modify:
  - `crates/app/src/editor/{mod.rs,paint.rs,canvas.rs}`
  - `crates/app/i18n/en-US/opendrape.ftl`
  - `crates/app/tests/editor.rs` (use `common`)

**Interfaces:**
- Consumes: Task 1 (`Project::owner`, `owner_mut`, `remove_piece` with twins, `Piece::translate` keeping twins, `Twin`); Task 2 (`geom::shapes`, `Shape`, `ShapeKind`, mapping methods); Task 3 (`cut_line`, `notch_marks`, `line_points`, `edge_label_anchor`).
- Produces:
  - `pub show_allowance: bool` on `PatternEditor` (default true; kept across `set_project` like `show_lengths`), with the toolbar checkbox **Show seam allowance**.
  - `cache::ShapeCache::shapes(&Project, zoom) -> Vec<Rc<Drawn>>` and `cache::MAX_DRAWN_POINTS = 4_000`.
  - Selections may now name twins: `Selection::{Piece,Vertex,Edge}(id, i)` take a shape id (a piece's or a twin's), with `i` a **stored** index. `Selection::validated` resolves the id with `Project::owner`.
  - `canvas::Hit` gains nothing new. Hits now come from shapes: a pale-half edge or vertex counts as `Inside`.
  - `canvas::nearest_edge` now returns `(shape: PieceId, source: PieceId, stored_edge: usize, t: f64, shown: Point2)`.
- Later tasks rely on:
  - `fn shapes(&self) -> Vec<geom::Shape>`, a private helper on `PatternEditor`: `geom::shapes(self.doc.project())`;
  - `pub(super) fn source_of(&self, id: PieceId) -> Option<PieceId>`.

**Behaviour:**
- **Folded piece:** drawn whole. The pale half's fill is lighter and its outline paler. The fold is a dashed line with a two-headed arrow across its middle and the label "Place on fold".
- **Twins:**
  - drawn like any piece, with their own name;
  - a small link badge (two overlapping rings) appears beside the name of both members of a pair;
  - dragging a twin's point, handle or edge edits the stored piece through the mirror;
  - dragging inside a twin moves only the twin (its offset);
  - dragging inside the stored piece moves only it.
- **Seam allowance** (when **Show seam allowance** is on): a light band out to the cut line, and the cut line drawn thin.
- **Notches:** drawn as their mark lines.
- **Internal lines:** Markings dashed, Cut-outs solid.
- **Edge-length labels:** placed just outside the piece. They sit 10 screen points beyond the stitching line, or beyond the cut line when the allowance is shown. Pale-half edges get no label.
- **Fill, outline, cut line, notch marks and line points:** cached per shape. They are recomputed only when the shape's content (its JSON) or the zoom's power-of-two bracket changes. Outlines with more than 4,000 points are thinned for drawing only.

- [ ] **Step 1: Share the test helpers**

Create `crates/app/tests/common/mod.rs`:
- Move into it, from `crates/app/tests/editor.rs`: `type H`, `harness`, `at`, `button`, `click`, `shift_click`, `drag`, `key`, `cmd`, `close`, `type_number`, `with_rectangle`, `SQUARE`, `piece_of`, `field_text`, `type_into` and `untouched_rectangle`. Each becomes `pub`, with its doc comment kept.
- Add these at the top:

```rust
//! Helpers shared by the pattern-window tests: clicks, drags and typing given in pattern
//! millimetres. Each test file uses a different subset of them.
#![allow(dead_code)]
```

Plus the `use` lines they need. In `editor.rs`, replace the moved items with:

```rust
mod common;
use common::*;
```

Run: `cargo nextest run -p opendrape --test editor`
Expected: every test still passes. This step only moves code.

- [ ] **Step 2: Strings** (append to the `.ftl`)

```
toolbar-show-allowance = Show seam allowance
fold-label = Place on fold
```

- [ ] **Step 3: Failing tests**

`crates/app/tests/details.rs`:

```rust
//! M2b on the pattern table: cut-on-fold pieces, mirrored pairs, seam allowance, notches and
//! internal lines, driven the way a student would.

mod common;
use common::*;
use egui::Key;
use egui_kittest::kittest::Queryable;
use opendrape::editor::{Selection, Tool};
use opendrape_core::{Piece, PieceId, Point2};
use opendrape_geom as geom;

/// A 150 × 300 mm half piece at (300,100), folded on its left edge (x = 300): its pale half
/// covers x = 150..300.
fn with_half(h: &mut H) -> PieceId {
    let id = h.state_mut().doc.edit(|p| {
        let mut half = Piece::rectangle(PieceId(0), "Front", Point2::new(300.0, 100.0), 150.0, 300.0);
        half.fold = Some(3);
        p.add_piece(half)
    });
    h.run();
    id
}

/// The `with_rectangle` piece (100,100)–(400,500), paired with a twin at x = 450..750.
fn with_pair(h: &mut H) -> (PieceId, PieceId) {
    let id = with_rectangle(h);
    let twin = h
        .state_mut()
        .doc
        .edit(|p| p.add_twin(id, "Front (mirror)".into(), Point2::new(850.0, 0.0)))
        .unwrap();
    h.run();
    (id, twin)
}

fn twin_vertex(h: &H, id: PieceId, k: usize) -> Point2 {
    h.state().doc.project().piece(id).unwrap().twin_shape().unwrap().vertices[k].pos
}

#[test]
fn a_folded_piece_is_drawn_whole_and_its_pale_half_selects_the_piece() {
    let mut h = harness();
    let id = with_half(&mut h);
    click(&mut h, 225.0, 250.0); // the pale half
    assert_eq!(h.state().selection, Selection::Piece(id));
    drag(&mut h, (450.0, 100.0), (470.0, 100.0)); // the drawn half's outer corner
    let full = geom::unfolded(&piece_of(&h, id));
    close(full.vertices[5].pos, Point2::new(130.0, 100.0)); // its mirror moved too
    assert_eq!(h.state().selection, Selection::Vertex(id, 1));
}

#[test]
fn dragging_the_pale_half_moves_the_whole_piece() {
    let mut h = harness();
    let id = with_half(&mut h);
    drag(&mut h, (225.0, 250.0), (235.0, 250.0));
    close(piece_of(&h, id).vertices[0].pos, Point2::new(310.0, 100.0));
}

#[test]
fn a_twin_moves_on_its_own_and_its_edits_reach_the_piece() {
    let mut h = harness();
    let (id, twin) = with_pair(&mut h);
    close(twin_vertex(&h, id, 1), Point2::new(450.0, 100.0)); // (400,100) mirrored: 850 - 400
    drag(&mut h, (450.0, 100.0), (470.0, 120.0)); // a twin corner
    close(piece_of(&h, id).vertices[1].pos, Point2::new(380.0, 120.0)); // mirrored back
    assert_eq!(h.state().selection, Selection::Vertex(twin, 1));
    let before = piece_of(&h, id);
    drag(&mut h, (600.0, 300.0), (620.0, 310.0)); // inside the twin
    assert_eq!(piece_of(&h, id).vertices, before.vertices, "the piece stays");
    close(twin_vertex(&h, id, 0), Point2::new(770.0, 110.0)); // offset now (870, 10)
    let twin_before = twin_vertex(&h, id, 0);
    drag(&mut h, (200.0, 300.0), (210.0, 300.0)); // inside the piece
    close(twin_vertex(&h, id, 0), twin_before);
    cmd(&mut h, Key::Z);
    cmd(&mut h, Key::Z);
    cmd(&mut h, Key::Z);
    close(piece_of(&h, id).vertices[1].pos, Point2::new(400.0, 100.0));
}

#[test]
fn clicking_and_deleting_a_twin() {
    let mut h = harness();
    let (id, twin) = with_pair(&mut h);
    click(&mut h, 600.0, 300.0);
    assert_eq!(h.state().selection, Selection::Piece(twin));
    key(&mut h, Key::Delete);
    assert!(piece_of(&h, id).twin.is_none(), "the twin goes, the piece stays");
    assert_eq!(h.state().selection, Selection::None);
}

#[test]
fn undoing_a_pair_drops_a_twin_selection() {
    let mut h = harness();
    let (_, twin) = with_pair(&mut h);
    click(&mut h, 600.0, 300.0);
    assert_eq!(h.state().selection, Selection::Piece(twin));
    cmd(&mut h, Key::Z); // takes the twin away
    assert_eq!(h.state().selection, Selection::None);
}

#[test]
fn add_point_works_on_a_twin_edge() {
    let mut h = harness();
    let (id, _) = with_pair(&mut h);
    key(&mut h, Key::X);
    click(&mut h, 600.0, 101.0); // the twin's bottom edge, halfway
    assert_eq!(piece_of(&h, id).len(), 5);
    close(piece_of(&h, id).vertices[1].pos, Point2::new(250.0, 100.0));
}

#[test]
fn seam_allowance_can_be_hidden() {
    let mut h = harness();
    with_rectangle(&mut h);
    assert!(h.state().show_allowance);
    h.get_by_label("Show seam allowance").click();
    h.run();
    assert!(!h.state().show_allowance);
}

#[test]
fn everything_draws_without_trouble() {
    let mut h = harness();
    let (id, _) = with_pair(&mut h);
    let half = with_half(&mut h);
    h.state_mut().doc.edit(|p| {
        let piece = p.piece_mut(id).unwrap();
        piece.notches = vec![opendrape_core::Notch::new(0, 50.0)];
        piece.lines = vec![opendrape_core::InternalLine::open(&[Point2::new(150.0, 200.0), Point2::new(300.0, 200.0)])];
        piece.edge_props[0].hem = true;
    });
    for sel in [Selection::Piece(id), Selection::Edge(half, 1), Selection::Vertex(half, 2)] {
        h.state_mut().selection = sel;
        h.run();
        assert_eq!(h.state().selection, sel);
    }
}
```

Also add to `crates/app/src/editor/cache.rs` (tests at the bottom):

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use opendrape_core::{Edge, Piece, PieceId, Point2, Project};

    fn project_with(piece: Piece) -> Project {
        let mut p = Project::new();
        p.add_piece(piece);
        p
    }

    #[test]
    fn shapes_are_reused_until_they_change() {
        let mut cache = ShapeCache::default();
        let mut project = project_with(Piece::rectangle(PieceId(0), "R", Point2::new(0.0, 0.0), 100.0, 100.0));
        cache.shapes(&project, 1.0);
        cache.shapes(&project, 1.3); // same power-of-two bracket
        assert_eq!(cache.computed, 1);
        project.pieces[0].translate(Point2::new(1.0, 0.0));
        cache.shapes(&project, 1.3);
        assert_eq!(cache.computed, 2);
        cache.shapes(&project, 2.5); // a new bracket: finer outlines
        assert_eq!(cache.computed, 3);
    }

    #[test]
    fn huge_shapes_are_cached_and_capped() {
        // A within-limits but deliberately heavy piece: 2,000 wildly curved edges.
        let corners: Vec<Point2> = (0..2000)
            .map(|k| {
                let a = k as f64 / 2000.0 * std::f64::consts::TAU;
                Point2::new(5000.0 * a.cos(), 5000.0 * a.sin())
            })
            .collect();
        let mut piece = Piece::polygon(PieceId(0), "Heavy", &corners);
        for i in 0..piece.len() {
            let (a, b) = piece.edge_ends(i);
            piece.edges[i] = Edge::Curve { c1: a + Point2::new(900.0, -900.0), c2: b + Point2::new(-900.0, 900.0) };
        }
        let project = project_with(piece);
        let mut cache = ShapeCache::default();
        let first = cache.shapes(&project, 50.0);
        assert!(first[0].outline.len() <= MAX_DRAWN_POINTS && first[0].cut.len() <= MAX_DRAWN_POINTS);
        let started = std::time::Instant::now();
        let again = cache.shapes(&project, 50.0);
        assert!(Rc::ptr_eq(&first[0], &again[0]), "not recomputed");
        assert!(started.elapsed() < std::time::Duration::from_secs(1), "{:?}", started.elapsed());
    }
}
```

Run: `cargo nextest run -p opendrape --test details && cargo nextest run -p opendrape --lib cache`
Expected: compile errors or failures (shapes aren't used yet; `show_allowance` and `cache` don't exist yet).

- [ ] **Step 4: The cache** (`crates/app/src/editor/cache.rs`, above its tests)

```rust
//! The shapes on the pattern table, with the slow parts of drawing them worked out once and
//! kept until the shape changes or the zoom passes a power of two: the fill triangles, the
//! cut line, the notch marks and the internal lines' points.

use opendrape_core::{LineKind, PieceId, Point2, Project};
use opendrape_geom::{self as geom, Shape};
use std::collections::HashMap;
use std::rc::Rc;

/// Most points an outline, cut line or internal line is drawn with; finer ones are thinned
/// for drawing only, so a deliberately huge piece can't make the window slow.
pub(super) const MAX_DRAWN_POINTS: usize = 4_000;

/// One shape ready to draw. All points are in pattern millimetres.
pub(super) struct Drawn {
    pub shape: Shape,
    pub outline: Vec<Point2>,
    /// Triangles (indices into `outline`) filling the piece.
    pub fill: Vec<u32>,
    /// The cut line and its fill triangles; empty when no edge has any allowance.
    pub cut: Vec<Point2>,
    pub cut_fill: Vec<u32>,
    pub notches: Vec<[Point2; 2]>,
    pub lines: Vec<(Vec<Point2>, LineKind)>,
}

#[derive(Default)]
pub(super) struct ShapeCache {
    entries: HashMap<PieceId, (u64, Rc<Drawn>)>,
    /// How many shapes have been worked out from scratch (for tests).
    pub computed: usize,
}

impl ShapeCache {
    /// Every shape of `project`, ready to draw at `zoom` (screen points per mm). A shape whose
    /// content and zoom bracket haven't changed since the last call is reused as it was.
    pub fn shapes(&mut self, project: &Project, zoom: f64) -> Vec<Rc<Drawn>> {
        let bracket = zoom.max(1e-9).log2().floor() as i32;
        // 0.25 to 0.5 screen points: smooth curves at every zoom.
        let tolerance = 0.25 / 2f64.powi(bracket);
        let mut kept = HashMap::new();
        let mut out = Vec::new();
        for shape in geom::shapes(project) {
            let key = fingerprint(&shape, bracket);
            let id = shape.id;
            let drawn = match self.entries.get(&id) {
                Some((k, d)) if *k == key => d.clone(),
                _ => {
                    self.computed += 1;
                    Rc::new(prepare(shape, tolerance))
                }
            };
            kept.insert(id, (key, drawn.clone()));
            out.push(drawn);
        }
        self.entries = kept;
        out
    }
}

/// Identifies a shape's content: its kind and every field of its piece, plus the zoom bracket.
fn fingerprint(shape: &Shape, bracket: i32) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    bracket.hash(&mut h);
    format!("{:?}", shape.kind).hash(&mut h);
    serde_json::to_vec(&shape.piece).unwrap_or_default().hash(&mut h);
    h.finish()
}

fn prepare(shape: Shape, tolerance: f64) -> Drawn {
    let piece = &shape.piece;
    let outline = thin(geom::outline_points(piece, tolerance));
    let fill = triangulate(&outline);
    let any_allowance = (0..piece.len()).any(|i| piece.edge_allowance(i) > 0.0);
    let cut = if any_allowance { thin(geom::cut_line(piece)) } else { Vec::new() };
    let cut_fill = triangulate(&cut);
    let notches = piece.notches.iter().flat_map(|n| geom::notch_marks(piece, n)).collect();
    let lines = piece.lines.iter().map(|l| (thin(geom::line_points(l, tolerance)), l.kind)).collect();
    Drawn { shape, outline, fill, cut, cut_fill, notches, lines }
}

/// At most [`MAX_DRAWN_POINTS`] of `points`, evenly spaced.
fn thin(points: Vec<Point2>) -> Vec<Point2> {
    if points.len() <= MAX_DRAWN_POINTS {
        return points;
    }
    let step = points.len().div_ceil(MAX_DRAWN_POINTS);
    points.into_iter().step_by(step).collect()
}

/// epaint fills only convex shapes, so pieces are triangulated (earcut) once here.
fn triangulate(points: &[Point2]) -> Vec<u32> {
    let mut triangles = Vec::new();
    if points.len() >= 3 {
        earcut::Earcut::new().earcut(points.iter().map(|p| [p.x, p.y]), &[] as &[u32], &mut triangles);
    }
    triangles
}
```

- [ ] **Step 5: Editor wiring** (`crates/app/src/editor/mod.rs`)

- Add `mod cache;`.
- New fields on `PatternEditor`:
  - `pub show_allowance: bool` (default `true`);
  - `cache: cache::ShapeCache`.
  `set_project` keeps `show_allowance` the same way as `show_lengths`.
- Toolbar: after the **Show lengths** checkbox, add `ui.checkbox(&mut self.show_allowance, tr!("toolbar-show-allowance"));`.
- `Selection::validated` resolves ids through owners:

```rust
    pub fn validated(self, project: &Project) -> Self {
        let Some(id) = self.piece() else { return Self::None };
        let Some((piece, _)) = project.owner(id) else { return Self::None };
        match self {
            Self::Vertex(_, i) | Self::Edge(_, i) if i >= piece.len() => Self::Piece(id),
            other => other,
        }
    }
```

- Add these helpers to `impl PatternEditor`:

```rust
    /// Every shape on the table (see `geom::shapes`).
    pub(super) fn shapes(&self) -> Vec<geom::Shape> {
        geom::shapes(self.doc.project())
    }
    /// The stored piece behind a shape id (the piece itself, or the one a twin mirrors).
    pub(super) fn source_of(&self, id: PieceId) -> Option<PieceId> {
        self.doc.project().owner(id).map(|(p, _)| p.id)
    }
```
  with `use opendrape_geom as geom;`.
- `project_bounds` uses every shape's outline: `geom::shapes(project).iter().flat_map(|s| geom::outline_points(&s.piece, 1.0))`. Collect the points first, then fold.

- [ ] **Step 6: Hit-testing and dragging over shapes** (`crates/app/src/editor/canvas.rs`)

- `Drag` remembers how the shape maps to the stored piece:

```rust
pub(super) struct Drag {
    /// The stored piece as it was when the drag began.
    original: Piece,
    hit: Hit,
    grab: Point2,
    /// The dragged shape's kind: a twin's movements are mirrored back onto the stored piece.
    kind: geom::ShapeKind,
}

impl Drag {
    /// The stored piece after the pointer moved `d` (in the dragged shape's coordinates).
    fn moved(&self, d: Point2) -> Piece {
        let o = &self.original;
        let mut p = o.clone();
        let twin = matches!(self.kind, geom::ShapeKind::Twin { .. });
        let ds = if twin { Point2::new(-d.x, d.y) } else { d };
        match self.hit {
            Hit::Vertex(_, i) => p.move_vertex(i, o.vertices[i].pos + ds),
            Hit::Handle(_, i, end) => {
                if let Edge::Curve { c1, c2 } = o.edges[i] {
                    let from = match end {
                        HandleEnd::Start => c1,
                        HandleEnd::End => c2,
                    };
                    p.set_handle(i, end, from + ds);
                }
            }
            Hit::Edge(_, i) => {
                let j = o.next(i);
                p.move_vertex(i, o.vertices[i].pos + ds);
                p.move_vertex(j, o.vertices[j].pos + ds);
            }
            // A twin moves on its own (its offset); the stored piece moves without its twin.
            Hit::Inside(_) if twin => {
                if let Some(t) = &mut p.twin {
                    t.offset = t.offset + d;
                }
            }
            Hit::Inside(_) => p.translate(d),
        }
        p
    }
}
```

- `hit` works over shapes. Indices are stored indices, and pale-half edges count as the inside:

```rust
    /// What the edit tool picks at `w`: the selected shape's curve handles first, then
    /// points, edges and insides, topmost shape first. Points and edges of a fold's pale half
    /// are not editable: they pick the piece.
    fn hit(&self, shapes: &[geom::Shape], w: Point2, tol: f64) -> Option<Hit> {
        if let Some(sel) = self.selection.piece()
            && let Some(s) = shapes.iter().find(|s| s.id == sel)
        {
            for (k, edge) in s.piece.edges.iter().enumerate() {
                let (Some(i), Edge::Curve { c1, c2 }) = (s.stored_edge(k), *edge) else { continue };
                if c1.distance(w) <= tol {
                    return Some(Hit::Handle(s.id, i, HandleEnd::Start));
                }
                if c2.distance(w) <= tol {
                    return Some(Hit::Handle(s.id, i, HandleEnd::End));
                }
            }
        }
        let topmost = || shapes.iter().rev();
        topmost()
            .find_map(|s| {
                (0..s.piece.len()).find_map(|k| {
                    let i = s.stored_vertex(k)?;
                    (s.piece.vertices[k].pos.distance(w) <= tol).then_some(Hit::Vertex(s.id, i))
                })
            })
            .or_else(|| {
                topmost().find_map(|s| {
                    geom::nearest_edge(&s.piece, w).filter(|e| e.2 <= tol).map(|(k, _, _)| match s.stored_edge(k) {
                        Some(i) => Hit::Edge(s.id, i),
                        None => Hit::Inside(s.id),
                    })
                })
            })
            .or_else(|| topmost().find(|s| geom::contains(&s.piece, w)).map(|s| Hit::Inside(s.id)))
    }
```

- `edit_tool` starts drags from shapes:

```rust
        if response.drag_started_by(PointerButton::Primary)
            && let Some(grab) = press
        {
            let shapes = self.shapes();
            if let Some(hit) = self.hit(&shapes, grab, tol)
                && let Some(shape) = shapes.iter().find(|s| s.id == hit.piece())
                && let Some(original) = self.doc.project().piece(shape.source).cloned()
            {
                self.selection = hit.selection();
                self.doc.begin_gesture();
                self.canvas.drag = Some(Drag { original, hit, grab, kind: shape.kind });
            }
        }
```

  - The per-frame drag applies `moved` to the stored piece: `let id = moved.id; self.doc.gesture_edit(|p| if let Some(piece) = p.piece_mut(id) { *piece = moved; });`. This already works, because `original` is the stored piece.
  - The click branch calls `self.hit(&self.shapes(), at, tol)`.

- `delete_selection`, vertex case: `p.owner_mut(id).is_some_and(|(piece, _)| geom::remove_vertex(piece, i))`. The piece case already works for twins through `Project::remove_piece`.

- `snap` collects candidate points from every shape. Replace the `vertices` iterator with:

```rust
        let shapes = geom::shapes(self.doc.project());
        let vertices = shapes.iter().flat_map(|s| s.piece.vertices.iter().map(|v| v.pos));
```

- `nearest_edge` (used by the add-point tool) works over shapes and returns where to split:

```rust
/// The editable outline edge nearest to `w` within `tol` mm, over all shapes:
/// (shape, stored piece, stored edge, curve parameter, the point on the shape).
pub(super) fn nearest_edge(project: &Project, w: Point2, tol: f64) -> Option<(PieceId, PieceId, usize, f64, Point2)> {
    geom::shapes(project)
        .iter()
        .filter_map(|s| {
            let (k, t, d) = geom::nearest_edge(&s.piece, w)?;
            let i = s.stored_edge(k)?;
            (d <= tol).then(|| (s.id, s.source, i, t, geom::point_on_edge(&s.piece, k, t), d))
        })
        .min_by(|a, b| a.5.total_cmp(&b.5))
        .map(|(id, source, i, t, at, _)| (id, source, i, t, at))
}
```

  `add_point_tool` uses `canvas.preview = hover.and_then(...).map(|h| h.4)`. On a click it splits `source`'s stored edge `i` at `t` (twins keep their edges' direction, so `t` is the same). The resulting selection is `Selection::Vertex(shape_id, v)`.

- [ ] **Step 7: Drawing shapes** (`crates/app/src/editor/paint.rs`)

`canvas_ui` gets the drawn shapes after the interactions and passes them in:

```rust
        let drawn = self.cache.shapes(self.doc.project(), self.view.zoom);
        self.paint(&painter, rect, ui.visuals().dark_mode, &drawn);
```

Add `band`, `cut`, `pale` and `pale_fill` colours to `Palette`:
- Light theme: band `from_rgba_unmultiplied(70, 110, 200, 18)`, cut `from_gray(150)`, pale `from_gray(165)`, pale_fill `from_rgba_unmultiplied(70, 110, 200, 20)`.
- Dark theme: band `(120, 160, 230, 20)`, cut `from_gray(120)`, pale `from_gray(110)`, pale_fill `(120, 160, 230, 22)`.

Replace `paint`, `paint_piece` and `paint_selection`, and the old `fill_polygon` (the cache triangulates now), with:

```rust
    pub(super) fn paint(&self, painter: &Painter, rect: Rect, dark: bool, drawn: &[Rc<Drawn>]) {
        let c = Palette::new(dark);
        painter.rect_filled(rect, 0.0, c.table);
        self.paint_grid(painter, rect, &c);
        for d in drawn {
            self.paint_shape(painter, rect, d, &c);
        }
        if let Some(d) = drawn.iter().find(|d| Some(d.shape.id) == self.selection.piece()) {
            self.paint_selection(painter, rect, d, &c);
        }
        self.paint_drafts(painter, rect, &c);
    }

    fn mesh(&self, rect: Rect, points: &[Point2], triangles: &[u32], fill: Color32) -> Shape {
        let mut mesh = egui::Mesh::default();
        for p in points {
            mesh.colored_vertex(self.view.to_screen(rect, *p), fill);
        }
        for &[a, b, t] in triangles.as_chunks::<3>().0 {
            mesh.add_triangle(a, b, t);
        }
        Shape::mesh(mesh)
    }

    fn paint_shape(&self, painter: &Painter, rect: Rect, d: &Drawn, c: &Palette) {
        let v = self.view;
        let piece = &d.shape.piece;
        let selected = self.selection.piece() == Some(d.shape.id);
        let ink = if selected { c.selected } else { c.ink };
        let width = if selected { 2.0 } else { 1.5 };
        if self.show_allowance && !d.cut.is_empty() {
            painter.add(self.mesh(rect, &d.cut, &d.cut_fill, c.band));
            painter.add(Shape::closed_line(self.screen_points(rect, d.cut.clone()), Stroke::new(1.0, c.cut)));
        }
        let folded = match d.shape.kind {
            geom::ShapeKind::Folded { drawn, fold, .. } => Some((drawn, fold)),
            _ => None,
        };
        let fill = if folded.is_some() { c.pale_fill } else { c.fill };
        painter.add(self.mesh(rect, &d.outline, &d.fill, fill));
        match folded {
            Some((drawn, fold)) => {
                painter.add(Shape::closed_line(self.screen_points(rect, d.outline.clone()), Stroke::new(1.0, c.pale)));
                for j in 0..drawn - 1 {
                    let pts = self.screen_points(rect, geom::edge_points(piece, j, v.mm(0.25)));
                    painter.add(Shape::line(pts, Stroke::new(width, ink)));
                }
                self.paint_fold(painter, rect, fold, c);
            }
            None => {
                painter.add(Shape::closed_line(self.screen_points(rect, d.outline.clone()), Stroke::new(width, ink)));
            }
        }
        for (k, vertex) in piece.vertices.iter().enumerate() {
            if d.shape.stored_vertex(k).is_some() {
                painter.circle_filled(v.to_screen(rect, vertex.pos), 3.0, ink);
            }
        }
        for [a, b] in &d.notches {
            painter.line_segment([v.to_screen(rect, *a), v.to_screen(rect, *b)], Stroke::new(1.5, ink));
        }
        for (points, kind) in &d.lines {
            let pts = self.screen_points(rect, points.clone());
            match kind {
                LineKind::Marking => {
                    painter.extend(Shape::dashed_line(&pts, Stroke::new(1.0, c.label), 5.0, 3.0));
                }
                LineKind::Cutout => {
                    painter.add(Shape::line(pts, Stroke::new(1.5, ink)));
                }
            }
        }
        // Grainline: a double-headed arrow through the middle, with the name beside it.
        let centre = v.to_screen(rect, geom::centroid(piece));
        let r = piece.grain_deg.to_radians();
        let along = vec2(r.cos() as f32, -(r.sin() as f32)) * 40.0;
        let grain = Stroke::new(1.0, c.label);
        painter.arrow(centre, along, grain);
        painter.arrow(centre, -along, grain);
        let name = painter.text(centre + vec2(6.0, -4.0), Align2::LEFT_BOTTOM, &piece.name, FontId::proportional(13.0), c.label);
        if self.is_paired(&d.shape) {
            // Link badge: two overlapping rings after the name.
            let at = name.right_center() + vec2(9.0, 0.0);
            painter.circle_stroke(at - vec2(3.0, 0.0), 4.0, grain);
            painter.circle_stroke(at + vec2(3.0, 0.0), 4.0, grain);
        }
        if self.show_lengths {
            self.paint_lengths(painter, rect, d, c);
        }
    }

    /// Whether a shape is one of a pair (a twin, or a piece that has one).
    fn is_paired(&self, shape: &geom::Shape) -> bool {
        matches!(shape.kind, geom::ShapeKind::Twin { .. })
            || self.doc.project().piece(shape.source).is_some_and(|p| p.twin.is_some())
    }

    /// The fold line: dashed, with a two-headed arrow across its middle and "Place on fold".
    fn paint_fold(&self, painter: &Painter, rect: Rect, (near, far): (Point2, Point2), c: &Palette) {
        let (a, b) = (self.view.to_screen(rect, near), self.view.to_screen(rect, far));
        painter.extend(Shape::dashed_line(&[a, b], Stroke::new(1.5, c.ink), 8.0, 4.0));
        let mid = a + (b - a) * 0.5;
        let across = (b - a).normalized().rot90() * 18.0;
        let stroke = Stroke::new(1.0, c.label);
        painter.arrow(mid, across, stroke);
        painter.arrow(mid, -across, stroke);
        painter.text(mid + across + vec2(4.0, 0.0), Align2::LEFT_CENTER, tr!("fold-label"), FontId::proportional(11.0), c.label);
    }

    /// Each editable edge's length, just outside the piece (and outside the allowance band when
    /// it is shown), so no line runs through the text.
    fn paint_lengths(&self, painter: &Painter, rect: Rect, d: &Drawn, c: &Palette) {
        let piece = &d.shape.piece;
        let units = self.doc.project().units;
        for j in 0..piece.len() {
            if d.shape.stored_edge(j).is_none() {
                continue;
            }
            let (at, out) = geom::edge_label_anchor(piece, j);
            let band = if self.show_allowance { piece.edge_allowance(j) * self.view.zoom } else { 0.0 };
            let gap = (band + 10.0) as f32;
            let pos = self.view.to_screen(rect, at) + vec2(out.x as f32, -(out.y as f32)) * gap;
            painter.text(pos, Align2::CENTER_CENTER, units.format(geom::edge_length(piece, j)), FontId::proportional(11.0), c.label);
        }
    }

    fn paint_selection(&self, painter: &Painter, rect: Rect, d: &Drawn, c: &Palette) {
        let v = self.view;
        let piece = &d.shape.piece;
        let handle = Stroke::new(1.0, c.handle);
        for (k, edge) in piece.edges.iter().enumerate() {
            if let (Some(_), Edge::Curve { c1, c2 }) = (d.shape.stored_edge(k), *edge) {
                let (a, b) = piece.edge_ends(k);
                for (end, h) in [(a, c1), (b, c2)] {
                    let (end, h) = (v.to_screen(rect, end), v.to_screen(rect, h));
                    painter.line_segment([end, h], handle);
                    painter.circle_stroke(h, 4.0, handle);
                }
            }
        }
        match self.selection {
            Selection::Edge(_, i) => {
                let k = d.shape.shape_edge(i);
                let pts = self.screen_points(rect, geom::edge_points(piece, k, v.mm(0.25)));
                painter.add(Shape::line(pts, Stroke::new(3.5, c.selected)));
            }
            Selection::Vertex(_, i) => {
                let p = v.to_screen(rect, piece.vertices[d.shape.shape_vertex(i)].pos);
                painter.circle_filled(p, 5.5, c.selected);
                painter.circle_stroke(p, 5.5, Stroke::new(1.5, c.table));
            }
            Selection::Piece(_) | Selection::None => {}
        }
    }
```

Imports in `paint.rs`: `use super::{PatternEditor, Selection, Tool, cache::Drawn, canvas::pen_piece};`, `use crate::tr;`, `use opendrape_core::{Edge, LineKind, Point2};`, `use std::rc::Rc;`.

`Pos2 - Pos2` is a `Vec2`, and `Vec2::normalized()` and `rot90()` exist in emath 0.36. If `rot90` turns out to be named differently, use `vec2(-y, x)`.

- [ ] **Step 8: Run and see them pass**

Run: `cargo nextest run -p opendrape`
Expected:
- the 8 `details` tests pass;
- the 2 cache tests pass;
- all the earlier editor tests (whose helpers moved into `common`) and app tests still pass.

- [ ] **Step 9: Commit**

```bash
cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings
git add crates/app
git commit -m "feat(editor): draw and edit whole folds, twins, allowance bands, notches and lines; labels outside; cached shapes

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---
### Task 5: Properties for allowance, hems, folds and pairs

**Files:**
- Modify: `crates/app/src/editor/panel.rs`, `crates/app/i18n/en-US/opendrape.ftl`, `crates/app/tests/details.rs`

**Interfaces:**
- Consumes:
  - Task 1: `Project::{owner, owner_mut, add_twin, break_twin}`, `Side`, `MAX_ALLOWANCE_MM`, `EdgeProps`.
  - Task 2: `geom::{shape_of, unfolded}`, `Shape::{from_stored, to_stored}`.
  - Task 4: `Selection` with shape ids, and `source_of`.
- Produces:
  - Panel sections that work for twins.
  - `apply_typed(typed, is_length, refusal: String, apply)`: the refusal message is now a parameter.
  - A pair's twin is placed 50 mm (`PAIR_GAP_MM`) to the right of its piece: `offset.x = 2·max_x + 50`, `offset.y = 0`.

**Behaviour:**
- **Piece** section (for a piece or a twin):
  - fields: Name (the twin's own name for a twin); Grain angle (a twin shows and sets the mirrored angle, 180° − g); Seam allowance (shared); Area and Perimeter of the full shape;
  - buttons:
    - a folded piece: **Unfold** and **Remove fold**;
    - a paired piece or a twin: **Break pair**;
    - otherwise: **Make mirrored pair**;
    - always: **Delete piece**;
  - a twin also shows "Mirror image of <name>".
- **Edge** section:
  - Length;
  - Seam allowance (this edge's effective value; typing sets the edge's own), with **Same as piece** shown when the edge has its own value;
  - **Hem**;
  - the Keep-fixed choice and **Curved**;
  - **Set as fold line** on a straight edge of an unfolded, unpaired piece;
  - on the fold edge itself: "This edge is the fold line." and **Remove fold**, with no allowance or hem controls.
- **Point** section: X and Y are shown where the shape shows the point (twin coordinates for a twin). Typed values are mapped back to the stored piece.
- **Refusals:**
  - an allowance outside 0–10 cm gives "The seam allowance must be between 0 and 10 cm.";
  - a fold the model refuses gives "The fold line must be a straight edge with the whole piece on one side of it."

- [ ] **Step 1: Strings** (append to the `.ftl`)

```
panel-allowance = Seam allowance
panel-allowance-reset = Same as piece
panel-hem = Hem
panel-set-fold = Set as fold line
panel-fold-line = This edge is the fold line.
panel-unfold = Unfold
panel-remove-fold = Remove fold
panel-make-pair = Make mirrored pair
panel-break-pair = Break pair
panel-twin-of = Mirror image of { $name }
twin-name = { $name } (mirror)
notice-bad-allowance = The seam allowance must be between 0 and 10 cm.
notice-fold-refused = The fold line must be a straight edge with the whole piece on one side of it.
```

- [ ] **Step 2: Failing tests** (append to `crates/app/tests/details.rs`)

```rust
#[test]
fn allowance_for_the_whole_piece_and_for_one_edge() {
    let mut h = harness();
    let id = with_rectangle(&mut h);
    click(&mut h, 250.0, 300.0);
    assert_eq!(field_text(&h, "Seam allowance"), "1.0");
    type_into(&mut h, "Seam allowance", "1,5");
    assert_eq!(piece_of(&h, id).allowance, 15.0);
    click(&mut h, 250.0, 100.0); // bottom edge
    assert_eq!(field_text(&h, "Seam allowance"), "1.5");
    type_into(&mut h, "Seam allowance", "2");
    assert_eq!(piece_of(&h, id).edge_props[0].allowance, Some(20.0));
    h.get_by_label("Same as piece").click();
    h.run();
    assert_eq!(piece_of(&h, id).edge_props[0].allowance, None);
}

#[test]
fn an_allowance_out_of_range_is_refused() {
    let mut h = harness();
    let id = with_rectangle(&mut h);
    click(&mut h, 250.0, 300.0);
    type_into(&mut h, "Seam allowance", "12"); // 12 cm
    assert_eq!(piece_of(&h, id).allowance, 10.0);
    assert!(h.state().notice.as_deref().is_some_and(|n| n.contains("between 0 and 10 cm")));
}

#[test]
fn the_hem_checkbox_gives_three_centimetres() {
    let mut h = harness();
    let id = with_rectangle(&mut h);
    click(&mut h, 250.0, 100.0);
    h.get_by_label("Hem").click();
    h.run();
    assert!(piece_of(&h, id).edge_props[0].hem);
    assert_eq!(field_text(&h, "Seam allowance"), "3.0");
}

#[test]
fn set_a_fold_then_unfold_or_remove_it() {
    let mut h = harness();
    let id = with_rectangle(&mut h); // (100,100)-(400,500)
    click(&mut h, 100.0, 300.0); // left edge
    assert_eq!(h.state().selection, Selection::Edge(id, 3));
    h.get_by_label("Set as fold line").click();
    h.run();
    assert_eq!(piece_of(&h, id).fold, Some(3));
    h.get_by_label("This edge is the fold line.");
    click(&mut h, 250.0, 300.0);
    h.get_by_label("2400.0 cm²"); // the full 60 × 40 cm piece
    h.get_by_label("Unfold").click();
    h.run();
    assert_eq!((piece_of(&h, id).len(), piece_of(&h, id).fold), (6, None));
    cmd(&mut h, Key::Z);
    assert_eq!((piece_of(&h, id).len(), piece_of(&h, id).fold), (4, Some(3)));
    h.get_by_label("Remove fold").click();
    h.run();
    assert_eq!((piece_of(&h, id).len(), piece_of(&h, id).fold), (4, None));
}

#[test]
fn a_fold_that_would_cross_the_piece_is_refused() {
    let mut h = harness();
    // A U shape: the inner edge x = 300 has parts of the piece on both sides of its line.
    let id = h.state_mut().doc.edit(|p| {
        let pts = [(100.0, 100.0), (400.0, 100.0), (400.0, 400.0), (300.0, 400.0), (300.0, 200.0), (200.0, 200.0), (200.0, 400.0), (100.0, 400.0)];
        p.add_piece(Piece::polygon(PieceId(0), "U", &pts.map(|(x, y)| Point2::new(x, y))))
    });
    h.run();
    click(&mut h, 300.0, 300.0);
    assert_eq!(h.state().selection, Selection::Edge(id, 3));
    h.get_by_label("Set as fold line").click();
    h.run();
    assert_eq!(piece_of(&h, id).fold, None);
    assert!(h.state().notice.as_deref().is_some_and(|n| n.contains("one side")));
}

#[test]
fn make_rename_and_break_a_pair() {
    let mut h = harness();
    let id = with_rectangle(&mut h);
    click(&mut h, 250.0, 300.0);
    h.get_by_label("Make mirrored pair").click();
    h.run();
    let twin = piece_of(&h, id).twin.unwrap().id;
    assert_eq!(h.state().selection, Selection::Piece(twin));
    assert_eq!(piece_of(&h, id).twin.unwrap().offset, Point2::new(850.0, 0.0));
    assert_eq!(field_text(&h, "Name"), "Front (mirror)");
    h.get_by_label("Mirror image of Front");
    type_into(&mut h, "Name", "Back right");
    assert_eq!(h.state().doc.project().name_of(twin), Some("Back right"));
    assert_eq!(piece_of(&h, id).name, "Front");
    h.get_by_label("Break pair").click();
    h.run();
    assert!(piece_of(&h, id).twin.is_none());
    assert_eq!(piece_of(&h, twin).name, "Back right");
    assert_eq!(h.state().selection, Selection::Piece(twin));
}

#[test]
fn undo_of_make_pair_drops_the_twin_selection() {
    let mut h = harness();
    with_rectangle(&mut h);
    click(&mut h, 250.0, 300.0);
    h.get_by_label("Make mirrored pair").click();
    h.run();
    cmd(&mut h, Key::Z);
    assert_eq!(h.state().selection, Selection::None);
}

#[test]
fn a_twin_point_is_shown_where_the_twin_is() {
    let mut h = harness();
    let id = with_rectangle(&mut h);
    click(&mut h, 250.0, 300.0);
    h.get_by_label("Make mirrored pair").click();
    h.run();
    let twin = piece_of(&h, id).twin.unwrap().id;
    click(&mut h, 450.0, 100.0); // the twin's image of corner 1
    assert_eq!(h.state().selection, Selection::Vertex(twin, 1));
    assert_eq!(field_text(&h, "X"), "45.0");
    type_into(&mut h, "X", "47");
    assert_eq!(piece_of(&h, id).vertices[1].pos, Point2::new(380.0, 100.0)); // 850 - 470
}
```

Run: `cargo nextest run -p opendrape --test details`
Expected: the 8 new tests fail. The labels and sections don't exist yet, and twin selections show an empty panel.

- [ ] **Step 3: Implement** (`crates/app/src/editor/panel.rs`)

- Imports: `use opendrape_core::{Edge, MAX_ALLOWANCE_MM, Piece, PieceId, Point2, Project, Side, Units, VertexKind};`.
- Add the gap constant:

```rust
/// Gap (mm) between a piece and the twin "Make mirrored pair" puts beside it.
const PAIR_GAP_MM: f64 = 50.0;
```

- `apply_typed` takes the refusal message:

```rust
    fn apply_typed(
        &mut self,
        typed: Option<String>,
        is_length: bool,
        refusal: String,
        apply: impl FnOnce(&mut Project, f64) -> bool,
    ) {
        let Some(text) = typed else { return };
        let units = self.doc.project().units;
        let value = Units::parse(&text).map(|v| if is_length { units.to_mm(v) } else { v });
        let Some(value) = value else {
            self.notice = Some(refusal);
            return;
        };
        let applied = self.doc.edit(|p| apply(p, value));
        if !self.note_if_refused() && !applied {
            self.notice = Some(refusal);
        }
    }
```

  Existing calls pass `tr!("notice-bad-number")`.

- Replace `piece_properties`, `edge_properties` and `vertex_properties`, and add the actions:

```rust
    fn piece_properties(&mut self, ui: &mut egui::Ui, id: PieceId) {
        let project = self.doc.project();
        let Some((piece, side)) = project.owner(id).map(|(p, s)| (p.clone(), s)) else { return };
        let Some(shape) = geom::shape_of(project, id) else { return };
        let units = project.units;
        let source = piece.id;
        ui.strong(tr!("panel-piece"));
        if side == Side::Twin {
            ui.label(tr!("panel-twin-of", name = piece.name.clone()));
        }
        egui::Grid::new("piece_properties").num_columns(3).show(ui, |ui| {
            if let Some(name) = self.field(ui, tr!("panel-name"), &shape.piece.name, "") {
                let name = name.trim().to_owned();
                if name.is_empty() {
                    self.notice = Some(tr!("notice-name-empty"));
                } else {
                    self.doc.edit(|p| match p.owner_mut(id) {
                        Some((pc, Side::Master)) => pc.name = name,
                        Some((pc, Side::Twin)) => {
                            if let Some(t) = &mut pc.twin {
                                t.name = name;
                            }
                        }
                        None => {}
                    });
                    self.note_if_refused();
                }
            }
            let grain = self.field(ui, tr!("panel-grain"), &format!("{:.1}", shape.piece.grain_deg), "°");
            self.apply_typed(grain, false, tr!("notice-bad-number"), |p, deg| {
                p.piece_mut(source).is_some_and(|pc| {
                    // A twin shows its piece's grain mirrored: 180° − angle.
                    let stored = if side == Side::Twin { 180.0 - deg } else { deg };
                    pc.grain_deg = stored.rem_euclid(360.0);
                    true
                })
            });
            let allowance = self.field(ui, tr!("panel-allowance"), &units.format_number(piece.allowance), units.suffix());
            self.apply_typed(allowance, true, tr!("notice-bad-allowance"), |p, mm| {
                allowance_ok(mm)
                    && p.piece_mut(source).is_some_and(|pc| {
                        pc.allowance = mm;
                        true
                    })
            });
            ui.label(tr!("panel-area"));
            ui.label(units.format_area(geom::area(&shape.piece)));
            ui.end_row();
            ui.label(tr!("panel-perimeter"));
            ui.label(units.format(geom::perimeter(&shape.piece)));
            ui.end_row();
        });
        ui.add_space(6.0);
        if piece.fold.is_some() {
            ui.horizontal(|ui| {
                if ui.button(tr!("panel-unfold")).clicked() {
                    self.unfold(source);
                }
                if ui.button(tr!("panel-remove-fold")).clicked() {
                    self.remove_fold(source);
                }
            });
        } else if piece.twin.is_some() {
            if ui.button(tr!("panel-break-pair")).clicked() {
                self.break_pair(source);
            }
        } else if ui.button(tr!("panel-make-pair")).clicked() {
            self.make_pair(&piece);
        }
        if ui.button(tr!("panel-delete-piece")).clicked() {
            self.delete_selection();
        }
    }

    fn edge_properties(&mut self, ui: &mut egui::Ui, id: PieceId, i: usize) {
        let Some((piece, side)) = self.doc.project().owner(id).map(|(p, s)| (p.clone(), s)) else { return };
        let units = self.doc.project().units;
        let source = piece.id;
        let anchor = self.panel.anchor;
        let is_fold = piece.fold == Some(i);
        ui.strong(tr!("panel-edge"));
        egui::Grid::new("edge_properties").num_columns(3).show(ui, |ui| {
            let length = units.format_number(geom::edge_length(&piece, i));
            let typed = self.field(ui, tr!("panel-length"), &length, units.suffix());
            self.apply_typed(typed, true, tr!("notice-bad-number"), |p, mm| {
                p.piece_mut(source).is_some_and(|pc| geom::set_edge_length(pc, i, mm, anchor))
            });
            if !is_fold {
                let own = units.format_number(piece.edge_allowance(i));
                let typed = self.field(ui, tr!("panel-allowance"), &own, units.suffix());
                self.apply_typed(typed, true, tr!("notice-bad-allowance"), |p, mm| {
                    allowance_ok(mm)
                        && p.piece_mut(source).is_some_and(|pc| {
                            pc.edge_props[i].allowance = Some(mm);
                            true
                        })
                });
            }
        });
        ui.label(tr!("panel-keep-fixed"));
        ui.horizontal(|ui| {
            ui.radio_value(&mut self.panel.anchor, Anchor::Start, tr!("panel-anchor-start"));
            ui.radio_value(&mut self.panel.anchor, Anchor::End, tr!("panel-anchor-end"));
        });
        if is_fold {
            ui.label(tr!("panel-fold-line"));
            if ui.button(tr!("panel-remove-fold")).clicked() {
                self.remove_fold(source);
            }
            return;
        }
        if piece.edge_props[i].allowance.is_some() && ui.button(tr!("panel-allowance-reset")).clicked() {
            self.doc.edit(|p| {
                if let Some(pc) = p.piece_mut(source) {
                    pc.edge_props[i].allowance = None;
                }
            });
            self.note_if_refused();
        }
        let mut hem = piece.edge_props[i].hem;
        if ui.checkbox(&mut hem, tr!("panel-hem")).changed() {
            self.doc.edit(|p| {
                if let Some(pc) = p.piece_mut(source) {
                    pc.edge_props[i].hem = hem;
                }
            });
            self.note_if_refused();
        }
        let mut curved = matches!(piece.edges[i], Edge::Curve { .. });
        if ui.checkbox(&mut curved, tr!("panel-curved")).changed() {
            self.doc.edit(|p| {
                if let Some(pc) = p.piece_mut(source) {
                    pc.set_curved(i, curved);
                }
            });
            self.note_if_refused();
        }
        let can_fold = side == Side::Master && piece.twin.is_none() && piece.fold.is_none() && piece.edges[i] == Edge::Line;
        if can_fold && ui.button(tr!("panel-set-fold")).clicked() {
            self.doc.edit(|p| {
                if let Some(pc) = p.piece_mut(source) {
                    pc.fold = Some(i);
                }
            });
            if self.note_if_refused() {
                self.notice = Some(tr!("notice-fold-refused"));
            }
        }
    }

    fn vertex_properties(&mut self, ui: &mut egui::Ui, id: PieceId, i: usize) {
        let project = self.doc.project();
        let Some((piece, _)) = project.owner(id).map(|(p, s)| (p.clone(), s)) else { return };
        let Some(shape) = geom::shape_of(project, id) else { return };
        let units = project.units;
        let source = piece.id;
        // Shown (and typed) where this shape shows the point: a twin's image for a twin.
        let shown = shape.from_stored(piece.vertices[i].pos);
        let move_to = move |p: &mut Project, to_shown: Point2| {
            let to = shape.to_stored(to_shown);
            to.x.abs() <= MAX_COORDINATE_MM
                && to.y.abs() <= MAX_COORDINATE_MM
                && p.piece_mut(source).is_some_and(|pc| {
                    pc.move_vertex(i, to);
                    true
                })
        };
        ui.strong(tr!("panel-point"));
        egui::Grid::new("vertex_properties").num_columns(3).show(ui, |ui| {
            let x = self.field(ui, tr!("panel-x"), &units.format_number(shown.x), units.suffix());
            self.apply_typed(x, true, tr!("notice-bad-number"), |p, mm| move_to(p, Point2::new(mm, shown.y)));
            let y = self.field(ui, tr!("panel-y"), &units.format_number(shown.y), units.suffix());
            self.apply_typed(y, true, tr!("notice-bad-number"), |p, mm| move_to(p, Point2::new(shown.x, mm)));
        });
        let mut smooth = piece.vertices[i].kind == VertexKind::Smooth;
        if ui.checkbox(&mut smooth, tr!("panel-smooth")).changed() {
            let kind = if smooth { VertexKind::Smooth } else { VertexKind::Corner };
            self.doc.edit(|p| {
                if let Some(pc) = p.piece_mut(source) {
                    pc.set_vertex_kind(i, kind);
                }
            });
            self.note_if_refused();
        }
        ui.add_space(6.0);
        if ui.button(tr!("panel-delete-point")).clicked() {
            self.delete_selection();
        }
    }

    fn make_pair(&mut self, piece: &Piece) {
        let name = tr!("twin-name", name = piece.name.clone());
        let offset = twin_offset_beside(piece);
        let source = piece.id;
        let twin = self.doc.edit(|p| p.add_twin(source, name, offset));
        if !self.note_if_refused()
            && let Some(t) = twin
        {
            self.selection = Selection::Piece(t);
        }
    }

    fn break_pair(&mut self, source: PieceId) {
        let piece = self.doc.edit(|p| p.break_twin(source));
        if !self.note_if_refused()
            && let Some(t) = piece
        {
            self.selection = Selection::Piece(t);
        }
    }

    fn unfold(&mut self, source: PieceId) {
        self.doc.edit(|p| {
            if let Some(pc) = p.piece_mut(source) {
                let full = geom::unfolded(pc);
                *pc = full;
            }
        });
        self.note_if_refused();
    }

    fn remove_fold(&mut self, source: PieceId) {
        self.doc.edit(|p| {
            if let Some(pc) = p.piece_mut(source) {
                pc.fold = None;
            }
        });
        self.note_if_refused();
    }
```

  `move_to` captures `shape` (a `Shape`, which is `Clone`) by move. Both `apply_typed` closures borrow `move_to` by reference, as in M2a.

- Add the helpers:

```rust
/// A seam allowance the pattern can hold: 0 to [`MAX_ALLOWANCE_MM`].
fn allowance_ok(mm: f64) -> bool {
    mm.is_finite() && (0.0..=MAX_ALLOWANCE_MM).contains(&mm)
}

/// The offset that puts a piece's mirror image [`PAIR_GAP_MM`] to its right, at the same
/// height (see `opendrape_core::Twin`).
fn twin_offset_beside(piece: &Piece) -> Point2 {
    let max_x = geom::outline_points(piece, 1.0).iter().map(|p| p.x).fold(f64::MIN, f64::max);
    Point2::new(2.0 * max_x + PAIR_GAP_MM, 0.0)
}
```

- The existing `MAX_COORDINATE_MM` const in `panel.rs` (100 m) shadows core's name. Rename it to `MAX_TYPED_COORDINATE_MM`; it was a deferred minor in M2a.

- [ ] **Step 4: Run and see them pass**

Run: `cargo nextest run -p opendrape`
Expected: the 8 new tests pass, plus all earlier tests (the editor panel tests are unchanged).

- [ ] **Step 5: Commit**

```bash
cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings
git add crates/app
git commit -m "feat(editor): seam allowance, hem, fold and mirrored-pair controls in the properties panel

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---
### Task 6: The Notch tool

**Files:**
- Create: `crates/app/src/editor/notch_tool.rs`
- Modify:
  - `crates/app/src/editor/{mod.rs,canvas.rs,length_box.rs,panel.rs,paint.rs}`
  - `crates/app/i18n/en-US/opendrape.ftl`
  - `crates/app/tests/details.rs`

**Interfaces:**
- Consumes: Task 3 (`distance_along`, `edge_length`, `notch_marks`); Task 4 (`canvas::nearest_edge` returning shape, source, stored edge, t and the shown point; `Hit`; `Drawn`).
- Produces:
  - `Tool::Notch`, key **N**. `Tool::ALL` gains it after `AddPoint`.
  - `Selection::Notch(PieceId, usize)`: a shape id and a **stored** notch index. `piece()` returns its id, and `validated` drops it to `Piece(id)` when the index is gone.
  - `canvas::Hit::Notch(PieceId, usize)`, and `pub(super) enum Placed` made visible to sibling modules.
  - `BoxKind::NotchDistance { shape: PieceId, source: PieceId, edge: usize, from_end: bool }`, a one-field box labelled **Distance**.
  - `pub(super) fn take_typed_digits(ui) -> String` in `canvas.rs`, shared by the pen and notch boxes.
  - `PatternEditor::add_notch(shape, source, edge, distance)`.

**Behaviour:**
- **Placing:** in the Notch tool, a click within 8 points of an editable edge adds a single slit notch there. Its distance is the arc length from the edge's start, and it becomes the selection.
- **Typing a distance:** with the pointer near an edge, typing a digit opens the box. The distance is measured from whichever end is nearer the pointer (`from_end` when the curve parameter > 0.5). Distances outside 0…edge length are refused with "Type a distance from 0 up to the length of the edge."
- **Fold edges:** they lie inside the full shape, so they can't receive notches.
- **Edit tool:**
  - clicking a notch mark selects it, checked before handles and points (it lies outside the outline, on the cut line);
  - notches are not dragged: a drag starting on one does nothing;
  - Delete removes the selected notch.
- **Notch panel:**
  - Distance (from the edge's start), **Marks**: Single / Double / Triple, **Style**: Slit / V;
  - **Delete notch**.
- **Highlight:** the selected notch is drawn thicker in the selection colour.

- [ ] **Step 1: Strings** (append to the `.ftl`)

```
tool-notch = Notch
tool-notch-tip = Add notches: marks that show where pieces line up when they are sewn.
hint-notch = Click on an edge to add a notch, or point at an edge and type its distance from the nearer end.
box-distance = Distance
panel-notch = Notch
panel-notch-marks = Marks
panel-notch-single = Single
panel-notch-double = Double
panel-notch-triple = Triple
panel-notch-style = Style
panel-notch-slit = Slit
panel-notch-v = V
panel-delete-notch = Delete notch
notice-bad-notch = Type a distance from 0 up to the length of the edge.
```

- [ ] **Step 2: Failing tests** (append to `crates/app/tests/details.rs`; add `use egui::accesskit::Role;` and `use opendrape_core::{Notch, NotchStyle};`)

```rust
#[test]
fn the_notch_tool_adds_a_notch_where_clicked() {
    let mut h = harness();
    let id = with_rectangle(&mut h);
    key(&mut h, Key::N);
    assert_eq!(h.state().tool, Tool::Notch);
    click(&mut h, 250.0, 101.0); // bottom edge, halfway
    let notches = piece_of(&h, id).notches;
    assert_eq!(notches.len(), 1);
    assert_eq!(notches[0].edge, 0);
    assert!((notches[0].distance - 150.0).abs() < 0.5, "{}", notches[0].distance);
    assert_eq!(h.state().selection, Selection::Notch(id, 0));
}

#[test]
fn a_typed_notch_distance_counts_from_the_nearer_end() {
    let mut h = harness();
    let id = with_rectangle(&mut h);
    key(&mut h, Key::N);
    let p = at(&h, 380.0, 101.0); // near the bottom edge's right end
    h.hover_at(p);
    h.run();
    type_number(&mut h, "5");
    assert!(h.get_by_role_and_label(Role::TextInput, "Distance").is_focused());
    key(&mut h, Key::Enter);
    assert_eq!(piece_of(&h, id).notches, vec![Notch::new(0, 250.0)]); // 300 - 50
}

#[test]
fn a_notch_distance_past_the_edge_is_refused() {
    let mut h = harness();
    let id = with_rectangle(&mut h);
    key(&mut h, Key::N);
    let p = at(&h, 380.0, 101.0);
    h.hover_at(p);
    h.run();
    type_number(&mut h, "5");
    h.get_by_role_and_label(Role::TextInput, "Distance").type_text("00"); // 500 cm
    h.run();
    key(&mut h, Key::Enter);
    assert!(piece_of(&h, id).notches.is_empty());
    assert!(h.state().notice.is_some());
}

#[test]
fn the_notch_panel_changes_marks_style_and_distance() {
    let mut h = harness();
    let id = with_rectangle(&mut h);
    h.state_mut().doc.edit(|p| p.piece_mut(id).unwrap().notches.push(Notch::new(0, 150.0)));
    h.run();
    click(&mut h, 250.0, 92.0); // on the mark, out on the cut line
    assert_eq!(h.state().selection, Selection::Notch(id, 0));
    h.get_by_label("Double").click();
    h.run();
    h.get_by_label("V").click();
    h.run();
    type_into(&mut h, "Distance", "10");
    assert_eq!(piece_of(&h, id).notches, vec![Notch { edge: 0, distance: 100.0, marks: 2, style: NotchStyle::V }]);
}

#[test]
fn notches_on_a_twin_belong_to_its_piece() {
    let mut h = harness();
    let (id, twin) = with_pair(&mut h);
    key(&mut h, Key::N);
    click(&mut h, 600.0, 101.0); // the twin's bottom edge, halfway
    let notches = piece_of(&h, id).notches;
    assert_eq!(notches.len(), 1);
    assert!((notches[0].distance - 150.0).abs() < 0.5);
    assert_eq!(h.state().selection, Selection::Notch(twin, 0));
}

#[test]
fn selection_of_a_removed_notch_is_dropped() {
    let mut h = harness();
    let id = with_rectangle(&mut h);
    key(&mut h, Key::N);
    click(&mut h, 250.0, 101.0);
    cmd(&mut h, Key::Z);
    assert!(piece_of(&h, id).notches.is_empty());
    assert_eq!(h.state().selection, Selection::Piece(id));
}

#[test]
fn delete_removes_the_selected_notch() {
    let mut h = harness();
    let id = with_rectangle(&mut h);
    h.state_mut().doc.edit(|p| p.piece_mut(id).unwrap().notches.push(Notch::new(0, 150.0)));
    h.run();
    click(&mut h, 250.0, 92.0);
    key(&mut h, Key::Delete);
    assert!(piece_of(&h, id).notches.is_empty());
    assert_eq!(h.state().selection, Selection::Piece(id));
}

#[test]
fn a_drag_starting_on_a_notch_moves_nothing() {
    let mut h = harness();
    let id = with_rectangle(&mut h);
    h.state_mut().doc.edit(|p| p.piece_mut(id).unwrap().notches.push(Notch::new(0, 150.0)));
    h.run();
    let before = piece_of(&h, id);
    drag(&mut h, (250.0, 92.0), (250.0, 60.0));
    assert_eq!(piece_of(&h, id), before);
}
```

Run: `cargo nextest run -p opendrape --test details`
Expected: compile errors (`Tool::Notch` and `Selection::Notch` don't exist yet).

- [ ] **Step 3: Implement**

`mod.rs`:
- Add `mod notch_tool;`.
- `Tool::Notch` with key `Key::N`; `label`/`tip` use `tool-notch`/`tool-notch-tip`; `Tool::ALL` becomes `[Edit, Pen, Rectangle, AddPoint, Notch]`.
- `Selection::Notch(PieceId, usize)`; `piece()` matches it.
- `validated` adds an arm: `Self::Notch(_, k) if k >= piece.notches.len() => Self::Piece(id),`.

`length_box.rs`: add the `BoxKind::NotchDistance { shape: PieceId, source: PieceId, edge: usize, from_end: bool }` variant (import `PieceId`), and make the second field optional:

```rust
        let (first_label, second) = match self.kind {
            BoxKind::PenSegment => (tr!("box-length"), Some((tr!("box-angle"), "°"))),
            BoxKind::Rectangle(_) => (tr!("box-width"), Some((tr!("box-height"), units.suffix()))),
            BoxKind::NotchDistance { .. } => (tr!("box-distance"), None),
        };
```

Inside the `ui.horizontal` closure, `b` becomes an `Option<egui::Response>`:

```rust
                        let b = second.map(|(label, unit)| {
                            let lb = ui.label(label);
                            let mut out = egui::TextEdit::singleline(&mut self.second).desired_width(48.0).show(ui);
                            select_all_on_focus(ui.ctx(), &mut out, self.second.chars().count());
                            let b = out.response.response.labelled_by(lb.id);
                            ui.label(unit);
                            b
                        });
                        if focus {
                            a.request_focus();
                        }
                        for r in std::iter::once(&a).chain(b.as_ref()) {
                            any_focus |= r.has_focus();
                            if r.lost_focus() {
                                lost = true;
                                enter |= ui.input(|i| i.key_pressed(Key::Enter));
                            }
                        }
```

`canvas.rs`:
- Make `enum Placed` `pub(super)`.
- Extract the digit-taking half of `open_box_on_digits` into:

```rust
/// Takes the digits (and decimal marks) typed this frame out of the input, so the number box
/// created later this frame doesn't get them twice. Returns them, or "" if none were typed.
pub(super) fn take_typed_digits(ui: &egui::Ui) -> String {
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
    typed
}
```

  `open_box_on_digits` calls it.
- `Hit::Notch(PieceId, usize)`, with `piece()` and `selection()` arms (`Selection::Notch(id, k)`).
- In `hit`, check notches first, right after the selected shape's handles:

```rust
        // Notch marks sit out on the cut line, so they never hide a point or an edge.
        for s in shapes.iter().rev() {
            let stored = self.doc.project().owner(s.id).map_or(0, |(p, _)| p.notches.len());
            for (j, notch) in s.piece.notches.iter().enumerate() {
                let near = geom::notch_marks(&s.piece, notch).iter().any(|[a, b]| segment_distance(w, *a, *b) <= tol);
                if near && stored > 0 {
                    // A fold's pale half repeats the stored notches after them, in order.
                    return Some(Hit::Notch(s.id, j % stored));
                }
            }
        }
```

  with:

```rust
/// Distance (mm) from `p` to the segment `a`–`b`.
fn segment_distance(p: Point2, a: Point2, b: Point2) -> f64 {
    let ab = b - a;
    let len2 = ab.x * ab.x + ab.y * ab.y;
    let t = if len2 < 1e-18 { 0.0 } else { (((p.x - a.x) * ab.x + (p.y - a.y) * ab.y) / len2).clamp(0.0, 1.0) };
    p.distance(a + ab * t)
}
```

- `edit_tool`: don't start a drag when the hit is a notch. Add `&& !matches!(hit, Hit::Notch(..))` to the drag-start `if let` chain. `Drag::moved` gets the arm `Hit::Notch(..) => {}`.
- `canvas_ui`'s tool match: `Tool::Notch => self.notch_tool(&response, hover, pointer, tol),`.
- `canvas_keys`: `Tool::Notch => self.notch_box_on_digits(ui, response),` before the `_` arm.
- `commit_box` gets the arm:

```rust
            BoxKind::NotchDistance { shape, source, edge, from_end } => {
                let len = self.doc.project().piece(source).map_or(0.0, |p| geom::edge_length(p, edge));
                let d = Units::parse(&number_box.first)
                    .map(|v| units.to_mm(v))
                    .filter(|d| d.is_finite() && (0.0..=len).contains(d));
                match d {
                    Some(d) => self.add_notch(shape, source, edge, if from_end { len - d } else { d }),
                    None => self.notice = Some(tr!("notice-bad-notch")),
                }
            }
```

- `delete_selection` gets the arm:

```rust
            Selection::Notch(id, k) => {
                self.doc.edit(|p| {
                    if let Some((pc, _)) = p.owner_mut(id)
                        && k < pc.notches.len()
                    {
                        pc.notches.remove(k);
                    }
                });
                if !self.note_if_refused() {
                    self.selection = Selection::Piece(id);
                }
            }
```

`notch_tool.rs`:

```rust
//! The Notch tool (N): click on an edge to add a notch there, or point at an edge and type a
//! distance to place it exactly that far from the nearer end.

use super::canvas::{nearest_edge, take_typed_digits};
use super::length_box::{BoxKind, LengthBox};
use super::{HIT_PX, PatternEditor, Selection};
use egui::{Response, vec2};
use opendrape_core::{Notch, PieceId, Point2};
use opendrape_geom as geom;

impl PatternEditor {
    pub(super) fn notch_tool(&mut self, response: &Response, hover: Option<Point2>, pointer: Option<Point2>, tol: f64) {
        let project = self.doc.project();
        self.canvas.preview = hover.and_then(|w| nearest_edge(project, w, tol)).map(|hit| hit.4);
        if response.clicked()
            && let Some(at) = pointer
            && let Some((shape, source, edge, t, _)) = nearest_edge(self.doc.project(), at, tol)
            && let Some(piece) = self.doc.project().piece(source)
        {
            let distance = geom::distance_along(piece, edge, t);
            self.add_notch(shape, source, edge, distance);
        }
    }

    /// Adds a single slit notch `distance` mm along stored edge `edge` of `source`, and selects
    /// it on `shape` (the piece, or its twin).
    pub(super) fn add_notch(&mut self, shape: PieceId, source: PieceId, edge: usize, distance: f64) {
        let index = self.doc.edit(|p| {
            p.piece_mut(source).map(|pc| {
                pc.notches.push(Notch::new(edge, distance));
                pc.notches.len() - 1
            })
        });
        if !self.note_if_refused()
            && let Some(k) = index
        {
            self.selection = Selection::Notch(shape, k);
        }
    }

    /// With the pointer near an edge, a typed digit opens the number box for the notch's
    /// distance from the edge's nearer end. Digits typed away from any edge are left alone.
    pub(super) fn notch_box_on_digits(&mut self, ui: &egui::Ui, response: &Response) {
        let tol = self.view.mm(HIT_PX);
        let Some(hover) = response.hover_pos().map(|p| self.view.to_world(self.canvas_rect, p)) else { return };
        let Some((shape, source, edge, t, _)) = nearest_edge(self.doc.project(), hover, tol) else { return };
        let typed = take_typed_digits(ui);
        if typed.is_empty() {
            return;
        }
        let pos = response.hover_pos().unwrap_or(response.rect.center()) + vec2(16.0, 16.0);
        let kind = BoxKind::NotchDistance { shape, source, edge, from_end: t > 0.5 };
        self.canvas.length_box = Some(LengthBox::new(kind, typed, String::new(), pos));
    }
}
```

`panel.rs`: `properties()` routes `Selection::Notch(id, k) => self.notch_properties(ui, id, k)`.

```rust
    fn notch_properties(&mut self, ui: &mut egui::Ui, id: PieceId, k: usize) {
        let Some((piece, _)) = self.doc.project().owner(id).map(|(p, s)| (p.clone(), s)) else { return };
        let Some(notch) = piece.notches.get(k).copied() else { return };
        let units = self.doc.project().units;
        let source = piece.id;
        let len = geom::edge_length(&piece, notch.edge);
        ui.strong(tr!("panel-notch"));
        egui::Grid::new("notch_properties").num_columns(3).show(ui, |ui| {
            let typed = self.field(ui, tr!("box-distance"), &units.format_number(notch.distance), units.suffix());
            self.apply_typed(typed, true, tr!("notice-bad-notch"), |p, mm| {
                (0.0..=len).contains(&mm)
                    && p.piece_mut(source).is_some_and(|pc| {
                        pc.notches[k].distance = mm;
                        true
                    })
            });
        });
        ui.label(tr!("panel-notch-marks"));
        ui.horizontal(|ui| {
            for (marks, label) in [(1, tr!("panel-notch-single")), (2, tr!("panel-notch-double")), (3, tr!("panel-notch-triple"))] {
                if ui.radio(notch.marks == marks, label).clicked() && notch.marks != marks {
                    self.edit_notch(source, k, |n| n.marks = marks);
                }
            }
        });
        ui.label(tr!("panel-notch-style"));
        ui.horizontal(|ui| {
            for (style, label) in [(NotchStyle::Slit, tr!("panel-notch-slit")), (NotchStyle::V, tr!("panel-notch-v"))] {
                if ui.radio(notch.style == style, label).clicked() && notch.style != style {
                    self.edit_notch(source, k, |n| n.style = style);
                }
            }
        });
        ui.add_space(6.0);
        if ui.button(tr!("panel-delete-notch")).clicked() {
            self.delete_selection();
        }
    }
```

with:

```rust
    /// Changes notch `k` of `source` as one undo step.
    fn edit_notch(&mut self, source: PieceId, k: usize, change: impl FnOnce(&mut Notch)) {
        self.doc.edit(|p| {
            if let Some(pc) = p.piece_mut(source)
                && let Some(n) = pc.notches.get_mut(k)
            {
                change(n);
            }
        });
        self.note_if_refused();
    }
```

(import `Notch` and `NotchStyle` from `opendrape_core` in `panel.rs`).

The `hint()` match gets `Tool::Notch => tr!("hint-notch")`.

`paint.rs`: in `paint_selection`, `Selection::Notch(_, k)` draws `geom::notch_marks(piece, &piece.notches[k])` with `Stroke::new(3.0, c.selected)`. Guard it with `if k < piece.notches.len()`. Also extend the add-point preview ring condition to `Tool::AddPoint | Tool::Notch`.

- [ ] **Step 4: Run and see them pass**

Run: `cargo nextest run -p opendrape`
Expected: the 8 new tests pass, plus all earlier ones.

- [ ] **Step 5: Commit**

```bash
cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings
git add crates/app
git commit -m "feat(editor): notch tool with typed distances, notch selection and panel

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 7: The Internal line tool

**Files:**
- Create: `crates/app/src/editor/line_tool.rs`
- Modify:
  - `crates/app/src/editor/{mod.rs,canvas.rs,panel.rs,paint.rs}`
  - `crates/app/i18n/en-US/opendrape.ftl`
  - `crates/app/tests/details.rs`

**Interfaces:**
- Consumes: Task 1 (`InternalLine`, `LineKind`); Task 3 (`line_length`, `nearest_line`); Task 4 (shapes, `Hit`, `Drag`); `canvas::{pen_piece, Placed, PenPoint}`.
- Produces:
  - `Tool::Line`, key **L**, placed after `Notch` in `Tool::ALL`.
  - `Selection::Line(PieceId, usize)`: a shape id and a stored line index.
  - `Hit::Line(PieceId, usize)` and `Hit::LineVertex(PieceId, usize, usize)`.
  - `pub fn line_draft(&self) -> &[PenPoint]`.
  - `CanvasState` fields `line: Vec<PenPoint>`, `line_owner: Option<PieceId>` and `line_dragging: bool`.

**Behaviour:**
- **Drawing:**
  - Works like the pen, but inside a piece: click points, and press and drag for a curve point.
  - Clicking the first point (with ≥ 3 points) closes a shape. Clicking the last point again, or Return, finishes an open line (≥ 2 points).
  - Backspace or Cmd+Z removes the last point; Esc cancels.
- **Which piece:** the first point decides, so it must be inside a shape or on its outline (within 0.5 mm). Every later point must be inside the same shape. Points outside are refused with "Internal lines must stay inside their piece."
- **Storage:**
  - Lines are stored on the stored piece. A twin's points are mapped back through the mirror.
  - A line drawn on a fold's pale half is stored as drawn; its mirror image shows on the drawn half.
- **Edit tool:**
  - Drags a line's points (with their curve handles) and whole lines.
  - A drag that would take any point of the line outside the piece is not applied: the line stays where it last was inside.
  - Lines on a fold's pale half aren't editable; they select the piece.
  - Delete removes the selected line.
- **Line panel:**
  - Length;
  - **Kind**: Marking / Cut-out, where Cut-out is only enabled for closed lines;
  - **Delete line**.

- [ ] **Step 1: Strings** (append to the `.ftl`)

```
tool-line = Internal line
tool-line-tip = Draw lines inside a piece: placement marks, fold lines, or shapes to cut out.
hint-line-start = Click inside a piece to start a line; press and drag for a curve point.
hint-line-drawing = Click the first point to close the shape, or press Return to finish the line. Backspace removes the last point; Esc cancels.
notice-line-outside = Internal lines must stay inside their piece.
notice-line-short = A line needs at least 2 points, and a closed shape 3.
panel-line = Internal line
panel-line-length = Length: { $length }
panel-line-kind = Kind
panel-line-marking = Marking
panel-line-cutout = Cut-out
panel-delete-line = Delete line
```

- [ ] **Step 2: Failing tests** (append to `crates/app/tests/details.rs`; add `use opendrape_core::{InternalLine, LineKind};`)

```rust
#[test]
fn the_line_tool_draws_an_open_line() {
    let mut h = harness();
    let id = with_rectangle(&mut h);
    key(&mut h, Key::L);
    assert_eq!(h.state().tool, Tool::Line);
    click(&mut h, 150.0, 200.0);
    click(&mut h, 300.0, 200.0);
    key(&mut h, Key::Enter);
    let lines = piece_of(&h, id).lines;
    assert_eq!(lines.len(), 1);
    assert!(!lines[0].closed);
    close(lines[0].vertices[1].pos, Point2::new(300.0, 200.0));
    assert_eq!(h.state().selection, Selection::Line(id, 0));
}

#[test]
fn clicking_the_first_point_closes_a_shape_that_can_be_cut_out() {
    let mut h = harness();
    let id = with_rectangle(&mut h);
    key(&mut h, Key::L);
    for (x, y) in [(150.0, 200.0), (300.0, 200.0), (220.0, 350.0), (150.0, 200.0)] {
        click(&mut h, x, y);
    }
    assert!(piece_of(&h, id).lines[0].closed);
    h.get_by_label("Cut-out").click();
    h.run();
    assert_eq!(piece_of(&h, id).lines[0].kind, LineKind::Cutout);
}

#[test]
fn points_outside_the_piece_are_refused() {
    let mut h = harness();
    with_rectangle(&mut h);
    key(&mut h, Key::L);
    click(&mut h, 150.0, 200.0);
    click(&mut h, 600.0, 200.0); // outside the piece
    assert_eq!(h.state().line_draft().len(), 1);
    assert!(h.state().notice.is_some());
}

#[test]
fn undo_while_drawing_a_line_removes_its_last_point() {
    let mut h = harness();
    let id = with_rectangle(&mut h);
    key(&mut h, Key::L);
    click(&mut h, 150.0, 200.0);
    click(&mut h, 300.0, 200.0);
    cmd(&mut h, Key::Z);
    assert_eq!(h.state().line_draft().len(), 1);
    assert!(piece_of(&h, id).lines.is_empty());
}

#[test]
fn the_edit_tool_moves_lines_and_their_points_but_keeps_them_inside() {
    let mut h = harness();
    let id = with_rectangle(&mut h);
    h.state_mut().doc.edit(|p| {
        p.piece_mut(id).unwrap().lines.push(InternalLine::open(&[Point2::new(150.0, 200.0), Point2::new(300.0, 200.0)]))
    });
    h.run();
    drag(&mut h, (225.0, 200.0), (225.0, 250.0)); // the whole line
    close(piece_of(&h, id).lines[0].vertices[0].pos, Point2::new(150.0, 250.0));
    assert_eq!(h.state().selection, Selection::Line(id, 0));
    drag(&mut h, (150.0, 250.0), (160.0, 260.0)); // one point
    close(piece_of(&h, id).lines[0].vertices[0].pos, Point2::new(160.0, 260.0));
    drag(&mut h, (160.0, 260.0), (700.0, 260.0)); // out of the piece: not applied
    close(piece_of(&h, id).lines[0].vertices[0].pos, Point2::new(160.0, 260.0));
}

#[test]
fn a_line_on_a_folded_piece_shows_on_both_halves() {
    let mut h = harness();
    let id = with_half(&mut h);
    key(&mut h, Key::L);
    click(&mut h, 350.0, 200.0);
    click(&mut h, 420.0, 200.0);
    key(&mut h, Key::Enter);
    assert_eq!(piece_of(&h, id).lines.len(), 1);
    let shape = geom::shape_of(h.state().doc.project(), id).unwrap();
    assert_eq!(shape.piece.lines.len(), 2);
}

#[test]
fn selection_of_a_removed_line_is_dropped() {
    let mut h = harness();
    let id = with_rectangle(&mut h);
    key(&mut h, Key::L);
    click(&mut h, 150.0, 200.0);
    click(&mut h, 300.0, 200.0);
    key(&mut h, Key::Enter);
    cmd(&mut h, Key::Z);
    assert!(piece_of(&h, id).lines.is_empty());
    assert_eq!(h.state().selection, Selection::Piece(id));
}

#[test]
fn delete_removes_the_selected_line() {
    let mut h = harness();
    let id = with_rectangle(&mut h);
    key(&mut h, Key::L);
    click(&mut h, 150.0, 200.0);
    click(&mut h, 300.0, 200.0);
    key(&mut h, Key::Enter);
    key(&mut h, Key::Z); // edit tool
    key(&mut h, Key::Delete);
    assert!(piece_of(&h, id).lines.is_empty());
}
```

Run: `cargo nextest run -p opendrape --test details`
Expected: compile errors (`Tool::Line`, `Selection::Line` and `line_draft` don't exist yet).

- [ ] **Step 3: Implement**

`mod.rs`:
- Add `mod line_tool;`.
- `Tool::Line` with key `Key::L`; `Tool::ALL` gains it after `Notch`.
- `Selection::Line(PieceId, usize)`; `piece()` matches it.
- `validated`: `Self::Line(_, l) if l >= piece.lines.len() => Self::Piece(id),`.
- `pub fn line_draft(&self) -> &[PenPoint] { &self.canvas.line }`.
- `undo()` pops a line draft point before a pen point or the history:

```rust
    pub fn undo(&mut self) {
        if self.canvas.line.pop().is_some() {
            if self.canvas.line.is_empty() {
                self.canvas.line_owner = None;
            }
        } else if self.canvas.pen.pop().is_none() {
            self.canvas.drag = None;
            self.doc.undo();
        }
        self.selection = self.selection.validated(self.doc.project());
    }
```

  `can_undo` also counts `!self.canvas.line.is_empty()`, and `can_redo` requires the line draft to be empty.

`canvas.rs`:
- New `CanvasState` fields: `pub line: Vec<PenPoint>`, `pub line_owner: Option<PieceId>`, `pub line_dragging: bool`.
- `Hit::Line(PieceId, usize)` and `Hit::LineVertex(PieceId, usize, usize)`. `piece()` returns the id; `selection()` returns `Selection::Line(id, l)` for both.
- In `hit`, after the outline-vertex search and before the outline-edge search, look for line points and then lines. A fold's pale copies have indices at or beyond the stored count and are skipped:

```rust
            .or_else(|| {
                topmost().find_map(|s| {
                    let stored = self.doc.project().owner(s.id).map_or(0, |(p, _)| p.lines.len());
                    s.piece.lines.iter().take(stored).enumerate().find_map(|(l, line)| {
                        line.vertices.iter().position(|v| v.pos.distance(w) <= tol).map(|k| Hit::LineVertex(s.id, l, k))
                    })
                })
            })
```

  After the outline-edge search, add:

```rust
            .or_else(|| {
                topmost().find_map(|s| {
                    let stored = self.doc.project().owner(s.id).map_or(0, |(p, _)| p.lines.len());
                    geom::nearest_line(&s.piece, w)
                        .filter(|hit| hit.3 <= tol && hit.0 < stored)
                        .map(|(l, ..)| Hit::Line(s.id, l))
                })
            })
```

- `Drag::moved` gets these arms, using `ds`, the movement mapped to the stored piece:

```rust
            Hit::LineVertex(_, l, k) => {
                let to = o.lines[l].vertices[k].pos + ds;
                move_line_vertex(&mut p.lines[l], k, to);
                if !line_inside(&p, l) {
                    return o.clone();
                }
            }
            Hit::Line(_, l) => {
                p.lines[l].translate(ds);
                if !line_inside(&p, l) {
                    return o.clone();
                }
            }
```

- `canvas_ui`'s tool match: `Tool::Line => self.line_tool(&response, press, pointer, tol),`.
- `canvas_keys`:

```rust
            Tool::Line if !self.canvas.line.is_empty() => {
                if pressed(Key::Enter) {
                    self.finish_line(false);
                }
                if pressed(Key::Escape) {
                    self.canvas.line.clear();
                    self.canvas.line_owner = None;
                }
                if pressed(Key::Backspace) || pressed(Key::Delete) {
                    self.canvas.line.pop();
                    if self.canvas.line.is_empty() {
                        self.canvas.line_owner = None;
                    }
                }
            }
```

- `delete_selection`: `Selection::Line(id, l)` removes `lines[l]` from the owner (same shape as the notch arm), then selects `Selection::Piece(id)`.

`line_tool.rs`:

```rust
//! The Internal line tool (L): draws a line inside a piece the way the pen draws a piece.
//! Click points, and press and drag for curve points. Return finishes an open line; clicking
//! the first point closes a shape.

use super::canvas::{PenPoint, Placed, pen_piece};
use super::{PatternEditor, Selection};
use crate::tr;
use egui::{PointerButton, Response};
use opendrape_core::{Edge, InternalLine, LineKind, Piece, Point2};
use opendrape_geom as geom;

impl PatternEditor {
    pub(super) fn line_tool(&mut self, response: &Response, press: Option<Point2>, pointer: Option<Point2>, tol: f64) {
        if response.drag_started_by(PointerButton::Primary)
            && let Some(at) = press
        {
            self.canvas.line_dragging = self.line_place(at, tol) == Placed::Added;
        }
        if self.canvas.line_dragging
            && response.dragged_by(PointerButton::Primary)
            && let Some(now) = pointer
            && let Some(last) = self.canvas.line.last_mut()
        {
            last.handle = (now.distance(last.pos) > tol).then_some(now);
        }
        if response.drag_stopped() {
            self.canvas.line_dragging = false;
        }
        if response.clicked()
            && let Some(at) = pointer
        {
            self.line_place(at, tol);
        }
    }

    /// Adds a point to the line being drawn, or finishes it when `at` is on its first point (a
    /// closed shape) or its last (an open line).
    fn line_place(&mut self, at: Point2, tol: f64) -> Placed {
        let line = &self.canvas.line;
        let on_first = line.first().is_some_and(|p| p.pos.distance(at) <= tol);
        let on_last = line.last().is_some_and(|p| p.pos.distance(at) <= tol);
        if (line.len() >= 3 && on_first) || (line.len() >= 2 && on_last) {
            return if self.finish_line(on_first && line.len() >= 3) { Placed::Finished } else { Placed::Kept };
        }
        if line.iter().any(|p| p.pos.distance(at) <= tol) {
            return Placed::Duplicate;
        }
        let shapes = self.shapes();
        let owner = match self.canvas.line_owner {
            Some(id) => shapes.iter().find(|s| s.id == id),
            None => shapes.iter().rev().find(|s| inside(&s.piece, at)),
        };
        let Some(shape) = owner.filter(|s| inside(&s.piece, at)) else {
            self.notice = Some(tr!("notice-line-outside"));
            return Placed::Duplicate;
        };
        self.canvas.line_owner = Some(shape.id);
        self.canvas.line.push(PenPoint { pos: at, handle: None });
        Placed::Added
    }

    /// Stores the drafted line on the piece it was drawn in. Returns false, keeping the draft,
    /// when it is too short or the change is refused.
    pub(super) fn finish_line(&mut self, closed: bool) -> bool {
        let n = self.canvas.line.len();
        if n < if closed { 3 } else { 2 } {
            self.notice = Some(tr!("notice-line-short"));
            return false;
        }
        let Some(shape) = self.canvas.line_owner.and_then(|id| geom::shape_of(self.doc.project(), id)) else {
            return false;
        };
        let draft = pen_piece(&self.canvas.line);
        let mut line = InternalLine { vertices: draft.vertices, edges: draft.edges, closed, kind: LineKind::Marking };
        if !closed {
            line.edges.pop(); // the pen's closing edge
        }
        let line = line.mapped(|p| shape.to_stored(p));
        let source = shape.source;
        let index = self.doc.edit(|p| {
            p.piece_mut(source).map(|pc| {
                pc.lines.push(line);
                pc.lines.len() - 1
            })
        });
        if self.note_if_refused() {
            return false;
        }
        self.canvas.line.clear();
        self.canvas.line_owner = None;
        if let Some(l) = index {
            self.selection = Selection::Line(shape.id, l);
        }
        true
    }
}

/// Inside `piece`'s outline, or on it (within 0.5 mm).
pub(super) fn inside(piece: &Piece, p: Point2) -> bool {
    geom::contains(piece, p) || geom::nearest_edge(piece, p).is_some_and(|e| e.2 <= 0.5)
}

/// Whether every point of `piece`'s line `l` (in stored coordinates) is inside the whole piece.
pub(super) fn line_inside(piece: &Piece, l: usize) -> bool {
    let full = geom::unfolded(piece);
    geom::line_points(&piece.lines[l], 0.5).iter().all(|p| inside(&full, *p))
}

/// Moves vertex `k` of `line` to `to`, carrying the curve handles next to it.
pub(super) fn move_line_vertex(line: &mut InternalLine, k: usize, to: Point2) {
    let d = to - line.vertices[k].pos;
    line.vertices[k].pos = to;
    let n = line.vertices.len();
    let before = if k > 0 { Some(k - 1) } else if line.closed { Some(n - 1) } else { None };
    if let Some(e) = before
        && let Edge::Curve { c2, .. } = &mut line.edges[e]
    {
        *c2 = *c2 + d;
    }
    if k < line.edges.len()
        && let Edge::Curve { c1, .. } = &mut line.edges[k]
    {
        *c1 = *c1 + d;
    }
}
```

  Import `line_tool::{line_inside, move_line_vertex}` in `canvas.rs`.

`panel.rs`: `Selection::Line(id, l) => self.line_properties(ui, id, l)`.

```rust
    fn line_properties(&mut self, ui: &mut egui::Ui, id: PieceId, l: usize) {
        let Some((piece, _)) = self.doc.project().owner(id).map(|(p, s)| (p.clone(), s)) else { return };
        let Some(line) = piece.lines.get(l).cloned() else { return };
        let units = self.doc.project().units;
        let source = piece.id;
        ui.strong(tr!("panel-line"));
        ui.label(tr!("panel-line-length", length = units.format(geom::line_length(&line))));
        ui.label(tr!("panel-line-kind"));
        let mut kind = line.kind;
        ui.horizontal(|ui| {
            ui.radio_value(&mut kind, LineKind::Marking, tr!("panel-line-marking"));
            ui.add_enabled_ui(line.closed, |ui| ui.radio_value(&mut kind, LineKind::Cutout, tr!("panel-line-cutout")));
        });
        if kind != line.kind {
            self.doc.edit(|p| {
                if let Some(pc) = p.piece_mut(source) {
                    pc.lines[l].kind = kind;
                }
            });
            self.note_if_refused();
        }
        ui.add_space(6.0);
        if ui.button(tr!("panel-delete-line")).clicked() {
            self.delete_selection();
        }
    }
```

`hint()`:

```rust
            Tool::Line if self.canvas.line.is_empty() => tr!("hint-line-start"),
            Tool::Line => tr!("hint-line-drawing"),
```

`paint.rs`:
- `paint_drafts` draws the line draft like the pen draft, reusing that code. Factor it into `fn paint_draft(&self, painter, rect, points: &[PenPoint], c)` and call it for both `self.canvas.pen` and `self.canvas.line`.
- `paint_selection`, `Selection::Line(_, l)`: draw `geom::line_points(&piece.lines[l], v.mm(0.25))` with `Stroke::new(3.0, c.selected)`, guarded by `l < piece.lines.len()`.

- [ ] **Step 4: Run and see them pass**

Run: `cargo nextest run -p opendrape`
Expected: the 8 new tests pass, plus all earlier ones.

- [ ] **Step 5: Commit**

```bash
cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings
git add crates/app
git commit -m "feat(editor): internal line tool; lines kept inside their piece; line panel

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---
### Task 8: Tab cycles inside the number box

**Files:**
- Modify: `crates/app/src/editor/length_box.rs`, `crates/app/tests/details.rs`

**Interfaces:**
- Consumes: Task 6's optional second field.
- Produces:
  - Both box fields use `TextEdit::lock_focus(true)`, so egui doesn't move focus out of the box on Tab. A singleline field ignores the Tab itself (verified in the egui 0.36.2 source: only multiline inserts `\t`).
  - The box then moves focus itself: Tab in the first field focuses the second, and Tab in the second focuses the first.
  - A one-field box (notch distance) just keeps its focus.

- [ ] **Step 1: Failing tests** (append to `crates/app/tests/details.rs`)

```rust
#[test]
fn tab_cycles_between_the_two_box_fields() {
    let mut h = harness();
    key(&mut h, Key::H);
    click(&mut h, 100.0, 100.0);
    type_number(&mut h, "3");
    key(&mut h, Key::Tab);
    assert!(h.get_by_role_and_label(Role::TextInput, "Angle").is_focused());
    key(&mut h, Key::Tab);
    assert!(h.state().length_box_open(), "Tab never closes the box");
    assert!(h.get_by_role_and_label(Role::TextInput, "Length").is_focused());
    key(&mut h, Key::Tab);
    assert!(h.get_by_role_and_label(Role::TextInput, "Angle").is_focused());
}

#[test]
fn tab_in_a_one_field_box_keeps_it_open() {
    let mut h = harness();
    with_rectangle(&mut h);
    key(&mut h, Key::N);
    let p = at(&h, 380.0, 101.0);
    h.hover_at(p);
    h.run();
    type_number(&mut h, "5");
    key(&mut h, Key::Tab);
    assert!(h.state().length_box_open());
    assert!(h.get_by_role_and_label(Role::TextInput, "Distance").is_focused());
}
```

Run: `cargo nextest run -p opendrape --test details tab_`
Expected: both tests FAIL. The second Tab moves focus out of the box, which then closes.

- [ ] **Step 2: Implement** (`length_box.rs`)

- Add `.lock_focus(true)` to both `TextEdit::singleline(..)` builders.
- After the `if focus { a.request_focus(); }` line, add:

```rust
                        // The fields keep their focus on Tab (lock_focus), and the box moves
                        // it itself, so Tab cycles between the two fields instead of leaving.
                        if ui.input(|i| i.key_pressed(Key::Tab)) {
                            match &b {
                                Some(b) if a.has_focus() => b.request_focus(),
                                Some(_) => a.request_focus(),
                                None => {}
                            }
                        }
```

  The `Some(_)` arm runs when `b` has focus, because one of the two fields always has focus while the box is open. Update `show`'s doc comment to: "Tab moves between the fields (and back)".

- [ ] **Step 3: Run and see them pass**

Run: `cargo nextest run -p opendrape`
Expected: both new tests pass. The M2a tests that press Tab once (`typed_angle_sets_the_direction`, `tabbing_into_a_filled_field_replaces_its_number_too`, `rectangle_by_typing_its_size`) still pass.

- [ ] **Step 4: Commit**

```bash
cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings
git add crates/app
git commit -m "fix(editor): Tab cycles between the number box's fields instead of closing it

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 9: A recovery copy when quitting can't ask first

**Files:**
- Create: `crates/app/src/recovery.rs`
- Modify:
  - `crates/app/src/{app.rs,lib.rs,main.rs}`
  - `crates/app/src/editor/{mod.rs,document.rs}`
  - `crates/app/i18n/en-US/opendrape.ftl`
  - `crates/app/tests/ui.rs`

**Interfaces:**
- Consumes: `opendrape_io::{save, load}`, `crate::startup_log::stage`, `directories::ProjectDirs` (already a dependency).
- Produces:
  - `pub struct Recovery` (Clone, Debug) with:
    - `new(dir: Option<&Path>)`;
    - `default_location()`;
    - `write(&self, project: &Project, from: Option<&Path>)`;
    - `take(&self) -> Option<(Project, Option<PathBuf>)>`;
    - `discard(&self)`.
  - `Startup.recovery: Recovery`.
  - `Document::recovered(project, path) -> Document`: dirty, and Save writes to `path`.
  - `PatternEditor::set_recovered(project, path)`.
  - Implementing `eframe::App::on_exit` on `OpenDrapeApp`.

**Behaviour:**
- **When the copy is written:** on exit, if there are unsaved changes and the student didn't choose to quit (`closing` is false), OpenDrape writes `recovery.odp`. It goes in the same folder as the graphics settings. It also writes `recovery-origin.txt` holding the project's file path when it has one (as UTF-8; a non-UTF-8 path is not remembered).
  - Cases that reach `on_exit` this way: Dock → Quit, logout, shutdown.
  - "Don't save" and the other quit paths set `closing` first, so they leave no copy.
- **On the next start:** if a copy loads, a modal asks "Restore unsaved work?" with "OpenDrape closed last time with unsaved work. Restore it?", and offers **Restore** or **Discard**.
  - **Restore** opens it as unsaved work, remembering the original file.
  - **Either answer** deletes the copy.
  - **A copy that won't load** is deleted silently and logged.
- **While the modal is up,** pattern keys are off (like the other modals).
- **Another running copy:** a second copy of OpenDrape (the instance lock is held elsewhere) uses `Recovery::new(None)`, which does nothing, so two copies never fight over one recovery file.

- [ ] **Step 1: Strings** (append to the `.ftl`)

```
recovery-title = Restore unsaved work?
recovery-body = OpenDrape closed last time with unsaved work. Restore it?
recovery-restore = Restore
recovery-discard = Discard
```

- [ ] **Step 2: Failing tests**

`crates/app/src/recovery.rs` unit tests (at the bottom of the new file):

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use opendrape_core::{Piece, PieceId, Point2};

    fn project() -> Project {
        let mut p = Project::new();
        p.add_piece(Piece::rectangle(PieceId(0), "Front", Point2::new(0.0, 0.0), 300.0, 500.0));
        p
    }

    #[test]
    fn writes_and_takes_back_a_copy() {
        let dir = tempfile::tempdir().unwrap();
        let r = Recovery::new(Some(dir.path()));
        assert!(r.take().is_none());
        r.write(&project(), Some(Path::new("/work/skirt.odp")));
        let (back, from) = r.take().unwrap();
        assert_eq!(back, project());
        assert_eq!(from, Some(PathBuf::from("/work/skirt.odp")));
        r.discard();
        assert!(r.take().is_none());
        r.write(&project(), None);
        assert_eq!(r.take().unwrap().1, None, "an untitled project has no file");
    }

    #[test]
    fn a_damaged_copy_is_dropped() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("recovery.odp"), b"not a zip").unwrap();
        let r = Recovery::new(Some(dir.path()));
        assert!(r.take().is_none());
        assert!(!dir.path().join("recovery.odp").exists());
    }

    #[test]
    fn without_a_folder_nothing_happens() {
        let r = Recovery::new(None);
        r.write(&project(), None);
        assert!(r.take().is_none());
        r.discard();
    }
}
```

`crates/app/tests/ui.rs`:
- Add `recovery: Recovery::new(None)` to the three existing `Startup` literals, and `use opendrape::Recovery;`.
- Add a helper and four tests:

```rust
fn harness_recovering(config_dir: &Path, recovery_dir: &Path) -> Harness<'static, OpenDrapeApp> {
    let startup = Startup {
        decision: SAVED_AUTO,
        previous: GpuState::default(),
        store: StateStore::new(Some(config_dir)),
        smoke_test: false,
        autoplay: false,
        file_dialogs: FileDialogs::always_cancel(),
        recovery: Recovery::new(Some(recovery_dir)),
    };
    Harness::builder()
        .with_size(egui::vec2(1000.0, 700.0))
        .wgpu()
        .build_eframe(move |cc| OpenDrapeApp::new(cc, startup, SharedState::default()))
}

#[test]
fn quitting_without_asking_keeps_a_copy_that_is_offered_next_time() {
    use eframe::App as _;
    let (config, rescue) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let mut h = harness_recovering(config.path(), rescue.path());
    h.run();
    add_piece(&mut h);
    h.state_mut().on_exit(); // what the Dock's Quit, logout and shutdown lead to
    assert!(rescue.path().join("recovery.odp").exists());
    drop(h);

    let mut h = harness_recovering(config.path(), rescue.path());
    h.run();
    h.get_by_label("Restore unsaved work?");
    h.get_by_label("Restore").click();
    h.run();
    assert_eq!(pieces(&h), 1);
    assert!(h.state().editor().doc.is_dirty(), "restored work is still unsaved");
    assert!(!rescue.path().join("recovery.odp").exists());
}

#[test]
fn dont_save_leaves_no_copy() {
    use eframe::App as _;
    let (config, rescue) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let mut h = harness_recovering(config.path(), rescue.path());
    h.run();
    add_piece(&mut h);
    h.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::Q);
    h.run();
    h.get_by_label("Don't save").click();
    h.run();
    assert!(h.state().is_closing());
    h.state_mut().on_exit();
    assert!(!rescue.path().join("recovery.odp").exists());
}

#[test]
fn a_saved_project_leaves_no_copy() {
    use eframe::App as _;
    let (config, rescue) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let mut h = harness_recovering(config.path(), rescue.path());
    h.run();
    h.state_mut().on_exit(); // nothing changed
    assert!(!rescue.path().join("recovery.odp").exists());
}

#[test]
fn discarding_the_copy_deletes_it() {
    let (config, rescue) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let mut project = opendrape_core::Project::new();
    project.add_piece(Piece::rectangle(PieceId(0), "Front", Point2::new(0.0, 0.0), 300.0, 500.0));
    Recovery::new(Some(rescue.path())).write(&project, None);
    let mut h = harness_recovering(config.path(), rescue.path());
    h.run();
    h.get_by_label("Discard").click();
    h.run();
    assert_eq!(pieces(&h), 0);
    assert!(!rescue.path().join("recovery.odp").exists());
}
```

Run: `cargo nextest run -p opendrape`
Expected: compile errors (`Recovery` and `Startup.recovery` don't exist yet).

- [ ] **Step 3: Implement**

`crates/app/src/recovery.rs` (above its tests):

```rust
//! A copy of unsaved work, written when OpenDrape is quit in a way that can't ask first (from
//! the Dock, or at logout or shutdown, where macOS ends the app without a close request) and
//! offered back the next time it starts.

use opendrape_core::Project;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug)]
pub struct Recovery {
    dir: Option<PathBuf>,
}

impl Recovery {
    /// Keeps the copy in `dir`; `None` keeps no copy at all (tests, or a second running copy of
    /// OpenDrape, which must not overwrite the first one's).
    pub fn new(dir: Option<&Path>) -> Self {
        Self { dir: dir.map(Path::to_path_buf) }
    }

    /// OpenDrape's settings folder, next to the graphics settings.
    pub fn default_location() -> Self {
        let dirs = directories::ProjectDirs::from("org", "OpenDrape", "OpenDrape");
        Self::new(dirs.as_ref().map(|d| d.config_local_dir()))
    }

    fn copy(&self) -> Option<PathBuf> {
        self.dir.as_ref().map(|d| d.join("recovery.odp"))
    }

    fn origin(&self) -> Option<PathBuf> {
        self.dir.as_ref().map(|d| d.join("recovery-origin.txt"))
    }

    /// Writes `project`, and the file it came from when it has one. OpenDrape is closing and
    /// can't show anything, so failures are only logged.
    pub fn write(&self, project: &Project, from: Option<&Path>) {
        let (Some(dir), Some(copy), Some(origin)) = (&self.dir, self.copy(), self.origin()) else { return };
        if let Err(e) = std::fs::create_dir_all(dir) {
            crate::startup_log::stage(format_args!("recovery: no folder {dir:?}: {e}"));
            return;
        }
        if let Err(e) = opendrape_io::save(project, &copy) {
            crate::startup_log::stage(format_args!("recovery: could not write {copy:?}: {e}"));
            return;
        }
        let remembered = match from.and_then(Path::to_str) {
            Some(path) => std::fs::write(&origin, path),
            None => std::fs::remove_file(&origin).or(Ok(())),
        };
        if let Err(e) = remembered {
            crate::startup_log::stage(format_args!("recovery: could not note the file: {e}"));
        }
    }

    /// The waiting copy and the file it came from, if there is a copy that opens. A copy that
    /// doesn't open is deleted.
    pub fn take(&self) -> Option<(Project, Option<PathBuf>)> {
        let copy = self.copy().filter(|c| c.exists())?;
        match opendrape_io::load(&copy) {
            Ok(project) => {
                let from = self.origin().and_then(|o| std::fs::read_to_string(o).ok()).map(PathBuf::from);
                Some((project, from))
            }
            Err(e) => {
                crate::startup_log::stage(format_args!("recovery: dropping {copy:?}: {e}"));
                self.discard();
                None
            }
        }
    }

    /// Deletes the copy.
    pub fn discard(&self) {
        for file in [self.copy(), self.origin()].into_iter().flatten() {
            let _ = std::fs::remove_file(file);
        }
    }
}
```

`lib.rs`: `pub mod recovery;` and `pub use recovery::Recovery;`.

`editor/document.rs`:

```rust
    /// Work brought back from a recovery copy: unsaved, and Save writes it to `path` (the file
    /// it came from) when there is one.
    pub fn recovered(project: Project, path: Option<PathBuf>) -> Self {
        let mut doc = Self::new(project, path);
        doc.saved = Project::new();
        doc
    }
```

`editor/mod.rs`:

```rust
    /// Like [`Self::set_project`], for work restored from a recovery copy: it stays unsaved.
    pub fn set_recovered(&mut self, project: Project, path: Option<PathBuf>) {
        self.set_project(Project::new(), None);
        self.doc = Document::recovered(project, path);
    }
```

`app.rs`:
- `Startup` gets `pub recovery: Recovery` (doc: "Where unsaved work is kept when quitting can't ask first.").
- `OpenDrapeApp` gets `recovery: Recovery` and `offered: Option<(Project, Option<PathBuf>)>`. In `new`: `let offered = startup.recovery.take();` and `recovery: startup.recovery.clone()`.
- `keys_for_pattern` also requires `self.offered.is_none()`.
- Add the modal, called right after `error_modal`:

```rust
    /// Offers back the work a quit without asking left behind (see `crate::recovery`).
    fn recovery_modal(&mut self, ctx: &egui::Context) {
        if self.offered.is_none() || self.pending.is_some() {
            return;
        }
        let mut answer = None;
        egui::Modal::new(egui::Id::new("recovery")).show(ctx, |ui| {
            ui.set_max_width(360.0);
            ui.heading(tr!("recovery-title"));
            ui.label(tr!("recovery-body"));
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                if ui.button(tr!("recovery-restore")).clicked() {
                    answer = Some(true);
                }
                if ui.button(tr!("recovery-discard")).clicked() {
                    answer = Some(false);
                }
            });
        });
        let Some(restore) = answer else { return };
        if let (true, Some((project, from))) = (restore, self.offered.take()) {
            self.editor.set_recovered(project, from);
        }
        self.offered = None;
        self.recovery.discard();
    }
```

  The modal has no "click outside to dismiss": only the two buttons answer it, so a stray click can't lose the work.
- Implement `on_exit` in `impl eframe::App for OpenDrapeApp`. Without the glow feature, its signature is `fn on_exit(&mut self)`.

```rust
    /// The last chance to save anything. Quitting from the Dock, logout and shutdown end the app
    /// without a close request, so nobody could be asked about unsaved work: keep a copy.
    /// Quitting through OpenDrape's own question sets `closing` first and leaves no copy.
    fn on_exit(&mut self) {
        if !self.closing && self.editor.doc.is_dirty() {
            self.recovery.write(self.editor.doc.project(), self.editor.doc.path.as_deref());
        }
    }
```

`main.rs`: the `Startup` literal gains:

```rust
        // A second copy of OpenDrape keeps no recovery file, so the two never overwrite each other's.
        recovery: if another_instance_running { opendrape::Recovery::new(None) } else { opendrape::Recovery::default_location() },
```

- [ ] **Step 4: Run and see them pass**

Run: `cargo nextest run -p opendrape`
Expected:
- the 3 unit tests and 4 UI tests pass;
- all earlier app tests still pass, since their harnesses use `Recovery::new(None)`.

- [ ] **Step 5: Commit**

```bash
cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings
git add crates/app
git commit -m "feat(app): keep a recovery copy when the app is quit without asking, and offer it next time

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 10: Checklist, docs, final checks

**Files:**
- Create: `docs/testing/M2b-checklist.md`
- Modify: `README.md`, `docs/specs/2026-10-09-opendrape-design.md`

- [ ] **Step 1: Tester checklist** (`docs/testing/M2b-checklist.md`)

```markdown
# OpenDrape M2b: what to try

This build finishes the pattern tools: seam allowance, notches, internal lines, pieces cut
on the fold, and mirrored left/right pairs. Pieces still don't go onto the body; sewing comes
in M3.

## Check these

- [ ] Every piece shows a light grey band around it: the 1 cm seam allowance. The outer line
      is where you cut. **Show seam allowance** at the top hides and shows it.
- [ ] Click a piece. In **Properties**, change **Seam allowance** to **1,5**: the band
      widens all round.
- [ ] Click one edge. Type **2** in its **Seam allowance**: only that edge changes. **Same as
      piece** puts it back. Tick **Hem**: that edge gets 3 cm, and the corners at its ends
      fold in so the hem can turn up.
- [ ] Draw half a skirt front. Click its straight centre edge, then **Set as fold line**: the
      whole piece appears, the other half pale, with a dashed fold line and "Place on fold".
      Drag a point on the drawn half: the pale half follows. There is no allowance along the
      fold.
- [ ] Press **N** (Notch) and click an edge: a small notch appears in the allowance. Point
      near an edge and type **5**, then **Return** (Enter): a notch exactly 5 cm from the
      nearer end. Click a notch with **Edit (Z)**, then try **Double** and **V** in
      Properties.
- [ ] Press **L** (Internal line). Click inside a piece twice and press **Return**: a dashed
      line. Click three points and then the first again: a closed shape. Choose **Cut-out**
      for it in Properties. Lines can't leave their piece.
- [ ] Select a back panel and click **Make mirrored pair**: its mirror image appears beside
      it, named "... (mirror)". Drag a point on either one: the other changes too. Drag the
      middle of one: only that one moves. **Break pair** makes them separate pieces.
- [ ] While typing a length and angle with the pen, **Tab** moves between the two boxes and
      back, and never closes them.
- [ ] Edge lengths sit just outside each piece, not on the line.
- [ ] **Draft a skirt front on the fold** with a 1 cm allowance and a 3 cm hem, put notches at
      the hip, and add a mirrored pair of back panels. **File → Save As…**, quit, reopen and
      **File → Open…**: everything is back. Undo several steps with **Cmd+Z**.
- [ ] Your M2a files still open; their pieces now show a 1 cm allowance.
- [ ] Change something, then quit from the Dock (right-click the OpenDrape icon → **Quit**).
      Open OpenDrape again: it offers to **Restore** your unsaved work.

## Known limits in this build

- Cmd+H (hide OpenDrape) does not work in this build.

If anything looks wrong, take a screenshot, then choose Help → About OpenDrape and
click **Copy diagnostics**.
```

- [ ] **Step 2: README and spec status**

README line 11:
> **Status:** early development (milestone M2b: pattern details: seam allowance, notches, internal lines, cut-on-fold pieces and mirrored pairs). Next: sewing pieces onto the body (M3).

Spec `## Status`, after the M2a bullet:
> - **M2b Pattern details: complete (2026-10-09).** Seam allowance per piece and edge (1 cm default, 3 cm hems with mirrored corners), notches (single/double/triple, slit/V), internal lines (markings and cut-outs), cut-on-fold pieces shown whole, mirrored left/right pairs, project format v2 (v1 files upgrade), a recovery copy when quitting can't ask, Tab cycling in the number box, labels outside pieces, cached drawing for huge files.

- [ ] **Step 3: Full local CI**

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo nextest run --workspace
cargo deny check
cargo about generate -m crates/app/Cargo.toml about.hbs -o target/licenses/THIRD_PARTY_LICENSES.html
```
Expected: all green. The `cargo about` run proves that the release packaging accepts i_overlay and its dependencies' licences.

- [ ] **Step 4: Commit**

```bash
git add docs README.md
git commit -m "docs: M2b tester checklist and status

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

The final whole-branch review, the merge and any publishing are done by the controller afterwards. Ask the user before merging or pushing. **Never run the app on the user's Mac.**
