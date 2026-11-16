//! fix-teachcore, part B (docs/lanes/hid-ai.md F5; docs/DECISION-MODEL.md v2 §3): the REAL engine serves the teaching block of each decision page,
//! `decision_teaching/<page>/corrections`, `/shared`, `decision_teaching/examples`, `decision_teaching/refits`, and `Command::RefitDecisionModel`.
//! A correction is the owner's hand on a tab or on Needs Reply (the built-in labels): the real `Label` command (actions `label_assign`, decided_by
//! 'owner'), never a row this file writes as the owner. Every test runs a store on this Mac with no mail server and no socket; each prints `PASS <case>`,
//! and the timed one prints `TIME <path> <ms> ms` on a store of 2,000 mails (the bar is 50 ms).
//!
//! BROKEN COPIES (each `find` occurs once in core/crates/engine/src/decision_teaching.rs; the named test goes red):
//! - D1 `COALESCE(a.decided_at_ms, a.updated_at_ms) >= ?` => `COALESCE(a.decided_at_ms, a.updated_at_ms) >= 0 AND ? IS NOT NULL`:
//!   corrections_are_counted_per_page_since_a_day_and_survive_a_restart (the correction of two days ago is counted)
//! - D2 `WHERE a.label_id IN ({}) AND a.decided_by = 'owner' AND` => `WHERE a.label_id IN ({}) AND`:
//!   corrections_are_counted_per_page_since_a_day_and_survive_a_restart (Laya's own filing is counted)
//! - D3 `"needs-reply" => Some(&["needs-reply"]),` => `"needs-reply" => Some(&["needs-reply", "updates"]),`:
//!   corrections_are_counted_per_page_since_a_day_and_survive_a_restart
//! - D4 `if !address.is_empty() && rows.iter().all(|r| r.1.trim().to_lowercase() == address) { return address; }` => `if false { return address; }`:
//!   what_the_corrections_share_is_the_sender_the_domain_or_the_list_and_nothing_when_they_differ
//! - D5 `if rows.iter().all(|r| domain_of(&r.1).as_deref() == Some(d.as_str())) { return d; }` =>
//!   `if rows.iter().any(|r| domain_of(&r.1).as_deref() == Some(d.as_str())) { return d; }`: what_the_corrections_share_is_the_sender_the_domain_or_the_list_and_nothing_when_they_differ (different domains)
//! - D6 `if rows.iter().all(|r| r.2.as_deref() == Some(list)) { return format!("list:{list}"); }` =>
//!   `if rows.iter().any(|r| r.2.as_deref() == Some(list)) { return format!("list:{list}"); }`: what_the_corrections_share_is_the_sender_the_domain_or_the_list_and_nothing_when_they_differ (two lists)
//! - D7 `Some(DecisionProviderKind::Jev) => return Err(CoreError::Invalid { field: "provider".into(), reason_words: JEV_NO_REFIT_WORDS.into() }),` =>
//!   `Some(DecisionProviderKind::Jev) => {}`: a_refit_is_refused_for_jev_for_rules_and_under_35_examples_and_never_claims_to_have_run
//! - D8 `if have < need {` => `if have < 1 {`: a_refit_is_refused_for_jev_for_rules_and_under_35_examples_and_never_claims_to_have_run (34 examples)
//! - D9 `*refits().lock().unwrap_or_else(|e| e.into_inner()).entry(store.path().to_path_buf()).or_insert(0) += 1;` => `` (removed): a_refit_is_refused_for_jev_for_rules_and_under_35_examples_and_never_claims_to_have_run
#[allow(dead_code, unused_imports)]
#[path = "../../autoconfig/tests/support/mod.rs"]
mod detect_fakes;

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;

use contracts::*;
use detect_fakes::{FakeDns, FakeHttp, FakeProbe};
use engine::{RustCore, TestTransport};
use secrets::{MemoryKeychain, Namespace};
use store::{NewMessage, Store};

const USER: &str = "owner@example.test";
const DAY_MS: i64 = 24 * 60 * 60 * 1000;
const BOUND_MS: u128 = 50;

