use crate::arrange::{ArrangedScene, Arranger, SceneCache, ScreenCamera};
use crate::diagnostics::Diagnostics;
use crate::draping::{Draper, MenuAt, Pull};
use crate::editor::{self, PatternEditor};
use crate::file_dialogs::{DialogKind, FileDialogs};
use crate::gpu::{Decision, GpuChoice, GpuState, Os, StateStore, confirmed_state};
use crate::icons::{self, ph};
use crate::recovery::Recovery;
use crate::sim_runner::{DrapeNote, SimFrame, SimRunner};
use crate::tr;
use crate::viewport::{Show, Viewport};
use crate::workspace::{self, Workspace};
use egui::{Key, KeyboardShortcut, Modifiers, ViewportCommand};
use opendrape_core::{PieceId, Project};
use opendrape_drape::Stage;
use opendrape_mesh::MeshNote;
use opendrape_mesh::place::PlaceAt;
use opendrape_render::OrbitCamera;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::mpsc::{Receiver, TryRecvError};
use std::{cell::Cell, rc::Rc};

/// What main() decided before the window opened.
#[derive(Clone, Debug)]
pub struct Startup {
    pub decision: Decision,
    pub previous: GpuState,
    pub store: StateStore,
    pub smoke_test: bool,
    /// Where Open and Save get file names: the system dialogs, or a script in tests.
    pub file_dialogs: FileDialogs,
    /// Where unsaved work is kept when quitting can't ask first.
    pub recovery: Recovery,
}

/// Results main() reads after the window closes.
#[derive(Debug, Default)]
pub struct Shared {
    /// The user picked another graphics mode: start a fresh process.
    pub restart_with: Cell<Option<GpuChoice>>,
    /// The GPU drew at least one 3D frame.
    pub first_frame_drawn: Cell<bool>,
}

pub type SharedState = Rc<Shared>;

/// Completed frames before a graphics mode counts as working (those frames have been presented by then).
const CONFIRM_AFTER_FRAMES: u64 = 3;

const NEW: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::N);
const OPEN: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::O);
const SAVE: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::S);
const QUIT: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::Q);
const SAVE_AS: KeyboardShortcut = KeyboardShortcut::new(
    Modifiers {
        shift: true,
        ..Modifiers::COMMAND
    },
    Key::S,
);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum FileAction {
    New,
    Open,
    Save,
    SaveAs,
    /// Close the window.
    Quit,
    /// Close the window and start OpenDrape again in this graphics mode.
    Restart(GpuChoice),
}

/// What to do once unsaved changes have been dealt with.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Then {
    NewProject,
    OpenFile,
    /// Close the window; with a graphics mode, OpenDrape then starts again in that mode.
    Quit(Option<GpuChoice>),
}

#[derive(Clone, Copy, Debug)]
enum DialogFor {
    Open,
    SaveAs(Option<Then>),
}

/// A question waiting for the user; at most one at a time.
enum Pending {
    /// "Save your changes?", asked before `Then`.
    AskToSave(Then),
    /// A file dialog is open; its answer arrives on the channel.
    Dialog(DialogFor, Receiver<Option<PathBuf>>),
}

#[derive(Clone, Copy)]
enum Answer {
    Save,
    Discard,
    Cancel,
}

pub struct OpenDrapeApp {
    viewport: Option<Viewport>,
    diagnostics: Diagnostics,
    startup: Startup,
    shared: SharedState,
    show_about: bool,
    copied: bool,
    runner: Option<SimRunner>,
    fps: f32,
    editor: PatternEditor,
    /// The form, shared with the simulation thread.
    stage: Arc<Stage>,
    /// The project the drape was last given (at Play, or since by an edit while draping).
    draped: Option<Arc<Project>>,
    /// The pieces as the 3D view shows them while arranging.
    arranged: SceneCache,
    /// What the pointer does in the 3D view while arranging.
    arranger: Arranger,
    /// What the pointer does in the 3D view while draping.
    draper: Draper,
    /// The 3D view's camera as last drawn.
    view_camera: Option<ScreenCamera>,
    /// The piece the 3D view's Place at… menu was opened on.
    menu_for: Option<PieceId>,
    pending: Option<Pending>,
    /// An action requested from code rather than the menu or keyboard; handled on the next frame.
    queued: Option<FileAction>,
    /// Shown in a message box after a failed open or save.
    error: Option<String>,
    /// The user chose to quit without saving: let the window close.
    closing: bool,
    /// The window title last sent, so it is sent only when it changes.
    title: String,
    /// Where unsaved work is kept when quitting can't ask first.
    recovery: Recovery,
    /// Work a quit without asking left behind, and the file it came from: waiting for the
    /// student to restore or discard it.
    offered: Option<(Project, Option<PathBuf>)>,
    /// The workspace tab open; screen state only, never saved or undone.
    workspace: Workspace,
}

impl OpenDrapeApp {
    pub fn new(cc: &eframe::CreationContext<'_>, startup: Startup, shared: SharedState) -> Self {
        crate::theme::install(&cc.egui_ctx);
        let render_state = cc.wgpu_render_state.as_ref();
        let info = render_state.map(|rs| rs.adapter.get_info());
        crate::startup_log::stage(format_args!(
            "window open, graphics: {:?}",
            info.as_ref().map(|i| (&i.name, i.device_type, i.backend))
        ));
        let stage = Stage::shared();
        let runner = render_state.map(|_| {
            let ctx = cc.egui_ctx.clone();
            SimRunner::start(stage.clone(), move || ctx.request_repaint())
        });
        // Place at… needs the form; there is none without a 3D view.
        let mut editor = PatternEditor::new();
        editor.stage = render_state.map(|_| stage.clone());
        // Read before `startup` moves into the app below.
        let recovery = startup.recovery.clone();
        let offered = recovery.take();
        Self {
            viewport: render_state.map(|rs| Viewport::new(rs, &stage)),
            diagnostics: Diagnostics::collect(info.as_ref(), startup.decision),
            startup,
            shared,
            show_about: false,
            copied: false,
            runner,
            fps: 0.0,
            editor,
            stage,
            draped: None,
            arranged: SceneCache::default(),
            arranger: Arranger::default(),
            draper: Draper::default(),
            view_camera: None,
            menu_for: None,
            pending: None,
            queued: None,
            error: None,
            closing: false,
            title: String::new(),
            recovery,
            offered,
            workspace: Workspace::default(),
        }
    }

    /// The workspace tab open.
    pub fn workspace(&self) -> Workspace {
        self.workspace
    }

    pub fn set_workspace(&mut self, workspace: Workspace) {
        self.workspace = workspace;
    }

    pub fn editor(&self) -> &PatternEditor {
        &self.editor
    }

    pub fn editor_mut(&mut self) -> &mut PatternEditor {
        &mut self.editor
    }

    /// The user agreed to close the window (after dealing with unsaved changes).
    pub fn is_closing(&self) -> bool {
        self.closing
    }

    /// "skirt.odp — OpenDrape", with a leading "•" while there are unsaved changes.
    pub fn window_title(&self) -> String {
        let name = self.document_name();
        if self.editor.doc.is_dirty() {
            tr!("window-title-unsaved", name = name)
        } else {
            tr!("window-title", name = name)
        }
    }

