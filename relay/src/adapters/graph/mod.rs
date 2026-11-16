//! Microsoft Graph change notifications. A rich subscription carries `encryptedContent` sealed to
//! a certificate only the device holds; the relay forwards it untouched and cannot read it.
//!
//! POST /v1/inbound/graph takes two kinds of request:
//!  * a Graph delivery (`{"value": [notification, ...]}`), proved by the `clientState` the device
//!    chose when it created the subscription (the relay keeps only a keyed hash of it);
//!  * a bind from the device (`{"bind": {"routing_id","subscription_id","client_state",
//!    "expires_at","rich"}}`) sent after the DEVICE created or renewed the subscription.
//! Renewing a subscription needs the mailbox credential, so the relay's job only tells the device
//! (signal `watch-renew:graph`) when renewal is due, before the subscription ends.
#[path = "../ledger.rs"]
pub mod ledger;

use crate::registry::adapter::{AdapterError, InboundRequest, MailSignal, ProviderAdapter, Reply};
use crate::registry::Dispatcher;
use ledger::{mac, Binding, Clock, Ledger, SystemClock};
use serde_json::{json, Value};
use std::sync::Arc;
use std::time::Duration;

/// Mirror docs/CONSTANTS.md GRAPH_SUB_RENEW_MIN: a subscription lasts under 10,080 minutes,
/// a rich one under 1,440. A test reads the ledger row and compares.
pub const GRAPH_SUB_MAX_MIN: i64 = 10_080;
pub const GRAPH_SUB_RICH_MAX_MIN: i64 = 1_440;
/// Minutes before expiry at which the device is told to renew: read from docs/CONSTANTS.md
/// RELAY_GRAPH_RENEW_MARGIN_MIN (generated), not declared here.
pub const RENEW_MARGIN_MIN: i64 = crate::registry::constants::RELAY_GRAPH_RENEW_MARGIN_MIN as i64;
/// How often the renew job looks: RELAY_RENEW_TICK_S, shared with the Gmail job.
pub const RENEW_TICK_S: u64 = crate::registry::constants::RELAY_RENEW_TICK_S;
/// Largest `ref` the relay puts in one APNs payload: RELAY_APNS_REF_MAX_B, which the APNs sender
/// enforces too (a test measures the whole body).
pub const REF_MAX_BYTES: usize = crate::registry::constants::RELAY_APNS_REF_MAX_B as usize;

#[derive(Clone)]
pub struct GraphConfig {
    pub renew_margin_min: i64,
    pub tick: Duration,
}

impl GraphConfig {
    pub fn from_env() -> GraphConfig {
        let margin = std::env::var("RELAY_GRAPH_RENEW_MARGIN_MIN").ok().and_then(|v| v.trim().parse().ok()).unwrap_or(RENEW_MARGIN_MIN);
        GraphConfig { renew_margin_min: margin, tick: Duration::from_secs(RENEW_TICK_S) }
    }
}

pub struct GraphAdapter {
    config: GraphConfig,
    ledger: Arc<Ledger>,
    clock: Arc<dyn Clock>,
    key: Vec<u8>,
}

pub fn adapter() -> Arc<dyn ProviderAdapter> {
    let ledger = Arc::new(Ledger::open("graph", &ledger::ledger_path("graph")));
    Arc::new(GraphAdapter::new(GraphConfig::from_env(), ledger, Arc::new(SystemClock), ledger::binding_key()))
}

fn bad(m: &str) -> AdapterError {
    AdapterError::Malformed(m.into())
}

fn same(a: &str, b: &str) -> bool {
    a.len() == b.len() && a.bytes().zip(b.bytes()).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

/// Graph proves a new notification URL by POSTing `?validationToken=<t>` and expecting `<t>` back
/// as text/plain within seconds. `ProviderAdapter::reply` below sends it.
pub fn validation_reply(query: &str) -> Option<String> {
    let raw = query.trim_start_matches('?').split('&').find_map(|kv| kv.strip_prefix("validationToken="))?;
    let bytes = raw.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'+' => out.push(b' '),
            b'%' if i + 2 < bytes.len() => {
                let h = std::str::from_utf8(&bytes[i + 1..i + 3]).ok().and_then(|h| u8::from_str_radix(h, 16).ok())?;
                out.push(h);
                i += 2;
            }
            b => out.push(b),
        }
        i += 1;
    }
    String::from_utf8(out).ok().filter(|t| !t.is_empty())
}

impl GraphAdapter {
    pub fn new(config: GraphConfig, ledger: Arc<Ledger>, clock: Arc<dyn Clock>, key: Vec<u8>) -> GraphAdapter {
        GraphAdapter { config, ledger, clock, key }
    }

    #[allow(dead_code)] // called by the test crates and kept next to the loop it mirrors
    /// One pass of the renew job now (the loop in `start` calls the same function).
    pub fn renew_now(&self, dispatcher: &Dispatcher) -> usize {
        ledger::renew_pass(&self.ledger, self.clock.as_ref(), dispatcher, "graph", &next_due)
    }

