//! Explicit translation keys — the single source of truth for key IDs.
//!
//! Every key used by `t()` call sites is declared here as a constant, grouped
//! by domain module (mirroring the FTL files under `assets/locales/`). Key IDs
//! are stable: they never derive from the English source text, so renaming an
//! English string cannot silently break translations.
//!
//! [`all`] aggregates every declared key; the i18n audit test compares it
//! against the FTL bundles (en-US hard-fails on missing keys, translation
//! languages report by default and hard-fail with `I18N_AUDIT_STRICT=1`).

/// Declare a domain module of key constants plus its `ALL` array.
macro_rules! define_keys {
    ($($name:ident = $key:literal),* $(,)?) => {
        $(pub const $name: &str = $key;)*
        /// Every key declared in this module.
        pub const ALL: &[&str] = &[$($key),*];
    };
}

pub mod app_menu {
    //! Native application menu keys (`crates/zed/src/zed/app_menus.rs`).
    //! Translations live in `assets/locales/{en-US,zh-CN}/app-menu.ftl`.

    define_keys! {
        // View menu — zoom & docks (explicit keys, ticket 04).
        ZOOM_IN = "app-menu-zoom-in",
        ZOOM_OUT = "app-menu-zoom-out",
        RESET_ZOOM = "app-menu-reset-zoom",
        RESET_ALL_ZOOM = "app-menu-reset-all-zoom",
        TOGGLE_LEFT_DOCK = "app-menu-toggle-left-dock",
        TOGGLE_RIGHT_DOCK = "app-menu-toggle-right-dock",
        TOGGLE_BOTTOM_DOCK = "app-menu-toggle-bottom-dock",
        TOGGLE_ALL_DOCKS = "app-menu-toggle-all-docks",
        SETTINGS = "app-menu-settings",
        OPEN_SETTINGS = "app-menu-open-settings",
        TOGGLE_LANGUAGE = "app-menu-toggle-language",

        // Zed menu.
        ABOUT_ZED = "about-zed",
        CHECK_FOR_UPDATES = "check-for-updates",
        HIDE_ZED = "hide-zed",
        HIDE_OTHERS = "hide-others",
        SHOW_ALL = "show-all",
        QUIT_ZED = "quit-zed",
        INSTALL_CLI = "install-cli",
        ZED = "zed",
        ZED_REPOSITORY = "zed-repository",
        VIEW_DEPENDENCY_LICENSES = "view-dependency-licenses",
        VIEW_TELEMETRY = "view-telemetry",
        REQUEST_FEATURE = "request-feature",
        FILE_BUG_REPORT = "file-bug-report",
        EMAIL_US = "email-us",
        OPEN_DEFAULT_SETTINGS = "open-default-settings",
        OPEN_DEFAULT_KEY_BINDINGS = "open-default-key-bindings",
        OPEN_SETTINGS_FILE = "open-settings-file",
        OPEN_KEYMAP = "open-keymap",
        OPEN_KEYMAP_FILE = "open-keymap-file",
        OPEN_PROJECT_SETTINGS = "open-project-settings",
        OPEN_PROJECT_SETTINGS_FILE = "open-project-settings-file",
        SHOW_WELCOME = "show-welcome",

        // File menu.
        FILE = "file",
        NEW = "new",
        NEW_WINDOW = "new-window",
        OPEN_FILE = "open-file",
        OPEN_RECENT = "open-recent",
        OPEN_REMOTE = "open-remote",
        ADD_FOLDER_TO_PROJECT = "add-folder-to-project",
        CLOSE_PROJECT = "close-project",
        CLOSE_WINDOW = "close-window",
        SAVE = "save",
        SAVE_AS = "save-as",
        SAVE_ALL = "save-all",

        // Edit menu.
        EDIT = "edit",
        COPY_AND_TRIM = "copy-and-trim",
        FIND = "find",
        FIND_IN_PROJECT = "find-in-project",

        // Selection menu.
        SELECTION = "selection",
        EXPAND_SELECTION = "expand-selection",
        SHRINK_SELECTION = "shrink-selection",
        SELECT_NEXT_SIBLING = "select-next-sibling",
        SELECT_ALL_OCCURRENCES = "select-all-occurrences",
        MOVE_LINE_UP = "move-line-up",
        MOVE_LINE_DOWN = "move-line-down",
        DUPLICATE_SELECTION = "duplicate-selection",

        // View menu — panels.
        VIEW = "view",
        EDITOR_LAYOUT = "editor-layout",
        SPLIT_UP = "split-up",
        SPLIT_DOWN = "split-down",
        SPLIT_LEFT = "split-left",
        SPLIT_RIGHT = "split-right",
        PROJECT_PANEL = "project-panel",
        OUTLINE_PANEL = "outline-panel",
        COLLAB_PANEL = "collab-panel",
        TERMINAL_PANEL = "terminal-panel",
        DEBUGGER_PANEL = "debugger-panel",
        AGENT_PANEL = "agent-panel",
        GIT_PANEL = "git-panel",
        DIAGNOSTICS = "diagnostics",

        // Go menu.
        GO = "go",
        BACK = "back",
        FORWARD = "forward",
        COMMAND_PALETTE = "command-palette",
        GO_TO_FILE = "go-to-file",
        GO_TO_LINE_COLUMN = "go-to-line-column",
        GO_TO_DECLARATION = "go-to-declaration",
        GO_TO_TYPE_DEFINITION = "go-to-type-definition",
        NEXT_PROBLEM = "next-problem",

        // Run menu.
        RUN = "run",
        START_DEBUGGER = "start-debugger",
        EDIT_TASKS_JSON = "edit-tasks-json",
        EDIT_DEBUG_JSON = "edit-debug-json",
        CONTINUE = "continue",
        STEP_OVER = "step-over",
        STEP_INTO = "step-into",
        STEP_OUT = "step-out",
        TOGGLE_BREAKPOINT = "toggle-breakpoint",
        EDIT_BREAKPOINT = "edit-breakpoint",
        CLEAR_ALL_BREAKPOINTS = "clear-all-breakpoints",

        // Window / Help / Extensions.
        WINDOW = "window",
        MINIMIZE = "minimize",
        ZOOM = "zoom",
        HELP = "help",
        EXTENSIONS = "extensions",
    }
}

