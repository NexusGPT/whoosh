//! fix-realcore round 15 (owner, main115: one "Connection reset by peer", attempts 1, never tried again). A connection that breaks before
//! the end of the body is tried again by the outbox itself, after SEND_RETRY_START_S, doubling, at most SEND_RETRY_PAUSE_MAX_S, and
//! SEND_RETRY_TRIES_MAX tries in all (docs/CONSTANTS.md); then it is failed. A break after the body is never tried again (the server may
//! hold the mail). The real sender (send::LettreDelivery) against the staged loopback SMTP server (send/tests/staged/mod.rs).
//! Broken copies (one edit each, red first):
//!   R1 usecases/run_due.rs: `record.waiting || record.retry_at_ms.is_some_and(|t| now >= t)` -> `record.waiting`
//!                     -> a_reset_on_the_first_try_is_sent_on_the_second_by_itself, a_retrying_entry_keeps_its_next_time_across_a_restart,
//!                        a_server_that_keeps_resetting_is_tried_four_times_then_failed
//!   R2 usecases/deliver_entry.rs `transient_break`: `&& c != AFTER_DATA_CODE` deleted
//!                                                     -> a_break_after_the_body_is_never_tried_again
//!   R3 domain/record.rs `retry_pause_ms`: `>= SEND_RETRY_TRIES_MAX` -> `>= SEND_RETRY_TRIES_MAX + 1`
//!                                                     -> a_server_that_keeps_resetting_is_tried_four_times_then_failed
//!   R4 usecases/run_due.rs: `now >= t` -> `now + 1 >= t` (a try one millisecond early) -> a_reset_on_the_first_try_is_sent_on_the_second_by_itself
mod support;
#[path = "../../send/tests/staged/mod.rs"]
mod staged;

use contracts::constants::{SEND_RETRY_PAUSE_MAX_S, SEND_RETRY_START_S, SEND_RETRY_TRIES_MAX};
use contracts::OutboxState;
use outbox::DeviceRole;
use staged::{BreakAt, How, StagedSmtp};
use support::*;

const T0: i64 = EIGHT - 3 * 60 * MINUTE;
const AFTER_WINDOW: i64 = T0 + 60_000;

fn device(port: u16) -> Device {
    Device::new("mac", DeviceRole::Primary, T0, port, None)
}

/// (state name, attempts, next try) of the stored entry.
fn look(d: &Device, id: &str) -> (String, u32, Option<i64>, Option<String>) {
    let v = d.outbox.get(id).unwrap().expect("the entry");
    let (name, code) = match &v.entry.state {
        OutboxState::Failed { failure, .. } => ("failed".to_string(), failure.code.clone()),
        OutboxState::Sent { .. } => ("sent".to_string(), None),
        other => (format!("{other:?}"), None),
    };
    (name, v.attempts, v.retry_at_ms, code)
}

#[test]
fn a_reset_on_the_first_try_is_sent_on_the_second_by_itself() {
    let smtp = StagedSmtp::start(BreakAt::MailFrom, How::Reset, 1);
    let d = device(smtp.port);
    d.put_draft("draft-1", "Yes, 12,000 units works.");
    let id = d.outbox.send_draft("draft-1").unwrap().entry.id;
    d.clock.set(AFTER_WINDOW);
    let r = d.outbox.tick().unwrap();
    assert_eq!(r.failed, vec![id.clone()]);
    let first_try = d.clock.now_ms_for_test();
    let pause = SEND_RETRY_START_S as i64 * 1000;
    assert_eq!(look(&d, &id), ("failed".into(), 1, Some(first_try + pause), Some("network_mail_from".into())), "retrying, with its next time");
    // one millisecond before the next try: nothing leaves
    d.clock.set(first_try + pause - 1);
    d.outbox.tick().unwrap();
    assert_eq!(smtp.connections.load(std::sync::atomic::Ordering::SeqCst), 1, "no try before its time");
    d.clock.set(first_try + pause);
    let r = d.outbox.tick().unwrap();
    assert_eq!(r.delivered, vec![id.clone()]);
    assert_eq!(smtp.received().len(), 1, "one mail at the server");
    let (state, attempts, next, _) = look(&d, &id);
    assert_eq!((state.as_str(), attempts, next), ("sent", 2, None));
    // nothing more leaves later
    d.clock.advance(60 * MINUTE);
    d.outbox.tick().unwrap();
    assert_eq!(smtp.received().len(), 1);
    assert_eq!(smtp.connections.load(std::sync::atomic::Ordering::SeqCst), 2);
    println!("PASS reset then sent: attempts 2, one mail, {} connections", smtp.connections.load(std::sync::atomic::Ordering::SeqCst));
}

