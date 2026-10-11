# M5c: the dress form replaces the human avatar, and an Assets section

**Status:** built (2026-10-11). The user approved the design on 2026-10-11; plan: `docs/superpowers/plans/2026-10-11-m5c-dress-forms-and-assets.md`.
**Branch:** `dress-forms-app`. It stacks on `m5b-studio-view`, with `dress-forms` (Track A) merged
in. The merge was clean, and all 978 tests pass on it.
**Builds on:** `docs/superpowers/specs/2026-10-09-dress-forms-design.md`. That design's "Track B"
is this milestone, laid out the way the user asked on 2026-10-11.

## What the user asked for
- "Replace the human avatar with one dress form."
- "Add an Assets section … shown in an option after View. When clicked it shows all the listed
  assets in place of the 2D viewport, and the 3D viewport shows a 3D and 2D viewport switch
  option."

## Decisions
| Topic | Decision |
|---|---|
| Assets layout | The user's idea. **Assets** sits in the menu row after **View**. Clicking it shows the asset list in the right-hand area, where the 2D pattern usually is. While it's open, the 3D area gets a small **3D \| 2D** switch, so the pattern stays one click away |
| What Assets lists | **Dress forms only** for now. Other kinds (fabrics, trims) join the list when they're built; there are no "coming soon" entries |
| Arms | **None for now.** Sleeves still go to the armhole (see "Sleeves" below) and hang loose when draped, like on a real dress form |
| Default form | The women's torso, Classic chart, US 8: the base size it was shaped at |
| MakeHuman | Removed from the app and the repo, as the dress-form design planned |
| Name | Milestone **M5c**. Checklist: `docs/testing/M5c-checklist.md` |

## What the user sees

### The 3D view
- The dress form on its stand replaces the human body, looking like the approved Blender forms
  (changed after the first look, 2026-10-11):
  - a light linen cover (`FORM_SRGB` 208, 200, 193) with a fine weave, slubby yarn and a little
    relief, drawn by the shader (no texture files, so no licence question), fading to plain
    linen where it would be finer than a pixel;
  - fine sewn seams along the tape lines that aren't girths, a little darker than the linen;
  - the measuring tapes round the girths only when "Show measuring tapes" is ticked;
  - a brushed-metal neck cap with a rounded top edge and a smooth outline, and a dark knob, pole
    and base;
  - a navy woven size label low on the front: "OpenDrape", the form and size, and its girths in
    the project's units.
- The camera frames the torso, centred at waist height. The four view buttons, Play/Reset and the
  studio lighting work as before.

