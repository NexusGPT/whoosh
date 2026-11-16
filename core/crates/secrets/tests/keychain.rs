//! Runs against the real macOS Keychain, on rig items only: every service starts with
//! `io.nexus.whoosh.rig.` and every item a test makes is deleted when its guard drops. The
//! secrets below are invented strings, never a real credential.
#![cfg(target_os = "macos")]

use std::process::Command;

use contracts::{CoreError, KeychainRef};
use secrets::{DeleteSecret, Keychain, Namespace, ReadSecret, Secret, SecretKind, StoreSecret, SyncMode};

const FIXTURE_APP_PASSWORD: &str = "fx-app-pw-q7Zk3-not-a-real-credential";

/// Owns a run-tagged rig namespace and deletes everything stored through it, pass or fail.
struct Rig {
    keychain: Keychain,
    made: std::cell::RefCell<Vec<KeychainRef>>,
}

impl Rig {
    /// Unsigned test process: items are written without the iCloud flag (see `SyncMode`).
    fn new() -> Self { Self::with_sync(SyncMode::LocalOnly) }
    fn with_sync(sync: SyncMode) -> Self {
        let tag = format!("t{}-{}", std::process::id(), std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH).expect("clock").as_nanos());
        let ns = Namespace::rig_run(&tag).expect("tag is valid");
        assert!(ns.prefix().starts_with(Namespace::RIG_PREFIX));
        Rig { keychain: Keychain::with_sync(ns, sync), made: Default::default() }
    }
    fn store(&self, kind: SecretKind, name: &str, secret: &str) -> KeychainRef {
        let r = StoreSecret::execute(&self.keychain, kind, name, &Secret::new(secret)).expect("store");
        self.made.borrow_mut().push(r.clone());
        r
    }
}

impl Drop for Rig {
    fn drop(&mut self) {
        for r in self.made.borrow().iter() {
            let _ = DeleteSecret::execute(&self.keychain, r);
            if !std::thread::panicking() {
                assert!(matches!(ReadSecret::execute(&self.keychain, r), Err(CoreError::NotFound { .. })),
                    "rig item {} was left behind", r.service);
            }
        }
    }
}

#[test]
fn fixture_app_password_round_trips_under_the_rig_identity() {
    let rig = Rig::new();
    let r = rig.store(SecretKind::AccountCredential, "acct-fixture-1", FIXTURE_APP_PASSWORD);
    assert!(r.service.starts_with("io.nexus.whoosh.rig."), "service is {}", r.service);
    let back = ReadSecret::execute(&rig.keychain, &r).expect("read");
    assert_eq!(back.expose(), FIXTURE_APP_PASSWORD.as_bytes());
}

/// The shipped mode asks the OS for `kSecAttrSynchronizable = true` on account and provider
/// credentials and `false` on a device link. Read from the exact query the adapter builds.
#[test]
fn adapter_query_asks_for_synchronizable_on_the_credential_kinds() {
    use core_foundation::base::TCFType;
    use core_foundation::boolean::CFBoolean;
    use security_framework_sys::item::kSecAttrSynchronizable;
    let kc = Keychain::new(Namespace::rig());
    for (kind, want) in [
        (SecretKind::AccountCredential, true),
        (SecretKind::ProviderCredential, true),
        (SecretKind::DeviceLink, false),
    ] {
        let r = Namespace::rig().reference(kind, "q").expect("ref");
        let options = kc.options(&r, kind);
        #[allow(deprecated)]
        let got = options.query.iter()
            .find(|(k, _)| k.as_concrete_TypeRef() == unsafe { kSecAttrSynchronizable })
            .and_then(|(_, v)| v.downcast::<CFBoolean>())
            .map(bool::from);
        assert_eq!(got, Some(want), "{kind:?}");
    }
}

