//! gap-calendar (item 7604cdeb, docs/lanes/gap-calendar.md): the busy and free times and the events with the correspondent, over FIXED events at a FIXED time
//! (Wednesday 2026-11-11 08:00, owner's offset +01:00: no clock is read). The module under test is the real file the writer's calendar part calls
//! (src/replies_work/usecases/calendar_free.rs), compiled in here by path because the usecases module is private to the crate.
//! Each test names its broken copy (the proof file checks/proofs/g3/7604cdeb.json holds them, one `find` once in its file):
//!   two_overlapping_events_*              busy(): `Some(last) if s <= last.1 =>` -> `Some(last) if false =>`   (two intervals, red)
//!   an_all_day_event_*                    busy(): `Some((day_start(first, off), day_start(last + 1, off)))` -> `Some((e.start_ms, e.end_ms))` (a flagged one is busy for its span only, red)
//!   a_declined_event_*                    busy(): `.filter(|e| !e.declined)` -> `.filter(|e| true)`            (the declined event is busy, red)
//!   no_event_gives_the_whole_working_day_free   free(): `let hi = (base + WORK_END_MIN * MIN_MS)` -> `let hi = (base + WORK_START_MIN * MIN_MS)` (no free time, red)
//!   an_event_with_the_correspondent_comes_first_*   event_lines(): `(!*with, e.start_ms)` -> `(false, e.start_ms)` (by date only, red)
//!   a_calendar_switched_off_*             only_enabled(): `on.contains(&e.calendar_id)` -> `true || on.contains(&e.calendar_id)` (its event is busy, red)
//!   the_computation_stays_under_50_ms_*   busy(): `spans.sort();` -> `spans.sort(); std::thread::sleep(std::time::Duration::from_millis(80));` (over 50 ms, red)
#[allow(dead_code, unused_imports)]
#[path = "../src/replies_work/usecases/calendar_free.rs"]
mod calendar_free;

use calendar_free::*;
use std::time::Instant;

const OFF_S: i32 = 3600;
const DAY: i64 = 86_400_000;
fn off(_ms: i64) -> i32 { OFF_S }

/// Days since 1970-01-01 of a civil date (Howard Hinnant's algorithm).
fn days(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = (if y >= 0 { y } else { y - 399 }) / 400;
    let yoe = y - era * 400;
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + d - 1;
    era * 146_097 + yoe * 365 + yoe / 4 - yoe / 100 + doy - 719_468
}
/// The instant of a local time (the owner's +01:00) on a day of November 2026.
fn at(day: i64, h: i64, m: i64) -> i64 { days(2026, 11, day) * DAY + (h * 60 + m) * 60_000 - OFF_S as i64 * 1000 }
fn now() -> i64 { at(11, 8, 0) }

fn ev(cal: &str, title: &str, start: i64, end: i64) -> Ev {
    Ev { calendar_id: cal.into(), title: title.into(), start_ms: start, end_ms: end, all_day: false, declined: false, attendees: vec![], location: None }
}
fn hm(ms: i64) -> String { let m = (ms + OFF_S as i64 * 1000).rem_euclid(DAY) / 60_000; format!("{:02}:{:02}", m / 60, m % 60) }

#[test]
fn two_overlapping_events_give_one_busy_interval() {
    let events = vec![ev("w", "Review", at(11, 10, 0), at(11, 11, 0)), ev("w", "Call", at(11, 10, 30), at(11, 12, 0))];
    let b = busy(&events, &off);
    println!("PASS two_overlapping_events_give_one_busy_interval: busy {:?}", b.iter().map(|s| (hm(s.0), hm(s.1))).collect::<Vec<_>>());
    assert_eq!(b, vec![(at(11, 10, 0), at(11, 12, 0))], "two overlapping events are one interval");
    let open = free(&b, now(), 14, &off);
    assert_eq!(&open[..2], &[(at(11, 9, 0), at(11, 10, 0)), (at(11, 12, 0), at(11, 18, 0))], "the free times of that day are around the one interval");
    let text = free_busy_text(&events, now(), 14, &off);
    assert!(text.contains("2026-11-11 (Wed) 10:00-12:00") && text.contains("2026-11-11 (Wed) 09:00-10:00, 12:00-18:00"), "the writer reads both lists:\n{text}");
}

#[test]
fn an_all_day_event_is_busy_for_its_whole_day() {
    // Thursday 12th, local midnight to the next local midnight, as the shell hands an all-day event over
    let (s, e) = (at(12, 0, 0), at(13, 0, 0));
    assert!(looks_all_day(s, e, OFF_S), "midnight to midnight is all day");
    assert!(!looks_all_day(at(12, 10, 0), at(12, 11, 0), OFF_S), "a meeting is not all day");
    let mut all = ev("w", "Holiday", s, e);
    all.all_day = true;
    let b = busy(std::slice::from_ref(&all), &off);
    println!("PASS an_all_day_event_is_busy_for_its_whole_day: busy {:?}", b.iter().map(|x| (hm(x.0), hm(x.1))).collect::<Vec<_>>());
    assert_eq!(b, vec![(s, e)]);
    // even when the shell gave it a short span, the flag makes the whole day busy
    let mut short = ev("w", "Holiday", at(12, 0, 0), at(12, 0, 1));
    short.all_day = true;
    assert_eq!(busy(&[short], &off), vec![(s, e)]);
    let open = free(&b, now(), 14, &off);
    assert!(open.iter().all(|f| f.1 <= s || f.0 >= e), "no free time on the all-day event's day: {open:?}");
    assert!(open.iter().any(|f| f.0 >= e), "the next working day is free");
    let lines = event_lines(std::slice::from_ref(&all), "", &off);
    assert_eq!(lines, vec!["Holiday · 2026-11-12 (Thu), all day".to_string()]);
}

