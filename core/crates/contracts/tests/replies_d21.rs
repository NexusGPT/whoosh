//! Delta 21 probes: calendars pushed by the shell, the chunk with its own id, the crawl's place, the folder progress.
//! MUTANTS (_captures/replies-contract/mutants.py N01..N20 apply each, run this file, expect the named test RED, restore):
//!   N01 a calendar the core did not know starts ENABLED                                   -> a_sync_keeps_what_the_core_knows_and_a_new_calendar_starts_off
//!   N02 a known calendar loses its switch (always off)                                    -> a_sync_keeps_what_the_core_knows_and_a_new_calendar_starts_off
//!   N03 the calendars the shell no longer lists are not reported dropped                  -> a_sync_drops_the_calendars_that_are_gone
//!   N04 an event of a calendar that is OFF is kept                                        -> only_the_events_of_an_enabled_listed_calendar_inside_the_window_are_kept
//!   N05 the window is ignored                                                             -> only_the_events_of_an_enabled_listed_calendar_inside_the_window_are_kept
//!   N06 a second event with the same (calendar, id) is kept beside the first             -> only_the_events_of_an_enabled_listed_calendar_inside_the_window_are_kept
//!   N07 an empty window is accepted                                                       -> a_sync_that_cannot_be_true_is_refused_in_words
//!   N08 an event that ends before it starts is accepted                                   -> a_sync_that_cannot_be_true_is_refused_in_words
//!   N09 a colour that is not #RRGGBB is accepted                                          -> a_sync_that_cannot_be_true_is_refused_in_words
//!   N10 the vector key is not the chunk's own id                                          -> a_chunk_is_keyed_by_its_own_id_and_has_one_spelling
//!   N11 `attachment:` with no id reads as a part                                          -> a_chunk_is_keyed_by_its_own_id_and_has_one_spelling
//!   N12 `check_chunk` lets blank text through                                             -> a_chunk_is_keyed_by_its_own_id_and_has_one_spelling
//!   N13 `crawl_place_is_legal` accepts one half of a place                                -> a_crawl_place_has_both_halves_and_a_finished_walk_has_none
//!   N14 `crawl_place_is_legal` accepts a finished walk that still has a place             -> a_crawl_place_has_both_halves_and_a_finished_walk_has_none
//!   N15 `check_folder_progress` lets more bodies than messages through                    -> the_progress_figures_are_nested_and_the_pane_state_is_derived_from_them
//!   N16 a folder is up to date when its bodies are stored, not when they are indexed      -> the_progress_figures_are_nested_and_the_pane_state_is_derived_from_them
//!   N17 a folder that is not enabled still reports syncing                                -> the_progress_figures_are_nested_and_the_pane_state_is_derived_from_them
//!   N18 the source's item count is the indexed figure, not the messages stored            -> the_progress_figures_are_nested_and_the_pane_state_is_derived_from_them
//!   N19 `enabled_calendar_ids` lists a disabled calendar                                  -> the_shell_reads_the_enabled_ids_and_the_json_names_its_keys
//!   N20 `folder_progress_json` spells `bodies` as `stored`                                -> the_shell_reads_the_enabled_ids_and_the_json_names_its_keys
//!   N21 a disabled mail folder of this account is in scope                                  -> the_scope_rule_is_written_once
//!   N22 the other accounts' folders are in scope while the scope is "this account"          -> the_scope_rule_is_written_once
//!   N23 the other accounts' folders are left out when the scope is "all accounts"           -> the_scope_rule_is_written_once
//!   N24 the other accounts' folders come before this account's                               -> the_scope_rule_is_written_once
//!   N25 the folders of this Mac are left out                                                 -> the_scope_rule_is_written_once
//!   N26 a disabled calendar is in scope                                                      -> the_scope_rule_is_written_once
//!   N27 the calendars are left out                                                           -> the_scope_rule_is_written_once
use contracts::*;

fn code_of<T: std::fmt::Debug>(r: Result<T, CoreError>) -> String { match r { Err(CoreError::Invalid { field, .. }) => field, o => panic!("expected a refusal, got {o:?}") } }
fn cal(id: &str, on: bool) -> ReplyCalendar { ReplyCalendar { calendar_id: id.into(), name: id.into(), account_label: "a".into(), colour: "#007AFF".into(), enabled: on } }
fn info(id: &str) -> ReplyCalendarInfo { ReplyCalendarInfo { calendar_id: id.into(), name: format!("name {id}"), account_label: "a".into(), colour: "#34C759".into() } }
fn ev(cal: &str, id: &str, s: i64, e: i64) -> CalendarEvent { CalendarEvent { event_id: id.into(), calendar_id: cal.into(), title: "Meeting".into(), start_ms: s, end_ms: e, attendees: vec!["a@x.example".into()], location: None } }

