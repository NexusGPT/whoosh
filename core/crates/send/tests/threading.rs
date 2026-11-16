//! Sending safety, piece 2: In-Reply-To and References on replies, References only on forwards (RFC 5322 3.6.4).
//! Everything runs against the local SMTP server the test starts; the server's bytes are what is asserted.
#[allow(dead_code)]
mod support;

use contracts::{Draft, Signature};
use send::{parse_message_ids, LettreDelivery, ReplyContext, Security, SendAccount, SendInput, SendMessage, SmtpSettings, ThreadKind};
use std::sync::Arc;
use support::{FakeSmtp, FixedClock, Mode, StubSent};

const ADDRESS: &str = "assem@northlight-example.de";

fn input(port: u16, reply: Option<ReplyContext>) -> SendInput {
    SendInput {
        account: SendAccount {
            id: "acc-nordlicht".into(),
            name: "Nordlicht".into(),
            address: ADDRESS.into(),
            smtp: SmtpSettings { host: "127.0.0.1".into(), port, security: Security::Plaintext, credentials: None },
        },
        draft: Draft {
            id: "draft-77".into(),
            from_account_id: "acc-nordlicht".into(),
            to: vec!["orders@linea-example.com".into()],
            cc: vec![],
            subject: "Re: Quote for the Q1 print run".into(),
            body: "Yes.".into(),
            attachment_ids: vec![],
            ai_marks: vec![],
            reply_to_thread_id: None,
            bcc: vec![],
        },
        signature: Some(Signature { address: ADDRESS.into(), text: "Assem".into() }),
        reply,
        files: vec![],
    }
}

fn send_one(reply: Option<ReplyContext>) -> String {
    let server = FakeSmtp::start(Mode::Accept);
    let uc = SendMessage::new(Arc::new(LettreDelivery), StubSent::ok(), Arc::new(FixedClock(1_794_000_000_000)));
    uc.execute(&input(server.port, reply)).expect("send");
    let got = server.received();
    assert_eq!(got.len(), 1);
    got[0].data.clone()
}

/// The header block with folded lines joined (RFC 5322 2.2.3), one entry per header.
fn headers(data: &str) -> Vec<(String, String)> {
    let head = data.split_once("\r\n\r\n").expect("header/body split").0;
    let unfolded = head.replace("\r\n ", " ").replace("\r\n\t", " ");
    unfolded.lines().filter_map(|l| l.split_once(':').map(|(n, v)| (n.to_ascii_lowercase(), v.trim().to_string()))).collect()
}

fn get(h: &[(String, String)], name: &str) -> Vec<String> {
    h.iter().filter(|(n, _)| n == name).map(|(_, v)| v.clone()).collect()
}

fn chain(n: usize) -> Vec<String> {
    (1..=n).map(|i| format!("<msg-{i:02}-0123456789abcdef@mail.linea-example.com>")).collect()
}

#[test]
fn a_reply_carries_in_reply_to_and_the_parents_references_then_the_parent_oldest_first() {
    let ctx = ReplyContext { kind: ThreadKind::Reply, parent_message_id: "<c@linea-example.com>".into(), parent_references: vec!["<a@linea-example.com>".into(), "<b@linea-example.com>".into()] };
    let h = headers(&send_one(Some(ctx)));
    assert_eq!(get(&h, "in-reply-to"), vec!["<c@linea-example.com>".to_string()]);
    assert_eq!(get(&h, "references"), vec!["<a@linea-example.com> <b@linea-example.com> <c@linea-example.com>".to_string()]);
}

#[test]
fn a_reply_to_a_first_message_references_only_the_parent() {
    let ctx = ReplyContext { kind: ThreadKind::Reply, parent_message_id: "<first@linea-example.com>".into(), parent_references: vec![] };
    let h = headers(&send_one(Some(ctx)));
    assert_eq!(get(&h, "in-reply-to"), vec!["<first@linea-example.com>".to_string()]);
    assert_eq!(get(&h, "references"), vec!["<first@linea-example.com>".to_string()]);
}

#[test]
fn a_forward_carries_references_and_no_in_reply_to() {
    let ctx = ReplyContext { kind: ThreadKind::Forward, parent_message_id: "<c@linea-example.com>".into(), parent_references: vec!["<a@linea-example.com>".into()] };
    let h = headers(&send_one(Some(ctx)));
    assert_eq!(get(&h, "references"), vec!["<a@linea-example.com> <c@linea-example.com>".to_string()]);
    assert!(get(&h, "in-reply-to").is_empty(), "a forward does not claim to answer its parent: {h:?}");
}

#[test]
fn a_new_message_carries_neither_header() {
    let h = headers(&send_one(None));
    assert!(get(&h, "in-reply-to").is_empty() && get(&h, "references").is_empty(), "{h:?}");
}

#[test]
fn a_long_chain_is_folded_under_the_line_limit_and_keeps_its_order() {
    let ids = chain(8);
    let ctx = ReplyContext { kind: ThreadKind::Reply, parent_message_id: "<parent@linea-example.com>".into(), parent_references: ids.clone() };
    let data = send_one(Some(ctx));
    let head = data.split_once("\r\n\r\n").unwrap().0;
    assert!(head.lines().all(|l| l.len() <= 78), "a header line is longer than 78 characters (RFC 5322 2.1.1):\n{head}");
    let h = headers(&data);
    let mut expected = ids;
    expected.push("<parent@linea-example.com>".into());
    assert_eq!(get(&h, "references"), vec![expected.join(" ")]);
}

#[test]
fn an_id_already_in_the_chain_is_not_repeated() {
    let ctx = ReplyContext { kind: ThreadKind::Reply, parent_message_id: "<b@x>".into(), parent_references: vec!["<a@x>".into(), "<b@x>".into()] };
    assert_eq!(ctx.references(), vec!["<a@x>".to_string(), "<b@x>".to_string()]);
}

#[test]
fn message_ids_are_read_from_a_header_value_in_order() {
    assert_eq!(parse_message_ids("<a@x>\r\n <b@x>  junk <c@x>"), vec!["<a@x>", "<b@x>", "<c@x>"]);
    assert!(parse_message_ids("no ids here <>").is_empty());
}
