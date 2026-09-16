//! Times each of the shelf's "fixed furniture" builders one by one — **on the device**.
//!
//! Why the device: `slot-ui` depends on `slot-power`'s `std::os::unix`, so a Windows host will
//! not build it; and even where it does build, an absolute duration measured on a desktop means
//! nothing. The device reports one total for the lot (`fixed faces 705 ms`) with no breakdown,
//! so this is pushed over and run once to split that number up.
//!
//! How to run it:
//!     adb push <this binary> /tmp/face_timing
//!     adb shell "chmod +x /tmp/face_timing && /tmp/face_timing"
//!
//! Built the way `toolchain/letters-check` is: an example that links the **real functions out of
//! the library** rather than standing in a copy of them.

use std::time::Instant;

fn ms(t: Instant) -> f64 {
    t.elapsed().as_secs_f64() * 1000.0
}

fn main() {
    // A warm-up pass. With the font unparsed, the allocator cold and nothing in the instruction
    // cache, the first numbers measured are noise.
    for ch in slot_ui::letters::SLOTS {
        let _ = slot_ui::letters::letter_face(ch);
    }
    let _ = slot_ui::menu_face("重启");
    let _ = slot_ui::shelf_title_face("宝可梦");
    let _ = slot_ui::toast_face(slot_ui::Toast::StateSaved);
    let _ = slot_ui::icon_face(slot_ui::Icon::Volume, 24.0, [245, 242, 239]);
    let _ = slot_ui::socket_face(slot_store::Core::Mgba);

    let mut rows: Vec<(&str, f64, usize)> = Vec::new();

    let t = Instant::now();
    let mut n = 0;
    for ch in slot_ui::letters::SLOTS {
        n += slot_ui::letters::letter_face(ch).rgba.len();
    }
    rows.push(("letter faces x27", ms(t), n));

    let (cw, chh) = slot_ui::letters::capsule_size();
    let t = Instant::now();
    let cap = slot_ui::letters::capsule_face(cw, chh);
    rows.push(("capsule shell", ms(t), cap.rgba.len()));

    let t = Instant::now();
    let rg = slot_ui::letters::ridge_face();
    rows.push(("ridge", ms(t), rg.rgba.len()));

    let t = Instant::now();
    let a = slot_ui::cart_shadow();
    let b = slot_ui::chip_shadow_face();
    rows.push(("cart/chip shadows", ms(t), a.rgba.len() + b.rgba.len()));

    let t = Instant::now();
    let mut n = 0;
    for c in slot_store::Core::ALL {
        n += slot_ui::socket_face(c).rgba.len();
        n += slot_ui::chip_face(Some(c)).rgba.len();
    }
    n += slot_ui::chip_face(None).rgba.len();
    rows.push(("sockets + chips", ms(t), n));

    let t = Instant::now();
    let mut n = 0;
    for i in slot_ui::Icon::ALL {
        n += slot_ui::icon_face(i, 24.0, [245, 242, 239]).rgba.len();
    }
    rows.push(("HUD icons x10", ms(t), n));

    let t = Instant::now();
    let mut n = 0;
    for x in slot_ui::Toast::ALL {
        n += slot_ui::toast_face(x).rgba.len();
    }
    rows.push(("toasts x4 (CJK)", ms(t), n));

    let t = Instant::now();
    let mut n = 0;
    for c in slot_ui::PowerChoice::ALL {
        n += slot_ui::menu_face(c.text()).rgba.len();
    }
    rows.push(("power menu x2", ms(t), n));

    // The in-game menu and link groups, stood in for by a batch of representative strings —
    // CJK titles and Latin codes mixed, as they are on the card.
    let menu_strings = [
        "继续游戏", "即时存档", "读取存档", "金手指", "重新开始", "退出", "重启", "关机",
        "保存状态", "读取状态", "联机", "等待对手", "连接失败", "JOIN", "HOST", "LINK",
        "READY", "NO CHEATS", "核心", "MGBA", "GPSP", "快速前进", "倒带", "静音",
    ];
    let t = Instant::now();
    let mut n = 0;
    for s in menu_strings {
        n += slot_ui::menu_face(s).rgba.len();
    }
    rows.push(("menu rows x24 (mixed)", ms(t), n));

    let t = Instant::now();
    let mut n = 0;
    for s in ["宝可梦-绿宝石", "恶魔城-月之轮回", "超级机器人大战OG1", "Fire Emblem"] {
        n += slot_ui::shelf_title_face(s).rgba.len();
    }
    rows.push(("shelf titles x4", ms(t), n));

    let total: f64 = rows.iter().map(|r| r.1).sum();
    println!();
    println!("  {:<24} {:>10} {:>8}  {:>9}", "block", "ms", "share", "pixels");
    println!("  {}", "-".repeat(58));
    for (name, t, bytes) in &rows {
        println!("  {:<24} {:>10.2} {:>7.1}%  {:>9}", name, t, 100.0 * t / total, bytes / 4);
    }
    println!("  {}", "-".repeat(58));
    println!("  {:<24} {:>10.2}", "total", total);
    println!("\n  The device reports `fixed faces 705 ms` for the same work.");
}
