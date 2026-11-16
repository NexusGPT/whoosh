//! contracts delta 18, the store side (docs/lanes/engine-import.md): imported mail names its source (migration 0010), the muted-arrival
//! rule, and the facts of a reply reminder with the rows of the Reminders view. Mutants (single edits of one production file; the runner is
//! _captures/engine-import/run-mutants.py, verbatim results in mutants-store.out):
//!   S1 ops.rs `insert_batch`: `apply_muted_arrival` never runs                -> a_muted_threads_new_mail_arrives_read_and_muted
//!   S2 read.rs `VISIBLE` forgets `m.muted = 0`                                -> a_muted_threads_new_mail_arrives_read_and_muted
//!   S3 import_rows.rs: the new rows are never marked imported                 -> imported_rows_name_their_source_and_a_known_row_keeps_its_marks
//!   S4 import_rows.rs: a row that was already stored is marked too            -> imported_rows_name_their_source_and_a_known_row_keeps_its_marks
//!   S5 reminders.rs: the view counts any later reply, not only one by the time -> a_fired_reminder_is_in_the_view_until_a_reply_in_time_or_a_dismissal
//!   S6 reminders.rs: the view forgets "the time has come"                     -> a_fired_reminder_is_in_the_view_until_a_reply_in_time_or_a_dismissal
//!   S7 reminders.rs `reply_reminders_mark_fired`: an answered reminder is marked -> the_firing_write_marks_each_reminder_once_and_never_an_answered_one
//!   S8 import_rows.rs `star_imported_in_run`: stars any run's rows            -> flagged_imports_are_starred_for_this_runs_rows_only
mod common;
use common::*;
use contracts::*;
use rusqlite::params;
use store::{Store, StoredView};

const OWNER: &str = "owner0@example.test"; // acc-a's address (small_store)

fn ids(v: &[&str]) -> Vec<String> { v.iter().map(|s| s.to_string()).collect() }

fn primary_ids(s: &Store) -> Vec<String> {
    let q = InboxQuery { tab_id: "primary".into(), account_id: None, label_id: None, mailbox: None, account_ids: vec![], filters: vec![], smart_folder_id: None };
    s.inbox(&q).unwrap().into_iter().map(|r| r.message_id).collect()
}

fn muted_ids(s: &Store) -> Vec<String> { s.view_rows(StoredView::Muted, None, 50).unwrap().into_iter().map(|r| r.message_id).collect() }

