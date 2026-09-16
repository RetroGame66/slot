//! The shelf's letter ring: a second ring, above the cart row and on the same axis.
//!
//! A Chinese library has no order of its own — the names are hanzi and the card hands them over
//! in whatever order the directory listing has them — so the way to a particular game is a
//! bucket. The shelf itself stays exactly what it was: one cart ring, left and right, seven
//! carts wide. This is the dial above it, and it is the *same* ring shape one level up, which
//! is why it reads without being explained: the marker in the middle is the letter the cart
//! under the caret belongs to, and pressing up or down moves the marker and the shelf follows.
//!
//! Every slot the alphabet has room for is laid out, whether or not the card uses it. A ring
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
/// ring at all is what leaves twenty-seven facets — the alphabet and the catch-all — which is
/// a dial a hand can cross rather than a keyboard.
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

/// The arc one slot takes up on the drum, at the front where it faces the housing square on.
/// Wide enough for the letter at its own size plus the gap that makes a run of them read as
/// separate facets rather than as a word.
const PITCH: f32 = 32.0;

/// Slots drawn either side of the marker.
const VISIBLE: i32 = 4;

/// The angle between two facets of the drum, and the one number the whole dial is built from:
/// the letters, the ridges between them and the teeth those ridges carry all read their place
/// off it.
///
/// A *drawn* angle rather than the ring's own. The ring has twenty-seven positions, and
/// twenty-seven facets around a circle are thirteen and a third degrees each — `cos 13.3°` is
/// 0.973, so a drum built to its own geometry is within three per cent of a flat row and looks
/// like one. The facets are therefore spaced as if there were fewer of them still, which is
/// the only way a drum reads as a drum: fourteen degrees is unmistakably a faceted surface, and
/// it shows four either side of the marker before the surface has turned away.
const FACET: f32 = 14.0 * std::f32::consts::PI / 180.0;

/// The drum's radius, set so a facet is `PITCH` of arc at the front: the marker's neighbours
/// then sit where the flat row put them, and it is the ones further out that pull in.
const DRUM_R: f32 = PITCH / FACET;

/// The capsule's height.
const CAPSULE_H: f32 = 48.0;

/// How far the drum's visible surface reaches either way: the seam half a facet past the last
/// slot, which is where the teeth stop. A function rather than a constant because it is a sine
/// and `f32::sin` is not const — and one function rather than the number written down twice,
/// because the housing and the teeth have to agree about where the drum ends.
fn drum_reach() -> f32 {
    drum((VISIBLE as f32 + 0.5) * FACET).0
}

/// The capsule's width: the drum's visible surface plus the radius of an end cap. The caps are
/// what closes the housing off past the teeth, and the drum runs on behind them.
fn capsule_w() -> f32 {
    2.0 * (drum_reach() + CAPSULE_H / 2.0)
}

/// The capsule's top edge: the drum is centred halfway between the top of the screen and the top
/// of the cart under the caret.
///
/// The cart under the caret and not a side one — it is drawn at `CENTER_SCALE` and is the tall
/// one, and it is the one the drum must not look like it is resting on. `shelf::FOOT_Y` is where
/// the row stands, so that cart's top is the foot less its height, and the middle of what is left
/// above it is the only place this dial has ever really had.
const CAPSULE_Y: f32 =
    (crate::shelf::FOOT_Y - crate::cart::CART_H as f32 * crate::shelf::CENTER_SCALE) / 2.0
        - CAPSULE_H / 2.0;

/// How much of the light a facet keeps when it has turned fully away. Not zero: the end slots
/// are context — which letters the library has either side of the marker — and context that
/// has gone black is not context.
const DEPTH_FLOOR: f32 = 0.55;

/// Where a slot sits across the drum, and how much of the surface it still shows, for a given
/// angle from the marker. `sin` places it, `cos` is what is left of it facing the housing.
fn drum(angle: f32) -> (f32, f32) {
    (DRUM_R * angle.sin(), angle.cos().max(0.0))
}

