use std::collections::HashSet;
use std::fmt;
use std::fs::Metadata;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use crate::atomic::atomic_write;
use crate::gba::{header, sgb_enhanced};

/// Which machine a cart is for, and so which shelf it belongs on.
///
/// Three shelves, because that is how a card is filed and how a hand asks for one. Decided off
/// the *extension* rather than by opening the rom, for the same reason everything else here is:
/// the fast path of `scan_cached` reads a name and nothing else, and a property that costs a
/// name to answer is free.
///
/// A zip deliberately gets no fourth. Two reasons, and the second is the one that decides it:
/// the cores are handed the bytes of the file this frontend read off the card, so a zip would
/// reach the core as an archive rather than as a rom — there is no extraction stage here the
/// way RetroArch has one — and worse, an extension is the only thing that tells these three
/// apart for free. A zip throws that away and leaves the machine unknowable without opening the
/// file to look inside it.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum System {
    Gba,
    Gb,
    Gbc,
}

impl System {
    /// From whatever follows the last dot in a file name. Compared without case, because a
    /// card written on another machine arrives in whatever case that machine felt like.
    pub fn from_extension(ext: &str) -> Option<Self> {
        if ext.eq_ignore_ascii_case("gba") {
            Some(System::Gba)
        } else if ext.eq_ignore_ascii_case("gb") {
            Some(System::Gb)
        } else if ext.eq_ignore_ascii_case("gbc") {
            Some(System::Gbc)
        } else {
            None
        }
    }

    /// The folder this machine's carts and faces go in, on a card that sorts them.
    pub fn dir_name(self) -> &'static str {
        match self {
            System::Gba => "GBA",
            System::Gb => "GB",
            System::Gbc => "GBC",
        }
    }
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Cart {
    /// Filename stem, which is the key for labels, saves and states. Not a content hash.
    pub stem: String,
    pub rom: PathBuf,
    pub label: Option<PathBuf>,
    pub title: String,
    /// The four character header game code, empty when the rom has none.
    pub code: String,
    /// Which bucket the shelf's letter ring files this cart under: the pinyin initial of the
    /// first character of the name the shelf shows — see `bucket` for why it is the file name
    /// and not the header title. Derived here rather than stored, because it is one binary
    /// search over a table already in the binary and the cache does not need a column for it.
    pub initial: char,
}

impl Cart {
    /// Which machine this cart is for, read off the rom's own extension.
    ///
    /// A method rather than a field, and derived rather than stored, because it is a fact about
    /// the file's name: one `extension` either way, and nothing for a second copy of it to
    /// disagree with.
    ///
    /// `Gba` for a rom whose extension is none of the three, which cannot reach a cart — `scan`
    /// and `scan_cached` both filter on `is_rom` first — so it is a default nothing ever takes.
    pub fn system(&self) -> System {
        self.rom
            .extension()
            .and_then(|e| e.to_str())
            .and_then(System::from_extension)
            .unwrap_or(System::Gba)
    }

    /// Whether this cart's rom is Super Game Boy enhanced — mGBA would draw an SGB border
    /// for it unless SLOT turns that off. Reads the rom on demand, so it is not part of the
    /// scan fast path; calling it per cart at boot would cost a header read apiece.
    pub fn sgb_enhanced(&self) -> bool {
        sgb_enhanced(&self.rom)
    }
}

#[derive(Debug)]
pub enum StoreError {
    Io(std::io::Error),
}

impl fmt::Display for StoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            StoreError::Io(e) => write!(f, "io: {e}"),
        }
    }
}

impl std::error::Error for StoreError {}

impl From<std::io::Error> for StoreError {
    fn from(e: std::io::Error) -> Self {
        StoreError::Io(e)
    }
}

/// An unmounted card or a card with no library is an empty shelf, not a boot failure.
///
/// Reads every rom's header, so a shelf built off this pays for the whole card. `scan_cached`
/// is what a boot calls; this is the reference the cache has to agree with.
pub fn scan(root: &Path) -> Result<Vec<Cart>, StoreError> {
    let games = list_names(&root.join("Games"), is_rom, true)?;
    let labels = labels_set(&root.join("Labels"))?;
    Ok(build(root, &games, &labels, None).0)
}

