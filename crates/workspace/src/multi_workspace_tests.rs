use std::path::PathBuf;

use super::*;
use crate::item::test::TestItem;
use agent_settings::AgentSettings;
use client::proto;
use fs::{FakeFs, Fs};
use gpui::{TestAppContext, VisualTestContext};
use project::DisableAiSettings;
use serde_json::json;
use settings::{Settings, SettingsStore};
use util::path;

fn init_test(cx: &mut TestAppContext) {
    cx.update(|cx| {
        let settings_store = SettingsStore::test(cx);
        cx.set_global(settings_store);
        theme_settings::init(theme::LoadThemes::JustBase, cx);
        DisableAiSettings::register(cx);
    });
}

fn setup_multi_workspace<'a>(
    projects: &[Entity<Project>],
    cx: &'a mut TestAppContext,
) -> (Entity<MultiWorkspace>, &'a mut VisualTestContext) {
    let mut iterator = projects.iter();
    let project = iterator
        .next()
        .expect("至少应提供一个项目")
        .clone();

    let (multi_workspace, cx) =
        cx.add_window_view(|window, cx| MultiWorkspace::test_new(project, window, cx));

    for project in iterator {
        multi_workspace.update_in(cx, |multi_workspace, window, cx| {
            multi_workspace.test_add_workspace(project.clone(), window, cx);
        })
    }

    // Opening the sidebar retains the workspaces and establishes their project groups.
    multi_workspace.update(cx, |multi_workspace, cx| multi_workspace.open_sidebar(cx));
    cx.run_until_parked();

    (multi_workspace, cx)
}

#[gpui::test]
async fn test_sidebar_disabled_when_disable_ai_is_enabled(cx: &mut TestAppContext) {
    init_test(cx);
    let fs = FakeFs::new(cx.executor());
    let project = Project::test(fs, [], cx).await;

    let (multi_workspace, cx) =
        cx.add_window_view(|window, cx| MultiWorkspace::test_new(project, window, cx));

    multi_workspace.read_with(cx, |mw, cx| {
        assert!(mw.multi_workspace_enabled(cx));
    });

    multi_workspace.update_in(cx, |mw, _window, cx| {
        mw.open_sidebar(cx);
        assert!(mw.sidebar_open());
    });

    cx.update(|_window, cx| {
        DisableAiSettings::override_global(DisableAiSettings { disable_ai: true }, cx);
    });
    cx.run_until_parked();

    multi_workspace.read_with(cx, |mw, cx| {
        assert!(
            !mw.sidebar_open(),
            "当 disable_ai 为 true 时,侧边栏应处于关闭状态"
        );
        assert!(
            !mw.multi_workspace_enabled(cx),
            "当 disable_ai 为 true 时,多工作区应被禁用"
        );
    });

    multi_workspace.update_in(cx, |mw, window, cx| {
        mw.toggle_sidebar(window, cx);
    });
    multi_workspace.read_with(cx, |mw, _cx| {
        assert!(
            !mw.sidebar_open(),
            "当 disable_ai 为 true 时切换侧边栏,其应保持关闭状态"
        );
    });

    cx.update(|_window, cx| {
        DisableAiSettings::override_global(DisableAiSettings { disable_ai: false }, cx);
    });
    cx.run_until_parked();

    multi_workspace.read_with(cx, |mw, cx| {
        assert!(
            mw.multi_workspace_enabled(cx),
            "重新启用 AI 后,多工作区应被启用"
        );
        assert!(
            !mw.sidebar_open(),
            "重新启用 AI 后,侧边栏仍应处于关闭状态(不会自动打开)"
        );
    });

    multi_workspace.update_in(cx, |mw, window, cx| {
        mw.toggle_sidebar(window, cx);
    });
    multi_workspace.read_with(cx, |mw, _cx| {
        assert!(
            mw.sidebar_open(),
            "重新启用 AI 后切换侧边栏,其应打开"
        );
    });
}

#[gpui::test]
async fn test_multi_workspace_collapses_when_agent_is_disabled(cx: &mut TestAppContext) {
    init_test(cx);
    let fs = FakeFs::new(cx.executor());
    fs.insert_tree("/root_a", json!({ "file.txt": "" })).await;
    fs.insert_tree("/root_b", json!({ "file.txt": "" })).await;
    let project_a = Project::test(fs.clone(), ["/root_a".as_ref()], cx).await;
    let project_b = Project::test(fs, ["/root_b".as_ref()], cx).await;

    let (multi_workspace, cx) =
        cx.add_window_view(|window, cx| MultiWorkspace::test_new(project_a, window, cx));

    multi_workspace.update_in(cx, |multi_workspace, window, cx| {
        multi_workspace.test_add_workspace(project_b, window, cx);
    });
    cx.run_until_parked();

    multi_workspace.read_with(cx, |multi_workspace, cx| {
        assert!(multi_workspace.multi_workspace_enabled(cx));
        assert_eq!(multi_workspace.workspaces().count(), 2);
    });

    cx.update(|_window, cx| {
        let mut settings = AgentSettings::get_global(cx).clone();
        settings.enabled = false;
        AgentSettings::override_global(settings, cx);
    });
    cx.run_until_parked();

    multi_workspace.read_with(cx, |multi_workspace, cx| {
        assert!(!multi_workspace.multi_workspace_enabled(cx));
        assert!(!multi_workspace.sidebar_open());
        assert_eq!(multi_workspace.workspaces().count(), 1);
        assert!(multi_workspace.project_group_keys().is_empty());
    });
}

#[gpui::test]
async fn test_project_group_keys_initial(cx: &mut TestAppContext) {
    init_test(cx);
    let fs = FakeFs::new(cx.executor());
    fs.insert_tree("/root_a", json!({ "file.txt": "" })).await;
    let project = Project::test(fs, ["/root_a".as_ref()], cx).await;

    let expected_key = project.read_with(cx, |project, cx| project.project_group_key(cx));

    let (multi_workspace, cx) =
        cx.add_window_view(|window, cx| MultiWorkspace::test_new(project, window, cx));

    multi_workspace.update(cx, |mw, cx| {
        mw.open_sidebar(cx);
    });

    multi_workspace.read_with(cx, |mw, _cx| {
        let keys: Vec<ProjectGroupKey> = mw.project_group_keys();
        assert_eq!(keys.len(), 1, "创建时应该恰好有一个键");
        assert_eq!(keys[0], expected_key);
    });
}

