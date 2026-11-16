//! real-accounts: the account commands of Settings > Accounts on the REAL core (they answered "The local engine does not do this yet."):
//! RenameAccount, PauseAccount, RemoveAccount, SetAccountIcon, SetAccountColour, ReorderAccounts, and the AddAccount refusal; the account state
//! paths; the default mark of a new account. docs/lanes/real-accounts.md holds the table (command, arm, store function, state path, test, broken copy).
//!
//! Every write is read back (accounts(), the state path), survives a restart (the store is reopened by a new core), and answers under 50 ms on a
//! store of 2,000 mails (each test prints `TIME <what>: <ms> ms`, and `PASS <case>` when it passes; run with `-- --nocapture`, release profile).
//! The mail servers of the live test are on 127.0.0.1 only; the secrets are the in-memory keychain's (a test never reads or prints one).
//!
//! Broken copies (one edit each, each red in the named test):
//! - R1 account_cmds.rs `Command::RenameAccount`: delete the `core.store.rename_account(...)` line: `rename_writes_reads_back_and_survives_a_restart`
//! - R2 account_cmds.rs `colour_of`: return `Ok(raw.to_string())` (no check): `colour_is_checked_stored_upper_case_and_survives_a_restart`
//! - R3 realcore.rs `set_account_library_icon`: delete the `DELETE FROM account_icon_file` line: `icon_library_and_upload_read_back_and_the_limits_refuse`
//! - R4 realcore.rs `reorder_accounts`: delete the `want != have` test: `reorder_needs_every_account_once_and_survives_a_restart`
//! - R5 account_cmds.rs `Command::AddAccount`: return `nothing()` (served as a no-op): `add_account_is_refused_and_names_the_command_to_use`
//! - R6 realcore.rs `remove_account`: delete the `DELETE FROM outbox_entry` line: `remove_takes_this_accounts_mail_queue_labels_and_secret_and_no_other`
//! - R7 account_cmds.rs `remove_account`: delete the `DeleteSecret::execute` line: the same test (the secret is still readable)
//! - R8 account_cmds.rs `remove_account`: delete the `SendNow`-in-flight `if ... Sending` refusal: `remove_is_refused_while_a_send_of_the_account_is_in_flight`
//! - R9 rust_core.rs `start_stored_account`: delete `if a.account.paused { return; }`: `a_paused_account_does_not_start_after_a_restart`
//! - R10 outbox_tick.rs `StoreSendAccounts::execute`: delete the `if a.account.paused` line: `a_paused_account_holds_its_sync_and_its_outbox_and_resume_sends_what_was_held`
//! - R11 account_cmds.rs `stop_account_sync`: delete the `stops...remove` line: the same live test (new mail still arrives while paused)
//! - R12 account_cmds.rs `state`: delete the `["account_credential", id, "stored"]` arm: `the_account_state_paths_are_served`
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
use contracts::constants::ACCOUNT_ICON_MAX_BYTES;
use detect_fakes::{FakeDns, FakeHttp, FakeProbe};
use engine::{RustCore, SyncPhase, TestTransport};
use imap_fakes::FakeImap;
use secrets::{DeleteSecret, MemoryKeychain, Namespace, ReadSecret, Secret, SecretKind, StoreSecret};
use store::{NewMessage, Store};

const BUDGET_MS: f64 = 50.0;
const WAIT: Duration = Duration::from_secs(25);
const PW: &str = "app-password";

fn detector() -> Box<dyn engine::Detect> { Box::new(autoconfig::Detector::new(FakeHttp::default(), FakeDns::default(), FakeProbe::default())) }
fn wait_until(what: &str, mut ok: impl FnMut() -> bool) {
    let t0 = Instant::now();
    while t0.elapsed() < WAIT { if ok() { return; } std::thread::sleep(Duration::from_millis(40)); }
    panic!("never reached within {WAIT:?}: {what}");
}
fn config(imap_port: u16, smtp_port: u16, user: &str) -> AccountConfig {
    AccountConfig {
        imap_host: "127.0.0.1".into(), imap_port, imap_security: ConnectionSecurity::Ssl, smtp_host: "127.0.0.1".into(), smtp_port,
        smtp_security: ConnectionSecurity::Starttls, username: user.into(), smtp_username: user.into(), auth_method: AuthMethod::Password, source: DetectionSource::Manual,
    }
}

