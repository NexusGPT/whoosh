//! The read queries: ordering, filters, and what they leave out.
mod common;
use common::*;
use contracts::InboxQuery;

fn q(tab: &str) -> InboxQuery {
    InboxQuery { tab_id: tab.into(), account_id: None, label_id: None, mailbox: None, account_ids: vec![], filters: vec![], smart_folder_id: None }
}

fn ids(rows: &[contracts::InboxRow]) -> Vec<String> {
    rows.iter().map(|r| r.message_id.clone()).collect()
}

#[test]
fn defaults_are_seeded_from_the_ledger_and_wal_and_foreign_keys_are_on() {
    let db = TempDb::new("defaults");
    let store = small_store(&db, 0);
    let c = store.counts().unwrap();
    let names: Vec<&str> = c.tabs.iter().map(|t| t.tab_id.as_str()).collect();
    assert_eq!(names, ["primary", "all", "needs-reply", "updates", "promotions", "social", "vips"], "delta 16: the two views, then the enabled tabs");
    let mode: String = store.write(|ctx| Ok(ctx.tx.query_row("PRAGMA journal_mode", [], |r| r.get(0))?)).unwrap();
    assert_eq!(mode, "wal");
    let fk: i64 = store.write(|ctx| Ok(ctx.tx.query_row("PRAGMA foreign_keys", [], |r| r.get(0))?)).unwrap();
    assert_eq!(fk, 1);
    let orphan = store.write(|ctx| Ok(ctx.tx.execute("INSERT INTO snooze (message_id, return_at_ms, updated_at_ms) VALUES ('nope', 1, 1)", [])?));
    assert!(orphan.is_err(), "a snooze for a message that does not exist is refused");
}

#[test]
fn a_tab_lists_newest_first_and_leaves_out_archived_snoozed_other_tabs_and_other_folders() {
    let db = TempDb::new("list");
    let store = small_store(&db, 12); // tabs by i%6: primary = m000, m006
    let mut sent = msg("sent1", "t000", "acc-a", "owner0@example.test", 1_700_000_100_000);
    sent.folder = "Sent".into();
    store.insert_messages(vec![new_msg(sent, Some("primary")), new_msg(msg("p3", "t900", "acc-b", "z@ext.test", 1_700_000_050_000), Some("primary")), new_msg(msg("p4", "t901", "acc-a", "z@ext.test", 1_700_000_060_000), None)]).unwrap();
    // primary now: m000, m006, p3, p4 (uncategorised sits in the first tab); sent1 is another folder
    assert_eq!(ids(&store.inbox(&q("primary")).unwrap()), ["p4", "p3", "m006", "m000"]);
    store.set_archived(vec!["p3".into()], true).unwrap();
    store.snooze(vec!["m006".into()], 9_000_000_000_000, 1).unwrap();
    assert_eq!(ids(&store.inbox(&q("primary")).unwrap()), ["p4", "m000"]);
    assert_eq!(ids(&store.inbox(&q("updates")).unwrap()), ["m002", "m008"].iter().rev().map(|s| s.to_string()).collect::<Vec<_>>());
}

#[test]
fn pinned_rows_sort_first_under_every_filter_and_newest_pin_first() {
    let db = TempDb::new("pins");
    let store = small_store(&db, 12);
    store.set_pinned("m000".into(), true, 10).unwrap();
    store.set_pinned("m006".into(), true, 20).unwrap();
    let all = store.inbox(&q("primary")).unwrap();
    assert_eq!(ids(&all), ["m006", "m000"]);
    assert!(all.iter().all(|r| r.pinned));
    store.insert_messages(vec![new_msg(msg("new", "t800", "acc-a", "n@ext.test", 1_990_000_000_000), Some("primary"))]).unwrap();
    assert_eq!(ids(&store.inbox(&q("primary")).unwrap()), ["m006", "m000", "new"]);
    let only_a = store.inbox(&InboxQuery { tab_id: "primary".into(), account_id: Some("acc-a".into()), label_id: None, mailbox: None, account_ids: vec![], filters: vec![], smart_folder_id: None }).unwrap();
    assert_eq!(ids(&only_a), ["m006", "m000", "new"], "m000 and m006 are both on acc-a");
    let only_b = store.inbox(&InboxQuery { tab_id: "primary".into(), account_id: Some("acc-b".into()), label_id: None, mailbox: None, account_ids: vec![], filters: vec![], smart_folder_id: None }).unwrap();
    assert!(only_b.is_empty());
    assert_eq!(ids(&store.pinned().unwrap()), ["m006", "m000"]);
    store.set_pinned("m006".into(), false, 30).unwrap();
    assert_eq!(ids(&store.pinned().unwrap()), ["m000"]);
}

