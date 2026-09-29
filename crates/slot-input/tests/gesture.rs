use slot_input::RawEvent::{Down, Up};
use slot_input::{
    Action::*, Btn, Btn::*, Gestures, RawEvent, COLOR_HOLD_MS, MENU_HOLD_MS, POWER_HOLD_MS,
    SELECT_CHORD_MS, SELECT_TAP_MS, VOLUME_REPEAT_DELAY_MS, VOLUME_REPEAT_MS,
};

#[test]
fn select_alone_reaches_the_game_after_the_chord_window() {
    let mut g = Gestures::new();
    assert!(g.feed(Down(Select), 0).is_empty()); // deferred, not swallowed
    assert!(g.tick(SELECT_CHORD_MS - 1).is_empty());
    assert_eq!(g.tick(SELECT_CHORD_MS), vec![GbaDown(Select)]);
}

#[test]
fn select_chord_swallows_select_entirely() {
    let mut g = Gestures::new();
    g.feed(Down(Select), 0);
    assert_eq!(g.feed(Down(R1), 50), vec![SaveState]);
    assert!(g.tick(500).is_empty()); // SELECT never reaches the game
    assert!(g.feed(Up(R1), 60).is_empty());
    assert!(g.feed(Up(Select), 70).is_empty());
}

#[test]
fn select_chords_map_to_all_four_axes() {
    for (btn, want) in [
        (Btn::Up, BrightnessUp),
        (Btn::Down, BrightnessDown),
        (Right, BlueLightUp),
        (Left, BlueLightDown),
    ] {
        let mut g = Gestures::new();
        g.feed(RawEvent::Down(Select), 0);
        assert_eq!(g.feed(RawEvent::Down(btn), 10), vec![want]);
    }
}

#[test]
fn select_and_volume_keys_switch_the_audio_profile() {
    for (btn, want) in [
        (Btn::VolUp, AudioProfileNext),
        (Btn::VolDown, AudioProfilePrev),
    ] {
        let mut g = Gestures::new();
        g.feed(RawEvent::Down(Select), 0);
        assert_eq!(g.feed(RawEvent::Down(btn), 10), vec![want]);
    }
}

/// SELECT+START prints the device the other way round.
///
/// START is also the shelf's core picker on its own, which is what makes this worth holding at
/// the gesture layer rather than only at the app's: the chord has to swallow the press, or the
/// picker would open under the mode change and the shelf would come back with its lid off.
#[test]
fn select_and_start_toggle_the_mode() {
    let mut g = Gestures::new();
    g.feed(RawEvent::Down(Select), 0);
    assert_eq!(g.feed(RawEvent::Down(Btn::Start), 10), vec![ModeToggle]);
    // The release of the chorded key is swallowed with the press: a bare START is a gesture the
    // shelf answers, and half of one arriving after the chord would be a second gesture.
    assert!(g.feed(RawEvent::Up(Btn::Start), 60).is_empty());
    assert!(g.feed(RawEvent::Up(Select), 70).is_empty());
}

/// And with no SELECT down, START is the picker's again — the chord does not take the button.
#[test]
fn start_alone_is_still_the_game_button() {
    let mut g = Gestures::new();
    assert_eq!(
        g.feed(RawEvent::Down(Btn::Start), 0),
        vec![GbaDown(Btn::Start)]
    );
    assert_eq!(
        g.feed(RawEvent::Up(Btn::Start), 30),
        vec![GbaUp(Btn::Start)]
    );
}

#[test]
fn select_released_inside_the_window_still_reaches_the_game() {
    let mut g = Gestures::new();
    g.feed(Down(Select), 0);
    assert_eq!(g.feed(Up(Select), 40), vec![GbaDown(Select)]);
    assert_eq!(g.tick(40 + SELECT_TAP_MS), vec![GbaUp(Select)]);
    assert!(g.tick(1_000).is_empty());
}

/// A tap of MENU opens the about screen, on the release. The other two MENU gestures are
/// unchanged: a tap was the one press this button did not already mean something by.
#[test]
fn menu_single_tap_opens_the_about_screen() {
    let mut g = Gestures::new();
    assert!(g.feed(Down(Menu), 0).is_empty(), "acted on the press");
    assert_eq!(g.feed(Up(Menu), 100), vec![OpenAbout]);
    assert!(g.tick(451).is_empty(), "fired a second time on the timer");
}