    fn document_name(&self) -> String {
        self.editor
            .doc
            .path
            .as_deref()
            .and_then(Path::file_name)
            .map_or_else(
                || tr!("file-untitled"),
                |n| n.to_string_lossy().into_owned(),
            )
    }

    /// The latest frame of the drape; None while arranging.
    pub fn sim_frame(&self) -> Option<Arc<SimFrame>> {
        self.runner.as_ref().and_then(SimRunner::latest)
    }

    /// Between Play and Reset.
    pub fn is_draping(&self) -> bool {
        self.runner.as_ref().is_some_and(SimRunner::is_draping)
    }

    /// The form garments are arranged round and draped on.
    pub fn stage(&self) -> &Arc<Stage> {
        &self.stage
    }

    /// The pieces as the 3D view shows them while arranging.
    pub fn arranged_scene(&mut self) -> Rc<ArrangedScene> {
        self.arranged
            .scene(self.editor.doc.project(), self.stage.shoulder_y())
    }

    /// What the pointer does in the 3D view: the gizmo handle under it, a drag in progress.
    pub fn arranger(&self) -> &Arranger {
        &self.arranger
    }

    /// What the pointer does in the 3D view while draping: a grab or pin move in progress.
    pub fn draper(&self) -> &Draper {
        &self.draper
    }

    /// The 3D view's camera and rectangle as last drawn.
    pub fn view_camera(&self) -> Option<ScreenCamera> {
        self.view_camera
    }

    /// The 3D view's orbit camera.
    pub fn orbit_camera(&self) -> Option<OrbitCamera> {
        self.viewport.as_ref().map(|v| *v.camera())
    }

    /// `fps` is `None` while paused: the window then only redraws on input.
    pub fn stats_text(fps: Option<f32>, step_ms: f64, points: usize) -> String {
        let (ms, points) = (format!("{step_ms:.1}"), points.to_string());
        match fps {
            Some(fps) => tr!(
                "overlay-stats",
                fps = format!("{fps:.0}"),
                ms = ms,
                points = points
            ),
            None => tr!("overlay-stats-paused", ms = ms, points = points),
        }
    }

    fn toolbar(&mut self, ui: &mut egui::Ui) {
        let Some(runner) = &self.runner else { return };
        let (draping, playing) = (runner.is_draping(), runner.is_playing());
        let mut clicked = None;
        ui.horizontal_wrapped(|ui| {
            let (icon, label, tip) = if draping && playing {
                (ph::PAUSE, tr!("toolbar-pause"), tr!("toolbar-pause-tip"))
            } else {
                (ph::PLAY, tr!("toolbar-play"), tr!("toolbar-play-tip"))
            };
            if icons::icon_button(ui, icon, &label, &tip, false, true).clicked() {
                clicked = Some(match (draping, playing) {
                    (false, _) => Toolbar::Play,
                    (true, true) => Toolbar::Pause,
                    (true, false) => Toolbar::Resume,
                });
            }
            let (label, tip) = (tr!("toolbar-reset"), tr!("toolbar-reset-tip"));
            let reset = ph::ARROW_COUNTER_CLOCKWISE;
            if icons::icon_button(ui, reset, &label, &tip, false, draping).clicked() {
                clicked = Some(Toolbar::Reset);
            }
            ui.separator();
            for (side, label) in [
                (PlaceAt::Front, tr!("view-front")),
                (PlaceAt::Back, tr!("view-back")),
                (PlaceAt::LeftSide, tr!("view-left")),
                (PlaceAt::RightSide, tr!("view-right")),
            ] {
                if ui.button(label).clicked() {
                    clicked = Some(Toolbar::Look(side));
                }
            }
        });
        match clicked {
            Some(Toolbar::Play) => {
                let snapshot = Arc::new(self.editor.doc.project().clone());
                runner.play(snapshot.clone());
                self.draped = Some(snapshot);
            }
            Some(Toolbar::Pause) => runner.set_playing(false),
            Some(Toolbar::Resume) => runner.set_playing(true),
            Some(Toolbar::Reset) => {
                runner.reset();
                self.draped = None;
            }
            Some(Toolbar::Look(side)) => {
                if let Some(v) = &mut self.viewport {
                    v.look_from(side.angle());
                }
            }
            None => {}
        }
    }

    /// A change to the project while draping (a pattern edit, a seam, a pin, an undo or a
    /// redo) carries the drape on with it: see [`SimRunner::update`].
    fn update_if_edited(&mut self) {
        let Some(runner) = &self.runner else { return };
        if !runner.is_draping() {
            self.draped = None;
            return;
        }
        let project = self.editor.doc.project();
        if self.draped.as_ref().is_some_and(|d| **d != *project) {
            let snapshot = Arc::new(project.clone());
            runner.update(snapshot.clone());
            self.draped = Some(snapshot);
        }
    }

    /// The hint while draping, and what the student should know about the drape.
    fn notes(&self, ui: &mut egui::Ui) {
        let warn = ui.visuals().warn_fg_color;
        if self.arranged.view_failed() {
            ui.colored_label(warn, tr!("note-view-failed"));
        }
        let Some(runner) = &self.runner else { return };
        if runner.went_wrong() {
            ui.colored_label(warn, tr!("note-went-wrong"));
        }
        if runner.is_draping() {
            ui.label(tr!("hint-draping"));
            ui.label(tr!("hint-pinning"));
        }
        if let Some(frame) = runner.latest() {
            for note in frame.notes.iter().take(MAX_NOTES_SHOWN) {
                ui.colored_label(warn, note_text(note, self.editor.doc.project()));
            }
            // A big pattern can have many: the 3D view must not shrink to nothing under them.
            let more = frame.notes.len().saturating_sub(MAX_NOTES_SHOWN);
            if more > 0 {
                ui.colored_label(warn, tr!("note-more", count = more));
            }
        }
    }

