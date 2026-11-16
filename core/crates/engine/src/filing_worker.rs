//! The one worker that runs the label flow for the real core, on its own thread: the sync never waits for a model, a rule or a
//! store write of the flow. The sync names what it stored (`expect`, before a page is stored; `kick`, after it is) and what flipped
//! (`flipped`); the worker files the new INBOX messages through the labelling order (`file_arrivals`), then re-evaluates the older
//! messages of the touched threads (a reply or a newer message flips "unanswered" and "replied by me") through `refile_flipped`.
//! A page can be announced before another folder's page is stored: ids not found yet wait for the next kick; `settle` (the end of a
//! sync run) files what is stored and forgets the rest.
use std::collections::HashSet;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::mpsc::{channel, Sender};
use std::sync::{Arc, Mutex};

use contracts::DecisionPort;
use store::{Store, INBOX_FOLDER};

use crate::decision_model::LabelModel;
use crate::file_arrivals;
use crate::flow_ctx::{port_of, PortSlot, RerunBook, RerunStatus};
use crate::imap_source::Roles;
use crate::refile_flipped;
use crate::rust_core::now_ms;

enum Job {
    /// File what the sync stored; `last` = the run is over, ids still not stored are dropped.
    Arrived { last: bool },
    Flipped(Vec<String>),
    /// "Re-run" of one label (flow_ctx::rerun_label), on this thread so no view waits for a model.
    Rerun(String),
    /// Lane fix-aisort: the mails waiting for the AI that no label holds are asked again (`label_ai_failure::retry_ids`).
    Retry,
    Flush(Sender<()>),
}

pub(crate) struct Filing {
    port: PortSlot,
    /// The model the AI card uses when the app set no port: asked afresh at every judgement (decision_model.rs).
    model: Arc<LabelModel>,
    /// What each label's last re-run did and whether one is running (state label/<id>/rerun/*).
    rerun: RerunBook,
    pending: Arc<Mutex<Vec<String>>>,
    jobs: Mutex<Option<Sender<Job>>>,
}

struct Work {
    store: Arc<Store>,
    roles: Roles,
    port: PortSlot,
    model: Arc<LabelModel>,
    rerun: RerunBook,
    pending: Arc<Mutex<Vec<String>>>,
    /// The hidden test copy's mutant `flow-never-files`: the worker takes every job and files nothing.
    never_files: bool,
    /// A140: what the model ruled per message and label in this process, so a flip asks it only the first time a message passes a
    /// label's step 1 (`file_arrivals::ask_on_flip`).
    ai_book: file_arrivals::AiBook,
    /// Round 11: the last retry judged mails and more wait: the worker runs the next batch as soon as no other job is queued (`start`).
    more: std::sync::atomic::AtomicBool,
}

impl Filing {
    pub fn start(store: Arc<Store>, roles: Roles, never_files: bool, model: Arc<LabelModel>) -> Filing {
        let (tx, rx) = channel::<Job>();
        let port: PortSlot = Arc::new(Mutex::new(None));
        let pending = Arc::new(Mutex::new(Vec::new()));
        let rerun: RerunBook = Arc::new(Mutex::new(std::collections::HashMap::new()));
        let ai_book: file_arrivals::AiBook = Arc::new(Mutex::new(std::collections::HashMap::new()));
        let work = Work { store, roles, port: port.clone(), model: model.clone(), rerun: rerun.clone(), pending: pending.clone(), never_files, ai_book, more: std::sync::atomic::AtomicBool::new(false) };
        let spawned = std::thread::Builder::new().name("whoosh-filing".into()).spawn(move || {
            loop {
                // round 11: while mails still wait for the AI after a batch that judged some, the next batch runs as soon as no other job
                // is queued: an arrival, a flip, a Re-run or a flush always goes first (the yield between batches); the loop ends when the
                // core goes (the channel closes)
                let job = if work.more.swap(false, std::sync::atomic::Ordering::SeqCst) {
                    match rx.try_recv() {
                        Ok(job) => { work.more.store(true, std::sync::atomic::Ordering::SeqCst); job }
                        Err(std::sync::mpsc::TryRecvError::Empty) => Job::Retry,
                        Err(std::sync::mpsc::TryRecvError::Disconnected) => break,
                    }
                } else {
                    match rx.recv() { Ok(job) => job, Err(_) => break }
                };
                let flush = if let Job::Flush(done) = &job { Some(done.clone()) } else { None };
                if catch_unwind(AssertUnwindSafe(|| work.handle(job))).is_err() {
                    eprintln!("filing: a job stopped unexpectedly; the next one runs");
                }
                // round 16: the on-device model answered after a remembered failure (its weights arrived): the mails that waited meanwhile
                // are asked in the background, batch after batch, as after a Save (the `more` loop above), with no Re-run
                if work.model.take_recovered() {
                    crate::label_ai_failure::ask_waiting_again(work.store.path());
                    work.more.store(true, std::sync::atomic::Ordering::SeqCst);
                }
                if let Some(done) = flush {
                    let _ = done.send(());
                }
            }
        });
        if let Err(e) = spawned {
            eprintln!("filing: the label flow could not start its worker, new mail is not sorted into labels: {e}");
        }
        Filing { port, model, rerun, pending, jobs: Mutex::new(Some(tx)) }
    }

