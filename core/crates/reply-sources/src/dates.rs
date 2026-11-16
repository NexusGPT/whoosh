//! A date and a time of day from epoch milliseconds, in the owner's offset, with no time library. `civil_from_days` is Howard Hinnant's
//! algorithm (http://howardhinnant.github.io/date_algorithms.html, public domain), the same one core/crates/import/src/timeutil.rs uses.

const DAY_MS: i64 = 86_400_000;

fn civil_from_days(z: i64) -> (i64, i64, i64) {
    let z = z + 719_468;
    let era = (if z >= 0 { z } else { z - 146_096 }) / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    (if m <= 2 { y + 1 } else { y }, m, d)
}

/// "2026-11-12" of a local instant.
pub(crate) fn day(ms: i64, utc_offset_s: i32) -> String {
    let local = ms + utc_offset_s as i64 * 1000;
    let (y, m, d) = civil_from_days(local.div_euclid(DAY_MS));
    format!("{y:04}-{m:02}-{d:02}")
}

/// "14:05" of a local instant.
pub(crate) fn time(ms: i64, utc_offset_s: i32) -> String {
    let local = ms + utc_offset_s as i64 * 1000;
    let in_day = local.rem_euclid(DAY_MS) / 60_000;
    format!("{:02}:{:02}", in_day / 60, in_day % 60)
}

pub(crate) const fn days_ms(days: i64) -> i64 {
    days * DAY_MS
}
