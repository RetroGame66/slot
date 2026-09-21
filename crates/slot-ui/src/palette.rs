//! The two ways the frontend is printed, and the one switch between them.
//!
//! Everything on the shelf was written in a single set of colours and those colours were
//! constants: `letters::INK`, `hud::HUD_INK`, `plate::INK`, `clock::INK`, `battery::INK`, the
//! card's four case colours in `theme.txt`, and one black scrim over the wallpaper. Six copies
//! of one idea, each in the file that happened to need it, which is why a mode could not exist
//! and why this module now owns all six.
//!
//! # Why a global and not a parameter
//!
//! The colours are read in some forty places, from the shelf's own draw to a dozen face
//! builders that run on `FaceBuilder`'s thread while the main loop keeps drawing. Threading a
//! palette through all of them would mean a parameter on every one of those functions, on both
//! sides of a thread boundary, to express a value that has exactly one answer at any moment.
//! An atomic is the honest shape of that: the mode is a fact about the device, not an argument
//! to a function.
//!
//! # Why switching is not free
//!
//! Most of the type on this device is not drawn, it is *baked*: a string is rasterised once
//! into a texture with the ink burned into it. The ink therefore cannot change under a face
//! that already exists, and the binary has to re-bake when the mode moves — see
//! `Frontend::upload_faces`, which is the one place that does it. What does follow the mode for
//! free is everything drawn as a filled quad: the case itself, the scrim, the gauge and the
//! HUD's bar.

use std::sync::atomic::{AtomicU8, Ordering};

use slot_store::Theme;
// Re-exported rather than defined here: the mode is a line in the state file before it is a
// palette, and `slot-store` is what reads and writes that file.
pub use slot_store::Mode;

static MODE: AtomicU8 = AtomicU8::new(0);

pub fn mode() -> Mode {
    match MODE.load(Ordering::Relaxed) {
        1 => Mode::Light,
        _ => Mode::Dark,
    }
}

pub fn set_mode(m: Mode) {
    MODE.store(
        match m {
            Mode::Dark => 0,
            Mode::Light => 1,
        },
        Ordering::Relaxed,
    );
}

// ---------------------------------------------------------------------------------------
// The case
// ---------------------------------------------------------------------------------------

/// The light case, in the same four roles the card's `theme.txt` addresses.
///
/// Not derived from the dark one by scaling, and that is the whole point of writing it out: a
/// pure inversion of `housing #242429` gives a near-black `opening`, so the letters would be
/// dark type on a black window — the one combination that is unreadable in either mode. The
/// four roles are what carry over, not the numbers: the housing is the outer plastic, the
/// recess is a step down from it, the opening is what you see *into*, and the edge is the lit
/// line where the plastic is cut. Each stays on the side of the housing that makes it read as
/// what it is, which is why the opening stays *darker* than the housing even though the
/// housing has gone light.
///
/// A warm grey rather than a white: the scrim over the wallpaper lands at about `#e8e5e0`, and
/// plastic the same tone as the ground behind it stops being an object on it.
///
/// `theme.txt` addresses the dark case and only the dark case: it is the file a card ships to
/// say what colour its machine is, and the light palette is the *same* machine seen the other
/// way up rather than a second theme to be overridden in four more names. `slot_chrome` chooses
/// between the two — it is the only thing that holds the card's theme — and this is only the
/// light half of that answer.
pub const LIGHT_CASE: Theme = Theme {
    housing: [0xcf, 0xc8, 0xbb],
    recess: [0xbf, 0xb8, 0xaa],
    opening: [0xb0, 0xa8, 0x9a],
    edge: [0xf6, 0xf3, 0xec],
};

// ---------------------------------------------------------------------------------------
// The type
// ---------------------------------------------------------------------------------------

/// The ink every letter, title, hint and number is set in.
pub fn ink() -> [u8; 3] {
    match mode() {
        Mode::Dark => [0xf5, 0xf2, 0xef],
        Mode::Light => [0x1c, 0x1b, 0x1f],
    }
}

pub fn ink_f() -> [f32; 4] {
    let c = ink();
    [
        c[0] as f32 / 255.0,
        c[1] as f32 / 255.0,
        c[2] as f32 / 255.0,
        1.0,
    ]
}

