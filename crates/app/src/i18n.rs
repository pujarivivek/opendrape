//! User-visible strings. English (`en-US`) ships in M0; other languages are added as
//! `i18n/<lang>/opendrape.ftl` files with the same message IDs.

use i18n_embed::fluent::{FluentLanguageLoader, fluent_language_loader};
use rust_embed::RustEmbed;
use std::sync::LazyLock;

#[derive(RustEmbed)]
#[folder = "i18n"]
struct Localizations;

pub static LOADER: LazyLock<FluentLanguageLoader> = LazyLock::new(|| {
    let loader: FluentLanguageLoader = fluent_language_loader!();
    i18n_embed::LanguageLoader::load_fallback_language(&loader, &Localizations)
        .expect("en-US strings are embedded in the binary");
    // Fluent wraps placeables in Unicode isolation marks, which egui renders as boxes.
    loader.set_use_isolating(false);
    loader
});

/// Look up a user-visible string: `tr!("menu-help")`, `tr!("about-version", version = v)`.
#[macro_export]
macro_rules! tr {
    ($id:literal) => {
        i18n_embed_fl::fl!($crate::i18n::LOADER, $id)
    };
    ($id:literal, $($args:tt)*) => {
        i18n_embed_fl::fl!($crate::i18n::LOADER, $id, $($args)*)
    };
}

#[cfg(test)]
mod tests {
    #[test]
    fn placeholders_are_filled_without_bidi_isolation_marks() {
        let s = crate::tr!("about-graphics", name = "Test GPU", backend = "Metal");
        assert_eq!(s, "Graphics: Test GPU (Metal)");
        assert!(
            !s.contains(['\u{2068}', '\u{2069}']),
            "egui would draw isolation marks as boxes"
        );
    }
}
