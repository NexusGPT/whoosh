//! Delta 23 addition: the on-device model's hand-over and the stored consent for a hosted decision model (owner's report "doesnt work for laya";
//! docs/lanes/labels-r5.md 15.4 rows 2 and 4). Pure rules; the command is `GiveDecisionConsent`, the read `WhooshCore::decision_consent`,
//! the state path `decision_consent/<jev|local|rules>` (JSON `{"needed":bool,"given":bool}`).
//!
//! WHAT THE CONSENT COVERS: the mail text a label's AI card ticks (subject, body, rest of the thread, attachments: `AiReads`) is sent to that provider to
//! be sorted, and NOTHING else (no address book, no other mail, no key). Only a HOSTED model needs it (`Jev`); `Local` (Laya, on this Mac) and `Rules` send
//! nothing off the Mac, so they need none and a consent for them is refused. Withdrawing it (`given: false`) stops the next call; nothing already sent is recalled.
use crate::*;

pub const DECISION_CONSENT_NEEDED_CODE: &str = "decision_consent_needed";
pub const DECISION_CONSENT_NOT_NEEDED_CODE: &str = "decision_consent_not_needed";

/// DATA-MAP `decision-consent`: one stored yes (no row = not given).
#[derive(uniffi::Record, Debug, Clone, PartialEq, Eq)]
pub struct DecisionConsent { pub provider: DecisionProviderKind, pub given_at_ms: i64 }

/// Whether a decision provider sends the label AI's mail text off this Mac.
#[uniffi::export]
pub fn decision_provider_is_hosted(provider: DecisionProviderKind) -> bool { matches!(provider, DecisionProviderKind::Jev) }

/// The stored spelling of a provider that can hold a consent ("jev"); None for one that needs none.
#[uniffi::export]
pub fn decision_consent_key(provider: DecisionProviderKind) -> Option<String> { if decision_provider_is_hosted(provider) { Some("jev".into()) } else { None } }

/// Refuses choosing a hosted model with no consent, in words (`decision_consent_needed`): the words say what goes where. A model that stays on this Mac passes.
#[uniffi::export]
pub fn check_decision_model_consent(provider: DecisionProviderKind, consent_given: bool) -> Result<(), CoreError> {
    if decision_provider_is_hosted(provider) && !consent_given {
        return Err(CoreError::Invalid { field: DECISION_CONSENT_NEEDED_CODE.into(), reason_words: "Allow the text of the mail your labels read (subject, body, rest of the thread, attachments) to be sent to this provider, or choose the model on this Mac".into() });
    }
    Ok(())
}

/// Refuses a consent for a provider that sends nothing off the Mac (`decision_consent_not_needed`).
#[uniffi::export]
pub fn check_give_decision_consent(provider: DecisionProviderKind) -> Result<(), CoreError> {
    if decision_provider_is_hosted(provider) { Ok(()) } else { Err(CoreError::Invalid { field: DECISION_CONSENT_NOT_NEEDED_CODE.into(), reason_words: "this model stays on this Mac: it needs no consent".into() }) }
}

/// `decision_consent/<provider>`: `needed` (the provider is hosted) and `given`.
#[uniffi::export]
pub fn decision_consent_json(provider: DecisionProviderKind, given: bool) -> String { format!("{{\"needed\":{},\"given\":{}}}", decision_provider_is_hosted(provider), given) }
