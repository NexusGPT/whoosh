//! The secret value. Its `Debug` never prints the bytes, so a log line or a panic message
//! cannot carry it.
use std::fmt;

#[derive(Clone, PartialEq, Eq)]
pub struct Secret(Vec<u8>);

impl Secret {
    pub fn new(bytes: impl Into<Vec<u8>>) -> Self { Secret(bytes.into()) }
    pub fn expose(&self) -> &[u8] { &self.0 }
}

impl fmt::Debug for Secret {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Secret(<{} bytes>)", self.0.len())
    }
}
