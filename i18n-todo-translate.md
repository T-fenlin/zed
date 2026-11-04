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

### 4. 失败提示 / 确认框收尾（第二批）

- `git_ui/src/git_panel.rs`：单文件与批量放弃确认框、上下文菜单全部标签、git init 提示、`No changes to commit`、提交流误「still conflicts」、`already pushed to` 详情
- `git_ui/src/branch_picker.rs`：未合并分支强制删除确认框与 `Force Delete`/`Cancel` 按钮、`Failed to create branch`/`create remote`/`change branch`
- `git_ui/src/stash_picker.rs`：`Failed to drop/pop/apply stash`
- `git_ui/src/git_ui.rs`：`Failed to rename branch`、`Failed to create tag`
- `git_ui/src/commit_view.rs`：stash 三动作（Apply/Pop/Drop）名称与 `{ $action } stash@{ $index }?` 确认框
- `git_ui_core/src/notifications.rs`：错误 toast 标题（`git { $action } failed`、`Push rejected…`）、`View Log`、`Output from git { $operation }`
- `git_ui_core/src/worktree_picker.rs`：`Failed to connect`
- `workspace/src/workspace.rs` + `collab_ui/src/collab_panel.rs`：`Failed to join channel` 及 6 条错误码明细
- `i18n/src/lib.rs`：新增 `ftl_resources_parse_cleanly`（全量 ftl 解析校验）与 `multiline_value_keeps_blank_line` 两个测试 —— 顺带修掉 `collab-screen_permission_hint` / `collab-join-error_try_again` 里无效的 `\n` 转义（Fluent 普通文本不处理转义，会原样显示反斜杠），改为多行值

## 二、仍待处理（后续）

> 以下为超出原清单范围、本轮未改的新发现项，需先补 key 或先引入 i18n 依赖。

### project_panel（本轮已完成，i18n 已提为常规依赖）
- `build_removal_prompt` 全面改走 `t()`/`t_with_args()`：单/多文件 × trash/delete 四种确认框、`.. 另有 N 个文件未显示`、未保存更改提示、`This action cannot be undone.`
- 右键菜单 30+ 条标签（新建/打开/剪切/复制/粘贴/撤销/重做/复制路径/git 相关/重命名/移到废纸篓/删除/展开折叠等）全部接入 `panel-*` key
- `restore_file` 的 `Discard changes to …?`、替换确认框的 `Replace`、`Failed to open file` 及错误码明细（SSH/远程断开、未共享路径）
- 测试同步：`assert_prompt`、`pending_prompt` 断言改为对照 `t()`/`t_with_args()`；`cx.answer("Trash")` 等应答改用 `t(keys::panel::PANEL_TRASH)`

### vim / open_path_prompt（本轮已完成，均已引入 i18n 依赖）
- `vim/src/command.rs`：`Argument required`、`No file name`、`Use ! to write partial buffer`、`Failed to save`（3 处）、两处「已存在是否替换」确认框与 `Replace` 按钮
- `open_path_prompt.rs`：「已存在是否替换」确认框
- 共用文案放在 `misc` domain：`file_exists_prompt`、`file_exists_detail`、`replace`、`vim-*`

### auto_update / feedback / install_cli（本轮已完成）
- `auto_update.rs`：包管理器安装说明、`Could not check for updates`
- `feedback.rs`：`Copied into clipboard`（2 处）
- `install_cli_binary.rs`：Linux 路径提示（改多行值）、安装失败通知、安装成功 toast

### 5. 失败提示 / 确认框收尾（第三批）

- `collab_ui`：`Failed to join project`、`grant mic/write access`、`revoke access`、`create channel`、`hang up`、`set channel visibility`、`move channel`、`leave channel`、`remove contact`、`respond to contact request`、`Call failed`、`update role`、`remove member`、`invite member`（全部复用已存在的 `collab-failed_to_*` key）
- `recent_projects`：`Failed to open project`（3 处）、`Cannot open Dev Container from remote project`、`Failed to start Dev Container…`、`Failed to reconnect`
- `workspace` / `pane_group`：`Failed to join project`、`Failed to save`（4 处）
- `editor`：`Failed to create buffer`
- `debugger_ui`：`Failed to edit debug.json`
- `vim`：`Failed to write lines`、`Failed to :w`（2 处）
- `install_cli`：`Cannot install the Zed CLI`
- `zed`：退出确认框、zed:// scheme 注册失败、不支持的 GPU 提示（4 段文案拼装）、inotify / ReadDirectoryChangesW 初始化失败提示与 `Troubleshoot and Quit` / `Skip` 按钮
- 补充扫描：`show_error(...)` 系列 5 处（此前只清 `prompt` / `prompt_err` / `detach_and_prompt_err`，漏掉了 toast 通道）
  - `collab_ui`：`There's no active call; join one first.` → 复用 `collab-no_active_call`
  - `workspace/pane.rs`：`Cannot drop files on a remote project`、`Cannot drop local files on a remote SSH/Docker project`、`Could not translate the dropped paths into WSL paths`
  - `workspace/workspace.rs`：`You cannot add folders to someone else's project`
  - 注意：`show_error` 签名是 `E: WorkspaceError`，`String` 有实现、`&String` 没有 → 直接传 `t(...)`，不要加 `&`

### 仍待处理
- `zed/src/zed/move_to_applications.rs`（**仅 macOS 编译**）：`Move Zed to Applications?` 等 3 条文案。当前开发机为 Windows，改了也无法编译验证，暂不动，留待 macOS 环境补。
- `ui` gallery preview / `agent_ui` thread_item 等组件预览里的英文示例文案（演示数据，是否译可再定）

### 有意不译（逻辑/遥测，非遗漏）
- `settings_ui.rs` `setting_type()` 返回值（telemetry type）、`banner()` label（telemetry label）、`title.as_ref() == "LLM Providers"` 比较
- 日志（`log::*`）、断言消息、测试用 `expect`/`panic` 文本
- 品牌名：Mercury、Codestral、Ollama、GitHub Copilot、OpenAI 等
