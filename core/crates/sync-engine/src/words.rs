//! The codes of every sync status the core publishes, and how a code and its parameters travel beside the English words.
//!
//! The contract has no field for a status code. A failure has `ActionFailed.code`; a running or paused status has only
//! `SyncState.provider_status_words`. So the code rides as text, and the Swift side maps each code to ONE catalogue sentence
//! in the app's language. The English sentence stays whole as the engine's own words: the "details" line under the sentence.
//!
//! Wire shape of a code: `sync.paused?retry_s=12&server_words=the%20server%20closed`. The name is lower-case dotted; each
//! parameter is `key=value` with the value percent-encoded (everything but A-Z a-z 0-9 - . _ ~), so a server's words or a folder
//! name with spaces, colons, `&` or non-ASCII letters crosses unchanged and is never translated.
//! In `provider_status_words` the code follows the words after one unit separator (U+001F): `<English words>\u{1f}<code>`.
//! In `ActionFailed.code` it is the whole value, the words stay in `reason_words`.

/// The account is opening its connection and listing its folders.
pub const CONNECTING: &str = "sync.connecting";
/// A page of a folder was stored: `folder`, and `count` (messages stored so far for the account).
pub const FETCHING: &str = "sync.fetching";
/// The history is stored: `count` messages.
pub const MESSAGES: &str = "sync.messages";
/// The sync waits to try again: `retry_s` seconds, and the `server_words` that stopped it when it has them.
pub const PAUSED: &str = "sync.paused";
/// The sync stopped: the `step` ("list", "fetch", "save", "sync"), the `folder` when the step has one, the `server_words`.
pub const FAILED: &str = "sync.failed";
/// The provider asked the app to slow down: its brand, or the mail server's domain, as `provider`.
pub const SLOWED: &str = "sync.slowed";
/// New mail is no longer pushed to this Mac (the connection cannot be kept): the `server_words`.
pub const LOST: &str = "sync.lost";

/// Separates the English words from the code inside `provider_status_words`.
pub const SEP: char = '\u{1f}';

/// A decoded code: its name and its parameters in the order written.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Coded {
    pub name: String,
    pub params: Vec<(String, String)>,
}

impl Coded {
    pub fn param(&self, key: &str) -> Option<&str> {
        self.params.iter().find(|(k, _)| k == key).map(|(_, v)| v.as_str())
    }
}

fn encode_value(v: &str) -> String {
    let mut out = String::with_capacity(v.len());
    for b in v.bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'.' | b'_' | b'~') {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

fn decode_value(v: &str) -> Option<String> {
    let bytes = v.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' {
            let hex = v.get(i + 1..i + 3)?;
            out.push(u8::from_str_radix(hex, 16).ok()?);
            i += 3;
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    String::from_utf8(out).ok()
}

/// `name?k=v&k=v` (just `name` with no parameters).
pub fn code(name: &str, params: &[(&str, &str)]) -> String {
    if params.is_empty() {
        return name.to_string();
    }
    let list: Vec<String> = params.iter().map(|(k, v)| format!("{k}={}", encode_value(v))).collect();
    format!("{name}?{}", list.join("&"))
}

/// A code read back; None when `text` is not a code (no dotted lower-case name, or a broken escape).
pub fn parse(text: &str) -> Option<Coded> {
    let (name, rest) = match text.split_once('?') {
        Some((n, r)) => (n, Some(r)),
        None => (text, None),
    };
    let dotted = name.contains('.') && name.bytes().all(|b| b.is_ascii_lowercase() || b == b'.' || b == b'_');
    if !dotted {
        return None;
    }
    let mut params = Vec::new();
    for pair in rest.into_iter().flat_map(|r| r.split('&')).filter(|p| !p.is_empty()) {
        let (k, v) = pair.split_once('=')?;
        params.push((k.to_string(), decode_value(v)?));
    }
    Some(Coded { name: name.to_string(), params })
}

/// `provider_status_words` for a status: the engine's English sentence, then the code.
pub fn status(words: &str, name: &str, params: &[(&str, &str)]) -> String {
    format!("{words}{SEP}{}", code(name, params))
}

/// A status read back: its English words and its code, when it carries one.
pub fn split_status(text: &str) -> (&str, Option<Coded>) {
    match text.split_once(SEP) {
        Some((words, tail)) => (words, parse(tail)),
        None => (text, None),
    }
}

/// The status of the account's connection while it waits `retry_s` seconds to try again.
pub fn paused_status(words: &str, retry_s: u64, server_words: &str) -> String {
    status(words, PAUSED, &[("retry_s", &retry_s.to_string()), ("server_words", server_words)])
}

/// `ActionFailed.code` of a sync that stopped at `step` (`folder` empty when the step has none).
pub fn failed_code(step: &str, folder: &str, server_words: &str) -> String {
    code(FAILED, &[("step", step), ("folder", folder), ("server_words", server_words)])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_code_and_its_odd_parameters_come_back_unchanged() {
        let c = failed_code("fetch", "[Gmail]/Sent Mail", "BAD: Argument & more=1 é");
        let p = parse(&c).unwrap();
        assert_eq!(p.name, FAILED);
        assert_eq!(p.param("step"), Some("fetch"));
        assert_eq!(p.param("folder"), Some("[Gmail]/Sent Mail"));
        assert_eq!(p.param("server_words"), Some("BAD: Argument & more=1 é"));
    }

    #[test]
    fn a_status_splits_into_its_english_words_and_its_code() {
        let s = paused_status("New mail is paused: x. Reconnecting in 3 s.", 3, "x");
        let (words, c) = split_status(&s);
        assert_eq!(words, "New mail is paused: x. Reconnecting in 3 s.");
        assert_eq!(c.unwrap().param("retry_s"), Some("3"));
        assert_eq!(split_status("plain words").1, None);
        assert_eq!(parse("451 4.7.1"), None);
    }
}
