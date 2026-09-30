use std::path::Path;

use slot_gfx::{Draw, TexId, OUT_W};
use slot_store::Cart;

use crate::cart::{label_colour, label_text, size_for, CartSize};
use slot_store::System;
use crate::hud::Millis;
use crate::slot_chrome::draw_empty_slot;

/// The centre cart is drawn larger than the row so it reads as the one in hand; the
/// neighbours keep their full size but are pushed to the edges (see the `CartSize` pitch). 1.5
/// makes the selection clearly the hero without dwarfing the neighbours off screen.
///
/// Public because it is not the shelf's business alone: the app scales the cart into the slot
/// from this size, and a cart drawn at rest is a `CENTER_SCALE` cart to anything looking for
/// one.
pub const CENTER_SCALE: f32 = 1.5;
/// How much of its face a side cart keeps at full recede. Public for the same reason as
/// `CENTER_SCALE`: the app derives how much further to dim the neighbours from it, and that
/// arithmetic has to follow this number rather than be written down against an old one.
pub const SIDE_ALPHA: f32 = 0.7;
/// Carts stand on the row rather than float: the foot stays put as a cart shrinks away.
/// Where the row's feet stand — the carts stand on the row rather than float on it. Derived
/// from the cart's height, so a taller shell stands lower and the row keeps its proportions.
pub(crate) fn foot_y(size: CartSize) -> f32 {
    size.foot_y()
}
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

/// How far a cart is drawn from its resting place at the top of a system-shelf swap beat, in
/// offscreen pixels. Well past the top of the panel, so a risen cart is off screen and not
/// merely high.
const SWAP_RISE: f32 = 560.0;
/// The stagger between neighbouring slots, as a share of the beat: the centre cart leads and
/// each cart further out follows it, so the row is drawn away in order rather than at once.
const SWAP_STAGGER: f32 = 0.16;

/// The row's vertical motion while the system shelf is being swapped.
///
/// `p` is 0..1 through the current beat; `ascend` is true while the old shelf is being drawn
/// up and away (the carts climb and fade), and false while the new shelf drops back into place
/// (the carts fall from above and fade in). Absent at rest, so a still row is laid out exactly
/// as it always was — this is the one thing a swap changes about the layout, and it is opt-in.
#[derive(Copy, Clone)]
pub struct Motion {
    pub p: f32,
    pub ascend: bool,
}

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
    /// Which machine's shelf is up. Every cart on the row belongs to it, and so does the row's
    /// geometry: a Game Boy cart is portrait and a Game Boy shelf is its own layout.
    system: System,
    /// The carts the ring actually walks, as indices into `carts`. The full library when
    /// nothing is filtered; the starred ones while the favourites shelf is up. A list of
    /// indices rather than a rebuilt cart list, and that is the whole point: `faces` stays
    /// indexed by the library, so every texture the row was showing is the same texture after
    /// the view changes. Rebuilding the shelf instead would drop those faces and put blank
    /// placeholders on screen while they were rasterised again.
    view: Vec<usize>,
    /// Each machine's own shelf, in the order that shelf shows — the three libraries the row
    /// can walk. `carts` is the card; these are the shelves, and the distinction is the whole
    /// reason this exists: a machine shelf is a *scattered* subset of the card's sorted list
    /// (this card files its three Game Boy carts at 27, 127 and 637 among nine hundred Advance
    /// ones), so any measurement of "how far away is this cart" that walks `carts` is measuring
    /// the wrong thing.
    machines: [Vec<usize>; 3],
    /// Each machine's own subfolders, sorted and distinct — the folders `folder_pick` chooses
    /// among. A cart loose in its machine's folder contributes none; the folders are read off the
    /// roms' own paths (`Games/<machine>/<folder>/<rom>`), so a card that files its games per
    /// machine and then per language needs no second index to be switchable.
    folders: [Vec<String>; 3],
    /// Which of a machine's subfolders its shelf is showing, or `None` for all of them. Per
    /// machine, so a folder picked on one shelf is still picked when the user comes back to it.
    folder_pick: [Option<usize>; 3],
    /// One entry per cart: its position in its own machine's shelf, or `u32::MAX` for a cart no
    /// shelf shows. Where a switch to that machine lands is the *front* of this list, which is
    /// what makes the front of every machine the set worth keeping faces for.
    pos_in_machine: Vec<u32>,
    /// One entry per cart: its position in `view`, or `u32::MAX` when the view does not hold it.
    /// A table rather than a search, because the filler asks this of every cart it could build,
    /// every frame.
    pos_in_view: Vec<u32>,
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
    /// The system-shelf swap in flight, if any. See `Motion`. `None` at rest.
    motion: Option<Motion>,
}

