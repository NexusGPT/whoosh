//! Configuration. The undo window bounds are read from the ledger (docs/CONSTANTS.md) at build time, so the
//! numbers live once. The two figures the ledger does not hold yet (late threshold, standby takeover) are
//! required arguments, never defaults: see docs/lanes/core-outbox.md "Contract deltas".
use contracts::CoreError;

const LEDGER: &str = include_str!("../../../../docs/CONSTANTS.md");

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UndoBounds {
    pub default_s: u32,
    pub min_s: u32,
    pub max_s: u32,
}

/// The row `UNDO_SEND_DEFAULT_S · MIN · MAX | 10 · 5 · 30`.
pub fn undo_bounds() -> Result<UndoBounds, CoreError> {
    let bad = |why: &str| CoreError::Unexpected { reason_words: format!("docs/CONSTANTS.md row UNDO_SEND_DEFAULT_S is unreadable: {why}") };
    let line = LEDGER.lines().find(|l| l.trim_start().starts_with("| UNDO_SEND_DEFAULT_S")).ok_or_else(|| bad("the row is missing"))?;
    let cell = line.split('|').nth(2).ok_or_else(|| bad("no value cell"))?;
    let nums: Vec<u32> = cell.split('·').filter_map(|p| p.trim().parse().ok()).collect();
    match nums.as_slice() {
        [d, lo, hi] if lo <= d && d <= hi => Ok(UndoBounds { default_s: *d, min_s: *lo, max_s: *hi }),
        _ => Err(bad("expected three numbers: default, min, max")),
    }
}

/// Who sends a scheduled entry when its time comes.
/// `Primary` sends at the time. `Standby` sends only if the entry is still Scheduled in its own view
/// `after_ms` later: the device that is awake when the primary is not.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeviceRole {
    Primary,
    Standby { after_ms: i64 },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutboxConfig {
    /// Names this device inside entry ids, so two devices never mint the same id.
    pub device_id: String,
    pub role: DeviceRole,
    /// A sent entry is late when it left more than this many ms after its time.
    pub late_after_ms: i64,
    undo_window_s: u32,
}

impl OutboxConfig {
    pub fn new(device_id: &str, role: DeviceRole, late_after_ms: i64) -> Result<Self, CoreError> {
        Ok(Self { device_id: device_id.to_string(), role, late_after_ms, undo_window_s: undo_bounds()?.default_s })
    }

    pub fn undo_window_s(&self) -> u32 {
        self.undo_window_s
    }

    /// The Settings value: refused outside the ledger's bounds.
    pub fn with_undo_window_s(mut self, seconds: u32) -> Result<Self, CoreError> {
        self.undo_window_s = checked_window(seconds)?;
        Ok(self)
    }
}

pub(crate) fn checked_window(seconds: u32) -> Result<u32, CoreError> {
    let b = undo_bounds()?;
    if seconds < b.min_s || seconds > b.max_s {
        return Err(CoreError::Invalid { field: "undo_window_s".into(), reason_words: format!("the undo window is between {} and {} seconds", b.min_s, b.max_s) });
    }
    Ok(seconds)
}
