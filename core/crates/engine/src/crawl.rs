//! sync-full-r1: the background crawl. Once an account's history is stored, every body of every folder it holds is downloaded
//! and kept, INBOX first, newest first, BODY_PREFETCH_BATCH UIDs per `UID FETCH` on one leased session, a batch also capped in
//! bytes. The crawl never makes a view or an action wait: it reads through its own connection, keeps its bodies without
//! redrawing the lists (`Store::set_bodies_quiet`), and before each server call it waits while the owner acts (an action, a
//! draft or a send written in the last CRAWL_QUIET_MS, or `note_owner_activity` from the shell), while the Mac is on battery
//! under the floor, or under serious thermal pressure (`set_background_conditions` from the shell). Each folder's place is kept in
//! its `folder_sync_state` row after every batch (contracts delta 21, `BodyCrawlPlace`), so a stopped crawl resumes where it stood;
//! a `<data dir>/body-crawl.tsv` left by the first version is read once, written to the columns, then removed.
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, Weak};
use std::time::{Duration, Instant};

use contracts::constants::{ATTACHMENT_INDEX_BATTERY_MIN, BODY_PREFETCH_BATCH, EMBED_NOTIFY_MS};
use contracts::{BodyCrawlPlace, CoreEvent, Embedder, FolderProgress, ReplySource, ReplySourceRef, ReplySyncProgress, StreamKind};
use store::TableChange;

use super::{lock, Bodies};

// Sizes this file needs that have no row in docs/CONSTANTS.md yet (proposed in docs/lanes/sync-full-r1.md); each is named here once.
/// Rows of a folder read per page while the crawl walks it (the body test reads each row's header only).
pub const CRAWL_SCAN_ROWS: usize = 200;
/// Bytes asked per `UID FETCH` at most (the messages' RFC822.SIZE): BODY.PEEK[] carries the attachments too, so ten large
/// messages in one answer would weigh hundreds of MB. A single larger message is fetched alone.
pub const CRAWL_FETCH_MAX_BYTES: i64 = 16 * 1024 * 1024;
/// After the owner acts, the crawl makes no new server call for this long (the action's own write-back goes first).
pub const CRAWL_QUIET_MS: u64 = 1_500;
/// While the crawl is held (battery, heat, a hold), it reads its conditions again at least this often.
pub const CRAWL_RECHECK_MS: u64 = 5_000;

/// The tables only the owner's own acts write: an undo entry (every mail action), a server write-back queued, a draft, a send.
const OWNER_TABLES: [&str; 4] = ["undo_entry", "pending_action", "draft", "outbox_entry"];
/// The file the first version kept the places in, read once and removed (the hand-over to the columns).
const CURSOR_FILE: &str = "body-crawl.tsv";

/// What the Mac tells the core about its power and heat (the shell's `set_background_conditions`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Conditions {
    /// None: no battery (a desktop Mac), or not known.
    pub battery_percent: Option<u8>,
    pub charging: bool,
    /// ProcessInfo.thermalState serious or critical.
    pub thermal_serious: bool,
}

/// What one account's crawl did in this launch: rows read, server calls, bodies kept.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct CrawlStats { pub scanned: u64, pub fetches: u64, pub kept: u64 }

/// Why the background (the crawl and the indexer) is paused now: lane reply-engine reads it so draft work respects it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackgroundPause {
    /// A test or the shell holds it (`hold_crawl`).
    Held,
    /// On battery under ATTACHMENT_INDEX_BATTERY_MIN percent, not charging.
    Battery,
    /// ProcessInfo.thermalState serious or critical.
    Heat,
    /// The owner acted in the last CRAWL_QUIET_MS.
    OwnerActive,
}

#[derive(Default)]
struct Run { running: bool, again: bool }

/// A folder's place: below this (date, id) every row was crawled; `done` once the walk reached the folder's oldest row.
#[derive(Debug, Clone, PartialEq)]
struct Place { below: (i64, String), done: bool }

