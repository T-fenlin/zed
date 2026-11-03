use gpui::SharedString;
use gpui::{Action as _, App};
use i18n::{keys, t};
use itertools::Itertools as _;
use settings::{
    AudioInputDeviceName, AudioOutputDeviceName, EditPredictionDataCollectionChoice,
    LanguageSettingsContent, SemanticTokens, SettingsContent,
};
use std::sync::{Arc, OnceLock};
use strum::{EnumMessage, IntoDiscriminant as _, VariantArray};
use theme::SystemAppearance;
use ui::IntoElement;

use crate::{
    ActionLink, DynamicItem, PROJECT, SettingField, SettingItem, SettingsFieldMetadata,
    SettingsPage, SettingsPageItem, SubPageLink, USER, active_language, all_language_names,
    pages::{
        open_audio_test_window, render_edit_prediction_setup_page, render_external_agents_page,
        render_llm_providers_page, render_mcp_servers_page, render_sandbox_settings_page,
        render_skills_setup_page, render_tool_permissions_setup_page,
    },
};

const DEFAULT_STRING: String = String::new();
/// A default empty string reference. Useful in `pick` functions for cases either in dynamic item fields, or when dealing with `settings::Maybe`
/// to avoid the "NO DEFAULT" case.
const DEFAULT_EMPTY_STRING: Option<&String> = Some(&DEFAULT_STRING);

const DEFAULT_AUDIO_OUTPUT: AudioOutputDeviceName = AudioOutputDeviceName(None);
const DEFAULT_EMPTY_AUDIO_OUTPUT: Option<&AudioOutputDeviceName> = Some(&DEFAULT_AUDIO_OUTPUT);
const DEFAULT_AUDIO_INPUT: AudioInputDeviceName = AudioInputDeviceName(None);
const DEFAULT_EMPTY_AUDIO_INPUT: Option<&AudioInputDeviceName> = Some(&DEFAULT_AUDIO_INPUT);

macro_rules! concat_sections {
    (@vec, $($arr:expr),+ $(,)?) => {{
        let total_len = 0_usize $(+ $arr.len())+;
        let mut out = Vec::with_capacity(total_len);

        $(
            out.extend($arr);
        )+

        out
    }};

    ($($arr:expr),+ $(,)?) => {{
        let total_len = 0_usize $(+ $arr.len())+;

        let mut out: Box<[std::mem::MaybeUninit<_>]> = Box::new_uninit_slice(total_len);

        let mut index = 0usize;
        $(
            let array = $arr;
            for item in array {
                out[index].write(item);
                index += 1;
            }
        )+

        debug_assert_eq!(index, total_len);

        // SAFETY: we wrote exactly `total_len` elements.
        unsafe { out.assume_init() }
    }};
}

pub(crate) fn settings_data(cx: &App) -> Vec<SettingsPage> {
    vec![
        general_page(cx),
        appearance_page(),
        keymap_page(),
        editor_page(),
        languages_and_tools_page(cx),
        search_and_files_page(),
        window_and_layout_page(),
        panels_page(),
        debugger_page(),
        terminal_page(),
        version_control_page(),
        collaboration_page(),
        ai_page(cx),
        network_page(),
        developer_page(cx),
    ]
}

fn developer_page(cx: &App) -> SettingsPage {
    use feature_flags::FeatureFlagAppExt as _;

    let mut items: Vec<SettingsPageItem> = Vec::new();

    // Feature flag overrides are a staff-only affordance, so only surface the section when the overrides are enabled.
    if cx.feature_flag_overrides_enabled() {
        items.push(SettingsPageItem::SectionHeader(
            t(keys::settings_ui::SETTINGS_DEVELOPER_SECTION_FEATURE_FLAGS).into(),
        ));
        items.push(SettingsPageItem::SubPageLink(SubPageLink {
            title: t(keys::settings_ui::SETTINGS_DEVELOPER_SECTION_FEATURE_FLAGS).into(),
            r#type: Default::default(),
            description: None,
            search_aliases: &[],
            json_path: Some("feature_flags"),
            in_json: true,
            files: USER,
            render: crate::pages::render_feature_flags_page,
        }));
    }

    items.push(SettingsPageItem::SectionHeader(
        t(keys::settings_ui::SETTINGS_DEVELOPER_SECTION_INSTRUMENTATION).into(),
    ));
    items.push(SettingsPageItem::SettingItem(SettingItem {
        title: t(keys::settings_ui::SETTINGS_DEVELOPER_PERFORMANCE_PROFILER).into(),
        description: t(
            keys::settings_ui::SETTINGS_DEVELOPER_COLLECT_TIMING_DATA_FOR_FOREGROUND_AND_B,
        )
        .into(),
        field: Box::new(SettingField {
            organization_override: None,
            json_path: Some("instrumentation.performance_profiler.enabled"),
            pick: |settings_content| {
                settings_content
                    .instrumentation
                    .as_ref()
                    .and_then(|i| i.performance_profiler.as_ref())
                    .and_then(|p| p.enabled.as_ref())
            },
            write: |settings_content, value, _| {
                settings_content
                    .instrumentation
                    .get_or_insert_default()
                    .performance_profiler
                    .get_or_insert_default()
                    .enabled = value;
            },
        }),
        metadata: None,
        files: USER,
    }));

    SettingsPage {
        title: t(keys::settings_ui::SETTINGS_DEVELOPER_DEVELOPER).into(),
        items: items.into_boxed_slice(),
    }
}

fn general_page(cx: &App) -> SettingsPage {
    fn general_settings_section(_cx: &App) -> Vec<SettingsPageItem> {
        vec![
            SettingsPageItem::SectionHeader(t(keys::settings_ui::SETTINGS_GENERAL_SETTINGS).into()),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_ACCESSIBLE_MODE).into(),
                description: t(keys::settings_ui::SETTINGS_ACCESSIBLE_MODE_DESCRIPTION).into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("accessible_mode"),
                    pick: |settings_content| settings_content.workspace.accessible_mode.as_ref(),
                    write: |settings_content, value, _| {
                        settings_content.workspace.accessible_mode = value;
                    },
                }),
                metadata: None,
                files: USER,
            }),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_WHEN_CLOSING_WITH_NO_TABS).into(),
                description: t(keys::settings_ui::SETTINGS_WHEN_CLOSING_WITH_NO_TABS_DESCRIPTION).into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("when_closing_with_no_tabs"),
                    pick: |settings_content| {
                        settings_content
                            .workspace
                            .when_closing_with_no_tabs
                            .as_ref()
                    },
                    write: |settings_content, value, _| {
                        settings_content.workspace.when_closing_with_no_tabs = value;
                    },
                }),
                metadata: None,
                files: USER,
            }),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_ON_NEW_WINDOW).into(),
                description: "What to show when opening a new window; Readme opens a project's README when no items are open, leaving the pane empty if none is found.".into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("on_new_window"),
                    pick: |settings_content| settings_content.workspace.on_new_window.as_ref(),
                    write: |settings_content, value, _| {
                        settings_content.workspace.on_new_window = value;
                    },
                }),
                metadata: None,
                files: USER,
            }),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_ON_LAST_WINDOW_CLOSED).into(),
                description: t(keys::settings_ui::SETTINGS_ON_LAST_WINDOW_CLOSED_DESCRIPTION).into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("on_last_window_closed"),
                    pick: |settings_content| {
                        settings_content.workspace.on_last_window_closed.as_ref()
                    },
                    write: |settings_content, value, _| {
                        settings_content.workspace.on_last_window_closed = value;
                    },
                }),
                metadata: None,
                files: USER,
            }),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_USE_SYSTEM_PATH_PROMPTS).into(),
                description: t(keys::settings_ui::SETTINGS_USE_SYSTEM_PATH_PROMPTS_DESCRIPTION).into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("use_system_path_prompts"),
                    pick: |settings_content| {
                        settings_content.workspace.use_system_path_prompts.as_ref()
                    },
                    write: |settings_content, value, _| {
                        settings_content.workspace.use_system_path_prompts = value;
                    },
                }),
                metadata: None,
                files: USER,
            }),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_USE_SYSTEM_PROMPTS).into(),
                description: t(keys::settings_ui::SETTINGS_USE_SYSTEM_PROMPTS_DESCRIPTION).into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("use_system_prompts"),
                    pick: |settings_content| settings_content.workspace.use_system_prompts.as_ref(),
                    write: |settings_content, value, _| {
                        settings_content.workspace.use_system_prompts = value;
                    },
                }),
                metadata: None,
                files: USER,
            }),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_REDACT_PRIVATE_VALUES).into(),
                description: t(keys::settings_ui::SETTINGS_REDACT_PRIVATE_VALUES_DESCRIPTION).into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("redact_private_values"),
                    pick: |settings_content| settings_content.editor.redact_private_values.as_ref(),
                    write: |settings_content, value, _| {
                        settings_content.editor.redact_private_values = value;
                    },
                }),
                metadata: None,
                files: USER,
            }),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_PRIVATE_FILES).into(),
                description: t(keys::settings_ui::SETTINGS_PRIVATE_FILES_DESCRIPTION).into(),
                field: Box::new(
                    SettingField {
                        organization_override: None,
                        json_path: Some("private_files"),
                        pick: |settings_content| {
                            settings_content.project.worktree.private_files.as_ref()
                        },
                        write: |settings_content, value, _| {
                            settings_content.project.worktree.private_files = value;
                        },
                    }
                    .unimplemented(),
                ),
                metadata: None,
                files: USER,
            }),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_CLI_DEFAULT_OPEN_BEHAVIOR).into(),
                description: t(keys::settings_ui::SETTINGS_CLI_DEFAULT_OPEN_BEHAVIOR_DESCRIPTION).into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("cli_default_open_behavior"),
                    pick: |settings_content| {
                        settings_content
                            .workspace
                            .cli_default_open_behavior
                            .as_ref()
                    },
                    write: |settings_content, value, _| {
                        settings_content.workspace.cli_default_open_behavior = value;
                    },
                }),
                metadata: Some(Box::new(SettingsFieldMetadata {
                    should_do_titlecase: Some(false),
                    ..Default::default()
                })),
                files: USER,
            }),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_GENERAL_REVEAL_IF_OPEN).into(),
                description: t(keys::settings_ui::SETTINGS_GENERAL_WHEN_ENABLED_ZED_WILL_PREFER_ALREADY_OPE).into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("reveal_if_open"),
                    pick: |settings_content| settings_content.workspace.reveal_if_open.as_ref(),
                    write: |settings_content, value, _| {
                        settings_content.workspace.reveal_if_open = value;
                    },
                }),
                metadata: None,
                files: USER,
            }),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_GENERAL_DEFAULT_OPEN_BEHAVIOR).into(),
                description: t(keys::settings_ui::SETTINGS_GENERAL_HOW_PROJECTS_OPEN_FROM_THE_UI_BY_DEFAULT).into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("default_open_behavior"),
                    pick: |settings_content| {
                        settings_content.workspace.default_open_behavior.as_ref()
                    },
                    write: |settings_content, value, _| {
                        settings_content.workspace.default_open_behavior = value;
                    },
                }),
                metadata: Some(Box::new(SettingsFieldMetadata {
                    should_do_titlecase: Some(false),
                    ..Default::default()
                })),
                files: USER,
            }),
        ]
    }
    fn security_section() -> [SettingsPageItem; 2] {
        [
            SettingsPageItem::SectionHeader(
                t(keys::settings_ui::SETTINGS_GENERAL_SECTION_SECURITY).into(),
            ),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_GENERAL_TRUST_ALL_PROJECTS_BY_DEFAULT).into(),
                description: t(
                    keys::settings_ui::SETTINGS_GENERAL_WHEN_OPENING_ZED_AVOID_RESTRICTED_MODE_B,
                )
                .into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("session.trust_all_projects"),
                    pick: |settings_content| {
                        settings_content
                            .session
                            .as_ref()
                            .and_then(|session| session.trust_all_worktrees.as_ref())
                    },
                    write: |settings_content, value, _| {
                        settings_content
                            .session
                            .get_or_insert_default()
                            .trust_all_worktrees = value;
                    },
                }),
                metadata: None,
                files: USER,
            }),
        ]
    }

    fn workspace_restoration_section() -> [SettingsPageItem; 3] {
        [
            SettingsPageItem::SectionHeader(
                t(keys::settings_ui::SETTINGS_GENERAL_SECTION_WORKSPACE_RESTORATION).into(),
            ),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_GENERAL_RESTORE_UNSAVED_BUFFERS).into(),
                description: t(
                    keys::settings_ui::SETTINGS_GENERAL_WHETHER_OR_NOT_TO_RESTORE_UNSAVED_BUFFER,
                )
                .into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("session.restore_unsaved_buffers"),
                    pick: |settings_content| {
                        settings_content
                            .session
                            .as_ref()
                            .and_then(|session| session.restore_unsaved_buffers.as_ref())
                    },
                    write: |settings_content, value, _| {
                        settings_content
                            .session
                            .get_or_insert_default()
                            .restore_unsaved_buffers = value;
                    },
                }),
                metadata: None,
                files: USER,
            }),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_GENERAL_RESTORE_ON_STARTUP).into(),
                description: t(
                    keys::settings_ui::SETTINGS_GENERAL_WHAT_TO_RESTORE_FROM_THE_PREVIOUS_SESSIO,
                )
                .into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("restore_on_startup"),
                    pick: |settings_content| settings_content.workspace.restore_on_startup.as_ref(),
                    write: |settings_content, value, _| {
                        settings_content.workspace.restore_on_startup = value;
                    },
                }),
                metadata: None,
                files: USER,
            }),
        ]
    }

    fn scoped_settings_section() -> [SettingsPageItem; 3] {
        [
            SettingsPageItem::SectionHeader(
                t(keys::settings_ui::SETTINGS_GENERAL_SECTION_SCOPED_SETTINGS).into(),
            ),
            SettingsPageItem::SettingItem(SettingItem {
                files: USER,
                title: t(keys::settings_ui::SETTINGS_GENERAL_PREVIEW_CHANNEL).into(),
                description: t(
                    keys::settings_ui::SETTINGS_GENERAL_WHICH_SETTINGS_SHOULD_BE_ACTIVATED_ONLY,
                )
                .into(),
                field: Box::new(
                    SettingField {
                        organization_override: None,
                        json_path: Some("preview_channel_settings"),
                        pick: |settings_content| Some(settings_content),
                        write: |_settings_content, _value, _| {},
                    }
                    .unimplemented(),
                ),
                metadata: None,
            }),
            SettingsPageItem::SettingItem(SettingItem {
                files: USER,
                title: t(keys::settings_ui::SETTINGS_GENERAL_SETTINGS_PROFILES).into(),
                description: t(
                    keys::settings_ui::SETTINGS_GENERAL_ANY_NUMBER_OF_SETTINGS_PROFILES_THAT_ARE,
                )
                .into(),
                field: Box::new(
                    SettingField {
                        organization_override: None,
                        json_path: Some("settings_profiles"),
                        pick: |settings_content| Some(settings_content),
                        write: |_settings_content, _value, _| {},
                    }
                    .unimplemented(),
                ),
                metadata: None,
            }),
        ]
    }

    fn privacy_section() -> [SettingsPageItem; 4] {
        [
            SettingsPageItem::SectionHeader(
                t(keys::edit_prediction::EDIT_PREDICTION_PRIVACY_HEADER).into(),
            ),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_GENERAL_TELEMETRY_DIAGNOSTICS).into(),
                description: t(
                    keys::settings_ui::SETTINGS_GENERAL_SEND_DEBUG_INFORMATION_LIKE_CRASH_REPORT,
                )
                .into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("telemetry.diagnostics"),
                    pick: |settings_content| {
                        settings_content
                            .telemetry
                            .as_ref()
                            .and_then(|telemetry| telemetry.diagnostics.as_ref())
                    },
                    write: |settings_content, value, _| {
                        settings_content
                            .telemetry
                            .get_or_insert_default()
                            .diagnostics = value;
                    },
                }),
                metadata: None,
                files: USER,
            }),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_GENERAL_TELEMETRY_METRICS).into(),
                description: t(
                    keys::settings_ui::SETTINGS_GENERAL_SEND_ANONYMIZED_USAGE_DATA_LIKE_WHAT_LAN,
                )
                .into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("telemetry.metrics"),
                    pick: |settings_content| {
                        settings_content
                            .telemetry
                            .as_ref()
                            .and_then(|telemetry| telemetry.metrics.as_ref())
                    },
                    write: |settings_content, value, _| {
                        settings_content.telemetry.get_or_insert_default().metrics = value;
                    },
                }),
                metadata: None,
                files: USER,
            }),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_GENERAL_ANTHROPIC_DATA_RETENTION).into(),
                description: t(
                    keys::settings_ui::SETTINGS_GENERAL_ALLOW_SENDING_REQUESTS_TO_ANTHROPIC_MODE,
                )
                .into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("telemetry.anthropic_retention"),
                    pick: |settings_content| {
                        settings_content
                            .telemetry
                            .as_ref()
                            .and_then(|telemetry| telemetry.anthropic_retention.as_ref())
                    },
                    write: |settings_content, value, _| {
                        settings_content
                            .telemetry
                            .get_or_insert_default()
                            .anthropic_retention = value;
                    },
                }),
                metadata: None,
                files: USER,
            }),
        ]
    }

    fn auto_update_section() -> [SettingsPageItem; 2] {
        [
            SettingsPageItem::SectionHeader(
                t(keys::settings_ui::SETTINGS_GENERAL_SECTION_AUTO_UPDATE).into(),
            ),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_GENERAL_SECTION_AUTO_UPDATE).into(),
                description: t(
                    keys::settings_ui::SETTINGS_GENERAL_WHETHER_OR_NOT_TO_AUTOMATICALLY_CHECK_FO,
                )
                .into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("auto_update"),
                    pick: |settings_content| settings_content.auto_update.as_ref(),
                    write: |settings_content, value, _| {
                        settings_content.auto_update = value;
                    },
                }),
                metadata: None,
                files: USER,
            }),
        ]
    }

    fn extensions_section() -> [SettingsPageItem; 2] {
        [
            SettingsPageItem::SectionHeader(t(keys::app_menu::MENU_ZED_EXTENSIONS).into()),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_GENERAL_SUGGEST_EXTENSIONS).into(),
                description: t(keys::settings_ui::SETTINGS_EXTENSIONS_SUGGEST_FOR_FILES).into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("suggest_extensions"),
                    pick: |settings_content| Some(&settings_content.extension.suggest_extensions),
                    write: |settings_content, value, _| {
                        settings_content.extension.suggest_extensions = value.unwrap_or(true);
                    },
                }),
                metadata: None,
                files: USER,
            }),
        ]
    }

    SettingsPage {
        title: t(keys::settings_ui::SETTINGS_GENERAL_GENERAL).into(),
        items: concat_sections!(
            @vec,
            general_settings_section(cx),
            security_section(),
            workspace_restoration_section(),
            scoped_settings_section(),
            privacy_section(),
            auto_update_section(),
            extensions_section(),
        )
        .into(),
    }
}

fn appearance_page() -> SettingsPage {
    fn theme_section() -> [SettingsPageItem; 3] {
        [
            SettingsPageItem::SectionHeader(t(keys::settings_ui::SETTINGS_APPEARANCE_SECTION_THEME).into()),
            SettingsPageItem::DynamicItem(DynamicItem {
                discriminant: SettingItem {
                    files: USER,
                    title: t(keys::settings_ui::SETTINGS_APPEARANCE_THEME_MODE).into(),
                    description: t(keys::settings_ui::SETTINGS_APPEARANCE_CHOOSE_A_STATIC_FIXED_THEME_OR_DYNAMICAL).into(),
                    field: Box::new(SettingField {
                        organization_override: None,
                        json_path: Some("theme$"),
                        pick: |settings_content| {
                            Some(&dynamic_variants::<settings::ThemeSelection>()[
                                settings_content
                                    .theme
                                    .theme
                                    .as_ref()?
                                    .discriminant() as usize])
                        },
                        write: |settings_content, value, app: &App| {
                            let Some(value) = value else {
                                settings_content.theme.theme = None;
                                return;
                            };
                            let settings_value = settings_content.theme.theme.get_or_insert_default();
                            *settings_value = match value {
                                settings::ThemeSelectionDiscriminants::Static => {
                                    let name = match settings_value {
                                        settings::ThemeSelection::Static(_) => return,
                                        settings::ThemeSelection::Dynamic { mode, light, dark } => {
                                            match mode {
                                                theme_settings::ThemeAppearanceMode::Light => light.clone(),
                                                theme_settings::ThemeAppearanceMode::Dark => dark.clone(),
                                                theme_settings::ThemeAppearanceMode::System => {
                                                    if SystemAppearance::global(app).is_light() {
                                                        light.clone()
                                                    } else {
                                                        dark.clone()
                                                    }
                                                }
                                            }
                                        },
                                    };
                                    settings::ThemeSelection::Static(name)
                                },
                                settings::ThemeSelectionDiscriminants::Dynamic => {
                                    let static_name = match settings_value {
                                        settings::ThemeSelection::Static(theme_name) => theme_name.clone(),
                                        settings::ThemeSelection::Dynamic {..} => return,
                                    };

                                    settings::ThemeSelection::Dynamic {
                                        mode: settings::ThemeAppearanceMode::System,
                                        light: static_name.clone(),
                                        dark: static_name,
                                    }
                                },
                            };
                        },
                    }),
                    metadata: None,
                },
                pick_discriminant: |settings_content| {
                    Some(settings_content.theme.theme.as_ref()?.discriminant() as usize)
                },
                fields: dynamic_variants::<settings::ThemeSelection>().into_iter().map(|variant| {
                    match variant {
                        settings::ThemeSelectionDiscriminants::Static => vec![
                            SettingItem {
                                files: USER,
                                title: t(keys::settings_ui::SETTINGS_APPEARANCE_THEME_NAME).into(),
                                description: t(keys::settings_ui::SETTINGS_APPEARANCE_THE_NAME_OF_YOUR_SELECTED_THEME).into(),
                                field: Box::new(SettingField {
                                    organization_override: None,
                                    json_path: Some("theme"),
                                    pick: |settings_content| {
                                        match settings_content.theme.theme.as_ref() {
                                            Some(settings::ThemeSelection::Static(name)) => Some(name),
                                            _ => None
                                        }
                                    },
                                    write: |settings_content, value, _| {
                                        let Some(value) = value else {
                                            return;
                                        };
                                        match settings_content
                                            .theme
                                            .theme.get_or_insert_default() {
                                                settings::ThemeSelection::Static(theme_name) => *theme_name = value,
                                                _ => return
                                            }
                                    },
                                }),
                                metadata: None,
                            }
                        ],
                        settings::ThemeSelectionDiscriminants::Dynamic => vec![
                            SettingItem {
                                files: USER,
                                title: t(keys::settings_ui::SETTINGS_APPEARANCE_MODE).into(),
                                description: t(keys::settings_ui::SETTINGS_APPEARANCE_CHOOSE_WHETHER_TO_USE_THE_SELECTED_LIGHT).into(),
                                field: Box::new(SettingField {
                                    organization_override: None,
                                    json_path: Some("theme.mode"),
                                    pick: |settings_content| {
                                        match settings_content.theme.theme.as_ref() {
                                            Some(settings::ThemeSelection::Dynamic { mode, ..}) => Some(mode),
                                            _ => None
                                        }
                                    },
                                    write: |settings_content, value, _| {
                                        let Some(value) = value else {
                                            return;
                                        };
                                        match settings_content
                                            .theme
                                            .theme.get_or_insert_default() {
                                                settings::ThemeSelection::Dynamic{ mode, ..} => *mode = value,
                                                _ => return
                                            }
                                    },
                                }),
                                metadata: None,
                            },
                            SettingItem {
                                files: USER,
                                title: t(keys::settings_ui::SETTINGS_APPEARANCE_LIGHT_THEME).into(),
                                description: t(keys::settings_ui::SETTINGS_APPEARANCE_THE_THEME_TO_USE_WHEN_MODE_IS_SET_TO_LIG).into(),
                                field: Box::new(SettingField {
                                    organization_override: None,
                                    json_path: Some("theme.light"),
                                    pick: |settings_content| {
                                        match settings_content.theme.theme.as_ref() {
                                            Some(settings::ThemeSelection::Dynamic { light, ..}) => Some(light),
                                            _ => None
                                        }
                                    },
                                    write: |settings_content, value, _| {
                                        let Some(value) = value else {
                                            return;
                                        };
                                        match settings_content
                                            .theme
                                            .theme.get_or_insert_default() {
                                                settings::ThemeSelection::Dynamic{ light, ..} => *light = value,
                                                _ => return
                                            }
                                    },
                                }),
                                metadata: None,
                            },
                            SettingItem {
                                files: USER,
                                title: t(keys::settings_ui::SETTINGS_APPEARANCE_DARK_THEME).into(),
                                description: t(keys::settings_ui::SETTINGS_APPEARANCE_THE_THEME_TO_USE_WHEN_MODE_IS_SET_TO_DAR).into(),
                                field: Box::new(SettingField {
                                    organization_override: None,
                                    json_path: Some("theme.dark"),
                                    pick: |settings_content| {
                                        match settings_content.theme.theme.as_ref() {
                                            Some(settings::ThemeSelection::Dynamic { dark, ..}) => Some(dark),
                                            _ => None
                                        }
                                    },
                                    write: |settings_content, value, _| {
                                        let Some(value) = value else {
                                            return;
                                        };
                                        match settings_content
                                            .theme
                                            .theme.get_or_insert_default() {
                                                settings::ThemeSelection::Dynamic{ dark, ..} => *dark = value,
                                                _ => return
                                            }
                                    },
                                }),
                                metadata: None,
                            }
                        ],
                    }
                }).collect(),
            }),
            SettingsPageItem::DynamicItem(DynamicItem {
                discriminant: SettingItem {
                    files: USER,
                    title: t(keys::settings_ui::SETTINGS_APPEARANCE_ICON_THEME).into(),
                    description: t(keys::settings_ui::SETTINGS_APPEARANCE_THE_CUSTOM_SET_OF_ICONS_ZED_WILL_ASSOCIA).into(),
                    field: Box::new(SettingField {
                        organization_override: None,
                        json_path: Some("icon_theme$"),
                        pick: |settings_content| {
                            Some(&dynamic_variants::<settings::IconThemeSelection>()[
                                settings_content
                                    .theme
                                    .icon_theme
                                    .as_ref()?
                                    .discriminant() as usize])
                        },
                        write: |settings_content, value, app| {
                            let Some(value) = value else {
                                settings_content.theme.icon_theme = None;
                                return;
                            };
                            let settings_value = settings_content.theme.icon_theme.get_or_insert_with(|| {
                                settings::IconThemeSelection::Static(settings::IconThemeName(theme::default_icon_theme().name.clone().into()))
                            });
                            *settings_value = match value {
                                settings::IconThemeSelectionDiscriminants::Static => {
                                    let name = match settings_value {
                                        settings::IconThemeSelection::Static(_) => return,
                                        settings::IconThemeSelection::Dynamic { mode, light, dark } => {
                                            match mode {
                                                theme_settings::ThemeAppearanceMode::Light => light.clone(),
                                                theme_settings::ThemeAppearanceMode::Dark => dark.clone(),
                                                theme_settings::ThemeAppearanceMode::System => {
                                                    if SystemAppearance::global(app).is_light() {
                                                        light.clone()
                                                    } else {
                                                        dark.clone()
                                                    }
                                                }
                                            }
                                        },
                                    };
                                    settings::IconThemeSelection::Static(name)
                                },
                                settings::IconThemeSelectionDiscriminants::Dynamic => {
                                    let static_name = match settings_value {
                                        settings::IconThemeSelection::Static(theme_name) => theme_name.clone(),
                                        settings::IconThemeSelection::Dynamic {..} => return,
                                    };

                                    settings::IconThemeSelection::Dynamic {
                                        mode: settings::ThemeAppearanceMode::System,
                                        light: static_name.clone(),
                                        dark: static_name,
                                    }
                                },
                            };
                        },
                    }),
                    metadata: None,
                },
                pick_discriminant: |settings_content| {
                    Some(settings_content.theme.icon_theme.as_ref()?.discriminant() as usize)
                },
                fields: dynamic_variants::<settings::IconThemeSelection>().into_iter().map(|variant| {
                    match variant {
                        settings::IconThemeSelectionDiscriminants::Static => vec![
                            SettingItem {
                                files: USER,
                                title: t(keys::settings_ui::SETTINGS_APPEARANCE_ICON_THEME_NAME).into(),
                                description: t(keys::settings_ui::SETTINGS_APPEARANCE_THE_NAME_OF_YOUR_SELECTED_ICON_THEME).into(),
                                field: Box::new(SettingField {
                                    organization_override: None,
                                    json_path: Some("icon_theme$string"),
                                    pick: |settings_content| {
                                        match settings_content.theme.icon_theme.as_ref() {
                                            Some(settings::IconThemeSelection::Static(name)) => Some(name),
                                            _ => None
                                        }
                                    },
                                    write: |settings_content, value, _| {
                                        let Some(value) = value else {
                                            return;
                                        };
                                        match settings_content
                                            .theme
                                            .icon_theme.as_mut() {
                                                Some(settings::IconThemeSelection::Static(theme_name)) => *theme_name = value,
                                                _ => return
                                            }
                                    },
                                }),
                                metadata: None,
                            }
                        ],
                        settings::IconThemeSelectionDiscriminants::Dynamic => vec![
                            SettingItem {
                                files: USER,
                                title: t(keys::settings_ui::SETTINGS_APPEARANCE_MODE).into(),
                                description: t(keys::settings_ui::SETTINGS_APPEARANCE_CHOOSE_WHETHER_TO_USE_THE_SELECTED_LIGHT_2).into(),
                                field: Box::new(SettingField {
                                    organization_override: None,
                                    json_path: Some("icon_theme"),
                                    pick: |settings_content| {
                                        match settings_content.theme.icon_theme.as_ref() {
                                            Some(settings::IconThemeSelection::Dynamic { mode, ..}) => Some(mode),
                                            _ => None
                                        }
                                    },
                                    write: |settings_content, value, _| {
                                        let Some(value) = value else {
                                            return;
                                        };
                                        match settings_content
                                            .theme
                                            .icon_theme.as_mut() {
                                                Some(settings::IconThemeSelection::Dynamic{ mode, ..}) => *mode = value,
                                                _ => return
                                            }
                                    },
                                }),
                                metadata: None,
                            },
                            SettingItem {
                                files: USER,
                                title: t(keys::settings_ui::SETTINGS_APPEARANCE_LIGHT_ICON_THEME).into(),
                                description: t(keys::settings_ui::SETTINGS_APPEARANCE_THE_ICON_THEME_TO_USE_WHEN_MODE_IS_SET_T).into(),
                                field: Box::new(SettingField {
                                    organization_override: None,
                                    json_path: Some("icon_theme.light"),
                                    pick: |settings_content| {
                                        match settings_content.theme.icon_theme.as_ref() {
                                            Some(settings::IconThemeSelection::Dynamic { light, ..}) => Some(light),
                                            _ => None
                                        }
                                    },
                                    write: |settings_content, value, _| {
                                        let Some(value) = value else {
                                            return;
                                        };
                                        match settings_content
                                            .theme
                                            .icon_theme.as_mut() {
                                                Some(settings::IconThemeSelection::Dynamic{ light, ..}) => *light = value,
                                                _ => return
                                            }
                                    },
                                }),
                                metadata: None,
                            },
                            SettingItem {
                                files: USER,
                                title: t(keys::settings_ui::SETTINGS_APPEARANCE_DARK_ICON_THEME).into(),
                                description: t(keys::settings_ui::SETTINGS_APPEARANCE_THE_ICON_THEME_TO_USE_WHEN_MODE_IS_SET_T_2).into(),
                                field: Box::new(SettingField {
                                    organization_override: None,
                                    json_path: Some("icon_theme.dark"),
                                    pick: |settings_content| {
                                        match settings_content.theme.icon_theme.as_ref() {
                                            Some(settings::IconThemeSelection::Dynamic { dark, ..}) => Some(dark),
                                            _ => None
                                        }
                                    },
                                    write: |settings_content, value, _| {
                                        let Some(value) = value else {
                                            return;
                                        };
                                        match settings_content
                                            .theme
                                            .icon_theme.as_mut() {
                                                Some(settings::IconThemeSelection::Dynamic{ dark, ..}) => *dark = value,
                                                _ => return
                                            }
                                    },
                                }),
                                metadata: None,
                            }
                        ],
                    }
                }).collect(),
            }),
        ]
    }

    fn buffer_font_section() -> [SettingsPageItem; 7] {
        [
            SettingsPageItem::SectionHeader(t(keys::settings_ui::SETTINGS_APPEARANCE_SECTION_BUFFER_FONT).into()),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_APPEARANCE_FONT_FAMILY).into(),
                description: t(keys::settings_ui::SETTINGS_APPEARANCE_FONT_FAMILY_FOR_EDITOR_TEXT).into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("buffer_font_family"),
                    pick: |settings_content| settings_content.theme.buffer_font_family.as_ref(),
                    write: |settings_content, value, _| {
                        settings_content.theme.buffer_font_family = value;
                    },
                }),
                metadata: None,
                files: USER,
            }),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_APPEARANCE_FONT_SIZE).into(),
                description: t(keys::settings_ui::SETTINGS_APPEARANCE_FONT_SIZE_FOR_EDITOR_TEXT).into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("buffer_font_size"),
                    pick: |settings_content| settings_content.theme.buffer_font_size.as_ref(),
                    write: |settings_content, value, _| {
                        settings_content.theme.buffer_font_size = value;
                    },
                }),
                metadata: None,
                files: USER,
            }),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_APPEARANCE_FONT_WEIGHT).into(),
                description: t(keys::settings_ui::SETTINGS_APPEARANCE_FONT_WEIGHT_FOR_EDITOR_TEXT_100_900).into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("buffer_font_weight"),
                    pick: |settings_content| settings_content.theme.buffer_font_weight.as_ref(),
                    write: |settings_content, value, _| {
                        settings_content.theme.buffer_font_weight = value;
                    },
                }),
                metadata: None,
                files: USER,
            }),
            SettingsPageItem::DynamicItem(DynamicItem {
                discriminant: SettingItem {
                    files: USER,
                    title: t(keys::settings_ui::SETTINGS_APPEARANCE_LINE_HEIGHT).into(),
                    description: t(keys::settings_ui::SETTINGS_APPEARANCE_LINE_HEIGHT_FOR_EDITOR_TEXT).into(),
                    field: Box::new(SettingField {
                        organization_override: None,
                        json_path: Some("buffer_line_height$"),
                        pick: |settings_content| {
                            Some(
                                &dynamic_variants::<settings::BufferLineHeight>()[settings_content
                                    .theme
                                    .buffer_line_height
                                    .as_ref()?
                                    .discriminant()
                                    as usize],
                            )
                        },
                        write: |settings_content, value, _| {
                            let Some(value) = value else {
                                settings_content.theme.buffer_line_height = None;
                                return;
                            };
                            let settings_value = settings_content
                                .theme
                                .buffer_line_height
                                .get_or_insert_with(|| settings::BufferLineHeight::default());
                            *settings_value = match value {
                                settings::BufferLineHeightDiscriminants::Comfortable => {
                                    settings::BufferLineHeight::Comfortable
                                }
                                settings::BufferLineHeightDiscriminants::Standard => {
                                    settings::BufferLineHeight::Standard
                                }
                                settings::BufferLineHeightDiscriminants::Custom => {
                                    let custom_value =
                                        theme_settings::buffer_line_height_from_settings(
                                            *settings_value,
                                        )
                                        .value();
                                    settings::BufferLineHeight::Custom(custom_value)
                                }
                            };
                        },
                    }),
                    metadata: None,
                },
                pick_discriminant: |settings_content| {
                    Some(
                        settings_content
                            .theme
                            .buffer_line_height
                            .as_ref()?
                            .discriminant() as usize,
                    )
                },
                fields: dynamic_variants::<settings::BufferLineHeight>()
                    .into_iter()
                    .map(|variant| match variant {
                        settings::BufferLineHeightDiscriminants::Comfortable => vec![],
                        settings::BufferLineHeightDiscriminants::Standard => vec![],
                        settings::BufferLineHeightDiscriminants::Custom => vec![SettingItem {
                            files: USER,
                            title: t(keys::settings_ui::SETTINGS_APPEARANCE_CUSTOM_LINE_HEIGHT).into(),
                            description: t(keys::settings_ui::SETTINGS_APPEARANCE_CUSTOM_LINE_HEIGHT_VALUE_MUST_BE_AT_LEAS).into(),
                            field: Box::new(SettingField {
                                organization_override: None,
                                json_path: Some("buffer_line_height"),
                                pick: |settings_content| match settings_content
                                    .theme
                                    .buffer_line_height
                                    .as_ref()
                                {
                                    Some(settings::BufferLineHeight::Custom(value)) => Some(value),
                                    _ => None,
                                },
                                write: |settings_content, value, _| {
                                    let Some(value) = value else {
                                        return;
                                    };
                                    match settings_content.theme.buffer_line_height.as_mut() {
                                        Some(settings::BufferLineHeight::Custom(line_height)) => {
                                            *line_height = f32::max(value, 1.0)
                                        }
                                        _ => return,
                                    }
                                },
                            }),
                            metadata: None,
                        }],
                    })
                    .collect(),
            }),
            SettingsPageItem::SettingItem(SettingItem {
                files: USER,
                title: t(keys::settings_ui::SETTINGS_APPEARANCE_FONT_FEATURES).into(),
                description: t(keys::settings_ui::SETTINGS_APPEARANCE_THE_OPENTYPE_FEATURES_TO_ENABLE_FOR_REND).into(),
                field: Box::new(
                    SettingField {
                        organization_override: None,
                        json_path: Some("buffer_font_features"),
                        pick: |settings_content| {
                            settings_content.theme.buffer_font_features.as_ref()
                        },
                        write: |settings_content, value, _| {
                            settings_content.theme.buffer_font_features = value;
                        },
                    }
                    .unimplemented(),
                ),
                metadata: None,
            }),
            SettingsPageItem::SettingItem(SettingItem {
                files: USER,
                title: t(keys::settings_ui::SETTINGS_APPEARANCE_FONT_FALLBACKS).into(),
                description: t(keys::settings_ui::SETTINGS_APPEARANCE_THE_FONT_FALLBACKS_TO_USE_FOR_RENDERING).into(),
                field: Box::new(
                    SettingField {
                        organization_override: None,
                        json_path: Some("buffer_font_fallbacks"),
                        pick: |settings_content| {
                            settings_content.theme.buffer_font_fallbacks.as_ref()
                        },
                        write: |settings_content, value, _| {
                            settings_content.theme.buffer_font_fallbacks = value;
                        },
                    }
                    .unimplemented(),
                ),
                metadata: None,
            }),
        ]
    }

    fn ui_font_section() -> [SettingsPageItem; 6] {
        [
            SettingsPageItem::SectionHeader(t(keys::settings_ui::SETTINGS_APPEARANCE_SECTION_UI_FONT).into()),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_APPEARANCE_FONT_FAMILY).into(),
                description: t(keys::settings_ui::SETTINGS_APPEARANCE_FONT_FAMILY_FOR_UI_ELEMENTS).into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("ui_font_family"),
                    pick: |settings_content| settings_content.theme.ui_font_family.as_ref(),
                    write: |settings_content, value, _| {
                        settings_content.theme.ui_font_family = value;
                    },
                }),
                metadata: None,
                files: USER,
            }),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_APPEARANCE_FONT_SIZE).into(),
                description: t(keys::settings_ui::SETTINGS_APPEARANCE_FONT_SIZE_FOR_UI_ELEMENTS).into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("ui_font_size"),
                    pick: |settings_content| settings_content.theme.ui_font_size.as_ref(),
                    write: |settings_content, value, _| {
                        settings_content.theme.ui_font_size = value;
                    },
                }),
                metadata: None,
                files: USER,
            }),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_APPEARANCE_FONT_WEIGHT).into(),
                description: t(keys::settings_ui::SETTINGS_APPEARANCE_FONT_WEIGHT_FOR_UI_ELEMENTS_100_900).into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("ui_font_weight"),
                    pick: |settings_content| settings_content.theme.ui_font_weight.as_ref(),
                    write: |settings_content, value, _| {
                        settings_content.theme.ui_font_weight = value;
                    },
                }),
                metadata: None,
                files: USER,
            }),
            SettingsPageItem::SettingItem(SettingItem {
                files: USER,
                title: t(keys::settings_ui::SETTINGS_APPEARANCE_FONT_FEATURES).into(),
                description: t(keys::settings_ui::SETTINGS_APPEARANCE_THE_OPENTYPE_FEATURES_TO_ENABLE_FOR_REND_2).into(),
                field: Box::new(
                    SettingField {
                        organization_override: None,
                        json_path: Some("ui_font_features"),
                        pick: |settings_content| settings_content.theme.ui_font_features.as_ref(),
                        write: |settings_content, value, _| {
                            settings_content.theme.ui_font_features = value;
                        },
                    }
                    .unimplemented(),
                ),
                metadata: None,
            }),
            SettingsPageItem::SettingItem(SettingItem {
                files: USER,
                title: t(keys::settings_ui::SETTINGS_APPEARANCE_FONT_FALLBACKS).into(),
                description: t(keys::settings_ui::SETTINGS_APPEARANCE_THE_FONT_FALLBACKS_TO_USE_FOR_RENDERING_2).into(),
                field: Box::new(
                    SettingField {
                        organization_override: None,
                        json_path: Some("ui_font_fallbacks"),
                        pick: |settings_content| settings_content.theme.ui_font_fallbacks.as_ref(),
                        write: |settings_content, value, _| {
                            settings_content.theme.ui_font_fallbacks = value;
                        },
                    }
                    .unimplemented(),
                ),
                metadata: None,
            }),
        ]
    }

    fn agent_panel_font_section() -> [SettingsPageItem; 5] {
        [
            SettingsPageItem::SectionHeader(
                t(keys::settings_ui::SETTINGS_APPEARANCE_SECTION_AGENT_PANEL_FONT).into(),
            ),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_APPEARANCE_UI_FONT_FAMILY).into(),
                description: t(
                    keys::settings_ui::SETTINGS_APPEARANCE_FONT_FAMILY_FOR_AGENT_RESPONSE_TEXT_IN_T,
                )
                .into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("agent_ui_font_family"),
                    pick: |settings_content| {
                        settings_content
                            .theme
                            .agent_ui_font_family
                            .as_ref()
                            .or(settings_content.theme.ui_font_family.as_ref())
                    },
                    write: |settings_content, value, _| {
                        settings_content.theme.agent_ui_font_family = value;
                    },
                }),
                metadata: None,
                files: USER,
            }),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_APPEARANCE_UI_FONT_SIZE).into(),
                description: t(
                    keys::settings_ui::SETTINGS_APPEARANCE_FONT_SIZE_FOR_AGENT_RESPONSE_TEXT_IN_THE,
                )
                .into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("agent_ui_font_size"),
                    pick: |settings_content| {
                        settings_content
                            .theme
                            .agent_ui_font_size
                            .as_ref()
                            .or(settings_content.theme.ui_font_size.as_ref())
                    },
                    write: |settings_content, value, _| {
                        settings_content.theme.agent_ui_font_size = value;
                    },
                }),
                metadata: None,
                files: USER,
            }),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_APPEARANCE_BUFFER_FONT_FAMILY).into(),
                description: t(
                    keys::settings_ui::SETTINGS_APPEARANCE_FONT_FAMILY_FOR_USER_MESSAGES_IN_THE_AGE,
                )
                .into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("agent_buffer_font_family"),
                    pick: |settings_content| {
                        settings_content
                            .theme
                            .agent_buffer_font_family
                            .as_ref()
                            .or(settings_content.theme.buffer_font_family.as_ref())
                    },
                    write: |settings_content, value, _| {
                        settings_content.theme.agent_buffer_font_family = value;
                    },
                }),
                metadata: None,
                files: USER,
            }),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_APPEARANCE_BUFFER_FONT_SIZE).into(),
                description: t(
                    keys::settings_ui::SETTINGS_APPEARANCE_FONT_SIZE_FOR_USER_MESSAGES_TEXT_IN_THE,
                )
                .into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("agent_buffer_font_size"),
                    pick: |settings_content| {
                        settings_content
                            .theme
                            .agent_buffer_font_size
                            .as_ref()
                            .or(settings_content.theme.buffer_font_size.as_ref())
                    },
                    write: |settings_content, value, _| {
                        settings_content.theme.agent_buffer_font_size = value;
                    },
                }),
                metadata: None,
                files: USER,
            }),
        ]
    }

    fn markdown_preview_font_section() -> [SettingsPageItem; 5] {
        [
            SettingsPageItem::SectionHeader(
                t(keys::settings_ui::SETTINGS_APPEARANCE_SECTION_MARKDOWN_PREVIEW_FONT).into(),
            ),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_APPEARANCE_FONT_FAMILY).into(),
                description: t(
                    keys::settings_ui::SETTINGS_APPEARANCE_FONT_FAMILY_FOR_THE_MARKDOWN_PREVIEW_FAL,
                )
                .into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("markdown_preview.font_family"),
                    pick: |settings_content| {
                        settings_content
                            .markdown_preview
                            .as_ref()?
                            .font_family
                            .as_ref()
                            .or(settings_content.theme.ui_font_family.as_ref())
                    },
                    write: |settings_content, value, _| {
                        settings_content
                            .markdown_preview
                            .get_or_insert_default()
                            .font_family = value;
                    },
                }),
                metadata: None,
                files: USER,
            }),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_APPEARANCE_CODE_FONT_FAMILY).into(),
                description: t(
                    keys::settings_ui::SETTINGS_APPEARANCE_FONT_FAMILY_FOR_CODE_BLOCKS_IN_THE_MARKD,
                )
                .into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("markdown_preview.code_font_family"),
                    pick: |settings_content| {
                        settings_content
                            .markdown_preview
                            .as_ref()?
                            .code_font_family
                            .as_ref()
                            .or(settings_content.theme.buffer_font_family.as_ref())
                    },
                    write: |settings_content, value, _| {
                        settings_content
                            .markdown_preview
                            .get_or_insert_default()
                            .code_font_family = value;
                    },
                }),
                metadata: None,
                files: USER,
            }),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_APPEARANCE_FONT_SIZE).into(),
                description: t(
                    keys::settings_ui::SETTINGS_APPEARANCE_FONT_SIZE_FOR_THE_MARKDOWN_PREVIEW_FALLS,
                )
                .into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("markdown_preview.font_size"),
                    pick: |settings_content| {
                        settings_content
                            .markdown_preview
                            .as_ref()
                            .and_then(|preview| preview.font_size.as_ref())
                            .or(settings_content.theme.buffer_font_size.as_ref())
                    },
                    write: |settings_content, value, _| {
                        settings_content
                            .markdown_preview
                            .get_or_insert_default()
                            .font_size = value;
                    },
                }),
                metadata: None,
                files: USER,
            }),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_APPEARANCE_HEADING_FONT_WEIGHT).into(),
                description: t(
                    keys::settings_ui::SETTINGS_APPEARANCE_FONT_WEIGHT_FOR_HEADINGS_IN_THE_MARKDOWN,
                )
                .into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("markdown_preview.heading_font_weight"),
                    pick: |settings_content| {
                        settings_content
                            .markdown_preview
                            .as_ref()?
                            .heading_font_weight
                            .as_ref()
                    },
                    write: |settings_content, value, _| {
                        settings_content
                            .markdown_preview
                            .get_or_insert_default()
                            .heading_font_weight = value;
                    },
                }),
                metadata: None,
                files: USER,
            }),
        ]
    }

    fn mermaid_font_section() -> [SettingsPageItem; 2] {
        [
            SettingsPageItem::SectionHeader(
                t(keys::settings_ui::SETTINGS_APPEARANCE_SECTION_MERMAID_FONT).into(),
            ),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_APPEARANCE_FONT_FAMILY).into(),
                description: t(
                    keys::settings_ui::SETTINGS_APPEARANCE_FONT_FAMILY_FOR_MERMAID_DIAGRAMS_IN_THE,
                )
                .into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("mermaid_font_family"),
                    pick: |settings_content| {
                        settings_content
                            .theme
                            .mermaid_font_family
                            .as_ref()
                            .or(settings_content.theme.ui_font_family.as_ref())
                    },
                    write: |settings_content, value, _| {
                        settings_content.theme.mermaid_font_family = value;
                    },
                }),
                metadata: None,
                files: USER,
            }),
        ]
    }

    fn text_rendering_section() -> [SettingsPageItem; 3] {
        [
            SettingsPageItem::SectionHeader(
                t(keys::settings_ui::SETTINGS_APPEARANCE_SECTION_TEXT_RENDERING).into(),
            ),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_APPEARANCE_TEXT_RENDERING_MODE).into(),
                description: t(
                    keys::settings_ui::SETTINGS_APPEARANCE_THE_TEXT_RENDERING_MODE_TO_USE,
                )
                .into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("text_rendering_mode"),
                    pick: |settings_content| {
                        settings_content.workspace.text_rendering_mode.as_ref()
                    },
                    write: |settings_content, value, _| {
                        settings_content.workspace.text_rendering_mode = value;
                    },
                }),
                metadata: None,
                files: USER,
            }),
            SettingsPageItem::SettingItem(SettingItem {
                title: "Display P3".into(),
                description: t(
                    keys::settings_ui::SETTINGS_APPEARANCE_MACOS_ONLY_RENDER_WINDOWS_USING_THE_DISP,
                )
                .into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("use_display_p3"),
                    pick: |settings_content| settings_content.workspace.use_display_p3.as_ref(),
                    write: |settings_content, value, _| {
                        settings_content.workspace.use_display_p3 = value;
                    },
                }),
                metadata: None,
                files: USER,
            }),
        ]
    }

    fn cursor_section() -> [SettingsPageItem; 7] {
        [
            SettingsPageItem::SectionHeader(
                t(keys::settings_ui::SETTINGS_APPEARANCE_SECTION_CURSOR).into(),
            ),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_APPEARANCE_MULTI_CURSOR_MODIFIER).into(),
                description: t(
                    keys::settings_ui::SETTINGS_APPEARANCE_MODIFIER_KEY_FOR_ADDING_MULTIPLE_CURSORS,
                )
                .into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("multi_cursor_modifier"),
                    pick: |settings_content| settings_content.editor.multi_cursor_modifier.as_ref(),
                    write: |settings_content, value, _| {
                        settings_content.editor.multi_cursor_modifier = value;
                    },
                }),
                metadata: None,
                files: USER,
            }),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_APPEARANCE_CURSOR_BLINK).into(),
                description: t(
                    keys::settings_ui::SETTINGS_APPEARANCE_WHETHER_THE_CURSOR_BLINKS_IN_THE_EDITOR,
                )
                .into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("cursor_blink"),
                    pick: |settings_content| settings_content.editor.cursor_blink.as_ref(),
                    write: |settings_content, value, _| {
                        settings_content.editor.cursor_blink = value;
                    },
                }),
                metadata: None,
                files: USER,
            }),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_APPEARANCE_CURSOR_ANIMATION).into(),
                description: t(
                    keys::settings_ui::SETTINGS_APPEARANCE_WHETHER_THE_CURSOR_SMOOTHLY_ANIMATES_WHE,
                )
                .into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("cursor_animation.enabled"),
                    pick: |settings_content| {
                        settings_content
                            .editor
                            .cursor_animation
                            .as_ref()?
                            .enabled
                            .as_ref()
                    },
                    write: |settings_content, value, _| {
                        settings_content
                            .editor
                            .cursor_animation
                            .get_or_insert_default()
                            .enabled = value;
                    },
                }),
                metadata: None,
                files: USER,
            }),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_APPEARANCE_CURSOR_SHAPE).into(),
                description: t(keys::settings_ui::SETTINGS_APPEARANCE_CURSOR_SHAPE_FOR_THE_EDITOR)
                    .into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("cursor_shape"),
                    pick: |settings_content| settings_content.editor.cursor_shape.as_ref(),
                    write: |settings_content, value, _| {
                        settings_content.editor.cursor_shape = value;
                    },
                }),
                metadata: None,
                files: USER,
            }),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_APPEARANCE_HIDE_MOUSE).into(),
                description: t(
                    keys::settings_ui::SETTINGS_APPEARANCE_WHEN_TO_HIDE_THE_MOUSE_CURSOR,
                )
                .into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("hide_mouse"),
                    pick: |settings_content| settings_content.hide_mouse.as_ref(),
                    write: |settings_content, value, _| {
                        settings_content.hide_mouse = value;
                    },
                }),
                metadata: None,
                files: USER,
            }),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_APPEARANCE_REDUCE_MOTION).into(),
                description: t(
                    keys::settings_ui::SETTINGS_APPEARANCE_WHETHER_TO_REDUCE_NON_ESSENTIAL_MOTION_S,
                )
                .into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("reduce_motion"),
                    pick: |settings_content| settings_content.reduce_motion.as_ref(),
                    write: |settings_content, value, _| {
                        settings_content.reduce_motion = value;
                    },
                }),
                metadata: None,
                files: USER,
            }),
        ]
    }

    fn highlighting_section() -> [SettingsPageItem; 6] {
        [
            SettingsPageItem::SectionHeader(
                t(keys::settings_ui::SETTINGS_APPEARANCE_SECTION_HIGHLIGHTING).into(),
            ),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_APPEARANCE_UNNECESSARY_CODE_FADE).into(),
                description: t(
                    keys::settings_ui::SETTINGS_APPEARANCE_HOW_MUCH_TO_FADE_OUT_UNUSED_CODE_0_0_0_9,
                )
                .into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("unnecessary_code_fade"),
                    pick: |settings_content| settings_content.theme.unnecessary_code_fade.as_ref(),
                    write: |settings_content, value, _| {
                        settings_content.theme.unnecessary_code_fade = value;
                    },
                }),
                metadata: None,
                files: USER,
            }),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_APPEARANCE_CURRENT_LINE_HIGHLIGHT).into(),
                description: t(
                    keys::settings_ui::SETTINGS_APPEARANCE_HOW_TO_HIGHLIGHT_THE_CURRENT_LINE,
                )
                .into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("current_line_highlight"),
                    pick: |settings_content| {
                        settings_content.editor.current_line_highlight.as_ref()
                    },
                    write: |settings_content, value, _| {
                        settings_content.editor.current_line_highlight = value;
                    },
                }),
                metadata: None,
                files: USER,
            }),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_APPEARANCE_SELECTION_HIGHLIGHT).into(),
                description: t(
                    keys::settings_ui::SETTINGS_APPEARANCE_HIGHLIGHT_ALL_OCCURRENCES_OF_SELECTED_TE,
                )
                .into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("selection_highlight"),
                    pick: |settings_content| settings_content.editor.selection_highlight.as_ref(),
                    write: |settings_content, value, _| {
                        settings_content.editor.selection_highlight = value;
                    },
                }),
                metadata: None,
                files: USER,
            }),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_APPEARANCE_ROUNDED_SELECTION).into(),
                description: t(
                    keys::settings_ui::SETTINGS_APPEARANCE_WHETHER_THE_TEXT_SELECTION_SHOULD_HAVE_R,
                )
                .into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("rounded_selection"),
                    pick: |settings_content| settings_content.editor.rounded_selection.as_ref(),
                    write: |settings_content, value, _| {
                        settings_content.editor.rounded_selection = value;
                    },
                }),
                metadata: None,
                files: USER,
            }),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_APPEARANCE_MINIMUM_CONTRAST_FOR_HIGHLIGHTS)
                    .into(),
                description: t(
                    keys::settings_ui::SETTINGS_APPEARANCE_THE_MINIMUM_APCA_PERCEPTUAL_CONTRAST_TO,
                )
                .into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("minimum_contrast_for_highlights"),
                    pick: |settings_content| {
                        settings_content
                            .editor
                            .minimum_contrast_for_highlights
                            .as_ref()
                    },
                    write: |settings_content, value, _| {
                        settings_content.editor.minimum_contrast_for_highlights = value;
                    },
                }),
                metadata: None,
                files: USER,
            }),
        ]
    }

    fn guides_section() -> [SettingsPageItem; 3] {
        [
            SettingsPageItem::SectionHeader(
                t(keys::settings_ui::SETTINGS_APPEARANCE_SECTION_GUIDES).into(),
            ),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_APPEARANCE_SHOW_WRAP_GUIDES).into(),
                description: t(
                    keys::settings_ui::SETTINGS_APPEARANCE_SHOW_WRAP_GUIDES_VERTICAL_RULERS,
                )
                .into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("show_wrap_guides"),
                    pick: |settings_content| {
                        settings_content
                            .project
                            .all_languages
                            .defaults
                            .show_wrap_guides
                            .as_ref()
                    },
                    write: |settings_content, value, _| {
                        settings_content
                            .project
                            .all_languages
                            .defaults
                            .show_wrap_guides = value;
                    },
                }),
                metadata: None,
                files: USER | PROJECT,
            }),
            // todo(settings_ui): This needs a custom component
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_APPEARANCE_WRAP_GUIDES).into(),
                description: t(
                    keys::settings_ui::SETTINGS_APPEARANCE_CHARACTER_COUNTS_AT_WHICH_TO_SHOW_WRAP_G,
                )
                .into(),
                field: Box::new(
                    SettingField {
                        organization_override: None,
                        json_path: Some("wrap_guides"),
                        pick: |settings_content| {
                            settings_content
                                .project
                                .all_languages
                                .defaults
                                .wrap_guides
                                .as_ref()
                        },
                        write: |settings_content, value, _| {
                            settings_content.project.all_languages.defaults.wrap_guides = value;
                        },
                    }
                    .unimplemented(),
                ),
                metadata: None,
                files: USER | PROJECT,
            }),
        ]
    }

    let items: Box<[SettingsPageItem]> = concat_sections!(
        theme_section(),
        buffer_font_section(),
        ui_font_section(),
        agent_panel_font_section(),
        markdown_preview_font_section(),
        mermaid_font_section(),
        text_rendering_section(),
        cursor_section(),
        highlighting_section(),
        guides_section(),
    );

    SettingsPage {
        title: t(keys::settings_ui::SETTINGS_APPEARANCE_APPEARANCE).into(),
        items,
    }
}

fn keymap_page() -> SettingsPage {
    fn keybindings_section() -> [SettingsPageItem; 2] {
        [
            SettingsPageItem::SectionHeader(
                t(keys::settings_ui::SETTINGS_KEYMAP_SECTION_KEYBINDINGS).into(),
            ),
            SettingsPageItem::ActionLink(ActionLink {
                title: t(keys::settings_ui::SETTINGS_KEYMAP_EDIT_KEYBINDINGS).into(),
                description: Some(t(keys::settings_ui::SETTINGS_KEYMAP_EDITOR_HINT).into()),
                button_text: t(keys::app_menu::MENU_ZED_OPEN_KEYMAP).into(),
                on_click: Arc::new(|settings_window, window, cx| {
                    let Some(original_window) = settings_window.original_window else {
                        return;
                    };
                    original_window
                        .update(cx, |_workspace, original_window, cx| {
                            original_window
                                .dispatch_action(zed_actions::OpenKeymap.boxed_clone(), cx);
                            original_window.activate_window();
                        })
                        .ok();
                    window.remove_window();
                }),
                files: USER,
            }),
        ]
    }

    fn base_keymap_section() -> [SettingsPageItem; 2] {
        [
            SettingsPageItem::SectionHeader(
                t(keys::settings_ui::SETTINGS_KEYMAP_SECTION_BASE_KEYMAP).into(),
            ),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_KEYMAP_SECTION_BASE_KEYMAP).into(),
                description: t(
                    keys::settings_ui::SETTINGS_KEYMAP_THE_NAME_OF_A_BASE_SET_OF_KEY_BINDINGS_T,
                )
                .into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("base_keymap"),
                    pick: |settings_content| settings_content.base_keymap.as_ref(),
                    write: |settings_content, value, _| {
                        settings_content.base_keymap = value;
                    },
                }),
                metadata: Some(Box::new(SettingsFieldMetadata {
                    should_do_titlecase: Some(false),
                    ..Default::default()
                })),
                files: USER,
            }),
        ]
    }

    fn modal_editing_section() -> [SettingsPageItem; 3] {
        [
            SettingsPageItem::SectionHeader(
                t(keys::settings_ui::SETTINGS_KEYMAP_SECTION_MODAL_EDITING).into(),
            ),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_KEYMAP_VIM_MODE).into(),
                description: t(keys::settings_ui::SETTINGS_KEYMAP_ENABLE_VIM_MODE_AND_KEY_BINDINGS)
                    .into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("vim_mode"),
                    pick: |settings_content| settings_content.vim_mode.as_ref(),
                    write: write_vim_mode,
                }),
                metadata: None,
                files: USER,
            }),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_KEYMAP_HELIX_MODE).into(),
                description: t(
                    keys::settings_ui::SETTINGS_KEYMAP_ENABLE_HELIX_MODE_AND_KEY_BINDINGS,
                )
                .into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("helix_mode"),
                    pick: |settings_content| settings_content.helix_mode.as_ref(),
                    write: write_helix_mode,
                }),
                metadata: None,
                files: USER,
            }),
        ]
    }

    let items: Box<[SettingsPageItem]> = concat_sections!(
        keybindings_section(),
        base_keymap_section(),
        modal_editing_section(),
    );

    SettingsPage {
        title: t(keys::settings_ui::SETTINGS_KEYMAP_KEYMAP).into(),
        items,
    }
}

fn editor_page() -> SettingsPage {
    fn auto_save_section() -> [SettingsPageItem; 2] {
        [
            SettingsPageItem::SectionHeader(t(keys::settings_ui::SETTINGS_EDITOR_SECTION_AUTO_SAVE).into()),
            SettingsPageItem::DynamicItem(DynamicItem {
                discriminant: SettingItem {
                    files: USER,
                    title: t(keys::settings_ui::SETTINGS_EDITOR_AUTO_SAVE_MODE).into(),
                    description: t(keys::settings_ui::SETTINGS_EDITOR_WHEN_TO_AUTO_SAVE_BUFFER_CHANGES).into(),
                    field: Box::new(SettingField {
                        organization_override: None,
                        json_path: Some("autosave$"),
                        pick: |settings_content| {
                            Some(
                                &dynamic_variants::<settings::AutosaveSetting>()[settings_content
                                    .workspace
                                    .autosave
                                    .as_ref()?
                                    .discriminant()
                                    as usize],
                            )
                        },
                        write: |settings_content, value, _| {
                            let Some(value) = value else {
                                settings_content.workspace.autosave = None;
                                return;
                            };
                            let settings_value = settings_content
                                .workspace
                                .autosave
                                .get_or_insert_with(|| settings::AutosaveSetting::Off);
                            *settings_value = match value {
                                settings::AutosaveSettingDiscriminants::Off => {
                                    settings::AutosaveSetting::Off
                                }
                                settings::AutosaveSettingDiscriminants::AfterDelay => {
                                    let milliseconds = match settings_value {
                                        settings::AutosaveSetting::AfterDelay { milliseconds } => {
                                            *milliseconds
                                        }
                                        _ => settings::DelayMs(1000),
                                    };
                                    settings::AutosaveSetting::AfterDelay { milliseconds }
                                }
                                settings::AutosaveSettingDiscriminants::OnFocusChange => {
                                    settings::AutosaveSetting::OnFocusChange
                                }
                                settings::AutosaveSettingDiscriminants::OnWindowChange => {
                                    settings::AutosaveSetting::OnWindowChange
                                }
                            };
                        },
                    }),
                    metadata: None,
                },
                pick_discriminant: |settings_content| {
                    Some(settings_content.workspace.autosave.as_ref()?.discriminant() as usize)
                },
                fields: dynamic_variants::<settings::AutosaveSetting>()
                    .into_iter()
                    .map(|variant| match variant {
                        settings::AutosaveSettingDiscriminants::Off => vec![],
                        settings::AutosaveSettingDiscriminants::AfterDelay => vec![SettingItem {
                            files: USER,
                            title: t(keys::settings_ui::SETTINGS_EDITOR_DELAY_MILLISECONDS).into(),
                            description: t(keys::settings_ui::SETTINGS_EDITOR_SAVE_AFTER_INACTIVITY_PERIOD_IN_MILLISEC).into(),
                            field: Box::new(SettingField {
                                organization_override: None,
                                json_path: Some("autosave.after_delay.milliseconds"),
                                pick: |settings_content| match settings_content
                                    .workspace
                                    .autosave
                                    .as_ref()
                                {
                                    Some(settings::AutosaveSetting::AfterDelay {
                                        milliseconds,
                                    }) => Some(milliseconds),
                                    _ => None,
                                },
                                write: |settings_content, value, _| {
                                    let Some(value) = value else {
                                        settings_content.workspace.autosave = None;
                                        return;
                                    };
                                    match settings_content.workspace.autosave.as_mut() {
                                        Some(settings::AutosaveSetting::AfterDelay {
                                            milliseconds,
                                        }) => *milliseconds = value,
                                        _ => return,
                                    }
                                },
                            }),
                            metadata: None,
                        }],
                        settings::AutosaveSettingDiscriminants::OnFocusChange => vec![],
                        settings::AutosaveSettingDiscriminants::OnWindowChange => vec![],
                    })
                    .collect(),
            }),
        ]
    }

    fn which_key_section() -> [SettingsPageItem; 3] {
        [
            SettingsPageItem::SectionHeader(
                t(keys::settings_ui::SETTINGS_EDITOR_SECTION_WHICH_KEY_MENU).into(),
            ),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_EDITOR_SHOW_WHICH_KEY_MENU).into(),
                description: t(
                    keys::settings_ui::SETTINGS_EDITOR_DISPLAY_THE_WHICH_KEY_MENU_WITH_MATCHING,
                )
                .into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("which_key.enabled"),
                    pick: |settings_content| {
                        settings_content
                            .which_key
                            .as_ref()
                            .and_then(|settings| settings.enabled.as_ref())
                    },
                    write: |settings_content, value, _| {
                        settings_content.which_key.get_or_insert_default().enabled = value;
                    },
                }),
                metadata: None,
                files: USER,
            }),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_EDITOR_MENU_DELAY).into(),
                description: t(
                    keys::settings_ui::SETTINGS_EDITOR_DELAY_IN_MILLISECONDS_BEFORE_THE_WHICH_K,
                )
                .into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("which_key.delay_ms"),
                    pick: |settings_content| {
                        settings_content
                            .which_key
                            .as_ref()
                            .and_then(|settings| settings.delay_ms.as_ref())
                    },
                    write: |settings_content, value, _| {
                        settings_content.which_key.get_or_insert_default().delay_ms = value;
                    },
                }),
                metadata: None,
                files: USER,
            }),
        ]
    }

    fn multibuffer_section() -> [SettingsPageItem; 7] {
        [
            SettingsPageItem::SectionHeader(
                t(keys::settings_ui::SETTINGS_EDITOR_SECTION_MULTIBUFFER).into(),
            ),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_EDITOR_DOUBLE_CLICK_IN_MULTIBUFFER).into(),
                description: t(
                    keys::settings_ui::SETTINGS_EDITOR_WHAT_TO_DO_WHEN_MULTIBUFFER_IS_DOUBLE_CL,
                )
                .into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("double_click_in_multibuffer"),
                    pick: |settings_content| {
                        settings_content.editor.double_click_in_multibuffer.as_ref()
                    },
                    write: |settings_content, value, _| {
                        settings_content.editor.double_click_in_multibuffer = value;
                    },
                }),
                metadata: None,
                files: USER,
            }),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_EDITOR_EXPAND_EXCERPT_LINES).into(),
                description: t(
                    keys::settings_ui::SETTINGS_EDITOR_HOW_MANY_LINES_TO_EXPAND_THE_MULTIBUFFER,
                )
                .into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("expand_excerpt_lines"),
                    pick: |settings_content| settings_content.editor.expand_excerpt_lines.as_ref(),
                    write: |settings_content, value, _| {
                        settings_content.editor.expand_excerpt_lines = value;
                    },
                }),
                metadata: None,
                files: USER,
            }),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_EDITOR_EXCERPT_CONTEXT_LINES).into(),
                description: t(
                    keys::settings_ui::SETTINGS_EDITOR_HOW_MANY_LINES_OF_CONTEXT_TO_PROVIDE_IN,
                )
                .into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("excerpt_context_lines"),
                    pick: |settings_content| settings_content.editor.excerpt_context_lines.as_ref(),
                    write: |settings_content, value, _| {
                        settings_content.editor.excerpt_context_lines = value;
                    },
                }),
                metadata: None,
                files: USER,
            }),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_EDITOR_EXPAND_OUTLINES_WITH_DEPTH).into(),
                description: t(
                    keys::settings_ui::SETTINGS_EDITOR_DEFAULT_DEPTH_TO_EXPAND_OUTLINE_ITEMS_IN,
                )
                .into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("outline_panel.expand_outlines_with_depth"),
                    pick: |settings_content| {
                        settings_content
                            .outline_panel
                            .as_ref()
                            .and_then(|outline_panel| {
                                outline_panel.expand_outlines_with_depth.as_ref()
                            })
                    },
                    write: |settings_content, value, _| {
                        settings_content
                            .outline_panel
                            .get_or_insert_default()
                            .expand_outlines_with_depth = value;
                    },
                }),
                metadata: None,
                files: USER,
            }),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_EDITOR_DIFF_VIEW_STYLE).into(),
                description: t(
                    keys::settings_ui::SETTINGS_EDITOR_HOW_TO_DISPLAY_DIFFS_IN_THE_EDITOR,
                )
                .into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("diff_view_style"),
                    pick: |settings_content| settings_content.editor.diff_view_style.as_ref(),
                    write: |settings_content, value, _| {
                        settings_content.editor.diff_view_style = value;
                    },
                }),
                metadata: None,
                files: USER,
            }),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_EDITOR_MINIMUM_SPLIT_DIFF_WIDTH).into(),
                description: t(
                    keys::settings_ui::SETTINGS_EDITOR_THE_MINIMUM_WIDTH_IN_COLUMNS_AT_WHICH_TH,
                )
                .into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("minimum_split_diff_width"),
                    pick: |settings_content| {
                        settings_content.editor.minimum_split_diff_width.as_ref()
                    },
                    write: |settings_content, value, _| {
                        settings_content.editor.minimum_split_diff_width = value;
                    },
                }),
                metadata: None,
                files: USER,
            }),
        ]
    }

    fn scrolling_section() -> [SettingsPageItem; 9] {
        [
            SettingsPageItem::SectionHeader(
                t(keys::settings_ui::SETTINGS_EDITOR_SECTION_SCROLLING).into(),
            ),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_EDITOR_SCROLL_BEYOND_LAST_LINE).into(),
                description: t(
                    keys::settings_ui::SETTINGS_EDITOR_WHETHER_THE_EDITOR_WILL_SCROLL_BEYOND_TH,
                )
                .into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("scroll_beyond_last_line"),
                    pick: |settings_content| {
                        settings_content.editor.scroll_beyond_last_line.as_ref()
                    },
                    write: |settings_content, value, _| {
                        settings_content.editor.scroll_beyond_last_line = value;
                    },
                }),
                metadata: None,
                files: USER,
            }),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_EDITOR_VERTICAL_SCROLL_MARGIN).into(),
                description: t(
                    keys::settings_ui::SETTINGS_EDITOR_THE_NUMBER_OF_LINES_TO_KEEP_ABOVE_BELOW,
                )
                .into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("vertical_scroll_margin"),
                    pick: |settings_content| {
                        settings_content.editor.vertical_scroll_margin.as_ref()
                    },
                    write: |settings_content, value, _| {
                        settings_content.editor.vertical_scroll_margin = value;
                    },
                }),
                metadata: None,
                files: USER,
            }),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_EDITOR_HORIZONTAL_SCROLL_MARGIN).into(),
                description: t(
                    keys::settings_ui::SETTINGS_EDITOR_THE_NUMBER_OF_CHARACTERS_TO_KEEP_ON_EITH,
                )
                .into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("horizontal_scroll_margin"),
                    pick: |settings_content| {
                        settings_content.editor.horizontal_scroll_margin.as_ref()
                    },
                    write: |settings_content, value, _| {
                        settings_content.editor.horizontal_scroll_margin = value;
                    },
                }),
                metadata: None,
                files: USER,
            }),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_EDITOR_SCROLL_SENSITIVITY).into(),
                description: t(
                    keys::settings_ui::SETTINGS_EDITOR_SCROLL_SENSITIVITY_MULTIPLIER_FOR_BOTH_H,
                )
                .into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("scroll_sensitivity"),
                    pick: |settings_content| settings_content.editor.scroll_sensitivity.as_ref(),
                    write: |settings_content, value, _| {
                        settings_content.editor.scroll_sensitivity = value;
                    },
                }),
                metadata: None,
                files: USER,
            }),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_EDITOR_MOUSE_WHEEL_ZOOM).into(),
                description: t(
                    keys::settings_ui::SETTINGS_EDITOR_WHETHER_TO_ZOOM_THE_EDITOR_FONT_SIZE_WIT,
                )
                .into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("mouse_wheel_zoom"),
                    pick: |settings_content| settings_content.editor.mouse_wheel_zoom.as_ref(),
                    write: |settings_content, value, _| {
                        settings_content.editor.mouse_wheel_zoom = value;
                    },
                }),
                metadata: None,
                files: USER,
            }),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_EDITOR_FAST_SCROLL_SENSITIVITY).into(),
                description: t(
                    keys::settings_ui::SETTINGS_EDITOR_FAST_SCROLL_SENSITIVITY_MULTIPLIER_FOR_B,
                )
                .into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("fast_scroll_sensitivity"),
                    pick: |settings_content| {
                        settings_content.editor.fast_scroll_sensitivity.as_ref()
                    },
                    write: |settings_content, value, _| {
                        settings_content.editor.fast_scroll_sensitivity = value;
                    },
                }),
                metadata: None,
                files: USER,
            }),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_EDITOR_AUTOSCROLL_ON_CLICKS).into(),
                description: t(
                    keys::settings_ui::SETTINGS_EDITOR_WHETHER_TO_SCROLL_WHEN_CLICKING_NEAR_THE,
                )
                .into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("autoscroll_on_clicks"),
                    pick: |settings_content| settings_content.editor.autoscroll_on_clicks.as_ref(),
                    write: |settings_content, value, _| {
                        settings_content.editor.autoscroll_on_clicks = value;
                    },
                }),
                metadata: None,
                files: USER,
            }),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_EDITOR_STICKY_SCROLL).into(),
                description: t(
                    keys::settings_ui::SETTINGS_EDITOR_WHETHER_TO_STICK_SCOPES_TO_THE_TOP_OF_TH,
                )
                .into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("sticky_scroll.enabled"),
                    pick: |settings_content| {
                        settings_content
                            .editor
                            .sticky_scroll
                            .as_ref()
                            .and_then(|sticky_scroll| sticky_scroll.enabled.as_ref())
                    },
                    write: |settings_content, value, _| {
                        settings_content
                            .editor
                            .sticky_scroll
                            .get_or_insert_default()
                            .enabled = value;
                    },
                }),
                metadata: None,
                files: USER,
            }),
        ]
    }

    fn signature_help_section() -> [SettingsPageItem; 4] {
        [
            SettingsPageItem::SectionHeader(
                t(keys::settings_ui::SETTINGS_EDITOR_SECTION_SIGNATURE_HELP).into(),
            ),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_EDITOR_AUTO_SIGNATURE_HELP).into(),
                description: t(
                    keys::settings_ui::SETTINGS_EDITOR_AUTOMATICALLY_SHOW_A_SIGNATURE_HELP_POP,
                )
                .into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("auto_signature_help"),
                    pick: |settings_content| settings_content.editor.auto_signature_help.as_ref(),
                    write: |settings_content, value, _| {
                        settings_content.editor.auto_signature_help = value;
                    },
                }),
                metadata: None,
                files: USER,
            }),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_EDITOR_SHOW_SIGNATURE_HELP_AFTER_EDITS).into(),
                description: t(
                    keys::settings_ui::SETTINGS_EDITOR_SHOW_THE_SIGNATURE_HELP_POP_UP_AFTER_COM,
                )
                .into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("show_signature_help_after_edits"),
                    pick: |settings_content| {
                        settings_content
                            .editor
                            .show_signature_help_after_edits
                            .as_ref()
                    },
                    write: |settings_content, value, _| {
                        settings_content.editor.show_signature_help_after_edits = value;
                    },
                }),
                metadata: None,
                files: USER,
            }),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_EDITOR_SNIPPET_SORT_ORDER).into(),
                description: t(
                    keys::settings_ui::SETTINGS_EDITOR_DETERMINES_HOW_SNIPPETS_ARE_SORTED_RELAT,
                )
                .into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("snippet_sort_order"),
                    pick: |settings_content| settings_content.editor.snippet_sort_order.as_ref(),
                    write: |settings_content, value, _| {
                        settings_content.editor.snippet_sort_order = value;
                    },
                }),
                metadata: None,
                files: USER,
            }),
        ]
    }

    fn hover_popover_section() -> [SettingsPageItem; 5] {
        [
            SettingsPageItem::SectionHeader(
                t(keys::settings_ui::SETTINGS_EDITOR_SECTION_HOVER_POPOVER).into(),
            ),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_EDITOR_ENABLED).into(),
                description: t(
                    keys::settings_ui::SETTINGS_EDITOR_SHOW_THE_INFORMATIONAL_HOVER_BOX_WHEN_MO,
                )
                .into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("hover_popover_enabled"),
                    pick: |settings_content| settings_content.editor.hover_popover_enabled.as_ref(),
                    write: |settings_content, value, _| {
                        settings_content.editor.hover_popover_enabled = value;
                    },
                }),
                metadata: None,
                files: USER,
            }),
            // todo(settings ui): add units to this number input
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_EDITOR_DELAY).into(),
                description: t(
                    keys::settings_ui::SETTINGS_EDITOR_TIME_TO_WAIT_IN_MILLISECONDS_BEFORE_SHOW,
                )
                .into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("hover_popover_delay"),
                    pick: |settings_content| settings_content.editor.hover_popover_delay.as_ref(),
                    write: |settings_content, value, _| {
                        settings_content.editor.hover_popover_delay = value;
                    },
                }),
                metadata: None,
                files: USER,
            }),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_EDITOR_STICKY).into(),
                description: t(
                    keys::settings_ui::SETTINGS_EDITOR_WHETHER_THE_HOVER_POPOVER_STICKS_WHEN_TH,
                )
                .into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("hover_popover_sticky"),
                    pick: |settings_content| settings_content.editor.hover_popover_sticky.as_ref(),
                    write: |settings_content, value, _| {
                        settings_content.editor.hover_popover_sticky = value;
                    },
                }),
                metadata: None,
                files: USER,
            }),
            // todo(settings ui): add units to this number input
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_EDITOR_HIDING_DELAY).into(),
                description: t(
                    keys::settings_ui::SETTINGS_EDITOR_TIME_TO_WAIT_IN_MILLISECONDS_BEFORE_HIDI,
                )
                .into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("hover_popover_hiding_delay"),
                    pick: |settings_content| {
                        settings_content.editor.hover_popover_hiding_delay.as_ref()
                    },
                    write: |settings_content, value, _| {
                        settings_content.editor.hover_popover_hiding_delay = value;
                    },
                }),
                metadata: None,
                files: USER,
            }),
        ]
    }

    fn drag_and_drop_selection_section() -> [SettingsPageItem; 3] {
        [
            SettingsPageItem::SectionHeader(
                t(keys::settings_ui::SETTINGS_EDITOR_SECTION_DRAG_AND_DROP_SELECTION).into(),
            ),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_EDITOR_ENABLED).into(),
                description: t(keys::settings_ui::SETTINGS_EDITOR_ENABLE_DRAG_AND_DROP_SELECTION)
                    .into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("drag_and_drop_selection.enabled"),
                    pick: |settings_content| {
                        settings_content
                            .editor
                            .drag_and_drop_selection
                            .as_ref()
                            .and_then(|drag_and_drop| drag_and_drop.enabled.as_ref())
                    },
                    write: |settings_content, value, _| {
                        settings_content
                            .editor
                            .drag_and_drop_selection
                            .get_or_insert_default()
                            .enabled = value;
                    },
                }),
                metadata: None,
                files: USER,
            }),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_EDITOR_DELAY).into(),
                description: t(
                    keys::settings_ui::SETTINGS_EDITOR_DELAY_IN_MILLISECONDS_BEFORE_DRAG_AND_DR,
                )
                .into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("drag_and_drop_selection.delay"),
                    pick: |settings_content| {
                        settings_content
                            .editor
                            .drag_and_drop_selection
                            .as_ref()
                            .and_then(|drag_and_drop| drag_and_drop.delay.as_ref())
                    },
                    write: |settings_content, value, _| {
                        settings_content
                            .editor
                            .drag_and_drop_selection
                            .get_or_insert_default()
                            .delay = value;
                    },
                }),
                metadata: None,
                files: USER,
            }),
        ]
    }

    fn gutter_section() -> [SettingsPageItem; 10] {
        [
            SettingsPageItem::SectionHeader(t(keys::settings_ui::SETTINGS_EDITOR_SECTION_GUTTER).into()),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_EDITOR_SHOW_LINE_NUMBERS).into(),
                description: t(keys::settings_ui::SETTINGS_EDITOR_SHOW_LINE_NUMBERS_IN_THE_GUTTER).into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("gutter.line_numbers"),
                    pick: |settings_content| {
                        settings_content
                            .editor
                            .gutter
                            .as_ref()
                            .and_then(|gutter| gutter.line_numbers.as_ref())
                    },
                    write: |settings_content, value, _| {
                        settings_content
                            .editor
                            .gutter
                            .get_or_insert_default()
                            .line_numbers = value;
                    },
                }),
                metadata: None,
                files: USER,
            }),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_EDITOR_RELATIVE_LINE_NUMBERS).into(),
                description: "Controls line number display in the editor's gutter. \"disabled\" shows absolute line numbers, \"enabled\" shows relative line numbers for each absolute line, and \"wrapped\" shows relative line numbers for every line, absolute or wrapped.".into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("relative_line_numbers"),
                    pick: |settings_content| settings_content.editor.relative_line_numbers.as_ref(),
                    write: |settings_content, value, _| {
                        settings_content.editor.relative_line_numbers = value;
                    },
                }),
                metadata: None,
                files: USER,
            }),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_EDITOR_SHOW_RUNNABLES).into(),
                description: t(keys::settings_ui::SETTINGS_EDITOR_SHOW_RUNNABLE_BUTTONS_IN_THE_GUTTER).into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("gutter.runnables"),
                    pick: |settings_content| {
                        settings_content
                            .editor
                            .gutter
                            .as_ref()
                            .and_then(|gutter| gutter.runnables.as_ref())
                    },
                    write: |settings_content, value, _| {
                        settings_content
                            .editor
                            .gutter
                            .get_or_insert_default()
                            .runnables = value;
                    },
                }),
                metadata: None,
                files: USER,
            }),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_EDITOR_SHOW_BREAKPOINTS).into(),
                description: t(keys::settings_ui::SETTINGS_EDITOR_SHOW_BREAKPOINTS_IN_THE_GUTTER).into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("gutter.breakpoints"),
                    pick: |settings_content| {
                        settings_content
                            .editor
                            .gutter
                            .as_ref()
                            .and_then(|gutter| gutter.breakpoints.as_ref())
                    },
                    write: |settings_content, value, _| {
                        settings_content
                            .editor
                            .gutter
                            .get_or_insert_default()
                            .breakpoints = value;
                    },
                }),
                metadata: None,
                files: USER,
            }),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_EDITOR_SHOW_BOOKMARKS).into(),
                description: t(keys::settings_ui::SETTINGS_EDITOR_SHOW_BOOKMARKS_IN_THE_GUTTER).into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("gutter.bookmarks"),
                    pick: |settings_content| {
                        settings_content
                            .editor
                            .gutter
                            .as_ref()
                            .and_then(|gutter| gutter.bookmarks.as_ref())
                    },
                    write: |settings_content, value, _| {
                        settings_content
                            .editor
                            .gutter
                            .get_or_insert_default()
                            .bookmarks = value;
                    },
                }),
                metadata: None,
                files: USER,
            }),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_EDITOR_SHOW_FOLDS).into(),
                description: t(keys::settings_ui::SETTINGS_EDITOR_SHOW_CODE_FOLDING_CONTROLS_IN_THE_GUTTER).into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("gutter.folds"),
                    pick: |settings_content| {
                        settings_content
                            .editor
                            .gutter
                            .as_ref()
                            .and_then(|gutter| gutter.folds.as_ref())
                    },
                    write: |settings_content, value, _| {
                        settings_content.editor.gutter.get_or_insert_default().folds = value;
                    },
                }),
                metadata: None,
                files: USER,
            }),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_EDITOR_MIN_LINE_NUMBER_DIGITS).into(),
                description: t(keys::settings_ui::SETTINGS_EDITOR_MINIMUM_NUMBER_OF_CHARACTERS_TO_RESERVE).into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("gutter.min_line_number_digits"),
                    pick: |settings_content| {
                        settings_content
                            .editor
                            .gutter
                            .as_ref()
                            .and_then(|gutter| gutter.min_line_number_digits.as_ref())
                    },
                    write: |settings_content, value, _| {
                        settings_content
                            .editor
                            .gutter
                            .get_or_insert_default()
                            .min_line_number_digits = value;
                    },
                }),
                metadata: None,
                files: USER,
            }),
            SettingsPageItem::DynamicItem(DynamicItem {
                discriminant: SettingItem {
                    title: t(keys::settings_ui::SETTINGS_EDITOR_GIT_GUTTER_WIDTH).into(),
                    description: t(keys::settings_ui::SETTINGS_EDITOR_WIDTH_OF_THE_GIT_DIFF_INDICATORS_IN_THE).into(),
                    field: Box::new(SettingField {
                        organization_override: None,
                        json_path: Some("gutter.git_gutter_width$"),
                        pick: |settings_content| {
                            Some(
                                &dynamic_variants::<settings::GitGutterWidth>()[settings_content
                                    .editor
                                    .gutter
                                    .as_ref()?
                                    .git_gutter_width
                                    .as_ref()?
                                    .discriminant()
                                    as usize],
                            )
                        },
                        write: |settings_content, value, _| {
                            let gutter = settings_content.editor.gutter.get_or_insert_default();
                            gutter.git_gutter_width = value.map(|value| match value {
                                settings::GitGutterWidthDiscriminants::Default => {
                                    settings::GitGutterWidth::Default
                                }
                                settings::GitGutterWidthDiscriminants::Custom => {
                                    let width = match gutter.git_gutter_width {
                                        Some(settings::GitGutterWidth::Custom(width)) => {
                                            settings::PixelSetting(*width)
                                        }
                                        _ => settings::PixelSetting(3.0),
                                    };
                                    settings::GitGutterWidth::Custom(width)
                                }
                            });
                        },
                    }),
                    metadata: None,
                    files: USER,
                },
                pick_discriminant: |settings_content| {
                    Some(
                        settings_content
                            .editor
                            .gutter
                            .as_ref()?
                            .git_gutter_width
                            .as_ref()?
                            .discriminant() as usize,
                    )
                },
                fields: dynamic_variants::<settings::GitGutterWidth>()
                    .into_iter()
                    .map(|variant| match variant {
                        settings::GitGutterWidthDiscriminants::Default => vec![],
                        settings::GitGutterWidthDiscriminants::Custom => vec![SettingItem {
                            files: USER,
                            title: t(keys::settings_ui::SETTINGS_EDITOR_CUSTOM_WIDTH).into(),
                            description: t(keys::settings_ui::SETTINGS_EDITOR_WIDTH_IN_PIXELS_OF_THE_GIT_DIFF_INDICATO).into(),
                            field: Box::new(SettingField {
                                organization_override: None,
                                json_path: Some("gutter.git_gutter_width"),
                                pick: |settings_content| match settings_content
                                    .editor
                                    .gutter
                                    .as_ref()
                                    .and_then(|gutter| gutter.git_gutter_width.as_ref())
                                {
                                    Some(settings::GitGutterWidth::Custom(value)) => Some(value),
                                    _ => None,
                                },
                                write: |settings_content, value, _| {
                                    let Some(value) = value else {
                                        return;
                                    };
                                    if let Some(settings::GitGutterWidth::Custom(width)) =
                                        settings_content
                                            .editor
                                            .gutter
                                            .as_mut()
                                            .and_then(|gutter| gutter.git_gutter_width.as_mut())
                                    {
                                        *width = value;
                                    }
                                },
                            }),
                            metadata: None,
                        }],
                    })
                    .collect(),
            }),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_EDITOR_INLINE_CODE_ACTIONS).into(),
                description: t(keys::settings_ui::SETTINGS_EDITOR_SHOW_CODE_ACTION_BUTTON_AT_START_OF_BUFF).into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("inline_code_actions"),
                    pick: |settings_content| settings_content.editor.inline_code_actions.as_ref(),
                    write: |settings_content, value, _| {
                        settings_content.editor.inline_code_actions = value;
                    },
                }),
                metadata: None,
                files: USER,
            }),
        ]
    }

    fn scrollbar_section() -> [SettingsPageItem; 10] {
        [
            SettingsPageItem::SectionHeader(
                t(keys::settings_ui::SETTINGS_EDITOR_SECTION_SCROLLBAR).into(),
            ),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_EDITOR_SHOW).into(),
                description: t(
                    keys::settings_ui::SETTINGS_EDITOR_WHEN_TO_SHOW_THE_SCROLLBAR_IN_THE_EDITOR,
                )
                .into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("scrollbar"),
                    pick: |settings_content| {
                        settings_content.editor.scrollbar.as_ref()?.show.as_ref()
                    },
                    write: |settings_content, value, _| {
                        settings_content
                            .editor
                            .scrollbar
                            .get_or_insert_default()
                            .show = value;
                    },
                }),
                metadata: None,
                files: USER,
            }),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_EDITOR_CURSORS).into(),
                description: t(
                    keys::settings_ui::SETTINGS_EDITOR_SHOW_CURSOR_POSITIONS_IN_THE_SCROLLBAR,
                )
                .into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("scrollbar.cursors"),
                    pick: |settings_content| {
                        settings_content.editor.scrollbar.as_ref()?.cursors.as_ref()
                    },
                    write: |settings_content, value, _| {
                        settings_content
                            .editor
                            .scrollbar
                            .get_or_insert_default()
                            .cursors = value;
                    },
                }),
                metadata: None,
                files: USER,
            }),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_EDITOR_GIT_DIFF).into(),
                description: t(
                    keys::settings_ui::SETTINGS_EDITOR_SHOW_GIT_DIFF_INDICATORS_IN_THE_SCROLLBA,
                )
                .into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("scrollbar.git_diff"),
                    pick: |settings_content| {
                        settings_content
                            .editor
                            .scrollbar
                            .as_ref()?
                            .git_diff
                            .as_ref()
                    },
                    write: |settings_content, value, _| {
                        settings_content
                            .editor
                            .scrollbar
                            .get_or_insert_default()
                            .git_diff = value;
                    },
                }),
                metadata: None,
                files: USER,
            }),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_EDITOR_SEARCH_RESULTS).into(),
                description: t(
                    keys::settings_ui::SETTINGS_EDITOR_SHOW_BUFFER_SEARCH_RESULT_INDICATORS_IN,
                )
                .into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("scrollbar.search_results"),
                    pick: |settings_content| {
                        settings_content
                            .editor
                            .scrollbar
                            .as_ref()?
                            .search_results
                            .as_ref()
                    },
                    write: |settings_content, value, _| {
                        settings_content
                            .editor
                            .scrollbar
                            .get_or_insert_default()
                            .search_results = value;
                    },
                }),
                metadata: None,
                files: USER,
            }),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_EDITOR_SELECTED_TEXT).into(),
                description: t(
                    keys::settings_ui::SETTINGS_EDITOR_SHOW_SELECTED_TEXT_OCCURRENCES_IN_THE_SC,
                )
                .into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("scrollbar.selected_text"),
                    pick: |settings_content| {
                        settings_content
                            .editor
                            .scrollbar
                            .as_ref()?
                            .selected_text
                            .as_ref()
                    },
                    write: |settings_content, value, _| {
                        settings_content
                            .editor
                            .scrollbar
                            .get_or_insert_default()
                            .selected_text = value;
                    },
                }),
                metadata: None,
                files: USER,
            }),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_EDITOR_SELECTED_SYMBOL).into(),
                description: t(
                    keys::settings_ui::SETTINGS_EDITOR_SHOW_SELECTED_SYMBOL_OCCURRENCES_IN_THE,
                )
                .into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("scrollbar.selected_symbol"),
                    pick: |settings_content| {
                        settings_content
                            .editor
                            .scrollbar
                            .as_ref()?
                            .selected_symbol
                            .as_ref()
                    },
                    write: |settings_content, value, _| {
                        settings_content
                            .editor
                            .scrollbar
                            .get_or_insert_default()
                            .selected_symbol = value;
                    },
                }),
                metadata: None,
                files: USER,
            }),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::agent::AGENT_B_DIAGNOSTICS).into(),
                description: t(
                    keys::settings_ui::SETTINGS_EDITOR_WHICH_DIAGNOSTIC_INDICATORS_TO_SHOW_IN_T,
                )
                .into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("scrollbar.diagnostics"),
                    pick: |settings_content| {
                        settings_content
                            .editor
                            .scrollbar
                            .as_ref()?
                            .diagnostics
                            .as_ref()
                    },
                    write: |settings_content, value, _| {
                        settings_content
                            .editor
                            .scrollbar
                            .get_or_insert_default()
                            .diagnostics = value;
                    },
                }),
                metadata: None,
                files: USER,
            }),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_EDITOR_HORIZONTAL_SCROLLBAR).into(),
                description: t(
                    keys::settings_ui::SETTINGS_EDITOR_WHEN_FALSE_FORCEFULLY_DISABLES_THE_HORIZ,
                )
                .into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("scrollbar.axes.horizontal"),
                    pick: |settings_content| {
                        settings_content
                            .editor
                            .scrollbar
                            .as_ref()?
                            .axes
                            .as_ref()?
                            .horizontal
                            .as_ref()
                    },
                    write: |settings_content, value, _| {
                        settings_content
                            .editor
                            .scrollbar
                            .get_or_insert_default()
                            .axes
                            .get_or_insert_default()
                            .horizontal = value;
                    },
                }),
                metadata: None,
                files: USER,
            }),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_EDITOR_VERTICAL_SCROLLBAR).into(),
                description: t(
                    keys::settings_ui::SETTINGS_EDITOR_WHEN_FALSE_FORCEFULLY_DISABLES_THE_VERTI,
                )
                .into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("scrollbar.axes.vertical"),
                    pick: |settings_content| {
                        settings_content
                            .editor
                            .scrollbar
                            .as_ref()?
                            .axes
                            .as_ref()?
                            .vertical
                            .as_ref()
                    },
                    write: |settings_content, value, _| {
                        settings_content
                            .editor
                            .scrollbar
                            .get_or_insert_default()
                            .axes
                            .get_or_insert_default()
                            .vertical = value;
                    },
                }),
                metadata: None,
                files: USER,
            }),
        ]
    }

    fn minimap_section() -> [SettingsPageItem; 7] {
        [
            SettingsPageItem::SectionHeader(
                t(keys::settings_ui::SETTINGS_EDITOR_SECTION_MINIMAP).into(),
            ),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_EDITOR_SHOW).into(),
                description: t(
                    keys::settings_ui::SETTINGS_EDITOR_WHEN_TO_SHOW_THE_MINIMAP_IN_THE_EDITOR,
                )
                .into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("minimap.show"),
                    pick: |settings_content| {
                        settings_content.editor.minimap.as_ref()?.show.as_ref()
                    },
                    write: |settings_content, value, _| {
                        settings_content.editor.minimap.get_or_insert_default().show = value;
                    },
                }),
                metadata: None,
                files: USER,
            }),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_EDITOR_DISPLAY_IN).into(),
                description: t(
                    keys::settings_ui::SETTINGS_EDITOR_WHERE_TO_SHOW_THE_MINIMAP_IN_THE_EDITOR,
                )
                .into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("minimap.display_in"),
                    pick: |settings_content| {
                        settings_content
                            .editor
                            .minimap
                            .as_ref()?
                            .display_in
                            .as_ref()
                    },
                    write: |settings_content, value, _| {
                        settings_content
                            .editor
                            .minimap
                            .get_or_insert_default()
                            .display_in = value;
                    },
                }),
                metadata: None,
                files: USER,
            }),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_EDITOR_THUMB).into(),
                description: t(keys::settings_ui::SETTINGS_EDITOR_WHEN_TO_SHOW_THE_MINIMAP_THUMB)
                    .into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("minimap.thumb"),
                    pick: |settings_content| {
                        settings_content.editor.minimap.as_ref()?.thumb.as_ref()
                    },
                    write: |settings_content, value, _| {
                        settings_content
                            .editor
                            .minimap
                            .get_or_insert_default()
                            .thumb = value;
                    },
                }),
                metadata: None,
                files: USER,
            }),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_EDITOR_THUMB_BORDER).into(),
                description: t(
                    keys::settings_ui::SETTINGS_EDITOR_BORDER_STYLE_FOR_THE_MINIMAP_S_SCROLLBAR,
                )
                .into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("minimap.thumb_border"),
                    pick: |settings_content| {
                        settings_content
                            .editor
                            .minimap
                            .as_ref()?
                            .thumb_border
                            .as_ref()
                    },
                    write: |settings_content, value, _| {
                        settings_content
                            .editor
                            .minimap
                            .get_or_insert_default()
                            .thumb_border = value;
                    },
                }),
                metadata: None,
                files: USER,
            }),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_APPEARANCE_CURRENT_LINE_HIGHLIGHT).into(),
                description: t(
                    keys::settings_ui::SETTINGS_EDITOR_HOW_TO_HIGHLIGHT_THE_CURRENT_LINE_IN_THE,
                )
                .into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("minimap.current_line_highlight"),
                    pick: |settings_content| {
                        settings_content
                            .editor
                            .minimap
                            .as_ref()
                            .and_then(|minimap| minimap.current_line_highlight.as_ref())
                            .or(settings_content.editor.current_line_highlight.as_ref())
                    },
                    write: |settings_content, value, _| {
                        settings_content
                            .editor
                            .minimap
                            .get_or_insert_default()
                            .current_line_highlight = value;
                    },
                }),
                metadata: None,
                files: USER,
            }),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_EDITOR_MAX_WIDTH_COLUMNS).into(),
                description: t(
                    keys::settings_ui::SETTINGS_EDITOR_MAXIMUM_NUMBER_OF_COLUMNS_TO_DISPLAY_IN,
                )
                .into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("minimap.max_width_columns"),
                    pick: |settings_content| {
                        settings_content
                            .editor
                            .minimap
                            .as_ref()?
                            .max_width_columns
                            .as_ref()
                    },
                    write: |settings_content, value, _| {
                        settings_content
                            .editor
                            .minimap
                            .get_or_insert_default()
                            .max_width_columns = value;
                    },
                }),
                metadata: None,
                files: USER,
            }),
        ]
    }

    fn toolbar_section() -> [SettingsPageItem; 6] {
        [
            SettingsPageItem::SectionHeader(
                t(keys::settings_ui::SETTINGS_EDITOR_SECTION_TOOLBAR).into(),
            ),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_EDITOR_BREADCRUMBS).into(),
                description: t(keys::settings_ui::SETTINGS_EDITOR_SHOW_BREADCRUMBS).into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("toolbar.breadcrumbs"),
                    pick: |settings_content| {
                        settings_content
                            .editor
                            .toolbar
                            .as_ref()?
                            .breadcrumbs
                            .as_ref()
                    },
                    write: |settings_content, value, _| {
                        settings_content
                            .editor
                            .toolbar
                            .get_or_insert_default()
                            .breadcrumbs = value;
                    },
                }),
                metadata: None,
                files: USER,
            }),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_EDITOR_QUICK_ACTIONS).into(),
                description: t(keys::settings_ui::SETTINGS_TOOLBAR_QUICK_ACTIONS_HINT).into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("toolbar.quick_actions"),
                    pick: |settings_content| {
                        settings_content
                            .editor
                            .toolbar
                            .as_ref()?
                            .quick_actions
                            .as_ref()
                    },
                    write: |settings_content, value, _| {
                        settings_content
                            .editor
                            .toolbar
                            .get_or_insert_default()
                            .quick_actions = value;
                    },
                }),
                metadata: None,
                files: USER,
            }),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_EDITOR_SELECTIONS_MENU).into(),
                description: t(
                    keys::settings_ui::SETTINGS_EDITOR_SHOW_THE_SELECTIONS_MENU_IN_THE_EDITOR_T,
                )
                .into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("toolbar.selections_menu"),
                    pick: |settings_content| {
                        settings_content
                            .editor
                            .toolbar
                            .as_ref()?
                            .selections_menu
                            .as_ref()
                    },
                    write: |settings_content, value, _| {
                        settings_content
                            .editor
                            .toolbar
                            .get_or_insert_default()
                            .selections_menu = value;
                    },
                }),
                metadata: None,
                files: USER,
            }),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_EDITOR_AGENT_REVIEW).into(),
                description: t(
                    keys::settings_ui::SETTINGS_EDITOR_SHOW_AGENT_REVIEW_BUTTONS_IN_THE_EDITOR,
                )
                .into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("toolbar.agent_review"),
                    pick: |settings_content| {
                        settings_content
                            .editor
                            .toolbar
                            .as_ref()?
                            .agent_review
                            .as_ref()
                    },
                    write: |settings_content, value, _| {
                        settings_content
                            .editor
                            .toolbar
                            .get_or_insert_default()
                            .agent_review = value;
                    },
                }),
                metadata: None,
                files: USER,
            }),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_EDITOR_CODE_ACTIONS).into(),
                description: t(
                    keys::settings_ui::SETTINGS_EDITOR_SHOW_CODE_ACTION_BUTTONS_IN_THE_EDITOR_T,
                )
                .into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("toolbar.code_actions"),
                    pick: |settings_content| {
                        settings_content
                            .editor
                            .toolbar
                            .as_ref()?
                            .code_actions
                            .as_ref()
                    },
                    write: |settings_content, value, _| {
                        settings_content
                            .editor
                            .toolbar
                            .get_or_insert_default()
                            .code_actions = value;
                    },
                }),
                metadata: None,
                files: USER,
            }),
        ]
    }

    fn vim_settings_section() -> [SettingsPageItem; 14] {
        [
            SettingsPageItem::SectionHeader("Vim".into()),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_EDITOR_DEFAULT_MODE).into(),
                description: t(keys::settings_ui::SETTINGS_EDITOR_THE_DEFAULT_MODE_WHEN_VIM_STARTS)
                    .into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("vim.default_mode"),
                    pick: |settings_content| settings_content.vim.as_ref()?.default_mode.as_ref(),
                    write: |settings_content, value, _| {
                        settings_content.vim.get_or_insert_default().default_mode = value;
                    },
                }),
                metadata: None,
                files: USER,
            }),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_EDITOR_TOGGLE_RELATIVE_LINE_NUMBERS).into(),
                description: t(
                    keys::settings_ui::SETTINGS_EDITOR_TOGGLE_RELATIVE_LINE_NUMBERS_IN_VIM_MODE,
                )
                .into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("vim.toggle_relative_line_numbers"),
                    pick: |settings_content| {
                        settings_content
                            .vim
                            .as_ref()?
                            .toggle_relative_line_numbers
                            .as_ref()
                    },
                    write: |settings_content, value, _| {
                        settings_content
                            .vim
                            .get_or_insert_default()
                            .toggle_relative_line_numbers = value;
                    },
                }),
                metadata: None,
                files: USER,
            }),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_EDITOR_USE_SYSTEM_CLIPBOARD).into(),
                description: t(
                    keys::settings_ui::SETTINGS_EDITOR_CONTROLS_WHEN_TO_USE_SYSTEM_CLIPBOARD_IN,
                )
                .into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("vim.use_system_clipboard"),
                    pick: |settings_content| {
                        settings_content.vim.as_ref()?.use_system_clipboard.as_ref()
                    },
                    write: |settings_content, value, _| {
                        settings_content
                            .vim
                            .get_or_insert_default()
                            .use_system_clipboard = value;
                    },
                }),
                metadata: None,
                files: USER,
            }),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_EDITOR_USE_SMARTCASE_FIND).into(),
                description: t(
                    keys::settings_ui::SETTINGS_EDITOR_ENABLE_SMARTCASE_SEARCHING_IN_VIM_MODE,
                )
                .into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("vim.use_smartcase_find"),
                    pick: |settings_content| {
                        settings_content.vim.as_ref()?.use_smartcase_find.as_ref()
                    },
                    write: |settings_content, value, _| {
                        settings_content
                            .vim
                            .get_or_insert_default()
                            .use_smartcase_find = value;
                    },
                }),
                metadata: None,
                files: USER,
            }),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_EDITOR_GLOBAL_SUBSTITUTION_DEFAULT).into(),
                description: t(
                    keys::settings_ui::SETTINGS_EDITOR_WHEN_ENABLED_THE_SUBSTITUTE_COMMAND_REPL,
                )
                .into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("vim.gdefault"),
                    pick: |settings_content| settings_content.vim.as_ref()?.gdefault.as_ref(),
                    write: |settings_content, value, _| {
                        settings_content.vim.get_or_insert_default().gdefault = value;
                    },
                }),
                metadata: None,
                files: USER,
            }),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_EDITOR_HIGHLIGHT_ON_YANK_DURATION).into(),
                description: t(
                    keys::settings_ui::SETTINGS_EDITOR_DURATION_IN_MILLISECONDS_TO_HIGHLIGHT_YA,
                )
                .into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("vim.highlight_on_yank_duration"),
                    pick: |settings_content| {
                        settings_content
                            .vim
                            .as_ref()?
                            .highlight_on_yank_duration
                            .as_ref()
                    },
                    write: |settings_content, value, _| {
                        settings_content
                            .vim
                            .get_or_insert_default()
                            .highlight_on_yank_duration = value;
                    },
                }),
                metadata: None,
                files: USER,
            }),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_EDITOR_REGEX_SEARCH).into(),
                description: t(
                    keys::settings_ui::SETTINGS_EDITOR_USE_REGEX_SEARCH_BY_DEFAULT_IN_VIM_SEARC,
                )
                .into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("vim.use_regex_search"),
                    pick: |settings_content| {
                        settings_content.vim.as_ref()?.use_regex_search.as_ref()
                    },
                    write: |settings_content, value, _| {
                        settings_content
                            .vim
                            .get_or_insert_default()
                            .use_regex_search = value;
                    },
                }),
                metadata: None,
                files: USER,
            }),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_EDITOR_SHOW_EDIT_PREDICTIONS_IN_NORMAL_MODE)
                    .into(),
                description: t(
                    keys::settings_ui::SETTINGS_EDITOR_WHETHER_EDIT_PREDICTIONS_ARE_SHOWN_IN_NO,
                )
                .into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("vim.show_edit_predictions_in_normal_mode"),
                    pick: |settings_content| {
                        settings_content
                            .vim
                            .as_ref()?
                            .show_edit_predictions_in_normal_mode
                            .as_ref()
                    },
                    write: |settings_content, value, _| {
                        settings_content
                            .vim
                            .get_or_insert_default()
                            .show_edit_predictions_in_normal_mode = value;
                    },
                }),
                metadata: None,
                files: USER,
            }),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_EDITOR_CURSOR_SHAPE_NORMAL_MODE).into(),
                description: t(keys::settings_ui::SETTINGS_EDITOR_CURSOR_SHAPE_FOR_NORMAL_MODE)
                    .into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("vim.cursor_shape.normal"),
                    pick: |settings_content| {
                        settings_content
                            .vim
                            .as_ref()?
                            .cursor_shape
                            .as_ref()?
                            .normal
                            .as_ref()
                    },
                    write: |settings_content, value, _| {
                        settings_content
                            .vim
                            .get_or_insert_default()
                            .cursor_shape
                            .get_or_insert_default()
                            .normal = value;
                    },
                }),
                metadata: None,
                files: USER,
            }),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_EDITOR_CURSOR_SHAPE_INSERT_MODE).into(),
                description: t(
                    keys::settings_ui::SETTINGS_EDITOR_CURSOR_SHAPE_FOR_INSERT_MODE_INHERIT_USE,
                )
                .into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("vim.cursor_shape.insert"),
                    pick: |settings_content| {
                        settings_content
                            .vim
                            .as_ref()?
                            .cursor_shape
                            .as_ref()?
                            .insert
                            .as_ref()
                    },
                    write: |settings_content, value, _| {
                        settings_content
                            .vim
                            .get_or_insert_default()
                            .cursor_shape
                            .get_or_insert_default()
                            .insert = value;
                    },
                }),
                metadata: None,
                files: USER,
            }),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_EDITOR_CURSOR_SHAPE_REPLACE_MODE).into(),
                description: t(keys::settings_ui::SETTINGS_EDITOR_CURSOR_SHAPE_FOR_REPLACE_MODE)
                    .into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("vim.cursor_shape.replace"),
                    pick: |settings_content| {
                        settings_content
                            .vim
                            .as_ref()?
                            .cursor_shape
                            .as_ref()?
                            .replace
                            .as_ref()
                    },
                    write: |settings_content, value, _| {
                        settings_content
                            .vim
                            .get_or_insert_default()
                            .cursor_shape
                            .get_or_insert_default()
                            .replace = value;
                    },
                }),
                metadata: None,
                files: USER,
            }),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_EDITOR_CURSOR_SHAPE_VISUAL_MODE).into(),
                description: t(keys::settings_ui::SETTINGS_EDITOR_CURSOR_SHAPE_FOR_VISUAL_MODE)
                    .into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("vim.cursor_shape.visual"),
                    pick: |settings_content| {
                        settings_content
                            .vim
                            .as_ref()?
                            .cursor_shape
                            .as_ref()?
                            .visual
                            .as_ref()
                    },
                    write: |settings_content, value, _| {
                        settings_content
                            .vim
                            .get_or_insert_default()
                            .cursor_shape
                            .get_or_insert_default()
                            .visual = value;
                    },
                }),
                metadata: None,
                files: USER,
            }),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_EDITOR_CUSTOM_DIGRAPHS).into(),
                description: t(
                    keys::settings_ui::SETTINGS_EDITOR_CUSTOM_DIGRAPH_MAPPINGS_FOR_VIM_MODE,
                )
                .into(),
                field: Box::new(
                    SettingField {
                        organization_override: None,
                        json_path: Some("vim.custom_digraphs"),
                        pick: |settings_content| {
                            settings_content.vim.as_ref()?.custom_digraphs.as_ref()
                        },
                        write: |settings_content, value, _| {
                            settings_content.vim.get_or_insert_default().custom_digraphs = value;
                        },
                    }
                    .unimplemented(),
                ),
                metadata: None,
                files: USER,
            }),
        ]
    }

    let items = concat_sections!(
        auto_save_section(),
        which_key_section(),
        multibuffer_section(),
        scrolling_section(),
        signature_help_section(),
        hover_popover_section(),
        drag_and_drop_selection_section(),
        gutter_section(),
        scrollbar_section(),
        minimap_section(),
        toolbar_section(),
        vim_settings_section(),
        language_settings_data(),
    );

    SettingsPage {
        title: t(keys::settings_ui::SETTINGS_EDITOR_EDITOR).into(),
        items: items,
    }
}

fn languages_and_tools_page(cx: &App) -> SettingsPage {
    fn file_types_section() -> [SettingsPageItem; 2] {
        [
            SettingsPageItem::SectionHeader(
                t(keys::settings_ui::SETTINGS_LANGUAGES_SECTION_FILE_TYPES).into(),
            ),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_LANGUAGES_FILE_TYPE_ASSOCIATIONS).into(),
                description: t(
                    keys::settings_ui::SETTINGS_LANGUAGES_A_MAPPING_FROM_LANGUAGES_TO_FILES_AND_FI,
                )
                .into(),
                field: Box::new(
                    SettingField {
                        organization_override: None,
                        json_path: Some("file_type_associations"),
                        pick: |settings_content| {
                            settings_content.project.all_languages.file_types.as_ref()
                        },
                        write: |settings_content, value, _| {
                            settings_content.project.all_languages.file_types = value;
                        },
                    }
                    .unimplemented(),
                ),
                metadata: None,
                files: USER | PROJECT,
            }),
        ]
    }

    fn diagnostics_section() -> [SettingsPageItem; 3] {
        [
            SettingsPageItem::SectionHeader(t(keys::agent::AGENT_B_DIAGNOSTICS).into()),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_LANGUAGES_MAX_SEVERITY).into(),
                description: t(keys::settings_ui::SETTINGS_DIAGNOSTICS_FILTER_LEVEL_HINT).into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("diagnostics_max_severity"),
                    pick: |settings_content| {
                        settings_content.editor.diagnostics_max_severity.as_ref()
                    },
                    write: |settings_content, value, _| {
                        settings_content.editor.diagnostics_max_severity = value;
                    },
                }),
                metadata: None,
                files: USER,
            }),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_LANGUAGES_INCLUDE_WARNINGS).into(),
                description: t(
                    keys::settings_ui::SETTINGS_LANGUAGES_WHETHER_TO_SHOW_WARNINGS_OR_NOT_BY_DEFAU,
                )
                .into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("diagnostics.include_warnings"),
                    pick: |settings_content| {
                        settings_content
                            .diagnostics
                            .as_ref()?
                            .include_warnings
                            .as_ref()
                    },
                    write: |settings_content, value, _| {
                        settings_content
                            .diagnostics
                            .get_or_insert_default()
                            .include_warnings = value;
                    },
                }),
                metadata: None,
                files: USER,
            }),
        ]
    }

    fn inline_diagnostics_section() -> [SettingsPageItem; 5] {
        [
            SettingsPageItem::SectionHeader(
                t(keys::settings_ui::SETTINGS_LANGUAGES_SECTION_INLINE_DIAGNOSTICS).into(),
            ),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_EDITOR_ENABLED).into(),
                description: t(
                    keys::settings_ui::SETTINGS_LANGUAGES_WHETHER_TO_SHOW_DIAGNOSTICS_INLINE_OR_NO,
                )
                .into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("diagnostics.inline.enabled"),
                    pick: |settings_content| {
                        settings_content
                            .diagnostics
                            .as_ref()?
                            .inline
                            .as_ref()?
                            .enabled
                            .as_ref()
                    },
                    write: |settings_content, value, _| {
                        settings_content
                            .diagnostics
                            .get_or_insert_default()
                            .inline
                            .get_or_insert_default()
                            .enabled = value;
                    },
                }),
                metadata: None,
                files: USER,
            }),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_LANGUAGES_UPDATE_DEBOUNCE).into(),
                description: t(
                    keys::settings_ui::SETTINGS_LANGUAGES_THE_DELAY_IN_MILLISECONDS_TO_SHOW_INLINE,
                )
                .into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("diagnostics.inline.update_debounce_ms"),
                    pick: |settings_content| {
                        settings_content
                            .diagnostics
                            .as_ref()?
                            .inline
                            .as_ref()?
                            .update_debounce_ms
                            .as_ref()
                    },
                    write: |settings_content, value, _| {
                        settings_content
                            .diagnostics
                            .get_or_insert_default()
                            .inline
                            .get_or_insert_default()
                            .update_debounce_ms = value;
                    },
                }),
                metadata: None,
                files: USER,
            }),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_LANGUAGES_PADDING).into(),
                description: t(
                    keys::settings_ui::SETTINGS_LANGUAGES_THE_AMOUNT_OF_PADDING_BETWEEN_THE_END_OF,
                )
                .into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("diagnostics.inline.padding"),
                    pick: |settings_content| {
                        settings_content
                            .diagnostics
                            .as_ref()?
                            .inline
                            .as_ref()?
                            .padding
                            .as_ref()
                    },
                    write: |settings_content, value, _| {
                        settings_content
                            .diagnostics
                            .get_or_insert_default()
                            .inline
                            .get_or_insert_default()
                            .padding = value;
                    },
                }),
                metadata: None,
                files: USER,
            }),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_LANGUAGES_MINIMUM_COLUMN).into(),
                description: t(
                    keys::settings_ui::SETTINGS_LANGUAGES_THE_MINIMUM_COLUMN_AT_WHICH_TO_DISPLAY_I,
                )
                .into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("diagnostics.inline.min_column"),
                    pick: |settings_content| {
                        settings_content
                            .diagnostics
                            .as_ref()?
                            .inline
                            .as_ref()?
                            .min_column
                            .as_ref()
                    },
                    write: |settings_content, value, _| {
                        settings_content
                            .diagnostics
                            .get_or_insert_default()
                            .inline
                            .get_or_insert_default()
                            .min_column = value;
                    },
                }),
                metadata: None,
                files: USER,
            }),
        ]
    }

    fn lsp_pull_diagnostics_section() -> [SettingsPageItem; 3] {
        [
            SettingsPageItem::SectionHeader(
                t(keys::settings_ui::SETTINGS_LANGUAGES_SECTION_LSP_PULL_DIAGNOSTICS).into(),
            ),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_EDITOR_ENABLED).into(),
                description: t(
                    keys::settings_ui::SETTINGS_LANGUAGES_WHETHER_TO_PULL_FOR_LANGUAGE_SERVER_POWE,
                )
                .into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("diagnostics.lsp_pull_diagnostics.enabled"),
                    pick: |settings_content| {
                        settings_content
                            .diagnostics
                            .as_ref()?
                            .lsp_pull_diagnostics
                            .as_ref()?
                            .enabled
                            .as_ref()
                    },
                    write: |settings_content, value, _| {
                        settings_content
                            .diagnostics
                            .get_or_insert_default()
                            .lsp_pull_diagnostics
                            .get_or_insert_default()
                            .enabled = value;
                    },
                }),
                metadata: None,
                files: USER,
            }),
            // todo(settings_ui): Needs unit
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_LANGUAGES_DEBOUNCE).into(),
                description: t(keys::settings_ui::SETTINGS_DIAGNOSTICS_PULL_DEBOUNCE_HINT).into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("diagnostics.lsp_pull_diagnostics.debounce_ms"),
                    pick: |settings_content| {
                        settings_content
                            .diagnostics
                            .as_ref()?
                            .lsp_pull_diagnostics
                            .as_ref()?
                            .debounce_ms
                            .as_ref()
                    },
                    write: |settings_content, value, _| {
                        settings_content
                            .diagnostics
                            .get_or_insert_default()
                            .lsp_pull_diagnostics
                            .get_or_insert_default()
                            .debounce_ms = value;
                    },
                }),
                metadata: None,
                files: USER,
            }),
        ]
    }

    fn lsp_highlights_section() -> [SettingsPageItem; 2] {
        [
            SettingsPageItem::SectionHeader(
                t(keys::settings_ui::SETTINGS_LANGUAGES_SECTION_LSP_HIGHLIGHTS).into(),
            ),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_LANGUAGES_DEBOUNCE).into(),
                description: t(
                    keys::settings_ui::SETTINGS_LANGUAGES_THE_DEBOUNCE_DELAY_BEFORE_QUERYING_HIGHL,
                )
                .into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("lsp_highlight_debounce"),
                    pick: |settings_content| {
                        settings_content.editor.lsp_highlight_debounce.as_ref()
                    },
                    write: |settings_content, value, _| {
                        settings_content.editor.lsp_highlight_debounce = value;
                    },
                }),
                metadata: None,
                files: USER,
            }),
        ]
    }

    fn languages_list_section(cx: &App) -> Box<[SettingsPageItem]> {
        // todo(settings_ui): Refresh on extension (un)/installed
        // Note that `crates/json_schema_store` solves the same problem, there is probably a way to unify the two
        std::iter::once(SettingsPageItem::SectionHeader(
            t(keys::extensions::EXTENSIONS_PROVIDES_LANGUAGES).into(),
        ))
        .chain(all_language_names(cx).into_iter().map(|language_name| {
            let link = format!("languages.{language_name}");
            SettingsPageItem::SubPageLink(SubPageLink {
                title: language_name,
                r#type: crate::SubPageType::Language,
                description: None,
                search_aliases: &[],
                json_path: Some(link.leak()),
                in_json: true,
                files: USER | PROJECT,
                render: |this, scroll_handle, window, cx| {
                    let items: Box<[SettingsPageItem]> = concat_sections!(
                        language_settings_data(),
                        non_editor_language_settings_data(),
                        edit_prediction_language_settings_section()
                    );
                    this.render_sub_page_items(items.iter().enumerate(), scroll_handle, window, cx)
                        .into_any_element()
                },
            })
        }))
        .collect()
    }

    SettingsPage {
        title: t(keys::settings_ui::SETTINGS_LANGUAGES_LANGUAGES_TOOLS).into(),
        items: {
            concat_sections!(
                non_editor_language_settings_data(),
                file_types_section(),
                diagnostics_section(),
                inline_diagnostics_section(),
                lsp_pull_diagnostics_section(),
                lsp_highlights_section(),
                languages_list_section(cx),
            )
        },
    }
}

fn search_and_files_page() -> SettingsPage {
    fn search_section() -> [SettingsPageItem; 10] {
        [
            SettingsPageItem::SectionHeader(
                t(keys::settings_ui::SETTINGS_SEARCH_SECTION_SEARCH).into(),
            ),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_SEARCH_WHOLE_WORD).into(),
                description: t(
                    keys::settings_ui::SETTINGS_SEARCH_SEARCH_FOR_WHOLE_WORDS_BY_DEFAULT,
                )
                .into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("search.whole_word"),
                    pick: |settings_content| {
                        settings_content.editor.search.as_ref()?.whole_word.as_ref()
                    },
                    write: |settings_content, value, _| {
                        settings_content
                            .editor
                            .search
                            .get_or_insert_default()
                            .whole_word = value;
                    },
                }),
                metadata: None,
                files: USER,
            }),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_SEARCH_CASE_SENSITIVE).into(),
                description: t(
                    keys::settings_ui::SETTINGS_SEARCH_SEARCH_CASE_SENSITIVELY_BY_DEFAULT,
                )
                .into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("search.case_sensitive"),
                    pick: |settings_content| {
                        settings_content
                            .editor
                            .search
                            .as_ref()?
                            .case_sensitive
                            .as_ref()
                    },
                    write: |settings_content, value, _| {
                        settings_content
                            .editor
                            .search
                            .get_or_insert_default()
                            .case_sensitive = value;
                    },
                }),
                metadata: None,
                files: USER,
            }),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_SEARCH_USE_SMARTCASE_SEARCH).into(),
                description: t(
                    keys::settings_ui::SETTINGS_SEARCH_WHETHER_TO_AUTOMATICALLY_ENABLE_CASE_SEN,
                )
                .into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("use_smartcase_search"),
                    pick: |settings_content| settings_content.editor.use_smartcase_search.as_ref(),
                    write: |settings_content, value, _| {
                        settings_content.editor.use_smartcase_search = value;
                    },
                }),
                metadata: None,
                files: USER,
            }),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_SEARCH_INCLUDE_IGNORED).into(),
                description: t(
                    keys::settings_ui::SETTINGS_SEARCH_INCLUDE_IGNORED_FILES_IN_SEARCH_RESULTS,
                )
                .into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("search.include_ignored"),
                    pick: |settings_content| {
                        settings_content
                            .editor
                            .search
                            .as_ref()?
                            .include_ignored
                            .as_ref()
                    },
                    write: |settings_content, value, _| {
                        settings_content
                            .editor
                            .search
                            .get_or_insert_default()
                            .include_ignored = value;
                    },
                }),
                metadata: None,
                files: USER,
            }),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_SEARCH_REGEX).into(),
                description: t(keys::settings_ui::SETTINGS_SEARCH_USE_REGEX_SEARCH_BY_DEFAULT)
                    .into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("search.regex"),
                    pick: |settings_content| {
                        settings_content.editor.search.as_ref()?.regex.as_ref()
                    },
                    write: |settings_content, value, _| {
                        settings_content.editor.search.get_or_insert_default().regex = value;
                    },
                }),
                metadata: None,
                files: USER,
            }),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_SEARCH_SEARCH_WRAP).into(),
                description: t(
                    keys::settings_ui::SETTINGS_SEARCH_WHETHER_THE_EDITOR_SEARCH_RESULTS_WILL_L,
                )
                .into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("search_wrap"),
                    pick: |settings_content| settings_content.editor.search_wrap.as_ref(),
                    write: |settings_content, value, _| {
                        settings_content.editor.search_wrap = value;
                    },
                }),
                metadata: None,
                files: USER,
            }),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_SEARCH_CENTER_ON_MATCH).into(),
                description: t(
                    keys::settings_ui::SETTINGS_SEARCH_WHETHER_TO_CENTER_THE_CURRENT_MATCH_IN_T,
                )
                .into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("editor.search.center_on_match"),
                    pick: |settings_content| {
                        settings_content
                            .editor
                            .search
                            .as_ref()
                            .and_then(|search| search.center_on_match.as_ref())
                    },
                    write: |settings_content, value, _| {
                        settings_content
                            .editor
                            .search
                            .get_or_insert_default()
                            .center_on_match = value;
                    },
                }),
                metadata: None,
                files: USER,
            }),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_SEARCH_SEARCH_ON_TYPE).into(),
                description: t(
                    keys::settings_ui::SETTINGS_SEARCH_START_SEARCHING_AS_YOU_TYPE_IN_PROJECT_S,
                )
                .into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("editor.search.search_on_type"),
                    pick: |settings_content| {
                        settings_content
                            .editor
                            .search
                            .as_ref()
                            .and_then(|search| search.search_on_type.as_ref())
                    },
                    write: |settings_content, value, _| {
                        settings_content
                            .editor
                            .search
                            .get_or_insert_default()
                            .search_on_type = value;
                    },
                }),
                metadata: None,
                files: USER,
            }),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_SEARCH_SEED_SEARCH_QUERY_FROM_CURSOR).into(),
                description: t(
                    keys::settings_ui::SETTINGS_SEARCH_WHEN_TO_POPULATE_A_NEW_SEARCH_S_QUERY_BA,
                )
                .into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("seed_search_query_from_cursor"),
                    pick: |settings_content| {
                        settings_content
                            .editor
                            .seed_search_query_from_cursor
                            .as_ref()
                    },
                    write: |settings_content, value, _| {
                        settings_content.editor.seed_search_query_from_cursor = value;
                    },
                }),
                metadata: None,
                files: USER,
            }),
        ]
    }

    fn command_palette_section() -> [SettingsPageItem; 2] {
        [
            SettingsPageItem::SectionHeader(
                t(keys::settings_ui::SETTINGS_SEARCH_SECTION_COMMAND_PALETTE).into(),
            ),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_SEARCH_USE_COMMAND_HISTORY).into(),
                description: t(keys::settings_ui::SETTINGS_COMMAND_PALETTE_HISTORY_RANKING_HINT)
                    .into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("command_palette.use_command_history"),
                    pick: |settings_content| {
                        settings_content
                            .command_palette
                            .as_ref()?
                            .use_command_history
                            .as_ref()
                    },
                    write: |settings_content, value, _| {
                        settings_content
                            .command_palette
                            .get_or_insert_default()
                            .use_command_history = value;
                    },
                }),
                metadata: None,
                files: USER,
            }),
        ]
    }

    fn file_finder_section() -> [SettingsPageItem; 5] {
        [
            SettingsPageItem::SectionHeader(
                t(keys::settings_ui::SETTINGS_SEARCH_SECTION_FILE_FINDER).into(),
            ),
            // todo: null by default
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_SEARCH_INCLUDE_IGNORED_IN_SEARCH).into(),
                description: t(
                    keys::settings_ui::SETTINGS_SEARCH_USE_GITIGNORED_FILES_WHEN_SEARCHING,
                )
                .into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("file_finder.include_ignored"),
                    pick: |settings_content| {
                        settings_content
                            .file_finder
                            .as_ref()?
                            .include_ignored
                            .as_ref()
                    },
                    write: |settings_content, value, _| {
                        settings_content
                            .file_finder
                            .get_or_insert_default()
                            .include_ignored = value;
                    },
                }),
                metadata: None,
                files: USER,
            }),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_SEARCH_FILE_ICONS).into(),
                description: t(
                    keys::settings_ui::SETTINGS_SEARCH_SHOW_FILE_ICONS_IN_THE_FILE_FINDER,
                )
                .into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("file_finder.file_icons"),
                    pick: |settings_content| {
                        settings_content.file_finder.as_ref()?.file_icons.as_ref()
                    },
                    write: |settings_content, value, _| {
                        settings_content
                            .file_finder
                            .get_or_insert_default()
                            .file_icons = value;
                    },
                }),
                metadata: None,
                files: USER,
            }),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_SEARCH_SKIP_FOCUS_FOR_ACTIVE_IN_SEARCH).into(),
                description: t(
                    keys::settings_ui::SETTINGS_SEARCH_WHETHER_THE_FILE_FINDER_SHOULD_SKIP_FOCU,
                )
                .into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("file_finder.skip_focus_for_active_in_search"),
                    pick: |settings_content| {
                        settings_content
                            .file_finder
                            .as_ref()?
                            .skip_focus_for_active_in_search
                            .as_ref()
                    },
                    write: |settings_content, value, _| {
                        settings_content
                            .file_finder
                            .get_or_insert_default()
                            .skip_focus_for_active_in_search = value;
                    },
                }),
                metadata: None,
                files: USER,
            }),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_SEARCH_PREFILL_QUERY_FROM_SELECTION).into(),
                description: t(
                    keys::settings_ui::SETTINGS_SEARCH_WHETHER_TO_PRE_FILL_THE_FILE_FINDER_S_QU,
                )
                .into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("file_finder.prefill_query_from_selection"),
                    pick: |settings_content| {
                        settings_content
                            .file_finder
                            .as_ref()?
                            .prefill_query_from_selection
                            .as_ref()
                    },
                    write: |settings_content, value, _| {
                        settings_content
                            .file_finder
                            .get_or_insert_default()
                            .prefill_query_from_selection = value;
                    },
                }),
                metadata: None,
                files: USER,
            }),
        ]
    }

    fn file_scan_section() -> [SettingsPageItem; 7] {
        [
            SettingsPageItem::SectionHeader(t(keys::settings_ui::SETTINGS_SEARCH_SECTION_FILE_SCAN).into()),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_SEARCH_FILE_SCAN_EXCLUSIONS).into(),
                description: "Files or globs of files that will be excluded by Zed entirely. They will be skipped during file scans, file searches, and not be displayed in the project file tree. Takes precedence over \"File Scan Inclusions\"".into(),
                field: Box::new(
                    SettingField {
                        organization_override: None,
                        json_path: Some("file_scan_exclusions"),
                        pick: |settings_content| {
                            settings_content
                                .project
                                .worktree
                                .file_scan_exclusions
                                .as_ref()
                        },
                        write: |settings_content, value, _| {
                            settings_content.project.worktree.file_scan_exclusions = value;
                        },
                    }
                    .unimplemented(),
                ),
                metadata: None,
                files: USER,
            }),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_SEARCH_FILE_SCAN_INCLUSIONS).into(),
                description: "Files or globs of files that will be included by Zed, even when ignored by git. This is useful for files that are not tracked by git, but are still important to your project. Note that globs that are overly broad can slow down Zed's file scanning. \"File Scan Exclusions\" takes precedence over these inclusions".into(),
                field: Box::new(
                    SettingField {
                        organization_override: None,
                        json_path: Some("file_scan_inclusions"),
                        pick: |settings_content| {
                            settings_content
                                .project
                                .worktree
                                .file_scan_inclusions
                                .as_ref()
                        },
                        write: |settings_content, value, _| {
                            settings_content.project.worktree.file_scan_inclusions = value;
                        },
                    }
                    .unimplemented(),
                ),
                metadata: None,
                files: USER,
            }),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_SEARCH_FILE_SCAN_DEPTH).into(),
                description: t(keys::settings_ui::SETTINGS_SEARCH_MAXIMUM_DIRECTORY_DEPTH_TO_EAGERLY_INDEX).into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("file_scan_depth"),
                    pick: |settings_content| {
                        settings_content.project.worktree.file_scan_depth.as_ref()
                    },
                    write: |settings_content, value, _| {
                        settings_content.project.worktree.file_scan_depth = value;
                    },
                }),
                metadata: None,
                files: USER | PROJECT,
            }),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_SEARCH_SCAN_SYMBOLIC_LINKS).into(),
                description: t(keys::settings_ui::SETTINGS_SEARCH_WHEN_TO_SCAN_CONTENT_OF_LINKED_DIRECTORI).into(),
                field: Box::new(SettingField {
                    json_path: Some("scan_symlinks"),
                    organization_override: None,
                    pick: |settings_content| {
                        settings_content.project.worktree.scan_symlinks.as_ref()
                    },
                    write: |settings_content, value, _| {
                        settings_content.project.worktree.scan_symlinks = value;
                    },
                }),
                metadata: None,
                files: USER,
            }),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_SEARCH_RESTORE_FILE_STATE).into(),
                description: t(keys::settings_ui::SETTINGS_SEARCH_RESTORE_PREVIOUS_FILE_STATE_WHEN_REOPENI).into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("restore_on_file_reopen"),
                    pick: |settings_content| {
                        settings_content.workspace.restore_on_file_reopen.as_ref()
                    },
                    write: |settings_content, value, _| {
                        settings_content.workspace.restore_on_file_reopen = value;
                    },
                }),
                metadata: None,
                files: USER,
            }),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_SEARCH_CLOSE_ON_FILE_DELETE).into(),
                description: t(keys::settings_ui::SETTINGS_SEARCH_AUTOMATICALLY_CLOSE_FILES_THAT_HAVE_BEEN).into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("close_on_file_delete"),
                    pick: |settings_content| {
                        settings_content.workspace.close_on_file_delete.as_ref()
                    },
                    write: |settings_content, value, _| {
                        settings_content.workspace.close_on_file_delete = value;
                    },
                }),
                metadata: None,
                files: USER,
            }),
        ]
    }

    SettingsPage {
        title: t(keys::settings_ui::SETTINGS_SEARCH_SEARCH_FILES).into(),
        items: concat_sections![
            search_section(),
            command_palette_section(),
            file_finder_section(),
            file_scan_section(),
        ],
    }
}

fn window_and_layout_page() -> SettingsPage {
    fn status_bar_section() -> [SettingsPageItem; 12] {
        [
            SettingsPageItem::SectionHeader(
                t(keys::settings_ui::SETTINGS_WINDOW_SECTION_STATUS_BAR).into(),
            ),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_WINDOW_PROJECT_PANEL_BUTTON).into(),
                description: t(
                    keys::settings_ui::SETTINGS_WINDOW_SHOW_THE_PROJECT_PANEL_BUTTON_IN_THE_STA,
                )
                .into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("project_panel.button"),
                    pick: |settings_content| {
                        settings_content.project_panel.as_ref()?.button.as_ref()
                    },
                    write: |settings_content, value, _| {
                        settings_content
                            .project_panel
                            .get_or_insert_default()
                            .button = value;
                    },
                }),
                metadata: None,
                files: USER,
            }),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_WINDOW_ACTIVE_LANGUAGE_BUTTON).into(),
                description: t(
                    keys::settings_ui::SETTINGS_WINDOW_SHOW_THE_ACTIVE_LANGUAGE_BUTTON_IN_THE_S,
                )
                .into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("status_bar.active_language_button"),
                    pick: |settings_content| {
                        settings_content
                            .status_bar
                            .as_ref()?
                            .active_language_button
                            .as_ref()
                    },
                    write: |settings_content, value, _| {
                        settings_content
                            .status_bar
                            .get_or_insert_default()
                            .active_language_button = value;
                    },
                }),
                metadata: None,
                files: USER,
            }),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_WINDOW_ACTIVE_ENCODING_BUTTON).into(),
                description: t(
                    keys::settings_ui::SETTINGS_WINDOW_CONTROL_WHEN_TO_SHOW_THE_ACTIVE_ENCODING,
                )
                .into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("status_bar.active_encoding_button"),
                    pick: |settings_content| {
                        settings_content
                            .status_bar
                            .as_ref()?
                            .active_encoding_button
                            .as_ref()
                    },
                    write: |settings_content, value, _| {
                        settings_content
                            .status_bar
                            .get_or_insert_default()
                            .active_encoding_button = value;
                    },
                }),
                metadata: None,
                files: USER,
            }),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_WINDOW_CURSOR_POSITION_BUTTON).into(),
                description: t(
                    keys::settings_ui::SETTINGS_WINDOW_SHOW_THE_CURSOR_POSITION_BUTTON_IN_THE_S,
                )
                .into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("status_bar.cursor_position_button"),
                    pick: |settings_content| {
                        settings_content
                            .status_bar
                            .as_ref()?
                            .cursor_position_button
                            .as_ref()
                    },
                    write: |settings_content, value, _| {
                        settings_content
                            .status_bar
                            .get_or_insert_default()
                            .cursor_position_button = value;
                    },
                }),
                metadata: None,
                files: USER,
            }),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_WINDOW_LINE_ENDINGS_BUTTON).into(),
                description: t(
                    keys::settings_ui::SETTINGS_WINDOW_SHOW_THE_ACTIVE_LINE_ENDINGS_BUTTON_IN_T,
                )
                .into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("status_bar.line_endings_button"),
                    pick: |settings_content| {
                        settings_content
                            .status_bar
                            .as_ref()?
                            .line_endings_button
                            .as_ref()
                    },
                    write: |settings_content, value, _| {
                        settings_content
                            .status_bar
                            .get_or_insert_default()
                            .line_endings_button = value;
                    },
                }),
                metadata: None,
                files: USER,
            }),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_WINDOW_PENDING_KEYSTROKES_INDICATOR).into(),
                description: t(
                    keys::settings_ui::SETTINGS_WINDOW_SHOW_AN_INDICATOR_WHILE_A_MULTI_STROKE_K,
                )
                .into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("status_bar.pending_keystrokes_indicator"),
                    pick: |settings_content| {
                        settings_content
                            .status_bar
                            .as_ref()?
                            .pending_keystrokes_indicator
                            .as_ref()
                    },
                    write: |settings_content, value, _| {
                        settings_content
                            .status_bar
                            .get_or_insert_default()
                            .pending_keystrokes_indicator = value;
                    },
                }),
                metadata: None,
                files: USER,
            }),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_WINDOW_TERMINAL_BUTTON).into(),
                description: t(
                    keys::settings_ui::SETTINGS_WINDOW_SHOW_THE_TERMINAL_BUTTON_IN_THE_STATUS_B,
                )
                .into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("terminal.button"),
                    pick: |settings_content| settings_content.terminal.as_ref()?.button.as_ref(),
                    write: |settings_content, value, _| {
                        settings_content.terminal.get_or_insert_default().button = value;
                    },
                }),
                metadata: None,
                files: USER,
            }),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_WINDOW_DIAGNOSTICS_BUTTON).into(),
                description: t(
                    keys::settings_ui::SETTINGS_WINDOW_SHOW_THE_PROJECT_DIAGNOSTICS_BUTTON_IN_T,
                )
                .into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("diagnostics.button"),
                    pick: |settings_content| settings_content.diagnostics.as_ref()?.button.as_ref(),
                    write: |settings_content, value, _| {
                        settings_content.diagnostics.get_or_insert_default().button = value;
                    },
                }),
                metadata: None,
                files: USER,
            }),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_WINDOW_PROJECT_SEARCH_BUTTON).into(),
                description: t(
                    keys::settings_ui::SETTINGS_WINDOW_SHOW_THE_PROJECT_SEARCH_BUTTON_IN_THE_ST,
                )
                .into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("search.button"),
                    pick: |settings_content| {
                        settings_content.editor.search.as_ref()?.button.as_ref()
                    },
                    write: |settings_content, value, _| {
                        settings_content
                            .editor
                            .search
                            .get_or_insert_default()
                            .button = value;
                    },
                }),
                metadata: None,
                files: USER,
            }),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_WINDOW_DEBUGGER_BUTTON).into(),
                description: t(
                    keys::settings_ui::SETTINGS_WINDOW_SHOW_THE_DEBUGGER_BUTTON_IN_THE_STATUS_B,
                )
                .into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("debugger.button"),
                    pick: |settings_content| settings_content.debugger.as_ref()?.button.as_ref(),
                    write: |settings_content, value, _| {
                        settings_content.debugger.get_or_insert_default().button = value;
                    },
                }),
                metadata: None,
                files: USER,
            }),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_WINDOW_ACTIVE_FILE_NAME).into(),
                description: t(
                    keys::settings_ui::SETTINGS_WINDOW_SHOW_THE_NAME_OF_THE_ACTIVE_FILE_IN_THE,
                )
                .into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("status_bar.show_active_file"),
                    pick: |settings_content| {
                        settings_content
                            .status_bar
                            .as_ref()?
                            .show_active_file
                            .as_ref()
                    },
                    write: |settings_content, value, _| {
                        settings_content
                            .status_bar
                            .get_or_insert_default()
                            .show_active_file = value;
                    },
                }),
                metadata: None,
                files: USER,
            }),
        ]
    }

    fn title_bar_section() -> [SettingsPageItem; 12] {
        [
            SettingsPageItem::SectionHeader(
                t(keys::settings_ui::SETTINGS_WINDOW_SECTION_TITLE_BAR).into(),
            ),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_WINDOW_SHOW_BRANCH_STATUS_ICON).into(),
                description: t(
                    keys::settings_ui::SETTINGS_WINDOW_SHOW_GIT_STATUS_INDICATORS_ON_THE_BRANCH,
                )
                .into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("title_bar.show_branch_status_icon"),
                    pick: |settings_content| {
                        settings_content
                            .title_bar
                            .as_ref()?
                            .show_branch_status_icon
                            .as_ref()
                    },
                    write: |settings_content, value, _| {
                        settings_content
                            .title_bar
                            .get_or_insert_default()
                            .show_branch_status_icon = value;
                    },
                }),
                metadata: None,
                files: USER,
            }),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_WINDOW_SHOW_BRANCH_NAME).into(),
                description: t(
                    keys::settings_ui::SETTINGS_WINDOW_SHOW_THE_BRANCH_NAME_BUTTON_IN_THE_TITLE,
                )
                .into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("title_bar.show_branch_name"),
                    pick: |settings_content| {
                        settings_content
                            .title_bar
                            .as_ref()?
                            .show_branch_name
                            .as_ref()
                    },
                    write: |settings_content, value, _| {
                        settings_content
                            .title_bar
                            .get_or_insert_default()
                            .show_branch_name = value;
                    },
                }),
                metadata: None,
                files: USER,
            }),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_WINDOW_SHOW_WORKTREE_NAME).into(),
                description: t(
                    keys::settings_ui::SETTINGS_WINDOW_SHOW_THE_WORKTREE_NAME_BUTTON_IN_THE_TIT,
                )
                .into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("title_bar.show_worktree_name"),
                    pick: |settings_content| {
                        settings_content
                            .title_bar
                            .as_ref()?
                            .show_worktree_name
                            .as_ref()
                    },
                    write: |settings_content, value, _| {
                        settings_content
                            .title_bar
                            .get_or_insert_default()
                            .show_worktree_name = value;
                    },
                }),
                metadata: None,
                files: USER,
            }),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_WINDOW_SHOW_PROJECT_ITEMS).into(),
                description: t(
                    keys::settings_ui::SETTINGS_WINDOW_SHOW_THE_PROJECT_HOST_AND_NAME_IN_THE_TI,
                )
                .into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("title_bar.show_project_items"),
                    pick: |settings_content| {
                        settings_content
                            .title_bar
                            .as_ref()?
                            .show_project_items
                            .as_ref()
                    },
                    write: |settings_content, value, _| {
                        settings_content
                            .title_bar
                            .get_or_insert_default()
                            .show_project_items = value;
                    },
                }),
                metadata: None,
                files: USER,
            }),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_WINDOW_SHOW_ONBOARDING_BANNER).into(),
                description: t(
                    keys::settings_ui::SETTINGS_WINDOW_SHOW_BANNERS_ANNOUNCING_NEW_FEATURES_IN,
                )
                .into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("title_bar.show_onboarding_banner"),
                    pick: |settings_content| {
                        settings_content
                            .title_bar
                            .as_ref()?
                            .show_onboarding_banner
                            .as_ref()
                    },
                    write: |settings_content, value, _| {
                        settings_content
                            .title_bar
                            .get_or_insert_default()
                            .show_onboarding_banner = value;
                    },
                }),
                metadata: None,
                files: USER,
            }),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_WINDOW_SHOW_SIGN_IN).into(),
                description: t(
                    keys::settings_ui::SETTINGS_WINDOW_SHOW_THE_SIGN_IN_BUTTON_IN_THE_TITLEBAR,
                )
                .into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("title_bar.show_sign_in"),
                    pick: |settings_content| {
                        settings_content.title_bar.as_ref()?.show_sign_in.as_ref()
                    },
                    write: |settings_content, value, _| {
                        settings_content
                            .title_bar
                            .get_or_insert_default()
                            .show_sign_in = value;
                    },
                }),
                metadata: None,
                files: USER,
            }),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_WINDOW_SHOW_USER_MENU).into(),
                description: t(
                    keys::settings_ui::SETTINGS_WINDOW_SHOW_THE_USER_MENU_BUTTON_IN_THE_TITLEBA,
                )
                .into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("title_bar.show_user_menu"),
                    pick: |settings_content| {
                        settings_content.title_bar.as_ref()?.show_user_menu.as_ref()
                    },
                    write: |settings_content, value, _| {
                        settings_content
                            .title_bar
                            .get_or_insert_default()
                            .show_user_menu = value;
                    },
                }),
                metadata: None,
                files: USER,
            }),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_WINDOW_SHOW_USER_PICTURE).into(),
                description: t(
                    keys::settings_ui::SETTINGS_WINDOW_SHOW_USER_PICTURE_IN_THE_TITLEBAR,
                )
                .into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("title_bar.show_user_picture"),
                    pick: |settings_content| {
                        settings_content
                            .title_bar
                            .as_ref()?
                            .show_user_picture
                            .as_ref()
                    },
                    write: |settings_content, value, _| {
                        settings_content
                            .title_bar
                            .get_or_insert_default()
                            .show_user_picture = value;
                    },
                }),
                metadata: None,
                files: USER,
            }),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_WINDOW_SHOW_MENUS).into(),
                description: t(keys::settings_ui::SETTINGS_WINDOW_SHOW_THE_MENUS_IN_THE_TITLEBAR)
                    .into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("title_bar.show_menus"),
                    pick: |settings_content| {
                        settings_content.title_bar.as_ref()?.show_menus.as_ref()
                    },
                    write: |settings_content, value, _| {
                        settings_content
                            .title_bar
                            .get_or_insert_default()
                            .show_menus = value;
                    },
                }),
                metadata: None,
                files: USER,
            }),
            SettingsPageItem::DynamicItem(DynamicItem {
                discriminant: SettingItem {
                    files: USER,
                    title: t(keys::settings_ui::SETTINGS_WINDOW_BUTTON_LAYOUT).into(),
                    description: t(keys::settings_ui::SETTINGS_TITLE_BAR_BUTTON_LAYOUT_HINT).into(),
                    field: Box::new(SettingField {
                        organization_override: None,
                        json_path: Some("title_bar.button_layout$"),
                        pick: |settings_content| {
                            Some(
                                &dynamic_variants::<settings::WindowButtonLayoutContent>()
                                    [settings_content
                                        .title_bar
                                        .as_ref()?
                                        .button_layout
                                        .as_ref()?
                                        .discriminant()
                                        as usize],
                            )
                        },
                        write: |settings_content, value, _| {
                            let Some(value) = value else {
                                settings_content
                                    .title_bar
                                    .get_or_insert_default()
                                    .button_layout = None;
                                return;
                            };

                            let current_custom_layout = settings_content
                                .title_bar
                                .as_ref()
                                .and_then(|title_bar| title_bar.button_layout.as_ref())
                                .and_then(|button_layout| match button_layout {
                                    settings::WindowButtonLayoutContent::Custom(layout) => {
                                        Some(layout.clone())
                                    }
                                    _ => None,
                                });

                            let button_layout = match value {
                                settings::WindowButtonLayoutContentDiscriminants::PlatformDefault => {
                                    settings::WindowButtonLayoutContent::PlatformDefault
                                }
                                settings::WindowButtonLayoutContentDiscriminants::Standard => {
                                    settings::WindowButtonLayoutContent::Standard
                                }
                                settings::WindowButtonLayoutContentDiscriminants::Custom => {
                                    settings::WindowButtonLayoutContent::Custom(
                                        current_custom_layout.unwrap_or_else(|| {
                                            "close:minimize,maximize".to_string()
                                        }),
                                    )
                                }
                            };

                            settings_content
                                .title_bar
                                .get_or_insert_default()
                                .button_layout = Some(button_layout);
                        },
                    }),
                    metadata: None,
                },
                pick_discriminant: |settings_content| {
                    Some(
                        settings_content
                            .title_bar
                            .as_ref()?
                            .button_layout
                            .as_ref()?
                            .discriminant() as usize,
                    )
                },
                fields: dynamic_variants::<settings::WindowButtonLayoutContent>()
                    .into_iter()
                    .map(|variant| match variant {
                        settings::WindowButtonLayoutContentDiscriminants::PlatformDefault => {
                            vec![]
                        }
                        settings::WindowButtonLayoutContentDiscriminants::Standard => vec![],
                        settings::WindowButtonLayoutContentDiscriminants::Custom => {
                            vec![SettingItem {
                                files: USER,
                                title: t(keys::settings_ui::SETTINGS_WINDOW_CUSTOM_BUTTON_LAYOUT)
                                    .into(),
                                description: t(
                                    keys::settings_ui::SETTINGS_TITLE_BAR_BUTTON_LAYOUT_EXAMPLE,
                                )
                                .into(),
                                field: Box::new(SettingField {
                                    organization_override: None,
                                    json_path: Some("title_bar.button_layout"),
                                    pick: |settings_content| match settings_content
                                        .title_bar
                                        .as_ref()?
                                        .button_layout
                                        .as_ref()?
                                    {
                                        settings::WindowButtonLayoutContent::Custom(layout) => {
                                            Some(layout)
                                        }
                                        _ => DEFAULT_EMPTY_STRING,
                                    },
                                    write: |settings_content, value, _| {
                                        settings_content
                                            .title_bar
                                            .get_or_insert_default()
                                            .button_layout =
                                            value.map(settings::WindowButtonLayoutContent::Custom);
                                    },
                                }),
                                metadata: Some(Box::new(SettingsFieldMetadata {
                                    placeholder: Some("close:minimize,maximize".into()),
                                    ..Default::default()
                                })),
                            }]
                        }
                    })
                    .collect(),
            }),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_WINDOW_OPEN_MENUS_ON_HOVER).into(),
                description: t(
                    keys::settings_ui::SETTINGS_WINDOW_AUTOMATICALLY_OPEN_MENUS_IN_THE_TITLEBAR,
                )
                .into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("title_bar.open_menus_on_hover"),
                    pick: |settings_content| {
                        settings_content
                            .title_bar
                            .as_ref()?
                            .open_menus_on_hover
                            .as_ref()
                    },
                    write: |settings_content, value, _| {
                        settings_content
                            .title_bar
                            .get_or_insert_default()
                            .open_menus_on_hover = value;
                    },
                }),
                metadata: None,
                files: USER,
            }),
        ]
    }

    fn tab_bar_section() -> [SettingsPageItem; 9] {
        [
            SettingsPageItem::SectionHeader(
                t(keys::settings_ui::SETTINGS_WINDOW_SECTION_TAB_BAR).into(),
            ),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_WINDOW_SHOW_TAB_BAR).into(),
                description: t(keys::settings_ui::SETTINGS_WINDOW_SHOW_THE_TAB_BAR_IN_THE_EDITOR)
                    .into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("tab_bar.show"),
                    pick: |settings_content| settings_content.tab_bar.as_ref()?.show.as_ref(),
                    write: |settings_content, value, _| {
                        settings_content.tab_bar.get_or_insert_default().show = value;
                    },
                }),
                metadata: None,
                files: USER,
            }),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_WINDOW_SHOW_GIT_STATUS_IN_TABS).into(),
                description: t(
                    keys::settings_ui::SETTINGS_WINDOW_SHOW_THE_GIT_FILE_STATUS_ON_A_TAB_ITEM,
                )
                .into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("tabs.git_status"),
                    pick: |settings_content| settings_content.tabs.as_ref()?.git_status.as_ref(),
                    write: |settings_content, value, _| {
                        settings_content.tabs.get_or_insert_default().git_status = value;
                    },
                }),
                metadata: None,
                files: USER,
            }),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_WINDOW_SHOW_FILE_ICONS_IN_TABS).into(),
                description: t(keys::settings_ui::SETTINGS_WINDOW_SHOW_THE_FILE_ICON_FOR_A_TAB)
                    .into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("tabs.file_icons"),
                    pick: |settings_content| settings_content.tabs.as_ref()?.file_icons.as_ref(),
                    write: |settings_content, value, _| {
                        settings_content.tabs.get_or_insert_default().file_icons = value;
                    },
                }),
                metadata: None,
                files: USER,
            }),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_WINDOW_TAB_CLOSE_POSITION).into(),
                description: t(
                    keys::settings_ui::SETTINGS_WINDOW_POSITION_OF_THE_CLOSE_BUTTON_IN_A_TAB,
                )
                .into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("tabs.close_position"),
                    pick: |settings_content| {
                        settings_content.tabs.as_ref()?.close_position.as_ref()
                    },
                    write: |settings_content, value, _| {
                        settings_content.tabs.get_or_insert_default().close_position = value;
                    },
                }),
                metadata: None,
                files: USER,
            }),
            SettingsPageItem::SettingItem(SettingItem {
                files: USER,
                title: t(keys::settings_ui::SETTINGS_WINDOW_MAXIMUM_TABS).into(),
                description: t(
                    keys::settings_ui::SETTINGS_WINDOW_MAXIMUM_OPEN_TABS_IN_A_PANE_WILL_NOT_CLO,
                )
                .into(),
                // todo(settings_ui): The default for this value is null and it's use in code
                // is complex, so I'm going to come back to this later
                field: Box::new(
                    SettingField {
                        organization_override: None,
                        json_path: Some("max_tabs"),
                        pick: |settings_content| settings_content.workspace.max_tabs.as_ref(),
                        write: |settings_content, value, _| {
                            settings_content.workspace.max_tabs = value;
                        },
                    }
                    .unimplemented(),
                ),
                metadata: None,
            }),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_WINDOW_SHOW_NAVIGATION_HISTORY_BUTTONS).into(),
                description: t(
                    keys::settings_ui::SETTINGS_WINDOW_SHOW_THE_NAVIGATION_HISTORY_BUTTONS_IN_T,
                )
                .into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("tab_bar.show_nav_history_buttons"),
                    pick: |settings_content| {
                        settings_content
                            .tab_bar
                            .as_ref()?
                            .show_nav_history_buttons
                            .as_ref()
                    },
                    write: |settings_content, value, _| {
                        settings_content
                            .tab_bar
                            .get_or_insert_default()
                            .show_nav_history_buttons = value;
                    },
                }),
                metadata: None,
                files: USER,
            }),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_WINDOW_SHOW_TAB_BAR_BUTTONS).into(),
                description: t(
                    keys::settings_ui::SETTINGS_WINDOW_SHOW_THE_TAB_BAR_BUTTONS_NEW_SPLIT_PANE,
                )
                .into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("tab_bar.show_tab_bar_buttons"),
                    pick: |settings_content| {
                        settings_content
                            .tab_bar
                            .as_ref()?
                            .show_tab_bar_buttons
                            .as_ref()
                    },
                    write: |settings_content, value, _| {
                        settings_content
                            .tab_bar
                            .get_or_insert_default()
                            .show_tab_bar_buttons = value;
                    },
                }),
                metadata: None,
                files: USER,
            }),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_WINDOW_PINNED_TABS_LAYOUT).into(),
                description: t(
                    keys::settings_ui::SETTINGS_WINDOW_SHOW_PINNED_TABS_IN_A_SEPARATE_ROW_ABOVE,
                )
                .into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("tab_bar.show_pinned_tabs_in_separate_row"),
                    pick: |settings_content| {
                        settings_content
                            .tab_bar
                            .as_ref()?
                            .show_pinned_tabs_in_separate_row
                            .as_ref()
                    },
                    write: |settings_content, value, _| {
                        settings_content
                            .tab_bar
                            .get_or_insert_default()
                            .show_pinned_tabs_in_separate_row = value;
                    },
                }),
                metadata: None,
                files: USER,
            }),
        ]
    }

    fn tab_settings_section() -> [SettingsPageItem; 4] {
        [
            SettingsPageItem::SectionHeader(
                t(keys::settings_ui::SETTINGS_WINDOW_SECTION_TAB_SETTINGS).into(),
            ),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_WINDOW_ACTIVATE_ON_CLOSE).into(),
                description: t(
                    keys::settings_ui::SETTINGS_WINDOW_WHAT_TO_DO_AFTER_CLOSING_THE_CURRENT_TAB,
                )
                .into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("tabs.activate_on_close"),
                    pick: |settings_content| {
                        settings_content.tabs.as_ref()?.activate_on_close.as_ref()
                    },
                    write: |settings_content, value, _| {
                        settings_content
                            .tabs
                            .get_or_insert_default()
                            .activate_on_close = value;
                    },
                }),
                metadata: None,
                files: USER,
            }),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_WINDOW_TAB_SHOW_DIAGNOSTICS).into(),
                description: t(keys::settings_ui::SETTINGS_TABS_DIAGNOSTICS_HINT).into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("tabs.show_diagnostics"),
                    pick: |settings_content| {
                        settings_content.tabs.as_ref()?.show_diagnostics.as_ref()
                    },
                    write: |settings_content, value, _| {
                        settings_content
                            .tabs
                            .get_or_insert_default()
                            .show_diagnostics = value;
                    },
                }),
                metadata: None,
                files: USER,
            }),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_WINDOW_SHOW_CLOSE_BUTTON).into(),
                description: t(
                    keys::settings_ui::SETTINGS_WINDOW_CONTROLS_THE_APPEARANCE_BEHAVIOR_OF_THE,
                )
                .into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("tabs.show_close_button"),
                    pick: |settings_content| {
                        settings_content.tabs.as_ref()?.show_close_button.as_ref()
                    },
                    write: |settings_content, value, _| {
                        settings_content
                            .tabs
                            .get_or_insert_default()
                            .show_close_button = value;
                    },
                }),
                metadata: None,
                files: USER,
            }),
        ]
    }

    fn preview_tabs_section() -> [SettingsPageItem; 8] {
        [
            SettingsPageItem::SectionHeader(
                t(keys::settings_ui::SETTINGS_WINDOW_SECTION_PREVIEW_TABS).into(),
            ),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_WINDOW_PREVIEW_TABS_ENABLED).into(),
                description: t(
                    keys::settings_ui::SETTINGS_WINDOW_SHOW_OPENED_EDITORS_AS_PREVIEW_TABS,
                )
                .into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("preview_tabs.enabled"),
                    pick: |settings_content| {
                        settings_content.preview_tabs.as_ref()?.enabled.as_ref()
                    },
                    write: |settings_content, value, _| {
                        settings_content
                            .preview_tabs
                            .get_or_insert_default()
                            .enabled = value;
                    },
                }),
                metadata: None,
                files: USER,
            }),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_WINDOW_ENABLE_PREVIEW_FROM_PROJECT_PANEL)
                    .into(),
                description: t(
                    keys::settings_ui::SETTINGS_WINDOW_WHETHER_TO_OPEN_TABS_IN_PREVIEW_MODE_WHE,
                )
                .into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("preview_tabs.enable_preview_from_project_panel"),
                    pick: |settings_content| {
                        settings_content
                            .preview_tabs
                            .as_ref()?
                            .enable_preview_from_project_panel
                            .as_ref()
                    },
                    write: |settings_content, value, _| {
                        settings_content
                            .preview_tabs
                            .get_or_insert_default()
                            .enable_preview_from_project_panel = value;
                    },
                }),
                metadata: None,
                files: USER,
            }),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_WINDOW_ENABLE_PREVIEW_FROM_FILE_FINDER).into(),
                description: t(
                    keys::settings_ui::SETTINGS_WINDOW_WHETHER_TO_OPEN_TABS_IN_PREVIEW_MODE_WHE_2,
                )
                .into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("preview_tabs.enable_preview_from_file_finder"),
                    pick: |settings_content| {
                        settings_content
                            .preview_tabs
                            .as_ref()?
                            .enable_preview_from_file_finder
                            .as_ref()
                    },
                    write: |settings_content, value, _| {
                        settings_content
                            .preview_tabs
                            .get_or_insert_default()
                            .enable_preview_from_file_finder = value;
                    },
                }),
                metadata: None,
                files: USER,
            }),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_WINDOW_ENABLE_PREVIEW_FROM_MULTIBUFFER).into(),
                description: t(
                    keys::settings_ui::SETTINGS_WINDOW_WHETHER_TO_OPEN_TABS_IN_PREVIEW_MODE_WHE_3,
                )
                .into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("preview_tabs.enable_preview_from_multibuffer"),
                    pick: |settings_content| {
                        settings_content
                            .preview_tabs
                            .as_ref()?
                            .enable_preview_from_multibuffer
                            .as_ref()
                    },
                    write: |settings_content, value, _| {
                        settings_content
                            .preview_tabs
                            .get_or_insert_default()
                            .enable_preview_from_multibuffer = value;
                    },
                }),
                metadata: None,
                files: USER,
            }),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(
                    keys::settings_ui::SETTINGS_WINDOW_ENABLE_PREVIEW_MULTIBUFFER_FROM_CODE_NAV,
                )
                .into(),
                description: t(
                    keys::settings_ui::SETTINGS_WINDOW_WHETHER_TO_OPEN_TABS_IN_PREVIEW_MODE_WHE_4,
                )
                .into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("preview_tabs.enable_preview_multibuffer_from_code_navigation"),
                    pick: |settings_content| {
                        settings_content
                            .preview_tabs
                            .as_ref()?
                            .enable_preview_multibuffer_from_code_navigation
                            .as_ref()
                    },
                    write: |settings_content, value, _| {
                        settings_content
                            .preview_tabs
                            .get_or_insert_default()
                            .enable_preview_multibuffer_from_code_navigation = value;
                    },
                }),
                metadata: None,
                files: USER,
            }),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(
                    keys::settings_ui::SETTINGS_WINDOW_ENABLE_PREVIEW_FILE_FROM_CODE_NAVIGATION,
                )
                .into(),
                description: t(
                    keys::settings_ui::SETTINGS_WINDOW_WHETHER_TO_OPEN_TABS_IN_PREVIEW_MODE_WHE_5,
                )
                .into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("preview_tabs.enable_preview_file_from_code_navigation"),
                    pick: |settings_content| {
                        settings_content
                            .preview_tabs
                            .as_ref()?
                            .enable_preview_file_from_code_navigation
                            .as_ref()
                    },
                    write: |settings_content, value, _| {
                        settings_content
                            .preview_tabs
                            .get_or_insert_default()
                            .enable_preview_file_from_code_navigation = value;
                    },
                }),
                metadata: None,
                files: USER,
            }),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_WINDOW_ENABLE_KEEP_PREVIEW_ON_CODE_NAVIGATION)
                    .into(),
                description: t(
                    keys::settings_ui::SETTINGS_WINDOW_WHETHER_TO_KEEP_TABS_IN_PREVIEW_MODE_WHE,
                )
                .into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("preview_tabs.enable_keep_preview_on_code_navigation"),
                    pick: |settings_content| {
                        settings_content
                            .preview_tabs
                            .as_ref()?
                            .enable_keep_preview_on_code_navigation
                            .as_ref()
                    },
                    write: |settings_content, value, _| {
                        settings_content
                            .preview_tabs
                            .get_or_insert_default()
                            .enable_keep_preview_on_code_navigation = value;
                    },
                }),
                metadata: None,
                files: USER,
            }),
        ]
    }

    fn layout_section() -> [SettingsPageItem; 6] {
        [
            SettingsPageItem::SectionHeader(
                t(keys::settings_ui::SETTINGS_WINDOW_SECTION_LAYOUT).into(),
            ),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_WINDOW_BOTTOM_DOCK_LAYOUT).into(),
                description: t(keys::settings_ui::SETTINGS_WINDOW_LAYOUT_MODE_FOR_THE_BOTTOM_DOCK)
                    .into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("bottom_dock_layout"),
                    pick: |settings_content| settings_content.workspace.bottom_dock_layout.as_ref(),
                    write: |settings_content, value, _| {
                        settings_content.workspace.bottom_dock_layout = value;
                    },
                }),
                metadata: None,
                files: USER,
            }),
            SettingsPageItem::SettingItem(SettingItem {
                files: USER,
                title: t(keys::settings_ui::SETTINGS_WINDOW_CENTERED_LAYOUT_LEFT_PADDING).into(),
                description: t(keys::settings_ui::SETTINGS_WINDOW_LEFT_PADDING_FOR_CENTERED_LAYOUT)
                    .into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("centered_layout.left_padding"),
                    pick: |settings_content| {
                        settings_content
                            .workspace
                            .centered_layout
                            .as_ref()?
                            .left_padding
                            .as_ref()
                    },
                    write: |settings_content, value, _| {
                        settings_content
                            .workspace
                            .centered_layout
                            .get_or_insert_default()
                            .left_padding = value;
                    },
                }),
                metadata: None,
            }),
            SettingsPageItem::SettingItem(SettingItem {
                files: USER,
                title: t(keys::settings_ui::SETTINGS_WINDOW_CENTERED_LAYOUT_RIGHT_PADDING).into(),
                description: t(
                    keys::settings_ui::SETTINGS_WINDOW_RIGHT_PADDING_FOR_CENTERED_LAYOUT,
                )
                .into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("centered_layout.right_padding"),
                    pick: |settings_content| {
                        settings_content
                            .workspace
                            .centered_layout
                            .as_ref()?
                            .right_padding
                            .as_ref()
                    },
                    write: |settings_content, value, _| {
                        settings_content
                            .workspace
                            .centered_layout
                            .get_or_insert_default()
                            .right_padding = value;
                    },
                }),
                metadata: None,
            }),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_WINDOW_FOCUS_FOLLOWS_MOUSE).into(),
                description: t(
                    keys::settings_ui::SETTINGS_WINDOW_WHETHER_TO_CHANGE_FOCUS_TO_A_PANE_WHEN_T,
                )
                .into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("focus_follows_mouse.enabled"),
                    pick: |settings_content| {
                        settings_content
                            .workspace
                            .focus_follows_mouse
                            .as_ref()
                            .and_then(|s| s.enabled.as_ref())
                    },
                    write: |settings_content, value, _| {
                        settings_content
                            .workspace
                            .focus_follows_mouse
                            .get_or_insert_default()
                            .enabled = value;
                    },
                }),
                metadata: None,
                files: USER,
            }),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_WINDOW_FOCUS_FOLLOWS_MOUSE_DEBOUNCE_MS).into(),
                description: t(
                    keys::settings_ui::SETTINGS_WINDOW_AMOUNT_OF_TIME_TO_WAIT_BEFORE_CHANGING_F,
                )
                .into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("focus_follows_mouse.debounce_ms"),
                    pick: |settings_content| {
                        settings_content
                            .workspace
                            .focus_follows_mouse
                            .as_ref()
                            .and_then(|s| s.debounce_ms.as_ref())
                    },
                    write: |settings_content, value, _| {
                        settings_content
                            .workspace
                            .focus_follows_mouse
                            .get_or_insert_default()
                            .debounce_ms = value;
                    },
                }),
                metadata: None,
                files: USER,
            }),
        ]
    }

    fn window_section() -> [SettingsPageItem; 6] {
        [
            SettingsPageItem::SectionHeader(t(keys::app_menu::MENU_WINDOW_TITLE).into()),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_WINDOW_TITLE_FORMAT).into(),
                description: "Window title template. Available variables are `${projectName}`, `${fileName}`, `${filePath}`, `${relativePath}`, `${fileStem}`, `${remoteName}`, `${remoteHost}`, `${appName}`, `${branch}`, and `${separator}`. `${separator}` is omitted when adjacent variables are empty, but literal text is preserved. The collaboration indicator, when present, is appended after the rendered template. If the template renders to nothing, the default template is used instead.".into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("window_title_format"),
                    pick: |settings_content| {
                        settings_content.workspace.window_title_format.as_ref()
                    },
                    write: |settings_content, value, _| {
                        settings_content.workspace.window_title_format =
                            value.filter(|format| !format.is_empty());
                    },
                }),
                metadata: Some(Box::new(SettingsFieldMetadata {
                    placeholder: Some("${projectName}${separator}${fileName}".into()),
                    ..Default::default()
                })),
                files: USER,
            }),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_WINDOW_TITLE_SEPARATOR).into(),
                description: "String substituted for `${separator}` in the window title format. Include any surrounding whitespace in the value.".into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("window_title_separator"),
                    pick: |settings_content| {
                        settings_content.workspace.window_title_separator.as_ref()
                    },
                    write: |settings_content, value, _| {
                        settings_content.workspace.window_title_separator = value;
                    },
                }),
                metadata: Some(Box::new(SettingsFieldMetadata {
                    placeholder: Some(" — ".into()),
                    ..Default::default()
                })),
                files: USER,
            }),
            // todo(settings_ui): Should we filter by platform.as_ref()?
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_WINDOW_USE_SYSTEM_WINDOW_TABS).into(),
                description: t(keys::settings_ui::SETTINGS_WINDOW_MACOS_ONLY_WHETHER_TO_ALLOW_WINDOWS_TO_T).into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("use_system_window_tabs"),
                    pick: |settings_content| {
                        settings_content.workspace.use_system_window_tabs.as_ref()
                    },
                    write: |settings_content, value, _| {
                        settings_content.workspace.use_system_window_tabs = value;
                    },
                }),
                metadata: None,
                files: USER,
            }),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_WINDOW_FULLSCREEN_MODE).into(),
                description: t(keys::settings_ui::SETTINGS_WINDOW_MACOS_ONLY_WHICH_FULLSCREEN_MODE_THE_TOG).into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("fullscreen_mode"),
                    pick: |settings_content| settings_content.workspace.fullscreen_mode.as_ref(),
                    write: |settings_content, value, _| {
                        settings_content.workspace.fullscreen_mode = value;
                    },
                }),
                metadata: None,
                files: USER,
            }),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_WINDOW_WINDOW_DECORATIONS).into(),
                description: t(keys::settings_ui::SETTINGS_WINDOW_LINUX_ONLY_WHETHER_ZED_OR_YOUR_COMPOSITO).into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("window_decorations"),
                    pick: |settings_content| settings_content.workspace.window_decorations.as_ref(),
                    write: |settings_content, value, _| {
                        settings_content.workspace.window_decorations = value;
                    },
                }),
                metadata: None,
                files: USER,
            }),
        ]
    }

    fn pane_modifiers_section() -> [SettingsPageItem; 5] {
        [
            SettingsPageItem::SectionHeader(
                t(keys::settings_ui::SETTINGS_WINDOW_SECTION_PANE_MODIFIERS).into(),
            ),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_WINDOW_INACTIVE_OPACITY).into(),
                description: t(
                    keys::settings_ui::SETTINGS_WINDOW_OPACITY_OF_INACTIVE_PANELS_0_0_1_0,
                )
                .into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("active_pane_modifiers.inactive_opacity"),
                    pick: |settings_content| {
                        settings_content
                            .workspace
                            .active_pane_modifiers
                            .as_ref()?
                            .inactive_opacity
                            .as_ref()
                    },
                    write: |settings_content, value, _| {
                        settings_content
                            .workspace
                            .active_pane_modifiers
                            .get_or_insert_default()
                            .inactive_opacity = value;
                    },
                }),
                metadata: None,
                files: USER,
            }),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_WINDOW_BORDER_SIZE).into(),
                description: t(
                    keys::settings_ui::SETTINGS_WINDOW_SIZE_OF_THE_BORDER_SURROUNDING_THE_ACTIV,
                )
                .into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("active_pane_modifiers.border_size"),
                    pick: |settings_content| {
                        settings_content
                            .workspace
                            .active_pane_modifiers
                            .as_ref()?
                            .border_size
                            .as_ref()
                    },
                    write: |settings_content, value, _| {
                        settings_content
                            .workspace
                            .active_pane_modifiers
                            .get_or_insert_default()
                            .border_size = value;
                    },
                }),
                metadata: None,
                files: USER,
            }),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_WINDOW_ZOOMED_PADDING).into(),
                description: t(keys::settings_ui::SETTINGS_WINDOW_SHOW_PADDING_FOR_ZOOMED_PANES)
                    .into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("zoomed_padding"),
                    pick: |settings_content| settings_content.workspace.zoomed_padding.as_ref(),
                    write: |settings_content, value, _| {
                        settings_content.workspace.zoomed_padding = value;
                    },
                }),
                metadata: None,
                files: USER,
            }),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_WINDOW_CLOSE_PANEL_ON_TOGGLE).into(),
                description: t(
                    keys::settings_ui::SETTINGS_WINDOW_WHETHER_INVOKING_A_PANEL_S_TOGGLEFOCUS_A,
                )
                .into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("close_panel_on_toggle"),
                    pick: |settings_content| {
                        settings_content.workspace.close_panel_on_toggle.as_ref()
                    },
                    write: |settings_content, value, _| {
                        settings_content.workspace.close_panel_on_toggle = value;
                    },
                }),
                metadata: None,
                files: USER,
            }),
        ]
    }

    fn pane_split_direction_section() -> [SettingsPageItem; 3] {
        [
            SettingsPageItem::SectionHeader(
                t(keys::settings_ui::SETTINGS_WINDOW_SECTION_PANE_SPLIT_DIRECTION).into(),
            ),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_WINDOW_VERTICAL_SPLIT_DIRECTION).into(),
                description: t(keys::settings_ui::SETTINGS_WINDOW_DIRECTION_TO_SPLIT_VERTICALLY)
                    .into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("pane_split_direction_vertical"),
                    pick: |settings_content| {
                        settings_content
                            .workspace
                            .pane_split_direction_vertical
                            .as_ref()
                    },
                    write: |settings_content, value, _| {
                        settings_content.workspace.pane_split_direction_vertical = value;
                    },
                }),
                metadata: None,
                files: USER,
            }),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_WINDOW_HORIZONTAL_SPLIT_DIRECTION).into(),
                description: t(keys::settings_ui::SETTINGS_WINDOW_DIRECTION_TO_SPLIT_HORIZONTALLY)
                    .into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("pane_split_direction_horizontal"),
                    pick: |settings_content| {
                        settings_content
                            .workspace
                            .pane_split_direction_horizontal
                            .as_ref()
                    },
                    write: |settings_content, value, _| {
                        settings_content.workspace.pane_split_direction_horizontal = value;
                    },
                }),
                metadata: None,
                files: USER,
            }),
        ]
    }

    SettingsPage {
        title: t(keys::settings_ui::SETTINGS_WINDOW_WINDOW_LAYOUT).into(),
        items: concat_sections![
            status_bar_section(),
            title_bar_section(),
            tab_bar_section(),
            tab_settings_section(),
            preview_tabs_section(),
            layout_section(),
            window_section(),
            pane_modifiers_section(),
            pane_split_direction_section(),
        ],
    }
}

fn panels_page() -> SettingsPage {
    fn project_panel_section() -> [SettingsPageItem; 30] {
        [
            SettingsPageItem::SectionHeader(t(keys::app_menu::MENU_VIEW_PROJECT_PANEL).into()),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_PANELS_PROJECT_PANEL_DOCK).into(),
                description: t(keys::settings_ui::SETTINGS_PANELS_WHERE_TO_DOCK_THE_PROJECT_PANEL).into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("project_panel.dock"),
                    pick: |settings_content| settings_content.project_panel.as_ref()?.dock.as_ref(),
                    write: |settings_content, value, _| {
                        settings_content.project_panel.get_or_insert_default().dock = value;
                    },
                }),
                metadata: None,
                files: USER,
            }),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_PANELS_PROJECT_PANEL_DEFAULT_WIDTH).into(),
                description: t(keys::settings_ui::SETTINGS_PANELS_DEFAULT_WIDTH_OF_THE_PROJECT_PANEL_IN_PI).into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("project_panel.default_width"),
                    pick: |settings_content| {
                        settings_content
                            .project_panel
                            .as_ref()?
                            .default_width
                            .as_ref()
                    },
                    write: |settings_content, value, _| {
                        settings_content
                            .project_panel
                            .get_or_insert_default()
                            .default_width = value;
                    },
                }),
                metadata: None,
                files: USER,
            }),
            SettingsPageItem::DynamicItem(DynamicItem {
                discriminant: SettingItem {
                    title: t(keys::settings_ui::SETTINGS_PANELS_PROJECT_PANEL_TITLE_TOOLTIPS_DELAY).into(),
                    description: t(keys::settings_ui::SETTINGS_PANELS_DELAY_IN_MILLISECONDS_BEFORE_TOOLTIPS_AP).into(),
                    field: Box::new(SettingField {
                        organization_override: None,
                        json_path: Some("project_panel.title_tooltip_delay$"),
                        pick: |settings_content| {
                            Some(
                                &dynamic_variants::<settings::ProjectPanelTitleTooltipDelay>()
                                    [settings_content
                                        .project_panel
                                        .as_ref()?
                                        .title_tooltip_delay
                                        .as_ref()?
                                        .discriminant()
                                        as usize],
                            )
                        },
                        write: |settings_content, value, _| {
                            let project_panel =
                                settings_content.project_panel.get_or_insert_default();
                            project_panel.title_tooltip_delay = value.map(|value| match value {
                                settings::ProjectPanelTitleTooltipDelayDiscriminants::Default => {
                                    settings::ProjectPanelTitleTooltipDelay::Default
                                }
                                settings::ProjectPanelTitleTooltipDelayDiscriminants::Disabled => {
                                    settings::ProjectPanelTitleTooltipDelay::Disabled
                                }
                                settings::ProjectPanelTitleTooltipDelayDiscriminants::Custom => {
                                    let delay = match project_panel.title_tooltip_delay {
                                        Some(settings::ProjectPanelTitleTooltipDelay::Custom(
                                            delay,
                                        )) => settings::DelayMs(delay.0),
                                        _ => settings::DelayMs(1500),
                                    };
                                    settings::ProjectPanelTitleTooltipDelay::Custom(delay)
                                }
                            });
                        },
                    }),
                    metadata: None,
                    files: USER,
                },
                pick_discriminant: |settings_content| {
                    Some(
                        settings_content
                            .project_panel
                            .as_ref()?
                            .title_tooltip_delay
                            .as_ref()?
                            .discriminant() as usize,
                    )
                },
                fields: dynamic_variants::<settings::ProjectPanelTitleTooltipDelay>()
                    .into_iter()
                    .map(|variant| match variant {
                        settings::ProjectPanelTitleTooltipDelayDiscriminants::Default => vec![],
                        settings::ProjectPanelTitleTooltipDelayDiscriminants::Disabled => vec![],
                        settings::ProjectPanelTitleTooltipDelayDiscriminants::Custom => {
                            vec![SettingItem {
                                files: USER,
                                title: t(keys::settings_ui::SETTINGS_PANELS_CUSTOM_DELAY).into(),
                                description: t(keys::settings_ui::SETTINGS_PANELS_DELAY_IN_MILLISECONDS_OF_THE_PROJECT_PAN).into(),
                                field: Box::new(SettingField {
                                    organization_override: None,
                                    json_path: Some("project_panel.title_tooltip_delay"),
                                    pick: |settings_content| match settings_content
                                        .project_panel
                                        .as_ref()
                                        .and_then(|project_panel| {
                                            project_panel.title_tooltip_delay.as_ref()
                                        }) {
                                        Some(settings::ProjectPanelTitleTooltipDelay::Custom(
                                            value,
                                        )) => Some(value),
                                        _ => None,
                                    },
                                    write: |settings_content, value, _| {
                                        let Some(value) = value else {
                                            return;
                                        };
                                        if let Some(
                                            settings::ProjectPanelTitleTooltipDelay::Custom(width),
                                        ) = settings_content.project_panel.as_mut().and_then(
                                            |project_panel| {
                                                project_panel.title_tooltip_delay.as_mut()
                                            },
                                        ) {
                                            *width = value;
                                        }
                                    },
                                }),
                                metadata: None,
                            }]
                        }
                    })
                    .collect(),
            }),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_PANELS_HIDE_GITIGNORE).into(),
                description: t(keys::settings_ui::SETTINGS_PANELS_WHETHER_TO_HIDE_THE_GITIGNORE_ENTRIES_IN).into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("project_panel.hide_gitignore"),
                    pick: |settings_content| {
                        settings_content
                            .project_panel
                            .as_ref()?
                            .hide_gitignore
                            .as_ref()
                    },
                    write: |settings_content, value, _| {
                        settings_content
                            .project_panel
                            .get_or_insert_default()
                            .hide_gitignore = value;
                    },
                }),
                metadata: None,
                files: USER,
            }),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_PANELS_ENTRY_SPACING).into(),
                description: t(keys::settings_ui::SETTINGS_PANELS_SPACING_BETWEEN_WORKTREE_ENTRIES_IN_THE).into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("project_panel.entry_spacing"),
                    pick: |settings_content| {
                        settings_content
                            .project_panel
                            .as_ref()?
                            .entry_spacing
                            .as_ref()
                    },
                    write: |settings_content, value, _| {
                        settings_content
                            .project_panel
                            .get_or_insert_default()
                            .entry_spacing = value;
                    },
                }),
                metadata: None,
                files: USER,
            }),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_SEARCH_FILE_ICONS).into(),
                description: t(keys::settings_ui::SETTINGS_PANELS_SHOW_FILE_ICONS_IN_THE_PROJECT_PANEL).into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("project_panel.file_icons"),
                    pick: |settings_content| {
                        settings_content.project_panel.as_ref()?.file_icons.as_ref()
                    },
                    write: |settings_content, value, _| {
                        settings_content
                            .project_panel
                            .get_or_insert_default()
                            .file_icons = value;
                    },
                }),
                metadata: None,
                files: USER,
            }),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_PANELS_FOLDER_INDICATOR).into(),
                description: t(keys::settings_ui::SETTINGS_PANELS_WHAT_TO_SHOW_FOR_DIRECTORIES_IN_THE_PROJ).into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("project_panel.folder_indicator"),
                    pick: |settings_content| {
                        settings_content
                            .project_panel
                            .as_ref()?
                            .folder_indicator
                            .as_ref()
                    },
                    write: |settings_content, value, _| {
                        settings_content
                            .project_panel
                            .get_or_insert_default()
                            .folder_indicator = value;
                    },
                }),
                metadata: None,
                files: USER,
            }),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_PANELS_GIT_STATUS).into(),
                description: t(keys::settings_ui::SETTINGS_PANELS_SHOW_THE_GIT_STATUS_IN_THE_PROJECT_PANEL).into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("project_panel.git_status"),
                    pick: |settings_content| {
                        settings_content.project_panel.as_ref()?.git_status.as_ref()
                    },
                    write: |settings_content, value, _| {
                        settings_content
                            .project_panel
                            .get_or_insert_default()
                            .git_status = value;
                    },
                }),
                metadata: None,
                files: USER,
            }),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_PANELS_INDENT_SIZE).into(),
                description: t(keys::settings_ui::SETTINGS_PANELS_AMOUNT_OF_INDENTATION_FOR_NESTED_ITEMS).into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("project_panel.indent_size"),
                    pick: |settings_content| {
                        settings_content
                            .project_panel
                            .as_ref()?
                            .indent_size
                            .as_ref()
                    },
                    write: |settings_content, value, _| {
                        settings_content
                            .project_panel
                            .get_or_insert_default()
                            .indent_size = value;
                    },
                }),
                metadata: None,
                files: USER,
            }),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_PANELS_AUTO_REVEAL_ENTRIES).into(),
                description: t(keys::settings_ui::SETTINGS_PANELS_WHETHER_TO_REVEAL_ENTRIES_IN_THE_PROJECT).into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("project_panel.auto_reveal_entries"),
                    pick: |settings_content| {
                        settings_content
                            .project_panel
                            .as_ref()?
                            .auto_reveal_entries
                            .as_ref()
                    },
                    write: |settings_content, value, _| {
                        settings_content
                            .project_panel
                            .get_or_insert_default()
                            .auto_reveal_entries = value;
                    },
                }),
                metadata: None,
                files: USER,
            }),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_PANELS_STARTS_OPEN).into(),
                description: t(keys::settings_ui::SETTINGS_PANELS_WHETHER_THE_PROJECT_PANEL_SHOULD_OPEN_ON).into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("project_panel.starts_open"),
                    pick: |settings_content| {
                        settings_content
                            .project_panel
                            .as_ref()?
                            .starts_open
                            .as_ref()
                    },
                    write: |settings_content, value, _| {
                        settings_content
                            .project_panel
                            .get_or_insert_default()
                            .starts_open = value;
                    },
                }),
                metadata: None,
                files: USER,
            }),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_PANELS_AUTO_FOLD_DIRECTORIES).into(),
                description: t(keys::settings_ui::SETTINGS_PANELS_WHETHER_TO_FOLD_DIRECTORIES_AUTOMATICALL).into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("project_panel.auto_fold_dirs"),
                    pick: |settings_content| {
                        settings_content
                            .project_panel
                            .as_ref()?
                            .auto_fold_dirs
                            .as_ref()
                    },
                    write: |settings_content, value, _| {
                        settings_content
                            .project_panel
                            .get_or_insert_default()
                            .auto_fold_dirs = value;
                    },
                }),
                metadata: None,
                files: USER,
            }),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_PANELS_BOLD_FOLDER_LABELS).into(),
                description: t(keys::settings_ui::SETTINGS_PANELS_WHETHER_TO_SHOW_FOLDER_NAMES_WITH_BOLD_T).into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("project_panel.bold_folder_labels"),
                    pick: |settings_content| {
                        settings_content
                            .project_panel
                            .as_ref()?
                            .bold_folder_labels
                            .as_ref()
                    },
                    write: |settings_content, value, _| {
                        settings_content
                            .project_panel
                            .get_or_insert_default()
                            .bold_folder_labels = value;
                    },
                }),
                metadata: None,
                files: USER,
            }),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_PANELS_SHOW_SCROLLBAR).into(),
                description: t(keys::settings_ui::SETTINGS_PANELS_SHOW_THE_SCROLLBAR_IN_THE_PROJECT_PANEL).into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("project_panel.scrollbar.show"),
                    pick: |settings_content| {
                        show_scrollbar_or_editor(settings_content, |settings_content| {
                            settings_content
                                .project_panel
                                .as_ref()?
                                .scrollbar
                                .as_ref()?
                                .show
                                .as_ref()
                        })
                    },
                    write: |settings_content, value, _| {
                        settings_content
                            .project_panel
                            .get_or_insert_default()
                            .scrollbar
                            .get_or_insert_default()
                            .show = value;
                    },
                }),
                metadata: None,
                files: USER,
            }),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_PANELS_HORIZONTAL_SCROLL).into(),
                description: t(keys::settings_ui::SETTINGS_PANELS_WHETHER_TO_ALLOW_HORIZONTAL_SCROLLING_IN).into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("project_panel.scrollbar.horizontal_scroll"),
                    pick: |settings_content| {
                        settings_content
                            .project_panel
                            .as_ref()?
                            .scrollbar
                            .as_ref()?
                            .horizontal_scroll
                            .as_ref()
                    },
                    write: |settings_content, value, _| {
                        settings_content
                            .project_panel
                            .get_or_insert_default()
                            .scrollbar
                            .get_or_insert_default()
                            .horizontal_scroll = value;
                    },
                }),
                metadata: None,
                files: USER,
            }),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_PANELS_SHOW_DIAGNOSTICS).into(),
                description: t(keys::settings_ui::SETTINGS_PANELS_WHICH_FILES_CONTAINING_DIAGNOSTIC_ERRORS).into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("project_panel.show_diagnostics"),
                    pick: |settings_content| {
                        settings_content
                            .project_panel
                            .as_ref()?
                            .show_diagnostics
                            .as_ref()
                    },
                    write: |settings_content, value, _| {
                        settings_content
                            .project_panel
                            .get_or_insert_default()
                            .show_diagnostics = value;
                    },
                }),
                metadata: None,
                files: USER,
            }),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_PANELS_DIAGNOSTIC_BADGES).into(),
                description: t(keys::settings_ui::SETTINGS_PANELS_SHOW_ERROR_AND_WARNING_COUNT_BADGES_NEXT).into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("project_panel.diagnostic_badges"),
                    pick: |settings_content| {
                        settings_content
                            .project_panel
                            .as_ref()?
                            .diagnostic_badges
                            .as_ref()
                    },
                    write: |settings_content, value, _| {
                        settings_content
                            .project_panel
                            .get_or_insert_default()
                            .diagnostic_badges = value;
                    },
                }),
                metadata: None,
                files: USER,
            }),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_PANELS_GIT_STATUS_INDICATOR).into(),
                description: t(keys::settings_ui::SETTINGS_PANELS_SHOW_A_GIT_STATUS_INDICATOR_NEXT_TO_FILE).into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("project_panel.git_status_indicator"),
                    pick: |settings_content| {
                        settings_content
                            .project_panel
                            .as_ref()?
                            .git_status_indicator
                            .as_ref()
                    },
                    write: |settings_content, value, _| {
                        settings_content
                            .project_panel
                            .get_or_insert_default()
                            .git_status_indicator = value;
                    },
                }),
                metadata: None,
                files: USER,
            }),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_EDITOR_STICKY_SCROLL).into(),
                description: t(keys::settings_ui::SETTINGS_PANELS_WHETHER_TO_STICK_PARENT_DIRECTORIES_AT_T).into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("project_panel.sticky_scroll"),
                    pick: |settings_content| {
                        settings_content
                            .project_panel
                            .as_ref()?
                            .sticky_scroll
                            .as_ref()
                    },
                    write: |settings_content, value, _| {
                        settings_content
                            .project_panel
                            .get_or_insert_default()
                            .sticky_scroll = value;
                    },
                }),
                metadata: None,
                files: USER,
            }),
            SettingsPageItem::SettingItem(SettingItem {
                files: USER,
                title: t(keys::settings_ui::SETTINGS_PANELS_SHOW_INDENT_GUIDES).into(),
                description: t(keys::settings_ui::SETTINGS_PANELS_SHOW_INDENT_GUIDES_IN_THE_PROJECT_PANEL).into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("project_panel.indent_guides.show"),
                    pick: |settings_content| {
                        settings_content
                            .project_panel
                            .as_ref()?
                            .indent_guides
                            .as_ref()?
                            .show
                            .as_ref()
                    },
                    write: |settings_content, value, _| {
                        settings_content
                            .project_panel
                            .get_or_insert_default()
                            .indent_guides
                            .get_or_insert_default()
                            .show = value;
                    },
                }),
                metadata: None,
            }),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_PANELS_DRAG_AND_DROP).into(),
                description: t(keys::settings_ui::SETTINGS_PANELS_WHETHER_TO_ENABLE_DRAG_AND_DROP_OPERATIO).into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("project_panel.drag_and_drop"),
                    pick: |settings_content| {
                        settings_content
                            .project_panel
                            .as_ref()?
                            .drag_and_drop
                            .as_ref()
                    },
                    write: |settings_content, value, _| {
                        settings_content
                            .project_panel
                            .get_or_insert_default()
                            .drag_and_drop = value;
                    },
                }),
                metadata: None,
                files: USER,
            }),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_PANELS_HIDE_ROOT).into(),
                description: t(keys::settings_ui::SETTINGS_PANELS_WHETHER_TO_HIDE_THE_ROOT_ENTRY_WHEN_ONLY).into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("project_panel.hide_root"),
                    pick: |settings_content| {
                        settings_content.project_panel.as_ref()?.hide_root.as_ref()
                    },
                    write: |settings_content, value, _| {
                        settings_content
                            .project_panel
                            .get_or_insert_default()
                            .hide_root = value;
                    },
                }),
                metadata: None,
                files: USER,
            }),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_PANELS_HIDE_HIDDEN).into(),
                description: t(keys::settings_ui::SETTINGS_PANELS_WHETHER_TO_HIDE_THE_HIDDEN_ENTRIES_IN_TH).into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("project_panel.hide_hidden"),
                    pick: |settings_content| {
                        settings_content
                            .project_panel
                            .as_ref()?
                            .hide_hidden
                            .as_ref()
                    },
                    write: |settings_content, value, _| {
                        settings_content
                            .project_panel
                            .get_or_insert_default()
                            .hide_hidden = value;
                    },
                }),
                metadata: None,
                files: USER,
            }),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_PANELS_SORT_MODE).into(),
                description: t(keys::settings_ui::SETTINGS_PANELS_SORT_ORDER_FOR_ENTRIES_IN_THE_PROJECT_PA).into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("project_panel.sort_mode"),
                    pick: |settings_content| {
                        settings_content.project_panel.as_ref()?.sort_mode.as_ref()
                    },
                    write: |settings_content, value, _| {
                        settings_content
                            .project_panel
                            .get_or_insert_default()
                            .sort_mode = value;
                    },
                }),
                metadata: None,
                files: USER,
            }),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_PANELS_SORT_ORDER).into(),
                description: t(keys::settings_ui::SETTINGS_PANELS_WHETHER_TO_SORT_FILE_AND_FOLDER_NAMES_CA).into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    pick: |settings_content| {
                        settings_content.project_panel.as_ref()?.sort_order.as_ref()
                    },
                    write: |settings_content, value, _| {
                        settings_content
                            .project_panel
                            .get_or_insert_default()
                            .sort_order = value;
                    },
                    json_path: Some("project_panel.sort_order"),
                }),
                metadata: None,
                files: USER,
            }),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_PANELS_AUTO_OPEN_FILES_ON_CREATE).into(),
                description: t(keys::settings_ui::SETTINGS_PANELS_WHETHER_TO_AUTOMATICALLY_OPEN_NEWLY_CREA).into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("project_panel.auto_open.on_create"),
                    pick: |settings_content| {
                        settings_content
                            .project_panel
                            .as_ref()?
                            .auto_open
                            .as_ref()?
                            .on_create
                            .as_ref()
                    },
                    write: |settings_content, value, _| {
                        settings_content
                            .project_panel
                            .get_or_insert_default()
                            .auto_open
                            .get_or_insert_default()
                            .on_create = value;
                    },
                }),
                metadata: None,
                files: USER,
            }),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_PANELS_AUTO_OPEN_FILES_ON_PASTE).into(),
                description: t(keys::settings_ui::SETTINGS_PANELS_WHETHER_TO_AUTOMATICALLY_OPEN_FILES_AFTE).into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("project_panel.auto_open.on_paste"),
                    pick: |settings_content| {
                        settings_content
                            .project_panel
                            .as_ref()?
                            .auto_open
                            .as_ref()?
                            .on_paste
                            .as_ref()
                    },
                    write: |settings_content, value, _| {
                        settings_content
                            .project_panel
                            .get_or_insert_default()
                            .auto_open
                            .get_or_insert_default()
                            .on_paste = value;
                    },
                }),
                metadata: None,
                files: USER,
            }),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_PANELS_AUTO_OPEN_FILES_ON_DROP).into(),
                description: t(keys::settings_ui::SETTINGS_PANELS_WHETHER_TO_AUTOMATICALLY_OPEN_FILES_DROP).into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("project_panel.auto_open.on_drop"),
                    pick: |settings_content| {
                        settings_content
                            .project_panel
                            .as_ref()?
                            .auto_open
                            .as_ref()?
                            .on_drop
                            .as_ref()
                    },
                    write: |settings_content, value, _| {
                        settings_content
                            .project_panel
                            .get_or_insert_default()
                            .auto_open
                            .get_or_insert_default()
                            .on_drop = value;
                    },
                }),
                metadata: None,
                files: USER,
            }),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_PANELS_HIDDEN_FILES).into(),
                description: "Globs to match files that will be considered \"hidden\" and can be hidden from the project panel.".into(),
                field: Box::new(
                    SettingField {
                        organization_override: None,
                        json_path: Some("hidden_files"),
                        pick: |settings_content| {
                            settings_content.project.worktree.hidden_files.as_ref()
                        },
                        write: |settings_content, value, _| {
                            settings_content.project.worktree.hidden_files = value;
                        },
                    }
                    .unimplemented(),
                ),
                metadata: None,
                files: USER,
            }),
        ]
    }

    fn terminal_panel_section() -> [SettingsPageItem; 5] {
        [
            SettingsPageItem::SectionHeader(t(keys::app_menu::MENU_VIEW_TERMINAL_PANEL).into()),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_PANELS_TERMINAL_DOCK).into(),
                description: t(keys::settings_ui::SETTINGS_PANELS_WHERE_TO_DOCK_THE_TERMINAL_PANEL)
                    .into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("terminal.dock"),
                    pick: |settings_content| settings_content.terminal.as_ref()?.dock.as_ref(),
                    write: |settings_content, value, _| {
                        settings_content.terminal.get_or_insert_default().dock = value;
                    },
                }),
                metadata: None,
                files: USER,
            }),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_PANELS_STARTS_OPEN).into(),
                description: t(
                    keys::settings_ui::SETTINGS_PANELS_WHETHER_THE_TERMINAL_PANEL_SHOULD_OPEN_O,
                )
                .into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("terminal.starts_open"),
                    pick: |settings_content| {
                        settings_content.terminal.as_ref()?.starts_open.as_ref()
                    },
                    write: |settings_content, value, _| {
                        settings_content
                            .terminal
                            .get_or_insert_default()
                            .starts_open = value;
                    },
                }),
                metadata: None,
                files: USER,
            }),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_PANELS_TERMINAL_PANEL_FLEXIBLE_SIZING).into(),
                description: t(
                    keys::settings_ui::SETTINGS_PANELS_WHETHER_THE_TERMINAL_PANEL_SHOULD_USE_FL,
                )
                .into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("terminal.flexible"),
                    pick: |settings_content| settings_content.terminal.as_ref()?.flexible.as_ref(),
                    write: |settings_content, value, _| {
                        settings_content.terminal.get_or_insert_default().flexible = value;
                    },
                }),
                metadata: None,
                files: USER,
            }),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_PANELS_SHOW_COUNT_BADGE).into(),
                description: t(
                    keys::settings_ui::SETTINGS_PANELS_SHOW_A_BADGE_ON_THE_TERMINAL_PANEL_ICON,
                )
                .into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("terminal.show_count_badge"),
                    pick: |settings_content| {
                        settings_content
                            .terminal
                            .as_ref()?
                            .show_count_badge
                            .as_ref()
                    },
                    write: |settings_content, value, _| {
                        settings_content
                            .terminal
                            .get_or_insert_default()
                            .show_count_badge = value;
                    },
                }),
                metadata: None,
                files: USER,
            }),
        ]
    }

    fn outline_panel_section() -> [SettingsPageItem; 12] {
        [
            SettingsPageItem::SectionHeader(t(keys::app_menu::MENU_VIEW_OUTLINE_PANEL).into()),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_PANELS_OUTLINE_PANEL_BUTTON).into(),
                description: t(
                    keys::settings_ui::SETTINGS_PANELS_SHOW_THE_OUTLINE_PANEL_BUTTON_IN_THE_STA,
                )
                .into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("outline_panel.button"),
                    pick: |settings_content| {
                        settings_content.outline_panel.as_ref()?.button.as_ref()
                    },
                    write: |settings_content, value, _| {
                        settings_content
                            .outline_panel
                            .get_or_insert_default()
                            .button = value;
                    },
                }),
                metadata: None,
                files: USER,
            }),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_PANELS_OUTLINE_PANEL_DOCK).into(),
                description: t(keys::settings_ui::SETTINGS_PANELS_WHERE_TO_DOCK_THE_OUTLINE_PANEL)
                    .into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("outline_panel.dock"),
                    pick: |settings_content| settings_content.outline_panel.as_ref()?.dock.as_ref(),
                    write: |settings_content, value, _| {
                        settings_content.outline_panel.get_or_insert_default().dock = value;
                    },
                }),
                metadata: None,
                files: USER,
            }),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_PANELS_OUTLINE_PANEL_DEFAULT_WIDTH).into(),
                description: t(
                    keys::settings_ui::SETTINGS_PANELS_DEFAULT_WIDTH_OF_THE_OUTLINE_PANEL_IN_PI,
                )
                .into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("outline_panel.default_width"),
                    pick: |settings_content| {
                        settings_content
                            .outline_panel
                            .as_ref()?
                            .default_width
                            .as_ref()
                    },
                    write: |settings_content, value, _| {
                        settings_content
                            .outline_panel
                            .get_or_insert_default()
                            .default_width = value;
                    },
                }),
                metadata: None,
                files: USER,
            }),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_SEARCH_FILE_ICONS).into(),
                description: t(
                    keys::settings_ui::SETTINGS_PANELS_SHOW_FILE_ICONS_IN_THE_OUTLINE_PANEL,
                )
                .into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("outline_panel.file_icons"),
                    pick: |settings_content| {
                        settings_content.outline_panel.as_ref()?.file_icons.as_ref()
                    },
                    write: |settings_content, value, _| {
                        settings_content
                            .outline_panel
                            .get_or_insert_default()
                            .file_icons = value;
                    },
                }),
                metadata: None,
                files: USER,
            }),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_PANELS_FOLDER_INDICATOR).into(),
                description: t(
                    keys::settings_ui::SETTINGS_PANELS_WHAT_TO_SHOW_FOR_DIRECTORIES_IN_THE_OUTL,
                )
                .into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("outline_panel.folder_indicator"),
                    pick: |settings_content| {
                        settings_content
                            .outline_panel
                            .as_ref()?
                            .folder_indicator
                            .as_ref()
                    },
                    write: |settings_content, value, _| {
                        settings_content
                            .outline_panel
                            .get_or_insert_default()
                            .folder_indicator = value;
                    },
                }),
                metadata: None,
                files: USER,
            }),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_PANELS_GIT_STATUS).into(),
                description: t(
                    keys::settings_ui::SETTINGS_PANELS_SHOW_THE_GIT_STATUS_IN_THE_OUTLINE_PANEL,
                )
                .into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("outline_panel.git_status"),
                    pick: |settings_content| {
                        settings_content.outline_panel.as_ref()?.git_status.as_ref()
                    },
                    write: |settings_content, value, _| {
                        settings_content
                            .outline_panel
                            .get_or_insert_default()
                            .git_status = value;
                    },
                }),
                metadata: None,
                files: USER,
            }),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_PANELS_INDENT_SIZE).into(),
                description: t(
                    keys::settings_ui::SETTINGS_PANELS_AMOUNT_OF_INDENTATION_FOR_NESTED_ITEMS,
                )
                .into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("outline_panel.indent_size"),
                    pick: |settings_content| {
                        settings_content
                            .outline_panel
                            .as_ref()?
                            .indent_size
                            .as_ref()
                    },
                    write: |settings_content, value, _| {
                        settings_content
                            .outline_panel
                            .get_or_insert_default()
                            .indent_size = value;
                    },
                }),
                metadata: None,
                files: USER,
            }),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_PANELS_AUTO_REVEAL_ENTRIES).into(),
                description: t(
                    keys::settings_ui::SETTINGS_PANELS_WHETHER_TO_REVEAL_WHEN_A_CORRESPONDING_O,
                )
                .into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("outline_panel.auto_reveal_entries"),
                    pick: |settings_content| {
                        settings_content
                            .outline_panel
                            .as_ref()?
                            .auto_reveal_entries
                            .as_ref()
                    },
                    write: |settings_content, value, _| {
                        settings_content
                            .outline_panel
                            .get_or_insert_default()
                            .auto_reveal_entries = value;
                    },
                }),
                metadata: None,
                files: USER,
            }),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_PANELS_AUTO_FOLD_DIRECTORIES).into(),
                description: t(
                    keys::settings_ui::SETTINGS_PANELS_WHETHER_TO_FOLD_DIRECTORIES_AUTOMATICALL_2,
                )
                .into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("outline_panel.auto_fold_dirs"),
                    pick: |settings_content| {
                        settings_content
                            .outline_panel
                            .as_ref()?
                            .auto_fold_dirs
                            .as_ref()
                    },
                    write: |settings_content, value, _| {
                        settings_content
                            .outline_panel
                            .get_or_insert_default()
                            .auto_fold_dirs = value;
                    },
                }),
                metadata: None,
                files: USER,
            }),
            SettingsPageItem::SettingItem(SettingItem {
                files: USER,
                title: t(keys::settings_ui::SETTINGS_PANELS_SHOW_INDENT_GUIDES).into(),
                description: t(
                    keys::settings_ui::SETTINGS_PANELS_WHEN_TO_SHOW_INDENT_GUIDES_IN_THE_OUTLIN,
                )
                .into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("outline_panel.indent_guides.show"),
                    pick: |settings_content| {
                        settings_content
                            .outline_panel
                            .as_ref()?
                            .indent_guides
                            .as_ref()?
                            .show
                            .as_ref()
                    },
                    write: |settings_content, value, _| {
                        settings_content
                            .outline_panel
                            .get_or_insert_default()
                            .indent_guides
                            .get_or_insert_default()
                            .show = value;
                    },
                }),
                metadata: None,
            }),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_PANELS_HIDE_SYMBOLS_IN_MULTI_BUFFERS).into(),
                description: t(
                    keys::settings_ui::SETTINGS_PANELS_WHETHER_TO_HIDE_SYMBOLS_EXCERPTS_AND_SEA,
                )
                .into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("outline_panel.multi_buffer_hide_symbols"),
                    pick: |settings_content| {
                        settings_content
                            .outline_panel
                            .as_ref()?
                            .multi_buffer_hide_symbols
                            .as_ref()
                    },
                    write: |settings_content, value, _| {
                        settings_content
                            .outline_panel
                            .get_or_insert_default()
                            .multi_buffer_hide_symbols = value;
                    },
                }),
                metadata: None,
                files: USER,
            }),
        ]
    }

    fn git_panel_section() -> [SettingsPageItem; 19] {
        [
            SettingsPageItem::SectionHeader(t(keys::app_menu::MENU_VIEW_GIT_PANEL).into()),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_PANELS_GIT_PANEL_BUTTON).into(),
                description: t(
                    keys::settings_ui::SETTINGS_PANELS_SHOW_THE_GIT_PANEL_BUTTON_IN_THE_STATUS,
                )
                .into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("git_panel.button"),
                    pick: |settings_content| settings_content.git_panel.as_ref()?.button.as_ref(),
                    write: |settings_content, value, _| {
                        settings_content.git_panel.get_or_insert_default().button = value;
                    },
                }),
                metadata: None,
                files: USER,
            }),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_PANELS_GIT_PANEL_DOCK).into(),
                description: t(keys::settings_ui::SETTINGS_PANELS_WHERE_TO_DOCK_THE_GIT_PANEL)
                    .into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("git_panel.dock"),
                    pick: |settings_content| settings_content.git_panel.as_ref()?.dock.as_ref(),
                    write: |settings_content, value, _| {
                        settings_content.git_panel.get_or_insert_default().dock = value;
                    },
                }),
                metadata: None,
                files: USER,
            }),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_PANELS_STARTS_OPEN).into(),
                description: t(
                    keys::settings_ui::SETTINGS_PANELS_WHETHER_THE_GIT_PANEL_SHOULD_OPEN_ON_STA,
                )
                .into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("git_panel.starts_open"),
                    pick: |settings_content| {
                        settings_content.git_panel.as_ref()?.starts_open.as_ref()
                    },
                    write: |settings_content, value, _| {
                        settings_content
                            .git_panel
                            .get_or_insert_default()
                            .starts_open = value;
                    },
                }),
                metadata: None,
                files: USER,
            }),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_PANELS_GIT_PANEL_DEFAULT_WIDTH).into(),
                description: t(
                    keys::settings_ui::SETTINGS_PANELS_DEFAULT_WIDTH_OF_THE_GIT_PANEL_IN_PIXELS,
                )
                .into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("git_panel.default_width"),
                    pick: |settings_content| {
                        settings_content.git_panel.as_ref()?.default_width.as_ref()
                    },
                    write: |settings_content, value, _| {
                        settings_content
                            .git_panel
                            .get_or_insert_default()
                            .default_width = value;
                    },
                }),
                metadata: None,
                files: USER,
            }),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_PANELS_GIT_PANEL_STATUS_STYLE).into(),
                description: t(keys::settings_ui::SETTINGS_PANELS_HOW_ENTRY_STATUSES_ARE_DISPLAYED)
                    .into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("git_panel.status_style"),
                    pick: |settings_content| {
                        settings_content.git_panel.as_ref()?.status_style.as_ref()
                    },
                    write: |settings_content, value, _| {
                        settings_content
                            .git_panel
                            .get_or_insert_default()
                            .status_style = value;
                    },
                }),
                metadata: None,
                files: USER,
            }),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_PANELS_FALLBACK_BRANCH_NAME).into(),
                description: t(
                    keys::settings_ui::SETTINGS_PANELS_DEFAULT_BRANCH_NAME_WILL_BE_WHEN_INIT_DE,
                )
                .into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("git_panel.fallback_branch_name"),
                    pick: |settings_content| {
                        settings_content
                            .git_panel
                            .as_ref()?
                            .fallback_branch_name
                            .as_ref()
                    },
                    write: |settings_content, value, _| {
                        settings_content
                            .git_panel
                            .get_or_insert_default()
                            .fallback_branch_name = value;
                    },
                }),
                metadata: None,
                files: USER,
            }),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::git_ui::GIT_SORT_BY).into(),
                description: t(
                    keys::settings_ui::SETTINGS_PANELS_HOW_TO_SORT_ENTRIES_IN_THE_GIT_PANEL,
                )
                .into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("git_panel.sort_by"),
                    pick: |settings_content| settings_content.git_panel.as_ref()?.sort_by.as_ref(),
                    write: |settings_content, value, _| {
                        settings_content.git_panel.get_or_insert_default().sort_by = value;
                    },
                }),
                metadata: None,
                files: USER,
            }),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::git_ui::GIT_GROUP_BY).into(),
                description: t(
                    keys::settings_ui::SETTINGS_PANELS_HOW_TO_GROUP_ENTRIES_IN_THE_GIT_PANEL,
                )
                .into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("git_panel.group_by"),
                    pick: |settings_content| settings_content.git_panel.as_ref()?.group_by.as_ref(),
                    write: |settings_content, value, _| {
                        settings_content.git_panel.get_or_insert_default().group_by = value;
                    },
                }),
                metadata: None,
                files: USER,
            }),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_PANELS_COLLAPSE_UNTRACKED_DIFF).into(),
                description: t(
                    keys::settings_ui::SETTINGS_PANELS_WHETHER_TO_COLLAPSE_UNTRACKED_FILES_IN_T,
                )
                .into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("git_panel.collapse_untracked_diff"),
                    pick: |settings_content| {
                        settings_content
                            .git_panel
                            .as_ref()?
                            .collapse_untracked_diff
                            .as_ref()
                    },
                    write: |settings_content, value, _| {
                        settings_content
                            .git_panel
                            .get_or_insert_default()
                            .collapse_untracked_diff = value;
                    },
                }),
                metadata: None,
                files: USER,
            }),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_PANELS_TREE_VIEW).into(),
                description: t(
                    keys::settings_ui::SETTINGS_PANELS_ENABLE_TO_SHOW_ENTRIES_IN_TREE_VIEW_LIST,
                )
                .into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("git_panel.tree_view"),
                    pick: |settings_content| {
                        settings_content.git_panel.as_ref()?.tree_view.as_ref()
                    },
                    write: |settings_content, value, _| {
                        settings_content.git_panel.get_or_insert_default().tree_view = value;
                    },
                }),
                metadata: None,
                files: USER,
            }),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_SEARCH_FILE_ICONS).into(),
                description: t(
                    keys::settings_ui::SETTINGS_PANELS_SHOW_FILE_ICONS_NEXT_TO_THE_GIT_STATUS_I,
                )
                .into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("git_panel.file_icons"),
                    pick: |settings_content| {
                        settings_content.git_panel.as_ref()?.file_icons.as_ref()
                    },
                    write: |settings_content, value, _| {
                        settings_content
                            .git_panel
                            .get_or_insert_default()
                            .file_icons = value;
                    },
                }),
                metadata: None,
                files: USER,
            }),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_PANELS_FOLDER_INDICATOR).into(),
                description: t(
                    keys::settings_ui::SETTINGS_PANELS_WHAT_TO_SHOW_FOR_DIRECTORIES_IN_THE_GIT,
                )
                .into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("git_panel.folder_indicator"),
                    pick: |settings_content| {
                        settings_content
                            .git_panel
                            .as_ref()?
                            .folder_indicator
                            .as_ref()
                    },
                    write: |settings_content, value, _| {
                        settings_content
                            .git_panel
                            .get_or_insert_default()
                            .folder_indicator = value;
                    },
                }),
                metadata: None,
                files: USER,
            }),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_PANELS_DIFF_STATS).into(),
                description: t(
                    keys::settings_ui::SETTINGS_PANELS_WHETHER_TO_SHOW_THE_ADDITION_DELETION_CH,
                )
                .into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("git_panel.diff_stats"),
                    pick: |settings_content| {
                        settings_content.git_panel.as_ref()?.diff_stats.as_ref()
                    },
                    write: |settings_content, value, _| {
                        settings_content
                            .git_panel
                            .get_or_insert_default()
                            .diff_stats = value;
                    },
                }),
                metadata: None,
                files: USER,
            }),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_PANELS_PRIMARY_CLICK_BEHAVIOR).into(),
                description: t(
                    keys::settings_ui::SETTINGS_PANELS_DEFAULT_ACTION_WHEN_CLICKING_A_CHANGED_F,
                )
                .into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("git_panel.entry_primary_click_action"),
                    pick: |settings_content| {
                        settings_content
                            .git_panel
                            .as_ref()?
                            .entry_primary_click_action
                            .as_ref()
                    },
                    write: |settings_content, value, _| {
                        settings_content
                            .git_panel
                            .get_or_insert_default()
                            .entry_primary_click_action = value;
                    },
                }),
                metadata: None,
                files: USER,
            }),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_PANELS_COMMIT_EDITOR).into(),
                description: t(
                    keys::settings_ui::SETTINGS_PANELS_WHETHER_THE_COMMIT_MESSAGE_EDITOR_IS_SHO,
                )
                .into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("git_panel.commit_editor"),
                    pick: |settings_content| {
                        settings_content.git_panel.as_ref()?.commit_editor.as_ref()
                    },
                    write: |settings_content, value, _| {
                        settings_content
                            .git_panel
                            .get_or_insert_default()
                            .commit_editor = value;
                    },
                }),
                metadata: None,
                files: USER,
            }),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_PANELS_SHOW_COUNT_BADGE).into(),
                description: t(
                    keys::settings_ui::SETTINGS_PANELS_WHETHER_TO_SHOW_A_BADGE_ON_THE_GIT_PANEL,
                )
                .into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("git_panel.show_count_badge"),
                    pick: |settings_content| {
                        settings_content
                            .git_panel
                            .as_ref()?
                            .show_count_badge
                            .as_ref()
                    },
                    write: |settings_content, value, _| {
                        settings_content
                            .git_panel
                            .get_or_insert_default()
                            .show_count_badge = value;
                    },
                }),
                metadata: None,
                files: USER,
            }),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_PANELS_COMMIT_TITLE_MAX_LENGTH).into(),
                description: t(
                    keys::settings_ui::SETTINGS_PANELS_MAXIMUM_LENGTH_OF_THE_COMMIT_MESSAGE_TIT,
                )
                .into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("git_panel.commit_title_max_length"),
                    pick: |settings_content| {
                        settings_content
                            .git_panel
                            .as_ref()?
                            .commit_title_max_length
                            .as_ref()
                    },
                    write: |settings_content, value, _| {
                        settings_content
                            .git_panel
                            .get_or_insert_default()
                            .commit_title_max_length = value;
                    },
                }),
                metadata: None,
                files: USER,
            }),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_PANELS_SCROLL_BAR).into(),
                description: t(
                    keys::settings_ui::SETTINGS_PANELS_HOW_AND_WHEN_THE_SCROLLBAR_SHOULD_BE_DIS,
                )
                .into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("git_panel.scrollbar.show"),
                    pick: |settings_content| {
                        show_scrollbar_or_editor(settings_content, |settings_content| {
                            settings_content
                                .git_panel
                                .as_ref()?
                                .scrollbar
                                .as_ref()?
                                .show
                                .as_ref()
                        })
                    },
                    write: |settings_content, value, _| {
                        settings_content
                            .git_panel
                            .get_or_insert_default()
                            .scrollbar
                            .get_or_insert_default()
                            .show = value;
                    },
                }),
                metadata: None,
                files: USER,
            }),
        ]
    }

    fn debugger_panel_section() -> [SettingsPageItem; 2] {
        [
            SettingsPageItem::SectionHeader(t(keys::app_menu::MENU_VIEW_DEBUGGER_PANEL).into()),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_PANELS_DEBUGGER_PANEL_DOCK).into(),
                description: t(
                    keys::settings_ui::SETTINGS_PANELS_THE_DOCK_POSITION_OF_THE_DEBUG_PANEL,
                )
                .into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("debugger.dock"),
                    pick: |settings_content| settings_content.debugger.as_ref()?.dock.as_ref(),
                    write: |settings_content, value, _| {
                        settings_content.debugger.get_or_insert_default().dock = value;
                    },
                }),
                metadata: None,
                files: USER,
            }),
        ]
    }

    fn collaboration_panel_section() -> [SettingsPageItem; 4] {
        [
            SettingsPageItem::SectionHeader(
                t(keys::settings_ui::SETTINGS_PANELS_SECTION_COLLABORATION_PANEL).into(),
            ),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_PANELS_COLLABORATION_PANEL_BUTTON).into(),
                description: t(
                    keys::settings_ui::SETTINGS_PANELS_SHOW_THE_COLLABORATION_PANEL_BUTTON_IN_T,
                )
                .into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("collaboration_panel.button"),
                    pick: |settings_content| {
                        settings_content
                            .collaboration_panel
                            .as_ref()?
                            .button
                            .as_ref()
                    },
                    write: |settings_content, value, _| {
                        settings_content
                            .collaboration_panel
                            .get_or_insert_default()
                            .button = value;
                    },
                }),
                metadata: None,
                files: USER,
            }),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_PANELS_COLLABORATION_PANEL_DOCK).into(),
                description: t(
                    keys::settings_ui::SETTINGS_PANELS_WHERE_TO_DOCK_THE_COLLABORATION_PANEL,
                )
                .into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("collaboration_panel.dock"),
                    pick: |settings_content| {
                        settings_content.collaboration_panel.as_ref()?.dock.as_ref()
                    },
                    write: |settings_content, value, _| {
                        settings_content
                            .collaboration_panel
                            .get_or_insert_default()
                            .dock = value;
                    },
                }),
                metadata: None,
                files: USER,
            }),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_PANELS_COLLABORATION_PANEL_DEFAULT_WIDTH)
                    .into(),
                description: t(
                    keys::settings_ui::SETTINGS_PANELS_DEFAULT_WIDTH_OF_THE_COLLABORATION_PANEL,
                )
                .into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("collaboration_panel.dock"),
                    pick: |settings_content| {
                        settings_content
                            .collaboration_panel
                            .as_ref()?
                            .default_width
                            .as_ref()
                    },
                    write: |settings_content, value, _| {
                        settings_content
                            .collaboration_panel
                            .get_or_insert_default()
                            .default_width = value;
                    },
                }),
                metadata: None,
                files: USER,
            }),
        ]
    }

    fn agent_panel_section() -> [SettingsPageItem; 7] {
        [
            SettingsPageItem::SectionHeader(t(keys::agent::AGENT_PANEL_TITLE).into()),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_PANELS_AGENT_PANEL_BUTTON).into(),
                description: t(keys::settings_ui::SETTINGS_PANELS_WHETHER_TO_SHOW_THE_AGENT_PANEL_BUTTON_I).into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("agent.button"),
                    pick: |settings_content| settings_content.agent.as_ref()?.button.as_ref(),
                    write: |settings_content, value, _| {
                        settings_content.agent.get_or_insert_default().button = value;
                    },
                }),
                metadata: None,
                files: USER,
            }),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_PANELS_AGENT_PANEL_DOCK).into(),
                description: t(keys::settings_ui::SETTINGS_PANELS_WHERE_TO_DOCK_THE_AGENT_PANEL).into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("agent.dock"),
                    pick: |settings_content| settings_content.agent.as_ref()?.dock.as_ref(),
                    write: |settings_content, value, _| {
                        settings_content.agent.get_or_insert_default().dock = value;
                    },
                }),
                metadata: None,
                files: USER,
            }),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_PANELS_AGENT_PANEL_FLEXIBLE_SIZING).into(),
                description: t(keys::settings_ui::SETTINGS_PANELS_WHETHER_THE_AGENT_PANEL_SHOULD_USE_FLEXI).into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("agent.flexible"),
                    pick: |settings_content| settings_content.agent.as_ref()?.flexible.as_ref(),
                    write: |settings_content, value, _| {
                        settings_content.agent.get_or_insert_default().flexible = value;
                    },
                }),
                metadata: None,
                files: USER,
            }),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_PANELS_AGENT_PANEL_DEFAULT_WIDTH).into(),
                description: t(keys::settings_ui::SETTINGS_PANELS_DEFAULT_FIXED_WIDTH_WHEN_THE_AGENT_PANEL).into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("agent.default_width"),
                    pick: |settings_content| {
                        settings_content.agent.as_ref()?.default_width.as_ref()
                    },
                    write: |settings_content, value, _| {
                        settings_content.agent.get_or_insert_default().default_width = value;
                    },
                }),
                metadata: None,
                files: USER,
            }),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_PANELS_AGENT_PANEL_DEFAULT_HEIGHT).into(),
                description: t(keys::settings_ui::SETTINGS_PANELS_DEFAULT_HEIGHT_WHEN_THE_AGENT_PANEL_IS_D).into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("agent.default_height"),
                    pick: |settings_content| {
                        settings_content.agent.as_ref()?.default_height.as_ref()
                    },
                    write: |settings_content, value, _| {
                        settings_content
                            .agent
                            .get_or_insert_default()
                            .default_height = value;
                    },
                }),
                metadata: None,
                files: USER,
            }),
            SettingsPageItem::DynamicItem(DynamicItem {
                discriminant: SettingItem {
                    files: USER,
                    title: t(keys::settings_ui::SETTINGS_PANELS_LIMIT_CONTENT_WIDTH).into(),
                    description: t(keys::settings_ui::SETTINGS_PANELS_WHETHER_TO_CONSTRAIN_THE_AGENT_PANEL_CON).into(),
                    field: Box::new(SettingField::<bool> {
                        organization_override: None,
                        json_path: Some("agent.limit_content_width"),
                        pick: |settings_content| {
                            settings_content
                                .agent
                                .as_ref()?
                                .limit_content_width
                                .as_ref()
                        },
                        write: |settings_content, value, _| {
                            settings_content
                                .agent
                                .get_or_insert_default()
                                .limit_content_width = value;
                        },
                    }),
                    metadata: None,
                },
                pick_discriminant: |settings_content| {
                    let enabled = settings_content
                        .agent
                        .as_ref()?
                        .limit_content_width
                        .unwrap_or(true);
                    Some(if enabled { 1 } else { 0 })
                },
                fields: vec![
                    vec![],
                    vec![SettingItem {
                        files: USER,
                        title: t(keys::settings_ui::SETTINGS_PANELS_MAX_CONTENT_WIDTH).into(),
                        description: t(keys::settings_ui::SETTINGS_PANELS_MAXIMUM_CONTENT_WIDTH_IN_PIXELS_CONTENT).into(),
                        field: Box::new(SettingField {
                            organization_override: None,
                            json_path: Some("agent.max_content_width"),
                            pick: |settings_content| {
                                settings_content.agent.as_ref()?.max_content_width.as_ref()
                            },
                            write: |settings_content, value, _| {
                                settings_content
                                    .agent
                                    .get_or_insert_default()
                                    .max_content_width = value;
                            },
                        }),
                        metadata: None,
                    }],
                ],
            }),
        ]
    }

    SettingsPage {
        title: t(keys::settings_ui::SETTINGS_PANELS_PANELS).into(),
        items: concat_sections![
            project_panel_section(),
            terminal_panel_section(),
            outline_panel_section(),
            git_panel_section(),
            debugger_panel_section(),
            collaboration_panel_section(),
            agent_panel_section(),
        ],
    }
}

fn debugger_page() -> SettingsPage {
    fn general_section() -> [SettingsPageItem; 6] {
        [
            SettingsPageItem::SectionHeader(t(keys::settings_ui::SETTINGS_GENERAL_GENERAL).into()),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_DEBUGGER_STEPPING_GRANULARITY).into(),
                description: t(
                    keys::settings_ui::SETTINGS_DEBUGGER_DETERMINES_THE_STEPPING_GRANULARITY_FOR,
                )
                .into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("debugger.stepping_granularity"),
                    pick: |settings_content| {
                        settings_content
                            .debugger
                            .as_ref()?
                            .stepping_granularity
                            .as_ref()
                    },
                    write: |settings_content, value, _| {
                        settings_content
                            .debugger
                            .get_or_insert_default()
                            .stepping_granularity = value;
                    },
                }),
                metadata: None,
                files: USER,
            }),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_DEBUGGER_SAVE_BREAKPOINTS).into(),
                description: t(
                    keys::settings_ui::SETTINGS_DEBUGGER_WHETHER_BREAKPOINTS_SHOULD_BE_REUSED_ACR,
                )
                .into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("debugger.save_breakpoints"),
                    pick: |settings_content| {
                        settings_content
                            .debugger
                            .as_ref()?
                            .save_breakpoints
                            .as_ref()
                    },
                    write: |settings_content, value, _| {
                        settings_content
                            .debugger
                            .get_or_insert_default()
                            .save_breakpoints = value;
                    },
                }),
                metadata: None,
                files: USER,
            }),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_DEBUGGER_TIMEOUT).into(),
                description: t(
                    keys::settings_ui::SETTINGS_DEBUGGER_TIME_IN_MILLISECONDS_UNTIL_TIMEOUT_ERROR,
                )
                .into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("debugger.timeout"),
                    pick: |settings_content| settings_content.debugger.as_ref()?.timeout.as_ref(),
                    write: |settings_content, value, _| {
                        settings_content.debugger.get_or_insert_default().timeout = value;
                    },
                }),
                metadata: None,
                files: USER,
            }),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_DEBUGGER_LOG_DAP_COMMUNICATIONS).into(),
                description: t(
                    keys::settings_ui::SETTINGS_DEBUGGER_WHETHER_TO_LOG_MESSAGES_BETWEEN_ACTIVE_D,
                )
                .into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("debugger.log_dap_communications"),
                    pick: |settings_content| {
                        settings_content
                            .debugger
                            .as_ref()?
                            .log_dap_communications
                            .as_ref()
                    },
                    write: |settings_content, value, _| {
                        settings_content
                            .debugger
                            .get_or_insert_default()
                            .log_dap_communications = value;
                    },
                }),
                metadata: None,
                files: USER,
            }),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_DEBUGGER_FORMAT_DAP_LOG_MESSAGES).into(),
                description: t(
                    keys::settings_ui::SETTINGS_DEBUGGER_WHETHER_TO_FORMAT_DAP_MESSAGES_WHEN_ADDI,
                )
                .into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("debugger.format_dap_log_messages"),
                    pick: |settings_content| {
                        settings_content
                            .debugger
                            .as_ref()?
                            .format_dap_log_messages
                            .as_ref()
                    },
                    write: |settings_content, value, _| {
                        settings_content
                            .debugger
                            .get_or_insert_default()
                            .format_dap_log_messages = value;
                    },
                }),
                metadata: None,
                files: USER,
            }),
        ]
    }

    SettingsPage {
        title: t(keys::settings_ui::SETTINGS_DEBUGGER_DEBUGGER).into(),
        items: concat_sections![general_section()],
    }
}

fn terminal_page() -> SettingsPage {
    fn environment_section() -> [SettingsPageItem; 5] {
        [
                SettingsPageItem::SectionHeader(t(keys::settings_ui::SETTINGS_TERMINAL_SECTION_ENVIRONMENT).into()),
                SettingsPageItem::DynamicItem(DynamicItem {
                    discriminant: SettingItem {
                        files: USER | PROJECT,
                        title: "Shell".into(),
                        description: t(keys::settings_ui::SETTINGS_TERMINAL_WHAT_SHELL_TO_USE_WHEN_OPENING_A_TERMINA).into(),
                        field: Box::new(SettingField {
                            organization_override: None,
                            json_path: Some("terminal.shell$"),
                            pick: |settings_content| {
                                Some(&dynamic_variants::<settings::Shell>()[
                                    settings_content
                                        .terminal
                                        .as_ref()?
                                        .project
                                        .shell
                                        .as_ref()?
                                        .discriminant() as usize
                                ])
                            },
                            write: |settings_content, value, _| {
                                let Some(value) = value else {
                                    if let Some(terminal) = settings_content.terminal.as_mut() {
                                        terminal.project.shell = None;
                                    }
                                    return;
                                };
                                let settings_value = settings_content
                                    .terminal
                                    .get_or_insert_default()
                                    .project
                                    .shell
                                    .get_or_insert_with(|| settings::Shell::default());
                                let default_shell = if cfg!(target_os = "windows") {
                                    "powershell.exe"
                                } else {
                                    "sh"
                                };
                                *settings_value = match value {
                                    settings::ShellDiscriminants::System => settings::Shell::System,
                                    settings::ShellDiscriminants::Program => {
                                        let program = match settings_value {
                                            settings::Shell::Program(program) => program.clone(),
                                            settings::Shell::WithArguments { program, .. } => program.clone(),
                                            _ => String::from(default_shell),
                                        };
                                        settings::Shell::Program(program)
                                    }
                                    settings::ShellDiscriminants::WithArguments => {
                                        let (program, args, title_override) = match settings_value {
                                            settings::Shell::Program(program) => (program.clone(), vec![], None),
                                            settings::Shell::WithArguments {
                                                program,
                                                args,
                                                title_override,
                                            } => (program.clone(), args.clone(), title_override.clone()),
                                            _ => (String::from(default_shell), vec![], None),
                                        };
                                        settings::Shell::WithArguments {
                                            program,
                                            args,
                                            title_override,
                                        }
                                    }
                                };
                            },
                        }),
                        metadata: None,
                    },
                    pick_discriminant: |settings_content| {
                        Some(
                            settings_content
                                .terminal
                                .as_ref()?
                                .project
                                .shell
                                .as_ref()?
                                .discriminant() as usize,
                        )
                    },
                    fields: dynamic_variants::<settings::Shell>()
                        .into_iter()
                        .map(|variant| match variant {
                            settings::ShellDiscriminants::System => vec![],
                            settings::ShellDiscriminants::Program => vec![SettingItem {
                                files: USER | PROJECT,
                                title: t(keys::debugger::DEBUGGER_PROGRAM).into(),
                                description: t(keys::settings_ui::SETTINGS_TERMINAL_THE_SHELL_PROGRAM_TO_USE).into(),
                                field: Box::new(SettingField {
                                    organization_override: None,
                                    json_path: Some("terminal.shell"),
                                    pick: |settings_content| match settings_content.terminal.as_ref()?.project.shell.as_ref()
                                    {
                                        Some(settings::Shell::Program(program)) => Some(program),
                                        _ => None,
                                    },
                                    write: |settings_content, value, _| {
                                        let Some(value) = value else {
                                            return;
                                        };
                                        match settings_content
                                            .terminal
                                            .get_or_insert_default()
                                            .project
                                            .shell
                                            .as_mut()
                                        {
                                            Some(settings::Shell::Program(program)) => *program = value,
                                            _ => return,
                                        }
                                    },
                                }),
                                metadata: None,
                            }],
                            settings::ShellDiscriminants::WithArguments => vec![
                                SettingItem {
                                    files: USER | PROJECT,
                                    title: t(keys::debugger::DEBUGGER_PROGRAM).into(),
                                    description: t(keys::settings_ui::SETTINGS_TERMINAL_THE_SHELL_PROGRAM_TO_RUN).into(),
                                    field: Box::new(SettingField {
                                        organization_override: None,
                                        json_path: Some("terminal.shell.program"),
                                        pick: |settings_content| {
                                            match settings_content.terminal.as_ref()?.project.shell.as_ref() {
                                                Some(settings::Shell::WithArguments { program, .. }) => Some(program),
                                                _ => None,
                                            }
                                        },
                                        write: |settings_content, value, _| {
                                            let Some(value) = value else {
                                                return;
                                            };
                                            match settings_content
                                                .terminal
                                                .get_or_insert_default()
                                                .project
                                                .shell
                                                .as_mut()
                                            {
                                                Some(settings::Shell::WithArguments { program, .. }) => {
                                                    *program = value
                                                }
                                                _ => return,
                                            }
                                        },
                                    }),
                                    metadata: None,
                                },
                                SettingItem {
                                    files: USER | PROJECT,
                                    title: t(keys::keymap::KEYMAP_ARGUMENTS).into(),
                                    description: t(keys::settings_ui::SETTINGS_TERMINAL_THE_ARGUMENTS_TO_PASS_TO_THE_SHELL_PROGR).into(),
                                    field: Box::new(
                                        SettingField {
                                            organization_override: None,
                                            json_path: Some("terminal.shell.args"),
                                            pick: |settings_content| {
                                                match settings_content.terminal.as_ref()?.project.shell.as_ref() {
                                                    Some(settings::Shell::WithArguments { args, .. }) => Some(args),
                                                    _ => None,
                                                }
                                            },
                                            write: |settings_content, value, _| {
                                                let Some(value) = value else {
                                                    return;
                                                };
                                                match settings_content
                                                    .terminal
                                                    .get_or_insert_default()
                                                    .project
                                                    .shell
                                                    .as_mut()
                                                {
                                                    Some(settings::Shell::WithArguments { args, .. }) => *args = value,
                                                    _ => return,
                                                }
                                            },
                                        }
                                        .unimplemented(),
                                    ),
                                    metadata: None,
                                },
                                SettingItem {
                                    files: USER | PROJECT,
                                    title: t(keys::settings_ui::SETTINGS_TERMINAL_TITLE_OVERRIDE).into(),
                                    description: t(keys::settings_ui::SETTINGS_TERMINAL_AN_OPTIONAL_STRING_TO_OVERRIDE_THE_TITLE).into(),
                                    field: Box::new(SettingField {
                                        organization_override: None,
                                        json_path: Some("terminal.shell.title_override"),
                                        pick: |settings_content| {
                                            match settings_content.terminal.as_ref()?.project.shell.as_ref() {
                                                Some(settings::Shell::WithArguments { title_override, .. }) => {
                                                    title_override.as_ref().or(DEFAULT_EMPTY_STRING)
                                                }
                                                _ => None,
                                            }
                                        },
                                        write: |settings_content, value, _| {
                                            match settings_content
                                                .terminal
                                                .get_or_insert_default()
                                                .project
                                                .shell
                                                .as_mut()
                                            {
                                                Some(settings::Shell::WithArguments { title_override, .. }) => {
                                                    *title_override = value.filter(|s| !s.is_empty())
                                                }
                                                _ => return,
                                            }
                                        },
                                    }),
                                    metadata: None,
                                },
                            ],
                        })
                        .collect(),
                }),
                SettingsPageItem::DynamicItem(DynamicItem {
                    discriminant: SettingItem {
                        files: USER | PROJECT,
                        title: t(keys::debugger::DEBUGGER_WORKING_DIRECTORY).into(),
                        description: t(keys::settings_ui::SETTINGS_TERMINAL_WHAT_WORKING_DIRECTORY_TO_USE_WHEN_LAUNC).into(),
                        field: Box::new(SettingField {
                            organization_override: None,
                            json_path: Some("terminal.working_directory$"),
                            pick: |settings_content| {
                                Some(&dynamic_variants::<settings::WorkingDirectory>()[
                                    settings_content
                                        .terminal
                                        .as_ref()?
                                        .project
                                        .working_directory
                                        .as_ref()?
                                        .discriminant() as usize
                                ])
                            },
                            write: |settings_content, value, _| {
                                let Some(value) = value else {
                                    if let Some(terminal) = settings_content.terminal.as_mut() {
                                        terminal.project.working_directory = None;
                                    }
                                    return;
                                };
                                let settings_value = settings_content
                                    .terminal
                                    .get_or_insert_default()
                                    .project
                                    .working_directory
                                    .get_or_insert_with(|| settings::WorkingDirectory::CurrentProjectDirectory);
                                *settings_value = match value {
                                    settings::WorkingDirectoryDiscriminants::CurrentFileDirectory => {
                                        settings::WorkingDirectory::CurrentFileDirectory
                                    },
                                    settings::WorkingDirectoryDiscriminants::CurrentProjectDirectory => {
                                        settings::WorkingDirectory::CurrentProjectDirectory
                                    }
                                    settings::WorkingDirectoryDiscriminants::FirstProjectDirectory => {
                                        settings::WorkingDirectory::FirstProjectDirectory
                                    }
                                    settings::WorkingDirectoryDiscriminants::AlwaysHome => {
                                        settings::WorkingDirectory::AlwaysHome
                                    }
                                    settings::WorkingDirectoryDiscriminants::Always => {
                                        let directory = match settings_value {
                                            settings::WorkingDirectory::Always { .. } => return,
                                            _ => String::new(),
                                        };
                                        settings::WorkingDirectory::Always { directory }
                                    }
                                };
                            },
                        }),
                        metadata: None,
                    },
                    pick_discriminant: |settings_content| {
                        Some(
                            settings_content
                                .terminal
                                .as_ref()?
                                .project
                                .working_directory
                                .as_ref()?
                                .discriminant() as usize,
                        )
                    },
                    fields: dynamic_variants::<settings::WorkingDirectory>()
                        .into_iter()
                        .map(|variant| match variant {
                            settings::WorkingDirectoryDiscriminants::CurrentFileDirectory => vec![],
                            settings::WorkingDirectoryDiscriminants::CurrentProjectDirectory => vec![],
                            settings::WorkingDirectoryDiscriminants::FirstProjectDirectory => vec![],
                            settings::WorkingDirectoryDiscriminants::AlwaysHome => vec![],
                            settings::WorkingDirectoryDiscriminants::Always => vec![SettingItem {
                                files: USER | PROJECT,
                                title: t(keys::settings_ui::SETTINGS_TERMINAL_DIRECTORY).into(),
                                description: t(keys::settings_ui::SETTINGS_TERMINAL_THE_DIRECTORY_PATH_TO_USE_WILL_BE_SHELL).into(),
                                field: Box::new(SettingField {
                                    organization_override: None,
                                    json_path: Some("terminal.working_directory.always"),
                                    pick: |settings_content| {
                                        match settings_content.terminal.as_ref()?.project.working_directory.as_ref() {
                                            Some(settings::WorkingDirectory::Always { directory }) => Some(directory),
                                            _ => None,
                                        }
                                    },
                                    write: |settings_content, value, _| {
                                        let value = value.unwrap_or_default();
                                        match settings_content
                                            .terminal
                                            .get_or_insert_default()
                                            .project
                                            .working_directory
                                            .as_mut()
                                        {
                                            Some(settings::WorkingDirectory::Always { directory }) => *directory = value,
                                            _ => return,
                                        }
                                    },
                                }),
                                metadata: None,
                            }],
                        })
                        .collect(),
                }),
                SettingsPageItem::SettingItem(SettingItem {
                    title: t(keys::settings_ui::SETTINGS_TERMINAL_ENVIRONMENT_VARIABLES).into(),
                    description: t(keys::settings_ui::SETTINGS_TERMINAL_KEY_VALUE_PAIRS_TO_ADD_TO_THE_TERMINAL_S).into(),
                    field: Box::new(
                        SettingField {
                            organization_override: None,
                            json_path: Some("terminal.env"),
                            pick: |settings_content| settings_content.terminal.as_ref()?.project.env.as_ref(),
                            write: |settings_content, value, _| {
                                settings_content.terminal.get_or_insert_default().project.env = value;
                            },
                        }
                        .unimplemented(),
                    ),
                    metadata: None,
                    files: USER | PROJECT,
                }),
                SettingsPageItem::SettingItem(SettingItem {
                    title: t(keys::settings_ui::SETTINGS_TERMINAL_DETECT_VIRTUAL_ENVIRONMENT).into(),
                    description: t(keys::settings_ui::SETTINGS_TERMINAL_ACTIVATES_THE_PYTHON_VIRTUAL_ENVIRONMENT).into(),
                    field: Box::new(
                        SettingField {
                            organization_override: None,
                            json_path: Some("terminal.detect_venv"),
                            pick: |settings_content| settings_content.terminal.as_ref()?.project.detect_venv.as_ref(),
                            write: |settings_content, value, _| {
                                settings_content
                                    .terminal
                                    .get_or_insert_default()
                                    .project
                                    .detect_venv = value;
                            },
                        }
                        .unimplemented(),
                    ),
                    metadata: None,
                    files: USER | PROJECT,
                }),
            ]
    }

    fn font_section() -> [SettingsPageItem; 6] {
        [
            SettingsPageItem::SectionHeader(
                t(keys::settings_ui::SETTINGS_TERMINAL_SECTION_FONT).into(),
            ),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_APPEARANCE_FONT_SIZE).into(),
                description: t(
                    keys::settings_ui::SETTINGS_TERMINAL_FONT_SIZE_FOR_TERMINAL_TEXT_IF_NOT_SET_D,
                )
                .into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("terminal.font_size"),
                    pick: |settings_content| {
                        settings_content
                            .terminal
                            .as_ref()
                            .and_then(|terminal| terminal.font_size.as_ref())
                            .or(settings_content.theme.buffer_font_size.as_ref())
                    },
                    write: |settings_content, value, _| {
                        settings_content.terminal.get_or_insert_default().font_size = value;
                    },
                }),
                metadata: None,
                files: USER,
            }),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_APPEARANCE_FONT_FAMILY).into(),
                description: t(
                    keys::settings_ui::SETTINGS_TERMINAL_FONT_FAMILY_FOR_TERMINAL_TEXT_IF_NOT_SET,
                )
                .into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("terminal.font_family"),
                    pick: |settings_content| {
                        settings_content
                            .terminal
                            .as_ref()
                            .and_then(|terminal| terminal.font_family.as_ref())
                            .or(settings_content.theme.buffer_font_family.as_ref())
                    },
                    write: |settings_content, value, _| {
                        settings_content
                            .terminal
                            .get_or_insert_default()
                            .font_family = value;
                    },
                }),
                metadata: None,
                files: USER,
            }),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_APPEARANCE_FONT_FALLBACKS).into(),
                description: t(
                    keys::settings_ui::SETTINGS_TERMINAL_FONT_FALLBACKS_FOR_TERMINAL_TEXT_IF_NOT,
                )
                .into(),
                field: Box::new(
                    SettingField {
                        organization_override: None,
                        json_path: Some("terminal.font_fallbacks"),
                        pick: |settings_content| {
                            settings_content
                                .terminal
                                .as_ref()
                                .and_then(|terminal| terminal.font_fallbacks.as_ref())
                                .or(settings_content.theme.buffer_font_fallbacks.as_ref())
                        },
                        write: |settings_content, value, _| {
                            settings_content
                                .terminal
                                .get_or_insert_default()
                                .font_fallbacks = value;
                        },
                    }
                    .unimplemented(),
                ),
                metadata: None,
                files: USER,
            }),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_APPEARANCE_FONT_WEIGHT).into(),
                description: t(
                    keys::settings_ui::SETTINGS_TERMINAL_FONT_WEIGHT_FOR_TERMINAL_TEXT_IN_CSS_WEI,
                )
                .into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("terminal.font_weight"),
                    pick: |settings_content| {
                        settings_content.terminal.as_ref()?.font_weight.as_ref()
                    },
                    write: |settings_content, value, _| {
                        settings_content
                            .terminal
                            .get_or_insert_default()
                            .font_weight = value;
                    },
                }),
                metadata: None,
                files: USER,
            }),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_APPEARANCE_FONT_FEATURES).into(),
                description: t(
                    keys::settings_ui::SETTINGS_TERMINAL_FONT_FEATURES_FOR_TERMINAL_TEXT,
                )
                .into(),
                field: Box::new(
                    SettingField {
                        organization_override: None,
                        json_path: Some("terminal.font_features"),
                        pick: |settings_content| {
                            settings_content
                                .terminal
                                .as_ref()
                                .and_then(|terminal| terminal.font_features.as_ref())
                                .or(settings_content.theme.buffer_font_features.as_ref())
                        },
                        write: |settings_content, value, _| {
                            settings_content
                                .terminal
                                .get_or_insert_default()
                                .font_features = value;
                        },
                    }
                    .unimplemented(),
                ),
                metadata: None,
                files: USER,
            }),
        ]
    }

    fn display_settings_section() -> [SettingsPageItem; 6] {
        [
            SettingsPageItem::SectionHeader(
                t(keys::settings_ui::SETTINGS_TERMINAL_SECTION_DISPLAY_SETTINGS).into(),
            ),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_APPEARANCE_LINE_HEIGHT).into(),
                description: t(keys::settings_ui::SETTINGS_TERMINAL_LINE_HEIGHT_FOR_TERMINAL_TEXT)
                    .into(),
                field: Box::new(
                    SettingField {
                        organization_override: None,
                        json_path: Some("terminal.line_height"),
                        pick: |settings_content| {
                            settings_content.terminal.as_ref()?.line_height.as_ref()
                        },
                        write: |settings_content, value, _| {
                            settings_content
                                .terminal
                                .get_or_insert_default()
                                .line_height = value;
                        },
                    }
                    .unimplemented(),
                ),
                metadata: None,
                files: USER,
            }),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_APPEARANCE_CURSOR_SHAPE).into(),
                description: t(
                    keys::settings_ui::SETTINGS_TERMINAL_DEFAULT_CURSOR_SHAPE_FOR_THE_TERMINAL_BA,
                )
                .into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("terminal.cursor_shape"),
                    pick: |settings_content| {
                        settings_content.terminal.as_ref()?.cursor_shape.as_ref()
                    },
                    write: |settings_content, value, _| {
                        settings_content
                            .terminal
                            .get_or_insert_default()
                            .cursor_shape = value;
                    },
                }),
                metadata: None,
                files: USER,
            }),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_TERMINAL_CURSOR_BLINKING).into(),
                description: t(
                    keys::settings_ui::SETTINGS_TERMINAL_SETS_THE_CURSOR_BLINKING_BEHAVIOR_IN_THE,
                )
                .into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("terminal.blinking"),
                    pick: |settings_content| settings_content.terminal.as_ref()?.blinking.as_ref(),
                    write: |settings_content, value, _| {
                        settings_content.terminal.get_or_insert_default().blinking = value;
                    },
                }),
                metadata: None,
                files: USER,
            }),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_TERMINAL_ALTERNATE_SCROLL).into(),
                description: t(
                    keys::settings_ui::SETTINGS_TERMINAL_WHETHER_ALTERNATE_SCROLL_MODE_IS_ACTIVE,
                )
                .into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("terminal.alternate_scroll"),
                    pick: |settings_content| {
                        settings_content
                            .terminal
                            .as_ref()?
                            .alternate_scroll
                            .as_ref()
                    },
                    write: |settings_content, value, _| {
                        settings_content
                            .terminal
                            .get_or_insert_default()
                            .alternate_scroll = value;
                    },
                }),
                metadata: None,
                files: USER,
            }),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_TERMINAL_MINIMUM_CONTRAST).into(),
                description: t(
                    keys::settings_ui::SETTINGS_TERMINAL_THE_MINIMUM_APCA_PERCEPTUAL_CONTRAST_BET,
                )
                .into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("terminal.minimum_contrast"),
                    pick: |settings_content| {
                        settings_content
                            .terminal
                            .as_ref()?
                            .minimum_contrast
                            .as_ref()
                    },
                    write: |settings_content, value, _| {
                        settings_content
                            .terminal
                            .get_or_insert_default()
                            .minimum_contrast = value;
                    },
                }),
                metadata: None,
                files: USER,
            }),
        ]
    }

    fn behavior_settings_section() -> [SettingsPageItem; 6] {
        [
            SettingsPageItem::SectionHeader(
                t(keys::settings_ui::SETTINGS_TERMINAL_SECTION_BEHAVIOR_SETTINGS).into(),
            ),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_TERMINAL_OPTION_AS_META).into(),
                description: t(
                    keys::settings_ui::SETTINGS_TERMINAL_WHETHER_THE_OPTION_KEY_BEHAVES_AS_THE_ME,
                )
                .into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("terminal.option_as_meta"),
                    pick: |settings_content| {
                        settings_content.terminal.as_ref()?.option_as_meta.as_ref()
                    },
                    write: |settings_content, value, _| {
                        settings_content
                            .terminal
                            .get_or_insert_default()
                            .option_as_meta = value;
                    },
                }),
                metadata: None,
                files: USER,
            }),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_TERMINAL_COPY_ON_SELECT).into(),
                description: t(
                    keys::settings_ui::SETTINGS_TERMINAL_WHETHER_SELECTING_TEXT_IN_THE_TERMINAL_A,
                )
                .into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("terminal.copy_on_select"),
                    pick: |settings_content| {
                        settings_content.terminal.as_ref()?.copy_on_select.as_ref()
                    },
                    write: |settings_content, value, _| {
                        settings_content
                            .terminal
                            .get_or_insert_default()
                            .copy_on_select = value;
                    },
                }),
                metadata: None,
                files: USER,
            }),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_TERMINAL_KEEP_SELECTION_ON_COPY).into(),
                description: t(
                    keys::settings_ui::SETTINGS_TERMINAL_WHETHER_TO_KEEP_THE_TEXT_SELECTION_AFTER,
                )
                .into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("terminal.keep_selection_on_copy"),
                    pick: |settings_content| {
                        settings_content
                            .terminal
                            .as_ref()?
                            .keep_selection_on_copy
                            .as_ref()
                    },
                    write: |settings_content, value, _| {
                        settings_content
                            .terminal
                            .get_or_insert_default()
                            .keep_selection_on_copy = value;
                    },
                }),
                metadata: None,
                files: USER,
            }),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_TERMINAL_OPEN_LINKS_IN_MOUSE_MODE).into(),
                description: t(
                    keys::settings_ui::SETTINGS_TERMINAL_WHETHER_CMD_CLICK_CTRL_CLICK_ON_LINUX_AN,
                )
                .into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("terminal.open_links_in_mouse_mode"),
                    pick: |settings_content| {
                        settings_content
                            .terminal
                            .as_ref()?
                            .open_links_in_mouse_mode
                            .as_ref()
                    },
                    write: |settings_content, value, _| {
                        settings_content
                            .terminal
                            .get_or_insert_default()
                            .open_links_in_mouse_mode = value;
                    },
                }),
                metadata: None,
                files: USER,
            }),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_TERMINAL_AUDIBLE_BELL).into(),
                description:
                    "Whether to play a sound when the BEL character (`\\a`, `0x07`) is printed"
                        .into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("terminal.bell"),
                    pick: |settings_content| settings_content.terminal.as_ref()?.bell.as_ref(),
                    write: |settings_content, value, _| {
                        settings_content.terminal.get_or_insert_default().bell = value;
                    },
                }),
                metadata: None,
                files: USER,
            }),
        ]
    }

    fn layout_settings_section() -> [SettingsPageItem; 3] {
        [
            SettingsPageItem::SectionHeader(
                t(keys::settings_ui::SETTINGS_TERMINAL_SECTION_LAYOUT_SETTINGS).into(),
            ),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_TERMINAL_DEFAULT_WIDTH).into(),
                description: t(keys::settings_ui::SETTINGS_TERMINAL_DEFAULT_WIDTH_HINT).into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("terminal.default_width"),
                    pick: |settings_content| {
                        settings_content.terminal.as_ref()?.default_width.as_ref()
                    },
                    write: |settings_content, value, _| {
                        settings_content
                            .terminal
                            .get_or_insert_default()
                            .default_width = value;
                    },
                }),
                metadata: None,
                files: USER,
            }),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_TERMINAL_DEFAULT_HEIGHT).into(),
                description: t(keys::settings_ui::SETTINGS_TERMINAL_DEFAULT_HEIGHT_HINT).into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("terminal.default_height"),
                    pick: |settings_content| {
                        settings_content.terminal.as_ref()?.default_height.as_ref()
                    },
                    write: |settings_content, value, _| {
                        settings_content
                            .terminal
                            .get_or_insert_default()
                            .default_height = value;
                    },
                }),
                metadata: None,
                files: USER,
            }),
        ]
    }

    fn advanced_settings_section() -> [SettingsPageItem; 3] {
        [
            SettingsPageItem::SectionHeader(
                t(keys::settings_ui::SETTINGS_TERMINAL_SECTION_ADVANCED_SETTINGS).into(),
            ),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_TERMINAL_MAX_SCROLL_HISTORY_LINES).into(),
                description: t(
                    keys::settings_ui::SETTINGS_TERMINAL_MAXIMUM_NUMBER_OF_LINES_TO_KEEP_IN_SCROL,
                )
                .into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("terminal.max_scroll_history_lines"),
                    pick: |settings_content| {
                        settings_content
                            .terminal
                            .as_ref()?
                            .max_scroll_history_lines
                            .as_ref()
                    },
                    write: |settings_content, value, _| {
                        settings_content
                            .terminal
                            .get_or_insert_default()
                            .max_scroll_history_lines = value;
                    },
                }),
                metadata: None,
                files: USER,
            }),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_TERMINAL_SCROLL_MULTIPLIER).into(),
                description: t(
                    keys::settings_ui::SETTINGS_TERMINAL_THE_MULTIPLIER_FOR_SCROLLING_IN_THE_TERM,
                )
                .into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("terminal.scroll_multiplier"),
                    pick: |settings_content| {
                        settings_content
                            .terminal
                            .as_ref()?
                            .scroll_multiplier
                            .as_ref()
                    },
                    write: |settings_content, value, _| {
                        settings_content
                            .terminal
                            .get_or_insert_default()
                            .scroll_multiplier = value;
                    },
                }),
                metadata: None,
                files: USER,
            }),
        ]
    }

    fn toolbar_section() -> [SettingsPageItem; 2] {
        [
            SettingsPageItem::SectionHeader(
                t(keys::settings_ui::SETTINGS_EDITOR_SECTION_TOOLBAR).into(),
            ),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_EDITOR_BREADCRUMBS).into(),
                description: t(
                    keys::settings_ui::SETTINGS_TERMINAL_DISPLAY_THE_TERMINAL_TITLE_IN_BREADCRUMB,
                )
                .into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("terminal.toolbar.breadcrumbs"),
                    pick: |settings_content| {
                        settings_content
                            .terminal
                            .as_ref()?
                            .toolbar
                            .as_ref()?
                            .breadcrumbs
                            .as_ref()
                    },
                    write: |settings_content, value, _| {
                        settings_content
                            .terminal
                            .get_or_insert_default()
                            .toolbar
                            .get_or_insert_default()
                            .breadcrumbs = value;
                    },
                }),
                metadata: None,
                files: USER,
            }),
        ]
    }

    fn scrollbar_section() -> [SettingsPageItem; 2] {
        [
            SettingsPageItem::SectionHeader(
                t(keys::settings_ui::SETTINGS_EDITOR_SECTION_SCROLLBAR).into(),
            ),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_PANELS_SHOW_SCROLLBAR).into(),
                description: t(
                    keys::settings_ui::SETTINGS_TERMINAL_WHEN_TO_SHOW_THE_SCROLLBAR_IN_THE_TERMIN,
                )
                .into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("terminal.scrollbar.show"),
                    pick: |settings_content| {
                        show_scrollbar_or_editor(settings_content, |settings_content| {
                            settings_content
                                .terminal
                                .as_ref()?
                                .scrollbar
                                .as_ref()?
                                .show
                                .as_ref()
                        })
                    },
                    write: |settings_content, value, _| {
                        settings_content
                            .terminal
                            .get_or_insert_default()
                            .scrollbar
                            .get_or_insert_default()
                            .show = value;
                    },
                }),
                metadata: None,
                files: USER,
            }),
        ]
    }

    SettingsPage {
        title: t(keys::settings_ui::SETTINGS_TERMINAL_TERMINAL).into(),
        items: concat_sections![
            environment_section(),
            font_section(),
            display_settings_section(),
            behavior_settings_section(),
            layout_settings_section(),
            advanced_settings_section(),
            toolbar_section(),
            scrollbar_section(),
        ],
    }
}

fn version_control_page() -> SettingsPage {
    fn git_integration_section() -> [SettingsPageItem; 3] {
        [
            SettingsPageItem::SectionHeader(t(keys::settings_ui::SETTINGS_VERSION_CONTROL_SECTION_GIT_INTEGRATION).into()),
            SettingsPageItem::DynamicItem(DynamicItem {
                discriminant: SettingItem {
                    files: USER,
                    title: t(keys::settings_ui::SETTINGS_VERSION_CONTROL_DISABLE_GIT_INTEGRATION).into(),
                    description: t(keys::settings_ui::SETTINGS_VERSION_CONTROL_DISABLE_ALL_GIT_INTEGRATION_FEATURES_IN).into(),
                    field: Box::new(SettingField::<bool> {
                        organization_override: None,
                        json_path: Some("git.disable_git"),
                        pick: |settings_content| {
                            settings_content
                                .git
                                .as_ref()?
                                .enabled
                                .as_ref()?
                                .disable_git
                                .as_ref()
                        },
                        write: |settings_content, value, _| {
                            settings_content
                                .git
                                .get_or_insert_default()
                                .enabled
                                .get_or_insert_default()
                                .disable_git = value;
                        },
                    }),
                    metadata: None,
                },
                pick_discriminant: |settings_content| {
                    let disabled = settings_content
                        .git
                        .as_ref()?
                        .enabled
                        .as_ref()?
                        .disable_git
                        .unwrap_or(false);
                    Some(if disabled { 0 } else { 1 })
                },
                fields: vec![
                    vec![],
                    vec![
                        SettingItem {
                            files: USER,
                            title: t(keys::settings_ui::SETTINGS_VERSION_CONTROL_ENABLE_GIT_STATUS).into(),
                            description: t(keys::settings_ui::SETTINGS_VERSION_CONTROL_SHOW_GIT_STATUS_INFORMATION_IN_THE_EDITO).into(),
                            field: Box::new(SettingField::<bool> {
                                organization_override: None,
                                json_path: Some("git.enable_status"),
                                pick: |settings_content| {
                                    settings_content
                                        .git
                                        .as_ref()?
                                        .enabled
                                        .as_ref()?
                                        .enable_status
                                        .as_ref()
                                },
                                write: |settings_content, value, _| {
                                    settings_content
                                        .git
                                        .get_or_insert_default()
                                        .enabled
                                        .get_or_insert_default()
                                        .enable_status = value;
                                },
                            }),
                            metadata: None,
                        },
                        SettingItem {
                            files: USER,
                            title: t(keys::settings_ui::SETTINGS_VERSION_CONTROL_ENABLE_GIT_DIFF).into(),
                            description: t(keys::settings_ui::SETTINGS_VERSION_CONTROL_SHOW_GIT_DIFF_INFORMATION_IN_THE_EDITOR).into(),
                            field: Box::new(SettingField::<bool> {
                                organization_override: None,
                                json_path: Some("git.enable_diff"),
                                pick: |settings_content| {
                                    settings_content
                                        .git
                                        .as_ref()?
                                        .enabled
                                        .as_ref()?
                                        .enable_diff
                                        .as_ref()
                                },
                                write: |settings_content, value, _| {
                                    settings_content
                                        .git
                                        .get_or_insert_default()
                                        .enabled
                                        .get_or_insert_default()
                                        .enable_diff = value;
                                },
                            }),
                            metadata: None,
                        },
                    ],
                ],
            }),
            SettingsPageItem::DynamicItem(DynamicItem {
                discriminant: SettingItem {
                    files: USER,
                    title: "Auto Fetch".into(),
                    description: "Automatically fetch the default branch's remote and the current branch's upstream remote.".into(),
                    field: Box::new(SettingField::<bool> {
                        organization_override: None,
                        json_path: Some("git.auto_fetch.enabled"),
                        pick: |settings_content| {
                            settings_content
                                .git
                                .as_ref()?
                                .auto_fetch
                                .as_ref()?
                                .enabled
                                .as_ref()
                        },
                        write: |settings_content, value, _| {
                            settings_content
                                .git
                                .get_or_insert_default()
                                .auto_fetch
                                .get_or_insert_default()
                                .enabled = value;
                        },
                    }),
                    metadata: None,
                },
                pick_discriminant: |settings_content| {
                    Some(
                        *settings_content
                            .git
                            .as_ref()?
                            .auto_fetch
                            .as_ref()?
                            .enabled
                            .as_ref()? as usize,
                    )
                },
                fields: vec![
                    vec![],
                    vec![SettingItem {
                        files: USER,
                        title: "Auto Fetch Interval".into(),
                        description: "How long to wait between automatic fetches, in seconds. (min: 15)".into(),
                        field: Box::new(SettingField::<u64> {
                            organization_override: None,
                            json_path: Some("git.auto_fetch.interval_secs"),
                            pick: |settings_content| {
                                settings_content
                                    .git
                                    .as_ref()?
                                    .auto_fetch
                                    .as_ref()?
                                    .interval_secs
                                    .as_ref()
                            },
                            write: |settings_content, value, _| {
                                settings_content
                                    .git
                                    .get_or_insert_default()
                                    .auto_fetch
                                    .get_or_insert_default()
                                    .interval_secs = value;
                            },
                        }),
                        metadata: None,
                    }],
                ],
            }),
        ]
    }

    fn git_gutter_section() -> [SettingsPageItem; 3] {
        [
            SettingsPageItem::SectionHeader(t(keys::settings_ui::SETTINGS_VERSION_CONTROL_SECTION_GIT_GUTTER).into()),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_VERSION_CONTROL_VISIBILITY).into(),
                description: t(keys::settings_ui::SETTINGS_VERSION_CONTROL_CONTROL_WHETHER_GIT_STATUS_IS_SHOWN_IN_T).into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("git.git_gutter"),
                    pick: |settings_content| settings_content.git.as_ref()?.git_gutter.as_ref(),
                    write: |settings_content, value, _| {
                        settings_content.git.get_or_insert_default().git_gutter = value;
                    },
                }),
                metadata: None,
                files: USER,
            }),
            // todo(settings_ui): Figure out the right default for this value in default.json
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_LANGUAGES_DEBOUNCE).into(),
                description: t(keys::settings_ui::SETTINGS_VERSION_CONTROL_DEBOUNCE_THRESHOLD_IN_MILLISECONDS_AFTER).into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("git.gutter_debounce"),
                    pick: |settings_content| {
                        settings_content.git.as_ref()?.gutter_debounce.as_ref()
                    },
                    write: |settings_content, value, _| {
                        settings_content.git.get_or_insert_default().gutter_debounce = value;
                    },
                }),
                metadata: None,
                files: USER,
            }),
        ]
    }

    fn inline_git_blame_section() -> [SettingsPageItem; 6] {
        [
            SettingsPageItem::SectionHeader(t(keys::settings_ui::SETTINGS_VERSION_CONTROL_SECTION_INLINE_GIT_BLAME).into()),
            SettingsPageItem::DynamicItem(DynamicItem {
                discriminant: SettingItem {
                    title: t(keys::settings_ui::SETTINGS_EDITOR_ENABLED).into(),
                    description: t(keys::settings_ui::SETTINGS_VERSION_CONTROL_WHETHER_OR_NOT_TO_SHOW_GIT_BLAME_DATA_FO).into(),
                    field: Box::new(SettingField {
                        organization_override: None,
                        json_path: Some("git.inline_blame.enabled"),
                        pick: |settings_content| {
                            settings_content
                                .git
                                .as_ref()?
                                .inline_blame
                                .as_ref()?
                                .enabled
                                .as_ref()
                        },
                        write: |settings_content, value, _| {
                            settings_content
                                .git
                                .get_or_insert_default()
                                .inline_blame
                                .get_or_insert_default()
                                .enabled = value;
                        },
                    }),
                    metadata: None,
                    files: USER,
                },
                pick_discriminant: |settings_content| {
                    Some(
                        *settings_content
                            .git
                            .as_ref()?
                            .inline_blame
                            .as_ref()?
                            .enabled
                            .as_ref()? as usize,
                    )
                },
                fields: vec![
                    vec![],
                    vec![SettingItem {
                        title: t(keys::settings_ui::SETTINGS_VERSION_CONTROL_LOCATION).into(),
                        description: t(keys::settings_ui::SETTINGS_VERSION_CONTROL_WHERE_TO_RENDER_GIT_BLAME_WHEN_IT_IS_ENA).into(),
                        field: Box::new(SettingField {
                            organization_override: None,
                            json_path: Some("git.inline_blame.location"),
                            pick: |settings_content| {
                                settings_content
                                    .git
                                    .as_ref()?
                                    .inline_blame
                                    .as_ref()?
                                    .location
                                    .as_ref()
                            },
                            write: |settings_content, value, _| {
                                settings_content
                                    .git
                                    .get_or_insert_default()
                                    .inline_blame
                                    .get_or_insert_default()
                                    .location = value;
                            },
                        }),
                        metadata: None,
                        files: USER,
                    }],
                ],
            }),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_EDITOR_DELAY).into(),
                description: t(keys::settings_ui::SETTINGS_VERSION_CONTROL_THE_DELAY_AFTER_WHICH_THE_INLINE_BLAME_I).into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("git.inline_blame.delay_ms"),
                    pick: |settings_content| {
                        settings_content
                            .git
                            .as_ref()?
                            .inline_blame
                            .as_ref()?
                            .delay_ms
                            .as_ref()
                    },
                    write: |settings_content, value, _| {
                        settings_content
                            .git
                            .get_or_insert_default()
                            .inline_blame
                            .get_or_insert_default()
                            .delay_ms = value;
                    },
                }),
                metadata: None,
                files: USER,
            }),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_LANGUAGES_PADDING).into(),
                description: t(keys::settings_ui::SETTINGS_VERSION_CONTROL_PADDING_BETWEEN_THE_END_OF_THE_SOURCE_LI).into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("git.inline_blame.padding"),
                    pick: |settings_content| {
                        settings_content
                            .git
                            .as_ref()?
                            .inline_blame
                            .as_ref()?
                            .padding
                            .as_ref()
                    },
                    write: |settings_content, value, _| {
                        settings_content
                            .git
                            .get_or_insert_default()
                            .inline_blame
                            .get_or_insert_default()
                            .padding = value;
                    },
                }),
                metadata: None,
                files: USER,
            }),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_LANGUAGES_MINIMUM_COLUMN).into(),
                description: t(keys::settings_ui::SETTINGS_VERSION_CONTROL_THE_MINIMUM_COLUMN_NUMBER_AT_WHICH_TO_SH).into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("git.inline_blame.min_column"),
                    pick: |settings_content| {
                        settings_content
                            .git
                            .as_ref()?
                            .inline_blame
                            .as_ref()?
                            .min_column
                            .as_ref()
                    },
                    write: |settings_content, value, _| {
                        settings_content
                            .git
                            .get_or_insert_default()
                            .inline_blame
                            .get_or_insert_default()
                            .min_column = value;
                    },
                }),
                metadata: None,
                files: USER,
            }),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_VERSION_CONTROL_SHOW_COMMIT_SUMMARY).into(),
                description: t(keys::settings_ui::SETTINGS_VERSION_CONTROL_SHOW_COMMIT_SUMMARY_AS_PART_OF_THE_INLIN).into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("git.inline_blame.show_commit_summary"),
                    pick: |settings_content| {
                        settings_content
                            .git
                            .as_ref()?
                            .inline_blame
                            .as_ref()?
                            .show_commit_summary
                            .as_ref()
                    },
                    write: |settings_content, value, _| {
                        settings_content
                            .git
                            .get_or_insert_default()
                            .inline_blame
                            .get_or_insert_default()
                            .show_commit_summary = value;
                    },
                }),
                metadata: None,
                files: USER,
            }),
        ]
    }

    fn git_blame_view_section() -> [SettingsPageItem; 2] {
        [
            SettingsPageItem::SectionHeader(t(keys::settings_ui::SETTINGS_VERSION_CONTROL_SECTION_GIT_BLAME_VIEW).into()),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_VERSION_CONTROL_SHOW_AVATAR).into(),
                description: t(keys::settings_ui::SETTINGS_VERSION_CONTROL_SHOW_THE_AVATAR_OF_THE_AUTHOR_OF_THE_COM).into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("git.blame.show_avatar"),
                    pick: |settings_content| {
                        settings_content
                            .git
                            .as_ref()?
                            .blame
                            .as_ref()?
                            .show_avatar
                            .as_ref()
                    },
                    write: |settings_content, value, _| {
                        settings_content
                            .git
                            .get_or_insert_default()
                            .blame
                            .get_or_insert_default()
                            .show_avatar = value;
                    },
                }),
                metadata: None,
                files: USER,
            }),
        ]
    }

    fn branch_picker_section() -> [SettingsPageItem; 2] {
        [
            SettingsPageItem::SectionHeader(t(keys::settings_ui::SETTINGS_VERSION_CONTROL_SECTION_BRANCH_PICKER).into()),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_VERSION_CONTROL_SHOW_AUTHOR_NAME).into(),
                description: t(keys::settings_ui::SETTINGS_VERSION_CONTROL_SHOW_AUTHOR_NAME_AS_PART_OF_THE_COMMIT_I).into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("git.branch_picker.show_author_name"),
                    pick: |settings_content| {
                        settings_content
                            .git
                            .as_ref()?
                            .branch_picker
                            .as_ref()?
                            .show_author_name
                            .as_ref()
                    },
                    write: |settings_content, value, _| {
                        settings_content
                            .git
                            .get_or_insert_default()
                            .branch_picker
                            .get_or_insert_default()
                            .show_author_name = value;
                    },
                }),
                metadata: None,
                files: USER,
            }),
        ]
    }

    fn git_hunks_section() -> [SettingsPageItem; 5] {
        [
            SettingsPageItem::SectionHeader(t(keys::settings_ui::SETTINGS_VERSION_CONTROL_SECTION_GIT_HUNKS).into()),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_VERSION_CONTROL_HUNK_STYLE).into(),
                description: t(keys::settings_ui::SETTINGS_VERSION_CONTROL_HOW_GIT_HUNKS_ARE_DISPLAYED_VISUALLY_IN).into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("git.hunk_style"),
                    pick: |settings_content| settings_content.git.as_ref()?.hunk_style.as_ref(),
                    write: |settings_content, value, _| {
                        settings_content.git.get_or_insert_default().hunk_style = value;
                    },
                }),
                metadata: None,
                files: USER,
            }),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_VERSION_CONTROL_DIFF_BASE).into(),
                description: t(keys::settings_ui::SETTINGS_VERSION_CONTROL_WHETHER_GIT_FEATURES_SHOW_CHANGES_RELATI).into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("git.diff_base"),
                    pick: |settings_content| settings_content.git.as_ref()?.diff_base.as_ref(),
                    write: |settings_content, value, _| {
                        settings_content.git.get_or_insert_default().diff_base = value;
                    },
                }),
                metadata: None,
                files: USER,
            }),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_VERSION_CONTROL_PATH_STYLE).into(),
                description: t(keys::settings_ui::SETTINGS_VERSION_CONTROL_SHOULD_THE_NAME_OR_PATH_BE_DISPLAYED_FIR).into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("git.path_style"),
                    pick: |settings_content| settings_content.git.as_ref()?.path_style.as_ref(),
                    write: |settings_content, value, _| {
                        settings_content.git.get_or_insert_default().path_style = value;
                    },
                }),
                metadata: None,
                files: USER,
            }),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_VERSION_CONTROL_SHOW_STAGE_RESTORE_BUTTONS).into(),
                description: t(keys::settings_ui::SETTINGS_VERSION_CONTROL_WHETHER_TO_SHOW_THE_STAGE_AND_RESTORE_BU).into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("git.show_stage_restore_buttons"),
                    pick: |settings_content| {
                        settings_content
                            .git
                            .as_ref()?
                            .show_stage_restore_buttons
                            .as_ref()
                    },
                    write: |settings_content, value, _| {
                        settings_content
                            .git
                            .get_or_insert_default()
                            .show_stage_restore_buttons = value;
                    },
                }),
                metadata: None,
                files: USER,
            }),
        ]
    }

    fn file_diff_section() -> [SettingsPageItem; 2] {
        [
            SettingsPageItem::SectionHeader(
                t(keys::settings_ui::SETTINGS_VERSION_CONTROL_SECTION_FILE_DIFF).into(),
            ),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_VERSION_CONTROL_SHOW_FULL_FILE_BY_DEFAULT)
                    .into(),
                description: t(keys::settings_ui::SETTINGS_GIT_FILE_DIFF_FULL_HINT).into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("git.file_diff.show_full_file"),
                    pick: |settings_content| {
                        settings_content
                            .git
                            .as_ref()?
                            .file_diff
                            .as_ref()?
                            .show_full_file
                            .as_ref()
                    },
                    write: |settings_content, value, _| {
                        settings_content
                            .git
                            .get_or_insert_default()
                            .file_diff
                            .get_or_insert_default()
                            .show_full_file = value;
                    },
                }),
                metadata: None,
                files: USER,
            }),
        ]
    }

    SettingsPage {
        title: t(keys::settings_ui::SETTINGS_VERSION_CONTROL_VERSION_CONTROL).into(),
        items: concat_sections![
            git_integration_section(),
            git_gutter_section(),
            inline_git_blame_section(),
            git_blame_view_section(),
            branch_picker_section(),
            file_diff_section(),
            git_hunks_section(),
        ],
    }
}

fn collaboration_page() -> SettingsPage {
    fn calls_section() -> [SettingsPageItem; 3] {
        [
            SettingsPageItem::SectionHeader(
                t(keys::settings_ui::SETTINGS_COLLABORATION_SECTION_CALLS).into(),
            ),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_COLLABORATION_MUTE_ON_JOIN).into(),
                description: t(keys::settings_ui::SETTINGS_CALLS_MUTE_ON_JOIN_HINT).into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("calls.mute_on_join"),
                    pick: |settings_content| settings_content.calls.as_ref()?.mute_on_join.as_ref(),
                    write: |settings_content, value, _| {
                        settings_content.calls.get_or_insert_default().mute_on_join = value;
                    },
                }),
                metadata: None,
                files: USER,
            }),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_COLLABORATION_SHARE_ON_JOIN).into(),
                description: t(keys::settings_ui::SETTINGS_CALLS_SHARE_ON_JOIN_HINT).into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("calls.share_on_join"),
                    pick: |settings_content| {
                        settings_content.calls.as_ref()?.share_on_join.as_ref()
                    },
                    write: |settings_content, value, _| {
                        settings_content.calls.get_or_insert_default().share_on_join = value;
                    },
                }),
                metadata: None,
                files: USER,
            }),
        ]
    }

    fn audio_settings() -> [SettingsPageItem; 3] {
        [
            SettingsPageItem::ActionLink(ActionLink {
                title: t(keys::settings_ui::SETTINGS_COLLABORATION_TEST_AUDIO).into(),
                description: Some(t(keys::settings_ui::SETTINGS_AUDIO_TEST_DESCRIPTION).into()),
                button_text: t(keys::settings_ui::SETTINGS_COLLABORATION_TEST_AUDIO).into(),
                on_click: Arc::new(|_settings_window, window, cx| {
                    open_audio_test_window(window, cx);
                }),
                files: USER,
            }),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_COLLABORATION_OUTPUT_AUDIO_DEVICE).into(),
                description: t(
                    keys::settings_ui::SETTINGS_COLLABORATION_SELECT_OUTPUT_AUDIO_DEVICE,
                )
                .into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("audio.experimental.output_audio_device"),
                    pick: |settings_content| {
                        settings_content
                            .audio
                            .as_ref()?
                            .output_audio_device
                            .as_ref()
                            .or(DEFAULT_EMPTY_AUDIO_OUTPUT)
                    },
                    write: |settings_content, value, _| {
                        settings_content
                            .audio
                            .get_or_insert_default()
                            .output_audio_device = value;
                    },
                }),
                metadata: None,
                files: USER,
            }),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_COLLABORATION_INPUT_AUDIO_DEVICE).into(),
                description: t(keys::settings_ui::SETTINGS_COLLABORATION_SELECT_INPUT_AUDIO_DEVICE)
                    .into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("audio.experimental.input_audio_device"),
                    pick: |settings_content| {
                        settings_content
                            .audio
                            .as_ref()?
                            .input_audio_device
                            .as_ref()
                            .or(DEFAULT_EMPTY_AUDIO_INPUT)
                    },
                    write: |settings_content, value, _| {
                        settings_content
                            .audio
                            .get_or_insert_default()
                            .input_audio_device = value;
                    },
                }),
                metadata: None,
                files: USER,
            }),
        ]
    }

    SettingsPage {
        title: t(keys::settings_ui::SETTINGS_COLLABORATION_COLLABORATION).into(),
        items: concat_sections![calls_section(), audio_settings()],
    }
}

fn ai_page(cx: &App) -> SettingsPage {
    fn general_section() -> [SettingsPageItem; 8] {
        [
            SettingsPageItem::SectionHeader(t(keys::settings_ui::SETTINGS_GENERAL_GENERAL).into()),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_AI_DISABLE_AI).into(),
                description: t(
                    keys::settings_ui::SETTINGS_AI_WHETHER_TO_DISABLE_ALL_AI_FEATURES_IN_ZE,
                )
                .into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("disable_ai"),
                    pick: |settings_content| settings_content.project.disable_ai.as_ref(),
                    write: |settings_content, value, _| {
                        settings_content.project.disable_ai = value;
                    },
                }),
                metadata: None,
                files: USER | PROJECT,
            }),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_AI_THREADS_SIDEBAR_POSITION).into(),
                description: t(
                    keys::settings_ui::SETTINGS_AI_WHICH_SIDE_OF_THE_WINDOW_THE_THREADS_SID,
                )
                .into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("agent.threads_sidebar.position"),
                    pick: |settings_content| {
                        settings_content
                            .agent
                            .as_ref()?
                            .threads_sidebar
                            .as_ref()?
                            .position
                            .as_ref()
                    },
                    write: |settings_content, value, _| {
                        settings_content
                            .agent
                            .get_or_insert_default()
                            .set_threads_sidebar_position(value);
                    },
                }),
                metadata: None,
                files: USER,
            }),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_AI_THREADS_SIDEBAR_DEFAULT_WIDTH).into(),
                description: t(
                    keys::settings_ui::SETTINGS_AI_DEFAULT_WIDTH_OF_THE_THREADS_SIDEBAR_CHA,
                )
                .into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("agent.threads_sidebar.default_width"),
                    pick: |settings_content| {
                        settings_content
                            .agent
                            .as_ref()?
                            .threads_sidebar
                            .as_ref()?
                            .default_width
                            .as_ref()
                    },
                    write: |settings_content, value, _| {
                        settings_content
                            .agent
                            .get_or_insert_default()
                            .set_threads_sidebar_default_width(value);
                    },
                }),
                metadata: None,
                files: USER,
            }),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_AI_THREADS_SIDEBAR_AUTO_OPEN).into(),
                description: t(
                    keys::settings_ui::SETTINGS_AI_WHETHER_OPENING_A_FOLDER_IN_AN_EXISTING,
                )
                .into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("agent.threads_sidebar.auto_open"),
                    pick: |settings_content| {
                        settings_content
                            .agent
                            .as_ref()?
                            .threads_sidebar
                            .as_ref()?
                            .auto_open
                            .as_ref()
                    },
                    write: |settings_content, value, _| {
                        settings_content
                            .agent
                            .get_or_insert_default()
                            .set_threads_sidebar_auto_open(value);
                    },
                }),
                metadata: None,
                files: USER,
            }),
            SettingsPageItem::SubPageLink(SubPageLink {
                title: t(keys::settings_ui::SETTINGS_AI_LLM_PROVIDERS).into(),
                r#type: Default::default(),
                json_path: Some("llm_providers"),
                description: Some(
                    t(keys::settings_ui::SETTINGS_LLM_NATIVE_PROVIDERS_DESCRIPTION).into(),
                ),
                search_aliases: &[
                    "ai",
                    "amazon",
                    "anthropic",
                    "api key",
                    "azure",
                    "bedrock",
                    "chat",
                    "claude",
                    "copilot",
                    "gemini",
                    "github",
                    "google",
                    "gpt",
                    "grok",
                    "llama",
                    "llm",
                    "lm studio",
                    "mistral",
                    "ollama",
                    "openai",
                    "opencode",
                    "provider",
                    "vercel",
                    "xai",
                ],
                in_json: false,
                files: USER,
                render: render_llm_providers_page,
            }),
            SettingsPageItem::SubPageLink(SubPageLink {
                title: t(keys::agent::AGENT_MENU_EXTERNAL_AGENTS).into(),
                r#type: Default::default(),
                json_path: Some("agent_servers"),
                description: Some(
                    t(keys::settings_ui::SETTINGS_EXTERNAL_AGENTS_DESCRIPTION).into(),
                ),
                search_aliases: &[
                    "acp",
                    "agent client protocol",
                    "amp",
                    "claude agent",
                    "claude code",
                    "codex",
                    "copilot cli",
                    "cursor",
                    "external agent",
                    "factory droid",
                    "github copilot",
                    "grok build",
                    "junie",
                    "opencode",
                ],
                in_json: false,
                files: USER,
                render: render_external_agents_page,
            }),
            SettingsPageItem::SubPageLink(SubPageLink {
                title: t(keys::agent::AGENT_MENU_MCP_SERVERS).into(),
                r#type: Default::default(),
                json_path: Some("context_servers"),
                description: Some(t(keys::settings_ui::SETTINGS_MCP_DESCRIPTION).into()),
                search_aliases: &["context server", "mcp", "model context protocol"],
                in_json: false,
                files: USER,
                render: render_mcp_servers_page,
            }),
        ]
    }

    fn agent_configuration_section(_cx: &App) -> Box<[SettingsPageItem]> {
        let mut items = vec![SettingsPageItem::SectionHeader(
            t(keys::settings_ui::SETTINGS_EXT_AGENT_AGENT_CONFIGURATION).into(),
        )];

        items.extend([
            SettingsPageItem::SubPageLink(SubPageLink {
                title: t(keys::agent::AGENT_MENU_SKILLS).into(),
                r#type: Default::default(),
                json_path: Some(zed_actions::AGENT_SKILLS_SETTINGS_PATH),
                description: Some(t(keys::settings_ui::SETTINGS_SKILLS_DESCRIPTION).into()),
                search_aliases: &[
                    "agent skill",
                    "agent skills",
                    "custom instructions",
                    "skill",
                    "skills",
                ],
                in_json: false,
                files: USER | PROJECT,
                render: render_skills_setup_page,
            }),
            SettingsPageItem::SubPageLink(SubPageLink {
                title: t(keys::settings_ui::SETTINGS_AI_SANDBOX).into(),
                r#type: Default::default(),
                json_path: Some(zed_actions::AGENT_SANDBOX_SETTINGS_PATH),
                description: Some(
                    t(keys::settings_ui::SETTINGS_TOOL_PERMISSIONS_DESCRIPTION).into(),
                ),
                search_aliases: &[
                    "allow",
                    "domain",
                    "filesystem",
                    "network",
                    "sandbox",
                    "unsandboxed",
                    "permissions",
                ],
                in_json: true,
                files: USER,
                render: render_sandbox_settings_page,
            }),
            SettingsPageItem::SubPageLink(SubPageLink {
                title: t(keys::settings_ui::SETTINGS_AI_TOOL_PERMISSIONS).into(),
                r#type: Default::default(),
                json_path: Some("agent.tool_permissions"),
                description: Some(
                    t(keys::settings_ui::SETTINGS_TOOL_PERMISSIONS_PATTERNS_DESCRIPTION).into(),
                ),
                search_aliases: &[],
                in_json: true,
                files: USER,
                render: render_tool_permissions_setup_page,
            }),
        ]);

        items.extend([
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_AI_SINGLE_FILE_REVIEW).into(),
                description: t(
                    keys::settings_ui::SETTINGS_AI_WHEN_ENABLED_AGENT_EDITS_WILL_ALSO_BE_DI,
                )
                .into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("agent.single_file_review"),
                    pick: |settings_content| {
                        settings_content.agent.as_ref()?.single_file_review.as_ref()
                    },
                    write: |settings_content, value, _| {
                        settings_content
                            .agent
                            .get_or_insert_default()
                            .single_file_review = value;
                    },
                }),
                metadata: None,
                files: USER,
            }),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_AI_ENABLE_FEEDBACK).into(),
                description: t(
                    keys::settings_ui::SETTINGS_AI_SHOW_VOTING_THUMBS_UP_DOWN_ICON_BUTTONS,
                )
                .into(),
                field: Box::new(SettingField {
                    organization_override: Some(|org_config| {
                        if org_config.is_agent_thread_feedback_enabled {
                            None
                        } else {
                            Some(&false)
                        }
                    }),
                    json_path: Some("agent.enable_feedback"),
                    pick: |settings_content| {
                        settings_content.agent.as_ref()?.enable_feedback.as_ref()
                    },
                    write: |settings_content, value, _| {
                        settings_content
                            .agent
                            .get_or_insert_default()
                            .enable_feedback = value;
                    },
                }),
                metadata: None,
                files: USER,
            }),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_AI_NOTIFY_WHEN_AGENT_WAITING).into(),
                description: t(
                    keys::settings_ui::SETTINGS_AI_WHERE_TO_SHOW_NOTIFICATIONS_WHEN_THE_AGE,
                )
                .into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("agent.notify_when_agent_waiting"),
                    pick: |settings_content| {
                        settings_content
                            .agent
                            .as_ref()?
                            .notify_when_agent_waiting
                            .as_ref()
                    },
                    write: |settings_content, value, _| {
                        settings_content
                            .agent
                            .get_or_insert_default()
                            .notify_when_agent_waiting = value;
                    },
                }),
                metadata: None,
                files: USER,
            }),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_AI_PLAY_SOUND_WHEN_AGENT_DONE).into(),
                description: t(
                    keys::settings_ui::SETTINGS_AI_WHEN_TO_PLAY_A_SOUND_WHEN_THE_AGENT_HAS,
                )
                .into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("agent.play_sound_when_agent_done"),
                    pick: |settings_content| {
                        settings_content
                            .agent
                            .as_ref()?
                            .play_sound_when_agent_done
                            .as_ref()
                    },
                    write: |settings_content, value, _| {
                        settings_content
                            .agent
                            .get_or_insert_default()
                            .play_sound_when_agent_done = value;
                    },
                }),
                metadata: None,
                files: USER,
            }),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_AI_PREVENT_IDLE_SLEEP).into(),
                description: t(
                    keys::settings_ui::SETTINGS_AI_WHETHER_TO_KEEP_THE_SYSTEM_AWAKE_WHILE_A,
                )
                .into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("agent.prevent_idle_sleep"),
                    pick: |settings_content| {
                        settings_content.agent.as_ref()?.prevent_idle_sleep.as_ref()
                    },
                    write: |settings_content, value, _| {
                        settings_content
                            .agent
                            .get_or_insert_default()
                            .prevent_idle_sleep = value;
                    },
                }),
                metadata: None,
                files: USER,
            }),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_AI_MAX_IDLE_THREADS).into(),
                description: t(
                    keys::settings_ui::SETTINGS_AI_MAXIMUM_NUMBER_OF_IDLE_AGENT_THREADS_WIT,
                )
                .into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("agent.max_idle_retained_threads"),
                    pick: |settings_content| {
                        settings_content
                            .agent
                            .as_ref()?
                            .max_idle_retained_threads
                            .as_ref()
                    },
                    write: |settings_content, value, _| {
                        settings_content
                            .agent
                            .get_or_insert_default()
                            .max_idle_retained_threads = value;
                    },
                }),
                metadata: None,
                files: USER,
            }),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_AI_EXPAND_EDIT_CARD).into(),
                description: t(
                    keys::settings_ui::SETTINGS_AI_WHETHER_TO_HAVE_EDIT_CARDS_IN_THE_AGENT,
                )
                .into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("agent.expand_edit_card"),
                    pick: |settings_content| {
                        settings_content.agent.as_ref()?.expand_edit_card.as_ref()
                    },
                    write: |settings_content, value, _| {
                        settings_content
                            .agent
                            .get_or_insert_default()
                            .expand_edit_card = value;
                    },
                }),
                metadata: None,
                files: USER,
            }),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_AI_EXPAND_TERMINAL_CARD).into(),
                description: t(
                    keys::settings_ui::SETTINGS_AI_WHETHER_TO_HAVE_TERMINAL_CARDS_IN_THE_AG,
                )
                .into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("agent.expand_terminal_card"),
                    pick: |settings_content| {
                        settings_content
                            .agent
                            .as_ref()?
                            .expand_terminal_card
                            .as_ref()
                    },
                    write: |settings_content, value, _| {
                        settings_content
                            .agent
                            .get_or_insert_default()
                            .expand_terminal_card = value;
                    },
                }),
                metadata: None,
                files: USER,
            }),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_AI_TERMINAL_THREAD_INIT_COMMAND).into(),
                description: t(
                    keys::settings_ui::SETTINGS_AI_COMMAND_TO_AUTOMATICALLY_RUN_WHEN_ZED_CR,
                )
                .into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("agent.terminal_init_command"),
                    pick: |settings_content| {
                        settings_content
                            .agent
                            .as_ref()?
                            .terminal_init_command
                            .as_ref()
                    },
                    write: |settings_content, value, _| {
                        settings_content
                            .agent
                            .get_or_insert_default()
                            .terminal_init_command = value;
                    },
                }),
                metadata: Some(Box::new(SettingsFieldMetadata {
                    placeholder: Some(
                        t(keys::settings_ui::SETTINGS_AGENT_TERMINAL_INIT_EXAMPLE).into(),
                    ),
                    display_confirm_button: true,
                    display_clear_button: true,
                    confirm_on_focus_out: true,
                    treat_missing_text_as_empty: true,
                    ..Default::default()
                })),
                files: USER,
            }),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_AI_THINKING_DISPLAY).into(),
                description: t(
                    keys::settings_ui::SETTINGS_AI_HOW_THINKING_BLOCKS_SHOULD_BE_DISPLAYED,
                )
                .into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("agent.thinking_display"),
                    pick: |settings_content| {
                        settings_content.agent.as_ref()?.thinking_display.as_ref()
                    },
                    write: |settings_content, value, _| {
                        settings_content
                            .agent
                            .get_or_insert_default()
                            .thinking_display = value;
                    },
                }),
                metadata: None,
                files: USER,
            }),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_AI_CANCEL_GENERATION_ON_TERMINAL_STOP).into(),
                description: t(
                    keys::settings_ui::SETTINGS_AI_WHETHER_CLICKING_THE_STOP_BUTTON_ON_A_RU,
                )
                .into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("agent.cancel_generation_on_terminal_stop"),
                    pick: |settings_content| {
                        settings_content
                            .agent
                            .as_ref()?
                            .cancel_generation_on_terminal_stop
                            .as_ref()
                    },
                    write: |settings_content, value, _| {
                        settings_content
                            .agent
                            .get_or_insert_default()
                            .cancel_generation_on_terminal_stop = value;
                    },
                }),
                metadata: None,
                files: USER,
            }),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_AI_USE_MODIFIER_TO_SEND).into(),
                description: t(
                    keys::settings_ui::SETTINGS_AI_WHETHER_TO_ALWAYS_USE_CMD_ENTER_OR_CTRL,
                )
                .into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("agent.use_modifier_to_send"),
                    pick: |settings_content| {
                        settings_content
                            .agent
                            .as_ref()?
                            .use_modifier_to_send
                            .as_ref()
                    },
                    write: |settings_content, value, _| {
                        settings_content
                            .agent
                            .get_or_insert_default()
                            .use_modifier_to_send = value;
                    },
                }),
                metadata: None,
                files: USER,
            }),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_AI_MESSAGE_EDITOR_MIN_LINES).into(),
                description: t(
                    keys::settings_ui::SETTINGS_AI_MINIMUM_NUMBER_OF_LINES_TO_DISPLAY_IN_TH,
                )
                .into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("agent.message_editor_min_lines"),
                    pick: |settings_content| {
                        settings_content
                            .agent
                            .as_ref()?
                            .message_editor_min_lines
                            .as_ref()
                    },
                    write: |settings_content, value, _| {
                        settings_content
                            .agent
                            .get_or_insert_default()
                            .message_editor_min_lines = value;
                    },
                }),
                metadata: None,
                files: USER,
            }),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_AI_SHOW_TURN_STATS).into(),
                description: t(
                    keys::settings_ui::SETTINGS_AI_WHETHER_TO_SHOW_TURN_STATISTICS_LIKE_ELA,
                )
                .into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("agent.show_turn_stats"),
                    pick: |settings_content| {
                        settings_content.agent.as_ref()?.show_turn_stats.as_ref()
                    },
                    write: |settings_content, value, _| {
                        settings_content
                            .agent
                            .get_or_insert_default()
                            .show_turn_stats = value;
                    },
                }),
                metadata: None,
                files: USER,
            }),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_AI_SHOW_MERGE_CONFLICT_INDICATOR).into(),
                description: t(
                    keys::settings_ui::SETTINGS_AI_WHETHER_TO_SHOW_THE_MERGE_CONFLICT_INDIC,
                )
                .into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("agent.show_merge_conflict_indicator"),
                    pick: |settings_content| {
                        settings_content
                            .agent
                            .as_ref()?
                            .show_merge_conflict_indicator
                            .as_ref()
                    },
                    write: |settings_content, value, _| {
                        settings_content
                            .agent
                            .get_or_insert_default()
                            .show_merge_conflict_indicator = value;
                    },
                }),
                metadata: None,
                files: USER,
            }),
        ]);

        items.extend([
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_AI_AUTO_COMPACT).into(),
                description: t(keys::settings_ui::SETTINGS_AI_AUTOMATICALLY_COMPACT_THE_AGENT_S_CONTEX).into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("agent.auto_compact.enabled"),
                    pick: |settings_content| {
                        settings_content
                            .agent
                            .as_ref()?
                            .auto_compact
                            .as_ref()?
                            .enabled
                            .as_ref()
                    },
                    write: |settings_content, value, _| {
                        settings_content
                            .agent
                            .get_or_insert_default()
                            .auto_compact
                            .get_or_insert_default()
                            .enabled = value;
                    },
                }),
                metadata: None,
                files: USER,
            }),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_AI_AUTO_COMPACT_THRESHOLD).into(),
                description: "When auto compaction runs. A percentage string like \"90%\" is measured against the context window. A positive integer is the number of used tokens to compact after. A negative integer is the number of tokens remaining in the context window before compacting.".into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("agent.auto_compact.threshold"),
                    pick: |settings_content| {
                        settings_content
                            .agent
                            .as_ref()?
                            .auto_compact
                            .as_ref()?
                            .threshold
                            .as_ref()
                    },
                    write: |settings_content, value, _| {
                        settings_content
                            .agent
                            .get_or_insert_default()
                            .auto_compact
                            .get_or_insert_default()
                            .threshold = value;
                    },
                }),
                metadata: Some(Box::new(SettingsFieldMetadata {
                    placeholder: Some("90%".into()),
                    ..Default::default()
                })),
                files: USER,
            }),
        ]);

        items.into_boxed_slice()
    }

    fn edit_prediction_display_sub_section() -> [SettingsPageItem; 1] {
        [SettingsPageItem::SettingItem(SettingItem {
            title: t(keys::settings_ui::SETTINGS_AI_DISPLAY_MODE).into(),
            description: t(keys::settings_ui::SETTINGS_AI_WHEN_TO_SHOW_EDIT_PREDICTIONS_PREVIEWS_I)
                .into(),
            field: Box::new(SettingField {
                organization_override: None,
                json_path: Some("edit_prediction.display_mode"),
                pick: |settings_content| {
                    settings_content
                        .project
                        .all_languages
                        .edit_predictions
                        .as_ref()?
                        .mode
                        .as_ref()
                },
                write: |settings_content, value, _| {
                    settings_content
                        .project
                        .all_languages
                        .edit_predictions
                        .get_or_insert_default()
                        .mode = value;
                },
            }),
            metadata: None,
            files: USER,
        })]
    }

    SettingsPage {
        title: "AI".into(),
        items: concat_sections!(
            @vec,
            general_section(),
            agent_configuration_section(cx),
            edit_prediction_language_settings_section(),
            edit_prediction_display_sub_section(),
        )
        .into(),
    }
}

fn network_page() -> SettingsPage {
    fn network_section() -> [SettingsPageItem; 3] {
        [
            SettingsPageItem::SectionHeader(t(keys::collab::COLLAB_NETWORK).into()),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_NETWORK_PROXY).into(),
                description: t(
                    keys::settings_ui::SETTINGS_NETWORK_THE_PROXY_TO_USE_FOR_NETWORK_REQUESTS,
                )
                .into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("proxy"),
                    pick: |settings_content| settings_content.proxy.as_ref(),
                    write: |settings_content, value, _| {
                        settings_content.proxy = value;
                    },
                }),
                metadata: Some(Box::new(SettingsFieldMetadata {
                    placeholder: Some("socks5h://localhost:10808".into()),
                    ..Default::default()
                })),
                files: USER,
            }),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_NETWORK_SERVER_URL).into(),
                description: t(
                    keys::settings_ui::SETTINGS_NETWORK_THE_URL_OF_THE_ZED_SERVER_TO_CONNECT_TO,
                )
                .into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("server_url"),
                    pick: |settings_content| settings_content.server_url.as_ref(),
                    write: |settings_content, value, _| {
                        settings_content.server_url = value;
                    },
                }),
                metadata: Some(Box::new(SettingsFieldMetadata {
                    placeholder: Some("https://zed.dev".into()),
                    ..Default::default()
                })),
                files: USER,
            }),
        ]
    }

    SettingsPage {
        title: t(keys::collab::COLLAB_NETWORK).into(),
        items: concat_sections![network_section()],
    }
}

fn language_settings_field<T>(
    settings_content: &SettingsContent,
    get_language_setting_field: fn(&LanguageSettingsContent) -> Option<&T>,
) -> Option<&T> {
    let all_languages = &settings_content.project.all_languages;

    active_language()
        .and_then(|current_language_name| {
            all_languages
                .languages
                .0
                .get(current_language_name.as_ref())
        })
        .and_then(get_language_setting_field)
        .or_else(|| get_language_setting_field(&all_languages.defaults))
}

fn language_settings_field_mut<T>(
    settings_content: &mut SettingsContent,
    value: Option<T>,
    write: fn(&mut LanguageSettingsContent, Option<T>),
) {
    let all_languages = &mut settings_content.project.all_languages;
    let language_content = if let Some(current_language) = active_language() {
        all_languages
            .languages
            .0
            .entry(current_language.to_string())
            .or_default()
    } else {
        &mut all_languages.defaults
    };
    write(language_content, value);
}

fn language_settings_data() -> Box<[SettingsPageItem]> {
    fn indentation_section() -> [SettingsPageItem; 5] {
        [
            SettingsPageItem::SectionHeader(
                t(keys::settings_ui::SETTINGS_NETWORK_SECTION_INDENTATION).into(),
            ),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_NETWORK_TAB_SIZE).into(),
                description: t(
                    keys::settings_ui::SETTINGS_NETWORK_HOW_MANY_COLUMNS_A_TAB_SHOULD_OCCUPY,
                )
                .into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("languages.$(language).tab_size"), // TODO(cameron): not JQ syntax because not URL-safe
                    pick: |settings_content| {
                        language_settings_field(settings_content, |language| {
                            language.tab_size.as_ref()
                        })
                    },
                    write: |settings_content, value, _| {
                        language_settings_field_mut(settings_content, value, |language, value| {
                            language.tab_size = value;
                        })
                    },
                }),
                metadata: None,
                files: USER | PROJECT,
            }),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_NETWORK_HARD_TABS).into(),
                description: t(keys::settings_ui::SETTINGS_LANGUAGES_HARD_TABS_HINT).into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("languages.$(language).hard_tabs"),
                    pick: |settings_content| {
                        language_settings_field(settings_content, |language| {
                            language.hard_tabs.as_ref()
                        })
                    },
                    write: |settings_content, value, _| {
                        language_settings_field_mut(settings_content, value, |language, value| {
                            language.hard_tabs = value;
                        })
                    },
                }),
                metadata: None,
                files: USER | PROJECT,
            }),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_NETWORK_AUTO_INDENT).into(),
                description: t(
                    keys::settings_ui::SETTINGS_NETWORK_CONTROLS_AUTOMATIC_INDENTATION_BEHAVIOR,
                )
                .into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("languages.$(language).auto_indent"),
                    pick: |settings_content| {
                        language_settings_field(settings_content, |language| {
                            language.auto_indent.as_ref()
                        })
                    },
                    write: |settings_content, value, _| {
                        language_settings_field_mut(settings_content, value, |language, value| {
                            language.auto_indent = value;
                        })
                    },
                }),
                metadata: None,
                files: USER | PROJECT,
            }),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_NETWORK_AUTO_INDENT_ON_PASTE).into(),
                description: t(keys::settings_ui::SETTINGS_LANGUAGES_AUTO_INDENT_ON_PASTE_HINT)
                    .into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("languages.$(language).auto_indent_on_paste"),
                    pick: |settings_content| {
                        language_settings_field(settings_content, |language| {
                            language.auto_indent_on_paste.as_ref()
                        })
                    },
                    write: |settings_content, value, _| {
                        language_settings_field_mut(settings_content, value, |language, value| {
                            language.auto_indent_on_paste = value;
                        })
                    },
                }),
                metadata: None,
                files: USER | PROJECT,
            }),
        ]
    }

    fn wrapping_section() -> [SettingsPageItem; 7] {
        [
            SettingsPageItem::SectionHeader(
                t(keys::settings_ui::SETTINGS_NETWORK_SECTION_WRAPPING).into(),
            ),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_NETWORK_SOFT_WRAP).into(),
                description: t(
                    keys::settings_ui::SETTINGS_NETWORK_HOW_TO_SOFT_WRAP_LONG_LINES_OF_TEXT,
                )
                .into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("languages.$(language).soft_wrap"),
                    pick: |settings_content| {
                        language_settings_field(settings_content, |language| {
                            language.soft_wrap.as_ref()
                        })
                    },
                    write: |settings_content, value, _| {
                        language_settings_field_mut(settings_content, value, |language, value| {
                            language.soft_wrap = value;
                        })
                    },
                }),
                metadata: None,
                files: USER | PROJECT,
            }),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_NETWORK_SOFT_WRAP_INDENT).into(),
                description: t(
                    keys::settings_ui::SETTINGS_NETWORK_HOW_TO_INDENT_SOFT_WRAPPED_LINES,
                )
                .into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("languages.$(language).soft_wrap_indent"),
                    pick: |settings_content| {
                        language_settings_field(settings_content, |language| {
                            language.soft_wrap_indent.as_ref()
                        })
                    },
                    write: |settings_content, value, _| {
                        language_settings_field_mut(settings_content, value, |language, value| {
                            language.soft_wrap_indent = value;
                        })
                    },
                }),
                metadata: None,
                files: USER | PROJECT,
            }),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_APPEARANCE_SHOW_WRAP_GUIDES).into(),
                description: t(keys::settings_ui::SETTINGS_NETWORK_SHOW_WRAP_GUIDES_IN_THE_EDITOR)
                    .into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("languages.$(language).show_wrap_guides"),
                    pick: |settings_content| {
                        language_settings_field(settings_content, |language| {
                            language.show_wrap_guides.as_ref()
                        })
                    },
                    write: |settings_content, value, _| {
                        language_settings_field_mut(settings_content, value, |language, value| {
                            language.show_wrap_guides = value;
                        })
                    },
                }),
                metadata: None,
                files: USER | PROJECT,
            }),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_NETWORK_PREFERRED_LINE_LENGTH).into(),
                description: t(
                    keys::settings_ui::SETTINGS_NETWORK_THE_COLUMN_AT_WHICH_TO_SOFT_WRAP_LINES_F,
                )
                .into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("languages.$(language).preferred_line_length"),
                    pick: |settings_content| {
                        language_settings_field(settings_content, |language| {
                            language.preferred_line_length.as_ref()
                        })
                    },
                    write: |settings_content, value, _| {
                        language_settings_field_mut(settings_content, value, |language, value| {
                            language.preferred_line_length = value;
                        })
                    },
                }),
                metadata: None,
                files: USER | PROJECT,
            }),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_APPEARANCE_WRAP_GUIDES).into(),
                description: t(
                    keys::settings_ui::SETTINGS_NETWORK_CHARACTER_COUNTS_AT_WHICH_TO_SHOW_WRAP_G,
                )
                .into(),
                field: Box::new(
                    SettingField {
                        organization_override: None,
                        json_path: Some("languages.$(language).wrap_guides"),
                        pick: |settings_content| {
                            language_settings_field(settings_content, |language| {
                                language.wrap_guides.as_ref()
                            })
                        },
                        write: |settings_content, value, _| {
                            language_settings_field_mut(
                                settings_content,
                                value,
                                |language, value| {
                                    language.wrap_guides = value;
                                },
                            )
                        },
                    }
                    .unimplemented(),
                ),
                metadata: None,
                files: USER | PROJECT,
            }),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_NETWORK_ALLOW_REWRAP).into(),
                description: t(
                    keys::settings_ui::SETTINGS_NETWORK_CONTROLS_WHERE_THE_EDITOR_REWRAP_ACTION,
                )
                .into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("languages.$(language).allow_rewrap"),
                    pick: |settings_content| {
                        language_settings_field(settings_content, |language| {
                            language.allow_rewrap.as_ref()
                        })
                    },
                    write: |settings_content, value, _| {
                        language_settings_field_mut(settings_content, value, |language, value| {
                            language.allow_rewrap = value;
                        })
                    },
                }),
                metadata: None,
                files: USER | PROJECT,
            }),
        ]
    }

    fn indent_guides_section() -> [SettingsPageItem; 6] {
        [
            SettingsPageItem::SectionHeader(
                t(keys::settings_ui::SETTINGS_NETWORK_SECTION_INDENT_GUIDES).into(),
            ),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_EDITOR_ENABLED).into(),
                description: t(
                    keys::settings_ui::SETTINGS_NETWORK_DISPLAY_INDENT_GUIDES_IN_THE_EDITOR,
                )
                .into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("languages.$(language).indent_guides.enabled"),
                    pick: |settings_content| {
                        language_settings_field(settings_content, |language| {
                            language
                                .indent_guides
                                .as_ref()
                                .and_then(|indent_guides| indent_guides.enabled.as_ref())
                        })
                    },
                    write: |settings_content, value, _| {
                        language_settings_field_mut(settings_content, value, |language, value| {
                            language.indent_guides.get_or_insert_default().enabled = value;
                        })
                    },
                }),
                metadata: None,
                files: USER | PROJECT,
            }),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_NETWORK_LINE_WIDTH).into(),
                description: t(
                    keys::settings_ui::SETTINGS_NETWORK_THE_WIDTH_OF_THE_INDENT_GUIDES_IN_PIXELS,
                )
                .into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("languages.$(language).indent_guides.line_width"),
                    pick: |settings_content| {
                        language_settings_field(settings_content, |language| {
                            language
                                .indent_guides
                                .as_ref()
                                .and_then(|indent_guides| indent_guides.line_width.as_ref())
                        })
                    },
                    write: |settings_content, value, _| {
                        language_settings_field_mut(settings_content, value, |language, value| {
                            language.indent_guides.get_or_insert_default().line_width = value;
                        })
                    },
                }),
                metadata: None,
                files: USER | PROJECT,
            }),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_NETWORK_ACTIVE_LINE_WIDTH).into(),
                description: t(
                    keys::settings_ui::SETTINGS_NETWORK_THE_WIDTH_OF_THE_ACTIVE_INDENT_GUIDE_IN,
                )
                .into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("languages.$(language).indent_guides.active_line_width"),
                    pick: |settings_content| {
                        language_settings_field(settings_content, |language| {
                            language
                                .indent_guides
                                .as_ref()
                                .and_then(|indent_guides| indent_guides.active_line_width.as_ref())
                        })
                    },
                    write: |settings_content, value, _| {
                        language_settings_field_mut(settings_content, value, |language, value| {
                            language
                                .indent_guides
                                .get_or_insert_default()
                                .active_line_width = value;
                        })
                    },
                }),
                metadata: None,
                files: USER | PROJECT,
            }),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_NETWORK_COLORING).into(),
                description: t(
                    keys::settings_ui::SETTINGS_NETWORK_DETERMINES_HOW_INDENT_GUIDES_ARE_COLORED,
                )
                .into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("languages.$(language).indent_guides.coloring"),
                    pick: |settings_content| {
                        language_settings_field(settings_content, |language| {
                            language
                                .indent_guides
                                .as_ref()
                                .and_then(|indent_guides| indent_guides.coloring.as_ref())
                        })
                    },
                    write: |settings_content, value, _| {
                        language_settings_field_mut(settings_content, value, |language, value| {
                            language.indent_guides.get_or_insert_default().coloring = value;
                        })
                    },
                }),
                metadata: None,
                files: USER | PROJECT,
            }),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_NETWORK_BACKGROUND_COLORING).into(),
                description: t(
                    keys::settings_ui::SETTINGS_NETWORK_DETERMINES_HOW_INDENT_GUIDE_BACKGROUNDS,
                )
                .into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("languages.$(language).indent_guides.background_coloring"),
                    pick: |settings_content| {
                        language_settings_field(settings_content, |language| {
                            language.indent_guides.as_ref().and_then(|indent_guides| {
                                indent_guides.background_coloring.as_ref()
                            })
                        })
                    },
                    write: |settings_content, value, _| {
                        language_settings_field_mut(settings_content, value, |language, value| {
                            language
                                .indent_guides
                                .get_or_insert_default()
                                .background_coloring = value;
                        })
                    },
                }),
                metadata: None,
                files: USER | PROJECT,
            }),
        ]
    }

    fn formatting_section() -> [SettingsPageItem; 8] {
        [
            SettingsPageItem::SectionHeader(t(keys::settings_ui::SETTINGS_NETWORK_SECTION_FORMATTING).into()),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_NETWORK_FORMAT_ON_SAVE).into(),
                description: "On: format the whole buffer.\nOff: do not format.\nModifications: format only lines with unstaged changes; skips formatting when a git diff or LSP range formatting is unavailable.\nModifications If Available: same, but falls back to formatting the whole buffer.".into(),
                field: Box::new(
                    // TODO(settings_ui): this setting should just be a bool
                    SettingField {
                        organization_override: None,
                        json_path: Some("languages.$(language).format_on_save"),
                        pick: |settings_content| {
                            language_settings_field(settings_content, |language| {
                                language.format_on_save.as_ref()
                            })
                        },
                        write: |settings_content, value, _| {
                            language_settings_field_mut(
                                settings_content,
                                value,
                                |language, value| {
                                    language.format_on_save = value;
                                },
                            )
                        },
                    },
                ),
                metadata: None,
                files: USER | PROJECT,
            }),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_NETWORK_REMOVE_TRAILING_WHITESPACE_ON_SAVE).into(),
                description: t(keys::settings_ui::SETTINGS_NETWORK_WHETHER_OR_NOT_TO_REMOVE_ANY_TRAILING_WH).into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("languages.$(language).remove_trailing_whitespace_on_save"),
                    pick: |settings_content| {
                        language_settings_field(settings_content, |language| {
                            language.remove_trailing_whitespace_on_save.as_ref()
                        })
                    },
                    write: |settings_content, value, _| {
                        language_settings_field_mut(settings_content, value, |language, value| {
                            language.remove_trailing_whitespace_on_save = value;
                        })
                    },
                }),
                metadata: None,
                files: USER | PROJECT,
            }),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_NETWORK_ENSURE_FINAL_NEWLINE_ON_SAVE).into(),
                description: t(keys::settings_ui::SETTINGS_NETWORK_WHETHER_OR_NOT_TO_ENSURE_THERE_S_A_SINGL).into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("languages.$(language).ensure_final_newline_on_save"),
                    pick: |settings_content| {
                        language_settings_field(settings_content, |language| {
                            language.ensure_final_newline_on_save.as_ref()
                        })
                    },
                    write: |settings_content, value, _| {
                        language_settings_field_mut(settings_content, value, |language, value| {
                            language.ensure_final_newline_on_save = value;
                        })
                    },
                }),
                metadata: None,
                files: USER | PROJECT,
            }),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_NETWORK_LINE_ENDING).into(),
                description: t(keys::settings_ui::SETTINGS_NETWORK_HOW_LINE_ENDINGS_SHOULD_BE_HANDLED_FOR_N).into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("languages.$(language).line_ending"),
                    pick: |settings_content| {
                        language_settings_field(settings_content, |language| {
                            language.line_ending.as_ref()
                        })
                    },
                    write: |settings_content, value, _| {
                        language_settings_field_mut(settings_content, value, |language, value| {
                            language.line_ending = value;
                        })
                    },
                }),
                metadata: Some(Box::new(SettingsFieldMetadata {
                    should_do_titlecase: Some(false),
                    ..Default::default()
                })),
                files: USER | PROJECT,
            }),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_NETWORK_FORMATTER).into(),
                description: t(keys::settings_ui::SETTINGS_NETWORK_HOW_TO_PERFORM_A_BUFFER_FORMAT).into(),
                field: Box::new(
                    SettingField {
                        organization_override: None,
                        json_path: Some("languages.$(language).formatter"),
                        pick: |settings_content| {
                            language_settings_field(settings_content, |language| {
                                language.formatter.as_ref()
                            })
                        },
                        write: |settings_content, value, _| {
                            language_settings_field_mut(
                                settings_content,
                                value,
                                |language, value| {
                                    language.formatter = value;
                                },
                            )
                        },
                    }
                    .unimplemented(),
                ),
                metadata: None,
                files: USER | PROJECT,
            }),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_NETWORK_USE_ON_TYPE_FORMAT).into(),
                description: "Whether to use additional LSP queries to format (and amend) the code after every \"trigger\" symbol input, defined by LSP server capabilities".into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("languages.$(language).use_on_type_format"),
                    pick: |settings_content| {
                        language_settings_field(settings_content, |language| {
                            language.use_on_type_format.as_ref()
                        })
                    },
                    write: |settings_content, value, _| {
                        language_settings_field_mut(settings_content, value, |language, value| {
                            language.use_on_type_format = value;
                        })
                    },
                }),
                metadata: None,
                files: USER | PROJECT,
            }),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_NETWORK_CODE_ACTIONS_ON_FORMAT).into(),
                description: t(keys::settings_ui::SETTINGS_NETWORK_ADDITIONAL_CODE_ACTIONS_TO_RUN_WHEN_FORM).into(),
                field: Box::new(
                    SettingField {
                        organization_override: None,
                        json_path: Some("languages.$(language).code_actions_on_format"),
                        pick: |settings_content| {
                            language_settings_field(settings_content, |language| {
                                language.code_actions_on_format.as_ref()
                            })
                        },
                        write: |settings_content, value, _| {
                            language_settings_field_mut(
                                settings_content,
                                value,
                                |language, value| {
                                    language.code_actions_on_format = value;
                                },
                            )
                        },
                    }
                    .unimplemented(),
                ),
                metadata: None,
                files: USER | PROJECT,
            }),
        ]
    }

    fn autoclose_section() -> [SettingsPageItem; 5] {
        [
            SettingsPageItem::SectionHeader(
                t(keys::settings_ui::SETTINGS_NETWORK_SECTION_AUTOCLOSE).into(),
            ),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_NETWORK_USE_AUTOCLOSE).into(),
                description: t(
                    keys::settings_ui::SETTINGS_NETWORK_WHETHER_TO_AUTOMATICALLY_TYPE_CLOSING_CH,
                )
                .into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("languages.$(language).use_autoclose"),
                    pick: |settings_content| {
                        language_settings_field(settings_content, |language| {
                            language.use_autoclose.as_ref()
                        })
                    },
                    write: |settings_content, value, _| {
                        language_settings_field_mut(settings_content, value, |language, value| {
                            language.use_autoclose = value;
                        })
                    },
                }),
                metadata: None,
                files: USER | PROJECT,
            }),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_NETWORK_USE_AUTO_SURROUND).into(),
                description: t(
                    keys::settings_ui::SETTINGS_NETWORK_WHETHER_TO_AUTOMATICALLY_SURROUND_TEXT_W,
                )
                .into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("languages.$(language).use_auto_surround"),
                    pick: |settings_content| {
                        language_settings_field(settings_content, |language| {
                            language.use_auto_surround.as_ref()
                        })
                    },
                    write: |settings_content, value, _| {
                        language_settings_field_mut(settings_content, value, |language, value| {
                            language.use_auto_surround = value;
                        })
                    },
                }),
                metadata: None,
                files: USER | PROJECT,
            }),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_NETWORK_ALWAYS_TREAT_BRACKETS_AS_AUTOCLOSED)
                    .into(),
                description: t(
                    keys::settings_ui::SETTINGS_NETWORK_CONTROLS_WHETHER_THE_CLOSING_CHARACTERS,
                )
                .into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("languages.$(language).always_treat_brackets_as_autoclosed"),
                    pick: |settings_content| {
                        language_settings_field(settings_content, |language| {
                            language.always_treat_brackets_as_autoclosed.as_ref()
                        })
                    },
                    write: |settings_content, value, _| {
                        language_settings_field_mut(settings_content, value, |language, value| {
                            language.always_treat_brackets_as_autoclosed = value;
                        })
                    },
                }),
                metadata: None,
                files: USER | PROJECT,
            }),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_NETWORK_JSX_TAG_AUTO_CLOSE).into(),
                description: t(
                    keys::settings_ui::SETTINGS_NETWORK_WHETHER_TO_AUTOMATICALLY_CLOSE_JSX_TAGS,
                )
                .into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("languages.$(language).jsx_tag_auto_close"),
                    // TODO(settings_ui): this setting should just be a bool
                    pick: |settings_content| {
                        language_settings_field(settings_content, |language| {
                            language.jsx_tag_auto_close.as_ref()?.enabled.as_ref()
                        })
                    },
                    write: |settings_content, value, _| {
                        language_settings_field_mut(settings_content, value, |language, value| {
                            language.jsx_tag_auto_close.get_or_insert_default().enabled = value;
                        })
                    },
                }),
                metadata: None,
                files: USER | PROJECT,
            }),
        ]
    }

    fn whitespace_section() -> [SettingsPageItem; 4] {
        [
            SettingsPageItem::SectionHeader(t(keys::settings_ui::SETTINGS_NETWORK_SECTION_WHITESPACE).into()),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_NETWORK_SHOW_WHITESPACES).into(),
                description: t(keys::settings_ui::SETTINGS_NETWORK_WHETHER_TO_SHOW_TABS_AND_SPACES_IN_THE_E).into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("languages.$(language).show_whitespaces"),
                    pick: |settings_content| {
                        language_settings_field(settings_content, |language| {
                            language.show_whitespaces.as_ref()
                        })
                    },
                    write: |settings_content, value, _| {
                        language_settings_field_mut(settings_content, value, |language, value| {
                            language.show_whitespaces = value;
                        })
                    },
                }),
                metadata: None,
                files: USER | PROJECT,
            }),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_NETWORK_SPACE_WHITESPACE_INDICATOR).into(),
                description: "Visible character used to render space characters when show_whitespaces is enabled (default: \"•\")".into(),
                field: Box::new(
                    SettingField {
                        organization_override: None,
                        json_path: Some("languages.$(language).whitespace_map.space"),
                        pick: |settings_content| {
                            language_settings_field(settings_content, |language| {
                                language.whitespace_map.as_ref()?.space.as_ref()
                            })
                        },
                        write: |settings_content, value, _| {
                            language_settings_field_mut(
                                settings_content,
                                value,
                                |language, value| {
                                    language.whitespace_map.get_or_insert_default().space = value;
                                },
                            )
                        },
                    }
                    .unimplemented(),
                ),
                metadata: None,
                files: USER | PROJECT,
            }),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_NETWORK_TAB_WHITESPACE_INDICATOR).into(),
                description: "Visible character used to render tab characters when show_whitespaces is enabled (default: \"→\")".into(),
                field: Box::new(
                    SettingField {
                        organization_override: None,
                        json_path: Some("languages.$(language).whitespace_map.tab"),
                        pick: |settings_content| {
                            language_settings_field(settings_content, |language| {
                                language.whitespace_map.as_ref()?.tab.as_ref()
                            })
                        },
                        write: |settings_content, value, _| {
                            language_settings_field_mut(
                                settings_content,
                                value,
                                |language, value| {
                                    language.whitespace_map.get_or_insert_default().tab = value;
                                },
                            )
                        },
                    }
                    .unimplemented(),
                ),
                metadata: None,
                files: USER | PROJECT,
            }),
        ]
    }

    fn completions_section() -> [SettingsPageItem; 8] {
        [
            SettingsPageItem::SectionHeader(
                t(keys::settings_ui::SETTINGS_NETWORK_SECTION_COMPLETIONS).into(),
            ),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_NETWORK_SHOW_COMPLETIONS_ON_INPUT).into(),
                description: t(
                    keys::settings_ui::SETTINGS_NETWORK_WHETHER_TO_POP_THE_COMPLETIONS_MENU_WHIL,
                )
                .into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("languages.$(language).show_completions_on_input"),
                    pick: |settings_content| {
                        language_settings_field(settings_content, |language| {
                            language.show_completions_on_input.as_ref()
                        })
                    },
                    write: |settings_content, value, _| {
                        language_settings_field_mut(settings_content, value, |language, value| {
                            language.show_completions_on_input = value;
                        })
                    },
                }),
                metadata: None,
                files: USER | PROJECT,
            }),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_NETWORK_SHOW_COMPLETION_DOCUMENTATION).into(),
                description: t(
                    keys::settings_ui::SETTINGS_NETWORK_WHETHER_TO_DISPLAY_INLINE_AND_ALONGSIDE,
                )
                .into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("languages.$(language).show_completion_documentation"),
                    pick: |settings_content| {
                        language_settings_field(settings_content, |language| {
                            language.show_completion_documentation.as_ref()
                        })
                    },
                    write: |settings_content, value, _| {
                        language_settings_field_mut(settings_content, value, |language, value| {
                            language.show_completion_documentation = value;
                        })
                    },
                }),
                metadata: None,
                files: USER | PROJECT,
            }),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_NETWORK_WORDS).into(),
                description: t(
                    keys::settings_ui::SETTINGS_NETWORK_CONTROLS_HOW_WORDS_ARE_COMPLETED,
                )
                .into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("languages.$(language).completions.words"),
                    pick: |settings_content| {
                        language_settings_field(settings_content, |language| {
                            language.completions.as_ref()?.words.as_ref()
                        })
                    },
                    write: |settings_content, value, _| {
                        language_settings_field_mut(settings_content, value, |language, value| {
                            language.completions.get_or_insert_default().words = value;
                        })
                    },
                }),
                metadata: None,
                files: USER | PROJECT,
            }),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_NETWORK_WORDS_MIN_LENGTH).into(),
                description: t(
                    keys::settings_ui::SETTINGS_NETWORK_HOW_MANY_CHARACTERS_HAS_TO_BE_IN_THE_COM,
                )
                .into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("languages.$(language).completions.words_min_length"),
                    pick: |settings_content| {
                        language_settings_field(settings_content, |language| {
                            language.completions.as_ref()?.words_min_length.as_ref()
                        })
                    },
                    write: |settings_content, value, _| {
                        language_settings_field_mut(settings_content, value, |language, value| {
                            language
                                .completions
                                .get_or_insert_default()
                                .words_min_length = value;
                        })
                    },
                }),
                metadata: None,
                files: USER | PROJECT,
            }),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_NETWORK_COMPLETION_MENU_SCROLLBAR).into(),
                description: t(
                    keys::settings_ui::SETTINGS_NETWORK_WHEN_TO_SHOW_THE_SCROLLBAR_IN_THE_COMPLE,
                )
                .into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("editor.completion_menu_scrollbar"),
                    pick: |settings_content| {
                        settings_content.editor.completion_menu_scrollbar.as_ref()
                    },
                    write: |settings_content, value, _| {
                        settings_content.editor.completion_menu_scrollbar = value;
                    },
                }),
                metadata: None,
                files: USER,
            }),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_NETWORK_COMPLETION_DETAIL_ALIGNMENT).into(),
                description: t(
                    keys::settings_ui::SETTINGS_NETWORK_WHETHER_TO_ALIGN_DETAIL_TEXT_IN_CODE_COM,
                )
                .into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("editor.completion_detail_alignment"),
                    pick: |settings_content| {
                        settings_content.editor.completion_detail_alignment.as_ref()
                    },
                    write: |settings_content, value, _| {
                        settings_content.editor.completion_detail_alignment = value;
                    },
                }),
                metadata: None,
                files: USER,
            }),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_NETWORK_COMPLETION_MENU_ITEM_KIND).into(),
                description: t(
                    keys::settings_ui::SETTINGS_NETWORK_HOW_TO_DISPLAY_THE_LSP_ITEM_KIND_FUNCTIO,
                )
                .into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("editor.completion_menu_item_kind"),
                    pick: |settings_content| {
                        settings_content.editor.completion_menu_item_kind.as_ref()
                    },
                    write: |settings_content, value, _| {
                        settings_content.editor.completion_menu_item_kind = value;
                    },
                }),
                metadata: None,
                files: USER,
            }),
        ]
    }

    fn inlay_hints_section() -> [SettingsPageItem; 10] {
        [
            SettingsPageItem::SectionHeader(
                t(keys::settings_ui::SETTINGS_NETWORK_SECTION_INLAY_HINTS).into(),
            ),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_EDITOR_ENABLED).into(),
                description: t(
                    keys::settings_ui::SETTINGS_NETWORK_GLOBAL_SWITCH_TO_TOGGLE_HINTS_ON_AND_OFF,
                )
                .into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("languages.$(language).inlay_hints.enabled"),
                    pick: |settings_content| {
                        language_settings_field(settings_content, |language| {
                            language.inlay_hints.as_ref()?.enabled.as_ref()
                        })
                    },
                    write: |settings_content, value, _| {
                        language_settings_field_mut(settings_content, value, |language, value| {
                            language.inlay_hints.get_or_insert_default().enabled = value;
                        })
                    },
                }),
                metadata: None,
                files: USER | PROJECT,
            }),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_NETWORK_SHOW_VALUE_HINTS).into(),
                description: t(
                    keys::settings_ui::SETTINGS_NETWORK_GLOBAL_SWITCH_TO_TOGGLE_INLINE_VALUES_ON,
                )
                .into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("languages.$(language).inlay_hints.show_value_hints"),
                    pick: |settings_content| {
                        language_settings_field(settings_content, |language| {
                            language.inlay_hints.as_ref()?.show_value_hints.as_ref()
                        })
                    },
                    write: |settings_content, value, _| {
                        language_settings_field_mut(settings_content, value, |language, value| {
                            language
                                .inlay_hints
                                .get_or_insert_default()
                                .show_value_hints = value;
                        })
                    },
                }),
                metadata: None,
                files: USER | PROJECT,
            }),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_NETWORK_SHOW_TYPE_HINTS).into(),
                description: t(
                    keys::settings_ui::SETTINGS_NETWORK_WHETHER_TYPE_HINTS_SHOULD_BE_SHOWN,
                )
                .into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("languages.$(language).inlay_hints.show_type_hints"),
                    pick: |settings_content| {
                        language_settings_field(settings_content, |language| {
                            language.inlay_hints.as_ref()?.show_type_hints.as_ref()
                        })
                    },
                    write: |settings_content, value, _| {
                        language_settings_field_mut(settings_content, value, |language, value| {
                            language.inlay_hints.get_or_insert_default().show_type_hints = value;
                        })
                    },
                }),
                metadata: None,
                files: USER | PROJECT,
            }),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_NETWORK_SHOW_PARAMETER_HINTS).into(),
                description: t(
                    keys::settings_ui::SETTINGS_NETWORK_WHETHER_PARAMETER_HINTS_SHOULD_BE_SHOWN,
                )
                .into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("languages.$(language).inlay_hints.show_parameter_hints"),
                    pick: |settings_content| {
                        language_settings_field(settings_content, |language| {
                            language.inlay_hints.as_ref()?.show_parameter_hints.as_ref()
                        })
                    },
                    write: |settings_content, value, _| {
                        language_settings_field_mut(settings_content, value, |language, value| {
                            language
                                .inlay_hints
                                .get_or_insert_default()
                                .show_parameter_hints = value;
                        })
                    },
                }),
                metadata: None,
                files: USER | PROJECT,
            }),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_NETWORK_SHOW_OTHER_HINTS).into(),
                description: t(
                    keys::settings_ui::SETTINGS_NETWORK_WHETHER_OTHER_HINTS_SHOULD_BE_SHOWN,
                )
                .into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("languages.$(language).inlay_hints.show_other_hints"),
                    pick: |settings_content| {
                        language_settings_field(settings_content, |language| {
                            language.inlay_hints.as_ref()?.show_other_hints.as_ref()
                        })
                    },
                    write: |settings_content, value, _| {
                        language_settings_field_mut(settings_content, value, |language, value| {
                            language
                                .inlay_hints
                                .get_or_insert_default()
                                .show_other_hints = value;
                        })
                    },
                }),
                metadata: None,
                files: USER | PROJECT,
            }),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_NETWORK_SHOW_BACKGROUND).into(),
                description: t(
                    keys::settings_ui::SETTINGS_NETWORK_SHOW_A_BACKGROUND_FOR_INLAY_HINTS,
                )
                .into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("languages.$(language).inlay_hints.show_background"),
                    pick: |settings_content| {
                        language_settings_field(settings_content, |language| {
                            language.inlay_hints.as_ref()?.show_background.as_ref()
                        })
                    },
                    write: |settings_content, value, _| {
                        language_settings_field_mut(settings_content, value, |language, value| {
                            language.inlay_hints.get_or_insert_default().show_background = value;
                        })
                    },
                }),
                metadata: None,
                files: USER | PROJECT,
            }),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_NETWORK_EDIT_DEBOUNCE_MS).into(),
                description: t(
                    keys::settings_ui::SETTINGS_NETWORK_WHETHER_OR_NOT_TO_DEBOUNCE_INLAY_HINTS_U,
                )
                .into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("languages.$(language).inlay_hints.edit_debounce_ms"),
                    pick: |settings_content| {
                        language_settings_field(settings_content, |language| {
                            language.inlay_hints.as_ref()?.edit_debounce_ms.as_ref()
                        })
                    },
                    write: |settings_content, value, _| {
                        language_settings_field_mut(settings_content, value, |language, value| {
                            language
                                .inlay_hints
                                .get_or_insert_default()
                                .edit_debounce_ms = value;
                        })
                    },
                }),
                metadata: None,
                files: USER | PROJECT,
            }),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_NETWORK_SCROLL_DEBOUNCE_MS).into(),
                description: t(
                    keys::settings_ui::SETTINGS_NETWORK_WHETHER_OR_NOT_TO_DEBOUNCE_INLAY_HINTS_U_2,
                )
                .into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("languages.$(language).inlay_hints.scroll_debounce_ms"),
                    pick: |settings_content| {
                        language_settings_field(settings_content, |language| {
                            language.inlay_hints.as_ref()?.scroll_debounce_ms.as_ref()
                        })
                    },
                    write: |settings_content, value, _| {
                        language_settings_field_mut(settings_content, value, |language, value| {
                            language
                                .inlay_hints
                                .get_or_insert_default()
                                .scroll_debounce_ms = value;
                        })
                    },
                }),
                metadata: None,
                files: USER | PROJECT,
            }),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_NETWORK_TOGGLE_ON_MODIFIERS_PRESS).into(),
                description: t(
                    keys::settings_ui::SETTINGS_NETWORK_TOGGLES_INLAY_HINTS_HIDES_OR_SHOWS_WHEN,
                )
                .into(),
                field: Box::new(
                    SettingField {
                        organization_override: None,
                        json_path: Some(
                            "languages.$(language).inlay_hints.toggle_on_modifiers_press",
                        ),
                        pick: |settings_content| {
                            language_settings_field(settings_content, |language| {
                                language
                                    .inlay_hints
                                    .as_ref()?
                                    .toggle_on_modifiers_press
                                    .as_ref()
                            })
                        },
                        write: |settings_content, value, _| {
                            language_settings_field_mut(
                                settings_content,
                                value,
                                |language, value| {
                                    language
                                        .inlay_hints
                                        .get_or_insert_default()
                                        .toggle_on_modifiers_press = value;
                                },
                            )
                        },
                    }
                    .unimplemented(),
                ),
                metadata: None,
                files: USER | PROJECT,
            }),
        ]
    }

    fn tasks_section() -> [SettingsPageItem; 4] {
        [
            SettingsPageItem::SectionHeader(
                t(keys::settings_ui::SETTINGS_NETWORK_SECTION_TASKS).into(),
            ),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_EDITOR_ENABLED).into(),
                description: t(
                    keys::settings_ui::SETTINGS_NETWORK_WHETHER_TASKS_ARE_ENABLED_FOR_THIS_LANGU,
                )
                .into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("languages.$(language).tasks.enabled"),
                    pick: |settings_content| {
                        language_settings_field(settings_content, |language| {
                            language.tasks.as_ref()?.enabled.as_ref()
                        })
                    },
                    write: |settings_content, value, _| {
                        language_settings_field_mut(settings_content, value, |language, value| {
                            language.tasks.get_or_insert_default().enabled = value;
                        })
                    },
                }),
                metadata: None,
                files: USER | PROJECT,
            }),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_NETWORK_VARIABLES).into(),
                description: t(
                    keys::settings_ui::SETTINGS_NETWORK_EXTRA_TASK_VARIABLES_TO_SET_FOR_A_PARTIC,
                )
                .into(),
                field: Box::new(
                    SettingField {
                        organization_override: None,
                        json_path: Some("languages.$(language).tasks.variables"),
                        pick: |settings_content| {
                            language_settings_field(settings_content, |language| {
                                language.tasks.as_ref()?.variables.as_ref()
                            })
                        },
                        write: |settings_content, value, _| {
                            language_settings_field_mut(
                                settings_content,
                                value,
                                |language, value| {
                                    language.tasks.get_or_insert_default().variables = value;
                                },
                            )
                        },
                    }
                    .unimplemented(),
                ),
                metadata: None,
                files: USER | PROJECT,
            }),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_NETWORK_PREFER_LSP).into(),
                description: t(
                    keys::settings_ui::SETTINGS_NETWORK_USE_LSP_TASKS_OVER_ZED_LANGUAGE_EXTENSIO,
                )
                .into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("languages.$(language).tasks.prefer_lsp"),
                    pick: |settings_content| {
                        language_settings_field(settings_content, |language| {
                            language.tasks.as_ref()?.prefer_lsp.as_ref()
                        })
                    },
                    write: |settings_content, value, _| {
                        language_settings_field_mut(settings_content, value, |language, value| {
                            language.tasks.get_or_insert_default().prefer_lsp = value;
                        })
                    },
                }),
                metadata: None,
                files: USER | PROJECT,
            }),
        ]
    }

    fn miscellaneous_section() -> [SettingsPageItem; 8] {
        [
            SettingsPageItem::SectionHeader(
                t(keys::settings_ui::SETTINGS_NETWORK_SECTION_MISCELLANEOUS).into(),
            ),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_NETWORK_LANGUAGE_DETECTION).into(),
                description: t(
                    keys::settings_ui::SETTINGS_NETWORK_WHETHER_TO_ENABLE_AUTOMATIC_LANGUAGE_DET,
                )
                .into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("language_detection"),
                    pick: |settings_content| settings_content.editor.language_detection.as_ref(),
                    write: |settings_content, value, _| {
                        settings_content.editor.language_detection = value;
                    },
                }),
                metadata: None,
                files: USER,
            }),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_NETWORK_WORD_DIFF_ENABLED).into(),
                description: t(
                    keys::settings_ui::SETTINGS_NETWORK_WHETHER_TO_ENABLE_WORD_DIFF_HIGHLIGHTING,
                )
                .into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("languages.$(language).word_diff_enabled"),
                    pick: |settings_content| {
                        language_settings_field(settings_content, |language| {
                            language.word_diff_enabled.as_ref()
                        })
                    },
                    write: |settings_content, value, _| {
                        language_settings_field_mut(settings_content, value, |language, value| {
                            language.word_diff_enabled = value;
                        })
                    },
                }),
                metadata: None,
                files: USER | PROJECT,
            }),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_NETWORK_DEBUGGERS).into(),
                description: t(
                    keys::settings_ui::SETTINGS_NETWORK_PREFERRED_DEBUGGERS_FOR_THIS_LANGUAGE,
                )
                .into(),
                field: Box::new(
                    SettingField {
                        organization_override: None,
                        json_path: Some("languages.$(language).debuggers"),
                        pick: |settings_content| {
                            language_settings_field(settings_content, |language| {
                                language.debuggers.as_ref()
                            })
                        },
                        write: |settings_content, value, _| {
                            language_settings_field_mut(
                                settings_content,
                                value,
                                |language, value| {
                                    language.debuggers = value;
                                },
                            )
                        },
                    }
                    .unimplemented(),
                ),
                metadata: None,
                files: USER | PROJECT,
            }),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_NETWORK_MIDDLE_CLICK_PASTE).into(),
                description: t(
                    keys::settings_ui::SETTINGS_NETWORK_ENABLE_MIDDLE_CLICK_PASTE_ON_LINUX,
                )
                .into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("languages.$(language).editor.middle_click_paste"),
                    pick: |settings_content| settings_content.editor.middle_click_paste.as_ref(),
                    write: |settings_content, value, _| {
                        settings_content.editor.middle_click_paste = value;
                    },
                }),
                metadata: None,
                files: USER,
            }),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_NETWORK_EXTEND_COMMENT_ON_NEWLINE).into(),
                description: t(
                    keys::settings_ui::SETTINGS_NETWORK_WHETHER_TO_START_A_NEW_LINE_WITH_A_COMME,
                )
                .into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("languages.$(language).extend_comment_on_newline"),
                    pick: |settings_content| {
                        language_settings_field(settings_content, |language| {
                            language.extend_comment_on_newline.as_ref()
                        })
                    },
                    write: |settings_content, value, _| {
                        language_settings_field_mut(settings_content, value, |language, value| {
                            language.extend_comment_on_newline = value;
                        })
                    },
                }),
                metadata: None,
                files: USER | PROJECT,
            }),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_NETWORK_COLORIZE_BRACKETS).into(),
                description: t(
                    keys::settings_ui::SETTINGS_NETWORK_WHETHER_TO_COLORIZE_BRACKETS_IN_THE_EDIT,
                )
                .into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("languages.$(language).colorize_brackets"),
                    pick: |settings_content| {
                        language_settings_field(settings_content, |language| {
                            language.colorize_brackets.as_ref()
                        })
                    },
                    write: |settings_content, value, _| {
                        language_settings_field_mut(settings_content, value, |language, value| {
                            language.colorize_brackets = value;
                        })
                    },
                }),
                metadata: None,
                files: USER | PROJECT,
            }),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_NETWORK_VIM_EMACS_MODELINE_SUPPORT).into(),
                description: t(
                    keys::settings_ui::SETTINGS_NETWORK_NUMBER_OF_LINES_TO_SEARCH_FOR_MODELINES,
                )
                .into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("modeline_lines"),
                    pick: |settings_content| settings_content.modeline_lines.as_ref(),
                    write: |settings_content, value, _| {
                        settings_content.modeline_lines = value;
                    },
                }),
                metadata: None,
                files: USER | PROJECT,
            }),
        ]
    }

    fn global_only_miscellaneous_sub_section() -> [SettingsPageItem; 4] {
        [
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_NETWORK_IMAGE_VIEWER).into(),
                description: t(keys::settings_ui::SETTINGS_NETWORK_THE_UNIT_FOR_IMAGE_FILE_SIZES).into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("image_viewer.unit"),
                    pick: |settings_content| {
                        settings_content
                            .image_viewer
                            .as_ref()
                            .and_then(|image_viewer| image_viewer.unit.as_ref())
                    },
                    write: |settings_content, value, _| {
                        settings_content.image_viewer.get_or_insert_default().unit = value;
                    },
                }),
                metadata: None,
                files: USER,
            }),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_NETWORK_OPEN_MARKDOWN_FILES_IN_PREVIEW).into(),
                description: t(keys::settings_ui::SETTINGS_NETWORK_WHETHER_TO_AUTOMATICALLY_OPEN_MARKDOWN_F).into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("markdown_preview.open_markdown_files_in_preview"),
                    pick: |settings_content| {
                        settings_content
                            .markdown_preview
                            .as_ref()?
                            .open_markdown_files_in_preview
                            .as_ref()
                    },
                    write: |settings_content, value, _| {
                        settings_content
                            .markdown_preview
                            .get_or_insert_default()
                            .open_markdown_files_in_preview = value;
                    },
                }),
                metadata: None,
                files: USER,
            }),
            SettingsPageItem::DynamicItem(DynamicItem {
                discriminant: SettingItem {
                    files: USER,
                    title: t(keys::settings_ui::SETTINGS_NETWORK_LIMIT_MARKDOWN_PREVIEW_WIDTH).into(),
                    description: t(keys::settings_ui::SETTINGS_NETWORK_WHETHER_TO_CONSTRAIN_THE_MARKDOWN_PREVIE).into(),
                    field: Box::new(SettingField::<bool> {
                        organization_override: None,
                        json_path: Some("markdown_preview.limit_content_width"),
                        pick: |settings_content| {
                            settings_content
                                .markdown_preview
                                .as_ref()?
                                .limit_content_width
                                .as_ref()
                        },
                        write: |settings_content, value, _| {
                            settings_content
                                .markdown_preview
                                .get_or_insert_default()
                                .limit_content_width = value;
                        },
                    }),
                    metadata: None,
                },
                pick_discriminant: |settings_content| {
                    let enabled = settings_content
                        .markdown_preview
                        .as_ref()?
                        .limit_content_width
                        .unwrap_or(true);
                    Some(if enabled { 1 } else { 0 })
                },
                fields: vec![
                    vec![],
                    vec![SettingItem {
                        files: USER,
                        title: t(keys::settings_ui::SETTINGS_NETWORK_MAX_WIDTH).into(),
                        description: t(keys::settings_ui::SETTINGS_NETWORK_MAXIMUM_CONTENT_WIDTH_IN_PIXELS_CONTENT).into(),
                        field: Box::new(SettingField {
                            organization_override: None,
                            json_path: Some("markdown_preview.max_width"),
                            pick: |settings_content| {
                                settings_content
                                    .markdown_preview
                                    .as_ref()?
                                    .max_width
                                    .as_ref()
                            },
                            write: |settings_content, value, _| {
                                settings_content
                                    .markdown_preview
                                    .get_or_insert_default()
                                    .max_width = value;
                            },
                        }),
                        metadata: None,
                    }],
                ],
            }),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_NETWORK_DROP_SIZE_TARGET).into(),
                description: t(keys::settings_ui::SETTINGS_NETWORK_RELATIVE_SIZE_OF_THE_DROP_TARGET_IN_THE).into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("drop_target_size"),
                    pick: |settings_content| settings_content.workspace.drop_target_size.as_ref(),
                    write: |settings_content, value, _| {
                        settings_content.workspace.drop_target_size = value;
                    },
                }),
                metadata: None,
                files: USER,
            }),
        ]
    }

    let is_global = active_language().is_none();

    let code_lens_item = [SettingsPageItem::SettingItem(SettingItem {
        title: t(keys::settings_ui::SETTINGS_NETWORK_CODE_LENS).into(),
        description: t(
            keys::settings_ui::SETTINGS_NETWORK_WHETHER_AND_HOW_TO_DISPLAY_CODE_LENSES_F,
        )
        .into(),
        field: Box::new(SettingField {
            organization_override: None,
            json_path: Some("code_lens"),
            pick: |settings_content| settings_content.editor.code_lens.as_ref(),
            write: |settings_content, value, _| {
                settings_content.editor.code_lens = value;
            },
        }),
        metadata: None,
        files: USER,
    })];

    let lsp_document_colors_item = [SettingsPageItem::SettingItem(SettingItem {
        title: t(keys::settings_ui::SETTINGS_NETWORK_LSP_DOCUMENT_COLORS).into(),
        description: t(keys::settings_ui::SETTINGS_NETWORK_HOW_TO_RENDER_LSP_COLOR_PREVIEWS_IN_THE)
            .into(),
        field: Box::new(SettingField {
            organization_override: None,
            json_path: Some("lsp_document_colors"),
            pick: |settings_content| settings_content.editor.lsp_document_colors.as_ref(),
            write: |settings_content, value, _| {
                settings_content.editor.lsp_document_colors = value;
            },
        }),
        metadata: None,
        files: USER,
    })];

    if is_global {
        concat_sections!(
            indentation_section(),
            wrapping_section(),
            indent_guides_section(),
            formatting_section(),
            autoclose_section(),
            whitespace_section(),
            completions_section(),
            inlay_hints_section(),
            code_lens_item,
            lsp_document_colors_item,
            tasks_section(),
            miscellaneous_section(),
            global_only_miscellaneous_sub_section(),
        )
    } else {
        concat_sections!(
            indentation_section(),
            wrapping_section(),
            indent_guides_section(),
            formatting_section(),
            autoclose_section(),
            whitespace_section(),
            completions_section(),
            inlay_hints_section(),
            code_lens_item,
            tasks_section(),
            miscellaneous_section(),
        )
    }
}

/// LanguageSettings items that should be included in the "Languages & Tools" page
/// not the "Editor" page
fn non_editor_language_settings_data() -> Box<[SettingsPageItem]> {
    fn lsp_section() -> [SettingsPageItem; 10] {
        [
            SettingsPageItem::SectionHeader("LSP".into()),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_NETWORK_ENABLE_LANGUAGE_SERVER).into(),
                description: t(
                    keys::settings_ui::SETTINGS_NETWORK_WHETHER_TO_USE_LANGUAGE_SERVERS_TO_PROVI,
                )
                .into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("languages.$(language).enable_language_server"),
                    pick: |settings_content| {
                        language_settings_field(settings_content, |language| {
                            language.enable_language_server.as_ref()
                        })
                    },
                    write: |settings_content, value, _| {
                        language_settings_field_mut(settings_content, value, |language, value| {
                            language.enable_language_server = value;
                        })
                    },
                }),
                metadata: None,
                files: USER | PROJECT,
            }),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::extensions::EXTENSIONS_PROVIDES_LANGUAGE_SERVERS).into(),
                description: t(
                    keys::settings_ui::SETTINGS_NETWORK_THE_LIST_OF_LANGUAGE_SERVERS_TO_USE_OR_D,
                )
                .into(),
                field: Box::new(
                    SettingField {
                        organization_override: None,
                        json_path: Some("languages.$(language).language_servers"),
                        pick: |settings_content| {
                            language_settings_field(settings_content, |language| {
                                language.language_servers.as_ref()
                            })
                        },
                        write: |settings_content, value, _| {
                            language_settings_field_mut(
                                settings_content,
                                value,
                                |language, value| {
                                    language.language_servers = value;
                                },
                            )
                        },
                    }
                    .unimplemented(),
                ),
                metadata: None,
                files: USER | PROJECT,
            }),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_NETWORK_LINKED_EDITS).into(),
                description: t(
                    keys::settings_ui::SETTINGS_NETWORK_WHETHER_TO_PERFORM_LINKED_EDITS_OF_ASSOC,
                )
                .into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("languages.$(language).linked_edits"),
                    pick: |settings_content| {
                        language_settings_field(settings_content, |language| {
                            language.linked_edits.as_ref()
                        })
                    },
                    write: |settings_content, value, _| {
                        language_settings_field_mut(settings_content, value, |language, value| {
                            language.linked_edits = value;
                        })
                    },
                }),
                metadata: None,
                files: USER | PROJECT,
            }),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_NETWORK_GO_TO_DEFINITION_FALLBACK).into(),
                description: t(
                    keys::settings_ui::SETTINGS_NETWORK_WHETHER_TO_FOLLOW_UP_EMPTY_GO_TO_DEFINIT,
                )
                .into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("go_to_definition_fallback"),
                    pick: |settings_content| {
                        settings_content.editor.go_to_definition_fallback.as_ref()
                    },
                    write: |settings_content, value, _| {
                        settings_content.editor.go_to_definition_fallback = value;
                    },
                }),
                metadata: None,
                files: USER,
            }),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_NETWORK_GO_TO_DEFINITION_SCROLL_STRATEGY)
                    .into(),
                description: t(
                    keys::settings_ui::SETTINGS_NETWORK_HOW_TO_SCROLL_THE_TARGET_INTO_VIEW_WHEN,
                )
                .into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("go_to_definition_scroll_strategy"),
                    pick: |settings_content| {
                        settings_content
                            .editor
                            .go_to_definition_scroll_strategy
                            .as_ref()
                    },
                    write: |settings_content, value, _| {
                        settings_content.editor.go_to_definition_scroll_strategy = value;
                    },
                }),
                metadata: None,
                files: USER,
            }),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_NETWORK_LSP_RESULTS_LOCATION).into(),
                description: t(
                    keys::settings_ui::SETTINGS_NETWORK_WHERE_TO_SHOW_LSP_RESULTS_THAT_CAN_CONTA,
                )
                .into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("lsp_results_location"),
                    pick: |settings_content| settings_content.editor.lsp_results_location.as_ref(),
                    write: |settings_content, value, _| {
                        settings_content.editor.lsp_results_location = value;
                    },
                }),
                metadata: None,
                files: USER,
            }),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::lang::LANG_TOOLS_SEMANTIC_TOKENS).into(),
                description: {
                    static DESCRIPTION: OnceLock<SharedString> = OnceLock::new();
                    DESCRIPTION
                        .get_or_init(|| {
                            SemanticTokens::VARIANTS
                                .iter()
                                .filter_map(|v| {
                                    v.get_documentation().map(|doc| format!("{v:?}: {doc}"))
                                })
                                .join("\n")
                                .into()
                        })
                        .clone()
                },
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("languages.$(language).semantic_tokens"),
                    pick: |settings_content| {
                        settings_content
                            .project
                            .all_languages
                            .defaults
                            .semantic_tokens
                            .as_ref()
                    },
                    write: |settings_content, value, _| {
                        settings_content
                            .project
                            .all_languages
                            .defaults
                            .semantic_tokens = value;
                    },
                }),
                metadata: None,
                files: USER | PROJECT,
            }),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_NETWORK_LSP_FOLDING_RANGES).into(),
                description: t(
                    keys::settings_ui::SETTINGS_NETWORK_WHEN_ENABLED_USE_FOLDING_RANGES_FROM_THE,
                )
                .into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("languages.$(language).document_folding_ranges"),
                    pick: |settings_content| {
                        language_settings_field(settings_content, |language| {
                            language.document_folding_ranges.as_ref()
                        })
                    },
                    write: |settings_content, value, _| {
                        language_settings_field_mut(settings_content, value, |language, value| {
                            language.document_folding_ranges = value;
                        })
                    },
                }),
                metadata: None,
                files: USER | PROJECT,
            }),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_NETWORK_LSP_DOCUMENT_SYMBOLS).into(),
                description: t(
                    keys::settings_ui::SETTINGS_NETWORK_WHEN_ENABLED_USE_THE_LANGUAGE_SERVER_S_D,
                )
                .into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("languages.$(language).document_symbols"),
                    pick: |settings_content| {
                        language_settings_field(settings_content, |language| {
                            language.document_symbols.as_ref()
                        })
                    },
                    write: |settings_content, value, _| {
                        language_settings_field_mut(settings_content, value, |language, value| {
                            language.document_symbols = value;
                        })
                    },
                }),
                metadata: None,
                files: USER | PROJECT,
            }),
        ]
    }

    fn lsp_completions_section() -> [SettingsPageItem; 4] {
        [
            SettingsPageItem::SectionHeader(
                t(keys::settings_ui::SETTINGS_NETWORK_SECTION_LSP_COMPLETIONS).into(),
            ),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_EDITOR_ENABLED).into(),
                description: t(
                    keys::settings_ui::SETTINGS_NETWORK_WHETHER_TO_FETCH_LSP_COMPLETIONS_OR_NOT,
                )
                .into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("languages.$(language).completions.lsp"),
                    pick: |settings_content| {
                        language_settings_field(settings_content, |language| {
                            language.completions.as_ref()?.lsp.as_ref()
                        })
                    },
                    write: |settings_content, value, _| {
                        language_settings_field_mut(settings_content, value, |language, value| {
                            language.completions.get_or_insert_default().lsp = value;
                        })
                    },
                }),
                metadata: None,
                files: USER | PROJECT,
            }),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_NETWORK_FETCH_TIMEOUT_MILLISECONDS).into(),
                description: t(
                    keys::settings_ui::SETTINGS_NETWORK_WHEN_FETCHING_LSP_COMPLETIONS_DETERMINES,
                )
                .into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("languages.$(language).completions.lsp_fetch_timeout_ms"),
                    pick: |settings_content| {
                        language_settings_field(settings_content, |language| {
                            language.completions.as_ref()?.lsp_fetch_timeout_ms.as_ref()
                        })
                    },
                    write: |settings_content, value, _| {
                        language_settings_field_mut(settings_content, value, |language, value| {
                            language
                                .completions
                                .get_or_insert_default()
                                .lsp_fetch_timeout_ms = value;
                        })
                    },
                }),
                metadata: None,
                files: USER | PROJECT,
            }),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_NETWORK_INSERT_MODE).into(),
                description: t(
                    keys::settings_ui::SETTINGS_NETWORK_CONTROLS_HOW_LSP_COMPLETIONS_ARE_INSERTE,
                )
                .into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("languages.$(language).completions.lsp_insert_mode"),
                    pick: |settings_content| {
                        language_settings_field(settings_content, |language| {
                            language.completions.as_ref()?.lsp_insert_mode.as_ref()
                        })
                    },
                    write: |settings_content, value, _| {
                        language_settings_field_mut(settings_content, value, |language, value| {
                            language.completions.get_or_insert_default().lsp_insert_mode = value;
                        })
                    },
                }),
                metadata: None,
                files: USER | PROJECT,
            }),
        ]
    }

    fn debugger_section() -> [SettingsPageItem; 2] {
        [
            SettingsPageItem::SectionHeader(
                t(keys::settings_ui::SETTINGS_NETWORK_DEBUGGERS).into(),
            ),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_NETWORK_DEBUGGERS).into(),
                description: t(
                    keys::settings_ui::SETTINGS_NETWORK_PREFERRED_DEBUGGERS_FOR_THIS_LANGUAGE,
                )
                .into(),
                field: Box::new(
                    SettingField {
                        organization_override: None,
                        json_path: Some("languages.$(language).debuggers"),
                        pick: |settings_content| {
                            language_settings_field(settings_content, |language| {
                                language.debuggers.as_ref()
                            })
                        },
                        write: |settings_content, value, _| {
                            language_settings_field_mut(
                                settings_content,
                                value,
                                |language, value| {
                                    language.debuggers = value;
                                },
                            )
                        },
                    }
                    .unimplemented(),
                ),
                metadata: None,
                files: USER | PROJECT,
            }),
        ]
    }

    fn prettier_section() -> [SettingsPageItem; 5] {
        [
            SettingsPageItem::SectionHeader("Prettier".into()),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_NETWORK_ALLOWED).into(),
                description: t(
                    keys::settings_ui::SETTINGS_NETWORK_ENABLES_OR_DISABLES_FORMATTING_WITH_PRET,
                )
                .into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("languages.$(language).prettier.allowed"),
                    pick: |settings_content| {
                        language_settings_field(settings_content, |language| {
                            language.prettier.as_ref()?.allowed.as_ref()
                        })
                    },
                    write: |settings_content, value, _| {
                        language_settings_field_mut(settings_content, value, |language, value| {
                            language.prettier.get_or_insert_default().allowed = value;
                        })
                    },
                }),
                metadata: None,
                files: USER | PROJECT,
            }),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_NETWORK_PARSER).into(),
                description: t(
                    keys::settings_ui::SETTINGS_NETWORK_FORCES_PRETTIER_INTEGRATION_TO_USE_A_SPE,
                )
                .into(),
                field: Box::new(SettingField {
                    organization_override: None,
                    json_path: Some("languages.$(language).prettier.parser"),
                    pick: |settings_content| {
                        language_settings_field(settings_content, |language| {
                            language.prettier.as_ref()?.parser.as_ref()
                        })
                    },
                    write: |settings_content, value, _| {
                        language_settings_field_mut(settings_content, value, |language, value| {
                            language.prettier.get_or_insert_default().parser = value;
                        })
                    },
                }),
                metadata: None,
                files: USER | PROJECT,
            }),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_NETWORK_PLUGINS).into(),
                description: t(
                    keys::settings_ui::SETTINGS_NETWORK_FORCES_PRETTIER_INTEGRATION_TO_USE_SPECI,
                )
                .into(),
                field: Box::new(
                    SettingField {
                        organization_override: None,
                        json_path: Some("languages.$(language).prettier.plugins"),
                        pick: |settings_content| {
                            language_settings_field(settings_content, |language| {
                                language.prettier.as_ref()?.plugins.as_ref()
                            })
                        },
                        write: |settings_content, value, _| {
                            language_settings_field_mut(
                                settings_content,
                                value,
                                |language, value| {
                                    language.prettier.get_or_insert_default().plugins = value;
                                },
                            )
                        },
                    }
                    .unimplemented(),
                ),
                metadata: None,
                files: USER | PROJECT,
            }),
            SettingsPageItem::SettingItem(SettingItem {
                title: t(keys::settings_ui::SETTINGS_NETWORK_OPTIONS).into(),
                description: t(
                    keys::settings_ui::SETTINGS_NETWORK_DEFAULT_PRETTIER_OPTIONS_IN_THE_FORMAT_A,
                )
                .into(),
                field: Box::new(
                    SettingField {
                        organization_override: None,
                        json_path: Some("languages.$(language).prettier.options"),
                        pick: |settings_content| {
                            language_settings_field(settings_content, |language| {
                                language.prettier.as_ref()?.options.as_ref()
                            })
                        },
                        write: |settings_content, value, _| {
                            language_settings_field_mut(
                                settings_content,
                                value,
                                |language, value| {
                                    language.prettier.get_or_insert_default().options = value;
                                },
                            )
                        },
                    }
                    .unimplemented(),
                ),
                metadata: None,
                files: USER | PROJECT,
            }),
        ]
    }

    concat_sections!(
        lsp_section(),
        lsp_completions_section(),
        debugger_section(),
        prettier_section(),
    )
}

fn edit_prediction_language_settings_section() -> [SettingsPageItem; 5] {
    [
        SettingsPageItem::SectionHeader(t(keys::edit_prediction::EDIT_PREDICTION_EDIT_PREDICTIONS).into()),
        SettingsPageItem::SubPageLink(SubPageLink {
            title: t(keys::edit_prediction::EDIT_PREDICTION_CONFIGURE_PROVIDERS).into(),
            r#type: Default::default(),
            json_path: Some("edit_predictions.providers"),
            description: Some(t(keys::settings_ui::SETTINGS_EDIT_PREDICTION_PROVIDERS_DESCRIPTION).into()),
            search_aliases: &[],
            in_json: false,
            files: USER,
            render: render_edit_prediction_setup_page
        }),
        SettingsPageItem::SettingItem(SettingItem {
            title: t(keys::settings_ui::SETTINGS_NETWORK_DATA_COLLECTION).into(),
            description: t(keys::settings_ui::SETTINGS_NETWORK_CONTROLS_WHETHER_ZED_MAY_COLLECT_TRAININ).into(),
            field: Box::new(SettingField {
                organization_override: Some(|org_settings| {
                    const DATA_COLLECTION_DISABLED: EditPredictionDataCollectionChoice = EditPredictionDataCollectionChoice::No;

                    if !org_settings.edit_prediction.is_feedback_enabled {
                        Some(&DATA_COLLECTION_DISABLED)
                    } else {
                        None
                    }
                }),
                json_path: Some("edit_predictions.allow_data_collection"),
                pick: |settings_content| {
                    settings_content
                        .project
                        .all_languages
                        .edit_predictions
                        .as_ref()?
                        .allow_data_collection
                        .as_ref()
                },
                write: |settings_content, value, _app| {
                    settings_content
                        .project
                        .all_languages
                        .edit_predictions
                        .get_or_insert_default()
                        .allow_data_collection = value;
                },
            }),
            metadata: None,
            files: USER,
        }),
        SettingsPageItem::SettingItem(SettingItem {
            title: t(keys::settings_ui::SETTINGS_NETWORK_SHOW_EDIT_PREDICTIONS).into(),
            description: t(keys::settings_ui::SETTINGS_NETWORK_CONTROLS_WHETHER_EDIT_PREDICTIONS_ARE_SH).into(),
            field: Box::new(SettingField {
                organization_override: None,
                json_path: Some("languages.$(language).show_edit_predictions"),
                pick: |settings_content| {
                    language_settings_field(settings_content, |language| {
                        language.show_edit_predictions.as_ref()
                    })
                },
                write: |settings_content, value, _| {
                    language_settings_field_mut(settings_content, value, |language, value| {
                        language.show_edit_predictions = value;
                    })
                },
            }),
            metadata: None,
            files: USER | PROJECT,
        }),
        SettingsPageItem::SettingItem(SettingItem {
            title: t(keys::settings_ui::SETTINGS_NETWORK_DISABLE_IN_LANGUAGE_SCOPES).into(),
            description: "Disable edit predictions in these language scopes, such as \"comment\" and \"string\". Use \"...\" to add scopes without repeating the inherited list.".into(),
            field: Box::new(
                SettingField {
                    organization_override: None,
                    json_path: Some("languages.$(language).edit_predictions_disabled_in"),
                    pick: |settings_content| {
                        language_settings_field(settings_content, |language| {
                            language.edit_predictions_disabled_in.as_ref()
                        })
                    },
                    write: |settings_content, value, _| {
                        language_settings_field_mut(settings_content, value, |language, value| {
                            language.edit_predictions_disabled_in = value;
                        })
                    },
                }
                .unimplemented(),
            ),
            metadata: None,
            files: USER | PROJECT,
        }),
    ]
}

fn show_scrollbar_or_editor(
    settings_content: &SettingsContent,
    show: fn(&SettingsContent) -> Option<&settings::ShowScrollbar>,
) -> Option<&settings::ShowScrollbar> {
    show(settings_content).or(settings_content
        .editor
        .scrollbar
        .as_ref()
        .and_then(|scrollbar| scrollbar.show.as_ref()))
}

fn dynamic_variants<T>() -> &'static [T::Discriminant]
where
    T: strum::IntoDiscriminant,
    T::Discriminant: strum::VariantArray,
{
    <<T as strum::IntoDiscriminant>::Discriminant as strum::VariantArray>::VARIANTS
}

/// Updates the `vim_mode` setting, disabling `helix_mode` if present and
/// `vim_mode` is being enabled.
fn write_vim_mode(settings: &mut SettingsContent, value: Option<bool>, _: &App) {
    write_vim_mode_inner(settings, value);
}

fn write_vim_mode_inner(settings: &mut SettingsContent, value: Option<bool>) {
    if value == Some(true) && settings.helix_mode == Some(true) {
        settings.helix_mode = Some(false);
    }
    settings.vim_mode = value;
}

/// Updates the `helix_mode` setting, disabling `vim_mode` if present and
/// `helix_mode` is being enabled.
fn write_helix_mode(settings: &mut SettingsContent, value: Option<bool>, _: &App) {
    write_helix_mode_inner(settings, value);
}

fn write_helix_mode_inner(settings: &mut SettingsContent, value: Option<bool>) {
    if value == Some(true) && settings.vim_mode == Some(true) {
        settings.vim_mode = Some(false);
    }
    settings.helix_mode = value;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_write_vim_helix_mode() {
        // Enabling vim mode while `vim_mode` and `helix_mode` are not yet set
        // should only update the `vim_mode` setting.
        let mut settings = SettingsContent::default();
        write_vim_mode_inner(&mut settings, Some(true));
        assert_eq!(settings.vim_mode, Some(true));
        assert_eq!(settings.helix_mode, None);

        // Enabling helix mode while `vim_mode` and `helix_mode` are not yet set
        // should only update the `helix_mode` setting.
        let mut settings = SettingsContent::default();
        write_helix_mode_inner(&mut settings, Some(true));
        assert_eq!(settings.helix_mode, Some(true));
        assert_eq!(settings.vim_mode, None);

        // Disabling helix mode should only touch `helix_mode` setting when
        // `vim_mode` is not set.
        write_helix_mode_inner(&mut settings, Some(false));
        assert_eq!(settings.helix_mode, Some(false));
        assert_eq!(settings.vim_mode, None);

        // Enabling vim mode should update `vim_mode` but leave `helix_mode`
        // untouched.
        write_vim_mode_inner(&mut settings, Some(true));
        assert_eq!(settings.vim_mode, Some(true));
        assert_eq!(settings.helix_mode, Some(false));

        // Enabling helix mode should update `helix_mode` and disable
        // `vim_mode`.
        write_helix_mode_inner(&mut settings, Some(true));
        assert_eq!(settings.helix_mode, Some(true));
        assert_eq!(settings.vim_mode, Some(false));

        // Enabling vim mode should update `vim_mode` and disable
        // `helix_mode`.
        write_vim_mode_inner(&mut settings, Some(true));
        assert_eq!(settings.vim_mode, Some(true));
        assert_eq!(settings.helix_mode, Some(false));
    }
}
