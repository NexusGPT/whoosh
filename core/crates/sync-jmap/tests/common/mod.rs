//! A local JMAP test server on 127.0.0.1 that a test starts and stops: an in-process fake of the
//! JMAP core and mail methods the adapter uses (RFC 8620 session, Mailbox/get, Email/query,
//! Email/get, Email/changes) and the EventSource stream. Nothing here leaves the machine, and the
//! only token is a fixture string.
#![allow(dead_code)]
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use async_trait::async_trait;
use contracts::CoreError;
use serde_json::{json, Value};
use sync_jmap::{Delivery, JmapAuth, JmapConfig, LinkStatus, MessageSink};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{broadcast, mpsc, watch};

pub const TOKEN: &str = "fixture-token";
pub const ACCOUNT: &str = "A1";
/// 2026-10-01T09:00:00Z; message n was received n-1 days later.
pub const BASE_EPOCH: i64 = 1_790_845_200;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Mode {
    /// A full JMAP host.
    Jmap,
    /// A host with no JMAP: 404 on /.well-known/jmap.
    NoJmap404,
    /// 200 with a web page, not a session.
    NotJson,
    /// A JMAP session without the mail capability.
    NoMail,
    /// 401 on everything.
    AuthRefused,
    /// 503 on the session request.
    Busy,
}

#[derive(Clone, Debug)]
pub struct FEmail {
    pub id: String,
    pub thread: String,
    pub mailboxes: Vec<String>,
    pub keywords: Vec<String>,
    pub day: i64,
    pub from_name: Option<String>,
    pub from_email: String,
    pub to: Vec<String>,
    pub subject: String,
    pub preview: String,
    pub attachments: Vec<String>,
}

impl FEmail {
    pub fn inbox(n: i64) -> FEmail {
        FEmail {
            id: format!("e{n:02}"),
            thread: format!("t{n:02}"),
            mailboxes: vec!["m-inbox".into()],
            keywords: vec![],
            day: n,
            from_name: Some(format!("Sender {n}")),
            from_email: format!("sender{n}@example.test"),
            to: vec!["owner@example.test".into()],
            subject: format!("Subject {n}"),
            preview: format!("Preview text of message number {n}"),
            attachments: vec![],
        }
    }
    pub fn received_iso(&self) -> String {
        format!("2026-10-{:02}T09:00:00Z", self.day)
    }
    pub fn received_ms(&self) -> i64 {
        (BASE_EPOCH + (self.day - 1) * 86_400) * 1000
    }
}

struct Change { after: u64, created: Vec<String>, updated: Vec<String>, destroyed: Vec<String> }

struct Inner {
    emails: Vec<FEmail>,
    seq: u64,
    changes: Vec<Change>,
    log: Vec<String>,
    max_get: usize,
}

struct Shared {
    mode: Mode,
    inner: Mutex<Inner>,
    push: broadcast::Sender<String>,
    kill: broadcast::Sender<()>,
    addr: Mutex<Option<SocketAddr>>,
}

pub struct Fake {
    shared: Arc<Shared>,
    pub addr: SocketAddr,
    stop: watch::Sender<bool>,
    task: Mutex<Option<tokio::task::JoinHandle<()>>>,
}

impl Fake {
    pub async fn start(mode: Mode, emails: Vec<FEmail>, max_get: usize) -> Fake {
        // No proxy may stand between the test and 127.0.0.1.
        std::env::set_var("NO_PROXY", "127.0.0.1");
        std::env::set_var("no_proxy", "127.0.0.1");
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind 127.0.0.1");
        let addr = listener.local_addr().unwrap();
        let (push, _) = broadcast::channel(64);
        let (kill, _) = broadcast::channel(8);
        let seq = emails.len() as u64;
        let shared = Arc::new(Shared {
            mode,
            inner: Mutex::new(Inner { emails, seq, changes: vec![], log: vec![], max_get }),
            push,
            kill,
            addr: Mutex::new(Some(addr)),
        });
        let (stop, mut stop_rx) = watch::channel(false);
        let s = shared.clone();
        let task = tokio::spawn(async move {
            loop {
                tokio::select! {
                    _ = stop_rx.changed() => break,
                    accepted = listener.accept() => {
                        if let Ok((sock, _)) = accepted {
                            let s = s.clone();
                            let stop_rx = stop_rx.clone();
                            tokio::spawn(async move { let _ = serve(sock, s, stop_rx).await; });
                        }
                    }
                }
            }
        });
        Fake { shared, addr, stop, task: Mutex::new(Some(task)) }
    }

