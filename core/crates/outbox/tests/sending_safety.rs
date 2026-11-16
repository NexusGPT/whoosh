//! Sending safety: (1) an idempotency key per queued message, (2) In-Reply-To and References on replies and
//! forwards, (3) "sent, copy not saved" when the Sent folder refuses the copy. The server's bytes are asserted.
mod support;

use contracts::{CoreError, OutboxState};
use outbox::ports::{FindByKeyPort, GetEntryPort};
use outbox::domain::{to_row, to_wire_row};
use outbox::{CopyPending, DeviceRole, Record};
use send::ParentHeaders;
use std::sync::atomic::Ordering;
use support::*;

const T0: i64 = EIGHT - 3 * 60 * MINUTE;
const AFTER_WINDOW: i64 = T0 + 60_000;

fn device(smtp: &FakeSmtp) -> Device {
    Device::new("mac", DeviceRole::Primary, T0, smtp.port, None)
}

/// The header block of a received message with folded lines joined, as (lower-case name, value).
fn headers(data: &str) -> Vec<(String, String)> {
    let head = data.split_once("\r\n\r\n").expect("header/body split").0;
    let unfolded = head.replace("\r\n ", " ").replace("\r\n\t", " ");
    unfolded.lines().filter_map(|l| l.split_once(':').map(|(n, v)| (n.to_ascii_lowercase(), v.trim().to_string()))).collect()
}

fn get(h: &[(String, String)], name: &str) -> Vec<String> {
    h.iter().filter(|(n, _)| n == name).map(|(_, v)| v.clone()).collect()
}

// ------------------------------------------------------------------------------------ piece 1: the key

#[test]
fn a_send_retried_with_the_same_key_makes_one_entry_and_the_server_gets_one_message() {
    let smtp = FakeSmtp::start(Mode::Accept);
    let d = device(&smtp);
    d.put_draft("draft-1", "Yes, 12,000 units works.");
    let first = d.outbox.send_draft_keyed("draft-1", "tap-1").unwrap();
    // The app timed out waiting for the answer and sends again with the same key; the draft has already left the draft table.
    let second = d.outbox.send_draft_keyed("draft-1", "tap-1").unwrap();
    assert_eq!(first.entry.id, second.entry.id, "the retry finds the first entry");
    assert_eq!(first.idempotency_key, "tap-1");
    assert_eq!(d.outbox.open_entries().unwrap().len(), 1);

    d.clock.set(AFTER_WINDOW);
    assert_eq!(d.outbox.tick().unwrap().delivered.len(), 1);
    // And once more after it left: still the same entry, nothing queued, nothing sent.
    let third = d.outbox.send_draft_keyed("draft-1", "tap-1").unwrap();
    assert_eq!(third.entry.id, first.entry.id);
    d.clock.advance(60_000);
    d.outbox.tick().unwrap();
    assert_eq!(smtp.received().len(), 1, "one message across a retried send");
}

#[test]
fn a_second_entry_with_a_key_already_sent_is_closed_and_sends_nothing() {
    let smtp = FakeSmtp::start(Mode::Accept);
    let d = device(&smtp);
    d.put_draft("draft-1", "Yes.");
    let first = d.outbox.send_draft_keyed("draft-1", "tap-7").unwrap();
    d.clock.set(AFTER_WINDOW);
    d.outbox.tick().unwrap();
    assert_eq!(smtp.received().len(), 1);
    let sent_at = match d.state(&first.entry.id) {
        OutboxState::Sent { at_ms } => at_ms,
        other => panic!("expected Sent, got {other:?}"),
    };

    // The same message queued again under the same key (a record pulled from another device, a re-queue after a crash).
    let again = d.put_draft("draft-1b", "Yes.");
    let dup = Record::undoable("dup-1".into(), again, AFTER_WINDOW, 10_000, "tap-7".into(), None);
    d.save_raw(&dup);
    d.clock.set(AFTER_WINDOW + 60_000);
    d.outbox.tick().unwrap();
    assert_eq!(smtp.received().len(), 1, "the key is already recorded as sent: no second message leaves");
    assert_eq!(d.state("dup-1"), OutboxState::Sent { at_ms: sent_at }, "the duplicate is closed with the first one's time");
    assert_eq!(d.probe.seen.lock().unwrap().len(), 1, "the sender was called once");
}