/// The ring's spring. Snappier than the cart row's: a dial is small, and a letter that took as
/// long to arrive as a cartridge does would not read as a dial being turned.
const OMEGA: f32 = 20.0;

/// One ridge and its size, in pixels: the raised knuckle between two facets.
///
/// It is not a line but a piece of metal seen at three heights — a beveled crest down the
/// middle, and a wider boss at each end where it meets the rim. That boss is the tooth, and
/// having it and the crest be one texture is what makes the drum and the gear the same object:
/// a seam, a ridge and a tooth are the same ridge passed at different depths, so they are not
/// three things to keep in agreement.
const RIDGE_W: u32 = 9;

/// How far the ridge stops short of the housing either end. The letters are 26 tall in a 48
/// capsule, so there are eleven pixels above and below them for the rim to show through.
const RIDGE_INSET: f32 = 3.0;

fn ridge_h() -> f32 {
    CAPSULE_H - 2.0 * RIDGE_INSET
}

/// The ink a ridge is drawn in, and how much it fades at the far side of the drum. Its peak is
/// deliberately below the dimmest letter: a slot with nothing in it is ink at `EMPTY`, and the
/// rim is metal *behind* the letters rather than another state of them. A ridge as bright as an
/// empty slot would be a drum where every seam looked like a letter nobody can read.
const RIDGE_INK: [u8; 3] = [0xd8, 0xd4, 0xcc];
const RIDGE_PEAK: f32 = 0.20;
const RIDGE_FLOOR: f32 = 0.75;

/// How far the housing's shadow reaches past it, and how dark the drum goes at its ends.
const HALO: f32 = 5.0;
const DRUM_DARK: f32 = 8.0;
const DRUM_LIT: f32 = 34.0;

/// The housing's fill, and the bevel that makes it an opening rather than a shape.
///
/// Heavier than the HUD plate it used to match, and that is the skeuomorphism rather than a
/// drift: the plate is a translucent panel and this is a recessed window with a drum behind it.
/// At the plate's 0.72 a bright wallpaper lifts the drum's whole ladder, and the dim states —
/// which are the ones that carry the information — are what loses its footing first.
const CAPSULE: [f32; 4] = [0.0, 0.0, 0.0, 0.88];

/// The bevel: what a lit top edge and a shadowed bottom one are, as a lift on the housing's
/// own colour. Both are drawn inside the rim, where the metal of the surround would catch the
/// light.
const BEVEL_LIP: f32 = 22.0;
const BEVEL_SHADE: f32 = 10.0;

/// The bezel: how wide the housing's own rim is, and how bright.
///
/// A rim is the whole answer to "make it sit on the wallpaper", and the reason is that it is the
/// only part of the drawing that does not depend on the wallpaper. A shadow does nothing on a
/// dark one, which is exactly what this shelf has: the top of the screen comes out at 9 to 19,
/// and a nearly black housing against it has no edge at all. A rim does not care — it is brighter
/// than the housing and darker than a letter, so it reads as an object either way.
///
/// Lit at the top and in its own shadow at the bottom, like the lip and shade inside it: the same
/// light, one step further out.
const BEZEL: f32 = 1.6;
const BEZEL_TOP: f32 = 68.0;
const BEZEL_FOOT: f32 = 30.0;

/// Ink for the three states, as a fraction of the letters' own colour.
///
/// Only brightness, and deliberately: a slot with nothing in it still has to read as a slot, so
/// it cannot be invisible, and the marker has to read as *chosen*, so the neighbour cannot be
/// near it. The other reason there is no hue here is that the tree has exactly one warning
/// colour — `ALERT_INK` — and a dial that borrowed it would spend it on nothing.
///
/// The floor is set by the worst background rather than the best: the housing is 88% black, so
/// over a bright wallpaper it lands around a fifth of the letters' own luminance, and ink much
/// below a third of it disappears into that.
const EMPTY: f32 = 0.34;
const NEIGHBOUR: f32 = 0.68;