fn detector() -> Box<dyn engine::Detect> { Box::new(autoconfig::Detector::new(FakeHttp::default(), FakeDns::default(), FakeProbe::default())) }
fn config() -> AccountConfig {
    AccountConfig {
        imap_host: "127.0.0.1".into(), imap_port: 1, imap_security: ConnectionSecurity::Ssl, smtp_host: "127.0.0.1".into(), smtp_port: 1,
        smtp_security: ConnectionSecurity::Starttls, username: USER.into(), smtp_username: USER.into(), auth_method: AuthMethod::Password, source: DetectionSource::Manual,
    }
}
fn now() -> i64 { std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis() as i64 }
fn msg(account: &str, id: &str, from: &str, list: Option<&str>, at: i64) -> NewMessage {
    NewMessage {
        message: Message {
            id: id.into(), thread_id: format!("t-{id}"), account_id: account.into(), sender_name: "Sender".into(), from_address: from.into(), to: vec![USER.into()],
            subject: format!("Subject {id}"), preview: format!("Preview {id}"), received_at_ms: at, read: false, archived: false, folder: "INBOX".into(),
            attachment_names: vec![], login_code: None, trackers_blocked: 0, list_unsubscribe: list.map(|l| vec![l.to_string()]).unwrap_or_default(),
            list_unsubscribe_one_click: false, reply_to_name: None, cc: vec![], bcc: vec![], is_outgoing: false,
        },
        remote_id: None, body_text: None, body_html: None, has_list_header: list.is_some(), size_bytes: Some(100), tab_id: None,
    }
}
fn open_core(path: &std::path::Path) -> (RustCore, String) {
    let store = Store::open(path).unwrap();
    let id = match store.account_summaries().unwrap().first() {
        Some(a) => a.id.clone(),
        None => store.add_account(USER, "Owner", MailProvider::Imap, "svc\u{1f}acct", false, config(), 1).unwrap(),
    };
    (RustCore::with_parts(store, Arc::new(MemoryKeychain::new(Namespace::rig())), detector(), TestTransport::PlainLoopback), id)
}
/// A store with `extra` mails and `bulk` more from 40 senders (no list).
fn rig(extra: &[(&str, &str, Option<&str>)], bulk: usize) -> (RustCore, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    let (core, id) = open_core(&dir.path().join("whoosh.sqlite"));
    let t = now() - 3 * DAY_MS;
    let mut batch: Vec<NewMessage> = extra.iter().enumerate().map(|(i, (m, from, list))| msg(&id, m, from, *list, t + i as i64)).collect();
    batch.extend((0..bulk).map(|i| msg(&id, &format!("b{i}"), &format!("sender{}@bulk.test", i % 40), None, t + 1000 + i as i64)));
    core.store().insert_messages(batch).unwrap();
    (core, dir)
}
fn ok(core: &RustCore, c: Command) -> CommandOutcome { core.run(c.clone()).unwrap_or_else(|e| panic!("{c:?} refused: {e:?}")) }
/// The owner's hand: the real Label command, by the label's name ("Updates", "Needs Reply").
fn correct(core: &RustCore, ids: &[&str], label_name: &str) -> CommandOutcome {
    ok(core, Command::Label { message_ids: ids.iter().map(|s| s.to_string()).collect(), label_name: label_name.into(), create_if_missing: false })
}
fn st(core: &RustCore, path: &str) -> Option<String> { core.state(path.to_string()) }
fn words(e: &CoreError) -> String {
    match e {
        CoreError::Invalid { reason_words, .. } | CoreError::Unexpected { reason_words } => reason_words.clone(),
        CoreError::Action { failure } => failure.reason_words.clone(),
        other => format!("{other:?}"),
    }
}
fn model(provider: DecisionProviderKind) -> DecisionModel {
    let keychain = (provider == DecisionProviderKind::Jev).then(|| KeychainRef { service: "svc".into(), account: "jev".into() });
    DecisionModel { provider, model: None, keychain, decisions_enabled: vec![], question_words: HashMap::new(), last_run_ms: None }
}

