use std::path::{Path, PathBuf};

use slot_gfx::{System as ScreenSystem, OUT_H, OUT_W};
use slot_input::{Action, Btn, MUTE_CHORD_MS};
use slot_power::{Battery, Charge, LedState, LidPolicy, Power};
use slot_retro::LinkChannel;
use slot_store::{
    format_stamp, read_slot_state, scan_cached, write_slot_state, Cart, Core, SlotState,
    StateEntry, StateRing, System, Theme, BLUE_LIGHT_MAX, BRIGHTNESS_MAX, RING_MAX, VOLUME_MAX,
};
use slot_ui::lang;
use slot_ui::{
    board_at, board_zoom, draw_backdrop, draw_edge_glow, draw_empty_slot, draw_shortcut_row,
    draw_status, draw_sticker_at, draw_top_band, ease, grown, letters, lid_at, lift_of, on_board,
    ClockPicker, Draw, FfState, Hud, HudKind, Icon, Letters, Millis, Placed, Polaroids,
    PowerChoice, Refusal, Shelf, SlotChrome, TexId, Toast, BOARD_W, BOARD_X, CART_W,
    CHIP_H, CHIP_U, CHIP_V, CHIP_W, HINT_EDGE, HINT_H, HOP_LIFT, MOUTH_H, PLATE_Y, SHADOW_H,
    SHADOW_W, SHELF_TITLE_H, SOCKET_H, SOCKET_U, SOCKET_V, SOCKET_W, STICKER_H, TURN_PAD,
};

use crate::audio::{Profile, Sfx};
use crate::core_picker::{Chip, CorePicker, Outcome, Press};
use crate::link_radio::LinkRole;
use crate::link_start::{link_port, LinkFail, LinkProgress, LinkStarter, LinkStep};
use crate::persist::{self, Snapshot};

/// A floor, not a delay. The animation is where the core load hides, so a slow load
/// extends it and a load that is already done still waits it out.
pub const INSERT_S: f32 = 0.73;
/// The tail of the insert, spent on a cart that has already landed. The game arriving on the
/// frame the cart seats reads as a cut; a beat of nothing first says the cart caused it.
///
/// Long enough to cover the rest of the sound of it landing and leave a little air after it,
/// which `the_game_waits_for_the_cart_to_finish_landing` holds against the clip.
const INSERT_HOLD_S: f32 = 0.28;

/// When the travel ends and the cart is against the contacts, which is what it clicks on.
pub const SEATED_AT: f32 = INSERT_S - INSERT_HOLD_S;

/// The eject is the insert played backwards, so it is the same length. It used to be shorter,
/// on the grounds that pulling something out is a quicker movement than pushing it in, but
/// every part of the screen is driven off one progress and two lengths make every one of them
/// come back faster than it left.
pub const EJECT_S: f32 = SEATED_AT;

/// Between the picture going out and the cart starting to move. Long enough for the game to
/// have actually stopped: the core is paused the moment the button is held, but what it has
/// already handed the device is up to a ring's worth of audio, and the cart must not start
/// coming out over the last of it.
const EJECT_HOLD_S: f32 = 0.35;

/// The panel striking, once the cart is home. Long enough to read as a screen coming up,
/// short enough that it is not something to sit through.
const POWER_ON_S: f32 = 0.22;

/// Going out is quicker than coming up, the way a panel dies faster than it strikes.
const POWER_OFF_S: f32 = 0.16;

/// Volume has ten times the range of the other two, so it moves ten times as far. Twenty
/// presses end to end is close enough to their ten that the three feel like one control.
const VOLUME_STEP: u8 = 5;

/// Crash insurance, and the only durable write that happens with the game still running.
const AUTOSAVE_MS: Millis = 60_000;

/// A dead battery is a far likelier hard cutoff than anyone holding POWER for eight
/// seconds, so the last of the charge goes on the state and then on stopping.
const BATTERY_CRITICAL: u8 = 5;

/// The gauge moves by a percent over minutes and on the device it is a sysfs read, so it
/// is not worth a look every frame.
const BATTERY_POLL_MS: Millis = 10_000;

/// The gauge moves over minutes but the charge state is a step change: it flips the instant
/// a cable goes in. Ten seconds of a stale bolt on screen, and a stale colour on the LED, is
/// worse than the read costs — `status` is a short string, far cheaper than the pair.
const CHARGE_POLL_MS: Millis = 1_000;

/// Below this the LED goes red. Well clear of `BATTERY_CRITICAL`, since it is a warning with
/// time to act on it rather than a cutoff.
const BATTERY_LOW: u8 = 20;

/// Long enough to notice the wrong state loading, short enough that the offer is gone by the
/// time the switcher is opened for any other reason.
pub const UNDO_GRACE_MS: Millis = 30_000;

/// How long A has to be down on the shelf before it means "start this cart clean". Past the
/// point a press could be a tap, and short enough to hold without wondering whether the
/// device is still listening.
const PLAY_HOLD_MS: Millis = 500;

/// How far apart the menu's rows sit, and what marks the one in hand. The pitch clears the
/// 40 px face with a little air; the bar is drawn to the face's own width, padding included,
/// so it wraps the words rather than the panel.
/// How long the shutdown screen is on the panel before the machine is allowed to stop. Only
/// needs to outlast a couple of frames — it exists so the ordinary loop presents the screen,
/// rather than the binary rendering one out of band on a GPU that is about to go away.
const SHUTDOWN_SHOW_MS: Millis = 250;

const POWER_MENU_PITCH: f32 = 44.0;
/// How much shorter the bar is than the row it marks, top and bottom. Enough that the rows
/// stay separate things rather than one continuous block when the selection moves.
const POWER_MENU_BAR_INSET: f32 = 4.0;
/// The cheat table's row spacing and the most rows it shows before it scrolls. Smaller than the
/// power menu's pitch because codes are short and there are often many of them.
const CHEAT_PITCH: f32 = 44.0;
const CHEAT_VISIBLE: usize = 12;
/// How far the row makes way while a cart is open, as `Shelf::draw_row` counts `recede`. It is
/// set by where the neighbours stand: here they come to rest at -41 and 574, where the mockup
/// frames the open cart with them. Parted far enough for the recede alone to dim them to a
/// quarter, they left the open cart alone in the frame.
const CORE_PICKER_RECEDE: f32 = 0.26;
/// How much further the neighbours' faces darken while a cart is open, since the recede that
/// stands them in place dims them only part of the way. Just the recede leaves a side cart's
/// face at `SIDE_ALPHA * (1 - CORE_PICKER_RECEDE)`, and the mockup has the neighbours at a
/// quarter, so this is what is left to take off.
///
/// Derived from the shelf's own `SIDE_ALPHA` rather than written down as a number. It used to
/// be a literal, computed against the 0.55 `SIDE_ALPHA` of the day, and when the look was
/// re-cut to 0.7 the literal stayed put: the neighbours came to rest at a third instead of a
/// quarter, which the test for it caught.
const CORE_PICKER_DIM: f32 = 0.25 / (slot_ui::SIDE_ALPHA * (1.0 - CORE_PICKER_RECEDE));
/// The legend's line, under the open cart and clear of the case band.
const CORE_LEGEND_Y: f32 = 386.0;
/// The soft oval under the resting lid, as the mockup draws it: its size, how far below the
/// lid's bottom edge its centre falls, and how dark it is. Scaled with the lid as it lifts.
const LID_SHADOW_W: f32 = 168.0;
const LID_SHADOW_H: f32 = 18.0;
const LID_SHADOW_DROP: f32 = 29.0;
const LID_SHADOW_ALPHA: f32 = 0.8;
/// The longest the cart stands on the shelf waiting for its faces before it opens anyway, so a
/// face that never comes cannot freeze the picker. A fast scroll can leave the worker still
/// finishing the cart it was already building before it starts on this one, so the cap has to
/// cover that wait too, not just this cart's own build.
const FACES_WAIT_MS: Millis = 1500;

/// How far through a refused cart's exit the alert holds at full, and where it has finished
/// going. Fractions of that exit rather than seconds, because a cart refused early has a
/// short way to come back and the symbol has to fit inside it either way. It is gone before
/// the end: an alert still lit on the frame the shelf returns reads as a thing to dismiss.
const ALERT_HOLD: f32 = 0.45;
const ALERT_GONE: f32 = 0.9;

/// Any clock reading earlier than this was never set. An RTC that has lost power reports a
/// fault rather than a time, the kernel then starts at the epoch, and nothing that reaches
/// this frontend can legitimately be older than the frontend itself.
const CLOCK_FLOOR: i64 = 1_577_836_800;

/// The most recent undoable action. There is exactly one slot for it and a new save or load
/// replaces it: a stack of undos would be a knob.
pub enum PendingUndo {
    Save {
        stamp: String,
        /// Read out of the ring before the push that dropped it, which is what makes the
        /// undo a restore rather than a reconstruction.
        evicted: Option<(String, Vec<u8>, Vec<u8>)>,
    },
    Load {
        prior: Vec<u8>,
    },
}

/// One side of a live netpacket session. Nothing about the transport or the packets lives
/// here — only what a session being live at all means for the rest of `App`, and which of
/// libretro's two client ids this device is, for whatever the UI ends up showing while one
/// is open.
struct LinkSession {
    client_id: u16,
}

/// A link being started: the worker doing the slow parts, and which of libretro's two client
/// ids this device becomes if it succeeds. The id is decided by the row that was picked and
/// has to outlive the pick, because it is `Ready`, frames later, that needs it.
struct LinkStarting {
    starter: LinkStarter,
    client_id: u16,
}

/// The in-game menu, over a paused game rather than instead of it. `Phase::Playing` carries
/// the session; leaving it to show a menu would mean rebuilding it to come back, and "cancel
/// returns you to your game" is the entire requirement.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum GameMenu {
    /// The top level, holding whichever row is in hand.
    Rows(usize),
    /// Host or Join.
    Link(usize),
    /// The worker is running, and `LinkStep` is what the screen says while it does.
    Working(LinkStep),
    /// It did not work, and this is which one. B returns to the game.
    Failed(LinkFail),
}

/// A row of the in-game menu. One today — the menu exists for it — and an enum rather than a
/// bare index so a second row is a variant and a face rather than a set of numbers to keep
/// in agreement.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum GameRow {
    Link,
}

impl GameRow {
    /// Every row, once, in the order their faces are uploaded. Which of them a given cart
    /// actually shows is `App::game_rows`.
    pub const ALL: [GameRow; 1] = [GameRow::Link];

    /// Position in `ALL`, which is the order the faces are in.
    pub fn index(self) -> usize {
        self as usize
    }

    pub fn text(self) -> &'static str {
        match self {
            GameRow::Link => lang::LINK_ROW,
        }
    }
}

/// Which end of a link this device is offering to be. The player picks; there is no
/// discovery on this network and nothing to negotiate it with.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum LinkRow {
    Host,
    Join,
}

impl LinkRow {
    /// Host first: it is the end that has to exist before the other one can arrive.
    pub const ALL: [LinkRow; 2] = [LinkRow::Host, LinkRow::Join];

    pub fn index(self) -> usize {
        self as usize
    }

    pub fn text(self) -> &'static str {
        match self {
            LinkRow::Host => lang::LINK_HOST,
            LinkRow::Join => lang::LINK_JOIN,
        }
    }

    /// What the radio is asked to bring up: an access point, or an association to one.
    pub fn role(self) -> LinkRole {
        match self {
            LinkRow::Host => LinkRole::Host,
            LinkRow::Join => LinkRole::Join,
        }
    }

    /// libretro's own client id, not ours — 0 the host and 1 the joiner, the only two this
    /// product has. The two devices must never both think they are the same one, which is
    /// exactly what the role they picked decides.
    pub fn client_id(self) -> u16 {
        match self {
            LinkRow::Host => 0,
            LinkRow::Join => 1,
        }
    }
}

#[derive(Debug)]
pub enum Phase {
    /// Slot's own first launch, ahead of the shelf and ahead of a seated cart. Three things
    /// run off the wall clock and a cartridge RTC is the one that breaks silently.
    SetClock {
        picker: ClockPicker,
    },
    Shelf,
    Inserting {
        cart: String,
        t: f32,
        core_ready: bool,
        /// A cart already in the slot at boot. It is drawn seated from the first frame and
        /// the shelf is never drawn behind it, because a resume is not a movement the user
        /// made and there is nothing for the cart to have come from.
        resumed: bool,
        /// Start the cart from the beginning, ignoring whatever `resume.state` holds. The
        /// state is skipped rather than deleted, so a later tap still resumes it.
        clean: bool,
    },
    Playing {
        cart: String,
    },
    Ejecting {
        cart: String,
        t: f32,
    },
    Polaroids {
        cart: String,
    },
    /// The label, and the shortcut card hanging under it. A screen of its own rather than a
    /// panel, because it is one object being looked at and there is nothing else on it — but
    /// the object grew a list, so it carries how far down it has been scrolled and how far down
    /// the last press asked it to go.
    About {
        scroll: f32,
        want: f32,
    },
    Doze {
        cart: Option<String>,
    },
}

/// Where the shelf's line of type sits: centred in the gap between the row's own foot and
/// the top of the case. Derived rather than picked, so moving either end — a taller cart, a
/// deeper bay — carries it along instead of leaving it behind.

/// How many carts fall under each slot of the letter ring, in `letters::SLOTS` order.
///
/// One pass at boot and never again: the card cannot change while slot is running. The strip
/// reads it to know which letters are worth stopping on and which are drawn dim, so it has to
/// be counted from the same list the row is drawn from rather than from anything on disk.
fn tally_letters(carts: &[Cart]) -> [usize; letters::N] {
    let mut counts = [0usize; letters::N];
    for cart in carts {
        counts[letters::slot_of(cart.initial)] += 1;
    }
    counts
}

/// Where the about card starts on the panel, and the gap between the label and the rows under
/// it. The label used to be centred because it was the whole screen; now that the card runs
/// past the bottom of the panel, it belongs at the top of what there is.
const ABOUT_PAD: f32 = 26.0;
const ABOUT_GAP: f32 = 20.0;

/// The first row's resting place: below the label and one gap past it. Derived from the two
/// numbers above rather than written down, so moving the label carries the rows with it.
fn about_first_y() -> f32 {
    ABOUT_PAD + STICKER_H as f32 + ABOUT_GAP
}

/// The bar the pinned foot line is set on: the panel's full width, opaque, flush with the
/// bottom edge. Drawn here rather than baked into the hint's own face, because it belongs to
/// the screen and not to the card — the card is 660 across, this is all 720.
const HINT_BAR_INK: [f32; 4] = [0.055, 0.05, 0.045, 1.0];
/// The bar's lit top edge: a few hairlines fading upward, which is as close as a list of flat
/// rects comes to a glow. Faint on purpose — it is a rim light, not a light.
const HINT_GLOW_STEPS: u32 = 8;
const HINT_GLOW_H: f32 = 1.5;
const HINT_GLOW_A: f32 = 0.13;
const HINT_GLOW_INK: [f32; 3] = [0.86, 0.83, 0.77];

/// How far one press takes the card. Six rows: less than that is a card the user has to walk to
/// the end of, and a whole screen is a card that jumps past the line it was being read at.
const ABOUT_PAGE: f32 = 180.0;

/// How fast the card closes on where the last press asked it to go, as a share of the way there
/// per second. Slower than the letter strip's spring on purpose: this is text being moved, and
/// there is nothing here that has to land on anything.
const ABOUT_EASE: f32 = 9.0;

/// The footer band and the faint light along its top edge. Opaque and the panel's full width,
/// so the rows scrolling under it pass behind it rather than through it. `top` is where the
/// band starts; the glow is laid above it, brightest against the edge.
fn draw_hint_bar(top: f32, out: &mut Vec<Draw>) {
    out.push(Draw::Rect {
        x: 0.0,
        y: top,
        w: OUT_W as f32,
        h: (OUT_H as f32 - top).max(0.0),
        colour: HINT_BAR_INK,
    });
    for i in 0..HINT_GLOW_STEPS {
        // Brightest next to the edge, dimming as it climbs away from it.
        let a = HINT_GLOW_A * (i as f32 + 1.0) / HINT_GLOW_STEPS as f32;
        let y = top - (HINT_GLOW_STEPS - i) as f32 * HINT_GLOW_H;
        out.push(Draw::Rect {
            x: 0.0,
            y,
            w: OUT_W as f32,
            h: HINT_GLOW_H,
            colour: [HINT_GLOW_INK[0], HINT_GLOW_INK[1], HINT_GLOW_INK[2], a],
        });
    }
}

/// The on-screen picture's two independent knobs.
///
/// `mask_mode` picks the panel mask: 0 = OFF (no aperture), 1 = LCD3X at 50% (the shipped LCD3x
/// grid softened to half strength), 2 = LCD3X at 100% (the full look), 3 = SCANLINE at 50%
/// (horizontal scanline overlay softened to half strength), 4 = SCANLINE at 100% (the full
/// scanline look). `cc_mode` picks the colour correction: 0 = FULLCOLOR (identity, 100%
/// saturation), 1 = HALFCOLOR (picture at 50% saturation — colours dulled halfway to grey),
/// 2 = NOCOLOR (grayscale / 0% saturation — black & white), and 3..=6 are four tinted-backlight
/// grayscale palettes (luma kept, hue replaced by a coloured LCD backlight): 3 = DMG green
/// (Game Boy dot-matrix green), 4 = ice-blue backlight, 5 = amber-orange backlight, 6 = pink
/// backlight. Each is cycled on its own chord — SELECT+X for the mask, SELECT+Y for the colour —
/// and the pair is persisted to `System/display.txt` as two integers "mask_mode cc_mode".
///
/// 半彩与四档单色背光在**线性空间**里做（`CC_GAMMA`）：直接在编码空间乘会把半彩压暗、
/// 把单色背光冲淡；转线性、乘完再转回，观感才对（RetroArch 手持着色器同法）。
/// FULLCOLOR 与 NOCOLOR 仍走 gamma 1.0，行为与旧版一致。
struct DisplayFilter {
    /// The card's LCD3x table if it ships one, else the built-in. Only used when `mask_mode` is 1 or 2.
    lcd3x: [[[u8; 3]; 3]; 3],
    /// The card's colour matrix (from `cc.txt`) if it ships one, else the NOCOLOR (0% saturation)
    /// default. Only used when `cc_mode` is 2. The matrix the game pass multiplies the picture by,
    /// in the spirit of a colour-saturation shader but applied as a single 3x3 multiply so it
    /// costs nothing here.
    nocolor_cc: [[f32; 3]; 3],
    mask_mode: u8,
    /// One colour-correction choice per machine, in `MACHINE_SLOTS` order (GBA, GB, GBC).
    ///
    /// Per machine because the three machines want different *kinds* of correction and there is
    /// no sensible shared index into them. A Game Boy is a greyscale machine: saturation is
    /// meaningless on it, and what its four shades are tinted with is the whole question — DMG
    /// green, Pocket grey, Light cyan. A Game Boy Color and an Advance are colour machines,
    /// where saturation and a backlight tint both mean something. One number shared between them
    /// would have to mean "entry 3 of whichever list", which is exactly the kind of thing that
    /// reads fine until someone cycles it on the wrong shelf.
    ///
    /// `u16` rather than `u8` because the Game Boy's slot is an index into the whole palette
    /// table, which is past 255 entries and will only grow. The two colour machines' slots still
    /// hold 0..=6; the width is shared because the three are read as one array.
    cc: [u16; 3],
}

/// How many machines have their own shelf and their own colour correction. The three the
/// device ships with; `machine_slot` is the mapping and it is the only one.
const MACHINE_SLOTS: usize = 3;
impl DisplayFilter {
    const IDENTITY: [[f32; 3]; 3] = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];
    // NOCOLOR grayscale (0% saturation): plain luma. The whole picture collapses to its luma
    // (Rec.601 weights 0.299/0.587/0.114), so colours keep their brightness but lose hue
    // entirely — the "black & white / 灰度" look. This is the mode-2 (NOCOLOR) matrix.
    // Overridable per card through
    // System/cc.txt; this is the fallback. Uploaded row-major -> column-major by
    // `Fbo::set_color_correction`, so this multiplies as written.
    const DEFAULT_CC: [[f32; 3]; 3] = [
        [0.299, 0.587, 0.114],
        [0.299, 0.587, 0.114],
        [0.299, 0.587, 0.114],
    ];
    /// HALFCOLOR (50% saturation): the original picture at half saturation. Each row is a
    /// luminance blend at s = 0.5 (out_i = 0.5*luma + 0.5*in_i), so the colours come through
    /// but dulled halfway toward grey — mode 1 (HALFCOLOR). Hardcoded; tune here (or in cc.txt
    /// for the NOCOLOR row) if you want more or less saturation.
    const HALF_CC: [[f32; 3]; 3] = [
        [0.650, 0.294, 0.057],
        [0.149, 0.794, 0.057],
        [0.149, 0.294, 0.557],
    ];
    /// Four tinted-backlight grayscale palettes. Each keeps the picture's luma (Rec.601
    /// 0.299/0.587/0.114) but recolours it in the hue of a coloured LCD backlight, by making
    /// every output channel a scaled copy of the luma: `out_c = (tint_c/255) * luma`. The tint
    /// colours are lifted from the light palettes of the pixel reader we built before
    /// (`PXReader/reader.c` light group): GB (DMG green) / LCDB (ice-blue) / SEPIA (amber) /
    /// PINKBG (pink). Modes 3..=6.
    /// 线性空间里做色彩校正所用的 gamma（对应 `GAME_FRAG` 的 `u_cc_gamma`）。
    /// 2.2 = sRGB。只有「半彩」和四档单色背光用它；FULLCOLOR(0)/NOCOLOR(2) 传 1.0，
    /// 仍是编码空间直乘，与旧版逐位一致（NOCOLOR 的 cc.txt 卡内覆盖语义也不变）。
    pub const CC_GAMMA: f32 = 2.2;
    /// 四档单色背光：把画面亮度重新映射成一块**彩色背光**。
    /// 因为是在线性空间里做（见 `CC_GAMMA`），行系数 = 「目标峰值色的线性值 × 亮度权重
    /// (0.299/0.587/0.114)」——这样画面最亮处正好落在目标色上，中间调被 gamma 拉出层次，
    /// 于是观感浓郁；先前在编码空间直乘，中间调被冲成灰调，四档都发淡。
    /// 峰值色：DMG 绿 rgb(155,188,15) / 冰蓝 rgb(120,170,215) / 琥珀 rgb(240,165,60) / 粉 rgb(240,130,185)。
    const DMG_GREEN_CC: [[f32; 3]; 3] = [
        [0.1000, 0.1963, 0.0381],
        [0.1529, 0.3002, 0.0583],
        [0.0006, 0.0012, 0.0002],
    ];
    const ICE_BLUE_CC: [[f32; 3]; 3] = [
        [0.0569, 0.1118, 0.0217],
        [0.1225, 0.2406, 0.0467],
        [0.2054, 0.4033, 0.0783],
    ];
    const AMBER_CC: [[f32; 3]; 3] = [
        [0.2617, 0.5137, 0.0998],
        [0.1147, 0.2253, 0.0438],
        [0.0124, 0.0243, 0.0047],
    ];
    const PINK_CC: [[f32; 3]; 3] = [
        [0.2617, 0.5137, 0.0998],
        [0.0679, 0.1333, 0.0259],
        [0.1476, 0.2898, 0.0563],
    ];
    /// Which shelf a machine's colour choice lives on.
    const fn machine_slot(s: System) -> usize {
        match s {
            System::Gba => 0,
            System::Gb => 1,
            System::Gbc => 2,
        }
    }

    /// How many colour-correction entries the colour machines have — the seven above.
    pub const CC_COUNT: u16 = 7;
    /// How many the Game Boy has: the whole palette table, which is every PixelShift and TWB64
    /// entry the core ships plus the two house looks in front of them. See `crate::palettes`.
    pub const GB_CC_COUNT: u16 = crate::palettes::GB_PALETTES.len() as u16;

    /// The Game Boy's palettes — the whole table, as `(name, four shades)` — live in
    /// `crate::palettes`, generated from gambatte's own `gbcpalettes.h`.
    ///
    /// The table is four shades rather than a matrix, because four shades is what the machine
    /// has. A 3x3 multiply can only slope between one white point and one black point: it can
    /// tint, and it cannot follow a palette whose middles sit off that line. `Sunburst` runs pale
    /// green -> yellow -> orange -> red, and a two-point map draws a straight line from one end to
    /// the other and loses both of the middle colours. `gb_lut` turns a table like this into a
    /// per-shade map instead, and the shader looks it up.
    ///
    /// Entry 0 原生灰 is the core's own output, untouched — and it is deliberately *not* routed
    /// through the lookup, so the machine's neutral look costs nothing.

    /// The three Game Boy models — the screen art **and** the four shades behind it — live in
    /// `crate::palettes::GB_MODELS`.
    ///
    /// They are not entries in the palette table, and that is the whole of what "bound" means:
    /// each model's four shades were drawn as the other half of its screen art, so the choice is
    /// made once, by choosing the art. Letting them in as palettes as well would be two ways to
    /// land on one look.
    ///
    /// How many of a Game Boy's overlay choices are models: they come first, so a card that ships
    /// the three masks opens on the DMG.
    pub const GB_MODEL_COUNT: usize = 3;

    /// How much of the lattice colour is mixed into the dots: half.
    ///
    /// The geometry is the shader's — the right column and bottom row of every 3x3 source pixel,
    /// 5 of its 9 — and at that coverage the mesh is much heavier than the old aperture table
    /// ever was, so this is the one number that decides whether the picture survives it. Tried
    /// at 35% and set back to 50% on the device; it is a taste dial and nothing else depends
    /// on the value.
    pub const GB_GRID_MIX: f32 = 0.5;

    /// The palette's own name is not a second array beside the table: each entry of
    /// `crate::palettes::GB_PALETTES` is `(name, four shades)`, so a name cannot drift from the
    /// colours it names. `display_palette_name` is the accessor.

    /// Where the four shades sit in the greys the core hands over. Not evenly spaced: mGBA's
    /// greyscale is its own four levels (RGB555 `0x7FFF / 0x56B5 / 0x294A / 0x0000`, which is
    /// 255 / 173 / 82 / 0 once widened), and the lookup has to put each shade's colour *on* its
    /// level rather than on an idealised third of the range.
    const GB_SHADE_LEVELS: [u8; 4] = [255, 173, 82, 0];

    /// Turn four shades into a 256-entry RGBA lookup, so the shader can be exact.
    ///
    /// Every grey the machine can hand over gets a colour, interpolated between the two shades
    /// either side of it. Interpolating matters even though a Game Boy only draws four levels:
    /// the reflection pass feeds this a *blurred* frame, which is continuous, and a lookup that
    /// only knew about four values would posterise the glow around the screen.
    ///
    /// Built in encoded space, the space the panels are described in, so a shade lands on its
    /// own colour exactly rather than a gamma away from it.
    ///
    /// Takes the shades themselves rather than an index into the table: the table's entries and
    /// the three models' four shades are the same kind of thing here, and only one of the two has
    /// an index at all.
    fn gb_lut(pal: [[u8; 3]; 4]) -> [u8; 1024] {
        let lv = Self::GB_SHADE_LEVELS;
        let mut lut = [0u8; 1024];
        for g in 0..256u32 {
            // Find the pair of shades this grey falls between. `lv` runs bright to dark and `g`
            // runs dark to bright, so the search walks the other way.
            let mut lo = 0;
            while lo < 3 && (g as u8) < lv[lo + 1] {
                lo += 1;
            }
            // Between lv[lo] (brighter) and lv[lo + 1] (darker).
            let (hi_lv, lo_lv) = (lv[lo] as u32, lv[(lo + 1).min(3)] as u32);
            let t = if hi_lv == lo_lv {
                0
            } else {
                ((hi_lv - g) * 255 / (hi_lv - lo_lv)).min(255)
            };
            let a = pal[lo];
            let b = pal[(lo + 1).min(3)];
            let i = (g * 4) as usize;
            for c in 0..3 {
                let (x, y) = (a[c] as u32, b[c] as u32);
                lut[i + c] = ((x * (255 - t) + y * t + 127) / 255) as u8;
            }
            lut[i + 3] = 255;
        }
        lut
    }

    /// The neutral aperture the LCD3x modes sit between: an everywhere-white table, so OFF is a
    /// clean framebuffer and the two LCD3x steps are the grid at half and full strength.
    const FLAT_MASK: [[[u8; 3]; 3]; 3] = [[[255u8; 3]; 3]; 3];
    /// A horizontal scanline overlay: the top row of each 3-line cell is the dark gap, the other
    /// two are lit. Used at full strength for mode 4; mode 3 is this lerped halfway from FLAT.
    const SCANLINE_MASK: [[[u8; 3]; 3]; 3] = [[[0u8; 3]; 3], [[255u8; 3]; 3], [[255u8; 3]; 3]];
    fn new(
        lcd3x: [[[u8; 3]; 3]; 3],
        nocolor_cc: [[f32; 3]; 3],
        mask_mode: u8,
        cc: [u16; 3],
    ) -> Self {
        Self {
            lcd3x,
            nocolor_cc,
            mask_mode: mask_mode.min(4),
            cc: [
                cc[0].min(Self::CC_COUNT - 1),
                cc[1].min(Self::GB_CC_COUNT - 1),
                cc[2].min(Self::CC_COUNT - 1),
            ],
        }
    }
    /// Mask rides on top of the picture: 0 is clear, 1 the LCD3x grid at 50%, 2 at full strength,
    /// 3 the scanline overlay at 50%, 4 at full strength.
    fn applied_mask(&self, flat: bool) -> [[[u8; 3]; 3]; 3] {
        // A Game Boy's lattice is not a table any more: it is drawn from the palette in the game
        // shader (see `GB_GRID_MIX`), which is what makes it follow the four shades. The aperture
        // here is therefore neutral, and the mesh and the picture are made of the same colours.
        if flat {
            return Self::FLAT_MASK;
        }
        match self.mask_mode {
            0 => Self::FLAT_MASK,
            1 => lerp_mask(&Self::FLAT_MASK, &self.lcd3x, 0.5),
            2 => self.lcd3x,
            3 => lerp_mask(&Self::FLAT_MASK, &Self::SCANLINE_MASK, 0.5),
            _ => Self::SCANLINE_MASK,
        }
    }
    /// Colour correction rides under the mask, and what it *is* depends on the machine.
    ///
    /// A Game Boy gets a palette: see `gb_cc`, whose black point is the reason this returns a
    /// bias at all. The colour machines keep the list they had — 0 identity, 1 the HALFCOLOR
    /// grade, 2 the NOCOLOR luma, 3 DMG green backlight, 4 ice-blue, 5 amber, 6 pink — all of
    /// them anchored on black, so their bias is the zero vector and their picture is bit for bit
    /// what it was before this pair existed.
    fn applied_cc(&self, machine: System) -> ([[f32; 3]; 3], [f32; 3]) {
        let mode = self.cc[Self::machine_slot(machine)];
        if machine == System::Gb {
            // A Game Boy is drawn by its palette lookup, not by a matrix: see `gb_lut`. The
            // matrix path stays identity so nothing double-applies when the lookup is on.
            return (Self::IDENTITY, [0.0, 0.0, 0.0]);
        }
        let m = match mode {
            0 => Self::IDENTITY,
            1 => Self::HALF_CC,
            2 => self.nocolor_cc,
            3 => Self::DMG_GREEN_CC,
            4 => Self::ICE_BLUE_CC,
            5 => Self::AMBER_CC,
            6 => Self::PINK_CC,
            _ => Self::PINK_CC,
        };
        (m, [0.0, 0.0, 0.0])
    }
    /// The signature colour of a tinted-backlight mode (3..=6) as sRGB 0..1 — the colour the
    /// matrix drives the picture's white to, which is also the tone the four modes are named
    /// after (see the peak colours on `DMG_GREEN_CC` and friends). Used to fill the letterbox
    /// around a GB screen so the whole panel reads as one tinted surface. `None` for the modes
    /// with no single colour — FULLCOLOR, HALFCOLOR, NOCOLOR — which keep the black surround.
    fn cc_border(&self, machine: System) -> Option<[f32; 3]> {
        // A Game Boy palette leaves the surround black. The tempting move is the palette's own
        // lightest shade, and for DMG green that is right — it is how the tinted-backlight modes
        // above behave. For the Pocket it is near-white and for the Light it is a bright cyan,
        // and a wall of that around a 160x144 picture at night is not what either machine looked
        // like: what surrounded a Pocket's LCD was its own grey case, in shadow. Black is the
        // honest answer for all four, and the picture inside is the palette.
        if machine == System::Gb {
            return None;
        }
        let c = match self.cc[Self::machine_slot(machine)] {
            3 => [155.0, 188.0, 15.0],
            4 => [120.0, 170.0, 215.0],
            5 => [240.0, 165.0, 60.0],
            6 => [240.0, 130.0, 185.0],
            _ => return None,
        };
        Some([c[0] / 255.0, c[1] / 255.0, c[2] / 255.0])
    }
    fn cycle_mask(&mut self) {
        self.mask_mode = (self.mask_mode + 1) % 5;
    }
    /// Step the colour correction of one of the **colour** machines, wrapping at the end of its
    /// list of seven.
    ///
    /// A Game Boy never comes through here: its palettes are a table hundreds of entries long
    /// walked a page at a time, and which page that is belongs to `App`, not to the table of
    /// seven this struct also holds. `App::cycle_cc` is the fork.
    fn cycle_cc(&mut self, machine: System) {
        let slot = Self::machine_slot(machine);
        self.cc[slot] = (self.cc[slot] + 1) % Self::CC_COUNT;
    }
}

