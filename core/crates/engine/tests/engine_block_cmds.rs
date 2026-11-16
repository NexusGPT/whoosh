//! fix-blockforget (docs/lanes/fix-blockforget.md): the REAL engine serves BlockSender (it answered "The local engine does not do this yet.")
//! and the contract's `contact_messages` (it answered an empty list for every contact). Every test runs a store on this Mac with no mail
//! server and no socket. Each test prints `PASS <case>` lines, and `TIME <what> <ms> ms` for each command on a store of 2,000 mails (the
//! bar is 50 ms; run with `-- --nocapture`, release profile, one test at a time for the timed ones).
//!
//! What the test engine (Sources/CoreBridge/FixtureLabels.swift `blockSenderLocked`, FixtureCore.swift `contactMessages`) and the contract
//! say, and these tests hold the real engine to: a blocked sender is one row of the block table, read back through `blocks()` and the state
//! paths block/count, block/list, block/<address>/blocked and message/<id>/junk; the address is trimmed and lower-cased; a second block
//! changes nothing; a non-address is refused in words and writes nothing; no undo entry; the mail already in the inbox stays where it is;
//! nothing is queued for the server. A contact's mail is every stored mail from any address of its card, newest first.
//!
//! BROKEN COPIES (each `find` occurs once in its file; the named test goes red):
//! - K1 core/crates/engine/src/block_cmds.rs `store.block_add(BlockTarget::Sender { address }, now_ms())?;` => `let _ = address;`:
//!   a_blocked_sender_is_kept_read_back_by_every_path_idempotent_and_survives_a_restart
//! - K2 core/crates/engine/src/block_cmds.rs `if parts.len() != 2 || parts[0].is_empty() || !parts[1].contains('.') {` => `if parts.is_empty() {`:
//!   what_is_not_a_full_address_is_refused_in_words_and_nothing_is_written
//! - K3 core/crates/engine/src/block_cmds.rs `let a = address.trim().to_lowercase();` => `let a = address.to_string();`:
//!   a_blocked_sender_is_kept_read_back_by_every_path_idempotent_and_survives_a_restart
//! - C1 core/crates/engine/src/rust_core.rs `self.store.contact_mail_ids(&contact_id).and_then(|ids| self.store.rows_for_messages(&ids)).unwrap_or_default()` => `vec![]`:
//!   a_contacts_mail_is_listed_for_a_person_and_for_a_card_newest_first_and_survives_a_restart
//! - C2 core/crates/store/src/contacts.rs `held.unwrap_or_else(|| json_strings(&[address]))` => `held.unwrap_or_default()`: the same test (a person with no card)
//! - C3 core/crates/store/src/contacts.rs `.query_row([&address], |r| r.get(0))` => `.query_row(["no-such-address"], |r| r.get(0))`: the same test (a merged person by its other address)
#[allow(dead_code, unused_imports)]
#[path = "../../autoconfig/tests/support/mod.rs"]
mod detect_fakes;

