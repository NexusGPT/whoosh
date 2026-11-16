//! lane real-notes (docs/lanes/real-notes.md): the real engine serves AddNote, EditNote, DeleteNote, SetMcpGrant, SetMcpServer, AnswerMcpApproval,
//! AcceptSuggestion, DismissSuggestion, MergeContacts and BlockDomain, and the state paths and reads the app uses for them. Every test runs a
//! seeded store on this Mac (no mail server, no socket but the loopback one the fence listens on). Each test prints `PASS <case>` lines and
//! `TIME <command> <ms> ms` lines; each command's write stays under 50 ms on a store of 2,000 mails.
//! BROKEN COPIES (one per behaviour: the exact `find` and `replace`, each `find` once in its file, and the test that goes red): docs/lanes/real-notes.md, table, last column.
#[allow(dead_code, unused_imports)]
#[path = "../../autoconfig/tests/support/mod.rs"]
mod detect_fakes;

use std::sync::{Arc, Mutex};
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
/// A store with a few mails: t1 holds m1 (ann) and m2 (ann), t2 holds m3 (bob), t3 holds m4 (ann's other address) and m5 (spam).
fn seeded() -> (RustCore, String, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    let (core, id) = open_core(&dir.path().join("whoosh.sqlite"));
    core.store().insert_messages(vec![
        msg(&id, "m1", "t1", "ann@ext.test", "Ann", 100), msg(&id, "m2", "t1", "ann@ext.test", "Ann", 110), msg(&id, "m3", "t2", "bob@ext.test", "Bob", 120),
        msg(&id, "m4", "t3", "ann.work@corp.test", "Ann W", 130), msg(&id, "m5", "t4", "promo@spam.test", "Promo", 140),
    ]).unwrap();
    (core, id, dir)
}
/// The same, with 1,995 more mails: 2,000 in all.
fn big() -> (RustCore, String, tempfile::TempDir) {
    let (core, id, dir) = seeded();
    let more: Vec<NewMessage> = (0..1995).map(|i| msg(&id, &format!("b{i}"), &format!("bt{}", i / 3), &format!("sender{}@bulk{}.test", i % 40, i % 7), "Bulk", 1000 + i as i64)).collect();
    core.store().insert_messages(more).unwrap();
    (core, id, dir)
}
fn reopen(core: RustCore, dir: &tempfile::TempDir) -> RustCore {
    drop(core);
    open_core(&dir.path().join("whoosh.sqlite")).0
}
fn timed<T>(what: &str, f: impl FnOnce() -> T) -> T {
    let t0 = Instant::now();
    let r = f();
    let ms = t0.elapsed().as_millis();
    println!("TIME {what} {ms} ms");
    assert!(ms < 50, "{what} took {ms} ms, the bar is 50");
    r
}
fn wait_until(what: &str, mut ok: impl FnMut() -> bool) {
    let t0 = Instant::now();
    while t0.elapsed() < Duration::from_secs(20) { if ok() { return; } std::thread::sleep(Duration::from_millis(20)); }
    panic!("never reached: {what}");
}
fn invalid(r: Result<CommandOutcome, CoreError>) -> (String, String) {
    match r { Err(CoreError::Invalid { field, reason_words }) => (field, reason_words), other => panic!("not an Invalid refusal: {other:?}") }
}
fn not_found(r: Result<CommandOutcome, CoreError>) -> (String, String) {
    match r { Err(CoreError::NotFound { kind, id }) => (kind, id), other => panic!("not a NotFound refusal: {other:?}") }
}
fn now() -> i64 { std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis() as i64 }
fn thread(t: &str) -> NoteTarget { NoteTarget::Thread { thread_id: t.into() } }
fn message(m: &str) -> NoteTarget { NoteTarget::Message { message_id: m.into() } }
fn st(core: &RustCore, p: &str) -> Option<String> { core.state(p.to_string()) }
fn sug(id: &str, kind: SuggestionKind, subject: &str, label: Option<&str>) -> Suggestion {
    Suggestion { id: id.into(), kind, subject: subject.into(), label_id: label.map(String::from), offered_at_ms: now(), accepted: false }
}

// ================================================================ notes

