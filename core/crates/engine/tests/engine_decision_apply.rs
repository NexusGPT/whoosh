//! gap-decisions-real: `Command::ApplyDecisionPreview` on the real core (decision_preview.rs `apply`), the preview's rows kept in a field of the core, and the speed of
//! the rules-only preview. Apply writes the placements of the LAST preview for its page (a tab through `write_placements`, a label through `label_file_flow`), a second
//! Apply writes nothing new, a stale preview is refused in words and writes nothing, and nothing leaves this Mac.
//!
//! Mutants (single edits; each to be seen red by the lead's run), one broken copy per behaviour:
//! - A1 decision_preview.rs `apply`: the `Target::Tab` arm does not push its placement (or `write_placements` is not called) -> apply_writes_exactly_the_previewed_tab_placements
//! - A2 decision_preview.rs `plan_of`: the `if differs` guard removed from the `WhichTab` arm (every row is written) -> apply_writes_exactly_the_previewed_tab_placements
//! - A3 decision_preview.rs `apply`: the `if let Some(n) = last.placed { return Ok(n); }` early return removed (a second Apply writes again) -> a_second_apply_writes_nothing_new
//! - A4 decision_preview.rs `apply`: the `if now != last.snapshot` refusal removed -> a_stale_preview_is_refused_in_words_and_writes_nothing
//! - A5 decision_preview.rs `apply`: the label arm does not file (the `label_file_flow` call removed) -> apply_files_the_labels_the_preview_showed
//! - A6 decision_preview.rs `plan_of`: the `if held { return None; }` check removed from the label arm (a filing that stands is rewritten) -> apply_leaves_an_existing_filing_as_it_is
//! - A7 decision_preview.rs `state`: `decision_preview/applied` always "false" -> apply_writes_exactly_the_previewed_tab_placements
//! - A8 rust_core.rs: the `ApplyDecisionPreview` arm answers `Ok` without calling `apply` -> apply_writes_exactly_the_previewed_tab_placements
//! - A9 decision_preview.rs `apply`: `Some(l) if !l.rows.is_empty() && l.page.is_some() => l,` made `Some(l) => l,` (a preview with no rows can be applied) -> apply_without_a_preview_is_refused_in_words
//! - A10 decision_preview.rs `PreviewBook::put` and `read` (the two adjacent lines): both made to use one `static` book (two cores share rows) -> two_cores_keep_their_own_preview
//! - S1 decision_preview.rs `preview`: an 80 ms pause before the walk -> the_rules_only_preview_on_20_of_2000_mails_stays_under_50_ms
//! - G1 (no broken copy in the files this lane owns): the guard is two layers, the ledger refusal in `preview` (no DECISION_PREVIEW_MS number, so a model is never asked) and the consent re-check in decision_model.rs `StoredConsent` (not this lane's file); the test goes red only when BOTH are gone -> preview_and_apply_send_nothing_without_the_owners_consent_and_with_rules_only
#[allow(dead_code, unused_imports)]
#[path = "../../autoconfig/tests/support/mod.rs"]
mod detect_fakes;

use std::sync::Arc;
use std::time::Instant;

use contracts::*;
use detect_fakes::{FakeDns, FakeHttp, FakeProbe};
use engine::{RustCore, TestTransport};
use secrets::{MemoryKeychain, Namespace};
use store::{NewMessage, Store};

const USER: &str = "owner@example.test";

fn detector() -> Box<dyn engine::Detect> { Box::new(autoconfig::Detector::new(FakeHttp::default(), FakeDns::default(), FakeProbe::default())) }
fn kc() -> Arc<MemoryKeychain> { Arc::new(MemoryKeychain::new(Namespace::rig())) }
fn config() -> AccountConfig {
    AccountConfig {
        imap_host: "127.0.0.1".into(), imap_port: 1, imap_security: ConnectionSecurity::Ssl, smtp_host: "127.0.0.1".into(), smtp_port: 1,
        smtp_security: ConnectionSecurity::Starttls, username: USER.into(), smtp_username: USER.into(), auth_method: AuthMethod::Password, source: DetectionSource::Manual,
    }
}

