mod support;

use import::{convert, count_olm, is_message_entry, mailbox_of, walk_olm, ImportError};
use support::{accounts, olm_file, olm_xml, Collect, OlmMsg, IMPORTED};

const MESSAGES: &str = "Local/com.microsoft.__Messages/Inbox";

fn archive(dir: &std::path::Path) -> std::path::PathBuf {
    let one = olm_xml(&OlmMsg {
        id: "o1@outlook.test",
        subject: "Réunion budget",
        from_name: "Dana Lee",
        from: "dana@far.test",
        to: "anna@example.test",
        sent: "2024-01-02T10:11:12",
        text: "Bonjour, voir le fichier joint.",
        html: "<p>Bonjour, <b>voir</b> le fichier joint.</p>",
        read: false,
        attachments: &[("report.pdf", "application/pdf", "Local/com.microsoft.__Attachments/1/report.pdf")],
    });
    let two = olm_xml(&OlmMsg { id: "o2@outlook.test", subject: "Plain", from_name: "", from: "dana@far.test", to: "ben@example.test", sent: "2024-01-03T08:00:00", text: "just text", html: "", read: true, attachments: &[] });
    let path = dir.join("export.olm");
    let (n1, n2) = (format!("{MESSAGES}/message_00001.xml"), format!("{MESSAGES}/message_00002.xml"));
    olm_file(
        &path,
        &[
            (n1.as_str(), one.into_bytes()),
            (n2.as_str(), two.into_bytes()),
            ("Local/com.microsoft.__Attachments/1/report.pdf", b"12345".to_vec()),
            ("Local/com.microsoft.__Calendar/Main/calendar_00001.xml", b"<x/>".to_vec()),
        ],
    );
    path
}

#[test]
fn entries_named_message_digits_xml_are_messages_and_the_folders_name_the_mailbox() {
    assert!(is_message_entry("Local/com.microsoft.__Messages/Inbox/message_00001.xml"));
    assert!(!is_message_entry("Local/com.microsoft.__Calendar/Main/calendar_00001.xml"));
    assert!(!is_message_entry("Local/com.microsoft.__Messages/Inbox/message_.xml"));
    assert_eq!(mailbox_of("Local/com.microsoft.__Messages/Inbox/message_00001.xml"), "Inbox");
    assert_eq!(mailbox_of("Local/com.microsoft.__Messages/Clients/Acme/message_00001.xml"), "Clients/Acme");
    assert_eq!(mailbox_of("message_00001.xml"), "Outlook");
}

#[test]
fn two_messages_and_one_attachment_are_read_and_nothing_else_is() {
    let dir = tempfile::tempdir().unwrap();
    let path = archive(dir.path());
    assert_eq!(count_olm(&path).unwrap(), 2);
    let mut got = Collect::default();
    walk_olm(&path, &mut got).unwrap();
    assert_eq!(got.mails.len(), 2);
    assert!(got.skips.is_empty());
    assert!(got.mails.iter().all(|m| m.mailbox == "Inbox"));
    let one = &got.mails[0];
    assert!(!one.flags.read);
    assert_eq!(one.received_ms, Some(1_704_190_272_000));
    assert_eq!(one.attachments.len(), 1);
    assert_eq!((one.attachments[0].name.as_str(), one.attachments[0].content_type.as_str(), one.attachments[0].size), ("report.pdf", "application/pdf", 5));
    assert!(got.mails[1].attachments.is_empty());
    assert!(got.mails[1].flags.read);
}

#[test]
fn the_message_goes_through_the_normal_parser_with_accents_headers_and_bodies_intact() {
    let dir = tempfile::tempdir().unwrap();
    let mut got = Collect::default();
    walk_olm(&archive(dir.path()), &mut got).unwrap();
    let row = convert(&got.mails[0], &accounts(), IMPORTED).unwrap();
    let m = &row.new.message;
    assert_eq!(m.subject, "Réunion budget");
    assert_eq!(m.sender_name, "Dana Lee");
    assert_eq!(m.from_address, "dana@far.test");
    assert_eq!(m.to, vec!["anna@example.test".to_string()]);
    assert_eq!(m.account_id, "acc-a");
    assert_eq!(m.attachment_names, vec!["report.pdf".to_string()]);
    assert!(row.new.body_text.as_deref().unwrap().contains("Bonjour, voir le fichier joint."));
    assert!(row.new.body_html.as_deref().unwrap().contains("<b>voir</b>"));
    assert_eq!(row.key, "o1@outlook.test");
    assert_eq!(m.id, "imp:acc-a:o1@outlook.test");
}

#[test]
fn a_plain_text_message_has_no_html_part() {
    let dir = tempfile::tempdir().unwrap();
    let mut got = Collect::default();
    walk_olm(&archive(dir.path()), &mut got).unwrap();
    let row = convert(&got.mails[1], &accounts(), IMPORTED).unwrap();
    assert_eq!(row.new.body_html, None);
    assert_eq!(row.new.message.account_id, "acc-b");
    assert!(row.new.message.read);
}

#[test]
fn a_file_that_is_not_an_archive_is_refused_in_words() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("fake.olm");
    std::fs::write(&path, b"this is not a zip file").unwrap();
    match count_olm(&path) {
        Err(ImportError::Unreadable { words, .. }) => assert!(words.contains("not a readable Outlook"), "{words}"),
        other => panic!("{other:?}"),
    }
    let mut got = Collect::default();
    assert!(matches!(walk_olm(&path, &mut got), Err(ImportError::Unreadable { .. })));
}

#[test]
fn a_message_xml_that_is_broken_is_skipped_by_name_and_the_other_is_read() {
    let dir = tempfile::tempdir().unwrap();
    let good = olm_xml(&OlmMsg { id: "g@o.test", subject: "ok", from_name: "", from: "a@far.test", to: "anna@example.test", sent: "2024-01-02T10:11:12", text: "t", html: "", read: true, attachments: &[] });
    let path = dir.path().join("mixed.olm");
    let (n1, n2) = (format!("{MESSAGES}/message_00001.xml"), format!("{MESSAGES}/message_00002.xml"));
    olm_file(&path, &[(n1.as_str(), b"<emails><email>".to_vec()), (n2.as_str(), good.into_bytes())]);
    let mut got = Collect::default();
    walk_olm(&path, &mut got).unwrap();
    assert_eq!(got.mails.len(), 1);
    assert_eq!(got.skips.len(), 1);
    assert!(got.skips[0].1.contains("message_00001.xml"), "{}", got.skips[0].1);
}
