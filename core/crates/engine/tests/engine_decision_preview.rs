//! gap-preview-core: `Command::PreviewDecisionList` on the real core. The preview runs the list the PAGE holds (an unticked entry decides nothing, an edited
//! question is the one sent), on the DECISION_PREVIEW_N newest messages of the inbox, with a verdict row each, and it writes nothing.
//!
//! Mutants (single edits, each to be seen red by the lead's run):
//! - P1 decision_preview.rs `list_of`: `if !e.enabled { continue; }` removed -> the_list_given_is_the_list_used (the unticked rule decides)
//! - P2 decision_preview.rs `list_of`: the model entry's question replaced by the shipped one (`question: String::new()`) -> the_list_given_is_the_list_used (the edited question is not the one sent)
//! - P3 decision_preview.rs `preview`: `.take(n)` replaced by the whole inbox (`LIMIT ?1` removed) -> twenty_rows_each_with_a_verdict_and_a_tab
//! - P4 decision_preview.rs `preview`: the empty-inbox early return removed -> an_empty_mailbox_gives_an_empty_preview_and_a_sentence
//! - P5 decision_preview.rs `preview`: a write added (e.g. `store.label_create_full`) or the read-only connection replaced by a writer -> the_preview_writes_nothing
//! - P6 rust_core.rs `decision_preview`: returns `vec![]` again -> every test here (the rows never come back)
//! - P7 decision_preview.rs `preview`: the `(Some(_), Err(e))` arm made to run the model anyway -> a_model_needs_the_ledgers_bound_and_is_refused_in_words_without_it
#[allow(dead_code, unused_imports)]
#[path = "../../autoconfig/tests/support/mod.rs"]
mod detect_fakes;

use std::sync::{Arc, Mutex};

use contracts::*;
use detect_fakes::{FakeDns, FakeHttp, FakeProbe};
use engine::{decision_preview, RustCore, TestTransport};
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

fn setup(count: i64) -> (RustCore, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(&dir.path().join("whoosh.sqlite")).unwrap();
    let account = store.add_account(USER, "Owner", MailProvider::Imap, "svc\u{1f}acct", false, config(), 1).unwrap();
    let core = RustCore::with_parts(store, kc(), detector(), TestTransport::PlainLoopback);
    // mail 1, 4, 7... come from alpha (subject "Alpha ..."), the rest from beta
    let msgs = (1..=count).map(|n| if n % 3 == 1 { mail(&account, n, "alpha@example.test", &format!("Alpha {n}")) } else { mail(&account, n, "beta@example.test", &format!("Beta {n}")) }).collect();
    if count > 0 { core.store().insert_messages(msgs).unwrap(); }
    (core, dir)
}

fn rule(key: &str, outcome: &str, enabled: bool) -> PreviewEntry { PreviewEntry { kind: PreviewEntryKind::Rule, enabled, match_key: key.into(), outcome: outcome.into(), question: String::new(), trust: None } }
fn model(question: &str, trust: u32, enabled: bool) -> PreviewEntry { PreviewEntry { kind: PreviewEntryKind::Model, enabled, match_key: String::new(), outcome: "the tab it picks".into(), question: question.into(), trust: Some(trust) } }
fn otherwise(outcome: &str) -> PreviewEntry { PreviewEntry { kind: PreviewEntryKind::Otherwise, enabled: true, match_key: String::new(), outcome: outcome.into(), question: String::new(), trust: None } }
fn preview_cmd(page: &str, entries: Vec<PreviewEntry>) -> Command { Command::PreviewDecisionList { page_id: page.into(), entries } }

