use std::path::{Path, PathBuf};

use slot_store::{
    atomic_write, describe, is_rzip, read_slot_state, save_plan, write_slot_state, Core, SavePlan,
    StateRing,
};

/// What a save, a load or a flush needs from the emulator. The core runs on a worker thread
/// and nothing above this trait knows that.
pub trait Snapshot {
    fn state(&self) -> Option<Vec<u8>>;
    fn save_ram(&self) -> Option<Vec<u8>>;
    /// The last frame the core produced, PNG encoded. Encoded on the worker, which is where
    /// the frame already is, so a save does not cost the compositor a hitch.
    fn thumb(&self) -> Option<Vec<u8>>;
    fn load(&self, state: Vec<u8>);

    /// Whether `state()` came from a core that actually accepted the resume it was opened
    /// with. Defaults to `true`, which is right for anything that was never handed a resume
    /// to refuse — a stub in a test, or a cart with none on disk yet. The one implementor
    /// that can ever answer `false` is a live core, and only once `unserialize` has failed on
    /// it: see `EmuSnapshot::resume_trusted`, which is where a real refusal is recorded. A
    /// flush path that writes `state()` back without checking this can turn a core's own
    /// default machine into the player's save.
    fn resume_trusted(&self) -> bool {
        true
    }

    /// The `save_ram()` twin of `resume_trusted`, and independent of it: a core can accept
    /// one and refuse the other.
    fn save_ram_trusted(&self) -> bool {
        true
    }
}

/// What lid close, the power press edge and the autosave all write. The slot is untouched:
/// none of them is an eject, and the cart has to still be in it on the next boot.
///
/// Takes `core` rather than resolving it here, for the same reason `read_resume` does below:
/// the caller already has to know which core is live to have anything worth flushing, and
/// asking this function to work it out too would be a second, independent read of
/// `selected_core.ini` for the same cart. `App` is that caller — it resolves `core` once, at
/// insert, stores it, and hands the stored value here on every later write, which is what
/// keeps this from ever disagreeing with the dylib actually running.
///
/// `state` is `Option` for the same reason `sav` already was: the caller — `App::flush_resume`
/// and `App::flush_eject` — passes `None` for whichever region the live core refused at open,
/// via `Snapshot::resume_trusted`/`save_ram_trusted`. This function trusts whatever it is
/// handed; it is the one place that decides what gets skipped.
pub fn flush(
    root: &Path,
    core: Core,
    stem: &str,
    state: Option<&[u8]>,
    sav: Option<&[u8]>,
) -> std::io::Result<()> {
    if let Some(state) = state {
        StateRing::new(root, core, stem).write_resume(state)?;
    }
    if let Some(sav) = sav {
        write_sav(root, stem, sav)?;
    }
    Ok(())
}

/// Both durable writes land before the slot is recorded empty, so a cut anywhere in here
/// leaves a cart that still resumes rather than a session with nowhere to go back to.
///
/// Clearing the slot still happens even when `state`/`sav` withheld a refused region: the
/// files that refusal left alone are exactly as durable as they were before this cart was
/// seated, so there is nothing an eject would be waiting on.
pub fn eject(
    root: &Path,
    core: Core,
    stem: &str,
    state: Option<&[u8]>,
    sav: Option<&[u8]>,
) -> std::io::Result<()> {
    flush(root, core, stem, state, sav)?;
    let mut slot = read_slot_state(root);
    slot.cart = None;
    write_slot_state(root, &slot)
}

