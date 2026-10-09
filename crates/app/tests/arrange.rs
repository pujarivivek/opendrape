//! Arranging pieces in 3D, driven through the gizmo's input handler with a fixed camera: no
//! window and no graphics card needed.

use glam::{DQuat, DVec2, DVec3, Vec3};
use opendrape::arrange::gizmo::{ARROW_PT, AXES, Gizmo, Handle, RING_PT};
use opendrape::arrange::{Arranger, SceneCache, ScreenCamera};
use opendrape::editor::{Document, Selection};
use opendrape_core::{Piece, PieceId, Point2, Units};
use opendrape_mesh::place;
use opendrape_render::OrbitCamera;

const SHOULDER: f64 = 1.3;

/// The view the tests look through: from the front left, a little from above.
fn camera() -> ScreenCamera {
    let orbit = OrbitCamera {
        target: Vec3::new(0.0, 1.0, 0.0),
        yaw: 0.6,
        pitch: 0.3,
        distance: 2.6,
        fov_y: 35f32.to_radians(),
    };
    ScreenCamera::new(&orbit, DVec2::new(0.0, 30.0), DVec2::new(700.0, 600.0))
}

/// A document with one 300 × 400 mm piece, selected.
fn one_piece() -> (Document, PieceId, Selection) {
    let mut doc = Document::default();
    let id = doc.edit(|p| {
        p.add_piece(Piece::rectangle(
            PieceId(0),
            "Front",
            Point2::new(0.0, 0.0),
            300.0,
            400.0,
        ))
    });
    (doc, id, Selection::Piece(id))
}

fn position(doc: &Document, id: PieceId, cache: &mut SceneCache) -> DVec3 {
    let scene = cache.scene(doc.project(), SHOULDER);
    DVec3::from_array(scene.panel(id).unwrap().placement.position)
}

fn gizmo(doc: &Document, cache: &mut SceneCache, sel: &Selection) -> Gizmo {
    let scene = cache.scene(doc.project(), SHOULDER);
    Arranger::gizmo(&camera(), &scene, sel).expect("a selected piece has a gizmo")
}

/// Presses on `from`, moves to `to`, releases (screen points).
fn drag(
    arranger: &mut Arranger,
    doc: &mut Document,
    cache: &mut SceneCache,
    sel: &Selection,
    from: DVec2,
    to: DVec2,
    shift: bool,
) -> bool {
    let cam = camera();
    let scene = cache.scene(doc.project(), SHOULDER);
    if !arranger.press(&cam, &scene, sel, doc, from) {
        return false;
    }
    arranger.drag_to(&cam, doc, from.lerp(to, 0.5), shift);
    arranger.drag_to(&cam, doc, to, shift);
    arranger.release(doc);
    true
}

#[test]
fn dragging_an_arrow_moves_the_piece_along_it_as_one_undo_step() {
    let (mut doc, id, sel) = one_piece();
    let mut cache = SceneCache::default();
    let mut arranger = Arranger::default();
    let start = position(&doc, id, &mut cache);
    let g = gizmo(&doc, &mut cache, &sel);
    let cam = camera();
    // Grab the up arrow near its tip and pull it up by the screen length of 10 cm.
    let tip = g.arrow_tip(1) - AXES[1] * g.size * 0.1;
    let (from, to) = (
        cam.project(tip).unwrap(),
        cam.project(tip + DVec3::Y * 0.1).unwrap(),
    );
    assert!(drag(
        &mut arranger,
        &mut doc,
        &mut cache,
        &sel,
        from,
        to,
        false
    ));
    let moved = position(&doc, id, &mut cache);
    assert!(
        (moved - (start + DVec3::Y * 0.1)).length() < 1e-6,
        "{start} → {moved}"
    );
    assert!(doc.undo(), "one step");
    assert_eq!(
        doc.project().pieces[0].placement,
        None,
        "back at its starting place"
    );
}