use std::sync::Arc;
use std::time::{Duration, Instant};

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
fn msg(account: &str, id: &str, thread: &str, from: &str, name: &str, at: i64) -> NewMessage {
    NewMessage {
        message: Message {
            id: id.into(), thread_id: thread.into(), account_id: account.into(), sender_name: name.into(), from_address: from.into(), to: vec![USER.into()],
            subject: format!("Subject {id}"), preview: format!("Preview {id}"), received_at_ms: at, read: false, archived: false, folder: "INBOX".into(),
            attachment_names: vec![], login_code: None, trackers_blocked: 0, list_unsubscribe: vec![], list_unsubscribe_one_click: false,
            reply_to_name: None, cc: vec![], bcc: vec![], is_outgoing: false,
        },
        remote_id: None, body_text: None, body_html: None, has_list_header: false, size_bytes: Some(100), tab_id: None,
    }
}
fn open_core(path: &std::path::Path) -> (RustCore, String) {
    let store = Store::open(path).unwrap();
    let id = match store.account_summaries().unwrap().first() {
        Some(a) => a.id.clone(),
        None => store.add_account(USER, "Owner", MailProvider::Imap, "svc\u{1f}acct", false, config(), 1).unwrap(),
    };
    (RustCore::with_parts(store, kc(), detector(), TestTransport::PlainLoopback), id)
}
/// 2,000 mails: m1 and m2 from ann@ext.test, m3 from bob@ext.test, m4 from ann.work@corp.test, m5 from promo@spam.test, and 1,995 bulk mails
/// from 40 senders.
fn big() -> (RustCore, String, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    let (core, id) = open_core(&dir.path().join("whoosh.sqlite"));
    let mut batch = vec![
        msg(&id, "m1", "t1", "ann@ext.test", "Ann", 100), msg(&id, "m2", "t1", "ann@ext.test", "Ann", 110), msg(&id, "m3", "t2", "bob@ext.test", "Bob", 120),
        msg(&id, "m4", "t3", "ann.work@corp.test", "Ann W", 130), msg(&id, "m5", "t4", "promo@spam.test", "Promo", 140),
    ];
    batch.extend((0..1995).map(|i| msg(&id, &format!("b{i}"), &format!("bt{}", i / 3), &format!("sender{}@bulk.test", i % 40), "Bulk", 1000 + i as i64)));
    core.store().insert_messages(batch).unwrap();
    (core, id, dir)
}
fn reopen(core: RustCore, dir: &tempfile::TempDir) -> RustCore {
    drop(core);
    open_core(&dir.path().join("whoosh.sqlite")).0
}
fn timed<T>(what: &str, f: impl FnOnce() -> T) -> T {
    let t0 = Instant::now();
    let r = f();
    let ms = t0.elapsed().as_secs_f64() * 1000.0;
    println!("TIME {what} {ms:.1} ms");
    assert!(ms < 50.0, "{what} took {ms:.1} ms, the bar is 50");
    r
}
fn invalid(r: Result<CommandOutcome, CoreError>) -> (String, String) {
    match r { Err(CoreError::Invalid { field, reason_words }) => (field, reason_words), other => panic!("not an Invalid refusal: {other:?}") }
}
fn st(core: &RustCore, p: &str) -> Option<String> { core.state(p.to_string()) }
fn count(core: &RustCore, sql: &str) -> i64 {
    let sql = sql.to_string();
    core.store().write(move |c| Ok(c.tx.query_row(&sql, [], |r| r.get::<_, i64>(0))?)).unwrap()
}
fn sender(address: &str) -> BlockTarget { BlockTarget::Sender { address: address.into() } }
fn ids(rows: Vec<InboxRow>) -> Vec<String> { rows.into_iter().map(|r| r.message_id).collect() }

// ---------------------------------------------------------------- BlockSender

#[test]
fn a_blocked_sender_is_kept_read_back_by_every_path_idempotent_and_survives_a_restart() {
    let (core, _id, dir) = big();
    let (undo_before, pending_before) = (count(&core, "SELECT count(*) FROM undo_entry"), count(&core, "SELECT count(*) FROM pending_action"));
    assert_eq!(st(&core, "block/count").as_deref(), Some("0"));
    assert!(st(&core, "message/m1/junk").unwrap().contains("\"blocked\":false"), "{:?}", st(&core, "message/m1/junk"));

    let out = timed("BlockSender (2,000 mails)", || core.run(Command::BlockSender { address: "  Ann@EXT.test ".into() })).expect("BlockSender is served");
    assert_eq!((out.undo_entry_id, out.created_id), (None, None), "no undo entry, nothing created (the test engine's answer)");
    // the write read back: the trait's list and the four state paths
    assert_eq!(core.blocks().into_iter().map(|b| b.target).collect::<Vec<_>>(), vec![sender("ann@ext.test")], "trimmed and lower case");
    assert_eq!(st(&core, "block/count").as_deref(), Some("1"));
    assert_eq!(st(&core, "block/list").as_deref(), Some("[\"sender:ann@ext.test\"]"));
    assert_eq!(st(&core, "block/ann@ext.test/blocked").as_deref(), Some("true"));
    assert_eq!(st(&core, "block/bob@ext.test/blocked").as_deref(), Some("false"), "another sender is not blocked");
    assert_eq!(st(&core, "block/someone@ext.test/blocked").as_deref(), Some("false"), "a sender block is not a domain block");
    // the junk filter reads the same row: the sender's mail is judged blocked, another sender's is not
    assert!(st(&core, "message/m1/junk").unwrap().contains("\"blocked\":true"), "{:?}", st(&core, "message/m1/junk"));
    assert!(st(&core, "message/m3/junk").unwrap().contains("\"blocked\":false"), "{:?}", st(&core, "message/m3/junk"));
    println!("PASS block-sender-kept-trimmed-lower-case-read-back-by-blocks-and-four-state-paths");
    // the mail already in the inbox stays where it is; no undo entry; nothing queued for the server
    for m in ["m1", "m2"] {
        let stored = core.message(m.into()).expect("still stored");
        assert_eq!((stored.folder.as_str(), stored.archived), ("INBOX", false), "{m} stays in the inbox");
    }
    assert_eq!(count(&core, "SELECT count(*) FROM undo_entry"), undo_before, "no undo entry");
    assert_eq!(count(&core, "SELECT count(*) FROM pending_action"), pending_before, "nothing is queued for the mail server");
    println!("PASS block-sender-leaves-the-inbox-mail-records-no-undo-and-queues-nothing");
    // idempotent: a second block, in another spelling, changes nothing (the first time stays)
    let since = core.blocks()[0].since_ms;
    std::thread::sleep(Duration::from_millis(5));
    timed("BlockSender again", || core.run(Command::BlockSender { address: "ann@ext.TEST".into() })).expect("blocked again");
    assert_eq!(core.blocks().len(), 1);
    assert_eq!(core.blocks()[0].since_ms, since, "the first time stays");
    println!("PASS block-sender-idempotent");
    let core = reopen(core, &dir);
    assert_eq!(core.blocks().into_iter().map(|b| b.target).collect::<Vec<_>>(), vec![sender("ann@ext.test")]);
    assert_eq!(st(&core, "block/ann@ext.test/blocked").as_deref(), Some("true"));
    assert!(st(&core, "message/m1/junk").unwrap().contains("\"blocked\":true"));
    println!("PASS block-sender-survives-a-restart");
}

