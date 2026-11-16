//! gap-engine-tests (item d99929d0, the engine half): a multipart mail whose parts are quoted-printable (or base64) is STORED as decoded
//! text: the stored `body_text` and `body_html` hold no boundary line, no part header and no soft line break (`=` at the end of a line),
//! and the reader's `plain_text` is the same decoded text. The mail is the owner's sweetgreen message in the shape the sync fetched it on
//! main56 (core/crates/sync-imap/src/envelope.rs:194-195: multipart/alternative, a quoted-printable text part and a quoted-printable HTML
//! part, soft breaks inside "almonds", "home" and "corn"), whole this time; a second mail is Soyo-World-shaped (a base64 text part and a
//! quoted-printable HTML part: the owner's real Soyo mail is not in the repo, so this is the shape, not its words).
//! Over the production core and the real IMAP adapter against the scripted server (127.0.0.1 only).
//!
//! RED-FIRST broken copy (one edit, the lead runs it; it turns both tests red): core/crates/engine/src/bodies.rs `kept`, the line
//! `let (text, html, trackers, preview) = crate::reads::parse_raw_with_preview(raw)?;` ->
//! `let (text, html, trackers, preview): (String, Option<String>, u32, String) = (String::from_utf8_lossy(raw).into_owned(), None, 0, String::new());`
//! (the raw message is stored as the text: the boundary line, the part headers and the soft breaks come back).
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
const BOUNDARY: &str = "--d58e6804b42fcd7ff1ee0a7a0a61d95ea7fedb6f4ba88fd5e2a798310c04";

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

/// The row the sync stored before the body came: the preview is a placeholder, the body is empty.
fn row(account: &str, uid: u32, subject: &str) -> NewMessage {
    NewMessage {
        message: Message {
            id: format!("{account}:INBOX:1:{uid}"), thread_id: format!("{account}:<qp{uid}>"), account_id: account.into(), sender_name: "Sender".into(),
            from_address: "sender@example.test".into(), to: vec![USER.into()], subject: subject.into(), preview: "preview".into(),
            received_at_ms: 1_700_000_000_000 + i64::from(uid) * 1000, read: false, archived: false, folder: "INBOX".into(), attachment_names: vec![], login_code: None,
            trackers_blocked: 0, list_unsubscribe: vec![], list_unsubscribe_one_click: false, reply_to_name: None, cc: vec![], bcc: vec![], is_outgoing: false,
        },
        remote_id: Some(uid.to_string()), body_text: None, body_html: None, has_list_header: false, size_bytes: Some(4000), tab_id: None,
    }
}

/// The sweetgreen message: the header block of envelope.rs:194, then the body its text names, whole, quoted-printable with soft breaks.
fn sweetgreen_raw() -> Vec<u8> {
    format!("From: sweetgreen <hello@sweetgreen.example>\r\nTo: owner@example.test\r\nSubject: Omega Glow Bowl\r\nMessage-ID: <omega@sweetgreen.example>\r\nMIME-Version: 1.0\r\n\
Content-Type: multipart/alternative; boundary=\"{b}\"\r\n\r\n\
--{b}\r\nContent-Transfer-Encoding: quoted-printable\r\nContent-Type: text/plain; charset=us-ascii\r\n\r\n\
The Omega Glow Bowl: miso glazed salmon, sweet corn salsa, and toasted almo=\r\nnds over warm quinoa. Order now and pick it up on your way h=\r\nome.\r\n\r\n\
--{b}\r\nContent-Transfer-Encoding: quoted-printable\r\nContent-Type: text/html; charset=us-ascii\r\n\r\n\
<html><body><p>The Omega Glow Bowl: miso glazed salmon, sweet co=\r\nrn salsa, and toasted almo=\r\nnds over warm quinoa.</p></body></html>\r\n\
--{b}--\r\n", b = BOUNDARY).into_bytes()
}

/// A Soyo-World-shaped message: the text part is base64, the HTML part quoted-printable with an escaped `=3D` and a soft break.
fn soyo_raw() -> Vec<u8> {
    // "Soyo World weekly picks: lanterns, kites and a quiet corner of the market." in base64, wrapped at 40 characters
    let text_b64 = "U295byBXb3JsZCB3ZWVrbHkgcGlja3M6IGxhbnRl\r\ncm5zLCBraXRlcyBhbmQgYSBxdWlldCBjb3JuZXIg\r\nb2YgdGhlIG1hcmtldC4=";
    format!("From: Soyo World <news@soyo.example>\r\nTo: owner@example.test\r\nSubject: Weekly picks\r\nMessage-ID: <picks@soyo.example>\r\nMIME-Version: 1.0\r\n\
Content-Type: multipart/alternative; boundary=\"soyo-bound\"\r\n\r\n\
--soyo-bound\r\nContent-Type: text/plain; charset=utf-8\r\nContent-Transfer-Encoding: base64\r\n\r\n{text_b64}\r\n\
--soyo-bound\r\nContent-Type: text/html; charset=utf-8\r\nContent-Transfer-Encoding: quoted-printable\r\n\r\n\
<html><body><p class=3D\"picks\">Soyo World weekly picks: lanterns, kites and a quiet cor=\r\nner of the market.</p></body></html>\r\n\
--soyo-bound--\r\n").into_bytes()
}

