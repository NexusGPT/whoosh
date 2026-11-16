//! fn-outbox-crossdevice: two in-process cores sharing a fake sync store. The iPhone schedules and sleeps;
//! the Mac sends at the time, exactly once. (The simulator half is reported blocked by the platform.)
mod support;

use contracts::OutboxState;
use outbox::DeviceRole;
use std::sync::atomic::Ordering;
use support::*;

const NIGHT: i64 = EIGHT - 9 * 60 * MINUTE;

struct Pair {
    smtp: FakeSmtp,
    cloud: std::sync::Arc<FakeCloud>,
    phone: Device,
    mac: Device,
}

fn pair(mac_role: DeviceRole) -> Pair {
    let smtp = FakeSmtp::start(Mode::Accept);
    let cloud = FakeCloud::new();
    let phone = Device::new("phone", DeviceRole::Primary, NIGHT, smtp.port, Some(cloud.clone()));
    let mac = Device::new("mac", mac_role, NIGHT, smtp.port, Some(cloud.clone()));
    Pair { smtp, cloud, phone, mac }
}

#[test]
fn scheduled_on_the_phone_leaves_from_the_mac_exactly_once_when_the_phone_sleeps() {
    let p = pair(DeviceRole::Primary);
    p.phone.put_draft("draft-1", "Good morning, the figures are attached.");
    let id = p.phone.outbox.schedule_draft("draft-1", EIGHT).unwrap().entry.id;
    assert_eq!(p.cloud.len(), 1, "the scheduled entry went to the sync store");

    // the phone sleeps from here on: it never ticks again
    p.mac.clock.set(NIGHT + 60 * MINUTE);
    assert_eq!(p.mac.outbox.tick().unwrap().pulled, 1, "the Mac took it in");
    assert!(matches!(p.mac.state(&id), OutboxState::Scheduled { at_ms } if at_ms == EIGHT));
    assert_eq!(p.smtp.received().len(), 0);

    p.mac.clock.set(EIGHT);
    assert_eq!(p.mac.outbox.tick().unwrap().delivered, vec![id.clone()]);
    assert_eq!(p.smtp.received().len(), 1, "the Mac sent it at 08:00");
    assert_eq!(p.mac.sent.count(), 1);

    // the phone wakes later and learns it is sent, and does not send it again
    p.phone.clock.set(EIGHT + 90 * MINUTE);
    let report = p.phone.outbox.tick().unwrap();
    assert!(report.delivered.is_empty(), "the phone must not send what the Mac sent");
    assert!(matches!(p.phone.state(&id), OutboxState::Sent { .. }));
    assert_eq!(p.smtp.received().len(), 1, "exactly once across both devices");
    p.mac.clock.advance(MINUTE);
    p.mac.outbox.tick().unwrap();
    assert_eq!(p.smtp.received().len(), 1);
}

#[test]
fn a_standby_phone_covers_for_a_sleeping_mac_but_only_after_the_grace_time() {
    let p = pair(DeviceRole::Primary);
    // roles swapped for this test: the phone is the standby
    let phone = Device::new("phone2", DeviceRole::Standby { after_ms: STANDBY_AFTER_MS }, NIGHT, p.smtp.port, Some(p.cloud.clone()));
    p.mac.put_draft("draft-1", "x");
    let id = p.mac.outbox.schedule_draft("draft-1", EIGHT).unwrap().entry.id;
    phone.clock.set(EIGHT + 1);
    phone.outbox.tick().unwrap();
    phone.clock.set(EIGHT + STANDBY_AFTER_MS - 1);
    assert!(phone.outbox.tick().unwrap().delivered.is_empty(), "the standby waits for the primary");
    assert_eq!(p.smtp.received().len(), 0);

    phone.clock.set(EIGHT + STANDBY_AFTER_MS);
    assert_eq!(phone.outbox.tick().unwrap().delivered, vec![id], "the primary stayed asleep, the standby sends");
    assert_eq!(p.smtp.received().len(), 1);
}

