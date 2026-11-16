mod support;

use import::{import, ImportContext, ImportError, ImportProgress, Source, IMPORT_BATCH_MESSAGES};
use support::{accounts, apple_mailbox, emlx, mail_dir, mbox, msg, msg_no_id, FailingSink, VecSink, IMPORTED};

fn ctx(a: &[import::AccountAddress]) -> ImportContext<'_> {
    ImportContext { accounts: a, imported_account_id: IMPORTED }
}

/// Inbox: 3 messages (two for Anna, one for a stranger). Sent Messages: 2 (from Anna).
fn tree(dir: &std::path::Path) {
    let to = |id: &str, who: &str| msg(id, "carol@far.test", who, id, "hello");
    apple_mailbox(
        dir,
        "ACCT-1",
        &["Inbox"],
        &[
            ("1.emlx", emlx(&to("i1@x.test", "anna@example.test"), 1, 1_700_000_001)),
            ("2.emlx", emlx(&to("i2@x.test", "anna@example.test"), 1, 1_700_000_002)),
            ("3.emlx", emlx(&to("i3@x.test", "stranger@far.test"), 1, 1_700_000_003)),
        ],
    );
    let sent = |id: &str| msg(id, "anna@example.test", "carol@far.test", id, "hello");
    apple_mailbox(dir, "ACCT-1", &["Sent Messages"], &[("4.emlx", emlx(&sent("s1@x.test"), 1, 1_700_000_004)), ("5.emlx", emlx(&sent("s2@x.test"), 1, 1_700_000_005))]);
}

fn run(source: &Source, sink: &mut VecSink) -> (import::ImportReport, Vec<ImportProgress>) {
    let a = accounts();
    let mut calls = Vec::new();
    let report = import(source, &ctx(&a), sink, &mut |p: &ImportProgress| calls.push(p.clone())).unwrap();
    (report, calls)
}

#[test]
fn progress_carries_a_count_per_mailbox_and_the_total_when_it_is_known() {
    let dir = tempfile::tempdir().unwrap();
    tree(dir.path());
    let mut sink = VecSink::default();
    let (report, calls) = run(&Source::AppleMail { mail_dir: mail_dir(dir.path()) }, &mut sink);
    let last = calls.last().unwrap();
    assert!(last.finished);
    assert_eq!(last.total_known, Some(5));
    let counts: Vec<(&str, u64, u64, u64)> = last.mailboxes.iter().map(|m| (m.mailbox.as_str(), m.seen, m.stored, m.skipped)).collect();
    assert_eq!(counts, vec![("Inbox", 3, 3, 0), ("Sent Messages", 2, 2, 0)]);
    assert_eq!((last.seen, last.stored, last.skipped), (5, 5, 0));
    assert_eq!(report.mailboxes, last.mailboxes);
    // the Inbox's count was already shown before the second mailbox finished
    assert!(calls.iter().any(|p| !p.finished && p.mailboxes.first().map(|m| (m.mailbox.as_str(), m.stored)) == Some(("Inbox", 3))));
    // never goes backwards
    assert!(calls.windows(2).all(|w| w[0].seen <= w[1].seen && w[0].stored <= w[1].stored));
    assert_eq!(report.stored, 5);
}

#[test]
fn each_message_goes_to_the_account_of_its_address_and_the_rest_to_imported() {
    let dir = tempfile::tempdir().unwrap();
    tree(dir.path());
    let mut sink = VecSink::default();
    let (report, _) = run(&Source::AppleMail { mail_dir: mail_dir(dir.path()) }, &mut sink);
    assert_eq!(report.per_account, vec![("acc-a".to_string(), 4), (IMPORTED.to_string(), 1)]);
    let row = sink.rows.iter().find(|r| r.message.id.ends_with("i3@x.test")).unwrap();
    assert_eq!(row.message.account_id, IMPORTED);
    assert_eq!(row.message.folder, "INBOX");
    let sent = sink.rows.iter().find(|r| r.message.id.ends_with("s1@x.test")).unwrap();
    assert_eq!((sent.message.account_id.as_str(), sent.message.folder.as_str()), ("acc-a", "Sent Messages"));
}

#[test]
fn the_same_source_imported_twice_stores_each_message_once() {
    let dir = tempfile::tempdir().unwrap();
    tree(dir.path());
    let source = Source::AppleMail { mail_dir: mail_dir(dir.path()) };
    let mut sink = VecSink::default();
    let (first, _) = run(&source, &mut sink);
    let (second, _) = run(&source, &mut sink);
    assert_eq!((first.stored, first.already_there), (5, 0));
    assert_eq!((second.stored, second.already_there, second.seen), (0, 5, 5));
    assert_eq!(sink.rows.len(), 5);
}