/// The hold is an eject and nothing else. A press long enough to eject is not also a tap, or
/// letting go of one would drop the about screen over the shelf the cart just came back to.
#[test]
fn a_menu_hold_is_not_also_a_tap() {
    let mut g = Gestures::new();
    g.feed(Down(Menu), 0);
    assert_eq!(g.tick(MENU_HOLD_MS), vec![Eject]);
    assert!(g.feed(Up(Menu), MENU_HOLD_MS + 50).is_empty());
}

#[test]
fn menu_double_tap_opens_polaroids_immediately() {
    let mut g = Gestures::new();
    g.feed(Down(Menu), 0);
    g.feed(Up(Menu), 100);
    assert_eq!(g.feed(Down(Menu), 300), vec![Polaroids]);
}

#[test]
fn menu_hold_ejects_at_the_hold_time_and_not_before() {
    let mut g = Gestures::new();
    g.feed(Down(Menu), 0);
    assert!(g.tick(MENU_HOLD_MS - 1).is_empty());
    assert_eq!(g.tick(MENU_HOLD_MS), vec![Eject]);
}

#[test]
fn menu_hold_released_early_ejects_nothing() {
    let mut g = Gestures::new();
    g.feed(Down(Menu), 0);
    g.feed(Up(Menu), MENU_HOLD_MS - 200);
    assert!(g.tick(3000).is_empty());
}

#[test]
fn r2_hold_is_momentary() {
    let mut g = Gestures::new();
    assert_eq!(g.feed(Down(R2), 0), vec![FfStart]);
    assert_eq!(g.feed(Up(R2), 900), vec![FfStop]);
}

#[test]
fn r2_double_tap_latches_and_single_press_clears() {
    let mut g = Gestures::new();
    g.feed(Down(R2), 0);
    g.feed(Up(R2), 50); // tap 1
    g.feed(Down(R2), 100);
    assert!(g.feed(Up(R2), 150).is_empty()); // latched, FF stays on
    g.feed(Down(R2), 5000);
    assert_eq!(g.feed(Up(R2), 5050), vec![FfStop]);
}

/// The flush hangs off the press, because a button being held may be cut by the PMIC before
/// there is any release to see. The lock waits for the release, so that a press on its way
/// to becoming a hold does not darken the panel on the way through.
#[test]
fn power_flushes_on_the_press_and_locks_on_the_release() {
    let mut g = Gestures::new();
    assert_eq!(g.feed(Down(Btn::Power), 0), vec![PowerPress]);
    assert_eq!(g.feed(Up(Btn::Power), 80), vec![PowerTap]);
}

/// The hold arms while the button is still down and the release commits, so the shutdown
/// screen is on the panel for as long as the user holds rather than flashing past.
#[test]
fn power_held_past_the_threshold_raises_the_menu_and_the_release_does_nothing() {
    let mut g = Gestures::new();
    g.feed(Down(Btn::Power), 0);
    assert!(g.tick(POWER_HOLD_MS - 1).is_empty());
    assert_eq!(g.tick(POWER_HOLD_MS), vec![PowerHold]);
    assert!(g.tick(4000).is_empty(), "the hold fires once, not per tick");
    assert_eq!(g.feed(Up(Btn::Power), 4500), vec![PowerOff]);
}

/// And a release that never reached the threshold is a lock, never a shutdown.
#[test]
fn a_press_just_short_of_the_threshold_is_a_lock() {
    let mut g = Gestures::new();
    g.feed(Down(Btn::Power), 0);
    assert!(g.tick(POWER_HOLD_MS - 1).is_empty());
    assert_eq!(g.feed(Up(Btn::Power), POWER_HOLD_MS - 1), vec![PowerTap]);
}

#[test]
fn rewind_beats_latched_fast_forward() {
    let mut g = Gestures::new();
    g.feed(Down(R2), 0);
    g.feed(Up(R2), 50);
    g.feed(Down(R2), 100);
    g.feed(Up(R2), 150); // latched
    assert_eq!(g.feed(Down(L2), 200), vec![FfStop, RewindStart]);
}

