mod common;

use common::tmp_root;
use slot_store::{
    atomic_write, read_slot_state, write_slot_state, Mode, SlotState, BRIGHTNESS_MAX,
};
use tempfile::tempdir;

#[test]
fn atomic_write_leaves_no_partial_file_and_no_temp_behind() {
    let d = tempdir().unwrap();
    let p = d.path().join("x.bin");
    atomic_write(&p, b"first").unwrap();
    atomic_write(&p, &vec![7u8; 4_000_000]).unwrap();
    assert_eq!(std::fs::read(&p).unwrap().len(), 4_000_000);
    let strays: Vec<_> = std::fs::read_dir(d.path())
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_name() != "x.bin")
        .collect();
    assert!(strays.is_empty(), "temp files left behind: {strays:?}");
}

#[test]
fn corrupt_slot_state_reads_as_default_rather_than_panicking() {
    let d = tmp_root();
    std::fs::write(d.path().join("System/slot.state"), b"\x00\xff not json").unwrap();
    assert_eq!(read_slot_state(d.path()), SlotState::default());
}

#[test]
fn slot_state_round_trips_including_a_stem_with_an_equals_sign() {
    let d = tmp_root();
    let s = SlotState {
        cart: Some("Cheats = On".into()),
        brightness: 3,
        blue_light: 9,
        volume: 71,
        muted: true,
        clock_set: true,
        utc_offset_min: 0,
        mode: Mode::Light,
    };
    write_slot_state(d.path(), &s).unwrap();
    assert_eq!(read_slot_state(d.path()), s);
}

/// The mode is the one field a card written before it existed can be missing, so it is the one
/// field whose absence has to read as something other than corruption. Everything else about
/// such a file is still strict — see `a_slot_state_missing_a_key_reads_as_default_not_half_populated`
/// for the other half of that rule.
#[test]
fn a_state_file_from_before_the_mode_reads_as_dark_and_keeps_the_rest() {
    let d = tmp_root();
    std::fs::write(
        d.path().join("System/slot.state"),
        "cart=Emerald\nbrightness=7\nblue_light=2\nvolume=44\nmuted=1\nclock_set=1\nutc_offset_min=120\n",
    )
    .unwrap();
    let s = read_slot_state(d.path());
    assert_eq!(s.mode, Mode::Dark, "an older card did not come up dark");
    assert_eq!(
        s.cart.as_deref(),
        Some("Emerald"),
        "the cart was thrown away"
    );
    assert_eq!(s.brightness, 7);
    assert_eq!(s.volume, 44);
    assert_eq!(s.utc_offset_min, 120);
}

/// And a mode this build cannot read is corruption like any other word it cannot read: the
/// strictness is there to catch a file someone has been editing, and a third mode is exactly
/// that.
#[test]
fn a_mode_that_is_not_a_mode_reads_as_default() {
    let d = tmp_root();
    std::fs::write(
        d.path().join("System/slot.state"),
        "cart=Emerald\nbrightness=7\nblue_light=2\nvolume=44\nmuted=1\nclock_set=1\nutc_offset_min=0\nmode=sepia\n",
    )
    .unwrap();
    assert_eq!(read_slot_state(d.path()), SlotState::default());
}

#[test]
fn a_slot_state_missing_a_key_reads_as_default_not_half_populated() {
    let d = tmp_root();
    std::fs::write(
        d.path().join("System/slot.state"),
        "cart=Emerald\nbrightness=3\nblue_light=1\n",
    )
    .unwrap();
    assert_eq!(read_slot_state(d.path()), SlotState::default());
}

#[test]
fn an_out_of_range_level_reads_as_default() {
    let d = tmp_root();
    for body in [
        "cart=\nbrightness=20\nblue_light=1\nvolume=50\n",
        "cart=\nbrightness=3\nblue_light=10\nvolume=50\n",
        "cart=\nbrightness=3\nblue_light=1\nvolume=101\n",
    ] {
        std::fs::write(d.path().join("System/slot.state"), body).unwrap();
        assert_eq!(
            read_slot_state(d.path()),
            SlotState::default(),
            "accepted {body:?}"
        );
    }
}

#[test]
fn a_first_boot_is_neither_dark_nor_silent() {
    let d = tmp_root();
    let s = read_slot_state(d.path());
    assert!(s.cart.is_none());
    assert!(s.brightness > 0, "boots with the backlight off");
    assert!(s.volume > 0, "boots muted");
}

/// The top of the ramp is a setting someone can pick, not one past the end of it. Nine was
/// the top until the scale doubled, and a card written back then still has to read as the
/// level it named rather than as a corrupt file.
#[test]
fn the_top_brightness_step_is_in_range() {
    let d = tmp_root();
    for value in [9, BRIGHTNESS_MAX] {
        std::fs::write(
            d.path().join("System/slot.state"),
            format!(
                "cart=\nbrightness={value}\nblue_light=0\nvolume=60\nmuted=0\nclock_set=1\nutc_offset_min=0\n"
            ),
        )
        .unwrap();
        assert_eq!(
            read_slot_state(d.path()).brightness,
            value,
            "step {value} should be a valid level"
        );
    }
}

/// The offset is what turns the card's UTC into the time on the shelf, so it has to outlive
/// the session that chose it.
#[test]
fn slot_state_round_trips_a_negative_utc_offset() {
    let d = tmp_root();
    let s = SlotState {
        cart: None,
        brightness: 5,
        blue_light: 0,
        volume: 60,
        muted: false,
        clock_set: true,
        utc_offset_min: -450,
        mode: Mode::Dark,
    };
    write_slot_state(d.path(), &s).unwrap();
    assert_eq!(read_slot_state(d.path()).utc_offset_min, -450);
}

/// Half hour zones are real and whole hour steps would put several countries permanently
/// thirty minutes out.
#[test]
fn an_offset_outside_the_range_of_real_zones_reads_as_default() {
    let d = tmp_root();
    for body in [
        "cart=\nbrightness=5\nblue_light=0\nvolume=60\nmuted=0\nclock_set=1\nutc_offset_min=900\n",
        "cart=\nbrightness=5\nblue_light=0\nvolume=60\nmuted=0\nclock_set=1\nutc_offset_min=-780\n",
    ] {
        std::fs::write(d.path().join("System/slot.state"), body).unwrap();
        assert_eq!(read_slot_state(d.path()), SlotState::default(), "{body}");
    }
}
