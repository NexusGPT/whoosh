//! engine-act: the real core acts on mail (docs/lanes/engine-act.md). The mail comes from sync-imap's in-process IMAP fake (clear text
//! on 127.0.0.1); the SMTP server is a TcpListener of this file that ACCEPTS messages and keeps them (the sync-imap fake never does).
//! Nothing leaves this Mac. Each test was seen red on main's sources (the two routing arms removed = main's `not built yet`), and on
//! the single-edit mutant named in the lane note.
#[allow(dead_code, unused_imports)]
#[path = "../../autoconfig/tests/support/mod.rs"]
mod detect_fakes;
#[allow(dead_code, unused_imports)]
#[path = "../../sync-imap/tests/support/mod.rs"]
mod imap_fakes;

use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use contracts::*;
use detect_fakes::{FakeDns, FakeHttp, FakeProbe};
use engine::writeback::{backoff_ms, Ledger, Worker};
use engine::{RustCore, SyncPhase, TestTransport};
use imap_fakes::FakeImap;
use secrets::{MemoryKeychain, Namespace};
use store::{NewMessage, Store};

const USER: &str = "owner@example.test";
const PW: &str = "app-password";
const WAIT: Duration = Duration::from_secs(25);

fn detector() -> Box<dyn engine::Detect> { Box::new(autoconfig::Detector::new(FakeHttp::default(), FakeDns::default(), FakeProbe::default())) }
fn kc() -> Arc<MemoryKeychain> { Arc::new(MemoryKeychain::new(Namespace::rig())) }
fn tab(t: &str) -> InboxQuery { InboxQuery { tab_id: t.into(), account_id: None, label_id: None, mailbox: None, account_ids: vec![], filters: vec![], smart_folder_id: None } }
fn mailbox(m: Mailbox) -> InboxQuery { InboxQuery { mailbox: Some(m), ..tab("primary") } }
fn ids(rows: &[InboxRow]) -> Vec<String> { let mut v: Vec<String> = rows.iter().map(|r| r.message_id.clone()).collect(); v.sort(); v }
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
fn not_built(r: &Result<CommandOutcome, CoreError>) -> bool {
    matches!(r, Err(CoreError::Action { failure }) if failure.code.as_deref() == Some("engine.not_built_yet"))
}

// ---------------------------------------------------------------- an SMTP server that accepts mail

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

/// The IMAP fake with `n` INBOX messages and the account added through the core; the sync is listening when it returns.
fn added(n: u32, without_archive: bool) -> (RustCore, String, FakeImap, Smtp, tokio::runtime::Runtime, tempfile::TempDir) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let imap = rt.block_on(FakeImap::start());
    if without_archive { imap.with(|s| s.boxes.retain(|b| b.name != "Archive")); }
    imap.seed("INBOX", n);
    let smtp = smtp();
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(&dir.path().join("whoosh.sqlite")).unwrap();
    let core = RustCore::with_parts(store, kc(), detector(), TestTransport::PlainLoopback);
    let out = core.run(Command::AddAccountConfigured { address: USER.into(), display_name: Some("Owner".into()), config: config(imap.port, smtp.port), password: SecretText { value: PW.into() } }).expect("added");
    let id = out.created_id.unwrap();
    wait_until("listening", || core.sync_phase(&id) == Some(SyncPhase::Listening));
    (core, id, imap, smtp, rt, dir)
}
fn by_subject(core: &RustCore, s: &str) -> InboxRow { core.inbox(tab("primary")).into_iter().find(|r| r.subject == s).unwrap_or_else(|| panic!("no row {s}")) }
fn uid_of(core: &RustCore, id: &str) -> u32 { core.store().write({ let id = id.to_string(); move |c| Ok(c.tx.query_row("SELECT remote_id FROM message WHERE id = ?1", [&id], |r| r.get::<_, String>(0))?) }).unwrap().parse().unwrap() }
fn box_of(imap: &FakeImap, name: &str) -> usize { imap.with(|s| s.boxes.iter().find(|b| b.name == name).map(|b| b.msgs.len()).unwrap_or(0)) }

