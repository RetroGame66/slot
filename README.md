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

## About this fork

A modified build of `slot`, aimed at a Chinese GBA library living on one SD card. **75 files
changed, +5,929 / −385**, across eleven feature areas.

Everything structural — boot, the launcher and core picker, emulator plumbing, the libretro cores,
saves, rewind, the link features — is upstream's and is left alone. What follows is what this fork
puts on top of it. Full detail, feature by feature and file by file, is in
[`CHANGES.md`](CHANGES.md); origin and licensing in [`NOTICE.md`](NOTICE.md).

### Added

| What | Why |
|---|---|
| **Chinese interface** | The UI is Chinese throughout. The typeface is read off the card at boot rather than baked into the binary, so swapping fonts no longer needs a rebuild — and most of the font payload leaves the binary. |
| **Pinyin letter ring** | The shelf gains a letter ring grouped by pinyin initial. A Chinese library's file names are hanzi, which carry no alphabetical order of their own. |
| **Cheat codes** | A per-cart cheat list on the card, browsable and toggled on the device (`SELECT` + `A`). No PC in the loop. |
| **Audio latency profiles** | `stable` / `balanced` / `strict` — one knob for the gap between the beat you hear and the press the game judges. |
| **Button remapping** | `System/remap.txt`, per card, so a hand used to a different layout can keep it. |
| **GB / GBC cartridge art** | Cartridge artwork drawn for GB and GBC carts. **Assets only — not wired to anything yet.** |
| **Card-side settings** | `display.txt`, `cc.txt`, `mask.txt`, `audio.txt`, `remap.txt`, `Cheats/`, `fonts/` — settings that travel with the card instead of with the binary. |
| **Build scripts** | `check_glibc.py`, `verify_slot.py`, `make_zip.py`, `tools/gen_pinyin.py`. |

### Changed

| What | Why |
|---|---|
| **Colour correction in linear space** | Multiplying in the encoded (gamma) space darkens a half-colour and washes a monochrome backlight out. The correction now runs in linear space, which is what saturation and monochrome actually need. |
| **Shelf performance** | A cart face costs about 70 ms to build, and a boot used to build every cart before the first frame. Faces are now built off the frame loop against a bounded texture pool: the shelf opens in the time seven faces take, whatever the card holds. |
| **Backlight in twenty steps, not ten** | Ten was too coarse at the bottom — step 1 was still bright enough to read by in a dark room. |
| **Cart geometry** | The selected cart sits at 1.5× with its neighbours half off-screen, so the row reads as continuing past the selection rather than as three equal carts. |
| **Backlight peak colours remapped** | DMG green, ice blue, amber and pink — retuned for the linear-space correction above. |
| **Documentation** | The control table was wrong: it said `SELECT` + `X` opened a chooser, when it cycles the display preset. That is fixed, and the chords this fork adds (`X` / `Y` / `A` / `VOL±`) are listed. |

### Not changed

Upstream's own work is left as it stands. **All comments in this fork are English**, in the prose
style the tree already uses. Chinese remains only where it is *data* the program parses and
compares against — the interface's own translated strings, the hanzi the pinyin table is built
from, and the `[中]` / `[日]` / `[英]` tags a dumped file name may carry. Those are values, not
remarks; translating them would break the code.

<details>
<summary><b>中文简介</b>（点开）</summary>

这是 `slot` 的修改版，面向**单张 SD 卡上的中文 GBA 游戏库**。改了 **75 个文件，+5,929 / −385**，
覆盖十一个功能域。启动、启动器、模拟器管线、libretro 核心、存档、回退、联机这些底层部分是
上游的，没有动。

**新增**：中文界面（字体从卡上读，不用重编）；拼音首字母索引环；每卡的金手指列表（机上可浏览、
`SELECT` + `A` 开关）；音频延迟档（`stable` / `balanced` / `strict`）；按键重映射；GB/GBC 卡带美术
（**仅素材，还没接上**）；随卡走的设置文件；几个构建脚本。

**改动**：色彩校正在**线性空间**做（在编码空间相乘会把半色调压暗、把单色背光冲淡）；卡带封面
改在帧循环外构建并用有界纹理池（原先开机要把所有卡带建完，每张约 70 ms）；背光 10 档改 20 档；
选中卡带放大 1.5 倍、相邻半出屏；四种背光峰值色重调；修正 README 里写错的控制表。

逐条说明见 [`CHANGES.md`](CHANGES.md)，来源与许可见 [`NOTICE.md`](NOTICE.md)。

</details>

## Controls

### Anywhere

| Input                       | Action                                |
|-----------------------------|---------------------------------------|
| `SELECT` + `Up` / `Down`    | Adjust brightness                     |
| `SELECT` + `Left` / `Right` | Adjust blue light                     |
| `SELECT` + `X`              | Cycle the display preset (panel mask) |
| `SELECT` + `Y`              | Cycle the colour correction           |
| `SELECT` + `A`              | Turn the cart's cheat codes on or off |
| `SELECT` + `VOL+` / `VOL-`  | Step the audio profile                |
| `VOL+` / `VOL-`             | Change the volume                     |
| `VOL+` + `VOL-`             | Mute, remembering the level           |

