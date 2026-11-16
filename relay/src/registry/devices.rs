//! Device registry: an APNs device token bound to an opaque account routing id. Nothing else.
use super::{Store, StoreError};
use rusqlite::params;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Environment {
    Sandbox,
    Production,
}

impl Environment {
    pub fn as_str(self) -> &'static str {
        match self {
            Environment::Sandbox => "sandbox",
            Environment::Production => "production",
        }
    }
}

pub struct DeviceRow {
    pub environment: String,
    pub registered_at: i64,
}

/// Idempotent. Returns true when the pair is new.
pub fn register(store: &Store, token: &str, routing_id: &str, env: Environment, now: i64) -> Result<bool, StoreError> {
    store.with(|c| {
        let existed: i64 = c.query_row(
            "SELECT COUNT(1) FROM registration WHERE device_token = ?1 AND routing_id = ?2",
            params![token, routing_id],
            |r| r.get(0),
        )?;
        c.execute(
            "INSERT INTO registration (device_token, routing_id, environment, registered_at) VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT (device_token, routing_id) DO UPDATE SET environment = excluded.environment, registered_at = excluded.registered_at",
            params![token, routing_id, env.as_str(), now],
        )?;
        Ok(existed == 0)
    })
}

/// Removes exactly one (device, account) pair. Returns true when it existed.
pub fn unregister(store: &Store, token: &str, routing_id: &str) -> Result<bool, StoreError> {
    store.with(|c| {
        let n = c.execute(
            "DELETE FROM registration WHERE device_token = ?1 AND routing_id = ?2",
            params![token, routing_id],
        )?;
        Ok(n > 0)
    })
}

/// Devices registered for ONE routing id. The device token itself is never returned.
pub fn lookup(store: &Store, routing_id: &str) -> Result<Vec<DeviceRow>, StoreError> {
    store.with(|c| {
        let mut st = c.prepare(
            "SELECT environment, registered_at FROM registration WHERE routing_id = ?1 ORDER BY registered_at, device_token",
        )?;
        let rows = st.query_map(params![routing_id], |r| {
            Ok(DeviceRow { environment: r.get(0)?, registered_at: r.get(1)? })
        })?;
        rows.collect()
    })
}

pub fn count(store: &Store) -> Result<i64, StoreError> {
    store.with(|c| c.query_row("SELECT COUNT(1) FROM registration", [], |r| r.get(0)))
}
