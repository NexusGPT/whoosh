//! fn-ai-providers against invented, recorded-shape fixtures: a valid key lists models, a refused key returns the
//! provider's reason string unchanged, a completion returns text. The fixture also pins the request each provider documents.
mod support;

use std::sync::Arc;

use ai_providers::providers::{self, Provider};
use ai_providers::table::{info, ProviderId, PROVIDERS};
use ai_providers::transport::Transport;
use ai_providers::{CompleteText, ListModels, TextRequest};
use contracts::CoreError;
use support::{Closed, Fixture, Vault};

fn adapter(id: ProviderId, t: Arc<dyn Transport>, vault: &Vault) -> Box<dyn Provider> {
    providers::build(id, t, vault.store.clone(), None)
}

fn request(model: &str) -> TextRequest {
    TextRequest { model: model.into(), system: Some("Write like the owner.".into()), prompt: "Shorten this reply to one line.".into(), max_output_tokens: 7 }
}

fn words(e: CoreError) -> (String, String, bool) {
    match e {
        CoreError::Action { failure } => (failure.account_name, failure.reason_words, failure.retryable),
        other => panic!("expected the provider's refusal, got {other:?}"),
    }
}

const ANSWER: &str = "Thanks Marta, Friday works for me.";

fn lists(slug: &str, id: ProviderId, expected: &[&str]) {
    let fx = Fixture::load("providers", slug);
    let vault = Vault::new(slug);
    let key = vault.put("k", fx.doc["key"].as_str().unwrap_or("none"));
    let replay = fx.replay("list_models");
    let p = adapter(id, replay.clone(), &vault);
    let got = ListModels::execute(&*p, if info(id).needs_key { Some(&key) } else { None }).expect("a valid key lists models");
    assert_eq!(got, expected);
    replay.assert_done();
}

fn completes(slug: &str, id: ProviderId, model: &str) {
    let fx = Fixture::load("providers", slug);
    let vault = Vault::new(slug);
    let key = vault.put("k", &fx.doc["key"].as_str().unwrap_or("none"));
    let replay = fx.replay("complete");
    let p = adapter(id, replay.clone(), &vault);
    let got = CompleteText::execute(&*p, if info(id).needs_key { Some(&key) } else { None }, &request(model)).expect("completion");
    assert_eq!(got, ANSWER);
    replay.assert_done();
}

/// A refused key through the list call; returns (provider name, words, retryable).
fn refused_list(slug: &str, id: ProviderId) -> (String, String, bool) {
    let fx = Fixture::load("providers", slug);
    let vault = Vault::new(slug);
    let key = vault.put("k", &fx.bad_key());
    let replay = fx.replay("refused");
    let p = adapter(id, replay.clone(), &vault);
    let e = ListModels::execute(&*p, Some(&key)).expect_err("a refused key is an error");
    replay.assert_done();
    words(e)
}

#[test]
fn openai_lists_models() { lists("openai", ProviderId::OpenAi, &["fixture-model-a", "fixture-model-b", "fixture-embed-c"]); }
#[test]
fn anthropic_lists_models_across_pages() { lists("anthropic", ProviderId::Anthropic, &["fixture-claude-a", "fixture-claude-b", "fixture-claude-c"]); }
#[test]
fn gemini_lists_only_models_that_write_text_across_pages() { lists("gemini", ProviderId::Gemini, &["fixture-gemini-a", "fixture-gemini-c"]); }
#[test]
fn openrouter_checks_the_key_then_lists_models() { lists("openrouter", ProviderId::OpenRouter, &["fixture-vendor/model-a", "fixture-vendor/model-b"]); }
#[test]
fn ollama_lists_the_pulled_models_with_no_key() { lists("ollama", ProviderId::Ollama, &["fixture-llama:latest", "fixture-gemma:2b"]); }
#[test]
fn lmstudio_lists_models_with_no_key() { lists("lmstudio", ProviderId::LmStudio, &["fixture-qwen-7b", "fixture-embed-nomic"]); }
#[test]
fn chatgpt_sign_in_token_lists_models() {
    let fx = Fixture::load("providers", "chatgpt");
    let vault = Vault::new("chatgpt-list");
    let key = vault.put("token", &fx.key());
    let replay = fx.replay("list_models");
    let p = providers::ChatGptSignIn::new(replay.clone(), vault.store.clone(), None, Some("acct-fixture-0001".into()));
    assert_eq!(ListModels::execute(&p, Some(&key)).unwrap(), ["fixture-chatgpt-a", "fixture-chatgpt-b"]);
    replay.assert_done();
}

