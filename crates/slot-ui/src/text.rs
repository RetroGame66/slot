use std::sync::OnceLock;

use fontdue::{Font, FontSettings};

const LABEL_TTF: &[u8] = include_bytes!("../assets/label.ttf");

pub struct Layout {
    pub lines: Vec<String>,
    pub px: f32,
    pub tracking: f32,
}

/// The card's face, if it supplied a usable one. Set once at boot by `set_font`; an empty slot
/// means "no card font", and every glyph then falls back to the embedded face.
static USER_FONT: OnceLock<Font> = OnceLock::new();
/// The face baked into the binary. Always present; the fallback for any glyph a card face lacks
/// (most CJK faces ship without Latin), so a Chinese title and an English code both render
/// instead of one blanking the other.
static EMBEDDED_FONT: OnceLock<Font> = OnceLock::new();

/// Set the interface's typeface from the bytes of a TTF, OTF or TTC. Called once at boot.
/// Only a parse that succeeds takes effect; a file that will not parse is ignored and the
/// embedded face remains the fallback. A `.ttc` collection works via `FontSettings::default()`
/// (face 0). A second call is ignored.
pub fn set_font(bytes: Vec<u8>) {
    // Leaked on purpose. `Font::from_bytes` borrows the bytes for as long as the font lives,
    // and the font lives as long as the process; the alternative is a self-referential struct
    // for a single allocation that would never be freed either way.
    let data: &'static [u8] = Box::leak(bytes.into_boxed_slice());
    if let Ok(font) = Font::from_bytes(data, FontSettings::default()) {
        let _ = USER_FONT.set(font);
    }
}

/// The embedded face, initialised on first need.
fn embedded_font() -> &'static Font {
    EMBEDDED_FONT.get_or_init(|| {
        Font::from_bytes(LABEL_TTF, FontSettings::default()).expect("embedded label.ttf must parse")
    })
}

/// Whether `font` actually carries `ch`, rather than only its `.notdef` box.
fn has_glyph(font: &Font, ch: char) -> bool {
    font.rasterize(ch, 1.0).0.width > 0
}

/// The characters in `text` that neither face can draw, in order of first appearance.
///
/// The card's face is a subset, cut to the titles the card carries; a card that gains a game
/// with a character outside it loses that character silently, because a missing glyph draws as
/// nothing rather than as an error. Checking is per character and does not rasterise a whole
/// title, so every title on the card can be asked at boot.
pub fn missing_in(text: &str) -> Vec<char> {
    let mut out = Vec::new();
    for ch in text.chars() {
        if ch == '\n' || ch.is_whitespace() {
            continue;
        }
        let user_has = USER_FONT.get().is_some_and(|f| has_glyph(f, ch));
        if !user_has && !has_glyph(embedded_font(), ch) && !out.contains(&ch) {
            out.push(ch);
        }
    }
    out
}

/// The face to draw one glyph with: the card's if it has that glyph, otherwise the embedded one.
/// This is what lets a card carry a CJK face (for the Chinese title) without also blanking the
/// Latin UI — codes and labels — that the embedded face still covers.
fn font_for(ch: char) -> &'static Font {
    if let Some(user) = USER_FONT.get() {
        if has_glyph(user, ch) {
            return user;
        }
    }
    embedded_font()
}

/// The interface's nominal typeface: the card's if one was supplied at boot, else the embedded
/// face (always present). Returns `None` only if even the embedded face failed to parse, which
/// should never happen. Per-glyph rasterisation inside `coverage` may still switch to the other
/// face for a glyph this one lacks, so a Chinese title and a Latin code render from one baseline.
pub fn label_font() -> Option<&'static Font> {
    USER_FONT.get().or_else(|| Some(embedded_font()))
}

/// Largest whole pixel size at which `text` wraps into `max_lines` or fewer whole words per
/// line. Shrinking is preferred to hyphenless mid-word breaks, so a long title reads smaller
/// rather than scrambled. Only a title that will not fit even at `min_px` is broken between
/// characters and clipped to `max_lines`.
pub fn fit(
    font: &Font,
    text: &str,
    max_w: f32,
    max_lines: usize,
    max_px: f32,
    min_px: f32,
) -> Layout {
    let upper = text.to_uppercase();
    let mut px = max_px;
    while px > min_px {
        let tracking = tracking_for(px);
        let lines = wrap(font, &upper, px, tracking, max_w, false);
        if lines.len() <= max_lines
            && lines
                .iter()
                .all(|l| line_width(font, l, px, tracking) <= max_w)
        {
            return Layout {
                lines,
                px,
                tracking,
            };
        }
        px -= 1.0;
    }
    let tracking = tracking_for(min_px);
    Layout {
        lines: wrap(font, &upper, min_px, tracking, max_w, true)
            .into_iter()
            .take(max_lines)
            .collect(),
        px: min_px,
        tracking,
    }
}