/// The shelf, re-read only where the card has changed.
///
/// A boot used to read the header of every cart on the card: three hundred games is three
/// hundred directory lookups and cluster chain walks on a card that is the slowest thing in
/// the machine, and it is the same answer every time until a game is added. This keeps the
/// answer on the card and checks whether it still applies before doing any of that work.
///
/// The check is the *shape* of the two directories the shelf is drawn from: the `Games` and
/// `Labels` directory timestamps, and the sorted file names under each. Adding, removing,
/// renaming or re-labelling a game all move one of those, so the cache is thrown away and
/// the card is read again — and when it is, the headers of the games that did not change are
/// reused from the cache rather than read back, so even a card with one new game on it pays
/// for one game rather than for three hundred.
///
/// What it does not catch is a rom rewritten in place under its own name with its own
/// timestamp untouched, which no directory listing can see. `System/library.cache` is a plain
/// file: delete it to force the slow path, which is the answer for a card that has been
/// patched rather than added to.
pub fn scan_cached(root: &Path) -> Result<Vec<Cart>, StoreError> {
    let games_dir = root.join("Games");
    let labels_dir = root.join("Labels");
    let games = list_names(&games_dir, is_rom, true)?;
    let labels = list_names(&labels_dir, is_png, true)?;
    let label_set = label_keys(&labels);
    let stamp = Stamp {
        games: dir_mtime(&games_dir),
        labels: dir_mtime(&labels_dir),
    };

    let cached = read_cache(root);
    if let Some(c) = &cached {
        if c.stamp == stamp && c.games == games && c.labels == labels && c.rows_agree() {
            return Ok(carts_from_rows(root, &c.rows, &label_set));
        }
    }

    let (carts, rows) = build(
        root,
        &games,
        &label_set,
        cached.as_ref().map(|c| c.rows.as_slice()),
    );
    write_cache(
        root,
        &Cache {
            stamp,
            games,
            labels,
            rows,
        },
    );
    Ok(carts)
}

// ---------------------------------------------------------------------------------------
// The cache
// ---------------------------------------------------------------------------------------

/// Under `System/` with the rest of slot's own bookkeeping. It describes the card rather than
/// the shelf the user curates, so it does not live beside the roms.
const CACHE: &str = "System/library.cache";

/// The first line, which is also the format's version. A stale stamp or a run of bytes that
/// only looks like one has to read as no cache at all rather than as a shelf with holes in
/// it, and the version is what makes that possible across a future change to the layout.
const MAGIC: &str = "slot-library 1";

/// The directory timestamps the names below were gathered at. Kept beside the names rather
/// than inferred: an empty directory and a directory that could not be read are both "no
/// names", and only the timestamp says which of the two the cache was written for.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
struct Stamp {
    games: i64,
    labels: i64,
}

struct Row {
    /// The file's own name in `Games/`, extension and all. The stem is derived from it
    /// rather than stored, so one name cannot disagree with the label it looks up.
    name: String,
    size: u64,
    mtime: i64,
    has_label: bool,
    title: String,
    code: String,
}

struct Cache {
    stamp: Stamp,
    /// Sorted, as `list_names` returns them. Compared as a whole: the order is part of the
    /// identity, and a cache written in another order is one this code did not write.
    games: Vec<String>,
    labels: Vec<String>,
    rows: Vec<Row>,
}

impl Cache {
    /// Whether the rows account for every game the cache claims. A truncated or hand-edited
    /// file can pass the name comparison and still be missing a row, which would take a cart
    /// off the shelf until the next change happened to rewrite it.
    fn rows_agree(&self) -> bool {
        self.rows.len() == self.games.len()
            && self
                .rows
                .iter()
                .zip(&self.games)
                .all(|(row, name)| row.name == *name)
    }
}

fn cache_path(root: &Path) -> PathBuf {
    root.join(CACHE)
}

fn read_cache(root: &Path) -> Option<Cache> {
    let text = std::fs::read_to_string(cache_path(root)).ok()?;
    parse_cache(&text)
}

/// Best effort. A card that will not take the write still boots, it only boots slowly — which
/// is what it did before the cache existed, so there is nothing here worth failing over.
fn write_cache(root: &Path, cache: &Cache) {
    let _ = atomic_write(&cache_path(root), render_cache(cache).as_bytes());
}

fn render_cache(cache: &Cache) -> String {
    let mut out = String::new();
    out.push_str(MAGIC);
    out.push('\n');
    section(&mut out, "games", cache.stamp.games, &cache.games);
    section(&mut out, "labels", cache.stamp.labels, &cache.labels);
    out.push_str(&format!("rows {}\n", cache.rows.len()));
    for row in &cache.rows {
        out.push_str(&format!(
            "{}\t{}\t{}\t{}\t{}\t{}\n",
            escape(&row.name),
            row.size,
            row.mtime,
            u8::from(row.has_label),
            escape(&row.title),
            escape(&row.code),
        ));
    }
    out
}