#[gpui::test]
async fn test_project_group_keys_add_workspace(cx: &mut TestAppContext) {
    init_test(cx);
    let fs = FakeFs::new(cx.executor());
    fs.insert_tree("/root_a", json!({ "file.txt": "" })).await;
    fs.insert_tree("/root_b", json!({ "file.txt": "" })).await;
    let project_a = Project::test(fs.clone(), ["/root_a".as_ref()], cx).await;
    let project_b = Project::test(fs.clone(), ["/root_b".as_ref()], cx).await;

    let key_a = project_a.read_with(cx, |p, cx| p.project_group_key(cx));
    let key_b = project_b.read_with(cx, |p, cx| p.project_group_key(cx));
    assert_ne!(
        key_a, key_b,
        "different roots should produce different keys"
    );

    let (multi_workspace, cx) =
        cx.add_window_view(|window, cx| MultiWorkspace::test_new(project_a, window, cx));

    multi_workspace.update(cx, |mw, cx| {
        mw.open_sidebar(cx);
    });

    multi_workspace.read_with(cx, |mw, _cx| {
        assert_eq!(mw.project_group_keys().len(), 1);
    });

    // Adding a workspace with a different project root adds a new key.
    multi_workspace.update_in(cx, |mw, window, cx| {
        mw.test_add_workspace(project_b, window, cx);
    });

    multi_workspace.read_with(cx, |mw, _cx| {
        let keys: Vec<ProjectGroupKey> = mw.project_group_keys();
        assert_eq!(
            keys.len(),
            2,
            "添加第二个工作区后应有两个键"
        );
        assert_eq!(keys[0], key_b);
        assert_eq!(keys[1], key_a);
    });
}

#[gpui::test]
async fn test_open_new_window_does_not_open_sidebar_on_existing_window(cx: &mut TestAppContext) {
    init_test(cx);

    let app_state = cx.update(AppState::test);
    let fs = app_state.fs.as_fake();
    fs.insert_tree(path!("/project_a"), json!({ "file.txt": "" }))
        .await;
    fs.insert_tree(path!("/project_b"), json!({ "file.txt": "" }))
        .await;

    let project = Project::test(app_state.fs.clone(), [path!("/project_a").as_ref()], cx).await;

    let window = cx.add_window(|window, cx| MultiWorkspace::test_new(project, window, cx));

    window
        .read_with(cx, |mw, _cx| {
            assert!(!mw.sidebar_open(), "侧边栏应初始为关闭状态",);
        })
        .unwrap();

    cx.update(|cx| {
        open_paths(
            &[PathBuf::from(path!("/project_b"))],
            app_state,
            OpenOptions {
                open_mode: OpenMode::NewWindow,
                ..OpenOptions::default()
            },
            cx,
        )
    })
    .await
    .unwrap();

    window
        .read_with(cx, |mw, _cx| {
            assert!(
                !mw.sidebar_open(),
                "opening a project in a new window must not open the sidebar on the original window",
            );
        })
        .unwrap();
}

#[gpui::test]
async fn test_open_directory_in_empty_workspace_does_not_open_sidebar(cx: &mut TestAppContext) {
    init_test(cx);

    let app_state = cx.update(AppState::test);
    let fs = app_state.fs.as_fake();
    fs.insert_tree(path!("/project"), json!({ "file.txt": "" }))
        .await;

    let project = Project::test(app_state.fs.clone(), [], cx).await;
    let window = cx.add_window(|window, cx| {
        let mw = MultiWorkspace::test_new(project, window, cx);
        // Simulate a blank project that has an untitled editor tab,
        // so that workspace_windows_for_location finds this window.
        mw.workspace().update(cx, |workspace, cx| {
            workspace.active_pane().update(cx, |pane, cx| {
                let item = cx.new(|cx| item::test::TestItem::new(cx));
                pane.add_item(Box::new(item), false, false, None, window, cx);
            });
        });
        mw
    });

    window
        .read_with(cx, |mw, _cx| {
            assert!(!mw.sidebar_open(), "侧边栏应初始为关闭状态");
        })
        .unwrap();

    // Simulate what open_workspace_for_paths does for an empty workspace:
    // it downgrades OpenMode::NewWindow to Activate and sets requesting_window.
    cx.update(|cx| {
        open_paths(
            &[PathBuf::from(path!("/project"))],
            app_state,
            OpenOptions {
                requesting_window: Some(window),
                open_mode: OpenMode::Activate,
                ..OpenOptions::default()
            },
            cx,
        )
    })
    .await
    .unwrap();

    window
        .read_with(cx, |mw, _cx| {
            assert!(
                !mw.sidebar_open(),
                "通过文件选择器在空白项目中打开目录不得打开侧边栏",
            );
        })
        .unwrap();
}

#[gpui::test]
async fn test_project_group_keys_duplicate_not_added(cx: &mut TestAppContext) {
    init_test(cx);
    let fs = FakeFs::new(cx.executor());
    fs.insert_tree("/root_a", json!({ "file.txt": "" })).await;
    let project_a = Project::test(fs.clone(), ["/root_a".as_ref()], cx).await;
    // A second project entity pointing at the same path produces the same key.
    let project_a2 = Project::test(fs.clone(), ["/root_a".as_ref()], cx).await;

    let key_a = project_a.read_with(cx, |p, cx| p.project_group_key(cx));
    let key_a2 = project_a2.read_with(cx, |p, cx| p.project_group_key(cx));
    assert_eq!(key_a, key_a2, "相同的根路径应产生相同的键");

    let (multi_workspace, cx) =
        cx.add_window_view(|window, cx| MultiWorkspace::test_new(project_a, window, cx));

    multi_workspace.update(cx, |mw, cx| {
        mw.open_sidebar(cx);
    });

    multi_workspace.update_in(cx, |mw, window, cx| {
        mw.test_add_workspace(project_a2, window, cx);
    });

    multi_workspace.read_with(cx, |mw, _cx| {
        let keys: Vec<ProjectGroupKey> = mw.project_group_keys();
        assert_eq!(
            keys.len(),
            1,
            "插入具有相同根路径的工作区时,不应添加重复的键"
        );
    });
}

#[gpui::test]
async fn test_adding_worktree_updates_project_group_key(cx: &mut TestAppContext) {
    init_test(cx);
    let fs = FakeFs::new(cx.executor());
    fs.insert_tree("/root_a", json!({ "file.txt": "" })).await;
    fs.insert_tree("/root_b", json!({ "other.txt": "" })).await;
    let project = Project::test(fs.clone(), ["/root_a".as_ref()], cx).await;

    let initial_key = project.read_with(cx, |p, cx| p.project_group_key(cx));

    let (multi_workspace, cx) =
        cx.add_window_view(|window, cx| MultiWorkspace::test_new(project.clone(), window, cx));

    // Open sidebar to retain the workspace and create the initial group.
    multi_workspace.update(cx, |mw, cx| {
        mw.open_sidebar(cx);
    });
    cx.run_until_parked();

    multi_workspace.read_with(cx, |mw, _cx| {
        let keys = mw.project_group_keys();
        assert_eq!(keys.len(), 1);
        assert_eq!(keys[0], initial_key);
    });

    // Add a second worktree to the project. This triggers WorktreeAdded →
    // handle_workspace_key_change, which should update the group key.
    project
        .update(cx, |project, cx| {
            project.find_or_create_worktree("/root_b", true, cx)
        })
        .await
        .expect("adding worktree should succeed");
    cx.run_until_parked();

    let updated_key = project.read_with(cx, |p, cx| p.project_group_key(cx));
    assert_ne!(
        initial_key, updated_key,
        "添加工作树应更改项目组键"
    );

    multi_workspace.read_with(cx, |mw, _cx| {
        let keys = mw.project_group_keys();
        assert!(
            keys.contains(&updated_key),
            "应包含更新后的键;实际为 {keys:?}"
        );
    });
}

