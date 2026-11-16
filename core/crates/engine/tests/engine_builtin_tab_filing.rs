//! Lane hid-labels, round 13 (owner, main106: "when quitting, it doesn't seem to keep the mails"): the Needs Reply tab read "All done" while 25 mails were
//! filed under that built-in label. A built-in label is also a TAB; a tab lists the mail whose `message_category` row names it, and the flow files by
//! `label_assignment` alone, so the mail was never on its tab and the strip counted none. Filing under a built-in label now also puts the mail on its tab
//! (`WriteCtx::builtin_tab_put`), taking it off takes it off the tab, and opening the core puts the filings of an earlier build there too
//! (`backfill_builtin_tabs`). Real store, reopened: the rows and the count are the same after the reopen.
//!
//! Mutants (single edits; each must go red, a run of the lead's):
//! - H1 label_meta.rs `label_file_entry`: the `builtin_tab_put` call is removed -> filed_mail_is_on_the_tab_and_stays_after_a_reopen, the_strip_counts_what_the_tab_lists
//! - H2 label_meta.rs `seed_builtin_labels`: the `backfill_builtin_tabs` call is removed -> an_earlier_builds_filings_reach_the_tab_when_the_core_opens
//! - H3 labels.rs `label_unassign`: the `builtin_tab_drop` loop is removed -> taking_the_mail_off_the_label_takes_it_off_the_tab
#[allow(dead_code, unused_imports)]
#[path = "../../autoconfig/tests/support/mod.rs"]
mod detect_fakes;

use std::sync::Arc;

use contracts::*;
use detect_fakes::{FakeDns, FakeHttp, FakeProbe};
use engine::{AiStep, RustCore, TestTransport};
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
fn id_of(account: &str, key: &str) -> String { format!("{account}:{key}") }


fn spec(name: &str, description: &str, words: Option<&str>) -> LabelSpec {
    let conditions = words.map(|w| vec![RuleCondition::SubjectContains { words: w.into() }]).unwrap_or_default();
    LabelSpec { name: name.into(), parent_id: None, colour: None, description: description.into(), scope: LabelScope::All, rules: RuleSet { conditions, match_mode: MatchMode::Any }, pinned: false }
}


fn label_of(core: &RustCore, id: &str) -> MailLabel { core.store().labels_full().unwrap().into_iter().find(|l| l.id == id).unwrap_or_else(|| panic!("no label {id}")) }

fn list_mail(account: &str, key: &str, thread: &str, subject: &str, at: i64) -> NewMessage {
    NewMessage { has_list_header: true, ..mail(account, key, thread, "news@shop.example", subject, at) }
}


fn open(dir: &std::path::Path) -> RustCore {
    let store = Store::open(&dir.join("whoosh.sqlite")).unwrap();
    RustCore::with_parts(store, Arc::new(MemoryKeychain::new(Namespace::rig())), detector(), TestTransport::PlainLoopback)
}

fn tab(core: &RustCore, id: &str) -> Vec<String> {
    core.inbox(InboxQuery { tab_id: id.into(), account_id: None, label_id: None, mailbox: None, account_ids: vec![], filters: vec![], smart_folder_id: None })
        .into_iter().map(|r| r.message_id).collect()
}

fn tab_count(core: &RustCore, id: &str) -> u64 { core.store().tab_count(id).unwrap() }

/// Two inbound mails, both filed under the built-in Needs Reply the way the label AI files them (decided by the on-device model, Apple's, stored as laya).
fn filed(dir: &std::path::Path) -> (String, Vec<String>) {
    let core = open(dir);
    let account = core.store().add_account(USER, "Owner", MailProvider::Imap, "svc\u{1f}acct", false, config(1, 1), 1).unwrap();
    core.store().insert_messages(vec![
        mail(&account, "INBOX:1", "t1", "ann@client.example", "Can you answer?", 1_100),
        mail(&account, "INBOX:2", "t2", "bob@client.example", "Need your decision", 1_200),
        mail(&account, "INBOX:3", "t3", "cy@client.example", "FYI", 1_300),
    ]).unwrap();
    let ids = vec![id_of(&account, "INBOX:1"), id_of(&account, "INBOX:2")];
    for m in &ids {
        core.store().label_file("needs-reply".into(), m.clone(), DecidedBy::Apple, Some(0.9), 5_000).unwrap();
    }
    (account, ids)
}