/// 120 ms was not enough time to land the second key of a chord, so SELECT reached the game
/// and opened a menu mid press. A held SELECT can wait much longer: the only reason to ever
/// give up on the chord is a game that wants SELECT held down.
#[test]
fn a_held_select_waits_much_longer_than_it_used_to() {
    let mut g = Gestures::new();
    g.feed(Down(Select), 0);
    assert!(g.tick(400).is_empty(), "gave up on the chord at 400 ms");
    assert_eq!(g.tick(SELECT_CHORD_MS), vec![GbaDown(Select)]);
}

#[test]
fn a_chord_landing_late_is_still_a_chord() {
    let mut g = Gestures::new();
    g.feed(Down(Select), 0);
    g.tick(400);
    assert_eq!(g.feed(Down(R1), 400), vec![SaveState]);
    assert!(
        g.tick(5_000).is_empty(),
        "SELECT leaked to the game after a late chord"
    );
}

/// The other half: releasing SELECT settles the question, so a tap should cost the game no
/// latency at all rather than waiting out a window that can no longer produce a chord.
#[test]
fn a_released_select_reaches_the_game_immediately() {
    let mut g = Gestures::new();
    g.feed(Down(Select), 0);
    assert_eq!(g.feed(Up(Select), 80), vec![GbaDown(Select)]);
}

/// Down and up in one batch net out to nothing: the mask is set and cleared before the core
/// ever reads it, so the press is invisible to the game.
#[test]
fn a_select_tap_is_held_long_enough_for_the_core_to_see_it() {
    let mut g = Gestures::new();
    g.feed(Down(Select), 0);
    let on_release = g.feed(Up(Select), 80);
    assert_eq!(on_release, vec![GbaDown(Select)]);
    assert!(
        !on_release.contains(&GbaUp(Select)),
        "press and release in the same frame"
    );
    assert!(
        g.tick(100).is_empty(),
        "released before the core could poll it"
    );
    assert_eq!(g.tick(80 + SELECT_TAP_MS), vec![GbaUp(Select)]);
}

#[test]
fn both_volume_keys_together_mute_once() {
    let mut g = Gestures::new();
    g.feed(Down(VolUp), 0);
    let out = g.feed(Down(VolDown), 80);
    assert!(out.contains(&MuteToggle), "the pair did not mute");
    assert!(
        g.tick(400).is_empty(),
        "it kept firing while both were held"
    );
}

#[test]
fn volume_keys_far_apart_are_not_a_chord() {
    let mut g = Gestures::new();
    g.feed(Down(VolUp), 0);
    let out = g.feed(Down(VolDown), 400);
    assert!(!out.contains(&MuteToggle), "two separate presses muted");
}

/// The app rolls the chord's own two presses back, so it has to see them before it sees the
/// chord. Reversed, the second press would move the level after the mute remembered it.
#[test]
fn the_chord_arrives_behind_the_press_that_completed_it() {
    let mut g = Gestures::new();
    g.feed(Down(VolUp), 0);
    assert_eq!(g.feed(Down(VolDown), 80), vec![VolumeDown, MuteToggle]);
}

/// Unmuting is the same gesture, so a pair that never rearmed would be a one way trip.
#[test]
fn releasing_both_rearms_the_chord() {
    let mut g = Gestures::new();
    g.feed(Down(VolUp), 0);
    g.feed(Down(VolDown), 80);
    g.feed(Up(VolUp), 200);
    g.feed(Up(VolDown), 220);
    g.feed(Down(VolUp), 1_000);
    assert!(g.feed(Down(VolDown), 1_050).contains(&MuteToggle));
}

/// A held volume key ramps. One step per press would mean tapping a dozen times to cross the
/// range, and the press already records when it went down for exactly this.
#[test]
fn a_held_volume_key_repeats() {
    let mut g = Gestures::new();
    assert_eq!(g.feed(Down(VolUp), 0), vec![VolumeUp]);
    assert!(
        g.tick(VOLUME_REPEAT_DELAY_MS - 1).is_empty(),
        "it repeated before the ramp was due"
    );
    assert_eq!(g.tick(VOLUME_REPEAT_DELAY_MS), vec![VolumeUp]);
    assert!(g.tick(VOLUME_REPEAT_DELAY_MS + 1).is_empty());
    assert_eq!(
        g.tick(VOLUME_REPEAT_DELAY_MS + VOLUME_REPEAT_MS),
        vec![VolumeUp]
    );
}