/// Linearly blend two 3x3 aperture tables, element by element, at `t` in [0,1]. Used to soften
/// the LCD3x grid to a 50% strength for the middle mask step.
fn lerp_mask(a: &[[[u8; 3]; 3]; 3], b: &[[[u8; 3]; 3]; 3], t: f32) -> [[[u8; 3]; 3]; 3] {
    let mut out = [[[0u8; 3]; 3]; 3];
    for i in 0..3 {
        for j in 0..3 {
            for k in 0..3 {
                let av = a[i][j][k] as f32;
                let bv = b[i][j][k] as f32;
                out[i][j][k] = (av + (bv - av) * t).round().clamp(0.0, 255.0) as u8;
            }
        }
    }
    out
}

/// One cheat code for the seated cart, plus whether it is live. `enabled` is per-code; the
/// master `cheats_on` in `App` gates the lot — a code switched on but under a master that is
/// off does nothing, exactly as a global toggle over a picker of switches should.
pub(crate) struct CheatItem {
    pub(crate) code: String,
    pub(crate) desc: String,
    pub(crate) enabled: bool,
}

/// Inset of the shelf's star indicator from the corner of the case. Clear of the cart bay,
/// which owns the true bottom edge of the screen.

/// The machine logo in the bottom-left corner — the mirror of the favourites star in the
/// bottom-right — and the size it is drawn at. Only ever one logo: it *is* the shelf, and it
/// swaps with it.
const SHELF_IND_PX: f32 = 22.0;
const SHELF_IND_MARGIN: f32 = 12.0;
/// The bottom bar's type badge: what a card's own art is fitted into, and the size the built-in
/// word is drawn at. The card's art is 150 x 50, so half of it lands in this box exactly.
const SHELF_IND_W: f32 = 75.0;
const SHELF_IND_H: f32 = 25.0;
/// How far in from the panel's left edge the badge sits. Flush was too loud for a mark this
/// small: it read as a cut-off rather than as a badge.
const SHELF_IND_LEFT: f32 = 20.0;
/// How tall the bottom-right favourites indicator is drawn. Enlarged on request, to match the
/// mark a favourite cart wears.
const FAV_IND_DRAW: f32 = 36.0;
/// The top band's own margin: the distance the clock and the battery reading are printed in from
/// their edges. Repeated here so the two bottom corner marks can stand in the same columns as the
/// two readings above them.
const STATUS_READ_MARGIN: f32 = 24.0;

/// The shell that brings WiFi up and starts the SSH server, on demand rather than at boot: the
/// radio is a real drain on a standby budget, so it is the user who asks for it. `ags-net ssh`
/// loads `8821cs`, raises `wlan0`, joins from `System/wifi.conf` and starts `dropbear`; the `udhcpc`
/// after it is the address the firmware's own helper sometimes misses. The address is left in
/// `/tmp/slot-ip` for the dialog to show, since DHCP is what decides it.
const DIRECT_ENTER: &str = "sync; export PATH=/usr/sbin:/usr/bin:/sbin:/bin:$PATH; \
/usr/sbin/ags-net ssh >/dev/null 2>&1; udhcpc -i wlan0 -n -q -t 10 >/dev/null 2>&1; \
ip -4 -o addr show wlan0 2>/dev/null | tr -s ' ' | cut -d' ' -f4 | cut -d/ -f1 > /tmp/slot-ip; sync";

/// The shell that takes the radio back down: the SSH server, the supplicant and the DHCP client
/// are killed, `wlan0` dropped and the driver unloaded, which is what makes the drain go away
/// again. The card is never touched — the whole point of doing this over the network.
const DIRECT_EXIT: &str = "sync; export PATH=/usr/sbin:/usr/bin:/sbin:/bin:$PATH; \
pkill dropbear 2>/dev/null; pkill udhcpc 2>/dev/null; \
wpa_cli -i wlan0 terminate 2>/dev/null; pkill wpa_supplicant 2>/dev/null; \
ip link set wlan0 down 2>/dev/null; rmmod 8821cs 2>/dev/null; rm -f /tmp/slot-ip; sync";

/// The two beats of a system-shelf swap. The old shelf is drawn up and away over the first,
/// the new one drops back into place over the second, and the app takes no input for their sum.
/// Short on purpose: this is a shelf change, not a cutscene.
const SWITCH_ASCEND_S: f32 = 0.34;
const SWITCH_DESCEND_S: f32 = 0.34;

/// A system-shelf swap in flight: which shelf is arriving, how far through the two beats it is,
/// and whether the view has been swapped yet. While `App::shelf_switch` holds one, the app
/// takes no input and the row is drawn with the swap motion — see `slot_ui::Motion`.
struct ShelfSwitch {
    /// The system whose shelf is leaving. Held for the length of the swap so the corner logo can
    /// cross-dissolve from it rather than cut.
    from: System,
    /// The system whose shelf comes up when the ascend beat ends.
    to: System,
    /// Seconds since the swap began, across both beats.
    t: f32,
    /// Whether the view has been swapped to `to`. The row is the old shelf before it, the new
    /// one after.
    swapped: bool,
}

pub struct App {
    phase: Phase,
    shelf: Shelf,
    /// The letter strip over the shelf: where the ring stands, and how far it still has to
    /// travel.
    letters: Letters,
    /// How many carts sit under each of the ring's slots, in `letters::SLOTS` order. Built
    /// once from the scan and never again — the card does not change while slot is running.
    /// The strip needs it to know which letters are worth stopping on, and which are drawn dim.
    letter_counts: [usize; letters::N],
    /// One small face per slot, in that order with the size each was rasterised at. Uploaded at
    /// boot: 27 letters of a fixed alphabet is less work than one cart face.
    ///
    /// The band the letters stand in is not here at all: it is the machine's own, and
    /// `slot_chrome::draw_top_band` draws it from the cart bay's numbers every frame.
    letter_faces: Vec<(TexId, u32, u32)>,
    /// The one piece of metal the strip repeats between two letters, with its own size.
    /// Uploaded beside the letters because it is the same kind of thing: fixed, tiny, and
    /// needed on the shelf from the first frame.
    letter_ridge_face: Option<(TexId, u32, u32)>,
    /// When A went down on the shelf, and `None` the rest of the time. The hold lives here
    /// rather than in the gesture layer because A is the GBA's A button everywhere else, and
    /// `Gestures` is deliberately blind to which screen is up.
    play_held: Option<Millis>,
    /// When X went down on the shelf, and `None` the rest of the time — the same shape as
    /// `play_held`, and for the same reason. X carries two shelf gestures, a short press that
    /// stars the cart and a hold that swaps the shelf, and which one a press is cannot be known
    /// until it either comes up or passes `PLAY_HOLD_MS`.
    fav_held: Option<Millis>,
    /// When Y went down on the shelf: a hold cycles which machine's shelf is up. Same shape as
    /// `fav_held`, and spent the same way.
    sys_held: Option<Millis>,
    /// When B went down on the shelf: a hold opens the direct-connect dialog. A tap does nothing.
    mtp_held: Option<Millis>,
    /// The mark a favourite cart wears, with the size it was rasterised at. One colour whatever
    /// shelf is up. Its own upload rather than one of the HUD's, because it is the one glyph on the
    /// shelf that is a colour of its own instead of the case's ink.
    fav_star: Option<(TexId, u32, u32)>,
    /// The shelf's indicator, unlit and lit: the same star hollow while the ordinary shelf is
    /// up and solid while the favourites are.
    fav_ind: Option<((TexId, u32, u32), (TexId, u32, u32))>,
    /// The last refused action, and the only thing that tells an eject apart from a cart
    /// that would not seat: both leave down the same path.
    refusal: Option<Refusal>,
    /// The `t` a refused cart's exit started from, which is also how long there is left to
    /// say so. `None` for an eject the user asked for: nothing was refused.
    refused_from: Option<f32>,
    alert_face: Option<TexId>,
    /// The game under the eye on the shelf, rasterised by the frontend whenever the selection
    /// moves — one line under the row naming what the row is showing. The cart carries a
    /// printed label of its own, but that label is a 240 px cart's: at the distance a handheld
    /// is held it says which of three carts is which, not which game this is, and a long title
    /// on it is three small lines rather than a name.
    shelf_title_face: Option<(TexId, u32, u32)>,
    /// What the shutdown says, one per `PowerChoice::ALL` in that order and rastered at the
    /// menu's own size. "Powering down" under a restart was the screen contradicting the row
    /// the user had just chosen.
    shutdown_faces: Vec<(TexId, u32, u32)>,
    /// Open when a held POWER raised the menu, holding the highlighted row. An overlay
    /// rather than a phase, so cancelling returns to whatever was underneath without the
    /// phase having to be remembered anywhere.
    power_menu: Option<usize>,
    /// One per `PowerChoice::ALL`, in that order, with the size each was rastered at.
    power_menu_faces: Vec<(TexId, u32, u32)>,
    /// The direct-connect (USB mass-storage) dialog: which stage it is showing, or `None` when it
    /// is not up. `0` is the confirm prompt, `1` the "connect the cable" beat, `2` the live mode.
    direct_link: Option<u8>,
    /// The dialog's lines, baked once at boot: [confirm title, confirm keys, connecting, active
    /// title, active hint].
    direct_faces: Vec<(TexId, u32, u32)>,
    /// When the "connect the cable" beat went up, so it can hand over to the live screen on its own.
    direct_at: Millis,
    /// The address the WiFi link came up on, once the connect script has left one.
    direct_ip: Option<String>,
    /// The baked face for that address, and whether the address has changed since it was baked.
    direct_ip_face: Option<(TexId, u32, u32)>,
    direct_ip_dirty: bool,
    /// The picker while the cart is open, and while its lid is going back on. The cart it acts
    /// on is whichever the shelf has, read when it opens rather than held here: the shelf cannot
    /// move while it is up, so there is only ever one answer.
    core_picker: Option<CorePicker>,
    /// The open cart under the highlight, and its lid: the shelf face with a transparent border
    /// so it can be turned. Rebuilt by the frontend when the highlighted cart changes.
    core_board_face: Option<TexId>,
    core_lid_face: Option<TexId>,
    /// The cart the uploaded board and lid were built for.
    core_faces_stem: Option<String>,
    /// In `Core::ALL` order: each socket empty, and the chip seated and named in each. Uploaded
    /// at boot, since none of them ever changes.
    core_socket_faces: Vec<TexId>,
    core_chip_faces: Vec<TexId>,
    /// The chip in flight, blank, and the shadow under it.
    core_blank_chip_face: Option<TexId>,
    core_chip_shadow_face: Option<TexId>,
    /// `B` Cancel, the two arrows, `A` Choose, each with the width it was rastered at, laid out
    /// by role in that order: Cancel under the cart's left edge, Swap on the panel's centre,
    /// Choose under its right edge.
    core_legend_faces: Vec<(TexId, u32)>,
    /// Open when SELECT+MENU raised the in-game menu over a running game. An overlay rather
    /// than a phase, and for a stronger reason than the power menu's: `Phase::Playing` is
    /// what holds the seated cart, and a menu that left it would have to rebuild the session
    /// to come back from cancelling.
    game_menu: Option<GameMenu>,
    /// One per `GameRow::ALL`, in that order. Every row is uploaded whether or not this
    /// cart shows it — the faces never change, and which rows exist does.
    game_menu_faces: Vec<(TexId, u32, u32)>,
    /// One per `LinkRow::ALL`, in that order.
    link_menu_faces: Vec<(TexId, u32, u32)>,
    /// One per `LinkStep::ALL`, and one per `LinkFail::SHOWN`, in those orders. A sentence
    /// each rather than a list, so nothing is ever in hand on either.
    link_step_faces: Vec<(TexId, u32, u32)>,
    link_fail_faces: Vec<(TexId, u32, u32)>,
    /// The worker behind `GameMenu::Working`, and `None` the rest of the time. It has no
    /// `Drop` of its own, so `close_game_menu` is what stops it: see there.
    starting: Option<LinkStarting>,
    /// The wire a finished starter handed over, waiting for whoever owns the emulator thread
    /// to collect it. `App` holds a session's own bookkeeping and never a transport (see
    /// `link`), and this is the one hop between the two — `Session::update` drains it into
    /// `EmuHandle::begin_link`, mirroring the hop `Session::bridge_link` already makes for
    /// an ending.
    link_transport: Option<(u16, Box<dyn LinkChannel>)>,
    /// Set when the menu's Restart is chosen. The binary acts on it, like `powering_off`.
    restarting: bool,
    /// When the binary is allowed to act. The screen is drawn from the instant the choice is
    /// made, but the shutdown itself waits a few frames so the ordinary loop has drawn and
    /// presented it. Rendering out of band instead — one extra draw and swap between the
    /// choice and `poweroff` — hung the device: the swap can block on a GPU about to be torn
    /// down, and slot then never reached `poweroff` at all, leaving a machine that needed
    /// the PMIC held to recover.
    act_at: Millis,
    /// `None` outside the binary, where there is no content root and nothing persists.
    root: Option<PathBuf>,
    state: SlotState,
    /// The volume and the silence as they stood before each of the last two volume presses,
    /// oldest first. The mute chord is delivered behind the two presses that make it, so
    /// toggling has to give back what they already moved.
    vol_before: Vec<(u8, bool, Millis)>,
    /// `None` until a cart is in the slot. There is nothing to flush without a core.
    snapshot: Option<Box<dyn Snapshot>>,
    /// The seated cart's `Core`, resolved once by whoever spawned `snapshot` and handed here
    /// through `set_core` rather than re-read. `ring`, `flush_resume` and the eject path all
    /// take this instead of calling `core_for` themselves, which is what makes it structurally
    /// impossible for a later read or write to disagree with the dylib actually running: there
    /// is nowhere left in this file to derive a second opinion from. Stale between carts in
    /// exactly the way `snapshot` is — both are set together and neither is cleared on eject —
    /// which is safe because every reader of either is gated on a cart actually being seated.
    core: Core,
    /// `Some` for as long as a netpacket session is live. `App` never touches the transport
    /// or the core itself — those live on the emulator thread, wherever `EmuHandle::begin_link`
    /// was called from the same gesture this answers — this is only what the interlocks below
    /// need: that one is live at all, and which side of it this device is.
    link: Option<LinkSession>,
    /// What the slot itself is about to sound like, drained by whoever owns the device. One
    /// slot: two of these in a frame is not a movement the cart can make.
    sfx: Option<Sfx>,
    /// `Some` exactly while the switcher is showing. It holds the ring as it was when it
    /// opened, so a save behind it cannot renumber what the user is looking at.
    polaroids: Option<Polaroids>,
    /// The one undoable action and the moment it happened. Belongs to the cart in the slot,
    /// so it leaves with it.
    pending: Option<(PendingUndo, Millis)>,
    /// The key caps on the switcher's bottom plate. The three fixed ones never change what
    /// they say and are uploaded once; the undo says which action it will take back, so it is
    /// rasterised on the way into the switcher. All of them outlive any one opening.
    legend_faces: Vec<TexId>,
    undo_face: Option<TexId>,
    /// The clock screen's line of type and its one instruction. Rasterised by the binary,
    /// and gone for the rest of the session once the clock is confirmed.
    clock_faces: Option<(TexId, TexId)>,
    /// The label, rasterised whole. Re-uploaded when the gauge moves.
    sticker_face: Option<TexId>,
    /// The shortcut card's rows, in the order they are written and each with the height it was
    /// rasterised at. Uploaded at boot with the rest of the fixed furniture. The heights are as
    /// much the point as the textures: the card is exactly as tall as its rows make it.
    shortcut_rows: Vec<(TexId, u32, u32)>,
    /// The one line of the card that does not move.
    shortcut_hint: Option<(TexId, u32, u32)>,
    /// One picture from `Wallpapers`, behind everything the shelf draws. `None` on a card
    /// that carries none, which is the common case.
    wallpaper: Option<TexId>,
    /// What is printed on the case: the battery's percent, and the time as it stands.
    battery_percent: slot_ui::Printed,
    /// The charging glyph, uploaded once at boot with the other icons rather than whenever
    /// the percent changes: unlike the percent, its face never varies.
    bolt: Option<TexId>,
    shelf_clock: slot_ui::Printed,
    hud: Hud,
    /// Whether the cart's cheat list (from `System/Cheats/<stem>.txt`) is currently applied to
    /// the core. Toggled by SELECT+A in game; persists across eject/reinsert within a session.
    pub(crate) cheats_on: bool,
    /// The seated cart's cheat codes, each with its own on/off. Loaded from
    /// `System/Cheats/<stem>.txt` when its core spawns; `set_cheats` rebuilds it and shuts the
    /// table. Browsable and switchable one by one in the cheat table (SELECT+A).
    cheats: Vec<CheatItem>,
    /// `Some` while the cheat table overlay is up, holding the row in hand. `None` otherwise.
    cheat_menu: Option<usize>,
    /// One rasterised row face per cheat, in cheat order, (re)built by the frontend when the
    /// seated cart changes. The ON/OFF state is drawn live beside each, so these never change
    /// on a toggle.
    cheat_faces: Vec<(TexId, u32, u32)>,
    /// The stem whose faces `cheat_faces` currently holds, so the frontend re-rasterises only
    /// when the cart under the highlight actually changes.
    cheat_face_stem: Option<String>,
    /// The "no cheats for this cart" panel, rasterised once at boot and shown when the table
    /// opens on a cart that carries none.
    cheat_empty_face: Option<(TexId, u32, u32)>,
    /// Set when a row is toggled inside the table, so `Session` knows to re-push the list to
    /// the core after `apply` returns (the table itself cannot reach the emulator).
    cheats_dirty: bool,
    /// Set when the light/dark mode has moved under the binary's feet. Everything on this side
    /// of the palette — the case, the scrim, the gauge, the bar — follows the mode by itself,
    /// because it is drawn as filled quads. Everything *baked* does not, and this flag is how
    /// the app says so: `Frontend` takes it and re-rasterises.
    mode_dirty: bool,
    /// A machine shelf has been swapped in, so the row's geometry has moved under the cart art,
    /// the shadow and the stand-in — all three were minted for the machine the device came up
    /// on. The frontend takes this and re-cuts them, the way `mode_dirty` re-cuts the furniture.
    shelf_dirty: bool,
    /// How far up the game layer's own screen is. Not a phase: it outlives the insert, since
    /// the cart is home and the chrome is still on screen while the picture arrives.
    screen: f32,
    /// Whether the core behind the slot has published anything yet. Pushed in, because only
    /// whoever owns the emulator knows: the compositor still holds the last cart's frame.
    game_ready: bool,
    /// Accumulated from `update`, which is the only clock the app has. Milliseconds, since
    /// that is what the HUD fade is stated in.
    clock: f64,
    /// `None` in unit tests, where there is no panel to darken and no battery to run out.
    power: Option<Power>,
    dozed_at: Millis,
    /// When the state next has to be on the card. Moved by every resume write, not only by
    /// the autosave itself.
    autosave_at: Millis,
    battery_at: Millis,
    charge_at: Millis,
    /// The last full reading, with its charge half kept current by the fast tick. One
    /// snapshot rather than two values, so nothing on screen can show a percent and a bolt
    /// that never coexisted.
    battery: Option<Battery>,
    /// What the platform was last told to show. The fast tick recomputes `led_state()` every
    /// second whether or not anything moved, and `set_led` is a real write on a real device —
    /// `motor_change` two crates over exists for exactly the same reason, translating a strength
    /// asked for every frame into a write only on the edge between still and moving. This is
    /// that same edge kept here rather than behind the platform boundary: unlike the motor, the
    /// LED has no protocol-specific state of its own to translate through (`LedState` is
    /// already the discrete value the tick computes), and `App` is where the state it is
    /// computed from already lives, so every `Platform` gets the deduplication for free instead
    /// of each one having to grow its own copy of it.
    last_led: Option<LedState>,
    /// In-game display filter (panel mask + colour correction), cycled with SELECT+X.
    display: DisplayFilter,
    /// Which page of `palettes::GB_PALETTES` the palette key is walking, and which cell of that
    /// page the browser has under its caret (`None` while the browser is closed).
    ///
    /// The page outlives the browser on purpose: a hold opens the browser, the user turns to the
    /// page they want and lets go, and the tap walks *that* page. Keeping the two in one place is
    /// what makes "the page you were last looking at" true — a second copy of it on the browser
    /// would be a second answer to which fifteen the key is cycling.
    pal_page: usize,
    pal_cursor: Option<usize>,
    /// The highlighted entry's name, as the frontend rasterised it. One name is ever minted:
    /// fifteen of them would be fifteen lines of type over fifteen blocks of colour.
    pal_name: Option<(TexId, u32, u32)>,
    /// The audio latency profile, chosen on the shelf with SELECT+VOL. Read from
    /// `System/audio.txt` at boot and written back whenever it changes.
    audio: Profile,
    /// A per-system screen overlay PNG from the card — `Overlay/GB/GB.png` (plain Game Boy)
    /// or `Overlay/GBC/GBC.png` (Game Boy Color) — drawn over the live game with its own alpha
    /// channel, so the user can theme the screen (bezel, dot-matrix grid, tint) without a
    /// rebuild. `None` = the built-in look. The RGBA is decoded once per insert in
    /// `Session::spawn_core`; the GL texture is minted lazily by the compositor when
    /// `overlay_dirty` is set, and released when the next cart has none.
    overlay_pixels: Option<(Vec<u8>, u32, u32)>,
    /// The compositor's texture handle for the decoded overlay, or `None` when there is no
    /// overlay on screen. Only the compositor can mint or drop the GL resource this names.
    overlay_tex: Option<TexId>,
    /// Set when `overlay_pixels` changed and the texture must be (re)built or released on the
    /// next frame. Pushed by `set_overlay`; cleared by the frontend after `sync_overlay`.
    overlay_dirty: bool,
    /// Every overlay PNG the seated cart's system offers, in rotation order, and which one is
    /// up. Built on insert (`Session::spawn_core`) from `Overlay/`; `overlay_next` cycles it.
    /// Empty for GBA, which has no overlay set.
    overlay_list: Vec<crate::root::OverlayFile>,
    /// Which system `overlay_index` counts in: 0 for GB, 1 for GBC. Kept because a Game Boy's
    /// index counts its three models before the card's own overlays and a Game Boy Color's does
    /// not, so the same number means different things on the two shelves.
    overlay_slot: usize,
    overlay_index: usize,
    /// Which overlay each system last showed, GB then GBC, persisted to `System/overlay.txt` so
    /// the choice survives a boot. Clamped against the current list length when it is applied.
    overlay_indices: [usize; 2],
    /// Which machine's shelf is up. The row holds only this system's carts; SELECT+L2 cycles it.
    shelf_system: System,
    /// Whether the favourites sub-shelf is up within the current system shelf. The whole of what
    /// the bottom-right star reads, and what the letter strip and letter jumps stand down for —
    /// a system shelf is still a library with letters, a favourites shelf is not.
    fav_view: bool,
    /// The three machine logos drawn in the bottom-left corner, in `GB, GBC, GBA` order, each
    /// with the size it was rasterised at. The lit one is chosen by `shelf_system`.
    shelf_ind_faces: Vec<(TexId, u32, u32)>,
    /// The same three badges for the light theme, which the card draws separately. Held as its
    /// own set rather than tinted, because a badge is artwork: the two are chosen between by
    /// the palette's mode, which can change while the device is on.
    shelf_ind_light: Vec<(TexId, u32, u32)>,
    /// The system-shelf swap in flight, if any. While it is `Some` the app takes no input.
    shelf_switch: Option<ShelfSwitch>,
    powering_off: bool,
}