/// Runs `f`, prints its time, and fails when it took 50 ms or more (docs/CONSTANTS.md INTERACTION_COMMIT_MS).
fn timed<R>(what: &str, f: impl FnOnce() -> R) -> R {
    let t = Instant::now();
    let r = f();
    let ms = t.elapsed().as_secs_f64() * 1000.0;
    println!("TIME {what}: {ms:.1} ms");
    assert!(ms < BUDGET_MS, "{what} took {ms:.1} ms, over {BUDGET_MS} ms");
    r
}

fn mail(account: &str, id: String, at: i64) -> NewMessage {
    NewMessage {
        message: Message {
            id: id.clone(), thread_id: format!("{id}:thread"), account_id: account.into(), sender_name: "Sender".into(), from_address: "sender@example.test".into(), to: vec!["owner@example.test".into()],
            subject: format!("Subject {id}"), preview: String::new(), received_at_ms: at, read: false, archived: false, folder: "INBOX".into(),
            attachment_names: vec![], login_code: None, trackers_blocked: 0, list_unsubscribe: vec![], list_unsubscribe_one_click: false,
            reply_to_name: None, cc: vec![], bcc: vec![], is_outgoing: false,
        },
        remote_id: Some(at.to_string()), body_text: None, body_html: None, has_list_header: false, size_bytes: None, tab_id: None,
    }
}

/// Three accounts on one store: A holds 2,000 mails, B and C 40 each. Each account's password is in the in-memory keychain.
struct World { core: RustCore, kc: Arc<MemoryKeychain>, ids: [String; 3], refs: [KeychainRef; 3], dir: tempfile::TempDir }
const ADDR: [&str; 3] = ["alpha@example.test", "beta@example.test", "gamma@example.test"];

fn world() -> World {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(&dir.path().join("whoosh.sqlite")).unwrap();
    let kc = Arc::new(MemoryKeychain::new(Namespace::rig()));
    let mut ids = vec![];
    let mut refs = vec![];
    for (i, addr) in ADDR.iter().enumerate() {
        let r = StoreSecret::execute(&*kc, SecretKind::AccountCredential, addr, &Secret::new(PW.as_bytes().to_vec())).unwrap();
        let kref = format!("{}\u{1f}{}", r.service, r.account);
        ids.push(store.add_account(addr, &format!("Name {i}"), MailProvider::Imap, &kref, false, config(1, 1, addr), 10 + i as i64).unwrap());
        refs.push(r);
    }
    let mut batch = Vec::new();
    for i in 0..2_000 { batch.push(mail(&ids[0], format!("a{i}"), 1_000 + i as i64)); }
    for i in 0..40 { batch.push(mail(&ids[1], format!("b{i}"), 1_000 + i as i64)); batch.push(mail(&ids[2], format!("c{i}"), 1_000 + i as i64)); }
    store.insert_messages(batch).unwrap();
    let core = RustCore::with_parts(store, kc.clone(), detector(), TestTransport::PlainLoopback);
    World { core, kc, ids: [ids[0].clone(), ids[1].clone(), ids[2].clone()], refs: [refs[0].clone(), refs[1].clone(), refs[2].clone()], dir }
}

/// A new core over the same store file: what a restart of the app reads.
fn restart(w: World) -> World {
    let path = w.dir.path().join("whoosh.sqlite");
    let World { core, kc, ids, refs, dir } = w;
    drop(core);
    let store = Store::open(&path).unwrap();
    World { core: RustCore::with_parts(store, kc.clone(), detector(), TestTransport::PlainLoopback), kc, ids, refs, dir }
}

fn count(core: &RustCore, sql: &str, arg: &str) -> i64 {
    let (sql, arg) = (sql.to_string(), arg.to_string());
    core.store().write(move |c| Ok(c.tx.query_row(&sql, [&arg], |r| r.get::<_, i64>(0))?)).unwrap()
}
fn account(core: &RustCore, id: &str) -> AccountSummary { core.accounts().into_iter().find(|a| a.id == id).unwrap_or_else(|| panic!("no account {id} in the list")) }
fn run(core: &RustCore, c: Command) -> Result<CommandOutcome, CoreError> { core.run(c) }
fn invalid(r: Result<CommandOutcome, CoreError>, field: &str) -> String {
    match r { Err(CoreError::Invalid { field: f, reason_words }) if f == field => reason_words, other => panic!("wanted Invalid({field}), got {other:?}") }
}
fn not_found(r: Result<CommandOutcome, CoreError>) {
    assert!(matches!(r, Err(CoreError::NotFound { ref kind, .. }) if kind == "account"), "wanted NotFound(account), got {r:?}");
}

