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

/// Where a key's name ends. Far enough in that the longest chord ends before it, so the card
/// reads as two columns rather than as twenty different gaps.
const KEY_COL: u32 = 300;
/// Between the two columns.
const COL_GAP: u32 = 26;

/// A row's height, and so the distance between two of them: room for the line's own air and
/// still twenty-odd rows to the card.
const ROW_H: u32 = 30;
/// A head is taller, and that is the whole of what makes it a head. It is set in the same
/// voice as the rows — a bolder one would make the card two designs — it simply has more room.
const HEAD_H: u32 = 38;

const KEY_PX: f32 = 15.0;
const DESC_PX: f32 = 14.0;
const HEAD_PX: f32 = 18.0;

/// The key's ink, and the quieter one the description is set in: the eye finds the button first
/// and reads what it does second. Both carry their own halo, because the card is drawn over a
/// photograph and nothing on it has a plate of its own to sit on.
const KEY_INK: [u8; 3] = [0xf4, 0xf1, 0xec];
const DESC_INK: [u8; 3] = [0xc6, 0xc2, 0xba];
const HEAD_INK: [u8; 3] = [0xf6, 0xf4, 0xef];

/// One line of the card.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Row {
    /// The name of a screen the keys below belong to.
    Head(&'static str),
    /// What a key is called, and what it does.
    Key(&'static str, &'static str),
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
pub const ROWS: [Row; 22] = [
    Row::Head("货架界面"),
    Row::Key("方向键 左 / 右", "浏览卡带"),
    Row::Key("方向键 上 / 下", "上一个 / 下一个字母"),
    Row::Key("L / R", "同上，哪一套定位都可以"),
    Row::Key("轻点 A", "从最近的即时存档继续"),
    Row::Key("长按 A", "把这个游戏从头开始"),
    Row::Key("START", "为这张卡挑一个模拟器"),
    Row::Key("MENU", "关于页与快捷键说明"),
    Row::Key("SELECT + START", "切换字母轮 / 侧边栏"),
    Row::Key("SELECT + X / Y", "面板遮罩 / 色彩校正"),
    Row::Key("SELECT + 音量 ±", "音频延迟档位"),
    Row::Key("SELECT + 上 / 下", "亮度（任何界面都可调）"),
    Row::Key("SELECT + 左 / 右", "蓝光滤镜（任何界面都可调）"),
    Row::Head("游戏中"),
    Row::Key("长按 MENU", "存档、弹出卡带、回货架"),
    Row::Key("双击 MENU", "即时存档切换器"),
    Row::Key("SELECT + MENU", "与另一台 RG SP 联机"),
    Row::Key("SELECT + R1 / L1", "存档 / 读取最近的存档"),
    Row::Key("SELECT + A", "这张卡的金手指码表"),
    Row::Key("长按 L2 / R2", "倒带 / 快进"),
    Row::Key("双击 R2", "锁定快进，再按一次取消"),
    Row::Key("音量 + / -", "音量；两个一起按静音"),
];

/// The line that stays put at the foot of the card while the rows move under it. It is the one
/// thing on the screen that can be read without scrolling, so it is the one that says there is
/// more below.
pub const HINT: &str = "上下键翻页    B 或 MENU 返回";

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
                let f = haloed(&cov, w, ch, HEAD_INK);
                blit(&mut rgba, h, &f.rgba, f.w, f.h, 0.0);
            }
        }
        Row::Key(key, desc) => {
            // Right aligned on the column, so the keys end in a straight edge and what they do
            // starts in one: two columns, read down one and across to the other.
            let (cov, w, ch) = line(key, KEY_PX);
            if w > 0 {
                let f = haloed(&cov, w, ch, KEY_INK);
                blit(
                    &mut rgba,
                    h,
                    &f.rgba,
                    f.w,
                    f.h,
                    KEY_COL as f32 - w as f32 - HALO_PX as f32,
                );
            }
            let (cov, w, ch) = line(desc, DESC_PX);
            if w > 0 {
                let f = haloed(&cov, w, ch, DESC_INK);
                blit(
                    &mut rgba,
                    h,
                    &f.rgba,
                    f.w,
                    f.h,
                    (KEY_COL + COL_GAP) as f32 - HALO_PX as f32,
                );
            }
        }
    }
    UndoFace { rgba, w: ROW_W, h }
}

/// The foot line, centred in a row of its own.
pub fn hint_face() -> UndoFace {
    let mut rgba = vec![0u8; (ROW_W * ROW_H * 4) as usize];
    let (cov, w, ch) = line(HINT, DESC_PX);
    if w > 0 {
        let f = haloed(&cov, w, ch, DESC_INK);
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
        assert!(Row::Head("游戏中").height() > Row::Key("A", "选择").height());
    }

    #[test]
    fn the_key_column_ends_where_the_description_starts() {
        // Two columns and not a ragged list: the longest key in the card has to stop before the
        // gap, or one row out of twenty-two pushes its description out of line with the rest.
        let mut widest: f32 = 0.0;
        for row in ROWS {
            if let Row::Key(key, _) = row {
                let (_, w, _) = line(key, KEY_PX);
                widest = widest.max(w as f32);
            }
        }
        assert!(
            widest <= KEY_COL as f32 - HALO_PX as f32,
            "widest key is {widest} across, and the column is {KEY_COL}"
        );
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
