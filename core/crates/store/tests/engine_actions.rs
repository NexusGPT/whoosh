//! engine-actions: the store doors the real core's Mute, Unarchive and RemindIfNoReply run on (docs/lanes/engine-actions.md).
//! Each test was seen red on main's sources, and on the named single-edit mutant written in the comment under it.
mod common;
use common::*;
use contracts::*;

fn ids(v: &[&str]) -> Vec<String> { v.iter().map(|s| s.to_string()).collect() }
fn column(s: &store::Store, sql: &'static str, id: &'static str) -> Option<String> {
    s.write(move |c| Ok(c.tx.query_row(sql, rusqlite::params![id], |r| r.get::<_, Option<String>>(0))?)).unwrap()
}
fn muted(s: &store::Store, id: &str) -> bool { s.is_muted(id).unwrap().expect("a stored message") }

/// Two threads: t1 = m1 (INBOX, read) + m2 (INBOX) + s1 (the owner's Sent copy); t2 = m3.
fn two_threads(tag: &str) -> (TempDb, store::Store) {
    let db = TempDb::new(tag);
    let s = small_store(&db, 0);
    let mut sent = msg("s1", "t1", "acc-a", "owner0@example.test", 300); sent.folder = "Sent".into();
    s.insert_messages(vec![
        new_msg(msg("m1", "t1", "acc-a", "them@ext.test", 100), Some("primary")),
        new_msg(msg("m2", "t1", "acc-a", "them@ext.test", 200), Some("primary")),
        new_msg(sent, None),
        new_msg(msg("m3", "t2", "acc-a", "other@ext.test", 400), Some("primary")),
    ]).unwrap();
    (db, s)
}

#[test]
fn muting_one_message_mutes_its_whole_thread_and_nothing_else_and_unmuting_lifts_it() {
    let (_db, s) = two_threads("ea-mute");
    let members = s.mute_threads(ids(&["m1"]), true).unwrap();
    let mut sorted = members.clone(); sorted.sort();
    assert_eq!(sorted, ids(&["m1", "m2", "s1"]), "the thread follows its messages");
    assert!(muted(&s, "m1") && muted(&s, "m2") && muted(&s, "s1"));
    assert!(!muted(&s, "m3"), "another thread is not muted");
    assert_eq!(s.is_muted("nope").unwrap(), None);
    // a muted thread is in no inbox list and no count
    let inbox = s.inbox(&InboxQuery { tab_id: "primary".into(), account_id: None, label_id: None, mailbox: None, account_ids: vec![], filters: vec![], smart_folder_id: None }).unwrap();
    assert_eq!(inbox.iter().map(|r| r.message_id.as_str()).collect::<Vec<_>>(), vec!["m3"], "{inbox:?}");
    let view = s.view_rows(store::StoredView::Muted, None, 50).unwrap();
    let mut held: Vec<&str> = view.iter().map(|r| r.message_id.as_str()).collect(); held.sort();
    assert_eq!(held, vec!["m1", "m2", "s1"], "the Muted view holds the thread");
    s.mute_threads(ids(&["m2"]), false).unwrap();
    assert!(!muted(&s, "m1") && !muted(&s, "m2") && !muted(&s, "s1"));
}
// mutant "mute-named-only": in WriteCtx::mute_threads, `self.set_muted(&members, on)` becomes `self.set_muted(message_ids, on)`.

#[test]
fn a_message_that_arrives_in_a_muted_thread_arrives_read_and_muted_and_stays_out_of_the_inbox() {
    let (_db, s) = two_threads("ea-arrival");
    s.mute_threads(ids(&["m1"]), true).unwrap();
    s.insert_messages(vec![new_msg(msg("m4", "t1", "acc-a", "them@ext.test", 500), Some("primary")), new_msg(msg("m5", "t2", "acc-a", "other@ext.test", 600), Some("primary"))]).unwrap();
    let m4 = s.message("m4").unwrap().expect("stored");
    assert!(m4.read, "it arrives read");
    assert!(muted(&s, "m4"), "and muted");
    let m5 = s.message("m5").unwrap().unwrap();
    assert!(!m5.read && !muted(&s, "m5"), "a message of an unmuted thread arrives as it always did");
}

