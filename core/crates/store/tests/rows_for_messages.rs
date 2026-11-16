//! rows-engine (SPEC A141): `Store::rows_for_messages(ids)`, the list row of each given message id whatever folder holds it, so
//! the search screen can draw every hit (the inbox read gives only the first INBOX_PAGE_ROWS rows of one tab).
//! Each test names, in its comment, the one-line broken copy of core/crates/store/src/read.rs that turns it red.
//! Failure messages name message ids and field names only, never a subject, a preview or any other mail text.
mod common;
use common::*;
use contracts::{DecidedBy, InboxQuery, InboxRow};
use std::time::Instant;
use store::read::ROWS_FOR_MESSAGES_CHUNK;
use store::{Store, INBOX_PAGE_ROWS};

/// The owner's rule: "The objective is <50ms for any operation".
const READ_BUDGET_MS: f64 = 50.0;

fn ids_of(rows: &[InboxRow]) -> Vec<String> { rows.iter().map(|r| r.message_id.clone()).collect() }
fn owned(ids: &[&str]) -> Vec<String> { ids.iter().map(|s| s.to_string()).collect() }
fn all_tab() -> InboxQuery {
    InboxQuery { tab_id: "all".into(), account_id: None, label_id: None, mailbox: None, account_ids: vec![], filters: vec![], smart_folder_id: None }
}

/// One message of acc-a in `folder`, in thread `thread`.
fn put(s: &Store, id: &str, thread: &str, folder: &str, archived: bool, at: i64) {
    let mut m = msg(id, thread, "acc-a", "someone@ext.test", at);
    m.folder = folder.into();
    m.archived = archived;
    s.insert_messages(vec![new_msg(m, Some("primary"))]).unwrap();
}

