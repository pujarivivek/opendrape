use crate::diagnostics::Diagnostics;
use crate::gpu::{Decision, GpuChoice, GpuState, Os, StateStore, confirmed_state};
use crate::sim_runner::{SimFrame, SimRunner};
use crate::tr;
use crate::viewport::Viewport;
use opendrape_testkit::garments::Garment;
use std::sync::Arc;
use std::{cell::Cell, rc::Rc};

/// What main() decided before the window opened.
#[derive(Clone, Debug)]
pub struct Startup {
    pub decision: Decision,
    pub previous: GpuState,
    pub store: StateStore,
    pub smoke_test: bool,
    /// Start simulating immediately (tests start paused, so `Harness::run` can settle).
    pub autoplay: bool,
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

pub struct OpenDrapeApp {
    viewport: Option<Viewport>,
    diagnostics: Diagnostics,
    startup: Startup,
    shared: SharedState,
    show_about: bool,
    copied: bool,
    runner: Option<SimRunner>,
    garment: Garment,
    fps: f32,
}

impl OpenDrapeApp {
    pub fn new(cc: &eframe::CreationContext<'_>, startup: Startup, shared: SharedState) -> Self {
        let render_state = cc.wgpu_render_state.as_ref();
        let info = render_state.map(|rs| rs.adapter.get_info());
        crate::startup_log::stage(format_args!(
            "window open, graphics: {:?}",
            info.as_ref().map(|i| (&i.name, i.device_type, i.backend))
        ));
        let runner = render_state.map(|_| {
            let ctx = cc.egui_ctx.clone();
            SimRunner::start(Garment::Skirt, startup.autoplay, move || {
                ctx.request_repaint()
            })
        });
        Self {
            viewport: render_state.map(Viewport::new),
            diagnostics: Diagnostics::collect(info.as_ref(), startup.decision),
            startup,
            shared,
            show_about: false,
            copied: false,
            runner,
            garment: Garment::Skirt,
            fps: 0.0,
        }
    }

    /// The latest simulation frame, if the 3D view is running.
    pub fn sim_frame(&self) -> Option<Arc<SimFrame>> {
        self.runner.as_ref().map(SimRunner::latest)
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
        ui.horizontal(|ui| {
            for g in Garment::ALL {
                if ui
                    .selectable_label(self.garment == g, garment_label(g))
                    .clicked()
                    && self.garment != g
                {
                    self.garment = g;
                    runner.reset(g);
                }
            }
            ui.separator();
            let playing = runner.is_playing();
            if ui
                .button(if playing {
                    tr!("toolbar-pause")
                } else {
                    tr!("toolbar-play")
                })
                .clicked()
            {
                runner.set_playing(!playing);
            }
            if ui.button(tr!("toolbar-reset")).clicked() {
                runner.reset(self.garment);
            }
        });
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
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
    }

    fn menu_bar(&mut self, ui: &mut egui::Ui) {
        egui::MenuBar::new().ui(ui, |ui| {
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
                            self.request_graphics_change(choice);
                            ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
                        }
                    }
                    ui.separator();
                    ui.label(tr!("graphics-restart-note"));
                });
            });
        });
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

fn garment_label(g: Garment) -> String {
    match g {
        Garment::Skirt => tr!("garment-skirt"),
        Garment::BodiceProxy => tr!("garment-bodice-proxy"),
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
        egui::Panel::top("menu_bar").show(ui, |ui| self.menu_bar(ui));
        self.about_window(ui.ctx());
        egui::Panel::top("toolbar").show(ui, |ui| self.toolbar(ui));
        let dt = ui.input(|i| i.unstable_dt).max(1e-3);
        self.fps = if self.fps == 0.0 {
            1.0 / dt
        } else {
            0.9 * self.fps + 0.1 / dt
        };
        let sim = self.sim_frame();
        let fps = self
            .runner
            .as_ref()
            .is_some_and(SimRunner::is_playing)
            .then_some(self.fps);
        egui::CentralPanel::default().show(ui, |ui| {
            match (self.viewport.as_mut(), frame.wgpu_render_state()) {
                (Some(viewport), Some(rs)) => {
                    let rect = ui.max_rect();
                    viewport.ui(ui, rs, sim.as_deref());
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
                _ => {
                    ui.centered_and_justified(|ui| ui.label(tr!("viewport-no-gpu")));
                }
            }
        });
        self.confirm_first_frame(ui.ctx());
    }
}
