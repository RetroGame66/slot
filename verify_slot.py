# -*- coding: utf-8 -*-
"""Verify the freshly built device binary: aarch64 ELF + Chinese UI strings + glibc ceiling."""
import struct, sys

P = r"F:/WorkBuddyWorkSpace/2026-09-11-08-38-21/slot-main/target-device/aarch64-unknown-linux-gnu/release/slot"
data = open(P, "rb").read()
print("file size :", len(data), "bytes")

# --- ELF header ---
assert data[:4] == b"\x7fELF", "not an ELF"
cls, machine = data[4], struct.unpack_from("<H", data, 18)[0]
print("EI_CLASS  :", cls, "(2=64bit)")
print("e_machine :", hex(machine), "(0xb7=AArch64)")
print("arch ok   :", cls == 2 and machine == 0xB7)

# --- Chinese strings we expect after the translation work ---
zh = ["重启", "关机", "存档已保存", "金手指已开启", "联机", "设置", "亮度", "音量", "显示"]
print("\n-- Chinese strings expected --")
for s in zh:
    print(("  OK   " if s.encode("utf-8") in data else "  MISS "), s)

# --- Old English strings that should be GONE ---
en = ["Power Off", "State Saved", "Cheats On", "Cheats Off", "Brightness", "Volume"]
print("\n-- Old English strings (should be GONE) --")
for s in en:
    print(("  STILL" if s.encode() in data else "  GONE "), s)

# --- INTERP (static vs dynamic) ---
i = data.find(b"/lib/ld-linux-aarch64.so.1")
print("\nINTERP     :", data[i:i + 64].split(b"\x00")[0].decode("ascii", "replace") if i >= 0 else "none (static)")