#[derive(Default)]
pub struct CrawlState {
    runs: Mutex<HashMap<String, Run>>,
    places: Mutex<HashMap<(String, String), Place>>,
    loaded: AtomicBool,
    conditions: Mutex<Conditions>,
    quiet_until: Mutex<Option<Instant>>,
    held: AtomicBool,
    wake: tokio::sync::Notify,
    stats: Mutex<HashMap<String, CrawlStats>>,
    watching: AtomicBool,
    /// The last progress report (at most one per EMBED_NOTIFY_MS) and what each mail folder source was told.
    reported: Mutex<(Option<Instant>, HashMap<(String, String), (u64, ReplySyncProgress)>)>,
    /// One report at a time, from reading the figures to writing them (round 3: two reports, from the crawl and the indexer, could
    /// write an older figure after a newer one, leaving a folder "syncing" after its work ended).
    reporting: Mutex<()>,
    /// fix-realcore round 12 (main111 `--slice-crawl --paused`: 100 of 100 bodies stored, "conditions it sent 100%"): the hidden check's conditions
    /// (`WHOOSH_CORE_CHECK_CONDITIONS`, rig only) are pinned: the shell's own reading of the Mac, sent once at start and on each change, no longer
    /// replaces them (it did, so the held crawl never held on a Mac on its charger).
    pinned: AtomicBool,
    /// Tests only: one report waits this long between reading its figures and writing them, once.
    report_pause: Mutex<Option<Duration>>,
    /// reds-search: the background downloads' turn (the crawl's batch, the prefetch's chunk): a batch is claimed, checked against the
    /// store and fetched while it is held, so two background downloads never send their fetches out of the newest-first order
    /// (engine_crawl_wire saw INBOX tops [40, 20, 30, 10]: the prefetch's 11-20 reached the server before the crawl's 21-30).
    turn: tokio::sync::Mutex<()>,
}

impl CrawlState {
    pub(crate) fn running(&self) -> bool { lock(&self.runs).values().any(|r| r.running) }
}

/// The cores of this process, for the shell's calls (one in the app; tests build several and reach each one directly).
fn registry() -> &'static Mutex<Vec<Weak<Bodies>>> {
    static R: std::sync::OnceLock<Mutex<Vec<Weak<Bodies>>>> = std::sync::OnceLock::new();
    R.get_or_init(|| Mutex::new(Vec::new()))
}

/// What the shell said last, given to a core that registers after it.
fn last_said() -> &'static Mutex<(Conditions, Option<Arc<dyn Embedder>>)> {
    static L: std::sync::OnceLock<Mutex<(Conditions, Option<Arc<dyn Embedder>>)>> = std::sync::OnceLock::new();
    L.get_or_init(|| Mutex::new((Conditions::default(), None)))
}

/// bg-hooks round 2: the shell handed over None: it looked for the meaning model's files and found none (the model is not installed on this Mac).
fn model_missing() -> &'static AtomicBool {
    static M: AtomicBool = AtomicBool::new(false);
    &M
}

/// model-late: the shell said at launch that it is looking for the model's files and will hand the model over, or say it is not on this
/// Mac (`expect_background_embedder`). A core that was never told this (a test core, a process with no shell) treats "no model handed"
/// as "no model on this Mac", as before.
fn model_coming_said() -> &'static AtomicBool {
    static C: AtomicBool = AtomicBool::new(false);
    &C
}

/// model-late: the model may still come (the shell said it is looking, and has not said "missing" since): a search with no model yet
/// sends its word list as stage "words" and ranks again when the model is handed over.
pub(crate) fn model_coming() -> bool { model_coming_said().load(Ordering::SeqCst) && !model_missing().load(Ordering::SeqCst) }

/// model-late: the shell's last word on the model was "not on this Mac" (a None hand-over).
pub(crate) fn model_said_missing() -> bool { model_missing().load(Ordering::SeqCst) }

/// The shell, at launch, before it looks for the model's files: the answer (`set_background_embedder`, Some or None) will follow. Until
/// it does, a search says nothing about a missing model; it sends the word list and ranks again when the model is handed over.
#[uniffi::export]
pub fn expect_background_embedder() {
    model_coming_said().store(true, Ordering::SeqCst);
    model_missing().store(false, Ordering::SeqCst);
}

/// What the Sources pane says of a mail folder while the meaning model is not on this Mac.
pub const MODEL_MISSING_WORDS: &str = "The meaning model is not on this Mac. Search by words works; search by meaning needs the model.";

fn live_cores() -> Vec<Arc<Bodies>> {
    let mut all = lock(registry());
    all.retain(|w| w.strong_count() > 0);
    all.iter().filter_map(Weak::upgrade).collect()
}

/// The indexer of the core that owns `store` is told these mails have new text to embed: an attachment's text that landed after its mail was
/// indexed (attachment_download.rs `index_text`). Without this the file's pieces waited for the next re-index of the mail.
pub(crate) fn enqueue_for_store(store: &Arc<store::Store>, ids: &[String]) {
    for b in live_cores() { if Arc::ptr_eq(&b.store, store) { b.indexer.enqueue(ids); } }
}

/// The shell (a non-contract seam, not in WhooshCore): the Mac's battery and heat, on every change (IOPowerSources,
/// ProcessInfo.thermalStateDidChangeNotification). The crawl and the indexer wait while on battery under
/// ATTACHMENT_INDEX_BATTERY_MIN percent, or at a serious or critical thermal state.
#[uniffi::export]
pub fn set_background_conditions(battery_percent: Option<u8>, charging: bool, thermal_serious: bool) {
    let c = Conditions { battery_percent, charging, thermal_serious };
    lock(last_said()).0 = c;
    for b in live_cores() { b.shell_conditions(c); }
}

