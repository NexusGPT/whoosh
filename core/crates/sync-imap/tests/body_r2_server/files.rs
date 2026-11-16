//! body-r3: the message with files the attachment tests share (sync-imap's, the engine's and the parse crate's). std only: written by hand,
//! with every name, part number and byte listed here, so a wrong answer of the code under test cannot also be wrong in the fixture.
#![allow(dead_code)]

/// body-r3: a message the server holds as it is (not a shaped one): its bytes, and the body of each part by its IMAP section number, as
/// the test's author wrote them (so a wrong section number in the client cannot also be wrong here).
#[derive(Clone)]
pub struct Fixture { pub raw: Vec<u8>, pub sections: Vec<(String, Vec<u8>)> }

/// One file of `files_mail`, as the author wrote it.
pub struct FixtureFile { pub name: &'static str, pub mime: &'static str, pub bytes: Vec<u8>, pub section: &'static str, pub inline: bool, pub cid: Option<&'static str> }

fn wrap76(b64: &str) -> String { b64.as_bytes().chunks(76).map(|c| std::str::from_utf8(c).unwrap()).collect::<Vec<_>>().join("\r\n") }

fn base64_of(bytes: &[u8]) -> String {
    const A: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::new();
    for c in bytes.chunks(3) {
        let n = (u32::from(c[0]) << 16) | (u32::from(*c.get(1).unwrap_or(&0)) << 8) | u32::from(*c.get(2).unwrap_or(&0));
        for i in 0..4 { if i <= c.len() { out.push(A[((n >> (18 - 6 * i)) & 63) as usize] as char) } else { out.push('=') } }
    }
    out
}

/// body-r3: a message with two attachments and one inline image, in a nested structure:
/// 1 multipart/related (1.1 multipart/alternative: 1.1.1 text, 1.1.2 html; 1.2 the inline logo), 2 a PDF whose name has accents and spaces
/// (RFC 2231: `filename*=UTF-8''...`), 3 a CSV whose name is an RFC 2047 encoded word, its body quoted-printable.
pub fn files_mail() -> (Fixture, Vec<FixtureFile>) {
    let png: Vec<u8> = [0x89u8, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A].iter().copied().chain((0..64u8).map(|i| i.wrapping_mul(37))).collect();
    let pdf: Vec<u8> = (0..=255u8).cycle().take(40_000).collect();
    let csv = "a;b;c\r\n1;\u{e9};3=4\r\n".as_bytes().to_vec();
    let (png64, pdf64) = (wrap76(&base64_of(&png)), wrap76(&base64_of(&pdf)));
    let csv_qp = "a;b;c\r\n1;=C3=A9;3=3D4\r\n";
    let html = "<html><body><p>Voir la facture&nbsp;ci-jointe.</p><img src=\"cid:logo@files.example\"></body></html>";
    let raw = format!("From: Compta <compta@files.example>\r\nTo: owner@example.test\r\nSubject: Facture\r\nMessage-ID: <files@files.example>\r\nMIME-Version: 1.0\r\n\
Content-Type: multipart/mixed; boundary=\"mix\"\r\n\r\n\
--mix\r\nContent-Type: multipart/related; boundary=\"rel\"\r\n\r\n\
--rel\r\nContent-Type: multipart/alternative; boundary=\"alt\"\r\n\r\n\
--alt\r\nContent-Type: text/plain; charset=utf-8\r\n\r\nVoir la facture ci-jointe.\r\n\
--alt\r\nContent-Type: text/html; charset=utf-8\r\n\r\n{html}\r\n\
--alt--\r\n\
--rel\r\nContent-Type: image/png; name=\"logo.png\"\r\nContent-Transfer-Encoding: base64\r\nContent-ID: <logo@files.example>\r\nContent-Disposition: inline; filename=\"logo.png\"\r\n\r\n{png64}\r\n\
--rel--\r\n\
--mix\r\nContent-Type: application/pdf\r\nContent-Transfer-Encoding: base64\r\nContent-Disposition: attachment;\r\n filename*=UTF-8''Facture%20%C3%A9t%C3%A9%20n%C2%B012.pdf\r\n\r\n{pdf64}\r\n\
--mix\r\nContent-Type: text/csv; charset=utf-8; name=\"=?utf-8?Q?Rapport_d=C3=A9c.csv?=\"\r\nContent-Transfer-Encoding: quoted-printable\r\nContent-Disposition: attachment; filename=\"=?utf-8?Q?Rapport_d=C3=A9c.csv?=\"\r\n\r\n{csv_qp}\r\n\
--mix--\r\n").into_bytes();
    let sections = vec![("1.2".to_string(), png64.clone().into_bytes()), ("2".to_string(), pdf64.clone().into_bytes()), ("3".to_string(), csv_qp.as_bytes().to_vec())];
    let files = vec![
        FixtureFile { name: "logo.png", mime: "image/png", bytes: png, section: "1.2", inline: true, cid: Some("logo@files.example") },
        FixtureFile { name: "Facture \u{e9}t\u{e9} n\u{b0}12.pdf", mime: "application/pdf", bytes: pdf, section: "2", inline: false, cid: None },
        FixtureFile { name: "Rapport d\u{e9}c.csv", mime: "text/csv", bytes: csv, section: "3", inline: false, cid: None },
    ];
    (Fixture { raw, sections }, files)
}
