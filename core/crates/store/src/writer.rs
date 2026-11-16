//! The single writer. One thread owns the only write connection; every write is a job sent to it, so two
//! writes can never overlap and SQLite never answers "database is locked" to our own code.
//!
//! A write declares the messages it changes BEFORE it changes them (`WriteCtx::will_change`). The writer
//! reads their contribution to the counts before and after, moves the in-memory tally by the difference,
//! and after the commit publishes: one event per stream, one table change per touched table.
use crate::changes::{Bus, TableChange};
use crate::counts::{self, Tally};
use crate::error::StoreError;
use crate::read;
use contracts::{CoreEvent, StreamKind};
use rusqlite::{Connection, Transaction, TransactionBehavior};
use std::collections::{BTreeSet, HashMap};
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::{Arc, Mutex};

pub type Job = Box<dyn FnOnce(&mut WriterState) + Send>;

/// None = the tally is unknown and the writer recomputes it before it is next needed.
pub type SharedTally = Arc<Mutex<Option<Tally>>>;
pub type Touched = Arc<Mutex<HashMap<String, TableChange>>>;

pub struct WriterState {
    pub conn: Connection,
    pub touched: Touched,
    pub bus: Arc<Bus>,
    pub tally: SharedTally,
}

/// What a write closure may use: the open transaction and the declaration of what it changes.
pub struct WriteCtx<'a> {
    pub tx: &'a Transaction<'a>,
    primary: String,
    before: Tally,
    affected: BTreeSet<String>,
    tabs: BTreeSet<String>,
    threads: BTreeSet<String>,
}

impl<'a> WriteCtx<'a> {
    /// Declare the messages this write is about to change (or create). Call it before the first statement
    /// that touches them: the counts' "before" is read here.
    pub fn will_change(&mut self, message_ids: &[String]) -> Result<(), StoreError> {
        for id in message_ids {
            if !self.affected.insert(id.clone()) {
                continue;
            }
            self.note(id)?;
            if let Some((tab, u, w)) = counts::contribution(self.tx, &self.primary, id)? {
                counts::add(&mut self.before, &tab, u, w, 1);
            }
            counts::add(&mut self.before, counts::VIPS_KEY, counts::vips_contribution(self.tx, id)?, 0, 1);
        }
        Ok(())
    }

    fn note(&mut self, id: &str) -> Result<(), StoreError> {
        if let Some(tab) = read::tab_of(self.tx, &self.primary, id)? {
            self.tabs.insert(tab);
        }
        let thread: Option<String> = self
            .tx
            .prepare_cached("SELECT thread_id FROM message WHERE id = ?1")?
            .query_row([id], |r| r.get(0))
            .map(Some)
            .or_else(|e| if matches!(e, rusqlite::Error::QueryReturnedNoRows) { Ok(None) } else { Err(e) })?;
        if let Some(t) = thread {
            self.threads.insert(t);
        }
        Ok(())
    }

    /// Also remember a thread whose members changed (a thread row written directly).
    pub fn thread_changed(&mut self, thread_id: &str) {
        self.threads.insert(thread_id.to_string());
    }
}

const INBOX_TABLES: &[&str] = &["message", "message_category", "snooze", "pin", "label_assignment", "label", "attachment", "thread", "split_tab"];
// fix-labelstreams: a pin, a rename, a move and a filing change a pinned label's tab and its figures (counts.rs `build`), so both label tables feed Counts
const COUNTS_TABLES: &[&str] = &["message", "message_category", "snooze", "pin", "split_tab", "draft", "outbox_entry", "label", "label_assignment", "vip"];
/// The counts tables other than the two label ones: a write that touches only `label_assignment` moves the Counts value only when an owner label is pinned.
const COUNTS_TABLES_BEYOND_FILING: &[&str] = &["message", "message_category", "snooze", "pin", "split_tab", "draft", "outbox_entry", "label", "vip"];
const TALLY_TABLES: &[&str] = &["message", "message_category", "snooze"];