fn mail(account: &str, n: i64, from: &str, subject: &str) -> NewMessage {
    let id = format!("{account}:INBOX:{n}");
    NewMessage {
        message: Message {
            id, thread_id: format!("t{n}"), account_id: account.into(), sender_name: from.split('@').next().unwrap_or(from).into(), from_address: from.into(),
            to: vec![USER.into()], subject: subject.into(), preview: format!("Body of {subject}"),
            received_at_ms: 1_000 + n, read: false, archived: false, folder: "INBOX".into(), attachment_names: vec![],
            login_code: None, trackers_blocked: 0, list_unsubscribe: vec![], list_unsubscribe_one_click: false, reply_to_name: None, cc: vec![], bcc: vec![], is_outgoing: false,
        },
        remote_id: Some(n.to_string()), body_text: None, body_html: None, has_list_header: false, size_bytes: Some(100), tab_id: None,
    }
}

struct Rig { core: RustCore, account: String, dir: tempfile::TempDir }

/// `count` inbox messages: n % 3 == 1 come from alpha, the rest from beta.
fn setup(count: i64) -> Rig {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(&dir.path().join("whoosh.sqlite")).unwrap();
    let account = store.add_account(USER, "Owner", MailProvider::Imap, "svc\u{1f}acct", false, config(), 1).unwrap();
    let core = RustCore::with_parts(store, kc(), detector(), TestTransport::PlainLoopback);
    let msgs = (1..=count).map(|n| if n % 3 == 1 { mail(&account, n, "alpha@example.test", &format!("Alpha {n}")) } else { mail(&account, n, "beta@example.test", &format!("Beta {n}")) }).collect();
    if count > 0 { core.store().insert_messages(msgs).unwrap(); }
    Rig { core, account, dir }
}

fn rule(key: &str, outcome: &str) -> PreviewEntry { PreviewEntry { kind: PreviewEntryKind::Rule, enabled: true, match_key: key.into(), outcome: outcome.into(), question: String::new(), trust: None } }
fn otherwise(outcome: &str) -> PreviewEntry { PreviewEntry { kind: PreviewEntryKind::Otherwise, enabled: true, match_key: String::new(), outcome: outcome.into(), question: String::new(), trust: None } }
fn preview(page: &str, entries: Vec<PreviewEntry>) -> Command { Command::PreviewDecisionList { page_id: page.into(), entries } }

/// Every stored placement, as the database holds it: tabs (`message_category`) and labels (`label_assignment`), every column, sorted.
fn dump(core: &RustCore) -> Vec<String> {
    let conn = smart_inbox::load::open_reader(core.store().path()).unwrap();
    let mut out = Vec::new();
    for (table, order) in [("message_category", "message_id"), ("label_assignment", "label_id, message_id")] {
        let mut st = conn.prepare(&format!("SELECT * FROM {table} ORDER BY {order}")).unwrap();
        let n = st.column_count();
        let rows = st.query_map([], |r| (0..n).map(|i| r.get_ref(i).map(|v| format!("{v:?}"))).collect::<Result<Vec<_>, _>>().map(|v| v.join("|"))).unwrap();
        for r in rows { out.push(format!("{table}: {}", r.unwrap())); }
    }
    out
}

fn tab_rows(core: &RustCore) -> Vec<(String, String, String)> {
    let conn = smart_inbox::load::open_reader(core.store().path()).unwrap();
    let mut st = conn.prepare("SELECT message_id, tab_id, decided_by FROM message_category ORDER BY message_id").unwrap();
    st.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?))).unwrap().map(|r| r.unwrap()).collect()
}

fn label_rows(core: &RustCore, label: &str) -> Vec<(String, String)> {
    let conn = smart_inbox::load::open_reader(core.store().path()).unwrap();
    let mut st = conn.prepare("SELECT message_id, decided_by FROM label_assignment WHERE label_id = ?1 ORDER BY message_id").unwrap();
    st.query_map([label], |r| Ok((r.get(0)?, r.get(1)?))).unwrap().map(|r| r.unwrap()).collect()
}

