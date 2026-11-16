//! fn-outbox-undo-send, the undo half: inside the window nothing leaves; after it, exactly once.
mod support;

use contracts::{CoreError, OutboxState};
use outbox::{undo_bounds, DeviceRole};
use support::*;

const T0: i64 = EIGHT - 3 * 60 * MINUTE;

fn device(smtp: &FakeSmtp) -> Device {
    Device::new("mac", DeviceRole::Primary, T0, smtp.port, None)
}

#[test]
fn the_default_window_is_the_ledgers_and_the_bounds_are_enforced() {
    let b = undo_bounds().expect("ledger row");
    let smtp = FakeSmtp::start(Mode::Accept);
    let d = device(&smtp);
    d.put_draft("draft-1", "Yes, 12,000 units works.");
    let view = d.outbox.send_draft("draft-1").expect("send");
    assert_eq!(view.entry.state, OutboxState::Undoable { until_ms: T0 + i64::from(b.default_s) * 1000 }, "default window comes from docs/CONSTANTS.md");
    assert!(matches!(d.outbox.set_undo_window_s(b.min_s - 1), Err(CoreError::Invalid { .. })));
    assert!(matches!(d.outbox.set_undo_window_s(b.max_s + 1), Err(CoreError::Invalid { .. })));
    d.outbox.set_undo_window_s(b.min_s).expect("min is allowed");
    d.outbox.set_undo_window_s(b.max_s).expect("max is allowed");
}

#[test]
fn undo_inside_the_window_sends_nothing_and_gives_the_draft_back() {
    let smtp = FakeSmtp::start(Mode::Accept);
    let d = device(&smtp);
    d.put_draft("draft-1", "Yes, 12,000 units works.");
    assert_eq!(d.draft_rows(), 1);
    let id = d.outbox.send_draft("draft-1").expect("send").entry.id;
    assert_eq!(d.draft_rows(), 0, "the draft moved into the outbox");

    d.clock.advance(4_000);
    d.outbox.cancel(&id).expect("undo inside the window");
    assert_eq!(d.state(&id), OutboxState::Cancelled);
    assert_eq!(d.draft_rows(), 1, "the composer reopens with the same text");

    d.clock.advance(60_000);
    let report = d.outbox.tick().expect("tick");
    assert!(report.delivered.is_empty());
    assert_eq!(smtp.received().len(), 0, "the test SMTP server received 0 messages");
    assert_eq!(d.sent.count(), 0, "the Sent folder holds 0 copies");
    assert_eq!(d.outbox.open_entries().unwrap().len(), 0);
}

#[test]
fn no_undo_sends_exactly_one_message_and_one_sent_copy_when_the_window_elapses() {
    let b = undo_bounds().unwrap();
    let smtp = FakeSmtp::start(Mode::Accept);
    let d = device(&smtp);
    d.put_draft("draft-1", "Yes, 12,000 units works.");
    let id = d.outbox.send_draft("draft-1").unwrap().entry.id;
    let window_ms = i64::from(b.default_s) * 1000;

    d.clock.set(T0 + window_ms - 1);
    assert!(d.outbox.tick().unwrap().delivered.is_empty(), "one ms before the end nothing leaves");
    assert_eq!(smtp.received().len(), 0);

    d.clock.set(T0 + window_ms);
    assert_eq!(d.outbox.tick().unwrap().delivered, vec![id.clone()]);
    assert_eq!(smtp.received().len(), 1, "exactly 1 received");
    assert_eq!(d.sent.count(), 1, "exactly 1 Sent copy");
    assert!(matches!(d.state(&id), OutboxState::Sent { .. }));

    for step in 1..=3 {
        d.clock.advance(step * 30_000);
        assert!(d.outbox.tick().unwrap().delivered.is_empty());
    }
    assert_eq!(smtp.received().len(), 1, "later ticks never send it again");
    assert_eq!(d.sent.count(), 1);
    assert_eq!(d.events.completed(), 1, "SendCompleted is published once");
    let states = d.events.states(&id);
    assert!(matches!(states.first(), Some(OutboxState::Undoable { .. })) && matches!(states.last(), Some(OutboxState::Sent { .. })), "stream order: {states:?}");
}

#[test]
fn the_window_is_closed_at_its_last_millisecond_and_after() {
    let b = undo_bounds().unwrap();
    let smtp = FakeSmtp::start(Mode::Accept);
    let d = device(&smtp);
    d.put_draft("draft-1", "x");
    let id = d.outbox.send_draft("draft-1").unwrap().entry.id;
    d.clock.set(T0 + i64::from(b.default_s) * 1000);
    let err = d.outbox.cancel(&id).expect_err("too late to undo");
    assert!(err.to_string().contains("undo window is over"), "{err}");
    d.outbox.tick().unwrap();
    assert_eq!(smtp.received().len(), 1);
    let err = d.outbox.cancel(&id).expect_err("sent");
    assert!(err.to_string().contains("already sent"), "{err}");
}

#[test]
fn a_shorter_window_set_in_settings_applies_to_the_next_send() {
    let b = undo_bounds().unwrap();
    let smtp = FakeSmtp::start(Mode::Accept);
    let d = device(&smtp);
    d.outbox.set_undo_window_s(b.min_s).unwrap();
    d.put_draft("draft-1", "x");
    let id = d.outbox.send_draft("draft-1").unwrap().entry.id;
    d.clock.set(T0 + i64::from(b.min_s) * 1000);
    d.outbox.tick().unwrap();
    assert!(matches!(d.state(&id), OutboxState::Sent { .. }));
    assert_eq!(smtp.received().len(), 1);
}

#[test]
fn a_second_send_of_the_same_draft_is_refused_while_it_waits() {
    let smtp = FakeSmtp::start(Mode::Accept);
    let d = device(&smtp);
    let draft = d.put_draft("draft-1", "x");
    d.outbox.send(draft.clone()).unwrap();
    let err = d.outbox.send(draft).expect_err("double tap");
    assert!(err.to_string().contains("already in the outbox"), "{err}");
    assert_eq!(d.outbox.open_entries().unwrap().len(), 1);
}

#[test]
fn a_draft_without_a_recipient_is_refused_before_anything_is_stored() {
    let smtp = FakeSmtp::start(Mode::Accept);
    let d = device(&smtp);
    let mut draft = d.put_draft("draft-1", "x");
    draft.to.clear();
    let err = d.outbox.send(draft).expect_err("no recipient");
    assert!(err.to_string().contains("recipient"), "{err}");
    assert_eq!(d.outbox.open_entries().unwrap().len(), 0);
    assert_eq!(d.draft_rows(), 1, "the draft stays where it was");
}
