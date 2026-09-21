//! The shelf's letter strip: a second ring, above the cart row and on the same axis.
//!
//! A Chinese library has no order of its own — the names are hanzi and the card hands them over
//! in whatever order the directory listing has them — so the way to a particular game is a
//! bucket. The shelf itself stays exactly what it was: one cart ring, left and right, seven
//! carts wide. This is the index above it, and it is the *same* ring one level up, which is
//! why it reads without being explained: the marker in the middle is the letter the cart under
//! the caret belongs to, and pressing left or right moves the marker and the shelf follows.
//!
//! It used to be a drum — a second ring standing on its edge in a housing of its own, seven
//! facets either side of the marker, each turned away from the eye by so many degrees. The
//! turning was the one thing on the shelf that had no counterpart in the machine, and the strip
//! it was drawn to replace is what it read as anyway. What took its place is the machine's own
//! band at the top of the screen (`slot_chrome::draw_top_band`), mirrored from the cart bay at
//! the bottom, with the letters laid flat inside its window: the same ring, read as an index
//! printed along the case rather than as a wheel behind it.
//!
//! What came back with it is the *metal*. The drum's facets were separated by a raised ridge
//! with a boss at each end — one piece of metal, seen at the three depths a seam, a ridge and a
//! tooth are — and losing the drum lost that too, which left a row of letters with nothing
//! between them. Laid flat, the same ridge is what makes the run read as a machined strip
//! rather than as a line of type that happens to be in a slot. It is the one part of the old
//! dial that was about the case rather than about the turning, so it is the one part that
//! survives the ring being unrolled: the ridges no longer foreshorten, because nothing does any
//! more, but they are the same shape at the same pitch and they travel with the letters.
//!
//! Every slot the alphabet has room for is laid out, whether or not the card uses it. A strip
//! that closed up its gaps would put B next to C one cart apart on one card and two hundred
//! apart on the next, and the distances would stop meaning anything; empty slots are drawn
//! dim, are never centred, and are stepped over. `#` holds everything the table cannot read and
//! the digits hold titles that start with a number.

use crate::cart::CartFace;
use crate::draw::{Draw, TexId};
use crate::hud::Millis;

/// Every bucket on the ring, in ring order. `#` first, and it holds everything that is not a
/// letter: a title that opens with a digit, punctuation, or a character the pinyin table does
/// not know. Putting it at an end keeps it out of the way of A, and keeping the digits off the
/// ring at all is what leaves twenty-seven slots — the alphabet and the catch-all — which is
/// an index a hand can cross in a moment rather than a keyboard.
pub const SLOTS: [char; 27] = [
    '#', 'A', 'B', 'C', 'D', 'E', 'F', 'G', 'H', 'I', 'J', 'K', 'L', 'M', 'N', 'O', 'P', 'Q', 'R',
    'S', 'T', 'U', 'V', 'W', 'X', 'Y', 'Z',
];

/// How many slots the ring has. Also how many counts and firsts a caller keeps.
pub const N: usize = SLOTS.len();

/// Which slot a cart's initial belongs in. Anything that is not on the ring lands under `#`,
/// which is every digit, every mark, and every character the pinyin table does not know.
pub fn slot_of(initial: char) -> usize {
    let c = initial.to_ascii_uppercase();
    SLOTS.iter().position(|s| *s == c).unwrap_or(0)
}

/// The letter a slot draws. `#` is shown as it is.
pub fn slot_label(slot: usize) -> char {
    SLOTS.get(slot).copied().unwrap_or('#')
}

// ---------------------------------------------------------------------------------------
// Layout
// ---------------------------------------------------------------------------------------

/// The whole widget, cut to four fifths.
///
/// The letters were sized to the window rather than to the band, and at full size the tallest of
/// them came close enough to the band's lower lip that the eye read the strip as standing *on*
/// that line instead of inside the opening above it — the line looked interrupted by the ink
/// even though nothing crossed it. One knob rather than five: every dimension that describes the
/// strip's shape multiplies by this, so the proportions stay the ones that were tuned and the
/// lip keeps its distance at a size the band can carry.
const SCALE: f32 = 0.8;

/// How far apart two slots stand. The drum got its spacing from its own turning and needed no
/// number of its own; laid flat along the band the strip has to be told one. A little over the
/// marker's cap height, so the lit letter keeps some air on either side of it and the run does
/// not read as a word.
///
/// This used to be thirty, and the number had a second job: `VISIBLE` slots either side of the
/// marker came to exactly `cart::CART_W`, so the strip was the width of one cart on the row
/// below. At four fifths it is deliberately narrower than that — the equality is gone on
/// purpose, and what ends the strip now is the fade, not the window's edge.
const PITCH: f32 = 30.0 * SCALE;

/// Slots drawn either side of the marker.
const VISIBLE: i32 = 4;

/// Where the strip starts giving up its light, and where it has none left, in pixels from the
/// middle.
///
/// Measured in slots rather than in pixels, which is the one pair of numbers here that is *not*
/// multiplied by `SCALE`: the fade is a property of the strip's own shape — how many letters
/// out it starts dissolving — so it has to follow the pitch or a rescale silently kills it. It
/// very nearly did: scaled, the outermost slot would have come out at 96 against a threshold of
/// 104, every letter at full ink, and the strip would have ended on a hard edge inside the
/// window, which is exactly what this exists to prevent. Fading is what keeps a letter from
/// being cut off by a straight line, the one thing that would give the band away as a rectangle
/// drawn over the screen rather than an opening in the case.
///
/// Three slots out to four and a half: the fourth letter each side arrives at a third of its
/// ink and the fifth is gone, so the run dissolves before the window's edge rather than at it.
const FADE_FROM: f32 = 3.0 * PITCH;
const FADE_TO: f32 = 4.5 * PITCH;

/// How tall the whole widget is: the letters and the metal between them, from the top of a ridge
/// to the bottom of one.
///
/// Sized off the window rather than off the type, and inset from it the way the drum's ridges
/// were inset from its housing: the ridges are the frame the letters are read through, so their
/// ends are what has to sit inside the opening rather than touch it. Thirty of the window's
/// thirty-six, three clear at each end.
const MODULE_H: f32 = crate::slot_chrome::TOP_WIN_H - 2.0 * RIDGE_INSET;