/// `<key> <stamp> <count>` followed by `count` escaped names, the shape `take_section` reads.
fn section(out: &mut String, key: &str, stamp: i64, names: &[String]) {
    out.push_str(&format!("{key} {stamp} {}\n", names.len()));
    for name in names {
        out.push_str(&escape(name));
        out.push('\n');
    }
}

fn parse_cache(text: &str) -> Option<Cache> {
    let mut lines = text.lines();
    if lines.next()? != MAGIC {
        return None;
    }
    let (games_stamp, games) = take_section(&mut lines, "games")?;
    let (labels_stamp, labels) = take_section(&mut lines, "labels")?;
    let count = lines.next()?.strip_prefix("rows ")?.parse().ok()?;
    let mut rows = Vec::with_capacity(count);
    for _ in 0..count {
        let mut fields = lines.next()?.split('\t');
        let row = Row {
            name: unescape(fields.next()?),
            size: fields.next()?.parse().ok()?,
            mtime: fields.next()?.parse().ok()?,
            has_label: fields.next()? == "1",
            title: unescape(fields.next()?),
            code: unescape(fields.next()?),
        };
        if fields.next().is_some() {
            return None;
        }
        rows.push(row);
    }
    Some(Cache {
        stamp: Stamp {
            games: games_stamp,
            labels: labels_stamp,
        },
        games,
        labels,
        rows,
    })
}

/// `<key> <stamp> <count>` followed by `count` escaped names.
fn take_section<'a>(lines: &mut std::str::Lines<'a>, key: &str) -> Option<(i64, Vec<String>)> {
    let rest = lines.next()?.strip_prefix(key)?.strip_prefix(' ')?;
    let (stamp, count) = rest.split_once(' ')?;
    let count: usize = count.parse().ok()?;
    let mut names = Vec::with_capacity(count);
    for _ in 0..count {
        names.push(unescape(lines.next()?));
    }
    Some((stamp.parse().ok()?, names))
}

/// The three bytes that would break the line format written out of the way. A card is the
/// user's and a rom's header text is not ours to assume printable, so neither a file name nor
/// a title is trusted to be free of them. Escaping rather than rejecting keeps a cart on the
/// shelf instead of quietly dropping it from the cache.
fn escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '\t' => out.push_str("\\t"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            _ => out.push(c),
        }
    }
    out
}

fn unescape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars();
    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match chars.next() {
            Some('t') => out.push('\t'),
            Some('n') => out.push('\n'),
            Some('r') => out.push('\r'),
            Some('\\') => out.push('\\'),
            // An escape this code did not write. Kept verbatim rather than dropped, so a
            // hand-edited file loses nothing it can be read back for.
            Some(other) => {
                out.push('\\');
                out.push(other);
            }
            None => out.push('\\'),
        }
    }
    out
}

// ---------------------------------------------------------------------------------------
// Reading the card
// ---------------------------------------------------------------------------------------

/// Names, sorted. Read as names rather than as entries because on the fast path that is all
/// the shelf needs: whether a game was added or removed is a question about the names, and
/// the answer must not cost a stat per file.
/// `recursive` walks `dir`'s subdirectories too, and the returned strings are then **paths
/// relative to `dir`** (`GBA/GBA中文/x.gba`). `Games/` is walked recursively so a card may file its
/// games per machine (or per machine and language) and still be one shelf; `Labels/` is walked
/// flat, so a card that files labels the same way is not found unless the lookup is told about
/// the subdirectory — which is why the label lookup uses `stem_of` (the *file* name's stem) either
/// way. On a flat card a relative path is just the name, so an existing card's cache still agrees.
fn list_names(dir: &Path, keep: fn(&Path) -> bool, recursive: bool) -> Result<Vec<String>, StoreError> {
    let mut names = Vec::new();
    collect_names(dir, Path::new(""), keep, recursive, &mut names)?;
    names.sort();
    Ok(names)
}

