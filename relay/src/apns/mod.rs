//! APNs sender: a provider-token (ES256 JWT) HTTP/2 client, the same construction the `a2` crate
//! uses, built on reqwest + jsonwebtoken so that one client can also reach the local stub over
//! cleartext HTTP/2 (a2 only speaks TLS). The payload carries a signal, never mail content.
pub mod stub;
pub mod worker;

use crate::registry::jobs::Job;
use jsonwebtoken::{Algorithm, EncodingKey, Header};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// Apple requires a provider token to be refreshed no less than every 60 minutes and no more
/// often than every 20. Constant requested for docs/CONSTANTS.md as APNS_JWT_REUSE_MIN = 50.
const JWT_REUSE: Duration = Duration::from_secs(50 * 60);
/// Constant requested for docs/CONSTANTS.md as APNS_SEND_TIMEOUT_S = 30.
const SEND_TIMEOUT: Duration = Duration::from_secs(30);

pub struct ApnsConfig {
    pub key_pem: String,
    pub key_id: String,
    pub team_id: String,
    pub topic: String,
    pub sandbox_url: String,
    pub production_url: String,
}

pub enum ApnsState {
    Ready(ApnsSender),
    Unconfigured,
    Broken(String),
}

impl ApnsState {
    /// Reads RELAY_APNS_*. Never fails the boot: a missing or unreadable key leaves the relay up,
    /// jobs queued and the health route degraded. The key comes from a file (RELAY_APNS_KEY_PATH)
    /// or inline (RELAY_APNS_KEY, the PEM with real or `\n` newlines) so a host that only offers
    /// environment secrets can run the relay; the path wins when both are set.
    pub fn from_env() -> ApnsState {
        let key_pem = match (env("RELAY_APNS_KEY_PATH"), env("RELAY_APNS_KEY")) {
            (Some(path), _) => match std::fs::read_to_string(&path) {
                Ok(k) => k,
                Err(e) => return ApnsState::Broken(format!("cannot read APNs key at {path}: {e}")),
            },
            (None, Some(inline)) => inline.replace("\\n", "\n"),
            (None, None) => return ApnsState::Unconfigured,
        };
        let (Some(key_id), Some(team_id), Some(topic)) =
            (env("RELAY_APNS_KEY_ID"), env("RELAY_APNS_TEAM_ID"), env("RELAY_APNS_TOPIC"))
        else {
            return ApnsState::Broken("RELAY_APNS_KEY_ID, RELAY_APNS_TEAM_ID and RELAY_APNS_TOPIC are all required".into());
        };
        let config = ApnsConfig {
            key_pem,
            key_id,
            team_id,
            topic,
            sandbox_url: env("RELAY_APNS_SANDBOX_URL").unwrap_or_else(|| "https://api.sandbox.push.apple.com".into()),
            production_url: env("RELAY_APNS_PRODUCTION_URL").unwrap_or_else(|| "https://api.push.apple.com".into()),
        };
        match ApnsSender::new(config) {
            Ok(s) => ApnsState::Ready(s),
            Err(e) => ApnsState::Broken(e),
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            ApnsState::Ready(_) => "ready",
            ApnsState::Unconfigured => "unconfigured",
            ApnsState::Broken(_) => "broken",
        }
    }

    pub fn detail(&self) -> Option<String> {
        match self {
            ApnsState::Broken(why) => Some(why.clone()),
            _ => None,
        }
    }
}

fn env(name: &str) -> Option<String> {
    std::env::var(name).ok().filter(|v| !v.trim().is_empty())
}

pub struct SendOutcome {
    pub status: u16,
    pub reason: Option<String>,
    /// The real response body, logged on failure (never reduced to the status).
    pub body: String,
}

#[derive(Serialize)]
struct Claims<'a> {
    iss: &'a str,
    iat: u64,
}

pub struct ApnsSender {
    config: ApnsConfig,
    plain: reqwest::Client,
    tls: reqwest::Client,
    token: Mutex<Option<(String, Instant)>>,
}

impl ApnsSender {
    fn new(config: ApnsConfig) -> Result<ApnsSender, String> {
        EncodingKey::from_ec_pem(config.key_pem.as_bytes()).map_err(|e| format!("APNs key is not an ES256 PEM: {e}"))?;
        let plain = reqwest::Client::builder().http2_prior_knowledge().timeout(SEND_TIMEOUT).build().map_err(|e| e.to_string())?;
        let tls = reqwest::Client::builder().timeout(SEND_TIMEOUT).build().map_err(|e| e.to_string())?;
        Ok(ApnsSender { config, plain, tls, token: Mutex::new(None) })
    }