/// The hidden check only (`WHOOSH_CORE_CHECK_CONDITIONS`, read by rust_core.rs under the rig guard): `battery:<percent>` (on battery, not charging) and/or
/// `heat` (serious thermal pressure), comma between. None for any other text, an empty one or a percent over 100: the copy then keeps its real conditions.
pub(crate) fn conditions_from_check(value: &str) -> Option<Conditions> {
    let mut c = Conditions::default();
    let mut any = false;
    for part in value.split(',').map(str::trim) {
        if let Some(percent) = part.strip_prefix("battery:") {
            match percent.trim().parse::<u8>() {
                Ok(n) if n <= 100 => { c.battery_percent = Some(n); c.charging = false; any = true; }
                _ => return None,
            }
        } else if part == "heat" {
            c.thermal_serious = true;
            any = true;
        } else {
            return None;
        }
    }
    any.then_some(c)
}

/// The shell: the owner pressed a key or clicked in the app. The crawl makes no new server call for CRAWL_QUIET_MS.
#[uniffi::export]
pub fn note_owner_activity() {
    for b in live_cores() { b.note_owner_activity(); }
}

/// The shell: the on-device embedding model the meaning index embeds with (None takes it away). Until it is given, the indexer
/// keeps the word index only; once given, every stored body without vectors is embedded in the background.
#[uniffi::export]
pub fn set_background_embedder(embedder: Option<Arc<dyn Embedder>>) {
    lock(last_said()).1 = embedder.clone();
    // the shell says "no model" by handing over None (at launch, when the weights are not on this Mac): the Sources pane then says so
    model_missing().store(embedder.is_none(), Ordering::SeqCst);
    for b in live_cores() {
        b.indexer.set_embedder(embedder.clone());
        // model-late: the search waiting for the model keeps its word list, stage no_model (a Some hand-over ranks it again, indexer.rs)
        if embedder.is_none() { b.indexer.said_missing(); }
        b.report_progress(true);
    }
}

/// pieces (SPEC A151): the folder the shell's embedding model loads from (its tokenizer.json sizes the meaning index's pieces in the
/// model's own tokens). Kept for the process; the indexer reads it when it builds its counter (indexer.rs `counter`). None: the
/// indexer falls back to the installer's folder beside the data folder, then to the byte bound.
fn model_folder() -> &'static Mutex<Option<String>> {
    static F: std::sync::OnceLock<Mutex<Option<String>>> = std::sync::OnceLock::new();
    F.get_or_init(|| Mutex::new(None))
}

/// pieces: the folder the handed model reads (None: unknown). Read by the indexer.
pub(crate) fn embedding_model_folder() -> Option<String> { lock(model_folder()).clone() }

/// The shell, with each hand-over of the model: the folder its weights load from (None when it has none). Generated Swift name:
/// `setEmbeddingModelFolder(path:)`.
#[uniffi::export]
pub fn set_embedding_model_folder(path: Option<String>) {
    *lock(model_folder()) = path.filter(|p| !p.trim().is_empty());
}

/// A row of the folder whose body is not stored yet.
#[derive(Debug, Clone)]
struct Missing { uid: u32, id: String, received_at_ms: i64, size: i64 }

/// Up to BODY_PREFETCH_BATCH rows per server call, and no more than CRAWL_FETCH_MAX_BYTES of them (a larger one goes alone).
fn batches(missing: &[Missing]) -> Vec<Vec<&Missing>> {
    let mut out: Vec<Vec<&Missing>> = Vec::new();
    let mut bytes = 0i64;
    for row in missing {
        let size = row.size;
        let full = out.last().is_none_or(|b| b.len() >= BODY_PREFETCH_BATCH as usize || (bytes + size > CRAWL_FETCH_MAX_BYTES && !b.is_empty()));
        if full { out.push(Vec::new()); bytes = 0; }
        bytes += size;
        if let Some(b) = out.last_mut() { b.push(row); }
    }
    out
}

fn now_ms() -> i64 { std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as i64).unwrap_or(0) }

/// INBOX first, then the other folders by name.
fn ordered(mut folders: Vec<String>) -> Vec<String> {
    folders.sort_by_key(|f| (!f.eq_ignore_ascii_case("INBOX"), f.clone()));
    folders
}

impl Bodies {
    fn cursor_path(&self) -> PathBuf { self.store.path().with_file_name(CURSOR_FILE) }

