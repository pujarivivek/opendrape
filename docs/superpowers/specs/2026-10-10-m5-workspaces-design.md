# M5 Workspaces: the app's skeleton (design)

## Why

OpenDrape works now: a student can draft a T-shirt, sew it and drape it on the form (M0–M4b). But
it grew feature by feature, as one crowded screen with default styling and text-only buttons. The user
wants the **UI and the main structure settled before more features arrive**, so that later features
drop into a place that already exists instead of reshuffling the screen each time.

## Product direction (confirmed with the user, 2026-10-09 and 2026-10-10)

OpenDrape is split into **five workspaces**, shown as tabs along the top, Blender-style. Each tab
shows only the tools for its stage of making a garment:

| Workspace | What it will hold |
|---|---|
| **Modeling** | Pattern drafting, seam allowance, notches, internal lines, sewing, arranging on the form, draping, pins. Dress forms and fitted garments arrive here later. **Everything built so far (M0–M4b) lives here** |
| **Finishing** | Topstitching and seam styles, binding and piping, buttons, snaps, zippers, elastic, pleats |
| **Texturing** | Fabrics, each with a look and a physical feel; colourways; image prints placed along the grain |
| **Rendering** | A built-in quick render (offline, accurate), plus a node-based AI render. AI puts a realistic model and background around the garment from several angles, without changing the garment. There are recipes for lookbook, catalogue, tech pack and social media |
| **Animation** | A turntable from the real 3D, plus AI video clips |

These decisions hold for the whole product:
- **AI is an optional plug-in, chosen by the user.** OpenDrape stays free (GPL-3.0) and fully usable offline.
  - AI steps connect either to a ComfyUI server (any PC or college lab machine with a graphics card) or to a cloud AI, using the user's or college's own key.
  - We run no servers and never ship AI models.
- **Connected to other tools.** A connector lets Claude Code, Codex and similar tools open, edit, drape, render and export projects. A Blender add-on sends garments to Blender and brings things back. An in-app chat panel comes after 1.0.
- **Order after M5:**
  1. connector (AI tools + Blender);
  2. Texturing;
  3. AI rendering, preceded by a short test that the AI keeps the garment unchanged;
  4. fitted garments;
  5. Finishing;
  6. Animation;
  7. printable patterns (PDF, DXF);
  8. 1.0.

  Dress forms (Track B) land in Modeling whenever they're ready.

## What M5 delivers

### 1. Five workspace tabs
- The menu row holds File, Edit, View and Help on the left, and the five tabs in the centre: **Modeling · Finishing · Texturing · Rendering · Animation**.
- The selected tab is highlighted. Tabs are buttons with their names as accessible labels.
- **Cmd+1…5** (Ctrl+1…5 on Windows) switch tabs. A new **View** menu lists the five tabs with their shortcuts.
- The app always opens in Modeling. The tab is screen state only: it isn't saved in the project, it isn't an undo step, and switching never marks the project as changed. New and Open keep the current tab.

### 2. The 3D view stays in every tab
- The left 3D panel is the same panel in all five tabs, with the same size, camera, Play/Pause/Reset and drape.
- What you can do in it doesn't change in M5: picking, gizmo, Place at, pins and grabbing work as today. Limiting 3D tools per tab waits until a tab needs it.

### 3. Modeling = today's tools, rearranged like mockup C
- **Tool strip:** the eight pattern tools move from the top toolbar to a **vertical icon strip** down the left edge of the pattern table.
  - Order: Edit (Z), Pen (H), Rectangle (S), Add point (X), Notch (N), Internal line (L), Sew (W), Free sew (F).
  - Hovering shows the name, key and today's tip.
- **Slim bar above the table:** cm / inch, Show lengths, Show seam allowance, Fit (Cmd+0). These stay text controls.
- **Properties panel** (right, 240 px) gains a scroll area, so long sections, such as a piece with its 3D placement, never get cut off on a 768-pixel-high laptop.
- **Status bar:** stays at the bottom of the pattern area, as today.
- **3D toolbar:** Play/Pause and Reset become icon buttons with tooltips. Front, Back, Left and Right stay as small text buttons.
- All behaviour is unchanged: the same keys, number box, drafts, seams, notches, pins and refusals.

### 4. The other four tabs: one "coming soon" page each
- The area right of the 3D view shows a quiet centred card: the tab's name, one line about what it will hold, and "Coming in a later update."
- No buttons, and no milestone numbers (they change).
- Tool letters (H, W, …) do nothing there, because the pattern table isn't open.

### 5. Plain neutral look, tidied
- **Light and dark:** follows the computer's light/dark setting, as today. egui's neutral greys stay; no new palette.
- **One theme module** applied at start-up:
  - consistent spacing (8 px between items, 8 px panel margins);
  - 4 px corner rounding;
  - a clear text scale: body, small and heading;
  - a standard-blue selection highlight, tuned separately for light and dark (light: blue tint with dark-blue text, as in mockup C).
- **Colours in one place:** every colour now hard-coded around the app moves into the theme module, keeping its current value:
  - `Palette` (`editor/paint.rs`), `PIN_COLOUR`, `SEAM_COLOURS` (`editor/seams.rs`);
  - `AXIS_COLOURS` and `BRIGHT` (`arrange/overlay.rs`);
  - `SKIN` and `FABRIC` (`viewport.rs`);
  - the grey of the stats overlay (`app.rs`).

  Later restyling then happens in one file.