/// The ink *on* an ink-coloured panel: a key cap's letter, which has to be the ground's colour
/// for the cap to read as a key in either mode. Dark mode is a light cap with a dark letter and
/// the light mode is a dark cap with a light one, so this is not "a darker ink" — it is the
/// opposite of `ink`, and the only value in here that is defined against another.
pub fn panel_ink() -> [u8; 3] {
    match mode() {
        Mode::Dark => [0x1a, 0x19, 0x17],
        Mode::Light => [0xf4, 0xf1, 0xea],
    }
}

/// The one pixel halo behind a glyph that sits on a picture with nothing under it — the HUD's
/// icons and the toast's own outline. It exists to separate type from whatever is behind it, so
/// it is the ground's colour and it flips with everything else.
pub fn halo() -> [u8; 3] {
    match mode() {
        Mode::Dark => [0x08, 0x08, 0x0a],
        Mode::Light => [0xf2, 0xef, 0xe8],
    }
}

/// The veil the HUD's bar and its toasts are read against.
///
/// Not the scrim's colour and not the housing's either, though in the light it is close to the
/// latter. Its job is to separate type from a moving picture, so it has to be a colour no
/// game frame is: black in the dark, the case's own light plastic in the light, both at an
/// alpha that leaves the picture visible underneath rather than erasing it.
pub fn plate() -> [f32; 4] {
    match mode() {
        Mode::Dark => [0.0, 0.0, 0.0, 0.72],
        Mode::Light => [0.81, 0.78, 0.73, 0.82],
    }
}

/// An opaque panel that type is set on, as opposed to `plate`, which is a translucent veil over
/// a picture. The clock picker is a screen of its own rather than something laid over one, so
/// what is behind it must not show through.
pub fn panel() -> [f32; 4] {
    match mode() {
        Mode::Dark => [0.06, 0.06, 0.07, 1.0],
        Mode::Light => [0.88, 0.86, 0.82, 1.0],
    }
}

/// A key cap on the shortcut card, where the caps are drawn as filled squares rather than as
/// the light plates a hint's cap is.
///
/// Mid grey in the dark and darker in the light, which looks backwards until you see what it
/// is drawn on: the card is a light panel in both modes, so the cap has to be *darker* than the
/// card in both. It is the same rule as `panel_ink` read one level out — a cap separates from
/// what it sits on, whichever way round the rest of the screen is.
pub fn keycap() -> [u8; 3] {
    match mode() {
        Mode::Dark => [0xb4, 0xb0, 0xa8],
        Mode::Light => [0x8a, 0x85, 0x7c],
    }
}

/// The flat the screen is when the card carries no wallpaper. The wallpaper-facing twin of
/// this is `scrim`, and the two are the same colour seen two ways: black and paper.
pub fn ground() -> [u8; 3] {
    match mode() {
        Mode::Dark => [0x00, 0x00, 0x00],
        Mode::Light => [0xe6, 0xe3, 0xdd],
    }
}

/// Type that is present but is not the thing being read: a shortcut's description, one step
/// back from the head above it.
///
/// Defined as a step towards the ground rather than as a fraction of the ink, and that is the
/// only reason it is a function at all: "dimmer" is towards the ground in both modes, but the
/// ground is black in one and paper in the other, so a fixed fraction of the ink would make the
/// description *brighter* than the head in the light mode and the hierarchy would invert with
/// the palette.
pub fn dim_ink() -> [u8; 3] {
    mix(ink(), ground(), 0.24)
}

fn mix(a: [u8; 3], b: [u8; 3], t: f32) -> [u8; 3] {
    let mut out = [0u8; 3];
    for i in 0..3 {
        out[i] = (a[i] as f32 + (b[i] as f32 - a[i] as f32) * t).round() as u8;
    }
    out
}

/// The metal between two letters on the index strip, and the tooth on the end of it.
///
/// The one colour in here that is not type and not a surface: it is a highlight standing on the
/// window, so it goes the same way as the light rather than the same way as the ink. Light metal
/// on a dark window in the dark mode, dark metal on a pale one in the light — the same ridge,
/// seen against whatever the window has become.
pub fn ridge_ink() -> [u8; 3] {
    match mode() {
        Mode::Dark => [0xd8, 0xd4, 0xcc],
        Mode::Light => [0x6b, 0x64, 0x5b],
    }
}

