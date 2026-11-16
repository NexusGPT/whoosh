//! Delta 21: what the AI replies need from the shell's calendars and from the background crawl (docs/lanes/settingscalendars-mac.md "The seam to
//! the core"; docs/lanes/sync-full-r1.md findings 4 and 5 and "Contract fields"). Types, pure rules and the state JSON in one file; the delta 20
//! conventions hold (English words with a code, `CoreError::Invalid { field }`, no sentence of the owner's language, reads are state paths).
//!   STATE PATHS  reply/calendars/enabled   ["<calendar id>", ...]        what the shell reads before it sends events
//!                reply/calendars/events    [CalendarEvent]               the events the core kept
//!                reply/progress            [FolderProgress + progress]   per account and folder: messages stored, bodies stored, bodies indexed
//!                reply/chunk/<chunk id>    Chunk                         one chunk, by its own id
use crate::*;

pub const CALENDAR_SYNC_REFUSAL_CODE: &str = "bad_calendar_sync";
pub const FOLDER_PROGRESS_REFUSAL_CODE: &str = "bad_folder_progress";
pub const CHUNK_REFUSAL_CODE: &str = "bad_chunk";
fn refuse(code: &str, words: impl Into<String>) -> CoreError { CoreError::Invalid { field: code.into(), reason_words: words.into() } }
fn blank(s: &str) -> bool { s.trim().is_empty() }

// ---------------------------------------------------------------- calendars pushed by the shell

/// DATA-MAP `reply-calendar`: a calendar the shell found on this Mac. The `enabled` switch is NOT here: it lives in `ReplySource` (a `Calendar` source),
/// so the shell cannot switch a calendar on by sending it. `colour` is "#RRGGBB".
#[derive(uniffi::Record, Debug, Clone, PartialEq, Eq)]
pub struct ReplyCalendarInfo { pub calendar_id: String, pub name: String, pub account_label: String, pub colour: String }

/// DATA-MAP `calendar-event`: one event of an enabled calendar, as the shell read it. `location` None = none. Read-only for replies: the shell never writes one.
#[derive(uniffi::Record, Debug, Clone, PartialEq, Eq)]
pub struct CalendarEvent { pub event_id: String, pub calendar_id: String, pub title: String, pub start_ms: i64, pub end_ms: i64, pub attendees: Vec<String>, pub location: Option<String> }

/// What `SyncReplyCalendars` leaves: the calendars the core now holds (with their switch), the events it kept, the calendars that went away, and how many
/// events it did not keep (a calendar that is off or unknown, an event outside the window).
#[derive(uniffi::Record, Debug, Clone, PartialEq, Eq)]
pub struct CalendarSyncResult { pub calendars: Vec<ReplyCalendar>, pub events: Vec<CalendarEvent>, pub dropped_calendar_ids: Vec<String>, pub ignored_events: u32 }

fn colour_ok(c: &str) -> bool { c.len() == 7 && c.starts_with('#') && c[1..].chars().all(|h| h.is_ascii_hexdigit()) }