#[test]
fn note_commands_write_read_back_refuse_and_survive_a_restart() {
    let (core, _acc, dir) = big();
    // a thread note and a message note: separate lists
    let a = timed("AddNote(thread)", || core.run(Command::AddNote { target: thread("t1"), text: "  call her back  ".into() })).expect("added");
    let a_id = a.created_id.clone().expect("created_id is the note");
    assert!(a.undo_entry_id.is_some(), "a note is undoable (the test engine pushes .noteAdd)");
    let b = timed("AddNote(message)", || core.run(Command::AddNote { target: message("m1"), text: "ask about the invoice".into() })).expect("added");
    let b_id = b.created_id.unwrap();
    assert_ne!(a_id, b_id);
    let on_thread = core.notes(thread("t1"));
    assert_eq!(on_thread.len(), 1, "a message's note is not on the thread's list: {on_thread:?}");
    assert_eq!((on_thread[0].id.as_str(), on_thread[0].text.as_str(), &on_thread[0].target), (a_id.as_str(), "call her back", &thread("t1")), "words are trimmed");
    let on_message = core.notes(message("m1"));
    assert_eq!((on_message.len(), on_message[0].text.as_str(), &on_message[0].target), (1, "ask about the invoice", &message("m1")));
    assert!(core.notes(message("m2")).is_empty() && core.notes(thread("t2")).is_empty(), "nothing on other targets");
    println!("PASS notes-written-and-read-back-per-target");
    // the state paths the app and the checks read
    assert_eq!(st(&core, "note/count").as_deref(), Some("2"));
    assert_eq!(st(&core, &format!("note/{a_id}/text")).as_deref(), Some("\"call her back\""));
    assert_eq!(st(&core, &format!("note/{a_id}/target")).as_deref(), Some("\"thread:t1\""));
    assert_eq!(st(&core, &format!("note/{b_id}/target")).as_deref(), Some("\"message:m1\""));
    assert_eq!(st(&core, "note/note-999/text"), None, "an unknown note answers nothing");
    println!("PASS note-state-paths");
    // refusals write nothing
    assert_eq!(invalid(core.run(Command::AddNote { target: thread("t1"), text: "   ".into() })), ("text".into(), "a note needs words".into()));
    assert_eq!(not_found(core.run(Command::AddNote { target: thread("ghost"), text: "x".into() })), ("thread".into(), "ghost".into()));
    assert_eq!(not_found(core.run(Command::AddNote { target: message("ghost"), text: "x".into() })), ("message".into(), "ghost".into()));
    assert_eq!(not_found(core.run(Command::EditNote { note_id: "note-999".into(), text: "x".into() })), ("note".into(), "note-999".into()));
    assert_eq!(invalid(core.run(Command::EditNote { note_id: a_id.clone(), text: "".into() })).0, "text");
    assert_eq!(not_found(core.run(Command::DeleteNote { note_id: "note-999".into() })), ("note".into(), "note-999".into()));
    assert_eq!(st(&core, "note/count").as_deref(), Some("2"), "no refusal wrote anything");
    assert_eq!(st(&core, &format!("note/{a_id}/text")).as_deref(), Some("\"call her back\""));
    println!("PASS note-refusals-write-nothing");
    // edit and delete
    let e = timed("EditNote", || core.run(Command::EditNote { note_id: a_id.clone(), text: "call her back at five".into() })).expect("edited");
    assert!(e.undo_entry_id.is_some() && e.created_id.is_none());
    assert_eq!(st(&core, &format!("note/{a_id}/text")).as_deref(), Some("\"call her back at five\""));
    // a restart: the notes are on disk
    let core = reopen(core, &dir);
    assert_eq!(core.notes(thread("t1"))[0].text, "call her back at five");
    assert_eq!(core.notes(message("m1"))[0].text, "ask about the invoice");
    assert_eq!(st(&core, "note/count").as_deref(), Some("2"));
    println!("PASS notes-survive-a-restart");
    let d = timed("DeleteNote", || core.run(Command::DeleteNote { note_id: a_id.clone() })).expect("deleted");
    assert!(d.undo_entry_id.is_some());
    assert!(core.notes(thread("t1")).is_empty());
    assert_eq!(st(&core, "note/count").as_deref(), Some("1"));
    assert_eq!(st(&core, &format!("note/{a_id}/text")), None);
    println!("PASS note-deleted");
}

