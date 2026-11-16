//! engine-act-r6 round 5, the other side: a core that is NOT a rig copy (the owner's) is not refused by the guard. Proved without the
//! network: the guard's resolver stub answers every name with a counting listener on 127.0.0.1, so "imap.example.org" is dialled THERE.
//! Its own process: no rig core opens in it.
#[allow(dead_code, unused_imports)]
#[path = "../../autoconfig/tests/support/mod.rs"]
mod detect_fakes;
#[path = "../../netguard/tests/stubs.rs"]
mod stubs;

use std::sync::atomic::Ordering;
use std::sync::Arc;

use contracts::*;
use detect_fakes::{FakeDns, FakeHttp, FakeProbe};
use engine::{RustCore, TestTransport};
use secrets::{MemoryKeychain, Namespace};
use store::Store;

#[test]
fn the_owners_core_is_not_refused_by_the_guard() {
    let imap = stubs::listener();
    let asked = stubs::resolver_to(&imap);
    let dir = tempfile::tempdir().unwrap();
    let core = RustCore::with_parts(Store::open(&dir.path().join("whoosh.sqlite")).unwrap(), Arc::new(MemoryKeychain::new(Namespace::rig())),
        Box::new(autoconfig::Detector::new(FakeHttp::default(), FakeDns::default(), FakeProbe::default())), TestTransport::Real);
    let config = AccountConfig {
        imap_host: "imap.example.org".into(), imap_port: 993, imap_security: ConnectionSecurity::Ssl, smtp_host: String::new(), smtp_port: 465,
        smtp_security: ConnectionSecurity::Ssl, username: "owner@example.org".into(), smtp_username: "owner@example.org".into(), auth_method: AuthMethod::Password, source: DetectionSource::Manual,
    };
    let r = core.run(Command::AddAccountConfigured { address: "owner@example.org".into(), display_name: None, config, password: SecretText { value: "pw".into() } });
    // the stub listener is no IMAP server, so the sign-in check fails, but at the server, not at the guard
    assert!(r.is_err());
    assert!(!format!("{r:?}").contains("not this Mac"), "{r:?}");
    assert!(asked.load(Ordering::SeqCst) >= 1, "the name was looked up");
    assert!(imap.count() >= 1, "the dial reached the (stub) server");
    assert!(!netguard::is_closed() && netguard::refused_count() == 0, "nothing refused");
    assert_eq!(core.state(netguard::REFUSED_STATE_PATH.into()), None, "the owner's copy serves no rig path");
}
// mutant "guard-always-closed": netguard `outbound_allowed`, `if !is_closed() || is_this_mac(host)` -> `if is_this_mac(host)` (red: refused on the owner's copy)