/// The core hands back the whole save ram whether or not the game touched it, so an
/// unchanged one is a rewrite of up to 128 KB of card for nothing.
///
/// What may be written is decided by `slot_store::sav::save_plan`, which is where the two ways
/// a GBA core's reported length lies about how much the game actually wrote are set out, along
/// with the rule that replaces the old length comparison. This function is the disk half of
/// it: turn a `SavePlan` into either a write, a rename-then-write, or a refusal with a line in
/// the log naming the rule and the shape of the bytes that stopped it.
///
/// `BackupAndWrite` is the answer for a shorter save whose dropped range is one repeated byte:
/// filler, in a file a tool made rather than a cartridge erased. It is renamed aside first, so
/// the write cannot be the thing that loses something nobody can prove was worthless -- and the
/// cart is not locked out of saving, which is what the old length rule did to any card that had
/// ever been played under gpSP (it answers 131072 for every cart, resolved or not).
///
/// `load_save_ram` can accept bytes it should have refused -- a libretro core that exposes a
/// save-ram region copies `len.min(data.len())` bytes into it and returns `Ok` regardless, so a
/// cart whose two cores disagree on `RETRO_MEMORY_SAVE_RAM`'s size truncates silently rather
/// than failing loudly -- and that is the class of bug the plan's `RefuseShrink` is still the
/// backstop for, since `resume_trusted`/`save_ram_trusted` cannot see it: as far as the core is
/// concerned, it accepted what it was given. It is deliberately narrower than "the file is
/// longer": a dropped range with real structure in it is a save being lost, and is still
/// stopped, while a dropped range that is one value repeated is not.
///
/// The comparison goes through `read_sav`, not a stat of `sav_path` alone: `read_sav` also
/// accepts `Saves/<stem>.srm` (RetroArch's name for the same battery bytes, see its own doc
/// comment below), and a card carrying only an `.srm` still has a real save on it. Stat-ing
/// `.sav` directly would find nothing there, wave a smaller write through unguarded, and that
/// new `.sav` would then shadow the larger `.srm` on every read after — this is the exact
/// loss shape the guard above exists to stop, just reached from the one path it could not see.
pub fn write_sav(root: &Path, stem: &str, sav: &[u8]) -> std::io::Result<bool> {
    let path = sav_path(root, stem);
    let old = read_sav(root, stem);
    let dropped = old
        .as_deref()
        .map_or(&[][..], |o| &o[sav.len().min(o.len())..]);
    match save_plan(old.as_deref(), sav) {
        SavePlan::Write => {}
        SavePlan::Unchanged => return Ok(false),
        // Silent on purpose, and not a refusal: a cart whose game has not saved anything yet
        // reports this on every autosave, and there is nothing wrong with the cart. Creating
        // the file is what would be wrong.
        SavePlan::Blank => return Ok(false),
        // Shorter, dropping a range that is one repeated byte but not the blank one: a file a
        // tool made rather than a chip erased. It is not a save that can be read, but it is not
        // provably nothing either, so it is neither overwritten blind nor used as a reason to
        // stop saving -- which is what it used to be, for good, on any card that had been
        // played under gpSP (it answers 131072 for every cart).
        SavePlan::BackupAndWrite => {
            let from = found_sav_path(root, stem);
            let to = backup_path(&from);
            match std::fs::rename(&from, &to) {
                Ok(()) => eprintln!(
                    "slot: save ram: {} was {} of filler past the {} bytes now reported; moved \
                     it to {} and wrote the shorter save",
                    from.display(),
                    describe(dropped),
                    sav.len(),
                    to.display()
                ),
                Err(e) => {
                    // Nothing was moved, so nothing may be written: leaving both files alone is
                    // the only outcome here that cannot lose a save.
                    eprintln!(
                        "slot: save ram: could not move {} aside ({e}); refusing to shrink it \
                         from {} to {} bytes",
                        from.display(),
                        old.as_deref().map_or(0, <[u8]>::len),
                        sav.len()
                    );
                    return Ok(false);
                }
            }
        }
        SavePlan::RefuseShrink => {
            eprintln!(
                "slot: save ram: refusing to shrink {} from {} to {} bytes -- what would be \
                 dropped holds {}",
                found_sav_path(root, stem).display(),
                old.as_deref().map_or(0, <[u8]>::len),
                sav.len(),
                describe(dropped)
            );
            return Ok(false);
        }
        SavePlan::RefuseCompressed => {
            eprintln!(
                "slot: save ram: refusing to overwrite the compressed RetroArch save {} \
                 (turn off RetroArch's save-file compression and re-export it)",
                found_sav_path(root, stem).display()
            );
            return Ok(false);
        }
    }
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    atomic_write(&path, sav)?;
    Ok(true)
}

