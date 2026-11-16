//! Replay transport over recorded-shape fixtures, a virtual clock, and sinks. No socket is ever opened.
#![allow(dead_code)]
use serde_json::Value;
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::{Arc, Mutex};
use sync_gmail_api::limits::*;
use sync_gmail_api::transport::{FeatureFlags, Method, Pacer, Request, Response, TokenSource, Transport};
use sync_gmail_api::{Batch, ChangeSink, GmailApi, Parts};

pub const TOKEN: &str = "fixture-token-not-real";
pub const T0: i64 = 1_793_700_000_000;

pub struct ClockInner {
    now: AtomicI64,
    slept: AtomicI64,
}
#[derive(Clone)]
pub struct Clock(pub Arc<ClockInner>);
impl Clock {
    pub fn new(start: i64) -> Clock {
        Clock(Arc::new(ClockInner { now: AtomicI64::new(start), slept: AtomicI64::new(0) }))
    }
    pub fn now(&self) -> i64 {
        self.0.now.load(Ordering::SeqCst)
    }
    pub fn advance(&self, ms: i64) {
        self.0.now.fetch_add(ms, Ordering::SeqCst);
    }
    pub fn slept(&self) -> i64 {
        self.0.slept.load(Ordering::SeqCst)
    }
}
impl Pacer for Clock {
    fn now_ms(&self) -> i64 {
        self.now()
    }
    fn sleep_ms(&self, ms: i64) {
        self.0.slept.fetch_add(ms, Ordering::SeqCst);
        self.advance(ms);
    }
}

struct Exchange {
    method: Method,
    path: String,
    query: Vec<(String, String)>,
    status: u16,
    body: String,
    retry_after_ms: Option<i64>,
    repeat: bool,
    used: bool,
}

pub struct Logged {
    pub at_ms: i64,
    pub request: Request,
}

struct Inner {
    exchanges: Mutex<Vec<Exchange>>,
    log: Mutex<Vec<Logged>>,
    clock: Clock,
}

#[derive(Clone)]
pub struct Replay(Arc<Inner>);

impl Replay {
    pub fn new(clock: &Clock) -> Replay {
        Replay(Arc::new(Inner { exchanges: Mutex::new(vec![]), log: Mutex::new(vec![]), clock: clock.clone() }))
    }

    pub fn from_files(clock: &Clock, files: &[&str]) -> Replay {
        let r = Replay::new(clock);
        for f in files {
            let path = format!("{}/fixtures/{}", env!("CARGO_MANIFEST_DIR"), f);
            let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("fixture {path}: {e}"));
            let doc: Value = serde_json::from_str(&text).unwrap_or_else(|e| panic!("fixture {path}: {e}"));
            for ex in doc["exchanges"].as_array().expect("exchanges") {
                r.push(ex.clone());
            }
        }
        r
    }

    /// One exchange: {method, path, query:[[k,v]], status, body, retry_after_ms?, repeat?}
    pub fn push(&self, ex: Value) {
        let query = ex["query"]
            .as_array()
            .map(|a| a.iter().map(|p| (p[0].as_str().unwrap().to_string(), p[1].as_str().unwrap().to_string())).collect())
            .unwrap_or_default();
        self.0.exchanges.lock().unwrap().push(Exchange {
            method: if ex["method"] == "POST" { Method::Post } else { Method::Get },
            path: ex["path"].as_str().unwrap().to_string(),
            query,
            status: ex["status"].as_u64().unwrap_or(200) as u16,
            body: ex["body"].to_string(),
            retry_after_ms: ex["retry_after_ms"].as_i64(),
            repeat: ex["repeat"].as_bool().unwrap_or(false),
            used: false,
        });
    }

    pub fn paths(&self) -> Vec<String> {
        self.0.log.lock().unwrap().iter().map(|l| l.request.path.clone()).collect()
    }
    pub fn requests(&self) -> Vec<Logged> {
        std::mem::take(&mut *self.0.log.lock().unwrap())
    }
    pub fn count(&self) -> usize {
        self.0.log.lock().unwrap().len()
    }
}