// ---------------------------------------------------------------- every arm answers

fn seeded() -> (RustCore, String, String, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(&dir.path().join("whoosh.sqlite")).unwrap();
    let a = store.add_account(USER, "Owner", MailProvider::Imap, "svc\u{1f}acct", false, config(1, 1), 1).unwrap();
    let b = store.add_account("second@example.test", "Second", MailProvider::Imap, "svc\u{1f}b", false, config(1, 1), 2).unwrap();
    let mut batch = Vec::new();
    for i in 1..=6 { batch.push(msg(&a, &format!("m{i}"), &format!("t{i}"), "sender@example.test", 1_000 + i as i64, Some(i.to_string()))); }
    store.insert_messages(batch).unwrap();
    (RustCore::with_parts(store, kc(), detector(), TestTransport::PlainLoopback), a, b, dir)
}
fn msg(account: &str, id: &str, thread: &str, from: &str, at: i64, uid: Option<String>) -> NewMessage {
    NewMessage {
        message: Message {
            id: id.into(), thread_id: thread.into(), account_id: account.into(), sender_name: from.into(), from_address: from.into(), to: vec![USER.into()],
            subject: format!("Subject {id}"), preview: format!("Preview {id}"), received_at_ms: at, read: false, archived: false, folder: "INBOX".into(),
            attachment_names: vec![], login_code: None, trackers_blocked: 0, list_unsubscribe: vec![], list_unsubscribe_one_click: false,
            reply_to_name: None, cc: vec![], bcc: vec![], is_outgoing: false,
        },
        remote_id: uid, body_text: None, body_html: None, has_list_header: false, size_bytes: Some(100), tab_id: None,
    }
}

#[test]
fn every_listed_command_answers_and_none_is_refused_as_not_built_yet() {
    let (core, a, b, _d) = seeded();
    let future = engine::SystemClock.now_ms() + 3_600_000;
    let draft = core.run(Command::SaveDraft { from_account_id: Some(a.clone()), to: vec!["x@example.test".into()], cc: vec![], subject: "Hi".into(), body: "words".into(), reply_to_thread_id: None }).expect("SaveDraft");
    let draft_id = draft.created_id.clone().expect("a draft id");
    let commands = vec![
        Command::Archive { message_ids: vec!["m1".into()] },
        Command::Snooze { message_ids: vec!["m2".into()], when: SnoozeWhen::At { at_ms: future } },
        Command::Pin { message_id: "m3".into(), pinned: true },
        Command::Label { message_ids: vec!["m3".into()], label_name: "Clients".into(), create_if_missing: true },
        Command::MoveToAccount { message_ids: vec!["m4".into()], account_id: b.clone() },
        Command::MarkRead { message_ids: vec!["m5".into()], read: true },
        Command::Star { message_ids: vec!["m5".into()], starred: true },
        Command::MarkSpam { message_ids: vec!["m6".into()] },
        Command::Undo,
        Command::UpdateDraft { draft_id: draft_id.clone(), from_account_id: None, to: vec!["y@example.test".into()], cc: vec![], bcc: vec![], subject: "Hi again".into(), body: "more".into() },
        Command::Schedule { draft_id: draft_id.clone(), at_ms: future },
        Command::QuickSend { thread_id: "t1".into(), body: "thanks".into() },
        Command::QuickSendFrom { thread_id: "t1".into(), body: "thanks".into(), from_account_id: a.clone() },
        Command::SendNow { outbox_id: "no-such-entry".into() },
        Command::CancelSend { outbox_id: "no-such-entry".into() },
        Command::Send { draft_id: "no-such-draft".into() },
    ];
    for c in commands {
        let shown = format!("{c:?}");
        let r = core.run(c);
        assert!(!not_built(&r), "{shown} is still refused as not built yet: {r:?}");
    }
    assert!(core.store().pending_actions().unwrap().iter().any(|r| r.kind == "set_archived"), "the archive left a write-back row");
}