#[test]
fn note_edit_keeps_its_place_and_its_written_time() {
    let (core, _acc, _dir) = seeded();
    let first = core.run(Command::AddNote { target: thread("t1"), text: "one".into() }).unwrap().created_id.unwrap();
    std::thread::sleep(Duration::from_millis(5));
    let second = core.run(Command::AddNote { target: thread("t1"), text: "two".into() }).unwrap().created_id.unwrap();
    let before = core.notes(thread("t1"));
    assert_eq!(before.iter().map(|n| n.id.clone()).collect::<Vec<_>>(), vec![first.clone(), second.clone()], "oldest first");
    std::thread::sleep(Duration::from_millis(5));
    core.run(Command::EditNote { note_id: first.clone(), text: "one, edited".into() }).unwrap();
    let after = core.notes(thread("t1"));
    assert_eq!(after.iter().map(|n| n.id.clone()).collect::<Vec<_>>(), vec![first, second], "the edited note keeps its place");
    assert_eq!(after[0].text, "one, edited");
    assert_eq!(after[0].written_at_ms, before[0].written_at_ms, "and its written time");
    println!("PASS note-edit-keeps-place-and-written-time");
}

#[test]
fn note_z_reverses_add_edit_and_delete_and_never_sticks() {
    let (core, _acc, _dir) = seeded();
    let id = core.run(Command::AddNote { target: message("m3"), text: "first words".into() }).unwrap().created_id.unwrap();
    let written = core.notes(message("m3"))[0].written_at_ms;
    core.run(Command::EditNote { note_id: id.clone(), text: "second words".into() }).unwrap();
    core.run(Command::DeleteNote { note_id: id.clone() }).unwrap();
    assert!(core.notes(message("m3")).is_empty());
    // Z 1: the deleted note is back with its id, target, words and written time
    core.run(Command::Undo).expect("Z after a delete");
    let back = core.notes(message("m3"));
    assert_eq!((back.len(), back[0].id.as_str(), back[0].text.as_str(), back[0].written_at_ms, &back[0].target), (1, id.as_str(), "second words", written, &message("m3")));
    // Z 2: the edit goes
    core.run(Command::Undo).expect("Z after an edit");
    assert_eq!(core.notes(message("m3"))[0].text, "first words");
    // Z 3: the add goes
    core.run(Command::Undo).expect("Z after an add");
    assert!(core.notes(message("m3")).is_empty() && st(&core, "note/count").as_deref() == Some("0"));
    // the stack is empty: the mail actions' own refusal in words, never a stuck entry
    let empty = core.run(Command::Undo);
    assert!(empty.is_err() && format!("{empty:?}").contains("Nothing to undo"), "{empty:?}");
    println!("PASS note-z-add-edit-delete-and-empty-stack");
    // an entry that can never apply (its thread is gone) is consumed and refused, and Z reaches the entries under it
    let keep = core.run(Command::AddNote { target: thread("t2"), text: "stays".into() }).unwrap().created_id.unwrap();
    let doomed = core.run(Command::AddNote { target: thread("t1"), text: "doomed".into() }).unwrap().created_id.unwrap();
    core.run(Command::DeleteNote { note_id: doomed.clone() }).unwrap();
    core.store().write(|c| { c.tx.execute_batch("DELETE FROM message WHERE thread_id = 't1'; DELETE FROM thread WHERE id = 't1';")?; Ok(()) }).unwrap();
    assert_eq!(not_found(core.run(Command::Undo)).0, "thread", "the delete cannot come back");
    assert_eq!(not_found(core.run(Command::Undo)).0, "note", "the add of a note that is gone: refused and consumed");
    core.run(Command::Undo).expect("the third Z reaches the entry under them: the first note's add");
    assert_eq!(st(&core, &format!("note/{keep}/text")), None, "the stack was never stuck");
    println!("PASS note-z-a-dead-entry-never-blocks");
}

// ================================================================ MCP (grants, server, answers)

/// Every name a guarded lookup of this process resolves is recorded and answered with this Mac's counting listener.
struct Fence { names: Mutex<Vec<String>>, listener: std::net::TcpListener, port: u16 }
impl netguard::Resolve for Fence {
    fn lookup(&self, host: &str, port: u16) -> std::io::Result<Vec<std::net::SocketAddr>> {
        self.names.lock().unwrap_or_else(|e| e.into_inner()).push(host.to_string());
        if netguard::is_this_mac(host) { return Ok(vec![std::net::SocketAddr::from(([127, 0, 0, 1], port))]); }
        Ok(vec![std::net::SocketAddr::from(([127, 0, 0, 1], self.port))])
    }
}
fn fence() -> &'static Arc<Fence> {
    static F: std::sync::OnceLock<Arc<Fence>> = std::sync::OnceLock::new();
    F.get_or_init(|| {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let port = listener.local_addr().unwrap().port();
        let f = Arc::new(Fence { names: Mutex::new(Vec::new()), listener, port });
        netguard::set_resolver(f.clone());
        f
    })
}
fn accepted(f: &Fence) -> usize { let mut n = 0; while f.listener.accept().is_ok() { n += 1; } n }

