//! attach-fetch (SPEC A152, owner 2026-11-11: "it's subject+body+attachment and they should be all related"): the TEXT attachments of stored mail are
//! fetched in the background, so their text is read and searchable without a click. Child of `attachment_download.rs` (declared there).
//!
//! WHAT IS QUEUED: an attachment of a stored mail whose kind is PDF or plain text (`AttachmentKind::detect`: declared type AND file extension), at most
//! ATTACH_AUTO_FETCH_MAX_BYTES, with no file on this Mac yet. Every other kind stays as before (fetched on a click). TWO TRIGGERS, both one line in
//! bodies.rs: (1) `after_bodies`, called when a batch of bodies has been kept (prefetch, crawl, the reader's own: they all end in `Bodies::download_with`),
//! queues the files of those mails at the FRONT; (2) `on_source`, called when an account's mail source is registered (`Bodies::set_source`), is the
//! BACKFILL: one scan per account per core start of the rows stored before this change (newest mail first), queued at the BACK.
//!
//! THE FETCH IS THE CLICK'S: `super::fetch` (= `Bodies::attachment_bytes`: one `BODY.PEEK[<part>]`, decoded, kept at `local_path`), then the second
//! round `super::index_text` reads the text from the disk copy and files it and its `chunk` rows. No second way to fetch, and this file never cuts text.
//!
//! BOUNDS. One file at a time. At most ATTACH_AUTO_FETCH_PER_STEP files per step, then the loop sleeps ATTACH_AUTO_STEP_PAUSE_MS, so a body fetch
//! or an action of the owner is never behind more than one file; before each file it also waits while the owner's own `DownloadAttachment` is running.
//! A file already on this Mac is skipped (no fetch). The queue holds at most ATTACH_AUTO_QUEUE_MAX ids (the oldest fall off the back; the backfill at
//! the next start brings them again). The bytes of ONE part are in memory only while `Bodies::attachment_bytes` decodes it (the limit named in
//! attachment_download.rs); this file holds ids, never bytes.
//!
//! FAILURE AND RETRY. A failed fetch leaves ONE core.log line (`attachment <id>: not fetched automatically: <reason in words>`), nothing for the owner, and
//! the queue goes on. The id is never queued again in this process (`attempted`), and its failure is kept in the ledger with its time: a later start (the
//! backfill) or a later body keep skips it until ATTACH_AUTO_RETRY_AFTER_MS has passed, so a file the server no longer holds is asked about once a week
//! at most, never in a loop.
//!
//! THE TOTAL CAP. The files this lane fetched are listed in `<data dir>/attachment-autofetch.ledger` (`ok<TAB>id<TAB>bytes`, `fail<TAB>ms<TAB>id`). At the
//! first use of a start the ledger is read against the rows (an `ok` whose row no longer has a file frees its bytes) and compacted. A file is fetched
//! only when the bytes so far plus its size stay within ATTACH_AUTO_KEEP_TOTAL_MAX_BYTES; at the cap the queue is dropped, nothing more is fetched
//! automatically, ONE core.log line says so, and the state path says `capped`. Files fetched by a click are not counted and not limited here.
//!
//! STATE: `attachment/auto-fetch/transfer` (served through `attachment_download::state`, which reads every `attachment/<x>/transfer`):
//! `{"queued":n,"fetched":n,"failed":n,"skipped":n,"bytes_on_disk":n,"capped":bool}`. fetched/failed/skipped count this start only; `queued` is the
//! queue now; `bytes_on_disk` is the ledger's total (0 until the ledger was read, at the first use).
//!
//! SWITCH: the environment variable WHOOSH_ATTACH_AUTO_FETCH=off turns the whole lane off for the process (for a run that must see the old behaviour).
//!
//! The numbers below are PROPOSED, ledger row owed (docs/CONSTANTS.md is the lead's).
use std::collections::{HashMap, HashSet, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock, Weak};
use std::time::Duration;

use attachment_index::AttachmentKind;
use store::Store;

use crate::bodies::Bodies;

