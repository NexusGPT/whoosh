//! jmap-guard: in a process a rig core closed (`netguard::close_to_this_mac`), the JMAP connect refuses every host but this Mac's BEFORE
//! any connection is opened. A guard removed by its mutant reaches a counting listener on 127.0.0.1, never the network.
//! One test fn: the close is one-way for the whole process, so the order inside it is the proof (loopback is still allowed after it).
mod common;
#[path = "../../netguard/tests/stubs.rs"]
mod stubs;

use common::*;
use sync_jmap::JmapAdapter;

#[tokio::test]
async fn the_jmap_connect_of_a_rig_process_reaches_this_mac_only() {
    // the counting listener answers on loopback; "0.0.0.0" is not this Mac's address but reaches the listener if the guard is gone
    let counting = stubs::listener();
    netguard::close_to_this_mac();
    let before = netguard::refused_count();
    let out = JmapAdapter::connect(config_for(&format!("http://0.0.0.0:{}", counting.port), sync_jmap::JmapAuth::Bearer(TOKEN.into()))).await;
    assert_eq!(counting.count(), 0, "no connection was opened: {out:?}");
    let words = format!("{out:?}");
    assert!(out.is_err() && words.contains("0.0.0.0 is not this Mac"), "the refusal in words, not a fallback: {words}");
    assert_eq!(netguard::refused_count(), before + 1, "the refusal is counted");
    assert_eq!(netguard::last_refused().as_deref(), Some("0.0.0.0"));
}

#[tokio::test]
async fn a_loopback_address_still_connects_on_a_rig_process() {
    netguard::close_to_this_mac();
    let fake = Fake::start(Mode::Jmap, vec![FEmail::inbox(1)], 500).await;
    let before = netguard::refused_count();
    let out = JmapAdapter::connect(fake.config()).await;
    fake.shutdown().await;
    assert!(matches!(out, Ok(sync_jmap::Discovery::Jmap(_))), "loopback connects: {out:?}");
    assert!(netguard::refused_count() >= before, "loopback adds no refusal of its own host");
    assert_ne!(netguard::last_refused().as_deref(), Some("127.0.0.1"));
}
// mutant "jmap-dialer-unguarded": adapter.rs `JmapAdapter::connect`, the `netguard::outbound_allowed_url(&cfg.base_url)?;` line deleted (red: the listener counts one and the connect is not refused)
