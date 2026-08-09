use crate::TestServer;
use call::ActiveCall;
use collections::{HashMap, HashSet};

use dap::{Capabilities, adapters::DebugTaskDefinition, transport::RequestHandling};
use debugger_ui::debugger_panel::DebugPanel;
use editor::{Editor, EditorMode, LSP_REQUEST_DEBOUNCE_TIMEOUT, MultiBuffer};
use extension::ExtensionHostProxy;
use fs::{FakeFs, Fs as _, RemoveOptions};
use futures::StreamExt as _;
use gpui::{
    AppContext as _, BackgroundExecutor, TestAppContext, UpdateGlobal as _, VisualContext as _,
};
use http_client::BlockedHttpClient;
use language::{
    FakeLspAdapter, Language, LanguageConfig, LanguageMatcher, LanguageRegistry,
    language_settings::{ConfiguredLanguageServer, Formatter, FormatterList, LanguageSettings},
    rust_lang, tree_sitter_typescript,
};
use node_runtime::NodeRuntime;
use project::{
    ProjectPath,
    debugger::session::ThreadId,
    lsp_store::{
        FormatTrigger, LspFormatTarget,
        log_store::{self, GlobalLogStore},
    },
    trusted_worktrees::{PathTrust, TrustedWorktrees},
};
use remote::RemoteClient;
use remote_server::{HeadlessAppState, HeadlessProject};
use rpc::proto;
use serde_json::json;
use settings::{
    InlayHintSettingsContent, LanguageServerFormatterSpecifier, PrettierSettingsContent,
    SettingsStore,
};
use std::{
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};
use task::TcpArgumentsTemplate;
use util::{path, rel_path::rel_path};

#[gpui::test(iterations = 10)]
async fn test_sharing_an_ssh_remote_project(
    cx_a: &mut TestAppContext,
    cx_b: &mut TestAppContext,
    server_cx: &mut TestAppContext,
) {
    let executor = cx_a.executor();
    cx_a.update(|cx| {
        release_channel::init(semver::Version::new(0, 0, 0), cx);
    });
    server_cx.update(|cx| {
        release_channel::init(semver::Version::new(0, 0, 0), cx);
    });
    let mut server = TestServer::start(executor.clone()).await;
    let client_a = server.create_client(cx_a, "user_a").await;
    let client_b = server.create_client(cx_b, "user_b").await;
    server
        .create_room(&mut [(&client_a, cx_a), (&client_b, cx_b)])
        .await;

    // Set up project on remote FS
    let (opts, server_ssh, _) = RemoteClient::fake_server(cx_a, server_cx);
    let remote_fs = FakeFs::new(server_cx.executor());
    remote_fs
        .insert_tree(
            path!("/code"),
            json!({
                "project1": {
                    ".zed": {
                        "settings.json": r#"{"languages":{"Rust":{"language_servers":["override-rust-analyzer"]}}}"#
                    },
                    "README.md": "# project 1",
                    "src": {
                        "lib.rs": "fn one() -> usize { 1 }"
                    }
                },
                "project2": {
                    "README.md": "# project 2",
                },
            }),
        )
        .await;

    // User A connects to the remote project via SSH.
    server_cx.update(HeadlessProject::init);
    let remote_http_client = Arc::new(BlockedHttpClient);
    let node = NodeRuntime::unavailable();
    let languages = Arc::new(LanguageRegistry::new(server_cx.executor()));
    languages.add(rust_lang());
    let _headless_project = server_cx.new(|cx| {
        HeadlessProject::new(
            HeadlessAppState {
                session: server_ssh,
                fs: remote_fs.clone(),
                http_client: remote_http_client,
                node_runtime: node,
                languages,
                extension_host_proxy: Arc::new(ExtensionHostProxy::new()),
                startup_time: std::time::Instant::now(),
            },
            false,
            cx,
        )
    });

    let client_ssh = RemoteClient::connect_mock(opts, cx_a).await;
    let (project_a, worktree_id) = client_a
        .build_ssh_project(path!("/code/project1"), client_ssh, false, cx_a)
        .await;

    // While the SSH worktree is being scanned, user A shares the remote project.
    let active_call_a = cx_a.read(ActiveCall::global);
    let project_id = active_call_a
        .update(cx_a, |call, cx| call.share_project(project_a.clone(), cx))
        .await
        .unwrap();

    // User B joins the project.
    let project_b = client_b.join_remote_project(project_id, cx_b).await;
    project_b.update(cx_b, |project, _| project.languages().add(rust_lang()));
    let worktree_b = project_b
        .update(cx_b, |project, cx| project.worktree_for_id(worktree_id, cx))
        .unwrap();

    let worktree_a = project_a
        .update(cx_a, |project, cx| project.worktree_for_id(worktree_id, cx))
        .unwrap();

    executor.run_until_parked();

    worktree_a.update(cx_a, |worktree, _cx| {
        assert_eq!(
            worktree.paths().collect::<Vec<_>>(),
            vec![
                rel_path(".zed"),
                rel_path(".zed/settings.json"),
                rel_path("README.md"),
                rel_path("src"),
                rel_path("src/lib.rs"),
            ]
        );
    });

    worktree_b.update(cx_b, |worktree, _cx| {
        assert_eq!(
            worktree.paths().collect::<Vec<_>>(),
            vec![
                rel_path(".zed"),
                rel_path(".zed/settings.json"),
                rel_path("README.md"),
                rel_path("src"),
                rel_path("src/lib.rs"),
            ]
        );
    });

    // User B can open buffers in the remote project.
    let buffer_b = project_b
        .update(cx_b, |project, cx| {
            project.open_buffer((worktree_id, rel_path("src/lib.rs")), cx)
        })
        .await
        .unwrap();
    buffer_b.update(cx_b, |buffer, cx| {
        assert_eq!(buffer.text(), "fn one() -> usize { 1 }");
        let ix = buffer.text().find('1').unwrap();
        buffer.edit([(ix..ix + 1, "100")], None, cx);
    });

    executor.run_until_parked();

    cx_b.read(|cx| {
        assert_eq!(
            LanguageSettings::for_buffer(buffer_b.read(cx), cx).language_servers,
            [ConfiguredLanguageServer::new("override-rust-analyzer")]
        )
    });

    project_b
        .update(cx_b, |project, cx| {
            project.save_buffer_as(
                buffer_b.clone(),
                ProjectPath {
                    worktree_id: worktree_id.to_owned(),
                    path: rel_path("src/renamed.rs").into(),
                },
                cx,
            )
        })
        .await
        .unwrap();
    assert_eq!(
        remote_fs
            .load(path!("/code/project1/src/renamed.rs").as_ref())
            .await
            .unwrap(),
        "fn one() -> usize { 100 }"
    );
    cx_b.run_until_parked();
    cx_b.update(|cx| {
        assert_eq!(
            buffer_b.read(cx).file().unwrap().path().as_ref(),
            rel_path("src/renamed.rs")
        );
    });
}