### The Assets section
```
File  Edit  View  Assets  Help │ [Modeling]  Finishing  Texturing  Rendering  Animation
┌──────────────────────────┬──────────────────────────────────────────────┐
│ [3D | 2D]           ⌂⌂⌂⌂ │ Dress forms                              ✕   │
│                          │ ┌───────────┐ ┌───────────┐                  │
│                          │ │ thumbnail │ │ thumbnail │                  │
│     the form and         │ │ Women's   │ │ Men's     │                  │
│     the garment          │ │ torso     │ │ torso     │                  │
│                          │ └───────────┘ └───────────┘                  │
│                     ▶ ⟲  │ For dresses, tops, blouses, skirts, kurtas   │
│                          │ Chart  [Classic form ▾]                      │
│                          │ Size   [US 8 / UK 12 · bust 90 cm ▾]         │
│                          │ ▸ Custom measurements…                       │
│                          │ ☑ Show tape lines                            │
└──────────────────────────┴──────────────────────────────────────────────┘
```
- **Opening and closing:** click **Assets** in the menu row; it stays highlighted while open.
  Close it by clicking Assets again, with the ✕, or by switching tab (choosing a tab means "show
  me that stage"). Assets opens in any tab.
- **Cards:** one per form, with a small picture, its name, and the garments it suits. The
  project's form is highlighted. Clicking the other card switches to that form at its base size
  (women US 8, men 40).
- **Pictures:** the app's own 3D renderer draws each form at its base size, in the studio light,
  the first time Assets opens. When the graphics can't draw them, the cards show a plain
  form icon instead.
- **Chart and size:** Classic form or Everyday body, then a size from that chart, written as in
  the dress-form design ("US 8 / UK 12 · bust 90 cm"; men: "40 · chest 101.5 cm"). Changing the
  chart keeps the nearest size by bust (men: chest).
- **Custom measurements…** opens the form's measurements (women: bust, under-bust, waist, hip,
  neck, shoulder length, back waist length, waist to hip; men: the same, with chest and no
  under-bust), pre-filled from the current size.
  - Values are typed in the project's units, and Enter applies them.
  - A value the form can't take is refused with the form's own message ("Waist can be 50–130 cm
    on this form"), and the old value stays.
  - Below the fields are read-only "measured" values: front waist length, apex to apex (women),
    and high hip.
- **Show measuring tapes** turns the girth tapes on or off (off at first). It's a viewing choice,
  remembered with the other View settings (`view.json`), and not saved in the project.

### The 3D | 2D switch
- It appears at the top left of the left area, only while Assets is open in the Modeling tab
  (the only tab with a 2D pattern today).
- **2D** shows the full pattern editor in the left area: the same tools, shortcuts and undo, just
  in the other area. **3D** brings the 3D view back.
- Closing Assets puts the pattern back on the right and the 3D view back on the left.

### What a form change does
- Changing the form, chart, size or a custom measurement is **one undo step**, and it marks the
  project unsaved.
- It **restarts the drape**: the cloth goes back to the pieces' start positions, as Reset does.
- **Pieces that would start inside the new form are moved straight out until they clear it**, with
  the same clearance Place at uses. This is part of the same undo step. Without it, going up
  several sizes would start the cloth inside the form. Pieces left further away from a smaller
  form are left alone: they fall in when draped, as now.
- Building a form takes a few milliseconds, so the switch is instant.

### Sleeves
- The form has no arms, so each side gets an **imaginary arm line**. It hangs from the armhole and
  leans out a little, like an arm at rest.
- **Place at → Left armhole / Right armhole** (renamed from "Left arm / Right arm") wraps a sleeve
  round that line, as today.
- When draped, nothing holds the sleeve up, so it hangs loose from the armhole, as it would on a
  real dress form. Soft arms are a later step.

## Saved in the project (format version 5)
- A new `form` field holds:
  - the form id (`women-torso` or `men-torso`);
  - the size choice: either a chart (`classic` or `everyday`) and a size label, or Custom;
  - the measurements in millimetres that the form was built at.
- Storing the measurements means a reopened file never silently changes its form, even if a later
  version retunes a chart.
- **Older files** (versions 1–4) open on the default form. A frozen `crates/io/tests/fixtures/v5` file joins the
  older fixtures.
- **A file this version can't build is refused, with a message naming the problem.** That covers
  a form id this version doesn't have, and measurements outside the form's ranges. Any other
  answer would open the file on a different form without saying so.

## How it's built (for the plan)
- **`crates/core`:**
  - `Project.form: FormChoice` (plain data, serde), and `SCHEMA_VERSION = 5`;
  - a default for older files;
  - the form change as a document edit that is undoable like any other.
- **`crates/io`:** the v4 → v5 step adds the default form; the v5 fixture; the refusals above.
- **`crates/drape`:**
  - `Stage::from_form(&BuiltForm)` replaces `Stage::makehuman()`.
  - **Collider:** the torso plus the floor, using sim's `CompoundCollider`, which replaces
    `BodyAndFloor`.
  - **Frame:** the shoulder height comes from the form's `shoulder` station, not 0.82 × height,
    and the centre line is the stand's pole.
  - **Arms:** `arms()` returns the two imaginary arm lines, built from the armhole landmarks.
  - **What's drawn:** the stage keeps the torso, tapes and stand as separate meshes, so the view
    can colour each one.
  - The one-per-app `Stage::shared()` goes. The app holds the current stage and swaps it when the
    form changes. Tests build theirs from the default form.
- **`crates/render`:** a small way to draw a mesh set into an off-screen picture, for the Assets
  thumbnails. It reuses the studio renderer, so the light matches.
- **`crates/app`:**
  - `assets.rs` holds the Assets section;
  - the menu-row toggle and the 3D | 2D switch;
  - `Viewport` draws torso, tapes and stand, and swaps them on a form change;
  - the sim runner takes the new stage and resets;
  - pieces are re-seated after a form change;
  - every new string goes through Fluent (`chart-classic`, `chart-everyday`, form names).
- **Retiring MakeHuman** removes:
  - `assets/body/female_average.odb`, `scripts/fetch-makehuman.sh`, `xtask/src/makehuman.rs` and
    `xtask body`;
  - the CI byte-compare job and `crates/body/tests/asset.rs`;
  - the M1 test-kit drapes on the MakeHuman body (the form drapes from Track A cover them);
  - the MakeHuman row in `ASSETS.md`.
- **Gates move to the form:**
  - **Skirt:** `project_skirt` drapes on the default form.
  - **T-shirt:** `project_tshirt` becomes "a drafted T-shirt drapes on the dress form with its
    sleeves hanging". It checks:
    - no NaN;
    - welded seams;
    - nothing inside the form;
    - the sleeves below their armholes and settled.
- **Docs:**
  - the main spec's avatar rows and the role of `crates/body`;
  - README status;
  - `docs/testing/M5c-checklist.md`;
  - the memory notes.

## Testing
- **UI (egui_kittest):**
  - **Assets view:** opens and closes from the menu row, the ✕ and a tab switch.
  - **Picking a form:** clicking the men's card switches form, marks the project unsaved, and Cmd+Z
    brings the women's form back.
  - **Chart and size:** changing the chart and size updates the 3D form.
  - **Custom:** a custom waist applies on Enter; an out-of-range value is refused and the message
    shows.
  - **3D | 2D switch:** it appears only while Assets is open in Modeling. 2D shows the pattern
    tools on the left, and the Pen shortcut works there.
  - **Small screens:** a tiny window with Assets open doesn't crash.
- **Project file:**
  - v5 round trip;
  - v1–v4 fixtures open on the default form;
  - an unknown form id and an out-of-range custom value are refused with their messages.
- **Drape:**
  - the stage's shoulder height matches the form's station;
  - the arm lines start at the armholes and lean out;
  - after a size change from US 8 to US 18, no piece starts inside the form;
  - skirt and T-shirt gates on the form.
- **The full suite and CI:** fmt, clippy, nextest, cargo-deny, and the GL shader translation test.

## Not in this step
- Soft arms; fabrics, trims or avatars in Assets.
- Grouping forms by garment type; the "suits" line covers it while there are two forms.
- An Indian size chart (it waits on NIFT permission).

## Known and unchanged
These are open from Track A and stay as they are:
- the men's "40" chart rows differ from the approved men's form (Classic waist +7 cm, hip −3 cm);
- the women's charts reach a bust of 81.5–116.5 cm, not 80–125.
