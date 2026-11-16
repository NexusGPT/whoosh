//! Delta 7 invariants that live in the types.
use contracts::{Command, SecretText};

#[test]
fn a_secret_in_a_command_never_reaches_a_log_line() {
    let key = "sk-test-anthropic-0001";
    let command = Command::SaveProviderKey { provider_id: "anthropic".into(), key: SecretText { value: key.into() } };
    let shown = format!("{command:?}");
    assert!(!shown.contains(key), "the key is printed: {shown}");
    assert!(shown.contains("anthropic"), "the rest of the command still prints: {shown}");
    assert_eq!(format!("{:?}", SecretText { value: key.into() }), "SecretText(hidden)");
}