fn archive_pairs() -> Vec<(String, String)> { vec![("acc-a".into(), "Archive".into())] }

#[test]
fn unarchive_brings_back_a_flag_archived_message_and_one_from_the_archive_folder_and_only_those() {
    let db = TempDb::new("ea-unarchive");
    let s = small_store(&db, 0);
    let mut flagged = msg("a1", "ta1", "acc-a", "x@ext.test", 100); flagged.archived = true;
    let mut filed = msg("a2", "ta2", "acc-a", "y@ext.test", 200); filed.folder = "Archive".into();
    let mut other_account = msg("a3", "ta3", "acc-b", "z@ext.test", 300); other_account.folder = "Archive".into();
    let mut kept = new_msg(msg("a4", "ta4", "acc-a", "w@ext.test", 400), Some("primary")); kept.remote_id = Some("4".into());
    let mut filed_n = new_msg(filed, None); filed_n.remote_id = Some("7".into());
    s.insert_messages(vec![new_msg(flagged, Some("primary")), filed_n, new_msg(other_account, None), kept]).unwrap();
    let changed = s.unarchive(ids(&["a1", "a2", "a3", "a4", "nope"]), archive_pairs()).unwrap();
    assert_eq!(changed, ids(&["a1", "a2"]), "a4 was never archived, a3 sits in acc-b's folder the roles did not name, nope is unknown");
    assert!(!s.message("a1").unwrap().unwrap().archived);
    let a2 = s.message("a2").unwrap().unwrap();
    assert_eq!((a2.folder.as_str(), a2.archived), ("INBOX", false));
    assert_eq!(column(&s, "SELECT remote_id FROM message WHERE id = ?1", "a2"), None, "the Archive folder's UID does not travel to INBOX");
    assert_eq!(column(&s, "SELECT folder FROM message WHERE id = ?1", "a3").as_deref(), Some("Archive"));
    assert_eq!(column(&s, "SELECT remote_id FROM message WHERE id = ?1", "a4").as_deref(), Some("4"));
    let inbox = s.inbox(&InboxQuery { tab_id: "primary".into(), account_id: None, label_id: None, mailbox: None, account_ids: vec![], filters: vec![], smart_folder_id: None }).unwrap();
    let mut seen: Vec<&str> = inbox.iter().map(|r| r.message_id.as_str()).collect(); seen.sort();
    assert!(seen.contains(&"a1") && seen.contains(&"a2") && seen.contains(&"a4"), "{seen:?}");
    assert!(s.unarchive(ids(&["a1", "a2"]), archive_pairs()).unwrap().is_empty(), "done twice changes nothing");
}
// mutant "unarchive-keeps-folder": in WriteCtx::unarchive, the archive-folder branch's SQL loses `folder = 'INBOX', remote_id = NULL` (it clears the flag only).

