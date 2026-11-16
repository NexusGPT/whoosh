//! fix-core (d99929d0, docs/lanes/fix-core.md): the hidden check's mutant `WHOOSH_CORE_MUTANT=body-stored-raw` (rust_core.rs, rig only) keeps a fetched
//! multipart quoted-printable mail as its raw text: the boundary line, the part headers and the soft line breaks come back, which is what the hidden
//! `--slice-body` must see go red; the same core opened without the variable stores it decoded (engine_body_qp.rs). One test, because the variable and
//! the switch it sets are the process's own and the two cores are opened one after the other.
//! Broken copies (one edit, red first):
//!   bodies.rs `kept`: `if STORE_RAW.load(Ordering::Relaxed) {` -> `if false {`   (the mutant stores nothing raw: the first half fails)
//!   rust_core.rs: `crate::bodies::set_store_raw(mutant == Mutant::BodyStoredRaw);` -> `crate::bodies::set_store_raw(true);` (the plain core stores raw: the second half fails)
use std::sync::Arc;
use std::time::{Duration, Instant};

use contracts::*;
use engine::RustCore;
use server::{Fixture, Server};
use store::NewMessage;

#[allow(dead_code, unused_imports)]
#[path = "../../sync-imap/tests/support/mod.rs"]
mod imap_fakes;
#[allow(dead_code, unused_imports)]
#[path = "../../sync-imap/tests/body_r2_server/mod.rs"]
mod server;

const USER: &str = "owner@example.test";
const WAIT: Duration = Duration::from_secs(30);

fn wait_until(what: &str, mut ok: impl FnMut() -> bool) {
    let t0 = Instant::now();
    while t0.elapsed() < WAIT {
        if ok() { return; }
        std::thread::sleep(Duration::from_millis(20));
    }
    panic!("never reached within {WAIT:?}: {what}");
}

fn config() -> AccountConfig {
    AccountConfig {
        imap_host: "127.0.0.1".into(), imap_port: 1, imap_security: ConnectionSecurity::Ssl, smtp_host: "127.0.0.1".into(), smtp_port: 1,
        smtp_security: ConnectionSecurity::Starttls, username: USER.into(), smtp_username: USER.into(), auth_method: AuthMethod::Password, source: DetectionSource::Manual,
    }
}

fn row(account: &str, uid: u32) -> NewMessage {
    NewMessage {
        message: Message {
            id: format!("{account}:INBOX:1:{uid}"), thread_id: format!("{account}:<qp{uid}>"), account_id: account.into(), sender_name: "Sender".into(),
            from_address: "sender@example.test".into(), to: vec![USER.into()], subject: "Omega".into(), preview: "preview".into(),
            received_at_ms: 1_700_000_000_000 + i64::from(uid) * 1000, read: false, archived: false, folder: "INBOX".into(), attachment_names: vec![], login_code: None,
            trackers_blocked: 0, list_unsubscribe: vec![], list_unsubscribe_one_click: false, reply_to_name: None, cc: vec![], bcc: vec![], is_outgoing: false,
        },
        remote_id: Some(uid.to_string()), body_text: None, body_html: None, has_list_header: false, size_bytes: Some(4000), tab_id: None,
    }
}

const RAW: &str = "From: sweetgreen <hello@sweetgreen.example>\r\nTo: owner@example.test\r\nSubject: Omega\r\nMessage-ID: <omega@sweetgreen.example>\r\nMIME-Version: 1.0\r\n\
Content-Type: multipart/alternative; boundary=\"fix-core-bound\"\r\n\r\n\
--fix-core-bound\r\nContent-Transfer-Encoding: quoted-printable\r\nContent-Type: text/plain; charset=us-ascii\r\n\r\n\
Toasted almo=\r\nnds over warm quinoa.\r\n\r\n\
--fix-core-bound\r\nContent-Transfer-Encoding: quoted-printable\r\nContent-Type: text/html; charset=us-ascii\r\n\r\n\
<html><body><p>Toasted almo=\r\nnds over warm quinoa.</p></body></html>\r\n\
--fix-core-bound--\r\n";

/// The text the core stores for the mail, fetched through the real adapter against the scripted server.
fn stored_text(uid: u32) -> String {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let srv = rt.block_on(Server::start(&[], 1));
    srv.put(uid, Fixture { raw: RAW.as_bytes().to_vec(), sections: vec![] });
    let dir = tempfile::tempdir().unwrap();
    let core = RustCore::open_production(dir.path().to_str().unwrap(), true).expect("the rig core opens");
    let store = core.store();
    let account = store.add_account(USER, "Owner", MailProvider::Microsoft, "svc\u{1f}acct", false, config(), 1).unwrap();
    store.insert_messages(vec![row(&account, uid)]).unwrap();
    core.set_body_source(&account, Arc::new(server::adapter(&srv, imap_fakes::limits())));
    let id = format!("{account}:INBOX:1:{uid}");
    let _ = core.body(id.clone());
    wait_until("the body is stored", || store.body_parts(&id).unwrap().is_some_and(|p| p.body_text.is_some()));
    assert!(core.settle_bodies(WAIT));
    store.body_parts(&id).unwrap().expect("stored").body_text.expect("text")
}

#[test]
fn the_body_mutant_stores_the_raw_mail_and_a_core_without_it_stores_it_decoded() {
    std::env::set_var("WHOOSH_CORE_MUTANT", "body-stored-raw");
    let raw = stored_text(31);
    assert!(raw.contains("--fix-core-bound") && raw.contains("Content-Transfer-Encoding") && raw.contains("almo=\r\nnds"), "the mutant keeps the raw message: {raw:?}");
    std::env::remove_var("WHOOSH_CORE_MUTANT");
    let decoded = stored_text(32);
    assert!(!decoded.contains("--fix-core-bound") && !decoded.contains("Content-Transfer-Encoding") && !decoded.contains("=\r\n"), "without the mutant the text is decoded: {decoded:?}");
    assert!(decoded.contains("Toasted almonds over warm quinoa."), "the soft break is joined: {decoded:?}");
}
