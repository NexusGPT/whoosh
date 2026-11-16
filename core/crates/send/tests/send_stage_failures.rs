//! fix-realcore round 15 (owner, main115: "the mail server did not accept the message: network error: Connection reset by peer (os error 54)",
//! Microsoft account, smtp.office365.com:587 STARTTLS, XOAUTH2). The SMTP path is driven step by step (send/src/smtp.rs `execute`): a
//! connection that breaks (a reset or a close) at each step comes back with the code `network_<step>`, words that name the step, and
//! `retryable` true, except after the body was written (`network_after_data`: the server may hold the mail, so it is never retried).
//! A server that answers with a code stays a server answer (its code, never `network_`). Loopback only (send/tests/staged/mod.rs).
//! Broken copies (one edit each in send/src/smtp.rs, red first):
//!   B1 `is_network`: `!e.is_response() || e.to_string().contains(CLOSED_MID_ANSWER)` -> `!e.is_response()`
//!      (a close is then filed as a server answer)  -> a_close_at_each_step_is_a_broken_connection_named_by_its_step, a_break_after_the_body_is_never_retryable
//!   B2 `stage_failure`: `stage != Stage::AfterData` -> `true`   -> a_break_after_the_body_is_never_retryable and both step tests
//!   B3 `Stage::words`: `Stage::Auth => "AUTH"` -> `Stage::Auth => "the server"`   -> both step tests
//!   B4 `execute`: the RCPT TO `.map_err(|e| fail(&mut conn, Stage::RcptTo, e))` -> `Stage::MailFrom`
//!                                                                                  -> both step tests
//!   B5 `stage_failure`: the network branch returns `failed_with_code(.., !e.is_permanent(), None)` (no stage code)
//!                                     -> both step tests and a_whole_send_after_the_breaks_delivers_once_by_xoauth2 (a_server_answer_is_not_a_broken_connection stays green)
//! The twelve broken copies of round 15 were run by fix-realcore-out/r15-mutants.py: each KILLED (the tests named here red, the edit undone).
mod staged;
#[allow(dead_code, unused_imports)]
mod support;

use contracts::{ActionFailed, CoreError};
use send::{DeliverPort, DeliverRequest, Envelope, LettreDelivery, Security, SmtpCredentials, SmtpSettings};
use staged::{BreakAt, How, StagedSmtp};

const FROM: &str = "owner@contoso-example.test";
const TO: &str = "someone@example.test";

fn request(port: u16, security: Security, bearer: bool) -> DeliverRequest {
    let rfc822 = format!("From: {FROM}\r\nTo: {TO}\r\nSubject: Re: the quote\r\nMessage-ID: <stage-test@contoso-example.test>\r\n\r\nYes, that works.\r\n").into_bytes();
    DeliverRequest {
        account_name: "Microsoft".into(),
        smtp: SmtpSettings {
            host: "127.0.0.1".into(),
            port,
            security,
            // the owner's shape: a separate SMTP user name and an OAuth2 token by XOAUTH2
            credentials: Some(SmtpCredentials { username: "smtp-user@contoso-example.test".into(), secret: "at-fixture".into(), bearer }),
        },
        envelope: Envelope { from: FROM.into(), to: vec![TO.into()] },
        rfc822,
    }
}

/// The request for a step: STARTTLS is reached only on a STARTTLS account; every other step on the plain loopback connection.
fn send_through(at: BreakAt, how: How) -> (Result<(), CoreError>, StagedSmtp) {
    let server = StagedSmtp::start(at, how, 1);
    let security = if at == BreakAt::StartTls { Security::StartTls } else { Security::Plaintext };
    let r = LettreDelivery.execute(&request(server.port, security, true));
    (r, server)
}

fn failure(r: Result<(), CoreError>) -> ActionFailed {
    match r {
        Err(CoreError::Action { failure }) => failure,
        other => panic!("expected an action failure, got {other:?}"),
    }
}