#[test]
fn mcp_grants_server_and_answers_are_kept_and_survive_a_restart() {
    let (core, _acc, dir) = big();
    // before any choice: no grant row, the server is off, the apps read false (state_extra's answer for an app with no row)
    assert!(core.mcp_grants().is_empty());
    assert_eq!(st(&core, "mcp_grant/Claude Desktop").as_deref(), Some("false"));
    assert_eq!(st(&core, "mcp_server/on").as_deref(), Some("false"));
    assert_eq!(st(&core, "mcp_request/pending_count").as_deref(), Some("0"));
    // grants: first-seen order, a revoke keeps the row with no grant time
    timed("SetMcpGrant(allow)", || core.run(Command::SetMcpGrant { app_name: "Claude Desktop".into(), allowed: true })).expect("allowed");
    core.run(Command::SetMcpGrant { app_name: "Cursor".into(), allowed: true }).expect("allowed");
    core.run(Command::SetMcpGrant { app_name: "Raycast".into(), allowed: false }).expect("a refusal to a new app is a row too");
    let g = core.mcp_grants();
    assert_eq!(g.iter().map(|x| (x.app_name.as_str(), x.allowed)).collect::<Vec<_>>(), vec![("Claude Desktop", true), ("Cursor", true), ("Raycast", false)]);
    assert!(g[0].granted_at_ms.is_some() && g[2].granted_at_ms.is_none());
    assert_eq!(st(&core, "mcp_grant/Claude Desktop").as_deref(), Some("true"));
    assert_eq!(st(&core, "mcp_grant/Raycast").as_deref(), Some("false"));
    timed("SetMcpGrant(revoke)", || core.run(Command::SetMcpGrant { app_name: "Cursor".into(), allowed: false })).expect("revoked");
    assert_eq!(st(&core, "mcp_grant/Cursor").as_deref(), Some("false"));
    assert!(core.mcp_grants().iter().find(|x| x.app_name == "Cursor").unwrap().granted_at_ms.is_none());
    assert_eq!(invalid(core.run(Command::SetMcpGrant { app_name: "  ".into(), allowed: true })).0, "app");
    assert_eq!(core.mcp_grants().len(), 3, "the refusal wrote nothing");
    println!("PASS mcp-grants-kept-in-first-seen-order");
    // the server switch
    timed("SetMcpServer", || core.run(Command::SetMcpServer { on: true })).expect("on");
    assert_eq!(st(&core, "mcp_server/on").as_deref(), Some("true"));
    // approvals: an unknown request is refused; the host that serves outside apps records a request, the owner answers it
    assert_eq!(not_found(core.run(Command::AnswerMcpApproval { request_id: "req-0".into(), allowed: true })), ("approval request".into(), "req-0".into()));
    for r in ["req-1", "req-2", "req-3"] { core.store().mcp_approval_ask(r.into(), "Claude Desktop".into(), "search_mail".into(), "search your mail".into(), now()).unwrap(); }
    core.store().mcp_approval_ask("req-4".into(), "Cursor".into(), "search_mail".into(), "search your mail".into(), now()).unwrap();
    assert_eq!(st(&core, "mcp_request/pending_count").as_deref(), Some("4"));
    assert_eq!(st(&core, "mcp_request/req-1/state").as_deref(), Some("\"pending\""));
    timed("AnswerMcpApproval", || core.run(Command::AnswerMcpApproval { request_id: "req-1".into(), allowed: true })).expect("answered");
    assert_eq!(st(&core, "mcp_request/req-1/state").as_deref(), Some("\"allowed\""));
    assert_eq!(st(&core, "mcp_request/req-1/results").as_deref(), Some("[]"), "the engine runs no outside search");
    core.run(Command::AnswerMcpApproval { request_id: "req-2".into(), allowed: false }).expect("answered");
    assert_eq!(st(&core, "mcp_request/req-2/state").as_deref(), Some("\"refused\""));
    // a revoke made while the prompt was open still wins (req-4 is Cursor's, revoked above)
    core.run(Command::AnswerMcpApproval { request_id: "req-4".into(), allowed: true }).expect("answered");
    assert_eq!(st(&core, "mcp_request/req-4/state").as_deref(), Some("\"refused\""), "the grant is read again at the answer");
    // an answered request cannot be answered twice
    assert_eq!(not_found(core.run(Command::AnswerMcpApproval { request_id: "req-1".into(), allowed: false })).0, "approval request");
    assert_eq!(st(&core, "mcp_request/req-1/state").as_deref(), Some("\"allowed\""), "the second answer changed nothing");
    assert_eq!(st(&core, "mcp_request/pending_count").as_deref(), Some("1"));
    println!("PASS mcp-server-and-answers-kept");
    // a restart keeps all of it
    let core = reopen(core, &dir);
    assert_eq!(core.mcp_grants().iter().map(|x| (x.app_name.as_str(), x.allowed)).collect::<Vec<_>>(), vec![("Claude Desktop", true), ("Cursor", false), ("Raycast", false)]);
    assert_eq!(st(&core, "mcp_server/on").as_deref(), Some("true"));
    assert_eq!(st(&core, "mcp_request/req-1/state").as_deref(), Some("\"allowed\""));
    assert_eq!(st(&core, "mcp_request/req-3/state").as_deref(), Some("\"pending\""));
    core.run(Command::SetMcpServer { on: false }).expect("off");
    assert_eq!(st(&core, "mcp_server/on").as_deref(), Some("false"));
    println!("PASS mcp-survives-a-restart");
}

