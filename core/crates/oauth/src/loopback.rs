//! The loopback redirect (RFC 8252 7.3): a listener on the loopback address and an ephemeral port, for one answer.
//! The redirect names the host `localhost`, never `127.0.0.1`: Microsoft matches `http://localhost` on any port
//! and treats `http://127.0.0.1:<port>` as an exact address (AADSTS50011 on the owner's Mac, 2026-11-09 19:43, the
//! registration holding `http://localhost/`); Google accepts both. The browser may resolve localhost to ::1 first,
//! so the same port is also bound on ::1 when the system allows it.
use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::time::{Duration, Instant};

use crate::url::percent_encode;

pub struct Loopback { listeners: Vec<TcpListener>, pub redirect_uri: String }

/// What the browser brought back: the code, or the provider's refusal in words.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LoopbackAnswer { Code(String), Refused(String) }

fn decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        match b[i] {
            b'+' => { out.push(b' '); i += 1 }
            b'%' if i + 2 < b.len() => match std::str::from_utf8(&b[i + 1..i + 3]).ok().and_then(|h| u8::from_str_radix(h, 16).ok()) {
                Some(v) => { out.push(v); i += 3 }
                None => { out.push(b'%'); i += 1 }
            },
            c => { out.push(c); i += 1 }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn query(target: &str) -> Vec<(String, String)> {
    target.split_once('?').map(|(_, q)| q).unwrap_or("").split('&').filter(|p| !p.is_empty())
        .map(|p| { let (k, v) = p.split_once('=').unwrap_or((p, "")); (decode(k), decode(v)) }).collect()
}

fn reply(mut s: &TcpStream, words: &str) {
    let body = format!("<!doctype html><meta charset=utf-8><title>Whoosh</title><p>{words}</p>");
    let _ = write!(s, "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len());
}

impl Loopback {
    /// Binds 127.0.0.1 on a port the system picks, and ::1 on the same port when it is free. The redirect is
    /// `http://localhost:<port>/`.
    pub fn bind() -> std::io::Result<Loopback> {
        let v4 = TcpListener::bind("127.0.0.1:0")?;
        let port = v4.local_addr()?.port();
        v4.set_nonblocking(true)?;
        let mut listeners = vec![v4];
        if let Ok(v6) = TcpListener::bind(("::1", port)) {
            if v6.set_nonblocking(true).is_ok() { listeners.push(v6); }
        }
        Ok(Loopback { listeners, redirect_uri: format!("http://localhost:{port}/") })
    }

    /// One accepted connection from any of the bound listeners, or WouldBlock when none has one.
    fn accept_any(&self) -> std::io::Result<TcpStream> {
        let mut last = std::io::Error::from(std::io::ErrorKind::WouldBlock);
        for l in &self.listeners {
            match l.accept() {
                Ok((s, _)) => return Ok(s),
                Err(e) => last = e,
            }
        }
        Err(last)
    }

    /// Waits for the one request that carries `state`, at most `timeout`. A request without a code or an error (a
    /// favicon) is answered and ignored; a code with another state is refused (it is not this sign-in's answer).
    pub fn wait(&self, state: &str, timeout: Duration) -> LoopbackAnswer {
        let until = Instant::now() + timeout;
        while Instant::now() < until {
            match self.accept_any() {
                Ok(stream) => {
                    let _ = stream.set_nonblocking(false);
                    let _ = stream.set_read_timeout(Some(Duration::from_secs(5)));
                    let mut line = String::new();
                    if BufReader::new(&stream).read_line(&mut line).is_err() { continue; }
                    let target = line.split_whitespace().nth(1).unwrap_or("");
                    let q = query(target);
                    let get = |k: &str| q.iter().find(|(n, _)| n == k).map(|(_, v)| v.clone());
                    if let Some(err) = get("error") {
                        let words = get("error_description").unwrap_or(err);
                        reply(&stream, "The sign-in did not finish. You can close this tab and go back to Whoosh.");
                        return LoopbackAnswer::Refused(format!("the provider refused the sign-in: {words}"));
                    }
                    let Some(code) = get("code") else { reply(&stream, ""); continue };
                    if get("state").as_deref() != Some(state) {
                        reply(&stream, "This answer belongs to another sign-in. You can close this tab.");
                        return LoopbackAnswer::Refused("the answer did not carry this sign-in's state, so it was not used".into());
                    }
                    reply(&stream, "You are signed in. You can close this tab and go back to Whoosh.");
                    return LoopbackAnswer::Code(code);
                }
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => std::thread::sleep(Duration::from_millis(20)),
                Err(e) => return LoopbackAnswer::Refused(format!("the sign-in listener on this Mac failed: {e}")),
            }
        }
        LoopbackAnswer::Refused(format!("no answer came back within {} seconds; start the sign-in again", timeout.as_secs()))
    }
}

/// For tests: the redirect a browser would follow (`?code=…&state=…`).
pub fn redirect_with(uri: &str, fields: &[(&str, &str)]) -> String {
    format!("{uri}?{}", fields.iter().map(|(k, v)| format!("{k}={}", percent_encode(v))).collect::<Vec<_>>().join("&"))
}
