//! attach-engine: the excerpts of the folders of this Mac and of calendar events are kept in the LOCAL DATABASE (`source_chunk`, 0016, with each item's stamp in
//! `source_item`, 0020), by the same walks as before (`reply_sources::sync_local_folder`, `sync_calendar`) through a store-backed `ExcerptSink`. Production core
//! over a real migrated store; the folders are temp folders of this Mac; no network. Each walk is observed by `settle` (the reply threads idle), every wait is
//! bounded and fails with a sentence naming what did not arrive.
//! MUTANTS (one edit each, source edited, test run, edit undone; logs in the lane's output folder):
//!   `rows-not-written`        put_item stores the stamp and no excerpt                     -> the_walks_write_their_excerpts_and_stamps_to_the_local_database
//!   `old-rows-kept`           a changed item's old rows are not deleted first              -> a_changed_item_is_replaced_and_an_unchanged_one_is_not_read_again
//!   `stamps-forgotten`        the stored stamps are not read back                          -> a_changed_item_is_replaced_and_an_unchanged_one_is_not_read_again
//!   `item-not-removed`        remove_item does nothing                                     -> a_file_that_is_gone_and_a_folder_taken_off_the_pane_leave_the_database
//!   `source-not-removed`      remove_source does nothing                                   -> a_file_that_is_gone_and_a_folder_taken_off_the_pane_leave_the_database
//!   `search-reads-nothing`    the retrieval's read finds no stored row                     -> after_a_restart_the_retrieval_reads_the_stored_excerpts_with_no_walk
//!   `index-dropped`           the search takes the plain scan, not the index (round 4)     -> a_search_over_ten_thousand_stored_excerpts_is_measured
//!   `index-ranks-by-id-only`  the indexed query's score is dropped from the order           -> the_index_answers_as_the_plain_scan_does_on_a_200_excerpt_fixture
use std::time::{Duration, Instant};

use contracts::*;
use engine::replies_work::{settle, StoredExcerpts};
use engine::RustCore;
use store::Store;
use reply_sources::{ExcerptSink, ItemStamp, SourceExcerpt};

const WAIT: Duration = Duration::from_secs(20);

fn wait_until(what: &str, mut ok: impl FnMut() -> bool) {
    let t0 = Instant::now();
    while t0.elapsed() < WAIT {
        if ok() { return; }
        std::thread::sleep(Duration::from_millis(20));
    }
    panic!("never reached within {WAIT:?}: {what}");
}

fn open(dir: &std::path::Path) -> RustCore { RustCore::open_production(dir.to_str().unwrap(), true).expect("the core opens") }
fn walked(core: &RustCore) { assert!(settle(core.store().path(), WAIT), "the reply threads did not settle within {WAIT:?}: a walk was still running"); }
fn run(core: &RustCore, c: Command) { core.run(c).expect("the command is accepted"); }

