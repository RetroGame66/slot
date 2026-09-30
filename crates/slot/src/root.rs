use std::path::{Path, PathBuf};

use slot_store::System;

use crate::audio::Profile;
use crate::input::Remap;

/// The seven top level folders of a content root. A card that has never held slot. has none
/// of them, and every write path below assumes its own is already there.
pub const DIRS: [&str; 7] = [
    "BIOS",
    "Games",
    "Labels",
    "Saves",
    "States",
    "System",
    "Wallpapers",
];

/// Where a card's own typefaces go. Nested under `System` rather than beside the seven:
/// the folders out there are things the user fills from the outside — ROMs, art, saves — and
/// this one is a system setting, like `theme.txt` and `selected_core.ini` next to it.
const FONT_DIR: &str = "System/fonts";

/// Best effort: an unmounted or read only card is an empty shelf, not a boot failure.
pub fn ensure(root: &Path) {
    for sub in DIRS {
        let _ = std::fs::create_dir_all(root.join(sub));
    }
        let _ = std::fs::create_dir_all(root.join(FONT_DIR));
        // The screen-overlay folder, flat: the user drops `gb.png`, `gb-01.png`, `gbc.png`, …
        // straight into it. See `overlay_files` for the naming. Creating it is all upside — a
        // card with no art in it simply shows the built-in look.
        let _ = std::fs::create_dir_all(root.join("Overlay"));
        // The per-machine save folders are made here rather than left to the core: a core handed
        // a save directory that does not exist writes nothing and says nothing, and the first
        // save of the first GB game is exactly when nobody is looking.
        for system in [System::Gba, System::Gb, System::Gbc] {
            let _ = std::fs::create_dir_all(root.join("Saves").join(system.dir_name()));
        }
    }

/// The card's own typeface, if it carries one. First by name out of `System/fonts`; then the
/// single-file spelling, `System/font.ttf`, for a card that would rather not keep a folder for
/// one file. Absent means the embedded face, which is the whole of the default behaviour — a
/// card that says nothing about type is set in the font slot ships with.
///
/// Sorted first, so which file wins does not depend on the order a directory hands its entries
/// back: one card must pick one face, on every boot.
pub fn font_file(root: &Path) -> Option<PathBuf> {
    let mut files: Vec<PathBuf> = std::fs::read_dir(root.join(FONT_DIR))
        .ok()?
        .flatten()
        .map(|e| e.path())
        .filter(|p| !slot_store::is_hidden(p))
        .filter(|p| is_font(p))
        .collect();
    files.sort();
    if let Some(first) = files.into_iter().next() {
        return Some(first);
    }
    let single = root.join("System/font.ttf");
    single.is_file().then_some(single)
}

/// The panel mask, if the card carries one. Nine RGB triples, one row per line, three
/// values per triple, 0-255: the table the game pass multiplies the picture by. `#` starts a
/// comment and blank lines are ignored, so the file documents itself.
///
/// Absent, unreadable, or the wrong shape means the built-in table, which is the whole of the
/// default behaviour — a card that says nothing about the panel is shown the panel slot ships
/// with. A malformed file is not an error worth reporting on a device with no console.
pub fn panel_mask(root: &Path) -> Option<[[[u8; 3]; 3]; 3]> {
    let text = std::fs::read_to_string(root.join("System/mask.txt")).ok()?;
    let mut rows: Vec<[[u8; 3]; 3]> = Vec::new();
    for line in text.lines() {
        let line = line.split('#').next().unwrap_or("");
        let values: Vec<u8> = line
            .split_whitespace()
            .filter_map(|v| v.parse::<u8>().ok())
            .collect();
        if values.is_empty() {
            continue;
        }
        if values.len() != 9 {
            return None;
        }
        let mut row = [[0u8; 3]; 3];
        for (cell, chunk) in row.iter_mut().zip(values.chunks_exact(3)) {
            cell.copy_from_slice(chunk);
        }
        rows.push(row);
    }
    match rows.len() {
        3 => Some([rows[0], rows[1], rows[2]]),
        _ => None,
    }
}

/// One screen overlay on the card, and whether it asks for the reflection layer.
pub struct OverlayFile {
    pub path: PathBuf,
    /// The file name carried the reflection marker (a trailing `x` on the stem), so the game's
    /// mirrored blur is drawn wherever this overlay's own alpha leaves a hole. See
    /// `overlay_files`.
    pub reflect: bool,
}

