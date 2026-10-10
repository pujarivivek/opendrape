//! The five workspaces along the top of the window, one for each stage of making a garment:
//! Modeling (patterns, sewing, draping), Finishing, Texturing, Rendering and Animation. Which
//! one is open is screen state only: it isn't saved with the project and isn't an undo step.

use crate::tr;
use egui::{
    Key, KeyboardShortcut, Modifiers, TextStyle, TextWrapMode, WidgetText, accesskit::Role,
};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Workspace {
    #[default]
    Modeling,
    Finishing,
    Texturing,
    Rendering,
    Animation,
}

impl Workspace {
    pub const ALL: [Self; 5] = [
        Self::Modeling,
        Self::Finishing,
        Self::Texturing,
        Self::Rendering,
        Self::Animation,
    ];

    pub fn label(self) -> String {
        match self {
            Self::Modeling => tr!("workspace-modeling"),
            Self::Finishing => tr!("workspace-finishing"),
            Self::Texturing => tr!("workspace-texturing"),
            Self::Rendering => tr!("workspace-rendering"),
            Self::Animation => tr!("workspace-animation"),
        }
    }

    /// One line on what the workspace holds.
    pub fn blurb(self) -> String {
        match self {
            Self::Modeling => tr!("workspace-modeling-blurb"),
            Self::Finishing => tr!("workspace-finishing-blurb"),
            Self::Texturing => tr!("workspace-texturing-blurb"),
            Self::Rendering => tr!("workspace-rendering-blurb"),
            Self::Animation => tr!("workspace-animation-blurb"),
        }
    }

    /// Cmd+1 to Cmd+5 (Ctrl on Windows), in tab order.
    pub fn shortcut(self) -> KeyboardShortcut {
        let key = match self {
            Self::Modeling => Key::Num1,
            Self::Finishing => Key::Num2,
            Self::Texturing => Key::Num3,
            Self::Rendering => Key::Num4,
            Self::Animation => Key::Num5,
        };
        KeyboardShortcut::new(Modifiers::COMMAND, key)
    }
}

/// What a workspace shows until its tools are built: its name, what it will hold, and that
/// it is on its way.
pub fn coming_soon(ui: &mut egui::Ui, ws: Workspace) {
    ui.vertical_centered(|ui| {
        ui.add_space(ui.available_height() * 0.3);
        ui.heading(ws.label());
        ui.label(ws.blurb());
        ui.weak(tr!("coming-soon"));
    });
}

/// The row of workspace tabs, centred in the window when there is room (after whatever is
/// already in the row when there isn't). Returns the tab clicked.
pub fn tabs(ui: &mut egui::Ui, current: Workspace) -> Option<Workspace> {
    let labels = Workspace::ALL.map(Workspace::label);
    let padding = 2.0 * ui.spacing().button_padding.x;
    let gap = ui.spacing().item_spacing.x;
    let width = labels
        .iter()
        .map(|label| {
            let text = WidgetText::from(label.as_str());
            let galley = text.into_galley(
                ui,
                Some(TextWrapMode::Extend),
                f32::INFINITY,
                TextStyle::Button,
            );
            galley.size().x + padding
        })
        .sum::<f32>()
        + gap * (labels.len() - 1) as f32;
    let start = ui.ctx().content_rect().center().x - width / 2.0;
    ui.add_space((start - ui.cursor().min.x).max(gap));
    let mut clicked = None;
    for (ws, label) in Workspace::ALL.into_iter().zip(labels) {
        let selected = ws == current;
        let response = ui.add(egui::Button::selectable(selected, label.as_str()));
        ui.ctx().accesskit_node_builder(response.id, |node| {
            node.set_role(Role::Tab);
            node.set_label(label.clone());
            node.set_selected(selected);
        });
        let shortcut = ui.ctx().format_shortcut(&ws.shortcut());
        let tip = format!("{label} ({shortcut})\n{}", ws.blurb());
        if response.on_hover_text(tip).clicked() {
            clicked = Some(ws);
        }
    }
    clicked
}
