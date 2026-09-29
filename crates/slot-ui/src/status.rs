use slot_gfx::{Draw, TexId, OUT_W};
use slot_power::Battery;

use crate::battery::{cluster_h, draw_gauge};
use crate::icon::icon_box;
use crate::plate::STATUS_H;
use crate::slot_chrome::TOP_BAND_H;

/// Where the status line stands in the band: the clock's own top edge, with the mode badge
/// centred on the same line.
///
/// Measured off the band and not off the screen's edge, because both readings are printed on the
/// plastic rather than on the panel. This is the mirror of the place the line had when it was
/// drawn at the bottom of the case, and there it was measured off the cart bay for the same
/// reason.
///
/// Both ends are now the same band: `cluster_h()` is `STATUS_H`, so the gauge's own centring
/// `(TOP_BAND_H - cluster_h()) / 2` lands on this same number and the two ends cannot drift.
/// That is the whole reason the battery's cluster height is still a function rather than a bare
/// constant — one place decides, both ends read it.
const STATUS_Y: f32 = (TOP_BAND_H - STATUS_H as f32) / 2.0;

/// Blank at each end. Matches the gap the row leaves beside the outer carts, so what is printed
/// on the case lines up with what is above it.
const STATUS_MARGIN: f32 = 24.0;

/// Between the clock and the mode badge beside it. The badge is placed off the clock's own width
/// and not off the screen's edge, so a clock that gains a digit pushes the badge along rather
/// than being sat on by it.
const BADGE_GAP: f32 = 10.0;

/// The mode badge's box. The face is rasterised once, at `HUD_ICON_PX`, and drawn at the size it
/// was baked at; the sun and the moon are the one pair that must stay the same size as each
/// other, which they do by both being this.
const BADGE_PX: f32 = crate::hud::HUD_ICON_PX;

/// A line of type and the width it rasterised to. The width cannot be recovered from a
/// `TexId`, and only the compositor can mint one, so the space is held from the width alone
/// while the face is still on its way.
#[derive(Copy, Clone, Default, PartialEq, Eq, Debug)]
pub struct Printed {
    pub face: Option<TexId>,
    pub w: u32,
}

impl Printed {
    pub fn new(face: TexId, w: u32) -> Self {
        Printed {
            face: Some(face),
            w,
        }
    }
}

/// The clock at the left end of the top band and the battery at the right, both on the case's
/// own plastic.
///
/// The clock takes the left and the gauge the right, at the same margin, because a right-handed
/// reading of a status line puts the number first and the icon it belongs to last, and because
/// the number the battery carries is what the eye is looking for — the capsule is there to be
/// recognised at a glance, not read. The gauge is right-anchored with its number to the left of
/// it, so a clock that grows a character moves nothing else on the band and a percent that
/// gains a digit pushes only itself outward.
///
/// These two used to sit at the bottom of the case, on a band the cart bay now owns alone: the
/// bay is where the cart goes in and out of, and everything printed on it competed with the one
/// thing on the screen that moves.
pub fn draw_status(
    battery: Option<Battery>,
    percent: Printed,
    bolt: Option<TexId>,
    clock: Printed,
    mode: Option<TexId>,
    out: &mut Vec<Draw>,
) {
    draw_printed(STATUS_MARGIN, STATUS_Y, clock, STATUS_H, out);
    // The mode badge rides the clock's own width, so the two travel together and a clock that
    // gains a digit pushes the badge along rather than sitting on top of it. Nothing is drawn
    // until the clock's face has landed: a badge that arrived first would jump sideways the
    // moment the time appeared, and the clock is the one reading on this row whose width is not
    // fixed.
    if let Some(tex) = mode.filter(|_| clock.w > 0) {
        let (w, h) = icon_box(BADGE_PX);
        let (w, h) = (w as f32, h as f32);
        out.push(Draw::Tex {
            x: STATUS_MARGIN + clock.w as f32 + BADGE_GAP,
            y: STATUS_Y + (STATUS_H as f32 - h) / 2.0,
            w,
            h,
            tex,
            alpha: 1.0,
        });
    }
    let y = (TOP_BAND_H - cluster_h()) / 2.0;
    draw_gauge(OUT_W as f32 - STATUS_MARGIN, y, battery, percent, bolt, out);
}

/// A line of type at an arbitrary `y`. The placeholder is what holds the space while the
/// face is still on its way, so a row does not reflow the moment type arrives.
///
/// `h` is stated rather than assumed: `Draw::Tex` scales the face to the quad it is given, so a
/// caller that draws a `STATUS_H`-tall face into a `HINT_H` quad squashes it instead of leaving
/// it alone. There are two such callers now — the status row and the polaroids plate — and they
/// agree because both pass the same number, not because they both happen to use a default.
pub(crate) fn draw_printed(x: f32, y: f32, p: Printed, h: u32, out: &mut Vec<Draw>) {
    if p.w == 0 {
        return;
    }
    let (w, h) = (p.w as f32, h as f32);
    out.push(match p.face {
        Some(tex) => Draw::Tex {
            x,
            y,
            w,
            h,
            tex,
            alpha: 1.0,
        },
        None => Draw::Rect {
            x,
            y,
            w,
            h,
            colour: [1.0, 1.0, 1.0, 0.08],
        },
    });
}