#[test]
fn after_a_restart_in_the_middle_of_a_send_the_same_key_finds_the_entry_and_nothing_goes_out() {
    let smtp = FakeSmtp::start(Mode::Accept);
    let mut d = device(&smtp);
    d.put_draft("draft-1", "x");
    let id = d.outbox.send_draft_keyed("draft-1", "tap-3").unwrap().entry.id;
    let mut record = GetEntryPort::execute(&*d.rows, &id).unwrap().expect("stored");
    record.begin_sending(T0); // the ledger is written, then the app dies
    d.save_raw(&record);
    d.restart();

    let again = d.outbox.send_draft_keyed("draft-1", "tap-3").unwrap();
    assert_eq!(again.entry.id, id, "the retry after the restart finds the entry");
    assert!(matches!(again.entry.state, OutboxState::Failed { .. }), "{:?}", again.entry.state);
    d.clock.set(AFTER_WINDOW);
    d.outbox.tick().unwrap();
    assert_eq!(smtp.received().len(), 0, "unknown outcome: the owner decides");
    assert_eq!(d.outbox.open_entries().unwrap().len(), 1, "no second entry was made");
}

#[test]
fn every_queued_message_gets_its_own_uuid_key_and_keeps_it_across_a_restart() {
    let smtp = FakeSmtp::start(Mode::Accept);
    let mut d = device(&smtp);
    d.put_draft("draft-1", "a");
    d.put_draft("draft-2", "b");
    let a = d.outbox.send_draft("draft-1").unwrap();
    let b = d.outbox.send_draft("draft-2").unwrap();
    for key in [&a.idempotency_key, &b.idempotency_key] {
        assert_eq!(key.len(), 36, "{key}");
        assert_eq!(key.chars().filter(|c| *c == '-').count(), 4, "{key}");
        assert_eq!(key.as_bytes()[14], b'4', "a version 4 UUID: {key}");
    }
    assert_ne!(a.idempotency_key, b.idempotency_key);
    d.restart();
    assert_eq!(d.outbox.get(&a.entry.id).unwrap().unwrap().idempotency_key, a.idempotency_key, "persisted with the entry");
    let found = FindByKeyPort::execute(&*d.rows, &a.idempotency_key).unwrap();
    assert_eq!(found.len(), 1);
}

#[test]
fn a_cancelled_entry_does_not_hold_its_key() {
    let smtp = FakeSmtp::start(Mode::Accept);
    let d = device(&smtp);
    d.put_draft("draft-1", "x");
    let first = d.outbox.send_draft_keyed("draft-1", "tap-9").unwrap();
    d.outbox.cancel(&first.entry.id).unwrap(); // undo: the draft is back in the composer
    let second = d.outbox.send_draft_keyed("draft-1", "tap-9").unwrap();
    assert_ne!(first.entry.id, second.entry.id, "it never left, so the key is free to use again");
}

// ------------------------------------------------------------------------------------ piece 2: threading

fn parent_headers() -> ParentHeaders {
    ParentHeaders { message_id: "<c-3@linea-example.com>".into(), references: vec!["<a-1@linea-example.com>".into(), "<b-2@linea-example.com>".into()] }
}

#[test]
fn a_reply_leaves_with_in_reply_to_and_the_parents_chain_read_from_the_server() {
    let smtp = FakeSmtp::start(Mode::Accept);
    let d = device(&smtp);
    d.put_parent("thr-1", "INBOX", 42, parent_headers());
    d.put_draft_in("draft-1", "Yes, 12,000 units works.", Some("thr-1"));
    d.outbox.send_draft("draft-1").unwrap();
    d.clock.set(AFTER_WINDOW);
    d.outbox.tick().unwrap();

    let got = smtp.received();
    assert_eq!(got.len(), 1);
    let h = headers(&got[0].data);
    assert_eq!(get(&h, "in-reply-to"), vec!["<c-3@linea-example.com>".to_string()]);
    assert_eq!(get(&h, "references"), vec!["<a-1@linea-example.com> <b-2@linea-example.com> <c-3@linea-example.com>".to_string()]);
    let asked = d.parents.asked.lock().unwrap().clone();
    assert_eq!((asked.len(), asked[0].folder.as_str(), asked[0].remote_id.as_str()), (1, "INBOX", "42"), "the server was asked for the thread's message: {asked:?}");
}