/// mGBA standalone writes `.sav`, RetroArch's libretro cores write `.srm`. Both are the
/// same battery bytes, so a card carrying either has a real save on it. Only `.sav` is ever
/// written, which makes it the newer of the two whenever both exist.
pub fn read_sav(root: &Path, stem: &str) -> Option<Vec<u8>> {
    std::fs::read(sav_path(root, stem))
        .or_else(|_| std::fs::read(crate::root::saves_dir(root).join(format!("{stem}.srm"))))
        .ok()
}

/// What may be handed to a core, as opposed to `read_sav`, which is what is on the card.
///
/// A save RetroArch compressed is an rzip container: 20 bytes of its own header where the
/// game's first bytes belong, then a deflate stream. A core handed that as its save ram takes
/// it literally and reads a save it cannot parse, which the game shows as a corrupted save
/// rather than as a missing one. Loading nothing gives it a fresh save it can then write over,
/// and it is the same answer the write side gives when it leaves the file alone -- so a card
/// with a compressed save on it neither loses the file nor pretends to use it.
pub fn load_sav(root: &Path, stem: &str) -> Option<Vec<u8>> {
    let data = read_sav(root, stem)?;
    if is_rzip(&data) {
        eprintln!(
            "slot: save ram: {} is a compressed RetroArch save; loading nothing rather than \
             feeding its rzip bytes to the core",
            found_sav_path(root, stem).display()
        );
        return None;
    }
    Some(data)
}

/// Where `SavePlan::BackupAndWrite` puts the file it replaces
/// `Saves/<stem>.sav.bak-<bytes>`, naming the size it had, so a card carrying two of them says
/// which is which without a stat.
///
/// A numbered sibling is used when that name is taken, rather than overwriting: the whole
/// reason this file is renamed instead of deleted is that nobody can prove it is worthless, and
/// that argument does not expire the second time a cart hits this. The loop bound is a
/// formality so an unwritable name cannot become an infinite one; past it the plain name is
/// reused, which is still better than refusing to save.
fn backup_path(from: &Path) -> PathBuf {
    let name = from.file_name().and_then(|n| n.to_str()).unwrap_or("save");
    let size = std::fs::metadata(from).map_or(0, |m| m.len());
    let candidate = |n: usize| {
        let nth = if n == 1 {
            String::new()
        } else {
            format!("-{n}")
        };
        from.with_file_name(format!("{name}.bak-{size}{nth}"))
    };
    for n in 1..100 {
        let path = candidate(n);
        if !path.exists() {
            return path;
        }
    }
    candidate(1)
}

/// Which of the two files this cart's save actually lives in. `read_sav` prefers `.sav` and
/// falls back to `.srm`, so a message that always names `.sav` sends the player to a file that
/// is not the one being talked about.
fn found_sav_path(root: &Path, stem: &str) -> PathBuf {
    let sav = sav_path(root, stem);
    if sav.exists() {
        return sav;
    }
    let srm = crate::root::saves_dir(root).join(format!("{stem}.srm"));
    if srm.exists() {
        srm
    } else {
        sav
    }
}

/// The counterpart to the resume write in `flush`. Without this the cart is seated on the
/// next boot but the game restarts.
///
/// Takes `core` rather than resolving it here: the caller already has to know which core it
/// is about to open, and asking this function to work it out too would be a second,
/// independent read of `selected_core.ini` for the same cart in the same breath as the
/// first. `session.rs` resolves it once per insert and hands that single value to both this
/// and `open_core`, which is what keeps the resume directory and the dylib from disagreeing
/// at that moment. It says nothing about later: `flush` and eject read the core `App` stored
/// from that same resolution rather than asking again, which is what keeps them agreeing too.
pub fn read_resume(root: &Path, core: Core, stem: &str) -> Option<Vec<u8>> {
    StateRing::new(root, core, stem)
        .read_resume()
        .ok()
        .flatten()
}

fn sav_path(root: &Path, stem: &str) -> PathBuf {
    crate::root::saves_dir(root).join(format!("{stem}.sav"))
}
