//! The palette browser: three swatches across over a paused game, named **one of two ways
//! depending on which table is up**.
//!
//! Opened by a hold of the palette key and dismissed by B. The panel is the whole of the
//! screen's furniture while it is up — the game is still behind it, dimmed, because it is
//! paused rather than gone — and it carries the page of the table, the names on it, and the
//! entry under the caret ringed.
//!
//! **A colour machine's nine grades are named cell by cell.** Their names are four to six
//! characters and every one of them fits the column, so the page can be read across instead of
//! interrogated one cell at a time — which is the whole of what a browser is for once the table
//! is short enough to see, and a three-by-three grid has the room to spare because it fills three
//! rows of a panel built for five.
//!
//! **A Game Boy's fifteen palettes are not, and that page is unchanged from top to bottom.** Its
//! names come from the core's own header and they are long: measured in the face the machine
//! carries, **263 of its 343 names are wider than a cell at the size the grades are set in**, and
//! the longest is still 30 pixels too wide at ten. A cell that held one would be setting it at
//! eight, which is not type, it is a stain. So that page keeps the one line at the foot of the
//! panel for the entry under the caret — 640 pixels to work in, so no name is ever cut — and it
//! keeps **the same five rows of 42 and the same 340-pixel panel it has always had**. None of
//! this was allowed to move it by a pixel, which is what `PLAIN_CELL_H` and the test named for it
//! are there to hold.
//!
//! The colours are drawn as plain rectangles rather than as textures. A swatch is a grid of
//! colours already known at compile time; rasterising them would cost a texture each for a
//! picture the draw list can state outright. The names are the other half of that trade — type
//! cannot be stated, so they are the textures.
//!
//! **What a cell is painted with is a picture's colours, not a grey ramp** — eight hues across,
//! with a black-to-white tone strip under them (`SWEEP`, `TONE`). A grey ramp answers "what does
//! this grade do to white and black", which is nothing much, and every entry came out looking
//! alike and grey; a hue sweep answers what a grade does to a picture, which is the question the
//! browser is asked. The tone strip stays because three of the four mechanisms only show
//! themselves on a neutral — see `TONE`.

use slot_gfx::{Draw, TexId, OUT_H, OUT_W};
use slot_store::pages;

use crate::palette;
use crate::plate::UndoFace;
use crate::slot_chrome::{edge, opening};
use crate::text;

/// Three across. Everything else follows from this and the page: a colour machine's eighteen
/// grades are two pages of nine, a Game Boy's table is fifteen to a page.
pub const COLS: usize = 3;
/// The Game Boy's grid — five rows of three. Its shape is fixed because its table is hundreds
/// long and every page of it but the last is full.
pub const ROWS: usize = 5;
pub const PER_PAGE: usize = COLS * ROWS;

/// The rows a page of `per` entries is laid out in. A colour machine's nine grades are three
/// rows of three; a Game Boy's fifteen are five. **The two machines do not share one height**:
/// drawing nine entries on a five-row panel left two rows of the panel doing nothing, which is
/// the space the names moved into.
pub fn rows_of(per: usize) -> usize {
    per.max(1).div_ceil(COLS).max(1)
}

