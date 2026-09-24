# What changed

A diff against upstream **`4345cb8bdb`** (2026-09-11, *Merge branch 'feat/core-picker-board'*).

**104 files changed, 11,256 insertions(+), 904 deletions(-)** across the branch — 20 added, 82
modified, 2 deleted. Some of them are this fork's notice and repository housekeeping rather than
changes to the frontend (§12), which is what lets the tree stand on its own as a public fork of an
MIT project.

The counts above were taken again after §19 landed; every section is in them.

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
| `README.md` | The bilingual banner at the top, and a front page written by the fork rather than by upstream: a *Downloads* section, what the fork adds, what it changes, what it leaves alone, the controls including the four chords it adds, the companion toolbox, what goes on the card, installing it on two cards or on one, and the licensing position. |
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

## 14. The letter ring, stood on its end

The ring shipped laid across the panel above the cart row, with `Up` / `Down` turning it. Hands
disagree about that: a dial across the panel asks for the shoulders, and `L` / `R` are bound to
nothing on the shelf — there is no core under it to want them. Rather than pick, the ring reads
two ways and `SELECT` + `START` swaps which one is standing.

Both views are **one** `Letters`. The rail is not a second index with a second position to keep in
step; it is the same drum, the same marker and the same counts drawn a quarter turn round, so the
two views cannot disagree about where the library is. All four keys step whichever one is up, so
swapping never leaves a key that does nothing.

| File | Change |
|---|---|
| `slot-ui/src/letters.rs` | `draw_rail()` — the drum down the right-hand edge (`rail_cx()` / `rail_cy()`). The capsule and the ridges are the dial's own textures drawn with `Draw::Turned` at `FRAC_PI_2` rather than rasterised again, so they cannot drift from the wheel's; a letter gives up *height* as its pane turns away and keeps its width, which is the vertical drum's rule through ninety degrees. |
| `slot/src/app.rs` | `LetterNav` (`Wheel` by default, `Sidebar`), `toggle_letter_nav()` on `SELECT` + `START`, and `L1` / `R1` stepping the marker beside `Up` / `Down`. Declined while the core picker is up, the way the other chords are. |
| `slot/src/root.rs` | `letter_nav()` / `write_letter_nav()` — `System/letternav.txt`, `wheel` or `sidebar`, in the same shape as `display.txt` and `audio.txt`. |
| `slot-input/src/gesture.rs` | `Action::LetterNavToggle`; `Btn::Start` at bit 2048 of the chord table. `SELECT` + `START` and not `SELECT` + `B`: `START` is bound to nothing but the core picker here, while `B` is every game's most-mashed key and the way out of every screen. |
| `slot-ui/src/toast.rs` | `Toast::NavWheel` / `NavSidebar`. The swap moves no cart and no letter, so the line is the whole of the answer — there is nothing else on screen that changed. |

## 15. A shortcut card under the about screen

The chords are the useful half of this fork and the half nobody could remember: five of them, on a
machine whose every button was already spoken for, discoverable only by being told. The about
screen is where a user goes to find out what a thing is, so the list went under it — `MENU` opens
the label and the card hangs below, in two sections (the carousel, and in game), paged with
`Up` / `Down` and closed with `B` or `MENU`.

The card scrolls rather than paging hard, easing to where the last press asked it to go the way the
letter dial does, and it is bounded by its own content: `about_scroll_max()` is the sum of the rows,
not a number written down, so adding a key to the list gives it somewhere to go without touching
the arithmetic. A line pinned at the foot says what the arrows do, which is the one thing on the
card that can be read without scrolling.

| File | Change |
|---|---|
| `slot-ui/src/shortcuts.rs` | **New.** The rows themselves: `Row::Head` / `Row::Key`, `SHORTCUT_ROWS` (22 of them, the shelf's eleven and the game's nine, each head included), `SHORTCUT_HINT`, and the faces. Two columns — the key's name ends at `KEY_COL`, the description starts a gap past it — haloed, because the card is drawn over a photograph. Four tests, one of which asserts the longest key still fits its column. |
| `slot/src/app.rs` | `Phase::About { scroll, want }` (it used to be a bare `About`), `ABOUT_PAD` / `ABOUT_GAP` / `ABOUT_PAGE` / `ABOUT_EASE`, `about_first_y()`, `about_scroll_max()`, `set_shortcut_faces()`, and the draw loop that skips the rows off the panel. |
| `slot/src/frontend.rs` | The 22 rows and the hint line are built at boot with the rest of the fixed furniture — opening the one screen whose job is to be read is the worst moment to be asking a font for twenty lines. |
*A correction, made after the fact:* the label's compliance block had been missing
`LIBRETRO` since it was written — the two cores are libretro cores and the README credits
them — and `sticker::the_compliance_block_is_the_credits` had been failing for exactly that.
The block names it now, and the ten lines still sit inside the column's 156 px.

