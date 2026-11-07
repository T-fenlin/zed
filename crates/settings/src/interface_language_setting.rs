use std::fmt::{Display, Formatter};

use crate::{self as settings, settings_content::InterfaceLanguageContent};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use settings::{RegisterSetting, Settings};

/// The language of the Zed user interface.
///
/// Default: system
#[derive(
    Copy, Clone, Debug, Serialize, Deserialize, JsonSchema, PartialEq, Eq, Default, RegisterSetting,
)]
#[serde(rename_all = "snake_case")]
pub enum InterfaceLanguage {
    /// Follow the system locale.
    #[default]
    System,
    /// English (en-US)
    EnUs,
    /// Simplified Chinese (zh-CN)
    ZhCn,
}

impl From<InterfaceLanguageContent> for InterfaceLanguage {
    fn from(value: InterfaceLanguageContent) -> Self {
        match value {
            InterfaceLanguageContent::System => Self::System,
            InterfaceLanguageContent::EnUs => Self::EnUs,
            InterfaceLanguageContent::ZhCn => Self::ZhCn,
        }
    }
}

impl From<InterfaceLanguage> for InterfaceLanguageContent {
    fn from(value: InterfaceLanguage) -> Self {
        match value {
            InterfaceLanguage::System => Self::System,
            InterfaceLanguage::EnUs => Self::EnUs,
            InterfaceLanguage::ZhCn => Self::ZhCn,
        }
    }
}

impl Display for InterfaceLanguage {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            InterfaceLanguage::System => write!(f, "Follow System"),
            InterfaceLanguage::EnUs => write!(f, "English"),
            InterfaceLanguage::ZhCn => write!(f, "简体中文"),
        }
    }
}

impl InterfaceLanguage {
    pub const OPTIONS: [(&'static str, Self); 3] = [
        ("Follow System", Self::System),
        ("English", Self::EnUs),
        ("简体中文", Self::ZhCn),
    ];

    pub fn names() -> impl Iterator<Item = &'static str> {
        Self::OPTIONS.iter().map(|(name, _)| *name)
    }

    pub fn from_names(option: &str) -> InterfaceLanguage {
        Self::OPTIONS
            .iter()
            .copied()
            .find_map(|(name, value)| (name == option).then_some(value))
            .unwrap_or_default()
    }
}

impl Settings for InterfaceLanguage {
    fn from_settings(s: &crate::settings_content::SettingsContent) -> Self {
        s.interface_language.unwrap_or_default().into()
    }
}