#[test]
fn a_released_volume_key_stops_repeating() {
    let mut g = Gestures::new();
    g.feed(Down(VolDown), 0);
    assert_eq!(g.tick(VOLUME_REPEAT_DELAY_MS), vec![VolumeDown]);
    assert!(g.feed(Up(VolDown), VOLUME_REPEAT_DELAY_MS + 10).is_empty());
    assert!(
        g.tick(VOLUME_REPEAT_DELAY_MS * 4).is_empty(),
        "a key nobody is holding kept ramping"
    );
}

/// Both keys held is the mute chord, not two levels moving at once. The ramp would fight the
/// mute it just fired and leave the level somewhere nobody asked for.
#[test]
fn the_mute_chord_does_not_ramp() {
    let mut g = Gestures::new();
    g.feed(Down(VolUp), 0);
    assert!(g.feed(Down(VolDown), 50).contains(&MuteToggle));
    assert!(
        g.tick(VOLUME_REPEAT_DELAY_MS * 3).is_empty(),
        "the mute chord ramped the volume while it was held"
    );
}

/// A second press after the ramp starts from the top again, rather than inheriting the pace
/// of the press before it.
#[test]
fn each_press_starts_its_own_ramp() {
    let mut g = Gestures::new();
    g.feed(Down(VolUp), 0);
    g.tick(VOLUME_REPEAT_DELAY_MS);
    g.feed(Up(VolUp), VOLUME_REPEAT_DELAY_MS + 5);
    assert_eq!(g.feed(Down(VolUp), 5_000), vec![VolumeUp]);
    assert!(
        g.tick(5_000 + VOLUME_REPEAT_DELAY_MS - 1).is_empty(),
        "the new press repeated early"
    );
    assert_eq!(g.tick(5_000 + VOLUME_REPEAT_DELAY_MS), vec![VolumeUp]);
}

/// X on its own is the game's X. A chord key is only a chord while SELECT is down.
#[test]
fn x_without_select_is_still_the_games_x() {
    let mut g = Gestures::new();
    assert_eq!(g.feed(Down(X), 0), vec![GbaDown(Btn::X)]);
    assert_eq!(g.feed(Up(X), 40), vec![GbaUp(Btn::X)]);
}

/// SELECT+MENU, which is what opens the in-game menu. One gesture, not two: it fires on the
/// MENU press and the release says nothing at all.
#[test]
fn select_and_menu_open_the_in_game_menu() {
    let mut g = Gestures::new();
    g.feed(Down(Select), 0);
    assert_eq!(g.feed(Down(Menu), 50), vec![GameMenu]);
    assert!(
        g.feed(Up(Menu), 150).is_empty(),
        "the release landed as a second gesture on top of the menu"
    );
    assert!(
        g.feed(Up(Select), 200).is_empty(),
        "SELECT reached the game behind the chord that consumed it"
    );
}

/// The chorded press must not also arm the eject hold, or reading the menu with the buttons
/// still down would eject the cart out from under it.
#[test]
fn a_chorded_menu_never_arms_the_eject_hold() {
    let mut g = Gestures::new();
    g.feed(Down(Select), 0);
    assert_eq!(g.feed(Down(Menu), 50), vec![GameMenu]);
    assert!(
        g.tick(50 + MENU_HOLD_MS + 1).is_empty(),
        "holding the menu open ejected the cart"
    );
}

/// The whole test is the ordering inside `menu_down`. Behind the double tap check, a
/// SELECT+MENU that follows a recent MENU tap opens the switcher instead of the menu.
#[test]
fn a_chord_after_a_recent_menu_tap_is_still_the_menu() {
    let mut g = Gestures::new();
    g.feed(Down(Menu), 0);
    assert_eq!(g.feed(Up(Menu), 100), vec![OpenAbout]);
    g.feed(Down(Select), 150);
    assert_eq!(
        g.feed(Down(Menu), 200),
        vec![GameMenu],
        "a recent tap turned the chord into the switcher"
    );
}

/// The difference between a chord and a trap. SELECT commits to being the game's after
/// `SELECT_CHORD_MS`, and MENU under a SELECT the game already has is an ordinary MENU.
#[test]
fn menu_under_a_select_the_game_already_has_is_not_the_menu() {
    let mut g = Gestures::new();
    g.feed(Down(Select), 0);
    assert_eq!(g.tick(SELECT_CHORD_MS), vec![GbaDown(Select)]);
    assert!(
        g.feed(Down(Menu), 700).is_empty(),
        "a SELECT the game already owns still chorded"
    );
    assert_eq!(g.feed(Up(Menu), 800), vec![OpenAbout]);
}