*A correction, made after the fact:* the label's compliance block had been missing
`LIBRETRO` since it was written — the two cores are libretro cores and the README credits
them — and `sticker::the_compliance_block_is_the_credits` had been failing for exactly that.
The block names it now, and the ten lines still sit inside the column's 156 px.

| `slot-ui/src/sticker.rs` | `draw_sticker_at()` — the label at a stated height rather than centred, because it is no longer the whole screen; `draw_sticker()` is that with the centred height. |

---

## 16. Light and dark modes

The interface printed itself in one ink from the first commit to this one. It now prints in either,
and the colours it prints in live in one module instead of six.

Everything on the shelf was written in a single set of colours, and those colours were *constants*
in the file that happened to need them: `letters::INK`, `hud::HUD_INK`, `plate::INK`, `clock::INK`,
`battery::INK`, the card's case colours in `theme.txt`, and one black scrim over the wallpaper. Six
copies of one idea is why a mode could not exist; `palette` owns all six now.

Three decisions shape it. **The mode is remembered on the card** (`System/slot.state`'s `mode=`
line) and a state file without that line reads dark, so a card written by the previous build boots
exactly as it did. **It is switched with `SELECT+START` on the shelf and nowhere else** — the shelf,
because the case and the letter strip are what the mode reprints, and answered there before the core
picker can take the arrow keys. **A cart does not change**: a shell is a colour its owner chose and
a label is artwork, and neither is chrome. A card's `theme.txt` still addresses the dark case only.

Switching is not free, and the reason is worth stating: most type on this device is not drawn, it is
*baked* — rasterised once into a texture with the ink burned into it — so a face that already exists
cannot change colour and the binary re-bakes on the toggle. What does follow the mode for nothing is
everything drawn as a filled quad: the case, the scrim, the gauge and the HUD's bar.

| File | Change |
|---|---|
| `slot-ui/src/palette.rs` | **New.** `mode()` / `set_mode()` and every colour derived from them: `ink`, `dim_ink`, `panel_ink`, `halo`, `plate`, `panel`, `keycap`, `ground`, `ridge_ink`, `scrim`, `LIGHT_CASE`. A process-wide atomic rather than a parameter: the colours are read in some forty places, a dozen of them face builders running on another thread, to express a value with exactly one answer at any moment. |
| `slot-ui/src/status.rs` | **New.** The status line — the clock at one end of the top band, the gauge and its percentage at the other, and in the middle the one badge that says which mode is on. |
| `slot-store/src/slot_state.rs` | `Mode`, and the `mode` field it is read into. Anything unreadable reads `Dark`. |
| `slot/src/app.rs` | `toggle_mode()`; `Action::ModeToggle` answered on the shelf and only while the core picker is shut; `Icon::of_mode(palette::mode())` for the badge. |
| `slot-input/src/gesture.rs` | `Btn::Start => Action::ModeToggle` on the shelf, where it used to do nothing. |
| `slot-ui/src/{battery,clock,plate,hud,slot_chrome,icon}.rs` | Each asks the palette instead of naming a constant. `battery` moved its percentage under the capsule and grew the gauge to 1.5×; `clock` gained `clock_face` / `CLOCK_PX`; `hud` clears the band (`PLATE_Y`); `icon` gained the sun and the moon. |
| `slot-ui/tests/{battery,chrome,hud,shelf}.rs`, `slot-input/tests/gesture.rs` | The same assertions against a mode that can be either. |

---

## 17. The letter strip, laid flat

§14 stood the ring on its end: a dial down the right-hand edge, scrolled in slots. It is now the
thing it was upstream — a flat index across the top band — with the drum's metal kept, because the
ridging was the one part of that experiment that read as machinery rather than as letters in a slot.
Nine-pixel ridges at a twenty-four-pixel pitch, both ends tapering, the ends of the run outside the
letters, and the whole strip centred on the band rather than on the window inside it, which is where
it had been sitting low enough to look like an accident.

The lit line under the band is gone. It had been mirrored from the cartridge bay's own front edge —
where it means something, because a cart really is cut off there — and under the letters it read as
a rule someone had drawn rather than as a moulding. The bay keeps its own.

