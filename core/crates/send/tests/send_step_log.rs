//! fix-realcore round 15b (lead: "core.log has no line for this send at all"): every try writes one line per SMTP step (send/src/smtp.rs
//! `LoggedDelivery`): the try itself (security, auth kind, token length, EHLO name, counts), the address that answered, each step with the server's
//! code and first answer line, or how the connection broke and at which step. Never the token, never an address of the mail, never a line of it.
//! Loopback only (send/tests/staged/mod.rs). Broken copies (one edit each in send/src/smtp.rs, red first):
//!   L1 `fail`: the `line(broke(stage, &e));` line deleted                 -> a_reset_at_auth_is_logged_as_the_step_it_broke_at
//!   L2 `auth`: `format!("XOAUTH2, token {} characters", c.secret.chars().count())` -> `format!("XOAUTH2, token {}", c.secret)`
//!                                        -> no_line_holds_the_token_an_address_or_the_message, a_sent_message_logs_every_step_with_the_servers_answer
//!   L3 the `line(format!("after_data: {}", answer(&r)));` line deleted     -> a_sent_message_logs_every_step_with_the_servers_answer
mod staged;

use send::{DeliverPort, DeliverRequest, Envelope, LoggedDelivery, Security, SmtpCredentials, SmtpSettings};
use staged::{BreakAt, How, StagedSmtp};
use std::sync::{Arc, Mutex};

const TOKEN: &str = "at-fixture-secret-token";
const BODY: &str = "The quarterly figures are attached.";
const TO: &str = "someone@example.test";

fn request(port: u16) -> DeliverRequest {
    DeliverRequest {
        account_name: "Microsoft".into(),
        smtp: SmtpSettings { host: "127.0.0.1".into(), port, security: Security::Plaintext,
            credentials: Some(SmtpCredentials { username: "smtp-user@contoso-example.test".into(), secret: TOKEN.into(), bearer: true }) },
        envelope: Envelope { from: "owner@contoso-example.test".into(), to: vec![TO.into()] },
        rfc822: format!("From: owner@contoso-example.test\r\nTo: {TO}\r\nSubject: Q3\r\nMessage-ID: <log-test@contoso-example.test>\r\n\r\n{BODY}\r\n").into_bytes(),
    }
}

fn logged(server: &StagedSmtp) -> (Result<(), contracts::CoreError>, Vec<String>) {
    let lines = Arc::new(Mutex::new(Vec::new()));
    let sink = lines.clone();
    let d = LoggedDelivery { log: Arc::new(move |l: &str| sink.lock().unwrap().push(l.to_string())) };
    let r = d.execute(&request(server.port));
    let got = lines.lock().unwrap().clone();
    for l in &got { println!("{l}"); }
    (r, got)
}

#[test]
fn a_sent_message_logs_every_step_with_the_servers_answer() {
    let server = StagedSmtp::start(BreakAt::Connect, How::Reset, 0);
    let (r, lines) = logged(&server);
    r.expect("sent");
    let steps = ["try:", "connect 127.0.0.1:", "auth: 235", "mail_from: 250", "rcpt_to: 250", "data: 354", "after_data: 250 queued"];
    for s in steps {
        assert!(lines.iter().any(|l| l.contains(s)), "no line with {s:?}: {lines:?}");
    }
    assert!(lines.iter().all(|l| l.starts_with(&format!("smtp Microsoft 127.0.0.1:{}: ", server.port))), "{lines:?}");
    assert!(lines[0].contains("auth XOAUTH2, token 23 characters") && lines[0].contains("EHLO name [127.0.0.1],"), "the neutral name, never the host name: {}", lines[0]);
}

#[test]
fn a_reset_at_auth_is_logged_as_the_step_it_broke_at() {
    let server = StagedSmtp::start(BreakAt::Auth, How::Reset, 1);
    let (r, lines) = logged(&server);
    assert!(r.is_err());
    let last = lines.last().expect("a line");
    assert!(last.contains("auth: broke: network error: Connection reset by peer"), "{lines:?}");
    assert!(!lines.iter().any(|l| l.contains("mail_from")), "nothing after the break: {lines:?}");
}

#[test]
fn no_line_holds_the_token_an_address_or_the_message() {
    for at in [None, Some(BreakAt::Auth), Some(BreakAt::AfterData)] {
        let server = match at { Some(a) => StagedSmtp::start(a, How::Reset, 1), None => StagedSmtp::start(BreakAt::Connect, How::Reset, 0) };
        let (_r, lines) = logged(&server);
        let all = lines.join("\n");
        for secret in [TOKEN, BODY, TO, "smtp-user@contoso-example.test", "owner@contoso-example.test"] {
            assert!(!all.contains(secret), "{at:?}: a line holds {secret:?}: {all}");
        }
    }
}
