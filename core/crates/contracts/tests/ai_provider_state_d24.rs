//! Delta 24: the provider state words and the two-step answer of SaveProviderKey.
//! MUTANTS (_captures/replies-contract/mutants.py A01..A07; each must turn the test named red):
//!   A01 "not connected" is spelt "not_connected" (the pane compares the spaced word)       -> the_words_are_the_ones_the_real_core_already_serves
//!   A02 "checking" is spelt "pending"                                                       -> the_words_are_the_ones_the_real_core_already_serves
//!   A03 a provider with no key check publishes "checking" at step 1 (it would never land)    -> step_one_publishes_checking_or_not_checked_and_step_two_lands
//!   A04 a refused check lands "connected"                                                    -> step_one_publishes_checking_or_not_checked_and_step_two_lands
//!   A05 a refused key is usable                                                              -> a_job_may_use_a_checked_or_a_saved_key_only
//!   A06 a key nothing could check is not usable                                              -> a_job_may_use_a_checked_or_a_saved_key_only
//!   A07 the models JSON loses its brackets                                                   -> the_paths_serve_json_the_pane_can_read
use contracts::*;

#[test]
fn the_words_are_the_ones_the_real_core_already_serves() {
    let all = [AiProviderState::NotConnected, AiProviderState::Checking, AiProviderState::Connected, AiProviderState::Refused, AiProviderState::NotChecked];
    let words: Vec<String> = all.iter().map(|s| ai_provider_state_key(*s)).collect();
    assert_eq!(words, ["not connected", "checking", "connected", "refused", "not checked"], "the first four are engine/src/ai_settings.rs STATE_* verbatim");
    for s in all { assert_eq!(ai_provider_state_from_key(ai_provider_state_key(s)), Some(s)); }
    assert_eq!(ai_provider_state_from_key("saved".into()), None);
}

#[test]
fn step_one_publishes_checking_or_not_checked_and_step_two_lands() {
    assert_eq!(provider_state_after_save(true), AiProviderState::Checking);
    assert_eq!(provider_state_after_save(false), AiProviderState::NotChecked, "Jev: kept, nothing can verify it, and no step 2 follows");
    assert_eq!(provider_state_after_check(true), AiProviderState::Connected);
    assert_eq!(provider_state_after_check(false), AiProviderState::Refused);
}

#[test]
fn a_job_may_use_a_checked_or_a_saved_key_only() {
    for (s, ok) in [(AiProviderState::Connected, true), (AiProviderState::NotChecked, true), (AiProviderState::Checking, false), (AiProviderState::Refused, false), (AiProviderState::NotConnected, false)] {
        assert_eq!(provider_state_is_usable(s), ok, "{s:?}");
    }
}

#[test]
fn the_paths_serve_json_the_pane_can_read() {
    assert_eq!(provider_state_json(AiProviderState::Checking), "\"checking\"", "what the pane compares: a JSON string");
    assert_eq!(provider_refusal_json("OpenAI said \"no\"".into()), "\"OpenAI said \\\"no\\\"\"");
    assert_eq!(provider_models_json(vec!["gpt-a".into(), "gpt-b".into()]), "[\"gpt-a\",\"gpt-b\"]");
    assert_eq!(provider_models_json(vec![]), "[]");
}