    fn provider_token(&self) -> Result<String, String> {
        let mut cached = self.token.lock().unwrap_or_else(|p| p.into_inner());
        if let Some((jwt, at)) = cached.as_ref() {
            if at.elapsed() < JWT_REUSE {
                return Ok(jwt.clone());
            }
        }
        let key = EncodingKey::from_ec_pem(self.config.key_pem.as_bytes()).map_err(|e| e.to_string())?;
        let mut header = Header::new(Algorithm::ES256);
        header.kid = Some(self.config.key_id.clone());
        let iat = crate::registry::now_unix() as u64;
        let jwt = jsonwebtoken::encode(&header, &Claims { iss: &self.config.team_id, iat }, &key).map_err(|e| e.to_string())?;
        *cached = Some((jwt.clone(), Instant::now()));
        Ok(jwt)
    }

    pub async fn send(&self, job: &Job) -> Result<SendOutcome, String> {
        let base = if job.environment == "production" { &self.config.production_url } else { &self.config.sandbox_url };
        let url = format!("{}/3/device/{}", base.trim_end_matches('/'), job.device_token);
        let client = if url.starts_with("http://") { &self.plain } else { &self.tls };
        let jwt = self.provider_token()?;
        let response = client
            .post(&url)
            .header("authorization", format!("bearer {jwt}"))
            .header("apns-topic", &self.config.topic)
            .header("apns-push-type", if is_renew(job) { "background" } else { "alert" })
            // Apple: a background push must use priority 5; 10 is refused.
            .header("apns-priority", if is_renew(job) { "5" } else { "10" })
            .header("apns-collapse-id", collapse_id(&job.signal_id))
            .json(&payload(job))
            .send()
            .await
            .map_err(|e| format!("transport: {e}"))?;
        let status = response.status().as_u16();
        let body = response.text().await.unwrap_or_default();
        let reason = serde_json::from_str::<serde_json::Value>(&body)
            .ok()
            .and_then(|v| v.get("reason").and_then(|r| r.as_str().map(str::to_owned)));
        Ok(SendOutcome { status, reason, body })
    }
}

/// APNs allows a collapse id of at most 64 bytes; a SHA-256 hex digest is exactly that, and a
/// repeated delivery of one signal then shows as one notification on the phone.
fn collapse_id(signal_id: &str) -> String {
    hex::encode(Sha256::digest(signal_id.as_bytes()))
}

/// A ref starting with this tells the device to renew a provider subscription (the adapters'
/// renew job emits it). It is not mail, so it never shows as an alert.
pub const RENEW_REF_PREFIX: &str = "watch-renew:";

fn is_renew(job: &Job) -> bool {
    job.opaque_ref.as_deref().is_some_and(|r| r.starts_with(RENEW_REF_PREFIX))
}

/// The ref as it goes into the payload. One longer than RELAY_APNS_REF_MAX_B would push the whole
/// payload toward Apple's cap; it is left out (the phone then fetches the mail itself) and logged.
fn bounded_ref(job: &Job) -> Option<&str> {
    let r = job.opaque_ref.as_deref()?;
    if r.len() > crate::registry::constants::RELAY_APNS_REF_MAX_B as usize {
        tracing::error!("job {}: ref is {} bytes, over the {} byte cap; sent without it", job.job_id, r.len(), crate::registry::constants::RELAY_APNS_REF_MAX_B);
        return None;
    }
    Some(r)
}

/// A signal only. The Notification Service Extension fetches sender and subject itself. A renew
/// signal is a silent background push (content-available, no alert, no sound) so it never reads
/// as "New mail".
fn payload(job: &Job) -> serde_json::Value {
    let whoosh = serde_json::json!({ "routing_id": job.routing_id, "ref": bounded_ref(job) });
    if is_renew(job) {
        return serde_json::json!({ "aps": { "content-available": 1 }, "whoosh": whoosh });
    }
    serde_json::json!({
        "aps": { "alert": { "title": "New mail", "body": "Open Whoosh to read it" }, "mutable-content": 1, "sound": "default" },
        "whoosh": whoosh
    })
}
