//! lane reply-headers (SPEC A144, migration 0023): the store keeps each mail's Message-ID and References (store/src/thread_headers.rs).
//! The three doors the brief names: the headers of one message (`thread_headers`, and the reply's `newest_thread_headers`), the fill-in
//! pass's list of unknown rows (`thread_headers_unknown`), the write of many (`set_thread_headers_quiet`, and the arrival's
//! `WriteCtx::set_thread_headers`). Not run by this lane (no cargo here): the lead runs `cargo test -p store`.
//! RED-FIRST (one-line broken copy of store/src/thread_headers.rs per test):
//!   T1 drop `AND rfc_message_id IS NULL` from the `thread_headers_unknown` SELECT            -> the_unknown_rows_are_the_placed_ones_newest_first_limited
//!   T2 `mark` writes `SET rfc_message_id = ''` with no `AND rfc_message_id IS NULL`          -> a_known_value_is_kept_when_the_server_gives_none_and_replaced_when_known
//!   T3 `newest_in_thread` orders `received_at_ms ASC`                                         -> the_reply_parent_is_the_newest_message_of_the_thread_with_stored_headers
//!   T4 `chain_of` reads `ORDER BY j.key DESC`                                                 -> the_arrival_write_sits_in_the_insert_transaction_and_keeps_the_chain_order
//!   T5 `set_thread_headers_quiet`'s body becomes `self.write(move |ctx| write_headers(ctx.tx, &rows))` (the publishing write) -> the_fill_in_write_publishes_nothing
use std::sync::Arc;

use contracts::StreamKind;
use store::{Store, ThreadHeaders};

mod common;
use common::{msg, new_msg, small_store, Collector, TempDb};

fn with_uid(store: &Store, id: &str, uid: &str) {
    let (id, uid) = (id.to_string(), uid.to_string());
    store.write(move |c| Ok(c.tx.execute("UPDATE message SET remote_id = ?2 WHERE id = ?1", rusqlite::params![id, uid])?)).unwrap();
}
fn raw_mid(store: &Store, id: &str) -> Option<String> {
    let id = id.to_string();
    store.write(move |c| Ok(c.tx.query_row("SELECT rfc_message_id FROM message WHERE id = ?1", [&id], |r| r.get::<_, Option<String>>(0))?)).unwrap()
}
fn h(mid: &str, chain: &[&str]) -> ThreadHeaders {
    ThreadHeaders { message_id: mid.to_string(), references: chain.iter().map(|s| s.to_string()).collect() }
}

#[test]
fn the_unknown_rows_are_the_placed_ones_newest_first_limited() {
    let db = TempDb::new("hdr-unknown");
    // acc-a holds m000, m002, m004 (newer as the number grows), all in INBOX, none with a server id yet
    let store = small_store(&db, 6);
    assert_eq!(store.thread_headers_unknown("acc-a", "INBOX", 10).unwrap(), vec![], "a row with no server place is never asked for");
    with_uid(&store, "m000", "10");
    with_uid(&store, "m002", "12");
    with_uid(&store, "m004", "not-a-uid");
    assert_eq!(store.thread_headers_unknown("acc-a", "INBOX", 10).unwrap(), vec![(12, "m002".to_string()), (10, "m000".to_string())]);
    assert_eq!(store.thread_headers_unknown("acc-a", "INBOX", 1).unwrap(), vec![(12, "m002".to_string())], "at most the limit, the newest first");
    assert_eq!(store.thread_headers_unknown("acc-a", "Archive", 10).unwrap(), vec![], "one folder only");
    assert_eq!(store.thread_headers_unknown("acc-b", "INBOX", 10).unwrap(), vec![], "one account only");
    // the newest gets its headers, the other is marked as not answered: neither is listed again
    let changed = store.set_thread_headers_quiet(vec![("m002".into(), Some(h("<m2@x.test>", &[]))), ("m000".into(), None)]).unwrap();
    assert_eq!(changed, 2);
    assert_eq!(store.thread_headers_unknown("acc-a", "INBOX", 10).unwrap(), vec![], "a known or marked row is not asked again");
    assert_eq!(raw_mid(&store, "m000").as_deref(), Some(""), "not answered: the empty mark");
    assert_eq!(store.thread_headers("m000").unwrap(), None, "the mark is not a Message-ID");
}