    fn send(&self, job: Job) {
        if let Some(tx) = self.jobs.lock().unwrap_or_else(|e| e.into_inner()).as_ref() {
            let _ = tx.send(job);
        }
    }

    /// Messages the sync is about to store (the page is fetched, not stored yet).
    pub fn expect(&self, ids: impl IntoIterator<Item = String>) {
        self.pending.lock().unwrap_or_else(|e| e.into_inner()).extend(ids);
    }

    /// A page was stored: file what is stored of the expected messages.
    pub fn kick(&self) {
        self.send(Job::Arrived { last: false });
    }

    /// The run is over: file what is stored and forget the rest.
    pub fn settle(&self) {
        self.send(Job::Arrived { last: true });
        self.send(Job::Retry);
    }

    /// Round 11: the label's step 1 or AI step changed (a Save, a card, its AI inputs): its picked mails that wait for the AI are judged now, in
    /// the background, batch after batch until none waits (no sync needed). Returns at once.
    pub fn judge_label(&self, store_path: &std::path::Path, label_id: &str) {
        crate::label_ai_failure::judge_label_now(store_path, label_id);
        self.send(Job::Retry);
    }

    /// Lane fix-aisort: asks the AI again, in the background, about the mails it could not judge (and, at start, those whose old "no" was
    /// cleared once). Returns at once.
    pub fn retry_waiting(&self) {
        self.send(Job::Retry);
    }

    /// Facts flipped on these messages (a read, a star): re-evaluate them now.
    pub fn flipped(&self, ids: Vec<String>) {
        if !ids.is_empty() {
            self.send(Job::Flipped(ids));
        }
    }

    /// Starts the re-run of a label on the worker and returns at once; its progress is `rerun_status`.
    pub fn rerun(&self, label_id: &str) {
        self.rerun.lock().unwrap_or_else(|e| e.into_inner()).entry(label_id.to_string()).or_default().running = true;
        self.send(Job::Rerun(label_id.to_string()));
    }

    pub(crate) fn rerun_book(&self) -> RerunBook { self.rerun.clone() }

    pub(crate) fn rerun_status(&self, label_id: &str) -> RerunStatus {
        self.rerun.lock().unwrap_or_else(|e| e.into_inner()).get(label_id).cloned().unwrap_or_default()
    }

    /// Waits until every job queued so far has run (the tests and the hidden check read after it).
    pub fn flush(&self) {
        let (tx, rx) = channel();
        self.send(Job::Flush(tx));
        let _ = rx.recv();
    }

    pub fn set_port(&self, port: Option<Arc<dyn DecisionPort>>) {
        *self.port.lock().unwrap_or_else(|e| e.into_inner()) = port;
    }

    /// The port for this judgement: what the app set, else the check's, else the model the settings give NOW (None while none is set).
    /// A reader-facing act's port: its on-device call takes no turn at the gate the worker's calls share with the junk filter.
    pub fn port(&self) -> Option<Arc<dyn DecisionPort>> {
        port_of(&self.port).or_else(|| self.model.port())
    }

    /// The port the app or a check set, None when neither did (the junk filter reads it to keep its opinion on this Mac, junk_filter.rs).
    pub(crate) fn app_port(&self) -> Option<Arc<dyn DecisionPort>> {
        port_of(&self.port)
    }

    /// Whether a model can answer the label AI now: a port the app or a check set, else the saved decision model with nothing refusing it.
    pub(crate) fn ai_can_answer(&self) -> bool {
        // round 16: a Laya failure that only waits for the model does not stop the asking (`LabelModel::blocks_asking`)
        port_of(&self.port).is_some() || (self.model.port().is_some() && !self.model.blocks_asking())
    }

    pub(crate) fn label_model(&self) -> &LabelModel {
        &self.model
    }
}

impl Work {
    fn handle(&self, job: Job) {
        match job {
            Job::Arrived { last } => {
                self.arrived(last);
                // delta 18: a reminder whose time came with no reply by then is marked fired once (the notification fires once)
                crate::reminders::fire_due_and_return(&self.store, &self.roles, now_ms());
            }
            Job::Flipped(ids) => self.flip(&ids),
            Job::Rerun(label) => {
                let port = port_of(&self.port).or_else(|| self.model.queued_port());
                // round 16 (main114: Re-run after the download read total 0, model_pending true): a Laya failure that only waits for the
                // model no longer skips the AI; the model is asked, and its first answer clears the failure
                let ai_ok = port_of(&self.port).is_some() || (port.is_some() && !self.model.blocks_asking());
                if let Err(e) = crate::flow_ctx::rerun_label(&self.store, port, ai_ok, &self.roles, &label, now_ms(), &self.rerun) {
                    eprintln!("filing: the re-run of {label} stopped: {e}");
                }
            }
            Job::Retry => self.retry(),
            Job::Flush(_) => {}
        }
    }