#[test]
fn a_forward_leaves_with_references_only() {
    let smtp = FakeSmtp::start(Mode::Accept);
    let d = device(&smtp);
    d.put_parent("thr-1", "INBOX", 42, parent_headers());
    d.put_draft("draft-2", "FYI, see below.");
    d.outbox.send_forward("draft-2", "thr-1").unwrap();
    d.clock.set(AFTER_WINDOW);
    d.outbox.tick().unwrap();

    let h = headers(&smtp.received()[0].data);
    assert_eq!(get(&h, "references"), vec!["<a-1@linea-example.com> <b-2@linea-example.com> <c-3@linea-example.com>".to_string()]);
    assert!(get(&h, "in-reply-to").is_empty(), "{h:?}");
}

#[test]
fn a_new_message_asks_the_server_for_nothing_and_carries_no_threading_headers() {
    let smtp = FakeSmtp::start(Mode::Accept);
    let d = device(&smtp);
    d.put_draft("draft-1", "Hello.");
    d.outbox.send_draft("draft-1").unwrap();
    d.clock.set(AFTER_WINDOW);
    d.outbox.tick().unwrap();
    let h = headers(&smtp.received()[0].data);
    assert!(get(&h, "in-reply-to").is_empty() && get(&h, "references").is_empty(), "{h:?}");
    assert!(d.parents.asked.lock().unwrap().is_empty());
}

#[test]
fn the_headers_are_fixed_at_queue_time_so_a_reply_sent_offline_keeps_them() {
    let smtp = FakeSmtp::start(Mode::Accept);
    let d = device(&smtp);
    d.put_parent("thr-1", "INBOX", 42, parent_headers());
    d.put_draft_in("draft-1", "Yes.", Some("thr-1"));
    d.outbox.send_draft("draft-1").unwrap();
    d.net.set(false);
    *d.parents.answer.lock().unwrap() = Some(Err(CoreError::Unexpected { reason_words: "no connection".into() }));
    d.clock.set(AFTER_WINDOW);
    d.outbox.tick().unwrap();
    d.net.set(true);
    d.outbox.tick().unwrap();
    let h = headers(&smtp.received()[0].data);
    assert_eq!(get(&h, "in-reply-to"), vec!["<c-3@linea-example.com>".to_string()], "no second ask of the server was needed");
}

#[test]
fn a_reply_whose_parent_cannot_be_read_is_not_queued_and_the_draft_stays() {
    let smtp = FakeSmtp::start(Mode::Accept);
    let d = device(&smtp);
    d.put_parent("thr-1", "INBOX", 42, parent_headers());
    *d.parents.answer.lock().unwrap() = Some(Err(CoreError::Action {
        failure: contracts::ActionFailed { account_name: "Nordlicht".into(), reason_words: "the server is not reachable".into(), retryable: true, code: None },
    }));
    d.put_draft_in("draft-1", "Yes.", Some("thr-1"));
    let err = d.outbox.send_draft("draft-1").expect_err("refused");
    assert!(err.to_string().contains("cannot be threaded") && err.to_string().contains("not reachable"), "{err}");
    assert_eq!(d.outbox.open_entries().unwrap().len(), 0);
    assert_eq!(d.draft_rows(), 1, "the draft is still in the composer");
}

#[test]
fn a_gmail_message_is_found_by_its_label_folder_and_uid_not_by_its_gmail_id() {
    let smtp = FakeSmtp::start(Mode::Accept);
    let d = device(&smtp);
    d.put_parent("thr-g", "[Gmail]/All Mail", 555_123_456, parent_headers()); // remote_id would be the X-GM-MSGID (a decimal number, not a UID)
    d.store
        .write(|ctx| {
            ctx.tx.execute("INSERT INTO message_location (account_id, folder, uid_validity, uid, message_id) VALUES (?1, 'INBOX', 7, 99, 'thr-g-m1')", [ACCOUNT_ID])?;
            Ok(())
        })
        .unwrap();
    d.put_draft_in("draft-1", "Yes.", Some("thr-g"));
    d.outbox.send_draft("draft-1").unwrap();
    let asked = d.parents.asked.lock().unwrap().clone();
    assert_eq!((asked[0].folder.as_str(), asked[0].remote_id.as_str()), ("INBOX", "99"), "{asked:?}");
}

// ------------------------------------------------------------------------------------ piece 3: the Sent copy

