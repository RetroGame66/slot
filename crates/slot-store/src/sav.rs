//! Whether a battery save is worth writing back to the card, and whether what is already
//! there may be replaced.
//!
//! This used to be one rule in the frontend: never replace a file with a shorter one. The
//! rule is right about what it protects -- a real truncation -- and wrong about how it
//! detects one, because the length a core reports says nothing about how much of it the game
//! ever wrote. On GBA that misfires two ways, both reachable without the player doing
//! anything unusual:
//!
//! * mGBA reports `GBA_SIZE_FLASH1M` (131072) for a cart whose save type it has not resolved
//!   yet, and the type is only decided by the game's own first access to the save region. A
//!   60-second autosave lands before that often enough, and persists a 128 KB region that is
//!   nothing but the blank byte. Once the game does reach its save region the core reports the
//!   real 32768, and every later write is shorter than that shell -- refused, for good.
//! * gpSP answers 131072 for every GBA cart, resolved or not. One session under it leaves a
//!   128 KB file that no mGBA session can write over again.
//!
//! Both leave a file whose tail past the real save is filler, and both cores keep the real
//! bytes at offset 0. So the length comparison is replaced by what it was standing in for:
//! refuse only when the bytes actually being dropped are something other than filler.
//!
//! Filler is more than the blank byte. A chip erases to `0xFF`, but a *file* is not a chip:
//! frontends that pre-create a save write zeroes, and gpSP's own source says so — it carries a
//! routine whose comment is "some frontends create blank save files filled with 0x00. Real
//! flash/EEPROM idle state is 0xFF". A card that has been played under gpSP is exactly the
//! reported case here: gpSP answers 131072 for every cart, so an SRAM or EEPROM game leaves a
//! 128 KB file, and every later mGBA write — 32768 or 512 — is shorter than it.
//!
//! So a dropped range that is one repeated byte is filler whatever that byte is (a real save
//! region is not 32 KB of a single value), and it is let through. Dropping `0xFF` loses
//! nothing, so it is a plain write. Dropping anything else is *probably* nothing, but not
//! provably, so the file is moved aside first and the write then goes through — the cart saves
//! again, and the old bytes are still on the card. Only a range with real structure in it —
//! several values, or one value interrupted — is still refused, because that is what a genuine
//! truncation looks like: a core that resolved FLASH512 as SRAM leaves the game's second flash
//! bank in the dropped range, and that write must still be stopped.
//!
//! Written as a pure function of two byte slices, and kept in this crate rather than the
//! frontend, because this crate's tests run on the host and the frontend's do not.

/// Flash and EEPROM idle state, and what both cores fill an untouched save region with. Not a
/// save anyone typed, so dropping it cannot lose one.
pub const BLANK: u8 = 0xFF;

/// What RetroArch writes when its "save file compression" option is on: `#RZIPv`, one version
/// byte, then `#`. The path is unchanged, so nothing but these bytes distinguishes it from a
/// raw save.
const RZIP_MAGIC: [u8; 6] = *b"#RZIPv";
const RZIP_VERSION_DEFLATE: u8 = 1;
const RZIP_VERSION_ZSTD: u8 = 2;

/// Nothing but the blank byte. An empty slice counts, so a core that hands back no bytes at
/// all is treated as having nothing to say rather than as a zero-length save.
fn is_blank(data: &[u8]) -> bool {
    data.iter().all(|b| *b == BLANK)
}

/// One value, repeated, for the whole slice.
///
/// This is the test for filler, and it is deliberately about *shape* rather than about which
/// value: which bytes a tool pads with is not something the frontend can enumerate. An empty
/// slice counts, which is what an empty dropped range is.
fn is_uniform(data: &[u8]) -> bool {
    match data.first() {
        None => true,
        Some(first) => data.iter().all(|b| b == first),
    }
}

/// What a dropped range actually holds, for the log.
///
/// Only ever called on a refusal, so walking the slice again costs nothing that matters. What
/// does matter is that the next report of "this cart will not save" carries the shape of the
/// bytes that were in the way, rather than only the fact that the write was shorter: that is
/// the difference between guessing at the cause and reading it off a card.
pub fn describe(data: &[u8]) -> String {
    let mut seen = [false; 256];
    let mut distinct = 0usize;
    let mut first_other: Option<(usize, u8)> = None;
    for (i, b) in data.iter().enumerate() {
        if *b != BLANK && first_other.is_none() {
            first_other = Some((i, *b));
        }
        if !seen[*b as usize] {
            seen[*b as usize] = true;
            distinct += 1;
        }
    }
    match first_other {
        None => format!("{} bytes, all {BLANK:#04x}", data.len()),
        Some((at, b)) => format!(
            "{} bytes, first non-blank {b:#04x} at +{at}, {} distinct value(s){}",
            data.len(),
            distinct.min(99),
            if distinct > 99 { "+" } else { "" }
        ),
    }
}