#[test]
fn a_grant_write_opens_no_connection() {
    let f = fence();
    let (core, _acc, _dir) = seeded();
    let (names_before, accepted_before) = (f.names.lock().unwrap().len(), accepted(f));
    core.run(Command::SetMcpServer { on: true }).unwrap();
    core.run(Command::SetMcpGrant { app_name: "Claude Desktop".into(), allowed: true }).unwrap();
    core.store().mcp_approval_ask("req-x".into(), "Claude Desktop".into(), "create_draft".into(), "write a draft".into(), now()).unwrap();
    core.run(Command::AnswerMcpApproval { request_id: "req-x".into(), allowed: true }).unwrap();
    core.run(Command::SetMcpGrant { app_name: "Claude Desktop".into(), allowed: false }).unwrap();
    std::thread::sleep(Duration::from_millis(150));
    assert_eq!(f.names.lock().unwrap().len(), names_before, "no name was looked up: {:?}", f.names.lock().unwrap());
    assert_eq!(accepted(f), accepted_before, "nothing reached the counting listener");
    // and the code that keeps them holds no socket call at all
    for (name, text) in [("notes_mcp.rs", include_str!("../../store/src/notes_mcp.rs")), ("notes_cmds.rs", include_str!("../src/notes_cmds.rs"))] {
        for word in ["TcpStream", "TcpListener", "UdpSocket", "reqwest", "connect_timeout", "ToSocketAddrs", "UnixStream"] {
            assert!(!text.lines().filter(|l| !l.trim_start().starts_with("//")).any(|l| l.contains(word)), "{name} mentions {word}");
        }
    }
    println!("PASS a-grant-write-opens-no-connection");
}

// ================================================================ suggestions

