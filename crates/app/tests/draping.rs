//! Grabbing and pinning the fabric in the 3D view while it drapes, driven through the 3D view's
//! input handler (`Draper`) with a fixed camera: no window and no graphics card needed.

use glam::{DVec2, DVec3, Vec3};
use opendrape::SimFrame;
use opendrape::arrange::ScreenCamera;
use opendrape::draping::{Draper, MenuAt, Pull};
use opendrape::editor::{Document, PinShift, Selection};
use opendrape::sim_runner::SimRunner;
use opendrape_core::{Half, Piece, PieceId, Pin, Placement, Point2, Project};
use opendrape_drape::{Drape, Stage};
use opendrape_render::OrbitCamera;
use std::sync::Arc;
use std::time::{Duration, Instant};

/// The view the tests look through: from the front, at 1 m up, 1.5 m away.
fn camera() -> ScreenCamera {
    let orbit = OrbitCamera {
        target: Vec3::new(0.0, 1.0, 0.5),
        yaw: 0.0,
        pitch: 0.0,
        distance: 1.5,
        fov_y: 35f32.to_radians(),
    };
    ScreenCamera::new(&orbit, DVec2::new(0.0, 0.0), DVec2::new(800.0, 600.0))
}

/// A 300 × 400 mm front cut on the fold (its left edge: the whole piece is x -300..300), flat
/// and upright at z = 0.5 m facing the camera, its middle at 1 m up.
fn front() -> Project {
    let mut pr = Project::new();
    let mut half = Piece::rectangle(PieceId(0), "Front", Point2::new(0.0, 0.0), 300.0, 400.0);
    half.fold = Some(3);
    let id = pr.add_piece(half);
    pr.set_placement(id, Some(Placement::at([0.0, 1.0, 0.5])));
    pr
}

/// The first frame of `project`'s drape, as the 3D view would get it.
fn first_frame(project: &Project) -> SimFrame {
    let drape = Drape::new(Arc::new(project.clone()), &Stage::shared());
    let cloth = drape.solver.cloth();
    SimFrame {
        drape: 1,
        seq: 1,
        time: 0.0,
        positions: cloth.positions().iter().map(|p| p.as_vec3()).collect(),
        triangles: Arc::new(cloth.triangles().to_vec()),
        step_ms: 0.0,
        notes: Arc::new(Vec::new()),
        fabric: drape.fabric.clone(),
    }
}

/// Where the piece's spot (x, y) mm on the pattern (the whole front, centred on (0, 200)) is
/// in 3D, and on screen.
fn spot(x: f64, y: f64) -> (DVec3, DVec2) {
    let at = DVec3::new(x / 1000.0, 1.0 + (y - 200.0) / 1000.0, 0.5);
    (at, camera().project(at).unwrap())
}

#[test]
fn a_press_on_the_fabric_grabs_the_spot_under_the_pointer_and_drags_it_facing_the_viewer() {
    let pr = front();
    let frame = first_frame(&pr);
    let mut doc = Document::new(pr, None);
    let mut draper = Draper::default();
    let (at, on_screen) = spot(100.0, 300.0);
    let Some(Pull::Grab {
        fabric,
        triangle,
        bary,
        target,
    }) = draper.press(&camera(), &frame, &mut doc, on_screen)
    else {
        panic!("a grab");
    };
    assert!((target - at).length() < 1e-6, "{target}");
    let (shape, back) = fabric.pattern_point(triangle, bary).unwrap();
    assert_eq!(shape, PieceId(1));
    assert!(back.distance(Point2::new(100.0, 300.0)) < 1e-3, "{back:?}");
    assert!(draper.is_dragging());
    // 40 points right and 20 up: the target moves in the plane facing the viewer, under the
    // pointer.
    let to = on_screen + DVec2::new(40.0, -20.0);
    let Some(Pull::To(moved)) = draper.drag_to(&camera(), &mut doc, to) else {
        panic!("a pull");
    };
    assert!((moved.z - 0.5).abs() < 1e-9, "at the depth it was grabbed");
    assert!(camera().project(moved).unwrap().distance(to) < 1e-6);
    assert_eq!(draper.release(&mut doc), Some(Pull::Release));
    assert!(!doc.can_undo(), "a grab is no edit");
    // Beside the fabric, a press grabs nothing (the drag turns the camera).
    assert_eq!(
        draper.press(&camera(), &frame, &mut doc, DVec2::new(5.0, 5.0)),
        None
    );
    assert!(!draper.is_dragging());
}

#[test]
fn pin_here_pins_the_spot_under_the_pointer_where_it_is_on_either_half() {
    let pr = front();
    let frame = first_frame(&pr);
    let mut draper = Draper::default();
    // On the drawn half (x > 0) the spot is kept as it is; on the pale half, as its mirror image.
    for (x, half, kept) in [(100.0, Half::Drawn, 100.0), (-100.0, Half::Pale, 100.0)] {
        let (at, on_screen) = spot(x, 300.0);
        draper.secondary_click(&camera(), &frame, &pr, on_screen);
        let Some(MenuAt::Fabric(pin)) = draper.menu else {
            panic!("Pin here at {x}");
        };
        assert_eq!((pin.shape, pin.half), (PieceId(1), half));
        assert!(
            pin.at.distance(Point2::new(kept, 300.0)) < 1e-3,
            "{:?}",
            pin.at
        );
        assert!((DVec3::from_array(pin.target) - at).length() < 1e-6);
    }
}

