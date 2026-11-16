//! The Jev engine against the replayed fixture: the request shape, the key read from the Keychain at call time and never
//! printed, Jev's own words on a refusal, no call before consent, the shape check.
mod support;
use ai_providers::transport::{Body, Method, Request, Response, Transport};
use contracts::{CoreError, DecisionPort, DecisionProviderKind};
use decision::adapters::jev::JevDecision;
use decision::{DecideMessage, DecisionSettings, MemoryConsent, Outcome, Source, TabOption, Value};
use secrets::{MemoryKeychain, Namespace, Secret, SecretKind, StoreSecret};
use serde_json::Value as Json;
use smart_inbox::TabRef;
use std::sync::{Arc, Mutex};
use support::*;

fn fixture() -> Json {
    serde_json::from_str(include_str!("../fixtures/jev/decide.json")).unwrap()
}

struct Replay {
    response: Response,
    seen: Mutex<Vec<Request>>,
}
impl Transport for Replay {
    fn send(&self, request: &Request) -> Result<Response, String> {
        self.seen.lock().unwrap().push(request.clone());
        Ok(self.response.clone())
    }
}

fn replay(step: &str) -> Arc<Replay> {
    let s = &fixture()[step];
    Arc::new(Replay { response: Response { status: s["status"].as_u64().unwrap() as u16, body: s["body"].to_string() }, seen: Mutex::new(vec![]) })
}

fn jev(transport: Arc<Replay>, consent: bool) -> (JevDecision, Arc<MemoryKeychain>) {
    let keychain = Arc::new(MemoryKeychain::new(Namespace::rig()));
    let key = keychain.execute(SecretKind::ProviderCredential, "jev", &Secret::new(fixture()["key"].as_str().unwrap())).unwrap();
    let c = Arc::new(MemoryConsent::default());
    if consent {
        c.give(DecisionProviderKind::Jev);
    }
    (JevDecision { transport, secrets: keychain.clone(), key, consent: c }, keychain)
}

fn tabs() -> Vec<TabOption> {
    ["Primary", "Updates", "Promotions", "Social"].iter().map(|n| TabOption { tab: TabRef::Builtin(n.to_string()), name: n.to_string() }).collect()
}

fn run(engine: &JevDecision) -> decision::Decisions {
    let settings = DecisionSettings::shipped(DecisionProviderKind::Jev, &[Outcome::NeedsMyReply, Outcome::SignInCodes, Outcome::WhichTab]);
    let t = tabs();
    DecideMessage { settings: &settings, engine: Some(engine), tabs: &t, labels: &[] }.execute(&msg("m1", "shop@store-example.com", "Your order", "Thanks for your order"))
}

#[test]
fn the_request_matches_the_fixture_and_the_answers_decide() {
    let t = replay("answers");
    let (engine, _) = jev(t.clone(), true);
    let d = run(&engine);
    let seen = t.seen.lock().unwrap();
    assert_eq!(seen.len(), 1, "one call for the three questions");
    let r = &seen[0];
    let want = &fixture()["request"];
    assert_eq!(r.method, Method::Post);
    assert_eq!(r.url, want["url"].as_str().unwrap());
    assert!(r.headers.iter().any(|(k, v)| k == "authorization" && v == want["authorization"].as_str().unwrap()));
    let Body::Json(body) = &r.body else { panic!("json body") };
    let body: Json = serde_json::from_str(body).unwrap();
    assert_eq!(body["model"], want["model"]);
    assert!(body["state"].as_str().unwrap().contains("subject: Your order"));
    for (id, ty) in want["question_types"].as_object().unwrap() {
        assert_eq!(&body["questions"][id]["type"], ty, "{id}");
    }
    assert_eq!(body["questions"]["needs-reply/model"]["criteria"], serde_json::json!({"true": "yes", "false": "no"}));
    assert!(body["questions"]["which-tab/model"]["criteria"].as_object().unwrap().contains_key("Updates"));
    // Jev's answers: Updates at 0.93 (trust 0.80), no reply at 1 - 0.12 = 0.88 (trust 0.75), no code at 0.97 (trust 0.90)
    let tab = d.get(Outcome::WhichTab).unwrap();
    assert_eq!(tab.value, Value::Tab(TabRef::Builtin("Updates".into())));
    assert!(matches!(tab.decided_by.source, Source::Model { provider: DecisionProviderKind::Jev, .. }));
    assert_eq!(tab.decided_by.to_contract(), contracts::DecidedBy::Jev);
    assert_eq!(d.get(Outcome::NeedsMyReply).unwrap().decided_by.entry_id, "needs-reply/model");
    assert_eq!(d.get(Outcome::SignInCodes).unwrap().value, Value::Yes(false));
}

#[test]
fn a_refused_key_returns_jevs_words_and_never_the_key() {
    let (engine, _) = jev(replay("refused_key"), true);
    let d = run(&engine);
    let Some(CoreError::Action { failure }) = d.engine_error else { panic!("a refusal: {:?}", d.engine_error) };
    assert_eq!(failure.account_name, "Jev");
    assert_eq!(failure.reason_words, "Invalid API key: <key> is not recognised");
    assert!(!failure.retryable);
    assert!(d.verdicts.iter().all(|v| !matches!(v.decided_by.source, Source::Model { .. })), "every model entry fell through");
    let (engine, _) = jev(replay("caller_error"), true);
    let Some(CoreError::Action { failure }) = run(&engine).engine_error else { panic!() };
    assert_eq!((failure.reason_words.as_str(), failure.retryable), ("criteria must be an object", false));
}

#[test]
fn no_mail_reaches_jev_before_consent() {
    let t = replay("answers");
    let (engine, _) = jev(t.clone(), false);
    let d = run(&engine);
    assert!(t.seen.lock().unwrap().is_empty(), "no request before consent");
    assert!(matches!(d.engine_error, Some(CoreError::Action { .. })));
}

#[test]
fn the_key_is_read_at_call_time_and_never_printed() {
    let t = replay("answers");
    let (engine, keychain) = jev(t.clone(), true);
    run(&engine);
    keychain.execute(SecretKind::ProviderCredential, "jev", &Secret::new("jev-rotated-key-91c2")).unwrap();
    run(&engine);
    let seen = t.seen.lock().unwrap();
    assert!(seen[1].headers.iter().any(|(k, v)| k == "authorization" && v == "Bearer jev-rotated-key-91c2"));
    for r in seen.iter() {
        let printed = format!("{r:?}");
        assert!(!printed.contains("jev-test-key") && !printed.contains("jev-rotated-key"), "{printed}");
    }
    assert!(!format!("{:?}", engine.key).contains("jev-test-key"));
}

#[test]
fn an_answer_missing_a_question_is_refused_whole() {
    let (engine, _) = jev(replay("bad_shape"), true);
    let r = engine.decide(decision::decide::state_of(&msg("m", "a@b-example.com", "s", "")), vec![
        contracts::DecisionQuestion::Choice { id: "which-tab/model".into(), prompt: "Which tab?".into(), options: vec!["Updates".into()] },
        contracts::DecisionQuestion::Noul { id: "needs-reply/model".into(), prompt: "Reply?".into() },
    ]);
    assert!(matches!(r, Err(CoreError::Action { ref failure }) if failure.reason_words.contains("unexpected shape")), "{r:?}");
    let ok = jev(replay("answers"), true).0.decide(decision::decide::state_of(&msg("m", "a@b-example.com", "s", "")), vec![contracts::DecisionQuestion::Noul { id: "needs-reply/model".into(), prompt: "Reply?".into() }]);
    assert!(ok.is_err(), "an answer with keys not asked is refused too");
}
