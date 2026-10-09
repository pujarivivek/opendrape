use opendrape_body::{BodyMesh, boundary_edge_count, girth_at, height};

#[test]
fn bundled_female_body_is_the_expected_closed_mesh() {
    let b = BodyMesh::female_average();
    assert_eq!((b.positions.len(), b.triangles.len()), (13380, 26756));
    assert_eq!(
        boundary_edge_count(&b),
        0,
        "must be closed for inside/outside tests"
    );
    let h = height(&b);
    assert!((1.585..1.595).contains(&h), "height {h}");
    assert_eq!(
        b.positions.iter().map(|p| p.y).fold(f32::MAX, f32::min),
        0.0,
        "feet on the floor"
    );
}

#[test]
fn bundled_female_body_has_plausible_girths() {
    let b = BodyMesh::female_average();
    let waist = girth_at(&b, 1.028, 0.2);
    let hips = girth_at(&b, 0.767, 0.2);
    assert!((0.63..0.68).contains(&waist), "waist {waist}");
    assert!((0.92..0.97).contains(&hips), "hips {hips}");
}
