//! Minimal i18n prototype: Fluent-based string lookup with runtime language switching.
//!
//! Prototype for wayfinder ticket 04 ("最小 Fluent 注入原型"). It wires a
//! `static_loader!` Fluent bundle into GPUI via a `Translator` app global and
//! exposes `t()` for the ui-layer injection point (plan B, ticket 01).
//!
//! Key convention (ticket 06): FTL keys are stable kebab-case ids declared in
//! [`keys`]; call sites look up by explicit key only (`t(keys::…::NAME)`). The
//! lookup walks the fluent-langneg chain (e.g. zh-CN -> zh -> en-US) and falls
//! back to the key itself when missing — English text lives in `en-US.ftl`,
//! never at call sites.

use fluent_templates::{static_loader, LanguageIdentifier, Loader};
use gpui::{App, Global, SharedString};
use std::sync::atomic::{AtomicU8, Ordering};

pub mod keys;

pub use fluent_templates::LanguageIdentifier as FluentLanguageIdentifier;

static_loader! {
    static LOCALES = {
        locales: "../../assets/locales",
        fallback_language: "en-US",
        customise: |bundle| bundle.set_use_isolating(false),
    };
}

/// Process-wide current UI language, kept in sync with [`Translator::set_locale`]
/// so the cx-free [`t`] API can resolve keys anywhere (menus, settings, renders).
/// 0 = en-US, 1 = zh-CN.
static CURRENT_LOCALE: AtomicU8 = AtomicU8::new(0);

fn locale_index(lang: &LanguageIdentifier) -> u8 {
    if lang == &"zh-CN".parse::<LanguageIdentifier>().unwrap() {
        1
    } else {
        0
    }
}

fn locale_from_index(index: u8) -> LanguageIdentifier {
    if index == 1 {
        "zh-CN".parse().unwrap()
    } else {
        "en-US".parse().unwrap()
    }
}

/// Look up `key` in `lang`, walking the fluent-langneg negotiation chain
/// (e.g. zh-CN -> zh -> en-US). Returns `None` when the key is missing entirely.
pub fn lookup(lang: &LanguageIdentifier, key: &str) -> Option<SharedString> {
    LOCALES.try_lookup(lang, key).map(SharedString::from)
}

