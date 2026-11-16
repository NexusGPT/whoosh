//! Test doubles. `FakeSmtp` is an in-process SMTP server on 127.0.0.1 that the test starts and
//! stops itself: no dependency, no licence, no network beyond the loopback.
use contracts::{Clock, CoreError};
use send::{AppendSentPort, AppendSentRequest};
use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::Duration;

#[derive(Clone, Debug)]
pub struct Received {
    pub mail_from: String,
    pub rcpt_to: Vec<String>,
    pub data: String,
}

#[derive(Clone, Copy)]
pub enum Mode {
    Accept,
    /// Answers MAIL FROM with a permanent 550 and its text.
    Refuse,
    /// Contracts delta 14: offers AUTH XOAUTH2 PLAIN LOGIN and records each AUTH line (`auths`), then accepts.
    Xoauth2,
}

pub struct FakeSmtp {
    pub port: u16,
    pub messages: Arc<Mutex<Vec<Received>>>,
    pub connections: Arc<AtomicUsize>,
    /// every AUTH command the client sent, as typed (delta 14)
    pub auths: Arc<Mutex<Vec<String>>>,
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

pub const REFUSAL_TEXT: &str = "5.7.1 relay denied for the fixture";

impl FakeSmtp {
    pub fn start(mode: Mode) -> FakeSmtp {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind loopback");
        listener.set_nonblocking(true).expect("nonblocking");
        let port = listener.local_addr().expect("addr").port();
        let messages = Arc::new(Mutex::new(Vec::new()));
        let connections = Arc::new(AtomicUsize::new(0));
        let stop = Arc::new(AtomicBool::new(false));
        let auths = Arc::new(Mutex::new(Vec::new()));
        let (m, c, s, a) = (messages.clone(), connections.clone(), stop.clone(), auths.clone());
        let thread = std::thread::spawn(move || {
            while !s.load(Ordering::SeqCst) {
                match listener.accept() {
                    Ok((stream, _)) => {
                        c.fetch_add(1, Ordering::SeqCst);
                        let _ = serve(stream, mode, &m, &a);
                    }
                    Err(_) => std::thread::sleep(Duration::from_millis(5)),
                }
            }
        });
        FakeSmtp { port, messages, connections, auths, stop, thread: Some(thread) }
    }

