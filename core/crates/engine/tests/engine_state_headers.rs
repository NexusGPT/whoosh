//! fix-core (docs/lanes/gap-moved-slice.md, "the unit tests for state_headers.rs are not written"): the state read `message/<id>/thread_headers`
//! (engine/src/state_headers.rs) that the hidden real-engine check `--slice-moved` reads. It answers `{"message_id":"<..>","references":[..]}` from the
//! store for a message id, or for a thread id (then the newest message of the thread that has stored headers), and answers nothing otherwise.
//! Each test names its broken copy (one edit, red first):
//!   the_stored_headers_of_a_message_are_served          state_headers.rs `.map(|r| json_text(r))` list joined with `refs.join(",")` -> `String::new()` (no references)
//!   a_thread_id_serves_the_newest_message_with_headers  state_headers.rs `store.newest_thread_headers(id)` arm removed (`None => return None`)
//!   nothing_is_served_while_no_headers_are_stored       state_headers.rs the `None => store.newest_thread_headers(id)...?` arm replaced by `None => store::ThreadHeaders { message_id: String::new(), references: vec![] }` (an empty object is answered)
//!   a_path_of_another_shape_is_not_claimed              state_headers.rs `["message", id, "thread_headers"]` -> `["message", id, _]`
use std::sync::Arc;

use contracts::*;
use engine::{RustCore, TestTransport};
use secrets::{MemoryKeychain, Namespace};
use store::{NewMessage, Store, ThreadHeaders};

#[allow(dead_code, unused_imports)]
#[path = "../../autoconfig/tests/support/mod.rs"]
mod detect_fakes;
use detect_fakes::{FakeDns, FakeHttp, FakeProbe};

const USER: &str = "owner@example.test";

fn detector() -> Box<dyn engine::Detect> { Box::new(autoconfig::Detector::new(FakeHttp::default(), FakeDns::default(), FakeProbe::default())) }
fn config() -> AccountConfig {
    AccountConfig {
        imap_host: "127.0.0.1".into(), imap_port: 1, imap_security: ConnectionSecurity::Ssl, smtp_host: "127.0.0.1".into(), smtp_port: 1,
        smtp_security: ConnectionSecurity::Starttls, username: USER.into(), smtp_username: USER.into(), auth_method: AuthMethod::Password, source: DetectionSource::Manual,
    }
}
fn msg(account: &str, id: &str, thread: &str, at: i64) -> NewMessage {
    NewMessage {
        message: Message {
            id: id.into(), thread_id: thread.into(), account_id: account.into(), sender_name: "Them".into(), from_address: "them@ext.test".into(), to: vec![USER.into()],
            subject: format!("Subject {id}"), preview: format!("Preview {id}"), received_at_ms: at, read: false, archived: false, folder: "INBOX".into(),
            attachment_names: vec![], login_code: None, trackers_blocked: 0, list_unsubscribe: vec![], list_unsubscribe_one_click: false,
            reply_to_name: None, cc: vec![], bcc: vec![], is_outgoing: false,
        },
        remote_id: None, body_text: None, body_html: None, has_list_header: false, size_bytes: Some(100), tab_id: None,
    }
}
fn seeded() -> (RustCore, String, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(&dir.path().join("whoosh.sqlite")).unwrap();
    let account = store.add_account(USER, "Owner", MailProvider::Imap, "svc\u{1f}acct", false, config(), 1).unwrap();
    let kc = Arc::new(MemoryKeychain::new(Namespace::rig()));
    (RustCore::with_parts(store, kc, detector(), TestTransport::PlainLoopback), account, dir)
}
fn headers(id: &str, refs: &[&str]) -> (String, ThreadHeaders) {
    (id.to_string(), ThreadHeaders { message_id: format!("<{id}@ext.test>"), references: refs.iter().map(|r| r.to_string()).collect() })
}
fn store_headers(core: &RustCore, list: Vec<(String, ThreadHeaders)>) {
    core.store().write(move |ctx| { ctx.set_thread_headers(&list)?; Ok(()) }).unwrap();
}
fn read(core: &RustCore, path: &str) -> Option<serde_json::Value> {
    core.state(path.to_string()).map(|s| serde_json::from_str(&s).unwrap_or_else(|e| panic!("{path}: not JSON ({e}): {s}")))
}

#[test]
fn the_stored_headers_of_a_message_are_served() {
    let (core, account, _d) = seeded();
    core.store().insert_messages(vec![msg(&account, "m1", "t1", 1_000)]).unwrap();
    store_headers(&core, vec![headers("m1", &["<root@ext.test>", "<mid@ext.test>"])]);
    let got = read(&core, "message/m1/thread_headers").expect("served");
    assert_eq!(got["message_id"], "<m1@ext.test>");
    assert_eq!(got["references"], serde_json::json!(["<root@ext.test>", "<mid@ext.test>"]), "the chain, oldest first, as stored");
}

#[test]
fn a_thread_id_serves_the_newest_message_with_headers() {
    let (core, account, _d) = seeded();
    core.store().insert_messages(vec![msg(&account, "old", "t2", 1_000), msg(&account, "new", "t2", 2_000), msg(&account, "newest-unknown", "t2", 3_000)]).unwrap();
    store_headers(&core, vec![headers("old", &[]), headers("new", &["<old@ext.test>"])]);
    let got = read(&core, "message/t2/thread_headers").expect("served by thread id");
    assert_eq!(got["message_id"], "<new@ext.test>", "the newest message that HAS stored headers (the newest of all has none)");
    assert_eq!(got["references"], serde_json::json!(["<old@ext.test>"]));
}

#[test]
fn nothing_is_served_while_no_headers_are_stored() {
    let (core, account, _d) = seeded();
    core.store().insert_messages(vec![msg(&account, "m3", "t3", 1_000)]).unwrap();
    assert!(core.state("message/m3/thread_headers".into()).is_none(), "a message with no stored headers: no answer");
    assert!(core.state("message/t3/thread_headers".into()).is_none(), "a thread with none: no answer");
    assert!(core.state("message/nobody/thread_headers".into()).is_none(), "an unknown id: no answer");
}

#[test]
fn a_path_of_another_shape_is_not_claimed() {
    let (core, account, _d) = seeded();
    core.store().insert_messages(vec![msg(&account, "m4", "t4", 1_000)]).unwrap();
    store_headers(&core, vec![headers("m4", &[])]);
    assert!(core.state("message/m4/thread_headers/extra".into()).is_none());
}