impl Transport for Replay {
    fn send(&self, request: &Request) -> Result<Response, String> {
        assert_eq!(request.bearer, TOKEN, "every request carries the token source's token");
        self.0.log.lock().unwrap().push(Logged { at_ms: self.0.clock.now(), request: request.clone() });
        let mut exchanges = self.0.exchanges.lock().unwrap();
        let hit = exchanges.iter_mut().find(|e| {
            (!e.used || e.repeat)
                && e.method == request.method
                && e.path == request.path
                && e.query.iter().all(|q| request.query.contains(q))
        });
        match hit {
            Some(e) => {
                e.used = true;
                Ok(Response { status: e.status, body: e.body.clone(), retry_after_ms: e.retry_after_ms })
            }
            None => Err(format!("no recorded exchange left for {:?} {} {:?}", request.method, request.path, request.query)),
        }
    }
}

pub struct FixedToken;
impl TokenSource for FixedToken {
    fn bearer(&self) -> Result<String, String> {
        Ok(TOKEN.to_string())
    }
}

pub struct Flags(pub &'static [&'static str]);
impl FeatureFlags for Flags {
    fn is_on(&self, name: &str) -> bool {
        self.0.contains(&name)
    }
}

pub fn open(replay: &Replay, clock: &Clock) -> GmailApi {
    let (r, c) = (replay.clone(), clock.clone());
    GmailApi::open(&Flags(&["gmail_api_verified"]), move || Parts {
        transport: Box::new(r),
        tokens: Box::new(FixedToken),
        pacer: Box::new(c),
        account_name: "Fixture account".to_string(),
    })
    .expect("flag is on")
}

#[derive(Default)]
pub struct VecSink {
    pub batches: Vec<Batch>,
}
impl ChangeSink for VecSink {
    fn accept(&mut self, batch: Batch) -> Result<(), String> {
        self.batches.push(batch);
        Ok(())
    }
}

pub struct FailingSink;
impl ChangeSink for FailingSink {
    fn accept(&mut self, _batch: Batch) -> Result<(), String> {
        Err("disk full".to_string())
    }
}

/// The quota cost of a logged request, from the ledger numbers (independent of the adapter's own bookkeeping).
pub fn cost_of(r: &Request) -> u64 {
    match r.path.as_str() {
        "history" => GMAIL_COST_HISTORY_LIST,
        "messages" => GMAIL_COST_MESSAGES_LIST,
        "watch" => GMAIL_COST_WATCH,
        "labels" => GMAIL_COST_LABELS_LIST,
        "profile" => GMAIL_COST_PROFILE,
        p if p.starts_with("messages/") => GMAIL_COST_MESSAGES_GET_RAW,
        p => panic!("no cost known for {p}"),
    }
}

/// Peak units in any sliding minute over the log, measured on the request timestamps alone.
pub fn peak_units_per_minute(log: &[Logged]) -> u64 {
    log.iter()
        .map(|l| log.iter().filter(|o| o.at_ms <= l.at_ms && o.at_ms > l.at_ms - 60_000).map(|o| cost_of(&o.request)).sum())
        .max()
        .unwrap_or(0)
}

/// Reads one number from docs/CONSTANTS.md independently of build.rs.
pub fn ledger_number(name: &str) -> u64 {
    let path = format!("{}/../../../docs/CONSTANTS.md", env!("CARGO_MANIFEST_DIR"));
    let text = std::fs::read_to_string(path).expect("ledger");
    let line = text.lines().find(|l| l.starts_with(&format!("| {name} |"))).unwrap_or_else(|| panic!("no ledger row {name}"));
    let cell = line.split('|').nth(2).unwrap().trim();
    cell.chars().take_while(|c| c.is_ascii_digit() || *c == ',').filter(|c| c.is_ascii_digit()).collect::<String>().parse().unwrap()
}
