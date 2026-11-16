//! Fake ports for the tests: a Sent folder in memory and a model that answers what it is told.
#![allow(dead_code)]
use std::sync::Mutex;

use contracts::CoreError;
use reply_voice::{SentMail, SentMessage, SentQuery, VoiceRequest, VoiceWriter};

pub fn msg(thread: &str, to: &str, date_ms: i64, text: &str) -> SentMessage {
    SentMessage { thread_id: thread.into(), recipients: vec![to.into()], date_ms, text: text.into() }
}

/// A Sent folder. `honest` = it answers the query as a store would (filtered, newest first, cut to the limit); otherwise it hands back everything it
/// holds in the order it holds it, as a careless adapter would.
pub struct FakeSent {
    pub messages: Vec<SentMessage>,
    pub honest: bool,
    pub fail_with: Option<CoreError>,
    pub calls: Mutex<Vec<(SentQuery, usize)>>,
}

impl FakeSent {
    pub fn honest(messages: Vec<SentMessage>) -> Self {
        Self { messages, honest: true, fail_with: None, calls: Mutex::new(Vec::new()) }
    }
    pub fn careless(messages: Vec<SentMessage>) -> Self {
        Self { messages, honest: false, fail_with: None, calls: Mutex::new(Vec::new()) }
    }
    pub fn asked(&self) -> Vec<(SentQuery, usize)> {
        self.calls.lock().unwrap().clone()
    }
}

impl SentMail for FakeSent {
    fn execute(&self, query: &SentQuery, limit: usize) -> Result<Vec<SentMessage>, CoreError> {
        self.calls.lock().unwrap().push((query.clone(), limit));
        if let Some(e) = &self.fail_with {
            return Err(e.clone());
        }
        if !self.honest {
            return Ok(self.messages.clone());
        }
        let mut out: Vec<SentMessage> = self
            .messages
            .iter()
            .filter(|m| match query {
                SentQuery::Latest => true,
                SentQuery::ToRecipient { recipient } => m.recipients.iter().any(|r| r.eq_ignore_ascii_case(recipient)),
                SentQuery::InThread { thread_id } => &m.thread_id == thread_id,
            })
            .cloned()
            .collect();
        out.sort_by(|a, b| b.date_ms.cmp(&a.date_ms));
        out.truncate(limit);
        Ok(out)
    }
}

pub struct FakeWriter {
    pub answer: Result<String, CoreError>,
    pub requests: Mutex<Vec<VoiceRequest>>,
}

impl FakeWriter {
    pub fn answering(text: &str) -> Self {
        Self { answer: Ok(text.into()), requests: Mutex::new(Vec::new()) }
    }
    pub fn failing(e: CoreError) -> Self {
        Self { answer: Err(e), requests: Mutex::new(Vec::new()) }
    }
    pub fn calls(&self) -> usize {
        self.requests.lock().unwrap().len()
    }
    pub fn last(&self) -> VoiceRequest {
        self.requests.lock().unwrap().last().cloned().expect("the model was not called")
    }
}

impl VoiceWriter for FakeWriter {
    fn execute(&self, request: &VoiceRequest) -> Result<String, CoreError> {
        self.requests.lock().unwrap().push(request.clone());
        self.answer.clone()
    }
}