    pub fn base_url(&self) -> String {
        format!("http://{}", self.addr)
    }

    pub fn config(&self) -> JmapConfig {
        config_for(&self.base_url(), JmapAuth::Bearer(TOKEN.into()))
    }

    pub fn log(&self) -> Vec<String> {
        self.shared.inner.lock().unwrap().log.clone()
    }

    pub fn note(&self, line: String) {
        self.shared.inner.lock().unwrap().log.push(line);
    }

    pub fn state(&self) -> String {
        format!("s{}", self.shared.inner.lock().unwrap().seq)
    }

    pub fn add_email(&self, email: FEmail) {
        let mut g = self.shared.inner.lock().unwrap();
        g.seq += 1;
        let after = g.seq;
        g.changes.push(Change { after, created: vec![email.id.clone()], updated: vec![], destroyed: vec![] });
        g.emails.push(email);
    }

    pub fn mark_seen(&self, id: &str) {
        let mut g = self.shared.inner.lock().unwrap();
        g.seq += 1;
        let after = g.seq;
        if let Some(e) = g.emails.iter_mut().find(|e| e.id == id) {
            e.keywords.push("$seen".into());
        }
        g.changes.push(Change { after, created: vec![], updated: vec![id.to_string()], destroyed: vec![] });
    }

    pub fn destroy(&self, id: &str) {
        let mut g = self.shared.inner.lock().unwrap();
        g.seq += 1;
        let after = g.seq;
        g.emails.retain(|e| e.id != id);
        g.changes.push(Change { after, created: vec![], updated: vec![], destroyed: vec![id.to_string()] });
    }

    /// Tell every open EventSource stream that the Email state moved (the server's push).
    pub fn push_state_change(&self) {
        let state = self.state();
        let data = json!({"@type": "StateChange", "changed": {ACCOUNT: {"Email": state}}});
        let _ = self.shared.push.send(format!("event: state\ndata: {data}\n\n"));
    }

    /// Cut every open EventSource stream, as a network drop would.
    pub fn drop_streams(&self) {
        let _ = self.shared.kill.send(());
    }

    /// Stop the server: the accept loop ends, open streams close, the port is released.
    pub async fn shutdown(&self) -> SocketAddr {
        let _ = self.stop.send(true);
        let handle = self.task.lock().unwrap().take();
        if let Some(h) = handle {
            let _ = h.await;
        }
        self.addr
    }
}

pub fn config_for(base_url: &str, auth: JmapAuth) -> JmapConfig {
    JmapConfig {
        account_id: "acct-1".into(),
        account_name: "Fastmail test".into(),
        base_url: base_url.to_string(),
        auth,
        redirect_hosts: vec![],
        request_timeout: Duration::from_secs(5),
    }
}

fn authorised(head: &str) -> bool {
    head.lines().any(|l| l.eq_ignore_ascii_case(&format!("authorization: Bearer {TOKEN}")))
}

async fn respond(sock: &mut TcpStream, status: &str, content_type: &str, body: &str) -> std::io::Result<()> {
    let head = format!("HTTP/1.1 {status}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", body.len());
    sock.write_all(head.as_bytes()).await?;
    sock.write_all(body.as_bytes()).await?;
    sock.shutdown().await
}

