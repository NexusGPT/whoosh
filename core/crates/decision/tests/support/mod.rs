#![allow(dead_code)]
//! Shared by the decision tests: a fake local runtime (it reads the Jev-shaped request it is handed and answers the
//! Jev-shaped object), a recording engine, a replaying HTTP transport for the recorded Jev fixture, a movable clock,
//! hand-made messages. The store seeding is the smart-inbox lane's own helper, compiled in by path (read only).
use contracts::{Clock, CoreError, DecisionPort, DecisionProviderKind, DecisionQuestion, DecisionResult, DecisionState, LocalModel};
use decision::adapters::laya_budget::LayaShape;
use decision::adapters::local::LocalDecision;
use serde_json::{json, Value};
use smart_inbox::MessageFacts;
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::{Arc, Mutex};

#[path = "../../../smart-inbox/tests/support/mod.rs"]
pub mod si;

/// Answers one question: (question id, the question object, the state text) -> the answer object, or None (no answer).
pub type Answerer = dyn Fn(&str, &Value, &str) -> Option<Value> + Send + Sync;

pub struct FakeLocal {
    pub answer: Box<Answerer>,
    /// Every prompt handed over, with its max_tokens.
    pub prompts: Mutex<Vec<(String, u32)>>,
}

impl FakeLocal {
    pub fn new(answer: impl Fn(&str, &Value, &str) -> Option<Value> + Send + Sync + 'static) -> Arc<FakeLocal> {
        Arc::new(FakeLocal { answer: Box::new(answer), prompts: Mutex::new(vec![]) })
    }
    pub fn asked_ids(&self) -> Vec<String> {
        self.prompts.lock().unwrap().iter().flat_map(|(p, _)| serde_json::from_str::<Value>(p).unwrap()["questions"].as_object().unwrap().keys().cloned().collect::<Vec<_>>()).collect()
    }
}

impl LocalModel for FakeLocal {
    fn generate(&self, prompt: String, max_tokens: u32) -> Result<String, CoreError> {
        self.prompts.lock().unwrap().push((prompt.clone(), max_tokens));
        let req: Value = serde_json::from_str(&prompt).map_err(|e| CoreError::Unexpected { reason_words: e.to_string() })?;
        let state = req["state"].as_str().unwrap_or("").to_string();
        let mut answers = serde_json::Map::new();
        for (id, q) in req["questions"].as_object().unwrap() {
            // lane fix-aisort round 3: a per-label yes/no question (`labels/model#<label>`) is answered by the question's form: the answerer
            // speaks as for the one choice (its pick at p); the label it names gets p, every other label FAKE_NO (decision::test_answers)
            let per_label = q["type"].as_str() == Some("noul") && id.contains(decision::decide::LABEL_QUESTION_SEP);
            if per_label {
                let asked = decision::decide::statement_label(q["instructions"].as_str().unwrap_or("")).map(|(p, _)| p.to_string()).unwrap_or_default();
                let said = (self.answer)(id, q, &state);
                let yes = match said.as_ref().and_then(|a| a["choice"].as_str().map(|c| (c.to_string(), a["confidence"].as_f64().unwrap_or(0.0)))) {
                    Some((pick, p)) => decision::test_answers::yes_for(&asked, Some(&pick), p),
                    None => said.as_ref().and_then(|a| a["noul"].as_f64()).unwrap_or(decision::test_answers::FAKE_NO),
                };
                answers.insert(id.clone(), json!({"type": "noul", "noul": yes}));
                continue;
            }
            let a = (self.answer)(id, q, &state).unwrap_or_else(|| match q["type"].as_str() {
                Some("choice") => json!({"type": "choice", "choice": q["criteria"].as_object().unwrap().keys().next().unwrap(), "confidence": 0.1}),
                _ => json!({"type": "noul", "noul": 0.5}),
            });
            answers.insert(id.clone(), a);
        }
        Ok(json!({"answers": answers}).to_string())
    }
}

pub fn noul(p: f64) -> Option<Value> {
    Some(json!({"type": "noul", "noul": p}))
}
pub fn choice(c: &str, p: f64) -> Option<Value> {
    Some(json!({"type": "choice", "choice": c, "confidence": p}))
}

/// A shape any real checkpoint config would fill in; here a test input, not a product number.
pub const TEST_SHAPE: LayaShape = LayaShape { max_len: 512, head_max_len: 128 };

pub fn local(fake: Arc<FakeLocal>) -> LocalDecision {
    LocalDecision { model: fake, model_id: "laya-test".into(), shape: TEST_SHAPE }
}

/// Wraps an engine and records each call.
pub struct Recording<P: DecisionPort> {
    pub inner: P,
    pub calls: Mutex<Vec<Vec<DecisionQuestion>>>,
}
impl<P: DecisionPort> Recording<P> {
    pub fn new(inner: P) -> Recording<P> {
        Recording { inner, calls: Mutex::new(vec![]) }
    }
    pub fn ids(&self) -> Vec<String> {
        self.calls.lock().unwrap().iter().flatten().map(|q| decision::adapters::wire::question_id(q).to_string()).collect()
    }
}
impl<P: DecisionPort> DecisionPort for Recording<P> {
    fn provider(&self) -> DecisionProviderKind {
        self.inner.provider()
    }
    fn decide(&self, state: DecisionState, questions: Vec<DecisionQuestion>) -> Result<DecisionResult, CoreError> {
        self.calls.lock().unwrap().push(questions.clone());
        self.inner.decide(state, questions)
    }
}

/// A clock that moves by `step` ms each time it is read.
pub struct StepClock {
    pub at: AtomicI64,
    pub step: i64,
}
impl Clock for StepClock {
    fn now_ms(&self) -> i64 {
        self.at.fetch_add(self.step, Ordering::SeqCst)
    }
    fn utc_offset_s(&self) -> i32 {
        0
    }
}

pub fn msg(id: &str, from: &str, subject: &str, body: &str) -> MessageFacts {
    MessageFacts {
        message_id: id.into(),
        thread_id: format!("t-{id}"),
        account_id: "acc-1".into(),
        from_address: from.into(),
        to: vec!["owner@whoosh-example.com".into()],
        subject: subject.into(),
        body: body.into(),
        received_at_ms: 1_000,
        ..Default::default()
    }
}

/// The state text a fake reads, for matching on the subject.
pub fn subject_in(state: &str) -> String {
    state.lines().find_map(|l| l.strip_prefix("subject: ")).unwrap_or("").to_string()
}
