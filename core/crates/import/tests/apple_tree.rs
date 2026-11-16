mod support;

use import::{count_apple_mail, find_apple_mail, walk_apple_mail, Source};
use support::{apple_mailbox, emlx, mail_dir, msg, partial_attachment, Collect};

fn m(id: &str, subject: &str) -> String {
    msg(id, "carol@far.test", "anna@example.test", subject, "hello")
}

/// Inbox: two plain files and one partial message with an attachment file. Sent Messages: two. Clients/Acme: one.
fn build(dir: &std::path::Path) {
    let inbox = apple_mailbox(
        dir,
        "ACCT-1",
        &["Inbox"],
        &[("101.emlx", emlx(&m("i1@x.test", "one"), 1, 1_700_000_001)), ("102.emlx", emlx(&m("i2@x.test", "two"), 0, 1_700_000_002)), ("103.partial.emlx", emlx(&m("i3@x.test", "three"), 1, 1_700_000_003))],
    );
    partial_attachment(&inbox, "103", "2", "contract.pdf", b"%PDF-1.4 fake");
    apple_mailbox(dir, "ACCT-1", &["Sent Messages"], &[("201.emlx", emlx(&m("s1@x.test", "sent one"), 1, 1_700_000_004)), ("202.emlx", emlx(&m("s2@x.test", "sent two"), 1, 1_700_000_005))]);
    apple_mailbox(dir, "ACCT-1", &["Clients", "Acme"], &[("301.emlx", emlx(&m("c1@x.test", "acme"), 1, 1_700_000_006))]);
}

#[test]
fn the_walk_reads_every_mailbox_with_its_name_flags_and_arrival_time() {
    let dir = tempfile::tempdir().unwrap();
    build(dir.path());
    let mut got = Collect::default();
    walk_apple_mail(&mail_dir(dir.path()), &mut got).unwrap();
    let names: Vec<(&str, usize)> = ["Clients/Acme", "Inbox", "Sent Messages"].iter().map(|n| (*n, got.mails.iter().filter(|x| x.mailbox == *n).count())).collect();
    assert_eq!(names, vec![("Clients/Acme", 1), ("Inbox", 3), ("Sent Messages", 2)]);
    let two = got.mails.iter().find(|x| x.origin.ends_with("102.emlx")).unwrap();
    assert!(!two.flags.read);
    assert_eq!(two.received_ms, Some(1_700_000_002_000));
    assert!(got.skips.is_empty());
}

#[test]
fn a_partial_message_carries_a_reference_to_its_attachment_file() {
    let dir = tempfile::tempdir().unwrap();
    build(dir.path());
    let mut got = Collect::default();
    walk_apple_mail(&mail_dir(dir.path()), &mut got).unwrap();
    let partial = got.mails.iter().find(|x| x.origin.ends_with("103.partial.emlx")).unwrap();
    assert_eq!(partial.attachments.len(), 1);
    let a = &partial.attachments[0];
    assert_eq!((a.name.as_str(), a.content_type.as_str(), a.size), ("contract.pdf", "application/pdf", 13));
    assert!(a.location.ends_with("Attachments/103/2/contract.pdf"), "{}", a.location);
    let plain = got.mails.iter().find(|x| x.origin.ends_with("101.emlx")).unwrap();
    assert!(plain.attachments.is_empty());
}

#[test]
fn counting_uses_the_folder_listings_and_agrees_with_the_walk() {
    let dir = tempfile::tempdir().unwrap();
    build(dir.path());
    let counts = count_apple_mail(&mail_dir(dir.path())).unwrap();
    assert_eq!(counts, vec![("Clients/Acme".to_string(), 1), ("Inbox".to_string(), 3), ("Sent Messages".to_string(), 2)]);
}

#[test]
fn a_file_that_cannot_be_read_is_skipped_by_name_and_the_rest_are_read() {
    let dir = tempfile::tempdir().unwrap();
    build(dir.path());
    apple_mailbox(dir.path(), "ACCT-1", &["Inbox"], &[("999.emlx", b"this is not an emlx file".to_vec())]);
    let mut got = Collect::default();
    walk_apple_mail(&mail_dir(dir.path()), &mut got).unwrap();
    assert_eq!(got.mails.len(), 6);
    assert_eq!(got.skips.len(), 1);
    assert_eq!(got.skips[0].0, "Inbox");
    assert!(got.skips[0].1.ends_with("999.emlx"), "{}", got.skips[0].1);
    assert!(got.skips[0].2.contains("byte count"), "{}", got.skips[0].2);
}

#[test]
fn folders_that_are_not_version_folders_are_left_alone() {
    let dir = tempfile::tempdir().unwrap();
    build(dir.path());
    let stray = mail_dir(dir.path()).join("MailData").join("Messages");
    std::fs::create_dir_all(&stray).unwrap();
    std::fs::write(stray.join("1.emlx"), emlx(&m("z@x.test", "stray"), 1, 1)).unwrap();
    let mut got = Collect::default();
    walk_apple_mail(&mail_dir(dir.path()), &mut got).unwrap();
    assert_eq!(got.mails.len(), 6);
}

#[test]
fn finding_apple_mail_answers_none_when_it_never_ran_and_the_source_when_it_did() {
    let home = tempfile::tempdir().unwrap();
    assert_eq!(find_apple_mail(home.path()).unwrap(), None);
    std::fs::create_dir_all(home.path().join("Library").join("Mail")).unwrap();
    assert_eq!(find_apple_mail(home.path()).unwrap(), Some(Source::AppleMail { mail_dir: home.path().join("Library").join("Mail") }));
}
