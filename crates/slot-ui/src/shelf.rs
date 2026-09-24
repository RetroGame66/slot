use slot_gfx::{Draw, TexId, OUT_H, OUT_W};
use slot_store::Cart;

use crate::cart::{label_colour, label_panel, label_text, CART_H, CART_W};
use crate::hud::Millis;
use crate::slot_chrome::draw_empty_slot;

/// Distance between cart centres. Wide enough that the side carts sit half off-screen, so the
/// row reads as continuing past them rather than as three equal carts side by side.
const PITCH: f32 = 360.0;
/// The centre cart is drawn larger than the row so it reads as the one in hand; the
/// neighbours keep their full size but are pushed to the edges (see PITCH). 1.5 makes the
/// selection clearly the hero without dwarfing the neighbours off screen.
///
/// Public because it is not the shelf's business alone: the app scales the cart into the slot
/// from this size, and a cart drawn at rest is a `CENTER_SCALE` cart to anything looking for
/// one.
pub const CENTER_SCALE: f32 = 1.5;
pub const SIDE_SCALE: f32 = 1.0;
/// How much of its face a side cart keeps at full recede. Public for the same reason as
/// `CENTER_SCALE`: the app derives how much further to dim the neighbours from it, and that
/// arithmetic has to follow this number rather than be written down against an old one.
pub const SIDE_ALPHA: f32 = 0.7;
/// Carts stand on the row rather than float: the foot stays put as a cart shrinks away.
pub(crate) const FOOT_Y: f32 = (OUT_H + CART_H) as f32 / 2.0;
/// Critically damped, so a flick lands on a cart instead of bouncing past and returning.
const OMEGA: f32 = 16.0;
/// How far the cart next to the selection is pushed aside as the chosen one goes in. Enough
/// to clear the frame from where it stands.
const PART: f32 = 130.0;
/// How much a face-less cart is stretched along the row while the ring glides. Small on
/// purpose: a cart stretched far enough to notice as a *shape* stops reading as a cart, and the
/// point is speed rather than distortion. Exactly 1 at rest, so a still row is untouched.
const SMEAR: f32 = 1.12;

/// Slots considered either side of the selection. Two reach the edges of a 720 row, the
/// third covers the lag while the spring is still catching up with a flick.
const SLOTS: i32 = 3;

/// Before the first repeat. Long enough that a press meaning one cart cannot become two.
const REPEAT_DELAY_MS: Millis = 400;
/// Between repeats after that. Fast enough to cross a thirty cart library, slow enough to
/// stop on one.
const REPEAT_MS: Millis = 110;

/// A letter jump's own travel, played by hand rather than by the spring.
///
/// The spring's own travel is what a single cart step wants, but a letter jump can cross
/// hundreds of carts and the spring's pace does not scale with distance — it settles in the
/// same few hundred milliseconds whatever it is asked to cover, which at range is a blur the
/// eye cannot read and at one cart is exactly right. So a jump glides instead: `scroll` is
/// swept from where the row stands to the cart the dial chose, over a duration that grows a
/// little with the distance and then stops growing. The sweep is a smoothstep, so it eases in
/// and out — the row *accelerates* away from the letter it is leaving and settles onto the
/// one it is arriving at, and every cart it skips passes through the middle on the way.
#[derive(Copy, Clone)]
struct Glide {
    from: f32,
    to: f32,
    t: f32,
    dur: f32,
}

/// One cart's place on screen this frame, addressed by its index in `Shelf::carts`.
///
/// The row's layout, stated once and read twice: `draw_row` draws the faces from it, and the
/// favourites star is placed from it, so a mark on a cart cannot drift off the cart it marks.
pub struct SlotRect {
    pub cart: usize,
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
    /// The cart's own opacity: 1.0 at the selection, `SIDE_ALPHA` at and past one slot out,
    /// then cut by the `recede` the caller asked for.
    pub alpha: f32,
}

