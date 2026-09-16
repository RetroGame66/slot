# What changed

A diff against upstream **`4345cb8bdb`** (2026-09-11, *Merge branch 'feat/core-picker-board'*).

**82 files changed, 6,829 insertions(+), 454 deletions(-)** across the branch — 15 added, 66
modified, 1 deleted. **The change set proper is 75 of them: 11 added and 64 modified, at 6,294
insertions and 453 deletions.** The other seven are this fork's notice and repository housekeeping
(§12), which is what lets the tree stand on its own as a public fork of an MIT project.

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

The readings in the table come from [pypinyin](https://github.com/mozillazg/python-pinyin) (MIT),
which is what `tools/gen_pinyin.py` imports to generate it; the table is a derived work of that
project's data, and `README.md` credits it as such.

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

## 4. Panel-mask presets, and a colour-correction stage

Neither existed upstream. The picture was multiplied by one fixed LCD3x table — always, with no
way to turn it down — and there was no colour correction of any kind.

**The panel mask** becomes five presets, cycled on `SELECT`+`X`: OFF / LCD3X 50% / LCD3X 100%
(the default, and upstream's look) / SCANLINE 50% / SCANLINE 100%. The card can ship its own 3x3
table in `System/mask.txt` in place of the built-in. `SCANLINE*` is a second table — the first row
of each three-row cell dark, the other two lit — and the two 50% steps are either table lerped
halfway toward an everywhere-white one, which is what lets `OFF` be a clean framebuffer rather
than a special case in the shader.

**The colour correction** is a 3x3 matrix in the game fragment shader, cycled on `SELECT`+`Y`
through seven presets: FULLCOLOR (identity), HALFCOLOR (50% saturation), NOCOLOR (luma only, with
the matrix overridable per card through `System/cc.txt`) and four tinted-backlight palettes.

Both modes persist to `System/display.txt` as two integers, written the moment either changes.

| File | Change |
|---|---|
| `slot/src/app.rs` | `DisplayFilter` — the five masks (built-in, lerped, scanline), the seven matrices, `applied_mask()` / `applied_cc()`, and the two `cycle_*` that write `display.txt`. |
| `slot-gfx/src/shaders.rs` | `u_cc` (mat3) and `u_cc_gamma` added to the game fragment shader. |
| `slot-gfx/src/pipeline.rs`, `fbo.rs` | Both pushed with the matrix, cached so a change uploads exactly once. |
| `slot-gfx/src/lcd3x.rs` | The built-in 3x3 table exposed as the same type the card's own table uses, so the card's and the shipped one are interchangeable. |
| `slot-input/src/gesture.rs` | `MaskCycle` and `ColorCycle` as chord actions. |
| `slot/src/root.rs` | `mask.txt`, `cc.txt`, and the read/write pair for `display.txt`. |

**The colour maths runs in linear.** `u_cc_gamma` is either `1.0` or `2.2`. At `1.0` the matrix
multiplies straight in the encoded space and the two `pow()` calls the shader gained are inverses
of one another, so they cancel — which is precisely what the game pass did before this existed, and
is why FULLCOLOR and NOCOLOR behave as they always did. At `2.2` (sRGB) the matrix multiplies in
linear, which is where saturation and a tint mapping have to live or they come out flat.
`CC_GAMMA = 2.2` is what HALFCOLOR and the four tinted backlights use; the card's `cc.txt` override
of the NOCOLOR matrix keeps its meaning because that mode still runs at `1.0`.

The panel mask keeps multiplying in the encoded space, after the correction. Both are
multiplications, so the order does not matter — the comment in the shader says so, because it looks
like it should.

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

Eight files that are not code, added or changed so the tree can stand on its own as a public fork
of an MIT project. Counting them, the repository commit is **82 files**; the 75 above are the
source change set on its own.

| File | Change |
|---|---|
| `NOTICE.md` | **New.** States that this is a modified fork, names the upstream author and the baseline commit, credits the modifications, lists which bundled parts keep their own licences, and disclaims affiliation. |
| `README.upstream.md` | **New.** Upstream's README, reproduced unmodified and introduced as such. Its place is here rather than at the tail of `README.md`, where it would have made the fork's front page speak in the upstream author's first person about a release this fork does not ship. |
| `README.md` | The bilingual banner at the top, and a front page written by the fork rather than by upstream: a *Downloads* section, what the fork adds, what it changes, what it leaves alone, the controls including the four chords it adds, the companion toolbox, what goes on the card, and the licensing position. |
| `LICENSE` | A second copyright line for the modifications, added beneath the original one. The upstream notice itself is untouched. |
| `CHANGES.md` | **New in the tree** — it already existed at the root of this package. Copying it in keeps the repository self-contained, since `NOTICE.md` links to it. |
| `.gitattributes` | **New.** `* text=auto eol=lf`, plus an explicit `binary` for the asset types. Without it, a Windows checkout with `core.autocrlf=true` rewrites every file it touches, and every diff comes back as a whole-file change. |
| `.gitignore` | `/deploy/`, `/toolchain/`, `/参考/`, `/rustup-init.exe` appended. A vendored build environment and a built card tree must never reach the repository. |
| `CNAME` | **Deleted.** It carried `slot.kowalski.io`, the upstream author's custom domain. Left in place, it makes a fork's Pages deployment fail on domain ownership. |

Nothing here changes the frontend's behaviour, and no upstream file was relicensed: `slot` and
every change in this fork stay MIT. The two emulator cores keep MPL-2.0 and GPL-2.0 — which is
what `licenses/` is there for.

---

## 13. The fork publishes builds

Not a change to the tree, but a change to what the tree is for, so it belongs in the same account.

Upstream is source-only: `README.md` used to say as much, and a reader wanting the frontend had to
install a cross toolchain first. This fork publishes a **Release** instead, and `README.md` gained a
*Downloads* section naming it. Two assets:

| Asset | What it is |
|---|---|
| `slot-frontend-System-<date>.zip` | The `deploy/` tree, ready to unzip onto a card. Top level is `System/`. |
| `slot-companion-toolbox-<date>-win64.zip` | A Windows maintenance tool — font subsetting, label pre-scaling, save backup and restore, clock, boot logo — with its source. Not in this tree and not part of `slot`: the frontend neither calls it nor needs it. |

Publishing a **binary** is what turns the licensing note from a caution into an obligation, so it is
worth being precise about what the frontend asset carries under `System/licenses/`: the MIT notice
for the frontend and this fork's modifications; `NOTICE.md`; mGBA's MPL-2.0 text with the section
3.1 notice; gpSP's GPL-2.0 text **and its corresponding source** as `gpsp-<commit>.tar.gz`, which is
what section 3(a) asks for when object code is conveyed; the OFL text for the card's typeface; and
an `OFFER.md` stating how that source commit was resolved and what that resolution is worth — the
libretro buildbot does not publish which commit built a given nightly, so the pairing is an
inference from a date rather than a proof, and saying so is more honest than implying otherwise.

The asset is assembled by `make_release_assets.py`, which lives beside this package rather than in
the repository, so it is not part of the diff counted above.

---

## Notes for a reader of the diff

- **Comments are English throughout.** Chinese text still appears where it is *data* rather than
  prose: the UI's own translated strings, the hanzi the pinyin table is built from, and the
  `[中]` / `[日]` / `[英]` tags a dump's filename may carry. Those are values the program
  parses and compares against; they are not remarks, and translating them would break the code.
- **Line endings are LF**, matching upstream.
- **`deploy/`, `toolchain/`, and the card's art** (covers, labels, roms) are not part of this
  package — it is the source change only.
- **The README's control tables were rebuilt, not corrected.** Upstream's listed six `SELECT`
  chords because six was all there was; the four this change adds (`X` is `MaskCycle`, `Y` is
  `ColorCycle`, `A` is `CheatToggle`, `VOL±` step the audio profile) are new chords, not
  newly-documented ones. The carousel row was relabelled as well: upstream's table says `L` / `R`,
  which are the shoulder buttons, and those do nothing on the shelf on their own — what
  `Shelf::hold_left` / `hold_right` answer is `Btn::Left` / `Btn::Right`, the D-pad.
- **Upstream's README now lives at `README.upstream.md`.** It used to be the tail of `README.md`,
  where a fork's front page ended up quoting the upstream author on his own release, his own
  issue policy, and a `System/` bundle this fork does not publish.
- **`README.md` is no longer a table of one-liners.** It carries an About this fork front page and
  a Controls section in which the four new chords are described one at a time — the rings, where
  each applies, what it persists, and what feedback it gives.
