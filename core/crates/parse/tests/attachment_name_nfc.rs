//! fix-realcore round 7 (main100 `--slice-attachsend`: every case on "Offre Téléfonica été 2026.pdf" missed by name): macOS hands file names back
//! decomposed (a letter, then U+0301); a mail that names its file that way is read with the name composed (NFC), the form the tiles, the gallery and a
//! forward compare. The bytes and the part are untouched.
//! Broken copy (one edit, red first):
//!   parse/src/mime.rs `attachment_list`: `name: p.attachment_name().map(crate::nfc::nfc),` -> `name: p.attachment_name().map(str::to_string),` (red)
use parse::{attachments_of, nfc};

const COMPOSED: &str = "Offre T\u{e9}l\u{e9}fonica \u{e9}t\u{e9} 2026.pdf";
const DECOMPOSED: &str = "Offre Te\u{301}le\u{301}fonica e\u{301}te\u{301} 2026.pdf";

fn mail(disposition: &str) -> Vec<u8> {
    format!("From: a@example.test\r\nTo: b@example.test\r\nSubject: s\r\nMIME-Version: 1.0\r\nContent-Type: multipart/mixed; boundary=\"b1\"\r\n\r\n\
--b1\r\nContent-Type: text/plain; charset=utf-8\r\n\r\nHello.\r\n\
--b1\r\nContent-Type: application/pdf\r\nContent-Disposition: {disposition}\r\nContent-Transfer-Encoding: base64\r\n\r\nJVBERi0xLjQK\r\n--b1--\r\n").into_bytes()
}

#[test]
fn a_file_named_decomposed_on_the_wire_is_read_composed() {
    assert_ne!(COMPOSED, DECOMPOSED, "two spellings of one name");
    assert_eq!(nfc(DECOMPOSED), COMPOSED, "the crate's NFC composes the name");
    // RFC 2231 with the decomposed bytes percent-encoded, as a client that sends the name macOS gave it writes it
    let rfc2231 = "attachment; filename*=utf-8''Offre%20Te%CC%81le%CC%81fonica%20e%CC%81te%CC%81%202026.pdf";
    let got = attachments_of(&mail(rfc2231)).unwrap();
    assert_eq!(got.len(), 1, "{got:?}");
    assert_eq!(got[0].name.as_deref(), Some(COMPOSED), "the name is read composed: {:?}", got[0].name.as_ref().map(|n| n.chars().map(|c| format!("{:x}", c as u32)).collect::<Vec<_>>()));
    assert_eq!((got[0].part.as_str(), got[0].size), ("2", 9), "the part and its decoded size are untouched");
    // a name already composed stays as it is
    let plain = attachments_of(&mail("attachment; filename*=utf-8''Offre%20T%C3%A9l%C3%A9fonica%20%C3%A9t%C3%A9%202026.pdf")).unwrap();
    assert_eq!(plain[0].name.as_deref(), Some(COMPOSED));
    println!("PASS received-name-composed: a decomposed name on the wire is listed composed");
}