impl App {
    pub fn new(carts: Vec<Cart>) -> Self {
        let letter_counts = tally_letters(&carts);
        App {
            phase: Phase::Shelf,
            shelf: Shelf::new(carts),
            letters: Letters::new(),
            letter_counts,
            letter_faces: Vec::new(),
            letter_ridge_face: None,
            play_held: None,
            fav_held: None,
            sys_held: None,
            mtp_held: None,
            fav_star: None,
            fav_ind: None,
            refusal: None,
            refused_from: None,
            alert_face: None,
            shelf_title_face: None,
            shutdown_faces: Vec::new(),
            power_menu: None,
            power_menu_faces: Vec::new(),
            direct_link: None,
            direct_faces: Vec::new(),
            direct_at: 0,
            direct_ip: None,
            direct_ip_face: None,
            direct_ip_dirty: false,
            core_picker: None,
            core_board_face: None,
            core_lid_face: None,
            core_faces_stem: None,
            core_socket_faces: Vec::new(),
            core_chip_faces: Vec::new(),
            core_blank_chip_face: None,
            core_chip_shadow_face: None,
            core_legend_faces: Vec::new(),
            game_menu: None,
            game_menu_faces: Vec::new(),
            link_menu_faces: Vec::new(),
            link_step_faces: Vec::new(),
            link_fail_faces: Vec::new(),
            starting: None,
            link_transport: None,
            restarting: false,
            act_at: 0,
            root: None,
            state: SlotState::default(),
            vol_before: Vec::new(),
            snapshot: None,
            core: Core::default(),
            link: None,
            sfx: None,
            polaroids: None,
            pending: None,
            legend_faces: Vec::new(),
            undo_face: None,
            clock_faces: None,
            sticker_face: None,
            shortcut_rows: Vec::new(),
            shortcut_hint: None,
            wallpaper: None,
            battery_percent: slot_ui::Printed::default(),
            bolt: None,
            shelf_clock: slot_ui::Printed::default(),
            hud: Hud::new(),
            cheats_on: true,
            cheats: Vec::new(),
            cheat_menu: None,
            cheat_faces: Vec::new(),
            cheat_face_stem: None,
            cheat_empty_face: None,
            cheats_dirty: false,
            mode_dirty: false,
            shelf_dirty: false,
            screen: 0.0,
            game_ready: false,
            clock: 0.0,
            power: None,
            dozed_at: 0,
            autosave_at: AUTOSAVE_MS,
            battery_at: BATTERY_POLL_MS,
            charge_at: CHARGE_POLL_MS,
            battery: None,
            last_led: None,
            powering_off: false,
            audio: Profile::default(),
            overlay_pixels: None,
            overlay_tex: None,
            overlay_dirty: false,
            overlay_list: Vec::new(),
            overlay_slot: 0,
            overlay_index: 0,
            overlay_indices: [0, 0],
            shelf_system: System::Gba,
            fav_view: false,
            shelf_ind_faces: Vec::new(),
            shelf_ind_light: Vec::new(),
            shelf_switch: None,
            display: DisplayFilter::new(
                slot_gfx::builtin_panel_mask(),
                DisplayFilter::DEFAULT_CC,
                2,
                [0, 0, 0],
            ),
            // Page one, browser shut. `App::boot` moves the page onto whatever the card had
            // chosen, so the first tap of the palette key walks the fifteen around it.
            pal_page: 0,
            pal_cursor: None,
            pal_name: None,
        }
    }

    /// A seated cart goes back in through the insert animation rather than appearing
    /// already playing, so a boot and a resume are the same movement. A card with no
    /// `Games` directory scans empty, which is a shelf, not a boot failure.
    pub fn boot(root: &Path) -> Self {
        crate::root::ensure(root);
        crate::root::migrate(root);
        // Read the card's label preference (strip_tags) before the first scan, so the displayed
        // and filed names both honour it. A missing file leaves the long-standing default.
        slot_store::init_label_config(root);
        // Before anything is drawn. The card's palette cannot change while the device is on,
        // so it is read once and never asked for again.
        slot_ui::set_theme(Theme::read(root));
        // The card's own cart art, if it has any, also read once before the first frame and
        // before the face builder thread starts. Read once because a cart face is built on that
        // thread and the art has to be there, whole, by then; and said out loud because a card
        // whose art was refused — an index-coloured PNG, say — has nothing else to say so.
        slot_ui::install_cart_art(slot_ui::load_cart_art(root));
        eprintln!("slot: cart art: {}", slot_ui::cart_art_summary());
        // The cached scan, not the plain one: a boot re-reads only the carts that changed
        // since the last one, which on a card of a few hundred games is the difference
        // between a shelf and a wait. See `slot_store::scan_cached`.
        let mut app = App::new(scan_cached(root).unwrap_or_default());
        app.root = Some(root.to_path_buf());
        // The display filter (panel mask + colour correction) is read from the card so the
        // shipping look can be a per-card choice, then cycled live during play. The mask falls
        // back to the built-in LCD3x table; the colour correction falls back to the NOCOLOR (0%
        // default; the modes fall back to (2, 0) (LCD3X + OFF, the shipped look).
        let lcd3x = crate::root::panel_mask(root).unwrap_or_else(slot_gfx::builtin_panel_mask);
        let nocolor_cc = crate::root::color_correction(root).unwrap_or(DisplayFilter::DEFAULT_CC);
        // Before the modes are read, not after: a card written before the Game Boy's palette list
        // was the table it is now has an index into the *old* one, and half of that range means
        // something else here. `migrate_palette` puts the look back where it was and leaves a
        // marker so it happens exactly once per card.
        crate::root::migrate_palette(root);
        let (mask_mode, cc) = crate::root::display_modes(root);
        app.display = DisplayFilter::new(lcd3x, nocolor_cc, mask_mode, cc);
        // The page the palette key walks starts on the entry the card left the Game Boy at, so
        // the first tap continues from that palette rather than from page one.
        app.pal_page = app.gb_palette() / slot_ui::PALETTE_PER_PAGE;
        app.audio = crate::root::audio_profile(root);
        app.state = read_slot_state(root);
        // Which machine's shelf opens, and which overlay each system was last showing, both
        // off the card. A cart still in the slot outranks the stored shelf — a resume has to
        // open the shelf that holds it — and a stored shelf the card has no carts for is
        // skipped, so a card whose only games are, say, GB never opens on an empty GBA shelf.
        app.overlay_indices = crate::root::overlay_indices(root);
        let last_system = app
            .state
            .cart
            .as_deref()
            .and_then(|stem| app.shelf.carts.iter().find(|c| c.stem == stem))
            .map(|c| c.system());
        let mut system = last_system.unwrap_or_else(|| crate::root::shelf_system(root));
        if !app.shelf.carts.iter().any(|c| c.system() == system) {
            system = [System::Gba, System::Gb, System::Gbc]
                .into_iter()
                .find(|s| app.shelf.carts.iter().any(|c| c.system() == *s))
                .unwrap_or(System::Gba);
        }
        app.shelf_system = system;
        app.shelf.set_system(system);
        let view = app.system_view(system);
        app.shelf.set_view(view, None);
        app.retally_letters();
        // The palette, before anything has been rasterised: every face on the device is baked
        // with the ink burned into it, so the mode has to be in place before the first one, not
        // after. Anything later than this line is a light-mode device drawn in dark type.
        slot_ui::palette::set_mode(app.state.mode);
        if app.state.clock_set {
            app.start();
        } else {
            // Seeded from the system clock and re-seeded by `set_power`, which is the first
            // moment there is a platform whose clock is the device's rather than the host's.
            app.phase = Phase::SetClock {
                picker: ClockPicker::from_secs(system_secs()),
            };
        }
        app
    }

    /// The mask the game pass should multiply by right now, resolved through the current mask_mode
    /// (white when the mask is cycled off). Pushed to the compositor every frame.
    pub fn display_mask(&self) -> [[[u8; 3]; 3]; 3] {
        // A Game Boy does not use the aperture table at all: its lattice comes from `display_grid`
        // below, so that the dots and the picture are the same four colours. SELECT+X is therefore
        // free on this machine and steps the screen art instead — see `Phase::Playing`.
        self.display.applied_mask(self.current_machine() == System::Gb)
    }

    /// The Game Boy's four shades right now, whichever of the two tables they came from: a model's
    /// own set while a model's screen art is up, and the entry `cc` holds otherwise.
    ///
    /// This is the one place that decides which of the two wins, so the palette, the lattice and
    /// the boot log cannot disagree about it.
    fn gb_shades(&self) -> Option<[[u8; 3]; 4]> {
        if self.current_machine() != System::Gb {
            return None;
        }
        if let Some(m) = App::model_of_index(self.overlay_slot, self.overlay_index) {
            let m = m.min(crate::palettes::GB_MODELS.len() - 1);
            return Some(crate::palettes::GB_MODELS[m].1);
        }
        Some(crate::palettes::GB_PALETTES[self.gb_palette()].1)
    }

    /// The Game Boy's entry in the palette table, clamped to it. Out of range can only come from
    /// a `display.txt` edited by hand, and a card that names entry 9000 should show the last
    /// palette rather than have the app fall over.
    fn gb_palette(&self) -> usize {
        (self.display.cc[DisplayFilter::machine_slot(System::Gb)] as usize)
            .min(crate::palettes::GB_PALETTES.len() - 1)
    }

    /// The panel lattice for the machine that is up, as its colour and how much of it to mix.
    ///
    /// The colour is the current palette's **lightest shade**, which is the background: a DMG's
    /// mesh is olive, a Light's is cyan, and neither is written down anywhere — change the
    /// palette and the lattice follows. `None` on every other machine, where the aperture table
    /// above still does the work and there is only one shade of lattice there is any sense in.
    pub fn display_grid(&self) -> Option<([f32; 3], f32)> {
        let light = self.gb_shades()?[0];
        Some((
            [
                light[0] as f32 / 255.0,
                light[1] as f32 / 255.0,
                light[2] as f32 / 255.0,
            ],
            DisplayFilter::GB_GRID_MIX,
        ))
    }

    /// Which machine the display filter is being asked about: the cart under the caret, which
    /// during play is the cart in the slot, and the shelf's own machine when there is no caret
    /// (an empty card). GB and GBC are two machines with two palettes even though they share one
    /// screen geometry, so this cannot be `screen_system`.
    pub fn current_machine(&self) -> System {
        self.shelf
            .current_cart()
            .map(|c| c.system())
            .unwrap_or(self.shelf_system)
    }

    /// The colour-correction matrix for the game pass right now, identity when off.
    pub fn display_cc(&self) -> [[f32; 3]; 3] {
        self.display.applied_cc(self.current_machine()).0
    }

    /// ...and its black point, which is the whole difference between a saturation grade and a
    /// hardware palette. Zero now that the Game Boy is drawn by a lookup and not a matrix; the
    /// colour machines never had one.
    pub fn display_cc_bias(&self) -> [f32; 3] {
        self.display.applied_cc(self.current_machine()).1
    }

    /// The Game Boy's palette as a 256-entry RGBA lookup, or `None` when this frame is not a
    /// Game Boy palette at all — 原生灰, where the core's own greys are the look, and every
    /// colour machine, which uses the matrix above.
    ///
    /// Worth being clear about the cost, because it looks like the expensive one and is not:
    /// the matrix path is two `pow`s and nine multiplies per pixel, and this is one extra
    /// nearest-neighbour sample replacing all of it. `pow` is the instruction to avoid on this
    /// class of GPU.
    pub fn display_palette(&self) -> Option<[u8; 1024]> {
        let machine = self.current_machine();
        if machine != System::Gb {
            return None;
        }
        // 原生灰 is the core's own greys, so there is nothing to look up — and only the *table's*
        // own entry means that. A model is always a palette: its four shades are the other half of
        // the art, not a preference layered over it.
        if self.display.cc[DisplayFilter::machine_slot(machine)] as usize
            == crate::palettes::GB_NEUTRAL
            && App::model_of_index(self.overlay_slot, self.overlay_index).is_none()
        {
            return None;
        }
        Some(DisplayFilter::gb_lut(self.gb_shades()?))
    }