/// Every screen-overlay PNG for one system, in rotation order.
///
/// Files live flat in `Overlay/`, named `<sys>.png` (the base) or `<sys>-NN.png` (a numbered
/// variant), compared without case: for plain Game Boy, `gb.png`, `gb-01.png`, `gb-02.png`, …;
/// for Game Boy Color, `gbc.png`, `gbc-01.png`, …. The base sorts first, then the numbered ones
/// by their number, so the rotation runs base, 01, 02, …. An absent or empty `Overlay/` is no
/// overlay at all — the built-in look stands — and a missing file is not an error worth a log
/// line on a device with no console.
///
/// A trailing `x` on the stem — `gbx.png`, `gb-02x.png` — marks a **reflective** overlay: its
/// transparent areas become the zones the screen reflection is drawn in (see
/// `Compositor::set_reflection`). No marker, no reflection; the art works exactly as before.
///
/// `which` is the stem a file must start with, `"gb"` or `"gbc"`. Matching on the following
/// `-` is what keeps `gbc-01.png` out of the plain GB set: it does not begin with `gb-`.
pub fn overlay_files(root: &Path, which: &str) -> Vec<OverlayFile> {
    let lower = which.to_ascii_lowercase();
    let numbered = format!("{lower}-");
    let Ok(dir) = std::fs::read_dir(root.join("Overlay")) else {
        return Vec::new();
    };
    let mut out: Vec<(i64, bool, String, PathBuf)> = Vec::new();
    for entry in dir.flatten() {
        let path = entry.path();
        let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        let name = name.to_ascii_lowercase();
        let Some(stem) = name.strip_suffix(".png") else {
            continue;
        };
        // A trailing `x` is the reflection marker; what remains is the plain name.
        let (rest, reflect) = match stem.strip_suffix('x') {
            Some(r) => (r, true),
            None => (stem, false),
        };
        // Numbered from `00` upward: `gb-00.png` is as valid as `gb-01.png`, and both are as
        // valid as the bare `gb.png`. (Requiring a number greater than zero silently dropped
        // `gb-00x.png` — the name the user actually used — leaving the rotation with nothing.)
        let rank = if rest == lower {
            0
        } else if let Some(digits) = rest.strip_prefix(&numbered) {
            match digits.parse::<i64>() {
                Ok(n) if n >= 0 => n,
                _ => continue,
            }
        } else {
            continue;
        };
        out.push((rank, reflect, name, path));
    }
    // A stable order whatever the filesystem hands back, so the rotation does not depend on
    // directory order. Ties — `gb-01.png` beside `gb-01x.png` — sort by name, deterministically.
    out.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.cmp(&b.1)).then(a.2.cmp(&b.2)));
    out.into_iter()
        .map(|(_, reflect, _, path)| OverlayFile { path, reflect })
        .collect()
}

/// A named overlay: `Overlay/<sys>-<model>x.png`, the screen art drawn for one Game Boy model.
///
/// `overlay_files` cannot see these and that is on purpose — they are not entries in a rotation,
/// they belong to the model whose art they are. The naming is `<sys>-<word>x.png`, matched
/// case-insensitively the same way `overlay_files` lowercases, so a card written on Windows
/// and a card written on the device agree about it.
///
/// `None` when the card has no art for that model, which is the normal case for a card that
/// never drew any: the overlay set is then just whatever ordinary files are there.
pub fn model_overlay(root: &Path, which: &str, model: &str) -> Option<PathBuf> {
    let want = format!(
        "{}-{}x.png",
        which.to_ascii_lowercase(),
        model.to_ascii_lowercase()
    );
    let dir = std::fs::read_dir(root.join("Overlay")).ok()?;
    for entry in dir.flatten() {
        let path = entry.path();
        let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        if name.to_ascii_lowercase() == want {
            return Some(path);
        }
    }
    None
}

/// Which overlay each system is showing, as indices into `overlay_files`. Kept in
/// `System/overlay.txt` as two integers, GB then GBC. A missing or unreadable file reads as
/// zero for both, and the caller clamps any index past the end of a system's list, so a card
/// the user edits without rebooting cannot land out of range.
pub fn overlay_indices(root: &Path) -> [usize; 2] {
    let vals: Vec<usize> = std::fs::read_to_string(root.join("System/overlay.txt"))
        .ok()
        .map(|t| {
            t.split_whitespace()
                .filter_map(|v| v.parse().ok())
                .collect()
        })
        .unwrap_or_default();
    [
        vals.first().copied().unwrap_or(0),
        vals.get(1).copied().unwrap_or(0),
    ]
}