fn label_spec(name: &str) -> LabelSpec {
    LabelSpec { name: name.into(), parent_id: None, colour: None, description: String::new(), scope: LabelScope::All, rules: RuleSet { conditions: vec![], match_mode: MatchMode::Any }, pinned: false }
}

#[test]
fn apply_writes_exactly_the_previewed_tab_placements() {
    let rig = setup(6);
    let core = &rig.core;
    let before = tab_rows(core);
    core.run(preview("which-tab", vec![rule("from:alpha", "Updates"), otherwise("Primary")])).unwrap();
    let rows = core.decision_preview();
    let moved: Vec<String> = rows.iter().filter(|r| r.differs).map(|r| r.message_id.clone()).collect();
    assert_eq!(moved.len(), 2, "the two alpha mails would land in Updates: {rows:?}");
    // the preview wrote nothing, and the state says so
    assert_eq!(tab_rows(core), before);
    assert_eq!(core.state("decision_preview/applied".into()).as_deref(), Some("false"));
    // Apply: exactly the previewed placements, who decided in the store's words, nothing for the others
    let out = core.run(Command::ApplyDecisionPreview).expect("Apply is served by the real core");
    assert_eq!(out.undo_entry_id, None, "the arrival path's writes carry no undo entry");
    let after = tab_rows(core);
    let wrote: Vec<(String, String, String)> = after.iter().filter(|r| !before.contains(r)).cloned().collect();
    assert_eq!(wrote.len(), 2, "{after:?}");
    for (id, tab, by) in &wrote {
        assert!(moved.contains(id), "{id} was not in the preview's moves");
        assert_eq!((tab.as_str(), by.as_str()), ("updates", "rule"));
    }
    assert_eq!(core.state("decision_preview/applied".into()).as_deref(), Some("true"));
    assert_eq!(core.state("decision_preview/placed".into()).as_deref(), Some("2"));
    let _ = &rig.account;
}

#[test]
fn a_second_apply_writes_nothing_new() {
    let rig = setup(6);
    let core = &rig.core;
    core.run(preview("which-tab", vec![rule("from:alpha", "Updates"), otherwise("Primary")])).unwrap();
    core.run(Command::ApplyDecisionPreview).unwrap();
    let once = dump(core);
    assert!(!once.is_empty());
    std::thread::sleep(std::time::Duration::from_millis(5)); // a rewrite would stamp a later time
    core.run(Command::ApplyDecisionPreview).expect("a second Apply of the same preview is not an error");
    assert_eq!(dump(core), once, "nothing was written the second time (every column, every row)");
    assert_eq!(core.state("decision_preview/placed".into()).as_deref(), Some("2"));
}

#[test]
fn a_stale_preview_is_refused_in_words_and_writes_nothing() {
    let rig = setup(6);
    let core = &rig.core;
    core.run(preview("which-tab", vec![rule("from:alpha", "Updates"), otherwise("Primary")])).unwrap();
    let before = dump(core);
    // a newer message arrives after the preview: the list the preview ran on is not the list any more
    core.store().insert_messages(vec![mail(&rig.account, 99, "alpha@example.test", "Alpha 99")]).unwrap();
    let r = core.run(Command::ApplyDecisionPreview);
    match r {
        Err(CoreError::Invalid { ref field, ref reason_words }) => {
            assert_eq!(field, "preview");
            assert!(reason_words.contains("changed") && reason_words.contains("nothing was applied"), "{reason_words}");
        }
        other => panic!("a stale preview must be refused in words: {other:?}"),
    }
    assert_eq!(dump(core), before, "nothing was written");
    assert_eq!(core.state("decision_preview/applied".into()).as_deref(), Some("false"));
    // run again on the new list: Apply works
    core.run(preview("which-tab", vec![rule("from:alpha", "Updates"), otherwise("Primary")])).unwrap();
    core.run(Command::ApplyDecisionPreview).unwrap();
    assert_eq!(core.state("decision_preview/placed".into()).as_deref(), Some("3"));
}

