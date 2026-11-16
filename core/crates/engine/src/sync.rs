//! The real core's mail sync (slice 2): each added account syncs on the core's own runtime. The sync engine runs the
//! account's first screen (the newest page of every folder), then its history in the background, newest to oldest;
//! then the account holds one IDLE connection on INBOX and applies each change through CONDSTORE/QRESYNC (or, on a
//! server without them, re-reads the newest page). A failure lands on the account's sync state in the server's words.
use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};

use contracts::{Account, AccountConfig, ActionFailed, Clock, ConnectionSecurity, CoreEvent, EventSink, StreamKind, SyncCursor};
use store::Store;
use sync_engine::{words, Engine, EngineConfig, Outcome, Resume};
use sync_imap::{Auth, IdleEvent, ImapAdapter, ImapConfig, ImapLimits, ImapMailAdapter, Resync, Security, TokioSleeper};
use tokio::sync::{mpsc, Notify};

use crate::bodies::Bodies;
use crate::consts::*;
use crate::filing_worker::Filing;
use crate::imap_source::{ImapSource, Roles, OPTS};

/// Where the sync of one account stands, for the core's reads and its tests.
#[derive(Debug, Clone, PartialEq)]
pub enum SyncPhase {
    /// The first screen and the history are being fetched.
    Fetching,
    /// The history is stored; the account listens for changes (or has stopped listening, in the stored words).
    Listening,
    /// The engine stopped the account; the failure is on its sync state.
    Failed(ActionFailed),
}

/// The sinks of the core's own events (SyncStatus), shared with the engine.
pub type Sinks = Arc<Mutex<(u64, HashMap<u64, (StreamKind, Arc<dyn EventSink>)>)>>;

pub fn emit(sinks: &Sinks, stream: StreamKind, event: CoreEvent) {
    let list: Vec<Arc<dyn EventSink>> = sinks.lock().map(|g| g.1.values().filter(|(k, _)| *k == stream).map(|(_, s)| s.clone()).collect()).unwrap_or_default();
    for s in list { s.on_event(event.clone()); }
}

/// The engine's events, handed to whoever subscribed to the SyncStatus stream. The engine announces each page AFTER storing it, so a
/// state change is also the moment the messages of that page can run the label flow (delta 17: `Filing::kick`) and the account's
/// newest bodies can be downloaded (body-r1: `Bodies::kick`, after the page the list paints from is in).
struct Forward(Sinks, Arc<Filing>, Arc<Bodies>, String);
impl EventSink for Forward {
    fn on_event(&self, event: CoreEvent) {
        if matches!(event, CoreEvent::SyncStateChanged { .. }) { self.1.kick(); self.2.kick(&self.3); }
        emit(&self.0, StreamKind::SyncStatus, event);
    }
}

pub struct SystemClock;
impl Clock for SystemClock {
    fn now_ms(&self) -> i64 { std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as i64).unwrap_or(0) }
    fn utc_offset_s(&self) -> i32 { 0 }
}

/// The connections Whoosh itself opens per account, IDLE included (docs/CONSTANTS.md IMAP_CONN_PER_ACCOUNT: "Whoosh itself opens
/// IMAP_CONN_PER_ACCOUNT, which this cap bounds" on the IMAP_MAX_CONNECTIONS_* rows).
const WHOOSH_CONNECTIONS: usize = contracts::constants::IMAP_CONN_PER_ACCOUNT as usize;