    /// Lane fix-aisort: the waiting mails no label holds, asked again as arrivals (at most `AI_RETRY_MAX` per call). Nothing when no model
    /// is set (nothing could answer: the refusal already says why) or the hidden copy's `flow-never-files` mutant is on.
    fn retry(&self) {
        if self.never_files {
            return;
        }
        let Some(port) = port_of(&self.port).or_else(|| self.model.queued_port()) else { return };
        let ids = crate::label_ai_failure::retry_ids(&self.store);
        if ids.is_empty() {
            return;
        }
        if let Err(e) = file_arrivals::run_with_book(&self.store, Some(port), &self.roles, &ids, now_ms(), Some(&self.ai_book)) {
            crate::log::log_line(&crate::log::path_beside(self.store.path()), &format!("label AI: {} waiting mails were not asked again: {e}", ids.len()));
            return;
        }
        // round 11: the next batch at once (`start`'s loop), while the batch made progress (one of its mails no longer waits) and mails still
        // wait. Bounded: every batch is at most AI_RETRY_MAX mails, each mail leaves the waiting set when judged, set aside or filed, and a
        // batch where none left (the model down: every mail failed) ends the loop until the next sync's end or start.
        let next = crate::label_ai_failure::retry_ids(&self.store);
        let progressed = ids.iter().any(|id| !next.contains(id));
        if progressed && !next.is_empty() {
            self.more.store(true, std::sync::atomic::Ordering::SeqCst);
        }
    }

    fn arrived(&self, last: bool) {
        let mut ids: Vec<String> = std::mem::take(&mut *self.pending.lock().unwrap_or_else(|e| e.into_inner()));
        ids.sort();
        ids.dedup();
        let (stored, missing): (Vec<String>, Vec<String>) = ids.into_iter().partition(|id| self.store.message(id).ok().flatten().is_some());
        if !last && !missing.is_empty() {
            self.pending.lock().unwrap_or_else(|e| e.into_inner()).extend(missing);
        }
        if stored.is_empty() || self.never_files {
            return;
        }
        let now = now_ms();
        match file_arrivals::run_with_book(&self.store, port_of(&self.port).or_else(|| self.model.queued_port()), &self.roles, &stored, now, Some(&self.ai_book)) {
            Ok(filed) => {
                for f in filed.iter().filter(|f| matches!(f.ai, file_arrivals::AiStep::Refused(_))) {
                    eprintln!("filing {}: {}", f.message_id, f.decided_by);
                }
            }
            Err(e) => eprintln!("filing: new mail was not sorted into labels: {e}"),
        }
        crate::replies_work::arrived(&self.store, &stored); // reply-engine: the auto use looks at the new mail once its labels are filed
        let mates = self.older_in_threads(&stored);
        self.flip(&mates);
    }

    /// The INBOX messages of the threads the stored ones belong to whose facts the stored message changes. A stored INBOX message
    /// (a newer one) flips the "unanswered" of the OLDER messages of its thread. A stored message of the OWNER's (the Sent copy of a
    /// reply, which may reach the store after newer inbound mail: a backfill, a late sync) flips every other INBOX message of its thread:
    /// the older ones lose "unanswered" and the newer ones gain "already answered by me" (owner ruling 2026-11-10: I wrote earlier in
    /// the thread). The re-check stays bounded by the threads of the messages just stored, INBOX messages only, each once, and it
    /// never asks the model (`refile_flipped`); an inbound message flips nothing that is newer than itself.
    fn older_in_threads(&self, stored: &[String]) -> Vec<String> {
        let skip: HashSet<&String> = stored.iter().collect();
        let mut out: Vec<String> = Vec::new();
        let mut seen: HashSet<String> = HashSet::new();
        for id in stored {
            let Some(m) = self.store.message(id).ok().flatten() else { continue };
            let Some(t) = self.store.thread(&m.thread_id).ok().flatten() else { continue };
            for mate in t.message_ids.iter().filter(|x| !skip.contains(x)) {
                let flips = self.store.message(mate).ok().flatten().is_some_and(|o| o.folder == INBOX_FOLDER && (m.is_outgoing || o.received_at_ms < m.received_at_ms));
                if flips && seen.insert(mate.clone()) {
                    out.push(mate.clone());
                }
            }
        }
        out
    }

    /// A140: a flipped fact re-reads steps 1 and 3 at once with no model (`refile_flipped`: the message leaves or joins); then, on this
    /// worker and never on a view's path, a message an AI-on label's step-1 chain picks for the first time is asked to the model once
    /// (`file_arrivals::ask_on_flip`).
    fn flip(&self, ids: &[String]) {
        if ids.is_empty() || self.never_files {
            return;
        }
        let port = port_of(&self.port).or_else(|| self.model.queued_port());
        if let Err(e) = refile_flipped::run(&self.store, port.clone(), &self.roles, ids, now_ms()) {
            eprintln!("filing: a flipped fact was not applied to its message: {e}");
        }
        if let Err(e) = file_arrivals::ask_on_flip(&self.store, port, &self.roles, ids, now_ms(), &self.ai_book) {
            eprintln!("filing: a message a flipped fact made pass a label's entry rules was not sorted by the AI: {e}");
        }
    }
}
