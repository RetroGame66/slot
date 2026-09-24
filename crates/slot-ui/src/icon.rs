use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};

use fontdue::{Font, FontSettings};

use crate::CartFace;

/// The Mono variant, because its fixed advance width keeps a HUD row from reflowing when the
/// glyph changes under it.
const SYMBOLS_TTF: &[u8] = include_bytes!("../assets/SymbolsNerdFontMono-Regular.ttf");

#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub enum Icon {
    Volume,
    /// Turned all the way down, which is not the same as silenced: the level can still be
    /// walked back up from here without touching the mute.
    VolumeZero,
    VolumeMuted,
    Brightness,
    BlueLight,
    FastForward,
    FastForwardLatched,
    Rewind,
    Alert,
    Charging,
    /// The two the status line carries: which way round the frontend is printed, shown beside
    /// the clock. Filled Material glyphs rather than the outline ones `Brightness` and
    /// `BlueLight` use, so the reading on the band is never mistaken for one of the levels on
    /// the bar — the level icons are a control the user just moved, and this is a state of the
    /// screen that has been sitting there all along.
    Sun,
    Moon,
    /// The star a favourite cart wears, and the same star hollow for the shelf's indicator
    /// while it is off. Solid and outline rather than one glyph tinted two ways, for the same
    /// reason `FastForward` and `FastForwardLatched` differ only in fill: the silhouette stays
    /// put and the weight carries the state, so an indicator that lights does not also move.
    ///
    /// Uploaded beside the rest of the icons so `frame()` bounds them with everything else —
    /// the glyphs never reach the HUD's own bar, but a glyph outside `ALL` would be clipped by
    /// a frame worked out without it.
    Star,
    StarOutline,
}

impl Icon {
    pub const ALL: [Icon; 14] = [
        Icon::Volume,
        Icon::VolumeZero,
        Icon::VolumeMuted,
        Icon::Brightness,
        Icon::BlueLight,
        Icon::FastForward,
        Icon::FastForwardLatched,
        Icon::Rewind,
        Icon::Alert,
        Icon::Charging,
        Icon::Sun,
        Icon::Moon,
        Icon::Star,
        Icon::StarOutline,
    ];

    /// Position in `ALL`, which is the order faces are uploaded in. Sound only while `ALL` is
    /// in declaration order, which `icons_are_indexed_in_declaration_order` holds it to.
    pub fn index(self) -> usize {
        self as usize
    }

    pub fn glyph(self) -> char {
        match self {
            Icon::Volume => '\u{f028}',
            // A bare cone for nothing coming out, and a struck one for silenced. The two
            // were one glyph, which made a level walked down to zero and a mute look
            // identical on a bar that is empty in both cases.
            Icon::VolumeZero => '\u{f026}',
            Icon::VolumeMuted => '\u{f075f}',
            Icon::Brightness => '\u{f185}',
            Icon::BlueLight => '\u{f186}',
            // Same silhouette at the same width, differing only in fill, so the badge
            // never reflows between the two and the weight carries the meaning: hollow
            // while the finger is down, solid once it is latched on.
            Icon::FastForward => '\u{f06d2}',
            Icon::FastForwardLatched => '\u{f0211}',
            Icon::Rewind => '\u{f04a}',
            Icon::Alert => '\u{f0026}',
            // Beside the capsule, in a slot the gauge reserves whether or not a cable is in,
            // so the gauge and the percent never move when one goes in.
            Icon::Charging => '\u{f0e7}',
            Icon::Sun => '\u{f0599}',
            Icon::Moon => '\u{f0594}',
            Icon::Star => '\u{f005}',
            Icon::StarOutline => '\u{f006}',
        }
    }

    /// The glyph for a mode: the sun while the frontend is light and the moon while it is
    /// dark, because the icon names the mode the device is *in* rather than the one a press
    /// would move it to. A phone's status bar does the same, and for the same reason: the
    /// reading is a state, and the control is the press that changes it.
    pub fn of_mode(mode: crate::palette::Mode) -> Icon {
        match mode {
            crate::palette::Mode::Dark => Icon::Moon,
            crate::palette::Mode::Light => Icon::Sun,
        }
    }
}

/// A one pixel halo, dilated out of the coverage itself. Glyphs and toasts are drawn
/// over a live game frame and neither sits on a plate, so each carries its own contrast — the
/// same colour as the type is not, which is what `palette::halo` is for: a light glyph needs a
/// dark outline against a bright frame and a dark one needs a light outline, and the mode
/// decides which of those two the screen is.
pub const HALO_PX: u32 = 1;