/// The ledger's numbers for the IMAP connections (docs/CONSTANTS.md, through build.rs).
/// fix-realcore (main92 s2-007.log `one-connection`: 3 sessions open at once): the adapter's pool and its permits are the sessions the
/// account may hold open, the IDLE one included (idle.rs leases a permit). The provider's cap was handed in alone (5, Gmail 15), so the
/// sync's two workers, the background body download and the IDLE session together opened 3 and more; the pool is now Whoosh's own number
/// under the provider's cap: the IDLE session and one more.
pub fn limits() -> ImapLimits {
    ImapLimits {
        max_default: IMAP_MAX_CONNECTIONS_DEFAULT.min(WHOOSH_CONNECTIONS),
        max_gmail: IMAP_MAX_CONNECTIONS_GMAIL.min(WHOOSH_CONNECTIONS),
        idle_per_account: IMAP_IDLE_CONNECTIONS_PER_ACCOUNT,
        backoff_start_ms: SYNC_BACKOFF_START_MS,
        backoff_max_ms: SYNC_BACKOFF_MAX_MS,
        transcript_lines: IMAP_TRANSCRIPT_LINES,
    }
}

pub fn engine_config() -> EngineConfig { EngineConfig::from_ledger(SYNC_HISTORY_PAGE, SYNC_BACKOFF_START_MS, SYNC_BACKOFF_MAX_MS, SYNC_RETRY_COUNT) }

/// What the sync of one account needs from the core.
pub struct SyncDeps {
    pub store: Arc<Store>,
    pub sinks: Sinks,
    pub roles: Roles,
    /// Delta 17: the worker that runs the labelling order on what the sync stores.
    pub(crate) filing: Arc<Filing>,
    pub phases: Arc<Mutex<HashMap<String, SyncPhase>>>,
    pub adapters: Arc<Mutex<HashMap<String, Arc<ImapAdapter>>>>,
    /// body-r1: the background body downloads (the reader's asks and the prefetch after each fetch). The adapters map keeps the
    /// concrete adapter (the write-back and the outbox call its own methods); the body source is registered here beside it.
    pub(crate) bodies: Arc<Bodies>,
    /// Clear text on a loopback host only: the in-process fakes of the tests and the hidden check (never the owner's copy).
    pub plain_loopback: bool,
    pub never_store: bool,
}

pub fn is_loopback(host: &str) -> bool { matches!(host.trim(), "127.0.0.1" | "::1" | "localhost") }

/// The adapter of one account (a password, or an OAuth2 token source). Refused in words for clear text on any host but the loopback.
pub fn adapter_for(account: &Account, config: &AccountConfig, auth: Auth, plain_loopback: bool) -> Result<ImapAdapter, String> {
    let security = if plain_loopback {
        if !is_loopback(&config.imap_host) { return Err(format!("clear text is used on this Mac's own address only, not on {}", config.imap_host)); }
        Security::PlainLoopbackOnly
    } else {
        match config.imap_security { ConnectionSecurity::Ssl => Security::Tls, ConnectionSecurity::Starttls => Security::StartTls }
    };
    Ok(ImapAdapter::new(ImapConfig {
        account_id: account.id.clone(),
        account_name: account.display_name.clone(),
        host: config.imap_host.clone(),
        port: config.imap_port,
        security,
        username: config.username.clone(),
        auth,
        extra_roots_der: vec![],
        limits: limits(),
    }))
}

fn set_phase(d: &SyncDeps, id: &str, p: SyncPhase) { d.phases.lock().unwrap_or_else(|e| e.into_inner()).insert(id.to_string(), p); }

