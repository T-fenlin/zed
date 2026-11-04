#!/usr/bin/env python3
"""i18n 接线（静态切片上下文）：把仍为 &'static str 的硬编码改用 t_static()。

用于数组字面量、match 臂、Option<&'static str> 字段等无法直接接受 String 的位置。
跳过：注释、const/static 定义、含 Fluent 占位符的值、同域多 key 的值。
"""

import glob
import os
import re
import sys
from collections import defaultdict

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
EN_DIR = os.path.join(ROOT, "assets", "locales", "en-US")

OWNER = {
    "settings-ui": ["settings_ui"], "git-ui": ["git_ui"], "collab": ["collab_ui"],
    "agent": ["agent_ui", "crates/agent/"], "acp": ["acp_thread"],
    "app-menu": ["crates/zed/"], "panel": ["project_panel"], "sidebar": ["sidebar"],
    "editor": ["crates/editor/"], "search": ["search"], "workspace": ["crates/workspace/"],
    "titlebar": ["title_bar"], "keymap": ["keymap_editor"], "extensions": ["extensions_ui"],
    "edit-prediction": ["edit_prediction"], "debugger": ["debugger_ui"], "repl": ["repl"],
    "outline": ["outline_panel"], "recent": ["recent_projects"], "pane": ["crates/workspace/"],
    "tab": ["crates/editor/"], "lang": ["crates/zed/"], "ui": ["crates/ui/"],
    "zed": ["crates/zed/"],
}

SKIP = re.compile(r"^\s*(///|//|\*)|^\s*(pub )?(const|static) ")


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
            if "{" not in v and len(v) >= 3:
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
                if "t(keys::" in line or "t_static(" in line or SKIP.search(line):
                    continue
                for v, keys in values.items():
                    if len(keys) != 1:
                        continue
                    lit = f'"{v}"'
                    if lit not in line:
                        continue
                    lines[i] = line.replace(lit, f"t_static(keys::{ns}::{const_name(keys[0])})")
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

    if not apply_flag:
        return
    for p in sorted(modified):
        s = open(p, encoding="utf-8").read()
        m = re.search(r"^use i18n::\{([^}]*)\};", s, re.M)
        if m:
            names = {n.strip() for n in m.group(1).split(",") if n.strip()}
            names.add("keys")
            if "t_static(" in s:
                names.add("t_static")
            s = s[: m.start()] + "use i18n::{" + ", ".join(sorted(names)) + "};" + s[m.end():]
        elif "use i18n" not in s:
            mm = re.search(r"^use [^;]+;", s, re.M)
            if mm:
                s = s[: mm.start()] + "use i18n::{keys, t_static};\n" + s[mm.start():]
        open(p, "w", encoding="utf-8", newline="").write(s)


if __name__ == "__main__":
    main()