/// Tints a coverage map and puts the halo behind it. `HALO_PX` wider and taller on every
/// side than the coverage it is given, so the dilation has somewhere to go.
pub fn haloed(cov: &[u8], cw: u32, ch: u32, colour: [u8; 3]) -> CartFace {
    let halo_ink = crate::palette::halo();
    let pad = HALO_PX as usize;
    let (cw, ch) = (cw as usize, ch as usize);
    let (w, h) = (cw + 2 * pad, ch + 2 * pad);
    let at = |x: isize, y: isize| -> u8 {
        if x < 0 || y < 0 || x >= cw as isize || y >= ch as isize {
            0
        } else {
            cov[y as usize * cw + x as usize]
        }
    };

    let mut rgba = Vec::with_capacity(w * h * 4);
    for y in 0..h {
        for x in 0..w {
            let (gx, gy) = (x as isize - pad as isize, y as isize - pad as isize);
            let ink = at(gx, gy);
            // The halo is the ink dilated by one pixel, so it only ever shows where the ink
            // does not already cover.
            let mut halo = 0u8;
            for dy in -1..=1 {
                for dx in -1..=1 {
                    halo = halo.max(at(gx + dx, gy + dy));
                }
            }
            if ink > 0 {
                let a = ink as u32;
                let inv = 255 - a;
                let mix = |c: u8, s: u8| ((c as u32 * a + s as u32 * inv) / 255) as u8;
                rgba.extend_from_slice(&[
                    mix(colour[0], halo_ink[0]),
                    mix(colour[1], halo_ink[1]),
                    mix(colour[2], halo_ink[2]),
                    ink.max(halo),
                ]);
            } else {
                rgba.extend_from_slice(&[halo_ink[0], halo_ink[1], halo_ink[2], halo]);
            }
        }
    }
    CartFace {
        rgba,
        w: w as u32,
        h: h as u32,
    }
}

/// Rasterised RGBA at `px` tall, transparent everywhere the glyph and its halo do not cover.
/// Two pixels wider and taller than the glyph, for the halo.
pub fn icon_face(icon: Icon, px: f32, colour: [u8; 3]) -> CartFace {
    let Some(r) = raster(icon, px) else {
        return CartFace {
            rgba: Vec::new(),
            w: 0,
            h: 0,
        };
    };
    haloed(&r.cov, r.w, r.h, colour)
}

/// The box every icon at this size is rastered into, for callers laying out around one before
/// they know which it will be. Zero when the font is missing, which costs the glyph its slot
/// rather than the row its shape.
pub fn icon_box(px: f32) -> (u32, u32) {
    match raster(Icon::Volume, px) {
        Some(r) => (r.w + 2 * HALO_PX, r.h + 2 * HALO_PX),
        None => (0, 0),
    }
}

/// Coverage only. The tint is applied per call, so two colours of one icon share a raster.
struct Raster {
    cov: Vec<u8>,
    w: u32,
    h: u32,
}

type Cache = Mutex<HashMap<(Icon, u32), Arc<Raster>>>;

fn raster(icon: Icon, px: f32) -> Option<Arc<Raster>> {
    static CACHE: OnceLock<Cache> = OnceLock::new();
    let cache = CACHE.get_or_init(Mutex::default);
    let key = (icon, px.to_bits());
    let mut cache = cache.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(r) = cache.get(&key) {
        return Some(r.clone());
    }
    let r = Arc::new(rasterise(icon, px)?);
    cache.insert(key, r.clone());
    Some(r)
}

fn rasterise(icon: Icon, px: f32) -> Option<Raster> {
    let font = symbols_font()?;
    let f = frame(font, px);
    let (m, cov) = font.rasterize(icon.glyph(), px);
    let x0 = m.xmin - f.left;
    let y0 = f.top - (m.ymin + m.height as i32);

    // Unclipped: the frame is the union of every icon's extent, so the glyph always lands
    // inside it and a frame that ever stopped holding one would panic here rather than
    // quietly shave a row off.
    let mut out = vec![0u8; (f.w * f.h) as usize];
    for gy in 0..m.height {
        for gx in 0..m.width {
            let (dx, dy) = ((x0 + gx as i32) as u32, (y0 + gy as i32) as u32);
            out[(dy * f.w + dx) as usize] = cov[gy * m.width + gx];
        }
    }
    Some(Raster {
        cov: out,
        w: f.w,
        h: f.h,
    })
}

struct Frame {
    w: u32,
    h: u32,
    left: i32,
    top: i32,
}

/// The union of every icon's extent at this size, so one box holds them all: swapping one
/// glyph for another moves nothing around it, and none is clipped. The line box will not do,
/// since these glyphs overshoot ascent and descent by a row.
fn frame(font: &Font, px: f32) -> Frame {
    let (mut left, mut right, mut top, mut bottom) = (i32::MAX, i32::MIN, i32::MIN, i32::MAX);
    for icon in Icon::ALL {
        let m = font.metrics(icon.glyph(), px);
        left = left.min(m.xmin);
        right = right.max((m.xmin + m.width as i32).max(m.advance_width.ceil() as i32));
        top = top.max(m.ymin + m.height as i32);
        bottom = bottom.min(m.ymin);
    }
    Frame {
        w: (right - left).max(1) as u32,
        h: (top - bottom).max(1) as u32,
        left,
        top,
    }
}

pub(crate) fn symbols_font() -> Option<&'static Font> {
    static FONT: OnceLock<Option<Font>> = OnceLock::new();
    FONT.get_or_init(|| Font::from_bytes(SYMBOLS_TTF, FontSettings::default()).ok())
        .as_ref()
}
