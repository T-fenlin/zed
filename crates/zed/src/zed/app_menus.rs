use collab_ui::collab_panel;
use gpui::{App, Menu, MenuItem, OsAction};
use i18n::{keys, t, t_static};
use project::DisableAiSettings;
use release_channel::ReleaseChannel;
use settings::Settings;
use terminal_view::terminal_panel;
use zed_actions::{Quit, assistant, debug_panel, dev, git_panel, project_panel};

pub fn app_menus(cx: &mut App) -> Vec<Menu> {
    let mut view_items = vec![
        MenuItem::action(
            t(keys::app_menu::MENU_VIEW_ZOOM_IN),
            zed_actions::IncreaseBufferFontSize { persist: false },
        ),
        MenuItem::action(
            t(keys::app_menu::MENU_VIEW_ZOOM_OUT),
            zed_actions::DecreaseBufferFontSize { persist: false },
        ),
        MenuItem::action(
            t(keys::app_menu::MENU_VIEW_RESET_ZOOM),
            zed_actions::ResetBufferFontSize { persist: false },
        ),
        MenuItem::action(
            t(keys::app_menu::MENU_VIEW_RESET_ALL_ZOOM),
            zed_actions::ResetAllZoom { persist: false },
        ),
        MenuItem::separator(),
        MenuItem::action(
            t(keys::app_menu::MENU_VIEW_TOGGLE_LEFT_DOCK),
            workspace::ToggleLeftDock,
        ),
        MenuItem::action(
            t(keys::app_menu::MENU_VIEW_TOGGLE_RIGHT_DOCK),
            workspace::ToggleRightDock,
        ),
        MenuItem::action(
            t(keys::app_menu::MENU_VIEW_TOGGLE_BOTTOM_DOCK),
            workspace::ToggleBottomDock,
        ),
        MenuItem::action(
            t(keys::app_menu::MENU_VIEW_TOGGLE_ALL_DOCKS),
            workspace::ToggleAllDocks,
        ),
        MenuItem::submenu(Menu {
            name: t(keys::app_menu::MENU_VIEW_EDITOR_LAYOUT).into(),
            disabled: false,
            items: vec![
                MenuItem::action(
                    t(keys::app_menu::MENU_VIEW_SPLIT_UP),
                    workspace::SplitUp::default(),
                ),
                MenuItem::action(
                    t(keys::app_menu::MENU_VIEW_SPLIT_DOWN),
                    workspace::SplitDown::default(),
                ),
                MenuItem::action(
                    t(keys::app_menu::MENU_VIEW_SPLIT_LEFT),
                    workspace::SplitLeft::default(),
                ),
                MenuItem::action(
                    t(keys::app_menu::MENU_VIEW_SPLIT_RIGHT),
                    workspace::SplitRight::default(),
                ),
            ],
        }),
        MenuItem::separator(),
        MenuItem::action(
            t(keys::app_menu::MENU_VIEW_PROJECT_PANEL),
            project_panel::ToggleFocus,
        ),
        MenuItem::action(
            t(keys::app_menu::MENU_VIEW_OUTLINE_PANEL),
            outline_panel::ToggleFocus,
        ),
        MenuItem::action(
            t(keys::app_menu::MENU_VIEW_COLLAB_PANEL),
            collab_panel::ToggleFocus,
        ),
        MenuItem::action(
            t(keys::app_menu::MENU_VIEW_TERMINAL_PANEL),
            terminal_panel::Toggle,
        ),
        MenuItem::action(
            t(keys::app_menu::MENU_VIEW_DEBUGGER_PANEL),
            debug_panel::ToggleFocus,
        ),
    ];

    if !DisableAiSettings::get_global(cx).disable_ai {
        view_items.push(MenuItem::action(
            t(keys::app_menu::MENU_VIEW_AGENT_PANEL),
            assistant::ToggleFocus,
        ));
    }

    view_items.extend([
        MenuItem::action(
            t(keys::app_menu::MENU_VIEW_GIT_PANEL),
            git_panel::ToggleFocus,
        ),
        MenuItem::separator(),
        MenuItem::action(t(keys::app_menu::MENU_VIEW_DIAGNOSTICS), diagnostics::Deploy),
        MenuItem::separator(),
    ]);

    if ReleaseChannel::try_global(cx) == Some(ReleaseChannel::Dev) {
        view_items.push(MenuItem::action(
            t(keys::app_menu::MENU_VIEW_TOGGLE_INSPECTOR),
            dev::ToggleInspector,
        ));
        view_items.push(MenuItem::separator());
    }

    vec![
        Menu {
            name: "Zed".into(),
            disabled: false,
            items: vec![
                MenuItem::action(t(keys::app_menu::MENU_ZED_ABOUT), zed_actions::About),
                MenuItem::action(
                    t(keys::app_menu::MENU_ZED_CHECK_FOR_UPDATES),
                    auto_update::Check,
                ),
                MenuItem::separator(),
                MenuItem::submenu(Menu::new(t(keys::app_menu::MENU_ZED_SETTINGS)).items([
                    MenuItem::action(
                        t(keys::app_menu::MENU_ZED_OPEN_SETTINGS),
                        zed_actions::OpenSettings,
                    ),
                    MenuItem::action(
                        t(keys::app_menu::MENU_ZED_OPEN_SETTINGS_FILE),
                        super::OpenSettingsFile,
                    ),
                    MenuItem::action(
                        t(keys::app_menu::MENU_ZED_OPEN_PROJECT_SETTINGS),
                        zed_actions::OpenProjectSettings,
                    ),
                    MenuItem::action(
                        t(keys::app_menu::MENU_ZED_OPEN_PROJECT_SETTINGS_FILE),
                        super::OpenProjectSettingsFile,
                    ),
                    MenuItem::action(
                        t(keys::app_menu::MENU_ZED_OPEN_DEFAULT_SETTINGS),
                        super::OpenDefaultSettings,
                    ),
                    MenuItem::separator(),
                    MenuItem::action(
                        t(keys::app_menu::MENU_ZED_OPEN_KEYMAP),
                        zed_actions::OpenKeymap,
                    ),
                    MenuItem::action(
                        t(keys::app_menu::MENU_ZED_OPEN_KEYMAP_FILE),
                        zed_actions::OpenKeymapFile,
                    ),
                    MenuItem::action(
                        t(keys::app_menu::MENU_ZED_OPEN_DEFAULT_KEYBINDINGS),
                        zed_actions::OpenDefaultKeymap,
                    ),
                    MenuItem::separator(),
                    MenuItem::action(
                        t(keys::app_menu::MENU_ZED_SELECT_THEME),
                        zed_actions::theme_selector::Toggle::default(),
                    ),
                    MenuItem::action(
                        t(keys::app_menu::MENU_ZED_SELECT_ICON_THEME),
                        zed_actions::icon_theme_selector::Toggle::default(),
                    ),
                ])),
                MenuItem::separator(),
                #[cfg(target_os = "macos")]
                MenuItem::os_submenu(
                    t_static(keys::app_menu::MENU_ZED_SERVICES),
                    gpui::SystemMenuType::Services,
                ),
                MenuItem::separator(),
                MenuItem::action(t(keys::app_menu::MENU_ZED_EXTENSIONS), zed_actions::Extensions::default()),
                #[cfg(not(target_os = "windows"))]
                MenuItem::action(
                    t(keys::app_menu::MENU_ZED_INSTALL_CLI),
                    install_cli::InstallCliBinary,
                ),
                MenuItem::separator(),
                #[cfg(target_os = "macos")]
                MenuItem::action(t(keys::app_menu::MENU_ZED_HIDE), super::Hide),
                #[cfg(target_os = "macos")]
                MenuItem::action(t(keys::app_menu::MENU_ZED_HIDE_OTHERS), super::HideOthers),
                #[cfg(target_os = "macos")]
                MenuItem::action(t(keys::app_menu::MENU_ZED_SHOW_ALL), super::ShowAll),
                MenuItem::separator(),
                MenuItem::action(t(keys::app_menu::MENU_ZED_QUIT), Quit),
            ],
        },
        Menu {
            name: t_static(keys::app_menu::MENU_FILE_TITLE).into(),
            disabled: false,
            items: vec![
                MenuItem::action(t_static(keys::app_menu::MENU_FILE_NEW), workspace::NewFile),
                MenuItem::action(
                    t(keys::app_menu::MENU_FILE_NEW_WINDOW),
                    workspace::NewWindow,
                ),
                MenuItem::separator(),
                #[cfg(not(target_os = "macos"))]
                MenuItem::action(t(keys::app_menu::MENU_FILE_OPEN_FILE), workspace::OpenFiles),
                MenuItem::action(
                    if cfg!(not(target_os = "macos")) {
                        t(keys::app_menu::MENU_FILE_OPEN_FOLDER)
                    } else {
                        t(keys::app_menu::MENU_FILE_OPEN_FOLDER_SHORT)
                    },
                    workspace::Open::default(),
                ),
                MenuItem::action(
                    t(keys::app_menu::MENU_FILE_OPEN_RECENT),
                    zed_actions::OpenRecent::default(),
                ),
                MenuItem::action(
                    t(keys::app_menu::MENU_FILE_OPEN_REMOTE),
                    zed_actions::OpenRemote::default(),
                ),
                MenuItem::separator(),
                MenuItem::action(
                    t(keys::app_menu::MENU_FILE_ADD_FOLDER_TO_PROJECT),
                    workspace::AddFolderToProject,
                ),
                MenuItem::separator(),
                MenuItem::action(t(keys::app_menu::MENU_FILE_SAVE), workspace::Save { save_intent: None }),
                MenuItem::action(t(keys::app_menu::MENU_FILE_SAVE_AS), workspace::SaveAs),
                MenuItem::action(
                    t(keys::app_menu::MENU_FILE_SAVE_ALL),
                    workspace::SaveAll { save_intent: None },
                ),
                MenuItem::separator(),
                MenuItem::action(
                    t(keys::app_menu::MENU_FILE_CLOSE_EDITOR),
                    workspace::CloseActiveItem {
                        save_intent: None,
                        close_pinned: true,
                    },
                ),
                MenuItem::action(
                    t(keys::app_menu::MENU_FILE_CLOSE_PROJECT),
                    workspace::CloseProject,
                ),
                MenuItem::action(
                    t(keys::app_menu::MENU_FILE_CLOSE_WINDOW),
                    workspace::CloseWindow,
                ),
            ],
        },
        Menu {
            name: t_static(keys::app_menu::MENU_EDIT_TITLE).into(),
            disabled: false,
            items: vec![
                MenuItem::os_action(t(keys::app_menu::MENU_EDIT_UNDO), editor::actions::Undo, OsAction::Undo),
                MenuItem::os_action(t(keys::app_menu::MENU_EDIT_REDO), editor::actions::Redo, OsAction::Redo),
                MenuItem::separator(),
                MenuItem::os_action(t(keys::app_menu::MENU_EDIT_CUT), editor::actions::Cut, OsAction::Cut),
                MenuItem::os_action(t(keys::app_menu::MENU_EDIT_COPY), editor::actions::Copy, OsAction::Copy),
                MenuItem::action(
                    t(keys::app_menu::MENU_EDIT_COPY_AND_TRIM),
                    editor::actions::CopyAndTrim,
                ),
                MenuItem::os_action(t(keys::app_menu::MENU_EDIT_PASTE), editor::actions::Paste, OsAction::Paste),
                MenuItem::separator(),
                MenuItem::action(
                    t_static(keys::app_menu::MENU_EDIT_FIND),
                    search::buffer_search::Deploy::find(),
                ),
                MenuItem::action(
                    t(keys::app_menu::MENU_EDIT_FIND_IN_PROJECT),
                    workspace::DeploySearch::default(),
                ),
                MenuItem::separator(),
                MenuItem::action(
                    t(keys::app_menu::MENU_EDIT_TOGGLE_LINE_COMMENT),
                    editor::actions::ToggleComments::default(),
                ),
            ],
        },
        Menu {
            name: t_static(keys::app_menu::MENU_SELECTION_TITLE).into(),
            disabled: false,
            items: vec![
                MenuItem::os_action(
                    t(keys::app_menu::MENU_SELECTION_SELECT_ALL),
                    editor::actions::SelectAll,
                    OsAction::SelectAll,
                ),
                MenuItem::action(
                    t(keys::app_menu::MENU_SELECTION_EXPAND),
                    editor::actions::SelectLargerSyntaxNode,
                ),
                MenuItem::action(
                    t(keys::app_menu::MENU_SELECTION_SHRINK),
                    editor::actions::SelectSmallerSyntaxNode,
                ),
                MenuItem::action(
                    t(keys::app_menu::MENU_SELECTION_SELECT_NEXT_SIBLING),
                    editor::actions::SelectNextSyntaxNode,
                ),
                MenuItem::action(
                    t(keys::app_menu::MENU_SELECTION_SELECT_PREVIOUS_SIBLING),
                    editor::actions::SelectPreviousSyntaxNode,
                ),
                MenuItem::separator(),
                MenuItem::action(
                    t(keys::app_menu::MENU_SELECTION_ADD_CURSOR_ABOVE),
                    editor::actions::AddSelectionAbove {
                        skip_soft_wrap: true,
                    },
                ),
                MenuItem::action(
                    t(keys::app_menu::MENU_SELECTION_ADD_CURSOR_BELOW),
                    editor::actions::AddSelectionBelow {
                        skip_soft_wrap: true,
                    },
                ),
                MenuItem::action(
                    t(keys::app_menu::MENU_SELECTION_SELECT_NEXT_OCCURRENCE),
                    editor::actions::SelectNext {
                        replace_newest: false,
                    },
                ),
                MenuItem::action(
                    t(keys::app_menu::MENU_SELECTION_SELECT_PREVIOUS_OCCURRENCE),
                    editor::actions::SelectPrevious {
                        replace_newest: false,
                    },
                ),
                MenuItem::action(
                    t(keys::app_menu::MENU_SELECTION_SELECT_ALL_OCCURRENCES),
                    editor::actions::SelectAllMatches,
                ),
                MenuItem::separator(),
                MenuItem::action(
                    t(keys::app_menu::MENU_SELECTION_MOVE_LINE_UP),
                    editor::actions::MoveLineUp,
                ),
                MenuItem::action(
                    t(keys::app_menu::MENU_SELECTION_MOVE_LINE_DOWN),
                    editor::actions::MoveLineDown,
                ),
                MenuItem::action(
                    t(keys::app_menu::MENU_SELECTION_DUPLICATE),
                    editor::actions::DuplicateLineDown,
                ),
            ],
        },
        Menu {
            name: t_static(keys::app_menu::MENU_VIEW_TITLE).into(),
            disabled: false,
            items: view_items,
        },
        Menu {
            name: t_static(keys::app_menu::MENU_GO_TITLE).into(),
            disabled: false,
            items: vec![
                MenuItem::action(t(keys::app_menu::MENU_GO_BACK), workspace::GoBack),
                MenuItem::action(
                    t_static(keys::app_menu::MENU_GO_FORWARD),
                    workspace::GoForward,
                ),
                MenuItem::separator(),
                MenuItem::action(
                    t(keys::app_menu::MENU_GO_COMMAND_PALETTE),
                    zed_actions::command_palette::Toggle,
                ),
                MenuItem::separator(),
                MenuItem::action(
                    t(keys::app_menu::MENU_GO_GO_TO_FILE),
                    workspace::ToggleFileFinder::default(),
                ),
                // MenuItem::action("Go to Symbol in Project", project_symbols::Toggle),
                MenuItem::action(
                    t(keys::app_menu::MENU_GO_GO_TO_SYMBOL),
                    zed_actions::outline::ToggleOutline,
                ),
                MenuItem::action(
                    t(keys::app_menu::MENU_GO_GO_TO_LINE),
                    editor::actions::ToggleGoToLine,
                ),
                MenuItem::separator(),
                MenuItem::action(
                    t(keys::app_menu::MENU_GO_GO_TO_DEFINITION),
                    editor::actions::GoToDefinition::default(),
                ),
                MenuItem::action(
                    t(keys::app_menu::MENU_GO_GO_TO_DECLARATION),
                    editor::actions::GoToDeclaration::default(),
                ),
                MenuItem::action(
                    t(keys::app_menu::MENU_GO_GO_TO_TYPE_DEFINITION),
                    editor::actions::GoToTypeDefinition::default(),
                ),
                MenuItem::action(
                    t(keys::app_menu::MENU_GO_FIND_ALL_REFERENCES),
                    editor::actions::FindAllReferences::default(),
                ),
                MenuItem::action(
                    t(keys::app_menu::MENU_GO_SHOW_INCOMING_CALLS),
                    call_hierarchy::ShowIncomingCalls,
                ),
                MenuItem::action(
                    t(keys::app_menu::MENU_GO_SHOW_OUTGOING_CALLS),
                    call_hierarchy::ShowOutgoingCalls,
                ),
                MenuItem::separator(),
                MenuItem::action(
                    t(keys::app_menu::MENU_GO_NEXT_PROBLEM),
                    editor::actions::GoToDiagnostic::default(),
                ),
                MenuItem::action(
                    t(keys::app_menu::MENU_GO_PREVIOUS_PROBLEM),
                    editor::actions::GoToPreviousDiagnostic::default(),
                ),
            ],
        },
        Menu {
            name: t_static(keys::app_menu::MENU_RUN_TITLE).into(),
            disabled: false,
            items: vec![
                MenuItem::action(
                    t(keys::app_menu::MENU_RUN_SPAWN_TASK),
                    zed_actions::Spawn::ViaModal {
                        reveal_target: None,
                    },
                ),
                MenuItem::action(
                    t(keys::app_menu::MENU_RUN_START_DEBUGGER),
                    debugger_ui::Start,
                ),
                MenuItem::separator(),
                MenuItem::action(
                    t(keys::app_menu::MENU_RUN_EDIT_TASKS),
                    zed_actions::OpenProjectTasks,
                ),
                MenuItem::action(
                    t(keys::app_menu::MENU_RUN_EDIT_DEBUG),
                    zed_actions::OpenProjectDebugTasks,
                ),
                MenuItem::separator(),
                MenuItem::action(
                    t_static(keys::app_menu::MENU_RUN_CONTINUE),
                    debugger_ui::Continue,
                ),
                MenuItem::action(t(keys::app_menu::MENU_RUN_STEP_OVER), debugger_ui::StepOver),
                MenuItem::action(t(keys::app_menu::MENU_RUN_STEP_INTO), debugger_ui::StepInto),
                MenuItem::action(t(keys::app_menu::MENU_RUN_STEP_OUT), debugger_ui::StepOut),
                MenuItem::separator(),
                MenuItem::action(
                    t(keys::app_menu::MENU_RUN_TOGGLE_BREAKPOINT),
                    editor::actions::ToggleBreakpoint,
                ),
                MenuItem::action(
                    t(keys::app_menu::MENU_RUN_EDIT_BREAKPOINT),
                    editor::actions::EditLogBreakpoint,
                ),
                MenuItem::action(
                    t(keys::app_menu::MENU_RUN_CLEAR_BREAKPOINTS),
                    debugger_ui::ClearAllBreakpoints,
                ),
            ],
        },
        Menu {
            name: t_static(keys::app_menu::MENU_WINDOW_TITLE).into(),
            disabled: false,
            items: vec![
                MenuItem::action(
                    t_static(keys::app_menu::MENU_WINDOW_MINIMIZE),
                    super::Minimize,
                ),
                MenuItem::action(t_static(keys::app_menu::MENU_WINDOW_ZOOM), super::Zoom),
                MenuItem::separator(),
            ],
        },
        Menu {
            name: t_static(keys::app_menu::MENU_HELP_TITLE).into(),
            disabled: false,
            items: vec![
                MenuItem::action(
                    t(keys::app_menu::MENU_HELP_RELEASE_NOTES),
                    auto_update_ui::ViewReleaseNotesLocally,
                ),
                MenuItem::action(
                    t(keys::app_menu::MENU_HELP_TELEMETRY),
                    zed_actions::OpenTelemetryLog,
                ),
                MenuItem::action(
                    t(keys::app_menu::MENU_HELP_LICENSES),
                    zed_actions::OpenLicenses,
                ),
                MenuItem::action(
                    t(keys::app_menu::MENU_HELP_SHOW_WELCOME),
                    onboarding::ShowWelcome,
                ),
                MenuItem::separator(),
                MenuItem::action(
                    t(keys::app_menu::MENU_HELP_FILE_BUG),
                    zed_actions::feedback::FileBugReport,
                ),
                MenuItem::action(
                    t(keys::app_menu::MENU_HELP_REQUEST_FEATURE),
                    zed_actions::feedback::RequestFeature,
                ),
                MenuItem::action(
                    t(keys::app_menu::MENU_HELP_EMAIL),
                    zed_actions::feedback::EmailZed,
                ),
                MenuItem::separator(),
                MenuItem::action(
                    t_static(keys::app_menu::MENU_HELP_DOCUMENTATION),
                    super::OpenBrowser {
                        url: "https://zed.dev/docs".into(),
                    },
                ),
                MenuItem::action(
                    t(keys::app_menu::MENU_HELP_REPOSITORY),
                    feedback::OpenZedRepo,
                ),
                MenuItem::action(
                    t(keys::app_menu::MENU_HELP_TWITTER),
                    super::OpenBrowser {
                        url: "https://twitter.com/zeddotdev".into(),
                    },
                ),
                MenuItem::action(
                    t(keys::app_menu::MENU_HELP_JOIN_TEAM),
                    super::OpenBrowser {
                        url: "https://zed.dev/jobs".into(),
                    },
                ),
            ],
        },
    ]
}
