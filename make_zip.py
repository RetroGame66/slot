# -*- coding: utf-8 -*-
"""Pack deploy/ into a zip whose top level is System/, ready to unzip onto the SD card."""
import os, zipfile

# Layout: this script sits at the root of the source tree, `deploy/` beside it and the zip one
# level up, so the whole thing travels with the tree.
HERE = os.path.dirname(os.path.abspath(__file__))
SRC = os.path.join(HERE, "deploy")
OUT = os.path.join(os.path.dirname(HERE), "slot-frontend-System.zip")

entries = []
for dirpath, _dirnames, filenames in os.walk(SRC):
    for fn in filenames:
        full = os.path.join(dirpath, fn)
        rel = os.path.relpath(full, SRC).replace("\\", "/")
        entries.append((full, rel))
entries.sort(key=lambda t: t[1])

with zipfile.ZipFile(OUT, "w", zipfile.ZIP_DEFLATED, compresslevel=9) as z:
    for full, rel in entries:
        z.write(full, rel)

print("zip :", OUT)
print("size:", os.path.getsize(OUT), "bytes")
print("files:", len(entries))
for _full, rel in entries:
    print("   ", rel, os.path.getsize(_full))
