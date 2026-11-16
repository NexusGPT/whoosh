//! The facade the app calls. Thin: it takes the engine lock, runs one use case, releases the lock, and only
//! then hands the events to the sinks (a sink may call back into the outbox).
use crate::config::{checked_window, OutboxConfig};
use crate::ctx::{Ctx, EngineState, Ports};
use crate::usecases::deliver_entry::Outcome;
use crate::usecases::queue_send::Enqueue;
use crate::usecases::{cancel_send, finish_sent_copies, pull_synced, queue_send, recover_interrupted, retry_send, run_due, schedule_send};
use contracts::{CoreError, CoreEvent, Draft, EventSink, OutboxEntry};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};


#[derive(Debug, Clone, PartialEq)]
pub struct EntryView {
    pub entry: OutboxEntry,
    pub due_at_ms: i64,
    pub scheduled: bool,
    /// Sent more than `late_after_ms` after its time: the Scheduled mailbox marks it late.
    pub late: bool,
    /// The idempotency key of the message: a retried Send with this key never makes a second entry.
    pub idempotency_key: String,
    /// "Sent, copy not saved" and why, while the Sent folder has not taken the copy of a delivered message.
    pub copy_not_saved: Option<String>,
    /// fix-realcore round 15: delivery tries begun, and the time of the next automatic try after a broken connection (None: no automatic try).
    pub attempts: u32,
    pub retry_at_ms: Option<i64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SendOutcome {
    Delivered,
    Waiting,
    Failed,
}

#[derive(Debug, Default, Clone, PartialEq)]
pub struct TickReport {
    pub pulled: usize,
    pub delivered: Vec<String>,
    pub waiting: usize,
    pub failed: Vec<String>,
    pub copies_stored: usize,
    /// The words of a sync failure; sending still ran.
    pub sync_error: Option<String>,
}

pub struct Outbox {
    ports: Ports,
    cfg: Mutex<OutboxConfig>,
    state: Mutex<EngineState>,
    sinks: Mutex<Vec<(u64, Arc<dyn EventSink>)>>,
    next_sub: AtomicU64,
}

fn locked<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

impl Outbox {
    /// Build the outbox and settle what a previous run left half done (Sending becomes Failed with words).
    pub fn open(ports: Ports, cfg: OutboxConfig) -> Result<Outbox, CoreError> {
        let outbox = Outbox { ports, cfg: Mutex::new(cfg), state: Mutex::new(EngineState::default()), sinks: Mutex::new(Vec::new()), next_sub: AtomicU64::new(1) };
        outbox.run(recover_interrupted::execute)?;
        Ok(outbox)
    }

    fn run<R>(&self, f: impl FnOnce(&mut Ctx) -> R) -> R {
        let cfg = locked(&self.cfg).clone();
        let mut st = locked(&self.state);
        let mut ctx = Ctx::new(&self.ports, &cfg, &mut st);
        let result = f(&mut ctx);
        let events = std::mem::take(&mut ctx.events);
        drop(st);
        self.dispatch(events);
        result
    }

    fn dispatch(&self, events: Vec<CoreEvent>) {
        let sinks: Vec<_> = locked(&self.sinks).iter().map(|(_, s)| s.clone()).collect();
        for event in events {
            for sink in &sinks {
                sink.on_event(event.clone());
            }
        }
    }

    /// Command::Send { draft_id }
    pub fn send_draft(&self, draft_id: &str) -> Result<EntryView, CoreError> {
        let draft = self.ports.load_draft.execute(draft_id)?;
        self.send(draft)
    }

    pub fn send(&self, draft: Draft) -> Result<EntryView, CoreError> {
        let record = self.run(|ctx| queue_send::execute(ctx, draft, Enqueue::default()))?;
        Ok(self.view(&record))
    }

    /// Send with the caller's idempotency key. The caller mints one key per tap of Send and reuses it when it
    /// retries (a timeout, a restart): the second call returns the first entry and queues nothing, even when the
    /// draft has already left the draft table.
    pub fn send_draft_keyed(&self, draft_id: &str, key: &str) -> Result<EntryView, CoreError> {
        if let Some(found) = self.run(|ctx| queue_send::existing(ctx, key))? {
            return Ok(self.view(&found));
        }
        let draft = self.ports.load_draft.execute(draft_id)?;
        let record = self.run(|ctx| queue_send::execute(ctx, draft, Enqueue { key: Some(key.to_string()), forward_of: None }))?;
        Ok(self.view(&record))
    }

