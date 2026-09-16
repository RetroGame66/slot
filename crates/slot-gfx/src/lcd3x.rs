use std::f32::consts::PI;

const BRIGHTEN_SCANLINES: f32 = 16.0;
const BRIGHTEN_LCD: f32 = 4.0;

/// LCD3x modulates by sin of the output pixel index, so at exactly 3x it repeats every 3
/// pixels on both axes and the whole shader collapses to this table sampled with GL_REPEAT.
/// Only true at 3x; any other scale needs the per pixel sin back.
pub fn lcd3x_mask() -> [[[f32; 3]; 3]; 3] {
    let mut mask = [[[0.0f32; 3]; 3]; 3];
    for (oy, row) in mask.iter_mut().enumerate() {
        let yfactor = (BRIGHTEN_SCANLINES + (PI * (oy as f32 + 0.5) * 2.0 / 3.0).sin())
            / (BRIGHTEN_SCANLINES + 1.0);
        for (ox, cell) in row.iter_mut().enumerate() {
            for (c, v) in cell.iter_mut().enumerate() {
                let xfactor = (BRIGHTEN_LCD
                    + (PI * (ox as f32 + 0.5) * 2.0 / 3.0 + c as f32 * 2.0 * PI / 3.0).sin())
                    / (BRIGHTEN_LCD + 1.0);
                *v = yfactor * xfactor;
            }
        }
    }
    mask
}

/// The table as an RGBA8 texture, row major, alpha opaque.
pub fn mask_texture_rgba8() -> [u8; 3 * 3 * 4] {
    let mask = lcd3x_mask();
    let mut tex = [255u8; 3 * 3 * 4];
    for (texel, cell) in tex.chunks_exact_mut(4).zip(mask.iter().flatten()) {
        for (out, v) in texel.iter_mut().zip(cell) {
            *out = (v * 255.0).round() as u8;
        }
    }
    tex
}

/// The built-in table as the `[[[u8; 3]; 3]; 3]` the rest of the tree passes around, so the
/// card's mask and the shipped one share one type and the app can fall back to it.
pub fn builtin_panel_mask() -> [[[u8; 3]; 3]; 3] {
    let m = lcd3x_mask();
    let mut out = [[[0u8; 3]; 3]; 3];
    for (row, mrow) in out.iter_mut().zip(m.iter()) {
        for (cell, mcell) in row.iter_mut().zip(mrow.iter()) {
            for (o, v) in cell.iter_mut().zip(mcell.iter()) {
                *o = (v * 255.0).round().clamp(0.0, 255.0) as u8;
            }
        }
    }
    out
}
