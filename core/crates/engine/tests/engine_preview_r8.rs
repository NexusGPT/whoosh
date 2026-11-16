//! Lane labels-r5, round 8: the re-run preview and the foot say the same thing. With a label whose AI card is on and NO model that can answer, the AI part
//! cannot add any message: the preview counts 0 by the AI and says so in one line, the offer's `candidates` is 0, and the run keeps every filing and says its
//! AI part is pending. With a model, the same mail is counted as the AI's to judge. One computation (`flow_ctx::candidate_parts`) feeds all three.
//!
//! Mutants (single edits; each seen red, logs in _captures/labels-r5/mutants.out; runner _captures/labels-r5/run-mutants.py):
//! - V1 flow_ctx.rs `rerun_candidates`: the AI part is added whether or not a model can answer -> with_no_model_the_preview_counts_nothing_by_the_ai_and_says_why
//! - V2 flow_ctx.rs `rerun_preview`: `by_ai` ignores `ai_ok` -> with_no_model_the_preview_counts_nothing_by_the_ai_and_says_why
//! - V3 flow_ctx.rs `ai_cannot_words`: the refusal is never turned into the line -> with_no_model_the_preview_counts_nothing_by_the_ai_and_says_why
//! - V4 decision_model.rs `refusal`: a Jev with key and consent still refuses -> a_jev_with_its_key_and_consent_has_no_refusal_and_the_preview_counts_its_mail
#[allow(dead_code, unused_imports)]
#[path = "../../autoconfig/tests/support/mod.rs"]
mod detect_fakes;

use std::sync::Arc;

use contracts::*;
use detect_fakes::{FakeDns, FakeHttp, FakeProbe};
use engine::{RustCore, TestTransport};
use secrets::{MemoryKeychain, Namespace};
use store::{NewMessage, Store};

const USER: &str = "owner@example.test";

fn detector() -> Box<dyn engine::Detect> { Box::new(autoconfig::Detector::new(FakeHttp::default(), FakeDns::default(), FakeProbe::default())) }
fn config(imap_port: u16, smtp_port: u16) -> AccountConfig {
    AccountConfig {
        imap_host: "127.0.0.1".into(), imap_port, imap_security: ConnectionSecurity::Ssl, smtp_host: "127.0.0.1".into(), smtp_port,
        smtp_security: ConnectionSecurity::Starttls, username: USER.into(), smtp_username: USER.into(), auth_method: AuthMethod::Password, source: DetectionSource::Manual,
    }
}

/// A stored message: `from` writes to the owner (an inbound one) unless it is the owner's own address (a reply, which lives in Sent).
fn mail(account: &str, key: &str, thread: &str, from: &str, subject: &str, at: i64) -> NewMessage {
    let outgoing = from == USER;
    let id = format!("{account}:{key}");
    NewMessage {
        message: Message {
            id, thread_id: thread.into(), account_id: account.into(), sender_name: from.into(), from_address: from.into(),
            to: vec![if outgoing { "friend@other.example".into() } else { USER.into() }], subject: subject.into(), preview: format!("Body of {subject}"),
            received_at_ms: at, read: outgoing, archived: false, folder: if outgoing { "Sent".into() } else { "INBOX".into() }, attachment_names: vec![],
            login_code: None, trackers_blocked: 0, list_unsubscribe: vec![], list_unsubscribe_one_click: false, reply_to_name: None, cc: vec![], bcc: vec![], is_outgoing: outgoing,
        },
        remote_id: Some(key.rsplit(':').next().unwrap_or("1").to_string()), body_text: None, body_html: None, has_list_header: false, size_bytes: Some(100), tab_id: None,
    }
}





struct Scripted { pick: std::sync::Mutex<String> }
impl DecisionPort for Scripted {
    fn provider(&self) -> DecisionProviderKind { DecisionProviderKind::Local }
    fn decide(&self, _state: DecisionState, questions: Vec<DecisionQuestion>) -> Result<DecisionResult, CoreError> {
        let pick = self.pick.lock().unwrap().clone();
        // fix-aisort round 3: the Labels question answered by its form (one choice, or one yes/no per label: LABEL_AI_QUESTION)
        let answers = questions.iter().map(|q| ::decision::test_answers::labels_answer(q, Some(pick.as_str()), 0.95).unwrap_or_else(|| match q {
            DecisionQuestion::Choice { id, .. } => DecisionAnswer::Choice { id: id.clone(), choice: pick.clone(), probabilities: [(pick.clone(), 0.95)].into() },
            DecisionQuestion::Score { id, .. } => DecisionAnswer::Score { id: id.clone(), value: 0.0 },
            DecisionQuestion::Noul { id, .. } => DecisionAnswer::Noul { id: id.clone(), yes: false, probability: 0.1 },
        })).collect();
        Ok(DecisionResult { answers, answered_by: DecisionProviderKind::Local, checkpoint: None })
    }
}