    /// Send a forward: the message carries `References` of the thread it is forwarded from, and no `In-Reply-To`.
    pub fn send_forward(&self, draft_id: &str, forwarded_thread_id: &str) -> Result<EntryView, CoreError> {
        let draft = self.ports.load_draft.execute(draft_id)?;
        let record = self.run(|ctx| queue_send::execute(ctx, draft, Enqueue { key: None, forward_of: Some(forwarded_thread_id.to_string()) }))?;
        Ok(self.view(&record))
    }

    /// Command::Schedule { draft_id, at_ms }
    pub fn schedule_draft(&self, draft_id: &str, at_ms: i64) -> Result<EntryView, CoreError> {
        let draft = self.ports.load_draft.execute(draft_id)?;
        self.schedule(draft, at_ms)
    }

    pub fn schedule(&self, draft: Draft, at_ms: i64) -> Result<EntryView, CoreError> {
        let record = self.run(|ctx| schedule_send::execute(ctx, draft, at_ms, Enqueue::default()))?;
        Ok(self.view(&record))
    }

    /// Command::CancelSend { outbox_id }
    pub fn cancel(&self, outbox_id: &str) -> Result<EntryView, CoreError> {
        let record = self.run(|ctx| cancel_send::execute(ctx, outbox_id))?;
        Ok(self.view(&record))
    }

    pub fn retry(&self, outbox_id: &str) -> Result<SendOutcome, CoreError> {
        match self.run(|ctx| retry_send::execute(ctx, outbox_id))? {
            Outcome::Delivered => Ok(SendOutcome::Delivered),
            Outcome::Waiting => Ok(SendOutcome::Waiting),
            Outcome::Failed | Outcome::Skipped(_) => Ok(SendOutcome::Failed),
        }
    }

    /// Call on a timer, on wake, and on reconnect. Pulls what other devices changed, sends what is due,
    /// retries Sent copies and unpublished records.
    pub fn tick(&self) -> Result<TickReport, CoreError> {
        self.run(|ctx| {
            let mut report = TickReport::default();
            match pull_synced::execute(ctx) {
                Ok(n) => report.pulled = n,
                Err(e) => report.sync_error = Some(e.to_string()),
            }
            run_due::execute(ctx, &mut report)?;
            report.copies_stored = finish_sent_copies::execute(ctx);
            Ok(report)
        })
    }

    /// The Settings value for the undo window, refused outside the ledger's bounds.
    pub fn set_undo_window_s(&self, seconds: u32) -> Result<(), CoreError> {
        let checked = checked_window(seconds)?;
        let mut cfg = locked(&self.cfg);
        *cfg = cfg.clone().with_undo_window_s(checked)?;
        Ok(())
    }

    pub fn get(&self, outbox_id: &str) -> Result<Option<EntryView>, CoreError> {
        Ok(self.ports.get.execute(outbox_id)?.map(|r| self.view(&r)))
    }

    /// `WhooshCore::outbox()`: every entry not yet sent or cancelled, oldest first.
    pub fn open_entries(&self) -> Result<Vec<OutboxEntry>, CoreError> {
        Ok(self.ports.list_open.execute()?.into_iter().map(|r| r.entry).collect())
    }

    /// The Outbox stream: the current state first (one OutboxTick per open entry), then every change.
    pub fn subscribe(&self, sink: Arc<dyn EventSink>) -> Result<u64, CoreError> {
        for entry in self.open_entries()? {
            sink.on_event(CoreEvent::OutboxTick { entry });
        }
        let id = self.next_sub.fetch_add(1, Ordering::SeqCst);
        locked(&self.sinks).push((id, sink));
        Ok(id)
    }

    pub fn unsubscribe(&self, subscription: u64) {
        locked(&self.sinks).retain(|(id, _)| *id != subscription);
    }

    fn view(&self, r: &crate::domain::Record) -> EntryView {
        let late_after_ms = locked(&self.cfg).late_after_ms;
        EntryView { entry: r.entry.clone(), due_at_ms: r.due_at_ms, scheduled: r.scheduled, late: r.is_late(late_after_ms), idempotency_key: r.idempotency_key.clone(), copy_not_saved: r.copy_pending.as_ref().map(|c| c.words.clone()), attempts: r.attempts, retry_at_ms: r.retry_at_ms }
    }
}
