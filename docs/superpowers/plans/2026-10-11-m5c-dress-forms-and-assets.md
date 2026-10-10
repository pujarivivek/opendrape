# M5c Dress forms and Assets Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** The dress form replaces the MakeHuman body everywhere (draping, the 3D view, the tests), the project file saves which form and size, and a new Assets section lets the student pick the form, chart, size or custom measurements.

**Architecture:** `opendrape-core` gets a plain-data `FormChoice` on `Project` (format version 5). `opendrape-drape` builds a `Stage` from a `FormChoice` through `opendrape-body`'s forms. The stage has a compound collider, the shoulder station, imaginary arm lines and separate torso/tape/stand meshes; it can also move pieces clear of a bigger form. The app holds the current stage and rebuilds it whenever the project's form differs (edit, undo, open). The Assets section is a toggle in the menu row that replaces the right-hand area, with a 3D | 2D switch on the left area.

**Tech Stack:** Rust 2024, egui/eframe 0.36.2, wgpu 30, egui_kittest, cargo-nextest; existing crates `opendrape-core`, `-io`, `-body`, `-sim`, `-mesh`, `-drape`, `-render`, `-app`, `-testkit`.

**Spec:** `docs/superpowers/specs/2026-10-11-m5c-dress-forms-and-assets-design.md` (and the earlier `docs/superpowers/specs/2026-10-09-dress-forms-design.md`, whose Track B this is).

## Global Constraints

- The default form is the women's torso, Classic chart, US 8: `bust 900, under_bust 750, waist 675, hip 930, neck 340, shoulder_length 130, back_waist_length 420, waist_to_hip 205` (mm).
- `SCHEMA_VERSION = 5`. Older files (1–4) open on the default form. A frozen `crates/io/tests/fixtures/v5/project.json` is added; older fixtures are never edited.
- A file whose form can't be built (unknown id, a measurement out of range) is refused with a message naming the problem; it never opens on a different form.
- Changing the form, chart, size or a custom measurement is one undo step, marks the project unsaved, restarts the drape, and moves pieces that would start inside the new form straight out until clear.
- No arms. Sleeves are placed round an imaginary arm line from each armhole; "Place at left/right arm" reads "Place at left/right armhole".
- Assets sits after View in the menu row; it closes with a second click, its ✕, or a tab switch. The 3D | 2D switch shows only while Assets is open in Modeling.
- MakeHuman leaves the repo: asset, `.odb` format, xtask `body`, fetch script, CI step, `ASSETS.md` row.
- Every user-facing string goes through Fluent (`crates/app/i18n/en-US/opendrape.ftl`). Never the word "CLO".
- Ask before launching the app; never push or merge to `main` without asking.
- Never `cargo clean` or delete a whole `target/`; `.claude/worktrees/dress-forms/target/forms` holds the approved form files.
- Clippy `-D warnings`; no `#[allow]` (the codebase has none).

## Review Focus

1. **Going up several sizes with pieces already placed:** none of them starts inside the new form (Task 3 unit test, Task 7 app test).
2. **Changing the form while the drape is playing:** the drape restarts on the new form, no stale frame from the old drape is shown, nothing panics (Task 5 test).
3. **Undo and redo of a form change:** the 3D form follows the project, both ways (Task 5 test).
4. **Opening files:** v1–v4 open on the default form; a v5 with an unknown form id or an out-of-range custom value is refused with its message and the open project is unchanged (Tasks 1 and 5).
5. **Typing in a Custom measurement field:** keys don't trigger tools (H, P…), Enter applies, a bad value shows the range message and changes nothing; a tiny window with Assets open doesn't crash (Tasks 6 and 7).

---

### Task 1: The form choice in the project file (format version 5)

**Files:**
- Create: `crates/core/src/form.rs`
- Modify: `crates/core/src/lib.rs` (module + re-exports), `crates/core/src/project.rs` (field, `SCHEMA_VERSION`, `ModelError::BadForm`, `check`)
- Create: `crates/io/tests/fixtures/v5/project.json`
- Modify: `crates/io/tests/fixtures.rs`, `crates/io/tests/fixtures/README.md`, `crates/io/src/lib.rs` (doc comments only)

**Interfaces:**
- Produces: `opendrape_core::{FormChoice, FormSize}`; `FormChoice::default()`; `FormChoice::check() -> Result<(), ModelError>`; `Project.form: FormChoice` (`#[serde(default)]`); `ModelError::BadForm`; `SCHEMA_VERSION == 5`.

- [ ] **Step 1: Write the failing tests** in `crates/core/src/form.rs` (module created with the tests first, types stubbed so it compiles is not allowed: write tests, run, see the compile error):

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ModelError, Project};

    #[test]
    fn the_default_is_the_womens_classic_us_8() {
        let d = FormChoice::default();
        assert_eq!(d.id, "women-torso");
        assert_eq!(
            d.size,
            FormSize::Chart { chart: "classic".into(), label: "US 8".into() }
        );
        assert_eq!(d.measurements["bust"], 900.0);
        assert_eq!(d.measurements["waist_to_hip"], 205.0);
        assert_eq!(d.measurements.len(), 8);
        assert_eq!(d.check(), Ok(()));
    }

    #[test]
    fn a_choice_round_trips_through_json_with_its_size_kind() {
        let mut c = FormChoice::default();
        c.size = FormSize::Custom;
        c.measurements.insert("waist".into(), 712.5);
        let json = serde_json::to_string(&c).unwrap();
        assert!(json.contains(r#""kind":"custom""#), "{json}");
        assert_eq!(serde_json::from_str::<FormChoice>(&json).unwrap(), c);
    }

    #[test]
    fn malformed_choices_are_refused() {
        let bad = |f: &dyn Fn(&mut FormChoice)| {
            let mut c = FormChoice::default();
            f(&mut c);
            c.check()
        };
        assert_eq!(bad(&|c| c.id.clear()), Err(ModelError::BadForm));
        assert_eq!(bad(&|c| c.id = "x".repeat(65)), Err(ModelError::BadForm));
        assert_eq!(
            bad(&|c| c.size = FormSize::Chart { chart: String::new(), label: "US 8".into() }),
            Err(ModelError::BadForm)
        );
        assert_eq!(bad(&|c| c.measurements.clear()), Err(ModelError::BadForm));
        assert_eq!(
            bad(&|c| { c.measurements.insert("waist".into(), f64::NAN); }),
            Err(ModelError::BadForm)
        );
        assert_eq!(
            bad(&|c| { c.measurements.insert("waist".into(), 0.5); }),
            Err(ModelError::BadForm)
        );
        assert_eq!(
            bad(&|c| {
                for k in 0..40 {
                    c.measurements.insert(format!("m{k}"), 100.0);
                }
            }),
            Err(ModelError::BadForm)
        );
        assert_eq!(
            bad(&|c| { c.measurements.insert(String::new(), 100.0); }),
            Err(ModelError::BadForm)
        );
    }

    #[test]
    fn a_project_checks_its_form_and_an_older_file_gets_the_default() {
        let mut p = Project::new();
        assert_eq!(p.form, FormChoice::default());
        p.form.id.clear();
        assert_eq!(p.check(), Err(ModelError::BadForm));
        let old: Project = serde_json::from_str(r#"{"schema_version":4}"#).unwrap();
        assert_eq!(old.form, FormChoice::default());
    }
}
```

- [ ] **Step 2: Run them, see them fail**

Run: `cargo nextest run -p opendrape-core form::`
Expected: compile errors (`FormChoice` not found).

- [ ] **Step 3: Implement `form.rs`** above the tests:

```rust
//! Which dress form a project drapes on, and at what size: plain data. Whether a form with that
//! id exists and takes those measurements is for `opendrape-drape` to say (it builds it).

use crate::ModelError;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Longest form id, chart name or size label a project may hold.
pub const MAX_FORM_NAME: usize = 64;
/// Most measurements a form choice may hold.
pub const MAX_FORM_MEASUREMENTS: usize = 32;
/// Every measurement is within this range (mm): far wider than any form's, it only stops a
/// damaged file.
pub const FORM_MM: std::ops::RangeInclusive<f64> = 1.0..=5000.0;

/// The dress form a project drapes on: which one, the size picked, and the measurements it is
/// built at.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FormChoice {
    /// A bundled form: `women-torso` or `men-torso`.
    pub id: String,
    pub size: FormSize,
    /// What the form is built at (mm, by name: `bust`, `waist`, …). Kept in the file, so a
    /// later change to a chart never reshapes a saved project's form.
    pub measurements: BTreeMap<String, f64>,
}

/// How the size was picked.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum FormSize {
    /// A row of one of the form's charts: `classic` or `everyday`, and its label (`US 8`).
    Chart { chart: String, label: String },
    /// Measurements the student typed.
    Custom,
}

impl Default for FormChoice {
    /// The women's torso, Classic chart, US 8: the size it was shaped at. `opendrape-drape`
    /// checks that this matches the bundled chart.
    fn default() -> Self {
        let mm = [
            ("back_waist_length", 420.0),
            ("bust", 900.0),
            ("hip", 930.0),
            ("neck", 340.0),
            ("shoulder_length", 130.0),
            ("under_bust", 750.0),
            ("waist", 675.0),
            ("waist_to_hip", 205.0),
        ];
        Self {
            id: "women-torso".into(),
            size: FormSize::Chart {
                chart: "classic".into(),
                label: "US 8".into(),
            },
            measurements: mm.into_iter().map(|(k, v)| (k.to_string(), v)).collect(),
        }
    }
}

