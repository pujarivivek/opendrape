# M5 Workspaces Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Five workspace tabs (Modeling · Finishing · Texturing · Rendering · Animation). Modeling holds today's tools in a tidier layout; the other four show a "coming soon" page.

**Architecture:**
- **Tabs:** a screen-only `Workspace` enum on `OpenDrapeApp` picks what the central panel shows. The left 3D panel is the same in every workspace.
- **Look:** a `theme` module installs the style and the Phosphor icon font, and owns every colour constant.
- **Icons:** an `icons` module draws icon buttons whose AccessKit labels stay the readable names the tests use.

**Tech Stack:** Rust 2024, eframe/egui 0.36.2, egui_kittest 0.36.2, egui-phosphor 0.14.0 (new), Fluent via `tr!`.

**Spec:** `docs/superpowers/specs/2026-10-10-m5-workspaces-design.md`

**Execution (user preference):** native, in this session, with one fresh whole-branch review at the end. Ask before launching the app and before any push or merge.

## Global Constraints
- The tabs are `Workspace` in code; never `Stage`, which is the dress-form scene.
- The workspace is screen state only: not in `Project`, not an undo step, never marks dirty. The app opens in Modeling; New and Open keep the tab.
- Shortcuts are Cmd+1…5 (Ctrl on Windows), shown in a View menu.
- **Labels the tests rely on stay exactly as they are:**
  - "Pen (H)" and every "<Tool> (<Key>)";
  - "Play", "Pause", "Reset";
  - "cm", "inch", "Show lengths", "Show seam allowance", "Fit (⌘0)";
  - all menu labels.
- Tooltips are never exactly equal to a button's label: name, newline, tip. A hovered tooltip must not make `get_by_label` ambiguous.
- Every user-visible string goes through `tr!` and `crates/app/i18n/en-US/opendrape.ftl`.
- Dependencies are pinned exactly: `egui-phosphor = { version = "=0.14.0", default-features = false, features = ["regular"] }`.
- Colours keep their current values; nothing on screen changes colour except the selection highlight.
- Out of scope: start screen, pieces list, settings, AI switch, tools in the four new workspaces, new fonts, saved panel sizes, per-workspace 3D rules.

## Review Focus
1. **Cmd+3 pressed mid-drag on the pattern table:** the drag must not be left half-done. Workspace keys are ignored while any pointer button is down.
2. **Cmd+3 while the number box is open, or a property field is being typed in:** stay in Modeling; the typing must not be lost.
3. **Undo in Texturing after a piece was added in Modeling:** the piece goes, the drape (if any) follows, and the selection doesn't point at a deleted piece.
4. **Hovering a tool icon:** the tooltip must not duplicate the button's accessible label (`get_by_label` panics on two matches).
5. **A 640×480 window:** the menu row with tabs must not panic (negative spacing). The Modeling properties panel must scroll, not clip.

---

### Task 1: Theme, icon font and icon buttons

**Files:**
- Create: `crates/app/src/theme.rs`, `crates/app/src/icons.rs`
- Modify: `Cargo.toml` (workspace deps), `crates/app/Cargo.toml`, `crates/app/src/lib.rs`, `crates/app/src/app.rs` (`OpenDrapeApp::new` calls `theme::install`; stats grey uses `theme::STATS_TEXT`), `crates/app/src/editor/paint.rs` (Palette and PIN_COLOUR move to theme), `crates/app/src/editor/mod.rs` (re-export), `crates/app/src/editor/seams.rs` (SEAM_COLOURS from theme), `crates/app/src/arrange/overlay.rs` (AXIS_COLOURS, BRIGHT from theme), `crates/app/src/viewport.rs` (SKIN, FABRIC, SELECTED_FABRIC from theme), `ASSETS.md`
- Test: unit tests in `theme.rs` and `icons.rs`

