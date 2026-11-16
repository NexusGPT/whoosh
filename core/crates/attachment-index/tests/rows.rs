use attachment_index::{AttachmentIndexEntry, AttachmentKind, AttachmentRows, EntryVector, IndexState, SqliteRows};
use rusqlite::Connection;

/// The store's real migrations, read from the store crate's folder (never copied): the table under test is the shipped one.
fn database() -> Connection {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../store/migrations/sql");
    let c = Connection::open_in_memory().unwrap();
    c.execute_batch("PRAGMA foreign_keys = ON;").unwrap();
    for f in ["0001_v1.sql", "0002_nested_labels.sql", "0003_account_setup_search.sql", "0004_labels_tabs_blocks.sql"] {
        c.execute_batch(&std::fs::read_to_string(dir.join(f)).unwrap()).unwrap_or_else(|e| panic!("{f}: {e}"));
    }
    c.execute_batch("PRAGMA foreign_keys = OFF;").unwrap();
    c.execute("INSERT INTO attachment (id, message_id, file_name, mime_type, size_bytes, local_path) VALUES ('a-photo', 'msg-s02', 'warehouse-photo.jpg', 'image/jpeg', 184320, '/x/warehouse-photo.jpg')", []).unwrap();
    c.execute("INSERT INTO attachment (id, message_id, file_name, mime_type, size_bytes, local_path) VALUES ('a-scan', 'msg-s06', 'scan-notes.pdf', 'application/pdf', 100, NULL)", []).unwrap();
    c.execute("INSERT INTO attachment (id, message_id, file_name, mime_type, size_bytes, local_path) VALUES ('a-dwg', 'msg-s07', 'site-plan.dwg', 'image/vnd.dwg', 100, '/x/site-plan.dwg')", []).unwrap();
    c
}

fn photo_entry() -> AttachmentIndexEntry {
    AttachmentIndexEntry {
        message_id: "msg-s02".into(),
        attachment_id: "a-photo".into(),
        name: "warehouse-photo.jpg".into(),
        kind: AttachmentKind::Image,
        state: IndexState::Indexed,
        text: None,
        ocr_text: Some("Q1 brochure \"12,000\"".into()),
        labels: vec!["forklift".into(), "pallets \"stacked\"".into()],
        transcript: None,
        vectors: vec![EntryVector { key: "msg-s02/a-photo".into(), vector: vec![0.5; 4] }],
        notes: vec![],
        updated_at_ms: 42,
    }
}

#[test]
fn saving_an_entry_fills_attachment_content_and_the_words_the_text_index_reads() {
    let rows = SqliteRows::new(database());
    rows.save(&photo_entry()).unwrap();
    let c = rows.load("a-photo").unwrap().expect("row");
    assert_eq!(c.ocr_text.as_deref(), Some("Q1 brochure \"12,000\""));
    assert_eq!(c.labels, vec!["forklift", "pallets \"stacked\""]);
    assert!(c.embedded);
    assert_eq!(c.vector_key.as_deref(), Some("msg-s02/a-photo"));
    let cat = rows.catalog(&["msg-s02".to_string()]).unwrap();
    assert_eq!(cat[0].state, IndexState::Indexed);
    assert!(cat[0].text.contains("forklift"), "attachment.extracted_text carries the words: {:?}", cat[0].text);
}

#[test]
fn saving_twice_replaces_and_never_duplicates() {
    let rows = SqliteRows::new(database());
    rows.save(&photo_entry()).unwrap();
    let mut e = photo_entry();
    e.labels = vec!["office".into()];
    e.updated_at_ms = 43;
    rows.save(&e).unwrap();
    let c = rows.load("a-photo").unwrap().unwrap();
    assert_eq!((c.labels, c.updated_at_ms), (vec!["office".to_string()], 43));
}

#[test]
fn the_catalog_tells_each_state_from_the_store() {
    let rows = SqliteRows::new(database());
    let ids: Vec<String> = ["msg-s02", "msg-s06", "msg-s07"].iter().map(|s| s.to_string()).collect();
    let cat = rows.catalog(&ids).unwrap();
    let state = |m: &str| cat.iter().find(|r| r.message_id == m).unwrap().state.clone();
    assert!(matches!(state("msg-s02"), IndexState::Failed { .. }), "downloaded, not indexed yet");
    assert_eq!(state("msg-s06"), IndexState::NotDownloaded);
    assert_eq!(state("msg-s07"), IndexState::Unreadable);
}

#[test]
fn an_entry_that_is_not_indexed_removes_a_stale_row() {
    let rows = SqliteRows::new(database());
    rows.save(&photo_entry()).unwrap();
    let mut e = photo_entry();
    e.state = IndexState::NotDownloaded;
    rows.save(&e).unwrap();
    assert_eq!(rows.load("a-photo").unwrap(), None);
}
