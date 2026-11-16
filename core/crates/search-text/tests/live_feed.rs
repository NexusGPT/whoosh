//! The index follows the store: a write is searchable within INDEX_LAG_MS of the store write returning.
mod common;
use common::*;
use search_text::TextIndex;
use std::time::{Duration, Instant};

fn lag_limit() -> Duration {
    Duration::from_millis(constant("INDEX_LAG_MS"))
}

fn seeded(dir: &TempDir) -> (std::sync::Arc<store::Store>, TextIndex) {
    let store = open_store(dir);
    let batch = (0..300).map(|i| new_msg(msg(&format!("m{i:03}"), &format!("Subject {i}"), "Sender", "owner@example.test", 1_000 + i), Some(&format!("ordinary body number {i}")), None)).collect();
    insert(&store, batch);
    let index = TextIndex::open(&store, &dir.index()).unwrap();
    (store, index)
}

#[test]
fn a_new_message_is_searchable_within_the_index_lag() {
    let dir = TempDir::new("lag-new");
    let (store, index) = seeded(&dir);
    assert!(index.search("zyzzyva", 5).unwrap().is_empty());
    let start = Instant::now();
    insert(&store, vec![new_msg(msg("fresh", "Fresh arrival", "Sender", "owner@example.test", 9_000), Some("the zyzzyva has arrived"), None)]);
    let took = within(lag_limit(), || ids(&index.search("zyzzyva", 5).unwrap()) == vec!["fresh"]);
    let total = start.elapsed();
    println!("new message searchable {:?} after the write started (limit {:?})", total, lag_limit());
    assert!(took.is_some() && total <= lag_limit(), "not searchable within INDEX_LAG_MS: {total:?}; last error: {:?}", index.last_error());
}

#[test]
fn an_attachment_whose_text_arrives_later_makes_its_message_searchable_within_the_lag() {
    let dir = TempDir::new("lag-att");
    let (store, index) = seeded(&dir);
    assert!(index.search("quokka", 5).unwrap().is_empty());
    let start = Instant::now();
    add_attachment(&store, "att-late", "m010", "notes.pdf", Some("the quokka paragraph"));
    assert!(within(lag_limit(), || ids(&index.search("quokka", 5).unwrap()) == vec!["m010"]).is_some(), "attachment text not searchable within the lag: {:?}", start.elapsed());
}

#[test]
fn a_body_filled_in_after_the_header_is_searchable_within_the_lag() {
    let dir = TempDir::new("lag-body");
    let (store, index) = seeded(&dir);
    store
        .write(|ctx| {
            ctx.tx.execute("UPDATE message SET body_text = 'the wombat chapter' WHERE id = 'm299'", [])?;
            Ok(())
        })
        .unwrap();
    assert!(within(lag_limit(), || ids(&index.search("wombat", 5).unwrap()) == vec!["m299"]).is_some(), "body update not searchable within the lag");
    // The old text is gone with the old document: one document per message.
    assert!(index.search("\"ordinary body number 299\"", 5).unwrap().is_empty());
    assert_eq!(index.documents(), 300);
}

#[test]
fn a_deleted_message_leaves_the_results_within_the_lag() {
    let dir = TempDir::new("lag-delete");
    let (store, index) = seeded(&dir);
    assert_eq!(ids(&index.search("\"body number 42\"", 5).unwrap()), vec!["m042"]);
    store
        .write(|ctx| {
            ctx.tx.execute("DELETE FROM message WHERE id = 'm042'", [])?;
            Ok(())
        })
        .unwrap();
    // The results drop it at once (the table is the truth for every hit); the index itself catches up after.
    assert!(index.search("\"body number 42\"", 5).unwrap().is_empty(), "deleted message still in the results");
    assert!(within(lag_limit(), || index.documents() == 299).is_some(), "the document is dropped from the index within the lag");
}

#[test]
fn a_flag_write_does_not_change_the_documents() {
    let dir = TempDir::new("flag");
    let (store, index) = seeded(&dir);
    let before = index.commits();
    store.set_read(vec!["m001".into()], true).unwrap();
    std::thread::sleep(Duration::from_millis(300));
    assert_eq!(index.commits(), before, "a flag write is not a text change: no commit");
    assert_eq!(index.documents(), 300);
    assert_eq!(ids(&index.search("\"body number 1\"", 5).unwrap()), vec!["m001"]);
}

#[test]
fn a_message_that_reuses_a_deleted_top_rowid_is_still_indexed() {
    let dir = TempDir::new("reuse");
    let (store, index) = seeded(&dir);
    store
        .write(|ctx| {
            ctx.tx.execute("DELETE FROM message WHERE id = 'm299'", [])?;
            Ok(())
        })
        .unwrap();
    assert!(within(lag_limit(), || index.documents() == 299).is_some());
    insert(&store, vec![new_msg(msg("reborn", "Reborn", "Sender", "owner@example.test", 9_999), Some("the axolotl returns"), None)]);
    assert!(within(lag_limit(), || ids(&index.search("axolotl", 5).unwrap()) == vec!["reborn"]).is_some(), "SQLite reuses the freed rowid; the index must still see the new row");
    assert_eq!(index.documents(), 300);
}