#[gpui::test]
async fn test_ssh_collaboration_git_branches(
    executor: BackgroundExecutor,
    cx_a: &mut TestAppContext,
    cx_b: &mut TestAppContext,
    server_cx: &mut TestAppContext,
) {
    cx_a.set_name("a");
    cx_b.set_name("b");
    server_cx.set_name("server");

    cx_a.update(|cx| {
        release_channel::init(semver::Version::new(0, 0, 0), cx);
    });
    server_cx.update(|cx| {
        release_channel::init(semver::Version::new(0, 0, 0), cx);
    });

    let mut server = TestServer::start(executor.clone()).await;
    let client_a = server.create_client(cx_a, "user_a").await;
    let client_b = server.create_client(cx_b, "user_b").await;
    server
        .create_room(&mut [(&client_a, cx_a), (&client_b, cx_b)])
        .await;

    // Set up project on remote FS
    let (opts, server_ssh, _) = RemoteClient::fake_server(cx_a, server_cx);
    let remote_fs = FakeFs::new(server_cx.executor());
    remote_fs
        .insert_tree("/project", serde_json::json!({ ".git":{} }))
        .await;

    let branches = ["main", "dev", "feature-1"];
    let branches_set = branches
        .iter()
        .map(ToString::to_string)
        .collect::<HashSet<_>>();
    remote_fs.insert_branches(Path::new("/project/.git"), &branches);

    // User A connects to the remote project via SSH.
    server_cx.update(HeadlessProject::init);
    let remote_http_client = Arc::new(BlockedHttpClient);
    let node = NodeRuntime::unavailable();
    let languages = Arc::new(LanguageRegistry::new(server_cx.executor()));
    let headless_project = server_cx.new(|cx| {
        HeadlessProject::new(
            HeadlessAppState {
                session: server_ssh,
                fs: remote_fs.clone(),
                http_client: remote_http_client,
                node_runtime: node,
                languages,
                extension_host_proxy: Arc::new(ExtensionHostProxy::new()),
                startup_time: std::time::Instant::now(),
            },
            false,
            cx,
        )
    });

    let client_ssh = RemoteClient::connect_mock(opts, cx_a).await;
    let (project_a, _) = client_a
        .build_ssh_project("/project", client_ssh, false, cx_a)
        .await;

    // While the SSH worktree is being scanned, user A shares the remote project.
    let active_call_a = cx_a.read(ActiveCall::global);
    let project_id = active_call_a
        .update(cx_a, |call, cx| call.share_project(project_a.clone(), cx))
        .await
        .unwrap();

    // User B joins the project.
    let project_b = client_b.join_remote_project(project_id, cx_b).await;

    // Give client A sometime to see that B has joined, and that the headless server
    // has some git repositories
    executor.run_until_parked();

    let repo_b = cx_b.update(|cx| project_b.read(cx).active_repository(cx).unwrap());

    let branches_b = cx_b
        .update(|cx| repo_b.update(cx, |repo_b, _cx| repo_b.branches()))
        .await
        .unwrap()
        .unwrap();

    let new_branch = branches[2];

    let branches_b = branches_b
        .branches
        .into_iter()
        .map(|branch| branch.name().to_string())
        .collect::<HashSet<_>>();

    assert_eq!(&branches_b, &branches_set);

    cx_b.update(|cx| {
        repo_b.update(cx, |repo_b, _cx| {
            repo_b.change_branch(new_branch.to_string())
        })
    })
    .await
    .unwrap()
    .unwrap();

    executor.run_until_parked();

    let server_branch = server_cx.update(|cx| {
        headless_project.update(cx, |headless_project, cx| {
            headless_project.git_store.update(cx, |git_store, cx| {
                git_store
                    .repositories()
                    .values()
                    .next()
                    .unwrap()
                    .read(cx)
                    .branch
                    .as_ref()
                    .unwrap()
                    .clone()
            })
        })
    });

    assert_eq!(server_branch.name(), branches[2]);

    // Also try creating a new branch
    cx_b.update(|cx| {
        repo_b.update(cx, |repo_b, _cx| {
            repo_b.create_branch("totally-new-branch".to_string(), None)
        })
    })
    .await
    .unwrap()
    .unwrap();

    cx_b.update(|cx| {
        repo_b.update(cx, |repo_b, _cx| {
            repo_b.change_branch("totally-new-branch".to_string())
        })
    })
    .await
    .unwrap()
    .unwrap();

    executor.run_until_parked();

    let server_branch = server_cx.update(|cx| {
        headless_project.update(cx, |headless_project, cx| {
            headless_project.git_store.update(cx, |git_store, cx| {
                git_store
                    .repositories()
                    .values()
                    .next()
                    .unwrap()
                    .read(cx)
                    .branch
                    .as_ref()
                    .unwrap()
                    .clone()
            })
        })
    });

    assert_eq!(server_branch.name(), "totally-new-branch");

    // Remove the git repository and check that all participants get the update.
    remote_fs
        .remove_dir("/project/.git".as_ref(), RemoveOptions::default())
        .await
        .unwrap();
    executor.run_until_parked();

    project_a.update(cx_a, |project, cx| {
        pretty_assertions::assert_eq!(
            project.git_store().read(cx).repo_snapshots(cx),
            HashMap::default()
        );
    });
    project_b.update(cx_b, |project, cx| {
        pretty_assertions::assert_eq!(
            project.git_store().read(cx).repo_snapshots(cx),
            HashMap::default()
        );
    });
}

