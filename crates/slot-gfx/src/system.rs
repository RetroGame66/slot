//! One machine's picture geometry: how big a frame its core hands us, and where on the
//! panel that frame lands.
//!
//! This used to be two constants, because there used to be one machine. The panel is 720x480
//! and the GBA is 240x160, and 240 lands in 720 exactly three times — which is why no other
//! scale is ever asked about, and why the LCD3x table can be a 3x3 mask sampled once per
//! source pixel rather than a resample.
//!
//! Game Boy is 160x144 and lands in the panel three times too, but 160x144 is short of the
//! panel in both directions, so it does not fill it: it gets a 480x432 window centred in the
//! frame. That window is what a bezel goes around.
//!
//! Game Boy and Game Boy Color get one row between them rather than one each: their frames
//! are the same 160x144, so they want the same texture and the same window, and a second copy
//! would only be a second place for the two to drift apart.

use crate::surface::{OUT_H, OUT_W};

/// The only scale that exists. One source pixel is exactly SCALE x SCALE panel pixels,
/// nearest neighbour, and it is what collapses LCD3x to a 3x3 mask tiled once per source
/// pixel.
pub const SCALE: u32 = 3;

/// The GBA's own frame. The pair this module used to be.
pub const GBA_SRC_W: u32 = 240;
pub const GBA_SRC_H: u32 = 160;

/// Game Boy's frame, and Game Boy Color's.
pub const GB_SRC_W: u32 = 160;
pub const GB_SRC_H: u32 = 144;

/// Which machine's picture is on the panel.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum System {
    Gba,
    /// Game Boy *and* Game Boy Color: one row, because one frame size.
    Gb,
}

impl System {
    /// The frame the core hands us, in pixels. This is also how many times the 3x3 mask
    /// tiles across the window: one RGB triad per source pixel, exactly — which is why a
    /// different source size needs no change to the mask itself.
    pub fn src_w(self) -> u32 {
        match self {
            System::Gba => GBA_SRC_W,
            System::Gb => GB_SRC_W,
        }
    }

    pub fn src_h(self) -> u32 {
        match self {
            System::Gba => GBA_SRC_H,
            System::Gb => GB_SRC_H,
        }
    }

    /// The rect the picture fills with the screen fully up, in offscreen pixels.
    ///
    /// Centred, which is all it takes: the GBA's own window comes out as the whole frame
    /// (240 lands in 720 three times and 160 in 480 three times, so the margins are zero),
    /// and Game Boy's comes out as 480x432 sat in the middle. One formula, and the machine
    /// that fills the panel is the case that falls out of it rather than a special case.
    pub fn screen_base(self) -> (f32, f32, f32, f32) {
        let w = self.src_w() as f32 * SCALE as f32;
        let h = self.src_h() as f32 * SCALE as f32;
        ((OUT_W as f32 - w) / 2.0, (OUT_H as f32 - h) / 2.0, w, h)
    }
}
