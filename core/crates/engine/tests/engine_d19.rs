//! contracts-d19: undo for Mute, Unmute and Unarchive, the server move behind Unarchive, and MarkNotSpam (the reverse of MarkSpam), all
//! through `RustCore::run` on a seeded store (no network: the write-back is read as the `pending_action` rows it becomes). Each test was
//! seen red on the single-edit mutant named under it (the logs are in _captures/contracts-d19/).
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

fn detector() -> Box<dyn engine::Detect> { Box::new(autoconfig::Detector::new(FakeHttp::default(), FakeDns::default(), FakeProbe::default())) }
fn kc() -> Arc<MemoryKeychain> { Arc::new(MemoryKeychain::new(Namespace::rig())) }
fn config() -> AccountConfig {
    AccountConfig {
        imap_host: "127.0.0.1".into(), imap_port: 1, imap_security: ConnectionSecurity::Ssl, smtp_host: "127.0.0.1".into(), smtp_port: 1,
        smtp_security: ConnectionSecurity::Starttls, username: USER.into(), smtp_username: USER.into(), auth_method: AuthMethod::Password, source: DetectionSource::Manual,
    }
}
/// A stored message with a server UID (`uid`), so the write-back has a row to make.
fn msg(account: &str, folder: &str, id: &str, thread: &str, uid: &str, at: i64) -> NewMessage {
    NewMessage {
        message: Message {
            id: id.into(), thread_id: thread.into(), account_id: account.into(), sender_name: "x".into(), from_address: "x@ext.test".into(), to: vec![USER.into()],
            subject: format!("Subject {id}"), preview: format!("Preview {id}"), received_at_ms: at, read: false, archived: false, folder: folder.into(),
            attachment_names: vec![], login_code: None, trackers_blocked: 0, list_unsubscribe: vec![], list_unsubscribe_one_click: false,
            reply_to_name: None, cc: vec![], bcc: vec![], is_outgoing: false,
        },
        remote_id: Some(uid.into()), body_text: Some(format!("body of {id}")), body_html: None, has_list_header: false, size_bytes: Some(100), tab_id: None,
    }
}
fn seeded() -> (RustCore, String, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(&dir.path().join("whoosh.sqlite")).unwrap();
    let id = store.add_account(USER, "Owner", MailProvider::Imap, "svc\u{1f}acct", false, config(), 1).unwrap();
    (RustCore::with_parts(store, kc(), detector(), TestTransport::PlainLoopback), id, dir)
}
fn sql(core: &RustCore, q: &'static str) { core.store().write(move |c| { c.tx.execute_batch(q)?; Ok(()) }).unwrap(); }
fn text(core: &RustCore, q: &'static str) -> String { core.store().write(move |c| Ok(c.tx.query_row(q, [], |r| r.get::<_, String>(0))?)).unwrap() }
fn num(core: &RustCore, q: &'static str) -> i64 { core.store().write(move |c| Ok(c.tx.query_row(q, [], |r| r.get::<_, i64>(0))?)).unwrap() }
fn muted(core: &RustCore, id: &str) -> String { core.state(format!("message/{id}/muted")).expect("a muted read") }
fn spam(core: &RustCore, id: &str) -> String { core.state(format!("message/{id}/spam")).expect("a spam read") }
fn ids(v: &[&str]) -> Vec<String> { v.iter().map(|s| s.to_string()).collect() }
/// The server rows queued so far: (kind, message id, folder, uid, payload), oldest first.
fn rows(core: &RustCore) -> Vec<(String, String, String, i64, String)> {
    core.store().pending_actions().unwrap().into_iter().map(|r| (r.kind, r.message_id.unwrap_or_default(), r.folder, r.uid.unwrap_or(0), r.payload_json)).collect()
}
fn undo(core: &RustCore) -> CommandOutcome { core.run(Command::Undo).expect("undone") }

// ---------------------------------------------------------------- Mute and Unmute

#[test]
fn z_after_a_mute_puts_each_messages_earlier_flag_back() {
    let (core, a, _d) = seeded();
    // thread t1 holds m1 (muted before) and m2 (not); t2 holds m3
    core.store().insert_messages(vec![msg(&a, "INBOX", "m1", "t1", "1", 100), msg(&a, "INBOX", "m2", "t1", "2", 200), msg(&a, "INBOX", "m3", "t2", "3", 300)]).unwrap();
    sql(&core, "UPDATE message SET muted = 1 WHERE id = 'm1'");
    let out = core.run(Command::Mute { message_ids: ids(&["m2"]), muted: true }).expect("muted");
    let entry = out.undo_entry_id.expect("a mute records one undo entry");
    assert_eq!((muted(&core, "m1"), muted(&core, "m2"), muted(&core, "m3")), ("true".into(), "true".into(), "false".into()), "the whole thread is muted");
    assert_eq!(undo(&core).undo_entry_id.as_deref(), Some(entry.as_str()), "Z reverses that entry");
    assert_eq!((muted(&core, "m1"), muted(&core, "m2")), ("true".into(), "false".into()), "m1 was muted before the command and stays muted; m2 is unmuted");
    assert_eq!(muted(&core, "m3"), "false", "another thread is not touched");
}
// mutant "undo-ignores-was-muted": actions/src/usecases/mute.rs, in `reverse`, the line `ctx.set_muted(was_muted, true)?;` is deleted.

#[test]
fn z_after_an_unmute_mutes_the_thread_again_and_a_command_that_changes_nothing_records_nothing() {
    let (core, a, _d) = seeded();
    core.store().insert_messages(vec![msg(&a, "INBOX", "m1", "t1", "1", 100), msg(&a, "INBOX", "m2", "t1", "2", 200), msg(&a, "INBOX", "m3", "t2", "3", 300)]).unwrap();
    sql(&core, "UPDATE message SET muted = 1 WHERE thread_id = 't1'");
    assert_eq!(core.run(Command::Mute { message_ids: ids(&["m1"]), muted: true }).unwrap().undo_entry_id, None, "already muted: nothing changed, no entry");
    assert_eq!(core.run(Command::Mute { message_ids: ids(&["m3"]), muted: false }).unwrap().undo_entry_id, None, "already not muted: no entry");
    assert_eq!(num(&core, "SELECT count(*) FROM undo_entry"), 0);
    let out = core.run(Command::Mute { message_ids: ids(&["m2"]), muted: false }).unwrap();
    assert!(out.undo_entry_id.is_some());
    assert_eq!((muted(&core, "m1"), muted(&core, "m2")), ("false".into(), "false".into()));
    undo(&core);
    assert_eq!((muted(&core, "m1"), muted(&core, "m2")), ("true".into(), "true".into()), "Z after an unmute mutes the thread again");
}

// ---------------------------------------------------------------- Unarchive

#[test]
fn unarchive_queues_the_servers_move_and_z_archives_the_messages_again() {
    let (core, a, _d) = seeded();
    // m1 was archived by the flag (UID 5 in INBOX); m2 sits in the Archive folder (UID 9); m3 is in the inbox and untouched
    let mut m1 = msg(&a, "INBOX", "m1", "t1", "5", 100); m1.message.archived = true;
    core.store().insert_messages(vec![m1, msg(&a, "Archive", "m2", "t2", "9", 200), msg(&a, "INBOX", "m3", "t3", "7", 300)]).unwrap();
    let out = core.run(Command::Unarchive { message_ids: ids(&["m1", "m2", "m3"]) }).expect("unarchived");
    let entry = out.undo_entry_id.expect("one undo entry for the messages that changed");
    assert_eq!((num(&core, "SELECT archived FROM message WHERE id = 'm1'"), text(&core, "SELECT folder FROM message WHERE id = 'm2'")), (0, "INBOX".to_string()));
    let queued = rows(&core);
    assert!(queued.contains(&("move_to_folder".into(), "m2".into(), "Archive".into(), 9, r#"{"folder":"INBOX"}"#.into())), "the Archive-folder message is moved to INBOX on the server: {queued:?}");
    assert!(queued.contains(&("set_archived".into(), "m1".into(), "INBOX".into(), 5, r#"{"archived":false}"#.into())), "the flag-archived one loses \\Archived on the server: {queued:?}");
    assert_eq!(queued.len(), 2, "m3 had nothing to undo and gets no row: {queued:?}");
    // Z archives both again
    assert_eq!(undo(&core).undo_entry_id.as_deref(), Some(entry.as_str()));
    assert_eq!((num(&core, "SELECT archived FROM message WHERE id = 'm1'"), num(&core, "SELECT archived FROM message WHERE id = 'm2'"), num(&core, "SELECT archived FROM message WHERE id = 'm3'")), (1, 1, 0), "m1 and m2 are archived again, m3 never changed");
    assert!(rows(&core).iter().any(|r| r.0 == "set_archived" && r.1 == "m1" && r.4 == r#"{"archived":true}"#), "and the server hears that m1 is archived again");
}
// mutants: "undo-does-not-rearchive": actions/src/usecases/undo.rs, the Unarchive arm `archive::apply(ctx, message_ids, true, false)?` becomes `archive::apply(ctx, message_ids, false, false)?`;
// "unarchive-queues-no-row": engine/src/unarchive.rs, the `Ledger.execute(...)` of the MoveToFolder request is deleted;
// "unarchive-wrong-folder": engine/src/unarchive.rs, the MoveToFolder `folder: store::INBOX_FOLDER.to_string()` becomes `folder: "Trash".to_string()`.

#[test]
fn unarchive_of_a_message_with_no_uid_changes_it_locally_and_queues_nothing() {
    let (core, a, _d) = seeded();
    core.store().insert_messages(vec![msg(&a, "Archive", "m1", "t1", "0", 100)]).unwrap();
    sql(&core, "UPDATE message SET remote_id = NULL WHERE id = 'm1'");
    core.run(Command::Unarchive { message_ids: ids(&["m1"]) }).expect("unarchived");
    assert_eq!(text(&core, "SELECT folder FROM message WHERE id = 'm1'"), "INBOX");
    assert!(rows(&core).is_empty(), "no UID, no server copy to move: {:?}", rows(&core));
    assert!(matches!(core.run(Command::Unarchive { message_ids: ids(&["ghost"]) }), Err(CoreError::NotFound { .. })));
}

// ---------------------------------------------------------------- MarkNotSpam

#[test]
fn mark_not_spam_leaves_spam_learns_the_lesson_and_the_state_reads_false() {
    let (core, a, _d) = seeded();
    core.store().insert_messages(vec![msg(&a, "INBOX", "m1", "t1", "1", 100), msg(&a, "INBOX", "m2", "t2", "2", 200)]).unwrap();
    core.run(Command::MarkSpam { message_ids: ids(&["m1"]) }).expect("spam");
    assert_eq!((spam(&core, "m1"), text(&core, "SELECT folder FROM message WHERE id = 'm1'")), ("true".into(), "Spam".into()));
    assert_eq!(text(&core, "SELECT label FROM junk_learned WHERE message_id = 'm1'"), "junk");
    core.run(Command::MarkNotSpam { message_ids: ids(&["m1", "m2"]) }).expect("not spam");
    assert_eq!(spam(&core, "m1"), "false", "state message/<id>/spam reads false after");
    assert_eq!(text(&core, "SELECT folder FROM message WHERE id = 'm1'"), "INBOX", "the message came back to the inbox");
    assert_eq!(text(&core, "SELECT label FROM junk_learned WHERE message_id = 'm1'"), "not_junk", "the lesson changed from junk to not junk (relearned)");
    assert_eq!(text(&core, "SELECT label FROM junk_learned WHERE message_id = 'm2'"), "not_junk", "a message that was never in Spam is still a lesson");
    assert_eq!(text(&core, "SELECT folder FROM message WHERE id = 'm2'"), "INBOX", "and stays where it was");
    assert_eq!(spam(&core, "m2"), "false");
}
// mutants: "not-spam-leaves-spam": actions/src/usecases/spam.rs, in `restore_from_spam`, `ctx.set_folder(&changed_ids, store::INBOX_FOLDER)?` is deleted;
// "lesson-not-learned": engine/src/rust_core.rs, in the MarkNotSpam arm, the `crate::junk_filter::learn(..., store::JunkLesson::NotJunk)` call is deleted;
// "not-spam-wrong-folder": spam.rs, `ctx.set_folder(&changed_ids, store::INBOX_FOLDER)?` becomes `ctx.set_folder(&changed_ids, crate::constants::TRASH_FOLDER)?`.

#[test]
fn mark_not_spam_on_a_message_the_sync_stored_under_the_servers_junk_folder_moves_it_back_on_the_server() {
    let (core, a, _d) = seeded();
    core.store().insert_messages(vec![msg(&a, "Junk", "j1", "t1", "42", 100), msg(&a, "INBOX", "m2", "t2", "2", 200)]).unwrap();
    core.run(Command::MarkNotSpam { message_ids: ids(&["j1"]) }).expect("not spam");
    assert_eq!(text(&core, "SELECT folder FROM message WHERE id = 'j1'"), "INBOX");
    assert!(rows(&core).contains(&("move_to_folder".into(), "j1".into(), "Junk".into(), 42, r#"{"folder":"INBOX"}"#.into())), "{:?}", rows(&core));
    assert!(matches!(core.run(Command::MarkNotSpam { message_ids: ids(&["ghost"]) }), Err(CoreError::NotFound { .. })), "an unknown id is refused in words");
}
