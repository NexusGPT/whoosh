//! The shared machinery of every use case: commit in the store first, write back through the port, roll back on a refusal.
use crate::port::{MailAdapter, MessageRef, Refusal, SendCanceller, WriteBack, WriteBackLedger};
use crate::undo;
use contracts::{ActionFailed, Clock, CommandOutcome, CoreError, UndoAction};
use rusqlite::params;
use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use store::{Store, StoreError, WriteCtx};

/// What the use cases share. Cloning is cheap (all Arcs).
#[derive(Clone)]
pub struct Env {
    pub store: Arc<Store>,
    pub adapter: Arc<dyn MailAdapter>,
    pub clock: Arc<dyn Clock>,
    pub sends: Option<Arc<dyn SendCanceller>>,
    /// engine-act: set = the write-back is recorded in the local write's transaction and applied later by a worker.
    pub ledger: Option<Arc<dyn WriteBackLedger>>,
    commit_us: Arc<AtomicU64>,
}

impl Env {
    pub fn new(store: Arc<Store>, adapter: Arc<dyn MailAdapter>, clock: Arc<dyn Clock>) -> Env {
        Env { store, adapter, clock, sends: None, ledger: None, commit_us: Arc::new(AtomicU64::new(0)) }
    }

    pub fn with_send_canceller(mut self, sends: Arc<dyn SendCanceller>) -> Env {
        self.sends = Some(sends);
        self
    }

    pub fn with_ledger(mut self, ledger: Arc<dyn WriteBackLedger>) -> Env {
        self.ledger = Some(ledger);
        self
    }

    /// How long the most recent store commit took, from the call to the committed answer. COMMIT_MS bounds it.
    pub fn last_commit(&self) -> Duration {
        Duration::from_micros(self.commit_us.load(Ordering::Relaxed))
    }

    pub(crate) fn now(&self) -> i64 {
        self.clock.now_ms()
    }

    pub(crate) fn account_name(&self, id: &str) -> String {
        let key = id.to_string();
        self.store
            .write(move |ctx| Ok(ctx.tx.query_row("SELECT display_name FROM account WHERE id = ?1", [&key], |r| r.get::<_, String>(0)).ok()))
            .ok()
            .flatten()
            .unwrap_or_else(|| id.to_string())
    }

    /// "<account name>: <reason in words>", with whether a retry can help.
    pub(crate) fn failure(&self, refusal: &Refusal) -> CoreError {
        CoreError::Action { failure: ActionFailed { account_name: self.account_name(&refusal.account_id), reason_words: refusal.reason_words.clone(), retryable: refusal.retryable, code: None } }
    }
}

