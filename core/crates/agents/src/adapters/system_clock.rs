use std::time::{SystemTime, UNIX_EPOCH};

use crate::ports::Clock;

pub struct SystemClock;

impl Clock for SystemClock {
    fn execute(&self) -> i64 {
        SystemTime::now().duration_since(UNIX_EPOCH).map(|d| i64::try_from(d.as_millis()).unwrap_or(i64::MAX)).unwrap_or(0)
    }
}
