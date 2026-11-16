use contracts::{ActionFailed, Draft, OutboxEntry, OutboxState};
use send::ReplyContext;

/// One outgoing message with what the stream's `OutboxEntry` does not carry.
///
/// Invariant: `entry.state` and the other fields change only through the methods below, and a record that is
/// Sent or Cancelled never becomes anything else.
#[derive(Debug, Clone, PartialEq)]
pub struct Record {
    pub entry: OutboxEntry,
    /// When it was due to leave: the end of the undo window, or the scheduled time. Kept after sending, so
    /// "late" can be read from the record alone.
    pub due_at_ms: i64,
    /// Created by "send later". Only these travel through sync.
    pub scheduled: bool,
    /// Delivery attempts begun. A wait for the network is not an attempt.
    pub attempts: u32,
    /// Failed only because the network was off: the engine sends it again by itself on reconnect.
    pub waiting: bool,
    pub updated_at_ms: i64,
    /// Minted once when the message is queued and kept with the entry for good. No two entries with this key
    /// ever both deliver: the send path asks whether the key is already recorded as sent.
    pub idempotency_key: String,
    /// What the message answers or forwards, resolved when it was queued (In-Reply-To and References).
    pub threading: Option<ReplyContext>,
    /// Set when the message was sent but the Sent folder did not take its copy. The message stays sent;
    /// only the copy is retried. Kept in the row, so a restart does not lose it.
    pub copy_pending: Option<CopyPending>,
    /// fix-realcore round 15: a failed try whose connection broke before the end of the body is tried again by itself at this time
    /// (SEND_RETRY_* in docs/CONSTANTS.md); None for a failure that waits for the owner's Retry (or for the network, `waiting`).
    pub retry_at_ms: Option<i64>,
}

/// A delivered message whose Sent copy is still to be saved.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CopyPending {
    /// "sent, copy not saved" and why, in the owner's words.
    pub words: String,
    /// The exact bytes that were delivered.
    pub rfc822: Vec<u8>,
}

impl Record {
    pub fn undoable(id: String, draft: Draft, now_ms: i64, window_ms: i64, idempotency_key: String, threading: Option<ReplyContext>) -> Self {
        let until = now_ms + window_ms;
        Self { entry: OutboxEntry { id, draft, state: OutboxState::Undoable { until_ms: until }, created_at_ms: now_ms }, due_at_ms: until, scheduled: false, attempts: 0, waiting: false, updated_at_ms: now_ms, idempotency_key, threading, copy_pending: None, retry_at_ms: None }
    }

    pub fn scheduled(id: String, draft: Draft, now_ms: i64, at_ms: i64, idempotency_key: String, threading: Option<ReplyContext>) -> Self {
        Self { entry: OutboxEntry { id, draft, state: OutboxState::Scheduled { at_ms }, created_at_ms: now_ms }, due_at_ms: at_ms, scheduled: true, attempts: 0, waiting: false, updated_at_ms: now_ms, idempotency_key, threading, copy_pending: None, retry_at_ms: None }
    }

    pub fn id(&self) -> &str {
        &self.entry.id
    }

    /// May a delivery start from here? The one guard that keeps a message from leaving twice:
    /// Sending, Sent and Cancelled never deliver.
    pub fn may_deliver(&self) -> bool {
        matches!(self.entry.state, OutboxState::Undoable { .. } | OutboxState::Scheduled { .. } | OutboxState::Failed { .. })
    }

    /// Not yet sent or cancelled: what `WhooshCore::outbox()` lists.
    pub fn is_open(&self) -> bool {
        !matches!(self.entry.state, OutboxState::Sent { .. } | OutboxState::Cancelled)
    }

    pub fn begin_sending(&mut self, now_ms: i64) {
        self.attempts += 1;
        self.waiting = false;
        self.retry_at_ms = None;
        self.entry.state = OutboxState::Sending;
        self.updated_at_ms = now_ms;
    }

    pub fn mark_sent(&mut self, sent_at_ms: i64, now_ms: i64) {
        self.waiting = false;
        self.retry_at_ms = None;
        self.entry.state = OutboxState::Sent { at_ms: sent_at_ms };
        self.updated_at_ms = now_ms;
    }

    /// Delivered, but the Sent folder refused the copy. The state stays Sent.
    pub fn copy_not_saved(&mut self, copy: CopyPending, now_ms: i64) {
        self.copy_pending = Some(copy);
        self.updated_at_ms = now_ms;
    }

    pub fn copy_saved(&mut self, now_ms: i64) {
        self.copy_pending = None;
        self.updated_at_ms = now_ms;
    }

    pub fn mark_failed(&mut self, failure: ActionFailed, waiting: bool, now_ms: i64) {
        self.waiting = waiting;
        self.retry_at_ms = None;
        self.entry.state = OutboxState::Failed { failure, attempts: self.attempts };
        self.updated_at_ms = now_ms;
    }

    /// fix-realcore round 15: the try failed because the connection broke before the end of the body; the entry is tried again at `retry_at_ms`
    /// by itself (the app reads "sending, try N" from it), and only after the last try does it stay failed.
    pub fn mark_retrying(&mut self, failure: ActionFailed, retry_at_ms: i64, now_ms: i64) {
        self.waiting = false;
        self.retry_at_ms = Some(retry_at_ms);
        self.entry.state = OutboxState::Failed { failure, attempts: self.attempts };
        self.updated_at_ms = now_ms;
    }

    /// fix-realcore round 15: the pause before try `attempts + 1` of a broken connection, or None when no try is left. SEND_RETRY_START_S
    /// doubling per try, at most SEND_RETRY_PAUSE_MAX_S; SEND_RETRY_TRIES_MAX tries in all.
    pub fn retry_pause_ms(attempts: u32) -> Option<i64> {
        use contracts::constants::{SEND_RETRY_PAUSE_MAX_S, SEND_RETRY_START_S, SEND_RETRY_TRIES_MAX};
        if u64::from(attempts) >= SEND_RETRY_TRIES_MAX { return None; }
        let s = SEND_RETRY_START_S.saturating_mul(1u64 << attempts.saturating_sub(1).min(20)).min(SEND_RETRY_PAUSE_MAX_S);
        Some(s as i64 * 1000)
    }

    pub fn mark_cancelled(&mut self, now_ms: i64) {
        self.waiting = false;
        self.retry_at_ms = None;
        self.entry.state = OutboxState::Cancelled;
        self.updated_at_ms = now_ms;
    }

    /// Left more than `late_after_ms` after its time (no device was awake at the time).
    pub fn is_late(&self, late_after_ms: i64) -> bool {
        matches!(self.entry.state, OutboxState::Sent { at_ms } if at_ms - self.due_at_ms > late_after_ms)
    }

    /// How far a state has got. Sync merges by this first, then by time, so a clock that runs ahead can
    /// never bring a sent message back to Scheduled.
    pub fn rank(&self) -> u8 {
        match self.entry.state {
            OutboxState::Undoable { .. } | OutboxState::Scheduled { .. } | OutboxState::Late { .. } => 0,
            OutboxState::Failed { .. } | OutboxState::WaitingForNetwork { .. } => 1,
            OutboxState::Cancelled => 2,
            OutboxState::Sending => 3,
            OutboxState::Sent { .. } => 4,
        }
    }

    /// Does a record that arrived through sync replace this one?
    pub fn replaced_by(&self, remote: &Record) -> bool {
        (remote.rank(), remote.updated_at_ms) > (self.rank(), self.updated_at_ms)
    }
}
