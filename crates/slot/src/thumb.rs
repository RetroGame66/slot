/// The polaroid picture: one emulated frame, PNG, at its own size. There is nothing to
/// downscale because the core's frame is already whatever the switcher shows — 240x160 for
/// the GBA, 160x144 for Game Boy and Game Boy Color.
///
/// The dimensions are passed in rather than read off a constant: the buffer `video_refresh`
/// packs is tightly laid out at the *frame's* width, so the only honest size to cut it at is
/// the one the core actually produced, which `av_info` reports. Passing the wrong size would
/// re-introduce exactly the row-misalignment the stride fix removed.
///
/// `xrgb8888` is libretro's frame buffer, little endian, so its bytes arrive B, G, R, X.
pub fn png(xrgb8888: &[u8], w: u32, h: u32) -> Option<Vec<u8>> {
    let n = (w * h) as usize;
    if xrgb8888.len() < n * 4 {
        return None;
    }
    let mut rgb = Vec::with_capacity(n * 3);
    for px in xrgb8888[..n * 4].chunks_exact(4) {
        rgb.extend_from_slice(&[px[2], px[1], px[0]]);
    }

    let mut out = Vec::new();
    let mut enc = png::Encoder::new(&mut out, w, h);
    enc.set_color(png::ColorType::Rgb);
    enc.set_depth(png::BitDepth::Eight);
    let mut writer = enc.write_header().ok()?;
    writer.write_image_data(&rgb).ok()?;
    writer.finish().ok()?;
    Some(out)
}
