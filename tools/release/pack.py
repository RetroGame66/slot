# -*- coding: utf-8 -*-
"""Assemble slot-release-YYYY-MM-DD/: the front-end System zip(s) + SHA256SUMS."""
import os, shutil, zipfile, hashlib, sys

W = r"F:/WorkBuddyWorkSpace/2026-09-11-08-38-21"
SRC = os.path.join(W, "slot-main", "deploy")
REL = os.path.join(W, "slot-release-2026-09-24")
DATE = "2026-09-24"
os.makedirs(REL, exist_ok=True)

# 1) the new binary onto the payload
newbin = os.path.join(W, "slot-main", "target-device", "aarch64-unknown-linux-gnu", "release", "slot")
dst = os.path.join(SRC, "System", "slot")
shutil.copyfile(newbin, dst)
print("payload slot:", os.path.getsize(dst), hashlib.md5(open(dst, "rb").read()).hexdigest())


def zip_tree(out):
    entries = []
    for dp, _dn, fns in os.walk(SRC):
        for fn in fns:
            full = os.path.join(dp, fn)
            rel = os.path.relpath(full, SRC).replace("\\", "/")
            entries.append((full, rel))
    entries.sort(key=lambda t: t[1])
    with zipfile.ZipFile(out, "w", zipfile.ZIP_DEFLATED, compresslevel=9) as z:
        for full, rel in entries:
            with open(full, "rb") as fh:
                data = fh.read()
            zi = zipfile.ZipInfo(rel, date_time=(2026, 9, 24, 12, 0, 0))
            zi.compress_type = zipfile.ZIP_DEFLATED
            zi.create_system = 3
            # executable on anything that has to run on the device, plain otherwise
            mode = 0o755 if (rel.endswith(".so") or rel.endswith("/slot") or rel == "System/slot") else 0o644
            zi.external_attr = (mode | 0o100000) << 16 | 0x20
            z.writestr(zi, data)
    return entries


zh = os.path.join(REL, "slot-frontend-System-%s.zip" % DATE)
ents = zip_tree(zh)
print("zh zip:", os.path.basename(zh), os.path.getsize(zh), "entries", len(ents))

# 2) checksums over whatever is in the release folder
lines = []
for fn in sorted(os.listdir(REL)):
    p = os.path.join(REL, fn)
    if fn == "SHA256SUMS.txt":
        continue
    if os.path.isfile(p):
        h = hashlib.sha256(open(p, "rb").read()).hexdigest()
        lines.append("%s  %s" % (h, fn))
    print("member:", fn, os.path.getsize(p))
with open(os.path.join(REL, "SHA256SUMS.txt"), "w", encoding="utf-8", newline="\n") as fh:
    fh.write("\n".join(lines) + "\n")
print("--- SHA256SUMS.txt ---")
print("\n".join(lines))
