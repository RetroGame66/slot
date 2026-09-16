//! Everything the binary does with a compositor except own one. The window is the only
//! difference between the host and the device, so it is the only thing left above this.

use std::time::{Duration, Instant};

use slot_gfx::{Compositor, Draw, TexId, OUT_H, OUT_W};
use slot_input::{InputSource, Millis};
use slot_power::{Platform, Power};
use slot_store::format_stamp;
use slot_ui::{
    arrows_hint_face, cart_face, cart_shadow, cheat_row_face, chip_face, chip_shadow_face,
    clean_label, hhmm, hint_face, icon_face, letters, menu_face, photo_face, set_clock_hint_face,
    shelf_title_face, socket_face, sticker_face, title_face, toast_face, wallpaper_face, word_face,
    Icon, PowerChoice, StickerFields, Toast, ALERT_PX, BOLT_PX, HUD_ICON_PX, HUD_INK, LEGEND,
};

use crate::app::{App, GameRow, LinkRow, Phase};
use crate::build_info::Build;
use crate::face_builder::{ring_distance, shelf_window, FaceBuilder, ShelfFaceFiller};
use crate::link_start::{LinkFail, LinkStep};
use crate::root;
use crate::session::Session;
use crate::wallpaper;

/// How long a dark panel waits before the machine actually stops. The dark is immediate —
/// the lid or the button kills the backlight on the edge — but the device is still running
/// flat out behind it at 400-700 mA, so this is the window in which the user might come
/// straight back, not a power saving.
///
/// Three minutes, and then the device powers off rather than sleeping. It cannot wake itself
/// from a sleep — the RTC alarm never fires on this board — so a standby would be a leak with
/// no end, and a power off is the honest version of putting it down.
const DOZE_TIMEOUT: Duration = Duration::from_secs(180);

/// Amber. The only warning colour in the tree, and the reason it is not the HUD's ink: a
/// refusal that looks like a volume glyph is a refusal nobody reads as one.
const ALERT_INK: [u8; 3] = [0xf0, 0xb4, 0x3c];

/// Carts either side of the caret that keep a face texture resident.
///
/// The shelf draws three. Twelve leaves room for a flick's overshoot and for the caret's own
/// neighbours to already be there when it lands, while bounding the shelf's memory at about
/// sixteen megabytes whatever the card holds — which is what makes a nine-hundred-game card
/// the same proposition as a hundred-game one.
const RESIDENT: usize = 12;

/// How far past `RESIDENT` a cart may sit before its texture is released. The slack is what
/// stops a caret parked on a boundary from releasing and rebuilding the same cart every step.
const RESIDENT_HYSTERESIS: usize = 3;

pub struct Frontend {
    session: Session,
    start: Instant,
    last: Instant,
    draws: Vec<Draw>,
    /// One texture per ring slot, reused every time the switcher opens.
    polaroid_texes: Vec<TexId>,
    /// The top plate's line of type, re-rasterised whenever the selection moves.
    title_tex: Option<TexId>,
    /// The shelf's own line of type, and the name it was built for. The same deal as the top
    /// plate's, on the screen that needs it most: one texture, rebuilt when the cart under the
    /// eye changes and at no other time.
    shelf_title_tex: Option<TexId>,
    shelf_titled: Option<String>,
    /// Builds the open cart's faces off the frame loop.
    faces: FaceBuilder,
    /// Builds the shelf's remaining cart faces off the frame loop, nearest the caret first.
    shelf_faces: ShelfFaceFiller,
    /// Carts whose face is not on the shelf yet. Ordered by the request, not by the list.
    shelf_missing: Vec<usize>,
    /// The caret position the resident window was last built for.
    resident_at: Option<usize>,
    /// The cart last asked for.
    core_asked: Option<String>,
    /// The open cart and its lid, and which cart they were built for.
    core_board_tex: Option<TexId>,
    core_lid_tex: Option<TexId>,
    core_built: Option<String>,
    /// The undo cap's label, which changes with what is on offer.
    undo_tex: Option<TexId>,
    switcher: Switcher,
    clocks: Clocks,
    about: AboutFace,
    /// Whether the first frame's uptime has been written to `boot.log`. One line, once, so the
    /// log says how long the device took to reach the shelf and which base it did it on.
    frame_logged: bool,
}

