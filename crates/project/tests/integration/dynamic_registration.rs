//! Tests for dynamic (un)registration of language server capabilities, handled in
//! `project::lsp_store::dynamic_registration`.

use std::collections::BTreeMap;

use pretty_assertions::assert_eq;

use super::*;

#[gpui::test]
async fn test_dynamic_semantic_tokens_registration(cx: &mut gpui::TestAppContext) {
    init_test(cx);

    let fs = FakeFs::new(cx.executor());
    fs.insert_tree(
        path!("/the-root"),
        json!({
            "a.rs": "fn main() {}",
        }),
    )
    .await;

    let project = Project::test(fs.clone(), [path!("/the-root").as_ref()], cx).await;
    let language_registry = project.read_with(cx, |project, _| project.languages().clone());
    language_registry.add(rust_lang());
    let mut fake_servers = language_registry.register_fake_lsp(
        "Rust",
        FakeLspAdapter {
            name: "the-language-server",
            // Crucially, no `semantic_tokens_provider` is advertised statically; the
            // server only offers it through dynamic registration (as Roslyn does).
            ..FakeLspAdapter::default()
        },
    );

    let _buffer = project
        .update(cx, |project, cx| {
            project.open_local_buffer_with_lsp(path!("/the-root/a.rs"), cx)
        })
        .await
        .unwrap();

    let fake_server = fake_servers.next().await.unwrap();
    let server_id = fake_server.server.server_id();
    cx.executor().run_until_parked();

    let semantic_tokens_provider = |cx: &mut gpui::TestAppContext| {
        project.read_with(cx, |project, cx| {
            project
                .lsp_store()
                .read(cx)
                .lsp_server_capabilities
                .get(&server_id)
                .and_then(|capabilities| capabilities.semantic_tokens_provider.clone())
        })
    };

    assert!(
        semantic_tokens_provider(cx).is_none(),
        "服务器在动态注册之前不应通告语义令牌"
    );

    fake_server
        .request::<lsp::request::RegisterCapability>(
            lsp::RegistrationParams {
                registrations: vec![lsp::Registration {
                    id: "semantic-tokens".to_string(),
                    method: "textDocument/semanticTokens".to_string(),
                    register_options: serde_json::to_value(
                        lsp::SemanticTokensRegistrationOptions {
                            text_document_registration_options:
                                lsp::TextDocumentRegistrationOptions {
                                    document_selector: None,
                                },
                            semantic_tokens_options: lsp::SemanticTokensOptions {
                                legend: lsp::SemanticTokensLegend {
                                    token_types: vec!["keyword".into(), "variable".into()],
                                    token_modifiers: vec![],
                                },
                                full: Some(lsp::SemanticTokensFullOptions::Bool(true)),
                                ..lsp::SemanticTokensOptions::default()
                            },
                            static_registration_options: lsp::StaticRegistrationOptions {
                                id: None,
                            },
                        },
                    )
                    .ok(),
                }],
            },
            DEFAULT_LSP_REQUEST_TIMEOUT,
        )
        .await
        .into_response()
        .unwrap();
    cx.executor().run_until_parked();

    let provider = semantic_tokens_provider(cx)
        .expect("动态注册后应设置语义令牌提供程序");
    // The capability round-trips through capability-sync serialization, which may
    // normalize the registration options into plain options; either shape is fine
    // as long as the legend survives.
    let legend = match provider {
        lsp::SemanticTokensServerCapabilities::SemanticTokensOptions(options) => options.legend,
        lsp::SemanticTokensServerCapabilities::SemanticTokensRegistrationOptions(options) => {
            options.semantic_tokens_options.legend
        }
    };
    assert_eq!(
        legend.token_types,
        vec!["keyword".into(), "variable".into()],
    );

    fake_server
        .request::<lsp::request::UnregisterCapability>(
            lsp::UnregistrationParams {
                unregisterations: vec![lsp::Unregistration {
                    id: "semantic-tokens".to_string(),
                    method: "textDocument/semanticTokens".to_string(),
                }],
            },
            DEFAULT_LSP_REQUEST_TIMEOUT,
        )
        .await
        .into_response()
        .unwrap();
    cx.executor().run_until_parked();

    assert!(
        semantic_tokens_provider(cx).is_none(),
        "注销后应清除语义令牌提供程序"
    );
}

#[gpui::test]
async fn test_multi_registration_inlay_hint(cx: &mut gpui::TestAppContext) {
    init_test(cx);
    let (project, fake_server) =
        setup_dynamic_registration_test(cx, lsp::ServerCapabilities::default()).await;
    let server_id = fake_server.server.server_id();
    let method = "textDocument/inlayHint";

    let options_a = lsp::InlayHintOptions {
        resolve_provider: Some(true),
        ..lsp::InlayHintOptions::default()
    };
    let options_b = lsp::InlayHintOptions {
        resolve_provider: Some(false),
        ..lsp::InlayHintOptions::default()
    };

    assert_eq!(
        server_capabilities(&project, server_id, cx).inlay_hint_provider,
        None,
        "在任何注册之前不应有内联提示提供程序",
    );

    register_capability(
        &fake_server,
        method,
        "inlay-hint-a",
        serde_json::to_value(&options_a).ok(),
    )
    .await;
    cx.executor().run_until_parked();
    assert_eq!(
        server_capabilities(&project, server_id, cx).inlay_hint_provider,
        Some(lsp::OneOf::Right(
            lsp::InlayHintServerCapabilities::Options(options_a.clone())
        )),
        "第一次注册后应使用第一次注册的选项",
    );

    register_capability(
        &fake_server,
        method,
        "inlay-hint-b",
        serde_json::to_value(&options_b).ok(),
    )
    .await;
    cx.executor().run_until_parked();
    assert_eq!(
        server_capabilities(&project, server_id, cx).inlay_hint_provider,
        Some(lsp::OneOf::Right(
            lsp::InlayHintServerCapabilities::Options(options_b)
        )),
        "expected the second registration's options after the second registration",
    );

    unregister_capabilities(&fake_server, method, &["inlay-hint-b"]).await;
    cx.executor().run_until_parked();
    assert_eq!(
        server_capabilities(&project, server_id, cx).inlay_hint_provider,
        Some(lsp::OneOf::Right(
            lsp::InlayHintServerCapabilities::Options(options_a)
        )),
        "expected the remaining registration's options to be restored",
    );

    unregister_capabilities(&fake_server, method, &["inlay-hint-a"]).await;
    cx.executor().run_until_parked();
    assert_eq!(
        server_capabilities(&project, server_id, cx).inlay_hint_provider,
        None,
        "注销最后一个注册后应清除内联提示提供程序",
    );
}

#[gpui::test]
async fn test_multi_registration_code_lens(cx: &mut gpui::TestAppContext) {
    init_test(cx);
    let (project, fake_server) =
        setup_dynamic_registration_test(cx, lsp::ServerCapabilities::default()).await;
    let server_id = fake_server.server.server_id();
    let method = "textDocument/codeLens";

    let options_a = lsp::CodeLensOptions {
        resolve_provider: Some(true),
    };
    let options_b = lsp::CodeLensOptions {
        resolve_provider: Some(false),
    };

    assert_eq!(
        server_capabilities(&project, server_id, cx).code_lens_provider,
        None,
        "在任何注册之前不应有代码透镜提供程序",
    );

    register_capability(
        &fake_server,
        method,
        "code-lens-a",
        serde_json::to_value(options_a).ok(),
    )
    .await;
    register_capability(
        &fake_server,
        method,
        "code-lens-b",
        serde_json::to_value(options_b).ok(),
    )
    .await;
    cx.executor().run_until_parked();
    assert_eq!(
        server_capabilities(&project, server_id, cx).code_lens_provider,
        Some(options_b),
        "expected the second registration's options after two registrations",
    );

    unregister_capabilities(&fake_server, method, &["code-lens-b"]).await;
    cx.executor().run_until_parked();
    assert_eq!(
        server_capabilities(&project, server_id, cx).code_lens_provider,
        Some(options_a),
        "expected the remaining registration's options to be restored",
    );

    unregister_capabilities(&fake_server, method, &["code-lens-a"]).await;
    cx.executor().run_until_parked();
    assert_eq!(
        server_capabilities(&project, server_id, cx).code_lens_provider,
        None,
        "注销最后一个注册后应清除代码透镜提供程序",
    );
}

#[gpui::test]
async fn test_multi_registration_document_symbol(cx: &mut gpui::TestAppContext) {
    init_test(cx);
    let (project, fake_server) =
        setup_dynamic_registration_test(cx, lsp::ServerCapabilities::default()).await;
    let server_id = fake_server.server.server_id();
    let method = "textDocument/documentSymbol";

    let options_b = lsp::DocumentSymbolOptions {
        label: Some("custom".to_string()),
        work_done_progress_options: lsp::WorkDoneProgressOptions::default(),
    };

    assert_eq!(
        server_capabilities(&project, server_id, cx).document_symbol_provider,
        None,
        "在任何注册之前不应有文档符号提供程序",
    );

    register_capability(&fake_server, method, "document-symbol-a", None).await;
    register_capability(
        &fake_server,
        method,
        "document-symbol-b",
        serde_json::to_value(&options_b).ok(),
    )
    .await;
    cx.executor().run_until_parked();
    assert_eq!(
        server_capabilities(&project, server_id, cx).document_symbol_provider,
        Some(lsp::OneOf::Right(options_b)),
        "expected the second registration's options after two registrations",
    );

    unregister_capabilities(&fake_server, method, &["document-symbol-b"]).await;
    cx.executor().run_until_parked();
    assert_eq!(
        server_capabilities(&project, server_id, cx).document_symbol_provider,
        Some(lsp::OneOf::Left(true)),
        "expected the remaining registration's options to be restored",
    );

    unregister_capabilities(&fake_server, method, &["document-symbol-a"]).await;
    cx.executor().run_until_parked();
    assert_eq!(
        server_capabilities(&project, server_id, cx).document_symbol_provider,
        None,
        "注销最后一个注册后应清除文档符号提供程序",
    );
}

