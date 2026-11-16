//! Shared by the four adapters (each includes this file with `#[path = "../ledger.rs"]`).
//!
//! A binding is what a device tells an adapter once, with its own credential, so the relay can
//! route a later provider event WITHOUT holding any credential: provider key -> routing id, plus a
//! verifier (a hash or a public key, never a secret) and the time the provider's subscription ends.
//! The relay never renews a provider subscription itself, because renewing needs the mailbox
//! credential. Its renew job only tells the device (one signal) that renewal is due.
//!
//! The ledger lives in its OWN sqlite file, never in the relay store: that store has a schema
//! allowlist (no address, no credential column) and this table must not widen it.
#![allow(dead_code)]

use crate::registry::adapter::{AdapterError, MailSignal};
use crate::registry::Dispatcher;
use hmac::{Hmac, Mac};
use rusqlite::{params, Connection, OptionalExtension};
use sha2::Sha256;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

pub trait Clock: Send + Sync {
    /// Unix seconds.
    fn now(&self) -> i64;
}

pub struct SystemClock;

impl Clock for SystemClock {
    fn now(&self) -> i64 {
        SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0)
    }
}

/// Keyed hash of a provider identifier. The relay stores this, never the address or account id.
pub fn mac(key: &[u8], material: &str) -> String {
    let mut m = Hmac::<Sha256>::new_from_slice(key).expect("hmac accepts any key length");
    m.update(material.as_bytes());
    hex::encode(m.finalize().into_bytes())
}

/// The key for `mac`. RELAY_BINDING_KEY keeps bindings valid across restarts; without it a random
/// key is used and every binding dies on restart (logged loudly, same stance as the tracking key).
pub fn binding_key() -> Vec<u8> {
    match std::env::var("RELAY_BINDING_KEY").ok().filter(|v| !v.trim().is_empty()) {
        Some(k) => k.into_bytes(),
        None => {
            tracing::error!("RELAY_BINDING_KEY is not set: provider bindings use a random key and are lost on restart");
            let mut k = [0u8; 32];
            rand::RngCore::fill_bytes(&mut rand::rngs::OsRng, &mut k);
            k.to_vec()
        }
    }
}

/// `<RELAY_DB>.<family>`; the default database name matches main.rs.
pub fn ledger_path(family: &str) -> String {
    let db = std::env::var("RELAY_DB").ok().filter(|v| !v.trim().is_empty()).unwrap_or_else(|| "relay.sqlite3".into());
    format!("{db}.{family}")
}

pub enum BindError {
    Taken,
    Store(String),
}

