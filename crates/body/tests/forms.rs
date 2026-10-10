//! Whole-library checks: every bundled form is built at every size of every bundled chart.

use opendrape_body::form::{Chart, Form, Measurements, Quality, SizeError};
use opendrape_body::{BodyMesh, boundary_edge_count, girth_at};
use std::collections::{BTreeSet, HashMap, HashSet};

#[test]
fn every_bundled_form_builds_closed_symmetric_and_in_budget() {
    for id in Form::IDS {
        let form = Form::bundled(id).expect(id);
        assert_eq!(form.file().id, id, "the registry maps {id} to its own file");
        let built = form
            .build(
                &form.base_measurements(Quality::Standard),
                Quality::Standard,
            )
            .expect("its own size fits");
        let t = built.torso.triangles.len();
        assert!((10_000..=30_000).contains(&t), "{id}: {t} triangles");
        assert_eq!(boundary_edge_count(&built.torso), 0, "{id} torso is closed");
        assert_eq!(boundary_edge_count(&built.stand), 0, "{id} stand is closed");
        assert!(!built.tapes.triangles.is_empty(), "{id} has tape ribbons");
        let key = |x: f32, y: f32, z: f32| {
            (
                (x * 1e5).round() as i64,
                (y * 1e5).round() as i64,
                (z * 1e5).round() as i64,
            )
        };
        let set: HashSet<_> = built
            .torso
            .positions
            .iter()
            .map(|p| key(p.x, p.y, p.z))
            .collect();
        assert!(
            built
                .torso
                .positions
                .iter()
                .all(|p| set.contains(&key(-p.x, p.y, p.z))),
            "{id} is mirror-symmetric"
        );
        let waist = built.stations["waist"];
        assert!((1.01..1.05).contains(&waist), "{id} waist at {waist} m");
        let lowest = built
            .stand
            .positions
            .iter()
            .map(|p| p.y)
            .fold(f32::MAX, f32::min);
        assert_eq!(lowest, 0.0, "{id} stands on the floor");
    }
    assert!(Form::bundled("men-trousers").is_none(), "no such form");
    assert!(Form::bundled("nonsense").is_none());
    assert!(Form::bundled("").is_none());
}

/// Girths sliced from the mesh (like a tape measure) within 1 mm, lengths within 2 mm.
fn check_fit(form: &Form, size: &Measurements, quality: Quality, what: &str) {
    let built = form
        .build(size, quality)
        .unwrap_or_else(|e| panic!("{what}: {e}"));
    let f = form.file();
    for m in &f.inputs {
        let (got, tol) = if f.stations.contains_key(m) {
            let slice = girth_at(&built.torso, built.stations[m] as f32, 10.0);
            (1000.0 * f64::from(slice), 1.0)
        } else {
            (built.measured[m], 2.0)
        };
        assert!(
            (got - size[m]).abs() <= tol,
            "{what} {m}: {got:.1} mm, chart says {}",
            size[m]
        );
    }
}

#[test]
fn every_chart_row_fits_its_form() {
    for chart in Chart::bundled() {
        let form = Form::bundled(&chart.form).expect("the chart's form is bundled");
        for size in &chart.sizes {
            check_fit(
                &form,
                &size.mm,
                Quality::Standard,
                &format!("{} {}", chart.id, size.label),
            );
        }
    }
}

#[test]
fn low_quality_still_fits_the_charts() {
    for chart in Chart::bundled() {
        let form = Form::bundled(&chart.form).unwrap();
        for size in [&chart.sizes[0], chart.sizes.last().unwrap()] {
            check_fit(
                &form,
                &size.mm,
                Quality::Low,
                &format!("{} {} (low)", chart.id, size.label),
            );
        }
    }
}

#[test]
fn charts_cover_their_forms() {
    let charts = Chart::bundled();
    assert_eq!(charts.len(), 4);
    for c in &charts {
        let form = Form::bundled(&c.form).unwrap();
        let first = form.file().inputs[0].clone();
        let inputs: BTreeSet<String> = form.inputs().into_iter().map(|(m, _)| m).collect();
        let mut labels = HashSet::new();
        for s in &c.sizes {
            assert!(labels.insert(&s.label), "{} repeats {}", c.id, s.label);
            // Every row carries exactly the form's inputs: none missing, none left over.
            let row: BTreeSet<String> = s.mm.keys().cloned().collect();
            assert_eq!(row, inputs, "{} {}", c.id, s.label);
        }
        assert!(
            c.sizes
                .windows(2)
                .all(|w| w[0].mm[&first] < w[1].mm[&first]),
            "{} grows size by size",
            c.id
        );
        assert!(c.size(&c.sizes[3].label).is_some());
        assert!(c.size("no such size").is_none());
    }
    assert_eq!(Chart::for_form("women-torso").len(), 2);
    assert_eq!(Chart::for_form("men-torso").len(), 2);
    assert!(Chart::for_form("nonsense").is_empty());
}