pub fn draw_centred(dst: &mut [u8], dst_w: u32, dst_h: u32, layout: &Layout, colour: [u8; 3]) {
    for (i, a) in coverage(dst_w, dst_h, layout).into_iter().enumerate() {
        if a > 0 {
            blend(&mut dst[i * 4..i * 4 + 4], a as u32, colour);
        }
    }
}

/// The same type as `draw_centred` lays down, as one byte of ink per pixel and no colour.
/// What a caller needs to dilate a halo out of the type before tinting it.
pub fn coverage(dst_w: u32, dst_h: u32, layout: &Layout) -> Vec<u8> {
    let mut out = vec![0u8; (dst_w * dst_h) as usize];
    let Some(font) = label_font() else {
        return out;
    };
    let Some(vm) = font.horizontal_line_metrics(layout.px) else {
        return out;
    };
    let line_h = vm.new_line_size;
    let block_h = line_h * layout.lines.len() as f32;
    let mut baseline = (dst_h as f32 - block_h) / 2.0 + vm.ascent;

    for line in &layout.lines {
        let mut pen = (dst_w as f32 - line_width(font, line, layout.px, layout.tracking)) / 2.0;
        let mut prev: Option<char> = None;
        for ch in line.chars() {
            // Spaced before the glyph, not after, so the two metric paths stay the same
            // arithmetic and cannot drift apart.
            pen += pair_tracking(prev, ch, layout.tracking);
            let g = font_for(ch);
            let (m, cov) = g.rasterize(ch, layout.px);
            stamp(
                &mut out,
                dst_w,
                dst_h,
                (pen + m.xmin as f32).round() as i32,
                (baseline - (m.height as f32 + m.ymin as f32)).round() as i32,
                &cov,
                m.width as u32,
                m.height as u32,
            );
            pen += m.advance_width;
            prev = Some(ch);
        }
        baseline += line_h;
    }
    out
}

/// `font` is the layout's nominal face, kept for call-site compatibility; the measured width
/// uses `font_for` per glyph, so a mixed-script line's width matches what `coverage` draws.
pub fn line_width(_font: &Font, line: &str, px: f32, tracking: f32) -> f32 {
    let mut w = 0.0;
    let mut prev: Option<char> = None;
    for ch in line.chars() {
        w += pair_tracking(prev, ch, tracking) + font_for(ch).metrics(ch, px).advance_width;
        prev = Some(ch);
    }
    w
}

fn tracking_for(px: f32) -> f32 {
    (px * 0.10).round()
}

/// Whether a character is set without spaces around it, which is what decides both where a
/// line may break and whether it gets the Latin letter spacing. These are the CJK blocks
/// proper plus hangul and the fullwidth forms, not a general "non-Latin" test: a Greek or
/// Cyrillic title is still spaced type.
pub(crate) fn is_cjk(c: char) -> bool {
    matches!(c as u32,
        0x1100..=0x11FF   // hangul jamo
        | 0x2E80..=0x2EFF // CJK radicals
        | 0x3000..=0x303F // CJK punctuation
        | 0x3040..=0x30FF // hiragana, katakana
        | 0x3130..=0x318F // hangul compatibility jamo
        | 0x31C0..=0x31EF // CJK strokes
        | 0x3400..=0x4DBF // CJK extension A
        | 0x4E00..=0x9FFF // CJK unified ideographs
        | 0xA960..=0xA97F // hangul jamo extended A
        | 0xAC00..=0xD7FF // hangul syllables
        | 0xF900..=0xFAFF // CJK compatibility ideographs
        | 0xFE30..=0xFE4F // CJK compatibility forms
        | 0xFF00..=0xFF60 // fullwidth forms
        | 0xFFE0..=0xFFE6
        | 0x20000..=0x2FA1F // CJK extensions B onwards
    )
}

/// The letter spacing between two neighbours. The layout's, except where either side is CJK:
/// 10% of the em is set for Latin and reads as a gap wedged between ideographs, which carry
/// their own rhythm on the square they are drawn in.
pub(crate) fn pair_tracking(prev: Option<char>, ch: char, tracking: f32) -> f32 {
    if tracking == 0.0 || prev.is_some_and(is_cjk) || is_cjk(ch) {
        0.0
    } else {
        tracking
    }
}

