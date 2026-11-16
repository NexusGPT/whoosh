#![allow(dead_code)]
//! fix-realcore round 15: an SMTP server on 127.0.0.1 that breaks the connection at one chosen step, the first `breaks` times a
//! connection reaches that step, and serves every other connection to the end (an account check that only says EHLO and AUTH and quits
//! never uses up a break at MAIL FROM). It answers EHLO with STARTTLS, 8BITMIME and AUTH XOAUTH2 PLAIN LOGIN, as
//! smtp.office365.com does on port 587. A break is a TCP reset (SO_LINGER 0, then close: the client reads "Connection reset by peer",
//! the owner's words on main115) or a plain close (the client reads the end of the stream). Loopback only; no other address is opened.
//! Used by send/tests/send_stage_failures.rs, outbox/tests/send_retry.rs and engine/tests/engine_send_retry.rs (included by path).
use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::Duration;

/// Where the server breaks the connection. The names are the send crate's stage ids (send/src/smtp.rs `Stage::id`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BreakAt {
    /// before the 220 greeting
    Connect,
    /// when the STARTTLS command arrives, before its 220
    StartTls,
    /// when the AUTH command arrives, before its answer
    Auth,
    MailFrom,
    RcptTo,
    /// when the DATA command arrives, before its 354
    Data,
    /// after the whole message and its terminating dot were read and KEPT, before the 250: the server has the mail
    AfterData,
}