/// Where the widget's middle is: the middle of the *band*, not of the window.
///
/// The window is not centred in the band — it stands twelve pixels down from the top of a
/// fifty-eight pixel band and ten up from the bottom — so a strip centred on the window reads as
/// sitting low in the case, which is what it looked like. The band is the thing the eye sees as
/// one object, so the band is what the strip is centred on.
///
/// This is also what keeps the lip honest without a nudge. The old strip carried a three pixel
/// lift purely to get its ink off that line, because centred on the window's middle it was
/// closer to the bottom of the opening than to the top. Centred on the band, the bottom of the
/// widget lands at 44 against a lip at 56 — twelve clear, the same margin the lip test asks of
/// the letters — so the lift has nothing left to do and is gone.
const MODULE_MID: f32 = crate::slot_chrome::TOP_BAND_H / 2.0;

/// The ring's spring. Snappier than the cart row's: a strip is a small thing, and a letter that
/// took as long to arrive as a cartridge does would not read as an index being stepped through.
const OMEGA: f32 = 20.0;

/// Before a held key steps a second letter. Longer than the cart row's: a press is a
/// deliberate move to a named letter, and two of those from one tap would be two letters of
/// an alphabet the user is reading.
const REPEAT_DELAY_MS: Millis = 450;

/// Between repeats after that. Faster than the cart row's — an index is a coarse control, and
/// the whole alphabet should be a couple of seconds away rather than twenty.
const REPEAT_MS: Millis = 130;

/// The ink a letter is drawn in. The palette's, because the window it is set on is the case's
/// `opening` and that flips with the mode — a fixed light ink would be dark type's background in
/// one mode and its own colour in the other.
fn ink() -> [u8; 3] {
    crate::palette::ink()
}

/// The size a letter is rasterised at, before it is cropped to its ink and scaled to one of
/// the two sizes below.
///
/// Larger than either of them on purpose: the face is built once and drawn at both, so it has
/// to be whichever is bigger, and drawing down is the direction that keeps an edge clean.
pub const FACE_PX: f32 = 40.0;

/// The cap height a letter is drawn at: the marker's, and its neighbours'.
///
/// These are heights and not box sizes, because the face is cropped to its ink first — a
/// letter drawn at a size that included its own leading would come out at about half of it,
/// which is an index set in type too small to read from where a handheld is held.
///
/// The marker's is also what the band's window has to carry: at full size it was twenty-six in
/// a thirty-six pixel opening, five pixels of air above and below, and that was too tight
/// against the lip. At four fifths it is 20.8 — five and a half clear of the ridge ends above
/// and below it, and twenty-two clear of the lit lip, which is air the strip did not have when
/// it was centred on the window rather than on the band.
const CENTRE_PX: f32 = 26.0 * SCALE;
const NEIGHBOUR_PX: f32 = 18.0 * SCALE;

/// Ink for the three states, as a fraction of the letters' own colour.
///
/// Only brightness, and deliberately: a slot with nothing in it still has to read as a slot, so
/// it cannot be invisible, and the marker has to read as *chosen*, so the neighbour cannot be
/// near it. The other reason there is no hue here is that the tree has exactly one warning
/// colour — `ALERT_INK` — and an index that borrowed it would spend it on nothing.
///
/// The floor is set by the worst background rather than the best: the window is `opening`, which
/// is nearly black, so ink much below a third of the letters' own luminance disappears into it.
const EMPTY: f32 = 0.34;
const NEIGHBOUR: f32 = 0.68;

// ---------------------------------------------------------------------------------------
// The metal between two slots
// ---------------------------------------------------------------------------------------

/// One ridge and its size, in pixels: the raised knuckle between two facets.
///
/// It is not a line but a piece of metal seen at three heights — a beveled crest down the
/// middle, and a wider boss at each end where it meets the rim. That boss is the tooth, and
/// having it and the crest be one texture is what made the drum and the gear the same object:
/// a seam, a ridge and a tooth are the same ridge passed at different depths, so they are not
/// three things to keep in agreement. Laid flat there is only one depth left, and the shape is
/// the same shape.
const RIDGE_W: u32 = 9;

/// How far a ridge stops short of the window either end, and so what `MODULE_H` is measured
/// back from. The drum's own inset, kept: the metal ends where the opening does rather than
/// running into the frame around it.
const RIDGE_INSET: f32 = 3.0;

/// The ink a ridge is drawn in, and its peak alpha.
///
/// The peak is deliberately below the dimmest letter: a slot with nothing in it is ink at
/// `EMPTY`, and this is metal *behind* the letters rather than another state of them. A ridge as
/// bright as an empty slot would be a strip where every seam looked like a letter nobody can
/// read.
///
/// The colour is the palette's, because metal behind light type and metal behind dark type are
/// not the same metal: light in the dark mode, where it is a highlight on a dark window, and
/// dark in the light one, where the window has gone pale and a light ridge would vanish into it.
const RIDGE_PEAK: f32 = 0.20;

/// Where the metal starts giving up, and where it has none left, in pixels from the middle.
///
/// Further out than the letters' own fade, and that is the point rather than an accident. The
/// outermost ridge stands half a slot past the outermost letter — it is there so the strip
/// carries on beyond the last thing it can name, which is what the drum's row of teeth did — and
/// fading it on the letters' curve would put it exactly on zero: drawn, and invisible, with the
/// run ending on a letter and no seam beyond it. Measured in slots like the letters' fade, so a
/// rescale cannot silently kill this one either.
const RIDGE_FADE_FROM: f32 = 3.5 * PITCH;
const RIDGE_FADE_TO: f32 = 5.5 * PITCH;

/// The metal is a highlight behind the letters, never another state of them: a ridge as bright as
/// an empty slot would be a strip where every seam looked like a letter nobody can read. Held as
/// a fact the compiler checks rather than as a test, the way `battery` holds its gauge's
/// proportions — the two numbers are what would drift, and they drift together in a diff.
const _: () = assert!(RIDGE_PEAK < EMPTY);

