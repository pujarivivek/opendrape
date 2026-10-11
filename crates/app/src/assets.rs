//! The Assets section: what can be put on the stage (dress forms for now), shown where the
//! pattern usually is while the student browses.

use crate::editor::text_field;
use crate::icons::icon_button;
use crate::tr;
use egui::{Align2, FontId, Id, Rect, Sense, StrokeKind, Vec2, WidgetInfo, WidgetType, vec2};
use egui_phosphor::regular::{DRESS, T_SHIRT, X};
use opendrape_body::form::{Form, Measurements};
use opendrape_core::{FormChoice, FormSize, Units};
use opendrape_drape::choice::{self, FormProblem};
use std::collections::{BTreeMap, HashMap};

/// A form's card: its picture and name.
const CARD: Vec2 = vec2(132.0, 188.0);
/// The picture on a card, and the size the 3D view draws it at.
pub const THUMB: Vec2 = vec2(116.0, 150.0);
/// The measurements shown read-only under the custom ones, when the form has them.
const MEASURED: [&str; 3] = ["front_waist_length", "apex_to_apex", "high_hip"];

/// The section's heading, with a ✕ that closes it on the right. Returns whether the ✕ was
/// clicked.
pub fn header(ui: &mut egui::Ui) -> bool {
    ui.horizontal(|ui| {
        ui.heading(tr!("assets-dress-forms"));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            icon_button(
                ui,
                X,
                &tr!("assets-close"),
                &tr!("assets-close-tip"),
                false,
                true,
            )
            .clicked()
        })
        .inner
    })
    .inner
}

/// What the student asked for in the dress-form panel.
pub enum FormAction {
    /// Use this form (and size) for the project.
    Pick(FormChoice),
    /// Show (true) or hide the form's tape lines.
    ShowTapes(bool),
}

/// The dress-form panel's own state: what is being typed, the last refusal, the pictures.
#[derive(Default)]
pub struct FormsPanel {
    /// What is being typed in each measurement field.
    editing: HashMap<Id, String>,
    /// Why the last change was refused, until the form changes.
    pub message: Option<String>,
    /// The form the message is about: a new form clears it.
    message_for: Option<FormChoice>,
    /// Each form's picture, drawn by the 3D view.
    pub thumbs: BTreeMap<String, egui::TextureId>,
}

impl FormsPanel {
    /// Shows a refusal (`text`) for the change made to `choice`.
    pub fn refused(&mut self, choice: &FormChoice, text: String) {
        self.message = Some(text);
        self.message_for = Some(choice.clone());
    }