/// The chord clears both of MENU's own windows, so the press after it has to start its own.
///
/// It has to begin with a real tap, or the half of that which matters is invisible: the tap
/// leaves a double tap window open, the chord lands inside it, and a chord that does not
/// close that window leaves the very next MENU tap opening the switcher instead of the about
/// screen — a press whose meaning depends on a chord two presses ago.
#[test]
fn the_menu_button_still_works_after_a_chord() {
    let mut g = Gestures::new();
    g.feed(Down(Menu), 0);
    assert_eq!(g.feed(Up(Menu), 100), vec![OpenAbout]);
    g.feed(Down(Select), 150);
    assert_eq!(g.feed(Down(Menu), 200), vec![GameMenu]);
    assert!(g.feed(Up(Menu), 250).is_empty());
    assert!(g.feed(Up(Select), 260).is_empty());
    assert!(
        g.feed(Down(Menu), 400).is_empty(),
        "the tap before the chord was still standing as half of a double tap"
    );
    assert_eq!(
        g.feed(Up(Menu), 450),
        vec![OpenAbout],
        "the chord left the menu button dead"
    );
}

/// SELECT+Y is the one chord that is two gestures on one press: tapped it walks the palette
/// page, held it opens the palette browser. Everything below is that one sentence.
///
/// The press is therefore **not** spent on the way down — every other chord is, and the tests
/// above are full of `vec![something]` on the `Down`. Here the down emits nothing at all, and
/// which of the two it was is decided by the threshold in `tick` or, failing that, by the
/// release. Losing that distinction is how one key becomes two that fire together: a hold that
/// also walks leaves the user on a different palette from the one they were aiming at when the
/// panel opens.
const Y_HOLD: u64 = COLOR_HOLD_MS;

#[test]
fn select_and_y_held_opens_the_palette_browser() {
    let mut g = Gestures::new();
    assert!(
        g.feed(Down(Select), 0).is_empty(),
        "SELECT is deferred behind the chord window anyway"
    );
    assert!(
        g.feed(Down(Y), 10).is_empty(),
        "the chord's press must not be spent: the two halves are not distinguishable yet"
    );
    assert!(g.tick(10 + Y_HOLD - 1).is_empty(), "not yet");
    assert_eq!(
        g.tick(10 + Y_HOLD),
        vec![ColorHold],
        "the hold is delivered at the threshold, while the key is still down"
    );
    // And the release owes nothing: the panel it opened is up, and B is what puts it away.
    assert!(g.feed(Up(Y), 10 + Y_HOLD + 200).is_empty());
    assert!(g.feed(Up(Select), 10 + Y_HOLD + 210).is_empty());
}

#[test]
fn a_select_y_tap_walks_the_palette_page() {
    let mut g = Gestures::new();
    g.feed(Down(Select), 0);
    assert!(g.feed(Down(Y), 10).is_empty());
    assert_eq!(
        g.feed(Up(Y), 60),
        vec![ColorCycle],
        "a tap is the walk, delivered on the release because the press could still have grown"
    );
    // The threshold must not reach back for a press that is already over.
    assert!(g.tick(10 + Y_HOLD * 4).is_empty());
    assert!(g.feed(Up(Select), 70).is_empty());
}

#[test]
fn the_two_halves_of_select_y_are_mutually_exclusive() {
    // The hold, watched all the way to the end of the press: a walk would show up here.
    let mut g = Gestures::new();
    let mut out = Vec::new();
    out.extend(g.feed(Down(Select), 0));
    out.extend(g.feed(Down(Y), 10));
    out.extend(g.tick(10 + Y_HOLD));
    out.extend(g.tick(10 + Y_HOLD * 10));
    out.extend(g.feed(Up(Y), 10 + Y_HOLD * 10 + 10));
    out.extend(g.feed(Up(Select), 10 + Y_HOLD * 10 + 20));
    assert_eq!(out, vec![ColorHold], "the hold also walked the page");
}

#[test]
fn the_browser_opens_once_per_press_not_once_per_tick() {
    let mut g = Gestures::new();
    g.feed(Down(Select), 0);
    g.feed(Down(Y), 10);
    let mut holds = 0;
    for t in 10..10 + Y_HOLD * 5 {
        holds += g.tick(t).iter().filter(|a| **a == ColorHold).count();
    }
    assert_eq!(holds, 1, "the threshold is an edge, not a level");
}