    /// Called when the core opens (rust_core.rs) and at each crawl kick: a core hears the shell's word on the model (handed over, or not on
    /// this Mac) from its first moment, also with no account and before any crawl (engine_model_late test a waited 120 s for a word that
    /// reached no core).
    pub(crate) fn register(self: &Arc<Self>) {
        let mut all = lock(registry());
        all.retain(|w| w.strong_count() > 0);
        if all.iter().any(|w| std::ptr::eq(w.as_ptr(), Arc::as_ptr(self))) { return; }
        all.push(Arc::downgrade(self));
        drop(all);
        let (c, e) = lock(last_said()).clone();
        if c != Conditions::default() { self.shell_conditions(c); }
        if e.is_some() { self.indexer.set_embedder(e); }
        let me = Arc::downgrade(self);
        self.indexer.after_step(Box::new(move |drained| if let Some(b) = me.upgrade() { b.report_progress(drained); }));
    }

    /// The owner's acts are seen in the store's change feed: a table only they write makes the crawl quiet. The sink only sets a
    /// time; it runs on the store's writer thread, inside the act's own write, so it must never wait.
    fn watch_owner(self: &Arc<Self>) {
        if self.crawl.watching.swap(true, Ordering::SeqCst) { return; }
        let me = Arc::downgrade(self);
        self.store.subscribe_tables(Arc::new(move |c: &TableChange| {
            if !OWNER_TABLES.contains(&c.table.as_str()) { return; }
            if let Some(b) = me.upgrade() { b.note_owner_activity(); }
        }));
    }

    pub fn set_conditions(&self, c: Conditions) {
        *lock(&self.crawl.conditions) = c;
        self.indexer.set_held(Self::held_by(c));
        self.crawl.wake.notify_waiters();
    }

    /// The shell's reading of the Mac: taken unless the hidden check pinned its own (`pin_conditions`).
    fn shell_conditions(&self, c: Conditions) {
        if self.crawl.pinned.load(Ordering::SeqCst) { return; }
        self.set_conditions(c);
    }

    /// The hidden check only (rust_core.rs, `WHOOSH_CORE_CHECK_CONDITIONS`, rig copies): these conditions hold for this core's life, whatever the shell
    /// reports later.
    pub(crate) fn pin_conditions(&self, c: Conditions) {
        self.crawl.pinned.store(true, Ordering::SeqCst);
        self.set_conditions(c);
    }

    /// fix-realcore round 13: the background is held by battery, heat or a hold (not by the owner's own activity, which only spaces the calls).
    pub(crate) fn held_now(&self) -> bool {
        matches!(self.paused(), Some(BackgroundPause::Held | BackgroundPause::Battery | BackgroundPause::Heat))
    }

    /// fix-realcore round 13: `background/pause`, the engine's own word for why the background waits now (`held`, `battery`, `heat`, `owner_active`),
    /// or null when it runs. The shell's row derives its state from what the shell itself measured; this is what the engine applies.
    pub(crate) fn pause_state(&self) -> String {
        match self.paused() {
            Some(BackgroundPause::Held) => "\"held\"".into(),
            Some(BackgroundPause::Battery) => "\"battery\"".into(),
            Some(BackgroundPause::Heat) => "\"heat\"".into(),
            Some(BackgroundPause::OwnerActive) => "\"owner_active\"".into(),
            None => "null".into(),
        }
    }

    fn held_by(c: Conditions) -> bool {
        c.thermal_serious || (!c.charging && c.battery_percent.is_some_and(|p| u64::from(p) < ATTACHMENT_INDEX_BATTERY_MIN))
    }

    /// The owner acted: no new server call for CRAWL_QUIET_MS.
    pub fn note_owner_activity(&self) {
        *lock(&self.crawl.quiet_until) = Some(Instant::now() + Duration::from_millis(CRAWL_QUIET_MS));
    }

    /// Tests and the shell: hold the crawl (no new server call) or let it go on.
    pub fn hold_crawl(&self, held: bool) {
        self.crawl.held.store(held, Ordering::SeqCst);
        self.crawl.wake.notify_waiters();
    }

    /// Tests: hold the crawl and wait until the batch it is downloading is kept (true), or `within` passed (false).
    pub fn hold_crawl_and_wait(&self, within: Duration) -> bool {
        self.hold_crawl(true);
        let t0 = Instant::now();
        while t0.elapsed() < within {
            if lock(&self.in_flight).is_empty() { return true; }
            std::thread::sleep(Duration::from_millis(10));
        }
        false
    }

    /// The meaning index's model for this core (the shell's `set_background_embedder` gives it to every core).
    pub fn set_embedder(&self, embedder: Option<Arc<dyn Embedder>>) { self.indexer.set_embedder(embedder); }

