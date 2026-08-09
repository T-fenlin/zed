use super::*;
// use crate::undo::tests::{build_create_operation, build_rename_operation};
use collections::HashSet;
use editor::{Editor, MultiBufferOffset};
use git::{
    Oid,
    repository::{InitialGraphCommitData, LogSource, RepoPath},
};
use gpui::{Empty, Entity, TestAppContext, VisualTestContext};
use menu::Cancel;
use pretty_assertions::assert_eq;
use project::{FakeFs, ProjectPath};
use serde_json::json;
use settings::{ProjectPanelAutoOpenSettings, SettingsStore};
use smallvec::smallvec;
use std::path::{Path, PathBuf};
use util::{path, paths::PathStyle, rel_path::rel_path};
use workspace::{
    AppState, ItemHandle, MultiWorkspace, Pane, Workspace,
    item::{Item, ProjectItem, test::TestItem},
    register_project_item,
};

#[gpui::test]
async fn test_visible_list(cx: &mut gpui::TestAppContext) {
    init_test(cx);

    let fs = FakeFs::new(cx.executor());
    fs.insert_tree(
        "/root1",
        json!({
            ".dockerignore": "",
            ".git": {
                "HEAD": "",
            },
            "a": {
                "0": { "q": "", "r": "", "s": "" },
                "1": { "t": "", "u": "" },
                "2": { "v": "", "w": "", "x": "", "y": "" },
            },
            "b": {
                "3": { "Q": "" },
                "4": { "R": "", "S": "", "T": "", "U": "" },
            },
            "C": {
                "5": {},
                "6": { "V": "", "W": "" },
                "7": { "X": "" },
                "8": { "Y": {}, "Z": "" }
            }
        }),
    )
    .await;
    fs.insert_tree(
        "/root2",
        json!({
            "d": {
                "9": ""
            },
            "e": {}
        }),
    )
    .await;

    let project = Project::test(fs.clone(), ["/root1".as_ref(), "/root2".as_ref()], cx).await;
    let window = cx.add_window(|window, cx| MultiWorkspace::test_new(project.clone(), window, cx));
    let workspace = window
        .read_with(cx, |mw, _| mw.workspace().clone())
        .unwrap();
    let cx = &mut VisualTestContext::from_window(window.into(), cx);
    let panel = workspace.update_in(cx, ProjectPanel::new);
    cx.run_until_parked();
    assert_eq!(
        visible_entries_as_strings(&panel, 0..50, cx),
        &[
            "v root1",
            "    > .git",
            "    > a",
            "    > b",
            "    > C",
            "      .dockerignore",
            "v root2",
            "    > d",
            "    > e",
        ]
    );

    toggle_expand_dir(&panel, "root1/b", cx);
    assert_eq!(
        visible_entries_as_strings(&panel, 0..50, cx),
        &[
            "v root1",
            "    > .git",
            "    > a",
            "    v b  <== selected",
            "        > 3",
            "        > 4",
            "    > C",
            "      .dockerignore",
            "v root2",
            "    > d",
            "    > e",
        ]
    );

    assert_eq!(
        visible_entries_as_strings(&panel, 6..9, cx),
        &[
            //
            "    > C",
            "      .dockerignore",
            "v root2",
        ]
    );
}

#[gpui::test]
async fn test_opening_file(cx: &mut gpui::TestAppContext) {
    init_test_with_editor(cx);

    let fs = FakeFs::new(cx.executor());
    fs.insert_tree(
        path!("/src"),
        json!({
            "test": {
                "first.rs": "// First Rust file",
                "second.rs": "// Second Rust file",
                "third.rs": "// Third Rust file",
            }
        }),
    )
    .await;

    let project = Project::test(fs.clone(), [path!("/src").as_ref()], cx).await;
    let window = cx.add_window(|window, cx| MultiWorkspace::test_new(project.clone(), window, cx));
    let workspace = window
        .read_with(cx, |mw, _| mw.workspace().clone())
        .unwrap();
    let cx = &mut VisualTestContext::from_window(window.into(), cx);
    let panel = workspace.update_in(cx, ProjectPanel::new);
    cx.run_until_parked();

    toggle_expand_dir(&panel, "src/test", cx);
    select_path(&panel, "src/test/first.rs", cx);
    panel.update_in(cx, |panel, window, cx| panel.open(&Open, window, cx));
    cx.executor().run_until_parked();
    assert_eq!(
        visible_entries_as_strings(&panel, 0..10, cx),
        &[
            "v src",
            "    v test",
            "          first.rs  <== selected  <== marked",
            "          second.rs",
            "          third.rs"
        ]
    );
    ensure_single_file_is_opened(&workspace, "test/first.rs", cx);

    select_path(&panel, "src/test/second.rs", cx);
    panel.update_in(cx, |panel, window, cx| panel.open(&Open, window, cx));
    cx.executor().run_until_parked();
    assert_eq!(
        visible_entries_as_strings(&panel, 0..10, cx),
        &[
            "v src",
            "    v test",
            "          first.rs",
            "          second.rs  <== selected  <== marked",
            "          third.rs"
        ]
    );
    ensure_single_file_is_opened(&workspace, "test/second.rs", cx);
}

#[gpui::test]
async fn test_file_history_action_uses_focused_project_panel_selection(
    cx: &mut gpui::TestAppContext,
) {
    init_test_with_git_ui(cx);

    let fs = FakeFs::new(cx.executor());
    fs.insert_tree(
        Path::new("/project"),
        json!({
            ".git": {},
            "tracked1.txt": "tracked 1",
            "tracked2.txt": "tracked 2",
        }),
    )
    .await;

    let commits = vec![Arc::new(InitialGraphCommitData {
        sha: Oid::from_bytes(&[1; 20]).unwrap(),
        parents: smallvec![],
        ref_names: vec!["HEAD".into(), "refs/heads/main".into()],
    })];
    fs.set_graph_commits(Path::new("/project/.git"), commits);

    let project = Project::test(fs.clone(), [Path::new("/project")], cx).await;
    cx.run_until_parked();

    let repository = project.read_with(cx, |project, cx| {
        project
            .active_repository(cx)
            .expect("should have active repository")
    });
    let project_panel_repo_path = RepoPath::new(&"tracked1.txt").unwrap();
    let editor_repo_path = RepoPath::new(&"tracked2.txt").unwrap();
    let project_panel_path = repository
        .read_with(cx, |repository, cx| {
            repository.repo_path_to_project_path(&project_panel_repo_path, cx)
        })
        .expect("project panel path should resolve");
    let editor_path = repository
        .read_with(cx, |repository, cx| {
            repository.repo_path_to_project_path(&editor_repo_path, cx)
        })
        .expect("editor path should resolve");

    let (multi_workspace, cx) =
        cx.add_window_view(|window, cx| MultiWorkspace::test_new(project.clone(), window, cx));
    let workspace = multi_workspace.read_with(&*cx, |multi_workspace, _| {
        multi_workspace.workspace().clone()
    });
    let project_panel = multi_workspace.update_in(cx, |multi_workspace, window, cx| {
        let workspace = multi_workspace.workspace();
        workspace.update(cx, |workspace, cx| {
            let project_panel = ProjectPanel::new(workspace, window, cx);
            workspace.add_panel(project_panel.clone(), window, cx);
            project_panel
        })
    });
    cx.run_until_parked();

    let editor_buffer = project
        .update(cx, |project, cx| {
            project.open_buffer(editor_path.clone(), cx)
        })
        .await
        .expect("editor buffer should open");
    multi_workspace.update_in(cx, |multi_workspace, window, cx| {
        let workspace = multi_workspace.workspace();
        let multibuffer = cx.new(|cx| {
            let mut multibuffer = editor::MultiBuffer::new(language::Capability::ReadWrite);
            multibuffer.set_excerpts_for_buffer(
                editor_buffer.clone(),
                [Default::default()..editor_buffer.read(cx).max_point()],
                0,
                cx,
            );
            multibuffer
        });
        let editor =
            cx.new(|cx| Editor::for_multibuffer(multibuffer, Some(project.clone()), window, cx));
        workspace.update(cx, |workspace, cx| {
            workspace.add_item_to_active_pane(Box::new(editor.clone()), None, true, window, cx);
        });
        editor.update(cx, |editor, cx| {
            window.focus(&editor.focus_handle(cx), cx);
        });
    });
    cx.run_until_parked();

    multi_workspace.update_in(cx, |_multi_workspace, window, cx| {
        project_panel.update(cx, |panel, cx| {
            panel.select_path_for_test(project_panel_path.clone(), cx);
        });
        project_panel.update(cx, |panel, cx| {
            panel.focus_handle(cx).focus(window, cx);
        });
    });
    cx.run_until_parked();

    cx.update(|window, cx| {
        window.dispatch_action(Box::new(git::FileHistory), cx);
    });
    cx.run_until_parked();

    workspace.read_with(&*cx, |workspace, cx| {
        let graphs = workspace
            .items_of_type::<git_ui::git_graph::GitGraph>(cx)
            .collect::<Vec<_>>();
        assert_eq!(graphs.len(), 1);
        assert_eq!(
            graphs[0].read(cx).log_source_for_test(),
            &LogSource::Path(project_panel_repo_path)
        );
    });
}

#[gpui::test]
async fn test_file_history_action_does_not_fall_back_to_editor_when_focused_project_panel_selection_has_no_git_repo(
    cx: &mut gpui::TestAppContext,
) {
    init_test_with_git_ui(cx);

    let fs = FakeFs::new(cx.executor());
    fs.insert_tree(
        Path::new("/git-project"),
        json!({
            ".git": {},
            "tracked.txt": "tracked",
        }),
    )
    .await;
    fs.insert_tree(
        Path::new("/plain-project"),
        json!({
            "plain.txt": "plain",
        }),
    )
    .await;

    fs.set_graph_commits(
        Path::new("/git-project/.git"),
        vec![Arc::new(InitialGraphCommitData {
            sha: Oid::from_bytes(&[1; 20]).unwrap(),
            parents: smallvec![],
            ref_names: vec!["HEAD".into(), "refs/heads/main".into()],
        })],
    );

    let project = Project::test(
        fs.clone(),
        [Path::new("/git-project"), Path::new("/plain-project")],
        cx,
    )
    .await;
    cx.run_until_parked();

    let repository = project.read_with(cx, |project, cx| {
        project
            .active_repository(cx)
            .expect("should have active repository")
    });
    let editor_repo_path = RepoPath::new(&"tracked.txt").unwrap();
    let editor_path = repository
        .read_with(cx, |repository, cx| {
            repository.repo_path_to_project_path(&editor_repo_path, cx)
        })
        .expect("editor path should resolve");
    let plain_worktree_id = project.read_with(cx, |project, cx| {
        project
            .worktree_for_root_name("plain-project", cx)
            .expect("plain worktree should exist")
            .read(cx)
            .id()
    });
    let plain_project_path = ProjectPath {
        worktree_id: plain_worktree_id,
        path: rel_path("plain.txt").into(),
    };

    let (multi_workspace, cx) =
        cx.add_window_view(|window, cx| MultiWorkspace::test_new(project.clone(), window, cx));
    let workspace = multi_workspace.read_with(&*cx, |multi_workspace, _| {
        multi_workspace.workspace().clone()
    });
    let project_panel = multi_workspace.update_in(cx, |multi_workspace, window, cx| {
        let workspace = multi_workspace.workspace();
        workspace.update(cx, |workspace, cx| {
            let project_panel = ProjectPanel::new(workspace, window, cx);
            workspace.add_panel(project_panel.clone(), window, cx);
            project_panel
        })
    });
    cx.run_until_parked();

    let editor_buffer = project
        .update(cx, |project, cx| {
            project.open_buffer(editor_path.clone(), cx)
        })
        .await
        .expect("editor buffer should open");
    multi_workspace.update_in(cx, |multi_workspace, window, cx| {
        let workspace = multi_workspace.workspace();
        let multibuffer = cx.new(|cx| {
            let mut multibuffer = editor::MultiBuffer::new(language::Capability::ReadWrite);
            multibuffer.set_excerpts_for_buffer(
                editor_buffer.clone(),
                [Default::default()..editor_buffer.read(cx).max_point()],
                0,
                cx,
            );
            multibuffer
        });
        let editor =
            cx.new(|cx| Editor::for_multibuffer(multibuffer, Some(project.clone()), window, cx));
        workspace.update(cx, |workspace, cx| {
            workspace.add_item_to_active_pane(Box::new(editor.clone()), None, true, window, cx);
        });
        editor.update(cx, |editor, cx| {
            window.focus(&editor.focus_handle(cx), cx);
        });
    });
    cx.run_until_parked();

    multi_workspace.update_in(cx, |_multi_workspace, window, cx| {
        project_panel.update(cx, |panel, cx| {
            panel.select_path_for_test(plain_project_path.clone(), cx);
        });
        project_panel.update(cx, |panel, cx| {
            panel.focus_handle(cx).focus(window, cx);
        });
    });
    cx.run_until_parked();

    cx.update(|window, cx| {
        window.dispatch_action(Box::new(git::FileHistory), cx);
    });
    cx.run_until_parked();

    workspace.read_with(&*cx, |workspace, cx| {
        assert_eq!(
            workspace
                .items_of_type::<git_ui::git_graph::GitGraph>(cx)
                .count(),
            0
        );
    });
}

#[gpui::test]
async fn test_exclusions_in_visible_list(cx: &mut gpui::TestAppContext) {
    init_test(cx);
    cx.update(|cx| {
        cx.update_global::<SettingsStore, _>(|store, cx| {
            store.update_user_settings(cx, |settings| {
                settings.project.worktree.file_scan_exclusions =
                    Some(vec!["**/.git".to_string(), "**/4/**".to_string()]);
            });
        });
    });

    let fs = FakeFs::new(cx.background_executor.clone());
    fs.insert_tree(
        "/root1",
        json!({
            ".dockerignore": "",
            ".git": {
                "HEAD": "",
            },
            "a": {
                "0": { "q": "", "r": "", "s": "" },
                "1": { "t": "", "u": "" },
                "2": { "v": "", "w": "", "x": "", "y": "" },
            },
            "b": {
                "3": { "Q": "" },
                "4": { "R": "", "S": "", "T": "", "U": "" },
            },
            "C": {
                "5": {},
                "6": { "V": "", "W": "" },
                "7": { "X": "" },
                "8": { "Y": {}, "Z": "" }
            }
        }),
    )
    .await;
    fs.insert_tree(
        "/root2",
        json!({
            "d": {
                "4": ""
            },
            "e": {}
        }),
    )
    .await;

    let project = Project::test(fs.clone(), ["/root1".as_ref(), "/root2".as_ref()], cx).await;
    let window = cx.add_window(|window, cx| MultiWorkspace::test_new(project.clone(), window, cx));
    let workspace = window
        .read_with(cx, |mw, _| mw.workspace().clone())
        .unwrap();
    let cx = &mut VisualTestContext::from_window(window.into(), cx);
    let panel = workspace.update_in(cx, ProjectPanel::new);
    cx.run_until_parked();
    assert_eq!(
        visible_entries_as_strings(&panel, 0..50, cx),
        &[
            "v root1",
            "    > a",
            "    > b",
            "    > C",
            "      .dockerignore",
            "v root2",
            "    > d",
            "    > e",
        ]
    );

    toggle_expand_dir(&panel, "root1/b", cx);
    assert_eq!(
        visible_entries_as_strings(&panel, 0..50, cx),
        &[
            "v root1",
            "    > a",
            "    v b  <== selected",
            "        > 3",
            "    > C",
            "      .dockerignore",
            "v root2",
            "    > d",
            "    > e",
        ]
    );

    toggle_expand_dir(&panel, "root2/d", cx);
    assert_eq!(
        visible_entries_as_strings(&panel, 0..50, cx),
        &[
            "v root1",
            "    > a",
            "    v b",
            "        > 3",
            "    > C",
            "      .dockerignore",
            "v root2",
            "    v d  <== selected",
            "    > e",
        ]
    );

    toggle_expand_dir(&panel, "root2/e", cx);
    assert_eq!(
        visible_entries_as_strings(&panel, 0..50, cx),
        &[
            "v root1",
            "    > a",
            "    v b",
            "        > 3",
            "    > C",
            "      .dockerignore",
            "v root2",
            "    v d",
            "    v e  <== selected",
        ]
    );
}

#[gpui::test]
async fn test_auto_collapse_dir_paths(cx: &mut gpui::TestAppContext) {
    init_test(cx);

    let fs = FakeFs::new(cx.executor());
    fs.insert_tree(
        path!("/root1"),
        json!({
            "dir_1": {
                "nested_dir_1": {
                    "nested_dir_2": {
                        "nested_dir_3": {
                            "file_a.java": "// File contents",
                            "file_b.java": "// File contents",
                            "file_c.java": "// File contents",
                            "nested_dir_4": {
                                "nested_dir_5": {
                                    "file_d.java": "// File contents",
                                }
                            }
                        }
                    }
                }
            }
        }),
    )
    .await;
    fs.insert_tree(
        path!("/root2"),
        json!({
            "dir_2": {
                "file_1.java": "// File contents",
            }
        }),
    )
    .await;

    // Test 1: Multiple worktrees with auto_fold_dirs = true
    let project = Project::test(
        fs.clone(),
        [path!("/root1").as_ref(), path!("/root2").as_ref()],
        cx,
    )
    .await;
    let window = cx.add_window(|window, cx| MultiWorkspace::test_new(project.clone(), window, cx));
    let workspace = window
        .read_with(cx, |mw, _| mw.workspace().clone())
        .unwrap();
    let cx = &mut VisualTestContext::from_window(window.into(), cx);
    cx.update(|_, cx| {
        let settings = *ProjectPanelSettings::get_global(cx);
        ProjectPanelSettings::override_global(
            ProjectPanelSettings {
                auto_fold_dirs: true,
                sort_mode: settings::ProjectPanelSortMode::DirectoriesFirst,
                ..settings
            },
            cx,
        );
    });
    let panel = workspace.update_in(cx, ProjectPanel::new);
    cx.run_until_parked();
    assert_eq!(
        visible_entries_as_strings(&panel, 0..10, cx),
        &[
            "v root1",
            "    > dir_1/nested_dir_1/nested_dir_2/nested_dir_3",
            "v root2",
            "    > dir_2",
        ]
    );

    toggle_expand_dir(
        &panel,
        "root1/dir_1/nested_dir_1/nested_dir_2/nested_dir_3",
        cx,
    );
    assert_eq!(
        visible_entries_as_strings(&panel, 0..10, cx),
        &[
            "v root1",
            "    v dir_1/nested_dir_1/nested_dir_2/nested_dir_3  <== selected",
            "        > nested_dir_4/nested_dir_5",
            "          file_a.java",
            "          file_b.java",
            "          file_c.java",
            "v root2",
            "    > dir_2",
        ]
    );

    toggle_expand_dir(
        &panel,
        "root1/dir_1/nested_dir_1/nested_dir_2/nested_dir_3/nested_dir_4/nested_dir_5",
        cx,
    );
    assert_eq!(
        visible_entries_as_strings(&panel, 0..10, cx),
        &[
            "v root1",
            "    v dir_1/nested_dir_1/nested_dir_2/nested_dir_3",
            "        v nested_dir_4/nested_dir_5  <== selected",
            "              file_d.java",
            "          file_a.java",
            "          file_b.java",
            "          file_c.java",
            "v root2",
            "    > dir_2",
        ]
    );
    toggle_expand_dir(&panel, "root2/dir_2", cx);
    assert_eq!(
        visible_entries_as_strings(&panel, 0..10, cx),
        &[
            "v root1",
            "    v dir_1/nested_dir_1/nested_dir_2/nested_dir_3",
            "        v nested_dir_4/nested_dir_5",
            "              file_d.java",
            "          file_a.java",
            "          file_b.java",
            "          file_c.java",
            "v root2",
            "    v dir_2  <== selected",
            "          file_1.java",
        ]
    );

    // Test 2: Single worktree with auto_fold_dirs = true and hide_root = true
    {
        let project = Project::test(fs.clone(), [path!("/root1").as_ref()], cx).await;
        let window =
            cx.add_window(|window, cx| MultiWorkspace::test_new(project.clone(), window, cx));
        let workspace = window
            .read_with(cx, |mw, _| mw.workspace().clone())
            .unwrap();
        let cx = &mut VisualTestContext::from_window(window.into(), cx);
        cx.update(|_, cx| {
            let settings = *ProjectPanelSettings::get_global(cx);
            ProjectPanelSettings::override_global(
                ProjectPanelSettings {
                    auto_fold_dirs: true,
                    hide_root: true,
                    ..settings
                },
                cx,
            );
        });
        let panel = workspace.update_in(cx, ProjectPanel::new);
        cx.run_until_parked();
        assert_eq!(
            visible_entries_as_strings(&panel, 0..10, cx),
            &["> dir_1/nested_dir_1/nested_dir_2/nested_dir_3"],
            "隐藏根目录为 true 的单个工作树应隐藏根目录并显示自动折叠的路径"
        );

        toggle_expand_dir(
            &panel,
            "root1/dir_1/nested_dir_1/nested_dir_2/nested_dir_3",
            cx,
        );
        assert_eq!(
            visible_entries_as_strings(&panel, 0..10, cx),
            &[
                "v dir_1/nested_dir_1/nested_dir_2/nested_dir_3  <== selected",
                "    > nested_dir_4/nested_dir_5",
                "      file_a.java",
                "      file_b.java",
                "      file_c.java",
            ],
            "展开隐藏根目录的自动折叠路径时,应显示内容而不带根目录前缀"
        );

        toggle_expand_dir(
            &panel,
            "root1/dir_1/nested_dir_1/nested_dir_2/nested_dir_3/nested_dir_4/nested_dir_5",
            cx,
        );
        assert_eq!(
            visible_entries_as_strings(&panel, 0..10, cx),
            &[
                "v dir_1/nested_dir_1/nested_dir_2/nested_dir_3",
                "    v nested_dir_4/nested_dir_5  <== selected",
                "          file_d.java",
                "      file_a.java",
                "      file_b.java",
                "      file_c.java",
            ],
            "隐藏根目录时,嵌套展开应保持正确的缩进"
        );
    }
}

#[gpui::test(iterations = 30)]
async fn test_editing_files(cx: &mut gpui::TestAppContext) {
    init_test(cx);

    let fs = FakeFs::new(cx.executor());
    fs.insert_tree(
        "/root1",
        json!({
            ".dockerignore": "",
            ".git": {
                "HEAD": "",
            },
            "a": {
                "0": { "q": "", "r": "", "s": "" },
                "1": { "t": "", "u": "" },
                "2": { "v": "", "w": "", "x": "", "y": "" },
            },
            "b": {
                "3": { "Q": "" },
                "4": { "R": "", "S": "", "T": "", "U": "" },
            },
            "C": {
                "5": {},
                "6": { "V": "", "W": "" },
                "7": { "X": "" },
                "8": { "Y": {}, "Z": "" }
            }
        }),
    )
    .await;
    fs.insert_tree(
        "/root2",
        json!({
            "d": {
                "9": ""
            },
            "e": {}
        }),
    )
    .await;

    let project = Project::test(fs.clone(), ["/root1".as_ref(), "/root2".as_ref()], cx).await;
    let window = cx.add_window(|window, cx| MultiWorkspace::test_new(project.clone(), window, cx));
    let workspace = window
        .read_with(cx, |mw, _| mw.workspace().clone())
        .unwrap();
    let cx = &mut VisualTestContext::from_window(window.into(), cx);
    let panel = workspace.update_in(cx, |workspace, window, cx| {
        let panel = ProjectPanel::new(workspace, window, cx);
        workspace.add_panel(panel.clone(), window, cx);
        panel
    });
    cx.run_until_parked();

    select_path(&panel, "root1", cx);
    assert_eq!(
        visible_entries_as_strings(&panel, 0..10, cx),
        &[
            "v root1  <== selected",
            "    > .git",
            "    > a",
            "    > b",
            "    > C",
            "      .dockerignore",
            "v root2",
            "    > d",
            "    > e",
        ]
    );

    // Add a file with the root folder selected. The filename editor is placed
    // before the first file in the root folder.
    panel.update_in(cx, |panel, window, cx| panel.new_file(&NewFile, window, cx));
    cx.run_until_parked();
    panel.update_in(cx, |panel, window, cx| {
        assert!(panel.filename_editor.read(cx).is_focused(window));
    });
    assert_eq!(
        visible_entries_as_strings(&panel, 0..10, cx),
        &[
            "v root1",
            "    > .git",
            "    > a",
            "    > b",
            "    > C",
            "      [EDITOR: '']  <== selected",
            "      .dockerignore",
            "v root2",
            "    > d",
            "    > e",
        ]
    );

    let confirm = panel.update_in(cx, |panel, window, cx| {
        panel.filename_editor.update(cx, |editor, cx| {
            editor.set_text("the-new-filename", window, cx)
        });
        panel.confirm_edit(true, window, cx).unwrap()
    });
    assert_eq!(
        visible_entries_as_strings(&panel, 0..10, cx),
        &[
            "v root1",
            "    > .git",
            "    > a",
            "    > b",
            "    > C",
            "      [PROCESSING: 'the-new-filename']  <== selected",
            "      .dockerignore",
            "v root2",
            "    > d",
            "    > e",
        ]
    );

    confirm.await.unwrap();
    cx.run_until_parked();
    assert_eq!(
        visible_entries_as_strings(&panel, 0..10, cx),
        &[
            "v root1",
            "    > .git",
            "    > a",
            "    > b",
            "    > C",
            "      .dockerignore",
            "      the-new-filename  <== selected  <== marked",
            "v root2",
            "    > d",
            "    > e",
        ]
    );

    select_path(&panel, "root1/b", cx);
    panel.update_in(cx, |panel, window, cx| panel.new_file(&NewFile, window, cx));
    cx.run_until_parked();
    assert_eq!(
        visible_entries_as_strings(&panel, 0..10, cx),
        &[
            "v root1",
            "    > .git",
            "    > a",
            "    v b",
            "        > 3",
            "        > 4",
            "          [EDITOR: '']  <== selected",
            "    > C",
            "      .dockerignore",
            "      the-new-filename",
        ]
    );

    panel
        .update_in(cx, |panel, window, cx| {
            panel.filename_editor.update(cx, |editor, cx| {
                editor.set_text("another-filename.txt", window, cx)
            });
            panel.confirm_edit(true, window, cx).unwrap()
        })
        .await
        .unwrap();
    cx.run_until_parked();
    assert_eq!(
        visible_entries_as_strings(&panel, 0..10, cx),
        &[
            "v root1",
            "    > .git",
            "    > a",
            "    v b",
            "        > 3",
            "        > 4",
            "          another-filename.txt  <== selected  <== marked",
            "    > C",
            "      .dockerignore",
            "      the-new-filename",
        ]
    );

    select_path(&panel, "root1/b/another-filename.txt", cx);
    panel.update_in(cx, |panel, window, cx| panel.rename(&Rename, window, cx));
    assert_eq!(
        visible_entries_as_strings(&panel, 0..10, cx),
        &[
            "v root1",
            "    > .git",
            "    > a",
            "    v b",
            "        > 3",
            "        > 4",
            "          [EDITOR: 'another-filename.txt']  <== selected  <== marked",
            "    > C",
            "      .dockerignore",
            "      the-new-filename",
        ]
    );

    let confirm = panel.update_in(cx, |panel, window, cx| {
        panel.filename_editor.update(cx, |editor, cx| {
            let file_name_selections = editor
                .selections
                .all::<MultiBufferOffset>(&editor.display_snapshot(cx));
            assert_eq!(
                file_name_selections.len(),
                1,
                "文件编辑应只有一个选中项,但实际有: {file_name_selections:?}"
            );
            let file_name_selection = &file_name_selections[0];
            assert_eq!(
                file_name_selection.start,
                MultiBufferOffset(0),
                "应从文件名开头开始选中"
            );
            assert_eq!(
                file_name_selection.end,
                MultiBufferOffset("another-filename".len()),
                "不应选中文件扩展名"
            );

            editor.set_text("a-different-filename.tar.gz", window, cx)
        });
        panel.confirm_edit(true, window, cx).unwrap()
    });
    assert_eq!(
        visible_entries_as_strings(&panel, 0..10, cx),
        &[
            "v root1",
            "    > .git",
            "    > a",
            "    v b",
            "        > 3",
            "        > 4",
            "          [PROCESSING: 'a-different-filename.tar.gz']  <== selected  <== marked",
            "    > C",
            "      .dockerignore",
            "      the-new-filename",
        ]
    );

    confirm.await.unwrap();
    cx.run_until_parked();
    assert_eq!(
        visible_entries_as_strings(&panel, 0..10, cx),
        &[
            "v root1",
            "    > .git",
            "    > a",
            "    v b",
            "        > 3",
            "        > 4",
            "          a-different-filename.tar.gz  <== selected",
            "    > C",
            "      .dockerignore",
            "      the-new-filename",
        ]
    );

    panel.update_in(cx, |panel, window, cx| panel.rename(&Rename, window, cx));
    assert_eq!(
        visible_entries_as_strings(&panel, 0..10, cx),
        &[
            "v root1",
            "    > .git",
            "    > a",
            "    v b",
            "        > 3",
            "        > 4",
            "          [EDITOR: 'a-different-filename.tar.gz']  <== selected",
            "    > C",
            "      .dockerignore",
            "      the-new-filename",
        ]
    );

    panel.update_in(cx, |panel, window, cx| {
            panel.filename_editor.update(cx, |editor, cx| {
                let file_name_selections = editor.selections.all::<MultiBufferOffset>(&editor.display_snapshot(cx));
                assert_eq!(file_name_selections.len(), 1, "文件编辑应只有一个选中项,但实际有: {file_name_selections:?}");
                let file_name_selection = &file_name_selections[0];
                assert_eq!(file_name_selection.start, MultiBufferOffset(0), "应从文件名开头开始选中");
                assert_eq!(file_name_selection.end, MultiBufferOffset("a-different-filename.tar".len()), "不应选中文件扩展名,但仍可选中直到最后一个点之前的内容..");

            });
            panel.cancel(&menu::Cancel, window, cx)
        });
    cx.run_until_parked();
    panel.update_in(cx, |panel, window, cx| {
        panel.new_directory(&NewDirectory, window, cx)
    });
    cx.run_until_parked();
    assert_eq!(
        visible_entries_as_strings(&panel, 0..10, cx),
        &[
            "v root1",
            "    > .git",
            "    > a",
            "    v b",
            "        > [EDITOR: '']  <== selected",
            "        > 3",
            "        > 4",
            "          a-different-filename.tar.gz",
            "    > C",
            "      .dockerignore",
        ]
    );

    let confirm = panel.update_in(cx, |panel, window, cx| {
        panel
            .filename_editor
            .update(cx, |editor, cx| editor.set_text("new-dir", window, cx));
        panel.confirm_edit(true, window, cx).unwrap()
    });
    panel.update_in(cx, |panel, window, cx| {
        panel.select_next(&Default::default(), window, cx)
    });
    assert_eq!(
        visible_entries_as_strings(&panel, 0..10, cx),
        &[
            "v root1",
            "    > .git",
            "    > a",
            "    v b",
            "        > [PROCESSING: 'new-dir']",
            "        > 3  <== selected",
            "        > 4",
            "          a-different-filename.tar.gz",
            "    > C",
            "      .dockerignore",
        ]
    );

    confirm.await.unwrap();
    cx.run_until_parked();
    assert_eq!(
        visible_entries_as_strings(&panel, 0..10, cx),
        &[
            "v root1",
            "    > .git",
            "    > a",
            "    v b",
            "        > 3  <== selected",
            "        > 4",
            "        > new-dir",
            "          a-different-filename.tar.gz",
            "    > C",
            "      .dockerignore",
        ]
    );

    panel.update_in(cx, |panel, window, cx| {
        panel.rename(&Default::default(), window, cx)
    });
    cx.run_until_parked();
    assert_eq!(
        visible_entries_as_strings(&panel, 0..10, cx),
        &[
            "v root1",
            "    > .git",
            "    > a",
            "    v b",
            "        > [EDITOR: '3']  <== selected",
            "        > 4",
            "        > new-dir",
            "          a-different-filename.tar.gz",
            "    > C",
            "      .dockerignore",
        ]
    );

    // Dismiss the rename editor when it loses focus.
    workspace.update_in(cx, |_, window, _| window.blur());
    assert_eq!(
        visible_entries_as_strings(&panel, 0..10, cx),
        &[
            "v root1",
            "    > .git",
            "    > a",
            "    v b",
            "        > 3  <== selected",
            "        > 4",
            "        > new-dir",
            "          a-different-filename.tar.gz",
            "    > C",
            "      .dockerignore",
        ]
    );

    // Test empty filename and filename with only whitespace
    panel.update_in(cx, |panel, window, cx| panel.new_file(&NewFile, window, cx));
    cx.run_until_parked();
    assert_eq!(
        visible_entries_as_strings(&panel, 0..10, cx),
        &[
            "v root1",
            "    > .git",
            "    > a",
            "    v b",
            "        v 3",
            "              [EDITOR: '']  <== selected",
            "              Q",
            "        > 4",
            "        > new-dir",
            "          a-different-filename.tar.gz",
        ]
    );
    panel.update_in(cx, |panel, window, cx| {
        panel.filename_editor.update(cx, |editor, cx| {
            editor.set_text("", window, cx);
        });
        assert!(panel.confirm_edit(true, window, cx).is_none());
        panel.filename_editor.update(cx, |editor, cx| {
            editor.set_text("   ", window, cx);
        });
        assert!(panel.confirm_edit(true, window, cx).is_none());
        panel.cancel(&menu::Cancel, window, cx);
    });
    cx.run_until_parked();
    assert_eq!(
        visible_entries_as_strings(&panel, 0..10, cx),
        &[
            "v root1",
            "    > .git",
            "    > a",
            "    v b",
            "        v 3  <== selected",
            "              Q",
            "        > 4",
            "        > new-dir",
            "          a-different-filename.tar.gz",
            "    > C",
        ]
    );
}

#[gpui::test]
async fn test_rename_folder_with_dot_selects_whole_name(cx: &mut gpui::TestAppContext) {
    init_test(cx);

    let fs = FakeFs::new(cx.executor());
    fs.insert_tree(
        "/root1",
        json!({
            "my.folder": {},
            "archive.tar.gz": "",
        }),
    )
    .await;

    let project = Project::test(fs.clone(), ["/root1".as_ref()], cx).await;
    let window = cx.add_window(|window, cx| MultiWorkspace::test_new(project.clone(), window, cx));
    let workspace = window
        .read_with(cx, |mw, _| mw.workspace().clone())
        .unwrap();
    let cx = &mut VisualTestContext::from_window(window.into(), cx);
    let panel = workspace.update_in(cx, |workspace, window, cx| {
        let panel = ProjectPanel::new(workspace, window, cx);
        workspace.add_panel(panel.clone(), window, cx);
        panel
    });
    cx.run_until_parked();

    // Renaming a folder whose name contains a dot should pre-select the whole
    // name. The dot belongs to the directory name; it is not a file extension.
    select_path(&panel, "root1/my.folder", cx);
    panel.update_in(cx, |panel, window, cx| panel.rename(&Rename, window, cx));
    panel.update_in(cx, |panel, window, cx| {
        panel.filename_editor.update(cx, |editor, cx| {
            let selections = editor
                .selections
                .all::<MultiBufferOffset>(&editor.display_snapshot(cx));
            assert_eq!(selections.len(), 1);
            assert_eq!(selections[0].start, MultiBufferOffset(0));
            assert_eq!(
                selections[0].end,
                MultiBufferOffset("my.folder".len()),
                "重命名文件夹时,应选中整个名称,包括点"
            );
        });
        panel.cancel(&Cancel, window, cx);
    });
    cx.run_until_parked();

    // Files keep the existing behavior: the last extension stays unselected.
    select_path(&panel, "root1/archive.tar.gz", cx);
    panel.update_in(cx, |panel, window, cx| panel.rename(&Rename, window, cx));
    panel.update_in(cx, |panel, _, cx| {
        panel.filename_editor.update(cx, |editor, cx| {
            let selections = editor
                .selections
                .all::<MultiBufferOffset>(&editor.display_snapshot(cx));
            assert_eq!(
                selections[0].end,
                MultiBufferOffset("archive.tar".len()),
                "重命名文件时,应保持最后一个扩展名不被选中"
            );
        });
    });
}

#[gpui::test(iterations = 10)]
async fn test_adding_directories_via_file(cx: &mut gpui::TestAppContext) {
    init_test(cx);

    let fs = FakeFs::new(cx.executor());
    fs.insert_tree(
        "/root1",
        json!({
            ".dockerignore": "",
            ".git": {
                "HEAD": "",
            },
            "a": {
                "0": { "q": "", "r": "", "s": "" },
                "1": { "t": "", "u": "" },
                "2": { "v": "", "w": "", "x": "", "y": "" },
            },
            "b": {
                "3": { "Q": "" },
                "4": { "R": "", "S": "", "T": "", "U": "" },
            },
            "C": {
                "5": {},
                "6": { "V": "", "W": "" },
                "7": { "X": "" },
                "8": { "Y": {}, "Z": "" }
            }
        }),
    )
    .await;
    fs.insert_tree(
        "/root2",
        json!({
            "d": {
                "9": ""
            },
            "e": {}
        }),
    )
    .await;

    let project = Project::test(fs.clone(), ["/root1".as_ref(), "/root2".as_ref()], cx).await;
    let window = cx.add_window(|window, cx| MultiWorkspace::test_new(project.clone(), window, cx));
    let workspace = window
        .read_with(cx, |mw, _| mw.workspace().clone())
        .unwrap();
    let cx = &mut VisualTestContext::from_window(window.into(), cx);
    let panel = workspace.update_in(cx, |workspace, window, cx| {
        let panel = ProjectPanel::new(workspace, window, cx);
        workspace.add_panel(panel.clone(), window, cx);
        panel
    });
    cx.run_until_parked();

    select_path(&panel, "root1", cx);
    assert_eq!(
        visible_entries_as_strings(&panel, 0..10, cx),
        &[
            "v root1  <== selected",
            "    > .git",
            "    > a",
            "    > b",
            "    > C",
            "      .dockerignore",
            "v root2",
            "    > d",
            "    > e",
        ]
    );

    // Add a file with the root folder selected. The filename editor is placed
    // before the first file in the root folder.
    panel.update_in(cx, |panel, window, cx| panel.new_file(&NewFile, window, cx));
    cx.run_until_parked();
    panel.update_in(cx, |panel, window, cx| {
        assert!(panel.filename_editor.read(cx).is_focused(window));
    });
    cx.run_until_parked();
    assert_eq!(
        visible_entries_as_strings(&panel, 0..10, cx),
        &[
            "v root1",
            "    > .git",
            "    > a",
            "    > b",
            "    > C",
            "      [EDITOR: '']  <== selected",
            "      .dockerignore",
            "v root2",
            "    > d",
            "    > e",
        ]
    );

    let confirm = panel.update_in(cx, |panel, window, cx| {
        panel.filename_editor.update(cx, |editor, cx| {
            editor.set_text("/bdir1/dir2/the-new-filename", window, cx)
        });
        panel.confirm_edit(true, window, cx).unwrap()
    });

    assert_eq!(
        visible_entries_as_strings(&panel, 0..10, cx),
        &[
            "v root1",
            "    > .git",
            "    > a",
            "    > b",
            "    > C",
            "      [PROCESSING: 'bdir1/dir2/the-new-filename']  <== selected",
            "      .dockerignore",
            "v root2",
            "    > d",
            "    > e",
        ]
    );

    confirm.await.unwrap();
    cx.run_until_parked();
    assert_eq!(
        visible_entries_as_strings(&panel, 0..13, cx),
        &[
            "v root1",
            "    > .git",
            "    > a",
            "    > b",
            "    v bdir1",
            "        v dir2",
            "              the-new-filename  <== selected  <== marked",
            "    > C",
            "      .dockerignore",
            "v root2",
            "    > d",
            "    > e",
        ]
    );
}

#[gpui::test]
async fn test_adding_directory_via_file(cx: &mut gpui::TestAppContext) {
    init_test(cx);

    let fs = FakeFs::new(cx.executor());
    fs.insert_tree(
        path!("/root1"),
        json!({
            ".dockerignore": "",
            ".git": {
                "HEAD": "",
            },
        }),
    )
    .await;

    let project = Project::test(fs.clone(), [path!("/root1").as_ref()], cx).await;
    let window = cx.add_window(|window, cx| MultiWorkspace::test_new(project.clone(), window, cx));
    let workspace = window
        .read_with(cx, |mw, _| mw.workspace().clone())
        .unwrap();
    let cx = &mut VisualTestContext::from_window(window.into(), cx);
    let panel = workspace.update_in(cx, |workspace, window, cx| {
        let panel = ProjectPanel::new(workspace, window, cx);
        workspace.add_panel(panel.clone(), window, cx);
        panel
    });
    cx.run_until_parked();

    select_path(&panel, "root1", cx);
    assert_eq!(
        visible_entries_as_strings(&panel, 0..10, cx),
        &["v root1  <== selected", "    > .git", "      .dockerignore",]
    );

    // Add a file with the root folder selected. The filename editor is placed
    // before the first file in the root folder.
    panel.update_in(cx, |panel, window, cx| panel.new_file(&NewFile, window, cx));
    cx.run_until_parked();
    panel.update_in(cx, |panel, window, cx| {
        assert!(panel.filename_editor.read(cx).is_focused(window));
    });
    assert_eq!(
        visible_entries_as_strings(&panel, 0..10, cx),
        &[
            "v root1",
            "    > .git",
            "      [EDITOR: '']  <== selected",
            "      .dockerignore",
        ]
    );

    let confirm = panel.update_in(cx, |panel, window, cx| {
        // If we want to create a subdirectory, there should be no prefix slash.
        panel
            .filename_editor
            .update(cx, |editor, cx| editor.set_text("new_dir/", window, cx));
        panel.confirm_edit(true, window, cx).unwrap()
    });

    assert_eq!(
        visible_entries_as_strings(&panel, 0..10, cx),
        &[
            "v root1",
            "    > .git",
            "      [PROCESSING: 'new_dir']  <== selected",
            "      .dockerignore",
        ]
    );

    confirm.await.unwrap();
    cx.run_until_parked();
    assert_eq!(
        visible_entries_as_strings(&panel, 0..10, cx),
        &[
            "v root1",
            "    > .git",
            "    v new_dir  <== selected",
            "      .dockerignore",
        ]
    );

    // Test filename with whitespace
    select_path(&panel, "root1", cx);
    panel.update_in(cx, |panel, window, cx| panel.new_file(&NewFile, window, cx));
    let confirm = panel.update_in(cx, |panel, window, cx| {
        // If we want to create a subdirectory, there should be no prefix slash.
        panel
            .filename_editor
            .update(cx, |editor, cx| editor.set_text("new dir 2/", window, cx));
        panel.confirm_edit(true, window, cx).unwrap()
    });
    confirm.await.unwrap();
    cx.run_until_parked();
    assert_eq!(
        visible_entries_as_strings(&panel, 0..10, cx),
        &[
            "v root1",
            "    > .git",
            "    v new dir 2  <== selected",
            "    v new_dir",
            "      .dockerignore",
        ]
    );

    // Test filename ends with "\"
    #[cfg(target_os = "windows")]
    {
        select_path(&panel, "root1", cx);
        panel.update_in(cx, |panel, window, cx| panel.new_file(&NewFile, window, cx));
        let confirm = panel.update_in(cx, |panel, window, cx| {
            // If we want to create a subdirectory, there should be no prefix slash.
            panel
                .filename_editor
                .update(cx, |editor, cx| editor.set_text("new_dir_3\\", window, cx));
            panel.confirm_edit(true, window, cx).unwrap()
        });
        confirm.await.unwrap();
        cx.run_until_parked();
        assert_eq!(
            visible_entries_as_strings(&panel, 0..10, cx),
            &[
                "v root1",
                "    > .git",
                "    v new dir 2",
                "    v new_dir",
                "    v new_dir_3  <== selected",
                "      .dockerignore",
            ]
        );
    }
}

#[gpui::test]
async fn test_copy_paste(cx: &mut gpui::TestAppContext) {
    init_test(cx);

    let fs = FakeFs::new(cx.executor());
    fs.insert_tree(
        "/root1",
        json!({
            "one.two.txt": "",
            "one.txt": ""
        }),
    )
    .await;

    let project = Project::test(fs.clone(), ["/root1".as_ref()], cx).await;
    let window = cx.add_window(|window, cx| MultiWorkspace::test_new(project.clone(), window, cx));
    let workspace = window
        .read_with(cx, |mw, _| mw.workspace().clone())
        .unwrap();
    let cx = &mut VisualTestContext::from_window(window.into(), cx);
    let panel = workspace.update_in(cx, ProjectPanel::new);
    cx.run_until_parked();

    panel.update_in(cx, |panel, window, cx| {
        panel.select_next(&Default::default(), window, cx);
        panel.select_next(&Default::default(), window, cx);
    });

    assert_eq!(
        visible_entries_as_strings(&panel, 0..50, cx),
        &[
            //
            "v root1",
            "      one.txt  <== selected",
            "      one.two.txt",
        ]
    );

    // Regression test - file name is created correctly when
    // the copied file's name contains multiple dots.
    panel.update_in(cx, |panel, window, cx| {
        panel.copy(&Default::default(), window, cx);
        panel.paste(&Default::default(), window, cx);
    });
    cx.executor().run_until_parked();
    panel.update_in(cx, |panel, window, cx| {
        assert!(panel.filename_editor.read(cx).is_focused(window));
    });
    assert_eq!(
        visible_entries_as_strings(&panel, 0..50, cx),
        &[
            //
            "v root1",
            "      one.txt",
            "      [EDITOR: 'one copy.txt']  <== selected  <== marked",
            "      one.two.txt",
        ]
    );

    panel.update_in(cx, |panel, window, cx| {
        panel.filename_editor.update(cx, |editor, cx| {
            let file_name_selections = editor
                .selections
                .all::<MultiBufferOffset>(&editor.display_snapshot(cx));
            assert_eq!(
                file_name_selections.len(),
                1,
                "文件编辑应只有一个选中项,但实际有: {file_name_selections:?}"
            );
            let file_name_selection = &file_name_selections[0];
            assert_eq!(
                file_name_selection.start,
                MultiBufferOffset(0),
                "应从文件名开头开始选中"
            );
            assert_eq!(
                file_name_selection.end,
                MultiBufferOffset("one copy".len()),
                "应选中文件名消歧部分直到扩展名"
            );
        });
        assert!(panel.confirm_edit(true, window, cx).is_none());
    });

    panel.update_in(cx, |panel, window, cx| {
        panel.paste(&Default::default(), window, cx);
    });
    cx.executor().run_until_parked();
    panel.update_in(cx, |panel, window, cx| {
        assert!(panel.filename_editor.read(cx).is_focused(window));
    });
    assert_eq!(
        visible_entries_as_strings(&panel, 0..50, cx),
        &[
            //
            "v root1",
            "      one.txt",
            "      one copy.txt",
            "      [EDITOR: 'one copy 1.txt']  <== selected  <== marked",
            "      one.two.txt",
        ]
    );

    panel.update_in(cx, |panel, window, cx| {
        assert!(panel.confirm_edit(true, window, cx).is_none())
    });
}

#[gpui::test]
async fn test_cut_paste(cx: &mut gpui::TestAppContext) {
    init_test(cx);

    let fs = FakeFs::new(cx.executor());
    fs.insert_tree(
        "/root",
        json!({
            "one.txt": "",
            "two.txt": "",
            "a": {},
            "b": {}
        }),
    )
    .await;

    let project = Project::test(fs.clone(), ["/root".as_ref()], cx).await;
    let window = cx.add_window(|window, cx| MultiWorkspace::test_new(project.clone(), window, cx));
    let workspace = window
        .read_with(cx, |mw, _| mw.workspace().clone())
        .unwrap();
    let cx = &mut VisualTestContext::from_window(window.into(), cx);
    let panel = workspace.update_in(cx, ProjectPanel::new);
    cx.run_until_parked();

    select_path_with_mark(&panel, "root/one.txt", cx);
    select_path_with_mark(&panel, "root/two.txt", cx);

    assert_eq!(
        visible_entries_as_strings(&panel, 0..50, cx),
        &[
            "v root",
            "    > a",
            "    > b",
            "      one.txt  <== marked",
            "      two.txt  <== selected  <== marked",
        ]
    );

    panel.update_in(cx, |panel, window, cx| {
        panel.cut(&Default::default(), window, cx);
    });

    select_path(&panel, "root/a", cx);

    panel.update_in(cx, |panel, window, cx| {
        panel.paste(&Default::default(), window, cx);
        panel.update_visible_entries(None, false, false, window, cx);
    });
    cx.executor().run_until_parked();

    assert_eq!(
        visible_entries_as_strings(&panel, 0..50, cx),
        &[
            "v root",
            "    v a",
            "          one.txt  <== marked",
            "          two.txt  <== selected  <== marked",
            "    > b",
        ],
        "Cut entries should be moved on first paste."
    );

    panel.update_in(cx, |panel, window, cx| {
        panel.cancel(&menu::Cancel {}, window, cx)
    });
    cx.executor().run_until_parked();

    select_path(&panel, "root/b", cx);

    panel.update_in(cx, |panel, window, cx| {
        panel.paste(&Default::default(), window, cx);
    });
    cx.executor().run_until_parked();

    assert_eq!(
        visible_entries_as_strings(&panel, 0..50, cx),
        &[
            "v root",
            "    v a",
            "          one.txt",
            "          two.txt",
            "    v b",
            "          one.txt",
            "          two.txt  <== selected",
        ],
        "剪切的项目在第二次粘贴时只应被复制!"
    );
}

#[gpui::test]
async fn test_cut_paste_between_different_worktrees(cx: &mut gpui::TestAppContext) {
    init_test(cx);

    let fs = FakeFs::new(cx.executor());
    fs.insert_tree(
        "/root1",
        json!({
            "one.txt": "",
            "two.txt": "",
            "three.txt": "",
            "a": {
                "0": { "q": "", "r": "", "s": "" },
                "1": { "t": "", "u": "" },
                "2": { "v": "", "w": "", "x": "", "y": "" },
            },
        }),
    )
    .await;

    fs.insert_tree(
        "/root2",
        json!({
            "one.txt": "",
            "two.txt": "",
            "four.txt": "",
            "b": {
                "3": { "Q": "" },
                "4": { "R": "", "S": "", "T": "", "U": "" },
            },
        }),
    )
    .await;

    let project = Project::test(fs.clone(), ["/root1".as_ref(), "/root2".as_ref()], cx).await;
    let window = cx.add_window(|window, cx| MultiWorkspace::test_new(project.clone(), window, cx));
    let workspace = window
        .read_with(cx, |mw, _| mw.workspace().clone())
        .unwrap();
    let cx = &mut VisualTestContext::from_window(window.into(), cx);
    let panel = workspace.update_in(cx, ProjectPanel::new);
    cx.run_until_parked();

    select_path(&panel, "root1/three.txt", cx);
    panel.update_in(cx, |panel, window, cx| {
        panel.cut(&Default::default(), window, cx);
    });

    select_path(&panel, "root2/one.txt", cx);
    panel.update_in(cx, |panel, window, cx| {
        panel.select_next(&Default::default(), window, cx);
        panel.paste(&Default::default(), window, cx);
    });
    cx.executor().run_until_parked();
    assert_eq!(
        visible_entries_as_strings(&panel, 0..50, cx),
        &[
            //
            "v root1",
            "    > a",
            "      one.txt",
            "      two.txt",
            "v root2",
            "    > b",
            "      four.txt",
            "      one.txt",
            "      three.txt  <== selected  <== marked",
            "      two.txt",
        ]
    );

    select_path(&panel, "root1/a", cx);
    panel.update_in(cx, |panel, window, cx| {
        panel.cut(&Default::default(), window, cx);
    });
    select_path(&panel, "root2/two.txt", cx);
    panel.update_in(cx, |panel, window, cx| {
        panel.select_next(&Default::default(), window, cx);
        panel.paste(&Default::default(), window, cx);
    });

    cx.executor().run_until_parked();
    assert_eq!(
        visible_entries_as_strings(&panel, 0..50, cx),
        &[
            //
            "v root1",
            "      one.txt",
            "      two.txt",
            "v root2",
            "    > a  <== selected",
            "    > b",
            "      four.txt",
            "      one.txt",
            "      three.txt  <== marked",
            "      two.txt",
        ]
    );
}

#[gpui::test]
async fn test_copy_paste_between_different_worktrees(cx: &mut gpui::TestAppContext) {
    init_test(cx);

    let fs = FakeFs::new(cx.executor());
    fs.insert_tree(
        "/root1",
        json!({
            "one.txt": "",
            "two.txt": "",
            "three.txt": "",
            "a": {
                "0": { "q": "", "r": "", "s": "" },
                "1": { "t": "", "u": "" },
                "2": { "v": "", "w": "", "x": "", "y": "" },
            },
        }),
    )
    .await;

    fs.insert_tree(
        "/root2",
        json!({
            "one.txt": "",
            "two.txt": "",
            "four.txt": "",
            "b": {
                "3": { "Q": "" },
                "4": { "R": "", "S": "", "T": "", "U": "" },
            },
        }),
    )
    .await;

    let project = Project::test(fs.clone(), ["/root1".as_ref(), "/root2".as_ref()], cx).await;
    let window = cx.add_window(|window, cx| MultiWorkspace::test_new(project.clone(), window, cx));
    let workspace = window
        .read_with(cx, |mw, _| mw.workspace().clone())
        .unwrap();
    let cx = &mut VisualTestContext::from_window(window.into(), cx);
    let panel = workspace.update_in(cx, ProjectPanel::new);
    cx.run_until_parked();

    select_path(&panel, "root1/three.txt", cx);
    panel.update_in(cx, |panel, window, cx| {
        panel.copy(&Default::default(), window, cx);
    });

    select_path(&panel, "root2/one.txt", cx);
    panel.update_in(cx, |panel, window, cx| {
        panel.select_next(&Default::default(), window, cx);
        panel.paste(&Default::default(), window, cx);
    });
    cx.executor().run_until_parked();
    assert_eq!(
        visible_entries_as_strings(&panel, 0..50, cx),
        &[
            //
            "v root1",
            "    > a",
            "      one.txt",
            "      three.txt",
            "      two.txt",
            "v root2",
            "    > b",
            "      four.txt",
            "      one.txt",
            "      three.txt  <== selected  <== marked",
            "      two.txt",
        ]
    );

    select_path(&panel, "root1/three.txt", cx);
    panel.update_in(cx, |panel, window, cx| {
        panel.copy(&Default::default(), window, cx);
    });
    select_path(&panel, "root2/two.txt", cx);
    panel.update_in(cx, |panel, window, cx| {
        panel.select_next(&Default::default(), window, cx);
        panel.paste(&Default::default(), window, cx);
    });

    cx.executor().run_until_parked();
    assert_eq!(
        visible_entries_as_strings(&panel, 0..50, cx),
        &[
            //
            "v root1",
            "    > a",
            "      one.txt",
            "      three.txt",
            "      two.txt",
            "v root2",
            "    > b",
            "      four.txt",
            "      one.txt",
            "      three.txt",
            "      [EDITOR: 'three copy.txt']  <== selected  <== marked",
            "      two.txt",
        ]
    );

    panel.update_in(cx, |panel, window, cx| {
        panel.cancel(&menu::Cancel {}, window, cx)
    });
    cx.executor().run_until_parked();

    select_path(&panel, "root1/a", cx);
    panel.update_in(cx, |panel, window, cx| {
        panel.copy(&Default::default(), window, cx);
    });
    select_path(&panel, "root2/two.txt", cx);
    panel.update_in(cx, |panel, window, cx| {
        panel.select_next(&Default::default(), window, cx);
        panel.paste(&Default::default(), window, cx);
    });

    cx.executor().run_until_parked();
    assert_eq!(
        visible_entries_as_strings(&panel, 0..50, cx),
        &[
            //
            "v root1",
            "    > a",
            "      one.txt",
            "      three.txt",
            "      two.txt",
            "v root2",
            "    > a  <== selected",
            "    > b",
            "      four.txt",
            "      one.txt",
            "      three.txt",
            "      three copy.txt",
            "      two.txt",
        ]
    );
}

#[gpui::test]
async fn test_copy_paste_directory(cx: &mut gpui::TestAppContext) {
    init_test(cx);

    let fs = FakeFs::new(cx.executor());
    fs.insert_tree(
        "/root",
        json!({
            "a": {
                "one.txt": "",
                "two.txt": "",
                "inner_dir": {
                    "three.txt": "",
                    "four.txt": "",
                }
            },
            "b": {},
            "d.1.20": {
                "default.conf": "",
            }
        }),
    )
    .await;

    let project = Project::test(fs.clone(), ["/root".as_ref()], cx).await;
    let window = cx.add_window(|window, cx| MultiWorkspace::test_new(project.clone(), window, cx));
    let workspace = window
        .read_with(cx, |mw, _| mw.workspace().clone())
        .unwrap();
    let cx = &mut VisualTestContext::from_window(window.into(), cx);
    let panel = workspace.update_in(cx, ProjectPanel::new);
    cx.run_until_parked();

    select_path(&panel, "root/a", cx);
    panel.update_in(cx, |panel, window, cx| {
        panel.copy(&Default::default(), window, cx);
        panel.select_next(&Default::default(), window, cx);
        panel.paste(&Default::default(), window, cx);
    });
    cx.executor().run_until_parked();

    let pasted_dir = find_project_entry(&panel, "root/b/a", cx);
    assert_ne!(pasted_dir, None, "粘贴的目录应有一个条目");

    let pasted_dir_file = find_project_entry(&panel, "root/b/a/one.txt", cx);
    assert_ne!(
        pasted_dir_file, None,
        "粘贴的目录文件应有一个条目"
    );

    let pasted_dir_inner_dir = find_project_entry(&panel, "root/b/a/inner_dir", cx);
    assert_ne!(
        pasted_dir_inner_dir, None,
        "粘贴目录内的目录应有一个条目"
    );

    toggle_expand_dir(&panel, "root/b/a", cx);
    toggle_expand_dir(&panel, "root/b/a/inner_dir", cx);

    assert_eq!(
        visible_entries_as_strings(&panel, 0..50, cx),
        &[
            //
            "v root",
            "    > a",
            "    v b",
            "        v a",
            "            v inner_dir  <== selected",
            "                  four.txt",
            "                  three.txt",
            "              one.txt",
            "              two.txt",
            "    > d.1.20",
        ]
    );

    select_path(&panel, "root", cx);
    panel.update_in(cx, |panel, window, cx| {
        panel.paste(&Default::default(), window, cx)
    });
    cx.executor().run_until_parked();
    assert_eq!(
        visible_entries_as_strings(&panel, 0..50, cx),
        &[
            //
            "v root",
            "    > a",
            "    > [EDITOR: 'a copy']  <== selected",
            "    v b",
            "        v a",
            "            v inner_dir",
            "                  four.txt",
            "                  three.txt",
            "              one.txt",
            "              two.txt",
            "    > d.1.20",
        ]
    );

    let confirm = panel.update_in(cx, |panel, window, cx| {
        panel
            .filename_editor
            .update(cx, |editor, cx| editor.set_text("c", window, cx));
        panel.confirm_edit(true, window, cx).unwrap()
    });
    assert_eq!(
        visible_entries_as_strings(&panel, 0..50, cx),
        &[
            //
            "v root",
            "    > a",
            "    > [PROCESSING: 'c']  <== selected",
            "    v b",
            "        v a",
            "            v inner_dir",
            "                  four.txt",
            "                  three.txt",
            "              one.txt",
            "              two.txt",
            "    > d.1.20",
        ]
    );

    confirm.await.unwrap();

    panel.update_in(cx, |panel, window, cx| {
        panel.paste(&Default::default(), window, cx)
    });
    cx.executor().run_until_parked();
    assert_eq!(
        visible_entries_as_strings(&panel, 0..50, cx),
        &[
            //
            "v root",
            "    > a",
            "    v b",
            "        v a",
            "            v inner_dir",
            "                  four.txt",
            "                  three.txt",
            "              one.txt",
            "              two.txt",
            "    v c",
            "        > a  <== selected",
            "        > inner_dir",
            "          one.txt",
            "          two.txt",
            "    > d.1.20",
        ]
    );

    select_path(&panel, "root/d.1.20", cx);
    panel.update_in(cx, |panel, window, cx| {
        panel.copy(&Default::default(), window, cx);
        panel.paste(&Default::default(), window, cx);
    });
    cx.executor().run_until_parked();
    assert_eq!(
        visible_entries_as_strings(&panel, 0..50, cx),
        &[
            //
            "v root",
            "    > a",
            "    v b",
            "        v a",
            "            v inner_dir",
            "                  four.txt",
            "                  three.txt",
            "              one.txt",
            "              two.txt",
            "    v c",
            "        > a",
            "        > inner_dir",
            "          one.txt",
            "          two.txt",
            "    v d.1.20",
            "          default.conf",
            "    > [EDITOR: 'd.1.20 copy']  <== selected",
        ],
        "带点的目录名在消歧时不应在点处拆分"
    );
}

#[gpui::test]
async fn test_copy_paste_directory_with_sibling_file(cx: &mut gpui::TestAppContext) {
    init_test(cx);

    let fs = FakeFs::new(cx.executor());
    fs.insert_tree(
        "/test",
        json!({
            "dir1": {
                "a.txt": "",
                "b.txt": "",
            },
            "dir2": {},
            "c.txt": "",
            "d.txt": "",
        }),
    )
    .await;

    let project = Project::test(fs.clone(), ["/test".as_ref()], cx).await;
    let window = cx.add_window(|window, cx| MultiWorkspace::test_new(project.clone(), window, cx));
    let workspace = window
        .read_with(cx, |mw, _| mw.workspace().clone())
        .unwrap();
    let cx = &mut VisualTestContext::from_window(window.into(), cx);
    let panel = workspace.update_in(cx, ProjectPanel::new);
    cx.run_until_parked();

    toggle_expand_dir(&panel, "test/dir1", cx);

    cx.simulate_modifiers_change(gpui::Modifiers {
        control: true,
        ..Default::default()
    });

    select_path_with_mark(&panel, "test/dir1", cx);
    select_path_with_mark(&panel, "test/c.txt", cx);

    assert_eq!(
        visible_entries_as_strings(&panel, 0..15, cx),
        &[
            "v test",
            "    v dir1  <== marked",
            "          a.txt",
            "          b.txt",
            "    > dir2",
            "      c.txt  <== selected  <== marked",
            "      d.txt",
        ],
        "复制 dir1 和 c.txt 之前的初始状态"
    );

    panel.update_in(cx, |panel, window, cx| {
        panel.copy(&Default::default(), window, cx);
    });
    select_path(&panel, "test/dir2", cx);
    panel.update_in(cx, |panel, window, cx| {
        panel.paste(&Default::default(), window, cx);
    });
    cx.executor().run_until_parked();

    toggle_expand_dir(&panel, "test/dir2/dir1", cx);

    assert_eq!(
        visible_entries_as_strings(&panel, 0..15, cx),
        &[
            "v test",
            "    v dir1  <== marked",
            "          a.txt",
            "          b.txt",
            "    v dir2",
            "        v dir1  <== selected",
            "              a.txt",
            "              b.txt",
            "          c.txt",
            "      c.txt  <== marked",
            "      d.txt",
        ],
        "应将 dir1 和 c.txt 复制到 dir2 中"
    );

    // Disambiguating multiple files should not open the rename editor.
    select_path(&panel, "test/dir2", cx);
    panel.update_in(cx, |panel, window, cx| {
        panel.paste(&Default::default(), window, cx);
    });
    cx.executor().run_until_parked();

    assert_eq!(
        visible_entries_as_strings(&panel, 0..15, cx),
        &[
            "v test",
            "    v dir1  <== marked",
            "          a.txt",
            "          b.txt",
            "    v dir2",
            "        v dir1",
            "              a.txt",
            "              b.txt",
            "        > dir1 copy  <== selected",
            "          c.txt",
            "          c copy.txt",
            "      c.txt  <== marked",
            "      d.txt",
        ],
        "应将 dir1 和 c.txt 复制到 dir2 中,并在不打开重命名编辑器的情况下对它们进行消歧"
    );
}

#[gpui::test]
async fn test_copy_paste_nested_and_root_entries(cx: &mut gpui::TestAppContext) {
    init_test(cx);

    let fs = FakeFs::new(cx.executor());
    fs.insert_tree(
        "/test",
        json!({
            "dir1": {
                "a.txt": "",
                "b.txt": "",
            },
            "dir2": {},
            "c.txt": "",
            "d.txt": "",
        }),
    )
    .await;

    let project = Project::test(fs.clone(), ["/test".as_ref()], cx).await;
    let window = cx.add_window(|window, cx| MultiWorkspace::test_new(project.clone(), window, cx));
    let workspace = window
        .read_with(cx, |mw, _| mw.workspace().clone())
        .unwrap();
    let cx = &mut VisualTestContext::from_window(window.into(), cx);
    let panel = workspace.update_in(cx, ProjectPanel::new);
    cx.run_until_parked();

    toggle_expand_dir(&panel, "test/dir1", cx);

    cx.simulate_modifiers_change(gpui::Modifiers {
        control: true,
        ..Default::default()
    });

    select_path_with_mark(&panel, "test/dir1/a.txt", cx);
    select_path_with_mark(&panel, "test/dir1", cx);
    select_path_with_mark(&panel, "test/c.txt", cx);

    assert_eq!(
        visible_entries_as_strings(&panel, 0..15, cx),
        &[
            "v test",
            "    v dir1  <== marked",
            "          a.txt  <== marked",
            "          b.txt",
            "    > dir2",
            "      c.txt  <== selected  <== marked",
            "      d.txt",
        ],
        "复制 a.txt、dir1 和 c.txt 之前的初始状态"
    );

    panel.update_in(cx, |panel, window, cx| {
        panel.copy(&Default::default(), window, cx);
    });
    select_path(&panel, "test/dir2", cx);
    panel.update_in(cx, |panel, window, cx| {
        panel.paste(&Default::default(), window, cx);
    });
    cx.executor().run_until_parked();

    toggle_expand_dir(&panel, "test/dir2/dir1", cx);

    assert_eq!(
        visible_entries_as_strings(&panel, 0..20, cx),
        &[
            "v test",
            "    v dir1  <== marked",
            "          a.txt  <== marked",
            "          b.txt",
            "    v dir2",
            "        v dir1  <== selected",
            "              a.txt",
            "              b.txt",
            "          c.txt",
            "      c.txt  <== marked",
            "      d.txt",
        ],
        "应将 dir1 和 c.txt 复制到 dir2 中。a.txt 已存在于复制的 dir1 中。"
    );
}

#[gpui::test]
async fn test_paste_external_paths(cx: &mut gpui::TestAppContext) {
    init_test(cx);
    set_auto_open_settings(
        cx,
        ProjectPanelAutoOpenSettings {
            on_drop: Some(false),
            ..Default::default()
        },
    );

    let fs = FakeFs::new(cx.executor());
    fs.insert_tree(
        path!("/root"),
        json!({
            "subdir": {}
        }),
    )
    .await;

    fs.insert_tree(
        path!("/external"),
        json!({
            "new_file.rs": "fn main() {}"
        }),
    )
    .await;

    let project = Project::test(fs.clone(), [path!("/root").as_ref()], cx).await;
    let window = cx.add_window(|window, cx| MultiWorkspace::test_new(project.clone(), window, cx));
    let workspace = window
        .read_with(cx, |mw, _| mw.workspace().clone())
        .unwrap();
    let cx = &mut VisualTestContext::from_window(window.into(), cx);
    let panel = workspace.update_in(cx, ProjectPanel::new);
    cx.run_until_parked();

    cx.write_to_clipboard(ClipboardItem {
        entries: vec![GpuiClipboardEntry::ExternalPaths(ExternalPaths(
            smallvec::smallvec![PathBuf::from(path!("/external/new_file.rs"))],
        ))],
    });

    select_path(&panel, "root/subdir", cx);
    panel.update_in(cx, |panel, window, cx| {
        panel.paste(&Default::default(), window, cx);
    });
    cx.executor().run_until_parked();

    assert_eq!(
        visible_entries_as_strings(&panel, 0..50, cx),
        &[
            "v root",
            "    v subdir",
            "          new_file.rs  <== selected",
        ],
    );
}

#[gpui::test]
async fn test_copy_and_cut_write_to_system_clipboard(cx: &mut gpui::TestAppContext) {
    init_test(cx);

    let fs = FakeFs::new(cx.executor());
    fs.insert_tree(
        path!("/root"),
        json!({
            "file_a.txt": "",
            "file_b.txt": ""
        }),
    )
    .await;

    let project = Project::test(fs.clone(), [path!("/root").as_ref()], cx).await;
    let window = cx.add_window(|window, cx| MultiWorkspace::test_new(project.clone(), window, cx));
    let workspace = window
        .read_with(cx, |mw, _| mw.workspace().clone())
        .unwrap();
    let cx = &mut VisualTestContext::from_window(window.into(), cx);
    let panel = workspace.update_in(cx, ProjectPanel::new);
    cx.run_until_parked();

    select_path(&panel, "root/file_a.txt", cx);
    panel.update_in(cx, |panel, window, cx| {
        panel.copy(&Default::default(), window, cx);
    });

    let clipboard = cx
        .read_from_clipboard()
        .expect("复制后剪贴板应有内容");
    let text = clipboard.text().expect("剪贴板应包含文本");
    assert!(
        text.contains("file_a.txt"),
        "系统剪贴板应包含复制的文件路径,实际为:{text}"
    );

    select_path(&panel, "root/file_b.txt", cx);
    panel.update_in(cx, |panel, window, cx| {
        panel.cut(&Default::default(), window, cx);
    });

    let clipboard = cx
        .read_from_clipboard()
        .expect("剪切后剪贴板应有内容");
    let text = clipboard.text().expect("剪贴板应包含文本");
    assert!(
        text.contains("file_b.txt"),
        "系统剪贴板应包含剪切的文件路径,实际为:{text}"
    );
}

#[gpui::test]
async fn test_remove_opened_file(cx: &mut gpui::TestAppContext) {
    init_test_with_editor(cx);

    let fs = FakeFs::new(cx.executor());
    fs.insert_tree(
        path!("/src"),
        json!({
            "test": {
                "first.rs": "// First Rust file",
                "second.rs": "// Second Rust file",
                "third.rs": "// Third Rust file",
            }
        }),
    )
    .await;

    let project = Project::test(fs.clone(), [path!("/src").as_ref()], cx).await;
    let window = cx.add_window(|window, cx| MultiWorkspace::test_new(project.clone(), window, cx));
    let workspace = window
        .read_with(cx, |mw, _| mw.workspace().clone())
        .unwrap();
    let cx = &mut VisualTestContext::from_window(window.into(), cx);
    let panel = workspace.update_in(cx, ProjectPanel::new);
    cx.run_until_parked();

    toggle_expand_dir(&panel, "src/test", cx);
    select_path(&panel, "src/test/first.rs", cx);
    panel.update_in(cx, |panel, window, cx| panel.open(&Open, window, cx));
    cx.executor().run_until_parked();
    assert_eq!(
        visible_entries_as_strings(&panel, 0..10, cx),
        &[
            "v src",
            "    v test",
            "          first.rs  <== selected  <== marked",
            "          second.rs",
            "          third.rs"
        ]
    );
    ensure_single_file_is_opened(&workspace, "test/first.rs", cx);

    submit_deletion(&panel, cx);
    assert_eq!(
        visible_entries_as_strings(&panel, 0..10, cx),
        &[
            "v src",
            "    v test",
            "          second.rs  <== selected",
            "          third.rs"
        ],
        "项目面板中不应有已删除的文件,且没有其他文件被选中"
    );
    ensure_no_open_items_and_panes(&workspace, cx);

    panel.update_in(cx, |panel, window, cx| panel.open(&Open, window, cx));
    cx.executor().run_until_parked();
    assert_eq!(
        visible_entries_as_strings(&panel, 0..10, cx),
        &[
            "v src",
            "    v test",
            "          second.rs  <== selected  <== marked",
            "          third.rs"
        ]
    );
    ensure_single_file_is_opened(&workspace, "test/second.rs", cx);

    workspace.update_in(cx, |workspace, window, cx| {
        let active_items = workspace
            .panes()
            .iter()
            .filter_map(|pane| pane.read(cx).active_item())
            .collect::<Vec<_>>();
        assert_eq!(active_items.len(), 1);
        let open_editor = active_items
            .into_iter()
            .next()
            .unwrap()
            .downcast::<Editor>()
            .expect("打开的项目应为编辑器");
        open_editor.update(cx, |editor, cx| {
            editor.set_text("另一段文本!", window, cx)
        });
    });
    submit_deletion_skipping_prompt(&panel, cx);
    assert_eq!(
        visible_entries_as_strings(&panel, 0..10, cx),
        &["v src", "    v test", "          third.rs  <== selected"],
        "项目面板中不应有已删除的文件,且还剩最后一个文件"
    );
    ensure_no_open_items_and_panes(&workspace, cx);
}

#[gpui::test]
async fn test_auto_open_new_file_when_enabled(cx: &mut gpui::TestAppContext) {
    init_test_with_editor(cx);
    set_auto_open_settings(
        cx,
        ProjectPanelAutoOpenSettings {
            on_create: Some(true),
            ..Default::default()
        },
    );

    let fs = FakeFs::new(cx.executor());
    fs.insert_tree(path!("/root"), json!({})).await;

    let project = Project::test(fs.clone(), [path!("/root").as_ref()], cx).await;
    let window = cx.add_window(|window, cx| MultiWorkspace::test_new(project.clone(), window, cx));
    let workspace = window
        .read_with(cx, |mw, _| mw.workspace().clone())
        .unwrap();
    let cx = &mut VisualTestContext::from_window(window.into(), cx);
    let panel = workspace.update_in(cx, ProjectPanel::new);
    cx.run_until_parked();

    panel.update_in(cx, |panel, window, cx| panel.new_file(&NewFile, window, cx));
    cx.run_until_parked();
    panel
        .update_in(cx, |panel, window, cx| {
            panel.filename_editor.update(cx, |editor, cx| {
                editor.set_text("auto-open.rs", window, cx);
            });
            panel.confirm_edit(true, window, cx).unwrap()
        })
        .await
        .unwrap();
    cx.run_until_parked();

    ensure_single_file_is_opened(&workspace, "auto-open.rs", cx);
}

#[gpui::test]
async fn test_auto_open_new_file_when_disabled(cx: &mut gpui::TestAppContext) {
    init_test_with_editor(cx);
    set_auto_open_settings(
        cx,
        ProjectPanelAutoOpenSettings {
            on_create: Some(false),
            ..Default::default()
        },
    );

    let fs = FakeFs::new(cx.executor());
    fs.insert_tree(path!("/root"), json!({})).await;

    let project = Project::test(fs.clone(), [path!("/root").as_ref()], cx).await;
    let window = cx.add_window(|window, cx| MultiWorkspace::test_new(project.clone(), window, cx));
    let workspace = window
        .read_with(cx, |mw, _| mw.workspace().clone())
        .unwrap();
    let cx = &mut VisualTestContext::from_window(window.into(), cx);
    let panel = workspace.update_in(cx, ProjectPanel::new);
    cx.run_until_parked();

    panel.update_in(cx, |panel, window, cx| panel.new_file(&NewFile, window, cx));
    cx.run_until_parked();
    panel
        .update_in(cx, |panel, window, cx| {
            panel.filename_editor.update(cx, |editor, cx| {
                editor.set_text("manual-open.rs", window, cx);
            });
            panel.confirm_edit(true, window, cx).unwrap()
        })
        .await
        .unwrap();
    cx.run_until_parked();

    ensure_no_open_items_and_panes(&workspace, cx);
}

#[gpui::test]
async fn test_auto_open_on_paste_when_enabled(cx: &mut gpui::TestAppContext) {
    init_test_with_editor(cx);
    set_auto_open_settings(
        cx,
        ProjectPanelAutoOpenSettings {
            on_paste: Some(true),
            ..Default::default()
        },
    );

    let fs = FakeFs::new(cx.executor());
    fs.insert_tree(
        path!("/root"),
        json!({
            "src": {
                "original.rs": ""
            },
            "target": {}
        }),
    )
    .await;

    let project = Project::test(fs.clone(), [path!("/root").as_ref()], cx).await;
    let window = cx.add_window(|window, cx| MultiWorkspace::test_new(project.clone(), window, cx));
    let workspace = window
        .read_with(cx, |mw, _| mw.workspace().clone())
        .unwrap();
    let cx = &mut VisualTestContext::from_window(window.into(), cx);
    let panel = workspace.update_in(cx, ProjectPanel::new);
    cx.run_until_parked();

    toggle_expand_dir(&panel, "root/src", cx);
    toggle_expand_dir(&panel, "root/target", cx);

    select_path(&panel, "root/src/original.rs", cx);
    panel.update_in(cx, |panel, window, cx| {
        panel.copy(&Default::default(), window, cx);
    });

    select_path(&panel, "root/target", cx);
    panel.update_in(cx, |panel, window, cx| {
        panel.paste(&Default::default(), window, cx);
    });
    cx.executor().run_until_parked();

    ensure_single_file_is_opened(&workspace, "target/original.rs", cx);
}

#[gpui::test]
async fn test_auto_open_on_paste_when_disabled(cx: &mut gpui::TestAppContext) {
    init_test_with_editor(cx);
    set_auto_open_settings(
        cx,
        ProjectPanelAutoOpenSettings {
            on_paste: Some(false),
            ..Default::default()
        },
    );

    let fs = FakeFs::new(cx.executor());
    fs.insert_tree(
        path!("/root"),
        json!({
            "src": {
                "original.rs": ""
            },
            "target": {}
        }),
    )
    .await;

    let project = Project::test(fs.clone(), [path!("/root").as_ref()], cx).await;
    let window = cx.add_window(|window, cx| MultiWorkspace::test_new(project.clone(), window, cx));
    let workspace = window
        .read_with(cx, |mw, _| mw.workspace().clone())
        .unwrap();
    let cx = &mut VisualTestContext::from_window(window.into(), cx);
    let panel = workspace.update_in(cx, ProjectPanel::new);
    cx.run_until_parked();

    toggle_expand_dir(&panel, "root/src", cx);
    toggle_expand_dir(&panel, "root/target", cx);

    select_path(&panel, "root/src/original.rs", cx);
    panel.update_in(cx, |panel, window, cx| {
        panel.copy(&Default::default(), window, cx);
    });

    select_path(&panel, "root/target", cx);
    panel.update_in(cx, |panel, window, cx| {
        panel.paste(&Default::default(), window, cx);
    });
    cx.executor().run_until_parked();

    ensure_no_open_items_and_panes(&workspace, cx);
    assert!(
        find_project_entry(&panel, "root/target/original.rs", cx).is_some(),
        "即使禁用自动打开,粘贴的条目也应存在"
    );
}

#[gpui::test]
async fn test_auto_open_on_drop_when_enabled(cx: &mut gpui::TestAppContext) {
    init_test_with_editor(cx);
    set_auto_open_settings(
        cx,
        ProjectPanelAutoOpenSettings {
            on_drop: Some(true),
            ..Default::default()
        },
    );

    let fs = FakeFs::new(cx.executor());
    fs.insert_tree(path!("/root"), json!({})).await;

    let temp_dir = tempfile::tempdir().unwrap();
    let external_path = temp_dir.path().join("dropped.rs");
    std::fs::write(&external_path, "// dropped").unwrap();
    fs.insert_tree_from_real_fs(temp_dir.path(), temp_dir.path())
        .await;

    let project = Project::test(fs.clone(), [path!("/root").as_ref()], cx).await;
    let window = cx.add_window(|window, cx| MultiWorkspace::test_new(project.clone(), window, cx));
    let workspace = window
        .read_with(cx, |mw, _| mw.workspace().clone())
        .unwrap();
    let cx = &mut VisualTestContext::from_window(window.into(), cx);
    let panel = workspace.update_in(cx, ProjectPanel::new);
    cx.run_until_parked();

    let root_entry = find_project_entry(&panel, "root", cx).unwrap();
    panel.update_in(cx, |panel, window, cx| {
        panel.drop_external_files(std::slice::from_ref(&external_path), root_entry, window, cx);
    });
    cx.executor().run_until_parked();

    ensure_single_file_is_opened(&workspace, "dropped.rs", cx);
}

#[gpui::test]
async fn test_auto_open_on_drop_when_disabled(cx: &mut gpui::TestAppContext) {
    init_test_with_editor(cx);
    set_auto_open_settings(
        cx,
        ProjectPanelAutoOpenSettings {
            on_drop: Some(false),
            ..Default::default()
        },
    );

    let fs = FakeFs::new(cx.executor());
    fs.insert_tree(path!("/root"), json!({})).await;

    let temp_dir = tempfile::tempdir().unwrap();
    let external_path = temp_dir.path().join("manual.rs");
    std::fs::write(&external_path, "// dropped").unwrap();
    fs.insert_tree_from_real_fs(temp_dir.path(), temp_dir.path())
        .await;

    let project = Project::test(fs.clone(), [path!("/root").as_ref()], cx).await;
    let window = cx.add_window(|window, cx| MultiWorkspace::test_new(project.clone(), window, cx));
    let workspace = window
        .read_with(cx, |mw, _| mw.workspace().clone())
        .unwrap();
    let cx = &mut VisualTestContext::from_window(window.into(), cx);
    let panel = workspace.update_in(cx, ProjectPanel::new);
    cx.run_until_parked();

    let root_entry = find_project_entry(&panel, "root", cx).unwrap();
    panel.update_in(cx, |panel, window, cx| {
        panel.drop_external_files(std::slice::from_ref(&external_path), root_entry, window, cx);
    });
    cx.executor().run_until_parked();

    ensure_no_open_items_and_panes(&workspace, cx);
    assert!(
        find_project_entry(&panel, "root/manual.rs", cx).is_some(),
        "即使禁用自动打开,拖放的条目也应存在"
    );
}

#[gpui::test]
async fn test_create_duplicate_items(cx: &mut gpui::TestAppContext) {
    init_test_with_editor(cx);

    let fs = FakeFs::new(cx.executor());
    fs.insert_tree(
        "/src",
        json!({
            "test": {
                "first.rs": "// First Rust file",
                "second.rs": "// Second Rust file",
                "third.rs": "// Third Rust file",
            }
        }),
    )
    .await;

    let project = Project::test(fs.clone(), ["/src".as_ref()], cx).await;
    let window = cx.add_window(|window, cx| MultiWorkspace::test_new(project.clone(), window, cx));
    let workspace = window
        .read_with(cx, |mw, _| mw.workspace().clone())
        .unwrap();
    let cx = &mut VisualTestContext::from_window(window.into(), cx);
    let panel = workspace.update_in(cx, |workspace, window, cx| {
        let panel = ProjectPanel::new(workspace, window, cx);
        workspace.add_panel(panel.clone(), window, cx);
        panel
    });
    cx.run_until_parked();

    select_path(&panel, "src", cx);
    panel.update_in(cx, |panel, window, cx| panel.confirm(&Confirm, window, cx));
    cx.executor().run_until_parked();
    assert_eq!(
        visible_entries_as_strings(&panel, 0..10, cx),
        &[
            //
            "v src  <== selected",
            "    > test"
        ]
    );
    panel.update_in(cx, |panel, window, cx| {
        panel.new_directory(&NewDirectory, window, cx)
    });
    cx.run_until_parked();
    panel.update_in(cx, |panel, window, cx| {
        assert!(panel.filename_editor.read(cx).is_focused(window));
    });
    cx.executor().run_until_parked();
    assert_eq!(
        visible_entries_as_strings(&panel, 0..10, cx),
        &[
            //
            "v src",
            "    > [EDITOR: '']  <== selected",
            "    > test"
        ]
    );
    panel.update_in(cx, |panel, window, cx| {
        panel
            .filename_editor
            .update(cx, |editor, cx| editor.set_text("test", window, cx));
        assert!(
            panel.confirm_edit(true, window, cx).is_none(),
            "不应允许在冲突的新目录名称上确认"
        );
    });
    cx.executor().run_until_parked();
    panel.update_in(cx, |panel, window, cx| {
        assert!(
            panel.state.edit_state.is_some(),
            "冲突的新目录名称后,编辑状态不应为 None"
        );
        panel.cancel(&menu::Cancel, window, cx);
    });
    cx.run_until_parked();
    assert_eq!(
        visible_entries_as_strings(&panel, 0..10, cx),
        &[
            //
            "v src  <== selected",
            "    > test"
        ],
        "文件夹创建确认失败后,文件列表应保持不变"
    );

    select_path(&panel, "src/test", cx);
    panel.update_in(cx, |panel, window, cx| panel.confirm(&Confirm, window, cx));
    cx.executor().run_until_parked();
    assert_eq!(
        visible_entries_as_strings(&panel, 0..10, cx),
        &[
            //
            "v src",
            "    > test  <== selected"
        ]
    );
    panel.update_in(cx, |panel, window, cx| panel.new_file(&NewFile, window, cx));
    cx.run_until_parked();
    panel.update_in(cx, |panel, window, cx| {
        assert!(panel.filename_editor.read(cx).is_focused(window));
    });
    assert_eq!(
        visible_entries_as_strings(&panel, 0..10, cx),
        &[
            "v src",
            "    v test",
            "          [EDITOR: '']  <== selected",
            "          first.rs",
            "          second.rs",
            "          third.rs"
        ]
    );
    panel.update_in(cx, |panel, window, cx| {
        panel
            .filename_editor
            .update(cx, |editor, cx| editor.set_text("first.rs", window, cx));
        assert!(
            panel.confirm_edit(true, window, cx).is_none(),
            "不应允许在冲突的新文件名上确认"
        );
    });
    cx.executor().run_until_parked();
    panel.update_in(cx, |panel, window, cx| {
        assert!(
            panel.state.edit_state.is_some(),
            "冲突的新文件名后,编辑状态不应为 None"
        );
        panel.cancel(&menu::Cancel, window, cx);
    });
    cx.run_until_parked();
    assert_eq!(
        visible_entries_as_strings(&panel, 0..10, cx),
        &[
            "v src",
            "    v test  <== selected",
            "          first.rs",
            "          second.rs",
            "          third.rs"
        ],
        "文件创建确认失败后,文件列表应保持不变"
    );

    select_path(&panel, "src/test/first.rs", cx);
    panel.update_in(cx, |panel, window, cx| panel.confirm(&Confirm, window, cx));
    cx.executor().run_until_parked();
    assert_eq!(
        visible_entries_as_strings(&panel, 0..10, cx),
        &[
            "v src",
            "    v test",
            "          first.rs  <== selected",
            "          second.rs",
            "          third.rs"
        ],
    );
    panel.update_in(cx, |panel, window, cx| panel.rename(&Rename, window, cx));
    cx.executor().run_until_parked();
    panel.update_in(cx, |panel, window, cx| {
        assert!(panel.filename_editor.read(cx).is_focused(window));
    });
    assert_eq!(
        visible_entries_as_strings(&panel, 0..10, cx),
        &[
            "v src",
            "    v test",
            "          [EDITOR: 'first.rs']  <== selected",
            "          second.rs",
            "          third.rs"
        ]
    );
    panel.update_in(cx, |panel, window, cx| {
        panel
            .filename_editor
            .update(cx, |editor, cx| editor.set_text("second.rs", window, cx));
        assert!(
            panel.confirm_edit(true, window, cx).is_none(),
            "不应允许在冲突的文件重命名上确认"
        )
    });
    cx.executor().run_until_parked();
    panel.update_in(cx, |panel, window, cx| {
        assert!(
            panel.state.edit_state.is_some(),
            "冲突的文件重命名后,编辑状态不应为 None"
        );
        panel.cancel(&menu::Cancel, window, cx);
    });
    cx.executor().run_until_parked();
    assert_eq!(
        visible_entries_as_strings(&panel, 0..10, cx),
        &[
            "v src",
            "    v test",
            "          first.rs  <== selected",
            "          second.rs",
            "          third.rs"
        ],
        "重命名确认失败后,文件列表应保持不变"
    );
}

// NOTE: This test is skipped on Windows, because on Windows,
// when it triggers the lsp store it converts `/src/test/first copy.txt` into an uri
// but it fails with message `"/src\\test\\first copy.txt" is not parseable as an URI`
#[gpui::test]
#[cfg_attr(target_os = "windows", ignore)]
async fn test_create_duplicate_items_and_check_history(cx: &mut gpui::TestAppContext) {
    init_test_with_editor(cx);

    let fs = FakeFs::new(cx.executor());
    fs.insert_tree(
        "/src",
        json!({
            "test": {
                "first.txt": "// First Txt file",
                "second.txt": "// Second Txt file",
                "third.txt": "// Third Txt file",
            }
        }),
    )
    .await;

    let project = Project::test(fs.clone(), ["/src".as_ref()], cx).await;
    let window = cx.add_window(|window, cx| MultiWorkspace::test_new(project.clone(), window, cx));
    let workspace = window
        .read_with(cx, |mw, _| mw.workspace().clone())
        .unwrap();
    let cx = &mut VisualTestContext::from_window(window.into(), cx);
    let panel = workspace.update_in(cx, |workspace, window, cx| {
        let panel = ProjectPanel::new(workspace, window, cx);
        workspace.add_panel(panel.clone(), window, cx);
        panel
    });
    cx.run_until_parked();

    select_path(&panel, "src", cx);
    panel.update_in(cx, |panel, window, cx| panel.confirm(&Confirm, window, cx));
    cx.executor().run_until_parked();
    assert_eq!(
        visible_entries_as_strings(&panel, 0..10, cx),
        &[
            //
            "v src  <== selected",
            "    > test"
        ]
    );
    panel.update_in(cx, |panel, window, cx| {
        panel.new_directory(&NewDirectory, window, cx)
    });
    cx.run_until_parked();
    panel.update_in(cx, |panel, window, cx| {
        assert!(panel.filename_editor.read(cx).is_focused(window));
    });
    cx.executor().run_until_parked();
    assert_eq!(
        visible_entries_as_strings(&panel, 0..10, cx),
        &[
            //
            "v src",
            "    > [EDITOR: '']  <== selected",
            "    > test"
        ]
    );
    panel.update_in(cx, |panel, window, cx| {
        panel
            .filename_editor
            .update(cx, |editor, cx| editor.set_text("test", window, cx));
        assert!(
            panel.confirm_edit(true, window, cx).is_none(),
            "不应允许在冲突的新目录名称上确认"
        );
    });
    cx.executor().run_until_parked();
    panel.update_in(cx, |panel, window, cx| {
        assert!(
            panel.state.edit_state.is_some(),
            "冲突的新目录名称后,编辑状态不应为 None"
        );
        panel.cancel(&menu::Cancel, window, cx);
    });
    cx.run_until_parked();
    assert_eq!(
        visible_entries_as_strings(&panel, 0..10, cx),
        &[
            //
            "v src  <== selected",
            "    > test"
        ],
        "文件夹创建确认失败后,文件列表应保持不变"
    );

    select_path(&panel, "src/test", cx);
    panel.update_in(cx, |panel, window, cx| panel.confirm(&Confirm, window, cx));
    cx.executor().run_until_parked();
    assert_eq!(
        visible_entries_as_strings(&panel, 0..10, cx),
        &[
            //
            "v src",
            "    > test  <== selected"
        ]
    );
    panel.update_in(cx, |panel, window, cx| panel.new_file(&NewFile, window, cx));
    cx.run_until_parked();
    panel.update_in(cx, |panel, window, cx| {
        assert!(panel.filename_editor.read(cx).is_focused(window));
    });
    assert_eq!(
        visible_entries_as_strings(&panel, 0..10, cx),
        &[
            "v src",
            "    v test",
            "          [EDITOR: '']  <== selected",
            "          first.txt",
            "          second.txt",
            "          third.txt"
        ]
    );
    panel.update_in(cx, |panel, window, cx| {
        panel
            .filename_editor
            .update(cx, |editor, cx| editor.set_text("first.txt", window, cx));
        assert!(
            panel.confirm_edit(true, window, cx).is_none(),
            "不应允许在冲突的新文件名上确认"
        );
    });
    cx.executor().run_until_parked();
    panel.update_in(cx, |panel, window, cx| {
        assert!(
            panel.state.edit_state.is_some(),
            "冲突的新文件名后,编辑状态不应为 None"
        );
        panel.cancel(&menu::Cancel, window, cx);
    });
    cx.run_until_parked();
    assert_eq!(
        visible_entries_as_strings(&panel, 0..10, cx),
        &[
            "v src",
            "    v test  <== selected",
            "          first.txt",
            "          second.txt",
            "          third.txt"
        ],
        "文件创建确认失败后,文件列表应保持不变"
    );

    select_path(&panel, "src/test/first.txt", cx);
    panel.update_in(cx, |panel, window, cx| panel.confirm(&Confirm, window, cx));
    cx.executor().run_until_parked();
    assert_eq!(
        visible_entries_as_strings(&panel, 0..10, cx),
        &[
            "v src",
            "    v test",
            "          first.txt  <== selected",
            "          second.txt",
            "          third.txt"
        ],
    );
    panel.update_in(cx, |panel, window, cx| panel.rename(&Rename, window, cx));
    cx.executor().run_until_parked();
    panel.update_in(cx, |panel, window, cx| {
        assert!(panel.filename_editor.read(cx).is_focused(window));
    });
    assert_eq!(
        visible_entries_as_strings(&panel, 0..10, cx),
        &[
            "v src",
            "    v test",
            "          [EDITOR: 'first.txt']  <== selected",
            "          second.txt",
            "          third.txt"
        ]
    );
    panel.update_in(cx, |panel, window, cx| {
        panel
            .filename_editor
            .update(cx, |editor, cx| editor.set_text("second.txt", window, cx));
        assert!(
            panel.confirm_edit(true, window, cx).is_none(),
            "不应允许在冲突的文件重命名上确认"
        )
    });
    cx.executor().run_until_parked();
    panel.update_in(cx, |panel, window, cx| {
        assert!(
            panel.state.edit_state.is_some(),
            "冲突的文件重命名后,编辑状态不应为 None"
        );
        panel.cancel(&menu::Cancel, window, cx);
    });
    cx.executor().run_until_parked();
    assert_eq!(
        visible_entries_as_strings(&panel, 0..10, cx),
        &[
            "v src",
            "    v test",
            "          first.txt  <== selected",
            "          second.txt",
            "          third.txt"
        ],
        "重命名确认失败后,文件列表应保持不变"
    );
    panel.update_in(cx, |panel, window, cx| panel.open(&Open, window, cx));
    cx.executor().run_until_parked();
    // Try to duplicate and check history
    panel.update_in(cx, |panel, window, cx| {
        panel.duplicate(&Duplicate, window, cx)
    });
    cx.executor().run_until_parked();

    assert_eq!(
        visible_entries_as_strings(&panel, 0..10, cx),
        &[
            "v src",
            "    v test",
            "          first.txt",
            "          [EDITOR: 'first copy.txt']  <== selected  <== marked",
            "          second.txt",
            "          third.txt"
        ],
    );

    let confirm = panel.update_in(cx, |panel, window, cx| {
        panel
            .filename_editor
            .update(cx, |editor, cx| editor.set_text("fourth.txt", window, cx));
        panel.confirm_edit(true, window, cx).unwrap()
    });
    confirm.await.unwrap();
    cx.executor().run_until_parked();

    assert_eq!(
        visible_entries_as_strings(&panel, 0..10, cx),
        &[
            "v src",
            "    v test",
            "          first.txt",
            "          fourth.txt  <== selected",
            "          second.txt",
            "          third.txt"
        ],
        "重命名确认后,文件列表应有所不同"
    );

    panel.update_in(cx, |panel, window, cx| {
        panel.update_visible_entries(None, false, false, window, cx);
    });
    cx.executor().run_until_parked();

    select_path(&panel, "src/test/first.txt", cx);
    panel.update_in(cx, |panel, window, cx| panel.open(&Open, window, cx));
    cx.executor().run_until_parked();

    workspace.read_with(cx, |this, cx| {
        assert!(
            this.recent_navigation_history_iter(cx)
                .any(|(project_path, abs_path)| {
                    project_path.path == Arc::from(rel_path("test/fourth.txt"))
                        && abs_path == Some(PathBuf::from(path!("/src/test/fourth.txt")))
                })
        );
    });
}

// NOTE: This test is skipped on Windows, because on Windows,
// when it triggers the lsp store it converts `/src/test/first.txt` into an uri
// but it fails with message `"/src\\test\\first.txt" is not parseable as an URI`
#[gpui::test]
#[cfg_attr(target_os = "windows", ignore)]
async fn test_rename_item_and_check_history(cx: &mut gpui::TestAppContext) {
    init_test_with_editor(cx);

    let fs = FakeFs::new(cx.executor());
    fs.insert_tree(
        "/src",
        json!({
            "test": {
                "first.txt": "// First Txt file",
                "second.txt": "// Second Txt file",
                "third.txt": "// Third Txt file",
            }
        }),
    )
    .await;

    let project = Project::test(fs.clone(), ["/src".as_ref()], cx).await;
    let window = cx.add_window(|window, cx| MultiWorkspace::test_new(project.clone(), window, cx));
    let workspace = window
        .read_with(cx, |mw, _| mw.workspace().clone())
        .unwrap();
    let cx = &mut VisualTestContext::from_window(window.into(), cx);
    let panel = workspace.update_in(cx, |workspace, window, cx| {
        let panel = ProjectPanel::new(workspace, window, cx);
        workspace.add_panel(panel.clone(), window, cx);
        panel
    });
    cx.run_until_parked();

    select_path(&panel, "src", cx);
    panel.update_in(cx, |panel, window, cx| panel.confirm(&Confirm, window, cx));
    cx.executor().run_until_parked();
    assert_eq!(
        visible_entries_as_strings(&panel, 0..10, cx),
        &[
            //
            "v src  <== selected",
            "    > test"
        ]
    );

    select_path(&panel, "src/test", cx);
    panel.update_in(cx, |panel, window, cx| panel.confirm(&Confirm, window, cx));
    cx.executor().run_until_parked();
    assert_eq!(
        visible_entries_as_strings(&panel, 0..10, cx),
        &[
            //
            "v src",
            "    > test  <== selected"
        ]
    );
    panel.update_in(cx, |panel, window, cx| panel.new_file(&NewFile, window, cx));
    cx.run_until_parked();
    panel.update_in(cx, |panel, window, cx| {
        assert!(panel.filename_editor.read(cx).is_focused(window));
    });

    select_path(&panel, "src/test/first.txt", cx);
    panel.update_in(cx, |panel, window, cx| panel.open(&Open, window, cx));
    cx.executor().run_until_parked();

    panel.update_in(cx, |panel, window, cx| panel.rename(&Rename, window, cx));
    cx.executor().run_until_parked();

    assert_eq!(
        visible_entries_as_strings(&panel, 0..10, cx),
        &[
            "v src",
            "    v test",
            "          [EDITOR: 'first.txt']  <== selected  <== marked",
            "          second.txt",
            "          third.txt"
        ],
    );

    let confirm = panel.update_in(cx, |panel, window, cx| {
        panel
            .filename_editor
            .update(cx, |editor, cx| editor.set_text("fourth.txt", window, cx));
        panel.confirm_edit(true, window, cx).unwrap()
    });
    confirm.await.unwrap();
    cx.executor().run_until_parked();

    assert_eq!(
        visible_entries_as_strings(&panel, 0..10, cx),
        &[
            "v src",
            "    v test",
            "          fourth.txt  <== selected",
            "          second.txt",
            "          third.txt"
        ],
        "重命名确认后,文件列表应有所不同"
    );

    panel.update_in(cx, |panel, window, cx| {
        panel.update_visible_entries(None, false, false, window, cx);
    });
    cx.executor().run_until_parked();

    select_path(&panel, "src/test/second.txt", cx);
    panel.update_in(cx, |panel, window, cx| panel.open(&Open, window, cx));
    cx.executor().run_until_parked();

    workspace.read_with(cx, |this, cx| {
        assert!(
            this.recent_navigation_history_iter(cx)
                .any(|(project_path, abs_path)| {
                    project_path.path == Arc::from(rel_path("test/fourth.txt"))
                        && abs_path == Some(PathBuf::from(path!("/src/test/fourth.txt")))
                })
        );
    });
}

#[gpui::test]
async fn test_select_git_entry(cx: &mut gpui::TestAppContext) {
    init_test_with_editor(cx);

    let fs = FakeFs::new(cx.executor());
    fs.insert_tree(
        path!("/root"),
        json!({
            "tree1": {
                ".git": {},
                "dir1": {
                    "modified1.txt": "1",
                    "unmodified1.txt": "1",
                    "modified2.txt": "1",
                },
                "dir2": {
                    "modified3.txt": "1",
                    "unmodified2.txt": "1",
                },
                "modified4.txt": "1",
                "unmodified3.txt": "1",
            },
            "tree2": {
                ".git": {},
                "dir3": {
                    "modified5.txt": "1",
                    "unmodified4.txt": "1",
                },
                "modified6.txt": "1",
                "unmodified5.txt": "1",
            }
        }),
    )
    .await;

    // Mark files as git modified
    fs.set_head_and_index_for_repo(
        path!("/root/tree1/.git").as_ref(),
        &[
            ("dir1/modified1.txt", "modified".into()),
            ("dir1/modified2.txt", "modified".into()),
            ("modified4.txt", "modified".into()),
            ("dir2/modified3.txt", "modified".into()),
        ],
    );
    fs.set_head_and_index_for_repo(
        path!("/root/tree2/.git").as_ref(),
        &[
            ("dir3/modified5.txt", "modified".into()),
            ("modified6.txt", "modified".into()),
        ],
    );

    let project = Project::test(
        fs.clone(),
        [path!("/root/tree1").as_ref(), path!("/root/tree2").as_ref()],
        cx,
    )
    .await;

    let (scan1_complete, scan2_complete) = project.update(cx, |project, cx| {
        let mut worktrees = project.worktrees(cx);
        let worktree1 = worktrees.next().unwrap();
        let worktree2 = worktrees.next().unwrap();
        (
            worktree1.read(cx).as_local().unwrap().scan_complete(),
            worktree2.read(cx).as_local().unwrap().scan_complete(),
        )
    });
    scan1_complete.await;
    scan2_complete.await;
    cx.run_until_parked();

    let window = cx.add_window(|window, cx| MultiWorkspace::test_new(project.clone(), window, cx));
    let workspace = window
        .read_with(cx, |mw, _| mw.workspace().clone())
        .unwrap();
    let cx = &mut VisualTestContext::from_window(window.into(), cx);
    let panel = workspace.update_in(cx, ProjectPanel::new);
    cx.run_until_parked();

    // Check initial state
    assert_eq!(
        visible_entries_as_strings(&panel, 0..15, cx),
        &[
            "v tree1",
            "    > .git",
            "    > dir1",
            "    > dir2",
            "      modified4.txt",
            "      unmodified3.txt",
            "v tree2",
            "    > .git",
            "    > dir3",
            "      modified6.txt",
            "      unmodified5.txt"
        ],
    );

    // Test selecting next modified entry
    panel.update_in(cx, |panel, window, cx| {
        panel.select_next_git_entry(&SelectNextGitEntry, window, cx);
    });
    cx.run_until_parked();

    assert_eq!(
        visible_entries_as_strings(&panel, 0..6, cx),
        &[
            "v tree1",
            "    > .git",
            "    v dir1",
            "          modified1.txt  <== selected",
            "          modified2.txt",
            "          unmodified1.txt",
        ],
    );

    panel.update_in(cx, |panel, window, cx| {
        panel.select_next_git_entry(&SelectNextGitEntry, window, cx);
    });
    cx.run_until_parked();

    assert_eq!(
        visible_entries_as_strings(&panel, 0..6, cx),
        &[
            "v tree1",
            "    > .git",
            "    v dir1",
            "          modified1.txt",
            "          modified2.txt  <== selected",
            "          unmodified1.txt",
        ],
    );

    panel.update_in(cx, |panel, window, cx| {
        panel.select_next_git_entry(&SelectNextGitEntry, window, cx);
    });
    cx.run_until_parked();

    assert_eq!(
        visible_entries_as_strings(&panel, 6..9, cx),
        &[
            "    v dir2",
            "          modified3.txt  <== selected",
            "          unmodified2.txt",
        ],
    );

    panel.update_in(cx, |panel, window, cx| {
        panel.select_next_git_entry(&SelectNextGitEntry, window, cx);
    });
    cx.run_until_parked();

    assert_eq!(
        visible_entries_as_strings(&panel, 9..11, cx),
        &["      modified4.txt  <== selected", "      unmodified3.txt",],
    );

    panel.update_in(cx, |panel, window, cx| {
        panel.select_next_git_entry(&SelectNextGitEntry, window, cx);
    });
    cx.run_until_parked();

    assert_eq!(
        visible_entries_as_strings(&panel, 13..16, cx),
        &[
            "    v dir3",
            "          modified5.txt  <== selected",
            "          unmodified4.txt",
        ],
    );

    panel.update_in(cx, |panel, window, cx| {
        panel.select_next_git_entry(&SelectNextGitEntry, window, cx);
    });
    cx.run_until_parked();

    assert_eq!(
        visible_entries_as_strings(&panel, 16..18, cx),
        &["      modified6.txt  <== selected", "      unmodified5.txt",],
    );

    // Wraps around to first modified file
    panel.update_in(cx, |panel, window, cx| {
        panel.select_next_git_entry(&SelectNextGitEntry, window, cx);
    });
    cx.run_until_parked();

    assert_eq!(
        visible_entries_as_strings(&panel, 0..18, cx),
        &[
            "v tree1",
            "    > .git",
            "    v dir1",
            "          modified1.txt  <== selected",
            "          modified2.txt",
            "          unmodified1.txt",
            "    v dir2",
            "          modified3.txt",
            "          unmodified2.txt",
            "      modified4.txt",
            "      unmodified3.txt",
            "v tree2",
            "    > .git",
            "    v dir3",
            "          modified5.txt",
            "          unmodified4.txt",
            "      modified6.txt",
            "      unmodified5.txt",
        ],
    );

    // Wraps around again to last modified file
    panel.update_in(cx, |panel, window, cx| {
        panel.select_prev_git_entry(&SelectPrevGitEntry, window, cx);
    });
    cx.run_until_parked();

    assert_eq!(
        visible_entries_as_strings(&panel, 16..18, cx),
        &["      modified6.txt  <== selected", "      unmodified5.txt",],
    );

    panel.update_in(cx, |panel, window, cx| {
        panel.select_prev_git_entry(&SelectPrevGitEntry, window, cx);
    });
    cx.run_until_parked();

    assert_eq!(
        visible_entries_as_strings(&panel, 13..16, cx),
        &[
            "    v dir3",
            "          modified5.txt  <== selected",
            "          unmodified4.txt",
        ],
    );

    panel.update_in(cx, |panel, window, cx| {
        panel.select_prev_git_entry(&SelectPrevGitEntry, window, cx);
    });
    cx.run_until_parked();

    assert_eq!(
        visible_entries_as_strings(&panel, 9..11, cx),
        &["      modified4.txt  <== selected", "      unmodified3.txt",],
    );

    panel.update_in(cx, |panel, window, cx| {
        panel.select_prev_git_entry(&SelectPrevGitEntry, window, cx);
    });
    cx.run_until_parked();

    assert_eq!(
        visible_entries_as_strings(&panel, 6..9, cx),
        &[
            "    v dir2",
            "          modified3.txt  <== selected",
            "          unmodified2.txt",
        ],
    );

    panel.update_in(cx, |panel, window, cx| {
        panel.select_prev_git_entry(&SelectPrevGitEntry, window, cx);
    });
    cx.run_until_parked();

    assert_eq!(
        visible_entries_as_strings(&panel, 0..6, cx),
        &[
            "v tree1",
            "    > .git",
            "    v dir1",
            "          modified1.txt",
            "          modified2.txt  <== selected",
            "          unmodified1.txt",
        ],
    );

    panel.update_in(cx, |panel, window, cx| {
        panel.select_prev_git_entry(&SelectPrevGitEntry, window, cx);
    });
    cx.run_until_parked();

    assert_eq!(
        visible_entries_as_strings(&panel, 0..6, cx),
        &[
            "v tree1",
            "    > .git",
            "    v dir1",
            "          modified1.txt  <== selected",
            "          modified2.txt",
            "          unmodified1.txt",
        ],
    );
}

#[gpui::test]
async fn test_select_directory(cx: &mut gpui::TestAppContext) {
    init_test_with_editor(cx);

    let fs = FakeFs::new(cx.executor());
    fs.insert_tree(
        "/project_root",
        json!({
            "dir_1": {
                "nested_dir": {
                    "file_a.py": "# File contents",
                }
            },
            "file_1.py": "# File contents",
            "dir_2": {

            },
            "dir_3": {

            },
            "file_2.py": "# File contents",
            "dir_4": {

            },
        }),
    )
    .await;

    let project = Project::test(fs.clone(), ["/project_root".as_ref()], cx).await;
    let window = cx.add_window(|window, cx| MultiWorkspace::test_new(project.clone(), window, cx));
    let workspace = window
        .read_with(cx, |mw, _| mw.workspace().clone())
        .unwrap();
    let cx = &mut VisualTestContext::from_window(window.into(), cx);
    let panel = workspace.update_in(cx, ProjectPanel::new);
    cx.run_until_parked();

    panel.update_in(cx, |panel, window, cx| panel.open(&Open, window, cx));
    cx.executor().run_until_parked();
    select_path(&panel, "project_root/dir_1", cx);
    cx.executor().run_until_parked();
    assert_eq!(
        visible_entries_as_strings(&panel, 0..10, cx),
        &[
            "v project_root",
            "    > dir_1  <== selected",
            "    > dir_2",
            "    > dir_3",
            "    > dir_4",
            "      file_1.py",
            "      file_2.py",
        ]
    );
    panel.update_in(cx, |panel, window, cx| {
        panel.select_prev_directory(&SelectPrevDirectory, window, cx)
    });

    assert_eq!(
        visible_entries_as_strings(&panel, 0..10, cx),
        &[
            "v project_root  <== selected",
            "    > dir_1",
            "    > dir_2",
            "    > dir_3",
            "    > dir_4",
            "      file_1.py",
            "      file_2.py",
        ]
    );

    panel.update_in(cx, |panel, window, cx| {
        panel.select_prev_directory(&SelectPrevDirectory, window, cx)
    });

    assert_eq!(
        visible_entries_as_strings(&panel, 0..10, cx),
        &[
            "v project_root",
            "    > dir_1",
            "    > dir_2",
            "    > dir_3",
            "    > dir_4  <== selected",
            "      file_1.py",
            "      file_2.py",
        ]
    );

    panel.update_in(cx, |panel, window, cx| {
        panel.select_next_directory(&SelectNextDirectory, window, cx)
    });

    assert_eq!(
        visible_entries_as_strings(&panel, 0..10, cx),
        &[
            "v project_root  <== selected",
            "    > dir_1",
            "    > dir_2",
            "    > dir_3",
            "    > dir_4",
            "      file_1.py",
            "      file_2.py",
        ]
    );
}

#[gpui::test]
async fn test_select_first_last(cx: &mut gpui::TestAppContext) {
    init_test_with_editor(cx);

    let fs = FakeFs::new(cx.executor());
    fs.insert_tree(
        "/project_root",
        json!({
            "dir_1": {
                "nested_dir": {
                    "file_a.py": "# File contents",
                }
            },
            "file_1.py": "# File contents",
            "file_2.py": "# File contents",
            "zdir_2": {
                "nested_dir2": {
                    "file_b.py": "# File contents",
                }
            },
        }),
    )
    .await;

    let project = Project::test(fs.clone(), ["/project_root".as_ref()], cx).await;
    let window = cx.add_window(|window, cx| MultiWorkspace::test_new(project.clone(), window, cx));
    let workspace = window
        .read_with(cx, |mw, _| mw.workspace().clone())
        .unwrap();
    let cx = &mut VisualTestContext::from_window(window.into(), cx);
    let panel = workspace.update_in(cx, ProjectPanel::new);
    cx.run_until_parked();

    assert_eq!(
        visible_entries_as_strings(&panel, 0..10, cx),
        &[
            "v project_root",
            "    > dir_1",
            "    > zdir_2",
            "      file_1.py",
            "      file_2.py",
        ]
    );
    panel.update_in(cx, |panel, window, cx| {
        panel.select_first(&SelectFirst, window, cx)
    });

    assert_eq!(
        visible_entries_as_strings(&panel, 0..10, cx),
        &[
            "v project_root  <== selected",
            "    > dir_1",
            "    > zdir_2",
            "      file_1.py",
            "      file_2.py",
        ]
    );

    panel.update_in(cx, |panel, window, cx| {
        panel.select_last(&SelectLast, window, cx)
    });

    assert_eq!(
        visible_entries_as_strings(&panel, 0..10, cx),
        &[
            "v project_root",
            "    > dir_1",
            "    > zdir_2",
            "      file_1.py",
            "      file_2.py  <== selected",
        ]
    );

    cx.update(|_, cx| {
        let settings = *ProjectPanelSettings::get_global(cx);
        ProjectPanelSettings::override_global(
            ProjectPanelSettings {
                hide_root: true,
                ..settings
            },
            cx,
        );
    });

    let panel = workspace.update_in(cx, ProjectPanel::new);
    cx.run_until_parked();

    #[rustfmt::skip]
    assert_eq!(
        visible_entries_as_strings(&panel, 0..10, cx),
        &[
            "> dir_1",
            "> zdir_2",
            "  file_1.py",
            "  file_2.py",
        ],
        "当 hide_root=true 时,根目录应被隐藏"
    );

    panel.update_in(cx, |panel, window, cx| {
        panel.select_first(&SelectFirst, window, cx)
    });

    assert_eq!(
        visible_entries_as_strings(&panel, 0..10, cx),
        &[
            "> dir_1  <== selected",
            "> zdir_2",
            "  file_1.py",
            "  file_2.py",
        ],
        "当 hide_root=true 时,第一个条目应为 dir_1,而不是隐藏的根目录"
    );
}

#[gpui::test]
async fn test_dir_toggle_collapse(cx: &mut gpui::TestAppContext) {
    init_test_with_editor(cx);

    let fs = FakeFs::new(cx.executor());
    fs.insert_tree(
        "/project_root",
        json!({
            "dir_1": {
                "nested_dir": {
                    "file_a.py": "# File contents",
                }
            },
            "file_1.py": "# File contents",
        }),
    )
    .await;

    let project = Project::test(fs.clone(), ["/project_root".as_ref()], cx).await;
    let window = cx.add_window(|window, cx| MultiWorkspace::test_new(project.clone(), window, cx));
    let workspace = window
        .read_with(cx, |mw, _| mw.workspace().clone())
        .unwrap();
    let cx = &mut VisualTestContext::from_window(window.into(), cx);
    let panel = workspace.update_in(cx, ProjectPanel::new);
    cx.run_until_parked();

    panel.update_in(cx, |panel, window, cx| panel.open(&Open, window, cx));
    cx.executor().run_until_parked();
    select_path(&panel, "project_root/dir_1", cx);
    panel.update_in(cx, |panel, window, cx| panel.open(&Open, window, cx));
    select_path(&panel, "project_root/dir_1/nested_dir", cx);
    panel.update_in(cx, |panel, window, cx| panel.open(&Open, window, cx));
    panel.update_in(cx, |panel, window, cx| panel.open(&Open, window, cx));
    cx.executor().run_until_parked();
    assert_eq!(
        visible_entries_as_strings(&panel, 0..10, cx),
        &[
            "v project_root",
            "    v dir_1",
            "        > nested_dir  <== selected",
            "      file_1.py",
        ]
    );
}

#[gpui::test]
async fn test_collapse_all_entries(cx: &mut gpui::TestAppContext) {
    init_test_with_editor(cx);

    let fs = FakeFs::new(cx.executor());
    fs.insert_tree(
        "/project_root",
        json!({
            "dir_1": {
                "nested_dir": {
                    "file_a.py": "# File contents",
                    "file_b.py": "# File contents",
                    "file_c.py": "# File contents",
                },
                "file_1.py": "# File contents",
                "file_2.py": "# File contents",
                "file_3.py": "# File contents",
            },
            "dir_2": {
                "file_1.py": "# File contents",
                "file_2.py": "# File contents",
                "file_3.py": "# File contents",
            }
        }),
    )
    .await;

    let project = Project::test(fs.clone(), ["/project_root".as_ref()], cx).await;
    let window = cx.add_window(|window, cx| MultiWorkspace::test_new(project.clone(), window, cx));
    let workspace = window
        .read_with(cx, |mw, _| mw.workspace().clone())
        .unwrap();
    let cx = &mut VisualTestContext::from_window(window.into(), cx);
    let panel = workspace.update_in(cx, ProjectPanel::new);
    cx.run_until_parked();

    panel.update_in(cx, |panel, window, cx| {
        panel.collapse_all_entries(&CollapseAllEntries, window, cx)
    });
    cx.executor().run_until_parked();
    assert_eq!(
        visible_entries_as_strings(&panel, 0..10, cx),
        &["v project_root", "    > dir_1", "    > dir_2",]
    );

    // Open dir_1 and make sure nested_dir was collapsed when running collapse_all_entries
    toggle_expand_dir(&panel, "project_root/dir_1", cx);
    cx.executor().run_until_parked();
    assert_eq!(
        visible_entries_as_strings(&panel, 0..10, cx),
        &[
            "v project_root",
            "    v dir_1  <== selected",
            "        > nested_dir",
            "          file_1.py",
            "          file_2.py",
            "          file_3.py",
            "    > dir_2",
        ]
    );
}

#[gpui::test]
async fn test_collapse_all_entries_multiple_worktrees(cx: &mut gpui::TestAppContext) {
    init_test_with_editor(cx);

    let fs = FakeFs::new(cx.executor());
    let worktree_content = json!({
        "dir_1": {
            "file_1.py": "# File contents",
        },
        "dir_2": {
            "file_1.py": "# File contents",
        }
    });

    fs.insert_tree("/project_root_1", worktree_content.clone())
        .await;
    fs.insert_tree("/project_root_2", worktree_content).await;

    let project = Project::test(
        fs.clone(),
        ["/project_root_1".as_ref(), "/project_root_2".as_ref()],
        cx,
    )
    .await;
    let window = cx.add_window(|window, cx| MultiWorkspace::test_new(project.clone(), window, cx));
    let workspace = window
        .read_with(cx, |mw, _| mw.workspace().clone())
        .unwrap();
    let cx = &mut VisualTestContext::from_window(window.into(), cx);
    let panel = workspace.update_in(cx, ProjectPanel::new);
    cx.run_until_parked();

    panel.update_in(cx, |panel, window, cx| {
        panel.collapse_all_entries(&CollapseAllEntries, window, cx)
    });
    cx.executor().run_until_parked();
    assert_eq!(
        visible_entries_as_strings(&panel, 0..10, cx),
        &["> project_root_1", "> project_root_2",]
    );
}

#[gpui::test]
async fn test_collapse_all_entries_with_collapsed_root(cx: &mut gpui::TestAppContext) {
    init_test_with_editor(cx);

    let fs = FakeFs::new(cx.executor());
    fs.insert_tree(
        "/project_root",
        json!({
            "dir_1": {
                "nested_dir": {
                    "file_a.py": "# File contents",
                    "file_b.py": "# File contents",
                    "file_c.py": "# File contents",
                },
                "file_1.py": "# File contents",
                "file_2.py": "# File contents",
                "file_3.py": "# File contents",
            },
            "dir_2": {
                "file_1.py": "# File contents",
                "file_2.py": "# File contents",
                "file_3.py": "# File contents",
            }
        }),
    )
    .await;

    let project = Project::test(fs.clone(), ["/project_root".as_ref()], cx).await;
    let window = cx.add_window(|window, cx| MultiWorkspace::test_new(project.clone(), window, cx));
    let workspace = window
        .read_with(cx, |mw, _| mw.workspace().clone())
        .unwrap();
    let cx = &mut VisualTestContext::from_window(window.into(), cx);
    let panel = workspace.update_in(cx, ProjectPanel::new);
    cx.run_until_parked();

    // Open project_root/dir_1 to ensure that a nested directory is expanded
    toggle_expand_dir(&panel, "project_root/dir_1", cx);
    cx.executor().run_until_parked();
    assert_eq!(
        visible_entries_as_strings(&panel, 0..10, cx),
        &[
            "v project_root",
            "    v dir_1  <== selected",
            "        > nested_dir",
            "          file_1.py",
            "          file_2.py",
            "          file_3.py",
            "    > dir_2",
        ]
    );

    // Close root directory
    toggle_expand_dir(&panel, "project_root", cx);
    cx.executor().run_until_parked();
    assert_eq!(
        visible_entries_as_strings(&panel, 0..10, cx),
        &["> project_root  <== selected"]
    );

    // Run collapse_all_entries and make sure root is not expanded
    panel.update_in(cx, |panel, window, cx| {
        panel.collapse_all_entries(&CollapseAllEntries, window, cx)
    });
    cx.executor().run_until_parked();
    assert_eq!(
        visible_entries_as_strings(&panel, 0..10, cx),
        &["> project_root  <== selected"]
    );
}

#[gpui::test]
async fn test_collapse_all_entries_with_invisible_worktree(cx: &mut gpui::TestAppContext) {
    init_test_with_editor(cx);

    let fs = FakeFs::new(cx.executor());
    fs.insert_tree(
        "/project_root",
        json!({
            "dir_1": {
                "nested_dir": {
                    "file_a.py": "# File contents",
                },
                "file_1.py": "# File contents",
            },
            "dir_2": {
                "file_1.py": "# File contents",
            }
        }),
    )
    .await;
    fs.insert_tree(
        "/external",
        json!({
            "external_file.py": "# External file",
        }),
    )
    .await;

    let project = Project::test(fs.clone(), ["/project_root".as_ref()], cx).await;
    let window = cx.add_window(|window, cx| MultiWorkspace::test_new(project.clone(), window, cx));
    let workspace = window
        .read_with(cx, |mw, _| mw.workspace().clone())
        .unwrap();
    let cx = &mut VisualTestContext::from_window(window.into(), cx);
    let panel = workspace.update_in(cx, ProjectPanel::new);
    cx.run_until_parked();

    let (_invisible_worktree, _) = project
        .update(cx, |project, cx| {
            project.find_or_create_worktree("/external/external_file.py", false, cx)
        })
        .await
        .unwrap();
    cx.run_until_parked();

    assert_eq!(
        visible_entries_as_strings(&panel, 0..10, cx),
        &["v project_root", "    > dir_1", "    > dir_2",],
        "不可见的工作树不应出现在项目面板中"
    );

    toggle_expand_dir(&panel, "project_root/dir_1", cx);
    cx.executor().run_until_parked();

    panel.update_in(cx, |panel, window, cx| {
        panel.collapse_all_entries(&CollapseAllEntries, window, cx)
    });
    cx.executor().run_until_parked();
    assert_eq!(
        visible_entries_as_strings(&panel, 0..10, cx),
        &["v project_root", "    > dir_1  <== selected", "    > dir_2",],
        "当只有一个可见工作树时,即使存在不可见工作树,根节点也应保持展开状态"
    );
}

#[gpui::test]
async fn test_new_file_move(cx: &mut gpui::TestAppContext) {
    init_test(cx);

    let fs = FakeFs::new(cx.executor());
    fs.as_fake().insert_tree(path!("/root"), json!({})).await;
    let project = Project::test(fs, [path!("/root").as_ref()], cx).await;
    let window = cx.add_window(|window, cx| MultiWorkspace::test_new(project.clone(), window, cx));
    let workspace = window
        .read_with(cx, |mw, _| mw.workspace().clone())
        .unwrap();
    let cx = &mut VisualTestContext::from_window(window.into(), cx);
    let panel = workspace.update_in(cx, ProjectPanel::new);
    cx.run_until_parked();

    // Make a new buffer with no backing file
    workspace.update_in(cx, |workspace, window, cx| {
        Editor::new_file(workspace, &Default::default(), window, cx)
    });

    cx.executor().run_until_parked();

    // "另存为" the buffer, creating a new backing file for it
    let save_task = workspace.update_in(cx, |workspace, window, cx| {
        workspace.save_active_item(workspace::SaveIntent::Save, window, cx)
    });

    cx.executor().run_until_parked();
    cx.simulate_new_path_selection(|_| Some(PathBuf::from(path!("/root/new"))));
    save_task.await.unwrap();

    // Rename the file
    select_path(&panel, "root/new", cx);
    assert_eq!(
        visible_entries_as_strings(&panel, 0..10, cx),
        &["v root", "      new  <== selected  <== marked"]
    );
    panel.update_in(cx, |panel, window, cx| panel.rename(&Rename, window, cx));
    panel.update_in(cx, |panel, window, cx| {
        panel
            .filename_editor
            .update(cx, |editor, cx| editor.set_text("newer", window, cx));
    });
    panel.update_in(cx, |panel, window, cx| panel.confirm(&Confirm, window, cx));

    cx.executor().run_until_parked();
    assert_eq!(
        visible_entries_as_strings(&panel, 0..10, cx),
        &["v root", "      newer  <== selected"]
    );

    workspace
        .update_in(cx, |workspace, window, cx| {
            workspace.save_active_item(workspace::SaveIntent::Save, window, cx)
        })
        .await
        .unwrap();

    cx.executor().run_until_parked();
    // assert that saving the file doesn't restore "new"
    assert_eq!(
        visible_entries_as_strings(&panel, 0..10, cx),
        &["v root", "      newer  <== selected"]
    );
}

// NOTE: This test is skipped on Windows, because on Windows, unlike on Unix,
// you can't rename a directory which some program has already open. This is a
// limitation of the Windows. Since Zed will have the root open, it will hold an open handle
// to it, and thus renaming it will fail on Windows.
// See: https://stackoverflow.com/questions/41365318/access-is-denied-when-renaming-folder
// See: https://learn.microsoft.com/en-us/windows-hardware/drivers/ddi/ntifs/ns-ntifs-_file_rename_information
#[gpui::test]
#[cfg_attr(target_os = "windows", ignore)]
async fn test_rename_root_of_worktree(cx: &mut gpui::TestAppContext) {
    init_test_with_editor(cx);

    let fs = FakeFs::new(cx.executor());
    fs.insert_tree(
        "/root1",
        json!({
            "dir1": {
                "file1.txt": "content 1",
            },
        }),
    )
    .await;

    let project = Project::test(fs.clone(), ["/root1".as_ref()], cx).await;
    let window = cx.add_window(|window, cx| MultiWorkspace::test_new(project.clone(), window, cx));
    let workspace = window
        .read_with(cx, |mw, _| mw.workspace().clone())
        .unwrap();
    let cx = &mut VisualTestContext::from_window(window.into(), cx);
    let panel = workspace.update_in(cx, ProjectPanel::new);
    cx.run_until_parked();

    toggle_expand_dir(&panel, "root1/dir1", cx);

    assert_eq!(
        visible_entries_as_strings(&panel, 0..20, cx),
        &["v root1", "    v dir1  <== selected", "          file1.txt",],
        "包含工作树的初始状态"
    );

    select_path(&panel, "root1", cx);
    assert_eq!(
        visible_entries_as_strings(&panel, 0..20, cx),
        &["v root1  <== selected", "    v dir1", "          file1.txt",],
    );

    // Rename root1 to new_root1
    panel.update_in(cx, |panel, window, cx| panel.rename(&Rename, window, cx));

    assert_eq!(
        visible_entries_as_strings(&panel, 0..20, cx),
        &[
            "v [EDITOR: 'root1']  <== selected",
            "    v dir1",
            "          file1.txt",
        ],
    );

    let confirm = panel.update_in(cx, |panel, window, cx| {
        panel
            .filename_editor
            .update(cx, |editor, cx| editor.set_text("new_root1", window, cx));
        panel.confirm_edit(true, window, cx).unwrap()
    });
    confirm.await.unwrap();
    cx.run_until_parked();
    assert_eq!(
        visible_entries_as_strings(&panel, 0..20, cx),
        &[
            "v new_root1  <== selected",
            "    v dir1",
            "          file1.txt",
        ],
        "应更新工作树名称"
    );

    // Ensure internal paths have been updated
    select_path(&panel, "new_root1/dir1/file1.txt", cx);
    assert_eq!(
        visible_entries_as_strings(&panel, 0..20, cx),
        &[
            "v new_root1",
            "    v dir1",
            "          file1.txt  <== selected",
        ],
        "重命名工作树中的文件应可被选中"
    );
}

#[gpui::test]
async fn test_rename_with_hide_root(cx: &mut gpui::TestAppContext) {
    init_test_with_editor(cx);

    let fs = FakeFs::new(cx.executor());
    fs.insert_tree(
        "/root1",
        json!({
            "dir1": { "file1.txt": "content" },
            "file2.txt": "content",
        }),
    )
    .await;
    fs.insert_tree("/root2", json!({ "file3.txt": "content" }))
        .await;

    // Test 1: Single worktree, hide_root=true - rename should be blocked
    {
        let project = Project::test(fs.clone(), ["/root1".as_ref()], cx).await;
        let window =
            cx.add_window(|window, cx| MultiWorkspace::test_new(project.clone(), window, cx));
        let workspace = window
            .read_with(cx, |mw, _| mw.workspace().clone())
            .unwrap();
        let cx = &mut VisualTestContext::from_window(window.into(), cx);

        cx.update(|_, cx| {
            let settings = *ProjectPanelSettings::get_global(cx);
            ProjectPanelSettings::override_global(
                ProjectPanelSettings {
                    hide_root: true,
                    ..settings
                },
                cx,
            );
        });

        let panel = workspace.update_in(cx, ProjectPanel::new);
        cx.run_until_parked();

        panel.update(cx, |panel, cx| {
            let project = panel.project.read(cx);
            let worktree = project.visible_worktrees(cx).next().unwrap();
            let root_entry = worktree.read(cx).root_entry().unwrap();
            panel.selection = Some(SelectedEntry {
                worktree_id: worktree.read(cx).id(),
                entry_id: root_entry.id,
            });
        });

        panel.update_in(cx, |panel, window, cx| panel.rename(&Rename, window, cx));

        assert!(
            panel.read_with(cx, |panel, _| panel.state.edit_state.is_none()),
            "当 hide_root=true 且只有一个工作树时,重命名应被阻止"
        );
    }

    // Test 2: Multiple worktrees, hide_root=true - rename should work
    {
        let project = Project::test(fs.clone(), ["/root1".as_ref(), "/root2".as_ref()], cx).await;
        let window =
            cx.add_window(|window, cx| MultiWorkspace::test_new(project.clone(), window, cx));
        let workspace = window
            .read_with(cx, |mw, _| mw.workspace().clone())
            .unwrap();
        let cx = &mut VisualTestContext::from_window(window.into(), cx);

        cx.update(|_, cx| {
            let settings = *ProjectPanelSettings::get_global(cx);
            ProjectPanelSettings::override_global(
                ProjectPanelSettings {
                    hide_root: true,
                    ..settings
                },
                cx,
            );
        });

        let panel = workspace.update_in(cx, ProjectPanel::new);
        cx.run_until_parked();

        select_path(&panel, "root1", cx);
        panel.update_in(cx, |panel, window, cx| panel.rename(&Rename, window, cx));

        #[cfg(target_os = "windows")]
        assert!(
            panel.read_with(cx, |panel, _| panel.state.edit_state.is_none()),
            "在 Windows 上,即使有多个工作树,重命名也应被阻止"
        );

        #[cfg(not(target_os = "windows"))]
        {
            assert!(
                panel.read_with(cx, |panel, _| panel.state.edit_state.is_some()),
                "在非 Windows 系统上,当 hide_root=true 且有多个工作树时,重命名应正常工作"
            );
            panel.update_in(cx, |panel, window, cx| {
                panel.cancel(&menu::Cancel, window, cx)
            });
        }
    }
}

async fn setup_three_worktree_panel(
    cx: &mut gpui::TestAppContext,
) -> (Entity<ProjectPanel>, VisualTestContext) {
    init_test(cx);

    let fs = FakeFs::new(cx.executor());
    fs.insert_tree("/root1", json!({ "a.txt": "" })).await;
    fs.insert_tree("/root2", json!({ "b.txt": "" })).await;
    fs.insert_tree("/root3", json!({ "c.txt": "" })).await;

    let project = Project::test(
        fs.clone(),
        ["/root1".as_ref(), "/root2".as_ref(), "/root3".as_ref()],
        cx,
    )
    .await;
    let window = cx.add_window(|window, cx| MultiWorkspace::test_new(project.clone(), window, cx));
    let workspace = window
        .read_with(cx, |mw, _| mw.workspace().clone())
        .unwrap();
    let mut cx = VisualTestContext::from_window(window.into(), cx);
    let panel = workspace.update_in(&mut cx, ProjectPanel::new);
    cx.run_until_parked();
    (panel, cx)
}

#[gpui::test]
async fn test_drag_worktree_root_reorders_worktrees(cx: &mut gpui::TestAppContext) {
    let (panel, mut cx) = setup_three_worktree_panel(cx).await;
    let cx = &mut cx;

    assert_eq!(
        visible_entries_as_strings(&panel, 0..20, cx),
        &[
            "v root1",
            "      a.txt",
            "v root2",
            "      b.txt",
            "v root3",
            "      c.txt",
        ],
        "工作树应按插入顺序排列"
    );

    // [r1, r2, r3] -> [r2, r1, r3].
    drag_entries_onto(&panel, &["root1"], "root2", false, cx);
    assert_eq!(
        visible_entries_as_strings(&panel, 0..20, cx),
        &[
            "v root2",
            "      b.txt",
            "v root1",
            "      a.txt",
            "v root3",
            "      c.txt",
        ],
        "将 root1 拖到 root2 上应交换它们的位置"
    );

    // [r2, r1, r3] -> [r3, r2, r1].
    drag_entries_onto(&panel, &["root3"], "root2", false, cx);
    assert_eq!(
        visible_entries_as_strings(&panel, 0..20, cx),
        &[
            "v root3",
            "      c.txt",
            "v root2",
            "      b.txt",
            "v root1",
            "      a.txt",
        ],
        "将最后一个根拖到第一个上应将其移到最前面"
    );
}

#[gpui::test]
async fn test_drag_including_worktree_root_only_reorders(cx: &mut gpui::TestAppContext) {
    let (panel, mut cx) = setup_three_worktree_panel(cx).await;
    let cx = &mut cx;

    // Drag {root1, root2/b.txt} onto root3's root entry: only the worktree
    // reorder should happen and b.txt must stay in root2.
    drag_entries_onto(&panel, &["root1", "root2/b.txt"], "root3", false, cx);
    assert_eq!(
        visible_entries_as_strings(&panel, 0..20, cx),
        &[
            "v root2",
            "      b.txt",
            "v root3",
            "      c.txt",
            "v root1",
            "      a.txt",
        ],
        "将混合选择拖放到根上应仅重新排序工作树"
    );

    // Drag {root2, root3/c.txt} onto root1/a.txt (a non-root entry): the root
    // still reorders to root1's position and c.txt must stay in root3.
    drag_entries_onto(&panel, &["root2", "root3/c.txt"], "root1/a.txt", true, cx);
    assert_eq!(
        visible_entries_as_strings(&panel, 0..20, cx),
        &[
            "v root3",
            "      c.txt",
            "v root1",
            "      a.txt",
            "v root2",
            "      b.txt",
        ],
        "将混合选择拖放到非根条目上应仅重新排序工作树"
    );

    // With the copy modifier held, a selection containing a root should still
    // only reorder worktrees and copy nothing.
    cx.simulate_modifiers_change(gpui::Modifiers {
        alt: true,
        control: true,
        ..Default::default()
    });
    drag_entries_onto(&panel, &["root3", "root1/a.txt"], "root2", false, cx);
    cx.simulate_modifiers_change(Default::default());
    assert_eq!(
        visible_entries_as_strings(&panel, 0..20, cx),
        &[
            "v root1",
            "      a.txt",
            "v root2",
            "      b.txt",
            "v root3",
            "      c.txt",
        ],
        "复制拖拽混合选择应仅重新排序工作树且不复制任何内容"
    );
}

#[gpui::test]
async fn test_rename_survives_window_deactivation(cx: &mut gpui::TestAppContext) {
    init_test_with_editor(cx);

    let fs = FakeFs::new(cx.executor());
    fs.insert_tree("/root", json!({ "file1.txt": "content" }))
        .await;

    let project = Project::test(fs.clone(), ["/root".as_ref()], cx).await;
    let window = cx.add_window(|window, cx| MultiWorkspace::test_new(project.clone(), window, cx));
    let workspace = window
        .read_with(cx, |mw, _| mw.workspace().clone())
        .unwrap();
    let cx = &mut VisualTestContext::from_window(window.into(), cx);
    let panel = workspace.update_in(cx, ProjectPanel::new);
    cx.run_until_parked();

    select_path(&panel, "root/file1.txt", cx);
    panel.update_in(cx, |panel, window, cx| panel.rename(&Rename, window, cx));
    assert!(
        panel.read_with(cx, |panel, _| panel.state.edit_state.is_some()),
        "重命名应已开始"
    );

    cx.deactivate_window();

    assert!(
        panel.read_with(cx, |panel, _| panel.state.edit_state.is_some()),
        "当窗口被停用时(例如键盘布局切换器抢占了键盘焦点),重命名不应被取消"
    );
}

#[gpui::test]
async fn test_file_drag_paths_use_worktree_snapshot(cx: &mut gpui::TestAppContext) {
    init_test(cx);

    let temp_dir = tempfile::tempdir().unwrap();
    let root_path = temp_dir.path();
    let existing_a = root_path.join("existing_a.txt");
    let existing_b = root_path.join("existing_b.txt");
    let deleted_directory = root_path.join("deleted_dir");
    std::fs::write(&existing_a, "a").unwrap();
    std::fs::write(&existing_b, "b").unwrap();
    std::fs::create_dir(&deleted_directory).unwrap();
    std::fs::write(deleted_directory.join("nested.txt"), "nested").unwrap();

    let fs = FakeFs::new(cx.executor());
    fs.insert_tree_from_real_fs(root_path, root_path).await;
    std::fs::remove_dir_all(&deleted_directory).unwrap();

    let project = Project::test(fs.clone(), [root_path], cx).await;
    let (worktree_id, existing_a_id, existing_b_id, deleted_directory_id) = cx.update(|cx| {
        let project = project.read(cx);
        let worktree = project.worktrees(cx).next().unwrap();
        let worktree = worktree.read(cx);
        (
            worktree.id(),
            worktree
                .entry_for_path(rel_path("existing_a.txt"))
                .unwrap()
                .id,
            worktree
                .entry_for_path(rel_path("existing_b.txt"))
                .unwrap()
                .id,
            worktree.entry_for_path(rel_path("deleted_dir")).unwrap().id,
        )
    });

    let dragged_selection = DraggedSelection {
        active_selection: SelectedEntry {
            worktree_id,
            entry_id: existing_a_id,
        },
        marked_selections: Arc::from(vec![
            SelectedEntry {
                worktree_id,
                entry_id: existing_a_id,
            },
            SelectedEntry {
                worktree_id,
                entry_id: existing_b_id,
            },
            SelectedEntry {
                worktree_id,
                entry_id: deleted_directory_id,
            },
        ]),
    };

    let paths = cx
        .update(|cx| {
            ProjectPanel::file_drag_paths_for_selections(
                &project,
                dragged_selection.items().copied(),
                cx,
            )
        })
        .unwrap();

    assert_eq!(
        paths.entries(),
        &[
            (existing_a, false),
            (existing_b, false),
            (deleted_directory, true),
        ]
    );
}

#[gpui::test]
async fn test_external_paths_for_dragged_selection_uses_active_selection_unless_marked(
    cx: &mut gpui::TestAppContext,
) {
    init_test(cx);

    let temp_dir = tempfile::tempdir().unwrap();
    let root_path = temp_dir.path();
    let active_path = root_path.join("active.txt");
    let marked_path = root_path.join("marked.txt");
    std::fs::write(&active_path, "active").unwrap();
    std::fs::write(&marked_path, "marked").unwrap();

    let fs = FakeFs::new(cx.executor());
    fs.insert_tree_from_real_fs(root_path, root_path).await;

    let project = Project::test(fs.clone(), [root_path], cx).await;
    let (worktree_id, active_id, marked_id) = cx.update(|cx| {
        let project = project.read(cx);
        let worktree = project.worktrees(cx).next().unwrap();
        let worktree = worktree.read(cx);
        (
            worktree.id(),
            worktree.entry_for_path(rel_path("active.txt")).unwrap().id,
            worktree.entry_for_path(rel_path("marked.txt")).unwrap().id,
        )
    });

    let dragged_selection = DraggedSelection {
        active_selection: SelectedEntry {
            worktree_id,
            entry_id: active_id,
        },
        marked_selections: Arc::from(vec![SelectedEntry {
            worktree_id,
            entry_id: marked_id,
        }]),
    };

    let paths = cx
        .update(|cx| {
            ProjectPanel::file_drag_paths_for_selections(
                &project,
                dragged_selection.items().copied(),
                cx,
            )
        })
        .unwrap();

    assert_eq!(paths.entries(), &[(active_path, false)]);
}

#[gpui::test]
async fn test_external_paths_for_dragged_selection_resolves_folded_directory(
    cx: &mut gpui::TestAppContext,
) {
    init_test(cx);

    let fs = FakeFs::new(cx.executor());
    fs.insert_tree(
        "/root",
        json!({
            "a": {
                "b": {
                    "c": {
                        "d": {}
                    }
                }
            }
        }),
    )
    .await;

    let project = Project::test(fs.clone(), ["/root".as_ref()], cx).await;
    let window = cx.add_window(|window, cx| MultiWorkspace::test_new(project.clone(), window, cx));
    let workspace = window
        .read_with(cx, |multi_workspace, _| multi_workspace.workspace().clone())
        .unwrap();
    let cx = &mut VisualTestContext::from_window(window.into(), cx);

    cx.update(|_, cx| {
        let settings = *ProjectPanelSettings::get_global(cx);
        ProjectPanelSettings::override_global(
            ProjectPanelSettings {
                auto_fold_dirs: true,
                ..settings
            },
            cx,
        );
    });

    let panel = workspace.update_in(cx, ProjectPanel::new);
    cx.run_until_parked();
    select_folded_path_with_mark(&panel, "root/a/b/c/d", "root/a/b", cx);

    let dragged_selection = panel.read_with(cx, |panel, _| DraggedSelection {
        active_selection: panel.selection.unwrap(),
        marked_selections: Arc::from(panel.marked_entries.clone()),
    });
    let paths = cx
        .update(|_, cx| {
            panel.read_with(cx, |panel, cx| {
                panel.external_paths_for_dragged_selection(&dragged_selection, cx)
            })
        })
        .unwrap();

    assert_eq!(paths.entries(), &[(PathBuf::from("/root/a/b"), true)]);
}

#[gpui::test]
async fn test_external_paths_for_dragged_selection_skips_remote_worktrees(
    cx: &mut gpui::TestAppContext,
) {
    init_test(cx);

    let fs = FakeFs::new(cx.executor());
    fs.insert_tree("/local", json!({})).await;
    let project = Project::test(fs.clone(), ["/local".as_ref()], cx).await;

    let remote_worktree = project.update(cx, |project, cx| {
        project.add_test_remote_worktree("/remote/project", cx)
    });
    let remote_worktree_id = remote_worktree.read_with(cx, |worktree, _| worktree.id());

    let dragged_selection = DraggedSelection {
        active_selection: SelectedEntry {
            worktree_id: remote_worktree_id,
            entry_id: ProjectEntryId::from_usize(1),
        },
        marked_selections: Arc::from(vec![SelectedEntry {
            worktree_id: remote_worktree_id,
            entry_id: ProjectEntryId::from_usize(1),
        }]),
    };

    let paths = cx.update(|cx| {
        ProjectPanel::file_drag_paths_for_selections(
            &project,
            dragged_selection.items().copied(),
            cx,
        )
    });

    assert!(paths.is_none());
}

#[gpui::test]
async fn test_multiple_marked_entries(cx: &mut gpui::TestAppContext) {
    init_test_with_editor(cx);
    let fs = FakeFs::new(cx.executor());
    fs.insert_tree(
        "/project_root",
        json!({
            "dir_1": {
                "nested_dir": {
                    "file_a.py": "# File contents",
                }
            },
            "file_1.py": "# File contents",
        }),
    )
    .await;

    let project = Project::test(fs.clone(), ["/project_root".as_ref()], cx).await;
    let worktree_id = cx.update(|cx| project.read(cx).worktrees(cx).next().unwrap().read(cx).id());
    let window = cx.add_window(|window, cx| MultiWorkspace::test_new(project.clone(), window, cx));
    let workspace = window
        .read_with(cx, |mw, _| mw.workspace().clone())
        .unwrap();
    let cx = &mut VisualTestContext::from_window(window.into(), cx);
    let panel = workspace.update_in(cx, ProjectPanel::new);
    cx.run_until_parked();

    cx.update(|window, cx| {
        panel.update(cx, |this, cx| {
            this.select_next(&Default::default(), window, cx);
            this.expand_selected_entry(&Default::default(), window, cx);
        })
    });
    cx.run_until_parked();

    cx.update(|window, cx| {
        panel.update(cx, |this, cx| {
            this.expand_selected_entry(&Default::default(), window, cx);
        })
    });
    cx.run_until_parked();

    cx.update(|window, cx| {
        panel.update(cx, |this, cx| {
            this.select_next(&Default::default(), window, cx);
            this.expand_selected_entry(&Default::default(), window, cx);
        })
    });
    cx.run_until_parked();

    cx.update(|window, cx| {
        panel.update(cx, |this, cx| {
            this.select_next(&Default::default(), window, cx);
        })
    });
    cx.run_until_parked();

    assert_eq!(
        visible_entries_as_strings(&panel, 0..10, cx),
        &[
            "v project_root",
            "    v dir_1",
            "        v nested_dir",
            "              file_a.py  <== selected",
            "      file_1.py",
        ]
    );
    let modifiers_with_shift = gpui::Modifiers {
        shift: true,
        ..Default::default()
    };
    cx.run_until_parked();
    cx.simulate_modifiers_change(modifiers_with_shift);
    cx.update(|window, cx| {
        panel.update(cx, |this, cx| {
            this.select_next(&Default::default(), window, cx);
        })
    });
    assert_eq!(
        visible_entries_as_strings(&panel, 0..10, cx),
        &[
            "v project_root",
            "    v dir_1",
            "        v nested_dir",
            "              file_a.py",
            "      file_1.py  <== selected  <== marked",
        ]
    );
    cx.update(|window, cx| {
        panel.update(cx, |this, cx| {
            this.select_previous(&Default::default(), window, cx);
        })
    });
    assert_eq!(
        visible_entries_as_strings(&panel, 0..10, cx),
        &[
            "v project_root",
            "    v dir_1",
            "        v nested_dir",
            "              file_a.py  <== selected  <== marked",
            "      file_1.py  <== marked",
        ]
    );
    cx.update(|window, cx| {
        panel.update(cx, |this, cx| {
            let drag = DraggedSelection {
                active_selection: this.selection.unwrap(),
                marked_selections: this.marked_entries.clone().into(),
            };
            let target_entry = this
                .project
                .read(cx)
                .entry_for_path(&(worktree_id, rel_path("")).into(), cx)
                .unwrap();
            this.drag_onto(&drag, target_entry.id, false, window, cx);
        });
    });
    cx.run_until_parked();
    assert_eq!(
        visible_entries_as_strings(&panel, 0..10, cx),
        &[
            "v project_root",
            "    v dir_1",
            "        v nested_dir",
            "      file_1.py  <== marked",
            "      file_a.py  <== selected  <== marked",
        ]
    );
    // ESC clears out all marks
    cx.update(|window, cx| {
        panel.update(cx, |this, cx| {
            this.cancel(&menu::Cancel, window, cx);
        })
    });
    cx.executor().run_until_parked();
    assert_eq!(
        visible_entries_as_strings(&panel, 0..10, cx),
        &[
            "v project_root",
            "    v dir_1",
            "        v nested_dir",
            "      file_1.py",
            "      file_a.py  <== selected",
        ]
    );
    // ESC clears out all marks
    cx.update(|window, cx| {
        panel.update(cx, |this, cx| {
            this.select_previous(&SelectPrevious, window, cx);
            this.select_next(&SelectNext, window, cx);
        })
    });
    assert_eq!(
        visible_entries_as_strings(&panel, 0..10, cx),
        &[
            "v project_root",
            "    v dir_1",
            "        v nested_dir",
            "      file_1.py  <== marked",
            "      file_a.py  <== selected  <== marked",
        ]
    );
    cx.simulate_modifiers_change(Default::default());
    cx.update(|window, cx| {
        panel.update(cx, |this, cx| {
            this.cut(&Cut, window, cx);
            this.select_previous(&SelectPrevious, window, cx);
            this.select_previous(&SelectPrevious, window, cx);

            this.paste(&Paste, window, cx);
            this.update_visible_entries(None, false, false, window, cx);
        })
    });
    cx.run_until_parked();
    assert_eq!(
        visible_entries_as_strings(&panel, 0..10, cx),
        &[
            "v project_root",
            "    v dir_1",
            "        v nested_dir",
            "              file_1.py  <== marked",
            "              file_a.py  <== selected  <== marked",
        ]
    );
    cx.simulate_modifiers_change(modifiers_with_shift);
    cx.update(|window, cx| {
        panel.update(cx, |this, cx| {
            this.expand_selected_entry(&Default::default(), window, cx);
            this.select_next(&SelectNext, window, cx);
            this.select_next(&SelectNext, window, cx);
        })
    });
    submit_deletion(&panel, cx);
    assert_eq!(
        visible_entries_as_strings(&panel, 0..10, cx),
        &[
            "v project_root",
            "    v dir_1",
            "        v nested_dir  <== selected",
        ]
    );
}

#[gpui::test]
async fn test_dragged_selection_resolve_entry(cx: &mut gpui::TestAppContext) {
    init_test(cx);

    let fs = FakeFs::new(cx.executor());
    fs.insert_tree(
        "/root",
        json!({
            "a": {
                "b": {
                    "c": {
                        "d": {}
                    }
                }
            },
            "target_destination": {}
        }),
    )
    .await;

    let project = Project::test(fs.clone(), ["/root".as_ref()], cx).await;
    let window = cx.add_window(|window, cx| MultiWorkspace::test_new(project.clone(), window, cx));
    let workspace = window
        .read_with(cx, |mw, _| mw.workspace().clone())
        .unwrap();
    let cx = &mut VisualTestContext::from_window(window.into(), cx);

    cx.update(|_, cx| {
        let settings = *ProjectPanelSettings::get_global(cx);
        ProjectPanelSettings::override_global(
            ProjectPanelSettings {
                auto_fold_dirs: true,
                ..settings
            },
            cx,
        );
    });

    let panel = workspace.update_in(cx, ProjectPanel::new);
    cx.run_until_parked();

    // Case 1: Move last dir 'd' - should move only 'd', leaving 'a/b/c'
    select_path(&panel, "root/a/b/c/d", cx);
    panel.update_in(cx, |panel, window, cx| {
        let drag = DraggedSelection {
            active_selection: *panel.selection.as_ref().unwrap(),
            marked_selections: Arc::new([*panel.selection.as_ref().unwrap()]),
        };
        let target_entry = panel
            .project
            .read(cx)
            .visible_worktrees(cx)
            .next()
            .unwrap()
            .read(cx)
            .entry_for_path(rel_path("target_destination"))
            .unwrap();
        panel.drag_onto(&drag, target_entry.id, false, window, cx);
    });
    cx.executor().run_until_parked();

    assert_eq!(
        visible_entries_as_strings(&panel, 0..10, cx),
        &[
            "v root",
            "    > a/b/c",
            "    > target_destination/d  <== selected"
        ],
        "移动最后一个空目录 'd' 应保留 'a/b/c' 并仅移动 'd'"
    );

    // Reset
    select_path(&panel, "root/target_destination/d", cx);
    panel.update_in(cx, |panel, window, cx| {
        let drag = DraggedSelection {
            active_selection: *panel.selection.as_ref().unwrap(),
            marked_selections: Arc::new([*panel.selection.as_ref().unwrap()]),
        };
        let target_entry = panel
            .project
            .read(cx)
            .visible_worktrees(cx)
            .next()
            .unwrap()
            .read(cx)
            .entry_for_path(rel_path("a/b/c"))
            .unwrap();
        panel.drag_onto(&drag, target_entry.id, false, window, cx);
    });
    cx.executor().run_until_parked();

    // Case 2: Move middle dir 'b' - should move 'b/c/d', leaving only 'a'
    select_path(&panel, "root/a/b", cx);
    panel.update_in(cx, |panel, window, cx| {
        let drag = DraggedSelection {
            active_selection: *panel.selection.as_ref().unwrap(),
            marked_selections: Arc::new([*panel.selection.as_ref().unwrap()]),
        };
        let target_entry = panel
            .project
            .read(cx)
            .visible_worktrees(cx)
            .next()
            .unwrap()
            .read(cx)
            .entry_for_path(rel_path("target_destination"))
            .unwrap();
        panel.drag_onto(&drag, target_entry.id, false, window, cx);
    });
    cx.executor().run_until_parked();

    assert_eq!(
        visible_entries_as_strings(&panel, 0..10, cx),
        &["v root", "    v a", "    > target_destination/b/c/d"],
        "移动中间目录 'b' 应仅保留 'a' 并移动 'b/c/d'"
    );

    // Reset
    select_path(&panel, "root/target_destination/b", cx);
    panel.update_in(cx, |panel, window, cx| {
        let drag = DraggedSelection {
            active_selection: *panel.selection.as_ref().unwrap(),
            marked_selections: Arc::new([*panel.selection.as_ref().unwrap()]),
        };
        let target_entry = panel
            .project
            .read(cx)
            .visible_worktrees(cx)
            .next()
            .unwrap()
            .read(cx)
            .entry_for_path(rel_path("a"))
            .unwrap();
        panel.drag_onto(&drag, target_entry.id, false, window, cx);
    });
    cx.executor().run_until_parked();

    // Case 3: Move first dir 'a' - should move whole 'a/b/c/d'
    select_path(&panel, "root/a", cx);
    panel.update_in(cx, |panel, window, cx| {
        let drag = DraggedSelection {
            active_selection: *panel.selection.as_ref().unwrap(),
            marked_selections: Arc::new([*panel.selection.as_ref().unwrap()]),
        };
        let target_entry = panel
            .project
            .read(cx)
            .visible_worktrees(cx)
            .next()
            .unwrap()
            .read(cx)
            .entry_for_path(rel_path("target_destination"))
            .unwrap();
        panel.drag_onto(&drag, target_entry.id, false, window, cx);
    });
    cx.executor().run_until_parked();

    assert_eq!(
        visible_entries_as_strings(&panel, 0..10, cx),
        &["v root", "    > target_destination/a/b/c/d"],
        "移动第一个目录 'a' 应移动整个 'a/b/c/d' 链"
    );
}

#[gpui::test]
async fn test_drag_marked_entries_in_folded_directories(cx: &mut gpui::TestAppContext) {
    init_test(cx);

    let fs = FakeFs::new(cx.executor());
    fs.insert_tree(
        "/root",
        json!({
            "a": {
                "b": {
                    "c": {}
                }
            },
            "e": {
                "f": {
                    "g": {}
                }
            },
            "target": {}
        }),
    )
    .await;

    let project = Project::test(fs.clone(), ["/root".as_ref()], cx).await;
    let window = cx.add_window(|window, cx| MultiWorkspace::test_new(project.clone(), window, cx));
    let workspace = window
        .read_with(cx, |mw, _| mw.workspace().clone())
        .unwrap();
    let cx = &mut VisualTestContext::from_window(window.into(), cx);

    cx.update(|_, cx| {
        let settings = *ProjectPanelSettings::get_global(cx);
        ProjectPanelSettings::override_global(
            ProjectPanelSettings {
                auto_fold_dirs: true,
                ..settings
            },
            cx,
        );
    });

    let panel = workspace.update_in(cx, ProjectPanel::new);
    cx.run_until_parked();

    assert_eq!(
        visible_entries_as_strings(&panel, 0..10, cx),
        &["v root", "    > a/b/c", "    > e/f/g", "    > target"]
    );

    select_folded_path_with_mark(&panel, "root/a/b/c", "root/a/b", cx);
    select_folded_path_with_mark(&panel, "root/e/f/g", "root/e/f", cx);

    panel.update_in(cx, |panel, window, cx| {
        let drag = DraggedSelection {
            active_selection: *panel.selection.as_ref().unwrap(),
            marked_selections: panel.marked_entries.clone().into(),
        };
        let target_entry = panel
            .project
            .read(cx)
            .visible_worktrees(cx)
            .next()
            .unwrap()
            .read(cx)
            .entry_for_path(rel_path("target"))
            .unwrap();
        panel.drag_onto(&drag, target_entry.id, false, window, cx);
    });
    cx.executor().run_until_parked();

    // After dragging 'b/c' and 'f/g' should be moved to target
    assert_eq!(
        visible_entries_as_strings(&panel, 0..10, cx),
        &[
            "v root",
            "    > a",
            "    > e",
            "    v target",
            "        > b/c",
            "        > f/g  <== selected  <== marked"
        ],
        "应将 'b/c' 和 'f/g' 移动到目标位置,保留 'a' 和 'e'"
    );
}

#[gpui::test]
async fn test_dragging_same_named_files_preserves_one_source_on_conflict(
    cx: &mut gpui::TestAppContext,
) {
    init_test(cx);

    let fs = FakeFs::new(cx.executor());
    fs.insert_tree(
        "/root",
        json!({
            "dir_a": {
                "shared.txt": "from a"
            },
            "dir_b": {
                "shared.txt": "from b"
            }
        }),
    )
    .await;

    let project = Project::test(fs.clone(), ["/root".as_ref()], cx).await;
    let window = cx.add_window(|window, cx| MultiWorkspace::test_new(project.clone(), window, cx));
    let workspace = window
        .read_with(cx, |multi_workspace, _| multi_workspace.workspace().clone())
        .unwrap();
    let cx = &mut VisualTestContext::from_window(window.into(), cx);
    let panel = workspace.update_in(cx, ProjectPanel::new);
    cx.run_until_parked();

    panel.update_in(cx, |panel, window, cx| {
        let (root_entry_id, worktree_id, entry_a_id, entry_b_id) = {
            let worktree = panel.project.read(cx).visible_worktrees(cx).next().unwrap();
            let worktree = worktree.read(cx);
            let root_entry_id = worktree.root_entry().unwrap().id;
            let worktree_id = worktree.id();
            let entry_a_id = worktree
                .entry_for_path(rel_path("dir_a/shared.txt"))
                .unwrap()
                .id;
            let entry_b_id = worktree
                .entry_for_path(rel_path("dir_b/shared.txt"))
                .unwrap()
                .id;
            (root_entry_id, worktree_id, entry_a_id, entry_b_id)
        };

        let drag = DraggedSelection {
            active_selection: SelectedEntry {
                worktree_id,
                entry_id: entry_a_id,
            },
            marked_selections: Arc::new([
                SelectedEntry {
                    worktree_id,
                    entry_id: entry_a_id,
                },
                SelectedEntry {
                    worktree_id,
                    entry_id: entry_b_id,
                },
            ]),
        };

        panel.drag_onto(&drag, root_entry_id, false, window, cx);
    });
    cx.executor().run_until_parked();

    let files = fs.files();
    assert!(files.contains(&PathBuf::from(path!("/root/shared.txt"))));

    let remaining_sources = [
        PathBuf::from(path!("/root/dir_a/shared.txt")),
        PathBuf::from(path!("/root/dir_b/shared.txt")),
    ]
    .into_iter()
    .filter(|path| files.contains(path))
    .count();

    assert_eq!(
        remaining_sources, 1,
        "一个冲突的源文件应保留在原位"
    );
}

#[gpui::test]
async fn test_drag_entries_between_different_worktrees(cx: &mut gpui::TestAppContext) {
    init_test(cx);

    let fs = FakeFs::new(cx.executor());
    fs.insert_tree(
        "/root_a",
        json!({
            "src": {
                "lib.rs": "",
                "main.rs": ""
            },
            "docs": {
                "guide.md": ""
            },
            "multi": {
                "alpha.txt": "",
                "beta.txt": ""
            }
        }),
    )
    .await;
    fs.insert_tree(
        "/root_b",
        json!({
            "dst": {
                "existing.md": ""
            },
            "target.txt": ""
        }),
    )
    .await;

    let project = Project::test(fs.clone(), ["/root_a".as_ref(), "/root_b".as_ref()], cx).await;
    let window = cx.add_window(|window, cx| MultiWorkspace::test_new(project.clone(), window, cx));
    let workspace = window
        .read_with(cx, |mw, _| mw.workspace().clone())
        .unwrap();
    let cx = &mut VisualTestContext::from_window(window.into(), cx);
    let panel = workspace.update_in(cx, ProjectPanel::new);
    cx.run_until_parked();

    // Case 1: move a file onto a directory in another worktree.
    select_path(&panel, "root_a/src/main.rs", cx);
    drag_selection_to(&panel, "root_b/dst", false, cx);
    assert!(
        find_project_entry(&panel, "root_b/dst/main.rs", cx).is_some(),
        "拖拽的文件应出现在目标工作树下"
    );
    assert_eq!(
        find_project_entry(&panel, "root_a/src/main.rs", cx),
        None,
        "拖拽的文件应从源工作树中移除"
    );

    // Case 2: drop a file onto another worktree file so it lands in the parent directory.
    select_path(&panel, "root_a/docs/guide.md", cx);
    drag_selection_to(&panel, "root_b/dst/existing.md", true, cx);
    assert!(
        find_project_entry(&panel, "root_b/dst/guide.md", cx).is_some(),
        "拖放到文件上应将条目放置在目标文件旁边"
    );
    assert_eq!(
        find_project_entry(&panel, "root_a/docs/guide.md", cx),
        None,
        "移动后源文件应被移除"
    );

    // Case 3: move an entire directory.
    select_path(&panel, "root_a/src", cx);
    drag_selection_to(&panel, "root_b/dst", false, cx);
    assert!(
        find_project_entry(&panel, "root_b/dst/src/lib.rs", cx).is_some(),
        "拖拽目录应移动其嵌套内容"
    );
    assert_eq!(
        find_project_entry(&panel, "root_a/src", cx),
        None,
        "源工作树中不应再存在该目录"
    );

    // Case 4: multi-selection drag between worktrees.
    panel.update(cx, |panel, _| panel.marked_entries.clear());
    select_path_with_mark(&panel, "root_a/multi/alpha.txt", cx);
    select_path_with_mark(&panel, "root_a/multi/beta.txt", cx);
    drag_selection_to(&panel, "root_b/dst", false, cx);
    assert!(
        find_project_entry(&panel, "root_b/dst/alpha.txt", cx).is_some()
            && find_project_entry(&panel, "root_b/dst/beta.txt", cx).is_some(),
        "所有标记的条目都应移动到目标工作树"
    );
    assert_eq!(
        find_project_entry(&panel, "root_a/multi/alpha.txt", cx),
        None,
        "标记的条目应从源工作树中移除"
    );
    assert_eq!(
        find_project_entry(&panel, "root_a/multi/beta.txt", cx),
        None,
        "标记的条目应从源工作树中移除"
    );
}

#[gpui::test]
async fn test_drag_multiple_entries(cx: &mut gpui::TestAppContext) {
    init_test(cx);

    let fs = FakeFs::new(cx.executor());
    fs.insert_tree(
        "/root",
        json!({
            "src": {
                "folder1": {
                    "mod.rs": "// folder1 mod"
                },
                "folder2": {
                    "mod.rs": "// folder2 mod"
                },
                "folder3": {
                    "mod.rs": "// folder3 mod",
                    "helper.rs": "// helper"
                },
                "main.rs": ""
            }
        }),
    )
    .await;

    let project = Project::test(fs.clone(), ["/root".as_ref()], cx).await;
    let window = cx.add_window(|window, cx| MultiWorkspace::test_new(project.clone(), window, cx));
    let workspace = window
        .read_with(cx, |mw, _| mw.workspace().clone())
        .unwrap();
    let cx = &mut VisualTestContext::from_window(window.into(), cx);
    let panel = workspace.update_in(cx, ProjectPanel::new);
    cx.run_until_parked();

    toggle_expand_dir(&panel, "root/src", cx);
    toggle_expand_dir(&panel, "root/src/folder1", cx);
    toggle_expand_dir(&panel, "root/src/folder2", cx);
    toggle_expand_dir(&panel, "root/src/folder3", cx);
    cx.run_until_parked();

    // Case 1: Dragging a folder and a file from a sibling folder together.
    panel.update(cx, |panel, _| panel.marked_entries.clear());
    select_path_with_mark(&panel, "root/src/folder1", cx);
    select_path_with_mark(&panel, "root/src/folder2/mod.rs", cx);

    drag_selection_to(&panel, "root", false, cx);

    assert!(
        find_project_entry(&panel, "root/folder1", cx).is_some(),
        "拖拽后 folder1 应位于根目录"
    );
    assert!(
        find_project_entry(&panel, "root/folder1/mod.rs", cx).is_some(),
        "拖拽后 folder1/mod.rs 应仍在 folder1 内"
    );
    assert_eq!(
        find_project_entry(&panel, "root/src/folder1", cx),
        None,
        "folder1 不应再位于 src 中"
    );
    assert!(
        find_project_entry(&panel, "root/mod.rs", cx).is_some(),
        "来自 folder2 的 mod.rs 应位于根目录"
    );

    // Case 2: Dragging a folder and its own child together.
    panel.update(cx, |panel, _| panel.marked_entries.clear());
    select_path_with_mark(&panel, "root/src/folder3", cx);
    select_path_with_mark(&panel, "root/src/folder3/mod.rs", cx);

    drag_selection_to(&panel, "root", false, cx);

    assert!(
        find_project_entry(&panel, "root/folder3", cx).is_some(),
        "拖拽后 folder3 应位于根目录"
    );
    assert!(
        find_project_entry(&panel, "root/folder3/mod.rs", cx).is_some(),
        "folder3/mod.rs 应仍在 folder3 内"
    );
    assert!(
        find_project_entry(&panel, "root/folder3/helper.rs", cx).is_some(),
        "folder3/helper.rs 应仍在 folder3 内"
    );
}

#[gpui::test]
async fn test_autoreveal_and_gitignored_files(cx: &mut gpui::TestAppContext) {
    init_test_with_editor(cx);
    cx.update(|cx| {
        cx.update_global::<SettingsStore, _>(|store, cx| {
            store.update_user_settings(cx, |settings| {
                settings.project.worktree.file_scan_exclusions = Some(Vec::new());
                settings
                    .project_panel
                    .get_or_insert_default()
                    .auto_reveal_entries = Some(false);
            });
        })
    });

    let fs = FakeFs::new(cx.background_executor.clone());
    fs.insert_tree(
        "/project_root",
        json!({
            ".git": {},
            ".gitignore": "**/gitignored_dir",
            "dir_1": {
                "file_1.py": "# File 1_1 contents",
                "file_2.py": "# File 1_2 contents",
                "file_3.py": "# File 1_3 contents",
                "gitignored_dir": {
                    "file_a.py": "# File contents",
                    "file_b.py": "# File contents",
                    "file_c.py": "# File contents",
                },
            },
            "dir_2": {
                "file_1.py": "# File 2_1 contents",
                "file_2.py": "# File 2_2 contents",
                "file_3.py": "# File 2_3 contents",
            }
        }),
    )
    .await;

    let project = Project::test(fs.clone(), ["/project_root".as_ref()], cx).await;
    let window = cx.add_window(|window, cx| MultiWorkspace::test_new(project.clone(), window, cx));
    let workspace = window
        .read_with(cx, |mw, _| mw.workspace().clone())
        .unwrap();
    let cx = &mut VisualTestContext::from_window(window.into(), cx);
    let panel = workspace.update_in(cx, ProjectPanel::new);
    cx.run_until_parked();

    assert_eq!(
        visible_entries_as_strings(&panel, 0..20, cx),
        &[
            "v project_root",
            "    > .git",
            "    > dir_1",
            "    > dir_2",
            "      .gitignore",
        ]
    );

    let dir_1_file = find_project_entry(&panel, "project_root/dir_1/file_1.py", cx)
        .expect("dir 1 中的文件未被忽略,应有一个条目");
    let dir_2_file = find_project_entry(&panel, "project_root/dir_2/file_1.py", cx)
        .expect("dir 2 中的文件未被忽略,应有一个条目");
    let gitignored_dir_file =
        find_project_entry(&panel, "project_root/dir_1/gitignored_dir/file_a.py", cx);
    assert_eq!(
        gitignored_dir_file, None,
        "在切换目录之前,gitignored 目录中的文件不应有条目"
    );

    toggle_expand_dir(&panel, "project_root/dir_1", cx);
    toggle_expand_dir(&panel, "project_root/dir_1/gitignored_dir", cx);
    cx.executor().run_until_parked();
    assert_eq!(
        visible_entries_as_strings(&panel, 0..20, cx),
        &[
            "v project_root",
            "    > .git",
            "    v dir_1",
            "        v gitignored_dir  <== selected",
            "              file_a.py",
            "              file_b.py",
            "              file_c.py",
            "          file_1.py",
            "          file_2.py",
            "          file_3.py",
            "    > dir_2",
            "      .gitignore",
        ],
        "应在项目面板中显示 gitignored 目录的文件列表"
    );
    let gitignored_dir_file =
        find_project_entry(&panel, "project_root/dir_1/gitignored_dir/file_a.py", cx)
            .expect("gitignored 目录打开后,应存在文件条目");

    toggle_expand_dir(&panel, "project_root/dir_1/gitignored_dir", cx);
    toggle_expand_dir(&panel, "project_root/dir_1", cx);
    assert_eq!(
        visible_entries_as_strings(&panel, 0..20, cx),
        &[
            "v project_root",
            "    > .git",
            "    > dir_1  <== selected",
            "    > dir_2",
            "      .gitignore",
        ],
        "应再次隐藏所有目录内容,并为自动显示测试做准备"
    );

    for file_entry in [dir_1_file, dir_2_file, gitignored_dir_file] {
        panel.update(cx, |panel, cx| {
            panel.project.update(cx, |_, cx| {
                cx.emit(project::Event::ActiveEntryChanged(Some(file_entry)))
            })
        });
        cx.run_until_parked();
        assert_eq!(
            visible_entries_as_strings(&panel, 0..20, cx),
            &[
                "v project_root",
                "    > .git",
                "    > dir_1  <== selected",
                "    > dir_2",
                "      .gitignore",
            ],
            "当未启用自动显示时,选中的条目不应在项目面板中显示"
        );
    }

    cx.update(|_, cx| {
        cx.update_global::<SettingsStore, _>(|store, cx| {
            store.update_user_settings(cx, |settings| {
                settings
                    .project_panel
                    .get_or_insert_default()
                    .auto_reveal_entries = Some(true)
            });
        })
    });

    panel.update(cx, |panel, cx| {
        panel.project.update(cx, |_, cx| {
            cx.emit(project::Event::ActiveEntryChanged(Some(dir_1_file)))
        })
    });
    cx.run_until_parked();
    assert_eq!(
        visible_entries_as_strings(&panel, 0..20, cx),
        &[
            "v project_root",
            "    > .git",
            "    v dir_1",
            "        > gitignored_dir",
            "          file_1.py  <== selected  <== marked",
            "          file_2.py",
            "          file_3.py",
            "    > dir_2",
            "      .gitignore",
        ],
        "当启用自动显示时,未被忽略的 dir_1 条目应被显示"
    );

    panel.update(cx, |panel, cx| {
        panel.project.update(cx, |_, cx| {
            cx.emit(project::Event::ActiveEntryChanged(Some(dir_2_file)))
        })
    });
    cx.run_until_parked();
    assert_eq!(
        visible_entries_as_strings(&panel, 0..20, cx),
        &[
            "v project_root",
            "    > .git",
            "    v dir_1",
            "        > gitignored_dir",
            "          file_1.py",
            "          file_2.py",
            "          file_3.py",
            "    v dir_2",
            "          file_1.py  <== selected  <== marked",
            "          file_2.py",
            "          file_3.py",
            "      .gitignore",
        ],
        "当启用自动显示时,未被忽略的 dir_2 条目应被显示"
    );

    panel.update(cx, |panel, cx| {
        panel.project.update(cx, |_, cx| {
            cx.emit(project::Event::ActiveEntryChanged(Some(
                gitignored_dir_file,
            )))
        })
    });
    cx.run_until_parked();
    assert_eq!(
        visible_entries_as_strings(&panel, 0..20, cx),
        &[
            "v project_root",
            "    > .git",
            "    v dir_1",
            "        > gitignored_dir",
            "          file_1.py",
            "          file_2.py",
            "          file_3.py",
            "    v dir_2",
            "          file_1.py  <== selected  <== marked",
            "          file_2.py",
            "          file_3.py",
            "      .gitignore",
        ],
        "当启用自动显示时,gitignored 的选中条目不应在项目面板中显示"
    );

    panel.update(cx, |panel, cx| {
        panel.project.update(cx, |_, cx| {
            cx.emit(project::Event::RevealInProjectPanel(gitignored_dir_file))
        })
    });
    cx.run_until_parked();
    assert_eq!(
        visible_entries_as_strings(&panel, 0..20, cx),
        &[
            "v project_root",
            "    > .git",
            "    v dir_1",
            "        v gitignored_dir",
            "              file_a.py  <== selected  <== marked",
            "              file_b.py",
            "              file_c.py",
            "          file_1.py",
            "          file_2.py",
            "          file_3.py",
            "    v dir_2",
            "          file_1.py",
            "          file_2.py",
            "          file_3.py",
            "      .gitignore",
        ],
        "当显式显示 gitignored 条目时,它应出现在项目树中"
    );

    panel.update(cx, |panel, cx| {
        panel.project.update(cx, |_, cx| {
            cx.emit(project::Event::ActiveEntryChanged(Some(dir_2_file)))
        })
    });
    cx.run_until_parked();
    assert_eq!(
        visible_entries_as_strings(&panel, 0..20, cx),
        &[
            "v project_root",
            "    > .git",
            "    v dir_1",
            "        v gitignored_dir",
            "              file_a.py",
            "              file_b.py",
            "              file_c.py",
            "          file_1.py",
            "          file_2.py",
            "          file_3.py",
            "    v dir_2",
            "          file_1.py  <== selected  <== marked",
            "          file_2.py",
            "          file_3.py",
            "      .gitignore",
        ],
        "切换到 dir_2_file 后,它应被选中并标记"
    );

    panel.update(cx, |panel, cx| {
        panel.project.update(cx, |_, cx| {
            cx.emit(project::Event::ActiveEntryChanged(Some(
                gitignored_dir_file,
            )))
        })
    });
    cx.run_until_parked();
    assert_eq!(
        visible_entries_as_strings(&panel, 0..20, cx),
        &[
            "v project_root",
            "    > .git",
            "    v dir_1",
            "        v gitignored_dir",
            "              file_a.py  <== selected  <== marked",
            "              file_b.py",
            "              file_c.py",
            "          file_1.py",
            "          file_2.py",
            "          file_3.py",
            "    v dir_2",
            "          file_1.py",
            "          file_2.py",
            "          file_3.py",
            "      .gitignore",
        ],
        "当 gitignored 条目已可见时,自动显示应将其标记为选中"
    );
}

#[gpui::test]
async fn test_gitignored_and_always_included(cx: &mut gpui::TestAppContext) {
    init_test_with_editor(cx);
    cx.update(|cx| {
        cx.update_global::<SettingsStore, _>(|store, cx| {
            store.update_user_settings(cx, |settings| {
                settings.project.worktree.file_scan_exclusions = Some(Vec::new());
                settings.project.worktree.file_scan_inclusions =
                    Some(vec!["always_included_but_ignored_dir/*".to_string()]);
                settings
                    .project_panel
                    .get_or_insert_default()
                    .auto_reveal_entries = Some(false)
            });
        })
    });

    let fs = FakeFs::new(cx.background_executor.clone());
    fs.insert_tree(
        "/project_root",
        json!({
            ".git": {},
            ".gitignore": "**/gitignored_dir\n/always_included_but_ignored_dir",
            "dir_1": {
                "file_1.py": "# File 1_1 contents",
                "file_2.py": "# File 1_2 contents",
                "file_3.py": "# File 1_3 contents",
                "gitignored_dir": {
                    "file_a.py": "# File contents",
                    "file_b.py": "# File contents",
                    "file_c.py": "# File contents",
                },
            },
            "dir_2": {
                "file_1.py": "# File 2_1 contents",
                "file_2.py": "# File 2_2 contents",
                "file_3.py": "# File 2_3 contents",
            },
            "always_included_but_ignored_dir": {
                "file_a.py": "# File contents",
                "file_b.py": "# File contents",
                "file_c.py": "# File contents",
            },
        }),
    )
    .await;

    let project = Project::test(fs.clone(), ["/project_root".as_ref()], cx).await;
    let window = cx.add_window(|window, cx| MultiWorkspace::test_new(project.clone(), window, cx));
    let workspace = window
        .read_with(cx, |mw, _| mw.workspace().clone())
        .unwrap();
    let cx = &mut VisualTestContext::from_window(window.into(), cx);
    let panel = workspace.update_in(cx, ProjectPanel::new);
    cx.run_until_parked();

    assert_eq!(
        visible_entries_as_strings(&panel, 0..20, cx),
        &[
            "v project_root",
            "    > .git",
            "    > always_included_but_ignored_dir",
            "    > dir_1",
            "    > dir_2",
            "      .gitignore",
        ]
    );

    let gitignored_dir_file =
        find_project_entry(&panel, "project_root/dir_1/gitignored_dir/file_a.py", cx);
    let always_included_but_ignored_dir_file = find_project_entry(
        &panel,
        "project_root/always_included_but_ignored_dir/file_a.py",
        cx,
    )
    .expect("被 .gitignore 忽略但设置为始终包含的文件应有一个条目");
    assert_eq!(
        gitignored_dir_file, None,
        "除非切换其目录,否则 gitignored 目录中的文件不应有条目"
    );

    toggle_expand_dir(&panel, "project_root/dir_1", cx);
    cx.run_until_parked();
    cx.update(|_, cx| {
        cx.update_global::<SettingsStore, _>(|store, cx| {
            store.update_user_settings(cx, |settings| {
                settings
                    .project_panel
                    .get_or_insert_default()
                    .auto_reveal_entries = Some(true)
            });
        })
    });

    panel.update(cx, |panel, cx| {
        panel.project.update(cx, |_, cx| {
            cx.emit(project::Event::ActiveEntryChanged(Some(
                always_included_but_ignored_dir_file,
            )))
        })
    });
    cx.run_until_parked();

    assert_eq!(
        visible_entries_as_strings(&panel, 0..20, cx),
        &[
            "v project_root",
            "    > .git",
            "    v always_included_but_ignored_dir",
            "          file_a.py  <== selected  <== marked",
            "          file_b.py",
            "          file_c.py",
            "    v dir_1",
            "        > gitignored_dir",
            "          file_1.py",
            "          file_2.py",
            "          file_3.py",
            "    > dir_2",
            "      .gitignore",
        ],
        "当启用自动显示时,gitignored 但始终包含的选中条目应在项目面板中显示"
    );
}

#[gpui::test]
async fn test_explicit_reveal(cx: &mut gpui::TestAppContext) {
    init_test_with_editor(cx);
    cx.update(|cx| {
        cx.update_global::<SettingsStore, _>(|store, cx| {
            store.update_user_settings(cx, |settings| {
                settings.project.worktree.file_scan_exclusions = Some(Vec::new());
                settings
                    .project_panel
                    .get_or_insert_default()
                    .auto_reveal_entries = Some(false)
            });
        })
    });

    let fs = FakeFs::new(cx.background_executor.clone());
    fs.insert_tree(
        "/project_root",
        json!({
            ".git": {},
            ".gitignore": "**/gitignored_dir",
            "dir_1": {
                "file_1.py": "# File 1_1 contents",
                "file_2.py": "# File 1_2 contents",
                "file_3.py": "# File 1_3 contents",
                "gitignored_dir": {
                    "file_a.py": "# File contents",
                    "file_b.py": "# File contents",
                    "file_c.py": "# File contents",
                },
            },
            "dir_2": {
                "file_1.py": "# File 2_1 contents",
                "file_2.py": "# File 2_2 contents",
                "file_3.py": "# File 2_3 contents",
            }
        }),
    )
    .await;

    let project = Project::test(fs.clone(), ["/project_root".as_ref()], cx).await;
    let window = cx.add_window(|window, cx| MultiWorkspace::test_new(project.clone(), window, cx));
    let workspace = window
        .read_with(cx, |mw, _| mw.workspace().clone())
        .unwrap();
    let cx = &mut VisualTestContext::from_window(window.into(), cx);
    let panel = workspace.update_in(cx, ProjectPanel::new);
    cx.run_until_parked();

    assert_eq!(
        visible_entries_as_strings(&panel, 0..20, cx),
        &[
            "v project_root",
            "    > .git",
            "    > dir_1",
            "    > dir_2",
            "      .gitignore",
        ]
    );

    let dir_1_file = find_project_entry(&panel, "project_root/dir_1/file_1.py", cx)
        .expect("dir 1 中的文件未被忽略,应有一个条目");
    let dir_2_file = find_project_entry(&panel, "project_root/dir_2/file_1.py", cx)
        .expect("dir 2 中的文件未被忽略,应有一个条目");
    let gitignored_dir_file =
        find_project_entry(&panel, "project_root/dir_1/gitignored_dir/file_a.py", cx);
    assert_eq!(
        gitignored_dir_file, None,
        "在切换目录之前,gitignored 目录中的文件不应有条目"
    );

    toggle_expand_dir(&panel, "project_root/dir_1", cx);
    toggle_expand_dir(&panel, "project_root/dir_1/gitignored_dir", cx);
    cx.run_until_parked();
    assert_eq!(
        visible_entries_as_strings(&panel, 0..20, cx),
        &[
            "v project_root",
            "    > .git",
            "    v dir_1",
            "        v gitignored_dir  <== selected",
            "              file_a.py",
            "              file_b.py",
            "              file_c.py",
            "          file_1.py",
            "          file_2.py",
            "          file_3.py",
            "    > dir_2",
            "      .gitignore",
        ],
        "应在项目面板中显示 gitignored 目录的文件列表"
    );
    let gitignored_dir_file =
        find_project_entry(&panel, "project_root/dir_1/gitignored_dir/file_a.py", cx)
            .expect("gitignored 目录打开后,应存在文件条目");

    toggle_expand_dir(&panel, "project_root/dir_1/gitignored_dir", cx);
    toggle_expand_dir(&panel, "project_root/dir_1", cx);
    assert_eq!(
        visible_entries_as_strings(&panel, 0..20, cx),
        &[
            "v project_root",
            "    > .git",
            "    > dir_1  <== selected",
            "    > dir_2",
            "      .gitignore",
        ],
        "应再次隐藏所有目录内容,并为显式显示测试做准备"
    );

    for file_entry in [dir_1_file, dir_2_file, gitignored_dir_file] {
        panel.update(cx, |panel, cx| {
            panel.project.update(cx, |_, cx| {
                cx.emit(project::Event::ActiveEntryChanged(Some(file_entry)))
            })
        });
        cx.run_until_parked();
        assert_eq!(
            visible_entries_as_strings(&panel, 0..20, cx),
            &[
                "v project_root",
                "    > .git",
                "    > dir_1  <== selected",
                "    > dir_2",
                "      .gitignore",
            ],
            "当未启用自动显示时,选中的条目不应在项目面板中显示"
        );
    }

    panel.update(cx, |panel, cx| {
        panel.project.update(cx, |_, cx| {
            cx.emit(project::Event::RevealInProjectPanel(dir_1_file))
        })
    });
    cx.run_until_parked();
    assert_eq!(
        visible_entries_as_strings(&panel, 0..20, cx),
        &[
            "v project_root",
            "    > .git",
            "    v dir_1",
            "        > gitignored_dir",
            "          file_1.py  <== selected  <== marked",
            "          file_2.py",
            "          file_3.py",
            "    > dir_2",
            "      .gitignore",
        ],
        "在无自动显示的情况下,显式显示应展示项目面板中的 dir_1 条目"
    );

    panel.update(cx, |panel, cx| {
        panel.project.update(cx, |_, cx| {
            cx.emit(project::Event::RevealInProjectPanel(dir_2_file))
        })
    });
    cx.run_until_parked();
    assert_eq!(
        visible_entries_as_strings(&panel, 0..20, cx),
        &[
            "v project_root",
            "    > .git",
            "    v dir_1",
            "        > gitignored_dir",
            "          file_1.py",
            "          file_2.py",
            "          file_3.py",
            "    v dir_2",
            "          file_1.py  <== selected  <== marked",
            "          file_2.py",
            "          file_3.py",
            "      .gitignore",
        ],
        "在无自动显示的情况下,显式显示应展示项目面板中的 dir_2 条目"
    );

    panel.update(cx, |panel, cx| {
        panel.project.update(cx, |_, cx| {
            cx.emit(project::Event::RevealInProjectPanel(gitignored_dir_file))
        })
    });
    cx.run_until_parked();
    assert_eq!(
        visible_entries_as_strings(&panel, 0..20, cx),
        &[
            "v project_root",
            "    > .git",
            "    v dir_1",
            "        v gitignored_dir",
            "              file_a.py  <== selected  <== marked",
            "              file_b.py",
            "              file_c.py",
            "          file_1.py",
            "          file_2.py",
            "          file_3.py",
            "    v dir_2",
            "          file_1.py",
            "          file_2.py",
            "          file_3.py",
            "      .gitignore",
        ],
        "在无自动显示的情况下,显式显示应展示项目面板中被 git 忽略的条目"
    );
}

/// Mirrors real multi-buffer views (`ProjectDiagnosticsEditor`, `ProjectDiff`,
/// etc.): the workspace `Item` is a thin wrapper that holds an inner `Editor`
/// and re-emits its events.
mod multibuffer_wrapper {
    use editor::{Editor, EditorEvent};
    use gpui::{
        App, Context, Entity, EntityId, EventEmitter, FocusHandle, Focusable, IntoElement,
        ParentElement, Render, SharedString, Subscription, Window, div,
    };
    use workspace::item::{Item, ItemEvent, TabContentParams};

    pub struct TestMultibufferWrapper {
        pub editor: Entity<Editor>,
        _subscription: Subscription,
    }

    impl TestMultibufferWrapper {
        pub fn new(editor: Entity<Editor>, cx: &mut Context<Self>) -> Self {
            let _subscription = cx.subscribe(&editor, |_, _, event: &EditorEvent, cx| {
                cx.emit(event.clone());
            });
            Self {
                editor,
                _subscription,
            }
        }
    }

    impl EventEmitter<EditorEvent> for TestMultibufferWrapper {}

    impl Focusable for TestMultibufferWrapper {
        fn focus_handle(&self, cx: &App) -> FocusHandle {
            self.editor.read(cx).focus_handle(cx)
        }
    }

    impl Render for TestMultibufferWrapper {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            div().child(self.editor.clone())
        }
    }

    impl Item for TestMultibufferWrapper {
        type Event = EditorEvent;

        fn tab_content_text(&self, _: usize, _: &App) -> SharedString {
            "wrapper".into()
        }

        fn for_each_project_item(
            &self,
            cx: &App,
            f: &mut dyn FnMut(EntityId, &dyn project::ProjectItem),
        ) {
            self.editor.read(cx).for_each_project_item(cx, f)
        }

        fn active_project_path(&self, cx: &App) -> Option<project::ProjectPath> {
            self.editor.read(cx).active_project_path(cx)
        }

        fn to_item_events(event: &EditorEvent, f: &mut dyn FnMut(ItemEvent)) {
            Editor::to_item_events(event, f)
        }

        fn tab_content(&self, params: TabContentParams, _: &Window, cx: &App) -> gpui::AnyElement {
            ui::Label::new(self.tab_content_text(params.detail.unwrap_or_default(), cx))
                .into_any_element()
        }
    }
}

#[gpui::test]
async fn test_autoreveal_follows_multibuffer_selection(cx: &mut gpui::TestAppContext) {
    use editor::{
        Editor, EditorEvent, EditorMode, MultiBuffer, PathKey, SelectionEffects, ToOffset,
    };
    use language::Point;
    use multibuffer_wrapper::TestMultibufferWrapper;

    init_test_with_editor(cx);
    cx.update(|cx| {
        cx.update_global::<SettingsStore, _>(|store, cx| {
            store.update_user_settings(cx, |settings| {
                settings
                    .project_panel
                    .get_or_insert_default()
                    .auto_reveal_entries = Some(true);
            });
        });
    });

    let fs = FakeFs::new(cx.background_executor.clone());
    fs.insert_tree(
        path!("/project_root"),
        json!({
            "dir_1": { "file_1.py": "alpha 1\nalpha 2\nalpha 3\n" },
            "dir_2": { "file_2.py": "beta 1\nbeta 2\nbeta 3\n" },
        }),
    )
    .await;

    let project = Project::test(fs.clone(), [path!("/project_root").as_ref()], cx).await;
    let window = cx.add_window(|window, cx| MultiWorkspace::test_new(project.clone(), window, cx));
    let workspace = window
        .read_with(cx, |mw, _| mw.workspace().clone())
        .unwrap();
    let cx = &mut VisualTestContext::from_window(window.into(), cx);
    let panel = workspace.update_in(cx, ProjectPanel::new);
    cx.run_until_parked();

    let buffer_1 = project
        .update(cx, |project, cx| {
            let project_path = project
                .find_project_path("project_root/dir_1/file_1.py", cx)
                .unwrap();
            project.open_buffer(project_path, cx)
        })
        .await
        .unwrap();
    let buffer_2 = project
        .update(cx, |project, cx| {
            let project_path = project
                .find_project_path("project_root/dir_2/file_2.py", cx)
                .unwrap();
            project.open_buffer(project_path, cx)
        })
        .await
        .unwrap();

    let multi_buffer = cx.update(|_, cx| {
        cx.new(|cx| {
            let mut multi_buffer = MultiBuffer::new(language::Capability::ReadWrite);
            multi_buffer.set_excerpts_for_path(
                PathKey::sorted(0),
                buffer_1.clone(),
                [Point::new(0, 0)..Point::new(2, 0)],
                0,
                cx,
            );
            multi_buffer.set_excerpts_for_path(
                PathKey::sorted(1),
                buffer_2.clone(),
                [Point::new(0, 0)..Point::new(2, 0)],
                0,
                cx,
            );
            multi_buffer
        })
    });

    let inner_editor = cx.update(|window, cx| {
        cx.new(|cx| {
            Editor::new(
                EditorMode::full(),
                multi_buffer.clone(),
                Some(project.clone()),
                window,
                cx,
            )
        })
    });

    // Wrap the multibuffer editor in an `Item`, mirroring real multibuffer
    // views (`ProjectDiagnosticsEditor`, `ProjectDiff`, etc.). Auto-reveal
    // should follow the inner editor's active buffer.
    workspace.update_in(cx, |workspace, window, cx| {
        let wrapper = cx.new(|cx| TestMultibufferWrapper::new(inner_editor.clone(), cx));
        workspace.add_item_to_active_pane(Box::new(wrapper), None, true, window, cx);
    });
    cx.run_until_parked();

    assert_eq!(
        visible_entries_as_strings(&panel, 0..20, cx),
        &[
            "v project_root",
            "    v dir_1",
            "          file_1.py  <== selected  <== marked",
            "    > dir_2",
        ],
        "当多缓冲区变为活动状态时,应显示其第一个摘录对应的文件"
    );

    let buffer_2_offset = multi_buffer.read_with(cx, |multi_buffer, cx| {
        let snapshot = multi_buffer.snapshot(cx);
        let buffer_2_id = buffer_2.read(cx).remote_id();
        let excerpt = snapshot
            .excerpts_for_buffer(buffer_2_id)
            .next()
            .expect("buffer_2 摘录必须存在");
        snapshot
            .anchor_in_excerpt(excerpt.context.start)
            .expect("摘录锚点必须能够解析")
            .to_offset(&snapshot)
    });

    inner_editor.update_in(cx, |editor, window, cx| {
        editor.change_selections(SelectionEffects::no_scroll(), window, cx, |s| {
            s.select_ranges([buffer_2_offset..buffer_2_offset]);
        });
    });
    cx.run_until_parked();

    assert_eq!(
        visible_entries_as_strings(&panel, 0..20, cx),
        &[
            "v project_root",
            "    v dir_1",
            "          file_1.py",
            "    v dir_2",
            "          file_2.py  <== selected  <== marked",
        ],
        "将光标移动到不同的摘录缓冲区时,应显示该缓冲区对应的条目"
    );

    // Wrappers re-emit inner-editor events through `to_item_events`, so a
    // benign `TitleChanged` (e.g. diagnostic summary updates) ultimately
    // reaches `Workspace::active_item_path_changed`. The active path should be
    // recomputed from the wrapper instead of falling back to a stale selection.
    inner_editor.update(cx, |_, cx| cx.emit(EditorEvent::TitleChanged));
    cx.run_until_parked();

    assert_eq!(
        visible_entries_as_strings(&panel, 0..20, cx),
        &[
            "v project_root",
            "    v dir_1",
            "          file_1.py",
            "    v dir_2",
            "          file_2.py  <== selected  <== marked",
        ],
        "包装器级别的标题更新不得覆盖内部编辑器的显示状态"
    );
}

#[gpui::test]
async fn test_reveal_in_project_panel_fallback(cx: &mut gpui::TestAppContext) {
    init_test_with_editor(cx);
    let fs = FakeFs::new(cx.background_executor.clone());
    fs.insert_tree(
        "/workspace",
        json!({
            "README.md": ""
        }),
    )
    .await;

    let project = Project::test(fs.clone(), ["/workspace".as_ref()], cx).await;
    let window = cx.add_window(|window, cx| MultiWorkspace::test_new(project.clone(), window, cx));
    let workspace = window
        .read_with(cx, |mw, _| mw.workspace().clone())
        .unwrap();
    let cx = &mut VisualTestContext::from_window(window.into(), cx);
    let panel = workspace.update_in(cx, |workspace, window, cx| {
        let panel = ProjectPanel::new(workspace, window, cx);
        workspace.add_panel(panel.clone(), window, cx);
        panel
    });
    cx.run_until_parked();

    // Project panel should still be activated and focused, when using `pane:
    // reveal in project panel` without an active item.
    cx.dispatch_action(workspace::RevealInProjectPanel::default());
    cx.run_until_parked();

    panel.update_in(cx, |panel, window, cx| {
        panel
            .workspace
            .update(cx, |workspace, cx| {
                assert!(
                    workspace.active_item(cx).is_none(),
                    "工作区不应有活动项。"
                );
            })
            .unwrap();

        assert!(
            panel.focus_handle(cx).is_focused(window),
            "即使没有活动项,项目面板也应保持聚焦。"
        );
    });

    // When working with a file that doesn't belong to an open project, we
    // should still activate the project panel on `pane: reveal in project
    // panel`.
    fs.insert_tree(
        "/external",
        json!({
            "file.txt": "外部文件",
        }),
    )
    .await;

    let (worktree, _) = project
        .update(cx, |project, cx| {
            project.find_or_create_worktree("/external/file.txt", false, cx)
        })
        .await
        .unwrap();

    workspace
        .update_in(cx, |workspace, window, cx| {
            let worktree_id = worktree.read(cx).id();
            let path = rel_path("").into();
            let project_path = ProjectPath { worktree_id, path };

            workspace.open_path(project_path, None, true, window, cx)
        })
        .await
        .unwrap();
    cx.run_until_parked();

    panel.update_in(cx, |panel, window, cx| {
        assert!(
            !panel.focus_handle(cx).is_focused(window),
            "打开外部文件后,项目面板不应保持聚焦。"
        );
    });

    cx.dispatch_action(workspace::RevealInProjectPanel::default());
    cx.run_until_parked();

    panel.update_in(cx, |panel, window, cx| {
        panel
            .workspace
            .update(cx, |workspace, cx| {
                assert!(
                    workspace.active_item(cx).is_some(),
                    "工作区应有一个活动项。"
                );
            })
            .unwrap();

        assert!(
            panel.focus_handle(cx).is_focused(window),
            "即使工作树条目不可见,项目面板也应保持聚焦。"
        );
    });

    // Focus again on the center pane so we're sure that the focus doesn't
    // remain on the project panel, otherwise later assertions wouldn't matter.
    panel.update_in(cx, |panel, window, cx| {
        panel
            .workspace
            .update(cx, |workspace, cx| {
                workspace.focus_center_pane(window, cx);
            })
            .log_err();

        assert!(
            !panel.focus_handle(cx).is_focused(window),
            "聚焦中央窗格后,项目面板不应保持聚焦。"
        );
    });

    panel.update_in(cx, |panel, window, cx| {
        assert!(
            !panel.focus_handle(cx).is_focused(window),
            "聚焦中央窗格后,项目面板不应保持聚焦。"
        );
    });

    // Create an unsaved buffer and verify that pane: reveal in project panel`
    // still activates and focuses the panel.
    let pane = workspace.update(cx, |workspace, _| workspace.active_pane().clone());
    pane.update_in(cx, |pane, window, cx| {
        let item = cx.new(|cx| TestItem::new(cx).with_label("未保存的缓冲区"));
        pane.add_item(Box::new(item), false, false, None, window, cx);
    });

    cx.dispatch_action(workspace::RevealInProjectPanel::default());
    cx.run_until_parked();

    panel.update_in(cx, |panel, window, cx| {
        panel
            .workspace
            .update(cx, |workspace, cx| {
                assert!(
                    workspace.active_item(cx).is_some(),
                    "工作区应有一个活动项。"
                );
            })
            .unwrap();

        assert!(
            panel.focus_handle(cx).is_focused(window),
            "即使缓冲区未保存,项目面板也应保持聚焦。"
        );
    });
}

#[gpui::test]
async fn test_creating_excluded_entries(cx: &mut gpui::TestAppContext) {
    init_test(cx);
    cx.update(|cx| {
        cx.update_global::<SettingsStore, _>(|store, cx| {
            store.update_user_settings(cx, |settings| {
                settings.project.worktree.file_scan_exclusions =
                    Some(vec!["excluded_dir".to_string(), "**/.git".to_string()]);
            });
        });
    });

    cx.update(|cx| {
        register_project_item::<TestProjectItemView>(cx);
    });

    let fs = FakeFs::new(cx.executor());
    fs.insert_tree(
        "/root1",
        json!({
            ".dockerignore": "",
            ".git": {
                "HEAD": "",
            },
        }),
    )
    .await;

    let project = Project::test(fs.clone(), ["/root1".as_ref()], cx).await;
    let window = cx.add_window(|window, cx| MultiWorkspace::test_new(project.clone(), window, cx));
    let workspace = window
        .read_with(cx, |mw, _| mw.workspace().clone())
        .unwrap();
    let cx = &mut VisualTestContext::from_window(window.into(), cx);
    let panel = workspace.update_in(cx, |workspace, window, cx| {
        let panel = ProjectPanel::new(workspace, window, cx);
        workspace.add_panel(panel.clone(), window, cx);
        panel
    });
    cx.run_until_parked();

    select_path(&panel, "root1", cx);
    assert_eq!(
        visible_entries_as_strings(&panel, 0..10, cx),
        &["v root1  <== selected", "      .dockerignore",]
    );
    workspace.update_in(cx, |workspace, _, cx| {
        assert!(
            workspace.active_item(cx).is_none(),
            "初始状态下不应有活动项"
        );
    });

    let excluded_file_path = ".git/COMMIT_EDITMSG";
    let excluded_dir_path = "excluded_dir";

    panel.update_in(cx, |panel, window, cx| panel.new_file(&NewFile, window, cx));
    cx.run_until_parked();
    panel.update_in(cx, |panel, window, cx| {
        assert!(panel.filename_editor.read(cx).is_focused(window));
    });
    panel
        .update_in(cx, |panel, window, cx| {
            panel.filename_editor.update(cx, |editor, cx| {
                editor.set_text(excluded_file_path, window, cx)
            });
            panel.confirm_edit(true, window, cx).unwrap()
        })
        .await
        .unwrap();

    assert_eq!(
        visible_entries_as_strings(&panel, 0..13, cx),
        &["v root1", "      .dockerignore"],
        "在排除目录中打开文件后,该目录不应显示"
    );
    panel.update_in(cx, |panel, window, cx| {
        assert!(
            !panel.filename_editor.read(cx).is_focused(window),
            "应已关闭文件名编辑器"
        );
    });
    workspace.update_in(cx, |workspace, _, cx| {
        let active_entry_path = workspace
            .active_item(cx)
            .expect("应已打开并激活被排除的项")
            .act_as::<TestProjectItemView>(cx)
            .expect("应为被排除的项打开对应的项目项")
            .read(cx)
            .path
            .clone();
        assert_eq!(
            active_entry_path.path.as_ref(),
            rel_path(excluded_file_path),
            "应打开被排除的文件"
        );

        assert!(
            workspace.notification_ids().is_empty(),
            "打开被排除的文件后不应有通知"
        );
    });
    assert!(
        fs.is_file(Path::new("/root1/.git/COMMIT_EDITMSG")).await,
        "应已创建被排除的文件"
    );

    select_path(&panel, "root1", cx);
    panel.update_in(cx, |panel, window, cx| {
        panel.new_directory(&NewDirectory, window, cx)
    });
    cx.run_until_parked();
    panel.update_in(cx, |panel, window, cx| {
        assert!(panel.filename_editor.read(cx).is_focused(window));
    });
    panel
        .update_in(cx, |panel, window, cx| {
            panel.filename_editor.update(cx, |editor, cx| {
                editor.set_text(excluded_file_path, window, cx)
            });
            panel.confirm_edit(true, window, cx).unwrap()
        })
        .await
        .unwrap();
    cx.run_until_parked();
    assert_eq!(
        visible_entries_as_strings(&panel, 0..13, cx),
        &["v root1", "      .dockerignore"],
        "尝试创建与被排除文件同名的排除目录后,项目面板不应发生变化"
    );
    panel.update_in(cx, |panel, window, cx| {
        assert!(
            !panel.filename_editor.read(cx).is_focused(window),
            "应已关闭文件名编辑器"
        );
    });
    workspace.update_in(cx, |workspace, _, cx| {
        let notifications = workspace.notification_ids();
        assert_eq!(
            notifications.len(),
            1,
            "应收到一条包含错误消息的通知"
        );
        workspace.dismiss_notification(notifications.first().unwrap(), cx);
        assert!(workspace.notification_ids().is_empty());
    });

    select_path(&panel, "root1", cx);
    panel.update_in(cx, |panel, window, cx| {
        panel.new_directory(&NewDirectory, window, cx)
    });
    cx.run_until_parked();

    panel.update_in(cx, |panel, window, cx| {
        assert!(panel.filename_editor.read(cx).is_focused(window));
    });

    panel
        .update_in(cx, |panel, window, cx| {
            panel.filename_editor.update(cx, |editor, cx| {
                editor.set_text(excluded_dir_path, window, cx)
            });
            panel.confirm_edit(true, window, cx).unwrap()
        })
        .await
        .unwrap();

    cx.run_until_parked();

    assert_eq!(
        visible_entries_as_strings(&panel, 0..13, cx),
        &["v root1", "      .dockerignore"],
        "尝试创建排除目录后,项目面板不应发生变化"
    );
    panel.update_in(cx, |panel, window, cx| {
        assert!(
            !panel.filename_editor.read(cx).is_focused(window),
            "应已关闭文件名编辑器"
        );
    });
    workspace.update_in(cx, |workspace, _, cx| {
        let notifications = workspace.notification_ids();
        assert_eq!(
            notifications.len(),
            1,
            "应收到一条通知,说明实际上没有显示任何目录"
        );
        workspace.dismiss_notification(notifications.first().unwrap(), cx);
        assert!(workspace.notification_ids().is_empty());
    });
    assert!(
        fs.is_dir(Path::new("/root1/excluded_dir")).await,
        "应已创建排除目录"
    );
}

#[gpui::test]
async fn test_selection_restored_when_creation_cancelled(cx: &mut gpui::TestAppContext) {
    init_test_with_editor(cx);

    let fs = FakeFs::new(cx.executor());
    fs.insert_tree(
        "/src",
        json!({
            "test": {
                "first.rs": "// First Rust file",
                "second.rs": "// Second Rust file",
                "third.rs": "// Third Rust file",
            }
        }),
    )
    .await;

    let project = Project::test(fs.clone(), ["/src".as_ref()], cx).await;
    let window = cx.add_window(|window, cx| MultiWorkspace::test_new(project.clone(), window, cx));
    let workspace = window
        .read_with(cx, |mw, _| mw.workspace().clone())
        .unwrap();
    let cx = &mut VisualTestContext::from_window(window.into(), cx);
    let panel = workspace.update_in(cx, |workspace, window, cx| {
        let panel = ProjectPanel::new(workspace, window, cx);
        workspace.add_panel(panel.clone(), window, cx);
        panel
    });
    cx.run_until_parked();

    select_path(&panel, "src", cx);
    panel.update_in(cx, |panel, window, cx| panel.confirm(&Confirm, window, cx));
    cx.executor().run_until_parked();
    assert_eq!(
        visible_entries_as_strings(&panel, 0..10, cx),
        &[
            //
            "v src  <== selected",
            "    > test"
        ]
    );
    panel.update_in(cx, |panel, window, cx| {
        panel.new_directory(&NewDirectory, window, cx)
    });
    cx.executor().run_until_parked();
    panel.update_in(cx, |panel, window, cx| {
        assert!(panel.filename_editor.read(cx).is_focused(window));
    });
    assert_eq!(
        visible_entries_as_strings(&panel, 0..10, cx),
        &[
            //
            "v src",
            "    > [EDITOR: '']  <== selected",
            "    > test"
        ]
    );

    panel.update_in(cx, |panel, window, cx| {
        panel.cancel(&menu::Cancel, window, cx);
    });
    cx.executor().run_until_parked();
    assert_eq!(
        visible_entries_as_strings(&panel, 0..10, cx),
        &[
            //
            "v src  <== selected",
            "    > test"
        ]
    );

    panel.update_in(cx, |panel, window, cx| {
        panel.new_directory(&NewDirectory, window, cx)
    });
    cx.executor().run_until_parked();
    panel.update_in(cx, |panel, window, cx| {
        assert!(panel.filename_editor.read(cx).is_focused(window));
    });
    assert_eq!(
        visible_entries_as_strings(&panel, 0..10, cx),
        &[
            //
            "v src",
            "    > [EDITOR: '']  <== selected",
            "    > test"
        ]
    );
    workspace.update_in(cx, |_, window, _| window.blur());
    cx.executor().run_until_parked();
    assert_eq!(
        visible_entries_as_strings(&panel, 0..10, cx),
        &[
            //
            "v src  <== selected",
            "    > test"
        ]
    );
}

#[gpui::test]
async fn test_basic_file_deletion_scenarios(cx: &mut gpui::TestAppContext) {
    init_test_with_editor(cx);

    let fs = FakeFs::new(cx.executor());
    fs.insert_tree(
        "/root",
        json!({
            "dir1": {
                "subdir1": {},
                "file1.txt": "",
                "file2.txt": "",
            },
            "dir2": {
                "subdir2": {},
                "file3.txt": "",
                "file4.txt": "",
            },
            "file5.txt": "",
            "file6.txt": "",
        }),
    )
    .await;

    let project = Project::test(fs.clone(), ["/root".as_ref()], cx).await;
    let window = cx.add_window(|window, cx| MultiWorkspace::test_new(project.clone(), window, cx));
    let workspace = window
        .read_with(cx, |mw, _| mw.workspace().clone())
        .unwrap();
    let cx = &mut VisualTestContext::from_window(window.into(), cx);
    let panel = workspace.update_in(cx, ProjectPanel::new);
    cx.run_until_parked();

    toggle_expand_dir(&panel, "root/dir1", cx);
    toggle_expand_dir(&panel, "root/dir2", cx);

    // Test Case 1: Delete middle file in directory
    select_path(&panel, "root/dir1/file1.txt", cx);
    assert_eq!(
        visible_entries_as_strings(&panel, 0..15, cx),
        &[
            "v root",
            "    v dir1",
            "        > subdir1",
            "          file1.txt  <== selected",
            "          file2.txt",
            "    v dir2",
            "        > subdir2",
            "          file3.txt",
            "          file4.txt",
            "      file5.txt",
            "      file6.txt",
        ],
        "删除中间文件前的初始状态"
    );

    submit_deletion(&panel, cx);
    assert_eq!(
        visible_entries_as_strings(&panel, 0..15, cx),
        &[
            "v root",
            "    v dir1",
            "        > subdir1",
            "          file2.txt  <== selected",
            "    v dir2",
            "        > subdir2",
            "          file3.txt",
            "          file4.txt",
            "      file5.txt",
            "      file6.txt",
        ],
        "删除中间文件后应选择下一个文件"
    );

    // Test Case 2: Delete last file in directory
    submit_deletion(&panel, cx);
    assert_eq!(
        visible_entries_as_strings(&panel, 0..15, cx),
        &[
            "v root",
            "    v dir1",
            "        > subdir1  <== selected",
            "    v dir2",
            "        > subdir2",
            "          file3.txt",
            "          file4.txt",
            "      file5.txt",
            "      file6.txt",
        ],
        "删除最后一个文件后应选择下一个目录"
    );

    // Test Case 3: Delete root level file
    select_path(&panel, "root/file6.txt", cx);
    assert_eq!(
        visible_entries_as_strings(&panel, 0..15, cx),
        &[
            "v root",
            "    v dir1",
            "        > subdir1",
            "    v dir2",
            "        > subdir2",
            "          file3.txt",
            "          file4.txt",
            "      file5.txt",
            "      file6.txt  <== selected",
        ],
        "删除根级文件前的初始状态"
    );

    submit_deletion(&panel, cx);
    assert_eq!(
        visible_entries_as_strings(&panel, 0..15, cx),
        &[
            "v root",
            "    v dir1",
            "        > subdir1",
            "    v dir2",
            "        > subdir2",
            "          file3.txt",
            "          file4.txt",
            "      file5.txt  <== selected",
        ],
        "在根级别应选择上一个条目"
    );
}

#[gpui::test]
async fn test_deletion_gitignored(cx: &mut gpui::TestAppContext) {
    init_test_with_editor(cx);

    let fs = FakeFs::new(cx.executor());
    fs.insert_tree(
        path!("/root"),
        json!({
            "aa": "// Testing 1",
            "bb": "// Testing 2",
            "cc": "// Testing 3",
            "dd": "// Testing 4",
            "ee": "// Testing 5",
            "ff": "// Testing 6",
            "gg": "// Testing 7",
            "hh": "// Testing 8",
            "ii": "// Testing 8",
            ".gitignore": "bb\ndd\nee\nff\nii\n'",
        }),
    )
    .await;

    let project = Project::test(fs.clone(), [path!("/root").as_ref()], cx).await;
    let window = cx.add_window(|window, cx| MultiWorkspace::test_new(project.clone(), window, cx));
    let workspace = window
        .read_with(cx, |mw, _| mw.workspace().clone())
        .unwrap();
    let cx = &mut VisualTestContext::from_window(window.into(), cx);

    // Test 1: Auto selection with one gitignored file next to the deleted file
    cx.update(|_, cx| {
        let settings = *ProjectPanelSettings::get_global(cx);
        ProjectPanelSettings::override_global(
            ProjectPanelSettings {
                hide_gitignore: true,
                ..settings
            },
            cx,
        );
    });

    let panel = workspace.update_in(cx, ProjectPanel::new);
    cx.run_until_parked();

    select_path(&panel, "root/aa", cx);
    assert_eq!(
        visible_entries_as_strings(&panel, 0..10, cx),
        &[
            "v root",
            "      .gitignore",
            "      aa  <== selected",
            "      cc",
            "      gg",
            "      hh"
        ],
        "初始状态应隐藏 .gitignore 中列出的文件"
    );

    submit_deletion(&panel, cx);

    assert_eq!(
        visible_entries_as_strings(&panel, 0..10, cx),
        &[
            "v root",
            "      .gitignore",
            "      cc  <== selected",
            "      gg",
            "      hh"
        ],
        "应选择下一个不在 .gitignore 中的条目"
    );

    // Test 2: Auto selection with many gitignored files next to the deleted file
    submit_deletion(&panel, cx);
    assert_eq!(
        visible_entries_as_strings(&panel, 0..10, cx),
        &[
            "v root",
            "      .gitignore",
            "      gg  <== selected",
            "      hh"
        ],
        "应选择下一个不在 .gitignore 中的条目"
    );

    // Test 3: Auto selection of entry before deleted file
    select_path(&panel, "root/hh", cx);
    assert_eq!(
        visible_entries_as_strings(&panel, 0..10, cx),
        &[
            "v root",
            "      .gitignore",
            "      gg",
            "      hh  <== selected"
        ],
        "应选择下一个不在 .gitignore 中的条目"
    );
    submit_deletion(&panel, cx);
    assert_eq!(
        visible_entries_as_strings(&panel, 0..10, cx),
        &["v root", "      .gitignore", "      gg  <== selected"],
        "应选择下一个不在 .gitignore 中的条目"
    );
}

#[gpui::test]
async fn test_nested_deletion_gitignore(cx: &mut gpui::TestAppContext) {
    init_test_with_editor(cx);

    let fs = FakeFs::new(cx.executor());
    fs.insert_tree(
        path!("/root"),
        json!({
            "dir1": {
                "file1": "// Testing",
                "file2": "// Testing",
                "file3": "// Testing"
            },
            "aa": "// Testing",
            ".gitignore": "file1\nfile3\n",
        }),
    )
    .await;

    let project = Project::test(fs.clone(), [path!("/root").as_ref()], cx).await;
    let window = cx.add_window(|window, cx| MultiWorkspace::test_new(project.clone(), window, cx));
    let workspace = window
        .read_with(cx, |mw, _| mw.workspace().clone())
        .unwrap();
    let cx = &mut VisualTestContext::from_window(window.into(), cx);

    cx.update(|_, cx| {
        let settings = *ProjectPanelSettings::get_global(cx);
        ProjectPanelSettings::override_global(
            ProjectPanelSettings {
                hide_gitignore: true,
                ..settings
            },
            cx,
        );
    });

    let panel = workspace.update_in(cx, ProjectPanel::new);
    cx.run_until_parked();

    // Test 1: Visible items should exclude files on gitignore
    toggle_expand_dir(&panel, "root/dir1", cx);
    select_path(&panel, "root/dir1/file2", cx);
    assert_eq!(
        visible_entries_as_strings(&panel, 0..10, cx),
        &[
            "v root",
            "    v dir1",
            "          file2  <== selected",
            "      .gitignore",
            "      aa"
        ],
        "初始状态应隐藏 .gitignore 中列出的文件"
    );
    submit_deletion(&panel, cx);

    // Test 2: Auto selection should go to the parent
    assert_eq!(
        visible_entries_as_strings(&panel, 0..10, cx),
        &[
            "v root",
            "    v dir1  <== selected",
            "      .gitignore",
            "      aa"
        ],
        "初始状态应隐藏 .gitignore 中列出的文件"
    );
}

#[gpui::test]
async fn test_complex_selection_scenarios(cx: &mut gpui::TestAppContext) {
    init_test_with_editor(cx);

    let fs = FakeFs::new(cx.executor());
    fs.insert_tree(
        "/root",
        json!({
            "dir1": {
                "subdir1": {
                    "a.txt": "",
                    "b.txt": ""
                },
                "file1.txt": "",
            },
            "dir2": {
                "subdir2": {
                    "c.txt": "",
                    "d.txt": ""
                },
                "file2.txt": "",
            },
            "file3.txt": "",
        }),
    )
    .await;

    let project = Project::test(fs.clone(), ["/root".as_ref()], cx).await;
    let window = cx.add_window(|window, cx| MultiWorkspace::test_new(project.clone(), window, cx));
    let workspace = window
        .read_with(cx, |mw, _| mw.workspace().clone())
        .unwrap();
    let cx = &mut VisualTestContext::from_window(window.into(), cx);
    let panel = workspace.update_in(cx, ProjectPanel::new);
    cx.run_until_parked();

    toggle_expand_dir(&panel, "root/dir1", cx);
    toggle_expand_dir(&panel, "root/dir1/subdir1", cx);
    toggle_expand_dir(&panel, "root/dir2", cx);
    toggle_expand_dir(&panel, "root/dir2/subdir2", cx);

    // Test Case 1: Select and delete nested directory with parent
    cx.simulate_modifiers_change(gpui::Modifiers {
        control: true,
        ..Default::default()
    });
    select_path_with_mark(&panel, "root/dir1/subdir1", cx);
    select_path_with_mark(&panel, "root/dir1", cx);

    assert_eq!(
        visible_entries_as_strings(&panel, 0..15, cx),
        &[
            "v root",
            "    v dir1  <== selected  <== marked",
            "        v subdir1  <== marked",
            "              a.txt",
            "              b.txt",
            "          file1.txt",
            "    v dir2",
            "        v subdir2",
            "              c.txt",
            "              d.txt",
            "          file2.txt",
            "      file3.txt",
        ],
        "删除带有父目录的嵌套目录前的初始状态"
    );

    submit_deletion(&panel, cx);
    assert_eq!(
        visible_entries_as_strings(&panel, 0..15, cx),
        &[
            "v root",
            "    v dir2  <== selected",
            "        v subdir2",
            "              c.txt",
            "              d.txt",
            "          file2.txt",
            "      file3.txt",
        ],
        "删除带有父目录的目录后应选择下一个目录"
    );

    // Test Case 2: Select mixed files and directories across levels
    select_path_with_mark(&panel, "root/dir2/subdir2/c.txt", cx);
    select_path_with_mark(&panel, "root/dir2/file2.txt", cx);
    select_path_with_mark(&panel, "root/file3.txt", cx);

    assert_eq!(
        visible_entries_as_strings(&panel, 0..15, cx),
        &[
            "v root",
            "    v dir2",
            "        v subdir2",
            "              c.txt  <== marked",
            "              d.txt",
            "          file2.txt  <== marked",
            "      file3.txt  <== selected  <== marked",
        ],
        "删除前的初始状态"
    );

    submit_deletion(&panel, cx);
    assert_eq!(
        visible_entries_as_strings(&panel, 0..15, cx),
        &[
            "v root",
            "    v dir2  <== selected",
            "        v subdir2",
            "              d.txt",
        ],
        "应选择同级目录"
    );
}

#[gpui::test]
async fn test_delete_all_files_and_directories(cx: &mut gpui::TestAppContext) {
    init_test_with_editor(cx);

    let fs = FakeFs::new(cx.executor());
    fs.insert_tree(
        "/root",
        json!({
            "dir1": {
                "subdir1": {
                    "a.txt": "",
                    "b.txt": ""
                },
                "file1.txt": "",
            },
            "dir2": {
                "subdir2": {
                    "c.txt": "",
                    "d.txt": ""
                },
                "file2.txt": "",
            },
            "file3.txt": "",
            "file4.txt": "",
        }),
    )
    .await;

    let project = Project::test(fs.clone(), ["/root".as_ref()], cx).await;
    let window = cx.add_window(|window, cx| MultiWorkspace::test_new(project.clone(), window, cx));
    let workspace = window
        .read_with(cx, |mw, _| mw.workspace().clone())
        .unwrap();
    let cx = &mut VisualTestContext::from_window(window.into(), cx);
    let panel = workspace.update_in(cx, ProjectPanel::new);
    cx.run_until_parked();

    toggle_expand_dir(&panel, "root/dir1", cx);
    toggle_expand_dir(&panel, "root/dir1/subdir1", cx);
    toggle_expand_dir(&panel, "root/dir2", cx);
    toggle_expand_dir(&panel, "root/dir2/subdir2", cx);

    // Test Case 1: Select all root files and directories
    cx.simulate_modifiers_change(gpui::Modifiers {
        control: true,
        ..Default::default()
    });
    select_path_with_mark(&panel, "root/dir1", cx);
    select_path_with_mark(&panel, "root/dir2", cx);
    select_path_with_mark(&panel, "root/file3.txt", cx);
    select_path_with_mark(&panel, "root/file4.txt", cx);
    assert_eq!(
        visible_entries_as_strings(&panel, 0..20, cx),
        &[
            "v root",
            "    v dir1  <== marked",
            "        v subdir1",
            "              a.txt",
            "              b.txt",
            "          file1.txt",
            "    v dir2  <== marked",
            "        v subdir2",
            "              c.txt",
            "              d.txt",
            "          file2.txt",
            "      file3.txt  <== marked",
            "      file4.txt  <== selected  <== marked",
        ],
        "删除所有内容前的状态"
    );

    submit_deletion(&panel, cx);
    assert_eq!(
        visible_entries_as_strings(&panel, 0..20, cx),
        &["v root  <== selected"],
        "删除所有内容后,应仅保留空的根目录"
    );
}

#[gpui::test]
async fn test_nested_selection_deletion(cx: &mut gpui::TestAppContext) {
    init_test_with_editor(cx);

    let fs = FakeFs::new(cx.executor());
    fs.insert_tree(
        "/root",
        json!({
            "dir1": {
                "subdir1": {
                    "file_a.txt": "content a",
                    "file_b.txt": "content b",
                },
                "subdir2": {
                    "file_c.txt": "content c",
                },
                "file1.txt": "content 1",
            },
            "dir2": {
                "file2.txt": "content 2",
            },
        }),
    )
    .await;

    let project = Project::test(fs.clone(), ["/root".as_ref()], cx).await;
    let window = cx.add_window(|window, cx| MultiWorkspace::test_new(project.clone(), window, cx));
    let workspace = window
        .read_with(cx, |mw, _| mw.workspace().clone())
        .unwrap();
    let cx = &mut VisualTestContext::from_window(window.into(), cx);
    let panel = workspace.update_in(cx, ProjectPanel::new);
    cx.run_until_parked();

    toggle_expand_dir(&panel, "root/dir1", cx);
    toggle_expand_dir(&panel, "root/dir1/subdir1", cx);
    toggle_expand_dir(&panel, "root/dir2", cx);
    cx.simulate_modifiers_change(gpui::Modifiers {
        control: true,
        ..Default::default()
    });

    // Test Case 1: Select parent directory, subdirectory, and a file inside the subdirectory
    select_path_with_mark(&panel, "root/dir1", cx);
    select_path_with_mark(&panel, "root/dir1/subdir1", cx);
    select_path_with_mark(&panel, "root/dir1/subdir1/file_a.txt", cx);

    assert_eq!(
        visible_entries_as_strings(&panel, 0..20, cx),
        &[
            "v root",
            "    v dir1  <== marked",
            "        v subdir1  <== marked",
            "              file_a.txt  <== selected  <== marked",
            "              file_b.txt",
            "        > subdir2",
            "          file1.txt",
            "    v dir2",
            "          file2.txt",
        ],
        "选中父目录、子目录和文件的状态"
    );
    submit_deletion(&panel, cx);
    assert_eq!(
        visible_entries_as_strings(&panel, 0..20, cx),
        &["v root", "    v dir2  <== selected", "          file2.txt",],
        "删除后应仅保留 dir2"
    );
}

#[gpui::test]
async fn test_multiple_worktrees_deletion(cx: &mut gpui::TestAppContext) {
    init_test_with_editor(cx);

    let fs = FakeFs::new(cx.executor());
    // First worktree
    fs.insert_tree(
        "/root1",
        json!({
            "dir1": {
                "file1.txt": "content 1",
                "file2.txt": "content 2",
            },
            "dir2": {
                "file3.txt": "content 3",
            },
        }),
    )
    .await;

    // Second worktree
    fs.insert_tree(
        "/root2",
        json!({
            "dir3": {
                "file4.txt": "content 4",
                "file5.txt": "content 5",
            },
            "file6.txt": "content 6",
        }),
    )
    .await;

    let project = Project::test(fs.clone(), ["/root1".as_ref(), "/root2".as_ref()], cx).await;
    let window = cx.add_window(|window, cx| MultiWorkspace::test_new(project.clone(), window, cx));
    let workspace = window
        .read_with(cx, |mw, _| mw.workspace().clone())
        .unwrap();
    let cx = &mut VisualTestContext::from_window(window.into(), cx);
    let panel = workspace.update_in(cx, ProjectPanel::new);
    cx.run_until_parked();

    // Expand all directories for testing
    toggle_expand_dir(&panel, "root1/dir1", cx);
    toggle_expand_dir(&panel, "root1/dir2", cx);
    toggle_expand_dir(&panel, "root2/dir3", cx);

    // Test Case 1: Delete files across different worktrees
    cx.simulate_modifiers_change(gpui::Modifiers {
        control: true,
        ..Default::default()
    });
    select_path_with_mark(&panel, "root1/dir1/file1.txt", cx);
    select_path_with_mark(&panel, "root2/dir3/file4.txt", cx);

    assert_eq!(
        visible_entries_as_strings(&panel, 0..20, cx),
        &[
            "v root1",
            "    v dir1",
            "          file1.txt  <== marked",
            "          file2.txt",
            "    v dir2",
            "          file3.txt",
            "v root2",
            "    v dir3",
            "          file4.txt  <== selected  <== marked",
            "          file5.txt",
            "      file6.txt",
        ],
        "从不同工作树中选择文件的初始状态"
    );

    submit_deletion(&panel, cx);
    assert_eq!(
        visible_entries_as_strings(&panel, 0..20, cx),
        &[
            "v root1",
            "    v dir1",
            "          file2.txt",
            "    v dir2",
            "          file3.txt",
            "v root2",
            "    v dir3",
            "          file5.txt  <== selected",
            "      file6.txt",
        ],
        "删除后应选择最后一个工作树中的下一个文件"
    );

    // Test Case 2: Delete directories from different worktrees
    select_path_with_mark(&panel, "root1/dir1", cx);
    select_path_with_mark(&panel, "root2/dir3", cx);

    assert_eq!(
        visible_entries_as_strings(&panel, 0..20, cx),
        &[
            "v root1",
            "    v dir1  <== marked",
            "          file2.txt",
            "    v dir2",
            "          file3.txt",
            "v root2",
            "    v dir3  <== selected  <== marked",
            "          file5.txt",
            "      file6.txt",
        ],
        "从不同工作树标记目录的状态"
    );

    submit_deletion(&panel, cx);
    assert_eq!(
        visible_entries_as_strings(&panel, 0..20, cx),
        &[
            "v root1",
            "    v dir2",
            "          file3.txt",
            "v root2",
            "      file6.txt  <== selected",
        ],
        "删除目录后应选择最后一个工作树中剩余的文件"
    );

    // Test Case 4: Delete all remaining files except roots
    select_path_with_mark(&panel, "root1/dir2/file3.txt", cx);
    select_path_with_mark(&panel, "root2/file6.txt", cx);

    assert_eq!(
        visible_entries_as_strings(&panel, 0..20, cx),
        &[
            "v root1",
            "    v dir2",
            "          file3.txt  <== marked",
            "v root2",
            "      file6.txt  <== selected  <== marked",
        ],
        "所有剩余文件都被标记的状态"
    );

    submit_deletion(&panel, cx);
    assert_eq!(
        visible_entries_as_strings(&panel, 0..20, cx),
        &["v root1", "    v dir2", "v root2  <== selected"],
        "删除后应选择第二个父根目录"
    );
}

#[gpui::test]
async fn test_selection_vs_marked_entries_priority(cx: &mut gpui::TestAppContext) {
    init_test_with_editor(cx);

    let fs = FakeFs::new(cx.executor());
    fs.insert_tree(
        "/root",
        json!({
            "dir1": {
                "file1.txt": "",
                "file2.txt": "",
                "file3.txt": "",
            },
            "dir2": {
                "file4.txt": "",
                "file5.txt": "",
            },
        }),
    )
    .await;

    let project = Project::test(fs.clone(), ["/root".as_ref()], cx).await;
    let window = cx.add_window(|window, cx| MultiWorkspace::test_new(project.clone(), window, cx));
    let workspace = window
        .read_with(cx, |mw, _| mw.workspace().clone())
        .unwrap();
    let cx = &mut VisualTestContext::from_window(window.into(), cx);
    let panel = workspace.update_in(cx, ProjectPanel::new);
    cx.run_until_parked();

    toggle_expand_dir(&panel, "root/dir1", cx);
    toggle_expand_dir(&panel, "root/dir2", cx);

    cx.simulate_modifiers_change(gpui::Modifiers {
        control: true,
        ..Default::default()
    });

    select_path_with_mark(&panel, "root/dir1/file2.txt", cx);
    select_path(&panel, "root/dir1/file1.txt", cx);

    assert_eq!(
        visible_entries_as_strings(&panel, 0..15, cx),
        &[
            "v root",
            "    v dir1",
            "          file1.txt  <== selected",
            "          file2.txt  <== marked",
            "          file3.txt",
            "    v dir2",
            "          file4.txt",
            "          file5.txt",
        ],
        "有一个标记条目和不同选择项的初始状态"
    );

    // Delete should operate on the selected entry (file1.txt)
    submit_deletion(&panel, cx);
    assert_eq!(
        visible_entries_as_strings(&panel, 0..15, cx),
        &[
            "v root",
            "    v dir1",
            "          file2.txt  <== selected  <== marked",
            "          file3.txt",
            "    v dir2",
            "          file4.txt",
            "          file5.txt",
        ],
        "应删除选中的文件,而不是标记的文件"
    );

    select_path_with_mark(&panel, "root/dir1/file3.txt", cx);
    select_path_with_mark(&panel, "root/dir2/file4.txt", cx);
    select_path(&panel, "root/dir2/file5.txt", cx);

    assert_eq!(
        visible_entries_as_strings(&panel, 0..15, cx),
        &[
            "v root",
            "    v dir1",
            "          file2.txt  <== marked",
            "          file3.txt  <== marked",
            "    v dir2",
            "          file4.txt  <== marked",
            "          file5.txt  <== selected",
        ],
        "有多个标记条目和不同选择项的初始状态"
    );

    // Delete should operate on all marked entries, ignoring the selection
    submit_deletion(&panel, cx);
    assert_eq!(
        visible_entries_as_strings(&panel, 0..15, cx),
        &[
            "v root",
            "    v dir1",
            "    v dir2",
            "          file5.txt  <== selected",
        ],
        "应删除所有标记的文件,仅保留选中的文件"
    );
}

#[gpui::test]
async fn test_selection_fallback_to_next_highest_worktree(cx: &mut gpui::TestAppContext) {
    init_test_with_editor(cx);

    let fs = FakeFs::new(cx.executor());
    fs.insert_tree(
        "/root_b",
        json!({
            "dir1": {
                "file1.txt": "content 1",
                "file2.txt": "content 2",
            },
        }),
    )
    .await;

    fs.insert_tree(
        "/root_c",
        json!({
            "dir2": {},
        }),
    )
    .await;

    let project = Project::test(fs.clone(), ["/root_b".as_ref(), "/root_c".as_ref()], cx).await;
    let window = cx.add_window(|window, cx| MultiWorkspace::test_new(project.clone(), window, cx));
    let workspace = window
        .read_with(cx, |mw, _| mw.workspace().clone())
        .unwrap();
    let cx = &mut VisualTestContext::from_window(window.into(), cx);
    let panel = workspace.update_in(cx, ProjectPanel::new);
    cx.run_until_parked();

    toggle_expand_dir(&panel, "root_b/dir1", cx);
    toggle_expand_dir(&panel, "root_c/dir2", cx);

    cx.simulate_modifiers_change(gpui::Modifiers {
        control: true,
        ..Default::default()
    });
    select_path_with_mark(&panel, "root_b/dir1/file1.txt", cx);
    select_path_with_mark(&panel, "root_b/dir1/file2.txt", cx);

    assert_eq!(
        visible_entries_as_strings(&panel, 0..20, cx),
        &[
            "v root_b",
            "    v dir1",
            "          file1.txt  <== marked",
            "          file2.txt  <== selected  <== marked",
            "v root_c",
            "    v dir2",
        ],
        "在 root_b 中标记文件的初始状态"
    );

    submit_deletion(&panel, cx);
    assert_eq!(
        visible_entries_as_strings(&panel, 0..20, cx),
        &[
            "v root_b",
            "    v dir1  <== selected",
            "v root_c",
            "    v dir2",
        ],
        "在 root_b 中删除后,由于这是最后一次删除,选择项应位于 root_b 中"
    );

    select_path_with_mark(&panel, "root_c/dir2", cx);

    submit_deletion(&panel, cx);
    assert_eq!(
        visible_entries_as_strings(&panel, 0..20, cx),
        &["v root_b", "    v dir1", "v root_c  <== selected",],
        "从 root_c 中删除后,选择项应保持在 root_c 中"
    );
}

pub(crate) fn toggle_expand_dir(
    panel: &Entity<ProjectPanel>,
    path: &str,
    cx: &mut VisualTestContext,
) {
    let path = rel_path(path);
    panel.update_in(cx, |panel, window, cx| {
        for worktree in panel.project.read(cx).worktrees(cx).collect::<Vec<_>>() {
            let worktree = worktree.read(cx);
            if let Ok(relative_path) = path.strip_prefix(worktree.root_name()) {
                let entry_id = worktree.entry_for_path(relative_path).unwrap().id;
                panel.toggle_expanded(entry_id, window, cx);
                return;
            }
        }
        panic!("路径 {:?} 没有对应的工作树", path);
    });
    cx.run_until_parked();
}

#[gpui::test]
async fn test_expand_all_for_entry(cx: &mut gpui::TestAppContext) {
    init_test_with_editor(cx);

    let fs = FakeFs::new(cx.executor());
    fs.insert_tree(
        path!("/root"),
        json!({
            ".gitignore": "**/ignored_dir\n**/ignored_nested",
            "dir1": {
                "empty1": {
                    "empty2": {
                        "empty3": {
                            "file.txt": ""
                        }
                    }
                },
                "subdir1": {
                    "file1.txt": "",
                    "file2.txt": "",
                    "ignored_nested": {
                        "ignored_file.txt": ""
                    }
                },
                "ignored_dir": {
                    "subdir": {
                        "deep_file.txt": ""
                    }
                }
            }
        }),
    )
    .await;

    let project = Project::test(fs.clone(), [path!("/root").as_ref()], cx).await;
    let window = cx.add_window(|window, cx| MultiWorkspace::test_new(project.clone(), window, cx));
    let workspace = window
        .read_with(cx, |mw, _| mw.workspace().clone())
        .unwrap();
    let cx = &mut VisualTestContext::from_window(window.into(), cx);

    // Test 1: When auto-fold is enabled
    cx.update(|_, cx| {
        let settings = *ProjectPanelSettings::get_global(cx);
        ProjectPanelSettings::override_global(
            ProjectPanelSettings {
                auto_fold_dirs: true,
                ..settings
            },
            cx,
        );
    });

    let panel = workspace.update_in(cx, ProjectPanel::new);
    cx.run_until_parked();

    assert_eq!(
        visible_entries_as_strings(&panel, 0..20, cx),
        &["v root", "    > dir1", "      .gitignore",],
        "初始状态应显示折叠的根目录结构"
    );

    toggle_expand_dir(&panel, "root/dir1", cx);
    assert_eq!(
        visible_entries_as_strings(&panel, 0..20, cx),
        &[
            "v root",
            "    v dir1  <== selected",
            "        > empty1/empty2/empty3",
            "        > ignored_dir",
            "        > subdir1",
            "      .gitignore",
        ],
        "应显示第一级,包含自动折叠的目录和可见的忽略目录"
    );

    let entry_id = find_project_entry(&panel, "root/dir1", cx).unwrap();
    panel.update_in(cx, |panel, window, cx| {
        let project = panel.project.read(cx);
        let worktree = project.worktrees(cx).next().unwrap().read(cx);
        panel.expand_all_for_entry(worktree.id(), entry_id, cx);
        panel.update_visible_entries(None, false, false, window, cx);
    });
    cx.run_until_parked();

    assert_eq!(
        visible_entries_as_strings(&panel, 0..20, cx),
        &[
            "v root",
            "    v dir1  <== selected",
            "        v empty1",
            "            v empty2",
            "                v empty3",
            "                      file.txt",
            "        > ignored_dir",
            "        v subdir1",
            "            > ignored_nested",
            "              file1.txt",
            "              file2.txt",
            "      .gitignore",
        ],
        "使用自动折叠执行全部展开后:不应展开 ignored_dir,应展开折叠的目录,且不应展开 ignored_nested"
    );

    // Test 2: When auto-fold is disabled
    cx.update(|_, cx| {
        let settings = *ProjectPanelSettings::get_global(cx);
        ProjectPanelSettings::override_global(
            ProjectPanelSettings {
                auto_fold_dirs: false,
                ..settings
            },
            cx,
        );
    });

    panel.update_in(cx, |panel, window, cx| {
        panel.collapse_all_entries(&CollapseAllEntries, window, cx);
    });

    toggle_expand_dir(&panel, "root/dir1", cx);
    assert_eq!(
        visible_entries_as_strings(&panel, 0..20, cx),
        &[
            "v root",
            "    v dir1  <== selected",
            "        > empty1",
            "        > ignored_dir",
            "        > subdir1",
            "      .gitignore",
        ],
        "禁用自动折叠时:应分别显示所有目录"
    );

    let entry_id = find_project_entry(&panel, "root/dir1", cx).unwrap();
    panel.update_in(cx, |panel, window, cx| {
        let project = panel.project.read(cx);
        let worktree = project.worktrees(cx).next().unwrap().read(cx);
        panel.expand_all_for_entry(worktree.id(), entry_id, cx);
        panel.update_visible_entries(None, false, false, window, cx);
    });
    cx.run_until_parked();

    assert_eq!(
        visible_entries_as_strings(&panel, 0..20, cx),
        &[
            "v root",
            "    v dir1  <== selected",
            "        v empty1",
            "            v empty2",
            "                v empty3",
            "                      file.txt",
            "        > ignored_dir",
            "        v subdir1",
            "            > ignored_nested",
            "              file1.txt",
            "              file2.txt",
            "      .gitignore",
        ],
        "After expand_all without auto-fold: should expand all dirs normally, \
         expand ignored_dir itself but not its subdirs, and not expand ignored_nested"
    );

    // Test 3: When explicitly called on ignored directory
    let ignored_dir_entry = find_project_entry(&panel, "root/dir1/ignored_dir", cx).unwrap();
    panel.update_in(cx, |panel, window, cx| {
        let project = panel.project.read(cx);
        let worktree = project.worktrees(cx).next().unwrap().read(cx);
        panel.expand_all_for_entry(worktree.id(), ignored_dir_entry, cx);
        panel.update_visible_entries(None, false, false, window, cx);
    });
    cx.run_until_parked();

    assert_eq!(
        visible_entries_as_strings(&panel, 0..20, cx),
        &[
            "v root",
            "    v dir1  <== selected",
            "        v empty1",
            "            v empty2",
            "                v empty3",
            "                      file.txt",
            "        v ignored_dir",
            "            v subdir",
            "                  deep_file.txt",
            "        v subdir1",
            "            > ignored_nested",
            "              file1.txt",
            "              file2.txt",
            "      .gitignore",
        ],
        "对 ignored_dir 执行全部展开后:应展开该忽略目录的所有内容"
    );
}

#[gpui::test]
async fn test_collapse_all_for_entry(cx: &mut gpui::TestAppContext) {
    init_test(cx);

    let fs = FakeFs::new(cx.executor());
    fs.insert_tree(
        path!("/root"),
        json!({
            "dir1": {
                "subdir1": {
                    "nested1": {
                        "file1.txt": "",
                        "file2.txt": ""
                    },
                },
                "subdir2": {
                    "file4.txt": ""
                }
            },
            "dir2": {
                "single_file": {
                    "file5.txt": ""
                }
            }
        }),
    )
    .await;

    let project = Project::test(fs.clone(), [path!("/root").as_ref()], cx).await;
    let window = cx.add_window(|window, cx| MultiWorkspace::test_new(project.clone(), window, cx));
    let workspace = window
        .read_with(cx, |mw, _| mw.workspace().clone())
        .unwrap();
    let cx = &mut VisualTestContext::from_window(window.into(), cx);

    // Test 1: Basic collapsing
    {
        let panel = workspace.update_in(cx, ProjectPanel::new);
        cx.run_until_parked();

        toggle_expand_dir(&panel, "root/dir1", cx);
        toggle_expand_dir(&panel, "root/dir1/subdir1", cx);
        toggle_expand_dir(&panel, "root/dir1/subdir1/nested1", cx);
        toggle_expand_dir(&panel, "root/dir1/subdir2", cx);

        assert_eq!(
            visible_entries_as_strings(&panel, 0..20, cx),
            &[
                "v root",
                "    v dir1",
                "        v subdir1",
                "            v nested1",
                "                  file1.txt",
                "                  file2.txt",
                "        v subdir2  <== selected",
                "              file4.txt",
                "    > dir2",
            ],
            "所有内容都已展开的初始状态"
        );

        let entry_id = find_project_entry(&panel, "root/dir1", cx).unwrap();
        panel.update_in(cx, |panel, window, cx| {
            let project = panel.project.read(cx);
            let worktree = project.worktrees(cx).next().unwrap().read(cx);
            panel.collapse_all_for_entry(worktree.id(), entry_id, cx);
            panel.update_visible_entries(None, false, false, window, cx);
        });
        cx.run_until_parked();

        assert_eq!(
            visible_entries_as_strings(&panel, 0..20, cx),
            &["v root", "    > dir1", "    > dir2",],
            "dir1 下的所有子目录都应折叠"
        );
    }

    // Test 2: With auto-fold enabled
    {
        cx.update(|_, cx| {
            let settings = *ProjectPanelSettings::get_global(cx);
            ProjectPanelSettings::override_global(
                ProjectPanelSettings {
                    auto_fold_dirs: true,
                    ..settings
                },
                cx,
            );
        });

        let panel = workspace.update_in(cx, ProjectPanel::new);
        cx.run_until_parked();

        toggle_expand_dir(&panel, "root/dir1", cx);
        toggle_expand_dir(&panel, "root/dir1/subdir1", cx);
        toggle_expand_dir(&panel, "root/dir1/subdir1/nested1", cx);

        assert_eq!(
            visible_entries_as_strings(&panel, 0..20, cx),
            &[
                "v root",
                "    v dir1",
                "        v subdir1/nested1  <== selected",
                "              file1.txt",
                "              file2.txt",
                "        > subdir2",
                "    > dir2/single_file",
            ],
            "初始状态,部分目录已展开"
        );

        let entry_id = find_project_entry(&panel, "root/dir1", cx).unwrap();
        panel.update(cx, |panel, cx| {
            let project = panel.project.read(cx);
            let worktree = project.worktrees(cx).next().unwrap().read(cx);
            panel.collapse_all_for_entry(worktree.id(), entry_id, cx);
        });

        toggle_expand_dir(&panel, "root/dir1", cx);

        assert_eq!(
            visible_entries_as_strings(&panel, 0..20, cx),
            &[
                "v root",
                "    v dir1  <== selected",
                "        > subdir1/nested1",
                "        > subdir2",
                "    > dir2/single_file",
            ],
            "启用自动折叠时,子目录应折叠"
        );
    }

    // Test 3: With auto-fold disabled
    {
        cx.update(|_, cx| {
            let settings = *ProjectPanelSettings::get_global(cx);
            ProjectPanelSettings::override_global(
                ProjectPanelSettings {
                    auto_fold_dirs: false,
                    ..settings
                },
                cx,
            );
        });

        let panel = workspace.update_in(cx, ProjectPanel::new);
        cx.run_until_parked();

        toggle_expand_dir(&panel, "root/dir1", cx);
        toggle_expand_dir(&panel, "root/dir1/subdir1", cx);
        toggle_expand_dir(&panel, "root/dir1/subdir1/nested1", cx);

        assert_eq!(
            visible_entries_as_strings(&panel, 0..20, cx),
            &[
                "v root",
                "    v dir1",
                "        v subdir1",
                "            v nested1  <== selected",
                "                  file1.txt",
                "                  file2.txt",
                "        > subdir2",
                "    > dir2",
            ],
            "初始状态,部分目录已展开且自动折叠已禁用"
        );

        let entry_id = find_project_entry(&panel, "root/dir1", cx).unwrap();
        panel.update(cx, |panel, cx| {
            let project = panel.project.read(cx);
            let worktree = project.worktrees(cx).next().unwrap().read(cx);
            panel.collapse_all_for_entry(worktree.id(), entry_id, cx);
        });

        toggle_expand_dir(&panel, "root/dir1", cx);

        assert_eq!(
            visible_entries_as_strings(&panel, 0..20, cx),
            &[
                "v root",
                "    v dir1  <== selected",
                "        > subdir1",
                "        > subdir2",
                "    > dir2",
            ],
            "禁用自动折叠时,子目录应折叠但不隐藏"
        );
    }
}

#[gpui::test]
async fn test_collapse_selected_entry_and_children_action(cx: &mut gpui::TestAppContext) {
    init_test(cx);

    let fs = FakeFs::new(cx.executor());
    fs.insert_tree(
        path!("/root"),
        json!({
            "dir1": {
                "subdir1": {
                    "nested1": {
                        "file1.txt": "",
                        "file2.txt": ""
                    },
                },
                "subdir2": {
                    "file3.txt": ""
                }
            },
            "dir2": {
                "file4.txt": ""
            }
        }),
    )
    .await;

    let project = Project::test(fs.clone(), [path!("/root").as_ref()], cx).await;
    let window = cx.add_window(|window, cx| MultiWorkspace::test_new(project.clone(), window, cx));
    let workspace = window
        .read_with(cx, |mw, _| mw.workspace().clone())
        .unwrap();
    let cx = &mut VisualTestContext::from_window(window.into(), cx);

    let panel = workspace.update_in(cx, ProjectPanel::new);
    cx.run_until_parked();

    toggle_expand_dir(&panel, "root/dir1", cx);
    toggle_expand_dir(&panel, "root/dir1/subdir1", cx);
    toggle_expand_dir(&panel, "root/dir1/subdir1/nested1", cx);
    toggle_expand_dir(&panel, "root/dir1/subdir2", cx);
    toggle_expand_dir(&panel, "root/dir2", cx);

    assert_eq!(
        visible_entries_as_strings(&panel, 0..20, cx),
        &[
            "v root",
            "    v dir1",
            "        v subdir1",
            "            v nested1",
            "                  file1.txt",
            "                  file2.txt",
            "        v subdir2",
            "              file3.txt",
            "    v dir2  <== selected",
            "          file4.txt",
        ],
        "初始状态,目录已展开"
    );

    select_path(&panel, "root/dir1", cx);
    cx.run_until_parked();

    panel.update_in(cx, |panel, window, cx| {
        panel.collapse_selected_entry_and_children(&CollapseSelectedEntryAndChildren, window, cx);
    });
    cx.run_until_parked();

    assert_eq!(
        visible_entries_as_strings(&panel, 0..20, cx),
        &[
            "v root",
            "    > dir1  <== selected",
            "    v dir2",
            "          file4.txt",
        ],
        "dir1 及其所有子项应折叠,dir2 应保持展开"
    );

    toggle_expand_dir(&panel, "root/dir1", cx);
    cx.run_until_parked();

    assert_eq!(
        visible_entries_as_strings(&panel, 0..20, cx),
        &[
            "v root",
            "    v dir1  <== selected",
            "        > subdir1",
            "        > subdir2",
            "    v dir2",
            "          file4.txt",
        ],
        "重新展开 dir1 后,其子项仍应折叠"
    );
}

#[gpui::test]
async fn test_collapse_root_single_worktree(cx: &mut gpui::TestAppContext) {
    init_test(cx);

    let fs = FakeFs::new(cx.executor());
    fs.insert_tree(
        path!("/root"),
        json!({
            "dir1": {
                "subdir1": {
                    "file1.txt": ""
                },
                "file2.txt": ""
            },
            "dir2": {
                "file3.txt": ""
            }
        }),
    )
    .await;

    let project = Project::test(fs.clone(), [path!("/root").as_ref()], cx).await;
    let window = cx.add_window(|window, cx| MultiWorkspace::test_new(project.clone(), window, cx));
    let workspace = window
        .read_with(cx, |mw, _| mw.workspace().clone())
        .unwrap();
    let cx = &mut VisualTestContext::from_window(window.into(), cx);

    let panel = workspace.update_in(cx, ProjectPanel::new);
    cx.run_until_parked();

    toggle_expand_dir(&panel, "root/dir1", cx);
    toggle_expand_dir(&panel, "root/dir1/subdir1", cx);
    toggle_expand_dir(&panel, "root/dir2", cx);

    assert_eq!(
        visible_entries_as_strings(&panel, 0..20, cx),
        &[
            "v root",
            "    v dir1",
            "        v subdir1",
            "              file1.txt",
            "          file2.txt",
            "    v dir2  <== selected",
            "          file3.txt",
        ],
        "初始状态,目录已展开"
    );

    // Select the root and collapse it and its children
    select_path(&panel, "root", cx);
    cx.run_until_parked();

    panel.update_in(cx, |panel, window, cx| {
        panel.collapse_selected_entry_and_children(&CollapseSelectedEntryAndChildren, window, cx);
    });
    cx.run_until_parked();

    // The root and all its children should be collapsed
    assert_eq!(
        visible_entries_as_strings(&panel, 0..20, cx),
        &["> root  <== selected"],
        "根目录及其所有子项应折叠"
    );

    // Re-expand root and dir1, verify children were recursively collapsed
    toggle_expand_dir(&panel, "root", cx);
    toggle_expand_dir(&panel, "root/dir1", cx);
    cx.run_until_parked();

    assert_eq!(
        visible_entries_as_strings(&panel, 0..20, cx),
        &[
            "v root",
            "    v dir1  <== selected",
            "        > subdir1",
            "          file2.txt",
            "    > dir2",
        ],
        "重新展开根目录和 dir1 后,subdir1 仍应折叠"
    );
}

#[gpui::test]
async fn test_collapse_root_multi_worktree(cx: &mut gpui::TestAppContext) {
    init_test(cx);

    let fs = FakeFs::new(cx.executor());
    fs.insert_tree(
        path!("/root1"),
        json!({
            "dir1": {
                "subdir1": {
                    "file1.txt": ""
                },
                "file2.txt": ""
            }
        }),
    )
    .await;
    fs.insert_tree(
        path!("/root2"),
        json!({
            "dir2": {
                "file3.txt": ""
            },
            "file4.txt": ""
        }),
    )
    .await;

    let project = Project::test(
        fs.clone(),
        [path!("/root1").as_ref(), path!("/root2").as_ref()],
        cx,
    )
    .await;
    let window = cx.add_window(|window, cx| MultiWorkspace::test_new(project.clone(), window, cx));
    let workspace = window
        .read_with(cx, |mw, _| mw.workspace().clone())
        .unwrap();
    let cx = &mut VisualTestContext::from_window(window.into(), cx);

    let panel = workspace.update_in(cx, ProjectPanel::new);
    cx.run_until_parked();

    toggle_expand_dir(&panel, "root1/dir1", cx);
    toggle_expand_dir(&panel, "root1/dir1/subdir1", cx);
    toggle_expand_dir(&panel, "root2/dir2", cx);

    assert_eq!(
        visible_entries_as_strings(&panel, 0..20, cx),
        &[
            "v root1",
            "    v dir1",
            "        v subdir1",
            "              file1.txt",
            "          file2.txt",
            "v root2",
            "    v dir2  <== selected",
            "          file3.txt",
            "      file4.txt",
        ],
        "初始状态,多个工作树中的目录已展开"
    );

    // Select root1 and collapse it and its children.
    // In a multi-worktree project, this should only collapse the selected worktree,
    // leaving other worktrees unaffected.
    select_path(&panel, "root1", cx);
    cx.run_until_parked();

    panel.update_in(cx, |panel, window, cx| {
        panel.collapse_selected_entry_and_children(&CollapseSelectedEntryAndChildren, window, cx);
    });
    cx.run_until_parked();

    assert_eq!(
        visible_entries_as_strings(&panel, 0..20, cx),
        &[
            "> root1  <== selected",
            "v root2",
            "    v dir2",
            "          file3.txt",
            "      file4.txt",
        ],
        "仅 root1 应折叠,root2 应保持展开"
    );

    // Re-expand root1 and verify its children were recursively collapsed
    toggle_expand_dir(&panel, "root1", cx);

    assert_eq!(
        visible_entries_as_strings(&panel, 0..20, cx),
        &[
            "v root1  <== selected",
            "    > dir1",
            "v root2",
            "    v dir2",
            "          file3.txt",
            "      file4.txt",
        ],
        "重新展开 root1 后,dir1 仍应折叠,root2 应不受影响"
    );
}

#[gpui::test]
async fn test_collapse_non_root_multi_worktree(cx: &mut gpui::TestAppContext) {
    init_test(cx);

    let fs = FakeFs::new(cx.executor());
    fs.insert_tree(
        path!("/root1"),
        json!({
            "dir1": {
                "subdir1": {
                    "file1.txt": ""
                },
                "file2.txt": ""
            }
        }),
    )
    .await;
    fs.insert_tree(
        path!("/root2"),
        json!({
            "dir2": {
                "subdir2": {
                    "file3.txt": ""
                },
                "file4.txt": ""
            }
        }),
    )
    .await;

    let project = Project::test(
        fs.clone(),
        [path!("/root1").as_ref(), path!("/root2").as_ref()],
        cx,
    )
    .await;
    let window = cx.add_window(|window, cx| MultiWorkspace::test_new(project.clone(), window, cx));
    let workspace = window
        .read_with(cx, |mw, _| mw.workspace().clone())
        .unwrap();
    let cx = &mut VisualTestContext::from_window(window.into(), cx);

    let panel = workspace.update_in(cx, ProjectPanel::new);
    cx.run_until_parked();

    toggle_expand_dir(&panel, "root1/dir1", cx);
    toggle_expand_dir(&panel, "root1/dir1/subdir1", cx);
    toggle_expand_dir(&panel, "root2/dir2", cx);
    toggle_expand_dir(&panel, "root2/dir2/subdir2", cx);

    assert_eq!(
        visible_entries_as_strings(&panel, 0..20, cx),
        &[
            "v root1",
            "    v dir1",
            "        v subdir1",
            "              file1.txt",
            "          file2.txt",
            "v root2",
            "    v dir2",
            "        v subdir2  <== selected",
            "              file3.txt",
            "          file4.txt",
        ],
        "初始状态,多个工作树中的目录已展开"
    );

    // Select dir1 in root1 and collapse it
    select_path(&panel, "root1/dir1", cx);
    cx.run_until_parked();

    panel.update_in(cx, |panel, window, cx| {
        panel.collapse_selected_entry_and_children(&CollapseSelectedEntryAndChildren, window, cx);
    });
    cx.run_until_parked();

    assert_eq!(
        visible_entries_as_strings(&panel, 0..20, cx),
        &[
            "v root1",
            "    > dir1  <== selected",
            "v root2",
            "    v dir2",
            "        v subdir2",
            "              file3.txt",
            "          file4.txt",
        ],
        "仅 dir1 应折叠,root2 应完全不受影响"
    );

    // Re-expand dir1 and verify subdir1 was recursively collapsed
    toggle_expand_dir(&panel, "root1/dir1", cx);

    assert_eq!(
        visible_entries_as_strings(&panel, 0..20, cx),
        &[
            "v root1",
            "    v dir1  <== selected",
            "        > subdir1",
            "          file2.txt",
            "v root2",
            "    v dir2",
            "        v subdir2",
            "              file3.txt",
            "          file4.txt",
        ],
        "重新展开 dir1 后,subdir1 仍应折叠"
    );
}

#[gpui::test]
async fn test_expand_all_entries(cx: &mut gpui::TestAppContext) {
    init_test_with_editor(cx);

    let fs = FakeFs::new(cx.executor());
    fs.insert_tree(
        "/project_root",
        json!({
            "dir_1": {
                "nested_dir": {
                    "file_a.py": "# File contents",
                    "file_b.py": "# File contents",
                    "file_c.py": "# File contents",
                },
                "file_1.py": "# File contents",
                "file_2.py": "# File contents",
                "file_3.py": "# File contents",
            },
            "dir_2": {
                "file_1.py": "# File contents",
                "file_2.py": "# File contents",
                "file_3.py": "# File contents",
            }
        }),
    )
    .await;

    let project = Project::test(fs.clone(), ["/project_root".as_ref()], cx).await;
    let window = cx.add_window(|window, cx| MultiWorkspace::test_new(project.clone(), window, cx));
    let workspace = window
        .read_with(cx, |mw, _| mw.workspace().clone())
        .unwrap();
    let cx = &mut VisualTestContext::from_window(window.into(), cx);
    let panel = workspace.update_in(cx, ProjectPanel::new);
    cx.run_until_parked();

    panel.update_in(cx, |panel, window, cx| {
        panel.collapse_all_entries(&CollapseAllEntries, window, cx)
    });
    cx.executor().run_until_parked();
    assert_eq!(
        visible_entries_as_strings(&panel, 0..10, cx),
        &["v project_root", "    > dir_1", "    > dir_2",]
    );

    panel.update_in(cx, |panel, window, cx| {
        panel.expand_all_entries(&ExpandAllEntries, window, cx)
    });
    cx.executor().run_until_parked();

    let entries = visible_entries_as_strings(&panel, 0..20, cx);
    assert_eq!(entries.len(), 13, "应显示全部 13 个条目");
    assert!(entries[0].starts_with("v project_root"), "根目录已展开");
    assert!(entries[1].contains("v dir_1"), "dir_1 已展开");
    assert!(entries[2].contains("v nested_dir"), "nested_dir 已展开");
    assert!(
        entries.iter().any(|e| e.contains("file_a.py")),
        "file_a 可见"
    );
    assert!(
        entries.iter().any(|e| e.contains("file_c.py")),
        "file_c 可见"
    );
    assert!(
        entries.iter().any(|e| e.contains("v dir_2")),
        "dir_2 已展开"
    );
    assert!(
        !entries.iter().any(|e| e.contains("> ")),
        "无折叠目录"
    );
}

#[gpui::test]
async fn test_expand_all_entries_multiple_worktrees(cx: &mut gpui::TestAppContext) {
    init_test_with_editor(cx);

    let fs = FakeFs::new(cx.executor());
    let worktree_content = json!({
        "dir_1": {
            "file_1.py": "# File contents",
        },
        "dir_2": {
            "file_1.py": "# File contents",
        }
    });

    fs.insert_tree("/project_root_1", worktree_content.clone())
        .await;
    fs.insert_tree("/project_root_2", worktree_content).await;

    let project = Project::test(
        fs.clone(),
        ["/project_root_1".as_ref(), "/project_root_2".as_ref()],
        cx,
    )
    .await;
    let window = cx.add_window(|window, cx| MultiWorkspace::test_new(project.clone(), window, cx));
    let workspace = window
        .read_with(cx, |mw, _| mw.workspace().clone())
        .unwrap();
    let cx = &mut VisualTestContext::from_window(window.into(), cx);
    let panel = workspace.update_in(cx, ProjectPanel::new);
    cx.run_until_parked();

    panel.update_in(cx, |panel, window, cx| {
        panel.collapse_all_entries(&CollapseAllEntries, window, cx)
    });
    cx.executor().run_until_parked();
    assert_eq!(
        visible_entries_as_strings(&panel, 0..10, cx),
        &["> project_root_1", "> project_root_2",]
    );

    panel.update_in(cx, |panel, window, cx| {
        panel.expand_all_entries(&ExpandAllEntries, window, cx)
    });
    cx.executor().run_until_parked();
    assert_eq!(
        visible_entries_as_strings(&panel, 0..20, cx),
        &[
            "v project_root_1",
            "    v dir_1",
            "          file_1.py",
            "    v dir_2",
            "          file_1.py",
            "v project_root_2",
            "    v dir_1",
            "          file_1.py",
            "    v dir_2",
            "          file_1.py",
        ]
    );
}

#[gpui::test]
async fn test_expand_all_entries_via_window_dispatch(cx: &mut gpui::TestAppContext) {
    init_test_with_editor(cx);

    let fs = FakeFs::new(cx.executor());
    let worktree_content = json!({
        "dir_1": {
            "file_1.py": "# File contents",
        },
        "dir_2": {
            "file_1.py": "# File contents",
        }
    });

    fs.insert_tree("/project_root_1", worktree_content.clone())
        .await;
    fs.insert_tree("/project_root_2", worktree_content).await;

    let project = Project::test(
        fs.clone(),
        ["/project_root_1".as_ref(), "/project_root_2".as_ref()],
        cx,
    )
    .await;
    let window = cx.add_window(|window, cx| MultiWorkspace::test_new(project.clone(), window, cx));
    let workspace = window
        .read_with(cx, |mw, _| mw.workspace().clone())
        .unwrap();
    let cx = &mut VisualTestContext::from_window(window.into(), cx);
    cx.update(|_, cx| {
        let settings = *ProjectPanelSettings::get_global(cx);
        ProjectPanelSettings::override_global(
            ProjectPanelSettings {
                auto_reveal_entries: false,
                ..settings
            },
            cx,
        );
    });
    let panel = workspace.update_in(cx, |workspace, window, cx| {
        let panel = ProjectPanel::new(workspace, window, cx);
        workspace.add_panel(panel.clone(), window, cx);
        panel
    });
    cx.run_until_parked();

    panel.update_in(cx, |panel, window, cx| {
        panel.collapse_all_entries(&CollapseAllEntries, window, cx)
    });
    cx.executor().run_until_parked();
    assert_eq!(
        visible_entries_as_strings(&panel, 0..10, cx),
        &["> project_root_1", "> project_root_2",]
    );

    panel.update_in(cx, |panel, window, cx| {
        panel.focus_handle(cx).focus(window, cx);
    });
    cx.dispatch_action(ExpandAllEntries);
    cx.executor().run_until_parked();
    assert_eq!(
        visible_entries_as_strings(&panel, 0..20, cx),
        &[
            "v project_root_1",
            "    v dir_1",
            "          file_1.py",
            "    v dir_2",
            "          file_1.py",
            "v project_root_2",
            "    v dir_1",
            "          file_1.py",
            "    v dir_2",
            "          file_1.py",
        ]
    );
}

#[gpui::test]
async fn test_expand_all_for_entry_single_worktree(cx: &mut gpui::TestAppContext) {
    init_test_with_editor(cx);

    let fs = FakeFs::new(cx.executor());
    let worktree_content = json!({
        "dir_1": {
            "file_1.py": "# File contents",
        },
        "dir_2": {
            "file_1.py": "# File contents",
        }
    });

    fs.insert_tree("/project_root_1", worktree_content.clone())
        .await;
    fs.insert_tree("/project_root_2", worktree_content).await;

    let project = Project::test(
        fs.clone(),
        ["/project_root_1".as_ref(), "/project_root_2".as_ref()],
        cx,
    )
    .await;
    let window = cx.add_window(|window, cx| MultiWorkspace::test_new(project.clone(), window, cx));
    let workspace = window
        .read_with(cx, |mw, _| mw.workspace().clone())
        .unwrap();
    let cx = &mut VisualTestContext::from_window(window.into(), cx);
    let panel = workspace.update_in(cx, ProjectPanel::new);
    cx.run_until_parked();

    panel.update_in(cx, |panel, window, cx| {
        panel.collapse_all_entries(&CollapseAllEntries, window, cx)
    });
    cx.executor().run_until_parked();
    assert_eq!(
        visible_entries_as_strings(&panel, 0..10, cx),
        &["> project_root_1", "> project_root_2",]
    );

    let root2_entry = find_project_entry(&panel, "project_root_2", cx).unwrap();
    panel.update_in(cx, |panel, window, cx| {
        let worktree_id = panel
            .project
            .read(cx)
            .worktree_id_for_entry(root2_entry, cx)
            .unwrap();
        panel.expand_all_for_entry(worktree_id, root2_entry, cx);
        panel.update_visible_entries(None, false, false, window, cx);
    });
    cx.executor().run_until_parked();
    assert_eq!(
        visible_entries_as_strings(&panel, 0..20, cx),
        &[
            "> project_root_1",
            "v project_root_2",
            "    v dir_1",
            "          file_1.py",
            "    v dir_2",
            "          file_1.py",
        ]
    );
}

#[gpui::test]
async fn test_expand_all_entries_with_auto_fold(cx: &mut gpui::TestAppContext) {
    init_test_with_editor(cx);

    let fs = FakeFs::new(cx.executor());
    fs.insert_tree(
        path!("/root"),
        json!({
            "dir1": {
                "empty1": {
                    "empty2": {
                        "empty3": {
                            "file.txt": ""
                        }
                    }
                },
            }
        }),
    )
    .await;

    let project = Project::test(fs.clone(), [path!("/root").as_ref()], cx).await;
    let window = cx.add_window(|window, cx| MultiWorkspace::test_new(project.clone(), window, cx));
    let workspace = window
        .read_with(cx, |mw, _| mw.workspace().clone())
        .unwrap();
    let cx = &mut VisualTestContext::from_window(window.into(), cx);

    cx.update(|_, cx| {
        let settings = *ProjectPanelSettings::get_global(cx);
        ProjectPanelSettings::override_global(
            ProjectPanelSettings {
                auto_fold_dirs: true,
                ..settings
            },
            cx,
        );
    });

    let panel = workspace.update_in(cx, ProjectPanel::new);
    cx.run_until_parked();

    panel.update_in(cx, |panel, window, cx| {
        panel.collapse_all_entries(&CollapseAllEntries, window, cx)
    });
    cx.executor().run_until_parked();

    panel.update_in(cx, |panel, window, cx| {
        panel.expand_all_entries(&ExpandAllEntries, window, cx)
    });
    cx.executor().run_until_parked();

    assert_eq!(
        visible_entries_as_strings(&panel, 0..20, cx),
        &[
            "v root",
            "    v dir1",
            "        v empty1",
            "            v empty2",
            "                v empty3",
            "                      file.txt",
        ],
        "全部展开应展开自动折叠的目录"
    );
}

#[gpui::test]
async fn test_create_entries_without_selection(cx: &mut gpui::TestAppContext) {
    init_test(cx);

    let fs = FakeFs::new(cx.executor());
    fs.insert_tree(
        path!("/root"),
        json!({
            "dir1": {
                "file1.txt": "",
            },
        }),
    )
    .await;

    let project = Project::test(fs.clone(), [path!("/root").as_ref()], cx).await;
    let window = cx.add_window(|window, cx| MultiWorkspace::test_new(project.clone(), window, cx));
    let workspace = window
        .read_with(cx, |mw, _| mw.workspace().clone())
        .unwrap();
    let cx = &mut VisualTestContext::from_window(window.into(), cx);

    let panel = workspace.update_in(cx, |workspace, window, cx| {
        let panel = ProjectPanel::new(workspace, window, cx);
        workspace.add_panel(panel.clone(), window, cx);
        panel
    });
    cx.run_until_parked();

    #[rustfmt::skip]
    assert_eq!(
        visible_entries_as_strings(&panel, 0..20, cx),
        &[
            "v root",
            "    > dir1",
        ],
        "初始状态,未选择任何项"
    );

    panel.update_in(cx, |panel, window, cx| {
        panel.new_file(&NewFile, window, cx);
    });
    cx.run_until_parked();
    panel.update_in(cx, |panel, window, cx| {
        assert!(panel.filename_editor.read(cx).is_focused(window));
    });
    panel
        .update_in(cx, |panel, window, cx| {
            panel.filename_editor.update(cx, |editor, cx| {
                editor.set_text("hello_from_no_selections", window, cx)
            });
            panel.confirm_edit(true, window, cx).unwrap()
        })
        .await
        .unwrap();
    cx.run_until_parked();
    #[rustfmt::skip]
    assert_eq!(
        visible_entries_as_strings(&panel, 0..20, cx),
        &[
            "v root",
            "    > dir1",
            "      hello_from_no_selections  <== selected  <== marked",
        ],
        "在根目录下创建新文件"
    );
}

#[gpui::test]
async fn test_create_entries_without_selection_hide_root(cx: &mut gpui::TestAppContext) {
    init_test(cx);

    let fs = FakeFs::new(cx.executor());
    fs.insert_tree(
        path!("/root"),
        json!({
            "existing_dir": {
                "existing_file.txt": "",
            },
            "existing_file.txt": "",
        }),
    )
    .await;

    let project = Project::test(fs.clone(), [path!("/root").as_ref()], cx).await;
    let window = cx.add_window(|window, cx| MultiWorkspace::test_new(project.clone(), window, cx));
    let workspace = window
        .read_with(cx, |mw, _| mw.workspace().clone())
        .unwrap();
    let cx = &mut VisualTestContext::from_window(window.into(), cx);

    cx.update(|_, cx| {
        let settings = *ProjectPanelSettings::get_global(cx);
        ProjectPanelSettings::override_global(
            ProjectPanelSettings {
                hide_root: true,
                ..settings
            },
            cx,
        );
    });

    let panel = workspace.update_in(cx, |workspace, window, cx| {
        let panel = ProjectPanel::new(workspace, window, cx);
        workspace.add_panel(panel.clone(), window, cx);
        panel
    });
    cx.run_until_parked();

    #[rustfmt::skip]
    assert_eq!(
        visible_entries_as_strings(&panel, 0..20, cx),
        &[
            "> existing_dir",
            "  existing_file.txt",
        ],
        "初始状态,hide_root=true,根目录应隐藏且未选择任何项"
    );

    panel.update(cx, |panel, _| {
        assert!(
            panel.selection.is_none(),
            "初始应无选择"
        );
    });

    // Test 1: Create new file when no entry is selected
    panel.update_in(cx, |panel, window, cx| {
        panel.new_file(&NewFile, window, cx);
    });
    cx.run_until_parked();
    panel.update_in(cx, |panel, window, cx| {
        assert!(panel.filename_editor.read(cx).is_focused(window));
    });
    cx.run_until_parked();
    #[rustfmt::skip]
    assert_eq!(
        visible_entries_as_strings(&panel, 0..20, cx),
        &[
            "> existing_dir",
            "  [EDITOR: '']  <== selected",
            "  existing_file.txt",
        ],
        "当 hide_root=true 且无选择时,编辑器应出现在根级别"
    );

    let confirm = panel.update_in(cx, |panel, window, cx| {
        panel.filename_editor.update(cx, |editor, cx| {
            editor.set_text("new_file_at_root.txt", window, cx)
        });
        panel.confirm_edit(true, window, cx).unwrap()
    });
    confirm.await.unwrap();
    cx.run_until_parked();

    #[rustfmt::skip]
    assert_eq!(
        visible_entries_as_strings(&panel, 0..20, cx),
        &[
            "> existing_dir",
            "  existing_file.txt",
            "  new_file_at_root.txt  <== selected  <== marked",
        ],
        "新文件应在根级别创建,且不带根前缀可见"
    );

    assert!(
        fs.is_file(Path::new("/root/new_file_at_root.txt")).await,
        "文件应在实际根目录中创建"
    );

    // Test 2: Create new directory when no entry is selected
    panel.update(cx, |panel, _| {
        panel.selection = None;
    });

    panel.update_in(cx, |panel, window, cx| {
        panel.new_directory(&NewDirectory, window, cx);
    });
    cx.run_until_parked();

    panel.update_in(cx, |panel, window, cx| {
        assert!(panel.filename_editor.read(cx).is_focused(window));
    });

    #[rustfmt::skip]
    assert_eq!(
        visible_entries_as_strings(&panel, 0..20, cx),
        &[
            "> [EDITOR: '']  <== selected",
            "> existing_dir",
            "  existing_file.txt",
            "  new_file_at_root.txt",
        ],
        "当 hide_root=true 且无选择时,目录编辑器应出现在根级别"
    );

    let confirm = panel.update_in(cx, |panel, window, cx| {
        panel.filename_editor.update(cx, |editor, cx| {
            editor.set_text("new_dir_at_root", window, cx)
        });
        panel.confirm_edit(true, window, cx).unwrap()
    });
    confirm.await.unwrap();
    cx.run_until_parked();

    #[rustfmt::skip]
    assert_eq!(
        visible_entries_as_strings(&panel, 0..20, cx),
        &[
            "> existing_dir",
            "v new_dir_at_root  <== selected",
            "  existing_file.txt",
            "  new_file_at_root.txt",
        ],
        "新目录应在根级别创建,且不带根前缀可见"
    );

    assert!(
        fs.is_dir(Path::new("/root/new_dir_at_root")).await,
        "目录应在实际根目录中创建"
    );
}

#[gpui::test]
async fn test_context_menu_new_file_in_empty_hidden_root(cx: &mut gpui::TestAppContext) {
    init_test(cx);

    let fs = FakeFs::new(cx.executor());
    fs.insert_tree(path!("/root"), json!({})).await;

    let project = Project::test(fs.clone(), [path!("/root").as_ref()], cx).await;
    let window = cx.add_window(|window, cx| MultiWorkspace::test_new(project.clone(), window, cx));
    let workspace = window
        .read_with(cx, |mw, _| mw.workspace().clone())
        .unwrap();
    let cx = &mut VisualTestContext::from_window(window.into(), cx);

    cx.update(|_, cx| {
        let settings = *ProjectPanelSettings::get_global(cx);
        ProjectPanelSettings::override_global(
            ProjectPanelSettings {
                hide_root: true,
                ..settings
            },
            cx,
        );
    });

    let panel = workspace.update_in(cx, |workspace, window, cx| {
        let panel = ProjectPanel::new(workspace, window, cx);
        workspace.add_panel(panel.clone(), window, cx);
        panel
    });
    cx.run_until_parked();

    assert!(
        visible_entries_as_strings(&panel, 0..20, cx).is_empty(),
        "hide_root=true 时,空工作树应不渲染任何条目"
    );

    panel.update(cx, |panel, _| {
        assert!(
            panel.selection.is_none(),
            "项目面板启动时不应有选择"
        );
        assert!(
            panel.state.last_worktree_root_id.is_some(),
            "项目面板仍应跟踪隐藏的根条目"
        );
    });

    panel.update_in(cx, |panel, window, cx| {
        let root_entry_id = panel
            .state
            .last_worktree_root_id
            .expect("隐藏的根应可用于后台上下文菜单操作");
        panel.deploy_context_menu(
            gpui::point(gpui::px(1.), gpui::px(1.)),
            root_entry_id,
            window,
            cx,
        );
        panel.new_file(&NewFile, window, cx);
    });
    cx.run_until_parked();

    panel.update_in(cx, |panel, window, cx| {
        assert!(
            panel.filename_editor.read(cx).is_focused(window),
            "后台上下文菜单中的“新建文件”应打开文件名编辑器"
        );
    });

    assert_eq!(
        visible_entries_as_strings(&panel, 0..20, cx),
        &["  [EDITOR: '']  <== selected"],
        "新文件编辑器应出现在隐藏的根级别"
    );

    let confirm = panel.update_in(cx, |panel, window, cx| {
        panel.filename_editor.update(cx, |editor, cx| {
            editor.set_text("new_file_from_context_menu.txt", window, cx)
        });
        panel.confirm_edit(true, window, cx).unwrap()
    });
    confirm.await.unwrap();
    cx.run_until_parked();

    assert_eq!(
        visible_entries_as_strings(&panel, 0..20, cx),
        &["  new_file_from_context_menu.txt  <== selected  <== marked"],
        "确认的文件应出现在隐藏的根级别"
    );

    assert!(
        fs.is_file(Path::new("/root/new_file_from_context_menu.txt"))
            .await,
        "文件应在空根目录中创建"
    );
}

#[cfg(windows)]
#[gpui::test]
async fn test_create_entry_with_trailing_dot_windows(cx: &mut gpui::TestAppContext) {
    init_test(cx);

    let fs = FakeFs::new(cx.executor());
    fs.insert_tree(
        path!("/root"),
        json!({
            "dir1": {
                "file1.txt": "",
            },
        }),
    )
    .await;

    let project = Project::test(fs.clone(), [path!("/root").as_ref()], cx).await;
    let window = cx.add_window(|window, cx| MultiWorkspace::test_new(project.clone(), window, cx));
    let workspace = window
        .read_with(cx, |mw, _| mw.workspace().clone())
        .unwrap();
    let cx = &mut VisualTestContext::from_window(window.into(), cx);

    let panel = workspace.update_in(cx, |workspace, window, cx| {
        let panel = ProjectPanel::new(workspace, window, cx);
        workspace.add_panel(panel.clone(), window, cx);
        panel
    });
    cx.run_until_parked();

    #[rustfmt::skip]
    assert_eq!(
        visible_entries_as_strings(&panel, 0..20, cx),
        &[
            "v root",
            "    > dir1",
        ],
        "初始状态,未选择任何项"
    );

    panel.update_in(cx, |panel, window, cx| {
        panel.new_file(&NewFile, window, cx);
    });
    cx.run_until_parked();
    panel.update_in(cx, |panel, window, cx| {
        assert!(panel.filename_editor.read(cx).is_focused(window));
    });
    panel
        .update_in(cx, |panel, window, cx| {
            panel
                .filename_editor
                .update(cx, |editor, cx| editor.set_text("foo.", window, cx));
            panel.confirm_edit(true, window, cx).unwrap()
        })
        .await
        .unwrap();
    cx.run_until_parked();
    #[rustfmt::skip]
    assert_eq!(
        visible_entries_as_strings(&panel, 0..20, cx),
        &[
            "v root",
            "    > dir1",
            "      foo  <== selected  <== marked",
        ],
        "在根目录下创建新文件,不带尾随点"
    );
}

#[gpui::test]
async fn test_highlight_entry_for_external_drag(cx: &mut gpui::TestAppContext) {
    init_test(cx);

    let fs = FakeFs::new(cx.executor());
    fs.insert_tree(
        "/root",
        json!({
            "dir1": {
                "file1.txt": "",
                "dir2": {
                    "file2.txt": ""
                }
            },
            "file3.txt": ""
        }),
    )
    .await;

    let project = Project::test(fs.clone(), ["/root".as_ref()], cx).await;
    let window = cx.add_window(|window, cx| MultiWorkspace::test_new(project.clone(), window, cx));
    let workspace = window
        .read_with(cx, |mw, _| mw.workspace().clone())
        .unwrap();
    let cx = &mut VisualTestContext::from_window(window.into(), cx);
    let panel = workspace.update_in(cx, ProjectPanel::new);
    cx.run_until_parked();

    panel.update(cx, |panel, cx| {
        let project = panel.project.read(cx);
        let worktree = project.visible_worktrees(cx).next().unwrap();
        let worktree = worktree.read(cx);

        // Test 1: Target is a directory, should highlight the directory itself
        let dir_entry = worktree.entry_for_path(rel_path("dir1")).unwrap();
        let result = panel.highlight_entry_for_external_drag(dir_entry, worktree);
        assert_eq!(
            result,
            Some(dir_entry.id),
            "应高亮目录本身"
        );

        // Test 2: Target is nested file, should highlight immediate parent
        let nested_file = worktree
            .entry_for_path(rel_path("dir1/dir2/file2.txt"))
            .unwrap();
        let nested_parent = worktree.entry_for_path(rel_path("dir1/dir2")).unwrap();
        let result = panel.highlight_entry_for_external_drag(nested_file, worktree);
        assert_eq!(
            result,
            Some(nested_parent.id),
            "应高亮直接父级"
        );

        // Test 3: Target is root level file, should highlight root
        let root_file = worktree.entry_for_path(rel_path("file3.txt")).unwrap();
        let result = panel.highlight_entry_for_external_drag(root_file, worktree);
        assert_eq!(
            result,
            Some(worktree.root_entry().unwrap().id),
            "根级别文件应返回 None"
        );

        // Test 4: Target is root itself, should highlight root
        let root_entry = worktree.root_entry().unwrap();
        let result = panel.highlight_entry_for_external_drag(root_entry, worktree);
        assert_eq!(
            result,
            Some(root_entry.id),
            "根级别文件应返回 None"
        );
    });
}

#[gpui::test]
async fn test_highlight_entry_for_selection_drag(cx: &mut gpui::TestAppContext) {
    init_test(cx);

    let fs = FakeFs::new(cx.executor());
    fs.insert_tree(
        "/root",
        json!({
            "parent_dir": {
                "child_file.txt": "",
                "sibling_file.txt": "",
                "child_dir": {
                    "nested_file.txt": ""
                }
            },
            "other_dir": {
                "other_file.txt": ""
            }
        }),
    )
    .await;

    let project = Project::test(fs.clone(), ["/root".as_ref()], cx).await;
    let window = cx.add_window(|window, cx| MultiWorkspace::test_new(project.clone(), window, cx));
    let workspace = window
        .read_with(cx, |mw, _| mw.workspace().clone())
        .unwrap();
    let cx = &mut VisualTestContext::from_window(window.into(), cx);
    let panel = workspace.update_in(cx, ProjectPanel::new);
    cx.run_until_parked();

    panel.update(cx, |panel, cx| {
        let project = panel.project.read(cx);
        let worktree = project.visible_worktrees(cx).next().unwrap();
        let worktree_id = worktree.read(cx).id();
        let worktree = worktree.read(cx);

        let parent_dir = worktree.entry_for_path(rel_path("parent_dir")).unwrap();
        let child_file = worktree
            .entry_for_path(rel_path("parent_dir/child_file.txt"))
            .unwrap();
        let sibling_file = worktree
            .entry_for_path(rel_path("parent_dir/sibling_file.txt"))
            .unwrap();
        let child_dir = worktree
            .entry_for_path(rel_path("parent_dir/child_dir"))
            .unwrap();
        let other_dir = worktree.entry_for_path(rel_path("other_dir")).unwrap();
        let other_file = worktree
            .entry_for_path(rel_path("other_dir/other_file.txt"))
            .unwrap();

        // Test 1: Single item drag, don't highlight parent directory
        let dragged_selection = DraggedSelection {
            active_selection: SelectedEntry {
                worktree_id,
                entry_id: child_file.id,
            },
            marked_selections: Arc::new([SelectedEntry {
                worktree_id,
                entry_id: child_file.id,
            }]),
        };
        let result =
            panel.highlight_entry_for_selection_drag(parent_dir, worktree, &dragged_selection, cx);
        assert_eq!(result, None, "不应高亮拖拽项的父级");

        // Test 2: Single item drag, don't highlight sibling files
        let result = panel.highlight_entry_for_selection_drag(
            sibling_file,
            worktree,
            &dragged_selection,
            cx,
        );
        assert_eq!(result, None, "不应高亮同级文件");

        // Test 3: Single item drag, highlight unrelated directory
        let result =
            panel.highlight_entry_for_selection_drag(other_dir, worktree, &dragged_selection, cx);
        assert_eq!(
            result,
            Some(other_dir.id),
            "应高亮无关目录"
        );

        // Test 4: Single item drag, highlight sibling directory
        let result =
            panel.highlight_entry_for_selection_drag(child_dir, worktree, &dragged_selection, cx);
        assert_eq!(
            result,
            Some(child_dir.id),
            "应高亮同级目录"
        );

        // Test 5: Multiple items drag, highlight parent directory
        let dragged_selection = DraggedSelection {
            active_selection: SelectedEntry {
                worktree_id,
                entry_id: child_file.id,
            },
            marked_selections: Arc::new([
                SelectedEntry {
                    worktree_id,
                    entry_id: child_file.id,
                },
                SelectedEntry {
                    worktree_id,
                    entry_id: sibling_file.id,
                },
            ]),
        };
        let result =
            panel.highlight_entry_for_selection_drag(parent_dir, worktree, &dragged_selection, cx);
        assert_eq!(
            result,
            Some(parent_dir.id),
            "应高亮包含多个项的父级"
        );

        // Test 6: Target is file in different directory, highlight parent
        let result =
            panel.highlight_entry_for_selection_drag(other_file, worktree, &dragged_selection, cx);
        assert_eq!(
            result,
            Some(other_dir.id),
            "应高亮目标文件的父级"
        );

        // Test 7: Target is directory, always highlight
        let result =
            panel.highlight_entry_for_selection_drag(child_dir, worktree, &dragged_selection, cx);
        assert_eq!(
            result,
            Some(child_dir.id),
            "应始终高亮目录"
        );
    });
}

#[gpui::test]
async fn test_highlight_entry_for_selection_drag_cross_worktree(cx: &mut gpui::TestAppContext) {
    init_test(cx);

    let fs = FakeFs::new(cx.executor());
    fs.insert_tree(
        "/root1",
        json!({
            "src": {
                "main.rs": "",
                "lib.rs": ""
            }
        }),
    )
    .await;
    fs.insert_tree(
        "/root2",
        json!({
            "src": {
                "main.rs": "",
                "test.rs": ""
            }
        }),
    )
    .await;

    let project = Project::test(fs.clone(), ["/root1".as_ref(), "/root2".as_ref()], cx).await;
    let window = cx.add_window(|window, cx| MultiWorkspace::test_new(project.clone(), window, cx));
    let workspace = window
        .read_with(cx, |mw, _| mw.workspace().clone())
        .unwrap();
    let cx = &mut VisualTestContext::from_window(window.into(), cx);
    let panel = workspace.update_in(cx, ProjectPanel::new);
    cx.run_until_parked();

    panel.update(cx, |panel, cx| {
        let project = panel.project.read(cx);
        let worktrees: Vec<_> = project.visible_worktrees(cx).collect();

        let worktree_a = &worktrees[0];
        let main_rs_from_a = worktree_a
            .read(cx)
            .entry_for_path(rel_path("src/main.rs"))
            .unwrap();

        let worktree_b = &worktrees[1];
        let src_dir_from_b = worktree_b.read(cx).entry_for_path(rel_path("src")).unwrap();
        let main_rs_from_b = worktree_b
            .read(cx)
            .entry_for_path(rel_path("src/main.rs"))
            .unwrap();

        // Test dragging file from worktree A onto parent of file with same relative path in worktree B
        let dragged_selection = DraggedSelection {
            active_selection: SelectedEntry {
                worktree_id: worktree_a.read(cx).id(),
                entry_id: main_rs_from_a.id,
            },
            marked_selections: Arc::new([SelectedEntry {
                worktree_id: worktree_a.read(cx).id(),
                entry_id: main_rs_from_a.id,
            }]),
        };

        let result = panel.highlight_entry_for_selection_drag(
            src_dir_from_b,
            worktree_b.read(cx),
            &dragged_selection,
            cx,
        );
        assert_eq!(
            result,
            Some(src_dir_from_b.id),
            "即使相对路径相同,也应高亮来自不同工作树的目标目录"
        );

        // Test dragging file from worktree A onto file with same relative path in worktree B
        let result = panel.highlight_entry_for_selection_drag(
            main_rs_from_b,
            worktree_b.read(cx),
            &dragged_selection,
            cx,
        );
        assert_eq!(
            result,
            Some(src_dir_from_b.id),
            "应高亮来自不同工作树的目标文件的父级"
        );
    });
}

#[gpui::test]
async fn test_should_highlight_background_for_selection_drag(cx: &mut gpui::TestAppContext) {
    init_test(cx);

    let fs = FakeFs::new(cx.executor());
    fs.insert_tree(
        "/root1",
        json!({
            "parent_dir": {
                "child_file.txt": "",
                "nested_dir": {
                    "nested_file.txt": ""
                }
            },
            "root_file.txt": ""
        }),
    )
    .await;

    fs.insert_tree(
        "/root2",
        json!({
            "other_dir": {
                "other_file.txt": ""
            }
        }),
    )
    .await;

    let project = Project::test(fs.clone(), ["/root1".as_ref(), "/root2".as_ref()], cx).await;
    let window = cx.add_window(|window, cx| MultiWorkspace::test_new(project.clone(), window, cx));
    let workspace = window
        .read_with(cx, |mw, _| mw.workspace().clone())
        .unwrap();
    let cx = &mut VisualTestContext::from_window(window.into(), cx);
    let panel = workspace.update_in(cx, ProjectPanel::new);
    cx.run_until_parked();

    panel.update(cx, |panel, cx| {
        let project = panel.project.read(cx);
        let worktrees: Vec<_> = project.visible_worktrees(cx).collect();
        let worktree1 = worktrees[0].read(cx);
        let worktree2 = worktrees[1].read(cx);
        let worktree1_id = worktree1.id();
        let _worktree2_id = worktree2.id();

        let root1_entry = worktree1.root_entry().unwrap();
        let root2_entry = worktree2.root_entry().unwrap();
        let _parent_dir = worktree1.entry_for_path(rel_path("parent_dir")).unwrap();
        let child_file = worktree1
            .entry_for_path(rel_path("parent_dir/child_file.txt"))
            .unwrap();
        let nested_file = worktree1
            .entry_for_path(rel_path("parent_dir/nested_dir/nested_file.txt"))
            .unwrap();
        let root_file = worktree1.entry_for_path(rel_path("root_file.txt")).unwrap();

        // Test 1: Multiple entries - should always highlight background
        let multiple_dragged_selection = DraggedSelection {
            active_selection: SelectedEntry {
                worktree_id: worktree1_id,
                entry_id: child_file.id,
            },
            marked_selections: Arc::new([
                SelectedEntry {
                    worktree_id: worktree1_id,
                    entry_id: child_file.id,
                },
                SelectedEntry {
                    worktree_id: worktree1_id,
                    entry_id: nested_file.id,
                },
            ]),
        };

        let result = panel.should_highlight_background_for_selection_drag(
            &multiple_dragged_selection,
            root1_entry.id,
            cx,
        );
        assert!(result, "应为多个条目高亮背景");

        // Test 2: Single entry with non-empty parent path - should highlight background
        let nested_dragged_selection = DraggedSelection {
            active_selection: SelectedEntry {
                worktree_id: worktree1_id,
                entry_id: nested_file.id,
            },
            marked_selections: Arc::new([SelectedEntry {
                worktree_id: worktree1_id,
                entry_id: nested_file.id,
            }]),
        };

        let result = panel.should_highlight_background_for_selection_drag(
            &nested_dragged_selection,
            root1_entry.id,
            cx,
        );
        assert!(result, "应为嵌套文件高亮背景");

        // Test 3: Single entry at root level, same worktree - should NOT highlight background
        let root_file_dragged_selection = DraggedSelection {
            active_selection: SelectedEntry {
                worktree_id: worktree1_id,
                entry_id: root_file.id,
            },
            marked_selections: Arc::new([SelectedEntry {
                worktree_id: worktree1_id,
                entry_id: root_file.id,
            }]),
        };

        let result = panel.should_highlight_background_for_selection_drag(
            &root_file_dragged_selection,
            root1_entry.id,
            cx,
        );
        assert!(
            !result,
            "不应为同一工作树中的根文件高亮背景"
        );

        // Test 4: Single entry at root level, different worktree - should highlight background
        let result = panel.should_highlight_background_for_selection_drag(
            &root_file_dragged_selection,
            root2_entry.id,
            cx,
        );
        assert!(
            result,
            "应为来自不同工作树的根文件高亮背景"
        );

        // Test 5: Single entry in subdirectory - should highlight background
        let child_file_dragged_selection = DraggedSelection {
            active_selection: SelectedEntry {
                worktree_id: worktree1_id,
                entry_id: child_file.id,
            },
            marked_selections: Arc::new([SelectedEntry {
                worktree_id: worktree1_id,
                entry_id: child_file.id,
            }]),
        };

        let result = panel.should_highlight_background_for_selection_drag(
            &child_file_dragged_selection,
            root1_entry.id,
            cx,
        );
        assert!(
            result,
            "应为具有非空父路径的文件高亮背景"
        );
    });
}

#[gpui::test]
async fn test_hide_root(cx: &mut gpui::TestAppContext) {
    init_test(cx);

    let fs = FakeFs::new(cx.executor());
    fs.insert_tree(
        "/root1",
        json!({
            "dir1": {
                "file1.txt": "content",
                "file2.txt": "content",
            },
            "dir2": {
                "file3.txt": "content",
            },
            "file4.txt": "content",
        }),
    )
    .await;

    fs.insert_tree(
        "/root2",
        json!({
            "dir3": {
                "file5.txt": "content",
            },
            "file6.txt": "content",
        }),
    )
    .await;

    // Test 1: Single worktree with hide_root = false
    {
        let project = Project::test(fs.clone(), ["/root1".as_ref()], cx).await;
        let window =
            cx.add_window(|window, cx| MultiWorkspace::test_new(project.clone(), window, cx));
        let workspace = window
            .read_with(cx, |mw, _| mw.workspace().clone())
            .unwrap();
        let cx = &mut VisualTestContext::from_window(window.into(), cx);

        cx.update(|_, cx| {
            let settings = *ProjectPanelSettings::get_global(cx);
            ProjectPanelSettings::override_global(
                ProjectPanelSettings {
                    hide_root: false,
                    ..settings
                },
                cx,
            );
        });

        let panel = workspace.update_in(cx, ProjectPanel::new);
        cx.run_until_parked();

        #[rustfmt::skip]
        assert_eq!(
            visible_entries_as_strings(&panel, 0..10, cx),
            &[
                "v root1",
                "    > dir1",
                "    > dir2",
                "      file4.txt",
            ],
            "hide_root=false 且单个工作树时,根应可见"
        );
    }

    // Test 2: Single worktree with hide_root = true
    {
        let project = Project::test(fs.clone(), ["/root1".as_ref()], cx).await;
        let window =
            cx.add_window(|window, cx| MultiWorkspace::test_new(project.clone(), window, cx));
        let workspace = window
            .read_with(cx, |mw, _| mw.workspace().clone())
            .unwrap();
        let cx = &mut VisualTestContext::from_window(window.into(), cx);

        // Set hide_root to true
        cx.update(|_, cx| {
            let settings = *ProjectPanelSettings::get_global(cx);
            ProjectPanelSettings::override_global(
                ProjectPanelSettings {
                    hide_root: true,
                    ..settings
                },
                cx,
            );
        });

        let panel = workspace.update_in(cx, ProjectPanel::new);
        cx.run_until_parked();

        assert_eq!(
            visible_entries_as_strings(&panel, 0..10, cx),
            &["> dir1", "> dir2", "  file4.txt",],
            "hide_root=true 且单个工作树时,根应隐藏"
        );

        // Test expanding directories still works without root
        toggle_expand_dir(&panel, "root1/dir1", cx);
        assert_eq!(
            visible_entries_as_strings(&panel, 0..10, cx),
            &[
                "v dir1  <== selected",
                "      file1.txt",
                "      file2.txt",
                "> dir2",
                "  file4.txt",
            ],
            "即使根隐藏,也应能展开目录"
        );
    }

    // Test 3: Multiple worktrees with hide_root = true
    {
        let project = Project::test(fs.clone(), ["/root1".as_ref(), "/root2".as_ref()], cx).await;
        let window =
            cx.add_window(|window, cx| MultiWorkspace::test_new(project.clone(), window, cx));
        let workspace = window
            .read_with(cx, |mw, _| mw.workspace().clone())
            .unwrap();
        let cx = &mut VisualTestContext::from_window(window.into(), cx);

        // Set hide_root to true
        cx.update(|_, cx| {
            let settings = *ProjectPanelSettings::get_global(cx);
            ProjectPanelSettings::override_global(
                ProjectPanelSettings {
                    hide_root: true,
                    ..settings
                },
                cx,
            );
        });

        let panel = workspace.update_in(cx, ProjectPanel::new);
        cx.run_until_parked();

        assert_eq!(
            visible_entries_as_strings(&panel, 0..10, cx),
            &[
                "v root1",
                "    > dir1",
                "    > dir2",
                "      file4.txt",
                "v root2",
                "    > dir3",
                "      file6.txt",
            ],
            "hide_root=true 且多个工作树时,根仍应可见"
        );
    }

    // Test 4: Multiple worktrees with hide_root = false
    {
        let project = Project::test(fs.clone(), ["/root1".as_ref(), "/root2".as_ref()], cx).await;
        let window =
            cx.add_window(|window, cx| MultiWorkspace::test_new(project.clone(), window, cx));
        let workspace = window
            .read_with(cx, |mw, _| mw.workspace().clone())
            .unwrap();
        let cx = &mut VisualTestContext::from_window(window.into(), cx);

        cx.update(|_, cx| {
            let settings = *ProjectPanelSettings::get_global(cx);
            ProjectPanelSettings::override_global(
                ProjectPanelSettings {
                    hide_root: false,
                    ..settings
                },
                cx,
            );
        });

        let panel = workspace.update_in(cx, ProjectPanel::new);
        cx.run_until_parked();

        assert_eq!(
            visible_entries_as_strings(&panel, 0..10, cx),
            &[
                "v root1",
                "    > dir1",
                "    > dir2",
                "      file4.txt",
                "v root2",
                "    > dir3",
                "      file6.txt",
            ],
            "hide_root=false 且多个工作树时,根应可见"
        );
    }
}

#[gpui::test]
async fn test_compare_selected_files(cx: &mut gpui::TestAppContext) {
    init_test_with_editor(cx);

    let fs = FakeFs::new(cx.executor());
    fs.insert_tree(
        "/root",
        json!({
            "file1.txt": "file1 的内容",
            "file2.txt": "file2 的内容",
            "dir1": {
                "file3.txt": "file3 的内容"
            }
        }),
    )
    .await;

    let project = Project::test(fs.clone(), ["/root".as_ref()], cx).await;
    let window = cx.add_window(|window, cx| MultiWorkspace::test_new(project.clone(), window, cx));
    let workspace = window
        .read_with(cx, |mw, _| mw.workspace().clone())
        .unwrap();
    let cx = &mut VisualTestContext::from_window(window.into(), cx);
    let panel = workspace.update_in(cx, ProjectPanel::new);
    cx.run_until_parked();

    let file1_path = "root/file1.txt";
    let file2_path = "root/file2.txt";
    select_path_with_mark(&panel, file1_path, cx);
    select_path_with_mark(&panel, file2_path, cx);

    panel.update_in(cx, |panel, window, cx| {
        panel.compare_marked_files(&CompareMarkedFiles, window, cx);
    });
    cx.executor().run_until_parked();

    workspace.update_in(cx, |workspace, _, cx| {
        let active_items = workspace
            .panes()
            .iter()
            .filter_map(|pane| pane.read(cx).active_item())
            .collect::<Vec<_>>();
        assert_eq!(active_items.len(), 1);
        let diff_view = active_items
            .into_iter()
            .next()
            .unwrap()
            .downcast::<FileDiffView>()
            .expect("打开的项目应为 FileDiffView");
        assert_eq!(diff_view.tab_content_text(0, cx), "file1.txt ↔ file2.txt");
        assert_eq!(
            diff_view.tab_tooltip_text(cx).unwrap(),
            format!(
                "{} ↔ {}",
                rel_path(file1_path).display(PathStyle::local()),
                rel_path(file2_path).display(PathStyle::local())
            )
        );
    });

    let file1_entry_id = find_project_entry(&panel, file1_path, cx).unwrap();
    let file2_entry_id = find_project_entry(&panel, file2_path, cx).unwrap();
    let worktree_id = panel.update(cx, |panel, cx| {
        panel
            .project
            .read(cx)
            .worktrees(cx)
            .next()
            .unwrap()
            .read(cx)
            .id()
    });

    let expected_entries = [
        SelectedEntry {
            worktree_id,
            entry_id: file1_entry_id,
        },
        SelectedEntry {
            worktree_id,
            entry_id: file2_entry_id,
        },
    ];
    panel.update(cx, |panel, _cx| {
        assert_eq!(
            &panel.marked_entries, &expected_entries,
            "比较后应保留标记的条目"
        );
    });

    panel.update(cx, |panel, cx| {
        panel.project.update(cx, |_, cx| {
            cx.emit(project::Event::RevealInProjectPanel(file2_entry_id))
        })
    });

    panel.update(cx, |panel, _cx| {
        assert_eq!(
            &panel.marked_entries, &expected_entries,
            "重新聚焦项目面板后,标记的条目应保留"
        );
    });
}

#[gpui::test]
async fn test_compare_files_context_menu(cx: &mut gpui::TestAppContext) {
    init_test_with_editor(cx);

    let fs = FakeFs::new(cx.executor());
    fs.insert_tree(
        "/root",
        json!({
            "file1.txt": "file1 的内容",
            "file2.txt": "file2 的内容",
            "dir1": {},
            "dir2": {
                "file3.txt": "file3 的内容"
            }
        }),
    )
    .await;

    let project = Project::test(fs.clone(), ["/root".as_ref()], cx).await;
    let window = cx.add_window(|window, cx| MultiWorkspace::test_new(project.clone(), window, cx));
    let workspace = window
        .read_with(cx, |mw, _| mw.workspace().clone())
        .unwrap();
    let cx = &mut VisualTestContext::from_window(window.into(), cx);
    let panel = workspace.update_in(cx, ProjectPanel::new);
    cx.run_until_parked();

    // Test 1: When only one file is selected, there should be no compare option
    select_path(&panel, "root/file1.txt", cx);

    let selected_files = panel.update(cx, |panel, cx| panel.file_abs_paths_to_diff(cx));
    assert_eq!(
        selected_files, None,
        "仅选择一个文件时,不应有比较选项"
    );

    // Test 2: When multiple files are selected, there should be a compare option
    select_path_with_mark(&panel, "root/file1.txt", cx);
    select_path_with_mark(&panel, "root/file2.txt", cx);

    let selected_files = panel.update(cx, |panel, cx| panel.file_abs_paths_to_diff(cx));
    assert!(
        selected_files.is_some(),
        "应有用于比较的文件被选中"
    );
    if let Some((file1, file2)) = selected_files {
        assert!(
            file1.to_string_lossy().ends_with("file1.txt")
                && file2.to_string_lossy().ends_with("file2.txt"),
            "多选时,应选中 file1.txt 和 file2.txt"
        );
    }

    // Test 3: Selecting a directory shouldn't count as a comparable file
    select_path_with_mark(&panel, "root/dir1", cx);

    let selected_files = panel.update(cx, |panel, cx| panel.file_abs_paths_to_diff(cx));
    assert!(
        selected_files.is_some(),
        "目录选择不应影响可比较的文件"
    );
    if let Some((file1, file2)) = selected_files {
        assert!(
            file1.to_string_lossy().ends_with("file1.txt")
                && file2.to_string_lossy().ends_with("file2.txt"),
            "选择目录不应影响可比较文件的数量"
        );
    }

    // Test 4: Selecting one more file
    select_path_with_mark(&panel, "root/dir2/file3.txt", cx);

    let selected_files = panel.update(cx, |panel, cx| panel.file_abs_paths_to_diff(cx));
    assert!(
        selected_files.is_some(),
        "目录选择不应影响可比较的文件"
    );
    if let Some((file1, file2)) = selected_files {
        assert!(
            file1.to_string_lossy().ends_with("file2.txt")
                && file2.to_string_lossy().ends_with("file3.txt"),
            "选择目录不应影响可比较文件的数量"
        );
    }
}

#[gpui::test]
async fn test_reveal_in_file_manager_path_falls_back_to_worktree_root(
    cx: &mut gpui::TestAppContext,
) {
    init_test(cx);

    let fs = FakeFs::new(cx.executor());
    fs.insert_tree(
        "/root",
        json!({
            "file.txt": "content",
            "dir": {},
        }),
    )
    .await;

    let project = Project::test(fs.clone(), ["/root".as_ref()], cx).await;
    let window = cx.add_window(|window, cx| MultiWorkspace::test_new(project.clone(), window, cx));
    let workspace = window
        .read_with(cx, |mw, _| mw.workspace().clone())
        .unwrap();
    let cx = &mut VisualTestContext::from_window(window.into(), cx);
    let panel = workspace.update_in(cx, ProjectPanel::new);
    cx.run_until_parked();

    select_path(&panel, "root/file.txt", cx);
    let selected_reveal_path = panel
        .update(cx, |panel, cx| panel.reveal_in_file_manager_path(cx))
        .expect("选中的条目应生成显示路径");
    assert!(
        selected_reveal_path.ends_with(Path::new("file.txt")),
        "应为选中的文件路径,实际为 {:?}",
        selected_reveal_path
    );

    panel.update(cx, |panel, _| {
        panel.selection = None;
        panel.marked_entries.clear();
    });
    let fallback_reveal_path = panel
        .update(cx, |panel, cx| panel.reveal_in_file_manager_path(cx))
        .expect("选择为空时,应使用项目根目录");
    assert!(
        fallback_reveal_path.ends_with(Path::new("root")),
        "应为工作树根路径,实际为 {:?}",
        fallback_reveal_path
    );
}

#[gpui::test]
async fn test_hide_hidden_entries(cx: &mut gpui::TestAppContext) {
    init_test(cx);

    let fs = FakeFs::new(cx.executor());
    fs.insert_tree(
        "/root",
        json!({
            ".hidden-file.txt": "隐藏文件内容",
            "visible-file.txt": "可见文件内容",
            ".hidden-parent-dir": {
                "nested-dir": {
                    "file.txt": "文件内容",
                }
            },
            "visible-dir": {
                "file-in-visible.txt": "文件内容",
                "nested": {
                    ".hidden-nested-dir": {
                        ".double-hidden-dir": {
                            "deep-file-1.txt": "深层内容 1",
                            "deep-file-2.txt": "深层内容 2"
                        },
                        "hidden-nested-file-1.txt": "隐藏嵌套 1",
                        "hidden-nested-file-2.txt": "隐藏嵌套 2"
                    },
                    "visible-nested-file.txt": "可见嵌套内容"
                }
            }
        }),
    )
    .await;

    let project = Project::test(fs.clone(), ["/root".as_ref()], cx).await;
    let window = cx.add_window(|window, cx| MultiWorkspace::test_new(project.clone(), window, cx));
    let workspace = window
        .read_with(cx, |mw, _| mw.workspace().clone())
        .unwrap();
    let cx = &mut VisualTestContext::from_window(window.into(), cx);

    cx.update(|_, cx| {
        let settings = *ProjectPanelSettings::get_global(cx);
        ProjectPanelSettings::override_global(
            ProjectPanelSettings {
                hide_hidden: false,
                ..settings
            },
            cx,
        );
    });

    let panel = workspace.update_in(cx, ProjectPanel::new);
    cx.run_until_parked();

    toggle_expand_dir(&panel, "root/.hidden-parent-dir", cx);
    toggle_expand_dir(&panel, "root/.hidden-parent-dir/nested-dir", cx);
    toggle_expand_dir(&panel, "root/visible-dir", cx);
    toggle_expand_dir(&panel, "root/visible-dir/nested", cx);
    toggle_expand_dir(&panel, "root/visible-dir/nested/.hidden-nested-dir", cx);
    toggle_expand_dir(
        &panel,
        "root/visible-dir/nested/.hidden-nested-dir/.double-hidden-dir",
        cx,
    );

    let expanded = [
        "v root",
        "    v .hidden-parent-dir",
        "        v nested-dir",
        "              file.txt",
        "    v visible-dir",
        "        v nested",
        "            v .hidden-nested-dir",
        "                v .double-hidden-dir  <== selected",
        "                      deep-file-1.txt",
        "                      deep-file-2.txt",
        "                  hidden-nested-file-1.txt",
        "                  hidden-nested-file-2.txt",
        "              visible-nested-file.txt",
        "          file-in-visible.txt",
        "      .hidden-file.txt",
        "      visible-file.txt",
    ];

    assert_eq!(
        visible_entries_as_strings(&panel, 0..30, cx),
        &expanded,
        "hide_hidden=false 时,隐藏嵌套目录的内容应可见"
    );

    cx.update(|_, cx| {
        let settings = *ProjectPanelSettings::get_global(cx);
        ProjectPanelSettings::override_global(
            ProjectPanelSettings {
                hide_hidden: true,
                ..settings
            },
            cx,
        );
    });

    panel.update_in(cx, |panel, window, cx| {
        panel.update_visible_entries(None, false, false, window, cx);
    });
    cx.run_until_parked();

    assert_eq!(
        visible_entries_as_strings(&panel, 0..30, cx),
        &[
            "v root",
            "    v visible-dir",
            "        v nested",
            "              visible-nested-file.txt",
            "          file-in-visible.txt",
            "      visible-file.txt",
        ],
        "hide_hidden=false 时,隐藏嵌套目录的内容应可见"
    );

    panel.update_in(cx, |panel, window, cx| {
        let settings = *ProjectPanelSettings::get_global(cx);
        ProjectPanelSettings::override_global(
            ProjectPanelSettings {
                hide_hidden: false,
                ..settings
            },
            cx,
        );
        panel.update_visible_entries(None, false, false, window, cx);
    });
    cx.run_until_parked();

    assert_eq!(
        visible_entries_as_strings(&panel, 0..30, cx),
        &expanded,
        "hide_hidden=false 时,深层嵌套的隐藏目录及其内容应可见"
    );
}

pub(crate) fn select_path(panel: &Entity<ProjectPanel>, path: &str, cx: &mut VisualTestContext) {
    let path = rel_path(path);
    panel.update_in(cx, |panel, window, cx| {
        for worktree in panel.project.read(cx).worktrees(cx).collect::<Vec<_>>() {
            let worktree = worktree.read(cx);
            if let Ok(relative_path) = path.strip_prefix(worktree.root_name()) {
                let entry_id = worktree.entry_for_path(relative_path).unwrap().id;
                panel.update_visible_entries(
                    Some((worktree.id(), entry_id)),
                    false,
                    false,
                    window,
                    cx,
                );
                return;
            }
        }
        panic!("路径 {:?} 没有对应的工作树", path);
    });
    cx.run_until_parked();
}

pub(crate) fn select_path_with_mark(
    panel: &Entity<ProjectPanel>,
    path: &str,
    cx: &mut VisualTestContext,
) {
    let path = rel_path(path);
    panel.update(cx, |panel, cx| {
        for worktree in panel.project.read(cx).worktrees(cx).collect::<Vec<_>>() {
            let worktree = worktree.read(cx);
            if let Ok(relative_path) = path.strip_prefix(worktree.root_name()) {
                let entry_id = worktree.entry_for_path(relative_path).unwrap().id;
                let entry = crate::SelectedEntry {
                    worktree_id: worktree.id(),
                    entry_id,
                };
                if !panel.marked_entries.contains(&entry) {
                    panel.marked_entries.push(entry);
                }
                panel.selection = Some(entry);
                return;
            }
        }
        panic!("路径 {:?} 没有对应的工作树", path);
    });
}

/// `leaf_path` is the full path to the leaf entry (e.g., "root/a/b/c")
/// `active_ancestor_path` is the path to the folded component that should be active.
fn select_folded_path_with_mark(
    panel: &Entity<ProjectPanel>,
    leaf_path: &str,
    active_ancestor_path: &str,
    cx: &mut VisualTestContext,
) {
    select_path_with_mark(panel, leaf_path, cx);
    set_folded_active_ancestor(panel, leaf_path, active_ancestor_path, cx);
}

fn set_folded_active_ancestor(
    panel: &Entity<ProjectPanel>,
    leaf_path: &str,
    active_ancestor_path: &str,
    cx: &mut VisualTestContext,
) {
    let leaf_path = rel_path(leaf_path);
    let active_ancestor_path = rel_path(active_ancestor_path);
    panel.update(cx, |panel, cx| {
        let mut leaf_entry_id = None;
        let mut target_entry_id = None;

        for worktree in panel.project.read(cx).worktrees(cx).collect::<Vec<_>>() {
            let worktree = worktree.read(cx);
            if let Ok(relative_path) = leaf_path.strip_prefix(worktree.root_name()) {
                leaf_entry_id = worktree.entry_for_path(relative_path).map(|entry| entry.id);
            }
            if let Ok(relative_path) = active_ancestor_path.strip_prefix(worktree.root_name()) {
                target_entry_id = worktree.entry_for_path(relative_path).map(|entry| entry.id);
            }
        }

        let leaf_entry_id =
            leaf_entry_id.unwrap_or_else(|| panic!("叶子路径 {leaf_path:?} 无条目"));
        let target_entry_id = target_entry_id
            .unwrap_or_else(|| panic!("活动路径 {active_ancestor_path:?} 无条目"));
        let folded_ancestors = panel
            .state
            .ancestors
            .get_mut(&leaf_entry_id)
            .unwrap_or_else(|| panic!("叶子路径 {leaf_path:?} 应折叠"));
        let ancestor_ids = folded_ancestors.ancestors.clone();

        let mut depth_for_target = None;
        for depth in 0..ancestor_ids.len() {
            let resolved_entry_id = if depth == 0 {
                leaf_entry_id
            } else {
                ancestor_ids.get(depth).copied().unwrap_or(leaf_entry_id)
            };
            if resolved_entry_id == target_entry_id {
                depth_for_target = Some(depth);
                break;
            }
        }

        folded_ancestors.current_ancestor_depth = depth_for_target.unwrap_or_else(|| {
            panic!(
                "活动路径 {active_ancestor_path:?} 不属于折叠祖先 {ancestor_ids:?}"
            )
        });
    });
}

pub(crate) fn drag_selection_to(
    panel: &Entity<ProjectPanel>,
    target_path: &str,
    is_file: bool,
    cx: &mut VisualTestContext,
) {
    let target_entry = find_project_entry(panel, target_path, cx)
        .unwrap_or_else(|| panic!("目标路径 {target_path:?} 无条目"));

    panel.update_in(cx, |panel, window, cx| {
        let selection = panel
            .selection
            .expect("拖拽前需要先选择");
        let drag = DraggedSelection {
            active_selection: SelectedEntry {
                worktree_id: selection.worktree_id,
                entry_id: panel.resolve_entry(selection.entry_id),
            },
            marked_selections: Arc::from(panel.marked_entries.clone()),
        };
        panel.drag_onto(&drag, target_entry, is_file, window, cx);
    });
    cx.executor().run_until_parked();
}

/// Drags the entries at `source_paths` onto `target_path`. Paths are worktree
/// root names optionally followed by a path inside the worktree, e.g. "root1"
/// or "root1/dir/file.txt". The first source path is the active selection.
pub(crate) fn drag_entries_onto(
    panel: &Entity<ProjectPanel>,
    source_paths: &[&str],
    target_path: &str,
    target_is_file: bool,
    cx: &mut VisualTestContext,
) {
    let target_entry_id = find_project_entry(panel, target_path, cx)
        .unwrap_or_else(|| panic!("目标路径 {target_path:?} 无条目"));
    let selections: Vec<SelectedEntry> = source_paths
        .iter()
        .map(|path| {
            let entry_id = find_project_entry(panel, path, cx)
                .unwrap_or_else(|| panic!("源路径 {path:?} 无条目"));
            let worktree_id = panel
                .update(cx, |panel, cx| {
                    panel.project.read(cx).worktree_id_for_entry(entry_id, cx)
                })
                .unwrap_or_else(|| panic!("源路径 {path:?} 无工作树"));
            SelectedEntry {
                worktree_id,
                entry_id,
            }
        })
        .collect();
    let active_selection = *selections
        .first()
        .expect("至少需要一个源路径");

    panel.update_in(cx, |panel, window, cx| {
        let drag = DraggedSelection {
            active_selection,
            marked_selections: Arc::from(selections),
        };
        panel.drag_onto(&drag, target_entry_id, target_is_file, window, cx);
    });
    cx.executor().run_until_parked();
}

pub(crate) fn find_project_entry(
    panel: &Entity<ProjectPanel>,
    path: &str,
    cx: &mut VisualTestContext,
) -> Option<ProjectEntryId> {
    let path = rel_path(path);
    panel.update(cx, |panel, cx| {
        for worktree in panel.project.read(cx).worktrees(cx).collect::<Vec<_>>() {
            let worktree = worktree.read(cx);
            if let Ok(relative_path) = path.strip_prefix(worktree.root_name()) {
                return worktree.entry_for_path(relative_path).map(|entry| entry.id);
            }
        }
        panic!("路径 {path:?} 无工作树");
    })
}

fn visible_entries_as_strings(
    panel: &Entity<ProjectPanel>,
    range: Range<usize>,
    cx: &mut VisualTestContext,
) -> Vec<String> {
    let mut result = Vec::new();
    let mut project_entries = HashSet::default();
    let mut has_editor = false;

    panel.update_in(cx, |panel, window, cx| {
        panel.for_each_visible_entry(range, window, cx, &mut |project_entry, details, _, _| {
            if details.is_editing {
                assert!(!has_editor, "重复的编辑器条目");
                has_editor = true;
            } else {
                assert!(
                    project_entries.insert(project_entry),
                    "重复的项目条目 {:?} {:?}",
                    project_entry,
                    details
                );
            }

            let indent = "    ".repeat(details.depth);
            let icon = if details.kind.is_dir() {
                if details.is_expanded { "v " } else { "> " }
            } else {
                "  "
            };
            #[cfg(windows)]
            let filename = details.filename.replace("\\", "/");
            #[cfg(not(windows))]
            let filename = details.filename;
            let name = if details.is_editing {
                format!("[EDITOR: '{}']", filename)
            } else if details.is_processing {
                format!("[PROCESSING: '{}']", filename)
            } else {
                filename
            };
            let selected = if details.is_selected {
                "  <== selected"
            } else {
                ""
            };
            let marked = if details.is_marked {
                "  <== marked"
            } else {
                ""
            };

            result.push(format!("{indent}{icon}{name}{selected}{marked}"));
        });
    });

    result
}

/// Test that missing sort_mode field defaults to DirectoriesFirst
#[gpui::test]
async fn test_sort_mode_default_fallback(cx: &mut gpui::TestAppContext) {
    init_test(cx);

    // Verify that when sort_mode is not specified, it defaults to DirectoriesFirst
    let default_settings = cx.read(|cx| *ProjectPanelSettings::get_global(cx));
    assert_eq!(
        default_settings.sort_mode,
        settings::ProjectPanelSortMode::DirectoriesFirst,
        "sort_mode 应默认为 DirectoriesFirst"
    );
}

/// Test sort modes: DirectoriesFirst (default) vs Mixed
#[gpui::test]
async fn test_sort_mode_directories_first(cx: &mut gpui::TestAppContext) {
    init_test(cx);

    let fs = FakeFs::new(cx.executor());
    fs.insert_tree(
        "/root",
        json!({
            "zebra.txt": "",
            "Apple": {},
            "banana.rs": "",
            "Carrot": {},
            "aardvark.txt": "",
        }),
    )
    .await;

    let project = Project::test(fs.clone(), ["/root".as_ref()], cx).await;
    let window = cx.add_window(|window, cx| MultiWorkspace::test_new(project.clone(), window, cx));
    let workspace = window
        .read_with(cx, |mw, _| mw.workspace().clone())
        .unwrap();
    let cx = &mut VisualTestContext::from_window(window.into(), cx);
    let panel = workspace.update_in(cx, ProjectPanel::new);
    cx.run_until_parked();

    // Default sort mode should be DirectoriesFirst
    assert_eq!(
        visible_entries_as_strings(&panel, 0..50, cx),
        &[
            "v root",
            "    > Apple",
            "    > Carrot",
            "      aardvark.txt",
            "      banana.rs",
            "      zebra.txt",
        ]
    );
}

#[gpui::test]
async fn test_sort_mode_mixed(cx: &mut gpui::TestAppContext) {
    init_test(cx);

    let fs = FakeFs::new(cx.executor());
    fs.insert_tree(
        "/root",
        json!({
            "Zebra.txt": "",
            "apple": {},
            "Banana.rs": "",
            "carrot": {},
            "Aardvark.txt": "",
        }),
    )
    .await;

    let project = Project::test(fs.clone(), ["/root".as_ref()], cx).await;
    let window = cx.add_window(|window, cx| MultiWorkspace::test_new(project.clone(), window, cx));
    let workspace = window
        .read_with(cx, |mw, _| mw.workspace().clone())
        .unwrap();
    let cx = &mut VisualTestContext::from_window(window.into(), cx);

    // Switch to Mixed mode
    cx.update(|_, cx| {
        cx.update_global::<SettingsStore, _>(|store, cx| {
            store.update_user_settings(cx, |settings| {
                settings.project_panel.get_or_insert_default().sort_mode =
                    Some(settings::ProjectPanelSortMode::Mixed);
            });
        });
    });

    let panel = workspace.update_in(cx, ProjectPanel::new);
    cx.run_until_parked();

    // Mixed mode: case-insensitive sorting
    // Aardvark < apple < Banana < carrot < Zebra (all case-insensitive)
    assert_eq!(
        visible_entries_as_strings(&panel, 0..50, cx),
        &[
            "v root",
            "      Aardvark.txt",
            "    > apple",
            "      Banana.rs",
            "    > carrot",
            "      Zebra.txt",
        ]
    );
}

#[gpui::test]
async fn test_sort_mode_files_first(cx: &mut gpui::TestAppContext) {
    init_test(cx);

    let fs = FakeFs::new(cx.executor());
    fs.insert_tree(
        "/root",
        json!({
            "Zebra.txt": "",
            "apple": {},
            "Banana.rs": "",
            "carrot": {},
            "Aardvark.txt": "",
        }),
    )
    .await;

    let project = Project::test(fs.clone(), ["/root".as_ref()], cx).await;
    let window = cx.add_window(|window, cx| MultiWorkspace::test_new(project.clone(), window, cx));
    let workspace = window
        .read_with(cx, |mw, _| mw.workspace().clone())
        .unwrap();
    let cx = &mut VisualTestContext::from_window(window.into(), cx);

    // Switch to FilesFirst mode
    cx.update(|_, cx| {
        cx.update_global::<SettingsStore, _>(|store, cx| {
            store.update_user_settings(cx, |settings| {
                settings.project_panel.get_or_insert_default().sort_mode =
                    Some(settings::ProjectPanelSortMode::FilesFirst);
            });
        });
    });

    let panel = workspace.update_in(cx, ProjectPanel::new);
    cx.run_until_parked();

    // FilesFirst mode: files first, then directories (both case-insensitive)
    assert_eq!(
        visible_entries_as_strings(&panel, 0..50, cx),
        &[
            "v root",
            "      Aardvark.txt",
            "      Banana.rs",
            "      Zebra.txt",
            "    > apple",
            "    > carrot",
        ]
    );
}

#[gpui::test]
async fn test_sort_mode_toggle(cx: &mut gpui::TestAppContext) {
    init_test(cx);

    let fs = FakeFs::new(cx.executor());
    fs.insert_tree(
        "/root",
        json!({
            "file2.txt": "",
            "dir1": {},
            "file1.txt": "",
        }),
    )
    .await;

    let project = Project::test(fs.clone(), ["/root".as_ref()], cx).await;
    let window = cx.add_window(|window, cx| MultiWorkspace::test_new(project.clone(), window, cx));
    let workspace = window
        .read_with(cx, |mw, _| mw.workspace().clone())
        .unwrap();
    let cx = &mut VisualTestContext::from_window(window.into(), cx);
    let panel = workspace.update_in(cx, ProjectPanel::new);
    cx.run_until_parked();

    // Initially DirectoriesFirst
    assert_eq!(
        visible_entries_as_strings(&panel, 0..50, cx),
        &["v root", "    > dir1", "      file1.txt", "      file2.txt",]
    );

    // Toggle to Mixed
    cx.update(|_, cx| {
        cx.update_global::<SettingsStore, _>(|store, cx| {
            store.update_user_settings(cx, |settings| {
                settings.project_panel.get_or_insert_default().sort_mode =
                    Some(settings::ProjectPanelSortMode::Mixed);
            });
        });
    });
    cx.run_until_parked();

    assert_eq!(
        visible_entries_as_strings(&panel, 0..50, cx),
        &["v root", "    > dir1", "      file1.txt", "      file2.txt",]
    );

    // Toggle back to DirectoriesFirst
    cx.update(|_, cx| {
        cx.update_global::<SettingsStore, _>(|store, cx| {
            store.update_user_settings(cx, |settings| {
                settings.project_panel.get_or_insert_default().sort_mode =
                    Some(settings::ProjectPanelSortMode::DirectoriesFirst);
            });
        });
    });
    cx.run_until_parked();

    assert_eq!(
        visible_entries_as_strings(&panel, 0..50, cx),
        &["v root", "    > dir1", "      file1.txt", "      file2.txt",]
    );
}

#[gpui::test]
async fn test_ensure_temporary_folding_when_creating_in_different_nested_dirs(
    cx: &mut gpui::TestAppContext,
) {
    init_test(cx);

    // parent: accept
    run_create_file_in_folded_path_case(
        "parent",
        "root1/parent",
        "file_in_parent.txt",
        &[
            "v root1",
            "    v parent",
            "        > subdir/child",
            "          [EDITOR: '']  <== selected",
        ],
        &[
            "v root1",
            "    v parent",
            "        > subdir/child",
            "          file_in_parent.txt  <== selected  <== marked",
        ],
        true,
        cx,
    )
    .await;

    // parent: cancel
    run_create_file_in_folded_path_case(
        "parent",
        "root1/parent",
        "file_in_parent.txt",
        &[
            "v root1",
            "    v parent",
            "        > subdir/child",
            "          [EDITOR: '']  <== selected",
        ],
        &["v root1", "    > parent/subdir/child  <== selected"],
        false,
        cx,
    )
    .await;

    // subdir: accept
    run_create_file_in_folded_path_case(
        "subdir",
        "root1/parent/subdir",
        "file_in_subdir.txt",
        &[
            "v root1",
            "    v parent/subdir",
            "        > child",
            "          [EDITOR: '']  <== selected",
        ],
        &[
            "v root1",
            "    v parent/subdir",
            "        > child",
            "          file_in_subdir.txt  <== selected  <== marked",
        ],
        true,
        cx,
    )
    .await;

    // subdir: cancel
    run_create_file_in_folded_path_case(
        "subdir",
        "root1/parent/subdir",
        "file_in_subdir.txt",
        &[
            "v root1",
            "    v parent/subdir",
            "        > child",
            "          [EDITOR: '']  <== selected",
        ],
        &["v root1", "    > parent/subdir/child  <== selected"],
        false,
        cx,
    )
    .await;

    // child: accept
    run_create_file_in_folded_path_case(
        "child",
        "root1/parent/subdir/child",
        "file_in_child.txt",
        &[
            "v root1",
            "    v parent/subdir/child",
            "          [EDITOR: '']  <== selected",
        ],
        &[
            "v root1",
            "    v parent/subdir/child",
            "          file_in_child.txt  <== selected  <== marked",
        ],
        true,
        cx,
    )
    .await;

    // child: cancel
    run_create_file_in_folded_path_case(
        "child",
        "root1/parent/subdir/child",
        "file_in_child.txt",
        &[
            "v root1",
            "    v parent/subdir/child",
            "          [EDITOR: '']  <== selected",
        ],
        &["v root1", "    v parent/subdir/child  <== selected"],
        false,
        cx,
    )
    .await;
}

#[gpui::test]
async fn test_preserve_temporary_unfolded_active_index_on_blur_from_context_menu(
    cx: &mut gpui::TestAppContext,
) {
    init_test(cx);

    let fs = FakeFs::new(cx.executor());
    fs.insert_tree(
        "/root1",
        json!({
            "parent": {
                "subdir": {
                    "child": {},
                }
            }
        }),
    )
    .await;

    let project = Project::test(fs.clone(), ["/root1".as_ref()], cx).await;
    let window = cx.add_window(|window, cx| MultiWorkspace::test_new(project.clone(), window, cx));
    let workspace = window
        .read_with(cx, |mw, _| mw.workspace().clone())
        .unwrap();
    let cx = &mut VisualTestContext::from_window(window.into(), cx);

    let panel = workspace.update_in(cx, |workspace, window, cx| {
        let panel = ProjectPanel::new(workspace, window, cx);
        workspace.add_panel(panel.clone(), window, cx);
        panel
    });

    cx.update(|_, cx| {
        let settings = *ProjectPanelSettings::get_global(cx);
        ProjectPanelSettings::override_global(
            ProjectPanelSettings {
                auto_fold_dirs: true,
                ..settings
            },
            cx,
        );
    });

    panel.update_in(cx, |panel, window, cx| {
        panel.collapse_all_entries(&CollapseAllEntries, window, cx);
    });
    cx.run_until_parked();

    select_folded_path_with_mark(
        &panel,
        "root1/parent/subdir/child",
        "root1/parent/subdir",
        cx,
    );
    panel.update(cx, |panel, _| {
        panel.marked_entries.clear();
    });

    let parent_entry_id = find_project_entry(&panel, "root1/parent", cx)
        .expect("父目录应存在于本次测试中");
    let subdir_entry_id = find_project_entry(&panel, "root1/parent/subdir", cx)
        .expect("子目录应存在于本次测试中");
    let child_entry_id = find_project_entry(&panel, "root1/parent/subdir/child", cx)
        .expect("子目录的子目录应存在于本次测试中");

    panel.update(cx, |panel, _| {
        let selection = panel
            .selection
            .expect("创建新条目前应选中叶子目录");
        assert_eq!(
            selection.entry_id, child_entry_id,
            "初始选中项应为折叠的叶子条目"
        );
        assert_eq!(
            panel.resolve_entry(selection.entry_id),
            subdir_entry_id,
            "活动折叠组件应从 subdir 开始"
        );
    });

    panel.update_in(cx, |panel, window, cx| {
        panel.deploy_context_menu(
            gpui::point(gpui::px(1.), gpui::px(1.)),
            child_entry_id,
            window,
            cx,
        );
        panel.new_file(&NewFile, window, cx);
    });
    cx.run_until_parked();
    panel.update_in(cx, |panel, window, cx| {
        assert!(panel.filename_editor.read(cx).is_focused(window));
    });
    cx.run_until_parked();

    set_folded_active_ancestor(&panel, "root1/parent/subdir", "root1/parent", cx);

    panel.update_in(cx, |panel, window, cx| {
        panel.deploy_context_menu(
            gpui::point(gpui::px(2.), gpui::px(2.)),
            subdir_entry_id,
            window,
            cx,
        );
    });
    cx.run_until_parked();

    panel.update(cx, |panel, _| {
        assert!(
            panel.state.edit_state.is_none(),
            "打开另一个上下文菜单应使文件名编辑器失焦并丢弃编辑状态"
        );
        let selection = panel
            .selection
            .expect("选中项应恢复到之前聚焦的叶子条目");
        assert_eq!(
            selection.entry_id, child_entry_id,
            "由失焦驱动的取消应恢复之前的叶子选中项"
        );
        assert_eq!(
            panel.resolve_entry(selection.entry_id),
            parent_entry_id,
            "临时展开的待处理状态应保留失焦前选择的活动祖先"
        );
    });

    panel.update_in(cx, |panel, window, cx| {
        panel.new_file(&NewFile, window, cx);
    });
    cx.run_until_parked();
    assert_eq!(
        visible_entries_as_strings(&panel, 0..10, cx),
        &[
            "v root1",
            "    v parent",
            "        > subdir/child",
            "          [EDITOR: '']  <== selected",
        ],
        "失焦后的新文件应使用保留的活动祖先"
    );
    panel.update(cx, |panel, _| {
        let edit_state = panel
            .state
            .edit_state
            .as_ref()
            .expect("新文件应进入编辑状态");
        assert_eq!(
            edit_state.temporarily_unfolded,
            Some(parent_entry_id),
            "恢复活动祖先后的临时展开应针对父目录"
        );
    });

    let file_name = "created_after_blur.txt";
    panel
        .update_in(cx, |panel, window, cx| {
            panel.filename_editor.update(cx, |editor, cx| {
                editor.set_text(file_name, window, cx);
            });
            panel.confirm_edit(true, window, cx).expect(
                "confirm_edit 应为失焦转换后创建的文件启动创建流程",
            )
        })
        .await
        .expect("失焦转换后创建文件应成功");
    cx.run_until_parked();

    assert!(
        fs.is_file(Path::new("/root1/parent/created_after_blur.txt"))
            .await,
        "活动祖先恢复为父目录后,文件应创建在父目录下"
    );
    assert!(
        !fs.is_file(Path::new("/root1/parent/subdir/created_after_blur.txt"))
            .await,
        "当父目录为活动祖先时,文件不应创建在 subdir 下"
    );
}

async fn run_create_file_in_folded_path_case(
    case_name: &str,
    active_ancestor_path: &str,
    created_file_name: &str,
    expected_temporary_state: &[&str],
    expected_final_state: &[&str],
    accept_creation: bool,
    cx: &mut gpui::TestAppContext,
) {
    let expected_collapsed_state = &["v root1", "    > parent/subdir/child  <== selected"];

    let fs = FakeFs::new(cx.executor());
    fs.insert_tree(
        "/root1",
        json!({
            "parent": {
                "subdir": {
                    "child": {},
                }
            }
        }),
    )
    .await;

    let project = Project::test(fs.clone(), ["/root1".as_ref()], cx).await;
    let window = cx.add_window(|window, cx| MultiWorkspace::test_new(project.clone(), window, cx));
    let workspace = window
        .read_with(cx, |mw, _| mw.workspace().clone())
        .unwrap();
    let cx = &mut VisualTestContext::from_window(window.into(), cx);

    let panel = workspace.update_in(cx, |workspace, window, cx| {
        let panel = ProjectPanel::new(workspace, window, cx);
        workspace.add_panel(panel.clone(), window, cx);
        panel
    });

    cx.update(|_, cx| {
        let settings = *ProjectPanelSettings::get_global(cx);
        ProjectPanelSettings::override_global(
            ProjectPanelSettings {
                auto_fold_dirs: true,
                ..settings
            },
            cx,
        );
    });

    panel.update_in(cx, |panel, window, cx| {
        panel.collapse_all_entries(&CollapseAllEntries, window, cx);
    });
    cx.run_until_parked();

    select_folded_path_with_mark(
        &panel,
        "root1/parent/subdir/child",
        active_ancestor_path,
        cx,
    );
    panel.update(cx, |panel, _| {
        panel.marked_entries.clear();
    });

    assert_eq!(
        visible_entries_as_strings(&panel, 0..10, cx),
        expected_collapsed_state,
        "用例 '{}' 应从折叠状态开始",
        case_name
    );

    panel.update_in(cx, |panel, window, cx| {
        panel.new_file(&NewFile, window, cx);
    });
    cx.run_until_parked();
    panel.update_in(cx, |panel, window, cx| {
        assert!(panel.filename_editor.read(cx).is_focused(window));
    });
    cx.run_until_parked();
    assert_eq!(
        visible_entries_as_strings(&panel, 0..10, cx),
        expected_temporary_state,
        "用例 '{}' ({}) 在编辑时应临时展开活动祖先",
        case_name,
        if accept_creation { "accept" } else { "cancel" }
    );

    let relative_directory = active_ancestor_path
        .strip_prefix("root1/")
        .expect("active_ancestor_path 应以 root1/ 开头");
    let created_file_path = PathBuf::from("/root1")
        .join(relative_directory)
        .join(created_file_name);

    if accept_creation {
        panel
            .update_in(cx, |panel, window, cx| {
                panel.filename_editor.update(cx, |editor, cx| {
                    editor.set_text(created_file_name, window, cx);
                });
                panel.confirm_edit(true, window, cx).unwrap()
            })
            .await
            .unwrap();
        cx.run_until_parked();

        assert_eq!(
            visible_entries_as_strings(&panel, 0..10, cx),
            expected_final_state,
            "用例 '{}' 在确认后应保持新创建的文件被选中并标记",
            case_name
        );
        assert!(
            fs.is_file(created_file_path.as_path()).await,
            "用例 '{}' 应创建文件 '{}'",
            case_name,
            created_file_path.display()
        );
    } else {
        panel.update_in(cx, |panel, window, cx| {
            panel.cancel(&Cancel, window, cx);
        });
        cx.run_until_parked();

        assert_eq!(
            visible_entries_as_strings(&panel, 0..10, cx),
            expected_final_state,
            "用例 '{}' 在取消后应保持预期的面板状态",
            case_name
        );
        assert!(
            !fs.is_file(created_file_path.as_path()).await,
            "用例 '{}' 在取消后不应创建文件",
            case_name
        );
    }
}

#[gpui::test]
async fn test_focus_follows_mouse_into_blank_area(cx: &mut gpui::TestAppContext) {
    init_test_with_editor(cx);
    cx.update(|cx| {
        cx.update_global::<SettingsStore, _>(|store, cx| {
            store.update_user_settings(cx, |settings| {
                settings.workspace.focus_follows_mouse = Some(settings::FocusFollowsMouse {
                    enabled: Some(true),
                    debounce_ms: Some(100),
                });
            });
        });
    });

    let fs = FakeFs::new(cx.executor());
    fs.insert_tree(path!("/root"), json!({ "a.txt": "" })).await;

    let project = Project::test(fs.clone(), [path!("/root").as_ref()], cx).await;
    let window = cx.add_window(|window, cx| MultiWorkspace::test_new(project.clone(), window, cx));
    let workspace = window
        .read_with(cx, |mw, _| mw.workspace().clone())
        .unwrap();
    let cx = &mut VisualTestContext::from_window(window.into(), cx);
    let panel = workspace.update_in(cx, |workspace, window, cx| {
        let panel = ProjectPanel::new(workspace, window, cx);
        workspace.add_panel(panel.clone(), window, cx);
        workspace.open_panel::<ProjectPanel>(window, cx);
        panel
    });
    cx.run_until_parked();

    workspace
        .update_in(cx, |workspace, window, cx| {
            let worktree_id = workspace.worktrees(cx).next().unwrap().read(cx).id();
            let project_path = ProjectPath {
                worktree_id,
                path: rel_path("a.txt").into(),
            };
            workspace.open_path(project_path, None, true, window, cx)
        })
        .await
        .unwrap();
    cx.run_until_parked();

    panel.update_in(cx, |panel, window, cx| {
        assert!(
            !panel.focus_handle(cx).is_focused(window),
            "打开文件后编辑器应获得焦点"
        );
    });

    // Hover over the blank space below the last entry in the project panel,
    // which lives in the right dock by default.
    cx.simulate_mouse_move(point(px(1800.), px(600.)), None, Modifiers::none());
    cx.executor().advance_clock(Duration::from_millis(200));
    cx.run_until_parked();

    panel.update_in(cx, |panel, window, cx| {
        assert!(
            panel.focus_handle(cx).is_focused(window),
            "悬停条目下方空白区域后项目面板应获得焦点"
        );
    });
}

pub(crate) fn init_test(cx: &mut TestAppContext) {
    cx.update(|cx| {
        let settings_store = SettingsStore::test(cx);
        cx.set_global(settings_store);
        theme_settings::init(theme::LoadThemes::JustBase, cx);
        crate::init(cx);

        cx.update_global::<SettingsStore, _>(|store, cx| {
            store.update_user_settings(cx, |settings| {
                settings
                    .project_panel
                    .get_or_insert_default()
                    .auto_fold_dirs = Some(false);
                settings.project.worktree.file_scan_exclusions = Some(Vec::new());
            });
        });
    });
}

fn init_test_with_editor(cx: &mut TestAppContext) {
    cx.update(|cx| {
        let app_state = AppState::test(cx);
        theme_settings::init(theme::LoadThemes::JustBase, cx);
        editor::init(cx);
        crate::init(cx);
        workspace::init(app_state, cx);

        cx.update_global::<SettingsStore, _>(|store, cx| {
            store.update_user_settings(cx, |settings| {
                settings
                    .project_panel
                    .get_or_insert_default()
                    .auto_fold_dirs = Some(false);
                settings.project.worktree.file_scan_exclusions = Some(Vec::new())
            });
        });
    });
}

fn init_test_with_git_ui(cx: &mut TestAppContext) {
    cx.update(|cx| {
        let app_state = AppState::test(cx);
        theme_settings::init(theme::LoadThemes::JustBase, cx);
        editor::init(cx);
        git_ui::init(cx);
        crate::init(cx);
        workspace::init(app_state, cx);

        cx.update_global::<SettingsStore, _>(|store, cx| {
            store.update_user_settings(cx, |settings| {
                settings
                    .project_panel
                    .get_or_insert_default()
                    .auto_fold_dirs = Some(false);
                settings.project.worktree.file_scan_exclusions = Some(Vec::new())
            });
        });
    });
}

fn set_auto_open_settings(
    cx: &mut TestAppContext,
    auto_open_settings: ProjectPanelAutoOpenSettings,
) {
    cx.update(|cx| {
        cx.update_global::<SettingsStore, _>(|store, cx| {
            store.update_user_settings(cx, |settings| {
                settings.project_panel.get_or_insert_default().auto_open = Some(auto_open_settings);
            });
        })
    });
}

fn ensure_single_file_is_opened(
    workspace: &Entity<Workspace>,
    expected_path: &str,
    cx: &mut VisualTestContext,
) {
    workspace.update_in(cx, |workspace, _, cx| {
        let worktrees = workspace.worktrees(cx).collect::<Vec<_>>();
        assert_eq!(worktrees.len(), 1);
        let worktree_id = worktrees[0].read(cx).id();

        let open_project_paths = workspace
            .panes()
            .iter()
            .filter_map(|pane| pane.read(cx).active_item()?.project_path(cx))
            .collect::<Vec<_>>();
        assert_eq!(
            open_project_paths,
            vec![ProjectPath {
                worktree_id,
                path: Arc::from(rel_path(expected_path))
            }],
            "应已打开文件并在项目面板中选中"
        );
    });
}

fn submit_deletion(panel: &Entity<ProjectPanel>, cx: &mut VisualTestContext) {
    assert!(
        !cx.has_pending_prompt(),
        "删除前不应有提示"
    );
    panel.update_in(cx, |panel, window, cx| {
        panel.delete(&Delete { skip_prompt: false }, window, cx)
    });
    assert!(
        cx.has_pending_prompt(),
        "删除后应有一个提示"
    );
    cx.simulate_prompt_answer("删除");
    assert!(
        !cx.has_pending_prompt(),
        "提示被回复后不应再有提示"
    );
    cx.executor().run_until_parked();
}

fn submit_deletion_skipping_prompt(panel: &Entity<ProjectPanel>, cx: &mut VisualTestContext) {
    assert!(
        !cx.has_pending_prompt(),
        "删除前不应有提示"
    );
    panel.update_in(cx, |panel, window, cx| {
        panel.delete(&Delete { skip_prompt: true }, window, cx)
    });
    assert!(!cx.has_pending_prompt(), "不应收到任何提示");
    cx.executor().run_until_parked();
}

fn ensure_no_open_items_and_panes(workspace: &Entity<Workspace>, cx: &mut VisualTestContext) {
    assert!(
        !cx.has_pending_prompt(),
        "删除操作关闭文件后不应再有提示"
    );
    workspace.update_in(cx, |workspace, _window, cx| {
        let open_project_paths = workspace
            .panes()
            .iter()
            .filter_map(|pane| pane.read(cx).active_item()?.project_path(cx))
            .collect::<Vec<_>>();
        assert!(
            open_project_paths.is_empty(),
            "已删除文件的缓冲区应已关闭,但仍有打开的文件:{open_project_paths:?}"
        );
    });
}

pub(crate) struct TestProjectItemView {
    focus_handle: FocusHandle,
    path: ProjectPath,
}

pub(crate) struct TestProjectItem {
    path: ProjectPath,
}

impl project::ProjectItem for TestProjectItem {
    fn try_open(
        _project: &Entity<Project>,
        path: &ProjectPath,
        cx: &mut App,
    ) -> Option<Task<anyhow::Result<Entity<Self>>>> {
        let path = path.clone();
        Some(cx.spawn(async move |cx| Ok(cx.new(|_| Self { path }))))
    }

    fn entry_id(&self, _: &App) -> Option<ProjectEntryId> {
        None
    }

    fn project_path(&self, _: &App) -> Option<ProjectPath> {
        Some(self.path.clone())
    }

    fn is_dirty(&self) -> bool {
        false
    }
}

impl ProjectItem for TestProjectItemView {
    type Item = TestProjectItem;

    fn for_project_item(
        _: Entity<Project>,
        _: Option<&Pane>,
        project_item: Entity<Self::Item>,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self
    where
        Self: Sized,
    {
        Self {
            path: project_item.update(cx, |project_item, _| project_item.path.clone()),
            focus_handle: cx.focus_handle(),
        }
    }
}

impl Item for TestProjectItemView {
    type Event = ();

    fn tab_content_text(&self, _detail: usize, _cx: &App) -> SharedString {
        "Test".into()
    }
}

impl EventEmitter<()> for TestProjectItemView {}

impl Focusable for TestProjectItemView {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for TestProjectItemView {
    fn render(&mut self, _window: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        Empty
    }
}

#[gpui::test]
async fn test_delete_prompt_escapes_markdown_in_file_name(cx: &mut gpui::TestAppContext) {
    init_test(cx);

    let fs = FakeFs::new(cx.executor());
    fs.insert_tree(
        "/root",
        json!({
            "__somefile__": "",
        }),
    )
    .await;

    let project = Project::test(fs.clone(), ["/root".as_ref()], cx).await;
    let window = cx.add_window(|window, cx| MultiWorkspace::test_new(project.clone(), window, cx));
    let workspace = window
        .read_with(cx, |mw, _| mw.workspace().clone())
        .unwrap();
    let cx = &mut VisualTestContext::from_window(window.into(), cx);
    let panel = workspace.update_in(cx, ProjectPanel::new);
    cx.run_until_parked();

    select_path(&panel, "root/__somefile__", cx);
    panel.update_in(cx, |panel, window, cx| {
        panel.delete(&Delete { skip_prompt: false }, window, cx)
    });
    let (message, _detail) = cx
        .pending_prompt()
        .expect("删除应显示确认提示");

    assert_eq!(
        message,
        "确定要永久删除 `__somefile__` 吗?"
    );
}

#[gpui::test]
async fn test_restore_file_prompt_escapes_markdown_in_file_name(cx: &mut gpui::TestAppContext) {
    init_test(cx);

    let fs = FakeFs::new(cx.executor());
    fs.insert_tree(
        path!("/root"),
        json!({
            ".git": {},
            "__init__.py": "修改后的内容",
        }),
    )
    .await;
    fs.set_head_and_index_for_repo(
        path!("/root/.git").as_ref(),
        &[("__init__.py", "原始内容".into())],
    );

    let project = Project::test(fs.clone(), [path!("/root").as_ref()], cx).await;
    let window = cx.add_window(|window, cx| MultiWorkspace::test_new(project.clone(), window, cx));
    let workspace = window
        .read_with(cx, |mw, _| mw.workspace().clone())
        .unwrap();
    let cx = &mut VisualTestContext::from_window(window.into(), cx);
    let panel = workspace.update_in(cx, ProjectPanel::new);
    cx.run_until_parked();

    select_path(&panel, "root/__init__.py", cx);
    panel.update_in(cx, |panel, window, cx| {
        panel.restore_file(&git::RestoreFile { skip_prompt: false }, window, cx)
    });
    let (message, _detail) = cx
        .pending_prompt()
        .expect("恢复应显示确认提示");

    assert_eq!(message, "放弃对 `__init__.py` 的更改?");
}
