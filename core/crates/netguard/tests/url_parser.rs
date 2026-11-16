//! rig-net-rust (audit r77, gap 4): the guard decides on the host the HTTP client dials. reqwest parses every URL with the `url`
//! crate, so for each awkward form below the guard's verdict on a closed (rig) process must be the verdict on the host `url::Url::parse`
//! names: allowed only when that host is this Mac, refused when the parser refuses the URL or names no host. The expected column is
//! written by hand and is itself checked against the parser, so a wrong row fails too.
//! mutant "old-hand-parser": lib.rs `host_of_url` back to the split on '/', '?', '#' and the last '@' (red: the backslash row and the
//! scheme-less rows disagree with the parser).
use netguard::*;

/// The host the client dials: `url::Url::parse(..).host_str()`, or None when the client could not send to it at all.
fn client_host(u: &str) -> Option<String> { url::Url::parse(u).ok().and_then(|p| p.host_str().map(str::to_string)) }

const HERE: bool = true;
const AWAY: bool = false;

/// (the URL, the host the `url` crate names or "" for none, allowed on a rig)
const TABLE: &[(&str, &str, bool)] = &[
    ("https://evil.example\\@127.0.0.1/", "evil.example", AWAY),       // backslash ends the authority of http(s): the @ is path
    ("https://evil.example\\@127.0.0.1:8080/x", "evil.example", AWAY),
    ("https://127.0.0.1:8080\\@evil.example/", "127.0.0.1", HERE),     // the client dials this Mac; the old parser read evil.example
    ("https://user:pw@evil.example/", "evil.example", AWAY),            // userinfo
    ("https://127.0.0.1@evil.example/", "evil.example", AWAY),
    ("https://evil.example@127.0.0.1/", "127.0.0.1", HERE),
    ("https://evil.example#@127.0.0.1/", "evil.example", AWAY),        // '#' ends the authority
    ("https://evil.example?@127.0.0.1/", "evil.example", AWAY),        // '?' ends the authority
    ("http://[::1]:8080/x", "[::1]", HERE),                             // IPv6 literal
    ("http://[2001:db8::1]/", "[2001:db8::1]", AWAY),
    ("http://[::ffff:127.0.0.1]/", "[::ffff:7f00:1]", AWAY),            // IPv4-mapped: not ::1, refused (the safe side)
    ("https://localhost./", "localhost.", HERE),                        // trailing dot
    ("http://127.0.0.1./", "127.0.0.1", HERE),
    ("HTTPS://LOCALHOST/", "localhost", HERE),                          // upper case
    ("https://EVIL.EXAMPLE/", "evil.example", AWAY),
    ("http://0x7f.0.0.1/", "127.0.0.1", HERE),                          // hex IPv4: the parser names 127.0.0.1
    ("http://2130706433/", "127.0.0.1", HERE),                          // decimal IPv4: the parser names 127.0.0.1
    ("http://0177.0.0.1/", "127.0.0.1", HERE),                          // octal IPv4
    ("http://127.1/", "127.0.0.1", HERE),                               // short IPv4
    ("http://0x0a.0.0.1/", "10.0.0.1", AWAY),
    ("https://%6c%6fcalhost/", "localhost", HERE),                      // percent-encoded host
    ("https://localhost.evil.example/", "localhost.evil.example", AWAY),
    ("https://127.0.0.1.evil.example/", "127.0.0.1.evil.example", AWAY),
    ("https://", "", AWAY),                                             // empty host: the parser refuses it
    ("http://:80/", "", AWAY),
    ("127.0.0.1:8080/x", "", AWAY),                                     // scheme-less: the client cannot send it
    ("localhost:8080", "", AWAY),                                       // parsed as scheme "localhost" with no host
    ("evil.example", "", AWAY),
    ("", "", AWAY),
];

#[test]
fn the_guard_decides_on_the_host_the_client_dials() {
    close_to_this_mac();
    let mut wrong = Vec::new();
    for &(u, host, here) in TABLE {
        let parsed = client_host(u).unwrap_or_default();
        assert_eq!(parsed, host, "table row {u:?}: the url crate names {parsed:?}");
        assert_eq!(!parsed.is_empty() && is_this_mac(&parsed), here, "table row {u:?}: is_this_mac({parsed:?})");
        let verdict = outbound_allowed_url(u).is_ok();
        if verdict != here || host_of_url(u) != parsed {
            wrong.push(format!("{u:?}: guard {} host {:?}, client dials {parsed:?} ({})", if verdict { "allows" } else { "refuses" }, host_of_url(u), if here { "this Mac" } else { "not this Mac" }));
        }
    }
    assert!(wrong.is_empty(), "guard and client disagree on {} of {} rows:\n{}", wrong.len(), TABLE.len(), wrong.join("\n"));
    // one test in this binary: the refusal record is process-wide, so no other test may refuse in between
    assert!(outbound_allowed_url("https://evil.example\\@127.0.0.1/?token=secret").is_err());
    assert!(refused_json().contains("evil.example") && !refused_json().contains("secret"), "{}", refused_json());
}
