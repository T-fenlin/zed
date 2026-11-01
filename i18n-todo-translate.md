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