    pub fn received(&self) -> Vec<Received> {
        self.messages.lock().unwrap().clone()
    }
}

impl Drop for FakeSmtp {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

fn serve(stream: TcpStream, mode: Mode, sink: &Mutex<Vec<Received>>, auths: &Mutex<Vec<String>>) -> std::io::Result<()> {
    stream.set_nonblocking(false)?;
    stream.set_read_timeout(Some(Duration::from_secs(5)))?;
    let mut out = stream.try_clone()?;
    let mut input = BufReader::new(stream);
    out.write_all(b"220 fake.local ESMTP fixture\r\n")?;
    let (mut from, mut rcpt) = (String::new(), Vec::<String>::new());
    let mut line = String::new();
    loop {
        line.clear();
        if input.read_line(&mut line)? == 0 {
            return Ok(());
        }
        let upper = line.trim_end().to_ascii_uppercase();
        if upper.starts_with("EHLO") && matches!(mode, Mode::Xoauth2) {
            out.write_all(b"250-fake.local\r\n250 AUTH XOAUTH2 PLAIN LOGIN\r\n")?;
        } else if upper.starts_with("AUTH") {
            auths.lock().unwrap().push(line.trim_end().to_string());
            out.write_all(b"235 2.7.0 accepted\r\n")?;
        } else if upper.starts_with("EHLO") || upper.starts_with("HELO") {
            out.write_all(b"250 fake.local\r\n")?;
        } else if upper.starts_with("MAIL FROM:") {
            if matches!(mode, Mode::Refuse) {
                out.write_all(format!("550 {REFUSAL_TEXT}\r\n").as_bytes())?;
                continue;
            }
            from = between_angles(&line);
            out.write_all(b"250 ok\r\n")?;
        } else if upper.starts_with("RCPT TO:") {
            rcpt.push(between_angles(&line));
            out.write_all(b"250 ok\r\n")?;
        } else if upper == "DATA" {
            out.write_all(b"354 go ahead\r\n")?;
            let mut data = String::new();
            loop {
                line.clear();
                if input.read_line(&mut line)? == 0 {
                    return Ok(());
                }
                if line == ".\r\n" {
                    break;
                }
                data.push_str(line.strip_prefix('.').filter(|_| line.starts_with("..")).unwrap_or(&line));
            }
            // The CRLF before the lone dot belongs to the terminator, not to the message.
            let data = data.strip_suffix("\r\n").unwrap_or(&data).to_string();
            sink.lock().unwrap().push(Received { mail_from: from.clone(), rcpt_to: std::mem::take(&mut rcpt), data });
            out.write_all(b"250 queued\r\n")?;
        } else if upper == "QUIT" {
            out.write_all(b"221 bye\r\n")?;
            return Ok(());
        } else {
            out.write_all(b"250 ok\r\n")?;
        }
    }
}

fn between_angles(line: &str) -> String {
    let (a, b) = (line.find('<'), line.rfind('>'));
    match (a, b) {
        (Some(a), Some(b)) if a < b => line[a + 1..b].to_string(),
        _ => String::new(),
    }
}

/// The IMAP adapter's port, stubbed: records every APPEND; can be told to fail.
pub struct StubSent {
    pub appended: Mutex<Vec<AppendSentRequest>>,
    pub fail_with: Option<String>,
}

impl StubSent {
    pub fn ok() -> Arc<StubSent> {
        Arc::new(StubSent { appended: Mutex::new(Vec::new()), fail_with: None })
    }
    pub fn failing(words: &str) -> Arc<StubSent> {
        Arc::new(StubSent { appended: Mutex::new(Vec::new()), fail_with: Some(words.to_string()) })
    }
    pub fn count(&self) -> usize {
        self.appended.lock().unwrap().len()
    }
}

impl AppendSentPort for StubSent {
    fn execute(&self, request: &AppendSentRequest) -> Result<(), CoreError> {
        if let Some(words) = &self.fail_with {
            return Err(CoreError::Unexpected { reason_words: words.clone() });
        }
        self.appended.lock().unwrap().push(AppendSentRequest { account_id: request.account_id.clone(), rfc822: request.rfc822.clone() });
        Ok(())
    }
}

pub struct FixedClock(pub i64);

impl Clock for FixedClock {
    fn now_ms(&self) -> i64 {
        self.0
    }
    fn utc_offset_s(&self) -> i32 {
        0
    }
}

/// The text body of a received message, transfer encoding undone (7bit or quoted-printable).
pub fn decoded_body(data: &str) -> String {
    let (head, body) = data.split_once("\r\n\r\n").expect("header/body split");
    let lower = head.to_ascii_lowercase();
    if lower.contains("content-transfer-encoding: quoted-printable") {
        qp_decode(body)
    } else {
        assert!(lower.contains("content-transfer-encoding: 7bit"), "unhandled transfer encoding in:\n{head}");
        body.to_string()
    }
}

fn qp_decode(s: &str) -> String {
    let s = s.replace("=\r\n", "");
    let bytes = s.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'=' && i + 2 < bytes.len() {
            if let Ok(b) = u8::from_str_radix(&s[i + 1..i + 3], 16) {
                out.push(b);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8(out).expect("utf8 after qp")
}

pub fn header<'a>(data: &'a str, name: &str) -> Option<&'a str> {
    let head = data.split_once("\r\n\r\n")?.0;
    head.lines().find_map(|l| {
        let (n, v) = l.split_once(':')?;
        n.eq_ignore_ascii_case(name).then_some(v.trim())
    })
}