fn collect_names(
    dir: &Path,
    rel: &Path,
    keep: fn(&Path) -> bool,
    recursive: bool,
    out: &mut Vec<String>,
) -> Result<(), StoreError> {
    let entries = match std::fs::read_dir(dir) {
        Ok(d) => d,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(e) => return Err(e.into()),
    };
    for entry in entries {
        let path = entry?.path();
        if path.is_dir() {
            // A card's own bookkeeping (System Volume Information, and a Mac's ._ sidecars) is not
            // a machine folder; walking it would only ever turn up junk.
            let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            if recursive && !name.starts_with('.') && name != "System Volume Information" {
                collect_names(&path, &rel.join(name), keep, recursive, out)?;
            }
            continue;
        }
        if !keep(&path) {
            continue;
        }
        // A name that is not UTF-8 cannot be keyed by stem, labelled, or written to the cache.
        // Skipped rather than failing the shelf over it.
        let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        let joined = rel.join(name);
        if let Some(s) = joined.to_str() {
            out.push(s.replace('\\', "/"));
        }
    }
    Ok(())
}

/// The stems of everything in `Labels/`, so a cart can be asked whether it has a face without
/// a stat of its own.
fn labels_set(dir: &Path) -> Result<HashSet<String>, StoreError> {
    Ok(label_keys(&list_names(dir, is_png, true)?))
}

/// The label files as keys: the path under `Labels/` with its `.png` taken off — `<machine>/<stem>`
/// for a card that sorts its art the way it sorts its games, `<stem>` for a flat one. Both
/// spellings live in one set, so a card that mirrors some of its art and not the rest still finds
/// every file it has, and `has_label` stays a set lookup rather than a stat per cart.
fn label_keys(names: &[String]) -> HashSet<String> {
    let mut keys = HashSet::new();
    for n in names {
        if n.len() > 4 && n.to_ascii_lowercase().ends_with(".png") {
            keys.insert(n[..n.len() - 4].to_string());
        }
    }
    keys
}

/// The machine folder a rom name belongs in, from its extension — the same decision
/// `System::from_extension` makes, spelled as the folder a sorted card files it under.
fn machine_of(name: &str) -> Option<&'static str> {
    let ext = Path::new(name).extension().and_then(|e| e.to_str())?;
    System::from_extension(ext).map(System::dir_name)
}

/// Where a cart's art is, if the card carries it: `Labels/<machine>/<stem>.png` on a sorted card,
/// else the flat `Labels/<stem>.png`. `keys` is what `label_keys` built from one directory walk.
fn label_for(
    labels_dir: &Path,
    machine: Option<&str>,
    stem: &str,
    keys: &HashSet<String>,
) -> Option<PathBuf> {
    if let Some(machine) = machine {
        if keys.contains(&format!("{machine}/{stem}")) {
            return Some(labels_dir.join(machine).join(format!("{stem}.png")));
        }
    }
    keys.contains(stem)
        .then(|| labels_dir.join(format!("{stem}.png")))
}

fn stem_of(name: &str) -> Option<String> {
    Path::new(name)
        .file_stem()
        .and_then(|s| s.to_str())
        .map(str::to_string)
}

/// Cart and cache row for every name, in one pass.
///
/// `cached` is the previous run's rows. A name whose size and timestamp are the ones the
/// cache already has keeps the header it came with, which is what makes a card with one new
/// game on it cost one header read rather than the whole shelf.
fn build(
    root: &Path,
    games: &[String],
    labels: &HashSet<String>,
    cached: Option<&[Row]>,
) -> (Vec<Cart>, Vec<Row>) {
    let games_dir = root.join("Games");
    let labels_dir = root.join("Labels");
    let mut carts = Vec::with_capacity(games.len());
    let mut rows = Vec::with_capacity(games.len());

    for name in games {
        let rom = games_dir.join(name);
        let meta = std::fs::metadata(&rom).ok();
        let (size, mtime) = meta.as_ref().map_or((0, 0), |m| (m.len(), mtime_secs(m)));
        let known = cached.and_then(|rows| {
            rows.iter()
                .find(|r| r.name == *name && r.size == size && r.mtime == mtime)
        });
        let (title, code) = match known {
            Some(row) => (row.title.clone(), row.code.clone()),
            None => header(&rom),
        };
        let stem = stem_of(name).unwrap_or_default();
        let label = label_for(&labels_dir, machine_of(name), &stem, labels);
        rows.push(Row {
            name: name.clone(),
            size,
            mtime,
            has_label: label.is_some(),
            title: title.clone(),
            code: code.clone(),
        });
        carts.push(Cart {
            initial: bucket(&stem),
            stem,
            rom,
            label,
            title,
            code,
        });
    }
    order(&mut carts);
    (carts, rows)
}

