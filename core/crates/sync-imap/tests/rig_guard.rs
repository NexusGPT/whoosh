//! engine-act-r6 round 5: in a process a rig core closed (`netguard::close_to_this_mac`), the IMAP dialer and the sign-in check's SMTP
//! dialer refuse every host but this Mac's BEFORE the name is resolved. A guard removed by its mutant reaches a counting listener on
//! 127.0.0.1 (the resolver stub answers it; "0.0.0.0" reaches it too), never the network.
#[path = "../../netguard/tests/stubs.rs"]
mod stubs;
use std::sync::atomic::Ordering;
use sync_imap::{validate, Credential, ImapEndpoint, Security, SmtpEndpoint, ValidationConfig};

fn rt() -> tokio::runtime::Runtime { tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap() }

#[test]
fn the_imap_dialer_and_the_smtp_check_of_a_rig_process_reach_this_mac_only() {
    let imap = stubs::listener();
    let smtp = stubs::listener();
    let asked = stubs::resolver_to(&imap);
    netguard::close_to_this_mac();
    // IMAP: a real-looking name; refused before the resolver is asked
    let cfg = ValidationConfig { username: "owner@example.org".into(), imap: ImapEndpoint { host: "imap.example.org".into(), port: 993, security: Security::Tls }, smtp: None, extra_roots_der: vec![] };
    let report = rt().block_on(validate(&cfg, &Credential::Password("pw".into())));
    assert!(!report.ok(), "{report:?}");
    assert!(format!("{report:?}").contains("not this Mac"), "the refusal in words: {report:?}");
    assert_eq!(asked.load(Ordering::SeqCst), 0, "the resolver was not asked");
    assert_eq!(imap.count(), 0, "no connection");
    // SMTP of the sign-in check: "0.0.0.0" is not this Mac's address (and reaches the local listener only if the guard is gone)
    let closed = std::net::TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap().port();
    let cfg = ValidationConfig { username: "owner@example.org".into(), imap: ImapEndpoint { host: "127.0.0.1".into(), port: closed, security: Security::PlainLoopbackOnly },
        smtp: Some(SmtpEndpoint { host: "0.0.0.0".into(), port: smtp.port, security: send::Security::Implicit }), extra_roots_der: vec![] };
    let report = rt().block_on(validate(&cfg, &Credential::Password("pw".into())));
    assert_eq!(smtp.count(), 0, "the SMTP check opened nothing: {report:?}");
    assert!(format!("{report:?}").contains("0.0.0.0 is not this Mac"), "{report:?}");
    assert!(netguard::refused_count() >= 2);
}
// mutant "imap-dialer-unguarded": transport.rs `connect`, the `netguard::outbound_allowed(host).map_err(refused)?;` line deleted (red: the resolver is asked, the listener counts one)
// mutant "smtp-check-unguarded": validate.rs `smtp_check`, the `netguard::outbound_allowed(host)...?;` line deleted (red: the listener counts one)
