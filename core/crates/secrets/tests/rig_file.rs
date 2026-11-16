//! rig-core: the hidden test copy's keyring file (`RigFileKeychain`). Mutants, each seen red (docs/lanes/ai-settings-core.md, round 2):
//! S1 `open` no longer refuses a namespace outside the rig's; S2 `StoreSecret` keeps the item in memory and skips `save`;
//! S3 `open` ignores the file it finds; S4 `DeleteSecret` removes the item from memory only.
use contracts::{CoreError, KeychainRef};
use secrets::{DeleteSecret, Namespace, ReadSecret, RigFileKeychain, Secret, SecretKind, StoreSecret};

const KEY: &str = "sk-rig-good";

fn text(k: &RigFileKeychain, r: &KeychainRef) -> Option<String> {
    ReadSecret::execute(k, r).ok().map(|s| String::from_utf8(s.expose().to_vec()).unwrap())
}

#[test]
fn a_relaunch_on_the_same_folder_keeps_the_pasted_key() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("rig-keys.json");
    let first = RigFileKeychain::open(Namespace::rig(), &path).unwrap();
    assert!(!path.exists(), "an untouched run writes no file");
    let r = StoreSecret::execute(&first, SecretKind::ProviderCredential, "anthropic", &Secret::new(KEY)).unwrap();
    assert!(path.exists());
    drop(first);
    let again = RigFileKeychain::open(Namespace::rig(), &path).unwrap();
    assert_eq!(text(&again, &r).as_deref(), Some(KEY), "the relaunched keyring reads the key back");
    assert!(!std::fs::read_to_string(&path).unwrap().contains(KEY), "the file holds the bytes as hex, not the key text a grep would show");
}

#[test]
fn delete_removes_the_item_from_the_file_too() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("rig-keys.json");
    let k = RigFileKeychain::open(Namespace::rig(), &path).unwrap();
    let a = StoreSecret::execute(&k, SecretKind::ProviderCredential, "anthropic", &Secret::new(KEY)).unwrap();
    let b = StoreSecret::execute(&k, SecretKind::AccountCredential, "owner@whoosh-check.invalid", &Secret::new("rig-pass")).unwrap();
    DeleteSecret::execute(&k, &a).unwrap();
    DeleteSecret::execute(&k, &a).expect("deleting a gone item is not an error");
    let again = RigFileKeychain::open(Namespace::rig(), &path).unwrap();
    assert_eq!(text(&again, &a), None, "the deleted key is gone after a relaunch");
    assert_eq!(text(&again, &b).as_deref(), Some("rig-pass"), "the other item stays");
}

#[test]
fn the_owners_namespace_is_refused() {
    let dir = tempfile::tempdir().unwrap();
    let refused = RigFileKeychain::open(Namespace::production(), dir.path().join("rig-keys.json"));
    assert!(matches!(refused, Err(CoreError::Invalid { .. })), "a production namespace never reaches a test file");
    assert!(RigFileKeychain::open(Namespace::rig_run("run7").unwrap(), dir.path().join("k.json")).is_ok(), "a narrowed rig namespace is a rig namespace");
}

#[test]
fn a_file_it_did_not_write_is_refused_in_words() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("rig-keys.json");
    std::fs::write(&path, "not a keyring").unwrap();
    assert!(matches!(RigFileKeychain::open(Namespace::rig(), &path), Err(CoreError::Invalid { .. })));
    let outside = r#"{"items": [{"service": "io.nexus.whoosh.provider-credential", "account": "anthropic", "secret_hex": "6b"}]}"#;
    std::fs::write(&path, outside).unwrap();
    assert!(RigFileKeychain::open(Namespace::rig(), &path).is_err(), "an item outside the rig namespace is not read");
    std::fs::write(&path, "{\"items\": []}\n").unwrap();
    assert!(RigFileKeychain::open(Namespace::rig(), &path).is_ok(), "an empty keyring file opens");
}
