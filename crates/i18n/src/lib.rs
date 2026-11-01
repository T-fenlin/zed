//! Minimal Fluent-based i18n runtime for the Zed zh-CN localization.
//!
//! Loads the `.ftl` bundles under `assets/locales/<locale>/` at startup and
//! resolves message ids via [`t`]. Falls back to English when a translation is
//! missing, and to the key itself when even the English bundle lacks it.

pub mod keys;


use fluent_bundle::{FluentArgs, FluentBundle, FluentResource, FluentValue};
use unic_langid::LanguageIdentifier;

/// Supported locales. `en-US` is authoritative; other locales fall back to it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
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
        let res = FluentResource::try_new(text.to_string())
            .unwrap_or_else(|(res, _errs)| res);
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
    static REGISTRY: Registry = {
        let active = Locale::from_tag(&std::env::var("ZED_LANG").unwrap_or_else(|_| "zh-CN".into()));
        Registry {
            active,
            active_bundle: build_bundle(active),
            fallback_bundle: build_bundle(Locale::EnUs),
        }
    };
}

fn with_registry<R>(f: impl FnOnce(&Registry) -> R) -> R {
    REGISTRY.with(|r| f(r))
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
        if errors.is_empty() { Some(out.into_owned()) } else { Some(out.into_owned()) }
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