#[gpui::test]
async fn test_find_or_create_local_workspace_reuses_active_workspace_when_sidebar_closed(
    cx: &mut TestAppContext,
) {
    init_test(cx);
    let fs = FakeFs::new(cx.executor());
    fs.insert_tree("/root_a", json!({ "file.txt": "" })).await;
    let project = Project::test(fs, ["/root_a".as_ref()], cx).await;

    let (multi_workspace, cx) =
        cx.add_window_view(|window, cx| MultiWorkspace::test_new(project, window, cx));

    let active_workspace = multi_workspace.read_with(cx, |mw, cx| {
        assert!(
            mw.project_groups(cx).is_empty(),
            "侧边栏关闭的设置应初始不保留任何项目组"
        );
        mw.workspace().clone()
    });
    let active_workspace_id = active_workspace.entity_id();

    let workspace = multi_workspace
        .update_in(cx, |mw, window, cx| {
            mw.find_or_create_local_workspace(
                PathList::new(&[PathBuf::from("/root_a")]),
                None,
                None,
                OpenMode::Activate,
                None,
                window,
                cx,
            )
        })
        .await
        .expect("重新打开相同的本地工作区应成功");

    assert_eq!(
        workspace.entity_id(),
        active_workspace_id,
        "当侧边栏关闭时,应重用当前活动工作区"
    );

    multi_workspace.read_with(cx, |mw, _cx| {
        assert_eq!(
            mw.workspace().entity_id(),
            active_workspace_id,
            "重新打开相同路径后,活动工作区应保持不变"
        );
        assert_eq!(
            mw.workspaces().count(),
            1,
            "重用活动工作区不应创建第二个打开的工作区"
        );
    });
}

#[gpui::test]
async fn test_find_or_create_workspace_uses_project_group_key_when_paths_are_missing(
    cx: &mut TestAppContext,
) {
    init_test(cx);
    let fs = FakeFs::new(cx.executor());
    fs.insert_tree(
        "/project",
        json!({
            ".git": {},
            "src": {},
        }),
    )
    .await;
    cx.update(|cx| <dyn Fs>::set_global(fs.clone(), cx));
    let project = Project::test(fs.clone(), ["/project".as_ref()], cx).await;
    project
        .update(cx, |project, cx| project.git_scans_complete(cx))
        .await;

    let project_group_key = project.read_with(cx, |project, cx| project.project_group_key(cx));

    let (multi_workspace, cx) =
        cx.add_window_view(|window, cx| MultiWorkspace::test_new(project, window, cx));

    let main_workspace = multi_workspace.read_with(cx, |mw, _cx| mw.workspace().clone());
    let main_workspace_id = main_workspace.entity_id();

    let workspace = multi_workspace
        .update_in(cx, |mw, window, cx| {
            mw.find_or_create_workspace(
                PathList::new(&[PathBuf::from("/wt-feature-a")]),
                None,
                Some(project_group_key.clone()),
                |_options, _window, _cx| Task::ready(Ok(None)),
                None,
                OpenMode::Activate,
                None,
                window,
                cx,
            )
        })
        .await
        .expect("打开缺失的链接工作树路径时,应回退到项目组键对应的工作区");

    assert_eq!(
        workspace.entity_id(),
        main_workspace_id,
        "缺失的链接工作树路径应重用项目组键对应的主工作树工作区"
    );

    multi_workspace.read_with(cx, |mw, cx| {
        assert_eq!(
            mw.workspace().entity_id(),
            main_workspace_id,
            "活动工作区应保持为主工作树工作区"
        );
        assert_eq!(
            PathList::new(&mw.workspace().read(cx).root_paths(cx)),
            project_group_key.path_list().clone(),
            "激活的工作区应使用项目组键路径列表,而不是缺失的链接工作树路径"
        );
        assert_eq!(
            mw.workspaces().count(),
            1,
            "回退到项目组键不应创建第二个工作区"
        );
    });
}

#[gpui::test]
async fn test_remove_fallback_via_find_or_create_skips_removed_workspaces(cx: &mut TestAppContext) {
    init_test(cx);
    let fs = FakeFs::new(cx.executor());
    fs.insert_tree("/root_a", json!({ "file.txt": "" })).await;
    let project_a = Project::test(fs.clone(), ["/root_a".as_ref()], cx).await;
    let project_b = Project::test(fs, ["/root_a".as_ref()], cx).await;

    let (multi_workspace, cx) =
        cx.add_window_view(|window, cx| MultiWorkspace::test_new(project_a, window, cx));

    let workspace_a = multi_workspace.read_with(cx, |mw, _cx| mw.workspace().clone());
    let workspace_b = multi_workspace.update_in(cx, |mw, window, cx| {
        mw.test_add_workspace(project_b, window, cx)
    });
    cx.run_until_parked();

    multi_workspace.update_in(cx, |mw, window, cx| {
        mw.activate(workspace_a.clone(), None, window, cx);
    });

    let removed = multi_workspace
        .update_in(cx, |mw, window, cx| {
            mw.remove(
                vec![workspace_a.clone()],
                RemovalIntent::CloseProject,
                window,
                cx,
            )
        })
        .await
        .expect("移除活动工作区应成功");
    assert!(removed, "该工作区应已被移除");

    multi_workspace.read_with(cx, |mw, _cx| {
        assert_eq!(
            mw.workspace().entity_id(),
            workspace_b.entity_id(),
            "未被排除的工作区应变为活动状态"
        );
        assert!(
            mw.workspaces()
                .all(|workspace| workspace.entity_id() != workspace_a.entity_id()),
            "被移除的工作区应已消失"
        );
    });
}

