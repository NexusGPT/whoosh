//! engine-act-r6 round 5: the token endpoint over HTTPS (feature `http`) of a rig process refuses every host but this Mac's.
#![cfg(feature = "http")]
#[path = "../../netguard/tests/stubs.rs"]
mod stubs;
use oauth::TokenEndpoint;

#[test]
fn the_token_endpoint_of_a_rig_process_reaches_this_mac_only() {
    let l = stubs::listener();
    netguard::close_to_this_mac();
    let r = oauth::HttpsTokenEndpoint.post_form(&format!("https://0.0.0.0:{}/oauth2/v2.0/token", l.port), &[("grant_type", "refresh_token")]);
    assert_eq!(l.count(), 0, "nothing opened: {r:?}");
    assert!(matches!(&r, Err(contracts::CoreError::Action { failure }) if failure.code.as_deref() == Some(netguard::OUTBOUND_REFUSED_CODE)), "{r:?}");
}
// mutant "token-endpoint-unguarded": token.rs `post_form`, the `netguard::outbound_allowed_url(url)?;` line deleted (red: the listener counts one)
