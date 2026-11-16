//! body-r2: the scripted IMAP server the body tests share (sync-imap's own and the engine's): it answers `UID FETCH <uids> (UID RFC822.SIZE
//! BODY.PEEK[])` in the shape each UID is given. Listens on 127.0.0.1 only; nothing here reaches a real host.
#![allow(dead_code)]
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use sync_imap::*;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpListener;

/// How the scripted server answers the FETCH of one message.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Shape {
    /// `BODY[] {n}` and the bytes, in one write.
    Whole,
    /// The same bytes, written in 4 KiB pieces, each flushed (a FETCH answer split across many reads).
    Chunked,
    /// The key carries its origin: `BODY[]<0> {n}`.
    Origin,
    /// `RFC822 {n}` instead of `BODY[]`.
    Rfc822,
    /// UID and size come after the literal, in the same FETCH.
    UidLast,
    /// Two untagged FETCH answers for one message: `(UID n RFC822.SIZE s)` and then `(BODY[] {n}...)` (RFC 3501 allows it).
    Split,
    /// `BODY[] NIL`.
    Nil,
    /// `BODY[] {0}`.
    Empty,
    /// The command ends OK with no FETCH for that UID.
    NoAnswer,
    /// A FETCH with UID and size only (the server left the body out).
    NoBodyKey,
    /// The first FETCH of the set carries no body; the same UID asked alone gets it (a server that drops a large message from a batch).
    DropsFromBatch,
    /// `BODY[] NIL` for the whole message, but the two sections (`BODY[HEADER]`, `BODY[TEXT]`) answered when they are asked.
    SectionsOnly,
}

pub struct Server { pub port: u16, pub shapes: Arc<Mutex<HashMap<u32, Shape>>>, pub log: Arc<Mutex<Vec<String>>>, pub files: Arc<Mutex<HashMap<u32, Fixture>>> }

#[path = "files.rs"]
mod files;
#[allow(unused_imports)]
pub use files::{files_mail, Fixture, FixtureFile};

pub fn raw(uid: u32, kb: usize) -> Vec<u8> {
    let line = "The Omega Glow Bowl: miso glazed salmon, sweet corn salsa, and toasted almonds over warm quinoa.\r\n";
    let filler = line.repeat(kb * 1000 / line.len() + 1);
    format!("From: sweetgreen <hello@sweetgreen.example>\r\nTo: owner@example.test\r\nSubject: Omega Glow Bowl {uid}\r\nMessage-ID: <{uid}@sweetgreen.example>\r\n\r\n{filler}").into_bytes()
}

fn set_of(s: &str) -> Vec<u32> {
    s.split(',').flat_map(|r| match r.split_once(':') { Some((a, b)) => (a.parse().unwrap()..=b.parse().unwrap()).collect::<Vec<u32>>(), None => vec![r.parse().unwrap()] }).collect()
}

impl Server {
    pub async fn start(shapes: &[(u32, Shape)], kb: usize) -> Server {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let shapes: Arc<Mutex<HashMap<u32, Shape>>> = Arc::new(Mutex::new(shapes.iter().copied().collect()));
        let log = Arc::new(Mutex::new(Vec::new()));
        let files: Arc<Mutex<HashMap<u32, Fixture>>> = Arc::new(Mutex::new(HashMap::new()));
        let (sh, lg, fl) = (shapes.clone(), log.clone(), files.clone());
        tokio::spawn(async move {
            loop {
                let Ok((sock, _)) = listener.accept().await else { return };
                let (sh, lg, fl) = (sh.clone(), lg.clone(), fl.clone());
                tokio::spawn(async move { let _ = serve(sock, sh, lg, fl, kb).await; });
            }
        });
        Server { port, shapes, log, files }
    }
    /// The server holds this message under `uid` (a UID FETCH of it answers the bytes, or the section asked).
    pub fn put(&self, uid: u32, fixture: Fixture) { self.files.lock().unwrap().insert(uid, fixture); }
    /// The UID FETCH lines that asked for one part (`BODY.PEEK[2]`).
    pub fn part_fetches(&self) -> Vec<String> { self.fetches().into_iter().filter(|l| l.contains("BODY.PEEK[") && !l.contains("BODY.PEEK[]") && !l.contains("HEADER") && !l.contains("TEXT")).collect() }
    pub fn fetches(&self) -> Vec<String> { self.log.lock().unwrap().iter().filter(|l| l.contains("FETCH")).cloned().collect() }
}

