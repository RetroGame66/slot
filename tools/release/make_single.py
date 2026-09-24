import os, re, base64

base = r"F:\WorkBuddyWorkSpace\2026-09-11-08-38-21\操作指南-图文版"
src = os.path.join(base, "操作指南-图文版.html")
dst = os.path.join(base, "操作指南-图文版-单文件.html")
img_dir = os.path.join(base, "img")

html = open(src, encoding="utf-8").read()

def repl(m):
    rel = m.group(1)
    # rel like img/bootlogo.png
    p = os.path.join(base, rel.replace("/", os.sep))
    if not os.path.exists(p):
        print("MISSING", rel)
        return m.group(0)
    ext = os.path.splitext(p)[1].lower().lstrip(".")
    mime = "png" if ext in ("png",) else ext
    data = base64.b64encode(open(p, "rb").read()).decode("ascii")
    return 'src="data:image/%s;base64,%s"' % (mime, data)

new, n = re.subn(r'src="(img/[^"]+)"', repl, html)
print("inlined images:", n)
open(dst, "w", encoding="utf-8").write(new)
print("wrote", dst, os.path.getsize(dst), "bytes")
# sanity: confirm no remaining external img/
left = new.count('src="img/')
print("remaining src=img/:", left)
