//! par-ai-jobs: the four named features of the todo, one driven test each, on the fake provider.
//!   writing instructions · suggested replies · AI summaries · translate fallback through the provider
//! The app-visible half (what the owner sees on screen) is not judged here: no surface is locked-on-build.
mod support;

use std::sync::Arc;

use ai_jobs::fake::FakeProvider;
use ai_jobs::{Input, JobId, MailLine, RunJob};
use contracts::{ActionFailed, CoreError};
use serde_json::json;
use support::*;

#[test]
fn feature_writing_instructions() {
    let fake = fake_with_fixtures();
    let jobs = jobs_on(&fake);
    for id in [JobId::DraftReply, JobId::SuggestedReplies, JobId::Rewrite, JobId::DraftFollowUp] {
        let input = input_of(id);
        assert!(input.get("instructions").is_some(), "{}: the fixture carries instructions", id.name());
        jobs.execute(id, &input).unwrap();
        let prompt = fake.seen().last().unwrap().prompt.clone();
        assert!(prompt.contains("## instructions: the owner's standing writing instructions (owner's words)\nNever promise a date. Keep it under 80 words."), "{}: {prompt}", id.name());
    }
    let mut bare = input_of(JobId::DraftReply);
    bare = Input::from_json(json!({"intent": bare.get("intent").unwrap()})).unwrap();
    jobs.execute(JobId::DraftReply, &bare).unwrap();
    assert!(!fake.seen().last().unwrap().prompt.contains("## instructions"), "no instructions, no section");
}

#[test]
fn feature_suggested_replies() {
    let fake = fake_with_fixtures();
    let out = jobs_on(&fake).execute(JobId::SuggestedReplies, &input_of(JobId::SuggestedReplies)).unwrap();
    let replies = out.value["replies"].as_array().unwrap();
    assert_eq!(replies.len(), 3);
    for r in replies {
        assert!(r["label"].as_str().is_some_and(|s| !s.is_empty()) && r["text"].as_str().is_some_and(|s| !s.is_empty()), "{r}");
    }
    assert!(fake.seen()[0].prompt.contains("## count: how many replies to offer\n3"));
    // more replies than asked for is refused
    let four = json!({"replies": (0..4).map(|i| json!({"label": format!("L{i}"), "text": "t"})).collect::<Vec<_>>()});
    let fake = Arc::new(FakeProvider::saying(&four.to_string()));
    let err = jobs_on(&fake).execute(JobId::SuggestedReplies, &input_of(JobId::SuggestedReplies)).unwrap_err();
    assert!(refusal(&err).1.contains("more items than the 3 asked for"), "{err}");
}

#[test]
fn feature_ai_summaries() {
    let fake = fake_with_fixtures();
    let thread = [
        MailLine { from: "marta@linea-supply-example.com".into(), date: Some("2026-11-02".into()), text: "FIRST-LINE offer for 400 chairs".into() },
        MailLine { from: "owner@whoosh-test.example".into(), date: Some("2026-11-03".into()), text: "SECOND-LINE could March work".into() },
        MailLine { from: "marta@linea-supply-example.com".into(), date: Some("2026-11-04".into()), text: "THIRD-LINE yes at 54 euros".into() },
    ];
    let input = Input::new().messages("thread", &thread);
    let out = jobs_on(&fake).execute(JobId::SummariseThread, &input).unwrap();
    assert!(out.value["summary"].as_str().is_some_and(|s| !s.is_empty()));
    assert!(out.value["key_points"].as_array().is_some_and(|a| !a.is_empty()));
    assert!(out.value["open_questions"].is_array());
    let p = fake.seen()[0].prompt.clone();
    let (a, b, c) = (p.find("FIRST-LINE").unwrap(), p.find("SECOND-LINE").unwrap(), p.find("THIRD-LINE").unwrap());
    assert!(a < b && b < c, "every message is sent, oldest first");
    let no_points = Arc::new(FakeProvider::saying(&json!({"summary": "s", "open_questions": []}).to_string()));
    assert!(jobs_on(&no_points).execute(JobId::SummariseThread, &input).is_err(), "a summary without its key points is refused");
}

#[test]
fn feature_translate_fallback_through_the_provider() {
    let fake = fake_with_fixtures();
    let out = jobs_on(&fake).execute(JobId::Translate, &input_of(JobId::Translate)).unwrap();
    assert_eq!(out.value, json!({"text": "Hello, can you confirm the March delivery?", "source_language": "French"}));
    let p = fake.seen()[0].prompt.clone();
    assert!(p.contains("## target_language: the language to translate into (owner's words)\nEnglish"), "{p}");
    assert!(p.contains("Bonjour, pouvez-vous confirmer la livraison de mars"), "{p}");
    // the provider says no: the owner reads the provider's words
    let said = CoreError::Action { failure: ActionFailed { account_name: "Anthropic".into(), reason_words: "invalid x-api-key".into(), retryable: false, code: None } };
    let fake = Arc::new(FakeProvider::failing(said.clone()));
    assert_eq!(jobs_on(&fake).execute(JobId::Translate, &input_of(JobId::Translate)), Err(said));
}

#[test]
fn feature_draft_in_the_owners_voice_dictation_and_codes() {
    let fake = fake_with_fixtures();
    let jobs = jobs_on(&fake);
    let d = jobs.execute(JobId::DraftReply, &input_of(JobId::DraftReply)).unwrap();
    assert!(d.value["text"].as_str().unwrap().contains("March"));
    assert!(fake.seen()[0].prompt.contains("## voice: how the owner writes, from their sent mail (owner's words)\nShort sentences."));
    let c = jobs.execute(JobId::DictationCleanup, &input_of(JobId::DictationCleanup)).unwrap();
    assert_eq!(c.value["text"], "Hi Jon, can you send me the contract by Friday? Thanks.");
    let code = jobs.execute(JobId::ExtractSignInCode, &input_of(JobId::ExtractSignInCode)).unwrap();
    assert_eq!(code.value["code"], "482913");
    let n = jobs.execute(JobId::ClassifyNeedsReply, &input_of(JobId::ClassifyNeedsReply)).unwrap();
    assert_eq!(n.value["status"], "needs_reply");
}