#[gpui::test]
async fn test_ssh_collaboration_git_worktrees(
    executor: BackgroundExecutor,
    cx_a: &mut TestAppContext,
    cx_b: &mut TestAppContext,
    server_cx: &mut TestAppContext,
) {
    cx_a.set_name("a");
    cx_b.set_name("b");
    server_cx.set_name("server");

    cx_a.update(|cx| {
        release_channel::init(semver::Version::new(0, 0, 0), cx);
    });
    server_cx.update(|cx| {
        release_channel::init(semver::Version::new(0, 0, 0), cx);
    });

    let mut server = TestServer::start(executor.clone()).await;
    let client_a = server.create_client(cx_a, "user_a").await;
    let client_b = server.create_client(cx_b, "user_b").await;
    server
        .create_room(&mut [(&client_a, cx_a), (&client_b, cx_b)])
        .await;

    let (opts, server_ssh, _) = RemoteClient::fake_server(cx_a, server_cx);
    let remote_fs = FakeFs::new(server_cx.executor());
    remote_fs
        .insert_tree("/project", json!({ ".git": {}, "file.txt": "content" }))
        .await;

    server_cx.update(HeadlessProject::init);
    let languages = Arc::new(LanguageRegistry::new(server_cx.executor()));
    let headless_project = server_cx.new(|cx| {
        HeadlessProject::new(
            HeadlessAppState {
                session: server_ssh,
                fs: remote_fs.clone(),
                http_client: Arc::new(BlockedHttpClient),
                node_runtime: NodeRuntime::unavailable(),
                languages,
                extension_host_proxy: Arc::new(ExtensionHostProxy::new()),
                startup_time: std::time::Instant::now(),
            },
            false,
            cx,
        )
    });

    let client_ssh = RemoteClient::connect_mock(opts, cx_a).await;
    let (project_a, _) = client_a
        .build_ssh_project("/project", client_ssh, false, cx_a)
        .await;

    let active_call_a = cx_a.read(ActiveCall::global);
    let project_id = active_call_a
        .update(cx_a, |call, cx| call.share_project(project_a.clone(), cx))
        .await
        .unwrap();
    let project_b = client_b.join_remote_project(project_id, cx_b).await;

    executor.run_until_parked();

    let repo_b = cx_b.update(|cx| project_b.read(cx).active_repository(cx).unwrap());

    let worktrees = cx_b
        .update(|cx| repo_b.update(cx, |repo, _| repo.worktrees()))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(worktrees.len(), 1);

    let worktree_directory = PathBuf::from("/worktrees");
    cx_b.update(|cx| {
        repo_b.update(cx, |repo, _| {
            repo.create_worktree(
                git::repository::CreateWorktreeTarget::NewBranch {
                    branch_name: "feature-branch".to_string(),
                    base_sha: Some("abc123".to_string()),
                },
                worktree_directory.join("feature-branch"),
            )
        })
    })
    .await
    .unwrap()
    .unwrap();

    executor.run_until_parked();

    let worktrees = cx_b
        .update(|cx| repo_b.update(cx, |repo, _| repo.worktrees()))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(worktrees.len(), 2);
    assert_eq!(worktrees[1].path, worktree_directory.join("feature-branch"));
    assert_eq!(
        worktrees[1].ref_name,
        Some("refs/heads/feature-branch".into())
    );
    assert_eq!(worktrees[1].sha.as_ref(), "abc123");

    let server_worktrees = {
        let server_repo = server_cx.update(|cx| {
            headless_project.update(cx, |headless_project, cx| {
                headless_project
                    .git_store
                    .read(cx)
                    .repositories()
                    .values()
                    .next()
                    .unwrap()
                    .clone()
            })
        });
        server_cx
            .update(|cx| server_repo.update(cx, |repo, _| repo.worktrees()))
            .await
            .unwrap()
            .unwrap()
    };
    assert_eq!(server_worktrees.len(), 2);
    assert_eq!(
        server_worktrees[1].path,
        worktree_directory.join("feature-branch")
    );

    // Host (client A) renames the worktree via SSH
    let repo_a = cx_a.update(|cx| {
        project_a
            .read(cx)
            .repositories(cx)
            .values()
            .next()
            .unwrap()
            .clone()
    });
    cx_a.update(|cx| {
        repo_a.update(cx, |repository, _| {
            repository.rename_worktree(
                PathBuf::from("/worktrees/feature-branch"),
                PathBuf::from("/worktrees/renamed-branch"),
            )
        })
    })
    .await
    .unwrap()
    .unwrap();

    executor.run_until_parked();

    let host_worktrees = cx_a
        .update(|cx| repo_a.update(cx, |repository, _| repository.worktrees()))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        host_worktrees.len(),
        2,
        "重命名后主机端应仍有 2 个工作树"
    );
    assert_eq!(
        host_worktrees[1].path,
        PathBuf::from("/worktrees/renamed-branch")
    );

    let server_worktrees = {
        let server_repo = server_cx.update(|cx| {
            headless_project.update(cx, |headless_project, cx| {
                headless_project
                    .git_store
                    .read(cx)
                    .repositories()
                    .values()
                    .next()
                    .unwrap()
                    .clone()
            })
        });
        server_cx
            .update(|cx| server_repo.update(cx, |repo, _| repo.worktrees()))
            .await
            .unwrap()
            .unwrap()
    };
    assert_eq!(
        server_worktrees.len(),
        2,
        "重命名后服务器端应仍有 2 个工作树"
    );
    assert_eq!(
        server_worktrees[1].path,
        PathBuf::from("/worktrees/renamed-branch")
    );

    // Host (client A) removes the renamed worktree via SSH
    cx_a.update(|cx| {
        repo_a.update(cx, |repository, _| {
            repository.remove_worktree(PathBuf::from("/worktrees/renamed-branch"), false)
        })
    })
    .await
    .unwrap()
    .unwrap();

    executor.run_until_parked();

    let host_worktrees = cx_a
        .update(|cx| repo_a.update(cx, |repository, _| repository.worktrees()))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        host_worktrees.len(),
        1,
        "移除后主机端应仅剩主工作树"
    );

    let server_worktrees = {
        let server_repo = server_cx.update(|cx| {
            headless_project.update(cx, |headless_project, cx| {
                headless_project
                    .git_store
                    .read(cx)
                    .repositories()
                    .values()
                    .next()
                    .unwrap()
                    .clone()
            })
        });
        server_cx
            .update(|cx| server_repo.update(cx, |repo, _| repo.worktrees()))
            .await
            .unwrap()
            .unwrap()
    };
    assert_eq!(
        server_worktrees.len(),
        1,
        "移除后服务器端应仅剩主工作树"
    );
}

