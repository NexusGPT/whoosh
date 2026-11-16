//! One write, one event per stream. Counted on a real store with real subscribers.
mod common;
use common::*;
use contracts::{CoreEvent, StreamKind};
use std::sync::{Arc, Mutex};

fn watch(store: &store::Store) -> Arc<Collector> {
    let c = Arc::new(Collector::default());
    for s in [StreamKind::Inbox, StreamKind::Counts, StreamKind::Thread] {
        store.subscribe(s, c.clone());
    }
    c
}

fn tally(events: &[CoreEvent]) -> (usize, usize, usize) {
    // delta 16: every inbox write also refreshes the All view; that one event is counted apart so the per-tab tallies read as before
    let inbox = events.iter().filter(|e| matches!(e, CoreEvent::InboxChanged { tab_id, .. } if tab_id != "all")).count();
    let counts = events.iter().filter(|e| matches!(e, CoreEvent::CountsChanged { .. })).count();
    let thread = events.iter().filter(|e| matches!(e, CoreEvent::ThreadChanged { .. })).count();
    (inbox, counts, thread)
}

#[test]
fn one_flag_write_emits_one_inbox_diff_and_one_counts_value() {
    let db = TempDb::new("flag");
    let store = small_store(&db, 12);
    store.counts().unwrap();
    let c = watch(&store);
    // m000 is in "primary" and unread.
    assert_eq!(store.set_read(vec!["m000".into()], true).unwrap(), 1);
    let events = c.take();
    assert_eq!(tally(&events), (1, 1, 0), "events: {events:?}");
    match &events[0] {
        CoreEvent::InboxChanged { tab_id, rows } => {
            assert_eq!(tab_id, "primary");
            let row = rows.iter().find(|r| r.message_id == "m000").expect("the changed row is in the page");
            assert!(!row.unread);
        }
        other => panic!("first event should be the inbox diff, got {other:?}"),
    }
    match events.iter().find(|e| matches!(e, CoreEvent::CountsChanged { .. })).unwrap() {
        CoreEvent::CountsChanged { counts } => {
            let primary = counts.tabs.iter().find(|t| t.tab_id == "primary").unwrap();
            let fresh = store.recount().unwrap();
            assert_eq!(counts, &fresh, "the counts value equals a recount");
            assert_eq!(primary.unread, fresh.tabs.iter().find(|t| t.tab_id == "primary").unwrap().unread);
        }
        _ => unreachable!(),
    }
}

#[test]
fn a_write_that_changes_nothing_emits_nothing() {
    let db = TempDb::new("noop");
    let store = small_store(&db, 6);
    store.set_read(vec!["m000".into()], true).unwrap();
    let c = watch(&store);
    assert_eq!(store.set_read(vec!["m000".into()], true).unwrap(), 0);
    assert!(c.take().is_empty());
}

#[test]
fn archive_pin_snooze_each_emit_once_per_stream() {
    let db = TempDb::new("each");
    let store = small_store(&db, 12);
    store.counts().unwrap();
    let c = watch(&store);
    store.set_archived(vec!["m000".into()], true).unwrap();
    assert_eq!(tally(&c.take()), (1, 1, 0), "archive");
    store.set_pinned("m006".into(), true, 5).unwrap();
    assert_eq!(tally(&c.take()), (1, 1, 0), "pin (the Pinned mailbox count moves)");
    store.snooze(vec!["m001".into()], 9_000_000_000_000, 5).unwrap();
    assert_eq!(tally(&c.take()), (1, 1, 0), "snooze");
    store.unsnooze(vec!["m001".into()]).unwrap();
    assert_eq!(tally(&c.take()), (1, 1, 0), "unsnooze");
}

#[test]
fn inserting_a_batch_emits_one_inbox_per_affected_tab_one_counts_one_thread_per_thread() {
    let db = TempDb::new("insert");
    let store = small_store(&db, 6);
    store.counts().unwrap();
    let c = watch(&store);
    // Two new messages in one NEW thread, one tab: one inbox event, one counts, one thread.
    store
        .insert_messages(vec![
            new_msg(msg("n1", "tnew", "acc-a", "x@ext.test", 1_800_000_000_000), Some("primary")),
            new_msg(msg("n2", "tnew", "acc-a", "y@ext.test", 1_800_000_001_000), Some("primary")),
        ])
        .unwrap();
    assert_eq!(tally(&c.take()), (1, 1, 1));
    // A reply into t000, whose two messages sit in two tabs (primary: m000, needs-reply: m001):
    // both tabs show that thread's size, so both tabs get exactly one inbox event.
    store.insert_messages(vec![new_msg(msg("n3", "t000", "acc-a", "x@ext.test", 1_810_000_000_000), Some("primary"))]).unwrap();
    let events = c.take();
    assert_eq!(tally(&events), (2, 1, 1), "{events:?}");
    let mut tabs: Vec<String> = events.iter().filter_map(|e| if let CoreEvent::InboxChanged { tab_id, .. } = e { Some(tab_id.clone()) } else { None }).collect();
    tabs.sort();
    assert_eq!(tabs, ["all", "needs-reply", "primary"], "both tabs of the thread, and the All view (delta 16)");
}

#[test]
fn a_failed_write_undoes_everything_and_emits_nothing() {
    let db = TempDb::new("fail");
    let store = small_store(&db, 6);
    store.counts().unwrap();
    let c = watch(&store);
    let r = store.write(|ctx| {
        ctx.will_change(&["m000".to_string()])?;
        ctx.tx.execute("UPDATE message SET read = 1 WHERE id = 'm000'", [])?;
        ctx.tx.execute("INSERT INTO pin (message_id, pinned_at_ms) VALUES ('no-such-message', 1)", [])?; // foreign key refuses
        Ok(())
    });
    assert!(r.is_err(), "the orphan pin is refused");
    assert!(c.take().is_empty());
    assert_eq!(store.counts().unwrap(), store.recount().unwrap());
    let unread: i64 = store.write(|ctx| Ok(ctx.tx.query_row("SELECT read FROM message WHERE id = 'm000'", [], |r| r.get(0))?)).unwrap();
    assert_eq!(unread, 0, "the read flag was rolled back");
}

#[test]
fn the_table_feed_has_one_change_per_touched_table() {
    let db = TempDb::new("tables");
    let store = small_store(&db, 6);
    let seen: Arc<Mutex<Vec<store::TableChange>>> = Arc::default();
    let sink = seen.clone();
    store.subscribe_tables(Arc::new(move |c: &store::TableChange| sink.lock().unwrap().push(c.clone())));
    store.set_read(vec!["m000".into(), "m002".into()], true).unwrap();
    let got = seen.lock().unwrap().clone();
    assert_eq!(got.len(), 1, "{got:?}");
    assert_eq!((got[0].table.as_str(), got[0].updates, got[0].inserts, got[0].deletes), ("message", 2, 0, 0));
}

#[test]
fn a_write_that_does_not_say_what_it_changed_still_leaves_exact_counts() {
    let db = TempDb::new("undeclared");
    let store = small_store(&db, 12);
    store.counts().unwrap();
    store.write(|ctx| Ok(ctx.tx.execute("UPDATE message SET read = 1", [])?)).unwrap(); // no will_change
    assert_eq!(store.counts().unwrap(), store.recount().unwrap());
    assert!(store.counts().unwrap().tabs.iter().all(|t| t.unread == 0));
}