fn marks(s: &Store, id: &str) -> (String, Option<String>, Option<i64>) {
    let id = id.to_string();
    s.write(move |c| Ok(c.tx.query_row("SELECT source, import_source_id, imported_at_ms FROM message WHERE id = ?1", params![id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?)).unwrap()
}

fn flags(s: &Store, id: &str) -> (bool, bool) {
    let id = id.to_string();
    s.write(move |c| Ok(c.tx.query_row("SELECT read, muted FROM message WHERE id = ?1", params![id], |r| Ok((r.get::<_, i64>(0)? != 0, r.get::<_, i64>(1)? != 0)))?)).unwrap()
}

#[test]
fn a_muted_threads_new_mail_arrives_read_and_muted() {
    let db = TempDb::new("d18-mute");
    let s = small_store(&db, 0);
    s.insert_messages(vec![new_msg(msg("a1", "t-muted", "acc-a", "x@ext.test", 1_000), None), new_msg(msg("c1", "t-calm", "acc-a", "y@ext.test", 1_100), None)]).unwrap();
    assert_eq!(s.set_muted(ids(&["a1"]), true).unwrap(), 1);
    // new mail in the muted thread, and new mail in an ordinary thread, arrive in one batch
    s.insert_messages(vec![new_msg(msg("a2", "t-muted", "acc-a", "x@ext.test", 2_000), None), new_msg(msg("c2", "t-calm", "acc-a", "y@ext.test", 2_100), None)]).unwrap();
    assert_eq!(flags(&s, "a2"), (true, true), "the muted thread's new mail is read and muted");
    assert_eq!(flags(&s, "c2"), (false, false), "an ordinary thread's new mail arrives as it always did");
    assert_eq!(muted_ids(&s), vec!["a2".to_string(), "a1".to_string()], "the Muted view holds both messages of the muted thread, newest first");
    let listed = primary_ids(&s);
    assert!(!listed.contains(&"a1".to_string()) && !listed.contains(&"a2".to_string()), "no muted message is in an inbox list: {listed:?}");
    // a list row is a conversation (13 Nov): the ordinary thread is one row, at its newest mail
    assert!(listed.contains(&"c2".to_string()) && !listed.contains(&"c1".to_string()), "{listed:?}");
    let counts = s.counts().unwrap();
    let primary = counts.tabs.iter().find(|t| t.tab_id == "primary").unwrap();
    assert_eq!(primary.unread, 2, "only the two ordinary messages are unread in the counts");
    assert_eq!(s.counts().unwrap(), s.recount().unwrap(), "the tally the writer keeps equals the count from scratch");
    // unmuting returns the thread to the lists as before
    s.set_muted(ids(&["a1", "a2"]), false).unwrap();
    assert!(primary_ids(&s).contains(&"a2".to_string()), "the unmuted thread is back, as one row at its newest mail");
}

#[test]
fn imported_rows_name_their_source_and_a_known_row_keeps_its_marks() {
    let db = TempDb::new("d18-marks");
    let s = small_store(&db, 0);
    s.insert_messages(vec![new_msg(msg("known", "t-known", "acc-a", "x@ext.test", 500), None)]).unwrap();
    let batch = vec![
        new_msg(msg("imp:acc-a:one", "t1", "acc-a", "x@ext.test", 1_000), None),
        new_msg(msg("imp:acc-a:two", "t2", "acc-a", "x@ext.test", 2_000), None),
        new_msg(msg("known", "t-known", "acc-a", "x@ext.test", 500), None),
        new_msg(msg("imp:acc-a:one", "t1", "acc-a", "x@ext.test", 1_000), None),
    ];
    let fresh = s.insert_imported(batch, "mbox:~/Mail/Inbox.mbox", 9_000).unwrap();
    assert_eq!(fresh, ids(&["imp:acc-a:one", "imp:acc-a:two"]), "new rows only, each once, in batch order");
    assert_eq!(marks(&s, "imp:acc-a:one"), ("imported".into(), Some("mbox:~/Mail/Inbox.mbox".into()), Some(9_000)));
    assert_eq!(marks(&s, "imp:acc-a:two"), ("imported".into(), Some("mbox:~/Mail/Inbox.mbox".into()), Some(9_000)));
    assert_eq!(marks(&s, "known"), ("synced".into(), None, None), "a message that was already stored keeps what it was");
    // a second run of the same source stores nothing new and marks nothing
    let again = s.insert_imported(vec![new_msg(msg("imp:acc-a:one", "t1", "acc-a", "x@ext.test", 1_000), None)], "mbox:~/Mail/Inbox.mbox", 9_500).unwrap();
    assert!(again.is_empty());
    assert_eq!(marks(&s, "imp:acc-a:one").2, Some(9_000), "the first run's time stays");
    // the source's history is derived from the rows
    assert_eq!(s.imported_history("mbox:~/Mail/Inbox.mbox").unwrap(), (2, Some(9_000)));
    assert_eq!(s.imported_history("olm:~/never.olm").unwrap(), (0, None));
}

#[test]
fn the_local_imported_account_is_made_once_and_is_never_synced() {
    let db = TempDb::new("d18-account");
    let s = small_store(&db, 0);
    let id = s.add_local_account("Imported@Whoosh.Local", "Imported", 10).unwrap();
    assert_eq!(s.add_local_account("imported@whoosh.local", "Imported", 20).unwrap(), id, "the same address is the same account");
    let all = s.account_summaries().unwrap();
    assert_eq!(all.iter().filter(|a| a.address == "imported@whoosh.local").count(), 1);
    let synced = s.sync_accounts().unwrap();
    let local = synced.iter().find(|a| a.account.id == id).expect("the account is listed");
    assert!(local.config.is_none(), "no server settings, so the sync never starts it and nothing is sent from it");
}

#[test]
fn flagged_imports_are_starred_for_this_runs_rows_only() {
    let db = TempDb::new("d18-star");
    let s = small_store(&db, 0);
    s.insert_imported(vec![new_msg(msg("imp:acc-a:old", "t1", "acc-a", "x@ext.test", 1_000), None)], "olm:~/a.olm", 100).unwrap();
    s.insert_imported(vec![new_msg(msg("imp:acc-a:new", "t2", "acc-a", "x@ext.test", 2_000), None)], "olm:~/a.olm", 200).unwrap();
    // the owner un-starred nothing here; the second run flags both, only its own row is starred
    assert_eq!(s.star_imported_in_run(ids(&["imp:acc-a:old", "imp:acc-a:new"]), "olm:~/a.olm", 200).unwrap(), 1);
    let starred: Vec<String> = s.view_rows(StoredView::Starred, None, 10).unwrap().into_iter().map(|r| r.message_id).collect();
    assert_eq!(starred, vec!["imp:acc-a:new".to_string()]);
}

/// A sent message of the owner's, a reminder riding it, and the replies given.
fn sent_with_reminder(s: &Store, thread: &str, sent_id: &str, sent_at: i64, remind_at: i64, updated: i64) {
    let mut sent = msg(sent_id, thread, "acc-a", OWNER, sent_at);
    sent.folder = "Sent".into();
    sent.read = true;
    s.insert_messages(vec![new_msg(sent, None)]).unwrap();
    let (sid, rid) = (sent_id.to_string(), format!("rr-{sent_id}"));
    s.write(move |c| {
        c.tx.execute("INSERT INTO reminder (id, message_id, when_ms, note, updated_at_ms, if_no_reply) VALUES (?1, ?2, ?3, '', ?4, 1)", params![rid, sid, remind_at, updated])?;
        Ok(())
    })
    .unwrap();
}

fn reply(s: &Store, thread: &str, id: &str, from: &str, at: i64) { s.insert_messages(vec![new_msg(msg(id, thread, "acc-a", from, at), None)]).unwrap(); }

fn in_view(s: &Store, now: i64) -> Vec<String> { s.view_rows(StoredView::Reminders { now_ms: now }, None, 50).unwrap().into_iter().map(|r| r.message_id).collect() }

#[test]
fn a_fired_reminder_is_in_the_view_until_a_reply_in_time_or_a_dismissal() {
    let db = TempDb::new("d18-view");
    let s = small_store(&db, 0);
    // t-wait: the time has not come · t-fired: the time came, nobody replied · t-answered: a reply came before the time
    // t-late: the reply came AFTER the time (it does not unfire) · t-own: the owner's own later message is no reply
    sent_with_reminder(&s, "t-wait", "s-wait", 1_000, 90_000, 1);
    sent_with_reminder(&s, "t-fired", "s-fired", 1_000, 5_000, 2);
    sent_with_reminder(&s, "t-answered", "s-answered", 1_000, 5_000, 3);
    reply(&s, "t-answered", "r-answered", "them@ext.test", 4_000);
    sent_with_reminder(&s, "t-late", "s-late", 1_000, 5_000, 4);
    reply(&s, "t-late", "r-late", "them@ext.test", 6_000);
    sent_with_reminder(&s, "t-own", "s-own", 1_000, 5_000, 5);
    let mut again = msg("s-own-2", "t-own", "acc-a", OWNER, 3_000);
    again.folder = "Sent".into();
    s.insert_messages(vec![new_msg(again, None)]).unwrap();
    let now = 10_000;
    let listed = in_view(&s, now);
    assert_eq!(listed.len(), 3, "{listed:?}");
    for want in ["s-fired", "s-late", "s-own"] {
        assert!(listed.contains(&want.to_string()), "{want} is in the view: {listed:?}");
    }
    assert!(!listed.contains(&"s-wait".to_string()), "a reminder whose time has not come is not in the view");
    assert!(!listed.contains(&"s-answered".to_string()), "a reply in time answers it: it never fires");
    // the facts the contract's state is derived from
    let f = s.reply_reminder_facts("t-fired").unwrap().unwrap();
    assert_eq!((f.key.as_str(), f.at_ms, f.first_reply_ms, f.on_draft), ("s-fired", 5_000, None, false));
    assert_eq!(s.reply_reminder_facts("s-answered").unwrap().unwrap().first_reply_ms, Some(4_000), "asked from the sent message's own id");
    assert_eq!(s.reply_reminder_facts("r-late").unwrap().unwrap().first_reply_ms, Some(6_000), "asked from a message of the thread");
    assert!(s.reply_reminder_facts("t-nothing").unwrap().is_none());
    // the account scope narrows the view
    assert!(s.view_rows(StoredView::Reminders { now_ms: now }, Some(ids(&["acc-b"])), 50).unwrap().is_empty());
    // a thread holds one reminder: the newer one counts, the older is not a second row
    s.write(|c| { c.tx.execute("INSERT INTO reminder (id, message_id, when_ms, note, updated_at_ms, if_no_reply) VALUES ('rr-extra', 's-fired', 5_500, '', 99, 1)", [])?; Ok(()) }).unwrap();
    assert_eq!(in_view(&s, now).iter().filter(|m| *m == "s-fired").count(), 1);
    // a dismissal takes the whole thread's reminders away
    assert!(s.reply_reminder_clear("s-fired").unwrap() >= 1);
    assert!(!in_view(&s, now).contains(&"s-fired".to_string()));
    assert!(s.reply_reminder_facts("t-fired").unwrap().is_none());
}

#[test]
fn the_firing_write_marks_each_reminder_once_and_never_an_answered_one() {
    let db = TempDb::new("d18-fire");
    let s = small_store(&db, 0);
    sent_with_reminder(&s, "t-due", "s-due", 1_000, 5_000, 1);
    sent_with_reminder(&s, "t-answered", "s-answered", 1_000, 5_000, 2);
    reply(&s, "t-answered", "r1", "them@ext.test", 4_000);
    sent_with_reminder(&s, "t-later", "s-later", 1_000, 50_000, 3);
    assert_eq!(s.reply_reminders_mark_fired(10_000).unwrap(), vec!["s-due".to_string()]);
    assert!(s.reply_reminders_mark_fired(10_001).unwrap().is_empty(), "once");
    let fired: Vec<(String, Option<i64>)> = s.write(|c| {
        let mut st = c.tx.prepare("SELECT message_id, fired_at_ms FROM reminder ORDER BY message_id")?;
        let rows = st.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?;
        Ok(rows.collect::<Result<Vec<_>, _>>()?)
    }).unwrap();
    assert_eq!(fired, vec![("s-answered".to_string(), None), ("s-due".to_string(), Some(10_000)), ("s-later".to_string(), None)]);
}

#[test]
fn a_draft_reminder_waits_and_clears_with_the_thread() {
    let db = TempDb::new("d18-draft");
    let s = small_store(&db, 0);
    s.write(|c| {
        c.tx.execute("INSERT INTO draft (id, from_account_id, reply_to_thread_id, remind_if_no_reply_at_ms, updated_at_ms) VALUES ('d1', 'acc-a', 't-d', 777, 1)", [])?;
        Ok(())
    }).unwrap();
    let f = s.reply_reminder_facts("t-d").unwrap().unwrap();
    assert_eq!((f.key.as_str(), f.at_ms, f.on_draft), ("d1", 777, true));
    assert_eq!(s.reply_reminder_facts("d1").unwrap().unwrap().key, "d1");
    assert_eq!(s.reply_reminder_clear("d1").unwrap(), 1);
    assert!(s.reply_reminder_facts("t-d").unwrap().is_none());
}
