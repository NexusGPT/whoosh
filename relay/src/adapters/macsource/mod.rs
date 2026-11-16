//! The owner's Mac as the push source for iCloud and plain IMAP accounts while it is awake. The Mac
//! watches the mailbox itself and sends the relay a SIGNED event; the relay never sees a mailbox.
//!
//! POST /v1/inbound/macsource takes:
//!  * an event: `Authorization: Bearer <ES256 JWT>` signed with the Mac's private key; claims
//!    `sub` (routing id), `jti` (the signal id, dedup key), `exp`, optional `ref`; body
//!    `{"routing_id": ...}` only picks which paired key verifies it. Everything routed comes from
//!    the signed claims.
//!  * a pairing: `{"pair": {"routing_id","public_key_pem"}}`. The first key for a routing id is
//!    taken as sent (the routing id is the secret that makes the pairing); replacing it needs a
//!    token from the old key whose `pair` claim is the hex SHA-256 of the new key.
//! An unsigned or wrongly signed event is refused (401) and signals nothing.
#[path = "../ledger.rs"]
pub mod ledger;

use crate::registry::adapter::{AdapterError, InboundRequest, MailSignal, ProviderAdapter};
use jsonwebtoken::{decode, Algorithm, DecodingKey, Validation};
use ledger::{Binding, Clock, Ledger, SystemClock};
use serde::Deserialize;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::sync::Arc;

const MAX_PEM_BYTES: usize = 1024;

pub struct MacSourceAdapter {
    ledger: Arc<Ledger>,
    clock: Arc<dyn Clock>,
}

pub fn adapter() -> Arc<dyn ProviderAdapter> {
    Arc::new(MacSourceAdapter::new(Arc::new(Ledger::open("macsource", &ledger::ledger_path("macsource"))), Arc::new(SystemClock)))
}

#[derive(Deserialize)]
struct Claims {
    sub: String,
    jti: Option<String>,
    r#ref: Option<String>,
    pair: Option<String>,
}

fn bad(m: &str) -> AdapterError {
    AdapterError::Malformed(m.into())
}

fn no(m: &str) -> AdapterError {
    AdapterError::Unauthorized(m.into())
}

impl MacSourceAdapter {
    pub fn new(ledger: Arc<Ledger>, clock: Arc<dyn Clock>) -> MacSourceAdapter {
        MacSourceAdapter { ledger, clock }
    }

    /// Verifies the bearer token against the PEM paired for `routing_id`.
    fn verify(&self, request: &InboundRequest, routing_id: &str, pem: &str) -> Result<Claims, AdapterError> {
        let token = request
            .headers
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case("authorization"))
            .and_then(|(_, v)| v.strip_prefix("Bearer "))
            .ok_or_else(|| no("the event is not signed"))?;
        let key = DecodingKey::from_ec_pem(pem.as_bytes()).map_err(|e| no(&format!("the paired key is unusable: {e}")))?;
        let mut validation = Validation::new(Algorithm::ES256);
        validation.validate_aud = false;
        validation.set_required_spec_claims(&["exp", "sub"]);
        let claims = decode::<Claims>(token, &key, &validation).map_err(|e| no(&format!("signature refused: {e}")))?.claims;
        if claims.sub != routing_id {
            return Err(no("the signed routing id is not the one this event names"));
        }
        Ok(claims)
    }

    fn pair(&self, request: &InboundRequest, p: &Value) -> Result<(), AdapterError> {
        let routing_id = p.get("routing_id").and_then(Value::as_str).filter(|r| crate::routes::valid_routing_id(r)).ok_or_else(|| bad("pair needs a valid routing_id"))?;
        let pem = p.get("public_key_pem").and_then(Value::as_str).filter(|k| k.len() <= MAX_PEM_BYTES).ok_or_else(|| bad("pair needs public_key_pem"))?;
        DecodingKey::from_ec_pem(pem.as_bytes()).map_err(|e| bad(&format!("public_key_pem is not an ES256 public key: {e}")))?;
        let existing = self.ledger.get(routing_id).map_err(|e| {
            tracing::error!("macsource pairing lookup: {e}");
            bad("the relay could not read its pairings")
        })?;
        if let Some(old) = existing.as_ref().and_then(|b| b.proof.as_deref()) {
            if old != pem {
                let claims = self.verify(request, routing_id, old)?;
                if claims.pair.as_deref() != Some(hex::encode(Sha256::digest(pem.as_bytes())).as_str()) {
                    return Err(no("replacing a paired key needs the old key to sign the new one"));
                }
            }
        }
        let binding = Binding { key_id: routing_id.into(), routing_id: routing_id.into(), expires_at: None, due_at: None, proof: Some(pem.into()), flags: 0 };
        Ok(self.ledger.bind(&binding, self.clock.now())?)
    }
}

impl ProviderAdapter for MacSourceAdapter {
    fn family(&self) -> &'static str {
        "macsource"
    }

    fn decode(&self, request: &InboundRequest) -> Result<Vec<MailSignal>, AdapterError> {
        let body: Value = serde_json::from_slice(&request.body).map_err(|e| bad(&e.to_string()))?;
        if let Some(p) = body.get("pair") {
            self.pair(request, p)?;
            return Ok(vec![]);
        }
        let routing_id = body.get("routing_id").and_then(Value::as_str).ok_or_else(|| bad("the event names no routing_id"))?;
        let paired = self.ledger.get(routing_id).map_err(|e| {
            tracing::error!("macsource lookup: {e}");
            bad("the relay could not read its pairings")
        })?;
        // An unpaired routing id answers like a bad signature: no oracle for which ids exist.
        let pem = paired.and_then(|b| b.proof).ok_or_else(|| no("no Mac is paired for this routing id"))?;
        let claims = self.verify(request, routing_id, &pem)?;
        let signal_id = claims.jti.filter(|j| !j.is_empty()).ok_or_else(|| bad("the signed event has no jti"))?;
        Ok(vec![MailSignal { signal_id: format!("mac:{routing_id}:{signal_id}"), routing_id: claims.sub, opaque_ref: claims.r#ref }])
    }
}
