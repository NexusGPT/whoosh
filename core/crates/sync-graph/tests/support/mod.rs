//! Test doubles: a fake clock, a sleeper that records instead of waiting, and a Graph stub on
//! 127.0.0.1 that serves recorded fixtures. Nothing here opens a connection to anything else.
#![allow(dead_code)]
use std::{
    collections::{BTreeMap, HashMap},
    sync::{
        atomic::{AtomicI64, AtomicUsize, Ordering::SeqCst},
        Arc, Mutex,
    },
    time::Duration,
};

use contracts::{ActionFailed, Clock, CoreError};
use sync_graph::{BoxFut, GraphConfig, GraphMailbox, Sleeper, SlowedSink, Throttle, TokenSource};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
};

pub const TOKEN: &str = "test-token-example";

pub fn fixture(name: &str) -> String {
    let p = format!("{}/fixtures/{name}", env!("CARGO_MANIFEST_DIR"));
    std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("fixture {p}: {e}"))
}

pub struct FakeClock(pub AtomicI64);
impl Clock for FakeClock {
    fn now_ms(&self) -> i64 {
        self.0.load(SeqCst)
    }
    fn utc_offset_s(&self) -> i32 {
        0
    }
}

pub struct RecSleeper {
    pub clock: Arc<FakeClock>,
    pub slept: Mutex<Vec<u64>>,
}
impl Sleeper for RecSleeper {
    fn sleep_ms(&self, ms: u64) -> BoxFut<'_, ()> {
        Box::pin(async move {
            self.slept.lock().unwrap().push(ms);
            self.clock.0.fetch_add(ms as i64, SeqCst);
        })
    }
}
impl RecSleeper {
    pub fn slept(&self) -> Vec<u64> {
        self.slept.lock().unwrap().clone()
    }
}

struct StaticToken;
impl TokenSource for StaticToken {
    fn access_token(&self) -> BoxFut<'_, Result<String, CoreError>> {
        Box::pin(async { Ok(TOKEN.to_string()) })
    }
}

pub struct Rig {
    pub mailbox: Arc<GraphMailbox>,
    pub clock: Arc<FakeClock>,
    pub sleeper: Arc<RecSleeper>,
    pub slowed: Arc<Mutex<Vec<ActionFailed>>>,
}

pub const ACCOUNT_NAME: &str = "Work Example";

pub fn rig(base: &str, page_size: u32, max_attempts: u32) -> Rig {
    assert!(base.starts_with("http://127.0.0.1:"), "tests only talk to the local stub, got {base}");
    let clock = Arc::new(FakeClock(AtomicI64::new(1_793_000_000_000)));
    let sleeper = Arc::new(RecSleeper { clock: clock.clone(), slept: Mutex::new(Vec::new()) });
    let throttle = Arc::new(Throttle::new(clock.clone(), sleeper.clone()));
    let slowed: Arc<Mutex<Vec<ActionFailed>>> = Arc::new(Mutex::new(Vec::new()));
    let sink: SlowedSink = {
        let s = slowed.clone();
        Arc::new(move |f| s.lock().unwrap().push(f))
    };
    let cfg = GraphConfig {
        base_url: format!("{base}/v1.0"),
        mailbox_path: "me".into(),
        account_id: "acct-work-example".into(),
        account_name: ACCOUNT_NAME.into(),
        max_attempts,
        backoff_base_ms: 1_000,
        backoff_max_ms: 60_000,
        timeout_ms: 20_000,
        page_size,
    };
    let mailbox = Arc::new(GraphMailbox::new(cfg, Arc::new(StaticToken), throttle, Some(sink)).expect("mailbox"));
    Rig { mailbox, clock, sleeper, slowed }
}

// ---- the stub server ----

#[derive(Clone, Debug)]
pub struct Req {
    pub method: String,
    pub target: String,
    pub headers: HashMap<String, String>,
    pub body: String,
}