// ---------------------------------------------------------------- rename

#[test]
fn rename_writes_reads_back_and_survives_a_restart() {
    let w = world();
    let before = account(&w.core, &w.ids[0]);
    timed("RenameAccount", || run(&w.core, Command::RenameAccount { account_id: w.ids[0].clone(), name: "  Work  ".into() })).expect("renamed");
    let a = account(&w.core, &w.ids[0]);
    assert_eq!(a.name, "Work", "trimmed and read back through accounts()");
    assert_eq!(a.address, before.address, "the address never changes");
    assert_eq!(w.core.state(format!("account/{}/name", w.ids[0])).as_deref(), Some("\"Work\""), "and through the state path");
    assert_eq!(account(&w.core, &w.ids[1]).name, "Name 1", "no other account is renamed");
    assert_eq!(invalid(run(&w.core, Command::RenameAccount { account_id: w.ids[0].clone(), name: "   ".into() }), "name"), "an account needs a name");
    not_found(run(&w.core, Command::RenameAccount { account_id: "no-such".into(), name: "x".into() }));
    assert_eq!(account(&w.core, &w.ids[0]).name, "Work", "a refused rename changes nothing");
    let w = restart(w);
    assert_eq!(account(&w.core, &w.ids[0]).name, "Work", "kept after a restart");
    println!("PASS rename-writes-reads-back-trims-refuses-empty-and-unknown-and-survives-a-restart");
}

// ---------------------------------------------------------------- colour

#[test]
fn colour_is_checked_stored_upper_case_and_survives_a_restart() {
    let w = world();
    timed("SetAccountColour", || run(&w.core, Command::SetAccountColour { account_id: w.ids[0].clone(), colour: " #2b8a3e ".into() })).expect("set");
    assert_eq!(account(&w.core, &w.ids[0]).colour, "#2B8A3E");
    assert_eq!(w.core.state(format!("account/{}/colour", w.ids[0])).as_deref(), Some("\"#2B8A3E\""));
    for bad in ["red", "#12345", "#GGGGGG", "2B8A3E", "", "#1234567"] {
        let words = invalid(run(&w.core, Command::SetAccountColour { account_id: w.ids[0].clone(), colour: bad.into() }), "colour");
        assert!(words.contains("#RRGGBB"), "{bad:?}: {words}");
    }
    not_found(run(&w.core, Command::SetAccountColour { account_id: "no-such".into(), colour: "#112233".into() }));
    assert_eq!(account(&w.core, &w.ids[0]).colour, "#2B8A3E", "a refused colour changes nothing");
    assert_ne!(account(&w.core, &w.ids[1]).colour, "#2B8A3E", "no other account changes");
    let w = restart(w);
    assert_eq!(account(&w.core, &w.ids[0]).colour, "#2B8A3E", "kept after a restart");
    println!("PASS colour-checked-upper-case-read-back-refuses-six-bad-ones-and-survives-a-restart");
}

#[test]
fn a_new_account_has_the_first_free_palette_colour_and_no_invented_icon() {
    let w = world();
    let colours: Vec<String> = w.core.accounts().into_iter().map(|a| a.colour).collect();
    assert_eq!(colours, ["#3B82F6", "#10B981", "#F59E0B"], "the palette in turn");
    for a in w.core.accounts() { assert_eq!(a.icon_id, None, "{} has an icon the owner never picked", a.address); assert_eq!(a.icon_file, None); }
    // the account that wore the first colour is removed: the next new account takes the freed colour, and sits after the last in the order
    run(&w.core, Command::RemoveAccount { account_id: w.ids[0].clone() }).expect("removed");
    let r = StoreSecret::execute(&*w.kc, SecretKind::AccountCredential, "delta@example.test", &Secret::new(PW.as_bytes().to_vec())).unwrap();
    let id = w.core.store().add_account("delta@example.test", "Delta", MailProvider::Imap, &format!("{}\u{1f}{}", r.service, r.account), false, config(1, 1, "delta@example.test"), 99).unwrap();
    let list = w.core.accounts();
    assert_eq!(list.last().map(|a| a.id.clone()), Some(id.clone()), "the new account is last");
    assert_eq!(list.last().map(|a| a.colour.clone()).as_deref(), Some("#3B82F6"), "it takes the colour the removed account freed");
    println!("PASS new-account-first-free-palette-colour-no-invented-icon-and-last-in-order");
}

