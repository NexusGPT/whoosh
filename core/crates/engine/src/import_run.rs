//! Delta 18 in the real core: the mail import (PRD A83). `ImportScan` finds the sources, `ImportStart` runs the import crate over one of
//! them on its own thread, `ImportCancel` stops it at the next batch; the screens read `import/sources`, `import/<run>/progress` and
//! `import/<run>/result` (docs/lanes/contracts.md "Delta 18", shapes and refusal words). The import crate reads and converts, the store
//! writes (`Store::insert_imported`: every NEW row names its source and its import time, migration 0010), this file only wires them.
//!
//! One run at a time (the contract's decision). A run writes in batches of the ledger's IMPORT_BATCH_MESSAGES / IMPORT_BATCH_BYTES (the
//! crate's pipeline holds the batch); what is stored before a cancel or a failure stays, and a re-run skips it (one row per message).
//! Imported mail is local only: nothing here talks to a server.
use std::collections::HashMap;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};

use ::import as mail_import;
use contracts::{
    check_import_cancel, check_import_start, import_access_key, import_access_refusal, import_access_words, import_kind_of_pick,
    import_progress_fraction, import_progress_is_legal, import_result_adds_up, import_run_state_key, import_source_id, import_source_kind_key,
    import_store_failed_refusal, CommandOutcome, CoreError, ImportAccess, ImportInto, ImportPerAccount, ImportRefusal, ImportResult, ImportRunProgress,
    ImportRunState, ImportSkipped, ImportSource, ImportSourceKind,
};
use store::Store;

use crate::reads::json_text;
use crate::rust_core::now_ms;

/// The local account that takes mail no account address matches. It has no server settings, so it is never synced or sent from.
const IMPORTED_ADDRESS: &str = "imported@whoosh.local";
const IMPORTED_NAME: &str = "Imported";
/// What the sink answers with when the owner cancelled: carried in the crate's own error type, recognised by this exact text.
const CANCEL_MARK: &str = "whoosh-import-cancelled";

/// Called after every progress report of a run, on the run's thread, outside any lock: the seam a test uses to stop a run between two
/// batches (and to cancel it there). Nothing in production sets it.
pub type ImportPace = Arc<dyn Fn(&ImportRunProgress) + Send + Sync>;

struct RunRecord {
    progress: ImportRunProgress,
    result: Option<ImportResult>,
    cancel: Arc<AtomicBool>,
}

#[derive(Default)]
struct Inner {
    sources: Vec<ImportSource>,
    readers: HashMap<String, mail_import::Source>,
    runs: HashMap<String, RunRecord>,
    running: Option<String>,
}

/// The scan's findings and every run of this launch.
pub(crate) struct ImportBook {
    inner: Mutex<Inner>,
    home: Mutex<PathBuf>,
    pace: Mutex<Option<ImportPace>>,
    /// The hidden test copy's mutant `import-never-stores`: every batch is taken and nothing is written.
    never_store: AtomicBool,
}

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

fn access_of(e: &mail_import::ImportError) -> ImportAccess {
    match e {
        mail_import::ImportError::NeedsFullDiskAccess { .. } => ImportAccess::NeedsFullDiskAccess,
        mail_import::ImportError::NotFound { .. } => ImportAccess::NotFound,
        mail_import::ImportError::Unreadable { .. } | mail_import::ImportError::Store { .. } => ImportAccess::Unreadable,
    }
}

/// A path as the owner reads it: `~/…` under the home folder, the whole path elsewhere.
fn location_of(path: &Path, home: &Path) -> String {
    match path.strip_prefix(home) {
        Ok(rel) if !rel.as_os_str().is_empty() => format!("~/{}", rel.display()),
        _ => path.display().to_string(),
    }
}

fn invalid(code: &str, words: &str) -> CoreError {
    CoreError::Invalid { field: code.to_string(), reason_words: words.to_string() }
}