/// The about label, and what it was last built for. The gauge is the only thing on it that
/// moves, so the reading is what decides whether it is rebuilt.
#[derive(Default)]
struct AboutFace {
    tex: Option<TexId>,
    /// `None` is a board with no gauge, which is a different thing from not having built one
    /// yet — `tex` says that.
    battery: Option<u8>,
}

/// The clock screen's two faces and the shelf's one, with what each was last built for. The
/// picker's line changes under the caret; the shelf clock changes once a minute; the battery
/// percent changes whenever the reading does.
#[derive(Default)]
struct Clocks {
    line: Option<TexId>,
    hint: Option<TexId>,
    shelf: Option<TexId>,
    picked: Option<String>,
    shown: String,
    battery: String,
    battery_tex: Option<TexId>,
}

/// What the switcher's textures were built for. The photos and the undo cap are per opening;
/// the title is per selection.
#[derive(Default)]
struct Switcher {
    open: bool,
    titled: Option<String>,
}

impl Frontend {
    pub fn boot(platform: Box<dyn Platform>) -> Self {
        let now = Instant::now();
        let mut session = Session::boot(platform.root().to_path_buf());
        session
            .app_mut()
            .set_power(Power::new(platform, DOZE_TIMEOUT));
        Frontend {
            session,
            start: now,
            last: now,
            draws: Vec::new(),
            polaroid_texes: Vec::new(),
            title_tex: None,
            shelf_title_tex: None,
            shelf_titled: None,
            faces: FaceBuilder::spawn(),
            shelf_faces: ShelfFaceFiller::spawn(),
            shelf_missing: Vec::new(),
            resident_at: None,
            core_asked: None,
            core_board_tex: None,
            core_lid_tex: None,
            core_built: None,
            undo_tex: None,
            switcher: Switcher::default(),
            clocks: Clocks::default(),
            about: AboutFace::default(),
            frame_logged: false,
        }
    }

    /// The card's own typeface, if it has one. `System/fonts/` belongs to the user, and the
    /// whole point of reading it rather than baking a face in is that a script the embedded one
    /// does not carry — CJK, most obviously — should not need a rebuild to appear.
    ///
    /// Silent on both failure paths on purpose. A card with no font file is the shipped
    /// configuration; a card with a broken one is that same configuration a moment later.
    /// Neither is worth a line on a device with no console and a boot budget to protect.
    fn load_font(&self) {
        let Some(path) = self.session.app().root().and_then(root::font_file) else {
            return;
        };
        if let Ok(bytes) = std::fs::read(&path) {
            slot_ui::text::set_font(bytes);
        }
    }