impl Shelf {
    pub fn new(carts: Vec<Cart>) -> Self {
        let view: Vec<usize> = (0..carts.len()).collect();
        let mut machines: [Vec<usize>; 3] = [Vec::new(), Vec::new(), Vec::new()];
        let mut pos_in_machine = vec![u32::MAX; carts.len()];
        for (i, c) in carts.iter().enumerate() {
            let list = &mut machines[Self::machine_slot(c.system())];
            pos_in_machine[i] = list.len() as u32;
            list.push(i);
        }
        let mut folders: [Vec<String>; 3] = [Vec::new(), Vec::new(), Vec::new()];
        for c in &carts {
            let slot = Self::machine_slot(c.system());
            if let Some(f) = folder_of(&c.rom, c.system().dir_name()) {
                if !folders[slot].contains(&f) {
                    folders[slot].push(f);
                }
            }
        }
        for f in folders.iter_mut() {
            f.sort();
        }
        let mut pos_in_view = vec![u32::MAX; carts.len()];
        for (p, &i) in view.iter().enumerate() {
            pos_in_view[i] = p as u32;
        }
        Shelf {
            carts,
            machines,
            folders,
            folder_pick: [None, None, None],
            pos_in_machine,
            pos_in_view,
            system: System::Gba,
            view,
            index: 0,
            scroll: 0.0,
            faces: Vec::new(),
            shadow: None,
            placeholder: None,
            vel: 0.0,
            held: None,
            glide: None,
            motion: None,
        }
    }

    /// Sets the swap motion, or clears it. Driven by the app, which owns the timing; the row
    /// only reads it when laying out. `None` restores the resting layout exactly.
    pub fn set_motion(&mut self, motion: Option<Motion>) {
        self.motion = motion;
    }

    /// Which machine the row is showing. Set with the view, because the view is the machine's
    /// carts: a Game Boy shelf is laid out for portrait carts and an Advance shelf for
    /// landscape ones, and the row has to be told which it is looking at.
    pub fn set_system(&mut self, system: System) {
        self.system = system;
    }

    pub fn system(&self) -> System {
        self.system
    }

    /// The row's geometry: the cart's own size, the pitch between neighbours and how much
    /// bigger the selected one is.
    pub fn size(&self) -> CartSize {
        size_for(self.system)
    }

    /// Face textures in `carts` order. The caller uploads them because only the compositor
    /// can mint a `TexId`.
    ///
    /// Returns the one it replaced: the frame is rebuilt whenever the row's machine changes,
    /// because the shadow and the stand-in are both cut to that machine's cart, and a caller
    /// that cannot release the old texture leaks half a megabyte per shelf switch.
    pub fn set_shadow(&mut self, face: TexId) -> Option<TexId> {
        self.shadow.replace(face)
    }

    /// The blank cart a not-yet-built face stands in as. Set once at boot, with the shadow.
    /// Returns what it replaced, for the reason `set_shadow` does.
    pub fn set_placeholder(&mut self, face: TexId) -> Option<TexId> {
        self.placeholder.replace(face)
    }

    /// Face textures in `carts` order. The caller uploads them because only the compositor
    /// can mint a `TexId`.
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

    /// The position of a cart in the current *view*, by stem — i.e. what `index` counts in.
    /// `None` when the view does not hold it, which for a system shelf means a cart belonging
    /// to another machine. `find` searches the whole library; this searches only what the row
    /// is actually showing, which is what a caller about to set `index` needs.
    pub fn index_of_stem(&self, stem: &str) -> Option<usize> {
        self.view.iter().position(|&i| self.carts[i].stem == stem)
    }

    /// The position in the view of the first cart filed under letter `slot`. The letter strip
    /// counts the view (see `retally_letters`), so a seat on a letter has to be found in the
    /// view too: a library index would be the wrong cart on a shelf that holds a subset of it.
    pub fn view_position_of_letter(&self, slot: usize) -> Option<usize> {
        self.view
            .iter()
            .position(|&i| crate::letters::slot_of(self.carts[i].initial) == slot)
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
        for slot in self.pos_in_view.iter_mut() {
            *slot = u32::MAX;
        }
        for (p, &i) in view.iter().enumerate() {
            self.pos_in_view[i] = p as u32;
        }
        self.view = view;
        self.index = keep
            .and_then(|stem| self.view.iter().position(|&i| self.carts[i].stem == stem))
            .unwrap_or(0);
        self.scroll = self.index as f32;
        self.vel = 0.0;
        self.glide = None;
        self.held = None;
    }