#[test]
fn filed_mail_is_on_the_tab_and_stays_after_a_reopen() {
    let dir = tempfile::tempdir().unwrap();
    let (_account, ids) = filed(dir.path());
    let before = { let core = open(dir.path()); let mut v = tab(&core, "needs-reply"); v.sort(); v };
    let mut want = ids.clone();
    want.sort();
    assert_eq!(before, want, "the tab lists the mail filed under it");
    // quit and reopen: the same rows through the same list query
    let core = open(dir.path());
    let mut after = tab(&core, "needs-reply");
    after.sort();
    assert_eq!(after, want, "after a reopen the tab still lists the filed mail");
}

/// (H1b) The tab holds the mail the moment it is filed, on the same open core: the other tests reopen the core first, and opening runs
/// `backfill_builtin_tabs`, which puts every filing on its tab, so they stayed green with the `builtin_tab_put` call removed (c7c52ea4 m0).
/// Here the core that files is the core that reads, and nothing runs the backfill between the two.
#[test]
fn a_mail_just_filed_is_on_the_tab_with_no_reopen_between() {
    let dir = tempfile::tempdir().unwrap();
    let core = open(dir.path());
    let account = core.store().add_account(USER, "Owner", MailProvider::Imap, "svc\u{1f}acct", false, config(1, 1), 1).unwrap();
    core.store().insert_messages(vec![mail(&account, "INBOX:1", "t1", "ann@client.example", "Can you answer?", 1_100)]).unwrap();
    let id = id_of(&account, "INBOX:1");
    assert!(tab(&core, "needs-reply").is_empty(), "nothing is filed yet");
    core.store().label_file("needs-reply".into(), id.clone(), DecidedBy::Apple, Some(0.9), 5_000).unwrap();
    assert_eq!(tab(&core, "needs-reply"), vec![id], "the mail is on its tab at once, with no backfill between the filing and the read");
    assert_eq!(tab_count(&core, "needs-reply"), 1, "and the strip counts it at once");
}

#[test]
fn the_strip_counts_what_the_tab_lists() {
    let dir = tempfile::tempdir().unwrap();
    let (_account, ids) = filed(dir.path());
    let core = open(dir.path());
    assert_eq!(tab_count(&core, "needs-reply"), ids.len() as u64, "the strip's figure is the list's size");
    assert_eq!(tab(&core, "primary").len(), 1, "the mail the AI put under Needs Reply left Primary; the unfiled one stays");
}

#[test]
fn an_earlier_builds_filings_reach_the_tab_when_the_core_opens() {
    let dir = tempfile::tempdir().unwrap();
    let (_account, ids) = filed(dir.path());
    // what main106 left on the owner's Mac: the filings, no tab row (message_category empty)
    {
        let core = open(dir.path());
        core.store().write(|ctx| { ctx.tx.execute("DELETE FROM message_category", []).map_err(|e| store::StoreError::Invalid(e.to_string()))?; Ok(()) }).unwrap();
        assert!(tab(&core, "needs-reply").is_empty(), "the state of the owner's Mac: filed, and the tab empty");
    }
    let core = open(dir.path());   // opening the core runs the built-in seeding and the backfill
    let mut got = tab(&core, "needs-reply");
    got.sort();
    let mut want = ids.clone();
    want.sort();
    assert_eq!(got, want, "the filings reach the tab at the next launch");
    assert_eq!(tab_count(&core, "needs-reply"), ids.len() as u64);
}

#[test]
fn taking_the_mail_off_the_label_takes_it_off_the_tab() {
    let dir = tempfile::tempdir().unwrap();
    let (_account, ids) = filed(dir.path());
    let core = open(dir.path());
    core.store().label_unassign("needs-reply".into(), vec![ids[0].clone()]).unwrap();
    assert_eq!(tab(&core, "needs-reply"), vec![ids[1].clone()]);
    assert!(tab(&core, "primary").contains(&ids[0]), "the unfiled mail is back in Primary");
}