/// Which tab counts the owner's tab moves, Needs my reply counts the Needs Reply toggles, each page its own; a correction older than a day, Laya's own
/// filing and an undone correction are not counted; the pages with no record answer 0; an unknown page answers nothing; a restart keeps the counts.
#[test]
fn corrections_are_counted_per_page_since_a_day_and_survive_a_restart() {
    let (core, dir) = rig(&[("m1", "ann@ext.test", None), ("m2", "bob@ext.test", None), ("m3", "cy@ext.test", None), ("m4", "dee@ext.test", None), ("m5", "eve@ext.test", None)], 0);
    assert_eq!(st(&core, "decision_teaching/which-tab/corrections").as_deref(), Some("0"));
    correct(&core, &["m1", "m2", "m3"], "Updates");
    correct(&core, &["m4"], "Needs Reply");
    correct(&core, &["m5"], "Needs Reply");
    assert_eq!(st(&core, "decision_teaching/which-tab/corrections").as_deref(), Some("3"));
    assert_eq!(st(&core, "decision_teaching/needs-reply/corrections").as_deref(), Some("2"));
    assert_eq!(st(&core, "decision_teaching/sign-in-codes/corrections").as_deref(), Some("0"));
    assert_eq!(st(&core, "decision_teaching/no-such-page/corrections"), None);
    // m1's correction was two days ago: outside "since a day"; Laya's own filing of m4 under Promotions is not the owner's
    let old = now() - 2 * DAY_MS;
    core.store().write(move |ctx| {
        ctx.tx.execute("UPDATE label_assignment SET updated_at_ms = ?1, decided_at_ms = NULL WHERE label_id = 'updates' AND message_id = 'm1'", rusqlite::params![old])?;
        ctx.tx.execute("INSERT INTO label_assignment (label_id, message_id, updated_at_ms, decided_by, probability, decided_at_ms) VALUES ('promotions', 'm4', ?1, 'laya', 0.9, ?1)", rusqlite::params![now()])?;
        Ok(())
    }).unwrap();
    assert_eq!(st(&core, "decision_teaching/which-tab/corrections").as_deref(), Some("2"));
    // Z takes the newest correction back (m5 under Needs Reply): it is no longer counted
    ok(&core, Command::Undo);
    assert_eq!(st(&core, "decision_teaching/needs-reply/corrections").as_deref(), Some("1"));
    drop(core);
    let (core, _) = open_core(&dir.path().join("whoosh.sqlite"));
    assert_eq!(st(&core, "decision_teaching/which-tab/corrections").as_deref(), Some("2"));
    assert_eq!(st(&core, "decision_teaching/needs-reply/corrections").as_deref(), Some("1"));
    println!("PASS corrections_are_counted_per_page_since_a_day_and_survive_a_restart");
}

/// "Make a rule from these" is offered when the corrections share a sender, a domain or a list: the sender address first, then its domain, then
/// `list:<the list's address>`; "" when they share none, and "" with no correction.
#[test]
fn what_the_corrections_share_is_the_sender_the_domain_or_the_list_and_nothing_when_they_differ() {
    let shared = |rows: &[(&str, &str, Option<&str>)]| {
        let (core, _dir) = rig(rows, 0);
        let ids: Vec<&str> = rows.iter().map(|r| r.0).collect();
        correct(&core, &ids, "Updates");
        st(&core, "decision_teaching/which-tab/shared")
    };
    let (empty, _d) = rig(&[], 0);
    assert_eq!(st(&empty, "decision_teaching/which-tab/shared").as_deref(), Some("\"\""), "no correction, nothing shared");
    assert_eq!(shared(&[("a1", "News@Host.test", None), ("a2", "news@host.test", None)]).as_deref(), Some("\"news@host.test\""), "one sender");
    assert_eq!(shared(&[("a1", "billing@host.test", None), ("a2", "alerts@host.test", None)]).as_deref(), Some("\"host.test\""), "one domain");
    assert_eq!(shared(&[("a1", "a@one.test", Some("mailto:leave@lists.test")), ("a2", "b@two.test", Some("mailto:leave@lists.test"))]).as_deref(),
        Some("\"list:mailto:leave@lists.test\""), "one list");
    assert_eq!(shared(&[("a1", "a@one.test", None), ("a2", "b@two.test", None)]).as_deref(), Some("\"\""), "different senders and domains, no list");
    assert_eq!(shared(&[("a1", "a@one.test", Some("mailto:x@lists.test")), ("a2", "b@two.test", Some("mailto:y@lists.test"))]).as_deref(), Some("\"\""), "two lists");
    println!("PASS what_the_corrections_share_is_the_sender_the_domain_or_the_list_and_nothing_when_they_differ");
}