/// A save that RetroArch compressed. Its first bytes are the container's header rather than
/// the game's, so it is not cartridge data and must not be handed to a core or overwritten by
/// one. The eight-byte magic is the whole test: the version byte is part of the container, and
/// needing the rest of the 20-byte header to parse it -- as RetroArch's own reader does -- is
/// a different question from recognising it.
pub fn is_rzip(data: &[u8]) -> bool {
    data.len() >= 8
        && data[..6] == RZIP_MAGIC
        && (data[6] == RZIP_VERSION_DEFLATE || data[6] == RZIP_VERSION_ZSTD)
        && data[7] == b'#'
}

/// What to do with `sav`, given `old` as whatever the card already holds for this cart.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SavePlan {
    /// Different bytes, and nothing being dropped that matters: replace the file.
    Write,
    /// Byte for byte what is already there. A GBA core hands back its whole save region
    /// whether or not the game touched it, so this is the common case, and the write is
    /// skipped rather than rewriting up to 128 KB of card for nothing.
    Unchanged,
    /// A blank save with no file to replace. Do not create one: a region of nothing but
    /// `0xFF` is what a core reports before the game has saved anything, and persisting it is
    /// what robs that cart of ever writing a smaller, real save later.
    Blank,
    /// Decided by `old`, never by `sav`: the on-card file is a compressed RetroArch save.
    /// Replacing it with raw bytes would trade a save the frontend cannot read for one
    /// neither it nor RetroArch will recognise. Left exactly as it is, and said out loud, so
    /// the card can be put right by hand instead of silently.
    RefuseCompressed,
    /// Shorter than what is on the card, dropping a range that is one repeated byte but not the
    /// blank one -- a save file some *tool* pre-created, not one a cartridge erased. gpSP
    /// answers 131072 for every cart, so this is what a card that has been played under it
    /// holds for an SRAM or EEPROM game, and it is the shape that used to lock those carts out
    /// of saving for good.
    ///
    /// The bytes cannot be read as a save and they are not provably nothing either, so the
    /// answer is neither "write over them" nor "refuse": move the file aside, then write. The
    /// cart saves again, and what was there is still on the card for whoever wants it.
    BackupAndWrite,
    /// Shorter than what is on the card, with a dropped range that has real structure in it.
    /// This is the real truncation the guard exists for: a core that resolved FLASH512 as SRAM
    /// leaves the game's second flash bank in the range it would drop.
    RefuseShrink,
}

