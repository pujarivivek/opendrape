//! The 2D pattern window: drawing and editing pattern pieces.

mod cache;
mod canvas;
mod document;
mod length_box;
mod line_tool;
mod notch_tool;
mod paint;
mod panel;
mod sew_tool;
mod view;

pub use canvas::PenPoint;
pub use document::{Document, UNDO_LIMIT};
pub use view::View;

use crate::tr;
use egui::{Key, KeyboardShortcut, Modifiers};
use opendrape_core::{PieceId, Point2, Project, SeamId, Units};
use opendrape_geom as geom;
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
    Notch,
    Line,
    Sew,
}

impl Tool {
    pub const ALL: [Self; 7] = [
        Self::Edit,
        Self::Pen,
        Self::Rectangle,
        Self::AddPoint,
        Self::Notch,
        Self::Line,
        Self::Sew,
    ];

    /// Single-key shortcut: the letters other pattern software uses, so habits carry over.
    pub fn key(self) -> Key {
        match self {
            Self::Edit => Key::Z,
            Self::Pen => Key::H,
            Self::Rectangle => Key::S,
            Self::AddPoint => Key::X,
            Self::Notch => Key::N,
            Self::Line => Key::L,
            // S is the Rectangle's.
            Self::Sew => Key::W,
        }
    }

    fn label(self) -> String {
        match self {
            Self::Edit => tr!("tool-edit"),
            Self::Pen => tr!("tool-pen"),
            Self::Rectangle => tr!("tool-rectangle"),
            Self::AddPoint => tr!("tool-add-point"),
            Self::Notch => tr!("tool-notch"),
            Self::Line => tr!("tool-line"),
            Self::Sew => tr!("tool-sew"),
        }
    }

    fn tip(self) -> String {
        match self {
            Self::Edit => tr!("tool-edit-tip"),
            Self::Pen => tr!("tool-pen-tip"),
            Self::Rectangle => tr!("tool-rectangle-tip"),
            Self::AddPoint => tr!("tool-add-point-tip"),
            Self::Notch => tr!("tool-notch-tip"),
            Self::Line => tr!("tool-line-tip"),
            Self::Sew => tr!("tool-sew-tip"),
        }
    }
}

/// What the properties panel shows and Delete removes. The ids are shape ids (a piece's own, or
/// its twin's); point and edge numbers are the stored piece's, whichever shape was clicked.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Selection {
    #[default]
    None,
    Piece(PieceId),
    Vertex(PieceId, usize),
    Edge(PieceId, usize),
    /// A notch: the shape's id and the index into the stored piece's notches.
    Notch(PieceId, usize),
    /// An internal line: the shape's id and the index into the stored piece's lines.
    Line(PieceId, usize),
    /// A stored seam (clicking a seam's mirror image selects the seam).
    Seam(SeamId),
}

impl Selection {
    pub fn piece(self) -> Option<PieceId> {
        match self {
            Self::None | Self::Seam(_) => None,
            Self::Piece(id)
            | Self::Vertex(id, _)
            | Self::Edge(id, _)
            | Self::Notch(id, _)
            | Self::Line(id, _) => Some(id),
        }
    }

    /// This selection if it still exists in `project` (after an undo, say); otherwise its
    /// piece, or nothing. The id may name a twin: its points and edges are its stored piece's.
    pub fn validated(self, project: &Project) -> Self {
        if let Self::Seam(id) = self {
            return if project.seam(id).is_some() {
                self
            } else {
                Self::None
            };
        }
        let Some(id) = self.piece() else {
            return Self::None;
        };
        let Some((piece, _)) = project.owner(id) else {
            return Self::None;
        };
        match self {
            Self::Vertex(_, i) | Self::Edge(_, i) if i >= piece.len() => Self::Piece(id),
            Self::Notch(_, k) if k >= piece.notches.len() => Self::Piece(id),
            Self::Line(_, l) if l >= piece.lines.len() => Self::Piece(id),
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
    /// Show the seam allowance: a light band out to the cut line.
    pub show_allowance: bool,
    /// Where the canvas was last drawn (tests use it to turn millimetres into screen points).
    pub canvas_rect: egui::Rect,
    /// Why the last action was refused, shown in the status bar until the next click.
    pub notice: Option<String>,
    canvas: canvas::CanvasState,
    panel: panel::PanelState,
    cache: cache::ShapeCache,
    fit_pending: bool,
    /// A units switch asked for in the toolbar this frame, applied once the panels and the
    /// canvas have run: a number typed in the old units and applied by the very same click
    /// (the click that moves focus out of its field) must still be read in the old units.
    pending_units: Option<Units>,
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
            show_allowance: true,
            canvas_rect: egui::Rect::NOTHING,
            notice: None,
            canvas: canvas::CanvasState::default(),
            panel: panel::PanelState::default(),
            cache: cache::ShapeCache::default(),
            fit_pending: true,
            pending_units: None,
        }
    }

    /// Starts over with `project` (File → New or Open): clears the history and fits the view.
    pub fn set_project(&mut self, project: Project, path: Option<PathBuf>) {
        let (show_lengths, show_allowance) = (self.show_lengths, self.show_allowance);
        *self = Self {
            show_lengths,
            show_allowance,
            ..Self::new()
        };
        self.doc = Document::new(project, path);
    }