impl ImportBook {
    pub(crate) fn new(home: PathBuf) -> Self {
        ImportBook { inner: Mutex::new(Inner::default()), home: Mutex::new(home), pace: Mutex::new(None), never_store: AtomicBool::new(false) }
    }

    /// Where Apple Mail's folder is looked for (`<home>/Library/Mail`). The app's own home folder unless a test or the rig says otherwise.
    pub(crate) fn set_home(&self, home: PathBuf) {
        *lock(&self.home) = home;
    }

    pub(crate) fn set_pace(&self, pace: Option<ImportPace>) {
        *lock(&self.pace) = pace;
    }

    pub(crate) fn set_never_store(&self, on: bool) {
        self.never_store.store(on, Ordering::SeqCst);
    }

    /// `ImportScan`: Apple Mail first when it is there, then each picked file in order. Every pick is checked before anything changes, so a
    /// bad one refuses the whole scan in words and leaves the last scan as it was. A source that cannot be opened stays in the list with its
    /// reason (`access`); it cannot be started.
    pub(crate) fn scan(&self, store: &Store, picked: &[String]) -> Result<(), CoreError> {
        let kinds: Vec<ImportSourceKind> = picked.iter().map(|p| import_kind_of_pick(p.clone())).collect::<Result<_, _>>()?;
        let home = lock(&self.home).clone();
        let mut sources: Vec<ImportSource> = Vec::new();
        let mut readers: HashMap<String, mail_import::Source> = HashMap::new();
        let mut add = |source: ImportSource, reader: mail_import::Source| {
            if !readers.contains_key(&source.id) {
                readers.insert(source.id.clone(), reader);
                sources.push(source);
            }
        };
        match mail_import::find_apple_mail(&home) {
            Ok(Some(reader)) => {
                let (count, access) = match mail_import::count_messages(&reader) {
                    Ok(n) => (n, ImportAccess::Readable),
                    Err(e) => (None, access_of(&e)),
                };
                let location = "~/Library/Mail".to_string();
                add(blank_source(ImportSourceKind::AppleMail, "Apple Mail", location, count, access), reader);
            }
            // Apple Mail never ran on this Mac: there is nothing to list
            Ok(None) => {}
            Err(e) => {
                let reader = mail_import::Source::AppleMail { mail_dir: home.join("Library").join("Mail") };
                add(blank_source(ImportSourceKind::AppleMail, "Apple Mail", "~/Library/Mail".to_string(), None, access_of(&e)), reader);
            }
        }
        for (path, kind) in picked.iter().zip(kinds) {
            let file = PathBuf::from(path.trim());
            let name = file.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| path.clone());
            let (count, access) = probe_file(&file, kind);
            let reader = match kind {
                ImportSourceKind::Olm => mail_import::Source::Olm { file: file.clone() },
                _ => mail_import::Source::Mbox { file: file.clone() },
            };
            add(blank_source(kind, &name, location_of(&file, &home), count, access), reader);
        }
        for s in &mut sources {
            let (n, last) = store.imported_history(&s.id).map_err(|e| CoreError::Unexpected { reason_words: format!("the import history could not be read: {e}") })?;
            s.imported_count = n;
            s.last_run_ms = last;
        }
        let mut inner = lock(&self.inner);
        inner.sources = sources;
        inner.readers = readers;
        Ok(())
    }

    /// `ImportStart`: the contract's checks in their order, then the run on its own thread. Returns at once with the run id.
    pub(crate) fn start(self: &Arc<Self>, store: &Arc<Store>, source_id: &str, into: ImportInto) -> Result<CommandOutcome, CoreError> {
        let account_ids: Vec<String> = store.account_summaries().map_err(|e| CoreError::Unexpected { reason_words: format!("the accounts could not be read: {e}") })?.into_iter().map(|a| a.id).collect();
        let (run_id, source, reader, cancel) = {
            let mut inner = lock(&self.inner);
            let source = inner.sources.iter().find(|s| s.id == source_id).cloned();
            check_import_start(source.clone(), into.clone(), account_ids, inner.running.clone())?;
            let (Some(source), Some(reader)) = (source, inner.readers.get(source_id).cloned()) else {
                return Err(invalid("import_unknown_source", "That source is not in the list. Scan again."));
            };
            let run_id = store::new_id("import");
            let cancel = Arc::new(AtomicBool::new(false));
            let progress = ImportRunProgress {
                run_id: run_id.clone(), source_id: source.id.clone(), state: ImportRunState::Running, done: 0, total: source.message_count,
                current: String::new(), stored: 0, skipped: 0, refusal: None,
            };
            inner.runs.insert(run_id.clone(), RunRecord { progress, result: None, cancel: cancel.clone() });
            inner.running = Some(run_id.clone());
            (run_id, source, reader, cancel)
        };
        let job = Job { book: self.clone(), store: store.clone(), run_id: run_id.clone(), source, reader, into, cancel };
        if let Err(e) = std::thread::Builder::new().name("whoosh-import".into()).spawn(move || job.run()) {
            eprintln!("import {run_id}: the import thread did not start: {e}");
            self.end(&run_id, ImportRunState::Failed, Some(import_store_failed_refusal()), 0, 0, Vec::new(), Vec::new(), 0);
            return Err(CoreError::Unexpected { reason_words: format!("the import could not start: {e}") });
        }
        Ok(CommandOutcome { undo_entry_id: None, created_id: Some(run_id) })
    }

    /// `ImportCancel`: refused in words when the run is not running; otherwise the run stops at its next batch.
    pub(crate) fn cancel(&self, run_id: &str) -> Result<CommandOutcome, CoreError> {
        let inner = lock(&self.inner);
        let record = inner.runs.get(run_id);
        check_import_cancel(record.map(|r| r.progress.clone()))?;
        if let Some(r) = record {
            r.cancel.store(true, Ordering::SeqCst);
        }
        Ok(CommandOutcome { undo_entry_id: None, created_id: None })
    }

    fn report(&self, run_id: &str, p: &mail_import::ImportProgress) {
        let progress = {
            let mut inner = lock(&self.inner);
            let Some(run) = inner.runs.get_mut(run_id) else { return };
            if run.progress.state != ImportRunState::Running {
                return;
            }
            let next = ImportRunProgress {
                done: p.seen.max(p.stored + p.skipped), total: p.total_known.or(run.progress.total), current: p.mailbox.clone(),
                stored: p.stored, skipped: p.skipped, ..run.progress.clone()
            };
            if import_progress_is_legal(next.clone()) {
                run.progress = next;
            }
            run.progress.clone()
        };
        let pace = lock(&self.pace).clone();
        if let Some(pace) = pace {
            pace(&progress);
        }
    }

    /// Closes a run: its final progress, its result, and the lock on "one run at a time" released. The numbers are what the run knows; a
    /// run that stopped before its report has only the last progress it published.
    #[allow(clippy::too_many_arguments)]
    fn end(&self, run_id: &str, state: ImportRunState, refusal: Option<ImportRefusal>, seen: u64, already_there: u64, skipped: Vec<ImportSkipped>, per_account: Vec<ImportPerAccount>, stored: u64) {
        let mut inner = lock(&self.inner);
        let Some(run) = inner.runs.get_mut(run_id) else { return };
        let result = ImportResult { run_id: run_id.to_string(), source_id: run.progress.source_id.clone(), state, seen, stored, already_there, skipped, per_account, refusal: refusal.clone() };
        run.progress = ImportRunProgress { state, done: seen, stored, skipped: result.skipped.len() as u64, refusal, ..run.progress.clone() };
        if !import_result_adds_up(result.clone()) {
            eprintln!("import {run_id}: the result does not add up (seen {seen}, stored {stored}, already there {already_there}, skipped {})", result.skipped.len());
        }
        run.result = Some(result);
        if inner.running.as_deref() == Some(run_id) {
            inner.running = None;
        }
    }

    /// The value of a state path of delta 18 (None: not one of its paths, or no value yet).
    pub(crate) fn state(&self, path: &str) -> Option<String> {
        let p: Vec<&str> = path.split('/').collect();
        let inner = lock(&self.inner);
        match p.as_slice() {
            ["import", "sources"] => Some(format!("[{}]", inner.sources.iter().map(source_json).collect::<Vec<_>>().join(","))),
            ["import", run, "progress"] => inner.runs.get(*run).map(|r| progress_json(&r.progress)),
            ["import", run, "result"] => inner.runs.get(*run).and_then(|r| r.result.as_ref()).map(result_json),
            _ => None,
        }
    }
}

