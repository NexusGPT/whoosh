//! The Whoosh SQLite schema and its runner.
//!
//! Migration v1 creates one table per docs/DATA-MAP.md record whose home is SQLite. The schema
//! version lives in `PRAGMA user_version`, so the runner adds no table of its own and the table
//! list stays exactly the data map's. A run on a database already at the latest version changes
//! nothing. A database from a newer build is refused, never downgraded.
use rusqlite::Connection;
use std::fmt;

/// Every migration in order: (version after it ran, its SQL). Append; never edit a shipped entry.
pub const MIGRATIONS: &[(i64, &str)] = &[
    (1, include_str!("../sql/0001_v1.sql")),
    (2, include_str!("../sql/0002_nested_labels.sql")),
    (3, include_str!("../sql/0003_account_setup_search.sql")),
    (4, include_str!("../sql/0004_labels_tabs_blocks.sql")),
    (5, include_str!("../sql/0005_views_unsubscribe_reminders.sql")),
    (6, include_str!("../sql/0006_labels_removed_by_user.sql")),
    (7, include_str!("../sql/0007_bcc.sql")),
    (8, include_str!("../sql/0008_mail_record_source.sql")),
    (9, include_str!("../sql/0009_label_flow.sql")),
    (10, include_str!("../sql/0010_mail_import.sql")),
    (11, include_str!("../sql/0011_pending_action.sql")),
    (12, include_str!("../sql/0012_junk.sql")),
    (13, include_str!("../sql/0013_ai_replies.sql")),
    (14, include_str!("../sql/0014_calendars_chunks_crawl.sql")),
    (15, include_str!("../sql/0015_outgoing_attachment.sql")),
    (16, include_str!("../sql/0016_attachment_columns_source_chunk.sql")),
    (17, include_str!("../sql/0017_decision_consent.sql")),
    (18, include_str!("../sql/0018_source_gone.sql")),
    (19, include_str!("../sql/0019_body_prepared.sql")),
    (20, include_str!("../sql/0020_source_item.sql")),
    (21, include_str!("../sql/0021_source_chunk_fts.sql")),
    (22, include_str!("../sql/0022_ai_label_no.sql")),
    (23, include_str!("../sql/0023_thread_headers.sql")),
    (27, include_str!("../sql/0027_reply_to.sql")),
    (28, include_str!("../sql/0028_settings_cmds.sql")),
    (29, include_str!("../sql/0029_smart_folder_label_parent.sql")),
    (30, include_str!("../sql/0030_thread_summary.sql")),
    (31, include_str!("../sql/0031_decided_model.sql")),
    (32, include_str!("../sql/0032_invitation_data_inline.sql")),
    (33, include_str!("../sql/0033_notch_alerts.sql")),
];

/// The reversal of each migration: (version it undoes, its SQL). `revert_to` runs these newest first.
/// v1 has none: undoing v1 means deleting the database file.
pub const REVERSALS: &[(i64, &str)] = &[
    (2, include_str!("../sql/0002_nested_labels.down.sql")),
    (3, include_str!("../sql/0003_account_setup_search.down.sql")),
    (4, include_str!("../sql/0004_labels_tabs_blocks.down.sql")),
    (5, include_str!("../sql/0005_views_unsubscribe_reminders.down.sql")),
    (6, include_str!("../sql/0006_labels_removed_by_user.down.sql")),
    (7, include_str!("../sql/0007_bcc.down.sql")),
    (8, include_str!("../sql/0008_mail_record_source.down.sql")),
    (9, include_str!("../sql/0009_label_flow.down.sql")),
    (10, include_str!("../sql/0010_mail_import.down.sql")),
    (11, include_str!("../sql/0011_pending_action.down.sql")),
    (12, include_str!("../sql/0012_junk.down.sql")),
    (13, include_str!("../sql/0013_ai_replies.down.sql")),
    (14, include_str!("../sql/0014_calendars_chunks_crawl.down.sql")),
    (15, include_str!("../sql/0015_outgoing_attachment.down.sql")),
    (16, include_str!("../sql/0016_attachment_columns_source_chunk.down.sql")),
    (17, include_str!("../sql/0017_decision_consent.down.sql")),
    (18, include_str!("../sql/0018_source_gone.down.sql")),
    (19, include_str!("../sql/0019_body_prepared.down.sql")),
    (20, include_str!("../sql/0020_source_item.down.sql")),
    (21, include_str!("../sql/0021_source_chunk_fts.down.sql")),
    (22, include_str!("../sql/0022_ai_label_no.down.sql")),
    (23, include_str!("../sql/0023_thread_headers.down.sql")),
    (27, include_str!("../sql/0027_reply_to.down.sql")),
    (28, include_str!("../sql/0028_settings_cmds.down.sql")),
    (29, include_str!("../sql/0029_smart_folder_label_parent.down.sql")),
    (30, include_str!("../sql/0030_thread_summary.down.sql")),
    (31, include_str!("../sql/0031_decided_model.down.sql")),
    (32, include_str!("../sql/0032_invitation_data_inline.down.sql")),
    (33, include_str!("../sql/0033_notch_alerts.down.sql")),
];