/// Syncs one account to its full history, then listens on INBOX until `stop` is notified. Runs on the core's runtime.
pub async fn run_account(d: Arc<SyncDeps>, account: Account, adapter: Arc<ImapAdapter>, stop: Arc<Notify>) {
    let id = account.id.clone();
    set_phase(&d, &id, SyncPhase::Fetching);
    d.adapters.lock().unwrap_or_else(|e| e.into_inner()).insert(id.clone(), adapter.clone());
    d.bodies.set_source(&id, adapter.clone());
    // lane reply-headers: the same adapter answers the threading headers' fill-in (sync-imap's blocking AccountAdapters)
    d.bodies.headers.set_adapter(&id, adapter.clone());
    // sync-full-r1: the crawl reads which folder is the account's Archive from the roles the sync keeps
    d.bodies.set_roles(d.roles.clone());
    let flagged = Arc::new(Mutex::new(Vec::new()));
    let source = Arc::new(ImapSource {
        account_id: id.clone(), adapter: adapter.clone(), roles: d.roles.clone(), flagged: flagged.clone(), never_store: d.never_store,
        store: d.store.clone(), filing: d.filing.clone(), bodies: d.bodies.clone(),
    });
    let engine = Engine::new(d.store.clone(), Arc::new(Forward(d.sinks.clone(), d.filing.clone(), d.bodies.clone(), id.clone())), Arc::new(SystemClock), engine_config());
    let retry = retry_handle(&id);
    let _forget = ForgetRetry(id.clone());
    // What a run stored is filed now, then the stars the server held are a flipped fact on those messages (delta 17).
    let settle = || {
        let starred: Vec<String> = std::mem::take(&mut *flagged.lock().unwrap_or_else(|e| e.into_inner()));
        d.filing.settle();
        if !starred.is_empty() {
            match d.store.set_starred(starred.clone(), true) {
                Ok(_) => d.filing.flipped(starred),
                Err(e) => eprintln!("sync {id}: could not mark the server's flagged messages starred: {e}"),
            }
        }
    };
    // A failed account is tried again after SYNC_BACKOFF_MAX_MS (when the failure can be retried) and at once on the owner's next
    // action on it (`retry_now`), and says so on its status (`sync.paused` with the seconds).
    let outcome = tokio::select! {
        out = engine.run_resuming(account, source, &retry, |r| match r {
            Resume::Failed(f) => { settle(); set_phase(&d, &id, SyncPhase::Failed(f)); }
            Resume::Again => set_phase(&d, &id, SyncPhase::Fetching),
        }) => Some(out),
        _ = stop.notified() => return,
    };
    settle();
    match outcome {
        // the history is stored: the status the engine ended on (`sync.messages`) is spent, and a listening account carries no status
        // until its connection says something (a healthy account shows no words beside its row)
        Some(Outcome::Complete) => {
            set_phase(&d, &id, SyncPhase::Listening);
            status_words(&d, &id, None);
            // sync-full-r1: the history's headers are all stored: every body of every folder is downloaded in the background now
            if !d.never_store { d.bodies.crawl_kick(&id); }
        }
        _ => return,
    }
    if d.never_store { return; }
    // junk-r2: the history taught, never scored: once per account, its Junk folder as junk and its inbox as not junk, before the first
    // live arrival is scored
    crate::junk_filter::bootstrap(&d, &id);
    listen(d, id, adapter, stop).await;
}

/// The status words while the IDLE connection is being opened again after the server closed it (a BYE, a dropped socket).
pub fn paused_words(reason_words: &str, retry_in_ms: u64) -> String {
    format!("New mail is paused: {reason_words}. Reconnecting in {} s.", retry_in_ms.div_ceil(1000))
}

/// The same status with its code: `sync.paused` with the seconds to the reconnect and the server's words.
pub fn paused_status(reason_words: &str, retry_in_ms: u64) -> String {
    words::paused_status(&paused_words(reason_words, retry_in_ms), retry_in_ms.div_ceil(1000), reason_words)
}

type Retries = Mutex<HashMap<String, Arc<Notify>>>;

fn retries() -> &'static Retries {
    static R: OnceLock<Retries> = OnceLock::new();
    R.get_or_init(|| Mutex::new(HashMap::new()))
}

fn retry_handle(id: &str) -> Arc<Notify> {
    retries().lock().unwrap_or_else(|e| e.into_inner()).entry(id.to_string()).or_insert_with(|| Arc::new(Notify::new())).clone()
}

struct ForgetRetry(String);
impl Drop for ForgetRetry {
    fn drop(&mut self) { retries().lock().unwrap_or_else(|e| e.into_inner()).remove(&self.0); }
}