/// Before a held key steps a second letter. Longer than the cart row's: a press is a
/// deliberate move to a named letter, and two of those from one tap would be two letters of
/// an alphabet the user is reading.
const REPEAT_DELAY_MS: Millis = 450;

/// Between repeats after that. Faster than the cart row's — a dial is a coarse control, and
/// the whole alphabet should be a couple of seconds away rather than twenty.
const REPEAT_MS: Millis = 130;

/// The ink a letter is drawn in, and the size it is rasterised at.
pub const INK: [u8; 3] = [0xf5, 0xf2, 0xef];

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
/// which is a dial set in type too small to read from where a handheld is held.
const CENTRE_PX: f32 = 26.0;
const NEIGHBOUR_PX: f32 = 18.0;

/// The capsule texture's size, for the caller building it: the housing plus the shadow it casts
/// all round, which the texture has to carry because the draw list has no soft edges.
pub fn capsule_size() -> (u32, u32) {
    (
        (capsule_w() + 2.0 * HALO).round() as u32,
        (CAPSULE_H + 2.0 * HALO).round() as u32,
    )
}

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
        crate::text::draw_centred(&mut rgba, box_px, box_px, &layout, INK);
    }
    crop_to_ink(rgba, box_px, box_px)
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

/// The housing, as a face: a recessed window with a drum behind it and a shadow under it.
///
/// Drawn as a texture rather than as a rectangle for the two things a rectangle cannot do. The
/// ends are round, which is the difference between a piece of furniture and a debug overlay.
/// And the inside is a *ramp* — dark at both ends, lightest in the middle — which is what a
/// cylinder looks like: the middle of the drum faces the eye square on, and everything towards
/// the ends is turning away and catching less of the light. That ramp is the same `cos` the
/// letters are placed and lit by, so the drum's shading and its movement agree about the shape.
///
/// The shadow outside it is what keeps it off the wallpaper. Baked here rather than drawn as a
/// second quad because it is a soft edge, and the draw list has no soft edges.
pub fn capsule_face(w: u32, h: u32) -> CartFace {
    let mut rgba = vec![0u8; (w * h * 4) as usize];
    let (fw, fh) = (w as f32, h as f32);
    // The body inside the shadow, in this texture's own coordinates.
    let (bx, by) = (HALO, HALO);
    let (bw, bh) = (fw - 2.0 * HALO, fh - 2.0 * HALO);
    let r = bh / 2.0;
    let cy = by + r;
    let mid = bx + bw / 2.0;
    for y in 0..h {
        for x in 0..w {
            let (px, py) = (x as f32 + 0.5, y as f32 + 0.5);
            // Distance to the housing's outline, negative inside. One number for all three of
            // the bands — the shadow outside, the bezel, and what is left of the drum in the
            // middle — so they cannot end up describing different shapes.
            let along = (px - (bx + r)).clamp(0.0, bw - 2.0 * r);
            let d = ((px - (bx + r + along)).powi(2) + (py - cy).powi(2)).sqrt() - r;
            let i = ((y * w + x) * 4) as usize;
            if d >= HALO {
                continue;
            }
            if d >= 0.0 {
                // Outside: the shadow it casts. Squared, so it is dense against the housing and
                // gone by `HALO` — which is what seats it on the wallpaper instead of leaving it
                // looking laid on top of one.
                let a = 1.0 - d / HALO;
                rgba[i..i + 4].copy_from_slice(&[0, 0, 0, (a * a * 0.55 * 255.0).round() as u8]);
                continue;
            }
            let inset = -d;
            let lit = if inset < BEZEL {
                // The bezel: the one part of this that is not the drum. A rim with a brightness
                // of its own is what an object sitting on a wallpaper needs, and it is the only
                // thing here that works on a dark one — a shadow on black is nothing, and without
                // a rim the housing is a dark pill with letters in it that has no edge at all.
                if py < cy {
                    BEZEL_TOP
                } else {
                    BEZEL_FOOT
                }
            } else {
                // Inside the bezel: the drum's ramp, then the lip and shade just within the rim.
                let t = ((px - mid) / (bw / 2.0)).clamp(-1.0, 1.0);
                let shallow = (1.0 - t * t).max(0.0).sqrt();
                let ramp = DRUM_DARK + (DRUM_LIT - DRUM_DARK) * shallow;
                let bevel = (1.0 - (inset - BEZEL) / 3.0).clamp(0.0, 1.0);
                if py < cy {
                    ramp + BEVEL_LIP * bevel
                } else {
                    ramp - BEVEL_SHADE * bevel
                }
            };
            let lit = lit.clamp(0.0, 255.0);
            // Steel: a touch of blue in the dark, so the ramp reads as metal rather than as a
            // grey gradient. The alpha is the housing's own, and it is nearly opaque — see
            // `CAPSULE` for why this one is not the HUD plate's 0.72. The outline's last pixel is
            // feathered rather than sampled, which is what having the distance is for.
            let cover = (0.5 - d).clamp(0.0, 1.0);
            rgba[i..i + 4].copy_from_slice(&[
                (lit * 0.98).round() as u8,
                lit.round() as u8,
                (lit * 1.06).min(255.0).round() as u8,
                (CAPSULE[3] * cover * 255.0).round() as u8,
            ]);
        }
    }
    CartFace { rgba, w, h }
}