#[gpui::test]
async fn test_ssh_collaboration_formatting_with_prettier(
    executor: BackgroundExecutor,
    cx_a: &mut TestAppContext,
    cx_b: &mut TestAppContext,
    server_cx: &mut TestAppContext,
) {
    cx_a.set_name("a");
    cx_b.set_name("b");
    server_cx.set_name("server");

    cx_a.update(|cx| {
        release_channel::init(semver::Version::new(0, 0, 0), cx);
    });
    server_cx.update(|cx| {
        release_channel::init(semver::Version::new(0, 0, 0), cx);
    });

    let mut server = TestServer::start(executor.clone()).await;
    let client_a = server.create_client(cx_a, "user_a").await;
    let client_b = server.create_client(cx_b, "user_b").await;
    server
        .create_room(&mut [(&client_a, cx_a), (&client_b, cx_b)])
        .await;

    let (opts, server_ssh, _) = RemoteClient::fake_server(cx_a, server_cx);
    let remote_fs = FakeFs::new(server_cx.executor());
    let buffer_text = "let one = \"two\"";
    let prettier_format_suffix = project::TEST_PRETTIER_FORMAT_SUFFIX;
    remote_fs
        .insert_tree(
            path!("/project"),
            serde_json::json!({ "a.ts": buffer_text }),
        )
        .await;

    let test_plugin = "test_plugin";
    let ts_lang = Arc::new(Language::new(
        LanguageConfig {
            name: "TypeScript".into(),
            matcher: (LanguageMatcher {
                path_suffixes: vec!["ts".to_string()],
                ..LanguageMatcher::default()
            })
            .into(),
            ..LanguageConfig::default()
        },
        Some(tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into()),
    ));
    client_a.language_registry().add(ts_lang.clone());
    client_b.language_registry().add(ts_lang.clone());

    let languages = Arc::new(LanguageRegistry::new(server_cx.executor()));
    let mut fake_language_servers = languages.register_fake_lsp(
        "TypeScript",
        FakeLspAdapter {
            prettier_plugins: vec![test_plugin],
            ..Default::default()
        },
    );

    // User A connects to the remote project via SSH.
    server_cx.update(HeadlessProject::init);
    let remote_http_client = Arc::new(BlockedHttpClient);
    let _headless_project = server_cx.new(|cx| {
        HeadlessProject::new(
            HeadlessAppState {
                session: server_ssh,
                fs: remote_fs.clone(),
                http_client: remote_http_client,
                node_runtime: NodeRuntime::unavailable(),
                languages,
                extension_host_proxy: Arc::new(ExtensionHostProxy::new()),
                startup_time: std::time::Instant::now(),
            },
            false,
            cx,
        )
    });

    let client_ssh = RemoteClient::connect_mock(opts, cx_a).await;
    let (project_a, worktree_id) = client_a
        .build_ssh_project(path!("/project"), client_ssh, false, cx_a)
        .await;

    // While the SSH worktree is being scanned, user A shares the remote project.
    let active_call_a = cx_a.read(ActiveCall::global);
    let project_id = active_call_a
        .update(cx_a, |call, cx| call.share_project(project_a.clone(), cx))
        .await
        .unwrap();

    // User B joins the project.
    let project_b = client_b.join_remote_project(project_id, cx_b).await;
    executor.run_until_parked();

    // Opens the buffer and formats it
    let (buffer_b, _handle) = project_b
        .update(cx_b, |p, cx| {
            p.open_buffer_with_lsp((worktree_id, rel_path("a.ts")), cx)
        })
        .await
        .expect("用户 B 打开缓冲区进行格式化");

    cx_a.update(|cx| {
        SettingsStore::update_global(cx, |store, cx| {
            store.update_user_settings(cx, |file| {
                file.project.all_languages.defaults.format_on_save =
                    Some(settings::FormatOnSave::On);
                file.project.all_languages.defaults.formatter = Some(FormatterList::default());
                file.project.all_languages.defaults.prettier = Some(PrettierSettingsContent {
                    allowed: Some(true),
                    ..Default::default()
                });
            });
        });
    });
    cx_b.update(|cx| {
        SettingsStore::update_global(cx, |store, cx| {
            store.update_user_settings(cx, |file| {
                file.project.all_languages.defaults.format_on_save =
                    Some(settings::FormatOnSave::On);
                file.project.all_languages.defaults.formatter = Some(FormatterList::Single(
                    Formatter::LanguageServer(LanguageServerFormatterSpecifier::Current),
                ));
                file.project.all_languages.defaults.prettier = Some(PrettierSettingsContent {
                    allowed: Some(true),
                    ..Default::default()
                });
            });
        });
    });
    let fake_language_server = fake_language_servers.next().await.unwrap();
    fake_language_server.set_request_handler::<lsp::request::Formatting, _, _>(|_, _| async move {
        panic!(
            "意外:Prettier 已启用且语言支持,应优先使用 Prettier"
        )
    });

    project_b
        .update(cx_b, |project, cx| {
            project.format(
                HashSet::from_iter([buffer_b.clone()]),
                LspFormatTarget::Buffers,
                true,
                FormatTrigger::Save,
                cx,
            )
        })
        .await
        .unwrap();

    executor.run_until_parked();
    assert_eq!(
        buffer_b.read_with(cx_b, |buffer, _| buffer.text()),
        buffer_text.to_string() + "\n" + prettier_format_suffix,
        "客户端请求后,Prettier 格式化未应用于客户端缓冲区"
    );

    // User A opens and formats the same buffer too
    let buffer_a = project_a
        .update(cx_a, |p, cx| {
            p.open_buffer((worktree_id, rel_path("a.ts")), cx)
        })
        .await
        .expect("用户 A 打开缓冲区进行格式化");

    cx_a.update(|cx| {
        SettingsStore::update_global(cx, |store, cx| {
            store.update_user_settings(cx, |file| {
                file.project.all_languages.defaults.formatter = Some(FormatterList::default());
                file.project.all_languages.defaults.prettier = Some(PrettierSettingsContent {
                    allowed: Some(true),
                    ..Default::default()
                });
            });
        });
    });
    project_a
        .update(cx_a, |project, cx| {
            project.format(
                HashSet::from_iter([buffer_a.clone()]),
                LspFormatTarget::Buffers,
                true,
                FormatTrigger::Manual,
                cx,
            )
        })
        .await
        .unwrap();

    executor.run_until_parked();
    assert_eq!(
        buffer_b.read_with(cx_b, |buffer, _| buffer.text()),
        buffer_text.to_string() + "\n" + prettier_format_suffix + "\n" + prettier_format_suffix,
        "主机请求后,Prettier 格式化未应用于客户端缓冲区"
    );
}

#[gpui::test(iterations = 10)]
async fn test_ssh_restarting_language_server_replaces_remote_status(
    executor: BackgroundExecutor,
    cx_a: &mut TestAppContext,
    server_cx: &mut TestAppContext,
) {
    cx_a.set_name("a");
    server_cx.set_name("server");

    cx_a.update(|cx| {
        release_channel::init(semver::Version::new(0, 0, 0), cx);
    });
    server_cx.update(|cx| {
        release_channel::init(semver::Version::new(0, 0, 0), cx);
    });

    let mut server = TestServer::start(executor.clone()).await;
    let client_a = server.create_client(cx_a, "user_a").await;
    let log_store = cx_a.update(|cx| log_store::init(false, cx));

    let (opts, server_ssh, _) = RemoteClient::fake_server(cx_a, server_cx);
    let remote_fs = FakeFs::new(server_cx.executor());
    remote_fs
        .insert_tree(path!("/project"), json!({ "a.rs": "fn main() {}" }))
        .await;

    client_a.language_registry().add(rust_lang());

    server_cx.update(HeadlessProject::init);
    let languages = Arc::new(LanguageRegistry::new(server_cx.executor()));
    languages.add(rust_lang());
    let mut fake_language_servers = languages.register_fake_lsp(
        "Rust",
        FakeLspAdapter {
            name: "the-language-server",
            ..Default::default()
        },
    );
    let _headless_project = server_cx.new(|cx| {
        HeadlessProject::new(
            HeadlessAppState {
                session: server_ssh,
                fs: remote_fs.clone(),
                http_client: Arc::new(BlockedHttpClient),
                node_runtime: NodeRuntime::unavailable(),
                languages,
                extension_host_proxy: Arc::new(ExtensionHostProxy::new()),
                startup_time: std::time::Instant::now(),
            },
            false,
            cx,
        )
    });

    let client_ssh = RemoteClient::connect_mock(opts, cx_a).await;
    let (project_a, worktree_id) = client_a
        .build_ssh_project(path!("/project"), client_ssh, false, cx_a)
        .await;
    log_store.update(cx_a, |log_store, cx| log_store.add_project(&project_a, cx));

    let (buffer, _handle) = project_a
        .update(cx_a, |project, cx| {
            project.open_buffer_with_lsp((worktree_id, rel_path("a.rs")), cx)
        })
        .await
        .unwrap();

    let first_server = fake_language_servers.next().await.unwrap();
    let first_server_id = first_server.server.server_id();
    executor.run_until_parked();

    project_a.read_with(cx_a, |project, cx| {
        let statuses = project.language_server_statuses(cx).collect::<Vec<_>>();
        assert_eq!(statuses.len(), 1);
        assert_eq!(statuses[0].0, first_server_id);
        assert_eq!(statuses[0].1.name.0, "the-language-server");
    });
    cx_a.read_global::<GlobalLogStore, _>(|global, cx| {
        let log_store = global.0.read(cx);
        let matching_server_ids = log_store
            .language_servers
            .iter()
            .filter_map(|(server_id, state)| {
                state
                    .name
                    .as_ref()
                    .is_some_and(|name| name.0 == "the-language-server")
                    .then_some(*server_id)
            })
            .collect::<Vec<_>>();
        assert_eq!(matching_server_ids, vec![first_server_id]);
    });

    project_a.update(cx_a, |project, cx| {
        project.restart_language_servers_for_buffers(vec![buffer], HashSet::default(), true, cx);
    });

    let restarted_server = fake_language_servers.next().await.unwrap();
    let restarted_server_id = restarted_server.server.server_id();
    assert_ne!(restarted_server_id, first_server_id);
    executor.run_until_parked();

    project_a.read_with(cx_a, |project, cx| {
        let statuses = project.language_server_statuses(cx).collect::<Vec<_>>();
        assert_eq!(
            statuses.len(),
            1,
            "重启远程语言服务器应替换先前的状态条目"
        );
        assert_eq!(
            statuses[0].0, restarted_server_id,
            "重启远程语言服务器应发布替换后的服务器 ID"
        );
        assert_ne!(
            statuses[0].0, first_server_id,
            "重启远程语言服务器应移除先前的服务器 ID"
        );
        assert_eq!(statuses[0].1.name.0, "the-language-server");
    });
    cx_a.read_global::<GlobalLogStore, _>(|global, cx| {
        let log_store = global.0.read(cx);
        let matching_server_ids = log_store
            .language_servers
            .iter()
            .filter_map(|(server_id, state)| {
                state
                    .name
                    .as_ref()
                    .is_some_and(|name| name.0 == "the-language-server")
                    .then_some(*server_id)
            })
            .collect::<Vec<_>>();
        assert_eq!(
            matching_server_ids,
            vec![restarted_server_id],
            "重启远程语言服务器应替换旧的日志存储条目"
        );
        assert!(
            !log_store.language_servers.contains_key(&first_server_id),
            "重启远程语言服务器应移除先前的日志存储条目"
        );
    });
}