#[gpui::test]
async fn test_multi_registration_restores_static_capability(cx: &mut gpui::TestAppContext) {
    init_test(cx);
    let (project, fake_server) = setup_dynamic_registration_test(
        cx,
        lsp::ServerCapabilities {
            inlay_hint_provider: Some(lsp::OneOf::Left(true)),
            ..lsp::ServerCapabilities::default()
        },
    )
    .await;
    let server_id = fake_server.server.server_id();
    let method = "textDocument/inlayHint";

    assert_eq!(
        server_capabilities(&project, server_id, cx).inlay_hint_provider,
        Some(lsp::OneOf::Left(true)),
        "在任何动态注册之前应存在静态声明的内联提示提供程序",
    );

    let dynamic_options = lsp::InlayHintOptions {
        resolve_provider: Some(true),
        ..lsp::InlayHintOptions::default()
    };
    register_capability(
        &fake_server,
        method,
        "inlay-hint-dynamic",
        serde_json::to_value(&dynamic_options).ok(),
    )
    .await;
    cx.executor().run_until_parked();
    assert_eq!(
        server_capabilities(&project, server_id, cx).inlay_hint_provider,
        Some(lsp::OneOf::Right(
            lsp::InlayHintServerCapabilities::Options(dynamic_options)
        )),
        "动态注册的选项应覆盖静态能力",
    );

    unregister_capabilities(&fake_server, method, &["unknown-id", "inlay-hint-dynamic"]).await;
    cx.executor().run_until_parked();
    assert_eq!(
        server_capabilities(&project, server_id, cx).inlay_hint_provider,
        Some(lsp::OneOf::Left(true)),
        "即使在批次中较早出现未知 ID,注销最后一个动态注册后也应恢复静态能力",
    );
}

#[gpui::test]
async fn test_multi_registration_duplicate_id_keeps_order(cx: &mut gpui::TestAppContext) {
    init_test(cx);
    let (project, fake_server) =
        setup_dynamic_registration_test(cx, lsp::ServerCapabilities::default()).await;
    let server_id = fake_server.server.server_id();
    let method = "textDocument/inlayHint";

    let options_a = lsp::InlayHintOptions {
        resolve_provider: Some(true),
        ..lsp::InlayHintOptions::default()
    };
    let options_b = lsp::InlayHintOptions {
        resolve_provider: Some(false),
        ..lsp::InlayHintOptions::default()
    };
    let options_a_replacement = lsp::InlayHintOptions::default();

    let (refresh_events, _refresh_events_subscription) = observe_refresh_events(&project, cx);
    register_capability(
        &fake_server,
        method,
        "inlay-hint-a",
        serde_json::to_value(&options_a).ok(),
    )
    .await;
    register_capability(
        &fake_server,
        method,
        "inlay-hint-b",
        serde_json::to_value(&options_b).ok(),
    )
    .await;
    cx.executor().run_until_parked();
    assert_eq!(
        refresh_events.lock().drain(..).collect::<Vec<_>>(),
        vec![
            format!("inlay_hints({server_id})"),
            format!("inlay_hints({server_id})"),
        ],
        "两次注册都应触发刷新",
    );

    register_capability(
        &fake_server,
        method,
        "inlay-hint-a",
        serde_json::to_value(&options_a_replacement).ok(),
    )
    .await;
    cx.executor().run_until_parked();
    assert_eq!(
        server_capabilities(&project, server_id, cx).inlay_hint_provider,
        Some(lsp::OneOf::Right(
            lsp::InlayHintServerCapabilities::Options(options_b)
        )),
        "在重复 ID 替换旧注册后,最新的不同注册应保持活动状态",
    );
    assert_eq!(
        refresh_events.lock().as_slice(),
        &[] as &[String],
        "重复 ID 替换非活动注册后不应触发刷新",
    );

    unregister_capabilities(&fake_server, method, &["inlay-hint-b"]).await;
    cx.executor().run_until_parked();
    assert_eq!(
        server_capabilities(&project, server_id, cx).inlay_hint_provider,
        Some(lsp::OneOf::Right(
            lsp::InlayHintServerCapabilities::Options(options_a_replacement)
        )),
        "expected the replaced registration's options to be restored",
    );

    unregister_capabilities(&fake_server, method, &["inlay-hint-a"]).await;
    cx.executor().run_until_parked();
    assert_eq!(
        server_capabilities(&project, server_id, cx).inlay_hint_provider,
        None,
        "注销最后一个注册后应清除内联提示提供程序",
    );
}

#[gpui::test]
async fn test_registration_with_unchanged_options_does_not_refresh(cx: &mut gpui::TestAppContext) {
    init_test(cx);
    let (project, fake_server) =
        setup_dynamic_registration_test(cx, lsp::ServerCapabilities::default()).await;
    let server_id = fake_server.server.server_id();
    let method = "textDocument/codeLens";

    let options = lsp::CodeLensOptions {
        resolve_provider: Some(true),
    };

    let (refresh_events, _refresh_events_subscription) = observe_refresh_events(&project, cx);
    register_capability(
        &fake_server,
        method,
        "lens-a",
        serde_json::to_value(options).ok(),
    )
    .await;
    cx.executor().run_until_parked();
    assert_eq!(
        refresh_events.lock().drain(..).collect::<Vec<_>>(),
        vec![format!("code_lens({server_id})")],
        "第一次注册应触发刷新",
    );

    register_capability(
        &fake_server,
        method,
        "lens-b",
        serde_json::to_value(options).ok(),
    )
    .await;
    cx.executor().run_until_parked();
    assert_eq!(
        refresh_events.lock().as_slice(),
        &[] as &[String],
        "expected a registration with options identical to the active ones to not refresh",
    );

    unregister_capabilities(&fake_server, method, &["lens-b"]).await;
    cx.executor().run_until_parked();
    assert_eq!(
        refresh_events.lock().as_slice(),
        &[] as &[String],
        "expected an unregistration that restores identical options to not refresh",
    );
    assert_eq!(
        server_capabilities(&project, server_id, cx).code_lens_provider,
        Some(options),
        "剩余注册的选项应保持活动状态",
    );

    unregister_capabilities(&fake_server, method, &["lens-a"]).await;
    cx.executor().run_until_parked();
    assert_eq!(
        refresh_events.lock().drain(..).collect::<Vec<_>>(),
        vec![format!("code_lens({server_id})")],
        "最后一次注销应清除能力并触发刷新",
    );
    assert_eq!(
        server_capabilities(&project, server_id, cx).code_lens_provider,
        None,
        "注销最后一个注册后应清除代码透镜提供程序",
    );
}

#[gpui::test]
async fn test_multi_registration_completion_triggers(cx: &mut gpui::TestAppContext) {
    init_test(cx);
    let (project, fake_server) =
        setup_dynamic_registration_test(cx, lsp::ServerCapabilities::default()).await;
    let method = "textDocument/completion";

    let (buffer, _lsp_handle) = project
        .update(cx, |project, cx| {
            project.open_local_buffer_with_lsp(path!("/the-root/a.rs"), cx)
        })
        .await
        .unwrap();
    let buffer_triggers = |cx: &mut gpui::TestAppContext| {
        buffer.read_with(cx, |buffer, _| buffer.completion_triggers().clone())
    };

    let options_a = lsp::CompletionOptions {
        trigger_characters: Some(vec![".".to_string()]),
        ..lsp::CompletionOptions::default()
    };
    let options_b = lsp::CompletionOptions {
        trigger_characters: Some(vec![":".to_string()]),
        ..lsp::CompletionOptions::default()
    };

    register_capability(
        &fake_server,
        method,
        "completion-a",
        serde_json::to_value(&options_a).ok(),
    )
    .await;
    cx.executor().run_until_parked();
    assert_eq!(
        buffer_triggers(cx),
        BTreeSet::from([".".to_string()]),
        "应应用第一次注册的触发器",
    );

    register_capability(
        &fake_server,
        method,
        "completion-b",
        serde_json::to_value(&options_b).ok(),
    )
    .await;
    cx.executor().run_until_parked();
    assert_eq!(
        buffer_triggers(cx),
        BTreeSet::from([":".to_string()]),
        "第二次注册的触发器应替换第一次的触发器",
    );

    let options_a_replacement = lsp::CompletionOptions {
        trigger_characters: Some(vec!["!".to_string()]),
        ..lsp::CompletionOptions::default()
    };
    register_capability(
        &fake_server,
        method,
        "completion-a",
        serde_json::to_value(&options_a_replacement).ok(),
    )
    .await;
    cx.executor().run_until_parked();
    assert_eq!(
        buffer_triggers(cx),
        BTreeSet::from([":".to_string()]),
        "在重复 ID 替换非活动注册后,活动注册的触发器应保持应用",
    );

    unregister_capabilities(&fake_server, method, &["completion-b"]).await;
    cx.executor().run_until_parked();
    assert_eq!(
        buffer_triggers(cx),
        BTreeSet::from(["!".to_string()]),
        "应恢复被替换注册的触发器",
    );

    unregister_capabilities(&fake_server, method, &["completion-a"]).await;
    cx.executor().run_until_parked();
    assert_eq!(
        buffer_triggers(cx),
        BTreeSet::new(),
        "注销最后一个注册后应清除补全触发器",
    );
}