#[test]
fn a_pin_marker_is_dragged_as_one_undo_step_selected_by_a_click_and_removed_from_its_menu() {
    let mut pr = front();
    let (at, on_screen) = spot(100.0, 300.0);
    pr.pins = vec![Pin {
        shape: PieceId(1),
        half: Half::Drawn,
        at: Point2::new(100.0, 300.0),
        target: at.to_array(),
    }];
    let frame = first_frame(&pr);
    let mut doc = Document::new(pr.clone(), None);
    let mut draper = Draper::default();
    assert_eq!(
        draper.press(&camera(), &frame, &mut doc, on_screen),
        None,
        "not a grab"
    );
    assert!(draper.is_dragging());
    for k in 1..=4 {
        draper.drag_to(
            &camera(),
            &mut doc,
            on_screen + DVec2::new(0.0, -10.0 * f64::from(k)),
        );
    }
    draper.release(&mut doc);
    let moved = DVec3::from_array(doc.project().pins[0].target);
    assert!(
        moved.y > at.y + 0.01 && (moved.z - 0.5).abs() < 1e-9,
        "{moved}"
    );
    assert!(doc.undo(), "one step");
    assert_eq!(doc.project().pins[0].target, at.to_array());
    assert!(!doc.undo());
    // A click on the marker selects the pin; a right-click offers Remove pin.
    let mut selection = Selection::None;
    draper.click(&camera(), doc.project(), &mut selection, on_screen);
    assert_eq!(selection, Selection::Pin(0));
    draper.secondary_click(&camera(), &frame, doc.project(), on_screen);
    assert_eq!(draper.menu, Some(MenuAt::Pin(0)));
}

/// Three pins on the front's right half at x = 50, 150 and 250 mm, 300 mm up, held where the
/// fabric is.
fn three_pins() -> (Project, Vec<DVec2>) {
    let mut pr = front();
    let mut on_screen = Vec::new();
    for x in [50.0, 150.0, 250.0] {
        let (at, shown) = spot(x, 300.0);
        pr.pins.push(Pin {
            shape: PieceId(1),
            half: Half::Drawn,
            at: Point2::new(x, 300.0),
            target: at.to_array(),
        });
        on_screen.push(shown);
    }
    (pr, on_screen)
}

#[test]
fn escape_gives_a_pin_move_up_the_pin_goes_back_and_there_is_no_undo_step() {
    let (pr, on_screen) = three_pins();
    let frame = first_frame(&pr);
    let mut doc = Document::new(pr.clone(), None);
    let mut draper = Draper::default();
    assert_eq!(
        draper.press(&camera(), &frame, &mut doc, on_screen[1]),
        None
    );
    for k in 1..=4 {
        draper.drag_to(
            &camera(),
            &mut doc,
            on_screen[1] + DVec2::new(0.0, -10.0 * f64::from(k)),
        );
    }
    assert_ne!(
        doc.project().pins[1].target,
        pr.pins[1].target,
        "it followed"
    );
    assert_eq!(
        draper.cancel(&mut doc),
        None,
        "a pin move asks nothing of the cloth"
    );
    assert_eq!(*doc.project(), pr, "back where it was held");
    assert!(!doc.can_undo(), "and no step was made");
    // The press is still down, and still the 3D view's: it does nothing more, and its
    // release makes no step either.
    assert!(draper.is_dragging());
    let more = on_screen[1] + DVec2::new(30.0, -50.0);
    assert_eq!(draper.drag_to(&camera(), &mut doc, more), None);
    assert_eq!(draper.release(&mut doc), None);
    assert_eq!(*doc.project(), pr);
    assert!(!doc.can_undo());
    assert!(!draper.is_dragging(), "the next press is a new one");
}

#[test]
fn escape_lets_go_of_a_grab_once() {
    let pr = front();
    let frame = first_frame(&pr);
    let mut doc = Document::new(pr, None);
    let mut draper = Draper::default();
    let (_, on_screen) = spot(100.0, 300.0);
    assert!(matches!(
        draper.press(&camera(), &frame, &mut doc, on_screen),
        Some(Pull::Grab { .. })
    ));
    assert_eq!(draper.cancel(&mut doc), Some(Pull::Release));
    assert!(
        draper.is_dragging(),
        "the press goes on until the button is up"
    );
    assert_eq!(
        draper.drag_to(&camera(), &mut doc, on_screen + DVec2::new(20.0, 0.0)),
        None,
        "it pulls nothing now"
    );
    assert_eq!(draper.cancel(&mut doc), None, "nothing left to give up");
    assert_eq!(
        draper.release(&mut doc),
        None,
        "and nothing to let go twice"
    );
    assert!(!draper.is_dragging());
    assert!(!doc.can_undo());
}