pub struct Resp {
    pub status: u16,
    pub headers: Vec<(String, String)>,
    pub body: String,
}
impl Resp {
    pub fn status(status: u16, body: String) -> Self {
        Resp { status, headers: Vec::new(), body }
    }
    pub fn ok(body: String) -> Self {
        Self::status(200, body)
    }
    pub fn header(mut self, k: &str, v: &str) -> Self {
        self.headers.push((k.into(), v.into()));
        self
    }
}

pub struct Stub {
    pub base: String,
    pub log: Arc<Mutex<Vec<Req>>>,
    pub max_inflight: Arc<AtomicUsize>,
}
impl Stub {
    pub fn requests(&self) -> Vec<Req> {
        self.log.lock().unwrap().clone()
    }
}

fn find(hay: &[u8], needle: &[u8]) -> Option<usize> {
    hay.windows(needle.len()).position(|w| w == needle)
}

async fn read_req(sock: &mut TcpStream) -> Option<Req> {
    let mut buf = Vec::new();
    let mut tmp = [0u8; 4096];
    let head_end = loop {
        let n = sock.read(&mut tmp).await.ok()?;
        if n == 0 {
            return None;
        }
        buf.extend_from_slice(&tmp[..n]);
        if let Some(p) = find(&buf, b"\r\n\r\n") {
            break p;
        }
    };
    let head = String::from_utf8_lossy(&buf[..head_end]).to_string();
    let mut lines = head.split("\r\n");
    let mut first = lines.next()?.split(' ');
    let method = first.next()?.to_string();
    let target = first.next()?.to_string();
    let headers: HashMap<String, String> = lines.filter_map(|l| l.split_once(':')).map(|(k, v)| (k.trim().to_lowercase(), v.trim().to_string())).collect();
    let want: usize = headers.get("content-length").and_then(|v| v.parse().ok()).unwrap_or(0);
    let mut body = buf[head_end + 4..].to_vec();
    while body.len() < want {
        let n = sock.read(&mut tmp).await.ok()?;
        if n == 0 {
            break;
        }
        body.extend_from_slice(&tmp[..n]);
    }
    Some(Req { method, target, headers, body: String::from_utf8_lossy(&body).to_string() })
}

/// Serve `handler` on 127.0.0.1. Each response is held `delay_ms` so concurrent requests overlap.
/// `{BASE}` in a body or header is replaced by the stub's own address.
pub async fn start(delay_ms: u64, handler: impl Fn(&Req) -> Resp + Send + Sync + 'static) -> Stub {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind loopback");
    let base = format!("http://{}", listener.local_addr().expect("addr"));
    let log = Arc::new(Mutex::new(Vec::new()));
    let inflight = Arc::new(AtomicUsize::new(0));
    let max_inflight = Arc::new(AtomicUsize::new(0));
    let handler = Arc::new(handler);
    {
        let (log, inflight, max_inflight, base) = (log.clone(), inflight.clone(), max_inflight.clone(), base.clone());
        tokio::spawn(async move {
            while let Ok((mut sock, _)) = listener.accept().await {
                let (handler, log, inflight, max_inflight, base) = (handler.clone(), log.clone(), inflight.clone(), max_inflight.clone(), base.clone());
                tokio::spawn(async move {
                    let Some(req) = read_req(&mut sock).await else { return };
                    let now = inflight.fetch_add(1, SeqCst) + 1;
                    max_inflight.fetch_max(now, SeqCst);
                    log.lock().unwrap().push(req.clone());
                    let resp = handler(&req);
                    if delay_ms > 0 {
                        tokio::time::sleep(Duration::from_millis(delay_ms)).await;
                    }
                    inflight.fetch_sub(1, SeqCst);
                    let body = resp.body.replace("{BASE}", &base);
                    let mut head = format!("HTTP/1.1 {} Stub\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n", resp.status, body.len());
                    for (k, v) in &resp.headers {
                        head.push_str(&format!("{k}: {}\r\n", v.replace("{BASE}", &base)));
                    }
                    head.push_str("\r\n");
                    let _ = sock.write_all(head.as_bytes()).await;
                    let _ = sock.write_all(body.as_bytes()).await;
                    let _ = sock.shutdown().await;
                });
            }
        });
    }
    Stub { base, log, max_inflight }
}

