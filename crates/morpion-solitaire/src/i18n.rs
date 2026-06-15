use i18n_embed::{
    fluent::{fluent_language_loader, FluentLanguageLoader},
    LanguageLoader,
};
use rust_embed::RustEmbed;
use std::sync::LazyLock;
use unic_langid::LanguageIdentifier;

#[derive(RustEmbed)]
#[folder = "locales/"]
pub struct Localizations;

pub static LANGUAGE_LOADER: LazyLock<FluentLanguageLoader> = LazyLock::new(|| {
    let loader: FluentLanguageLoader = fluent_language_loader!();
    loader
        .load_languages(&Localizations, &[loader.fallback_language().clone()])
        .expect("failed to load fallback language");
    // Don't wrap interpolated values in Unicode bidi isolation marks (FSI/PDI):
    // they show up as stray characters in the terminal and as tofu in egui.
    loader.set_use_isolating(false);
    loader
});

/// Switch the active UI language.  Silently falls back to French on error.
pub fn set_language(lang: &LanguageIdentifier) {
    let _ = i18n_embed::select(
        &*LANGUAGE_LOADER,
        &Localizations,
        std::slice::from_ref(lang),
    );
    // `select` rebuilds the bundles, which resets the isolating flag — turn it
    // off again so interpolated values aren't wrapped in bidi marks (FSI/PDI).
    LANGUAGE_LOADER.set_use_isolating(false);
}

/// Switch the active UI language from a BCP-47 tag (e.g. a browser's
/// `navigator.language`). Ignored if the tag doesn't parse.
pub fn set_locale(tag: &str) {
    if let Ok(lang) = tag.parse::<LanguageIdentifier>() {
        set_language(&lang);
    }
}

/// Languages bundled in `locales/`, sorted by tag — the source of truth for the
/// language switcher, so adding a `locales/<xx>/` folder makes it appear with no
/// code change.
pub fn available_languages() -> Vec<LanguageIdentifier> {
    let mut v = LANGUAGE_LOADER
        .available_languages(&Localizations)
        .unwrap_or_default();
    v.sort_by_key(|l| l.to_string());
    v
}

/// The currently active UI language.
pub fn current_language() -> LanguageIdentifier {
    LANGUAGE_LOADER
        .current_languages()
        .into_iter()
        .next()
        .unwrap_or_else(|| "en".parse().unwrap())
}

/// A language's own name (endonym), for the switcher. Falls back to the BCP-47
/// tag for any language not in the table.
pub fn language_endonym(lang: &LanguageIdentifier) -> String {
    match lang.language.as_str() {
        "en" => "English",
        "fr" => "Français",
        "de" => "Deutsch",
        "es" => "Español",
        "it" => "Italiano",
        "pt" => "Português",
        "nl" => "Nederlands",
        "ja" => "日本語",
        _ => return lang.to_string(),
    }
    .to_owned()
}

/// Detect the operating-system locale. Native only — the browser locale is
/// detected by the `morpion-solitaire-wasm` entry point, which keeps the library
/// free of any wasm-specific dependency.
#[cfg(not(target_arch = "wasm32"))]
pub fn detect_locale() -> LanguageIdentifier {
    sys_locale::get_locale()
        .and_then(|l| l.parse().ok())
        .unwrap_or_else(|| "fr".parse().unwrap())
}

#[cfg(test)]
mod tests {
    use super::*;
    use i18n_embed_fl::fl;

    // One test, sequenced: `set_language` mutates the shared LANGUAGE_LOADER, so
    // splitting these would let parallel tests race on the global state.
    #[test]
    fn locales_load_and_negotiate() {
        assert_eq!(available_languages().len(), 8);

        let de: LanguageIdentifier = "de".parse().unwrap();
        set_language(&de);
        assert_eq!(fl!(LANGUAGE_LOADER, "meta-author"), "Autor");

        // A region tag must negotiate down to the base language (de-DE → de).
        let de_de: LanguageIdentifier = "de-DE".parse().unwrap();
        set_language(&de_de);
        assert_eq!(fl!(LANGUAGE_LOADER, "meta-author"), "Autor");

        let ja: LanguageIdentifier = "ja".parse().unwrap();
        set_language(&ja);
        assert_eq!(fl!(LANGUAGE_LOADER, "meta-author"), "作者");
    }
}