#[test]
fn openai_refused_key_returns_the_providers_words_unchanged() {
    let (who, why, retry) = refused_list("openai", ProviderId::OpenAi);
    assert_eq!(who, "OpenAI");
    assert_eq!(why, "Incorrect API key provided: sk-FIXT****FUSED. You can find your API key at https://platform.openai.com/account/api-keys.");
    assert!(!retry);
}
#[test]
fn anthropic_refused_key_returns_the_providers_words_unchanged() {
    let (who, why, retry) = refused_list("anthropic", ProviderId::Anthropic);
    assert_eq!((who.as_str(), why.as_str(), retry), ("Anthropic", "invalid x-api-key", false));
}
#[test]
fn gemini_refused_key_returns_the_providers_words_unchanged() {
    let (_, why, _) = refused_list("gemini", ProviderId::Gemini);
    assert_eq!(why, "API key not valid. Please pass a valid API key.");
}
#[test]
fn openrouter_refused_key_returns_the_providers_words_unchanged() {
    let (_, why, _) = refused_list("openrouter", ProviderId::OpenRouter);
    assert_eq!(why, "User not found.");
}
#[test]
fn lmstudio_refused_key_returns_the_providers_words_unchanged() {
    let (_, why, _) = refused_list("lmstudio", ProviderId::LmStudio);
    assert_eq!(why, "Invalid API key.");
}
#[test]
fn chatgpt_refused_token_returns_the_providers_words_unchanged() {
    let fx = Fixture::load("providers", "chatgpt");
    let vault = Vault::new("chatgpt-refused");
    let key = vault.put("token", &fx.bad_key());
    let replay = fx.replay("refused");
    let p = providers::ChatGptSignIn::new(replay.clone(), vault.store.clone(), None, Some("acct-fixture-0001".into()));
    let (_, why, _) = words(ListModels::execute(&p, Some(&key)).unwrap_err());
    assert_eq!(why, "Your authentication token has expired. Please try signing in again.");
    replay.assert_done();
}
#[test]
fn ollama_refusal_is_the_servers_words_about_a_missing_model() {
    let fx = Fixture::load("providers", "ollama");
    let vault = Vault::new("ollama-refused");
    let replay = fx.replay("refused");
    let p = adapter(ProviderId::Ollama, replay.clone(), &vault);
    let (who, why, retry) = words(CompleteText::execute(&*p, None, &request("ghost-model")).unwrap_err());
    assert_eq!((who.as_str(), why.as_str(), retry), ("Ollama", "model \"ghost-model\" not found, try pulling it first", false));
    replay.assert_done();
}

#[test]
fn openai_completes_through_the_responses_api() { completes("openai", ProviderId::OpenAi, "fixture-model-a"); }
#[test]
fn anthropic_completes_through_messages() { completes("anthropic", ProviderId::Anthropic, "fixture-claude-a"); }
#[test]
fn gemini_completes_through_generate_content() { completes("gemini", ProviderId::Gemini, "models/fixture-gemini-a"); }
#[test]
fn openrouter_completes_through_chat_completions() { completes("openrouter", ProviderId::OpenRouter, "fixture-vendor/model-a"); }
#[test]
fn ollama_completes_with_no_key() { completes("ollama", ProviderId::Ollama, "fixture-llama:latest"); }
#[test]
fn lmstudio_completes_with_no_key() { completes("lmstudio", ProviderId::LmStudio, "fixture-qwen-7b"); }
#[test]
fn chatgpt_sign_in_completes_through_responses() {
    let fx = Fixture::load("providers", "chatgpt");
    let vault = Vault::new("chatgpt-complete");
    let key = vault.put("token", &fx.key());
    let replay = fx.replay("complete");
    let p = providers::ChatGptSignIn::new(replay.clone(), vault.store.clone(), None, Some("acct-fixture-0001".into()));
    assert_eq!(CompleteText::execute(&p, Some(&key), &request("fixture-chatgpt-a")).unwrap(), ANSWER);
    replay.assert_done();
}

#[test]
fn lmstudio_sends_the_key_when_the_owner_set_one() {
    let fx = Fixture::load("providers", "lmstudio");
    let vault = Vault::new("lm-key");
    let key = vault.put("k", &fx.key());
    let replay = fx.replay("list_models_with_key");
    let p = adapter(ProviderId::LmStudio, replay.clone(), &vault);
    assert_eq!(ListModels::execute(&*p, Some(&key)).unwrap(), ["fixture-qwen-7b"]);
    replay.assert_done();
}