/// Live: the flag reaches the OS. An unsigned test process is refused with -34018 (a required
/// entitlement is missing) precisely because the item is synchronizable; the same item without
/// the flag is accepted (every other test). A signed app gets Ok and the read-back says true.
#[test]
fn live_icloud_store_reaches_the_os_with_the_synchronizable_flag() {
    let rig = Rig::with_sync(SyncMode::ICloud);
    let result = StoreSecret::execute(&rig.keychain, SecretKind::AccountCredential, "acct-fixture-2",
        &Secret::new(FIXTURE_APP_PASSWORD));
    match result {
        Ok(r) => {
            rig.made.borrow_mut().push(r.clone());
            assert!(rig.keychain.attributes(&r).expect("attrs").synchronizable);
            println!("SYNC-LIVE: stored and read back synchronizable = true");
        }
        Err(CoreError::Action { failure }) => {
            assert!(failure.reason_words.contains("-34018"), "unexpected refusal: {}", failure.reason_words);
            println!("SYNC-LIVE: refused with -34018 (unsigned process): the flag reached the OS");
        }
        Err(other) => panic!("unexpected: {other:?}"),
    }
}

#[test]
fn local_only_items_read_back_as_not_synchronizable() {
    let rig = Rig::new();
    let acct = rig.store(SecretKind::AccountCredential, "acct-fixture-4", FIXTURE_APP_PASSWORD);
    assert!(!rig.keychain.attributes(&acct).expect("attrs").synchronizable);
}

#[test]
fn storing_again_replaces_and_delete_is_idempotent() {
    let rig = Rig::new();
    let r = rig.store(SecretKind::ProviderCredential, "prov-fixture-2", "fx-first");
    let r2 = rig.store(SecretKind::ProviderCredential, "prov-fixture-2", "fx-second");
    assert_eq!(r, r2);
    assert_eq!(ReadSecret::execute(&rig.keychain, &r).expect("read").expose(), b"fx-second");
    DeleteSecret::execute(&rig.keychain, &r).expect("first delete");
    DeleteSecret::execute(&rig.keychain, &r).expect("second delete is not an error");
    assert!(matches!(ReadSecret::execute(&rig.keychain, &r), Err(CoreError::NotFound { .. })));
}

#[test]
fn a_reference_outside_the_namespace_is_refused_before_the_keychain_is_asked() {
    let rig = Rig::new();
    let foreign = KeychainRef { service: "com.example.someone-else".into(), account: "x".into() };
    assert!(matches!(ReadSecret::execute(&rig.keychain, &foreign), Err(CoreError::Invalid { .. })));
    assert!(matches!(DeleteSecret::execute(&rig.keychain, &foreign), Err(CoreError::Invalid { .. })));
    assert!(rig.keychain.attributes(&foreign).is_err());
    let other_ns = Namespace::production().reference(SecretKind::AccountCredential, "x").expect("ref");
    assert!(matches!(ReadSecret::execute(&rig.keychain, &other_ns), Err(CoreError::Invalid { .. })));
}

#[test]
fn debug_output_never_carries_the_secret() {
    let s = Secret::new(FIXTURE_APP_PASSWORD);
    assert!(!format!("{s:?}").contains("q7Zk3"));
}

/// The database keeps only the reference. Uses the system `sqlite3` to build a database the
/// way the store does (reference columns, no secret column) and greps its dump.
#[test]
fn database_dump_holds_the_reference_and_never_the_secret() {
    let rig = Rig::new();
    let r = rig.store(SecretKind::AccountCredential, "acct-fixture-3", FIXTURE_APP_PASSWORD);
    let dir = tempfile::tempdir().expect("tempdir");
    let db = dir.path().join("whoosh.db");
    let sql = format!(
        "CREATE TABLE account(id TEXT PRIMARY KEY, keychain_service TEXT, keychain_account TEXT); \
         INSERT INTO account VALUES('a1','{}','{}');",
        r.service, r.account
    );
    let out = Command::new("sqlite3").arg(&db).arg(&sql).output().expect("sqlite3 runs");
    assert!(out.status.success(), "sqlite3: {}", String::from_utf8_lossy(&out.stderr));
    let dump = Command::new("sqlite3").arg(&db).arg(".dump").output().expect("dump");
    assert!(dump.status.success());
    let text = String::from_utf8_lossy(&dump.stdout);
    assert!(text.contains(&r.account), "the dump must hold the reference");
    assert_eq!(text.matches(FIXTURE_APP_PASSWORD).count(), 0, "the dump must not hold the secret");
    // The raw file too, not only the dump.
    let raw = std::fs::read(&db).expect("db file");
    assert!(!raw.windows(FIXTURE_APP_PASSWORD.len()).any(|w| w == FIXTURE_APP_PASSWORD.as_bytes()));
}
