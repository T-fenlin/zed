# i18n 待补译清单

> 规则：英文目录 `assets/locales/en-US/` 为唯一事实来源；中文译文写入 `assets/locales/zh-CN/`；
> 代码统一通过 `i18n::t()` / `t_with_args()` 取文案，key 常量集中在 `crates/i18n/src/keys.rs`。

## 一、已完成（本轮）

### 1. 原「无 key 的新文案」13 条 —— 全部补齐 key 并替换调用点

| 英文原文 | 位置 | key | 状态 |
|---|---|---|---|
| Open Project in This Window | recent_projects.rs、sidebar_recent_projects.rs | `recent-open_project_this_window` | ✅ |
| Current Folders | recent_projects.rs | `recent-current_folders` | ✅ |
| This Window | recent_projects.rs、threads_archive_view.rs、worktree_picker.rs | `recent-this_window`（本轮新增） | ✅ |
| Remove Key | ui/components/ai/configured_api_card.rs（gallery preview） | `ui-remove_key` | ✅ |
| Failed to connect over SSH | remote_connections.rs | `recent-failed_connect_ssh` | ✅ |
| Failed to connect to WSL | remote_connections.rs | `recent-failed_connect_wsl` | ✅ |
| Failed to connect to Dev Container | remote_connections.rs | `recent-failed_connect_dev_container` | ✅ |
| Failed to connect | remote_servers.rs（5 处） | `recent-failed_connect`（本轮新增） | ✅ |
| Save all / Discard all / Close Without Saving / Overwrite / Discard Edits / Don't Save | pane.rs、workspace.rs、project_search.rs | `prompt-*` 系列 | ✅ |

同时补译了同一批 prompt 的消息文本：
- pane.rs：`prompt-save_files`、`prompt-unable_to_save_file`、`prompt-overwrite_changed`、`prompt-recreate_deleted`、`prompt-save_edits`、`prompt-save_edits_unnamed`
- workspace.rs：`prompt-save_all_changes`、`prompt-leave_call`、`prompt-close_window_and_hang_up`、`prompt-switch_channels`、`prompt-leave_call_unshare`、`prompt-yes_join_channel`、`prompt-restart_confirm`、`prompt-restart`
- project_search.rs：`search-unsaved_prompt`、`search-save`、`search-dont_save`
- remote_connections.rs：`agent-a-registry_retry`（Retry 按钮）
- debugger_panel.rs：`prompt-terminate_debug_session`、`prompt-yes`、`prompt-no`
- git_ui/clone.rs：`prompt-git_clone`、`prompt-add_repo_to_project`、`prompt-open_repo_in_new_project`
- collab_ui/collab_panel.rs：`prompt-leave_channel`、`prompt-remove_channel`、`prompt-remove_contact`、`collab-leave`、`collab-remove`
- onboarding.rs：`prompt-import_vscode_settings`、`prompt-import`、`prompt-could_not_find_settings`（本轮新增 key）

### 2. 原「结构性待补译」—— 已通过改代码结构解决