#[gpui::test]
async fn test_remote_server_debugger(
    cx_a: &mut TestAppContext,
    server_cx: &mut TestAppContext,
    executor: BackgroundExecutor,
) {
    cx_a.update(|cx| {
        release_channel::init(semver::Version::new(0, 0, 0), cx);
        command_palette_hooks::init(cx);
        zlog::init_test();
        dap_adapters::init(cx);
    });
    server_cx.update(|cx| {
        release_channel::init(semver::Version::new(0, 0, 0), cx);
        dap_adapters::init(cx);
    });
    let (opts, server_ssh, _) = RemoteClient::fake_server(cx_a, server_cx);
    let remote_fs = FakeFs::new(server_cx.executor());
    remote_fs
        .insert_tree(
            path!("/code"),
            json!({
                "lib.rs": "fn one() -> usize { 1 }"
            }),
        )
        .await;

    // User A connects to the remote project via SSH.
    server_cx.update(HeadlessProject::init);
    let remote_http_client = Arc::new(BlockedHttpClient);
    let node = NodeRuntime::unavailable();
    let languages = Arc::new(LanguageRegistry::new(server_cx.executor()));
    let _headless_project = server_cx.new(|cx| {
        HeadlessProject::new(
            HeadlessAppState {
                session: server_ssh,
                fs: remote_fs.clone(),
                http_client: remote_http_client,
                node_runtime: node,
                languages,
                extension_host_proxy: Arc::new(ExtensionHostProxy::new()),
                startup_time: std::time::Instant::now(),
            },
            false,
            cx,
        )
    });

    let client_ssh = RemoteClient::connect_mock(opts, cx_a).await;
    let mut server = TestServer::start(server_cx.executor()).await;
    let client_a = server.create_client(cx_a, "user_a").await;
    cx_a.update(|cx| {
        debugger_ui::init(cx);
        command_palette_hooks::init(cx);
    });
    let (project_a, _) = client_a
        .build_ssh_project(path!("/code"), client_ssh.clone(), false, cx_a)
        .await;

    let (workspace, cx_a) = client_a.build_workspace(&project_a, cx_a);

    let debugger_panel = workspace
        .update_in(cx_a, |_workspace, window, cx| {
            cx.spawn_in(window, DebugPanel::load)
        })
        .await
        .unwrap();

    workspace.update_in(cx_a, |workspace, window, cx| {
        workspace.add_panel(debugger_panel, window, cx);
    });

    cx_a.run_until_parked();
    let debug_panel = workspace
        .update(cx_a, |workspace, cx| workspace.panel::<DebugPanel>(cx))
        .unwrap();

    let workspace_window = cx_a
        .window_handle()
        .downcast::<workspace::MultiWorkspace>()
        .unwrap();

    let session = debugger_ui::tests::start_debug_session(&workspace_window, cx_a, |_| {}).unwrap();
    cx_a.run_until_parked();
    debug_panel.update(cx_a, |debug_panel, cx| {
        assert_eq!(
            debug_panel.active_session().unwrap().read(cx).session(cx),
            session.clone()
        )
    });

    session.update(
        cx_a,
        |session: &mut project::debugger::session::Session, _| {
            assert_eq!(session.binary().unwrap().command.as_deref(), Some("mock"));
        },
    );

    let shutdown_session = workspace.update(cx_a, |workspace, cx| {
        workspace.project().update(cx, |project, cx| {
            project.dap_store().update(cx, |dap_store, cx| {
                dap_store.shutdown_session(session.read(cx).session_id(), cx)
            })
        })
    });

    client_ssh.update(cx_a, |a, _| {
        a.shutdown_processes(Some(proto::ShutdownRemoteServer {}), executor)
    });

    shutdown_session.await.unwrap();
}

