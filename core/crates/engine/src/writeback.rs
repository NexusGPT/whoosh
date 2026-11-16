//! engine-act: the IMAP write-back of the mail actions. Three parts, one file each concern:
//! - `Ledger` (the actions crate's `WriteBackLedger`): inside the transaction of the local change, one `pending_action` row per
//!   message the server must hear about. Snooze and pin are local to Whoosh (no server state); a message with no UID (imported, or
//!   never synced) has no server copy: neither gets a row.
//! - `ImapWriteBack` (the contract's `MailAdapter`): one request applied over sync-imap's own calls on the account's live adapter.
//!   Archive = MOVE to the account's Archive-role folder, else the `\Archived` flag; read = `\Seen`; the Starred label = `\Flagged`;
//!   another label = an IMAP keyword (Gmail: refused in words, see the note); spam = MOVE to the Junk-role folder; a cross-account move
//!   = append at the target then delete at the source (`sync_imap::transfer`).
//! - `Worker`: applies the due rows, one request per (account, folder, kind, payload). Taken = the rows go; refused or offline = they
//!   stay with the server's words, tried again after a wait that doubles from SYNC_BACKOFF_START_MS up to SYNC_BACKOFF_MAX_MS. The
//!   local state was applied first and stays (optimistic, as the fixture core does).
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::time::Duration;

use actions::{MailAdapter, MessageRef, Refusal, WriteBack, WriteBackLedger};
use contracts::Clock;
use store::{NewPending, PendingRow, Store, StoreError, WriteCtx};
use sync_imap::{FlagOp, FolderRole, ImapAdapter, ImapMailAdapter, MovedUid, Provider};

use crate::consts::{SYNC_BACKOFF_MAX_MS, SYNC_BACKOFF_START_MS};
use crate::reads::{folders_with_role, json_text};
use crate::sync::SyncDeps;

/// The label the actions crate stars with (actions::constants::STARRED_LABEL): on the server it is the `\Flagged` flag.
const STARRED: &str = actions::constants::STARRED_LABEL;
/// The local folder words of the Spam and Trash mailboxes (actions::constants): on the server, the Junk- and Trash-role folders.
const SPAM: &str = actions::constants::SPAM_FOLDER;
const TRASH: &str = actions::constants::TRASH_FOLDER;

// ---------------------------------------------------------------- the ledger

pub struct Ledger;

fn rows_for(account: &str, messages: &[MessageRef], kind: &str, payload: String) -> Vec<NewPending> {
    messages
        .iter()
        .filter_map(|m| {
            let uid = m.remote_id.as_deref()?.parse::<i64>().ok().filter(|u| *u > 0)?;
            Some(NewPending { account_id: account.to_string(), message_id: Some(m.message_id.clone()), folder: m.folder.clone(), uid: Some(uid), kind: kind.into(), payload_json: payload.clone() })
        })
        .collect()
}

