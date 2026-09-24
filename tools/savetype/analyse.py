#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""在 scan.py 的明细上做归因：把「两个核心会判不一致」的游戏挑出来并分类。"""
import os, re, csv, collections

HERE = os.path.dirname(os.path.abspath(__file__))
rows = list(csv.DictReader(open(os.path.join(HERE, "scan-result.csv"), encoding="utf-8-sig")))

# 随包 gpSP 的签名表实测只有 4 个（无 EEPROM_V）——按它的口径重算
SHIP_SIGS = ["SRAM_V", "FLASH1M_V", "FLASH512_V", "FLASH_V"]
SIG_ORDER = ["EEPROM_V", "SRAM_V", "FLASH1M_V", "FLASH512_V", "FLASH_V"]


def normsig(r):
    return [s for s in SIG_ORDER if s in r["sig"].split("+")]


def gpsp_ship(sigs):
    for s in SHIP_SIGS:
        if s in sigs:
            return {"SRAM_V": "SRAM", "FLASH1M_V": "FLASH1M",
                    "FLASH512_V": "FLASH512", "FLASH_V": "FLASH512"}[s]
    return "默认(SRAM)"


CH = re.compile(r"\[(中|改|简|繁)\]|汉化|中文|简中|繁中")


def main():
    n = len(rows)
    print(f"库内 ROM: {n}\n")

    # 1. 按随包 gpSP 口径分类
    print("== 按「随包 gpSP」（签名表 4 个、无 EEPROM_V）分类 ==")
    c = collections.Counter(gpsp_ship(normsig(r)) for r in rows)
    for k, v in c.most_common():
        print(f"  {v:>4}  {k}")
    only_eeprom = [r for r in rows if normsig(r) == ["EEPROM_V"]]
    print(f"  └ 其中「签名只有 EEPROM_V」= {len(only_eeprom)} 款 "
          f"→ 随包 gpSP 无法靠签名定类型，只能命中游戏码表或落到默认")

    # 2. 多签名（两核心必然分歧）
    multi = [r for r in rows if len(normsig(r)) >= 2]
    print(f"\n== 签名自相矛盾（≥2 个签名串共存）: {len(multi)} 款 ==")
    hdr = f"  {'文件名':<48} {'码':<5} {'签名':<34} gpSP→"
    print(hdr)
    for r in multi:
        s = normsig(r)
        print(f"  {r['name'][:46]:<48} {r['code']:<5} {'+'.join(s):<34} "
              f"{r['gpsp']:<10} / mGBA 运行时探测")
    print("  ⇒ gpSP 取 EEPROM 优先；mGBA 完全不看签名串，只按运行时总线行为。")
    print("     真身若是 SRAM/FLASH，两个核心就会给出不同尺寸的存档。")

    # 3. 完全无签名
    none = [r for r in rows if len(normsig(r)) == 0]
    print(f"\n== 一个签名串都没有（两个核心都只能靠猜）: {len(none)} 款 ==")
    print("  样例 25 个：")
    for r in none[:25]:
        print(f"    {r['name'][:56]:<58} {r['code']:<5} {int(r['size'])//1024:>5} KB")

    # 4. EEPROM 类整体
    eep = [r for r in rows if "EEPROM_V" in normsig(r)]
    print(f"\n== 含 EEPROM_V 的共 {len(eep)} 款（占 {len(eep)*100//n}%）==")
    print("  EEPROM 是 GBA 上最大的一类存档；两个核心判定依据差别也最大：")
    print("    gpSP：靠这一串签名（随包版本还没有这串）")
    print("    mGBA：完全靠「有没有 16-bit DMA 写到 0x0E000000」")

    # 5. 中文/改版 标记者
    ch = [r for r in rows if CH.search(r["name"])]
    chr_ = [r for r in ch if len(normsig(r)) == 0 or len(normsig(r)) >= 2]
    print(f"\n== 文件名带 [中]/[改]/汉化 的: {len(ch)} 款，其中落在『无签名/多签名』区: {len(chr_)} 款 ==")
    for r in chr_:
        print(f"    {r['name'][:56]:<58} {r['code']:<5} {r['sig'] or '-'}")

    # 6. 宝可梦路径
    pks = [r for r in rows if r["is_pokemon"] == "Y"]
    print(f"\n== 命中 mGBA「宝可梦改版」路径(isPokemon=Y)的: {len(pks)} 款 ==")
    for r in pks:
        print(f"    {r['name'][:50]:<52} {r['code']} 官方CRC={r['pk_known'] or '-'} "
              f"→ 强制 FLASH1M+RTC")


if __name__ == "__main__":
    main()