#[gpui::test]
async fn test_remove_keeping_the_project_does_not_switch_projects(cx: &mut TestAppContext) {
    init_test(cx);
    let fs = FakeFs::new(cx.executor());
    fs.insert_tree("/root_a", json!({ "file.txt": "" })).await;
    fs.insert_tree("/root_b", json!({ "file.txt": "" })).await;
    cx.update(|cx| <dyn Fs>::set_global(fs.clone(), cx));
    let project_a = Project::test(fs.clone(), ["/root_a".as_ref()], cx).await;
    let project_b = Project::test(fs, ["/root_b".as_ref()], cx).await;

    let (multi_workspace, cx) =
        cx.add_window_view(|window, cx| MultiWorkspace::test_new(project_a, window, cx));

    let workspace_a = multi_workspace.read_with(cx, |mw, _cx| mw.workspace().clone());
    let _workspace_b = multi_workspace.update_in(cx, |mw, window, cx| {
        mw.test_add_workspace(project_b, window, cx)
    });
    cx.run_until_parked();

    multi_workspace.update_in(cx, |mw, window, cx| {
        mw.activate(workspace_a.clone(), None, window, cx);
    });
    cx.run_until_parked();

    multi_workspace
        .update_in(cx, |mw, window, cx| {
            mw.remove(
                vec![workspace_a.clone()],
                RemovalIntent::KeepProject,
                window,
                cx,
            )
        })
        .await
        .expect("removing the active workspace should succeed");
    cx.run_until_parked();

    multi_workspace.read_with(cx, |mw, cx| {
        assert_eq!(
            PathList::new(&mw.workspace().read(cx).root_paths(cx)),
            PathList::new(&[PathBuf::from("/root_a")]),
            "替换的工作区应位于被移除工作区的项目中"
        );
    });
}

#[gpui::test]
async fn test_find_or_create_local_workspace_reuses_active_workspace_after_sidebar_open(
    cx: &mut TestAppContext,
) {
    init_test(cx);
    let fs = FakeFs::new(cx.executor());
    fs.insert_tree("/root_a", json!({ "file.txt": "" })).await;
    let project = Project::test(fs, ["/root_a".as_ref()], cx).await;

    let (multi_workspace, cx) =
        cx.add_window_view(|window, cx| MultiWorkspace::test_new(project, window, cx));

    multi_workspace.update(cx, |mw, cx| {
        mw.open_sidebar(cx);
    });
    cx.run_until_parked();

    let active_workspace = multi_workspace.read_with(cx, |mw, cx| {
        assert_eq!(
            mw.project_groups(cx).len(),
            1,
            "打开侧边栏应将活动工作区保留在项目组中"
        );
        mw.workspace().clone()
    });
    let active_workspace_id = active_workspace.entity_id();

    let workspace = multi_workspace
        .update_in(cx, |mw, window, cx| {
            mw.find_or_create_local_workspace(
                PathList::new(&[PathBuf::from("/root_a")]),
                None,
                None,
                OpenMode::Activate,
                None,
                window,
                cx,
            )
        })
        .await
        .expect("重新打开相同的已保留本地工作区应成功");

    assert_eq!(
        workspace.entity_id(),
        active_workspace_id,
        "打开侧边栏后,应重用已保留的活动工作区"
    );

    multi_workspace.read_with(cx, |mw, _cx| {
        assert_eq!(
            mw.workspaces().count(),
            1,
            "重新打开相同的已保留工作区不应创建另一个工作区"
        );
    });
}

#[gpui::test]
async fn test_close_workspace_prefers_already_loaded_neighboring_workspace(
    cx: &mut TestAppContext,
) {
    init_test(cx);
    let fs = FakeFs::new(cx.executor());
    fs.insert_tree("/root_a", json!({ "file_a.txt": "" })).await;
    fs.insert_tree("/root_b", json!({ "file_b.txt": "" })).await;
    fs.insert_tree("/root_c", json!({ "file_c.txt": "" })).await;
    let project_a = Project::test(fs.clone(), ["/root_a".as_ref()], cx).await;
    let project_b = Project::test(fs.clone(), ["/root_b".as_ref()], cx).await;
    let project_b_key = project_b.read_with(cx, |project, cx| project.project_group_key(cx));
    let project_c = Project::test(fs, ["/root_c".as_ref()], cx).await;
    let project_c_key = project_c.read_with(cx, |project, cx| project.project_group_key(cx));

    let (multi_workspace, cx) =
        cx.add_window_view(|window, cx| MultiWorkspace::test_new(project_a, window, cx));

    multi_workspace.update(cx, |multi_workspace, cx| {
        multi_workspace.open_sidebar(cx);
    });
    cx.run_until_parked();

    let workspace_a = multi_workspace.read_with(cx, |multi_workspace, _cx| {
        multi_workspace.workspace().clone()
    });
    let workspace_b = multi_workspace.update_in(cx, |multi_workspace, window, cx| {
        multi_workspace.test_add_workspace(project_b, window, cx)
    });

    multi_workspace.update_in(cx, |multi_workspace, window, cx| {
        multi_workspace.activate(workspace_a.clone(), None, window, cx);
        multi_workspace.test_add_project_group(ProjectGroup {
            key: project_c_key.clone(),
            workspaces: Vec::new(),
            expanded: true,
        });
    });

    multi_workspace.read_with(cx, |multi_workspace, _cx| {
        let keys = multi_workspace.project_group_keys();
        assert_eq!(
            keys.len(),
            3,
            "测试设置中应有三个项目组"
        );
        assert_eq!(keys[0], project_b_key);
        assert_eq!(
            keys[1],
            workspace_a.read_with(cx, |workspace, cx| { workspace.project_group_key(cx) })
        );
        assert_eq!(keys[2], project_c_key);
        assert_eq!(
            multi_workspace.workspace().entity_id(),
            workspace_a.entity_id(),
            "关闭前,工作区 A 应为活动状态"
        );
    });

    let closed = multi_workspace
        .update_in(cx, |multi_workspace, window, cx| {
            multi_workspace.remove(
                [workspace_a.clone()],
                RemovalIntent::CloseProject,
                window,
                cx,
            )
        })
        .await
        .expect("关闭活动工作区应成功");

    assert!(
        closed,
        "close_workspace 应报告其移除了一个工作区"
    );

    multi_workspace.read_with(cx, |multi_workspace, cx| {
        assert_eq!(
            multi_workspace.workspace().entity_id(),
            workspace_b.entity_id(),
            "关闭工作区 A 应激活已加载的工作区 B,而不是打开组 C"
        );
        assert_eq!(
            multi_workspace.workspaces().count(),
            1,
            "关闭工作区 A 后,应仅保留已加载的工作区 B"
        );
        assert!(
            multi_workspace
                .workspaces_for_project_group(&project_c_key, cx)
                .is_empty(),
            "未加载的相邻组 C 应保持未打开状态"
        );
    });
}

