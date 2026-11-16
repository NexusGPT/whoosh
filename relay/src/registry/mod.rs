//! Store handle, device registry, push-job queue and the provider-adapter seam.
pub mod adapter;
pub mod devices;
pub mod jobs;
pub mod loopback;
pub mod schema;

/// The RELAY_* rows of docs/CONSTANTS.md as generated Rust (tools/gen-constants.py writes
/// relay/src/constants.rs). The relay reads its numbers from this file inside relay/ and declares
/// none of them again; no path leaves relay/, so the folder builds alone.
#[allow(dead_code)]
#[path = "../constants.rs"]
pub mod constants;

use rusqlite::Connection;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::sync::Notify;

#[derive(Debug)]
pub struct StoreError(pub String);

impl std::fmt::Display for StoreError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "store: {}", self.0)
    }
}

impl std::error::Error for StoreError {}

impl From<rusqlite::Error> for StoreError {
    fn from(e: rusqlite::Error) -> Self {
        StoreError(e.to_string())
    }
}

#[derive(Clone)]
pub struct Store {
    conn: Arc<Mutex<Connection>>,
}

impl Store {
    pub fn open(path: &str) -> Result<Store, StoreError> {
        let conn = Connection::open(path)?;
        conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON;")?;
        conn.execute_batch(schema::SCHEMA)?;
        Ok(Store { conn: Arc::new(Mutex::new(conn)) })
    }

    /// Runs `f` under the connection lock. Never hold the closure across an await.
    pub fn with<R>(&self, f: impl FnOnce(&mut Connection) -> rusqlite::Result<R>) -> Result<R, StoreError> {
        let mut guard = self.conn.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        Ok(f(&mut guard)?)
    }
}

pub fn now_unix() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0)
}

/// Turns a mail signal into queued push jobs and wakes the APNs worker.
pub struct Dispatcher {
    store: Store,
    wake: Notify,
}

impl Dispatcher {
    pub fn new(store: Store) -> Dispatcher {
        Dispatcher { store, wake: Notify::new() }
    }

    /// Returns the number of NEW jobs queued; a repeated signal id queues none.
    pub fn emit(&self, signal: &adapter::MailSignal) -> Result<usize, StoreError> {
        let queued = jobs::enqueue(&self.store, signal, now_unix())?;
        if queued > 0 {
            self.wake.notify_one();
        }
        Ok(queued)
    }

    pub async fn idle(&self) {
        self.wake.notified().await
    }
}