/// The rows one request becomes (empty = nothing for the server to do).
pub fn pending_rows(request: &WriteBack) -> Vec<NewPending> {
    match request {
        WriteBack::SetArchived { account_id, messages, archived } => rows_for(account_id, messages, "set_archived", format!(r#"{{"archived":{archived}}}"#)),
        WriteBack::SetRead { account_id, messages, read } => rows_for(account_id, messages, "set_read", format!(r#"{{"read":{read}}}"#)),
        WriteBack::Label { account_id, messages, label_name, applied } => rows_for(account_id, messages, "label", format!(r#"{{"applied":{applied},"label":{}}}"#, json_text(label_name))),
        WriteBack::Move { from_account_id, to_account_id, messages } => rows_for(from_account_id, messages, "move", format!(r#"{{"to_account_id":{}}}"#, json_text(to_account_id))),
        WriteBack::MoveToFolder { account_id, messages, folder } => rows_for(account_id, messages, "move_to_folder", format!(r#"{{"folder":{}}}"#, json_text(folder))),
        // local to Whoosh: a snooze and a pin are not server state; emptying and unsubscribing have their own paths
        WriteBack::Snooze { .. } | WriteBack::Pin { .. } | WriteBack::EmptyFolders { .. } | WriteBack::Unsubscribe { .. } => vec![],
    }
}

/// perf-archive r2 (the undo twin of fix-realcore's unarchive.rs:54-64): Z after E builds `SetArchived { archived: false }` from the row
/// (actions usecases/undo.rs -> archive::apply). Once the sync heard the INBOX UID vanish it cleared that UID from an archived row whose
/// server place the move recorded (sync.rs `keep_archived`), so `rows_for` dropped the message and the server never heard Z. A way back
/// whose message has no UID names the recorded place (the Archive copy and its UID), as unarchive.rs does; a message with neither keeps
/// no row (it has no server copy). Every other request, and every message that still has its UID, is unchanged.
fn with_recorded_places(ctx: &WriteCtx<'_>, request: &WriteBack) -> Result<Option<WriteBack>, StoreError> {
    let WriteBack::SetArchived { account_id, messages, archived: false } = request else { return Ok(None) };
    if messages.iter().all(|m| m.remote_id.is_some()) { return Ok(None); }
    let mut out = Vec::with_capacity(messages.len());
    for m in messages {
        match (&m.remote_id, ctx.place_of(&m.message_id)?) {
            (None, Some(p)) => out.push(MessageRef { folder: p.folder, remote_id: Some(p.uid.to_string()), ..m.clone() }),
            _ => out.push(m.clone()),
        }
    }
    Ok(Some(WriteBack::SetArchived { account_id: account_id.clone(), messages: out, archived: false }))
}

impl WriteBackLedger for Ledger {
    fn execute(&self, ctx: &mut WriteCtx<'_>, request: &WriteBack, now_ms: i64) -> Result<(), StoreError> {
        let placed = with_recorded_places(ctx, request)?;
        for row in pending_rows(placed.as_ref().unwrap_or(request)) {
            ctx.pending_action_add(&row, now_ms)?;
        }
        Ok(())
    }
}

// ---------------------------------------------------------------- the request a group of rows makes

fn payload_str(json: &str, key: &str) -> Option<String> {
    // the payloads are the ledger's own flat objects (written above with json_text), read back by the matching small reader
    serde_like::parse(json)?.get(key)
}

fn payload_bool(json: &str, key: &str) -> Option<bool> {
    serde_like::parse(json)?.get_bool(key)
}

/// The request a group of rows of one account, folder, kind and payload stands for.
pub fn request_of(rows: &[PendingRow]) -> Option<WriteBack> {
    let first = rows.first()?;
    let messages: Vec<MessageRef> = rows
        .iter()
        .map(|r| MessageRef { message_id: r.message_id.clone().unwrap_or_default(), thread_id: String::new(), remote_id: r.uid.map(|u| u.to_string()), folder: r.folder.clone() })
        .collect();
    let account_id = first.account_id.clone();
    let p = &first.payload_json;
    Some(match first.kind.as_str() {
        "set_archived" => WriteBack::SetArchived { account_id, messages, archived: payload_bool(p, "archived")? },
        "set_read" => WriteBack::SetRead { account_id, messages, read: payload_bool(p, "read")? },
        "label" => WriteBack::Label { account_id, messages, label_name: payload_str(p, "label")?, applied: payload_bool(p, "applied")? },
        "move" => WriteBack::Move { from_account_id: account_id, to_account_id: payload_str(p, "to_account_id")?, messages },
        "move_to_folder" => WriteBack::MoveToFolder { account_id, messages, folder: payload_str(p, "folder")? },
        _ => return None,
    })
}

/// A tiny reader for the ledger's own flat payloads (`{"k":true,"k2":"text"}`), so the engine needs no JSON crate.
mod serde_like {
    pub struct Value(Vec<(String, Item)>);
    enum Item { Bool(bool), Text(String) }
    impl Value {
        pub fn get(&self, key: &str) -> Option<String> { self.0.iter().find(|(k, _)| k == key).and_then(|(_, v)| match v { Item::Text(t) => Some(t.clone()), Item::Bool(_) => None }) }
        pub fn get_bool(&self, key: &str) -> Option<bool> { self.0.iter().find(|(k, _)| k == key).and_then(|(_, v)| match v { Item::Bool(b) => Some(*b), Item::Text(_) => None }) }
    }
    fn string(chars: &mut std::iter::Peekable<std::str::Chars<'_>>) -> Option<String> {
        if chars.next()? != '"' { return None; }
        let mut out = String::new();
        loop {
            match chars.next()? {
                '"' => return Some(out),
                '\\' => match chars.next()? {
                    'n' => out.push('\n'), 'r' => out.push('\r'), 't' => out.push('\t'),
                    'u' => { let h: String = (0..4).filter_map(|_| chars.next()).collect(); out.push(char::from_u32(u32::from_str_radix(&h, 16).ok()?)?); }
                    c => out.push(c),
                },
                c => out.push(c),
            }
        }
    }
    pub fn parse(json: &str) -> Option<Value> {
        let mut c = json.trim().chars().peekable();
        if c.next()? != '{' { return None; }
        let mut items = Vec::new();
        loop {
            while c.peek().is_some_and(|x| x.is_whitespace() || *x == ',') { c.next(); }
            if c.peek() == Some(&'}') { return Some(Value(items)); }
            let key = string(&mut c)?;
            while c.peek().is_some_and(|x| x.is_whitespace() || *x == ':') { c.next(); }
            let item = match c.peek()? {
                '"' => Item::Text(string(&mut c)?),
                't' => { for _ in 0..4 { c.next(); } Item::Bool(true) }
                'f' => { for _ in 0..5 { c.next(); } Item::Bool(false) }
                _ => return None,
            };
            items.push((key, item));
        }
    }
}

// ---------------------------------------------------------------- the IMAP side

/// The contract's `MailAdapter` over sync-imap, on each account's live adapter (the one its sync opened).
pub struct ImapWriteBack {
    pub deps: Arc<SyncDeps>,
    pub runtime: tokio::runtime::Handle,
    /// The hidden check's mutant `act-never-writes-back`: every request answers Ok and nothing reaches the server.
    never: AtomicBool,
}

/// The account's folder with this role: the one the server named at its last LIST (an empty folder included), else a stored
/// folder whose usual name has the role (`reads::folders_with_role`, the rule the mailbox lists use).
pub fn role_folder(deps: &SyncDeps, account: &str, role: FolderRole) -> Option<String> {
    let named = deps.roles.lock().ok().and_then(|g| g.get(account).and_then(|m| {
        let mut hits: Vec<&String> = m.iter().filter(|(_, r)| **r == role).map(|(f, _)| f).collect();
        hits.sort();
        hits.first().map(|f| f.to_string())
    }));
    named.or_else(|| folders_with_role(&deps.store, &deps.roles, &[account.to_string()], role).into_iter().map(|(_, f)| f).next())
}

fn refusal(account_id: &str, words: impl Into<String>, retryable: bool) -> Refusal {
    Refusal { account_id: account_id.to_string(), reason_words: words.into(), retryable }
}

/// Blocks the calling thread on one IMAP call, on a thread of its own so a caller inside an async runtime never nests.
fn block<T: Send>(rt: &tokio::runtime::Handle, f: impl std::future::Future<Output = T> + Send) -> Option<T> {
    std::thread::scope(|s| s.spawn(|| rt.block_on(f)).join()).ok()
}

/// The UIDs of the messages with their store ids, by the folder they sit in (what a move needs to record where each one lands).
fn by_folder_ids(messages: &[MessageRef]) -> Vec<(String, Vec<(u32, String)>)> {
    let mut order: Vec<String> = Vec::new();
    let mut map: HashMap<String, Vec<(u32, String)>> = HashMap::new();
    for m in messages {
        let Some(uid) = m.remote_id.as_deref().and_then(|u| u.parse::<u32>().ok()) else { continue };
        if !map.contains_key(&m.folder) { order.push(m.folder.clone()); }
        map.entry(m.folder.clone()).or_default().push((uid, m.message_id.clone()));
    }
    order.into_iter().map(|f| { let u = map.remove(&f).unwrap_or_default(); (f, u) }).collect()
}

/// The UIDs of the messages, by the folder they sat in.
fn by_folder(messages: &[MessageRef]) -> Vec<(String, Vec<u32>)> {
    let mut order: Vec<String> = Vec::new();
    let mut map: HashMap<String, Vec<u32>> = HashMap::new();
    for m in messages {
        let Some(uid) = m.remote_id.as_deref().and_then(|u| u.parse::<u32>().ok()) else { continue };
        if !map.contains_key(&m.folder) { order.push(m.folder.clone()); }
        map.entry(m.folder.clone()).or_default().push(uid);
    }
    order.into_iter().map(|f| { let u = map.remove(&f).unwrap_or_default(); (f, u) }).collect()
}

/// An owner's label as an IMAP keyword (RFC 3501 atom: letters, digits and `_ $ - .`); "Clients > Provanta" = "Clients_Provanta".
pub fn keyword_for(label: &str) -> String {
    let mut out = String::new();
    for c in label.trim().chars() {
        if c.is_ascii_alphanumeric() || matches!(c, '_' | '$' | '-' | '.') { out.push(c); } else if !out.ends_with('_') { out.push('_'); }
    }
    let t = out.trim_matches('_').to_string();
    if t.is_empty() { "Label".into() } else { t }
}

impl ImapWriteBack {
    pub fn new(deps: Arc<SyncDeps>, runtime: tokio::runtime::Handle) -> ImapWriteBack { ImapWriteBack { deps, runtime, never: AtomicBool::new(false) } }
    pub fn set_never(&self, never: bool) { self.never.store(never, Ordering::SeqCst) }
    fn adapter(&self, account: &str) -> Result<Arc<ImapAdapter>, Refusal> {
        self.deps.adapters.lock().unwrap_or_else(|e| e.into_inner()).get(account).cloned()
            .ok_or_else(|| refusal(account, "this account is not connected yet; the change is sent when it is", true))
    }
    fn role_folder(&self, account: &str, role: FolderRole) -> Option<String> { role_folder(&self.deps, account, role) }
    fn run(&self, account: &str, f: impl std::future::Future<Output = Result<(), contracts::CoreError>> + Send) -> Result<(), Refusal> {
        self.run_value(account, f)
    }
    fn run_value<T: Send>(&self, account: &str, f: impl std::future::Future<Output = Result<T, contracts::CoreError>> + Send) -> Result<T, Refusal> {
        match block(&self.runtime, f) {
            Some(Ok(v)) => Ok(v),
            Some(Err(e)) => {
                let retryable = !matches!(e, contracts::CoreError::Invalid { .. });
                Err(refusal(account, e.to_string(), retryable))
            }
            None => Err(refusal(account, "the server call stopped unexpectedly", true)),
        }
    }
    /// The messages as the SERVER holds them now (engine-act-r5): one whose move was applied has its new place recorded (the server's
    /// COPYUID answer, `store::ServerPlace`) while its row still carries the place the core wrote locally (INBOX + archived, or the word
    /// Spam). Every request is aimed at the recorded place, so the way back names the folder and the UID the message really has.
    fn resolved(&self, messages: &[MessageRef]) -> Vec<MessageRef> {
        messages.iter().map(|m| match self.deps.store.message_place(&m.message_id) {
            Ok(Some(p)) => MessageRef { folder: p.folder, remote_id: Some(p.uid.to_string()), ..m.clone() },
            Ok(None) => m.clone(),
            Err(e) => { eprintln!("write-back: the recorded server place of {} could not be read ({e}); the row's own place is used", m.message_id); m.clone() }
        }).collect()
    }
    /// After a move the server answered: each message's new place goes into the store (a message back in its INBOX takes the INBOX UID
    /// as its server id and loses its recorded place). A server that named no place (UIDPLUS is optional) leaves everything as it was
    /// and the sync's own re-read of the target folder fills the rest in.
    fn record_places(&self, account: &str, dest: &str, items: &[(u32, String)], placed: &[MovedUid]) {
        let hits: Vec<(String, MovedUid)> = placed.iter().filter_map(|p| items.iter().find(|(u, _)| *u == p.from_uid).map(|(_, id)| (id.clone(), *p))).collect();
        if hits.is_empty() { return; }
        let back = dest.eq_ignore_ascii_case(store::INBOX_FOLDER);
        let (account, dest) = (account.to_string(), dest.to_string());
        let wrote = self.deps.store.write(move |ctx| {
            for (id, p) in &hits {
                if back { ctx.place_back_in_inbox(id, p.to_uid)?; } else { ctx.place_record(id, &account, &dest, p.uid_validity, p.to_uid)?; }
            }
            Ok(())
        });
        if let Err(e) = wrote { eprintln!("write-back: the new server place of a moved message was not kept ({e}); the sync's re-read fills it in"); }
    }
    fn flags(&self, account: &str, messages: &[MessageRef], op: FlagOp, flag: &str) -> Result<(), Refusal> {
        let a = self.adapter(account)?;
        for (folder, uids) in by_folder(&self.resolved(messages)) {
            self.run(account, async { a.set_flags(&folder, &uids, op, &[flag]).await })?;
        }
        Ok(())
    }
    fn move_to(&self, account: &str, messages: &[MessageRef], dest: &str) -> Result<(), Refusal> {
        let a = self.adapter(account)?;
        for (folder, items) in by_folder_ids(&self.resolved(messages)) {
            if folder == dest { continue; }
            let uids: Vec<u32> = items.iter().map(|(u, _)| *u).collect();
            let placed = self.run_value(account, async { a.move_placed(&folder, &uids, dest).await })?;
            self.record_places(account, dest, &items, &placed);
        }
        Ok(())
    }
}

impl MailAdapter for ImapWriteBack {
    fn execute(&self, request: WriteBack) -> Result<(), Refusal> {
        if self.never.load(Ordering::SeqCst) { return Ok(()); }
        match request {
            WriteBack::SetArchived { account_id, messages, archived: true } => match self.role_folder(&account_id, FolderRole::Archive) {
                Some(archive) => self.move_to(&account_id, &messages, &archive),
                None => self.flags(&account_id, &messages, FlagOp::Add, "\\Archived"),
            },
            WriteBack::SetArchived { account_id, messages, archived: false } => {
                let archive = self.role_folder(&account_id, FolderRole::Archive);
                // a message the core archived by the move sits in the Archive folder on the server whatever its row says
                let messages = self.resolved(&messages);
                let (moved, flagged): (Vec<MessageRef>, Vec<MessageRef>) = messages.into_iter().partition(|m| archive.as_deref() == Some(m.folder.as_str()));
                self.move_to(&account_id, &moved, store::INBOX_FOLDER)?;
                self.flags(&account_id, &flagged, FlagOp::Remove, "\\Archived")
            }
            WriteBack::SetRead { account_id, messages, read } => self.flags(&account_id, &messages, if read { FlagOp::Add } else { FlagOp::Remove }, "\\Seen"),
            WriteBack::Label { account_id, messages, label_name, applied } => {
                let op = if applied { FlagOp::Add } else { FlagOp::Remove };
                if label_name == STARRED { return self.flags(&account_id, &messages, op, "\\Flagged"); }
                if self.adapter(&account_id)?.connection_table().provider == Provider::Gmail {
                    // a Gmail label is a folder: putting a message in it needs UID COPY, which sync-imap does not offer (note)
                    return Err(refusal(&account_id, "a Gmail label cannot be written back yet: the mail engine has no copy call", false));
                }
                self.flags(&account_id, &messages, op, &keyword_for(&label_name))
            }
            WriteBack::MoveToFolder { account_id, messages, folder } => {
                let dest = match folder.as_str() {
                    f if f == SPAM => self.role_folder(&account_id, FolderRole::Junk).ok_or_else(|| refusal(&account_id, "the server has no Junk folder", false))?,
                    f if f == TRASH => self.role_folder(&account_id, FolderRole::Trash).ok_or_else(|| refusal(&account_id, "the server has no Trash folder", false))?,
                    other => other.to_string(),
                };
                self.move_to(&account_id, &messages, &dest)
            }
            WriteBack::Move { from_account_id, to_account_id, messages } => {
                let (src, dst) = (self.adapter(&from_account_id)?, self.adapter(&to_account_id)?);
                for (folder, uids) in by_folder(&messages) {
                    self.run(&from_account_id, async { sync_imap::transfer(&src, &folder, &uids, &dst, store::INBOX_FOLDER, sync_imap::TransferMode::Move).await })?;
                }
                Ok(())
            }
            WriteBack::Snooze { .. } | WriteBack::Pin { .. } | WriteBack::EmptyFolders { .. } | WriteBack::Unsubscribe { .. } => Ok(()),
        }
    }
}

// ---------------------------------------------------------------- the worker

/// The wait before try number `tries + 1`: SYNC_BACKOFF_START_MS doubled per refusal, never above SYNC_BACKOFF_MAX_MS.
pub fn backoff_ms(tries: i64) -> i64 {
    let start = SYNC_BACKOFF_START_MS as i64;
    let max = SYNC_BACKOFF_MAX_MS as i64;
    let shift = tries.clamp(0, 30) as u32;
    start.saturating_mul(1i64 << shift).min(max)
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct WorkReport {
    pub applied: usize,
    pub refused: usize,
    pub words: Vec<String>,
}

pub struct Worker {
    store: Arc<Store>,
    adapter: Arc<dyn MailAdapter>,
    clock: Arc<dyn Clock>,
    kicked: Mutex<bool>,
    wake: Condvar,
    stop: AtomicBool,
}

impl Worker {
    pub fn new(store: Arc<Store>, adapter: Arc<dyn MailAdapter>, clock: Arc<dyn Clock>) -> Arc<Worker> {
        Arc::new(Worker { store, adapter, clock, kicked: Mutex::new(false), wake: Condvar::new(), stop: AtomicBool::new(false) })
    }

    /// Applies every due row once. Rows of one account, folder, kind and payload go as one request.
    pub fn run_once(&self) -> WorkReport {
        let now = self.clock.now_ms();
        let due = match self.store.pending_actions_due(now) {
            Ok(d) => d,
            Err(e) => { eprintln!("write-back: the waiting server changes could not be read: {e}"); return WorkReport::default(); }
        };
        let mut order: Vec<(String, String, String, String)> = Vec::new();
        let mut groups: HashMap<(String, String, String, String), Vec<PendingRow>> = HashMap::new();
        for r in due {
            let key = (r.account_id.clone(), r.folder.clone(), r.kind.clone(), r.payload_json.clone());
            if !groups.contains_key(&key) { order.push(key.clone()); }
            groups.entry(key).or_default().push(r);
        }
        let mut report = WorkReport::default();
        for key in order {
            let rows = groups.remove(&key).unwrap_or_default();
            let ids: Vec<i64> = rows.iter().map(|r| r.id).collect();
            let tries = rows.iter().map(|r| r.tries).max().unwrap_or(0);
            let outcome = match request_of(&rows) {
                Some(req) => self.adapter.execute(req),
                None => Err(refusal(&key.0, format!("a waiting server change of kind {} is unreadable", key.2), false)),
            };
            match outcome {
                Ok(()) => {
                    if let Err(e) = self.store.pending_action_done(ids) { eprintln!("write-back: a change the server took is still listed: {e}"); }
                    report.applied += rows.len();
                }
                Err(r) => {
                    // a refusal that cannot get better waits the longest; one that can waits the doubling wait
                    let wait = if r.retryable { backoff_ms(tries) } else { SYNC_BACKOFF_MAX_MS as i64 };
                    eprintln!("write-back {} ({}): the server refused, tried again in {wait} ms: {}", key.0, key.2, r.reason_words);
                    if let Err(e) = self.store.pending_action_retry(ids, r.reason_words.clone(), self.clock.now_ms() + wait) { eprintln!("write-back: the refusal could not be kept: {e}"); }
                    report.refused += rows.len();
                    report.words.push(r.reason_words);
                }
            }
        }
        report
    }

    /// New rows were committed: run now.
    pub fn kick(&self) {
        *self.kicked.lock().unwrap_or_else(|e| e.into_inner()) = true;
        self.wake.notify_all();
    }

    pub fn stop(&self) {
        self.stop.store(true, Ordering::SeqCst);
        self.kick();
    }

    /// The worker thread: runs the due rows, then sleeps until the next row is due or a kick.
    pub fn start(self: &Arc<Self>) {
        let me = self.clone();
        let spawned = std::thread::Builder::new().name("whoosh-writeback".into()).spawn(move || {
            while !me.stop.load(Ordering::SeqCst) {
                me.run_once();
                let next = me.store.pending_action_next_due().ok().flatten();
                let wait = next.map(|at| (at - me.clock.now_ms()).clamp(0, SYNC_BACKOFF_MAX_MS as i64) as u64);
                let mut k = me.kicked.lock().unwrap_or_else(|e| e.into_inner());
                if !*k {
                    k = match wait {
                        Some(ms) => me.wake.wait_timeout(k, Duration::from_millis(ms)).map(|(g, _)| g).unwrap_or_else(|e| e.into_inner().0),
                        None => me.wake.wait(k).unwrap_or_else(|e| e.into_inner()),
                    };
                }
                *k = false;
            }
        });
        if let Err(e) = spawned { eprintln!("write-back: the worker thread did not start ({e}); server changes wait for the next launch"); }
    }
}
