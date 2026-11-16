//! Time the crate sleeps on. "Now" comes from the contract's `Clock` (Swift owns time so tests can
//! move it); sleeping is separate so a test can record a wait instead of living through it.
use std::{future::Future, pin::Pin};

pub type BoxFut<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

pub trait Sleeper: Send + Sync {
    fn sleep_ms(&self, ms: u64) -> BoxFut<'_, ()>;
}

/// The production sleeper.
pub struct TokioSleeper;

impl Sleeper for TokioSleeper {
    fn sleep_ms(&self, ms: u64) -> BoxFut<'_, ()> {
        Box::pin(tokio::time::sleep(std::time::Duration::from_millis(ms)))
    }
}