/// The biggest file fetched by itself. Depends on: the disk a file takes (the cap below), the time one fetch holds the connection and the memory of one decode
/// (the whole part, about 1.4 x this in base64 plus the decoded copy). ledger row owed
pub const ATTACH_AUTO_FETCH_MAX_BYTES: u64 = contracts::constants::ATTACH_AUTO_FETCH_MAX_BYTES;
/// Files per step before the queue yields. Depends on: how long the owner's own action may wait behind the step (4 small files, a second or two). ledger row owed
pub const ATTACH_AUTO_FETCH_PER_STEP: usize = contracts::constants::ATTACH_AUTO_FETCH_PER_STEP as usize;
/// The pause between two steps. Depends on: the per-step number (the connection is free to the body fetches and the owner's actions in between). ledger row owed
pub const ATTACH_AUTO_STEP_PAUSE_MS: u64 = contracts::constants::ATTACH_AUTO_STEP_PAUSE_MS;
/// The total the automatic files may take on this Mac. Depends on: the owner's disk; about 10 000 files of 200 KB. ledger row owed
pub const ATTACH_AUTO_KEEP_TOTAL_MAX_BYTES: u64 = contracts::constants::ATTACH_AUTO_KEEP_TOTAL_MAX_BYTES;
/// Ids the queue holds at most (memory: a few hundred KB). ledger row owed
pub const ATTACH_AUTO_QUEUE_MAX: usize = contracts::constants::ATTACH_AUTO_QUEUE_MAX as usize;
/// A failed file is asked again after this long at the earliest (7 days). ledger row owed
pub const ATTACH_AUTO_RETRY_AFTER_MS: i64 = contracts::constants::ATTACH_AUTO_RETRY_AFTER_MS as i64;
/// The owner's own download is waited for in polls of this length, at most OWNER_FIRST_POLLS_MAX of them (30 s).
const OWNER_FIRST_POLL_MS: u64 = 200;
const OWNER_FIRST_POLLS_MAX: u32 = 150;
/// The ledger's name, in the data folder.
pub const LEDGER_FILE: &str = "attachment-autofetch.ledger";
/// The state path (served by `attachment_download::state`).
pub const STATE_PATH: &str = "attachment/auto-fetch/transfer";

#[derive(Default)]
struct S {
    queue: VecDeque<String>,
    queued: HashSet<String>,
    /// Popped in this process (fetched, failed or skipped): never queued again before the next start.
    attempted: HashSet<String>,
    fetched: u64,
    failed: u64,
    skipped: u64,
    bytes: u64,
    capped: bool,
    running: bool,
    loaded: bool,
    failed_at: HashMap<String, i64>,
    backfilled: HashSet<String>,
}

#[derive(Default)]
pub struct Auto { s: Mutex<S> }

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> { m.lock().unwrap_or_else(|e| e.into_inner()) }

fn registry() -> &'static Mutex<HashMap<usize, (Weak<Store>, Arc<Auto>)>> {
    static R: OnceLock<Mutex<HashMap<usize, (Weak<Store>, Arc<Auto>)>>> = OnceLock::new();
    R.get_or_init(Default::default)
}

fn auto_of(store: &Arc<Store>) -> Arc<Auto> {
    let mut reg = lock(registry());
    reg.retain(|_, (w, _)| w.strong_count() > 0);
    reg.entry(Arc::as_ptr(store) as usize).or_insert_with(|| (Arc::downgrade(store), Arc::new(Auto::default()))).1.clone()
}

fn existing_auto(store: &Store) -> Option<Arc<Auto>> {
    let reg = lock(registry());
    reg.get(&(store as *const Store as usize)).filter(|(w, _)| w.strong_count() > 0).map(|(_, a)| a.clone())
}

fn off() -> bool { matches!(std::env::var("WHOOSH_ATTACH_AUTO_FETCH").as_deref(), Ok("off") | Ok("0")) }

fn ledger_path(store: &Store) -> PathBuf { store.path().parent().unwrap_or_else(|| Path::new(".")).join(LEDGER_FILE) }

/// The state path's JSON (numbers and one bool only; a store read-free answer).
pub fn state_json(store: &Store) -> String {
    let (q, f, x, k, b, c) = match existing_auto(store) {
        Some(a) => { let s = lock(&a.s); (s.queue.len(), s.fetched, s.failed, s.skipped, s.bytes, s.capped) }
        None => (0, 0, 0, 0, 0, false),
    };
    format!("{{\"queued\":{q},\"fetched\":{f},\"failed\":{x},\"skipped\":{k},\"bytes_on_disk\":{b},\"capped\":{c}}}")
}