#[gpui::test]
async fn test_slow_adapter_startup_retries(
    cx_a: &mut TestAppContext,
    server_cx: &mut TestAppContext,
    executor: BackgroundExecutor,
) {
    cx_a.update(|cx| {
        release_channel::init(semver::Version::new(0, 0, 0), cx);
        command_palette_hooks::init(cx);
        zlog::init_test();
        dap_adapters::init(cx);
    });
    server_cx.update(|cx| {
        release_channel::init(semver::Version::new(0, 0, 0), cx);
        dap_adapters::init(cx);
    });
    let (opts, server_ssh, _) = RemoteClient::fake_server(cx_a, server_cx);
    let remote_fs = FakeFs::new(server_cx.executor());
    remote_fs
        .insert_tree(
            path!("/code"),
            json!({
                "lib.rs": "fn one() -> usize { 1 }"
            }),
        )
        .await;

    // User A connects to the remote project via SSH.
    server_cx.update(HeadlessProject::init);
    let remote_http_client = Arc::new(BlockedHttpClient);
    let node = NodeRuntime::unavailable();
    let languages = Arc::new(LanguageRegistry::new(server_cx.executor()));
    let _headless_project = server_cx.new(|cx| {
        HeadlessProject::new(
            HeadlessAppState {
                session: server_ssh,
                fs: remote_fs.clone(),
                http_client: remote_http_client,
                node_runtime: node,
                languages,
                extension_host_proxy: Arc::new(ExtensionHostProxy::new()),
                startup_time: std::time::Instant::now(),
            },
            false,
            cx,
        )
    });

    let client_ssh = RemoteClient::connect_mock(opts, cx_a).await;
    let mut server = TestServer::start(server_cx.executor()).await;
    let client_a = server.create_client(cx_a, "user_a").await;
    cx_a.update(|cx| {
        debugger_ui::init(cx);
        command_palette_hooks::init(cx);
    });
    let (project_a, _) = client_a
        .build_ssh_project(path!("/code"), client_ssh.clone(), false, cx_a)
        .await;

    let (workspace, cx_a) = client_a.build_workspace(&project_a, cx_a);

    let debugger_panel = workspace
        .update_in(cx_a, |_workspace, window, cx| {
            cx.spawn_in(window, DebugPanel::load)
        })
        .await
        .unwrap();

    workspace.update_in(cx_a, |workspace, window, cx| {
        workspace.add_panel(debugger_panel, window, cx);
    });

    cx_a.run_until_parked();
    let debug_panel = workspace
        .update(cx_a, |workspace, cx| workspace.panel::<DebugPanel>(cx))
        .unwrap();

    let workspace_window = cx_a
        .window_handle()
        .downcast::<workspace::MultiWorkspace>()
        .unwrap();

    let count = Arc::new(AtomicUsize::new(0));
    let session = debugger_ui::tests::start_debug_session_with(
        &workspace_window,
        cx_a,
        DebugTaskDefinition {
            adapter: "fake-adapter".into(),
            label: "test".into(),
            config: json!({
                "request": "launch"
            }),
            tcp_connection: Some(TcpArgumentsTemplate {
                port: None,
                host: None,
                timeout: None,
            }),
        },
        move |client| {
            let count = count.clone();
            client.on_request_ext::<dap::requests::Initialize, _>(move |_seq, _request| {
                if count.fetch_add(1, std::sync::atomic::Ordering::SeqCst) < 5 {
                    return RequestHandling::Exit;
                }
                RequestHandling::Respond(Ok(Capabilities::default()))
            });
        },
    )
    .unwrap();
    cx_a.run_until_parked();

    let client = session.update(
        cx_a,
        |session: &mut project::debugger::session::Session, _| session.adapter_client().unwrap(),
    );
    client
        .fake_event(dap::messages::Events::Stopped(dap::StoppedEvent {
            reason: dap::StoppedEventReason::Pause,
            description: None,
            thread_id: Some(1),
            preserve_focus_hint: None,
            text: None,
            all_threads_stopped: None,
            hit_breakpoint_ids: None,
        }))
        .await;

    cx_a.run_until_parked();

    let active_session = debug_panel
        .update(cx_a, |this, _| this.active_session())
        .unwrap();

    let running_state = active_session.update(cx_a, |active_session, _| {
        active_session.running_state().clone()
    });

    assert_eq!(
        client.id(),
        running_state.read_with(cx_a, |running_state, _| running_state.session_id())
    );
    assert_eq!(
        ThreadId(1),
        running_state.read_with(cx_a, |running_state, _| running_state
            .selected_thread_id()
            .unwrap())
    );

    let shutdown_session = workspace.update(cx_a, |workspace, cx| {
        workspace.project().update(cx, |project, cx| {
            project.dap_store().update(cx, |dap_store, cx| {
                dap_store.shutdown_session(session.read(cx).session_id(), cx)
            })
        })
    });

    client_ssh.update(cx_a, |a, _| {
        a.shutdown_processes(Some(proto::ShutdownRemoteServer {}), executor)
    });

    shutdown_session.await.unwrap();
}

