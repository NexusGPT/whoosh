//! Delta 24: the provider state words both cores serve at `ai_provider/<id>/state`, `/refusal` and `/models`, and the two-step answer of `SaveProviderKey`
//! (docs/lanes/ai-settings-core.md; lane tests-r58's T-04: the fixture served only `/key` and `/connected`). The spellings are the ones the real core already
//! serves and the pane already compares (`"checking"`, `"connected"`, `"refused"`, `"not connected"`); the one new word is `"not checked"`.
//!
//! THE TWO STEPS: `SaveProviderKey` ANSWERS AT ONCE (step 1) and the state follows (step 2). Step 1 (`provider_state_after_save`): `Checking` when a key check exists for
//! the provider, else `NotChecked` (the key is kept, no check is written down for that provider: Jev). Step 2 (`provider_state_after_check`), only after `Checking`:
//! `Connected` with the provider's model list at `/models`, or `Refused` with the provider's own words at `/refusal`; a refused key stores nothing, so the state after
//! a refusal on a provider that had no key is `Refused` until the next save or remove, and a remove always ends in `NotConnected` (and clears `/refusal`).

/// What a provider's key stands at. `NotChecked` = a key is saved and usable but nothing verified it (no key check exists for that provider).
#[derive(uniffi::Enum, Debug, Clone, Copy, PartialEq, Eq)]
pub enum AiProviderState { NotConnected, Checking, Connected, Refused, NotChecked }

#[uniffi::export]
pub fn ai_provider_state_key(s: AiProviderState) -> String {
    match s { AiProviderState::NotConnected => "not connected", AiProviderState::Checking => "checking", AiProviderState::Connected => "connected", AiProviderState::Refused => "refused", AiProviderState::NotChecked => "not checked" }.to_string()
}
#[uniffi::export]
pub fn ai_provider_state_from_key(key: String) -> Option<AiProviderState> {
    [AiProviderState::NotConnected, AiProviderState::Checking, AiProviderState::Connected, AiProviderState::Refused, AiProviderState::NotChecked].into_iter().find(|s| ai_provider_state_key(*s) == key)
}

/// Step 1 of SaveProviderKey: the state published the moment the command answers.
#[uniffi::export]
pub fn provider_state_after_save(has_key_check: bool) -> AiProviderState { if has_key_check { AiProviderState::Checking } else { AiProviderState::NotChecked } }

/// Step 2: where the check landed (`accepted`).
#[uniffi::export]
pub fn provider_state_after_check(accepted: bool) -> AiProviderState { if accepted { AiProviderState::Connected } else { AiProviderState::Refused } }

/// Whether a job may use the provider: a checked key, or a saved one nothing could check. Checking, refused and no key cannot.
#[uniffi::export]
pub fn provider_state_is_usable(s: AiProviderState) -> bool { matches!(s, AiProviderState::Connected | AiProviderState::NotChecked) }

/// The words the pane reads for a state the owner must see (None for the quiet ones).
#[uniffi::export]
pub fn provider_state_json(s: AiProviderState) -> String { format!("\"{}\"", ai_provider_state_key(s)) }

fn js(v: &str) -> String {
    let mut o = String::from("\"");
    for c in v.chars() { match c { '"' => o.push_str("\\\""), '\\' => o.push_str("\\\\"), '\n' => o.push_str("\\n"), '\r' => o.push_str("\\r"), '\t' => o.push_str("\\t"), c if (c as u32) < 0x20 => o.push_str(&format!("\\u{:04x}", c as u32)), c => o.push(c) } }
    o.push('"');
    o
}
/// `ai_provider/<id>/refusal`: the provider's own words as a JSON string; only while the state is `Refused`.
#[uniffi::export]
pub fn provider_refusal_json(words: String) -> String { js(&words) }
/// `ai_provider/<id>/models`: the model ids the accepted key's check listed, as a JSON array of strings (empty for a provider with no listing).
#[uniffi::export]
pub fn provider_models_json(models: Vec<String>) -> String { format!("[{}]", models.iter().map(|m| js(m)).collect::<Vec<_>>().join(",")) }