    /// What the Game Boy's current palette is called, for the boot log. One table, names and
    /// colours together, so a name cannot drift from what it names.
    pub fn display_palette_name(&self) -> &'static str {
        if self.current_machine() == System::Gb {
            if let Some(m) = App::model_of_index(self.overlay_slot, self.overlay_index) {
                return crate::palettes::GB_MODEL_NAMES
                    [m.min(crate::palettes::GB_MODEL_NAMES.len() - 1)];
            }
        }
        crate::palettes::GB_PALETTES[self.gb_palette()].0
    }

    /// 当前色彩校正该用的 gamma：FULLCOLOR(0)/NOCOLOR(2) 保持 1.0（编码空间，旧行为），
    /// 半彩(1) 与四档单色背光(3..=6) 用 `CC_GAMMA` 在线性空间里做。
    /// GB 的色板同理：原生灰走 1.0（本来就是核心给的灰），其余色板都走 `CC_GAMMA`
    /// —— LUT 的插值要在线性空间里做，否则中间两档会被压暗。
    pub fn display_cc_gamma(&self) -> f32 {
        let machine = self.current_machine();
        let mode = self.display.cc[DisplayFilter::machine_slot(machine)] as usize;
        let flat = if machine == System::Gb {
            mode == crate::palettes::GB_NEUTRAL
                && App::model_of_index(self.overlay_slot, self.overlay_index).is_none()
        } else {
            mode == 0 || mode == 2
        };
        if flat {
            1.0
        } else {
            DisplayFilter::CC_GAMMA
        }
    }

    /// The audio profile the shelf has settled on. `Session::sync_audio_profile` is what
    /// notices a change and reopens the device for it.
    pub fn audio_profile(&self) -> Profile {
        self.audio
    }

    /// SELECT+VOL+ / SELECT+VOL- on the shelf: step the audio profile and persist it to
    /// `System/audio.txt`. Nothing is reopened here — the sink belongs to the session, and
    /// this only decides what it should open for.
    fn cycle_audio(&mut self, forward: bool) {
        self.audio = if forward {
            self.audio.next()
        } else {
            self.audio.prev()
        };
        if let Some(root) = &self.root {
            crate::root::write_audio_profile(root, self.audio);
        }
        // The profile moves no pixel of the picture, so this line is the only thing that tells
        // the player the press landed at all.
        let said = match self.audio {
            Profile::Stable => Toast::AudioStable,
            Profile::Balanced => Toast::AudioBalanced,
            Profile::Strict => Toast::AudioStrict,
        };
        self.hud.toast(said, self.now());
    }

    /// SELECT+X: advance the panel mask (OFF -> LCD3X 50% -> LCD3X 100% -> SCANLINE 50% ->
    /// SCANLINE 100%) and persist both modes to `System/display.txt`.
    fn cycle_mask(&mut self) {
        self.display.cycle_mask();
        if let Some(root) = &self.root {
            crate::root::write_display_modes(root, self.display.mask_mode, self.display.cc);
        }
    }
    /// SELECT+Y: advance the colour correction of the machine that is up, and persist all of
    /// them to `System/display.txt`. On a colour machine that is FULLCOLOR -> HALFCOLOR ->
    /// NOCOLOR -> DMG green -> ice-blue -> amber -> pink backlight; on a Game Boy it is the next
    /// palette on the page the browser last showed.
    ///
    /// **One page, not the whole table.** The Game Boy's list is every palette gambatte ships —
    /// hundreds of them — and a key that walked all of them would be a key nobody could aim. The
    /// page is the unit: hold the key to open the browser and turn to the page you want, let go,
    /// and this walks the fifteen on it. That is also what makes the ring's length something the
    /// user can see, rather than a number that changes with a palette pack.
    ///
    /// **Refused while a model is up**, and that is the whole of what "bound" means. A model's
    /// four shades are not a preference layered over its art, they are the other half of it: the
    /// DMG screen was drawn against the DMG ramp, and letting this key recolour the picture
    /// inside that case would leave the two disagreeing with nothing on screen to say so. The
    /// press is refused rather than dropped, so it reads as answered — the same as every other
    /// "nothing doing" action in this file.
    fn cycle_cc(&mut self) {
        if self.current_machine() == System::Gb {
            if App::model_of_index(self.overlay_slot, self.overlay_index).is_some() {
                self.refuse();
                return;
            }
            self.step_palette();
        } else {
            self.display.cycle_cc(self.current_machine());
        }
        if let Some(root) = &self.root {
            crate::root::write_display_modes(root, self.display.mask_mode, self.display.cc);
        }
    }

    /// The Game Boy's palette one along the page that is up, wrapping at the end of that page.
    /// `self.pal_page` is the page the browser was last showing, so a palette picked on page 7
    /// keeps being cycled among page 7's fifteen until the browser is opened again.
    fn step_palette(&mut self) {
        let total = crate::palettes::GB_PALETTES.len();
        let next = slot_store::cycle_on_page(
            self.pal_page,
            self.gb_palette(),
            total,
            slot_ui::PALETTE_PER_PAGE,
        );
        self.display.cc[DisplayFilter::machine_slot(System::Gb)] = next as u16;
    }

    /// Into the slot or onto the shelf. Reached on boot once the clock is known, and from
    /// the clock screen when it becomes known.
    fn start(&mut self) {
        // One cart is a dedicated device. There is nothing to choose between, so whatever
        // `slot.state` remembers, including a cart that is no longer on the card, names the
        // only thing it could have meant.
        // A position in the *view*, not the library: the shelf is one machine's carts, and a
        // library index would seat the caret on the wrong cart — or past the end of a shorter
        // view — the moment the shelf is filtered. `index_of_stem` reads the view.
        let seated = if self.single_cart() {
            Some(0)
        } else {
            self.state
                .cart
                .as_deref()
                .and_then(|stem| self.shelf.index_of_stem(stem))
        };
        self.phase = Phase::Shelf;
        match seated {
            Some(i) => {
                // The shelf sits on the resumed cart so ejecting it lands where it left.
                self.shelf.index = i;
                self.shelf.scroll = i as f32;
                // Never clean: a resume is the whole point of the cart still being in there.
                self.insert(false);
                if let Phase::Inserting { resumed, t, .. } = &mut self.phase {
                    *resumed = true;
                    // Seated already. The floor still runs, so the core has the same time to
                    // load; the cart simply does not travel to get there.
                    *t = INSERT_S;
                }
            }
            // A cart the library no longer has is an empty slot. Left uncorrected on disk:
            // the next seat rewrites it, and a boot is the worst moment to need a write.
            None => self.state.cart = None,
        }
        // The strip starts on the letter of whatever the row is showing. Snapped rather than
        // travelled to: there is no previous position at a boot, and a strip arriving from `#`
        // would be the shelf's first movement being one nobody made.
        if let Some(cart) = self.shelf.current_cart() {
            self.letters.snap_to(cart.initial);
        }
    }

    /// Confirms whatever is on the clock screen. It is asked once, so this is also the only
    /// way off it: there is no way back and no second chance to get it wrong.
    pub fn confirm_clock(&mut self) {
        let Phase::SetClock { picker } = &self.phase else {
            return;
        };
        // Both read off before the borrow ends. The platform is given utc, because that is
        // what the base system's clock and its ntp both assume the card holds; the offset is
        // kept beside it as the only thing that turns it back into the time on the wall.
        let (secs, offset) = (picker.secs(), picker.offset_min());
        if let Some(power) = &mut self.power {
            power.set_clock(secs);
        }
        self.state.utc_offset_min = offset as i16;
        self.state.clock_set = true;
        self.persist();
        self.start();
    }

    /// Seconds since the epoch, from the platform once there is one. The shelf clock and the
    /// polaroid captions both read it, so neither can disagree with the cartridge RTC.
    /// Where the card is mounted. `None` only in the tests that never touch one.
    /// Which machine's shelf is up. The cart's own size on screen follows from it, so anything
    /// drawing a row of carts has to ask rather than assume the Advance cart's.
    pub fn shelf_system(&self) -> slot_store::System {
        self.shelf_system
    }

    /// Where the shelf's line of type sits: centred in the gap between the row's own foot and
    /// the top of the case. Derived rather than picked, so moving either end — a taller cart, a
    /// deeper bay — carries it along instead of leaving it behind.
    fn shelf_title_y(&self) -> f32 {
        let foot = slot_ui::size_for(self.shelf_system).foot_y();
        let case = OUT_H as f32 - MOUTH_H;
        (foot + case) / 2.0 - SHELF_TITLE_H as f32 / 2.0
    }

    pub fn root(&self) -> Option<&Path> {
        self.root.as_deref()
    }

    /// Local, not utc. The shelf clock, the polaroid captions and the stamps the states are
    /// named by all come through here, so the offset is applied once rather than at each of
    /// them, and none of them can disagree with the others about what time it is.
    pub fn wall_secs(&self) -> i64 {
        let utc = self.power.as_ref().map_or_else(system_secs, |p| p.now());
        utc + i64::from(self.state.utc_offset_min) * 60
    }

    /// What the clock screen is showing, or `None` off it. The binary rasterises from it and
    /// watches its text to know when to do so again.
    pub fn picker(&self) -> Option<&ClockPicker> {
        match &self.phase {
            Phase::SetClock { picker } => Some(picker),
            _ => None,
        }
    }

    pub fn set_sticker_face(&mut self, face: TexId) {
        self.sticker_face = Some(face);
    }

    /// The shortcut card: one face per row and the line that stays at its foot. Fixed strings,
    /// so they are built once at boot and never asked for again.
    pub fn set_shortcut_faces(
        &mut self,
        rows: Vec<(TexId, u32, u32)>,
        hint: Option<(TexId, u32, u32)>,
    ) {
        self.shortcut_rows = rows;
        self.shortcut_hint = hint;
    }

    /// How far the about card can be scrolled: how much taller than the panel its label and its
    /// rows make it. Zero while the rows are not up yet, which is a card that does not move
    /// rather than one that scrolls into nothing.
    fn about_scroll_max(&self) -> f32 {
        let rows: f32 = self.shortcut_rows.iter().map(|(_, _, h)| *h as f32).sum();
        // The last row has to come to rest clear of the pinned hint's band, not under it, so the
        // room below the rows is the band's whole height. Falls back to `ABOUT_PAD` before the
        // hint is up, which is a card that does not move rather than one that scrolls into
        // nothing.
        let foot = self
            .shortcut_hint
            .map(|(_, _, h)| h as f32)
            .unwrap_or(ABOUT_PAD);
        let end = about_first_y() + rows + foot;
        (end - OUT_H as f32).max(0.0)
    }

    pub fn set_clock_faces(&mut self, line: TexId, hint: TexId) {
        self.clock_faces = Some((line, hint));
    }

    pub fn set_cart_shadow(&mut self, face: TexId) -> Option<TexId> {
        self.shelf.set_shadow(face)
    }

    /// The blank cart a cart whose own face is not built yet is drawn as. Uploaded at boot,
    /// with the shadow: it is the same for every cart and never changes — except when the row's
    /// machine does, which is why this hands back what it replaced.
    pub fn set_cart_placeholder(&mut self, face: TexId) -> Option<TexId> {
        self.shelf.set_placeholder(face)
    }

    pub fn set_wallpaper(&mut self, face: TexId) {
        self.wallpaper = Some(face);
    }

    pub fn set_bolt_face(&mut self, bolt: TexId) {
        self.bolt = Some(bolt);
    }

    pub fn set_battery_percent_face(&mut self, face: TexId, w: u32) {
        self.battery_percent = slot_ui::Printed::new(face, w);
    }

    pub fn set_shelf_clock_face(&mut self, face: TexId, w: u32) {
        self.shelf_clock = slot_ui::Printed::new(face, w);
    }

    pub fn phase(&self) -> &Phase {
        &self.phase
    }

    pub fn carts(&self) -> &[Cart] {
        &self.shelf.carts
    }

    /// Which cart the caret is on. Read by the boot path to decide which faces are worth
    /// rasterising before the first frame, and by the background filler to order the rest.
    pub fn shelf_index(&self) -> usize {
        self.shelf.current().unwrap_or(0)
    }

    /// The carts the row is drawing, as library indices — what a build has to cover, as opposed
    /// to a contiguous run of indices around the caret. See `Shelf::draw_window`.
    pub fn shelf_draw_window(&self) -> Vec<usize> {
        self.shelf.draw_window()
    }

    /// How many carts the shelf's view holds, as opposed to how many are on the card.
    pub fn shelf_view_len(&self) -> usize {
        self.shelf.view_len()
    }

    /// How far a cart is from the caret that would be showing it — the row on screen when the
    /// view holds it, otherwise the front of its own machine's shelf, where a switch lands.
    /// What the resident window and the filler both measure in. See `Shelf::shelf_distance`.
    pub fn shelf_distance(&self, cart: usize) -> Option<usize> {
        self.shelf.shelf_distance(cart)
    }

    /// The cart index in each slot the row would draw, in slot order — the draw's own answer to
    /// "which cart is this", read back for the boot log rather than recomputed.
    pub fn shelf_slot_carts(&self) -> Vec<usize> {
        self.shelf
            .slot_rects(None, 0.0, 0.0)
            .iter()
            .map(|r| r.cart)
            .collect()
    }

    /// Exactly one cart on the card. The shelf is unreachable and eject is refused.
    pub fn single_cart(&self) -> bool {
        self.shelf.carts.len() == 1
    }

    /// Face textures in `carts` order; `None` where the face is still being rasterised.
    /// Only the compositor can mint a `TexId`.
    pub fn set_faces(&mut self, faces: Vec<Option<TexId>>) {
        self.shelf.set_faces(faces);
    }

    /// One face, uploaded after `set_faces`, as the background filler finishes it.
    pub fn set_face(&mut self, i: usize, tex: TexId) {
        self.shelf.set_face(i, Some(tex));
    }

    /// Forgets a face whose texture has been released.
    pub fn clear_face(&mut self, i: usize) {
        self.shelf.clear_face(i);
    }

    /// Which face texture a cart is currently holding, if any.
    pub fn face_of(&self, i: usize) -> Option<TexId> {
        self.shelf.face_of(i)
    }

    /// Hand the seated cart's overlay set to the app, from `Session::spawn_core`: every
    /// `Overlay/<sys>.png` / `<sys>-NN.png` for the cart's system, in rotation order (`slot` 0
    /// for GB, 1 for GBC), with the persisted index for that system applied. `list` empty for
    /// GBA — or a card with no art — clears the overlay. The chosen file is decoded here and
    /// `overlay_dirty` is set so the frontend re-mints the GL texture once.
    pub(crate) fn set_overlay_set(&mut self, list: Vec<crate::root::OverlayFile>, slot: usize) {
        self.overlay_list = list;
        let slot = slot.min(1);
        self.overlay_slot = slot;
        let choices = Self::overlay_choices(slot, self.overlay_list.len());
        self.overlay_index = if choices == 0 {
            0
        } else {
            self.overlay_indices[slot].min(choices - 1)
        };
        self.refresh_overlay_pixels();
    }

    /// How many overlay choices a system has. A Game Boy's three models come first and the
    /// card's own overlays after them; a Game Boy Color has only its overlays.
    fn overlay_choices(slot: usize, ordinary: usize) -> usize {
        if slot == 0 {
            DisplayFilter::GB_MODEL_COUNT + ordinary
        } else {
            ordinary
        }
    }

    /// Which model a combined index names, if it names one.
    fn model_of_index(slot: usize, index: usize) -> Option<usize> {
        (slot == 0 && index < DisplayFilter::GB_MODEL_COUNT).then_some(index)
    }

    /// Where the card's *own* overlay for this index is, if the index is one of those rather
    /// than a model.
    fn ordinary_overlay_at(&self) -> Option<&crate::root::OverlayFile> {
        let at = if self.overlay_slot == 0 {
            self.overlay_index.saturating_sub(DisplayFilter::GB_MODEL_COUNT)
        } else {
            self.overlay_index
        };
        self.overlay_list.get(at)
    }

    /// Re-decode the chosen overlay file into `overlay_pixels` and mark the texture dirty.
    /// `None` pixels (`None` from a missing/corrupt file, or an empty list) is a real answer:
    /// the frontend reads it as "release the texture", so the built-in look stands.
    fn refresh_overlay_pixels(&mut self) {
        // A model's art is not in `overlay_list` — `overlay_files` cannot name it and should not
        // — so the model index is resolved to its own file here.
        self.overlay_pixels = match Self::model_of_index(self.overlay_slot, self.overlay_index) {
            Some(m) => self
                .root
                .clone()
                .and_then(|root| {
                    crate::root::model_overlay(
                        &root,
                        "gb",
                        crate::palettes::GB_MODELS[m.min(crate::palettes::GB_MODELS.len() - 1)].0,
                    )
                })
                .and_then(|path| slot_ui::decode_rgba(&path)),
            None => self
                .ordinary_overlay_at()
                .and_then(|overlay| slot_ui::decode_rgba(&overlay.path)),
        };
        self.overlay_dirty = true;
    }

    /// Whether the overlay up asks for the screen reflection — its file name carried the `x`
    /// marker. Read by the frontend to hand the compositor the overlay texture as the effect's
    /// zone mask; `None`/false leaves the game drawn exactly as before.
    pub(crate) fn overlay_reflects(&self) -> bool {
        // Every model mask is a reflection layer: that is what they were drawn as, and it is why
        // their names carry the same trailing `x` the ordinary ones use to say so.
        if Self::model_of_index(self.overlay_slot, self.overlay_index).is_some() {
            return true;
        }
        self.ordinary_overlay_at().is_some_and(|o| o.reflect)
    }

    /// The overlay texture the compositor minted, if any. The reflection samples its alpha as
    /// the zone, so the same texture is both the art and the effect's mask.
    pub(crate) fn overlay_tex(&self) -> Option<TexId> {
        self.overlay_tex
    }

    /// SELECT+R2: step to the next overlay for the highlighted cart's system (the one in the
    /// slot while a game is playing). A system with a single file has nothing to rotate, and
    /// GBA has no overlay set at all; both are silent no-ops rather than a refusal, since the
    /// press is a step through a ring and a card may simply carry one image.
    fn overlay_next(&mut self) {
        let Some(system) = self.shelf.current_cart().map(|c| c.system()) else {
            return;
        };
        let slot = match system {
            System::Gb => 0,
            System::Gbc => 1,
            System::Gba => return,
        };
        let Some(root) = self.root.clone() else {
            return;
        };
        let which = if slot == 0 { "gb" } else { "gbc" };
        let list = crate::root::overlay_files(&root, which);
        // A Game Boy's ring is its three models and then the card's own overlays; a Game Boy
        // Color's is just its overlays.
        let choices = Self::overlay_choices(slot, list.len());
        if choices <= 1 {
            return;
        }
        // Note the ring is never empty on a Game Boy even when the card carries no art at all:
        // `choices` counts the three models before anything in `Overlay/`.
        self.overlay_indices[slot] = (self.overlay_indices[slot] + 1) % choices;
        crate::root::write_overlay_indices(&root, self.overlay_indices);
        // Landing on a model adopts its palette — and nothing is written for that here, because
        // there is nothing to write: a model's four shades are not an entry in the table and are
        // not reachable from the palette key, so `gb_shades` reads them straight off the model
        // that is up. The palette the card holds is left exactly as it was, which is what makes
        // walking through the DMG and out again land back on the look that was chosen before it.
        // With the cart in the slot, swap the live set too so the change is on screen now.
        if matches!(self.phase, Phase::Playing { .. }) {
            self.overlay_list = list;
            self.overlay_slot = slot;
            self.overlay_index = self.overlay_indices[slot];
            self.refresh_overlay_pixels();
        }
    }

    /// `true` while the overlay texture needs (re)building. Read by the frontend's `sync_overlay`.
    pub(crate) fn overlay_dirty(&self) -> bool {
        self.overlay_dirty
    }

    /// The decoded overlay pixels, if any — read by `sync_overlay` to (re)build the texture.
    pub(crate) fn overlay_pixels(&self) -> Option<&(Vec<u8>, u32, u32)> {
        self.overlay_pixels.as_ref()
    }

    /// Drop the current overlay texture handle, returning it for the compositor to release.
    pub(crate) fn take_overlay_tex(&mut self) -> Option<TexId> {
        self.overlay_tex.take()
    }

    /// Store the freshly minted overlay texture handle (or `None` when there is no overlay).
    pub(crate) fn set_overlay_tex(&mut self, tex: Option<TexId>) {
        self.overlay_tex = tex;
    }

    /// Clear the dirty flag once `sync_overlay` has reconciled the texture with `overlay_pixels`.
    pub(crate) fn set_overlay_clean(&mut self) {
        self.overlay_dirty = false;
    }

    /// Whether the app is taking no input right now — true only for the length of a
    /// system-shelf swap. `Session::act` reads it so every key is inert mid-swap, the volume
    /// and mute included: they are handled on the session's side of `apply`.
    pub fn input_locked(&self) -> bool {
        self.shelf_switch.is_some()
    }

    /// The three machine logos for the bottom-left indicator, in `GB, GBC, GBA` order, each
    /// with the size it was rasterised at. Uploaded by the frontend at boot.
    pub fn set_shelf_ind_faces(&mut self, faces: Vec<(TexId, u32, u32)>) {
        self.shelf_ind_faces = faces;
    }

    /// The light theme's own three, used when the palette is in light mode.
    pub fn set_shelf_ind_light(&mut self, faces: Vec<(TexId, u32, u32)>) {
        self.shelf_ind_light = faces;
    }

    /// The library indices of one machine's carts, in library order — the view a system shelf
    /// shows. A card can carry all three machines at once, so this is how the row is narrowed
    /// to the one whose shelf is up.
    /// One machine's shelf, in the order it shows. Kept as a call rather than a field so the
    /// shelf stays the only thing that knows how a machine's list is built.
    fn system_view(&self, system: System) -> Vec<usize> {
        self.shelf.machine_view(system)
    }

    /// Put one machine's shelf up: rebuild the view to its carts, drop any favourites sub-view,
    /// park the caret at the front, and remember the choice on the card.
    fn show_system(&mut self, system: System) {
        self.shelf_system = system;
        // The row is laid out for whichever machine is up: an Advance cart is landscape and a
        // Game Boy cart portrait, so the geometry travels with the shelf rather than the app.
        self.shelf.set_system(system);
        // ...and so does the row's *frame*: the shadow under a dimmed cart and the blank cart a
        // not-yet-built face stands in as are both cut to one machine's shape, and they were cut
        // for the shelf this device came up on. Raised here and taken by the frontend, which is
        // the only party with a compositor.
        self.shelf_dirty = true;
        self.fav_view = false;
        let view = self.system_view(system);
        self.shelf.set_view(view, None);
        self.retally_letters();
        if let Some(root) = &self.root {
            crate::root::write_shelf_system(root, system);
        }
    }

    /// SELECT+L2: start a swap to the next machine that has any carts. Nothing happens — and
    /// the press is refused, so it reads as answered — on a card that carries only one machine.
    /// A swap already in flight swallows the press, so a second one cannot stack on the first.
    fn cycle_system(&mut self) {
        if self.shelf_switch.is_some() || !self.on_shelf() || self.core_picker.is_some() {
            return;
        }
        const ORDER: [System; 3] = [System::Gba, System::Gb, System::Gbc];
        let cur = ORDER.iter().position(|&s| s == self.shelf_system).unwrap_or(0);
        for k in 1..=ORDER.len() {
            let to = ORDER[(cur + k) % ORDER.len()];
            if self.shelf.carts.iter().any(|c| c.system() == to) {
                // A held arrow belongs to the shelf that is leaving; the swap owns the row now.
                self.shelf.release_hold();
                self.letters.release_hold();
                self.shelf_switch = Some(ShelfSwitch {
                    from: self.shelf_system,
                    to,
                    t: 0.0,
                    swapped: false,
                });
                return;
            }
        }
        self.refuse();
    }

    /// The swap's clock and the row's motion. Runs every frame; with no swap in flight it
    /// simply clears the motion, so a still row is laid out exactly as before this existed.
    fn update_shelf_switch(&mut self, dt: f32) {
        let Some(mut sw) = self.shelf_switch.take() else {
            self.shelf.set_motion(None);
            return;
        };
        sw.t += dt;
        // The view swaps at the bottom of the ascend: the old carts are off the top of the
        // panel by then, so the new shelf appears to fall into the space they left.
        if !sw.swapped && sw.t >= SWITCH_ASCEND_S {
            sw.swapped = true;
            self.show_system(sw.to);
        }
        if sw.t >= SWITCH_ASCEND_S + SWITCH_DESCEND_S {
            self.shelf.set_motion(None);
            return;
        }
        let p = if sw.t < SWITCH_ASCEND_S {
            sw.t / SWITCH_ASCEND_S
        } else {
            (sw.t - SWITCH_ASCEND_S) / SWITCH_DESCEND_S
        };
        self.shelf.set_motion(Some(slot_ui::Motion {
            p: p.clamp(0.0, 1.0),
            ascend: !sw.swapped,
        }));
        self.shelf_switch = Some(sw);
    }

    /// The logo of the machine whose shelf is up, in the bottom-left corner — the mirror of the
    /// favourites star in the bottom-right. Only ever one logo: it *is* the shelf. While a swap
    /// runs, the change is a cross-dissolve — the old machine's logo fades out as the new one
    /// fades in, both in place — so it dissolves rather than cuts. A reading, not a control:
    /// SELECT+L2 is the control.
    fn draw_shelf_indicator(&self, out: &mut Vec<Draw>) {
        // The top of the bottom band, which is the box the badge is centred in.
        let y = OUT_H as f32 - MOUTH_H;
        match &self.shelf_switch {
            Some(sw) => {
                let total = SWITCH_ASCEND_S + SWITCH_DESCEND_S;
                let p = (sw.t / total).clamp(0.0, 1.0);
                // Smoothstep, so the dissolve eases in and out rather than fading linearly.
                let e = p * p * (3.0 - 2.0 * p);
                self.draw_shelf_logo(sw.from, 1.0 - e, y, out);
                self.draw_shelf_logo(sw.to, e, y, out);
            }
            None => self.draw_shelf_logo(self.shelf_system, 1.0, y, out),
        }
    }

    /// One machine logo, bottom-left, at `alpha`. Both halves of a cross-dissolve come through
    /// here, so they are placed identically and only their opacity differs.
    fn draw_shelf_logo(&self, system: System, alpha: f32, y: f32, out: &mut Vec<Draw>) {
        if alpha <= 0.0 {
            return;
        }
        let idx = match system {
            System::Gb => 0,
            System::Gbc => 1,
            System::Gba => 2,
        };
        let faces = match slot_ui::palette::mode() {
            slot_ui::Mode::Light => &self.shelf_ind_light,
            slot_ui::Mode::Dark => &self.shelf_ind_faces,
        };
        let Some(&(tex, w, h)) = faces.get(idx) else {
            return;
        };
        if h == 0 {
            return;
        }
        // Centred in the band's height, and a fixed margin in from the panel's left edge. A
        // card's own badge is contained in the box rather than stretched, so one that arrives a
        // different shape still sits right; the built-in word is the same width it always was.
        let cy = y + MOUTH_H / 2.0;
        let k = (SHELF_IND_W / w as f32).min(SHELF_IND_H / h as f32);
        let (sw, sh) = (w as f32 * k, h as f32 * k);
        // Centred on the clock's own centre at the other end of the top band, so the two left-hand
        // marks stand in one column.
        let clock_cx = STATUS_READ_MARGIN + self.shelf_clock.w as f32 / 2.0;
        out.push(Draw::Tex {
            x: clock_cx - sw / 2.0,
            y: cy - sh / 2.0,
            w: sw,
            h: sh,
            tex,
            alpha,
        });
    }

    /// One face per slot in `letters::SLOTS` order, each with the size it was rasterised at,
    /// uploaded at boot. The sizes come with them because a face cropped to its ink is as wide
    /// as the letter is, and drawing it without them would set every letter to one width.
    pub fn set_letter_faces(&mut self, faces: Vec<(TexId, u32, u32)>) {
        self.letter_faces = faces;
    }

    /// The metal between two letters, with the size it was rasterised at — a face cropped to
    /// its ink is as wide as it is and no wider, so the size has to travel with the id the same
    /// way a letter's does.
    pub fn set_letter_ridge_face(&mut self, ridge: (TexId, u32, u32)) {
        self.letter_ridge_face = Some(ridge);
    }

    /// Left or right on the shelf: move the index one letter and bring the row with it.
    ///
    /// The ring keeps the repeat, so this is the press and nothing else. The caret is seated on
    /// the first cart of the letter it landed on — that is the whole point of the index — and
    /// the row is handed the move as a glide, so it sweeps across every cart it is skipping
    /// rather than arriving as if only one had moved.
    ///
    /// `by` is the direction of the step: +1 for right (the next letter), -1 for left (the
    /// previous one).
    fn step_letters(&mut self, by: i32, now: Millis) {
        // The dial counts the library and the favourites shelf is narrower than it, so a
        // letter jump could seat the caret on a cart this view does not hold. Letter nav is
        // the library's own control and stands down while a view is filtered; the arrows
        // still walk the row.
        if self.fav_view {
            return;
        }
        let counts = self.letter_counts;
        if !self.letters.hold(by, now, &counts) {
            return;
        }
        self.seat_on_letter();
    }

    /// The same move, without a press behind it: the repeat of a held key.
    fn repeat_letters(&mut self, now: Millis) {
        if self.fav_view {
            return;
        }
        let counts = self.letter_counts;
        if self.letters.tick(now, &counts) {
            self.seat_on_letter();
        }
    }

    /// Puts the caret on the first cart of the letter the strip is showing, and hands the row
    /// the move as a glide so it sweeps across every cart it is skipping rather than flicking
    /// through only the last one. The direction is the row's own to take — it is a ring, and
    /// the glide runs to the nearest image of the chosen cart — so the strip and the row agree
    /// about which way is "onward" without this having to say so.
    fn seat_on_letter(&mut self) {
        if self.fav_view {
            return;
        }
        let slot = self.letters.target();
        // Found in the *view*, not the library: the shelf holds one machine's carts, and the
        // strip counts those (`retally_letters`), so `index` must be a view position.
        let Some(i) = self.shelf.view_position_of_letter(slot) else {
            // A slot the strip can only have reached by being counted as non-empty, so this is
            // unreachable; the next frame's `centre_on` puts it back if it ever happens.
            return;
        };
        self.shelf.index = i;
        // A held arrow key is a direction the shelf is no longer travelling in, and leaving
        // it armed would fire a cart step over the letter that was just chosen.
        self.shelf.release_hold();
        self.shelf.glide_to_target();
    }

    /// Handed over when the core is spawned, which is on the way into the slot.
    pub fn set_snapshot(&mut self, snapshot: Box<dyn Snapshot>) {
        self.snapshot = Some(snapshot);
    }

    /// The `Core` that `session.rs` resolved for the cart it just spawned. Called in the same
    /// breath as `set_snapshot`, from the one place a cart's core is ever decided, so every
    /// later read or write in this file has a stored answer to take rather than a reason to
    /// ask `core_for` again.
    pub fn set_core(&mut self, core: Core) {
        self.core = core;
    }

    /// Whether a netpacket session is live right now. libretro disables an entire class of
    /// time manipulation for as long as one is — see `may_rewind`/`may_load_state` — because
    /// rewinding or loading a state on one device desynchronises the other with no way back
    /// to agreement.
    pub fn link_active(&self) -> bool {
        self.link.is_some()
    }

    /// Begins a session. `client_id` is libretro's own: 0 the host, 1 the joiner — the only
    /// two this product has. Nothing here touches a transport or a core; that lives on the
    /// emulator thread, wherever `EmuHandle::begin_link` is called from the same gesture this
    /// answers. A session always starts from the cart's battery save, never a state — that
    /// falls out for free here, since nothing on this path touches the state ring at all.
    pub fn begin_link(&mut self, client_id: u16) {
        self.link = Some(LinkSession { client_id });
    }

    /// Which side of the session this device is, for whatever the UI ends up showing while
    /// one is live. `None` when there is nothing to ask about.
    pub fn link_client_id(&self) -> Option<u16> {
        self.link.as_ref().map(|s| s.client_id)
    }

    /// Rewinding one device desynchronises the other with no way back, which is exactly what
    /// libretro's netpacket contract forbids for as long as a session is open.
    pub fn may_rewind(&self) -> bool {
        !self.link_active()
    }

    /// Loading a state is the same hazard rewinding is: it moves this device's machine to a
    /// moment the peer never agreed to and has no way to follow.
    pub fn may_load_state(&self) -> bool {
        !self.link_active()
    }

    /// Fast forward runs this device's machine out ahead of what the peer has actually been
    /// sent — the same desync `may_rewind` refuses for running it backwards instead, and
    /// named in the very libretro.h sentence `may_rewind`'s own contract comes from
    /// ("pausing, slow motion, fast forward, rewinding, save state loading... are disabled").
    pub fn may_fast_forward(&self) -> bool {
        !self.link_active()
    }

    /// Ends the session and leaves the cart playing single player. Never an error: the peer
    /// vanishing and the user ending it deliberately look the same from here.
    ///
    /// This is the app's own bookkeeping only, exactly like `link` itself (see its doc
    /// comment) — it does not touch the core or the transport, both of which live on the
    /// emulator thread. `Session::act` is what actually reaches them: it watches
    /// `link_active()` around every `apply`, and mirrors an ending onto
    /// `EmuHandle::end_link()`, which is what tells the core (`RetroCore::stop_link`, if it
    /// offered one to call) and drops the transport. Anything that ends a session without
    /// going through `apply` would need to repeat that mirroring by hand — there is no such
    /// call site today.
    pub fn end_link(&mut self) {
        self.link = None;
    }

    /// The app has no device, so the sound it wants is left here for whoever does.
    pub fn take_sfx(&mut self) -> Option<Sfx> {
        self.sfx.take()
    }

    /// The panel comes up at the level the card remembers rather than at whatever the
    /// kernel left it at.
    pub fn set_power(&mut self, mut power: Power) {
        power.set_backlight(self.state.brightness);
        // The device's own clock, which the host's stands in for. Boot has nothing better to
        // seed the picker from, so a device with a live RTC only gets its confirmation here.
        // The first moment the device's own clock can be asked, and so the first moment a
        // clock that was never set can be told apart from one that was. Boot has already
        // taken `clock_set` at its word by here, which is exactly the case that leaves a
        // dead RTC with no way back to the one screen that could fix it.
        let secs = power.now();
        match &mut self.phase {
            Phase::SetClock { picker } => *picker = ClockPicker::from_secs(secs),
            _ if secs < CLOCK_FLOOR => {
                self.phase = Phase::SetClock {
                    picker: ClockPicker::from_secs(secs),
                }
            }
            _ => {}
        }
        self.power = Some(power);
        // There is nothing to read before this call — no gauge for `battery_at`, no charge
        // for `charge_at`, and whatever `led_state` computed with no battery is not a real
        // state to have already told a platform that did not exist yet either. All three are
        // placeholders with nothing behind them, not real deadlines or a real prior write, so
        // the first tick after this one has to act as if nothing has been read or written
        // yet — or the case band's left shelf sits blank and the LED sits stale for the first
        // poll of whichever cadence is longer, which is the one moment either is cheapest to
        // have been wrong to skip.
        self.battery_at = self.now();
        self.charge_at = self.now();
        self.last_led = None;
    }

    /// The motor. Never persisted and never a level: it belongs to the cart that asked for
    /// it and stops with it.
    pub fn set_rumble(&mut self, strength: u16) {
        if let Some(power) = &mut self.power {
            power.set_rumble(strength);
        }
    }

    /// Set by the doze timeout and by a graceful power off. The binary is what acts on it:
    /// everything durable has already been written by the time it is true.
    ///
    pub fn powering_off(&self) -> bool {
        self.powering_off
    }

    /// What the binary waits for. The decision is made the instant the choice is, but the
    /// machine is not allowed to stop until the ordinary loop has drawn and presented the
    /// shutdown screen. Rendering out of band instead — one extra draw and swap between the
    /// choice and `poweroff` — hung the device: the swap can block on a GPU about to be torn
    /// down, and slot then never reached `poweroff` at all.
    pub fn ready_to_power_off(&self) -> bool {
        self.powering_off && self.now() >= self.act_at
    }

    pub fn ready_to_restart(&self) -> bool {
        self.restarting && self.now() >= self.act_at
    }

    /// Whether the shutdown screen is what should be on the panel. True from the instant the
    /// choice is made, which is earlier than `powering_off`.
    pub fn shutting_down(&self) -> bool {
        self.powering_off || self.restarting
    }

    /// Set by the menu's Restart. Goes through the same shutdown as a power off — busybox
    /// init runs rcK for a reboot too — so the GPU module is unloaded either way, which is
    /// what stops this hardware hanging with the rails up.
    pub fn restarting(&self) -> bool {
        self.restarting
    }

    pub fn restart(&mut self) {
        if let Some(power) = &mut self.power {
            power.restart();
        }
    }

    pub fn power_menu(&self) -> Option<usize> {
        self.power_menu
    }

    /// The core the chip is in or heading for, and `None` once the picker has gone. Still
    /// `Some` while the lid is going back on.
    pub fn core_picker(&self) -> Option<Core> {
        self.core_picker.map(|p| p.seat())
    }

    /// The chip's pose this frame, for whatever draws it.
    pub fn core_picker_chip(&self) -> Option<Chip> {
        self.core_picker.map(|p| p.chip(self.now()))
    }

    /// Whether the in-game menu is up. Read by whoever owns the emulator as well as by the
    /// draw: the game underneath is paused for as long as it is.
    pub fn game_menu_open(&self) -> bool {
        self.game_menu.is_some()
    }

    /// Which screen of the in-game menu is up, and `None` while it is closed.
    pub fn game_menu(&self) -> Option<GameMenu> {
        self.game_menu
    }

    /// What is on the menu right now, in the order it is drawn. Empty while it is closed,
    /// and on the two screens that are a sentence rather than a list.
    pub fn game_menu_rows(&self) -> Vec<&'static str> {
        match self.game_menu {
            Some(GameMenu::Rows(_)) => self.game_rows().iter().map(|r| r.text()).collect(),
            Some(GameMenu::Link(_)) => LinkRow::ALL.iter().map(|r| r.text()).collect(),
            _ => Vec::new(),
        }
    }

    /// The wire a link that just came up runs over, handed on exactly once. `App` never
    /// touches a transport itself — the core and the socket both live on the emulator
    /// thread — so this is left here for whoever owns that thread to collect.
    pub fn take_link_transport(&mut self) -> Option<(u16, Box<dyn LinkChannel>)> {
        self.link_transport.take()
    }

    /// The cart under the highlight, and `None` on an empty shelf.
    pub fn selected_stem(&self) -> Option<&str> {
        self.shelf.current_cart().map(|c| c.stem.as_str())
    }

    /// Which machine's picture belongs on the panel.
    ///
    /// The card's three shelves collapse into the renderer's two here, because Game Boy and
    /// Game Boy Color hand back the same 160x144 frame — `slot_store::System` is the three and
    /// `slot_gfx::System` is the two. `Gba` stands in for an empty shelf, where there is no
    /// picture to misplace either way.
    pub fn screen_system(&self) -> ScreenSystem {
        match self.shelf.current_cart().map(|c| c.system()) {
            Some(System::Gb) | Some(System::Gbc) => ScreenSystem::Gb,
            _ => ScreenSystem::Gba,
        }
    }

    /// The cached reading. `None` until the first slow tick, and on any device with no gauge.
    pub fn battery(&self) -> Option<Battery> {
        self.battery
    }

    /// Does not return when there is a platform to power off. A unit test has none, and
    /// there the flag is the whole of it.
    pub fn poweroff(&mut self) {
        if let Some(power) = &mut self.power {
            power.poweroff();
        }
    }

    /// The face buttons belong to whatever is on screen. The shelf and the switcher each
    /// take them; while the game is playing they are the game's and the app sees only the
    /// gestures that are never the game's.
    pub fn apply(&mut self, action: Action) {
        // A system-shelf swap owns the screen for its two beats: every key is inert until it
        // lands, so nothing can be pressed into the middle of the movement. `Session::act`
        // gates the volume and mute on `input_locked` for the same reason — those are handled
        // on the session's side of `apply` and would otherwise slip through.
        if self.shelf_switch.is_some() {
            return;
        }
        // Ahead of everything, including the device's own keys: the menu is a decision the
        // user is in the middle of making, and a volume press underneath it would be one
        // more thing happening while they read.
        if self.power_menu.is_some() {
            match action {
                Action::LidClose => return self.doze(),
                Action::LidOpen => return self.wake(),
                _ => return self.power_menu_input(action),
            }
        }
        // (WiFi 直连对话框的输入拦截已移除——功能暂缓。)
        // The cheat table owns every button on the game's side while it is up: up/down move, A
        // toggles the row, B leaves, and the lid is honoured so the device can still sleep under it.
        if self.cheat_menu.is_some() {
            match action {
                Action::LidClose => return self.doze(),
                Action::LidOpen => return self.wake(),
                _ => return self.cheat_menu_input(action),
            }
        }
        // The palette browser owns them the same way while it is up: it is a modal over a paused
        // game, and the lid is honoured so the device can still sleep under it.
        if self.pal_cursor.is_some() {
            match action {
                Action::LidClose => return self.doze(),
                Action::LidOpen => return self.wake(),
                _ => return self.palette_browser_input(action),
            }
        }
        // The lid, the light and the sound belong to the device rather than to whatever is
        // on screen, so they are taken before the phase gets a look at the action.
        match action {
            Action::LidClose => return self.doze(),
            Action::LidOpen => return self.wake(),
            Action::PowerPress => {
                // A live session ends here rather than flushing: this is the one button a
                // trade partner mid-exchange can still reach, and ending the session is a
                // decision, not "nothing to flush" — the two must not be the same press.
                if self.link_active() {
                    self.end_link();
                    return;
                }
                return self.flush_resume();
            }
            Action::PowerTap => return self.power_press(),
            Action::PowerHold => return self.open_power_menu(),
            // The release no longer means anything once the menu is what a hold raises:
            // the choice is the commitment, and it is made with A.
            Action::PowerOff => return,
            _ => {}
        }
        // Ahead of the levels too. A screen with no way back is not one to be adjusting the
        // backlight from, and the clock owns all four directions.
        if let Phase::SetClock { picker } = &mut self.phase {
            match action {
                Action::GbaDown(Btn::Left) | Action::ShelfLeft => picker.left(),
                Action::GbaDown(Btn::Right) | Action::ShelfRight => picker.right(),
                Action::GbaDown(Btn::Up) => picker.up(),
                Action::GbaDown(Btn::Down) => picker.down(),
                Action::GbaDown(Btn::A) | Action::Insert => self.confirm_clock(),
                _ => {}
            }
            return;
        }
        if self.adjust(action) {
            return;
        }
        // The release reaches the shelf whatever is on screen. A direction let go of during
        // an insert would otherwise still be held when the cart comes back out.
        match action {
            Action::GbaUp(Btn::Left) => self.shelf.release_left(),
            Action::GbaUp(Btn::Right) => self.shelf.release_right(),
            _ => {}
        }
        // Beside the power menu's own block rather than inside the phase match, so the two
        // menus can never both take a press: that one returns above this, and this returns
        // above the phase. Below the lid, the button and the levels, unlike that one — this
        // is a menu over a game that is still running, not a machine about to stop, so the
        // device's own keys keep working while it is up.
        if self.game_menu.is_some() {
            return self.game_menu_input(action);
        }
        let now = self.now();
        // Ahead of the match that borrows the phase: the card's limit is a fact about how many
        // rows are on it, not about the press that is being answered.
        let about_max = self.about_scroll_max();
        match self.phase {
            Phase::Shelf => match action {
                // START rather than SELECT, and the difference is not cosmetic. SELECT is
                // the chord key: held, it turns Up/Down into brightness and Left/Right into
                // blue light, and `adjust` answers those on every screen including this one.
                // Opening a menu the instant SELECT goes down would eat the first half of
                // every one of those chords; waiting out the 600 ms window instead would put
                // that delay in front of the menu. START is bound to nothing here and reaches
                // no core from the shelf, so it costs neither.
                Action::GbaDown(Btn::Start) if self.core_picker.is_none() => {
                    self.open_core_picker()
                }
                // Colour correction (SELECT+Y) still applies here, because it recolours the
                // carts themselves. The panel mask (SELECT+X) and the screen overlay
                // (SELECT+R2) are the game panel's alone and are answered in `Phase::Playing`
                // only — on the shelf they do nothing. The shelf cycle is a hold of Y now,
                // spent in `sys_hold`, not a chord.
                Action::ColorCycle if self.core_picker.is_none() => self.cycle_cc(),
                // SELECT+VOL, and the shelf is the only screen that answers it: the change
                // reopens the audio device, which is free here and a gap in the sound anywhere
                // else. See `Action::AudioProfileNext`.
                Action::AudioProfileNext if self.core_picker.is_none() => self.cycle_audio(true),
                Action::AudioProfilePrev if self.core_picker.is_none() => self.cycle_audio(false),
                // SELECT+START. The shelf, like the audio profile and for a weaker reason: the
                // case and the index are what the mode reprints, and both of them are here. It
                // is answered on this screen and nowhere else, so the press does nothing under
                // the picker's open lid — where the band is not drawn either.
                Action::ModeToggle if self.core_picker.is_none() => self.toggle_mode(),
                // Ahead of the shelf's own movement, so an open picker takes the arrows
                // before the row of carts underneath it does.
                _ if self.core_picker.is_some() => self.core_picker_input(action),
                Action::ShelfLeft | Action::GbaDown(Btn::Left) => self.shelf.hold_left(now),
                Action::ShelfRight | Action::GbaDown(Btn::Right) => self.shelf.hold_right(now),
                // L and R are the whole of the index, and the only pair of keys that could be:
                // they are either end of the strip printed across the top of the case, and on
                // their own they are the GBA's own shoulders with no core under this screen to
                // want them. The arrows are the *row's* — left and right walk the carts and up
                // and down mean nothing up there — and SELECT turns up and down into
                // brightness, which `adjust` answers before any of this is reached.
                Action::GbaDown(Btn::L1) => self.step_letters(-1, now),
                Action::GbaDown(Btn::R1) => self.step_letters(1, now),
                Action::GbaUp(Btn::L1) => self.letters.release(-1),
                Action::GbaUp(Btn::R1) => self.letters.release(1),
                Action::OpenAbout => {
                    self.phase = Phase::About {
                        scroll: 0.0,
                        want: 0.0,
                    }
                }
                // A is two actions and the press cannot tell them apart yet, so the cart
                // goes in on the release. The hold has already taken it if it got there
                // first, and then the release is not a second press.
                Action::GbaDown(Btn::A) => self.play_held = Some(now),
                Action::GbaUp(Btn::A) => {
                    if self.play_held.take().is_some() {
                        self.insert(false);
                    }
                }
                // X, the screen's other two-gesture button, on the same mechanism as A's: the
                // press is remembered, the release decides which gesture it was, and the hold
                // has already spent the press if it got to the threshold first. A short press
                // stars the cart; a hold swaps the shelf it is shown on.
                Action::GbaDown(Btn::X) => self.fav_held = Some(now),
                Action::GbaUp(Btn::X) => {
                    if self.fav_held.take().is_some() {
                        self.toggle_favorite();
                    }
                }
                // Y holds to swap which machine's shelf is up. Nothing fires on the press: the
                // hold is spent in `sys_hold` on the timer and the release only clears.
                Action::GbaDown(Btn::Y) => self.sys_held = Some(now),
                Action::GbaUp(Btn::Y) => self.sys_held = None,
                // (长按 B 起 WiFi 直连的入口已按用户要求移除——功能暂缓。)
                Action::Insert => self.insert(false),
                _ => {}
            },
            // Eject reaches an insert as well, so a cart whose core never arrived can still
            // be got out. Nothing else here applies until there is a game.
            Phase::Inserting { .. } if action == Action::Eject => self.eject(),
            Phase::Playing { .. } => match action {
                Action::Eject => self.eject(),
                // SELECT+X. The chord is the same on both machines and it means the thing the
                // machine can actually use: on a Game Boy the grid is fixed at LCD3X 50% (see
                // `display_mask`) and the key is the way through the models and the card's own
                // screen art, which is the choice that matters when the picture has four shades
                // and the panel around it is the rest of the look.
                Action::MaskCycle if self.current_machine() == System::Gb => self.overlay_next(),
                Action::MaskCycle => self.cycle_mask(),
                Action::ColorCycle => self.cycle_cc(),
                // The long half of the same key. Held, SELECT+Y opens the browser; tapped, it
                // walks the page. Only a Game Boy has a table long enough to need browsing, and
                // the shelf has no palette on screen to change, so the hold lands nowhere else.
                Action::ColorHold if self.current_machine() == System::Gb => {
                    self.open_palette_browser()
                }
                // The overlay is on screen here, so this is where rotating it is *seen*; the
                // choice is written to the card all the same.
                Action::OverlayNext => self.overlay_next(),
                Action::GameMenu => self.open_game_menu(),
                Action::Polaroids => self.open_polaroids(),
                Action::SaveState => self.save_state(),
                Action::LoadState => self.load_newest(),
                // Rewinding interrupts communication libretro's contract says must not be
                // interrupted. Declined the same way every other "nothing doing" action in
                // this file is, so the press reads as answered rather than dropped.
                Action::RewindStart if !self.may_rewind() => self.refuse(),
                // Fast forward is the same interruption run forwards. `Session::sync_speed`
                // is what actually withholds `Speed::Fast` for as long as `may_fast_forward`
                // says no — this is only the shake, so the press reads as answered.
                Action::FfStart if !self.may_fast_forward() => self.refuse(),
                _ => {}
            },
            Phase::Polaroids { .. } => match action {
                Action::ShelfLeft | Action::GbaDown(Btn::Left) => self.flick(Polaroids::left),
                Action::ShelfRight | Action::GbaDown(Btn::Right) => self.flick(Polaroids::right),
                Action::GbaDown(Btn::A) => self.load_selected(),
                Action::GbaDown(Btn::B) | Action::Polaroids => self.close_polaroids(),
                // The offer lives on this screen and nowhere else. X and Y are free
                // everywhere: the GBA has neither, so the game can never want them.
                Action::GbaDown(Btn::X) => self.undo(self.now()),
                Action::GbaDown(Btn::Y) => self.delete_selected(),
                _ => {}
            },
            // MENU closes it as well as opening it, so the button that got you here gets you
            // back without having to know that B also works. The arrows belong to the card
            // while it is up: it is the only screen with more on it than fits, so it is the one
            // screen where up and down have somewhere to go.
            Phase::About { scroll, want } => match action {
                // Page, do not nudge: the card is a list you travel, not a cursor you
                // steer. Setting the phase rather than poking a field keeps the eased
                // scroll and the wanted scroll in the same place as the rest of the state.
                Action::GbaDown(Btn::Up) => {
                    let w = (want - ABOUT_PAGE).max(0.0);
                    self.phase = Phase::About { scroll, want: w };
                }
                Action::GbaDown(Btn::Down) => {
                    let w = (want + ABOUT_PAGE).min(about_max);
                    self.phase = Phase::About { scroll, want: w };
                }
                _ if action == Action::GbaDown(Btn::B) || action == Action::OpenAbout => {
                    self.phase = Phase::Shelf
                }
                _ => {}
            },
            _ => {}
        }
    }

    /// Applied at a stated moment rather than at whatever the accumulated clock has reached.
    /// The clock is set rather than advanced: a caller that says when something happened is
    /// stating the whole timeline, not adding to one.
    pub fn apply_at(&mut self, action: Action, now: Millis) {
        self.clock = now as f64;
        self.apply(action);
    }

    /// `true` if the action was one of the three levels, whether or not it moved. A press
    /// that hits an end still shows the bar, which is how the end announces itself.
    fn adjust(&mut self, action: Action) -> bool {
        if action == Action::MuteToggle {
            self.mute_toggle();
            return true;
        }
        let s = &self.state;
        let (kind, value) = match action {
            Action::BrightnessUp => (HudKind::Brightness, up(s.brightness, 1, BRIGHTNESS_MAX)),
            Action::BrightnessDown => (HudKind::Brightness, s.brightness.saturating_sub(1)),
            Action::BlueLightUp => (HudKind::BlueLight, up(s.blue_light, 1, BLUE_LIGHT_MAX)),
            Action::BlueLightDown => (HudKind::BlueLight, s.blue_light.saturating_sub(1)),
            Action::VolumeUp => (HudKind::Volume, up(s.volume, VOLUME_STEP, VOLUME_MAX)),
            Action::VolumeDown => (HudKind::Volume, s.volume.saturating_sub(VOLUME_STEP)),
            _ => return false,
        };
        if kind == HudKind::Volume {
            self.remember_volume();
        }
        let level = match kind {
            HudKind::Brightness => &mut self.state.brightness,
            HudKind::BlueLight => &mut self.state.blue_light,
            HudKind::Volume => &mut self.state.volume,
            // The bar is shared with rewind, which is not a level and is never an action.
            HudKind::Rewind => return false,
        };
        let moved = *level != value;
        *level = value;
        // Turning it up or down is the plainest way to say you want to hear it again.
        let unmuted = kind == HudKind::Volume && std::mem::take(&mut self.state.muted);
        let (shown, now) = (self.hud_value(kind, value), self.now());
        self.hud.show(kind, shown, self.state.muted, now);
        if let (HudKind::Brightness, Some(power)) = (kind, &mut self.power) {
            power.set_backlight(value);
        }
        // A key held against an end would otherwise rewrite the file at the repeat rate.
        if moved || unmuted {
            self.persist();
        }
        true
    }

    /// Where the volume stood before the press about to happen. Only the last two are kept:
    /// a chord is two presses, and anything older belongs to a gesture that already ended.
    fn remember_volume(&mut self) {
        if self.vol_before.len() == 2 {
            self.vol_before.remove(0);
        }
        self.vol_before
            .push((self.state.volume, self.state.muted, self.now()));
    }

    /// Silence is a state rather than a level, so muting neither moves the number nor is
    /// moved by the two presses that asked for it. Both keys fire their own adjustment on
    /// the way to the chord, and from an end those two do not cancel.
    fn mute_toggle(&mut self) {
        let now = self.now();
        if let Some((volume, muted, _)) = self
            .vol_before
            .iter()
            .find(|(_, _, at)| now.saturating_sub(*at) <= MUTE_CHORD_MS)
            .copied()
        {
            self.state.volume = volume;
            self.state.muted = muted;
        }
        self.vol_before.clear();
        self.state.muted = !self.state.muted;
        self.hud
            .show(HudKind::Volume, self.output_volume(), self.state.muted, now);
        self.persist();
    }

    /// What the bar reads. Muted draws as an empty bar under the muted glyph, which is what
    /// zero already looks like and is what it already means.
    fn hud_value(&self, kind: HudKind, value: u8) -> u8 {
        match kind {
            HudKind::Volume => self.output_volume(),
            _ => value,
        }
    }

    /// Pushed from outside because only the emulator knows how much history is left. Held
    /// open until `hide_rewind`, unlike the levels.
    pub fn show_rewind(&mut self, fill: u8) {
        let now = self.now();
        // Rewind is not a level and cannot be silenced, so it is never the muted glyph.
        self.hud.show(HudKind::Rewind, fill, false, now);
    }

    pub fn hide_rewind(&mut self) {
        self.hud.release_rewind();
    }

    /// Pushed from outside for the same reason the rewind fill is: held and latched are one
    /// action apiece to the app and two different things on screen.
    pub fn set_ff(&mut self, ff: FfState) {
        self.hud.set_ff(ff);
    }

    pub fn ff_badge(&self) -> Option<Icon> {
        self.hud.badge()
    }

    pub fn blue_light(&self) -> u8 {
        self.state.blue_light
    }

    /// The level the user chose, which a mute does not touch.
    pub fn volume(&self) -> u8 {
        self.state.volume
    }

    pub fn muted(&self) -> bool {
        self.state.muted
    }

    /// What the sink is actually to be set to. The only one of the two the audio path may
    /// read: a muted device at level 70 is silent, not 70.
    pub fn output_volume(&self) -> u8 {
        if self.state.muted {
            0
        } else {
            self.state.volume
        }
    }

    pub fn hud_icon(&self) -> Icon {
        self.hud.glyph()
    }

    pub fn now(&self) -> Millis {
        self.clock as Millis
    }

    fn flick(&mut self, step: fn(&mut Polaroids)) {
        if let Some(p) = &mut self.polaroids {
            step(p);
        }
    }

    pub fn update(&mut self, dt: f32) {
        self.clock += dt as f64 * 1000.0;
        self.timers();
        // The system-shelf swap, before the phase walk: it advances the beats, swaps the view
        // at the bottom of the ascend, and hands the row its motion for this frame. With no
        // swap in flight it clears the motion, so a still row is laid out as it always was.
        self.update_shelf_switch(dt);
        // A queue poll rather than a syscall, so the frame loop can afford it every frame —
        // which is the whole reason the slow parts of starting a link are on a thread of
        // their own.
        self.poll_link();
        let now = self.now();
        // The cart opens once its board is on the GPU, so a slow build is a pause on the shelf
        // rather than an animation spent before its first frame.
        let ready = self.core_faces_ready();
        if let Some(picker) = &mut self.core_picker {
            if picker.waiting() && (ready || picker.waited(now) >= FACES_WAIT_MS) {
                picker.start(now);
            }
        }
        // The lid is back on, so the shelf is the shelf again.
        if self.core_picker.is_some_and(|p| p.finished(now)) {
            self.core_picker = None;
        }
        // A direction still held as the shelf leaves the screen is not held when it comes
        // back: the row repeats only while it is the thing being looked at.
        if !self.on_shelf() {
            self.shelf.release_hold();
            self.letters.release_hold();
        }
        let mut touched = false;
        let next = match &mut self.phase {
            Phase::Shelf => {
                self.shelf.tick(now);
                self.shelf.update(dt);
                None
            }
            Phase::Inserting {
                cart,
                t,
                core_ready,
                resumed,
                ..
            } => {
                let was = *t;
                *t += dt;
                // Started early enough that the contacts in the clip land on the frame
                // the cart does. A resumed cart never travelled, so it never touched
                // anything.
                let at = SEATED_AT - Sfx::Insert.lead();
                touched = !*resumed && was < at && *t >= at;
                (*t >= INSERT_S && *core_ready).then(|| Phase::Playing {
                    cart: std::mem::take(cart),
                })
            }
            // Two movements, in the order the insert made them: the panel goes out, and only
            // once there is nothing on it does the cart start to travel. A cart sliding out
            // across a live picture is the insert played back with its halves overlapping.
            // The clock only starts once the picture is out, and it starts below zero: the
            // beat before the cart moves is that stretch. The contacts let go as it starts
            // moving, which is neither when the button was held nor while the screen is
            // still going down.
            Phase::Ejecting { t, .. } => {
                if self.screen <= 0.0 {
                    let was = *t;
                    *t += dt;
                    touched = was < 0.0 && *t >= 0.0;
                }
                (*t >= EJECT_S).then_some(Phase::Shelf)
            }
            // The card closes on where the last press asked it to go rather than arriving there
            // between two frames: a page of text that changed while the eye was mid-line is a
            // page that has to be found again. `want` is the press and `scroll` is the picture,
            // which is the same split the letter strip keeps.
            Phase::About { scroll, want } => {
                if (*want - *scroll).abs() < 0.5 {
                    *scroll = *want;
                } else {
                    *scroll += (*want - *scroll) * (ABOUT_EASE * dt).min(1.0);
                }
                None
            }
            _ => None,
        };
        // The strip, after the row has moved rather than as part of it. Out here because it
        // needs the whole of `self` — the repeat reaches the caret, which is the shelf's — and
        // the match above is holding `phase` borrowed. The order within it is the order the
        // three things depend on each other: the repeat may move the marker, the spring may
        // move it further, and the last line puts the marker back on the cart the row is
        // actually showing. That last line is what makes the strip a readout as well as a
        // control — an arrow on the row moves the caret and the strip follows it with nothing
        // telling it to.
        if self.on_shelf() {
            self.repeat_letters(now);
            if let Some(cart) = self.shelf.current_cart() {
                self.letters.centre_on(cart.initial);
            }
            self.letters.settle(dt);
        }
        // One clip for the whole movement, and the only thing done to it is when it starts.
        if touched {
            self.sfx = Some(match self.phase {
                Phase::Ejecting { .. } => Sfx::Eject,
                _ => Sfx::Insert,
            });
        }
        if let Some(phase) = next {
            // Ahead of the screen step, so the frame the cart finishes arriving is already
            // the first frame of the power on rather than one more frame of nothing.
            self.phase = phase;
            // The cart is out. Whatever it was carrying goes with it.
            self.refused_from = None;
            // `slot.state` mirrors the slot, so it changes where the phase does: seated on
            // the way in, empty on the way back to the shelf whether that was an eject or
            // a refusal.
            let seated = match &self.phase {
                Phase::Playing { cart } => Some(cart.clone()),
                _ => None,
            };
            self.record_cart(seated);
        }
        self.step_screen(dt);
    }

    /// The game layer's own power, which answers to the phase rather than to an event: an
    /// insert, a resume and a wake all bring the picture up the same way.
    fn step_screen(&mut self, dt: f32) {
        let lit = matches!(self.phase, Phase::Playing { .. } | Phase::Polaroids { .. });
        let step = if lit {
            dt / POWER_ON_S
        } else {
            -dt / POWER_OFF_S
        };
        self.screen = (self.screen + step).clamp(0.0, 1.0);
    }

    /// 0.0 dark, 1.0 fully on. The compositor scales and brightens the game layer by it, and
    /// nothing may draw the game at all while it is zero.
    pub fn screen_power(&self) -> f32 {
        self.screen
    }

    pub fn set_game_ready(&mut self, ready: bool) {
        self.game_ready = ready;
    }

    /// Whether the draw list carries the game layer. A core that has published nothing would
    /// otherwise show the last cart's final frame for the length of the insert.
    pub fn game_visible(&self) -> bool {
        self.game_ready && self.screen > 0.0
    }

    /// Jumps the clock without advancing an animation. The autosave and the doze timeout
    /// are minutes apart, which is further than a test wants to walk a frame at a time.
    pub fn tick_ms(&mut self, now: Millis) {
        self.clock = self.clock.max(now as f64);
        self.timers();
    }

    /// Everything the clock alone drives. The play hold is the one thing here the user did
    /// ask for; it is only the clock that decides which of the two things it was.
    fn timers(&mut self) {
        self.play_hold();
        self.fav_hold();
        self.sys_hold();
        // The "connect the cable" beat hands over to the live screen on its own, a moment after
        // the gadget has been switched.
        // While the connect beat is up, poll for the address the enter script leaves behind: the
        // beat hands over to the live screen the moment one appears, however long the join took.
        if self.direct_link == Some(1) && self.now().saturating_sub(self.direct_at) >= 1000 {
            self.direct_at = self.now();
            if let Ok(ip) = std::fs::read_to_string("/tmp/slot-ip") {
                let ip = ip.trim().to_string();
                if !ip.is_empty() {
                    self.direct_ip = Some(ip);
                    self.direct_ip_dirty = true;
                    self.direct_link = Some(2);
                }
            }
        }
        // The grace period can run out with the switcher open, so the hint answers to the
        // clock rather than to whatever was on offer on the way in.
        let offer = self.undo_label();
        if let Some(p) = &mut self.polaroids {
            p.set_undo(offer);
        }
        if self.doze_expired() {
            self.on_doze_timeout();
        }
        if self.now() >= self.autosave_at {
            self.flush_resume();
        }
        if self.now() >= self.battery_at {
            self.battery_at = self.now() + BATTERY_POLL_MS;
            self.battery = self.power.as_ref().and_then(|p| p.battery());
            if let Some(b) = self.battery {
                self.on_battery(b);
            }
        }
        // Only the charge half. The percent it is written beside is at most one slow tick
        // old, which is the staleness the slow tick was always chosen for.
        if self.now() >= self.charge_at {
            self.charge_at = self.now() + CHARGE_POLL_MS;
            if let (Some(power), Some(b)) = (self.power.as_ref(), self.battery.as_mut()) {
                b.charge = power.charge();
            }
            // On the fast tick rather than the slow one: an amber-on-plug-in that lags ten
            // seconds behind the cable is worse than no LED at all.
            let state = self.led_state();
            self.set_led(state);
        }
    }

    /// Modelled on the OG SP: green running, red low, amber charging, green once it is full.
    /// Charging outranks low, since a flat device on a cable is filling rather than dying.
    pub fn led_state(&self) -> LedState {
        let Some(b) = self.battery else {
            return LedState::Running;
        };
        match b.charge {
            Charge::Charging => LedState::Charging,
            Charge::Full => LedState::Charged,
            _ if b.percent <= BATTERY_LOW => LedState::Low,
            _ => LedState::Running,
        }
    }

    /// The one place that ever reaches the platform's own `set_led`, so the edge kept in
    /// `last_led` cannot be bypassed by a call site that forgot it. Called every second with
    /// whatever `led_state` just computed, so on any device that never asserts a charge state
    /// this is the only branch pair — `Low` and `Running` — a write ever leaves this function
    /// with; a state repeated from the previous second returns before touching the platform.
    fn set_led(&mut self, state: LedState) {
        // A shutdown darkens the case and nothing lights it again. The fast tick recomputes
        // `led_state` from the gauge every second and knows nothing about a shutdown in
        // progress, so a charge tick landing inside the window between the choice and
        // `poweroff` put the light straight back to green for the five seconds rcK takes.
        // Guarded here rather than at that call site for the same reason the edge is: this is
        // the one seam, and a caller cannot forget what it never has to remember.
        if self.shutting_down() && state != LedState::Off {
            return;
        }
        if self.last_led == Some(state) {
            return;
        }
        self.last_led = Some(state);
        if let Some(power) = self.power.as_mut() {
            power.set_led(state);
        }
    }

    fn record_cart(&mut self, cart: Option<String>) {
        if self.state.cart == cart {
            return;
        }
        self.state.cart = cart;
        self.persist();
    }

    fn persist(&self) {
        let Some(root) = &self.root else {
            return;
        };
        if let Err(e) = write_slot_state(root, &self.state) {
            eprintln!("slot: slot.state: {e}");
        }
    }

    pub fn on_core_ready(&mut self) {
        if let Phase::Inserting { core_ready, .. } = &mut self.phase {
            *core_ready = true;
        }
    }

    pub fn on_core_failed(&mut self) {
        let caught = self.seat();
        let Phase::Inserting { cart, .. } = &mut self.phase else {
            return;
        };
        let cart = std::mem::take(cart);
        // Resumed at the depth it caught rather than at zero, so the refusal reads as one
        // movement instead of a jump to seated and back out.
        let t = (1.0 - caught) * EJECT_S;
        self.phase = Phase::Ejecting { cart, t };
        // No shake here. The cart is on screen and carries the alert instead, and a screen
        // that flinched as well would read as two separate failures.
        self.refused_from = Some(t);
    }

    /// Any action the app will not carry out. There are no words for it and no state to
    /// clear: it decays on its own clock, wherever it is being drawn.
    pub fn refuse(&mut self) {
        self.refusal = Some(Refusal::started(self.now()));
    }

    pub fn refusal_active(&self, now: Millis) -> bool {
        self.refusal.is_some_and(|r| r.active(now))
    }

    /// How far the cart is into the slot: 0.0 standing on the shelf, 1.0 swallowed.
    pub fn seat(&self) -> f32 {
        match &self.phase {
            Phase::Shelf => 0.0,
            Phase::Inserting { t, resumed, .. } => {
                if *resumed {
                    1.0
                } else {
                    (t / SEATED_AT).clamp(0.0, 1.0)
                }
            }
            Phase::Ejecting { t, .. } => 1.0 - (t / EJECT_S).clamp(0.0, 1.0),
            _ => 1.0,
        }
    }

    pub fn draw(&self, out: &mut Vec<Draw>) {
        // Ahead of every phase, because a shutdown is not a screen the user navigated to.
        // rcK takes about five seconds on this hardware — it stops the frontend and unloads
        // the GPU module before the kernel is allowed to halt — and five seconds of black
        // panel after holding the button is indistinguishable from a device that has hung.
        if let Some(index) = self.power_menu {
            self.draw_power_menu(index, out);
            return;
        }
        // (WiFi 直连对话框的绘制已移除——功能暂缓。)
        if let Some(index) = self.cheat_menu {
            self.draw_cheat_menu(index, out);
            return;
        }
        if self.shutting_down() {
            out.push(Draw::Rect {
                x: 0.0,
                y: 0.0,
                w: OUT_W as f32,
                h: OUT_H as f32,
                colour: [0.0, 0.0, 0.0, 1.0],
            });
            // The row the user picked is the row the screen repeats back.
            let which = if self.restarting {
                PowerChoice::Restart
            } else {
                PowerChoice::PowerOff
            };
            if let Some((tex, w, h)) = self.shutdown_faces.get(which.index()).copied() {
                out.push(Draw::Tex {
                    x: ((OUT_W - w) / 2) as f32,
                    y: ((OUT_H - h) / 2) as f32,
                    w: w as f32,
                    h: h as f32,
                    tex,
                    alpha: 1.0,
                });
            }
            return;
        }
        // Whether the letter band went down this frame. The HUD reads it to place its plate: the
        // band is only on the shelf, and it is the one phase where the top of the screen is not
        // free for a plate to sit against.
        let mut band_on = false;
        match &self.phase {
            // Nothing else is on screen and nothing goes over it, the HUD included: the
            // levels are unreachable here and there is no game to say anything about.
            Phase::SetClock { picker } => {
                let (line, hint) = match self.clock_faces {
                    Some((line, hint)) => (Some(line), Some(hint)),
                    None => (None, None),
                };
                picker.draw(line, hint, out);
                return;
            }
            Phase::Shelf => {
                draw_backdrop(self.wallpaper, out);
                match (self.core_picker_shown(), self.selected_stem()) {
                    // The highlighted cart is the picker's to draw while its lid is off, and the
                    // rest of the row makes way for it the way it does for a cart going in.
                    // Until its faces are up the picker has only bare parts, so the cart stands.
                    (Some(picker), Some(stem)) => {
                        // Eased on the whole progress rather than on either beat: the row makes
                        // way across the slide and the lift as one movement.
                        let open = ease(picker.openness(self.now()));
                        // Dimmed by as much of the open as has happened, so the dark arrives
                        // with the lid coming off and leaves with it going back on.
                        let dim = 1.0 + (CORE_PICKER_DIM - 1.0) * open;
                        self.shelf
                            .draw_row(Some(stem), 0.0, CORE_PICKER_RECEDE * open, dim, out);
                        draw_empty_slot(out);
                    }
                    _ => {
                        self.shelf.draw(self.shelf_shake(), out);
                        // The stars, over the row and under the case's own furniture: a starred
                        // cart wears one above it, and the shelf's own star sits in the corner
                        // saying which shelf is up.
                        self.draw_fav_stars(out);
                        // The band at the top of the case, and the index printed along it. The
                        // band is the machine's — the cart bay's mirror, drawn from the bay's
                        // own numbers — so it goes down as furniture and the letters go over
                        // it. Not drawn while the picker has the lid off: the index says where
                        // in the library the caret is, and with a cart open there is no
                        // library on screen for it to be somewhere in.
                        draw_top_band(out);
                        // The light off both bands of the case, drawn with the band because the
                        // two boundaries are the same object seen at the top and the bottom of
                        // the screen. After the row, so a cart on its way into the slot is lit
                        // by it rather than drawn over it.
                        draw_edge_glow(out);
                        band_on = true;
                        // The letter strip is the library's index, and the favourites shelf is
                        // not the library: the dial has stood down, so the band is left with an
                        // empty window rather than an index that no longer means anything.
                        if !self.fav_view {
                            self.letters.draw_strip(
                                &self.letter_faces,
                                self.letter_ridge_face,
                                &self.letter_counts,
                                out,
                            );
                        }
                        self.draw_fav_indicator(out);
                        // Which machine's shelf is up, in the opposite bottom corner — the
                        // mirror of the favourites star.
                        self.draw_shelf_indicator(out);
                    }
                }
                // After the row and before the case: it names what the row is showing, so it
                // belongs to the gap the row leaves rather than to the plastic below it.
                if let Some((tex, w, _)) = self.shelf_title_face {
                    out.push(Draw::Tex {
                        x: (OUT_W as f32 - w as f32) / 2.0,
                        y: self.shelf_title_y(),
                        w: w as f32,
                        h: SHELF_TITLE_H as f32,
                        tex,
                        alpha: 1.0,
                    });
                }
                // Last of the shelf's own furniture, because it is printed on the band the
                // letters are cut into and belongs over it: the clock at one end of the case
                // and the battery at the other, both clear of the window between them.
                draw_status(
                    self.battery,
                    self.battery_percent,
                    self.bolt,
                    self.shelf_clock,
                    self.hud.face(Icon::of_mode(slot_ui::palette::mode())),
                    out,
                );
            }
            Phase::About { scroll, .. } => {
                // The same ground the shelf stands on, scrim and all. The label is a dark
                // object and the scrim is what a dark object needs to read over a
                // photograph — it is there for the carts for exactly the same reason.
                draw_backdrop(self.wallpaper, out);
                // One card, one scroll: the label goes up with the rows under it rather than
                // staying in the middle of the panel while the list moves behind it.
                draw_sticker_at(self.sticker_face, ABOUT_PAD - *scroll, out);
                let mut y = about_first_y() - *scroll;
                for face in self.shortcut_rows.iter().copied() {
                    // Off the panel is off the panel: twenty rows of which five are ever on
                    // screen is not a reason to hand the compositor twenty.
                    if y < OUT_H as f32 && y + face.2 as f32 > 0.0 {
                        draw_shortcut_row(Some(face), y, out);
                    }
                    y += face.2 as f32;
                }
                // Pinned. It is the line that says the rest of the card moves, so it cannot be
                // the thing that moves.
                if let Some((_, _, h)) = self.shortcut_hint {
                    // The band, then the line on it. The band is the panel's and the line is the
                    // card's, so they are laid from two different widths.
                    let top = OUT_H as f32 - h as f32;
                    draw_hint_bar(top, out);
                    draw_shortcut_row(self.shortcut_hint, top, out);
                }
                return;
            }
            // The shelf recedes behind the cart on the way in; on the way out the live
            // game is what darkens, and the compositor has already drawn it.
            Phase::Inserting { cart, resumed, .. } => {
                // Spec section 3: a resumed cart shows no shelf, not even one frame of it.
                if !resumed {
                    draw_backdrop(self.wallpaper, out);
                    self.shelf.draw_row(Some(cart), 0.0, self.seat(), 1.0, out);
                }
                self.chrome(cart, self.seat(), out);
            }
            // The insert run backwards, all of it: the veil lifts, the row closes back up
            // and the cart comes out, every one of them off the same progress running the
            // other way. Darkening on the way out as well as on the way in was the screen
            // playing the same movement twice rather than reversing it.
            Phase::Ejecting { cart, .. } => {
                draw_backdrop(self.wallpaper, out);
                self.shelf.draw_row(Some(cart), 0.0, self.seat(), 1.0, out);
                self.chrome(cart, self.seat(), out);
            }
            // The slot stays on screen until the picture behind it has finished arriving,
            // so the game blooms out of a lit lip rather than replacing it.
            Phase::Playing { cart } if self.screen < 1.0 => self.chrome(cart, 0.0, out),
            Phase::Playing { .. } => {
                self.push_game(out);
                self.draw_palette_browser(out);
            }
            // The paused game stays underneath, covered by the screenshot the switcher
            // draws over the whole screen.
            Phase::Polaroids { .. } => {
                self.push_game(out);
                if let Some(p) = &self.polaroids {
                    p.draw(
                        self.battery,
                        self.battery_percent,
                        self.bolt,
                        self.shelf_clock,
                        out,
                    );
                }
            }
            // The device answers a shut lid with the backlight; the host has no panel to
            // darken, so the doze is drawn. Nothing goes over it, the HUD included.
            Phase::Doze { .. } => {
                out.push(Draw::Rect {
                    x: 0.0,
                    y: 0.0,
                    w: OUT_W as f32,
                    h: OUT_H as f32,
                    colour: [0.0, 0.0, 0.0, 1.0],
                });
                return;
            }
        }
        // After the shelf, never before it: drawn first it would be painted over by the very
        // row of carts it is a menu for, and START would look like a button that does
        // nothing. Only the shelf can raise it, so no phase needs excluding here — the
        // phases that own the whole panel have already returned.
        if let Some(picker) = self.core_picker_shown() {
            self.draw_core_picker(&picker, out);
        }
        // Over the game and under the HUD, for the same reason the picker is over the shelf:
        // it is a menu about the thing still on screen behind it, and the level bars have to
        // stay visible while it is up. Only a running game can raise it, so no phase needs
        // excluding here — the ones that own the whole panel have already returned.
        if let Some(menu) = self.game_menu {
            self.draw_game_menu(menu, out);
        }
        // Over everything, in every phase. The bar is never what the user is looking at.
        //
        // The plate hangs under the letter band on the shelf and from the top of the screen
        // everywhere else, which is the one thing about the HUD a phase gets to decide: the band
        // is only on the shelf, and covering it with a plate would hide the index and the clock
        // for the second and a half a level is up.
        let plate_top = if band_on { PLATE_Y } else { 0.0 };
        self.hud.draw(self.now(), plate_top, out);
    }

    pub fn screen_shake(&self) -> f32 {
        self.shake_at(self.now())
    }

    /// Offscreen pixels the whole presented image is displaced by. The screen flinches only
    /// while the game is playing, which is the one phase whose content fills the frame.
    pub fn shake_at(&self, now: Millis) -> f32 {
        self.shake_when(matches!(self.phase, Phase::Playing { .. }), now)
    }

    /// Pixels the cart row is displaced by. On the shelf the frame is mostly backdrop, so
    /// shaking the whole image would just slide the letterbox in at the edges.
    pub fn shelf_shake(&self) -> f32 {
        // The chip is what flinches while the picker is up, and two things shaking at once reads
        // as two separate refusals.
        self.shake_when(self.on_shelf() && self.core_picker.is_none(), self.now())
    }

    /// Shake whatever represents the thing that was refused, and only that: two of them at
    /// once reads as two separate failures.
    fn shake_when(&self, mine: bool, now: Millis) -> f32 {
        if !mine {
            return 0.0;
        }
        self.refusal.map_or(0.0, |r| r.offset(now))
    }

    /// The game layer's place in the list. Where there is no slot on screen it is the whole
    /// picture; the chrome puts it in the same list, in front of the cart.
    fn push_game(&self, out: &mut Vec<Draw>) {
        if self.game_visible() {
            // A tinted-backlight colour correction makes a GB picture glow one colour; the
            // letterbox around it would otherwise stay black and read as a frame that does not
            // belong. Fill it with the same colour, so the whole panel is one tinted surface.
            //
            // **Plain GB only.** GBC is a colour machine whose games use their own palettes, so
            // its surround keeps the black letterbox — a mono tint there would be a lie about
            // the machine, and GBC is themed through an overlay PNG instead. A GBA picture
            // fills the frame, so it has no letterbox to fill either. And only modes 3..=6 have
            // a single colour to fill with.
            if matches!(
                self.shelf.current_cart().map(|c| c.system()),
                Some(System::Gb)
            ) {
                if let Some(c) = self.display.cc_border(self.current_machine()) {
                    out.push(Draw::Rect {
                        x: 0.0,
                        y: 0.0,
                        w: OUT_W as f32,
                        h: OUT_H as f32,
                        colour: [c[0], c[1], c[2], 1.0],
                    });
                }
            }
            out.push(Draw::Game);
            // Over the game and under the HUD: a per-system screen overlay PNG from the card,
            // composited with its own alpha. Opaque pixels replace the picture (bezel, grid),
            // transparent ones let it through. No texture means the built-in look.
            if let Some(tex) = self.overlay_tex {
                out.push(Draw::Tex {
                    x: 0.0,
                    y: 0.0,
                    w: OUT_W as f32,
                    h: OUT_H as f32,
                    tex,
                    alpha: 1.0,
                });
            }
            // ...and the glow over the art, not under it: the art is the plastic's colour and
            // the glow is light landing on that plastic. Push it whether or not the card has an
            // overlay — the compositor is what knows whether anything reflective is up, and a
            // marker that costs one comparison is cheaper than asking the same question twice.
            out.push(Draw::Glow);
        }
    }

    fn chrome(&self, stem: &str, dim: f32, out: &mut Vec<Draw>) {
        let Some((cart, face)) = self.shelf.find(stem) else {
            return;
        };
        let alpha = self.alert_alpha();
        SlotChrome {
            cart,
            face,
            seat: self.seat(),
            alert: self.alert_face.filter(|_| alpha > 0.0).map(|t| (t, alpha)),
            dim,
            screen: self.screen,
            game: self.game_ready,
        }
        .draw(out);
    }

    /// Whether the cart on its way back out is carrying the refusal symbol.
    pub fn alert_visible(&self) -> bool {
        self.alert_alpha() > 0.0
    }

    /// How lit that symbol is. It holds for most of the exit and is gone before the end of
    /// it, so the alert leaves with the cart rather than being cut off by the shelf.
    pub fn alert_alpha(&self) -> f32 {
        let (Some(from), Phase::Ejecting { t, .. }) = (self.refused_from, &self.phase) else {
            return 0.0;
        };
        let span = EJECT_S - from;
        if span <= 0.0 {
            return 0.0;
        }
        let u = ((t - from) / span).clamp(0.0, 1.0);
        ((ALERT_GONE - u) / (ALERT_GONE - ALERT_HOLD)).clamp(0.0, 1.0)
    }

    pub fn set_shelf_title_face(&mut self, face: Option<(TexId, u32, u32)>) {
        self.shelf_title_face = face;
    }

    pub fn set_alert_face(&mut self, face: TexId) {
        self.alert_face = Some(face);
    }

    pub fn set_shutdown_faces(&mut self, faces: Vec<(TexId, u32, u32)>) {
        self.shutdown_faces = faces;
    }

    pub fn set_power_menu_faces(&mut self, faces: Vec<(TexId, u32, u32)>) {
        self.power_menu_faces = faces;
    }

    /// Recorded against the highlighted cart, since that is the only cart they are ever built for.
    pub fn set_core_board_faces(&mut self, board: TexId, lid: TexId) {
        self.core_board_face = Some(board);
        self.core_lid_face = Some(lid);
        self.core_faces_stem = self.selected_stem().map(str::to_string);
    }

    /// `sockets` and `chips` in `Core::ALL` order.
    pub fn set_core_part_faces(
        &mut self,
        sockets: Vec<TexId>,
        chips: Vec<TexId>,
        blank: TexId,
        shadow: TexId,
    ) {
        self.core_socket_faces = sockets;
        self.core_chip_faces = chips;
        self.core_blank_chip_face = Some(blank);
        self.core_chip_shadow_face = Some(shadow);
    }

    pub fn set_core_legend_faces(&mut self, faces: Vec<(TexId, u32)>) {
        self.core_legend_faces = faces;
    }

    /// One per `GameRow::ALL`, in that order, whether or not the cart in the slot shows it.
    pub fn set_game_menu_faces(&mut self, faces: Vec<(TexId, u32, u32)>) {
        self.game_menu_faces = faces;
    }

    /// One per `LinkRow::ALL`, in that order.
    pub fn set_link_menu_faces(&mut self, faces: Vec<(TexId, u32, u32)>) {
        self.link_menu_faces = faces;
    }

    /// One per `LinkStep::ALL`, and one per `LinkFail::SHOWN`, in those orders. Uploaded at
    /// boot with every other menu face: a link that is failing is the worst moment to be
    /// asking a font for a sentence.
    pub fn set_link_step_faces(&mut self, faces: Vec<(TexId, u32, u32)>) {
        self.link_step_faces = faces;
    }

    pub fn set_link_fail_faces(&mut self, faces: Vec<(TexId, u32, u32)>) {
        self.link_fail_faces = faces;
    }

    /// The case's own ground, and the rows on it. No plate behind them: the menu is three
    /// words and a choice, and a box around those was furniture the screen did not need.
    ///
    /// The ground is drawn here rather than in `draw_menu_rows` because it is the only thing
    /// about this menu that is its own: the core picker draws over a shelf it did not paint,
    /// and the in-game menu over a game it did not either. See `draw_menu_rows` for why the
    /// bar behind the row in hand is the colour it is.
    fn draw_power_menu(&self, index: usize, out: &mut Vec<Draw>) {
        out.push(Draw::Rect {
            x: 0.0,
            y: 0.0,
            w: OUT_W as f32,
            h: OUT_H as f32,
            colour: slot_ui::opening(),
        });
        draw_menu_rows(
            &self.power_menu_faces,
            Some(index),
            centred_top(self.power_menu_faces.len()),
            out,
        );
    }

    /// The direct-connect dialog: a scrim over everything, then the lines for its stage centred on
    /// the panel. Stage 0 asks, stage 1 waits for the cable, stage 2 is the live mode.
    fn draw_direct_link(&self, stage: u8, out: &mut Vec<Draw>) {
        out.push(Draw::Rect {
            x: 0.0,
            y: 0.0,
            w: OUT_W as f32,
            h: OUT_H as f32,
            colour: slot_ui::opening(),
        });
        match stage {
            0 => {
                self.draw_centred_line(0, 200.0, out);
                self.draw_centred_line(1, 262.0, out);
            }
            1 => self.draw_centred_line(2, 240.0, out),
            _ => {
                self.draw_centred_line(3, 172.0, out);
                if let Some((tex, w, h)) = self.direct_ip_face {
                    out.push(Draw::Tex {
                        x: (OUT_W as f32 - w as f32) / 2.0,
                        y: 226.0,
                        w: w as f32,
                        h: h as f32,
                        tex,
                        alpha: 1.0,
                    });
                }
                self.draw_centred_line(4, 296.0, out);
            }
        }
    }

    /// One baked dialog line, centred horizontally on the panel at the given vertical centre.
    fn draw_centred_line(&self, i: usize, cy: f32, out: &mut Vec<Draw>) {
        if let Some(&(tex, w, h)) = self.direct_faces.get(i) {
            out.push(Draw::Tex {
                x: (OUT_W as f32 - w as f32) / 2.0,
                y: cy - h as f32 / 2.0,
                w: w as f32,
                h: h as f32,
                tex,
                alpha: 1.0,
            });
        }
    }

    /// The direct-connect lines, uploaded once at boot.
    pub fn set_direct_faces(&mut self, faces: Vec<(TexId, u32, u32)>) {
        self.direct_faces = faces;
    }

    /// Open the direct-connect prompt — a long hold of B on the shelf.
    fn open_direct_link(&mut self) {
        self.direct_link.get_or_insert(0);
    }

    /// A on the dialog: switch the gadget to mass storage. Nothing else happens on this stage.
    fn direct_confirm(&mut self) {
        if self.direct_link == Some(0) {
            self.direct_link = Some(1);
            self.direct_at = self.now();
            self.direct_ip = None;
            self.direct_ip_face = None;
            let _ = std::fs::remove_file("/tmp/slot-ip");
            self.run_direct_script(true);
        }
    }

    /// B on the dialog: cancel the prompt, or leave the live mode (drop the radio).
    fn direct_cancel(&mut self) {
        if self.direct_link == Some(2) {
            self.run_direct_script(false);
        }
        self.direct_link = None;
        self.direct_ip = None;
        self.direct_ip_face = None;
        self.direct_ip_dirty = false;
    }

    /// Run the enter/exit shell detached, so the dialog keeps drawing while the radio comes up.
    fn run_direct_script(&self, enter: bool) {
        let script = if enter { DIRECT_ENTER } else { DIRECT_EXIT };
        let _ = std::process::Command::new("/bin/sh")
            .arg("-c")
            .arg(format!("{} >/dev/null 2>&1", script))
            .spawn();
    }

    /// The address the WiFi link came up on, and the frontend's bake hook for it. The dialog's own
    /// lines are baked once at boot, but this one is whatever DHCP handed out, so the frontend
    /// bakes it when `direct_ip_dirty` says it has arrived.
    pub fn direct_ip_dirty(&self) -> bool {
        self.direct_ip_dirty
    }

    pub fn direct_ip_str(&self) -> Option<&str> {
        self.direct_ip.as_deref()
    }

    pub fn set_direct_ip_face(&mut self, face: Option<(TexId, u32, u32)>) {
        self.direct_ip_face = face;
    }

    pub fn clear_direct_ip_dirty(&mut self) {
        self.direct_ip_dirty = false;
    }

    /// The picker, but only once it has started opening: `None` while it is still standing on
    /// the shelf waiting for this cart's faces, so nothing of it is on screen yet and the row
    /// has not made way for it.
    fn core_picker_shown(&self) -> Option<CorePicker> {
        self.core_picker.filter(|p| !p.waiting())
    }

    /// The open cart over the shelf that is making way for it: the board growing out of the cart
    /// that stood there, both sockets on it, the chip in one of them or in the air between, the
    /// lid slid off it and lifted away with the cart's own face on it, and the legend. Over the
    /// shelf and under the HUD: brightness and blue light are still answered while it is up.
    ///
    /// `ready` is this cart's own board and lid, not merely whatever is on the GPU: a picker
    /// that started on `FACES_WAIT_MS`'s cap has neither yet, and must never wear a build left
    /// over from the cart the caret was on before — showing nothing is the only honest choice
    /// until this cart's own faces land, so the board, the sockets, the chip and the chip's own
    /// shadow wait for `ready` and the lid falls back to the shelf's plain face for this cart.
    fn draw_core_picker(&self, picker: &CorePicker, out: &mut Vec<Draw>) {
        let now = self.now();
        let progress = picker.openness(now);
        // The shadows and the legend come in with the lift, not with the slide.
        let lift = lift_of(progress);
        let board = board_at(progress);
        let zoom = board_zoom(board);
        let ready = self.core_faces_ready();
        // A card that supplies its own board has the chips drawn into the art, so there is
        // nothing for the picker to place: no sockets, no chip, and no shadow for a chip that
        // is not coming across. What is left of the picker on such a card is the two lamps.
        let own_board = slot_ui::board_art().is_some();

        if ready {
            // Opaque from the first frame, and the sockets and the chip with it: the back half
            // was always there under the front, and the slide only uncovers it.
            if let Some(tex) = self.core_board_face {
                out.push(Draw::Tex {
                    x: board.x,
                    y: board.y,
                    w: board.w,
                    h: board.h,
                    tex,
                    alpha: 1.0,
                });
            }
            // The card's board carries its own chips, so the lamps are the whole of what the
            // picker adds: the one the cursor is over lit orange, the other out, and the lit one
            // green once the choice has been made and the picker is on its way out. Nothing is
            // baked into the board texture, because the state changes every press.
            if let Some(art) = slot_ui::board_art() {
                let chip = picker.chip(now);
                let here = usize::from(chip.across >= 0.5);
                let done = picker.closing();
                for (i, lamp) in art.lamps.iter().enumerate() {
                    let colour = if i == here {
                        if done {
                            [0.18, 0.76, 0.37, 1.0]
                        } else {
                            [1.0, 0.55, 0.10, 1.0]
                        }
                    } else {
                        // Out, but not invisible: against the board's own plastic a dark lamp
                        // reads as a hole rather than as a lamp that is off.
                        [0.62, 0.62, 0.65, 1.0]
                    };
                    out.push(Draw::Rect {
                        x: board.x + lamp.x * board.w,
                        y: board.y + lamp.y * board.h,
                        w: lamp.w * board.w,
                        h: lamp.h * board.h,
                        colour,
                    });
                }
            }
            // A face drawn at its own size is only sharp on whole pixels.
            for (i, tex) in self.core_socket_faces.iter().copied().enumerate() {
                let (x, y) = on_board(board, SOCKET_U[i], SOCKET_V);
                out.push(Draw::Tex {
                    x: x.round(),
                    y: y.round(),
                    w: SOCKET_W as f32 * zoom,
                    h: SOCKET_H as f32 * zoom,
                    tex,
                    alpha: 1.0,
                });
            }

            let chip = picker.chip(now);
            let u = CHIP_U[0] + (CHIP_U[1] - CHIP_U[0]) * chip.across;
            if !own_board && chip.lift > 0.0 {
                if let Some(tex) = self.core_chip_shadow_face {
                    // Under the body's middle and 90 units down the board, where the mockup's
                    // oval falls: low enough to read as cast on the board rather than tucked
                    // under the pins.
                    let (cx, cy) = on_board(board, u + 19.0, CHIP_V + 29.4);
                    let (w, h) = (SHADOW_W as f32 * zoom, SHADOW_H as f32 * zoom);
                    out.push(Draw::Tex {
                        x: cx - w / 2.0,
                        y: cy - h / 2.0,
                        w,
                        h,
                        tex,
                        alpha: 0.6 * chip.lift * lift,
                    });
                }
            }
            // The blank stand-in is for a chip in flight, and there is no chip to fly across a
            // board that has its own: drawn anyway it would be a grey chip flashing over a
            // finished board, which is worse than nothing.
            let face = if own_board {
                None
            } else {
                match chip.seated {
                    Some(core) => self.core_chip_faces.get(core.index()).copied(),
                    None => self.core_blank_chip_face,
                }
            };
            if let Some(tex) = face {
                let (x, y) = on_board(board, u, CHIP_V - HOP_LIFT * chip.lift);
                let body = Placed {
                    x: x + chip.shake,
                    y,
                    w: CHIP_W as f32 * zoom,
                    h: CHIP_H as f32 * zoom,
                };
                // Whole pixels, as the sockets: a seated chip is drawn at its own size too.
                let at = grown(body, TURN_PAD as f32 * zoom);
                out.push(Draw::Turned {
                    x: at.x.round(),
                    y: at.y.round(),
                    w: at.w,
                    h: at.h,
                    tex,
                    alpha: 1.0,
                    turn: chip.tip,
                });
            }
        }

        // The soft oval on the ground under the lid. Without it the lid reads as printed on the
        // backdrop rather than held up off the board. The chip's shadow, stretched: it grows
        // with the lid and comes in as the lid rises. Drawn whether or not this cart's faces are
        // ready: the lid is always something, the fallback included, and it always casts one.
        if let Some(tex) = self.core_chip_shadow_face {
            let (lid, _) = lid_at(progress);
            let k = lid.w / lid_at(1.0).0.w;
            let (w, h) = (LID_SHADOW_W * k, LID_SHADOW_H * k);
            out.push(Draw::Tex {
                x: lid.x + (lid.w - w) / 2.0,
                y: lid.y + lid.h + LID_SHADOW_DROP * k - h / 2.0,
                w,
                h,
                tex,
                alpha: LID_SHADOW_ALPHA * lift,
            });
        }

        if ready {
            // Always opaque: at the very start and end of the movement the lid is the cart on
            // the shelf, and a cart there does not fade.
            if let Some(tex) = self.core_lid_face {
                let (lid, turn) = lid_at(progress);
                let at = grown(lid, TURN_PAD as f32 * lid.w / CART_W as f32);
                out.push(Draw::Turned {
                    x: at.x,
                    y: at.y,
                    w: at.w,
                    h: at.h,
                    tex,
                    alpha: 1.0,
                    turn,
                });
            }
        } else if let Some((_, Some(tex))) =
            self.selected_stem().and_then(|stem| self.shelf.find(stem))
        {
            // The cap ran out before this cart's own lid arrived. The shelf's own face for the
            // cart is the only thing left to lift — not `core_lid_face`, which would still be
            // whatever cart the worker built last — and it is drawn unpadded: unlike a face
            // built for the picker, the shelf's face carries no transparent border to grow into.
            let (lid, turn) = lid_at(progress);
            out.push(Draw::Turned {
                x: lid.x,
                y: lid.y,
                w: lid.w,
                h: lid.h,
                tex,
                alpha: 1.0,
                turn,
            });
        }

        // Cancel under the open cart's left edge, Swap centred on the panel, Choose under its
        // right edge, each placed by what shows of it — the key caps and the word — and not by
        // the transparent strip every hint face carries after its label.
        if let [cancel, swap, choose] = self.core_legend_faces.as_slice() {
            let right = BOARD_X + BOARD_W as f32;
            let seen = |w: u32| w.saturating_sub(HINT_EDGE) as f32;
            for (tex, w, x) in [
                (cancel.0, cancel.1, BOARD_X),
                (swap.0, swap.1, (OUT_W as f32 - seen(swap.1)) / 2.0),
                (choose.0, choose.1, right - seen(choose.1)),
            ] {
                out.push(Draw::Tex {
                    x: x.round(),
                    y: CORE_LEGEND_Y,
                    w: w as f32,
                    h: HINT_H as f32,
                    tex,
                    alpha: lift,
                });
            }
        }
    }

    /// The power menu's rows, at its pitch, in its materials — the third menu on the device
    /// and the third to be the same object. What differs is where it goes in the frame and
    /// what it is drawn over: this one goes over the paused game, which is still on screen
    /// behind it and is what the player gets back by cancelling.
    fn draw_game_menu(&self, menu: GameMenu, out: &mut Vec<Draw>) {
        out.push(Draw::Rect {
            x: 0.0,
            y: 0.0,
            w: OUT_W as f32,
            h: OUT_H as f32,
            colour: slot_ui::opening(),
        });
        // The two working screens are a sentence rather than a list, so nothing on them is
        // in hand and nothing is marked.
        let (faces, index) = match menu {
            GameMenu::Rows(index) => (self.game_row_faces(), Some(index)),
            GameMenu::Link(index) => (self.link_menu_faces.clone(), Some(index)),
            GameMenu::Working(step) => (one(&self.link_step_faces, Some(step.index())), None),
            GameMenu::Failed(fail) => (one(&self.link_fail_faces, fail.shown()), None),
        };
        draw_menu_rows(&faces, index, centred_top(faces.len()), out);
    }

    /// A face per row this cart actually shows, in the order they are drawn. The uploads are
    /// one per `GameRow::ALL`, so a row this cart hides is skipped here rather than missing
    /// from the upload.
    fn game_row_faces(&self) -> Vec<(TexId, u32, u32)> {
        self.game_rows()
            .iter()
            .filter_map(|row| self.game_menu_faces.get(row.index()).copied())
            .collect()
    }

    fn on_shelf(&self) -> bool {
        matches!(self.phase, Phase::Shelf)
    }

    fn insert(&mut self, clean: bool) {
        if !self.on_shelf() {
            return;
        }
        let Some(cart) = self.shelf.current_cart().map(|c| c.stem.clone()) else {
            return;
        };
        self.play_held = None;
        self.refusal = None;
        self.refused_from = None;
        self.phase = Phase::Inserting {
            cart,
            t: 0.0,
            core_ready: false,
            resumed: false,
            clean,
        };
    }

    /// Whether the cart going in is starting from the beginning. Read by whoever spawns the
    /// core, which is the one thing that has to know.
    pub fn starting_clean(&self) -> bool {
        matches!(self.phase, Phase::Inserting { clean: true, .. })
    }

    /// The hold fires under the finger rather than on the release, so it has an end the
    /// player can feel. A shelf that left the screen with A still down takes the arming with
    /// it: the press belonged to that screen.
    fn play_hold(&mut self) {
        let Some(at) = self.play_held else {
            return;
        };
        if !self.on_shelf() {
            self.play_held = None;
            return;
        }
        if self.now().saturating_sub(at) >= PLAY_HOLD_MS {
            self.insert(true);
        }
    }

    /// The other hold on this screen. Spent rather than left for the release, because the
    /// shelf stays up afterwards and a hold that kept firing every timer tick would swap the
    /// view over and over for as long as the button was down.
    fn fav_hold(&mut self) {
        let Some(at) = self.fav_held else {
            return;
        };
        if !self.on_shelf() {
            self.fav_held = None;
            return;
        }
        if self.now().saturating_sub(at) >= PLAY_HOLD_MS {
            self.fav_held = None;
            self.toggle_fav_view();
        }
    }

    /// A hold of Y: swap which machine's shelf is up. Spent like `fav_hold`, so it fires once
    /// rather than on every timer tick the button stays down.
    fn sys_hold(&mut self) {
        let Some(at) = self.sys_held else {
            return;
        };
        if !self.on_shelf() {
            self.sys_held = None;
            return;
        }
        if self.now().saturating_sub(at) >= PLAY_HOLD_MS {
            self.sys_held = None;
            self.cycle_system();
        }
    }

    /// A hold of B: open the direct-connect dialog. Spent like `fav_hold`.
    fn mtp_hold(&mut self) {
        let Some(at) = self.mtp_held else {
            return;
        };
        if !self.on_shelf() {
            self.mtp_held = None;
            return;
        }
        if self.now().saturating_sub(at) >= PLAY_HOLD_MS {
            self.mtp_held = None;
            self.open_direct_link();
        }
    }

    /// A short X on the shelf: star the cart under the caret, or take the star back.
    /// The star is the whole of the feedback — it appears on the cart, which is where the
    /// user is already looking.
    fn toggle_favorite(&mut self) {
        let Some(stem) = self.shelf.current_cart().map(|c| c.stem.clone()) else {
            return;
        };
        if !self.state.favorites.remove(&stem) {
            self.state.favorites.insert(stem);
        }
        self.persist();
        // The favourites shelf shows the starred carts and nothing else, so a cart that just
        // lost its star has to leave the view it is standing in.
        if self.fav_view {
            self.show_favorites();
        }
    }

    /// A hold of X: swap between the whole of the current machine's shelf and its starred
    /// carts. The library is three machines and only one shelf is up, so "the whole shelf" is
    /// this machine's carts, not all of them.
    fn toggle_fav_view(&mut self) {
        if self.fav_view {
            self.show_all();
        } else {
            self.show_favorites();
        }
    }

    /// The whole of the machine whose shelf is up — what leaving the favourites sub-view
    /// returns to.
    fn show_all(&mut self) {
        let keep = self.shelf.current_cart().map(|c| c.stem.clone());
        let view = self.system_view(self.shelf_system);
        self.fav_view = false;
        self.shelf.set_view(view, keep.as_deref());
        self.retally_letters();
    }

    /// The starred carts of the machine whose shelf is up, in library order so a cart sits on
    /// the same shelf, at the same place among its neighbours, as the one it came from.
    /// Refused rather than shown empty when nothing is starred: an empty shelf is a screen with
    /// nothing on it and no way to tell why.
    fn show_favorites(&mut self) {
        let keep = self.shelf.current_cart().map(|c| c.stem.clone());
        let view: Vec<usize> = self
            .shelf
            .carts
            .iter()
            .enumerate()
            .filter(|(_, c)| {
                c.system() == self.shelf_system && self.state.favorites.contains(&c.stem)
            })
            .map(|(i, _)| i)
            .collect();
        if view.is_empty() {
            return self.refuse();
        }
        self.fav_view = true;
        self.shelf.set_view(view, keep.as_deref());
        self.retally_letters();
    }

    /// The letter dial counts carts, so a view that is not the whole library has its own
    /// tally. Rebuilt on the edge rather than per frame: the card cannot change while slot is
    /// running, so the only thing that can move it is a view swap.
    fn retally_letters(&mut self) {
        let visible: Vec<Cart> = self.shelf.visible().into_iter().cloned().collect();
        self.letter_counts = tally_letters(&visible);
    }

    /// The star over every starred cart in the row.
    ///
    /// **Inside the cart**, in the moulded band the shell leaves between its curved top edge and
    /// the top of the label — `cart::label_panel` puts the label at 22.8% of the face, so that
    /// band is the top fifth of the shell. Directly above the sticker and still on the cartridge,
    /// under the arc, rather than floating over the row: the star is printed on the plastic, not
    /// pinned above the object. Scaled with the cart, so the enlarged selection wears a larger
    /// star and the row still reads as one row.
    fn draw_fav_stars(&self, out: &mut Vec<Draw>) {
        /// How tall the mark is on a cart, in screen pixels. Absolute rather than a share of the
        /// cart, because it is a reading and not part of the artwork: a star that grew with the
        /// selected cart would say the cart was more of a favourite than its neighbours.
        const STAR_PX: f32 = 20.0;
        let Some((tex, w, h)) = self.fav_star else {
            return;
        };
        if self.state.favorites.is_empty() {
            return;
        }
        let (w, h) = (w as f32, h as f32);
        for r in self.shelf.slot_rects(None, self.shelf_shake(), 0.0) {
            if !self
                .state
                .favorites
                .contains(&self.shelf.carts[r.cart].stem)
            {
                continue;
            }
            let (sh, sw) = (STAR_PX, w * (STAR_PX / h));
            // Centred on the label's top right corner: the corner of the paper, which is the one
            // piece of the cart that is the same on every machine's artwork and the place the eye
            // already goes. A card that supplies its own art says where its label is — the
            // magenta slot the label is actually fitted into — and that answer has to be used,
            // because it is not the built-in panel. Without one, the panel stands in.
            let corner = slot_ui::cart_label_share(self.shelf.system())
                .map(|(_, y0, x1, _)| (r.x + x1 * r.w, r.y + y0 * r.h))
                .unwrap_or_else(|| {
                    let (_, y0, x1, _) = self.shelf.size().label_panel_at(r.w as u32, r.h as u32);
                    (r.x + x1 as f32, r.y + y0 as f32)
                });
            let x = corner.0 - sw / 2.0;
            let y = corner.1 - sh / 2.0;
            // A bloom under it: the same star, larger and faint, so it reads as light coming
            // off the glyph rather than as a second outline. One extra quad, and the linear
            // tap on an upscaled alpha map is the blur — no pass and no blur shader.
            let g = sh * 0.45;
            out.push(Draw::Tex {
                x: x - g / 2.0,
                y: y - g / 2.0,
                w: sw + g,
                h: sh + g,
                tex,
                alpha: r.alpha * 0.30,
            });
            out.push(Draw::Tex {
                x,
                y,
                w: sw,
                h: sh,
                tex,
                alpha: r.alpha,
            });
        }
    }

    /// Which shelf is up, in the bottom right corner of the screen — where the original printed
    /// the time: hollow while the ordinary shelf is showing, solid and yellow while the
    /// favourites are. A reading rather than a control — the hold of B is the control — so it
    /// never looks like something to press.
    ///
    /// The mode badge (sun in light, moon in dark) is not here: it is printed beside the clock at
    /// the top of the band, where it reads as part of the same row of type.
    fn draw_fav_indicator(&self, out: &mut Vec<Draw>) {
        let lit = self.fav_view;
        let Some((tex, w, h)) = (if lit {
            self.fav_ind.map(|p| p.1)
        } else {
            self.fav_ind.map(|p| p.0)
        }) else {
            return;
        };
        if h == 0 {
            return;
        }
        let k = FAV_IND_DRAW / h as f32;
        let (sw, sh) = (w as f32 * k, FAV_IND_DRAW);
        // Centred on the battery reading's own centre at the other end of the top band, so the two
        // right-hand marks stand in one column.
        let read_right = OUT_W as f32 - STATUS_READ_MARGIN;
        let read_cx = read_right - self.battery_percent.w as f32 / 2.0;
        let ind_x = read_cx - sw / 2.0;
        let y = OUT_H as f32 - MOUTH_H + MOUTH_H / 2.0 - sh / 2.0;
        out.push(Draw::Tex {
            x: ind_x,
            y,
            w: sw,
            h: sh,
            tex,
            alpha: 1.0,
        });
    }

    fn eject(&mut self) {
        // Nowhere to eject to. Refused rather than ignored, so the held MENU says no
        // instead of reading as a device that stopped listening.
        if self.single_cart() {
            return self.refuse();
        }
        // Inserting as well as Playing, so a slot with no core behind it can still be
        // emptied: that is the only way to watch the travel more than once.
        let cart = match &mut self.phase {
            Phase::Playing { cart } | Phase::Inserting { cart, .. } => std::mem::take(cart),
            _ => return,
        };
        // A session does not survive its cart. Without this a phantom session outlives the
        // eject: `link_active()` stays true with no core left to carry it, `may_rewind`/
        // `may_load_state` stay wedged closed for whatever cart goes in next, and
        // `doze_expired`'s own guard refuses to let the device sleep again — forever, since
        // nothing left in the app ever flips it back. `Session::act`'s edge bridge (watching
        // `link_active()` fall across every `apply`) is what carries this to
        // `EmuHandle::end_link` on the emulator thread, the same way it does for a doze or a
        // power press.
        self.end_link();
        // The cart the menu was about is on its way out. Not reachable through the overlay
        // itself, which swallows the eject; this is here for whatever route into an eject
        // comes next, the way `begin_power_off` guards its own chokepoint rather than the
        // one caller that happened to need it.
        self.close_game_menu();
        self.close_cheat_menu();
        self.close_palette_browser();
        self.flush_eject(&cart);
        // The offer names a file in this cart's ring and a state only this cart's core can
        // read. Carried across the slot it would delete or load the wrong one.
        self.pending = None;
        // An eject asked for is not an eject refused, whatever was refused a moment ago.
        self.refusal = None;
        self.refused_from = None;
        self.phase = Phase::Ejecting {
            cart,
            t: -EJECT_HOLD_S,
        };
    }

    /// Everything durable happens here, before the animation rather than after it: the
    /// card can be pulled while the cart is still sliding out. A write that failed leaves
    /// the cart recorded as seated, so the next boot resumes it and the end of the
    /// animation retries the clear.
    fn flush_eject(&mut self, stem: &str) {
        let (Some(root), Some(snapshot)) = (&self.root, &self.snapshot) else {
            return;
        };
        let Some(state) = snapshot.state() else {
            eprintln!("slot: eject: the core gave up no state");
            return;
        };
        let Some(system) = self.system_of(stem) else {
            eprintln!("slot: eject: no cart in the shelf for {stem}");
            return;
        };
        let (state, sav) = trusted_write(snapshot.as_ref(), state, "eject");
        match persist::eject(root, self.core, system, stem, state.as_deref(), sav.as_deref()) {
            Ok(()) => self.state.cart = None,
            Err(e) => eprintln!("slot: eject: {e}"),
        }
    }

    /// Flush, then dark, then idle. The cart stays in the slot and `slot.state` is not
    /// touched: a sleep is not an eject, and the next boot has to resume this session
    /// whether the lid opens again or the battery runs out first.
    ///
    /// The one function every doze actually goes through: both `LidClose` arms (with the
    /// power menu open, and without) and `PowerTap` by way of `power_press` all return
    /// `self.doze()` rather than reimplementing it, so a guard here — and only here — closes
    /// every path in at once. Three copies of the same `if self.link_active()` at each call
    /// site is exactly the kind of duplication that let a mutation slip through unnoticed
    /// last time: `on_doze_timeout` carried a redundant copy of `doze_expired`'s own guard,
    /// and that second copy alone was enough to keep `doze_never_expires_while_a_session_is_live`
    /// passing after the real guard was mutated away.
    ///
    /// A live session ends here rather than surviving the doze — but the doze still happens:
    /// this used to `return` right after `end_link()`, which ended the session and then left
    /// the device sitting in `Phase::Playing`, wide awake, behind a lid the player had just
    /// shut. That is exactly the 400-700 mA outcome the paragraph below argues against,
    /// reached anyway, with the session dead on top of it — proven by `phase` still reading
    /// `Playing` ten seconds after a `LidClose` that hardware delivers exactly once per
    /// physical close, with no second press coming to "retry" into an actual doze.
    ///
    /// `Session::sync_speed` maps `Phase::Doze` to `Speed::Paused`, and pausing is one of the
    /// exact manipulations libretro's netpacket contract names as forbidden while players are
    /// connected — the same desync hazard as dropping the transport outright, not a lesser
    /// one. The alternative — holding the session open through a doze that keeps the core
    /// running *unpaused*, so the panel can go dark for free — is a bigger change than this
    /// fix (`sync_speed` would have to learn about sessions too) and would not even save the
    /// battery it sounds like it would: `doze_expired` already refuses to end a session on
    /// its own idle timer, so a session left open behind a shut lid would sit at 400-700 mA
    /// with the radio up for as long as the lid stayed shut, never once reaching the sub-45
    /// mA a real doze exists to reach. Ending the session costs a trade partner who shut the
    /// lid only to think for a moment — there is no answer here that costs nothing — but it
    /// is the one already chosen for `PowerPress`, and completing the doze underneath it is
    /// the only way to actually reach the low-power state this function exists for.
    fn doze(&mut self) {
        if self.link_active() {
            self.end_link();
        }
        // The overlay is drawn over a game that is about to go dark, and a starter left
        // running behind it would keep a radio up through the doze.
        self.close_game_menu();
        self.close_cheat_menu();
        self.close_palette_browser();
        // A shut lid is walking away, not choosing. Nothing is written and nothing animates:
        // waking comes back to a plain shelf.
        self.core_picker = None;
        if matches!(self.phase, Phase::Doze { .. }) {
            return;
        }
        self.flush_resume();
        // Only a running cart is worth waking back into. A lid closed over an animation
        // wakes to the shelf, one press from where it was, rather than into a core that
        // may not have finished loading.
        let cart = match &mut self.phase {
            Phase::Playing { cart } | Phase::Polaroids { cart } => Some(std::mem::take(cart)),
            _ => None,
        };
        self.polaroids = None;
        self.phase = Phase::Doze { cart };
        self.dozed_at = self.now();
        if let Some(power) = &mut self.power {
            power.on_close();
        }
    }

    fn wake(&mut self) {
        let Phase::Doze { cart } = &mut self.phase else {
            return;
        };
        self.phase = match cart.take() {
            Some(cart) => Phase::Playing { cart },
            None => Phase::Shelf,
        };
        if let Some(power) = &mut self.power {
            power.on_open();
        }
    }

    /// A dark panel is not a saving: the machine is still running flat out behind it at
    /// 400-700 mA. So the dark is a grace period rather than a state, and when it runs out
    /// the device stops for real.
    ///
    /// It suspends beautifully — under 45 mA — and that is not on offer, because it cannot
    /// wake itself back up: the RTC alarm arms, reads back, and never fires. A sleep nothing
    /// can end is a slow leak with a better name. Powering off costs the user a three second
    /// boot, and `slot.state` still names the cart, so they come back to the same frame.
    pub fn on_doze_timeout(&mut self) {
        if !matches!(self.phase, Phase::Doze { .. }) {
            return;
        }
        self.begin_power_off();
    }

    /// The lid's twin, and the only one of the two the device is certain to see. A tap
    /// dozes and a second one wakes.
    fn power_press(&mut self) {
        match self.phase {
            Phase::Doze { .. } => self.wake(),
            _ => self.doze(),
        }
    }

    /// A held button powers off, through the OS rather than the PMIC. The PMIC's own
    /// six-second hold cuts the rails in hardware with no sync, no unmount and no driver
    /// teardown; the software path unloads the GPU module first, which is the difference
    /// between a machine that stops and one that hangs with the rails up draining the
    /// battery. Six seconds remains the emergency underneath, and needs no help from here.
    ///
    /// Not an eject: the cart stays in the slot so the next boot resumes it. The flush is
    /// a no-op after a doze, which has already written the same file.
    /// The hold threshold raises the menu and nothing else. Every outcome from here is one
    /// the user chose rather than one the button committed them to, which is what makes the
    /// hold safe to discover by accident.
    fn open_power_menu(&mut self) {
        if self.power_menu.is_some() {
            return;
        }
        // Same hazard as the switcher: the menu pauses the core too — `Session::sync_speed`
        // maps `held()`, which the menu is one of, to `Speed::Paused` — one of the exact
        // manipulations libretro's netpacket contract forbids while a session is live. Unlike
        // `PowerPress` this button does not end the session for the player; it just declines,
        // the same shake every other "nothing doing" action in this file answers with.
        if self.link_active() {
            return self.refuse();
        }
        // Durable before the menu is even on screen: from here the user may hold on to the
        // PMIC's own six second cutoff, which takes the rails away whatever we wanted.
        self.flush_resume();
        self.power_menu = Some(0);
    }

    /// Up and down move, A commits, B leaves. Nothing times out: a menu that closed itself
    /// would do it exactly when the user looked away to think.
    fn power_menu_input(&mut self, action: Action) {
        let Some(index) = self.power_menu else {
            return;
        };
        let last = PowerChoice::ALL.len() - 1;
        match action {
            Action::GbaDown(Btn::Up) => self.power_menu = Some(index.saturating_sub(1)),
            Action::GbaDown(Btn::Down) => self.power_menu = Some((index + 1).min(last)),
            Action::GbaDown(Btn::B) => self.power_menu = None,
            Action::GbaDown(Btn::A) => {
                self.power_menu = None;
                // Both choices end the game whatever was underneath was drawn over, and only
                // one of them reaches `begin_power_off`: a restart sets its flag here and
                // goes straight to the shutdown screen.
                self.close_game_menu();
                match PowerChoice::ALL[index] {
                    PowerChoice::Restart => {
                        self.restarting = true;
                        self.act_at = self.now() + SHUTDOWN_SHOW_MS;
                        self.set_led(LedState::Off);
                    }
                    PowerChoice::PowerOff => self.begin_power_off(),
                }
            }
            _ => {}
        }
    }

    /// The seated cart's cheat codes with their live on/off, in file order. Read by the
    /// frontend to rasterise the table's row faces and by the draw to colour each chip.
    pub(crate) fn cheats(&self) -> &[CheatItem] {
        &self.cheats
    }

    /// Replace the cheat list (called when a cart's core spawns) and close any open table, so a
    /// switch to a cart with none does not leave the highlight pointing at the old codes.
    pub(crate) fn set_cheats(&mut self, entries: Vec<crate::root::CheatEntry>) {
        self.cheats = entries
            .into_iter()
            .map(|e| CheatItem {
                code: e.code,
                desc: e.desc,
                // Default every code OFF: the list opens with nothing applied, and the
                // player switches on only the codes they want. The master `cheats_on`
                // stays on so a per-row toggle here still takes effect when pushed.
                enabled: false,
            })
            .collect();
        self.cheat_menu = None;
    }

    /// `Some` while the cheat table is up, `None` otherwise. Read by `Session` to pause the core
    /// and to decide what SELECT+A does: open it, or master-toggle-all while it is open.
    pub fn cheat_menu(&self) -> Option<usize> {
        self.cheat_menu
    }

    /// Whether the table's per-row edit needs pushing to the core. Cleared by `Session` once it
    /// has re-sent the list, because the table cannot reach the emulator itself.
    pub(crate) fn take_cheats_dirty(&mut self) -> bool {
        std::mem::take(&mut self.cheats_dirty)
    }

    /// Whether the mode moved since the last frame, and clears it. The binary takes this every
    /// frame the way it takes `take_cheats_dirty`: the palette is a global and the textures are
    /// not, so the one thing that has to be told is whoever owns the textures.
    pub(crate) fn take_mode_dirty(&mut self) -> bool {
        std::mem::take(&mut self.mode_dirty)
    }

    /// Whether a shelf swap has left the row's frame cut for the wrong machine. Taken once by
    /// the frontend, which has the only compositor.
    pub(crate) fn take_shelf_dirty(&mut self) -> bool {
        std::mem::take(&mut self.shelf_dirty)
    }

    /// SELECT+START: print the device the other way round.
    ///
    /// Three things, and the order is the order they have to happen in. The palette moves
    /// first, so that every quad drawn from here on — this frame included — is already the new
    /// colour. The state file follows, so a device switched off a second later comes back the
    /// way it was left. And the flag goes up last, because it is a request to someone else and
    /// nothing about this frame depends on the answer.
    ///
    /// The mode is a *setting* rather than a screen, and it is written through the same door
    /// the levels use, so a device switched off a second after the press comes back the way it
    /// was left rather than the way it booted.
    pub(crate) fn toggle_mode(&mut self) {
        let mode = slot_ui::palette::mode().other();
        slot_ui::palette::set_mode(mode);
        // A theme change repaints everything the palette touches: the fixed furniture (icons,
        // favourite stars, machine logos), the lazy caches (shelf title, clock, charge, about,
        // switcher), and the cart faces — which bake the shell colour in light mode
        // (`housing()`). `mode_dirty` pokes the frontend, which re-runs `rebake` in place on the
        // next frame: it re-rasterises the fixed faces and the visible shelf window, and drops
        // the rest so the resident pump rebuilds each as the caret reaches it. That is the whole
        // job a restart did, without re-running the boot — so no restart here.
        self.state.mode = mode;
        self.mode_dirty = true;
        self.persist();
    }

    /// SELECT+A over a running game. Opens the table — the "see the whole list and pick" screen
    /// a bare ON/OFF toast never was. The same chord again, while the table is up, flips every
    /// code at once: the old master toggle, now reachable from inside.
    pub(crate) fn open_cheat_menu(&mut self) {
        if self.cheat_menu.is_some() {
            return;
        }
        // The table is the only overlay that should be up while it is; an open in-game menu would
        // otherwise sit underneath it and steal nothing but still be half-true.
        self.game_menu = None;
        self.cheat_menu = Some(0);
    }

    /// Closes the table without touching any code's state.
    fn close_cheat_menu(&mut self) {
        self.cheat_menu = None;
    }

    /// `Some(cell)` while the palette browser is up, `None` otherwise. Read by `Session` to pause
    /// the core and to keep the pad out of it, the same way it reads the cheat table.
    pub fn palette_browser(&self) -> Option<usize> {
        self.pal_cursor
    }

    /// Hold of the palette key: the browser, on the page the key is walking, with the caret on the
    /// palette that is actually up.
    ///
    /// A Game Boy's list is hundreds of entries and this is the only way to see them; the colour
    /// machines have seven and no browser at all. Only while a game is running, too: there is no
    /// picture on the shelf to recolour, and a hold there is simply nothing.
    ///
    /// **Refused while a model is up**, for the reason `cycle_cc` gives: a model's four shades are
    /// bound to its screen art, so a browser that offered to recolour the picture inside that case
    /// would be offering something it could not then do. Refused rather than dropped, so the press
    /// reads as answered.
    fn open_palette_browser(&mut self) {
        if self.pal_cursor.is_some() || self.current_machine() != System::Gb {
            return;
        }
        if !matches!(self.phase, Phase::Playing { .. }) {
            return;
        }
        if App::model_of_index(self.overlay_slot, self.overlay_index).is_some() {
            self.refuse();
            return;
        }
        let base = self.pal_page * slot_ui::PALETTE_PER_PAGE;
        self.pal_cursor = Some(self.gb_palette().saturating_sub(base));
    }

    fn close_palette_browser(&mut self) {
        self.pal_cursor = None;
    }

    /// The browser owns every button on the game's side while it is up. The arrows ring inside the
    /// page — a row down is three cells — L and R turn the page, A takes the palette under the
    /// caret and B leaves without it.
    ///
    /// The caret rings rather than stops at the edges because the page is what bounds it, and L/R
    /// are the way out; a caret that could reach an edge and stop would be a caret the user has to
    /// find their way back from.
    fn palette_browser_input(&mut self, action: Action) {
        let Some(cursor) = self.pal_cursor else {
            return;
        };
        let total = crate::palettes::GB_PALETTES.len();
        let per = slot_ui::PALETTE_PER_PAGE;
        match action {
            Action::GbaDown(Btn::Left) | Action::ShelfLeft => {
                self.pal_cursor = Some(slot_store::step_cursor(self.pal_page, cursor, -1, total, per));
            }
            Action::GbaDown(Btn::Right) | Action::ShelfRight => {
                self.pal_cursor = Some(slot_store::step_cursor(self.pal_page, cursor, 1, total, per));
            }
            Action::GbaDown(Btn::Up) => {
                self.pal_cursor = Some(slot_store::step_cursor(self.pal_page, cursor, -3, total, per));
            }
            Action::GbaDown(Btn::Down) => {
                self.pal_cursor = Some(slot_store::step_cursor(self.pal_page, cursor, 3, total, per));
            }
            Action::GbaDown(Btn::L1) | Action::GbaDown(Btn::R1) => {
                // The caret is a cell on the old page, so a shorter page it arrives on clamps it
                // rather than dropping it off the end. Only the last page is short, and only by
                // thirteen cells, so this is one press of the boundary rather than a rule.
                let delta = if action == Action::GbaDown(Btn::L1) {
                    -1
                } else {
                    1
                };
                self.pal_page = slot_store::step_page(self.pal_page, delta, total, per);
                let n = slot_store::len_on(self.pal_page, total, per);
                self.pal_cursor = Some(cursor.min(n.saturating_sub(1)));
            }
            Action::GbaDown(Btn::A) => {
                let picked = slot_store::index_of(self.pal_page, cursor, total, per);
                self.display.cc[DisplayFilter::machine_slot(System::Gb)] = picked as u16;
                if let Some(root) = &self.root {
                    crate::root::write_display_modes(root, self.display.mask_mode, self.display.cc);
                }
                self.close_palette_browser();
            }
            Action::GbaDown(Btn::B) => self.close_palette_browser(),
            _ => {}
        }
    }

    /// The name of the entry under the browser's caret, or `None` while it is shut. The frontend
    /// mints a face from this and nothing else; comparing it to the last one it built is what
    /// keeps a moved caret to one rasterise and a still one to none.
    pub fn palette_name_want(&self) -> Option<String> {
        let cursor = self.pal_cursor?;
        let per = slot_ui::PALETTE_PER_PAGE;
        let total = crate::palettes::GB_PALETTES.len();
        let at = slot_store::index_of(self.pal_page, cursor, total, per);
        Some(crate::palettes::GB_PALETTES[at].0.to_string())
    }

    /// The face the frontend built for that name.
    pub(crate) fn set_palette_name_face(&mut self, face: Option<(TexId, u32, u32)>) {
        self.pal_name = face;
    }

    /// The palette browser, over the paused game. Only ever up over a cart in the slot, so the
    /// `Phase::Playing` arm of `draw` is the only caller.
    fn draw_palette_browser(&self, out: &mut Vec<Draw>) {
        let Some(cursor) = self.pal_cursor else {
            return;
        };
        let per = slot_ui::PALETTE_PER_PAGE;
        let total = crate::palettes::GB_PALETTES.len();
        let base = (self.pal_page * per).min(total);
        let n = slot_store::len_on(self.pal_page, total, per);
        // A copy of the page's four shades, at most fifteen entries and only while the browser is
        // open. The alternative is a second compile-time array of the same 347 rows, which is
        // exactly the parallel table that drifts the first time one of them is edited.
        let shades: Vec<[[u8; 3]; 4]> = crate::palettes::GB_PALETTES[base..base + n]
            .iter()
            .map(|entry| entry.1)
            .collect();
        let view = slot_ui::PaletteView {
            page: self.pal_page,
            cursor,
            pages: slot_ui::palette_pages(total),
            shades: &shades,
            name: self.pal_name,
        };
        slot_ui::draw_palette_browser(&view, out);
    }

    /// The master switch behind SELECT+A's second press: flips the lot on or off at once.
    pub(crate) fn toggle_all_cheats(&mut self) {
        self.cheats_on = !self.cheats_on;
    }

    /// The table owns every button on the game's side while it is up. Up and down move, A flips
    /// the highlighted code, B leaves. The master chord is handled in `Session::act`, before this
    /// sees the action, so it never arrives here as a toggle.
    fn cheat_menu_input(&mut self, action: Action) {
        let Some(index) = self.cheat_menu else {
            return;
        };
        let last = self.cheats.len().saturating_sub(1);
        match action {
            Action::GbaDown(Btn::Up) => self.cheat_menu = Some(index.saturating_sub(1)),
            Action::GbaDown(Btn::Down) => self.cheat_menu = Some((index + 1).min(last)),
            Action::GbaDown(Btn::A) => {
                if let Some(item) = self.cheats.get_mut(index) {
                    item.enabled = !item.enabled;
                }
                self.cheats_dirty = true;
            }
            Action::GbaDown(Btn::B) => self.close_cheat_menu(),
            _ => {}
        }
    }

    /// The "no cheats" panel, rasterised once at boot. Shown by `draw_cheat_menu` when the table
    /// opens on a cart that carries no codes.
    pub(crate) fn set_cheat_empty_face(&mut self, face: (TexId, u32, u32)) {
        self.cheat_empty_face = Some(face);
    }

    /// (Re)bind the row faces to a cart's codes, with the stem they belong to so the frontend
    /// knows when to rebuild. Called from the frontend's per-frame sync.
    pub(crate) fn set_cheat_faces(&mut self, stem: Option<String>, faces: Vec<(TexId, u32, u32)>) {
        self.cheat_face_stem = stem;
        self.cheat_faces = faces;
    }

    /// The stem whose faces `cheat_faces` currently holds. `None` until the first cart with
    /// cheats seats, and again after eject.
    pub(crate) fn cheat_face_stem(&self) -> Option<&str> {
        self.cheat_face_stem.as_deref()
    }

    /// How many row faces `cheat_faces` currently holds. Compared against `cheats().len()` so the
    /// frontend knows to rebuild after the codes load from the card post-spawn.
    pub(crate) fn cheat_face_count(&self) -> usize {
        self.cheat_faces.len()
    }

    /// The cheat table: a dark panel over the paused game, listing every code with a green (on)
    /// or grey (off) chip, the highlighted row carried on the menu's own bar. A window of
    /// `CHEAT_VISIBLE` rows scrolls so the highlight stays on screen no matter how long the list.
    fn draw_cheat_menu(&self, index: usize, out: &mut Vec<Draw>) {
        out.push(Draw::Rect {
            x: 0.0,
            y: 0.0,
            w: OUT_W as f32,
            h: OUT_H as f32,
            colour: slot_ui::opening(),
        });
        if self.cheats.is_empty() {
            if let Some((tex, w, h)) = self.cheat_empty_face {
                out.push(Draw::Tex {
                    x: ((OUT_W - w) / 2) as f32,
                    y: ((OUT_H - h) / 2) as f32,
                    w: w as f32,
                    h: h as f32,
                    tex,
                    alpha: 1.0,
                });
            }
            return;
        }
        let n = self.cheats.len();
        let vis = n.min(CHEAT_VISIBLE);
        let start = if n <= CHEAT_VISIBLE {
            0
        } else {
            index
                .saturating_sub(CHEAT_VISIBLE / 2)
                .min(n - CHEAT_VISIBLE)
        };
        let top = centred_top(vis);
        let left: f32 = 56.0;
        for v in 0..vis {
            let row = start + v;
            if row >= n {
                break;
            }
            let y = top + CHEAT_PITCH * v as f32;
            if row == index {
                out.push(Draw::Rect {
                    x: 8.0,
                    y: y + POWER_MENU_BAR_INSET,
                    w: (OUT_W - 16) as f32,
                    h: CHEAT_PITCH - 2.0 * POWER_MENU_BAR_INSET,
                    colour: slot_ui::edge(),
                });
            }
            // ON/OFF chip to the left of the code: green when live, grey otherwise. "Live" means
            // the code is switched on AND the master is on.
            let on = self.cheats_on && self.cheats[row].enabled;
            let iw = 14.0;
            let ix = 24.0;
            let iy = y + (CHEAT_PITCH - iw) / 2.0;
            out.push(Draw::Rect {
                x: ix,
                y: iy,
                w: iw,
                h: iw,
                colour: if on {
                    [0.30, 0.85, 0.45, 1.0]
                } else {
                    [0.55, 0.55, 0.6, 1.0]
                },
            });
            if let Some((tex, w, h)) = self.cheat_faces.get(row).copied() {
                out.push(Draw::Tex {
                    x: left,
                    y: y + (CHEAT_PITCH - h as f32) / 2.0,
                    w: w as f32,
                    h: h as f32,
                    tex,
                    alpha: 1.0,
                });
            }
        }
    }

    /// SELECT on the shelf offers the highlighted cart's core, opening on the one it already
    /// uses so the menu answers "which is this?" before it asks "which do you want?".
    ///
    /// Both the read that positions the highlight and the write that follows need the card.
    /// Without one there is nothing to configure and nowhere to put an answer, so the button
    /// stays inert rather than raising a menu whose choice would evaporate.
    fn open_core_picker(&mut self) {
        let Some(root) = self.root.clone() else {
            return;
        };
        // Nothing to configure with no cart under the highlight, and a picker that wrote to
        // an empty stem would leave a line for a cart that is not there.
        let Some(cart) = self.shelf.current_cart() else {
            return;
        };
        // Nothing to choose for anything that is not a GBA cart: gpSP does not take Game Boy,
        // so mGBA is not a preference to record but the only answer there is. The picker is two
        // sockets to hop between, and opening it over one socket is a question that can only be
        // refused.
        if cart.system() != System::Gba {
            return;
        }
        let seat = slot_store::core_for(&root, &cart.stem);
        let now = self.now();
        let mut picker = CorePicker::open(seat, now);
        if self.core_faces_ready() {
            picker.start(now);
        }
        self.core_picker = Some(picker);
        // Whatever the shelf had armed before START belonged to the shelf that was showing,
        // not to the cart now open over it: a held direction would keep repeating underneath
        // the lid, and a held A would still insert the cart once its 500 ms ran out.
        self.shelf.release_hold();
        self.play_held = None;
        self.fav_held = None;
        self.sys_held = None;
        self.mtp_held = None;
    }

    /// Whether the board and lid on the GPU are the highlighted cart's, so its open can start.
    fn core_faces_ready(&self) -> bool {
        self.core_faces_stem
            .as_deref()
            .is_some_and(|stem| self.selected_stem() == Some(stem))
    }

    /// The picker owns every button while it is up, including the arrows the shelf uses: a
    /// board that let the row behind it move would act on a different cart than the one whose
    /// lid is off. The arrows point at the sockets, so they do not wrap.
    fn core_picker_input(&mut self, action: Action) {
        let press = match action {
            Action::GbaDown(Btn::Left) | Action::ShelfLeft => Press::Left,
            Action::GbaDown(Btn::Right) | Action::ShelfRight => Press::Right,
            Action::GbaDown(Btn::A) => Press::Keep,
            Action::GbaDown(Btn::B) => Press::Back,
            _ => return,
        };
        let now = self.now();
        let Some(picker) = &mut self.core_picker else {
            return;
        };
        let outcome = picker.press(press, now);
        if let Outcome::Write(core) = outcome {
            self.write_core(core);
        }
    }

    /// The rows this menu has right now. A question about the cart in the slot rather than
    /// about the menu, so it is asked per open and never held: a stored answer could only
    /// ever go stale against the cart it was about.
    fn game_rows(&self) -> Vec<GameRow> {
        GameRow::ALL
            .into_iter()
            .filter(|row| match row {
                // gpSP is the only core with a netpacket interface to link over. Absent
                // rather than shown and refused: the fix is not on this screen — it is four
                // steps away on the shelf — and a row that says so is a row that teaches a
                // dead end.
                GameRow::Link => self.core == Core::Gpsp,
            })
            .collect()
    }

    /// Where the Link row sits among the rows that exist, so backing out of it lands back on
    /// it rather than on whatever happens to be first.
    fn link_row(&self) -> usize {
        self.game_rows()
            .iter()
            .position(|r| *r == GameRow::Link)
            .unwrap_or(0)
    }

    /// SELECT+MENU over a running game. Nothing is torn down and no phase changes: the cart
    /// is still seated behind it and cancelling gives it straight back.
    fn open_game_menu(&mut self) {
        if self.game_menu.is_some() {
            return;
        }
        // The same hazard the power menu's own guard exists for: this overlay pauses the
        // core underneath it (`Session::held` names it, and `sync_speed` maps that to
        // `Speed::Paused`), which is one of the exact manipulations libretro's netpacket
        // contract forbids while players are connected. Declined with the shake every other
        // "nothing doing" in this file answers with — and a device already in a session has
        // nothing to pick in here anyway.
        if self.link_active() {
            return self.refuse();
        }
        // An empty panel over a paused game is worse than no panel at all. Today that means
        // an mGBA cart raises nothing, Link being the only row there is; when a second row
        // lands this stops being about the link at all and stays exactly as true.
        if self.game_rows().is_empty() {
            return;
        }
        self.game_menu = Some(GameMenu::Rows(0));
    }

    /// The menu owns every button on the game's side of the device while it is up. Up and
    /// down wrap, for the core picker's reason: with two rows either arrow is the other's
    /// undo, and an end that stuck would need the player to know which one they were against.
    fn game_menu_input(&mut self, action: Action) {
        let Some(menu) = self.game_menu else {
            return;
        };
        match menu {
            GameMenu::Rows(row) => {
                let rows = self.game_rows();
                let last = rows.len();
                match action {
                    Action::GbaDown(Btn::Up) if last > 0 => {
                        self.game_menu = Some(GameMenu::Rows((row + last - 1) % last))
                    }
                    Action::GbaDown(Btn::Down) if last > 0 => {
                        self.game_menu = Some(GameMenu::Rows((row + 1) % last))
                    }
                    Action::GbaDown(Btn::A) => match rows.get(row) {
                        Some(GameRow::Link) => self.game_menu = Some(GameMenu::Link(0)),
                        None => {}
                    },
                    // The chord closes it as well as opening it, so the gesture that got
                    // here gets back without the player having to know B also works.
                    Action::GbaDown(Btn::B) | Action::GameMenu => self.close_game_menu(),
                    _ => {}
                }
            }
            GameMenu::Link(row) => {
                let last = LinkRow::ALL.len();
                match action {
                    Action::GbaDown(Btn::Up) => {
                        self.game_menu = Some(GameMenu::Link((row + last - 1) % last))
                    }
                    Action::GbaDown(Btn::Down) => {
                        self.game_menu = Some(GameMenu::Link((row + 1) % last))
                    }
                    Action::GbaDown(Btn::A) => {
                        if let Some(pick) = LinkRow::ALL.get(row).copied() {
                            self.start_link(
                                LinkStarter::spawn(pick.role(), link_port()),
                                pick.client_id(),
                            );
                        }
                    }
                    // Back to the rows rather than out of the menu: a player one press into
                    // a two-press choice is not asking to leave.
                    Action::GbaDown(Btn::B) => {
                        self.game_menu = Some(GameMenu::Rows(self.link_row()))
                    }
                    Action::GameMenu => self.close_game_menu(),
                    _ => {}
                }
            }
            // B asks the worker to stop and the screen stays where it is until it answers.
            // `link_radio::up` is an opaque blocking process spawn and nothing can interrupt
            // it for one to five seconds, so closing here would put the player back in their
            // game with an access point still coming up behind them.
            GameMenu::Working(_) => {
                if action == Action::GbaDown(Btn::B) {
                    if let Some(starting) = &mut self.starting {
                        starting.starter.cancel();
                    }
                }
            }
            // A sentence to read, and either button is the way off it.
            GameMenu::Failed(_) => match action {
                Action::GbaDown(Btn::A) | Action::GbaDown(Btn::B) | Action::GameMenu => {
                    self.close_game_menu()
                }
                _ => {}
            },
        }
    }

    /// Hands the overlay a worker that is already running, and puts the screen on the first
    /// step. Split from the pick that spawns one so a caller can supply its own: that is the
    /// only seam by which this screen can be driven with no network interface anywhere near
    /// it, and it is the same seam `LinkStarter::spawn_with` exists for one layer down.
    pub fn start_link(&mut self, starter: LinkStarter, client_id: u16) {
        // Whatever was already running is asked to stop on its way out. `LinkStarter` has no
        // `Drop`: dropping one silently leaves its radio up behind the screen.
        if let Some(mut old) = self.starting.replace(LinkStarting { starter, client_id }) {
            old.starter.cancel();
        }
        self.game_menu = Some(GameMenu::Working(LinkStep::Radio));
    }

    /// Ends the overlay and anything it had running.
    ///
    /// `LinkStarter` has no `Drop`, so a starter dropped mid-wait keeps working: a host
    /// dropped while waiting leaves its access point up for up to thirty seconds with
    /// nothing on the other end of it. Every path that ends the overlay comes through here,
    /// including the three that never touched it — a shut lid, a cart coming out and a power
    /// off all end the game this was drawn over.
    fn close_game_menu(&mut self) {
        self.game_menu = None;
        if let Some(mut starting) = self.starting.take() {
            starting.starter.cancel();
        }
    }

    /// One message a frame, which is all the worker ever has for it.
    fn poll_link(&mut self) {
        // Only while this overlay is the thing on screen. A power menu raised over it pauses
        // the core, and a session must not begin under one — the worker's message keeps in
        // its own queue until that menu is gone.
        if self.power_menu.is_some() {
            return;
        }
        let Some(mut starting) = self.starting.take() else {
            return;
        };
        match starting.starter.poll() {
            None => self.starting = Some(starting),
            Some(LinkProgress::At(step)) => {
                self.game_menu = Some(GameMenu::Working(step));
                self.starting = Some(starting);
            }
            // The overlay's job is done and the game comes back with a session live over it.
            // `begin_link` is this side's bookkeeping; the transport is left for whoever owns
            // the emulator thread to collect, since nothing here may touch a wire.
            Some(LinkProgress::Ready(link)) => {
                self.game_menu = None;
                self.begin_link(starting.client_id);
                self.link_transport = Some((starting.client_id, Box::new(link)));
            }
            // The player asked for this. Not a screen to read: straight back to the game.
            Some(LinkProgress::Failed(LinkFail::Cancelled)) => self.game_menu = None,
            Some(LinkProgress::Failed(fail)) => self.game_menu = Some(GameMenu::Failed(fail)),
        }
    }

    /// The choice, onto the card. Best effort, like every other card write here: a read only
    /// or absent card is a shelf that still works, not a boot failure. Nothing else in the
    /// app is told — `self.core` is the seated cart's, set when a core is actually spawned,
    /// and the shelf has none seated.
    fn write_core(&self, core: Core) {
        let (Some(root), Some(cart)) = (self.root.clone(), self.shelf.current_cart()) else {
            return;
        };
        if let Err(e) = slot_store::write_selected_core(&root, &cart.stem, core) {
            eprintln!("slot: core: could not write selected_core.ini: {e}");
        }
    }

    /// Every path to shutdown — a held button, an idle doze timing out, and a critical
    /// battery reading that needs no button at all — funnels through here, so none of them
    /// leaves the LED reporting Running or Charging through a shutdown the user is not
    /// watching finish. A real behaviour on a handheld: the case still has a light on it for
    /// as long as `poweroff` takes to actually cut power.
    fn begin_power_off(&mut self) {
        // Idempotent, and that is the whole of why: `doze_expired` is a level rather than an
        // edge and this leaves the phase on `Doze`, so `timers` calls back here every frame
        // for as long as the lid is shut. Re-arming `act_at` each time walked the deadline
        // ahead of the clock forever, and the device sat dark and awake until the lid opened
        // and took the phase out of `Doze` — at which point it powered off in the user's
        // hands, on the frame they came back to the session.
        if self.powering_off {
            return;
        }
        // A power-off pauses the core outright (`shutting_down()`, of which this is the
        // start, is one of the states `Session::sync_speed` maps to `Speed::Paused`) —
        // libretro's netpacket contract forbids that for as long as a session is live, the
        // same hazard `doze`'s own guard exists for. `doze` and the power menu's own open
        // already end a session before either of their own routes reaches here, which is
        // why this was previously always false in practice by the time any caller arrived —
        // right up until a critical battery reading turned out to be a fifth route in, with
        // no button, no menu and no doze anywhere upstream of it to have ended one first.
        // Guarding the chokepoint itself, rather than that one caller, is what keeps a sixth
        // route from reopening the same hole: whatever calls `begin_power_off` next inherits
        // this for free.
        if self.link_active() {
            self.end_link();
        }
        self.close_game_menu();
        self.close_cheat_menu();
        self.close_palette_browser();
        self.powering_off = true;
        self.act_at = self.now() + SHUTDOWN_SHOW_MS;
        self.set_led(LedState::Off);
    }

    /// The gauge, polled from `timers` and injected by the tests. Only a charge state the
    /// device positively asserted suppresses the cutoff: unknown and discharging both power
    /// off at the threshold, which is what the frontend did before it could read one.
    pub fn on_battery(&mut self, b: Battery) {
        if b.percent > BATTERY_CRITICAL || self.powering_off {
            return;
        }
        if matches!(b.charge, Charge::Charging | Charge::Full) {
            return;
        }
        // A real power off, not a sleep. This is the one shutdown the user did not ask for,
        // and suspending a cell this empty only spends what is left of it more slowly.
        self.flush_resume();
        self.begin_power_off();
    }

    fn doze_expired(&self) -> bool {
        // Suspended for as long as a session is live: a trade partner reading a menu on the
        // other device must not have the link dropped out from under them by this one's own
        // idle timer.
        if self.link_active() {
            return false;
        }
        let (Phase::Doze { .. }, Some(power)) = (&self.phase, &self.power) else {
            return false;
        };
        self.now().saturating_sub(self.dozed_at) >= power.timeout().as_millis() as Millis
    }

    /// resume.state and the battery save, with the slot left alone. A cart that is not
    /// playing has no state of its own to write.
    fn flush_resume(&mut self) {
        // The invariant is 60 s since the state was last durable, not 60 s since the last
        // autosave, so an attempt that had nothing to write still moves the deadline.
        self.autosave_at = self.now() + AUTOSAVE_MS;
        let (Some(root), Some(snapshot), Some(cart)) = (&self.root, &self.snapshot, self.seated())
        else {
            return;
        };
        let Some(state) = snapshot.state() else {
            eprintln!("slot: flush: the core gave up no state");
            return;
        };
        let Some(system) = self.system_of(cart) else {
            eprintln!("slot: flush: no cart in the shelf for {cart}");
            return;
        };
        let (state, sav) = trusted_write(snapshot.as_ref(), state, "flush");
        if let Err(e) =
            persist::flush(root, self.core, system, cart, state.as_deref(), sav.as_deref())
        {
            eprintln!("slot: flush: {e}");
        }
    }

    /// The ring for the cart in the slot. `None` outside the binary, where there is no
    /// content root, which reads as a cart that has never been saved.
    /// The machine of the cart that owns `stem` — the folder its save and its states live under.
    /// Read from the shelf rather than kept on `App`, because the shelf already carries every
    /// cart's machine and a second copy is exactly the thing that drifts.
    fn system_of(&self, stem: &str) -> Option<System> {
        self.shelf
            .carts
            .iter()
            .find(|c| c.stem == stem)
            .map(|c| c.system())
    }

    fn ring(&self) -> Option<StateRing> {
        let (Some(root), Some(cart)) = (&self.root, self.seated()) else {
            return None;
        };
        let system = self.system_of(cart)?;
        Some(StateRing::new(root, self.core, system, cart))
    }

    fn seated(&self) -> Option<&str> {
        match &self.phase {
            Phase::Playing { cart } | Phase::Polaroids { cart } => Some(cart),
            _ => None,
        }
    }

    fn entries(&self) -> Vec<StateEntry> {
        self.ring().and_then(|r| r.list().ok()).unwrap_or_default()
    }

    /// An empty ring shakes rather than opening an empty screen, per spec section 4.
    fn open_polaroids(&mut self) {
        // Opening the switcher pauses the core — `Session::sync_speed` maps
        // `Phase::Polaroids` straight to `Speed::Paused` — one of the exact manipulations
        // libretro's netpacket contract forbids while a session is live, whether or not the
        // player means to load anything once inside. Checked ahead of even looking for
        // states to show, the same way `load_newest` already checked ahead of looking for
        // one to load.
        if self.link_active() {
            return self.refuse();
        }
        let entries = self.entries();
        if entries.is_empty() {
            return self.refuse();
        }
        let Phase::Playing { cart } = &mut self.phase else {
            return;
        };
        let cart = std::mem::take(cart);
        let mut p = Polaroids::new(entries);
        p.set_undo(self.undo_label());
        self.polaroids = Some(p);
        self.phase = Phase::Polaroids { cart };
        self.push_hint_faces();
    }

    fn close_polaroids(&mut self) {
        let Phase::Polaroids { cart } = &mut self.phase else {
            return;
        };
        let cart = std::mem::take(cart);
        self.polaroids = None;
        self.phase = Phase::Playing { cart };
    }

    fn load_selected(&mut self) {
        let state = self
            .polaroids
            .as_ref()
            .and_then(|p| p.selected())
            .map(|e| e.state.clone());
        // `None` means nothing was selected, not a refusal, and still closes exactly as
        // before. `Some(false)` means `load_file` refused (a live session, most reachably —
        // see its own doc comment) and already shook the screen for it; closing the switcher
        // on top of that shake would read as the pick landing and then being dismissed, when
        // nothing happened at all. Unreachable today, since `open_polaroids` already refuses
        // to open a switcher a session forbids picking from — but wrong the moment that guard
        // moves, and cheap to keep correct regardless of where it lives.
        let refused = state.map(|state| self.load_file(&state)) == Some(false);
        if !refused {
            self.close_polaroids();
        }
    }

    /// Not undoable, and deliberately so. The undo slot holds one save or one load, and a
    /// third kind in it would be an undo whose meaning depended on what you did last. A state
    /// chosen off a screen showing you exactly which one is a decision, not a slip.
    fn delete_selected(&mut self) {
        let stamp = self
            .polaroids
            .as_ref()
            .and_then(|p| p.selected())
            .map(|e| e.stamp.clone());
        let (Some(stamp), Some(ring)) = (stamp, self.ring()) else {
            return;
        };
        if let Err(e) = ring.remove(&stamp) {
            eprintln!("slot: delete: {e}");
            return;
        }
        // An offer left pointing at a file that is gone would remove nothing and then put the
        // evicted entry back, which is not what undoing that save means any more.
        if self.undo_targets(&stamp) {
            self.pending = None;
        }
        let Some(p) = &mut self.polaroids else {
            return;
        };
        p.remove_selected();
        if p.is_empty() {
            self.close_polaroids();
        }
    }

    fn undo_targets(&self, stamp: &str) -> bool {
        match self.pending.as_ref() {
            Some((PendingUndo::Save { stamp: pending, .. }, _)) => pending == stamp,
            // A load's undo holds the prior state in memory, so no file on the card can
            // invalidate it.
            _ => false,
        }
    }

    fn load_newest(&mut self) {
        let Some(newest) = self.entries().first().map(|e| e.state.clone()) else {
            return self.refuse();
        };
        self.load_file(&newest);
    }

    /// The chokepoint every load-from-disk route funnels through — `load_newest` above and
    /// `load_selected` alike — so the session guard lives here once rather than at each
    /// caller. That used to be `load_newest`'s own job, checked ahead of even looking for a
    /// state to load; `load_selected` never got the same check, which is what let the
    /// switcher's own A-button pick bypass it entirely. Guarding here instead closes that
    /// hole for both today's callers and whatever the next one turns out to be.
    /// Reports whether the load actually happened, so a caller that only means to load —
    /// `load_newest` — can ignore it, and one that has something else riding on the answer —
    /// `load_selected`, which must not close the switcher out from under a refusal it just
    /// drew — can ask rather than repeating the guard above for itself.
    fn load_file(&mut self, state: &Path) -> bool {
        // A state load would desynchronise the other device with no way back to agreement.
        if !self.may_load_state() {
            self.refuse();
            return false;
        }
        let Some(snapshot) = &self.snapshot else {
            return false;
        };
        let bytes = match std::fs::read(state) {
            Ok(bytes) => bytes,
            Err(e) => {
                eprintln!("slot: load: {e}");
                return false;
            }
        };
        // Taken before the load, which is the last moment there is anything to go back to.
        let prior = snapshot.state();
        snapshot.load(bytes);
        self.hud.toast(Toast::StateLoaded, self.now());
        if let Some(prior) = prior {
            self.pending = Some((PendingUndo::Load { prior }, self.now()));
        }
        true
    }

    /// `SELECT+A` and nothing else reaches here. A state with no picture is still worth
    /// keeping: the switcher draws a blank card rather than losing the save.
    ///
    /// Declines outright when the live core refused the resume it was opened with — the same
    /// condition `trusted_write` withholds from `flush`/`eject` for. This is the one durable
    /// sink that guard does not reach, because it is not a write-back over an existing file:
    /// `ring.push` is a deliberate ring buffer, and once it holds `RING_MAX` entries, pushing
    /// an eleventh evicts the oldest to make room. A core running on its own default machine
    /// has nothing worth keeping in that slot, so pushing it would not just waste an entry —
    /// it would delete a real one to make room for a placeholder. Refused the same way every
    /// other "nothing to do here" action in this file is, via `refuse()`: the player gets the
    /// same shake `load_newest`/`open_polaroids` already answer with, rather than a save that
    /// silently did not happen.
    fn save_state(&mut self) {
        let (Some(ring), Some(snapshot)) = (self.ring(), &self.snapshot) else {
            return;
        };
        if !snapshot.resume_trusted() {
            eprintln!("slot: save: the core refused the resume it was given, not pushing a state");
            return self.refuse();
        }
        let Some(state) = snapshot.state() else {
            eprintln!("slot: save: the core gave up no state");
            return;
        };
        let thumb = snapshot.thumb().unwrap_or_default();
        let stamp = free_stamp(&ring, self.wall_secs());
        let evicted = doomed(&ring);
        if let Err(e) = ring.push(&state, &thumb, &stamp) {
            eprintln!("slot: save: {e}");
            return;
        }
        self.hud.toast(Toast::StateSaved, self.now());
        self.pending = Some((PendingUndo::Save { stamp, evicted }, self.now()));
    }

    pub fn undo_available(&self, now: Millis) -> bool {
        self.pending
            .as_ref()
            .is_some_and(|(_, at)| now.saturating_sub(*at) <= UNDO_GRACE_MS)
    }

    /// What the offer says, or `None` when there is nothing on offer. The binary rasterises
    /// it; the grace period is read off the app's own clock so the two cannot disagree.
    pub fn undo_label(&self) -> Option<&'static str> {
        if !self.undo_available(self.now()) {
            return None;
        }
        match self.pending.as_ref()?.0 {
            PendingUndo::Save { .. } => Some(lang::UNDO_SAVE),
            PendingUndo::Load { .. } => Some(lang::UNDO_LOAD),
        }
    }

    /// In `LEGEND` order, and uploaded once: none of the three ever changes what it says.
    pub fn set_legend_faces(&mut self, faces: Vec<TexId>) {
        self.legend_faces = faces;
        self.push_hint_faces();
    }

    pub fn set_undo_face(&mut self, face: Option<TexId>) {
        self.undo_face = face;
        self.push_hint_faces();
    }

    /// The undo goes last because `hints` puts it last, which is what keeps faces and hints
    /// on the same index.
    fn push_hint_faces(&mut self) {
        let mut faces = self.legend_faces.clone();
        faces.extend(self.undo_face);
        if let Some(p) = &mut self.polaroids {
            p.set_hint_faces(faces);
        }
    }

    /// The mark a favourite wears, and the corner indicator on the ordinary shelf and in the
    /// favourites, each with the size it was rasterised at. Uploaded at boot: none of them changes.
    pub fn set_fav_faces(
        &mut self,
        star: (TexId, u32, u32),
        ind_off: (TexId, u32, u32),
        ind_on: (TexId, u32, u32),
    ) {
        self.fav_star = Some(star);
        self.fav_ind = Some((ind_off, ind_on));
    }

    /// The HUD glyphs, in `Icon::ALL` order. Uploaded once: they never change.
    pub fn set_icon_faces(&mut self, faces: Vec<TexId>) {
        self.hud.set_icons(faces);
    }

    /// The two lines the HUD can say, in `Toast::ALL` order.
    pub fn set_toast_faces(&mut self, faces: Vec<TexId>) {
        self.hud.set_toasts(faces);
    }

    /// What the HUD is saying, or `None` once it has faded. Only ever set by an action that
    /// happened: a refusal shakes instead.
    pub fn toast(&self) -> Option<Toast> {
        self.hud.said(self.now())
    }

    /// Show a toast from outside `App` — the in-game cheat toggle, which the session drives
    /// because the core it reaches lives on the emulator thread. `now` is read off the app's own
    /// clock so the fade lines up with everything else the HUD says.
    pub fn toast_now(&mut self, t: Toast) {
        self.hud.toast(t, self.now());
    }

    /// One shot, and it hands the game back the way loading does. Undoing an undo would be a
    /// redo, and the switcher is not a place to sit and shuffle.
    pub fn undo(&mut self, now: Millis) {
        if !self.undo_available(now) {
            self.pending = None;
            return;
        }
        // Undoing a load moves the core to a moment the peer never agreed to — the exact
        // hazard `load_file` guards against, and the one route into it that never passes
        // through `load_file` at all: the bytes are already in hand from when the load
        // happened, not read fresh off disk. Refused without consuming the offer, the same
        // way a refused rewind or state load leaves the player able to try again once the
        // session that refused it is gone — an undo's own save-file cleanup, `undo_save`
        // below, touches no core state at all, so only this arm needs the check.
        if matches!(&self.pending, Some((PendingUndo::Load { .. }, _))) && !self.may_load_state() {
            return self.refuse();
        }
        let Some((what, _)) = self.pending.take() else {
            return;
        };
        match what {
            PendingUndo::Save { stamp, evicted } => self.undo_save(&stamp, evicted),
            PendingUndo::Load { prior } => {
                if let Some(snapshot) = &self.snapshot {
                    snapshot.load(prior);
                }
            }
        }
        self.close_polaroids();
    }

    fn undo_save(&self, stamp: &str, evicted: Option<(String, Vec<u8>, Vec<u8>)>) {
        let Some(ring) = self.ring() else {
            return;
        };
        if let Err(e) = ring.remove(stamp) {
            eprintln!("slot: undo: {e}");
            return;
        }
        let Some((stamp, state, thumb)) = evicted else {
            return;
        };
        if let Err(e) = ring.push(&state, &thumb, &stamp) {
            eprintln!("slot: undo: {e}");
        }
    }

    /// Entries in the switcher's order, newest first. The binary reads these to build the
    /// faces, since only the compositor can mint a `TexId`.
    pub fn polaroid_entries(&self) -> &[StateEntry] {
        match &self.polaroids {
            Some(p) => &p.entries,
            None => &[],
        }
    }

    pub fn set_polaroid_faces(&mut self, faces: Vec<TexId>) {
        if let Some(p) = &mut self.polaroids {
            p.set_faces(faces);
        }
    }

    /// Which entry is under the eye. The binary watches this to know when the title has to be
    /// rasterised again. The stamp rather than the index, because a delete leaves the index
    /// where it was and moves a different entry under it.
    pub fn polaroid_stamp(&self) -> Option<&str> {
        self.polaroids
            .as_ref()
            .and_then(|p| p.selected())
            .map(|e| e.stamp.as_str())
    }

    /// What the top plate says. `now` is a stamp rather than the app's clock: the entries
    /// are named by their filenames and the title is relative to the wall clock.
    pub fn polaroid_title(&self, now: &str) -> String {
        self.polaroids
            .as_ref()
            .map_or_else(String::new, |p| p.title(now))
    }

    pub fn set_polaroid_title_face(&mut self, face: TexId) {
        if let Some(p) = &mut self.polaroids {
            p.set_title_face(Some(face));
        }
    }
}