    /// Everything that never changes: the carts, the HUD glyphs and the key caps. All of it
    /// needs a live context, so it happens after the compositor and not at boot.
    pub fn upload_faces(&mut self, compositor: &mut Compositor) {
        // Timed: this is the one part of the boot that scales with the number of games on the
        // card, and the number that says whether a card's covers or the shelf's faces are what
        // the wait is being spent on.
        let faces_at = Instant::now();
        // Before anything is rasterised, and so before anything has an opinion about type: the
        // card's face has to be in place for the first cart's label, not for the second frame.
        let font_at = Instant::now();
        self.load_font();
        // Its own line, because it is the one cost here that does not scale with anything:
        // the whole card font is read off the SD card and parsed before a glyph can be drawn.
        self.session
            .boot_note(&format!("font load {} ms", font_at.elapsed().as_millis()));
        // Every title on the card, checked once against both faces: a subset font goes stale
        // the moment a game is added, and a missing glyph fails silently as a blank.
        let mut missing: Vec<char> = Vec::new();
        for cart in self.session.app().carts() {
            for ch in slot_ui::text::missing_in(&slot_ui::label_text(cart)) {
                if !missing.contains(&ch) {
                    missing.push(ch);
                }
            }
        }
        if !missing.is_empty() {
            self.session.boot_note(&format!(
                "font missing {} chars: {}",
                missing.len(),
                missing.iter().collect::<String>()
            ));
        }
        let carts_at = Instant::now();
        // Only the carts the shelf can actually draw. Everything else is rasterised behind
        // the caret by `shelf_faces` while the user reads the row: see `ShelfFaceFiller`.
        let index = self.session.app().shelf_index();
        let window = shelf_window(index, self.session.app().carts().len());
        let mut faces: Vec<Option<TexId>> = vec![None; self.session.app().carts().len()];
        for &i in &window {
            let f = cart_face(&self.session.app().carts()[i]);
            faces[i] = Some(compositor.create_texture(f.w, f.h, &f.rgba));
        }
        let fixed_at = Instant::now();
        self.session.boot_note(&format!(
            "cart faces ({} built of {}) {} ms",
            window.len(),
            self.session.app().carts().len(),
            carts_at.elapsed().as_millis()
        ));
        self.session.app_mut().set_faces(faces);
        self.shelf_missing = (0..self.session.app().carts().len())
            .filter(|i| !window.contains(i))
            .collect();
        let icons = Icon::ALL
            .iter()
            .map(|i| {
                let f = icon_face(*i, HUD_ICON_PX, HUD_INK);
                compositor.create_texture(f.w, f.h, &f.rgba)
            })
            .collect();
        self.session.app_mut().set_icon_faces(icons);
        // Its own upload rather than one of the HUD's: it is drawn on a cart, at its own
        // size, and in a warning colour the level glyphs have no business borrowing.
        let alert = icon_face(Icon::Alert, ALERT_PX, ALERT_INK);
        let alert = compositor.create_texture(alert.w, alert.h, &alert.rgba);
        self.session.app_mut().set_alert_face(alert);
        // Uploaded at boot like everything else: a shutdown is the one moment there is no
        // time to rasterise anything, and the GPU is about to be taken away. One line per
        // choice, in `PowerChoice::ALL` order, at the menu's own size so the screen that
        // follows a choice is set in the same voice as the row that was chosen.
        let lines = PowerChoice::ALL
            .iter()
            .map(|c| {
                let f = menu_face(match c {
                    PowerChoice::Restart => "正在重启",
                    PowerChoice::PowerOff => "正在关机",
                });
                (compositor.create_texture(f.w, f.h, &f.rgba), f.w, f.h)
            })
            .collect();
        self.session.app_mut().set_shutdown_faces(lines);
        let menu = PowerChoice::ALL
            .iter()
            .map(|c| {
                let f = menu_face(c.text());
                (compositor.create_texture(f.w, f.h, &f.rgba), f.w, f.h)
            })
            .collect();
        self.session.app_mut().set_power_menu_faces(menu);
        // The open cart's parts that never change: each socket, the chip seated in each, the
        // blank chip in flight and its shadow, in `Core::ALL` order. At boot like the power
        // menu's rows, so the first frame of a lid coming off is not spent in a rasteriser.
        let sockets = slot_store::Core::ALL
            .iter()
            .map(|c| {
                let f = socket_face(*c);
                compositor.create_texture(f.w, f.h, &f.rgba)
            })
            .collect();
        let chips = slot_store::Core::ALL
            .iter()
            .map(|c| {
                let f = chip_face(Some(*c));
                compositor.create_texture(f.w, f.h, &f.rgba)
            })
            .collect();
        let blank = chip_face(None);
        let blank = compositor.create_texture(blank.w, blank.h, &blank.rgba);
        let shadow = chip_shadow_face();
        let shadow = compositor.create_texture(shadow.w, shadow.h, &shadow.rgba);
        self.session
            .app_mut()
            .set_core_part_faces(sockets, chips, blank, shadow);
        // Every action the picker takes, the way out first and the choice last, as the
        // switcher's legend is ordered.
        let legend = [
            hint_face("B", "取消"),
            arrows_hint_face("切换"),
            hint_face("A", "选择"),
        ]
        .into_iter()
        .map(|f| (compositor.create_texture(f.w, f.h, &f.rgba), f.w))
        .collect();
        self.session.app_mut().set_core_legend_faces(legend);
        // The in-game menu, its two link rows, and the sentences the screen says while a
        // link is coming up or after it did not. All of it at the same size and through the
        // same rasteriser as the two menus above, because they are the same object — and all
        // of it at boot, because a link that is failing is the worst moment to be asking a
        // font for a sentence.
        let rows = menu_faces(compositor, GameRow::ALL.iter().map(|r| r.text()));
        self.session.app_mut().set_game_menu_faces(rows);
        let link = menu_faces(compositor, LinkRow::ALL.iter().map(|r| r.text()));
        self.session.app_mut().set_link_menu_faces(link);
        let steps = menu_faces(compositor, LinkStep::ALL.iter().map(|s| s.line()));
        self.session.app_mut().set_link_step_faces(steps);
        let fails = menu_faces(compositor, LinkFail::SHOWN.iter().map(|f| f.line()));
        self.session.app_mut().set_link_fail_faces(fails);
        let toasts = Toast::ALL
            .iter()
            .map(|t| {
                let f = toast_face(*t);
                compositor.create_texture(f.w, f.h, &f.rgba)
            })
            .collect();
        self.session.app_mut().set_toast_faces(toasts);
        // The cheat table's "this cart has no codes" panel, rasterised at boot like the other
        // fixed strings so opening it never waits on a font.
        let empty = menu_face("NO CHEATS");
        let empty = (
            compositor.create_texture(empty.w, empty.h, &empty.rgba),
            empty.w,
            empty.h,
        );
        self.session.app_mut().set_cheat_empty_face(empty);
        let legend = legend_faces(compositor, &LEGEND);
        self.session.app_mut().set_legend_faces(legend);
        let shadow = cart_shadow();
        let id = compositor.create_texture(shadow.w, shadow.h, &shadow.rgba);
        self.session.app_mut().set_cart_shadow(id);
        // `draw_gauge` now draws the bolt beside the capsule, on the housing, in its own
        // reserved slot rather than over the fill. The housing tint was only ever needed to
        // hide the bolt inside the fill it sat on; out here it sits where every other HUD
        // glyph does, so it takes the same ink they do.
        let bolt = icon_face(Icon::Charging, BOLT_PX, HUD_INK);
        let bolt_id = compositor.create_texture(bolt.w, bolt.h, &bolt.rgba);
        self.session.app_mut().set_bolt_face(bolt_id);
        // The letter drum: one face per slot of a fixed alphabet, the housing it shows through,
        // and the ridge that joins two of its facets — twenty-nine small textures against the
        // cart faces' one apiece. None of them can ever change, so they are built here with the
        // rest of the fixed furniture rather than lazily the way a cart's is.
        let letter_faces = letters::SLOTS
            .iter()
            .map(|ch| {
                let f = letters::letter_face(*ch);
                (compositor.create_texture(f.w, f.h, &f.rgba), f.w, f.h)
            })
            .collect();
        self.session.app_mut().set_letter_faces(letter_faces);
        let (w, h) = letters::capsule_size();
        let capsule = letters::capsule_face(w, h);
        let capsule_id = compositor.create_texture(capsule.w, capsule.h, &capsule.rgba);
        self.session
            .app_mut()
            .set_letter_capsule_face(capsule_id, capsule.w, capsule.h);
        let ridge = letters::ridge_face();
        let ridge_id = compositor.create_texture(ridge.w, ridge.h, &ridge.rgba);
        self.session.app_mut().set_letter_ridge_face(ridge_id);
        // Everything above is one texture per fixed string or glyph: none of it scales with
        // the card, all of it has to be rasterised before the first frame, and it is the last
        // large block left in the boot once the font and the cart faces are accounted for.
        self.session.boot_note(&format!(
            "fixed faces {} ms",
            fixed_at.elapsed().as_millis()
        ));
        let wall_at = Instant::now();
        self.upload_wallpaper(compositor);
        // The card's wallpaper, decoded and scaled. A card whose wallpaper is a photograph
        // pays for it here, once, in the same way a label used to.
        self.session
            .boot_note(&format!("wallpaper {} ms", wall_at.elapsed().as_millis()));
        self.upload_faces_note(faces_at);
    }

