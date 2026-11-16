//! Calendar invites: the VEVENT of an iCalendar part (`icalendar`'s parser reads the format).
use icalendar::parser::{read_calendar, unfold};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EventTime {
    /// Milliseconds since the Unix epoch, known only for UTC times ("...Z").
    pub utc_ms: Option<i64>,
    /// The wall-clock time as written: `2026-11-12T14:00:00`, or `2026-11-12` for a whole day.
    pub local: Option<String>,
    /// The zone name of a zoned time, such as `Europe/Brussels`. The caller resolves it.
    pub tzid: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Invite {
    /// REQUEST, CANCEL, REPLY, PUBLISH... upper-cased; empty when the part has no METHOD.
    pub method: String,
    pub uid: String,
    pub summary: String,
    pub organizer: Option<String>,
    pub organizer_name: Option<String>,
    pub attendees: Vec<String>,
    pub location: Option<String>,
    pub description: Option<String>,
    pub all_day: bool,
    pub start: EventTime,
    pub end: Option<EventTime>,
}

fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let doy = (153 * (m + if m > 2 { -3 } else { 9 }) + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146097 + doe - 719468
}

fn time_of(value: &str, tzid: Option<&str>, date_only: bool) -> Option<EventTime> {
    let v = value.trim();
    let num = |a: usize, b: usize| v.get(a..b)?.parse::<i64>().ok();
    let (y, mo, d) = (num(0, 4)?, num(4, 6)?, num(6, 8)?);
    if date_only || v.len() == 8 {
        return Some(EventTime { utc_ms: None, local: Some(format!("{y:04}-{mo:02}-{d:02}")), tzid: None });
    }
    let (h, mi, s) = (num(9, 11)?, num(11, 13)?, num(13, 15).unwrap_or(0));
    let local = format!("{y:04}-{mo:02}-{d:02}T{h:02}:{mi:02}:{s:02}");
    if v.ends_with('Z') {
        let secs = days_from_civil(y, mo, d) * 86400 + h * 3600 + mi * 60 + s;
        return Some(EventTime { utc_ms: Some(secs * 1000), local: None, tzid: None });
    }
    Some(EventTime { utc_ms: None, local: Some(local), tzid: tzid.map(str::to_string) })
}

fn strip_mailto(v: &str) -> String {
    let t = v.trim();
    if t.len() >= 7 && t[..7].eq_ignore_ascii_case("mailto:") { t[7..].to_string() } else { t.to_string() }
}

/// Read the first VEVENT of an iCalendar text. None when the text holds no event.
pub fn parse_invite(ics: &str) -> Option<Invite> {
    let unfolded = unfold(ics);
    let cal = read_calendar(&unfolded).ok()?;
    let method = cal
        .properties
        .iter()
        .find(|p| p.name.as_str().eq_ignore_ascii_case("METHOD"))
        .map(|p| p.val.as_str().trim().to_ascii_uppercase())
        .unwrap_or_default();
    let ev = cal.components.iter().find(|c| c.name.as_str().eq_ignore_ascii_case("VEVENT"))?;
    let text = |name: &str| ev.find_prop(name).map(|p| p.val.clone().unescape_text().as_str().to_string());
    let param = |p: &icalendar::parser::Property<'_>, key: &str| {
        p.params.iter().find(|x| x.key.as_str().eq_ignore_ascii_case(key)).and_then(|x| x.val.as_ref().map(|v| v.as_str().trim_matches('"').to_string()))
    };
    let when = |name: &str| {
        let p = ev.find_prop(name)?;
        let date_only = param(p, "VALUE").is_some_and(|v| v.eq_ignore_ascii_case("DATE"));
        time_of(p.val.as_str(), param(p, "TZID").as_deref(), date_only).map(|t| (t, date_only || p.val.as_str().trim().len() == 8))
    };
    let (start, all_day) = when("DTSTART")?;
    let organizer = ev.find_prop("ORGANIZER");
    Some(Invite {
        method,
        uid: text("UID").unwrap_or_default(),
        summary: text("SUMMARY").unwrap_or_default(),
        organizer: organizer.map(|p| strip_mailto(p.val.as_str())),
        organizer_name: organizer.and_then(|p| param(p, "CN")),
        attendees: ev.properties.iter().filter(|p| p.name.as_str().eq_ignore_ascii_case("ATTENDEE")).map(|p| strip_mailto(p.val.as_str())).collect(),
        location: text("LOCATION"),
        description: text("DESCRIPTION"),
        all_day,
        start,
        end: when("DTEND").map(|(t, _)| t),
    })
}