/// The write-back half of the guard `EmuSnapshot` records. `state` is the bytes the live core
/// actually holds; `snapshot.resume_trusted()`/`save_ram_trusted()` say whether the core that
/// produced them actually accepted the resume/save-ram it was opened with. A region it
/// refused is withheld here — turned into `None` rather than passed on to `persist::flush`/
/// `eject` — because a core running with its own default state has nothing worth writing back
/// over the file that refusal left alone. `verb` names the caller only for the log line
/// ("flush" or "eject"), so a withheld region reads the same as everything else either one
/// already prints.
fn trusted_write(
    snapshot: &dyn Snapshot,
    state: Vec<u8>,
    verb: &str,
) -> (Option<Vec<u8>>, Option<Vec<u8>>) {
    let state = if snapshot.resume_trusted() {
        Some(state)
    } else {
        eprintln!(
            "slot: {verb}: the core refused the resume it was given, not overwriting the saved one"
        );
        None
    };
    let sav = snapshot.save_ram();
    let sav = if snapshot.save_ram_trusted() {
        sav
    } else {
        if sav.is_some() {
            eprintln!(
                "slot: {verb}: the core refused the save ram it was given, not overwriting the saved one"
            );
        }
        None
    };
    (state, sav)
}

fn up(level: u8, step: u8, max: u8) -> u8 {
    level.saturating_add(step).min(max)
}