    /// The third boot line: how long the card's faces took, which is the only part of the boot
    /// that grows with the library. Split out so the timing call does not need a borrow of the
    /// compositor that `upload_faces` has already given back.
    fn upload_faces_note(&self, since: Instant) {
        self.session.boot_note(&format!(
            "shelf faces total {} ms",
            since.elapsed().as_millis()
        ));
        // Where that time went, by pass. A hundred-game card and a seventy-millisecond face
        // is a number worth splitting before anyone decides what to optimise.
        self.session.boot_note(&format!(
            "cart face phases: {}",
            slot_ui::face_profile::line()
        ));
    }

    /// One decode, at boot. A card with no `Wallpapers`, no readable picture in it, or a
    /// picture the decoder will not take, gets the plain ground it had before.
    fn upload_wallpaper(&mut self, compositor: &mut Compositor) {
        let app = self.session.app();
        let seed = app.wall_secs().unsigned_abs();
        let Some(rgba) = app
            .root()
            .and_then(|root| wallpaper::pick(root, seed))
            .and_then(|path| wallpaper_face(&path))
        else {
            return;
        };
        let id = compositor.create_texture(OUT_W, OUT_H, &rgba);
        self.session.app_mut().set_wallpaper(id);
    }

    /// Upload whatever the shelf filler finished, then ask it for the cart nearest the caret
    /// that is still missing a face.
    ///
    /// One request in flight at a time: the answer is always wanted, and a queue would only
    /// let the worker fall further behind where the caret actually is.
    fn pump_shelf_faces(&mut self, compositor: &mut Compositor) {
        self.sync_resident_faces(compositor);
        for (i, f) in self.shelf_faces.drain() {
            let id = compositor.create_texture(f.w, f.h, &f.rgba);
            self.session.app_mut().set_face(i, id);
        }
        if self.shelf_faces.busy() || self.shelf_missing.is_empty() {
            return;
        }
        let count = self.session.app().carts().len();
        let index = self.session.app().shelf_index();
        // Only within the resident window. Without this the filler would keep working
        // outward — the nearest missing cart is always just past the window — and the cap
        // would be a treadmill instead of a cap.
        let Some(pos) = self
            .shelf_missing
            .iter()
            .enumerate()
            .filter(|(_, &i)| ring_distance(i, index, count) <= RESIDENT)
            .min_by_key(|(_, &i)| ring_distance(i, index, count))
            .map(|(p, _)| p)
        else {
            return;
        };
        let i = self.shelf_missing.swap_remove(pos);
        let cart = self.session.app().carts()[i].clone();
        self.shelf_faces.request(i, cart);
    }