/// RefitDecisionModel: refused in words for Jev and for rules only, refused under 35 examples (and the count says how many there are), and past the
/// gates the ask is counted but the answer says in words that no refit ran (code engine.not_built_yet): nothing claims a refit happened.
#[test]
fn a_refit_is_refused_for_jev_for_rules_and_under_35_examples_and_never_claims_to_have_run() {
    let ids: Vec<String> = (0..40).map(|i| format!("r{i}")).collect();
    let rows: Vec<(&str, &str, Option<&str>)> = ids.iter().map(|m| (m.as_str(), "someone@ext.test", None)).collect();
    let (core, dir) = rig(&rows, 0);
    let refit = |core: &RustCore| core.run(Command::RefitDecisionModel).unwrap_err();
    correct(&core, &ids[..34].iter().map(String::as_str).collect::<Vec<_>>(), "Updates");
    assert_eq!(st(&core, "decision_teaching/examples").as_deref(), Some("34"));
    // no model saved: the shipped engine, Local, whose gate is the examples
    assert_eq!(words(&refit(&core)), "A refit needs 35 corrections; you have 34 so far.");
    assert_eq!(st(&core, "decision_teaching/refits").as_deref(), Some("0"));
    correct(&core, &[ids[34].as_str()], "Needs Reply");
    assert_eq!(st(&core, "decision_teaching/examples").as_deref(), Some("35"), "the Needs Reply toggles are examples too");
    core.store().decision_model_put(model(DecisionProviderKind::Jev), now()).unwrap();
    assert_eq!(words(&refit(&core)), "Jev is not refitted: make a rule from these corrections instead.");
    core.store().decision_model_put(model(DecisionProviderKind::Rules), now()).unwrap();
    assert_eq!(words(&refit(&core)), "The model is off, so there is nothing to refit: choose Local in Settings, Decisions.");
    assert_eq!(st(&core, "decision_teaching/refits").as_deref(), Some("0"), "a refused ask is not counted");
    core.store().decision_model_put(model(DecisionProviderKind::Local), now()).unwrap();
    let e = refit(&core);
    match &e {
        CoreError::Action { failure } => {
            assert_eq!(failure.code.as_deref(), Some("engine.not_built_yet"), "{e:?}");
            assert_eq!(failure.reason_words, "Whoosh cannot refit the model on this Mac yet: your corrections are kept for when it can.");
        }
        other => panic!("past the gates the answer is the refusal in words, got {other:?}"),
    }
    assert_eq!(st(&core, "decision_teaching/refits").as_deref(), Some("1"));
    let log = std::fs::read_to_string(dir.path().join("core.log")).unwrap_or_default();
    assert!(log.contains("decision refit: asked on 35 examples; not run"), "core.log: {log}");
    println!("PASS a_refit_is_refused_for_jev_for_rules_and_under_35_examples_and_never_claims_to_have_run");
}

/// Each teaching state read answers under 50 ms on a store of 2,000 mails with 40 corrections (printed).
#[test]
fn each_teaching_state_reads_under_50_ms_on_2000_mails() {
    let extra: Vec<String> = (0..40).map(|i| format!("c{i}")).collect();
    let rows: Vec<(&str, &str, Option<&str>)> = extra.iter().map(|m| (m.as_str(), "news@host.test", Some("mailto:leave@lists.test"))).collect();
    let (core, _dir) = rig(&rows, 2000 - rows.len());
    correct(&core, &extra[..30].iter().map(String::as_str).collect::<Vec<_>>(), "Updates");
    correct(&core, &extra[30..].iter().map(String::as_str).collect::<Vec<_>>(), "Needs Reply");
    let paths = ["decision_teaching/which-tab/corrections", "decision_teaching/which-tab/shared", "decision_teaching/needs-reply/corrections",
        "decision_teaching/needs-reply/shared", "decision_teaching/examples", "decision_teaching/refits"];
    let mut slow = vec![];
    for p in paths {
        let t = Instant::now();
        let v = st(&core, p);
        let ms = t.elapsed().as_millis();
        println!("TIME {p} {ms} ms = {v:?}");
        assert!(v.is_some(), "{p} answered nothing");
        if ms >= BOUND_MS { slow.push(format!("{p} took {ms} ms, not under {BOUND_MS} ms")); }
    }
    assert_eq!(st(&core, "decision_teaching/which-tab/corrections").as_deref(), Some("30"));
    assert_eq!(st(&core, "decision_teaching/examples").as_deref(), Some("40"));
    assert!(slow.is_empty(), "{slow:?}");
    println!("PASS each_teaching_state_reads_under_50_ms_on_2000_mails");
}