pub fn write_overlay_indices(root: &Path, indices: [usize; 2]) {
    let _ = std::fs::write(
        root.join("System/overlay.txt"),
        format!("{} {}", indices[0], indices[1]),
    );
}

/// Which machine's shelf is up, from `System/shelf.txt`. Absent or misspelt reads as GBA — the
/// machine this device is a frontend for, and the natural default for a card that has not been
/// told otherwise.
pub fn shelf_system(root: &Path) -> System {
    std::fs::read_to_string(root.join("System/shelf.txt"))
        .ok()
        .and_then(|t| t.split_whitespace().next().map(str::to_string))
        .and_then(|w| match w.to_ascii_lowercase().as_str() {
            "gb" => Some(System::Gb),
            "gbc" => Some(System::Gbc),
            "gba" => Some(System::Gba),
            _ => None,
        })
        .unwrap_or(System::Gba)
}

pub fn write_shelf_system(root: &Path, system: System) {
    let _ = std::fs::write(root.join("System/shelf.txt"), system.dir_name());
}

/// The colour-correction matrix, if the card carries one. Nine floats, three rows of three,
/// row-major (output row, input column), 0.0-2.0: the matrix the game pass multiplies the
/// picture by, in the spirit of a colour-saturation shader but applied as a single 3x3 multiply so
/// it costs nothing on this device. `#` starts a comment and blank lines are ignored.
///
/// Absent, unreadable, or the wrong shape means the built-in table (a gentle desaturation),
/// which is the whole of the default behaviour. A malformed file is not an error worth
/// reporting on a device with no console.
pub fn color_correction(root: &Path) -> Option<[[f32; 3]; 3]> {
    let text = std::fs::read_to_string(root.join("System/cc.txt")).ok()?;
    let mut rows: Vec<[f32; 3]> = Vec::new();
    for line in text.lines() {
        let line = line.split('#').next().unwrap_or("");
        let values: Vec<f32> = line
            .split_whitespace()
            .filter_map(|v| v.parse::<f32>().ok())
            .collect();
        if values.is_empty() {
            continue;
        }
        if values.len() != 3 {
            return None;
        }
        rows.push([values[0], values[1], values[2]]);
    }
    match rows.len() {
        3 => Some([rows[0], rows[1], rows[2]]),
        _ => None,
    }
}

/// The display modes, read from `System/display.txt` as "mask_mode cc_gba cc_gb cc_gbc":
/// mask_mode 0..=4 (OFF, LCD3X 50%, LCD3X 100%, SCANLINE 50%, SCANLINE 100%) and one colour
/// choice per machine — 0..=2 for the colour machines (GRAYSCALE, AGB-001, NDS) and the whole
/// palette table for the Game Boy, whose list is palettes rather than saturations. The ranges are
/// the app's to clamp, since it is the app that owns the lists; this only reads non-negative
/// integers.
///
/// The colour machines' three grades: **0 GRAYSCALE** (plain black & white), **1 AGB-001** (the
/// authentic original Game Boy Advance panel — desaturated, faintly green, darker gamma), **2 NDS**
/// (the Nintendo DS Phat screen — a softer, near-sRGB middle ground). A Game Boy is greyscale and
/// keeps its own palette table instead.
///
/// The Game Boy's slot is deliberately **not** clamped here. Its list is hundreds of entries long
/// and grew once already, and a cap written into the file reader would silently send a card back
/// to an early palette the next time it did. Only the app knows how long that table is, so that
/// is where the clamp lives (`DisplayFilter::new`).
///
/// Missing or unparsable means the shipped look: mask on (LCD3X, 2) and the colour machines on
/// AGB-001 (the authentic restore), the Game Boy on its neutral grey. **Two integers is the older
/// file** — "mask cc" from before the colour choice went per machine — and that one value is
/// applied to all three, so a card written by the previous build keeps the look it was set to
/// (remapped onto the new list by `migrate_display_modes`).
/// How many aperture presets `display.txt`'s first number may name: OFF, four strengths of the
/// LCD3x table, the scanline at two.
///
/// **It lives here and not beside the ring it describes**, because this is the file that indexes
/// it: `read` clamps against this number and `DisplayFilter::MASK_STATES` — the count of arms in
/// `applied_mask`, the length `cycle_mask` walks — is the ring agreeing with the card. One number,
/// because the file, the ring and the cycle all have to agree about it.
///
/// It was a literal `4` in the clamp until 2026-09-30 — the count when the ring held five presets.
/// It was never raised when the scanline was added, so a card that saved the scanline (5 or 6) was
/// read back as LCD3x 100% on the next boot: the two newest presets were written, shown, persisted
/// and then silently replaced. `tests/display.rs` now walks the whole ring through the file.
pub const MASK_STATES: usize = 7;