async fn serve(sock: tokio::net::TcpStream, shapes: Arc<Mutex<HashMap<u32, Shape>>>, log: Arc<Mutex<Vec<String>>>, files: Arc<Mutex<HashMap<u32, Fixture>>>, kb: usize) -> std::io::Result<()> {
    let (r, mut w) = tokio::io::split(sock);
    let mut r = BufReader::new(r);
    w.write_all(b"* OK [CAPABILITY IMAP4rev1] scripted\r\n").await?;
    let mut line = String::new();
    loop {
        line.clear();
        if r.read_line(&mut line).await? == 0 { return Ok(()); }
        let t = line.trim_end().to_string();
        log.lock().unwrap().push(t.clone());
        let (tag, rest) = t.split_once(' ').unwrap_or((&t, ""));
        let up = rest.to_ascii_uppercase();
        let words: Vec<&str> = rest.split_whitespace().collect();
        if up.starts_with("CAPABILITY") {
            w.write_all(format!("* CAPABILITY IMAP4rev1 UIDPLUS\r\n{tag} OK done\r\n").as_bytes()).await?;
        } else if up.starts_with("LOGIN") {
            w.write_all(format!("{tag} OK [CAPABILITY IMAP4rev1 UIDPLUS] logged in\r\n").as_bytes()).await?;
        } else if up.starts_with("SELECT") {
            w.write_all(format!("* 3 EXISTS\r\n* OK [UIDVALIDITY 1] ok\r\n* OK [UIDNEXT 200000] ok\r\n{tag} OK [READ-WRITE] done\r\n").as_bytes()).await?;
        } else if up.starts_with("LOGOUT") {
            w.write_all(format!("* BYE bye\r\n{tag} OK done\r\n").as_bytes()).await?;
            return Ok(());
        } else if up.starts_with("UID SEARCH UID ") {
            // fix-realcore round 9: the existence check after three asks with no FETCH line. Every UID this server was scripted with (a shape or a
            // fixture) is in the mailbox: it lists them; any other UID is not there.
            let held: Vec<String> = set_of(words.get(3).copied().unwrap_or("")).into_iter()
                .filter(|u| shapes.lock().unwrap().contains_key(u) || files.lock().unwrap().contains_key(u)).map(|u| u.to_string()).collect();
            let list = if held.is_empty() { String::new() } else { format!(" {}", held.join(" ")) };
            w.write_all(format!("* SEARCH{list}\r\n{tag} OK done\r\n").as_bytes()).await?;
        } else if up.starts_with("UID FETCH") {
            let uids = set_of(words[2]);
            let alone = uids.len() == 1;
            for (i, uid) in uids.iter().enumerate() {
                let held = files.lock().unwrap().get(uid).cloned();
                if let Some(fx) = held {
                    let seq = i + 1;
                    let mut o = format!("* {seq} FETCH (UID {uid}").into_bytes();
                    if up.contains("RFC822.SIZE") { o.extend(format!(" RFC822.SIZE {}", fx.raw.len()).bytes()); }
                    for item in up.split("BODY.PEEK[").skip(1) {
                        let section = item.split(']').next().unwrap_or("").to_string();
                        let bytes = if section.is_empty() { Some(fx.raw.clone()) } else { fx.sections.iter().find(|(k, _)| *k == section).map(|(_, b)| b.clone()) };
                        match bytes {
                            Some(b) => { o.extend(format!(" BODY[{section}] {{{}}}\r\n", b.len()).bytes()); o.extend(b); }
                            None => o.extend(format!(" BODY[{section}] NIL").bytes()),
                        }
                    }
                    o.extend(b")\r\n");
                    w.write_all(&o).await?;
                    continue;
                }
                let shape = shapes.lock().unwrap().get(uid).copied().unwrap_or(Shape::Whole);
                let body = raw(*uid, kb);
                let (seq, n) = (i + 1, body.len());
                let open = |extra: &str| format!("* {seq} FETCH (UID {uid} RFC822.SIZE {n}{extra}").into_bytes();
                match shape {
                    Shape::Whole => { let mut o = open(&format!(" BODY[] {{{n}}}\r\n")); o.extend(&body); o.extend(b")\r\n"); w.write_all(&o).await?; }
                    Shape::Chunked => {
                        let mut o = open(&format!(" BODY[] {{{n}}}\r\n"));
                        w.write_all(&o).await?; w.flush().await?;
                        for piece in body.chunks(4096) { w.write_all(piece).await?; w.flush().await?; }
                        o = b")\r\n".to_vec(); w.write_all(&o).await?;
                    }
                    Shape::Origin => { let mut o = open(&format!(" BODY[]<0> {{{n}}}\r\n")); o.extend(&body); o.extend(b")\r\n"); w.write_all(&o).await?; }
                    Shape::Rfc822 => { let mut o = open(&format!(" RFC822 {{{n}}}\r\n")); o.extend(&body); o.extend(b")\r\n"); w.write_all(&o).await?; }
                    Shape::UidLast => {
                        let mut o = format!("* {seq} FETCH (BODY[] {{{n}}}\r\n").into_bytes(); o.extend(&body);
                        o.extend(format!(" RFC822.SIZE {n} UID {uid})\r\n").bytes()); w.write_all(&o).await?;
                    }
                    Shape::Split => {
                        w.write_all(format!("* {seq} FETCH (UID {uid} RFC822.SIZE {n})\r\n").as_bytes()).await?;
                        let mut o = format!("* {seq} FETCH (BODY[] {{{n}}}\r\n").into_bytes(); o.extend(&body); o.extend(b")\r\n"); w.write_all(&o).await?;
                    }
                    Shape::Nil => w.write_all(&open(" BODY[] NIL)\r\n")).await?,
                    Shape::Empty => w.write_all(&open(" BODY[] {0}\r\n)\r\n")).await?,
                    Shape::NoAnswer => {}
                    Shape::NoBodyKey => w.write_all(&open(")\r\n")).await?,
                    Shape::DropsFromBatch if !alone => w.write_all(&open(")\r\n")).await?,
                    Shape::SectionsOnly if up.contains("BODY.PEEK[HEADER]") => {
                        let at = body.windows(4).position(|w| w == b"\r\n\r\n").unwrap() + 4;
                        let mut o = open(&format!(" BODY[HEADER] {{{}}}\r\n", at)); o.extend(&body[..at]);
                        o.extend(format!(" BODY[TEXT] {{{}}}\r\n", n - at).bytes()); o.extend(&body[at..]); o.extend(b")\r\n"); w.write_all(&o).await?;
                    }
                    Shape::SectionsOnly => w.write_all(&open(" BODY[] NIL)\r\n")).await?,
                    Shape::DropsFromBatch => { let mut o = open(&format!(" BODY[] {{{n}}}\r\n")); o.extend(&body); o.extend(b")\r\n"); w.write_all(&o).await?; }
                }
            }
            w.write_all(format!("{tag} OK UID FETCH completed\r\n").as_bytes()).await?;
        } else {
            w.write_all(format!("{tag} OK done\r\n").as_bytes()).await?;
        }
    }
}

pub fn adapter(srv: &Server, limits: ImapLimits) -> ImapAdapter {
    ImapAdapter::new(ImapConfig {
        account_id: "acc".into(), account_name: "acc".into(), host: "127.0.0.1".into(), port: srv.port, security: Security::PlainLoopbackOnly,
        username: "owner@example.test".into(), auth: Auth::Password("app-password".into()), extra_roots_der: Vec::new(), limits,
    })
}
