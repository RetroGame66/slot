//! What a card's `System/display.txt` is allowed to ask for.
//!
//! The file is one line — `mask cc_gba cc_gb cc_gbc` — and it is the only thing that survives a
//! reboot, so every preset the interface can reach has to come back as itself.

mod common;

use common::tmp_root_with_carts;
use slot::app::MASK_STATES;
use slot::root::{display_modes, write_display_modes};

#[test]
fn the_whole_ring_survives_the_file() {
    // Every preset the Advance's ring holds, written and read back. The clamp that reads the file
    // was left on the old count of five when the scanline was added, so the two newest presets
    // were written, shown, persisted — and then silently read back as LCD3x 100% on the next boot.
    // Nothing else in the tree could have caught it: the file was always the right file, and the
    // number in it was always the right number.
    let d = tmp_root_with_carts(&["Emerald"]);
    for mask in 0..MASK_STATES as u8 {
        write_display_modes(d.path(), mask, [3, 7, 2]);
        let (read, cc) = display_modes(d.path());
        assert_eq!(read, mask, "preset {mask} did not survive the file");
        assert_eq!(cc, [3, 7, 2], "and neither did the three colour slots");
    }
    // Past the end clamps to the last preset rather than wrapping, for a file edited on a PC.
    write_display_modes(d.path(), 99, [0, 0, 0]);
    assert_eq!(display_modes(d.path()).0 as usize, MASK_STATES - 1);
}

#[test]
fn a_missing_or_older_file_still_reads() {
    // A card from before the colour machines had a slot each, and one with nothing at all. Neither
    // is an error: the older two-integer file meant "whatever was chosen, for every machine", and
    // that is still what it means.
    let d = tmp_root_with_carts(&["Emerald"]);
    let file = d.path().join("System/display.txt");
    let _ = std::fs::remove_file(&file);
    assert_eq!(display_modes(d.path()), (2, [0, 0, 0]), "the built-in default");

    std::fs::write(&file, "1 4").unwrap();
    assert_eq!(display_modes(d.path()), (1, [4, 4, 4]), "one colour, every machine");

    std::fs::write(&file, "not a number").unwrap();
    assert_eq!(display_modes(d.path()), (2, [0, 0, 0]), "and it does not panic on nonsense");
}