/// A model on this Mac that records every question it is asked and always answers "Updates" at 0.95.
struct Scripted { questions: Mutex<Vec<String>> }
impl Scripted {
    fn new() -> Arc<Scripted> { Arc::new(Scripted { questions: Mutex::new(vec![]) }) }
    fn asked(&self) -> Vec<String> { self.questions.lock().unwrap().clone() }
}
impl DecisionPort for Scripted {
    fn provider(&self) -> DecisionProviderKind { DecisionProviderKind::Local }
    fn decide(&self, _state: DecisionState, questions: Vec<DecisionQuestion>) -> Result<DecisionResult, CoreError> {
        let mut answers = vec![];
        for q in questions {
            match q {
                DecisionQuestion::Choice { id, prompt, .. } => {
                    self.questions.lock().unwrap().push(prompt);
                    answers.push(DecisionAnswer::Choice { id, choice: "Updates".into(), probabilities: [("Updates".to_string(), 0.95)].into() });
                }
                DecisionQuestion::Noul { id, prompt } => {
                    self.questions.lock().unwrap().push(prompt);
                    answers.push(DecisionAnswer::Noul { id, yes: true, probability: 0.95 });
                }
                DecisionQuestion::Score { id, .. } => answers.push(DecisionAnswer::Score { id, value: 0.0 }),
            }
        }
        Ok(DecisionResult { answers, answered_by: DecisionProviderKind::Local, checkpoint: None })
    }
}

struct Fixed(i64);
impl Clock for Fixed { fn now_ms(&self) -> i64 { self.0 } fn utc_offset_s(&self) -> i32 { 0 } }

fn file_bytes(store: &Store) -> (Vec<u8>, Vec<u8>) {
    let p = store.path().to_path_buf();
    let wal = std::path::PathBuf::from(format!("{}-wal", p.display()));
    (std::fs::read(&p).unwrap(), std::fs::read(&wal).unwrap_or_default())
}

#[test]
fn the_list_given_is_the_list_used() {
    let (core, _d) = setup(6);
    // ticked: the rule "from alpha" answers Updates for mail 1 and 4, entry 1
    core.run(preview_cmd("which-tab", vec![rule("from:alpha", "Updates", true), otherwise("Primary")])).unwrap();
    let rows = core.decision_preview();
    assert_eq!(rows.len(), 6);
    let alpha: Vec<&DecisionVerdict> = rows.iter().filter(|r| r.sender.starts_with("alpha")).collect();
    assert_eq!(alpha.len(), 2);
    assert!(alpha.iter().all(|r| r.entry == Some(1) && r.outcome == "Updates" && r.decided_by == "entry 1 · rule"), "{alpha:?}");
    // unticked: the same rule decides nothing, the closing entry (still number 2 on the page) decides every row
    core.run(preview_cmd("which-tab", vec![rule("from:alpha", "Updates", false), otherwise("Primary")])).unwrap();
    let rows = core.decision_preview();
    assert_eq!(rows.len(), 6);
    assert!(rows.iter().all(|r| r.entry == Some(2) && r.outcome == "Primary" && r.decided_by == "entry 2 · otherwise"), "{rows:?}");

    // an edited question is the question the model is sent
    let m = Scripted::new();
    core.set_decision_port(Some(m.clone()));
    let words = "Is this a newsletter I never read, my own words?";
    let run = decision_preview::preview(&core.store(), Some(m.clone()), &Fixed(0), "which-tab", &[model(words, 80, true), otherwise("Primary")], Ok(60_000)).unwrap();
    assert!(m.asked().iter().any(|q| q == words), "the model was asked {:?}", m.asked());
    let rows = run.rows;
    assert!(rows.iter().all(|r| r.entry == Some(1) && r.outcome == "Updates" && r.decided_by == "entry 1 · the model said Updates (0.95)"), "{rows:?}");
    // the same model entry unticked is never asked and decides nothing
    let before = m.asked().len();
    let run = decision_preview::preview(&core.store(), Some(m.clone()), &Fixed(0), "which-tab", &[model(words, 80, false), otherwise("Primary")], Ok(60_000)).unwrap();
    assert_eq!(m.asked().len(), before, "an unticked model entry sends no question");
    assert!(run.rows.iter().all(|r| r.entry == Some(2)));
}

