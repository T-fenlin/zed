#!/usr/bin/env python3
"""i18n 还原审计：ftl ↔ keys.rs ↔ 代码调用点 三层闭环校验。

用途：从编译产物(exe)抢救回 zh-CN ftl 后，校验每条翻译是否已接回源码。

检查项：
  1. 每个 en-US key 在 zh-CN 中存在且值已翻译（非纯英文原样）
  2. 每个 ftl key 在 crates/i18n/src/keys.rs 中有常量（反向：无常量的 ftl key）
  3. 每个常量在 crates/ 中至少有 1 处 `keys::<domain>::<CONST>` 引用
     —— 引用为 0 = 翻译已还原但代码未接线（还原遗漏）

用法：python script/i18n_audit.py [--verbose]
"""

import re
import sys
import glob
import os
from collections import defaultdict

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
EN_DIR = os.path.join(ROOT, "assets", "locales", "en-US")
ZH_DIR = os.path.join(ROOT, "assets", "locales", "zh-CN")
KEYS_RS = os.path.join(ROOT, "crates", "i18n", "src", "keys.rs")


def parse_ftl_keys(path):
    """返回 {key: value}，多行值合并为一行（用于比对）。"""
    result = {}
    key = None
    val = []
    for line in open(path, encoding="utf-8"):
        s = line.rstrip("\n")
        m = re.match(r"^([a-z0-9][a-z0-9_-]*)\s*=\s?(.*)$", s)
        if m:
            if key:
                result[key] = " ".join(v.strip() for v in val if v.strip())
            key, val = m.group(1), [m.group(2)]
        elif key is not None and s.startswith("    "):
            val.append(s)
        elif key is not None and s == "":
            val.append("")
        elif key is not None:
            result[key] = " ".join(v.strip() for v in val if v.strip())
            key, val = None, []
    if key:
        result[key] = " ".join(v.strip() for v in val if v.strip())
    return result


def parse_keys_rs():
    """解析 define_keys! 宏，返回 {domain: {const_name: ftl_key}}。"""
    text = open(KEYS_RS, encoding="utf-8").read()
    domains = {}
    for mod in re.finditer(r"pub mod (\w+) \{.*?define_keys! \{(.*?)\}\s*\}", text, re.S):
        domain, body = mod.group(1), mod.group(2)
        entries = dict(re.findall(r"([A-Z0-9_]+) = \"([a-z0-9][a-z0-9_-]*)\"", body))
        domains[domain] = entries
    return domains


def collect_call_sites():
    """扫描 crates/**/*.rs，返回 {(domain, const): count}。"""
    usage = defaultdict(int)
    pat = re.compile(r"keys::([a-z_]+)::([A-Z0-9_]+)")
    for path in glob.glob(os.path.join(ROOT, "crates", "**", "*.rs"), recursive=True):
        try:
            content = open(path, encoding="utf-8").read()
        except (OSError, UnicodeDecodeError):
            continue
        for m in pat.finditer(content):
            usage[(m.group(1), m.group(2))] += 1
    return usage


def main():
    verbose = "--verbose" in sys.argv
    problems = 0

    keys_rs = parse_keys_rs()
    usage = collect_call_sites()

    # 域名 -> ftl 文件名（app_menu -> app-menu）
    def ftl_name(domain):
        return domain.replace("_", "-")

    total_keys = 0
    total_unwired = 0

    print("=" * 72)
    print(f"{'域':<16}{'ftl(en/zh)':<12}{'常量':<6}{'已接线':<8}{'未接线':<8}状态")
    print("=" * 72)

    unwired_by_domain = {}
    for domain in sorted(keys_rs):
        entries = keys_rs[domain]
        en_path = os.path.join(EN_DIR, ftl_name(domain) + ".ftl")
        zh_path = os.path.join(ZH_DIR, ftl_name(domain) + ".ftl")
        if not os.path.exists(en_path):
            print(f"{ftl_name(domain):<16}缺少 en-US ftl！")
            problems += 1
            continue
        en = parse_ftl_keys(en_path)
        zh = parse_ftl_keys(zh_path) if os.path.exists(zh_path) else {}

        missing_zh = [k for k in en if k not in zh]
        untranslated = [
            k
            for k in en
            if k in zh
            and zh[k] == en[k]
            and re.search(r"[a-zA-Z]{3}", zh[k])
        ]
        no_const = [k for k in en if k not in set(entries.values())]
        stale_const = [v for v in entries.values() if v not in en]

        unwired = []
        for const, key in entries.items():
            if usage.get((domain, const), 0) == 0:
                unwired.append(key)

        total_keys += len(entries)
        total_unwired += len(unwired)
        unwired_by_domain[domain] = unwired

        status = "OK"
        if missing_zh or untranslated or no_const or stale_const or unwired:
            status = "有问题"
            problems += 1
        print(
            f"{ftl_name(domain):<16}{len(en)}/{len(zh):<9}{len(entries):<6}"
            f"{len(entries) - len(unwired):<8}{len(unwired):<8}{status}"
        )
        if missing_zh:
            print(f"    [zh缺失 {len(missing_zh)}] {', '.join(sorted(missing_zh)[:10])}")
        if untranslated:
            print(f"    [未翻译 {len(untranslated)}] {', '.join(sorted(untranslated)[:10])}")
        if no_const:
            print(f"    [无常量 {len(no_const)}] {', '.join(sorted(no_const)[:10])}")
        if stale_const:
            print(f"    [陈旧常量 {len(stale_const)}] {', '.join(sorted(stale_const)[:10])}")

    print("=" * 72)
    print(f"合计：{total_keys} 个 key，{total_unwired} 个未接线（翻译在 ftl 中但代码未引用）")

    if verbose:
        print("\n—— 未接线 key 明细 ——")
        for domain, keys in unwired_by_domain.items():
            if keys:
                print(f"\n[{ftl_name(domain)}] {len(keys)} 个：")
                for k in sorted(keys):
                    print(f"  {k}")

    return 1 if problems else 0


if __name__ == "__main__":
    sys.exit(main())
