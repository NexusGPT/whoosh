use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    Index(String),
    Sqlite(String),
    Io(String),
    Store(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Index(m) => write!(f, "text index: {m}"),
            Error::Sqlite(m) => write!(f, "sqlite: {m}"),
            Error::Io(m) => write!(f, "io: {m}"),
            Error::Store(m) => write!(f, "store: {m}"),
        }
    }
}
impl std::error::Error for Error {}

impl From<tantivy::TantivyError> for Error {
    fn from(e: tantivy::TantivyError) -> Self {
        Error::Index(e.to_string())
    }
}
impl From<rusqlite::Error> for Error {
    fn from(e: rusqlite::Error) -> Self {
        Error::Sqlite(e.to_string())
    }
}
impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Self {
        Error::Io(e.to_string())
    }
}
impl From<store::StoreError> for Error {
    fn from(e: store::StoreError) -> Self {
        Error::Store(e.to_string())
    }
}
