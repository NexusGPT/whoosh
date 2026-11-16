//! The built-in trusted adapter: POST /v1/inbound/loopback with `Authorization: Bearer <token>`
//! and a JSON `MailSignal` (or an array of them). Used by a signed Mac source before its own
//! adapter lands, by operators, and by the check scripts. Absent unless RELAY_SIGNAL_TOKEN is set.
use super::adapter::{AdapterError, InboundRequest, MailSignal, ProviderAdapter};
use sha2::{Digest, Sha256};

pub struct Loopback {
    token: String,
}

impl Loopback {
    pub fn new(token: String) -> Loopback {
        Loopback { token }
    }
}

fn same(a: &str, b: &str) -> bool {
    Sha256::digest(a.as_bytes()) == Sha256::digest(b.as_bytes())
}

impl ProviderAdapter for Loopback {
    fn family(&self) -> &'static str {
        "loopback"
    }

    fn decode(&self, request: &InboundRequest) -> Result<Vec<MailSignal>, AdapterError> {
        let presented = request
            .headers
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case("authorization"))
            .and_then(|(_, v)| v.strip_prefix("Bearer "))
            .unwrap_or("");
        if !same(presented, &self.token) {
            return Err(AdapterError::Unauthorized("missing or wrong signal token".into()));
        }
        let value: serde_json::Value = serde_json::from_slice(&request.body).map_err(|e| AdapterError::Malformed(e.to_string()))?;
        let items = match value {
            serde_json::Value::Array(items) => items,
            single => vec![single],
        };
        items
            .into_iter()
            .map(|v| serde_json::from_value::<MailSignal>(v).map_err(|e| AdapterError::Malformed(e.to_string())))
            .map(|r| {
                r.and_then(|s| {
                    if s.signal_id.is_empty() || !crate::routes::valid_routing_id(&s.routing_id) {
                        Err(AdapterError::Malformed("signal_id and a valid routing_id are required".into()))
                    } else {
                        Ok(s)
                    }
                })
            })
            .collect()
    }
}