impl FormChoice {
    /// Well formed: its names present and not too long, one to [`MAX_FORM_MEASUREMENTS`]
    /// measurements, each a number in [`FORM_MM`].
    pub fn check(&self) -> Result<(), ModelError> {
        let name_ok = |s: &str| !s.is_empty() && s.len() <= MAX_FORM_NAME;
        let size_ok = match &self.size {
            FormSize::Chart { chart, label } => name_ok(chart) && name_ok(label),
            FormSize::Custom => true,
        };
        let measurements_ok = (1..=MAX_FORM_MEASUREMENTS).contains(&self.measurements.len())
            && self
                .measurements
                .iter()
                .all(|(k, v)| name_ok(k) && FORM_MM.contains(v));
        if name_ok(&self.id) && size_ok && measurements_ok {
            Ok(())
        } else {
            Err(ModelError::BadForm)
        }
    }
}
```

In `crates/core/src/lib.rs`: `mod form;` and `pub use form::{FORM_MM, FormChoice, FormSize, MAX_FORM_MEASUREMENTS, MAX_FORM_NAME};` (follow the file's existing `mod`/`pub use` layout).

In `crates/core/src/project.rs`:
- `SCHEMA_VERSION = 5`, and extend its doc: "version 5 added the dress form (2026-10-11)".
- On `Project`, after `pins`:
  ```rust
  /// The dress form the garment drapes on.
  #[serde(default)]
  pub form: FormChoice,
  ```
- `Project::new()` sets `form: FormChoice::default()`.
- `ModelError::BadForm`, displayed as `"the dress form is invalid"`.
- `check()` ends with `self.check_pins()?; self.form.check()`.
- The test at the old line 1463 (`assert_eq!(SCHEMA_VERSION, 4)`) becomes 5.

- [ ] **Step 4: Run the core tests**

Run: `cargo nextest run -p opendrape-core`
Expected: all pass.

- [ ] **Step 5: The frozen v5 fixture.** Write `crates/io/tests/fixtures/v5/project.json` by hand: one rectangle piece (copy the v4 fixture's simplest piece, id 1), `"schema_version": 5`, `"units": "cm"`, and

```json
"form": {
  "id": "men-torso",
  "size": {"kind": "custom"},
  "measurements": {"back_waist_length": 455.0, "chest": 1015.0, "hip": 985.0, "neck": 395.0,
                   "shoulder_length": 160.0, "waist": 880.0, "waist_to_hip": 195.0}
}
```

Add to `crates/io/tests/fixtures.rs`:

```rust
#[test]
fn format_v5_still_opens_with_its_form() {
    let loaded = opendrape_io::from_bytes(&odp(include_str!("fixtures/v5/project.json")))
        .expect("the frozen v5 project opens");
    assert_eq!(loaded.form.id, "men-torso");
    assert_eq!(loaded.form.size, FormSize::Custom);
    assert_eq!(loaded.form.measurements["waist"], 880.0);
    assert_eq!(loaded.form.measurements.len(), 7);
    assert_eq!(loaded.pieces.len(), 1);
}

#[test]
fn older_formats_open_on_the_default_form() {
    for json in [
        include_str!("fixtures/v1/project.json"),
        include_str!("fixtures/v2/project.json"),
        include_str!("fixtures/v3/project.json"),
        include_str!("fixtures/v4/project.json"),
    ] {
        let loaded = opendrape_io::from_bytes(&odp(json)).unwrap();
        assert_eq!(loaded.form, FormChoice::default());
    }
}