#[test]
fn undo_answers_the_entry_it_reversed() {
    let (core, _a, _b, _d) = seeded();
    let archived = core.run(Command::Archive { message_ids: vec!["m1".into()] }).expect("archived");
    let entry = archived.undo_entry_id.expect("an undo entry");
    assert!(!ids(&core.inbox(tab("primary"))).contains(&"m1".to_string()), "archived = out of the inbox");
    let undone = core.run(Command::Undo).expect("undone");
    assert_eq!(undone.undo_entry_id.as_deref(), Some(entry.as_str()), "Undo names the entry it reversed, as the fixture core does");
    assert!(ids(&core.inbox(tab("primary"))).contains(&"m1".to_string()), "and the message is back");
}

#[test]
fn snooze_hides_the_row_and_the_snoozed_read_lists_it() {
    let (core, _a, _b, _d) = seeded();
    let at = engine::SystemClock.now_ms() + 3_600_000;
    core.run(Command::Snooze { message_ids: vec!["m2".into()], when: SnoozeWhen::At { at_ms: at } }).expect("snoozed");
    assert!(!ids(&core.inbox(tab("primary"))).contains(&"m2".to_string()), "the list hides it");
    assert_eq!(core.snoozed_messages(), vec![Snooze { message_id: "m2".into(), return_at_ms: at }]);
    assert!(core.store().pending_actions().unwrap().is_empty(), "a snooze is local: no server row");
}

#[test]
fn drafts_are_saved_updated_and_listed_and_send_takes_them_out() {
    let (core, a, _b, _d) = seeded();
    let saved = core.run(Command::SaveDraft { from_account_id: None, to: vec![], cc: vec![], subject: "".into(), body: "a reply".into(), reply_to_thread_id: Some("t1".into()) }).expect("saved");
    let id = saved.created_id.unwrap();
    assert_eq!(core.drafts("t1".into()).iter().map(|d| d.id.clone()).collect::<Vec<_>>(), vec![id.clone()], "the thread's drafts");
    core.run(Command::UpdateDraft { draft_id: id.clone(), from_account_id: Some(a), to: vec!["z@example.test".into()], cc: vec![], bcc: vec![], subject: "Re".into(), body: "edited".into() }).expect("updated");
    // a draft with no reply thread is sent as it stands (a reply's threading headers need its server, tested below)
    let plain = core.run(Command::SaveDraft { from_account_id: None, to: vec!["w@example.test".into()], cc: vec![], subject: "Plan".into(), body: "later".into(), reply_to_thread_id: None }).expect("saved").created_id.unwrap();
    let sched = core.run(Command::Schedule { draft_id: plain.clone(), at_ms: engine::SystemClock.now_ms() + 3_600_000 }).expect("scheduled");
    let entry = sched.created_id.unwrap();
    let left: i64 = core.store().write({ let p = plain.clone(); move |c| Ok(c.tx.query_row("SELECT count(*) FROM draft WHERE id = ?1", [&p], |r| r.get(0))?) }).unwrap();
    assert_eq!(left, 0, "a queued draft leaves the draft list");
    assert_eq!(core.outbox().iter().map(|e| e.id.clone()).collect::<Vec<_>>(), vec![entry.clone()], "the outbox read serves it");
    assert_eq!(ids(&core.inbox(mailbox(Mailbox::Outbox))), vec![plain], "and the Outbox mailbox lists it by its draft");
    assert_eq!(core.drafts("t1".into()).len(), 1, "the reply draft is still a draft");
    core.run(Command::CancelSend { outbox_id: entry }).expect("cancelled");
    assert!(core.outbox().is_empty(), "a cancelled entry is not open");
}

// ---------------------------------------------------------------- the ledger and the worker

