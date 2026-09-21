//! 把 slot 开机那块「固定家具」的每个构建函数逐个计时 —— **在设备上跑**。
//!
//! 为什么要在设备上：`slot-ui` 依赖 `slot-power` 的 `std::os::unix`，Windows 宿主编不过；
//! 而且就算编得过，电脑上的绝对耗时也没有意义。设备上只有一个总数（`fixed faces 705 ms`），
//! 拆分不出来，所以把这个小程序推上去跑一次。
//!
//! 跑法：
//!     adb push <这个二进制> /tmp/face_timing
//!     adb shell "chmod +x /tmp/face_timing && /tmp/face_timing"
//!
//! 做法与 `toolchain/letters-check` 同一思路：把它当独立例子编，但用的是**库里的真函数**。

use std::time::Instant;

fn ms(t: Instant) -> f64 {
    t.elapsed().as_secs_f64() * 1000.0
}

fn main() {
    // 热身：字体首解析、分配器、指令缓存都还没热时量到的是噪声。
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
    rows.push(("字母面 ×27", ms(t), n));

    let t = Instant::now();
    let a = slot_ui::cart_shadow();
    let b = slot_ui::chip_shadow_face();
    rows.push(("卡带/芯片投影", ms(t), a.rgba.len() + b.rgba.len()));

    let t = Instant::now();
    let mut n = 0;
    for c in slot_store::Core::ALL {
        n += slot_ui::socket_face(c).rgba.len();
        n += slot_ui::chip_face(Some(c)).rgba.len();
    }
    n += slot_ui::chip_face(None).rgba.len();
    rows.push(("插座 + 芯片", ms(t), n));

    let t = Instant::now();
    let mut n = 0;
    for i in slot_ui::Icon::ALL {
        n += slot_ui::icon_face(i, 24.0, [245, 242, 239]).rgba.len();
    }
    rows.push(("HUD 图标 ×10", ms(t), n));

    let t = Instant::now();
    let mut n = 0;
    for x in slot_ui::Toast::ALL {
        n += slot_ui::toast_face(x).rgba.len();
    }
    rows.push(("提示条 ×4（中文）", ms(t), n));

    let t = Instant::now();
    let mut n = 0;
    for c in slot_ui::PowerChoice::ALL {
        n += slot_ui::menu_face(c.text()).rgba.len();
    }
    rows.push(("电源菜单 ×2", ms(t), n));

    // 游戏内菜单/联机那几组，用一批有代表性的中英混排字符串代替。
    let menu_strings = [
        "继续游戏",
        "即时存档",
        "读取存档",
        "金手指",
        "重新开始",
        "退出",
        "重启",
        "关机",
        "保存状态",
        "读取状态",
        "联机",
        "等待对手",
        "连接失败",
        "JOIN",
        "HOST",
        "LINK",
        "READY",
        "NO CHEATS",
        "核心",
        "MGBA",
        "GPSP",
        "快速前进",
        "倒带",
        "静音",
    ];
    let t = Instant::now();
    let mut n = 0;
    for s in menu_strings {
        n += slot_ui::menu_face(s).rgba.len();
    }
    rows.push(("菜单行 ×24（中英混排）", ms(t), n));

    let t = Instant::now();
    let mut n = 0;
    for s in [
        "宝可梦-绿宝石",
        "恶魔城-月之轮回",
        "超级机器人大战OG1",
        "Fire Emblem",
    ] {
        n += slot_ui::shelf_title_face(s).rgba.len();
    }
    rows.push(("货架标题 ×4", ms(t), n));

    let total: f64 = rows.iter().map(|r| r.1).sum();
    println!();
    println!(
        "  {:<24} {:>10} {:>8}  {:>9}",
        "块", "耗时 ms", "占比", "像素"
    );
    println!("  {}", "-".repeat(58));
    for (name, t, bytes) in &rows {
        println!(
            "  {:<24} {:>10.2} {:>7.1}%  {:>9}",
            name,
            t,
            100.0 * t / total,
            bytes / 4
        );
    }
    println!("  {}", "-".repeat(58));
    println!("  {:<24} {:>10.2}", "合计", total);
    println!("\n  设备上 slot 自己报的 fixed faces 是 705 ms。");
}