#[gpui::test]
async fn test_multi_registration_middle_removal(cx: &mut gpui::TestAppContext) {
    init_test(cx);
    let (project, fake_server) =
        setup_dynamic_registration_test(cx, lsp::ServerCapabilities::default()).await;
    let server_id = fake_server.server.server_id();
    let method = "textDocument/inlayHint";

    let options_a = lsp::InlayHintOptions {
        resolve_provider: Some(true),
        ..lsp::InlayHintOptions::default()
    };
    let options_b = lsp::InlayHintOptions {
        resolve_provider: Some(false),
        ..lsp::InlayHintOptions::default()
    };

    register_capability(
        &fake_server,
        method,
        "inlay-hint-a",
        serde_json::to_value(&options_a).ok(),
    )
    .await;
    register_capability(
        &fake_server,
        method,
        "inlay-hint-b",
        serde_json::to_value(&options_b).ok(),
    )
    .await;
    cx.executor().run_until_parked();

    let (refresh_events, _refresh_events_subscription) = observe_refresh_events(&project, cx);
    unregister_capabilities(&fake_server, method, &["inlay-hint-a"]).await;
    cx.executor().run_until_parked();
    assert_eq!(
        server_capabilities(&project, server_id, cx).inlay_hint_provider,
        Some(lsp::OneOf::Right(
            lsp::InlayHintServerCapabilities::Options(options_b)
        )),
        "从中间移除较旧的注册后,最新的注册应保持活动状态",
    );
    assert_eq!(
        refresh_events.lock().as_slice(),
        &[] as &[String],
        "移除非活动注册后不应触发刷新",
    );

    unregister_capabilities(&fake_server, method, &["inlay-hint-b"]).await;
    cx.executor().run_until_parked();
    assert_eq!(
        server_capabilities(&project, server_id, cx).inlay_hint_provider,
        None,
        "expected inlay hint provider to be cleared after unregistering the last registration",
    );
}

#[gpui::test]
async fn test_refresh_during_code_lens_fetch_does_not_resurrect_stale_data(
    cx: &mut gpui::TestAppContext,
) {
    init_test(cx);
    let (project, fake_server) =
        setup_dynamic_registration_test(cx, lsp::ServerCapabilities::default()).await;
    let method = "textDocument/codeLens";

    let (buffer, _lsp_handle) = project
        .update(cx, |project, cx| {
            project.open_local_buffer_with_lsp(path!("/the-root/a.rs"), cx)
        })
        .await
        .unwrap();

    let lens_requests = Arc::new(atomic::AtomicUsize::new(0));
    let (gate_tx, gate_rx) = futures::channel::oneshot::channel::<()>();
    let gate_rx = Arc::new(Mutex::new(Some(gate_rx)));
    fake_server.set_request_handler::<lsp::request::CodeLensRequest, _, _>({
        let lens_requests = lens_requests.clone();
        let gate_rx = gate_rx.clone();
        move |_, _| {
            lens_requests.fetch_add(1, atomic::Ordering::Release);
            let gate = gate_rx.lock().take();
            async move {
                if let Some(gate) = gate {
                    gate.await.ok();
                }
                Ok(Some(vec![lsp::CodeLens {
                    range: lsp::Range::new(lsp::Position::new(0, 0), lsp::Position::new(0, 1)),
                    command: Some(lsp::Command {
                        title: "过期透镜".to_string(),
                        command: "lens_cmd".to_string(),
                        arguments: None,
                    }),
                    data: None,
                }]))
            }
        }
    });

    register_capability(
        &fake_server,
        method,
        "lens",
        serde_json::to_value(lsp::CodeLensOptions {
            resolve_provider: None,
        })
        .ok(),
    )
    .await;
    cx.executor().run_until_parked();

    let lsp_store = project.read_with(cx, |project, _| project.lsp_store());
    let first_fetch =
        lsp_store.update(cx, |lsp_store, cx| lsp_store.code_lens_actions(&buffer, cx));
    cx.executor().advance_clock(Duration::from_millis(50));
    cx.executor().run_until_parked();
    assert_eq!(
        lens_requests.load(atomic::Ordering::Acquire),
        1,
        "注销前代码透镜请求应正在进行中",
    );

    unregister_capabilities(&fake_server, method, &["lens"]).await;
    cx.executor().run_until_parked();

    gate_tx.send(()).unwrap();
    first_fetch.await.unwrap();
    cx.executor().run_until_parked();

    let actions = lsp_store
        .update(cx, |lsp_store, cx| lsp_store.code_lens_actions(&buffer, cx))
        .await
        .unwrap();
    assert_eq!(
        actions.map(|actions| actions.len()),
        Some(0),
        "注销后不应有代码透镜数据,即使过期的获取在其后完成",
    );
    assert_eq!(
        lens_requests.load(atomic::Ordering::Acquire),
        1,
        "不应再次查询已注销的服务器",
    );
}

#[gpui::test]
async fn test_multi_registration_unregister_with_static_only(cx: &mut gpui::TestAppContext) {
    init_test(cx);
    let (project, fake_server) = setup_dynamic_registration_test(
        cx,
        lsp::ServerCapabilities {
            inlay_hint_provider: Some(lsp::OneOf::Left(true)),
            ..lsp::ServerCapabilities::default()
        },
    )
    .await;
    let server_id = fake_server.server.server_id();

    unregister_capabilities(&fake_server, "textDocument/inlayHint", &["unknown-id"]).await;
    cx.executor().run_until_parked();
    assert_eq!(
        server_capabilities(&project, server_id, cx).inlay_hint_provider,
        Some(lsp::OneOf::Left(true)),
        "注销未知 ID 后静态能力应保持不变",
    );
}

#[gpui::test]
async fn test_multi_registration_completion_static_restore(cx: &mut gpui::TestAppContext) {
    init_test(cx);
    let static_options = lsp::CompletionOptions {
        trigger_characters: Some(vec![".".to_string()]),
        ..lsp::CompletionOptions::default()
    };
    let (project, fake_server) = setup_dynamic_registration_test(
        cx,
        lsp::ServerCapabilities {
            completion_provider: Some(static_options.clone()),
            ..lsp::ServerCapabilities::default()
        },
    )
    .await;
    let server_id = fake_server.server.server_id();
    let method = "textDocument/completion";

    let (buffer, _lsp_handle) = project
        .update(cx, |project, cx| {
            project.open_local_buffer_with_lsp(path!("/the-root/a.rs"), cx)
        })
        .await
        .unwrap();
    let buffer_triggers = |cx: &mut gpui::TestAppContext| {
        buffer.read_with(cx, |buffer, _| buffer.completion_triggers().clone())
    };
    assert_eq!(
        buffer_triggers(cx),
        BTreeSet::from([".".to_string()]),
        "在任何动态注册之前应存在静态触发器字符",
    );

    let dynamic_options = lsp::CompletionOptions {
        trigger_characters: Some(vec![":".to_string()]),
        ..lsp::CompletionOptions::default()
    };
    register_capability(
        &fake_server,
        method,
        "completion-dynamic",
        serde_json::to_value(&dynamic_options).ok(),
    )
    .await;
    cx.executor().run_until_parked();
    assert_eq!(
        buffer_triggers(cx),
        BTreeSet::from([":".to_string()]),
        "动态注册的触发器应覆盖静态触发器",
    );

    unregister_capabilities(&fake_server, method, &["completion-dynamic"]).await;
    cx.executor().run_until_parked();
    assert_eq!(
        server_capabilities(&project, server_id, cx).completion_provider,
        Some(static_options),
        "应恢复静态补全提供程序",
    );
    assert_eq!(
        buffer_triggers(cx),
        BTreeSet::from([".".to_string()]),
        "应恢复静态触发器字符",
    );
}

#[gpui::test]
async fn test_multi_registration_same_id_different_methods(cx: &mut gpui::TestAppContext) {
    init_test(cx);
    let (project, fake_server) =
        setup_dynamic_registration_test(cx, lsp::ServerCapabilities::default()).await;
    let server_id = fake_server.server.server_id();

    let code_lens_options = lsp::CodeLensOptions {
        resolve_provider: Some(true),
    };
    register_capability(&fake_server, "textDocument/inlayHint", "shared-id", None).await;
    register_capability(
        &fake_server,
        "textDocument/codeLens",
        "shared-id",
        serde_json::to_value(code_lens_options).ok(),
    )
    .await;
    cx.executor().run_until_parked();

    unregister_capabilities(&fake_server, "textDocument/codeLens", &["shared-id"]).await;
    cx.executor().run_until_parked();
    let capabilities = server_capabilities(&project, server_id, cx);
    assert_eq!(
        capabilities.code_lens_provider, None,
        "应移除代码透镜注册",
    );
    assert_eq!(
        capabilities.inlay_hint_provider,
        Some(lsp::OneOf::Left(true)),
        "具有相同 ID 的内联提示注册应保持完整",
    );
}