#[test]
fn dragging_a_ring_turns_the_piece_and_shift_snaps_to_15_degrees() {
    let (mut doc, _, sel) = one_piece();
    let mut cache = SceneCache::default();
    let mut arranger = Arranger::default();
    let g = gizmo(&doc, &mut cache, &sel);
    let cam = camera();
    // A point of the y ring that grabs it, and the point 40° further round.
    let (u, v) = AXES[1].any_orthonormal_pair();
    let r = g.size * RING_PT / ARROW_PT;
    let on_ring = |a: f64| g.centre + (u * a.cos() + v * a.sin()) * r;
    let a0 = (0..36)
        .map(|k| f64::from(k) * 10f64.to_radians())
        .find(|a| g.hit(&cam, cam.project(on_ring(*a)).unwrap()) == Some(Handle::Turn(1)))
        .expect("a point that grabs the y ring");
    let turn = 40f64.to_radians();
    let (from, to) = (
        cam.project(on_ring(a0)).unwrap(),
        cam.project(on_ring(a0 + turn)).unwrap(),
    );
    assert!(drag(
        &mut arranger,
        &mut doc,
        &mut cache,
        &sel,
        from,
        to,
        false
    ));
    let q = DQuat::from_array(doc.project().pieces[0].placement.unwrap().rotation);
    let (axis, angle) = q.to_axis_angle();
    assert!(
        (angle - turn).abs() < 1e-6 && (axis - u.cross(v)).length() < 1e-6,
        "{axis} {angle}"
    );
    doc.undo();
    assert!(drag(
        &mut arranger,
        &mut doc,
        &mut cache,
        &sel,
        from,
        to,
        true
    ));
    let q = DQuat::from_array(doc.project().pieces[0].placement.unwrap().rotation);
    assert!(
        (q.to_axis_angle().1 - 45f64.to_radians()).abs() < 1e-6,
        "snapped to 45°"
    );
}

#[test]
fn the_centre_square_moves_the_piece_under_the_pointer() {
    let (mut doc, id, sel) = one_piece();
    let mut cache = SceneCache::default();
    let mut arranger = Arranger::default();
    let start = position(&doc, id, &mut cache);
    let cam = camera();
    let from = cam.project(start).unwrap();
    let to = from + DVec2::new(40.0, 25.0);
    assert!(drag(
        &mut arranger,
        &mut doc,
        &mut cache,
        &sel,
        from,
        to,
        false
    ));
    let moved = position(&doc, id, &mut cache);
    assert!(cam.project(moved).unwrap().distance(to) < 1e-6);
}

#[test]
fn a_press_away_from_the_gizmo_leaves_the_drag_to_the_camera() {
    let (mut doc, _, sel) = one_piece();
    let mut cache = SceneCache::default();
    let mut arranger = Arranger::default();
    let far = DVec2::new(5.0, 40.0);
    assert!(!drag(
        &mut arranger,
        &mut doc,
        &mut cache,
        &sel,
        far,
        far + DVec2::new(30.0, 0.0),
        false
    ));
    assert!(!arranger.is_dragging());
    assert!(!drag(
        &mut arranger,
        &mut doc,
        &mut cache,
        &Selection::None,
        far,
        far,
        false
    ));
    assert_eq!(doc.project().pieces[0].placement, None);
}

#[test]
fn moving_a_piece_moves_its_unplaced_twin_and_a_moved_twin_keeps_its_place() {
    let (mut doc, id, sel) = one_piece();
    let twin = doc
        .edit(|p| p.add_twin(id, "Front (mirror)".into(), Point2::new(700.0, 0.0)))
        .unwrap();
    let mut cache = SceneCache::default();
    let mut arranger = Arranger::default();
    let cam = camera();
    let start = position(&doc, id, &mut cache);
    let to = cam.project(start + DVec3::new(0.0, 0.05, 0.0)).unwrap();
    assert!(drag(
        &mut arranger,
        &mut doc,
        &mut cache,
        &sel,
        cam.project(start).unwrap(),
        to,
        false
    ));
    let placed = doc.project().pieces[0].placement.unwrap();
    let scene = cache.scene(doc.project(), SHOULDER);
    assert_eq!(
        scene.panel(twin).unwrap().placement,
        place::mirrored(&placed),
        "mirrors it"
    );
    // Now move the twin itself: it keeps its own place from then on.
    let twin_sel = Selection::Piece(twin);
    let at = position(&doc, twin, &mut cache);
    let to = cam.project(at + DVec3::new(0.0, -0.05, 0.0)).unwrap();
    assert!(drag(
        &mut arranger,
        &mut doc,
        &mut cache,
        &twin_sel,
        cam.project(at).unwrap(),
        to,
        false
    ));
    let own = doc.project().placement_of(twin).expect("its own placement");
    let at = cam.project(position(&doc, id, &mut cache)).unwrap();
    drag(
        &mut arranger,
        &mut doc,
        &mut cache,
        &sel,
        at,
        at + DVec2::new(20.0, 0.0),
        false,
    );
    assert_eq!(
        doc.project().placement_of(twin),
        Some(own),
        "the twin stays put"
    );
}