#[test]
fn a_sync_keeps_what_the_core_knows_and_a_new_calendar_starts_off() {
    let known = vec![cal("work", true), cal("home", false)];
    let r = apply_calendar_sync(known, vec![info("work"), info("home"), info("new")], 0, 100, vec![]).unwrap();
    let on: Vec<(String, bool)> = r.calendars.iter().map(|c| (c.calendar_id.clone(), c.enabled)).collect();
    assert_eq!(on, vec![("work".into(), true), ("home".into(), false), ("new".into(), false)], "known switches kept, a new calendar OFF, the shell's order");
    assert_eq!(r.calendars[0].name, "name work", "the shell's name and colour replace the held ones");
    assert_eq!(r.calendars[0].colour, "#34C759");
}

#[test]
fn a_sync_drops_the_calendars_that_are_gone() {
    let r = apply_calendar_sync(vec![cal("work", true), cal("old", true)], vec![info("work")], 0, 100, vec![]).unwrap();
    assert_eq!(r.dropped_calendar_ids, vec!["old".to_string()]);
    assert_eq!(r.calendars.len(), 1);
}

#[test]
fn only_the_events_of_an_enabled_listed_calendar_inside_the_window_are_kept() {
    let known = vec![cal("work", true), cal("home", false)];
    let events = vec![ev("work", "e1", 10, 20), ev("home", "e2", 10, 20), ev("ghost", "e3", 10, 20), ev("work", "e4", 500, 600), ev("work", "e5", 0, 5),
        ev("work", "e1", 30, 40)];
    let r = apply_calendar_sync(known, vec![info("work"), info("home")], 8, 100, events).unwrap();
    let kept: Vec<(String, i64)> = r.events.iter().map(|e| (e.event_id.clone(), e.start_ms)).collect();
    assert_eq!(kept, vec![("e1".into(), 30)], "off calendar, unknown calendar, after the window, before the window are ignored; the second e1 replaces the first");
    assert_eq!(r.ignored_events, 4);
    // an event that overlaps the window's start is kept
    let r = apply_calendar_sync(vec![cal("work", true)], vec![info("work")], 10, 100, vec![ev("work", "e", 5, 12)]).unwrap();
    assert_eq!(r.events.len(), 1);
}

#[test]
fn a_sync_that_cannot_be_true_is_refused_in_words() {
    let ok = |c: Vec<ReplyCalendarInfo>, s, e, ev: Vec<CalendarEvent>| apply_calendar_sync(vec![], c, s, e, ev);
    assert_eq!(code_of(ok(vec![info("a")], 5, 5, vec![])), CALENDAR_SYNC_REFUSAL_CODE, "an empty window");
    assert_eq!(code_of(ok(vec![info("a")], 9, 5, vec![])), CALENDAR_SYNC_REFUSAL_CODE, "an inverted window");
    assert_eq!(code_of(ok(vec![ReplyCalendarInfo { colour: "blue".into(), ..info("a") }], 0, 5, vec![])), CALENDAR_SYNC_REFUSAL_CODE);
    assert_eq!(code_of(ok(vec![ReplyCalendarInfo { colour: "#12345G".into(), ..info("a") }], 0, 5, vec![])), CALENDAR_SYNC_REFUSAL_CODE);
    assert_eq!(code_of(ok(vec![ReplyCalendarInfo { name: " ".into(), ..info("a") }], 0, 5, vec![])), CALENDAR_SYNC_REFUSAL_CODE);
    assert_eq!(code_of(ok(vec![ReplyCalendarInfo { calendar_id: "".into(), ..info("a") }], 0, 5, vec![])), CALENDAR_SYNC_REFUSAL_CODE);
    assert_eq!(code_of(ok(vec![info("a")], 0, 50, vec![ev("a", "e", 20, 10)])), CALENDAR_SYNC_REFUSAL_CODE, "ends before it starts");
    assert_eq!(code_of(ok(vec![info("a")], 0, 50, vec![ev("a", " ", 1, 2)])), CALENDAR_SYNC_REFUSAL_CODE, "no event id");
    ok(vec![info("a")], 0, 50, vec![ev("a", "zero", 7, 7)]).expect("a zero-length event is legal");
}

fn chunk(id: i64) -> Chunk {
    Chunk { id, message_id: "m1".into(), part: ChunkPart::Body, chunk_no: 0, received_at_ms: 5, sender: "Laura".into(), folder: "Clients".into(), subject: "Devis".into(), text: "Le devis est de 4 200.".into(), embedded_at_ms: None }
}