#[test]
fn a_known_value_is_kept_when_the_server_gives_none_and_replaced_when_known() {
    let db = TempDb::new("hdr-keep");
    let store = small_store(&db, 4);
    let first = h("<m1@x.test>", &["<root@x.test>"]);
    assert_eq!(store.set_thread_headers_quiet(vec![("m001".into(), Some(first.clone()))]).unwrap(), 1);
    assert_eq!(store.set_thread_headers_quiet(vec![("m001".into(), None)]).unwrap(), 0, "an unknown answer changes nothing");
    assert_eq!(store.thread_headers("m001").unwrap(), Some(first.clone()), "the row keeps what it held");
    assert_eq!(store.set_thread_headers_quiet(vec![("m001".into(), Some(first.clone()))]).unwrap(), 0, "the same value is no change");
    let second = h("<m1@x.test>", &["<root@x.test>", "<mid@x.test>"]);
    assert_eq!(store.set_thread_headers_quiet(vec![("m001".into(), Some(second.clone()))]).unwrap(), 1);
    assert_eq!(store.thread_headers("m001").unwrap(), Some(second), "a known value replaces the old one");
    assert_eq!(store.thread_headers("nope").unwrap(), None, "a message the store does not hold");
    assert_eq!(store.set_thread_headers_quiet(vec![("nope".into(), Some(first))]).unwrap(), 0, "no row is made for it");
}

#[test]
fn the_reply_parent_is_the_newest_message_of_the_thread_with_stored_headers() {
    let db = TempDb::new("hdr-parent");
    // thread t001 = m002 (older), m003 (newer)
    let store = small_store(&db, 4);
    assert_eq!(store.newest_thread_headers("t001").unwrap(), None, "no stored headers: the caller asks the server");
    store.set_thread_headers_quiet(vec![("m002".into(), Some(h("<m2@x.test>", &["<root@x.test>"])))]).unwrap();
    assert_eq!(store.newest_thread_headers("t001").unwrap(), Some(("m002".to_string(), h("<m2@x.test>", &["<root@x.test>"]))), "the only one with headers");
    store.set_thread_headers_quiet(vec![("m003".into(), Some(h("<m3@x.test>", &["<root@x.test>", "<m2@x.test>"])))]).unwrap();
    assert_eq!(store.newest_thread_headers("t001").unwrap(), Some(("m003".to_string(), h("<m3@x.test>", &["<root@x.test>", "<m2@x.test>"]))), "the newest wins");
    assert_eq!(store.newest_thread_headers("t000").unwrap(), None, "another thread is not read");
}

#[test]
fn the_arrival_write_sits_in_the_insert_transaction_and_keeps_the_chain_order() {
    let db = TempDb::new("hdr-arrival");
    let store = small_store(&db, 0);
    let chain = h("<new@x.test>", &["<a@x.test>", "<b@x.test>", "<c@x.test>"]);
    let row = new_msg(msg("n1", "tn", "acc-a", "s@ext.test", 1_700_000_100_000), Some("primary"));
    let kept = chain.clone();
    store.write(move |ctx| { ctx.insert_batch(&[row])?; ctx.set_thread_headers(&[("n1".to_string(), kept)]) }).unwrap();
    assert_eq!(store.thread_headers("n1").unwrap(), Some(chain), "stored with the row, the chain oldest first");
}

#[test]
fn the_fill_in_write_publishes_nothing() {
    let db = TempDb::new("hdr-quiet");
    let store = small_store(&db, 2);
    let seen = Arc::new(Collector::default());
    for stream in [StreamKind::Inbox, StreamKind::Counts, StreamKind::Thread] { store.subscribe(stream, seen.clone()); }
    assert_eq!(store.set_thread_headers_quiet(vec![("m000".into(), Some(h("<m0@x.test>", &[])))]).unwrap(), 1);
    assert!(seen.take().is_empty(), "no list, count or thread is redrawn for a header the screens never show");
}
