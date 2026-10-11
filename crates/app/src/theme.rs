//! OpenDrape's look in one place: the style installed at start-up (spacing, rounding, text
//! sizes, the selection highlight, the icon font) and every colour the app draws with.

use egui::{Color32, CornerRadius, FontFamily, FontId, Stroke, TextStyle, Theme, vec2};

/// The selected tool, tab or text in light mode: a pale blue with dark-blue text.
pub const LIGHT_SELECTION_BG: Color32 = Color32::from_rgb(220, 231, 251);
pub const LIGHT_SELECTION_TEXT: Color32 = Color32::from_rgb(28, 74, 160);
/// The same in dark mode: a deep blue with pale-blue text.
pub const DARK_SELECTION_BG: Color32 = Color32::from_rgb(42, 74, 127);
pub const DARK_SELECTION_TEXT: Color32 = Color32::from_rgb(207, 224, 255);

/// A pin's marker, on the pattern table and in the 3D view.
pub const PIN: Color32 = Color32::from_rgb(200, 30, 60);

/// Each seam's colour on the pattern table, in turn.
pub const SEAM_COLOURS: [Color32; 8] = [
    Color32::from_rgb(0, 150, 136),
    Color32::from_rgb(156, 39, 176),
    Color32::from_rgb(33, 120, 243),
    Color32::from_rgb(67, 160, 71),
    Color32::from_rgb(216, 27, 96),
    Color32::from_rgb(121, 85, 72),
    Color32::from_rgb(63, 81, 181),
    Color32::from_rgb(190, 145, 0),
];

/// The gizmo's X, Y and Z handles in the 3D view.
pub const AXIS_COLOURS: [Color32; 3] = [
    Color32::from_rgb(220, 60, 60),
    Color32::from_rgb(60, 170, 70),
    Color32::from_rgb(60, 110, 235),
];
/// A gizmo handle under the pointer, or being dragged.
pub const GIZMO_BRIGHT: Color32 = Color32::from_rgb(255, 200, 0);
/// The gizmo's centre square (move facing the viewer) and its outline.
pub const GIZMO_SQUARE: Color32 = Color32::from_rgba_premultiplied(170, 170, 170, 170);
pub const GIZMO_OUTLINE: Color32 = Color32::from_gray(60);
/// The readout beside the pointer while dragging in the 3D view.
pub const READOUT_TEXT: Color32 = Color32::from_gray(30);
/// A seam's number on its coloured badge on the pattern table.
pub const SEAM_BADGE_TEXT: Color32 = Color32::WHITE;

/// The dress form in the 3D view: matte linen beige (sRGB).
pub const FORM_SRGB: [u8; 3] = opendrape_render::studio::look::FORM_SRGB;
/// The form's seams, measuring tapes, stand and metal neck cap.
pub const SEAM_SRGB: [u8; 3] = opendrape_render::studio::look::SEAM_SRGB;
pub const TAPE_SRGB: [u8; 3] = opendrape_render::studio::look::TAPE_SRGB;
pub const STAND_SRGB: [u8; 3] = opendrape_render::studio::look::STAND_SRGB;
pub const METAL_SRGB: [u8; 3] = opendrape_render::studio::look::METAL_SRGB;
/// The fabric (a cotton blue) and the selected piece (a warmer orange), as linear RGB.
/// Bleached muslin (sRGB 242, 240, 236), the fabric every garment is cut from for now.
pub const FABRIC: [f32; 3] = [0.888, 0.871, 0.839];
pub const SELECTED_FABRIC: [f32; 3] = [0.95, 0.55, 0.25];

/// The speed numbers over the 3D view.
pub const STATS_TEXT: Color32 = Color32::from_gray(60);

/// The pattern table's colours, for light or dark mode.
pub(crate) struct PatternPalette {
    pub table: Color32,
    pub minor: Color32,
    pub major: Color32,
    pub ink: Color32,
    pub fill: Color32,
    pub selected: Color32,
    pub handle: Color32,
    pub label: Color32,
    /// The seam allowance band.
    pub band: Color32,
    /// The cut line.
    pub cut: Color32,
    /// The outline and fill of the pale half of a cut-on-fold piece.
    pub pale: Color32,
    pub pale_fill: Color32,
}

impl PatternPalette {
    pub(crate) fn new(dark: bool) -> Self {
        if dark {
            Self {
                table: Color32::from_gray(30),
                minor: Color32::from_gray(42),
                major: Color32::from_gray(64),
                ink: Color32::from_gray(220),
                fill: Color32::from_rgba_unmultiplied(120, 160, 230, 46),
                selected: Color32::from_rgb(255, 150, 90),
                handle: Color32::from_rgb(120, 170, 255),
                label: Color32::from_gray(190),
                band: Color32::from_rgba_unmultiplied(120, 160, 230, 20),
                cut: Color32::from_gray(120),
                pale: Color32::from_gray(110),
                pale_fill: Color32::from_rgba_unmultiplied(120, 160, 230, 22),
            }
        } else {
            Self {
                table: Color32::from_gray(250),
                minor: Color32::from_gray(232),
                major: Color32::from_gray(205),
                ink: Color32::from_rgb(40, 40, 60),
                fill: Color32::from_rgba_unmultiplied(70, 110, 200, 40),
                selected: Color32::from_rgb(220, 90, 30),
                handle: Color32::from_rgb(50, 110, 220),
                label: Color32::from_gray(70),
                band: Color32::from_rgba_unmultiplied(70, 110, 200, 18),
                cut: Color32::from_gray(150),
                pale: Color32::from_gray(165),
                pale_fill: Color32::from_rgba_unmultiplied(70, 110, 200, 20),
            }
        }
    }
}

