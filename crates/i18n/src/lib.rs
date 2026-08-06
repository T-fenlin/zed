//! Minimal i18n prototype: Fluent-based string lookup with runtime language switching.
//!
//! Prototype for wayfinder ticket 04 ("最小 Fluent 注入原型"). It wires a
//! `static_loader!` Fluent bundle into GPUI via a `Translator` app global and
//! exposes `t()` for the ui-layer injection point (plan B, ticket 01).
//!
//! Key convention: FTL keys are kebab-case ids; call sites pass the English
//! source string as fallback when a key is missing (`t()` returns `None`).

use fluent_templates::{static_loader, LanguageIdentifier, Loader};
use gpui::{App, Global, SharedString};

pub use fluent_templates::LanguageIdentifier as FluentLanguageIdentifier;

static_loader! {
    static LOCALES = {
        locales: "../../assets/locales",
        fallback_language: "en-US",
        customise: |bundle| bundle.set_use_isolating(false),
    };
}

/// Look up `key` in `lang`, walking the fluent-langneg negotiation chain
/// (e.g. zh-CN -> zh -> en-US). Returns `None` when the key is missing entirely.
pub fn t(lang: &LanguageIdentifier, key: &str) -> Option<SharedString> {
    LOCALES.try_lookup(lang, key).map(SharedString::from)
}

/// Convert an English UI string to a kebab-case lookup key ("Zoom In" -> "zoom-in").
///
/// Used by the ui-layer fallback injection (ticket 05 step ①): strings that were
/// not explicitly rewritten with `t()` are looked up by their slugified form and
/// only replaced when a translation exists — "hit, then swap".
///
/// Note: slugs starting with a digit (e.g. "2x Zoom" -> "2x-zoom") are not
/// valid Fluent message identifiers and will never match — keep translation
/// keys letter-initial.
pub fn slugify(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut last_dash = false;
    for ch in text.chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch.to_ascii_lowercase());
            last_dash = false;
        } else if !last_dash && !out.is_empty() {
            out.push('-');
            last_dash = true;
        }
    }
    while out.ends_with('-') {
        out.pop();
    }
    out
}

/// Look up `text` in `translator`'s language by its slugified key. Returns the
/// translation when one exists, otherwise the original text unchanged.
///
/// This is the pure core of [`translate_text`]; kept separate so the fallback
/// logic is testable without a GPUI app context.
pub fn translate_with(translator: &Translator, text: &str) -> SharedString {
    // en-US is Zed's source language: en-US.ftl is intentionally empty, so
    // every lookup would miss. Short-circuit to avoid per-label slugify +
    // Fluent lookup cost on every frame in the default language.
    if translator.current_lang == "en-US".parse::<LanguageIdentifier>().unwrap() {
        return text.into();
    }
    let key = slugify(text);
    translator.t(&key).unwrap_or_else(|| text.into())
}

/// Look up `text` in the current UI language by its slugified key. Returns the
/// translation when one exists, otherwise the original text unchanged.
///
/// This is the ui-layer fallback (ticket 01 plan B, ticket 05 step ①): every
/// `Label` render goes through this, so un-rewritten strings pick up
/// translations automatically; untranslated strings keep their English source.
pub fn translate_text(text: &str, cx: &App) -> SharedString {
    let Some(translator) = cx.try_global::<Translator>() else {
        return text.into();
    };
    translate_with(translator, text)
}

/// App-global holder of the currently selected UI language.
///
/// Language switch = `update_global::<Translator>` with a new `current_lang`,
/// then refresh windows via `observe_global` (see ticket 02 — the
/// `SettingsStore` precedent in `crates/zed/src/zed.rs`).
pub struct Translator {
    pub current_lang: LanguageIdentifier,
}

impl Translator {
    pub fn new() -> Self {
        Self {
            current_lang: "en-US".parse().unwrap(),
        }
    }

    /// Look up `key` in the current language. `None` means the key is missing
    /// and the caller should fall back to its English source string.
    pub fn t(&self, key: &str) -> Option<SharedString> {
        t(&self.current_lang, key)
    }

    /// Toggle between the two supported prototype languages (en-US / zh-CN).
    pub fn toggle(&mut self) {
        self.set_locale(if self.current_lang == "en-US".parse::<LanguageIdentifier>().unwrap() {
            "zh-CN"
        } else {
            "en-US"
        });
    }