#[test]
fn a_chunk_is_keyed_by_its_own_id_and_has_one_spelling() {
    assert_eq!(chunk_vector_key(42), 42, "the vector is filed under the chunk's own id, never the message's rowid");
    assert_eq!(chunk_id_from_excerpt_id(chunk_excerpt_id(42)), Some(42));
    assert_eq!(chunk_id_from_excerpt_id("0".into()), None);
    assert_eq!(chunk_id_from_excerpt_id("chunk-1".into()), None);
    for p in [ChunkPart::Body, ChunkPart::Attachment { attachment_id: "att-9".into() }] { assert_eq!(chunk_part_from_key(chunk_part_key(p.clone())), Some(p)); }
    assert_eq!(chunk_part_key(ChunkPart::Attachment { attachment_id: "att-9".into() }), "attachment:att-9");
    assert_eq!(chunk_part_from_key("attachment:".into()), None, "an attachment part names its attachment");
    assert_eq!(chunk_part_from_key("footer".into()), None);
    check_chunk(chunk(1)).unwrap();
    assert_eq!(code_of(check_chunk(chunk(0))), CHUNK_REFUSAL_CODE);
    assert_eq!(code_of(check_chunk(Chunk { text: "  ".into(), ..chunk(1) })), CHUNK_REFUSAL_CODE);
    assert_eq!(code_of(check_chunk(Chunk { message_id: "".into(), ..chunk(1) })), CHUNK_REFUSAL_CODE);
    assert_eq!(code_of(check_chunk(Chunk { part: ChunkPart::Attachment { attachment_id: " ".into() }, ..chunk(1) })), CHUNK_REFUSAL_CODE);
    assert!(!chunk_is_indexed(chunk(1)));
    assert!(chunk_is_indexed(Chunk { embedded_at_ms: Some(9), ..chunk(1) }));
}

#[test]
fn a_crawl_place_has_both_halves_and_a_finished_walk_has_none() {
    let p = |b: Option<i64>, i: Option<&str>, d: Option<i64>| BodyCrawlPlace { account_id: "a".into(), folder: "INBOX".into(), before_ms: b, before_id: i.map(String::from), done_ms: d };
    assert!(crawl_place_is_legal(p(None, None, None)), "never started");
    assert!(crawl_place_is_legal(p(Some(5), Some("m9"), None)), "walking");
    assert!(crawl_place_is_legal(p(None, None, Some(9))), "finished");
    assert!(!crawl_place_is_legal(p(Some(5), None, None)), "a time with no id");
    assert!(!crawl_place_is_legal(p(None, Some("m9"), None)), "an id with no time");
    assert!(!crawl_place_is_legal(p(Some(5), Some("m9"), Some(9))), "finished and still walking");
    assert!(crawl_is_done(p(None, None, Some(9))) && !crawl_is_done(p(Some(5), Some("m9"), None)));
}

fn prog(m: u64, b: u64, i: u64) -> FolderProgress { FolderProgress { account_id: "a".into(), folder: "Clients".into(), messages: m, bodies: b, indexed: i } }

#[test]
fn the_progress_figures_are_nested_and_the_pane_state_is_derived_from_them() {
    check_folder_progress(prog(10, 8, 5)).unwrap();
    check_folder_progress(prog(0, 0, 0)).unwrap();
    assert_eq!(code_of(check_folder_progress(prog(5, 6, 0))), FOLDER_PROGRESS_REFUSAL_CODE, "more bodies than messages");
    assert_eq!(code_of(check_folder_progress(prog(5, 5, 6))), FOLDER_PROGRESS_REFUSAL_CODE, "more indexed than bodies");
    assert_eq!(folder_sync_progress(prog(10, 10, 6), true), ReplySyncProgress::Syncing { percent: 60 });
    assert_eq!(folder_sync_progress(prog(10, 10, 10), true), ReplySyncProgress::UpToDate);
    assert_eq!(folder_sync_progress(prog(10, 10, 9), true), ReplySyncProgress::Syncing { percent: 90 }, "bodies stored is not done: the vectors are the end of the pipe");
    assert_eq!(folder_sync_progress(prog(0, 0, 0), true), ReplySyncProgress::Syncing { percent: 0 }, "enabled, nothing stored yet");
    assert_eq!(folder_sync_progress(prog(10, 10, 10), false), ReplySyncProgress::NotStarted, "a folder that is off");
    assert_eq!(folder_sync_progress(prog(1000, 1000, 999), true), ReplySyncProgress::Syncing { percent: 99 }, "100 only when every message is indexed");
    let s = folder_progress_source(prog(9204, 9204, 100), true);
    assert_eq!((s.item_count, s.enabled), (9204, true), "the count is the messages stored");
    assert_eq!(s.source, ReplySourceRef::MailFolder { account_id: "a".into(), folder: "Clients".into() });
}