// ---------------------------------------------------------------------------------------
// Faces
// ---------------------------------------------------------------------------------------

/// One letter, rasterised and then cropped to its ink.
///
/// The crop is what makes the sizes above mean anything. Rasterised at a type size, a capital
/// comes out about three quarters of it tall, sitting inside a box as tall as the type — so a
/// face drawn at a stated size is a letter a quarter smaller than the stated size, on every
/// letter, with the gap between neighbours set by the box rather than by the letter. Cropped,
/// the texture *is* the letter, and drawing it at a height draws the letter at that height.
///
/// Built once per slot at boot: they are tiny, fixed, and the shelf is the only screen that
/// uses them.
pub fn letter_face(ch: char) -> CartFace {
    let box_px = (FACE_PX * 1.6).ceil() as u32;
    let mut rgba = vec![0u8; (box_px * box_px * 4) as usize];
    if let Some(font) = crate::text::label_font() {
        let text = ch.to_string();
        let layout = crate::text::fit(font, &text, box_px as f32, 1, FACE_PX, FACE_PX * 0.7);
        crate::text::draw_centred(&mut rgba, box_px, box_px, &layout, ink());
    }
    crop_to_ink(rgba, box_px, box_px)
}

/// One ridge with its teeth, as a face.
///
/// A crest down the middle — lit on the side the light comes from, in shadow on the other — and
/// a wider boss at each end. The boss is what reads as a tooth, and the whole thing is one
/// texture because a seam, a ridge and a tooth are the same piece of metal; drawn as three
/// shapes they would be three things that had to be kept in agreement.
///
/// Carried over from the drum unchanged in every way that is not a consequence of the ring
/// being unrolled: the same nine pixels across, the same seven-pixel boss, the same lit half and
/// shadowed half. What is gone is the depth — flat, every ridge is drawn at its full width and
/// full alpha rather than scaled by how far the drum has turned — which is what the letters
/// gave up when they were laid flat too.
pub fn ridge_face() -> CartFace {
    let (w, h) = (RIDGE_W, MODULE_H.round() as u32);
    let mut rgba = vec![0u8; (w * h * 4) as usize];
    let (fw, fh) = (w as f32, h as f32);
    let mid = fw / 2.0;
    let metal = crate::palette::ridge_ink();
    // How far down the ridge the boss ends and the crest starts.
    let boss = 7.0;
    for y in 0..h {
        for x in 0..w {
            let px = x as f32 + 0.5;
            let py = y as f32 + 0.5;
            // The knob is a rounded end; the crest is a thin shaft between two of them.
            let t = (py / boss).min((fh - py) / boss).clamp(0.0, 1.0);
            let width = 1.0 + 3.0 * t; // half-width of the shaft, in pixels
            let off = (px - mid).abs();
            if off > width {
                continue;
            }
            // Metal: the light comes from above, so the half towards the top of the ridge is
            // the lit one and the far half is in its own shadow.
            let across = (px - mid) / width;
            let face = if py < fh / 2.0 {
                1.0 - 0.55 * (across + 1.0) / 2.0
            } else {
                0.45 - 0.30 * (across + 1.0) / 2.0
            };
            let soft = (1.0 - (off / width).powi(3)).clamp(0.0, 1.0);
            let a = (RIDGE_PEAK * face.max(0.0) * soft * 255.0).round() as u8;
            let i = ((y * w + x) * 4) as usize;
            rgba[i..i + 4].copy_from_slice(&[metal[0], metal[1], metal[2], a]);
        }
    }
    CartFace { rgba, w, h }
}

/// The smallest box holding every pixel that got ink. A face with no ink at all comes back as
/// one transparent pixel rather than as an empty box, so a caller that divides by its size
/// does not divide by zero — the letter costs one pixel and draws as nothing.
fn crop_to_ink(rgba: Vec<u8>, w: u32, h: u32) -> CartFace {
    let (mut x0, mut y0, mut x1, mut y1) = (w, h, 0u32, 0u32);
    for y in 0..h {
        for x in 0..w {
            if rgba[((y * w + x) * 4 + 3) as usize] > 0 {
                x0 = x0.min(x);
                y0 = y0.min(y);
                x1 = x1.max(x + 1);
                y1 = y1.max(y + 1);
            }
        }
    }
    if x0 >= x1 || y0 >= y1 {
        return CartFace {
            rgba: vec![0; 4],
            w: 1,
            h: 1,
        };
    }
    let (cw, ch) = (x1 - x0, y1 - y0);
    let mut out = vec![0u8; (cw * ch * 4) as usize];
    for y in 0..ch {
        let src = ((y + y0) * w + x0) as usize * 4;
        let dst = (y * cw) as usize * 4;
        out[dst..dst + cw as usize * 4].copy_from_slice(&rgba[src..src + cw as usize * 4]);
    }
    CartFace {
        rgba: out,
        w: cw,
        h: ch,
    }
}

// ---------------------------------------------------------------------------------------
// State
// ---------------------------------------------------------------------------------------

/// The ring's position. Continuous, so the spring has somewhere to live, and wrapped on demand
/// the way the cart row wraps: the nearest representation of a target is the one it travels to.
pub struct Letters {
    scroll: f32,
    vel: f32,
    target: usize,
    /// The direction being held and when it next fires, in the same shape the cart row keeps
    /// it: the repeat lives here rather than in the gesture layer so that nothing in a game
    /// starts reading the left and right shoulders.
    held: Option<(i32, Millis)>,
}

impl Default for Letters {
    fn default() -> Self {
        Letters::new()
    }
}

impl Letters {
    pub fn new() -> Self {
        Letters {
            scroll: 0.0,
            vel: 0.0,
            target: 0,
            held: None,
        }
    }

    /// Which slot the marker is on.
    pub fn target(&self) -> usize {
        self.target
    }