#[gpui::test]
async fn test_ssh_remote_worktree_trust(cx_a: &mut TestAppContext, server_cx: &mut TestAppContext) {
    cx_a.update(|cx| {
        release_channel::init(semver::Version::new(0, 0, 0), cx);
        project::trusted_worktrees::init(HashMap::default(), cx);
    });
    server_cx.update(|cx| {
        release_channel::init(semver::Version::new(0, 0, 0), cx);
        project::trusted_worktrees::init(HashMap::default(), cx);
    });

    let mut server = TestServer::start(cx_a.executor().clone()).await;
    let client_a = server.create_client(cx_a, "user_a").await;

    let server_name = "override-rust-analyzer";
    let lsp_inlay_hint_request_count = Arc::new(AtomicUsize::new(0));

    let (opts, server_ssh, _) = RemoteClient::fake_server(cx_a, server_cx);
    let remote_fs = FakeFs::new(server_cx.executor());
    remote_fs
        .insert_tree(
            path!("/projects"),
            json!({
                "project_a": {
                    ".zed": {
                        "settings.json": r#"{"languages":{"Rust":{"language_servers":["override-rust-analyzer"]}}}"#
                    },
                    "main.rs": "fn main() {}"
                },
                "project_b": { "lib.rs": "pub fn lib() {}" }
            }),
        )
        .await;

    server_cx.update(HeadlessProject::init);
    let remote_http_client = Arc::new(BlockedHttpClient);
    let node = NodeRuntime::unavailable();
    let languages = Arc::new(LanguageRegistry::new(server_cx.executor()));
    languages.add(rust_lang());

    let capabilities = lsp::ServerCapabilities {
        inlay_hint_provider: Some(lsp::OneOf::Left(true)),
        ..lsp::ServerCapabilities::default()
    };
    let mut fake_language_servers = languages.register_fake_lsp(
        "Rust",
        FakeLspAdapter {
            name: server_name,
            capabilities: capabilities.clone(),
            initializer: Some(Box::new({
                let lsp_inlay_hint_request_count = lsp_inlay_hint_request_count.clone();
                move |fake_server| {
                    let lsp_inlay_hint_request_count = lsp_inlay_hint_request_count.clone();
                    fake_server.set_request_handler::<lsp::request::InlayHintRequest, _, _>(
                        move |_params, _| {
                            lsp_inlay_hint_request_count.fetch_add(1, Ordering::Release);
                            async move {
                                Ok(Some(vec![lsp::InlayHint {
                                    position: lsp::Position::new(0, 0),
                                    label: lsp::InlayHintLabel::String("hint".to_string()),
                                    kind: None,
                                    text_edits: None,
                                    tooltip: None,
                                    padding_left: None,
                                    padding_right: None,
                                    data: None,
                                }]))
                            }
                        },
                    );
                }
            })),
            ..FakeLspAdapter::default()
        },
    );

    let _headless_project = server_cx.new(|cx| {
        HeadlessProject::new(
            HeadlessAppState {
                session: server_ssh,
                fs: remote_fs.clone(),
                http_client: remote_http_client,
                node_runtime: node,
                languages,
                extension_host_proxy: Arc::new(ExtensionHostProxy::new()),
                startup_time: std::time::Instant::now(),
            },
            true,
            cx,
        )
    });

    let client_ssh = RemoteClient::connect_mock(opts, cx_a).await;
    let (project_a, worktree_id_a) = client_a
        .build_ssh_project(path!("/projects/project_a"), client_ssh.clone(), true, cx_a)
        .await;

    cx_a.update(|cx| {
        release_channel::init(semver::Version::new(0, 0, 0), cx);

        SettingsStore::update_global(cx, |store, cx| {
            store.update_user_settings(cx, |settings| {
                let language_settings = &mut settings.project.all_languages.defaults;
                language_settings.inlay_hints = Some(InlayHintSettingsContent {
                    enabled: Some(true),
                    ..InlayHintSettingsContent::default()
                })
            });
        });
    });

    project_a
        .update(cx_a, |project, cx| {
            project.languages().add(rust_lang());
            project.languages().register_fake_lsp_adapter(
                "Rust",
                FakeLspAdapter {
                    name: server_name,
                    capabilities,
                    ..FakeLspAdapter::default()
                },
            );
            project.find_or_create_worktree(path!("/projects/project_b"), true, cx)
        })
        .await
        .unwrap();

    cx_a.run_until_parked();

    let worktree_ids = project_a.read_with(cx_a, |project, cx| {
        project
            .worktrees(cx)
            .map(|wt| wt.read(cx).id())
            .collect::<Vec<_>>()
    });
    assert_eq!(worktree_ids.len(), 2);

    let trusted_worktrees =
        cx_a.update(|cx| TrustedWorktrees::try_get_global(cx).expect("trust global should exist"));
    let worktree_store = project_a.read_with(cx_a, |project, _| project.worktree_store());

    let can_trust_a = trusted_worktrees.update(cx_a, |store, cx| {
        store.can_trust(&worktree_store, worktree_ids[0], cx)
    });
    let can_trust_b = trusted_worktrees.update(cx_a, |store, cx| {
        store.can_trust(&worktree_store, worktree_ids[1], cx)
    });
    assert!(!can_trust_a, "project_a 初始应处于受限状态");
    assert!(!can_trust_b, "project_b 初始应处于受限状态");

    let has_restricted = trusted_worktrees.read_with(cx_a, |store, cx| {
        store.has_restricted_worktrees(&worktree_store, cx)
    });
    assert!(has_restricted, "应存在受限工作树");

    let buffer_before_approval = project_a
        .update(cx_a, |project, cx| {
            project.open_buffer((worktree_id_a, rel_path("main.rs")), cx)
        })
        .await
        .unwrap();

    let (editor, cx_a) = cx_a.add_window_view(|window, cx| {
        Editor::new(
            EditorMode::full(),
            cx.new(|cx| MultiBuffer::singleton(buffer_before_approval.clone(), cx)),
            Some(project_a.clone()),
            window,
            cx,
        )
    });
    cx_a.run_until_parked();
    let fake_language_server = fake_language_servers.next();

    cx_a.read(|cx| {
        assert_eq!(
            LanguageSettings::for_buffer(buffer_before_approval.read(cx), cx).language_servers,
            [ConfiguredLanguageServer::new("...")],
            "在信任批准前,远程 .zed/设置.json 不得同步"
        )
    });

    editor.update_in(cx_a, |editor, window, cx| {
        editor.handle_input("1", window, cx);
    });
    cx_a.run_until_parked();
    cx_a.executor().advance_clock(Duration::from_secs(1));
    assert_eq!(
        lsp_inlay_hint_request_count.load(Ordering::Acquire),
        0,
        "在信任批准前不得查询内联提示"
    );

    trusted_worktrees.update(cx_a, |store, cx| {
        store.trust(
            &worktree_store,
            HashSet::from_iter([PathTrust::Worktree(worktree_ids[0])]),
            cx,
        );
    });
    cx_a.run_until_parked();

    cx_a.read(|cx| {
        assert_eq!(
            LanguageSettings::for_buffer(buffer_before_approval.read(cx), cx).language_servers,
            [ConfiguredLanguageServer::new("override-rust-analyzer")],
            "信任批准后,远程 .zed/设置.json 应同步"
        )
    });
    let _fake_language_server = fake_language_server.await.unwrap();
    editor.update_in(cx_a, |editor, window, cx| {
        editor.handle_input("1", window, cx);
    });
    cx_a.run_until_parked();
    cx_a.executor().advance_clock(Duration::from_secs(1));
    assert!(
        lsp_inlay_hint_request_count.load(Ordering::Acquire) > 0,
        "应在信任批准后查询内联提示"
    );

    let can_trust_a = trusted_worktrees.update(cx_a, |store, cx| {
        store.can_trust(&worktree_store, worktree_ids[0], cx)
    });
    let can_trust_b = trusted_worktrees.update(cx_a, |store, cx| {
        store.can_trust(&worktree_store, worktree_ids[1], cx)
    });
    assert!(can_trust_a, "调用 trust() 后 project_a 应被信任");
    assert!(!can_trust_b, "project_b 仍应处于受限状态");

    trusted_worktrees.update(cx_a, |store, cx| {
        store.trust(
            &worktree_store,
            HashSet::from_iter([PathTrust::Worktree(worktree_ids[1])]),
            cx,
        );
    });

    let can_trust_a = trusted_worktrees.update(cx_a, |store, cx| {
        store.can_trust(&worktree_store, worktree_ids[0], cx)
    });
    let can_trust_b = trusted_worktrees.update(cx_a, |store, cx| {
        store.can_trust(&worktree_store, worktree_ids[1], cx)
    });
    assert!(can_trust_a, "project_a 应保持受信任状态");
    assert!(can_trust_b, "project_b 现在应被信任");

    let has_restricted_after = trusted_worktrees.read_with(cx_a, |store, cx| {
        store.has_restricted_worktrees(&worktree_store, cx)
    });
    assert!(
        !has_restricted_after,
        "信任两者后,应不存在受限的工作树"
    );
}

