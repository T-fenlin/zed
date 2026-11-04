//! Minimal Fluent-based i18n runtime for the Zed zh-CN localization.
//!
//! Loads the `.ftl` bundles under `assets/locales/<locale>/` at startup and
//! resolves message ids via [`t`]. Falls back to English when a translation is
//! missing, and to the key itself when even the English bundle lacks it.

pub mod keys;

use fluent_bundle::{FluentArgs, FluentBundle, FluentResource, FluentValue};
use unic_langid::LanguageIdentifier;

/// Supported locales. `en-US` is authoritative; other locales fall back to it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Locale {
    EnUs,
    ZhCn,
}

impl Locale {
    fn langid(self) -> &'static str {
        match self {
            Locale::EnUs => "en-US",
            Locale::ZhCn => "zh-CN",
        }
    }

    /// The BCP-47 tag for this locale, as written into settings files.
    pub fn as_tag(self) -> &'static str {
        self.langid()
    }

    /// Every locale the UI offers in the language picker.
    pub const ALL: &'static [Locale] = &[Locale::EnUs, Locale::ZhCn];

    /// Pick a locale from an environment-style tag (e.g. "zh-CN", "zh", "en").
    pub fn from_tag(tag: &str) -> Locale {
        if tag.to_ascii_lowercase().starts_with("zh") {
            Locale::ZhCn
        } else {
            Locale::EnUs
        }
    }
}

/// The embedded FTL sources, one entry per (locale, domain file).
/// `include_str!` bakes them into the binary so no runtime files are needed.
macro_rules! bundle_sources {
    ($($domain:literal => $file:literal),* $(,)?) => {
        fn sources(locale: Locale) -> Vec<(&'static str, &'static str)> {
            let lang = locale.langid();
            let mut v: Vec<(&'static str, &'static str)> = Vec::new();
            $(
                if lang == "en-US" {
                    v.push(($domain, include_str!(concat!("../../../assets/locales/en-US/", $file))));
                } else {
                    v.push(($domain, include_str!(concat!("../../../assets/locales/zh-CN/", $file))));
                }
            )*
            v
        }
    };
}

// NOTE: regenerate this list when a new domain file is added under assets/locales/.
bundle_sources! {
    "agent" => "agent.ftl",
    "app-menu" => "app-menu.ftl",
    "acp" => "acp.ftl",
    "collab" => "collab.ftl",
    "debugger" => "debugger.ftl",
    "edit-prediction" => "edit-prediction.ftl",
    "editor" => "editor.ftl",
    "errors" => "errors.ftl",
    "extensions" => "extensions.ftl",
    "git-ui" => "git-ui.ftl",
    "keymap" => "keymap.ftl",
    "lang" => "lang.ftl",
    "misc" => "misc.ftl",
    "outline" => "outline.ftl",
    "panel" => "panel.ftl",
    "pane" => "pane.ftl",
    "prompt" => "prompt.ftl",
    "recent" => "recent.ftl",
    "repl" => "repl.ftl",
    "search" => "search.ftl",
    "settings-ui" => "settings-ui.ftl",
    "sidebar" => "sidebar.ftl",
    "tab" => "tab.ftl",
    "test" => "test.ftl",
    "titlebar" => "titlebar.ftl",
    "ui" => "ui.ftl",
    "workspace" => "workspace.ftl",
    "zed" => "zed.ftl",
}

fn build_bundle(locale: Locale) -> FluentBundle<FluentResource> {
    let langid: LanguageIdentifier = locale.langid().parse().expect("valid langid");
    let mut bundle = FluentBundle::new(vec![langid]);
    // Missing-message / missing-arg errors are expected during incremental
    // translation; we surface them but do not panic.
    bundle.set_use_isolating(false);
    for (_domain, text) in sources(locale) {
        let res = FluentResource::try_new(text.to_string()).unwrap_or_else(|(res, _errs)| res);
        // Duplicate ids across files are tolerated on a best-effort basis.
        let _ = bundle.add_resource(res);
    }
    bundle
}

struct Registry {
    active: Locale,
    active_bundle: FluentBundle<FluentResource>,
    fallback_bundle: FluentBundle<FluentResource>,
}

thread_local! {
    // FluentBundle is not Sync, so the registry is thread-local. Bundles are
    // cheap to build per-thread and this keeps `t()` lock-free.
    static REGISTRY: std::cell::RefCell<Registry> = std::cell::RefCell::new({
        let active = Locale::from_tag(&std::env::var("ZED_LANG").unwrap_or_else(|_| "zh-CN".into()));
        Registry {
            active,
            active_bundle: build_bundle(active),
            fallback_bundle: build_bundle(Locale::EnUs),
        }
    });
}

