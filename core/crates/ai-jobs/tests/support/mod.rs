#![allow(dead_code)]
//! Shared by the tests: the hand-written fixture (an input and the model's answer per job) and a Jobs wired to the fake provider.
use std::collections::HashMap;
use std::sync::Arc;

use ai_jobs::fake::FakeProvider;
use ai_jobs::{Input, JobId, Jobs};
use contracts::{CoreError, KeychainRef};
use serde_json::Value;

pub const MAX_TOKENS: u32 = 777;

pub fn fixtures() -> HashMap<String, Value> {
    let raw = include_str!("../../fixtures/jobs.json");
    serde_json::from_str::<HashMap<String, Value>>(raw).expect("fixtures/jobs.json is JSON")
}

pub fn input_of(id: JobId) -> Input {
    Input::from_json(fixtures()[id.name()]["input"].clone()).expect("fixture input is an object")
}

pub fn answer_of(id: JobId) -> Value {
    fixtures()[id.name()]["answer"].clone()
}

/// Every job answers with its fixture answer.
pub fn fake_with_fixtures() -> Arc<FakeProvider> {
    let answers = fixtures().into_iter().map(|(k, v)| (k, v["answer"].to_string())).collect();
    Arc::new(FakeProvider::by_job(answers))
}

pub fn jobs_on(fake: &Arc<FakeProvider>) -> Jobs {
    Jobs::new(fake.clone(), "fake-model", None, MAX_TOKENS)
}

pub fn jobs_with_key(fake: &Arc<FakeProvider>, key: KeychainRef) -> Jobs {
    Jobs::new(fake.clone(), "fake-model", Some(key), MAX_TOKENS)
}

/// The words of a refusal; panics when the error is not an Action refusal.
pub fn refusal(e: &CoreError) -> (&str, &str, bool) {
    match e {
        CoreError::Action { failure } => (&failure.account_name, &failure.reason_words, failure.retryable),
        other => panic!("expected an Action refusal, got {other:?}"),
    }
}