The gear is back in the strip. Type grew where it had been set too small to read at arm's length:
the shelf's title 24 → 30, the clock 16 → 20.

| File | Change |
|---|---|
| `slot-ui/src/letters.rs` | `SCALE = 0.8` as one multiplier over every shape (`PITCH`, `CENTRE_PX`, `NEIGHBOUR_PX`), `MODULE_MID` to centre on the band, the ridges restored, and the fade measured in slots (`FADE_FROM = 3.0 × PITCH`) so changing the scale cannot quietly flatten it. |
| `slot-ui/src/shelf.rs` | `SHELF_TITLE_PX` 24 → 30 with `SHELF_TITLE_H` 34 → 40 — `fit` shrinks for width and has no opinion at all about height, so a larger title in an unchanged box is a title that gets clipped. |
| `slot-ui/src/slot_chrome.rs` | The band stops printing its lit under-edge; the bay keeps it. |
| `slot/src/{app,frontend}.rs`, `slot/src/root.rs` | The strip's layout, and the removal of `letter_nav()` / `write_letter_nav()`: the wheel-or-sidebar choice belonged to the drum, and a strip laid flat has no second way to stand. |

---

## 18. Two builds from one source

The fork began as a translation done in place: upstream's English was replaced line by line, in
whichever file each line lived in. That was the right shape for one language and the wrong shape for
two — by this commit the strings were spread across ten files, with nothing holding the two
languages beside each other and nothing to notice when one moved.

`slot-ui/src/lang.rs` now holds the words, as two tables in one file, chosen **at compile time**.
Chinese is the default, so the build that ships is the build that shipped before this commit;
English is `--features device,lang-en`. A compile-time feature rather than a setting on the card,
because the English build's whole point is to be free of the other language, and because the two are
two artifacts rather than one artifact with a preference.

The shortcut card keeps its own pair of tables in `shortcuts.rs` instead. A row is not a word: it is
a key line and a description that have to fit a fixed column, so its wording is a layout decision as
much as a translation, and the two belong in the file that owns the layout.

**Most of the English is not a translation.** Upstream is an English program and this fork replaced
its words, so the originals were recoverable from the commit the fork was taken from, and were
recovered rather than reinvented: *State Saved*, *Power Off*, *Restarting*, *Back / Delete / Load*,
*Bringing the radio up*, *Nobody arrived*, *set the clock*. Only what the fork added — the cheat and
audio toasts, the clock hint, the whole card — is new writing.

Two things about the columns are worth knowing before editing either table. The key line's budget is
`DESC_X − HALO_PX`, because a keycap is its text plus padding and never narrower than a key. The
description's budget is the card's own width, and it fails differently: `line()` asks `fit` for
`f32::MAX`, so a description that is too long is **not** shrunk or wrapped, it is drawn past the row
and clipped by it. In English capitals against the Chinese card's proportions that comes out at
about eighteen characters, which is why several rows read terser than the Chinese they answer. Both
columns are measured; a test already held the key line, and this adds one for the description.

Tests name neither language. Where an assertion used to spell out a string it now asks `lang`, and
where it was checking that two things differ it still says so without naming either.

| File | Change |
|---|---|
| `slot-ui/src/lang.rs` | **New.** The vocabulary, twice: toasts, the power menu and its two shutdown lines, the clock hint's label, the switcher's legend, the core picker's legend, the link rows and their steps and failures, and the two undo labels. |
| `slot-ui/src/shortcuts.rs` | A second `ROWS` and `HINT`, `cfg`-chosen, and the English rows written to the columns rather than translated into them. `L1`/`R1` where the Chinese card says `L` / `R`, because `L/R` is what the D-pad's left and right need two rows down. |
| `slot-ui/src/{toast,power_menu,clock,polaroids}.rs`, `slot/src/{app,frontend,link_start}.rs` | Every user-visible literal replaced by a `lang` name. |
| `slot-ui/Cargo.toml`, `slot/Cargo.toml` | `lang-en`; forwarded from `slot` so one flag switches the whole program. |
| `crates/*/tests/*` | The assertions that named a string, and the description-column test. |

---

## 19. Favourites, a save that survives a smaller rewrite, and a sticker in the jump

### 19.1 Favourites

B on the shelf carries two gestures — a short press stars the cart under the caret, a hold swaps the
shelf between the library and the starred carts — on the same press-then-release mechanism A already
used for insert. Which of the two a press is cannot be known until it either comes up or passes the
threshold, so the press is remembered and the release decides, exactly as A's is.

