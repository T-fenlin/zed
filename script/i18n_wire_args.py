#!/usr/bin/env python3
"""把 `format!("前缀 {} 后缀", expr)` 重构为 t_with_args(key, &[("var", &expr.to_string())])。

只处理：format! 恰好一个参数、格式串恰好一个占位符（{}/{:...}?/具名）、
且能按 en-US 值精确匹配到唯一 key 的情况。
"""

import glob
import os
import re
import sys

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
PH = re.compile(r"\{\s*\$([A-Za-z_][A-Za-z0-9_]*)\s*\}")


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


def scan_format_calls(text):
    """返回 [(start, end, fmt_string, arg_expr)]，仅单参数且单占位符。"""
    out = []
    for m in re.finditer(r"format!\(", text):
        i = m.end()
        # 跳过空白，读取字符串字面量
        while i < len(text) and text[i] in " \t\n":
            i += 1
        if i >= len(text) or text[i] != '"':
            continue
        j = i + 1
        while j < len(text):
            if text[j] == "\\":
                j += 2
                continue
            if text[j] == '"':
                break
            j += 1
        if j >= len(text):
            continue
        raw = text[i + 1 : j]
        k = j + 1
        while k < len(text) and text[k] in " \t\n":
            k += 1
        if k >= len(text) or text[k] != ",":
            continue
        # 解析参数直到 format! 的右括号
        depth, p = 0, k + 1
        while p < len(text):
            c = text[p]
            if c in "([{":
                depth += 1
            elif c in ")]}":
                if depth == 0:
                    break
                depth -= 1
            elif c == '"':
                p += 1
                while p < len(text) and text[p] != '"':
                    p += 2 if text[p] == "\\" else 1
            p += 1
        args = text[k + 1 : p].strip()
        out.append((m.start(), p + 1, raw, args))
    return out


def split_single_placeholder(fmt):
    """返回 (before, var, after) 若恰好一个占位符且无格式说明符，否则 None。"""
    holders = list(re.finditer(r"\{([^}]*)\}", fmt))
    if len(holders) != 1:
        return None
    h = holders[0]
    inner = h.group(1).strip()
    if inner.startswith(":") or ":" in inner:
        return None
    return fmt[: h.start()], inner, fmt[h.end() :]


def main():
    apply_flag = "--apply" in sys.argv
    only = sys.argv[sys.argv.index("--domain") + 1] if "--domain" in sys.argv else None
    domains = {os.path.splitext(os.path.basename(p))[0]: parse_ftl(p)
               for p in glob.glob(os.path.join(EN_DIR, "*.ftl"))}
    targets = [only] if only else list(domains)

    # (before, after) -> [(domain, key, var)]
    sig = {}
    for d in targets:
        for k, v in domains[d].items():
            holders = list(PH.finditer(v))
            if len(holders) != 1:
                continue
            h = holders[0]
            before, after = v[: h.start()], v[h.end():]
            sig.setdefault((before, after), []).append((d, k, h.group(1)))

    total, modified, samples = 0, set(), []
    for p in glob.glob(os.path.join(ROOT, "crates", "**", "*.rs"), recursive=True):
        norm = p.replace("\\", "/")
        if "/target/" in norm or "/fixtures/" in norm or "/evals/" in norm:
            continue
        try:
            text = open(p, encoding="utf-8").read()
        except (OSError, UnicodeDecodeError):
            continue
        if "format!(" not in text:
            continue
        outs, last, changed = [], 0, False
        for start, end, fmt, args in scan_format_calls(text):
            if start < last:
                continue
            sp = split_single_placeholder(fmt)
            if not sp:
                continue
            before, _, after = sp
            cands = sig.get((before, after))
            if not cands:
                continue
            owners = [c for c in cands if any(o in norm for o in OWNER.get(c[0], []))]
            pick = owners[0] if len(owners) == 1 else (cands[0] if len(cands) == 1 else None)
            if not pick:
                continue
            d, key, var = pick
            ns = d.replace("-", "_")
            if "," in args and args.count(",") > 0:
                # 单参数里可能有闭包/泛型逗号，保守跳过
                if not re.fullmatch(r"[^,]+", args):
                    continue
            replacement = (
                f't_with_args(keys::{ns}::{const_name(key)}, &[("{var}", &({args}).to_string())])'
            )
            outs.append(text[last:start])
            outs.append(replacement)
            last = end
            total += 1
            changed = True
            if len(samples) < 20:
                samples.append(f"{os.path.relpath(p, ROOT)}  {text[start:end][:90]}")
        if changed:
            outs.append(text[last:])
            new = "".join(outs)
            modified.add(p)
            if apply_flag:
                open(p, "w", encoding="utf-8", newline="").write(new)

    print(f"{'' if apply_flag else '[dry-run] '}重构处数: {total}, 文件: {len(modified)}")
    for s in samples:
        print("  ", s.replace("\n", " "))

    if not apply_flag:
        return
    for p in sorted(modified):
        s = open(p, encoding="utf-8").read()
        m = re.search(r"use i18n::\{([^}]*)\};", s, re.S)
        if m:
            names = {n.strip() for n in m.group(1).replace("\n", " ").split(",") if n.strip()}
            names.update({"keys", "t_with_args"})
            s = s[: m.start()] + "use i18n::{" + ", ".join(sorted(names)) + "};" + s[m.end():]
        elif "use i18n" not in s:
            mm = re.search(r"^use [^;]+;", s, re.M)
            if mm:
                s = s[: mm.start()] + "use i18n::{keys, t_with_args};\n" + s[mm.start():]
        open(p, "w", encoding="utf-8", newline="").write(s)


if __name__ == "__main__":
    main()
