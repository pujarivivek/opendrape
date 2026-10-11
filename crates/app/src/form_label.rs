//! The woven size label on the dress form: OpenDrape's name, the form and its size, and its
//! girths, light on navy, drawn into a picture the 3D view puts on the form's front.

use crate::tr;
use egui::epaint::text::{FontDefinitions, Fonts, TextOptions};
use egui::{Color32, FontId};
use opendrape_core::{FormChoice, FormSize, Units};

/// The label's picture, in pixels: as wide for its height as the label on the form.
pub const LABEL_PX: [u32; 2] = [512, 192];
/// The label's navy weave and its lettering (sRGB).
pub const NAVY: [u8; 3] = [52, 64, 98];
const LETTERING: [u8; 3] = [236, 232, 224];
/// Each line's height in the picture (px), and where its middle is.
const LINES: [(f32, f32); 3] = [(58.0, 52.0), (30.0, 112.0), (24.0, 152.0)];

/// The label's three lines: the name, the form and its size, and its girths in `units`.
pub fn lines(choice: &FormChoice, units: Units) -> [String; 3] {
    let form = match choice.id.as_str() {
        "men-torso" => tr!("label-form-men"),
        _ => tr!("label-form-women"),
    };
    let size = match &choice.size {
        // "US 8" says what it is; a bare "40" is a size.
        FormSize::Chart { label, .. } if label.starts_with(|c: char| c.is_ascii_digit()) => {
            tr!("label-size", size = label.as_str())
        }
        FormSize::Chart { label, .. } => label.clone(),
        FormSize::Custom => tr!("label-custom"),
    };
    let mm = |name: &str| choice.measurements.get(name).copied().unwrap_or(0.0);
    let short = |v: f64| {
        let text = units.format_number(v);
        let trimmed = text.trim_end_matches('0').trim_end_matches('.');
        if text.contains('.') {
            trimmed.to_string()
        } else {
            text
        }
    };
    let (waist, hip, unit) = (short(mm("waist")), short(mm("hip")), units.suffix());
    let girths = if choice.measurements.contains_key("chest") {
        tr!(
            "label-girths-chest",
            chest = short(mm("chest")),
            waist = waist,
            hip = hip,
            unit = unit
        )
    } else {
        tr!(
            "label-girths-bust",
            bust = short(mm("bust")),
            waist = waist,
            hip = hip,
            unit = unit
        )
    };
    [
        tr!("label-brand"),
        tr!("label-form-and-size", form = form, size = size),
        girths,
    ]
}

/// The label as a picture [`LABEL_PX`] large: the lines, centred, light on navy, inside a thin
/// woven edge.
pub fn picture(lines: &[String; 3]) -> image::RgbaImage {
    let [w, h] = LABEL_PX;
    let [nr, ng, nb] = NAVY;
    let mut img = image::RgbaImage::from_pixel(w, h, image::Rgba([nr, ng, nb, 255]));
    // The woven edge: a slightly lighter line just inside the border.
    let edge = image::Rgba([nr + 22, ng + 22, nb + 22, 255]);
    for x in 6..w - 6 {
        for y in [6, 7, h - 8, h - 7] {
            img.put_pixel(x, y, edge);
        }
    }
    for y in 6..h - 6 {
        for x in [6, 7, w - 8, w - 7] {
            img.put_pixel(x, y, edge);
        }
    }
    let mut fonts = Fonts::new(TextOptions::default(), FontDefinitions::default());
    let mut view = fonts.with_pixels_per_point(1.0);
    let galleys: Vec<_> = lines
        .iter()
        .zip(LINES)
        .map(|(text, (size, middle))| {
            let galley =
                view.layout_no_wrap(text.clone(), FontId::proportional(size), Color32::WHITE);
            (galley, middle)
        })
        .collect();
    // The glyphs are drawn into the font atlas as they are laid out: read it after.
    let atlas = view.image();
    for (galley, middle) in galleys {
        let left = (w as f32 - galley.size().x) / 2.0;
        let top = middle - galley.size().y / 2.0;
        for row in &galley.rows {
            for glyph in &row.row.glyphs {
                let uv = glyph.uv_rect;
                let at =
                    egui::pos2(left, top) + row.pos.to_vec2() + glyph.pos.to_vec2() + uv.offset;
                for ty in uv.min[1]..uv.max[1] {
                    for tx in uv.min[0]..uv.max[0] {
                        let cover = atlas.pixels[ty as usize * atlas.size[0] + tx as usize].a();
                        let x = (at.x + f32::from(tx - uv.min[0])).round() as i64;
                        let y = (at.y + f32::from(ty - uv.min[1])).round() as i64;
                        if cover == 0 || x < 0 || y < 0 || x >= i64::from(w) || y >= i64::from(h) {
                            continue;
                        }
                        let p = img.get_pixel_mut(x as u32, y as u32);
                        let a = f32::from(cover) / 255.0;
                        for (channel, ink) in p.0.iter_mut().zip(LETTERING) {
                            let mixed = f32::from(*channel) * (1.0 - a) + f32::from(ink) * a;
                            *channel = mixed.round() as u8;
                        }
                    }
                }
            }
        }
    }
    img
}

#[cfg(test)]
mod tests {
    use super::*;
    use opendrape_core::{FormChoice, FormSize, Units};

    #[test]
    fn the_label_names_the_form_its_size_and_its_girths() {
        assert_eq!(
            lines(&FormChoice::default(), Units::Cm),
            [
                "OpenDrape".to_string(),
                "WOMEN'S FORM · US 8".to_string(),
                "BUST 90 · WAIST 67.5 · HIP 93 cm".to_string(),
            ]
        );
        let men = opendrape_drape::choice::base_choice("men-torso").unwrap();
        let [_, size, girths] = lines(&men, Units::Cm);
        assert_eq!(size, "MEN'S FORM · SIZE 40");
        assert_eq!(girths, "CHEST 101.5 · WAIST 91 · HIP 98.5 cm");
        let custom = FormChoice {
            size: FormSize::Custom,
            ..FormChoice::default()
        };
        assert_eq!(lines(&custom, Units::Cm)[1], "WOMEN'S FORM · CUSTOM SIZE");
        assert_eq!(
            lines(&FormChoice::default(), Units::Inch)[2],
            "BUST 35.43 · WAIST 26.57 · HIP 36.61 in"
        );
    }

    #[test]
    fn the_picture_is_light_lettering_on_navy() {
        let lines = lines(&FormChoice::default(), Units::Cm);
        let img = picture(&lines);
        assert_eq!((img.width(), img.height()), (LABEL_PX[0], LABEL_PX[1]));
        assert_eq!(img.get_pixel(3, 3).0, [NAVY[0], NAVY[1], NAVY[2], 255]);
        let light = img
            .pixels()
            .filter(|p| p.0[0] > 150 && p.0[1] > 150 && p.0[2] > 150)
            .count() as f64
            / f64::from(img.width() * img.height());
        assert!((0.02..0.4).contains(&light), "lettering covers {light}");
        // Different words, a different picture.
        let men = opendrape_drape::choice::base_choice("men-torso").unwrap();
        assert_ne!(picture(&super::lines(&men, Units::Cm)), img);
    }
}