/// A text kind this lane fetches: PDF or plain text, by declared type and by extension (`AttachmentKind::detect`).
fn text_kind(mime: &str, name: &str) -> bool { matches!(AttachmentKind::detect(mime, name), AttachmentKind::Pdf | AttachmentKind::PlainText) }

fn within_size(size: u64) -> bool { size > 0 && size <= ATTACH_AUTO_FETCH_MAX_BYTES }

fn id_ok_for_ledger(id: &str) -> bool { !id.contains('\t') && !id.contains('\n') && !id.contains('\r') }

fn append_ledger(store: &Store, line: &str) {
    use std::io::Write;
    let r = std::fs::OpenOptions::new().create(true).append(true).open(ledger_path(store)).and_then(|mut f| f.write_all(format!("{line}\n").as_bytes()));
    if let Err(e) = r { crate::log::log_line(&crate::log::path_beside(store.path()), &format!("attachment auto fetch: the ledger was not written: {e}")); }
}

/// The ids of the rows that hold a file path now (one read-only query).
fn ids_with_files(store: &Store) -> Option<HashSet<String>> {
    let c = smart_inbox::load::open_reader(store.path()).ok()?;
    let mut st = c.prepare("SELECT id FROM attachment WHERE local_path IS NOT NULL").ok()?;
    let rows = st.query_map([], |r| r.get::<_, String>(0)).ok()?;
    Some(rows.flatten().collect())
}

/// Reads the ledger once per start (blocking): the failures still inside the retry window and the bytes the automatic files still take.
fn ensure_loaded(store: &Store, a: &Auto) {
    if lock(&a.s).loaded { return; }
    let text = std::fs::read_to_string(ledger_path(store)).unwrap_or_default();
    let now = crate::rust_core::now_ms();
    let (mut ok, mut failed): (HashMap<String, u64>, HashMap<String, i64>) = (HashMap::new(), HashMap::new());
    let total_lines = text.lines().count();
    for line in text.lines() {
        match line.split('\t').collect::<Vec<_>>().as_slice() {
            ["ok", id, b] => { if let Ok(b) = b.parse::<u64>() { ok.insert((*id).to_string(), b); } }
            ["fail", ms, id] => { if let Ok(ms) = ms.parse::<i64>() { failed.insert((*id).to_string(), ms); } }
            _ => {}
        }
    }
    if let Some(on_disk) = ids_with_files(store) { ok.retain(|id, _| on_disk.contains(id)); }
    failed.retain(|_, ms| now - *ms < ATTACH_AUTO_RETRY_AFTER_MS);
    let kept = ok.len() + failed.len();
    if kept != total_lines {
        let body: String = ok.iter().map(|(id, b)| format!("ok\t{id}\t{b}\n")).chain(failed.iter().map(|(id, ms)| format!("fail\t{ms}\t{id}\n"))).collect();
        let tmp = ledger_path(store).with_extension("ledger.tmp");
        if std::fs::write(&tmp, body).and_then(|_| std::fs::rename(&tmp, ledger_path(store))).is_err() { let _ = std::fs::remove_file(&tmp); }
    }
    let mut s = lock(&a.s);
    if s.loaded { return; }
    s.bytes = ok.values().sum();
    s.failed_at = failed;
    s.loaded = true;
}

/// One candidate as the store lists it: (id, file name, MIME type, size, a file is on this Mac).
type Cand = (String, String, String, u64, bool);

/// Puts the eligible candidates in the queue (front: the new mail, in the order given; back: the backfill), the oldest falling off the back at the bound.
fn enqueue(a: &Auto, cands: Vec<Cand>, front: bool) -> usize {
    let now = crate::rust_core::now_ms();
    let mut s = lock(&a.s);
    if s.capped { return 0; }
    let (mut fresh, mut seen): (Vec<String>, HashSet<String>) = (Vec::new(), HashSet::new());
    for (id, name, mime, size, on_disk) in cands {
        let again = s.failed_at.get(&id).is_some_and(|at| now - at < ATTACH_AUTO_RETRY_AFTER_MS);
        if on_disk || !within_size(size) || !text_kind(&mime, &name) || s.attempted.contains(&id) || s.queued.contains(&id) || again || !seen.insert(id.clone()) { continue; }
        fresh.push(id);
    }
    let n = fresh.len();
    if front {
        for id in fresh.into_iter().rev() { s.queued.insert(id.clone()); s.queue.push_front(id); }
        while s.queue.len() > ATTACH_AUTO_QUEUE_MAX { if let Some(old) = s.queue.pop_back() { s.queued.remove(&old); } }
    } else {
        for id in fresh { if s.queue.len() >= ATTACH_AUTO_QUEUE_MAX { break; } s.queued.insert(id.clone()); s.queue.push_back(id); }
    }
    n
}