fn setup() -> (RustCore, String, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(&dir.path().join("whoosh.sqlite")).unwrap();
    let account = store.add_account(USER, "Owner", MailProvider::Imap, "svc\u{1f}acct", false, config(1, 1), 1).unwrap();
    (RustCore::with_parts(store, Arc::new(MemoryKeychain::new(Namespace::rig())), detector(), TestTransport::PlainLoopback), account, dir)
}
fn st(core: &RustCore, path: String) -> String { core.state(path).unwrap_or_else(|| "<none>".into()) }
fn rerun(core: &RustCore, label: &str) { core.run(Command::RerunLabel { label_id: label.into() }).unwrap(); core.flush_filing(); }


fn inbound(core: &RustCore, a: &str, n: usize) {
    let rows: Vec<_> = (1..=n).map(|i| mail(a, &format!("INBOX:{i}"), &format!("t{i}"), "ann@client.example", "Can you answer?", 10_000 + i as i64)).collect();
    core.store().insert_messages(rows).unwrap();
}
fn preview(core: &RustCore, label: &str) -> serde_json::Value { serde_json::from_str(&st(core, format!("label/{label}/rerun/preview"))).expect("the preview is JSON") }

#[test]
fn with_no_model_the_preview_counts_nothing_by_the_ai_and_says_why() {
    let (core, a, _d) = setup();
    inbound(&core, &a, 38);
    // the seeded Needs Reply: its AI card on with its description, no entry rule (the owner's empty "Subject contains"), no model anywhere
    let at = |leaf: &str| st(&core, format!("label/needs-reply/rerun/{leaf}"));
    let p = preview(&core, "needs-reply");
    assert_eq!((p["by_rule"].as_u64(), p["by_ai"].as_u64()), (Some(0), Some(0)), "38 unread unanswered messages and nothing can sort them: {p}");
    assert_eq!(p["ai_words"].as_str(), Some("The AI cannot sort yet: no model on this Mac."), "{p}");
    assert_eq!((at("candidates").as_str(), at("beyond").as_str()), ("0", "0"), "the offer says 0 too");
    // the foot is the same 0
    assert_eq!(st(&core, "label/needs-reply/flow/counts".into()), "{\"caught_by_entry\":0,\"kept_by_exit\":0,\"sent_to_fallback\":0,\"sorted_by_ai\":0,\"waiting_for_ai\":0}", "A140: with waiting_for_ai; the foot: 0 by rule, 0 by the AI, as the preview");
    // pressing Re-run changes nothing and says the model part is pending
    rerun(&core, "needs-reply");
    assert_eq!((at("total").as_str(), at("filed").as_str(), at("model_pending").as_str()), ("0", "0", "true"));
    // a model arrives: the same mail is now the AI's to judge, and no line says it cannot
    core.set_laya_port(Some(Arc::new(Scripted { pick: std::sync::Mutex::new("Needs reply".into()) })));
    let p = preview(&core, "needs-reply");
    assert_eq!((p["by_rule"].as_u64(), p["by_ai"].as_u64()), (Some(0), Some(38)), "{p}");
    assert!(p["ai_words"].is_null(), "{p}");
    assert_eq!(at("candidates"), "38");
}

#[test]
fn a_jev_with_its_key_and_consent_has_no_refusal_and_the_preview_counts_its_mail() {
    let (core, a, _d) = setup();
    inbound(&core, &a, 12);
    let key = core.run(Command::SaveProviderKey { provider_id: "jev".into(), key: SecretText { value: "FIXTURE-jev-key".into() } }).unwrap();
    let _ = key;
    let keychain = core.ai_providers().into_iter().find(|p| p.id == "jev").and_then(|p| p.keychain).expect("Jev's reference");
    let model = DecisionModel { provider: DecisionProviderKind::Jev, model: None, keychain: Some(keychain), decisions_enabled: vec!["labels".into()], question_words: Default::default(), last_run_ms: None };
    // chosen before the owner's yes: the preview says the consent is missing
    core.run(Command::GiveDecisionConsent { provider: DecisionProviderKind::Jev, given: true }).unwrap();
    core.run(Command::SaveDecisionModel { model }).unwrap();
    assert_eq!(st(&core, "label/needs-reply/flow/ai_refusal".into()), "null", "key, consent and model are all set: nothing refuses");
    assert_eq!(preview(&core, "needs-reply")["by_ai"].as_u64(), Some(12));
    core.run(Command::GiveDecisionConsent { provider: DecisionProviderKind::Jev, given: false }).unwrap();
    let p = preview(&core, "needs-reply");
    assert_eq!(p["by_ai"].as_u64(), Some(0));
    assert_eq!(p["ai_words"].as_str(), Some("The AI cannot sort yet: you have not allowed Jev to read your mail."), "{p}");
}
