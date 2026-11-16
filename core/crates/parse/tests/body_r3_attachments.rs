//! body-r3: what the parse crate says about a message's files: names decoded (RFC 2231, RFC 2047), sizes decoded, content id, inline flag,
//! the IMAP section of each part and its transfer encoding; the bytes of a part fetched by its section decode to the original.
#[path = "../../sync-imap/tests/body_r2_server/files.rs"]
mod files;

use files::files_mail;
use parse::{attachments_of, attachments_with_bytes, decode_transfer, parse_message};

#[test]
fn two_attachments_and_an_inline_image_are_three_parts_with_their_names_sections_and_sizes() {
    let (fx, files) = files_mail();
    let got = attachments_of(&fx.raw).unwrap();
    assert_eq!(got.len(), 3, "{got:?}");
    for f in &files {
        let a = got.iter().find(|a| a.part == f.section).unwrap_or_else(|| panic!("no part {} in {got:?}", f.section));
        assert_eq!(a.name.as_deref(), Some(f.name), "part {}", f.section);
        assert_eq!(a.content_type, f.mime, "part {}", f.section);
        assert_eq!(a.size, f.bytes.len(), "the decoded size of part {}", f.section);
        assert_eq!(a.inline, f.inline, "part {}", f.section);
        assert_eq!(a.content_id.as_deref(), f.cid, "part {}", f.section);
    }
    let by = |s: &str| got.iter().find(|a| a.part == s).unwrap().encoding.clone();
    assert_eq!((by("1.2"), by("2"), by("3")), ("base64".to_string(), "base64".to_string(), "quoted-printable".to_string()));
    // the one list: the full parse says the same
    assert_eq!(parse_message(&fx.raw).unwrap().attachments, got);
}

#[test]
fn the_bytes_of_a_part_fetched_by_its_section_decode_to_the_original() {
    let (fx, files) = files_mail();
    let got = attachments_of(&fx.raw).unwrap();
    for f in &files {
        let a = got.iter().find(|a| a.part == f.section).unwrap();
        let body = &fx.sections.iter().find(|(k, _)| k == f.section).unwrap().1;
        assert_eq!(decode_transfer(&a.encoding, body), f.bytes, "part {}", f.section);
    }
    // the same bytes from the whole message
    for (a, bytes) in attachments_with_bytes(&fx.raw).unwrap() {
        assert_eq!(bytes, files.iter().find(|f| f.section == a.part).unwrap().bytes, "part {}", a.part);
    }
}

#[test]
fn a_message_that_is_one_attachment_part_and_a_nested_message_are_numbered_as_imap_numbers_them() {
    // not multipart: the only part is 1
    let one = b"From: a@x.example\r\nContent-Type: application/pdf; name=\"a.pdf\"\r\nContent-Transfer-Encoding: base64\r\n\r\nSGVsbG8=\r\n";
    let got = attachments_of(one).unwrap();
    assert_eq!((got.len(), got[0].part.as_str(), got[0].size), (1, "1", 5));
    // a message/rfc822 attachment is a file of its own, at its own number
    let nested = b"From: a@x.example\r\nContent-Type: multipart/mixed; boundary=b\r\n\r\n--b\r\nContent-Type: text/plain\r\n\r\nhi\r\n--b\r\nContent-Type: message/rfc822\r\nContent-Disposition: attachment\r\n\r\nFrom: z@x.example\r\nSubject: inner\r\n\r\ninner body\r\n--b--\r\n";
    let got = attachments_of(nested).unwrap();
    assert_eq!(got.len(), 1, "{got:?}");
    assert_eq!((got[0].part.as_str(), got[0].content_type.as_str()), ("2", "message/rfc822"));
}

#[test]
fn a_message_with_no_file_has_no_attachment_and_bytes_that_are_not_a_message_are_refused() {
    assert!(attachments_of(b"From: a@x.example\r\nSubject: s\r\n\r\njust text\r\n").unwrap().is_empty());
    assert!(attachments_of(b"").is_err());
}