/// Which letter ring bucket a cart belongs in.
///
/// From the name the shelf *shows*, which is the file name with its tags taken off — not from
/// the header title. The header title is twelve characters of the publisher's language, so a
/// card of Chinese file names filed under it puts POKEMON EMER and ROCKMANZERO on the ring and
/// jumps to a letter that matches nothing anybody can see. The label, the line of type under the
/// row and this all read the same string, so they cannot disagree.
fn bucket(stem: &str) -> char {
    crate::pinyin::initial(&crate::name::clean_label_stripped(stem))
}

/// The carts a cache holds, rebuilt without touching a rom.
fn carts_from_rows(root: &Path, rows: &[Row], labels: &HashSet<String>) -> Vec<Cart> {
    let games_dir = root.join("Games");
    let labels_dir = root.join("Labels");
    let mut carts: Vec<Cart> = rows
        .iter()
        .map(|row| {
            let stem = stem_of(&row.name).unwrap_or_default();
            let label = row
                .has_label
                .then(|| label_for(&labels_dir, machine_of(&row.name), &stem, labels))
                .flatten();
            Cart {
                initial: bucket(&stem),
                stem,
                rom: games_dir.join(&row.name),
                label,
                title: row.title.clone(),
                code: row.code.clone(),
            }
        })
        .collect();
    order(&mut carts);
    carts
}

/// By letter, then by stem, then by file name.
///
/// The letter comes first because the ring above the shelf is an index, and an index is only an
/// index if its entries are in order. File names are hanzi, and hanzi in code point order have
/// nothing to do with the order they are read in — `宝` sorts before `恶` and `洛` comes after
/// both, but the pinyin runs B, E, L — so a shelf sorted by file name has its B carts scattered
/// down the row and stepping the dial to C lands *behind* where the user is standing. Grouped,
/// the row reads A, B, C left to right, down is always forwards and up is always back, and a
/// letter is a run of carts rather than a hop.
///
/// Within a letter it is the file name, which is the order the shelf had before — the grouping
/// is the only thing that changed. That order is a code point one and not a pinyin one, and it
/// is the honest place to stop: sorting `宝可梦` before `白色` needs every character's full
/// pinyin, and the card carries only the first letter of each.
///
/// The file name breaks the tie after the stem so a card holding both `Foo.gba` and `Foo.GBA`
/// comes out the same way on every boot rather than in whatever order the directory handed them
/// over.
///
/// `char` order is the ring's order — `#` is below `A` — so the shelf and the dial agree on
/// which letter comes first without either of them being told.
fn order(carts: &mut [Cart]) {
    carts.sort_by(|a, b| {
        a.initial
            .cmp(&b.initial)
            .then_with(|| a.stem.cmp(&b.stem))
            .then_with(|| a.rom.cmp(&b.rom))
    });
}

fn dir_mtime(dir: &Path) -> i64 {
    std::fs::metadata(dir).map_or(0, |m| mtime_secs(&m))
}

/// Seconds, or 0 for a filesystem that will not say. Zero is a value the stamp can hold and
/// compare like any other: a card whose directory timestamps are unreadable simply never
/// matches, which is a slow boot rather than a wrong shelf.
fn mtime_secs(m: &Metadata) -> i64 {
    m.modified()
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map_or(0, |d| d.as_secs() as i64)
}

/// Whether this file is a cart at all: any of the three extensions, whichever machine's.
///
/// What used to be a plain `.gba` test, widened rather than replaced, and `scan` and
/// `scan_cached` still share one predicate for exactly the reason they shared that one — two
/// predicates would let the cache and the reference disagree about the same card.
fn is_rom(p: &Path) -> bool {
    !is_hidden(p)
        && p.extension()
            .and_then(|e| e.to_str())
            .is_some_and(|e| System::from_extension(e).is_some())
}

fn is_png(p: &Path) -> bool {
    !is_hidden(p)
        && p.extension()
            .and_then(|e| e.to_str())
            .is_some_and(|e| e.eq_ignore_ascii_case("png"))
}

/// A leading dot is card metadata rather than content, and every folder on the card is read
/// through this. macOS writes `._<name>` beside each file it copies onto a FAT volume, which
/// carries the extension of the file it shadows, so the extension alone cannot tell them
/// apart. It also sorts first, which is why the sidecar rather than the file is what a picker
/// walking the folder in order tends to land on.
pub fn is_hidden(p: &Path) -> bool {
    p.file_name()
        .and_then(|n| n.to_str())
        .is_some_and(|n| n.starts_with('.'))
}
