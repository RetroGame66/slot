#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""跨版本配对：同一款游戏（游戏码前 3 位相同）的不同 ROM，
若有的带签名串、有的不带 —— 那就是「汉化/改版把签名弄丢了」的直接证据。

依据：GBA ROM 的存档类型库签名串（EEPROM_V/SRAM_V/FLASH512_V/FLASH1M_V）是编译进
游戏二进制里的字面量。同款游戏各区域版本（同一 3 位游戏码）的存档类型一致，
所以签名串应当也存在或不存在的规律一致；出现分歧，只能是有人在改 ROM。
"""
import os, csv, collections, re

HERE = os.path.dirname(os.path.abspath(__file__))
rows = [r for r in csv.DictReader(open(os.path.join(HERE, "scan-result.csv"),
                                      encoding="utf-8-sig"))
        if len(r["code"]) == 4 and r["code"].isalnum()]
CH = re.compile(r"\[(中|改)\]|汉化")

groups = collections.defaultdict(list)
for r in rows:
    groups[r["code"][:3]].append(r)

mixed = []
for k, g in groups.items():
    had = [r for r in g if r["sig"] != "-"]
    not_ = [r for r in g if r["sig"] == "-"]
    if had and not_:
        mixed.append((k, had, not_))

mixed.sort()
print(f"=== 同款游戏里「有签名 / 无签名」并存的分组：{len(mixed)} 组 ===\n")
for k, had, not_ in mixed:
    print(f"[{k}*]")
    for r in had:
        print(f"   有签名  {r['name'][:52]:<54} {r['code']}  {r['sig']}")
    for r in not_:
        print(f"   无签名  {r['name'][:52]:<54} {r['code']}  (落空→默认)")
    print()

# 反向：同款全都有 / 全都无
allnone = [(k, g) for k, g in groups.items() if g and all(r["sig"] == "-" for r in g)]
print(f"=== 同款全部无签名（无法用此判据，需另查）: {len(allnone)} 组 ===")

print(f"\n=== 重点对照：恶魔城 ===")
for k, g in groups.items():
    if any("恶魔城" in r["name"] for r in g):
        for r in g:
            print(f"   {r['name'][:50]:<52} {r['code']}  sig={r['sig'] or '-'}")
