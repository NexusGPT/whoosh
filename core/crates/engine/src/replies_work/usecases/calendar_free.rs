//! gap-calendar (item 7604cdeb): what the writer is told about the owner's calendar beside the events themselves: the BUSY times (the events of the
//! calendars switched on, overlapping ones merged), the FREE times between them inside working hours, and the events WITH THE CORRESPONDENT first and marked.
//! Pure (std and the ledger's generated numbers only): the time comes in as an argument (`now_ms`), never read here; the owner's offset from UTC comes in as a function of the instant
//! (`off`, seconds), so summer time is followed. The contract's event carries instants only (no time zone, no all-day flag, no answer status): every time is
//! read in the owner's offset, an all-day event is recognised by its local midnights (`looks_all_day`), and `declined` is false until the contract carries it
//! (a finding in docs/lanes/gap-calendar.md). The text holds titles, times, attendees and places as the events carry them today; nothing new leaves this Mac.
#![allow(dead_code)]

/// Working hours, in minutes after local midnight: the ledger's REPLY_WORK_START_MIN and REPLY_WORK_END_MIN (docs/CONSTANTS.md).
pub const WORK_START_MIN: i64 = contracts::constants::REPLY_WORK_START_MIN as i64;
pub const WORK_END_MIN: i64 = contracts::constants::REPLY_WORK_END_MIN as i64;
/// A free stretch shorter than this is not offered: the ledger's REPLY_FREE_MIN_MIN.
pub const FREE_MIN_MIN: i64 = contracts::constants::REPLY_FREE_MIN_MIN as i64;
/// Working days are Monday to Friday (days of the week 0..=4, Monday = 0).
pub const LAST_WORK_WEEKDAY: i64 = 4;
/// The marker in front of an event the correspondent is part of.
pub const MARK_WITH_CORRESPONDENT: &str = "[with the correspondent] ";

const DAY_MS: i64 = 86_400_000;
const MIN_MS: i64 = 60_000;
const WEEKDAYS: [&str; 7] = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"];

/// One event, as the writer's calendar part reads it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ev { pub calendar_id: String, pub title: String, pub start_ms: i64, pub end_ms: i64, pub all_day: bool, pub declined: bool, pub attendees: Vec<String>, pub location: Option<String> }

/// [start, end) in epoch milliseconds.
pub type Span = (i64, i64);

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

/// The local day number (days since 1970-01-01) of an instant.
fn local_day(ms: i64, off: &dyn Fn(i64) -> i32) -> i64 { (ms + off(ms) as i64 * 1000).div_euclid(DAY_MS) }
/// The instant of local midnight at the start of local day `n` (the offset is read at that day's local noon, so a day with a clock change keeps one offset).
fn day_start(n: i64, off: &dyn Fn(i64) -> i32) -> i64 { n * DAY_MS - off(n * DAY_MS + DAY_MS / 2) as i64 * 1000 }
fn day_text(n: i64) -> String {
    let (y, m, d) = civil_from_days(n);
    format!("{y:04}-{m:02}-{d:02} ({})", WEEKDAYS[(n + 3).rem_euclid(7) as usize])
}
fn clock(min_in_day: i64) -> String { format!("{:02}:{:02}", min_in_day / 60, min_in_day % 60) }
fn time_text(ms: i64, off: &dyn Fn(i64) -> i32) -> String { clock((ms + off(ms) as i64 * 1000).rem_euclid(DAY_MS) / MIN_MS) }

/// An all-day event as the shell hands it over: it starts and ends on a local midnight.
pub fn looks_all_day(start_ms: i64, end_ms: i64, off_s: i32) -> bool {
    let o = off_s as i64 * 1000;
    end_ms > start_ms && (start_ms + o).rem_euclid(DAY_MS) == 0 && (end_ms + o).rem_euclid(DAY_MS) == 0
}

/// Only the events of the calendars switched on (`on` = their ids): a calendar switched off contributes nothing.
pub fn only_enabled(events: Vec<Ev>, on: &[String]) -> Vec<Ev> { events.into_iter().filter(|e| on.contains(&e.calendar_id)).collect() }

/// The busy intervals: every event that is not declined, an all-day one for its whole local day(s), overlapping or touching ones merged, by start.
pub fn busy(events: &[Ev], off: &dyn Fn(i64) -> i32) -> Vec<Span> {
    let mut spans: Vec<Span> = events.iter().filter(|e| !e.declined).filter_map(|e| {
        if e.all_day {
            let (first, last) = (local_day(e.start_ms, off), local_day((e.end_ms - 1).max(e.start_ms), off));
            Some((day_start(first, off), day_start(last + 1, off)))
        } else if e.end_ms > e.start_ms { Some((e.start_ms, e.end_ms)) } else { None }
    }).collect();
    spans.sort();
    let mut out: Vec<Span> = vec![];
    for (s, e) in spans {
        match out.last_mut() {
            Some(last) if s <= last.1 => { if e > last.1 { last.1 = e; } }
            _ => out.push((s, e)),
        }
    }
    out
}