#[test]
fn twenty_rows_each_with_a_verdict_and_a_tab() {
    let (core, _d) = setup(30);
    core.run(preview_cmd("which-tab", vec![rule("from:alpha", "Updates", true), otherwise("Primary")])).unwrap();
    let rows = core.decision_preview();
    assert_eq!(rows.len(), 20, "the ledger's DECISION_PREVIEW_N");
    let conn = smart_inbox::load::open_reader(core.store().path()).unwrap();
    let tabs: Vec<String> = smart_inbox::load::load_context(&conn).unwrap().tabs.tabs.into_iter().map(|t| t.name).collect();
    for r in &rows {
        assert!(r.entry.is_some() && !r.decided_by.is_empty() && !r.sender.is_empty() && !r.subject.is_empty(), "{r:?}");
        assert!(tabs.contains(&r.outcome), "{} is not a tab of {tabs:?}", r.outcome);
    }
    // the newest 20: mail 11..=30
    assert!(rows.iter().all(|r| r.subject.rsplit(' ').next().and_then(|n| n.parse::<i64>().ok()).is_some_and(|n| n > 10)), "{rows:?}");
}

#[test]
fn the_preview_writes_nothing() {
    let (core, _d) = setup(8);
    // round 26 (main115 cargo: failed 1 run in 2, here 14 of 25): the background the core starts writes on its own after the mails arrive, the reply
    // engine's Sources pass adding the INBOX mail folder to `reply_source` (0 -> 1 row) and then its progress (the same row changed). That is the
    // product working, not the preview; the image is taken once the background has settled, so only the preview's own writes could change it
    let before = quiet(&core);
    core.run(preview_cmd("which-tab", vec![rule("from:alpha", "Updates", true), otherwise("Primary")])).unwrap();
    core.run(preview_cmd("labels", vec![rule("from:alpha", "none", true), otherwise("none")])).unwrap();
    let m = Scripted::new();
    let run = decision_preview::preview(&core.store(), Some(m), &Fixed(0), "which-tab", &[model("Which tab?", 80, true), otherwise("Primary")], Ok(60_000)).unwrap();
    assert!(!run.rows.is_empty());
    let after = file_bytes(&core.store());
    // round 26 (main115 cargo: this assertion failed): when the bytes differ, the message names the tables whose rows differ (names and counts only)
    let changed = if after != before { tables_changed(&before, &after) } else { String::new() };
    assert_eq!(after, before, "the store is byte for byte what it was; tables whose rows changed: {changed}");
}

/// The store's image once the background is quiet: the reply engine's queued jobs ran (replies_work::settle) and the filing worker is flushed, again
/// until two images in a row are the same (a write by one pass queues the next: the Sources pass reacts to its own row).
fn quiet(core: &RustCore) -> (Vec<u8>, Vec<u8>) {
    let path = core.store().path().to_path_buf();
    let mut last = file_bytes(&core.store());
    for _ in 0..20 {
        assert!(engine::replies_work::settle(&path, std::time::Duration::from_secs(10)), "the reply engine's background did not settle in 10 s");
        core.flush_filing();
        std::thread::sleep(std::time::Duration::from_millis(50));
        let now = file_bytes(&core.store());
        if now == last { return now; }
        last = now;
    }
    panic!("the store kept changing through 20 settled passes before the preview ran");
}