#[test]
fn apply_without_a_preview_is_refused_in_words() {
    let rig = setup(3);
    let r = rig.core.run(Command::ApplyDecisionPreview);
    assert!(matches!(r, Err(CoreError::Invalid { ref field, .. }) if field == "preview"), "{r:?}");
    assert!(tab_rows(&rig.core).is_empty());
    // a preview that found no mail has nothing to apply either
    let empty = setup(0);
    empty.core.run(preview("which-tab", vec![rule("vip", "Primary"), otherwise("Primary")])).expect("an empty inbox is not an error");
    let r = empty.core.run(Command::ApplyDecisionPreview);
    assert!(matches!(r, Err(CoreError::Invalid { ref field, .. }) if field == "preview"), "{r:?}");
}

#[test]
fn apply_files_the_labels_the_preview_showed() {
    let rig = setup(6);
    let core = &rig.core;
    let clients = core.store().label_create_full(label_spec("Clients"), 1).unwrap().id;
    core.run(preview("labels", vec![rule("from:alpha", "Clients"), otherwise("none")])).unwrap();
    let rows = core.decision_preview();
    let alpha: Vec<String> = rows.iter().filter(|r| r.sender.starts_with("alpha")).map(|r| r.message_id.clone()).collect();
    assert_eq!(alpha.len(), 2, "{rows:?}");
    assert!(label_rows(core, &clients).is_empty(), "the preview files nothing");
    core.run(Command::ApplyDecisionPreview).unwrap();
    let filed = label_rows(core, &clients);
    assert_eq!(filed.iter().map(|(m, _)| m.clone()).collect::<Vec<_>>(), { let mut a = alpha.clone(); a.sort(); a }, "exactly the previewed mails");
    assert!(filed.iter().all(|(_, by)| by == "rule"), "{filed:?}");
    assert_eq!(core.state("decision_preview/placed".into()).as_deref(), Some("2"));
    // a second Apply files nothing new
    let once = dump(core);
    core.run(Command::ApplyDecisionPreview).unwrap();
    assert_eq!(dump(core), once);
}

#[test]
fn apply_leaves_an_existing_filing_as_it_is() {
    let rig = setup(6);
    let core = &rig.core;
    let clients = core.store().label_create_full(label_spec("Clients"), 1).unwrap().id;
    let alpha1 = format!("{}:INBOX:1", rig.account);
    // the owner filed alpha 1 under "Clients" by hand before the preview
    core.store().write({ let (c, m) = (clients.clone(), alpha1.clone()); move |ctx| ctx.label_file_entry(&c, &m, &DecidedBy::Owner, None, None, 5) }).unwrap();
    let hand = label_rows(core, &clients);
    assert_eq!(hand, vec![(alpha1.clone(), "owner".to_string())]);
    core.run(preview("labels", vec![rule("from:alpha", "Clients"), otherwise("none")])).unwrap();
    core.run(Command::ApplyDecisionPreview).unwrap();
    let filed = label_rows(core, &clients);
    assert!(filed.contains(&(alpha1.clone(), "owner".to_string())), "the owner's own filing stays the owner's: {filed:?}");
    assert_eq!(filed.len(), 2, "the other alpha mail was filed by the rule: {filed:?}");
    assert_eq!(core.state("decision_preview/placed".into()).as_deref(), Some("1"), "only the mail that was not filed yet counts");
}

#[test]
fn two_cores_keep_their_own_preview() {
    let a = setup(6);
    let b = setup(6);
    a.core.run(preview("which-tab", vec![rule("from:alpha", "Updates"), otherwise("Primary")])).unwrap();
    assert_eq!(a.core.decision_preview().len(), 6);
    assert!(b.core.decision_preview().is_empty(), "the rows belong to the core that ran the preview");
    assert!(b.core.run(Command::ApplyDecisionPreview).is_err(), "b has no preview to apply");
    let _ = (&a.dir, &b.dir);
}