#[test]
fn the_same_message_in_two_formats_is_one_row_keyed_by_its_message_id() {
    let dir = tempfile::tempdir().unwrap();
    let to = "anna@example.test";
    let in_apple = msg("same@x.test", "carol@far.test", to, "hello", "body as Apple Mail kept it");
    apple_mailbox(dir.path(), "ACCT-1", &["Inbox"], &[("1.emlx", emlx(&in_apple, 1, 1))]);
    // the same message from a Thunderbird mbox: other bytes (a Status header, other body whitespace), same Message-ID
    let in_mbox = format!("X-Mozilla-Status: 0001\n{}", msg("same@x.test", "carol@far.test", to, "hello", "body as Thunderbird kept it\n\n"));
    let file = dir.path().join("Inbox");
    std::fs::write(&file, mbox(&[&in_mbox])).unwrap();
    let mut sink = VecSink::default();
    let (a, _) = run(&Source::AppleMail { mail_dir: mail_dir(dir.path()) }, &mut sink);
    let (b, _) = run(&Source::Mbox { file }, &mut sink);
    assert_eq!((a.stored, b.stored, b.already_there), (1, 0, 1));
    assert_eq!(sink.rows[0].message.id, "imp:acc-a:same@x.test");
}

#[test]
fn without_a_message_id_the_key_is_a_hash_of_the_bytes() {
    let dir = tempfile::tempdir().unwrap();
    let a = msg_no_id("carol@far.test", "anna@example.test", "same", "same body");
    let b = msg_no_id("carol@far.test", "anna@example.test", "other", "other body");
    let file = dir.path().join("Inbox");
    std::fs::write(&file, mbox(&[&a, &a, &b])).unwrap();
    let mut sink = VecSink::default();
    let (report, _) = run(&Source::Mbox { file }, &mut sink);
    assert_eq!((report.seen, report.stored, report.already_there), (3, 2, 1));
    assert!(sink.rows.iter().all(|r| r.message.id.starts_with("imp:acc-a:h:") && r.message.id.len() == "imp:acc-a:h:".len() + 32));
}

#[test]
fn a_message_marked_deleted_and_a_bad_file_are_skipped_and_counted() {
    let dir = tempfile::tempdir().unwrap();
    let m = |id: &str| msg(id, "carol@far.test", "anna@example.test", id, "x");
    apple_mailbox(dir.path(), "ACCT-1", &["Inbox"], &[("1.emlx", emlx(&m("ok@x.test"), 1, 1)), ("2.emlx", emlx(&m("gone@x.test"), 1 | 2, 1)), ("3.emlx", b"junk".to_vec())]);
    let mut sink = VecSink::default();
    let (report, _) = run(&Source::AppleMail { mail_dir: mail_dir(dir.path()) }, &mut sink);
    assert_eq!((report.seen, report.stored, report.already_there), (3, 1, 0));
    assert_eq!(report.skipped.len(), 2);
    assert!(report.skipped.iter().any(|s| s.origin.ends_with("2.emlx") && s.words.contains("deleted")));
    assert!(report.skipped.iter().any(|s| s.origin.ends_with("3.emlx")));
    assert_eq!(report.mailboxes[0].skipped, 2);
}

#[test]
fn flagged_messages_are_named_for_the_engine() {
    let dir = tempfile::tempdir().unwrap();
    let m = |id: &str| msg(id, "carol@far.test", "anna@example.test", id, "x");
    apple_mailbox(dir.path(), "ACCT-1", &["Inbox"], &[("1.emlx", emlx(&m("star@x.test"), 1 | 16, 1)), ("2.emlx", emlx(&m("plain@x.test"), 1, 1))]);
    let mut sink = VecSink::default();
    let (report, _) = run(&Source::AppleMail { mail_dir: mail_dir(dir.path()) }, &mut sink);
    assert_eq!(report.flagged_ids, vec!["imp:acc-a:star@x.test".to_string()]);
}

#[test]
fn messages_are_written_in_batches_of_the_ledger_size() {
    let dir = tempfile::tempdir().unwrap();
    let n = IMPORT_BATCH_MESSAGES + 5;
    let msgs: Vec<String> = (0..n).map(|i| msg(&format!("b{i}@x.test"), "carol@far.test", "anna@example.test", "s", "b")).collect();
    let refs: Vec<&str> = msgs.iter().map(String::as_str).collect();
    let file = dir.path().join("Inbox");
    std::fs::write(&file, mbox(&refs)).unwrap();
    let mut sink = VecSink::default();
    let (report, calls) = run(&Source::Mbox { file }, &mut sink);
    assert_eq!(sink.batches, vec![IMPORT_BATCH_MESSAGES, 5]);
    assert_eq!(report.stored as usize, n);
    // an mbox has no index, so the total is not known up front
    assert_eq!(calls.last().unwrap().total_known, None);
}

#[test]
fn a_refused_write_stops_the_import_with_the_real_cause_in_words() {
    let dir = tempfile::tempdir().unwrap();
    tree(dir.path());
    let a = accounts();
    let err = import(&Source::AppleMail { mail_dir: mail_dir(dir.path()) }, &ctx(&a), &mut FailingSink, &mut |_| {}).unwrap_err();
    assert!(matches!(err, ImportError::Store { .. }));
    assert!(err.words().contains("disk full"), "{}", err.words());
}