// ---------------------------------------------------------------- icon

#[test]
fn icon_library_and_upload_read_back_and_the_limits_refuse() {
    let w = world();
    let a = w.ids[0].clone();
    assert_eq!(w.core.state(format!("account_icon/{a}")), None, "no icon until the owner picks one");
    timed("SetAccountIcon library", || run(&w.core, Command::SetAccountIcon { account_id: a.clone(), source: AccountIconSource::Library { icon_id: "briefcase".into() } })).expect("library pick");
    assert_eq!(account(&w.core, &a).icon_id.as_deref(), Some("briefcase"));
    assert_eq!(w.core.state(format!("account_icon/{a}")).as_deref(), Some("\"briefcase\""));
    invalid(run(&w.core, Command::SetAccountIcon { account_id: a.clone(), source: AccountIconSource::Library { icon_id: "  ".into() } }), "icon");
    not_found(run(&w.core, Command::SetAccountIcon { account_id: "no-such".into(), source: AccountIconSource::Library { icon_id: "star".into() } }));

    let files = tempfile::tempdir().unwrap();
    let png = files.path().join("mark.png");
    std::fs::write(&png, [0x89u8, b'P', b'N', b'G', 1, 2, 3, 4]).unwrap();
    timed("SetAccountIcon upload", || run(&w.core, Command::SetAccountIcon { account_id: a.clone(), source: AccountIconSource::Uploaded { file_path: png.to_string_lossy().into() } })).expect("upload");
    let s = account(&w.core, &a);
    assert_eq!(s.icon_file.as_deref(), Some(png.to_string_lossy().as_ref()), "the image the app holds is the mark");
    assert_eq!(s.icon_id.as_deref(), Some("briefcase"), "an upload never clears the glyph");
    assert_eq!(w.core.state(format!("account_icon/{a}")).as_deref(), Some("\"uploaded\""));

    let big = files.path().join("big.png");
    std::fs::write(&big, vec![0u8; ACCOUNT_ICON_MAX_BYTES as usize + 1]).unwrap();
    let words = invalid(run(&w.core, Command::SetAccountIcon { account_id: a.clone(), source: AccountIconSource::Uploaded { file_path: big.to_string_lossy().into() } }), "icon");
    assert!(words.contains("too big") && words.contains("1 MB"), "{words}");
    let exact = files.path().join("exact.png");
    std::fs::write(&exact, vec![1u8; ACCOUNT_ICON_MAX_BYTES as usize]).unwrap();
    run(&w.core, Command::SetAccountIcon { account_id: a.clone(), source: AccountIconSource::Uploaded { file_path: exact.to_string_lossy().into() } }).expect("exactly the limit is accepted");
    let words = invalid(run(&w.core, Command::SetAccountIcon { account_id: a.clone(), source: AccountIconSource::Uploaded { file_path: files.path().join("missing.png").to_string_lossy().into() } }), "icon");
    assert_eq!(words, "that image could not be read");
    assert_eq!(account(&w.core, &a).icon_file.as_deref(), Some(exact.to_string_lossy().as_ref()), "a refused upload leaves the mark as it was");

    let w = restart(w);
    assert_eq!(account(&w.core, &a).icon_file.as_deref(), Some(exact.to_string_lossy().as_ref()), "the upload is kept after a restart");
    run(&w.core, Command::SetAccountIcon { account_id: a.clone(), source: AccountIconSource::Library { icon_id: "star".into() } }).expect("back to the library");
    let s = account(&w.core, &a);
    assert_eq!((s.icon_id.as_deref(), s.icon_file.as_deref()), (Some("star"), None), "a library pick sets the upload back to none");
    assert_eq!(w.core.state(format!("account_icon/{a}")).as_deref(), Some("\"star\""));
    println!("PASS icon-library-and-upload-read-back-limit-refused-exact-limit-accepted-library-pick-clears-upload-survives-a-restart");
}

// ---------------------------------------------------------------- order