/// What leaves this Mac: nothing. A hosted model (Jev) is reached only with the owner's consent; a preview or an Apply with consent withdrawn, or with rules only,
/// connects nowhere. The listener is on this Mac and the core is pointed at it (`set_decision_loopback`); a connection to it would be a request.
#[test]
fn preview_and_apply_send_nothing_without_the_owners_consent_and_with_rules_only() {
    let rig_dir = tempfile::tempdir().unwrap();
    engine::rig::apply(&engine::rig::RigSwitches { rig: true, ..Default::default() }, &rig_dir.path().join("whoosh.sqlite"));
    let rig = setup(6);
    let core = &rig.core;
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    core.set_decision_loopback(listener.local_addr().unwrap().port());
    core.run(Command::SaveProviderKey { provider_id: "jev".into(), key: SecretText { value: "FIXTURE-jev-key-9z8y7x".into() } }).expect("the key is kept");
    let keychain = core.ai_providers().into_iter().find(|p| p.id == "jev").and_then(|p| p.keychain).expect("the key's reference");
    core.run(Command::GiveDecisionConsent { provider: DecisionProviderKind::Jev, given: true }).unwrap();
    let model = DecisionModel { provider: DecisionProviderKind::Jev, model: None, keychain: Some(keychain), decisions_enabled: vec!["which-tab".into()], question_words: Default::default(), last_run_ms: None };
    core.run(Command::SaveDecisionModel { model: model.clone() }).unwrap();
    core.run(Command::GiveDecisionConsent { provider: DecisionProviderKind::Jev, given: false }).unwrap();
    let list = vec![PreviewEntry { kind: PreviewEntryKind::Model, enabled: true, match_key: String::new(), outcome: "the tab it picks".into(), question: "Which tab?".into(), trust: Some(80) }, otherwise("Primary")];
    // consent withdrawn: the run is refused or the rules alone decide, never a request; Apply after it never connects either
    let _ = core.run(preview("which-tab", list.clone()));
    let _ = core.run(Command::ApplyDecisionPreview);
    assert!(listener.accept().is_err(), "nothing connected with the owner's consent withdrawn");
    // rules only: no model is chosen, so nothing can leave
    core.run(Command::SaveDecisionModel { model: DecisionModel { provider: DecisionProviderKind::Rules, ..model } }).unwrap();
    core.run(preview("which-tab", list)).expect("rules only runs");
    core.run(Command::ApplyDecisionPreview).expect("and applies");
    assert!(listener.accept().is_err(), "nothing connected with rules only");
}

/// The owner's budget: "The objective is <50ms for any operation". The rules-only preview on 20 mails of a 2,000-mail store, PRINTED, and failing over 50 ms. The first
/// run is the one the owner's click gets (nothing warm), so it is the one judged; four more are printed beside it. A preview WITH a model is bounded by the model's
/// own time (DECISION_PREVIEW_MS) and the page shows progress: it is not judged by this test.
#[test]
fn the_rules_only_preview_on_20_of_2000_mails_stays_under_50_ms() {
    let rig = setup(2_000);
    let core = &rig.core;
    let list = vec![rule("vip", "Primary"), rule("list", "Updates"), rule("from:alpha", "Updates"), otherwise("Primary")];
    let mut ms = Vec::new();
    for _ in 0..5 {
        let t = Instant::now();
        core.run(preview("which-tab", list.clone())).unwrap();
        ms.push(t.elapsed().as_secs_f64() * 1000.0);
    }
    assert_eq!(core.decision_preview().len(), 20);
    let budget = contracts::constants::DECISION_PREVIEW_RULES_ONLY_MS as f64;   // the ledger's row (50), generated
    println!("DECISION_PREVIEW_RULES_ONLY_MS first={:.2} then={:.2?} (20 mails of 2000, rules only, budget {budget})", ms[0], &ms[1..]);
    assert!(ms[0] <= budget, "the first rules-only preview took {:.2} ms (budget {budget} ms): {ms:?}", ms[0]);
}