/// The one rule of `SyncReplyCalendars`. A calendar the core knows keeps its `enabled`; a new one starts OFF; one the shell no longer lists is dropped (its
/// switch with it). An event is kept only when its calendar is listed AND enabled AND it overlaps the window; a second event with the same (calendar, id)
/// replaces the first. Refused in words (`bad_calendar_sync`): a calendar with no id, name or "#RRGGBB" colour, an empty or inverted window, an event with no
/// id or one that ends before it starts. `known` = the calendars the core holds now, `enabled` read from their sources.
#[uniffi::export]
pub fn apply_calendar_sync(known: Vec<ReplyCalendar>, calendars: Vec<ReplyCalendarInfo>, window_start_ms: i64, window_end_ms: i64, events: Vec<CalendarEvent>) -> Result<CalendarSyncResult, CoreError> {
    if window_end_ms <= window_start_ms { return Err(refuse(CALENDAR_SYNC_REFUSAL_CODE, "the window has to end after it starts")); }
    let mut held: Vec<ReplyCalendar> = vec![];
    for c in &calendars {
        if blank(&c.calendar_id) { return Err(refuse(CALENDAR_SYNC_REFUSAL_CODE, "a calendar has no id")); }
        if blank(&c.name) { return Err(refuse(CALENDAR_SYNC_REFUSAL_CODE, "a calendar has no name")); }
        if !colour_ok(&c.colour) { return Err(refuse(CALENDAR_SYNC_REFUSAL_CODE, "a calendar colour looks like #RRGGBB")); }
        let enabled = known.iter().find(|k| k.calendar_id == c.calendar_id).map(|k| k.enabled).unwrap_or(false);
        let one = ReplyCalendar { calendar_id: c.calendar_id.clone(), name: c.name.clone(), account_label: c.account_label.clone(), colour: c.colour.clone(), enabled };
        match held.iter().position(|h| h.calendar_id == one.calendar_id) { Some(i) => held[i] = one, None => held.push(one) }
    }
    let dropped: Vec<String> = known.iter().filter(|k| !held.iter().any(|h| h.calendar_id == k.calendar_id)).map(|k| k.calendar_id.clone()).collect();
    let mut kept: Vec<CalendarEvent> = vec![];
    let mut ignored = 0u32;
    for e in events {
        if blank(&e.event_id) { return Err(refuse(CALENDAR_SYNC_REFUSAL_CODE, "an event has no id")); }
        if e.end_ms < e.start_ms { return Err(refuse(CALENDAR_SYNC_REFUSAL_CODE, "an event ends before it starts")); }
        let on = held.iter().any(|h| h.calendar_id == e.calendar_id && h.enabled);
        if !on || !(e.start_ms < window_end_ms && e.end_ms >= window_start_ms) { ignored += 1; continue; }
        match kept.iter().position(|k| k.calendar_id == e.calendar_id && k.event_id == e.event_id) { Some(i) => kept[i] = e, None => kept.push(e) }
    }
    Ok(CalendarSyncResult { calendars: held, events: kept, dropped_calendar_ids: dropped, ignored_events: ignored })
}

/// The ids of the enabled calendars: what the shell reads before it sends events (events of a calendar that is off never leave the shell's process).
#[uniffi::export]
pub fn enabled_calendar_ids(calendars: Vec<ReplyCalendar>) -> Vec<String> { calendars.into_iter().filter(|c| c.enabled).map(|c| c.calendar_id).collect() }

// ---------------------------------------------------------------- the chunk

/// Which part of a message a chunk was cut from.
#[derive(uniffi::Enum, Debug, Clone, PartialEq, Eq, Hash)]
pub enum ChunkPart { Body, Attachment { attachment_id: String } }

/// DATA-MAP `chunk`: one piece of a message part, with the facts retrieval shows beside it. Its `id` is its OWN key (AUTOINCREMENT, never reused, so a deleted
/// message's vectors can never point at another message): the meaning index files the vector under `chunk_vector_key(id)`, never under the message's rowid.
/// `embedded_at_ms` None = no vector yet; Some = in the meaning index ("indexed" is exact and survives a launch without the model). Written by the indexer.
#[derive(uniffi::Record, Debug, Clone, PartialEq, Eq)]
pub struct Chunk {
    pub id: i64, pub message_id: String, pub part: ChunkPart, pub chunk_no: u32, pub received_at_ms: i64,
    pub sender: String, pub folder: String, pub subject: String, pub text: String, pub embedded_at_ms: Option<i64>,
}