/// The tables whose rows differ between two images of the store (each written to its own folder with its WAL and opened read-only): `name rows before -> after`,
/// or "none" when every table holds the same rows (the bytes moved without a row changing, a WAL checkpoint).
fn tables_changed(before: &(Vec<u8>, Vec<u8>), after: &(Vec<u8>, Vec<u8>)) -> String {
    let open = |img: &(Vec<u8>, Vec<u8>), d: &std::path::Path| {
        std::fs::write(d.join("s.sqlite"), &img.0).unwrap();
        if !img.1.is_empty() { std::fs::write(d.join("s.sqlite-wal"), &img.1).unwrap(); }
        rusqlite::Connection::open(d.join("s.sqlite")).unwrap()
    };
    let (da, db) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let (a, b) = (open(before, da.path()), open(after, db.path()));
    let names: Vec<String> = {
        let mut st = b.prepare("SELECT name FROM sqlite_master WHERE type = 'table' ORDER BY name").unwrap();
        st.query_map([], |r| r.get::<_, String>(0)).unwrap().map(|x| x.unwrap()).collect()
    };
    let dump = |c: &rusqlite::Connection, t: &str| -> Option<Vec<String>> {
        let mut st = c.prepare(&format!("SELECT * FROM \"{t}\"")).ok()?;
        let width = st.column_count();
        let mut rows: Vec<String> = st.query_map([], |r| Ok((0..width).map(|i| format!("{:?}", r.get_ref(i).map(|v| format!("{v:?}")).unwrap_or_default())).collect::<Vec<_>>().join("|"))).ok()?
            .filter_map(Result::ok).collect();
        rows.sort();
        Some(rows)
    };
    let mut out = Vec::new();
    for t in names {
        let (x, y) = (dump(&a, &t), dump(&b, &t));
        if x != y {
            // the rows that differ, by their first column (a key: an id or a source key, never a mail's text), three at most
            let (xs, ys) = (x.clone().unwrap_or_default(), y.clone().unwrap_or_default());
            let key = |r: &String| r.split('|').next().unwrap_or("").to_string();
            let mut keys: Vec<String> = xs.iter().filter(|r| !ys.contains(r)).chain(ys.iter().filter(|r| !xs.contains(r))).map(key).collect();
            keys.dedup();
            keys.truncate(3);
            out.push(format!("{t} {} -> {} rows, changed {}", xs.len(), ys.len(), keys.join(" ; ")));
        }
    }
    if out.is_empty() { "none".into() } else { out.join(", ") }
}

#[test]
fn an_empty_mailbox_gives_an_empty_preview_and_a_sentence() {
    let (core, _d) = setup(0);
    core.run(preview_cmd("which-tab", vec![rule("vip", "Primary", true), otherwise("Primary")])).expect("an empty inbox is not an error");
    assert!(core.decision_preview().is_empty());
    assert_eq!(core.state("decision_preview/words".into()).as_deref(), Some(decision_preview::EMPTY_WORDS));
}

#[test]
fn a_model_needs_the_ledgers_bound_and_is_refused_in_words_without_it() {
    let (core, _d) = setup(4);
    let m = Scripted::new();
    core.set_decision_port(Some(m.clone()));
    let entries = vec![model("Which tab?", 80, true), otherwise("Primary")];
    let r = core.run(preview_cmd("which-tab", entries.clone()));
    match ::decision::tuning::decision_preview_ms() {   // the crate, not contracts::decision (the glob import above brings that name)
        None => {
            assert!(matches!(r, Err(CoreError::Invalid { ref field, .. }) if field == "DECISION_PREVIEW_MS"), "{r:?}");
            assert!(m.asked().is_empty(), "no question left before the bound was known");
            assert!(core.state("decision_preview/words".into()).unwrap_or_default().contains("DECISION_PREVIEW_MS") || core.state("decision_preview/words".into()).unwrap_or_default().contains("preview bound"));
        }
        Some(_) => assert!(r.is_ok(), "{r:?}"),
    }
    // the same list with no model chosen needs no bound: rules only, nothing asked
    core.set_decision_port(None);
    core.run(preview_cmd("which-tab", entries)).expect("rules only");
    assert!(core.decision_preview().iter().all(|r| r.entry == Some(2) && r.outcome == "Primary"));
    assert!(m.asked().is_empty());
}

#[test]
fn an_unknown_page_is_refused_in_words() {
    let (core, _d) = setup(2);
    let r = core.run(preview_cmd("no-such-page", vec![otherwise("Primary")]));
    assert!(matches!(r, Err(CoreError::Invalid { .. })), "{r:?}");
}
