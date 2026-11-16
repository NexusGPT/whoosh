//! real-labels (main86 wave): the label record commands on the REAL core, which answered "The local engine does not do this yet." to every
//! one of them (rust_core.rs `run`, final arm). Real core, real store on a temp folder, 2,000 stored mails; no network (the write-back worker
//! has no server to reach, so the ledger rows stay where a test can read them). Each test prints the time of each command it measures; the
//! objective is under 50 ms per operation (the owner: "The objective is <50ms for any operation").
//!
//! Broken copies (one per behaviour; each is a single edit, listed with its proof in checks/proofs/g3/f31ec0ea.json and docs/lanes/real-labels.md):
//! - B1 rust_core.rs: the dispatch arm never taken (`c if label_cmds::is_command(&c) =>` -> `c if false && label_cmds::is_command(&c) =>`) -> every command test here
//! - B2 counts.rs `build`: the pinned owner label not pushed (`tabs.push(one_tab(id, figures));` -> `let _ = (id, figures);`) -> a_pinned_owner_label_is_a_tab_with_its_count_and_its_rows_and_unpinning_removes_it
//! - B3 read.rs `label_tab_query`: the label filter dropped (`label_id: Some(label)` -> `label_id: None`) -> the same test (the tab lists every inbox row)
//! - B4 label_meta.rs `label_pin`: the built-in split tab no longer follows (`self.label_tab_follow(id)` -> `Ok(())`) -> unpinning_a_built_in_label_takes_its_tab_away_and_pinning_brings_it_back
//! - B5 label_cmds.rs `remove`: no undo entry (the `Unlabel` push -> `Ok(None)`) -> remove_label_takes_it_off_tells_the_server_and_z_brings_it_back
//! - B6 label_cmds.rs `delete`: the server is not told (`carrier_refs(ctx, id)? {` -> `carrier_refs(ctx, id)?.into_iter().take(0) {`) -> delete_label_takes_the_subtree_keeps_the_mail_and_tells_the_server
//! - B7 label_cmds.rs `state`: `labels/tree` answers nothing (`["labels", "tree"] => {` -> `["labels", "tree-off"] => {`) -> the_label_state_paths_answer_as_the_test_engine_does
//! - B8 label_cmds.rs `remove_all`: no server rows (`back.push((account, l.path, r));` -> `let _ = (account, l.path, r);`) -> remove_all_labels_takes_every_one_off_and_z_brings_them_back
//! - B9 label_cmds.rs `rename`: nothing stored (`ctx.label_rename(&label_id, &n, now)?;` -> `let _ = (&label_id, &n, now);`) -> rename_label_moves_the_paths_below_and_refuses_in_words
//! - B10 label_cmds.rs `move_label`: the parent ignored (`ctx.label_reorder(&label_id, parent_id, u32::MAX)?;` -> `ctx.label_reorder(&label_id, None, u32::MAX)?;`) -> move_label_takes_the_subtree_and_refuses_in_words
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
fn tab(t: &str) -> InboxQuery { InboxQuery { tab_id: t.into(), account_id: None, label_id: None, mailbox: None, account_ids: vec![], filters: vec![], smart_folder_id: None } }

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

struct Fx { core: RustCore, account: String, dir: tempfile::TempDir }

/// One account, MAILS inbox mails (odd numbers unread), the core over it.
fn setup() -> Fx {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(&dir.path().join("whoosh.sqlite")).unwrap();
    let account = store.add_account(USER, "Owner", MailProvider::Imap, "svc\u{1f}acct", false, config(), 1).unwrap();
    store.insert_messages((1..=MAILS).map(|n| mail(&account, n)).collect()).unwrap();
    Fx { core: RustCore::with_parts(store, kc(), detector(), TestTransport::PlainLoopback), account, dir }
}