/// (item key, chunk no, title, text, date) of every stored excerpt of one source, by item then number.
type Rows = Vec<(String, i64, String, String, Option<i64>)>;
fn excerpts(core: &RustCore, source_key: &str) -> Rows {
    let key = source_key.to_string();
    core.store().write(move |ctx| {
        let mut st = ctx.tx.prepare("SELECT item_key, chunk_no, title, text, date_ms FROM source_chunk WHERE source_key = ?1 ORDER BY item_key, chunk_no")?;
        let v = st.query_map([&key], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)))?.collect::<Result<Vec<_>, _>>()?;
        Ok(v)
    }).unwrap()
}
/// (item key, version, count) of every stored stamp of one source.
fn stamps(core: &RustCore, source_key: &str) -> Vec<(String, String, i64)> {
    let key = source_key.to_string();
    core.store().write(move |ctx| {
        let mut st = ctx.tx.prepare("SELECT item_key, version, excerpt_count FROM source_item WHERE source_key = ?1 ORDER BY item_key")?;
        let v = st.query_map([&key], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?.collect::<Result<Vec<_>, _>>()?;
        Ok(v)
    }).unwrap()
}
/// the row ids of one item's excerpts (a row read again and written again has a new id)
fn ids(core: &RustCore, source_key: &str, item: &str) -> Vec<i64> {
    let (key, item) = (source_key.to_string(), item.to_string());
    core.store().write(move |ctx| {
        let mut st = ctx.tx.prepare("SELECT id FROM source_chunk WHERE source_key = ?1 AND item_key = ?2 ORDER BY chunk_no")?;
        let v = st.query_map([&key, &item], |r| r.get(0))?.collect::<Result<Vec<i64>, _>>()?;
        Ok(v)
    }).unwrap()
}
fn local_key(path: &str) -> String { reply_source_key(ReplySourceRef::LocalFolder { path: path.to_string() }) }

fn size_and_mtime(p: &std::path::Path) -> (u64, i64) {
    let m = std::fs::metadata(p).unwrap();
    (m.len(), m.modified().unwrap().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis() as i64)
}

/// A folder of this Mac with a price note, a second note that never changes and a word-processor file only the app can read (a file with no text), added on the Sources pane and walked.
fn folder(core: &RustCore) -> (tempfile::TempDir, String) {
    let d = tempfile::tempdir().unwrap();
    std::fs::write(d.path().join("prices.md"), "# Prices\n\nThe contract price is 42 euros a month, paid each quarter.").unwrap();
    std::fs::write(d.path().join("stable.md"), "# Stable\n\nThe office is open from nine to five, Monday to Friday, all year round.").unwrap();
    std::fs::write(d.path().join("report.docx"), [0x50u8, 0x4B, 3, 4, 1, 2, 3]).unwrap();
    let path = d.path().to_string_lossy().to_string();
    run(core, Command::AddReplyLocalFolder { path: path.clone() });
    wait_until("the folder's three items are stored", || stamps(core, &local_key(&path)).len() == 3);
    walked(core);
    (d, path)
}

/// (1) The walks write what they read to the local database as contract delta 23 and migration 0016 describe: a file is an origin File (source key `local`,
/// item key = its path inside the folder), an event an origin Event (source key `calendar`, item key "<event id>@<start ms>"); each item has its stamp, a file
/// with no text for the core (a .docx: only the app can read it) has a stamp with count 0 and no excerpt.
/// Mutant: `rows-not-written`.
#[test]
fn the_walks_write_their_excerpts_and_stamps_to_the_local_database() {
    let data = tempfile::tempdir().unwrap();
    let core = open(data.path());
    let (docs, path) = folder(&core);
    let key = local_key(&path);
    assert!(key.starts_with("local\u{1f}"), "{key:?}");
    let rows = excerpts(&core, &key);
    let prices: Vec<_> = rows.iter().filter(|r| r.0 == "prices.md").collect();
    assert_eq!(prices.len(), 1, "one excerpt for the short note: {rows:?}");
    assert_eq!((prices[0].1, prices[0].2.as_str()), (0, "Prices"), "chunk 0, titled by its heading");
    assert!(prices[0].3.contains("The contract price is 42 euros a month"), "{:?}", prices[0].3);
    let (size, mtime) = size_and_mtime(&docs.path().join("prices.md"));
    assert_eq!(prices[0].4, Some(mtime), "the file's modified date");
    assert!(rows.iter().any(|r| r.0 == "stable.md"), "{rows:?}");
    assert!(rows.iter().all(|r| r.0 != "report.docx"), "a word-processor file only the app can read holds no text: no excerpt row");
    let st = stamps(&core, &key);
    assert_eq!(st.iter().map(|s| (s.0.as_str(), s.2)).collect::<Vec<_>>(), vec![("prices.md", 1), ("report.docx", 0), ("stable.md", 1)], "{st:?}");
    assert_eq!(st.iter().find(|s| s.0 == "prices.md").unwrap().1, format!("{size}:{mtime}"), "the stamp's version is the file's size and date");

    // a calendar event: an origin Event under the calendar's key
    let work = ReplyCalendarInfo { calendar_id: "cal-work".into(), name: "Travail".into(), account_label: "iCloud".into(), colour: "#3366FF".into() };
    let at = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis() as i64 + 86_400_000;
    let event = CalendarEvent { event_id: "e1".into(), calendar_id: "cal-work".into(), title: "Board meeting with Claire".into(), start_ms: at, end_ms: at + 3_600_000, attendees: vec!["Claire".into()], location: Some("Brussels".into()) };
    let push = |core: &RustCore| run(core, Command::SyncReplyCalendars { calendars: vec![work.clone()], window_start_ms: at - 7 * 86_400_000, window_end_ms: at + 14 * 86_400_000, events: vec![event.clone()] });
    push(&core);
    walked(&core);
    run(&core, Command::SetReplySourceEnabled { source: ReplySourceRef::Calendar { calendar_id: "cal-work".into() }, enabled: true });
    push(&core);
    walked(&core);
    let ckey = reply_source_key(ReplySourceRef::Calendar { calendar_id: "cal-work".into() });
    wait_until("the event is stored", || !excerpts(&core, &ckey).is_empty());
    let ev = excerpts(&core, &ckey);
    assert_eq!(ev.len(), 1, "{ev:?}");
    assert_eq!((ev[0].0.clone(), ev[0].1, ev[0].2.as_str(), ev[0].4), (format!("e1@{at}"), 0, "Board meeting with Claire", Some(at)));
    assert!(ev[0].3.contains("Brussels") && ev[0].3.contains("Claire"), "{:?}", ev[0].3);
    assert_eq!(stamps(&core, &ckey).len(), 1);
}

/// (2) After a restart an item whose stamp changed is replaced (its old rows go, the new ones come, one stamp), and an item whose stamp did not is not read
/// again: its rows keep their ids (a row written again has a new id). The restart is what walks the folder again.
/// Mutants: `old-rows-kept` (a changed item's old rows stay), `stamps-forgotten` (the stored stamps are not read back: every file is read and written again).
#[test]
fn a_changed_item_is_replaced_and_an_unchanged_one_is_not_read_again() {
    let data = tempfile::tempdir().unwrap();
    let core = open(data.path());
    let (docs, path) = folder(&core);
    let key = local_key(&path);
    let (prices_before, stable_before) = (ids(&core, &key, "prices.md"), ids(&core, &key, "stable.md"));
    let version_before = stamps(&core, &key).into_iter().find(|s| s.0 == "prices.md").unwrap().1;
    drop(core);
    std::fs::write(docs.path().join("prices.md"), "# Prices\n\nThe contract price is now 57 euros a month, paid each month, with a one year commitment.").unwrap();
    let core = open(data.path());
    wait_until("the changed file is read again", || excerpts(&core, &key).iter().any(|r| r.0 == "prices.md" && r.3.contains("57 euros")));
    walked(&core);
    let rows = excerpts(&core, &key);
    let prices: Vec<_> = rows.iter().filter(|r| r.0 == "prices.md").collect();
    assert_eq!(prices.len(), 1, "the old rows are gone, one excerpt for the new text: {prices:?}");
    assert!(!prices[0].3.contains("42 euros"), "the old text is gone: {:?}", prices[0].3);
    let st = stamps(&core, &key);
    assert_eq!(st.iter().filter(|s| s.0 == "prices.md").count(), 1, "one stamp for the file");
    assert_ne!(st.iter().find(|s| s.0 == "prices.md").unwrap().1, version_before, "the stamp follows the file");
    assert_ne!(ids(&core, &key, "prices.md"), prices_before, "the changed file was written again");
    assert_eq!(ids(&core, &key, "stable.md"), stable_before, "the unchanged file was not read and written again: its rows keep their ids");
}

/// (3) A file that is gone leaves the database at the next walk (its excerpts and its stamp), the others stay; a folder taken off the Sources pane takes every
/// excerpt and stamp of its source.
/// Mutants: `item-not-removed`, `source-not-removed`.
#[test]
fn a_file_that_is_gone_and_a_folder_taken_off_the_pane_leave_the_database() {
    let data = tempfile::tempdir().unwrap();
    let core = open(data.path());
    let (docs, path) = folder(&core);
    let key = local_key(&path);
    drop(core);
    std::fs::remove_file(docs.path().join("prices.md")).unwrap();
    let core = open(data.path());
    wait_until("the gone file leaves the database", || stamps(&core, &key).iter().all(|s| s.0 != "prices.md"));
    walked(&core);
    assert!(excerpts(&core, &key).iter().all(|r| r.0 != "prices.md"), "its excerpts left with its stamp");
    assert!(excerpts(&core, &key).iter().any(|r| r.0 == "stable.md"), "the others stay");
    // the full-text index follows: the gone file's words find nothing, the others' still do
    let index = StoredExcerpts::new(core.store());
    let source = ReplySourceRef::LocalFolder { path: path.clone() };
    assert!(index.search("contract price euros", std::slice::from_ref(&source), 5).is_empty(), "the gone file's words left the index");
    assert_eq!(index.search("office open monday", std::slice::from_ref(&source), 5).len(), 1, "the other file is still found");
    run(&core, Command::RemoveReplyLocalFolder { path: path.clone() });
    walked(&core);
    wait_until("the removed folder's rows leave", || excerpts(&core, &key).is_empty() && stamps(&core, &key).is_empty());
    assert_eq!(StoredExcerpts::new(core.store()).excerpt_count(&ReplySourceRef::LocalFolder { path: path.clone() }), 0);
    assert!(index.search("office open monday", std::slice::from_ref(&source), 5).is_empty(), "a removed folder's words left the index");
    let fts_rows: i64 = core.store().write(|ctx| Ok(ctx.tx.query_row("SELECT count(*) FROM source_chunk_fts WHERE source_chunk_fts MATCH '\"office\"'", [], |r| r.get(0))?)).unwrap();
    assert_eq!(fts_rows, 0, "no index row of the removed source is left");
}

/// (3b) Excerpts of a source that is no longer on the Sources pane leave the database at the next pass of the reply thread (the explicit removal; the delete
/// of a pane row also takes them through the schema's triggers, so the test uses a source that never had a row).
/// Mutant: `source-not-removed`.
#[test]
fn the_excerpts_of_a_source_that_is_not_on_the_pane_are_removed_by_the_reply_thread() {
    let data = tempfile::tempdir().unwrap();
    let core = open(data.path());
    let orphan = ReplySourceRef::LocalFolder { path: "/Users/nobody/Gone".into() };
    let sink = StoredExcerpts::new(core.store());
    let x = SourceExcerpt { id: "x".into(), source: orphan.clone(), item_key: "a.md".into(), ordinal: 0, title: "A".into(), folder: "/Users/nobody/Gone/a.md".into(), date_ms: Some(1), text: "an excerpt of a folder nobody lists".into() };
    sink.put_item(&orphan, &ItemStamp { item_key: "a.md".into(), version: "1:1".into(), excerpt_count: 1 }, &[x]).unwrap();
    // any change of the pane makes the reply thread look at what the index holds
    let d = tempfile::tempdir().unwrap();
    let other = d.path().to_string_lossy().to_string();
    run(&core, Command::AddReplyLocalFolder { path: other.clone() });
    run(&core, Command::RemoveReplyLocalFolder { path: other });
    walked(&core);
    wait_until("the orphan source's excerpts are removed", || sink.excerpt_count(&orphan) == 0 && !sink.sources().contains(&orphan));
    assert!(sink.search("excerpt folder nobody", std::slice::from_ref(&orphan), 5).is_empty(), "and its index rows");
}

/// (4) After a restart the retrieval reads the stored excerpts with NO walk: the folder is gone from the disk before the core opens again (a walk could only
/// fail), and the retrieval's index still finds the note by its words, with the folder as the owner reads it and the walks' own excerpt id.
/// Mutant: `search-reads-nothing`.
#[test]
fn after_a_restart_the_retrieval_reads_the_stored_excerpts_with_no_walk() {
    let data = tempfile::tempdir().unwrap();
    let core = open(data.path());
    let (docs, path) = folder(&core);
    let key = local_key(&path);
    drop(core);
    drop(docs); // the folder is deleted from the disk
    let core = open(data.path());
    walked(&core);
    let source = ReplySourceRef::LocalFolder { path: path.clone() };
    let index = StoredExcerpts::new(core.store());
    let found = index.search("contract price", std::slice::from_ref(&source), 5);
    assert_eq!(found.len(), 1, "{found:?}");
    assert!(found[0].text.contains("42 euros"), "{:?}", found[0].text);
    assert_eq!(found[0].chunk_id, format!("{key}\u{1f}prices.md\u{1f}0"), "the walks' own id of an excerpt: stable across restarts");
    assert_eq!(found[0].folder.as_deref(), Some(format!("{}/prices.md", path.trim_end_matches('/')).as_str()), "where the owner reads it");
    assert_eq!(found[0].subject.as_deref(), Some("Prices"));
    assert!(index.sources().contains(&source));
    assert_eq!(index.dated_texts(std::slice::from_ref(&source)).len(), 2, "both notes");
    // the walk could not read the folder, so it removed nothing
    assert_eq!(stamps(&core, &key).len(), 3);
}

/// The machine's one-minute load (from `uptime`; its decimal comma or point both read).
fn load_now() -> f64 {
    let out = std::process::Command::new("uptime").output().map(|o| String::from_utf8_lossy(&o.stdout).into_owned()).unwrap_or_default();
    out.split("load average").nth(1).and_then(|r| r.trim_start_matches(|c: char| c == 's' || c == ':' || c == ' ').split_whitespace().next().map(|w| w.trim_end_matches(',').replace(',', ".")))
        .and_then(|w| w.parse().ok()).unwrap_or(f64::MAX)
}

/// A bare store (no core: a core's reply thread removes the excerpts of a source that is not on the Sources pane) holding `items` x `per_item` excerpts of
/// one folder. Each excerpt is `words` tokens of a 300-word vocabulary (v0 to v299), so a query word is in about a fifth of the rows.
fn big_store(items: u32, per_item: u32, words: u32) -> (tempfile::TempDir, StoredExcerpts, ReplySourceRef) {
    let data = tempfile::tempdir().unwrap();
    let store = std::sync::Arc::new(Store::open(&data.path().join("whoosh.sqlite")).unwrap());
    let source = ReplySourceRef::LocalFolder { path: "/Users/o/Big".into() };
    let sink = StoredExcerpts::new(store);
    for item in 0..items {
        let excerpts: Vec<SourceExcerpt> = (0..per_item).map(|n| {
            let text: String = (0..words).map(|w| format!("v{} ", (item * 7 + n * 3 + w * 13 + (item * n) % 11) % 300)).collect();
            SourceExcerpt { id: format!("x{item}-{n}"), source: source.clone(), item_key: format!("f{item}.md"), ordinal: n, title: format!("File v{} number{item}", item % 300), folder: "/Users/o/Big".into(), date_ms: Some(1), text }
        }).collect();
        sink.put_item(&source, &ItemStamp { item_key: format!("f{item}.md"), version: "1:1".into(), excerpt_count: per_item }, &excerpts).unwrap();
    }
    (data, sink, source)
}

/// (5) One search over 10,000 stored excerpts answers under the ledger's operation limit (the owner: "The objective is <50ms for any operation";
/// INTERACTION_COMMIT_MS): the median of five warm searches, judged only when the machine load is under 12 (above it the figures are printed and the test says
/// "not judged"). The rows the search READS are counted whatever the load: the index hands over at most `limit` rows where the scan reads all 10,000.
/// Mutants: `index-dropped` (the search takes the plain scan: red on the row count at any load, and on the timing at a quiet one).
#[test]
fn a_search_over_ten_thousand_stored_excerpts_is_measured() {
    let (_data, sink, source) = big_store(100, 100, 60);
    assert_eq!(sink.excerpt_count(&source), 10_000);
    let q = "v12 v250 v77";
    let mut times = Vec::new();
    let mut found = Vec::new();
    for _ in 0..5 {
        let t0 = Instant::now();
        found = sink.search(q, std::slice::from_ref(&source), 8);
        times.push(t0.elapsed());
    }
    times.sort();
    let (median, load) = (times[2], load_now());
    println!("SEARCH 10000 stored excerpts, indexed: median {median:?} (all {times:?}) for {} hits at load {load}", found.len());
    assert_eq!(found.len(), 8);
    assert_eq!(sink.rows_read(), 8, "the index hands over only the best rows, not all 10,000");
    let t0 = Instant::now();
    let scanned = sink.search_scan(q, std::slice::from_ref(&source), 8);
    println!("SEARCH 10000 stored excerpts, plain scan: {:?} at load {load}", t0.elapsed());
    assert_eq!(sink.rows_read(), 10_000, "the reference scan reads every row");
    assert_eq!(found, scanned, "the same excerpts in the same order as the plain scan");
    let limit = Duration::from_millis(contracts::constants::INTERACTION_COMMIT_MS);
    if load < 12.0 {
        assert!(median < limit, "one search over 10,000 stored excerpts took {median:?} (all {times:?}) at load {load}, over the {limit:?} an operation may take");
    } else {
        println!("not judged: load {load}");
    }
}

/// (6) The index answers exactly as the plain scan does: the same excerpts, scores and order, on a 200-excerpt fixture, for queries of one, two and three words,
/// a repeated word, a word nobody holds, a word in a title only, and limits below, at and above the number of matches.
/// Mutant: `index-ranks-by-id-only` (the tie-break or the score of the indexed query is dropped).
#[test]
fn the_index_answers_as_the_plain_scan_does_on_a_200_excerpt_fixture() {
    let (_data, sink, source) = big_store(10, 20, 12);
    assert_eq!(sink.excerpt_count(&source), 200);
    let sources = std::slice::from_ref(&source);
    let mut compared = 0;
    for q in ["v12", "v12 v250", "v12 v250 v77", "v12 v12 v250", "v1 v2 v3 v4 v5 v6", "nothing here", "number3", "File v7 number7", "the v100", "V12 v250!"] {
        for limit in [1usize, 5, 50, 500] {
            let (fast, slow) = (sink.search(q, sources, limit), sink.search_scan(q, sources, limit));
            assert_eq!(fast, slow, "query {q:?}, limit {limit}");
            compared += fast.len();
        }
    }
    assert!(compared > 100, "the fixture finds something: {compared} results compared");
    assert!(sink.search("nothing here", sources, 5).is_empty());
}