- **Icons:** Phosphor icons through `egui-phosphor` 0.14.0. It needs egui 0.36, is licensed MIT OR Apache-2.0 (checked 2026-10-10), and only the regular weight is used. Phosphor's font is registered at start-up, and `ASSETS.md` lists the icon licence.
- The fonts stay egui's defaults.

## Structure in code

Names follow the existing code. The tabs are called **workspaces** in code, because `Stage` already
means the 3D scene around the dress form (`crates/drape/src/stage.rs`, `OpenDrapeApp.stage`, and about 200
other uses).

| New or changed | Role |
|---|---|
| `crates/app/src/workspace.rs` (new) | `enum Workspace { Modeling, Finishing, Texturing, Rendering, Animation }`: `ALL`, `label()`, `blurb()` (both via `tr!`), `shortcut()`; `coming_soon(ui, ws)` draws the placeholder card. A tab that gets real tools later gets its own module then |
| `crates/app/src/theme.rs` (new) | `apply(ctx)` sets the style (spacing, rounding, text sizes, selection colours) for light and dark; it holds the colour constants that move here |
| `crates/app/src/icons.rs` (new) | Registers the Phosphor font. It names the icons used, and has one helper for an icon button whose AccessKit label, tooltip and selected state are set explicitly. Tests keep finding **"Pen (H)"**, **"Play"**, **"Pause"** and **"Reset"** by their labels |
| `crates/app/src/app.rs` | `OpenDrapeApp` gains `workspace: Workspace`, with `workspace()` and `set_workspace()` for tests. `menu_bar()` adds the View menu and the tabs. `ui()` (line 1117) keeps the left `view_3d` panel for every workspace, and its `CentralPanel` matches on the workspace. New `workspace_shortcut()` sits beside `file_shortcut()` (line 753), and `app_undo_redo()` serves the four non-Modeling tabs. The 3D toolbar (line 279) uses icon buttons |
| `crates/app/src/editor/mod.rs` | `toolbar()` (line 403) splits into a vertical `tool_strip()` (left panel inside the editor) and a slim `view_bar()` (top). Properties get a `ScrollArea`. Key handling (`shortcuts`, line 386) is unchanged |
| `crates/app/i18n/en-US/opendrape.ftl` | `workspace-*` labels and blurbs, `coming-soon`, `menu-view` |
| `Cargo.toml` (workspace) and `crates/app/Cargo.toml` | `egui-phosphor = "=0.14.0"` (exact pin, like egui) |

### Keyboard rules
- **Switching tabs:** `workspace_shortcut()` runs before the panels, like `file_shortcut()`. It is ignored while:
  - a text field has focus, or the number box is open;
  - the unsaved-changes question, an error box or the recovery question is open;
  - a 3D drag (gizmo, pin or fabric) is held.
- **Undo and redo:** in Modeling the editor still handles Undo/Redo (Cmd+Z, Shift+Cmd+Z, Ctrl+Y), as today. In the other four tabs `app_undo_redo()` handles the same keys under the same conditions and calls the editor's existing `undo()`/`redo()`. The drape carries the change on through `update_if_edited`, as it does today. The Edit menu works in every tab.
- **Tool letters and Cmd+0 Fit:** only in Modeling.

## Out of scope for M5
- Start screen, pieces and seams list, Settings window, the "Allow AI tools" switch.
- Any tool inside Finishing, Texturing, Rendering or Animation.
- New fonts, saved panel sizes, and per-tab rules for the 3D view.

## Testing
- **Existing tests stay green, unchanged.**
  - `tests/ui.rs` (70 whole-app tests) starts in Modeling.
  - The editor-only tests (`editor`, `details`, `sewing`, `free_sew`, `pins`, `placing`) keep using `common::harness()`. They find tools by "Pen (H)" etc., which the icon helper keeps as AccessKit labels.
- **New `crates/app/tests/workspaces.rs`** (whole app, egui_kittest) checks:
  - **The tab row:** all five tabs are present; Modeling is selected at start.
  - **Switching:** Cmd+3 opens Texturing, which shows its coming-soon text; "Pen (H)" is absent there, and "Play" is still present; clicking a tab switches the same way.
  - **Tool letters:** H typed in Texturing leaves the tool unchanged when you return to Modeling.
  - **Undo:** a piece added in Modeling, then Cmd+Z in Texturing, removes the piece.
  - **Unsaved mark:** switching tabs doesn't mark a saved project as changed.
  - **Shortcuts blocked:** Cmd+3 does nothing while the unsaved-changes question is open, or while a property field is being typed in.
  - **View menu:** its items switch tabs.
  - **Small window:** at 640×480 nothing panics, and the properties panel scrolls.
- `theme.rs` unit test: the moved colour constants keep their exact current values, so nothing on screen changes colour by accident.
- **CI:** fmt, clippy, nextest, cargo-deny (egui-phosphor's MIT OR Apache-2.0 is already allowed) and the Windows WARP smoke test stay green on all three systems.
- **User checklist:** `docs/testing/M5-checklist.md`, in plain language:
  - click all five tabs: the 3D view stays and the others say "coming soon";
  - Cmd+1…5 work;
  - the icons show names and keys on hover;
  - Undo works in every tab;
  - drafting and draping a T-shirt behaves exactly as in M4b.

## Merge note
Dress forms Track B also edits `app.rs` (it replaces the body). Whichever lands second rebases. The
workspace match in `ui()` and Track B's body swap touch different lines.
