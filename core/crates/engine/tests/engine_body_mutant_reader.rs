//! fix-realcore (main92 s2-001.log, `--slice-body --mutant`: "mutant body-stored-raw: SURVIVED"): the mutant kept the raw mail as the text but also
//! kept the decoded prepared result (`display_html`, segments), so the reader drew the decoded html and nothing went red. Under the mutant the
//! body the reader receives must be the raw text alone: no prepared row, no html, no display html, the boundary and part headers in the text.
//! Broken copies (one edit, red first):
//!   bodies.rs `kept`, the STORE_RAW branch: `prepared: None });` -> `prepared: Some(crate::reads::prepared_json(&prepared)) });` (the old code: red)
//!   bodies.rs `kept`: `if STORE_RAW.load(Ordering::Relaxed) {` -> `if false {`   (the mutant stores nothing raw: red)
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
            id: format!("{account}:INBOX:1:{uid}"), thread_id: format!("{account}:<mr{uid}>"), account_id: account.into(), sender_name: "Sender".into(),
            from_address: "sender@example.test".into(), to: vec![USER.into()], subject: "Omega".into(), preview: "preview".into(),
            received_at_ms: 1_700_000_000_000 + i64::from(uid) * 1000, read: false, archived: false, folder: "INBOX".into(), attachment_names: vec![], login_code: None,
            trackers_blocked: 0, list_unsubscribe: vec![], list_unsubscribe_one_click: false, reply_to_name: None, cc: vec![], bcc: vec![], is_outgoing: false,
        },
        remote_id: Some(uid.to_string()), body_text: None, body_html: None, has_list_header: false, size_bytes: Some(4000), tab_id: None,
    }
}

const RAW: &str = "From: sweetgreen <hello@sweetgreen.example>\r\nTo: owner@example.test\r\nSubject: Omega\r\nMessage-ID: <omega-r@sweetgreen.example>\r\nMIME-Version: 1.0\r\n\
Content-Type: multipart/alternative; boundary=\"fix-realcore-bound\"\r\n\r\n\
--fix-realcore-bound\r\nContent-Transfer-Encoding: quoted-printable\r\nContent-Type: text/plain; charset=us-ascii\r\n\r\n\
Toasted almo=\r\nnds over warm quinoa.\r\n\r\n\
--fix-realcore-bound\r\nContent-Transfer-Encoding: quoted-printable\r\nContent-Type: text/html; charset=us-ascii\r\n\r\n\
<html><body><p>Toasted almo=\r\nnds over warm quinoa.</p></body></html>\r\n\
--fix-realcore-bound--\r\n";

/// The parts the core stores for the mail, fetched through the real adapter against the scripted server.
fn stored(uid: u32) -> store::BodyParts {
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
    store.body_parts(&id).unwrap().expect("stored")
}

#[test]
fn under_the_body_mutant_the_reader_receives_only_the_raw_text_and_no_decoded_html() {
    std::env::set_var("WHOOSH_CORE_MUTANT", "body-stored-raw");
    let p = stored(41);
    std::env::remove_var("WHOOSH_CORE_MUTANT");
    let text = p.body_text.clone().expect("text");
    assert!(text.contains("--fix-realcore-bound") && text.contains("almo=\r\nnds"), "the mutant keeps the raw message: {text:?}");
    assert!(p.body_html.is_none(), "the mutant keeps no html: {:?}", p.body_html);
    assert!(p.prepared.is_none(), "the mutant keeps no prepared result (its display_html is the decoded mail): {:?}", p.prepared);
    let opened = engine::reads::body_from_stored(&p.message_id, text, p.body_html.clone(), p.trackers_blocked, p.prepared.as_deref());
    assert!(opened.display_html.is_none() && opened.written_html.is_none() && opened.safe_html.is_none(), "the reader gets no decoded document");
    assert!(opened.segments.is_empty(), "the reader gets no decoded segments: {:?}", opened.segments.len());
    assert!(opened.plain_text.contains("--fix-realcore-bound") && opened.plain_text.contains("Content-Transfer-Encoding"), "the reader shows the raw text");
}