pub(crate) type RevertFn = Arc<dyn Fn(&mut WriteCtx<'_>, &[String]) -> Result<(), StoreError> + Send + Sync>;

pub(crate) fn revert_fn<F>(f: F) -> RevertFn
where
    F: Fn(&mut WriteCtx<'_>, &[String]) -> Result<(), StoreError> + Send + Sync + 'static,
{
    Arc::new(f)
}

/// The write-back for one account and the messages it covers.
pub(crate) struct Group {
    pub request: WriteBack,
    pub message_ids: Vec<String>,
}

/// What a use case's store step did: how to tell the servers, how to take it back, what Z reverses.
pub(crate) struct Applied {
    pub undo: Option<UndoAction>,
    pub groups: Vec<Group>,
    pub revert: RevertFn,
    pub created_id: Option<String>,
}

impl Applied {
    /// The command changed nothing (the flags already had that value): no write-back, no undo entry.
    pub fn nothing() -> Applied {
        Applied { undo: None, groups: Vec::new(), revert: revert_fn(|_, _| Ok(())), created_id: None }
    }

    /// Two steps of one action (for example label then archive): one list of write-backs, both taken back together.
    pub fn merge(a: Applied, b: Applied) -> Applied {
        let (ra, rb) = (a.revert.clone(), b.revert.clone());
        let mut groups = a.groups;
        groups.extend(b.groups);
        Applied { undo: None, groups, revert: revert_fn(move |ctx, ids| { ra(ctx, ids)?; rb(ctx, ids) }), created_id: a.created_id.or(b.created_id) }
    }
}

pub(crate) type Step = Result<Result<Applied, CoreError>, StoreError>;

/// Commit in the store first (with the undo entry, in the same transaction), then ask the servers, one account at a time.
/// A refusal takes back the refused account's messages and every account not yet asked, shrinks the undo entry to
/// what stayed, and returns the failure in words.
pub(crate) fn core_first<F>(env: &Env, apply: F) -> Result<CommandOutcome, CoreError>
where
    F: FnOnce(&mut WriteCtx<'_>, i64) -> Step + Send + 'static,
{
    let now = env.now();
    let started = Instant::now();
    let ledger = env.ledger.clone();
    let step = env.store.write(move |ctx| match apply(ctx, now)? {
        Err(refusal) => Ok(Err(refusal)),
        Ok(applied) => {
            let undo_id = match &applied.undo {
                Some(action) => Some(undo::push(ctx, action, now)?),
                None => None,
            };
            // engine-act: the server write-back is a row of this same transaction (or nothing at all)
            if let Some(l) = &ledger {
                for g in &applied.groups {
                    l.execute(ctx, &g.request, now)?;
                }
            }
            Ok(Ok((applied, undo_id)))
        }
    })?;
    env.commit_us.store(started.elapsed().as_micros() as u64, Ordering::Relaxed);
    let (applied, undo_id) = step?;
    if env.ledger.is_some() {
        return Ok(CommandOutcome { undo_entry_id: undo_id, created_id: applied.created_id });
    }
    write_back(env, applied, undo_id)
}

fn write_back(env: &Env, applied: Applied, undo_id: Option<String>) -> Result<CommandOutcome, CoreError> {
    for (i, group) in applied.groups.iter().enumerate() {
        if let Err(refusal) = env.adapter.execute(group.request.clone()) {
            let failed: Vec<String> = applied.groups[i..].iter().flat_map(|g| g.message_ids.clone()).collect();
            let kept: HashSet<String> = applied.groups[..i].iter().flat_map(|g| g.message_ids.clone()).collect();
            let (revert, undo) = (applied.revert.clone(), applied.undo.clone());
            let entry = undo_id.clone();
            env.store.write(move |ctx| {
                revert(ctx, &failed)?;
                if let (Some(id), Some(action)) = (entry, undo) {
                    undo::restrict(ctx, &id, &action, &kept)?;
                }
                Ok(())
            })?;
            return Err(env.failure(&refusal));
        }
    }
    Ok(CommandOutcome { undo_entry_id: undo_id, created_id: applied.created_id })
}

/// Run the store step only (no undo entry, no write-back): for steps that follow a server action already accepted.
pub(crate) fn local_only<F>(env: &Env, apply: F) -> Result<(), CoreError>
where
    F: FnOnce(&mut WriteCtx<'_>, i64) -> Step + Send + 'static,
{
    let now = env.now();
    env.store.write(move |ctx| match apply(ctx, now)? {
        Err(refusal) => Ok(Err(refusal)),
        Ok(_) => Ok(Ok(())),
    })?
}

/// A message as the actions need it.
#[derive(Debug, Clone)]
pub(crate) struct MsgRow {
    pub id: String,
    pub account_id: String,
    pub thread_id: String,
    pub remote_id: Option<String>,
    pub folder: String,
    pub archived: bool,
    pub read: bool,
}

impl MsgRow {
    pub fn as_ref(&self) -> MessageRef {
        MessageRef { message_id: self.id.clone(), thread_id: self.thread_id.clone(), remote_id: self.remote_id.clone(), folder: self.folder.clone() }
    }
}

/// Read the named messages (duplicates once). An unknown id is NotFound; an empty list is Invalid. Nothing is written.
pub(crate) fn load(ctx: &WriteCtx<'_>, ids: &[String]) -> Result<Result<Vec<MsgRow>, CoreError>, StoreError> {
    if ids.is_empty() {
        return Ok(Err(CoreError::Invalid { field: "message_ids".into(), reason_words: "no message was named".into() }));
    }
    let mut st = ctx.tx.prepare_cached("SELECT id, account_id, thread_id, remote_id, folder, archived, read FROM message WHERE id = ?1")?;
    let mut seen = HashSet::new();
    let mut out = Vec::with_capacity(ids.len());
    for id in ids {
        if !seen.insert(id.clone()) {
            continue;
        }
        let row = st.query_row([id], |r| {
            Ok(MsgRow { id: r.get(0)?, account_id: r.get(1)?, thread_id: r.get(2)?, remote_id: r.get(3)?, folder: r.get(4)?, archived: r.get::<_, i64>(5)? != 0, read: r.get::<_, i64>(6)? != 0 })
        });
        match row {
            Ok(r) => out.push(r),
            Err(rusqlite::Error::QueryReturnedNoRows) => return Ok(Err(CoreError::NotFound { kind: "message".into(), id: id.clone() })),
            Err(e) => return Err(e.into()),
        }
    }
    Ok(Ok(out))
}

/// One write-back group per account, in the order the accounts first appear.
pub(crate) fn groups_by_account(rows: &[MsgRow], make: impl Fn(&str, Vec<MessageRef>) -> WriteBack) -> Vec<Group> {
    let mut order: Vec<String> = Vec::new();
    let mut by: HashMap<String, Vec<&MsgRow>> = HashMap::new();
    for r in rows {
        if !by.contains_key(&r.account_id) {
            order.push(r.account_id.clone());
        }
        by.entry(r.account_id.clone()).or_default().push(r);
    }
    order
        .into_iter()
        .map(|account| {
            let rs = &by[&account];
            Group { request: make(&account, rs.iter().map(|r| r.as_ref()).collect()), message_ids: rs.iter().map(|r| r.id.clone()).collect() }
        })
        .collect()
}

pub(crate) fn set_flag(ctx: &WriteCtx<'_>, column: &str, ids: &[String], value: bool) -> Result<(), StoreError> {
    let sql = format!("UPDATE message SET {column} = ?1 WHERE id = ?2");
    let mut st = ctx.tx.prepare_cached(&sql)?;
    for id in ids {
        st.execute(params![value as i64, id])?;
    }
    Ok(())
}

pub(crate) fn optional<T>(r: Result<T, rusqlite::Error>) -> Result<Option<T>, rusqlite::Error> {
    match r {
        Ok(v) => Ok(Some(v)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(e),
    }
}

static SEQ: AtomicU64 = AtomicU64::new(0);

/// A fresh id: prefix, nanoseconds, a process counter.
pub(crate) fn new_id(prefix: &str) -> String {
    let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
    format!("{prefix}-{nanos:x}-{}", SEQ.fetch_add(1, Ordering::Relaxed))
}

/// A JSON string literal, escaped. The schema checks json_valid on these columns.
pub(crate) fn json_str(s: &str) -> String {
    let mut out = String::from("\"");
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

pub(crate) fn json_strs(items: &[String]) -> String {
    format!("[{}]", items.iter().map(|s| json_str(s)).collect::<Vec<_>>().join(","))
}
