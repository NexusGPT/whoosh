//! fn-cloud-speech: each adapter returns text for the fixture audio, and a refused key returns the provider's words.
mod support;

use std::sync::Arc;

use ai_providers::speech;
use ai_providers::transport::Body;
use ai_providers::{AudioClip, SpeechId};
use contracts::CoreError;
use support::{fixtures, Fixture, Replay, Vault};

const TEXT: &str = "hold the shipment until Friday and copy Marta";

fn clip(language: Option<&str>) -> AudioClip {
    let bytes = std::fs::read(fixtures().join("speech/clip.wav")).expect("fixture audio");
    AudioClip { bytes, content_type: "audio/wav".into(), file_name: "clip.wav".into(), language: language.map(String::from) }
}

fn run(slug: &str, id: SpeechId, scenario: &str, key_of: fn(&Fixture) -> String, language: Option<&str>) -> (Result<String, CoreError>, Arc<Replay>) {
    let fx = Fixture::load("speech", slug);
    let vault = Vault::new(&format!("speech-{slug}"));
    let key = vault.put("k", &key_of(&fx));
    let replay = fx.replay(scenario);
    let adapter = speech::build(id, replay.clone(), vault.store.clone(), None);
    (adapter.execute(&key, &clip(language)), replay)
}

fn transcribes(slug: &str, id: SpeechId, language: Option<&str>) {
    let (out, replay) = run(slug, id, "transcribe", Fixture::key, language);
    assert_eq!(out.expect("text for the fixture audio"), TEXT);
    replay.assert_done();
    // The clip itself went out, byte for byte.
    let sent = &replay.seen()[0];
    let data = match &sent.body { Body::Bytes { data, .. } => data.clone(), _ => panic!("expected a bytes body") };
    let audio = clip(None).bytes;
    assert!(data.windows(audio.len()).any(|w| w == audio.as_slice()), "{slug}: the audio is not in the request");
}

fn refused(slug: &str, id: SpeechId, expected: &str) {
    let (out, replay) = run(slug, id, "refused", Fixture::bad_key, if slug == "deepgram" { Some("en") } else { None });
    match out.expect_err("a refused key is an error") {
        CoreError::Action { failure } => {
            assert_eq!(failure.reason_words, expected);
            assert!(!failure.retryable);
        }
        other => panic!("expected the provider's refusal, got {other:?}"),
    }
    replay.assert_done();
}

#[test]
fn openai_transcription_returns_text() { transcribes("openai", SpeechId::OpenAi, None); }
#[test]
fn deepgram_returns_text_and_sends_the_language() { transcribes("deepgram", SpeechId::Deepgram, Some("en")); }
#[test]
fn elevenlabs_scribe_returns_text() { transcribes("elevenlabs", SpeechId::ElevenLabs, None); }
#[test]
fn groq_returns_text() { transcribes("groq", SpeechId::Groq, None); }
#[test]
fn mistral_voxtral_returns_text() { transcribes("mistral", SpeechId::Mistral, None); }

#[test]
fn openai_transcription_refused_key_returns_the_providers_words() {
    refused("openai", SpeechId::OpenAi, "Incorrect API key provided: sk-FIXT****FUSED. You can find your API key at https://platform.openai.com/account/api-keys.");
}
#[test]
fn deepgram_refused_key_returns_the_providers_words() { refused("deepgram", SpeechId::Deepgram, "Invalid credentials."); }
#[test]
fn elevenlabs_refused_key_returns_the_providers_words() { refused("elevenlabs", SpeechId::ElevenLabs, "Invalid API key"); }
#[test]
fn groq_refused_key_returns_the_providers_words() { refused("groq", SpeechId::Groq, "Invalid API Key"); }
#[test]
fn mistral_refused_key_returns_the_providers_words() { refused("mistral", SpeechId::Mistral, "Unauthorized"); }

#[test]
fn an_empty_recording_is_refused_before_any_request() {
    let fx = Fixture::load("speech", "groq");
    let vault = Vault::new("speech-empty");
    let key = vault.put("k", &fx.key());
    let replay = fx.replay("transcribe");
    let adapter = speech::build(SpeechId::Groq, replay.clone(), vault.store.clone(), None);
    let empty = AudioClip { bytes: Vec::new(), content_type: "audio/wav".into(), file_name: "x.wav".into(), language: None };
    assert!(matches!(adapter.execute(&key, &empty), Err(CoreError::Invalid { .. })));
    assert!(replay.seen().is_empty());
}

#[test]
fn a_missing_key_is_not_found_and_no_audio_leaves() {
    let fx = Fixture::load("speech", "openai");
    let vault = Vault::new("speech-missing");
    let replay = fx.replay("transcribe");
    let adapter = speech::build(SpeechId::OpenAi, replay.clone(), vault.store.clone(), None);
    let ghost = contracts::KeychainRef { service: format!("{}provider-credential", secrets::Namespace::rig_run("speech-missing").unwrap().prefix()), account: "ghost".into() };
    assert!(matches!(adapter.execute(&ghost, &clip(None)), Err(CoreError::NotFound { .. })));
    assert!(replay.seen().is_empty());
}

#[test]
fn the_boundary_never_appears_inside_the_audio() {
    let mut audio = clip(None);
    audio.bytes.extend_from_slice(b"whoosh-form-boundary");
    let fx = Fixture::load("speech", "groq");
    let vault = Vault::new("speech-boundary");
    let key = vault.put("k", &fx.key());
    let replay = fx.replay("transcribe");
    let adapter = speech::build(SpeechId::Groq, replay.clone(), vault.store.clone(), None);
    adapter.execute(&key, &audio).unwrap();
    let Body::Bytes { content_type, data } = &replay.seen()[0].body else { panic!("bytes") };
    let boundary = content_type.split("boundary=").nth(1).unwrap();
    assert_ne!(boundary, "whoosh-form-boundary");
    let count = data.windows(boundary.len()).filter(|w| *w == boundary.as_bytes()).count();
    assert_eq!(count, 3, "the boundary opens the model part, opens the file part and closes the form, and is in no audio byte");
}
