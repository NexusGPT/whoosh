//! Gmail through Pub/Sub. The relay sees an address and an opaque history id and never a token.
//!
//! Two kinds of request arrive on POST /v1/inbound/gmail:
//!  * a Pub/Sub push (`{"message": {"data": base64({"emailAddress","historyId"})}}`), proved by the
//!    Google-signed OIDC bearer token Pub/Sub attaches to every authenticated push;
//!  * a bind from the device (`{"bind": {"routing_id","email_address","expires_at"}}`) sent after
//!    the DEVICE created or renewed the Gmail watch with its own OAuth token; `{"unbind": {"routing_id",
//!    "email_address"}}` releases it. Only the routing id that owns a binding can replace or release it.
//! Renewal needs that token, so the relay's daily job cannot renew: it sends the device one
//! signal (`watch-renew:gmail`) when the watch is due, and the device renews and binds again.
#[path = "../ledger.rs"]
pub mod ledger;

use crate::registry::adapter::{AdapterError, InboundRequest, MailSignal, ProviderAdapter};
use crate::registry::Dispatcher;
use base64::Engine;
use jsonwebtoken::{decode, decode_header, jwk::JwkSet, Algorithm, DecodingKey, Validation};
use ledger::{mac, Binding, Clock, Ledger, SystemClock};
use serde::Deserialize;
use serde_json::Value;
use std::sync::Arc;
use std::time::Duration;

/// Mirrors docs/CONSTANTS.md GMAIL_WATCH_RENEW_H (a test reads the ledger row and compares).
pub const GMAIL_WATCH_RENEW_H: i64 = 24;
/// How often the renew job looks: read from docs/CONSTANTS.md RELAY_RENEW_TICK_S (generated).
pub const RENEW_TICK_S: u64 = crate::registry::constants::RELAY_RENEW_TICK_S;
const ISSUERS: [&str; 2] = ["https://accounts.google.com", "accounts.google.com"];

#[derive(Clone)]
pub struct GmailConfig {
    /// File holding Google's JWKS, refreshed out of band; read on every push so rotation needs no restart.
    pub jwks_path: Option<String>,
    /// The audience set on the Pub/Sub push subscription.
    pub audience: Option<String>,
    /// The service account the push subscription signs as.
    pub push_account: Option<String>,
    pub tick: Duration,
}

impl GmailConfig {
    pub fn from_env() -> GmailConfig {
        let get = |n: &str| std::env::var(n).ok().filter(|v| !v.trim().is_empty());
        GmailConfig { jwks_path: get("RELAY_GMAIL_JWKS_PATH"), audience: get("RELAY_GMAIL_AUDIENCE"), push_account: get("RELAY_GMAIL_PUSH_ACCOUNT"), tick: Duration::from_secs(RENEW_TICK_S) }
    }
}

pub struct GmailAdapter {
    config: GmailConfig,
    ledger: Arc<Ledger>,
    clock: Arc<dyn Clock>,
    key: Vec<u8>,
}

pub fn adapter() -> Arc<dyn ProviderAdapter> {
    let ledger = Arc::new(Ledger::open("gmail", &ledger::ledger_path("gmail")));
    Arc::new(GmailAdapter::new(GmailConfig::from_env(), ledger, Arc::new(SystemClock), ledger::binding_key()))
}

#[derive(Deserialize)]
struct Claims {
    email: Option<String>,
    email_verified: Option<bool>,
}

fn bad(m: &str) -> AdapterError {
    AdapterError::Malformed(m.into())
}

impl GmailAdapter {
    pub fn new(config: GmailConfig, ledger: Arc<Ledger>, clock: Arc<dyn Clock>, key: Vec<u8>) -> GmailAdapter {
        GmailAdapter { config, ledger, clock, key }
    }

    #[allow(dead_code)] // called by the test crates and kept next to the loop it mirrors
    /// One pass of the daily job now (the loop in `start` calls the same function).
    pub fn renew_now(&self, dispatcher: &Dispatcher) -> usize {
        ledger::renew_pass(&self.ledger, self.clock.as_ref(), dispatcher, "gmail", &next_due)
    }

