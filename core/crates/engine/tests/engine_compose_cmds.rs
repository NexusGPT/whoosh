//! real-compose (docs/lanes/real-compose.md): `ReplyAll` and `SetSignature` in the real core (they answered "The local engine does not do
//! this yet."), and the reads the app makes for them. The mail server is the sync-imap fake on 127.0.0.1; the SMTP server is a TcpListener of
//! this file on 127.0.0.1 that accepts messages and keeps them. Nothing leaves this Mac. Each test names its broken copy (one line of
//! compose_cmds.rs, outbox_tick.rs or delta15.rs, listed in docs/lanes/real-compose.md and in checks/proofs/g3/cded3f29.json).
#[allow(dead_code, unused_imports)]
#[path = "../../autoconfig/tests/support/mod.rs"]
mod detect_fakes;
#[allow(dead_code, unused_imports)]
#[path = "../../sync-imap/tests/support/mod.rs"]
mod imap_fakes;

use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use contracts::*;
use detect_fakes::{FakeDns, FakeHttp, FakeProbe};
use engine::{RustCore, SyncPhase, TestTransport};
use imap_fakes::FakeImap;
use secrets::{MemoryKeychain, Namespace};
use store::{NewMessage, Store, ThreadHeaders};

const USER: &str = "owner@example.test";
const SECOND: &str = "second@example.test";
const PW: &str = "app-password";
const WAIT: Duration = Duration::from_secs(25);
const SIG: &str = "Sig-Line-ZQX";

fn detector() -> Box<dyn engine::Detect> { Box::new(autoconfig::Detector::new(FakeHttp::default(), FakeDns::default(), FakeProbe::default())) }
fn kc() -> Arc<MemoryKeychain> { Arc::new(MemoryKeychain::new(Namespace::rig())) }
fn wait_until(what: &str, mut ok: impl FnMut() -> bool) {
    let t0 = Instant::now();
    while t0.elapsed() < WAIT { if ok() { return; } std::thread::sleep(Duration::from_millis(40)); }
    panic!("never reached within {WAIT:?}: {what}");
}
fn config(imap_port: u16, smtp_port: u16) -> AccountConfig {
    AccountConfig {
        imap_host: "127.0.0.1".into(), imap_port, imap_security: ConnectionSecurity::Ssl, smtp_host: "127.0.0.1".into(), smtp_port,
        smtp_security: ConnectionSecurity::Starttls, username: USER.into(), smtp_username: USER.into(), auth_method: AuthMethod::Password, source: DetectionSource::Manual,
    }
}

// ---------------------------------------------------------------- an SMTP server that accepts mail (loopback only)

struct Smtp { port: u16, got: Arc<Mutex<Vec<String>>> }
fn smtp() -> Smtp {
    let l = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = l.local_addr().unwrap().port();
    let got = Arc::new(Mutex::new(Vec::new()));
    let g = got.clone();
    std::thread::spawn(move || {
        for s in l.incoming() {
            let Ok(s) = s else { break };
            let g = g.clone();
            std::thread::spawn(move || {
                let mut w = s.try_clone().unwrap();
                let mut r = BufReader::new(s);
                let _ = w.write_all(b"220 accept.test ESMTP\r\n");
                let mut line = String::new();
                loop {
                    line.clear();
                    if r.read_line(&mut line).unwrap_or(0) == 0 { return; }
                    let up = line.trim_end().to_ascii_uppercase();
                    let reply: &[u8] = if up.starts_with("EHLO") || up.starts_with("HELO") { b"250-accept.test\r\n250-AUTH PLAIN LOGIN\r\n250 8BITMIME\r\n" }
                        else if up.starts_with("AUTH") { b"235 2.7.0 ok\r\n" }
                        else if up.starts_with("DATA") {
                            let _ = w.write_all(b"354 go\r\n");
                            let mut body = String::new();
                            loop { let mut l2 = String::new(); if r.read_line(&mut l2).unwrap_or(0) == 0 { return; } if l2 == ".\r\n" { break; } body.push_str(&l2); }
                            g.lock().unwrap().push(body);
                            b"250 2.0.0 queued\r\n"
                        }
                        else if up.starts_with("QUIT") { let _ = w.write_all(b"221 bye\r\n"); return; }
                        else { b"250 ok\r\n" };
                    if w.write_all(reply).is_err() { return; }
                }
            });
        }
    });
    Smtp { port, got }
}