pub mod errors {
    //! Error / notification message keys (`errors.ftl`).

    define_keys! {
        AGENT_CHANGES_REJECTED = "agent-changes-rejected",
        NO_NEW_THREADS_FOUND_TO_IMPORT = "no-new-threads-found-to-import",
        NO_THREADS_FOUND_TO_IMPORT = "no-threads-found-to-import",
        OPERATION_COMPLETED = "operation-completed",
        UPDATE_READY_TO_INSTALL = "update-ready-to-install",
        YOU_HAVE_OUTDATED_SETTINGS = "you-have-outdated-settings",
    }
}

pub mod onboarding {
    //! Onboarding / first-run flow keys (`onboarding.ftl`).

    define_keys! {
        AGENT_SETUP = "agent-setup",
        API_KEYS = "api-keys",
        AUTOMATICALLY_PULL_YOUR_SETTINGS_FROM_OTHER_EDITORS = "automatically-pull-your-settings-from-other-editors",
        FINISH_SETUP = "finish-setup",
        GET_STARTED = "get-started",
        HERE_S_WHAT_YOU_GET_FOR_THE_NEXT_14_DAYS = "here-s-what-you-get-for-the-next-14-days",
        HERE_S_WHAT_YOU_GET = "here-s-what-you-get",
        IMPORT_SETTINGS = "import-settings",
        INSTALL_YOUR_FAVORITE_AGENTS_AND_START_YOUR_FIRST_THREAD = "install-your-favorite-agents-and-start-your-first-thread",
        PRO_TRIAL = "pro-trial",
        SIGN_IN = "sign-in",
        SIGN_IN_TO_TRY_ZED_PRO_FREE_FOR_14_DAYS = "sign-in-to-try-zed-pro-free-for-14-days",
        SIGNING_IN = "signing-in",
        START_FREE_TRIAL = "start-free-trial",
        START_NOW_USING_API_KEYS_FROM_YOUR_ENVIRONMENT_FOR_THE_FOLLOWING_PROVIDERS = "start-now-using-api-keys-from-your-environment-for-the-following-providers",
        TRY_ZED_PRO_FOR_FREE = "try-zed-pro-for-free",
    }
}