/// A breakable piece of a line, and whether a space separated it from the one before.
///
/// Whitespace is the only break opportunity Latin gives; CJK gives none at all. A Chinese
/// title under a whitespace splitter is one token, so the fitter shrinks it all the way to
/// `min_px` before it can break it anywhere, and a name that reads fine at 20 px arrives at
/// 10. Cutting at every ideograph is what the script does anyway.
struct Token {
    text: String,
    space_before: bool,
}

fn tokens(text: &str) -> Vec<Token> {
    let mut out = Vec::new();
    let mut run = String::new();
    let mut pending_space = false;
    for ch in text.chars() {
        if ch.is_whitespace() {
            if !run.is_empty() {
                out.push(Token {
                    text: std::mem::take(&mut run),
                    space_before: pending_space,
                });
            }
            // Set after the run is flushed, not before: every token still to come is separated
            // from the one before it by this space, and the run that just ended was not.
            pending_space = true;
            continue;
        }
        if is_cjk(ch) {
            if !run.is_empty() {
                out.push(Token {
                    text: std::mem::take(&mut run),
                    space_before: pending_space,
                });
                pending_space = false;
            }
            out.push(Token {
                text: ch.to_string(),
                space_before: pending_space,
            });
            pending_space = false;
            continue;
        }
        run.push(ch);
    }
    if !run.is_empty() {
        out.push(Token {
            text: run,
            space_before: pending_space,
        });
    }
    out
}

/// Greedy word wrap. With `break_words`, a word wider than `max_w` on its own is split
/// between characters so every line fits; without it that word overflows its line and the
/// caller sees a line wider than `max_w`.
fn wrap(
    font: &Font,
    text: &str,
    px: f32,
    tracking: f32,
    max_w: f32,
    break_words: bool,
) -> Vec<String> {
    let mut lines: Vec<String> = Vec::new();
    let mut line = String::new();
    for token in tokens(text) {
        let parts = if break_words {
            break_word(font, &token.text, px, tracking, max_w)
        } else {
            vec![token.text.clone()]
        };
        for (i, part) in parts.iter().enumerate() {
            // The space belongs between the tokens it separated, so it goes back only where
            // the token it preceded stayed on this line. A continuation of a broken word
            // never gets one.
            let sep = if line.is_empty() {
                ""
            } else if i == 0 && token.space_before {
                " "
            } else {
                ""
            };
            let joined = format!("{line}{sep}{part}");
            if line.is_empty() || line_width(font, &joined, px, tracking) <= max_w {
                line = joined;
            } else {
                lines.push(std::mem::take(&mut line));
                line = part.clone();
            }
        }
    }
    if !line.is_empty() {
        lines.push(line);
    }
    lines
}

fn break_word(font: &Font, word: &str, px: f32, tracking: f32, max_w: f32) -> Vec<String> {
    if line_width(font, word, px, tracking) <= max_w {
        return vec![word.to_string()];
    }
    let mut parts = Vec::new();
    let mut part = String::new();
    for ch in word.chars() {
        part.push(ch);
        if line_width(font, &part, px, tracking) > max_w && part.chars().count() > 1 {
            part.pop();
            parts.push(std::mem::take(&mut part));
            part.push(ch);
        }
    }
    if !part.is_empty() {
        parts.push(part);
    }
    parts
}

/// Ink laid into a coverage buffer, taking the greater of what is there and what arrives.
/// Adjacent glyphs only ever touch at their antialiased edges, where a sum would darken the
/// seam into a visible join.
#[allow(clippy::too_many_arguments)]
fn stamp(dst: &mut [u8], dst_w: u32, dst_h: u32, x: i32, y: i32, cov: &[u8], gw: u32, gh: u32) {
    for gy in 0..gh {
        let dy = y + gy as i32;
        if dy < 0 || dy >= dst_h as i32 {
            continue;
        }
        for gx in 0..gw {
            let dx = x + gx as i32;
            if dx < 0 || dx >= dst_w as i32 {
                continue;
            }
            let i = (dy as u32 * dst_w + dx as u32) as usize;
            dst[i] = dst[i].max(cov[(gy * gw + gx) as usize]);
        }
    }
}

/// Source over, carrying the destination's own alpha, so type lands on a transparent buffer
/// as ink rather than as ink faded toward black. Over an opaque buffer it collapses to the
/// plain coverage blend.
fn blend(px: &mut [u8], a: u32, colour: [u8; 3]) {
    let under = px[3] as u32 * (255 - a) / 255;
    let out = a + under;
    for c in 0..3 {
        px[c] = ((colour[c] as u32 * a + px[c] as u32 * under + out / 2) / out) as u8;
    }
    px[3] = out as u8;
}