    /// The 3D view: its toolbar and notes, the form with the pieces being arranged or the
    /// drape, and the speed overlay.
    fn view_3d(&mut self, ui: &mut egui::Ui, frame: &eframe::Frame) {
        // A pin taken away (or brought back by an undo) renumbers the others: a pin being
        // moved here and the menu on a pin follow theirs.
        for shift in self.editor.take_pin_shifts() {
            self.draper.pins_shifted(&shift, &mut self.editor.doc);
        }
        self.update_if_edited();
        // Kept up to date while draping too: the project doesn't change then, so it costs
        // nothing, and Reset shows the pieces at once. Made before the notes are shown, so that
        // a view that couldn't be made says so on the frame the pattern changed.
        let scene = (self.viewport.is_some() && frame.wgpu_render_state().is_some()).then(|| {
            self.arranged
                .scene(self.editor.doc.project(), self.stage.shoulder_y())
        });
        self.toolbar(ui);
        self.notes(ui);
        ui.separator();
        // From the moment Play is pressed, not from the first frame of the drape: the drape is
        // made from the pieces as they are then.
        let draping = self.is_draping();
        self.editor.draping = draping;
        let (Some(viewport), Some(rs), Some(scene)) =
            (self.viewport.as_mut(), frame.wgpu_render_state(), scene)
        else {
            ui.centered_and_justified(|ui| ui.label(tr!("viewport-no-gpu")));
            return;
        };
        let sim = self.runner.as_ref().and_then(SimRunner::latest);
        let fps = self
            .runner
            .as_ref()
            .is_some_and(SimRunner::is_playing)
            .then_some(self.fps);
        let rect = ui.available_rect_before_wrap();
        // Arranging until the drape's first frame arrives.
        let show = match &sim {
            Some(f) => Show::Drape(f),
            None => Show::Pieces {
                scene: &scene,
                selected: self.editor.selection.piece(),
            },
        };
        let drawn = viewport.ui(ui, rs, show);
        match (&drawn, &sim) {
            (Some(drawn), _) if !draping => {
                // The drape ended (Reset, or it went wrong) under a held grab or pin move: the
                // press is over, and the camera is free at once.
                self.stop_pulling();
                self.arrange(ui, &drawn.response, &drawn.camera, &scene)
            }
            (Some(drawn), Some(frame)) => {
                self.stop_arranging();
                self.drape_input(ui, &drawn.response, &drawn.camera, frame);
            }
            _ => {
                // Nothing to arrange or pull (no view, or the drape's fabric is being made): a
                // drag still held ends where it is.
                self.stop_arranging();
                self.stop_pulling();
            }
        }
        if let Some(drawn) = &drawn {
            self.view_camera = Some(drawn.camera);
            if let Some(viewport) = self.viewport.as_mut() {
                // A drag that didn't grab the gizmo turns the camera; while one has, the camera
                // stays put (the handle is held at a screen point), scroll-zoom included.
                let grabbed = self.arranger.is_dragging() || self.draper.is_dragging();
                let drag = drawn.response.drag_delta();
                if drag != egui::Vec2::ZERO && !grabbed {
                    viewport.camera_mut().drag(drag.x, drag.y);
                }
                if drawn.response.hovered() && !grabbed {
                    let scroll = ui.input(|i| i.smooth_scroll_delta.y);
                    if scroll != 0.0 {
                        viewport.camera_mut().zoom(scroll);
                    }
                }
            }
        }
        if let Some(f) = &sim {
            ui.painter().text(
                rect.left_top() + egui::vec2(10.0, 8.0),
                egui::Align2::LEFT_TOP,
                Self::stats_text(fps, f.step_ms, f.positions.len()),
                egui::FontId::proportional(13.0),
                crate::theme::STATS_TEXT,
            );
        }
    }

    /// No arranging now: a gizmo drag still held ends where it is, and nothing is lit.
    fn stop_arranging(&mut self) {
        self.arranger.release(&mut self.editor.doc);
        self.arranger.released();
        self.arranger.hovered = None;
    }

    /// No pulling now: a grab lets go, and a pin being moved stays where it is (one step).
    fn stop_pulling(&mut self) {
        let pull = self.draper.release(&mut self.editor.doc);
        self.send(pull.into_iter().collect());
        self.draper.menu = None;
    }

    /// Passes what a grab asks for on to the simulation.
    fn send(&self, pulls: Vec<Pull>) {
        let Some(runner) = &self.runner else { return };
        for pull in pulls {
            match pull {
                Pull::Grab {
                    fabric,
                    triangle,
                    bary,
                    target,
                } => runner.grab(fabric, triangle, bary, target),
                Pull::To(target) => runner.pull(target),
                Pull::Release => runner.release(),
            }
        }
    }

    /// The pointer in the 3D view while draping: a press on the fabric pulls it, a pin's marker
    /// is dragged to move it, right-clicks offer Pin here and Remove pin, and the pins are drawn.
    fn drape_input(
        &mut self,
        ui: &egui::Ui,
        response: &egui::Response,
        cam: &ScreenCamera,
        frame: &SimFrame,
    ) {
        let at = |p: egui::Pos2| glam::DVec2::new(f64::from(p.x), f64::from(p.y));
        let editor = &mut self.editor;
        let mut pulls = Vec::new();
        // Esc gives a pull or a pin move up: the pin goes back, with no undo step. The pattern
        // window did not hear it (see `ui`).
        if self.draper.is_dragging() && ui.input(|i| i.key_pressed(egui::Key::Escape)) {
            pulls.extend(self.draper.cancel(&mut editor.doc));
        }
        if response.drag_started_by(egui::PointerButton::Primary)
            && let Some(p) = ui.input(|i| i.pointer.press_origin())
        {
            pulls.extend(self.draper.press(cam, frame, &mut editor.doc, at(p)));
        }
        if self.draper.is_dragging()
            && let Some(p) = response.interact_pointer_pos()
        {
            pulls.extend(self.draper.drag_to(cam, &mut editor.doc, at(p)));
        }
        if response.drag_stopped() {
            pulls.extend(self.draper.release(&mut editor.doc));
        }
        if response.clicked()
            && let Some(p) = response.interact_pointer_pos()
        {
            self.draper
                .click(cam, editor.doc.project(), &mut editor.selection, at(p));
        }
        if response.secondary_clicked()
            && let Some(p) = response.interact_pointer_pos()
        {
            self.draper
                .secondary_click(cam, frame, editor.doc.project(), at(p));
        }
        if let Some(menu) = self.draper.menu {
            response.context_menu(|ui| match menu {
                MenuAt::Fabric(pin) => {
                    if ui.button(tr!("menu-pin-here")).clicked() {
                        editor.add_pin(pin);
                        ui.close();
                    }
                }
                MenuAt::Pin(k) => {
                    if ui.button(tr!("panel-remove-pin")).clicked() {
                        editor.remove_pin(k);
                        ui.close();
                    }
                }
            });
        }
        let selected = match editor.selection {
            editor::Selection::Pin(k) => Some(k),
            _ => None,
        };
        let painter = ui.painter_at(response.rect);
        crate::arrange::overlay::paint_pins(&painter, cam, editor.doc.project(), selected);
        self.send(pulls);
    }

