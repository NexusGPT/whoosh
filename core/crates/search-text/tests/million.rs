//! The acceptance run: a generated 1M-message database (built here into a temp folder, never committed).
//! An exact quoted figure ranks the holding message first; a new message is searchable within INDEX_LAG_MS;
//! the index folder is built from the message table when it is missing.
mod common;
use common::*;
use rusqlite::Connection;
use search_text::TextIndex;
use std::sync::Arc;
use std::time::{Duration, Instant};

const MESSAGES: u64 = 1_000_000;
const HOLDER: &str = "m777777";

fn build_fixture(path: &std::path::Path) {
    let mut conn = Connection::open(path).unwrap();
    conn.pragma_update(None, "journal_mode", "OFF").unwrap();
    conn.pragma_update(None, "synchronous", "OFF").unwrap();
    store::migrations::migrate(&mut conn).unwrap();
    let tx = conn.transaction().unwrap();
    tx.execute_batch(&format!(
        "INSERT INTO account (id, address, provider, display_name, colour, sort_order, credential_kind, keychain_ref, updated_at_ms)
           VALUES ('acc-a', 'owner@example.test', 'imap', 'A', '#336699', 0, 'manual', 'svc' || char(31) || 'a', 0);
         WITH RECURSIVE s(n) AS (SELECT 0 UNION ALL SELECT n + 1 FROM s WHERE n < {m} / 4 - 1)
         INSERT INTO thread (id, account_id, subject, participants_json, last_message_at_ms) SELECT 't' || n, 'acc-a', 'Thread ' || n, '[]', 1700000000000 + n * 4000 FROM s;
         WITH RECURSIVE s(i) AS (SELECT 0 UNION ALL SELECT i + 1 FROM s WHERE i < {m} - 1)
         INSERT INTO message (id, thread_id, account_id, sender_name, from_address, to_json, subject, preview, body_text, received_at_ms, read, archived, folder)
         SELECT 'm' || i, 't' || (i / 4), 'acc-a', 'Sender ' || (i % 5000), 'sender' || (i % 5000) || '@ext.test', '[\"owner@example.test\"]',
                'Subject ' || (i % 100000), 'Preview ' || i,
                'tok' || (i * 7919 % 9973) || ' w' || (i * 104729 % 8191) || ' alpha' || (i % 13) || ' beta' || (i * 31 % 1009)
                  || ' delta' || (i * 17 % 4001) || ' sigma' || (i * 3 % 20011) || ' filler words for body ' || i,
                1700000000000 + i * 1000, CASE WHEN i % 5 = 0 THEN 0 ELSE 1 END, 0, 'INBOX' FROM s;",
        m = MESSAGES
    ))
    .unwrap();
    // The holder, and decoys that share its words: reversed, split, repeated many times, and a different figure.
    let set = |id: &str, body: &str| {
        tx.execute("UPDATE message SET body_text = ?2 WHERE id = ?1", rusqlite::params![id, body]).unwrap();
    };
    set(HOLDER, "Wire transfer confirmed: total 1,284,550.37 euros received on Monday.");
    set("m123456", "Owed 550.37 and separately 1,284 still pending.");
    set("m234567", "Subtotal 1,284 plus levy 550 plus fee 37 cents.");
    set("m345678", &"37 550 284 1 ".repeat(12));
    set("m456789", "Wire transfer confirmed: total 1,284,550.38 euros received on Monday.");
    set("m567890", "Confirmed transfer wire: total 9 euros received on Monday.");
    tx.execute("INSERT INTO attachment (id, message_id, file_name, mime_type, size_bytes, extracted_text) VALUES ('att-z', 'm888888', 'report.pdf', 'application/pdf', 10, 'appendix zebraquartz tables')", []).unwrap();
    tx.commit().unwrap();
}

fn p95(mut ms: Vec<f64>) -> f64 {
    ms.sort_by(|a, b| a.partial_cmp(b).unwrap());
    ms[((ms.len() as f64 * 0.95).ceil() as usize).saturating_sub(1)]
}

