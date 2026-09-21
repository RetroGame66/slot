use slot_ui::{
    ff_badge, icon_face, Draw, FfState, Hud, HudKind, Icon, Toast, OUT_H, OUT_W, PLATE_H, PLATE_Y,
    TOP_BAND_H,
};

fn top_edge(d: &Draw) -> Option<f32> {
    match *d {
        Draw::Rect { y, .. } | Draw::Tex { y, .. } | Draw::Turned { y, .. } => Some(y),
        Draw::Game | Draw::Shot { .. } => None,
    }
}

fn bottom_edge(d: &Draw) -> Option<f32> {
    match *d {
        Draw::Rect { y, h, .. } | Draw::Tex { y, h, .. } | Draw::Turned { y, h, .. } => Some(y + h),
        Draw::Game | Draw::Shot { .. } => None,
    }
}

/// Width of the filled part of the bar, which is the last rect in the list.
fn fill(kind: HudKind, value: u8) -> f32 {
    let mut h = Hud::new();
    h.show(kind, value, false, 0);
    let mut out = Vec::new();
    h.draw(0, 0.0, &mut out);
    match out.last().expect("no bar in the list") {
        Draw::Rect { w, .. } => *w,
        _ => panic!("the bar must be a rect"),
    }
}

#[test]
fn hud_fades_after_1500ms() {
    let mut h = Hud::new();
    h.show(HudKind::Volume, 5, false, 1000);
    assert!(h.visible(2499));
    assert!(!h.visible(2500));
}

/// A zero timestamp is not an adjustment. Without this every boot opens with a bar on it.
#[test]
fn nothing_shows_until_something_is_adjusted() {
    assert!(!Hud::new().visible(0));
}

#[test]
fn volume_reads_against_100_not_the_brightness_scale() {
    let full = fill(HudKind::Brightness, slot_store::BRIGHTNESS_MAX);
    assert!(
        fill(HudKind::Volume, 9) < full * 0.2,
        "volume 9 filled the bar, so it is being read against the brightness scale"
    );
    assert_eq!(fill(HudKind::Volume, 100), full);
}

/// On a screen with nothing at the top of it the plate takes the top edge; on the shelf it
/// hangs off the letter band instead, and the whole readout goes with it.
#[test]
fn the_hud_sits_at_the_top_of_the_screen() {
    let mut h = Hud::new();
    h.show(HudKind::Volume, 50, false, 0);
    let mut out = Vec::new();
    h.draw(0, 0.0, &mut out);
    let lowest = out.iter().filter_map(bottom_edge).fold(0.0f32, f32::max);
    assert!(
        lowest < OUT_H as f32 / 2.0,
        "hud reaches {lowest}, expected the top half"
    );
    let top = out.iter().filter_map(top_edge).fold(f32::MAX, f32::min);
    assert!(
        top.abs() < 0.01,
        "the plate does not start at the top: {top}"
    );
}

/// The shelf's band is at the top of the screen, and a plate over it would hide the index and
/// the clock for the second and a half a level is up. So the plate starts where the band ends,
/// and nothing it draws reaches back up into the band.
#[test]
fn the_plate_hangs_under_the_letter_band() {
    let mut h = Hud::new();
    h.show(HudKind::Volume, 50, false, 0);
    let mut out = Vec::new();
    h.draw(0, PLATE_Y, &mut out);

    let top = out.iter().filter_map(top_edge).fold(f32::MAX, f32::min);
    assert!(
        (top - PLATE_Y).abs() < 0.01,
        "the plate does not start at the band's lower edge: {top}"
    );
    assert!(
        (PLATE_Y - TOP_BAND_H).abs() < 0.01,
        "the plate is not hung off the band's own height"
    );
    // Nothing crosses back into the band, which would be the defect in a different shape.
    for d in &out {
        let t = top_edge(d).expect("everything the hud draws is a quad");
        assert!(
            t >= PLATE_Y - 0.01,
            "something the hud drew reaches into the band: {t}"
        );
    }
    // And the readout still fits inside the plate it is read against.
    let bottom = out.iter().filter_map(bottom_edge).fold(0.0f32, f32::max);
    assert!(
        bottom <= PLATE_Y + PLATE_H + 0.01,
        "the readout overflows its own plate: {bottom}"
    );
    // A toast is the other thing read against the plate, and `toast_rect` places it inside a
    // plate standing at zero. It has to travel with the plate all the same, so the offset is
    // applied where the plate's own top is known. Checked here without a face, which is the
    // part of it that is arithmetic.
    let mut toast = Vec::new();
    let mut h = Hud::new();
    h.toast(Toast::StateSaved, 0);
    h.draw(0, PLATE_Y, &mut toast);
    let t = toast.iter().filter_map(top_edge).fold(f32::MAX, f32::min);
    assert!(
        (t - PLATE_Y).abs() < 0.01,
        "the plate did not travel with the toast: {t}"
    );
}