    /// The pointer in the 3D view while arranging: clicks pick pieces, the selected piece's
    /// gizmo moves and turns it, and the gizmo is drawn over the view.
    fn arrange(
        &mut self,
        ui: &egui::Ui,
        response: &egui::Response,
        cam: &ScreenCamera,
        scene: &ArrangedScene,
    ) {
        let at = |p: egui::Pos2| glam::DVec2::new(f64::from(p.x), f64::from(p.y));
        let shift = ui.input(|i| i.modifiers.shift);
        let editor = &mut self.editor;
        // Esc gives a gizmo drag up: the piece goes back, with no undo step.
        if self.arranger.is_dragging() && ui.input(|i| i.key_pressed(egui::Key::Escape)) {
            self.arranger.cancel(&mut editor.doc);
        }
        // Where a button went down in the view: a click is judged by that as well as by where
        // the button comes up (egui no longer says, by then). A press and its release in one
        // frame leave no trace of the press; the release stands for it.
        if ui.input(|i| i.pointer.any_pressed())
            && response.is_pointer_button_down_on()
            && let Some(p) = ui.input(|i| i.pointer.press_origin())
        {
            self.arranger.pressed(at(p));
        }
        if response.drag_started_by(egui::PointerButton::Primary)
            && let Some(p) = ui.input(|i| i.pointer.press_origin())
        {
            self.arranger
                .press(cam, scene, &editor.selection, &mut editor.doc, at(p));
        }
        if self.arranger.is_dragging()
            && let Some(p) = response.interact_pointer_pos()
            && self.arranger.drag_to(cam, &mut editor.doc, at(p), shift)
        {
            // The first move of this drag the project refused.
            editor.notice = Some(tr!("notice-refused"));
        }
        if response.drag_stopped() {
            self.arranger.release(&mut editor.doc);
        }
        if response.clicked()
            && let Some(p) = response.interact_pointer_pos()
        {
            self.arranger
                .click(cam, scene, &mut editor.selection, at(p));
        }
        // A right-click on a piece selects it and opens Place at… for it.
        if response.secondary_clicked()
            && let Some(p) = response.interact_pointer_pos()
        {
            self.arranger
                .click(cam, scene, &mut editor.selection, at(p));
            self.menu_for = editor.selection.piece();
        }
        if ui.input(|i| i.pointer.any_released()) {
            self.arranger.released();
        }
        if let Some(id) = self.menu_for {
            response.context_menu(|ui| editor.place_menu(ui, id));
        }
        if !self.arranger.is_dragging() {
            // Off the view, nothing is under the pointer.
            let over = response.hover_pos().map(at);
            self.arranger.hover(cam, scene, &editor.selection, over);
        }
        // Drawn where the piece is now, after this frame's drag.
        let scene = self
            .arranged
            .scene(self.editor.doc.project(), self.stage.shoulder_y());
        let painter = ui.painter_at(response.rect);
        if let Some(gizmo) = Arranger::gizmo(cam, &scene, &self.editor.selection) {
            let lit = self.arranger.active().or(self.arranger.hovered);
            crate::arrange::overlay::paint(&painter, cam, &gizmo, lit);
        }
        if let (Some(text), Some(p)) = (
            self.arranger.readout(self.editor.doc.project().units),
            response.interact_pointer_pos(),
        ) {
            crate::arrange::overlay::paint_readout(&painter, p, text);
        }
    }

    pub fn viewport_frames(&self) -> u64 {
        self.viewport.as_ref().map_or(0, |v| v.frames_drawn)
    }

    /// Save `choice` as the preferred graphics mode and ask main() to restart.
    pub fn request_graphics_change(&mut self, choice: GpuChoice) {
        self.startup.store.save(&GpuState {
            preferred: choice,
            pending: None,
        });
        self.shared.restart_with.set(Some(choice));
    }

    /// What Help → Graphics does: restart in `choice`, after dealing with unsaved changes.
    pub fn choose_graphics(&mut self, choice: GpuChoice) {
        self.queued = Some(FileAction::Restart(choice));
    }

    /// Frames presented to the screen are proof this graphics mode works: clear the crash
    /// marker. egui presents a frame only after `ui()` returns, and some drivers crash on
    /// their first present, so wait until [`CONFIRM_AFTER_FRAMES`] frames have completed.
    /// (Counted with egui's frame number: `ui()` can run more than once per frame.)
    fn confirm_first_frame(&mut self, ctx: &egui::Context) {
        if self.shared.first_frame_drawn.get() {
            return;
        }
        crate::startup_log::stage(format_args!(
            "frame {}, 3D frames drawn: {}",
            ctx.cumulative_frame_nr(),
            self.viewport_frames()
        ));
        if self.viewport_frames() == 0 {
            return;
        }
        if ctx.cumulative_frame_nr() < CONFIRM_AFTER_FRAMES {
            ctx.request_repaint();
            return;
        }
        self.shared.first_frame_drawn.set(true);
        if let Some(state) = confirmed_state(self.startup.decision) {
            self.startup.store.save(&state);
        }
        if self.startup.smoke_test {
            ctx.send_viewport_cmd(ViewportCommand::Close);
        }
    }

    /// The menus, then the workspace tabs. Returns the file action chosen and the workspace
    /// picked (from the View menu or a tab).
    fn menu_bar(&mut self, ui: &mut egui::Ui) -> (Option<FileAction>, Option<Workspace>) {
        let mut action = None;
        let mut picked = None;
        egui::MenuBar::new().ui(ui, |ui| {
            ui.menu_button(tr!("menu-file"), |ui| {
                let items = [
                    (tr!("menu-new"), NEW, FileAction::New),
                    (tr!("menu-open"), OPEN, FileAction::Open),
                    (tr!("menu-save"), SAVE, FileAction::Save),
                    (tr!("menu-save-as"), SAVE_AS, FileAction::SaveAs),
                ];
                for (label, shortcut, item) in items {
                    if menu_item(ui, true, label, &shortcut) {
                        action = Some(item);
                        ui.close();
                    }
                }
                ui.separator();
                if menu_item(ui, true, tr!("menu-quit"), &QUIT) {
                    action = Some(FileAction::Quit);
                    ui.close();
                }
            });
            ui.menu_button(tr!("menu-edit"), |ui| {
                if menu_item(ui, self.editor.can_undo(), tr!("menu-undo"), &editor::UNDO) {
                    self.editor.undo();
                    ui.close();
                }
                if menu_item(ui, self.editor.can_redo(), tr!("menu-redo"), &editor::REDO) {
                    self.editor.redo();
                    ui.close();
                }
            });
            ui.menu_button(tr!("menu-view"), |ui| {
                for ws in Workspace::ALL {
                    if menu_item(ui, true, ws.label(), &ws.shortcut()) {
                        picked = Some(ws);
                        ui.close();
                    }
                }
            });
            ui.menu_button(tr!("menu-help"), |ui| {
                if ui.button(tr!("menu-about")).clicked() {
                    self.show_about = true;
                    self.copied = false;
                    ui.close();
                }
                ui.menu_button(tr!("menu-graphics"), |ui| {
                    for &choice in GpuChoice::available(Os::current()) {
                        let current = self.startup.decision.choice == choice;
                        if ui.radio(current, choice_label(choice)).clicked() && !current {
                            self.choose_graphics(choice);
                        }
                    }
                    ui.separator();
                    ui.label(tr!("graphics-restart-note"));
                });
            });
            if let Some(ws) = workspace::tabs(ui, self.workspace) {
                picked = Some(ws);
            }
        });
        (action, picked)
    }

    /// Cmd+1…5 (Ctrl on Windows). Not while a text field or the number box has the keyboard, a
    /// question or message box is open, or the mouse is held (a drag on the pattern table or
    /// in the 3D view must end where it started).
    fn workspace_shortcut(&self, ctx: &egui::Context) -> Option<Workspace> {
        let busy = ctx.text_edit_focused()
            || self.editor.length_box_open()
            || self.pending.is_some()
            || self.error.is_some()
            || self.offered.is_some()
            || self.arranger.is_dragging()
            || self.draper.is_dragging()
            || ctx.input(|i| i.pointer.any_down());
        if busy {
            return None;
        }
        Workspace::ALL
            .into_iter()
            .find(|ws| ctx.input_mut(|i| i.consume_shortcut(&ws.shortcut())))
    }

