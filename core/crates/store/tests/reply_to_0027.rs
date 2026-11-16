//! real-replyto (migration 0027): the store keeps each mail's Reply-To (every address and name of the header, in order): the read
//! (`Store::reply_to`), the writes (`set_reply_to_quiet`, the arrival's `WriteCtx::set_reply_to`), the contract's `Message.reply_to_name`
//! served from it, a restart, and the time on 2,000 mails. Not run by this lane (no cargo here): the lead runs `cargo test --release -p store`.
//! RED-FIRST (one-line broken copy of store/src/reply_to.rs per test):
//!   R1 `read_reply_to` ends `ORDER BY j.key DESC`                                                 -> several_addresses_come_back_with_their_names_in_header_order
//!   R3 `write_reply_to`'s loop writes nothing (`changed += 0`)                                    -> several_addresses_come_back_with_their_names_in_header_order
//!   R4 `COALESCE(m.reply_to_json, '[]')` -> `COALESCE(m.reply_to_json, '[{"name":"","address":"x@x"}]')` -> a_mail_not_read_yet_and_a_mail_with_none_both_read_as_no_reply_to
//!   R5 `first_reply_to_name` returns the whole name (`.map(|r| r.name)`)                          -> the_contracts_reply_to_name_is_the_first_names_first_word
//!   R6 `set_reply_to_quiet`'s body becomes `self.write(move |ctx| write_reply_to(ctx.tx, &rows))` (the publishing write, inserted as a first `if true { return ... }` line) -> the_quiet_write_publishes_nothing
//!   R7 `WriteCtx::set_reply_to` writes nothing (`Ok(0)`)                                           -> the_arrival_write_sits_in_the_insert_transaction_and_a_rollback_drops_it
//!   R8 `encode` keeps an entry with no address (`.filter(|_| true)`)                        -> an_entry_with_no_address_is_not_kept
use std::sync::Arc;
use std::time::Instant;

use contracts::StreamKind;
use store::{ReplyTo, Store};

mod common;
use common::{msg, new_msg, small_store, Collector, TempDb};

fn rt(name: &str, address: &str) -> ReplyTo { ReplyTo { name: name.into(), address: address.into() } }

#[test]
fn several_addresses_come_back_with_their_names_in_header_order() {
    let db = TempDb::new("rt-order");
    let store = small_store(&db, 4);
    let items = vec![rt("Dev List", "dev@lists.example.test"), rt("", "bare@example.test"), rt("Zoe \"Z\" Q", "zoe@example.test")];
    assert_eq!(store.set_reply_to_quiet(vec![("m001".into(), items.clone())]).unwrap(), 1);
    assert_eq!(store.reply_to("m001").unwrap(), items, "every address and name, in order (a quote in a name survives)");
    assert_eq!(store.set_reply_to_quiet(vec![("m001".into(), items.clone())]).unwrap(), 0, "the same value is no change");
    assert_eq!(store.set_reply_to_quiet(vec![("m001".into(), vec![rt("", "only@example.test")])]).unwrap(), 1, "a later read replaces it");
    assert_eq!(store.reply_to("m001").unwrap(), vec![rt("", "only@example.test")]);
    assert_eq!(store.reply_to("m002").unwrap(), vec![], "another mail is untouched");
}

#[test]
fn a_mail_not_read_yet_and_a_mail_with_none_both_read_as_no_reply_to() {
    let db = TempDb::new("rt-none");
    let store = small_store(&db, 4);
    assert_eq!(store.reply_to("m000").unwrap(), vec![], "stored before 0027: NULL reads as none");
    assert_eq!(store.reply_to("no-such-mail").unwrap(), vec![], "not stored: none");
    store.set_reply_to_quiet(vec![("m000".into(), vec![])]).unwrap();
    assert_eq!(store.reply_to("m000").unwrap(), vec![]);
    let raw = |id: &str| { let id = id.to_string(); store.write(move |c| Ok(c.tx.query_row("SELECT reply_to_json FROM message WHERE id = ?1", [&id], |r| r.get::<_, Option<String>>(0))?)).unwrap() };
    assert_eq!(raw("m000").as_deref(), Some("[]"), "read, and none: '[]'");
    assert_eq!(raw("m001"), None, "never read: NULL");
}