    /// Keeps the textures resident to a window around the caret, and the queue of carts still
    /// wanting one.
    ///
    /// A face is `FACE_W * FACE_H * 4` — a little over half a megabyte — and costs about 26 ms
    /// to rasterise, so a card of nine hundred games would hold nearly five hundred megabytes
    /// of shelf if every cart kept its own. The shelf draws three either side; this keeps
    /// twelve, releases what falls outside, and re-queues it for the build it will need if the
    /// caret ever comes back.
    ///
    /// Runs on the caret's edge rather than every frame: it walks the whole library, and the
    /// caret moves at most a few times a second.
    fn sync_resident_faces(&mut self, compositor: &mut Compositor) {
        let count = self.session.app().carts().len();
        if count == 0 {
            return;
        }
        let index = self.session.app().shelf_index();
        if self.resident_at == Some(index) {
            return;
        }
        self.resident_at = Some(index);

        self.shelf_missing.clear();
        let mut released = Vec::new();
        for i in 0..count {
            if let Some(tex) = self.session.app().face_of(i) {
                if ring_distance(i, index, count) > RESIDENT + RESIDENT_HYSTERESIS {
                    released.push((i, tex));
                }
            } else {
                self.shelf_missing.push(i);
            }
        }
        for (i, tex) in released {
            compositor.release_texture(tex);
            self.session.app_mut().clear_face(i);
            self.shelf_missing.push(i);
        }
    }

