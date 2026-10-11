//! How the 3D view should look, remembered between launches in `view.json` beside the GPU
//! state. Like that file, it is read and written quietly: a locked-down lab PC still starts.

use opendrape_render::studio::Lighting;
use opendrape_render::studio::quality::Quality;
use serde::{Deserialize, Serialize};
use std::path::Path;

const FILE: &str = "view.json";

/// The 3D quality the student picked: Auto follows the graphics chip.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum QualityChoice {
    #[default]
    Auto,
    Basic,
    Medium,
    High,
}

impl QualityChoice {
    pub const ALL: [Self; 4] = [Self::Auto, Self::Basic, Self::Medium, Self::High];

    /// The level to draw at, `auto` being the one the graphics chip suggests.
    pub fn resolve(self, auto: Quality) -> Quality {
        match self {
            Self::Auto => auto,
            Self::Basic => Quality::Basic,
            Self::Medium => Quality::Medium,
            Self::High => Quality::High,
        }
    }
}

/// How the studio balances its key light against its soft fill (View → Lighting).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LightingChoice {
    Soft,
    Balanced,
    #[default]
    Sculpted,
}

impl LightingChoice {
    pub const ALL: [Self; 3] = [Self::Soft, Self::Balanced, Self::Sculpted];

    pub fn lighting(self) -> Lighting {
        match self {
            Self::Soft => Lighting::Soft,
            Self::Balanced => Lighting::Balanced,
            Self::Sculpted => Lighting::Sculpted,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ViewSettings {
    #[serde(default)]
    pub quality: QualityChoice,
    #[serde(default)]
    pub lighting: LightingChoice,
    /// The dress form's measuring tapes are drawn (Assets → Show measuring tapes); its seams
    /// always are.
    #[serde(default)]
    pub show_tapes: bool,
}

impl ViewSettings {
    /// The settings in `dir`, or the defaults when there are none (or they can't be read).
    pub fn load(dir: Option<&Path>) -> Self {
        dir.and_then(|d| std::fs::read(d.join(FILE)).ok())
            .and_then(|bytes| serde_json::from_slice(&bytes).ok())
            .unwrap_or_default()
    }

    /// Writes the settings to `dir` (a temporary file, then renamed, so a power cut never
    /// leaves half a file). Returns whether they reached the disk.
    pub fn save(&self, dir: Option<&Path>) -> bool {
        let Some(dir) = dir else { return false };
        let _ = std::fs::create_dir_all(dir);
        let Ok(json) = serde_json::to_vec_pretty(self) else {
            return false;
        };
        let (path, tmp) = (dir.join(FILE), dir.join("view.json.tmp"));
        std::fs::write(&tmp, json).is_ok() && std::fs::rename(&tmp, &path).is_ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settings_round_trip() {
        let dir = tempfile::tempdir().unwrap();
        let s = ViewSettings {
            quality: QualityChoice::Basic,
            lighting: LightingChoice::Soft,
            show_tapes: false,
        };
        assert!(s.save(Some(dir.path())));
        assert_eq!(ViewSettings::load(Some(dir.path())), s);
        let text = std::fs::read_to_string(dir.path().join("view.json")).unwrap();
        assert!(text.contains("basic"), "{text}");
    }

    #[test]
    fn a_missing_or_broken_file_means_auto() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(
            ViewSettings::load(Some(dir.path())).quality,
            QualityChoice::Auto
        );
        std::fs::write(dir.path().join("view.json"), "{ not json").unwrap();
        assert_eq!(
            ViewSettings::load(Some(dir.path())).quality,
            QualityChoice::Auto
        );
        assert_eq!(ViewSettings::load(None).quality, QualityChoice::Auto);
        assert!(!ViewSettings::default().save(None), "nowhere to save");
    }

    #[test]
    fn lighting_is_sculpted_unless_chosen_and_old_files_still_load() {
        use opendrape_render::studio::Lighting;
        assert_eq!(ViewSettings::default().lighting, LightingChoice::Sculpted);
        assert_eq!(LightingChoice::Soft.lighting(), Lighting::Soft);
        assert_eq!(LightingChoice::Balanced.lighting(), Lighting::Balanced);
        let dir = tempfile::tempdir().unwrap();
        // Saved before lighting was a setting.
        std::fs::write(dir.path().join("view.json"), r#"{ "quality": "basic" }"#).unwrap();
        let s = ViewSettings::load(Some(dir.path()));
        assert_eq!(s.quality, QualityChoice::Basic);
        assert_eq!(s.lighting, LightingChoice::Sculpted);
    }

    #[test]
    fn measuring_tapes_are_hidden_unless_turned_on_and_old_files_still_load() {
        // The approved forms show their seams, not their measuring tapes.
        assert!(!ViewSettings::default().show_tapes);
        let dir = tempfile::tempdir().unwrap();
        // Saved before measuring tapes were a setting.
        std::fs::write(dir.path().join("view.json"), r#"{ "quality": "basic" }"#).unwrap();
        assert!(!ViewSettings::load(Some(dir.path())).show_tapes);
        let on = ViewSettings {
            show_tapes: true,
            ..ViewSettings::default()
        };
        assert!(on.save(Some(dir.path())));
        assert!(ViewSettings::load(Some(dir.path())).show_tapes);
    }

    #[test]
    fn auto_follows_the_graphics_chip_and_the_rest_are_fixed() {
        use opendrape_render::studio::quality::Quality;
        assert_eq!(QualityChoice::Auto.resolve(Quality::High), Quality::High);
        assert_eq!(QualityChoice::Basic.resolve(Quality::High), Quality::Basic);
        assert_eq!(QualityChoice::High.resolve(Quality::Basic), Quality::High);
    }
}