#[test]
fn suggestions_are_accepted_dismissed_and_kept() {
    let (core, _acc, dir) = big();
    let s = core.store();
    s.suggestion_put(sug("s-sender", SuggestionKind::BlockSender, "Promo@Spam.test", None)).unwrap();
    assert_eq!(core.suggestions().len(), 1);
    assert_eq!(st(&core, "suggestion/count").as_deref(), Some("1"));
    assert_eq!(st(&core, "suggestion/last").as_deref(), Some("\"s-sender\""));
    assert_eq!(st(&core, "suggestion/s-sender/kind").as_deref(), Some("\"blockSender\""));
    assert_eq!(st(&core, "suggestion/s-sender/subject").as_deref(), Some("\"Promo@Spam.test\""));
    assert_eq!(st(&core, "suggestion/s-sender/accepted").as_deref(), Some("false"));
    assert_eq!(st(&core, "suggestion/s-sender/label_id"), None);
    assert_eq!(st(&core, "suggestion/s-sender/words"), Some(format!("{:?}", suggestion_words(SuggestionKind::BlockSender, "Promo@Spam.test".into()))));
    assert_eq!(st(&core, "block/promo@spam.test/blocked").as_deref(), Some("false"));
    // accept a block-sender suggestion: the block is written (the junk filter reads that table), the suggestion is done
    timed("AcceptSuggestion(block sender)", || core.run(Command::AcceptSuggestion { suggestion_id: "s-sender".into() })).expect("accepted");
    assert_eq!(st(&core, "block/promo@spam.test/blocked").as_deref(), Some("true"));
    assert_eq!(st(&core, "suggestion/count").as_deref(), Some("0"), "an accepted suggestion is no longer offered");
    assert_eq!(st(&core, "suggestion/s-sender/accepted").as_deref(), Some("true"));
    assert_eq!(core.blocks().len(), 1);
    assert_eq!(invalid(core.run(Command::AcceptSuggestion { suggestion_id: "s-sender".into() })).0, "suggestion", "a second accept is refused in words");
    assert_eq!(not_found(core.run(Command::AcceptSuggestion { suggestion_id: "ghost".into() })), ("suggestion".into(), "ghost".into()));
    println!("PASS suggestion-accept-writes-the-block");
    // a domain suggestion blocks the domain and its sub-domains
    s.suggestion_put(sug("s-domain", SuggestionKind::AlwaysJunkDomain, "@Evil.test", None)).unwrap();
    core.run(Command::AcceptSuggestion { suggestion_id: "s-domain".into() }).expect("accepted");
    assert_eq!(st(&core, "block/x@evil.test/blocked").as_deref(), Some("true"));
    assert_eq!(st(&core, "block/x@mail.evil.test/blocked").as_deref(), Some("true"));
    assert_eq!(st(&core, "block/x@notevil.test/blocked").as_deref(), Some("false"));
    // a mute suggestion mutes the thread
    s.suggestion_put(sug("s-mute", SuggestionKind::MuteThread, "t1", None)).unwrap();
    assert_eq!(st(&core, "message/m1/muted").as_deref(), Some("false"));
    core.run(Command::AcceptSuggestion { suggestion_id: "s-mute".into() }).expect("accepted");
    assert_eq!(st(&core, "message/m1/muted").as_deref(), Some("true"));
    assert_eq!(st(&core, "message/m2/muted").as_deref(), Some("true"), "the whole thread");
    // an always-label suggestion adds the sender to the label's rules
    let label = core.labels().into_iter().next().expect("a built-in label").id;
    s.suggestion_put(sug("s-label", SuggestionKind::AlwaysLabel, "boss@ext.test", Some(label.as_str()))).unwrap();
    assert_eq!(st(&core, "suggestion/s-label/label_id"), Some(format!("{label:?}")));
    core.run(Command::AcceptSuggestion { suggestion_id: "s-label".into() }).expect("accepted");
    assert!(s.label_rules(&label).unwrap().conditions.contains(&RuleCondition::From { words: "boss@ext.test".into() }));
    // an unsubscribe suggestion for a sender with no way out changes nothing and is still accepted
    s.suggestion_put(sug("s-unsub", SuggestionKind::Unsubscribe, "bob@ext.test", None)).unwrap();
    core.run(Command::AcceptSuggestion { suggestion_id: "s-unsub".into() }).expect("accepted");
    println!("PASS suggestion-effects-block-domain-mute-label-unsubscribe");
    // dismiss: gone, nothing written
    s.suggestion_put(sug("s-dismiss", SuggestionKind::BlockSender, "other@spam.test", None)).unwrap();
    let blocks_before = core.blocks().len();
    timed("DismissSuggestion", || core.run(Command::DismissSuggestion { suggestion_id: "s-dismiss".into() })).expect("dismissed");
    assert_eq!(st(&core, "suggestion/s-dismiss/kind"), None);
    assert_eq!(core.blocks().len(), blocks_before, "a dismissal wrote no block");
    assert_eq!(not_found(core.run(Command::DismissSuggestion { suggestion_id: "s-dismiss".into() })), ("suggestion".into(), "s-dismiss".into()), "a second dismiss");
    assert_eq!(not_found(core.run(Command::DismissSuggestion { suggestion_id: "s-sender".into() })).0, "suggestion", "an accepted one cannot be dismissed");
    println!("PASS suggestion-dismiss");
    // a restart keeps the blocks and the accepted marks
    let core = reopen(core, &dir);
    assert_eq!(st(&core, "block/promo@spam.test/blocked").as_deref(), Some("true"));
    assert_eq!(st(&core, "suggestion/s-sender/accepted").as_deref(), Some("true"));
    assert_eq!(st(&core, "suggestion/count").as_deref(), Some("0"));
    println!("PASS suggestions-survive-a-restart");
}

// ================================================================ contacts

