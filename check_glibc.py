#!/usr/bin/env python3
"""Print the max GLIBC_* version an ELF needs, its interpreter, and linkage."""
import sys, struct

def main(path):
    with open(path, "rb") as f:
        data = f.read()
    if data[:4] != b"\x7fELF":
        print(f"{path}: not an ELF"); return
    ei_class = data[4]            # 1=32, 2=64
    ei_data = data[5]             # 1=LE, 2=BE
    endian = "<" if ei_data == 1 else ">"
    is64 = ei_class == 2
    # e_shoff at 0x20 (32) / 0x28 (64); e_shentsize at 0x2e/0x3a; e_shnum at 0x30/0x3c
    if is64:
        e_shoff = struct.unpack(endian+"Q", data[0x28:0x30])[0]
        e_shentsize = struct.unpack(endian+"H", data[0x3a:0x3c])[0]
        e_shnum = struct.unpack(endian+"H", data[0x3c:0x3e])[0]
        e_shstrndx = struct.unpack(endian+"H", data[0x3e:0x40])[0]
    else:
        e_shoff = struct.unpack(endian+"I", data[0x20:0x24])[0]
        e_shentsize = struct.unpack(endian+"H", data[0x2e:0x30])[0]
        e_shnum = struct.unpack(endian+"H", data[0x30:0x32])[0]
        e_shstrndx = struct.unpack(endian+"H", data[0x32:0x34])[0]

    def section(i):
        off = e_shoff + i*e_shentsize
        # sh_name, sh_type, sh_flags, sh_addr, sh_offset, sh_size, sh_link, sh_info, sh_addralign, sh_entsize
        if is64:
            vals = struct.unpack(endian+"IIQQQQIIQQ", data[off:off+64])
        else:
            vals = struct.unpack(endian+"IIIIIIIIII", data[off:off+40])
        return dict(zip(["name","type","flags","addr","offset","size","link","info","align","entsize"], vals))

    shdrs = [section(i) for i in range(e_shnum)]
    # string table for section names
    shstr = shdrs[e_shstrndx]
    strtab = data[shstr["offset"]:shstr["offset"]+shstr["size"]]
    def sname(s):
        e = strtab.find(b"\x00", s["name"])
        return strtab[s["name"]:e if e>=0 else len(strtab)].decode("latin1")
    # locate .gnu.version_r (needed versions) and .dynamic (interp/flags) and .dynstr
    vern = dynstr = None
    interp = None
    maxver = "0.0"
    for s in shdrs:
        n = sname(s)
        if n == ".gnu.version_r": vern = s
        elif n == ".dynstr": dynstr = s
        elif n == ".interp": interp = data[s["offset"]:s["offset"]+s["size"]].split(b"\x00")[0].decode("latin1")
    if dynstr:
        ds = data[dynstr["offset"]:dynstr["offset"]+dynstr["size"]]
        def dstr(off):
            e = ds.find(b"\x00", off)
            return ds[off:e if e>=0 else len(ds)].decode("latin1")
        if vern:
            v = vern
            # version_need entries: each has header (16 bytes 32 / 24 bytes 64) then auxiliary
            off = v["offset"]; end = off + v["size"]
            while off < end:
                # Elf_Verneed: vn_version(Half) vn_cnt(Half) vn_file(Word) vn_aux(Word) vn_next(Word) = 16 bytes
                cnt, _, vn_file, vn_aux, vn_next = struct.unpack(endian+"HHIII", data[off:off+16])
                fname = dstr(vn_file)
                aoff = off + vn_aux
                for _ in range(cnt):
                    # Elf_Vernaux: vna_hash(Word) vna_flags(Half) vna_other(Half) vna_name(Word) vna_next(Word) = 16 bytes
                    vna_hash, vna_flags, vna_other, vna_name, vna_next = struct.unpack(endian+"IHHII", data[aoff:aoff+16])
                    vername = dstr(vna_name)
                    if vername.startswith("GLIBC_") or vername.startswith("GLIBC_PRIVATE"):
                        import re
                        m = re.match(r"GLIBC_(\d+)\.(\d+)", vername)
                        if m:
                            v = (int(m.group(1)), int(m.group(2)))
                            if v > tuple(map(int, maxver.split("."))):
                                maxver = f"{v[0]}.{v[1]}"
                    aoff += vna_next or 16
                if vn_next == 0: break
                off += vn_next
    print(f"file: {path}")
    print(f"  arch: {'64' if is64 else '32'}-bit, endian={'LE' if ei_data==1 else 'BE'}")
    print(f"  interp: {interp}")
    print(f"  max GLIBC_* requirement: {maxver}")

if __name__ == "__main__":
    for p in sys.argv[1:]:
        try:
            main(p)
        except Exception as e:
            print(f"{p}: error {e}")