#[test]
fn a_standby_that_sees_the_primary_send_first_stays_quiet() {
    let p = pair(DeviceRole::Primary);
    let phone = Device::new("phone2", DeviceRole::Standby { after_ms: STANDBY_AFTER_MS }, NIGHT, p.smtp.port, Some(p.cloud.clone()));
    p.mac.put_draft("draft-1", "x");
    p.mac.outbox.schedule_draft("draft-1", EIGHT).unwrap();
    phone.clock.set(EIGHT);
    phone.outbox.tick().unwrap();

    p.mac.clock.set(EIGHT);
    p.mac.outbox.tick().unwrap();
    assert_eq!(p.smtp.received().len(), 1);

    phone.clock.set(EIGHT + STANDBY_AFTER_MS + MINUTE);
    let report = phone.outbox.tick().unwrap();
    assert!(report.delivered.is_empty());
    assert_eq!(report.pulled, 2, "it took in the Mac's Sending and Sent records");
    assert_eq!(p.smtp.received().len(), 1, "no second send from the standby");
}

#[test]
fn a_cancel_on_one_device_stops_the_other() {
    let p = pair(DeviceRole::Primary);
    p.phone.put_draft("draft-1", "x");
    let id = p.phone.outbox.schedule_draft("draft-1", EIGHT).unwrap().entry.id;
    p.mac.outbox.tick().unwrap();
    p.phone.clock.advance(MINUTE);
    p.phone.outbox.cancel(&id).unwrap();
    p.mac.clock.set(EIGHT);
    let report = p.mac.outbox.tick().unwrap();
    assert!(report.delivered.is_empty(), "the cancel arrived before the time");
    assert_eq!(p.smtp.received().len(), 0);
}

#[test]
fn a_late_cancel_does_not_undo_a_message_that_already_left() {
    let p = pair(DeviceRole::Primary);
    p.phone.put_draft("draft-1", "x");
    let id = p.phone.outbox.schedule_draft("draft-1", EIGHT).unwrap().entry.id;
    p.mac.outbox.tick().unwrap();
    p.mac.clock.set(EIGHT);
    p.mac.outbox.tick().unwrap();
    assert_eq!(p.smtp.received().len(), 1);

    // the phone, not yet synced, cancels at 08:00:30 by its own clock (a cancel racing the send)
    p.phone.clock.set(EIGHT + 30_000);
    p.phone.outbox.cancel(&id).unwrap();
    p.mac.outbox.tick().unwrap();
    assert!(matches!(p.mac.state(&id), OutboxState::Sent { .. }), "Sent is further along than Cancelled and wins the merge");
    p.phone.outbox.tick().unwrap();
    assert!(matches!(p.phone.state(&id), OutboxState::Sent { .. }), "the phone learns the truth");
    assert_eq!(p.smtp.received().len(), 1);
}

#[test]
fn an_unreachable_sync_store_never_blocks_sending_and_the_record_goes_out_later() {
    let p = pair(DeviceRole::Primary);
    p.cloud.down.store(true, Ordering::SeqCst);
    p.phone.put_draft("draft-1", "x");
    let id = p.phone.outbox.schedule_draft("draft-1", EIGHT).expect("scheduling works with sync down").entry.id;
    assert_eq!(p.cloud.len(), 0);

    p.phone.clock.set(NIGHT + MINUTE);
    let report = p.phone.outbox.tick().unwrap();
    assert!(report.sync_error.is_some(), "the failure is reported in words, not hidden");
    p.cloud.down.store(false, Ordering::SeqCst);
    p.phone.clock.advance(MINUTE);
    p.phone.outbox.tick().unwrap();
    assert_eq!(p.cloud.len(), 1, "the unpublished record went out when the store came back");

    p.mac.outbox.tick().unwrap();
    p.mac.clock.set(EIGHT);
    assert_eq!(p.mac.outbox.tick().unwrap().delivered, vec![id]);
    assert_eq!(p.smtp.received().len(), 1);
}

#[test]
fn undoable_entries_do_not_travel() {
    let p = pair(DeviceRole::Primary);
    p.phone.put_draft("draft-1", "x");
    p.phone.outbox.send_draft("draft-1").unwrap();
    assert_eq!(p.cloud.len(), 0, "an undo window is seconds long and local");
    assert_eq!(p.mac.outbox.tick().unwrap().pulled, 0);
}
