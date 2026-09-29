//! The palette browser: three swatches across, five down, over a paused game.
//!
//! Opened by a hold of the palette key and dismissed by B. The panel is the whole of the
//! screen's furniture while it is up — the game is still behind it, dimmed, because it is
//! paused rather than gone — and it carries exactly two readings: which page of the table is
//! up, and the name of the one entry under the caret. Fifteen names would be fifteen lines of
//! type competing with fifteen blocks of colour, which is why only the highlighted one is set.
//!
//! The colours are drawn as plain rectangles rather than as textures. A swatch is four flat
//! bands of a colour already known at compile time; rasterising them would cost a texture each
//! for a picture the draw list can state outright.

use slot_gfx::{Draw, TexId, OUT_H, OUT_W};
use slot_store::pages;

use crate::palette;
use crate::slot_chrome::{edge, opening};

/// Three across and five down, which is fifteen palettes to a page.
pub const COLS: usize = 3;
pub const ROWS: usize = 5;
pub const PER_PAGE: usize = COLS * ROWS;

/// The four shades a palette is made of — the machine's whole colour story, so a cell is four
/// bands wide and one band thin.
pub const BANDS: usize = 4;

/// The panel is inset from the screen's own edges and padded again inside them; the cells take
/// whatever is left over. Nothing here is a cell width that could disagree with the panel: the
/// grid is centred by construction, which is what the first cut got wrong — a grid centred on
/// its own furniture read as sitting high on the panel.
const PANEL_X: f32 = 34.0;
const PANEL_PAD: f32 = 14.0;
const PANEL_W: f32 = OUT_W as f32 - 2.0 * PANEL_X;
/// The rim around the panel: the one element that does not depend on what is behind it, so the
/// panel reads as an object over a bright game as well as over a dark one.
const RIM: f32 = 2.0;

const CELL_GAP: f32 = 18.0;
const ROW_GAP: f32 = 8.0;
const CELL_H: f32 = 42.0;
/// How thick the ring around the highlighted cell is, and the air between it and the swatch.
///
/// The air is the part that took a round of testing to arrive at. A ring drawn straight onto the
/// cell has its inner edge swallowed by whatever the cell's outermost band is — and four of these
/// palettes are near-white (`原生灰`'s paper is `#FFFFFF`), so on those the caret looked like a
/// cell that was merely bigger, not one that was chosen. One pixel of panel between the two
/// separates them on every palette there is.
///
/// The colour is the theme's **ink**, the same one the name under the grid is set in, rather than
/// the mid-grey `edge()` this started as: a grey ring is close to half the palettes in the table
/// and to the panel itself. `ink` flips with the mode, so the ring stays a ring on a light panel
/// too — a hardcoded white would vanish into it.
const FRAME: f32 = 3.0;
const FRAME_GAP: f32 = 1.0;

/// The page pips, and the band they sit in above the grid. Twenty-four of them is 330 px of the
/// 624 the panel has inside its padding, and the table is a compile-time constant — so this
/// cannot outgrow its row. `draw_pips` still checks rather than trusting that.
const PIP: f32 = 8.0;
const PIP_GAP: f32 = 6.0;
const PIP_H: f32 = 10.0;
const PIP_DIM: f32 = 0.30;
const PIP_GAP_Y: f32 = 12.0;

/// The band the highlighted entry's name is set in, under the grid.
const NAME_H: f32 = 36.0;
const NAME_GAP_Y: f32 = 12.0;
/// The size the name is rasterised at. Twice this is the band, so the font's own line box fits
/// rather than being clipped — see `plate::dialog_line_face`.
pub const NAME_PX: f32 = 18.0;

/// How much of the paused game is left showing through: enough to see there is one, not enough
/// for it to compete with fifteen rectangles of colour.
const VEIL: f32 = 0.62;

fn cell_w() -> f32 {
    (PANEL_W - 2.0 * PANEL_PAD - (COLS as f32 - 1.0) * CELL_GAP) / COLS as f32
}

fn grid_h() -> f32 {
    ROWS as f32 * CELL_H + (ROWS as f32 - 1.0) * ROW_GAP
}

fn panel_h() -> f32 {
    PANEL_PAD + PIP_H + PIP_GAP_Y + grid_h() + NAME_GAP_Y + NAME_H + PANEL_PAD
}

/// The panel's top edge. The whole panel is centred on the screen rather than the grid, which
/// is the difference the user asked for: the grid sat high because the panel around it did not
/// count for anything.
pub fn panel_y() -> f32 {
    (OUT_H as f32 - panel_h()) / 2.0
}