#[test]
fn a_provider_that_echoes_the_whole_key_never_shows_it() {
    let fx = Fixture::load("providers", "openai");
    let vault = Vault::new("echo");
    let bad = fx.bad_key();
    let key = vault.put("k", &bad);
    let p = adapter(ProviderId::OpenAi, fx.replay("refused_echo"), &vault);
    let (_, why, _) = words(ListModels::execute(&*p, Some(&key)).unwrap_err());
    assert!(!why.contains(&bad), "the key leaked into the words: {why}");
    assert_eq!(why, "Incorrect API key provided: <key>.");
}

#[test]
fn a_rate_limit_can_be_retried_and_a_refused_key_cannot() {
    let fx = Fixture::load("providers", "openai");
    let vault = Vault::new("rate");
    let key = vault.put("k", &fx.key());
    let p = adapter(ProviderId::OpenAi, fx.replay("rate_limited"), &vault);
    let (_, why, retry) = words(CompleteText::execute(&*p, Some(&key), &request("fixture-model-a")).unwrap_err());
    assert!(retry, "a 429 is retryable");
    assert_eq!(why, "Rate limit reached for fixture-model-a. Please try again in 20s.");
}

#[test]
fn a_missing_keychain_item_is_not_found_and_no_request_is_made() {
    let fx = Fixture::load("providers", "openai");
    let vault = Vault::new("missing");
    let replay = fx.replay("list_models");
    let p = adapter(ProviderId::OpenAi, replay.clone(), &vault);
    let ghost = contracts::KeychainRef { service: format!("{}provider-credential", secrets::Namespace::rig_run("missing").unwrap().prefix()), account: "ghost".into() };
    let e = ListModels::execute(&*p, Some(&ghost)).unwrap_err();
    assert!(matches!(e, CoreError::NotFound { .. }), "{e:?}");
    assert!(replay.seen().is_empty());
}

#[test]
fn a_cloud_provider_with_no_key_is_refused_before_any_request() {
    for p in PROVIDERS.iter().filter(|p| p.needs_key) {
        let vault = Vault::new("nokey");
        let replay = Fixture::load("providers", "openai").replay("list_models");
        let a = adapter(p.id, replay.clone(), &vault);
        assert!(matches!(ListModels::execute(&*a, None), Err(CoreError::Invalid { .. })), "{} accepted no key", p.name);
        assert!(replay.seen().is_empty(), "{} sent a request with no key", p.name);
    }
}

#[test]
fn a_server_that_is_not_running_shows_the_real_transport_message() {
    let vault = Vault::new("closed");
    let p = adapter(ProviderId::Ollama, Arc::new(Closed("Connection refused (os error 61)")), &vault);
    let (who, why, retry) = words(ListModels::execute(&*p, None).unwrap_err());
    assert_eq!((who.as_str(), why.as_str(), retry), ("Ollama", "Connection refused (os error 61)", true));
}

#[test]
fn a_key_never_shows_in_a_debug_line_of_a_request() {
    let fx = Fixture::load("providers", "openai");
    let vault = Vault::new("debug");
    let key = vault.put("k", &fx.key());
    let replay = fx.replay("list_models");
    let p = adapter(ProviderId::OpenAi, replay.clone(), &vault);
    ListModels::execute(&*p, Some(&key)).unwrap();
    let line = format!("{:?}", replay.seen());
    assert!(line.contains("<redacted>") && !line.contains(&fx.key()), "{line}");
}

#[test]
fn the_chatgpt_option_carries_the_note_that_it_can_stop_working() {
    let row = info(ProviderId::ChatGptSignIn);
    assert!(row.sign_in);
    assert!(row.note.expect("a note").contains("can stop working"));
    assert_eq!(PROVIDERS.iter().filter(|p| p.sign_in).count(), 1, "only the ChatGPT option signs in");
}

#[test]
fn local_providers_need_no_key_and_say_nothing_leaves_the_mac() {
    for id in [ProviderId::Ollama, ProviderId::LmStudio] {
        let row = info(id);
        assert!(!row.needs_key && row.base_url.starts_with("http://localhost"));
        assert!(row.note.expect("a note").contains("Nothing leaves it"));
    }
}
