//! One calendar brought up to date in the index: each event of the window (the last 7 days and the next 14) becomes one excerpt with
//! its title, start, end, attendees and location, so a draft can read when the owner is free. An event that left the window or was
//! deleted leaves the index; an unchanged one is not written again.
use crate::constants::{REPLY_CALENDAR_DAYS_AHEAD, REPLY_CALENDAR_DAYS_BEHIND};
use crate::dates::{day, days_ms, time};
use crate::excerpt::{excerpt_id, ItemStamp, SourceExcerpt};
use crate::ports::{CalendarEvent, CalendarStore, ExcerptSink, ProgressSink};
use crate::progress::ProgressTracker;
use crate::usecases::SyncOutcome;
use contracts::{ReplySourceRef, ReplySyncProgress};
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CalendarSyncReport {
    pub outcome: SyncOutcome,
    /// the events of the window, now in the index
    pub item_count: u64,
    pub written: u32,
    pub unchanged: u32,
    pub removed: u32,
    pub last: ReplySyncProgress,
}

/// [from, to) of the window around `now_ms`.
pub fn calendar_window(now_ms: i64) -> (i64, i64) {
    (now_ms - days_ms(REPLY_CALENDAR_DAYS_BEHIND), now_ms + days_ms(REPLY_CALENDAR_DAYS_AHEAD))
}

fn when(e: &CalendarEvent, off: i32) -> String {
    if e.all_day {
        // an all-day event ends at the next midnight: its last day is the one before
        let (first, last) = (day(e.start_ms, off), day((e.end_ms - 1).max(e.start_ms), off));
        return if first == last { format!("{first}, all day") } else { format!("{first} to {last}, all day") };
    }
    let (d0, d1) = (day(e.start_ms, off), day(e.end_ms, off));
    if d0 == d1 {
        format!("{d0} {} to {}", time(e.start_ms, off), time(e.end_ms, off))
    } else {
        format!("{d0} {} to {d1} {}", time(e.start_ms, off), time(e.end_ms, off))
    }
}

/// The one excerpt of an event: its title, then when (in the owner's offset `utc_offset_s`), then who, then where.
pub fn event_excerpt(source: &ReplySourceRef, calendar_name: &str, e: &CalendarEvent, utc_offset_s: i32) -> SourceExcerpt {
    let title = if e.title.trim().is_empty() { "(no title)".to_string() } else { e.title.trim().to_string() };
    let mut text = format!("{title}\n{}", when(e, utc_offset_s));
    let people: Vec<&str> = e.attendees.iter().map(|a| a.trim()).filter(|a| !a.is_empty()).collect();
    if !people.is_empty() {
        text.push_str(&format!("\nWith: {}", people.join(", ")));
    }
    if let Some(place) = e.location.as_deref().map(str::trim).filter(|l| !l.is_empty()) {
        text.push_str(&format!("\nAt: {place}"));
    }
    let item_key = format!("{}@{}", e.event_id, e.start_ms);
    SourceExcerpt { id: excerpt_id(source, &item_key, 0), source: source.clone(), item_key, ordinal: 0, title, folder: calendar_name.to_string(), date_ms: Some(e.start_ms), text }
}

/// FNV-1a of the excerpt's text and title: the same event read twice gives the same version.
fn version(x: &SourceExcerpt) -> String {
    let mut h: u64 = 0xcbf29ce484222325;
    for b in x.title.bytes().chain([0u8]).chain(x.text.bytes()) {
        h = (h ^ b as u64).wrapping_mul(0x100000001b3);
    }
    format!("{h:016x}")
}

/// Syncs the calendar `calendar_id` at `now_ms`. `keep_going` is asked before each event.
pub fn sync_calendar(store: &dyn CalendarStore, sink: &dyn ExcerptSink, progress: &dyn ProgressSink, calendar_id: &str, now_ms: i64, utc_offset_s: i32, keep_going: &dyn Fn() -> bool) -> CalendarSyncReport {
    let source = ReplySourceRef::Calendar { calendar_id: calendar_id.to_string() };
    let mut tracker = ProgressTracker::new(source.clone(), progress);
    let mut r = CalendarSyncReport { outcome: SyncOutcome::UpToDate, item_count: 0, written: 0, unchanged: 0, removed: 0, last: ReplySyncProgress::NotStarted };
    let fail = |tracker: &mut ProgressTracker, r: &mut CalendarSyncReport, words: String| {
        tracker.failed(r.item_count, words.clone());
        r.outcome = SyncOutcome::Failed { reason_words: words };
        r.last = tracker.state().clone();
    };
    let name = match store.calendars() {
        Ok(list) => match list.into_iter().find(|c| c.calendar_id == calendar_id) {
            Some(c) => c.name,
            None => {
                fail(&mut tracker, &mut r, "this calendar is no longer on this Mac".to_string());
                return r;
            }
        },
        Err(e) => {
            fail(&mut tracker, &mut r, format!("the calendars could not be read: {e}"));
            return r;
        }
    };
    let (from, to) = calendar_window(now_ms);
    let events = match store.events(calendar_id, from, to) {
        Ok(ev) => ev,
        Err(e) => {
            fail(&mut tracker, &mut r, format!("the events of {name} could not be read: {e}"));
            return r;
        }
    };
    let known: HashMap<String, ItemStamp> = match sink.stamps(&source) {
        Ok(s) => s.into_iter().map(|s| (s.item_key.clone(), s)).collect(),
        Err(e) => {
            fail(&mut tracker, &mut r, format!("the index of {name} could not be read: {e}"));
            return r;
        }
    };
    let excerpts: Vec<SourceExcerpt> = events.iter().filter(|e| e.end_ms > from && e.start_ms < to).map(|e| event_excerpt(&source, &name, e, utc_offset_s)).collect();
    let total = excerpts.len() as u64;
    tracker.advance(0, total, 0);
    for (i, x) in excerpts.iter().enumerate() {
        if !keep_going() {
            r.outcome = SyncOutcome::Stopped;
            r.last = tracker.state().clone();
            return r;
        }
        let v = version(x);
        if known.get(&x.item_key).is_some_and(|s| s.version == v) {
            r.unchanged += 1;
        } else {
            let stamp = ItemStamp { item_key: x.item_key.clone(), version: v, excerpt_count: 1 };
            if let Err(e) = sink.put_item(&source, &stamp, std::slice::from_ref(x)) {
                fail(&mut tracker, &mut r, format!("{} could not be added to the index: {e}", x.title));
                return r;
            }
            r.written += 1;
        }
        r.item_count += 1;
        tracker.advance(i as u64 + 1, total, r.item_count);
    }
    let now_in: HashSet<&str> = excerpts.iter().map(|x| x.item_key.as_str()).collect();
    let mut gone: Vec<&String> = known.keys().filter(|k| !now_in.contains(k.as_str())).collect();
    gone.sort();
    for key in gone {
        if let Err(e) = sink.remove_item(&source, key) {
            fail(&mut tracker, &mut r, format!("an old event could not be removed from the index: {e}"));
            return r;
        }
        r.removed += 1;
    }
    tracker.up_to_date(r.item_count);
    r.last = tracker.state().clone();
    r
}