/// The same store, opened again after the core is gone (what a restart of the app does).
fn reopen(fx: Fx) -> Fx {
    let Fx { core, account, dir } = fx;
    drop(core);
    let store = Store::open(&dir.path().join("whoosh.sqlite")).unwrap();
    Fx { core: RustCore::with_parts(store, kc(), detector(), TestTransport::PlainLoopback), account, dir }
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
fn tab_ids(core: &RustCore) -> Vec<String> { core.counts().tabs.into_iter().map(|t| t.tab_id).collect() }
fn refusal(r: Result<CommandOutcome, CoreError>) -> String {
    match r {
        Err(CoreError::Invalid { reason_words, .. }) => reason_words,
        other => panic!("not a refusal in words: {other:?}"),
    }
}

#[test]
fn a_pinned_owner_label_is_a_tab_with_its_count_and_its_rows_and_unpinning_removes_it() {
    let fx = setup();
    let core = &fx.core;
    let label = create_full(core, spec("Contracts", None, false));
    assert!(!tab_ids(core).contains(&label), "an unpinned label is no tab");
    // three mails filed under it: 1 and 3 unread, 2 read
    let filed = vec![mid(&fx.account, 1), mid(&fx.account, 2), mid(&fx.account, 3)];
    core.store().label_assign(label.clone(), filed.clone(), 1).unwrap();
    timed(core, "PinLabel on", Command::PinLabel { label_id: label.clone(), pinned: true }).unwrap();
    assert_eq!(st(core, &format!("label/{label}/pinned")).as_deref(), Some("true"));
    let t = Instant::now();
    let counts = core.counts();
    println!("counts with the label tab: {:.2} ms", t.elapsed().as_secs_f64() * 1000.0);
    let figures = counts.tabs.iter().find(|t| t.tab_id == label).unwrap_or_else(|| panic!("the pinned label is a tab: {:?}", counts.tabs));
    assert_eq!(figures.unread, 2, "the unread mail filed under it");
    let t = Instant::now();
    let rows = core.inbox(tab(&label));
    let ms = t.elapsed().as_secs_f64() * 1000.0;
    println!("inbox read of the label tab: {ms:.2} ms on {MAILS} mails");
    assert!(ms < BUDGET_MS, "the label tab's read took {ms:.2} ms");
    let mut got: Vec<String> = rows.iter().map(|r| r.message_id.clone()).collect();
    got.sort();
    let mut want = filed.clone();
    want.sort();
    assert_eq!(got, want, "the tab lists exactly the mails filed under the label");
    assert!(st(core, "inbox/tabs").unwrap().contains(&format!(r#""id":"{label}","kind":"label","name":"Contracts","unread":2"#)), "{:?}", st(core, "inbox/tabs"));
    assert!(st(core, "labels/pinned").unwrap().contains(&format!(r#""id":"{label}""#)));
    timed(core, "PinLabel off", Command::PinLabel { label_id: label.clone(), pinned: false }).unwrap();
    assert!(!tab_ids(core).contains(&label), "unpinning removes the tab: {:?}", tab_ids(core));
    assert_eq!(core.counts(), core.store().recount().unwrap(), "the counts equal a recount");
}

#[test]
fn a_pin_survives_a_restart() {
    let fx = setup();
    let label = create_full(&fx.core, spec("Kept", None, false));
    fx.core.run(Command::PinLabel { label_id: label.clone(), pinned: true }).unwrap();
    let fx = reopen(fx);
    assert_eq!(st(&fx.core, &format!("label/{label}/pinned")).as_deref(), Some("true"), "the pin is stored on this Mac");
    assert!(tab_ids(&fx.core).contains(&label), "and the tab is there after the restart");
}

#[test]
fn unpinning_a_built_in_label_takes_its_tab_away_and_pinning_brings_it_back() {
    let fx = setup();
    let core = &fx.core;
    assert!(tab_ids(core).contains(&"updates".to_string()), "{:?}", tab_ids(core));
    timed(core, "PinLabel updates off", Command::PinLabel { label_id: "updates".into(), pinned: false }).unwrap();
    assert!(!tab_ids(core).contains(&"updates".to_string()), "{:?}", tab_ids(core));
    timed(core, "PinLabel updates on", Command::PinLabel { label_id: "updates".into(), pinned: true }).unwrap();
    assert!(tab_ids(core).contains(&"updates".to_string()), "{:?}", tab_ids(core));
}

#[test]
fn pin_refuses_an_unknown_label_and_a_view() {
    let fx = setup();
    assert!(matches!(fx.core.run(Command::PinLabel { label_id: "nope".into(), pinned: true }), Err(CoreError::NotFound { .. })));
    assert!(matches!(fx.core.run(Command::PinLabel { label_id: "all".into(), pinned: true }), Err(CoreError::Invalid { .. })), "a view is not a label");
}

#[test]
fn create_label_makes_a_label_and_a_sub_label_and_refuses_in_words() {
    let fx = setup();
    let core = &fx.core;
    let clients = timed(core, "CreateLabel", Command::CreateLabel { name: "Clients".into(), parent_id: None }).unwrap().created_id.unwrap();
    let sub = core.run(Command::CreateLabel { name: " Provanta ".into(), parent_id: Some(clients.clone()) }).unwrap().created_id.unwrap();
    assert_eq!(st(core, &format!("label/{sub}/path")).as_deref(), Some(r#""Clients > Provanta""#));
    assert_eq!(st(core, &format!("label/{sub}/parent")), Some(format!("\"{clients}\"")));
    assert_eq!(st(core, &format!("label/{clients}/kind")).as_deref(), Some(r#""owner""#));
    assert_eq!(refusal(core.run(Command::CreateLabel { name: "a > b".into(), parent_id: None })), "a label name cannot hold \">\"; make the parent first");
    assert_eq!(refusal(core.run(Command::CreateLabel { name: "  ".into(), parent_id: None })), "a label needs a name");
    assert_eq!(refusal(core.run(Command::CreateLabel { name: "clients".into(), parent_id: None })), "\"clients\" exists already here");
    assert!(matches!(core.run(Command::CreateLabel { name: "X".into(), parent_id: Some("nope".into()) }), Err(CoreError::NotFound { .. })));
    let mut parent = sub.clone();
    for name in ["L3", "L4", "L5"] {
        parent = core.run(Command::CreateLabel { name: name.into(), parent_id: Some(parent.clone()) }).unwrap().created_id.unwrap();
    }
    assert!(refusal(core.run(Command::CreateLabel { name: "L6".into(), parent_id: Some(parent) })).starts_with("a label can be at most 5 levels deep;"));
    let fx = reopen(fx);
    assert_eq!(st(&fx.core, &format!("label/{sub}/name")).as_deref(), Some(r#""Provanta""#), "the label is stored on this Mac");
}

#[test]
fn create_label_full_writes_the_sheet_and_a_pinned_one_is_a_tab() {
    let fx = setup();
    let core = &fx.core;
    let mut s = spec("Deals", None, true);
    s.description = "offers I asked for".into();
    let id = timed(core, "CreateLabelFull", Command::CreateLabelFull { spec: s }).unwrap().created_id.unwrap();
    assert_eq!(st(core, &format!("label/{id}/description")).as_deref(), Some(r#""offers I asked for""#));
    assert!(tab_ids(core).contains(&id), "pinned at birth: a tab");
    assert_eq!(refusal(core.run(Command::CreateLabelFull { spec: spec("deals", None, false) })), "there is already a label named \"deals\" there");
    assert!(matches!(core.run(Command::CreateLabelFull { spec: spec("Sub", Some("nope"), false) }), Err(CoreError::NotFound { .. })));
}

#[test]
fn update_label_replaces_the_fields_and_a_built_in_keeps_its_name() {
    let fx = setup();
    let core = &fx.core;
    let id = create_full(core, spec("Reply soon", None, false));
    let mut s = spec("Reply now", None, true);
    s.description = "mail that waits for me".into();
    timed(core, "UpdateLabel", Command::UpdateLabel { label_id: id.clone(), spec: s }).unwrap();
    assert_eq!(st(core, &format!("label/{id}/name")).as_deref(), Some(r#""Reply now""#));
    assert_eq!(st(core, &format!("label/{id}/description")).as_deref(), Some(r#""mail that waits for me""#));
    assert!(tab_ids(core).contains(&id), "the sheet's pin makes it a tab");
    let words = refusal(core.run(Command::UpdateLabel { label_id: "updates".into(), spec: spec("News", None, true) }));
    assert!(words.contains("built-in"), "{words}");
    assert!(matches!(core.run(Command::UpdateLabel { label_id: "nope".into(), spec: spec("X", None, false) }), Err(CoreError::NotFound { .. })));
    // the sheet unpins a built-in: its tab goes
    core.run(Command::UpdateLabel { label_id: "updates".into(), spec: spec("Updates", None, false) }).unwrap();
    assert!(!tab_ids(core).contains(&"updates".to_string()), "{:?}", tab_ids(core));
}

#[test]
fn rename_label_moves_the_paths_below_and_refuses_in_words() {
    let fx = setup();
    let core = &fx.core;
    let clients = create_full(core, spec("Clients", None, false));
    let sub = create_full(core, spec("Provanta", Some(&clients), false));
    create_full(core, spec("Partners", None, false));
    timed(core, "RenameLabel", Command::RenameLabel { label_id: clients.clone(), name: "Customers".into() }).unwrap();
    assert_eq!(st(core, &format!("label/{sub}/path")).as_deref(), Some(r#""Customers > Provanta""#));
    assert_eq!(refusal(core.run(Command::RenameLabel { label_id: clients.clone(), name: "partners".into() })), "\"partners\" exists already here");
    assert!(refusal(core.run(Command::RenameLabel { label_id: "updates".into(), name: "News".into() })).contains("built-in"));
    let fx = reopen(fx);
    assert_eq!(st(&fx.core, &format!("label/{clients}/name")).as_deref(), Some(r#""Customers""#));
}

#[test]
fn move_label_takes_the_subtree_and_refuses_in_words() {
    let fx = setup();
    let core = &fx.core;
    let work = create_full(core, spec("Work", None, false));
    let clients = create_full(core, spec("Clients", None, false));
    let sub = create_full(core, spec("Provanta", Some(&clients), false));
    timed(core, "MoveLabel", Command::MoveLabel { label_id: clients.clone(), parent_id: Some(work.clone()) }).unwrap();
    assert_eq!(st(core, &format!("label/{clients}/parent")), Some(format!("\"{work}\"")));
    assert_eq!(st(core, &format!("label/{sub}/path")).as_deref(), Some(r#""Work > Clients > Provanta""#));
    assert_eq!(refusal(core.run(Command::MoveLabel { label_id: work.clone(), parent_id: Some(sub.clone()) })), "\"Work\" cannot go inside itself");
    assert_eq!(refusal(core.run(Command::MoveLabel { label_id: "updates".into(), parent_id: Some(work.clone()) })), "Updates is a built-in category: it stays at the top level");
    core.run(Command::MoveLabel { label_id: clients.clone(), parent_id: None }).unwrap();
    assert_eq!(st(core, &format!("label/{clients}/parent")), None, "back at the top level");
}

#[test]
fn reorder_label_puts_it_under_a_new_parent_at_its_place() {
    let fx = setup();
    let core = &fx.core;
    let a = create_full(core, spec("Alpha", None, false));
    let b = create_full(core, spec("Beta", None, false));
    let c = create_full(core, spec("Gamma", Some(&a), false));
    timed(core, "ReorderLabel", Command::ReorderLabel { label_id: b.clone(), new_parent_id: Some(a.clone()), new_index: 0 }).unwrap();
    assert_eq!(st(core, &format!("label/{b}/parent")), Some(format!("\"{a}\"")));
    assert_eq!(st(core, &format!("label/{b}/order")).as_deref(), Some("0"));
    assert_eq!(st(core, &format!("label/{c}/order")).as_deref(), Some("1"));
    assert!(matches!(core.run(Command::ReorderLabel { label_id: "nope".into(), new_parent_id: None, new_index: 0 }), Err(CoreError::NotFound { .. })));
    let fx = reopen(fx);
    assert_eq!(st(&fx.core, &format!("label/{b}/parent")), Some(format!("\"{a}\"")), "the order is stored on this Mac");
}

#[test]
fn delete_label_takes_the_subtree_keeps_the_mail_and_tells_the_server() {
    let fx = setup();
    let core = &fx.core;
    let clients = create_full(core, spec("Clients", None, false));
    let sub = create_full(core, spec("Provanta", Some(&clients), false));
    let m = mid(&fx.account, 7);
    core.store().label_assign(sub.clone(), vec![m.clone()], 1).unwrap();
    let before = st(core, "labels/count").unwrap().parse::<u32>().unwrap();
    timed(core, "DeleteLabel", Command::DeleteLabel { label_id: clients.clone() }).unwrap();
    assert_eq!(st(core, "labels/count").unwrap().parse::<u32>().unwrap(), before - 2, "the label and the one under it");
    assert_eq!(st(core, &format!("label/{sub}/name")), None);
    assert!(core.store().message(&m).unwrap().is_some(), "the mail stays");
    assert_eq!(st(core, &format!("label_assignment/{m}")).as_deref(), Some("[]"), "and loses the label");
    let rows = core.store().pending_actions().unwrap();
    assert!(rows.iter().any(|r| r.kind == "label" && r.message_id.as_deref() == Some(m.as_str()) && r.payload_json.contains("\"applied\":false")), "the server hears the removal: {rows:?}");
    assert!(refusal(core.run(Command::DeleteLabel { label_id: "updates".into() })).contains("built-in"));
    assert!(matches!(core.run(Command::DeleteLabel { label_id: "nope".into() }), Err(CoreError::NotFound { .. })));
}

/// Diagnostic for the DeleteLabel time: the three store steps the command is made of, each timed on its own on the same 2,000 mails,
/// so a run that is over the budget says which step it is (the read of the tree, the one write, or the cascade inside it).
#[test]
fn delete_label_steps_are_each_under_the_budget() {
    let fx = setup();
    let core = &fx.core;
    let clients = create_full(core, spec("Clients", None, false));
    let sub = create_full(core, spec("Provanta", Some(&clients), false));
    core.store().label_assign(sub.clone(), vec![mid(&fx.account, 7)], 1).unwrap();
    let t = Instant::now();
    let all = core.store().labels_full().unwrap();
    let read_ms = t.elapsed().as_secs_f64() * 1000.0;
    let t = Instant::now();
    let gone = core.store().label_delete_tree(clients.clone()).unwrap();
    let delete_ms = t.elapsed().as_secs_f64() * 1000.0;
    println!("DeleteLabel steps: labels_full {read_ms:.2} ms ({} labels), label_delete_tree {delete_ms:.2} ms ({gone} labels)", all.len());
    assert_eq!(gone, 2);
    assert!(read_ms < BUDGET_MS, "labels_full took {read_ms:.2} ms, not under {BUDGET_MS} ms");
    assert!(delete_ms < BUDGET_MS, "label_delete_tree took {delete_ms:.2} ms, not under {BUDGET_MS} ms");
}

#[test]
fn remove_label_takes_it_off_tells_the_server_and_z_brings_it_back() {
    let fx = setup();
    let core = &fx.core;
    let label = create_full(core, spec("Clients", None, false));
    let (m1, m2) = (mid(&fx.account, 11), mid(&fx.account, 12));
    core.store().label_assign(label.clone(), vec![m1.clone()], 1).unwrap();
    let out = timed(core, "RemoveLabel", Command::RemoveLabel { message_ids: vec![m1.clone(), m2.clone()], label_id: label.clone() }).unwrap();
    assert!(out.undo_entry_id.is_some(), "one undo entry");
    assert_eq!(st(core, &format!("label_assignment/{m1}")).as_deref(), Some("[]"));
    assert_eq!(st(core, &format!("label_removed/{m1}/{label}")).as_deref(), Some("true"), "the owner's removal is kept: no rule files it back");
    let rows = core.store().pending_actions().unwrap();
    assert!(rows.iter().any(|r| r.kind == "label" && r.message_id.as_deref() == Some(m1.as_str()) && r.payload_json.contains("\"applied\":false")), "the server hears it: {rows:?}");
    assert!(!rows.iter().any(|r| r.message_id.as_deref() == Some(m2.as_str())), "a message that did not carry it gets no row");
    core.run(Command::Undo).unwrap();
    assert_eq!(st(core, &format!("label_assignment/{m1}")).as_deref(), Some(r#"["Clients"]"#), "Z brings it back");
    let nothing = core.run(Command::RemoveLabel { message_ids: vec![m2.clone()], label_id: label.clone() }).unwrap();
    assert_eq!(nothing.undo_entry_id, None, "a label the message does not carry: no-op");
}

#[test]
fn remove_all_labels_takes_every_one_off_and_z_brings_them_back() {
    let fx = setup();
    let core = &fx.core;
    let a = create_full(core, spec("Alpha", None, false));
    let b = create_full(core, spec("Beta", None, false));
    let m = mid(&fx.account, 21);
    core.store().label_assign(a.clone(), vec![m.clone()], 1).unwrap();
    core.store().label_assign(b.clone(), vec![m.clone()], 1).unwrap();
    let out = timed(core, "RemoveAllLabels", Command::RemoveAllLabels { message_ids: vec![m.clone()] }).unwrap();
    assert!(out.undo_entry_id.is_some());
    assert_eq!(st(core, &format!("label_assignment/{m}")).as_deref(), Some("[]"));
    assert_eq!(core.store().pending_actions().unwrap().iter().filter(|r| r.kind == "label" && r.message_id.as_deref() == Some(m.as_str())).count(), 2, "one server row per label");
    core.run(Command::Undo).unwrap();
    assert_eq!(st(core, &format!("label_assignment/{m}")).as_deref(), Some(r#"["Alpha","Beta"]"#));
    let fx = reopen(fx);
    assert_eq!(st(&fx.core, &format!("label_assignment/{m}")).as_deref(), Some(r#"["Alpha","Beta"]"#), "kept on this Mac");
}

#[test]
fn the_label_state_paths_answer_as_the_test_engine_does() {
    let fx = setup();
    let core = &fx.core;
    let clients = create_full(core, spec("Clients", None, true));
    let sub = create_full(core, spec("Provanta", Some(&clients), false));
    core.store().label_assign(sub.clone(), vec![mid(&fx.account, 1)], 1).unwrap();
    let t = Instant::now();
    let tree = st(core, "labels/tree").expect("labels/tree");
    println!("labels/tree: {:.2} ms", t.elapsed().as_secs_f64() * 1000.0);
    assert!(tree.contains(r#""id":"updates","kind":"built_in","name":"Updates""#), "{tree}");
    assert!(!tree.contains(r#""id":"primary""#), "a view is no label: {tree}");
    assert!(tree.contains(&format!(r#"{{"children":[],"depth":2,"id":"{sub}","kind":"owner","name":"Provanta","order":0,"path":"Clients > Provanta","pinned":false,"scope":"all","unread":1}}"#)), "{tree}");
    assert_eq!(st(core, &format!("label/{clients}/depth")).as_deref(), Some("1"));
    assert_eq!(st(core, &format!("label/{clients}/scope")).as_deref(), Some(r#""all""#));
    assert_eq!(st(core, &format!("label/{clients}/colour")), None);
    assert_eq!(st(core, &format!("label/{clients}/rule_sets")).as_deref(), Some("0"));
    assert_eq!(st(core, &format!("labels/{sub}/rerun_count")).as_deref(), Some("1"));
    assert_eq!(st(core, "label/primary/name"), None, "a view is no label");
    assert_eq!(st(core, &format!("label_removed/{}/{sub}", mid(&fx.account, 1))).as_deref(), Some("false"));
}
