//! The game layer coming up and going out. A panel does not fade in: it strikes as a line
//! and blooms, and it dies back down to a dot. `t` is 0.0 dark and 1.0 fully on throughout,
//! so power off is the same curve walked backwards.

use crate::surface::{OUT_H, OUT_W};

/// The line the picture opens from and closes to. Two pixels rather than one: an odd height
/// centred in an even frame lands on a half pixel and the line reads as grey.
const LINE_PX: f32 = 2.0;

/// How much of the travel the horizontal collapse gets. Only the tail of it, or the whole
/// thing reads as an iris closing rather than a screen going out.
const DOT_T: f32 = 0.12;

/// How far past normal the strike goes. The picture is a line when it is brightest, so this
/// is a flash on a few hundred pixels rather than on the whole frame.
const OVERSHOOT: f32 = 0.6;

/// Height of the picture as a fraction of the frame. Ease out: it snaps open and settles,
/// which is a panel striking rather than a blind going up.
pub fn screen_scale(t: f32) -> f32 {
    let left = 1.0 - t.clamp(0.0, 1.0);
    1.0 - (1.0 - LINE_PX / OUT_H as f32) * left * left
}

/// Width, and only over the last of the collapse. This is the dot the line closes to.
pub fn screen_width(t: f32) -> f32 {
    let left = 1.0 - (t.clamp(0.0, 1.0) / DOT_T).min(1.0);
    1.0 - (1.0 - LINE_PX / OUT_W as f32) * left
}

/// Gain on the game layer. Brightest as the line appears and settling to exactly 1.0, so a
/// screen that is up is not a screen that is being graded.
pub fn screen_brightness(t: f32) -> f32 {
    let left = 1.0 - t.clamp(0.0, 1.0);
    1.0 + OVERSHOOT * left * left
}

/// The rect the game layer fills, in offscreen pixels. Centred: the line is at the vertical
/// middle of the frame, not at the slot.
///
/// The GBA's window, which is the whole frame — see `screen_rect_in` for a picture whose
/// window is smaller than the frame.
pub fn screen_rect(t: f32) -> (f32, f32, f32, f32) {
    screen_rect_in((0.0, 0.0, OUT_W as f32, OUT_H as f32), t)
}

/// The same strike and bloom inside a window of its own, for a machine whose picture does
/// not fill the frame.
///
/// Everything above is written against the whole frame, and the Game Boy's window is only
/// part of it: a picture that opened from the middle of the panel and settled into a window
/// beside it would read as the panel being *oddly* switched on, rather than as a smaller
/// screen coming up. So the collapse is applied to the window, centred on the window, and
/// the GBA remains the case where the window happens to be everything.
pub fn screen_rect_in(base: (f32, f32, f32, f32), t: f32) -> (f32, f32, f32, f32) {
    let (bx, by, bw, bh) = base;
    let w = bw * screen_width(t);
    let h = bh * screen_scale(t);
    (bx + (bw - w) / 2.0, by + (bh - h) / 2.0, w, h)
}