/// The hues the demo chart is built from, one **column** each: a full turn in eight steps.
///
/// **A cell is painted with a picture's colours and not with a grey ramp**, and that is the whole
/// of what this table is. The first cut walked the grey axis — black, 85, 170, 255 and every step
/// between — which shows what a grade does to *white and black*, and most grades leave those
/// alone. Every entry therefore came out looking the same and grey, which is exactly what the
/// player said about it. A sweep of hues is what a picture is actually made of, so a grade that
/// moves colour moves the strip: a desaturating matrix washes it out, a palette grade snaps it
/// onto that palette's own colours, and a duotone folds it onto its two ends.
pub const SWEEP: [[u8; 3]; 8] = [
    [255, 0, 0],
    [255, 191, 0],
    [191, 255, 0],
    [0, 255, 191],
    [0, 255, 255],
    [0, 191, 255],
    [0, 0, 255],
    [255, 0, 191],
];
/// The greys the chart's **second row** is made of: the picture's shadows and highlights, which is
/// where a tone map does its work and where a hue sweep says nothing at all.
///
/// It was the dimmed hues at first, on the reasoning that two brightnesses of the same colour say
/// more than one. Measured against the actual assets, that is wrong: a film grade keeps a
/// saturated dark red red — `TealOrange` maps `(89,0,0)` to `(64,0,0)`, not to teal — so the split
/// a split-tone is *for* never appeared. Its teal lives in the neutral shadows, and a neutral is
/// what has to be in the cell for it to show. Hence a tone strip rather than a second hue row.
pub const TONE: [[u8; 3]; 8] = [
    [0, 0, 0],
    [36, 36, 36],
    [73, 73, 73],
    [109, 109, 109],
    [146, 146, 146],
    [182, 182, 182],
    [219, 219, 219],
    [255, 255, 255],
];
/// The rows a chart cell carries — `SWEEP`, and the tone strip under it.
const SWEEP_ROWS: usize = 2;

/// The most stops a cell can carry. A Game Boy palette is four; a colour grade's chart is the
/// whole sweep. One array wide enough for the longer of the two, so a cell is a value rather than
/// a `Vec` built per frame.
pub const MAX_STOPS: usize = SWEEP.len() * SWEEP_ROWS;
/// How many stops a colour grade's cell is drawn with.
pub const STOPS: usize = MAX_STOPS;

/// What one cell is painted with: `n` stops laid out `cols` across, read left to right and then
/// down.
///
/// **A Game Boy palette is four shades and its cell is four bands across.** Four is not a
/// simplification there, it is the machine's own resolution — the DMG screen had four levels and
/// the palette names them — so that cell is one row.
///
/// **A colour grade has no shades at all**, which is what made this a type rather than an array
/// of four: the cell is `SWEEP` taken through the grade (`chart`), which is the same arithmetic
/// the game pass does, so what the cell shows is what the picture will get.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub struct Swatch {
    stops: [[u8; 3]; MAX_STOPS],
    n: usize,
    cols: usize,
}

impl Swatch {
    /// A palette's four shades, one band each.
    pub fn shades(shades: [[u8; 3]; 4]) -> Self {
        let mut stops = [[0u8; 3]; MAX_STOPS];
        stops[..4].copy_from_slice(&shades);
        Self {
            stops,
            n: 4,
            cols: 4,
        }
    }

    /// The demo chart: every hue in `SWEEP` and then every step in `TONE`, each one handed to
    /// `f` — which is the grade, applied to it exactly as the picture is applied to. The caller
    /// owns the transform; this owns what a picture is made of.
    pub fn chart(mut f: impl FnMut([u8; 3]) -> [u8; 3]) -> Self {
        let cols = SWEEP.len();
        let mut stops = [[0u8; 3]; MAX_STOPS];
        for (i, s) in stops.iter_mut().enumerate() {
            let ink = if i < cols { SWEEP[i] } else { TONE[i - cols] };
            *s = f(ink);
        }
        Self {
            stops,
            n: MAX_STOPS,
            cols,
        }
    }

    /// The stops themselves, in reading order.
    pub fn stops(&self) -> &[[u8; 3]] {
        &self.stops[..self.n]
    }

    /// How many of them there are across — one for a palette's shades, `SWEEP`'s length for a
    /// chart. The rest is the height.
    pub fn cols(&self) -> usize {
        self.cols.max(1)
    }

    /// How many rows the cell's stops make. Exact, because both shapes are a whole number of
    /// rows: the chart's two, a palette's one.
    pub fn rows(&self) -> usize {
        self.n.div_ceil(self.cols()).max(1)
    }
}

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

