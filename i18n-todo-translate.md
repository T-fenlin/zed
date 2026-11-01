# i18n 待补译清单（无 key 的新文案，统一补译）

> 这些英文串在代码里出现、但 `assets/locales` 里还没有对应的 key，暂不改代码（保持英文），
> 待后续统一补：给 en-US + zh-CN 两份 ftl 加 key，并在 keys.rs 生成常量后再替换。

## 已发现
| 英文原文 | 位置 | 备注 |
|---|---|---|
| Open Project in This Window | crates/recent_projects/src/recent_projects.rs | 与 sidebar-open-project-new-window 成对 |
| Current Folders | crates/recent_projects/src/recent_projects.rs | 选择器 Header |
| This Window | crates/recent_projects/src/recent_projects.rs | 选择器 Header |
| Remove Key | crates/ui/src/components/ai/configured_api_card.rs | 与 Reset Key 成对 |
| Failed to connect over SSH | crates/recent_projects/src/remote_connections.rs | prompt message |
| Failed to connect to WSL | crates/recent_projects/src/remote_connections.rs | prompt message |
| Failed to connect to Dev Container | crates/recent_projects/src/remote_connections.rs | prompt message |
| Failed to connect | crates/recent_projects/src/remote_servers.rs | prompt message |
| Save all | crates/workspace/src/pane.rs | prompt 按钮 |
| Discard all | crates/workspace/src/pane.rs | prompt 按钮 |
| Close Without Saving | crates/workspace/src/pane.rs | prompt 按钮 |
| Overwrite | crates/workspace/src/pane.rs | prompt 按钮 |
| Discard Edits | crates/workspace/src/pane.rs | prompt 按钮 |
| Don't Save | crates/workspace/src/pane.rs | prompt 按钮 |

## 结构性待补译（代码结构限制，无法直接用 t()，需改逻辑/API 后补）
- crates/settings_ui/src/pages/sandbox_settings.rs: const DOMAINS_DESCRIPTION / WRITE_PATHS_DESCRIPTION（const &str 不能调 t()，需改使用处）
- crates/settings_ui/src/pages/audio_input_output_setup.rs: const SYSTEM_DEFAULT（同上）
- crates/settings_ui/src/pages/skills_setup.rs: window.prompt(... &["Delete","Cancel"] ...)（&[&str] 类型，需改 prompt API 才能本地化）
- 有意不译（逻辑/遥测，非遗漏）：settings_ui.rs setting_type() 返回值(telemetry type)、banner() label(telemetry label)、title.as_ref()=="LLM Providers" 比较


## render_settings_item_layout 参数限制（需把 SharedString::new_static 改成 owned 才能本地化）
- 该函数 title/description 用 SharedString::new_static(title)（要求 &'static str），无法直接传运行时 t()
- 受影响未译项：external_agents_page.rs 的 Agent Name/Command/Arguments/Environment Variables 及其 hint（共 8 处）；mcp_servers_page.rs 同类（2 处）


## edit_prediction_provider_setup.rs 参数类型限制（需改 &'static str→SharedString 的辅助函数）
- render_api_key_provider 的 title 参数是 &'static str → 未译 "OpenAI Compatible API"(L90)
- ConfiguredApiCard::new(label) / button_label(&str) → 未译 "API Key Set in Environment Variable"/"API Key Configured"(L286/288)/"Reset Key"(L294)
- display_name().unwrap_or("No provider set") → unwrap_or 需 &str → 未译(L133)


## mcp_servers_page.rs 同类 new_static 限制（18 处未译）
- Server Name/Command/Arguments/Environment Variables/Timeout/URL/Headers/OAuth Client ID 及其 description，均经 render_form_field→render_settings_item_layout(SharedString::new_static) 无法传 t()

