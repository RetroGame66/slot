# What changed

A diff against upstream **`4345cb8bdb`** (2026-09-11, *Merge branch 'feat/core-picker-board'*).

**75 files changed, 5,929 insertions(+), 385 deletions(-)** — 11 new files, 64 modified.

Everything below is listed by feature rather than by file, because that is how it was built:
one concern at a time, with the files it touched.

---

## 1. Chinese interface, and a typeface the card supplies

The interface is Chinese. Rather than bake a CJK face into the binary, the interface reads its
typeface off the card at boot; the embedded face stays as the fallback for anything the card's
face does not carry — most CJK fonts ship without Latin glyphs, and a game code is Latin.

| File | Change |
|---|---|
| `slot-ui/src/text.rs` | `set_font(bytes)` — parse a TTF/OTF/TTC from the card. A parse that fails is ignored, leaving the embedded face. The bytes are leaked on purpose: `Font::from_bytes` borrows them and the font lives as long as the process. |
| `slot/src/root.rs` | `font_file()` — `System/fonts/` (first file by name, so which one wins does not depend on directory order), then the single-file spelling `System/font.ttf`. |
| `slot/src/frontend.rs` | `load_font()` called from `upload_faces`, before anything is rasterised — the card's face has to be in place for the *first* cart's label, not the second frame. |
| `crates/**` (many) | Every user-facing string. |
| `verify_slot.py` | Checks the built binary carries the Chinese strings **and** that the old English ones are gone. |

The build also carries a subset CJK font rather than the full 17 MB face, which is most of the
boot-time win in §3.

## 2. The letter ring, and the scan cache under it

A Chinese library has no alphabetical order of its own — the file names are hanzi — so the shelf
grew a second ring above the cart row that groups by the **pinyin initial** of the first
character. `宝可梦` files under `B`.

| File | Change |
|---|---|
| `slot-store/src/pinyin.rs` | **New.** Every hanzi in GB2312 plus the ones this card's titles use, each with its pinyin initial, sorted by code point so a lookup is one binary search. Generated, not hand-written: `tools/gen_pinyin.py`. |
| `slot-ui/src/letters.rs` | **New.** The ring itself: the same ring shape as the cart row, one level up. Up/Down move the marker and the shelf follows. |
| `slot-store/src/name.rs` | **New.** Filename → title. Parses the run of parenthesised tags a dump carries, and strips them unless the card says otherwise. |
| `slot-store/src/scan.rs` | `bucket()` — which letter a cart files under — plus a **scan cache** keyed on the `Games` and `Labels` directory stamps and the sorted file names under each. Adding, removing, renaming or re-labelling a game moves one of those, so the cache is thrown away then and only then. |
| `slot-store/src/lib.rs`, `slot-ui/src/lib.rs` | The rule that turns a file name into a title lives in the store, because the ring's letter is decided from it and a scan cannot ask the ui. |
| `slot/src/app.rs`, `frontend.rs` | Ring input; the shelf's own line of type, rebuilt only when the cart under the eye changes. |

Two decisions worth naming:

- **Digits do not get a bucket.** `1080 Snowboarding` files under `#`. A facet on the ring is a
  thing a hand crosses, so it has to be worth the crossing.
- **Empty slots are drawn.** A ring that closed up its gaps would put `B` next to `C` one cart
  apart on one card and two hundred apart on the next, and the distances would stop meaning
  anything.

## 3. Shelf performance: faces built off the frame loop, a bounded texture pool

A cart face costs about **70 ms** on this hardware, and a boot used to rasterise every cart on
the card before the first frame — seven seconds on a hundred games, all of it spent on carts
nobody is looking at.

| File | Change |
|---|---|
| `slot/src/face_builder.rs` | **New.** Builds the shelf's faces one cart at a time, nearest the caret first, off the frame loop. One request in flight: the answer is always wanted, and a queue would only let the worker fall behind the caret. |
| `slot/src/frontend.rs` | `RESIDENT` — twelve carts either side of the caret keep a texture; beyond that with slack, a cart's texture is released. This is what bounds the shelf's memory at about **16 MB** whatever the card holds, and makes a nine-hundred-game card the same proposition as a hundred-game one. |
| `slot-gfx/src/draw.rs` | `SpritePool::release_texture` — frees a slot and leaves a hole. Nothing reuses the index; a face is rebuilt into a fresh slot when its cart comes back. |
| `slot-store/src/gba.rs` | Title and code read in **one** open. Two opens per cart is two directory lookups and two cluster-chain walks where one will do, multiplied by the size of the card. Only the first bytes are read, never the whole 32 MB rom. |
| `slot-ui/src/cart.rs` | Per-phase timing inside `cart_face` (`shell 12.3 label 4.5 …`), because the device has no profiler and no console and a `boot.log` is where the numbers can exist. |
| `slot-ui/examples/face_timing.rs` | **New.** Times each builder individually, on the device. |