impl WriterState {
    pub fn run<R>(&mut self, f: impl FnOnce(&mut WriteCtx<'_>) -> Result<R, StoreError>) -> Result<R, StoreError> {
        self.touched.lock().unwrap_or_else(|e| e.into_inner()).clear();
        let primary = read::primary_tab(&self.conn)?;
        let tx = self.conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let mut ctx = WriteCtx { tx: &tx, primary, before: Tally::new(), affected: BTreeSet::new(), tabs: BTreeSet::new(), threads: BTreeSet::new() };
        let outcome = catch_unwind(AssertUnwindSafe(|| f(&mut ctx)));
        let WriteCtx { primary, before, affected, mut tabs, mut threads, .. } = ctx;
        let value = match outcome {
            Ok(Ok(v)) => v,
            Ok(Err(e)) => {
                drop(tx);
                self.touched.lock().unwrap_or_else(|e| e.into_inner()).clear();
                return Err(e);
            }
            Err(_) => {
                drop(tx);
                self.touched.lock().unwrap_or_else(|e| e.into_inner()).clear();
                return Err(StoreError::JobPanicked);
            }
        };

        // The "after" of the declared messages, read inside the transaction.
        let mut after = Tally::new();
        for id in &affected {
            if let Some(tab) = read::tab_of(&tx, &primary, id)? {
                tabs.insert(tab);
            }
            if let Ok(t) = tx.query_row("SELECT thread_id FROM message WHERE id = ?1", [id], |r| r.get::<_, String>(0)) {
                threads.insert(t);
            }
            if let Some((tab, u, w)) = counts::contribution(&tx, &primary, id)? {
                counts::add(&mut after, &tab, u, w, 1);
            }
            counts::add(&mut after, counts::VIPS_KEY, counts::vips_contribution(&tx, id)?, 0, 1);
        }
        tx.commit()?;

        let touched: HashMap<String, TableChange> = std::mem::take(&mut *self.touched.lock().unwrap_or_else(|e| e.into_inner()));
        if touched.is_empty() {
            return Ok(value);
        }
        self.publish(&touched, &primary, &before, &after, !affected.is_empty(), &tabs, &threads)?;
        Ok(value)
    }

    #[allow(clippy::too_many_arguments)]
    fn publish(
        &mut self,
        touched: &HashMap<String, TableChange>,
        primary: &str,
        before: &Tally,
        after: &Tally,
        declared: bool,
        tabs: &BTreeSet<String>,
        threads: &BTreeSet<String>,
    ) -> Result<(), StoreError> {
        let has = |names: &[&str]| names.iter().any(|n| touched.contains_key(*n));

        // The tally: moved by the declared difference, or dropped when a write touched counted tables without declaring.
        {
            let mut guard = self.tally.lock().unwrap_or_else(|e| e.into_inner());
            if touched.contains_key("split_tab") || touched.contains_key("vip") || (has(TALLY_TABLES) && !declared) {
                *guard = None;
            } else if let Some(t) = guard.as_mut() {
                for (tab, (u, w)) in after {
                    counts::add(t, tab, *u, *w, 1);
                }
                for (tab, (u, w)) in before {
                    counts::add(t, tab, *u, *w, -1);
                }
            }
        }

        let mut tables: Vec<&TableChange> = touched.values().collect();
        tables.sort_by(|a, b| a.table.cmp(&b.table));
        for change in tables {
            self.bus.emit_table(change);
        }

        // fix-labelstreams: the pinned owner labels are tabs too; read ONCE per write, and only when a label table changed
        let pinned_labels: Vec<String> = if has(&["label", "label_assignment"]) { crate::label_meta::pinned_owner_label_ids(&self.conn)? } else { Vec::new() };
        if has(INBOX_TABLES) {
            let mut wanted: Vec<String> = if declared && !tabs.is_empty() { tabs.iter().cloned().collect() } else { read::enabled_tabs(&self.conn)? };
            for id in &pinned_labels {
                if !wanted.contains(id) { wanted.push(id.clone()); }
            }
            // delta 16: the All view lists every inbox row, so every inbox write refreshes a list open on it
            let all = contracts::inbox_view_id(contracts::InboxView::All);
            if !wanted.contains(&all) { wanted.push(all); }
            for tab_id in wanted {
                let rows = read::inbox(&self.conn, &contracts::InboxQuery { tab_id: tab_id.clone(), account_id: None, label_id: None, mailbox: None, account_ids: vec![], filters: vec![], smart_folder_id: None }, read::INBOX_PAGE_ROWS)?;
                self.bus.emit(StreamKind::Inbox, CoreEvent::InboxChanged { tab_id, rows });
            }
        }
        // a filing alone (label_assignment, nothing else) cannot move the Counts value while no owner label is pinned: no event then
        let filing_only = !has(COUNTS_TABLES_BEYOND_FILING);
        if has(COUNTS_TABLES) && !(filing_only && pinned_labels.is_empty()) {
            let counts = self.counts_value(primary)?;
            self.bus.emit(StreamKind::Counts, CoreEvent::CountsChanged { counts });
        }
        let message_rows_added_or_removed = touched.get("message").map(|c| c.inserts + c.deletes > 0).unwrap_or(false);
        if has(&["thread"]) || message_rows_added_or_removed {
            for id in threads {
                if let Some(thread) = read::thread(&self.conn, id)? {
                    self.bus.emit(StreamKind::Thread, CoreEvent::ThreadChanged { thread });
                }
            }
        }
        Ok(())
    }

    /// The counts value, recomputing the tally first when it is unknown.
    pub fn counts_value(&mut self, primary: &str) -> Result<contracts::Counts, StoreError> {
        let known = self.tally.lock().unwrap_or_else(|e| e.into_inner()).clone();
        let tally = match known {
            Some(t) => t,
            None => {
                let fresh = counts::recount(&self.conn, primary)?;
                *self.tally.lock().unwrap_or_else(|e| e.into_inner()) = Some(fresh.clone());
                fresh
            }
        };
        counts::build(&self.conn, &tally)
    }
}
