//! Numbers and words read from docs/CONSTANTS.md at build time, so they live once.
use contracts::Clock;

const LEDGER: &str = include_str!("../../../../docs/CONSTANTS.md");

/// The Value cell of the ledger row named `name`.
pub fn cell(name: &str) -> Option<&'static str> {
    LEDGER
        .lines()
        .find(|l| l.trim_start().starts_with(&format!("| {name} |")) || l.trim_start().starts_with(&format!("| {name} ")))
        .and_then(|l| l.split('|').nth(2))
        .map(str::trim)
}

fn first_int(text: &str) -> Option<i64> {
    let digits: String = text.chars().skip_while(|c| !c.is_ascii_digit()).take_while(|c| c.is_ascii_digit()).collect();
    digits.parse().ok()
}

/// COMMIT_MS: the store commit of an interaction must finish inside it.
pub fn commit_ms() -> u64 {
    cell("COMMIT_MS").and_then(first_int).expect("docs/CONSTANTS.md has no COMMIT_MS row") as u64
}

/// UNDO_DEPTH: how many undo entries Z can walk back.
pub fn undo_depth() -> usize {
    cell("UNDO_DEPTH").and_then(first_int).expect("docs/CONSTANTS.md has no UNDO_DEPTH row") as usize
}

/// The folder words the sync lanes write for these two mailboxes (the inbox is store::INBOX_FOLDER).
pub const TRASH_FOLDER: &str = "Trash";
pub const SPAM_FOLDER: &str = "Spam";
/// System labels: star and mute and unsubscribe are labels, because the schema has no column for them.
pub const STARRED_LABEL: &str = "Starred";
pub const MUTED_LABEL: &str = "Muted";
pub const UNSUBSCRIBED_LABEL: &str = "Unsubscribed";

/// SNOOZE_LATER_TODAY · TOMORROW · NEXT_WEEK, parsed from its ledger cell: "+3 h rounded · 08:00 next day · Monday 08:00 local".
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SnoozeRule {
    pub later_hours: i64,
    pub tomorrow_minute: i64,
    pub week_day: i64,
    pub week_minute: i64,
}

const DAY_MS: i64 = 86_400_000;
const HOUR_MS: i64 = 3_600_000;
const MINUTE_MS: i64 = 60_000;

fn minute_of_day(part: &str) -> Option<i64> {
    let tok = part.split_whitespace().find(|t| t.contains(':'))?;
    let (h, m) = tok.split_once(':')?;
    Some(h.parse::<i64>().ok()? * 60 + m.parse::<i64>().ok()?)
}

impl SnoozeRule {
    pub fn from_ledger() -> SnoozeRule {
        Self::parse(cell("SNOOZE_LATER_TODAY").expect("docs/CONSTANTS.md has no SNOOZE_LATER_TODAY row")).expect("the SNOOZE_LATER_TODAY cell is not in the shape '+N h rounded · HH:MM next day · Weekday HH:MM local'")
    }

    pub fn parse(cell: &str) -> Option<SnoozeRule> {
        let parts: Vec<&str> = cell.split('·').collect();
        if parts.len() != 3 {
            return None;
        }
        let week_day = ["monday", "tuesday", "wednesday", "thursday", "friday", "saturday", "sunday"]
            .iter()
            .position(|d| parts[2].to_lowercase().contains(d))? as i64;
        Some(SnoozeRule { later_hours: first_int(parts[0])?, tomorrow_minute: minute_of_day(parts[1])?, week_day, week_minute: minute_of_day(parts[2])? })
    }

    /// "later today": now plus the hours, rounded up to the next full local hour. UTC milliseconds.
    pub fn later_today(&self, now_ms: i64, utc_offset_s: i32) -> i64 {
        let off = utc_offset_s as i64 * 1000;
        let local = now_ms + off + self.later_hours * HOUR_MS;
        let rounded = (local + HOUR_MS - 1).div_euclid(HOUR_MS) * HOUR_MS;
        rounded - off
    }

    /// "tomorrow morning": the next local day at the ledger time. UTC milliseconds.
    pub fn tomorrow_morning(&self, now_ms: i64, utc_offset_s: i32) -> i64 {
        let off = utc_offset_s as i64 * 1000;
        let day = (now_ms + off).div_euclid(DAY_MS);
        (day + 1) * DAY_MS + self.tomorrow_minute * MINUTE_MS - off
    }

    /// "next week": the coming ledger weekday (never today) at the ledger time. UTC milliseconds.
    pub fn next_week(&self, now_ms: i64, utc_offset_s: i32) -> i64 {
        let off = utc_offset_s as i64 * 1000;
        let day = (now_ms + off).div_euclid(DAY_MS);
        let weekday_today = (day + 3).rem_euclid(7); // 1970-01-01 was a Thursday; Monday = 0
        let mut ahead = (self.week_day - weekday_today).rem_euclid(7);
        if ahead == 0 {
            ahead = 7;
        }
        (day + ahead) * DAY_MS + self.week_minute * MINUTE_MS - off
    }

    pub fn resolve(&self, when: &contracts::SnoozeWhen, clock: &dyn Clock) -> i64 {
        let (now, off) = (clock.now_ms(), clock.utc_offset_s());
        match when {
            contracts::SnoozeWhen::LaterToday => self.later_today(now, off),
            contracts::SnoozeWhen::TomorrowMorning => self.tomorrow_morning(now, off),
            contracts::SnoozeWhen::NextWeek => self.next_week(now, off),
            contracts::SnoozeWhen::At { at_ms } => *at_ms,
        }
    }
}
