//! fn-send-smtp against a local SMTP server the test starts and stops itself.
mod support;

use contracts::{ActionFailed, CoreError, Draft, Signature};
use send::{LettreDelivery, Security, SendAccount, SendInput, SendMessage, SentCopy, SmtpCredentials, SmtpSettings};
use std::sync::atomic::Ordering;
use std::sync::Arc;
use support::{decoded_body, header, FakeSmtp, FixedClock, Mode, StubSent, REFUSAL_TEXT};

const ADDRESS: &str = "assem@northlight-example.de";
const SIGNATURE: &str = "Assem Chammah | Nordlicht";
const NOW_MS: i64 = 1_794_000_000_000;

fn account(port: u16) -> SendAccount {
    SendAccount {
        id: "acc-nordlicht".into(),
        name: "Nordlicht".into(),
        address: ADDRESS.into(),
        smtp: SmtpSettings { host: "127.0.0.1".into(), port, security: Security::Plaintext, credentials: None },
    }
}

fn draft(body: &str) -> Draft {
    Draft {
        id: "draft-01".into(),
        from_account_id: "acc-nordlicht".into(),
        to: vec!["Linea Supply <orders@linea-example.com>".into()],
        cc: vec![],
        subject: "Re: Quote for the Q1 print run".into(),
        body: body.into(),
        attachment_ids: vec![],
        ai_marks: vec![],
        reply_to_thread_id: None,
        bcc: vec![],
    }
}

fn signature() -> Option<Signature> {
    Some(Signature { address: ADDRESS.into(), text: SIGNATURE.into() })
}

fn use_case(sent: Arc<StubSent>) -> SendMessage {
    SendMessage::new(Arc::new(LettreDelivery), sent, Arc::new(FixedClock(NOW_MS)))
}

fn input(port: u16, body: &str) -> SendInput {
    SendInput { account: account(port), draft: draft(body), signature: signature(), reply: None, files: vec![] }
}

#[test]
fn one_delivery_signature_once_one_sent_copy_from_the_account_address() {
    let server = FakeSmtp::start(Mode::Accept);
    let sent = StubSent::ok();
    let receipt = use_case(sent.clone()).execute(&input(server.port, "Yes, 12,000 units works.\nThanks.")).expect("send");

    let got = server.received();
    assert_eq!(got.len(), 1, "one delivery per send");
    let m = &got[0];
    assert_eq!(m.mail_from, ADDRESS, "envelope sender is the account address");
    assert_eq!(m.rcpt_to, vec!["orders@linea-example.com".to_string()]);
    assert!(header(&m.data, "From").expect("From").contains(ADDRESS), "From header is the account address");
    assert_eq!(header(&m.data, "Message-ID"), Some("<draft-01@northlight-example.de>"));
    assert_eq!(decoded_body(&m.data).matches(SIGNATURE).count(), 1, "the signature is present exactly once");

    assert_eq!(sent.count(), 1, "the Sent folder gains exactly one copy");
    let copy = sent.appended.lock().unwrap();
    assert_eq!(copy[0].account_id, "acc-nordlicht");
    assert_eq!(String::from_utf8(copy[0].rfc822.clone()).unwrap().replace("\r\n", "\n"), m.data.replace("\r\n", "\n"), "the Sent copy is the delivered message");
    assert_eq!(receipt.sent_copy, SentCopy::Stored);
    assert_eq!(receipt.sent_at_ms, NOW_MS);
}

#[test]
fn a_body_that_already_ends_with_the_signature_block_still_carries_it_once() {
    let server = FakeSmtp::start(Mode::Accept);
    let body = format!("Yes.\n\n-- \n{SIGNATURE}");
    use_case(StubSent::ok()).execute(&input(server.port, &body)).expect("send");
    let got = server.received();
    assert_eq!(decoded_body(&got[0].data).matches(SIGNATURE).count(), 1);
}

#[test]
fn a_non_ascii_signature_and_body_arrive_intact() {
    let server = FakeSmtp::start(Mode::Accept);
    let mut i = input(server.port, "Grüße aus Köln — wir bestätigen die Menge.");
    i.signature = Some(Signature { address: ADDRESS.into(), text: "Assem Chammah · Nordlicht".into() });
    use_case(StubSent::ok()).execute(&i).expect("send");
    let body = decoded_body(&server.received()[0].data);
    assert_eq!(body.matches("Assem Chammah · Nordlicht").count(), 1);
    assert!(body.contains("Grüße aus Köln"));
}

#[test]
fn cc_recipients_are_in_the_envelope_and_the_header() {
    let server = FakeSmtp::start(Mode::Accept);
    let mut i = input(server.port, "Hi");
    i.draft.cc = vec!["tom@verheyden-example.be".into()];
    use_case(StubSent::ok()).execute(&i).expect("send");
    let got = server.received();
    assert_eq!(got[0].rcpt_to.len(), 2);
    assert!(got[0].rcpt_to.contains(&"tom@verheyden-example.be".to_string()));
    assert!(header(&got[0].data, "Cc").expect("Cc").contains("tom@verheyden-example.be"));
}

