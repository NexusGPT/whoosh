//! fix-labelstreams (main88 wave): a pinned label's tab and its figures reach the app at once (the Counts and Inbox streams, no refresh call), and Z
//! works after CreateLabel, RenameLabel and MoveLabel; Z after RemoveAllLabels keeps each filing's decider. Real core, real store on a temp
//! folder, 2,000 stored mails, no network. Every command it measures prints its time ("<what>: N ms on 2000 mails") and is held under 50 ms,
//! each timed alone (the owner: "The objective is <50ms for any operation").
//!
//! Broken copies (one per behaviour; each is a single edit and names the test that then fails):
//! - S1 store/src/writer.rs `COUNTS_TABLES`: drop "label" -> a_pin_publishes_counts_and_the_label_tab_without_any_refresh
//! - S2 store/src/writer.rs `COUNTS_TABLES`: drop "label_assignment" -> a_filing_under_a_pinned_label_publishes_counts_and_the_tab_rows
//! - S3 store/src/writer.rs `publish`: the pinned labels not added (`for id in &pinned_labels {` -> `for id in &Vec::<String>::new() {`) -> a_pin_publishes_counts_and_the_label_tab_without_any_refresh
//! - S4 store/src/writer.rs `publish`: the filing veto removed (`!(filing_only && pinned_labels.is_empty())` -> `true`) -> a_filing_under_an_unpinned_label_publishes_no_counts
//! - S5 writer.rs `publish`: a second Counts event (emit the CountsChanged twice) -> an_arriving_mail_publishes_counts_once
//! - U1 label_cmds.rs `create`: no undo entry (`push_entry(ctx, &UndoAction::CreateLabel` -> an entry that is not pushed: `Ok(String::new())`) -> create_label_then_z_removes_it_and_a_second_z_reaches_the_entry_before
//! - U2 actions usecases/undo.rs `create_back`: the label stays (`ctx.label_delete_tree(label_id)` -> `Ok(0)`) -> the same test
//! - U3 actions usecases/undo.rs `rename_back`: the new name stays (`ctx.label_rename(label_id, previous_name, now)` -> `ctx.label_rename(label_id, "Customers", now)`) -> rename_label_then_z_gives_the_old_name_back_and_a_second_z_reaches_the_entry_before
//! - U4 actions usecases/undo.rs `move_back`: the old parent ignored (`previous_parent_id.clone(), place` -> `None, place` with a parent first) -> move_label_then_z_gives_the_old_parent_and_place_back_and_a_second_z_reaches_the_entry_before
//! - U5 actions usecases/undo.rs `move_back`: the place ignored (`place` -> `u32::MAX`) -> the same test (Q1 comes back last)
//! - U6 actions undo.rs `latest`: the "rename_label" decode arm removed -> rename_label_then_z_... (Z answers an unknown kind and the entry stays)
//! - U7 actions usecases/undo.rs `execute`: a refused label entry not consumed (`label_record && matches!(e, CoreError::Invalid { .. })` -> `false`) -> a_label_entry_the_tree_refuses_does_not_block_the_next_z
//! - D1 actions usecases/undo.rs: `undo::restore_deciders(ctx, &entry)?;` removed -> remove_all_labels_then_z_keeps_each_assignments_decider
//! - D2 engine label_cmds.rs `remove_all`: the deciders not kept (`"deciders", &deciders` -> `"deciders", "[]"`) -> the same test
#[allow(dead_code, unused_imports)]
#[path = "../../autoconfig/tests/support/mod.rs"]
mod detect_fakes;

use std::sync::{Arc, Mutex};
use std::time::Instant;

use contracts::*;
use detect_fakes::{FakeDns, FakeHttp, FakeProbe};
use engine::{RustCore, TestTransport};
use rusqlite::OptionalExtension;
use secrets::{MemoryKeychain, Namespace};
use store::{NewMessage, Store};

const USER: &str = "owner@example.test";
const MAILS: i64 = 2_000;
const BUDGET_MS: f64 = 50.0;

fn detector() -> Box<dyn engine::Detect> { Box::new(autoconfig::Detector::new(FakeHttp::default(), FakeDns::default(), FakeProbe::default())) }
fn kc() -> Arc<MemoryKeychain> { Arc::new(MemoryKeychain::new(Namespace::rig())) }
fn config() -> AccountConfig {
    AccountConfig {
        imap_host: "127.0.0.1".into(), imap_port: 1, imap_security: ConnectionSecurity::Ssl, smtp_host: "127.0.0.1".into(), smtp_port: 1,
        smtp_security: ConnectionSecurity::Starttls, username: USER.into(), smtp_username: USER.into(), auth_method: AuthMethod::Password, source: DetectionSource::Manual,
    }
}

