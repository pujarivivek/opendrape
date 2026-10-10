//! Icons (Phosphor, MIT) and the icon buttons drawn with them. A button shows only its icon,
//! so its name is set as the accessible label (screen readers and the UI tests find it by
//! that), and its tooltip gives the name again with a line about what it does.

pub use egui_phosphor::regular as ph;

use egui::{Button, RichText, vec2};

/// Icon size in points; the button around it is at least [`BUTTON_PT`] square.
const ICON_PT: f32 = 18.0;
const BUTTON_PT: f32 = 28.0;

/// A button showing `icon`, highlighted when `selected`. Its accessible name is `label`, and
/// hovering it (enabled or not) shows `label` with `tip` on the line below.
pub fn icon_button(
    ui: &mut egui::Ui,
    icon: &str,
    label: &str,
    tip: &str,
    selected: bool,
    enabled: bool,
) -> egui::Response {
    let button = Button::selectable(selected, RichText::new(icon).size(ICON_PT))
        .min_size(vec2(BUTTON_PT, BUTTON_PT));
    let response = ui.add_enabled(enabled, button);
    let name = label.to_owned();
    ui.ctx()
        .accesskit_node_builder(response.id, |node| node.set_label(name));
    let tooltip = format!("{label}\n{tip}");
    response
        .on_hover_text(tooltip.clone())
        .on_disabled_hover_text(tooltip)
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui_kittest::{Harness, kittest::Queryable};

    fn harness(enabled: bool) -> Harness<'static, u32> {
        let mut h = Harness::new_ui_state(
            move |ui, clicks: &mut u32| {
                let tip = "Drape the pieces on the form.";
                if icon_button(ui, ph::PLAY, "Play", tip, false, enabled).clicked() {
                    *clicks += 1;
                }
            },
            0,
        );
        crate::theme::install(&h.ctx);
        h.run();
        h
    }

    #[test]
    fn icon_button_is_found_by_its_name_and_clicks() {
        let mut h = harness(true);
        h.get_by_label("Play").click();
        h.run();
        assert_eq!(*h.state(), 1);
    }

    /// The tooltip names the button and says what it does; it is never just the name, so a
    /// tooltip on screen can't make the button's name ambiguous.
    #[test]
    fn hovering_shows_name_and_tip_without_repeating_the_name_alone() {
        let mut h = harness(true);
        h.get_by_label("Play").hover();
        h.run_steps(30);
        h.get_by_label("Play\nDrape the pieces on the form.");
        h.get_by_label("Play");
    }

    #[test]
    fn a_disabled_button_still_explains_itself() {
        let mut h = harness(false);
        h.get_by_label("Play").hover();
        h.run_steps(30);
        h.get_by_label("Play\nDrape the pieces on the form.");
        h.get_by_label("Play").click();
        h.run();
        assert_eq!(*h.state(), 0);
    }
}