/// `body`, or `attachment:<id>`: the stored spelling of a part.
#[uniffi::export]
pub fn chunk_part_key(part: ChunkPart) -> String { match part { ChunkPart::Body => "body".into(), ChunkPart::Attachment { attachment_id } => format!("attachment:{attachment_id}") } }
#[uniffi::export]
pub fn chunk_part_from_key(key: String) -> Option<ChunkPart> {
    if key == "body" { return Some(ChunkPart::Body); }
    key.strip_prefix("attachment:").filter(|id| !id.is_empty()).map(|id| ChunkPart::Attachment { attachment_id: id.to_string() })
}
/// The key a chunk's vector is filed under in the meaning index: the chunk's own id.
#[uniffi::export]
pub fn chunk_vector_key(chunk_id: i64) -> u64 { chunk_id.max(0) as u64 }
/// `ReplyExcerpt.chunk_id` and `ReplyDraft.excerpt_ids` hold a chunk id as decimal words.
#[uniffi::export]
pub fn chunk_excerpt_id(chunk_id: i64) -> String { chunk_id.to_string() }
#[uniffi::export]
pub fn chunk_id_from_excerpt_id(excerpt_id: String) -> Option<i64> { excerpt_id.parse::<i64>().ok().filter(|n| *n >= 1) }
/// Whether the chunk has its vector.
#[uniffi::export]
pub fn chunk_is_indexed(chunk: Chunk) -> bool { chunk.embedded_at_ms.is_some() }
/// Refuses a chunk with no message, no text, an id below 1 or an attachment part with no attachment id.
#[uniffi::export]
pub fn check_chunk(chunk: Chunk) -> Result<(), CoreError> {
    if chunk.id < 1 { return Err(refuse(CHUNK_REFUSAL_CODE, "a chunk id starts at 1")); }
    if blank(&chunk.message_id) { return Err(refuse(CHUNK_REFUSAL_CODE, "a chunk belongs to a message")); }
    if blank(&chunk.text) { return Err(refuse(CHUNK_REFUSAL_CODE, "a chunk has text")); }
    if matches!(&chunk.part, ChunkPart::Attachment { attachment_id } if blank(attachment_id)) { return Err(refuse(CHUNK_REFUSAL_CODE, "an attachment chunk names its attachment")); }
    Ok(())
}

// ---------------------------------------------------------------- the crawl's place and progress

/// DATA-MAP `body-crawl-place`: where the background body crawl of one folder stands (columns of `folder_sync_state`, replacing the `body-crawl.tsv` file).
/// A place is a pair (`before_ms`, `before_id`): everything newer was visited. `done_ms` = the walk finished then, and then there is no place.
#[derive(uniffi::Record, Debug, Clone, PartialEq, Eq)]
pub struct BodyCrawlPlace { pub account_id: String, pub folder: String, pub before_ms: Option<i64>, pub before_id: Option<String>, pub done_ms: Option<i64> }

/// The legal shapes (the store's trigger repeats them): the two halves of a place are both set or both absent; a finished walk has no place.
#[uniffi::export]
pub fn crawl_place_is_legal(place: BodyCrawlPlace) -> bool {
    place.before_ms.is_some() == place.before_id.is_some() && !(place.done_ms.is_some() && place.before_ms.is_some())
}
/// Whether the walk of this folder finished.
#[uniffi::export]
pub fn crawl_is_done(place: BodyCrawlPlace) -> bool { place.done_ms.is_some() }

/// DATA-MAP-less read shape: what the crawl and the indexer did in one folder of one account. `messages` = messages stored, `bodies` = bodies stored,
/// `indexed` = bodies whose chunks are all in the meaning index. Each is a subset of the one before (`check_folder_progress`).
#[derive(uniffi::Record, Debug, Clone, PartialEq, Eq)]
pub struct FolderProgress { pub account_id: String, pub folder: String, pub messages: u64, pub bodies: u64, pub indexed: u64 }

/// Refuses figures that cannot be: more bodies than messages, more indexed than bodies.
#[uniffi::export]
pub fn check_folder_progress(p: FolderProgress) -> Result<(), CoreError> {
    if p.bodies > p.messages { return Err(refuse(FOLDER_PROGRESS_REFUSAL_CODE, "more bodies than messages")); }
    if p.indexed > p.bodies { return Err(refuse(FOLDER_PROGRESS_REFUSAL_CODE, "more indexed than stored bodies")); }
    Ok(())
}

/// The Sources pane's `ReplySyncProgress` of one folder, derived (never stored twice): not enabled = NotStarted; enabled with nothing stored yet = Syncing 0;
/// every message indexed = UpToDate; else Syncing with the share of messages indexed (100 only when all are).
#[uniffi::export]
pub fn folder_sync_progress(p: FolderProgress, enabled: bool) -> ReplySyncProgress {
    if !enabled { return ReplySyncProgress::NotStarted; }
    if p.messages > 0 && p.indexed >= p.messages { return ReplySyncProgress::UpToDate; }
    ReplySyncProgress::Syncing { percent: sync_percent(p.indexed, p.messages).min(99) }
}

