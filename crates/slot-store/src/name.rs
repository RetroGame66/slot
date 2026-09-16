//! What a dumped file is called, as a title.
//!
//! A rom's own header title is twelve characters and in the publisher's language — `POKEMON
//! EMER`, `ROCKMANZERO` — while the file name is the one somebody wrote out in full. The shelf
//! shows the file name, and everything that has to agree with what the shelf shows reads it
//! through here: the printed label, the line of type under the row, and the letter ring's
//! bucket. That last one is why these live in the store rather than beside the drawing code —
//! the scan is what decides which cart belongs to which letter, and it cannot ask the ui.
//!
//! The tags are parsed rather than discarded because a dump's filename carries them as a run of
//! parenthesised facts, and each one is worth being able to show on its own.
//!
//! Whether the bracketed tags (`(USA)`, `[中]`, `[日]`, …) are stripped off is a *card*
//! preference, read once from `System/labels.txt` at boot — see [`init_label_config`]. Some
//! packs name their dumps with a language suffix (`[中]`/`[日]`/`[英]`) to tell editions apart;
//! turning tag-stripping off keeps those suffixes visible.

use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};

/// A dumped filename carries region and revision tags and separates title from subtitle
/// with a spaced hyphen. A bare hyphen is part of a word, so `Spider-Man` keeps its own.
///
/// This is the *display* cleaning: it honours the card's `strip_tags` setting. When stripping
/// is off, the bracketed tags are kept (only whitespace is normalised) so a name like
/// `[中]魂斗罗` shows in full.
pub fn clean_label(stem: &str) -> String {
    if strip_tags() {
        clean_label_stripped(stem)
    } else {
        // Keep the bracketed tags (e.g. `[中]`/`[日]`/`[英]` that some packs use to mark
        // language): only trim and collapse internal whitespace, keeping every character.
        let mut out = String::with_capacity(stem.len());
        let mut prev_ws = true;
        for ch in stem.chars() {
            if ch.is_whitespace() {
                if !prev_ws {
                    out.push(' ');
                    prev_ws = true;
                }
            } else {
                out.push(ch);
                prev_ws = false;
            }
        }
        out.trim().to_string()
    }
}

/// The original `clean_label`: drop every parenthesised/bracketed group and any bare hyphen,
/// collapse runs of whitespace to a single space. Used wherever the *filing* name matters — the
/// letter-ring bucket — so carts still sort by their base title even when the displayed title
/// keeps its tags. Always strips, regardless of the card setting.
pub fn clean_label_stripped(stem: &str) -> String {
    let mut bare = String::with_capacity(stem.len());
    let mut depth = 0u32;
    for ch in stem.chars() {
        match ch {
            '(' | '[' => depth += 1,
            ')' | ']' => depth = depth.saturating_sub(1),
            _ if depth == 0 => bare.push(ch),
            _ => {}
        }
    }

    let mut out = String::with_capacity(bare.len());
    for word in bare.split_whitespace().filter(|w| *w != "-") {
        if !out.is_empty() {
            out.push(' ');
        }
        out.push_str(word);
    }
    if out.is_empty() {
        stem.to_string()
    } else {
        out
    }
}

/// The bracketed groups `clean_label` throws away, in the order they appeared. A dump's
/// filename carries them as one run of parentheses — `(USA, Europe) (Rev 1)` — and each group
/// is one fact about this dump rather than about the game, which is why they are worth
/// keeping apart from the title instead of inside it.
///
/// One tag per group, not per comma: `(USA, Europe)` is a single release in two regions, and
/// splitting it would claim two.
pub fn label_tags(stem: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut depth = 0u32;
    let mut cur = String::new();
    for ch in stem.chars() {
        match ch {
            '(' | '[' => {
                depth += 1;
                if depth == 1 {
                    cur.clear();
                    continue;
                }
            }
            ')' | ']' => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    let t = cur.trim();
                    if !t.is_empty() {
                        out.push(t.to_string());
                    }
                    continue;
                }
            }
            _ => {}
        }
        if depth >= 1 {
            cur.push(ch);
        }
    }
    out
}

/// Whether `clean_label` strips the bracketed tags off a name. Read once from
/// `System/labels.txt` on the card at boot (see [`init_label_config`]); defaults to `true`,
/// which is the long-standing behaviour, so a card with no such file is unchanged.
///
/// An `AtomicBool` rather than a `OnceLock` only so the unit tests can re-establish state
/// between cases — in the running frontend it is still written exactly once, at boot.
static STRIP_TAGS: AtomicBool = AtomicBool::new(true);

/// Set the tag-stripping flag. Called once from [`init_label_config`] at boot; in production the
/// frontend never writes it again, so the resolved value is fixed for the whole session.
pub fn set_strip_tags(value: bool) {
    STRIP_TAGS.store(value, Ordering::Relaxed);
}

fn strip_tags() -> bool {
    STRIP_TAGS.load(Ordering::Relaxed)
}

/// Read the `strip_tags` preference from `System/labels.txt` on the card.
///
/// The file follows the same plain-text, comment-friendly convention as `theme.txt` and
/// `mask.txt`: one setting per line, `#` starts a comment, and anything unreadable or absent
/// leaves the default in place. A line
///
/// ```text
/// strip_tags off    # keep [中]/[日]/[英] suffixes in game names
/// ```
///
/// turns tag-stripping off (names keep their suffixes); `strip_tags on` — or no file at all —
/// keeps the original behaviour of stripping the tags.
pub fn init_label_config(root: &Path) {
    let Ok(text) = std::fs::read_to_string(root.join("System").join("labels.txt")) else {
        return;
    };
    let mut strip = true; // default: strip, as before
    for line in text.lines() {
        let line = line.split('#').next().unwrap_or("").trim();
        if let Some(rest) = line.strip_prefix("strip_tags") {
            let token = rest
                .trim_start_matches('=')
                .trim()
                .split_whitespace()
                .next()
                .unwrap_or("");
            match token.to_ascii_lowercase().as_str() {
                "off" | "false" | "no" | "0" => strip = false,
                "on" | "true" | "yes" | "1" => strip = true,
                _ => {}
            }
        }
    }
    set_strip_tags(strip);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stripped_is_the_classic_behaviour() {
        // No init call -> default strip = true.
        assert_eq!(clean_label("Spider-Man 2 (USA)"), "Spider-Man 2");
        assert_eq!(clean_label("Pokemon - Ruby Version (USA, Europe) (Rev 2)"), "Pokemon Ruby Version");
        assert_eq!(clean_label("[中]魂斗罗"), "魂斗罗");
        assert_eq!(clean_label("(USA) (Rev 1)"), "(USA) (Rev 1)");
        assert_eq!(clean_label(""), "");
    }

    #[test]
    fn stripped_variant_always_strips() {
        assert_eq!(clean_label_stripped("[中]魂斗罗"), "魂斗罗");
        assert_eq!(clean_label_stripped("Wario Land 4 - Time Attack"), "Wario Land 4 Time Attack");
    }

    #[test]
    fn keep_mode_preserves_tags() {
        set_strip_tags(false);
        assert_eq!(clean_label("[中]魂斗罗"), "[中]魂斗罗");
        assert_eq!(clean_label("Pokemon - Ruby Version (USA, Europe) (Rev 2)"),
                   "Pokemon - Ruby Version (USA, Europe) (Rev 2)");
        assert_eq!(clean_label("  [日]  超级马里奥   "), "[日] 超级马里奥");
        // restore default for any later test in this process
        set_strip_tags(true);
    }
}