    /// The panel: a card per form, the chart and size, custom measurements with what they
    /// measure, and the tape-line switch. `choice` is the project's form, `measured` what it
    /// measures as built.
    pub fn ui(
        &mut self,
        ui: &mut egui::Ui,
        choice: &FormChoice,
        measured: &Measurements,
        units: Units,
        show_tapes: bool,
    ) -> Option<FormAction> {
        if self.message_for.as_ref() != Some(choice) {
            self.message = None;
        }
        let mut action = None;
        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.horizontal_wrapped(|ui| {
                    for id in Form::IDS {
                        let icon = if id == "men-torso" { T_SHIRT } else { DRESS };
                        let picked = id == choice.id;
                        let thumb = self.thumbs.get(id).copied();
                        if card(ui, &form_name(id), thumb, icon, picked).clicked()
                            && !picked
                            && let Some(base) = choice::base_choice(id)
                        {
                            action = Some(FormAction::Pick(base));
                        }
                    }
                });
                ui.label(form_suits(&choice.id));
                ui.add_space(8.0);
                action = action.take().or(chart_and_size(ui, choice, units));
                ui.add_space(4.0);
                egui::CollapsingHeader::new(tr!("form-custom-heading"))
                    .id_salt("form_custom")
                    .show(ui, |ui| {
                        let typed = self.custom(ui, choice, measured, units);
                        action = action.take().or(typed);
                    });
                if let Some(message) = &self.message {
                    ui.colored_label(ui.visuals().warn_fg_color, message);
                }
                ui.add_space(4.0);
                let mut on = show_tapes;
                if ui.checkbox(&mut on, tr!("form-show-tapes")).changed() {
                    action = Some(FormAction::ShowTapes(on));
                }
            });
        action
    }

    /// One field per measurement the form takes, in the project's units: a value typed and
    /// returned (Enter, or clicking elsewhere) is the form at a custom size. Under them, what
    /// the form measures that can't be set.
    fn custom(
        &mut self,
        ui: &mut egui::Ui,
        choice: &FormChoice,
        measured: &Measurements,
        units: Units,
    ) -> Option<FormAction> {
        let form = choice::form(&choice.id)?;
        let mut action = None;
        egui::Grid::new("form_measurements")
            .num_columns(3)
            .show(ui, |ui| {
                for (name, _) in form.inputs() {
                    let mm = choice.measurements.get(&name).copied().unwrap_or(0.0);
                    let id = Id::new(("form-measurement", &name));
                    let typed = text_field(
                        ui,
                        &mut self.editing,
                        id,
                        &measure_name(&name),
                        &units.format_number(mm),
                    );
                    ui.label(units.suffix());
                    ui.end_row();
                    if let Some(value) = typed.as_deref().and_then(Units::parse) {
                        let new = choice::with_measurement(choice, &name, units.to_mm(value));
                        action = Some(FormAction::Pick(new));
                    }
                }
            });
        ui.add_space(4.0);
        ui.label(tr!("form-measured"));
        for name in MEASURED {
            if let Some(&mm) = measured.get(name) {
                ui.label(format!("{}: {}", measure_name(name), units.format(mm)));
            }
        }
        action
    }
}

/// The chart (Classic form or Everyday body) as two buttons, and the size as a list. A custom
/// size shows "Custom" until a size is picked from the chart.
fn chart_and_size(ui: &mut egui::Ui, choice: &FormChoice, units: Units) -> Option<FormAction> {
    let charts = choice::charts(&choice.id);
    let current = match &choice.size {
        FormSize::Chart { chart, label } => Some((chart.as_str(), label.as_str())),
        FormSize::Custom => None,
    };
    let mut action = None;
    egui::Grid::new("form_chart_and_size")
        .num_columns(2)
        .show(ui, |ui| {
            ui.label(tr!("form-chart"));
            ui.horizontal(|ui| {
                for (kind, _) in &charts {
                    let on = current.is_some_and(|(chart, _)| chart == kind);
                    if ui
                        .add(egui::Button::selectable(on, chart_name(kind)))
                        .clicked()
                        && !on
                    {
                        action = choice::nearest_in(choice, kind).map(FormAction::Pick);
                    }
                }
            });
            ui.end_row();
            ui.label(tr!("form-size"));
            let kind = current.map_or("classic", |(chart, _)| chart);
            let girth = choice::girth_name(&choice.id).unwrap_or_default();
            if let Some((_, chart)) = charts.iter().find(|(k, _)| k == kind) {
                let entry = |row: &opendrape_body::form::ChartSize| {
                    let size = match &row.alt {
                        Some(alt) => format!("{} / {alt}", row.label),
                        None => row.label.clone(),
                    };
                    let value = row
                        .mm
                        .get(&girth)
                        .map_or(String::new(), |&mm| units.format(mm));
                    match girth.as_str() {
                        "chest" => tr!("form-size-chest", size = size, value = value),
                        _ => tr!("form-size-bust", size = size, value = value),
                    }
                };
                let selected = current
                    .and_then(|(_, label)| chart.size(label))
                    .map_or_else(|| tr!("form-custom"), entry);
                egui::ComboBox::from_id_salt("form_size")
                    .selected_text(selected)
                    .width(220.0)
                    .show_ui(ui, |ui| {
                        for row in &chart.sizes {
                            let on = current.is_some_and(|(_, label)| label == row.label);
                            if ui.selectable_label(on, entry(row)).clicked() && !on {
                                action = choice::chart_choice(&choice.id, kind, &row.label)
                                    .map(FormAction::Pick);
                            }
                        }
                    });
            }
            ui.end_row();
        });
    action
}

