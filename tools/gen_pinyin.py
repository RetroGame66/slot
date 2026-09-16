#!/usr/bin/env python3
"""Generate slot's pinyin initial table: `crates/slot-store/src/pinyin.rs`.

The scan uses it to turn the first character of a game's name into a letter, which is the bucket
the ring above the shelf groups by. The table carries no metadata: it is every hanzi in GB2312
plus the ones this card's titles actually use, each with its pinyin initial, sorted by code point
— so a lookup at runtime is one binary search.

    python tools/gen_pinyin.py

To add characters, edit this and run it again rather than hand-editing `pinyin.rs`. The `LETTERS`
byte string and the `HANZI` array have to correspond one for one, and a mismatch does not fail
loudly — it quietly looks up the wrong letter (see below).

**Run the unit test**: `bash toolchain/letters-check.sh`. `the_table_is_sorted` is not optional:
a binary search over an unsorted table does not error, it just returns the wrong value, so the
table's sortedness is the only thing standing between this and that bug. It was hit on the first
generation — inside a Rust multi-line byte string the newline and the indentation after it are
literal characters, and the first line's four-space indent was not absorbed by the continuation
either, so the whole table landed 465 + 4 bytes out and '宝' looked up as 'Z'. Hence the `\\`
continuations below, and the first line deliberately unindented.
"""

import os
from collections import Counter

from pypinyin import Style, lazy_pinyin

# The script lives in the tree it generates into, so the tree is two levels up. `Labels/` is a
# sibling of the tree — the card's own art, which this only glances at to catch a character that
# falls outside GB2312.
TREE = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
OUT = os.path.join(TREE, "crates", "slot-store", "src", "pinyin.rs")
LABELS = os.path.join(os.path.dirname(TREE), "Labels")


def hanzi() -> list[tuple[str, str]]:
    """(hanzi, initial), sorted by code point."""
    chars = set()
    # The whole GB2312 hanzi range.
    for hi in range(0xB0, 0xF8):
        for lo in range(0xA1, 0xFF):
            try:
                ch = bytes([hi, lo]).decode("gb2312")
            except Exception:
                continue
            if len(ch) == 1:
                chars.add(ch)
    # Plus every character this card's titles actually use, in case one falls outside GB2312.
    if os.path.isdir(LABELS):
        for f in os.listdir(LABELS):
            for ch in os.path.splitext(f)[0]:
                if ord(ch) > 0x2E80:
                    chars.add(ch)

    table = {}
    for c in chars:
        p = lazy_pinyin(c, style=Style.FIRST_LETTER)
        if p and p[0] and p[0].isalpha() and ord(c) > 0x2E80:
            table[c] = p[0].upper()
    return sorted(table.items(), key=lambda kv: ord(kv[0]))


def main():
    items = hanzi()
    if not items:
        raise SystemExit("collected no hanzi at all; check Labels/ and pypinyin")
    dist = Counter(v for _, v in items)
    print("covering %d hanzi; letter distribution %s" % (len(items), dict(sorted(dist.items()))))

    out = []
    out.append('''//! Pinyin initials, for the shelf's letter ring.
//!
//! A Chinese library has no useful alphabetical order of its own — the file names are hanzi —
//! so the ring groups by the initial of the title's first character, which is what a player
//! reading 宝可梦 reaches for when they want B.
//!
//! GENERATED, and not by hand: the table below is every hanzi in GB2312 plus the ones this
//! card's titles actually use, each with the first letter of its pinyin, sorted by code point
//! so a lookup is a binary search. Regenerate rather than edit; `tools/gen_pinyin.py`.

/// Every hanzi the table knows, sorted by code point.
const HANZI: &[char] = &[
''')
    line = []
    for c, _ in items:
        line.append("'%s'," % c)
        if len(line) == 24:
            out.append("    " + "".join(line) + "\n")
            line = []
    if line:
        out.append("    " + "".join(line) + "\n")
    out.append("];\n\n")

    # The byte string uses `\` continuations: the newline and the indent that follows it are
    # eaten, so only letters reach the data. The first line must NOT be indented — a continuation
    # eats "newline + next line's indent" and never the first character of a line.
    letters = "".join(v for _, v in items)
    out.append("/// The initial of `HANZI[i]`, as a byte, in the same order.\nconst LETTERS: &[u8] = b\"")
    for i in range(0, len(letters), 64):
        chunk = letters[i : i + 64]
        out.append(chunk + ("\\\n    " if i + 64 < len(letters) else ""))
    out.append('";\n\n')

    out.append('''/// The bucket a title belongs in: the pinyin initial of its first character.
///
/// A Latin letter is its own bucket. Everything else is `#` — the digits, punctuation, kana, and
/// any hanzi the table does not know. The digits are the case worth stating: filing `1080
/// Snowboarding` under `1` bought a facet on the shelf's ring for a title nobody looks for by
/// number, and the ring is a dial a hand crosses, so a facet has to be worth the crossing.
pub fn initial(title: &str) -> char {
    let Some(c) = title.chars().next() else {
        return '#';
    };
    if c.is_ascii_alphabetic() {
        return c.to_ascii_uppercase();
    }
    if c.is_ascii() {
        return '#';
    }
    match HANZI.binary_search(&c) {
        Ok(i) => LETTERS[i] as char,
        Err(_) => '#',
    }
}

#[cfg(test)]
mod tests {
    use super::{initial, HANZI, LETTERS};

    #[test]
    fn reads_the_first_character() {
        assert_eq!(initial("宝可梦-红宝石"), 'B');
        assert_eq!(initial("洛克人Zero1"), 'L');
        assert_eq!(initial("恶魔城晓月MOD版"), 'E');
        assert_eq!(initial("超级机器人大战A"), 'C');
        assert_eq!(initial("Advance Wars"), 'A');
        assert_eq!(initial("1080 Snowboarding"), '#', "a digit is not a bucket");
        assert_eq!(initial("007 - The World Is Not Enough"), '#');
        assert_eq!(initial("-"), '#');
        assert_eq!(initial(""), '#');
    }

    #[test]
    fn the_table_is_sorted() {
        // A binary search over an unsorted table silently misses, which would file games under
        // `#` with no other symptom.
        for w in HANZI.windows(2) {
            assert!(w[0] < w[1], "table out of order at {:?}", w[0]);
        }
        assert_eq!(LETTERS.len(), HANZI.len());
    }
}
''')

    text = "".join(out)
    with open(OUT, "w", encoding="utf-8", newline="\n") as f:
        f.write(text)
    print("wrote %s (%.1f KB)" % (os.path.relpath(OUT, TREE), os.path.getsize(OUT) / 1024))
    print("now run:  bash toolchain/letters-check.sh   (or on the device: test-on-device.sh -p slot-store)")


if __name__ == "__main__":
    main()
