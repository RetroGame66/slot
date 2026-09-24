import os, glob, re
from fontTools.ttLib import TTFont
from fontTools import subset

S = r'F:\WorkBuddyWorkSpace\2026-09-15-21-43-08\_scratch'
FULL = r'F:\WorkBuddyWorkSpace\2026-09-11-08-38-21\slot-main\deploy\System\fonts\NotoSansCJKsc-Bold.otf'
DEV = os.path.join(S, 'devfont.otf')
OUT = os.path.join(S, 'NotoSansCJKsc-Bold.sub.otf')
SRC = r'F:\WorkBuddyWorkSpace\2026-09-11-08-38-21\slot-main\crates'

BASE_RANGES = [
    (0x0020, 0x007E), (0x00A0, 0x00FF), (0x2010, 0x2027), (0x2030, 0x205E),
    (0x2190, 0x21FF), (0x2460, 0x24FF), (0x25A0, 0x25FF), (0x3000, 0x303F),
    (0xFE30, 0xFE4F), (0xFF00, 0xFFEF),
]

# A = every codepoint the current on-card font carries
A = set(TTFont(DEV, lazy=True).getBestCmap().keys())

# B = BASE_RANGES + non-ASCII chars in Rust string literals of the current source
B = set()
for lo, hi in BASE_RANGES:
    B.update(range(lo, hi + 1))
src_chars = set()
for p in glob.glob(os.path.join(SRC, '**', '*.rs'), recursive=True):
    try:
        t = open(p, encoding='utf-8', errors='replace').read()
    except Exception:
        continue
    for m in re.finditer(r'"((?:\\.|[^"\\])*)"', t):
        for ch in m.group(1):
            if ord(ch) > 0x7F:
                src_chars.add(ord(ch))
B |= src_chars

need = sorted(A | B)
print('A=%d B=%d src=%d need=%d 夹(0x5939) in B: %s' % (len(A), len(B), len(src_chars), len(need), 0x5939 in B))

subset.main([
    FULL,
    '--unicodes=' + ','.join('U+%04X' % u for u in need),
    '--layout-features=*', '--no-hinting', '--desubroutinize',
    '--name-IDs=*', '--drop-tables+=DSIG',
    '--output-file=' + OUT,
])

new = set(TTFont(OUT, lazy=True).getBestCmap().keys())
print('new size=%d glyphs=%d' % (os.path.getsize(OUT), len(new)))
print('superset of A: %s' % A.issubset(new))
print('夹 in new: %s' % (0x5939 in new))
print('lost (A - new): %d' % len(A - new))

rep = []
rep.append('A(old)=%d B=%d src=%d need=%d' % (len(A), len(B), len(src_chars), len(need)))
rep.append('new size=%d glyphs=%d' % (os.path.getsize(OUT), len(new)))
rep.append('superset_of_A=%s' % A.issubset(new))
rep.append('jia_0x5939_in_new=%s' % (0x5939 in new))
rep.append('lost_count=%d' % len(A - new))
open(os.path.join(S, 'aug_report.txt'), 'w', encoding='ascii').write('\n'.join(rep) + '\n')