#[test]
fn reorder_needs_every_account_once_and_survives_a_restart() {
    let w = world();
    let [a, b, c] = w.ids.clone();
    timed("ReorderAccounts", || run(&w.core, Command::ReorderAccounts { ids: vec![c.clone(), a.clone(), b.clone()] })).expect("reordered");
    let order = |core: &RustCore| core.accounts().into_iter().map(|x| x.id).collect::<Vec<_>>();
    assert_eq!(order(&w.core), vec![c.clone(), a.clone(), b.clone()]);
    assert_eq!(w.core.state("account/order".into()), Some(format!("[\"{c}\",\"{a}\",\"{b}\"]")));
    for bad in [vec![a.clone(), b.clone()], vec![a.clone(), a.clone(), b.clone()], vec![a.clone(), b.clone(), "no-such".into()], vec![a.clone(), b.clone(), c.clone(), c.clone()], vec![]] {
        let words = invalid(run(&w.core, Command::ReorderAccounts { ids: bad.clone() }), "ids");
        assert_eq!(words, "the order must list every account once", "{bad:?}");
    }
    assert_eq!(order(&w.core), vec![c.clone(), a.clone(), b.clone()], "a refused order changes nothing");
    let w = restart(w);
    assert_eq!(order(&w.core), vec![c, a, b], "kept after a restart");
    println!("PASS reorder-read-back-five-bad-lists-refused-and-survives-a-restart");
}

// ---------------------------------------------------------------- AddAccount

#[test]
fn add_account_is_refused_and_names_the_command_to_use() {
    let w = world();
    let r = run(&w.core, Command::AddAccount { address: "new@example.test".into(), provider: MailProvider::Imap });
    let Err(CoreError::Action { failure }) = r else { panic!("AddAccount must be refused in words, got {r:?}") };
    assert_eq!(failure.code.as_deref(), Some("account.use_configured"));
    assert!(failure.reason_words.contains("AddAccountConfigured"), "{}", failure.reason_words);
    assert!(!failure.reason_words.contains("does not do this yet"), "a purpose-made sentence, not the generic one");
    assert_eq!(w.core.accounts().len(), 3, "no account was added: there is no second add path");
    println!("PASS add-account-refused-with-its-own-code-and-names-AddAccountConfigured");
}

/// Queues a NEW mail of the account (a saved draft, then Send) and returns its Outbox entry id. Not a reply: a reply is queued with the
/// threading headers of the message it answers, read from the server, and these accounts are not connected (outbox resolve_reply.rs refuses
/// it in words, which is right: an unthreaded reply must not be sent). A queued send is all these tests need.
fn queue_new_mail(core: &RustCore, account: &str) -> String {
    let draft = core.run(Command::SaveDraft { from_account_id: Some(account.into()), to: vec!["friend@example.test".into()], cc: vec![], subject: "Queued".into(), body: "thanks".into(), reply_to_thread_id: None }).expect("queued draft").created_id.expect("the draft id");
    core.run(Command::Send { draft_id: draft }).expect("queued").created_id.expect("the outbox entry id")
}

// ---------------------------------------------------------------- remove