/// A form's card: its picture (or `icon` while there is none) over its name, highlighted when
/// it is the project's form. Read out as a button named after the form.
fn card(
    ui: &mut egui::Ui,
    name: &str,
    thumb: Option<egui::TextureId>,
    icon: &str,
    picked: bool,
) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(CARD, Sense::click());
    response.widget_info(|| WidgetInfo::selected(WidgetType::Button, true, picked, name));
    if ui.is_rect_visible(rect) {
        let visuals = ui.style().interact_selectable(&response, picked);
        let painter = ui.painter();
        painter.rect(
            rect,
            visuals.corner_radius,
            visuals.weak_bg_fill,
            visuals.bg_stroke,
            StrokeKind::Inside,
        );
        let pad = (CARD.x - THUMB.x) / 2.0;
        let picture = Rect::from_min_size(rect.min + vec2(pad, pad), THUMB);
        match thumb {
            Some(texture) => {
                egui::Image::new((texture, THUMB)).paint_at(ui, picture);
            }
            None => {
                painter.text(
                    picture.center(),
                    Align2::CENTER_CENTER,
                    icon,
                    FontId::proportional(56.0),
                    visuals.text_color(),
                );
            }
        }
        painter.text(
            egui::pos2(rect.center().x, picture.max.y + 8.0),
            Align2::CENTER_TOP,
            name,
            FontId::proportional(13.0),
            visuals.text_color(),
        );
    }
    response
}

/// Why a form can't be built, in plain words and the project's units: "Waist can be 50.0–130.0
/// cm on this form".
pub fn form_problem_text(problem: &FormProblem, units: Units) -> String {
    match problem {
        FormProblem::Size(e) => tr!(
            "form-out-of-range",
            name = measure_name(&e.measurement),
            min = units.format_number(e.min_mm),
            max = units.format(e.max_mm)
        ),
        FormProblem::Unknown(id) => tr!("form-unknown", id = id.as_str()),
    }
}

/// A form's name, from its id; an unknown id shows itself.
pub fn form_name(id: &str) -> String {
    match id {
        "women-torso" => tr!("form-women-torso"),
        "men-torso" => tr!("form-men-torso"),
        other => other.to_string(),
    }
}

/// What a form is for.
fn form_suits(id: &str) -> String {
    match id {
        "women-torso" => tr!("form-women-torso-suits"),
        "men-torso" => tr!("form-men-torso-suits"),
        _ => String::new(),
    }
}

/// A chart's name, from its kind.
fn chart_name(kind: &str) -> String {
    match kind {
        "classic" => tr!("chart-classic"),
        "everyday" => tr!("chart-everyday"),
        other => other.to_string(),
    }
}

/// A measurement's name, from its key; an unknown key shows itself.
pub fn measure_name(name: &str) -> String {
    match name {
        "bust" => tr!("measure-bust"),
        "chest" => tr!("measure-chest"),
        "under_bust" => tr!("measure-under-bust"),
        "waist" => tr!("measure-waist"),
        "hip" => tr!("measure-hip"),
        "high_hip" => tr!("measure-high-hip"),
        "neck" => tr!("measure-neck"),
        "shoulder_length" => tr!("measure-shoulder-length"),
        "back_waist_length" => tr!("measure-back-waist-length"),
        "waist_to_hip" => tr!("measure-waist-to-hip"),
        "front_waist_length" => tr!("measure-front-waist-length"),
        "apex_to_apex" => tr!("measure-apex-to-apex"),
        other => other.to_string(),
    }
}
