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
        self.current_lang = if self.current_lang
            == "en-US".parse::<LanguageIdentifier>().unwrap()
        {
            "zh-CN".parse().unwrap()
        } else {
            "en-US".parse().unwrap()
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
}