/// The display modes a card asks for: the Advance's aperture preset, then one colour slot per
/// machine — `mask cc_gba cc_gb cc_gbc`.
///
/// Both are read as *indices* rather than as names, so every list has to clamp: an out-of-range
/// `cc` is corrected by `DisplayFilter::new`, and an out-of-range `mask` here.
pub fn display_modes(root: &Path) -> (u8, [u16; 3]) {
    let text = std::fs::read_to_string(root.join("System/display.txt")).ok();
    let vals: Vec<u16> = text
        .map(|t| {
            t.split_whitespace()
                .filter_map(|v| v.parse::<u16>().ok())
                .collect()
        })
        .unwrap_or_default();
    // Clamped against the ring's own count, not a literal: this was `4` — the number when the
    // Advance's ring held five presets, and it never moved when the scanline joined it.
    let mask = vals
        .first()
        .copied()
        .unwrap_or(2)
        .min(MASK_STATES as u16 - 1) as u8;
    let one = vals.get(1).copied().unwrap_or(0);
    let cc = if vals.len() >= 4 {
        [one, vals[2], vals[3]]
    } else {
        // The older two-integer file, or the one-integer one: whatever colour was chosen then
        // was chosen for every machine, and that is still what it means.
        [one, one, one]
    };
    (mask, cc)
}

/// Persist the display modes. Best effort: a read only card simply keeps the default.
pub fn write_display_modes(root: &Path, mask: u8, cc: [u16; 3]) {
    let _ = std::fs::write(
        root.join("System/display.txt"),
        format!("{mask} {} {} {}", cc[0], cc[1], cc[2]),
    );
}

/// Where the Game Boy's palette list stood before the browser, and where those same looks are in
/// `palettes::GB_PALETTES` now.
///
/// The old list was five house looks followed by ten PixelShift picks: 原生灰, DMG 初代绿,
/// DMG 经典绿, GBP 暖白, GBL 青, then PS01/03/05/17/18/24/31/32/40/44. Three of those — the three
/// model palettes — are no longer entries at all: they are the colour half of the screen art they
/// were drawn with (`palettes::GB_MODELS`), reached by choosing the art rather than by picking a
/// colour. A card left holding one of those indices was showing the model it had landed on; the
/// number itself no longer names anything, so it falls back to 初代绿, the one house look that is
/// still a palette in its own right.
const GB_LEGACY: [u16; 15] = [0, 1, 1, 1, 1, 2, 4, 6, 18, 19, 25, 32, 33, 41, 45];

/// Written once `migrate_palette` has run, so it runs exactly once per card.
pub const PALETTE_MARKER: &str = "System/palette.v2";

/// Bring a card written before the palette browser onto the table it has now, so the Game Boy look
/// it was set to is the look it still shows.
///
/// Keyed on a marker file rather than on the value, because the two tables overlap across 0..=14
/// and no reading of a single number can say which table wrote it. Best effort throughout: a card
/// that cannot be written to simply keeps the default, exactly as it would have anyway.
pub fn migrate_palette(root: &Path) {
    let marker = root.join(PALETTE_MARKER);
    if marker.exists() {
        return;
    }
    let (mask, mut cc) = display_modes(root);
    if let Some(mapped) = GB_LEGACY.get(cc[1] as usize) {
        cc[1] = *mapped;
    }
    write_display_modes(root, mask, cc);
    let _ = std::fs::write(&marker, "palette.v2\n");
}