async fn serve(mut sock: TcpStream, sh: Arc<Shared>, mut stop: watch::Receiver<bool>) -> std::io::Result<()> {
    let mut buf = Vec::new();
    let mut chunk = [0u8; 4096];
    let (head_end, content_length) = loop {
        let n = sock.read(&mut chunk).await?;
        if n == 0 {
            return Ok(());
        }
        buf.extend_from_slice(&chunk[..n]);
        if let Some(pos) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
            let head = String::from_utf8_lossy(&buf[..pos]).to_string();
            let len = head.lines().find_map(|l| l.to_ascii_lowercase().strip_prefix("content-length:").map(|v| v.trim().parse::<usize>().unwrap_or(0))).unwrap_or(0);
            break (pos + 4, len);
        }
    };
    while buf.len() < head_end + content_length {
        let n = sock.read(&mut chunk).await?;
        if n == 0 {
            break;
        }
        buf.extend_from_slice(&chunk[..n]);
    }
    let head = String::from_utf8_lossy(&buf[..head_end]).to_string();
    let body = String::from_utf8_lossy(&buf[head_end..]).to_string();
    let request_line = head.lines().next().unwrap_or("").to_string();
    let mut parts = request_line.split_whitespace();
    let method = parts.next().unwrap_or("");
    let target = parts.next().unwrap_or("");
    let path = target.split('?').next().unwrap_or("");

    if sh.mode == Mode::AuthRefused || !authorised(&head) {
        return respond(&mut sock, "401 Unauthorized", "text/plain", "no").await;
    }
    match (method, path) {
        ("GET", "/.well-known/jmap") => match sh.mode {
            Mode::NoJmap404 => respond(&mut sock, "404 Not Found", "text/html", "<html>not here</html>").await,
            Mode::NotJson => respond(&mut sock, "200 OK", "text/html", "<html><body>Welcome to our web site</body></html>").await,
            Mode::Busy => respond(&mut sock, "503 Service Unavailable", "text/plain", "try later").await,
            _ => {
                let addr = sh.addr.lock().unwrap().unwrap();
                let max_get = sh.inner.lock().unwrap().max_get;
                let session = session_json(addr, max_get, sh.mode != Mode::NoMail);
                respond(&mut sock, "200 OK", "application/json", &session.to_string()).await
            }
        },
        ("POST", "/api") => {
            let reply = api(&sh, &body);
            respond(&mut sock, "200 OK", "application/json", &reply.to_string()).await
        }
        ("GET", "/events") => {
            let mut push = sh.push.subscribe();
            let mut kill = sh.kill.subscribe();
            sock.write_all(b"HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nCache-Control: no-cache\r\nConnection: close\r\n\r\nevent: ping\ndata: {\"interval\": 1}\n\n").await?;
            sh.inner.lock().unwrap().log.push("events: open".into());
            loop {
                tokio::select! {
                    _ = stop.changed() => break,
                    _ = kill.recv() => break,
                    ev = push.recv() => match ev {
                        Ok(text) => sock.write_all(text.as_bytes()).await?,
                        Err(_) => break,
                    },
                }
            }
            sock.shutdown().await
        }
        _ => respond(&mut sock, "404 Not Found", "text/plain", "no such route").await,
    }
}

fn session_json(addr: SocketAddr, max_get: usize, with_mail: bool) -> Value {
    let mut caps = json!({
        "urn:ietf:params:jmap:core": {
            "maxSizeUpload": 50000000, "maxConcurrentUpload": 4, "maxSizeRequest": 10000000,
            "maxConcurrentRequests": 4, "maxCallsInRequest": 16, "maxObjectsInGet": max_get,
            "maxObjectsInSet": 500, "collationAlgorithms": ["i;ascii-casemap"]
        }
    });
    let mut acct_caps = json!({"urn:ietf:params:jmap:core": {}});
    let mut primary = json!({});
    if with_mail {
        let mail = json!({
            "maxMailboxesPerEmail": null, "maxMailboxDepth": 10, "maxSizeMailboxName": 255,
            "maxSizeAttachmentsPerEmail": 50000000, "emailQuerySortOptions": ["receivedAt"],
            "mayCreateTopLevelMailbox": true
        });
        caps["urn:ietf:params:jmap:mail"] = json!({});
        acct_caps["urn:ietf:params:jmap:mail"] = mail;
        primary["urn:ietf:params:jmap:mail"] = json!(ACCOUNT);
    }
    json!({
        "capabilities": caps,
        "accounts": {ACCOUNT: {"name": "owner@example.test", "isPersonal": true, "isReadOnly": false, "accountCapabilities": acct_caps}},
        "primaryAccounts": primary,
        "username": "owner@example.test",
        "apiUrl": format!("http://{addr}/api"),
        "downloadUrl": format!("http://{addr}/dl/{{accountId}}/{{blobId}}/{{name}}?type={{type}}"),
        "uploadUrl": format!("http://{addr}/ul/{{accountId}}"),
        "eventSourceUrl": format!("http://{addr}/events?types={{types}}&closeafter={{closeafter}}&ping={{ping}}"),
        "state": "session-1"
    })
}

