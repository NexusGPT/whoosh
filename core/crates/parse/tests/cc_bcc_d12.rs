//! Contract delta 12: the parser reads Cc and Bcc (Bcc only exists on the owner's own sent copies).
use parse::parse_message;

#[test]
fn cc_and_bcc_are_read_in_header_order() {
    let raw = b"From: Me <me@x.example>\r\nTo: a@x.example\r\nCc: Tom <tom@y.example>, sara@z.example\r\nBcc: boss@x.example\r\nSubject: s\r\n\r\nbody\r\n";
    let m = parse_message(raw).expect("parses");
    assert_eq!(m.cc.iter().map(|a| a.email.as_str()).collect::<Vec<_>>(), ["tom@y.example", "sara@z.example"]);
    assert_eq!(m.cc[0].name.as_deref(), Some("Tom"));
    assert_eq!(m.bcc.iter().map(|a| a.email.as_str()).collect::<Vec<_>>(), ["boss@x.example"]);
    let plain = parse_message(b"From: a@x.example\r\nSubject: s\r\n\r\nb\r\n").expect("parses");
    assert!(plain.cc.is_empty() && plain.bcc.is_empty());
}