    /// Set the UI language from a settings `locale` string (e.g. "en-US",
    /// "zh-CN"). Unknown or unset values fall back to "en-US".
    pub fn set_locale(&mut self, locale: &str) {
        self.current_lang = match locale {
            "zh-CN" => "zh-CN".parse().unwrap(),
            _ => "en-US".parse().unwrap(),
        };
    }
}

impl Global for Translator {}

/// Install the [`Translator`] app global (defaults to `en-US`).
pub fn init(cx: &mut App) {
    cx.set_global(Translator::new());
}

#[cfg(test)]
mod tests {
    use super::*;
    use fluent_templates::Loader;

    #[test]
    fn lookup_zh_cn_translates_app_menu_keys() {
        let zh: LanguageIdentifier = "zh-CN".parse().unwrap();
        assert_eq!(t(&zh, "app-menu-zoom-in").unwrap(), SharedString::from("放大"));
        assert_eq!(t(&zh, "app-menu-open-settings").unwrap(), SharedString::from("打开设置"));
    }

    #[test]
    fn missing_key_falls_back_to_none() {
        let zh: LanguageIdentifier = "zh-CN".parse().unwrap();
        assert!(t(&zh, "app-menu-does-not-exist").is_none());
        assert_eq!(LOCALES.lookup(&zh, "app-menu-zoom-in"), "放大");
    }

    #[test]
    fn translator_global_defaults_to_english() {
        let translator = Translator::new();
        assert_eq!(translator.current_lang, "en-US".parse::<LanguageIdentifier>().unwrap());
        // en-US has no zh keys; english fallback means the key is absent.
        assert!(translator.t("app-menu-zoom-in").is_none());
    }

    #[test]
    fn toggle_switches_between_en_and_zh() {
        let mut translator = Translator::new();
        translator.toggle();
        assert_eq!(
            translator.current_lang,
            "zh-CN".parse::<LanguageIdentifier>().unwrap()
        );
        assert_eq!(translator.t("app-menu-zoom-in").unwrap(), SharedString::from("放大"));
        translator.toggle();
        assert_eq!(
            translator.current_lang,
            "en-US".parse::<LanguageIdentifier>().unwrap()
        );
        assert!(translator.t("app-menu-zoom-in").is_none());
    }

    #[test]
    fn set_locale_accepts_valid_values_and_falls_back() {
        let mut translator = Translator::new();
        translator.set_locale("zh-CN");
        assert_eq!(
            translator.current_lang,
            "zh-CN".parse::<LanguageIdentifier>().unwrap()
        );
        assert_eq!(translator.t("app-menu-zoom-in").unwrap(), SharedString::from("放大"));

        translator.set_locale("en-US");
        assert_eq!(
            translator.current_lang,
            "en-US".parse::<LanguageIdentifier>().unwrap()
        );

        // Unknown values fall back to en-US.
        translator.set_locale("fr-FR");
        assert_eq!(
            translator.current_lang,
            "en-US".parse::<LanguageIdentifier>().unwrap()
        );
    }

    #[test]
    fn slugify_converts_english_to_kebab_case() {
        assert_eq!(slugify("Zoom In"), "zoom-in");
        assert_eq!(slugify("New File..."), "new-file");
        assert_eq!(slugify("Reset All Zoom"), "reset-all-zoom");
        assert_eq!(slugify("  Toggle Left Dock  "), "toggle-left-dock");
        assert_eq!(slugify("Settings"), "settings");
        assert_eq!(slugify(""), "");
    }

    #[test]
    fn translate_text_looks_up_slugified_key() {
        let zh: LanguageIdentifier = "zh-CN".parse().unwrap();
        assert_eq!(
            t(&zh, &slugify("New File...")).unwrap(),
            SharedString::from("新建文件")
        );
        // Untranslated strings keep the original text.
        assert_eq!(t(&zh, &slugify("Something Untranslated")), None);
    }

    #[test]
    fn translate_with_falls_back_and_short_circuits_english() {
        let mut zh = Translator::new();
        zh.set_locale("zh-CN");
        // Hit: translated.
        assert_eq!(
            translate_with(&zh, "New File..."),
            SharedString::from("新建文件")
        );
        // Miss: original text.
        assert_eq!(
            translate_with(&zh, "Something Untranslated"),
            SharedString::from("Something Untranslated")
        );

        // en-US short-circuit: always the original text, no lookup cost.
        let en = Translator::new();
        assert_eq!(translate_with(&en, "New File..."), SharedString::from("New File..."));
        assert_eq!(translate_with(&en, "Any String"), SharedString::from("Any String"));
    }
}
