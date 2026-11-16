//! fn-outbox-schedule: send later, with a clock the test moves.
mod support;

use contracts::OutboxState;
use outbox::DeviceRole;
use support::*;

const NIGHT: i64 = EIGHT - 9 * 60 * MINUTE; // 23:00 the evening before

fn device(smtp: &FakeSmtp) -> Device {
    Device::new("mac", DeviceRole::Primary, NIGHT, smtp.port, None)
}

#[test]
fn an_entry_scheduled_for_eight_sends_at_eight_once() {
    let smtp = FakeSmtp::start(Mode::Accept);
    let d = device(&smtp);
    d.put_draft("draft-1", "Good morning, the figures are attached.");
    let view = d.outbox.schedule_draft("draft-1", EIGHT).expect("schedule");
    assert_eq!(view.entry.state, OutboxState::Scheduled { at_ms: EIGHT });
    assert_eq!(d.draft_rows(), 0);

    for t in [NIGHT + MINUTE, EIGHT - 60 * MINUTE, EIGHT - 1] {
        d.clock.set(t);
        assert!(d.outbox.tick().unwrap().delivered.is_empty());
    }
    assert_eq!(smtp.received().len(), 0, "nothing before 08:00");

    d.clock.set(EIGHT);
    assert_eq!(d.outbox.tick().unwrap().delivered.len(), 1);
    assert_eq!(smtp.received().len(), 1, "sent at 08:00");
    assert_eq!(d.sent.count(), 1);
    let id = view.entry.id;
    assert!(!d.outbox.get(&id).unwrap().unwrap().late, "on time is not late");

    for k in 1..=3 {
        d.clock.set(EIGHT + k * 5 * MINUTE);
        d.outbox.tick().unwrap();
    }
    assert_eq!(smtp.received().len(), 1, "once");
}

#[test]
fn with_the_mac_asleep_past_the_time_it_sends_on_wake_and_marks_itself_late() {
    let smtp = FakeSmtp::start(Mode::Accept);
    let d = device(&smtp);
    d.put_draft("draft-1", "Good morning.");
    let id = d.outbox.schedule_draft("draft-1", EIGHT).unwrap().entry.id;

    // no tick between 23:00 and 10:30: the Mac slept through 08:00
    let wake = EIGHT + 150 * MINUTE;
    d.clock.set(wake);
    assert_eq!(smtp.received().len(), 0, "asleep: nothing could leave");
    assert_eq!(d.outbox.tick().unwrap().delivered, vec![id.clone()], "on wake the next tick sends it");
    assert_eq!(smtp.received().len(), 1);
    let view = d.outbox.get(&id).unwrap().unwrap();
    assert_eq!(view.entry.state, OutboxState::Sent { at_ms: wake });
    assert!(view.late, "Scheduled marks it late");
    assert_eq!(view.due_at_ms, EIGHT);
    d.outbox.tick().unwrap();
    assert_eq!(smtp.received().len(), 1);
}

#[test]
fn the_late_line_is_the_configured_figure() {
    let smtp = FakeSmtp::start(Mode::Accept);
    let d = device(&smtp);
    d.put_draft("draft-1", "x");
    let id = d.outbox.schedule_draft("draft-1", EIGHT).unwrap().entry.id;
    d.clock.set(EIGHT + LATE_AFTER_MS);
    d.outbox.tick().unwrap();
    assert!(!d.outbox.get(&id).unwrap().unwrap().late, "exactly on the line is not late");

    d.put_draft("draft-2", "y");
    let id2 = d.outbox.schedule_draft("draft-2", EIGHT + 10 * MINUTE).unwrap().entry.id;
    d.clock.set(EIGHT + 10 * MINUTE + LATE_AFTER_MS + 1);
    d.outbox.tick().unwrap();
    assert!(d.outbox.get(&id2).unwrap().unwrap().late, "one ms past it is late");
}

#[test]
fn a_time_in_the_past_is_refused_and_a_scheduled_entry_can_be_cancelled() {
    let smtp = FakeSmtp::start(Mode::Accept);
    let d = device(&smtp);
    d.put_draft("draft-1", "x");
    let err = d.outbox.schedule_draft("draft-1", NIGHT).expect_err("now is not the future");
    assert!(err.to_string().contains("future"), "{err}");
    assert_eq!(d.draft_rows(), 1);

    let id = d.outbox.schedule_draft("draft-1", EIGHT).unwrap().entry.id;
    d.outbox.cancel(&id).expect("cancel scheduled");
    assert_eq!(d.draft_rows(), 1, "the text comes back for editing");
    d.clock.set(EIGHT + MINUTE);
    d.outbox.tick().unwrap();
    assert_eq!(smtp.received().len(), 0);
}

#[test]
fn scheduled_and_waiting_entries_survive_a_restart() {
    let smtp = FakeSmtp::start(Mode::Accept);
    let mut d = device(&smtp);
    d.put_draft("draft-1", "x");
    let id = d.outbox.schedule_draft("draft-1", EIGHT).unwrap().entry.id;
    d.restart();
    assert_eq!(d.outbox.open_entries().unwrap().len(), 1);
    d.clock.set(EIGHT);
    d.outbox.tick().unwrap();
    assert!(matches!(d.state(&id), OutboxState::Sent { .. }));
    assert_eq!(smtp.received().len(), 1);
}