#[test]
fn remove_takes_this_accounts_mail_queue_labels_and_secret_and_no_other() {
    let w = world();
    let [a, b, _c] = w.ids.clone();
    // what waits: a draft and a queued send of each of A and B, a label of each, and a pending server operation of each
    for acc in [&a, &b] {
        w.core.run(Command::SaveDraft { from_account_id: Some(acc.clone()), to: vec!["x@example.test".into()], cc: vec![], subject: "Hi".into(), body: "words".into(), reply_to_thread_id: None }).expect("draft");
        queue_new_mail(&w.core, acc);
        let acc2 = acc.clone();
        w.core.store().write(move |c| {
            c.tx.execute("INSERT INTO label (id, name, colour, account_ids_json, updated_at_ms, scope_account_id) VALUES (?1, ?2, '#112233', '[]', 1, ?3)", rusqlite::params![format!("lab-{acc2}"), format!("Only {acc2}"), acc2])?;
            Ok(())
        }).unwrap();
        w.core.run(Command::MarkRead { message_ids: vec![if *acc == a { "a7".into() } else { "b7".into() }], read: true }).expect("read");
    }
    let rows = |core: &RustCore, acc: &str| (
        count(core, "SELECT count(*) FROM message WHERE account_id = ?1", acc), count(core, "SELECT count(*) FROM thread WHERE account_id = ?1", acc),
        count(core, "SELECT count(*) FROM draft WHERE from_account_id = ?1", acc),
        count(core, "SELECT count(*) FROM outbox_entry WHERE json_extract(draft_json, '$.draft.from_account_id') = ?1", acc),
        count(core, "SELECT count(*) FROM label WHERE scope_account_id = ?1", acc), count(core, "SELECT count(*) FROM pending_action WHERE account_id = ?1", acc));
    let (a_before, b_before) = (rows(&w.core, &a), rows(&w.core, &b));
    assert_eq!(a_before, (2_000, 2_000, 1, 1, 1, a_before.5), "A holds its mail, draft, queued send and label");
    assert!(a_before.5 >= 1 && b_before.5 >= 1, "each account has a pending server operation (A {} B {})", a_before.5, b_before.5);
    assert_eq!(b_before.0, 40);

    let removal = w.core.account_removal(a.clone()).expect("the removal is described before it is done");
    assert_eq!((removal.messages, removal.drafts, removal.outbox, removal.labels), (2_000, 1, 1, 1));
    assert!(removal.words.contains("2000 messages") && removal.words.contains("its sign-in") && removal.words.contains("Nothing is deleted on the server"), "{}", removal.words);
    assert!(w.core.account_removal("no-such".into()).is_none());
    assert!(ReadSecret::execute(&*w.kc, &w.refs[0]).is_ok(), "A's password is in the keychain before");

    timed("RemoveAccount (2,000 mails)", || run(&w.core, Command::RemoveAccount { account_id: a.clone() })).expect("removed");

    assert_eq!(rows(&w.core, &a), (0, 0, 0, 0, 0, 0), "nothing of A is left in the store");
    assert_eq!(rows(&w.core, &b), b_before, "nothing of B changed");
    assert_eq!(count(&w.core, "SELECT count(*) FROM message WHERE ?1 = ''", ""), 80, "B and C keep their 80 mails");
    assert_eq!(w.core.accounts().iter().map(|x| x.id.clone()).collect::<Vec<_>>(), vec![w.ids[1].clone(), w.ids[2].clone()]);
    assert!(matches!(ReadSecret::execute(&*w.kc, &w.refs[0]), Err(CoreError::NotFound { .. })), "A's secret is gone from the keychain");
    assert!(ReadSecret::execute(&*w.kc, &w.refs[1]).is_ok() && ReadSecret::execute(&*w.kc, &w.refs[2]).is_ok(), "the other secrets stay");
    assert_eq!(w.core.state(format!("account/{a}/address")), None, "the state paths forget it");
    assert_eq!(w.core.state(format!("account_credential/{a}/stored")), None);
    assert_eq!(w.core.state("account/count".into()).as_deref(), Some("2"));
    not_found(run(&w.core, Command::RemoveAccount { account_id: a.clone() }));
    let w = restart(w);
    assert_eq!(w.core.accounts().len(), 2, "still removed after a restart");
    assert_eq!(count(&w.core, "SELECT count(*) FROM message WHERE account_id = ?1", &a), 0);
    println!("PASS remove-takes-mail-drafts-queue-labels-pending-and-secret-of-this-account-only-and-survives-a-restart");
}

#[test]
fn remove_is_refused_while_a_send_of_the_account_is_in_flight() {
    let w = world();
    let [a, b, _c] = w.ids.clone();
    let entry = queue_new_mail(&w.core, &a);
    // the entry is mid-send (the outbox's own state word); a send of ANOTHER account does not stop the removal of B
    let e = entry.clone();
    w.core.store().write(move |c| { c.tx.execute("UPDATE outbox_entry SET state = 'sending' WHERE id = ?1", [&e])?; Ok(()) }).unwrap();
    assert!(w.core.outbox().iter().any(|o| o.id == entry && matches!(o.state, OutboxState::Sending)), "the entry reads as sending");
    let r = run(&w.core, Command::RemoveAccount { account_id: a.clone() });
    let Err(CoreError::Action { failure }) = r else { panic!("must be refused while sending, got {r:?}") };
    assert_eq!(failure.code.as_deref(), Some("account.send_in_flight"));
    assert!(failure.reason_words.contains("sending a message right now"), "{}", failure.reason_words);
    assert_eq!(count(&w.core, "SELECT count(*) FROM message WHERE account_id = ?1", &a), 2_000, "nothing was removed");
    assert!(ReadSecret::execute(&*w.kc, &w.refs[0]).is_ok(), "its secret is still there");
    run(&w.core, Command::RemoveAccount { account_id: b.clone() }).expect("another account is removed while A sends");
    println!("PASS remove-refused-in-words-while-a-send-of-the-account-is-in-flight-and-another-account-is-not-held");
}

// ---------------------------------------------------------------- state paths