fn mailbox_json(id: &str, name: &str, role: Option<&str>, emails: &[FEmail]) -> Value {
    let mine: Vec<&FEmail> = emails.iter().filter(|e| e.mailboxes.iter().any(|m| m == id)).collect();
    let unread = mine.iter().filter(|e| !e.keywords.iter().any(|k| k == "$seen")).count();
    json!({
        "id": id, "name": name, "parentId": null, "role": role, "sortOrder": 1,
        "totalEmails": mine.len(), "unreadEmails": unread, "totalThreads": mine.len(), "unreadThreads": unread,
        "myRights": {"mayReadItems": true, "mayAddItems": true, "mayRemoveItems": true, "maySetSeen": true,
                     "maySetKeywords": true, "mayCreateChild": true, "mayRename": true, "mayDelete": true, "maySubmit": true},
        "isSubscribed": true
    })
}

fn email_json(e: &FEmail) -> Value {
    let keywords: serde_json::Map<String, Value> = e.keywords.iter().map(|k| (k.clone(), json!(true))).collect();
    let mailboxes: serde_json::Map<String, Value> = e.mailboxes.iter().map(|m| (m.clone(), json!(true))).collect();
    let attachments: Vec<Value> = e.attachments.iter().enumerate().map(|(i, n)| json!({"partId": format!("p{i}"), "blobId": format!("b{i}"), "size": 10, "name": n, "type": "application/pdf"})).collect();
    json!({
        "id": e.id, "threadId": e.thread, "mailboxIds": mailboxes, "keywords": keywords,
        "receivedAt": e.received_iso(),
        "from": [{"name": e.from_name, "email": e.from_email}],
        "to": e.to.iter().map(|a| json!({"name": null, "email": a})).collect::<Vec<_>>(),
        "subject": e.subject, "preview": e.preview, "attachments": attachments
    })
}

fn resolve_ids(args: &Value, results: &[(String, Value)]) -> Vec<String> {
    if let Some(ids) = args.get("ids").and_then(|v| v.as_array()) {
        return ids.iter().filter_map(|v| v.as_str().map(str::to_string)).collect();
    }
    if let Some(r) = args.get("#ids") {
        let call = r["resultOf"].as_str().unwrap_or("");
        if let Some((_, resp)) = results.iter().find(|(id, _)| id == call) {
            return resp["ids"].as_array().map(|a| a.iter().filter_map(|v| v.as_str().map(str::to_string)).collect()).unwrap_or_default();
        }
    }
    vec![]
}