#[test]
fn an_entry_with_no_address_is_not_kept() {
    let db = TempDb::new("rt-blank");
    let store = small_store(&db, 2);
    store.set_reply_to_quiet(vec![("m000".into(), vec![rt("Nobody", " "), rt("Ann", "ann@example.test")])]).unwrap();
    assert_eq!(store.reply_to("m000").unwrap(), vec![rt("Ann", "ann@example.test")]);
    // the read drops a blank address too, so the read alone cannot show the write kept it out: the column itself holds only Ann
    let raw: Option<String> = store.write(|c| Ok(c.tx.query_row("SELECT reply_to_json FROM message WHERE id = 'm000'", [], |r| r.get::<_, Option<String>>(0))?)).unwrap();
    assert_eq!(raw.as_deref(), Some(r#"[{"name":"Ann","address":"ann@example.test"}]"#), "the entry with no address never reached the column");
}

#[test]
fn the_contracts_reply_to_name_is_the_first_names_first_word() {
    let db = TempDb::new("rt-name");
    let store = small_store(&db, 4);
    assert_eq!(store.message("m000").unwrap().unwrap().reply_to_name, None, "no Reply-To: none");
    store.set_reply_to_quiet(vec![("m000".into(), vec![rt("Karen Bosch", "karen@example.test"), rt("Other", "o@example.test")]), ("m001".into(), vec![rt("", "x@example.test")])]).unwrap();
    assert_eq!(store.message("m000").unwrap().unwrap().reply_to_name.as_deref(), Some("Karen"), "the first entry's first word");
    assert_eq!(store.message("m001").unwrap().unwrap().reply_to_name, None, "a Reply-To with no name: none");
}

#[test]
fn the_quiet_write_publishes_nothing() {
    let db = TempDb::new("rt-quiet");
    let store = small_store(&db, 4);
    let sink = Arc::new(Collector::default());
    store.subscribe(StreamKind::Inbox, sink.clone());
    sink.take();
    store.set_reply_to_quiet(vec![("m001".into(), vec![rt("A", "a@example.test")])]).unwrap();
    assert!(sink.take().is_empty(), "no list, count or thread shows this column");
}

#[test]
fn the_arrival_write_sits_in_the_insert_transaction_and_a_rollback_drops_it() {
    let db = TempDb::new("rt-arrival");
    let store = small_store(&db, 2);
    let fresh = new_msg(msg("fresh", "tf", "acc-a", "s@ext.test", 1_700_000_100_000), None);
    store.write(move |ctx| { ctx.insert_batch(&[fresh])?; ctx.set_reply_to(&[("fresh".into(), vec![rt("L", "l@example.test")])])?; Ok(()) }).unwrap();
    assert_eq!(store.reply_to("fresh").unwrap(), vec![rt("L", "l@example.test")]);
    let failed = store.write(|ctx| {
        ctx.set_reply_to(&[("m000".into(), vec![rt("Gone", "gone@example.test")])])?;
        Err::<(), _>(store::StoreError::Invalid("rolled back on purpose".into()))
    });
    assert!(failed.is_err());
    assert_eq!(store.reply_to("m000").unwrap(), vec![], "the failed transaction wrote nothing");
}

#[test]
fn the_reply_to_survives_a_restart() {
    let db = TempDb::new("rt-restart");
    {
        let store = small_store(&db, 2);
        store.set_reply_to_quiet(vec![("m000".into(), vec![rt("Karen Bosch", "karen@example.test")])]).unwrap();
    }
    let reopened = Store::open(&db.0).unwrap();
    assert_eq!(reopened.reply_to("m000").unwrap(), vec![rt("Karen Bosch", "karen@example.test")]);
}

#[test]
fn the_read_and_the_write_answer_in_under_50_ms_on_2000_mails() {
    let db = TempDb::new("rt-time");
    let store = small_store(&db, 2000);
    let rows: Vec<(String, Vec<ReplyTo>)> = (0..2000).map(|i| (format!("m{i:03}"), vec![rt("List", &format!("list{i}@example.test")), rt("", "b@example.test")])).collect();
    let t = Instant::now();
    store.set_reply_to_quiet(rows).unwrap();
    let write_ms = t.elapsed().as_millis();
    let t = Instant::now();
    let got = store.reply_to("m1999").unwrap();
    let read_ms = t.elapsed().as_millis();
    let t = Instant::now();
    let m = store.message("m1999").unwrap().unwrap();
    let message_ms = t.elapsed().as_millis();
    println!("real-replyto store timing on 2000 mails: write of all 2000 {write_ms} ms, reply_to {read_ms} ms, message {message_ms} ms");
    assert_eq!(got.len(), 2);
    assert_eq!(m.reply_to_name.as_deref(), Some("List"));
    assert!(read_ms < 50 && message_ms < 50, "a read is under 50 ms (reply_to {read_ms} ms, message {message_ms} ms)");
    assert!(write_ms < 500, "even the write of all 2000 rows at once is under half a second ({write_ms} ms)");
}