    /// One frame into the offscreen target and out to a surface of `window` pixels. The
    /// caller swaps: only it knows what presenting costs.
    pub fn render(&mut self, compositor: &mut Compositor, window: (u32, u32)) {
        // The one number that can be compared across base OSes, logged once on the frame the
        // user first sees. Everything else in `boot.log` is a duration, and a duration cannot
        // tell you which base you booted — nor does it carry what the kernel and initramfs cost
        // before this process started, which is exactly what a faster base trims.
        //
        // Device uptime is the right clock: it starts at kernel boot, so it includes the kernel
        // and initramfs, and the bootloader before it is the same whichever base is underneath.
        // The base's own version comes with it so the log says what it was measured on.
        if !self.frame_logged {
            self.frame_logged = true;
            let uptime = std::fs::read_to_string("/proc/uptime")
                .ok()
                .and_then(|s| s.split_whitespace().next().map(str::to_string))
                .unwrap_or_else(|| "?".into());
            let base = std::fs::read_to_string("/etc/os-release")
                .ok()
                .and_then(|s| {
                    s.lines().find(|l| l.starts_with("VERSION_ID=")).map(|l| {
                        l["VERSION_ID=".len()..]
                            .trim()
                            .trim_matches('"')
                            .to_string()
                    })
                })
                .unwrap_or_else(|| "unknown".into());
            self.session.boot_note(&format!(
                "first frame at {} s of uptime  (base VERSION_ID={})",
                uptime, base
            ));
        }
        // First, so a face that finished since the last frame is on screen this frame.
        self.pump_shelf_faces(compositor);
        // Set every frame rather than on the edge: the grade is part of the final blit, so
        // it has to be right whether or not anything just changed it.
        compositor.set_blue_light(self.session.app().blue_light());
        compositor.set_shake(self.session.app().screen_shake());
        compositor.set_screen_power(self.session.app().screen_power());
        // The card's display filter, every frame so a SELECT+X (mask) or SELECT+Y (colour)
        // press lands on the next one.
        compositor.set_panel_mask(&self.session.app().display_mask());
        compositor.set_color_correction(&self.session.app().display_cc());
        compositor.set_cc_gamma(self.session.app().display_cc_gamma());
        compositor.begin_frame();
        if let Some(frame) = self.session.frame() {
            compositor.upload_game(&frame);
        }
        sync_clock(self.session.app_mut(), compositor, &mut self.clocks);
        sync_about(self.session.app_mut(), compositor, &mut self.about);
        sync_shelf_title(
            self.session.app_mut(),
            compositor,
            &mut self.shelf_title_tex,
            &mut self.shelf_titled,
        );
        sync_core_picker(
            self.session.app_mut(),
            compositor,
            &self.faces,
            &mut self.core_asked,
            &mut self.core_board_tex,
            &mut self.core_lid_tex,
            &mut self.core_built,
        );
        sync_switcher(
            self.session.app_mut(),
            compositor,
            Faces {
                pool: &mut self.polaroid_texes,
                title: &mut self.title_tex,
                undo: &mut self.undo_tex,
            },
            &mut self.switcher,
        );
        sync_cheat_faces(self.session.app_mut(), compositor);
        self.draws.clear();
        self.session.app().draw(&mut self.draws);
        compositor.draw_list(&self.draws);
        compositor.end_frame(window);
    }

    /// Input and time, after the frame is on screen. The gesture windows expire on this
    /// whether or not anything was pressed, so it is called every frame.
    pub fn advance(&mut self, input: &mut dyn InputSource) {
        let now = self.now();
        let events = input.poll(now);
        self.session.feed(events, now);
        let dt = self.last.elapsed().as_secs_f32();
        self.last = Instant::now();
        self.session.update(dt);
    }

    fn now(&self) -> Millis {
        self.start.elapsed().as_millis() as Millis
    }

    pub fn powering_off(&self) -> bool {
        self.session.app().ready_to_power_off()
    }

    pub fn restarting(&self) -> bool {
        self.session.app().ready_to_restart()
    }

    pub fn restart(&mut self) {
        self.session.app_mut().restart();
    }

    /// The state was flushed on the edge that set `powering_off`, so there is nothing left to
    /// do but go.
    pub fn poweroff(&mut self) {
        self.session.app_mut().poweroff();
    }
}

/// A line of menu type per label, in the order they were handed over, each with the size it
/// was rastered at. Every menu on the device is drawn from a list shaped exactly like this,
/// so the four the in-game menu needs are built through one function rather than four copies
/// of the same three lines.
fn menu_faces<'a>(
    compositor: &mut Compositor,
    labels: impl Iterator<Item = &'a str>,
) -> Vec<(TexId, u32, u32)> {
    labels
        .map(|label| {
            let f = menu_face(label);
            (compositor.create_texture(f.w, f.h, &f.rgba), f.w, f.h)
        })
        .collect()
}

/// A screen's key caps, in the order the legend names them. None of them ever changes what
/// it says, so they are uploaded once and outlive every visit to that screen.
fn legend_faces(compositor: &mut Compositor, legend: &[(&str, &str)]) -> Vec<TexId> {
    legend
        .iter()
        .map(|(key, label)| {
            let f = hint_face(key, label);
            compositor.create_texture(f.w, f.h, &f.rgba)
        })
        .collect()
}

