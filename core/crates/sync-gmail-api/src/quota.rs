//! Sliding one-minute budget of Gmail quota units. Pure: the caller passes the time.

/// A request that would push the units spent in the last minute over `cap` waits until it fits.
#[derive(Debug)]
pub struct QuotaGate {
    cap: u64,
    window_ms: i64,
    spent: std::collections::VecDeque<(i64, u64)>,
}

impl QuotaGate {
    pub fn new(cap_units: u64, window_ms: i64) -> Self {
        QuotaGate { cap: cap_units, window_ms, spent: Default::default() }
    }

    fn evict(&mut self, now_ms: i64) {
        while let Some(&(at, _)) = self.spent.front() {
            if at <= now_ms - self.window_ms {
                self.spent.pop_front();
            } else {
                break;
            }
        }
    }

    /// Milliseconds to wait before `cost` units fit. 0 means go now. A cost above the cap can never fit and is clamped to the cap.
    pub fn wait_needed(&mut self, now_ms: i64, cost: u64) -> i64 {
        self.evict(now_ms);
        let cost = cost.min(self.cap);
        let mut total: u64 = self.spent.iter().map(|&(_, c)| c).sum();
        if total + cost <= self.cap {
            return 0;
        }
        for &(at, c) in self.spent.iter() {
            total -= c;
            if total + cost <= self.cap {
                return (at + self.window_ms - now_ms).max(1);
            }
        }
        1
    }

    pub fn record(&mut self, now_ms: i64, cost: u64) {
        self.spent.push_back((now_ms, cost.min(self.cap)));
    }
}
