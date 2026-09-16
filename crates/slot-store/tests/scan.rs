mod common;

use common::tmp_root;
use slot_store::{scan, scan_cached};
use tempfile::TempDir;

fn write_rom(d: &TempDir, name: &str, title: &str) {
    let mut rom = vec![0u8; 0x100];
    rom[0xa0..0xa0 + title.len()].copy_from_slice(title.as_bytes());
    std::fs::write(d.path().join("Games").join(name), rom).expect("write rom");
}

fn write_png(d: &TempDir, name: &str) {
    std::fs::write(d.path().join("Labels").join(name), b"\x89PNG\r\n\x1a\n").expect("write png");
}

#[test]
fn a_png_in_labels_is_paired_to_its_rom_by_stem() {
    let d = tmp_root();
    write_rom(&d, "Pokemon Emerald.gba", "POKEMON EMER");
    write_png(&d, "Pokemon Emerald.png");
    write_rom(&d, "Advance Wars.gba", "ADVANCEWARS");
    let carts = scan(d.path()).unwrap();
    assert_eq!(carts.len(), 2);
    assert_eq!(carts[0].stem, "Advance Wars");
    assert!(carts[0].label.is_none());
    assert!(
        carts[1].label.is_some(),
        "a label in Labels/ was not picked up"
    );
    assert_eq!(carts[1].title, "POKEMON EMER");
}

#[test]
fn scan_ignores_non_gba_files() {
    let d = tmp_root();
    write_rom(&d, "Real.gba", "REAL");
    std::fs::write(d.path().join("Games/notes.txt"), "hi").unwrap();
    assert_eq!(scan(d.path()).unwrap().len(), 1);
}

#[test]
fn an_appledouble_sidecar_is_not_shelved_as_a_cart() {
    let d = tmp_root();
    write_rom(&d, "Metroid Fusion.gba", "METROID");
    // Copying a rom onto a FAT card from macOS leaves this beside it, carrying the same
    // extension and the same stem, so only the leading dot tells the two apart.
    write_rom(&d, "._Metroid Fusion.gba", "METROID");
    let carts = scan(d.path()).unwrap();
    assert_eq!(carts.len(), 1, "an AppleDouble sidecar reached the shelf");
    assert_eq!(carts[0].stem, "Metroid Fusion");
}

#[test]
fn header_title_of_a_truncated_rom_is_none_not_a_panic() {
    let d = tmp_root();
    std::fs::write(d.path().join("Games/Tiny.gba"), [0u8; 8]).unwrap();
    assert!(scan(d.path()).unwrap()[0].title.is_empty());
}

#[test]
fn a_header_title_that_is_not_text_is_dropped_rather_than_mangled() {
    let d = tmp_root();
    let mut rom = vec![0u8; 0x100];
    rom[0xa0..0xac].copy_from_slice(&[0xffu8; 12]);
    std::fs::write(d.path().join("Games/Garbage.gba"), rom).unwrap();
    assert!(scan(d.path()).unwrap()[0].title.is_empty());
}

#[test]
fn a_root_with_no_games_directory_scans_as_empty() {
    let d = tempfile::tempdir().unwrap();
    assert!(scan(d.path()).unwrap().is_empty());
}

// The cache is what a boot calls, so everything the plain scan promises has to hold through
// it as well, and the two have to agree about the same card.

#[test]
fn the_cache_agrees_with_a_full_scan() {
    let d = tmp_root();
    write_rom(&d, "Pokemon Emerald.gba", "POKEMON EMER");
    write_rom(&d, "Advance Wars.gba", "ADVANCEWARS");
    write_png(&d, "Pokemon Emerald.png");
    assert_eq!(scan_cached(d.path()).unwrap(), scan(d.path()).unwrap());
}

