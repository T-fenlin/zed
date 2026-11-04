#!/usr/bin/env python3
"""i18n 接线（类型安全子集）：只替换目标类型可接受 String 的位置。

允许替换的上下文（目标为 impl Into<SharedString> / String）：
  Label::new(LIT)  button(LIT, ...)  .child(LIT)  Tooltip::simple(LIT, ...)
  .label(LIT) .title(LIT) .tooltip(LIT) .placeholder(LIT)
  field: LIT.into()   field: LIT,   Some(LIT.to_string())  LIT.into()
  editor.set_placeholder_text(LIT, ...)   .action(LIT, ...)

拒绝（&'static str 上下文，需结构性改造）：
  & [LIT, ...]  数组字面量、Some(LIT)（Option<&str>）、match 臂、常量、
  fn -> &'static str、json!、assert、注释行
"""

import glob
import os
import re
import sys
from collections import defaultdict

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
EN_DIR = os.path.join(ROOT, "assets", "locales", "en-US")

OWNER = {
    "settings-ui": ["settings_ui"], "git-ui": ["git_ui"],
    "collab": ["collab_ui"], "agent": ["agent_ui", "crates/agent/"],
    "acp": ["acp_thread"], "app-menu": ["crates/zed/"],
    "panel": ["project_panel"], "sidebar": ["sidebar"],
    "editor": ["crates/editor/"], "search": ["search"],
    "workspace": ["crates/workspace/"], "titlebar": ["title_bar"],
    "keymap": ["keymap_editor"], "extensions": ["extensions_ui"],
    "edit-prediction": ["edit_prediction"], "debugger": ["debugger_ui"],
    "repl": ["repl"], "outline": ["outline_panel"], "recent": ["recent_projects"],
    "pane": ["crates/workspace/"], "tab": ["crates/editor/"],
    "lang": ["crates/zed/"], "ui": ["crates/ui/"], "zed": ["crates/zed/"],
    "misc": [], "errors": [], "prompt": [], "test": [],
}

SAFE = re.compile(
    r"(?:Label::new\(|button\(|\.child\(|Tooltip::simple\(|\.label\(|\.title\(|"
    r"\.tooltip\(|\.placeholder\(|set_placeholder_text\(|\.action\(|\.tooltip_label\(|"
    r"\b\w+:\s*)LIT"
)
DENY_LINE = re.compile(r"^\s*(///|//|\*)|&\[|json!|assert|KeyBinding::new|aria\.|\bconst\b|"
                       r"->\s*(?:Option<)?&")


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


def const_name(k):
    return re.sub(r"[^A-Z0-9]", "_", k.upper())


def main():
    apply_flag = "--apply" in sys.argv
    only = sys.argv[sys.argv.index("--domain") + 1] if "--domain" in sys.argv else None
    domains = {os.path.splitext(os.path.basename(p))[0]: parse_ftl(p)
               for p in glob.glob(os.path.join(EN_DIR, "*.ftl"))}
    targets = [only] if only else [d for d in sorted(domains) if OWNER.get(d)]

    total, modified, samples = 0, set(), []
    for d in targets:
        values = defaultdict(list)
        for k, v in domains[d].items():
            if "{" not in v:
                values[v].append(k)
        ns = d.replace("-", "_")
        for p in glob.glob(os.path.join(ROOT, "crates", "**", "*.rs"), recursive=True):
            norm = p.replace("\\", "/")
            if "/target/" in norm or not any(o in norm for o in OWNER[d]):
                continue
            try:
                lines = open(p, encoding="utf-8").read().splitlines(keepends=True)
            except (OSError, UnicodeDecodeError):
                continue
            changed = False
            for i, line in enumerate(lines):
                if "t(keys::" in line or DENY_LINE.search(line):
                    continue
                for v, keys in values.items():
                    if len(keys) != 1:
                        continue
                    lit = f'"{v}"'
                    if lit not in line:
                        continue
                    # 仅当字面量出现在允许的上下文里
                    if not SAFE.search(line.replace(lit, "LIT")):
                        # 也接受形如  "X".into() / Some("X".to_string())
                        if not re.search(re.escape(lit) + r"\.into\(\)|" + re.escape(lit) + r"\.to_string\(\)", line):
                            continue
                    lines[i] = line.replace(lit, f"t(keys::{ns}::{const_name(keys[0])})")
                    total += 1
                    changed = True
                    if len(samples) < 25:
                        samples.append(f"{os.path.relpath(p, ROOT)}:{i+1}  {lines[i].strip()[:95]}")
            if changed:
                modified.add(p)
                if apply_flag:
                    open(p, "w", encoding="utf-8", newline="").write("".join(lines))

    print(f"{'' if apply_flag else '[dry-run] '}替换处数: {total}, 文件: {len(modified)}")
    for s in samples:
        print("  ", s)

    if apply_flag:
        for p in sorted(modified):
            s = open(p, encoding="utf-8").read()
            if "use i18n" in s:
                continue
            m = re.search(r"^use [^;]+;", s, re.M)
            if m:
                imp = "use i18n::{keys, t};" if "t(keys::" in s else "use i18n::keys;"
                open(p, "w", encoding="utf-8", newline="").write(s[: m.start()] + imp + "\n" + s[m.start():])


if __name__ == "__main__":
    main()