/// The glide's length in seconds, as a function of the carts it has to cross. Short jumps are
/// near a spring's own settle so a single-letter step does not feel slower than an arrow; the
/// cap keeps a four-hundred cart jump from becoming a slideshow.
fn glide_seconds(distance: f32) -> f32 {
    (0.22 + distance * 0.004).clamp(0.22, 0.85)
}

pub struct Shelf {
    pub carts: Vec<Cart>,
    /// The carts the ring actually walks, as indices into `carts`. The full library when
    /// nothing is filtered; the starred ones while the favourites shelf is up. A list of
    /// indices rather than a rebuilt cart list, and that is the whole point: `faces` stays
    /// indexed by the library, so every texture the row was showing is the same texture after
    /// the view changes. Rebuilding the shelf instead would drop those faces and put blank
    /// placeholders on screen while they were rasterised again.
    view: Vec<usize>,
    /// Position in `view`, not an index into `carts`. `current()` is what turns it back.
    pub index: usize,
    pub scroll: f32,
    /// One slot per cart, `None` until its face has been rasterised and uploaded. The
    /// shelf is born showing placeholders and fills in behind the caret: rasterising every
    /// cart up front costs about 70 ms each on this hardware, which is seven seconds on a
    /// hundred-game card spent before the first frame.
    faces: Vec<Option<TexId>>,
    /// The cart silhouette in black, drawn under a dimmed cart. One texture for the whole
    /// row: every cart is the same shape.
    shadow: Option<TexId>,
    /// A cart with a blank label, drawn for a cart whose own face is not built yet. One texture
    /// for the whole row for the same reason `shadow` is one — until the label goes on they are
    /// all the same cart — and it is what a jump across the alphabet shows sliding past.
    placeholder: Option<TexId>,
    vel: f32,
    /// The direction being held and when it next repeats. Repeat lives here rather than in
    /// the gesture layer so nothing in game starts auto firing.
    held: Option<(i32, Millis)>,
    /// A letter jump in flight. While one is running it owns `scroll` and the spring stands
    /// down; a cart step drops it so the arrows always answer immediately.
    glide: Option<Glide>,
}

impl Shelf {
    pub fn new(carts: Vec<Cart>) -> Self {
        let view = (0..carts.len()).collect();
        Shelf {
            carts,
            view,
            index: 0,
            scroll: 0.0,
            faces: Vec::new(),
            shadow: None,
            placeholder: None,
            vel: 0.0,
            held: None,
            glide: None,
        }
    }

    /// Face textures in `carts` order. The caller uploads them because only the compositor
    /// can mint a `TexId`.
    pub fn set_shadow(&mut self, face: TexId) {
        self.shadow = Some(face);
    }

    /// The blank cart a not-yet-built face stands in as. Set once at boot, with the shadow.
    pub fn set_placeholder(&mut self, face: TexId) {
        self.placeholder = Some(face);
    }

    pub fn set_faces(&mut self, faces: Vec<Option<TexId>>) {
        self.faces = faces;
    }

    /// The texture a cart is holding, if its face has been built and not since released.
    pub fn face_of(&self, i: usize) -> Option<TexId> {
        self.faces.get(i).copied().flatten()
    }

    /// Drops one face back to its placeholder. The caller releases the texture; this only
    /// forgets the handle, so the shelf stops pointing at something that is no longer there.
    pub fn clear_face(&mut self, i: usize) {
        if let Some(slot) = self.faces.get_mut(i) {
            *slot = None;
        }
    }

    /// One face, as it arrives. The list is grown to the cartridge count first, because the
    /// background filler answers out of order and an index can land before its turn.
    pub fn set_face(&mut self, i: usize, tex: Option<TexId>) {
        if self.faces.len() < self.carts.len() {
            self.faces.resize(self.carts.len(), None);
        }
        if let Some(slot) = self.faces.get_mut(i) {
            *slot = tex;
        }
    }

