//! The battery reading: the percent, and the bolt while a cable is in.
//!
//! There used to be a capsule here — a 33 × 16.5 picture of a cell with a fill bar that tracked
//! the percent, and the number printed *under* it. Two objects stacked into a forty-one and a
//! half pixel cluster inside a fifty-eight pixel band, which read as crowded. What is left is the
//! reading itself: the percent at the clock's own size, right-anchored at the band's margin, with
//! the bolt beside it while charging.

use slot_gfx::{Draw, TexId};
use slot_power::{Battery, Charge};

use crate::plate::STATUS_H;
use crate::status::Printed;

/// The bolt's raster size, not its drawn one: `icon_face` cuts the glyph to its ink and the drawn
/// box is `BOLT_W` square, so this only has to be big enough for the glyph to be cut from.
pub const BOLT_PX: f32 = 27.0;

/// The bolt's drawn box, and the gap between it and the number.
pub const BOLT_W: f32 = 21.0;
pub const BOLT_H: f32 = 21.0;
const BOLT_GAP: f32 = 5.0;

/// The cluster is one band of type now: the same one the clock is set in.
///
/// Kept as a function rather than folded into `status`'s own constant so that exactly one place
/// decides how tall the row is, and both ends read it — the two ends of a status band drifting
/// apart is the failure this shape prevents.
pub fn cluster_h() -> f32 {
    STATUS_H as f32
}

/// The percent, right-anchored, with the bolt to its left whenever a cable is in.
///
/// `right` is the band's own margin, and the anchor is the number's **right** edge: the reading
/// grows leftward, so a percent that gains a digit moves nothing else and the margin holds.
///
/// The bolt is placed off the number's own width rather than from a reserved slot. There is no
/// longer a fixed-width capsule to reserve beside, and nothing needs reserving: the bolt appears
/// in the space the number leaves, in the gap, and the row does not move when it does.
///
/// Nothing is drawn until the percent's face has landed. With no capsule, a bolt placed against a
/// zero width would sit out at the margin and then jump left the moment the type arrived.
pub fn draw_gauge(
    right: f32,
    top: f32,
    battery: Option<Battery>,
    percent: Printed,
    bolt: Option<TexId>,
    out: &mut Vec<Draw>,
) {
    // No gauge is no reading, rather than an empty one: a device with no battery node has nothing
    // to say, and a bare "0%" would be a lie rather than an absence.
    let Some(b) = battery else {
        return;
    };
    if percent.w == 0 {
        return;
    }

    let x = right - percent.w as f32;
    let h = STATUS_H as f32;
    out.push(match percent.face {
        Some(tex) => Draw::Tex {
            x,
            y: top,
            w: percent.w as f32,
            h,
            tex,
            alpha: 1.0,
        },
        None => Draw::Rect {
            x,
            y: top,
            w: percent.w as f32,
            h,
            colour: [1.0, 1.0, 1.0, 0.08],
        },
    });

    if let (Charge::Charging, Some(tex)) = (b.charge, bolt) {
        out.push(Draw::Tex {
            x: x - BOLT_GAP - BOLT_W,
            y: top + (h - BOLT_H) / 2.0,
            w: BOLT_W,
            h: BOLT_H,
            tex,
            alpha: 1.0,
        });
    }
}