pub mod workspace {
    //! Workspace UI keys (`workspace.ftl`): welcome page / restricted mode.

    define_keys! {
        COLLABORATE_WITH_AGENTS = "collaborate-with-agents",
        OPEN_AGENT_PANEL = "open-agent-panel",
        OPEN_IN_DEFAULT_APP = "open-in-default-app",
        RESTRICTED_MODE_PREVENTS = "restricted-mode-prevents",
        RETURN_TO_ONBOARDING = "return-to-onboarding",
        STAY_IN_RESTRICTED_MODE = "stay-in-restricted-mode",
        THE_EDITOR_FOR_WHAT_S_NEXT = "the-editor-for-what-s-next",
        TRUST_AND_CONTINUE = "trust-and-continue",
    }
}

pub mod git_ui {
    //! Git panel keys (`git-ui.ftl`).

    define_keys! {
        AUTHOR = "author",
        CLONE_A_REPOSITORY_FROM_GITHUB_OR_OTHER_SOURCES = "clone-a-repository-from-github-or-other-sources",
        CLOSE = "close",
        COMMIT = "commit",
        COMMIT_SHA = "commit-sha",
        CONFIRM = "confirm",
        CREATE = "create",
        CREATE_REMOTE_REPOSITORY = "create-remote-repository",
        CURRENT_BRANCH = "current-branch",
        DATE = "date",
        DELETE = "delete",
        DESCRIPTION = "description",
        DROP = "drop",
        GIT_COMMIT_NO_VERIFY = "git-commit-no-verify",
        GRAPH = "graph",
        INITIALIZE_REPOSITORY = "initialize-repository",
        LIST = "list",
        NAME = "name",
        NO_CHANGES_TO_COMMIT = "no-changes-to-commit",
        NO_COMMITS_FOUND = "no-commits-found",
        NO_GIT_REPOSITORIES = "no-git-repositories",
        NONE = "none",
        PATH = "path",
        POP = "pop",
        REMOTE_UP_TO_DATE = "remote-up-to-date",
        RESOLVE_WITH_AGENT = "resolve-with-agent",
        RESTORE = "restore",
        RESTORE_ALL = "restore-all",
        REVIEW_DIFF = "review-diff",
        SEE_DOCS = "see-docs",
        SELECTED_BRANCH = "selected-branch",
        SKIP_HOOKS = "skip-hooks",
        STAGE = "stage",
        STAGE_ALL = "stage-all",
        SWITCH = "switch",
        THIS_WILL_UPDATE_YOUR_MOST_RECENT_COMMIT = "this-will-update-your-most-recent-commit",
        TOGGLE_STAGED = "toggle-staged",
        TREE = "tree",
        TRUST_DIRECTORY = "trust-directory",
        UNSTAGE = "unstage",
        UNSTAGE_ALL = "unstage-all",
        USE_BOTH = "use-both",
        VIEW_BRANCH_DIFF = "view-branch-diff",
        VIEW_COMMIT = "view-commit",
        VIEW_DIFF = "view-diff",
    }
}

/// Every declared key, across all domain modules.
pub fn all() -> Vec<&'static str> {
    app_menu::ALL
        .iter()
        .copied()
        .chain(errors::ALL.iter().copied())
        .chain(onboarding::ALL.iter().copied())
        .chain(workspace::ALL.iter().copied())
        .chain(git_ui::ALL.iter().copied())
        .collect()
}