fn draft(s: &store::Store, id: &'static str, thread: Option<&'static str>) {
    s.write(move |c| { c.tx.execute("INSERT INTO draft (id, from_account_id, reply_to_thread_id, updated_at_ms) VALUES (?1, 'acc-a', ?2, 0)", rusqlite::params![id, thread])?; Ok(()) }).unwrap();
}
fn outbox_entry(s: &store::Store, entry: &'static str, draft_id: &'static str, state: &'static str) {
    s.write(move |c| {
        let json = format!(r#"{{"scheduled":false,"draft":{{"id":"{draft_id}"}},"idempotency_key":"k","threading":null,"copy_pending":null}}"#);
        c.tx.execute("INSERT INTO outbox_entry (id, draft_json, state, due_at_ms, created_at_ms, updated_at_ms) VALUES (?1, ?2, ?3, 10, 0, 0)", rusqlite::params![entry, json, state])?;
        Ok(())
    }).unwrap();
}

#[test]
fn a_reminder_is_set_on_a_draft_refused_in_the_past_and_replaces_the_one_of_the_same_reply_thread() {
    let db = TempDb::new("ea-remind");
    let s = small_store(&db, 0);
    draft(&s, "d1", Some("t1")); draft(&s, "d2", Some("t1")); draft(&s, "d3", Some("t9"));
    assert!(!s.reply_reminder_target_exists("nope").unwrap());
    assert!(s.reply_reminder_target_exists("d1").unwrap());
    assert!(matches!(s.remind_if_no_reply("d1".into(), 100, 100), Err(store::StoreError::Invalid(_))), "now is the past");
    assert_eq!(s.remind_if_no_reply("nope".into(), 9_000, 100).unwrap(), false, "an unknown draft");
    assert!(s.remind_if_no_reply("d1".into(), 9_000, 100).unwrap());
    assert!(s.remind_if_no_reply("d3".into(), 8_000, 100).unwrap());
    let at = |id: &'static str| column(&s, "SELECT CAST(remind_if_no_reply_at_ms AS TEXT) FROM draft WHERE id = ?1", id);
    assert_eq!((at("d1").as_deref(), at("d3").as_deref()), (Some("9000"), Some("8000")));
    assert!(s.remind_if_no_reply("d2".into(), 7_000, 100).unwrap());
    assert_eq!((at("d2").as_deref(), at("d1").as_deref(), at("d3").as_deref()), (Some("7000"), None, Some("8000")), "a thread holds one reminder: d2 replaced d1's, d3 answers another thread");
}
// mutant "remind-keeps-both": in WriteCtx::remind_if_no_reply the second UPDATE (the one that clears the other drafts of the reply thread) gets `WHERE 1 = 0 AND` in front of its condition, so it clears nothing.

#[test]
fn a_reminder_set_on_an_outbox_entry_survives_the_draft_and_follows_the_sent_message() {
    let db = TempDb::new("ea-remind-outbox");
    let s = small_store(&db, 0);
    outbox_entry(&s, "ob1", "d-gone", "undoable");
    outbox_entry(&s, "ob2", "d-cancelled", "cancelled");
    assert!(s.reply_reminder_target_exists("ob1").unwrap() && s.reply_reminder_target_exists("d-gone").unwrap(), "by the entry's id or by the id of its draft");
    assert!(!s.reply_reminder_target_exists("ob2").unwrap(), "a cancelled entry never leaves: nothing to remind about");
    assert!(s.remind_if_no_reply("d-gone".into(), 5_000, 100).unwrap());
    assert_eq!(s.outbox_reply_reminder("ob1").unwrap(), Some(5_000));
    assert_eq!(s.outbox_reply_reminder("ob2").unwrap(), None);
    // the message goes out: the sent copy is stored and the reminder rides it
    let mut sent = msg("s1", "t1", "acc-a", "owner0@example.test", 200); sent.folder = "Sent".into();
    s.insert_messages(vec![new_msg(sent, None)]).unwrap();
    let rid = s.reply_reminder_on_sent("d-gone".into(), "s1".into(), 200).unwrap();
    assert_eq!(rid.as_deref(), Some("rr-s1"));
    let f = s.reply_reminder_facts("t1").unwrap().expect("the thread holds the reminder");
    assert_eq!((f.at_ms, f.key.as_str(), f.on_draft), (5_000, "s1", false));
    assert_eq!(s.reply_reminder_on_sent("never-had-one".into(), "s1".into(), 200).unwrap(), None);
    assert_eq!(s.outbox_reply_reminder_clear("ob1".into()).unwrap(), 1);
    assert_eq!(s.outbox_reply_reminder("ob1").unwrap(), None);
}
// mutant "sent-ignores-outbox": in WriteCtx::reply_reminder_on_sent the outbox fallback `None => self.outbox_reply_reminder(draft_id)?` becomes `None => None`.

#[test]
fn the_list_header_reads_a_message_account_and_the_done_mark() {
    let db = TempDb::new("ea-list");
    let s = small_store(&db, 2);
    s.set_list_unsubscribe("m000".into(), ids(&["<https://l.example/u>"]), true).unwrap();
    assert_eq!(s.message_account("m000").unwrap().as_deref(), Some("acc-a"));
    assert_eq!(s.message_account("nope").unwrap(), None);
    assert_eq!(s.unsubscribed_at("m000").unwrap(), None);
    assert!(s.mark_unsubscribed("m000".into(), 77).unwrap());
    assert_eq!(s.unsubscribed_at("m000").unwrap(), Some(77));
    assert_eq!(s.unsubscribed_at("m001").unwrap(), None);
}