    /// These stored messages join the indexes again (their text changed, or a caller wants them current); returns at once.
    pub fn reindex(&self, ids: &[String]) { self.indexer.enqueue(ids); }

    /// Tests only: the indexer waits `d` between taking each job and running it.
    pub fn indexer_pause_after_take(&self, d: Option<Duration>) { self.indexer.pause_after_take(d); }

    /// Tests: hold the indexer (no step) or let it go on.
    pub fn hold_index(&self, held: bool) { self.indexer.set_held(held); }

    /// Waits until the indexer has nothing left to do (true), or `within` passed (false).
    pub fn settle_index(&self, within: Duration) -> bool { self.indexer.settle(within) }

    /// indexer-race, tests: opens the word index now with nothing queued (true once it is open).
    pub fn open_word_index(&self) -> bool { self.indexer.open_words() }

    /// The word search over every stored body.
    pub fn search_words(&self, query: &str, limit: usize) -> Vec<contracts::SearchHit> { self.indexer.search_words(query, limit) }

    /// The meaning search: the closest chunks with their message's date, sender, folder and subject (what AI replies retrieve).
    pub fn search_meaning(&self, query: &str, k: usize) -> Result<Vec<super::indexer::ChunkHit>, String> { self.indexer.search_meaning(query, k) }

    /// The folder roles the sync read at its last LIST (tests set them for an account the sync did not run).
    pub fn set_roles(&self, roles: crate::imap_source::Roles) { *lock(&self.roles) = Some(roles); }

    /// The account's Archive folder, as its server named it.
    fn is_archive(&self, account: &str, folder: &str) -> bool {
        let roles = lock(&self.roles).clone();
        roles.is_some_and(|r| lock(&r).get(account).and_then(|m| m.get(folder)).is_some_and(|role| *role == sync_imap::FolderRole::Archive))
    }

    /// A server without UIDPLUS named no place for the core's E: the copy its Archive folder holds is linked to the owner's archived row
    /// (`Store::link_archived_copies`), so the Archive view lists one row and the way back names the copy. Needs the folder's UIDVALIDITY.
    fn link_archive(&self, account: &str, folder: &str) {
        if !self.is_archive(account, folder) { return; }
        let Ok(Some((uid_validity, _))) = self.store.imap_cursor(account, folder) else { return };
        if let Err(e) = self.store.link_archived_copies(account, folder, uid_validity) {
            crate::log::log_line(&self.log, &format!("crawl {account}: the Archive copies were not linked: {e}"));
        }
    }

    /// Whether the background is paused now, and why (None: it runs). The one public read of the pause: the crawl waits on it before
    /// each server call; lane reply-engine reads it before draft work.
    pub fn paused(&self) -> Option<BackgroundPause> {
        if self.crawl.held.load(Ordering::SeqCst) { return Some(BackgroundPause::Held); }
        let c = *lock(&self.crawl.conditions);
        if c.thermal_serious { return Some(BackgroundPause::Heat); }
        if Self::held_by(c) { return Some(BackgroundPause::Battery); }
        if self.quiet_left().is_some() { return Some(BackgroundPause::OwnerActive); }
        None
    }

    fn quiet_left(&self) -> Option<Duration> {
        let until = *lock(&self.crawl.quiet_until);
        until.and_then(|u| u.checked_duration_since(Instant::now())).filter(|d| !d.is_zero())
    }

    /// How long the crawl waits before its next server call (None: it may go now).
    fn crawl_wait(&self) -> Option<Duration> {
        match self.paused()? {
            BackgroundPause::OwnerActive => self.quiet_left(),
            _ => Some(Duration::from_millis(CRAWL_RECHECK_MS)),
        }
    }

    /// Waits until the crawl may make its next server call.
    async fn crawl_gate(&self) {
        loop {
            let woken = self.crawl.wake.notified();
            tokio::pin!(woken);
            woken.as_mut().enable();
            let Some(wait) = self.crawl_wait() else { return };
            tokio::select! { _ = tokio::time::sleep(wait) => {}, _ = woken => {} }
        }
    }

    /// Starts the account's crawl on the core's runtime (one run at a time; a kick during a run walks again after it). The sync
    /// kicks it once the account's history is stored and after each change it applies.
    pub fn crawl_kick(self: &Arc<Self>, account: &str) {
        self.register();
        self.watch_owner();
        {
            let mut runs = lock(&self.crawl.runs);
            let r = runs.entry(account.to_string()).or_default();
            if r.running { r.again = true; return; }
            r.running = true;
        }
        let (me, account) = (self.clone(), account.to_string());
        self.runtime.spawn(async move { me.crawl_run(account).await });
    }

