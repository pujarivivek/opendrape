use crate::arrange::{ArrangedScene, Arranger, SceneCache, ScreenCamera};
use crate::diagnostics::Diagnostics;
use crate::editor::{self, PatternEditor};
use crate::file_dialogs::{DialogKind, FileDialogs};
use crate::gpu::{Decision, GpuChoice, GpuState, Os, StateStore, confirmed_state};
use crate::recovery::Recovery;
use crate::sim_runner::{DrapeNote, SimFrame, SimRunner};
use crate::stage::Stage;
use crate::tr;
use crate::viewport::{Show, Viewport};
use egui::{Key, KeyboardShortcut, Modifiers, ViewportCommand};
use opendrape_core::{PieceId, Project};
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
    /// The project as it was when Play was pressed: any change to it returns to arranging.
    draped: Option<Arc<Project>>,
    /// The pieces as the 3D view shows them while arranging.
    arranged: SceneCache,
    /// What the pointer does in the 3D view while arranging.
    arranger: Arranger,
    /// The 3D view's camera as last drawn.
    view_camera: Option<ScreenCamera>,
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
}

impl OpenDrapeApp {
    pub fn new(cc: &eframe::CreationContext<'_>, startup: Startup, shared: SharedState) -> Self {
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
            editor: PatternEditor::new(),
            stage,
            draped: None,
            arranged: SceneCache::default(),
            arranger: Arranger::default(),
            view_camera: None,
            pending: None,
            queued: None,
            error: None,
            closing: false,
            title: String::new(),
            recovery,
            offered,
        }
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
            let label = if draping && playing {
                tr!("toolbar-pause")
            } else {
                tr!("toolbar-play")
            };
            if ui.button(label).clicked() {
                clicked = Some(match (draping, playing) {
                    (false, _) => Toolbar::Play,
                    (true, true) => Toolbar::Pause,
                    (true, false) => Toolbar::Resume,
                });
            }
            if ui
                .add_enabled(draping, egui::Button::new(tr!("toolbar-reset")))
                .clicked()
            {
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

    /// A pattern edit while draped returns to arranging (live updates come later).
    fn reset_if_edited(&mut self) {
        let Some(runner) = &self.runner else { return };
        let edited = self
            .draped
            .as_ref()
            .is_some_and(|d| **d != *self.editor.doc.project());
        if edited {
            runner.reset();
        }
        if edited || !runner.is_draping() {
            self.draped = None;
        }
    }

    /// The hint while draping, and what the student should know about the drape.
    fn notes(&self, ui: &mut egui::Ui) {
        let Some(runner) = &self.runner else { return };
        let warn = ui.visuals().warn_fg_color;
        if runner.went_wrong() {
            ui.colored_label(warn, tr!("note-went-wrong"));
        }
        if runner.is_draping() {
            ui.label(tr!("hint-draping"));
        }
        if let Some(frame) = runner.latest() {
            for note in frame.notes.iter() {
                ui.colored_label(warn, note_text(note, self.editor.doc.project()));
            }
        }
    }

    /// The 3D view: its toolbar and notes, the form with the pieces being arranged or the
    /// drape, and the speed overlay.
    fn view_3d(&mut self, ui: &mut egui::Ui, frame: &eframe::Frame) {
        self.reset_if_edited();
        self.toolbar(ui);
        self.notes(ui);
        ui.separator();
        let (Some(viewport), Some(rs)) = (self.viewport.as_mut(), frame.wgpu_render_state()) else {
            ui.centered_and_justified(|ui| ui.label(tr!("viewport-no-gpu")));
            return;
        };
        let sim = self.runner.as_ref().and_then(SimRunner::latest);
        let fps = self
            .runner
            .as_ref()
            .is_some_and(SimRunner::is_playing)
            .then_some(self.fps);
        // Kept up to date while draping too: the project doesn't change then, so it costs
        // nothing, and Reset shows the pieces at once.
        let scene = self
            .arranged
            .scene(self.editor.doc.project(), self.stage.shoulder_y());
        let rect = ui.available_rect_before_wrap();
        // Arranging until the drape's first frame arrives.
        let show = match &sim {
            Some(f) => Show::Drape(f),
            None => Show::Pieces {
                scene: &scene,
                selected: self.editor.selection.piece(),
            },
        };
        if let Some(drawn) = viewport.ui(ui, rs, show) {
            self.view_camera = Some(drawn.camera);
            if sim.is_none() {
                self.arrange(&drawn.response, &drawn.camera, &scene);
            }
            if let Some(viewport) = self.viewport.as_mut() {
                // A drag turns the camera.
                let drag = drawn.response.drag_delta();
                if drag != egui::Vec2::ZERO {
                    viewport.camera_mut().drag(drag.x, drag.y);
                }
                if drawn.response.hovered() {
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
                egui::Color32::from_gray(60),
            );
        }
    }

    /// The pointer in the 3D view while arranging: a click picks a piece (or clears the
    /// selection).
    fn arrange(&mut self, response: &egui::Response, cam: &ScreenCamera, scene: &ArrangedScene) {
        if response.clicked()
            && let Some(p) = response.interact_pointer_pos()
        {
            let at = glam::DVec2::new(f64::from(p.x), f64::from(p.y));
            self.arranger
                .click(cam, scene, &mut self.editor.selection, at);
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

    fn menu_bar(&mut self, ui: &mut egui::Ui) -> Option<FileAction> {
        let mut action = None;
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
        });
        action
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
            Then::NewProject => self.editor.set_project(Project::new(), None),
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
                Ok(project) => self.editor.set_project(project, Some(path)),
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
            self.editor.set_recovered(project, from);
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

/// A drape note as the student reads it, naming pieces as the project does now.
fn note_text(note: &DrapeNote, project: &Project) -> String {
    let name = |id: PieceId| project.name_of(id).unwrap_or_default().to_owned();
    match *note {
        DrapeNote::Mesh(MeshNote::Coarser { .. }) => tr!("note-coarser"),
        DrapeNote::Mesh(MeshNote::CrossesItself(id)) => tr!("note-crosses-itself", name = name(id)),
        DrapeNote::Mesh(MeshNote::Unmeshable(id)) => tr!("note-unmeshable", name = name(id)),
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
        let menu = egui::Panel::top("menu_bar")
            .show(ui, |ui| self.menu_bar(ui))
            .inner;
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
        egui::Panel::left("view_3d")
            .resizable(true)
            .default_size(width * 0.42)
            .size_range(240.0..=(width - 360.0).max(240.0))
            .show(ui, |ui| self.view_3d(ui, frame));
        // Decided here, before the question or message box below has run: when one of them is
        // closed by Escape this frame, that Escape must not reach the pattern table too.
        let keys_for_pattern =
            self.pending.is_none() && self.error.is_none() && self.offered.is_none();
        egui::CentralPanel::default().show(ui, |ui| self.editor.ui_with_keys(ui, keys_for_pattern));
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
