//! fn-outbox-undo-send, the other half: wait offline and leave on reconnect; retry with the server's reason;
//! the ledger comes before the effect; a delivered receipt means never send again.
mod support;

use contracts::OutboxState;
use outbox::ports::GetEntryPort;
use outbox::{DeviceRole, SendOutcome};
use support::*;

const T0: i64 = EIGHT - 3 * 60 * MINUTE;
const AFTER_WINDOW: i64 = T0 + 60_000;

fn device(smtp: &FakeSmtp) -> Device {
    Device::new("mac", DeviceRole::Primary, T0, smtp.port, None)
}

#[test]
fn network_off_the_entry_waits_and_sends_once_on_reconnect() {
    let smtp = FakeSmtp::start(Mode::Accept);
    let d = device(&smtp);
    d.put_draft("draft-1", "Yes, 12,000 units works.");
    let id = d.outbox.send_draft("draft-1").unwrap().entry.id;
    d.net.set(false);

    d.clock.set(AFTER_WINDOW);
    let report = d.outbox.tick().unwrap();
    assert_eq!((report.waiting, report.delivered.len()), (1, 0));
    assert_eq!(smtp.received().len(), 0);
    match d.state(&id) {
        OutboxState::Failed { failure, attempts } => {
            assert!(failure.reason_words.contains("no network") && failure.retryable && failure.account_name == "Nordlicht", "{failure:?}");
            assert_eq!(attempts, 0, "waiting for the network is not an attempt");
        }
        other => panic!("expected the waiting state, got {other:?}"),
    }
    let quiet = d.events.len();
    for _ in 0..3 {
        d.clock.advance(30_000);
        d.outbox.tick().unwrap();
    }
    assert_eq!(d.events.len(), quiet, "waiting entries make no churn while the network stays off");
    assert_eq!(smtp.received().len(), 0);

    d.net.set(true);
    assert_eq!(d.outbox.tick().unwrap().delivered, vec![id.clone()]);
    assert_eq!(smtp.received().len(), 1, "sent once on reconnect");
    assert_eq!(d.sent.count(), 1);
    d.clock.advance(60_000);
    d.outbox.tick().unwrap();
    assert_eq!(smtp.received().len(), 1);
}

#[test]
fn an_entry_cancelled_while_it_waits_never_leaves() {
    let smtp = FakeSmtp::start(Mode::Accept);
    let d = device(&smtp);
    d.put_draft("draft-1", "x");
    let id = d.outbox.send_draft("draft-1").unwrap().entry.id;
    d.net.set(false);
    d.clock.set(AFTER_WINDOW);
    d.outbox.tick().unwrap();
    d.outbox.cancel(&id).expect("cancel a waiting entry");
    d.net.set(true);
    d.outbox.tick().unwrap();
    assert_eq!(smtp.received().len(), 0);
    assert_eq!(d.draft_rows(), 1);
}

#[test]
fn a_refusal_keeps_the_servers_reason_and_retry_sends_once() {
    let refusing = FakeSmtp::start(Mode::Refuse);
    let accepting = FakeSmtp::start(Mode::Accept);
    let d = device(&refusing);
    d.put_draft("draft-1", "Yes, 12,000 units works.");
    let id = d.outbox.send_draft("draft-1").unwrap().entry.id;
    d.clock.set(AFTER_WINDOW);
    let report = d.outbox.tick().unwrap();
    assert_eq!(report.failed, vec![id.clone()]);
    match d.state(&id) {
        OutboxState::Failed { failure, attempts } => {
            assert!(failure.reason_words.contains(REFUSAL_TEXT), "the server's words reach the owner: {failure:?}");
            assert_eq!((failure.account_name.as_str(), failure.retryable, attempts), ("Nordlicht", false, 1));
        }
        other => panic!("expected Failed, got {other:?}"),
    }
    assert_eq!(refusing.received().len(), 0);

    d.clock.advance(60_000);
    d.outbox.tick().unwrap();
    assert_eq!(refusing.connections.load(std::sync::atomic::Ordering::SeqCst), 1, "a refusal is not retried by itself");

    d.accounts.0.store(accepting.port, std::sync::atomic::Ordering::SeqCst);
    assert_eq!(d.outbox.retry(&id).unwrap(), SendOutcome::Delivered);
    assert_eq!((refusing.received().len(), accepting.received().len(), d.sent.count()), (0, 1, 1));
}