    fn verify(&self, request: &InboundRequest) -> Result<(), AdapterError> {
        let no = |m: &str| AdapterError::Unauthorized(m.into());
        let (Some(path), Some(audience), Some(account)) = (&self.config.jwks_path, &self.config.audience, &self.config.push_account) else {
            return Err(no("gmail push verification is not configured"));
        };
        let token = request
            .headers
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case("authorization"))
            .and_then(|(_, v)| v.strip_prefix("Bearer "))
            .ok_or_else(|| no("missing bearer token"))?;
        let jwks: JwkSet = std::fs::read_to_string(path).map_err(|e| no(&format!("no verification keys: {e}"))).and_then(|t| serde_json::from_str(&t).map_err(|e| no(&format!("verification keys unreadable: {e}"))))?;
        let kid = decode_header(token).map_err(|e| no(&format!("bad token header: {e}")))?.kid.ok_or_else(|| no("token has no key id"))?;
        let jwk = jwks.find(&kid).ok_or_else(|| no("token key id is not in the key set"))?;
        let key = DecodingKey::from_jwk(jwk).map_err(|e| no(&format!("unusable key: {e}")))?;
        let mut validation = Validation::new(Algorithm::RS256);
        validation.set_audience(&[audience]);
        validation.set_issuer(&ISSUERS);
        let claims = decode::<Claims>(token, &key, &validation).map_err(|e| no(&format!("token refused: {e}")))?.claims;
        if claims.email.as_deref() != Some(account.as_str()) || claims.email_verified != Some(true) {
            return Err(no("token was not issued for the push service account"));
        }
        Ok(())
    }

    fn bind(&self, b: &Value) -> Result<(), AdapterError> {
        let routing_id = b.get("routing_id").and_then(Value::as_str).filter(|r| crate::routes::valid_routing_id(r)).ok_or_else(|| bad("bind needs a valid routing_id"))?;
        let address = b.get("email_address").and_then(Value::as_str).map(str::trim).filter(|a| a.contains('@') && a.len() <= 320).ok_or_else(|| bad("bind needs email_address"))?;
        let now = self.clock.now();
        let expires_at = b.get("expires_at").and_then(Value::as_i64).filter(|e| *e > now + 1).ok_or_else(|| bad("bind needs expires_at (unix seconds) in the future"))?;
        let due_at = (now + GMAIL_WATCH_RENEW_H * 3600).min(expires_at - 1);
        let binding = Binding { key_id: mac(&self.key, &address.to_lowercase()), routing_id: routing_id.into(), expires_at: Some(expires_at), due_at: Some(due_at), proof: None, flags: 0 };
        Ok(self.ledger.bind(&binding, now)?)
    }

    fn unbind(&self, b: &Value) -> Result<(), AdapterError> {
        let routing_id = b.get("routing_id").and_then(Value::as_str).ok_or_else(|| bad("unbind needs routing_id"))?;
        let address = b.get("email_address").and_then(Value::as_str).ok_or_else(|| bad("unbind needs email_address"))?;
        self.ledger.unbind(&mac(&self.key, &address.trim().to_lowercase()), routing_id).map_err(|e| {
            tracing::error!("gmail unbind: {e}");
            bad("the relay could not release this binding")
        })
    }
}

fn next_due(b: &Binding, _now: i64) -> i64 {
    b.due_at.unwrap_or(0) + GMAIL_WATCH_RENEW_H * 3600
}

impl ProviderAdapter for GmailAdapter {
    fn family(&self) -> &'static str {
        "gmail"
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
        self.verify(request)?;
        let data = body.pointer("/message/data").and_then(Value::as_str).ok_or_else(|| bad("not a Pub/Sub push: message.data is missing"))?;
        let raw = base64::engine::general_purpose::STANDARD.decode(data).map_err(|e| bad(&format!("message.data is not base64: {e}")))?;
        let note: Value = serde_json::from_slice(&raw).map_err(|e| bad(&format!("message.data is not a Gmail notification: {e}")))?;
        let address = note.get("emailAddress").and_then(Value::as_str).ok_or_else(|| bad("notification has no emailAddress"))?;
        let history = match note.get("historyId") {
            Some(Value::Number(n)) => n.to_string(),
            Some(Value::String(s)) if !s.is_empty() => s.clone(),
            _ => return Err(bad("notification has no historyId")),
        };
        let key_id = mac(&self.key, &address.to_lowercase());
        let binding = self.ledger.get(&key_id).map_err(|e| {
            tracing::error!("gmail lookup: {e}");
            bad("the relay could not read its bindings")
        })?;
        // An unbound address is acknowledged with no signal, so Pub/Sub stops redelivering it.
        Ok(binding.map(|b| vec![MailSignal { signal_id: format!("gmail:{key_id}:{history}"), routing_id: b.routing_id, opaque_ref: Some(history) }]).unwrap_or_default())
    }

    fn start(&self, dispatcher: Arc<Dispatcher>) {
        ledger::spawn_renew_loop(self.ledger.clone(), self.clock.clone(), dispatcher, "gmail", self.config.tick, next_due);
    }
}