#[test]
fn merge_contacts_keeps_every_address_and_every_mail_sender() {
    let (core, acc, dir) = big();
    wait_until("the people book", || st(&core, "people/ready").as_deref() == Some("true"));
    assert_eq!(st(&core, "contact/count").as_deref(), Some("0"));
    let before = core.contacts();
    let (ann, work) = (before.iter().find(|c| c.addresses == ["ann@ext.test"]).unwrap().clone(), before.iter().find(|c| c.addresses == ["ann.work@corp.test"]).unwrap().clone());
    assert_eq!((ann.id.as_str(), work.id.as_str()), ("person:ann@ext.test", "person:ann.work@corp.test"));
    // refusals first: nothing is written
    assert_eq!(invalid(core.run(Command::MergeContacts { keep_id: ann.id.clone(), merge_id: ann.id.clone() })), ("merge".into(), "a card cannot be merged into itself".into()));
    assert_eq!(not_found(core.run(Command::MergeContacts { keep_id: "person:nobody@ext.test".into(), merge_id: ann.id.clone() })), ("contact".into(), "person:nobody@ext.test".into()));
    assert_eq!(not_found(core.run(Command::MergeContacts { keep_id: ann.id.clone(), merge_id: "contact-9".into() })), ("contact".into(), "contact-9".into()));
    assert_eq!(st(&core, "contact/count").as_deref(), Some("0"), "no refusal wrote a card");
    // the merge
    timed("MergeContacts", || core.run(Command::MergeContacts { keep_id: ann.id.clone(), merge_id: work.id.clone() })).expect("merged");
    assert_eq!(st(&core, "contact/count").as_deref(), Some("1"));
    assert_eq!(st(&core, &format!("contact/{}/addresses", ann.id)).as_deref(), Some("[\"ann@ext.test\",\"ann.work@corp.test\"]"));
    let cards = core.store().contact_cards().unwrap();
    assert_eq!((cards[0].0.as_str(), cards[0].1.as_str()), ("person:ann@ext.test", "Ann"), "the kept card keeps its id and its name");
    // no mail's sender was lost: the messages are untouched, the card finds all of them by either address
    let count = |sql: &'static str| core.store().write(move |c| Ok(c.tx.query_row(sql, [], |r| r.get::<_, i64>(0))?)).unwrap();
    assert_eq!(count("SELECT count(*) FROM message WHERE from_address = 'ann@ext.test'"), 2);
    assert_eq!(count("SELECT count(*) FROM message WHERE from_address = 'ann.work@corp.test'"), 1);
    let ids = core.store().write(|c| Ok(c.contact_message_ids("person:ann@ext.test")?)).unwrap().expect("the card exists");
    let mut ids_sorted = ids.clone();
    ids_sorted.sort();
    assert_eq!(ids_sorted, vec!["m1", "m2", "m4"], "the card's mail is the mail of both addresses");
    let counts = core.store().write(|c| Ok(c.contact_get("person:ann@ext.test")?)).unwrap().unwrap();
    assert_eq!(counts.addresses.len(), 2);
    let stored_counts: String = core.store().write(|c| Ok(c.tx.query_row("SELECT mail_count_json FROM contact WHERE id = 'person:ann@ext.test'", [], |r| r.get::<_, String>(0))?)).unwrap();
    assert_eq!(stored_counts, format!("{{\"{acc}\":3}}"), "the per-account count is recounted over both addresses");
    // merging again (the same card by an address it already holds) is refused
    assert_eq!(invalid(core.run(Command::MergeContacts { keep_id: ann.id.clone(), merge_id: work.id.clone() })).0, "merge");
    println!("PASS merge-keeps-addresses-and-senders");
    // a third person joins the card by the card's id; the card's name stays
    core.run(Command::MergeContacts { keep_id: "person:ann@ext.test".into(), merge_id: "person:bob@ext.test".into() }).expect("merged");
    assert_eq!(st(&core, "contact/count").as_deref(), Some("1"), "still one card");
    assert_eq!(core.store().contact_cards().unwrap()[0].2.len(), 3);
    // restart
    let core = reopen(core, &dir);
    assert_eq!(st(&core, "contact/count").as_deref(), Some("1"));
    assert_eq!(core.store().contact_cards().unwrap()[0].2, vec!["ann@ext.test", "ann.work@corp.test", "bob@ext.test"]);
    println!("PASS merge-survives-a-restart");
}