    /// Undo and Redo from the keyboard in the workspaces that don't show the pattern table
    /// (which has its own); the same keys, waiting while a text field has the keyboard.
    fn app_undo_redo(&mut self, ctx: &egui::Context) {
        if ctx.text_edit_focused() {
            return;
        }
        // Redo first: `consume_shortcut` also matches Cmd+Z while Shift is held.
        if ctx
            .input_mut(|i| i.consume_shortcut(&editor::REDO) || i.consume_shortcut(&editor::REDO_Y))
        {
            self.editor.redo();
        } else if ctx.input_mut(|i| i.consume_shortcut(&editor::UNDO)) {
            self.editor.undo();
        }
    }

    /// ⌘N, ⌘O, ⌘S, ⇧⌘S, ⌘Q (Ctrl on Windows). While a text field is being typed in, or the
    /// restore question is up, only ⌘Q works: it means nothing to a text field, and quitting
    /// must always be possible (the waiting copy stays for next time).
    fn file_shortcut(&self, ctx: &egui::Context) -> Option<FileAction> {
        if ctx.input_mut(|i| i.consume_shortcut(&QUIT)) {
            return Some(FileAction::Quit);
        }
        if ctx.text_edit_focused() || self.offered.is_some() {
            return None;
        }
        // Save As first: `consume_shortcut` also matches ⌘S while Shift is held.
        [
            (SAVE_AS, FileAction::SaveAs),
            (SAVE, FileAction::Save),
            (NEW, FileAction::New),
            (OPEN, FileAction::Open),
        ]
        .into_iter()
        .find(|(shortcut, _)| ctx.input_mut(|i| i.consume_shortcut(shortcut)))
        .map(|(_, action)| action)
    }

    fn file_action(&mut self, action: FileAction, frame: &eframe::Frame, ctx: &egui::Context) {
        if self.pending.is_some() {
            return; // a question or dialog is already open
        }
        match action {
            FileAction::New => self.after_saving_changes(Then::NewProject, frame, ctx),
            FileAction::Open => self.after_saving_changes(Then::OpenFile, frame, ctx),
            FileAction::Quit => self.after_saving_changes(Then::Quit(None), frame, ctx),
            FileAction::Restart(choice) => {
                self.after_saving_changes(Then::Quit(Some(choice)), frame, ctx)
            }
            FileAction::Save => self.save(None, frame, ctx),
            FileAction::SaveAs => {
                self.ask_file(DialogKind::Save, DialogFor::SaveAs(None), frame, ctx)
            }
        }
    }

    /// Starts over with `project` (File → New, File → Open, or work restored from a recovery
    /// copy, which stays unsaved): a fresh history, and a 3D view with nothing in it yet. The
    /// view keeps the pieces of the last pattern it could make when a pattern can't be made, so
    /// it must not keep those of the project that has just gone, or a file that fails to mesh
    /// would show the pieces of the one before it. A drape is of the project that has gone
    /// too: the 3D view is back to arranging, as it is after Reset, so that the new pieces
    /// start at their own placements and not from the old fabric.
    fn replace_project(&mut self, project: Project, path: Option<PathBuf>, recovered: bool) {
        // First, while a held grab or move still has the old project to end in.
        self.stop_pulling();
        self.stop_arranging();
        if let Some(runner) = &self.runner
            && runner.is_draping()
        {
            runner.reset();
        }
        self.draped = None;
        if recovered {
            self.editor.set_recovered(project, path);
        } else {
            self.editor.set_project(project, path);
        }
        self.arranged = SceneCache::default();
    }

    /// Runs `then`, first asking whether to save any unsaved changes.
    fn after_saving_changes(&mut self, then: Then, frame: &eframe::Frame, ctx: &egui::Context) {
        if self.editor.doc.is_dirty() {
            self.pending = Some(Pending::AskToSave(then));
        } else {
            self.run(then, frame, ctx);
        }
    }

    fn run(&mut self, then: Then, frame: &eframe::Frame, ctx: &egui::Context) {
        match then {
            Then::NewProject => self.replace_project(Project::new(), None, false),
            Then::OpenFile => self.ask_file(DialogKind::Open, DialogFor::Open, frame, ctx),
            Then::Quit(restart_in) => {
                if let Some(choice) = restart_in {
                    self.request_graphics_change(choice);
                }
                self.closing = true;
                ctx.send_viewport_cmd(ViewportCommand::Close);
            }
        }
    }

    /// Saves to the project's file, asking for a name the first time, then runs `then`.
    fn save(&mut self, then: Option<Then>, frame: &eframe::Frame, ctx: &egui::Context) {
        match self.editor.doc.path.clone() {
            Some(path) => {
                if self.write(&path)
                    && let Some(then) = then
                {
                    self.run(then, frame, ctx);
                }
            }
            None => self.ask_file(DialogKind::Save, DialogFor::SaveAs(then), frame, ctx),
        }
    }

    fn ask_file(
        &mut self,
        kind: DialogKind,
        purpose: DialogFor,
        frame: &eframe::Frame,
        ctx: &egui::Context,
    ) {
        let suggested = self
            .editor
            .doc
            .path
            .as_deref()
            .and_then(Path::file_name)
            .map_or_else(
                || format!("{}.{}", tr!("file-untitled"), opendrape_io::EXTENSION),
                |n| n.to_string_lossy().into_owned(),
            );
        let answer = self.startup.file_dialogs.ask(kind, &suggested, frame, ctx);
        self.pending = Some(Pending::Dialog(purpose, answer));
    }

    fn write(&mut self, path: &Path) -> bool {
        match opendrape_io::save(self.editor.doc.project(), path) {
            Ok(()) => {
                self.editor.doc.mark_saved(path.to_path_buf());
                true
            }
            Err(e) => {
                self.error = Some(tr!("error-save", error = e.to_string()));
                false
            }
        }
    }

    /// Acts on a file dialog's answer once it arrives.
    fn poll_dialog(&mut self, frame: &eframe::Frame, ctx: &egui::Context) {
        let Some(Pending::Dialog(purpose, answer)) = &self.pending else {
            return;
        };
        let answer = match answer.try_recv() {
            Ok(answer) => answer,
            Err(TryRecvError::Empty) => return,
            Err(TryRecvError::Disconnected) => None,
        };
        let purpose = *purpose;
        self.pending = None;
        let Some(path) = answer else { return }; // cancelled
        match purpose {
            DialogFor::Open => match opendrape_io::load(&path) {
                Ok(project) => self.replace_project(project, Some(path), false),
                Err(e) => self.error = Some(tr!("error-open", error = e.to_string())),
            },
            DialogFor::SaveAs(then) => {
                if self.write(&with_project_extension(path))
                    && let Some(then) = then
                {
                    self.run(then, frame, ctx);
                }
            }
        }
    }

    /// Closing the window with unsaved changes asks first.
    fn guard_close(&mut self, ctx: &egui::Context) {
        if ctx.input(|i| i.viewport().close_requested())
            && !self.closing
            && self.editor.doc.is_dirty()
        {
            ctx.send_viewport_cmd(ViewportCommand::CancelClose);
            if self.pending.is_none() {
                self.pending = Some(Pending::AskToSave(Then::Quit(None)));
            }
        }
    }