### On the carousel

| Input     | Action                              |
|-----------|-------------------------------------|
| `L` / `R` | Browse the carousel                 |
| Tap `A`   | Resume the last save state          |
| Hold `A`  | Start the game fresh                |
| `MENU`    | Open the about screen               |
| `START`   | Choose which emulator runs the cart |

### In game

| Input                     | Action                                                                                           |
|---------------------------|--------------------------------------------------------------------------------------------------|
| Hold `MENU`               | Save state, eject the cart, back to the carousel                                                 |
| Double tap `MENU`         | Save state switcher: pick one to load or delete, or undo the last save or load within 30 seconds |
| `SELECT` + `MENU`         | Link with another RG SP. gpSP carts only                                                         |
| `SELECT` + `R1`           | Save state                                                                                       |
| `SELECT` + `L1`           | Load the most recent save state                                                                  |
| Hold `L2`                 | Rewind                                                                                           |
| Hold `R2`                 | Fast-forward                                                                                     |
| Double tap `R2`           | Lock fast-forward on. Press again to unlock                                                      |

A `/` means either one. A `+` means both together.

Closing the lid writes a save state and turns off the display. Open it again and you're
back in the game. Leave it shut for three minutes and slot powers off, resuming from that
save state on the next boot.

The lid is not a sleep. The panel goes dark but the board keeps running, which is why the
three minutes exist rather than an indefinite standby.

## SD Card Layout

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

Label art is drawn at 196x86, or about 2.28:1. Anything else is scaled to cover that box
and centre cropped, so a square or portrait image loses its top and bottom. Bigger art is
fine and comes down to size; smaller gets stretched up and shows it.

`System/theme.txt` is entirely optional and controls the appearance of the slot:

```
housing #24242a
recess  #1a1a1e
opening #050508
edge    #4d4d57
```

`System/selected_core.ini` is entirely optional and names which core a cart's save states
belong to, one `<rom stem> = <core>` per line. Every cart defaults to mGBA, and states are
kept apart per core under `States/<core>/<rom stem>/` so switching cores later never mixes
one core's save with another's. Both cores ship in `System/`, so naming `gpsp` actually
switches emulators for that cart — gpSP exists for the serial link hardware mGBA's libretro
build does not carry:

```
Emerald = gpsp
```

## Installing on your RG SP

1. Download the latest [AGS-102](https://github.com/BrandonKowalski/AGS-102) `.img` release.
2. Use Raspberry PI Imager, RUFUS, et. al. to write the `.img` to an SD Card.
3. Insert this SD Card into Slot 1 of your RG SP. This is the one on the side of the device next to the volume buttons.
4. Download the latest slot release from this repo.
5. Unzip the download
6. Copy all the contents of the zip to a second SD Card
7. Add Games, Saves, BIOS (if you like the boot animation), etc.
8. Insert this SD Card into Slot 2. This is on the side where the power and reset buttons live.

## Updating
I doubt I am gonna work on this more and add to it but in case I do here is how you update.

1. Power off your RG SP.
2. Eject SD Card 2.
3. Connect to your computer.
4. Replace the `System` folder with the `System` folder contained in the update zip.
5. Done.


## Credits

Emulation is [mGBA](https://mgba.io) by endrift, and [gpSP](https://github.com/libretro/gpsp)
by Gilead "Exophase" Kutnick — a cart's `System/selected_core.ini` picks between them, gpSP
for the serial link hardware mGBA's libretro build does not carry — both through
[libretro](https://www.libretro.com). The release ships both cores' compiled libretro
binaries unmodified: mGBA's under MPL-2.0, gpSP's under GPL-2.0. Their license texts, and
gpSP's own corresponding source (fetched at build time and shipped alongside the binary, per
GPL-2.0 section 3(a)), are in [`licenses/`](licenses/), which `dist:device` copies into the
shipped tree alongside the cores they cover.

The device boots [AGS-102](https://github.com/BrandonKowalski/AGS-102), a purpose-made fork
of [BaseOS](https://github.com/pvaibhav/BaseOS) by @pvaibhav.

Type is [Open Sans](https://github.com/googlefonts/opensans), under the SIL Open Font
License, and [Nerd Fonts](https://www.nerdfonts.com) symbols by Ryan L. McIntyre, under MIT.

The panel mask is derived from LCD3x, a public-domain shader by Gigaherz in the libretro
shader collection. At exactly 3x it reduces to a 3 by 3 table, which is what ships here
rather than the shader.

The cart sounds are a recording of me shoving a cartridge into my childhood GBA.

## AI Disclosure

The Rust frontend was put together by Claude Opus. I reviewed everything that was
produced. This documentation is 100% free-range, meatbag prose.

The project is extremely low stakes. I wanted a bespoke frontend for my RG SP and thought
that something this focused on GBA would be kind of neat.

This is just a glorified wrapper around mGBA, which is the real star of the show.

Provided without support. I will selectively address filed issues and PRs.

Use it, don't use it, I don't care. Figured I should share the end result of all the
wasted water.