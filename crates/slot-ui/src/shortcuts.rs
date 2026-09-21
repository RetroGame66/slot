//! The shortcut card: what every key does, on the two screens that have keys to press.
//!
//! It hangs under the about screen's label and is scrolled by the same arrows that move
//! everything else. It is there because the list outgrew a hand: chords were added to a machine
//! whose every button was already spoken for, and a shortcut the user cannot find again is a
//! shortcut they were never given.
//!
//! Rasterised at boot with the rest of the fixed furniture — the rows never change, and opening
//! a help screen is the worst moment to be asking a font for twenty lines.

use crate::draw::{Draw, TexId, OUT_W};
use crate::icon::{haloed, HALO_PX};
use crate::plate::UndoFace;
use crate::text;

/// How wide a row is. The sticker's width, because they are one card: the label at the top and
/// the list under it are read together, and a list narrower than the label above it looks like
/// a second thing that failed to line up.
pub const ROW_W: u32 = 660;

/// Where a key's name begins. Left-aligned, so the caps form a clean left edge and the eye
/// reads down the buttons rather than hunting for where each one starts. The name sits `BOX_PAD_X`
/// inside its cap, which is what puts the cap's own left edge at `KEY_X - BOX_PAD_X`.
const KEY_X: u32 = 30;
/// Where the description begins. Also left-aligned, and the same column for every row, so the
/// two columns read as two lists rather than as twenty different gaps.
const DESC_X: u32 = 320;

/// A row's height, and so the distance between two of them: room for the line's own air and
/// still twenty-odd rows to the card. Tall enough for a 24px key inside its cap.
const ROW_H: u32 = 64;
/// A head is taller, and that is the whole of what makes it a head. It is set in the same
/// voice as the rows — a bolder one would make the card two designs — it simply has more room.
/// Taller than `ROW_H` and not merely taller than a line of its own type: the head is what
/// divides the card into sections, and a head no taller than the rows under it divides nothing.
const HEAD_H: u32 = 72;

const KEY_PX: f32 = 24.0;
const DESC_PX: f32 = 24.0;
const HEAD_PX: f32 = 26.0;
/// The foot line is a hair smaller than the rows: it is furniture, not a shortcut.
const HINT_PX: f32 = 20.0;

/// The keycap drawn round each key: a faint body and a light outline, the same shape a manual
/// uses for a button. `BOX_PAD_X` is the air between the ink and the outline; `BOX_MARGIN_Y` the
/// air between the outline and the row's top and bottom; `BOX_RADIUS` the corner; `BOX_THICK` the
/// outline's weight.
const BOX_PAD_X: f32 = 10.0;
const BOX_MARGIN_Y: f32 = 6.0;
const BOX_RADIUS: f32 = 10.0;
const BOX_THICK: f32 = 2.0;
/// The body's wash, and how solid it is: a translucent dark panel in both modes, because a
/// keycap is a recess pressed into the card whichever way round the card is printed.
const KEYCAP_FILL: [u8; 3] = [0x14, 0x13, 0x12];
const KEYCAP_FILL_A: u8 = 38;

/// The outline's ink. Darker than the card in *both* modes rather than lighter, which reads
/// backwards until you see what the card is: it is a light panel in the dark mode and a light
/// panel in the light one, so the cap that has to separate from it does not flip with the
/// palette the way the type does.
fn keycap_ink() -> [u8; 3] {
    crate::palette::keycap()
}

/// The key's ink, and the quieter one the description is set in: the eye finds the button first
/// and reads what it does second. Both carry their own halo, because the card is drawn over a
/// photograph and nothing on it has a plate of its own to sit on.
///
/// The description is the ink stepped towards the ground rather than a fraction of it, so
/// "quieter" survives the palette going light — see `palette::dim_ink`.
fn key_ink() -> [u8; 3] {
    crate::palette::ink()
}

fn desc_ink() -> [u8; 3] {
    crate::palette::dim_ink()
}

fn head_ink() -> [u8; 3] {
    crate::palette::ink()
}

/// The smallest a keycap gets. A single-letter key ("A", "L") is a key like any other and is
/// drawn as one, not as a sliver of a box.
const MIN_BTN_W: f32 = 46.0;
/// Between two pieces of a key line: a cap, a `+`, another cap.
const SEG_GAP: f32 = 8.0;