#[gpui::test]
async fn test_multi_registration_diagnostics(cx: &mut gpui::TestAppContext) {
    init_test(cx);
    let (project, fake_server) =
        setup_dynamic_registration_test(cx, lsp::ServerCapabilities::default()).await;
    let server_id = fake_server.server.server_id();
    let method = "textDocument/diagnostic";
    fake_server.set_request_handler::<lsp::request::DocumentDiagnosticRequest, _, _>(
        move |_, _| async move {
            Ok(lsp::DocumentDiagnosticReportResult::Report(
                lsp::DocumentDiagnosticReport::Full(
                    lsp::RelatedFullDocumentDiagnosticReport::default(),
                ),
            ))
        },
    );

    let options_a = lsp::DiagnosticServerCapabilities::Options(lsp::DiagnosticOptions {
        identifier: Some("diagnostics-a".to_string()),
        ..lsp::DiagnosticOptions::default()
    });
    let options_b = lsp::DiagnosticServerCapabilities::Options(lsp::DiagnosticOptions {
        identifier: Some("diagnostics-b".to_string()),
        ..lsp::DiagnosticOptions::default()
    });

    register_capability(
        &fake_server,
        method,
        "diag-a",
        serde_json::to_value(&options_a).ok(),
    )
    .await;
    register_capability(
        &fake_server,
        method,
        "diag-b",
        serde_json::to_value(&options_b).ok(),
    )
    .await;
    cx.executor().run_until_parked();
    assert_eq!(
        server_capabilities(&project, server_id, cx).diagnostic_provider,
        Some(options_b),
        "expected the latest diagnostic registration to be active",
    );

    unregister_capabilities(&fake_server, method, &["diag-b"]).await;
    cx.executor().run_until_parked();
    assert_eq!(
        server_capabilities(&project, server_id, cx).diagnostic_provider,
        Some(options_a.clone()),
        "expected the remaining diagnostic registration to be restored",
    );

    unregister_capabilities(&fake_server, method, &["unknown-id"]).await;
    cx.executor().run_until_parked();
    assert_eq!(
        server_capabilities(&project, server_id, cx).diagnostic_provider,
        Some(options_a),
        "未知的注销 ID 应使诊断提供程序保持完整",
    );

    unregister_capabilities(&fake_server, method, &["diag-a"]).await;
    cx.executor().run_until_parked();
    assert_eq!(
        server_capabilities(&project, server_id, cx).diagnostic_provider,
        None,
        "注销最后一个注册后应清除诊断提供程序",
    );
}

/// One large scenario for all per-server-refreshable LSP data kinds (document colors,
/// links, folding ranges, symbols, code lens, semantic tokens, inlay hints), asserting
/// at the data level that per-server refreshes never disturb other servers:
/// neither the second server on the same buffer, nor a server of an unrelated language.
#[gpui::test]
async fn test_per_server_refreshes_keep_other_servers_data(cx: &mut gpui::TestAppContext) {
    init_test(cx);

    let fs = FakeFs::new(cx.executor());
    fs.insert_tree(
        path!("/the-root"),
        json!({ "a.rs": "fn main() {\n}\n", "b.md": "# title\ntext\n" }),
    )
    .await;
    let project = Project::test(fs, [path!("/the-root").as_ref()], cx).await;
    let language_registry = project.read_with(cx, |project, _| project.languages().clone());
    language_registry.add(rust_lang());
    language_registry.add(markdown_lang());

    let mut static_servers = language_registry.register_fake_lsp(
        "Rust",
        FakeLspAdapter {
            name: "static-server",
            capabilities: all_lsp_data_server_capabilities(),
            ..FakeLspAdapter::default()
        },
    );
    let mut dynamic_servers = language_registry.register_fake_lsp(
        "Rust",
        FakeLspAdapter {
            name: "dynamic-server",
            ..FakeLspAdapter::default()
        },
    );
    let mut markdown_servers = language_registry.register_fake_lsp(
        "Markdown",
        FakeLspAdapter {
            name: "markdown-server",
            capabilities: all_lsp_data_server_capabilities(),
            ..FakeLspAdapter::default()
        },
    );
    cx.executor().run_until_parked();

    let (rust_buffer, _rust_lsp_handle) = project
        .update(cx, |project, cx| {
            project.open_local_buffer_with_lsp(path!("/the-root/a.rs"), cx)
        })
        .await
        .unwrap();
    let (markdown_buffer, _markdown_lsp_handle) = project
        .update(cx, |project, cx| {
            project.open_local_buffer_with_lsp(path!("/the-root/b.md"), cx)
        })
        .await
        .unwrap();
    let static_server = static_servers.next().await.unwrap();
    let dynamic_server = dynamic_servers.next().await.unwrap();
    let markdown_server = markdown_servers.next().await.unwrap();
    let static_server_id = static_server.server.server_id();
    let dynamic_server_id = dynamic_server.server.server_id();
    let markdown_server_id = markdown_server.server.server_id();
    let static_counts = serve_lsp_data(&static_server, 1);
    let dynamic_counts = serve_lsp_data(&dynamic_server, 2);
    let markdown_counts = serve_lsp_data(&markdown_server, 3);
    let (refresh_events, _refresh_events_subscription) = observe_refresh_events(&project, cx);
    cx.executor().run_until_parked();

    let only_static = lsp_data_labels(&[(static_server_id, 1)]);
    let both_rust_servers = lsp_data_labels(&[(static_server_id, 1), (dynamic_server_id, 2)]);
    let only_markdown = lsp_data_labels(&[(markdown_server_id, 3)]);
    let dynamic_refresh = InvalidationStrategy::RefreshRequested {
        server_id: dynamic_server_id,
    };
    let all_dynamic_refresh_events = vec![
        format!("code_lens({dynamic_server_id})"),
        format!("document_colors({dynamic_server_id})"),
        format!("document_links({dynamic_server_id})"),
        format!("document_symbols({dynamic_server_id})"),
        format!("folding_ranges({dynamic_server_id})"),
        format!("inlay_hints({dynamic_server_id})"),
        format!("semantic_tokens({dynamic_server_id})"),
    ];

    assert_eq!(
        fetch_all_lsp_data(&project, &rust_buffer, InvalidationStrategy::None, cx).await,
        only_static,
        "初始获取后应仅包含具有静态能力的服务器的数据",
    );
    assert_eq!(
        fetch_all_lsp_data(&project, &markdown_buffer, InvalidationStrategy::None, cx).await,
        only_markdown,
        "初始获取后应包含 Markdown 服务器的数据",
    );
    assert_eq!(refresh_events.lock().as_slice(), &[] as &[String]);

    register_all_lsp_data_capabilities(&dynamic_server).await;
    cx.executor().run_until_parked();
    assert_eq!(
        sorted(refresh_events.lock().drain(..)),
        all_dynamic_refresh_events,
        "每个动态注册都应触发相应的按服务器刷新",
    );
    assert_eq!(
        fetch_all_lsp_data(&project, &rust_buffer, dynamic_refresh, cx).await,
        both_rust_servers,
        "动态注册后应包含两个服务器的数据",
    );
    assert_eq!(
        fetch_all_lsp_data(&project, &markdown_buffer, InvalidationStrategy::None, cx).await,
        only_markdown,
        "其他语言服务器的注册不应干扰 Markdown 数据",
    );

    dynamic_server
        .request::<lsp::request::CodeLensRefresh>((), DEFAULT_LSP_REQUEST_TIMEOUT)
        .await
        .into_response()
        .unwrap();
    dynamic_server
        .request::<lsp::request::SemanticTokensRefresh>((), DEFAULT_LSP_REQUEST_TIMEOUT)
        .await
        .into_response()
        .unwrap();
    dynamic_server
        .request::<lsp::request::InlayHintRefreshRequest>((), DEFAULT_LSP_REQUEST_TIMEOUT)
        .await
        .into_response()
        .unwrap();
    cx.executor().run_until_parked();
    assert_eq!(
        sorted(refresh_events.lock().drain(..)),
        vec![
            format!("code_lens({dynamic_server_id})"),
            format!("inlay_hints({dynamic_server_id})"),
            format!("semantic_tokens({dynamic_server_id})"),
        ],
        "服务器发送的刷新请求应触发按服务器刷新",
    );
    assert_eq!(
        fetch_all_lsp_data(&project, &rust_buffer, dynamic_refresh, cx).await,
        both_rust_servers,
        "按服务器刷新请求应保留其他服务器的数据",
    );
    assert_eq!(
        fetch_all_lsp_data(&project, &markdown_buffer, InvalidationStrategy::None, cx).await,
        only_markdown,
        "其他语言服务器的刷新请求不应干扰 Markdown 数据",
    );

    unregister_all_lsp_data_capabilities(&dynamic_server).await;
    cx.executor().run_until_parked();
    assert_eq!(
        sorted(refresh_events.lock().drain(..)),
        all_dynamic_refresh_events,
        "每次注销都应触发相应的按服务器刷新",
    );
    assert_eq!(
        fetch_all_lsp_data(&project, &rust_buffer, dynamic_refresh, cx).await,
        only_static,
        "注销应仅丢弃动态服务器的数据",
    );
    assert_eq!(
        fetch_all_lsp_data(&project, &markdown_buffer, InvalidationStrategy::None, cx).await,
        only_markdown,
        "其他语言服务器的注销不应干扰 Markdown 数据",
    );

    assert_eq!(
        *static_counts.lock(),
        BTreeMap::from_iter([
            ("code_lens", 1),
            ("colors", 1),
            ("document_symbols", 1),
            ("folding_ranges", 1),
            ("inlay_hints", 1),
            ("links", 1),
            ("semantic_tokens", 1),
        ]),
        "静态服务器不应因其他服务器的更改而被重新查询",
    );
    assert_eq!(
        *dynamic_counts.lock(),
        BTreeMap::from_iter([
            ("code_lens", 2),
            ("colors", 1),
            ("document_symbols", 1),
            ("folding_ranges", 1),
            ("inlay_hints", 2),
            ("links", 1),
            ("semantic_tokens", 2),
        ]),
        "动态服务器应仅在注册后以及为其请求刷新的类型被重新查询",
    );
    assert_eq!(
        *markdown_counts.lock(),
        BTreeMap::from_iter([
            ("code_lens", 1),
            ("colors", 1),
            ("document_symbols", 1),
            ("folding_ranges", 1),
            ("inlay_hints", 1),
            ("links", 1),
            ("semantic_tokens", 1),
        ]),
        "Markdown 服务器不应因 Rust 服务器的更改而被重新查询",
    );
}