- `sandbox_settings.rs`：`DOMAINS_DESCRIPTION` / `WRITE_PATHS_DESCRIPTION` 由 const 改为 `domains_description()` / `write_paths_description()` 函数，接入 `settings-b-sandbox-domains-desc`、`settings-b-sandbox-writable-paths-desc` ✅
- `audio_input_output_setup.rs`：`SYSTEM_DEFAULT` const 改为 `system_default()` 函数，接入 `settings-b-audio-system-default` ✅
- `skills_setup.rs`：`&["Delete","Cancel"]` 改为 `t()` 取词 + 消息改用 `settings-a-skills-delete_prompt_project/global`、`settings-a-skills-delete_detail` ✅
- `edit_prediction_provider_setup.rs`：`render_api_key_provider` 的 `title` 参数由 `&'static str` 加宽为 `impl Into<SharedString>`；`display_name().unwrap_or(...)` 改为 `unwrap_or_else(|| t(...))`；接入 `settings-b-ep-openai-compatible`、`settings-b-ep-no-provider`、`settings-b-ep-api-key-env/configured`、`settings-b-ep-reset-key`、`settings-b-api-key-reset-tooltip`、`settings-b-ep-env-var-hint`、`settings-b-ep-api-key-bearer` ✅
- `external_agents_page.rs`、`mcp_servers_page.rs`：`render_settings_item_layout` 已接受 `SharedString`，两页表单字段此前已全部接入 `t()`；本轮补齐遗漏的空状态/标题/错误消息（`settings-a-mcp-page_title` 等）与 `collect_kv` 的重复项报错（`settings-b-ext-agent-duplicate`、`settings-a-mcp-duplicate_kv`、`settings-a-mcp-invalid_url`） ✅
- `llm_providers_page.rs`：两处 env-var tooltip/hint 接入 `settings-b-api-key-reset-tooltip`、`settings-b-llm-env-var-hint` ✅

### 3. 测试基础设施

- `gpui` 测试平台 `simulate_prompt_answer` 增加兜底匹配：OK/Cancel 按钮按 `PromptButton` 类型匹配，避免 i18n 本地化后英文应答失配。
- 所有应答已本地化按钮的测试改为 `simulate_prompt_answer(&t(keys::prompt::...))`，与界面语言无关：
  - workspace：pane.rs（8 处）、workspace.rs（7 处）、multi_workspace_tests.rs（1 处）
  - zed：zed.rs（7 处）
  - project_panel：tests/undo.rs（1 处，新增 i18n dev-dependency）
  - collab：tests/integration/following_tests.rs（1 处，新增 i18n dev-dependency）
- settings_ui 单测断言改为对照 `t()` 结果：external_agents_page.rs（3 处）、mcp_servers_page.rs（7 处）
- agent_ui/profile_selector.rs、git_ui_core/worktree_picker.rs 的 Header 标签断言改为对照 `t()` 结果（git_ui_core 新增 i18n 依赖）

## 二、仍待处理（后续）

> 以下为超出原清单范围、本轮未改的新发现项，需先补 key 或先引入 i18n 依赖。

### git_ui（已依赖 i18n，可直接补）
- `git_panel.rs`：`"Are you sure you want to discard changes to \`{path}\`?"`、`"Discard changes to these files?"`、`"Unable to initialize a git repository"` 等 discard/revert 提示（注意测试 `pending_prompt()` 会对消息文本做断言）
- `worktree_picker.rs`：其它提示消息

### project_panel（i18n 目前仅为 dev-dependency）
- `project_panel.rs`：`"Do you want to trash"`、`"Replace"` 等删除/重命名提示
- 若要在生产代码中使用 `t()`，需把 `i18n` 从 dev-dependencies 提升为常规依赖

### vim / open_path_prompt（尚无 i18n 依赖）
- `vim/src/command.rs`：`"Argument required"`、`"No file name"`、`"Use ! to write partial buffer"`、`"Replace"`
- `open_path_prompt.rs`：`"Replace"`

### workspace
- `workspace.rs`：`"Failed to join channel"`（尚无对应 key）

### 其它 crate（无 i18n 依赖，需逐个引入）
- `auto_update.rs`：`"Zed was installed via a package manager."`、`"Could not check for updates"`
- `feedback.rs`：`"Copied into clipboard"`
- `install_cli_binary.rs`、`move_to_applications.rs`
- `ui` gallery preview / `agent_ui` thread_item 等组件预览中的演示文案（英文示例，是否译可再定）

### 有意不译（逻辑/遥测，非遗漏）
- `settings_ui.rs` `setting_type()` 返回值（telemetry type）、`banner()` label（telemetry label）、`title.as_ref() == "LLM Providers"` 比较
- 日志（`log::*`）、断言消息、测试用 `expect`/`panic` 文本
- 品牌名：Mercury、Codestral、Ollama、GitHub Copilot、OpenAI 等
