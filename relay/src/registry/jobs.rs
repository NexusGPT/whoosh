//! Push-job queue. The job row IS the idempotency ledger: it is written before the APNs call,
//! and (signal_id, device_token) is unique, so a repeated signal never queues a second push.
use super::adapter::MailSignal;
use super::{Store, StoreError};
use rusqlite::{params, OptionalExtension};
use std::collections::BTreeMap;

pub struct Job {
    pub job_id: i64,
    pub signal_id: String,
    pub device_token: String,
    pub routing_id: String,
    pub environment: String,
    pub opaque_ref: Option<String>,
}

pub fn enqueue(store: &Store, signal: &MailSignal, now: i64) -> Result<usize, StoreError> {
    store.with(|c| {
        let tx = c.transaction()?;
        let devices: Vec<(String, String)> = {
            let mut st = tx.prepare("SELECT device_token, environment FROM registration WHERE routing_id = ?1")?;
            let rows = st.query_map(params![signal.routing_id], |r| Ok((r.get(0)?, r.get(1)?)))?;
            rows.collect::<Result<_, _>>()?
        };
        let mut queued = 0;
        for (token, env) in devices {
            queued += tx.execute(
                "INSERT OR IGNORE INTO push_job (signal_id, device_token, routing_id, environment, opaque_ref, state, enqueued_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, 'queued', ?6)",
                params![signal.signal_id, token, signal.routing_id, env, signal.opaque_ref, now],
            )?;
        }
        tx.commit()?;
        Ok(queued)
    })
}

pub fn next_queued(store: &Store) -> Result<Option<Job>, StoreError> {
    store.with(|c| {
        c.query_row(
            "SELECT job_id, signal_id, device_token, routing_id, environment, opaque_ref
             FROM push_job WHERE state = 'queued' ORDER BY job_id LIMIT 1",
            [],
            |r| {
                Ok(Job {
                    job_id: r.get(0)?,
                    signal_id: r.get(1)?,
                    device_token: r.get(2)?,
                    routing_id: r.get(3)?,
                    environment: r.get(4)?,
                    opaque_ref: r.get(5)?,
                })
            },
        )
        .optional()
    })
}

pub fn finish(store: &Store, job_id: i64, state: &str, status: Option<u16>, reason: Option<&str>, now: i64) -> Result<(), StoreError> {
    store.with(|c| {
        c.execute(
            "UPDATE push_job SET state = ?2, apns_status = ?3, apns_reason = ?4, finished_at = ?5 WHERE job_id = ?1",
            params![job_id, state, status, reason, now],
        )?;
        Ok(())
    })
}

/// Jobs per state. `queued` is the queue depth the health route reports.
pub fn counts(store: &Store) -> Result<BTreeMap<String, i64>, StoreError> {
    store.with(|c| {
        let mut st = c.prepare("SELECT state, COUNT(1) FROM push_job GROUP BY state")?;
        let rows = st.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?)))?;
        rows.collect()
    })
}
