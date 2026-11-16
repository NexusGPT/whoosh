//! vip-mark: the real core runs `MarkVip { message_id, vip }` (docs/lanes/vip-mark.md). A seeded store holds unread inbox mail from two
//! senders; marking a message of sender A puts every unread inbox mail of A, and only those, in the VIPs view and its count; unmarking
//! takes both back to nothing; an id the store does not hold is refused and writes nothing. Nothing leaves this Mac.
//! Seen red on the broken copy named under the first test, then green.
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
const SENDER_A: &str = "alice@partner.test";
const SENDER_B: &str = "bob@other.test";

fn detector() -> Box<dyn engine::Detect> { Box::new(autoconfig::Detector::new(FakeHttp::default(), FakeDns::default(), FakeProbe::default())) }
fn vips_query() -> InboxQuery {
    InboxQuery { tab_id: "primary".into(), account_id: None, label_id: None, mailbox: Some(Mailbox::Vips), account_ids: vec![], filters: vec![], smart_folder_id: None }
}
fn config() -> AccountConfig {
    AccountConfig {
        imap_host: "127.0.0.1".into(), imap_port: 1, imap_security: ConnectionSecurity::Ssl, smtp_host: "127.0.0.1".into(), smtp_port: 1,
        smtp_security: ConnectionSecurity::Starttls, username: USER.into(), smtp_username: USER.into(), auth_method: AuthMethod::Password, source: DetectionSource::Manual,
    }
}
fn msg(account: &str, id: &str, from: &str, at: i64) -> NewMessage {
    NewMessage {
        message: Message {
            id: id.into(), thread_id: format!("t-{id}"), account_id: account.into(), sender_name: from.into(), from_address: from.into(), to: vec![USER.into()],
            subject: format!("Subject {id}"), preview: format!("Preview {id}"), received_at_ms: at, read: false, archived: false, folder: "INBOX".into(),
            attachment_names: vec![], login_code: None, trackers_blocked: 0, list_unsubscribe: vec![], list_unsubscribe_one_click: false,
            reply_to_name: None, cc: vec![], bcc: vec![], is_outgoing: false,
        },
        remote_id: None, body_text: None, body_html: None, has_list_header: false, size_bytes: Some(100), tab_id: None,
    }
}

/// The store with unread inbox mail: a1, a2 from sender A (a2's address in mixed case, as a server may send it), b1, b2 from sender B.
fn seeded() -> (RustCore, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(&dir.path().join("whoosh.sqlite")).unwrap();
    let acc = store.add_account(USER, "Owner", MailProvider::Imap, "svc\u{1f}acct", false, config(), 1).unwrap();
    store
        .insert_messages(vec![
            msg(&acc, "a1", SENDER_A, 1_000), msg(&acc, "a2", "Alice@Partner.test", 2_000),
            msg(&acc, "b1", SENDER_B, 3_000), msg(&acc, "b2", SENDER_B, 4_000),
        ])
        .unwrap();
    (RustCore::with_parts(store, Arc::new(MemoryKeychain::new(Namespace::rig())), detector(), TestTransport::PlainLoopback), dir)
}
fn vips_count(core: &RustCore) -> Option<u32> { core.counts().mailboxes.get("vips").copied() }
fn vips_view(core: &RustCore) -> Vec<String> {
    let mut v: Vec<String> = core.inbox(vips_query()).into_iter().map(|r| r.message_id).collect();
    v.sort();
    v
}
fn vip_rows(core: &RustCore) -> i64 { core.store().write(|c| Ok(c.tx.query_row("SELECT count(*) FROM vip", [], |r| r.get::<_, i64>(0))?)).unwrap() }

#[test]
fn marking_a_sender_fills_the_vips_view_and_count_and_unmarking_empties_them() {
    let (core, _d) = seeded();
    assert_eq!(vips_count(&core), Some(0), "before: no VIP, {:?}", core.counts().mailboxes);
    assert!(vips_view(&core).is_empty(), "before: the VIPs view is empty");

    let out = core.run(Command::MarkVip { message_id: "a1".into(), vip: true }).expect("marked");
    assert_eq!(out.undo_entry_id, None, "no undo entry");
    assert_eq!(vips_count(&core), Some(2), "the VIPs figure is sender A's unread inbox mail (a1, a2): {:?}", core.counts().mailboxes);
    assert_eq!(vips_view(&core), vec!["a1".to_string(), "a2".to_string()], "the VIPs view lists exactly sender A's mail");

    core.run(Command::MarkVip { message_id: "a2".into(), vip: false }).expect("unmarked");
    assert_eq!(vips_count(&core), Some(0), "unmarked from A's other mail: the figure is back to nothing, {:?}", core.counts().mailboxes);
    assert!(vips_view(&core).is_empty(), "and the VIPs view is empty again: {:?}", vips_view(&core));
    assert_eq!(vip_rows(&core), 0, "the VIP list holds nothing");
}
// broken copy "marks-nothing": core/crates/engine/src/rust_core.rs, the `Command::MarkVip` arm becomes
// `Command::MarkVip { .. } => Ok(CommandOutcome { undo_entry_id: None, created_id: None })` (red here on the figure after the mark, and below
// on the refusal). Run and saved under _captures/vip-mark/.

#[test]
fn mark_vip_refuses_an_unknown_message_and_writes_nothing() {
    let (core, _d) = seeded();
    let Err(CoreError::NotFound { kind, id }) = core.run(Command::MarkVip { message_id: "ghost".into(), vip: true }) else { panic!("an unknown message was not refused as NotFound") };
    assert_eq!((kind.as_str(), id.as_str()), ("message", "ghost"));
    assert_eq!(vip_rows(&core), 0, "nothing was written");
    assert_eq!(vips_count(&core), Some(0));
}