// ---- a mailbox model that serves delta pages the way Graph does ----

pub struct Model {
    pub msgs: BTreeMap<String, bool>,
    pub log: Vec<(u32, String, bool)>,
    pub version: u32,
}
impl Model {
    pub fn new(n: usize) -> Self {
        Model { msgs: (0..n).map(|i| (format!("AAMkMsg{i:03}="), false)).collect(), log: Vec::new(), version: 0 }
    }
    pub fn mark_read(&mut self, id: &str) {
        self.version += 1;
        self.msgs.insert(id.into(), true);
        self.log.push((self.version, id.into(), false));
    }
    pub fn remove(&mut self, id: &str) {
        self.version += 1;
        self.msgs.remove(id);
        self.log.push((self.version, id.into(), true));
    }
    pub fn add(&mut self, id: &str) {
        self.version += 1;
        self.msgs.insert(id.into(), false);
        self.log.push((self.version, id.into(), false));
    }
}

pub fn msg_json(id: &str, read: bool) -> serde_json::Value {
    serde_json::json!({
        "id": id, "conversationId": format!("conv-{id}"), "subject": "Quarterly numbers",
        "bodyPreview": "The numbers are in the shared sheet.", "receivedDateTime": "2026-11-02T08:15:00Z",
        "isRead": read,
        "from": { "emailAddress": { "name": "Dana Example", "address": "dana@example.net" } },
        "toRecipients": [ { "emailAddress": { "name": "Owner Example", "address": "owner@example.net" } } ]
    })
}

/// Delta pages from `model`; `served` counts every message record put on the wire.
pub fn model_handler(model: Arc<Mutex<Model>>, served: Arc<AtomicUsize>) -> impl Fn(&Req) -> Resp + Send + Sync + 'static {
    move |req: &Req| {
        let m = model.lock().unwrap();
        let query = req.target.split_once('?').map_or("", |x| x.1);
        let param = |k: &str| query.split('&').find_map(|kv| kv.strip_prefix(&format!("{k}=")).map(str::to_string));
        let page: usize = req.headers.get("prefer").and_then(|p| p.split("odata.maxpagesize=").nth(1)).and_then(|r| r.split(',').next()).and_then(|n| n.trim().parse().ok()).expect("a page size header");
        let delta_link = format!("{{BASE}}/v1.0/me/mailFolders/inbox-id/messages/delta?$deltatoken=v{}", m.version);
        if let Some(tok) = param("$deltatoken") {
            let since: u32 = tok.trim_start_matches('v').parse().unwrap();
            let mut last: BTreeMap<&str, bool> = BTreeMap::new();
            for (v, id, removed) in &m.log {
                if *v > since {
                    last.insert(id, *removed);
                }
            }
            let items: Vec<_> = last.iter().map(|(id, removed)| if *removed { serde_json::json!({ "id": id, "@removed": { "reason": "deleted" } }) } else { msg_json(id, m.msgs[*id]) }).collect();
            served.fetch_add(items.len(), SeqCst);
            return Resp::ok(serde_json::json!({ "value": items, "@odata.deltaLink": delta_link }).to_string());
        }
        let skip: usize = param("$skiptoken").map_or(0, |s| s.parse().unwrap());
        let items: Vec<_> = m.msgs.iter().skip(skip).take(page).map(|(id, read)| msg_json(id, *read)).collect();
        served.fetch_add(items.len(), SeqCst);
        let mut body = serde_json::json!({ "value": items });
        if skip + page < m.msgs.len() {
            body["@odata.nextLink"] = format!("{{BASE}}/v1.0/me/mailFolders/inbox-id/messages/delta?$skiptoken={}", skip + page).into();
        } else {
            body["@odata.deltaLink"] = delta_link.into();
        }
        Resp::ok(body.to_string())
    }
}
