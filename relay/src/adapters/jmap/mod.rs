//! JMAP WebPush (RFC 8620 section 7.2) for Fastmail and other JMAP hosts. The JMAP server POSTs a
//! `StateChange` to the push URL; the relay turns an `EmailDelivery` change for a bound account
//! into one signal. `EmailDelivery` (RFC 8621) moves only when new mail arrives, so flag and
//! folder changes (the `Email` state) never alert.
//!
//! POST /v1/inbound/jmap takes a StateChange, a PushVerification (acknowledged, not routed) or a
//! bind from the device (`{"bind": {"routing_id","account_id","secret"?}}`) sent after the DEVICE
//! created the push subscription with its own credential. WebPush sends no custom header, so a
//! bind that carries a `secret` (16 to 128 characters of A-Z a-z 0-9 - _) is proved by the push
//! URL the device registered: `/v1/inbound/jmap?s=<secret>`. The relay keeps a keyed hash only.
//! A bind without a secret behaves as before (no proof).
#[path = "../ledger.rs"]
pub mod ledger;

use crate::registry::adapter::{AdapterError, InboundRequest, MailSignal, ProviderAdapter};
use ledger::{mac, Binding, Clock, Ledger, SystemClock};
use serde_json::Value;
use std::sync::Arc;

pub struct JmapAdapter {
    ledger: Arc<Ledger>,
    clock: Arc<dyn Clock>,
    key: Vec<u8>,
}

pub fn adapter() -> Arc<dyn ProviderAdapter> {
    Arc::new(JmapAdapter::new(Arc::new(Ledger::open("jmap", &ledger::ledger_path("jmap"))), Arc::new(SystemClock), ledger::binding_key()))
}

fn bad(m: &str) -> AdapterError {
    AdapterError::Malformed(m.into())
}

fn same(a: &str, b: &str) -> bool {
    a.len() == b.len() && a.bytes().zip(b.bytes()).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

/// The `s` value of the push URL's query, if any.
fn url_secret(request: &InboundRequest) -> Option<&str> {
    request.query().split('&').find_map(|kv| kv.strip_prefix("s="))
}

impl JmapAdapter {
    pub fn new(ledger: Arc<Ledger>, clock: Arc<dyn Clock>, key: Vec<u8>) -> JmapAdapter {
        JmapAdapter { ledger, clock, key }
    }

    fn bind(&self, b: &Value) -> Result<(), AdapterError> {
        let routing_id = b.get("routing_id").and_then(Value::as_str).filter(|r| crate::routes::valid_routing_id(r)).ok_or_else(|| bad("bind needs a valid routing_id"))?;
        let account = b.get("account_id").and_then(Value::as_str).filter(|a| !a.is_empty() && a.len() <= 255).ok_or_else(|| bad("bind needs account_id"))?;
        let secret = match b.get("secret") {
            None => None,
            Some(v) => Some(
                v.as_str()
                    .filter(|s| (16..=128).contains(&s.len()) && s.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'-' || c == b'_'))
                    .ok_or_else(|| bad("secret must be 16 to 128 characters of A-Z a-z 0-9 - _"))?,
            ),
        };
        let binding = Binding { key_id: mac(&self.key, account), routing_id: routing_id.into(), expires_at: None, due_at: None, proof: secret.map(|s| mac(&self.key, s)), flags: 0 };
        Ok(self.ledger.bind(&binding, self.clock.now())?)
    }

    fn unbind(&self, b: &Value) -> Result<(), AdapterError> {
        let routing_id = b.get("routing_id").and_then(Value::as_str).ok_or_else(|| bad("unbind needs routing_id"))?;
        let account = b.get("account_id").and_then(Value::as_str).ok_or_else(|| bad("unbind needs account_id"))?;
        self.ledger.unbind(&mac(&self.key, account), routing_id).map_err(|e| {
            tracing::error!("jmap unbind: {e}");
            bad("the relay could not release this binding")
        })
    }
}

impl ProviderAdapter for JmapAdapter {
    fn family(&self) -> &'static str {
        "jmap"
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
        match body.get("@type").and_then(Value::as_str) {
            Some("PushVerification") => return Ok(vec![]),
            Some("StateChange") => {}
            _ => return Err(bad("not a JMAP push: @type must be StateChange")),
        }
        let changed = body.get("changed").and_then(Value::as_object).ok_or_else(|| bad("StateChange has no changed object"))?;
        let mut signals = Vec::new();
        for (account, types) in changed {
            let Some(state) = types.get("EmailDelivery").and_then(Value::as_str).filter(|s| !s.is_empty()) else { continue };
            let key_id = mac(&self.key, account);
            let found = self.ledger.get(&key_id).map_err(|e| {
                tracing::error!("jmap lookup: {e}");
                bad("the relay could not read its bindings")
            })?;
            if let Some(binding) = found {
                if let Some(proof) = binding.proof.as_deref() {
                    let presented = url_secret(request).map(|s| mac(&self.key, s)).unwrap_or_default();
                    if !same(&presented, proof) {
                        return Err(AdapterError::Unauthorized("the push URL secret does not match this account".into()));
                    }
                }
                signals.push(MailSignal { signal_id: format!("jmap:{key_id}:{state}"), routing_id: binding.routing_id, opaque_ref: Some(state.to_string()) });
            }
        }
        Ok(signals)
    }
}
