use strum::{EnumDiscriminants, EnumMessage};

/// The UI language selection (i18n prototype: en-US / zh-CN).
///
/// Stored in settings as a string (`"locale": "zh-CN"`); this enum is used by
/// the settings UI to render the language picker.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, EnumMessage, EnumDiscriminants)]
#[strum_discriminants(derive(strum::VariantArray, strum::VariantNames, strum::FromRepr))]
pub enum LocaleSelection {
    /// English (Zed's built-in UI language).
    #[default]
    #[strum(message = "English")]
    EnUs,
    /// Simplified Chinese.
    #[strum(message = "简体中文")]
    ZhCn,
}

impl LocaleSelection {
    /// The `settings.json` string value for this locale.
    pub fn as_str(self) -> &'static str {
        match self {
            LocaleSelection::EnUs => "en-US",
            LocaleSelection::ZhCn => "zh-CN",
        }
    }
}

/// Index of the [`LocaleSelection`] discriminant matching a settings `locale`
/// string; unknown or unset values map to `en-US`.
pub fn locale_discriminant_index(locale: &Option<String>) -> usize {
    match locale.as_deref() {
        Some("zh-CN") => LocaleSelectionDiscriminants::ZhCn as usize,
        _ => LocaleSelectionDiscriminants::EnUs as usize,
    }
}