**Interfaces:**
- **Produces in `theme`:**
  - `pub fn install(ctx: &egui::Context)`
  - `pub(crate) struct PatternPalette { table, minor, major, ink, fill, selected, handle, label, band, cut, pale, pale_fill }` with `pub(crate) fn new(dark: bool) -> Self` (fields as today's `Palette`)
  - `pub(crate) const PIN: Color32`, `SEAM_COLOURS: [Color32; 8]`, `AXIS_COLOURS: [Color32; 3]`, `GIZMO_BRIGHT: Color32`, `SKIN: [f32; 3]`, `FABRIC: [f32; 3]`, `SELECTED_FABRIC: [f32; 3]`, `STATS_TEXT: Color32`
- **Produces in `icons`:**
  - `pub fn icon_button(ui: &mut egui::Ui, icon: &str, label: &str, tip: &str, selected: bool, enabled: bool) -> egui::Response`
    - accessible label `label`;
    - tooltip `"{label}\n{tip}"`, also shown when disabled;
    - min size 28×28; icon at 18 pt.
  - Icon names: `pub use egui_phosphor::regular as ph;` plus `pub const TOOL_ICONS` mapping (used in Task 3).

- [ ] **Step 1: Add the dependency.** In the workspace `Cargo.toml` `[workspace.dependencies]`, add `egui-phosphor = { version = "=0.14.0", default-features = false, features = ["regular"] }`. In `crates/app/Cargo.toml` `[dependencies]`, add `egui-phosphor.workspace = true`.
- [ ] **Step 2: Write the failing tests** (`theme.rs` `#[cfg(test)]`):
  - `colours_keep_their_values`: assert every moved constant equals today's literal, e.g. `PIN == Color32::from_rgb(200, 30, 60)`, `SEAM_COLOURS[7] == from_rgb(190,145,0)`, `AXIS_COLOURS`, `GIZMO_BRIGHT == from_rgb(255,200,0)`, `SKIN == [0.62,0.45,0.36]`, `FABRIC == [0.17,0.36,0.70]`, `SELECTED_FABRIC == [0.95,0.55,0.25]`, `STATS_TEXT == from_gray(60)`, and `PatternPalette::new(false).table == from_gray(250)`, `PatternPalette::new(true).selected == from_rgb(255,150,90)`.
  - `install_registers_icons_and_selection` (`icons.rs` or `theme.rs`): `let ctx = egui::Context::default(); install(&ctx);` Then the light style's `visuals.selection.bg_fill` is `LIGHT_SELECTION_BG`, and `ctx.fonts(...)` has a glyph for `ph::PEN_NIB` (or `FontDefinitions` contains "phosphor").
- [ ] **Step 3: Run them and see them fail:** `cargo nextest run -p opendrape theme` fails to compile (no module).
- [ ] **Step 4: Implement.** `theme.rs`:
  - Move `Palette` (renamed `PatternPalette`, fields `pub(crate)`) and the constants verbatim.
  - `install`:
    - `FontDefinitions::default()` + `egui_phosphor::add_to_fonts(&mut fonts, Variant::Regular)` → `ctx.set_fonts`.
    - `ctx.all_styles_mut`: `spacing.item_spacing = vec2(8.0, 6.0)`, `spacing.button_padding = vec2(6.0, 3.0)`, every `widgets.*.corner_radius = CornerRadius::same(4)`, and text sizes Body/Button 13, Small 11, Heading 16, Monospace 12.5.
    - `ctx.style_mut_of(Theme::Light, …)`: selection `bg_fill = rgb(220,231,251)`, `stroke = Stroke::new(1.0, rgb(28,74,160))`.
    - `Theme::Dark`: `bg_fill = rgb(42,74,127)`, `stroke` colour `rgb(207,224,255)`.
  - `icons.rs`: `icon_button` uses `egui::Button::selectable(selected, RichText::new(icon).size(18.0)).min_size(vec2(28.0, 28.0))`, then `ui.add_enabled`. AccessKit label via `ui.ctx().accesskit_node_builder(id, |n| n.set_label(label))` (same as `menu_item` in app.rs). Tooltip via `on_hover_text` + `on_disabled_hover_text` with `format!("{label}\n{tip}")`.
  - **Call sites:** `paint.rs` uses `crate::theme::PatternPalette`; `editor/mod.rs` re-exports `pub(crate) use crate::theme::PIN as PIN_COLOUR;` (keeps the overlay.rs path working), or update overlay to `crate::theme::PIN`. Update seams/overlay/viewport. `OpenDrapeApp::new` calls `crate::theme::install(&cc.egui_ctx)` first.
  - **ASSETS.md:** add "Phosphor Icons (via egui-phosphor 0.14.0), MIT, https://github.com/phosphor-icons/core".
- [ ] **Step 5: Run the tests:** `cargo nextest run -p opendrape` (all pass; colours unchanged), then `cargo clippy -p opendrape --all-targets -- -D warnings`.
- [ ] **Step 6: Commit:** `feat(app): one theme module with the icon font, the selection colour and every colour constant`

### Task 2: Workspaces: tabs, View menu, shortcuts, coming-soon pages

**Files:**
- Create: `crates/app/src/workspace.rs`, `crates/app/tests/workspaces.rs`
- Modify: `crates/app/src/lib.rs` (`pub mod workspace;`), `crates/app/src/app.rs` (field, accessors, `menu_bar`, `ui`, `workspace_shortcut`, `app_undo_redo`), `crates/app/src/editor/mod.rs` (`REDO_Y` becomes `pub`), `crates/app/i18n/en-US/opendrape.ftl`

**Interfaces:**
- **Consumes:** `theme::install`, `editor::{UNDO, REDO, REDO_Y}`, `PatternEditor::{undo, redo, length_box_open}`.
- **Produces:**
  - `pub enum Workspace { Modeling, Finishing, Texturing, Rendering, Animation }` (Copy, Eq, Debug, Default = Modeling) with `pub const ALL: [Self; 5]`, `pub fn label(self) -> String`, `pub fn blurb(self) -> String`, `pub fn shortcut(self) -> KeyboardShortcut` (Cmd+Num1…Num5).
  - `pub fn coming_soon(ui: &mut egui::Ui, ws: Workspace)`
  - `pub fn tabs(ui: &mut egui::Ui, current: Workspace) -> Option<Workspace>` draws the centred tab row; each tab has AccessKit role `Role::Tab`, its label, and `set_selected(current == ws)`.
  - `OpenDrapeApp::workspace(&self) -> Workspace`, `OpenDrapeApp::set_workspace(&mut self, Workspace)`.

- [ ] **Step 1: Add the strings** to `opendrape.ftl` (new section at the end):
  ```
  ## Workspaces
  menu-view = View
  workspace-modeling = Modeling
  workspace-finishing = Finishing
  workspace-texturing = Texturing
  workspace-rendering = Rendering
  workspace-animation = Animation
  workspace-modeling-blurb = Draft pattern pieces, sew them and drape them on the form.
  workspace-finishing-blurb = Topstitching, binding, buttons, zippers and other finishing details.
  workspace-texturing-blurb = Fabrics, colours and prints for your pieces.
  workspace-rendering-blurb = Pictures of your garment, from quick views to AI photos.
  workspace-animation-blurb = Turntables and short clips of your garment.
  coming-soon = Coming in a later update.
  ```
- [ ] **Step 2: Write the failing tests** in `tests/workspaces.rs`. It has a local whole-app `harness()` copied from `tests/ui.rs` (`Startup` with `FileDialogs::always_cancel()`, `Recovery::new(None)`, size 1000×700, `.wgpu().build_eframe`). Tests:
  - `five_tabs_and_modeling_first`: `get_by_role_and_label(Role::Tab, l)` for all five; `state().workspace() == Modeling`.
  - `cmd_3_opens_texturing_with_its_coming_soon_page`: `key_press_modifiers(COMMAND, Num3)`, `run`. Then: Texturing; `get_by_label("Coming in a later update.")`; `query_by_label("Pen (H)").is_none()`; `get_by_label("Play")` still there.
  - `clicking_a_tab_switches`: click the Rendering tab → Rendering. Click Modeling → `get_by_label("Pen (H)")` back.
  - `tool_letters_do_nothing_outside_modeling`: Cmd+3, press H, Cmd+1 → `editor().tool == Tool::Edit`.
  - `undo_works_in_every_workspace`: add a rectangle via `editor_mut().doc.edit`, select it (`editor_mut().selection = Selection::Piece(id)`), Cmd+4, Cmd+Z → no pieces and `editor().selection == Selection::None`. Shift+Cmd+Z → the piece is back.
  - `switching_never_marks_the_project_changed`: fresh app, cycle Cmd+1…5 → `!editor().doc.is_dirty()` and `window_title()` has no "•".
  - `workspace_keys_wait_for_the_save_question`: make the doc dirty, Cmd+N (question opens), Cmd+3 → still Modeling; click "Cancel".
  - `workspace_keys_wait_while_typing`: add and select a piece, click the `Role::TextInput` "Name", Cmd+3 → still Modeling.
  - `workspace_keys_wait_while_the_mouse_is_down`: press the primary button over the canvas (no release), Cmd+3 → still Modeling; release.
  - `view_menu_switches`: click "View", click `Role::Button` "Animation" → Animation.
  - `small_window_with_tabs`: 640×480 and 300×200, each workspace, run → no panic.
- [ ] **Step 3: Run them and see them fail:** `cargo nextest run -p opendrape --test workspaces` fails to compile.
- [ ] **Step 4: Implement:**
  - **`workspace.rs`:**
    - The enum and its methods above.
    - `coming_soon`: `ui.vertical_centered` with top space `available_height * 0.3`, `heading(label)`, `label(blurb)`, `weak(tr!("coming-soon"))`.
    - `tabs`:
      - Widths come from `WidgetText::into_galley` for the Button text style.
      - Start x = `max(cursor.x + 8, (ctx.content_rect().center().x - total / 2))` via `ui.add_space((start - ui.cursor().min.x).max(0.0))`.
      - Each tab is `ui.add(Button::selectable(current == ws, ws.label()))` + `accesskit_node_builder(id, |n| { n.set_role(Role::Tab); n.set_label(label); n.set_selected(current == ws) })`, hover `"{label} ({shortcut})\n{blurb}"`.
  - **`app.rs`:**
    - Field `workspace: Workspace` (default Modeling), accessors.
    - `menu_bar` adds `ui.menu_button(tr!("menu-view"), …)` between Edit and Help, with `menu_item(ui, true, ws.label(), &ws.shortcut())` per workspace, then calls `workspace::tabs` after the menus.
    - `menu_bar` now returns `(Option<FileAction>, Option<Workspace>)`.
    - `workspace_shortcut(&self, ctx) -> Option<Workspace>` returns None when:
      - `text_edit_focused()`;
      - `editor.length_box_open()`;
      - `pending`, `error` or `offered` is Some;
      - `arranger.is_dragging() || draper.is_dragging()`;
      - `ctx.input(|i| i.pointer.any_down())`.

      Otherwise it returns the first `ws.shortcut()` consumed.
    - In `ui()`: call it right after `file_shortcut`; apply a menu click or shortcut with `self.workspace = ws`. The central panel becomes:
      ```rust
      match self.workspace {
          Workspace::Modeling => egui::CentralPanel::default().show(ui, |ui| self.editor.ui_with_keys(ui, keys_for_pattern)),
          ws => egui::CentralPanel::default().show(ui, |ui| { if keys_for_pattern { self.app_undo_redo(ui.ctx()) } workspace::coming_soon(ui, ws) }),
      };
      ```
    - `app_undo_redo` returns at once while `text_edit_focused()`. Otherwise: Redo first (`REDO` or `REDO_Y`) → `editor.redo()`, else `UNDO` → `editor.undo()`. The editor's own `undo`/`redo` already fix selection, pins and drafts.
- [ ] **Step 5: Run the tests:** `cargo nextest run -p opendrape` (all, including the 70 in ui.rs) and clippy.
- [ ] **Step 6: Commit:** `feat(app): five workspace tabs with Cmd+1…5, a View menu and coming-soon pages`

### Task 3: Modeling layout: tool strip, view bar, scrolling properties, 3D icon buttons

**Files:**
- Modify: `crates/app/src/editor/mod.rs` (`toolbar` → `tool_strip` + `view_bar`; panels in `ui_with_keys`; properties in a ScrollArea), `crates/app/src/app.rs` (`toolbar` at line 279 uses `icons::icon_button` for Play/Pause/Reset), `crates/app/src/icons.rs` (tool → icon map), `opendrape.ftl` (`toolbar-play-tip`, `toolbar-pause-tip`, `toolbar-reset-tip`)
- Test: `crates/app/tests/editor.rs` (a new test), and existing suites

**Interfaces:**
- **Consumes:** `icons::icon_button`, `ph::*`.
- **Produces:**
  - `pub fn tool_icon(tool: Tool) -> &'static str`. Mapping:
    - Edit → `CURSOR`, Pen → `PEN_NIB`, Rectangle → `RECTANGLE`, AddPoint → `PLUS_CIRCLE`;
    - Notch → `TRIANGLE`, Line → `LINE_SEGMENT`, Sew → `NEEDLE`, FreeSew → `PATH`.

- [ ] **Step 1: Write the failing test** in `tests/editor.rs`, `every_tool_has_a_button_in_the_strip`. For each `Tool::ALL`, click `get_by_label(format!("{} ({})", name, key))`, run, and check `state().tool == tool`. Use the labels "Edit (Z)", "Pen (H)", "Rectangle (S)", "Add point (X)", "Notch (N)", "Internal line (L)", "Sew (W)", "Free sew (F)".
  - Also `strip_is_left_of_the_canvas`: the "Pen (H)" node's rect `max.x <= state().canvas_rect.min.x + 1`.
- [ ] **Step 2: Run it:** the first test may already pass (labels exist today); the second fails, because the toolbar sits above the canvas.
- [ ] **Step 3: Implement:**
  - In `ui_with_keys`, order the panels:
    - `Panel::top("pattern_view_bar")`: `view_bar`, i.e. units, checkboxes and Fit, unchanged code from today's toolbar after the first separator;
    - `Panel::bottom("pattern_status")`;
    - `Panel::left("pattern_tools").resizable(false).exact_size(40.0)`: `tool_strip`;
    - `Panel::right("pattern_properties")` with `egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| self.properties(ui))`;
    - `CentralPanel` canvas.
  - `tool_strip`: `ui.vertical_centered`, and for each tool `icons::icon_button(ui, tool_icon(tool), &format!("{} ({})", tool.label(), tool.key().name()), &tool.tip(), self.tool == tool, true)` → on click, `set_tool`.
  - **3D toolbar:** Play/Pause become `icon_button(ui, if pause {ph::PAUSE} else {ph::PLAY}, &label, &tip, false, true)`; Reset becomes `icon_button(ui, ph::ARROW_COUNTER_CLOCKWISE, &tr!("toolbar-reset"), &tr!("toolbar-reset-tip"), false, draping)`. The camera buttons stay text.
  - **New strings:**
    - `toolbar-play-tip = Drape the pieces on the form.`
    - `toolbar-pause-tip = Pause the drape.`
    - `toolbar-reset-tip = Stop draping and go back to arranging the pieces.`
- [ ] **Step 4: Run everything:** `cargo nextest run --workspace`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo fmt --check`. Fix any test whose pattern clicks assumed the old canvas position. They should all go through `canvas_rect`, so none should need changes.
- [ ] **Step 5: Commit:** `feat(editor): pattern tools in a vertical icon strip, view options above the table, scrolling properties; 3D toolbar icons`

### Task 4: Checklist, docs, final checks

**Files:**
- Create: `docs/testing/M5-checklist.md`
- Modify: `README.md` (status line), `docs/specs/2026-10-09-opendrape-design.md` (Status: M5 entry and a pointer to the direction in the M5 spec), `crates/app/tests/workspaces.rs` (nothing new unless review finds gaps)

- [ ] **Step 1: Write the checklist** in plain language, like `docs/testing/M4b-checklist.md`:
  - install the nightly;
  - the five tabs are in the top bar, and Modeling is selected;
  - click each tab: the 3D view stays, and the other four say "Coming in a later update.";
  - Cmd+1…5 and the View menu switch tabs;
  - hover each tool icon: its name and key show;
  - draft, sew and drape the T-shirt (M4b steps) exactly as before;
  - add a piece, go to Texturing, Cmd+Z removes it;
  - switching tabs doesn't put "•" in the title;
  - in dark mode, everything is readable and the selected tool is highlighted.
- [ ] **Step 2: Update the README and spec status:**
  - **README:** "Status: M5: five workspaces (Modeling holds the pattern, sewing and draping tools; the other four are coming)".
  - **Spec:** add an M5 bullet to Status, plus one line under Context: "Since 2026-10-10 the app is organised into five workspaces; see docs/superpowers/specs/2026-10-10-m5-workspaces-design.md for the product direction."
- [ ] **Step 3: Run the full checks:** `cargo fmt --check && cargo clippy --workspace --all-targets -- -D warnings && cargo nextest run --workspace && cargo deny check licenses`.
- [ ] **Step 4: Commit:** `docs: M5 tester checklist and status`
- [ ] **Step 5: Final review:** dispatch one fresh reviewer agent (most capable model) on `git diff main...m5-workspaces` against the spec and this plan's Review Focus. Fix what it confirms; re-run the checks; commit.
- [ ] **Step 6: Hand back to the user:** a short summary. Ask before launching the app on the Mac, and before merging or pushing.