#[test]
fn what_is_not_a_full_address_is_refused_in_words_and_nothing_is_written() {
    let (core, _id, _dir) = big();
    for bad in ["", "   ", "ann", "@ext.test", "ann@", "ann@ext", "a@b@c.test", "ext.test"] {
        assert_eq!(invalid(core.run(Command::BlockSender { address: bad.into() })),
            ("address".to_string(), "a sender is a full address like name@example.com".to_string()), "{bad:?}");
    }
    assert_eq!(st(&core, "block/count").as_deref(), Some("0"), "a refusal writes nothing");
    assert!(core.blocks().is_empty());
    println!("PASS block-sender-refuses-eight-non-addresses-in-the-test-engines-words-and-writes-nothing");
}

// ---------------------------------------------------------------- contact_messages

#[test]
fn a_contacts_mail_is_listed_for_a_person_and_for_a_card_newest_first_and_survives_a_restart() {
    let (core, _id, dir) = big();
    // a person of the people list (`person:<address>`, the ids `contacts()` gives), any case
    let ann = timed("contact_messages person (2,000 mails)", || core.contact_messages("person:ann@ext.test".into()));
    assert_eq!(ids(ann), vec!["m2", "m1"], "ann's two mails, newest first");
    assert_eq!(ids(core.contact_messages("person:ANN@Ext.test".into())), vec!["m2", "m1"], "any case");
    assert_eq!(ids(core.contact_messages("person:bob@ext.test".into())), vec!["m3"]);
    let bulk = timed("contact_messages bulk sender (2,000 mails)", || core.contact_messages("person:sender0@bulk.test".into()));
    assert_eq!(bulk.len(), 50, "every 40th of the 1,995 bulk mails");
    assert!(bulk.windows(2).all(|w| w[0].received_at_ms >= w[1].received_at_ms), "newest first");
    for unknown in ["person:nobody@ext.test", "person:", "nonsense", ""] {
        assert!(core.contact_messages(unknown.into()).is_empty(), "{unknown:?} names nobody");
    }
    println!("PASS contact-messages-of-a-person-any-case-newest-first-unknown-is-empty");
    // a merged person: the card keeps the first id and holds both addresses; either id lists the mail of both
    core.run(Command::MergeContacts { keep_id: "person:ann@ext.test".into(), merge_id: "person:ann.work@corp.test".into() }).expect("merged");
    assert_eq!(ids(core.contact_messages("person:ann@ext.test".into())), vec!["m4", "m2", "m1"]);
    assert_eq!(ids(core.contact_messages("person:ann.work@corp.test".into())), vec!["m4", "m2", "m1"], "by the card that holds the other address");
    let listed = core.contacts().into_iter().find(|c| c.addresses.iter().any(|a| a == "ann.work@corp.test")).expect("the merged person is listed");
    assert_eq!(ids(core.contact_messages(listed.id.clone())), vec!["m4", "m2", "m1"], "by the id contacts() gives");
    // a stored card by its own id
    let now = 5_000;
    core.store().write(move |c| c.contact_save("card-bob", "Bob", &["bob@ext.test".to_string(), "promo@spam.test".to_string()], now)).unwrap();
    assert_eq!(ids(timed("contact_messages card (2,000 mails)", || core.contact_messages("card-bob".into()))), vec!["m5", "m3"]);
    println!("PASS contact-messages-of-a-merged-person-and-of-a-stored-card");
    let core = reopen(core, &dir);
    assert_eq!(ids(core.contact_messages("card-bob".into())), vec!["m5", "m3"]);
    assert_eq!(ids(core.contact_messages("person:ann.work@corp.test".into())), vec!["m4", "m2", "m1"]);
    println!("PASS contact-messages-survive-a-restart");
}
