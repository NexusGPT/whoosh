//! The words "later today", "tomorrow morning", "next week" become return times with the SNOOZE row of docs/CONSTANTS.md.
use actions::constants::SnoozeRule;

const H: i64 = 3_600_000;
const D: i64 = 24 * H;
// 2023-11-14 (a Tuesday) 10:30 UTC.
const TUE_1030: i64 = 1_699_957_800_000;

#[test]
fn the_ledger_cell_parses() {
    let rule = SnoozeRule::from_ledger();
    assert_eq!(rule, SnoozeRule { later_hours: 3, tomorrow_minute: 8 * 60, week_day: 0, week_minute: 8 * 60 });
}

#[test]
fn later_today_is_three_hours_rounded_up_to_the_hour_in_local_time() {
    let r = SnoozeRule::from_ledger();
    assert_eq!(r.later_today(TUE_1030, 0), TUE_1030 - 30 * 60_000 + 4 * H, "10:30 + 3 h = 13:30, rounded up to 14:00");
    assert_eq!(r.later_today(TUE_1030 - 30 * 60_000, 0), TUE_1030 - 30 * 60_000 + 3 * H, "10:00 + 3 h = 13:00 exactly");
    // In a zone at +05:30 the local hour boundary is 30 minutes off the UTC one.
    let off = 5 * 3600 + 1800;
    assert_eq!((r.later_today(TUE_1030, off) + off as i64 * 1000) % H, 0, "rounded on the LOCAL hour");
}

#[test]
fn tomorrow_morning_is_eight_local_the_next_day() {
    let r = SnoozeRule::from_ledger();
    let midnight = TUE_1030 - 10 * H - 30 * 60_000;
    assert_eq!(r.tomorrow_morning(TUE_1030, 0), midnight + D + 8 * H);
    // 23:30 local on Tuesday in a +1 h zone (22:30 UTC): tomorrow is Wednesday, 08:00 local = 07:00 UTC.
    let late_utc = midnight + 22 * H + 30 * 60_000;
    assert_eq!(r.tomorrow_morning(late_utc, 3600), midnight + D + 7 * H);
}

#[test]
fn next_week_is_the_coming_monday_even_on_a_monday() {
    let r = SnoozeRule::from_ledger();
    let tue = TUE_1030;
    let next_monday_0800 = tue - 10 * H - 30 * 60_000 + 6 * D + 8 * H;
    assert_eq!(r.next_week(tue, 0), next_monday_0800);
    let monday_noon = next_monday_0800 - 8 * H + 12 * H;
    assert_eq!(r.next_week(monday_noon, 0), next_monday_0800 + 7 * D, "on a Monday, next week is the Monday after");
}