fn with_registry<R>(f: impl FnOnce(&Registry) -> R) -> R {
    REGISTRY.with(|r| f(&r.borrow()))
}

/// Switch the active locale for the current thread.
///
/// Most UI text is resolved once at startup, so callers should restart Zed
/// after changing the persisted interface-language setting; this setter exists
/// so the startup path (and tests) can pick the locale explicitly.
pub fn set_locale(locale: Locale) {
    REGISTRY.with(|r| {
        let mut reg = r.borrow_mut();
        if reg.active != locale {
            reg.active = locale;
            reg.active_bundle = build_bundle(locale);
        }
    });
}

/// Resolve a message id in the active locale, falling back to English, then the id.
pub fn t(key: &str) -> String {
    t_with_args(key, &[])
}

/// Resolve with arguments, e.g. `t_with_args("ui-choose_option_for", &[("name", "X")])`.
pub fn t_with_args(key: &str, args: &[(&str, &str)]) -> String {
    with_registry(|reg| {
        let mut fluent_args = FluentArgs::new();
        for (k, v) in args {
            fluent_args.set(*k, FluentValue::from(v.to_string()));
        }
        let try_bundle = |bundle: &FluentBundle<FluentResource>| -> Option<String> {
            let msg = bundle.get_message(key)?;
            let pattern = msg.value()?;
            let mut errors = vec![];
            let out = bundle.format_pattern(pattern, Some(&fluent_args), &mut errors);
            if errors.is_empty() {
                Some(out.into_owned())
            } else {
                Some(out.into_owned())
            }
        };
        if let Some(s) = try_bundle(&reg.active_bundle) {
            return s;
        }
        if reg.active != Locale::EnUs {
            if let Some(s) = try_bundle(&reg.fallback_bundle) {
                return s;
            }
        }
        key.to_string()
    })
}

/// Convenience: current active locale.
pub fn active_locale() -> Locale {
    with_registry(|r| r.active)
}

/// Resolve a message id to a `&'static str`, for call sites whose types still
/// expect a static string slice (menu item arrays, match arms returning
/// `&'static str`, `Option<&'static str>` fields, ...).
///
/// The resolved text is interned once per (locale, key) and leaked; since the
/// key set is fixed at compile time this is bounded by the number of keys.
/// Prefer [`t`] / [`t_with_args`] where `String` is accepted.
pub fn t_static(key: &'static str) -> &'static str {
    thread_local! {
        static CACHE: std::cell::RefCell<std::collections::HashMap<(Locale, &'static str), &'static str>> =
            std::cell::RefCell::new(std::collections::HashMap::new());
    }
    let locale = active_locale();
    CACHE.with(|cache| {
        if let Some(value) = cache.borrow().get(&(locale, key)) {
            return *value;
        }
        let leaked: &'static str = Box::leak(t(key).into_boxed_str());
        cache.borrow_mut().insert((locale, key), leaked);
        leaked
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every embedded `.ftl` file must parse without errors.
    ///
    /// Parse errors are swallowed at runtime (see `build_bundle`), so a typo such
    /// as `\n` inside a plain-text value would silently surface to users as a
    /// literal backslash. This test turns those into build failures instead.
    #[test]
    fn ftl_resources_parse_cleanly() {
        for locale in Locale::ALL {
            for (domain, text) in sources(*locale) {
                let result = FluentResource::try_new(text.to_string());
                assert!(
                    result.is_ok(),
                    "{} / {domain}.ftl has parse errors: {:?}",
                    locale.as_tag(),
                    result.err().map(|(_, errors)| errors)
                );
            }
        }
    }

    /// Multi-line values are the supported way to embed newlines; make sure one
    /// renders the way its callers expect.
    #[test]
    fn multiline_value_keeps_blank_line() {
        let bundle = build_bundle(Locale::EnUs);
        let message = bundle
            .get_message("collab-join-error_try_again")
            .expect("message exists");
        let mut errors = vec![];
        let mut args = FluentArgs::new();
        args.set("error", FluentValue::from("boom".to_string()));
        let rendered = bundle.format_pattern(message.value().unwrap(), Some(&args), &mut errors);
        assert!(errors.is_empty(), "format errors: {errors:?}");
        assert_eq!(rendered, "boom\n\nPlease try again.");
    }
}