    fn bind(&self, b: &Value) -> Result<(), AdapterError> {
        let text = |k: &str| b.get(k).and_then(Value::as_str).filter(|v| !v.is_empty());
        let routing_id = text("routing_id").filter(|r| crate::routes::valid_routing_id(r)).ok_or_else(|| bad("bind needs a valid routing_id"))?;
        let sub = text("subscription_id").ok_or_else(|| bad("bind needs subscription_id"))?;
        let state = text("client_state").ok_or_else(|| bad("bind needs client_state"))?;
        let rich = b.get("rich").and_then(Value::as_bool).unwrap_or(false);
        let now = self.clock.now();
        let expires_at = b.get("expires_at").and_then(Value::as_i64).filter(|e| *e > now).ok_or_else(|| bad("bind needs expires_at (unix seconds) in the future"))?;
        let ceiling_min = if rich { GRAPH_SUB_RICH_MAX_MIN } else { GRAPH_SUB_MAX_MIN };
        if expires_at - now > ceiling_min * 60 {
            return Err(bad("expires_at is further out than Graph grants for this kind of subscription"));
        }
        let due_at = (expires_at - self.config.renew_margin_min * 60).max(now);
        let binding = Binding { key_id: mac(&self.key, sub), routing_id: routing_id.into(), expires_at: Some(expires_at), due_at: Some(due_at), proof: Some(mac(&self.key, state)), flags: rich as i64 };
        Ok(self.ledger.bind(&binding, now)?)
    }

    fn unbind(&self, b: &Value) -> Result<(), AdapterError> {
        let routing_id = b.get("routing_id").and_then(Value::as_str).ok_or_else(|| bad("unbind needs routing_id"))?;
        let sub = b.get("subscription_id").and_then(Value::as_str).ok_or_else(|| bad("unbind needs subscription_id"))?;
        self.ledger.unbind(&mac(&self.key, sub), routing_id).map_err(|e| {
            tracing::error!("graph unbind: {e}");
            bad("the relay could not release this binding")
        })
    }

    fn one(&self, n: &Value) -> Result<Option<MailSignal>, AdapterError> {
        let sub = n.get("subscriptionId").and_then(Value::as_str).ok_or_else(|| bad("notification has no subscriptionId"))?;
        let key_id = mac(&self.key, sub);
        let found = self.ledger.get(&key_id).map_err(|e| {
            tracing::error!("graph lookup: {e}");
            bad("the relay could not read its bindings")
        })?;
        let Some(binding) = found else { return Ok(None) };
        let presented = n.get("clientState").and_then(Value::as_str).map(|s| mac(&self.key, s)).unwrap_or_default();
        if !same(&presented, binding.proof.as_deref().unwrap_or("-")) {
            return Err(AdapterError::Unauthorized("clientState does not match this subscription".into()));
        }
        if let Some(event) = n.get("lifecycleEvent").and_then(Value::as_str) {
            let stamp = n.get("subscriptionExpirationDateTime").and_then(Value::as_str).unwrap_or("-");
            return Ok(Some(MailSignal { signal_id: format!("graph-life:{key_id}:{event}:{stamp}"), routing_id: binding.routing_id, opaque_ref: Some("watch-renew:graph".into()) }));
        }
        if n.get("changeType").and_then(Value::as_str) != Some("created") {
            return Ok(None);
        }
        let message_id = n
            .pointer("/resourceData/id")
            .and_then(Value::as_str)
            .or_else(|| n.get("resource").and_then(Value::as_str).and_then(|r| r.rsplit('/').next()))
            .filter(|m| !m.is_empty())
            .ok_or_else(|| bad("notification names no message"))?;
        Ok(Some(MailSignal { signal_id: format!("graph:{key_id}:{message_id}"), routing_id: binding.routing_id, opaque_ref: Some(reference(n, message_id)) }))
    }
}

/// A rich notification travels to the phone still encrypted: the five strings Graph sent are
/// copied, never decoded. If they would not fit one APNs payload, the phone gets the message id
/// and fetches the mail itself.
fn reference(n: &Value, message_id: &str) -> String {
    let Some(enc) = n.get("encryptedContent").and_then(Value::as_object) else { return message_id.to_string() };
    let field = |k: &str| enc.get(k).and_then(Value::as_str).map(str::to_owned);
    let (Some(data), Some(key), Some(sig)) = (field("data"), field("dataKey"), field("dataSignature")) else { return message_id.to_string() };
    let sealed = json!({ "id": message_id, "encryptedContent": {
        "data": data, "dataKey": key, "dataSignature": sig,
        "encryptionCertificateId": field("encryptionCertificateId"), "encryptionCertificateThumbprint": field("encryptionCertificateThumbprint") } })
    .to_string();
    if sealed.len() <= REF_MAX_BYTES {
        sealed
    } else {
        tracing::info!("graph: encrypted content is {} bytes, over the {} byte payload cap; sending the message id only", sealed.len(), REF_MAX_BYTES);
        message_id.to_string()
    }
}

fn next_due(b: &Binding, now: i64) -> i64 {
    // Ask again halfway to the end if the device has not renewed.
    (now + (b.expires_at.unwrap_or(now) - now) / 2).max(now + 1)
}

impl ProviderAdapter for GraphAdapter {
    fn family(&self) -> &'static str {
        "graph"
    }

    fn reply(&self, request: &InboundRequest) -> Option<Reply> {
        validation_reply(request.query()).map(Reply::text)
    }

    fn decode(&self, request: &InboundRequest) -> Result<Vec<MailSignal>, AdapterError> {
        let body: Value = serde_json::from_slice(&request.body).map_err(|e| bad(&e.to_string()))?;
        if let Some(b) = body.get("bind") {
            self.bind(b)?;
            return Ok(vec![]);
        }
        if let Some(b) = body.get("unbind") {
            self.unbind(b)?;
            return Ok(vec![]);
        }
        let items = body.get("value").and_then(Value::as_array).ok_or_else(|| bad("not a Graph delivery: value[] is missing"))?;
        let mut signals = Vec::new();
        for n in items {
            if let Some(s) = self.one(n)? {
                signals.push(s);
            }
        }
        Ok(signals)
    }

    fn start(&self, dispatcher: Arc<Dispatcher>) {
        ledger::spawn_renew_loop(self.ledger.clone(), self.clock.clone(), dispatcher, "graph", self.config.tick, next_due);
    }
}