    /// In `hints` order.
    pub fn find(&self, stem: &str) -> Option<(&Cart, Option<TexId>)> {
        let i = self.carts.iter().position(|c| c.stem == stem)?;
        Some((&self.carts[i], self.faces.get(i).copied().flatten()))
    }

    /// Send the row to the cart the letter dial has just chosen, sweeping it across everything
    /// in between rather than flicking through only the last cart. Called after `index` has
    /// been set, since that is what the row is measured towards.
    ///
    /// The direction is not chosen here. The row is a ring, so the cart the dial arrived at has
    /// an image on either side, and the sweep takes the one nearest where the row already
    /// stands: stepping the dial backward runs the row backward. That is the whole of what
    /// keeps the dial and the row agreeing about which way is "down" — a seat that always
    /// played forward would have the two controls contradict each other on every backward step.
    ///
    /// A card with no more carts than the row has slots is left to the spring: there is nothing
    /// to glide across, and a sweep one cart wide would only be a slower version of the single
    /// step the row already draws cleanly.
    pub fn glide_to_target(&mut self) {
        let rows = SLOTS * 2 + 1;
        if (self.view.len() as i32) < rows {
            self.glide = None;
            self.vel = 0.0;
            return;
        }
        let from = self.scroll;
        let to = self.scroll_target();
        self.glide = Some(Glide {
            from,
            to,
            t: 0.0,
            dur: glide_seconds((to - from).abs()),
        });
        self.vel = 0.0;
    }

    /// Which way the ring is travelling while it glides, or `None` at rest. The smear and its
    /// trail hang off this: a still row must be drawn exactly as it always was.
    pub(crate) fn glide_dir(&self) -> Option<f32> {
        self.glide
            .map(|g| (g.to - g.from).signum())
            .filter(|d| *d != 0.0)
    }

    /// The cart the caret is on, as an index into `carts`. `None` only for an empty view.
    pub fn current(&self) -> Option<usize> {
        self.view.get(self.index).copied()
    }

    /// The cart itself, which is what most callers actually want.
    pub fn current_cart(&self) -> Option<&Cart> {
        self.carts.get(self.current()?)
    }

    /// Whether the view is narrower than the library — i.e. the favourites shelf is up.
    pub fn filtered(&self) -> bool {
        self.view.len() != self.carts.len()
    }

    /// Swap the set of carts the ring walks. The frame — the shadow and the placeholder — is
    /// the shelf's rather than the list's, so it is left alone; `faces` is left alone too, for
    /// the reason `view` is a list of indices. The caret is parked on `keep` when the new view
    /// still holds it, and on the first cart otherwise.
    pub fn set_view(&mut self, view: Vec<usize>, keep: Option<&str>) {
        self.view = view;
        self.index = keep
            .and_then(|stem| self.view.iter().position(|&i| self.carts[i].stem == stem))
            .unwrap_or(0);
        self.scroll = self.index as f32;
        self.vel = 0.0;
        self.glide = None;
        self.held = None;
    }

    /// The carts the current view walks, in view order. A view is a list of indices, so this
    /// is how a caller that needs the carts themselves — the letter tally — reads one.
    pub fn visible(&self) -> Vec<&Cart> {
        self.view.iter().map(|&i| &self.carts[i]).collect()
    }

    #[cfg(test)]
    pub fn view_len(&self) -> usize {
        self.view.len()
    }

    pub fn left(&mut self) {
        self.step(-1);
    }

    pub fn right(&mut self) {
        self.step(1);
    }

    pub fn hold_left(&mut self, now: Millis) {
        self.hold(-1, now);
    }

    pub fn hold_right(&mut self, now: Millis) {
        self.hold(1, now);
    }

    /// The press moves a cart itself, so the repeat is what the delay is measured from
    /// rather than what it produces.
    fn hold(&mut self, by: i32, now: Millis) {
        self.step(by);
        self.held = Some((by, now + REPEAT_DELAY_MS));
    }

