//! Calendar events as excerpts a draft can read, against a fake CalendarStore.
mod common;
use common::*;
use contracts::{ReplySourceRef, ReplySyncProgress};
use reply_sources::{calendar_window, event_excerpt, sync_calendar, CalendarEvent, CalendarInfo, CalendarSyncReport, SyncOutcome};

const H: i64 = 3_600_000;
const DAY: i64 = 24 * H;
/// 2026-11-10 00:00 UTC
const NOW: i64 = 1_794_268_800_000;
const PARIS: i32 = 3600;

fn travail() -> FakeCalendars {
    FakeCalendars {
        calendars: vec![CalendarInfo { calendar_id: "cal-1".into(), name: "Travail".into(), account_label: "assem@nexus-example.com".into(), colour: "#007AFF".into() }],
        ..Default::default()
    }
}

fn ev(id: &str, title: &str, start: i64, end: i64) -> CalendarEvent {
    CalendarEvent { event_id: id.into(), calendar_id: "cal-1".into(), title: title.into(), start_ms: start, end_ms: end, all_day: false, attendees: vec![], location: None }
}

fn source() -> ReplySourceRef {
    ReplySourceRef::Calendar { calendar_id: "cal-1".into() }
}

fn run(store: &FakeCalendars, sink: &FakeSink, progress: &FakeProgress) -> CalendarSyncReport {
    sync_calendar(store, sink, progress, "cal-1", NOW, PARIS, &|| true)
}

#[test]
fn an_event_becomes_one_excerpt_with_title_start_end_attendees_and_location() {
    let store = travail();
    let mut e = ev("e1", "Revue du contrat Oranta", NOW + 2 * DAY + 13 * H, NOW + 2 * DAY + 14 * H + H / 2);
    e.attendees = vec!["Marie Durand".into(), " ".into(), "paul@oranta.example".into()];
    e.location = Some("Salle Rivoli".into());
    store.events.lock().unwrap().push(e);
    let (sink, progress) = (FakeSink::default(), FakeProgress::default());
    let rep = run(&store, &sink, &progress);
    assert_eq!((rep.outcome.clone(), rep.item_count), (SyncOutcome::UpToDate, 1));
    let ex = sink.excerpts(&source());
    assert_eq!(ex.len(), 1);
    let x = &ex[0];
    assert_eq!(x.title, "Revue du contrat Oranta");
    assert_eq!(x.text, "Revue du contrat Oranta\n2026-11-12 14:00 to 15:30\nWith: Marie Durand, paul@oranta.example\nAt: Salle Rivoli", "times in the owner's offset");
    assert_eq!(x.date_ms, Some(NOW + 2 * DAY + 13 * H));
    assert_eq!(x.folder, "Travail", "the calendar's name in the folder slot");
    assert_eq!(x.source, source());
}

#[test]
fn an_event_with_no_one_and_nowhere_says_only_what_it_has() {
    let x = event_excerpt(&source(), "Travail", &ev("e2", "", NOW + 9 * H, NOW + 10 * H), 0);
    assert_eq!(x.text, "(no title)\n2026-11-10 09:00 to 10:00");
}

#[test]
fn an_event_over_two_days_names_both_days() {
    let x = event_excerpt(&source(), "Travail", &ev("e3", "Salon", NOW + 18 * H, NOW + DAY + 10 * H), 0);
    assert_eq!(x.text, "Salon\n2026-11-10 18:00 to 2026-11-11 10:00");
}

#[test]
fn an_all_day_event_names_its_days_not_midnight() {
    let mut one = ev("d1", "Congé", NOW, NOW + DAY);
    one.all_day = true;
    assert_eq!(event_excerpt(&source(), "Travail", &one, 0).text, "Congé\n2026-11-10, all day");
    let mut three = ev("d2", "Séminaire", NOW, NOW + 3 * DAY);
    three.all_day = true;
    assert_eq!(event_excerpt(&source(), "Travail", &three, 0).text, "Séminaire\n2026-11-10 to 2026-11-12, all day");
}