/// The schema version a database reaches after `migrate`.
pub fn latest_version() -> i64 {
    MIGRATIONS.last().map(|(v, _)| *v).unwrap_or(0)
}

#[derive(Debug)]
pub enum MigrationError {
    Sqlite(rusqlite::Error),
    /// The database was written by a newer build than this one.
    NewerSchema { found: i64, supported: i64 },
}

impl fmt::Display for MigrationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MigrationError::Sqlite(e) => write!(f, "database error during migration: {e}"),
            MigrationError::NewerSchema { found, supported } => write!(
                f,
                "this database is at schema version {found} but this build only understands up to {supported}; update Whoosh"
            ),
        }
    }
}

impl std::error::Error for MigrationError {}

impl From<rusqlite::Error> for MigrationError {
    fn from(e: rusqlite::Error) -> Self {
        MigrationError::Sqlite(e)
    }
}

/// What one `migrate` call did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Applied {
    pub from_version: i64,
    pub to_version: i64,
}

impl Applied {
    pub fn changed_anything(&self) -> bool {
        self.from_version != self.to_version
    }
}

pub fn schema_version(conn: &Connection) -> Result<i64, MigrationError> {
    Ok(conn.query_row("PRAGMA user_version", [], |r| r.get(0))?)
}

/// Bring `conn` to the latest schema. Foreign keys are switched on for this connection (SQLite
/// keeps that setting per connection, so the store must switch it on for every connection it opens).
/// All pending migrations run in one transaction: a failure leaves the database as it was.
pub fn migrate(conn: &mut Connection) -> Result<Applied, MigrationError> {
    conn.pragma_update(None, "foreign_keys", true)?;
    let found = schema_version(conn)?;
    let supported = latest_version();
    if found > supported {
        return Err(MigrationError::NewerSchema { found, supported });
    }
    if found == supported {
        return Ok(Applied { from_version: found, to_version: found });
    }
    let tx = conn.transaction()?;
    for (version, sql) in MIGRATIONS.iter().filter(|(v, _)| *v > found) {
        tx.execute_batch(sql)?;
        tx.pragma_update(None, "user_version", *version)?;
    }
    tx.commit()?;
    Ok(Applied { from_version: found, to_version: supported })
}

/// Undo migrations, newest first, until the database is at `target` (the reversal of the lowest
/// undone version leaves `target`). One transaction: a failure leaves the database as it was.
/// Only a deliberate, approved act calls this; `migrate` itself never goes backwards.
pub fn revert_to(conn: &mut Connection, target: i64) -> Result<Applied, MigrationError> {
    conn.pragma_update(None, "foreign_keys", true)?;
    let found = schema_version(conn)?;
    if found <= target {
        return Ok(Applied { from_version: found, to_version: found });
    }
    let tx = conn.transaction()?;
    for (version, sql) in REVERSALS.iter().rev().filter(|(v, _)| *v <= found && *v > target) {
        tx.execute_batch(sql)?;
        tx.pragma_update(None, "user_version", *version - 1)?;
    }
    tx.commit()?;
    schema_version(conn).map(|to| Applied { from_version: found, to_version: to })
}

/// Names of the user tables, sorted (internal `sqlite_*` tables left out).
pub fn table_names(conn: &Connection) -> Result<Vec<String>, MigrationError> {
    let mut stmt = conn.prepare("SELECT name FROM sqlite_master WHERE type = 'table' AND name NOT LIKE 'sqlite_%' ORDER BY name")?;
    let rows = stmt.query_map([], |r| r.get::<_, String>(0))?;
    Ok(rows.collect::<Result<Vec<_>, _>>()?)
}

/// The schema as SQLite stores it: every table, index and trigger statement, in name order.
pub fn schema_text(conn: &Connection) -> Result<String, MigrationError> {
    let mut stmt = conn.prepare("SELECT type, name, sql FROM sqlite_master WHERE name NOT LIKE 'sqlite_%' ORDER BY type, name")?;
    let rows = stmt.query_map([], |r| Ok(format!("{}|{}|{}", r.get::<_, String>(0)?, r.get::<_, String>(1)?, r.get::<_, Option<String>>(2)?.unwrap_or_default())))?;
    Ok(rows.collect::<Result<Vec<_>, _>>()?.join("\n"))
}

/// A short stable fingerprint of `schema_text` (FNV-1a, 64 bit): equal schema, equal hash.
pub fn schema_hash(conn: &Connection) -> Result<String, MigrationError> {
    let mut h: u64 = 0xcbf29ce484222325;
    for b in schema_text(conn)?.bytes() {
        h ^= u64::from(b);
        h = h.wrapping_mul(0x100000001b3);
    }
    Ok(format!("{h:016x}"))
}