    pub fn release_left(&mut self) {
        self.release(-1);
    }

    pub fn release_right(&mut self) {
        self.release(1);
    }

    /// Only the direction that is being held stops it. Letting go of the other one is a
    /// change of direction the shelf has already acted on.
    fn release(&mut self, by: i32) {
        if matches!(self.held, Some((held, _)) if held == by) {
            self.held = None;
        }
    }

    /// Whatever is held, let go of. Nothing on screen is holding it.
    pub fn release_hold(&mut self) {
        self.held = None;
    }

    /// Fires the repeat. Due from `now` rather than from the deadline it passed, so a frame
    /// the app was late for costs one cart instead of a burst of catching up.
    pub fn tick(&mut self, now: Millis) {
        let Some((by, due)) = self.held else {
            return;
        };
        if now < due {
            return;
        }
        self.step(by);
        self.held = Some((by, now + REPEAT_MS));
    }

    fn step(&mut self, by: i32) {
        let n = self.view.len();
        if n == 0 {
            return;
        }
        // An arrow is a direction of its own, and it has to answer on the press: a jump still
        // gliding is dropped so the spring takes the row from wherever the glide left it.
        self.glide = None;
        self.index = (self.index as i32 + by).rem_euclid(n as i32) as usize;
    }

    /// Where the spring is heading, in the continuous coordinate `scroll` lives in. The row
    /// is a ring, so the selected cart has an image every `n` slots; this is the one nearest
    /// where the row already is, which is what stops a wrap unwinding the whole row.
    pub fn scroll_target(&self) -> f32 {
        let n = self.view.len();
        if n == 0 {
            return 0.0;
        }
        let n = n as f32;
        self.scroll + (self.index as f32 - self.scroll + n / 2.0).rem_euclid(n) - n / 2.0
    }

    /// The cart `off` slots right of the selection. `None` when the row is empty, or when
    /// this slot would repeat a cart another slot is already showing: with two carts the
    /// left and right neighbours are the same one, and a row holding it twice reads as a
    /// bug. The row is left with a gap instead.
    pub fn cart_at_offset(&self, off: i32) -> Option<usize> {
        let n = self.view.len() as i32;
        if n == 0 {
            return None;
        }
        let r = off.rem_euclid(n);
        let nearest = if r * 2 > n { r - n } else { r };
        (nearest == off).then(|| self.view[(self.index as i32 + off).rem_euclid(n) as usize])
    }

    pub fn update(&mut self, dt: f32) {
        // A letter jump owns the row while it runs. Its sweep is the movement, so the spring
        // must not also be pulling at `scroll`, or the two would argue over the same cart.
        if let Some(mut g) = self.glide {
            g.t += dt;
            let p = (g.t / g.dur).clamp(0.0, 1.0);
            // Smoothstep: ease in, ease out, so the row accelerates away and settles in.
            let e = p * p * (3.0 - 2.0 * p);
            self.scroll = g.from + (g.to - g.from) * e;
            self.vel = 0.0;
            self.glide = if p >= 1.0 {
                self.scroll = g.to;
                None
            } else {
                Some(g)
            };
            return;
        }
        let accel = -2.0 * OMEGA * self.vel - OMEGA * OMEGA * (self.scroll - self.scroll_target());
        self.vel += accel * dt;
        self.scroll += self.vel * dt;
    }

    /// The shelf screen: the row of carts and the slot under it. What is printed on the case
    /// is drawn after this, by whoever holds the type.
    pub fn draw(&self, shake: f32, out: &mut Vec<Draw>) {
        self.draw_row(None, shake, 0.0, 1.0, out);
        draw_empty_slot(out);
    }