#[test]
fn bcc_recipients_are_in_the_envelope_and_never_in_a_header() {
    let server = FakeSmtp::start(Mode::Accept);
    let mut i = input(server.port, "Hi");
    i.draft.bcc = vec!["boss@nordlicht-example.de".into()];
    use_case(StubSent::ok()).execute(&i).expect("send");
    let got = server.received();
    assert!(got[0].rcpt_to.contains(&"boss@nordlicht-example.de".to_string()), "the Bcc recipient receives it");
    assert!(header(&got[0].data, "Bcc").is_none(), "no Bcc header leaves");
    assert!(!got[0].data.contains("boss@nordlicht-example.de"), "the address is nowhere in the message");
}

#[test]
fn a_refusing_server_fails_with_its_real_words_and_nothing_is_appended() {
    let server = FakeSmtp::start(Mode::Refuse);
    let sent = StubSent::ok();
    let err = use_case(sent.clone()).execute(&input(server.port, "Hi")).expect_err("must fail");
    match err {
        CoreError::Action { failure: ActionFailed { account_name, reason_words, retryable, code } } => {
            assert_eq!(account_name, "Nordlicht");
            assert_eq!(code.as_deref(), Some("550 5.7.1"), "delta 12: the reply code and the enhanced status beside the words");
            assert!(reason_words.contains("550"), "the real status is kept: {reason_words}");
            assert!(reason_words.contains(REFUSAL_TEXT), "the real body is kept: {reason_words}");
            assert!(!retryable, "a permanent refusal is not retryable");
        }
        other => panic!("wrong error: {other:?}"),
    }
    assert_eq!(server.received().len(), 0);
    assert_eq!(sent.count(), 0, "no delivery, no Sent copy");
}

#[test]
fn a_failed_append_after_delivery_is_reported_pending_and_nothing_is_sent_twice() {
    let server = FakeSmtp::start(Mode::Accept);
    let sent = StubSent::failing("IMAP connection reset");
    let receipt = use_case(sent).execute(&input(server.port, "Hi")).expect("delivery succeeded");
    assert_eq!(server.received().len(), 1, "delivered once");
    match receipt.sent_copy {
        SentCopy::Pending { failure, rfc822 } => {
            assert!(failure.reason_words.contains("IMAP connection reset"));
            assert!(failure.retryable);
            assert!(!rfc822.is_empty(), "the bytes to retry the append with");
        }
        SentCopy::Stored => panic!("must be pending"),
    }
}

#[test]
fn a_draft_of_another_account_or_a_foreign_signature_is_refused_before_any_connection() {
    let server = FakeSmtp::start(Mode::Accept);
    let sent = StubSent::ok();
    let uc = use_case(sent.clone());

    let mut other = input(server.port, "Hi");
    other.draft.from_account_id = "acc-work".into();
    assert!(matches!(uc.execute(&other), Err(CoreError::Invalid { .. })));

    let mut foreign = input(server.port, "Hi");
    foreign.signature = Some(Signature { address: "assem@nexus-example.io".into(), text: "Other".into() });
    assert!(matches!(uc.execute(&foreign), Err(CoreError::Invalid { .. })));

    assert_eq!(server.connections.load(Ordering::SeqCst), 0);
    assert_eq!(sent.count(), 0);
}

#[test]
fn attachments_are_refused_not_silently_dropped() {
    let server = FakeSmtp::start(Mode::Accept);
    let mut i = input(server.port, "Hi");
    i.draft.attachment_ids = vec!["att-1".into()];
    assert!(matches!(use_case(StubSent::ok()).execute(&i), Err(CoreError::Invalid { field, .. }) if field == "draft.attachment_ids"));
    assert_eq!(server.connections.load(Ordering::SeqCst), 0);
}

#[test]
fn plain_smtp_is_refused_for_any_host_but_the_loopback() {
    let mut i = input(25, "Hi");
    i.account.smtp.host = "smtp.mail-example.com".into();
    let err = use_case(StubSent::ok()).execute(&i).expect_err("refused");
    assert!(matches!(err, CoreError::Invalid { field, .. } if field == "smtp.security"));
}

#[test]
fn the_secret_never_prints() {
    let c = SmtpCredentials { username: "assem".into(), secret: "hunter2-fixture".into(), bearer: false };
    assert!(!format!("{c:?}").contains("hunter2"));
}

#[test]
fn an_oauth2_token_goes_by_xoauth2_and_never_by_plain() {
    use base64::Engine as _;
    let server = FakeSmtp::start(Mode::Xoauth2);
    let mut i = input(server.port, "Hi");
    i.account.smtp.credentials = Some(SmtpCredentials { username: "assem@northlight-example.de".into(), secret: "at-fixture".into(), bearer: true });
    use_case(StubSent::ok()).execute(&i).expect("send");
    let auths = server.auths.lock().unwrap().clone();
    assert_eq!(auths.len(), 1, "{auths:?}");
    let b64 = auths[0].strip_prefix("AUTH XOAUTH2 ").expect("XOAUTH2, not PLAIN or LOGIN");
    let raw = base64::engine::general_purpose::STANDARD.decode(b64).unwrap();
    assert_eq!(String::from_utf8(raw).unwrap(), contracts::xoauth2_sasl("assem@northlight-example.de", "at-fixture"));
    assert_eq!(server.received().len(), 1);
}
