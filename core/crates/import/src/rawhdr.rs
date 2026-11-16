//! A header read straight from message bytes, for the fields `parse` does not return (Delivered-To, Status, ...).

/// The values of every header called `name` (case-insensitive) in the header block, folded lines joined, trimmed.
pub fn header_values(bytes: &[u8], name: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut current: Option<String> = None;
    for line in bytes.split(|b| *b == b'\n') {
        let line = line.strip_suffix(b"\r").unwrap_or(line);
        if line.is_empty() {
            break;
        }
        if line[0] == b' ' || line[0] == b'\t' {
            if let Some(c) = current.as_mut() {
                c.push(' ');
                c.push_str(String::from_utf8_lossy(line).trim());
            }
            continue;
        }
        if let Some(done) = current.take() {
            out.push(done);
        }
        if let Some(colon) = line.iter().position(|b| *b == b':') {
            if line[..colon].eq_ignore_ascii_case(name.as_bytes()) {
                current = Some(String::from_utf8_lossy(&line[colon + 1..]).trim().to_string());
            }
        }
    }
    if let Some(done) = current.take() {
        out.push(done);
    }
    out
}

pub fn first_header(bytes: &[u8], name: &str) -> Option<String> {
    header_values(bytes, name).into_iter().next()
}

/// The address inside "Name <a@b.c>", or the bare "a@b.c", lowercased. None when no address is there.
pub fn address_of(value: &str) -> Option<String> {
    let v = value.trim();
    let inner = match (v.rfind('<'), v.rfind('>')) {
        (Some(a), Some(b)) if a < b => &v[a + 1..b],
        _ => v,
    };
    let inner = inner.trim().trim_matches('"');
    inner.contains('@').then(|| inner.to_ascii_lowercase())
}