    /// The row alone. The cart on its way into the slot is drawn by the chrome, at the same
    /// place the row would draw it; leaving it in the row as well puts two of one cart on
    /// screen and the travel then reads as a copy sliding away from the original.
    ///
    /// `shake` displaces the carts and nothing else. On the shelf the frame is mostly
    /// backdrop, so shaking that slides the letterbox in at the edges rather than reading as
    /// a refusal.
    ///
    /// `recede` clears the row for the cart going into the slot: 0.0 leaves it alone, 1.0
    /// has every other cart gone. They part outwards rather than fading in place, so the row
    /// reads as making way for the one that was chosen.
    ///
    /// `dim` darkens the faces further and nothing else: 1.0 leaves them as `recede` has them.
    /// The black under a dimmed cart stays as `recede` alone makes it, so a dimmed cart reads
    /// as a cart in shadow rather than a ghost over the wallpaper.
    pub fn draw_row(
        &self,
        hidden: Option<&str>,
        shake: f32,
        recede: f32,
        dim: f32,
        out: &mut Vec<Draw>,
    ) {
        let dim = dim.clamp(0.0, 1.0);
        for r in self.slot_rects(hidden, shake, recede) {
            let cart = &self.carts[r.cart];
            // Black in the cart's own shape, under the dimmed face. Without it the dimming is
            // transparency, and over a wallpaper the row reads as ghosts of carts.
            if r.alpha < 1.0 {
                if let Some(tex) = self.shadow {
                    out.push(Draw::Tex {
                        x: r.x,
                        y: r.y,
                        w: r.w,
                        h: r.h,
                        tex,
                        alpha: recede_alpha(r.alpha),
                    });
                }
            }
            out.push(match self.faces.get(r.cart).copied().flatten() {
                Some(tex) => Draw::Tex {
                    x: r.x,
                    y: r.y,
                    w: r.w,
                    h: r.h,
                    tex,
                    alpha: r.alpha * dim,
                },
                // A cart whose face has not been uploaded still holds its place, and as a cart.
                // A jump across the alphabet crosses hundreds of carts and hundreds of faces
                // cannot be held at once — but every cart can still carry *its own* colour, so
                // the row reads as a ribbon of labels going past rather than as a row of empty
                // cases. The shell goes down, then one solid quad in the panel `label_panel`
                // defines, in the colour this cart's own title hashes to: no texture is minted
                // and nothing is rasterised, which is the whole reason it can be done at all.
                None => match self.placeholder {
                    Some(tex) => {
                        let c = label_colour(&label_text(cart));
                        let (x0, y0, x1, y1) = label_panel(r.w as u32, r.h as u32);
                        let ink = [
                            c[0] as f32 / 255.0,
                            c[1] as f32 / 255.0,
                            c[2] as f32 / 255.0,
                            r.alpha * dim,
                        ];
                        // While the ring glides, a cart with no face of its own is smeared along
                        // the travel: one fainter copy behind it and both stretched a little. A
                        // jump crosses hundreds of carts in well under a second, and a plain cart
                        // sliding past at that speed reads as a slideshow; the smear is what tells
                        // the eye it is moving fast. Only the placeholder carts get it — a cart
                        // wearing a real face is the game's own picture, and smearing that would be
                        // a lie about the art — and at rest the stretch is exactly 1.
                        let (dir, grow) = match self.glide_dir() {
                            Some(dir) => (dir, r.w * (SMEAR - 1.0)),
                            None => (0.0, 0.0),
                        };
                        if grow > 0.0 {
                            let alpha = r.alpha * dim * 0.30;
                            let tx = r.x - dir * grow * 2.0;
                            out.push(Draw::Tex {
                                x: tx - grow / 2.0,
                                y: r.y,
                                w: r.w + grow,
                                h: r.h,
                                tex,
                                alpha,
                            });
                            out.push(Draw::Rect {
                                x: tx + x0 as f32 - grow / 2.0,
                                y: r.y + y0 as f32,
                                w: (x1 - x0) as f32 + grow,
                                h: (y1 - y0) as f32,
                                colour: [ink[0], ink[1], ink[2], alpha],
                            });
                        }
                        out.push(Draw::Tex {
                            x: r.x - grow / 2.0,
                            y: r.y,
                            w: r.w + grow,
                            h: r.h,
                            tex,
                            alpha: r.alpha * dim,
                        });
                        out.push(Draw::Rect {
                            x: r.x - grow / 2.0 + x0 as f32,
                            y: r.y + y0 as f32,
                            w: (x1 - x0) as f32 + grow,
                            h: (y1 - y0) as f32,
                            colour: ink,
                        });
                        continue;
                    }
                    None => {
                        let c = label_colour(&label_text(cart));
                        Draw::Rect {
                            x: r.x,
                            y: r.y,
                            w: r.w,
                            h: r.h,
                            colour: [
                                c[0] as f32 / 255.0,
                                c[1] as f32 / 255.0,
                                c[2] as f32 / 255.0,
                                r.alpha * dim,
                            ],
                        }
                    }
                },
            });
        }
    }