/// The `ReplySource` of a mail folder from its progress: the item count is the messages stored.
#[uniffi::export]
pub fn folder_progress_source(p: FolderProgress, enabled: bool) -> ReplySource {
    ReplySource { source: ReplySourceRef::MailFolder { account_id: p.account_id.clone(), folder: p.folder.clone() }, enabled, item_count: p.messages, progress: folder_sync_progress(p, enabled) }
}

// ---------------------------------------------------------------- state JSON

fn js(v: &str) -> String {
    let mut o = String::from("\"");
    for c in v.chars() { match c { '"' => o.push_str("\\\""), '\\' => o.push_str("\\\\"), '\n' => o.push_str("\\n"), '\r' => o.push_str("\\r"), '\t' => o.push_str("\\t"), c if (c as u32) < 0x20 => o.push_str(&format!("\\u{:04x}", c as u32)), c => o.push(c) } }
    o.push('"');
    o
}
fn ojs(v: &Option<String>) -> String { v.as_deref().map(js).unwrap_or_else(|| "null".into()) }
fn obj(fields: Vec<(&str, String)>) -> String { format!("{{{}}}", fields.into_iter().map(|(k, v)| format!("{}:{}", js(k), v)).collect::<Vec<_>>().join(",")) }
fn arr(items: Vec<String>) -> String { format!("[{}]", items.join(",")) }

#[uniffi::export]
pub fn enabled_calendar_ids_json(calendars: Vec<ReplyCalendar>) -> String { arr(enabled_calendar_ids(calendars).iter().map(|i| js(i)).collect()) }
#[uniffi::export]
pub fn calendar_events_json(events: Vec<CalendarEvent>) -> String {
    arr(events.iter().map(|e| obj(vec![("event_id", js(&e.event_id)), ("calendar_id", js(&e.calendar_id)), ("title", js(&e.title)), ("start_ms", e.start_ms.to_string()),
        ("end_ms", e.end_ms.to_string()), ("attendees", arr(e.attendees.iter().map(|a| js(a)).collect())), ("location", ojs(&e.location))])).collect())
}
#[uniffi::export]
pub fn chunk_json(c: Chunk) -> String {
    obj(vec![("id", c.id.to_string()), ("message_id", js(&c.message_id)), ("part", js(&chunk_part_key(c.part))), ("chunk_no", c.chunk_no.to_string()),
        ("received_at_ms", c.received_at_ms.to_string()), ("sender", js(&c.sender)), ("folder", js(&c.folder)), ("subject", js(&c.subject)), ("text", js(&c.text)),
        ("embedded_at_ms", c.embedded_at_ms.map(|n| n.to_string()).unwrap_or_else(|| "null".into()))])
}
/// `reply/progress`: one object per account and folder, with the derived `progress` as if the folder were enabled (the screen reads the switch from `reply/sources`).
#[uniffi::export]
pub fn folder_progress_json(rows: Vec<FolderProgress>) -> String {
    arr(rows.into_iter().map(|p| {
        let state = match folder_sync_progress(p.clone(), true) {
            ReplySyncProgress::UpToDate => obj(vec![("state", js("up_to_date"))]),
            ReplySyncProgress::Syncing { percent } => obj(vec![("state", js("syncing")), ("percent", percent.to_string())]),
            ReplySyncProgress::NotStarted => obj(vec![("state", js("not_started"))]),
            ReplySyncProgress::Failed { reason_words } => obj(vec![("state", js("failed")), ("reason_words", js(&reason_words))]),
            ReplySyncProgress::Gone => obj(vec![("state", js("gone"))]),
        };
        obj(vec![("account_id", js(&p.account_id)), ("folder", js(&p.folder)), ("messages", p.messages.to_string()), ("bodies", p.bodies.to_string()), ("indexed", p.indexed.to_string()), ("progress", state)])
    }).collect())
}