    fn unsaved_changes_modal(&mut self, frame: &eframe::Frame, ctx: &egui::Context) {
        let Some(Pending::AskToSave(then)) = &self.pending else {
            return;
        };
        let then = *then;
        let name = self.document_name();
        let mut answer = None;
        let modal = egui::Modal::new(egui::Id::new("unsaved_changes")).show(ctx, |ui| {
            ui.set_max_width(360.0);
            ui.heading(tr!("unsaved-title"));
            ui.label(tr!("unsaved-body", name = name));
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                if ui.button(tr!("unsaved-save")).clicked() {
                    answer = Some(Answer::Save);
                }
                if ui.button(tr!("unsaved-discard")).clicked() {
                    answer = Some(Answer::Discard);
                }
                if ui.button(tr!("unsaved-cancel")).clicked() {
                    answer = Some(Answer::Cancel);
                }
            });
        });
        if answer.is_none() && modal.should_close() {
            answer = Some(Answer::Cancel); // Escape, or a click outside the box
        }
        let Some(answer) = answer else { return };
        self.pending = None;
        match answer {
            Answer::Save => self.save(Some(then), frame, ctx),
            Answer::Discard => self.run(then, frame, ctx),
            Answer::Cancel => {}
        }
    }

    fn error_modal(&mut self, ctx: &egui::Context) {
        let Some(message) = self.error.clone() else {
            return;
        };
        let modal = egui::Modal::new(egui::Id::new("file_error")).show(ctx, |ui| {
            ui.set_max_width(360.0);
            ui.heading(tr!("error-title"));
            ui.label(message);
            ui.add_space(8.0);
            ui.button(tr!("error-ok")).clicked()
        });
        if modal.inner || modal.should_close() {
            self.error = None;
        }
    }

    /// Offers back the work a quit without asking left behind (see `crate::recovery`).
    fn recovery_modal(&mut self, ctx: &egui::Context) {
        if self.offered.is_none() || self.pending.is_some() {
            return;
        }
        let mut answer = None;
        egui::Modal::new(egui::Id::new("recovery")).show(ctx, |ui| {
            ui.set_max_width(360.0);
            ui.heading(tr!("recovery-title"));
            ui.label(tr!("recovery-body"));
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                if ui.button(tr!("recovery-restore")).clicked() {
                    answer = Some(true);
                }
                if ui.button(tr!("recovery-discard")).clicked() {
                    answer = Some(false);
                }
            });
        });
        let Some(restore) = answer else { return };
        if let (true, Some((project, from))) = (restore, self.offered.take()) {
            self.replace_project(project, from, true);
        }
        // `take` above cleared the offer, whichever the answer was.
        self.recovery.discard();
    }

    fn update_title(&mut self, ctx: &egui::Context) {
        let title = self.window_title();
        if title != self.title {
            ctx.send_viewport_cmd(ViewportCommand::Title(title.clone()));
            self.title = title;
        }
    }

    fn about_window(&mut self, ctx: &egui::Context) {
        let mut open = self.show_about;
        egui::Window::new(tr!("menu-about"))
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .show(ctx, |ui| {
                let d = &self.diagnostics;
                ui.heading(tr!("app-name"));
                ui.label(tr!("about-tagline"));
                ui.label(tr!("about-version", version = d.app_version.clone()));
                ui.label(tr!(
                    "about-graphics",
                    name = d.adapter.clone(),
                    backend = d.backend.clone()
                ));
                ui.label(tr!("about-license"));
                if ui.button(tr!("about-copy")).clicked() {
                    ctx.copy_text(d.to_text());
                    self.copied = true;
                }
                if self.copied {
                    ui.label(tr!("about-copied"));
                }
            });
        self.show_about = open;
    }
}

/// A menu item with its shortcut shown on the right; true when it was clicked. egui reads the
/// shortcut out as part of the item's name ("Save As… Ctrl+Shift+S"), so the name is set back to
/// the plain label and the shortcut is announced as the item's keyboard shortcut instead.
fn menu_item(ui: &mut egui::Ui, enabled: bool, label: String, shortcut: &KeyboardShortcut) -> bool {
    let shortcut = ui.ctx().format_shortcut(shortcut);
    let button = egui::Button::new(label.clone()).shortcut_text(shortcut.clone());
    let response = ui.add_enabled(enabled, button);
    ui.ctx().accesskit_node_builder(response.id, |node| {
        node.set_label(label);
        node.set_keyboard_shortcut(shortcut);
    });
    response.clicked()
}

/// `path`, with ".odp" added unless it already ends in it ("skirt.v2" becomes "skirt.v2.odp").
fn with_project_extension(path: PathBuf) -> PathBuf {
    if path
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case(opendrape_io::EXTENSION))
    {
        return path;
    }
    let mut name = path.into_os_string();
    name.push(".");
    name.push(opendrape_io::EXTENSION);
    name.into()
}

/// The toolbar buttons of the 3D view.
#[derive(Clone, Copy)]
enum Toolbar {
    Play,
    Pause,
    Resume,
    Reset,
    /// Turn the camera to look from this side of the form.
    Look(PlaceAt),
}

/// Most drape notes the 3D view lists; the rest are counted ("…and 3 more").
const MAX_NOTES_SHOWN: usize = 5;

/// A drape note as the student reads it, naming pieces as the project does now.
fn note_text(note: &DrapeNote, project: &Project) -> String {
    let name = |id: PieceId| project.name_of(id).unwrap_or_default().to_owned();
    match *note {
        DrapeNote::Mesh(MeshNote::Coarser { .. }) => tr!("note-coarser"),
        DrapeNote::Mesh(MeshNote::CrossesItself(id)) => tr!("note-crosses-itself", name = name(id)),
        DrapeNote::Mesh(MeshNote::Unmeshable(id)) => tr!("note-unmeshable", name = name(id)),
        DrapeNote::Mesh(MeshNote::CutoutLeftOut(id)) => {
            tr!("note-cutout-left-out", name = name(id))
        }
        DrapeNote::Mesh(MeshNote::LengthsDiffer { seam, by_mm }) => tr!(
            "note-lengths-differ",
            number = seam.0,
            difference = project.units.format(by_mm)
        ),
        DrapeNote::StartsInside(id) => tr!("note-starts-inside", name = name(id)),
    }
}

fn choice_label(choice: GpuChoice) -> String {
    match choice {
        GpuChoice::Auto => tr!("graphics-auto"),
        GpuChoice::Dx12 => tr!("graphics-dx12"),
        GpuChoice::Vulkan => tr!("graphics-vulkan"),
        GpuChoice::Metal => tr!("graphics-metal"),
        GpuChoice::Gl => tr!("graphics-gl"),
        GpuChoice::Software => tr!("graphics-software"),
    }
}

