use std::path::{Path, PathBuf};

use crate::atomic::atomic_write;

/// Twenty steps, and so nineteen of them lit: step 0 is the panel off, which is what the lid
/// closes with. Ten was too coarse at the bottom — step 1 sat well above what a dark room
/// wants, with nothing between it and black — so the ramp is finer rather than the top higher.
pub const BRIGHTNESS_MAX: u8 = 19;
pub const BLUE_LIGHT_MAX: u8 = 9;
pub const VOLUME_MAX: u8 = 100;

/// What a real zone can be, in minutes. The card keeps UTC because the base system's clock
/// and its ntp both assume it; this is the only thing that turns it into the time on the
/// shelf. Minutes rather than hours: several zones are offset by thirty and forty five.
pub const UTC_OFFSET_MIN: i16 = -720;
pub const UTC_OFFSET_MAX: i16 = 840;

/// Which way round the frontend prints itself: light type on dark plastic, or dark type on
/// light plastic.
///
/// It lives here rather than in the ui because it is a setting the card remembers, like the
/// brightness and the volume, and the ui reads its palette off whatever is in the state file.
/// A `Mode` owned by the drawing code would have to be mirrored here anyway, and two enums for
/// one line of text is how a file and a screen come apart.
#[derive(Copy, Clone, PartialEq, Eq, Default, Debug)]
pub enum Mode {
    #[default]
    Dark,
    Light,
}

impl Mode {
    /// The other one. What SELECT+START does, and the whole of what it does: there is no third
    /// mode to step through, so a toggle is the honest shape of the control.
    pub fn other(self) -> Mode {
        match self {
            Mode::Dark => Mode::Light,
            Mode::Light => Mode::Dark,
        }
    }

    /// What the state file calls it. Spelled at the definition rather than at either end, so
    /// the reader and the writer cannot disagree about the word.
    pub fn word(self) -> &'static str {
        match self {
            Mode::Dark => "dark",
            Mode::Light => "light",
        }
    }

    /// Anything else is not a mode. A state file edited on a desktop must not be able to put
    /// the device into a third one.
    pub fn from_word(word: &str) -> Option<Mode> {
        match word.trim().to_ascii_lowercase().as_str() {
            "dark" => Some(Mode::Dark),
            "light" => Some(Mode::Light),
            _ => None,
        }
    }
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct SlotState {
    /// Filename stem. `None` is an empty slot, which is the shelf.
    pub cart: Option<String>,
    pub brightness: u8,
    pub blue_light: u8,
    pub volume: u8,
    /// Silence on top of the level rather than instead of it, so unmuting gives back the
    /// number the user last chose.
    pub muted: bool,
    /// Whether anyone has ever confirmed the wall clock. The marker for slot's own first
    /// launch, and the one field a fresh card must read as false.
    pub clock_set: bool,
    /// Minutes to add to the card's UTC to get local time. Zero is a device that never left
    /// Greenwich, which is also what a card that has never been asked reads as.
    pub utc_offset_min: i16,
    /// Light or dark. The one field whose absence is not an error — see `parse`.
    pub mode: Mode,
}

/// Not derived. `read_slot_state` falls back here on a first boot, and all zeroes would
/// be a device with the backlight off and the mixer muted. The brightness default is the
/// middle of the ramp about to be written, not five of it: on a twenty step scale the old
/// five would come up at a quarter, and a card that has never been touched should look like
/// a card someone set at a sensible level.
impl Default for SlotState {
    fn default() -> Self {
        SlotState {
            cart: None,
            brightness: 10,
            blue_light: 0,
            volume: 60,
            muted: false,
            clock_set: false,
            utc_offset_min: 0,
            mode: Mode::Dark,
        }
    }
}

fn state_path(root: &Path) -> PathBuf {
    root.join("System").join("slot.state")
}

pub fn read_slot_state(root: &Path) -> SlotState {
    std::fs::read(state_path(root))
        .ok()
        .and_then(|b| String::from_utf8(b).ok())
        .and_then(|s| parse(&s))
        .unwrap_or_default()
}

pub fn write_slot_state(root: &Path, s: &SlotState) -> std::io::Result<()> {
    let text = format!(
        "cart={}\nbrightness={}\nblue_light={}\nvolume={}\nmuted={}\nclock_set={}\nutc_offset_min={}\nmode={}\n",
        s.cart.as_deref().unwrap_or(""),
        s.brightness,
        s.blue_light,
        s.volume,
        s.muted as u8,
        s.clock_set as u8,
        s.utc_offset_min,
        s.mode.word()
    );
    atomic_write(&state_path(root), text.as_bytes())
}

/// All or nothing. A file we only half recognise is not one we wrote, and inheriting the
/// missing fields from the defaults would hide the corruption behind plausible values.
///
/// `mode` is the one exception, and deliberately: every card in the field has a state file
/// written before the mode existed, and under the rule above the first boot of a binary that
/// knows about it would throw the whole file away — the cart the user left selected, the
/// brightness they set, the timezone they confirmed. Strictness here exists to catch a file
/// that has been corrupted, not to punish one that is merely older, and a missing mode has
/// exactly one sensible reading: dark, which is what the device did before it could be
/// anything else. Every *other* key is still all-or-nothing, including one this build has
/// never heard of.
fn parse(text: &str) -> Option<SlotState> {
    let mut cart = None;
    let mut brightness = None;
    let mut blue_light = None;
    let mut volume = None;
    let mut muted = None;
    let mut clock_set = None;
    let mut utc_offset_min = None;
    let mut mode = None;
    for line in text.lines().filter(|l| !l.is_empty()) {
        let (key, value) = line.split_once('=')?;
        match key {
            "cart" => cart = Some(value.to_string()),
            "brightness" => brightness = Some(level(value, BRIGHTNESS_MAX)?),
            "blue_light" => blue_light = Some(level(value, BLUE_LIGHT_MAX)?),
            "volume" => volume = Some(level(value, VOLUME_MAX)?),
            "muted" => muted = Some(level(value, 1)? == 1),
            "clock_set" => clock_set = Some(level(value, 1)? == 1),
            "utc_offset_min" => utc_offset_min = Some(offset(value)?),
            // Written by this build and unreadable in it is still corruption: a misspelt word
            // here means someone has been editing the file, and the defaults are the safe
            // answer for the rest of it too.
            "mode" => mode = Some(Mode::from_word(value)?),
            _ => return None,
        }
    }
    let cart = cart?;
    Some(SlotState {
        cart: (!cart.is_empty()).then_some(cart),
        brightness: brightness?,
        blue_light: blue_light?,
        volume: volume?,
        muted: muted?,
        clock_set: clock_set?,
        utc_offset_min: utc_offset_min?,
        mode: mode.unwrap_or_default(),
    })
}

fn offset(value: &str) -> Option<i16> {
    value
        .parse()
        .ok()
        .filter(|n| (UTC_OFFSET_MIN..=UTC_OFFSET_MAX).contains(n))
}

fn level(value: &str, max: u8) -> Option<u8> {
    value.parse().ok().filter(|n| *n <= max)
}