/// The host's own clock, which is all there is before `set_power` hands over the device's.
fn system_secs() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// The entry the next push will evict, read out while it is still there. `None` until the
/// ring is full, which is where most of a cart's life is spent.
fn doomed(ring: &StateRing) -> Option<(String, Vec<u8>, Vec<u8>)> {
    let entries = ring.list().ok()?;
    let oldest = entries.get(RING_MAX - 1)?;
    let (state, thumb) = ring.read(&oldest.stamp).ok()?;
    Some((oldest.stamp.clone(), state, thumb))
}

/// The stamp is the filename, so two saves inside one second would be one save. The second
/// one moves on by a second, which keeps the ring in order without a finer format that the
/// polaroid captions would then have to read.
fn free_stamp(ring: &StateRing, now: i64) -> String {
    let taken: Vec<String> = ring
        .list()
        .map(|l| l.into_iter().map(|e| e.stamp).collect())
        .unwrap_or_default();
    // Local, from the same wall clock the captions are read against. A stamp in utc would
    // name every state an hour or several from the time the polaroid says it was taken.
    let mut secs = now;
    let mut stamp = format_stamp(secs);
    while taken.contains(&stamp) {
        secs += 1;
        stamp = format_stamp(secs);
    }
    stamp
}

/// Where a block of `rows` menu rows starts, so it sits in the middle of the panel.
fn centred_top(rows: usize) -> f32 {
    (OUT_H as f32 - POWER_MENU_PITCH * rows as f32) / 2.0
}