/// Sets OpenDrape's style on `ctx`, for light and dark mode: the icon font, spacing, corner
/// rounding, text sizes and the selection highlight. Called once, when the window opens.
pub fn install(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();
    egui_phosphor::add_to_fonts(&mut fonts, egui_phosphor::Variant::Regular);
    ctx.set_fonts(fonts);
    ctx.all_styles_mut(|style| {
        style.spacing.item_spacing = vec2(8.0, 6.0);
        style.spacing.button_padding = vec2(6.0, 3.0);
        let widgets = &mut style.visuals.widgets;
        for w in [
            &mut widgets.noninteractive,
            &mut widgets.inactive,
            &mut widgets.hovered,
            &mut widgets.active,
            &mut widgets.open,
        ] {
            w.corner_radius = CornerRadius::same(4);
        }
        for (text, size, family) in [
            (TextStyle::Body, 13.0, FontFamily::Proportional),
            (TextStyle::Button, 13.0, FontFamily::Proportional),
            (TextStyle::Small, 11.0, FontFamily::Proportional),
            (TextStyle::Heading, 16.0, FontFamily::Proportional),
            (TextStyle::Monospace, 12.5, FontFamily::Monospace),
        ] {
            style.text_styles.insert(text, FontId::new(size, family));
        }
    });
    for (theme, bg, text) in [
        (Theme::Light, LIGHT_SELECTION_BG, LIGHT_SELECTION_TEXT),
        (Theme::Dark, DARK_SELECTION_BG, DARK_SELECTION_TEXT),
    ] {
        ctx.style_mut_of(theme, |style| {
            style.visuals.selection.bg_fill = bg;
            style.visuals.selection.stroke = Stroke::new(1.0, text);
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui::Color32;

    /// The colours moved here from around the app keep the values they had, so nothing on
    /// screen changes colour by accident.
    #[test]
    fn colours_keep_their_values() {
        assert_eq!(PIN, Color32::from_rgb(200, 30, 60));
        assert_eq!(SEAM_COLOURS[0], Color32::from_rgb(0, 150, 136));
        assert_eq!(SEAM_COLOURS[7], Color32::from_rgb(190, 145, 0));
        assert_eq!(
            AXIS_COLOURS,
            [
                Color32::from_rgb(220, 60, 60),
                Color32::from_rgb(60, 170, 70),
                Color32::from_rgb(60, 110, 235),
            ]
        );
        assert_eq!(GIZMO_BRIGHT, Color32::from_rgb(255, 200, 0));
        assert_eq!(FORM_SRGB, [208, 200, 193]);
        assert_eq!(FABRIC, [0.888, 0.871, 0.839]);
        assert_eq!(SELECTED_FABRIC, [0.95, 0.55, 0.25]);
        assert_eq!(STATS_TEXT, Color32::from_gray(60));
        assert_eq!(GIZMO_SQUARE, Color32::from_white_alpha(170));
        assert_eq!(GIZMO_OUTLINE, Color32::from_gray(60));
        assert_eq!(READOUT_TEXT, Color32::from_gray(30));
        assert_eq!(SEAM_BADGE_TEXT, Color32::WHITE);
        let light = PatternPalette::new(false);
        assert_eq!(light.table, Color32::from_gray(250));
        assert_eq!(light.handle, Color32::from_rgb(50, 110, 220));
        let dark = PatternPalette::new(true);
        assert_eq!(dark.selected, Color32::from_rgb(255, 150, 90));
        assert_eq!(
            dark.pale_fill,
            Color32::from_rgba_unmultiplied(120, 160, 230, 22)
        );
    }

    #[test]
    fn install_sets_the_selection_highlight_for_light_and_dark() {
        let ctx = egui::Context::default();
        install(&ctx);
        let light = ctx.style_of(egui::Theme::Light);
        assert_eq!(light.visuals.selection.bg_fill, LIGHT_SELECTION_BG);
        assert_eq!(light.visuals.selection.stroke.color, LIGHT_SELECTION_TEXT);
        let dark = ctx.style_of(egui::Theme::Dark);
        assert_eq!(dark.visuals.selection.bg_fill, DARK_SELECTION_BG);
        assert_eq!(dark.visuals.selection.stroke.color, DARK_SELECTION_TEXT);
    }

    #[test]
    fn install_adds_the_icon_font() {
        let has_pen_icon = |installed: bool| {
            let ctx = egui::Context::default();
            if installed {
                install(&ctx);
            }
            // The fonts are built at the start of the next frame; its textures go unused.
            let mut output = ctx.run_ui(egui::RawInput::default(), |_| {});
            output.textures_delta.clear();
            let icon = crate::icons::ph::PEN_NIB.chars().next().unwrap();
            let font = egui::FontId::proportional(13.0);
            ctx.fonts_mut(|f| f.has_glyph(&font, icon))
        };
        assert!(
            !has_pen_icon(false),
            "egui's own fonts have no Phosphor icons"
        );
        assert!(has_pen_icon(true), "no glyph for the pen icon");
    }
}