#[test]
fn the_account_state_paths_are_served() {
    let w = world();
    let [a, b, _c] = w.ids.clone();
    let get = |p: String| w.core.state(p);
    assert_eq!(get("account/count".into()).as_deref(), Some("3"));
    assert_eq!(get("account/order".into()), Some(format!("[\"{a}\",\"{b}\",\"{}\"]", w.ids[2])));
    assert_eq!(get(format!("account/{a}/address")).as_deref(), Some("\"alpha@example.test\""));
    assert_eq!(get(format!("account/{a}/provider")).as_deref(), Some("\"imap\""));
    assert_eq!(get(format!("account/{a}/colour")).as_deref(), Some("\"#3B82F6\""));
    assert_eq!(get(format!("account/{a}/paused")).as_deref(), Some("false"));
    for (field, want) in [("imap_host", "\"127.0.0.1\""), ("imap_port", "1"), ("imap_security", "\"ssl\""), ("smtp_host", "\"127.0.0.1\""), ("smtp_port", "1"),
        ("smtp_security", "\"starttls\""), ("username", "\"alpha@example.test\""), ("smtp_username", "\"alpha@example.test\""), ("auth_method", "\"password\""), ("detection_source", "\"manual\"")] {
        assert_eq!(get(format!("account/{a}/{field}")).as_deref(), Some(want), "account/<id>/{field}");
    }
    assert_eq!(get(format!("account_credential/{a}/stored")).as_deref(), Some("true"), "the password is in the keychain");
    assert_eq!(get(format!("account/{a}/password_stored")).as_deref(), Some("true"));
    assert_eq!(get(format!("account/{a}/password")), None, "the password itself is never a state");
    assert_eq!(get(format!("account/{a}/no_such_field")), None);
    assert_eq!(get("account/no-such/address".into()), None);
    assert_eq!(get("account_credential/no-such/stored".into()), None);
    assert_eq!(get(format!("account_icon/{a}")), None, "no icon picked yet");
    // an account that never synced reads as nothing fetched and no error (`sync_state_rows`: its LEFT JOIN gives it a row; the test engine's
    // `state(path:)` answers the same for every account it lists, FixtureCore.swift "sync_state")
    assert_eq!(get(format!("sync_state/{a}/healthy")).as_deref(), Some("true"), "an account that never synced has no error");
    assert_eq!(get(format!("sync_state/{a}/history_fetched")).as_deref(), Some("false"));
    assert_eq!(get(format!("sync_state/{a}/messages_indexed")).as_deref(), Some("0"));
    assert_eq!(get(format!("sync_state/{a}/last_synced_ms")), None, "it never synced");
    assert_eq!(get(format!("sync_state/{a}/error")), None);
    assert_eq!(get(format!("sync_state/no-such/healthy")), None, "an unknown account has no sync state");
    // a credential that left the keychain reads as not stored
    DeleteSecret::execute(&*w.kc, &w.refs[1]).unwrap();
    assert_eq!(get(format!("account_credential/{b}/stored")).as_deref(), Some("false"));
    assert_eq!(get(format!("account_credential/{a}/stored")).as_deref(), Some("true"), "another account's credential is unaffected");
    println!("PASS account-state-paths-count-order-fields-config-credential-icon-and-the-password-never-served");
}

// ---------------------------------------------------------------- pause

#[test]
fn a_paused_account_does_not_start_after_a_restart() {
    let w = world();
    let [a, b, _c] = w.ids.clone();
    timed("PauseAccount", || run(&w.core, Command::PauseAccount { account_id: a.clone(), paused: true })).expect("paused");
    let s = account(&w.core, &a);
    assert!(s.paused, "the account says it is paused in the list the app reads");
    assert_eq!(w.core.state(format!("account/{a}/paused")).as_deref(), Some("true"));
    assert!(w.core.sync_states().iter().any(|x| x.account_id == a && x.provider_status_words.as_deref() == Some("Paused. Nothing is fetched or sent until you resume.")), "and on its sync status");
    assert!(!account(&w.core, &b).paused, "no other account is paused");
    assert_eq!(count(&w.core, "SELECT count(*) FROM message WHERE account_id = ?1", &a), 2_000, "pausing deletes nothing");
    not_found(run(&w.core, Command::PauseAccount { account_id: "no-such".into(), paused: true }));
    let w = restart(w);
    assert!(account(&w.core, &a).paused, "the pause is kept after a restart");
    w.core.start_stored_accounts();
    assert_eq!(w.core.sync_phase(&a), None, "a paused account is not started at launch");
    wait_until("the other account's sync starts", || w.core.sync_phase(&b).is_some());
    timed("PauseAccount resume", || run(&w.core, Command::PauseAccount { account_id: a.clone(), paused: false })).expect("resumed");
    assert!(!account(&w.core, &a).paused);
    wait_until("the resumed account's sync starts", || w.core.sync_phase(&a).is_some());
    println!("PASS paused-flag-read-back-status-words-nothing-deleted-not-started-at-launch-resume-starts-it");
}