/// The IMAP fake and the account added through the core (the sync is listening when it returns), plus a second account of the owner.
fn added() -> (RustCore, String, FakeImap, Smtp, tokio::runtime::Runtime, tempfile::TempDir) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let imap = rt.block_on(FakeImap::start());
    imap.seed("INBOX", 1);
    let smtp = smtp();
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(&dir.path().join("whoosh.sqlite")).unwrap();
    let core = RustCore::with_parts(store, kc(), detector(), TestTransport::PlainLoopback);
    let out = core.run(Command::AddAccountConfigured { address: USER.into(), display_name: Some("Owner".into()), config: config(imap.port, smtp.port), password: SecretText { value: PW.into() } }).expect("added");
    let id = out.created_id.unwrap();
    wait_until("listening", || core.sync_phase(&id) == Some(SyncPhase::Listening));
    core.store().add_account(SECOND, "Second", MailProvider::Imap, "svc\u{1f}second", false, config(1, 1), 2).unwrap();
    (core, id, imap, smtp, rt, dir)
}

/// A store with two accounts of the owner and no network: for the tests that only read and write the store.
fn local() -> (RustCore, String, String, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(&dir.path().join("whoosh.sqlite")).unwrap();
    let a = store.add_account(USER, "Owner", MailProvider::Imap, "svc\u{1f}acct", false, config(1, 1), 1).unwrap();
    let b = store.add_account(SECOND, "Second", MailProvider::Imap, "svc\u{1f}b", false, config(1, 1), 2).unwrap();
    (RustCore::with_parts(store, kc(), detector(), TestTransport::PlainLoopback), a, b, dir)
}

fn mail(account: &str, id: &str, thread: &str, from: &str, to: &[&str], cc: &[&str], at: i64) -> NewMessage {
    let list = |v: &[&str]| v.iter().map(|s| s.to_string()).collect::<Vec<_>>();
    NewMessage {
        message: Message {
            id: id.into(), thread_id: thread.into(), account_id: account.into(), sender_name: "Ann Sender".into(), from_address: from.into(), to: list(to),
            subject: format!("Subject {id}"), preview: format!("Preview {id}"), received_at_ms: at, read: false, archived: false, folder: "INBOX".into(),
            attachment_names: vec![], login_code: None, trackers_blocked: 0, list_unsubscribe: vec![], list_unsubscribe_one_click: false,
            reply_to_name: None, cc: list(cc), bcc: vec![], is_outgoing: false,
        },
        // reds-reply: no server UID. The store keeps one row per (account, folder, remote_id) and ignores a second insert, so the same "1" on
        // every mail kept only the first mail of each account (ReplyAll then answered NotFound); None also keeps the sync's reconcile off these rows.
        remote_id: None, body_text: Some(format!("Body of {id}\nsecond line")), body_html: None, has_list_header: false, size_bytes: Some(100), tab_id: None,
    }
}
fn put(core: &RustCore, m: NewMessage) { core.store().insert_messages(vec![m]).unwrap(); }
fn reply_all(core: &RustCore, id: &str) -> String { core.run(Command::ReplyAll { message_id: id.into() }).expect("reply all").created_id.expect("the draft id") }
fn list(core: &RustCore, draft: &str, leaf: &str) -> Vec<String> {
    let json = core.state(format!("draft/{draft}/{leaf}")).unwrap_or_else(|| panic!("no state draft/{draft}/{leaf}"));
    serde_json::from_str(&json).unwrap_or_else(|e| panic!("{json}: {e}"))
}
fn text(core: &RustCore, draft: &str, leaf: &str) -> Option<String> { core.state(format!("draft/{draft}/{leaf}")).map(|j| serde_json::from_str(&j).unwrap_or_else(|e| panic!("{j}: {e}"))) }

// ---------------------------------------------------------------- the recipients