## 4. Colour correction in linear space

The display filter's two knobs (panel mask, colour correction) were already there; what changed
is **where** the colour maths happens.

| File | Change |
|---|---|
| `slot-gfx/src/shaders.rs` | `u_cc_gamma` added to the game fragment shader. `1.0` multiplies straight in the encoded space, as the picture did before — the two `pow()` calls are then inverses and cancel. `2.2` converts to linear, multiplies, and converts back. |
| `slot-gfx/src/pipeline.rs`, `fbo.rs` | The gamma is pushed with the matrix, cached so a change uploads exactly once. |
| `slot/src/app.rs` | `CC_GAMMA = 2.2`. HALFCOLOR (1) and the four tinted backlights (3..=6) use it; FULLCOLOR (0) and NOCOLOR (2) are pushed at 1.0 and stay **bit-identical** to before. |
| `slot-gfx/src/lcd3x.rs` | The built-in 3x3 mask exposed as the same type the card's own table uses, so the app can fall back to it. |

**Expected:** a half-colour stops going darker, and a monochrome backlight keeps its body
instead of being washed out. Saturation and a tint mapping have to be done in linear or they
come out flat — the same thing RetroArch's handheld shaders (`nds-color`, `lcd1x_nds`) do.
Multiplying the encoded value is what made the four backlights look pale.

The four backlights are rebuilt on the new basis, with a row = *target colour's linear value ×
luma weight (0.299/0.587/0.114)*. That puts the brightest part of the picture exactly on the
target colour and lets gamma pull the mid-tones apart. Peak colours:

```
DMG green  rgb(155,188,15)      ice-blue  rgb(120,170,215)
amber      rgb(240,165,60)      pink      rgb(240,130,185)
```

## 5. Audio latency profiles

What a player feels in a rhythm game is the distance between the beat they *hear* and the press
the game *judges* — the ring depth plus whatever the device buffers. That is now one knob.

| File | Change |
|---|---|
| `slot/src/audio/mod.rs` | `Profile` — **stable / balanced / strict** — owns `latency_us` (the device's share) and `period_frames` (the writer's granularity). Both were constants in `alsa.rs`; moving them onto the profile is what lets the tuning change on the device without a rebuild. |
| `slot/src/audio/alsa.rs` | Both come from the profile that opened the PCM. ALSA needs at least two periods, so a smaller request is quietly rounded up and buys nothing. |
| `slot/src/drc.rs` | The ring's lead follows the profile (`stable` parks four video frames ≈ 66.7 ms ahead). |
| `slot/src/session.rs`, `emu.rs`, `audio/sink.rs` | The profile is a parameter, not a property of the sink: it can only change by reopening the hardware, so it is fixed for the life of a session. |
| `slot/src/root.rs` | `System/audio.txt` — `stable` / `balanced` / `strict`. |
| `slot-ui/src/toast.rs`, `slot-input/src/gesture.rs` | `SELECT`+`VOL±` steps it, and a toast is the only confirmation it gets: the setting moves no pixel, so without the line nothing on screen says it moved. |

Chosen on the shelf only: applying a profile reopens the PCM, and the shelf is the one screen
with nothing playing to interrupt.

## 6. Cheat codes

| File | Change |
|---|---|
| `slot-retro/src/ffi.rs` | `retro_cheat_set` / `retro_cheat_reset` wired through, both **optional**. libretro names it `retro_cheat_set` — the intuitive `retro_set_cheat` does not exist, so looking it up under that name silently yields `None` and every cheat no-ops. gpSP exports neither. |
| `slot-retro/src/libretro.rs`, `core.rs` | Applied on the emulator thread like every other core entry point. |
| `slot/src/emu.rs` | Apply a list of `(enabled, code)`; the index is the slot the core builds its table from, so re-sending with a flipped flag toggles one code without the core losing track. |
| `slot/src/root.rs` | `System/Cheats/<stem>.txt`. One code per line, `#` starts a comment, blank lines ignored. A multi-line GameShark / Action Replay code is joined with `+` on one line. |
| `slot-ui/src/power_menu.rs`, `slot/src/app.rs` | A browsable per-code table on `SELECT`+`A`, two lines per row — the description above the raw code, in dimmer ink, so the label reads first. Pressing again while it is up flips every code at once. |

The codes are fed to the core verbatim; whether a given format works is mGBA's call. A missing or
unreadable file means no cheats, not an error — a device with no console is not the place to
surface that.

## 7. Backlight in twenty steps

`slot-power/src/device.rs`, `slot-store/src/slot_state.rs`. Twenty steps, nineteen of them lit:
step 0 is the panel off. Ten was too coarse at the bottom — step 1 sat well above what a dark
room wants, with nothing between it and black.

`slot-store/tests/durability.rs` covers the migration: a card written when the top was nine
still has to read as the level it named rather than as a corrupt file.

## 8. Button remapping

