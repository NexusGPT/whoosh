//! body-r2: HTML character references in message text (`&nbsp;`, `&amp;`, `&eacute;`, `&#233;`, `&#x2019;`). One decoder, used by
//! every path that makes text from a message (HTML made text, a text/plain part a sender wrote HTML into). The named set is the
//! HTML living standard's own table, the one html5ever (under ammonia, already a dependency) reads: nothing is typed by hand here.
//!
//! Rules, each with a test:
//! - one pass over the input: `&amp;nbsp;` is the text `&nbsp;`, never a space (a decoded `&` is not read again);
//! - only a complete reference is decoded (`&name;` with its semicolon, `&#N;`, `&#xH;`): `?a=1&copy=2` and "AT&T" stay as written;
//! - a numeric reference in the Windows-1252 block (`&#150;`) is the character a browser shows, an impossible one is U+FFFD.
use markup5ever::data::NAMED_ENTITIES;

/// The characters of the Windows-1252 block U+0080..=U+009F as a browser reads a numeric reference to them.
const CP1252: [char; 32] = [
    '\u{20AC}', '\u{81}', '\u{201A}', '\u{0192}', '\u{201E}', '\u{2026}', '\u{2020}', '\u{2021}', '\u{02C6}', '\u{2030}', '\u{0160}', '\u{2039}', '\u{0152}', '\u{8D}', '\u{017D}', '\u{8F}',
    '\u{90}', '\u{2018}', '\u{2019}', '\u{201C}', '\u{201D}', '\u{2022}', '\u{2013}', '\u{2014}', '\u{02DC}', '\u{2122}', '\u{0161}', '\u{203A}', '\u{0153}', '\u{9D}', '\u{017E}', '\u{0178}',
];

/// The longest reference name or number taken (the longest HTML name is 31 letters).
const NAME_MAX: usize = 32;

fn numeric(code: u32) -> char {
    match code {
        0 => '\u{FFFD}',
        0x80..=0x9F => CP1252[(code - 0x80) as usize],
        c => char::from_u32(c).unwrap_or('\u{FFFD}'),
    }
}

/// The reference that starts right after an `&`: the text it stands for and how many bytes of `rest` it used (its `;` included).
fn reference(rest: &str) -> Option<(String, usize)> {
    let b = rest.as_bytes();
    if b.first() == Some(&b'#') {
        let (hex, from) = if matches!(b.get(1), Some(b'x' | b'X')) { (true, 2) } else { (false, 1) };
        let digits = b[from..].iter().take(NAME_MAX).take_while(|c| if hex { c.is_ascii_hexdigit() } else { c.is_ascii_digit() }).count();
        if digits == 0 || b.get(from + digits) != Some(&b';') { return None; }
        // a number past u32 is no character: the browser's replacement character
        let code = u32::from_str_radix(&rest[from..from + digits], if hex { 16 } else { 10 }).unwrap_or(0x11_0000);
        return Some((numeric(code).to_string(), from + digits + 1));
    }
    let letters = b.iter().take(NAME_MAX).take_while(|c| c.is_ascii_alphanumeric()).count();
    if letters == 0 || b.get(letters) != Some(&b';') { return None; }
    let &(first, second) = NAMED_ENTITIES.get(&rest[..=letters])?;
    if first == 0 { return None; }
    let mut out = String::new();
    out.extend(char::from_u32(first));
    if second != 0 { out.extend(char::from_u32(second)); }
    Some((out, letters + 1))
}

/// `s` with every complete HTML character reference replaced by its character, in one pass.
pub fn decode_entities(s: &str) -> String {
    if !s.contains('&') { return s.to_string(); }
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(at) = rest.find('&') {
        out.push_str(&rest[..at]);
        let after = &rest[at + 1..];
        match reference(after) {
            Some((text, used)) => { out.push_str(&text); rest = &after[used..]; }
            None => { out.push('&'); rest = after; }
        }
    }
    out.push_str(rest);
    out
}

/// The text of a text/plain part: its references decoded, a non-breaking space a normal space. A part with no reference is returned
/// as it came (a literal U+00A0 in a plain text stays what the sender typed).
pub fn plain_text(s: &str) -> String {
    let decoded = decode_entities(s);
    if decoded == s { decoded } else { decoded.replace('\u{a0}', " ") }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_five_entities_of_the_owners_mail() {
        assert_eq!(decode_entities("Bonjour&nbsp;Assem"), "Bonjour\u{a0}Assem");
        assert_eq!(decode_entities("a &amp; b &lt;c&gt; &quot;d&quot; it&#39;s"), "a & b <c> \"d\" it's");
        assert_eq!(decode_entities("caf&eacute; &#233; &#xE9; l&#x2019;heure &hellip; &euro;"), "caf\u{e9} \u{e9} \u{e9} l\u{2019}heure \u{2026} \u{20ac}");
    }

    #[test]
    fn a_decoded_ampersand_is_never_read_again() {
        assert_eq!(decode_entities("&amp;nbsp;"), "&nbsp;");
        assert_eq!(decode_entities("&amp;amp;"), "&amp;");
        assert_eq!(decode_entities("&amp;#233;"), "&#233;");
    }

    #[test]
    fn an_incomplete_or_unknown_reference_stays_as_written() {
        for s in ["AT&T", "?a=1&copy=2", "&nbsp", "&;", "&#;", "&#xZZ;", "&nosuchentity;", "tom & jerry", "&"] {
            assert_eq!(decode_entities(s), s);
        }
    }

    #[test]
    fn numeric_references_follow_the_browsers_table() {
        assert_eq!(decode_entities("a&#150;b&#8211;c"), "a\u{2013}b\u{2013}c", "Windows-1252 en dash, written two ways");
        assert_eq!(decode_entities("&#0;&#xD800;&#99999999;"), "\u{FFFD}\u{FFFD}\u{FFFD}");
    }

    #[test]
    fn a_plain_text_part_loses_its_non_breaking_spaces_only_when_it_held_references() {
        assert_eq!(plain_text("Bonjour&nbsp;Assem,&nbsp;&nbsp;ceci"), "Bonjour Assem,  ceci");
        assert_eq!(plain_text("one\u{a0}two"), "one\u{a0}two");
    }
}