fn sql(s: &Store, text: &'static str) { s.write(move |c| { c.tx.execute_batch(text)?; Ok(()) }).unwrap(); }

/// The ids come back in the order given, not in date or id order.
/// Broken copy (red): the last line of `rows_for_messages` returns `Ok(found.into_values().collect())` (the hash map's order).
#[test]
fn rows_come_in_the_order_of_the_ids_given() {
    let db = TempDb::new("rfm-order");
    let s = small_store(&db, 30);
    let mut asked: Vec<String> = (0..30).map(|i| format!("m{i:03}")).collect();
    asked.reverse();
    asked.swap(3, 17);
    let rows = s.rows_for_messages(&asked).unwrap();
    assert_eq!(ids_of(&rows), asked, "one row per id, in the order given");
}

/// An id that names no stored message is skipped; the rows after it still come.
/// Broken copy (red): the last line of `rows_for_messages` uses `.map_while(|id| found.remove(id.as_str()))` instead of `.filter_map(...)`.
#[test]
fn an_unknown_id_is_skipped_and_the_rest_still_come() {
    let db = TempDb::new("rfm-unknown");
    let s = small_store(&db, 6);
    let rows = s.rows_for_messages(&owned(&["m004", "no-such-id", "m001", "", "m005"])).unwrap();
    assert_eq!(ids_of(&rows), owned(&["m004", "m001", "m005"]));
    assert!(s.rows_for_messages(&owned(&["no-such-id"])).unwrap().is_empty(), "only unknown ids: no row");
    assert!(s.rows_for_messages(&[]).unwrap().is_empty(), "no id: no row, no error");
}

/// A mail in Sent gets its own row, with its thread's id and the count of its thread, its pin, its labels sorted by name; and the
/// row of an inbox mail is the row the inbox read serves for it, field for field.
/// Broken copy (red): the row statement of `rows_for_messages` reads `... FROM message m WHERE m.folder = 'INBOX' AND m.id IN ({marks})`.
#[test]
fn a_mail_in_sent_gets_its_own_row() {
    let db = TempDb::new("rfm-sent");
    let s = small_store(&db, 4);
    // t000 holds m000 and m001; the Sent reply joins it
    put(&s, "s-sent", "t000", "Sent", false, 1_700_000_900_000);
    sql(&s, "INSERT INTO pin (message_id, pinned_at_ms) VALUES ('s-sent', 5);");
    let beta = s.label_ensure_path("Beta".into(), vec![], 1).unwrap().label.id;
    let alpha = s.label_ensure_path("Alpha".into(), vec![], 1).unwrap().label.id;
    s.label_file(beta, "s-sent".into(), DecidedBy::Owner, None, 2).unwrap();
    s.label_file(alpha, "s-sent".into(), DecidedBy::Owner, None, 2).unwrap();

    let rows = s.rows_for_messages(&owned(&["s-sent", "m001"])).unwrap();
    assert_eq!(ids_of(&rows), owned(&["s-sent", "m001"]), "the Sent mail has its row");
    let r = &rows[0];
    assert_eq!((r.thread_id.as_str(), r.thread_count, r.account_id.as_str()), ("t000", 3, "acc-a"), "its own thread and the thread's count");
    assert_eq!((r.received_at_ms, r.unread, r.pinned), (1_700_000_900_000, true, true), "its date, unread, pinned");
    assert_eq!(r.sender_address, "someone@ext.test", "its sender's address");
    assert_eq!(r.label_names, owned(&["Alpha", "Beta"]), "its labels, sorted by name as every list sorts them");

    // the inbox mail: the same row as the inbox read's (one builder for both)
    let page = s.inbox(&all_tab()).unwrap();
    let from_inbox = page.iter().find(|x| x.message_id == "m001").expect("m001 is on the inbox page");
    assert!(&rows[1] == from_inbox, "m001's row equals its inbox row field for field");
}

/// An archived mail (in the inbox folder with archived = 1, or in an Archive folder) gets its row.
/// Broken copy (red): the row statement of `rows_for_messages` reads `... FROM message m WHERE m.archived = 0 AND m.id IN ({marks})`.
#[test]
fn an_archived_mail_gets_its_row() {
    let db = TempDb::new("rfm-archived");
    let s = small_store(&db, 4);
    put(&s, "a-flag", "t-a1", "INBOX", true, 1_700_000_800_000);
    put(&s, "a-folder", "t-a2", "Archive", true, 1_700_000_700_000);
    let rows = s.rows_for_messages(&owned(&["a-flag", "m002", "a-folder"])).unwrap();
    assert_eq!(ids_of(&rows), owned(&["a-flag", "m002", "a-folder"]));
    assert!(!s.inbox(&all_tab()).unwrap().iter().any(|r| r.message_id == "a-flag"), "premise: the inbox read does not list it");
}

/// A mail beyond the first INBOX_PAGE_ROWS rows of the inbox gets its row, and so does every one of more than a page of ids.
/// Broken copy (red): the `wanted` line of `rows_for_messages` ends `.filter(|id| seen.insert(id.as_str())).take(INBOX_PAGE_ROWS).collect();`.
#[test]
fn a_mail_beyond_the_first_page_gets_its_row() {
    let db = TempDb::new("rfm-page");
    let n = INBOX_PAGE_ROWS + 10;
    let s = small_store(&db, n);
    let page = s.inbox(&all_tab()).unwrap();
    assert_eq!(page.len(), INBOX_PAGE_ROWS, "premise: the inbox read stops at a page");
    assert!(!page.iter().any(|r| r.message_id == "m000"), "premise: the oldest mail is beyond the first page");
    let newest_first: Vec<String> = (0..n).rev().map(|i| format!("m{i:03}")).collect();
    let rows = s.rows_for_messages(&newest_first).unwrap();
    assert_eq!(rows.len(), n, "every id has its row, not a page of them");
    assert_eq!(rows.last().map(|r| r.message_id.as_str()), Some("m000"), "the oldest mail, beyond the page, has its row");
}

/// A snoozed mail and a mail in a custom folder get their rows: no inbox filter applies.
/// Broken copy (red): the row statement of `rows_for_messages` reads `... FROM message m WHERE {VISIBLE} AND m.id IN ({marks})` (VISIBLE = the inbox's filter).
#[test]
fn a_snoozed_mail_and_a_mail_in_a_custom_folder_get_their_rows() {
    let db = TempDb::new("rfm-folders");
    let s = small_store(&db, 4);
    put(&s, "c-custom", "t-c1", "Clients/2026", false, 1_700_000_600_000);
    sql(&s, "INSERT INTO snooze (message_id, return_at_ms, updated_at_ms) VALUES ('m003', 9000000000000, 0);");
    let rows = s.rows_for_messages(&owned(&["c-custom", "m003"])).unwrap();
    assert_eq!(ids_of(&rows), owned(&["c-custom", "m003"]));
}

/// A repeated id gives one row, at its first place.
/// Broken copy (red): the last line of `rows_for_messages` uses `found.get(id.as_str()).cloned()` instead of `found.remove(id.as_str())`.
#[test]
fn a_repeated_id_gives_one_row_at_its_first_place() {
    let db = TempDb::new("rfm-repeat");
    let s = small_store(&db, 6);
    let rows = s.rows_for_messages(&owned(&["m002", "m005", "m002", "m001", "m005", "m002"])).unwrap();
    assert_eq!(ids_of(&rows), owned(&["m002", "m005", "m001"]));
}

/// 1000 ids, more than one statement's chunk: every row comes, in order, a repeat across the chunk border once, an unknown id
/// in the second chunk skipped, the labels of a mail in the second chunk read.
/// Broken copy (red): the loop of `rows_for_messages` reads `for chunk in wanted.chunks(ROWS_FOR_MESSAGES_CHUNK).take(1) {`.
#[test]
fn a_thousand_ids_over_the_chunk_limit_all_come_back() {
    let db = TempDb::new("rfm-chunks");
    let s = small_store(&db, 1000);
    let mut asked: Vec<String> = (0..1000).rev().map(|i| format!("m{i:03}")).collect();
    assert!(asked.len() > ROWS_FOR_MESSAGES_CHUNK, "premise: more ids than one chunk ({ROWS_FOR_MESSAGES_CHUNK})");
    asked.insert(700, "no-such-id".into());
    asked.insert(800, asked[0].clone());
    let late = asked[900].clone();
    let l = s.label_ensure_path("Late".into(), vec![], 1).unwrap().label.id;
    s.label_file(l, late.clone(), DecidedBy::Owner, None, 2).unwrap();

    let rows = s.rows_for_messages(&asked).unwrap();
    let want: Vec<String> = (0..1000).rev().map(|i| format!("m{i:03}")).collect();
    assert_eq!(rows.len(), 1000, "one row per stored id");
    assert!(ids_of(&rows) == want, "every row, in the order given");
    let r = rows.iter().find(|r| r.message_id == late).unwrap();
    assert_eq!(r.label_names, owned(&["Late"]), "a mail in the second chunk has its labels");
}

/// The time of a 1000-id read on a store of 4000 messages, printed; red above READ_BUDGET_MS. The first read is timed (no warm
/// statement cache), with the rows of every id checked.
/// Broken copy (red): in ROW_COLUMNS the thread count reads `(SELECT count(*) FROM message t WHERE t.thread_id || '' = m.thread_id)`
/// (no index: every row scans the 4000 messages).
#[test]
fn a_thousand_id_read_on_four_thousand_messages_is_under_fifty_ms() {
    let db = TempDb::new("rfm-time");
    let total = 4000;
    let s = small_store(&db, total);
    let asked: Vec<String> = (0..1000).map(|k| format!("m{:03}", (k * 7919) % total)).collect();
    let t = Instant::now();
    let rows = s.rows_for_messages(&asked).unwrap();
    let ms = t.elapsed().as_secs_f64() * 1000.0;
    println!("rows_for_messages: {} ids, {} rows, store of {total} messages: {ms:.2} ms (budget {READ_BUDGET_MS} ms)", asked.len(), rows.len());
    assert_eq!(rows.len(), 1000, "every id names a stored message");
    assert!(ms < READ_BUDGET_MS, "a 1000-id read took {ms:.2} ms, over {READ_BUDGET_MS} ms");
}