impl BreakAt {
    pub const ALL: [BreakAt; 7] = [BreakAt::Connect, BreakAt::StartTls, BreakAt::Auth, BreakAt::MailFrom, BreakAt::RcptTo, BreakAt::Data, BreakAt::AfterData];
    pub fn id(self) -> &'static str {
        match self {
            BreakAt::Connect => "connect",
            BreakAt::StartTls => "starttls",
            BreakAt::Auth => "auth",
            BreakAt::MailFrom => "mail_from",
            BreakAt::RcptTo => "rcpt_to",
            BreakAt::Data => "data",
            BreakAt::AfterData => "after_data",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum How {
    Reset,
    Close,
    /// The server ANSWERS, with a "try later" code, and the connection stays open (lead, 13 Nov, owner picture 208: Microsoft 365 answered
    /// `451 4.7.0 Temporary server error. Please try again later.` at AUTH).
    TryLater,
}

#[derive(Clone, Debug)]
pub struct Got {
    pub mail_from: String,
    pub rcpt_to: Vec<String>,
    pub data: String,
}

pub struct StagedSmtp {
    pub port: u16,
    pub messages: Arc<Mutex<Vec<Got>>>,
    pub connections: Arc<AtomicUsize>,
    /// how many times the connection was broken
    pub broken: Arc<AtomicUsize>,
    /// the commands of every connection, in order (AUTH lines shortened to their mechanism: no token is kept)
    pub log: Arc<Mutex<Vec<Vec<String>>>>,
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl StagedSmtp {
    /// The first `breaks` connections that reach `at` break there by `how`; the rest are served to the end.
    pub fn start(at: BreakAt, how: How, breaks: usize) -> StagedSmtp {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind loopback");
        listener.set_nonblocking(true).expect("nonblocking");
        let port = listener.local_addr().expect("addr").port();
        let messages = Arc::new(Mutex::new(Vec::new()));
        let connections = Arc::new(AtomicUsize::new(0));
        let log = Arc::new(Mutex::new(Vec::new()));
        let stop = Arc::new(AtomicBool::new(false));
        let broken = Arc::new(AtomicUsize::new(0));
        let (m, c, l, s, b) = (messages.clone(), connections.clone(), log.clone(), stop.clone(), broken.clone());
        let thread = std::thread::spawn(move || {
            while !s.load(Ordering::SeqCst) {
                match listener.accept() {
                    Ok((stream, _)) => {
                        c.fetch_add(1, Ordering::SeqCst);
                        let mut lines = Vec::new();
                        let r = serve(stream, (at, how, breaks, &b), &m, &mut lines);
                        if let Err(e) = r { lines.push(format!("(io: {e})")); }
                        l.lock().unwrap().push(lines);
                    }
                    Err(_) => std::thread::sleep(Duration::from_millis(5)),
                }
            }
        });
        StagedSmtp { port, messages, connections, broken, log, stop, thread: Some(thread) }
    }

    pub fn received(&self) -> Vec<Got> {
        self.messages.lock().unwrap().clone()
    }

    /// Connections that sent MAIL FROM: tries to send a message (an account check never does).
    pub fn mail_sessions(&self) -> usize {
        self.log.lock().unwrap().iter().filter(|c| c.iter().any(|l| l.to_ascii_uppercase().starts_with("MAIL FROM:"))).count()
    }
}

impl Drop for StagedSmtp {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

/// Ends the connection: a reset (linger 0 then close) or a plain close. The caller drops both handles on return.
fn hang_up(stream: &TcpStream, how: How, lines: &mut Vec<String>, at: BreakAt) -> std::io::Result<()> {
    if how == How::Reset {
        socket2::SockRef::from(stream).set_linger(Some(Duration::ZERO))?;
    }
    lines.push(format!("(broke at {} by {how:?})", at.id()));
    Ok(())
}

fn serve(stream: TcpStream, plan: (BreakAt, How, usize, &AtomicUsize), sink: &Mutex<Vec<Got>>, lines: &mut Vec<String>) -> std::io::Result<()> {
    stream.set_nonblocking(false)?;
    stream.set_read_timeout(Some(Duration::from_secs(10)))?;
    let (at, how, breaks, broken) = plan;
    // a break is used up only when it happens
    let breaks_at = |s: BreakAt| (s == at && broken.fetch_update(Ordering::SeqCst, Ordering::SeqCst, |n| (n < breaks).then_some(n + 1)).is_ok()).then_some(how);
    if let Some(how) = breaks_at(BreakAt::Connect) {
        return hang_up(&stream, how, lines, BreakAt::Connect);
    }
    let mut out = stream.try_clone()?;
    let mut input = BufReader::new(stream.try_clone()?);
    out.write_all(b"220 fake.local ESMTP fixture\r\n")?;
    let (mut from, mut rcpt) = (String::new(), Vec::<String>::new());
    let mut line = String::new();
    loop {
        line.clear();
        if input.read_line(&mut line)? == 0 {
            return Ok(());
        }
        let upper = line.trim_end().to_ascii_uppercase();
        lines.push(if upper.starts_with("AUTH") { upper.split_whitespace().take(2).collect::<Vec<_>>().join(" ") } else { line.trim_end().to_string() });
        let step = if upper.starts_with("STARTTLS") { Some(BreakAt::StartTls) }
            else if upper.starts_with("AUTH") { Some(BreakAt::Auth) }
            else if upper.starts_with("MAIL FROM:") { Some(BreakAt::MailFrom) }
            else if upper.starts_with("RCPT TO:") { Some(BreakAt::RcptTo) }
            else if upper == "DATA" { Some(BreakAt::Data) }
            else { None };
        if let Some((s, how)) = step.and_then(|s| breaks_at(s).map(|h| (s, h))) {
            if how == How::TryLater {
                lines.push(format!("(answered 451 at {})", s.id()));
                out.write_all(b"451 4.7.0 Temporary server error. Please try again later. CPRX1\r\n")?;
                continue;
            }
            return hang_up(&stream, how, lines, s);
        }
        if upper.starts_with("EHLO") {
            out.write_all(b"250-fake.local\r\n250-STARTTLS\r\n250-8BITMIME\r\n250 AUTH XOAUTH2 PLAIN LOGIN\r\n")?;
        } else if upper.starts_with("HELO") {
            out.write_all(b"250 fake.local\r\n")?;
        } else if upper.starts_with("STARTTLS") {
            out.write_all(b"454 4.7.0 TLS not available on the fixture\r\n")?;
        } else if upper.starts_with("AUTH") {
            out.write_all(b"235 2.7.0 accepted\r\n")?;
        } else if upper.starts_with("MAIL FROM:") {
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
            let data = data.strip_suffix("\r\n").unwrap_or(&data).to_string();
            sink.lock().unwrap().push(Got { mail_from: from.clone(), rcpt_to: std::mem::take(&mut rcpt), data });
            if let Some(how) = breaks_at(BreakAt::AfterData) {
                return hang_up(&stream, how, lines, BreakAt::AfterData);
            }
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
