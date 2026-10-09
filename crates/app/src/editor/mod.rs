//! The 2D pattern window: drawing and editing pattern pieces.

mod canvas;
mod document;
mod length_box;
mod paint;
mod panel;
mod view;

pub use canvas::PenPoint;
pub use document::{Document, UNDO_LIMIT};
pub use view::View;

use crate::tr;
use egui::{Key, KeyboardShortcut, Modifiers};
use opendrape_core::{PieceId, Point2, Project, Units};
use std::path::PathBuf;

/// Undo: Cmd+Z (Ctrl+Z on Windows).
pub const UNDO: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::Z);
/// Redo: Shift+Cmd+Z (Shift+Ctrl+Z on Windows).
pub const REDO: KeyboardShortcut = KeyboardShortcut::new(
    Modifiers {
        shift: true,
        ..Modifiers::COMMAND
    },
    Key::Z,
);
/// Redo the Windows way: Ctrl+Y.
const REDO_Y: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::Y);

/// Pointer distance (screen points) that counts as touching a point, handle or edge.
const HIT_PX: f64 = 8.0;
/// What an empty pattern table shows: 80 × 60 cm.
const EMPTY_TABLE: (Point2, Point2) = (Point2::new(0.0, 0.0), Point2::new(800.0, 600.0));

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Tool {
    #[default]
    Edit,
    Pen,
    Rectangle,
    AddPoint,
}

impl Tool {
    pub const ALL: [Self; 4] = [Self::Edit, Self::Pen, Self::Rectangle, Self::AddPoint];

    /// Single-key shortcut: the letters other pattern software uses, so habits carry over.
    pub fn key(self) -> Key {
        match self {
            Self::Edit => Key::Z,
            Self::Pen => Key::H,
            Self::Rectangle => Key::S,
            Self::AddPoint => Key::X,
        }
    }

    fn label(self) -> String {
        match self {
            Self::Edit => tr!("tool-edit"),
            Self::Pen => tr!("tool-pen"),
            Self::Rectangle => tr!("tool-rectangle"),
            Self::AddPoint => tr!("tool-add-point"),
        }
    }

    fn tip(self) -> String {
        match self {
            Self::Edit => tr!("tool-edit-tip"),
            Self::Pen => tr!("tool-pen-tip"),
            Self::Rectangle => tr!("tool-rectangle-tip"),
            Self::AddPoint => tr!("tool-add-point-tip"),
        }
    }
}

/// What the properties panel shows and Delete removes.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Selection {
    #[default]
    None,
    Piece(PieceId),
    Vertex(PieceId, usize),
    Edge(PieceId, usize),
}

impl Selection {
    pub fn piece(self) -> Option<PieceId> {
        match self {
            Self::None => None,
            Self::Piece(id) | Self::Vertex(id, _) | Self::Edge(id, _) => Some(id),
        }
    }

    /// This selection if it still exists in `project` (after an undo, say); otherwise its
    /// piece, or nothing.
    pub fn validated(self, project: &Project) -> Self {
        let Some(piece) = self.piece().and_then(|id| project.piece(id)) else {
            return Self::None;
        };
        match self {
            Self::Vertex(_, i) | Self::Edge(_, i) if i >= piece.len() => Self::Piece(piece.id),
            other => other,
        }
    }
}

pub struct PatternEditor {
    pub doc: Document,
    pub view: View,
    pub tool: Tool,
    pub selection: Selection,
    /// Show every edge's length on the pattern.
    pub show_lengths: bool,
    /// Where the canvas was last drawn (tests use it to turn millimetres into screen points).
    pub canvas_rect: egui::Rect,
    /// Why the last action was refused, shown in the status bar until the next click.
    pub notice: Option<String>,
    canvas: canvas::CanvasState,
    panel: panel::PanelState,
    fit_pending: bool,
}

impl Default for PatternEditor {
    fn default() -> Self {
        Self::new()
    }
}

impl PatternEditor {
    pub fn new() -> Self {
        Self {
            doc: Document::default(),
            view: View::default(),
            tool: Tool::default(),
            selection: Selection::None,
            show_lengths: true,
            canvas_rect: egui::Rect::NOTHING,
            notice: None,
            canvas: canvas::CanvasState::default(),
            panel: panel::PanelState::default(),
            fit_pending: true,
        }
    }

    /// Starts over with `project` (File → New or Open): clears the history and fits the view.
    pub fn set_project(&mut self, project: Project, path: Option<PathBuf>) {
        let show_lengths = self.show_lengths;
        *self = Self {
            show_lengths,
            ..Self::new()
        };
        self.doc = Document::new(project, path);
    }

