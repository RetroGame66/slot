use crate::plate::UndoFace;
use crate::text;

/// What a held POWER offers. Restart, because a device you develop on wants one that is not
/// "off, then find the button again" — and off, which is the only other thing this hardware
/// can honestly do.
///
/// There is no Standby. The board suspends well, under 45 mA, but it cannot wake itself: the
/// RTC alarm arms, reads back, and never fires — measured on a fully awake machine as well as
/// a suspended one, and unrelated to Super Standby, which was the first two things I blamed.
/// A standby nothing can end is a slow leak with a nicer name, so the lid and the button run
/// a timer and then power off properly instead.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum PowerChoice {
    Restart,
    PowerOff,
}

impl PowerChoice {
    /// Restart first: it is the one that costs nothing to pick by mistake.
    pub const ALL: [PowerChoice; 2] = [PowerChoice::Restart, PowerChoice::PowerOff];

    /// Position in `ALL`, which is the order the faces are uploaded in.
    pub fn index(self) -> usize {
        self as usize
    }

    pub fn text(self) -> &'static str {
        match self {
            PowerChoice::Restart => "重启",
            PowerChoice::PowerOff => "关机",
        }
    }
}

/// The menu is read at arm's length on a 720x480 panel while the user is deciding something
/// they cannot undo, so it is set well above the key-caption type the rest of the chrome
/// uses. The shutdown line that follows a choice is rastered at the same size: the words
/// change but the voice should not.
const MENU_PX: f32 = 30.0;
const MENU_MIN_PX: f32 = 18.0;
const MENU_H: u32 = 40;
/// Breathing room either side of the ink, which is also what the highlight bar is padded by
/// so the bar hugs the words rather than the panel.
pub const MENU_PAD: u32 = 18;
const MENU_INK: [u8; 3] = [0xf6, 0xf4, 0xef];

/// Sized to its own text rather than to a fixed box, so a caller can put a bar behind it
/// that fits the words. A fixed width would make the bar the same size under "重启" and
/// "关机", which is the thing that looks wrong when the selection moves.
pub fn menu_face(label: &str) -> UndoFace {
    let Some(font) = text::label_font() else {
        return UndoFace {
            rgba: Vec::new(),
            w: 0,
            h: 0,
        };
    };
    let ink = text::line_width(font, label, MENU_PX, 0.0).ceil() as u32;
    let w = ink + 2 * MENU_PAD;
    let mut rgba = vec![0u8; (w * MENU_H * 4) as usize];
    let layout = text::fit(font, label, w as f32, 1, MENU_PX, MENU_MIN_PX);
    text::draw_centred(&mut rgba, w, MENU_H, &layout, MENU_INK);
    UndoFace { rgba, w, h: MENU_H }
}

/// A cheat code row in the cheat table. Smaller than a menu line because codes are short and
/// there are often many of them on one panel; the same auto-width sizing as `menu_face` so the
/// ON/OFF chip drawn beside it hugs the words. `desc` is the human label from the cheat file
/// (the text after `#`); `code` is the raw code. When `desc` is empty the code stands alone on
/// one line, as before.
const CHEAT_PX: f32 = 16.0;
const CHEAT_MIN_PX: f32 = 11.0;
const CHEAT_H: u32 = 22;
const CHEAT_PAD: u32 = 8;
/// The description and code sizes for the two-line layout; the code sits beneath the description
/// in a dimmer ink so the label reads first and the code second.
const CHEAT_DESC_PX: f32 = 16.0;
const CHEAT_CODE_PX: f32 = 11.0;
const CHEAT_CODE_INK: [u8; 3] = [0x9a, 0x98, 0x92];

pub fn cheat_row_face(desc: &str, code: &str) -> UndoFace {
    let Some(font) = text::label_font() else {
        return UndoFace {
            rgba: Vec::new(),
            w: 0,
            h: 0,
        };
    };
    let track = |px: f32| (px * 0.10).round();
    let desc = desc.trim();
    let has_desc = !desc.is_empty();

    let desc_w = if has_desc {
        text::line_width(font, desc, CHEAT_DESC_PX, track(CHEAT_DESC_PX)).ceil() as u32
    } else {
        0
    };
    let code_w = text::line_width(font, code, CHEAT_CODE_PX, track(CHEAT_CODE_PX)).ceil() as u32;
    let ink = desc_w.max(code_w);
    let w = ink + 2 * CHEAT_PAD;

    if has_desc {
        // Two lines: description above, raw code beneath.
        let desc_band: u32 = 24;
        let code_band: u32 = 16;
        let h = desc_band + code_band;
        let mut rgba = vec![0u8; (w * h * 4) as usize];
        let desc_layout = text::fit(font, desc, w as f32, 1, CHEAT_DESC_PX, CHEAT_DESC_PX * 0.7);
        let mut top = vec![0u8; (w * desc_band * 4) as usize];
        text::draw_centred(&mut top, w, desc_band, &desc_layout, MENU_INK);
        blit_band(&mut rgba, w, &top, 0);
        let code_layout = text::fit(font, code, w as f32, 1, CHEAT_CODE_PX, CHEAT_CODE_PX * 0.7);
        let mut bot = vec![0u8; (w * code_band * 4) as usize];
        text::draw_centred(&mut bot, w, code_band, &code_layout, CHEAT_CODE_INK);
        blit_band(&mut rgba, w, &bot, desc_band);
        UndoFace { rgba, w, h }
    } else {
        // No description: the code alone, exactly as the old single-line row did.
        let mut rgba = vec![0u8; (w * CHEAT_H * 4) as usize];
        let layout = text::fit(font, code, w as f32, 1, CHEAT_PX, CHEAT_MIN_PX);
        text::draw_centred(&mut rgba, w, CHEAT_H, &layout, MENU_INK);
        UndoFace {
            rgba,
            w,
            h: CHEAT_H,
        }
    }
}

/// Copy a transparent-backed text band into `dst` starting at row `y0`. The bands never overlap
/// and the destination is transparent there, so source-over collapses to a straight copy.
fn blit_band(dst: &mut [u8], dst_w: u32, src: &[u8], y0: u32) {
    let row_w = dst_w as usize;
    let rows = src.len() / 4 / row_w;
    for row in 0..rows {
        let dy = y0 as usize + row;
        let s = row * row_w * 4;
        let d = dy * row_w * 4;
        for col in 0..row_w {
            let si = s + col * 4;
            if src[si + 3] == 0 {
                continue;
            }
            dst[d + col * 4..d + col * 4 + 4].copy_from_slice(&src[si..si + 4]);
        }
    }
}
