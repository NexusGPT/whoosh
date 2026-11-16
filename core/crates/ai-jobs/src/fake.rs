//! A provider that answers from a table and remembers what it was asked. For tests only: it opens no socket.
use std::collections::HashMap;
use std::sync::Mutex;

use ai_providers::{CompleteText, TextRequest};
use contracts::{CoreError, KeychainRef};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Seen {
    pub model: String,
    pub system: String,
    pub prompt: String,
    pub max_output_tokens: u32,
    pub key: Option<KeychainRef>,
}

enum Plan {
    ByJob(HashMap<String, String>),
    Always(Result<String, CoreError>),
}

pub struct FakeProvider {
    plan: Plan,
    seen: Mutex<Vec<Seen>>,
}

impl FakeProvider {
    /// Answers each job with the text filed under its id (`Job: <id>` is the first line of every system prompt).
    pub fn by_job(answers: HashMap<String, String>) -> Self {
        Self { plan: Plan::ByJob(answers), seen: Mutex::new(vec![]) }
    }
    /// Answers every request with this text.
    pub fn saying(text: &str) -> Self {
        Self { plan: Plan::Always(Ok(text.to_string())), seen: Mutex::new(vec![]) }
    }
    /// Refuses every request with this error.
    pub fn failing(error: CoreError) -> Self {
        Self { plan: Plan::Always(Err(error)), seen: Mutex::new(vec![]) }
    }
    pub fn seen(&self) -> Vec<Seen> {
        self.seen.lock().expect("fake provider log").clone()
    }
    pub fn calls(&self) -> usize {
        self.seen.lock().expect("fake provider log").len()
    }
}

impl CompleteText for FakeProvider {
    fn execute(&self, key: Option<&KeychainRef>, request: &TextRequest) -> Result<String, CoreError> {
        let system = request.system.clone().unwrap_or_default();
        self.seen.lock().expect("fake provider log").push(Seen {
            model: request.model.clone(),
            system: system.clone(),
            prompt: request.prompt.clone(),
            max_output_tokens: request.max_output_tokens,
            key: key.cloned(),
        });
        match &self.plan {
            Plan::Always(r) => r.clone(),
            Plan::ByJob(map) => {
                let id = system.lines().next().and_then(|l| l.strip_prefix("Job: ")).unwrap_or("");
                map.get(id).cloned().ok_or_else(|| CoreError::Unexpected { reason_words: format!("the fake provider has no answer for job \"{id}\"") })
            }
        }
    }
}