#[gpui::test]
async fn test_close_workspace_prefers_workspace_in_same_project_group(cx: &mut TestAppContext) {
    init_test(cx);
    let fs = FakeFs::new(cx.executor());
    fs.insert_tree("/project-a", json!({})).await;
    fs.insert_tree("/project-b", json!({})).await;

    let project_a_1 = Project::test(fs.clone(), ["/project-a".as_ref()], cx).await;
    let project_a_2 = Project::test(fs.clone(), ["/project-a".as_ref()], cx).await;
    let project_b = Project::test(fs, ["/project-b".as_ref()], cx).await;
    let key_a = project_a_1.read_with(cx, |project, cx| project.project_group_key(cx));
    let (multi_workspace, cx) = setup_multi_workspace(&[project_a_1, project_a_2, project_b], cx);

    let (workspace_a_1, workspace_a_2) = multi_workspace.read_with(cx, |multi_workspace, cx| {
        let mut workspaces = multi_workspace
            .workspaces_for_project_group(&key_a, cx)
            .into_iter();
        let first = workspaces
            .next()
            .expect("project group A should have a first workspace");
        let second = workspaces
            .next()
            .expect("项目组 A 应有一个第二个工作区");

        assert!(
            workspaces.next().is_none(),
            "项目组 A 应恰好有两个工作区"
        );

        (first, second)
    });

    multi_workspace.update_in(cx, |multi_workspace, window, cx| {
        multi_workspace.activate(workspace_a_1.clone(), None, window, cx);
    });

    let closed = multi_workspace
        .update_in(cx, |multi_workspace, window, cx| {
            multi_workspace.remove(
                [workspace_a_1.clone()],
                RemovalIntent::CloseProject,
                window,
                cx,
            )
        })
        .await
        .expect("关闭活动工作区应成功");

    assert!(closed, "close_workspace 应移除活动工作区");
    multi_workspace.read_with(cx, |multi_workspace, cx| {
        assert_eq!(
            multi_workspace.workspace(),
            &workspace_a_2,
            "应优先选择项目组 a 的第二个工作区"
        );

        assert_eq!(
            multi_workspace.workspaces_for_project_group(&key_a, cx),
            vec![workspace_a_2],
            "项目组 A 中应仅保留回退工作区"
        );
    });
}

#[gpui::test]
async fn test_close_workspace_opens_unloaded_local_neighbor(cx: &mut TestAppContext) {
    init_test(cx);
    let fs = FakeFs::new(cx.executor());
    fs.insert_tree("/project-a", json!({})).await;
    fs.insert_tree("/project-b", json!({})).await;
    cx.update(|cx| <dyn Fs>::set_global(fs.clone(), cx));

    let project_a = Project::test(fs.clone(), ["/project-a".as_ref()], cx).await;
    let project_b = Project::test(fs, ["/project-b".as_ref()], cx).await;
    let key_b = project_b.read_with(cx, |project, cx| project.project_group_key(cx));
    let (multi_workspace, cx) = setup_multi_workspace(&[project_a], cx);
    let workspace_a = multi_workspace.read_with(cx, |multi_workspace, _cx| {
        multi_workspace.workspace().clone()
    });

    multi_workspace.update(cx, |multi_workspace, _cx| {
        multi_workspace.test_add_project_group(ProjectGroup {
            key: key_b.clone(),
            workspaces: Vec::new(),
            expanded: true,
        });
    });

    let closed = multi_workspace
        .update_in(cx, |multi_workspace, window, cx| {
            multi_workspace.remove(
                [workspace_a.clone()],
                RemovalIntent::CloseProject,
                window,
                cx,
            )
        })
        .await
        .expect("closing the active workspace should succeed");

    assert!(closed, "close_workspace 应移除活动工作区");
    multi_workspace.read_with(cx, |multi_workspace, cx| {
        assert_eq!(
            multi_workspace.workspace().read(cx).project_group_key(cx),
            key_b,
            "应打开未加载的本地相邻组"
        );
    });
}

#[gpui::test]
async fn test_remove_project_group_opens_unloaded_local_neighbor(cx: &mut TestAppContext) {
    init_test(cx);
    let fs = FakeFs::new(cx.executor());
    fs.insert_tree("/project-a", json!({})).await;
    fs.insert_tree("/project-b", json!({})).await;
    cx.update(|cx| <dyn Fs>::set_global(fs.clone(), cx));

    let project_a = Project::test(fs.clone(), ["/project-a".as_ref()], cx).await;
    let project_b = Project::test(fs, ["/project-b".as_ref()], cx).await;
    let key_a = project_a.read_with(cx, |project, cx| project.project_group_key(cx));
    let key_b = project_b.read_with(cx, |project, cx| project.project_group_key(cx));
    let (multi_workspace, cx) = setup_multi_workspace(&[project_a], cx);

    multi_workspace.update(cx, |multi_workspace, _cx| {
        multi_workspace.test_add_project_group(ProjectGroup {
            key: key_b.clone(),
            workspaces: Vec::new(),
            expanded: true,
        });
    });

    let removed = multi_workspace
        .update_in(cx, |multi_workspace, window, cx| {
            multi_workspace.remove_project_group(&key_a, window, cx)
        })
        .await
        .expect("removing the active project group should succeed");

    assert!(
        removed,
        "remove_project_group 应移除活动组"
    );

    multi_workspace.read_with(cx, |multi_workspace, cx| {
        assert_eq!(
            multi_workspace.workspace().read(cx).project_group_key(cx),
            key_b,
            "应打开未加载的本地相邻组"
        );
    });
}

#[gpui::test]
async fn test_remove_project_group_replaces_unretained_active_workspace(cx: &mut TestAppContext) {
    init_test(cx);
    let fs = FakeFs::new(cx.executor());
    fs.insert_tree("/project-a", json!({})).await;

    let project_a = Project::test(fs, ["/project-a".as_ref()], cx).await;
    let key_a = project_a.read_with(cx, |project, cx| project.project_group_key(cx));
    let remote_key = ProjectGroupKey::new(
        Some(RemoteConnectionOptions::Mock(
            remote::MockConnectionOptions { id: 1 },
        )),
        PathList::new(&[PathBuf::from("/remote/project")]),
    );
    let (multi_workspace, cx) =
        cx.add_window_view(|window, cx| MultiWorkspace::test_new(project_a, window, cx));
    let workspace_a = multi_workspace.read_with(cx, |multi_workspace, _cx| {
        multi_workspace.workspace().clone()
    });

    multi_workspace.update(cx, |multi_workspace, cx| {
        multi_workspace.restore_project_groups(
            vec![
                SerializedProjectGroupState {
                    key: key_a.clone(),
                    expanded: true,
                },
                SerializedProjectGroupState {
                    key: remote_key.clone(),
                    expanded: true,
                },
            ],
            cx,
        );

        assert!(
            !multi_workspace.active_workspace_is_retained(),
            "活动工作区应保持临时状态"
        );
        assert_eq!(
            multi_workspace.project_group_keys(),
            vec![key_a.clone(), remote_key.clone()],
            "远程项目组应立即跟在活动本地组之后"
        );
    });

    multi_workspace
        .update_in(cx, |multi_workspace, window, cx| {
            multi_workspace.remove_project_group(&key_a, window, cx)
        })
        .await
        .expect("移除活动项目组应成功");

    multi_workspace.read_with(cx, |multi_workspace, cx| {
        assert_ne!(
            multi_workspace.workspace(),
            &workspace_a,
            "移除活动项目组应替换其临时工作区"
        );
        assert!(
            multi_workspace
                .workspace()
                .read(cx)
                .root_paths(cx)
                .is_empty(),
            "未加载的远程相邻组应回退到空工作区"
        );
        assert_eq!(
            multi_workspace.project_group_keys(),
            vec![remote_key],
            "应仅保留远程项目组"
        );
    });
}