/// Broken copy R1: compose_cmds.rs `let own = store.own_addresses()...?;` becomes `let own: Vec<String> = vec![];` (the owner is answered too).
/// R2: `m.to.clone()` becomes `vec![]` (the other To addresses are lost). R3: `m.cc.clone()` becomes `vec![]` (the Cc is lost).
/// R4: `reply_all_recipients(m.from_address.clone(),` becomes `reply_all_recipients(String::new(),` (the sender is not answered).
#[test]
fn reply_all_goes_to_the_sender_and_copies_everyone_else_but_the_owners_own_addresses() {
    let (core, a, _b, _d) = local();
    put(&core, mail(&a, "m1", "t1", "ann@x.example", &[USER, "bob@x.example", SECOND], &["cy@x.example", "OWNER@example.test"], 1_000));
    let d = reply_all(&core, "m1");
    assert_eq!(list(&core, &d, "to"), vec!["ann@x.example", "bob@x.example"], "the sender first, then the other To; neither of the owner's accounts");
    assert_eq!(list(&core, &d, "cc"), vec!["cy@x.example"], "the Cc, without the owner's address in capitals");
}

/// The one-address-once rule is the contract's (core/crates/contracts/src/reply.rs, tested in contracts/tests/delta9.rs): here its cases go through the
/// real command on stored mail. Broken copies R2 and R3 turn it red (the To or the Cc is lost).
#[test]
fn no_address_is_listed_twice_the_case_is_ignored_and_an_address_in_to_is_not_copied() {
    let (core, a, _b, _d) = local();
    put(&core, mail(&a, "m2", "t2", "Ann@X.example", &["bob@x.example", "BOB@x.example", "ann@x.example"], &["bob@X.EXAMPLE", "cy@x.example", "Cy@x.example", "ann@x.example"], 1_000));
    let d = reply_all(&core, "m2");
    assert_eq!(list(&core, &d, "to"), vec!["Ann@X.example", "bob@x.example"], "each once, the first spelling kept");
    assert_eq!(list(&core, &d, "cc"), vec!["cy@x.example"], "no address twice, none already in To");
}

/// Broken copy R1 (the own address is not dropped: the owner would be in To and the original To would not be the whole list).
#[test]
fn a_mail_the_owner_wrote_is_answered_to_its_original_recipients() {
    let (core, a, _b, _d) = local();
    put(&core, mail(&a, "m3", "t3", USER, &["bob@x.example", "cy@x.example"], &["dee@x.example", SECOND], 1_000));
    let d = reply_all(&core, "m3");
    assert_eq!(list(&core, &d, "to"), vec!["bob@x.example", "cy@x.example"], "the owner wrote it: the reply goes to who it went to");
    assert_eq!(list(&core, &d, "cc"), vec!["dee@x.example"]);
}

// ---------------------------------------------------------------- the draft

/// Broken copies: R6 `"Re".into()` becomes `"Fwd".into()`; R7 `m.account_id.clone(), m.thread_id.clone()` becomes `m.account_id.clone(), String::new()`;
/// R8 `when_words(m.received_at_ms, offset_s), text);` becomes `when_words(m.received_at_ms, offset_s), String::new());`;
/// R9 `(draft_id.clone(), m.account_id.clone(),` becomes `(draft_id.clone(), String::new(),` (a draft from no account).
#[test]
fn the_draft_is_in_the_thread_from_the_account_that_holds_the_mail_with_a_re_subject_and_the_original_quoted() {
    let (core, a, b, _d) = local();
    put(&core, mail(&b, "m4", "t4", "ann@x.example", &[SECOND], &[], 1_000));
    put(&core, mail(&a, "m5", "t5", "ann@x.example", &[USER], &[], 2_000));
    let d = reply_all(&core, "m4");
    assert_eq!(text(&core, &d, "from_account_id").as_deref(), Some(b.as_str()), "From is the account that received it");
    assert_eq!(text(&core, &d, "reply_to_thread_id").as_deref(), Some("t4"), "in the message's thread (threads the headers when it is sent)");
    assert_eq!(text(&core, &d, "subject").as_deref(), Some("Re: Subject m4"));
    let body = text(&core, &d, "body").expect("the body state path");
    assert!(body.starts_with("\n\nOn ") && body.contains("Ann Sender wrote:") && body.contains("> Body of m4") && body.contains("> second line"), "{body:?}");
    // a subject that already says Re: is not doubled
    core.store().insert_messages(vec![{ let mut m = mail(&a, "m6", "t6", "ann@x.example", &[USER], &[], 3_000); m.message.subject = "re: Already".into(); m }]).unwrap();
    let d2 = reply_all(&core, "m6");
    assert_eq!(text(&core, &d2, "subject").as_deref(), Some("re: Already"));
    assert_eq!(core.state("draft/count".into()).as_deref(), Some("2"));
    assert!(core.state(format!("draft/{d}/nothing")).is_none() && core.state("draft/no-such/to".into()).is_none(), "an unknown draft or leaf answers nothing");
}

