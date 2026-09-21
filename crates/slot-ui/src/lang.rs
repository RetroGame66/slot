//! Every word this frontend says, in one place, twice.
//!
//! The fork began as a translation: upstream's English was replaced with Chinese, line by line, in
//! whichever file each line lived in. That was the right shape for one language and the wrong shape
//! for two — with the strings scattered through ten files there is nothing to keep in step, and a
//! change made in one language is invisible in the other until someone sees it on a screen. So the
//! words are gathered here, where the two languages sit as two columns of the same list.
//!
//! **Two tables chosen at compile time, not one table chosen at runtime.** A running switch would
//! put both languages in every binary, and for the English build that is the one thing it must not
//! be: the point of that build is to be free of the other language. It would also mean every
//! release carries whichever language it is not.
//!
//! Chinese is the default, so the build that ships today is the build that shipped yesterday.
//! English is `--features lang-en`.
//!
//! **Where the English came from.** Most of it is not a translation. Upstream is an English program
//! and the fork replaced its words, so the original wording is recoverable from the commit the fork
//! was taken from — and it has been recovered phrase by phrase rather than reinvented: "State
//! Saved", "Power Off", "Restarting", "Bringing the radio up", "Back / Delete / Load" are all
//! upstream's own. What is genuinely new is what the fork added — the toasts for the cheat toggle
//! and the three audio profiles, the clock hint — and the shortcut card, which lives in
//! `shortcuts.rs` because its words are inseparable from its layout.
//!
//! Two things are deliberately *not* here. A key's own name ("A", "SELECT") is a button rather than
//! a word and never changes with the language. And a string the game supplies — a cart's title, a
//! cheat's description — belongs to the card, not to this build.

#[cfg(feature = "lang-en")]
mod words {
    /// The one key the clock screen names, and what pressing it does. The key is a button rather
    /// than a word, but it lives beside its label: a column with a gap in it is a column someone
    /// will fill wrongly.
    pub const SET_CLOCK_LABEL: &str = "set the clock";

    pub const TOAST_STATE_SAVED: &str = "State Saved";
    pub const TOAST_STATE_LOADED: &str = "State Loaded";
    pub const TOAST_CHEATS_ON: &str = "Cheats On";
    pub const TOAST_CHEATS_OFF: &str = "Cheats Off";
    pub const TOAST_AUDIO_STABLE: &str = "Audio: Stable";
    pub const TOAST_AUDIO_BALANCED: &str = "Audio: Balanced";
    pub const TOAST_AUDIO_STRICT: &str = "Audio: Strict";

    pub const POWER_RESTART: &str = "Restart";
    pub const POWER_OFF: &str = "Power Off";
    pub const SHUTDOWN_RESTART: &str = "Restarting";
    pub const SHUTDOWN_POWER_OFF: &str = "Powering Down";

    pub const SWITCHER_LEGEND: [(&str, &str); 3] = [("B", "Back"), ("Y", "Delete"), ("A", "Load")];

    pub const CORE_CANCEL: &str = "Cancel";
    pub const CORE_SWAP: &str = "Swap";
    pub const CORE_CHOOSE: &str = "Choose";

    pub const LINK_ROW: &str = "Link";
    pub const LINK_HOST: &str = "Host";
    pub const LINK_JOIN: &str = "Join";

    pub const LINK_RADIO: &str = "Bringing the radio up";
    pub const LINK_WAITING: &str = "Looking for the other player";
    pub const LINK_FAIL_RADIO: &str = "The radio did not come up";
    pub const LINK_FAIL_NOBODY: &str = "Nobody arrived";
    pub const LINK_FAIL_PEER: &str = "The other player vanished";
    pub const LINK_FAIL_CANCELLED: &str = "Cancelled";

    pub const UNDO_SAVE: &str = "undo save";
    pub const UNDO_LOAD: &str = "undo load";
}

#[cfg(not(feature = "lang-en"))]
mod words {
    pub const SET_CLOCK_LABEL: &str = "设置时钟";

    pub const TOAST_STATE_SAVED: &str = "存档已保存";
    pub const TOAST_STATE_LOADED: &str = "存档已读取";
    pub const TOAST_CHEATS_ON: &str = "金手指已开启";
    pub const TOAST_CHEATS_OFF: &str = "金手指已关闭";
    pub const TOAST_AUDIO_STABLE: &str = "音频：稳定";
    pub const TOAST_AUDIO_BALANCED: &str = "音频：均衡";
    pub const TOAST_AUDIO_STRICT: &str = "音频：严格";

    pub const POWER_RESTART: &str = "重启";
    pub const POWER_OFF: &str = "关机";
    pub const SHUTDOWN_RESTART: &str = "正在重启";
    pub const SHUTDOWN_POWER_OFF: &str = "正在关机";

    pub const SWITCHER_LEGEND: [(&str, &str); 3] = [("B", "返回"), ("Y", "删除"), ("A", "读取")];

    pub const CORE_CANCEL: &str = "取消";
    pub const CORE_SWAP: &str = "切换";
    pub const CORE_CHOOSE: &str = "选择";

    pub const LINK_ROW: &str = "联机";
    pub const LINK_HOST: &str = "主机";
    pub const LINK_JOIN: &str = "加入";

    pub const LINK_RADIO: &str = "正在启动无线连接";
    pub const LINK_WAITING: &str = "正在寻找其他玩家";
    pub const LINK_FAIL_RADIO: &str = "无线连接未能启动";
    pub const LINK_FAIL_NOBODY: &str = "无人加入";
    pub const LINK_FAIL_PEER: &str = "对方已断开";
    pub const LINK_FAIL_CANCELLED: &str = "已取消";

    pub const UNDO_SAVE: &str = "撤销存档";
    pub const UNDO_LOAD: &str = "撤销读取";
}

pub use words::*;