fn assert_decoded(what: &str, text: &str, boundary: &str) {
    assert!(!text.contains(boundary), "{what}: no boundary line: {text:?}");
    assert!(!text.contains("--"), "{what}: no boundary dashes: {text:?}");
    assert!(!text.contains("Content-Type") && !text.contains("Content-Transfer-Encoding") && !text.contains("Content-"), "{what}: no part header: {text:?}");
    assert!(!text.contains("=\r\n") && !text.contains("=\n"), "{what}: no soft line break: {text:?}");
    assert!(!text.contains("=3D") && !text.contains("=C2") && !text.contains("=E2"), "{what}: no quoted-printable escape left: {text:?}");
}

/// The mail is fetched by the background download as the reader opens it, and waited for in the store.
fn stored_body(uid: u32, subject: &str, raw: Vec<u8>) -> (store::BodyParts, MessageBody) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let srv = rt.block_on(Server::start(&[], 1));
    srv.put(uid, Fixture { raw, sections: vec![] });
    let dir = tempfile::tempdir().unwrap();
    let core = RustCore::open_production(dir.path().to_str().unwrap(), true).expect("the rig core opens");
    let store = core.store();
    let account = store.add_account(USER, "Owner", MailProvider::Microsoft, "svc\u{1f}acct", false, config(), 1).unwrap();
    store.insert_messages(vec![row(&account, uid, subject)]).unwrap();
    core.set_body_source(&account, Arc::new(server::adapter(&srv, imap_fakes::limits())));
    let id = format!("{account}:INBOX:1:{uid}");
    let _ = core.body(id.clone());
    wait_until("the body is stored", || store.body_parts(&id).unwrap().is_some_and(|p| p.body_text.is_some() || p.body_html.is_some()));
    assert!(core.settle_bodies(WAIT));
    let parts = store.body_parts(&id).unwrap().expect("stored");
    let opened = core.body(id).expect("the reader's body");
    (parts, opened)
}

#[test]
fn a_multipart_quoted_printable_mail_is_stored_as_decoded_text_with_no_boundary_no_header_and_no_soft_break() {
    let (parts, opened) = stored_body(21, "Omega Glow Bowl", sweetgreen_raw());
    let text = parts.body_text.as_deref().expect("the text part is stored");
    assert_decoded("stored text", text, BOUNDARY);
    assert!(text.contains("toasted almonds over warm quinoa"), "the soft break inside 'almonds' is joined: {text:?}");
    assert!(text.contains("on your way home."), "the soft break inside 'home' is joined: {text:?}");
    let html = parts.body_html.as_deref().expect("the HTML part is stored");
    assert_decoded("stored html", html, BOUNDARY);
    assert!(html.contains("sweet corn salsa") && html.contains("toasted almonds"), "{html:?}");
    assert_decoded("the reader's plain text", &opened.plain_text, BOUNDARY);
    assert!(opened.plain_text.contains("toasted almonds over warm quinoa"), "{:?}", opened.plain_text);
    if let Some(safe) = opened.safe_html.as_deref() { assert_decoded("the reader's html", safe, BOUNDARY); }
    println!("PASS sweetgreen body: stored text {} chars, html {} chars, no boundary, no header, no soft break", text.chars().count(), html.chars().count());
}

#[test]
fn a_multipart_mail_with_a_base64_text_part_and_a_quoted_printable_html_part_is_stored_decoded() {
    let (parts, opened) = stored_body(22, "Weekly picks", soyo_raw());
    let text = parts.body_text.as_deref().expect("the text part is stored");
    assert_decoded("stored text", text, "--soyo-bound");
    assert!(text.contains("Soyo World weekly picks: lanterns, kites and a quiet corner of the market."), "base64 decoded: {text:?}");
    let html = parts.body_html.as_deref().expect("the HTML part is stored");
    assert_decoded("stored html", html, "--soyo-bound");
    assert!(html.contains("a quiet corner of the market."), "the soft break inside 'corner' is joined: {html:?}");
    assert_decoded("the reader's plain text", &opened.plain_text, "--soyo-bound");
    println!("PASS soyo-shaped body: stored text {} chars, html {} chars, decoded", text.chars().count(), html.chars().count());
}