// ---------------------------------------------------------------- refusals

/// Broken copies: R10 `kind: "message".into(), id: message_id.to_string() })?;` becomes `kind: "thread".into(), id: message_id.to_string() })?;`;
/// S1 `if !found {` becomes `if false {` (an unknown account is silently accepted).
#[test]
fn an_unknown_message_or_account_is_refused_in_words_and_nothing_is_written() {
    let (core, a, _b, _d) = local();
    put(&core, mail(&a, "m7", "t7", "ann@x.example", &[USER], &[], 1_000));
    match core.run(Command::ReplyAll { message_id: "no-such".into() }) { Err(CoreError::NotFound { kind, id }) => assert_eq!((kind.as_str(), id.as_str()), ("message", "no-such")), other => panic!("{other:?}") }
    assert_eq!(core.state("draft/count".into()).as_deref(), Some("0"), "no draft was made");
    match core.run(Command::SetSignature { account_id: "no-such".into(), text: SIG.into() }) { Err(CoreError::NotFound { kind, id }) => assert_eq!((kind.as_str(), id.as_str()), ("account", "no-such")), other => panic!("{other:?}") }
    assert!(core.signature(USER.into()).is_none() && core.accounts().iter().all(|x| x.signature.is_empty()), "no signature was written");
}

// ---------------------------------------------------------------- the signature reads

/// Broken copies: S2 delta15.rs `self.set_signature(&address, text.trim(), now_ms)?;` becomes `self.set_signature(&address, "", now_ms)?;` (nothing is kept);
/// S3 rust_core.rs `fn signature(&self, address: String) -> Option<Signature> { crate::compose_cmds::signature(&self.store, &address) }` returns None;
/// S4 rust_core.rs `a.signature = signatures.get(&a.id).cloned().unwrap_or_default();` becomes `a.signature = String::new();`;
/// S5 delta15.rs `self.set_signature(&address, text.trim(), now_ms)?;` becomes `self.set_signature(&address, text, now_ms)?;` (the blank-edge rule).
#[test]
fn a_saved_signature_is_read_back_by_address_in_any_case_and_by_the_accounts_list_and_clears_when_empty() {
    let (core, a, b, _d) = local();
    assert!(core.signature(USER.into()).is_none() && core.accounts().iter().all(|x| x.signature.is_empty()), "none before");
    core.run(Command::SetSignature { account_id: a.clone(), text: format!("  Best regards,\n{SIG}  \n") }).expect("saved");
    let want = format!("Best regards,\n{SIG}");
    assert_eq!(core.signature(USER.into()).map(|s| s.text), Some(want.clone()), "trimmed at both ends");
    assert_eq!(core.signature("OWNER@Example.Test".into()).map(|s| s.text), Some(want.clone()), "any case of the address");
    assert!(core.signature(SECOND.into()).is_none(), "another account is untouched");
    let rows = core.accounts();
    assert_eq!(rows.iter().find(|x| x.id == a).map(|x| x.signature.clone()), Some(want.clone()), "the accounts list carries it");
    assert_eq!(rows.iter().find(|x| x.id == b).map(|x| x.signature.clone()), Some(String::new()));
    core.run(Command::SetSignature { account_id: a.clone(), text: "second".into() }).unwrap();
    assert_eq!(core.signature(USER.into()).map(|s| s.text), Some("second".into()), "a second save replaces the first");
    core.run(Command::SetSignature { account_id: a.clone(), text: " \n ".into() }).unwrap();
    assert!(core.signature(USER.into()).is_none() && core.accounts().iter().all(|x| x.signature.is_empty()), "an empty text clears it");
}