/// Over a white game frame a white bar on a translucent white track is invisible.
#[test]
fn the_hud_draws_a_dark_plate_behind_itself() {
    let mut h = Hud::new();
    h.show(HudKind::Brightness, 5, false, 0);
    let mut out = Vec::new();
    h.draw(0, 0.0, &mut out);
    let plate = out.first().expect("nothing drawn");
    let Draw::Rect { w, colour, .. } = plate else {
        panic!("first draw is not the plate")
    };
    assert_eq!(*w, OUT_W as f32, "the plate does not span the screen");
    assert!(
        colour[0] < 0.2 && colour[1] < 0.2 && colour[2] < 0.2,
        "the plate is not dark"
    );
    assert!(
        colour[3] > 0.6,
        "the plate is too transparent to give contrast"
    );
}

#[test]
fn muted_volume_uses_the_muted_icon() {
    // Turned down to nothing, silenced, and neither: three states and three glyphs. Zero and
    // muted draw the same empty bar, so the glyph is the only thing carrying the difference.
    assert_eq!(HudKind::Volume.icon(0, false), Icon::VolumeZero);
    assert_eq!(HudKind::Volume.icon(0, true), Icon::VolumeMuted);
    assert_eq!(HudKind::Volume.icon(40, true), Icon::VolumeMuted);
    assert_eq!(HudKind::Volume.icon(40, false), Icon::Volume);
}

#[test]
fn the_rewind_bar_is_held_open_rather_than_fading() {
    let mut h = Hud::new();
    h.show(HudKind::Rewind, 80, false, 0);
    assert!(
        h.visible(60_000),
        "the rewind bar timed out while still held"
    );
    h.release_rewind();
    assert!(!h.visible(60_000));
}

#[test]
fn an_empty_rewind_buffer_still_draws_an_empty_bar() {
    let mut h = Hud::new();
    h.show(HudKind::Rewind, 0, false, 0);
    let mut out = Vec::new();
    h.draw(0, 0.0, &mut out);
    assert!(
        out.len() >= 2,
        "the track disappeared when the buffer emptied"
    );
}

/// L2 can be let go while a level bar is still on its own timer. Only the rewind bar leaves
/// with it.
#[test]
fn releasing_rewind_leaves_a_level_bar_alone() {
    let mut h = Hud::new();
    h.show(HudKind::Volume, 50, false, 0);
    h.release_rewind();
    assert!(h.visible(0));
}

/// One glyph in both states, and a different one in each. Two glyphs for the latch would
/// widen the badge and shift it the moment it locked.
#[test]
fn held_and_latched_fast_forward_are_one_glyph_each_and_differ() {
    let held = ff_badge(FfState::Held).expect("held draws nothing");
    let latched = ff_badge(FfState::Latched).expect("latched draws nothing");
    assert_ne!(
        held, latched,
        "the latch is indistinguishable from the hold"
    );
    assert_eq!(ff_badge(FfState::Off), None);
}

/// Same footprint, so latching cannot make the badge jump.
#[test]
fn both_fast_forward_glyphs_share_one_box() {
    let held = icon_face(Icon::FastForward, 18.0, [255, 255, 255]);
    let latched = icon_face(Icon::FastForwardLatched, 18.0, [255, 255, 255]);
    assert_eq!((held.w, held.h), (latched.w, latched.h));
    assert_ne!(
        held.rgba, latched.rgba,
        "the two variants rasterise identically"
    );
}

/// Fast forward is not a level, so the bar's own timer never starts and nothing fades it
/// out. The badge has to survive that on its own, without dragging a plate up with it.
#[test]
fn the_badge_outlives_the_bar_timer_without_a_plate() {
    let mut h = Hud::new();
    h.set_ff(FfState::Latched);
    let mut out = Vec::new();
    h.draw(60_000, 0.0, &mut out);
    assert!(
        !out.iter()
            .any(|d| matches!(d, Draw::Rect { w, .. } if *w == OUT_W as f32)),
        "the badge dragged the full width plate up with it"
    );
    assert!(
        ff_badge(FfState::Latched).is_some(),
        "the badge itself went away"
    );
}

#[test]
fn the_badge_leaves_when_fast_forward_stops() {
    let mut h = Hud::new();
    h.set_ff(FfState::Held);
    h.set_ff(FfState::Off);
    let mut out = Vec::new();
    h.draw(0, 0.0, &mut out);
    assert!(out.is_empty(), "the plate outlived the fast forward");
}

/// Fast forward can be latched for minutes. A full width plate over the game for all of it
/// is a worse trade than the halo the icon carries, so the badge stands alone. The pair
/// matters together: the second half is what stops this passing by drawing no plate ever.
#[test]
fn the_ff_badge_draws_no_plate_but_the_bar_still_does() {
    let full = |out: &Vec<Draw>| {
        out.iter()
            .filter(|d| matches!(d, Draw::Rect { w, .. } if *w == OUT_W as f32))
            .count()
    };

    let mut badge_only = Hud::new();
    badge_only.set_ff(FfState::Held);
    let mut out = Vec::new();
    badge_only.draw(60_000, 0.0, &mut out);
    assert_eq!(
        full(&out),
        0,
        "the badge is still drawing the full width plate"
    );

    let mut with_bar = Hud::new();
    with_bar.show(HudKind::Volume, 50, false, 0);
    let mut out = Vec::new();
    with_bar.draw(0, 0.0, &mut out);
    assert_eq!(full(&out), 1, "the bar lost the plate it is read against");
}