/// The veil over the wallpaper: what the picture is taken back out by, and in which direction.
///
/// Black in the dark and white in the light, and the light one is stronger. Not symmetric, and
/// it is worth saying why: black at 0.62 leaves a photograph that is still a photograph, which
/// is what a dark ground needs to stay one. White at 0.62 leaves a pale wash with the picture's
/// own contrast showing through as grey blotches, which is exactly what `tests/contrast.rs`
/// exists to keep out of the ground the case is printed on.
pub fn scrim() -> [f32; 4] {
    match mode() {
        Mode::Dark => [0.0, 0.0, 0.0, 0.62],
        Mode::Light => [1.0, 1.0, 1.0, 0.74],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every test in the crate shares one mode, so a test that moves it has to put it back or
    /// the next one to run sees a light palette it never asked for.
    struct Restore(Mode);

    impl Drop for Restore {
        fn drop(&mut self) {
            set_mode(self.0);
        }
    }

    fn pinned(m: Mode) -> Restore {
        let was = mode();
        set_mode(m);
        Restore(was)
    }

    #[test]
    fn the_ink_inverts_and_the_panel_ink_is_its_opposite() {
        let _pin = pinned(Mode::Dark);
        assert_eq!(ink(), [0xf5, 0xf2, 0xef]);
        assert_eq!(panel_ink(), [0x1a, 0x19, 0x17]);

        set_mode(Mode::Light);
        assert_eq!(ink(), [0x1c, 0x1b, 0x1f]);
        assert_eq!(panel_ink(), [0xf4, 0xf1, 0xea]);

        // The one property that has to hold in both: a cap's letter is never the same colour
        // as the cap, or the key is a blank square.
        for m in [Mode::Dark, Mode::Light] {
            set_mode(m);
            assert_ne!(ink(), panel_ink(), "{m:?} draws a letter on its own ink");
        }
    }

    #[test]
    fn the_ground_is_always_on_the_other_side_of_the_type() {
        // The scrim is the ground the case is printed on, and the ink has to be legible on it
        // in both modes. Stated as one number — how far apart they are — because "readable" is
        // not otherwise a thing a test can hold, and a palette edit is exactly the change that
        // would quietly break it.
        //
        // The wallpaper behind the scrim is taken as mid grey, which is the worst case for
        // both modes: black at 0.62 leaves a light picture dark and white at 0.74 leaves a dark
        // picture light, so a mid grey is the ground least like the scrim's own colour.
        for m in [Mode::Dark, Mode::Light] {
            let _pin = pinned(m);
            let s = scrim();
            let ground = s[0] * 255.0 * s[3] + (1.0 - s[3]) * 128.0;
            let ink_lum = lum(ink());
            assert!(
                (ink_lum - ground).abs() > 60.0,
                "{m:?}: ink {ink_lum:.0} on a ground of {ground:.0} is too close to read"
            );
        }
    }

    #[test]
    fn the_light_case_keeps_the_roles_the_dark_one_has() {
        // The housing is the plastic, the recess is a step *down* from it and the opening is
        // further in still; the edge is the lit line and so is lighter than all of them. Which
        // way round the housing itself sits is the mode's business — that is the whole point of
        // the light palette — but the order of the three surfaces is not.
        for (name, t) in [("dark", Theme::default()), ("light", LIGHT_CASE)] {
            assert!(
                lum(t.recess) < lum(t.housing),
                "{name}: the recess is not below the housing"
            );
            assert!(
                lum(t.opening) < lum(t.recess),
                "{name}: the opening is not inside the recess"
            );
            assert!(
                lum(t.edge) > lum(t.housing),
                "{name}: the lit edge is not the lightest of the four"
            );
        }
    }

    #[test]
    fn the_type_is_legible_on_the_case_in_both_modes() {
        // The letters sit in the band's window, which is `opening`, and the clock and the gauge
        // sit on the housing. Both pairs have to separate, in both modes, or a mode is a
        // palette that only looks right in the mock.
        for m in [Mode::Dark, Mode::Light] {
            let _pin = pinned(m);
            let t = match m {
                Mode::Dark => Theme::default(),
                Mode::Light => LIGHT_CASE,
            };
            for (where_, surface) in [("the window", t.opening), ("the housing", t.housing)] {
                let gap = (lum(ink()) - lum(surface)).abs();
                assert!(gap > 90.0, "{m:?}: ink on {where_} is only {gap:.0} apart");
            }
        }
    }

    fn lum(c: [u8; 3]) -> f32 {
        0.299 * c[0] as f32 + 0.587 * c[1] as f32 + 0.114 * c[2] as f32
    }
}