/// The whole point: a card that has not changed must not go back to the roms. Rewriting the
/// title the cache holds leaves the header on disk saying something else, so a run that
/// returns the cache's answer is one that did not open a single rom.
#[test]
fn an_unchanged_card_is_answered_from_the_cache_without_reading_a_rom() {
    let d = tmp_root();
    write_rom(&d, "Emerald.gba", "POKEMON EMER");
    assert_eq!(scan_cached(d.path()).unwrap()[0].title, "POKEMON EMER");

    let cache = d.path().join("System/library.cache");
    let text = std::fs::read_to_string(&cache).expect("no cache written");
    std::fs::write(&cache, text.replace("POKEMON EMER", "FROM CACHE")).unwrap();

    assert_eq!(scan_cached(d.path()).unwrap()[0].title, "FROM CACHE");
}

#[test]
fn a_rom_added_after_the_first_scan_is_shelved() {
    let d = tmp_root();
    write_rom(&d, "Emerald.gba", "POKEMON EMER");
    assert_eq!(scan_cached(d.path()).unwrap().len(), 1);
    // Twelve bytes is the whole of the header title field, so this is a title rather than
    // the start of one.
    write_rom(&d, "Metroid.gba", "METROID");
    let carts = scan_cached(d.path()).unwrap();
    assert_eq!(carts.len(), 2);
    assert!(carts
        .iter()
        .any(|c| c.stem == "Metroid" && c.title == "METROID"));
}

#[test]
fn a_rom_removed_after_the_first_scan_leaves_the_shelf() {
    let d = tmp_root();
    write_rom(&d, "Emerald.gba", "POKEMON EMER");
    write_rom(&d, "Metroid.gba", "METROID");
    assert_eq!(scan_cached(d.path()).unwrap().len(), 2);
    std::fs::remove_file(d.path().join("Games/Metroid.gba")).unwrap();
    let carts = scan_cached(d.path()).unwrap();
    assert_eq!(carts.len(), 1);
    assert_eq!(carts[0].stem, "Emerald");
}

#[test]
fn a_label_added_after_the_first_scan_is_picked_up() {
    let d = tmp_root();
    write_rom(&d, "Emerald.gba", "POKEMON EMER");
    assert!(scan_cached(d.path()).unwrap()[0].label.is_none());
    write_png(&d, "Emerald.png");
    assert!(scan_cached(d.path()).unwrap()[0].label.is_some());
}

/// A cache that does not parse is a slow boot, not a failure to boot.
#[test]
fn a_corrupt_cache_is_a_full_scan_rather_than_an_error() {
    let d = tmp_root();
    write_rom(&d, "Emerald.gba", "POKEMON EMER");
    std::fs::write(
        d.path().join("System/library.cache"),
        b"\x00 not a cache at all",
    )
    .unwrap();
    let carts = scan_cached(d.path()).unwrap();
    assert_eq!(carts.len(), 1);
    assert_eq!(carts[0].title, "POKEMON EMER");
}

/// A cache missing a row would take a cart off the shelf even though nothing on the card
/// moved, so the row count is part of what makes it trustworthy: a `rows 2` header with one
/// row under it has to read as no cache rather than as a one cart shelf.
#[test]
fn a_cache_with_a_row_torn_out_is_a_full_scan() {
    let d = tmp_root();
    write_rom(&d, "Emerald.gba", "POKEMON EMER");
    write_rom(&d, "Metroid.gba", "METROID FUSION");
    assert_eq!(scan_cached(d.path()).unwrap().len(), 2);

    let cache = d.path().join("System/library.cache");
    let text = std::fs::read_to_string(&cache).unwrap();
    let mut lines: Vec<&str> = text.lines().collect();
    lines.pop();
    std::fs::write(&cache, lines.join("\n")).unwrap();

    assert_eq!(scan_cached(d.path()).unwrap().len(), 2);
}

/// A card whose games are named with a title that itself carries the cache's own separators
/// must round trip: a rom title is not ours to assume printable.
#[test]
fn a_title_with_a_separator_in_it_survives_the_cache() {
    let d = tmp_root();
    write_rom(&d, "Odd.gba", "ODD\tTITLE");
    assert_eq!(scan_cached(d.path()).unwrap()[0].title, "ODD\tTITLE");
    assert_eq!(scan_cached(d.path()).unwrap()[0].title, "ODD\tTITLE");
}