    async fn crawl_run(self: Arc<Self>, account: String) {
        loop {
            if let Err(why) = self.crawl_pass(&account).await {
                crate::log::log_line(&self.log, &format!("crawl {account}: {why}"));
            }
            self.report_progress(true);
            let mut runs = lock(&self.crawl.runs);
            let r = runs.entry(account.clone()).or_default();
            if r.again { r.again = false; continue; }
            r.running = false;
            return;
        }
    }

    fn stat(&self, account: &str, f: impl FnOnce(&mut CrawlStats)) { f(lock(&self.crawl.stats).entry(account.to_string()).or_default()); }

    pub fn crawl_stats(&self, account: &str) -> CrawlStats { lock(&self.crawl.stats).get(account).copied().unwrap_or_default() }

    /// One walk over every folder of the account. Stops (Err) when the account has no server connection or the server refused:
    /// the folder's place is kept, so the next kick resumes there.
    async fn crawl_pass(self: &Arc<Self>, account: &str) -> Result<(), String> {
        self.load_places();
        let reads = self.store.background_reads().map_err(|e| format!("the store did not open a read: {e}"))?;
        let folders = ordered(reads.folders(account).map_err(|e| e.to_string())?);
        for folder in folders {
            self.link_archive(account, &folder);
            let key = (account.to_string(), folder.clone());
            loop {
                let below = lock(&self.crawl.places).get(&key).map(|p| p.below.clone());
                let page = reads.crawl_page(account, &folder, below, CRAWL_SCAN_ROWS).map_err(|e| e.to_string())?;
                self.stat(account, |s| s.scanned += page.len() as u64);
                let Some(last) = page.last() else {
                    self.set_done(&key);
                    break;
                };
                let last = (last.received_at_ms, last.message_id.clone());
                let missing: Vec<Missing> = page.iter().filter(|r| !r.has_body)
                    .map(|r| Missing { uid: r.uid, id: r.message_id.clone(), received_at_ms: r.received_at_ms, size: r.size_bytes.unwrap_or(0).max(0) }).collect();
                for batch in batches(&missing) {
                    self.crawl_gate().await;
                    self.after_readers().await;      // fix-realcore round 2: the mail the owner opened goes before the next batch
                    let _turn = self.background_turn().await;
                    let claimed = self.claim(batch.iter().map(|r| (r.uid, r.id.clone())).collect());
                    // reds-search: the page was read before this batch; a body another download kept since (the prefetch runs beside
                    // the crawl) is not asked again (engine_crawl_wire saw INBOX tops [40, 30, 20, 10, 20, 10]: 11-20 and 1-10 twice)
                    let claimed = self.drop_stored(claimed);
                    if !claimed.is_empty() {
                        if self.source(account).is_none() { return Err("this account is not connected to its mail server yet".into()); }
                        self.stat(account, |s| s.fetches += 1);
                        let n = claimed.len() as u64;
                        if !self.download_with(account, &folder, claimed, true).await {
                            return Err(format!("the server did not send the bodies of {folder}; the crawl resumes there at the next sync"));
                        }
                        self.stat(account, |s| s.kept += n);
                    }
                    if let Some(end) = batch.last() { self.set_place(&key, (end.received_at_ms, end.id.clone())); }
                    self.report_progress(false);
                }
                self.set_place(&key, last);
            }
        }
        Ok(())
    }

    /// reds-search: the claimed rows whose body is stored now are given back (out of flight, as `keep` does after a fetch, the
    /// threads a reader waits for announced); the rest are returned to be fetched. A row the store cannot read stays to be fetched.
    pub(crate) fn drop_stored(&self, claimed: Vec<(u32, String)>) -> Vec<(u32, String)> {
        if claimed.is_empty() { return claimed; }
        // fix-realcore round 12 (main111 --slice-bodygone: the gone UID asked 5 times, checked twice): a row the store no longer holds (another
        // download found its mail gone and removed it, `Bodies::left_the_folder`, after this list was read) is given back like a stored one,
        // never asked again. A read that fails keeps the row to be fetched.
        let stored = |id: &str| match self.store.body_parts(id) {
            Ok(Some(p)) => p.body_text.is_some() || p.body_html.is_some(),
            Ok(None) => true,
            Err(_) => false,
        };
        let (have, fetch): (Vec<(u32, String)>, Vec<(u32, String)>) = claimed.into_iter().partition(|(_, id)| stored(id));
        if have.is_empty() { return fetch; }
        let asked: Vec<String> = {
            let mut flying = lock(&self.in_flight);
            for (_, id) in &have { flying.remove(id); lock(&self.reader_asked).remove(id); }
            let mut wanted = lock(&self.wanted);
            have.iter().filter(|(_, id)| wanted.remove(id)).map(|(_, id)| id.clone()).collect()
        };
        if !asked.is_empty() { self.announce(&asked); }
        fetch
    }

