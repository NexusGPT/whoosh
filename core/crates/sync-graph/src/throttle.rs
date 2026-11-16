//! The per-mailbox limiter: at most `concurrent` requests in flight and at most `per_window`
//! request starts in any sliding window, plus a pause every request honours after a 429.
//! One `Throttle` per mailbox; Microsoft counts these limits per app and mailbox.
use std::{
    collections::VecDeque,
    sync::{Arc, Mutex, MutexGuard},
};

use contracts::Clock;
use tokio::sync::{OwnedSemaphorePermit, Semaphore};

use crate::{limits, time::Sleeper};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Limits {
    pub concurrent: usize,
    pub per_window: usize,
    pub window_ms: i64,
}

impl Limits {
    /// The documented limits, from docs/CONSTANTS.md.
    pub fn documented() -> Self {
        Limits { concurrent: limits::GRAPH_CONCURRENT, per_window: limits::GRAPH_REQ_PER_10MIN, window_ms: limits::GRAPH_WINDOW_MS }
    }
}

struct State {
    started: VecDeque<i64>,
    paused_until_ms: i64,
}

pub struct Throttle {
    permits: Arc<Semaphore>,
    limits: Limits,
    state: Mutex<State>,
    clock: Arc<dyn Clock>,
    sleeper: Arc<dyn Sleeper>,
}

impl Throttle {
    pub fn new(clock: Arc<dyn Clock>, sleeper: Arc<dyn Sleeper>) -> Self {
        Self::with_limits(Limits::documented(), clock, sleeper)
    }

    /// Smaller limits let a test see the sliding window without 10,000 requests.
    #[doc(hidden)]
    pub fn with_limits(limits: Limits, clock: Arc<dyn Clock>, sleeper: Arc<dyn Sleeper>) -> Self {
        Throttle {
            permits: Arc::new(Semaphore::new(limits.concurrent)),
            limits,
            state: Mutex::new(State { started: VecDeque::new(), paused_until_ms: 0 }),
            clock,
            sleeper,
        }
    }

    pub fn limits(&self) -> Limits {
        self.limits
    }

    fn state(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Every request of this mailbox waits until `now + wait_ms` (Retry-After after a 429).
    pub fn pause_for(&self, wait_ms: u64) {
        let until = self.clock.now_ms() + wait_ms as i64;
        let mut st = self.state();
        st.paused_until_ms = st.paused_until_ms.max(until);
    }

    /// Wait for a concurrency slot, then for the window and any pause; the permit is held for the
    /// whole request and released when dropped.
    pub async fn acquire(&self) -> OwnedSemaphorePermit {
        let permit = self.permits.clone().acquire_owned().await.expect("the semaphore is never closed");
        loop {
            let wait = {
                let mut st = self.state();
                let now = self.clock.now_ms();
                while st.started.front().is_some_and(|t| now - *t >= self.limits.window_ms) {
                    st.started.pop_front();
                }
                let paused = (st.paused_until_ms - now).max(0);
                let full = if st.started.len() >= self.limits.per_window {
                    st.started.front().map_or(0, |t| (t + self.limits.window_ms - now).max(0))
                } else {
                    0
                };
                let wait = paused.max(full);
                if wait == 0 {
                    st.started.push_back(now);
                    return permit;
                }
                wait
            };
            self.sleeper.sleep_ms(wait as u64).await;
        }
    }
}
