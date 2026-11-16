//! The guard itself: open while no rig core opened, closed for every host but this Mac's after `close_to_this_mac`, each refusal counted.
//! One process: the close is one-way, so the open case is read first.
use netguard::*;

#[test]
fn open_then_closed_this_mac_always_allowed_each_refusal_counted() {
    assert!(outbound_allowed("imap.example.org").is_ok(), "open before a rig core closes the process");
    assert_eq!(refused_count(), 0);
    close_to_this_mac();
    for here in ["127.0.0.1", "127.8.9.10", "::1", "[::1]", "localhost", "LOCALHOST.", " localhost "] { assert!(outbound_allowed(here).is_ok(), "{here}"); }
    for away in ["imap.example.org", "0.0.0.0", "10.0.0.1", "192.0.2.1", "::", "[2001:db8::1]", "localhost.example.org", "127.0.0.1.nip.io", ""] {
        let e = outbound_allowed(away).expect_err(away);
        assert!(matches!(&e, contracts::CoreError::Action { failure } if failure.code.as_deref() == Some(OUTBOUND_REFUSED_CODE) && failure.reason_words.contains("not this Mac")), "{e:?}");
    }
    assert_eq!(refused_count(), 9);
    assert_eq!(last_refused().as_deref(), Some(""));
    assert!(outbound_allowed_url("https://user@login.example.com:443/token?x=1").is_err());
    assert_eq!(last_refused().as_deref(), Some("login.example.com"));
    assert!(outbound_allowed_url("http://127.0.0.1:8080/x").is_ok());
    assert!(outbound_allowed_url("http://[::1]:8080/x").is_ok());
    assert_eq!(refused_json(), "{\"count\":10,\"last_host\":\"login.example.com\"}");
}
