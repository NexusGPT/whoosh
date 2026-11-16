//! The in-memory adapter honours the same ports, for other lanes' unit tests.
use contracts::CoreError;
use secrets::{DeleteSecret, MemoryKeychain, Namespace, ReadSecret, Secret, SecretKind, StoreSecret};

#[test]
fn memory_adapter_round_trips_and_deletes() {
    let m = MemoryKeychain::new(Namespace::rig());
    let r = StoreSecret::execute(&m, SecretKind::DeviceLink, "link-1", &Secret::new("fx")).expect("store");
    assert_eq!(ReadSecret::execute(&m, &r).expect("read").expose(), b"fx");
    DeleteSecret::execute(&m, &r).expect("delete");
    assert!(matches!(ReadSecret::execute(&m, &r), Err(CoreError::NotFound { .. })));
}

#[test]
fn names_are_validated_at_the_boundary() {
    let m = MemoryKeychain::new(Namespace::rig());
    let bad = StoreSecret::execute(&m, SecretKind::AccountCredential, "has space/..", &Secret::new("fx"));
    assert!(matches!(bad, Err(CoreError::Invalid { .. })));
}