#[gpui::test]
async fn test_switching_projects_with_sidebar_closed_retains_old_active_workspace(
    cx: &mut TestAppContext,
) {
    init_test(cx);
    let fs = FakeFs::new(cx.executor());
    fs.insert_tree("/root_a", json!({ "file_a.txt": "" })).await;
    fs.insert_tree("/root_b", json!({ "file_b.txt": "" })).await;
    let project_a = Project::test(fs.clone(), ["/root_a".as_ref()], cx).await;
    let project_b = Project::test(fs, ["/root_b".as_ref()], cx).await;

    let (multi_workspace, cx) =
        cx.add_window_view(|window, cx| MultiWorkspace::test_new(project_a, window, cx));

    let workspace_a = multi_workspace.read_with(cx, |mw, cx| {
        assert!(
            mw.project_groups(cx).is_empty(),
            "sidebar-closed setup should start with no retained project groups"
        );
        mw.workspace().clone()
    });
    assert!(
        workspace_a.read_with(cx, |workspace, _cx| workspace.session_id().is_some()),
        "初始活动工作区应开始附加到会话"
    );

    let workspace_b = multi_workspace.update_in(cx, |mw, window, cx| {
        mw.test_add_workspace(project_b, window, cx)
    });
    cx.run_until_parked();

    multi_workspace.read_with(cx, |mw, cx| {
        assert_eq!(
            mw.workspace().entity_id(),
            workspace_b.entity_id(),
            "新工作区应变为活动状态"
        );
        assert_eq!(
            mw.workspaces().count(),
            2,
            "在侧边栏关闭的情况下切换后,先前的活动工作区应保持打开状态"
        );
        assert_eq!(mw.project_groups(cx).len(), 2);
    });

    assert!(
        workspace_a.read_with(cx, |workspace, _cx| workspace.session_id().is_some()),
        "在侧边栏关闭的情况下切换离开时,先前的活动工作区应保持附加状态"
    );
}

#[gpui::test]
async fn test_remote_project_root_dir_changes_update_groups(cx: &mut TestAppContext) {
    init_test(cx);
    let fs = FakeFs::new(cx.executor());
    fs.insert_tree("/root_a", json!({ "file.txt": "" })).await;
    fs.insert_tree("/local_b", json!({ "file.txt": "" })).await;
    let project_a = Project::test(fs.clone(), ["/root_a".as_ref()], cx).await;
    let project_b = Project::test(fs.clone(), ["/local_b".as_ref()], cx).await;

    let (multi_workspace, cx) =
        cx.add_window_view(|window, cx| MultiWorkspace::test_new(project_a, window, cx));

    multi_workspace.update(cx, |mw, cx| {
        mw.open_sidebar(cx);
    });
    cx.run_until_parked();

    let workspace_b = multi_workspace.update_in(cx, |mw, window, cx| {
        let workspace = cx.new(|cx| Workspace::test_new(project_b.clone(), window, cx));
        let key = workspace.read(cx).project_group_key(cx);
        mw.activate_provisional_workspace(workspace.clone(), key, window, cx);
        workspace
    });
    cx.run_until_parked();

    multi_workspace.read_with(cx, |mw, _cx| {
        assert_eq!(
            mw.workspace().entity_id(),
            workspace_b.entity_id(),
            "registered workspace should become active"
        );
    });

    let initial_key = project_b.read_with(cx, |p, cx| p.project_group_key(cx));
    multi_workspace.read_with(cx, |mw, _cx| {
        let keys = mw.project_group_keys();
        assert!(
            keys.contains(&initial_key),
            "项目组应包含已注册工作区的初始键"
        );
    });

    let remote_worktree = project_b.update(cx, |project, cx| {
        project.add_test_remote_worktree("/remote/project", cx)
    });
    cx.run_until_parked();

    let worktree_id = remote_worktree.read_with(cx, |wt, _| wt.id().to_proto());
    remote_worktree.update(cx, |worktree, _cx| {
        worktree
            .as_remote()
            .unwrap()
            .update_from_remote(proto::UpdateWorktree {
                project_id: 0,
                worktree_id,
                abs_path: "/remote/project".to_string(),
                root_name: "project".to_string(),
                updated_entries: vec![proto::Entry {
                    id: 1,
                    is_dir: true,
                    path: "".to_string(),
                    inode: 1,
                    mtime: Some(proto::Timestamp {
                        seconds: 0,
                        nanos: 0,
                    }),
                    is_ignored: false,
                    is_hidden: false,
                    is_external: false,
                    is_fifo: false,
                    size: None,
                    canonical_path: None,
                }],
                removed_entries: vec![],
                scan_id: 1,
                is_last_update: true,
                updated_repositories: vec![],
                removed_repositories: vec![],
                root_repo_common_dir: None,
                root_repo_is_linked_worktree: false,
            });
    });
    cx.run_until_parked();

    let updated_key = project_b.read_with(cx, |p, cx| p.project_group_key(cx));
    assert_ne!(
        initial_key, updated_key,
        "远程工作树更新应更改项目组键"
    );

    multi_workspace.read_with(cx, |mw, _cx| {
        let keys = mw.project_group_keys();
        assert!(
            keys.contains(&updated_key),
            "远程更改后,项目组应包含更新后的键;实际为 {keys:?}"
        );
        assert!(
            !keys.contains(&initial_key),
            "项目组不应再包含过时的初始键;实际为 {keys:?}"
        );
    });
}

