use std::fs::File;
use std::io::Read;
use std::path::Path;

/// GBA cartridge header: a 12 byte ASCII game title, NUL padded on the right.
const TITLE_OFF: usize = 0xa0;
const TITLE_LEN: usize = 12;

/// The four character game code, immediately after the title. The fourth character is the
/// region, so callers keying on the game itself want the first three.
const CODE_OFF: usize = 0xac;
const CODE_LEN: usize = 4;

/// Everything the shelf wants out of a header, and the point at which the read stops. The
/// game code is the last field in it, so this is exactly as far as anything here reaches.
const HEAD_LEN: u64 = (CODE_OFF + CODE_LEN) as u64;

/// Title and code, in one read.
///
/// Read once rather than twice because the shelf asks for both for every cart on the card,
/// and on the device the card is the slow part: two opens per rom is two directory lookups
/// and two cluster chain walks where one will do, multiplied by however many carts are on
/// it. Only the first `HEAD_LEN` bytes are read, never the whole rom, which can be 32 MB.
///
/// A rom that is missing, truncated, or unreadable is two empty strings rather than an
/// error. Nothing on this path can act on a failure — an unreadable rom has no title, and
/// the shelf still has to show it — so the empty string is the whole of the answer.
pub fn header(rom: &Path) -> (String, String) {
    let Ok(file) = File::open(rom) else {
        return (String::new(), String::new());
    };
    let mut buf = Vec::with_capacity(HEAD_LEN as usize);
    // `take` rather than `read_exact`: a short rom reads the bytes that are there, so a
    // truncated file is an empty field rather than a cart that never reaches the shelf.
    if file.take(HEAD_LEN).read_to_end(&mut buf).is_err() {
        return (String::new(), String::new());
    }
    (
        field_at(&buf, TITLE_OFF, TITLE_LEN).unwrap_or_default(),
        field_at(&buf, CODE_OFF, CODE_LEN).unwrap_or_default(),
    )
}

/// The title alone. Kept for callers that want one field; it costs an open of its own, so
/// anything that wants both should ask `header` instead.
pub fn header_title(rom: &Path) -> Option<String> {
    let (title, _) = header(rom);
    (!title.is_empty()).then_some(title)
}

pub fn header_code(rom: &Path) -> Option<String> {
    let (_, code) = header(rom);
    (!code.is_empty()).then_some(code)
}

fn field_at(buf: &[u8], off: usize, len: usize) -> Option<String> {
    text_from_bytes(buf.get(off..off + len)?)
}

fn text_from_bytes(buf: &[u8]) -> Option<String> {
    let end = buf.iter().position(|b| *b == 0).unwrap_or(buf.len());
    let text = std::str::from_utf8(&buf[..end]).ok()?.trim();
    (!text.is_empty()).then(|| text.to_string())
}

#[cfg(test)]
mod tests {
    use super::{header, text_from_bytes};
    use std::io::Write;

    #[test]
    fn padding_is_trimmed_but_interior_spaces_are_kept() {
        let full = text_from_bytes(b"POKEMON EMER");
        assert_eq!(full.as_deref(), Some("POKEMON EMER"));
        assert_eq!(
            text_from_bytes(b"ADVANCEWARS\0").as_deref(),
            Some("ADVANCEWARS")
        );
        assert_eq!(text_from_bytes(b"KIRBY      \0").as_deref(), Some("KIRBY"));
        assert_eq!(text_from_bytes(&[0u8; 12]), None);
    }

    #[test]
    fn one_read_returns_both_fields() {
        let dir = tempfile::tempdir().unwrap();
        let rom = dir.path().join("Emerald.gba");
        let mut bytes = vec![0u8; 0x100];
        bytes[0xa0..0xac].copy_from_slice(b"POKEMON EMER");
        bytes[0xac..0xb0].copy_from_slice(b"BPEE");
        std::fs::File::create(&rom)
            .unwrap()
            .write_all(&bytes)
            .unwrap();
        assert_eq!(
            header(&rom),
            ("POKEMON EMER".to_string(), "BPEE".to_string())
        );
    }

    #[test]
    fn a_truncated_rom_yields_empty_fields_rather_than_an_error() {
        let dir = tempfile::tempdir().unwrap();
        let rom = dir.path().join("Tiny.gba");
        std::fs::write(&rom, [0u8; 8]).unwrap();
        assert_eq!(header(&rom), (String::new(), String::new()));
    }

    #[test]
    fn a_missing_rom_yields_empty_fields() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(
            header(&dir.path().join("nope.gba")),
            (String::new(), String::new())
        );
    }
}
