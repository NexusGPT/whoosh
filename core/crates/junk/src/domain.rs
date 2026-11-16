//! Small, pure helpers over addresses and hosts. No public-suffix list is shipped: `registrable_domain` is a
//! heuristic (last two labels, last three under a two-letter country code with a generic second level).

/// The first `max` characters of `s` (all of it when it is shorter).
pub(crate) fn head(s: &str, max: usize) -> &str {
    match s.char_indices().nth(max) {
        Some((i, _)) => &s[..i],
        None => s,
    }
}

/// The lower-case domain of an address: "Ann <a@Mail.Example.com>" is not an address, "a@mail.example.com" is.
/// None when there is no "@" or the part after it has no dot.
pub fn domain_of(email: &str) -> Option<String> {
    let at = email.rfind('@')?;
    let d = email[at + 1..].trim().trim_end_matches('>').trim_end_matches('.').to_ascii_lowercase();
    if d.is_empty() || !d.contains('.') {
        None
    } else {
        Some(d)
    }
}

/// The part of a host a person would call "the site": mail.news.example.com gives example.com, shop.example.co.uk gives example.co.uk.
pub fn registrable_domain(host: &str) -> String {
    let h = host.trim().trim_end_matches('.').to_ascii_lowercase();
    let labels: Vec<&str> = h.split('.').filter(|l| !l.is_empty()).collect();
    let n = labels.len();
    if n <= 2 {
        return labels.join(".");
    }
    let tld = labels[n - 1];
    let sld = labels[n - 2];
    let two_level = tld.len() == 2 && matches!(sld, "co" | "com" | "org" | "net" | "gov" | "edu" | "ac" | "or" | "ne" | "go");
    let keep = if two_level { 3 } else { 2 };
    labels[n - keep..].join(".")
}

/// The host of an http(s) URL, lower-case, without port or user info ("http://paypal.com@evil.example/" gives evil.example).
/// None for any other scheme, or a host with no dot.
pub fn host_of_url(url: &str) -> Option<String> {
    let u = url.trim();
    let lower = u.to_ascii_lowercase();
    let rest = if lower.starts_with("https://") {
        &u[8..]
    } else if lower.starts_with("http://") {
        &u[7..]
    } else if u.starts_with("//") {
        &u[2..]
    } else {
        return None;
    };
    let authority = rest.split(|c: char| c == '/' || c == '?' || c == '#').next().unwrap_or("");
    let host_port = authority.rsplit('@').next().unwrap_or("");
    let host = host_port.split(':').next().unwrap_or("").trim().trim_end_matches('.').to_ascii_lowercase();
    if host.is_empty() || !host.contains('.') {
        None
    } else {
        Some(host)
    }
}

/// The host a piece of link text shows to the reader, when the text itself reads as an address
/// ("www.example.com", "https://example.com/offer"); None for words such as "Click here".
pub fn visible_host(text: &str) -> Option<String> {
    let t = text.trim();
    if t.is_empty() || t.len() > 253 || t.chars().any(char::is_whitespace) {
        return None;
    }
    let lower = t.to_ascii_lowercase();
    let rest = lower.strip_prefix("https://").or_else(|| lower.strip_prefix("http://")).unwrap_or(lower.as_str());
    let host = rest.split(|c: char| c == '/' || c == '?' || c == '#' || c == ':').next().unwrap_or("");
    let labels: Vec<&str> = host.split('.').collect();
    if labels.len() < 2 {
        return None;
    }
    if !labels.iter().all(|l| !l.is_empty() && l.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')) {
        return None;
    }
    let tld = labels[labels.len() - 1];
    if tld.len() < 2 || !tld.chars().all(|c| c.is_ascii_alphabetic()) {
        return None;
    }
    Some(host.to_string())
}