/// An SMTP server of this file that accepts messages and keeps them (127.0.0.1 only).
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

#[test]
fn a_paused_account_holds_its_sync_and_its_outbox_and_resume_sends_what_was_held() {
    let user = "owner@example.test";
    let rt = tokio::runtime::Runtime::new().unwrap();
    let imap = rt.block_on(FakeImap::start());
    imap.seed("INBOX", 2);
    let smtp = smtp();
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(&dir.path().join("whoosh.sqlite")).unwrap();
    let core = RustCore::with_parts(store, Arc::new(MemoryKeychain::new(Namespace::rig())), detector(), TestTransport::PlainLoopback);
    let id = core.run(Command::AddAccountConfigured { address: user.into(), display_name: Some("Owner".into()), config: config(imap.port, smtp.port, user), password: SecretText { value: PW.into() } }).expect("added").created_id.unwrap();
    wait_until("listening", || core.sync_phase(&id) == Some(SyncPhase::Listening));
    let stored = |core: &RustCore| count(core, "SELECT count(*) FROM message WHERE account_id = ?1 AND folder = 'INBOX'", &id);
    assert_eq!(stored(&core), 2);

    timed("PauseAccount (live)", || core.run(Command::PauseAccount { account_id: id.clone(), paused: true })).expect("paused");
    assert_eq!(core.sync_phase(&id), None, "the sync task is stopped");
    // (a) no mail is fetched while paused: the server gets a message, the store does not
    imap.add_message("INBOX");
    std::thread::sleep(Duration::from_millis(1500));
    assert_eq!(stored(&core), 2, "nothing is fetched for a paused account");
    // (a) its outgoing queue is held, not lost: the send stays in the Outbox and nothing reaches the server
    let draft = core.run(Command::SaveDraft { from_account_id: Some(id.clone()), to: vec!["friend@example.test".into()], cc: vec![], subject: "Held".into(), body: "Held until resume.".into(), reply_to_thread_id: None }).expect("draft").created_id.unwrap();
    let entry = core.run(Command::Send { draft_id: draft }).expect("queued").created_id.unwrap();
    core.run(Command::SendNow { outbox_id: entry.clone() }).expect("send now answers");
    std::thread::sleep(Duration::from_millis(300));
    assert!(smtp.got.lock().unwrap().is_empty(), "a paused account sends nothing");
    let held = core.outbox().into_iter().find(|o| o.id == entry).expect("the entry is still in the Outbox");
    assert!(matches!(&held.state, OutboxState::Failed { failure, .. } if failure.code.as_deref() == Some("account.paused") && failure.retryable), "{:?}", held.state);
    // a pending server operation made while paused is kept
    let mid = core.inbox(InboxQuery { tab_id: "primary".into(), account_id: None, label_id: None, mailbox: None, account_ids: vec![], filters: vec![], smart_folder_id: None })[0].message_id.clone();
    core.run(Command::MarkRead { message_ids: vec![mid], read: true }).expect("read");
    assert!(!core.store().pending_actions().unwrap().is_empty(), "the pending operation waits for the account");

    // resume: the sync runs again (the message that arrived is fetched), the held message leaves, the pending operation reaches the server
    timed("PauseAccount resume (live)", || core.run(Command::PauseAccount { account_id: id.clone(), paused: false })).expect("resumed");
    wait_until("the missed mail is fetched", || stored(&core) == 3);
    wait_until("the held message left through SMTP", || smtp.got.lock().unwrap().len() == 1);
    assert!(smtp.got.lock().unwrap()[0].contains("Held until resume."));
    wait_until("the held entry is no longer open", || core.outbox().iter().all(|o| o.id != entry));
    wait_until("the pending operation was sent", || core.store().pending_actions().unwrap().is_empty());
    println!("PASS paused-account-fetches-nothing-keeps-its-outbox-and-pending-and-resume-fetches-sends-and-flushes");
    drop(core);
    drop(rt);
}