#[test]
fn a_refused_sent_copy_leaves_the_message_sent_and_says_why_in_words() {
    let smtp = FakeSmtp::start(Mode::Accept);
    let d = device(&smtp);
    d.sent.refuse.store(true, Ordering::SeqCst);
    d.put_draft("draft-1", "x");
    let id = d.outbox.send_draft("draft-1").unwrap().entry.id;
    d.clock.set(AFTER_WINDOW);
    let report = d.outbox.tick().unwrap();
    assert_eq!((report.delivered.len(), report.copies_stored), (1, 0));
    assert!(matches!(d.state(&id), OutboxState::Sent { .. }), "a failed copy never un-sends the message");
    let view = d.outbox.get(&id).unwrap().unwrap();
    let words = view.copy_not_saved.expect("recorded as sent, copy not saved");
    assert!(words.starts_with("Sent, copy not saved") && words.contains("Sent folder is full"), "{words}");
    assert_eq!(smtp.received().len(), 1);
}

#[test]
fn the_unsaved_copy_survives_a_restart_and_is_saved_once_without_a_second_delivery() {
    let smtp = FakeSmtp::start(Mode::Accept);
    let mut d = device(&smtp);
    d.sent.refuse.store(true, Ordering::SeqCst);
    d.put_draft("draft-1", "x");
    let id = d.outbox.send_draft("draft-1").unwrap().entry.id;
    d.clock.set(AFTER_WINDOW);
    d.outbox.tick().unwrap();
    let delivered_bytes = smtp.received()[0].data.clone();

    d.restart(); // the app closes and opens again
    d.sent.refuse.store(false, Ordering::SeqCst);
    d.clock.advance(30_000);
    assert_eq!(d.outbox.tick().unwrap().copies_stored, 1);
    assert_eq!(d.outbox.get(&id).unwrap().unwrap().copy_not_saved, None, "the note is gone once the copy is saved");
    let appended = d.sent.appended.lock().unwrap().clone();
    assert_eq!(appended.len(), 1);
    assert_eq!(String::from_utf8(appended[0].rfc822.clone()).unwrap().replace("\r\n", "\n"), delivered_bytes.replace("\r\n", "\n"), "the copy is the bytes that were delivered");
    d.clock.advance(30_000);
    d.outbox.tick().unwrap();
    assert_eq!((smtp.received().len(), d.sent.count()), (1, 1));
}

#[test]
fn the_unsaved_copy_does_not_travel_to_the_other_devices() {
    let smtp = FakeSmtp::start(Mode::Accept);
    let cloud = FakeCloud::new();
    let d = Device::new("mac", DeviceRole::Primary, T0, smtp.port, Some(cloud.clone()));
    d.sent.refuse.store(true, Ordering::SeqCst);
    d.put_draft("draft-1", "x");
    let at = EIGHT + 5 * MINUTE;
    d.outbox.schedule_draft("draft-1", at).unwrap();
    d.clock.set(at);
    d.outbox.tick().unwrap();
    let log = cloud.log.lock().unwrap();
    assert!(!log.is_empty());
    assert!(log.iter().all(|c| !String::from_utf8_lossy(&c.payload).contains("rfc822_hex")), "two devices saving the copy would put two copies in Sent");

    // The row written for the other devices never holds the copy, whatever record it is made from; the local row does.
    let mut record = Record::scheduled("s-1".into(), d.put_draft("draft-2", "y"), T0, at, "k".into(), None);
    record.copy_not_saved(CopyPending { words: "Sent, copy not saved: full".into(), rfc822: b"Subject: y\r\n\r\nbody".to_vec() }, T0);
    assert!(to_row(&record).draft_json.contains("rfc822_hex"));
    assert!(!to_wire_row(&record).draft_json.contains("rfc822_hex"));
}

#[test]
fn a_record_written_before_keys_existed_answers_to_its_own_id() {
    let smtp = FakeSmtp::start(Mode::Accept);
    let d = device(&smtp);
    let draft = d.put_draft("draft-1", "x");
    let mut record = Record::undoable("old-1".into(), draft, T0, 10_000, "old-1".into(), None);
    d.save_raw(&record);
    d.store.write(|ctx| { ctx.tx.execute("UPDATE outbox_entry SET draft_json = json_remove(draft_json, '$.idempotency_key') WHERE id = 'old-1'", [])?; Ok(()) }).unwrap();
    record = GetEntryPort::execute(&*d.rows, "old-1").unwrap().unwrap();
    assert_eq!(record.idempotency_key, "old-1");
    assert_eq!(FindByKeyPort::execute(&*d.rows, "old-1").unwrap().len(), 1);
}
