//! Jev: TypeSafe's typed classifier, called with the owner's API key. The key is read from the Keychain through the
//! secrets port at the moment of the call and lives only in the request's Authorization header, which the transport's
//! Debug prints as <redacted>. No mail is sent before the owner's consent for Jev is recorded.
use crate::adapters::wire::{read_answers, request_body, state_text_with_thread};
use crate::thread_context::jev_budget_chars;
use crate::consent::HasConsent;
use ai_providers::transport::{Body, Method, Request, Transport};
use contracts::{ActionFailed, CoreError, DecisionPort, DecisionProviderKind, DecisionQuestion, DecisionResult, DecisionState, KeychainRef};
use secrets::ReadSecret;
use serde_json::Value;
use std::sync::Arc;

/// The direct route of the org's client (retrieval-dossier-jev.md Need 1: `TYPESAFE_URL`, header `Authorization: Bearer`).
pub const JEV_URL: &str = "https://api.typesafe.ai/v1/systemone";
/// The model name in every request of the org's client.
pub const JEV_MODEL: &str = "jev-latest";
const NAME: &str = "Jev";

pub struct JevDecision {
    pub transport: Arc<dyn Transport>,
    pub secrets: Arc<dyn ReadSecret>,
    pub key: KeychainRef,
    pub consent: Arc<dyn HasConsent>,
}

fn failed(reason_words: String, retryable: bool) -> CoreError {
    CoreError::Action { failure: ActionFailed { account_name: NAME.into(), reason_words, retryable, code: None } }
}

/// Jev's own words from an error body: `error.message`, `error`, `detail` or `message`, else the whole body. A key the
/// body echoes is replaced by <key>.
pub fn jev_words(body: &str, key: &str) -> String {
    let parsed: Option<Value> = serde_json::from_str(body).ok();
    let pick = parsed.as_ref().and_then(|v| {
        v.pointer("/error/message").and_then(Value::as_str).or_else(|| v.get("error").and_then(Value::as_str)).or_else(|| v.get("detail").and_then(Value::as_str)).or_else(|| v.get("message").and_then(Value::as_str))
    });
    let words = pick.map(str::to_string).unwrap_or_else(|| body.trim().to_string());
    if key.is_empty() { words } else { words.replace(key, "<key>") }
}

impl DecisionPort for JevDecision {
    fn provider(&self) -> DecisionProviderKind {
        DecisionProviderKind::Jev
    }

    fn decide(&self, state: DecisionState, questions: Vec<DecisionQuestion>) -> Result<DecisionResult, CoreError> {
        if !self.consent.execute(DecisionProviderKind::Jev) {
            return Err(failed("you have not agreed yet to send mail to Jev; nothing was sent".into(), false));
        }
        let secret = self.secrets.execute(&self.key)?;
        let key = std::str::from_utf8(secret.expose()).map_err(|_| CoreError::Invalid { field: "keychain_ref".into(), reason_words: "the stored Jev key is not text".into() })?.trim().to_string();
        if key.is_empty() {
            return Err(CoreError::Invalid { field: "keychain_ref".into(), reason_words: "the stored Jev key is empty".into() });
        }
        let body = request_body(&state_text_with_thread(&state, &state.text, jev_budget_chars()), JEV_MODEL, &questions);
        let request = Request {
            method: Method::Post,
            url: JEV_URL.into(),
            headers: vec![("authorization".into(), format!("Bearer {key}")), ("content-type".into(), "application/json".into())],
            body: Body::Json(body),
        };
        let response = self.transport.send(&request).map_err(|e| failed(format!("no answer: {e}"), true))?;
        if !response.is_success() {
            // 422 is a caller bug and 402 an exhausted key: never retried (retrieval-dossier-jev.md Need 4).
            return Err(failed(jev_words(&response.body, &key), response.is_retryable()));
        }
        let answers = read_answers(NAME, &response.body, &questions)?;
        let checkpoint = serde_json::from_str::<Value>(&response.body).ok().and_then(|v| v.get("model").and_then(Value::as_str).map(str::to_string));
        Ok(DecisionResult { answers, answered_by: DecisionProviderKind::Jev, checkpoint })
    }
}
