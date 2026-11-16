//! ai-settings-core: the pasted key's check (`check::check_key`). Mutants, each seen red (docs/lanes/ai-settings-core.md):
//! K1 `check_key` skips the empty-key refusal; K2 the Candidate port answers an empty secret; K3 `provider_by_slug` matches on the name.
use std::sync::{Arc, Mutex};

use ai_providers::check::{check_key, provider_by_slug};
use ai_providers::table::ProviderId;
use ai_providers::transport::{Request, Response, Transport};
use contracts::CoreError;

const KEY: &str = "FIXTURE-sk-ant-good-0001";

/// Answers every request with one status and body, and keeps what it was sent.
struct Answer { status: u16, body: String, seen: Mutex<Vec<Request>> }

impl Answer {
    fn new(status: u16, body: &str) -> Arc<Answer> { Arc::new(Answer { status, body: body.into(), seen: Mutex::new(Vec::new()) }) }
}

impl Transport for Answer {
    fn send(&self, r: &Request) -> Result<Response, String> {
        self.seen.lock().unwrap().push(r.clone());
        Ok(Response { status: self.status, body: self.body.clone() })
    }
}

fn words(e: &CoreError) -> (String, bool) {
    match e { CoreError::Action { failure } => (failure.reason_words.clone(), failure.retryable), other => panic!("expected a refusal in words, got {other:?}") }
}

#[test]
fn an_accepted_key_answers_the_providers_models_and_was_sent_as_its_header() {
    let t = Answer::new(200, r#"{"data":[{"id":"claude-sonnet-5-5"},{"id":"claude-haiku-5-5"}],"has_more":false}"#);
    let models = check_key(ProviderId::Anthropic, t.clone(), KEY, Some("http://127.0.0.1:9".into())).expect("accepted");
    assert_eq!(models, vec!["claude-sonnet-5-5", "claude-haiku-5-5"]);
    let seen = t.seen.lock().unwrap();
    assert_eq!(seen.len(), 1);
    assert_eq!(seen[0].url, "http://127.0.0.1:9/models");
    assert_eq!(seen[0].headers.iter().find(|(k, _)| k == "x-api-key").map(|(_, v)| v.as_str()), Some(KEY));
    assert!(!format!("{:?}", seen[0]).contains(KEY), "a request's Debug never prints the key");
}

#[test]
fn a_refused_key_answers_the_providers_words_with_the_key_scrubbed() {
    let body = format!(r#"{{"type":"error","error":{{"type":"authentication_error","message":"invalid x-api-key {KEY}"}}}}"#);
    let t = Answer::new(401, &body);
    let (w, retry) = words(&check_key(ProviderId::Anthropic, t, KEY, None).unwrap_err());
    assert_eq!(w, "invalid x-api-key <key>");
    assert!(!retry, "a refused key does not pass on a later try");
}

#[test]
fn a_server_fault_is_retryable_in_the_providers_words() {
    let (w, retry) = words(&check_key(ProviderId::OpenAi, Answer::new(503, r#"{"error":{"message":"The server is overloaded"}}"#), KEY, None).unwrap_err());
    assert_eq!(w, "The server is overloaded");
    assert!(retry);
}

#[test]
fn an_empty_key_is_refused_before_any_request() {
    let t = Answer::new(200, r#"{"data":[]}"#);
    let e = check_key(ProviderId::Anthropic, t.clone(), "  \n", None).unwrap_err();
    assert!(matches!(e, CoreError::Invalid { ref field, .. } if field == "key"), "{e:?}");
    assert!(t.seen.lock().unwrap().is_empty(), "nothing was asked of the provider");
}

#[test]
fn slugs_name_the_table_rows() {
    assert_eq!(provider_by_slug("anthropic"), Some(ProviderId::Anthropic));
    assert_eq!(provider_by_slug("openrouter"), Some(ProviderId::OpenRouter));
    assert_eq!(provider_by_slug("Anthropic"), None, "the slug, never the shown name");
    assert_eq!(provider_by_slug("jev"), None);
}