/// The next id (marked attempted), or None with `running` cleared in the same step (so a push after it starts a new loop).
fn next(a: &Auto) -> Option<String> {
    let mut s = lock(&a.s);
    match s.queue.pop_front() {
        Some(id) => { s.queued.remove(&id); s.attempted.insert(id.clone()); Some(id) }
        None => { s.running = false; None }
    }
}

/// Trigger 1 (bodies.rs, `download_with`): a batch of bodies was kept. Returns at once; the reads and the fetches run on the core's runtime.
pub fn after_bodies(bodies: &Arc<Bodies>, store: &Arc<Store>, rt: &tokio::runtime::Handle, message_ids: &[String]) {
    if off() || message_ids.is_empty() { return; }
    let a = auto_of(store);
    if lock(&a.s).capped { return; }
    let (b, st, a2, ids) = (bodies.clone(), store.clone(), a, message_ids.to_vec());
    rt.spawn(async move {
        let (st2, a3) = (st.clone(), a2.clone());
        let found = tokio::task::spawn_blocking(move || {
            ensure_loaded(&st2, &a3);
            let mut out: Vec<Cand> = Vec::new();
            for id in &ids { if let Ok(files) = st2.attachments(id) { out.extend(files.into_iter().map(|f| (f.id, f.file_name, f.mime_type, f.size_bytes, f.local_path.is_some()))); } }
            out
        }).await.unwrap_or_default();
        enqueue(&a2, found, true);
        start(b, st, a2);
    });
}

/// Trigger 2 (bodies.rs, `set_source`): the backfill of the rows stored before this change, once per account per start, newest mail first.
pub fn on_source(bodies: &Arc<Bodies>, store: &Arc<Store>, rt: &tokio::runtime::Handle, account: &str) {
    if off() { return; }
    let a = auto_of(store);
    if !lock(&a.s).backfilled.insert(account.to_string()) { return; }
    let (b, st, a2, account) = (bodies.clone(), store.clone(), a, account.to_string());
    rt.spawn(async move {
        let (st2, a3) = (st.clone(), a2.clone());
        let found = tokio::task::spawn_blocking(move || { ensure_loaded(&st2, &a3); backfill_rows(&st2, &account) }).await.unwrap_or_default();
        enqueue(&a2, found, false);
        start(b, st, a2);
    });
}

/// The text-kind rows of one account with no file, newest mail first (at most ATTACH_AUTO_QUEUE_MAX).
fn backfill_rows(store: &Store, account: &str) -> Vec<Cand> {
    let Ok(c) = smart_inbox::load::open_reader(store.path()) else { return Vec::new() };
    let Ok(mut st) = c.prepare(
        "SELECT a.id, a.file_name, a.mime_type, a.size_bytes FROM attachment a JOIN message m ON m.id = a.message_id \
         WHERE m.account_id = ?1 AND a.local_path IS NULL AND a.size_bytes > 0 AND a.size_bytes <= ?2 ORDER BY m.received_at_ms DESC, a.id") else { return Vec::new() };
    let Ok(rows) = st.query_map(rusqlite::params![account, ATTACH_AUTO_FETCH_MAX_BYTES as i64], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?, r.get::<_, String>(2)?, r.get::<_, i64>(3)?.max(0) as u64, false))) else { return Vec::new() };
    let mut out: Vec<Cand> = Vec::new();
    for row in rows.flatten() {
        if text_kind(&row.2, &row.1) { out.push(row); if out.len() >= ATTACH_AUTO_QUEUE_MAX { break; } }
    }
    out
}

/// Starts the loop when there is work and none is running.
fn start(bodies: Arc<Bodies>, store: Arc<Store>, a: Arc<Auto>) {
    { let mut s = lock(&a.s); if s.running || s.queue.is_empty() { return; } s.running = true; }
    tokio::spawn(run_loop(bodies, store, a));
}