    /// Where each cart the row would draw sits this frame, in draw order and in offscreen
    /// pixels. The one piece of layout the row and a mark drawn over it — the favourites star —
    /// must agree about, so it is stated once and read twice rather than written twice.
    ///
    /// `hidden` drops one cart by stem, which is how the shelf gets out of the way of the copy
    /// the chrome is sliding into the slot.
    pub fn slot_rects(&self, hidden: Option<&str>, shake: f32, recede: f32) -> Vec<SlotRect> {
        let recede = recede.clamp(0.0, 1.0);
        let n = self.view.len() as i64;
        if n == 0 {
            return Vec::new();
        }
        // The row is laid out around where it *is*, not around the cart it is heading for.
        // While a letter jump glides, `scroll` sweeps across everything between two letters
        // and each of those carts has to be drawn where `scroll` puts it this frame; anchoring
        // on the destination would leave the row blank for the length of the sweep. At rest
        // the two are the same place, so a still row is drawn exactly as it always was.
        let base = self.scroll.round() as i64;
        let mut out = Vec::new();
        for slot in -SLOTS..=SLOTS {
            let off = slot as i64;
            let r = off.rem_euclid(n);
            // Only a cart's representative nearest the middle is drawn, so a row too short to
            // fill its slots does not show the same cart on both sides of the selection.
            let nearest = if r * 2 > n { r - n } else { r };
            if nearest != off {
                continue;
            }
            let coord = base + off;
            let i = self.view[coord.rem_euclid(n) as usize];
            let cart = &self.carts[i];
            if hidden == Some(cart.stem.as_str()) {
                continue;
            }
            let offset = coord as f32 - self.scroll;
            let t = offset.abs().min(1.0);
            let scale = CENTER_SCALE + (SIDE_SCALE - CENTER_SCALE) * t;
            let alpha = (1.0 + (SIDE_ALPHA - 1.0) * t) * (1.0 - recede);
            let (w, h) = (CART_W as f32 * scale, CART_H as f32 * scale);
            // Away from the middle, and further the further out it already was, so the row
            // opens rather than sliding sideways.
            let away = offset.signum() * (1.0 + offset.abs());
            let x = OUT_W as f32 / 2.0 + offset * PITCH - w / 2.0 + away * PART * recede;
            if x + w <= 0.0 || x >= OUT_W as f32 || alpha <= 0.0 {
                continue;
            }
            out.push(SlotRect {
                cart: i,
                x: x + shake,
                y: FOOT_Y - h,
                w,
                h,
                alpha,
            });
        }
        out
    }
}

/// How solid the shadow under a dimmed cart is. It carries the whole of the cart's opacity
/// while the face is translucent over it, and leaves with the face as the row parts.
fn recede_alpha(face_alpha: f32) -> f32 {
    (face_alpha / SIDE_ALPHA).clamp(0.0, 1.0)
}
