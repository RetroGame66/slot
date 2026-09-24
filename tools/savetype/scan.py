#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""扫 GBA 库：按两个核心各自的规则预判存档类型，找出会「核心之间判不一致」的游戏。

规则来源（均为实测，附出处）：
  gpSP   —— ROM 全文件扫签名串 EEPROM_V / SRAM_V / FLASH1M_V / FLASH512_V / FLASH_V，
            优先级 EEPROM > SRAM > FLASH1M > FLASH512；全会落空则退化为 SRAM。
            （libretro/gpsp master: gba_memory.c detect_backup_subcircuit / read_backup）
            注意：随包那份 gpSP 二进制里没有 "EEPROM_V"，见报告。
  mGBA   —— 不扫任何签名串。按游戏码查内建 override 表；宝可梦改版按 ROM CRC32 比对
            官方表，非官方 CRC 强制 FLASH1M+RTC；其余走运行时探测
            （DMA16 写 0x0E -> EEPROM；CPU 写 0x0E005555 -> FLASH512；CPU 读 -> SRAM）。
            （mgba-emu/mgba master: src/gba/overrides.c / savedata.c / dma.c / memory.c）
"""
import os, re, csv, zlib, sys, collections

MGBA_SRC = r"C:\Users\C.R.A.Z.Y\AppData\Local\Temp\mgba\overrides.c"  # 下面会重定位
ROOT = r"G:\[掌机][GBA][202608]\SLOT前端游戏包(全整合游戏)\Games"
OUT = os.path.dirname(os.path.abspath(__file__))

SIGS = [b"EEPROM_V", b"SRAM_V", b"FLASH1M_V", b"FLASH512_V", b"FLASH_V"]


def parse_overrides(path):
    s = open(path, encoding="utf-8", errors="replace").read()
    tbl = s[s.index("_overrides[] = {"): s.index("bool GBAOverrideFindConfig")]
    codes = {}
    for m in re.finditer(r'\{\s*"([A-Z0-9]{4})"\s*,\s*(GBA_SAVEDATA_[A-Z0-9]+)', tbl):
        codes[m.group(1)] = m.group(2).replace("GBA_SAVEDATA_", "")
    pk = s[s.index("pokemonTable[] = {"):]
    pk = pk[: pk.index("};")]
    crcs = {}
    for m in re.finditer(r"(0x[0-9A-F]{8}),\s*//\s*(\S+)", pk):
        crcs[int(m.group(1), 16)] = m.group(2)
    return codes, crcs


def read_head(p):
    with open(p, "rb") as f:
        b = f.read(0xB0)
    title = b[0xA0:0xAC].split(b"\x00")[0]
    code = b[0xAC:0xB0]
    try:
        t = title.decode("ascii")
    except UnicodeDecodeError:
        t = title.decode("latin-1")
    try:
        c = code.decode("ascii")
    except UnicodeDecodeError:
        c = code.decode("latin-1")
    return t, c


def sigs_of(p):
    b = open(p, "rb").read()
    return {s.decode(): (s in b) for s in SIGS}, zlib.crc32(b) & 0xFFFFFFFF, len(b)


def gpsp_type(sig):
    if sig["EEPROM_V"]:
        return "EEPROM"
    if sig["SRAM_V"]:
        return "SRAM"
    if sig["FLASH1M_V"]:
        return "FLASH1M"
    if sig["FLASH512_V"] or sig["FLASH_V"]:
        return "FLASH512"
    return "(落空→SRAM)"


def mgba_type(code, crc, sig):
    # 宝可梦改版路径：isPokemon 用 0x108 / 0xAC，这个在 scan 里单独算
    pass


def main():
    codes, crcs = parse_overrides(sys.argv[1])
    rows = []
    n = 0
    for name in sorted(os.listdir(ROOT)):
        if not name.lower().endswith(".gba"):
            continue
        p = os.path.join(ROOT, name)
        sig, crc, size = sigs_of(p)
        title, code = read_head(p)
        # 宝可梦改版判定（mGBA 的 isPokemon）
        with open(p, "rb") as f:
            f.seek(0x108)
            at108 = f.read(24)
        is_pk = (at108.startswith(b"pokemon red version") or
                 at108.startswith(b"pokemon emerald version") or
                 code == "AXVE")
        known = crc in crcs
        n += 1
        rows.append(dict(
            name=name, size=size, title=title, code=code,
            sig="+".join(k for k in ("EEPROM_V", "SRAM_V", "FLASH1M_V",
                                     "FLASH512_V", "FLASH_V") if sig[k]) or "-",
            nsig=sum(1 for k in sig if sig[k]),
            crc=f"{crc:08X}",
            gpsp=gpsp_type(sig),
            in_override=codes.get(code, ""),
            is_pokemon="Y" if is_pk else "",
            pk_known=(crcs.get(crc, "") if is_pk else ""),
        ))
    print(f"扫描 {n} 个 ROM")

    # 摘要
    sigc = collections.Counter(r["sig"] for r in rows)
    print("\n== 签名组合分布 ==")
    for k, v in sigc.most_common():
        print(f"  {v:>4}  {k}")

    print("\n== 会被 gpSP 判成 X 但 X 可疑的（多签名 / 落空）==")
    risk = [r for r in rows if r["nsig"] >= 2 or r["nsig"] == 0]
    print(f"  合计 {len(risk)} / {len(rows)}")
    c2 = collections.Counter(r["gpsp"] for r in risk)
    for k, v in c2.most_common():
        print(f"    {v:>4}  gpSP 判为 {k}")

    print("\n== 宝可梦相关（mGBA 会走改版路径或 override 表）==")
    pk = [r for r in rows if r["is_pokemon"] or r["code"].startswith(("AXV", "AXP", "BPE", "BPR", "BPG"))]
    for r in pk[:40]:
        print(f"  {r['code']}  {r['name'][:46]:<46} sig={r['sig']:<28} gpsp={r['gpsp']:<11} "
              f"pk={'Y' if r['is_pokemon'] else '-'} 官方CRC={r['pk_known'] or '-'}")

    with open(os.path.join(OUT, "scan-result.csv"), "w", newline="", encoding="utf-8-sig") as f:
        w = csv.DictWriter(f, fieldnames=list(rows[0].keys()))
        w.writeheader()
        w.writerows(rows)
    print(f"\n明细 -> {os.path.join(OUT, 'scan-result.csv')}")


if __name__ == "__main__":
    main()
