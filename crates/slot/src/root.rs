use std::path::{Path, PathBuf};

use crate::app::LetterNav;
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

/// The two display modes, read from `System/display.txt` as two integers "mask_mode cc_mode",
/// each clamped to its valid range: mask_mode to 0..=4 (OFF, LCD3X 50%, LCD3X 100%, SCANLINE 50%,
/// SCANLINE 100%) and cc_mode to 0..=6 (FULLCOLOR, HALFCOLOR, NOCOLOR, DMG green, ice-blue, amber,
/// pink backlight). Missing or unparsable means the shipped look: mask on (LCD3X, 2) and colour
/// correction off (0). A single integer is read as the mask mode with colour off. Read once at
/// boot and thereafter owned by the running app, which writes both back out with
/// `write_display_modes` whenever SELECT+X or SELECT+Y cycles one of them.
pub fn display_modes(root: &Path) -> (u8, u8) {
    let text = std::fs::read_to_string(root.join("System/display.txt")).ok();
    let vals: Vec<u8> = text
        .map(|t| {
            t.split_whitespace()
                .filter_map(|v| v.parse::<u8>().ok())
                .collect()
        })
        .unwrap_or_default();
    let mask = vals.first().copied().unwrap_or(2).min(4);
    let cc = vals.get(1).copied().unwrap_or(0).min(6);
    (mask, cc)
}

/// Persist the two display modes. Best effort: a read only card simply keeps the default.
pub fn write_display_modes(root: &Path, mask: u8, cc: u8) {
    let _ = std::fs::write(root.join("System/display.txt"), format!("{mask} {cc}"));
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

/// Which of the shelf's two letter dials is standing, read from `System/letternav.txt`:
/// `wheel` (laid across the panel above the row, what shipped) or `sidebar` (the same dial on
/// its end, down the right-hand edge).
///
/// Missing, blank or misspelled reads as `wheel` — a card edited on a PC should not be able to
/// leave the shelf with no dial at all on it, which is what an unrecognised word would mean if
/// it were an error.
pub fn letter_nav(root: &Path) -> LetterNav {
    std::fs::read_to_string(root.join("System/letternav.txt"))
        .ok()
        .and_then(|t| LetterNav::parse(&t))
        .unwrap_or_default()
}

/// Persist the standing dial. Best effort, as the rest are: a read only card keeps whichever
/// one it booted with.
pub fn write_letter_nav(root: &Path, nav: LetterNav) {
    let _ = std::fs::write(root.join("System/letternav.txt"), nav.as_str());
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

pub fn saves_dir(root: &Path) -> PathBuf {
    root.join("Saves")
}
