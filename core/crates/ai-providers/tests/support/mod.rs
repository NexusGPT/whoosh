//! Replay transport over the invented fixtures under fixtures/. No socket is ever opened.
#![allow(dead_code)]
use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use ai_providers::transport::{Body, Method, Request, Response, Transport};
use contracts::KeychainRef;
use secrets::{MemoryKeychain, Namespace, Secret, SecretKind, StoreSecret};
use serde_json::Value;

pub fn fixtures() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("fixtures")
}

pub struct Fixture {
    pub doc: Value,
}

impl Fixture {
    pub fn load(kind: &str, slug: &str) -> Fixture {
        let path = fixtures().join(kind).join(format!("{slug}.json"));
        let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("fixture {} missing: {e}", path.display()));
        Fixture { doc: serde_json::from_str(&text).expect("fixture is JSON") }
    }
    pub fn key(&self) -> String {
        self.doc["key"].as_str().expect("fixture has a key").to_string()
    }
    pub fn bad_key(&self) -> String {
        self.doc["bad_key"].as_str().expect("fixture has a bad_key").to_string()
    }
    pub fn replay(&self, scenario: &str) -> Arc<Replay> {
        let steps = self.doc["scenarios"][scenario].as_array().unwrap_or_else(|| panic!("no scenario {scenario}"));
        Arc::new(Replay { steps: Mutex::new(steps.iter().cloned().collect()), seen: Mutex::new(Vec::new()), mismatches: Mutex::new(Vec::new()) })
    }
}

pub struct Replay {
    steps: Mutex<VecDeque<Value>>,
    seen: Mutex<Vec<Request>>,
    mismatches: Mutex<Vec<String>>,
}

impl Replay {
    pub fn seen(&self) -> Vec<Request> {
        self.seen.lock().unwrap().clone()
    }
    /// Every step was used and every request matched what the provider documents.
    pub fn assert_done(&self) {
        assert_eq!(*self.mismatches.lock().unwrap(), Vec::<String>::new(), "requests that did not match the fixture");
        assert!(self.steps.lock().unwrap().is_empty(), "fixture steps never requested");
    }
    fn fail(&self, why: String) -> Result<Response, String> {
        self.mismatches.lock().unwrap().push(why.clone());
        Err(format!("fixture mismatch: {why}"))
    }
}

impl Transport for Replay {
    fn send(&self, r: &Request) -> Result<Response, String> {
        self.seen.lock().unwrap().push(r.clone());
        let Some(step) = self.steps.lock().unwrap().pop_front() else { return self.fail(format!("unexpected request {} {}", format!("{:?}", r.method), r.url)) };
        let e = &step["expect"];
        let method = if e["method"] == "GET" { Method::Get } else { Method::Post };
        if r.method != method || e["url"] != r.url.as_str() {
            return self.fail(format!("expected {} {} but got {:?} {}", e["method"], e["url"], r.method, r.url));
        }
        for (name, value) in e["headers"].as_object().into_iter().flatten() {
            let found = r.headers.iter().find(|(k, _)| k.eq_ignore_ascii_case(name)).map(|(_, v)| v.as_str());
            if found != value.as_str() {
                return self.fail(format!("header {name}: expected {value}, got {found:?}"));
            }
        }
        for name in e["absent"].as_array().into_iter().flatten().filter_map(Value::as_str) {
            if r.headers.iter().any(|(k, _)| k.eq_ignore_ascii_case(name)) {
                return self.fail(format!("header {name} must not be sent"));
            }
        }
        let (content_type, bytes): (Option<&str>, Vec<u8>) = match &r.body {
            Body::None => (None, Vec::new()),
            Body::Json(j) => (Some("application/json"), j.clone().into_bytes()),
            Body::Bytes { content_type, data } => (Some(content_type.as_str()), data.clone()),
        };
        if let Some(prefix) = e["content_type_starts"].as_str() {
            if !content_type.is_some_and(|c| c.starts_with(prefix)) {
                return self.fail(format!("content type {content_type:?} does not start with {prefix}"));
            }
        }
        let body_text = String::from_utf8_lossy(&bytes).to_string();
        for needle in e["body_contains"].as_array().into_iter().flatten().filter_map(Value::as_str) {
            if !body_text.contains(needle) {
                return self.fail(format!("body lacks {needle}"));
            }
        }
        let resp = &step["response"];
        let body = match resp.get("body_text") {
            Some(t) => t.as_str().unwrap_or_default().to_string(),
            None => resp["body"].to_string(),
        };
        Ok(Response { status: resp["status"].as_u64().expect("status") as u16, body })
    }
}

/// A transport with no network at all: every call fails the way a closed port does.
pub struct Closed(pub &'static str);
impl Transport for Closed {
    fn send(&self, _r: &Request) -> Result<Response, String> {
        Err(self.0.to_string())
    }
}

pub struct Vault {
    pub store: Arc<MemoryKeychain>,
}

impl Vault {
    pub fn new(tag: &str) -> Vault {
        Vault { store: Arc::new(MemoryKeychain::new(Namespace::rig_run(tag).expect("rig namespace"))) }
    }
    pub fn put(&self, name: &str, secret: &str) -> KeychainRef {
        StoreSecret::execute(&*self.store, SecretKind::ProviderCredential, name, &Secret::new(secret.as_bytes())).expect("store fixture key")
    }
}
