mod art;
mod backdrop;
mod barcode;
mod battery;
mod board;
mod cart;
mod clock;
mod draw;
mod hud;
mod icon;
pub mod lang;
pub mod letters;
pub mod palette;
mod plate;
mod polaroids;
mod power_menu;
mod refusal;
mod shelf;
mod shell;
mod shortcuts;
mod silhouette;
mod slot_chrome;
mod status;
mod sticker;
pub mod text;
mod toast;

pub use backdrop::{draw_backdrop, wallpaper_face};
pub use barcode::{code39, CODE39_NARROW, CODE39_WIDE};
pub use battery::{capsule_left, cluster_h, draw_gauge, BOLT_PX, GAUGE_H, GAUGE_W, WALL};
pub use board::{
    board_at, board_face, board_zoom, chip_face, chip_shadow_face, grown, lid_at, lift_of,
    on_board, padded, rom_marking, rom_marking_face, shelf_cart, slide_of, socket_face, Placed,
    BOARD_H, BOARD_W, BOARD_X, BOARD_Y, CHIP_H, CHIP_TIP, CHIP_U, CHIP_V, CHIP_W, HOP_LIFT,
    LID_TURN, ROM_H, ROM_W, ROM_X, ROM_Y, SHADOW_H, SHADOW_W, SLIDE_SHARE, SLIDE_UP, SOCKET_H,
    SOCKET_U, SOCKET_V, SOCKET_W, TURN_PAD,
};
pub use cart::face_profile;
// Re-exported rather than defined here: the rule that turns a file name into a title belongs
// with the scan, because the ring's letter is decided from it and a scan cannot ask the ui.
pub use slot_store::{clean_label, label_tags};

pub use cart::{
    cart_face, cart_placeholder, cart_shadow, label_colour, label_panel, label_text, CartFace,
    CART_H, CART_W, FACE_H, FACE_SCALE, FACE_W, LABEL_H, LABEL_W, LABEL_X, LABEL_Y,
};
pub use clock::{clock_label, hhmm, set_clock_hint_face, ClockPicker, Field};
pub use draw::{Draw, TexId, OUT_H, OUT_W};
pub use hud::{ff_badge, FfState, Hud, HudKind, Millis, HUD_ICON_PX, HUD_MS, PLATE_H, PLATE_Y};
pub use icon::{icon_box, icon_face, Icon};
pub use letters::Letters;
pub use palette::Mode;
pub use plate::{
    arrows_hint_face, arrows_hint_width, cap_width, clock_face, clock_width, hint_face, hint_quad,
    hint_row, hint_width, shelf_title_face, title_face, word_face, word_width, Hint, UndoFace,
    ARROW_GAP, CAP, CAP_GAP, HINT_EDGE, HINT_GAP, HINT_H, SHELF_TITLE_H, SHELF_TITLE_W, TITLE_H,
    TITLE_W,
};
pub use polaroids::{photo_face, PhotoFace, Polaroids, DOT, LEGEND, PHOTO_H, PHOTO_W};
pub use power_menu::{cheat_row_face, menu_face, PowerChoice, MENU_PAD};
pub use refusal::Refusal;
pub use shelf::{Shelf, CENTER_SCALE, SIDE_ALPHA};
pub use shell::{
    lookup_order_is_exact_then_family_then_default, shell_for, table_keys, Finish, Shell,
    DEFAULT_SHELL,
};
pub use shortcuts::{
    draw_row as draw_shortcut_row, hint_face as shortcut_hint_face, row_face as shortcut_row_face,
    Row as ShortcutRow, HINT as SHORTCUT_HINT, ROWS as SHORTCUT_ROWS, ROW_W as SHORTCUT_ROW_W,
};
pub use silhouette::silhouette;
pub use slot_chrome::{
    draw_edge_glow, draw_empty_slot, draw_top_band, ease, edge, housing, opening, recess,
    set_theme, SlotChrome, ALERT_PX, LIP_H, MOUTH_H, MOUTH_W, TOP_BAND_H, TOP_WIN_H, TOP_WIN_W,
    TOP_WIN_X, TOP_WIN_Y,
};
pub use status::{draw_status, Printed};
pub use sticker::{
    draw_sticker, draw_sticker_at, head_rows, sticker_face, sticker_lines, StickerFields,
    COPYRIGHT, CREDITS, DC, HOME, ORIGIN, STICKER_H, STICKER_W,
};
pub use toast::{toast_box, toast_face, toast_rect, Toast};