async fn owner_first(store: &Arc<Store>) {
    for _ in 0..OWNER_FIRST_POLLS_MAX {
        let busy = super::existing(store).is_some_and(|t| !super::lock(&t.running).is_empty());
        if !busy { return; }
        tokio::time::sleep(Duration::from_millis(OWNER_FIRST_POLL_MS)).await;
    }
}

async fn run_loop(bodies: Arc<Bodies>, store: Arc<Store>, a: Arc<Auto>) {
    let log = crate::log::path_beside(store.path());
    loop {
        for _ in 0..ATTACH_AUTO_FETCH_PER_STEP {
            owner_first(&store).await;
            let Some(id) = next(&a) else { return };
            one(&bodies, &store, &a, &id, &log).await;
            let capped = lock(&a.s).capped;
            if capped { let mut s = lock(&a.s); s.queue.clear(); s.queued.clear(); s.running = false; return; }
        }
        // the step is over: the queue yields, body fetches and the owner's own actions go first
        tokio::time::sleep(Duration::from_millis(ATTACH_AUTO_STEP_PAUSE_MS)).await;
    }
}

enum Pre { Skip, Cap(u64), Go }

/// One file: the checks, the click's own fetch, the text round, the books. A failure is one log line and the loop goes on.
async fn one(bodies: &Arc<Bodies>, store: &Arc<Store>, a: &Arc<Auto>, id: &str, log: &Path) {
    let (st, a2, id2) = (store.clone(), a.clone(), id.to_string());
    let pre = tokio::task::spawn_blocking(move || -> Pre {
        let Ok(Some(f)) = super::file_of(&st, &id2) else { return Pre::Skip };
        if f.local_path.is_some() || !within_size(f.size_bytes) || !text_kind(&f.mime_type, &f.file_name) { return Pre::Skip; }
        let used = lock(&a2.s).bytes;
        if used + f.size_bytes > ATTACH_AUTO_KEEP_TOTAL_MAX_BYTES { return Pre::Cap(used); }
        Pre::Go
    }).await.unwrap_or(Pre::Skip);
    match pre {
        Pre::Skip => { lock(&a.s).skipped += 1; return; }
        Pre::Cap(used) => {
            let first = { let mut s = lock(&a.s); let first = !s.capped; s.capped = true; first };
            if first {
                crate::log::log_line(log, &format!("attachment auto fetch: stopped, the files fetched by themselves take {} MB and the limit is {} MB; nothing more is fetched by itself", used / (1024 * 1024), ATTACH_AUTO_KEEP_TOTAL_MAX_BYTES / (1024 * 1024)));
            }
            return;
        }
        Pre::Go => {}
    }
    // the click's own fetch, then (in the blocking task) the size check and the second round that reads and files the text
    let outcome: Result<u64, String> = match super::fetch(store, bodies, id).await {
        Err(why) => Err(why),
        Ok(()) => {
            let (st, id2, lg) = (store.clone(), id.to_string(), log.to_path_buf());
            tokio::task::spawn_blocking(move || -> Result<u64, String> {
                let path = super::file_of(&st, &id2).ok().flatten().and_then(|f| f.local_path).ok_or_else(|| "the file came but its place was not recorded".to_string())?;
                let len = std::fs::metadata(&path).map(|m| m.len()).map_err(|e| format!("the file could not be measured: {e}"))?;
                if len > ATTACH_AUTO_FETCH_MAX_BYTES {
                    let _ = std::fs::remove_file(&path);
                    let _ = st.set_attachment_local_path(&id2, None);
                    return Err(format!("the file is bigger than the {} MB fetched by itself: open it to fetch it", ATTACH_AUTO_FETCH_MAX_BYTES / (1024 * 1024)));
                }
                super::index_text(&st, &id2, &lg);
                Ok(len)
            }).await.unwrap_or_else(|e| Err(format!("the text round did not finish: {e}")))
        }
    };
    match outcome {
        Ok(len) => {
            { let mut s = lock(&a.s); s.fetched += 1; s.bytes += len; }
            if id_ok_for_ledger(id) { append_ledger(store, &format!("ok\t{id}\t{len}")); }
        }
        Err(why) => {
            let now = crate::rust_core::now_ms();
            { let mut s = lock(&a.s); s.failed += 1; s.failed_at.insert(id.to_string(), now); }
            crate::log::log_line(log, &format!("attachment {id}: not fetched automatically: {why}"));
            if id_ok_for_ledger(id) { append_ledger(store, &format!("fail\t{now}\t{id}")); }
        }
    }
}