impl From<BindError> for AdapterError {
    fn from(e: BindError) -> AdapterError {
        match e {
            BindError::Taken => AdapterError::Unauthorized("this account is already bound to another routing id; release it from the owning device first".into()),
            BindError::Store(m) => {
                tracing::error!("binding ledger: {m}");
                AdapterError::Malformed("the relay could not store this binding".into())
            }
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Binding {
    pub key_id: String,
    pub routing_id: String,
    pub expires_at: Option<i64>,
    pub due_at: Option<i64>,
    pub proof: Option<String>,
    pub flags: i64,
}

pub struct Ledger {
    family: &'static str,
    conn: Mutex<Connection>,
}

impl Ledger {
    /// Falls back to memory with an error log when the file cannot be opened: the relay still boots.
    pub fn open(family: &'static str, path: &str) -> Ledger {
        let conn = Connection::open(path).or_else(|e| {
            tracing::error!("{family} ledger cannot open {path}: {e}; bindings are kept in memory only");
            Connection::open_in_memory()
        });
        let conn = conn.expect("an in-memory sqlite database always opens");
        let created = conn.execute_batch(
            "PRAGMA journal_mode=WAL;
             CREATE TABLE IF NOT EXISTS binding (
                 family TEXT NOT NULL, key_id TEXT NOT NULL, routing_id TEXT NOT NULL,
                 expires_at INTEGER, due_at INTEGER, proof TEXT, flags INTEGER NOT NULL DEFAULT 0,
                 bound_at INTEGER NOT NULL, PRIMARY KEY (family, key_id));",
        );
        if let Err(e) = created {
            tracing::error!("{family} ledger schema: {e}");
        }
        Ledger { family, conn: Mutex::new(conn) }
    }

    fn with<R>(&self, f: impl FnOnce(&Connection) -> rusqlite::Result<R>) -> Result<R, String> {
        let guard = self.conn.lock().unwrap_or_else(|p| p.into_inner());
        f(&guard).map_err(|e| e.to_string())
    }

    /// Stores or refreshes a binding. A key already bound to ANOTHER routing id is refused: the
    /// routing id is the secret that owns the binding, so a stranger who knows an address cannot
    /// take its alerts over. The owner releases it with `unbind`.
    pub fn bind(&self, b: &Binding, now: i64) -> Result<(), BindError> {
        self.with(|c| {
            let owner: Option<String> = c
                .query_row("SELECT routing_id FROM binding WHERE family = ?1 AND key_id = ?2", params![self.family, b.key_id], |r| r.get(0))
                .optional()?;
            if owner.as_deref().is_some_and(|o| o != b.routing_id) {
                return Ok(false);
            }
            c.execute(
                "INSERT INTO binding (family, key_id, routing_id, expires_at, due_at, proof, flags, bound_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
                 ON CONFLICT (family, key_id) DO UPDATE SET routing_id = excluded.routing_id, expires_at = excluded.expires_at,
                    due_at = excluded.due_at, proof = excluded.proof, flags = excluded.flags, bound_at = excluded.bound_at",
                params![self.family, b.key_id, b.routing_id, b.expires_at, b.due_at, b.proof, b.flags, now],
            )?;
            Ok(true)
        })
        .map_err(BindError::Store)
        .and_then(|stored| if stored { Ok(()) } else { Err(BindError::Taken) })
    }

    /// Releases a binding, only for the routing id that owns it. Idempotent.
    pub fn unbind(&self, key_id: &str, routing_id: &str) -> Result<(), String> {
        self.with(|c| c.execute("DELETE FROM binding WHERE family = ?1 AND key_id = ?2 AND routing_id = ?3", params![self.family, key_id, routing_id]).map(|_| ()))
    }

    pub fn get(&self, key_id: &str) -> Result<Option<Binding>, String> {
        self.with(|c| {
            c.query_row(
                "SELECT key_id, routing_id, expires_at, due_at, proof, flags FROM binding WHERE family = ?1 AND key_id = ?2",
                params![self.family, key_id],
                row,
            )
            .optional()
        })
    }

    /// Bindings whose renewal is due and whose provider subscription has not yet ended.
    pub fn due(&self, now: i64) -> Result<Vec<Binding>, String> {
        self.with(|c| {
            let mut st = c.prepare(
                "SELECT key_id, routing_id, expires_at, due_at, proof, flags FROM binding
                 WHERE family = ?1 AND due_at IS NOT NULL AND due_at <= ?2 AND (expires_at IS NULL OR expires_at > ?2) ORDER BY due_at",
            )?;
            let rows = st.query_map(params![self.family, now], row)?;
            rows.collect()
        })
    }

    pub fn postpone(&self, key_id: &str, next_due: i64) -> Result<(), String> {
        self.with(|c| {
            c.execute("UPDATE binding SET due_at = ?3 WHERE family = ?1 AND key_id = ?2", params![self.family, key_id, next_due]).map(|_| ())
        })
    }
}

fn row(r: &rusqlite::Row<'_>) -> rusqlite::Result<Binding> {
    Ok(Binding { key_id: r.get(0)?, routing_id: r.get(1)?, expires_at: r.get(2)?, due_at: r.get(3)?, proof: r.get(4)?, flags: r.get(5)? })
}

/// One pass of the renew job. A due binding yields ONE signal whose id is derived from the due
/// time, so a crash between emit and postpone only repeats an id the push-job ledger already
/// holds (UNIQUE signal_id, device): at-least-once, never a second alert. Returns signals emitted.
pub fn renew_pass(ledger: &Ledger, clock: &dyn Clock, dispatcher: &Dispatcher, family: &str, next_due: &dyn Fn(&Binding, i64) -> i64) -> usize {
    let now = clock.now();
    let due = match ledger.due(now) {
        Ok(d) => d,
        Err(e) => {
            tracing::error!("{family} renew pass: cannot read the ledger: {e}");
            return 0;
        }
    };
    let mut emitted = 0;
    for b in due {
        let due_at = b.due_at.unwrap_or(now);
        let signal = MailSignal { signal_id: format!("{family}-renew:{}:{due_at}", b.key_id), routing_id: b.routing_id.clone(), opaque_ref: Some(format!("watch-renew:{family}")) };
        if let Err(e) = dispatcher.emit(&signal) {
            tracing::error!("{family} renew pass: could not queue the renew signal: {e}");
            continue;
        }
        emitted += 1;
        if let Err(e) = ledger.postpone(&b.key_id, next_due(&b, now)) {
            tracing::error!("{family} renew pass: could not advance the due time: {e}");
        }
    }
    emitted
}

/// Starts the periodic job. Returns at once and needs no upstream answer.
pub fn spawn_renew_loop(ledger: Arc<Ledger>, clock: Arc<dyn Clock>, dispatcher: Arc<Dispatcher>, family: &'static str, tick: std::time::Duration, next_due: fn(&Binding, i64) -> i64) {
    tokio::spawn(async move {
        loop {
            renew_pass(&ledger, clock.as_ref(), &dispatcher, family, &next_due);
            tokio::time::sleep(tick).await;
        }
    });
}