#[test]
fn rows_carry_labels_attachments_and_thread_size() {
    let db = TempDb::new("rows");
    let store = small_store(&db, 4);
    store
        .write(|ctx| {
            ctx.will_change(&["m000".to_string()])?;
            ctx.tx.execute("INSERT INTO label (id, name, updated_at_ms) VALUES ('l1', 'Receipts', 1), ('l2', 'Alpha', 1)", [])?;
            ctx.tx.execute("INSERT INTO label_assignment (label_id, message_id, updated_at_ms) VALUES ('l1', 'm000', 1), ('l2', 'm000', 1)", [])?;
            ctx.tx.execute("INSERT INTO attachment (id, message_id, file_name, mime_type, size_bytes) VALUES ('a1', 'm000', 'x.pdf', 'application/pdf', 10)", [])?;
            Ok(())
        })
        .unwrap();
    let row = store.inbox(&q("primary")).unwrap().into_iter().find(|r| r.message_id == "m000").unwrap();
    assert_eq!(row.label_names, ["Alpha", "Receipts"]);
    assert_eq!((row.attachment_count, row.thread_count, row.unread, row.pinned), (1, 2, true, false));
    let by_label = store.inbox(&InboxQuery { tab_id: "primary".into(), account_id: None, label_id: Some("l1".into()), mailbox: None, account_ids: vec![], filters: vec![], smart_folder_id: None }).unwrap();
    assert_eq!(ids(&by_label), ["m000"]);
}

#[test]
fn thread_snoozed_and_zero_state_reads() {
    let db = TempDb::new("thread");
    let store = small_store(&db, 6);
    let t = store.thread("t000").unwrap().unwrap();
    assert_eq!(t.message_ids, ["m000", "m001"]);
    assert_eq!(t.participants, ["sender0@ext.test", "sender1@ext.test"]);
    assert_eq!(t.account_id, "acc-a");
    assert!(store.thread("nope").unwrap().is_none());
    assert_eq!(store.next_snooze_return_ms().unwrap(), None);
    store.snooze(vec!["m002".into()], 5_000, 1).unwrap();
    store.snooze(vec!["m001".into()], 3_000, 1).unwrap();
    let s = store.snoozed().unwrap();
    assert_eq!(s.iter().map(|x| (x.row.message_id.as_str(), x.return_at_ms)).collect::<Vec<_>>(), [("m001", 3_000), ("m002", 5_000)]);
    assert_eq!(store.next_snooze_return_ms().unwrap(), Some(3_000));
    store.snooze(vec!["m001".into()], 7_000, 2).unwrap(); // snoozing again moves the return time
    assert_eq!(store.next_snooze_return_ms().unwrap(), Some(5_000));
}

#[test]
fn many_threads_writing_at_once_never_see_a_locked_database() {
    let db = TempDb::new("hammer");
    let store = std::sync::Arc::new(small_store(&db, 40));
    let handles: Vec<_> = (0..8)
        .map(|t| {
            let s = store.clone();
            std::thread::spawn(move || {
                for i in 0..40 {
                    s.set_read(vec![format!("m{:03}", (i * 8 + t) % 40)], i % 2 == 0).expect("no write fails");
                    let _ = s.inbox(&q("primary")).expect("reads run beside writes");
                }
            })
        })
        .collect();
    for h in handles {
        h.join().unwrap();
    }
    assert_eq!(store.counts().unwrap(), store.recount().unwrap());
}