`slot/src/input/mask.rs`, `System/remap.txt`. A physical button can be redirected to another game
button — `X = A` for a hand used to reaching for X where a GBA expects A.

Held as a target per **physical** index rather than one folded bit, so remapping two physical
buttons onto the same game button still releases each correctly: releasing one clears only that
physical key's contribution, not the bit the other, still-held key is driving.

## 9. Cart geometry and the shelf row

| File | Change |
|---|---|
| `slot-ui/src/shelf.rs` | `PITCH` (side carts half off-screen, so the row reads as continuing), `CENTER_SCALE = 1.5` (the hero), `SIDE_ALPHA` (how much face a side cart keeps). |
| `slot-ui/src/cart.rs` | `FACE_SCALE = 2` — faces are rasterised and composited at twice the logical cart size and drawn into the logical quad, so the hero is only ever downsampled, never stretched. The label panel is laid out at the higher resolution too. |
| `slot-ui/src/slot_chrome.rs` | The cart eases from the carousel's enlarged scale back to its own as it seats, **anchored by its foot** — centring it lets the extra height out both ways and the cart drops by half of it the moment the insert starts. |
| `slot-ui/src/board.rs`, `silhouette.rs` | Cart back, sockets, chip; masks rasterised at the face resolution. |

## 10. GB/GBC cart art — added, not yet wired

`crates/slot-ui/assets/cart_gb.svg`, `cart_gbc.svg`, `cart_gb_detail.svg`.

White-filled shapes only, same contract as `cart.svg` (the code colours them), same 4 px/mm
scale. **Nothing references them yet** — they are assets for extending the frontend to GB and
GBC, which mGBA's libretro build already emulates from the same `.so`. Listed here so the
addition is not mistaken for a working feature.

## 11. Build and release scripts

| File | Change |
|---|---|
| `check_glibc.py` | Print the max `GLIBC_*` a binary needs, its interpreter, and linkage. |
| `verify_slot.py` | Check a freshly built device binary: aarch64 ELF, the Chinese strings present, the old English ones gone. |
| `make_zip.py` | Pack `deploy/` into a zip whose top level is `System/`, ready to unzip onto the card. |
| `tools/gen_pinyin.py` | **New.** Regenerates `slot-store/src/pinyin.rs`. |

`make_zip.py` and `verify_slot.py` derive their paths from the script's own location, so they
travel with the tree. `gen_pinyin.py` needs `pypinyin` and, optionally, the card's `Labels/`
beside the tree.

---

## 12. Fork notice, attribution, and the line-ending policy

Seven files that are not code, added or changed so the tree can stand on its own as a public
fork of an MIT project.

| File | Change |
|---|---|
| `NOTICE.md` | **New.** States that this is a modified fork, names the upstream author and the baseline commit, credits the modifications, lists which bundled parts keep their own licences, and disclaims affiliation. |
| `LICENSE` | A second copyright line for the modifications, added beneath the original one. The upstream notice itself is untouched. |
| `README.md` | A short bilingual banner at the top: modified fork, not the upstream project, with links to `NOTICE.md` and `CHANGES.md`. |
| `CHANGES.md` | **New in the tree** — it already existed at the root of this package. Copying it in keeps the repository self-contained, since `NOTICE.md` links to it. |
| `.gitattributes` | **New.** `* text=auto eol=lf`, plus an explicit `binary` for the asset types. Without it, a Windows checkout with `core.autocrlf=true` rewrites every file it touches, and every diff comes back as a whole-file change. |
| `.gitignore` | `/deploy/`, `/toolchain/`, `/参考/`, `/rustup-init.exe` appended. A vendored build environment and a built card tree must never reach the repository. |
| `CNAME` | **Deleted.** It carried `slot.kowalski.io`, the upstream author's custom domain. Left in place, it makes a fork's Pages deployment fail on domain ownership. |

Nothing here changes the frontend's behaviour, and no upstream file was relicensed: `slot` and
every change in this fork stay MIT. The two emulator cores keep MPL-2.0 and GPL-2.0 — which is
what `licenses/` is there for.

---

## Notes for a reader of the diff

- **Comments are English throughout.** Chinese text still appears where it is *data* rather than
  prose: the UI's own translated strings, the hanzi the pinyin table is built from, and the
  `[中]` / `[日]` / `[英]` tags a dump's filename may carry. Those are values the program
  parses and compares against; they are not remarks, and translating them would break the code.
- **Line endings are LF**, matching upstream.
- **`deploy/`, `toolchain/`, and the card's art** (covers, labels, roms) are not part of this
  package — it is the source change only.
- **The README's control table was corrected.** It listed `SELECT`+`X` as opening a chooser;
  the code (`Action::MaskCycle` → `cycle_mask()` → `(mode + 1) % 5`) cycles the preset. The
  other three chords the change added (`Y`, `A`, `VOL±`) were missing and are now listed, along
  with the files the change added under `System/`.