/// Where the colour machines' grade list stood before the rebuild, and where those same looks
/// are in the new three-entry list now.
///
/// The old list (0..=6) was FULLCOLOR, HALFCOLOR, NOCOLOR, DMG green, ice-blue, amber, pink. The
/// rebuild keeps just the three that mean something on a colour machine: 0 GRAYSCALE, 1 AGB-001
/// (the authentic original panel), 2 NDS (the soft near-sRGB middle ground). The single-colour
/// backlights (DMG green / ice-blue / amber / pink) have no equivalent — a colour grade has no
/// one signature tint — so they collapse to GRAYSCALE, the nearest the new list has. FULLCOLOR was
/// the shipped default and the faithful-restore intent, so it lands on AGB-001; HALFCOLOR was
/// already the softened look, so it lands on NDS.
const CC_LEGACY: [u16; 7] = [1, 2, 0, 0, 0, 0, 0];

/// Written once `migrate_display_modes` has run, so it runs exactly once per card.
pub const DISPLAY_MARKER: &str = "System/display.v2";

/// Bring a card written before the colour-grade rebuild onto the new three-entry list, so the
/// look it was set to is still the look it shows.
///
/// Keyed on a marker file (like `migrate_palette`) rather than on the value, because the old and
/// new lists overlap across 0..=2 and no single number can say which list wrote it. Only the
/// colour machines are remapped — the Game Boy's slot (1) is its own palette table and is left
/// untouched by this. Best effort throughout: a card that cannot be written to simply keeps the
/// default, exactly as it would have anyway.
pub fn migrate_display_modes(root: &Path) {
    let marker = root.join(DISPLAY_MARKER);
    if marker.exists() {
        return;
    }
    let (mask, mut cc) = display_modes(root);
    for slot in [0usize, 2usize] {
        // GB slot (1) is a palette index, not a grade; skip it.
        if let Some(mapped) = CC_LEGACY.get(cc[slot] as usize) {
            cc[slot] = *mapped;
        }
    }
    write_display_modes(root, mask, cc);
    let _ = std::fs::write(&marker, "display.v2\n");
}

/// The audio profile, read from `System/audio.txt`: `stable`, `balanced` or `strict`.
///
/// This is a latency dial, not a quality one. The GBA's own 32768 Hz already carries
/// everything the console can produce, so what is being traded here is how much audio sits
/// queued between the emulator and the speaker — which is exactly the offset a rhythm game
/// judges you against. `stable` is the shipped behaviour; a missing, blank or misspelled file
/// reads as `stable`, so a card edited on a PC cannot make the machine click by typo.
pub fn audio_profile(root: &Path) -> Profile {
    std::fs::read_to_string(root.join("System/audio.txt"))
        .ok()
        .and_then(|t| t.lines().find_map(Profile::parse))
        .unwrap_or_default()
}

/// Persist the chosen profile. Best effort, like the display modes: a read only card keeps the
/// profile it booted with.
pub fn write_audio_profile(root: &Path, profile: Profile) {
    let _ = std::fs::write(root.join("System/audio.txt"), profile.as_str());
}

/// One cheat code for a cart. `code` is the raw code fed to the core verbatim; `desc` is the
/// human label written after a `#` on the same line, used only for the on-device list — mGBA
/// never sees it.
pub struct CheatEntry {
    pub code: String,
    pub desc: String,
}

/// Cheat codes for a cart, if the card carries any. One code per line in
/// `System/Cheats/<stem>.txt`; `#` starts a comment and blank lines are ignored. A multi-line
/// GameShark / Action Replay code is joined with `+` on a single line, which is mGBA's libretro
/// cheat format. Everything after the first `#` on a line is kept as the code's `desc` (the
/// on-device label) and never reaches the core. The codes are fed to the core verbatim; whether
/// a given format works is mGBA's call. Absent or unreadable means no cheats for this cart —
/// best effort, a device with no console is not the place to surface a missing file.
pub fn cheats(root: &Path, stem: &str) -> Vec<CheatEntry> {
    let path = root
        .join("System")
        .join("Cheats")
        .join(format!("{stem}.txt"));
    let Ok(text) = std::fs::read_to_string(&path) else {
        return Vec::new();
    };
    text.lines()
        .filter_map(|l| {
            let (code, desc) = match l.split_once('#') {
                Some((c, d)) => (c.trim(), d.trim()),
                None => (l.trim(), ""),
            };
            if code.is_empty() {
                return None;
            }
            Some(CheatEntry {
                code: code.to_string(),
                desc: desc.to_string(),
            })
        })
        .collect()
}

