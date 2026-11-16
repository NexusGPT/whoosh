//! `CalendarStore` of reply-sources over the calendars the shell pushed (delta 21, `SyncReplyCalendars`: tables `reply_calendar` and `calendar_event`),
//! and the owner's offset from UTC, so an event's excerpt reads in the owner's own time.
use contracts::CoreError;
use reply_sources::{CalendarEvent, CalendarInfo, CalendarStore};
use store::Store;

pub(crate) struct StoreCalendars<'a> { pub store: &'a Store }

impl CalendarStore for StoreCalendars<'_> {
    fn calendars(&self) -> Result<Vec<CalendarInfo>, CoreError> {
        Ok(self.store.reply_calendars()?.into_iter().map(|c| CalendarInfo { calendar_id: c.calendar_id, name: c.name, account_label: c.account_label, colour: c.colour }).collect())
    }
    /// The contract's event has no all-day flag (a gap named in the note): every event reads with its times.
    fn events(&self, calendar_id: &str, from_ms: i64, to_ms: i64) -> Result<Vec<CalendarEvent>, CoreError> {
        Ok(self.store.reply_calendar_events()?.into_iter().filter(|e| e.calendar_id == calendar_id && e.end_ms > from_ms && e.start_ms < to_ms).map(|e| CalendarEvent {
            event_id: e.event_id, calendar_id: e.calendar_id, title: e.title, start_ms: e.start_ms, end_ms: e.end_ms, all_day: false, attendees: e.attendees, location: e.location,
        }).collect())
    }
}

/// The owner's offset from UTC at `at_ms`, in seconds (the Mac's time zone, summer time included). 0 when the system cannot say.
#[cfg(unix)]
pub(crate) fn utc_offset_s(at_ms: i64) -> i32 {
    // struct tm of macOS and glibc: nine ints, then tm_gmtoff (long) and tm_zone (char *)
    #[repr(C)]
    struct Tm { sec: i32, min: i32, hour: i32, mday: i32, mon: i32, year: i32, wday: i32, yday: i32, isdst: i32, gmtoff: std::os::raw::c_long, zone: *const std::os::raw::c_char }
    extern "C" { fn localtime_r(t: *const i64, out: *mut Tm) -> *mut Tm; }
    let t: i64 = at_ms / 1000;
    let mut tm = Tm { sec: 0, min: 0, hour: 0, mday: 0, mon: 0, year: 0, wday: 0, yday: 0, isdst: 0, gmtoff: 0, zone: std::ptr::null() };
    // SAFETY: both pointers are to live locals of the right layout; localtime_r writes `tm` only.
    let ok = unsafe { !localtime_r(&t, &mut tm).is_null() };
    if ok { tm.gmtoff as i32 } else { 0 }
}

#[cfg(not(unix))]
pub(crate) fn utc_offset_s(_at_ms: i64) -> i32 { 0 }