/// The letter ring files a cart under the name the shelf shows, which is the file name — never
/// the header title. The header is twelve characters of the publisher's language, so a card of
/// Chinese file names filed under it puts `POKEMON RUBY` and `ROCKMANZERO` on the ring and a
/// press of the dial lands on a letter with nothing under it.
#[test]
fn the_ring_files_a_cart_under_its_file_name_and_not_its_header() {
    let d = tmp_root();
    // A rom's header title says one thing and the file name another, which is the whole case.
    write_rom(&d, "宝可梦-红宝石 (CN).gba", "POKEMON RUBY");
    // Bracketed tags are not part of the name, and the letter has to see through them: this
    // cart is an `L`, not a `#`.
    write_rom(&d, "[GBA] 洛克人Zero1.gba", "ROCKMANZERO");
    write_rom(&d, "恶魔城-晓月圆舞曲.gba", "AKUMAJO");

    let carts = scan(d.path()).unwrap();
    // Grouped by letter, which is the shelf's order.
    let filed: Vec<(&str, char)> = carts.iter().map(|c| (c.stem.as_str(), c.initial)).collect();
    assert_eq!(
        filed,
        vec![
            ("宝可梦-红宝石 (CN)", 'B'),
            ("恶魔城-晓月圆舞曲", 'E'),
            ("[GBA] 洛克人Zero1", 'L'),
        ]
    );
    // And the header is still carried, for the things that want it.
    assert!(carts.iter().any(|c| c.title == "ROCKMANZERO"));
}

/// The same cart through the cache: `initial` is derived rather than stored, so it has to be
/// derived the same way from a row as from a rom. A cache that answered with a different letter
/// than the scan that wrote it would put the ring a boot out of step with the shelf.
#[test]
fn the_cache_files_a_cart_under_the_same_letter_as_the_scan() {
    let d = tmp_root();
    write_rom(&d, "宝可梦-红宝石 (CN).gba", "POKEMON RUBY");
    write_rom(&d, "[GBA] 洛克人Zero1.gba", "ROCKMANZERO");

    let first = scan_cached(d.path()).unwrap();
    let again = scan_cached(d.path()).unwrap();
    let letters = |cs: &[slot_store::Cart]| -> Vec<char> { cs.iter().map(|c| c.initial).collect() };
    assert_eq!(
        letters(&first),
        vec!['B', 'L'],
        "the first scan is already wrong"
    );
    assert_eq!(
        letters(&again),
        letters(&first),
        "the cache changed the letters"
    );
}

/// The shelf is grouped by letter, because the ring above it is an index and an index whose
/// entries are out of order is not one. Hanzi in code point order are not in pinyin order, so
/// the file name alone leaves the B carts scattered down the row and stepping the dial to C
/// lands behind the user.
#[test]
fn the_shelf_is_grouped_by_letter() {
    let d = tmp_root();
    // Written in an order that is neither: 宝 (B) sorts before 恶 (E) before 洛 (L) by code
    // point, and the file names below are handed over in the third of those orders.
    write_rom(&d, "恶魔城-晓月圆舞曲.gba", "AKUMAJO");
    write_rom(&d, "宝可梦-红宝石.gba", "POKEMON");
    write_rom(&d, "洛克人Zero1.gba", "ROCKMAN");
    write_rom(&d, "超级机器人大战A.gba", "SRW A");
    write_rom(&d, "宝可梦-蓝宝石.gba", "POKEMON");
    write_rom(&d, "(J) 洛克人Zero2.gba", "ROCKMAN");

    let carts = scan(d.path()).unwrap();
    let filed: Vec<(char, &str)> = carts.iter().map(|c| (c.initial, c.stem.as_str())).collect();
    assert_eq!(
        filed,
        vec![
            ('B', "宝可梦-红宝石"),
            ('B', "宝可梦-蓝宝石"),
            ('C', "超级机器人大战A"),
            ('E', "恶魔城-晓月圆舞曲"),
            ('L', "(J) 洛克人Zero2"),
            ('L', "洛克人Zero1"),
        ],
        "the shelf is not grouped by letter, or not by file name inside a letter"
    );
    // Non-decreasing, which is the property the dial's up and down depend on: down is always
    // forwards along the row and up is always back.
    assert!(carts.windows(2).all(|w| w[0].initial <= w[1].initial));
}