/// The owner acted on this account (a click on its mailbox): a failed account is tried again at once. Nothing happens for an
/// account that is not waiting after a failure. The core's mailbox read calls this for the account it serves.
pub fn retry_now(account_id: &str) {
    let hit = retries().lock().unwrap_or_else(|e| e.into_inner()).get(account_id).cloned();
    if let Some(n) = hit { n.notify_waiters(); }
}

/// Writes the account's provider status words (None clears them) and announces the row.
fn status_words(d: &SyncDeps, id: &str, words: Option<String>) {
    let patch = sync_engine::StatePatch { provider_status_words: Some(words), ..Default::default() };
    match sync_engine::state::write_state(&d.store, id, patch) {
        Ok(state) => emit(&d.sinks, StreamKind::SyncStatus, CoreEvent::SyncStateChanged { state }),
        Err(e) => eprintln!("sync {id}: could not write the status words: {e}"),
    }
}

/// One IDLE connection on INBOX; every change the server reports is applied by `apply_changes`. A lost connection puts
/// "New mail is paused: <the server's words>" on the account's status while the adapter reconnects; IDLE running again
/// clears it.
async fn listen(d: Arc<SyncDeps>, id: String, adapter: Arc<ImapAdapter>, stop: Arc<Notify>) {
    let (tx, mut rx) = mpsc::unbounded_channel::<IdleEvent>();
    let worker = {
        let (d, id, adapter) = (d.clone(), id.clone(), adapter.clone());
        tokio::spawn(async move {
            let mut paused = false;
            while let Some(first) = rx.recv().await {
                let mut changed = false;
                let mut next = Some(first);
                // several notices in a row are one resync; the last connection notice wins
                while let Some(e) = next {
                    match e {
                        IdleEvent::Changed => changed = true,
                        IdleEvent::Lost { reason_words, retry_in_ms } => {
                            eprintln!("sync {id}: the IDLE connection was lost: {reason_words}; reconnecting in {retry_in_ms} ms");
                            status_words(&d, &id, Some(paused_status(&reason_words, retry_in_ms)));
                            paused = true;
                        }
                        IdleEvent::Listening if paused => { status_words(&d, &id, None); paused = false; changed = true; }
                        IdleEvent::Listening => {}
                    }
                    next = rx.try_recv().ok();
                }
                // after a reconnect the folder is brought up to date too: mail that arrived while the connection was down
                if changed {
                    if let Err(words) = apply_changes(&d, &id, &adapter, "INBOX").await {
                        eprintln!("sync {id}: a change on INBOX was not applied: {words}");
                    }
                }
            }
        })
    };
    let r = adapter.idle_supervised("INBOX", &TokioSleeper, &stop, &mut |e| { let _ = tx.send(e); }).await;
    drop(tx);
    let _ = worker.await;
    if let Err(e) = r {
        // a server without IDLE, or a refused sign-in while listening: the account keeps its mail and says why it stopped
        let reason = match &e { contracts::CoreError::Action { failure } => failure.reason_words.clone(), other => other.to_string() };
        eprintln!("sync {id}: push stopped: {reason}");
        status_words(&d, &id, Some(words::status(&format!("New mail is not pushed: {reason}"), words::LOST, &[("server_words", &reason)])));
    }
}

/// sync-full-r1 (finding from engine-act-r6): of the rows whose INBOX UID vanished, the ones the owner archived with E. E keeps the row in
/// INBOX with the archived mark and its old UID while the server moves its copy to Archive, so that UID vanishing is the move, not a delete.
fn archived_apart(d: &SyncDeps, folder: &str, ids: Vec<String>) -> Result<(Vec<String>, Vec<String>), String> {
    if !folder.eq_ignore_ascii_case(store::INBOX_FOLDER) { return Ok((Vec::new(), ids)); }
    let mut archived = Vec::new();
    let mut gone = Vec::new();
    for id in ids {
        let row = d.store.message(&id).map_err(|e| e.to_string())?;
        if row.is_some_and(|m| m.archived && m.folder.eq_ignore_ascii_case(store::INBOX_FOLDER)) { archived.push(id); } else { gone.push(id); }
    }
    Ok((archived, gone))
}