#[test]
fn the_window_is_the_last_7_days_and_the_next_14() {
    assert_eq!(calendar_window(NOW), (NOW - 7 * DAY, NOW + 14 * DAY));
    let store = travail();
    {
        let mut events = store.events.lock().unwrap();
        events.push(ev("old", "Trop vieux", NOW - 8 * DAY, NOW - 8 * DAY + H));
        events.push(ev("past", "La semaine dernière", NOW - 6 * DAY, NOW - 6 * DAY + H));
        events.push(ev("soon", "Dans 13 jours", NOW + 13 * DAY, NOW + 13 * DAY + H));
        events.push(ev("far", "Dans 15 jours", NOW + 15 * DAY, NOW + 15 * DAY + H));
    }
    let (sink, progress) = (FakeSink::default(), FakeProgress::default());
    let rep = run(&store, &sink, &progress);
    assert_eq!(store.asked.lock().unwrap().clone(), vec![("cal-1".to_string(), NOW - 7 * DAY, NOW + 14 * DAY)]);
    assert_eq!(rep.item_count, 2);
    let titles: Vec<String> = sink.excerpts(&source()).into_iter().map(|x| x.title).collect();
    assert!(titles.contains(&"La semaine dernière".to_string()) && titles.contains(&"Dans 13 jours".to_string()));
}

#[test]
fn a_second_sync_writes_only_what_changed_and_removes_what_left() {
    let store = travail();
    store.events.lock().unwrap().extend([ev("a", "Point", NOW + H, NOW + 2 * H), ev("b", "Déjeuner", NOW + DAY, NOW + DAY + H), ev("c", "Annulé", NOW + 2 * DAY, NOW + 2 * DAY + H)]);
    let (sink, progress) = (FakeSink::default(), FakeProgress::default());
    run(&store, &sink, &progress);
    sink.take_puts();
    {
        let mut events = store.events.lock().unwrap();
        events.retain(|e| e.event_id != "c");
        events.iter_mut().find(|e| e.event_id == "b").unwrap().location = Some("Chez Paul".into());
    }
    let rep = run(&store, &sink, &progress);
    assert_eq!((rep.written, rep.unchanged, rep.removed, rep.item_count), (1, 1, 1, 2));
    assert_eq!(sink.take_puts().len(), 1);
    assert!(sink.excerpts(&source()).iter().any(|x| x.text.ends_with("At: Chez Paul")));
    assert!(!sink.excerpts(&source()).iter().any(|x| x.title == "Annulé"));
}

#[test]
fn two_occurrences_of_a_repeating_event_are_two_excerpts() {
    let store = travail();
    store.events.lock().unwrap().extend([ev("weekly", "Hebdo", NOW + H, NOW + 2 * H), ev("weekly", "Hebdo", NOW + 7 * DAY + H, NOW + 7 * DAY + 2 * H)]);
    let (sink, progress) = (FakeSink::default(), FakeProgress::default());
    assert_eq!(run(&store, &sink, &progress).item_count, 2);
    assert_eq!(sink.item_keys(&source()).len(), 2);
}

#[test]
fn calendar_progress_ends_up_to_date_and_failure_carries_words() {
    let store = travail();
    store.events.lock().unwrap().extend([ev("a", "A", NOW + H, NOW + 2 * H), ev("b", "B", NOW + 3 * H, NOW + 4 * H)]);
    let (sink, progress) = (FakeSink::default(), FakeProgress::default());
    run(&store, &sink, &progress);
    assert_eq!(progress.states(), vec![ReplySyncProgress::Syncing { percent: 0 }, ReplySyncProgress::Syncing { percent: 50 }, ReplySyncProgress::Syncing { percent: 100 }, ReplySyncProgress::UpToDate]);

    let refused = FakeCalendars { refuse: true, ..travail() };
    let p2 = FakeProgress::default();
    let rep = run(&refused, &FakeSink::default(), &p2);
    assert!(matches!(&rep.outcome, SyncOutcome::Failed { reason_words } if reason_words.contains("not allowed") && reason_words.contains("Travail")));
    assert!(matches!(p2.states().as_slice(), [ReplySyncProgress::Failed { .. }]));

    let gone = FakeCalendars::default();
    let rep = run(&gone, &FakeSink::default(), &FakeProgress::default());
    assert!(matches!(&rep.outcome, SyncOutcome::Failed { reason_words } if reason_words.contains("no longer")));
}
