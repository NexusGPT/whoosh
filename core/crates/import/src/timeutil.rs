//! Calendar arithmetic in UTC, with no clock and no time library. The civil-date algorithms are Howard Hinnant's
//! `days_from_civil` and `civil_from_days` (http://howardhinnant.github.io/date_algorithms.html, public domain).

const DAY_MS: i64 = 86_400_000;
const MONTHS: [&str; 12] = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];
const WEEKDAYS: [&str; 7] = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];

pub fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = (if y >= 0 { y } else { y - 399 }) / 400;
    let yoe = y - era * 400;
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

pub fn civil_from_days(z: i64) -> (i64, i64, i64) {
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

pub fn ms_from_civil(y: i64, mo: i64, d: i64, h: i64, mi: i64, s: i64) -> Option<i64> {
    if !(1..=12).contains(&mo) || !(1..=31).contains(&d) || !(0..24).contains(&h) || !(0..60).contains(&mi) || !(0..=60).contains(&s) {
        return None;
    }
    Some(days_from_civil(y, mo, d) * DAY_MS + ((h * 60 + mi) * 60 + s) * 1000)
}

/// "2024-01-02T10:11:12", with an optional fraction and a trailing Z. Outlook for Mac writes no zone; it is read as UTC.
pub fn parse_iso_naive(s: &str) -> Option<i64> {
    let s = s.trim().trim_end_matches('Z');
    let (date, time) = s.split_once('T').or_else(|| s.split_once(' '))?;
    let mut d = date.split('-');
    let (y, mo, da) = (d.next()?.parse().ok()?, d.next()?.parse().ok()?, d.next()?.parse().ok()?);
    let time = time.split(['.', '+']).next()?;
    let mut t = time.split(':');
    let (h, mi) = (t.next()?.parse().ok()?, t.next()?.parse().ok()?);
    let sec = t.next().map_or(Some(0), |x| x.parse().ok())?;
    ms_from_civil(y, mo, da, h, mi, sec)
}

/// An mbox From_ line's date, "Mon Jan  2 10:11:12 2024" (a zone word may sit before the year), read as UTC.
pub fn parse_asctime(s: &str) -> Option<i64> {
    let toks: Vec<&str> = s.split_whitespace().collect();
    let mon_at = toks.iter().position(|t| MONTHS.iter().any(|m| m.eq_ignore_ascii_case(t)))?;
    let mo = MONTHS.iter().position(|m| m.eq_ignore_ascii_case(toks[mon_at]))? as i64 + 1;
    let day: i64 = toks.get(mon_at + 1)?.parse().ok()?;
    let mut t = toks.get(mon_at + 2)?.split(':');
    let (h, mi) = (t.next()?.parse().ok()?, t.next()?.parse().ok()?);
    let sec = t.next().map_or(Some(0), |x| x.parse().ok())?;
    let year: i64 = toks.iter().skip(mon_at + 3).find(|t| t.len() == 4 && t.bytes().all(|b| b.is_ascii_digit()))?.parse().ok()?;
    ms_from_civil(year, mo, day, h, mi, sec)
}

/// "Tue, 02 Jan 2024 10:11:12 +0000"
pub fn rfc2822(ms: i64) -> String {
    let days = ms.div_euclid(DAY_MS);
    let secs = ms.rem_euclid(DAY_MS) / 1000;
    let (y, m, d) = civil_from_days(days);
    let wd = (days + 4).rem_euclid(7) as usize;
    format!("{}, {:02} {} {} {:02}:{:02}:{:02} +0000", WEEKDAYS[wd], d, MONTHS[(m - 1) as usize], y, secs / 3600, secs % 3600 / 60, secs % 60)
}