/// The archived rows whose INBOX UID vanished stay. One whose server place the move recorded (the Archive copy, COPYUID) drops the
/// stale INBOX UID: the write-back and the body download go to the recorded place, and the next resync does not report the UID again.
/// One with no recorded place (a server without UIDPLUS) keeps its UID until the Archive folder's copy is linked to it (the crawl's
/// `link_archived_copies`), so the way back still has a place to name.
fn keep_archived(d: &SyncDeps, ids: Vec<String>) -> Result<(), String> {
    let placed: Vec<String> = ids.into_iter().filter(|id| d.store.message_place(id).ok().flatten().is_some()).collect();
    if placed.is_empty() { return Ok(()); }
    d.store.write(move |ctx| ctx.clear_remote_ids(&placed)).map(|_| ()).map_err(|e| e.to_string())
}

/// What `apply_flag_changes` did: the store writes it made (one per value and direction, each for all its messages) and the messages whose
/// read or starred value really changed (the label flow re-reads only those).
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct FlagsApplied {
    pub writes: usize,
    pub flipped: Vec<String>,
}

/// The server's flags for the folder's known UIDs, applied where they differ from the store. The stored read and starred values of the
/// folder are read once (`Store::flags_in_folder`); a UID the store does not know is skipped, as before; a message stored after that read is
/// written as before. On QRESYNC and CONDSTORE the list holds only changed mails, so the comparison costs one read; on a server with no
/// change list (Microsoft 365) the list repeats every known UID and nothing unchanged is written or refiled.
pub fn apply_flag_changes(store: &Store, account: &str, folder: &str, by_uid: &HashMap<u32, String>, changes: Vec<sync_imap::FlagChange>) -> Result<FlagsApplied, String> {
    let mut out = FlagsApplied::default();
    if changes.is_empty() { return Ok(out); }
    let stored = store.flags_in_folder(account, folder).map_err(|e| e.to_string())?;
    let (mut read_on, mut read_off, mut star_on, mut star_off) = (Vec::new(), Vec::new(), Vec::new(), Vec::new());
    for change in changes {
        let Some(mid) = by_uid.get(&change.uid) else { continue };
        let seen = change.flags.iter().any(|f| f.eq_ignore_ascii_case("\\Seen"));
        let flagged = change.flags.iter().any(|f| f.eq_ignore_ascii_case("\\Flagged"));
        let (was_read, was_starred) = match stored.get(mid) { Some((r, s)) => (Some(*r), Some(*s)), None => (None, None) };
        let mut changed = false;
        if was_read != Some(seen) { if seen { read_on.push(mid.clone()) } else { read_off.push(mid.clone()) }; changed = true; }
        if was_starred != Some(flagged) { if flagged { star_on.push(mid.clone()) } else { star_off.push(mid.clone()) }; changed = true; }
        if changed && !out.flipped.contains(mid) { out.flipped.push(mid.clone()); }
    }
    for (ids, on) in [(read_on, true), (read_off, false)] {
        if ids.is_empty() { continue; }
        store.set_read(ids, on).map_err(|e| e.to_string())?;
        out.writes += 1;
    }
    for (ids, on) in [(star_on, true), (star_off, false)] {
        if ids.is_empty() { continue; }
        store.set_starred(ids, on).map_err(|e| e.to_string())?;
        out.writes += 1;
    }
    Ok(out)
}