/// Look up `key` in the current UI language, falling back to the key itself
/// when missing. This is the explicit-key call-site API: `t(keys::domain::NAME)`.
///
/// The lookup walks lang -> language family -> en-US.ftl (default language
/// resource) -> the key string, so missing keys surface visibly during
/// development while en-US users always see English text.
pub fn t(key: &str) -> SharedString {
    let lang = locale_from_index(CURRENT_LOCALE.load(Ordering::Relaxed));
    lookup(&lang, key).unwrap_or_else(|| key.into())
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
    /// entirely (no translation in any negotiated language).
    pub fn t(&self, key: &str) -> Option<SharedString> {
        lookup(&self.current_lang, key)
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
        // Keep the cx-free `t()` API in sync with the app-global Translator.
        CURRENT_LOCALE.store(locale_index(&self.current_lang), Ordering::Relaxed);
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
        assert_eq!(lookup(&zh, "app-menu-zoom-in").unwrap(), SharedString::from("放大"));
        assert_eq!(lookup(&zh, "app-menu-open-settings").unwrap(), SharedString::from("打开设置"));
    }

    #[test]
    fn missing_key_falls_back_to_none() {
        let zh: LanguageIdentifier = "zh-CN".parse().unwrap();
        assert!(lookup(&zh, "app-menu-does-not-exist").is_none());
        assert_eq!(LOCALES.lookup(&zh, "app-menu-zoom-in"), "放大");
    }

    #[test]
    fn translator_global_defaults_to_english() {
        let translator = Translator::new();
        assert_eq!(translator.current_lang, "en-US".parse::<LanguageIdentifier>().unwrap());
        // en-US is the default language resource: explicit keys resolve to
        // the English source strings from en-US.ftl.
        assert_eq!(
            translator.t(keys::app_menu::ZOOM_IN),
            Some(SharedString::from("Zoom In"))
        );
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
        assert_eq!(
            translator.t(keys::app_menu::ZOOM_IN),
            Some(SharedString::from("Zoom In"))
        );
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
            lookup(&zh, &slugify("Welcome to Zed")).unwrap(),
            SharedString::from("欢迎使用 Zed")
        );
        // Untranslated strings keep the original text.
        assert_eq!(lookup(&zh, &slugify("Something Untranslated")), None);
    }

    #[test]
    fn translate_with_falls_back_to_original_text() {
        let mut zh = Translator::new();
        zh.set_locale("zh-CN");
        // Hit: translated.
        assert_eq!(
            translate_with(&zh, "Welcome to Zed"),
            SharedString::from("欢迎使用 Zed")
        );
        // Miss: original text.
        assert_eq!(
            translate_with(&zh, "Something Untranslated"),
            SharedString::from("Something Untranslated")
        );

        // en-US looks up en-US.ftl: no slug keys there, so the original text
        // is returned unchanged.
        let en = Translator::new();
        assert_eq!(translate_with(&en, "Welcome to Zed"), SharedString::from("Welcome to Zed"));
        assert_eq!(translate_with(&en, "Any String"), SharedString::from("Any String"));
    }

    #[test]
    fn settings_ui_titles_are_translated() {
        let mut zh = Translator::new();
        zh.set_locale("zh-CN");
        // Settings page setting titles go through Label fallback; these keys
        // live in settings-ui.ftl.
        assert_eq!(translate_with(&zh, "Theme Mode"), SharedString::from("主题模式"));
        assert_eq!(translate_with(&zh, "Buffer Font Size"), SharedString::from("缓冲区字号"));
        assert_eq!(translate_with(&zh, "Language"), SharedString::from("语言"));
        assert_eq!(translate_with(&zh, "Vim Mode"), SharedString::from("Vim 模式"));
    }

    #[test]
    fn audit_keys_match_ftl_bundles() {
        use std::collections::BTreeSet;
        use std::fs;
        use std::path::Path;

        fn keys_of(path: &Path) -> BTreeSet<String> {
            let mut keys = BTreeSet::new();
            for line in fs::read_to_string(path).unwrap().lines() {
                if line.starts_with(char::is_whitespace) || line.starts_with('#') || line.is_empty()
                {
                    continue;
                }
                if let Some(eq) = line.find('=') {
                    let key = line[..eq].trim();
                    if !key.is_empty()
                        && key.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
                    {
                        keys.insert(key.to_owned());
                    }
                }
            }
            keys
        }

        fn keys_in_dir(dir: &Path) -> BTreeSet<String> {
            let mut keys = BTreeSet::new();
            for entry in fs::read_dir(dir).unwrap() {
                let path = entry.unwrap().path();
                if path.extension().is_some_and(|ext| ext == "ftl") {
                    keys.extend(keys_of(&path));
                }
            }
            keys
        }

        let locales = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/locales");
        let declared: BTreeSet<String> = keys::all().into_iter().map(str::to_owned).collect();
        let en_us = keys_in_dir(&locales.join("en-US"));
        let zh_cn = keys_in_dir(&locales.join("zh-CN"));
        let strict = std::env::var_os("I18N_AUDIT_STRICT").is_some();

        let mut failed = false;

        // en-US is the default language resource: every key must resolve, and
        // orphan keys are not allowed (hard failure).
        let en_missing: Vec<_> = declared.difference(&en_us).cloned().collect();
        if !en_missing.is_empty() {
            println!(
                "[i18n audit] en-US missing {} key(s): {en_missing:?}",
                en_missing.len()
            );
            failed = true;
        }
        let en_orphans: Vec<_> = en_us.difference(&declared).cloned().collect();
        if !en_orphans.is_empty() {
            println!("[i18n audit] en-US orphan key(s): {en_orphans:?}");
            failed = true;
        }

        // Translation languages report by default; hard-fail with
        // I18N_AUDIT_STRICT=1 (e.g. before a release).
        for (lang, ftl) in [("zh-CN", &zh_cn)] {
            let missing: Vec<_> = declared.difference(ftl).cloned().collect();
            if !missing.is_empty() {
                println!(
                    "[i18n audit] {lang} missing {} key(s): {missing:?}",
                    missing.len()
                );
                if strict {
                    failed = true;
                }
            }
            let orphans: Vec<_> = ftl.difference(&declared).cloned().collect();
            if !orphans.is_empty() {
                let shown: Vec<_> = orphans.iter().take(50).collect();
                println!(
                    "[i18n audit] {lang} has {} orphan key(s) (first 50): {shown:?}",
                    orphans.len()
                );
                if strict {
                    failed = true;
                }
            }
        }

        if failed {
            panic!(
                "i18n audit failed — see messages above. \
                 Run with I18N_AUDIT_STRICT=1 to enforce translation languages."
            );
        }
    }
}
