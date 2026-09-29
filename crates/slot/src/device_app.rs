use std::path::PathBuf;
use std::time::{Duration, Instant};

use slot::frontend::Frontend;
use slot::input::DeviceInput;
use slot_gfx::{Compositor, FbdevSurface, Surface};
use slot_power::DevicePlatform;

/// Where BaseOS mounts the card slot has never been checked against a running device, so
/// `launch.sh` exports `SLOT_ROOT` and this is only what is left if it did not.
const CARD: &str = "/mnt/sdcard";

/// The panel is 60 Hz and EGL is asked to lock to it, but a driver that ignores the swap
/// interval would spin this loop as fast as the GPU can clear, so the frame is timed too.
const FRAME: Duration = Duration::from_micros(16_667);

/// Append one line to `System/boot.err`, truncated once per boot by `run`. Best effort: a card
/// that is read only or half mounted is not a reason to fail a boot, and this is only ever for
/// a human reading the card afterwards. A device has no console, and a startup that dies
/// before the frontend exists otherwise leaves nothing behind at all.
fn note_startup(root: &std::path::Path, line: &str) {
    use std::io::Write;
    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(root.join("System/boot.err"))
    {
        let _ = writeln!(f, "{line}");
    }
}

pub fn run() {
    let root = std::env::var_os("SLOT_ROOT")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(CARD));
    // This boot's startup log, clean. `boot.log` is only written once the frontend exists, so
    // a failure before that would otherwise say nothing anywhere.
    let _ = std::fs::write(root.join("System/boot.err"), "");
    let mut surface = match FbdevSurface::new() {
        Ok(s) => s,
        Err(e) => {
            eprintln!("slot: {e}");
            note_startup(&root, &format!("surface: {e}"));
            return;
        }
    };
    let mut compositor = match Compositor::new(&surface) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("slot: {e}");
            note_startup(&root, &format!("compositor: {e}"));
            return;
        }
    };
    // Anything the compositor wanted to say but could not print — today, why the screen
    // reflection was left off. Goes on the card, where a PC can read it.
    for line in compositor.warnings() {
        note_startup(&root, &format!("warn: {line}"));
    }
    // The card's panel mask, if it has one, and before the first frame is drawn. Same shape as
    // the typeface: a setting the card owns, read once, and a failure that leaves the built-in
    // table in place rather than taking the picture away.
    if let Some(mask) = slot::root::panel_mask(&root) {
        compositor.set_panel_mask(&mask);
    }
    let platform = DevicePlatform::new(root.clone());
    eprintln!("slot: {}", platform.report());
    platform.trace_boot();
    // The card's panel mask (and colour correction) is now owned by the app and pushed to the
    // compositor every frame from `frontend.render`, so it is not set here.
    let mut frontend = Frontend::boot(Box::new(platform));
    frontend.upload_faces(&mut compositor);
    let mut input = DeviceInput::open(&root);
    loop {
        let began = Instant::now();
        frontend.render(&mut compositor, surface.window_size());
        if let Err(e) = surface.swap() {
            eprintln!("slot: {e}");
            return;
        }
        frontend.advance(&mut input);
        if frontend.restarting() {
            frontend.restart();
        }
        if frontend.powering_off() {
            frontend.poweroff();
            return;
        }
        if let Some(left) = FRAME.checked_sub(began.elapsed()) {
            std::thread::sleep(left);
        }
    }
}
