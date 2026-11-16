//! The contract's `CloudSync` behind the two sync ports. Scheduled entries travel as the record kind
//! `outbox-entry`; the payload is the row as JSON (the app's sync layer encrypts it).
use crate::domain::{from_row, row_to_wire, to_wire_row, wire_to_row, Record};
use crate::ports::{PublishChangePort, PullChangesPort};
use contracts::{CloudSync, CoreError, SyncChange};
use std::sync::{Arc, Mutex};

pub const RECORD_KIND: &str = "outbox-entry";

pub struct CloudPublisher {
    cloud: Arc<dyn CloudSync>,
}

impl CloudPublisher {
    pub fn new(cloud: Arc<dyn CloudSync>) -> Self {
        Self { cloud }
    }
}

impl PublishChangePort for CloudPublisher {
    fn execute(&self, record: &Record) -> Result<(), CoreError> {
        self.cloud.push(SyncChange { record_kind: RECORD_KIND.into(), id: record.id().to_string(), payload: row_to_wire(&to_wire_row(record)), changed_at_ms: record.updated_at_ms })
    }
}

pub struct CloudPuller {
    cloud: Arc<dyn CloudSync>,
    token: Mutex<Option<String>>,
}

impl CloudPuller {
    pub fn new(cloud: Arc<dyn CloudSync>) -> Self {
        Self { cloud, token: Mutex::new(None) }
    }
}

impl PullChangesPort for CloudPuller {
    /// One pull per call. Records of other kinds and records that cannot be read are skipped, so one bad
    /// record never blocks the others; the token moves on only after a successful pull.
    fn execute(&self) -> Result<Vec<Record>, CoreError> {
        let since = self.token.lock().unwrap_or_else(|e| e.into_inner()).clone();
        let batch = self.cloud.pull(since)?;
        let records = batch
            .changes
            .iter()
            .filter(|c| c.record_kind == RECORD_KIND)
            .filter_map(|c| wire_to_row(&c.payload).and_then(from_row).ok())
            .collect();
        if let Some(next) = batch.next_token {
            *self.token.lock().unwrap_or_else(|e| e.into_inner()) = Some(next);
        }
        Ok(records)
    }
}