    /// Like [`Self::set_project`], for work restored from a recovery copy: it stays unsaved.
    pub fn set_recovered(&mut self, project: Project, path: Option<PathBuf>) {
        self.set_project(Project::new(), None);
        self.doc = Document::recovered(project, path);
    }

    /// Shows the "can't be made" notice when the document refused the last change (it would
    /// have made the pattern too large or invalid); returns whether it did. Call it straight
    /// after the change.
    pub(super) fn note_if_refused(&mut self) -> bool {
        let refused = self.doc.last_change_refused();
        if refused {
            self.notice = Some(tr!("notice-refused"));
        }
        refused
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

    /// Undo. While a line or a piece is being drawn, removes its last point instead.
    pub fn undo(&mut self) {
        if self.canvas.line.pop().is_some() {
            if self.canvas.line.is_empty() {
                self.canvas.line_owner = None;
            }
        } else if self.canvas.pen.pop().is_none() {
            self.canvas.drag = None;
            self.doc.undo();
        }
        self.selection = self.selection.validated(self.doc.project());
    }

    pub fn redo(&mut self) {
        if self.canvas.pen.is_empty() && self.canvas.line.is_empty() {
            self.canvas.drag = None;
            self.doc.redo();
        }
        self.selection = self.selection.validated(self.doc.project());
    }

    pub fn can_undo(&self) -> bool {
        !self.canvas.pen.is_empty() || !self.canvas.line.is_empty() || self.doc.can_undo()
    }

    pub fn can_redo(&self) -> bool {
        self.canvas.pen.is_empty() && self.canvas.line.is_empty() && self.doc.can_redo()
    }

    /// Show every piece on the next frame.
    pub fn fit(&mut self) {
        self.fit_pending = true;
    }

    /// Every shape on the table (see `geom::shapes`).
    pub(super) fn shapes(&self) -> Vec<geom::Shape> {
        geom::shapes(self.doc.project())
    }

    /// Points placed so far in the piece being drawn with the pen.
    pub fn pen(&self) -> &[PenPoint] {
        &self.canvas.pen
    }

    /// Points placed so far in the internal line being drawn with the line tool.
    pub fn line_draft(&self) -> &[PenPoint] {
        &self.canvas.line
    }

    /// The number box (typed length and angle, or width and height) is open.
    pub fn length_box_open(&self) -> bool {
        self.canvas.length_box.is_some()
    }

    pub fn ui(&mut self, ui: &mut egui::Ui) {
        self.ui_with_keys(ui, true);
    }

    /// Like [`Self::ui`]. With `keys_allowed` false nothing the pattern table listens for on
    /// the keyboard acts (no shortcuts, Delete, Escape or typed numbers): the app passes false
    /// while a question or message box is open over it, so its keys don't also reach the
    /// pattern. Decide it from the state at the start of the frame, before that box has had
    /// the chance to close itself on this very key press.
    pub fn ui_with_keys(&mut self, ui: &mut egui::Ui, keys_allowed: bool) {
        // Sampled before any text box runs this frame. Escape has already cleared focus by
        // now, so an open number box, or a property field that was being typed in last frame,
        // also counts as owning the keyboard.
        let keys_free = keys_allowed
            && !ui.ctx().text_edit_focused()
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
        if let Some(units) = self.pending_units.take() {
            self.doc.edit(|p| p.units = units);
            self.note_if_refused();
            ui.ctx().request_repaint(); // show the new units at once, not on the next input
        }
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
                    self.pending_units = Some(u);
                }
            }
            ui.separator();
            ui.checkbox(&mut self.show_lengths, tr!("toolbar-show-lengths"));
            ui.checkbox(&mut self.show_allowance, tr!("toolbar-show-allowance"));
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

/// Selects all `chars` characters of a pre-filled text field the moment it gains focus, so
/// typing replaces the number instead of being added to the end of it (egui would put the
/// cursor where it was clicked, or at the end after Tab).
fn select_all_on_focus(
    ctx: &egui::Context,
    output: &mut egui::text_edit::TextEditOutput,
    chars: usize,
) {
    if output.response.gained_focus() {
        let all = egui::text::CCursorRange::two(
            egui::text::CCursor::new(0),
            egui::text::CCursor::new(chars),
        );
        output.state.cursor.set_char_range(Some(all));
        output.state.clone().store(ctx, output.response.id);
    }
}

/// The smallest box (mm) holding every shape (twins and the pale halves of folds included).
fn project_bounds(project: &Project) -> Option<(Point2, Point2)> {
    let points: Vec<Point2> = geom::shapes(project)
        .iter()
        .flat_map(|s| geom::outline_points(&s.piece, 1.0))
        .collect();
    let first = *points.first()?;
    Some(points.iter().fold((first, first), |(lo, hi), p| {
        (
            Point2::new(lo.x.min(p.x), lo.y.min(p.y)),
            Point2::new(hi.x.max(p.x), hi.y.max(p.y)),
        )
    }))
}