/// What the panel has to spend on a page **whose cells are named**: the pips, the padding and the
/// rim take 36 + 14 + 14 of its 340 pixels and this is the rest.
///
/// A colour machine spends the lot — three rows of cells with a name band under each. It comes to
/// the same panel as the Game Boy's, because the 290 is exactly what that page already spent on
/// five plain rows and its foot line: **naming a colour machine's cells cost the Game Boy
/// nothing**, which is the point. The named grid pays for names what the plain one pays for the
/// two rows a three-by-three table never fills.
const SPARE_H: f32 = 290.0;

/// How tall a cell is on a page that keeps a foot line instead of naming each cell — the Game
/// Boy's layout, and **the only layout there was before the colour machines were named**.
///
/// A fixed number rather than a fill, because that page has nothing to fit: five of these and the
/// foot line make the same 340-pixel panel they have always made, and the cell keeps the strip
/// proportions it has always had. Making the two layouts one expression is what put 56 pixels of
/// Game Boy into a place no one asked for it.
const PLAIN_CELL_H: f32 = 42.0;

/// The tallest a cell is allowed to be: what a three-row grid asks for to the pixel, which is the
/// shallowest grid in the tree. So the cap only ever binds on a page shallower than that (one or
/// two rows, i.e. the tail of a table), where a cell left to itself would be a skyscraper — and
/// neither table's own grid reaches it.
const MAX_CELL_H: f32 = SPARE_H / 3.0;

/// The band a cell's name is set in, under the cell and inside its own column.
///
/// The name is rasterised into a box **the width of the cell** rather than cut to its own ink:
/// every cell is the same width, so every face is the same size, and the frontend's pool of
/// fifteen textures is written over rather than grown and thrown away each time the page turns.
/// `fit` shrinks a long name instead of letting it run past its column, which is what makes it
/// safe to reuse the card's own type on a list whose names are not ours.
///
/// The height is **not** the 2× the type size the single-line dialog uses. `text::coverage`
/// centres a line on its true em box — ascent minus descent — and a card face's em box is not one
/// em: Plix ships 1.00 em, Noto 1.48, and a face with a taller one would be clipped at the top of
/// a band that only just held it. At 1.6× there is room for an em box half again as tall as
/// either, which is what a card face the author has never seen has to fit in.
const NAME_H: u32 = 24;
const NAME_PX: f32 = 15.0;
const NAME_MIN_PX: f32 = 10.0;
const NAME_GAP_Y: f32 = 5.0;

/// The foot line, used when the cells carry no names: the entry under the caret, in a box the
/// whole panel wide. It is the reading a Game Boy needs **and the one it already had** — the
/// colour machines are the pages that gave it up, not this one.
///
/// `FOOT_H` is twice `FOOT_PX` for the reason `plate::dialog_line_face` gives — the font's own
/// line box has to fit inside the band rather than being clipped by it — and its box is 640 wide,
/// which is what makes a name that no cell could hold readable at all. Both numbers are the ones
/// this line has always used: 36 and 12 under the grid are what make a Game Boy's page the 340
/// pixels it has been since the panel was written.
const FOOT_H: f32 = 36.0;
pub const FOOT_PX: f32 = 18.0;
const FOOT_GAP_Y: f32 = 12.0;
/// The two bands, added up once: a named row's, and the foot line's.
const NAME_BAND: f32 = NAME_GAP_Y + NAME_H as f32;
const FOOT_BAND: f32 = FOOT_GAP_Y + FOOT_H;

/// How thick the ring around the highlighted cell is, and the air between it and the swatch.
///
/// The air is the part that took a round of testing to arrive at. A ring drawn straight onto the
/// cell has its inner edge swallowed by whatever the cell's outermost band is — and four of these
/// palettes are near-white (`原生灰`'s paper is `#FFFFFF`), so on those the caret looked like a
/// cell that was merely bigger, not one that was chosen. One pixel of panel between the two
/// separates them on every palette there is.
///
/// The colour is the theme's **ink**, the same one the names are set in, rather than the
/// mid-grey `edge()` this started as: a grey ring is close to half the palettes in the table
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