| File | Change |
|---|---|
| `slot-store/src/slot_state.rs` | `favorites: BTreeSet<String>`, written one `favorites=` line per stem so a stem carrying `=`, a space or a comma survives the round trip. **The reader tolerates the key's absence**, like `mode`: a card that has never been starred carries no line, and reading that as corruption would throw the whole file away — the cart the user left selected, the brightness, the timezone — on the first boot of this build. |
| `slot-ui/src/shelf.rs` | **The view.** `view: Vec<usize>` is the list of carts the ring walks; `faces` stays indexed by the library, so a swap reuses every face already rasterised rather than dropping them and showing placeholders while they are built again. `slot_rects()` states the row's layout once, so a mark drawn over a cart cannot drift off the cart it marks. |
| `slot-ui/src/icon.rs` | `Icon::Star` (`\uf005`) and `Icon::StarOutline` (`\uf006`), appended to `ALL` so existing indices are untouched. |
| `slot/src/app.rs` | `fav_held`/`fav_hold()` beside `play_held`/`play_hold()`; `toggle_favorite()`, `toggle_fav_view()`, `show_all()`, `show_favorites()`, `retally_letters()`; the star and the indicator. |
| `slot/src/frontend.rs` | Three textures uploaded at boot — the star, and the indicator unlit and lit — because the star is the one glyph on the shelf drawn in a colour of its own rather than the case's ink. |
| `slot-ui/src/shortcuts.rs` | Two rows on the about card's shelf section, in both tables: `轻点 B` / `长按 B` and their English pair. The card is the frontend's own list of its keys, so a key that is not on it is a key the user has to be told about somewhere else. |

Three decisions worth naming:

- **Letter nav stands down on the favourites shelf.** The dial counts the library, and a view
  narrower than the library would seat the caret on a cart the view does not hold.
- **An empty favourites shelf is refused, not shown.** A screen with nothing on it and no way to tell
  why is worse than the shake every other refusal gets.
- **The star is inside the cart.** In the band the moulding leaves between the shell's curved top
  edge and the label it reads as printed on the cartridge; floating above the row it read as a badge
  on the shelf.

### 19.2 A sticker in the jump

A cart whose face has not been built — which is most of what a jump across the alphabet is made of —
now carries its own label colour, and is smeared along the travel while the ring glides.

| File | Change |
|---|---|
| `slot-ui/src/shelf.rs` | The placeholder branch of `draw_row` draws the shell, then one solid `Draw::Rect` in the panel `label_panel` defines, in `label_colour(stem)`. While `glide` is running, the cart and its label are stretched by `SMEAR` and one fainter copy is drawn behind them (`glide_dir()` gives the direction). At rest the stretch is exactly 1, so a still row is drawn as it always was, and a cart wearing a real face is never smeared — that is the game's own picture. |

Two quads a cart and no new texture. A jump crosses hundreds of carts and hundreds of faces cannot be
resident, but every cart can still hold its own colour, and the row no longer reads as empty cases
going past.

### 19.3 A save that shrinks to a single repeated byte

The size guard refused any save shorter than the file on the card. That is right for a core that
truncates a real save and wrong for a card written under one core and read under another — gpSP
reports 128 KiB for every game while mGBA reports the game's real size, so the two disagree about a
file that is neither of their doing, and the guard turned it into a save that could never be written
again.

| File | Change |
|---|---|
| `slot-store/src/sav.rs` | **New.** `save_plan(old, sav)` — pure, twenty tests. `Blank` / `Write` / `Unchanged` / `RefuseCompressed` / `RefuseShrink` / `BackupAndWrite`. The *shape* of the range that would be dropped decides: filler is a write, one repeated byte is a rename-aside, anything with structure is still a refusal. |
| `slot/src/persist.rs` | `write_sav` dispatches on the plan; `load_sav()` — an rzip container is recognised and not fed to the core; `backup_path()`. |
| `slot-store/src/lib.rs`, `slot/src/session.rs` | The exports, and the read path. |
| `slot/tests/eject.rs` | The integration case: a cart whose old file is longer than the core now reports still lands. |

The property the change is built around: **what could be written before can still be written**, and
what is refused is a subset of what was refused. A wrong plan costs a `.bak` next to the save, not
the save.

## Notes for a reader of the diff

- **The interface's words live in `slot-ui/src/lang.rs`**, in two tables, one of which is
  compiled. The Chinese strings are still values the program parses and compares against — but they
  are values in one place now, and the English build contains none of them.
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