    /// fix-realcore round 2: waits while a download the reader asked for is waiting or running (`Bodies::request`), so the next background
    /// batch is never sent ahead of the mail the owner opened; the batch already in flight finishes first. Returns at once when none is.
    pub(crate) async fn after_readers(&self) {
        loop {
            let done = self.readers_done.notified();
            tokio::pin!(done);
            done.as_mut().enable();
            if self.readers.load(Ordering::SeqCst) == 0 { return; }
            done.await;
        }
    }

    /// reds-search: waits for the background downloads' turn (held over one claim, store check and fetch; see `CrawlState::turn`). The
    /// reader's own download (`request`) never takes it, so an opened mail never waits behind the background.
    pub(crate) async fn background_turn(&self) -> tokio::sync::MutexGuard<'_, ()> { self.crawl.turn.lock().await }

    /// The folder is walked down to `below`: kept in memory and in its `folder_sync_state` row.
    fn set_place(&self, key: &(String, String), below: (i64, String)) {
        lock(&self.crawl.places).insert(key.clone(), Place { below: below.clone(), done: false });
        self.write_place(BodyCrawlPlace { account_id: key.0.clone(), folder: key.1.clone(), before_ms: Some(below.0), before_id: Some(below.1), done_ms: None });
    }

    /// The walk reached the folder's oldest row: in memory the place stays at the bottom (the history may add older rows in this
    /// launch); the row says done and holds no place, so the next launch walks the folder again from its newest row (headers only),
    /// and a row moved into it later is not missed.
    fn set_done(&self, key: &(String, String)) {
        if let Some(p) = lock(&self.crawl.places).get_mut(key) { p.done = true; }
        self.write_place(BodyCrawlPlace { account_id: key.0.clone(), folder: key.1.clone(), before_ms: None, before_id: None, done_ms: Some(now_ms()) });
    }

    fn write_place(&self, place: BodyCrawlPlace) {
        if let Err(e) = self.store.set_crawl_place(place) { crate::log::log_line(&self.log, &format!("crawl: the place was not kept: {e}")); }
    }

    /// Once per launch: a `body-crawl.tsv` the first version left ("account TAB folder TAB date TAB id" per folder being walked) is
    /// written to the columns and removed; then the places are read from the columns (a finished walk has none: it starts again).
    fn load_places(&self) {
        if self.crawl.loaded.swap(true, Ordering::SeqCst) { return; }
        if let Ok(text) = std::fs::read_to_string(self.cursor_path()) {
            let mut kept = true;
            for line in text.lines() {
                let cells: Vec<&str> = line.split('\t').collect();
                let [a, f, at, id] = cells.as_slice() else { continue };
                let Ok(at) = at.parse::<i64>() else { continue };
                let place = BodyCrawlPlace { account_id: a.to_string(), folder: f.to_string(), before_ms: Some(at), before_id: Some(id.to_string()), done_ms: None };
                if let Err(e) = self.store.set_crawl_place(place) { kept = false; crate::log::log_line(&self.log, &format!("crawl: a place of {} was not handed over: {e}", CURSOR_FILE)); }
            }
            // the file goes only once every place is in the columns (a failed write keeps it for the next launch)
            if kept { if let Err(e) = std::fs::remove_file(self.cursor_path()) { crate::log::log_line(&self.log, &format!("crawl: {CURSOR_FILE} was not removed: {e}")); } }
        }
        let places = match self.store.background_reads().and_then(|r| r.crawl_places()) {
            Ok(p) => p,
            Err(e) => { crate::log::log_line(&self.log, &format!("crawl: the places were not read: {e}")); return; }
        };
        let mut mem = lock(&self.crawl.places);
        for p in places {
            if let (Some(at), Some(id), None) = (p.before_ms, p.before_id, p.done_ms) {
                mem.entry((p.account_id, p.folder)).or_insert(Place { below: (at, id), done: false });
            }
        }
    }

    /// True while the shell says the meaning model is not on this Mac.
    pub fn meaning_model_missing(&self) -> bool { model_missing().load(Ordering::SeqCst) }

    /// `base` (the folder's progress), except that an enabled folder with mail not yet embedded says in words that the model is
    /// missing, when it is (bg-hooks round 2).
    pub(crate) fn model_note(&self, p: &FolderProgress, enabled: bool, base: ReplySyncProgress) -> ReplySyncProgress {
        if enabled && self.meaning_model_missing() && p.indexed < p.messages { ReplySyncProgress::Failed { reason_words: MODEL_MISSING_WORDS.into() } } else { base }
    }