    /// The carts the row is actually drawing, as indices into `carts`.
    ///
    /// This is the set a build has to cover, and it is **not** the same thing as a contiguous
    /// run of indices around the caret. A machine shelf and the favourites shelf are both
    /// *subsets* of the library — this card files its three Game Boy carts at 48..50 among nine
    /// hundred Advance ones — so a window that walks the library lands on nine hundred Advance
    /// carts and misses the three the row is showing. That is what "the side carts are still the
    /// placeholder" looked like on the device: the boot had built seven carts nobody was looking
    /// at.
    ///
    /// A cart's distance from the caret **in the row's own order**, or `None` when the view
    /// does not hold it.
    ///
    /// Every measurement of "worth keeping a face for" has to be this one and not a library
    /// distance. A machine shelf is a scattered subset of the library — this card's three Game
    /// Boy carts sit at 27, 127 and 637 among nine hundred Advance ones, because the library is
    /// sorted by title and the row is not — so the two carts either side of the caret are a
    /// hundred and three hundred indices away. A resident window measured in the library
    /// therefore releases exactly the faces the user is looking at, and the filler spends its
    /// time on carts that are on no shelf at all.
    pub fn view_distance(&self, cart: usize) -> Option<usize> {
        let n = self.view.len();
        if n == 0 {
            return None;
        }
        let pos = *self.pos_in_view.get(cart)?;
        if pos == u32::MAX {
            return None;
        }
        let d = (pos as usize).abs_diff(self.index);
        Some(d.min(n - d))
    }

    /// Which of the three shelves a machine's carts live on.
    fn machine_slot(s: System) -> usize {
        match s {
            System::Gba => 0,
            System::Gb => 1,
            System::Gbc => 2,
        }
    }

    /// One machine's shelf: its carts as library indices, in the order the row shows them, cut to
    /// the subfolder `folder_pick` has chosen for that machine (`None` leaves all of them).
    pub fn machine_view(&self, s: System) -> Vec<usize> {
        let slot = Self::machine_slot(s);
        let all = &self.machines[slot];
        let Some(name) = self.folder_pick[slot].and_then(|p| self.folders[slot].get(p)) else {
            return all.clone();
        };
        all.iter()
            .copied()
            .filter(|&i| {
                folder_of(&self.carts[i].rom, s.dir_name()).as_deref() == Some(name.as_str())
            })
            .collect()
    }

    /// Step one machine's subfolder filter: all -> the first folder -> ... -> all. `false` when
    /// the machine has no subfolders, so the caller can refuse the press rather than let it read
    /// as a key that worked.
    pub fn cycle_folder(&mut self, s: System) -> bool {
        let slot = Self::machine_slot(s);
        let n = self.folders[slot].len();
        if n == 0 {
            return false;
        }
        self.folder_pick[slot] = match self.folder_pick[slot] {
            None => Some(0),
            Some(p) if p + 1 < n => Some(p + 1),
            Some(_) => None,
        };
        true
    }

    /// The subfolder one machine's shelf is showing, or `None` when it is showing them all.
    pub fn folder_name(&self, s: System) -> Option<&str> {
        let slot = Self::machine_slot(s);
        self.folders[slot]
            .get(self.folder_pick[slot]?)
            .map(String::as_str)
    }

    /// How far a cart is from the caret that would be showing it.
    ///
    /// Two roads to the same cart, and the nearer one wins. If the view up holds it, that is the
    /// distance that matters — the row on screen, favourites included. Otherwise it is counted
    /// from the *front* of its own machine's shelf, because that is where a switch to that
    /// machine parks the caret. Keeping both windows resident is what makes a machine switch
    /// show real carts on the frame it lands rather than a row of stand-ins: by the time the
    /// user reaches for the machine, its first screen is already built.
    ///
    /// O(1), deliberately: the filler asks this of every cart it could build, every frame.
    pub fn shelf_distance(&self, cart: usize) -> Option<usize> {
        let here = self.view_distance(cart);
        let front = self
            .pos_in_machine
            .get(cart)
            .copied()
            .filter(|&p| p != u32::MAX)
            .map(|p| p as usize);
        match (here, front) {
            (Some(a), Some(b)) => Some(a.min(b)),
            (a, b) => a.or(b),
        }
    }

    /// Same walk as `slot_rects`, which is what draws them: the slots either side of the caret
    /// in view order, and a cart's representative nearest the middle when the view is too short
    /// to fill the row.
    pub fn draw_window(&self) -> Vec<usize> {
        let n = self.view.len() as i64;
        if n == 0 {
            return Vec::new();
        }
        let base = self.scroll.round() as i64;
        let mut out = Vec::new();
        for slot in -SLOTS..=SLOTS {
            let off = slot as i64;
            let r = off.rem_euclid(n);
            let nearest = if r * 2 > n { r - n } else { r };
            if nearest != off {
                continue;
            }
            let i = self.view[(base + off).rem_euclid(n) as usize];
            if !out.contains(&i) {
                out.push(i);
            }
        }
        out
    }