/// The switcher's textures, which outlive any one opening.
struct Faces<'a> {
    pool: &'a mut Vec<TexId>,
    title: &'a mut Option<TexId>,
    undo: &'a mut Option<TexId>,
}

/// Photos and the undo cap are built once per opening, on the way in, while the game is
/// already paused. Rebuilt each time rather than cached because the ring changes underneath
/// them. The title names the selection, so it follows a flick instead.
fn sync_switcher(app: &mut App, compositor: &mut Compositor, texes: Faces, state: &mut Switcher) {
    if !matches!(app.phase(), Phase::Polaroids { .. }) {
        state.open = false;
        return;
    }
    if !state.open {
        state.open = true;
        state.titled = None;
        let faces: Vec<_> = app.polaroid_entries().iter().map(photo_face).collect();
        let ids = faces
            .iter()
            .enumerate()
            .map(|(i, f)| match texes.pool.get(i) {
                Some(id) => {
                    compositor.update_texture(*id, f.w, f.h, &f.rgba);
                    *id
                }
                None => {
                    let id = compositor.create_texture_nearest(f.w, f.h, &f.rgba);
                    texes.pool.push(id);
                    id
                }
            })
            .collect();
        app.set_polaroid_faces(ids);

        // An offer can expire while the switcher is up but it cannot change into the other
        // kind, so the cap only has to be rasterised on the way in. Whether it is drawn at
        // all is the app's call.
        let label = app
            .undo_label()
            .map(|l| upload(compositor, texes.undo, hint_face("X", l)));
        app.set_undo_face(label);
    }
    if state.titled.as_deref() != app.polaroid_stamp() {
        state.titled = app.polaroid_stamp().map(str::to_string);
        let face = title_face(&app.polaroid_title(&format_stamp(app.wall_secs())));
        let id = upload(compositor, texes.title, face);
        app.set_polaroid_title_face(id);
    }
}

/// The picker is rasterised on every change under the caret, which is once per press. The
/// shelf clock follows the wall clock, so it is rebuilt when the minute turns and not on the
/// fifty nine seconds either side of it.
fn sync_clock(app: &mut App, compositor: &mut Compositor, clocks: &mut Clocks) {
    let picked = app.picker().map(|p| p.text());
    if picked != clocks.picked {
        clocks.picked = picked;
        if let Some(face) = app.picker().map(|p| p.face()) {
            let line = upload(compositor, &mut clocks.line, face);
            let hint = upload(compositor, &mut clocks.hint, set_clock_hint_face());
            app.set_clock_faces(line, hint);
        }
    }
    let shown = hhmm(app.wall_secs());
    if shown != clocks.shown {
        let face = word_face(&shown);
        clocks.shown = shown;
        let w = face.w;
        let id = upload(compositor, &mut clocks.shelf, face);
        app.set_shelf_clock_face(id, w);
    }
    let battery_shown = app
        .battery()
        .map(|b| format!("{}%", b.percent))
        .unwrap_or_default();
    if battery_shown != clocks.battery {
        clocks.battery = battery_shown.clone();
        if !battery_shown.is_empty() {
            let face = word_face(&battery_shown);
            let w = face.w;
            let id = upload(compositor, &mut clocks.battery_tex, face);
            app.set_battery_percent_face(id, w);
        }
    }
}

/// Built only once the screen is up: it is a 660 by 228 rasterisation and most sessions never
/// open it.
fn sync_about(app: &mut App, compositor: &mut Compositor, state: &mut AboutFace) {
    if !matches!(app.phase(), Phase::About) {
        return;
    }
    let battery = app.battery().map(|b| b.percent);
    if state.tex.is_some() && state.battery == battery {
        return;
    }
    state.battery = battery;
    let build = Build::current();
    let face = sticker_face(&StickerFields {
        battery,
        serial: &build.serial(),
        dirty_digit: build.dirty_digit(),
    });
    let id = upload(compositor, &mut state.tex, face);
    app.set_sticker_face(id);
}