impl eframe::App for OpenDrapeApp {
    fn ui(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        self.guard_close(&ctx);
        let shortcut = self.file_shortcut(&ctx);
        let workspace_key = self.workspace_shortcut(&ctx);
        let (menu, picked) = egui::Panel::top("menu_bar")
            .show(ui, |ui| self.menu_bar(ui))
            .inner;
        // A key switches at once: it is never taken while something is being typed. A tab
        // or View menu click switches at the end of the frame, after the pattern table has seen
        // it as a click elsewhere (so a field being typed in keeps its text, and an open
        // number box closes) like any other.
        if let Some(ws) = workspace_key {
            self.workspace = ws;
        }
        if let Some(action) = menu.or(shortcut).or(self.queued.take()) {
            self.file_action(action, frame, &ctx);
        }
        self.about_window(&ctx);
        let dt = ui.input(|i| i.unstable_dt).max(1e-3);
        self.fps = if self.fps == 0.0 {
            1.0 / dt
        } else {
            0.9 * self.fps + 0.1 / dt
        };
        let width = ui.available_width();
        // Decided before the 3D view has run: Escape that gives a drag in the 3D view up (a
        // gizmo handle, a pin, the fabric), or an Undo or Delete typed while one is held, is not
        // also the pattern table's.
        let view_drag = self.arranger.is_dragging() || self.draper.is_dragging();
        egui::Panel::left("view_3d")
            .resizable(true)
            .default_size(width * 0.42)
            .size_range(240.0..=(width - 360.0).max(240.0))
            .show(ui, |ui| self.view_3d(ui, frame));
        // Decided here, before the question or message box below has run: when one of them is
        // closed by Escape this frame, that Escape must not reach the pattern table too.
        let keys_for_pattern =
            self.pending.is_none() && self.error.is_none() && self.offered.is_none() && !view_drag;
        match self.workspace {
            Workspace::Modeling => {
                egui::CentralPanel::default()
                    .show(ui, |ui| self.editor.ui_with_keys(ui, keys_for_pattern));
            }
            ws => {
                if keys_for_pattern {
                    self.app_undo_redo(&ctx);
                }
                egui::CentralPanel::default().show(ui, |ui| workspace::coming_soon(ui, ws));
            }
        }
        if let Some(ws) = picked {
            self.workspace = ws;
            ctx.request_repaint(); // show it now, not on the next input
        }
        self.unsaved_changes_modal(frame, &ctx);
        self.error_modal(&ctx);
        self.recovery_modal(&ctx);
        self.poll_dialog(frame, &ctx);
        self.update_title(&ctx);
        self.confirm_first_frame(&ctx);
    }

    /// The last chance to save anything. Quitting from the Dock, logout and shutdown end the app
    /// without a close request, so nobody could be asked about unsaved work: keep a copy.
    /// Quitting through OpenDrape's own question sets `closing` first and leaves no copy.
    fn on_exit(&mut self) {
        if !self.closing && self.editor.doc.is_dirty() {
            self.recovery
                .write(self.editor.doc.project(), self.editor.doc.path.as_deref());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use opendrape_core::{Piece, Point2, SeamId};

    #[test]
    fn every_drape_note_reads_as_a_sentence_naming_the_piece() {
        let mut project = Project::new();
        let id = project.add_piece(Piece::rectangle(
            PieceId(0),
            "Front",
            Point2::new(0.0, 0.0),
            10.0,
            10.0,
        ));
        let text = |note: DrapeNote| note_text(&note, &project);
        assert_eq!(
            text(DrapeNote::Mesh(MeshNote::CutoutLeftOut(id))),
            "A cut-out in Front touches its outline or another cut-out, or lies outside the piece, so it was left out."
        );
        assert_eq!(
            text(DrapeNote::Mesh(MeshNote::CrossesItself(id))),
            "Front couldn't be made into fabric: its outline crosses itself."
        );
        assert_eq!(
            text(DrapeNote::Mesh(MeshNote::Unmeshable(id))),
            "Front couldn't be made into fabric."
        );
        assert_eq!(
            text(DrapeNote::StartsInside(id)),
            "Front starts inside the form; move it out first."
        );
        assert_eq!(
            text(DrapeNote::Mesh(MeshNote::Coarser { edge_mm: 15.0 })),
            "Large pattern: using coarser fabric"
        );
        assert!(
            text(DrapeNote::Mesh(MeshNote::LengthsDiffer {
                seam: SeamId(3),
                by_mm: 36.0
            }))
            .starts_with("Seam 3: the sides' lengths differ by ")
        );
    }

    /// The app in a headless window, drawing the 3D view off-screen.
    fn headless_app() -> egui_kittest::Harness<'static, OpenDrapeApp> {
        headless_app_recovering(Recovery::new(None))
    }

    /// [`headless_app`], offering back the work `recovery` holds.
    fn headless_app_recovering(recovery: Recovery) -> egui_kittest::Harness<'static, OpenDrapeApp> {
        use crate::gpu::{GpuState, Reason, StateStore};
        let dir = tempfile::tempdir().unwrap();
        let startup = Startup {
            decision: Decision {
                choice: GpuChoice::Auto,
                reason: Reason::Saved,
            },
            previous: GpuState::default(),
            store: StateStore::new(Some(dir.path())),
            smoke_test: false,
            file_dialogs: FileDialogs::always_cancel(),
            recovery,
        };
        egui_kittest::Harness::builder()
            .with_size(egui::vec2(1000.0, 700.0))
            .wgpu()
            .build_eframe(move |cc| OpenDrapeApp::new(cc, startup, SharedState::default()))
    }

    #[test]
    fn a_3d_view_that_cannot_be_made_says_so_and_the_app_carries_on() {
        use egui_kittest::kittest::Queryable;
        use opendrape_core::{Piece, Point2};
        let rectangle = |name: &str, x: f64| {
            Piece::rectangle(PieceId(0), name, Point2::new(x, 0.0), 300.0, 500.0)
        };
        let mut h = headless_app();
        h.run();
        h.state_mut()
            .editor
            .doc
            .edit(|p| p.add_piece(rectangle("Front", 0.0)));
        h.run();
        let note = "The 3D view couldn't show this pattern. Your work is safe; save it and send it to the OpenDrape team.";
        assert_eq!(h.state_mut().arranged_scene().panels.len(), 1);
        assert!(h.query_by_label(note).is_none(), "no note while it works");

        // The pattern changes, and the mesher panics on it (as the cache is asked on the
        // frame that follows). The window keeps drawing and says what happened.
        let shoulder = h.state().stage.shoulder_y();
        h.state_mut()
            .editor
            .doc
            .edit(|p| p.add_piece(rectangle("Back", 500.0)));
        let project = h.state().editor.doc.project().clone();
        h.state_mut()
            .arranged
            .scene_with(&project, shoulder, |_| panic!("the mesher fell over"));
        let drawn = h.state().viewport_frames();
        h.run();
        h.get_by_label(note);
        assert!(
            h.state().viewport_frames() > drawn,
            "the 3D view still draws"
        );
        assert_eq!(
            h.state_mut().arranged_scene().panels.len(),
            1,
            "the last pieces that could be shown"
        );
        // The pattern window and its file still work: the project is whole.
        assert_eq!(h.state().editor.doc.project().pieces.len(), 2);
        assert!(h.state().editor.doc.project().check().is_ok());

        // The next change to the pattern is made again, and the note goes.
        h.state_mut()
            .editor
            .doc
            .edit(|p| p.add_piece(rectangle("Sleeve", 1000.0)));
        h.run();
        assert!(h.query_by_label(note).is_none(), "the note goes");
        assert_eq!(h.state_mut().arranged_scene().panels.len(), 3);
    }

