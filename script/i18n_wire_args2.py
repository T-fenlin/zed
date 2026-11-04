#!/usr/bin/env python3
"""把多占位符 `format!("... {} ... {}", a, b)` 重构为 t_with_args。

匹配规则：按 en-US 值的静态片段序列，与 format! 格式串的静态片段序列完全一致
（含转义还原），占位符个数相同，参数个数与占位符个数相同。
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
    "outline": ["outline_panel"], "recent": ["recent_projects"],
    "pane": ["crates/workspace/"], "tab": ["crates/editor/"], "lang": ["crates/zed/"],
    "ui": ["crates/ui/"], "zed": ["crates/zed/"],
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


def unescape(s):
    return (s.replace("\\n", "\n").replace("\\t", "\t").replace('\\"', '"')
             .replace("\\\\", "\\"))


def parse_format_calls(text):
    """yield (start, end, fmt_raw, args_raw)。"""
    for m in re.finditer(r"format!\(", text):
        i = m.end()
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
        raw = text[i + 1 : j]
        k = j + 1
        while k < len(text) and text[k] in " \t\n":
            k += 1
        if k >= len(text) or text[k] != ",":
            continue
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
        yield m.start(), p + 1, raw, text[k + 1 : p].strip()


def split_top_level(s):
    parts, depth, cur, q = [], 0, [], False
    i = 0
    while i < len(s):
        c = s[i]
        if q:
            cur.append(c)
            if c == "\\":
                if i + 1 < len(s):
                    cur.append(s[i + 1]); i += 2; continue
            elif c == '"':
                q = False
            i += 1
            continue
        if c == '"':
            q = True; cur.append(c)
        elif c in "([{<":
            depth += 1; cur.append(c)
        elif c in ")]}>":
            depth -= 1; cur.append(c)
        elif c == "," and depth == 0:
            parts.append("".join(cur).strip()); cur = []
        else:
            cur.append(c)
        i += 1
    if cur:
        parts.append("".join(cur).strip())
    return [p for p in parts if p]


def fmt_pieces(fmt):
    """返回 (静态片段列表, 占位符数量)；含格式说明符则返回 (None, 0)。"""
    pieces, holders, last = [], [], 0
    for m in re.finditer(r"\{([^}]*)\}", fmt):
        inner = m.group(1).strip()
        if ":" in inner:
            return None, 0
        if inner.startswith("{"):
            return None, 0
        holders.append(inner)
        pieces.append(fmt[last : m.start()])
        last = m.end()
    pieces.append(fmt[last:])
    return pieces, len(holders)


def val_pieces(v):
    pieces, vars_ = [], []
    last = 0
    for m in PH.finditer(v):
        pieces.append(v[last : m.start()])
        vars_.append(m.group(1))
        last = m.end()
    pieces.append(v[last:])
    return pieces, vars_


def main():
    apply_flag = "--apply" in sys.argv
    domains = {os.path.splitext(os.path.basename(p))[0]: parse_ftl(p)
               for p in glob.glob(os.path.join(EN_DIR, "*.ftl"))}
    index = {}
    for d, kv in domains.items():
        for k, v in kv.items():
            if "{" not in v:
                continue
            pieces, vars_ = val_pieces(v)
            if len(vars_) >= 1 and all("{" not in p for p in pieces):
                index.setdefault(tuple(pieces), []).append((d, k, vars_))

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
        for start, end, raw, args in parse_format_calls(text):
            if start < last:
                continue
            pieces, n = fmt_pieces(raw)
            if not pieces or n == 0:
                continue
            norm_pieces = tuple(unescape(x) for x in pieces)
            cands = index.get(norm_pieces)
            if not cands:
                continue
            arg_list = split_top_level(args)
            owners = [c for c in cands if any(o in norm for o in OWNER.get(c[0], []))]
            pick = owners[0] if len(owners) == 1 else (cands[0] if len(cands) == 1 else None)
            if not pick:
                continue
            d, key, vars_ = pick
            if len(arg_list) != len(vars_):
                continue
            ns = d.replace("-", "_")
            args_src = ", ".join(f'("{v}", &({a}).to_string())' for v, a in zip(vars_, arg_list))
            outs.append(text[last:start])
            outs.append(f"t_with_args(keys::{ns}::{const_name(key)}, &[{args_src}])")
            last = end
            total += 1
            changed = True
            if len(samples) < 20:
                samples.append(f"{os.path.relpath(p, ROOT)}  {raw[:70]}")
        if changed:
            outs.append(text[last:])
            modified.add(p)
            if apply_flag:
                open(p, "w", encoding="utf-8", newline="").write("".join(outs))

    print(f"{'' if apply_flag else '[dry-run] '}重构处数: {total}, 文件: {len(modified)}")
    for s in samples:
        print("  ", s)

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