struct FakeClock(AtomicI64);
impl Clock for FakeClock { fn now_ms(&self) -> i64 { self.0.load(Ordering::SeqCst) } fn utc_offset_s(&self) -> i32 { 0 } }
struct Recorder { refuse_first: Mutex<u32>, got: Mutex<Vec<actions::WriteBack>> }
impl actions::MailAdapter for Recorder {
    fn execute(&self, request: actions::WriteBack) -> Result<(), actions::Refusal> {
        let mut left = self.refuse_first.lock().unwrap();
        if *left > 0 { *left -= 1; return Err(actions::Refusal { account_id: "a".into(), reason_words: "NO [UNAVAILABLE] try later".into(), retryable: true }); }
        self.got.lock().unwrap().push(request);
        Ok(())
    }
}
struct Refusing;
impl actions::WriteBackLedger for Refusing {
    fn execute(&self, _: &mut store::WriteCtx<'_>, _: &actions::WriteBack, _: i64) -> Result<(), store::StoreError> { Err(store::StoreError::Invalid("the ledger is full".into())) }
}
fn plain_store() -> (Arc<Store>, String, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(&dir.path().join("whoosh.sqlite")).unwrap();
    let a = store.add_account(USER, "Owner", MailProvider::Imap, "svc\u{1f}acct", false, config(1, 1), 1).unwrap();
    store.insert_messages(vec![msg(&a, "m1", "t1", "s@example.test", 10, Some("7".into())), msg(&a, "m2", "t2", "s@example.test", 11, Some("8".into()))]).unwrap();
    (Arc::new(store), a, dir)
}
fn archived(store: &Store, id: &'static str) -> bool { store.write(move |c| Ok(c.tx.query_row("SELECT archived FROM message WHERE id = ?1", [id], |r| r.get::<_, i64>(0))? != 0)).unwrap() }

#[test]
fn the_write_back_row_is_written_in_the_transaction_of_the_local_change() {
    let (store, a, _d) = plain_store();
    let clock: Arc<dyn Clock> = Arc::new(FakeClock(AtomicI64::new(1_000)));
    let adapter = Arc::new(Recorder { refuse_first: Mutex::new(0), got: Mutex::new(vec![]) });
    // a ledger that refuses takes the local change back with it: they are one transaction
    let refusing = actions::Actions::new(actions::Env::new(store.clone(), adapter.clone(), clock.clone()).with_ledger(Arc::new(Refusing)));
    assert!(refusing.run(Command::Archive { message_ids: vec!["m1".into()] }).is_err(), "the refused ledger fails the action");
    assert!(!archived(&store, "m1"), "and the local archive did not stay");
    // the engine's ledger: the change and its row are there together, and the server is not asked inline
    let acts = actions::Actions::new(actions::Env::new(store.clone(), adapter.clone(), clock).with_ledger(Arc::new(Ledger)));
    acts.run(Command::Archive { message_ids: vec!["m1".into()] }).expect("archived");
    assert!(archived(&store, "m1"));
    let rows = store.pending_actions().unwrap();
    assert_eq!(rows.iter().map(|r| (r.account_id.clone(), r.folder.clone(), r.uid, r.kind.clone(), r.payload_json.clone())).collect::<Vec<_>>(),
        vec![(a, "INBOX".into(), Some(7), "set_archived".into(), r#"{"archived":true}"#.into())]);
    assert!(adapter.got.lock().unwrap().is_empty(), "with a ledger the adapter is the worker's, never called inline");
}

#[test]
fn the_worker_applies_the_rows_and_retries_a_refusal_with_a_doubling_capped_wait() {
    let (store, a, _d) = plain_store();
    let clock = Arc::new(FakeClock(AtomicI64::new(1_000)));
    let adapter = Arc::new(Recorder { refuse_first: Mutex::new(2), got: Mutex::new(vec![]) });
    let acts = actions::Actions::new(actions::Env::new(store.clone(), adapter.clone(), clock.clone()).with_ledger(Arc::new(Ledger)));
    acts.run(Command::Star { message_ids: vec!["m1".into(), "m2".into()], starred: true }).expect("starred");
    let worker = Worker::new(store.clone(), adapter.clone(), clock.clone());
    let r1 = worker.run_once();
    assert_eq!((r1.applied, r1.refused), (0, 2), "the refusal keeps both rows");
    let rows = store.pending_actions().unwrap();
    assert!(rows.iter().all(|r| r.tries == 1 && r.last_error.as_deref() == Some("NO [UNAVAILABLE] try later") && r.next_at_ms == 1_000 + backoff_ms(0)), "{rows:?}");
    assert_eq!(worker.run_once().refused, 0, "nothing is tried before its time");
    clock.0.store(1_000 + backoff_ms(0), Ordering::SeqCst);
    assert_eq!(worker.run_once().refused, 2, "the second refusal");
    let rows = store.pending_actions().unwrap();
    assert!(rows.iter().all(|r| r.tries == 2 && r.next_at_ms == 1_000 + backoff_ms(0) + backoff_ms(1)), "the wait doubles: {rows:?}");
    clock.0.store(1_000 + backoff_ms(0) + backoff_ms(1), Ordering::SeqCst);
    assert_eq!(worker.run_once().applied, 2);
    assert!(store.pending_actions().unwrap().is_empty(), "taken = gone");
    let got = adapter.got.lock().unwrap().clone();
    assert_eq!(got.len(), 1, "one request for the account, folder and kind: {got:?}");
    match &got[0] {
        actions::WriteBack::Label { account_id, messages, label_name, applied } => {
            assert_eq!((account_id.as_str(), label_name.as_str(), *applied), (a.as_str(), "Starred", true));
            assert_eq!(messages.iter().map(|m| (m.folder.clone(), m.remote_id.clone())).collect::<Vec<_>>(), vec![("INBOX".into(), Some("7".into())), ("INBOX".into(), Some("8".into()))]);
        }
        other => panic!("not the star: {other:?}"),
    }
    let max = imap_fakes::ledger("SYNC_BACKOFF_MAX_MS") as i64;
    assert_eq!(backoff_ms(0), imap_fakes::ledger("SYNC_BACKOFF_START_MS") as i64, "the first wait is the ledger's start");
    assert!((0..40).all(|t| backoff_ms(t) <= max) && backoff_ms(39) == max, "the wait never passes SYNC_BACKOFF_MAX_MS");
}

// ---------------------------------------------------------------- through the real IMAP server

#[test]
fn archive_star_read_label_and_spam_reach_the_imap_server() {
    let (core, _id, imap, _smtp, _rt, _d) = added(5, false);
    let (m1, m2, m3, m4, m5) = (by_subject(&core, "Message 1"), by_subject(&core, "Message 2"), by_subject(&core, "Message 3"), by_subject(&core, "Message 4"), by_subject(&core, "Message 5"));
    let (u2, u3, u4) = (uid_of(&core, &m2.message_id), uid_of(&core, &m3.message_id), uid_of(&core, &m4.message_id));
    core.run(Command::Archive { message_ids: vec![m1.message_id.clone()] }).expect("archive");
    core.run(Command::Star { message_ids: vec![m2.message_id.clone()], starred: true }).expect("star");
    core.run(Command::MarkRead { message_ids: vec![m3.message_id.clone()], read: true }).expect("read");
    core.run(Command::Label { message_ids: vec![m4.message_id.clone()], label_name: "Clients > Provanta".into(), create_if_missing: true }).expect("label");
    core.run(Command::MarkSpam { message_ids: vec![m5.message_id.clone()] }).expect("spam");
    wait_until("the archive moved to Archive", || box_of(&imap, "Archive") == 1);
    wait_until("the spam moved to Junk", || box_of(&imap, "Junk") == 1);
    wait_until("the star", || imap.flags_of("INBOX", u2).contains(&"\\Flagged".to_string()));
    wait_until("the read", || imap.flags_of("INBOX", u3).contains(&"\\Seen".to_string()));
    wait_until("the label keyword", || imap.flags_of("INBOX", u4).contains(&"Clients_Provanta".to_string()));
    assert_eq!(box_of(&imap, "INBOX"), 3, "two of five left the server's INBOX");
    wait_until("every row taken", || core.store().pending_actions().unwrap().is_empty());
}

#[test]
fn archive_on_a_server_with_no_archive_folder_sets_the_archived_flag() {
    let (core, _id, imap, _smtp, _rt, _d) = added(1, true);
    let m1 = by_subject(&core, "Message 1");
    let u1 = uid_of(&core, &m1.message_id);
    core.run(Command::Archive { message_ids: vec![m1.message_id.clone()] }).expect("archive");
    wait_until("the flag", || imap.flags_of("INBOX", u1).contains(&"\\Archived".to_string()));
}

#[test]
fn a_refused_write_back_stays_with_the_servers_words_and_the_local_change_stays() {
    let (core, _id, imap, _smtp, _rt, _d) = added(1, false);
    let m1 = by_subject(&core, "Message 1");
    imap.fault(imap_fakes::Fault::bad("UID MOVE", "MOVE refused for now").times(50));
    core.run(Command::Archive { message_ids: vec![m1.message_id.clone()] }).expect("archive commits locally");
    wait_until("the refusal kept", || core.store().pending_actions().unwrap().iter().any(|r| r.tries >= 1 && r.last_error.as_deref().is_some_and(|w| w.contains("MOVE refused for now"))));
    let row = core.store().pending_actions().unwrap().remove(0);
    assert!(row.next_at_ms > row.created_ms, "tried again later, not at once: {row:?}");
    assert_eq!(box_of(&imap, "Archive"), 0, "the server did not take it");
    assert!(!ids(&core.inbox(tab("primary"))).contains(&m1.message_id), "the local archive stays (optimistic)");
}

// ---------------------------------------------------------------- the outbox and the send

#[test]
fn a_quick_reply_goes_out_through_smtp_reaches_sent_and_joins_its_thread() {
    let (core, id, imap, smtp, _rt, _d) = added(2, false);
    let m1 = by_subject(&core, "Message 1");
    let out = core.run(Command::QuickSend { thread_id: m1.thread_id.clone(), body: "Thanks, that works.".into() }).expect("queued");
    let entry = out.created_id.clone().expect("the outbox entry");
    assert!(out.undo_entry_id.is_some(), "a send is one undo entry");
    assert_eq!(core.outbox().len(), 1, "open in the outbox");
    core.run(Command::SendNow { outbox_id: entry.clone() }).expect("sent now");
    let got = smtp.got.lock().unwrap().clone();
    assert_eq!(got.len(), 1, "the SMTP server received the reply");
    assert!(got[0].contains("Subject: Re: Message 1") && got[0].contains("In-Reply-To: <1@example.test>") && got[0].contains("Thanks, that works."), "{}", got[0]);
    assert!(got[0].contains("To: sender1@example.test") || got[0].contains("<sender1@example.test>"), "to the sender: {}", got[0]);
    assert_eq!(box_of(&imap, "Sent"), 1, "the Sent copy was appended on the server");
    let sent_id = format!("sent-{entry}");
    let copy = core.message(sent_id.clone()).expect("the stored copy");
    assert!(copy.is_outgoing && copy.read && copy.thread_id == m1.thread_id && copy.account_id == id, "{copy:?}");
    assert!(core.thread(m1.thread_id.clone()).expect("thread").message_ids.contains(&sent_id), "it joins its thread");
    assert!(!ids(&core.inbox(tab("primary"))).contains(&sent_id), "never an inbox row");
    assert!(core.outbox().is_empty(), "the entry is no longer open");
    assert!(matches!(core.run(Command::SendNow { outbox_id: entry }), Err(CoreError::Invalid { .. })), "a sent entry is refused in words");
}

#[test]
fn undo_inside_the_window_cancels_the_send_and_the_timer_sends_the_other() {
    let (core, _id, _imap, smtp, _rt, _d) = added(2, false);
    let (m1, m2) = (by_subject(&core, "Message 1"), by_subject(&core, "Message 2"));
    let kept = core.run(Command::QuickSend { thread_id: m2.thread_id.clone(), body: "This one goes.".into() }).expect("queued").created_id.unwrap();
    let undone = core.run(Command::QuickSend { thread_id: m1.thread_id.clone(), body: "This one is taken back.".into() }).expect("queued");
    let r = core.run(Command::Undo).expect("undo inside the window");
    assert_eq!(r.undo_entry_id, undone.undo_entry_id, "Z reversed the newest send");
    assert_eq!(core.outbox().iter().map(|e| e.id.clone()).collect::<Vec<_>>(), vec![kept.clone()], "the undone entry is cancelled");
    // the timer sends the other one when its window ends (UNDO_SEND_DEFAULT_S), and never the cancelled one
    wait_until("the kept reply delivered by the timer", || !smtp.got.lock().unwrap().is_empty());
    std::thread::sleep(Duration::from_millis(1_500));
    let got = smtp.got.lock().unwrap().clone();
    assert_eq!(got.len(), 1, "exactly one message reached SMTP");
    assert!(got[0].contains("This one goes.") && !got[0].contains("taken back"), "{}", got[0]);
    wait_until("the kept reply joined its thread", || core.message(format!("sent-{kept}")).is_some());
}

#[test]
fn a_reply_reminder_held_on_the_queued_entry_survives_the_send_and_rides_the_sent_copy() {
    let (core, _id, _imap, smtp, _rt, _d) = added(1, false);
    let m1 = by_subject(&core, "Message 1");
    let entry = core.run(Command::QuickSend { thread_id: m1.thread_id.clone(), body: "Any news?".into() }).expect("queued").created_id.unwrap();
    let at = engine::SystemClock.now_ms() + 86_400_000;
    core.run(Command::RemindIfNoReply { draft_id: entry.clone(), at_ms: at }).expect("reminder on the queued entry");
    core.run(Command::SendNow { outbox_id: entry.clone() }).expect("sent");
    assert_eq!(smtp.got.lock().unwrap().len(), 1);
    let sent = format!("sent-{entry}");
    let held: i64 = core.store().write({ let s = sent.clone(); move |c| Ok(c.tx.query_row("SELECT count(*) FROM reminder WHERE message_id = ?1 AND if_no_reply = 1 AND when_ms = ?2", rusqlite::params![s, at], |r| r.get(0))?) }).unwrap();
    assert_eq!(held, 1, "the reminder rides the sent copy");
    let state = core.state(format!("thread/{}/reminder", m1.thread_id)).expect("the reminder state");
    assert!(state.contains(r#""state":"waiting""#) && state.contains(&format!(r#""at_ms":{at}"#)), "{state}");
}

// ---------------------------------------------------------------- the list's own reads (sync-bye-r2 finding, main53)

/// What the app's list asks (Sources/App/AppModel.swift `refresh`): tab "primary", the account filter the add flow sets
/// (Sources/Features/AddAccount/AddAccountActions.swift: `accountFilter = id`), no mailbox; and the view reads the tab strip uses.
#[test]
fn three_synced_inbox_messages_answer_the_lists_query_and_the_view_reads() {
    let (core, id, _imap, _smtp, _rt, _d) = added(3, false);
    let want = vec!["Message 1".to_string(), "Message 2".to_string(), "Message 3".to_string()];
    let subjects = |q: InboxQuery| { let mut s: Vec<String> = core.inbox(q).into_iter().map(|r| r.subject).collect(); s.sort(); s };
    assert_eq!(subjects(tab("primary")), want, "the list's query, no account filter");
    assert_eq!(subjects(InboxQuery { account_id: Some(id.clone()), ..tab("primary") }), want, "the list's query with the account filter the add flow sets");
    assert_eq!(subjects(InboxQuery { account_ids: vec![id.clone()], ..tab("primary") }), want, "the account set form");
    assert_eq!(subjects(tab("all")), want, "the All view");
    assert_eq!(core.state("view/primary/count".into()).as_deref(), Some("3"), "the view's count");
    assert_eq!(core.state("view/primary/unread".into()).as_deref(), Some("3"), "the view's unread");
    let strip = core.state("inbox/tabs".into()).expect("the tab strip");
    assert!(strip.contains(r#""id":"primary","kind":"view""#) && strip.contains(r#""unread":3"#), "{strip}");
    assert_eq!(core.counts().tabs.iter().find(|t| t.tab_id == "primary").map(|t| t.unread), Some(3), "the counts the tab badge reads");
}

/// sync-words' `retry_now`, called by the mailbox read: opening the failed account's list syncs it again at once.
#[test]
fn opening_a_failed_accounts_list_syncs_it_again_at_once() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let imap = rt.block_on(FakeImap::start());
    imap.seed("INBOX", 4);
    // one refused SEARCH ends the run; a refusal is not retried by the clock, so only the owner's action brings it back
    imap.fault(imap_fakes::Fault::bad("UID SEARCH", "NO [UNAVAILABLE] busy"));
    let smtp = smtp();
    let dir = tempfile::tempdir().unwrap();
    let core = RustCore::with_parts(Store::open(&dir.path().join("whoosh.sqlite")).unwrap(), kc(), detector(), TestTransport::PlainLoopback);
    let id = core.run(Command::AddAccountConfigured { address: USER.into(), display_name: Some("Owner".into()), config: config(imap.port, smtp.port), password: SecretText { value: PW.into() } }).expect("added").created_id.unwrap();
    wait_until("the failure", || matches!(core.sync_phase(&id), Some(SyncPhase::Failed(_))));
    std::thread::sleep(Duration::from_millis(300));
    assert!(matches!(core.sync_phase(&id), Some(SyncPhase::Failed(_))), "it stays failed with nobody acting on it");
    let _ = core.inbox(InboxQuery { account_id: Some(id.clone()), ..tab("primary") });   // the owner opens its list
    wait_until("synced again after the list read", || core.sync_phase(&id) == Some(SyncPhase::Listening));
    wait_until("its four messages listed", || core.inbox(InboxQuery { account_id: Some(id.clone()), ..tab("primary") }).len() == 4);
}

struct Heard(Mutex<Vec<CoreEvent>>);
impl EventSink for Heard { fn on_event(&self, event: CoreEvent) { self.0.lock().unwrap().push(event) } }

/// The core's side of the list (the empty list on main53 is the app's: nothing re-read AppModel.rows on these events). After the sync
/// stores its page, the inbox read AND the Inbox stream carry the three messages, and the Counts stream carries their unread count.
/// Mutant: RustCore::subscribe stops handing the sink to the store (`self.store.subscribe(stream, sink)` becomes `{ let _ = sink; 0 }`).
#[test]
fn after_a_sync_page_the_inbox_read_and_the_inbox_stream_both_carry_the_stored_messages() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let imap = rt.block_on(FakeImap::start());
    imap.seed("INBOX", 3);
    let smtp = smtp();
    let dir = tempfile::tempdir().unwrap();
    let core = RustCore::with_parts(Store::open(&dir.path().join("whoosh.sqlite")).unwrap(), kc(), detector(), TestTransport::PlainLoopback);
    let (inbox, counts) = (Arc::new(Heard(Mutex::new(vec![]))), Arc::new(Heard(Mutex::new(vec![]))));
    core.subscribe(StreamKind::Inbox, inbox.clone());
    core.subscribe(StreamKind::Counts, counts.clone());
    let id = core.run(Command::AddAccountConfigured { address: USER.into(), display_name: Some("Owner".into()), config: config(imap.port, smtp.port), password: SecretText { value: PW.into() } }).expect("added").created_id.unwrap();
    wait_until("listening", || core.sync_phase(&id) == Some(SyncPhase::Listening));
    let want = vec!["Message 1".to_string(), "Message 2".to_string(), "Message 3".to_string()];
    let mut read: Vec<String> = core.inbox(tab("primary")).into_iter().map(|r| r.subject).collect();
    read.sort();
    assert_eq!(read, want, "the inbox read");
    let streamed = |e: &CoreEvent| match e {
        CoreEvent::InboxChanged { tab_id, rows } if tab_id == "primary" => { let mut s: Vec<String> = rows.iter().map(|r| r.subject.clone()).collect(); s.sort(); Some(s) }
        _ => None,
    };
    wait_until("the Inbox stream carries the three", || inbox.0.lock().unwrap().iter().filter_map(streamed).any(|s| s == want));
    wait_until("the Counts stream carries their unread", || counts.0.lock().unwrap().iter().any(|e| matches!(e, CoreEvent::CountsChanged { counts } if counts.tabs.iter().any(|t| t.tab_id == "primary" && t.unread == 3))));
}