#[test]
fn a_pin_move_and_a_pin_menu_follow_their_pin_when_pins_before_it_are_taken_away() {
    let (pr, on_screen) = three_pins();
    let frame = first_frame(&pr);
    let mut doc = Document::new(pr.clone(), None);
    let mut draper = Draper::default();
    // Pin 2 is being moved when pin 0 is taken away: it is pin 1 now, and the move goes on
    // with it, not with the pin that took its number.
    draper.press(&camera(), &frame, &mut doc, on_screen[2]);
    draper.drag_to(&camera(), &mut doc, on_screen[2] + DVec2::new(0.0, -20.0));
    let so_far = DVec3::from_array(doc.project().pins[2].target);
    assert!(so_far.y > DVec3::from_array(pr.pins[2].target).y + 0.02);
    doc.edit(|p| {
        p.pins.remove(0);
    });
    for shift in doc.take_pin_shifts() {
        draper.pins_shifted(&shift, &mut doc);
    }
    draper.drag_to(&camera(), &mut doc, on_screen[2] + DVec2::new(0.0, -60.0));
    let pins = &doc.project().pins;
    assert_eq!(pins.len(), 2);
    assert_eq!(
        pins[0].target, pr.pins[1].target,
        "the other pin was not touched"
    );
    let moved = DVec3::from_array(pins[1].target);
    assert!(
        moved.y > so_far.y + 0.03,
        "the pin it began with was moved on: {moved}"
    );
    // Its own pin going ends the move.
    doc.edit(|p| {
        p.pins.remove(1);
    });
    for shift in doc.take_pin_shifts() {
        draper.pins_shifted(&shift, &mut doc);
    }
    assert!(!draper.is_dragging());
    assert_eq!(draper.drag_to(&camera(), &mut doc, on_screen[2]), None);
    assert_eq!(doc.project().pins.len(), 1);
    assert_eq!(
        doc.project().pins[0].target,
        pr.pins[1].target,
        "no pin was moved"
    );
    // A menu on a pin follows it, or goes with it.
    draper.menu = Some(MenuAt::Pin(2));
    draper.pins_shifted(&PinShift::between(&pr.pins, &pr.pins[1..]), &mut doc);
    assert_eq!(draper.menu, Some(MenuAt::Pin(1)));
    draper.pins_shifted(&PinShift::between(&pr.pins[1..], &pr.pins[..1]), &mut doc);
    assert_eq!(draper.menu, None, "its pin is gone");
    draper.menu = Some(MenuAt::Fabric(pr.pins[0]));
    draper.pins_shifted(&PinShift::none_kept(), &mut doc);
    assert_eq!(
        draper.menu,
        Some(MenuAt::Fabric(pr.pins[0])),
        "Pin here is for a spot of fabric, not for a pin"
    );
}

fn wait_for(what: &str, mut cond: impl FnMut() -> bool) {
    let start = Instant::now();
    while !cond() {
        assert!(
            start.elapsed() < Duration::from_secs(20),
            "timed out waiting for {what}"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
}

#[test]
fn a_grab_through_the_handler_pulls_the_draping_fabric() {
    let mut pr = front();
    // Held up by two pins at its top corners, so it hangs where the camera looks.
    let corner = |x: f64| Pin {
        shape: PieceId(1),
        half: if x < 0.0 { Half::Pale } else { Half::Drawn },
        at: Point2::new(x.abs(), 400.0),
        target: spot(x, 400.0).0.to_array(),
    };
    pr.pins = vec![corner(-300.0), corner(300.0)];
    let runner = SimRunner::start(Stage::shared(), || {});
    runner.play(Arc::new(pr.clone()));
    wait_for("a frame", || runner.latest().is_some_and(|f| f.time > 0.1));
    let frame = runner.latest().unwrap();
    let mut doc = Document::new(pr, None);
    let mut draper = Draper::default();
    let (_, on_screen) = spot(0.0, 100.0);
    let to = on_screen + DVec2::new(0.0, -60.0); // pulled up
    let mut pulls: Vec<Pull> = draper
        .press(&camera(), &frame, &mut doc, on_screen)
        .into_iter()
        .collect();
    pulls.extend(draper.drag_to(&camera(), &mut doc, to));
    let Some(Pull::To(target)) = pulls.last().cloned() else {
        panic!("{pulls:?}");
    };
    let Pull::Grab {
        fabric,
        triangle,
        bary,
        ..
    } = pulls[0].clone()
    else {
        panic!("{pulls:?}");
    };
    runner.grab(fabric, triangle, bary, target);
    let held = |f: &SimFrame| -> DVec3 {
        let t = f.triangles[triangle];
        (0..3)
            .map(|k| f.positions[t[k] as usize].as_dvec3() * bary[k])
            .sum()
    };
    wait_for("the fabric to follow", || {
        runner
            .latest()
            .is_some_and(|f| (held(&f) - target).length() < 0.01)
    });
    runner.release();
}
