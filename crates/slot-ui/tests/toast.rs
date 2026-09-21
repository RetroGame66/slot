use slot_ui::lang;
use slot_ui::text::{missing_in, set_font};
use slot_ui::{toast_face, toast_rect, Draw, Hud, HudKind, Toast, OUT_W, PLATE_H};

/// The face the app loads off the card at boot, by the path the deploy puts it at.
///
/// A test binary boots with no card, so `label_font` can only hand back the embedded face — and
/// that face carries no CJK, while the Chinese build's toasts are all Chinese. Anything looking at
/// rasterised type therefore has to bring the card's face with it, which is what this does.
///
/// The English build is the other way round and needs none of it: its toasts are Latin and the
/// embedded face covers them. So what is asked below is whether the *actual* string has a face,
/// rather than whether a Chinese one does — and the English run answers yes on the spot.
const CARD_FONT: &str = "/mnt/sdcard/System/fonts/NotoSansCJKsc-Bold.otf";

/// True when the process now has a face that can draw the toasts. Loads the card's if it has to.
fn with_a_cjk_face() -> bool {
    let sample = Toast::StateSaved.text();
    if missing_in(sample).is_empty() {
        return true;
    }
    match std::fs::read(CARD_FONT) {
        Ok(bytes) => set_font(bytes),
        Err(_) => return false,
    }
    missing_in(sample).is_empty()
}

/// Both name what happened, and they name *different* things — which is the whole of what a toast
/// is for. Compared against the table rather than against a literal so the wording can change, and
/// so this runs in either language: it is the wiring and the distinction being held, not the
/// spelling.
#[test]
fn saving_and_loading_say_which_one_happened() {
    assert_eq!(Toast::StateSaved.text(), lang::TOAST_STATE_SAVED);
    assert_eq!(Toast::StateLoaded.text(), lang::TOAST_STATE_LOADED);
    assert_ne!(
        Toast::StateSaved.text(),
        Toast::StateLoaded.text(),
        "the two toasts say the same thing"
    );
}

#[test]
fn a_toast_fades_on_the_same_curve_as_the_bar() {
    let mut h = Hud::new();
    h.toast(Toast::StateSaved, 1_000);
    assert!(h.toast_visible(2_499));
    assert!(!h.toast_visible(2_500));
}

#[test]
fn a_toast_is_centred() {
    let (x, _, w, _) = toast_rect();
    assert_eq!(x + w / 2.0, OUT_W as f32 / 2.0);
}

/// Nothing backs the type, so the type carries its own contrast or it disappears on a white
/// game frame. Same halo the badge beside it uses.
///
/// Skipped rather than failed where the card's face is not reachable: the halo is a property of
/// the rasteriser, and asserting it against a face that draws no type at all would be a report
/// about the test machine, not about the halo.
#[test]
fn a_toast_carries_its_own_halo() {
    if !with_a_cjk_face() {
        eprintln!("no CJK face on this machine; the halo is checked on a device with a card");
        return;
    }
    let f = toast_face(Toast::StateSaved);
    assert!(!f.rgba.is_empty(), "the toast rasterised to nothing");
    let dark = f
        .rgba
        .chunks(4)
        .any(|p| p[3] > 0 && p[0] < 0x40 && p[1] < 0x40 && p[2] < 0x40);
    assert!(dark, "there is nothing dark behind the type");
}

/// The toast reads against the same plate the level bar does, in the same place. It used to
/// sit below the band with no backing, which put two different treatments on one screen.
#[test]
fn a_toast_sits_in_the_plate_band_and_is_backed_by_it() {
    let mut h = Hud::new();
    h.toast(Toast::StateSaved, 0);
    let mut out = Vec::new();
    // From the top of the screen: this is checking where the toast sits inside its plate, not
    // where the plate hangs, which `hud.rs` covers.
    h.draw(0, 0.0, &mut out);

    let plate = out
        .iter()
        .find(|d| matches!(d, Draw::Rect { w, .. } if *w == OUT_W as f32))
        .expect("the toast has nothing to be read against");
    let Draw::Rect { colour, h: ph, .. } = plate else {
        unreachable!()
    };
    assert!(colour[3] > 0.6, "the plate is too faint to give contrast");
    assert!((*ph - PLATE_H).abs() < 0.01, "the plate is not the band");

    let (_, y, _, th) = toast_rect();
    assert!(
        y >= 0.0 && y + th <= PLATE_H,
        "the toast at {y} is outside the band"
    );
}

/// They share one strip, so only one can have it. A toast names something that just
/// happened; a level is visible in its own effect.
#[test]
fn a_toast_takes_the_band_from_the_bar() {
    let mut h = Hud::new();
    h.show(HudKind::Volume, 50, false, 0);
    let mut bar_only = Vec::new();
    h.draw(0, 0.0, &mut bar_only);
    let bars = bar_only.len();

    h.toast(Toast::StateSaved, 0);
    let mut both = Vec::new();
    h.draw(0, 0.0, &mut both);
    assert!(
        both.len() < bars,
        "the bar is still drawn underneath the toast"
    );
}
