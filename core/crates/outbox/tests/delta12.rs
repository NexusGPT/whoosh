//! contracts delta 12 hook: Command::Send carries the draft's Bcc out, and a refusal keeps the server's code.
mod support;

use contracts::OutboxState;
use support::*;

const T0: i64 = EIGHT - 3 * 60 * MINUTE;
const AFTER_WINDOW: i64 = T0 + 60_000;

fn device(smtp: &FakeSmtp) -> Device {
    Device::new("mac", outbox::DeviceRole::Primary, T0, smtp.port, None)
}

#[test]
fn a_sent_draft_takes_its_bcc_to_the_envelope_only() {
    let smtp = FakeSmtp::start(Mode::Accept);
    let d = device(&smtp);
    d.put_draft("draft-1", "Yes, 12,000 units works.");
    d.store.write(|c| Ok(c.tx.execute("UPDATE draft SET bcc_json = '[\"boss@northlight-example.de\"]' WHERE id = 'draft-1'", [])?)).unwrap();
    let id = d.outbox.send_draft("draft-1").unwrap().entry.id;
    d.clock.set(AFTER_WINDOW);
    assert_eq!(d.outbox.tick().unwrap().delivered, vec![id]);
    let got = smtp.received();
    assert!(got[0].rcpt_to.contains(&"boss@northlight-example.de".to_string()), "the Bcc recipient receives it: {:?}", got[0].rcpt_to);
    assert!(!got[0].data.contains("boss@northlight-example.de"), "no header names the Bcc recipient");
}

#[test]
fn a_refusal_keeps_the_servers_code_beside_its_words() {
    let smtp = FakeSmtp::start(Mode::Refuse);
    let d = device(&smtp);
    d.put_draft("draft-1", "x");
    let id = d.outbox.send_draft("draft-1").unwrap().entry.id;
    d.clock.set(AFTER_WINDOW);
    d.outbox.tick().unwrap();
    match d.state(&id) {
        OutboxState::Failed { failure, .. } => {
            assert_eq!(failure.code.as_deref(), Some("550 5.7.1"), "{failure:?}");
            assert!(failure.reason_words.contains(REFUSAL_TEXT));
        }
        other => panic!("expected Failed, got {other:?}"),
    }
}