    /// The contract's `folder_progress_source` with `model_note` on top. The Sources pane's other writer (`replies_work::sync_sources::mail_folders`) calls this.
    pub fn folder_source(&self, p: FolderProgress, enabled: bool) -> ReplySource {
        let mut source = contracts::folder_progress_source(p.clone(), enabled);
        source.progress = self.model_note(&p, enabled, source.progress);
        source
    }

    /// Reports each mail folder source's progress (the contract's `folder_sync_progress`) to the store (`reply_source_progress`) and as
    /// a `ReplySourceProgress` event on the SyncStatus stream, at most once per EMBED_NOTIFY_MS (`force`: now, at the end of a walk),
    /// and only for a source whose figure changed. Reads through the background connection.
    pub(crate) fn report_progress(&self, force: bool) {
        let _one = lock(&self.crawl.reporting);
        {
            let mut r = lock(&self.crawl.reported);
            if !force && r.0.is_some_and(|t| t.elapsed() < Duration::from_millis(EMBED_NOTIFY_MS)) { return; }
            r.0 = Some(Instant::now());
        }
        let (Ok(rows), Ok(sources)) = (self.progress(), self.store.reply_sources()) else { return };
        let pause = lock(&self.crawl.report_pause).take();
        if let Some(d) = pause { std::thread::sleep(d); }
        let synced = self.store.reply_folder_first_sync().unwrap_or_default(); // replies-settings: an empty folder whose first sync ended is up to date (crate::replies::mail_folder_progress)
        for s in sources {
            let ReplySourceRef::MailFolder { account_id, folder } = &s.source else { continue };
            let p = rows.iter().find(|p| &p.account_id == account_id && &p.folder == folder).cloned()
                .unwrap_or(FolderProgress { account_id: account_id.clone(), folder: folder.clone(), messages: 0, bodies: 0, indexed: 0 });
            let key = (account_id.clone(), folder.clone());
            let progress = self.model_note(&p, s.enabled, crate::replies::mail_folder_progress(&p, s.enabled, synced.contains(&key)));   // bg-hooks r2 on top of replies-settings
            let now = (p.messages, progress.clone());
            if lock(&self.crawl.reported).1.get(&key) == Some(&now) { continue; }
            if let Err(e) = self.store.reply_source_progress(s.source.clone(), p.messages, progress.clone(), now_ms()) {
                crate::log::log_line(&self.log, &format!("crawl: the progress of {folder} was not kept: {e}"));
                continue;
            }
            lock(&self.crawl.reported).1.insert(key, now);
            crate::sync::emit(&self.sinks, StreamKind::SyncStatus, CoreEvent::ReplySourceProgress { source: s.source, item_count: p.messages, progress });
        }
    }

    /// Reports every mail folder source's progress now (the report the crawl and the indexer make on their own; tests and the shell).
    pub fn report_progress_now(&self) { self.report_progress(true); }

    /// Tests only: the next progress report waits `d` between reading its figures and writing them.
    pub fn pause_next_report(&self, d: Duration) { *lock(&self.crawl.report_pause) = Some(d); }

    /// Waits until no crawl runs (true), or `within` passed (false).
    pub fn settle_crawl(&self, within: Duration) -> bool {
        let t0 = Instant::now();
        while t0.elapsed() < within {
            if !self.crawl.running() { return true; }
            std::thread::sleep(Duration::from_millis(10));
        }
        false
    }

    /// Per account and folder: messages stored, bodies stored, and bodies whose chunks are all in the meaning index (the contract's
    /// `FolderProgress`, the same figures as state `reply/progress`). Reads through the background connection, so the pane that shows
    /// it never makes a view wait.
    pub fn progress(&self) -> Result<Vec<FolderProgress>, String> {
        let rows = self.store.background_reads().and_then(|r| r.folder_progress()).map_err(|e| e.to_string())?;
        for p in &rows { contracts::check_folder_progress(p.clone()).map_err(|e| e.to_string())?; }
        Ok(rows)
    }

    /// fix-blockforget (docs/lanes/real-accounts.md finding 2): an account was removed from this Mac. The crawl forgets what it held of it in
    /// memory (each folder's place, this launch's figures, the progress last told per folder; its folder rows went with the account), and
    /// the indexer forgets its bodies and their vectors (`Indexer::forget_account`, with what `Store::account_index_rows` read before the
    /// removal). Returns how many vectors left the meaning index. No store write, no server call, no wait.
    pub fn forget_account(&self, account_id: &str, message_ids: &[String], chunk_ids: &[i64]) -> usize {
        lock(&self.crawl.places).retain(|(account, _), _| account != account_id);
        lock(&self.crawl.stats).remove(account_id);
        lock(&self.crawl.reported).1.retain(|(account, _), _| account != account_id);
        self.indexer.forget_account(message_ids, chunk_ids)
    }
}
