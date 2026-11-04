#!/usr/bin/env python3
"""i18n 批量接线器（保守版）：把硬编码英文按 en-US ftl 值反向替换为 t(keys::<domain>::CONST)。

安全策略（宁可少换，不可错换）：
  1. 只在域名归属 crate 内替换（OWNER 映射），不跨 crate
  2. 跳过含 Fluent 占位符（{ $x }）的值 —— 需 t_with_args，人工处理
  3. 跳过过短/过于通用的值（单 token 且 < 10 字符）
  4. 仅替换"UI 标签上下文"的行（.label(/.title(/label:/... 等），
     且该行不含 const / &'static str / assert / json! / KeyBinding / aria / serde / =>
  5. 跳过同域内对应多个 key 的值

用法：
  python script/i18n_wire.py --domain settings-ui --dry-run
  python script/i18n_wire.py --domain settings-ui --apply
"""

import glob
import os
import re
import sys
from collections import defaultdict

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
EN_DIR = os.path.join(ROOT, "assets", "locales", "en-US")
LOCALE_DOMAINS = None

OWNER = {
    "settings-ui": ["settings_ui"],
    "git-ui": ["git_ui"],
    "collab": ["collab_ui", "collab"],
    "agent": ["agent_ui", "crates/agent/"],
    "acp": ["acp_thread"],
    "app-menu": ["crates/zed/"],
    "panel": ["project_panel"],
    "sidebar": ["sidebar"],
    "editor": ["crates/editor/"],
    "search": ["search"],
    "workspace": ["crates/workspace/"],
    "titlebar": ["title_bar"],
    "keymap": ["keymap_editor"],
    "extensions": ["extensions_ui"],
    "edit-prediction": ["edit_prediction"],
    "debugger": ["debugger_ui"],
    "repl": ["repl"],
    "outline": ["outline_panel"],
    "recent": ["recent_projects"],
    "pane": ["crates/workspace/"],
    "tab": ["crates/editor/"],
    "lang": ["crates/zed/"],
    "ui": ["crates/ui/"],
    "zed": ["crates/zed/"],
}

ALLOW = re.compile(
    r"\.label\(|\.title\(|\blabel:|\btext:|\bname:|\bplaceholder|tooltip|\bheading|"
    r"\bdescription\b|\bmessage\b|Label::new\(|Button::new\(|SharedString::from\(|"
    r"\bsingle_example\(|\baria_label|\bchild\("
)
DENY = re.compile(
    r"\bconst\b|&'static str|assert|json!|KeyBinding::new|aria\.|serde|=>|"
    r"fn \w+\([^)]*\)\s*->|\.to_owned\(\)|\.to_string\(\)\s*$"
)


def parse_ftl(path):
    out, key = {}, None
    for raw in open(path, encoding="utf-8"):
        s = raw.rstrip("\n")
        m = re.match(r"^([a-z0-9][a-z0-9_-]*)\s*=\s*(.*)$", s)
        if m:
            key, out[m.group(1)] = m.group(1), m.group(2).strip()
        elif key and s.startswith("    "):
            out[key] = (out[key] + " " + s.strip()).strip()
        elif key and s.strip() == "":
            key = None
    return out


def const_name(key):
    return re.sub(r"[^A-Z0-9]", "_", key.upper())


def main():
    apply_flag = "--apply" in sys.argv
    only = None
    if "--domain" in sys.argv:
        only = sys.argv[sys.argv.index("--domain") + 1]

    domains = {}
    for p in glob.glob(os.path.join(EN_DIR, "*.ftl")):
        domains[os.path.splitext(os.path.basename(p))[0]] = parse_ftl(p)

    targets = [only] if only else [d for d in sorted(domains) if d in OWNER]

    files = [p for p in glob.glob(os.path.join(ROOT, "crates", "**", "*.rs"), recursive=True)
             if "/target/" not in p.replace("\\", "/")]

    total, modified = 0, set()
    skipped = defaultdict(int)
    samples = []

    for d in targets:
        if d not in domains or d not in OWNER:
            print(f"跳过（无归属映射）: {d}")
            continue
        owners = OWNER[d]
        values = defaultdict(list)
        for k, v in domains[d].items():
            if "{" in v:
                skipped["含占位符"] += 1
                continue
            if len(v.replace("…", "").strip()) < 10 or (" " not in v and len(v) < 12):
                skipped["过短/通用"] += 1
                continue
            values[v].append(k)

        ns = d.replace("-", "_")
        for p in files:
            norm = p.replace("\\", "/")
            if not any(o in norm for o in owners):
                continue
            try:
                lines = open(p, encoding="utf-8").read().splitlines(keepends=True)
            except (OSError, UnicodeDecodeError):
                continue
            changed = False
            for i, line in enumerate(lines):
                if "t(keys::" in line or not ALLOW.search(line) or DENY.search(line):
                    continue
                for v, keys in values.items():
                    if len(keys) != 1:
                        continue
                    lit = f'"{v}"'
                    if lit in line:
                        lines[i] = line.replace(lit, f"t(keys::{ns}::{const_name(keys[0])})")
                        line = lines[i]
                        total += 1
                        changed = True
                        if len(samples) < 400:
                            samples.append(f"{os.path.relpath(p, ROOT)}:{i+1}  {lines[i].strip()[:100]}")
            if changed:
                open(p, "w", encoding="utf-8", newline="").write("".join(lines))
                modified.add(p)

    print(f"{'[dry-run] ' if not apply_flag else ''}替换处数: {total}, 文件: {len(modified)}")
    print("跳过统计:", dict(skipped))
    for s in samples[:30]:
        print("  ", s)

    if not apply_flag:
        return

    # 补 import
    for p in sorted(modified):
        s = open(p, encoding="utf-8").read()
        if "use i18n" in s:
            continue
        m = re.search(r"^use [^;]+;", s, re.M)
        if m:
            imp = "use i18n::{keys, t};" if "t(keys::" in s else "use i18n::keys;"
            s = s[: m.start()] + imp + "\n" + s[m.start():]
            open(p, "w", encoding="utf-8", newline="").write(s)
    print(f"已写入 {len(modified)} 个文件")


if __name__ == "__main__":
    main()
