//! body-r4 stage 2: the canonical text of a message (line ends to LF, a leading BOM and every NUL removed, NFC), the quote depth of a plain
//! line, format=flowed unfolding, and the charset chain (the declared charset, then a tolerant decoder, then windows-1252, which cannot fail).
//! Hard-wrapped lines are never joined: only format=flowed's soft breaks (an encoding, RFC 3676) are.
use mail_parser::decoders::charsets::Charset;

/// A body declared UTF-8 whose decoding holds more than this share of U+FFFD is decoded again as windows-1252 (ledger row
/// `PARSE_UTF8_REPLACEMENT_MAX_SHARE`, source: the round-4 brief).
pub const UTF8_REPLACEMENT_MAX_SHARE: f64 = 0.02;

/// What the charset chain had to do (each one is a warning on the message).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CharsetDowngrade {
    /// The declared charset is not one we know: read as UTF-8, invalid bytes replaced.
    UnknownCharsetReadTolerantly,
    /// Declared UTF-8, but more than 2 % of the text came out as replacement characters: read as windows-1252.
    Utf8ReadAsWindows1252,
}

/// Bytes to text by the charset chain. `declared` is the part's `charset` parameter.
pub fn decode_text(bytes: &[u8], declared: Option<&str>) -> (String, Vec<CharsetDowngrade>) {
    let mut warnings = Vec::new();
    let label = declared.map(|d| d.trim().trim_matches('"').to_ascii_lowercase()).filter(|d| !d.is_empty());
    let is_utf8 = label.as_deref().is_none_or(|l| l == "utf-8" || l == "utf8" || l == "us-ascii" || l == "ascii");
    if !is_utf8 {
        if let Some(cs) = Charset::from_label(label.as_deref().unwrap_or("").as_bytes()) {
            return (cs.decode(bytes).into_owned(), warnings);
        }
        warnings.push(CharsetDowngrade::UnknownCharsetReadTolerantly);
    }
    let text = String::from_utf8_lossy(bytes).into_owned();
    let total = text.chars().count().max(1);
    let bad = text.chars().filter(|c| *c == '\u{FFFD}').count();
    if bad > 0 && (bad as f64) / (total as f64) > UTF8_REPLACEMENT_MAX_SHARE {
        warnings.push(CharsetDowngrade::Utf8ReadAsWindows1252);
        return (windows_1252(bytes), warnings);
    }
    (text, warnings)
}

/// windows-1252, every byte mapped (the five bytes it leaves undefined become the C1 control of the same number), so it cannot fail.
pub fn windows_1252(bytes: &[u8]) -> String {
    const HIGH: [u32; 32] = [
        0x20AC, 0x81, 0x201A, 0x0192, 0x201E, 0x2026, 0x2020, 0x2021, 0x02C6, 0x2030, 0x0160, 0x2039, 0x0152, 0x8D, 0x017D, 0x8F,
        0x90, 0x2018, 0x2019, 0x201C, 0x201D, 0x2022, 0x2013, 0x2014, 0x02DC, 0x2122, 0x0161, 0x203A, 0x0153, 0x9D, 0x017E, 0x0178,
    ];
    bytes.iter().map(|b| match *b { 0x80..=0x9F => char::from_u32(HIGH[(*b - 0x80) as usize]).unwrap_or('\u{FFFD}'), b => b as char }).collect()
}

/// RFC 3676: a line that ends in a space continues on the next (the space is kept unless `delsp`); a leading space is space-stuffing; the
/// signature line "-- " is never joined; quoted lines join only lines of the same depth.
pub fn unflow(text: &str, delsp: bool) -> String {
    let mut out = String::with_capacity(text.len());
    let mut pending: Option<(usize, String)> = None;
    for raw in text.strip_suffix('\n').unwrap_or(text).split('\n') {
        let line = raw.strip_suffix('\r').unwrap_or(raw);
        let depth = line.chars().take_while(|c| *c == '>').count();
        let mut body = &line[depth..];
        if body.starts_with(' ') { body = &body[1..]; }
        let soft = body.ends_with(' ') && body != "-- ";
        let piece = if soft && delsp { &body[..body.len() - 1] } else { body };
        match pending.take() {
            Some((d, mut acc)) if d == depth => { acc.push_str(piece); if soft { pending = Some((d, acc)); } else { flush(&mut out, d, &acc); } }
            Some((d, acc)) => { flush(&mut out, d, &acc); if soft { pending = Some((depth, piece.to_string())); } else { flush(&mut out, depth, piece); } }
            None => { if soft { pending = Some((depth, piece.to_string())); } else { flush(&mut out, depth, piece); } }
        }
    }
    if let Some((d, acc)) = pending { flush(&mut out, d, &acc); }
    if out.ends_with('\n') && !text.ends_with('\n') { out.pop(); }
    out
}

fn flush(out: &mut String, depth: usize, s: &str) {
    for _ in 0..depth { out.push('>'); }
    if depth > 0 { out.push(' '); }
    out.push_str(s);
    out.push('\n');
}

/// The canonical form every segment's range points into: CRLF and CR to LF, a leading BOM removed, NUL removed, NFC.
pub fn canonical(text: &str) -> String {
    let t = text.strip_prefix('\u{FEFF}').unwrap_or(text);
    let t = t.replace("\r\n", "\n").replace('\r', "\n").replace('\0', "");
    crate::nfc::nfc(&t)
}

/// The quote depth of one plain line and the byte offset where its content starts: ">" marks only, the first after up to three spaces of
/// indent, each optionally followed by one space. "|" and ":" never count (tables, code, diffs).
pub fn quote_depth(line: &str) -> (usize, usize) {
    let b = line.as_bytes();
    let mut i = 0;
    while i < b.len() && i < 3 && b[i] == b' ' { i += 1; }
    if i >= b.len() || b[i] != b'>' { return (0, 0); }
    let mut depth = 0;
    while i < b.len() && b[i] == b'>' {
        depth += 1;
        i += 1;
        if i < b.len() && b[i] == b' ' && b.get(i + 1) == Some(&b'>') { i += 1; }
    }
    if i < b.len() && b[i] == b' ' { i += 1; }
    (depth, i)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn depth_counts_only_angle_marks() {
        assert_eq!(quote_depth("> a"), (1, 2));
        assert_eq!(quote_depth("   >> b"), (2, 6));
        assert_eq!(quote_depth("> > c"), (2, 4));
        assert_eq!(quote_depth("    > code"), (0, 0));
        assert_eq!(quote_depth("| cell"), (0, 0));
        assert_eq!(quote_depth(": diff"), (0, 0));
    }
    #[test]
    fn flowed_lines_join_and_hard_lines_do_not() {
        assert_eq!(unflow("one \ntwo\nthree\n", false), "one two\nthree\n");
        assert_eq!(unflow("> a \n> b\nc", false), "> a b\nc");
        assert_eq!(unflow("-- \nAlice", false), "-- \nAlice");
    }
    #[test]
    fn windows_1252_maps_every_byte() {
        assert_eq!(windows_1252(&[0x80, 0xE9, 0x41]), "\u{20AC}\u{E9}A");
    }
}
