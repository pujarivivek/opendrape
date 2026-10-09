//! Helpers shared by the pattern-window tests: clicks, drags and typing given in pattern
//! millimetres. Each test file uses a different subset of them.
#![allow(dead_code)]

use egui::{Event, Key, Modifiers, PointerButton, Pos2, accesskit::Role, vec2};
use egui_kittest::{Harness, kittest::Queryable};
use opendrape::editor::PatternEditor;
use opendrape_core::{Piece, PieceId, Point2};

pub type H = Harness<'static, PatternEditor>;

pub fn harness() -> H {
    let mut h = Harness::builder()
        .with_size(vec2(1100.0, 750.0))
        .build_ui_state(|ui, ed: &mut PatternEditor| ed.ui(ui), PatternEditor::new());
    h.run();
    h
}

/// Screen position of the pattern point (x, y) mm.
pub fn at(h: &H, x: f64, y: f64) -> Pos2 {
    let ed = h.state();
    ed.view.to_screen(ed.canvas_rect, Point2::new(x, y))
}

pub fn button(h: &H, pos: Pos2, pressed: bool, modifiers: Modifiers) {
    h.event(Event::PointerButton {
        pos,
        button: PointerButton::Primary,
        pressed,
        modifiers,
    });
}

pub fn click(h: &mut H, x: f64, y: f64) {
    let p = at(h, x, y);
    h.hover_at(p);
    button(h, p, true, Modifiers::NONE);
    button(h, p, false, Modifiers::NONE);
    h.run();
}

pub fn shift_click(h: &mut H, x: f64, y: f64) {
    let p = at(h, x, y);
    h.hover_at(p);
    h.event(Event::ModifiersChanged(Modifiers::SHIFT));
    button(h, p, true, Modifiers::SHIFT);
    button(h, p, false, Modifiers::SHIFT);
    h.event(Event::ModifiersChanged(Modifiers::NONE));
    h.run();
}

/// Press at `from`, move there in steps, release at `to` (all in mm).
pub fn drag(h: &mut H, from: (f64, f64), to: (f64, f64)) {
    let (a, b) = (at(h, from.0, from.1), at(h, to.0, to.1));
    h.hover_at(a);
    button(h, a, true, Modifiers::NONE);
    for i in 1..=5 {
        h.hover_at(a + (b - a) * (i as f32 / 5.0));
    }
    button(h, b, false, Modifiers::NONE);
    h.run();
}

/// Press at the first of `path` (all in mm), move through the rest one at a time, and release
/// at the last.
pub fn drag_through(h: &mut H, path: &[(f64, f64)]) {
    let points: Vec<Pos2> = path.iter().map(|&(x, y)| at(h, x, y)).collect();
    h.hover_at(points[0]);
    button(h, points[0], true, Modifiers::NONE);
    for &p in &points[1..] {
        h.hover_at(p);
    }
    button(h, *points.last().unwrap(), false, Modifiers::NONE);
    h.run();
}

pub fn key(h: &mut H, k: Key) {
    h.key_press(k);
    h.run();
}

pub fn cmd(h: &mut H, k: Key) {
    h.key_press_modifiers(Modifiers::COMMAND, k);
    h.run();
}

/// Within 0.05 mm (clicks pass through f32 screen coordinates).
pub fn close(a: Point2, b: Point2) {
    assert!(a.distance(b) < 0.05, "{a:?} vs {b:?}");
}

/// Types `first` over the canvas, which opens the number box.
pub fn type_number(h: &mut H, first: &str) {
    h.event(Event::Text(first.into()));
    h.run();
}

/// Adds a 300 × 400 mm rectangle with its lower-left corner at (100, 100), as if drawn.
pub fn with_rectangle(h: &mut H) -> PieceId {
    let id = h.state_mut().doc.edit(|p| {
        p.add_piece(Piece::rectangle(
            PieceId(0),
            "Front",
            Point2::new(100.0, 100.0),
            300.0,
            400.0,
        ))
    });
    h.run();
    id
}

pub const SQUARE: [(f64, f64); 4] = [
    (100.0, 100.0),
    (400.0, 100.0),
    (400.0, 500.0),
    (100.0, 500.0),
];

pub fn piece_of(h: &H, id: PieceId) -> Piece {
    h.state().doc.project().piece(id).unwrap().clone()
}

pub fn field_text(h: &H, label: &str) -> String {
    h.get_by_role_and_label(Role::TextInput, label)
        .value()
        .unwrap_or_default()
}

/// Clicks the property field `label`, replaces its text with `text` and presses Enter.
pub fn type_into(h: &mut H, label: &str, text: &str) {
    h.get_by_role_and_label(Role::TextInput, label).click();
    h.run();
    h.get_by_role_and_label(Role::TextInput, label)
        .type_text(text);
    h.run();
    key(h, Key::Enter);
}

pub fn untouched_rectangle(id: PieceId) -> Piece {
    Piece::rectangle(id, "Front", Point2::new(100.0, 100.0), 300.0, 400.0)
}

/// The notice shown is the one saying a change was refused.
pub fn refused_notice(h: &H) -> bool {
    h.state()
        .notice
        .as_deref()
        .is_some_and(|n| n.contains("can't be made"))
}

/// Ends the whole test run, rather than hanging it for good, if the test that holds the
/// returned sender (until the end of the test) is not done within `seconds`.
pub fn watchdog(seconds: u64) -> std::sync::mpsc::Sender<()> {
    use std::sync::mpsc::{RecvTimeoutError, channel};
    let (done, wait) = channel::<()>();
    std::thread::spawn(move || {
        if wait.recv_timeout(std::time::Duration::from_secs(seconds))
            == Err(RecvTimeoutError::Timeout)
        {
            eprintln!("a test hung for {seconds} s: aborting");
            std::process::abort();
        }
    });
    done
}

/// Shows the pattern at `zoom` screen points per mm with pattern point (x, y) mm in the middle
/// of the canvas.
pub fn view_at(h: &mut H, x: f64, y: f64, zoom: f64) {
    let ed = h.state_mut();
    ed.view.zoom = zoom;
    ed.view.pan = ed.canvas_rect.size() * 0.5 - vec2((x * zoom) as f32, (-y * zoom) as f32);
}

/// Zooms right in (the most the pattern table allows) on pattern point (x, y) mm.
pub fn zoom_in_on(h: &mut H, x: f64, y: f64) {
    view_at(h, x, y, 50.0);
}
