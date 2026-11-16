//! The index is a cache of the message table: lose it and it comes back; close it and it catches up.
mod common;
use common::*;
use search_text::TextIndex;

fn seed(store: &store::Store) {
    let mut batch: Vec<_> = (0..500).map(|i| new_msg(msg(&format!("m{i:03}"), &format!("Subject {i}"), "Sender", "owner@example.test", 1_000 + i), Some(&format!("filler text {i}")), None)).collect();
    batch.push(new_msg(msg("holder", "Wire", "Billing", "owner@example.test", 5_000), Some("wire exactly 99,100.25 today"), None));
    insert(store, batch);
    add_attachment(store, "att-r", "m007", "scan.pdf", Some("the marmoset page"));
}

fn manifest(path: &std::path::Path) -> (String, i64) {
    let c = rusqlite::Connection::open(path).unwrap();
    c.query_row("SELECT folder, documents FROM index_manifest WHERE index_kind = 'text'", [], |r| Ok((r.get(0)?, r.get(1)?))).unwrap()
}

#[test]
fn a_deleted_index_folder_is_rebuilt_from_the_message_table() {
    let dir = TempDir::new("rebuild");
    let store = open_store(&dir);
    seed(&store);
    let first = TextIndex::open(&store, &dir.index()).unwrap();
    assert!(first.was_rebuilt(), "no folder yet: built from the table");
    assert_eq!(first.documents(), 501);
    drop(first);
    std::fs::remove_dir_all(dir.index()).unwrap();
    let again = TextIndex::open(&store, &dir.index()).unwrap();
    assert!(again.was_rebuilt(), "folder gone: rebuilt");
    assert_eq!(again.documents(), 501);
    assert_eq!(ids(&again.search("\"99,100.25\"", 5).unwrap()), vec!["holder"]);
    assert_eq!(ids(&again.search("marmoset", 5).unwrap()), vec!["m007"], "attachment text comes back with the rebuild");
    let (folder, documents) = manifest(&dir.db());
    assert_eq!(folder, dir.index().to_string_lossy());
    assert_eq!(documents, 501, "index_manifest records the folder and the document count");
}

#[test]
fn a_damaged_index_is_rebuilt_not_trusted() {
    let dir = TempDir::new("damaged");
    let store = open_store(&dir);
    seed(&store);
    drop(TextIndex::open(&store, &dir.index()).unwrap());
    std::fs::write(dir.index().join("meta.json"), b"{ not json").unwrap();
    let again = TextIndex::open(&store, &dir.index()).unwrap();
    assert!(again.was_rebuilt());
    assert_eq!(again.documents(), 501);
}

#[test]
fn a_closed_index_catches_up_on_reopen_without_a_rebuild() {
    let dir = TempDir::new("catchup");
    let store = open_store(&dir);
    seed(&store);
    drop(TextIndex::open(&store, &dir.index()).unwrap());
    // While the index is closed: new mail, a late attachment, a deletion.
    insert(&store, vec![new_msg(msg("later", "Later", "Sender", "owner@example.test", 9_000), Some("the pangolin walked in"), None)]);
    add_attachment(&store, "att-c", "m008", "late.pdf", Some("the lemur appendix"));
    store
        .write(|ctx| {
            ctx.tx.execute("DELETE FROM message WHERE id = 'm009'", [])?;
            Ok(())
        })
        .unwrap();
    let again = TextIndex::open(&store, &dir.index()).unwrap();
    assert!(!again.was_rebuilt(), "a whole index is reused");
    assert!(within(std::time::Duration::from_secs(5), || again.documents() == 501 && !again.search("pangolin", 5).unwrap().is_empty() && !again.search("lemur", 5).unwrap().is_empty()).is_some());
    assert_eq!(ids(&again.search("pangolin", 5).unwrap()), vec!["later"]);
    assert_eq!(ids(&again.search("lemur", 5).unwrap()), vec!["m008"]);
    assert!(again.search("\"filler text 9\"", 5).unwrap().iter().all(|h| h.message_id != "m009"), "the deleted message is gone");
}