/// How much of the paused game is left showing through: enough to see there is one, not enough
/// for it to compete with the cells.
const VEIL: f32 = 0.62;

fn cell_w() -> f32 {
    (PANEL_W - 2.0 * PANEL_PAD - (COLS as f32 - 1.0) * CELL_GAP) / COLS as f32
}

/// The width of a cell, and so of a name — the one number the frontend needs to rasterise a
/// page of them.
pub fn cell_width() -> u32 {
    cell_w().round() as u32
}

/// The name band's height, for the same reason.
pub const fn name_height() -> u32 {
    NAME_H
}

/// The height of a cell: whatever the panel has left once the names are paid for.
///
/// `named` is whether the cells carry names of their own — see `View::names`. A named page fills:
/// the cells take the slack after each row has paid for its band. **A page with a foot line does
/// not fill anything** — it has no band to pay for and no reason to be a different height than it
/// has always been, so it is `PLAIN_CELL_H` and nothing else. The two come out on the same panel
/// because the numbers were chosen that way, not because the arithmetic is forced to agree.
///
/// There is deliberately **no floor** under the named one. A floor is exactly what would put the
/// panel off the bottom of the screen; a page deeper than either table has would get thin cells
/// instead, which at least is a failure that can be seen.
fn cell_h(rows: usize, named: bool) -> f32 {
    if !named {
        return PLAIN_CELL_H;
    }
    let rows = rows.max(1) as f32;
    let spare = SPARE_H - rows * NAME_BAND - (rows - 1.0) * ROW_GAP;
    (spare / rows).clamp(1.0, MAX_CELL_H)
}

/// The height of everything the grid takes, names and all.
fn block_h(rows: usize, named: bool) -> f32 {
    let rows_f = rows.max(1) as f32;
    let band = if named { NAME_BAND } else { 0.0 };
    rows_f * (cell_h(rows, named) + band) + (rows_f - 1.0) * ROW_GAP
}

/// The distance from one row's top to the next row's: **a cell, the name under it, and the air
/// between one row and the next**.
///
/// The name band is part of the pitch and not furniture drawn over the gap — a row is a cell and
/// what it is called, and the only thing between two rows is `ROW_GAP`. Leaving the band out of
/// this was the first cut's mistake: the names sat on top of the row below them and five rows
/// came out 362 pixels deep in the arithmetic and 254 deep on the screen.
fn row_pitch(rows: usize, named: bool) -> f32 {
    let band = if named { NAME_BAND } else { 0.0 };
    cell_h(rows, named) + band + ROW_GAP
}

fn panel_h(rows: usize, named: bool) -> f32 {
    let foot = if named { 0.0 } else { FOOT_BAND };
    PANEL_PAD + PIP_H + PIP_GAP_Y + block_h(rows, named) + foot + PANEL_PAD
}

/// The panel's top edge. The whole panel is centred on the screen rather than the grid, which
/// is the difference the user asked for: the grid sat high because the panel around it did not
/// count for anything.
fn panel_y(rows: usize, named: bool) -> f32 {
    (OUT_H as f32 - panel_h(rows, named)) / 2.0
}

fn grid_y(rows: usize, named: bool) -> f32 {
    panel_y(rows, named) + PANEL_PAD + PIP_H + PIP_GAP_Y
}

fn cell_x(col: usize) -> f32 {
    PANEL_X + PANEL_PAD + col as f32 * (cell_w() + CELL_GAP)
}

fn row_y(row: usize, rows: usize, named: bool) -> f32 {
    grid_y(rows, named) + row as f32 * row_pitch(rows, named)
}

