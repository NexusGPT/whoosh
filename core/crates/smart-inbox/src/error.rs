//! One error type. It keeps the real cause (the SQLite or store words), never a bare status.
use std::fmt;

#[derive(Debug)]
pub enum SmartInboxError {
    Store(store::StoreError),
    Sqlite(rusqlite::Error),
    /// A request the data cannot honour (removing the last enabled tab, an unknown tab id).
    Invalid(String),
}

impl fmt::Display for SmartInboxError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SmartInboxError::Store(e) => write!(f, "{e}"),
            SmartInboxError::Sqlite(e) => write!(f, "database error: {e}"),
            SmartInboxError::Invalid(why) => write!(f, "{why}"),
        }
    }
}
impl std::error::Error for SmartInboxError {}
impl From<store::StoreError> for SmartInboxError {
    fn from(e: store::StoreError) -> Self {
        SmartInboxError::Store(e)
    }
}
impl From<rusqlite::Error> for SmartInboxError {
    fn from(e: rusqlite::Error) -> Self {
        SmartInboxError::Sqlite(e)
    }
}