    /// The carts the current view walks, in view order. A view is a list of indices, so this
    /// is how a caller that needs the carts themselves — the letter tally — reads one.
    pub fn visible(&self) -> Vec<&Cart> {
        self.view.iter().map(|&i| &self.carts[i]).collect()
    }

    /// How many carts the current view holds. The library count and this differ on every
    /// shelf that is a subset of it, which is the distinction the boot's face window has to
    /// respect.
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
            // The shadow exists to keep a dimmed cart solid: drawn at 0.7 the face is see-through,
            // and over a wallpaper a translucent cart is a ghost. A cart that came with its own
            // art does not need it — the art is opaque — and drawing it anyway leaves the
            // silhouette showing through the art's antialiased edge as a dark rim on a light
            // theme, which is exactly what it looked like.
            if r.alpha < 1.0 && crate::cart_art::art_for(self.system).is_none() {
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
                        let (x0, y0, x1, y1) = self.size().label_panel_at(r.w as u32, r.h as u32);
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
            let size = self.size();
            let scale = size.middle + (size.side - size.middle) * t;
            let mut alpha = (1.0 + (SIDE_ALPHA - 1.0) * t) * (1.0 - recede);
            let (w, h) = (size.w as f32 * scale, size.h as f32 * scale);
            // Away from the middle, and further the further out it already was, so the row
            // opens rather than sliding sideways.
            let away = offset.signum() * (1.0 + offset.abs());
            let x = OUT_W as f32 / 2.0 + offset * size.pitch - w / 2.0 + away * PART * recede;
            let mut y = foot_y(size) - h;
            // The system-shelf swap, when one is running. The centre cart leads the beat and
            // each cart further out follows it — drawn up and away on the way out, fallen back
            // from above on the way in — so the row moves in order rather than all at once.
            if let Some(m) = self.motion {
                let lead = SWAP_STAGGER * (slot as f32).abs();
                let span = (1.0 - SWAP_STAGGER * SLOTS as f32).max(0.05);
                let f = ((m.p - lead) / span).clamp(0.0, 1.0);
                let e = f * f * (3.0 - 2.0 * f);
                if m.ascend {
                    y -= e * SWAP_RISE;
                    alpha *= 1.0 - e;
                } else {
                    y -= (1.0 - e) * SWAP_RISE;
                    alpha *= e;
                }
            }
            if x + w <= 0.0 || x >= OUT_W as f32 || alpha <= 0.0 {
                continue;
            }
            out.push(SlotRect {
                cart: i,
                x: x + shake,
                y,
                w,
                h,
                alpha,
            });
        }
        out
    }
}

/// The directory the scanner reads games out of, spelled as the scanner spells it. Kept here only
/// so `folder_of` can tell a subfolder from the root the subfolders live under.
const GAMES_DIR: &str = "Games";

/// The subfolder a cart is filed under, for the shelf's per-machine folder switch.
///
/// A card may file its games however it likes under `Games/`: loose (`Games/x.gba`), one level of
/// machine folder (`Games/GBA/x.gba`), a folder inside that (`Games/GBA/GBA中文/x.gba`), or with no
/// machine folder at all (`Games/GBA中文/x.gba`) — the machine is read off the *extension*, so all
/// four layouts are the same card to everything else. The subfolder is therefore whatever sits
/// between the machine (when there is one) and the file: the name directly above the rom, and
/// `None` when that name is the machine folder or the `Games` root itself. Both of those are
/// otherwise ordinary directory names, which is exactly why the root is found by name rather than
/// by counting — a loose rom's parent is `Games`, and mistaking that for a folder would offer a
/// "subfolder" on every flat card and switch to a set of carts identical to the one already up.
fn folder_of(rom: &Path, machine: &str) -> Option<String> {
    let parts: Vec<&str> = rom
        .components()
        .filter_map(|c| c.as_os_str().to_str())
        .collect();
    let root = parts.iter().rposition(|p| *p == GAMES_DIR)?;
    let mut rest = &parts[root + 1..];
    if rest.first() == Some(&machine) {
        rest = &rest[1..];
    }
    // Whatever is left in front of the file is the subfolder, if any is left at all.
    (rest.len() >= 2).then(|| rest[0].to_string())
}

/// How solid the shadow under a dimmed cart is. It carries the whole of the cart's opacity
/// while the face is translucent over it, and leaves with the face as the row parts.
fn recede_alpha(face_alpha: f32) -> f32 {
    (face_alpha / SIDE_ALPHA).clamp(0.0, 1.0)
}
