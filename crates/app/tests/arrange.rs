//! Arranging pieces in 3D, driven through the gizmo's input handler with a fixed camera: no
//! window and no graphics card needed.

use glam::{DQuat, DVec2, DVec3, Vec3};
use opendrape::arrange::gizmo::{ARROW_PT, AXES, Gizmo, Handle, RING_PT};
use opendrape::arrange::{Arranger, SceneCache, ScreenCamera};
use opendrape::editor::{Document, Selection};
use opendrape_core::{InternalLine, Piece, PieceId, Placement, Point2, Project, Units};
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

/// A view of `at` from the corner of the room: every axis is 54.7° from the line of sight, so
/// every ring is seen from well off edge-on, where it is turned by where the pointer is on the
/// ring's own plane. (In `camera()` the y ring is seen at a grazing angle, and turns with the
/// pointer's movement across it instead.)
fn diagonal_camera(at: DVec3) -> ScreenCamera {
    let orbit = OrbitCamera {
        target: at.as_vec3(),
        yaw: std::f32::consts::FRAC_PI_4,
        pitch: (1.0_f32 / 3.0_f32.sqrt()).asin(),
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
    gizmo_in(&camera(), doc, cache, sel)
}

fn gizmo_in(cam: &ScreenCamera, doc: &Document, cache: &mut SceneCache, sel: &Selection) -> Gizmo {
    let scene = cache.scene(doc.project(), SHOULDER);
    Arranger::gizmo(cam, &scene, sel).expect("a selected piece has a gizmo")
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
    drag_in(&camera(), arranger, doc, cache, sel, (from, to), shift)
}

/// [`drag`] as seen through `cam`.
fn drag_in(
    cam: &ScreenCamera,
    arranger: &mut Arranger,
    doc: &mut Document,
    cache: &mut SceneCache,
    sel: &Selection,
    (from, to): (DVec2, DVec2),
    shift: bool,
) -> bool {
    let scene = cache.scene(doc.project(), SHOULDER);
    if !arranger.press(cam, &scene, sel, doc, from) {
        return false;
    }
    arranger.drag_to(cam, doc, from.lerp(to, 0.5), shift);
    arranger.drag_to(cam, doc, to, shift);
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
    let (mut doc, id, sel) = one_piece();
    let mut cache = SceneCache::default();
    let mut arranger = Arranger::default();
    let cam = diagonal_camera(position(&doc, id, &mut cache));
    let g = gizmo_in(&cam, &doc, &mut cache, &sel);
    // A point of the y ring that grabs it, and the point 40° further round.
    let (u, v) = AXES[1].any_orthonormal_pair();
    let r = g.size * RING_PT / ARROW_PT;
    let on_ring = |a: f64| g.centre + (u * a.cos() + v * a.sin()) * r;
    let a0 = (0..36)
        .map(|k| f64::from(k) * 10f64.to_radians())
        .find(|a| g.hit(&cam, cam.project(on_ring(*a)).unwrap()) == Some(Handle::Turn(1)))
        .expect("a point that grabs the y ring");
    let turn = 40f64.to_radians();
    let pull = (
        cam.project(on_ring(a0)).unwrap(),
        cam.project(on_ring(a0 + turn)).unwrap(),
    );
    assert!(drag_in(
        &cam,
        &mut arranger,
        &mut doc,
        &mut cache,
        &sel,
        pull,
        false
    ));
    let q = DQuat::from_array(doc.project().pieces[0].placement.unwrap().rotation);
    let (axis, angle) = q.to_axis_angle();
    assert!(
        (angle - turn).abs() < 1e-6 && (axis - u.cross(v)).length() < 1e-6,
        "{axis} {angle}"
    );
    doc.undo();
    assert!(drag_in(
        &cam,
        &mut arranger,
        &mut doc,
        &mut cache,
        &sel,
        pull,
        true
    ));
    let q = DQuat::from_array(doc.project().pieces[0].placement.unwrap().rotation);
    assert!(
        (q.to_axis_angle().1 - 45f64.to_radians()).abs() < 1e-6,
        "snapped to 45°"
    );
}

#[test]
fn a_ring_seen_at_a_grazing_angle_turns_with_the_pointer_across_it_and_shift_snaps() {
    // In this view the y ring is a thin sliver: it turns by 1 radian for every ring-radius
    // (60 points) the pointer goes across it, wherever on or beside the ring that is.
    let (mut doc, _, sel) = one_piece();
    let mut cache = SceneCache::default();
    let mut arranger = Arranger::default();
    let cam = camera();
    let g = gizmo(&doc, &mut cache, &sel);
    let centre = cam.project(g.centre).unwrap();
    // The side of the ring nearest the eye goes this way on screen, for a positive turn.
    let near = (cam.eye() - g.centre).normalize();
    let near = (near - DVec3::Y * near.y).normalize() * (g.size * RING_PT / ARROW_PT);
    let heading = (cam
        .project(g.centre + near + DVec3::Y.cross(near).normalize() * 0.001)
        .unwrap()
        - cam.project(g.centre + near).unwrap())
    .normalize();
    let (u, v) = AXES[1].any_orthonormal_pair();
    let on_ring = |a: f64| g.centre + (u * a.cos() + v * a.sin()) * (g.size * RING_PT / ARROW_PT);
    let grab = (0..36)
        .map(|k| {
            cam.project(on_ring(f64::from(k) * 10f64.to_radians()))
                .unwrap()
        })
        .find(|p| g.hit(&cam, *p) == Some(Handle::Turn(1)))
        .expect("a point that grabs the y ring");
    assert!(grab.distance(centre) > 20.0, "grabbed away from the middle");
    let turned = |doc: &Document| {
        let q = DQuat::from_array(doc.project().pieces[0].placement.unwrap().rotation);
        2.0 * q.y.atan2(q.w)
    };
    assert!(drag(
        &mut arranger,
        &mut doc,
        &mut cache,
        &sel,
        grab,
        grab + heading * 30.0,
        false
    ));
    // 30 points of 60: half a radian, a little off for perspective.
    assert!(
        (turned(&doc) - 0.5).abs() < 0.04,
        "{}°",
        turned(&doc).to_degrees()
    );
    doc.undo();
    // The other way is the other way round; along the ring's axis is no turn at all.
    assert!(drag(
        &mut arranger,
        &mut doc,
        &mut cache,
        &sel,
        grab,
        grab - heading * 30.0,
        false
    ));
    assert!((turned(&doc) + 0.5).abs() < 0.04);
    doc.undo();
    // Shift: half a radian is 28.6°, which snaps to 30°.
    assert!(drag(
        &mut arranger,
        &mut doc,
        &mut cache,
        &sel,
        grab,
        grab + heading * 30.0,
        true
    ));
    assert!((turned(&doc) - 30f64.to_radians()).abs() < 1e-6);
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
    arranger.hover(&cam, &scene, &sel, cam.project(g.centre));
    assert_eq!(arranger.hovered, Some(Handle::Plane));
    arranger.hover(&cam, &scene, &Selection::None, cam.project(g.centre));
    assert_eq!(arranger.hovered, None, "no gizmo without a selected piece");
}

#[test]
fn dragging_a_ring_further_round_than_half_a_turn_keeps_turning() {
    let (mut doc, id, sel) = one_piece();
    let mut cache = SceneCache::default();
    let mut arranger = Arranger::default();
    let cam = diagonal_camera(position(&doc, id, &mut cache));
    let g = gizmo_in(&cam, &doc, &mut cache, &sel);
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

/// The screen point of the arrow along `axis`, a tenth of its length in from the tip.
fn near_tip(g: &Gizmo, cam: &ScreenCamera, axis: usize) -> DVec2 {
    cam.project(g.arrow_tip(axis) - AXES[axis] * g.size * 0.1)
        .unwrap()
}

/// A screen point that grabs the ring about `axis`, and the screen point `turn` radians further
/// round it (its right-handed way: from `u` towards `v`, about `u × v`).
fn on_ring(g: &Gizmo, cam: &ScreenCamera, axis: usize, turn: f64) -> (DVec2, DVec2, DVec3) {
    let (u, v) = AXES[axis].any_orthonormal_pair();
    let r = g.size * RING_PT / ARROW_PT;
    let at = |a: f64| g.centre + (u * a.cos() + v * a.sin()) * r;
    let a0 = (0..36)
        .map(|k| f64::from(k) * 10f64.to_radians())
        .find(|a| g.hit(cam, cam.project(at(*a)).unwrap()) == Some(Handle::Turn(axis)))
        .unwrap_or_else(|| panic!("a point that grabs ring {axis}"));
    (
        cam.project(at(a0)).unwrap(),
        cam.project(at(a0 + turn)).unwrap(),
        u.cross(v),
    )
}

#[test]
fn a_click_on_a_handle_is_the_gizmos_and_neither_picks_nor_clears() {
    let (mut doc, a, sel) = one_piece();
    let b = doc.edit(|p| {
        p.add_piece(Piece::rectangle(
            PieceId(0),
            "Back",
            Point2::new(500.0, 0.0),
            300.0,
            400.0,
        ))
    });
    let mut cache = SceneCache::default();
    let g = gizmo(&doc, &mut cache, &sel);
    let cam = camera();
    let on_arrow = near_tip(&g, &cam, 1);
    // The back hangs right behind that point of the y arrow, 20 cm nearer the viewer than the
    // front: a click there would pick it.
    let (origin, dir) = cam.ray(on_arrow);
    let t = (g.centre - origin).dot(dir) - 0.2;
    doc.edit(|p| p.set_placement(b, Some(Placement::at((origin + dir * t).to_array()))));
    let scene = cache.scene(doc.project(), SHOULDER);
    assert_eq!(
        scene.pick(origin, dir),
        Some(b),
        "a piece is behind the handle"
    );

    let (ring, _, _) = on_ring(&g, &cam, 0, 0.1);
    let handles = [
        (on_arrow, "an arrow"),
        (ring, "a ring"),
        (cam.project(g.centre).unwrap(), "the centre square"),
    ];
    let mut arranger = Arranger::default();
    for (at, what) in handles {
        assert!(g.hit(&cam, at).is_some(), "{what} is under the pointer");
        let mut selection = sel;
        arranger.click(&cam, &scene, &mut selection, at);
        assert_eq!(selection, Selection::Piece(a), "a click on {what}");
    }
    // Off the gizmo, a click still picks the piece there, or clears the selection.
    let beside = cam
        .project(DVec3::from_array(scene.panel(b).unwrap().placement.position) - DVec3::X * 0.1)
        .unwrap();
    assert_eq!(g.hit(&cam, beside), None);
    let mut selection = sel;
    arranger.click(&cam, &scene, &mut selection, beside);
    assert_eq!(selection, Selection::Piece(b));
    arranger.click(&cam, &scene, &mut selection, DVec2::new(5.0, 40.0));
    assert_eq!(selection, Selection::None);
}

#[test]
fn hover_clears_when_the_pointer_leaves_the_view_or_nothing_is_selected() {
    let (doc, _, sel) = one_piece();
    let mut cache = SceneCache::default();
    let mut arranger = Arranger::default();
    let g = gizmo(&doc, &mut cache, &sel);
    let cam = camera();
    let scene = cache.scene(doc.project(), SHOULDER);
    arranger.hover(&cam, &scene, &sel, cam.project(g.centre));
    assert_eq!(arranger.hovered, Some(Handle::Plane));
    arranger.hover(&cam, &scene, &sel, None);
    assert_eq!(arranger.hovered, None, "the pointer left the view");
    arranger.hover(&cam, &scene, &sel, near_tip(&g, &cam, 2).into());
    assert_eq!(
        arranger.hovered,
        Some(Handle::Move(2)),
        "arrows are noted too"
    );
    arranger.hover(&cam, &scene, &Selection::None, near_tip(&g, &cam, 2).into());
    assert_eq!(arranger.hovered, None, "the selection went");
    let (ring, _, _) = on_ring(&g, &cam, 1, 0.1);
    arranger.hover(&cam, &scene, &sel, Some(ring));
    assert_eq!(arranger.hovered, Some(Handle::Turn(1)), "and rings");
}

#[test]
fn every_arrow_moves_the_piece_along_its_own_axis_and_every_ring_turns_it_about_its_own() {
    let cam = camera();
    for (axis, along) in AXES.into_iter().enumerate() {
        let (mut doc, id, sel) = one_piece();
        let mut cache = SceneCache::default();
        let mut arranger = Arranger::default();
        let start = position(&doc, id, &mut cache);
        let g = gizmo(&doc, &mut cache, &sel);
        assert!(
            g.arrow_shown(&cam, axis),
            "the test camera sees arrow {axis}"
        );
        let tip = g.arrow_tip(axis) - along * g.size * 0.1;
        let to = cam.project(tip + along * 0.1).unwrap();
        assert!(
            drag(
                &mut arranger,
                &mut doc,
                &mut cache,
                &sel,
                near_tip(&g, &cam, axis),
                to,
                false
            ),
            "arrow {axis} was grabbed"
        );
        let moved = position(&doc, id, &mut cache);
        assert!(
            (moved - (start + along * 0.1)).length() < 1e-6,
            "arrow {axis}: {start} → {moved}"
        );
    }
    // The rings, from the corner of the room (see `diagonal_camera`).
    for (axis, along) in AXES.into_iter().enumerate() {
        let (mut doc, id, sel) = one_piece();
        let mut cache = SceneCache::default();
        let mut arranger = Arranger::default();
        let cam = diagonal_camera(position(&doc, id, &mut cache));
        let g = gizmo_in(&cam, &doc, &mut cache, &sel);
        let turn = 40f64.to_radians();
        let (from, to, about) = on_ring(&g, &cam, axis, turn);
        assert!(drag_in(
            &cam,
            &mut arranger,
            &mut doc,
            &mut cache,
            &sel,
            (from, to),
            false
        ));
        let q = DQuat::from_array(doc.project().pieces[0].placement.unwrap().rotation);
        let want = DQuat::from_axis_angle(about, turn);
        assert!(
            q.dot(want).abs() > 1.0 - 1e-9,
            "ring {axis}: turned to {q:?}, wanted {want:?}"
        );
        assert!(
            (about - along).length() < 1e-9,
            "ring {axis} turns about axis {axis}"
        );
    }
}

#[test]
fn the_readout_says_how_far_and_which_way_for_every_axis_and_the_plane() {
    let cam = camera();
    // (axis, a move along it in metres, what the student reads)
    let cases = [
        (0, 0.12, "12.0 cm to the form's left"),
        (0, -0.12, "12.0 cm to the form's right"),
        (1, 0.12, "12.0 cm up"),
        (1, -0.12, "12.0 cm down"),
        (2, 0.12, "12.0 cm forward"),
        (2, -0.12, "12.0 cm back"),
    ];
    for (axis, by, says) in cases {
        let (mut doc, _, sel) = one_piece();
        let mut cache = SceneCache::default();
        let mut arranger = Arranger::default();
        let g = gizmo(&doc, &mut cache, &sel);
        let scene = cache.scene(doc.project(), SHOULDER);
        let tip = g.arrow_tip(axis) - AXES[axis] * g.size * 0.1;
        assert!(arranger.press(&cam, &scene, &sel, &mut doc, cam.project(tip).unwrap()));
        arranger.drag_to(
            &cam,
            &mut doc,
            cam.project(tip + AXES[axis] * by).unwrap(),
            false,
        );
        assert_eq!(
            arranger.readout(Units::Cm).as_deref(),
            Some(says),
            "axis {axis}, {by}"
        );
        arranger.release(&mut doc);
    }
    // In inches, and across the plane.
    let (mut doc, _, sel) = one_piece();
    let mut cache = SceneCache::default();
    let mut arranger = Arranger::default();
    let g = gizmo(&doc, &mut cache, &sel);
    let scene = cache.scene(doc.project(), SHOULDER);
    let tip = g.arrow_tip(1) - AXES[1] * g.size * 0.1;
    assert!(arranger.press(&cam, &scene, &sel, &mut doc, cam.project(tip).unwrap()));
    arranger.drag_to(
        &cam,
        &mut doc,
        cam.project(tip + DVec3::Y * 0.0254 * 4.0).unwrap(),
        false,
    );
    assert_eq!(arranger.readout(Units::Inch).as_deref(), Some("4.00 in up"));
    arranger.release(&mut doc);
    let (mut doc, _, sel) = one_piece();
    let mut arranger = Arranger::default();
    let scene = cache.scene(doc.project(), SHOULDER);
    let centre = cam.project(g.centre).unwrap();
    assert!(arranger.press(&cam, &scene, &sel, &mut doc, centre));
    arranger.drag_to(&cam, &mut doc, centre + DVec2::new(30.0, 0.0), false);
    let readout = arranger.readout(Units::Cm).expect("a readout");
    assert!(
        readout.ends_with(" cm") && !readout.contains("up"),
        "{readout}"
    );
}

#[test]
fn a_move_that_cannot_be_placed_leaves_the_piece_where_it_last_was_valid() {
    let (mut doc, id, sel) = one_piece();
    // The piece hangs 9 m up; the farthest a piece may be placed is 10 m from the origin.
    doc.edit(|p| p.set_placement(id, Some(Placement::at([0.0, 9.0, 0.5]))));
    let orbit = OrbitCamera {
        target: Vec3::new(0.0, 9.0, 0.5),
        yaw: 0.6,
        pitch: 0.3,
        distance: 2.6,
        fov_y: 35f32.to_radians(),
    };
    let cam = ScreenCamera::new(&orbit, DVec2::new(0.0, 30.0), DVec2::new(700.0, 600.0));
    let mut cache = SceneCache::default();
    let scene = cache.scene(doc.project(), SHOULDER);
    let g = Arranger::gizmo(&cam, &scene, &sel).unwrap();
    let tip = g.arrow_tip(1) - AXES[1] * g.size * 0.1;
    let mut arranger = Arranger::default();
    assert!(arranger.press(&cam, &scene, &sel, &mut doc, cam.project(tip).unwrap()));
    let up = |m: f64, arranger: &mut Arranger, doc: &mut Document| {
        arranger.drag_to(&cam, doc, cam.project(tip + DVec3::Y * m).unwrap(), false)
    };
    assert!(!up(0.5, &mut arranger, &mut doc));
    let valid = doc.project().placement_of(id).unwrap();
    assert!((valid.position[1] - 9.5).abs() < 1e-6);
    // 2 m up would be 11 m from the origin: nowhere a piece may go, so the piece stays.
    assert!(
        !up(2.0, &mut arranger, &mut doc),
        "nothing to report: nothing was refused"
    );
    assert_eq!(doc.project().placement_of(id), Some(valid));
    assert_eq!(
        arranger.readout(Units::Cm).as_deref(),
        Some("50.0 cm up"),
        "the readout is of the piece, not of the move that was held back"
    );
    // The drag carries on from there.
    up(0.3, &mut arranger, &mut doc);
    assert!((doc.project().placement_of(id).unwrap().position[1] - 9.3).abs() < 1e-6);
    arranger.release(&mut doc);
    assert!(doc.undo(), "one step");
    assert_eq!(
        doc.project().placement_of(id),
        Some(Placement::at([0.0, 9.0, 0.5]))
    );
}

#[test]
fn dragging_an_almost_end_on_arrow_towards_its_vanishing_point_never_leaves_the_readout_behind() {
    let mut held_back = 0;
    for yaw in [0.17, 0.2, 0.25, 0.3] {
        // The piece hangs 3 m up, so that 10 m back is further than a piece may be placed.
        let orbit = OrbitCamera {
            target: Vec3::new(0.0, 3.0, 0.4),
            yaw,
            pitch: 0.1,
            distance: 2.6,
            fov_y: 35f32.to_radians(),
        };
        let cam = ScreenCamera::new(&orbit, DVec2::new(0.0, 30.0), DVec2::new(700.0, 600.0));
        let (mut doc, id, sel) = one_piece();
        doc.edit(|p| p.set_placement(id, Some(Placement::at([0.0, 3.0, 0.4]))));
        let mut cache = SceneCache::default();
        let start = position(&doc, id, &mut cache);
        let scene = cache.scene(doc.project(), SHOULDER);
        let g = Arranger::gizmo(&cam, &scene, &sel).unwrap();
        let mut arranger = Arranger::default();
        let from = near_tip(&g, &cam, 2);
        assert!(
            arranger.press(&cam, &scene, &sel, &mut doc, from),
            "yaw {yaw}: z arrow"
        );
        let along =
            (cam.project(g.arrow_tip(2)).unwrap() - cam.project(g.centre).unwrap()).normalize();
        let mut last = start;
        for step in 1..=120 {
            let to = from - along * (f64::from(step) * 4.0);
            arranger.drag_to(&cam, &mut doc, to, false);
            let placement = doc
                .project()
                .placement_of(id)
                .unwrap_or(Placement::at(start.to_array()));
            let now = DVec3::from_array(placement.position);
            assert!(
                placement.is_valid(),
                "yaw {yaw}, step {step}: {placement:?}"
            );
            held_back += usize::from(now == last);
            last = now;
            // What the student reads is what the piece did.
            let dz = now.z - start.z;
            let way = if dz >= 0.0 { "forward" } else { "back" };
            let says = format!("{} {way}", Units::Cm.format(dz.abs() * 1000.0));
            if let Some(readout) = arranger.readout(Units::Cm) {
                assert_eq!(readout, says, "yaw {yaw}, step {step}");
            }
        }
        arranger.release(&mut doc);
    }
    assert!(
        held_back > 0,
        "the sweep never ran past what a piece may do"
    );
}

#[test]
fn a_move_the_project_refuses_is_reported_once_per_drag_and_not_shown() {
    // A project that is already invalid refuses every change (an internal line of one point).
    let mut project = Project::new();
    let mut piece = Piece::rectangle(PieceId(0), "Front", Point2::new(0.0, 0.0), 300.0, 400.0);
    piece.lines = vec![InternalLine::open(&[Point2::new(10.0, 10.0)])];
    let id = project.add_piece(piece);
    let mut doc = Document::new(project, None);
    assert!(doc.project().check().is_err());
    let sel = Selection::Piece(id);
    let mut cache = SceneCache::default();
    let mut arranger = Arranger::default();
    let cam = camera();
    for drag in 1..=2 {
        let scene = cache.scene(doc.project(), SHOULDER);
        let g = Arranger::gizmo(&cam, &scene, &sel).unwrap();
        let centre = cam.project(g.centre).unwrap();
        assert!(arranger.press(&cam, &scene, &sel, &mut doc, centre));
        let mut reported = Vec::new();
        for k in 1..=4 {
            let to = centre + DVec2::new(10.0 * f64::from(k), 0.0);
            reported.push(arranger.drag_to(&cam, &mut doc, to, false));
        }
        assert_eq!(
            reported,
            [true, false, false, false],
            "drag {drag}: once, the first time"
        );
        assert_eq!(
            doc.project().pieces[0].placement,
            None,
            "nothing was written"
        );
        assert_eq!(
            arranger.readout(Units::Cm),
            None,
            "no readout of a move that was refused"
        );
        arranger.release(&mut doc);
        assert!(!doc.can_undo(), "no step");
    }
}

#[test]
fn a_drag_that_nets_to_nothing_changes_nothing_and_makes_no_undo_step() {
    let cam = camera();
    // A piece with no placement of its own, and its twin that mirrors it.
    let (mut doc, id, sel) = one_piece();
    let twin = doc
        .edit(|p| p.add_twin(id, "Front (mirror)".into(), Point2::new(700.0, 0.0)))
        .unwrap();
    let mut cache = SceneCache::default();
    let mut arranger = Arranger::default();
    let before = doc.project().clone();
    let g = gizmo(&doc, &mut cache, &sel);
    let scene = cache.scene(doc.project(), SHOULDER);

    // Up and back to where the pointer began.
    let from = near_tip(&g, &cam, 1);
    assert!(arranger.press(&cam, &scene, &sel, &mut doc, from));
    arranger.drag_to(&cam, &mut doc, from + DVec2::new(0.0, -30.0), false);
    assert!(doc.project().placement_of(id).is_some(), "it moved");
    arranger.drag_to(&cam, &mut doc, from, false);
    assert_eq!(
        doc.project().placement_of(id),
        None,
        "back to taking its place from elsewhere"
    );
    arranger.release(&mut doc);
    assert_eq!(*doc.project(), before);

    // A ring turned 3°, which Shift snaps to 0°.
    let (from, to, _) = on_ring(&g, &cam, 1, 3f64.to_radians());
    assert!(drag(
        &mut arranger,
        &mut doc,
        &mut cache,
        &sel,
        from,
        to,
        true
    ));
    assert_eq!(*doc.project(), before, "snapped back to where it began");
    // The same, plainly: a 3° turn really is a turn, and a drag back undoes it.
    assert!(arranger.press(
        &cam,
        &cache.scene(doc.project(), SHOULDER),
        &sel,
        &mut doc,
        from
    ));
    arranger.drag_to(&cam, &mut doc, to, false);
    assert!(doc.project().placement_of(id).is_some());
    arranger.drag_to(&cam, &mut doc, from, false);
    arranger.release(&mut doc);
    assert_eq!(*doc.project(), before, "turned and turned back");

    // The twin still follows its piece: nothing was copied onto it.
    assert_eq!(doc.project().placement_of(twin), None);
    // Nothing was added to the history by any of it: one Undo takes the twin away.
    assert!(doc.undo());
    assert!(doc.project().pieces[0].twin.is_none());
    assert!(doc.undo());
    assert!(
        doc.project().pieces.is_empty(),
        "the piece's own step is the first"
    );
    assert!(!doc.can_undo());
}

#[test]
fn a_piece_that_had_its_own_place_gets_it_back_when_a_drag_nets_to_nothing() {
    let cam = camera();
    let (mut doc, id, sel) = one_piece();
    // A turn a student typed in, a hair off unit length (a placement may be off by 1e-6).
    let rotation = DQuat::from_xyzw(0.1, 0.2, 0.3, 0.9).normalize() * (1.0 + 3e-7);
    let own = Placement {
        position: [0.05, 1.1, 0.4],
        rotation: rotation.to_array(),
        curve: Some(0.25),
    };
    assert!(own.is_valid());
    doc.edit(|p| p.set_placement(id, Some(own)));
    let mut cache = SceneCache::default();
    let mut arranger = Arranger::default();
    let before = doc.project().clone();
    let g = gizmo(&doc, &mut cache, &sel);
    let (from, to, _) = on_ring(&g, &cam, 1, 3f64.to_radians());
    assert!(drag(
        &mut arranger,
        &mut doc,
        &mut cache,
        &sel,
        from,
        to,
        true
    ));
    assert_eq!(*doc.project(), before, "its rotation is exactly as it was");
    assert!(doc.undo());
    assert_eq!(
        doc.project().placement_of(id),
        None,
        "that was the step that placed it"
    );
}

#[test]
fn giving_up_a_drag_puts_the_piece_back_with_no_undo_step() {
    let cam = camera();
    for own in [None, Some(Placement::at([0.1, 1.2, 0.3]))] {
        let (mut doc, id, sel) = one_piece();
        if own.is_some() {
            doc.edit(|p| p.set_placement(id, own));
        }
        let mut cache = SceneCache::default();
        let mut arranger = Arranger::default();
        let before = doc.project().clone();
        let scene = cache.scene(doc.project(), SHOULDER);
        let g = Arranger::gizmo(&cam, &scene, &sel).unwrap();
        let from = near_tip(&g, &cam, 1);
        assert!(arranger.press(&cam, &scene, &sel, &mut doc, from));
        arranger.drag_to(&cam, &mut doc, from + DVec2::new(0.0, -40.0), false);
        assert_ne!(*doc.project(), before, "it moved");
        arranger.cancel(&mut doc);
        assert_eq!(
            *doc.project(),
            before,
            "back where the drag found it ({own:?})"
        );
        assert!(!arranger.is_dragging());
        assert_eq!(arranger.readout(Units::Cm), None);
        // The pointer going on moving does nothing, and letting go makes no step.
        arranger.drag_to(&cam, &mut doc, from + DVec2::new(0.0, -60.0), false);
        arranger.release(&mut doc);
        assert_eq!(*doc.project(), before);
        // The history is as it was: the placing (if any) and the piece.
        if own.is_some() {
            assert!(doc.undo());
            assert_eq!(doc.project().placement_of(id), None);
        }
        assert!(doc.undo());
        assert!(doc.project().pieces.is_empty());
        assert!(!doc.can_undo(), "the drag made no step");
    }
}