#[test]
fn a_select_y_hold_never_reaches_the_game() {
    let mut g = Gestures::new();
    g.feed(Down(Select), 0);
    g.feed(Down(Y), 10);
    g.tick(10 + Y_HOLD);
    let mut out = g.feed(Up(Y), 10 + Y_HOLD + 100);
    out.extend(g.feed(Up(Select), 10 + Y_HOLD + 110));
    // SELECT's own press is owed to the game only when it was never chorded; the `GbaUp` for it
    // is emitted by `select_up` on the release, and the down was swallowed, so nothing here.
    assert!(out.is_empty(), "a chorded press leaked to the game: {out:?}");
}

#[test]
fn a_second_press_arms_its_own_hold() {
    let mut g = Gestures::new();
    g.feed(Down(Select), 0);
    g.feed(Down(Y), 10);
    assert_eq!(g.tick(10 + Y_HOLD), vec![ColorHold]);
    g.feed(Up(Y), 10 + Y_HOLD + 50);
    g.feed(Up(Select), 10 + Y_HOLD + 60);
    // The next press is a fresh gesture, and this one is a tap: the fired flag from the last
    // press must not turn it into a second hold.
    g.feed(Down(Select), 1000);
    g.feed(Down(Y), 1010);
    assert!(g.tick(1010 + Y_HOLD - 1).is_empty(), "the flag was left set");
    assert_eq!(g.feed(Up(Y), 1010 + Y_HOLD - 1), vec![ColorCycle]);
}

#[test]
fn y_released_just_short_of_the_threshold_walks() {
    let mut g = Gestures::new();
    g.feed(Down(Select), 0);
    g.feed(Down(Y), 10);
    assert!(g.tick(10 + Y_HOLD - 1).is_empty());
    assert_eq!(g.feed(Up(Y), 10 + Y_HOLD - 1), vec![ColorCycle]);
}

#[test]
fn a_select_released_under_a_held_y_owes_nothing() {
    // SELECT let go first, with Y still down and the panel already open. The chord's own
    // release handling must survive SELECT going away underneath it.
    let mut g = Gestures::new();
    g.feed(Down(Select), 0);
    g.feed(Down(Y), 10);
    assert_eq!(g.tick(10 + Y_HOLD), vec![ColorHold]);
    assert!(g.feed(Up(Select), 10 + Y_HOLD + 20).is_empty());
    assert!(g.feed(Up(Y), 10 + Y_HOLD + 400).is_empty(), "the hold also walked");
}

/// And with no SELECT down, Y is the game's again — the chord does not take the button.
#[test]
fn y_without_select_is_still_the_games_y() {
    let mut g = Gestures::new();
    assert_eq!(g.feed(Down(Y), 0), vec![GbaDown(Y)]);
    assert_eq!(g.feed(Up(Y), 40), vec![GbaUp(Y)]);
    // ...and holding it is not a hold, because there is no chord to be the hold half of.
    assert!(g.tick(0 + Y_HOLD * 3).is_empty());
    assert_eq!(g.feed(Down(Y), 2000), vec![GbaDown(Y)]);
    assert_eq!(g.feed(Up(Y), 2000 + Y_HOLD * 3), vec![GbaUp(Y)]);
}

/// The chord leaves nothing behind for the next press to find.
///
/// The menu button needed exactly this test and for the same reason: the deferred press is the
/// one place a chord keeps state across two edges, and state that is not cleared on the way out
/// changes the meaning of a press that comes much later.
#[test]
fn the_y_button_still_works_after_a_chord() {
    for (label, hold) in [("held", true), ("tapped", false)] {
        let mut g = Gestures::new();
        g.feed(Down(Select), 0);
        g.feed(Down(Y), 10);
        if hold {
            g.tick(10 + Y_HOLD);
        }
        g.feed(Up(Y), 10 + Y_HOLD + 50);
        g.feed(Up(Select), 10 + Y_HOLD + 60);
        assert_eq!(
            g.feed(Down(Y), 5000),
            vec![GbaDown(Y)],
            "a bare Y after the {label} chord was not the game's"
        );
        assert_eq!(g.feed(Up(Y), 5040), vec![GbaUp(Y)]);
    }
}