#[gpui::test]
async fn test_ssh_document_links_resolve(
    cx_a: &mut TestAppContext,
    server_cx: &mut TestAppContext,
) {
    cx_a.update(|cx| {
        release_channel::init(semver::Version::new(0, 0, 0), cx);
        project::trusted_worktrees::init(HashMap::default(), cx);
    });
    server_cx.update(|cx| {
        release_channel::init(semver::Version::new(0, 0, 0), cx);
        project::trusted_worktrees::init(HashMap::default(), cx);
    });

    let mut server = TestServer::start(cx_a.executor()).await;
    let client_a = server.create_client(cx_a, "user_a").await;

    let document_link_count = Arc::new(AtomicUsize::new(0));
    let resolve_count = Arc::new(AtomicUsize::new(0));

    let (opts, server_ssh, _) = RemoteClient::fake_server(cx_a, server_cx);
    let remote_fs = FakeFs::new(server_cx.executor());
    remote_fs
        .insert_tree(
            path!("/code"),
            json!({
                "main.rs": "// see LICENSE for details\nfn main() {}",
                "other.rs": "fn other() {}\n",
            }),
        )
        .await;

    server_cx.update(HeadlessProject::init);
    let remote_http_client = Arc::new(BlockedHttpClient);
    let node = NodeRuntime::unavailable();
    let languages = Arc::new(LanguageRegistry::new(server_cx.executor()));
    languages.add(rust_lang());

    let capabilities = lsp::ServerCapabilities {
        document_link_provider: Some(lsp::DocumentLinkOptions {
            resolve_provider: Some(true),
            work_done_progress_options: lsp::WorkDoneProgressOptions::default(),
        }),
        ..lsp::ServerCapabilities::default()
    };
    let other_path_for_remote = path!("/code/other.rs");
    let mut fake_language_servers = languages.register_fake_lsp(
        "Rust",
        FakeLspAdapter {
            capabilities: capabilities.clone(),
            initializer: Some(Box::new({
                let document_link_count = document_link_count.clone();
                let resolve_count = resolve_count.clone();
                move |fake_server| {
                    let document_link_count = document_link_count.clone();
                    fake_server.set_request_handler::<lsp::request::DocumentLinkRequest, _, _>({
                        move |_params, _| {
                            let document_link_count = document_link_count.clone();
                            async move {
                                document_link_count.fetch_add(1, Ordering::Release);
                                Ok(Some(vec![lsp::DocumentLink {
                                    range: lsp::Range {
                                        start: lsp::Position {
                                            line: 0,
                                            character: 7,
                                        },
                                        end: lsp::Position {
                                            line: 0,
                                            character: 14,
                                        },
                                    },
                                    target: None,
                                    tooltip: None,
                                    data: Some(serde_json::json!({"id": 7})),
                                }]))
                            }
                        }
                    });
                    let resolve_count = resolve_count.clone();
                    fake_server.set_request_handler::<lsp::request::DocumentLinkResolve, _, _>({
                        move |link, _| {
                            let resolve_count = resolve_count.clone();
                            async move {
                                resolve_count.fetch_add(1, Ordering::Release);
                                Ok(lsp::DocumentLink {
                                    range: link.range,
                                    target: Some(
                                        lsp::Uri::from_file_path(other_path_for_remote).unwrap(),
                                    ),
                                    tooltip: Some("打开 other.rs".into()),
                                    data: None,
                                })
                            }
                        }
                    });
                }
            })),
            ..FakeLspAdapter::default()
        },
    );

    let _headless_project = server_cx.new(|cx| {
        HeadlessProject::new(
            HeadlessAppState {
                session: server_ssh,
                fs: remote_fs.clone(),
                http_client: remote_http_client,
                node_runtime: node,
                languages,
                extension_host_proxy: Arc::new(ExtensionHostProxy::new()),
                startup_time: std::time::Instant::now(),
            },
            true,
            cx,
        )
    });

    let client_ssh = RemoteClient::connect_mock(opts, cx_a).await;
    let (project_a, worktree_id) = client_a
        .build_ssh_project(path!("/code"), client_ssh.clone(), true, cx_a)
        .await;

    cx_a.run_until_parked();
    let trusted_worktrees =
        cx_a.update(|cx| TrustedWorktrees::try_get_global(cx).expect("全局信任"));
    let worktree_store = project_a.read_with(cx_a, |project, _| project.worktree_store());
    trusted_worktrees.update(cx_a, |store, cx| {
        store.trust(
            &worktree_store,
            HashSet::from_iter([PathTrust::Worktree(worktree_id)]),
            cx,
        );
    });
    cx_a.run_until_parked();

    cx_a.update(|cx| {
        SettingsStore::update_global(cx, |store, cx| {
            store.update_user_settings(cx, |settings| {
                settings.editor.lsp_document_links = Some(true);
            });
        });
    });

    project_a.update(cx_a, |project, _| {
        project.languages().add(rust_lang());
        project.languages().register_fake_lsp_adapter(
            "Rust",
            FakeLspAdapter {
                capabilities,
                ..FakeLspAdapter::default()
            },
        );
    });

    let (buffer, _registration) = project_a
        .update(cx_a, |project, cx| {
            project.open_buffer_with_lsp((worktree_id, rel_path("main.rs")), cx)
        })
        .await
        .unwrap();
    let buffer_id = buffer.read_with(cx_a, |buffer, _| buffer.remote_id());
    cx_a.run_until_parked();
    let _fake_language_server = fake_language_servers.next().await.unwrap();
    cx_a.run_until_parked();

    let (editor, cx_a) = cx_a.add_window_view(|window, cx| {
        Editor::new(
            EditorMode::full(),
            cx.new(|cx| MultiBuffer::singleton(buffer.clone(), cx)),
            Some(project_a.clone()),
            window,
            cx,
        )
    });
    cx_a.executor()
        .advance_clock(LSP_REQUEST_DEBOUNCE_TIMEOUT + Duration::from_millis(100));
    cx_a.run_until_parked();

    let fetched = project_a.read_with(cx_a, |project, cx| {
        project
            .lsp_store()
            .read(cx)
            .document_links_for_buffer(buffer_id)
            .unwrap_or_default()
    });
    assert_eq!(
        fetched.values().map(|links| links.len()).sum::<usize>(),
        1,
        "编辑器应通过 SSH 自动拉取单个文档链接"
    );
    assert!(
        document_link_count.load(Ordering::Acquire) >= 1,
        "远程 LSP 应已处理获取请求"
    );

    let unresolved = fetched
        .values()
        .flat_map(|per_server| per_server.values())
        .next()
        .expect("本地缓存应镜像远程文档链接");
    assert!(
        !unresolved.resolved,
        "新获取的链接必须保持未解析状态"
    );

    let anchor = buffer.read_with(cx_a, |buffer, _| buffer.anchor_after(10));
    let resolved = editor
        .update(cx_a, |editor, cx| {
            editor.document_links_at(buffer.clone(), anchor, cx)
        })
        .expect("编辑器应在光标处显示缓存的文档链接")
        .await;
    cx_a.run_until_parked();

    assert_eq!(
        resolved.len(),
        1,
        "编辑器应在光标处显示恰好一个已解析的链接"
    );
    assert!(
        resolve_count.load(Ordering::Acquire) >= 1,
        "本地解析应通过 SSH 转发并在远程 LSP 上执行"
    );

    let other_uri = lsp::Uri::from_file_path(path!("/code/other.rs"))
        .unwrap()
        .to_string();
    let links = project_a.read_with(cx_a, |project, cx| {
        project
            .lsp_store()
            .read(cx)
            .document_links_for_buffer(buffer_id)
            .unwrap_or_default()
    });
    assert_eq!(
        1,
        links.values().map(|m| m.len()).sum::<usize>(),
        "本地缓存应镜像单个文档链接"
    );
    let link = links
        .values()
        .flat_map(|per_server| per_server.values())
        .next()
        .expect("本地缓存应包含镜像的链接");
    assert_eq!(
        link.target.as_deref(),
        Some(other_uri.as_str()),
        "本地应看到在远程解析的 file:// 目标"
    );
    assert_eq!(link.tooltip.as_deref(), Some("打开 other.rs"));

    let executor = cx_a.executor();
    client_ssh.update(cx_a, |a, _| {
        a.shutdown_processes(Some(proto::ShutdownRemoteServer {}), executor)
    });
}
