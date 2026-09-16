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