#!/usr/bin/env python3
"""不依赖 readelf 的 ELF 体检：magic / class / machine / type / interp / GLIBC 上限。"""
import re
import struct
import sys

ET = {0: "NONE", 1: "REL", 2: "EXEC", 3: "DYN", 4: "CORE"}
EM = {0xB7: "AArch64", 0x3E: "x86-64", 0x28: "ARM"}


def cstr(buf, off):
    end = buf.find(b"\x00", off)
    return buf[off:end].decode("utf-8", "replace")


def main(path):
    with open(path, "rb") as fh:
        buf = fh.read()
    print(f"file    : {path}")
    print(f"size    : {len(buf)}")

    magic = buf[:4]
    print(f"magic   : {magic.hex()}  {'OK' if magic == b'\\x7fELF' else 'NOT ELF'}")
    if magic != b"\x7fELF":
        return 1
    is64 = buf[4] == 2
    print(f"class   : {'ELF64' if is64 else 'ELF32'}")
    print(f"endian  : {'little' if buf[5] == 1 else 'big'}")
    e_type, e_machine = struct.unpack_from("<HH", buf, 16)
    print(f"type    : {e_type} ({ET.get(e_type, '?')})")
    print(f"machine : {e_machine:#06x} ({EM.get(e_machine, '?')})")

    if not is64:
        return 0
    e_phoff, = struct.unpack_from("<Q", buf, 32)
    e_phentsize, e_phnum = struct.unpack_from("<HH", buf, 54)
    interp = None
    for i in range(e_phnum):
        off = e_phoff + i * e_phentsize
        p_type, = struct.unpack_from("<I", buf, off)
        if p_type == 3:  # PT_INTERP
            p_offset, = struct.unpack_from("<Q", buf, off + 8)
            interp = cstr(buf, p_offset)
    print(f"interp  : {interp}")

    # 动态段里的版本需求（GLIBC_2.x）
    versions = sorted(
        {m.group(0) for m in re.finditer(rb"GLIBC_(\d+)\.(\d+)", buf)},
        key=lambda b: tuple(int(x) for x in b.decode()[6:].split(".")),
    )
    if versions:
        print(f"glibc   : max {versions[-1].decode()}, 共 {len(versions)} 个版本符号")
    else:
        print("glibc   : 无（可能静态链接）")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1]))
