//! What one use case runs with: the ports, the config snapshot, the engine's small memory, and the events
//! it wants published (sent to the sinks after the engine lock is released).
use crate::config::OutboxConfig;
use crate::domain::Record;
use crate::ports::{
    AccountNamePort, DraftEffect, FindByKeyPort, GetEntryPort, ListOpenEntriesPort, ListPendingCopiesPort, LoadDraftPort, OnlinePort, PublishChangePort,
    PullChangesPort, ReplyContextPort, SaveEntryPort, SaveRequest, SendEntryPort,
};
use contracts::{Clock, CoreError, CoreEvent};
use send::AppendSentPort;
use std::collections::BTreeMap;
use std::sync::Arc;

pub struct SyncPorts {
    pub publish: Arc<dyn PublishChangePort>,
    pub pull: Arc<dyn PullChangesPort>,
}

pub struct Ports {
    pub save: Arc<dyn SaveEntryPort>,
    pub get: Arc<dyn GetEntryPort>,
    pub list_open: Arc<dyn ListOpenEntriesPort>,
    pub find_by_key: Arc<dyn FindByKeyPort>,
    pub pending_copies: Arc<dyn ListPendingCopiesPort>,
    pub reply_context: Arc<dyn ReplyContextPort>,
    pub load_draft: Arc<dyn LoadDraftPort>,
    pub account_names: Arc<dyn AccountNamePort>,
    pub sender: Arc<dyn SendEntryPort>,
    pub online: Arc<dyn OnlinePort>,
    pub append_sent: Arc<dyn AppendSentPort>,
    pub clock: Arc<dyn Clock>,
    pub sync: Option<SyncPorts>,
}

/// The engine's memory between calls. Nothing here is needed to stay correct after a restart:
/// a lost unpublished record is retried at worst late (a pending Sent copy lives in its row).
#[derive(Default)]
pub struct EngineState {
    pub seq: u64,
    pub unpublished: BTreeMap<String, Record>,
}

pub struct Ctx<'a> {
    pub ports: &'a Ports,
    pub cfg: &'a OutboxConfig,
    pub st: &'a mut EngineState,
    pub events: Vec<CoreEvent>,
}

impl<'a> Ctx<'a> {
    pub fn new(ports: &'a Ports, cfg: &'a OutboxConfig, st: &'a mut EngineState) -> Self {
        Self { ports, cfg, st, events: Vec::new() }
    }

    pub fn now(&self) -> i64 {
        self.ports.clock.now_ms()
    }

    /// The idempotency key of a message being queued: a random UUID (version 4), minted once, kept with the entry.
    pub fn new_key(&self) -> String {
        uuid::Uuid::new_v4().to_string()
    }

    pub fn new_id(&mut self, draft_id: &str, now_ms: i64) -> String {
        self.st.seq += 1;
        format!("{}-{}-{}-{}", self.cfg.device_id, draft_id, now_ms, self.st.seq)
    }

    /// Persist first. Only after the store holds the change: tell the streams, then the other devices.
    /// A failure to reach the other devices never fails the action; the record is retried at the next tick.
    pub fn commit(&mut self, record: &Record, draft: DraftEffect, publish: bool) -> Result<(), CoreError> {
        self.ports.save.execute(&SaveRequest { record: record.clone(), draft })?;
        self.events.push(CoreEvent::OutboxTick { entry: record.entry.clone() });
        if publish && record.scheduled {
            self.publish(record);
        }
        Ok(())
    }

    pub fn publish(&mut self, record: &Record) {
        let Some(sync) = &self.ports.sync else { return };
        match sync.publish.execute(record) {
            Ok(()) => {
                self.st.unpublished.remove(record.id());
            }
            Err(_) => {
                self.st.unpublished.insert(record.id().to_string(), record.clone());
            }
        }
    }

    pub fn get(&self, id: &str) -> Result<Record, CoreError> {
        self.ports.get.execute(id)?.ok_or_else(|| CoreError::NotFound { kind: "outbox-entry".into(), id: id.to_string() })
    }
}