#[gpui::test]
async fn test_open_project_closes_empty_workspace_but_not_non_empty_ones(cx: &mut TestAppContext) {
    init_test(cx);
    let app_state = cx.update(AppState::test);
    let fs = app_state.fs.as_fake();
    fs.insert_tree(path!("/project_a"), json!({ "file_a.txt": "" }))
        .await;
    fs.insert_tree(path!("/project_b"), json!({ "file_b.txt": "" }))
        .await;

    // Start with an empty (no-worktrees) workspace.
    let project = Project::test(app_state.fs.clone(), [], cx).await;
    let window = cx.add_window(|window, cx| MultiWorkspace::test_new(project, window, cx));
    cx.run_until_parked();

    window
        .update(cx, |mw, _window, cx| mw.open_sidebar(cx))
        .unwrap();
    cx.run_until_parked();

    let empty_workspace = window
        .read_with(cx, |mw, _| mw.workspace().clone())
        .unwrap();
    let cx = &mut VisualTestContext::from_window(window.into(), cx);

    // Add a dirty untitled item to the empty workspace.
    let dirty_item = cx.new(|cx| TestItem::new(cx).with_dirty(true));
    empty_workspace.update_in(cx, |workspace, window, cx| {
        workspace.add_item_to_active_pane(Box::new(dirty_item.clone()), None, true, window, cx);
    });

    // Opening a project while the lone empty workspace has unsaved
    // changes prompts the user.
    let open_task = window
        .update(cx, |mw, window, cx| {
            mw.open_project(
                vec![PathBuf::from(path!("/project_a"))],
                OpenMode::Activate,
                window,
                cx,
            )
        })
        .unwrap();
    cx.run_until_parked();

    // Cancelling keeps the empty workspace.
    assert!(cx.has_pending_prompt(),);
    cx.simulate_prompt_answer("取消");
    cx.run_until_parked();
    assert_eq!(open_task.await.unwrap(), empty_workspace);
    window
        .read_with(cx, |mw, _cx| {
            assert_eq!(mw.workspaces().count(), 1);
            assert_eq!(mw.workspace(), &empty_workspace);
            assert_eq!(mw.project_group_keys(), vec![]);
        })
        .unwrap();

    // Discarding the unsaved changes closes the empty workspace
    // and opens the new project in its place.
    let open_task = window
        .update(cx, |mw, window, cx| {
            mw.open_project(
                vec![PathBuf::from(path!("/project_a"))],
                OpenMode::Activate,
                window,
                cx,
            )
        })
        .unwrap();
    cx.run_until_parked();

    assert!(cx.has_pending_prompt(),);
    cx.simulate_prompt_answer("不保存");
    cx.run_until_parked();

    let workspace_a = open_task.await.unwrap();
    assert_ne!(workspace_a, empty_workspace);

    window
        .read_with(cx, |mw, _cx| {
            assert_eq!(mw.workspaces().count(), 1);
            assert_eq!(mw.workspace(), &workspace_a);
            assert_eq!(
                mw.project_group_keys(),
                vec![ProjectGroupKey::new(
                    None,
                    PathList::new(&[path!("/project_a")])
                )]
            );
        })
        .unwrap();
    assert!(
        empty_workspace.read_with(cx, |workspace, _cx| workspace.session_id().is_none()),
        "分离的空工作区不应再附加到会话",
    );

    let dirty_item = cx.new(|cx| TestItem::new(cx).with_dirty(true));
    workspace_a.update_in(cx, |workspace, window, cx| {
        workspace.add_item_to_active_pane(Box::new(dirty_item.clone()), None, true, window, cx);
    });

    // Opening another project does not close the existing project or prompt.
    let workspace_b = window
        .update(cx, |mw, window, cx| {
            mw.open_project(
                vec![PathBuf::from(path!("/project_b"))],
                OpenMode::Activate,
                window,
                cx,
            )
        })
        .unwrap()
        .await
        .unwrap();
    cx.run_until_parked();

    assert!(!cx.has_pending_prompt());
    assert_ne!(workspace_b, workspace_a);
    window
        .read_with(cx, |mw, _cx| {
            assert_eq!(mw.workspaces().count(), 2);
            assert_eq!(mw.workspace(), &workspace_b);
            assert_eq!(
                mw.project_group_keys(),
                vec![
                    ProjectGroupKey::new(None, PathList::new(&[path!("/project_b")])),
                    ProjectGroupKey::new(None, PathList::new(&[path!("/project_a")]))
                ]
            );
        })
        .unwrap();
    assert!(workspace_a.read_with(cx, |workspace, _cx| workspace.session_id().is_some()),);
}

#[gpui::test]
async fn test_close_workspace_with_remote_neighbor_does_not_create_local_workspace(
    cx: &mut TestAppContext,
) {
    // Regression test: closing a workspace whose neighboring group is
    // remote with no existing workspace should not create a local
    // workspace with the remote paths.
    init_test(cx);
    let fs = FakeFs::new(cx.executor());
    fs.insert_tree("/root_a", json!({ "file.txt": "" })).await;
    let project_a = Project::test(fs, ["/root_a".as_ref()], cx).await;

    let (multi_workspace, cx) =
        cx.add_window_view(|window, cx| MultiWorkspace::test_new(project_a, window, cx));

    multi_workspace.update(cx, |mw, cx| {
        mw.open_sidebar(cx);
    });
    cx.run_until_parked();

    // Add a mock-remote group with no workspace as the second group.
    let remote_key = ProjectGroupKey::new(
        Some(RemoteConnectionOptions::Mock(
            remote::MockConnectionOptions { id: 1 },
        )),
        PathList::new(&[PathBuf::from("/remote/project")]),
    );
    multi_workspace.update(cx, |mw, _cx| {
        mw.test_add_project_group(ProjectGroup {
            key: remote_key.clone(),
            workspaces: Vec::new(),
            expanded: true,
        });
    });

    let workspace_a = multi_workspace.read_with(cx, |mw, _cx| mw.workspace().clone());

    // Close workspace A. The neighbor is the remote group with no workspace.
    // The fix should skip find_or_create_local_workspace and fall through
    // to creating an empty workspace instead.
    multi_workspace
        .update_in(cx, |mw, window, cx| {
            mw.remove(
                [workspace_a.clone()],
                RemovalIntent::CloseProject,
                window,
                cx,
            )
        })
        .await
        .expect("close_workspace 应成功");

    cx.run_until_parked();

    multi_workspace.update(cx, |mw, cx| {
        // The active workspace should NOT be a local workspace with the
        // remote paths. It should be an empty workspace (no worktrees).
        let workspaces: Vec<_> = mw.workspaces().cloned().collect();
        for ws in &workspaces {
            let key = ws.read(cx).project_group_key(cx);
            assert!(
                key.host().is_some()
                    || key.path_list().paths() != [PathBuf::from("/remote/project")],
                "远程相邻组不应创建本地工作区"
            );
        }
    });
}

#[gpui::test]
async fn test_remove_project_group_with_remote_neighbor_does_not_create_local_workspace(
    cx: &mut TestAppContext,
) {
    // Regression test: removing a project group whose neighboring group is
    // remote with no workspace should not create a local workspace with
    // the remote paths.
    init_test(cx);
    let fs = FakeFs::new(cx.executor());
    fs.insert_tree("/root_a", json!({ "file.txt": "" })).await;
    let project_a = Project::test(fs, ["/root_a".as_ref()], cx).await;

    let (multi_workspace, cx) =
        cx.add_window_view(|window, cx| MultiWorkspace::test_new(project_a.clone(), window, cx));

    multi_workspace.update(cx, |mw, cx| {
        mw.open_sidebar(cx);
    });
    cx.run_until_parked();

    let key_a = project_a.read_with(cx, |p, cx| p.project_group_key(cx));

    // Add a mock-remote group with no workspace.
    let remote_key = ProjectGroupKey::new(
        Some(RemoteConnectionOptions::Mock(
            remote::MockConnectionOptions { id: 1 },
        )),
        PathList::new(&[PathBuf::from("/remote/project")]),
    );
    multi_workspace.update(cx, |mw, _cx| {
        mw.test_add_project_group(ProjectGroup {
            key: remote_key.clone(),
            workspaces: Vec::new(),
            expanded: true,
        });
    });

    // Remove the local group A. The neighbor is the remote group with no
    // workspace. The fix should skip find_or_create_local_workspace and
    // fall through to creating an empty workspace.
    multi_workspace
        .update_in(cx, |mw, window, cx| {
            mw.remove_project_group(&key_a, window, cx)
        })
        .await
        .expect("remove_project_group 应成功");

    cx.run_until_parked();

    multi_workspace.update(cx, |mw, cx| {
        let workspaces: Vec<_> = mw.workspaces().cloned().collect();
        for ws in &workspaces {
            let key = ws.read(cx).project_group_key(cx);
            assert!(
                key.host().is_some() || key.path_list().paths() != [PathBuf::from("/remote/project")],
                "remove_project_group 之后,远程相邻组不应创建本地工作区"
            );
        }
    });
}

