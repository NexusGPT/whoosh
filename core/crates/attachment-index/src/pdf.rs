//! A small PDF text reader for the common case: content streams (plain or Flate) that show text with simple fonts.
//! Anything it cannot read with confidence (encrypted file, CID fonts whose glyph ids are not characters, scans) yields None,
//! and the caller asks the Swift side's PDFKit through `TextReader`. No PDF crate is in the offline registry; flate2 is.
use flate2::read::ZlibDecoder;
use std::io::Read;

/// The text of a PDF in reading order of its content streams, or None when it holds none the reader can trust.
pub fn extract_pdf_text(bytes: &[u8]) -> Option<String> {
    if !bytes.starts_with(b"%PDF") || find(bytes, b"/Encrypt", 0).is_some() {
        return None;
    }
    let mut pages: Vec<String> = Vec::new();
    let mut at = 0;
    while let Some(s) = find(bytes, b"stream", at) {
        if bytes[..s].ends_with(b"end") {
            at = s + 6;
            continue;
        }
        let mut data = s + 6;
        if bytes.get(data) == Some(&b'\r') {
            data += 1;
        }
        if bytes.get(data) == Some(&b'\n') {
            data += 1;
        }
        let Some(end) = find(bytes, b"endstream", data) else { break };
        let dict_from = rfind(&bytes[..s], b"obj").unwrap_or(0);
        let dict = &bytes[dict_from..s];
        let raw = &bytes[data..end];
        let content = if find(dict, b"FlateDecode", 0).is_some() { inflate(raw) } else { Some(raw.to_vec()) };
        if let Some(c) = content.filter(|c| find(c, b"BT", 0).is_some() && find(c, b"ET", 0).is_some()) {
            let t = show_text(&c);
            if !t.trim().is_empty() {
                pages.push(t);
            }
        }
        at = end + 9;
    }
    let text = pages.join("\n").trim().to_string();
    if looks_like_words(&text) { Some(text) } else { None }
}

fn find(h: &[u8], n: &[u8], from: usize) -> Option<usize> {
    if from >= h.len() || n.is_empty() {
        return None;
    }
    h[from..].windows(n.len()).position(|w| w == n).map(|p| p + from)
}

fn rfind(h: &[u8], n: &[u8]) -> Option<usize> {
    h.windows(n.len()).rposition(|w| w == n)
}

fn inflate(raw: &[u8]) -> Option<Vec<u8>> {
    let mut out = Vec::new();
    ZlibDecoder::new(raw).read_to_end(&mut out).ok()?;
    Some(out)
}

/// Enough letters and spaces to be language, not glyph ids.
fn looks_like_words(t: &str) -> bool {
    let total = t.chars().count();
    if total < 3 {
        return false;
    }
    let ok = t.chars().filter(|c| c.is_alphanumeric() || c.is_whitespace() || ".,;:!?'\"-()%€$/&".contains(*c)).count();
    ok * 100 >= total * 90 && t.chars().any(|c| c.is_alphabetic())
}

/// Walks one content stream: strings before Tj ' " and TJ are text; line moves and ET break the line.
fn show_text(c: &[u8]) -> String {
    let (mut out, mut i) = (String::new(), 0);
    let mut pending: Vec<String> = Vec::new();
    let mut in_array = false;
    while i < c.len() {
        match c[i] {
            b'(' => {
                let (s, next) = literal(c, i);
                pending.push(s);
                i = next;
            }
            b'<' if c.get(i + 1) != Some(&b'<') => {
                let close = c[i..].iter().position(|&b| b == b'>').map_or(c.len(), |p| i + p);
                pending.push(hex(&c[i + 1..close]));
                i = close + 1;
            }
            b'[' => {
                in_array = true;
                i += 1;
            }
            b']' => {
                in_array = false;
                i += 1;
            }
            b'-' | b'0'..=b'9' | b'.' if in_array => {
                let st = i;
                while i < c.len() && (c[i] == b'-' || c[i] == b'.' || c[i].is_ascii_digit()) {
                    i += 1;
                }
                let n: f64 = std::str::from_utf8(&c[st..i]).ok().and_then(|s| s.parse().ok()).unwrap_or(0.0);
                if n < -200.0 {
                    pending.push(" ".to_string());
                }
            }
            b if b.is_ascii_alphabetic() || b == b'\'' || b == b'"' || b == b'*' => {
                let st = i;
                while i < c.len() && (c[i].is_ascii_alphabetic() || c[i] == b'\'' || c[i] == b'"' || c[i] == b'*') {
                    i += 1;
                }
                match &c[st..i] {
                    b"Tj" | b"TJ" => out.push_str(&pending.concat()),
                    b"'" | b"\"" => {
                        out.push('\n');
                        out.push_str(&pending.concat());
                    }
                    b"T*" | b"Td" | b"TD" | b"ET" => out.push('\n'),
                    _ => {}
                }
                pending.clear();
            }
            _ => i += 1,
        }
    }
    out.lines().map(|l| l.split_whitespace().collect::<Vec<_>>().join(" ")).filter(|l| !l.is_empty()).collect::<Vec<_>>().join("\n")
}

fn literal(c: &[u8], open: usize) -> (String, usize) {
    let (mut bytes, mut depth, mut i) = (Vec::new(), 1, open + 1);
    while i < c.len() {
        match c[i] {
            b'\\' if i + 1 < c.len() => {
                i += 1;
                match c[i] {
                    b'n' => bytes.push(b'\n'),
                    b'r' => bytes.push(b'\r'),
                    b't' => bytes.push(b'\t'),
                    d @ b'0'..=b'7' => {
                        let mut v = (d - b'0') as u32;
                        for _ in 0..2 {
                            if let Some(n @ b'0'..=b'7') = c.get(i + 1) {
                                v = v * 8 + (n - b'0') as u32;
                                i += 1;
                            }
                        }
                        bytes.push(v as u8);
                    }
                    other => bytes.push(other),
                }
            }
            b'(' => {
                depth += 1;
                bytes.push(b'(');
            }
            b')' => {
                depth -= 1;
                if depth == 0 {
                    return (decode(&bytes), i + 1);
                }
                bytes.push(b')');
            }
            b => bytes.push(b),
        }
        i += 1;
    }
    (decode(&bytes), c.len())
}

fn hex(h: &[u8]) -> String {
    let digits: Vec<u8> = h.iter().filter(|b| b.is_ascii_hexdigit()).map(|b| (*b as char).to_digit(16).unwrap_or(0) as u8).collect();
    decode(&digits.chunks(2).map(|p| p[0] << 4 | p.get(1).copied().unwrap_or(0)).collect::<Vec<u8>>())
}

/// UTF-16BE with a byte-order mark, else one byte one character (WinAnsi is Latin-1 for letters).
fn decode(b: &[u8]) -> String {
    if b.starts_with(&[0xFE, 0xFF]) {
        let units: Vec<u16> = b[2..].chunks(2).map(|p| (p[0] as u16) << 8 | p.get(1).copied().unwrap_or(0) as u16).collect();
        return String::from_utf16_lossy(&units);
    }
    b.iter().map(|&x| x as char).collect()
}