/// Decide, without touching the disk.
///
/// Order matters in three places. `RefuseCompressed` comes before `Unchanged` so that a
/// compressed file is never treated as a save to compare against; the empty-save guard comes
/// before the comparison so a core that reports no save-ram at all cannot write a zero-length
/// file over a real one; and `Blank` is reachable only with no file on the card, because a game
/// that erases its own save writes the blank byte *over* real data, and that write has to land,
/// or "delete save data" would undo itself at the next boot. What must not happen is creating
/// the file in the first place.
pub fn save_plan(old: Option<&[u8]>, sav: &[u8]) -> SavePlan {
    let Some(old) = old else {
        return if is_blank(sav) {
            SavePlan::Blank
        } else {
            SavePlan::Write
        };
    };
    if is_rzip(old) {
        return SavePlan::RefuseCompressed;
    }
    if sav.is_empty() {
        return SavePlan::Blank;
    }
    if old == sav {
        return SavePlan::Unchanged;
    }
    if sav.len() < old.len() {
        let dropped = &old[sav.len()..];
        if is_blank(dropped) {
            return SavePlan::Write;
        }
        return if is_uniform(dropped) {
            SavePlan::BackupAndWrite
        } else {
            SavePlan::RefuseShrink
        };
    }
    SavePlan::Write
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A save region of `len` blank bytes with `run` overwritten with real data.
    fn region(len: usize, run: std::ops::Range<usize>) -> Vec<u8> {
        debug_assert!(run.end <= len, "test data would not fit in {len} bytes");
        let mut v = vec![BLANK; len];
        for i in run {
            v[i] = 0x42;
        }
        v
    }

    /// Real data up to `keep`, then `fill` for the rest: a file whose tail is one repeated
    /// value, which is what a *tool*-made save looks like past the real save.
    fn with_tail(len: usize, keep: usize, fill: u8) -> Vec<u8> {
        let mut v = vec![fill; len];
        for b in &mut v[..keep] {
            *b = 0x42;
        }
        v
    }

    /// The 20-byte header RetroArch writes, with a byte of payload after it.
    fn rzip(version: u8) -> Vec<u8> {
        let mut v = b"#RZIPv".to_vec();
        v.push(version);
        v.push(b'#');
        v.extend_from_slice(&[0x0C, 0x00, 0x00, 0x00, 0x78, 0x9C, 0x03, 0x00]);
        v.extend_from_slice(&[0x00; 4]);
        v
    }

    // --- A: never create the shell -----------------------------------------------------

    #[test]
    fn a_blank_region_with_no_file_on_the_card_is_not_a_save() {
        // What mGBA reports before the game has touched its save region: 131072 bytes of
        // 0xFF. Writing this is the bug, not the fix.
        assert_eq!(save_plan(None, &region(131072, 0..0)), SavePlan::Blank);
    }

    #[test]
    fn the_blank_shell_does_not_shadow_the_real_save_that_follows() {
        // The whole point: nothing is written for the shell, so the smaller real save that
        // arrives later has no larger file to lose to.
        assert_eq!(save_plan(None, &region(131072, 0..0)), SavePlan::Blank);
        assert_eq!(save_plan(None, &region(32768, 0..16)), SavePlan::Write);
    }

    #[test]
    fn an_empty_slice_is_blank_too() {
        assert_eq!(save_plan(None, &[]), SavePlan::Blank);
    }

    #[test]
    fn a_save_with_no_file_on_the_card_is_still_written() {
        assert_eq!(save_plan(None, &region(32768, 0..16)), SavePlan::Write);
        assert_eq!(save_plan(None, &region(131072, 0..16)), SavePlan::Write);
    }

    // --- B: a shrink is refused for what it drops, not for its length -------------------

    #[test]
    fn a_larger_placeholder_yields_to_the_real_size() {
        // gpSP (or an unresolved mGBA) left 128 KB; mGBA now reports the real 32768. The
        // dropped tail is blank, so nothing is lost and the lock is lifted.
        let old = region(131072, 0..32768);
        assert_eq!(
            save_plan(Some(&old), &region(32768, 0..32768)),
            SavePlan::Write
        );
    }

    #[test]
    fn an_all_blank_shell_yields_to_any_real_save() {
        assert_eq!(
            save_plan(Some(&region(131072, 0..0)), &region(32768, 0..16)),
            SavePlan::Write
        );
    }

    #[test]
    fn a_real_truncation_is_still_refused() {
        // The case the guard was written for: something in the dropped range is not blank, so
        // a shorter write would lose it. A core that resolved FLASH512 as SRAM looks like
        // this, and must still be stopped.
        let old = region(65536, 0..40000);
        assert_eq!(
            save_plan(Some(&old), &region(32768, 0..32768)),
            SavePlan::RefuseShrink
        );
    }

    #[test]
    fn one_byte_of_data_in_the_dropped_range_is_enough_to_refuse() {
        // The boundary sits at the first byte the shorter write would drop, not at "the file
        // is longer": one non-blank byte there is a save being lost.
        let mut old = vec![BLANK; 65536];
        old[32768] = 0x01;
        assert_eq!(
            save_plan(Some(&old), &region(32768, 0..0)),
            SavePlan::RefuseShrink
        );

        // Same file with that byte blank, and the same shorter write goes through.
        let old = vec![BLANK; 65536];
        assert_eq!(save_plan(Some(&old), &region(32768, 0..0)), SavePlan::Write);
    }

    // --- B2: a tool-made tail is filler too, and is moved aside rather than refused -----

    #[test]
    fn a_tool_made_tail_is_backed_up_rather_than_refused() {
        // The reported card. gpSP answers 131072 for *every* cart, and a save file that a
        // frontend pre-created holds zeroes where a chip would have erased to 0xFF. mGBA now
        // reports the real 32768 -- and this used to be refused for good, which is exactly the
        // "some games will not save" the players are reporting.
        let old = with_tail(131072, 32768, 0x00);
        assert_eq!(
            save_plan(Some(&old), &region(32768, 0..32768)),
            SavePlan::BackupAndWrite
        );
    }

    #[test]
    fn filler_is_any_single_repeated_value_not_just_zero() {
        // Which byte a tool pads with is not a list the frontend can hold, and a real save
        // region is not 32 KB of one value whichever value it is.
        for fill in [0x00, 0xAA, 0x55, 0x0F] {
            let old = with_tail(65536, 32768, fill);
            assert_eq!(
                save_plan(Some(&old), &region(32768, 0..32768)),
                SavePlan::BackupAndWrite,
                "a tail of {fill:#04x} should read as filler"
            );
        }
    }

    #[test]
    fn structure_in_the_dropped_range_is_still_refused() {
        // One byte that is neither the filler nor the blank byte is enough to make the range a
        // save rather than padding. This is the truncation the guard exists for, and it is the
        // boundary that must not move: the whole point of the change above is that it only
        // reaches ranges that cannot be a save.
        let mut old = with_tail(65536, 32768, 0x00);
        old[50000] = 0x7F;
        assert_eq!(
            save_plan(Some(&old), &region(32768, 0..32768)),
            SavePlan::RefuseShrink
        );
    }

    #[test]
    fn one_stray_byte_among_zeroes_is_still_structure() {
        // The mirror of the blank-byte boundary above, on the new filler value.
        let mut old = vec![0x00; 131072];
        old[60000] = 0x01;
        assert_eq!(
            save_plan(Some(&old), &region(8192, 0..0)),
            SavePlan::RefuseShrink
        );
        let old = vec![0x00; 131072];
        assert_eq!(
            save_plan(Some(&old), &region(8192, 0..0)),
            SavePlan::BackupAndWrite
        );
    }

    #[test]
    fn a_core_reporting_no_save_ram_does_not_truncate_the_file() {
        // A core that hands back no bytes has nothing to say. Writing that would replace a real
        // save with a zero-length file which then shadows it on every read, since `.sav` wins.
        let old = region(32768, 0..16);
        assert_eq!(save_plan(Some(&old), &[]), SavePlan::Blank);
    }

    #[test]
    fn an_identical_save_is_not_rewritten() {
        let sav = region(32768, 0..16);
        assert_eq!(save_plan(Some(&sav), &sav.clone()), SavePlan::Unchanged);
    }

    // --- the shape of a refusal, for the log --------------------------------------------

    #[test]
    fn describe_names_the_shape_of_what_would_be_dropped() {
        assert_eq!(describe(&[BLANK; 8]), "8 bytes, all 0xff");
        assert!(describe(&[0x00; 4096]).contains("first non-blank 0x00 at +0"));
        assert!(describe(&[BLANK, BLANK, 0x42]).contains("first non-blank 0x42 at +2"));
        // More than one value is what "structure" means, and what a caller reads the log for.
        assert!(describe(&[0x01, 0x02]).contains("2 distinct value(s)"));
        assert!(describe(&[]).contains("all 0xff"));
    }

    #[test]
    fn growing_a_save_is_always_written() {
        let old = region(32768, 0..16);
        assert_eq!(
            save_plan(Some(&old), &region(131072, 0..16)),
            SavePlan::Write
        );
    }

    // --- erasing still works -------------------------------------------------------------

    #[test]
    fn a_game_erasing_its_own_save_still_lands() {
        // "Delete save data" writes the blank byte over a real save. That has to stick, or the
        // delete would undo itself at the next boot. Only *creating* a blank file is refused.
        let old = region(32768, 0..32);
        assert_eq!(save_plan(Some(&old), &region(32768, 0..0)), SavePlan::Write);
    }

    // --- C: a compressed RetroArch save is recognised, never clobbered -------------------

    #[test]
    fn a_compressed_retroarch_save_is_recognised() {
        assert!(is_rzip(&rzip(RZIP_VERSION_DEFLATE)));
        assert!(is_rzip(&rzip(RZIP_VERSION_ZSTD)));
    }

    #[test]
    fn a_raw_save_is_not_mistaken_for_a_compressed_one() {
        assert!(!is_rzip(&[]));
        assert!(!is_rzip(b"#RZIP"));
        assert!(!is_rzip(b"#RZIPv\x01")); // magic and version, no closing '#'
        assert!(!is_rzip(b"#RZIPv\x09#")); // unknown version
        assert!(!is_rzip(&region(32768, 0..4096)));
        // A real save may start with the blank byte, which is the common case for a fresh one.
        assert!(!is_rzip(&region(32768, 0..0)));
    }

    #[test]
    fn a_compressed_retroarch_save_is_never_overwritten() {
        let old = rzip(RZIP_VERSION_DEFLATE);
        assert_eq!(
            save_plan(Some(&old), &region(32768, 0..16)),
            SavePlan::RefuseCompressed
        );
        // Even by a save that would otherwise be a plain, larger write.
        assert_eq!(
            save_plan(Some(&old), &region(131072, 0..16)),
            SavePlan::RefuseCompressed
        );
    }
}