/// Everything the panel needs, gathered by the app in one go.
pub struct View<'a> {
    /// Which page of the table is up, and which cell on it carries the caret.
    pub page: usize,
    pub cursor: usize,
    /// How many pages the table fills, for the pips.
    pub pages: usize,
    /// The grid's shape for the machine that is up — `rows_of` of the page size.
    pub rows: usize,
    /// One swatch per entry **on this page**, in table order.
    pub swatches: &'a [Swatch],
    /// One name face per entry, in the same order — **or empty, for a table whose names do not
    /// fit a cell**. Which of the two the app hands over is what decides the panel's shape, so
    /// there is no third thing to keep in step with it: named cells grow a band each and take the
    /// height from `foot`, an unnamed page has no bands and sets its one line at the foot.
    ///
    /// Short or empty on the frame before the frontend has minted them too, which is why a page
    /// whose table *is* named is drawn unnamed for exactly one frame — an absent line rather than
    /// a grey bar.
    pub names: &'a [(TexId, u32, u32)],
    /// The entry under the caret, for the pages that have no per-cell names. `None` when the
    /// cells carry their own, which is every colour machine.
    pub foot: Option<(TexId, u32, u32)>,
}

impl View<'_> {
    /// Whether the cells carry names of their own, and so whether there is a foot line.
    fn named(&self) -> bool {
        !self.names.is_empty()
    }
}

pub fn draw(v: &View, out: &mut Vec<Draw>) {
    let rows = v.rows.max(1);
    let named = v.named();
    let ch = cell_h(rows, named);
    let py = panel_y(rows, named);

    out.push(Draw::Rect {
        x: 0.0,
        y: 0.0,
        w: OUT_W as f32,
        h: OUT_H as f32,
        colour: [0.0, 0.0, 0.0, VEIL],
    });
    out.push(Draw::Rect {
        x: PANEL_X - RIM,
        y: py - RIM,
        w: PANEL_W + 2.0 * RIM,
        h: panel_h(rows, named) + 2.0 * RIM,
        colour: edge(),
    });
    out.push(Draw::Rect {
        x: PANEL_X,
        y: py,
        w: PANEL_W,
        h: panel_h(rows, named),
        colour: opening(),
    });
    draw_pips(v, out);

    let cw = cell_w();
    for (i, swatch) in v.swatches.iter().enumerate() {
        let row = i / COLS;
        if row >= rows {
            break;
        }
        let (x, y) = (cell_x(i % COLS), row_y(row, rows, named));
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
                h: ch + 2.0 * reach,
                colour: palette::ink_f(),
            });
            out.push(Draw::Rect {
                x: x - FRAME_GAP,
                y: y - FRAME_GAP,
                w: cw + 2.0 * FRAME_GAP,
                h: ch + 2.0 * FRAME_GAP,
                colour: opening(),
            });
        }
        // One band a stop, laid edge to edge in the cell's own grid: the last column and the last
        // row take whatever the arithmetic left over rather than their own share, so a cell can
        // never end a pixel short. A Game Boy's four shades are one row of four; a colour grade's
        // chart is eight hues across two rows (`SWEEP`).
        let stops = swatch.stops();
        let (cols, rows_in) = (swatch.cols(), swatch.rows());
        let (bw, bh) = (cw / cols as f32, ch / rows_in as f32);
        for (b, c) in stops.iter().enumerate() {
            let (col, row) = (b % cols, b / cols);
            if row >= rows_in {
                break;
            }
            let (bx, by) = (x + col as f32 * bw, y + row as f32 * bh);
            out.push(Draw::Rect {
                x: bx,
                y: by,
                w: if col + 1 == cols { x + cw - bx } else { bw },
                h: if row + 1 == rows_in { y + ch - by } else { bh },
                colour: [
                    c[0] as f32 / 255.0,
                    c[1] as f32 / 255.0,
                    c[2] as f32 / 255.0,
                    1.0,
                ],
            });
        }
        // The cell's own name, under it and in its own column.
        if let Some(&(tex, w, h)) = v.names.get(i) {
            out.push(Draw::Tex {
                x: x + (cw - w as f32) / 2.0,
                y: y + ch + NAME_GAP_Y,
                w: w as f32,
                h: h as f32,
                tex,
                alpha: 1.0,
            });
        }
    }

    // The line at the foot, for the pages whose cells are anonymous. Centred in the band the
    // panel kept for it, and cut to its own ink rather than to a box — it is the only line on the
    // panel and there is nothing for it to line up with.
    if let Some((tex, w, h)) = v.foot {
        out.push(Draw::Tex {
            x: PANEL_X + (PANEL_W - w as f32) / 2.0,
            y: grid_y(rows, named) + block_h(rows, named) + FOOT_GAP_Y + (FOOT_H - h as f32) / 2.0,
            w: w as f32,
            h: h as f32,
            tex,
            alpha: 1.0,
        });
    }
}