fn mail(account: &str, n: i64) -> NewMessage {
    let id = format!("{account}:INBOX:{n}");
    NewMessage {
        message: Message {
            id: id.clone(), thread_id: id, account_id: account.into(), sender_name: format!("Sender {n}"), from_address: format!("sender{n}@other.example"),
            to: vec![USER.into()], subject: format!("Subject {n}"), preview: format!("Preview {n}"), received_at_ms: 1_700_000_000_000 + n * 1000, read: n % 2 == 0, archived: false,
            folder: "INBOX".into(), attachment_names: vec![], login_code: None, trackers_blocked: 0, list_unsubscribe: vec![], list_unsubscribe_one_click: false,
            reply_to_name: None, cc: vec![], bcc: vec![], is_outgoing: false,
        },
        remote_id: Some(n.to_string()), body_text: None, body_html: None, has_list_header: false, size_bytes: Some(100), tab_id: None,
    }
}
fn mid(account: &str, n: i64) -> String { format!("{account}:INBOX:{n}") }

struct Fx { core: RustCore, account: String, _dir: tempfile::TempDir }

/// One account, MAILS inbox mails (odd numbers unread), the core over it.
fn setup() -> Fx {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(&dir.path().join("whoosh.sqlite")).unwrap();
    let account = store.add_account(USER, "Owner", MailProvider::Imap, "svc\u{1f}acct", false, config(), 1).unwrap();
    store.insert_messages((1..=MAILS).map(|n| mail(&account, n)).collect()).unwrap();
    Fx { core: RustCore::with_parts(store, kc(), detector(), TestTransport::PlainLoopback), account, _dir: dir }
}

/// Runs a command, prints its time, and holds it under the budget.
fn timed(core: &RustCore, what: &str, c: Command) -> Result<CommandOutcome, CoreError> {
    let t = Instant::now();
    let out = core.run(c);
    let ms = t.elapsed().as_secs_f64() * 1000.0;
    println!("{what}: {ms:.2} ms on {MAILS} mails");
    assert!(ms < BUDGET_MS, "{what} took {ms:.2} ms, not under {BUDGET_MS} ms");
    out
}

fn spec(name: &str, parent: Option<&str>, pinned: bool) -> LabelSpec {
    LabelSpec { name: name.into(), parent_id: parent.map(str::to_string), colour: None, description: String::new(), scope: LabelScope::All, rules: RuleSet { conditions: vec![], match_mode: MatchMode::Any }, pinned }
}
fn create_full(core: &RustCore, s: LabelSpec) -> String { core.run(Command::CreateLabelFull { spec: s }).unwrap().created_id.expect("the new label's id") }
fn st(core: &RustCore, p: &str) -> Option<String> { core.state(p.into()) }
fn top_level(core: &RustCore) -> Vec<String> { core.store().labels_full().unwrap().into_iter().filter(|l| l.parent_id.is_none()).map(|l| l.id).collect() }

struct Heard(Mutex<Vec<CoreEvent>>);
impl EventSink for Heard { fn on_event(&self, event: CoreEvent) { self.0.lock().unwrap().push(event) } }
impl Heard {
    fn take(&self) -> Vec<CoreEvent> { std::mem::take(&mut *self.0.lock().unwrap()) }
}

/// Two listeners on the core's streams, as the app has them: Counts and Inbox. The events of the set-up are dropped.
fn listen(core: &RustCore) -> (Arc<Heard>, Arc<Heard>) {
    let (counts, inbox) = (Arc::new(Heard(Mutex::new(vec![]))), Arc::new(Heard(Mutex::new(vec![]))));
    core.subscribe(StreamKind::Counts, counts.clone());
    core.subscribe(StreamKind::Inbox, inbox.clone());
    counts.take();
    inbox.take();
    (counts, inbox)
}

fn counts_events(heard: &[CoreEvent]) -> Vec<Counts> {
    heard.iter().filter_map(|e| match e { CoreEvent::CountsChanged { counts } => Some(counts.clone()), _ => None }).collect()
}
fn rows_of_tab(heard: &[CoreEvent], tab: &str) -> Vec<Vec<String>> {
    heard
        .iter()
        .filter_map(|e| match e {
            CoreEvent::InboxChanged { tab_id, rows } if tab_id == tab => { let mut ids: Vec<String> = rows.iter().map(|r| r.message_id.clone()).collect(); ids.sort(); Some(ids) }
            _ => None,
        })
        .collect()
}
fn unread_of(counts: &Counts, tab: &str) -> Option<u32> { counts.tabs.iter().find(|t| t.tab_id == tab).map(|t| t.unread) }