fn blank_source(kind: ImportSourceKind, name: &str, location: String, count: Option<u64>, access: ImportAccess) -> ImportSource {
    ImportSource { id: import_source_id(kind, location.clone()), kind, name: name.to_string(), location, message_count: count, imported_count: 0, last_run_ms: None, access }
}

/// Whether a picked file can be opened, and how many messages it holds when that is cheap (Outlook archives count their entries; an
/// mbox has no index).
fn probe_file(file: &Path, kind: ImportSourceKind) -> (Option<u64>, ImportAccess) {
    match kind {
        ImportSourceKind::Olm => match mail_import::count_olm(file) {
            Ok(n) => (Some(n), ImportAccess::Readable),
            Err(e) => (None, access_of(&e)),
        },
        _ => match std::fs::File::open(file) {
            Ok(_) => (None, ImportAccess::Readable),
            Err(e) => (None, access_of(&mail_import::classify_io_error(file, &e))),
        },
    }
}

// ---------------------------------------------------------------- state values (keys sorted, as the fixture core writes them)

fn num(v: Option<u64>) -> String { v.map(|n| n.to_string()).unwrap_or_else(|| "null".into()) }

fn refusal_json(r: &Option<ImportRefusal>) -> String {
    match r { Some(r) => format!(r#"{{"code":{},"words":{}}}"#, json_text(&r.code), json_text(&r.words)), None => "null".into() }
}

fn source_json(s: &ImportSource) -> String {
    format!(
        r#"{{"access":{},"id":{},"imported_count":{},"kind":{},"last_run_ms":{},"location":{},"message_count":{},"name":{},"words":{}}}"#,
        json_text(&import_access_key(s.access)), json_text(&s.id), s.imported_count, json_text(&import_source_kind_key(s.kind)),
        s.last_run_ms.map(|n| n.to_string()).unwrap_or_else(|| "null".into()), json_text(&s.location), num(s.message_count), json_text(&s.name),
        json_text(&import_access_words(s.access)),
    )
}

fn progress_json(p: &ImportRunProgress) -> String {
    let fraction = import_progress_fraction(p.done, p.total).map(|f| f.to_string()).unwrap_or_else(|| "null".into());
    format!(
        r#"{{"current":{},"done":{},"fraction":{fraction},"refusal":{},"run_id":{},"skipped":{},"source_id":{},"state":{},"stored":{},"total":{}}}"#,
        json_text(&p.current), p.done, refusal_json(&p.refusal), json_text(&p.run_id), p.skipped, json_text(&p.source_id),
        json_text(&import_run_state_key(p.state)), p.stored, num(p.total),
    )
}

fn result_json(r: &ImportResult) -> String {
    let skipped = r.skipped.iter().map(|s| format!(r#"{{"origin":{},"words":{}}}"#, json_text(&s.origin), json_text(&s.words))).collect::<Vec<_>>().join(",");
    let per = r.per_account.iter().map(|a| format!(r#"{{"account_id":{},"stored":{}}}"#, json_text(&a.account_id), a.stored)).collect::<Vec<_>>().join(",");
    format!(
        r#"{{"already_there":{},"per_account":[{per}],"refusal":{},"run_id":{},"seen":{},"skipped":[{skipped}],"source_id":{},"state":{},"stored":{}}}"#,
        r.already_there, refusal_json(&r.refusal), json_text(&r.run_id), r.seen, json_text(&r.source_id), json_text(&import_run_state_key(r.state)), r.stored,
    )
}

// ---------------------------------------------------------------- the run

/// What a run counts as it writes: the rows that were NEW, by account. The pipeline's own per-account list counts every converted
/// message, already-stored ones included, so it cannot say what this run stored.
#[derive(Default)]
struct Tally {
    stored: u64,
    by_account: HashMap<String, u64>,
}

/// The store, as the import crate's one-method port: each batch goes through `Store::insert_imported`. A cancel is answered here, at the
/// next batch, with the crate's own error type carrying a mark only this file knows.
struct StoreWriter {
    store: Arc<Store>,
    source_id: String,
    at_ms: i64,
    cancel: Arc<AtomicBool>,
    tally: Arc<Mutex<Tally>>,
    never_store: bool,
}

impl mail_import::MessageSink for StoreWriter {
    fn insert(&mut self, batch: Vec<store::NewMessage>) -> Result<usize, mail_import::ImportError> {
        if self.cancel.load(Ordering::SeqCst) {
            return Err(mail_import::ImportError::Store { words: CANCEL_MARK.to_string() });
        }
        if self.never_store {
            return Ok(0);
        }
        let accounts: HashMap<String, String> = batch.iter().map(|n| (n.message.id.clone(), n.message.account_id.clone())).collect();
        let fresh = self.store.insert_imported(batch, &self.source_id, self.at_ms).map_err(|e| mail_import::ImportError::Store { words: format!("the local database refused the imported mail: {e}") })?;
        crate::bodies::crawl::enqueue_for_store(&self.store, &fresh); // index-queue: the new rows' text is cut and embedded now, not at the next launch
        let mut t = lock(&self.tally);
        for id in &fresh {
            if let Some(a) = accounts.get(id) {
                *t.by_account.entry(a.clone()).or_insert(0) += 1;
            }
        }
        t.stored += fresh.len() as u64;
        Ok(fresh.len())
    }
}

struct Job {
    book: Arc<ImportBook>,
    store: Arc<Store>,
    run_id: String,
    source: ImportSource,
    reader: mail_import::Source,
    into: ImportInto,
    cancel: Arc<AtomicBool>,
}

impl Job {
    fn run(self) {
        let run_id = self.run_id.clone();
        let book = self.book.clone();
        if catch_unwind(AssertUnwindSafe(|| self.execute())).is_err() {
            eprintln!("import {run_id}: the run stopped unexpectedly");
            let seen = lock(&book.inner).runs.get(&run_id).map(|r| r.progress.done).unwrap_or(0);
            book.end(&run_id, ImportRunState::Failed, Some(import_store_failed_refusal()), seen, 0, Vec::new(), Vec::new(), 0);
        }
    }

    fn execute(self) {
        let at_ms = now_ms();
        let stop = |state: ImportRunState, refusal: Option<ImportRefusal>| self.book.end(&self.run_id, state, refusal, 0, 0, Vec::new(), Vec::new(), 0);
        // who can receive: every account by its address, else the local Imported account; or every message to the one account chosen
        let summaries = match self.store.account_summaries() {
            Ok(s) => s,
            Err(e) => {
                eprintln!("import {}: the accounts could not be read: {e}", self.run_id);
                return stop(ImportRunState::Failed, Some(import_store_failed_refusal()));
            }
        };
        let (accounts, imported_id) = match &self.into {
            ImportInto::Account { account_id } => (Vec::new(), account_id.clone()),
            ImportInto::Imported => match self.store.add_local_account(IMPORTED_ADDRESS, IMPORTED_NAME, at_ms) {
                Ok(id) => (summaries.into_iter().filter(|a| a.id != id).map(|a| mail_import::AccountAddress { id: a.id, address: a.address }).collect(), id),
                Err(e) => {
                    eprintln!("import {}: the Imported account could not be made: {e}", self.run_id);
                    return stop(ImportRunState::Failed, Some(import_store_failed_refusal()));
                }
            },
        };
        let ctx = mail_import::ImportContext { accounts: &accounts, imported_account_id: &imported_id };
        let tally = Arc::new(Mutex::new(Tally::default()));
        let mut sink = StoreWriter { store: self.store.clone(), source_id: self.source.id.clone(), at_ms, cancel: self.cancel.clone(), tally: tally.clone(), never_store: self.book.never_store.load(Ordering::SeqCst) };
        let last: Arc<Mutex<Option<mail_import::ImportProgress>>> = Arc::new(Mutex::new(None));
        let seen_by = last.clone();
        let (book, run_id) = (self.book.clone(), self.run_id.clone());
        let outcome = mail_import::import(&self.reader, &ctx, &mut sink, &mut |p: &mail_import::ImportProgress| {
            *lock(&seen_by) = Some(p.clone());
            book.report(&run_id, p);
        });
        let (stored, per_account) = {
            let t = lock(&tally);
            let mut per: Vec<ImportPerAccount> = t.by_account.iter().map(|(a, n)| ImportPerAccount { account_id: a.clone(), stored: *n }).collect();
            per.sort_by(|a, b| a.account_id.cmp(&b.account_id));
            (t.stored, per)
        };
        match outcome {
            Ok(report) => {
                self.star_flagged(&report.flagged_ids, at_ms);
                let skipped: Vec<ImportSkipped> = report.skipped.iter().map(|s| ImportSkipped { origin: s.origin.clone(), words: s.words.clone() }).collect();
                self.book.end(&self.run_id, ImportRunState::Finished, None, report.seen, report.already_there, skipped, per_account, stored);
            }
            Err(e) => {
                eprintln!("import {}: stopped: {}", self.run_id, e.words());
                let snapshot = lock(&last).clone();
                let seen = snapshot.as_ref().map(|p| p.seen).unwrap_or(0);
                // the crate's report is lost when its run stops on an error: the skipped messages are named by their mailbox only
                let skipped = self.skipped_by_mailbox(&snapshot);
                let already = seen.saturating_sub(skipped.len() as u64).saturating_sub(stored);
                let (state, refusal) = match &e {
                    mail_import::ImportError::Store { words } if words == CANCEL_MARK => (ImportRunState::Cancelled, None),
                    mail_import::ImportError::Store { .. } => (ImportRunState::Failed, Some(import_store_failed_refusal())),
                    other => (ImportRunState::Refused, import_access_refusal(access_of(other))),
                };
                self.book.end(&self.run_id, state, refusal, seen, already, skipped, per_account, stored);
            }
        }
    }

    /// One entry per message the last report counted as skipped, named by the mailbox it sat in.
    fn skipped_by_mailbox(&self, last: &Option<mail_import::ImportProgress>) -> Vec<ImportSkipped> {
        let Some(p) = last else { return Vec::new() };
        p.mailboxes.iter().flat_map(|m| (0..m.skipped).map(|_| ImportSkipped { origin: m.mailbox.clone(), words: "could not be read and was left out".to_string() })).collect()
    }

    /// The messages the source had flagged become Starred once stored, as the sync does for `\Flagged`; only rows THIS run stored, so a
    /// star the owner removed after an earlier run is not put back.
    fn star_flagged(&self, ids: &[String], at_ms: i64) {
        if ids.is_empty() {
            return;
        }
        if let Err(e) = self.store.star_imported_in_run(ids.to_vec(), &self.source.id, at_ms) {
            eprintln!("import {}: the flagged messages were not starred: {e}", self.run_id);
        }
    }
}