/// Unlike [`test_per_server_refreshes_keep_other_servers_data`], focuses on the
/// dynamic capability changes as the refresh trigger: how they combine data from
/// static and dynamic capabilities (e.g. completion trigger characters), and which
/// servers get [re-]queried after the capability changes.
#[gpui::test]
async fn test_dynamic_registration_refreshes_lsp_data(cx: &mut gpui::TestAppContext) {
    init_test(cx);

    let fs = FakeFs::new(cx.executor());
    fs.insert_tree(path!("/the-root"), json!({ "a.rs": "fn main() {\n}\n" }))
        .await;
    let project = Project::test(fs, [path!("/the-root").as_ref()], cx).await;
    let language_registry = project.read_with(cx, |project, _| project.languages().clone());
    language_registry.add(rust_lang());

    let mut static_servers = language_registry.register_fake_lsp(
        "Rust",
        FakeLspAdapter {
            name: "static-server",
            capabilities: lsp::ServerCapabilities {
                completion_provider: Some(lsp::CompletionOptions {
                    trigger_characters: Some(vec![".".to_string()]),
                    ..lsp::CompletionOptions::default()
                }),
                ..all_lsp_data_server_capabilities()
            },
            ..FakeLspAdapter::default()
        },
    );
    let mut dynamic_servers = language_registry.register_fake_lsp(
        "Rust",
        FakeLspAdapter {
            name: "dynamic-server",
            ..FakeLspAdapter::default()
        },
    );
    cx.executor().run_until_parked();

    let (buffer, _lsp_handle) = project
        .update(cx, |project, cx| {
            project.open_local_buffer_with_lsp(path!("/the-root/a.rs"), cx)
        })
        .await
        .unwrap();
    let static_server = static_servers.next().await.unwrap();
    let dynamic_server = dynamic_servers.next().await.unwrap();
    let dynamic_server_id = dynamic_server.server.server_id();
    let static_counts = serve_lsp_data(&static_server, 1);
    let dynamic_counts = serve_lsp_data(&dynamic_server, 2);
    let (refresh_events, _refresh_events_subscription) = observe_refresh_events(&project, cx);
    let buffer_triggers = |cx: &mut gpui::TestAppContext| {
        buffer.read_with(cx, |buffer, _| buffer.completion_triggers().clone())
    };
    cx.executor().run_until_parked();

    let all_kinds_queried_once = BTreeMap::from_iter([
        ("code_lens", 1),
        ("colors", 1),
        ("document_symbols", 1),
        ("folding_ranges", 1),
        ("inlay_hints", 1),
        ("links", 1),
        ("semantic_tokens", 1),
    ]);
    let all_dynamic_refresh_events = vec![
        format!("code_lens({dynamic_server_id})"),
        format!("document_colors({dynamic_server_id})"),
        format!("document_links({dynamic_server_id})"),
        format!("document_symbols({dynamic_server_id})"),
        format!("folding_ranges({dynamic_server_id})"),
        format!("inlay_hints({dynamic_server_id})"),
        format!("semantic_tokens({dynamic_server_id})"),
    ];

    fetch_all_lsp_data(&project, &buffer, InvalidationStrategy::None, cx).await;
    assert_eq!(
        *static_counts.lock(),
        all_kinds_queried_once,
        "初始获取应查询具有静态能力的服务器",
    );
    assert_eq!(
        *dynamic_counts.lock(),
        BTreeMap::new(),
        "初始获取应跳过没有能力的服务器",
    );
    assert_eq!(buffer_triggers(cx), BTreeSet::from([".".to_string()]));
    assert_eq!(refresh_events.lock().as_slice(), &[] as &[String]);

    register_all_lsp_data_capabilities(&dynamic_server).await;
    register_capability(
        &dynamic_server,
        "textDocument/completion",
        "completions",
        serde_json::to_value(lsp::CompletionOptions {
            trigger_characters: Some(vec![":".to_string()]),
            ..lsp::CompletionOptions::default()
        })
        .ok(),
    )
    .await;
    cx.executor().run_until_parked();

    assert_eq!(
        sorted(refresh_events.lock().drain(..)),
        all_dynamic_refresh_events,
        "每个动态注册都应触发相应的按服务器刷新",
    );
    assert_eq!(
        buffer_triggers(cx),
        BTreeSet::from([".".to_string(), ":".to_string()]),
        "两个服务器的触发器字符应合并",
    );

    fetch_all_lsp_data(
        &project,
        &buffer,
        InvalidationStrategy::RefreshRequested {
            server_id: dynamic_server_id,
        },
        cx,
    )
    .await;
    assert_eq!(
        *dynamic_counts.lock(),
        all_kinds_queried_once,
        "刷新后应查询新注册的服务器",
    );
    assert_eq!(
        *static_counts.lock(),
        all_kinds_queried_once,
        "按服务器刷新应保持静态服务器的缓存数据不变",
    );

    unregister_all_lsp_data_capabilities(&dynamic_server).await;
    unregister_capabilities(&dynamic_server, "textDocument/completion", &["completions"]).await;
    cx.executor().run_until_parked();

    assert_eq!(
        sorted(refresh_events.lock().drain(..)),
        all_dynamic_refresh_events,
        "每次注销都应触发相应的按服务器刷新",
    );
    assert_eq!(
        buffer_triggers(cx),
        BTreeSet::from([".".to_string()]),
        "应仅保留静态服务器的触发器字符",
    );

    fetch_all_lsp_data(
        &project,
        &buffer,
        InvalidationStrategy::RefreshRequested {
            server_id: dynamic_server_id,
        },
        cx,
    )
    .await;
    assert_eq!(
        *dynamic_counts.lock(),
        all_kinds_queried_once,
        "不应再查询已注销的服务器",
    );
    assert_eq!(
        *static_counts.lock(),
        all_kinds_queried_once,
        "静态服务器不应因其他服务器的能力更改而被重新查询",
    );
}

#[gpui::test]
async fn test_semantic_tokens_refresh_invalidates_only_the_refreshed_server(
    cx: &mut gpui::TestAppContext,
) {
    init_test(cx);

    let fs = FakeFs::new(cx.executor());
    fs.insert_tree(path!("/the-root"), json!({ "a.rs": "fn main() {}" }))
        .await;
    let project = Project::test(fs, [path!("/the-root").as_ref()], cx).await;
    let language_registry = project.read_with(cx, |project, _| project.languages().clone());
    language_registry.add(rust_lang());

    let semantic_tokens_capabilities = lsp::ServerCapabilities {
        semantic_tokens_provider: Some(
            lsp::SemanticTokensServerCapabilities::SemanticTokensOptions(
                lsp::SemanticTokensOptions {
                    full: Some(lsp::SemanticTokensFullOptions::Bool(true)),
                    ..lsp::SemanticTokensOptions::default()
                },
            ),
        ),
        ..lsp::ServerCapabilities::default()
    };
    let mut servers_a = language_registry.register_fake_lsp(
        "Rust",
        FakeLspAdapter {
            name: "server-a",
            capabilities: semantic_tokens_capabilities.clone(),
            ..FakeLspAdapter::default()
        },
    );
    let mut servers_b = language_registry.register_fake_lsp(
        "Rust",
        FakeLspAdapter {
            name: "server-b",
            capabilities: semantic_tokens_capabilities,
            ..FakeLspAdapter::default()
        },
    );
    cx.executor().run_until_parked();

    let (buffer, _lsp_handle) = project
        .update(cx, |project, cx| {
            project.open_local_buffer_with_lsp(path!("/the-root/a.rs"), cx)
        })
        .await
        .unwrap();
    let buffer_id = buffer.read_with(cx, |buffer, _| buffer.remote_id());
    let server_a = servers_a.next().await.unwrap();
    let server_b = servers_b.next().await.unwrap();
    for server in [&server_a, &server_b] {
        server.set_request_handler::<lsp::request::SemanticTokensFullRequest, _, _>(
            move |_, _| async move {
                Ok(Some(lsp::SemanticTokensResult::Tokens(
                    lsp::SemanticTokens::default(),
                )))
            },
        );
    }
    let server_a_id = server_a.server.server_id();
    let server_b_id = server_b.server.server_id();
    cx.executor().run_until_parked();

    let lsp_store = project.read_with(cx, |project, _| project.lsp_store());
    let cached_token_servers = |cx: &mut gpui::TestAppContext| {
        lsp_store.read_with(cx, |lsp_store, _| {
            lsp_store.semantic_token_servers(buffer_id)
        })
    };

    lsp_store
        .update(cx, |lsp_store, cx| {
            lsp_store.semantic_tokens(buffer.clone(), cx)
        })
        .await
        .unwrap();
    assert_eq!(
        cached_token_servers(cx),
        vec![server_a_id, server_b_id],
        "初始获取后应包含两个服务器的令牌",
    );

    for repetition in 0..2 {
        server_a
            .request::<lsp::request::SemanticTokensRefresh>((), DEFAULT_LSP_REQUEST_TIMEOUT)
            .await
            .into_response()
            .unwrap();
        let refresh_task = lsp_store.update(cx, |lsp_store, cx| {
            lsp_store.semantic_tokens(buffer.clone(), cx)
        });
        assert_eq!(
            cached_token_servers(cx),
            vec![server_b_id],
            "刷新 {repetition} 应仅使已刷新服务器的令牌失效",
        );
        let concurrent_task = lsp_store.update(cx, |lsp_store, cx| {
            lsp_store.semantic_tokens(buffer.clone(), cx)
        });
        assert_eq!(
            cached_token_servers(cx),
            vec![server_b_id],
            "并发查询 {repetition} 不应再次使数据失效",
        );
        refresh_task.await.unwrap();
        concurrent_task.await.unwrap();
        assert_eq!(
            cached_token_servers(cx),
            vec![server_a_id, server_b_id],
            "刷新 {repetition} 后应重新获取已刷新服务器的令牌",
        );
    }
}

