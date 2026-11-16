//! How a connection is opened. SQLite keeps foreign keys per connection, so every opener sets it.
use crate::error::StoreError;
use rusqlite::{Connection, OpenFlags};
use std::path::Path;
use std::time::Duration;

/// Page cache per connection, in KiB (a negative cache_size is KiB). Engine tuning, not a product number.
const PAGE_CACHE_KIB: i64 = 65_536;
const BUSY_TIMEOUT: Duration = Duration::from_secs(5);

/// The one write connection: WAL, migrated to the latest schema, foreign keys on.
pub fn open_writer(path: &Path) -> Result<Connection, StoreError> {
    let mut conn = Connection::open(path)?;
    let mode: String = conn.pragma_update_and_check(None, "journal_mode", "WAL", |r| r.get(0))?;
    if !mode.eq_ignore_ascii_case("wal") {
        return Err(StoreError::Invalid(format!("the database would not switch to WAL (it answered {mode:?})")));
    }
    conn.pragma_update(None, "synchronous", "NORMAL")?;
    conn.pragma_update(None, "cache_size", -PAGE_CACHE_KIB)?;
    conn.busy_timeout(BUSY_TIMEOUT)?;
    crate::migrations::migrate(&mut conn)?; // also switches foreign_keys on for this connection
    Ok(conn)
}

/// A read connection: it cannot write, so a read path can never become a second writer.
pub fn open_reader(path: &Path) -> Result<Connection, StoreError> {
    let conn = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_URI)?;
    conn.pragma_update(None, "cache_size", -PAGE_CACHE_KIB)?;
    conn.pragma_update(None, "query_only", true)?;
    conn.pragma_update(None, "foreign_keys", true)?;
    conn.busy_timeout(BUSY_TIMEOUT)?;
    Ok(conn)
}