/// One piece of a key line. A chord is several buttons with the signs between them, and the card
/// has to tell the two apart: the buttons carry the caps, the signs do not.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Seg {
    /// A key: drawn in a rounded rectangle of its own. One rectangle, one button.
    Btn(&'static str),
    /// Plain text — a chord's `+` or `/`, or the word that says *how* a button is used ("hold",
    /// "tap") — none of which is itself a key.
    Sep(&'static str),
}

/// One line of the card.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Row {
    /// The name of a screen the keys below belong to.
    Head(&'static str),
    /// The keys in order, and what they do.
    Key(&'static [Seg], &'static str),
}

impl Row {
    fn height(self) -> u32 {
        match self {
            Row::Head(_) => HEAD_H,
            Row::Key(..) => ROW_H,
        }
    }
}

/// The card, in reading order.
///
/// Two screens and nothing else: a shortcut only has to be found on the screen it is pressed
/// on. The handful that answer everywhere say so in their own line rather than earning a third
/// section of their own.
///
/// **Two tables, chosen at compile time** — the same shape as `lang`, and for the same reason:
/// one source, two builds, nothing to keep in step by hand. It is not in `lang` because a row is
/// not a word. A row is a key line and a description that have to fit a fixed column, so the
/// wording is a layout decision as much as a translation, and the two belong together.
///
/// The descriptions are written to a budget rather than merely translated, and the budget is a
/// number rather than a feel: the column is `DESC_X`..`ROW_W` — 340 pixels, 290 for a key line —
/// and `fit` shrinks whatever will not fit. An over-long row is therefore not a failure; it is one
/// row set smaller than the other seventeen, which is worse, because nothing reports it. Chinese
/// says these in four to six characters and uses half the column; the English below was shortened
/// until it measured inside it, which is why several read terser than the Chinese they answer.
/// Whoever edits either table should measure again — the widest line here is 337 of 340.
#[cfg(not(feature = "lang-en"))]
pub const ROWS: [Row; 20] = [
    Row::Head("货架界面"),
    Row::Key(
        &[
            Seg::Sep("方向键"),
            Seg::Btn("左"),
            Seg::Sep("/"),
            Seg::Btn("右"),
        ],
        "浏览卡带",
    ),
    Row::Key(
        &[Seg::Btn("L"), Seg::Sep("/"), Seg::Btn("R")],
        "上一个 / 下一个字母",
    ),
    Row::Key(&[Seg::Sep("轻点"), Seg::Btn("A")], "从最近的即时存档继续"),
    Row::Key(&[Seg::Sep("长按"), Seg::Btn("A")], "把这个游戏从头开始"),
    Row::Key(&[Seg::Btn("START")], "为这张卡挑一个模拟器"),
    Row::Key(&[Seg::Btn("MENU")], "关于页与快捷键说明"),
    Row::Key(
        &[
            Seg::Btn("SELECT"),
            Seg::Sep("+"),
            Seg::Btn("X"),
            Seg::Sep("/"),
            Seg::Btn("Y"),
        ],
        "面板遮罩 / 色彩校正",
    ),
    Row::Key(
        &[Seg::Btn("SELECT"), Seg::Sep("+"), Seg::Btn("音量 ±")],
        "音频延迟档位",
    ),
    Row::Key(
        &[
            Seg::Btn("SELECT"),
            Seg::Sep("+"),
            Seg::Btn("上"),
            Seg::Sep("/"),
            Seg::Btn("下"),
        ],
        "亮度（任何界面都可调）",
    ),
    Row::Key(
        &[
            Seg::Btn("SELECT"),
            Seg::Sep("+"),
            Seg::Btn("左"),
            Seg::Sep("/"),
            Seg::Btn("右"),
        ],
        "蓝光滤镜（任何界面都可调）",
    ),
    Row::Head("游戏中"),
    Row::Key(
        &[Seg::Sep("长按"), Seg::Btn("MENU")],
        "存档、弹出卡带、回货架",
    ),
    Row::Key(&[Seg::Sep("双击"), Seg::Btn("MENU")], "即时存档切换器"),
    Row::Key(
        &[Seg::Btn("SELECT"), Seg::Sep("+"), Seg::Btn("MENU")],
        "与另一台 RG SP 联机",
    ),
    Row::Key(
        &[
            Seg::Btn("SELECT"),
            Seg::Sep("+"),
            Seg::Btn("R1"),
            Seg::Sep("/"),
            Seg::Btn("L1"),
        ],
        "存档 / 读取最近的存档",
    ),
    Row::Key(
        &[Seg::Btn("SELECT"), Seg::Sep("+"), Seg::Btn("A")],
        "这张卡的金手指码表",
    ),
    Row::Key(
        &[
            Seg::Sep("长按"),
            Seg::Btn("L2"),
            Seg::Sep("/"),
            Seg::Btn("R2"),
        ],
        "倒带 / 快进",
    ),
    Row::Key(
        &[Seg::Sep("双击"), Seg::Btn("R2")],
        "锁定快进，再按一次取消",
    ),
    Row::Key(
        &[
            Seg::Sep("音量"),
            Seg::Btn("+"),
            Seg::Sep("/"),
            Seg::Btn("-"),
        ],
        "音量；两个一起按静音",
    ),
];

/// The same card in English. Its own table rather than a second arm at each row, because a table
/// is what it is: the order, the key lines and the counts are identical, and only the words move.
/// Measured against the column budget above, not eyeballed — see the note on the Chinese table.
#[cfg(feature = "lang-en")]
pub const ROWS: [Row; 20] = [
    Row::Head("On the shelf"),
    Row::Key(&[Seg::Btn("D-PAD"), Seg::Sep("L/R")], "Browse carts"),
    // `L1`/`R1` rather than the Chinese card's `L / R`: the two names would collide with the
    // d-pad's left and right two rows down, and the code's own button is `Btn::L1`.
    Row::Key(
        &[Seg::Btn("L1"), Seg::Sep("/"), Seg::Btn("R1")],
        "Prev / next letter",
    ),
    Row::Key(&[Seg::Sep("Tap"), Seg::Btn("A")], "Resume last save"),
    Row::Key(&[Seg::Sep("Hold"), Seg::Btn("A")], "Restart the game"),
    Row::Key(&[Seg::Btn("START")], "Pick the core"),
    Row::Key(&[Seg::Btn("MENU")], "About & shortcuts"),
    Row::Key(
        &[
            Seg::Btn("SELECT"),
            Seg::Sep("+"),
            Seg::Btn("X"),
            Seg::Sep("/"),
            Seg::Btn("Y"),
        ],
        "Mask / colour",
    ),
    Row::Key(
        &[Seg::Btn("SELECT"), Seg::Sep("+"), Seg::Btn("VOL ±")],
        "Audio latency",
    ),
    Row::Key(
        &[Seg::Btn("SELECT"), Seg::Sep("+"), Seg::Btn("UP/DN")],
        "Brightness (any)",
    ),
    Row::Key(
        &[Seg::Btn("SELECT"), Seg::Sep("+"), Seg::Btn("L/R")],
        "Blue light (any)",
    ),
    Row::Head("In a game"),
    Row::Key(&[Seg::Sep("Hold"), Seg::Btn("MENU")], "Save, eject, back"),
    Row::Key(
        &[Seg::Sep("Double-tap"), Seg::Btn("MENU")],
        "Save-state list",
    ),
    Row::Key(
        &[Seg::Btn("SELECT"), Seg::Sep("+"), Seg::Btn("MENU")],
        "Link two RG SPs",
    ),
    Row::Key(
        &[
            Seg::Btn("SELECT"),
            Seg::Sep("+"),
            Seg::Btn("R1"),
            Seg::Sep("/"),
            Seg::Btn("L1"),
        ],
        "Save / load latest",
    ),
    Row::Key(
        &[Seg::Btn("SELECT"), Seg::Sep("+"), Seg::Btn("A")],
        "Cheat list",
    ),
    Row::Key(
        &[
            Seg::Sep("Hold"),
            Seg::Btn("L2"),
            Seg::Sep("/"),
            Seg::Btn("R2"),
        ],
        "Rewind / fast fwd",
    ),
    Row::Key(
        &[Seg::Sep("Double-tap"), Seg::Btn("R2")],
        "Lock fast forward",
    ),
    Row::Key(
        &[
            Seg::Sep("Volume"),
            Seg::Btn("+"),
            Seg::Sep("/"),
            Seg::Btn("-"),
        ],
        "Volume; both mute",
    ),
];

/// The line that stays put at the foot of the card while the rows move under it. It is the one
/// thing on the screen that can be read without scrolling, so it is the one that says there is
/// more below.
#[cfg(not(feature = "lang-en"))]
pub const HINT: &str = "上下键翻页    B 或 MENU 返回";

/// The same foot line in English. Wide enough at `HINT_PX` for the card's own width, which the
/// Chinese line is not close to using.
#[cfg(feature = "lang-en")]
pub const HINT: &str = "Up / Down to scroll      B or MENU to go back";

/// One row, its own width and its own height, haloed and inked. `HEAD_PX` for a head, the two
/// column inks for a key — and nothing but the columns, so the card has no chrome of its own to
/// keep in step with the label above it.
pub fn row_face(row: Row) -> UndoFace {
    let h = row.height();
    let mut rgba = vec![0u8; (ROW_W * h * 4) as usize];
    match row {
        Row::Head(s) => {
            let (cov, w, ch) = line(s, HEAD_PX);
            if w > 0 {
                let f = haloed(&cov, w, ch, head_ink());
                blit(&mut rgba, h, &f.rgba, f.w, f.h, 0.0);
            }
            // A hairline under the head, so the two sections read as chapters of a guide.
            draw_hairline(&mut rgba, ROW_W, h as f32 - 5.0);
        }
        Row::Key(segs, desc) => {
            // The key line, laid out piece by piece. Every button gets a keycap of its own — the
            // shape a manual draws for a key — and the pieces that are not buttons are set as
            // plain text between them, so a chord reads as `[A] + [B]` and never as one long box
            // with the sign caught inside it. Left-aligned from `KEY_X`, so the buttons start on
            // one edge and the eye reads across them.
            let mut x = KEY_X as f32;
            for seg in segs {
                match *seg {
                    Seg::Btn(t) => {
                        let (cov, w, ch) = line(t, KEY_PX);
                        if w == 0 {
                            continue;
                        }
                        // The cap is the key's own width plus its padding, and never narrower
                        // than a key: a lone "A" is still a button.
                        let box_w = (w as f32 + 2.0 * BOX_PAD_X).max(MIN_BTN_W);
                        let box_h = h as f32 - 2.0 * BOX_MARGIN_Y;
                        draw_keycap(&mut rgba, ROW_W, h, x, BOX_MARGIN_Y, box_w, box_h);
                        // Centred in the cap, whatever the cap's extra width is.
                        let f = haloed(&cov, w, ch, key_ink());
                        blit(
                            &mut rgba,
                            h,
                            &f.rgba,
                            f.w,
                            f.h,
                            x + (box_w - w as f32) / 2.0 - HALO_PX as f32,
                        );
                        x += box_w + SEG_GAP;
                    }
                    Seg::Sep(t) => {
                        let (cov, w, ch) = line(t, KEY_PX);
                        if w == 0 {
                            continue;
                        }
                        let f = haloed(&cov, w, ch, key_ink());
                        blit(&mut rgba, h, &f.rgba, f.w, f.h, x - HALO_PX as f32);
                        x += w as f32 + SEG_GAP;
                    }
                }
            }
            let (cov, w, ch) = line(desc, DESC_PX);
            if w > 0 {
                let f = haloed(&cov, w, ch, desc_ink());
                blit(
                    &mut rgba,
                    h,
                    &f.rgba,
                    f.w,
                    f.h,
                    DESC_X as f32 - HALO_PX as f32,
                );
            }
        }
    }
    UndoFace { rgba, w: ROW_W, h }
}

/// The foot line, centred in a row of its own. The bar it is set on belongs to the panel, not
/// to the card, so the app draws that; this is only the words.
pub fn hint_face() -> UndoFace {
    let mut rgba = vec![0u8; (ROW_W * ROW_H * 4) as usize];
    let (cov, w, ch) = line(HINT, HINT_PX);
    if w > 0 {
        let f = haloed(&cov, w, ch, desc_ink());
        blit(
            &mut rgba,
            ROW_H,
            &f.rgba,
            f.w,
            f.h,
            (ROW_W - f.w) as f32 / 2.0 - HALO_PX as f32,
        );
    }
    UndoFace {
        rgba,
        w: ROW_W,
        h: ROW_H,
    }
}

/// Where a row of this width goes on the panel.
pub fn row_x(w: u32) -> f32 {
    (OUT_W as f32 - w as f32) / 2.0
}

/// One row on the panel, at a height the caller has already taken the scroll off.
pub fn draw_row(face: Option<(TexId, u32, u32)>, y: f32, out: &mut Vec<Draw>) {
    let Some((tex, w, h)) = face else {
        return;
    };
    out.push(Draw::Tex {
        x: row_x(w),
        y,
        w: w as f32,
        h: h as f32,
        tex,
        alpha: 1.0,
    });
}

/// The coverage of one line, in a box cut to the line's own width: `coverage` centres what it is
/// given, so a box no wider than the ink puts the pen at zero and the caller can place it.
fn line(s: &str, px: f32) -> (Vec<u8>, u32, u32) {
    let Some(font) = text::label_font() else {
        return (Vec::new(), 0, 0);
    };
    let layout = text::fit(font, s, f32::MAX, 1, px, px);
    let w = layout
        .lines
        .first()
        .map(|l| text::line_width(font, l, px, layout.tracking))
        .unwrap_or(0.0)
        .ceil() as u32;
    if w == 0 {
        return (Vec::new(), 0, 0);
    }
    let h = (px * 1.6).ceil() as u32;
    (text::coverage(w, h, &layout), w, h)
}

/// Lay a strip into a row, centred in it. The row is empty underneath, so source-over collapses
/// to a copy.
fn blit(dst: &mut [u8], dst_h: u32, src: &[u8], sw: u32, sh: u32, x: f32) {
    let y = (dst_h.saturating_sub(sh) / 2) as i32;
    let x0 = x.round() as i32;
    for row in 0..sh {
        let dy = y + row as i32;
        if dy < 0 || dy >= dst_h as i32 {
            continue;
        }
        for col in 0..sw {
            let dx = x0 + col as i32;
            if dx < 0 || dx >= ROW_W as i32 {
                continue;
            }
            let s = ((row * sw + col) * 4) as usize;
            if src[s + 3] == 0 {
                continue;
            }
            let d = ((dy as u32 * ROW_W + dx as u32) * 4) as usize;
            dst[d..d + 4].copy_from_slice(&src[s..s + 4]);
        }
    }
}

/// Signed distance to a rounded rectangle centred at `(cx, cy)` with half-extents `hw`, `hh`
/// and corner radius `r`: negative inside, zero on the centreline, positive outside. Used to
/// carve the keycap's body and outline out of the row's own buffer.
fn round_rect_sdf(px: f32, py: f32, cx: f32, cy: f32, hw: f32, hh: f32, r: f32) -> f32 {
    let dx = (px - cx).abs() - (hw - r);
    let dy = (py - cy).abs() - (hh - r);
    let ox = dx.max(0.0);
    let oy = dy.max(0.0);
    (ox * ox + oy * oy).sqrt() + dx.min(0.0).max(dy.min(0.0))
}

/// One keycap: a faint body filling the rounded rect, then a light outline on its edge. Drawn
/// into the row's own buffer, so it travels with the row and needs no chrome of its own.
fn draw_keycap(dst: &mut [u8], dst_w: u32, dst_h: u32, x: f32, y: f32, w: f32, h: f32) {
    let x0 = x.floor();
    let y0 = y.floor();
    let bw = w.ceil() as i32;
    let bh = h.ceil() as i32;
    let cx = x + w / 2.0;
    let cy = y + h / 2.0;
    let hw = w / 2.0;
    let hh = h / 2.0;
    for row in 0..bh {
        let py = y0 + row as f32 + 0.5;
        let dy = py.round() as i32;
        if dy < 0 || dy >= dst_h as i32 {
            continue;
        }
        for col in 0..bw {
            let px = x0 + col as f32 + 0.5;
            let dx = px.round() as i32;
            if dx < 0 || dx >= dst_w as i32 {
                continue;
            }
            let d = round_rect_sdf(px, py, cx, cy, hw, hh, BOX_RADIUS);
            let di = ((dy as u32 * dst_w + dx as u32) * 4) as usize;
            if d <= 0.0 {
                dst[di..di + 3].copy_from_slice(&KEYCAP_FILL);
                dst[di + 3] = dst[di + 3].max(KEYCAP_FILL_A);
            }
            if d.abs() <= BOX_THICK / 2.0 {
                dst[di..di + 3].copy_from_slice(&keycap_ink());
                dst[di + 3] = 255;
            }
        }
    }
}

/// A one-pixel rule across the row, for the hairline under a head.
fn draw_hairline(dst: &mut [u8], dst_w: u32, y: f32) {
    let dy = y.round() as i32;
    if dy < 0 || dy >= dst_w as i32 {
        return;
    }
    for dx in 0..dst_w as i32 {
        let di = ((dy as u32 * dst_w + dx as u32) * 4) as usize;
        dst[di..di + 3].copy_from_slice(&keycap_ink());
        dst[di + 3] = dst[di + 3].max(170);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_row_fits_its_own_width() {
        // The card is one column of a fixed width and the sticker above it is the same width, so
        // a row that ran past it would not be a row, it would be an overflow nobody clips.
        for row in ROWS {
            let f = row_face(row);
            assert_eq!(f.w, ROW_W, "{row:?} is not the card's width");
            assert_eq!(f.h, row.height(), "{row:?} is not its own height");
            assert_eq!(f.rgba.len(), (ROW_W * f.h * 4) as usize, "{row:?}");
        }
    }

    #[test]
    fn a_head_is_taller_than_a_key() {
        // The only thing that distinguishes them. A head is not set in a different voice, so if
        // it were also the same height, the card would have no sections.
        assert!(Row::Head("游戏中").height() > Row::Key(&[Seg::Btn("A")], "选择").height());
    }

    #[test]
    fn a_description_stays_inside_the_card() {
        // The key line is not the only column with a wall at the end of it. The description is
        // laid out by `line`, which asks `fit` for `f32::MAX` width — so too long is not shrunk
        // and not wrapped, it is drawn straight past the row and clipped by the row's own width.
        // A sentence cut mid-word is worse than a shorter one, and nothing measured this until
        // the English table was written to the Chinese card's proportions and came out twenty
        // characters wide against a fifteen-character column.
        let mut edge: f32 = 0.0;
        for row in ROWS {
            if let Row::Key(_, desc) = row {
                let (_, w, _) = line(desc, DESC_PX);
                // `haloed` dilates the type by `HALO_PX` a side and the blit subtracts it once, so
                // the ink's right edge is the column's start plus the width plus one halo.
                edge = edge.max(DESC_X as f32 + w as f32 + HALO_PX as f32);
            }
        }
        assert!(
            edge <= ROW_W as f32,
            "the widest description reaches {edge}, and the card ends at {ROW_W}"
        );
    }

    #[test]
    fn the_keycaps_end_where_the_description_starts() {
        // Two left-aligned columns and not a ragged list: the widest key line — the last cap's
        // outline included, since that is the furthest thing in the column — has to end before
        // the description column begins, or one row out of twenty-two writes over the next.
        let mut right: f32 = 0.0;
        for row in ROWS {
            if let Row::Key(segs, _) = row {
                let mut x = KEY_X as f32;
                for seg in segs {
                    let text = match *seg {
                        Seg::Btn(t) | Seg::Sep(t) => t,
                    };
                    let (_, w, _) = line(text, KEY_PX);
                    let adv = match *seg {
                        Seg::Btn(_) => (w as f32 + 2.0 * BOX_PAD_X).max(MIN_BTN_W),
                        Seg::Sep(_) => w as f32,
                    };
                    right = right.max(x + adv);
                    x += adv + SEG_GAP;
                }
            }
        }
        assert!(
            right <= DESC_X as f32 - HALO_PX as f32,
            "the widest key line reaches {right}, and the description starts at {DESC_X}"
        );
    }

    #[test]
    fn a_chord_is_buttons_and_signs_not_one_box() {
        // The whole point of the split: a chord has to read as several caps with the signs
        // between them. So no cap may hold a *chord* — a `+` or a `/` with words around it —
        // which is the shape the old single-box rows had. A lone `+` or `-` is itself a key
        // (the volume rocker's ends), so a sign with no words beside it is a button and stays.
        for row in ROWS {
            if let Row::Key(segs, _) = row {
                let btns = segs.iter().filter(|s| matches!(s, Seg::Btn(_))).count();
                assert!(btns >= 1, "{segs:?} has no button at all");
                for seg in segs {
                    if let Seg::Btn(t) = seg {
                        assert!(
                            !t.contains(" + ") && !t.contains(" / "),
                            "a chord is caught inside one cap: {t:?}"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn no_description_runs_past_the_card() {
        // The card is the sticker's width and cannot grow; a description longer than the room
        // left of it would just be dropped at the panel edge with nothing to clip it.
        for row in ROWS {
            if let Row::Key(_, desc) = row {
                let (_, w, _) = line(desc, DESC_PX);
                let right = DESC_X as f32 + w as f32;
                assert!(
                    right <= ROW_W as f32,
                    "{desc:?} reaches {right} across a {ROW_W} card"
                );
            }
        }
    }

    #[test]
    fn the_card_starts_on_a_head_and_holds_two_sections() {
        // Every key has to belong to a screen it is pressed on. A card that opened with a key
        // would be a list of twenty-two keys with nothing saying where any of them work.
        assert!(matches!(ROWS[0], Row::Head(_)));
        let heads = ROWS.iter().filter(|r| matches!(r, Row::Head(_))).count();
        assert_eq!(heads, 2, "the shelf and the game, and nothing else");
    }
}
