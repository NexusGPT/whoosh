//! Links read out of an HTML body: the address each `<a href>` goes to and the text the reader sees on it.
use crate::domain::head;
use crate::tuning::HTML_CHARS_MAX;

/// One anchor of an HTML body.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Link {
    /// The visible text, tags removed, whitespace collapsed.
    pub text: String,
    /// The href, as written (an `&amp;` read as `&`).
    pub href: String,
}

/// At most `max` anchors of the first `HTML_CHARS_MAX` characters of `html`, in document order.
pub fn extract_links(html: &str, max: usize) -> Vec<Link> {
    let html = head(html, HTML_CHARS_MAX);
    let lower = html.to_ascii_lowercase(); // same byte offsets as `html`
    let bytes = lower.as_bytes();
    let mut out: Vec<Link> = Vec::new();
    let mut pos = 0usize;
    while out.len() < max {
        let start = match lower[pos..].find("<a") {
            Some(rel) => pos + rel,
            None => break,
        };
        let after = start + 2;
        // "<a" must be followed by whitespace: this skips <abbr>, <address>, <article>
        let spaced = matches!(bytes.get(after), Some(&b' ') | Some(&b'\t') | Some(&b'\n') | Some(&b'\r'));
        if !spaced {
            pos = after;
            continue;
        }
        let tag_end = match lower[after..].find('>') {
            Some(rel) => after + rel,
            None => break,
        };
        let href = attr_value(&html[after..tag_end], &lower[after..tag_end], "href");
        let close = match lower[tag_end..].find("</a") {
            Some(rel) => tag_end + rel,
            None => lower.len(),
        };
        let inner = &html[tag_end + 1..close];
        if let Some(h) = href {
            out.push(Link { text: visible_text(inner), href: h.replace("&amp;", "&") });
        }
        pos = close.max(after);
    }
    out
}

/// The value of attribute `name` in a tag's attribute text (`tag`; `tag_lower` is the same text in lower case).
fn attr_value(tag: &str, tag_lower: &str, name: &str) -> Option<String> {
    let mut from = 0usize;
    while let Some(rel) = tag_lower[from..].find(name) {
        let at = from + rel;
        let prev_ok = at == 0 || tag_lower.as_bytes()[at - 1].is_ascii_whitespace();
        let rest = tag[at + name.len()..].trim_start();
        if prev_ok && rest.starts_with('=') {
            let v = rest[1..].trim_start();
            let quote = v.chars().next().filter(|c| *c == '"' || *c == '\'');
            let value = match quote {
                Some(q) => v[1..].split(q).next().unwrap_or("").to_string(),
                None => v.split(|c: char| c.is_whitespace() || c == '>').next().unwrap_or("").to_string(),
            };
            return Some(value);
        }
        from = at + name.len();
    }
    None
}

/// Text of an anchor's inside: tags dropped, entities for ampersand and no-break space read, whitespace collapsed.
fn visible_text(inner: &str) -> String {
    let mut out = String::new();
    let mut in_tag = false;
    for c in inner.chars() {
        match c {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => out.push(c),
            _ => {}
        }
    }
    let out = out.replace("&amp;", "&").replace("&nbsp;", " ");
    out.split_whitespace().collect::<Vec<&str>>().join(" ")
}