/// The button remap, if the card carries one. One redirection per line, `PHYSICAL=GBA`, so
/// `X=A` sends the X button to the core as GBA A. `#` starts a comment and blank lines are
/// ignored. Names are the button letters: up/down/left/right/a/b/x/y/l1/r1/l2/r2/start/select
/// (and a few that reach no GBA button: menu/volup/voldown/power/lid). The remap only touches
/// what reaches the core, so a `X=A` line leaves X doing its usual job in the menus (undo, etc.)
/// — those listen for the raw action, not for the bit this builds.
///
/// Absent or unreadable means no remap, which is identity — every button reports as itself. A
/// malformed line is skipped, not fatal: a device with no console is not the place to surface a
/// typo in a config file.
pub fn remap(root: &Path) -> Option<Remap> {
    let text = std::fs::read_to_string(root.join("System/remap.txt")).ok()?;
    let mut r = Remap::identity();
    for line in text.lines() {
        let line = line.split('#').next().unwrap_or("").trim();
        if line.is_empty() {
            continue;
        }
        let (phys, tgt) = line.split_once('=')?;
        let (phys, tgt) = (phys.trim(), tgt.trim());
        let (Some(phys), Some(tgt)) = (btn_by_name(phys), btn_by_name(tgt)) else {
            continue;
        };
        r.set(phys, tgt);
    }
    Some(r)
}

/// Case-insensitive button name to `Btn`. Only the names a config file would spell are accepted;
/// anything else returns `None` and is skipped by `remap`.
fn btn_by_name(name: &str) -> Option<slot_input::Btn> {
    use slot_input::Btn;
    Some(match name.to_ascii_uppercase().as_str() {
        "UP" => Btn::Up,
        "DOWN" => Btn::Down,
        "LEFT" => Btn::Left,
        "RIGHT" => Btn::Right,
        "A" => Btn::A,
        "B" => Btn::B,
        "X" => Btn::X,
        "Y" => Btn::Y,
        "L1" => Btn::L1,
        "R1" => Btn::R1,
        "L2" => Btn::L2,
        "R2" => Btn::R2,
        "START" => Btn::Start,
        "SELECT" => Btn::Select,
        "MENU" => Btn::Menu,
        "VOLUP" => Btn::VolUp,
        "VOLDOWN" => Btn::VolDown,
        "POWER" => Btn::Power,
        "LID" => Btn::Lid,
        _ => return None,
    })
}

/// TTC as well as TTF and OTF: `text::set_font` reads face 0, and several of the common
/// Chinese faces a desktop already has are collections rather than single faces.
fn is_font(p: &Path) -> bool {
    p.extension().is_some_and(|e| {
        e.eq_ignore_ascii_case("ttf")
            || e.eq_ignore_ascii_case("otf")
            || e.eq_ignore_ascii_case("ttc")
    })
}

/// Bring a card written before states were namespaced up to the current layout. Best
/// effort on purpose: a read only or half mounted card is an empty shelf, not a boot
/// failure, exactly as `ensure` treats it.
///
/// A per-entry failure does not stop the sweep — the rest of the shelf still gets a chance —
/// but silently eating every one of them would leave a cart stuck pre-migration forever with
/// nothing on the card to say so. Logged here, once per boot, rather than inside
/// `migrate_states` itself, which only counts and has no read on where "once per boot" ends.
pub fn migrate(root: &Path) {
    match slot_store::migrate_states(root) {
        Ok(report) if report.failed > 0 => {
            eprintln!(
                "slot: migrate: {} of {} state director{} did not move",
                report.failed,
                report.moved + report.failed,
                if report.moved + report.failed == 1 {
                    "y"
                } else {
                    "ies"
                }
            );
        }
        _ => {}
    }
}

/// Reported to the core as the libretro system directory. `gba_bios.bin` present means the
/// real BIOS, absent means mGBA's HLE BIOS. Neither is an error.
pub fn bios_dir(root: &Path) -> PathBuf {
    root.join("BIOS")
}

/// The battery saves, under the machine's own folder. A GB `Tetris` and a GBA `Tetris` share a
/// stem, so on a flat card they also share one `.sav`; sorting by machine is what keeps each
/// game's bytes its own. `bios_dir` is the one folder that does **not** sort: a bios is the
/// machine, not the game, so it stays shared.
pub fn saves_dir(root: &Path, system: System) -> PathBuf {
    root.join("Saves").join(system.dir_name())
}