/// One cell's name, set to the width of the cell it belongs to and centred in it.
///
/// `fit` shrinks rather than wraps, because a name on two lines in a 24-pixel band would be two
/// clipped lines; and it is asked for the cell's width rather than the ink's, so the face is the
/// same size for every name and the frontend's textures are reused instead of reallocated.
pub fn name_face(name: &str, w: u32, h: u32) -> UndoFace {
    let mut rgba = vec![0u8; (w.max(1) * h.max(1) * 4) as usize];
    if let Some(font) = text::label_font() {
        let layout = text::fit(font, name, w as f32, 1, NAME_PX, NAME_MIN_PX);
        text::draw_centred(&mut rgba, w.max(1), h.max(1), &layout, palette::ink());
    }
    UndoFace {
        rgba,
        w: w.max(1),
        h: h.max(1),
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
    let y = panel_y(v.rows.max(1), v.named()) + PANEL_PAD + (PIP_H - PIP) / 2.0;
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

/// How many pages a table of `total` entries fills, `per_page` at a time. A thin wrapper so
/// callers do not have to know which crate the arithmetic lives in.
pub fn pages_of(total: usize, per_page: usize) -> usize {
    pages(total, per_page)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The two page sizes the tree actually asks for: a Game Boy's fifteen and a colour machine's
    /// nine. Everything about the layout is a function of one of them.
    const PAGES: [usize; 2] = [PER_PAGE, 9];

    /// The two readings: a colour machine's page, whose nine short names go under their own cells,
    /// and a Game Boy's, whose long ones go on the foot line instead. See `View::names`.
    const NAMED: [bool; 2] = [true, false];

    #[test]
    fn a_page_is_laid_out_the_shape_its_machine_asked_for() {
        assert_eq!(
            rows_of(PER_PAGE),
            ROWS,
            "a Game Boy's page is five rows of three"
        );
        assert_eq!(
            rows_of(9),
            3,
            "a colour machine's page is three rows of three"
        );
        // Every page in the tree is a whole number of rows; a ragged one would need the last row
        // half-filled and nothing here is written for that.
        for per in PAGES {
            assert_eq!(per % COLS, 0, "{per} entries do not fill a row");
        }
    }

    #[test]
    fn the_game_boys_page_is_the_page_it_has_always_been() {
        // The one thing the naming was not allowed to touch. Every number here is the value this
        // panel had before a colour machine's cells were named, written out rather than derived —
        // so if a future change to the named layout drags the Game Boy's with it, this fails and
        // says which of the two moved.
        assert_eq!(PLAIN_CELL_H, 42.0, "the Game Boy's cells are 42 pixels");
        assert_eq!(cell_h(ROWS, false), 42.0);
        assert_eq!(
            block_h(ROWS, false),
            5.0 * 42.0 + 4.0 * 8.0,
            "five rows and four gaps"
        );
        assert_eq!(
            panel_h(ROWS, false),
            340.0,
            "and the panel it has always been"
        );
        assert_eq!(
            panel_y(ROWS, false),
            70.0,
            "centred on the screen, as it was"
        );
        assert_eq!(FOOT_H, 36.0, "the foot line's band");
        assert_eq!(FOOT_PX, 18.0, "and its type");
    }

    #[test]
    fn the_panel_does_not_change_size_with_the_machine() {
        // The pairing the tree actually uses: a Game Boy's five plain rows against a colour
        // machine's three named ones. Both are 340 pixels, because what the named grid spends on
        // names is what the plain one was already spending on the two rows a three-by-three table
        // never fills — so the panel does not jump when the machine changes, and the Game Boy did
        // not move to make room for the names.
        let (gb, colour) = (panel_h(ROWS, false), panel_h(rows_of(9), true));
        assert!(
            (gb - colour).abs() < 0.5,
            "a Game Boy's panel is {gb} tall and a colour machine's {colour}"
        );
        // And the named grid really is spending the freed space on cells rather than being the
        // plain one with a coat of paint: it takes exactly the plain grid's height plus the foot
        // line that grid pays and it does not.
        let (named_grid, plain_grid) = (block_h(rows_of(9), true), block_h(ROWS, false));
        assert!(
            (named_grid - plain_grid - FOOT_BAND).abs() < 0.5,
            "a named grid is {named_grid}, a plain one {plain_grid} plus a {FOOT_BAND} foot line"
        );
        assert!(
            cell_h(rows_of(9), true) > cell_h(ROWS, false),
            "a named cell has the room"
        );
    }

    #[test]
    fn every_panel_fits_inside_the_screen() {
        // A panel is drawn from its top edge down with nothing at the bottom edge to clip
        // against, so one taller than the screen is drawn off it silently. Checked for every
        // depth either table can reach, in both readings.
        for named in NAMED {
            for rows in 1..=ROWS {
                let (h, y) = (panel_h(rows, named), panel_y(rows, named));
                assert!(
                    h <= OUT_H as f32,
                    "a {rows}-row panel with names {named} is {h} tall on a {OUT_H} screen"
                );
                assert!(y >= 0.0, "a {rows}-row panel starts at {y}");
            }
        }
    }

    #[test]
    fn a_name_sits_under_its_own_cell_and_clear_of_the_next_row() {
        // The band is inside the row pitch, so the invariant is that one row's name finishes
        // before the next row's cell begins. It did not, in the first cut: the pitch counted the
        // gap between cells but not the names between them, so every name was drawn over the row
        // below and the panel was a third shorter than it claimed to be.
        for named in NAMED {
            for rows in 1..=ROWS {
                let (ch, pitch) = (cell_h(rows, named), row_pitch(rows, named));
                let band = if named { NAME_BAND } else { 0.0 };
                assert!(ch > 0.0, "{rows} rows leave the cells {ch} tall");
                for r in 1..rows {
                    let above = row_y(r - 1, rows, named) + ch + band;
                    assert!(
                        above <= row_y(r, rows, named),
                        "row {r}'s cell starts at {} and the row above ends at {above}",
                        row_y(r, rows, named)
                    );
                }
                // The pitch is the rows' own spacing, to within the rounding of one
                // multiplication — which is all an equality between two products can promise.
                let spaced = row_y(1, rows, named) - row_y(0, rows, named);
                assert!(
                    (spaced - pitch).abs() < 0.001,
                    "{rows} rows are spaced {spaced}, pitch {pitch}"
                );
            }
        }
    }

    #[test]
    fn the_grid_and_its_names_end_inside_the_panel() {
        // The last column, the last row's name band (or the foot line under it) and the panel's
        // own inner edge all have to agree about where the grid ends.
        for named in NAMED {
            for per in PAGES {
                let rows = rows_of(per);
                let right = cell_x(COLS - 1) + cell_w();
                assert!(
                    right <= PANEL_X + PANEL_W - PANEL_PAD,
                    "the last column ends at {right}"
                );
                let band = if named { NAME_BAND } else { FOOT_BAND };
                let bottom = row_y(rows - 1, rows, named) + cell_h(rows, named) + band;
                let edge = panel_y(rows, named) + panel_h(rows, named) - PANEL_PAD;
                assert!(
                    (bottom - edge).abs() < 0.5,
                    "a {per}-entry grid with names {named} ends at {bottom}, panel {edge}"
                );
            }
        }
    }

    #[test]
    fn a_foot_line_has_room_for_the_type_that_goes_in_it() {
        // `FOOT_H` is twice `FOOT_PX` because `dialog_line_face` rasterises into a box of exactly
        // that, and a band shorter than the box would clip the bottom of a descender.
        assert!(
            (FOOT_H - 2.0 * FOOT_PX).abs() < 0.5,
            "the foot band is {FOOT_H} for {FOOT_PX}px"
        );
        // And the same rule for a cell's name: the band has to hold the font's em box, not just
        // its type size.
        assert!(
            NAME_H as f32 >= NAME_PX * 1.5,
            "a {NAME_H}px band for {NAME_PX}px type"
        );
    }

    #[test]
    fn a_cell_is_wide_enough_to_set_a_name_in() {
        // The names are rasterised into boxes the width of a cell, so a narrow cell would set
        // every name at the same illegible size rather than shrinking only the long ones.
        assert!(cell_width() >= 120, "a cell is only {} wide", cell_width());
    }

    #[test]
    fn a_swatch_says_what_shape_it_is() {
        let shades = Swatch::shades([[0, 0, 0], [85, 85, 85], [170, 170, 170], [255, 255, 255]]);
        assert_eq!(
            shades.stops().len(),
            4,
            "a palette is four shades and its cell four bands"
        );
        assert_eq!(shades.stops()[3], [255, 255, 255]);
        // One row of four across: that is the machine's own resolution, not a layout choice.
        assert_eq!((shades.cols(), shades.rows()), (4, 1));

        // The chart: the hues across, the tone strip under them, and the two together are what a
        // picture is — so the cell answers the question the browser is asked. Asserted rather than
        // left to whoever edits `SWEEP` next.
        let chart = Swatch::chart(|c| c);
        assert_eq!(chart.stops().len(), SWEEP.len() * SWEEP_ROWS);
        assert_eq!(chart.stops().len(), STOPS);
        assert_eq!((chart.cols(), chart.rows()), (SWEEP.len(), SWEEP_ROWS));
        // An untransformed chart is exactly the tables it is built from, in reading order.
        for (i, s) in chart.stops().iter().enumerate() {
            let want = if i < SWEEP.len() { SWEEP[i] } else { TONE[i - SWEEP.len()] };
            assert_eq!(*s, want, "stop {i}");
        }
        // The tone row climbs and the hues do not, which is what makes the two rows tell two
        // different things rather than being one picture printed twice.
        let tones = &chart.stops()[SWEEP.len()..];
        for (i, s) in tones.iter().enumerate() {
            assert_eq!((s[0], s[1]), (s[2], s[2]), "tone stop {i} is {s:?}, which is not neutral");
            if i > 0 {
                assert!(s[0] > tones[i - 1][0], "the tone strip is not monotonic at {i}");
            }
        }
        assert_eq!(tones[0], [0, 0, 0], "the strip starts at black");
        assert_eq!(tones[SWEEP.len() - 1], [255, 255, 255], "and ends at white");
        // Every hue is a hue: a stop with no channel in it is a hole in the panel, not a colour.
        for (i, s) in chart.stops()[..SWEEP.len()].iter().enumerate() {
            assert!(s.iter().any(|v| *v > 200), "hue {i} is {s:?}, which reads as black");
        }

        // The transform is the caller's, and it is applied to every stop — which is what makes a
        // desaturating grade show up in both rows and a split-tone in the second one.
        let flipped = Swatch::chart(|c| [255 - c[0], 255 - c[1], 255 - c[2]]);
        assert_eq!(flipped.stops()[0], [0, 255, 255]);
        assert_eq!(flipped.stops()[SWEEP.len()], [255, 255, 255]);
    }

    #[test]
    fn a_table_pages_the_way_the_app_expects_it_to() {
        assert_eq!(
            pages_of(18, 9),
            2,
            "a colour machine's eighteen grades are two pages"
        );
        assert_eq!(
            pages_of(343, 15),
            23,
            "and a Game Boy's table is twenty-three"
        );
        assert_eq!(
            pages_of(0, 15),
            1,
            "an empty table still has a page to stand on"
        );
    }
}