    pub fn set_tool(&mut self, tool: Tool) {
        if tool == self.tool {
            return;
        }
        self.canvas = canvas::CanvasState::default();
        self.doc.end_gesture();
        self.notice = None;
        self.tool = tool;
    }

    /// Undo. While a piece is being drawn with the pen, removes its last point instead.
    pub fn undo(&mut self) {
        if self.canvas.pen.pop().is_none() {
            self.canvas.drag = None;
            self.doc.undo();
        }
        self.selection = self.selection.validated(self.doc.project());
    }

    pub fn redo(&mut self) {
        if self.canvas.pen.is_empty() {
            self.canvas.drag = None;
            self.doc.redo();
        }
        self.selection = self.selection.validated(self.doc.project());
    }

    pub fn can_undo(&self) -> bool {
        !self.canvas.pen.is_empty() || self.doc.can_undo()
    }

    pub fn can_redo(&self) -> bool {
        self.canvas.pen.is_empty() && self.doc.can_redo()
    }

    /// Show every piece on the next frame.
    pub fn fit(&mut self) {
        self.fit_pending = true;
    }

    /// Points placed so far in the piece being drawn with the pen.
    pub fn pen(&self) -> &[PenPoint] {
        &self.canvas.pen
    }

    /// The number box (typed length and angle, or width and height) is open.
    pub fn length_box_open(&self) -> bool {
        self.canvas.length_box.is_some()
    }

    pub fn ui(&mut self, ui: &mut egui::Ui) {
        // Sampled before any text box runs this frame. Escape has already cleared focus by
        // now, so an open number box, or a property field that was being typed in last frame,
        // also counts as owning the keyboard.
        let keys_free = !ui.ctx().text_edit_focused()
            && self.canvas.length_box.is_none()
            && !self.panel.is_editing();
        if keys_free {
            self.shortcuts(ui);
        }
        self.selection = self.selection.validated(self.doc.project());
        egui::Panel::top("pattern_tools").show(ui, |ui| self.toolbar(ui));
        egui::Panel::bottom("pattern_status").show(ui, |ui| self.status_bar(ui));
        egui::Panel::right("pattern_properties")
            .resizable(false)
            .exact_size(240.0)
            .show(ui, |ui| self.properties(ui));
        egui::CentralPanel::default().show(ui, |ui| self.canvas_ui(ui, keys_free));
    }

    fn shortcuts(&mut self, ui: &egui::Ui) {
        // Redo first: `consume_shortcut` also matches Cmd+Z while Shift is held.
        if ui.input_mut(|i| i.consume_shortcut(&REDO) || i.consume_shortcut(&REDO_Y)) {
            self.redo();
        } else if ui.input_mut(|i| i.consume_shortcut(&UNDO)) {
            self.undo();
        }
        for tool in Tool::ALL {
            if ui.input_mut(|i| i.consume_key(Modifiers::NONE, tool.key())) {
                self.set_tool(tool);
            }
        }
        if ui.input_mut(|i| i.consume_key(Modifiers::NONE, Key::F)) {
            self.fit_pending = true;
        }
    }

    fn toolbar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal_wrapped(|ui| {
            for tool in Tool::ALL {
                let label = format!("{} ({})", tool.label(), tool.key().name());
                if ui
                    .selectable_label(self.tool == tool, label)
                    .on_hover_text(tool.tip())
                    .clicked()
                {
                    self.set_tool(tool);
                }
            }
            ui.separator();
            let units = self.doc.project().units;
            for (u, label) in [
                (Units::Cm, tr!("units-cm")),
                (Units::Inch, tr!("units-inch")),
            ] {
                if ui.selectable_label(units == u, label).clicked() && units != u {
                    self.doc.edit(|p| p.units = u);
                }
            }
            ui.separator();
            ui.checkbox(&mut self.show_lengths, tr!("toolbar-show-lengths"));
            if ui
                .button(tr!("toolbar-fit"))
                .on_hover_text(tr!("toolbar-fit-tip"))
                .clicked()
            {
                self.fit_pending = true;
            }
        });
    }
}

/// The smallest box (mm) holding every piece.
fn project_bounds(project: &Project) -> Option<(Point2, Point2)> {
    let mut points = project
        .pieces
        .iter()
        .flat_map(|p| opendrape_geom::outline_points(p, 1.0));
    let first = points.next()?;
    Some(points.fold((first, first), |(lo, hi), p| {
        (
            Point2::new(lo.x.min(p.x), lo.y.min(p.y)),
            Point2::new(hi.x.max(p.x), hi.y.max(p.y)),
        )
    }))
}