#[test]
fn the_shell_reads_the_enabled_ids_and_the_json_names_its_keys() {
    let cals = vec![cal("work", true), cal("home", false), cal("side", true)];
    assert_eq!(enabled_calendar_ids(cals.clone()), vec!["work".to_string(), "side".to_string()]);
    assert_eq!(enabled_calendar_ids_json(cals), "[\"work\",\"side\"]");
    let j: serde_json::Value = serde_json::from_str(&calendar_events_json(vec![CalendarEvent { location: Some("Room 2".into()), ..ev("work", "e1", 1, 2) }])).unwrap();
    let mut k: Vec<&String> = j[0].as_object().unwrap().keys().collect(); k.sort();
    assert_eq!(k, ["attendees", "calendar_id", "end_ms", "event_id", "location", "start_ms", "title"]);
    assert_eq!(j[0]["location"], "Room 2");
    let n: serde_json::Value = serde_json::from_str(&calendar_events_json(vec![ev("work", "e1", 1, 2)])).unwrap();
    assert!(n[0]["location"].is_null());
    let p: serde_json::Value = serde_json::from_str(&folder_progress_json(vec![prog(10, 8, 4)])).unwrap();
    let mut k: Vec<&String> = p[0].as_object().unwrap().keys().collect(); k.sort();
    assert_eq!(k, ["account_id", "bodies", "folder", "indexed", "messages", "progress"]);
    assert_eq!((p[0]["progress"]["state"].clone(), p[0]["progress"]["percent"].clone()), (serde_json::json!("syncing"), serde_json::json!(40)));
    let c: serde_json::Value = serde_json::from_str(&chunk_json(Chunk { part: ChunkPart::Attachment { attachment_id: "a1".into() }, ..chunk(7) })).unwrap();
    let mut k: Vec<&String> = c.as_object().unwrap().keys().collect(); k.sort();
    assert_eq!(k, ["chunk_no", "embedded_at_ms", "folder", "id", "message_id", "part", "received_at_ms", "sender", "subject", "text"]);
    assert_eq!((c["part"].clone(), c["id"].clone()), (serde_json::json!("attachment:a1"), serde_json::json!(7)));
    let cmd = Command::SyncReplyCalendars { calendars: vec![info("a")], window_start_ms: 0, window_end_ms: 9, events: vec![ev("a", "e", 1, 2)] };
    assert_eq!(cmd.clone(), cmd);
}

#[test]
fn the_scope_rule_is_written_once() {
    let mail = |a: &str, f: &str, on: bool| ReplySource { source: ReplySourceRef::MailFolder { account_id: a.into(), folder: f.into() }, enabled: on, item_count: 1, progress: ReplySyncProgress::UpToDate };
    let local = |p: &str, on: bool| ReplySource { source: ReplySourceRef::LocalFolder { path: p.into() }, enabled: on, item_count: 1, progress: ReplySyncProgress::UpToDate };
    // the pane lists account "b" before account "a": this account's folders still come first
    let accounts = vec![reply_account_sources("b".into(), vec![mail("b", "INBOX", true), mail("b", "Old", false)]),
        reply_account_sources("a".into(), vec![mail("a", "INBOX", true), mail("a", "Off", false), mail("a", "Clients", true)])];
    let read = |scope| reply_sources_read(accounts.clone(), vec![local("~/Docs", true), local("~/Off", false)], vec![cal("work", true), cal("home", false)], scope);
    let m = |a: &str, f: &str| ReplySourceRef::MailFolder { account_id: a.into(), folder: f.into() };
    let tail = vec![ReplySourceRef::LocalFolder { path: "~/Docs".into() }, ReplySourceRef::Calendar { calendar_id: "work".into() }];
    let this = sources_in_scope(read(RetrievalScope::ThisAccount), "a".into());
    assert_eq!(this, [vec![m("a", "INBOX"), m("a", "Clients")], tail.clone()].concat(), "this account's enabled folders, then this Mac's enabled folders and the enabled calendars; no other account, nothing disabled");
    let all = sources_in_scope(read(RetrievalScope::AllAccounts), "a".into());
    assert_eq!(all, [vec![m("a", "INBOX"), m("a", "Clients"), m("b", "INBOX")], tail].concat(), "all accounts: this one first, then the others in the pane's order, then the shared sources");
    assert_eq!(sources_in_scope(read(RetrievalScope::ThisAccount), "ghost".into()).len(), 2, "an account with no folder still gets this Mac's folders and the calendars");
}
