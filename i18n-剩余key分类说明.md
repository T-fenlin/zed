# i18n 剩余未接线 key 分类说明（当前 230 条）

> 由 `python script/i18n_audit.py --verbose` 实时生成。这些 key 的**翻译已在 ftl 里**，
> 但源码里还没引用（未接线）。下面按「是不是多余的 / 重复的」分四类，你看完就知道哪些可删、哪些值得接。

## 一、真·重复 key（可直接删，约 70+ 条）
FTL 从二进制重建时，对**同一句英文**按出现次数生成了 `_2 / _3 / _4` 后缀的重复项。
它们和已有的基础 key 值完全一样，接任何一个效果相同，留着没意义。典型：
- `settings-appearance-font_family_2/3/4`、`font_size_2/3`、`font_weight_2`、`font_features_2`
- `settings-panels-file_icons_2/3`、`folder_indicator_2/3`、`starts_open_2/3`、`git_status_2`、`indent_size_2`、`amount_of_indentation..._2`、`auto_fold_directories_2`、`auto_reveal_entries_2`、`show_indent_guides_2`、`show_count_badge_2`
- `settings-network-..._2`、`settings-window-...`
→ **建议：删除这些 _2/_3/_4 条目（ftl + keys.rs），是真正「多余的」。**

## 二、enum 显示名（24 条，建议保留、不接）
`settings-enum-*`（all/always/auto/bottom/center/.../visible 共 24 个）：
这类是下拉枚举的显示名，代码里走 match 逻辑映射或 Display，不通过 key 直接取词。
→ **不算多余，但也不需要单独接线**；若追求彻底可改成用 key，属低优先。

## 三、源码当前 checkout 里不存在对应文案的「死翻译」（约 100 条，建议删）
settings-ui / agent / zed / edit-prediction / extensions 里不少 key，比如：
- `settings-ui_locale*`（locale / Interface Language 整页在本分支不存在）
- `settings-panels-section-*_panel`、`settings-network-*`（部分 section 名此分支无 UI）
- `agent-*` 42 条（i18n-agent 报告：3 个 crate 源码里 grep 不到这些英文串）
- `zed-b-*` 19 条（多为 enum→动作显示名逻辑映射、macOS-only、或重复）
→ **这些是「重建自另一版本二进制」的残留，此分支无调用点，属真正的多余，可删。**

## 四、可能有对应实现、值得再接的（建议先核实再接）
- `extensions-sort_*`、`extensions-version*`、`extensions-install*`（扩展页排序/版本 UI，可能在 extensions_ui 有实现待接）
- `prompt-close / prompt-replace`、`debugger-*`、`git-clone-* / git-picker-toggle_*`、`tab-rename`
- `misc: drop-remote / models`
→ 需要逐个 grep 核实源码有没有对应字面量，有就接、没有就归入「死翻译」删。

---

## ⚠️ 另外发现的【真·漏译】（不在上表，因为连 ftl key 都没有）
- **欢迎页 welcome.rs**：`Welcome back to Zed` / `Welcome to Zed` / `The editor for what's next` / `Return to Onboarding` / 标签页标题 `Welcome` —— 全部硬编码，无 key。
- **终端右键菜单 terminal_view.rs:600-643**：`New Terminal` / `New Center Terminal` / `Copy` / `Paste` / `Paste Text` / `Select All` / `Clear` / `Inline Assist` / `Add to Agent Thread` / `Close Terminal Tab` —— 多数有现成 key 可复用，缺 `Paste Text`。