// ---------------------------------------------------------------- restart and speed

/// Broken copy R12: compose_cmds.rs `rusqlite::params![id, account, to_json, cc_json, subject, body, thread, now_ms],` becomes
/// `rusqlite::params![format!("{id}x"), account, to_json, cc_json, subject, body, thread, now_ms],` (the draft id answered is not the stored one).
#[test]
fn the_draft_and_the_signature_survive_a_restart() {
    let (core, a, _b, dir) = local();
    put(&core, mail(&a, "m8", "t8", "ann@x.example", &[USER, "bob@x.example"], &["cy@x.example"], 1_000));
    let d = reply_all(&core, "m8");
    core.run(Command::SetSignature { account_id: a.clone(), text: SIG.into() }).unwrap();
    drop(core);
    let again = RustCore::with_parts(Store::open(&dir.path().join("whoosh.sqlite")).unwrap(), kc(), detector(), TestTransport::PlainLoopback);
    assert_eq!(list(&again, &d, "to"), vec!["ann@x.example", "bob@x.example"]);
    assert_eq!(list(&again, &d, "cc"), vec!["cy@x.example"]);
    assert_eq!(text(&again, &d, "reply_to_thread_id").as_deref(), Some("t8"));
    assert_eq!(again.signature(USER.into()).map(|s| s.text), Some(SIG.into()), "the signature is on this Mac still");
}

/// Broken copy P1: compose_cmds.rs `store_failed("the accounts could not be read", e))?;` becomes `store_failed("the accounts could not be read", e))?; std::thread::sleep(std::time::Duration::from_millis(80));`.
#[test]
fn both_commands_answer_in_under_50_ms_on_a_store_of_2000_mails() {
    let (core, a, _b, _d) = local();
    let batch: Vec<NewMessage> = (0..2000).map(|i| mail(&a, &format!("bulk{i}"), &format!("tb{i}"), "ann@x.example", &[USER, "bob@x.example"], &["cy@x.example"], 10_000 + i as i64)).collect();
    core.store().insert_messages(batch).unwrap();
    let t0 = Instant::now();
    let d = reply_all(&core, "bulk1999");
    let reply_ms = t0.elapsed().as_millis();
    let t1 = Instant::now();
    core.run(Command::SetSignature { account_id: a.clone(), text: SIG.into() }).unwrap();
    let sig_ms = t1.elapsed().as_millis();
    let t2 = Instant::now();
    let reads = (core.state(format!("draft/{d}/to")).is_some(), core.signature(USER.into()).is_some(), core.accounts().len());
    let read_ms = t2.elapsed().as_millis();
    println!("real-compose timing on 2000 mails: ReplyAll {reply_ms} ms, SetSignature {sig_ms} ms, the three reads {read_ms} ms {reads:?}");
    assert!(reply_ms < 50 && sig_ms < 50 && read_ms < 50, "ReplyAll {reply_ms} ms, SetSignature {sig_ms} ms, reads {read_ms} ms: the bar is 50 ms");
}

// ---------------------------------------------------------------- the send path (queued, undo window, threaded, once)

fn headers(data: &str) -> Vec<(String, String)> {
    let head = data.split_once("\r\n\r\n").map(|p| p.0).unwrap_or(data);
    head.replace("\r\n ", " ").replace("\r\n\t", " ").lines().filter_map(|l| l.split_once(':').map(|(n, v)| (n.to_ascii_lowercase(), v.trim().to_string()))).collect()
}
fn header(h: &[(String, String)], name: &str) -> Vec<String> { h.iter().filter(|(n, _)| n == name).map(|(_, v)| v.clone()).collect() }
fn addresses(v: &str) -> Vec<String> { v.split(',').map(|a| a.rsplit('<').next().unwrap().trim_end_matches('>').trim().to_string()).collect() }

