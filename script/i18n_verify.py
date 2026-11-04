#!/usr/bin/env python3
"""i18n zh-CN 逐模块校验器：翻译正确性 + 结构完整性。

对 assets/locales/{en-US,zh-CN} 下每个域文件逐条校验：
  1. key 双向一致（en 有 / zh 有）
  2. 重复 key
  3. 占位符变量一致（en 的 { $x } 集合 == zh 的 { $x } 集合）—— 错译/错位的第一征兆
  4. 未翻译（zh 值与 en 完全相同且含英文单词）
  5. 空值 / 残留字面量 \\n（Fluent 普通文本不处理转义）
  6. 中文值里混入英文单词的比例（人工复核提示，非硬错误）

用法：python script/i18n_verify.py [--domain acp] [--quiet]
退出码：有硬错误时非 0。
"""

import os
import re
import sys
from collections import Counter

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
EN_DIR = os.path.join(ROOT, "assets", "locales", "en-US")
ZH_DIR = os.path.join(ROOT, "assets", "locales", "zh-CN")

KEY_RE = re.compile(r"^([a-z0-9][a-z0-9_-]*)\s*=\s?(.*)$")
PLACEHOLDER_RE = re.compile(r"\{\s*\$([a-zA-Z_][a-zA-Z0-9_]*)")
CJK_RE = re.compile(r"[\u4e00-\u9fff]")
WORD_RE = re.compile(r"[A-Za-z]{2,}")


def parse_ftl(path):
    """返回 [(key, value, lineno)]，保留重复 key 以检测。"""
    entries = []
    key, val, start = None, [], 0
    for lineno, raw in enumerate(open(path, encoding="utf-8"), 1):
        s = raw.rstrip("\n")
        m = KEY_RE.match(s)
        if m:
            if key:
                entries.append((key, " ".join(v.strip() for v in val if v.strip()), start))
            key, val, start = m.group(1), [m.group(2)], lineno
        elif key is not None and s.startswith("    "):
            val.append(s)
        elif key is not None and s.strip() == "":
            val.append("")
        elif key is not None:
            entries.append((key, " ".join(v.strip() for v in val if v.strip()), start))
            key, val = None, []
    if key:
        entries.append((key, " ".join(v.strip() for v in val if v.strip()), start))
    return entries


def check_domain(name):
    en = parse_ftl(os.path.join(EN_DIR, name + ".ftl"))
    zh_path = os.path.join(ZH_DIR, name + ".ftl")
    if not os.path.exists(zh_path):
        return {"status": "FAIL", "errors": ["缺少 zh-CN 文件"], "warnings": [], "count": 0}
    zh = parse_ftl(zh_path)

    en_map, zh_map = {}, {}
    errors, warnings = [], []
    for k, v, _ in en:
        en_map.setdefault(k, v)
    dup_zh = [k for k, c in Counter(k for k, _, _ in zh).items() if c > 1]
    if dup_zh:
        errors.append(f"zh 重复 key: {', '.join(sorted(dup_zh)[:8])}")
    for k, v, _ in zh:
        zh_map.setdefault(k, v)

    missing = sorted(set(en_map) - set(zh_map))
    extra = sorted(set(zh_map) - set(en_map))
    if missing:
        errors.append(f"zh 缺 key {len(missing)}: {', '.join(missing[:8])}")
    if extra:
        errors.append(f"zh 多余 key {len(extra)}: {', '.join(extra[:8])}")

    untranslated, empty, bad_escape, placeholder_mismatch, mixed = [], [], [], [], []
    for k, en_val in en_map.items():
        if k not in zh_map:
            continue
        zh_val = zh_map[k]
        if not zh_val.strip():
            empty.append(k)
            continue
        if zh_val == en_val and WORD_RE.search(zh_val):
            untranslated.append(k)
        if "\\n" in zh_val:
            bad_escape.append(k)
        en_ph = set(PLACEHOLDER_RE.findall(en_val))
        zh_ph = set(PLACEHOLDER_RE.findall(zh_val))
        if en_ph != zh_ph:
            placeholder_mismatch.append(f"{k}(en:{sorted(en_ph)} zh:{sorted(zh_ph)})")
        if CJK_RE.search(zh_val):
            # 中文里夹的英文单词数（排除变量名后的值）
            words = set(WORD_RE.findall(PLACEHOLDER_RE.sub("", zh_val)))
            if len(words) >= 4:
                mixed.append(f"{k}:{sorted(words)[:5]}")

    if untranslated:
        errors.append(f"未翻译 {len(untranslated)}: {', '.join(untranslated[:8])}")
    if empty:
        errors.append(f"空值 {len(empty)}: {', '.join(empty[:8])}")
    if bad_escape:
        errors.append(f"字面量\\\\n {len(bad_escape)}: {', '.join(bad_escape[:8])}")
    if placeholder_mismatch:
        errors.append(f"占位符不一致 {len(placeholder_mismatch)}: {'; '.join(placeholder_mismatch[:5])}")
    if mixed:
        warnings.append(f"中英混排 {len(mixed)}: {'; '.join(mixed[:3])}")

    return {
        "status": "FAIL" if errors else ("WARN" if warnings else "PASS"),
        "errors": errors,
        "warnings": warnings,
        "count": len(en_map),
    }


def main():
    only = None
    if "--domain" in sys.argv:
        only = sys.argv[sys.argv.index("--domain") + 1]
    quiet = "--quiet" in sys.argv

    names = sorted(
        os.path.splitext(f)[0] for f in os.listdir(EN_DIR) if f.endswith(".ftl")
    )
    if only:
        names = [n for n in names if n == only]

    print(f"{'模块':<18}{'key 数':<8}{'结果'}")
    print("-" * 60)
    fails = warns = total = 0
    for name in names:
        r = check_domain(name)
        total += r["count"]
        if r["status"] == "FAIL":
            fails += 1
        elif r["status"] == "WARN":
            warns += 1
        print(f"{name:<18}{r['count']:<8}{r['status']}")
        if r["status"] != "PASS":
            for e in r["errors"]:
                print(f"    ✗ {e}")
            if not quiet:
                for w in r["warnings"]:
                    print(f"    ! {w}")

    print("-" * 60)
    print(f"{len(names)} 个模块 / {total} 条文案：PASS {len(names) - fails - warns}，"
          f"WARN {warns}，FAIL {fails}")
    return 1 if fails else 0


if __name__ == "__main__":
    sys.exit(main())