#[test]
fn the_people_read_shows_a_merged_person_once() {
    let (core, _acc, _dir) = seeded();
    wait_until("the people book", || st(&core, "people/ready").as_deref() == Some("true"));
    let n = core.contacts().len();
    assert!(core.contacts().iter().filter(|c| c.name.starts_with("Ann")).count() == 2);
    core.run(Command::MergeContacts { keep_id: "person:ann@ext.test".into(), merge_id: "person:ann.work@corp.test".into() }).expect("merged");
    let after = core.contacts();
    assert_eq!(after.len(), n - 1, "two persons are one now");
    let anns: Vec<&Contact> = after.iter().filter(|c| c.addresses.iter().any(|a| a == "ann@ext.test" || a == "ann.work@corp.test")).collect();
    assert_eq!(anns.len(), 1, "the merged person is listed once: {anns:?}");
    assert_eq!((anns[0].id.as_str(), anns[0].name.as_str(), anns[0].addresses.clone()), ("person:ann@ext.test", "Ann", vec!["ann@ext.test".to_string(), "ann.work@corp.test".to_string()]));
    assert_eq!(anns[0].mail_count_per_account.values().sum::<u32>(), 3, "the mail of both addresses");
    // the one-address read finds the same person by either address
    assert_eq!(core.contact("ann.work@corp.test".into()).unwrap().id, "person:ann@ext.test");
    assert_eq!(core.contact("ann@ext.test".into()).unwrap().addresses.len(), 2);
    assert_eq!(core.contact("bob@ext.test".into()).unwrap().id, "person:bob@ext.test", "an unmerged person is as before");
    assert_eq!(st(&core, "contacts/count"), Some((n - 1).to_string()));
    println!("PASS the-people-read-shows-a-merged-person-once");
}

// ================================================================ blocks

#[test]
fn block_domain_is_the_senders_table_in_the_domain_form() {
    let (core, _acc, dir) = big();
    assert_eq!(st(&core, "block/count").as_deref(), Some("0"));
    // the junk reading of a stored mail: m5 is from promo@spam.test
    assert!(st(&core, "message/m5/junk").unwrap().contains("\"blocked\":false"));
    timed("BlockDomain", || core.run(Command::BlockDomain { domain: " Spam.TEST ".into() })).expect("blocked");
    assert_eq!(core.blocks().iter().map(|b| b.target.clone()).collect::<Vec<_>>(), vec![BlockTarget::Domain { domain: "spam.test".into() }]);
    assert_eq!(st(&core, "block/count").as_deref(), Some("1"));
    assert_eq!(st(&core, "block/list").as_deref(), Some("[\"domain:spam.test\"]"));
    assert_eq!(st(&core, "block/someone@spam.test/blocked").as_deref(), Some("true"));
    assert_eq!(st(&core, "block/someone@news.spam.test/blocked").as_deref(), Some("true"), "a sub-domain too");
    assert_eq!(st(&core, "block/someone@spam.test.org/blocked").as_deref(), Some("false"));
    assert_eq!(st(&core, "block/someone@other.test/blocked").as_deref(), Some("false"));
    // the junk filter reads the same list for a sender block and a domain block
    assert!(st(&core, "message/m5/junk").unwrap().contains("\"blocked\":true"), "{:?}", st(&core, "message/m5/junk"));
    assert!(st(&core, "message/m3/junk").unwrap().contains("\"blocked\":false"), "another domain is not blocked");
    // idempotent: a second block, in another spelling, changes nothing (the first time stays)
    let since = core.blocks()[0].since_ms;
    std::thread::sleep(Duration::from_millis(5));
    core.run(Command::BlockDomain { domain: "@spam.test".into() }).expect("blocked again");
    assert_eq!(core.blocks().len(), 1);
    assert_eq!(core.blocks()[0].since_ms, since);
    // the same table as a sender block
    core.store().block_add(BlockTarget::Sender { address: "bob@ext.test".into() }, now()).unwrap();
    assert_eq!(st(&core, "block/count").as_deref(), Some("2"));
    assert!(st(&core, "message/m3/junk").unwrap().contains("\"blocked\":true"));
    // refusals write nothing
    for bad in ["", "   ", "nodot", "a@b.test", "has space.test", "@"] {
        assert_eq!(invalid(core.run(Command::BlockDomain { domain: bad.into() })), ("domain".into(), "a domain looks like example.com".into()), "{bad:?}");
    }
    assert_eq!(st(&core, "block/count").as_deref(), Some("2"));
    println!("PASS block-domain-is-the-senders-table");
    let core = reopen(core, &dir);
    assert_eq!(st(&core, "block/someone@spam.test/blocked").as_deref(), Some("true"));
    assert_eq!(st(&core, "block/count").as_deref(), Some("2"));
    println!("PASS block-domain-survives-a-restart");
}