/// Brings one folder up to date from its stored position: QRESYNC/CONDSTORE's change set when the server has one
/// (new messages stored, \Seen applied, vanished messages removed), else the newest page again (the store ignores ids
/// it already has).
pub async fn apply_changes(d: &SyncDeps, id: &str, adapter: &ImapAdapter, folder: &str) -> Result<(), String> {
    let known = d.store.uids_in_folder(id, folder).map_err(|e| e.to_string())?;
    let by_uid: HashMap<u32, String> = known.iter().cloned().collect();
    let uids: Vec<u32> = known.iter().map(|(u, _)| *u).collect();
    let cursor = d.store.imap_cursor(id, folder).map_err(|e| e.to_string())?;
    let resync = match cursor {
        Some((uv, ms)) => adapter.resync(folder, &SyncCursor::Imap { uid_validity: u64::from(uv), highest_mod_seq: ms }, &uids, OPTS).await.map_err(|e| e.to_string())?,
        None => Resync::Reset { reason_words: "no stored position for this folder yet".into() },
    };
    match resync {
        Resync::Delta(delta) => {
            if !delta.new.is_empty() {
                // delta 17: the ones the store does not hold yet run the labelling order once stored (`known` is the folder's stored ids)
                let held: std::collections::HashSet<&String> = by_uid.values().collect();
                d.filing.expect(delta.new.iter().map(|f| f.message.id.clone()).filter(|m| !held.contains(m)).collect::<Vec<_>>());
                // junk-r2: a change set's new messages are live arrivals (above every UID the store held): the spam filter scores them
                crate::junk_filter::store_arrivals(d, id, adapter, folder, delta.new, |f| !held.contains(&f.message.id)).await?;
                d.filing.kick();
            }
            // lane reply-headers (lead item): only the values that differ from the store are written and refiled (a server with no change
            // list reports every known UID's flags on each resync)
            let applied = apply_flag_changes(&d.store, id, folder, &by_uid, delta.flag_changes)?;
            d.filing.flipped(applied.flipped);
            let vanished: Vec<String> = delta.vanished.iter().filter_map(|u| by_uid.get(u).cloned()).collect();
            let (archived, gone) = archived_apart(d, folder, vanished)?;
            if !gone.is_empty() { d.store.delete_messages(gone).map_err(|e| e.to_string())?; }
            keep_archived(d, archived)?;
            if let SyncCursor::Imap { uid_validity, highest_mod_seq } = delta.cursor {
                d.store.set_imap_cursor(id, folder, uid_validity as u32, highest_mod_seq).map_err(|e| e.to_string())?;
            }
        }
        Resync::Reset { .. } => {
            let page = adapter.history_page(folder, u32::MAX, store::INBOX_PAGE_ROWS as u32, OPTS).await.map_err(|e| e.to_string())?;
            if !page.messages.is_empty() {
                let held: std::collections::HashSet<&String> = by_uid.values().collect();
                d.filing.expect(page.messages.iter().map(|f| f.message.id.clone()).filter(|m| !held.contains(m)).collect::<Vec<_>>());
                // junk-r2: a re-read page is old mail except what arrived above the stored position: the same UIDVALIDITY as the stored
                // cursor and a UID above every one the store held. A rebuilt folder (UIDVALIDITY changed) or no stored position scores nothing.
                let top = uids.iter().copied().max().unwrap_or(0);
                let live_uv = cursor.map(|(uv, _)| uv);
                crate::junk_filter::store_arrivals(d, id, adapter, folder, page.messages, |f| {
                    live_uv == Some(f.uid_validity) && top > 0 && f.uid > top && !held.contains(&f.message.id)
                }).await?;
                d.filing.kick();
            }
            let state = adapter.folder_state(folder).await.map_err(|e| e.to_string())?;
            d.store.set_imap_cursor(id, folder, state.uid_validity, state.highest_modseq.unwrap_or(0)).map_err(|e| e.to_string())?;
        }
    }
    // body-r1: an arrival's body is downloaded before it is opened (the folder's newest that have none)
    d.bodies.folder_synced(id, folder);
    d.bodies.kick(id);
    // sync-full-r1: the crawl goes on below where it stood (rows the change set stored are walked by it too)
    d.bodies.crawl_kick(id);
    Ok(())
}