/// A RemoveAllLabels entry under the stack: what a second Z must reach after the entry above it is undone. Returns the message.
fn entry_below(fx: &Fx) -> String {
    let core = &fx.core;
    let a = create_full(core, spec("Base", None, false));
    let m = mid(&fx.account, 41);
    core.store().label_assign(a, vec![m.clone()], 1).unwrap();
    let out = core.run(Command::RemoveAllLabels { message_ids: vec![m.clone()] }).unwrap();
    assert!(out.undo_entry_id.is_some(), "the entry below");
    assert_eq!(st(core, &format!("label_assignment/{m}")).as_deref(), Some("[]"));
    m
}

// ------------------------------------------------------------------------------------------------ streams

#[test]
fn a_pin_publishes_counts_and_the_label_tab_without_any_refresh() {
    let fx = setup();
    let core = &fx.core;
    let label = create_full(core, spec("Contracts", None, false));
    let filed = vec![mid(&fx.account, 1), mid(&fx.account, 2), mid(&fx.account, 3)];
    core.store().label_assign(label.clone(), filed.clone(), 1).unwrap();
    let (counts, inbox) = listen(core);
    timed(core, "PinLabel on", Command::PinLabel { label_id: label.clone(), pinned: true }).unwrap();
    // no counts(), no inbox(): what the streams carried is all the app would have
    let heard_counts = counts_events(&counts.take());
    assert_eq!(heard_counts.len(), 1, "one Counts event for the pin: {heard_counts:?}");
    assert_eq!(unread_of(&heard_counts[0], &label), Some(2), "the pinned label is a tab in the Counts event, with the unread mail filed under it: {:?}", heard_counts[0].tabs);
    let mut want = filed.clone();
    want.sort();
    let heard_rows = rows_of_tab(&inbox.take(), &label);
    assert_eq!(heard_rows.len(), 1, "one Inbox event for the label's tab: {heard_rows:?}");
    assert_eq!(heard_rows[0], want, "the tab's rows are the mails filed under the label");
    // and unpinning tells the strip at once too
    timed(core, "PinLabel off", Command::PinLabel { label_id: label.clone(), pinned: false }).unwrap();
    let after = counts_events(&counts.take());
    assert_eq!(after.len(), 1, "one Counts event for the unpin");
    assert_eq!(unread_of(&after[0], &label), None, "the unpinned label is no tab any more");
}

#[test]
fn a_filing_under_a_pinned_label_publishes_counts_and_the_tab_rows() {
    let fx = setup();
    let core = &fx.core;
    let label = create_full(core, spec("Kept", None, true));
    core.store().label_assign(label.clone(), vec![mid(&fx.account, 1)], 1).unwrap();
    let (counts, inbox) = listen(core);
    let t = Instant::now();
    core.store().label_assign(label.clone(), vec![mid(&fx.account, 3), mid(&fx.account, 4)], 2).unwrap();
    let ms = t.elapsed().as_secs_f64() * 1000.0;
    println!("filing two mails under a pinned label: {ms:.2} ms on {MAILS} mails");
    assert!(ms < BUDGET_MS, "the filing took {ms:.2} ms, not under {BUDGET_MS} ms");
    let heard_counts = counts_events(&counts.take());
    assert_eq!(heard_counts.len(), 1, "one Counts event for the filing: {heard_counts:?}");
    assert_eq!(unread_of(&heard_counts[0], &label), Some(2), "mails 1 and 3 are unread, 4 is read");
    let heard_rows = rows_of_tab(&inbox.take(), &label);
    assert_eq!(heard_rows.len(), 1, "one Inbox event for the label's tab");
    assert_eq!(heard_rows[0].len(), 3, "the tab lists the three mails now filed: {heard_rows:?}");
}

#[test]
fn a_filing_under_an_unpinned_label_publishes_no_counts() {
    // the label tables feed Counts, but a filing alone moves no figure while no owner label is pinned: no event on every filing
    let fx = setup();
    let core = &fx.core;
    let label = create_full(core, spec("Quiet", None, false));
    let (counts, _inbox) = listen(core);
    core.store().label_assign(label, vec![mid(&fx.account, 1), mid(&fx.account, 3)], 1).unwrap();
    let heard = counts_events(&counts.take());
    assert!(heard.is_empty(), "no pinned owner label: no Counts event for a filing, got {heard:?}");
}