    /// The app in a headless window with no 3D view (as on a machine without a graphics card), so
    /// that nothing asks for the arranged scene unless the test does.
    fn app_without_a_view(
        file_dialogs: FileDialogs,
        recovery: Recovery,
    ) -> egui_kittest::Harness<'static, OpenDrapeApp> {
        use crate::gpu::{GpuState, Reason, StateStore};
        let startup = Startup {
            decision: Decision {
                choice: GpuChoice::Auto,
                reason: Reason::Saved,
            },
            previous: GpuState::default(),
            store: StateStore::new(None),
            smoke_test: false,
            file_dialogs,
            recovery,
        };
        egui_kittest::Harness::builder()
            .with_size(egui::vec2(1000.0, 700.0))
            .build_eframe(move |cc| OpenDrapeApp::new(cc, startup, SharedState::default()))
    }

    fn rectangle(name: &str, x: f64, side_mm: f64) -> opendrape_core::Piece {
        opendrape_core::Piece::rectangle(
            PieceId(0),
            name,
            opendrape_core::Point2::new(x, 0.0),
            side_mm,
            side_mm,
        )
    }

    /// Two pieces (ids 1 and 2) in the project, and the 3D view's scene made for them.
    fn show_two_pieces(h: &mut egui_kittest::Harness<'static, OpenDrapeApp>) {
        h.state_mut().editor.doc.edit(|p| {
            p.add_piece(rectangle("Front", 0.0, 300.0));
            p.add_piece(rectangle("Back", 500.0, 300.0));
        });
        h.run();
        assert_eq!(h.state_mut().arranged_scene().panels.len(), 2);
    }

    /// The pieces the 3D view shows for the project that is open, when the fabric for it can't
    /// be made (as it would be asked for on the next frame, which no test frame gets to first).
    fn shown_when_the_pattern_cannot_be_made(
        h: &mut egui_kittest::Harness<'static, OpenDrapeApp>,
    ) -> Vec<PieceId> {
        let shoulder = h.state().stage.shoulder_y();
        let project = h.state().editor.doc.project().clone();
        let scene = h
            .state_mut()
            .arranged
            .scene_with(&project, shoulder, |_| panic!("the mesher fell over"));
        assert!(h.state().arranged.view_failed());
        scene.panels.iter().map(|p| p.shape).collect()
    }

    #[test]
    fn a_new_project_that_cannot_be_shown_never_shows_the_last_ones_pieces() {
        use egui_kittest::kittest::Queryable;
        let mut h = app_without_a_view(FileDialogs::always_cancel(), Recovery::new(None));
        h.run();
        show_two_pieces(&mut h);
        h.get_by_label("File").click();
        h.run();
        h.get_by_label("New").click();
        h.run();
        h.get_by_label("Don't save").click(); // there are unsaved changes: asked first
        h.run();
        assert_eq!(h.state().editor.doc.project().pieces.len(), 0, "File → New");
        // The new project has a piece under the id the old one's first had.
        h.state_mut()
            .editor
            .doc
            .edit(|p| p.add_piece(rectangle("Skirt", 0.0, 100.0)));
        assert_eq!(shown_when_the_pattern_cannot_be_made(&mut h), vec![]);
    }

    #[test]
    fn an_opened_file_that_cannot_be_shown_never_shows_the_last_ones_pieces() {
        use egui_kittest::kittest::Queryable;
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("skirt.odp");
        let mut skirt = Project::new();
        skirt.add_piece(rectangle("Skirt", 0.0, 100.0));
        opendrape_io::save(&skirt, &file).unwrap();
        let mut h =
            app_without_a_view(FileDialogs::scripted(vec![Some(file)]), Recovery::new(None));
        h.run();
        show_two_pieces(&mut h);
        h.get_by_label("File").click();
        h.run();
        h.get_by_label("Open…").click();
        h.run();
        h.get_by_label("Don't save").click(); // there are unsaved changes: asked first
        h.run();
        assert_eq!(*h.state().editor.doc.project(), skirt, "File → Open");
        assert_eq!(shown_when_the_pattern_cannot_be_made(&mut h), vec![]);
    }

    #[test]
    fn restored_work_that_cannot_be_shown_never_shows_the_last_ones_pieces() {
        use egui_kittest::kittest::Queryable;
        let dir = tempfile::tempdir().unwrap();
        let mut skirt = Project::new();
        skirt.add_piece(rectangle("Skirt", 0.0, 100.0));
        Recovery::new(Some(dir.path())).write(&skirt, None);
        let mut h = app_without_a_view(
            FileDialogs::always_cancel(),
            Recovery::new(Some(dir.path())),
        );
        h.run();
        // The question is asked; the student has already drawn pieces in the window behind it.
        show_two_pieces(&mut h);
        h.get_by_label("Restore").click();
        h.run();
        assert_eq!(*h.state().editor.doc.project(), skirt, "the restored copy");
        assert_eq!(shown_when_the_pattern_cannot_be_made(&mut h), vec![]);
    }

    #[test]
    fn restoring_work_while_draping_returns_to_arranging() {
        use egui_kittest::kittest::Queryable;
        let dir = tempfile::tempdir().unwrap();
        let mut skirt = Project::new();
        skirt.add_piece(rectangle("Skirt", 0.0, 100.0));
        Recovery::new(Some(dir.path())).write(&skirt, None);
        let mut h = headless_app_recovering(Recovery::new(Some(dir.path())));
        h.run();
        // The question is up; a drape is running behind it (nothing in the window can start
        // one while it is open, so the runner is told directly).
        h.state_mut()
            .editor
            .doc
            .edit(|p| p.add_piece(rectangle("Front", 0.0, 300.0)));
        let snapshot = Arc::new(h.state().editor.doc.project().clone());
        let runner = h.state().runner.as_ref().unwrap();
        runner.play(snapshot.clone());
        h.state_mut().draped = Some(snapshot);
        let start = std::time::Instant::now();
        while h.state().sim_frame().is_none() {
            assert!(start.elapsed().as_secs() < 20, "no drape to restore over");
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        assert!(h.state().is_draping());
        h.get_by_label("Restore").click();
        h.run_steps(2);
        assert_eq!(*h.state().editor.doc.project(), skirt, "the restored copy");
        assert!(!h.state().is_draping(), "arranging, as after Reset");
        assert!(h.state().sim_frame().is_none(), "no cloth of the old work");
        assert!(h.state().draped.is_none());
    }

    #[test]
    fn project_extension_is_added_once() {
        assert_eq!(
            with_project_extension("a/skirt".into()),
            PathBuf::from("a/skirt.odp")
        );
        assert_eq!(
            with_project_extension("a/skirt.ODP".into()),
            PathBuf::from("a/skirt.ODP")
        );
        assert_eq!(
            with_project_extension("a/skirt.v2".into()),
            PathBuf::from("a/skirt.v2.odp")
        );
    }
}