/// One ridge with its teeth, as a face.
///
/// A crest down the middle — lit on the side the light comes from, in shadow on the other —
/// and a wider boss at each end. The boss is what reads as a tooth, and the whole thing is one
/// texture because a seam, a ridge and a tooth are the same piece of metal; drawn as three
/// shapes they would be three things that had to be kept in agreement.
pub fn ridge_face() -> CartFace {
    let (w, h) = (RIDGE_W, ridge_h().round() as u32);
    let mut rgba = vec![0u8; (w * h * 4) as usize];
    let (fw, fh) = (w as f32, h as f32);
    let mid = fw / 2.0;
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
            rgba[i..i + 4].copy_from_slice(&[RIDGE_INK[0], RIDGE_INK[1], RIDGE_INK[2], a]);
        }
    }
    CartFace { rgba, w, h }
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
    /// starts reading the up and down keys.
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

    /// Where the strip is, continuously. Every part of the dial — where a letter sits, how much
    /// of it shows, where the teeth are — is read off this, so they all move as one piece. The
    /// spring is what keeps it off the whole numbers.
    pub fn scroll(&self) -> f32 {
        self.scroll
    }

    /// The marker follows the cart under the caret. Called every frame: setting the same slot
    /// twice is a write of the same number.
    pub fn centre_on(&mut self, initial: char) {
        self.target = slot_of(initial);
    }

    /// Puts the marker on a letter without travelling there, for the first frame of a session:
    /// there is no previous position to turn from, and a strip that arrived from `#` would be
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
        // Land, rather than creeping: an exponential tail on a dial is visible as a letter that
        // is almost in place.
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
    /// is not a move, and reporting it as one leaves a held key repeating forever at a dial
    /// that never turns.
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

    /// The draws. `capsule` is the housing the drum shows through and `ridge` the piece of
    /// metal between two facets; either missing costs the dial that piece and nothing else.
    ///
    /// `faces` are the slot's textures with the size each was rasterised at, because a letter
    /// cropped to its ink is as wide as it is and no wider: drawing it into a square would set
    /// every letter to the same width, which is a dial set in a typewriter's face rather than
    /// in this one.
    pub fn draw(
        &self,
        capsule: Option<(TexId, u32, u32)>,
        faces: &[(TexId, u32, u32)],
        counts: &[usize; N],
        ridge: Option<TexId>,
        out: &mut Vec<Draw>,
    ) {
        let cx = crate::draw::OUT_W as f32 / 2.0;
        if let Some((tex, w, h)) = capsule {
            // The texture carries `HALO` of shadow all round the housing, so its corner is
            // that much above and left of the housing's own.
            out.push(Draw::Tex {
                x: cx - w as f32 / 2.0,
                y: CAPSULE_Y - HALO,
                w: w as f32,
                h: h as f32,
                tex,
                alpha: 1.0,
            });
        }

        // Where the drum is between two slots. Every part of what follows is a function of a
        // slot's angle from the marker, and the fraction is what lets a slot be caught midway
        // between two of them rather than snapped to one.
        let frac = self.scroll - self.scroll.round();
        let mid_y = CAPSULE_Y + CAPSULE_H / 2.0;
        let sub = self.scroll.round() as i32;

        // The ridge between two facets, drawn before the letters because the letter sits on the
        // facet and the ridge is the raised join beside it.
        //
        // One per *boundary* of the facets being drawn, and the facets drawn run half a slot
        // past the last letter either way — so there is a ridge past each end of the readable
        // row, and the drum visibly carries on behind the housing instead of stopping at the
        // outermost letter. That is `2 * VISIBLE + 2` of them, from `-(VISIBLE + 0.5)` to
        // `+(VISIBLE + 0.5)` in slot units: an odd count would be the drum off centre.
        if let Some(tex) = ridge {
            for b in 0..=VISIBLE * 2 + 1 {
                // `- frac` is what makes the metal turn with the letters. Without it the ridges
                // sit still and the drum slides through them, which is a glass tube with lines
                // drawn on it.
                let angle = (b as f32 - (VISIBLE as f32 + 0.5) - frac) * FACET;
                let (x, depth) = drum(angle);
                if depth <= 0.0 || x.abs() > drum_reach() + RIDGE_W as f32 / 2.0 {
                    continue;
                }
                // Turned away, the ridge is both narrower and fainter — the one is the surface
                // foreshortening and the other is less of it catching the light.
                let w = (RIDGE_W as f32 * depth).max(1.0);
                out.push(Draw::Tex {
                    x: cx + x - w / 2.0,
                    y: CAPSULE_Y + RIDGE_INSET,
                    w,
                    h: ridge_h(),
                    tex,
                    alpha: RIDGE_FLOOR + (1.0 - RIDGE_FLOOR) * depth,
                });
            }
        }

        // The letters, on the facets between them. A slot's place across the housing is the
        // front of the drum seen side-on — `R sin θ` — so they bunch up as they go, and how
        // much of the letter is left is `cos θ`: the pane has turned away from the eye, and a
        // letter does not merely shrink, it narrows.
        for slot in -VISIBLE..=VISIBLE {
            let at = (sub + slot).rem_euclid(N as i32) as usize;
            let Some(&(tex, fw, fh)) = faces.get(at) else {
                continue;
            };
            let offset = slot as f32 - frac;
            let (x, depth) = drum(offset * FACET);
            if depth <= 0.0 {
                continue;
            }
            // How much of the marker a letter is. The marker is the only one at full height,
            // and the one arriving becomes it as it comes, which is what a step of the dial
            // looks like rather than a swap.
            let reach = offset.abs().min(1.0);
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
            let lit = DEPTH_FLOOR + (1.0 - DEPTH_FLOOR) * depth;
            out.push(Draw::Tex {
                x: cx + x - h * depth * fw as f32 / fh as f32 / 2.0,
                y: mid_y - h / 2.0,
                w: h * depth * fw as f32 / fh as f32,
                h,
                tex,
                alpha: state * lit,
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
        // the point of the ring — a hand crosses it in a couple of seconds — so a facet added
        // back for the digits is a facet that has to be earned.
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

    /// The id the ridge is given in these tests. The letter faces start at `FACE_ID`, so a test
    /// can tell a letter from a piece of metal by which side of it the id falls on.
    const RIDGE_ID: usize = 1;
    const FACE_ID: usize = 100;

    /// One face per slot, each with its own id, so a test can tell which slot was drawn.
    fn faces() -> Vec<(TexId, u32, u32)> {
        (0..N)
            .map(|i| (TexId::from_raw(FACE_ID + i), 20u32, 24u32))
            .collect()
    }

    /// Every `Tex` in the list, as (id, x, w, h, alpha).
    fn texts(l: &Letters, counts: &[usize; N]) -> Vec<(TexId, f32, f32, f32, f32)> {
        let mut out = Vec::new();
        l.draw(
            None,
            &faces(),
            counts,
            Some(TexId::from_raw(RIDGE_ID)),
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

    /// The letters, left to right. The ridges are taken back out by id rather than by never
    /// having been drawn: a ridge in the place a letter should be is a hole in the drum, and a
    /// test that switched the ridges off would have nothing to say about it.
    fn letters_drawn(l: &Letters, counts: &[usize; N]) -> Vec<(TexId, f32, f32, f32, f32)> {
        texts(l, counts)
            .into_iter()
            .filter(|(t, ..)| *t != TexId::from_raw(RIDGE_ID))
            .collect()
    }

    #[test]
    fn the_drum_draws_the_slots_either_side_of_the_marker() {
        // In ring order and nothing else: every letter of the ring between the two ends of the
        // drum's reach is on screen, and missing one would leave a hole in it.
        let counts = counts_with(&['A']);
        let mut l = Letters::new();
        l.snap_to('A');
        let all = faces();
        let drawn: Vec<TexId> = letters_drawn(&l, &counts)
            .into_iter()
            .map(|(t, ..)| t)
            .collect();
        let a = slot_of('A') as i32;
        let expected: Vec<TexId> = (-VISIBLE..=VISIBLE)
            .map(|s| all[(a + s).rem_euclid(N as i32) as usize].0)
            .collect();
        assert_eq!(drawn, expected);
    }

    #[test]
    fn one_ridge_is_drawn_between_every_pair_of_facets() {
        // `VISIBLE` facets either side of the marker are drawn — nine of them — so there are
        // ten joins to draw, one past each end. The spacing is widest at the marker and tightens
        // towards both ends, symmetrically: that bunching is the surface turning away, and a set
        // of ridges laid out evenly would be a flat row with lines drawn on it.
        let counts = counts_with(&['A']);
        let mut l = Letters::new();
        l.snap_to('A');
        let ridges: Vec<f32> = texts(&l, &counts)
            .into_iter()
            .filter(|(t, ..)| *t == TexId::from_raw(RIDGE_ID))
            .map(|(_, x, w, ..)| x + w / 2.0)
            .collect();
        assert_eq!(ridges.len(), (VISIBLE * 2 + 2) as usize);

        let gaps: Vec<f32> = ridges.windows(2).map(|w| w[1] - w[0]).collect();
        let mid = gaps.len() / 2;
        for i in 0..gaps.len() {
            assert!(
                (gaps[i] - gaps[gaps.len() - 1 - i]).abs() < 0.01,
                "the drum is not symmetric: {gaps:?}"
            );
        }
        for i in 0..mid {
            assert!(
                gaps[i] < gaps[i + 1],
                "the ridges do not open out towards the marker: {gaps:?}"
            );
        }
        assert!(
            gaps[gaps.len() - 1] < gaps[0] + 0.01,
            "the drum does not run on past the last letter: {ridges:?}"
        );
    }

    #[test]
    fn the_surface_turns_away_toward_the_ends() {
        // The whole point of the drum: what is not at the marker is narrower, closer to its
        // neighbour and dimmer, because the pane it is printed on has turned away from the
        // eye. A row whose letters were all the same width and evenly spaced is the flat strip
        // the drum was drawn to replace.
        let counts = counts_with(&['A']);
        let mut l = Letters::new();
        l.snap_to('A');
        let drawn = letters_drawn(&l, &counts);
        assert_eq!(drawn.len(), (VISIBLE * 2 + 1) as usize);

        let centre = &drawn[VISIBLE as usize];
        assert_eq!(centre.3, CENTRE_PX, "the marker is not at full height");
        assert_eq!(
            centre.2,
            CENTRE_PX * 20.0 / 24.0,
            "the marker's width is not its own aspect at full depth"
        );
        assert!(centre.4 > drawn[0].4, "the marker is not the brightest");
        let right = &drawn[VISIBLE as usize + 1..];
        for w in right.windows(2) {
            let (nearer, further) = (&w[0], &w[1]);
            assert!(
                further.2 < nearer.2,
                "widths do not fall off: {nearer:?} {further:?}"
            );
            assert!(
                further.4 < nearer.4,
                "ink does not fall off: {nearer:?} {further:?}"
            );
        }
        let gaps: Vec<f32> = right.windows(2).map(|w| w[1].1 - w[0].1).collect();
        for g in gaps.windows(2) {
            assert!(g[1] < g[0], "the spacing does not bunch up: {gaps:?}");
        }
    }

    #[test]
    fn the_metal_turns_with_the_letters() {
        // Half a slot of spring, and everything drawn through the drum's faces has moved left by
        // the same amount: the ridges, the teeth on them and the letters are one object. A ridge
        // that stood still while the letters slid past it is a glass tube with lines drawn on.
        let counts = counts_with(&['A', 'B', 'C', 'D']);
        let mut l = Letters::new();
        l.snap_to('A');
        let before = texts(&l, &counts);
        let moved = {
            let mut l = Letters::new();
            l.snap_to('A');
            // One frame of the spring towards the next letter, which is a fraction of a slot and
            // therefore the whole of what this test is about.
            l.centre_on('B');
            l.settle(1.0 / 60.0);
            texts(&l, &counts)
        };
        assert_eq!(before.len(), moved.len());
        for (a, b) in before.iter().zip(moved.iter()) {
            assert_eq!(a.0, b.0, "the two runs drew different things");
            let shift = b.1 - a.1;
            assert!(
                shift < -0.5,
                "{} did not move with the drum: {shift}",
                if a.0 == TexId::from_raw(1) {
                    "a ridge"
                } else {
                    "a letter"
                }
            );
        }
    }

    #[test]
    fn the_drum_sits_centred_in_the_gap_above_the_cart() {
        // The drum's middle is halfway between the top of the screen and the top of the cart
        // under the caret. Not halfway between the HUD plate and the cart: the plate is only up
        // while a level is being changed, so the band it takes is not one this can be planned
        // around. And it is the tall cart that matters — the one drawn at `CENTER_SCALE`, the one
        // it must not look like it is resting on.
        let counts = counts_with(&['A']);
        let (cw, chh) = capsule_size();
        let mut out = Vec::new();
        Letters::new().draw(
            Some((TexId::from_raw(9), cw, chh)),
            &faces(),
            &counts,
            None,
            &mut out,
        );
        let Draw::Tex { y, h, .. } = out[0] else {
            panic!("the capsule was not drawn");
        };
        // The texture carries `HALO` of shadow all round, so its own middle is the drum's.
        let middle = y + h / 2.0;
        let cart_top =
            crate::shelf::FOOT_Y - crate::cart::CART_H as f32 * crate::shelf::CENTER_SCALE;
        assert!(
            (middle - cart_top / 2.0).abs() < 0.01,
            "the drum is at {middle}, not at the middle of 0..{cart_top}"
        );
        assert!(
            middle + CAPSULE_H / 2.0 < cart_top,
            "the drum overlaps the cart under the caret"
        );
    }

    #[test]
    fn nothing_is_drawn_where_the_drum_has_turned_past_the_housing() {
        // The drum is drawn half a facet past the outermost letter either way and stops: past
        // that it is behind the housing's end cap, and anything drawn there is metal floating
        // over the wallpaper outside the window.
        let counts = counts_with(&['A']);
        let mut l = Letters::new();
        l.snap_to('A');
        let rim = drum_reach();
        let cx = crate::draw::OUT_W as f32 / 2.0;
        for (_, x, w, ..) in texts(&l, &counts) {
            let mid = x + w / 2.0 - cx;
            assert!(
                mid.abs() <= rim + RIDGE_W as f32,
                "something is past the flat of the housing: {mid}"
            );
        }
    }
}