#[test]
fn a_malformed_form_is_refused_as_invalid() {
    let json = include_str!("fixtures/v5/project.json").replace(r#""men-torso""#, r#""""#);
    assert!(matches!(
        opendrape_io::from_bytes(&odp(&json)),
        Err(opendrape_io::OdpError::Invalid(opendrape_core::ModelError::BadForm))
    ));
}
```

(Add `FormChoice, FormSize` to the file's `use opendrape_core::{…}`.) Update `fixtures/README.md` with "v5 (M5c: the dress form)". In `crates/io/src/lib.rs` the test that says "version 4 is typed at once" now says "version 5"; no code change is needed (serde's default fills `form` in for versions 1–4).

- [ ] **Step 6: Run io and the whole workspace build**

Run: `cargo nextest run -p opendrape-io && cargo build --workspace --all-targets`
Expected: pass; any `Project { … }` literal elsewhere that now lacks `form` fails to compile: add `form: FormChoice::default()` there (or `..Project::new()`).

- [ ] **Step 7: Commit**

```bash
git add crates/core crates/io
git commit -m "feat(core,io): the project saves its dress form (format version 5)"
```

---

### Task 2: A stage built from a dress form

**Files:**
- Create: `crates/drape/src/choice.rs`
- Modify: `crates/drape/src/stage.rs`, `crates/drape/src/lib.rs`, `crates/drape/src/build.rs`, `crates/drape/src/live.rs`, `crates/app/src/sim_runner.rs`, `crates/testkit/tests/project_skirt.rs`, `crates/testkit/tests/project_tshirt.rs` (the `drape_collider` call sites only)

**Interfaces:**
- Consumes: `FormChoice`, `FormSize` (Task 1); `opendrape_body::form::{Form, Chart, BuiltForm, Quality, SizeError, Measurements}`; `opendrape_sim::CompoundCollider`.
- Produces:
  - `opendrape_drape::choice::{FormProblem, charts, chart_choice, base_choice, nearest_in, with_measurement, build_form, girth_name}`:
    - `pub enum FormProblem { Unknown(String), Size(SizeError) }` with `Display`;
    - `pub fn charts(form_id: &str) -> Vec<(String, Chart)>` (kind `classic`/`everyday`, chart), picker order;
    - `pub fn chart_choice(form_id: &str, kind: &str, label: &str) -> Option<FormChoice>`;
    - `pub fn base_choice(form_id: &str) -> Option<FormChoice>` (Classic, the form's `base_size`);
    - `pub fn nearest_in(choice: &FormChoice, kind: &str) -> Option<FormChoice>` (nearest row by the girth);
    - `pub fn with_measurement(choice: &FormChoice, name: &str, mm: f64) -> FormChoice` (Custom);
    - `pub fn girth_name(form_id: &str) -> Option<String>` (`bust` or `chest`: the form's first input);
    - `pub fn build_form(choice: &FormChoice) -> Result<BuiltForm, FormProblem>`.
  - `Stage::from_form(choice: FormChoice, built: &BuiltForm) -> Option<Stage>`, `Stage::for_choice(choice: &FormChoice) -> Result<Stage, FormProblem>`.
  - `Stage::choice(&self) -> Option<&FormChoice>` (None for `from_mesh` stages), `Stage::waist_y(&self) -> f64`, `Stage::measured(&self) -> &Measurements`, `Stage::tapes_mesh(&self) -> (&[Vec3], &[[u32; 3]])`, `Stage::stand_mesh(&self) -> (&[Vec3], &[[u32; 3]])` (both empty for `from_mesh`).
  - `Stage::drape_collider(&self) -> &CompoundCollider` (was `BodyAndFloor<'_>`; callers pass it straight: `step(Some(collider))`). `BodyAndFloor` is deleted.

- [ ] **Step 1: Write the failing tests** at the end of `stage.rs`'s test module:

```rust
    fn form_stage(choice: &FormChoice) -> Stage {
        Stage::for_choice(choice).expect("a bundled size")
    }

    #[test]
    fn a_form_stage_has_its_shoulders_waist_and_centre_line_from_the_form() {
        let choice = FormChoice::default();
        let built = crate::choice::build_form(&choice).unwrap();
        let stage = form_stage(&choice);
        assert_eq!(stage.choice(), Some(&choice));
        assert!((stage.shoulder_y() - built.stations["shoulder"]).abs() < 1e-12);
        assert!((stage.waist_y() - built.stations["waist"]).abs() < 1e-12);
        // The pole is the centre line: rays from it find the torso all round at the waist.
        use std::f64::consts::{FRAC_PI_2, PI};
        for a in [0.0, FRAC_PI_2, PI, -FRAC_PI_2] {
            let d = stage.surface_distance(a, stage.waist_y()).expect("the waist");
            assert!((0.05..0.2).contains(&d), "{a}: {d}");
        }
        assert!(stage.signed_distance(DVec3::new(0.0, stage.waist_y(), 0.0)) < 0.0);
        // Its own signed distance is the torso's alone; the collider also has the floor.
        assert!(stage.signed_distance(DVec3::new(2.0, -0.1, 2.0)) > 0.0);
        assert!(
            (opendrape_sim::Solid::signed_distance(stage.drape_collider(), DVec3::new(2.0, -0.1, 2.0))
                + 0.1)
                .abs()
                < 1e-9
        );
        assert_eq!(stage.measured(), &built.measured);
        assert!(!stage.tapes_mesh().1.is_empty() && !stage.stand_mesh().1.is_empty());
        let low = stage.stand_mesh().0.iter().map(|p| p.y).fold(f32::MAX, f32::min);
        assert!(low.abs() < 1e-4, "the stand stands on the floor: {low}");
    }

    #[test]
    fn a_form_has_an_imaginary_arm_line_from_each_armhole_with_no_surface() {
        let stage = form_stage(&FormChoice::default());
        let [left, right] = *stage.arms().expect("imaginary arms");
        let mirror = |v: DVec3| DVec3::new(-v.x, v.y, v.z);
        assert!((right.shoulder - mirror(left.shoulder)).length() < 1e-9);
        assert!((right.direction - mirror(left.direction)).length() < 1e-9);
        let lean = left.direction.y.abs().acos().to_degrees();
        assert!(left.direction.x > 0.0 && (FORM_ARM_LEAN_DEG - 1e-6..FORM_ARM_LEAN_DEG + 1e-6).contains(&lean));
        // Clear of the torso by more than a sleeve's starting radius, at the shoulder's height.
        assert!(
            stage.signed_distance(left.shoulder) > opendrape_mesh::place::ARM_FALLBACK_RADIUS_M,
            "{}",
            stage.signed_distance(left.shoulder)
        );
        assert!((left.shoulder.y - stage.shoulder_y()).abs() < 0.08, "{}", left.shoulder);
        assert_eq!(stage.arm_surface_distance(0, 0.2, 0.0), None);
        assert_eq!(stage.arm_surface_distance(1, 0.2, 1.0), None);
    }

    #[test]
    fn a_sleeve_placed_at_an_armhole_starts_clear_of_the_form() {
        use opendrape_core::{Piece, Point2};
        let stage = form_stage(&FormChoice::default());
        let mut pr = Project::new();
        let id = pr.add_piece(Piece::rectangle(PieceId(0), "Sleeve", Point2::new(0.0, 0.0), 340.0, 200.0));
        for arm in [0, 1] {
            let placed = stage.place_at_arm(&pr, id, arm).expect("an arm");
            let shape = &geom::shapes(&pr)[0];
            let outline = geom::outline_points(&shape.piece, 0.5);
            let (lo, hi) = (
                outline.iter().fold(Point2::new(f64::MAX, f64::MAX), |a, p| Point2::new(a.x.min(p.x), a.y.min(p.y))),
                outline.iter().fold(Point2::new(f64::MIN, f64::MIN), |a, p| Point2::new(a.x.max(p.x), a.y.max(p.y))),
            );
            let centre = lo.lerp(hi, 0.5);
            for q in &outline {
                assert!(stage.signed_distance(place::apply(&placed, centre, *q)) > 0.0);
            }
        }
    }
```

And in a new test module at the end of `choice.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_core_default_is_the_bundled_womens_classic_us_8() {
        assert_eq!(base_choice("women-torso"), Some(FormChoice::default()));
    }

    #[test]
    fn each_form_has_a_classic_and_an_everyday_chart() {
        for id in Form::IDS {
            let kinds: Vec<String> = charts(id).into_iter().map(|(k, _)| k).collect();
            assert_eq!(kinds, ["classic", "everyday"], "{id}");
        }
        assert_eq!(girth_name("women-torso").as_deref(), Some("bust"));
        assert_eq!(girth_name("men-torso").as_deref(), Some("chest"));
        assert_eq!(base_choice("men-torso").unwrap().size,
            FormSize::Chart { chart: "classic".into(), label: "40".into() });
    }

    #[test]
    fn switching_chart_keeps_the_nearest_size_by_girth() {
        let us8 = FormChoice::default(); // bust 900
        let everyday = nearest_in(&us8, "everyday").unwrap();
        let bust = |c: &FormChoice| c.measurements["bust"];
        for (_, row) in charts("women-torso").into_iter().filter(|(k, _)| k == "everyday") {
            for s in &row.sizes {
                assert!((bust(&everyday) - 900.0).abs() <= (s.mm["bust"] - 900.0).abs());
            }
        }
        assert!(matches!(everyday.size, FormSize::Chart { ref chart, .. } if chart == "everyday"));
    }

    #[test]
    fn a_custom_measurement_keeps_the_rest_and_says_custom() {
        let c = with_measurement(&FormChoice::default(), "waist", 712.0);
        assert_eq!(c.size, FormSize::Custom);
        assert_eq!(c.measurements["waist"], 712.0);
        assert_eq!(c.measurements["bust"], 900.0);
    }

    #[test]
    fn what_cannot_be_built_says_why() {
        let mut c = FormChoice::default();
        c.id = "child-torso".into();
        assert_eq!(build_form(&c).err(), Some(FormProblem::Unknown("child-torso".into())));
        let c = with_measurement(&FormChoice::default(), "waist", 2000.0);
        assert!(matches!(build_form(&c), Err(FormProblem::Size(ref e)) if e.measurement == "waist"));
        assert!(chart_choice("women-torso", "classic", "US 99").is_none());
        assert!(chart_choice("women-torso", "nordic", "US 8").is_none());
    }
}
```

- [ ] **Step 2: Run, see them fail**

Run: `cargo nextest run -p opendrape-drape`
Expected: compile errors (`for_choice`, `choice` module missing).

- [ ] **Step 3: Implement `choice.rs`**

```rust
//! The dress form a project names (`FormChoice`), made real: its charts, its sizes, and the
//! built form. The one place the app and the tests turn a choice into a form.

use opendrape_body::form::{BuiltForm, Chart, Form, Quality, SizeError};
use opendrape_core::{FormChoice, FormSize};

/// Why a choice can't be built.
#[derive(Clone, Debug, PartialEq)]
pub enum FormProblem {
    /// No bundled form has this id.
    Unknown(String),
    /// The form can't take one of the measurements.
    Size(SizeError),
}

impl std::fmt::Display for FormProblem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Unknown(id) => write!(f, "there is no dress form called \"{id}\" in this version"),
            Self::Size(e) => e.fmt(f),
        }
    }
}

impl std::error::Error for FormProblem {}

/// The form's charts, in picker order, each with its kind (`classic`, `everyday`): the part of
/// its id after the form's.
pub fn charts(form_id: &str) -> Vec<(String, Chart)> {
    let prefix = format!("{form_id}-");
    Chart::for_form(form_id)
        .into_iter()
        .filter_map(|c| Some((c.id.strip_prefix(&prefix)?.to_string(), c)))
        .collect()
}

/// Row `label` of the form's `kind` chart.
pub fn chart_choice(form_id: &str, kind: &str, label: &str) -> Option<FormChoice> {
    let (_, chart) = charts(form_id).into_iter().find(|(k, _)| k == kind)?;
    let row = chart.size(label)?;
    Some(FormChoice {
        id: form_id.to_string(),
        size: FormSize::Chart { chart: kind.to_string(), label: label.to_string() },
        measurements: row.mm.clone(),
    })
}

/// The form at the size it was shaped at, from its Classic chart.
pub fn base_choice(form_id: &str) -> Option<FormChoice> {
    let form = Form::bundled(form_id)?;
    chart_choice(form_id, "classic", &form.file().base_size)
}

/// The girth sizes are known by: the form's first input (`bust`, or `chest`).
pub fn girth_name(form_id: &str) -> Option<String> {
    Form::bundled(form_id)?.inputs().into_iter().next().map(|(name, _)| name)
}

/// The row of the form's `kind` chart nearest `choice` by the girth.
pub fn nearest_in(choice: &FormChoice, kind: &str) -> Option<FormChoice> {
    let girth = girth_name(&choice.id)?;
    let now = *choice.measurements.get(&girth)?;
    let (_, chart) = charts(&choice.id).into_iter().find(|(k, _)| k == kind)?;
    let row = chart
        .sizes
        .iter()
        .min_by(|a, b| (a.mm[&girth] - now).abs().total_cmp(&(b.mm[&girth] - now).abs()))?;
    chart_choice(&choice.id, kind, &row.label)
}

/// `choice` with measurement `name` set to `mm`: a custom size.
pub fn with_measurement(choice: &FormChoice, name: &str, mm: f64) -> FormChoice {
    let mut out = choice.clone();
    out.size = FormSize::Custom;
    out.measurements.insert(name.to_string(), mm);
    out
}

/// The form `choice` names, built at its measurements.
pub fn build_form(choice: &FormChoice) -> Result<BuiltForm, FormProblem> {
    let form = Form::bundled(&choice.id).ok_or_else(|| FormProblem::Unknown(choice.id.clone()))?;
    form.build(&choice.measurements, Quality::Standard).map_err(FormProblem::Size)
}
```

(If `Form::build` doesn't check ranges itself and an out-of-range waist builds anyway, the `what_cannot_be_built_says_why` test fails: then add a range check here against `form.inputs()` returning `SizeError { measurement, min_mm, max_mm }` before building, and ledger it.)

- [ ] **Step 4: Rework `Stage`** in `stage.rs`:
  - Fields: `choice: Option<FormChoice>`, `positions`, `triangles` (the torso), `tapes: (Vec<Vec3>, Vec<[u32; 3]>)`, `stand: (Vec<Vec3>, Vec<[u32; 3]>)`, `torso: BodyCollider` (for rays and the stage's own signed distance), `collider: CompoundCollider` (torso + floor at 0), `shoulder_y`, `waist_y`, `arms: Option<[Arm; 2]>`, `arm_surfaces: bool`, `measured: Measurements`.
  - `CompoundCollider::new(parts, floor)` takes `Vec<BodyCollider>`; `BodyCollider` isn't `Clone`, so build it twice from the same mesh (one for rays, one inside the compound). Ledger this if it costs noticeable time; it's tens of ms.
  - `from_mesh` keeps its behaviour (arms found by cutting, `arm_surfaces: true`, shoulders at `SHOULDER_SHARE`, `waist_y = 0.62 × height`, empty tapes and stand, empty `measured`, `choice: None`).
  - New constants and `from_form`:

```rust
/// A dress form has no arms: each side gets an imaginary arm line for Place at → armhole. It
/// starts this far (m) out from the armhole plate's centre, at the shoulder point's height...
pub const FORM_ARM_OUT_M: f64 = 0.10;
/// ...leans out this far from straight down...
pub const FORM_ARM_LEAN_DEG: f64 = 20.0;
/// ...and is this long (m).
pub const FORM_ARM_LENGTH_M: f64 = 0.6;

impl Stage {
    /// The stage for a dress form built for `choice`: its torso to drape on (with the floor),
    /// its tapes and stand to draw, its shoulder and waist stations, and an imaginary arm line
    /// from each armhole (the form has no arms; nothing is measured round them). None when the
    /// solver's collider refuses the torso.
    pub fn from_form(choice: FormChoice, built: &BuiltForm) -> Option<Self> {
        let mesh = &built.torso;
        let torso = BodyCollider::new(&mesh.positions, &mesh.triangles).ok()?;
        let parts = vec![BodyCollider::new(&mesh.positions, &mesh.triangles).ok()?];
        Some(Self {
            choice: Some(choice),
            positions: mesh.positions.clone(),
            triangles: mesh.triangles.clone(),
            tapes: (built.tapes.positions.clone(), built.tapes.triangles.clone()),
            stand: (built.stand.positions.clone(), built.stand.triangles.clone()),
            torso,
            collider: CompoundCollider::new(parts, Some(0.0)),
            shoulder_y: *built.stations.get("shoulder")?,
            waist_y: *built.stations.get("waist")?,
            arms: imaginary_arms(built),
            arm_surfaces: false,
            measured: built.measured.clone(),
        })
    }

    /// The stage for the form `choice` names, or why it can't be built.
    pub fn for_choice(choice: &FormChoice) -> Result<Self, FormProblem> {
        let built = crate::choice::build_form(choice)?;
        Ok(Self::from_form(choice.clone(), &built).expect("a built form's torso is closed"))
    }
}

/// The imaginary arm lines of a form without arms: from [`FORM_ARM_OUT_M`] out of each
/// armhole plate's centre, at the shoulder point's height, down and out at
/// [`FORM_ARM_LEAN_DEG`]. The left (+x) first. None when the form lacks the landmarks.
fn imaginary_arms(built: &BuiltForm) -> Option<[Arm; 2]> {
    let lean = FORM_ARM_LEAN_DEG.to_radians();
    let arm = |suffix: &str| -> Option<Arm> {
        let plate = *built.landmarks.get(&format!("plate_centre{suffix}"))?;
        let shoulder = *built.landmarks.get(&format!("shoulder_point{suffix}"))?;
        let side = plate.x.signum();
        Some(Arm {
            shoulder: DVec3::new(plate.x + side * FORM_ARM_OUT_M, shoulder.y, plate.z),
            direction: DVec3::new(side * lean.sin(), -lean.cos(), 0.0),
            length: FORM_ARM_LENGTH_M,
            free: 0.0,
        })
    };
    Some([arm("")?, arm("_R")?])
}
```

  - `arm_surface_distance` returns `None` when `!self.arm_surfaces`.
  - `signed_distance` uses `self.torso`; `surface_distance` and the arm rays use `self.torso.ray_exit`.
  - `drape_collider(&self) -> &CompoundCollider { &self.collider }`; delete `BodyAndFloor` and the test `a_particle_takes_the_nearest_way_out_of_the_form_and_the_floor` (sim's `CompoundCollider` tests cover the same rules); `the_floor_holds_particles_up_and_the_form_still_counts` keeps working against the compound.
  - New accessors `choice`, `waist_y`, `measured`, `tapes_mesh`, `stand_mesh` with one-line docs.
  - Every `let collider = stage.drape_collider(); … step(Some(&collider))` becomes `step(Some(collider))` (in `build.rs`, `live.rs`, `stage.rs` tests, `sim_runner.rs`, `project_skirt.rs`, `project_tshirt.rs`).
  - `lib.rs`: `pub mod choice;` and `pub use stage::{Arm, Stage, FORM_ARM_LEAN_DEG, FORM_ARM_LENGTH_M, FORM_ARM_OUT_M};` (drop `BodyAndFloor`).

- [ ] **Step 5: Run the drape tests and the workspace**

Run: `cargo nextest run -p opendrape-drape && cargo nextest run --workspace --release > target/t2.log 2>&1; tail -3 target/t2.log`
Expected: all pass (`Stage::shared()` is still MakeHuman; nothing else changed behaviour). If `a_form_has_an_imaginary_arm_line…` fails on the clearance or the height, tune `FORM_ARM_OUT_M` (not the test), and ledger the value.

- [ ] **Step 6: Commit**

```bash
git add crates/drape crates/app/src/sim_runner.rs crates/testkit/tests
git commit -m "feat(drape): a stage built from a dress form, with imaginary arm lines and the form's collider"
```

---

### Task 3: Pieces move out of a bigger form

**Files:**
- Modify: `crates/mesh/src/place.rs` (new `moved_clear`, shared point sampling), `crates/drape/src/stage.rs` (new `Stage::reseat`)

**Interfaces:**
- Consumes: `Stage::for_choice`, `choice::chart_choice` (Task 2); `Project::placement_of`, `Project::set_placement`.
- Produces: `opendrape_mesh::place::{moved_clear, RESEAT_GAP_M, RESEAT_MAX_M}`; `Stage::reseat(&self, project: &mut Project)`.

- [ ] **Step 1: Write the failing tests.** In `place.rs` tests:

```rust
    #[test]
    fn moved_clear_moves_a_curved_piece_out_round_the_same_axis() {
        let shapes = shapes_of(&rect_project(400.0, 300.0)); // reuse the module's helpers
        let p = Placement { position: [0.0, 1.0, 0.15], rotation: Placement::NO_ROTATION, curve: Some(0.15) };
        // A cylinder of radius 0.2 round the y axis: the piece (radius 0.15) is inside it.
        let distance = |q: DVec3| q.x.hypot(q.z) - 0.2;
        let moved = moved_clear(&shapes[0], &p, &distance);
        let r = moved.curve.unwrap();
        assert!((moved.position[2] - r).abs() < 1e-9, "the axis stays at z = 0");
        assert!(r >= 0.2 + RESEAT_GAP_M && r <= 0.2 + PLACE_GAP_M + 0.011, "{r}");
        // Already clear: unchanged.
        assert_eq!(moved_clear(&shapes[0], &moved, &distance), moved);
    }

    #[test]
    fn moved_clear_gives_up_past_its_reach_and_leaves_the_piece() {
        let shapes = shapes_of(&rect_project(400.0, 300.0));
        let p = Placement::at([0.0, 1.0, 0.0]);
        let everywhere = |_: DVec3| -1.0;
        assert_eq!(moved_clear(&shapes[0], &p, &everywhere), p);
    }
```

(Use whatever the test module already has for building a one-rectangle `Shape`; if it has none, build it with `geom::shapes(&project)` after `project.add_piece(Piece::rectangle(…))`.)

In `stage.rs` tests:

```rust
    #[test]
    fn going_up_sizes_moves_placed_pieces_out_of_the_bigger_form() {
        use opendrape_core::{Piece, Point2};
        let small = Stage::for_choice(&FormChoice::default()).unwrap();
        let big = Stage::for_choice(&crate::choice::chart_choice("women-torso", "classic", "US 18").unwrap()).unwrap();
        let mut pr = Project::new();
        let front = pr.add_piece(Piece::rectangle(PieceId(0), "Front", Point2::new(0.0, 0.0), 380.0, 600.0));
        let back = pr.add_piece(Piece::rectangle(PieceId(0), "Back", Point2::new(500.0, 0.0), 380.0, 600.0));
        for (id, at) in [(front, PlaceAt::Front), (back, PlaceAt::Back)] {
            let p = small.place_at(&pr, id, at).unwrap();
            pr.set_placement(id, Some(p));
        }
        let deepest = |stage: &Stage, pr: &Project| -> f64 {
            geom::shapes(pr).iter().flat_map(|s| {
                let p = pr.placement_of(s.id).unwrap();
                let outline = geom::outline_points(&s.piece, 0.5);
                let centre = centre_of(&outline);
                outline.into_iter().map(move |q| stage.signed_distance(place::apply(&p, centre, q))).collect::<Vec<_>>()
            }).fold(f64::MAX, f64::min)
        };
        assert!(deepest(&big, &pr) < 0.0, "the test needs a piece inside the bigger form");
        let before = pr.clone();
        big.reseat(&mut pr);
        assert!(deepest(&big, &pr) >= opendrape_mesh::place::RESEAT_GAP_M - 1e-9);
        // Going back down leaves them where they are: they fall in when draped.
        let mut down = pr.clone();
        small.reseat(&mut down);
        assert_eq!(down, pr);
        assert_ne!(pr, before);
    }
```

(`centre_of(&outline)` is the midpoint of the outline's bounding box, the same centre `place::apply` is used with; write it as a small helper in the test module.)

- [ ] **Step 2: Run, see them fail**

Run: `cargo nextest run -p opendrape-mesh moved_clear && cargo nextest run -p opendrape-drape going_up`
Expected: compile errors.

- [ ] **Step 3: Implement.** In `place.rs`, factor `place_at_arm`'s point sampling into `fn sample_points(shape: &Shape) -> (Point2, Vec<Point2>)` (the bounding-box centre, and the outline plus inside-grid points about a spacing apart), use it in `place_at_arm`, and add:

```rust
/// A piece is moved out of a form when one of its points is closer to it than this (m)...
pub const RESEAT_GAP_M: f64 = 0.005;
/// ...in steps of this (m)...
const RESEAT_STEP_M: f64 = 0.01;
/// ...until all are [`PLACE_GAP_M`] clear, but no further than this (m).
pub const RESEAT_MAX_M: f64 = 0.3;

/// `placement` moved straight out, along the piece's front (`rotation` · +z), by the least
/// multiple of [`RESEAT_STEP_M`] that puts every point of `shape` [`PLACE_GAP_M`] clear of
/// the form (`distance` is its signed distance, negative inside). A curved piece's curve grows
/// by the same amount, so it stays wrapped round the same axis. Unchanged when every point is
/// [`RESEAT_GAP_M`] clear already, or when nothing within [`RESEAT_MAX_M`] clears it.
pub fn moved_clear(shape: &Shape, placement: &Placement, distance: &dyn Fn(DVec3) -> f64) -> Placement {
    let (centre, points) = sample_points(shape);
    let nearest = |p: &Placement| {
        points
            .iter()
            .map(|q| distance(apply(p, centre, *q)))
            .fold(f64::INFINITY, f64::min)
    };
    if nearest(placement) >= RESEAT_GAP_M {
        return *placement;
    }
    let out = rotation(placement) * DVec3::Z;
    let steps = (RESEAT_MAX_M / RESEAT_STEP_M).round() as usize;
    (1..=steps)
        .map(|k| {
            let d = k as f64 * RESEAT_STEP_M;
            let mut moved = *placement;
            moved.position = (position(placement) + out * d).to_array();
            moved.curve = placement.curve.map(|r| r + d);
            moved
        })
        .find(|p| nearest(p) >= PLACE_GAP_M)
        .unwrap_or(*placement)
}
```

In `stage.rs`:

```rust
    /// Every piece and twin with a placement of its own, moved straight out of this form where
    /// it would start inside it or touching it (see `place::moved_clear`). A twin that mirrors
    /// its piece follows the piece. Run after the form changes, as part of the same edit.
    pub fn reseat(&self, project: &mut Project) {
        let moved: Vec<(PieceId, Placement)> = geom::shapes(project)
            .iter()
            .filter_map(|s| {
                let own = own_placement(project, s.id)?;
                let m = place::moved_clear(s, &own, &|q| self.signed_distance(q));
                (m != own).then_some((s.id, m))
            })
            .collect();
        for (id, m) in moved {
            project.set_placement(id, Some(m));
        }
    }
```

`own_placement` reads the piece's `placement` or the twin's own `placement` field (not the mirrored one `placement_of` may report for a twin; check `Project::placement_of`'s doc and use it directly if it already returns only stored placements).

- [ ] **Step 4: Run the tests**

Run: `cargo nextest run -p opendrape-mesh -p opendrape-drape`
Expected: all pass.

- [ ] **Step 5: Commit**

```bash
git add crates/mesh crates/drape
git commit -m "feat(drape): pieces placed round a smaller form move out of a bigger one"
```

---

### Task 4: Everything drapes on the dress form; MakeHuman leaves

**Files:**
- Modify: `crates/drape/src/stage.rs` (`shared()`, delete `makehuman`, `torso_centre`, the cutting arm finder `find_arm`/`cross_sections`/`loop_middle`/`depth_inside` and their tests, adjust the remaining tests), `crates/drape/src/build.rs`, `crates/drape/src/live.rs` (tests only, as needed)
- Modify: `crates/testkit/src/garments.rs`, `crates/testkit/src/lib.rs`, `crates/testkit/examples/drape_bench.rs`, `crates/testkit/tests/forms.rs`, `crates/testkit/tests/project_skirt.rs`, `crates/testkit/tests/project_tshirt.rs`; delete `crates/testkit/tests/drape.rs`
- Modify: `crates/app/tests/placing.rs`, `crates/app/tests/draping.rs` and any other app test that assumed the human body
- Delete: `assets/body/female_average.odb`, `crates/body/src/format.rs`, `crates/body/tests/asset.rs`, `xtask/src/makehuman.rs`, `scripts/fetch-makehuman.sh`
- Modify: `crates/body/src/lib.rs`, `xtask/src/main.rs`, `.github/workflows/ci.yml`, `ASSETS.md`

**Interfaces:**
- Consumes: `Stage::for_choice`, `FormChoice::default()`.
- Produces: `Stage::shared()` = the default form's stage (one per process); `opendrape_testkit::garments::Scene::new(garment)` drapes on the default form; `opendrape_testkit::forms::default_form() -> &'static BuiltForm`.

- [ ] **Step 1: Switch `Stage::shared()`**

```rust
    /// One stage of the default form (women's Classic US 8) for the tests: building the
    /// collider takes a moment. The app holds its own, built for the project's form.
    pub fn shared() -> Arc<Stage> {
        static STAGE: OnceLock<Arc<Stage>> = OnceLock::new();
        STAGE
            .get_or_init(|| {
                Arc::new(Stage::for_choice(&FormChoice::default()).expect("the default form builds"))
            })
            .clone()
    }
```

Delete `makehuman`, `torso_centre`, `find_arm`, `cross_sections`, `loop_middle`, `depth_inside`, `ARM_STEP_M`, `ARM_OUT_M`, `UPPER_ARM_M`, `MIN_ARM_CUTS`, `ARM_LINE_MARGIN_M`; `from_mesh` keeps `arms: None` (a bare mesh has no arm lines). Keep `ARM_RAY_M` if `arm_surface_distance` still uses it. Rewrite `the_form_stands_on_the_floor_round_its_centre_line` for the form (stand on the floor, torso above it, left/right symmetric at the hip, shoulders at the station) and delete the MakeHuman-only arm tests (`the_arms_hang_down…`, `arms_hanging_straight_down…`, `a_form_is_without_arms_unless…`). `a_form_without_arms_has_none…` stays (a bare `from_mesh` torso). Fix `sleeves_from_ten_to_sixty_centimetres_long_are_curved_round_the_upper_arm`: on imaginary arms every sleeve takes `ARM_FALLBACK_RADIUS_M`; assert that instead, and that every sleeve starts clear of the form.

- [ ] **Step 2: Run the drape tests; fix what moved**

Run: `cargo nextest run -p opendrape-drape --release > target/t4.log 2>&1; grep -E "FAIL|Summary" target/t4.log`
Expected: failures only where a test leaned on the human body's shape (heights, distances). Fix each test's expectation to the form's real numbers, never weaken what it checks; ledger each change in one line.

- [ ] **Step 3: The garment gates on the form.**
  - `project_skirt.rs`: it already drapes through `Stage::shared()`; run it, fix heights that assumed the human body.
  - `project_tshirt.rs`: rename `a_drafted_t_shirt_drapes_with_its_sleeves_on_the_arms` to `a_drafted_t_shirt_drapes_on_the_dress_form_with_its_sleeves_hanging`. Keep: no NaN, nothing to tell the student, welded shut, cap notch near the shoulder seam, settles, nothing inside the form (signed distance ≥ −2 mm everywhere). Replace "within 12 cm of the arm's line" with: every sleeve particle is below its armhole's top (`y < stage.shoulder_y() + 0.02`) and on its own side (`x · side > 0`). `the_sleeves_start_round_the_arms_clear_of_the_body` becomes `the_sleeves_start_round_the_arm_lines_clear_of_the_form` (every starting point outside the form; near its arm line within `ARM_FALLBACK_RADIUS_M + 0.03`).
  - `garments.rs`: delete `body()`, `collider()`, `torso_axis_z`, `Contact::Bundled`, and the MakeHuman skirt/bodice builders; `Scene::new(g)` becomes `Scene::new_on(g, crate::forms::default_form())`. In `forms.rs` add:

```rust
/// The default form (women's Classic US 8), built once.
pub fn default_form() -> &'static BuiltForm {
    static FORM: std::sync::OnceLock<BuiltForm> = std::sync::OnceLock::new();
    FORM.get_or_init(|| {
        let form = opendrape_body::form::Form::bundled("women-torso").expect("bundled");
        let chart = opendrape_body::form::Chart::for_form("women-torso").remove(0);
        form.build(&chart.size("US 8").expect("US 8").mm, opendrape_body::form::Quality::Standard)
            .expect("US 8 builds")
    })
}
```

  - Delete `tests/drape.rs` (its skirt, tube and determinism gates are covered on the forms by `tests/forms.rs`); move `welded_skirt_is_consistently_wound` into `tests/forms.rs` using `Scene::new(Garment::Skirt)`.
  - `examples/drape_bench.rs`: use `Scene::new` and `scene.collider()` instead of the removed `collider()`.

- [ ] **Step 4: The app tests.** Run `cargo nextest run -p opendrape --release > target/t4app.log 2>&1; grep -E "FAIL|Summary" target/t4app.log`. `placing.rs` uses `Stage::shared().arms()` for arm lines: they are the imaginary lines now; fix any assertion that measured the human arm's surface. Fix other failures the same way as in Step 2.

- [ ] **Step 5: Retire MakeHuman.**
  - `git rm assets/body/female_average.odb crates/body/src/format.rs crates/body/tests/asset.rs xtask/src/makehuman.rs scripts/fetch-makehuman.sh`
  - `crates/body/src/lib.rs`: drop `mod format`, its `pub use`, and `BodyMesh::female_average`; the crate doc says "dress forms … and tape-measure measurements" (no MakeHuman).
  - `xtask/src/main.rs`: drop `mod makehuman`, the `body` command and its doc lines.
  - `.github/workflows/ci.yml`: drop the three MakeHuman lines (fetch, `xtask body`, `cmp`) and the step's name if the step is now empty.
  - `ASSETS.md`: drop the `female_average.odb` row.

- [ ] **Step 6: The whole suite, clippy and fmt**

Run: `cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings && cargo nextest run --workspace --release > target/t4all.log 2>&1; tail -3 target/t4all.log`
Expected: all pass, no warnings. `git grep -n -i "makehuman\|female_average\|\.odb"` lists only docs under `docs/` (history).

- [ ] **Step 7: Commit**

```bash
git add -A crates xtask scripts .github ASSETS.md assets
git commit -m "feat: everything drapes on the dress form; the MakeHuman body is retired"
```

---

### Task 5: The app follows the project's form

**Files:**
- Modify: `crates/app/src/app.rs` (stage sync, open check), `crates/app/src/sim_runner.rs` (`set_stage`), `crates/app/src/viewport.rs` (torso, tapes, stand; `set_stage`; `set_show_tapes`), `crates/app/src/view_settings.rs` (`show_tapes`), `crates/render/src/studio/look.rs` (`TAPE_SRGB`, `STAND_SRGB`), `crates/app/src/theme.rs` (re-exports), `crates/app/i18n/en-US/opendrape.ftl` (armhole labels, open error)
- Test: `crates/app/tests/forms.rs` (new)

**Interfaces:**
- Consumes: `Stage::for_choice`, `Stage::choice`, `Stage::waist_y`, `Stage::tapes_mesh`, `Stage::stand_mesh`, `Stage::reseat` (Tasks 2–3).
- Produces: `SimRunner::set_stage(&self, stage: Arc<Stage>)`; `Viewport::set_stage(&mut self, rs, stage: &Stage)`; `Viewport::set_show_tapes(&mut self, on: bool)`; `ViewSettings.show_tapes: bool` (default true); `OpenDrapeApp::stage()` always matches `project().form` after a frame; `OpenDrapeApp::apply_form(&mut self, choice: FormChoice) -> Result<(), FormProblem>` (builds, then one undoable edit setting `form` and reseating, then the next frame swaps the stage).

- [ ] **Step 1: Write the failing tests** in `crates/app/tests/forms.rs` (copy the harness setup from `tests/workspaces.rs`: `harness`, `MAX_STEPS`):

```rust
#[test]
fn a_form_change_swaps_the_stage_and_undo_brings_the_old_one_back() {
    let dir = tempdir();
    let mut h = harness(dir.path());
    let men = opendrape_drape::choice::base_choice("men-torso").unwrap();
    h.state_mut().apply_form(men.clone()).unwrap();
    h.run();
    assert_eq!(h.state().stage().choice(), Some(&men));
    assert!(h.state().editor().doc.is_dirty());
    cmd(&mut h, Key::Z);
    assert_eq!(h.state().stage().choice(), Some(&FormChoice::default()));
    cmd_shift(&mut h, Key::Z);
    assert_eq!(h.state().stage().choice(), Some(&men));
}

#[test]
fn a_form_change_while_draping_restarts_the_drape_on_the_new_form() {
    let dir = tempdir();
    let mut h = harness(dir.path());
    add_piece(&mut h);
    h.state_mut().play();
    h.run();
    assert!(h.state().is_draping());
    let us14 = opendrape_drape::choice::chart_choice("women-torso", "classic", "US 14").unwrap();
    h.state_mut().apply_form(us14.clone()).unwrap();
    h.run();
    assert!(!h.state().is_draping(), "the drape restarts, like Reset");
    assert_eq!(h.state().stage().choice(), Some(&us14));
}

#[test]
fn a_file_whose_form_cannot_be_built_is_refused_and_the_open_project_stays() {
    let dir = tempdir();
    let mut p = Project::new();
    p.form.id = "child-torso".into();
    let file = dir.path().join("child.odp");
    opendrape_io::save(&p, &file).unwrap();
    let mut h = harness_opening(dir.path(), file); // a harness whose file dialog answers `file`
    h.state_mut().open(); // File → Open
    h.run();
    assert!(h.query_by_label_contains("child-torso").is_some(), "the error names the form");
    assert_eq!(h.state().editor().doc.project().form, FormChoice::default());
}
```

(Use the names the app really has for Play/Open/editor access: look at `tests/workspaces.rs`, `tests/draping.rs` and `tests/ui.rs` for `play`, the file-dialog stub that answers a path, and `query_by_label_contains`; adapt, don't invent. `tempdir()` is whatever those files use.)

Also in `view_settings.rs` tests: a `view.json` without `show_tapes` loads with `show_tapes: true`.

- [ ] **Step 2: Run, see them fail**

Run: `cargo nextest run -p opendrape --test forms`
Expected: compile errors (`apply_form` missing).

- [ ] **Step 3: Implement.**
  - `look.rs`: `pub const TAPE_SRGB: [u8; 3] = [72, 58, 50];` (dark tape) and `pub const STAND_SRGB: [u8; 3] = [64, 64, 68];` (charcoal), each with a one-line doc; re-export in `theme.rs` beside `FORM_SRGB`.
  - `ViewSettings`: `#[serde(default = "yes")] pub show_tapes: bool` with `fn yes() -> bool { true }`, and `Default` sets it.
  - `Viewport`: `body: StudioMesh` becomes `form: Vec<(Part, StudioMesh)>` with `enum Part { Torso, Tapes, Stand }`, built by `fn form_meshes(renderer, rs, stage) -> Vec<(Part, StudioMesh)>` (skips empty meshes; torso `FORM_SRGB`, tapes `TAPE_SRGB`, stand `STAND_SRGB`, all `Material::Form`). `Viewport::new` takes `show_tapes` from `settings`; `set_stage` rebuilds the meshes and moves `camera.target.y` to `stage.waist_y()`; `ui()` draws every part except `Tapes` when `show_tapes` is off. In `new`, the camera's target y is `stage.waist_y()` (was 0.95).
  - `SimRunner`: `Command::Stage(Arc<Stage>)`; `run` holds `let mut stage: Arc<Stage>` (was `&Stage`), and on `Command::Stage(s)` drops the drape, clears `latest`, sets `stage = s`, calls `on_frame()`. `pub fn set_stage(&self, stage: Arc<Stage>) { self.reset(); let _ = self.tx.send(Command::Stage(stage)); }`. A unit test in its test module: start, play two panels, `set_stage(another)`, and the next frame (if any) comes from a drape made after the swap (`latest()` is None right after, `is_draping()` false).
  - `app.rs`:
    - fields `next_stage: Option<Arc<Stage>>` (built ahead, for the next sync) and `unbuilt: Option<FormChoice>` (a choice that failed to build, so it isn't retried every frame);
    - `fn sync_stage(&mut self)`, run first thing in `ui()` (before `update_if_edited` can send the form to a running drape):

```rust
    /// The stage follows the project's form: after a form change, an undo or redo of one, or
    /// opening a file. The drape restarts, held drags end, and the 3D view redraws the form.
    fn sync_stage(&mut self) {
        let wanted = &self.editor.doc.project().form;
        if self.stage.choice() == Some(wanted) || self.unbuilt.as_ref() == Some(wanted) {
            return;
        }
        let stage = match self.next_stage.take().filter(|s| s.choice() == Some(wanted)) {
            Some(s) => s,
            None => match Stage::for_choice(wanted) {
                Ok(s) => Arc::new(s),
                Err(e) => {
                    crate::startup_log::stage(format_args!("form: keeping the old one: {e}"));
                    self.unbuilt = Some(wanted.clone());
                    return;
                }
            },
        };
        self.unbuilt = None;
        self.stop_pulling();
        self.stop_arranging();
        if let Some(runner) = &self.runner {
            runner.set_stage(stage.clone());
        }
        self.draped = None;
        self.arranged = SceneCache::default();
        if self.editor.stage.is_some() {
            self.editor.stage = Some(stage.clone());
        }
        if let (Some(viewport), Some(rs)) = (self.viewport.as_mut(), self.render_state.as_ref()) {
            viewport.set_stage(rs, &stage);
        }
        self.stage = stage;
    }
```

      (If the app keeps no `render_state` handle, pass `frame.wgpu_render_state()` in: `sync_stage(&mut self, frame: &eframe::Frame)`.)
    - `pub fn apply_form(&mut self, choice: FormChoice) -> Result<(), FormProblem>`: `let stage = Stage::for_choice(&choice)?;` then `self.editor.doc.edit(|p| { p.form = choice; stage.reseat(p); });` and `self.next_stage = Some(Arc::new(stage));`.
    - `new()`: build the stage from `FormChoice::default()` via `Stage::shared()` (the default form, shared) — the sync swaps it when a project with another form opens.
    - Open (`DialogFor::Open`): `Ok(project)` → `match Stage::for_choice(&project.form) { Ok(s) => { self.next_stage = Some(Arc::new(s)); self.replace_project(…) } Err(e) => self.error = Some(tr!("error-open", error = e.to_string())) }`.
  - `opendrape.ftl`: `place-left-arm = Place at left armhole`, `place-right-arm = Place at right armhole`, `notice-no-arms = This form has no armholes to place a sleeve at.` Fix any test that finds the old labels.

- [ ] **Step 4: Run the app tests**

Run: `cargo nextest run -p opendrape --release > target/t5.log 2>&1; grep -E "FAIL|Summary" target/t5.log`
Expected: all pass.

- [ ] **Step 5: Commit**

```bash
git add crates/app crates/render
git commit -m "feat(app): the 3D view shows the project's dress form, with its tapes and stand, and follows every form change"
```

---

### Task 6: The Assets section and the 3D | 2D switch

**Files:**
- Create: `crates/app/src/assets.rs` (the section's frame: heading, ✕; the form panel comes in Task 7)
- Modify: `crates/app/src/app.rs` (menu button, layout, closing rules, area switch), `crates/app/src/lib.rs` (`pub mod assets;`), `crates/app/i18n/en-US/opendrape.ftl`
- Test: `crates/app/tests/assets.rs` (new)

**Interfaces:**
- Consumes: `Workspace`, `PatternEditor::ui_with_keys`, `view_3d`.
- Produces: `OpenDrapeApp::assets_open() -> bool`, `OpenDrapeApp::left_shows_pattern() -> bool`; Fluent ids `menu-assets`, `assets-close`, `assets-dress-forms`, `area-3d`, `area-2d`.

- [ ] **Step 1: Write the failing tests** in `crates/app/tests/assets.rs` (harness copied from `tests/workspaces.rs`):

```rust
#[test]
fn assets_opens_from_the_menu_row_and_closes_three_ways() {
    let dir = tempdir();
    let mut h = harness(dir.path());
    h.get_by_label("Assets").click();
    h.run();
    assert!(h.state().assets_open());
    assert!(h.query_by_label("Dress forms").is_some());
    h.get_by_label("Assets").click(); // again: closes
    h.run();
    assert!(!h.state().assets_open());
    h.get_by_label("Assets").click();
    h.run();
    h.get_by_label("Close assets").click();
    h.run();
    assert!(!h.state().assets_open());
    h.get_by_label("Assets").click();
    h.run();
    h.get_by_label("Texturing").click();
    h.run();
    assert!(!h.state().assets_open(), "choosing a tab shows that stage");
}

#[test]
fn the_area_switch_shows_only_while_assets_is_open_in_modeling() {
    let dir = tempdir();
    let mut h = harness(dir.path());
    assert!(h.query_by_label("2D").is_none());
    h.get_by_label("Assets").click();
    h.run();
    assert!(h.query_by_label("2D").is_some());
    cmd(&mut h, Key::Num3); // Texturing: closes Assets
    cmd(&mut h, Key::Num1);
    h.get_by_label("Assets").click();
    h.run();
    h.get_by_label("2D").click();
    h.run();
    assert!(h.state().left_shows_pattern());
    assert!(h.query_by_label("Pen (H)").is_some(), "the pattern tools are on the left");
    h.key_press(Key::H);
    h.run();
    assert_eq!(h.state().editor().tool, Tool::Pen);
    h.get_by_label("Assets").click(); // closing puts the 3D view back
    h.run();
    assert!(!h.state().left_shows_pattern());
}

#[test]
fn a_tiny_window_with_assets_open_does_not_crash() {
    let dir = tempdir();
    let mut h = harness_sized(dir.path(), egui::vec2(320.0, 240.0));
    h.state_mut().set_assets_open(true);
    h.run();
    h.state_mut().set_left_shows_pattern(true);
    h.run();
}
```

- [ ] **Step 2: Run, see them fail**

Run: `cargo nextest run -p opendrape --test assets`
Expected: compile errors.

- [ ] **Step 3: Implement.**
  - Fields on `OpenDrapeApp`: `assets_open: bool`, `left_2d: bool` (screen state: never saved or undone). Public getters and `set_assets_open` / `set_left_shows_pattern` (the latter only has effect while Assets is open in Modeling).
  - `menu_bar`: after the View menu, `let assets = ui.add(egui::Button::selectable(self.assets_open, tr!("menu-assets")));` returning a toggle request; like the tab pick, the toggle is applied after the central panel has run. A workspace pick (tab, View menu or Cmd+1…5) closes Assets and sets `left_2d = false`.
  - In `ui()`: the left panel shows, while Assets is open in Modeling, a slim `egui::Panel::top("area_switch")` with two `Button::selectable`s (`area-3d` "3D", `area-2d` "2D"); below it `self.editor.ui_with_keys(ui, keys_for_pattern)` when `left_2d`, else `self.view_3d(ui, frame)`. The central panel shows `assets::ui(...)` while Assets is open; otherwise the workspace as now. While Assets is open and the editor isn't drawn, `self.app_undo_redo(&ctx)` handles Cmd+Z (as the other tabs do).
  - `assets.rs`: `pub fn header(ui) -> bool` draws the "Dress forms" heading with a ✕ `icon_button` (`assets-close`, "Close assets") right-aligned and returns whether it was clicked. Task 7 adds the form panel under it.
  - `opendrape.ftl`: `menu-assets = Assets`, `assets-close = Close assets`, `assets-dress-forms = Dress forms`, `area-3d = 3D`, `area-2d = 2D`.

- [ ] **Step 4: Run the app tests**

Run: `cargo nextest run -p opendrape --release > target/t6.log 2>&1; grep -E "FAIL|Summary" target/t6.log`
Expected: all pass.

- [ ] **Step 5: Commit**

```bash
git add crates/app
git commit -m "feat(app): an Assets section after View, with a 3D | 2D switch on the left area"
```

---

### Task 7: Picking the dress form, chart, size and custom measurements

**Files:**
- Modify: `crates/app/Cargo.toml` (`opendrape-body.workspace = true`), `crates/app/src/assets.rs`, `crates/app/src/app.rs`, `crates/app/src/viewport.rs` (thumbnails), `crates/app/i18n/en-US/opendrape.ftl`
- Test: `crates/app/tests/assets.rs`

**Interfaces:**
- Consumes: `choice::{charts, chart_choice, base_choice, nearest_in, with_measurement, girth_name}`, `OpenDrapeApp::apply_form`, `Stage::measured`, `Form::IDS`, `Form::bundled(..).inputs()`, `Units::{format, parse, to_mm}`.
- Produces: `Viewport::thumbnail(&mut self, rs, stage: &Stage) -> egui::TextureId`; the form panel.

- [ ] **Step 1: Write the failing tests** (append to `tests/assets.rs`):

```rust
fn open_assets(h: &mut App) {
    h.get_by_label("Assets").click();
    h.run();
}

#[test]
fn clicking_the_mens_card_switches_form_and_undo_switches_back() {
    let dir = tempdir();
    let mut h = harness(dir.path());
    open_assets(&mut h);
    h.get_by_label("Men's torso").click();
    h.run();
    assert_eq!(h.state().editor().doc.project().form.id, "men-torso");
    assert!(h.state().editor().doc.is_dirty());
    cmd(&mut h, Key::Z);
    assert_eq!(h.state().editor().doc.project().form, FormChoice::default());
}

#[test]
fn the_chart_and_size_lists_change_the_form() {
    let dir = tempdir();
    let mut h = harness(dir.path());
    open_assets(&mut h);
    h.get_by_label("Chart").click();
    h.run();
    h.get_by_label("Everyday body").click();
    h.run();
    let f = h.state().editor().doc.project().form.clone();
    assert!(matches!(f.size, FormSize::Chart { ref chart, .. } if chart == "everyday"));
    h.get_by_label("Size").click();
    h.run();
    h.get_by_label_contains("US 14").click();
    h.run();
    let f = h.state().editor().doc.project().form.clone();
    assert!(matches!(f.size, FormSize::Chart { ref label, .. } if label == "US 14"));
    let bust = h.state().stage().measured()["bust"];
    assert!((bust - f.measurements["bust"]).abs() < 1.0, "the form is built at the size: {bust}");
}

#[test]
fn a_custom_waist_applies_on_enter_and_an_impossible_one_is_refused() {
    let dir = tempdir();
    let mut h = harness(dir.path());
    open_assets(&mut h);
    h.get_by_label("Custom measurements").click();
    h.run();
    let waist = h.get_by_label("Waist");
    waist.click();
    h.run();
    h.get_by_label("Waist").type_text("\u{8}\u{8}\u{8}\u{8}\u{8}\u{8}72");
    h.key_press(Key::Enter);
    h.run();
    let f = h.state().editor().doc.project().form.clone();
    assert_eq!(f.size, FormSize::Custom);
    assert!((f.measurements["waist"] - 720.0).abs() < 1e-9);
    assert_eq!(h.state().editor().tool, Tool::Select, "typing 7 or 2 picked no tool");
    h.get_by_label("Waist").click();
    h.run();
    h.get_by_label("Waist").type_text("\u{8}\u{8}\u{8}\u{8}\u{8}\u{8}200");
    h.key_press(Key::Enter);
    h.run();
    assert!(h.query_by_label_contains("Waist can be 50").is_some());
    assert!((h.state().editor().doc.project().form.measurements["waist"] - 720.0).abs() < 1e-9);
}

#[test]
fn going_up_to_the_largest_size_moves_placed_pieces_out_of_the_form() {
    let dir = tempdir();
    let mut h = harness(dir.path());
    let id = add_piece(&mut h);
    h.state_mut().place_at(id, PlaceAt::Front); // the app's Place at… action
    h.run();
    let largest = opendrape_drape::choice::chart_choice("women-torso", "classic", "US 18").unwrap();
    h.state_mut().apply_form(largest).unwrap();
    h.run();
    let stage = h.state().stage().clone();
    let project = h.state().editor().doc.project().clone();
    let p = project.placement_of(id).unwrap();
    for q in sampled_points(&project, id) {
        assert!(stage.signed_distance(opendrape_mesh::place::apply(&p, q.0, q.1)) > 0.0);
    }
}

#[test]
fn show_tape_lines_is_remembered_with_the_view_settings() {
    let dir = tempdir();
    let mut h = harness(dir.path());
    open_assets(&mut h);
    h.get_by_label("Show tape lines").click();
    h.run();
    let saved = ViewSettings::load(Some(dir.path()));
    assert!(!saved.show_tapes);
}
```

(Adapt the helper names to what the tests already use; `sampled_points` returns `(centre, point)` pairs like the drape tests' helper. Use whichever app call `tests/placing.rs` uses for Place at.)

- [ ] **Step 2: Run, see them fail**

Run: `cargo nextest run -p opendrape --test assets`
Expected: FAIL (no cards, lists or fields yet).

- [ ] **Step 3: Implement the form panel** in `assets.rs`, called from the central panel under the header, inside a vertical `ScrollArea` (so a small window scrolls instead of clipping):
  - **State** `FormsPanel { fields: BTreeMap<String, String>, fields_for: Option<FormChoice>, message: Option<String>, thumbs: BTreeMap<String, egui::TextureId> }`, held on the app.
  - **Cards:** for each `Form::IDS` id, a selectable card (`egui::Frame` + `Sense::click`, highlighted when it's the project's form) with the thumbnail (`thumbs[id]` if present, else a large `egui_phosphor::regular::DRESS` for women / `T_SHIRT` for men), the name (`form_name(id)`) and the suits line (`form_suits(id)`); the whole card is labelled with the name for AccessKit. A click on the other form calls `apply_form(base_choice(id))`.
  - **Thumbnails:** the first time the panel shows with a 3D view, for each id: `Stage::for_choice(&base_choice(id))` and `viewport.thumbnail(rs, &stage)`; stored in `thumbs`.
  - **Chart:** `egui::ComboBox::from_label(tr!("form-chart"))`, entries `chart_name(kind)` for `charts(id)`; selected text: the choice's chart, or `form-custom` when Custom. Picking a kind applies `nearest_in(choice, kind)`.
  - **Size:** `ComboBox::from_label(tr!("form-size"))`; entries for the current chart (the choice's chart, Classic when Custom): `size_text(row)` = `"{label} / {alt} · {girth} {value}"` (or without `/ {alt}`), `girth` = `measure_name(girth_name)` lowercased by the Fluent string itself (`form-size-entry`), value in project units via `Units::format`. Selected text: the current row's, or `form-custom`. Picking applies `chart_choice`.
  - **Custom measurements:** `egui::CollapsingHeader::new(tr!("form-custom-heading"))`. One row per `Form::bundled(id).inputs()`: `measure_name(name)` label and a `TextEdit::singleline` (labelled with the name for AccessKit, `desired_width` 70) holding the value in project units, refreshed from the choice whenever `fields_for != Some(choice)` and no field has focus; the unit suffix after it. On Enter (`response.lost_focus() && i.key_pressed(Key::Enter)`): `Units::parse` → `units.to_mm` → if outside the input's range, `message = tr!("form-out-of-range", name = measure_name(name), min = units.format(lo), max = units.format(hi))` and the field resets; else `apply_form(with_measurement(choice, name, mm))`, and a `FormProblem` from it becomes the message. Below: read-only "measured" lines for `front_waist_length`, `apex_to_apex` (if present) and `high_hip` from `stage.measured()`.
  - **Show tape lines:** a checkbox bound to `view_settings.show_tapes`; on change, save the settings and call `viewport.set_show_tapes`.
  - **Keys:** the pattern editor's shortcuts already stay off while a text field has focus (check `keys_for_pattern` / `wants_keyboard_input`); if the test shows a tool picked, gate the editor's keys on `!ctx.wants_keyboard_input()`.
  - `Viewport::thumbnail`: a 200×260 `RenderTarget`, registered as an egui texture and kept in `self.thumbs: Vec<RenderTarget>`. A fixed camera frames the torso from the front three-quarter: target `(0, waist_y, 0)`, yaw 0.5, pitch 0.12, distance 2.2. It draws the form meshes for `stage`, with `set_moving(false)`, up to 64 times until `still_done`. The view's own next frame redraws from scratch (its frame signature changes).
  - Fluent:

```
form-women-torso = Women's torso
form-men-torso = Men's torso
form-women-torso-suits = For dresses, tops, blouses, skirts and kurtas
form-men-torso-suits = For shirts, kurtas, waistcoats and jackets
form-chart = Chart
chart-classic = Classic form
chart-everyday = Everyday body
form-size = Size
form-size-entry = { $size } · { $girth } { $value }
form-custom = Custom
form-custom-heading = Custom measurements
form-out-of-range = { $name } can be { $min }–{ $max } on this form
form-measured = Measured
form-show-tapes = Show tape lines
measure-bust = Bust
measure-chest = Chest
measure-under_bust = Under-bust
measure-waist = Waist
measure-hip = Hip
measure-high_hip = High hip
measure-neck = Neck
measure-shoulder_length = Shoulder length
measure-back_waist_length = Back waist length
measure-waist_to_hip = Waist to hip
measure-front_waist_length = Front waist length
measure-apex_to_apex = Apex to apex
```

    with `fn form_name(id)`, `fn form_suits(id)`, `fn chart_name(kind)` and `fn measure_name(name)` as `match`es over these ids (the `tr!` macro needs literal ids); an unknown id shows itself.

- [ ] **Step 4: Run the app tests, clippy**

Run: `cargo clippy -p opendrape --all-targets -- -D warnings && cargo nextest run -p opendrape --release > target/t7.log 2>&1; grep -E "FAIL|Summary" target/t7.log`
Expected: all pass.

- [ ] **Step 5: Commit**

```bash
git add crates/app
git commit -m "feat(app): pick the dress form, its chart and size, or type custom measurements, in Assets"
```

---

### Task 8: Docs, checklist and memory

**Files:**
- Create: `docs/testing/M5c-checklist.md`
- Modify: `docs/specs/2026-10-09-opendrape-design.md` (avatar rows → dress forms, `crates/body` role, MakeHuman retired, status line), `README.md` (status), `docs/superpowers/specs/2026-10-11-m5c-dress-forms-and-assets-design.md` (status: built), memory `opendrape-project.md`

- [ ] **Step 1: Write the checklist** in plain language, like `docs/testing/M5b-checklist.md`:
  1. The 3D view shows the dress form on its stand (beige torso, dark tape lines, charcoal stand); no human.
  2. Assets (after View) opens the list in the right-hand area; ✕, a second click or a tab closes it.
  3. The 3D | 2D switch shows the pattern on the left; draw with the Pen there.
  4. Click Men's torso: the form changes; Cmd+Z brings the women's form back.
  5. Chart → Everyday body, Size → US 14: the form grows.
  6. Custom measurements → Waist 72 → Enter: the waist changes; Waist 200 → a message, nothing changes.
  7. Show tape lines off and on; it is remembered after a restart.
  8. Place a skirt's pieces (Place at → Front/Back), switch to US 18: nothing starts inside the form; Play drapes it.
  9. T-shirt: Place at → Left armhole puts the sleeve beside the armhole; it hangs when draped.
  10. Save, reopen: the same form and size; an older project opens on Women's US 8.
- [ ] **Step 2: Update the design doc, README, the M5c spec status and the memory note.**
- [ ] **Step 3: Full check**

Run: `cargo fmt --all --check && cargo clippy --workspace --all-targets -- -D warnings && cargo nextest run --workspace --release > target/t8.log 2>&1; tail -3 target/t8.log && cargo deny check`
Expected: all pass.

- [ ] **Step 4: Commit**

```bash
git add docs README.md
git commit -m "docs: M5c checklist; the design doc and README say dress forms, not MakeHuman"
```