fn api(sh: &Shared, body: &str) -> Value {
    let request: Value = serde_json::from_str(body).unwrap_or(Value::Null);
    let calls = request["methodCalls"].as_array().cloned().unwrap_or_default();
    let mut g = sh.inner.lock().unwrap();
    let mut results: Vec<(String, Value)> = Vec::new();
    let mut out: Vec<Value> = Vec::new();
    for call in calls {
        let name = call[0].as_str().unwrap_or("").to_string();
        let args = call[1].clone();
        let call_id = call[2].as_str().unwrap_or("").to_string();
        let (reply_name, reply) = match name.as_str() {
            "Mailbox/get" => {
                g.log.push("mailbox/get".into());
                let list = vec![
                    mailbox_json("m-inbox", "Inbox", Some("inbox"), &g.emails),
                    mailbox_json("m-archive", "Archive", Some("archive"), &g.emails),
                    mailbox_json("m-sent", "Sent", Some("sent"), &g.emails),
                ];
                (name.clone(), json!({"accountId": ACCOUNT, "state": format!("mb{}", g.seq), "list": list, "notFound": []}))
            }
            "Email/query" => {
                let mailbox = args["filter"]["inMailbox"].as_str().unwrap_or("").to_string();
                let limit = args["limit"].as_u64().map(|v| v as usize);
                let anchor = args["anchor"].as_str().map(str::to_string);
                g.log.push(format!("query mailbox={mailbox} anchor={} limit={}", anchor.clone().unwrap_or("-".into()), limit.map(|l| l.to_string()).unwrap_or("-".into())));
                let mut matching: Vec<&FEmail> = g.emails.iter().filter(|e| e.mailboxes.contains(&mailbox)).collect();
                matching.sort_by(|a, b| b.day.cmp(&a.day).then(b.id.cmp(&a.id)));
                let position = match &anchor {
                    Some(a) => match matching.iter().position(|e| &e.id == a) {
                        Some(i) => (i as i64 + args["anchorOffset"].as_i64().unwrap_or(0)).max(0) as usize,
                        None => {
                            out.push(json!(["error", {"type": "anchorNotFound"}, call_id]));
                            continue;
                        }
                    },
                    None => args["position"].as_i64().unwrap_or(0).max(0) as usize,
                };
                let ids: Vec<String> = matching.iter().skip(position).take(limit.unwrap_or(usize::MAX)).map(|e| e.id.clone()).collect();
                let mut r = json!({"accountId": ACCOUNT, "queryState": format!("q{}", g.seq), "canCalculateChanges": false, "position": position, "ids": ids});
                if args["calculateTotal"].as_bool().unwrap_or(false) {
                    r["total"] = json!(matching.len());
                }
                (name.clone(), r)
            }
            "Email/get" => {
                let ids = resolve_ids(&args, &results);
                g.log.push(format!("get ids={}", ids.len()));
                let (found, missing): (Vec<&String>, Vec<&String>) = ids.iter().partition(|id| g.emails.iter().any(|e| &e.id == *id));
                // The list comes back in storage order, not request order, as a real server may.
                let list: Vec<Value> = g.emails.iter().filter(|e| found.contains(&&e.id)).map(email_json).collect();
                (name.clone(), json!({"accountId": ACCOUNT, "state": format!("s{}", g.seq), "list": list, "notFound": missing}))
            }
            "Email/changes" => {
                let since = args["sinceState"].as_str().unwrap_or("").to_string();
                g.log.push(format!("changes since={since}"));
                match since.strip_prefix('s').and_then(|n| n.parse::<u64>().ok()) {
                    Some(k) if k <= g.seq => {
                        let mut created = vec![];
                        let mut updated = vec![];
                        let mut destroyed = vec![];
                        for c in g.changes.iter().filter(|c| c.after > k) {
                            created.extend(c.created.clone());
                            updated.extend(c.updated.clone());
                            destroyed.extend(c.destroyed.clone());
                        }
                        (name.clone(), json!({"accountId": ACCOUNT, "oldState": since, "newState": format!("s{}", g.seq), "hasMoreChanges": false, "created": created, "updated": updated, "destroyed": destroyed}))
                    }
                    _ => {
                        out.push(json!(["error", {"type": "cannotCalculateChanges"}, call_id]));
                        continue;
                    }
                }
            }
            other => {
                out.push(json!(["error", {"type": "unknownMethod", "description": other}, call_id]));
                continue;
            }
        };
        results.push((call_id.clone(), reply.clone()));
        out.push(json!([reply_name, reply, call_id]));
    }
    json!({"methodResponses": out, "sessionState": "session-1"})
}

/// A sink that records what the core receives and writes each delivery into the fake's log, so a
/// test can read server requests and core deliveries as one ordered story.
pub struct RecSink {
    pub deliveries: Vec<Delivery>,
    pub statuses: Vec<LinkStatus>,
    pub log: Option<Arc<Fake>>,
    pub tx: Option<mpsc::UnboundedSender<Delivery>>,
    pub status_tx: Option<mpsc::UnboundedSender<LinkStatus>>,
}

impl RecSink {
    pub fn new(log: Option<Arc<Fake>>) -> RecSink {
        RecSink { deliveries: vec![], statuses: vec![], log, tx: None, status_tx: None }
    }
}

#[async_trait]
impl MessageSink for RecSink {
    async fn deliver(&mut self, batch: Delivery) -> Result<(), CoreError> {
        if let Some(f) = &self.log {
            f.note(format!("deliver n={} vanished={} cursor={}", batch.changed.len(), batch.vanished.len(), batch.cursor.is_some()));
        }
        if let Some(tx) = &self.tx {
            let _ = tx.send(batch.clone());
        }
        self.deliveries.push(batch);
        Ok(())
    }
    async fn status(&mut self, status: LinkStatus) {
        if let Some(tx) = &self.status_tx {
            let _ = tx.send(status.clone());
        }
        self.statuses.push(status);
    }
}

pub fn ids_of(d: &Delivery) -> Vec<String> {
    d.changed.iter().map(|m| m.server_id.clone()).collect()
}
