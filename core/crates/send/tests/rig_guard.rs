//! engine-act-r6 round 5: the SMTP dialer of a rig process (`netguard::close_to_this_mac`) refuses every host but this Mac's, before lettre
//! resolves the name. "0.0.0.0" is not this Mac's address and reaches the counting listener only if the guard is gone.
#[path = "../../netguard/tests/stubs.rs"]
mod stubs;
use contracts::CoreError;
use send::{DeliverPort, DeliverRequest, Envelope, LettreDelivery, Security, SmtpSettings};

fn request(host: &str, port: u16) -> DeliverRequest {
    DeliverRequest { account_name: "Owner".into(), smtp: SmtpSettings { host: host.into(), port, security: Security::Implicit, credentials: None },
        envelope: Envelope { from: "owner@example.org".into(), to: vec!["friend@example.org".into()] }, rfc822: b"Subject: hi\r\n\r\nhi\r\n".to_vec() }
}

#[test]
fn the_smtp_dialer_of_a_rig_process_reaches_this_mac_only() {
    let l = stubs::listener();
    netguard::close_to_this_mac();
    let r = LettreDelivery.execute(&request("0.0.0.0", l.port));
    assert_eq!(l.count(), 0, "nothing opened: {r:?}");
    assert!(matches!(&r, Err(CoreError::Action { failure }) if failure.code.as_deref() == Some(netguard::OUTBOUND_REFUSED_CODE)), "{r:?}");
    // a name is refused the same way, never resolved (checked after the address, so a mutant stops above and never asks a resolver)
    assert!(LettreDelivery.execute(&request("smtp.example.org", 465)).is_err());
    assert_eq!(netguard::last_refused().as_deref(), Some("smtp.example.org"));
}
// mutant "smtp-dialer-unguarded": smtp.rs, the `netguard::outbound_allowed(&smtp.host)?;` line deleted (red: the listener counts one)