#[test]
fn an_exact_quoted_figure_ranks_the_holding_message_first_on_one_million_messages() {
    let dir = TempDir::new("million");
    let t = Instant::now();
    build_fixture(&dir.db());
    println!("fixture: {MESSAGES} messages built in {:.1}s, file {} MB", t.elapsed().as_secs_f64(), std::fs::metadata(dir.db()).unwrap().len() / 1_000_000);

    let store = Arc::new(store::Store::open(&dir.db()).unwrap());
    let t = Instant::now();
    let index = TextIndex::open(&store, &dir.index()).unwrap();
    println!("index built from the message table: {:.1}s, {} documents, {} MB on disk", t.elapsed().as_secs_f64(), index.documents(), dir_size(&dir.index()) / 1_000_000);
    assert!(index.was_rebuilt());
    assert_eq!(index.documents(), MESSAGES);

    // 1. The exact quoted figure: only the holder matches, so it is first.
    let hits = index.search("\"1,284,550.37\"", 10).unwrap();
    println!("quoted figure -> {:?}", hits.iter().map(|h| (h.message_id.as_str(), h.score)).collect::<Vec<_>>());
    assert_eq!(hits.first().map(|h| h.message_id.as_str()), Some(HOLDER), "the holding message ranks first");
    assert_eq!(hits.len(), 1, "an exact phrase excludes the reversed, split, repeated and different-figure decoys");
    // The same words as a bare query reach the decoys too, so the phrase is what narrows (the check has teeth).
    let bare = ids(&index.search("550 284 37", 10).unwrap());
    assert!(bare.contains(&"m123456".to_string()) && bare.contains(&"m234567".to_string()) && bare.contains(&"m345678".to_string()) && bare.contains(&HOLDER.to_string()), "bare words reach the decoys: {bare:?}");
    assert_eq!(ids(&index.search("zebraquartz", 5).unwrap()), vec!["m888888"], "attachment text is in the 1M index");
    // A phrase of plain words: the quotes alone make the order matter.
    let mut said = ids(&index.search("\"wire transfer confirmed\"", 10).unwrap());
    said.sort();
    assert_eq!(said, vec!["m456789".to_string(), HOLDER.to_string()], "the same words in another order do not match the phrase");
    assert!(ids(&index.search("wire transfer confirmed", 10).unwrap()).contains(&"m567890".to_string()), "unquoted, the reordered message matches");

    let mut samples = Vec::new();
    for q in ["\"1,284,550.37\"", "zebraquartz", "\"filler words for body 424242\"", "tok77 alpha5", "sender1234"] {
        for _ in 0..20 {
            let t = Instant::now();
            let _ = index.search(q, 20).unwrap();
            samples.push(t.elapsed().as_secs_f64() * 1000.0);
        }
    }
    println!("query p95 {:.1} ms over {} samples (debug build, loaded Mac; SEARCH_FIRST_SCREEN_MS in the ledger is {})", p95(samples.clone()), samples.len(), constant("SEARCH_FIRST_SCREEN_MS"));

    // 2. A new message is searchable within INDEX_LAG_MS of its store write.
    let limit = Duration::from_millis(constant("INDEX_LAG_MS"));
    let mut worst = Duration::ZERO;
    for n in 0..5 {
        let word = format!("novelword{n}x");
        let write_started = Instant::now();
        insert(&store, vec![new_msg(msg(&format!("new{n}"), "Brand new", "Sender", "owner@example.test", 2_000_000_000_000 + n), Some(&format!("hello {word} world")), None)]);
        println!("store write {n} returned after {:?} (the first one includes the store's from-scratch counts at 1M)", write_started.elapsed());
        // The lag clock starts when the store write returns, as the brief words it.
        let start = Instant::now();
        let seen = within(limit, || ids(&index.search(&word, 5).unwrap()) == vec![format!("new{n}")]);
        let total = start.elapsed();
        worst = worst.max(total);
        assert!(seen.is_some() && total <= limit, "new message {n} not searchable within INDEX_LAG_MS at 1M: {total:?}; last error {:?}", index.last_error());
    }
    println!("new message searchable in at most {worst:?} after its store write returned (limit {limit:?})");

    // 3. Reopening a whole index reuses it; a delete runs the exact table comparison at 1M.
    drop(index);
    let t = Instant::now();
    let index = TextIndex::open(&store, &dir.index()).unwrap();
    println!("reopen of the whole index: {:.2}s, rebuilt={}", t.elapsed().as_secs_f64(), index.was_rebuilt());
    assert!(!index.was_rebuilt());
    assert_eq!(index.documents(), MESSAGES + 5);
    let t = Instant::now();
    store
        .write(|ctx| {
            ctx.tx.execute("DELETE FROM message WHERE id = 'm345678'", [])?;
            Ok(())
        })
        .unwrap();
    println!("store delete returned after {:?}", t.elapsed());
    let after_write = Instant::now();
    let hidden = !ids(&index.search("550 284 37", 10).unwrap()).contains(&"m345678".to_string());
    println!("deleted message hidden from results {:?} after the store delete returned (hidden={hidden})", after_write.elapsed());
    assert!(hidden, "a deleted message must not be returned, even before the index has caught up");
    let dropped = within(Duration::from_secs(120), || index.documents() == MESSAGES + 4);
    println!("document dropped from the index {:?} after the store delete returned (exact comparison at 1M, windows of {} rowids)", after_write.elapsed(), search_text::limits::RECONCILE_WINDOW_ROWIDS);
    assert!(dropped.is_some(), "the delete reaches the index");

    // New mail during an exact comparison is served between windows, not after the whole comparison.
    index.request_repair();
    std::thread::sleep(Duration::from_millis(300));
    let start = Instant::now();
    insert(&store, vec![new_msg(msg("during", "During repair", "Sender", "owner@example.test", 2_000_000_000_100), Some("hello duringword world"), None)]);
    let start_after_write = Instant::now();
    let seen = within(Duration::from_secs(60), || ids(&index.search("duringword", 5).unwrap()) == vec!["during".to_string()]);
    println!("message written during a full comparison: store write {:?}, searchable {:?} after the write returned", start_after_write.duration_since(start), start_after_write.elapsed());
    assert!(seen.is_some(), "new mail is served while the comparison runs");
}

fn dir_size(p: &std::path::Path) -> u64 {
    std::fs::read_dir(p).unwrap().map(|e| e.unwrap().metadata().unwrap().len()).sum()
}
