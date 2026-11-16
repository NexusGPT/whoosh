//! The per-account protocol transcript: the IMAP lines the adapter sent and received, kept in a
//! ring buffer the UI can save as the "download connection log". Passwords, tokens and message
//! bodies never enter it: a LOGIN password and a SASL response are replaced when the line is
//! recorded, and every literal (a message header, a body, an APPEND payload) is replaced by its
//! byte count.
use std::{
    collections::VecDeque,
    io,
    pin::Pin,
    sync::{atomic::{AtomicBool, Ordering}, Arc, Mutex},
    task::{Context, Poll},
};

use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Dir { Sent, Received }

#[derive(Default)]
struct Half {
    line: Vec<u8>,
    literal_left: usize,
    literal_total: usize,
    secret_next: bool,
}

struct Inner {
    cap: usize,
    lines: VecDeque<String>,
    sent: Half,
    recv: Half,
}

/// A ring buffer of the most recent lines. `cap` comes from the caller (docs/CONSTANTS.md).
pub struct Transcript {
    inner: Mutex<Inner>,
}

fn push(lines: &mut VecDeque<String>, cap: usize, text: String) {
    if cap == 0 { return; }
    while lines.len() >= cap { lines.pop_front(); }
    lines.push_back(text);
}

/// The first token of an IMAP argument list: a quoted string (with its quotes) or an atom.
fn first_token(s: &str) -> &str {
    if let Some(rest) = s.strip_prefix('"') {
        let mut escaped = false;
        for (i, c) in rest.char_indices() {
            match c {
                '\\' if !escaped => escaped = true,
                '"' if !escaped => return &s[..i + 2],
                _ => escaped = false,
            }
        }
        s
    } else {
        s.split(' ').next().unwrap_or(s)
    }
}

fn literal_size(line: &str) -> Option<usize> {
    let body = line.strip_suffix('}')?;
    let open = body.rfind('{')?;
    body[open + 1..].trim_end_matches('+').parse().ok()
}

fn redact_sent(text: &str, half: &mut Half) -> String {
    if half.secret_next {
        half.secret_next = false;
        return "[credential redacted]".into();
    }
    let mut parts = text.splitn(3, ' ');
    let (tag, cmd, rest) = (parts.next().unwrap_or(""), parts.next().unwrap_or(""), parts.next().unwrap_or(""));
    match cmd.to_ascii_uppercase().as_str() {
        "LOGIN" => format!("{tag} LOGIN {} [password redacted]", first_token(rest)),
        "AUTHENTICATE" => {
            let mut a = rest.splitn(2, ' ');
            let mech = a.next().unwrap_or("");
            if a.next().is_some() {
                format!("{tag} AUTHENTICATE {mech} [credential redacted]")
            } else {
                half.secret_next = true;
                format!("{tag} AUTHENTICATE {mech}")
            }
        }
        _ => text.to_string(),
    }
}

impl Transcript {
    pub fn new(cap: usize) -> Arc<Transcript> {
        Arc::new(Transcript { inner: Mutex::new(Inner { cap, lines: VecDeque::new(), sent: Half::default(), recv: Half::default() }) })
    }

    pub(crate) fn feed(&self, dir: Dir, bytes: &[u8]) {
        let mut g = self.inner.lock().expect("transcript lock");
        let Inner { cap, lines, sent, recv } = &mut *g;
        let (half, prefix) = if dir == Dir::Sent { (sent, "C: ") } else { (recv, "S: ") };
        for &b in bytes {
            if half.literal_left > 0 {
                half.literal_left -= 1;
                if half.literal_left == 0 {
                    push(lines, *cap, format!("{prefix}[{} bytes of message data not logged]", half.literal_total));
                }
                continue;
            }
            if b != b'\n' {
                half.line.push(b);
                continue;
            }
            let raw = std::mem::take(&mut half.line);
            let text = String::from_utf8_lossy(&raw).trim_end_matches('\r').to_string();
            if text.is_empty() { continue; } // the CRLF that follows a literal
            let shown = if dir == Dir::Sent { redact_sent(&text, half) } else { text.clone() };
            push(lines, *cap, format!("{prefix}{shown}"));
            if let Some(n) = literal_size(&text) {
                half.literal_left = n;
                half.literal_total = n;
            }
        }
    }

    /// The kept lines, oldest first. `C:` is what the app sent, `S:` what the server answered.
    pub fn lines(&self) -> Vec<String> {
        self.inner.lock().expect("transcript lock").lines.iter().cloned().collect()
    }

    /// The same lines as one text, for the "download connection log" action.
    pub fn text(&self) -> String {
        let mut out = self.lines().join("\n");
        out.push('\n');
        out
    }
}

/// Wraps a stream and feeds every byte that passes to the transcript.
pub(crate) struct Tapped<T> {
    pub(crate) inner: T,
    pub(crate) tap: Option<Arc<Transcript>>,
    /// imap-compress: set once the connection is compressed. The bytes under the compression are not protocol text, so this
    /// tap stops and a second tap above the compression records the plain lines.
    pub(crate) mute: Arc<AtomicBool>,
}

impl<T> Tapped<T> {
    pub(crate) fn new(inner: T, tap: Option<Arc<Transcript>>) -> Self { Tapped { inner, tap, mute: Arc::new(AtomicBool::new(false)) } }
    fn tap(&self) -> Option<&Arc<Transcript>> { if self.mute.load(Ordering::SeqCst) { None } else { self.tap.as_ref() } }
}

impl<T: AsyncRead + Unpin> AsyncRead for Tapped<T> {
    fn poll_read(self: Pin<&mut Self>, cx: &mut Context<'_>, buf: &mut ReadBuf<'_>) -> Poll<io::Result<()>> {
        let this = self.get_mut();
        let before = buf.filled().len();
        let r = Pin::new(&mut this.inner).poll_read(cx, buf);
        if let (Poll::Ready(Ok(())), Some(t)) = (&r, this.tap()) {
            t.feed(Dir::Received, &buf.filled()[before..]);
        }
        r
    }
}

impl<T: AsyncWrite + Unpin> AsyncWrite for Tapped<T> {
    fn poll_write(self: Pin<&mut Self>, cx: &mut Context<'_>, buf: &[u8]) -> Poll<io::Result<usize>> {
        let this = self.get_mut();
        let r = Pin::new(&mut this.inner).poll_write(cx, buf);
        if let (Poll::Ready(Ok(n)), Some(t)) = (&r, this.tap()) {
            t.feed(Dir::Sent, &buf[..*n]);
        }
        r
    }
    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> { Pin::new(&mut self.get_mut().inner).poll_flush(cx) }
    fn poll_shutdown(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> { Pin::new(&mut self.get_mut().inner).poll_shutdown(cx) }
}