#[test]
fn a_server_that_keeps_resetting_is_tried_four_times_then_failed() {
    let smtp = StagedSmtp::start(BreakAt::RcptTo, How::Reset, 1000);
    let d = device(smtp.port);
    d.put_draft("draft-1", "x");
    let id = d.outbox.send_draft("draft-1").unwrap().entry.id;
    d.clock.set(AFTER_WINDOW);
    let mut pauses = Vec::new();
    for n in 1..=SEND_RETRY_TRIES_MAX as u32 {
        d.outbox.tick().unwrap();
        let (state, attempts, next, code) = look(&d, &id);
        assert_eq!((state.as_str(), attempts, code.as_deref()), ("failed", n, Some("network_rcpt_to")), "try {n}");
        match next {
            Some(t) => { pauses.push(t - d.clock.now_ms_for_test()); d.clock.set(t); }
            None => { assert_eq!(n, SEND_RETRY_TRIES_MAX as u32, "only the last try leaves no next time"); }
        }
    }
    let want: Vec<i64> = (0..SEND_RETRY_TRIES_MAX - 1).map(|k| (SEND_RETRY_START_S << k).min(SEND_RETRY_PAUSE_MAX_S) as i64 * 1000).collect();
    assert_eq!(pauses, want, "the pauses double from SEND_RETRY_START_S");
    d.clock.advance(24 * 60 * MINUTE);
    d.outbox.tick().unwrap();
    assert_eq!(smtp.connections.load(std::sync::atomic::Ordering::SeqCst), SEND_RETRY_TRIES_MAX as usize, "no try after the last");
    assert_eq!(smtp.received().len(), 0);
    println!("PASS {} tries, pauses {:?} ms, then failed", SEND_RETRY_TRIES_MAX, pauses);
}

#[test]
fn a_break_after_the_body_is_never_tried_again() {
    let smtp = StagedSmtp::start(BreakAt::AfterData, How::Reset, 1000);
    let d = device(smtp.port);
    d.put_draft("draft-1", "x");
    let id = d.outbox.send_draft("draft-1").unwrap().entry.id;
    d.clock.set(AFTER_WINDOW);
    d.outbox.tick().unwrap();
    let (state, attempts, next, code) = look(&d, &id);
    assert_eq!((state.as_str(), attempts, next, code.as_deref()), ("failed", 1, None, Some("network_after_data")));
    for _ in 0..5 {
        d.clock.advance(30 * MINUTE);
        d.outbox.tick().unwrap();
    }
    assert_eq!(smtp.connections.load(std::sync::atomic::Ordering::SeqCst), 1, "never tried again by itself");
    assert_eq!(smtp.received().len(), 1, "the server holds one copy");
    println!("PASS after-data: one connection, one copy, failed with no next try");
}

/// Lead, 13 Nov (owner picture 208): the server ANSWERS "451 … try again later". The outbox tries again by itself on the same schedule.
/// Broken copy: usecases/deliver_entry.rs `transient_break(&failure) || transient_answer(&failure)` -> `transient_break(&failure)`.
#[test]
fn a_try_later_answer_is_sent_on_the_second_try_by_itself() {
    let smtp = StagedSmtp::start(BreakAt::MailFrom, How::TryLater, 1);
    let d = device(smtp.port);
    d.put_draft("draft-1", "Yes, 12,000 units works.");
    let id = d.outbox.send_draft("draft-1").unwrap().entry.id;
    d.clock.set(AFTER_WINDOW);
    d.outbox.tick().unwrap();
    let first_try = d.clock.now_ms_for_test();
    let pause = SEND_RETRY_START_S as i64 * 1000;
    let (state, attempts, next, code) = look(&d, &id);
    assert_eq!((state.as_str(), attempts, next), ("failed", 1, Some(first_try + pause)), "retrying, with its next time");
    assert!(code.as_deref().is_some_and(|c| c.starts_with("451")), "the server's own code: {code:?}");
    d.clock.set(first_try + pause);
    d.outbox.tick().unwrap();
    let (state, attempts, next, _) = look(&d, &id);
    assert_eq!((state.as_str(), attempts, next), ("sent", 2, None));
    assert_eq!(smtp.received().len(), 1, "one mail at the server");
}

#[test]
fn a_server_refusal_is_not_tried_again_by_itself() {
    let smtp = FakeSmtp::start(Mode::Refuse);
    let d = device(smtp.port);
    d.put_draft("draft-1", "x");
    let id = d.outbox.send_draft("draft-1").unwrap().entry.id;
    d.clock.set(AFTER_WINDOW);
    d.outbox.tick().unwrap();
    let (state, attempts, next, _) = look(&d, &id);
    assert_eq!((state.as_str(), attempts, next), ("failed", 1, None));
    d.clock.advance(60 * MINUTE);
    d.outbox.tick().unwrap();
    assert_eq!(smtp.connections.load(std::sync::atomic::Ordering::SeqCst), 1);
}

#[test]
fn a_retrying_entry_keeps_its_next_time_across_a_restart() {
    let smtp = StagedSmtp::start(BreakAt::Data, How::Close, 1);
    let mut d = device(smtp.port);
    d.put_draft("draft-1", "x");
    let id = d.outbox.send_draft("draft-1").unwrap().entry.id;
    d.clock.set(AFTER_WINDOW);
    d.outbox.tick().unwrap();
    let before = look(&d, &id);
    assert_eq!(before.3.as_deref(), Some("network_data"));
    d.restart();
    assert_eq!(look(&d, &id), before, "the next time is stored in the row");
    d.clock.set(before.2.unwrap());
    assert_eq!(d.outbox.tick().unwrap().delivered, vec![id.clone()]);
    assert_eq!(smtp.received().len(), 1);
}

trait NowForTest { fn now_ms_for_test(&self) -> i64; }
impl NowForTest for std::sync::Arc<TestClock> {
    fn now_ms_for_test(&self) -> i64 { contracts::Clock::now_ms(&**self) }
}
