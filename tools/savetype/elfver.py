#!/usr/bin/env python3
"""精确读 ELF 的「版本需求」表（.gnu.version_r / Verneed），而不是全文搜字符串。

全文搜 GLIBC_x.y 会连调试/只读数据里的字面量一起收进来，报出的上限偏高。
真正决定能不能在被测机器上加载的，是 .gnu.version_r 里列出的那些 verneed 项。
"""
import struct
import sys


def sections(buf):
    e_shoff, = struct.unpack_from("<Q", buf, 40)
    e_shentsize, e_shnum, e_shstrndx = struct.unpack_from("<HHH", buf, 58)
    out = []
    for i in range(e_shnum):
        off = e_shoff + i * e_shentsize
        name, stype = struct.unpack_from("<II", buf, off)
        # Elf64_Shdr: flags, addr, offset, size are four consecutive u64 at off+8
        flags, addr, offset, size = struct.unpack_from("<QQQQ", buf, off + 8)
        out.append(dict(nameoff=name, type=stype, offset=offset, size=size, idx=i))
    shstr = next(s for s in out if s["idx"] == e_shstrndx)
    base = shstr["offset"]
    for s in out:
        end = buf.find(b"\x00", base + s["nameoff"])
        s["name"] = buf[base + s["nameoff"]:end].decode("utf-8", "replace")
    return out


def cstr(buf, off):
    end = buf.find(b"\x00", off)
    return buf[off:end].decode("utf-8", "replace")


def verneed(buf, secs, dynstr_off):
    """返回 [(version_string, need_lib)]"""
    sec = next((s for s in secs if s["name"] == ".gnu.version_r"), None)
    if sec is None:
        return []
    p = sec["offset"]
    vn_version, vn_cnt, vn_file, vn_aux, vn_next = struct.unpack_from("<HHIII", buf, p)
    out = []
    seen = set()
    while p and p not in seen:
        seen.add(p)
        vn_version, vn_cnt, vn_file, vn_aux, vn_next = struct.unpack_from("<HHIII", buf, p)
        lib = cstr(buf, dynstr_off + vn_file)
        a = p + vn_aux
        for _ in range(vn_cnt):
            vna_hash, vna_flags, vna_other, vna_name, vna_next = struct.unpack_from("<IHHII", buf, a)
            out.append((cstr(buf, dynstr_off + vna_name), lib))
            if not vna_next:
                break
            a += vna_next
        p = p + vn_next if vn_next else 0
    return out


def key(v):
    try:
        return tuple(int(x) for x in v.split("_")[1].split("."))
    except Exception:
        return (0,)


def main(path):
    with open(path, "rb") as fh:
        buf = fh.read()
    print(f"file    : {path}")
    print(f"size    : {len(buf)}")
    print(f"magic   : {buf[:4].hex()}  {'OK' if buf[:4] == bytes([0x7f]) + b'ELF' else 'NOT ELF'}")
    e_type, e_machine = struct.unpack_from("<HH", buf, 16)
    print(f"type    : {e_type} / machine {e_machine:#06x}")

    e_phoff, = struct.unpack_from("<Q", buf, 32)
    e_phentsize, e_phnum = struct.unpack_from("<HH", buf, 54)
    for i in range(e_phnum):
        off = e_phoff + i * e_phentsize
        p_type, = struct.unpack_from("<I", buf, off)
        if p_type == 3:
            p_offset, = struct.unpack_from("<Q", buf, off + 8)
            print(f"interp  : {cstr(buf, p_offset)}")

    secs = sections(buf)
    ds = next((s for s in secs if s["name"] == ".dynstr"), None)
    if ds is None:
        print("dynstr  : 无（静态链接？）")
        return 0
    reqs = verneed(buf, secs, ds["offset"])
    if not reqs:
        print("version : 无 verneed 项")
        return 0
    glibc = sorted({v for v, _ in reqs if v.startswith("GLIBC_")}, key=key)
    print(f"glibc   : max {glibc[-1] if glibc else '-'}  (共 {len(glibc)} 个: {', '.join(glibc)})")
    others = sorted({v for v, _ in reqs if not v.startswith("GLIBC_")})
    if others:
        print(f"其他版本 : {', '.join(others)}")
    libs = sorted({lib for _, lib in reqs})
    print(f"需要库   : {', '.join(libs)}")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1]))
