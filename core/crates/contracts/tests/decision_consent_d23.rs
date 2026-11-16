//! Delta 23 addition: stored consent for a hosted decision model, and the on-device model's hand-over.
//! MUTANTS (_captures/replies-contract/mutants.py Z01..Z06; each must turn the test named red):
//!   Z01 a hosted model is accepted with no consent                         -> a_hosted_model_needs_the_owners_yes_and_a_local_one_needs_none
//!   Z02 the local model (Laya) is treated as hosted                        -> a_hosted_model_needs_the_owners_yes_and_a_local_one_needs_none
//!   Z03 a consent for a model that stays on this Mac is accepted           -> a_consent_is_given_only_for_a_hosted_provider
//!   Z04 the consent key of Jev changes spelling                            -> a_consent_is_given_only_for_a_hosted_provider
//!   Z05 the state JSON says needed:false for Jev                           -> the_state_json_and_the_trait_defaults
//!   Z06 the trait's default consent answers true                           -> the_state_json_and_the_trait_defaults
use contracts::*;
use std::sync::Arc;

fn code(r: Result<(), CoreError>) -> String { match r { Err(CoreError::Invalid { field, .. }) => field, o => panic!("expected a refusal, got {o:?}") } }

#[test]
fn a_hosted_model_needs_the_owners_yes_and_a_local_one_needs_none() {
    assert_eq!(code(check_decision_model_consent(DecisionProviderKind::Jev, false)), DECISION_CONSENT_NEEDED_CODE);
    check_decision_model_consent(DecisionProviderKind::Jev, true).unwrap();
    for p in [DecisionProviderKind::Local, DecisionProviderKind::Rules] { check_decision_model_consent(p, false).expect("nothing leaves the Mac: no consent needed"); }
    assert!(decision_provider_is_hosted(DecisionProviderKind::Jev) && !decision_provider_is_hosted(DecisionProviderKind::Local));
}

#[test]
fn a_consent_is_given_only_for_a_hosted_provider() {
    check_give_decision_consent(DecisionProviderKind::Jev).unwrap();
    assert_eq!(code(check_give_decision_consent(DecisionProviderKind::Local)), DECISION_CONSENT_NOT_NEEDED_CODE);
    assert_eq!(code(check_give_decision_consent(DecisionProviderKind::Rules)), DECISION_CONSENT_NOT_NEEDED_CODE);
    assert_eq!(decision_consent_key(DecisionProviderKind::Jev), Some("jev".to_string()));
    assert_eq!(decision_consent_key(DecisionProviderKind::Local), None);
}

struct Dummy;
impl LocalModel for Dummy { fn generate(&self, _p: String, _m: u32) -> Result<String, CoreError> { Ok("x".into()) } }

#[test]
fn the_state_json_and_the_trait_defaults() {
    assert_eq!(decision_consent_json(DecisionProviderKind::Jev, true), "{\"needed\":true,\"given\":true}");
    assert_eq!(decision_consent_json(DecisionProviderKind::Local, false), "{\"needed\":false,\"given\":false}");
    // the trait's new read has a default body (a Swift or Rust core written before it still compiles; the fixture core overrides it): pinned in the source,
    // because WhooshCore has too many required methods to implement in a test
    let src = std::fs::read_to_string(format!("{}/src/command.rs", env!("CARGO_MANIFEST_DIR"))).unwrap();
    assert!(src.contains("fn decision_consent(&self, provider: DecisionProviderKind) -> bool { let _ = provider; false }"), "no consent until the owner gives it");
    let cmd = Command::GiveDecisionConsent { provider: DecisionProviderKind::Jev, given: true };
    assert_eq!(cmd.clone(), cmd);
    let m: Arc<dyn LocalModel> = Arc::new(Dummy);
    assert_eq!(m.generate("p".into(), 1).unwrap(), "x");
}
