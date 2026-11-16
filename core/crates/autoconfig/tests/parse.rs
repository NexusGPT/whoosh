//! The Mozilla format parser on its own: placeholders, plain-text servers, unknown placeholders.
use autoconfig::parse::mozilla;
use autoconfig::*;

fn doc(imap_sock: &str, user: &str) -> String {
    format!(r#"<clientConfig version="1.1"><emailProvider id="x"><incomingServer type="imap"><hostname>imap.%EMAILDOMAIN%</hostname><port>993</port><socketType>{imap_sock}</socketType><username>{user}</username><authentication>password-cleartext</authentication></incomingServer><outgoingServer type="smtp"><hostname>smtp.x.example</hostname><port>587</port><socketType>STARTTLS</socketType><username>%EMAILADDRESS%</username></outgoingServer></emailProvider></clientConfig>"#)
}

#[test]
fn placeholders_resolve_in_hosts_and_usernames() {
    let e = EmailAddress::parse("jane+tag@x.example").expect("address");
    let c = mozilla::parse(&doc("SSL", "%EMAILLOCALPART%"), &e, Source::Ispdb).expect("parses");
    assert_eq!((c.imap_host.as_str(), c.username.as_str(), c.smtp_username.as_str()), ("imap.x.example", "jane+tag", "jane+tag@x.example"));
    assert_eq!((c.smtp_security, c.auth_method), (Security::Starttls, AuthMethod::Password));
}

#[test]
fn a_plain_text_imap_server_and_an_unknown_placeholder_give_nothing() {
    let e = EmailAddress::parse("jane@x.example").expect("address");
    assert!(mozilla::parse(&doc("plain", "%EMAILADDRESS%"), &e, Source::Ispdb).is_none());
    assert!(mozilla::parse(&doc("SSL", "%REALNAME%"), &e, Source::Ispdb).is_none());
    assert!(mozilla::parse("<html><body>not found</body></html>", &e, Source::Ispdb).is_none());
}