fn grid_y() -> f32 {
    panel_y() + PANEL_PAD + PIP_H + PIP_GAP_Y
}

fn cell_x(col: usize) -> f32 {
    PANEL_X + PANEL_PAD + col as f32 * (cell_w() + CELL_GAP)
}

fn cell_y(row: usize) -> f32 {
    grid_y() + row as f32 * (CELL_H + ROW_GAP)
}

/// Everything the panel needs, gathered by the app in one go.
pub struct View<'a> {
    /// Which page of the table is up, and which cell on it carries the caret.
    pub page: usize,
    pub cursor: usize,
    /// How many pages the table fills, for the pips.
    pub pages: usize,
    /// The four shades of each entry **on this page**, in table order.
    pub shades: &'a [[[u8; 3]; 4]],
    /// The highlighted entry's name, already rasterised. `None` on the frame before the
    /// frontend has minted it, which is an absent line rather than a grey bar.
    pub name: Option<(TexId, u32, u32)>,
}

pub fn draw(v: &View, out: &mut Vec<Draw>) {
    out.push(Draw::Rect {
        x: 0.0,
        y: 0.0,
        w: OUT_W as f32,
        h: OUT_H as f32,
        colour: [0.0, 0.0, 0.0, VEIL],
    });
    let (px, py) = (PANEL_X, panel_y());
    out.push(Draw::Rect {
        x: px - RIM,
        y: py - RIM,
        w: PANEL_W + 2.0 * RIM,
        h: panel_h() + 2.0 * RIM,
        colour: edge(),
    });
    out.push(Draw::Rect {
        x: px,
        y: py,
        w: PANEL_W,
        h: panel_h(),
        colour: opening(),
    });
    draw_pips(v, out);

    let (cw, band) = (cell_w(), cell_w() / BANDS as f32);
    for (i, shades) in v.shades.iter().enumerate() {
        let (x, y) = (cell_x(i % COLS), cell_y(i / COLS));
        // The caret first, so the bands sit inside it and the highlight is a frame rather than a
        // shift of the colours under it — the swatch has to show the palette, not the caret. Two
        // rects, because the ring and the swatch must not touch: the ink, then the panel punched
        // back out of it, and the bands cover the middle of that. See `FRAME_GAP`.
        if i == v.cursor {
            let reach = FRAME + FRAME_GAP;
            out.push(Draw::Rect {
                x: x - reach,
                y: y - reach,
                w: cw + 2.0 * reach,
                h: CELL_H + 2.0 * reach,
                colour: palette::ink_f(),
            });
            out.push(Draw::Rect {
                x: x - FRAME_GAP,
                y: y - FRAME_GAP,
                w: cw + 2.0 * FRAME_GAP,
                h: CELL_H + 2.0 * FRAME_GAP,
                colour: opening(),
            });
        }
        for (b, c) in shades.iter().enumerate() {
            out.push(Draw::Rect {
                x: x + b as f32 * band,
                y,
                w: band,
                h: CELL_H,
                colour: [
                    c[0] as f32 / 255.0,
                    c[1] as f32 / 255.0,
                    c[2] as f32 / 255.0,
                    1.0,
                ],
            });
        }
    }

    if let Some((tex, w, h)) = v.name {
        out.push(Draw::Tex {
            x: px + (PANEL_W - w as f32) / 2.0,
            y: grid_y() + grid_h() + NAME_GAP_Y + (NAME_H - h as f32) / 2.0,
            w: w as f32,
            h: h as f32,
            tex,
            alpha: 1.0,
        });
    }
}

/// One pip a page, the one that is up lit. Silent when the row would not fit, which cannot
/// happen for the table this was written for but would for a longer one.
fn draw_pips(v: &View, out: &mut Vec<Draw>) {
    if v.pages < 2 {
        return;
    }
    let row = v.pages as f32 * PIP + (v.pages as f32 - 1.0) * PIP_GAP;
    if row > PANEL_W - 2.0 * PANEL_PAD {
        return;
    }
    let mut x = PANEL_X + (PANEL_W - row) / 2.0;
    let y = panel_y() + PANEL_PAD + (PIP_H - PIP) / 2.0;
    for i in 0..v.pages {
        out.push(Draw::Rect {
            x,
            y,
            w: PIP,
            h: PIP,
            colour: [1.0, 1.0, 1.0, if i == v.page { 1.0 } else { PIP_DIM }],
        });
        x += PIP + PIP_GAP;
    }
}

/// How many pages a table of `total` entries fills. A thin wrapper so callers do not have to
/// know the page size, which is this module's business.
pub fn pages_of(total: usize) -> usize {
    pages(total, PER_PAGE)
}
