use slot_gfx::{Draw, TexId};
use slot_power::{Battery, Charge};

use crate::palette;
use crate::plate::HINT_H;
use crate::status::Printed;

/// The bolt's slot, to the left of the capsule. Squeezed inside the capsule it had an 8 px box
/// to live in and came out as a smudge, not a bolt, and it punched a hole in whatever fill was
/// under it. Out here it sits on the housing at a size that actually reads, on the order of the
/// capsule's own height rather than half of it.
///
/// The raster size, not the drawn one: `icon_face` cuts the glyph to its ink and the drawn box
/// is `BOLT_W` square, so this only has to be big enough for the glyph to be cut from.
pub const BOLT_PX: f32 = 27.0;

/// The capsule, in the proportions of the thing it is a picture of. Wider than tall, with a
/// nub on the positive end.
///
/// One and a half times the size it was drawn at when it sat at the bottom of the case: it is
/// now read from the band at the top of the screen, which is where the eye goes for the time,
/// and a gauge at the old size read as a detail of the band rather than as one of the two
/// readings on it. The proportions are the old ones to the pixel — 22 × 11 × 1.5 — so nothing
/// about the picture changed, only its size.
pub const GAUGE_W: f32 = 33.0;
pub const GAUGE_H: f32 = 16.5;
// The doc comment above claims wider than tall; this is what makes that a fact the compiler
// enforces rather than a sentence someone could quietly falsify by editing one constant.
const _: () = assert!(GAUGE_H < GAUGE_W);
const NUB_W: f32 = 3.75;
const NUB_H: f32 = 6.0;
/// The wall of the capsule, drawn as four rects rather than an outline: the draw list has
/// only filled quads. Public so a test can bound the fill against the capsule's own inner
/// edge instead of trusting a hand-copied number that could drift from this one.
pub const WALL: f32 = 2.25;
/// Between the capsule and the number. Tight: the two are one reading, and a number that floated
/// free of the capsule would be a second thing on the band rather than the capsule's own value.
const UNDER_GAP: f32 = 1.0;

/// The whole cluster's height: the capsule with the number under it.
///
/// Exported because whoever places the cluster needs it to centre the thing — the caller cannot
/// work it out from the two constants above without also knowing they stack rather than sit side
/// by side, which is exactly the decision that changed here.
pub fn cluster_h() -> f32 {
    GAUGE_H + UNDER_GAP + HINT_H as f32
}

const BOLT_W: f32 = 21.0;
const BOLT_H: f32 = 21.0;
/// Between the bolt's slot and the capsule.
const BOLT_GAP: f32 = 5.0;
// A zero or negative gap would let the bolt's own quad touch or cross into the capsule's,
// which is the defect this file exists to fix, just moved from vertical overlap to
// horizontal. `the_bolt_never_reaches_the_capsule` in tests/battery.rs holds the same
// property at runtime, against the actual draw list rather than these numbers.
const _: () = assert!(BOLT_GAP > 0.0);

/// Where the capsule's own left wall is, for a cluster anchored at `right`: the tip of the nub
/// is the anchor, so everything else on the capsule measures back from it.
///
/// Public because the tests have to know which quad is the capsule's wall — the number's
/// placeholder is a `Rect` too, and a hand-copied offset would drift from the layout it is
/// meant to be checking.
pub fn capsule_left(right: f32) -> f32 {
    right - NUB_W - GAUGE_W
}

/// The capsule, its fill, and the number under it — read from the right, which is the way every
/// screen in the tree shows it: the clock has the left corner of the band and the battery the
/// right, so the whole cluster hangs off `right` and the number sits *below* the capsule.
///
/// Under rather than beside is the one thing on this band that has changed shape twice. It was
/// to the right of the capsule when the pair sat at the bottom of the case; moving to the band
/// put it on the other side so that a right-handed reading met the number first; and it is under
/// now, which buys back the width the cluster was spending on a number that is at most three
/// characters, and lets the capsule itself sit closer to the corner it belongs to.
///
/// `right` is the outer tip of the nub and `top` is the top of the *cluster* — the capsule's own
/// top, since the capsule is the upper of the two. The bolt's slot is reserved first,
/// unconditionally, so the capsule and the number sit in the same place whether or not a cable is
/// in; the bolt itself only ever draws inside that reserved slot, never over the fill.
pub fn draw_gauge(
    right: f32,
    top: f32,
    battery: Option<Battery>,
    percent: Printed,
    bolt: Option<TexId>,
    out: &mut Vec<Draw>,
) {
    // No gauge is no capsule, rather than an empty one: a device with no battery node has
    // nothing to say, and an empty capsule says the battery is flat.
    let Some(b) = battery else {
        return;
    };

    let rect = |x: f32, y: f32, w: f32, h: f32, out: &mut Vec<Draw>| {
        out.push(Draw::Rect {
            x,
            y,
            w,
            h,
            colour: palette::ink_f(),
        });
    };

    // Held whether or not anything is charging. Making this depend on `b.charge` is exactly
    // the bug this file exists to prevent, in a different shape: the capsule would still jump
    // sideways the instant a cable went in, just horizontally instead of losing its fill.
    let cx = capsule_left(right);
    let y = top;
    // The bolt's slot, worked out here rather than only where the bolt is drawn, so the slot is
    // held empty when there is no cable in. It is the one thing still beside the capsule: it
    // marks a state of the capsule, not a value of it, and a bolt under the capsule would be in
    // the space the number has just been given.
    let bolt_x = cx - BOLT_GAP - BOLT_W;

    rect(cx, y, GAUGE_W, WALL, out);
    rect(cx, y + GAUGE_H - WALL, GAUGE_W, WALL, out);
    rect(cx, y, WALL, GAUGE_H, out);
    rect(cx + GAUGE_W - WALL, y, WALL, GAUGE_H, out);
    rect(cx + GAUGE_W, y + (GAUGE_H - NUB_H) / 2.0, NUB_W, NUB_H, out);

    let inner = GAUGE_W - 4.0 * WALL;
    let fill = inner * f32::from(b.percent.min(100)) / 100.0;
    if fill > 0.0 {
        rect(
            cx + 2.0 * WALL,
            y + 2.0 * WALL,
            fill,
            GAUGE_H - 4.0 * WALL,
            out,
        );
    }

    // In its own slot, left of the capsule and vertically centred on it. Never over the
    // fill: the fill has to read as the same length at a given percent whether or not the
    // device is charging.
    if let (Charge::Charging, Some(tex)) = (b.charge, bolt) {
        out.push(Draw::Tex {
            x: bolt_x,
            y: y + (GAUGE_H - BOLT_H) / 2.0,
            w: BOLT_W,
            h: BOLT_H,
            tex,
            alpha: 1.0,
        });
    }

    if percent.w > 0 {
        // Centred on the capsule's own width, not on the cluster's: the nub is a decoration on
        // the positive end and centring under the whole thing would put the number a couple of
        // pixels off the body it names.
        let px = cx + (GAUGE_W - percent.w as f32) / 2.0;
        let py = y + GAUGE_H + UNDER_GAP;
        out.push(match percent.face {
            Some(tex) => Draw::Tex {
                x: px,
                y: py,
                w: percent.w as f32,
                h: HINT_H as f32,
                tex,
                alpha: 1.0,
            },
            None => Draw::Rect {
                x: px,
                y: py,
                w: percent.w as f32,
                h: HINT_H as f32,
                colour: [1.0, 1.0, 1.0, 0.08],
            },
        });
    }
}