#[gpui::test]
async fn test_semantic_tokens_refresh_during_fetch_does_not_resurrect_stale_data(
    cx: &mut gpui::TestAppContext,
) {
    init_test(cx);
    let (project, fake_server) = setup_dynamic_registration_test(
        cx,
        lsp::ServerCapabilities {
            semantic_tokens_provider: Some(
                lsp::SemanticTokensServerCapabilities::SemanticTokensOptions(
                    lsp::SemanticTokensOptions {
                        full: Some(lsp::SemanticTokensFullOptions::Bool(true)),
                        ..lsp::SemanticTokensOptions::default()
                    },
                ),
            ),
            ..lsp::ServerCapabilities::default()
        },
    )
    .await;
    let server_id = fake_server.server.server_id();

    let (buffer, _lsp_handle) = project
        .update(cx, |project, cx| {
            project.open_local_buffer_with_lsp(path!("/the-root/a.rs"), cx)
        })
        .await
        .unwrap();
    let buffer_id = buffer.read_with(cx, |buffer, _| buffer.remote_id());
    cx.executor().run_until_parked();

    let stale_data = vec![0, 0, 2, 0, 0];
    let fresh_data = vec![0, 0, 2, 0, 0, 0, 3, 4, 0, 0];
    let token_requests = Arc::new(atomic::AtomicUsize::new(0));
    let (gate_tx, gate_rx) = futures::channel::oneshot::channel::<()>();
    let gate_rx = Arc::new(Mutex::new(Some(gate_rx)));
    fake_server.set_request_handler::<lsp::request::SemanticTokensFullRequest, _, _>({
        let token_requests = token_requests.clone();
        let gate_rx = gate_rx.clone();
        let stale_data = stale_data.clone();
        let fresh_data = fresh_data.clone();
        move |_, _| {
            let request = token_requests.fetch_add(1, atomic::Ordering::Release);
            let gate = gate_rx.lock().take();
            let data = if request == 0 {
                stale_data.clone()
            } else {
                fresh_data.clone()
            };
            async move {
                if let Some(gate) = gate {
                    gate.await.ok();
                }
                Ok(Some(lsp::SemanticTokensResult::Tokens(
                    lsp::SemanticTokens {
                        result_id: None,
                        data,
                    },
                )))
            }
        }
    });

    let lsp_store = project.read_with(cx, |project, _| project.lsp_store());
    let stale_fetch = lsp_store.update(cx, |lsp_store, cx| {
        lsp_store.semantic_tokens(buffer.clone(), cx)
    });
    cx.executor().run_until_parked();
    assert_eq!(
        token_requests.load(atomic::Ordering::Acquire),
        1,
        "刷新前第一次获取应正在进行中",
    );

    fake_server
        .request::<lsp::request::SemanticTokensRefresh>((), DEFAULT_LSP_REQUEST_TIMEOUT)
        .await
        .into_response()
        .unwrap();
    cx.executor().run_until_parked();

    let fresh_fetch = lsp_store.update(cx, |lsp_store, cx| {
        lsp_store.semantic_tokens(buffer.clone(), cx)
    });
    fresh_fetch.await.unwrap();

    gate_tx.send(()).unwrap();
    stale_fetch.await.unwrap();
    cx.executor().run_until_parked();

    assert_eq!(
        lsp_store.read_with(cx, |lsp_store, _| lsp_store.semantic_token_data(buffer_id)),
        vec![(server_id, fresh_data)],
        "在刷新后完成的过期获取不应覆盖已刷新的令牌",
    );
    assert_eq!(
        token_requests.load(atomic::Ordering::Acquire),
        2,
        "应恰好有过期和新的获取查询过服务器",
    );
}

#[gpui::test]
async fn test_code_lens_concurrent_fetches_are_deduplicated(cx: &mut gpui::TestAppContext) {
    init_test(cx);
    let (project, fake_server) = setup_dynamic_registration_test(
        cx,
        lsp::ServerCapabilities {
            code_lens_provider: Some(lsp::CodeLensOptions {
                resolve_provider: None,
            }),
            ..lsp::ServerCapabilities::default()
        },
    )
    .await;

    let (buffer, _lsp_handle) = project
        .update(cx, |project, cx| {
            project.open_local_buffer_with_lsp(path!("/the-root/a.rs"), cx)
        })
        .await
        .unwrap();
    cx.executor().run_until_parked();

    let lens_requests = Arc::new(atomic::AtomicUsize::new(0));
    fake_server.set_request_handler::<lsp::request::CodeLensRequest, _, _>({
        let lens_requests = lens_requests.clone();
        move |_, _| {
            lens_requests.fetch_add(1, atomic::Ordering::Release);
            async move { Ok(Some(Vec::new())) }
        }
    });

    let lsp_store = project.read_with(cx, |project, _| project.lsp_store());
    let first_fetch =
        lsp_store.update(cx, |lsp_store, cx| lsp_store.code_lens_actions(&buffer, cx));
    let second_fetch =
        lsp_store.update(cx, |lsp_store, cx| lsp_store.code_lens_actions(&buffer, cx));
    cx.executor().advance_clock(Duration::from_millis(50));
    first_fetch.await.unwrap();
    second_fetch.await.unwrap();
    assert_eq!(
        lens_requests.load(atomic::Ordering::Acquire),
        1,
        "并发的代码透镜获取应共享一个 LSP 请求",
    );

    lsp_store
        .update(cx, |lsp_store, cx| lsp_store.code_lens_actions(&buffer, cx))
        .await
        .unwrap();
    assert_eq!(
        lens_requests.load(atomic::Ordering::Acquire),
        1,
        "对未更改缓冲区的重复获取应从缓存中提供",
    );
}

#[gpui::test]
async fn test_multiple_did_change_watched_files_registrations(cx: &mut gpui::TestAppContext) {
    init_test(cx);

    let fs = FakeFs::new(cx.executor());
    fs.insert_tree(
        path!("/root"),
        json!({
            "src": {
                "a.rs": "",
                "b.rs": "",
            },
            "docs": {
                "readme.md": "",
            },
        }),
    )
    .await;

    let project = Project::test(fs.clone(), [path!("/root").as_ref()], cx).await;
    let language_registry = project.read_with(cx, |project, _| project.languages().clone());
    language_registry.add(rust_lang());
    let mut fake_servers = language_registry.register_fake_lsp(
        "Rust",
        FakeLspAdapter {
            name: "the-language-server",
            ..FakeLspAdapter::default()
        },
    );

    cx.executor().run_until_parked();

    project
        .update(cx, |project, cx| {
            project.open_local_buffer_with_lsp(path!("/root/src/a.rs"), cx)
        })
        .await
        .unwrap();

    let fake_server = fake_servers.next().await.unwrap();
    cx.executor().run_until_parked();

    let file_changes = Arc::new(Mutex::new(Vec::new()));

    // Register two separate watched file registrations.
    register_capability(
        &fake_server,
        "workspace/didChangeWatchedFiles",
        "reg-1",
        serde_json::to_value(lsp::DidChangeWatchedFilesRegistrationOptions {
            watchers: vec![lsp::FileSystemWatcher {
                glob_pattern: lsp::GlobPattern::String(path!("/root/src/*.rs").to_string()),
                kind: None,
            }],
        })
        .ok(),
    )
    .await;

    register_capability(
        &fake_server,
        "workspace/didChangeWatchedFiles",
        "reg-2",
        serde_json::to_value(lsp::DidChangeWatchedFilesRegistrationOptions {
            watchers: vec![lsp::FileSystemWatcher {
                glob_pattern: lsp::GlobPattern::String(path!("/root/docs/*.md").to_string()),
                kind: None,
            }],
        })
        .ok(),
    )
    .await;

    fake_server.handle_notification::<lsp::notification::DidChangeWatchedFiles, _>({
        let file_changes = file_changes.clone();
        move |params, _| {
            let mut file_changes = file_changes.lock();
            file_changes.extend(params.changes);
            file_changes.sort_by(|a, b| a.uri.cmp(&b.uri));
        }
    });

    cx.executor().run_until_parked();

    // Both registrations should match their respective patterns.
    fs.create_file(
        path!("/root/src/c.rs").as_ref(),
        fs::CreateOptions::default(),
    )
    .await
    .unwrap();
    fs.create_file(
        path!("/root/docs/guide.md").as_ref(),
        fs::CreateOptions::default(),
    )
    .await
    .unwrap();
    cx.executor().run_until_parked();

    assert_eq!(
        &*file_changes.lock(),
        &[
            lsp::FileEvent {
                uri: lsp::Uri::from_file_path(path!("/root/docs/guide.md")).unwrap(),
                typ: lsp::FileChangeType::CREATED,
            },
            lsp::FileEvent {
                uri: lsp::Uri::from_file_path(path!("/root/src/c.rs")).unwrap(),
                typ: lsp::FileChangeType::CREATED,
            },
        ]
    );
    file_changes.lock().clear();

    // Unregister the first registration.
    unregister_capabilities(&fake_server, "workspace/didChangeWatchedFiles", &["reg-1"]).await;
    cx.executor().run_until_parked();

    // Only the second registration should still match.
    fs.create_file(
        path!("/root/src/d.rs").as_ref(),
        fs::CreateOptions::default(),
    )
    .await
    .unwrap();
    fs.create_file(
        path!("/root/docs/notes.md").as_ref(),
        fs::CreateOptions::default(),
    )
    .await
    .unwrap();
    cx.executor().run_until_parked();

    assert_eq!(
        &*file_changes.lock(),
        &[lsp::FileEvent {
            uri: lsp::Uri::from_file_path(path!("/root/docs/notes.md")).unwrap(),
            typ: lsp::FileChangeType::CREATED,
        }]
    );
}