#[test]
fn an_arriving_mail_publishes_counts_once() {
    // checked with a pinned owner label in place (the case the label tables could have doubled): the arrival is one write, one Counts event
    let fx = setup();
    let core = &fx.core;
    let label = create_full(core, spec("Pinned", None, true));
    core.store().label_assign(label.clone(), vec![mid(&fx.account, 1)], 1).unwrap();
    let (counts, _inbox) = listen(core);
    let t = Instant::now();
    core.store().insert_messages(vec![mail(&fx.account, MAILS + 1)]).unwrap();
    let ms = t.elapsed().as_secs_f64() * 1000.0;
    println!("one arriving mail: {ms:.2} ms on {MAILS} mails");
    assert!(ms < BUDGET_MS, "the arrival took {ms:.2} ms, not under {BUDGET_MS} ms");
    let heard = counts_events(&counts.take());
    assert_eq!(heard.len(), 1, "one Counts event for one arriving mail, got {}: {heard:?}", heard.len());
    assert_eq!(heard[0], core.store().recount().unwrap(), "and it equals a recount");
}

// ------------------------------------------------------------------------------------------------ undo

#[test]
fn create_label_then_z_removes_it_and_a_second_z_reaches_the_entry_before() {
    let fx = setup();
    let core = &fx.core;
    let m = entry_below(&fx);
    let before = st(core, "labels/count").unwrap();
    let made = timed(core, "CreateLabel", Command::CreateLabel { name: "Fresh".into(), parent_id: None }).unwrap();
    assert!(made.undo_entry_id.is_some(), "CreateLabel pushes an undo entry");
    let id = made.created_id.unwrap();
    assert_eq!(st(core, &format!("label/{id}/name")).as_deref(), Some(r#""Fresh""#));
    // a mail filed under it afterwards goes with it, and the server hears it
    let m2 = mid(&fx.account, 42);
    core.store().label_assign(id.clone(), vec![m2.clone()], 1).unwrap();
    timed(core, "Undo (CreateLabel)", Command::Undo).unwrap();
    assert_eq!(st(core, &format!("label/{id}/name")), None, "the label is gone");
    assert_eq!(st(core, "labels/count").unwrap(), before, "the label count is the earlier one");
    assert_eq!(st(core, &format!("label_assignment/{m2}")).as_deref(), Some("[]"), "its filing went with it");
    let rows = core.store().pending_actions().unwrap();
    assert!(rows.iter().any(|r| r.kind == "label" && r.message_id.as_deref() == Some(m2.as_str()) && r.payload_json.contains("\"applied\":false")), "the server hears the label leave: {rows:?}");
    timed(core, "Undo (the entry before)", Command::Undo).unwrap();
    assert_eq!(st(core, &format!("label_assignment/{m}")).as_deref(), Some(r#"["Base"]"#), "the second Z reached RemoveAllLabels");
    assert!(core.run(Command::Undo).is_err(), "nothing left to undo");
}

#[test]
fn rename_label_then_z_gives_the_old_name_back_and_a_second_z_reaches_the_entry_before() {
    let fx = setup();
    let core = &fx.core;
    let m = entry_below(&fx);
    let clients = create_full(core, spec("Clients", None, false));
    let sub = create_full(core, spec("Provanta", Some(&clients), false));
    let out = timed(core, "RenameLabel", Command::RenameLabel { label_id: clients.clone(), name: "Customers".into() }).unwrap();
    assert!(out.undo_entry_id.is_some(), "RenameLabel pushes an undo entry");
    assert_eq!(st(core, &format!("label/{sub}/path")).as_deref(), Some(r#""Customers > Provanta""#));
    timed(core, "Undo (RenameLabel)", Command::Undo).unwrap();
    assert_eq!(st(core, &format!("label/{clients}/name")).as_deref(), Some(r#""Clients""#), "the old name is back");
    assert_eq!(st(core, &format!("label/{sub}/path")).as_deref(), Some(r#""Clients > Provanta""#), "and the paths below follow");
    timed(core, "Undo (the entry before)", Command::Undo).unwrap();
    assert_eq!(st(core, &format!("label_assignment/{m}")).as_deref(), Some(r#"["Base"]"#), "the second Z reached RemoveAllLabels");
}

#[test]
fn move_label_then_z_gives_the_old_parent_and_place_back_and_a_second_z_reaches_the_entry_before() {
    let fx = setup();
    let core = &fx.core;
    let m = entry_below(&fx);
    let q1 = create_full(core, spec("Q1", None, false));
    let _q2 = create_full(core, spec("Q2", None, false));
    let _q3 = create_full(core, spec("Q3", None, false));
    let parent = create_full(core, spec("Parent", None, false));
    let order_before = top_level(core);
    let out = timed(core, "MoveLabel", Command::MoveLabel { label_id: q1.clone(), parent_id: Some(parent.clone()) }).unwrap();
    assert!(out.undo_entry_id.is_some(), "MoveLabel pushes an undo entry");
    assert_eq!(st(core, &format!("label/{q1}/parent")).as_deref(), Some(format!("\"{parent}\"").as_str()));
    timed(core, "Undo (MoveLabel)", Command::Undo).unwrap();
    assert_eq!(st(core, &format!("label/{q1}/parent")), None, "back at the top level");
    assert_eq!(top_level(core), order_before, "and at its old place among its siblings (Q1 was first of the three, not last)");
    timed(core, "Undo (the entry before)", Command::Undo).unwrap();
    assert_eq!(st(core, &format!("label_assignment/{m}")).as_deref(), Some(r#"["Base"]"#), "the second Z reached RemoveAllLabels");
}

#[test]
fn a_label_entry_the_tree_refuses_does_not_block_the_next_z() {
    let fx = setup();
    let core = &fx.core;
    let m = entry_below(&fx);
    let x = create_full(core, spec("Xeno", None, false));
    core.run(Command::RenameLabel { label_id: x.clone(), name: "Yank".into() }).unwrap();
    // the old name is taken meanwhile: the rename cannot be undone
    create_full(core, spec("Xeno", None, false));
    let refused = core.run(Command::Undo);
    assert!(matches!(refused, Err(CoreError::Invalid { .. })), "refused in words: {refused:?}");
    assert_eq!(st(core, &format!("label/{x}/name")).as_deref(), Some(r#""Yank""#), "nothing changed");
    timed(core, "Undo (the entry before)", Command::Undo).unwrap();
    assert_eq!(st(core, &format!("label_assignment/{m}")).as_deref(), Some(r#"["Base"]"#), "the refused entry did not stay on top");
}

type Filing = (String, Option<String>, Option<f64>, Option<i64>, Option<String>, Option<i64>, Option<String>);
/// The whole decider stored for one filing: who decided, the rule, the probability, when, the flow card and chain, the page entry.
fn filing(core: &RustCore, label: &str, message: &str) -> Option<Filing> {
    let (l, m) = (label.to_string(), message.to_string());
    core.store()
        .write(move |ctx| {
            Ok(ctx
                .tx
                .query_row(
                    "SELECT decided_by, rule_id, probability, decided_at_ms, flow_card, flow_chain_id, decided_entry_id FROM label_assignment WHERE label_id = ?1 AND message_id = ?2",
                    rusqlite::params![l, m],
                    |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?, r.get(6)?)),
                )
                .optional()?)
        })
        .unwrap()
}

#[test]
fn remove_all_labels_then_z_keeps_each_assignments_decider() {
    let fx = setup();
    let core = &fx.core;
    let (a, b, c) = (create_full(core, spec("ByModel", None, false)), create_full(core, spec("ByOwner", None, false)), create_full(core, spec("ByRule", None, false)));
    let m = mid(&fx.account, 51);
    core.store().label_file_entry(a.clone(), m.clone(), DecidedBy::Laya, Some("labels/model".into()), Some(0.8), 5).unwrap();
    core.store().label_file(b.clone(), m.clone(), DecidedBy::Owner, None, 6).unwrap();
    {
        let (c2, m2) = (c.clone(), m.clone());
        core.store().write(move |ctx| ctx.label_file_flow(&c2, &m2, &DecidedBy::Rule { rule_id: "rule-x".into() }, None, Some(0.5), WhyCard::Entry, Some(0), 7)).unwrap();
    }
    let before: Vec<Option<Filing>> = [&a, &b, &c].iter().map(|l| filing(core, l, &m)).collect();
    assert_eq!(before[0].as_ref().map(|f| f.0.as_str()), Some("laya"));
    assert_eq!(before[0].as_ref().and_then(|f| f.6.as_deref()), Some("labels/model"), "the model's page entry is stored");
    assert_eq!(before[2].as_ref().map(|f| f.0.as_str()), Some("rule"));
    let out = timed(core, "RemoveAllLabels", Command::RemoveAllLabels { message_ids: vec![m.clone()] }).unwrap();
    assert!(out.undo_entry_id.is_some());
    assert!(filing(core, &a, &m).is_none(), "the filings are gone");
    timed(core, "Undo (RemoveAllLabels)", Command::Undo).unwrap();
    let after: Vec<Option<Filing>> = [&a, &b, &c].iter().map(|l| filing(core, l, &m)).collect();
    assert_eq!(after, before, "each assignment is back with its decider: who decided, the rule, the probability, when, the card, the page entry");
}