/// Broken copies: R7 (the draft is in no thread: no In-Reply-To), R1 (the owner is in To), R3 (the Cc is lost); S6 below is the signature's.
#[test]
fn a_reply_all_draft_is_queued_with_an_undo_entry_threaded_from_the_stored_headers_and_sent_once() {
    let (core, a, _imap, smtp, _rt, _d) = added();
    put(&core, mail(&a, "ra1", "tra", "ann@example.test", &[USER, "bob@example.test", SECOND], &["cy@example.test", "Owner@Example.Test"], 5_000));
    core.store().set_thread_headers_quiet(vec![("ra1".into(), Some(ThreadHeaders { message_id: "<ra1@example.test>".into(), references: vec!["<ra0@example.test>".into()] }))]).unwrap();
    let d = reply_all(&core, "ra1");
    let out = core.run(Command::Send { draft_id: d.clone() }).expect("queued");
    let entry = out.created_id.clone().expect("the outbox entry");
    assert!(out.undo_entry_id.is_some(), "a send is one undo entry (the undo window)");
    assert_eq!(core.outbox().len(), 1, "open in the outbox until the window ends");
    assert!(smtp.got.lock().unwrap().is_empty(), "nothing left inside the window");
    core.run(Command::SendNow { outbox_id: entry.clone() }).expect("sent now");
    let got = smtp.got.lock().unwrap().clone();
    assert_eq!(got.len(), 1, "the SMTP server received exactly one message");
    let h = headers(&got[0]);
    assert_eq!(header(&h, "subject"), vec!["Re: Subject ra1".to_string()]);
    assert_eq!(header(&h, "in-reply-to"), vec!["<ra1@example.test>".to_string()], "threaded under the message");
    assert_eq!(header(&h, "references"), vec!["<ra0@example.test> <ra1@example.test>".to_string()], "the chain, then the parent, each once");
    assert_eq!(header(&h, "to").iter().flat_map(|v| addresses(v)).collect::<Vec<_>>(), vec!["ann@example.test", "bob@example.test"], "{}", got[0]);
    assert_eq!(header(&h, "cc").iter().flat_map(|v| addresses(v)).collect::<Vec<_>>(), vec!["cy@example.test"], "{}", got[0]);
    std::thread::sleep(Duration::from_millis(1_500));
    assert_eq!(smtp.got.lock().unwrap().len(), 1, "the timer does not send it a second time");
    assert!(core.outbox().is_empty());
}

/// Broken copy S6: outbox_tick.rs `self.store.signature(&a.account.address).map_err(|e| unexpected(format!("the signature could not be read: {e}")))?` becomes `None`
/// (the saved signature never leaves with a mail).
#[test]
fn a_saved_signature_leaves_with_the_mail_exactly_once_and_clearing_it_stops_it() {
    let (core, a, _imap, smtp, _rt, _d) = added();
    let send = |body: &str| -> String {
        let n = smtp.got.lock().unwrap().len();
        let draft = core.run(Command::SaveDraft { from_account_id: Some(a.clone()), to: vec!["ann@example.test".into()], cc: vec![], subject: "Hi".into(), body: body.into(), reply_to_thread_id: None }).unwrap().created_id.unwrap();
        let entry = core.run(Command::Send { draft_id: draft }).expect("queued").created_id.unwrap();
        core.run(Command::SendNow { outbox_id: entry }).expect("sent");
        let got = smtp.got.lock().unwrap().clone();
        assert_eq!(got.len(), n + 1, "one more message at SMTP");
        got[n].clone()
    };
    core.run(Command::SetSignature { account_id: a.clone(), text: SIG.into() }).unwrap();
    let plain = send("Hello there");
    assert_eq!(plain.matches(SIG).count(), 1, "a body without it gets the saved signature once: {plain}");
    assert!(plain.find("Hello there") < plain.find(SIG), "after the owner's words");
    let written = send(&format!("Hello again\n\n{SIG}"));
    assert_eq!(written.matches(SIG).count(), 1, "the composer already wrote it: still one copy: {written}");
    core.run(Command::SetSignature { account_id: a.clone(), text: "".into() }).unwrap();
    let cleared = send("Hello once more");
    assert_eq!(cleared.matches(SIG).count(), 0, "a cleared signature is not sent: {cleared}");
}
