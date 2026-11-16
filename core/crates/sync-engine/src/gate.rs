//! The first-screen gate: backfill waits until every account has its first screen stored, has failed,
//! or has been slowed by its provider. A failed or slowed account never holds the others.
use tokio::sync::watch;
use std::sync::Mutex;

pub struct Gate {
    arrived: Mutex<Vec<bool>>,
    remaining: watch::Sender<usize>,
}

impl Gate {
    pub fn new(accounts: usize) -> Gate {
        Gate { arrived: Mutex::new(vec![false; accounts]), remaining: watch::Sender::new(accounts) }
    }

    /// Account `index` no longer holds the gate. Idempotent.
    pub fn arrive(&self, index: usize) {
        let mut arrived = self.arrived.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(slot) = arrived.get_mut(index) {
            if !*slot {
                *slot = true;
                self.remaining.send_modify(|r| *r = r.saturating_sub(1));
            }
        }
    }

    pub async fn wait_all(&self) {
        let mut rx = self.remaining.subscribe();
        // The sender lives as long as the gate, so the wait cannot end with an error.
        let _ = rx.wait_for(|r| *r == 0).await;
    }
}

/// Arrives when dropped: an account task that ends any way at all (error, panic, abort) releases the gate.
pub struct Arrival<'a> {
    pub gate: &'a Gate,
    pub index: usize,
}

impl Drop for Arrival<'_> {
    fn drop(&mut self) {
        self.gate.arrive(self.index);
    }
}
