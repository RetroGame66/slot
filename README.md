# slot.

> **This is a modified fork.** It is based on
> [BrandonKowalski/slot](https://github.com/BrandonKowalski/slot) by **Brandon T. Kowalski**
> (MIT), with changes by [@RetroGame66](https://github.com/RetroGame66) on top.
> It is **not** the upstream project, and is not endorsed by or affiliated with its author.
> What changed is in [`CHANGES.md`](CHANGES.md); origin and licensing in [`NOTICE.md`](NOTICE.md).
>
> **本仓库是上游 slot 的修改版，非官方版本。** 原作 © Brandon T. Kowalski，MIT 许可；
> 修改由 [@RetroGame66](https://github.com/RetroGame66) 完成。

A bespoke, GBA-only frontend for the Anbernic RG SP.

Upstream's own README is kept **unmodified** beside this one, at
[`README.upstream.md`](README.upstream.md). It is the project's documentation in its author's own
words, and it is worth reading — but it describes *upstream*, not this fork. This file describes
the fork: what it adds, what it changes, and what the buttons actually do here.

---

## About this fork

A modified build of `slot`, aimed at a Chinese GBA library living on one SD card.

**99 files changed, +9,377 / −828** against upstream `4345cb8b` — 92 of them the change set proper
(15 new files, 77 modified) and 7 the fork's notice and repository housekeeping. The full account,
feature by feature and file by file with the reasoning, is in [`CHANGES.md`](CHANGES.md).

The short of it: the interface is Chinese and takes its typeface off the card; the shelf's index
runs across the top of the case, because a library of hanzi has no alphabetical order of its own; a
cart can carry its own cheats, button remap and display settings, so tuning lives with the card
instead of with the binary; the whole frontend prints in either a dark case or a light one; and the
picture gained two things it never had — selectable panel-mask presets and a colour-correction stage
— reachable from the device instead of by rebuilding.

**There are two builds of it.** The Chinese one is what this branch compiles by default; the English
one is `--features device,lang-en`. They are one source tree — the interface's words live in
`slot-ui/src/lang.rs`, in two tables one of which is compiled — so a change to the interface is a
change to both, and neither is a translation of the other kept in a separate repository.

## Downloads

Prebuilt, so a card does not have to be compiled from source first: see
[**Releases**](https://github.com/RetroGame66/slot/releases).

| Asset | What it is |
|---|---|
| `slot-frontend-System-*.zip` | The frontend, ready to unzip onto the card. Top level is `System/`, so its contents go at the card's root. Includes the helper `.bat` files and the third-party licence texts the two emulator cores oblige a distribution to carry. |
| `slot-companion-toolbox-*-win64.zip` | The Windows maintenance tool described below, with its source. Windows 10 / 11, x64. |
| `SHA256SUMS.txt` | Hashes for the two above. |

Both are **this fork's** builds, with everything on this page in them. Upstream's releases are a
different build and contain none of it.

## What this fork adds

**Chinese interface.** Every user-facing string is Chinese. The typeface is read off the card at
boot — `System/fonts/` (first file by name) or the single-file spelling `System/font.ttf`, as TTF,
OTF or TTC. Changing fonts no longer needs a rebuild, and most of the font payload leaves the
binary. The embedded face stays as the fallback for glyphs a CJK font does not carry: most CJK
faces ship without Latin, and a game code is Latin.

**The letter ring.** A second ring above the cart row, on the same axis and the same shape, so it
reads without being explained — the marker in the middle is the letter the cart under the caret
belongs to, and `Up` / `Down` moves the marker with the shelf following. Twenty-seven facets: `#`
then A to Z. `#` holds everything the pinyin table cannot read, which is a title that opens with a
digit, punctuation, or a character the table does not know. Empty facets are drawn dim and stepped
over rather than closed up, because a ring that hid its gaps would make "B next to C" mean one
cart apart on one card and two hundred apart on the next.

**The same ring, stood on its end.** The ring reads two ways — a **wheel** laid across the panel,
or a **sidebar** stood down its right-hand edge — and `SELECT` + `START` swaps which one is up. It
is one dial either way, drawn twice, so the two views cannot disagree about where the library is:
the facets still bunch towards the ends and one letter still plainly owns the middle. `L` / `R` step
it as well as `Up` / `Down`, which is the pair a dial lying across the panel asks for and the pair
users asked for by name. Which one reads better is a matter of hands, so the choice lives on the
card in `System/letternav.txt` (`wheel` or `sidebar`) rather than in the build.

**A shortcut card under the about screen.** `MENU` on the carousel opens the label, and hanging
under it is every key the machine answers: one section for the carousel, one for in game, the five
`SELECT` chords included. `Up` / `Down` page it, `B` or `MENU` closes, and a line pinned at the foot
says so. The rows are fixed strings rasterised at boot with the rest of the furniture — opening
the one screen whose job is to be read is the worst moment to be asking a font for twenty lines.

**Panel-mask presets.** Upstream multiplies the picture by one fixed LCD3x table, always. Here the
mask is five presets on `SELECT` + `X` — OFF, LCD3X 50%, LCD3X 100% (the default), SCANLINE 50%,
SCANLINE 100% — and the card may ship its own 3x3 table in `System/mask.txt` instead of the
built-in.

**Colour correction.** Upstream has none. Here the game shader gains a 3x3 colour matrix, cycled
on `SELECT` + `Y` through seven presets: FULLCOLOR (identity, the default), HALFCOLOR (50%
saturation), NOCOLOR (luma only, black and white), and four tinted-backlight palettes — DMG green,
ice-blue, amber, pink — which keep the picture's luma and recolour it in the hue of a coloured
LCD. The card may override the NOCOLOR matrix in `System/cc.txt`.

The grading is done in **linear** space (gamma 2.2) for HALFCOLOR and the four tints; FULLCOLOR
and NOCOLOR are pushed at gamma 1.0 and multiply exactly as the shader did before. That matters
because multiplying a half-colour in the encoded space darkens it and washes a tinted backlight
out; converting to linear, multiplying and converting back is what makes them read as rich. The
four tints are built on that basis, with a row = the target colour's linear value times the luma
weight (0.299 / 0.587 / 0.114).

**Cheat codes.** One code per line in `System/Cheats/<rom stem>.txt`; `#` starts a comment, blank
lines are ignored, and a multi-line GameShark or Action Replay code is joined with `+` on one line.
Text after the `#` is the code's on-device label and never reaches the core. `SELECT` + `A` opens a
browsable table over the paused game — `Up` / `Down` move, `A` toggles the row in hand, `B` leaves
— and pressing `SELECT` + `A` again while the table is up flips every code at once. The toast says
which way.

**Audio latency profiles.** What a rhythm game judges you on is the distance between the beat you
hear and the press it sees — the ring depth plus whatever the device buffers. That is now one
setting, `stable` / `balanced` / `strict`, read from `System/audio.txt` and stepped with
`SELECT` + `VOL+` / `VOL-`. It is a latency dial, not a quality one: the GBA's own 32768 Hz already
carries everything the console can produce, so what is being traded is how much audio sits queued
between the emulator and the speaker.

**Button remapping.** `System/remap.txt`, one `<physical> = <game>` per line — `X = A` for a hand
that reaches for X where a GBA expects A. It touches only what reaches the core, so a remapped
button keeps its usual job in the menus. Names: `up`, `down`, `left`, `right`, `a`, `b`, `x`, `y`,
`l1`, `r1`, `l2`, `r2`, `start`, `select`, plus `menu`, `volup`, `voldown`, `power` and `lid`,
which reach no GBA button of their own but can be pointed at one.

**Name tags, kept or stripped.** `System/labels.txt` — `strip_tags on` (the default) drops the
bracketed facts a dump carries in its filename; `strip_tags off` keeps them, which is what you want
for a pack that marks editions with `[中]` / `[日]` / `[英]` suffixes. The ring files carts by their
base title either way, so a title that keeps its tags still lands under the right letter.

**GB / GBC cartridge art.** Cartridge shapes drawn for GB and GBC carts, at the same 4 px/mm and
under the same white-fill contract as `cart.svg` (the code colours them). **Assets only — nothing
references them yet.** They are here so the addition is not mistaken for a working feature.

**Build and release scripts.** `check_glibc.py` (the highest `GLIBC_*` symbol a binary needs, its
interpreter and its linkage), `verify_slot.py` (a built device binary really is aarch64, and really
carries the Chinese strings and not the old English ones), `make_zip.py` (pack a `deploy/` tree into
the zip that goes on a card), `tools/gen_pinyin.py` (regenerate the pinyin table), and
`crates/slot-ui/examples/face_timing.rs` (time the face builders on the device).

**A Windows companion tool.** `slot-companion-gui.exe` — one standalone window, no browser, no
Python, no runtime to install — does the card upkeep that would otherwise be a shell session:
subset the interface font down to the characters the card actually uses, pre-scale `Labels/` art so
the shelf stops resampling all of it on every boot, back up and restore `Saves/` and `States/`, set
the device clock from the PC, and swap the boot logo. It talks to the device over `adb`. It is a
separate download from the frontend and the frontend neither needs it nor knows about it — it is
here because the fiddly parts of maintaining a card should be a button. Described in
[The companion toolbox](#the-companion-toolbox).

**Settings that travel with the card** rather than with the binary. `System/` now carries `fonts/`,
`labels.txt`, `display.txt`, `mask.txt`, `cc.txt`, `audio.txt`, `remap.txt`, `letternav.txt` and `Cheats/`. Every one
of them is optional: a missing, blank or misspelled file is the shipped default, not an error,
because a device with no console is not the place to surface a typo in a config file.

## What this fork changes

**The shelf's faces are built off the frame loop.** A cart face costs about **70 ms** on this
hardware, and a boot used to rasterise every cart on the card before the first frame — seven
seconds on a hundred games, all of it spent on carts nobody is looking at. Faces are now built one
at a time, nearest the caret first, against a bounded pool: twelve carts either side of the caret
keep a texture, with three carts of slack before one is released. That holds the shelf at about
**16 MB** whatever the card holds, so a nine-hundred-game card is the same proposition as a
hundred-game one. Title and code are read in one open rather than two.

**The backlight has twenty steps instead of ten.** Ten was too coarse at the bottom — step 1 sat
well above what a dark room wants, with nothing between it and black. Step 0 is the panel off.

**The cart row is a hero and a row, not three equal carts.** The selected cart is drawn at 1.5x
with its neighbours pushed further out, half off-screen, so the row reads as continuing past the
selection rather than stopping at it. Faces are rasterised at twice the logical cart size and drawn
down into it, so the hero is only ever downsampled, never stretched. A cart eases into the slot
anchored by its foot, so it does not drop by half its own height the moment the insert starts.

**The control tables are rebuilt.** The four `SELECT` chords above are written down for the first
time, the `System/` inventory is complete (upstream's listed `theme.txt` and `selected_core.ini`
only), and the save-state switcher's own keys are documented. The carousel's row is browsed with
`Left` / `Right` — the D-pad — and not with the shoulder buttons, which do nothing there on their
own.

## What this fork leaves alone

Boot, the launcher and the core picker, emulator plumbing, the two libretro cores, the save files
and save-state rings, rewind, fast-forward, the lid behaviour, mute, the blue-light control, the
link features, the about screen and the power menu are upstream's and are left as they stand.

**Comments in this tree are English**, in the prose style the tree already uses. Chinese appears
only where it is *data* the program parses and compares against: the interface's own translated
strings, the hanzi the pinyin table is built from, and the `[中]` / `[日]` / `[英]` tags a dumped
filename may carry. Those are values, not remarks, and translating them would break the code.

---

## Controls

A `+` means both together. `†` marks a chord this fork adds; everything else is upstream's.

### Anywhere

| Input                       | Action                                |
|-----------------------------|---------------------------------------|
| `SELECT` + `Up` / `Down`    | Adjust brightness                     |
| `SELECT` + `Left` / `Right` | Adjust blue light                     |
| `SELECT` + `X` †            | Cycle the panel mask preset           |
| `SELECT` + `Y` †            | Cycle the colour correction           |
| `VOL+` / `VOL-`             | Change the volume                     |
| `VOL+` + `VOL-`             | Mute, remembering the level           |

### On the carousel

| Input           | Action                              |
|-----------------|-------------------------------------|
| `Left` / `Right`| Browse the cart row                 |
| `L` / `R` †     | Step the letter index, one slot at a time |
| Tap `A`         | Resume the last save state          |
| Hold `A`        | Start the game fresh                |
| `MENU`          | About screen, and the shortcut card under it |
| `START`         | Choose which emulator runs the cart |
| `SELECT` + `START` † | Print the interface light, or dark again |
| `SELECT` + `VOL+` / `VOL-` † | Step the audio profile  |

### On the about card (`MENU`)

| Input           | Action                              |
|-----------------|-------------------------------------|
| `Up` / `Down`   | Page the shortcut list              |
| `B` or `MENU`   | Close, back to the carousel         |

The card is two sections — the carousel and in game — and it carries the `SELECT` chords as well,
which is the part that was being forgotten.

### In game

| Input                     | Action                                                                                           |
|---------------------------|--------------------------------------------------------------------------------------------------|
| Hold `MENU`               | Save state, eject the cart, back to the carousel                                                 |
| Double tap `MENU`         | Save state switcher: pick one to load or delete, or undo the last save or load within 30 seconds |
| `SELECT` + `MENU`         | Link with another RG SP. gpSP carts only                                                         |
| `SELECT` + `R1`           | Save state                                                                                       |
| `SELECT` + `L1`           | Load the most recent save state                                                                  |
| `SELECT` + `A` †          | Open the cart's cheat table                                                                      |
| Hold `L2`                 | Rewind                                                                                           |
| Hold `R2`                 | Fast-forward                                                                                     |
| Double tap `R2`           | Lock fast-forward on. Press again to unlock                                                      |

### On the save state switcher (double tap `MENU`)

| Input                    | Action                                          |
|--------------------------|-------------------------------------------------|
| `Left` / `Right`         | Move through the saved states                    |
| `A`                      | Load the one in hand                             |
| `X`                      | Undo the last save or load, within 30 seconds    |
| `Y`                      | Delete the one in hand                           |
| `B` or `MENU`            | Close, back to the game                          |

### The five chords this fork adds, in full

**`SELECT` + `X` — panel mask.** Five stops, in a ring: `OFF` → `LCD3X 50%` → `LCD3X 100%` →
`SCANLINE 50%` → `SCANLINE 100%`. The default is `LCD3X 100%`, which is upstream's look. The
50% steps are the same table lerped halfway toward a clean framebuffer.

**`SELECT` + `Y` — colour correction.** Seven stops, in a ring: `FULLCOLOR` (identity, the
default) → `HALFCOLOR` (50% saturation) → `NOCOLOR` (luma only) → `DMG green` → `ice-blue` →
`amber` → `pink`. The last four are the tinted-backlight palettes.

Both of those work on the carousel **and** in game, and both write the pair to
`System/display.txt` the moment either changes — so the look you picked is the look on the next
boot. They are declined while the core picker is up, so a chord cannot stack on top of it.

**`SELECT` + `A` — cheat table.** In game. The first press opens the table over the paused game:
`Up` / `Down` move, `A` toggles the code in hand, `B` leaves. A second `SELECT` + `A` while it is
up flips every code at once, and the toast says `金手指已开启` or `金手指已关闭`. With no file for
that cart, the table says so rather than failing.

**`SELECT` + `VOL+` / `VOL-` — audio profile.** On the carousel, and only there: applying a
profile reopens the PCM, which is free on the shelf and a gap in the sound anywhere else. `VOL+`
steps forward, `VOL-` back, and a toast (`音频：稳定` / `音频：均衡` / `音频：严格`) is the only
confirmation it gets — the setting moves no pixel, so without the line nothing on screen would say
it moved.

**`SELECT` + `START` — light or dark.** On the carousel. The whole frontend prints one of two ways:
a dark case with light type, or a light case with dark type. The choice is written to
`System/slot.state` as `mode=dark` or `mode=light` and survives a reboot; a state file written before
this existed has no such line and reads dark, which is what that card was already showing. A
cartridge does not change with it — a shell is a colour its owner chose and a label is artwork, and
neither is chrome. Switching re-rasterises the type, because most of what is on this screen is baked
into textures with the ink burned in.

### How the `SELECT` chords behave

`SELECT` is the chord key, and it is deferred rather than swallowed: on a press it waits up to
**600 ms** for a second button. If one arrives, the chord fires and the whole gesture is consumed —
`SELECT` never reaches the game, on either edge. If none arrives, the press is handed to the core
as an ordinary `SELECT`, which is why a game that wants `SELECT` held still gets it after that
window rather than not at all.

`X` and `Y` on their own are still the game's, or the switcher's; a chord key is only a chord while
`SELECT` is down.

Closing the lid writes a save state and turns off the display. Open it again and you are back in
the game. Leave it shut for three minutes and slot powers off, resuming from that save state on the
next boot. The lid is not a sleep — the panel goes dark but the board keeps running, which is why
the three minutes exist rather than an indefinite standby.

---

## What goes on the card

```
BIOS/         gba_bios.bin, optional. Absent means mGBA's own high level BIOS.
Games/        .gba roms.
Labels/       <rom stem>.png, drawn on the cartridge face. Absent means a text only label.
Saves/        .sav and .srm battery saves.
States/       <core>/<rom stem>/, save state rings ten deep per cart.
System/       the binary, both cores, and the settings listed below.
Wallpapers/   .png, one picked at random each boot and drawn behind the shelf.
```

Everything in `System/` other than the binary and the two cores is optional:

```
theme.txt          housing / recess / opening / edge, the four colours of the shell.
selected_core.ini  <rom stem> = <core>, which emulator runs a cart. Every cart defaults to mGBA.
fonts/             the card's own typeface for the interface. First file by name wins;
                   System/font.ttf is the single-file spelling of the same thing.
labels.txt         strip_tags on / off. Off keeps the [中] / [日] / [英] suffixes some packs
                   use to mark editions, rather than stripping bracketed tags from names.
display.txt        "mask_mode cc_mode", written when SELECT+X or SELECT+Y changes them.
audio.txt          stable / balanced / strict, written when SELECT+VOL steps it.
mask.txt           the card's own 3x3 panel mask, if it ships one.
cc.txt             the card's own colour-correction matrix, for the NOCOLOR mode.
remap.txt          physical button remapping, one "<physical> = <game>" per line.
Cheats/<stem>.txt  one cheat code per line; `#` starts a comment and blank lines are ignored.
                   A multi-line GameShark or Action Replay code is joined with `+` on one
                   line. Text after the first `#` is the code's on-device label and never
                   reaches the core.
```

`display.txt` holds two integers, so the two display modes survive a reboot:

```
2 0        # mask_mode cc_mode: LCD3X 100%, FULLCOLOR
```

`mask_mode` runs 0 to 4 (`OFF`, `LCD3X 50%`, `LCD3X 100%`, `SCANLINE 50%`, `SCANLINE 100%`) and
`cc_mode` runs 0 to 6 (`FULLCOLOR`, `HALFCOLOR`, `NOCOLOR`, `DMG green`, `ice-blue`, `amber`,
`pink`). An unparsable file — or no file — means the shipped look.

Label art is drawn at 196x86, or about 2.28:1. Anything else is scaled to cover that box and centre
cropped, so a square or portrait image loses its top and bottom. Bigger art is fine and comes down
to size; smaller gets stretched up and shows it.

`System/theme.txt` is entirely optional and controls the appearance of the slot:

```
housing #24242a
recess  #1a1a1e
opening #050508
edge    #4d4d57
```

`System/selected_core.ini` is entirely optional and names which core a cart's save states belong
to, one `<rom stem> = <core>` per line. Every cart defaults to mGBA, and states are kept apart per
core under `States/<core>/<rom stem>/` so switching cores later never mixes one core's save with
another's. Both cores ship in `System/`, so naming `gpsp` actually switches emulators for that cart
— gpSP exists for the serial link hardware mGBA's libretro build does not carry:

```
Emerald = gpsp
```

## Installing

The card layout is upstream's; what differs is where the binary comes from. The frontend goes in
`System/` at the root of whichever card the device mounts as its content card — and which card that
is depends on whether you use one or two, so it is worth deciding before you flash anything.

### Two cards

1. Write the latest [AGS-102](https://github.com/BrandonKowalski/AGS-102) `.img` release to an SD
   card, and insert it into **Slot 1** — the side with the volume buttons. That image is the
   device's operating system; this frontend is not part of it, and this fork does not change it.
2. Unzip `slot-frontend-System-*.zip` from the [releases](#downloads) onto a second SD card. Its
   top level is `System/`, so its contents go at the card's root, `System/licenses/` with them.
   Building it yourself instead is described in *Building*.
3. Add `Games/`, `Saves/`, `BIOS/`, `Labels/`, `Wallpapers/` as you like.
4. Insert that card into **Slot 2** — the side with the power and reset buttons.

Both cards are ordinary FAT filesystems. Nothing is flashed onto the second one and nothing on it
is write-protected, so a card laid out by hand and a card built by `task dist:device` are the same
card. The companion toolbox below is the least tedious way to do that laying out.

### One card

AGS-102 supports a single card natively, so there is nothing to patch to get it. Its init script
mounts the content card as **Slot 2's first partition if that partition exists and mounts, and
otherwise this card's own data partition** — its own words, from `overlay/etc/init.d/rcS`:

> Normal boot mounts the frontend/data card: TF2 p1 wins if usable, else this card's own
> "primary" FAT partition (p7).

Leave Slot 2 empty and that first branch never fires, so the OS falls through to its own card. That
makes the procedure:

1. Write `ags102.img` to the card, insert it into **Slot 1**, and leave **Slot 2 empty**.
2. Power on and let the first boot finish. It paints `EXPANDING STORAGE` while it grows the data
   partition to the card's full size and formats it. It then reports **`SLOT NOT FOUND`**, which is
   expected rather than a fault: the card still holds no frontend. (AGS-102's own `INSTALL.md` says
   `ADD FRONTEND TO SD CARD` at this point. That string belongs to BaseOS's NextUI session, which
   AGS-102 replaced with its own — `SLOT NOT FOUND` is what this OS actually shows.)
3. **Copy nothing onto the card before that first boot.** Step 2 *creates* the partition, so
   anything placed there beforehand is erased by the format. Flash, boot, then copy, in that order.
4. Power off by holding `POWER`, take the card out, and plug it into a computer. A volume labelled
   **`BASEOS`** appears, sized to the whole card.
5. Unzip `slot-frontend-System-*.zip` from the [releases](#downloads) into the **root of that
   volume**. Its top level is `System/`, so `System/` lands at the root; put `Games/`, `Saves/`,
   `BIOS/`, `Labels/` and `Wallpapers/` beside it. Building it yourself is described in *Building*.
6. Put the card back into **Slot 1** and power on. `slot` starts.

Two consequences of the arrangement are worth weighing before committing to it:

- **The whole card is the content card.** On a 16 GB card roughly 14.5 GB is that volume, and the
  frontend plus both cores take about 9 MB of it. There is no second volume to spill games into.
- **Reflashing costs you the games.** AGS-102 publishes no `.bosupd` payloads, so moving to a new
  version means reflashing the 1.4 GB image — and the first boot afterwards grows and reformats the
  data partition again, taking `Games/`, `Saves/`, `States/` and `Labels/` with it. Copy those to a
  computer first. That is a property of the partition layout rather than something AGS-102 or this
  fork can route around, and it is the reason the two-card arrangement exists at all.

The first boot also leaves a `README.txt` at the card root. It describes installing NextUI, which
is not what this OS runs, so it can go.

**Optional: keep the card's root clean by putting the frontend in the system image instead.** The
session looks for the binary in two places, the card first (`overlay/usr/sbin/ags-session`):

```
/mnt/sdcard/System/slot     the card
/usr/bin/slot               baked into the rootfs
```

`adb` is active by default over a data-capable USB-C cable, so with the card in the device:

```
adb push System/slot /usr/bin/slot
adb push System/mgba_libretro.so /usr/bin/mgba_libretro.so
adb shell chmod +x /usr/bin/slot
adb shell sync && adb reboot
```

The two files share a directory because that is the second place `slot` looks for its cores
(`crates/slot/src/core.rs`). `Games/`, `Saves/` and the rest stay on the card regardless, since the
session sets the content root to the card either way. The rootfs is a 512 MiB slot with about
100 MB used, so 9 MB is not a space question. What this buys is a clean card root and a fallback if
the card's copy is ever lost — not immunity from a reflash, which overwrites the rootfs too.

## The companion toolbox

A small Windows program, shipped in the releases as `slot-companion-toolbox-*-win64.zip`. It is
**not** part of the frontend: `slot` does not call it, does not require it, and runs identically
without it. It exists because the maintenance a card needs — fonts, art, saves, clock, boot logo —
is otherwise an `adb` shell session, and that is a poor thing to ask of anyone who just wants to
play. Unzip it anywhere, plug the device in over USB, and double-click the `.exe`; the badge at the
top reads red for "not connected", green for "connected".

| Button | What it does |
|---|---|
| **Subset font** | Reads every game name in the card's `Games/`, then cuts a subset of the full typeface in `fonts/` down to just the characters those names use — plus the interface's own wording, which is why the button can be pressed and the menus still be readable. Pushes the result to `System/fonts/`. Reboot to take effect. The full face is backed up first. |
| **Restore font** | Pushes the full typeface back over the subset. Reboot to take effect. |
| **Set clock** | Writes the PC's UTC time to the device's battery-backed clock and sets the timezone (default `Asia/Shanghai`). |
| **Pre-scale labels** | Resizes the card's `Labels/*.png` down to the cartridge face's own **394x171** — cover-fit and centre-cropped, by the same algorithm `slot`'s `art::cover` uses, so the picture on screen does not change. `slot` otherwise resamples every label at boot (about **74 ms** each, so roughly seven seconds across a hundred carts); pre-scaled art is 1:1 and costs nothing. Idempotent — art that is already that size or smaller is skipped. |
| **Back up saves** | Pulls `Saves/` (battery saves) and `States/<core>/` (save states) to a timestamped folder on the PC. |
| **Restore saves** | Pushes a backup back to the device. If the card already holds a save of the same name, it lists them and asks before overwriting. |
| **Boot logo** | Previews the card's current `bootlogo.bmp` and replaces it: drop any image on the window and it is scaled or cropped to **720x480** and converted to 24-bit BMP. The existing picture is backed up and the write is confirmed once more before it happens, because this writes to the boot partition. |

It is a single self-contained executable — Python, pygame, Pillow, fontTools and `adb` are all
inside it, so there is nothing to install. It needs no display driver beyond what Windows provides.
Its source is in the same zip under `source/`, and a newer `adb.exe` placed beside the executable
takes precedence over the bundled one.

## Building

The device build is one cargo invocation, against an aarch64 Linux target:

```
cargo build --release -p slot --no-default-features --features device
```

Add the target when cross-compiling rather than building on the device:

```
cargo build --release --target aarch64-unknown-linux-gnu \
  -p slot --no-default-features --features device
```

`--no-default-features --features device` is not optional: the default features pull in the host
backends (`gilrs`, `libudev`, `alsa`), which do not cross-compile and are not what the device runs.

Upstream's `taskfile.yml` does the same thing inside an arm64 container, and is worth using when
you want a whole card tree rather than just a binary:

- `task build:device` — build the binary (`target-device/release/slot`).
- `task dist:device` — build it, fetch both cores, scaffold the tree, and copy `licenses/` in
  beside the cores.

Two of this fork's own scripts are useful right after a build:

- `python check_glibc.py <binary>` — the highest `GLIBC_*` symbol the binary needs, plus its
  interpreter and linkage. AGS-102 is glibc 2.35; building against something older is what keeps
  one binary usable on more than one image.
- `python verify_slot.py <binary>` — confirms it is an aarch64 ELF and that it carries the Chinese
  strings and not the English ones they replaced.

## Credits and licensing

`slot`, and every change in this fork, is **MIT**. The upstream copyright notice is at the top of
[`LICENSE`](LICENSE) and is untouched; the modifications are the line beneath it. The full
statement of origin, baseline and attribution is in [`NOTICE.md`](NOTICE.md).

Emulation is [mGBA](https://mgba.io) by endrift, and [gpSP](https://github.com/libretro/gpsp) by
Gilead "Exophase" Kutnick, both through [libretro](https://www.libretro.com). slot loads a core
with `dlopen` and never links against or modifies either, so neither licence reaches this
frontend or anything in it. A cart's `System/selected_core.ini` picks between them — gpSP for the
serial link hardware mGBA's libretro build does not carry.

The cores keep their own licences, and they are different: **mGBA's libretro build is MPL-2.0**,
**gpSP's is GPL-2.0**. Their texts are in [`licenses/`](licenses/). This repository's source tree
carries no binaries, so nothing in *it* is conveyed under either licence — but a distributed card
is, and so is the zip in this fork's releases. That is why the release's `System/licenses/` holds
what it holds for gpSP: **GPL-2.0 section 3(a)** wants the corresponding source to travel with the
binary, so it does, as `gpsp-<commit>.tar.gz` in the same directory, with a `.meta` recording how
that commit was resolved and an `OFFER.md` stating plainly what that resolution is and is not worth
— the libretro buildbot does not publish which commit built a given nightly, so the pairing is an
inference from a date, not a proof. mGBA obliges only the notice **MPL-2.0 section 3.1** asks for,
which `licenses/README.md` gives. Upstream's `dist:device` and `deploy:device` tasks assemble the
same directory; a card built by hand and handed to someone has to assemble it too.

The rest of what ships:

- The card's typeface in `System/fonts/` is Noto Sans CJK SC Bold, under the SIL Open Font License,
  which the release zip carries beside it.
- The device boots [AGS-102](https://github.com/BrandonKowalski/AGS-102), a purpose-made fork of
  [BaseOS](https://github.com/pvaibhav/BaseOS) by @pvaibhav.
- Type is [Open Sans](https://github.com/googlefonts/opensans), under the SIL Open Font License, and
  [Nerd Fonts](https://www.nerdfonts.com) symbols by Ryan L. McIntyre, under MIT.
- The panel mask is derived from LCD3x, a public-domain shader by Gigaherz in the libretro shader
  collection. At exactly 3x it reduces to a 3 by 3 table, which is what ships here rather than the
  shader.
- The cart sounds are upstream's own recording.
- The pinyin table's hanzi carry their readings from the data published by the
  [pypinyin](https://github.com/mozillazg/python-pinyin) project (MIT), which is what
  `tools/gen_pinyin.py` regenerates it with.

## AI disclosure

Upstream's frontend was put together with Claude and reviewed by its author. The changes in this
fork were drafted the same way, with an AI assistant, and then reviewed and tested on the device
by [@RetroGame66](https://github.com/RetroGame66) — including the parts that only show up on real
hardware, like the colour grades and the chords. **The prose in this repository is largely
machine-written**, which is a departure from upstream's README and worth saying plainly.

This fork is a personal one, aimed at one library on one card. Issues may not be answered; for
`slot` itself, upstream is the place to go. Upstream's own note about support and PRs is in
[`README.upstream.md`](README.upstream.md).

## Upstream

Everything that is not listed above is **Brandon T. Kowalski's** work, and this fork would not
exist without it. His README — the original documentation, including what is deliberately not in
this file (his release pipeline, his notes on the hardware, his own credits and disclosure) — is
reproduced unmodified at [`README.upstream.md`](README.upstream.md).

If you want `slot` itself, go to <https://github.com/BrandonKowalski/slot>.