/// The free stretches inside working hours on the working days from `now_ms` to `now_ms` + `days_ahead` days: what is left of each day's hours after the
/// busy intervals (`busy` sorted and merged, as `busy()` gives them), stretches under FREE_MIN_MIN left out.
pub fn free(busy: &[Span], now_ms: i64, days_ahead: i64, off: &dyn Fn(i64) -> i32) -> Vec<Span> {
    let horizon = now_ms + days_ahead * DAY_MS;
    let today = local_day(now_ms, off);
    let min = FREE_MIN_MIN * MIN_MS;
    let mut out: Vec<Span> = vec![];
    for n in today..=today + days_ahead {
        if (n + 3).rem_euclid(7) > LAST_WORK_WEEKDAY { continue; }
        let base = day_start(n, off);
        let hi = (base + WORK_END_MIN * MIN_MS).min(horizon);
        let mut at = (base + WORK_START_MIN * MIN_MS).max(now_ms);
        for &(s, e) in busy {
            if e <= at { continue; }
            if s >= hi { break; }
            if s > at && s - at >= min { out.push((at, s)); }
            at = at.max(e);
            if at >= hi { break; }
        }
        if at < hi && hi - at >= min { out.push((at, hi)); }
    }
    out
}

/// "2026-11-12 (Thu) 10:00-11:00, 14:00-15:00" per local day, an interval crossing midnight cut at it.
fn day_lines(spans: &[Span], off: &dyn Fn(i64) -> i32) -> String {
    let mut days: Vec<(i64, Vec<String>)> = vec![];
    for &(s, e) in spans {
        let mut n = local_day(s, off);
        loop {
            let (ds, de) = (day_start(n, off), day_start(n + 1, off));
            let (a, b) = (s.max(ds), e.min(de));
            if b > a {
                let part = format!("{}-{}", clock((a - ds) / MIN_MS), clock((b - ds) / MIN_MS));
                match days.last_mut() { Some((d, parts)) if *d == n => parts.push(part), _ => days.push((n, vec![part])) }
            }
            if e <= de { break; }
            n += 1;
        }
    }
    days.into_iter().map(|(n, parts)| format!("{} {}", day_text(n), parts.join(", "))).collect::<Vec<_>>().join("\n")
}

/// The two lists the writer reads beside the events: the busy times from now to the end of the window, then the free times in working hours.
pub fn free_busy_text(events: &[Ev], now_ms: i64, days_ahead: i64, off: &dyn Fn(i64) -> i32) -> String {
    let horizon = now_ms + days_ahead * DAY_MS;
    let all = busy(events, off);
    let ahead: Vec<Span> = all.iter().filter(|s| s.1 > now_ms && s.0 < horizon).map(|&(s, e)| (s.max(now_ms), e.min(horizon))).collect();
    let busy_text = if ahead.is_empty() { "Busy times: none in the coming days.".to_string() } else { format!("Busy times (my time):\n{}", day_lines(&ahead, off)) };
    let open = free(&all, now_ms, days_ahead, off);
    let hours = format!("{}-{}, Monday to Friday", clock(WORK_START_MIN), clock(WORK_END_MIN));
    let free_text = if open.is_empty() { format!("Free times: none in my working hours ({hours}) in the coming days.") } else { format!("Free times in my working hours ({hours}, my time):\n{}", day_lines(&open, off)) };
    format!("{busy_text}\n{free_text}")
}

/// The address inside "Name <a@b>" or "mailto:a@b", else the text; lower case, trimmed.
fn address_of(who: &str) -> String {
    let t = who.trim();
    let inner = match (t.rfind('<'), t.rfind('>')) { (Some(a), Some(b)) if a < b => &t[a + 1..b], _ => t };
    inner.trim().trim_start_matches("mailto:").trim().to_lowercase()
}

/// Whether the correspondent (an address) is an attendee or the organiser of the event, compared without case. Nobody (blank) is never "with".
pub fn is_with(event: &Ev, correspondent: &str) -> bool {
    let who = address_of(correspondent);
    !who.is_empty() && event.attendees.iter().any(|a| address_of(a) == who)
}

fn when(e: &Ev, off: &dyn Fn(i64) -> i32) -> String {
    if e.all_day {
        let (first, last) = (local_day(e.start_ms, off), local_day((e.end_ms - 1).max(e.start_ms), off));
        return if first == last { format!("{}, all day", day_text(first)) } else { format!("{} to {}, all day", day_text(first), day_text(last)) };
    }
    let (d0, d1) = (local_day(e.start_ms, off), local_day(e.end_ms, off));
    if d0 == d1 { format!("{} {} to {}", day_text(d0), time_text(e.start_ms, off), time_text(e.end_ms, off)) }
    else { format!("{} {} to {} {}", day_text(d0), time_text(e.start_ms, off), day_text(d1), time_text(e.end_ms, off)) }
}

/// One line per event (a declined event is left out): those with the correspondent first and marked, then the rest, each group by start.
pub fn event_lines(events: &[Ev], correspondent: &str, off: &dyn Fn(i64) -> i32) -> Vec<String> {
    let mut kept: Vec<(bool, &Ev)> = events.iter().filter(|e| !e.declined).map(|e| (is_with(e, correspondent), e)).collect();
    kept.sort_by_key(|(with, e)| (!*with, e.start_ms));
    kept.into_iter().map(|(with, e)| {
        let title = if e.title.trim().is_empty() { "(no title)" } else { e.title.trim() };
        let mut line = format!("{}{title} · {}", if with { MARK_WITH_CORRESPONDENT } else { "" }, when(e, off));
        let people: Vec<&str> = e.attendees.iter().map(|a| a.trim()).filter(|a| !a.is_empty()).collect();
        if !people.is_empty() { line.push_str(&format!(" · With: {}", people.join(", "))); }
        if let Some(place) = e.location.as_deref().map(str::trim).filter(|l| !l.is_empty()) { line.push_str(&format!(" · At: {place}")); }
        line
    }).collect()
}