async fn setup_dynamic_registration_test(
    cx: &mut gpui::TestAppContext,
    capabilities: lsp::ServerCapabilities,
) -> (Entity<Project>, lsp::FakeLanguageServer) {
    let fs = FakeFs::new(cx.executor());
    fs.insert_tree(path!("/the-root"), json!({ "a.rs": "" }))
        .await;

    let project = Project::test(fs, [path!("/the-root").as_ref()], cx).await;
    let language_registry = project.read_with(cx, |project, _| project.languages().clone());
    language_registry.add(rust_lang());
    let mut fake_servers = language_registry.register_fake_lsp(
        "Rust",
        FakeLspAdapter {
            name: "the-language-server",
            capabilities,
            ..FakeLspAdapter::default()
        },
    );

    cx.executor().run_until_parked();

    project
        .update(cx, |project, cx| {
            project.open_local_buffer_with_lsp(path!("/the-root/a.rs"), cx)
        })
        .await
        .unwrap();

    let fake_server = fake_servers.next().await.unwrap();
    cx.executor().run_until_parked();
    (project, fake_server)
}

async fn register_capability(
    fake_server: &lsp::FakeLanguageServer,
    method: &str,
    id: &str,
    register_options: Option<serde_json::Value>,
) {
    fake_server
        .request::<lsp::request::RegisterCapability>(
            lsp::RegistrationParams {
                registrations: vec![lsp::Registration {
                    id: id.to_string(),
                    method: method.to_string(),
                    register_options,
                }],
            },
            DEFAULT_LSP_REQUEST_TIMEOUT,
        )
        .await
        .into_response()
        .unwrap();
}

async fn unregister_capabilities(
    fake_server: &lsp::FakeLanguageServer,
    method: &str,
    ids: &[&str],
) {
    fake_server
        .request::<lsp::request::UnregisterCapability>(
            lsp::UnregistrationParams {
                unregisterations: ids
                    .iter()
                    .map(|id| lsp::Unregistration {
                        id: id.to_string(),
                        method: method.to_string(),
                    })
                    .collect(),
            },
            DEFAULT_LSP_REQUEST_TIMEOUT,
        )
        .await
        .into_response()
        .unwrap();
}

fn server_capabilities(
    project: &Entity<Project>,
    server_id: LanguageServerId,
    cx: &mut gpui::TestAppContext,
) -> lsp::ServerCapabilities {
    project.read_with(cx, |project, cx| {
        project
            .lsp_store()
            .read(cx)
            .language_server_for_id(server_id)
            .unwrap()
            .capabilities()
    })
}

fn observe_refresh_events(
    project: &Entity<Project>,
    cx: &mut gpui::TestAppContext,
) -> (Arc<Mutex<Vec<String>>>, gpui::Subscription) {
    let events = Arc::new(Mutex::new(Vec::new()));
    let subscription = cx.update({
        let events = events.clone();
        let project = project.clone();
        move |cx| {
            cx.subscribe(&project, move |_, event, _| {
                let label = |name: &str, server_id: &Option<LanguageServerId>| {
                    let server_id = server_id
                        .map_or_else(|| "all".to_string(), |server_id| server_id.to_string());
                    format!("{name}({server_id})")
                };
                let event = match event {
                    Event::RefreshInlayHints { server_id, .. } => {
                        format!("inlay_hints({server_id})")
                    }
                    Event::RefreshSemanticTokens { server_id, .. } => {
                        format!("semantic_tokens({server_id})")
                    }
                    Event::RefreshCodeLens { server_id } => label("code_lens", server_id),
                    Event::RefreshDocumentColors { server_id } => {
                        label("document_colors", server_id)
                    }
                    Event::RefreshDocumentLinks { server_id } => label("document_links", server_id),
                    Event::RefreshFoldingRanges { server_id } => label("folding_ranges", server_id),
                    Event::RefreshDocumentSymbols { server_id } => {
                        label("document_symbols", server_id)
                    }
                    _ => return,
                };
                events.lock().push(event);
            })
        }
    });
    (events, subscription)
}

fn sorted(events: impl IntoIterator<Item = String>) -> Vec<String> {
    let mut events = events.into_iter().collect::<Vec<_>>();
    events.sort();
    events
}

fn all_lsp_data_server_capabilities() -> lsp::ServerCapabilities {
    lsp::ServerCapabilities {
        color_provider: Some(lsp::ColorProviderCapability::Simple(true)),
        document_link_provider: Some(lsp::DocumentLinkOptions {
            resolve_provider: None,
            work_done_progress_options: lsp::WorkDoneProgressOptions::default(),
        }),
        folding_range_provider: Some(lsp::FoldingRangeProviderCapability::Simple(true)),
        document_symbol_provider: Some(lsp::OneOf::Left(true)),
        code_lens_provider: Some(lsp::CodeLensOptions {
            resolve_provider: None,
        }),
        semantic_tokens_provider: Some(
            lsp::SemanticTokensServerCapabilities::SemanticTokensOptions(
                lsp::SemanticTokensOptions {
                    legend: lsp::SemanticTokensLegend {
                        token_types: vec!["keyword".into()],
                        token_modifiers: Vec::new(),
                    },
                    full: Some(lsp::SemanticTokensFullOptions::Bool(true)),
                    ..lsp::SemanticTokensOptions::default()
                },
            ),
        ),
        inlay_hint_provider: Some(lsp::OneOf::Left(true)),
        ..lsp::ServerCapabilities::default()
    }
}

async fn register_all_lsp_data_capabilities(fake_server: &lsp::FakeLanguageServer) {
    register_capability(fake_server, "textDocument/documentColor", "colors", None).await;
    register_capability(
        fake_server,
        "textDocument/documentLink",
        "links",
        serde_json::to_value(lsp::DocumentLinkOptions {
            resolve_provider: None,
            work_done_progress_options: lsp::WorkDoneProgressOptions::default(),
        })
        .ok(),
    )
    .await;
    register_capability(fake_server, "textDocument/foldingRange", "folding", None).await;
    register_capability(fake_server, "textDocument/documentSymbol", "symbols", None).await;
    register_capability(
        fake_server,
        "textDocument/codeLens",
        "code-lens",
        serde_json::to_value(lsp::CodeLensOptions {
            resolve_provider: None,
        })
        .ok(),
    )
    .await;
    register_capability(
        fake_server,
        "textDocument/semanticTokens",
        "tokens",
        serde_json::to_value(lsp::SemanticTokensRegistrationOptions {
            text_document_registration_options: lsp::TextDocumentRegistrationOptions {
                document_selector: None,
            },
            semantic_tokens_options: lsp::SemanticTokensOptions {
                legend: lsp::SemanticTokensLegend {
                    token_types: vec!["keyword".into()],
                    token_modifiers: Vec::new(),
                },
                full: Some(lsp::SemanticTokensFullOptions::Bool(true)),
                ..lsp::SemanticTokensOptions::default()
            },
            static_registration_options: lsp::StaticRegistrationOptions::default(),
        })
        .ok(),
    )
    .await;
    register_capability(fake_server, "textDocument/inlayHint", "hints", None).await;
}

async fn unregister_all_lsp_data_capabilities(fake_server: &lsp::FakeLanguageServer) {
    unregister_capabilities(fake_server, "textDocument/documentColor", &["colors"]).await;
    unregister_capabilities(fake_server, "textDocument/documentLink", &["links"]).await;
    unregister_capabilities(fake_server, "textDocument/foldingRange", &["folding"]).await;
    unregister_capabilities(fake_server, "textDocument/documentSymbol", &["symbols"]).await;
    unregister_capabilities(fake_server, "textDocument/codeLens", &["code-lens"]).await;
    unregister_capabilities(fake_server, "textDocument/semanticTokens", &["tokens"]).await;
    unregister_capabilities(fake_server, "textDocument/inlayHint", &["hints"]).await;
}