#[test]
fn retry_of_a_delivered_entry_is_refused_and_sends_nothing() {
    let smtp = FakeSmtp::start(Mode::Accept);
    let d = device(&smtp);
    d.put_draft("draft-1", "x");
    let id = d.outbox.send_draft("draft-1").unwrap().entry.id;
    d.clock.set(AFTER_WINDOW);
    d.outbox.tick().unwrap();
    assert_eq!(smtp.received().len(), 1);
    let err = d.outbox.retry(&id).expect_err("already sent");
    assert!(err.to_string().contains("already sent"), "{err}");
    d.outbox.tick().unwrap();
    assert_eq!(smtp.received().len(), 1, "after a delivered receipt, never a second send");
    assert_eq!(d.sent.count(), 1);
}

#[test]
fn the_ledger_is_written_before_the_message_leaves() {
    let smtp = FakeSmtp::start(Mode::Accept);
    let d = device(&smtp);
    d.put_draft("draft-1", "x");
    d.outbox.send_draft("draft-1").unwrap();
    d.clock.set(AFTER_WINDOW);
    d.outbox.tick().unwrap();
    assert_eq!(*d.probe.seen.lock().unwrap(), vec!["Sending".to_string()], "the stored state at the moment of delivery");
}

#[test]
fn a_sent_copy_the_folder_refused_is_retried_without_a_second_delivery() {
    let smtp = FakeSmtp::start(Mode::Accept);
    let d = device(&smtp);
    d.sent.refuse.store(true, std::sync::atomic::Ordering::SeqCst);
    d.put_draft("draft-1", "x");
    let id = d.outbox.send_draft("draft-1").unwrap().entry.id;
    d.clock.set(AFTER_WINDOW);
    let report = d.outbox.tick().unwrap();
    assert_eq!((report.delivered.len(), report.copies_stored), (1, 0));
    assert!(matches!(d.state(&id), OutboxState::Sent { .. }), "the message did leave");
    assert_eq!((smtp.received().len(), d.sent.count()), (1, 0));

    d.sent.refuse.store(false, std::sync::atomic::Ordering::SeqCst);
    d.clock.advance(30_000);
    assert_eq!(d.outbox.tick().unwrap().copies_stored, 1);
    assert_eq!((smtp.received().len(), d.sent.count()), (1, 1), "the copy arrived; the delivery did not repeat");
    d.clock.advance(30_000);
    d.outbox.tick().unwrap();
    assert_eq!((smtp.received().len(), d.sent.count()), (1, 1));
}

#[test]
fn an_entry_left_in_sending_by_a_closed_app_is_never_sent_again_by_itself() {
    let smtp = FakeSmtp::start(Mode::Accept);
    let mut d = device(&smtp);
    d.put_draft("draft-1", "x");
    let id = d.outbox.send_draft("draft-1").unwrap().entry.id;
    // the app dies after the ledger and before the receipt
    let mut record = d.rows.execute(&id).unwrap().expect("stored");
    record.begin_sending(T0);
    d.save_raw(&record);
    d.restart();

    match d.state(&id) {
        OutboxState::Failed { failure, .. } => assert!(failure.reason_words.contains("look in Sent"), "{failure:?}"),
        other => panic!("expected Failed with words, got {other:?}"),
    }
    d.clock.set(AFTER_WINDOW);
    d.outbox.tick().unwrap();
    assert_eq!(smtp.received().len(), 0, "unknown outcome: the owner decides, not the engine");
}