    /// Where the strip is, continuously. Every part of it — where a letter sits and how much of
    /// it shows — is read off this, so they all move as one piece. The spring is what keeps it
    /// off the whole numbers.
    pub fn scroll(&self) -> f32 {
        self.scroll
    }

    /// The marker follows the cart under the caret. Called every frame: setting the same slot
    /// twice is a write of the same number.
    pub fn centre_on(&mut self, initial: char) {
        self.target = slot_of(initial);
    }

    /// Puts the marker on a letter without travelling there, for the first frame of a session:
    /// there is no previous position to travel from, and a strip that arrived from `#` would be
    /// the shelf's first movement being one nobody asked for.
    pub fn snap_to(&mut self, initial: char) {
        self.target = slot_of(initial);
        self.scroll = self.target as f32;
        self.vel = 0.0;
    }

    /// Where the spring is heading, expressed as the nearest way round the ring.
    fn scroll_target(&self) -> f32 {
        let n = N as f32;
        self.scroll + (self.target as f32 - self.scroll + n / 2.0).rem_euclid(n) - n / 2.0
    }

    pub fn settle(&mut self, dt: f32) {
        let dt = dt.clamp(0.0, 0.1);
        let accel = -2.0 * OMEGA * self.vel - OMEGA * OMEGA * (self.scroll - self.scroll_target());
        self.vel += accel * dt;
        self.scroll += self.vel * dt;
        // Land, rather than creeping: an exponential tail on an index is visible as a letter
        // that is almost in place.
        if (self.scroll - self.scroll_target()).abs() < 0.001 && self.vel.abs() < 0.01 {
            self.scroll = self.scroll_target();
            self.vel = 0.0;
        }
    }

    /// Step the marker to the next slot that has anything in it. Returns false when there is
    /// nothing to step to, which is a card with one letter on it.
    ///
    /// Empty slots are stepped *over*, not landed on: they are drawn so the spacing means
    /// something, not so the user can park on one.
    ///
    /// `N - 1` candidates and not `N`: walking the whole ring comes back to the slot it set out
    /// from, and a card whose entire library is one letter has every direction lead there. That
    /// is not a move, and reporting it as one leaves a held key repeating forever at an index
    /// that never moves.
    pub fn step(&mut self, by: i32, counts: &[usize; N]) -> bool {
        if by == 0 {
            return false;
        }
        let dir = by.signum();
        let mut at = self.target as i32;
        for _ in 0..N - 1 {
            at = (at + dir).rem_euclid(N as i32);
            if counts[at as usize] > 0 {
                self.target = at as usize;
                return true;
            }
        }
        false
    }

    /// A key going down: the press itself moves the marker, and the repeat is measured from
    /// the press rather than producing it. Returns whether the marker moved, which is what
    /// tells the caller to bring the shelf along.
    ///
    /// A press that cannot move anything — a card whose whole library is one letter — arms
    /// nothing, so a key held down does not sit there firing at a wall.
    pub fn hold(&mut self, by: i32, now: Millis, counts: &[usize; N]) -> bool {
        if !self.step(by, counts) {
            return false;
        }
        self.held = Some((by, now + REPEAT_DELAY_MS));
        true
    }

    /// Fires the repeat. Due from `now` rather than from the deadline it passed, so a frame
    /// the app was late for costs one letter instead of a burst of catching up.
    pub fn tick(&mut self, now: Millis, counts: &[usize; N]) -> bool {
        let Some((by, due)) = self.held else {
            return false;
        };
        if now < due {
            return false;
        }
        if !self.step(by, counts) {
            self.held = None;
            return false;
        }
        self.held = Some((by, now + REPEAT_MS));
        true
    }

    /// Only the direction that was being held stops it. Letting go of the other one is a
    /// change of direction the ring has already acted on.
    pub fn release(&mut self, by: i32) {
        if matches!(self.held, Some((held, _)) if held == by) {
            self.held = None;
        }
    }

    /// Whatever is held, let go of. Nothing on screen is holding it.
    pub fn release_hold(&mut self) {
        self.held = None;
    }