/// The open cart's faces, asked for as soon as the caret lands on a cart and uploaded when the
/// worker hands them back, so they are normally on the GPU before START. The worker is the only
/// place they are built: rasterised on the frame loop, a board freezes the shelf for the better
/// part of half a second on the H700.
/// The game under the eye on the shelf, rebuilt only when the cart under the eye changes.
/// One rasterise per arrow press, and none at all on a frame where nothing moved.
fn sync_shelf_title(
    app: &mut App,
    compositor: &mut Compositor,
    slot: &mut Option<TexId>,
    shown: &mut Option<String>,
) {
    let want = app.selected_stem().map(str::to_string);
    if *shown == want {
        return;
    }
    *shown = want.clone();
    match want {
        Some(stem) => {
            // Named from the cart's own name, cut out of the filename the same way its printed
            // label is, so the two readings cannot disagree about what the game is called.
            let face = shelf_title_face(&clean_label(&stem));
            let (w, h) = (face.w, face.h);
            let id = upload(compositor, slot, face);
            app.set_shelf_title_face(Some((id, w, h)));
        }
        None => app.set_shelf_title_face(None),
    }
}

/// (Re)build the cheat table's row faces when the seated cart changes. Mirrors `sync_shelf_title`:
/// only rasterises when the highlight's stem differs from the one the faces were last built for,
/// so a long list is rasterised once per cart, not once per frame.
fn sync_cheat_faces(app: &mut App, compositor: &mut Compositor) {
    let want = app.selected_stem().map(str::to_string);
    let n = app.cheats().len();
    // Rebuild when the cart changes OR when the code count changes. Cheats load from the card
    // after the first build (when the core spawns), so an early build would otherwise cache empty
    // faces and the table would show chips with no text.
    if app.cheat_face_stem() == want.as_deref() && app.cheat_face_count() == n {
        return;
    }
    match want {
        Some(stem) => {
            let rows: Vec<(String, String)> = app
                .cheats()
                .iter()
                .map(|c| (c.desc.clone(), c.code.clone()))
                .collect();
            let faces: Vec<(TexId, u32, u32)> = rows
                .iter()
                .map(|(desc, code)| {
                    let f = cheat_row_face(desc, code);
                    (compositor.create_texture(f.w, f.h, &f.rgba), f.w, f.h)
                })
                .collect();
            app.set_cheat_faces(Some(stem), faces);
        }
        None => app.set_cheat_faces(None, Vec::new()),
    }
}

fn sync_core_picker(
    app: &mut App,
    compositor: &mut Compositor,
    builder: &FaceBuilder,
    asked: &mut Option<String>,
    board: &mut Option<TexId>,
    lid: &mut Option<TexId>,
    built: &mut Option<String>,
) {
    let highlighted = app.selected_stem().map(str::to_string);
    if highlighted.is_some() && *asked != highlighted {
        if let Some(cart) = app
            .carts()
            .iter()
            .find(|c| highlighted.as_deref() == Some(c.stem.as_str()))
        {
            builder.request(cart.clone());
        }
        *asked = highlighted.clone();
    }
    let Some(faces) = builder.take() else {
        return;
    };
    // A build for a cart the caret has since left is dropped; the one it is on is on its way.
    if highlighted.as_deref() != Some(faces.stem.as_str()) || *built == highlighted {
        return;
    }
    let board_id = upload_rgba(
        compositor,
        board,
        faces.board.w,
        faces.board.h,
        &faces.board.rgba,
    );
    let lid_id = upload_rgba(compositor, lid, faces.lid.w, faces.lid.h, &faces.lid.rgba);
    app.set_core_board_faces(board_id, lid_id);
    *built = Some(faces.stem);
}

fn upload(compositor: &mut Compositor, slot: &mut Option<TexId>, face: slot_ui::UndoFace) -> TexId {
    upload_rgba(compositor, slot, face.w, face.h, &face.rgba)
}

/// Into the slot's own texture if it has one, so the pool stops growing after the first time.
fn upload_rgba(
    compositor: &mut Compositor,
    slot: &mut Option<TexId>,
    w: u32,
    h: u32,
    rgba: &[u8],
) -> TexId {
    match *slot {
        Some(id) => {
            compositor.update_texture(id, w, h, rgba);
            id
        }
        None => {
            let id = compositor.create_texture(w, h, rgba);
            *slot = Some(id);
            id
        }
    }
}
