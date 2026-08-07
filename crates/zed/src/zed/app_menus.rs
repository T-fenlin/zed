use collab_ui::collab_panel;
use gpui::{App, Menu, MenuItem, OsAction};
use i18n::{keys, t};
use release_channel::ReleaseChannel;
use terminal_view::terminal_panel;
use zed_actions::{Quit, assistant, debug_panel, dev, git_panel, project_panel};

use crate::zed::ToggleLanguage;

pub fn app_menus(cx: &mut App) -> Vec<Menu> {
    let mut view_items = vec![
        MenuItem::action(
            t(keys::app_menu::ZOOM_IN),
            zed_actions::IncreaseBufferFontSize { persist: false },
        ),
        MenuItem::action(
            t(keys::app_menu::ZOOM_OUT),
            zed_actions::DecreaseBufferFontSize { persist: false },
        ),
        MenuItem::action(
            t(keys::app_menu::RESET_ZOOM),
            zed_actions::ResetBufferFontSize { persist: false },
        ),
        MenuItem::action(
            t(keys::app_menu::RESET_ALL_ZOOM),
            zed_actions::ResetAllZoom { persist: false },
        ),
        MenuItem::separator(),
        MenuItem::action(
            t(keys::app_menu::TOGGLE_LEFT_DOCK),
            workspace::ToggleLeftDock,
        ),
        MenuItem::action(
            t(keys::app_menu::TOGGLE_RIGHT_DOCK),
            workspace::ToggleRightDock,
        ),
        MenuItem::action(
            t(keys::app_menu::TOGGLE_BOTTOM_DOCK),
            workspace::ToggleBottomDock,
        ),
        MenuItem::action(
            t(keys::app_menu::TOGGLE_ALL_DOCKS),
            workspace::ToggleAllDocks,
        ),
        MenuItem::submenu(Menu {
            name: t(keys::app_menu::EDITOR_LAYOUT),
            disabled: false,
            items: vec![
                MenuItem::action(t(keys::app_menu::SPLIT_UP), workspace::SplitUp::default()),
                MenuItem::action(t(keys::app_menu::SPLIT_DOWN), workspace::SplitDown::default()),
                MenuItem::action(t(keys::app_menu::SPLIT_LEFT), workspace::SplitLeft::default()),
                MenuItem::action(t(keys::app_menu::SPLIT_RIGHT), workspace::SplitRight::default()),
            ],
        }),
        MenuItem::separator(),
        MenuItem::action(t(keys::app_menu::PROJECT_PANEL), project_panel::ToggleFocus),
        MenuItem::action(t(keys::app_menu::OUTLINE_PANEL), outline_panel::ToggleFocus),
        MenuItem::action(t(keys::app_menu::COLLAB_PANEL), collab_panel::ToggleFocus),
        MenuItem::action(t(keys::app_menu::TERMINAL_PANEL), terminal_panel::Toggle),
        MenuItem::action(t(keys::app_menu::DEBUGGER_PANEL), debug_panel::ToggleFocus),
        MenuItem::action(t(keys::app_menu::AGENT_PANEL), assistant::ToggleFocus),
        MenuItem::action(t(keys::app_menu::GIT_PANEL), git_panel::ToggleFocus),
        MenuItem::separator(),
        MenuItem::action(t(keys::app_menu::DIAGNOSTICS), diagnostics::Deploy),
        MenuItem::separator(),
        MenuItem::action(t(keys::app_menu::TOGGLE_LANGUAGE), ToggleLanguage),
        MenuItem::separator(),
    ];

    if ReleaseChannel::try_global(cx) == Some(ReleaseChannel::Dev) {
        view_items.push(MenuItem::action(
            "Toggle GPUI Inspector",
            dev::ToggleInspector,
        ));
        view_items.push(MenuItem::separator());
    }

    vec![
        Menu {
            name: t(keys::app_menu::ZED),
            disabled: false,
            items: vec![
                MenuItem::action(t(keys::app_menu::ABOUT_ZED), zed_actions::About),
                MenuItem::action(t(keys::app_menu::CHECK_FOR_UPDATES), auto_update::Check),
                MenuItem::separator(),
                MenuItem::submenu(Menu::new(t(keys::app_menu::SETTINGS)).items([
                    MenuItem::action(t(keys::app_menu::OPEN_SETTINGS), zed_actions::OpenSettings),
                    MenuItem::action(t(keys::app_menu::OPEN_SETTINGS_FILE), super::OpenSettingsFile),
                    MenuItem::action(t(keys::app_menu::OPEN_PROJECT_SETTINGS), zed_actions::OpenProjectSettings),
                    MenuItem::action(t(keys::app_menu::OPEN_PROJECT_SETTINGS_FILE), super::OpenProjectSettingsFile),
                    MenuItem::action(t(keys::app_menu::OPEN_DEFAULT_SETTINGS), super::OpenDefaultSettings),
                    MenuItem::separator(),
                    MenuItem::action(t(keys::app_menu::OPEN_KEYMAP), zed_actions::OpenKeymap),
                    MenuItem::action(t(keys::app_menu::OPEN_KEYMAP_FILE), zed_actions::OpenKeymapFile),
                    MenuItem::action(t(keys::app_menu::OPEN_DEFAULT_KEY_BINDINGS), zed_actions::OpenDefaultKeymap),
                    MenuItem::separator(),
                    MenuItem::action(
                        "Select Theme...",
                        zed_actions::theme_selector::Toggle::default(),
                    ),
                    MenuItem::action(
                        "Select Icon Theme...",
                        zed_actions::icon_theme_selector::Toggle::default(),
                    ),
                ])),
                MenuItem::separator(),
                #[cfg(target_os = "macos")]
                MenuItem::os_submenu("Services", gpui::SystemMenuType::Services),
                MenuItem::separator(),
                MenuItem::action(t(keys::app_menu::EXTENSIONS), zed_actions::Extensions::default()),
                #[cfg(not(target_os = "windows"))]
                MenuItem::action(t(keys::app_menu::INSTALL_CLI), install_cli::InstallCliBinary),
                MenuItem::separator(),
                #[cfg(target_os = "macos")]
                MenuItem::action(t(keys::app_menu::HIDE_ZED), super::Hide),
                #[cfg(target_os = "macos")]
                MenuItem::action(t(keys::app_menu::HIDE_OTHERS), super::HideOthers),
                #[cfg(target_os = "macos")]
                MenuItem::action(t(keys::app_menu::SHOW_ALL), super::ShowAll),
                MenuItem::separator(),
                MenuItem::action(t(keys::app_menu::QUIT_ZED), Quit),
            ],
        },
        Menu {
            name: t(keys::app_menu::FILE),
            disabled: false,
            items: vec![
                MenuItem::action(t(keys::app_menu::NEW), workspace::NewFile),
                MenuItem::action(t(keys::app_menu::NEW_WINDOW), workspace::NewWindow),
                MenuItem::separator(),
                #[cfg(not(target_os = "macos"))]
                MenuItem::action(t(keys::app_menu::OPEN_FILE), workspace::OpenFiles),
                MenuItem::action(
                    if cfg!(not(target_os = "macos")) {
                        "Open Folder..."
                    } else {
                        "Open…"
                    },
                    workspace::Open::default(),
                ),
                MenuItem::action(t(keys::app_menu::OPEN_RECENT), zed_actions::OpenRecent::default()),
                MenuItem::action(t(keys::app_menu::OPEN_REMOTE), zed_actions::OpenRemote::default()),
                MenuItem::separator(),
                MenuItem::action(t(keys::app_menu::ADD_FOLDER_TO_PROJECT), workspace::AddFolderToProject),
                MenuItem::separator(),
                MenuItem::action(t(keys::app_menu::SAVE), workspace::Save { save_intent: None }),
                MenuItem::action(t(keys::app_menu::SAVE_AS), workspace::SaveAs),
                MenuItem::action(t(keys::app_menu::SAVE_ALL), workspace::SaveAll { save_intent: None }),
                MenuItem::separator(),
                MenuItem::action(
                    "Close Editor",
                    workspace::CloseActiveItem {
                        save_intent: None,
                        close_pinned: true,
                    },
                ),
                MenuItem::action(t(keys::app_menu::CLOSE_PROJECT), workspace::CloseProject),
                MenuItem::action(t(keys::app_menu::CLOSE_WINDOW), workspace::CloseWindow),
            ],
        },
        Menu {
            name: t(keys::app_menu::EDIT),
            disabled: false,
            items: vec![
                MenuItem::os_action("Undo", editor::actions::Undo, OsAction::Undo),
                MenuItem::os_action("Redo", editor::actions::Redo, OsAction::Redo),
                MenuItem::separator(),
                MenuItem::os_action("Cut", editor::actions::Cut, OsAction::Cut),
                MenuItem::os_action("Copy", editor::actions::Copy, OsAction::Copy),
                MenuItem::action(t(keys::app_menu::COPY_AND_TRIM), editor::actions::CopyAndTrim),
                MenuItem::os_action("Paste", editor::actions::Paste, OsAction::Paste),
                MenuItem::separator(),
                MenuItem::action(t(keys::app_menu::FIND), search::buffer_search::Deploy::find()),
                MenuItem::action(t(keys::app_menu::FIND_IN_PROJECT), workspace::DeploySearch::default()),
                MenuItem::separator(),
                MenuItem::action(
                    "Toggle Line Comment",
                    editor::actions::ToggleComments::default(),
                ),
            ],
        },
        Menu {
            name: t(keys::app_menu::SELECTION),
            disabled: false,
            items: vec![
                MenuItem::os_action(
                    "Select All",
                    editor::actions::SelectAll,
                    OsAction::SelectAll,
                ),
                MenuItem::action(t(keys::app_menu::EXPAND_SELECTION), editor::actions::SelectLargerSyntaxNode),
                MenuItem::action(t(keys::app_menu::SHRINK_SELECTION), editor::actions::SelectSmallerSyntaxNode),
                MenuItem::action(t(keys::app_menu::SELECT_NEXT_SIBLING), editor::actions::SelectNextSyntaxNode),
                MenuItem::action(
                    "Select Previous Sibling",
                    editor::actions::SelectPreviousSyntaxNode,
                ),
                MenuItem::separator(),
                MenuItem::action(
                    "Add Cursor Above",
                    editor::actions::AddSelectionAbove {
                        skip_soft_wrap: true,
                    },
                ),
                MenuItem::action(
                    "Add Cursor Below",
                    editor::actions::AddSelectionBelow {
                        skip_soft_wrap: true,
                    },
                ),
                MenuItem::action(
                    "Select Next Occurrence",
                    editor::actions::SelectNext {
                        replace_newest: false,
                    },
                ),
                MenuItem::action(
                    "Select Previous Occurrence",
                    editor::actions::SelectPrevious {
                        replace_newest: false,
                    },
                ),
                MenuItem::action(t(keys::app_menu::SELECT_ALL_OCCURRENCES), editor::actions::SelectAllMatches),
                MenuItem::separator(),
                MenuItem::action(t(keys::app_menu::MOVE_LINE_UP), editor::actions::MoveLineUp),
                MenuItem::action(t(keys::app_menu::MOVE_LINE_DOWN), editor::actions::MoveLineDown),
                MenuItem::action(t(keys::app_menu::DUPLICATE_SELECTION), editor::actions::DuplicateLineDown),
            ],
        },
        Menu {
            name: t(keys::app_menu::VIEW),
            disabled: false,
            items: view_items,
        },
        Menu {
            name: t(keys::app_menu::GO),
            disabled: false,
            items: vec![
                MenuItem::action(t(keys::app_menu::BACK), workspace::GoBack),
                MenuItem::action(t(keys::app_menu::FORWARD), workspace::GoForward),
                MenuItem::separator(),
                MenuItem::action(t(keys::app_menu::COMMAND_PALETTE), zed_actions::command_palette::Toggle),
                MenuItem::separator(),
                MenuItem::action(t(keys::app_menu::GO_TO_FILE), workspace::ToggleFileFinder::default()),
                // MenuItem::action("Go to Symbol in Project", project_symbols::Toggle),
                MenuItem::action(
                    "Go to Symbol in Editor...",
                    zed_actions::outline::ToggleOutline,
                ),
                MenuItem::action(t(keys::app_menu::GO_TO_LINE_COLUMN), editor::actions::ToggleGoToLine),
                MenuItem::separator(),
                MenuItem::action(
                    "Go to Definition",
                    editor::actions::GoToDefinition::default(),
                ),
                MenuItem::action(t(keys::app_menu::GO_TO_DECLARATION), editor::actions::GoToDeclaration),
                MenuItem::action(t(keys::app_menu::GO_TO_TYPE_DEFINITION), editor::actions::GoToTypeDefinition),
                MenuItem::action(
                    "Find All References",
                    editor::actions::FindAllReferences::default(),
                ),
                MenuItem::separator(),
                MenuItem::action(t(keys::app_menu::NEXT_PROBLEM), editor::actions::GoToDiagnostic::default()),
                MenuItem::action(
                    "Previous Problem",
                    editor::actions::GoToPreviousDiagnostic::default(),
                ),
            ],
        },
        Menu {
            name: t(keys::app_menu::RUN),
            disabled: false,
            items: vec![
                MenuItem::action(
                    "Spawn Task",
                    zed_actions::Spawn::ViaModal {
                        reveal_target: None,
                    },
                ),
                MenuItem::action(t(keys::app_menu::START_DEBUGGER), debugger_ui::Start),
                MenuItem::separator(),
                MenuItem::action(t(keys::app_menu::EDIT_TASKS_JSON), zed_actions::OpenProjectTasks),
                MenuItem::action(t(keys::app_menu::EDIT_DEBUG_JSON), zed_actions::OpenProjectDebugTasks),
                MenuItem::separator(),
                MenuItem::action(t(keys::app_menu::CONTINUE), debugger_ui::Continue),
                MenuItem::action(t(keys::app_menu::STEP_OVER), debugger_ui::StepOver),
                MenuItem::action(t(keys::app_menu::STEP_INTO), debugger_ui::StepInto),
                MenuItem::action(t(keys::app_menu::STEP_OUT), debugger_ui::StepOut),
                MenuItem::separator(),
                MenuItem::action(t(keys::app_menu::TOGGLE_BREAKPOINT), editor::actions::ToggleBreakpoint),
                MenuItem::action(t(keys::app_menu::EDIT_BREAKPOINT), editor::actions::EditLogBreakpoint),
                MenuItem::action(t(keys::app_menu::CLEAR_ALL_BREAKPOINTS), debugger_ui::ClearAllBreakpoints),
            ],
        },
        Menu {
            name: t(keys::app_menu::WINDOW),
            disabled: false,
            items: vec![
                MenuItem::action(t(keys::app_menu::MINIMIZE), super::Minimize),
                MenuItem::action(t(keys::app_menu::ZOOM), super::Zoom),
                MenuItem::separator(),
            ],
        },
        Menu {
            name: t(keys::app_menu::HELP),
            disabled: false,
            items: vec![
                MenuItem::action(
                    "View Release Notes Locally",
                    auto_update_ui::ViewReleaseNotesLocally,
                ),
                MenuItem::action(t(keys::app_menu::VIEW_TELEMETRY), zed_actions::OpenTelemetryLog),
                MenuItem::action(t(keys::app_menu::VIEW_DEPENDENCY_LICENSES), zed_actions::OpenLicenses),
                MenuItem::action(t(keys::app_menu::SHOW_WELCOME), onboarding::ShowWelcome),
                MenuItem::separator(),
                MenuItem::action(t(keys::app_menu::FILE_BUG_REPORT), zed_actions::feedback::FileBugReport),
                MenuItem::action(t(keys::app_menu::REQUEST_FEATURE), zed_actions::feedback::RequestFeature),
                MenuItem::action(t(keys::app_menu::EMAIL_US), zed_actions::feedback::EmailZed),
                MenuItem::separator(),
                MenuItem::action(
                    "Documentation",
                    super::OpenBrowser {
                        url: "https://zed.dev/docs".into(),
                    },
                ),
                MenuItem::action(t(keys::app_menu::ZED_REPOSITORY), feedback::OpenZedRepo),
                MenuItem::action(
                    "Zed Twitter",
                    super::OpenBrowser {
                        url: "https://twitter.com/zeddotdev".into(),
                    },
                ),
                MenuItem::action(
                    "Join the Team",
                    super::OpenBrowser {
                        url: "https://zed.dev/jobs".into(),
                    },
                ),
            ],
        },
    ]
}