#[test]
fn a_declined_event_is_not_busy_and_not_told_to_the_writer() {
    let mut declined = ev("w", "Declined lunch", at(11, 12, 0), at(11, 13, 0));
    declined.declined = true;
    assert!(busy(std::slice::from_ref(&declined), &off).is_empty());
    assert!(event_lines(std::slice::from_ref(&declined), "", &off).is_empty());
    println!("PASS a_declined_event_is_not_busy_and_not_told_to_the_writer");
}

#[test]
fn no_event_gives_the_whole_working_day_free() {
    let b = busy(&[], &off);
    assert!(b.is_empty());
    let open = free(&b, now(), 14, &off);
    println!("PASS no_event_gives_the_whole_working_day_free: first free {} to {}, {} stretches", hm(open[0].0), hm(open[0].1), open.len());
    assert_eq!(open[0], (at(11, 9, 0), at(11, 18, 0)), "Wednesday 09:00 to 18:00");
    assert_eq!(open[1], (at(12, 9, 0), at(12, 18, 0)), "Thursday");
    assert_eq!(open[2], (at(13, 9, 0), at(13, 18, 0)), "Friday");
    assert_eq!(open[3], (at(16, 9, 0), at(16, 18, 0)), "the weekend (14th, 15th) is skipped: Monday 16th follows");
    assert!(open.iter().all(|f| f.1 <= now() + 14 * DAY), "nothing past the window");
    let text = free_busy_text(&[], now(), 14, &off);
    assert!(text.contains("Busy times: none") && text.contains("2026-11-11 (Wed) 09:00-18:00"), "{text}");
    // later in the day only what is left is free
    let late = free(&b, at(11, 15, 30), 14, &off);
    assert_eq!(late[0], (at(11, 15, 30), at(11, 18, 0)));
}

#[test]
fn an_event_with_the_correspondent_comes_first_and_is_marked() {
    let mut early = ev("w", "Team sync", at(12, 9, 0), at(12, 10, 0));
    early.attendees = vec!["Someone Else <else@example.test>".into()];
    let mut mine = ev("w", "Lunch with Dana", at(13, 12, 0), at(13, 13, 0));
    mine.attendees = vec!["Dana <DANA@Example.TEST>".into()];
    let mut org = ev("w", "Planning", at(16, 9, 0), at(16, 10, 0));
    org.attendees = vec!["mailto:dana@example.test".into()];
    let lines = event_lines(&[early, mine, org], "dana@example.test", &off);
    println!("PASS an_event_with_the_correspondent_comes_first_and_is_marked: {lines:?}");
    assert!(lines[0].starts_with(MARK_WITH_CORRESPONDENT) && lines[0].contains("Lunch with Dana"), "{lines:?}");
    assert!(lines[1].starts_with(MARK_WITH_CORRESPONDENT) && lines[1].contains("Planning"), "both events with Dana come first, by date: {lines:?}");
    assert!(!lines[2].contains(MARK_WITH_CORRESPONDENT) && lines[2].contains("Team sync"), "{lines:?}");
    assert!(event_lines(&[ev("w", "Solo", at(12, 9, 0), at(12, 10, 0))], "", &off)[0] == "Solo · 2026-11-12 (Thu) 09:00 to 10:00", "no correspondent marks nothing");
}

#[test]
fn a_calendar_switched_off_contributes_nothing() {
    let events = vec![ev("on", "Kept", at(11, 10, 0), at(11, 11, 0)), ev("off", "Dentist", at(11, 14, 0), at(11, 15, 0))];
    let kept = only_enabled(events, &["on".to_string()]);
    println!("PASS a_calendar_switched_off_contributes_nothing: {} event kept", kept.len());
    assert_eq!(kept.len(), 1);
    let text = free_busy_text(&kept, now(), 14, &off);
    assert!(!text.contains("14:00-15:00") && text.contains("10:00-11:00"), "{text}");
    assert!(event_lines(&kept, "", &off).iter().all(|l| !l.contains("Dentist")));
    assert!(only_enabled(kept, &[]).is_empty(), "no calendar on: nothing");
}

#[test]
fn the_computation_stays_under_50_ms_on_500_events() {
    // 500 events spread over the window, a third with attendees, some overlapping, a few all-day
    let events: Vec<Ev> = (0..500).map(|i: i64| {
        let start = now() - 7 * DAY + (i * 37 * 60_000 * 7) % (21 * DAY);
        let mut e = ev(if i % 5 == 0 { "off" } else { "on" }, &format!("Event {i}"), start, start + (30 + i % 90) * 60_000);
        if i % 3 == 0 { e.attendees = vec![format!("p{i}@example.test"), "dana@example.test".into()]; }
        if i % 50 == 0 { e.all_day = true; }
        e
    }).collect();
    let t = Instant::now();
    let on = only_enabled(events, &["on".to_string()]);
    let lines = event_lines(&on, "dana@example.test", &off);
    let text = free_busy_text(&on, now(), 14, &off);
    let took = t.elapsed();
    println!("PASS the_computation_stays_under_50_ms_on_500_events: {} events, {} lines, {} chars of times, {:?}", on.len(), lines.len(), text.len(), took);
    assert!(took.as_millis() < 50, "took {took:?}");
    assert!(lines[0].starts_with(MARK_WITH_CORRESPONDENT));
}
