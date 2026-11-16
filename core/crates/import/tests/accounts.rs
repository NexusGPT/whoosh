mod support;

use import::{account_for, candidate_addresses, convert, is_sent_mailbox, MailFlags, RawMail};
use support::{accounts, IMPORTED};

fn mail(mailbox: &str, raw: &str) -> RawMail {
    RawMail { mailbox: mailbox.into(), bytes: raw.as_bytes().to_vec(), flags: MailFlags::default(), received_ms: None, attachments: vec![], origin: "test".into() }
}

fn account_of(mailbox: &str, raw: &str) -> String {
    convert(&mail(mailbox, raw), &accounts(), IMPORTED).unwrap().account_id
}

#[test]
fn the_to_address_picks_the_account() {
    assert_eq!(account_of("Inbox", "Message-ID: <a@x>\nFrom: z@far.test\nTo: anna@example.test\nSubject: s\n\nb\n"), "acc-a");
    assert_eq!(account_of("Inbox", "Message-ID: <b@x>\nFrom: z@far.test\nTo: Someone <someone@far.test>, Ben <ben@example.test>\nSubject: s\n\nb\n"), "acc-b");
}

#[test]
fn the_match_ignores_case_on_both_sides() {
    // the account is stored as "Ben@Example.Test"; the message says BEN@EXAMPLE.TEST
    assert_eq!(account_of("Inbox", "Message-ID: <c@x>\nFrom: z@far.test\nTo: BEN@EXAMPLE.TEST\nSubject: s\n\nb\n"), "acc-b");
}

#[test]
fn a_delivery_header_beats_the_to_header() {
    let raw = "Delivered-To: BEN@example.test\nMessage-ID: <d@x>\nFrom: z@far.test\nTo: anna@example.test\nSubject: s\n\nb\n";
    assert_eq!(account_of("Inbox", raw), "acc-b");
}

#[test]
fn a_message_no_account_receives_goes_to_the_imported_account() {
    assert_eq!(account_of("Inbox", "Message-ID: <e@x>\nFrom: z@far.test\nTo: stranger@far.test\nSubject: s\n\nb\n"), IMPORTED);
    assert_eq!(account_of("Inbox", "Message-ID: <f@x>\nSubject: no addresses at all\n\nb\n"), IMPORTED);
}

#[test]
fn in_a_sent_mailbox_the_sender_comes_first() {
    let raw = "Message-ID: <g@x>\nFrom: anna@example.test\nTo: ben@example.test\nSubject: s\n\nb\n";
    assert_eq!(account_of("Sent Messages", raw), "acc-a");
    assert_eq!(account_of("Inbox", raw), "acc-b");
    assert!(is_sent_mailbox("Sent Messages") && is_sent_mailbox("[Gmail]/Sent Mail") && is_sent_mailbox("Sent"));
    assert!(!is_sent_mailbox("Consent forms") && !is_sent_mailbox("Inbox"));
}

#[test]
fn the_candidates_come_in_the_documented_order() {
    let raw = b"Delivered-To: d@x.test\nX-Original-To: o@x.test\nEnvelope-To: e@x.test\nFrom: f@x.test\n\nbody\n";
    let c = candidate_addresses(raw, &["T@x.test".into()], &["c@x.test".into()], &["b@x.test".into()], Some("F@x.test"), false);
    assert_eq!(c, vec!["d@x.test", "o@x.test", "e@x.test", "t@x.test", "c@x.test", "b@x.test", "f@x.test"]);
    let sent = candidate_addresses(raw, &[], &[], &[], Some("F@x.test"), true);
    assert_eq!(sent[0], "f@x.test");
    assert_eq!(account_for(&accounts(), &["nobody@x.test".to_string()]), None);
}