#[test]
fn the_readout_says_how_far_and_which_way() {
    let (mut doc, _, sel) = one_piece();
    let mut cache = SceneCache::default();
    let mut arranger = Arranger::default();
    let g = gizmo(&doc, &mut cache, &sel);
    let cam = camera();
    let scene = cache.scene(doc.project(), SHOULDER);
    let tip = g.arrow_tip(1) - AXES[1] * g.size * 0.1;
    assert!(arranger.press(&cam, &scene, &sel, &mut doc, cam.project(tip).unwrap()));
    assert_eq!(arranger.readout(Units::Cm), None, "nothing moved yet");
    arranger.drag_to(
        &cam,
        &mut doc,
        cam.project(tip - DVec3::Y * 0.12).unwrap(),
        false,
    );
    assert_eq!(arranger.readout(Units::Cm).as_deref(), Some("12.0 cm down"));
    arranger.release(&mut doc);
    assert_eq!(arranger.readout(Units::Cm), None);
}

#[test]
fn the_handle_under_the_pointer_is_noted_for_drawing() {
    let (doc, _, sel) = one_piece();
    let mut cache = SceneCache::default();
    let mut arranger = Arranger::default();
    let g = gizmo(&doc, &mut cache, &sel);
    let cam = camera();
    let scene = cache.scene(doc.project(), SHOULDER);
    arranger.hover(&cam, &scene, &sel, cam.project(g.centre).unwrap());
    assert_eq!(arranger.hovered, Some(Handle::Plane));
    arranger.hover(
        &cam,
        &scene,
        &Selection::None,
        cam.project(g.centre).unwrap(),
    );
    assert_eq!(arranger.hovered, None, "no gizmo without a selected piece");
}

#[test]
fn dragging_a_ring_further_round_than_half_a_turn_keeps_turning() {
    let (mut doc, _, sel) = one_piece();
    let mut cache = SceneCache::default();
    let mut arranger = Arranger::default();
    let g = gizmo(&doc, &mut cache, &sel);
    let cam = camera();
    let (u, v) = AXES[1].any_orthonormal_pair();
    let r = g.size * RING_PT / ARROW_PT;
    let on_ring = |a: f64| g.centre + (u * a.cos() + v * a.sin()) * r;
    let a0 = (0..36)
        .map(|k| f64::from(k) * 10f64.to_radians())
        .find(|a| g.hit(&cam, cam.project(on_ring(*a)).unwrap()) == Some(Handle::Turn(1)))
        .expect("a point that grabs the y ring");
    let scene = cache.scene(doc.project(), SHOULDER);
    assert!(arranger.press(
        &cam,
        &scene,
        &sel,
        &mut doc,
        cam.project(on_ring(a0)).unwrap()
    ));
    // Round the ring in twelve moves of about 19°: 229° in all. Measured from where the drag
    // began, 229° reads as -131°, and the piece would turn back.
    let turn = 229f64.to_radians();
    for step in 1..=12 {
        let a = a0 + turn * f64::from(step) / 12.0;
        arranger.drag_to(&cam, &mut doc, cam.project(on_ring(a)).unwrap(), false);
    }
    assert!(
        arranger
            .readout(Units::Cm)
            .is_some_and(|text| text.contains("229")),
        "{:?}",
        arranger.readout(Units::Cm)
    );
    arranger.release(&mut doc);
    let q = DQuat::from_array(doc.project().pieces[0].placement.unwrap().rotation);
    let want = DQuat::from_axis_angle(u.cross(v), turn);
    assert!(
        q.dot(want).abs() > 1.0 - 1e-9,
        "turned to {q:?}, wanted {want:?}"
    );
    assert!(doc.undo(), "one step");
    assert_eq!(doc.project().pieces[0].placement, None);
}
