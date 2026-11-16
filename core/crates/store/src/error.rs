//! One error type for the crate. The message keeps the real cause (the SQLite words), never a bare status.
use std::fmt;

#[derive(Debug)]
pub enum StoreError {
    Sqlite(rusqlite::Error),
    Migration(crate::migrations::MigrationError),
    /// The writer thread has stopped (the store was dropped, or a write job panicked).
    WriterGone,
    /// A write job panicked; the transaction was dropped, so nothing it did is stored.
    JobPanicked,
    Invalid(String),
}

impl fmt::Display for StoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            StoreError::Sqlite(e) => write!(f, "database error: {e}"),
            StoreError::Migration(e) => write!(f, "{e}"),
            StoreError::WriterGone => write!(f, "the store's writer has stopped"),
            StoreError::JobPanicked => write!(f, "a store write failed unexpectedly and was undone"),
            StoreError::Invalid(why) => write!(f, "{why}"),
        }
    }
}

impl std::error::Error for StoreError {}

impl From<rusqlite::Error> for StoreError {
    fn from(e: rusqlite::Error) -> Self {
        StoreError::Sqlite(e)
    }
}

impl From<crate::migrations::MigrationError> for StoreError {
    fn from(e: crate::migrations::MigrationError) -> Self {
        StoreError::Migration(e)
    }
}

impl From<StoreError> for contracts::CoreError {
    fn from(e: StoreError) -> Self {
        contracts::CoreError::Unexpected { reason_words: e.to_string() }
    }
}