    /// The strip, drawn into the band's window. The band itself is the machine's, so it is
    /// drawn by `slot_chrome`; this draws the letters, and the metal between them, and nothing
    /// else.
    ///
    /// `faces` are the slot's textures with the size each was rasterised at, because a letter
    /// cropped to its ink is as wide as it is and no wider: drawing it into a square would set
    /// every letter to the same width, which is an index set in a typewriter's face rather than
    /// in this one. `ridge` is the one piece of metal the strip repeats; `None` costs the strip
    /// that piece and nothing else, the same way a missing cart face costs a cart its label.
    ///
    /// Flat, and that is the design rather than a simplification. A drum could only ever show
    /// its middle facet square on, so everything else on it was narrower and dimmer than what
    /// the marker held — which meant the eye had to be taught that the dim ones were the same
    /// letters. Along an opening in the case, the letters are all at the same depth and the
    /// only thing that separates the marker from its neighbours is the ink, which is what the
    /// three states below are for. The ridges are flat with them: full width, and fading only
    /// with distance from the marker rather than with the angle they stand at.
    pub fn draw_strip(
        &self,
        faces: &[(TexId, u32, u32)],
        ridge: Option<(TexId, u32, u32)>,
        counts: &[usize; N],
        out: &mut Vec<Draw>,
    ) {
        let cx = crate::draw::OUT_W as f32 / 2.0;
        // Where the widget's middle is: the middle of the band. Not the middle of the window —
        // the window is not centred in the band — and not the window less a nudge either; both
        // of those are what `MODULE_MID` is written out to replace.
        let mid = MODULE_MID;
        // Where the strip is between two slots. Everything below is a function of a slot's
        // distance from the marker, and the fraction is what lets a slot be caught midway
        // between two of them rather than snapped to one.
        let frac = self.scroll - self.scroll.round();
        let sub = self.scroll.round() as i32;

        // How much of its ink the strip still has this far out. Two curves, and the second is
        // wider than the first on purpose: the metal reaches half a slot past the last letter,
        // and a ridge faded on the letters' own curve would arrive at that half slot with
        // nothing left to draw — see `RIDGE_FADE_FROM`.
        let letter_fade = |off: f32| {
            let dx = (off * PITCH).abs();
            (1.0 - (dx - FADE_FROM) / (FADE_TO - FADE_FROM)).clamp(0.0, 1.0)
        };
        let metal_fade = |off: f32| {
            let dx = (off * PITCH).abs();
            (1.0 - (dx - RIDGE_FADE_FROM) / (RIDGE_FADE_TO - RIDGE_FADE_FROM)).clamp(0.0, 1.0)
        };

        // The metal first, because the letters sit *on* the strip and the ridge is the raised
        // join beside them — the order the drum drew them in, and the reason a seam never ends
        // up drawn across a letter.
        //
        // At the boundaries rather than at the slots: half a slot out either way from the
        // marker, which is ten ridges for nine letters, and one past each end so the strip
        // visibly carries on rather than stopping at the last thing it can name.
        if let Some((tex, rw, rh)) = ridge {
            for boundary in -VISIBLE..=VISIBLE + 1 {
                let off = boundary as f32 - 0.5 - frac;
                let fade = metal_fade(off);
                if fade <= 0.0 {
                    continue;
                }
                // Flat, there is no narrowing to do: the ridge is drawn at its own width and
                // dimmed only by how far out it stands. The drum's floor is gone with the
                // turning it existed for.
                let w = rw as f32;
                let h = rh as f32;
                out.push(Draw::Tex {
                    x: cx + off * PITCH - w / 2.0,
                    y: mid - h / 2.0,
                    w,
                    h,
                    tex,
                    alpha: fade,
                });
            }
        }

        for slot in -VISIBLE..=VISIBLE {
            let at = (sub + slot).rem_euclid(N as i32) as usize;
            let Some(&(tex, fw, fh)) = faces.get(at) else {
                continue;
            };
            let off = slot as f32 - frac;
            // How much of the marker a letter is. The marker is the only one at full height,
            // and the one arriving becomes it as it comes, which is what a step of the index
            // looks like rather than a swap.
            let reach = off.abs().min(1.0);
            let h = CENTRE_PX + (NEIGHBOUR_PX - CENTRE_PX) * reach;
            // Three states. The marker is never on an empty slot — `step` skips them and the
            // caret's own letter always has at least the cart it is on — so the dim one is
            // only ever a neighbour.
            let state = if counts[at] == 0 {
                EMPTY
            } else if reach < 0.5 {
                1.0
            } else {
                NEIGHBOUR
            };
            let fade = letter_fade(off);
            if fade <= 0.0 {
                continue;
            }
            // The letter keeps its own width at its own height — no turning, so no
            // foreshortening: a letter 26 tall is as wide as that font draws it 26 tall.
            let w = h * fw as f32 / fh as f32;
            out.push(Draw::Tex {
                x: cx + off * PITCH - w / 2.0,
                y: mid - h / 2.0,
                w,
                h,
                tex,
                alpha: state * fade,
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn counts_with(letters: &[char]) -> [usize; N] {
        let mut c = [0usize; N];
        for l in letters {
            c[slot_of(*l)] += 1;
        }
        c
    }

    #[test]
    fn slots_are_unique() {
        for (i, s) in SLOTS.iter().enumerate() {
            assert_eq!(SLOTS.iter().position(|x| x == s), Some(i), "{s} twice");
        }
    }

    #[test]
    fn an_initial_maps_to_its_slot() {
        assert_eq!(slot_label(slot_of('B')), 'B');
        assert_eq!(slot_label(slot_of('z')), 'Z', "case is not a bucket");
        assert_eq!(
            slot_label(slot_of('7')),
            '#',
            "a digit is not a facet of its own"
        );
        assert_eq!(slot_label(slot_of('0')), '#');
        assert_eq!(slot_label(slot_of('★')), '#', "neither is punctuation");
        assert_eq!(slot_label(slot_of('#')), '#');
    }

    #[test]
    fn the_ring_is_the_alphabet_and_the_catch_all() {
        // Twenty-seven: A-Z and `#`, which takes the digits and everything else. The number is
        // the point of the ring — a hand crosses it in a couple of seconds — so a slot added
        // back for the digits is a slot that has to be earned.
        assert_eq!(N, 27);
        assert_eq!(SLOTS[0], '#');
        assert_eq!(SLOTS[N - 1], 'Z');
        assert!(!SLOTS.iter().any(|c| c.is_ascii_digit()));
    }

    #[test]
    fn stepping_skips_the_empty_slots() {
        // B and E only, which is what makes C and D something to step over.
        let counts = counts_with(&['B', 'E']);
        let mut l = Letters::new();
        l.centre_on('B');
        assert_eq!(l.target(), slot_of('B'));

        assert!(l.step(1, &counts));
        assert_eq!(slot_label(l.target()), 'E', "C and D are empty");

        assert!(l.step(1, &counts));
        assert_eq!(slot_label(l.target()), 'B', "and it wraps");

        assert!(l.step(-1, &counts));
        assert_eq!(slot_label(l.target()), 'E', "backwards skips too");
    }

    #[test]
    fn a_card_with_one_letter_has_nowhere_to_go() {
        let counts = counts_with(&['B', 'B']);
        let mut l = Letters::new();
        l.centre_on('B');
        assert!(!l.step(1, &counts));
        assert!(!l.step(-1, &counts));
        assert_eq!(slot_label(l.target()), 'B');
    }

    #[test]
    fn a_step_of_zero_is_not_a_move() {
        let counts = counts_with(&['B']);
        let mut l = Letters::new();
        l.centre_on('B');
        assert!(!l.step(0, &counts));
    }

    /// How far apart two places on the ring are, the short way round. The spring is free to
    /// settle on any image of its target — that is the whole point of `scroll_target` — so the
    /// only meaningful question about where it stopped is which slot that is.
    fn gulf(a: f32, b: f32) -> f32 {
        let n = N as f32;
        ((a - b + n / 2.0).rem_euclid(n) - n / 2.0).abs()
    }

    #[test]
    fn the_spring_settles_on_the_target() {
        let mut l = Letters::new();
        l.centre_on('M');
        for _ in 0..600 {
            l.settle(1.0 / 60.0);
        }
        assert!(
            gulf(l.scroll(), l.target() as f32) < 0.01,
            "settled on {} rather than {}",
            l.scroll(),
            l.target()
        );
    }

    #[test]
    fn the_spring_takes_the_short_way_round() {
        // `#` is slot 0 and A is slot 11, so there are two ways between them: eleven slots
        // backwards or twenty-six forwards. A ring that took the long one would sweep the
        // whole alphabet to move one letter.
        let mut l = Letters::new();
        l.centre_on('A');
        for _ in 0..600 {
            l.settle(1.0 / 60.0);
        }
        let at_a = l.scroll();
        l.centre_on('#');
        for _ in 0..600 {
            l.settle(1.0 / 60.0);
        }
        assert!(
            gulf(l.scroll(), 0.0) < 0.01,
            "did not land on #: {}",
            l.scroll()
        );
        assert!(
            l.scroll() <= at_a + 0.01,
            "went the long way: {at_a} -> {}",
            l.scroll()
        );
    }

    #[test]
    fn a_held_key_sets_off_and_then_keeps_going() {
        let counts = counts_with(&['B', 'E']);
        let mut l = Letters::new();
        l.centre_on('B');

        assert!(l.hold(1, 1_000, &counts), "the press itself moves");
        assert_eq!(slot_label(l.target()), 'E');

        // Nothing until the delay is out, however many frames go by.
        assert!(!l.tick(1_100, &counts));
        assert_eq!(slot_label(l.target()), 'E');

        assert!(l.tick(1_000 + REPEAT_DELAY_MS, &counts));
        assert_eq!(slot_label(l.target()), 'B', "E wraps round to B");

        // And letting go stops it.
        l.release(1);
        assert!(!l.tick(u64::MAX, &counts));
    }

    #[test]
    fn letting_go_of_the_other_key_does_not_stop_this_one() {
        let counts = counts_with(&['B', 'E']);
        let mut l = Letters::new();
        l.centre_on('B');
        l.hold(1, 0, &counts);
        l.release(-1);
        assert!(
            l.tick(REPEAT_DELAY_MS, &counts),
            "the held direction survives"
        );
    }

    #[test]
    fn a_hold_that_cannot_move_arms_nothing() {
        // One letter on the whole card: there is nowhere to step to, so a key held down must
        // not leave a timer running behind it.
        let counts = counts_with(&['B']);
        let mut l = Letters::new();
        l.centre_on('B');
        assert!(!l.hold(1, 0, &counts));
        assert!(!l.tick(u64::MAX, &counts));
    }

    const FACE_ID: usize = 100;

    /// One face per slot, each with its own id, so a test can tell which slot was drawn.
    fn faces() -> Vec<(TexId, u32, u32)> {
        (0..N)
            .map(|i| (TexId::from_raw(FACE_ID + i), 20u32, 24u32))
            .collect()
    }

    /// The id the ridge is given in these tests. The letter faces start at `FACE_ID`, so a test
    /// can tell a piece of metal from a letter.
    const RIDGE_ID: usize = 1;

    /// Every `Tex` in the list, as (id, x, w, h, alpha).
    fn drawn(l: &Letters, counts: &[usize; N]) -> Vec<(TexId, f32, f32, f32, f32)> {
        let mut out = Vec::new();
        l.draw_strip(
            &faces(),
            Some((TexId::from_raw(RIDGE_ID), RIDGE_W, MODULE_H as u32)),
            counts,
            &mut out,
        );
        out.iter()
            .filter_map(|d| match d {
                Draw::Tex {
                    x,
                    w,
                    h,
                    alpha,
                    tex,
                    ..
                } => Some((*tex, *x, *w, *h, *alpha)),
                _ => None,
            })
            .collect()
    }

    /// The ridges, left to right. Taken out of the strip by id rather than by never having been
    /// drawn: a ridge in the place a letter should be is a hole in the strip, and a test that
    /// switched the metal off would have nothing to say about it.
    fn ridges_drawn(l: &Letters, counts: &[usize; N]) -> Vec<(TexId, f32, f32, f32, f32)> {
        drawn(l, counts)
            .into_iter()
            .filter(|(t, ..)| *t == TexId::from_raw(RIDGE_ID))
            .collect()
    }

    /// The letters, left to right, with the metal between them taken back out.
    fn letters_drawn(l: &Letters, counts: &[usize; N]) -> Vec<(TexId, f32, f32, f32, f32)> {
        drawn(l, counts)
            .into_iter()
            .filter(|(t, ..)| *t != TexId::from_raw(RIDGE_ID))
            .collect()
    }

    #[test]
    fn the_strip_draws_the_slots_either_side_of_the_marker() {
        // In ring order and nothing else: every letter of the ring between the two ends of the
        // strip is on screen, and missing one would leave a hole in it.
        let counts = counts_with(&['A']);
        let mut l = Letters::new();
        l.snap_to('A');
        let all = faces();
        let seen: Vec<TexId> = letters_drawn(&l, &counts)
            .into_iter()
            .map(|(t, ..)| t)
            .collect();
        let a = slot_of('A') as i32;
        let expected: Vec<TexId> = (-VISIBLE..=VISIBLE)
            .map(|s| all[(a + s).rem_euclid(N as i32) as usize].0)
            .collect();
        assert_eq!(seen, expected);
    }

    #[test]
    fn one_ridge_is_drawn_between_every_pair_of_facets() {
        // `VISIBLE` slots either side of the marker are drawn — nine of them — so there are ten
        // boundaries and ten ridges: one between every pair and one past each end. All ten at
        // rest, and that is the part worth holding. The outermost ridge stands half a slot past
        // the outermost letter, and fading the metal on the letters' own curve would bring it to
        // zero exactly there. It did, in the first cut: the strip ended on a letter at a third
        // ink with no seam beyond it, which is the one thing the last ridge exists to prevent.
        let counts = counts_with(&['A']);
        let mut l = Letters::new();
        l.snap_to('A');
        let metal: Vec<(f32, f32)> = ridges_drawn(&l, &counts)
            .into_iter()
            .map(|(_, x, w, _, a)| (x + w / 2.0, a))
            .collect();
        assert_eq!(metal.len(), (VISIBLE * 2 + 2) as usize);
        let gaps: Vec<f32> = metal.windows(2).map(|w| w[1].0 - w[0].0).collect();
        for g in &gaps {
            assert!(
                (g - PITCH).abs() < 0.01,
                "the ridges are not even: {gaps:?}"
            );
        }
        // And they sit *between* the letters: every ridge is half a pitch from a letter.
        let centres: Vec<f32> = letters_drawn(&l, &counts)
            .into_iter()
            .map(|(_, x, w, ..)| x + w / 2.0)
            .collect();
        for (r, _) in &metal {
            let nearest = centres
                .iter()
                .map(|c| (c - r).abs())
                .fold(f32::MAX, f32::min);
            assert!(
                (nearest - PITCH / 2.0).abs() < 0.01,
                "a ridge is not between two letters: {nearest}"
            );
        }
        // One ridge outside each outermost letter, with ink in it: the strip carries on past
        // what it can name. The two ends are the pair this test exists for.
        let outer_left = centres[0];
        let outer_right = centres[centres.len() - 1];
        assert!(
            metal[0].0 < outer_left - PITCH / 2.0 + 0.01,
            "no ridge past the first letter: {} against {outer_left}",
            metal[0].0
        );
        assert!(
            metal[metal.len() - 1].0 > outer_right + PITCH / 2.0 - 0.01,
            "no ridge past the last letter: {} against {outer_right}",
            metal[metal.len() - 1].0
        );
        for (at, alpha) in &metal {
            assert!(*alpha > 0.0, "a ridge at {at} was drawn with no ink in it");
        }
    }

    /// Every `Tex` the strip draws, with the vertical it was placed at — which is the one field
    /// the flat tuples above drop, and the one the centring assertions are about.
    fn strip_texts(l: &Letters, counts: &[usize; N]) -> Vec<(TexId, f32, f32, f32, f32)> {
        let mut out = Vec::new();
        l.draw_strip(
            &faces(),
            Some((TexId::from_raw(RIDGE_ID), RIDGE_W, MODULE_H as u32)),
            counts,
            &mut out,
        );
        out.iter()
            .filter_map(|d| match d {
                Draw::Tex {
                    y, h, alpha, tex, ..
                } => Some((*tex, *y, *h, *alpha, 0.0)),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn the_metal_is_dimmer_than_the_letters_and_taller_than_them() {
        // The ridge is a highlight behind the letters, never another state of them: a ridge as
        // bright as an empty slot would be a strip where every seam looked like a letter nobody
        // can read. And it is the frame the letters are read through, so the marker has to fit
        // inside it — same centre, less height.
        let counts = counts_with(&['A', 'B', 'C', 'D']);
        let mut l = Letters::new();
        l.snap_to('A');
        let texts = strip_texts(&l, &counts);
        let ridge: Vec<_> = texts
            .iter()
            .filter(|(t, ..)| *t == TexId::from_raw(RIDGE_ID))
            .collect();
        let letters: Vec<_> = texts
            .iter()
            .filter(|(t, ..)| *t != TexId::from_raw(RIDGE_ID))
            .collect();

        assert!(!ridge.is_empty(), "no metal was drawn at all");
        for (_, y, h, ..) in &ridge {
            assert!(
                (h - MODULE_H).abs() < 0.01,
                "a ridge is not the widget's height: {h} vs {MODULE_H}"
            );
            assert!(
                (y - (MODULE_MID - MODULE_H / 2.0)).abs() < 0.01,
                "a ridge is not hung on the widget's top edge: {y}"
            );
        }

        // The metal's peak is `RIDGE_PEAK` of its colour and the dimmest letter is `EMPTY` of the
        // ink; which of those is brighter is settled at compile time a few hundred lines up, and
        // here it is only the placement that is being asked about.
        let brightest_ridge = ridge
            .iter()
            .map(|(_, _, _, _, a)| *a)
            .fold(0.0f32, f32::max);
        assert!(
            brightest_ridge <= 1.0,
            "a ridge is drawn more solid than its own alpha allows: {brightest_ridge}"
        );

        // And the marker sits inside the frame: shorter than a ridge, on the same centre.
        let marker = letters[VISIBLE as usize];
        assert!(marker.2 < MODULE_H, "the marker stands over the metal");
        assert!(
            (marker.1 + marker.2 / 2.0 - MODULE_MID).abs() < 0.01,
            "the marker is not on the widget's middle"
        );
    }

    #[test]
    fn the_widget_is_centred_in_the_band_and_clear_of_its_edges() {
        // The three things the placement has to do at once: sit on the middle of the band, stay
        // inside the window it is read through, and leave both the window's frame and the band's
        // own boundary alone. All three are one number's business — `MODULE_MID` — so they are
        // checked together, on the tallest thing the strip can draw.
        //
        // The band's lower boundary used to carry a lit lip and this test measured against it.
        // The lip is gone from the band (see `slot_chrome::draw_top_band`: the bay's line under
        // the window read as a rule under the letters), so what is left to clear is the edge
        // itself and the window's own frame.
        let counts = counts_with(&['A']);
        let mut l = Letters::new();
        l.snap_to('A');
        let texts = strip_texts(&l, &counts);

        let top = MODULE_MID - MODULE_H / 2.0;
        let bottom = MODULE_MID + MODULE_H / 2.0;
        // Centred on the band rather than on the window: the window is not centred in the band,
        // so centring on it is what used to leave the strip reading as low in the case.
        assert!(
            ((top + bottom) / 2.0 - crate::slot_chrome::TOP_BAND_H / 2.0).abs() < 0.01,
            "the widget is not centred in the band"
        );
        // Inside the opening, so no part of it is printed on the plastic around the window.
        let (win_y, win_bottom) = (
            crate::slot_chrome::TOP_WIN_Y,
            crate::slot_chrome::TOP_WIN_Y + crate::slot_chrome::TOP_WIN_H,
        );
        assert!(
            top >= win_y && bottom <= win_bottom,
            "the widget leaves the window: {top}..{bottom} against {win_y}..{win_bottom}"
        );
        // And clear of the band's own lower boundary, which is the only line left down there.
        let band = crate::slot_chrome::TOP_BAND_H;
        for (_, y, h, ..) in &texts {
            assert!(
                y + h <= band - 6.0,
                "something is too close to the band's edge: {} vs {band}",
                y + h
            );
        }
    }

    #[test]
    fn the_letters_stand_at_even_gaps() {
        // What the drum could not do: the slots are all at the same depth, so they are all the
        // same distance apart and the run is a row rather than a surface turning away. An
        // uneven gap here would mean a leftover of the turning had survived into the strip.
        let counts = counts_with(&['A']);
        let mut l = Letters::new();
        l.snap_to('A');
        let centres: Vec<f32> = letters_drawn(&l, &counts)
            .into_iter()
            .map(|(_, x, w, ..)| x + w / 2.0)
            .collect();
        assert_eq!(centres.len(), (VISIBLE * 2 + 1) as usize);
        let gaps: Vec<f32> = centres.windows(2).map(|w| w[1] - w[0]).collect();
        for g in &gaps {
            assert!((g - PITCH).abs() < 0.01, "the gaps are not even: {gaps:?}");
        }
    }

    #[test]
    fn the_marker_is_the_tallest_and_the_brightest() {
        let counts = counts_with(&['A', 'B', 'C', 'D']);
        let mut l = Letters::new();
        l.snap_to('A');
        let d = letters_drawn(&l, &counts);
        let centre = &d[VISIBLE as usize];
        assert_eq!(centre.3, CENTRE_PX, "the marker is not at full height");
        assert_eq!(
            centre.2,
            CENTRE_PX * 20.0 / 24.0,
            "the marker's width is not its own aspect"
        );
        assert!(centre.4 > d[VISIBLE as usize + 1].4, "not the brightest");
        for pair in d[VISIBLE as usize..].windows(2) {
            assert!(
                pair[1].3 <= pair[0].3,
                "the letters grow towards the ends: {d:?}"
            );
        }
    }

    #[test]
    fn the_ends_of_the_strip_fade_out() {
        // The fade runs from three slots out, and the outermost slot the strip draws is four
        // out, so that letter is at a third of its ink. The letter one in from it is not faded
        // at all, which is what makes this a fade rather than a general dimming.
        let counts = counts_with(&['A', 'B', 'C', 'D', 'E', 'F', 'G', 'H', 'I']);
        let mut l = Letters::new();
        l.snap_to('E');
        let d = letters_drawn(&l, &counts);
        let inner = d[VISIBLE as usize + 1].4;
        assert!(d[0].4 < inner, "the outermost letter is not faded: {d:?}");
        assert!(
            d[d.len() - 1].4 < inner,
            "the outermost letter is not faded: {d:?}"
        );
    }

    #[test]
    fn the_strip_of_centres_sits_inside_the_window() {
        // `VISIBLE` slots either side of the marker, 192 px across at four fifths of the old
        // pitch, and it has to be inside the opening or the outermost letters would read as
        // printed on the case beside it.
        let span = (VISIBLE * 2) as f32 * PITCH;
        assert!(
            span < crate::slot_chrome::TOP_WIN_W,
            "the strip is wider than the window: {span}"
        );
        // Narrower than one cart, where it used to be exactly as wide as one. That equality was
        // the reason the pitch was thirty; at four fifths it is gone on purpose, and the bound
        // below is only here so a later rescale cannot quietly make the strip a sliver.
        let cart = crate::cart::CART_W as f32;
        assert!(
            span < cart,
            "the strip is not narrower than a cart on the row: {span}"
        );
        assert!(span > cart * 0.7, "the strip has become too narrow: {span}");
    }

    /// The strip's own position, so a test can build the two frames the travel is measured
    /// between without repeating the spring's first step twice.
    fn settled_at(letter: char, towards: Option<char>) -> Letters {
        let mut l = Letters::new();
        l.snap_to(letter);
        if let Some(next) = towards {
            l.centre_on(next);
            l.settle(1.0 / 60.0);
        }
        l
    }

    #[test]
    fn the_strip_moves_as_one_piece() {
        // One frame of the spring towards the next letter, and everything the strip draws has
        // moved left by the same amount: it is one object rather than a row of quads that happen
        // to travel together. The metal is in the same motion, which is what makes the letters
        // read as sitting *on* something rather than sliding through it.
        let counts = counts_with(&['A', 'B', 'C', 'D']);
        let still = settled_at('A', None);
        let moving = settled_at('A', Some('B'));

        let shift = {
            let before = letters_drawn(&still, &counts);
            let after = letters_drawn(&moving, &counts);
            assert_eq!(before.len(), after.len());
            for (a, b) in before.iter().zip(after.iter()) {
                assert_eq!(a.0, b.0, "the two runs drew different letters");
                assert!(b.1 < a.1, "a letter did not move with the strip: {a:?}");
            }
            before[0].1 - after[0].1
        };
        assert!(shift > 0.0, "the strip did not move");

        // The metal, drawn pairwise rather than matched by nearest position: all ten ridges are
        // on screen in both frames, so each of them is a piece the two frames can be asked about
        // directly.
        let before = ridges_drawn(&still, &counts);
        let after = ridges_drawn(&moving, &counts);
        assert_eq!(
            before.len(),
            after.len(),
            "the number of ridges changed as the strip moved"
        );
        for (a, b) in before.iter().zip(after.iter()) {
            assert_eq!(a.0, b.0, "the metal changed identity as it moved");
            assert!(
                ((a.1 - shift) - b.1).abs() < 0.01,
                "a ridge moved by {} while the letters moved by {shift}",
                a.1 - b.1
            );
        }
    }
}
