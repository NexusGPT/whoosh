//! Test rig shared by relay/tests/{gmail,graph,jmap,macsource}.rs (each includes this file with
//! `#[path]`; it is not part of the binary). The test crate compiles the relay's own registry,
//! routes, apns and tracking sources and the adapter under test, serves the real router on a
//! loopback port, runs the real APNs worker, and points it at relay-core's APNs stub process.
//! One stub process per rig, killed when the rig drops. Only the clock is fake. Needs `mod common` (tests/relay/common.rs) at the test crate root.
use crate::apns::ApnsState;
use crate::common::{start_stub, work_dir, Stub, FIXTURE_TOKEN};
use crate::registry::adapter::{AdapterSet, ProviderAdapter};
use crate::registry::devices::{self, Environment};
use crate::registry::{Dispatcher, Store};
use crate::routes::{router, AppState};
use crate::tracking::Signer;
use serde_json::Value;
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

pub const OTHER_TOKEN: &str = "abcdef0123456789";

pub struct FakeClock(pub AtomicI64);

impl FakeClock {
    pub fn at(t: i64) -> Arc<FakeClock> {
        Arc::new(FakeClock(AtomicI64::new(t)))
    }
    pub fn now(&self) -> i64 {
        self.0.load(Ordering::SeqCst)
    }
    pub fn advance(&self, secs: i64) {
        self.0.fetch_add(secs, Ordering::SeqCst);
    }
}

static ENV_LOCK: Mutex<()> = Mutex::new(());

/// ApnsState can only be built from RELAY_APNS_* environment variables, so the variables are set
/// and read under one lock: concurrent tests never see each other's stub address.
fn apns_for(stub: &Stub) -> ApnsState {
    let _guard = ENV_LOCK.lock().unwrap_or_else(|p| p.into_inner());
    std::env::set_var("RELAY_APNS_KEY_PATH", &stub.key_path);
    std::env::set_var("RELAY_APNS_KEY_ID", "TESTKEYID01");
    std::env::set_var("RELAY_APNS_TEAM_ID", "TESTTEAM01");
    std::env::set_var("RELAY_APNS_TOPIC", "app.whoosh.test");
    std::env::set_var("RELAY_APNS_SANDBOX_URL", &stub.url);
    std::env::set_var("RELAY_APNS_PRODUCTION_URL", &stub.url);
    ApnsState::from_env()
}

pub struct Rig {
    pub base: String,
    pub store: Store,
    pub dispatcher: Arc<Dispatcher>,
    pub http: reqwest::Client,
    log: std::path::PathBuf,
    _stub: Stub,
}

/// Builds the relay around `adapters` (their `start` runs, as at boot) and returns the rig.
pub async fn rig(adapters: Vec<Arc<dyn ProviderAdapter>>) -> Rig {
    let dir = work_dir(&format!("adapter-{}", rand::random::<u32>()));
    let stub = start_stub(&dir);
    let apns = apns_for(&stub);
    let log = stub.log.clone();
    let store = Store::open(dir.join("relay.sqlite3").to_str().unwrap()).expect("store");
    let dispatcher = Arc::new(Dispatcher::new(store.clone()));
    let set = Arc::new(AdapterSet::new(adapters));
    set.start_all(&dispatcher);
    let state = AppState {
        store: store.clone(),
        dispatcher: dispatcher.clone(),
        adapters: set,
        apns: Arc::new(apns),
        signer: Arc::new(Signer::new(None)),
        public_base: Arc::new("http://relay.test".into()),
    };
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let base = format!("http://{}", listener.local_addr().unwrap());
    tokio::spawn(crate::apns::worker::run(state.clone()));
    tokio::spawn(async move { axum::serve(listener, router(state)).await });
    Rig { base, store, dispatcher, http: reqwest::Client::new(), log, _stub: stub }
}

impl Rig {
    /// Registers a device the way POST /v1/registrations does.
    pub fn register(&self, token: &str, routing_id: &str) {
        devices::register(&self.store, token, routing_id, Environment::Sandbox, 1).expect("register");
    }

    pub async fn post(&self, family: &str, headers: &[(&str, String)], body: Vec<u8>) -> (u16, Value) {
        let mut req = self.http.post(format!("{}/v1/inbound/{family}", self.base)).body(body);
        for (k, v) in headers {
            req = req.header(*k, v);
        }
        let r = req.send().await.expect("inbound POST");
        (r.status().as_u16(), r.json().await.unwrap_or(Value::Null))
    }

    /// Every request the APNs stub has seen for this routing id.
    pub fn alerts(&self, routing_id: &str) -> Vec<Value> {
        std::fs::read_to_string(&self.log)
            .unwrap_or_default()
            .lines()
            .filter_map(|l| serde_json::from_str::<Value>(l).ok())
            .filter(|l| l["body"]["whoosh"]["routing_id"] == routing_id)
            .collect()
    }

    pub async fn wait_alerts(&self, routing_id: &str, n: usize) -> Vec<Value> {
        let start = Instant::now();
        loop {
            let a = self.alerts(routing_id);
            if a.len() >= n {
                return a;
            }
            assert!(start.elapsed() < Duration::from_secs(20), "expected {n} alerts for {routing_id}, saw {}", a.len());
            tokio::time::sleep(Duration::from_millis(25)).await;
        }
    }

    /// Waits long enough for the worker to have drained the queue, then returns the alert count.
    pub async fn settle(&self, routing_id: &str) -> usize {
        for _ in 0..40 {
            let queued = self.store.with(|c| c.query_row("SELECT COUNT(1) FROM push_job WHERE state = 'queued'", [], |r| r.get::<_, i64>(0))).unwrap();
            if queued == 0 {
                break;
            }
            tokio::time::sleep(Duration::from_millis(25)).await;
        }
        tokio::time::sleep(Duration::from_millis(150)).await;
        self.alerts(routing_id).len()
    }
}

pub fn ok_token() -> &'static str {
    FIXTURE_TOKEN
}
