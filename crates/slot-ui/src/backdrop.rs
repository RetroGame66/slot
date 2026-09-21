use std::path::Path;

use slot_gfx::{Draw, TexId, OUT_H, OUT_W};

use crate::art;
use crate::palette;

/// The flat the screen is when the card carries no wallpaper, and what is behind the picture
/// when it does.
///
/// The device used to rely on the compositor's clear colour for this, which is black and is
/// compiled into `slot-gfx`. That is a palette fact, so it belongs here with the other one: in
/// the light mode a card with no wallpaper has to come up on paper rather than on a black
/// screen. Drawing it rather than clearing to it costs one full screen quad and keeps the
/// decision on this side of the crate boundary, where the mode lives.
fn ground() -> [f32; 4] {
    match palette::mode() {
        palette::Mode::Dark => [0.0, 0.0, 0.0, 1.0],
        palette::Mode::Light => [0.90, 0.89, 0.86, 1.0],
    }
}

/// Cover the whole panel, centre cropped. PNG only, as the labels are.
pub fn wallpaper_face(path: &Path) -> Option<Vec<u8>> {
    art::cover(path, OUT_W, OUT_H)
}

/// The ground, the picture and the scrim over it, as the first things on the screen.
///
/// The ground goes down whether or not there is a picture, and the picture and its scrim only
/// when there is one: that way the two modes differ by a colour rather than by a branch, and
/// neither has a case where the bottom of the stack is whatever the last frame left.
pub fn draw_backdrop(face: Option<TexId>, out: &mut Vec<Draw>) {
    out.push(Draw::Rect {
        x: 0.0,
        y: 0.0,
        w: OUT_W as f32,
        h: OUT_H as f32,
        colour: ground(),
    });
    let Some(tex) = face else {
        return;
    };
    out.push(Draw::Tex {
        x: 0.0,
        y: 0.0,
        w: OUT_W as f32,
        h: OUT_H as f32,
        tex,
        alpha: 1.0,
    });
    out.push(Draw::Rect {
        x: 0.0,
        y: 0.0,
        w: OUT_W as f32,
        h: OUT_H as f32,
        colour: palette::scrim(),
    });
}