#[test]
fn each_forms_base_size_is_a_size_in_every_one_of_its_charts() {
    for (id, base_size) in [("women-torso", "US 8"), ("men-torso", "40")] {
        let form = Form::bundled(id).unwrap();
        assert_eq!(form.file().base_size, base_size);
        let charts = Chart::for_form(id);
        assert!(!charts.is_empty(), "{id} has no chart");
        for c in charts {
            assert!(
                c.size(&form.file().base_size).is_some(),
                "{} has no size {base_size}",
                c.id
            );
        }
    }
    // Every form of the library is covered by the list above.
    assert_eq!(Form::IDS, ["women-torso", "men-torso"]);
}

#[test]
fn refusals_name_the_measurement_and_its_range() {
    let form = Form::bundled("women-torso").unwrap();
    let mut size = Chart::for_form("women-torso")[0].sizes[4].mm.clone();
    size.insert("waist".into(), 400.0);
    let SizeError {
        measurement,
        min_mm,
        max_mm,
    } = form.build(&size, Quality::Standard).unwrap_err();
    assert_eq!(measurement, "waist");
    // 400 mm is outside what the form takes. The range named is what it can really reach, which
    // may be narrower than the file's, but never wider.
    assert!(!(min_mm..=max_mm).contains(&400.0), "{min_mm}–{max_mm}");
    let [lo, hi] = form.file().ranges["waist"];
    assert!(
        lo <= min_mm && min_mm < max_mm && max_mm <= hi,
        "{min_mm}–{max_mm} lies outside the file's {lo}–{hi}"
    );
}

/// Every edge a→b of every triangle is used once, and so is b→a: the mesh is closed and its
/// triangles agree on which way is out. Its signed volume is then positive when that way is out.
fn assert_wound_outward(what: &str, mesh: &BodyMesh) {
    let mut directed = HashMap::<(u32, u32), u32>::new();
    for t in &mesh.triangles {
        for k in 0..3 {
            *directed.entry((t[k], t[(k + 1) % 3])).or_default() += 1;
        }
    }
    for (&(a, b), &n) in &directed {
        assert_eq!(n, 1, "{what}: edge {a}→{b} is used {n} times");
        assert_eq!(
            directed.get(&(b, a)),
            Some(&1),
            "{what}: edge {a}→{b} has no single reverse"
        );
    }
    let volume: f64 = mesh
        .triangles
        .iter()
        .map(|t| {
            let [a, b, c] = t.map(|i| mesh.positions[i as usize].as_dvec3());
            a.dot(b.cross(c)) / 6.0
        })
        .sum();
    assert!(
        volume > 0.0,
        "{what}: signed volume {volume} m³ faces inward"
    );
}

#[test]
fn every_triangle_of_every_form_is_wound_outward_at_every_chart_end() {
    for id in Form::IDS {
        let form = Form::bundled(id).unwrap();
        for quality in [Quality::Standard, Quality::Low] {
            let mut sizes = vec![("own size".to_string(), form.base_measurements(quality))];
            for chart in Chart::for_form(id) {
                for size in [&chart.sizes[0], chart.sizes.last().unwrap()] {
                    sizes.push((format!("{} {}", chart.id, size.label), size.mm.clone()));
                }
            }
            for (case, size) in sizes {
                let built = form.build(&size, quality).unwrap();
                let what = format!("{id} {case} ({quality:?})");
                assert_wound_outward(&format!("{what} torso"), &built.torso);
                assert_wound_outward(&format!("{what} stand"), &built.stand);
            }
        }
    }
}

/// The winding check has teeth: it refuses a mesh with one triangle turned round, and one turned
/// inside out.
mod the_winding_check {
    use super::*;

    fn torso() -> BodyMesh {
        let form = Form::bundled("men-torso").unwrap();
        let size = form.base_measurements(Quality::Low);
        form.build(&size, Quality::Low).unwrap().torso
    }

    #[test]
    fn accepts_the_form_as_built() {
        assert_wound_outward("torso", &torso());
    }

    #[test]
    #[should_panic(expected = "is used 2 times")]
    fn refuses_one_triangle_turned_round() {
        let mut m = torso();
        m.triangles[100].swap(1, 2);
        assert_wound_outward("torso", &m);
    }

    #[test]
    #[should_panic(expected = "faces inward")]
    fn refuses_a_mesh_turned_inside_out() {
        let mut m = torso();
        for t in &mut m.triangles {
            t.swap(1, 2);
        }
        assert_wound_outward("torso", &m);
    }
}