#[gpui::test]
async fn test_nearest_retained_workspace(cx: &mut TestAppContext) {
    init_test(cx);

    let fs = FakeFs::new(cx.executor());
    fs.insert_tree("/project-a", json!({})).await;
    fs.insert_tree("/project-b", json!({})).await;
    fs.insert_tree("/project-c", json!({})).await;
    fs.insert_tree("/project-d", json!({})).await;

    // These two projects create separate workspaces in the same project group. The second
    // workspace is activated after the first, making it the group's last active workspace.
    let project_a_1 = Project::test(fs.clone(), ["/project-a".as_ref()], cx).await;
    let project_a_2 = Project::test(fs.clone(), ["/project-a".as_ref()], cx).await;
    let project_b = Project::test(fs.clone(), ["/project-b".as_ref()], cx).await;
    let project_c = Project::test(fs.clone(), ["/project-c".as_ref()], cx).await;
    let project_d = Project::test(fs, ["/project-d".as_ref()], cx).await;
    let key_a = project_a_1.read_with(cx, |project, cx| project.project_group_key(cx));
    let key_b = project_b.read_with(cx, |project, cx| project.project_group_key(cx));
    let key_c = project_c.read_with(cx, |project, cx| project.project_group_key(cx));
    let key_d = project_d.read_with(cx, |project, cx| project.project_group_key(cx));

    let (multi_workspace, cx) = setup_multi_workspace(
        &[project_a_1, project_a_2, project_b, project_c, project_d],
        cx,
    );

    multi_workspace.update(cx, |multi_workspace, cx| {
        assert_eq!(
            multi_workspace.project_group_keys(),
            vec![key_d.clone(), key_c.clone(), key_b.clone(), key_a.clone()],
            "新项目组应插入到现有组之前"
        );

        let group_c_index = multi_workspace
            .project_groups(cx)
            .iter()
            .position(|project_group| project_group.key == key_c)
            .expect("project-c 的项目组应存在");
        let workspace_b = multi_workspace
            .workspaces_for_project_group(&key_b, cx)
            .into_iter()
            .next()
            .expect("项目-b 的工作区应存在");
        let workspace_d = multi_workspace
            .workspaces_for_project_group(&key_d, cx)
            .into_iter()
            .next()
            .expect("项目-d 的工作区应存在");
        let retained_workspaces_a = multi_workspace
            .workspaces_for_project_group(&key_a, cx);
        assert_eq!(
            retained_workspaces_a.len(),
            2,
            "项目组 A 应保留两个工作区"
        );
        let workspace_a_1 = retained_workspaces_a
            .first()
            .expect("项目组 A 应有一个已保留的工作区")
            .clone();
        let workspace_a_2 = multi_workspace
            .last_active_workspace_for_group(&key_a, cx)
            .expect("项目组 A 应有一个最后活动的工作区");
        assert_ne!(
            workspace_a_1, workspace_a_2,
            "项目组 A 的最后活动工作区应与其第一个保留的工作区不同"
        );

        // Since Project Group B is the one after C, it is preferred over
        // Project Group D, even if they're at the same distance.
        assert_eq!(
            multi_workspace.nearest_retained_workspace(group_c_index, &[], cx),
            Some(workspace_b.clone()),
            "距离相等时,应优先选择后面的项目组"
        );

        // With Project Group B being excluded, Project Group D is picked as it
        // is the one with the smallest distance.
        assert_eq!(
            multi_workspace.nearest_retained_workspace(
                group_c_index,
                std::slice::from_ref(&workspace_b),
                cx,
            ),
            Some(workspace_d.clone()),
            "当后面的工作区被排除时,应使用前面的项目组"
        );

        // With both adjacent Project Groups excluded, the search expands and
        // reaches Project A at distance 2 and prefers its last active workspace (A2)
        // over its first retained workspace (A1).
        assert_eq!(
            multi_workspace.nearest_retained_workspace(
                group_c_index,
                &[workspace_b.clone(), workspace_d.clone()],
                cx
            ),
            Some(workspace_a_2.clone()),
            "应优先选择更远组的最后活动工作区"
        );

        // With the group's most recently activated workspace excluded, the
        // search falls back to the member activated before it.
        assert_eq!(
            multi_workspace.nearest_retained_workspace(
                group_c_index,
                &[
                    workspace_b.clone(),
                    workspace_d.clone(),
                    workspace_a_2.clone()
                ],
                cx
            ),
            Some(workspace_a_1.clone()),
            "当最后活动的工作区被排除时,应使用先前激活的工作区"
        );

        // Excluding every neighboring workspace exhausts the search.
        assert_eq!(
            multi_workspace.nearest_retained_workspace(
                group_c_index,
                &[
                    workspace_b,
                    workspace_d,
                    workspace_a_1,
                    workspace_a_2,
                ],
                cx
            ),
            None,
            "当所有候选都被排除时,不应返回任何工作区"
        );
    });
}

#[gpui::test]
async fn test_nearest_retained_workspace_skips_disconnected_workspace(cx: &mut TestAppContext) {
    init_test(cx);
    let fs = FakeFs::new(cx.executor());
    fs.insert_tree("/project-a", json!({})).await;
    fs.insert_tree("/project-b", json!({})).await;

    let project_a = Project::test(fs.clone(), ["/project-a".as_ref()], cx).await;
    let project_b = Project::test(fs, ["/project-b".as_ref()], cx).await;
    let key_a = project_a.read_with(cx, |project, cx| project.project_group_key(cx));
    let (multi_workspace, cx) = setup_multi_workspace(&[project_a.clone(), project_b.clone()], cx);

    project_b.update(cx, |project, cx| {
        project.mark_as_collab_for_testing();
        project.disconnected_from_host(cx);
    });
    cx.run_until_parked();

    multi_workspace.update(cx, |multi_workspace, cx| {
        let group_a_index = multi_workspace
            .project_groups(cx)
            .iter()
            .position(|group| group.key == key_a)
            .expect("project group A should exist");

        assert_eq!(
            multi_workspace.nearest_retained_workspace(group_a_index, &[], cx),
            None,
            "不应选择断开连接的工作区作为回退"
        );
    });
}
