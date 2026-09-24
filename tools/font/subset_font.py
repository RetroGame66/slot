#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""
把卡上的中文字体子集化到 slot 真正需要的那几个字。

为什么值得做
------------
slot 开机时会把 `System/fonts/` 里的字体**整个读进内存**再交给 fontdue 解析
（`frontend.rs::load_font` → `text::set_font`）。卡上现在放的是 16.9 MB 的思源黑体，
在 A53 上"读 17MB + 解析"实测要好几秒 —— 而 slot 要显示的字其实只有：

  * 100 个游戏名（来自 `Labels/` 的文件名）
  * 界面固定文案（源码里那 66 个非 ASCII 字符）
  * ASCII、数字、常用标点

子集化到这些字之后字体通常只有几十到一两百 KB，读入和解析都快一个数量级。

用法
----
    python subset_font.py --src <源字体.otf> --out <目标.otf>
    python subset_font.py --src ... --out ... --wide    # 额外带上整个 GB2312（将来加游戏不愁）

加新游戏后重新跑一次即可（`--wide` 就不必）。
要求：Python 3 + fontTools（pip install fonttools）
"""

import argparse
import glob
import os
import re
import sys

# 字号以外的兜底字符：ASCII 可打印区 + 常用标点/符号
BASE_RANGES = [
    (0x0020, 0x007E),   # ASCII
    (0x00A0, 0x00FF),   # Latin-1
    (0x2010, 0x2027),   # 连字符、破折号、引号
    (0x2030, 0x205E),   # 更多标点
    (0x2190, 0x21FF),   # 箭头
    (0x2460, 0x24FF),   # 带圈数字
    (0x25A0, 0x25FF),   # 几何图形
    (0x3000, 0x303F),   # CJK 标点
    (0xFE30, 0xFE4F),   # CJK 兼容形式
    (0xFF00, 0xFFEF),   # 全角
]


def gb2312_chars():
    """GB2312 全部汉字。"""
    out = set()
    for hi in range(0xB0, 0xF8):
        for lo in range(0xA1, 0xFF):
            try:
                ch = bytes([hi, lo]).decode("gb2312")
            except Exception:
                continue
            if len(ch) == 1:
                out.add(ch)
    return out


def chars_from_labels(labels_dir):
    out = set()
    if not labels_dir or not os.path.isdir(labels_dir):
        return out
    for f in os.listdir(labels_dir):
        out.update(os.path.splitext(f)[0])
    return out


def chars_from_sources(src_root):
    """slot 源码字符串字面量里出现的所有非 ASCII 字符 —— 界面固定文案就在里面。"""
    out = set()
    if not src_root or not os.path.isdir(src_root):
        return out
    for path in glob.glob(os.path.join(src_root, "**", "*.rs"), recursive=True):
        try:
            text = open(path, encoding="utf-8", errors="replace").read()
        except Exception:
            continue
        for m in re.finditer(r'"((?:\\.|[^"\\])*)"', text):
            for ch in m.group(1):
                if ord(ch) > 0x7F:
                    out.add(ch)
    return out


def main():
    ap = argparse.ArgumentParser(description="子集化 slot 的卡上字体")
    ap.add_argument("--src", required=True, help="源字体（.otf/.ttf/.ttc）")
    ap.add_argument("--out", required=True, help="输出的子集字体")
    ap.add_argument("--labels", default=None, help="游戏名来源目录，默认 <out 所在工作区>/Labels")
    ap.add_argument("--src-root", default=None, help="slot 源码根，默认 <out 所在工作区>/slot-main/crates")
    ap.add_argument("--extra", default="", help="额外要保留的字符（直接写一串）")
    ap.add_argument("--wide", action="store_true", help="额外带上整个 GB2312（约 +6800 字）")
    args = ap.parse_args()

    try:
        from fontTools import subset
        from fontTools.ttLib import TTFont
    except ImportError:
        print("缺少 fontTools：pip install fonttools", file=sys.stderr)
        return 2

    guess_root = os.path.dirname(os.path.dirname(os.path.abspath(args.out)))
    labels = args.labels or os.path.join(guess_root, "Labels")
    src_root = args.src_root or os.path.join(guess_root, "slot-main", "crates")

    needed = set()
    for lo, hi in BASE_RANGES:
        needed.update(chr(c) for c in range(lo, hi + 1))
    needed |= chars_from_labels(labels)
    needed |= chars_from_sources(src_root)
    needed |= set(args.extra)
    if args.wide:
        needed |= gb2312_chars()
    needed = {c for c in needed if c.strip() or c == " "}

    print("需要的字符：%d 个（游戏名 %d，界面 %d%s）" % (
        len(needed), len(chars_from_labels(labels)), len(chars_from_sources(src_root)),
        "，含 GB2312 全量" if args.wide else ""))

    before = os.path.getsize(args.src)
    unicodes = sorted({ord(c) for c in needed})
    subset.main([
        args.src,
        "--unicodes=" + ",".join("U+%04X" % u for u in unicodes),
        "--layout-features=*",
        "--no-hinting",
        "--desubroutinize",
        "--name-IDs=*",
        "--drop-tables+=DSIG",
        "--output-file=" + args.out,
    ])
    after = os.path.getsize(args.out)
    print("输出：%s" % args.out)
    print("  %.2f MB -> %.2f MB（%.1f 倍小）" % (before / 1048576, after / 1048576, before / after))

    cm = TTFont(args.out, lazy=True).getBestCmap()
    missing = [c for c in sorted(needed) if ord(c) not in cm]
    print("  覆盖校验：缺 %d 个 %s" % (len(missing), "".join(missing[:20]) if missing else "(全有)"))
    return 0


if __name__ == "__main__":
    sys.exit(main())
