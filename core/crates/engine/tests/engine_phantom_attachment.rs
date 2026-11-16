//! Owner's picture 212 (13 Nov 2026): "fantom attachment". A forwarded meeting invitation showed a tile "1 attachment": the calendar data of
//! the invitation (a text/calendar part with no file name) was listed as a file. Found with it: the list's paperclip count, the label rules'
//! "has attachment" fact and a mail's attachment names counted EVERY stored part, pictures shown inside the text included.
//! MUTANTS (each turns the test named red):
//!   P1 parse/src/mime.rs `is_invitation_data` answers false                      -> the_calendar_data_of_an_invitation_is_not_a_file
//!   P2 store/src/read.rs ROW_COLUMNS counts every attachment row                 -> only_files_count_as_attachments_in_the_list_the_rules_and_the_names
//!   P3 store/src/label_meta.rs the fact reads EXISTS over every row              -> only_files_count_as_attachments_in_the_list_the_rules_and_the_names
//!   P4 store/src/realcore.rs `message` lists every row's name                    -> only_files_count_as_attachments_in_the_list_the_rules_and_the_names
use contracts::*;
use store::{AttachmentIn, NewMessage, Store};

const USER: &str = "owner@example.test";

fn config() -> AccountConfig {
    AccountConfig {
        imap_host: "127.0.0.1".into(), imap_port: 1, imap_security: ConnectionSecurity::Ssl, smtp_host: "127.0.0.1".into(), smtp_port: 1,
        smtp_security: ConnectionSecurity::Starttls, username: USER.into(), smtp_username: USER.into(), auth_method: AuthMethod::Password, source: DetectionSource::Manual,
    }
}


fn mail(account: &str, id: &str, word: &str, at: i64) -> NewMessage {
    NewMessage {
        message: Message {
            id: id.into(), thread_id: format!("{id}:thread"), account_id: account.into(), sender_name: "Sender".into(), from_address: "sender@example.test".into(),
            to: vec![USER.into()], subject: "Weekly notes".into(), preview: String::new(), received_at_ms: at, read: false, archived: false,
            folder: "INBOX".into(), attachment_names: vec![], login_code: None, trackers_blocked: 0, list_unsubscribe: vec![], list_unsubscribe_one_click: false,
            reply_to_name: None, cc: vec![], bcc: vec![], is_outgoing: false,
        },
        remote_id: None, body_text: Some(format!("The {word} figures for this week are attached.")), body_html: None, has_list_header: false, size_bytes: None, tab_id: None,
    }
}


fn invitation(calendar_headers: &str) -> Vec<u8> {
    format!("From: Nadia <nadia@example.test>\r\nTo: owner@example.test\r\nSubject: TR: Rehearsal\r\nMIME-Version: 1.0\r\nContent-Type: multipart/alternative; boundary=\"b1\"\r\n\r\n\
--b1\r\nContent-Type: text/plain; charset=utf-8\r\n\r\nYou are invited.\r\n\
--b1\r\nContent-Type: text/html; charset=utf-8\r\n\r\n<p>You are invited.</p>\r\n\
--b1\r\n{calendar_headers}\r\n\r\nBEGIN:VCALENDAR\r\nMETHOD:REQUEST\r\nBEGIN:VEVENT\r\nUID:1@example.test\r\nDTSTART:20261120T090000Z\r\nSUMMARY:Rehearsal\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n\
--b1--\r\n").into_bytes()
}

#[test]
fn the_calendar_data_of_an_invitation_is_not_a_file() {
    let data = parse::attachments_of(&invitation("Content-Type: text/calendar; charset=utf-8; method=REQUEST")).unwrap();
    assert_eq!(data.len(), 1, "the part is still listed (its place in the list is its number)");
    assert_eq!((data[0].content_type.as_str(), data[0].name.as_deref(), data[0].inline), ("text/calendar", None, true), "invitation data: no name, not a file");
    // an .ics the sender named and attached stays a file
    let file = parse::attachments_of(&invitation("Content-Type: text/calendar; charset=utf-8; name=\"invite.ics\"\r\nContent-Disposition: attachment; filename=\"invite.ics\"")).unwrap();
    assert_eq!((file.len(), file[0].name.as_deref(), file[0].inline), (1, Some("invite.ics"), false));
}

#[test]
fn only_files_count_as_attachments_in_the_list_the_rules_and_the_names() {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(&dir.path().join("whoosh.sqlite")).unwrap();
    let account = store.add_account(USER, "Owner", MailProvider::Imap, "svc\u{1f}acct", false, config(), 1).unwrap();
    let ids = ["m-picture", "m-invitation", "m-file", "m-plain"].map(String::from);
    store.insert_messages(ids.iter().enumerate().map(|(i, id)| mail(&account, id, "word", 1_700_000_000_000 + i as i64 * 1_000)).collect()).unwrap();
    let part = |name: &str, mime: &str, cid: Option<&str>, inline: bool| AttachmentIn { part: "2".into(), name: name.into(), mime_type: mime.into(), size_bytes: 1_000,
                                                                                         content_id: cid.map(String::from), inline, encoding: "base64".into() };
    store.put_attachment_rows(vec![
        (ids[0].clone(), vec![part("logo.png", "image/png", Some("logo@sig"), true)]),                 // a picture shown inside the text
        (ids[1].clone(), vec![part("attachment-1", "text/calendar", None, true)]),                     // invitation data, as the parser now lists it
        (ids[2].clone(), vec![part("quote.pdf", "application/pdf", None, false), part("logo.png", "image/png", Some("logo@sig"), true)]),
    ], true).unwrap();
    let rows = store.rows_for_messages(&ids.to_vec()).unwrap();
    let count = |id: &str| rows.iter().find(|r| r.message_id == id).map(|r| r.attachment_count);
    assert_eq!((count("m-picture"), count("m-invitation"), count("m-file"), count("m-plain")), (Some(0), Some(0), Some(1), Some(0)), "the list's paperclip counts files only");
    let fact = |id: &str| store.rule_facts(id).unwrap().unwrap().has_attachment;
    assert_eq!((fact("m-picture"), fact("m-invitation"), fact("m-file"), fact("m-plain")), (false, false, true, false), "the rules' fact: a file, never a picture in the text");
    let names = |id: &str| store.message(id).unwrap().unwrap().attachment_names;
    assert_eq!((names("m-picture"), names("m-invitation"), names("m-file")), (vec![], vec![], vec!["quote.pdf".to_string()]), "a mail's attachment names are its files");
}