type LspRequestCounts = Arc<Mutex<BTreeMap<&'static str, usize>>>;

/// Serves distinct data, derived from `index`, for every LSP data kind, and counts the
/// requests served per kind. [`lsp_data_labels`] builds the expected fetch results for
/// the same `index`.
fn serve_lsp_data(fake_server: &lsp::FakeLanguageServer, index: u8) -> LspRequestCounts {
    let counts = LspRequestCounts::default();
    fake_server.set_request_handler::<lsp::request::DocumentColor, _, _>({
        let counts = counts.clone();
        move |_, _| {
            *counts.lock().entry("colors").or_default() += 1;
            async move {
                Ok(vec![lsp::ColorInformation {
                    range: lsp::Range::new(lsp::Position::new(0, 0), lsp::Position::new(0, 1)),
                    color: lsp::Color {
                        red: 0.0,
                        green: 0.0,
                        blue: 0.0,
                        alpha: f32::from(index),
                    },
                }])
            }
        }
    });
    fake_server.set_request_handler::<lsp::request::DocumentLinkRequest, _, _>({
        let counts = counts.clone();
        move |_, _| {
            *counts.lock().entry("links").or_default() += 1;
            async move {
                Ok(Some(vec![lsp::DocumentLink {
                    range: lsp::Range::new(lsp::Position::new(0, 0), lsp::Position::new(0, 1)),
                    target: Some(format!("https://server-{index}.example/").parse().unwrap()),
                    tooltip: None,
                    data: None,
                }]))
            }
        }
    });
    fake_server.set_request_handler::<lsp::request::FoldingRangeRequest, _, _>({
        let counts = counts.clone();
        move |_, _| {
            *counts.lock().entry("folding_ranges").or_default() += 1;
            async move {
                Ok(Some(vec![lsp::FoldingRange {
                    start_line: 0,
                    start_character: None,
                    end_line: 1,
                    end_character: None,
                    kind: None,
                    collapsed_text: Some(format!("fold-{index}")),
                }]))
            }
        }
    });
    fake_server.set_request_handler::<lsp::request::DocumentSymbolRequest, _, _>({
        let counts = counts.clone();
        move |_, _| {
            *counts.lock().entry("document_symbols").or_default() += 1;
            async move {
                #[allow(deprecated)]
                Ok(Some(lsp::DocumentSymbolResponse::Nested(vec![
                    lsp::DocumentSymbol {
                        name: format!("symbol-{index}"),
                        detail: None,
                        kind: lsp::SymbolKind::FUNCTION,
                        tags: None,
                        deprecated: None,
                        range: lsp::Range::default(),
                        selection_range: lsp::Range::default(),
                        children: None,
                    },
                ])))
            }
        }
    });
    fake_server.set_request_handler::<lsp::request::CodeLensRequest, _, _>({
        let counts = counts.clone();
        move |_, _| {
            *counts.lock().entry("code_lens").or_default() += 1;
            async move {
                Ok(Some(vec![lsp::CodeLens {
                    range: lsp::Range::new(lsp::Position::new(0, 0), lsp::Position::new(0, 1)),
                    command: Some(lsp::Command {
                        title: format!("lens-{index}"),
                        command: "test.command".to_string(),
                        arguments: None,
                    }),
                    data: None,
                }]))
            }
        }
    });
    fake_server.set_request_handler::<lsp::request::SemanticTokensFullRequest, _, _>({
        let counts = counts.clone();
        move |_, _| {
            *counts.lock().entry("semantic_tokens").or_default() += 1;
            async move {
                Ok(Some(lsp::SemanticTokensResult::Tokens(
                    lsp::SemanticTokens {
                        result_id: None,
                        data: semantic_token_test_data(index),
                    },
                )))
            }
        }
    });
    fake_server.set_request_handler::<lsp::request::InlayHintRequest, _, _>({
        let counts = counts.clone();
        move |_, _| {
            *counts.lock().entry("inlay_hints").or_default() += 1;
            async move {
                Ok(Some(vec![lsp::InlayHint {
                    position: lsp::Position::new(0, 0),
                    label: lsp::InlayHintLabel::String(format!("hint-{index}")),
                    kind: None,
                    text_edits: None,
                    tooltip: None,
                    padding_left: None,
                    padding_right: None,
                    data: None,
                }]))
            }
        }
    });
    counts
}

fn semantic_token_test_data(index: u8) -> Vec<u32> {
    vec![0, u32::from(index - 1) * 4, 1, 0, 0]
}

#[derive(Debug, Default, PartialEq)]
struct LspDataLabels {
    colors: Vec<String>,
    links: Vec<String>,
    folding_ranges: Vec<String>,
    document_symbols: Vec<String>,
    code_lens: Vec<String>,
    semantic_tokens: Vec<(LanguageServerId, Vec<u32>)>,
    inlay_hints: Vec<(LanguageServerId, Vec<String>)>,
}

impl LspDataLabels {
    fn sort(&mut self) {
        self.colors.sort();
        self.links.sort();
        self.folding_ranges.sort();
        self.document_symbols.sort();
        self.code_lens.sort();
        self.semantic_tokens.sort();
        self.inlay_hints.sort();
    }
}

/// The expected [`fetch_all_lsp_data`] results for servers set up with [`serve_lsp_data`].
fn lsp_data_labels(servers: &[(LanguageServerId, u8)]) -> LspDataLabels {
    let mut labels = LspDataLabels::default();
    for &(server_id, index) in servers {
        labels.colors.push(format!("color-{index}"));
        labels
            .links
            .push(format!("https://server-{index}.example/"));
        labels.folding_ranges.push(format!("fold-{index}"));
        labels.document_symbols.push(format!("symbol-{index}"));
        labels.code_lens.push(format!("lens-{index}"));
        labels
            .semantic_tokens
            .push((server_id, semantic_token_test_data(index)));
        labels
            .inlay_hints
            .push((server_id, vec![format!("hint-{index}")]));
    }
    labels.sort();
    labels
}

async fn fetch_all_lsp_data(
    project: &Entity<Project>,
    buffer: &Entity<Buffer>,
    inlay_hints_invalidate: InvalidationStrategy,
    cx: &mut gpui::TestAppContext,
) -> LspDataLabels {
    let lsp_store = project.read_with(cx, |project, _| project.lsp_store());
    let buffer_id = buffer.read_with(cx, |buffer, _| buffer.remote_id());
    let colors = lsp_store.update(cx, |lsp_store, cx| {
        lsp_store.document_colors(buffer.clone(), cx)
    });
    let links = lsp_store.update(cx, |lsp_store, cx| {
        lsp_store.fetch_document_links(buffer, cx)
    });
    let folding_ranges = lsp_store.update(cx, |lsp_store, cx| {
        lsp_store.fetch_folding_ranges(buffer, cx)
    });
    let document_symbols = lsp_store.update(cx, |lsp_store, cx| {
        lsp_store.fetch_document_symbols(buffer, cx)
    });
    let code_lens = lsp_store.update(cx, |lsp_store, cx| lsp_store.code_lens_actions(buffer, cx));
    let semantic_tokens = lsp_store.update(cx, |lsp_store, cx| {
        lsp_store.semantic_tokens(buffer.clone(), cx)
    });
    let inlay_hint_tasks = lsp_store.update(cx, |lsp_store, cx| {
        let query_range =
            buffer.read(cx).anchor_before(0)..buffer.read(cx).anchor_after(buffer.read(cx).len());
        lsp_store.inlay_hints(
            inlay_hints_invalidate,
            buffer.clone(),
            vec![query_range],
            None,
            cx,
        )
    });

    let mut labels = LspDataLabels::default();
    if let Some(colors) = colors {
        labels.colors.extend(
            colors
                .await
                .unwrap()
                .colors
                .into_iter()
                .map(|color| format!("color-{}", color.color.alpha as u8)),
        );
    }
    labels.links.extend(
        links
            .await
            .unwrap_or_default()
            .values()
            .flat_map(|server_links| server_links.values())
            .filter_map(|link| link.target.clone())
            .map(|target| target.to_string()),
    );
    labels.folding_ranges.extend(
        folding_ranges
            .await
            .into_iter()
            .filter_map(|folding_range| folding_range.collapsed_text)
            .map(|collapsed_text| collapsed_text.to_string()),
    );
    labels.document_symbols.extend(
        document_symbols
            .await
            .into_iter()
            .map(|symbol| symbol.text.to_string()),
    );
    labels.code_lens.extend(
        code_lens
            .await
            .unwrap()
            .unwrap_or_default()
            .values()
            .map(|action| action.lsp_action.title().to_string()),
    );
    semantic_tokens.await.unwrap();
    labels.semantic_tokens =
        lsp_store.read_with(cx, |lsp_store, _| lsp_store.semantic_token_data(buffer_id));
    for (_, hints_task) in inlay_hint_tasks {
        for (server_id, server_hints) in hints_task.await.unwrap() {
            let hint_labels = server_hints
                .into_iter()
                .map(|(_, hint)| hint.text().to_string())
                .collect::<Vec<_>>();
            if !hint_labels.is_empty() {
                labels.inlay_hints.push((server_id, hint_labels));
            }
        }
    }
    labels.sort();
    labels
}