/// One face out of a list, as the one-row list the row drawer takes. `None` for an index
/// that has no face, which is a screen with nothing to say rather than a blank panel with a
/// highlight on it.
fn one(faces: &[(TexId, u32, u32)], index: Option<usize>) -> Vec<(TexId, u32, u32)> {
    index
        .and_then(|i| faces.get(i))
        .copied()
        .into_iter()
        .collect()
}

/// The rows of a menu, at the menu pitch from `top`, with a bar behind the one in hand and
/// none at all when nothing is. Shared by all three menus on the device: the power menu, the
/// core picker and the in-game menu are the same object, and a device this small has no
/// business carrying three copies of the loop that draws one.
///
/// The bar is `edge` — the lightest thing in the theme — because it has to read at a glance.
/// `recess` was tried first and is the right idea and the wrong value: it and `housing` are
/// adjacent dark greys by design, which is correct for a slot you look into and far too quiet
/// for a selection.
///
/// A rect rather than a second face per row: the labels are rastered once at boot and never
/// again, and a device about to lose its GPU is not the place to be uploading textures.
fn draw_menu_rows(
    faces: &[(TexId, u32, u32)],
    index: Option<usize>,
    top: f32,
    out: &mut Vec<Draw>,
) {
    for (row, (tex, w, h)) in faces.iter().copied().enumerate() {
        let y = top + POWER_MENU_PITCH * row as f32;
        let x = ((OUT_W as f32 - w as f32) / 2.0).round();
        if index == Some(row) {
            out.push(Draw::Rect {
                x,
                y: y + POWER_MENU_BAR_INSET,
                w: w as f32,
                h: POWER_MENU_PITCH - 2.0 * POWER_MENU_BAR_INSET,
                colour: slot_ui::edge(),
            });
        }
        out.push(Draw::Tex {
            x,
            y: y + (POWER_MENU_PITCH - h as f32) / 2.0,
            w: w as f32,
            h: h as f32,
            tex,
            alpha: 1.0,
        });
    }
}