fn step_words(at: BreakAt) -> &'static str {
    match at {
        BreakAt::Connect => "at the greeting",
        BreakAt::StartTls => "at STARTTLS",
        BreakAt::Auth => "at AUTH",
        BreakAt::MailFrom => "at MAIL FROM",
        BreakAt::RcptTo => "at RCPT TO",
        BreakAt::Data => "at DATA",
        BreakAt::AfterData => "at the end of the message",
    }
}

fn check_every_step(how: How) {
    let mut lines = Vec::new();
    for at in BreakAt::ALL {
        let (r, server) = send_through(at, how);
        let f = failure(r);
        let want_code = format!("network_{}", at.id());
        lines.push(format!("{how:?} {:<10} code={:?} retryable={} words={:?} log={:?}", at.id(), f.code, f.retryable, f.reason_words, server.log.lock().unwrap().clone()));
        assert_eq!(f.code.as_deref(), Some(want_code.as_str()), "{how:?} at {}: {f:?}", at.id());
        assert!(f.reason_words.contains(step_words(at)), "{how:?} at {}: the words name the step: {:?}", at.id(), f.reason_words);
        assert!(f.reason_words.starts_with("the connection to the mail server broke"), "{how:?} at {}: {:?}", at.id(), f.reason_words);
        assert_eq!(f.retryable, at != BreakAt::AfterData, "{how:?} at {}", at.id());
        assert_eq!(f.account_name, "Microsoft");
        assert_eq!(server.received().len(), usize::from(at == BreakAt::AfterData), "{how:?} at {}: the server kept the mail only once the body was written", at.id());
        assert!(!f.reason_words.contains("at-fixture"), "the token never reaches the words");
    }
    for l in &lines { println!("{l}"); }
}

#[test]
fn a_reset_at_each_step_is_a_broken_connection_named_by_its_step() {
    check_every_step(How::Reset);
}

#[test]
fn a_close_at_each_step_is_a_broken_connection_named_by_its_step() {
    check_every_step(How::Close);
}

#[test]
fn a_break_after_the_body_is_never_retryable() {
    for how in [How::Reset, How::Close] {
        let (r, server) = send_through(BreakAt::AfterData, how);
        let f = failure(r);
        assert!(!f.retryable, "{how:?}: the server may hold the mail; a second try could deliver it twice: {f:?}");
        assert_eq!(server.received().len(), 1);
    }
}

#[test]
fn the_reset_words_carry_the_systems_own_reason() {
    let (r, _server) = send_through(BreakAt::Auth, How::Reset);
    let f = failure(r);
    println!("reset at AUTH: {:?}", f.reason_words);
    assert!(f.reason_words.to_ascii_lowercase().contains("reset"), "a reset says so: {:?}", f.reason_words);
}

#[test]
fn a_whole_send_after_the_breaks_delivers_once_by_xoauth2() {
    // the first connection breaks at MAIL FROM, the second is served: one mail, sent by XOAUTH2 both times
    let server = StagedSmtp::start(BreakAt::MailFrom, How::Reset, 1);
    let first = LettreDelivery.execute(&request(server.port, Security::Plaintext, true));
    assert_eq!(failure(first).code.as_deref(), Some("network_mail_from"));
    LettreDelivery.execute(&request(server.port, Security::Plaintext, true)).expect("the second connection is served");
    assert_eq!(server.received().len(), 1);
    let log = server.log.lock().unwrap().clone();
    println!("{log:?}");
    assert_eq!(log.len(), 2);
    assert!(log.iter().all(|c| c.iter().any(|l| l == "AUTH XOAUTH2")), "{log:?}");
}

#[test]
fn a_server_answer_is_not_a_broken_connection() {
    // the send crate's own fake refuses MAIL FROM with a 550 and its text
    let server = support::FakeSmtp::start(support::Mode::Refuse);
    let mut req = request(server.port, Security::Plaintext, false);
    req.smtp.credentials = None;
    let f = failure(LettreDelivery.execute(&req));
    println!("refusal: {f:?}");
    assert!(!f.code.as_deref().unwrap_or("").starts_with("network_"), "{f:?}");
    assert!(f.reason_words.contains(support::REFUSAL_TEXT), "{f:?}");
    assert!(!f.retryable);
}
