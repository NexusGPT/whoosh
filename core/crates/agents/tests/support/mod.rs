//! The test rig: the mail rows of docs/design/fixtures/mac-main-sample.json (the rows the Ask panel shows), a scripted
//! provider that opens no socket, a label book standing in for the actions crate, and the crate's in-memory books.
#![allow(dead_code)]
use std::collections::{HashMap, HashSet, VecDeque};
use std::path::PathBuf;
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::{Arc, Mutex};

use agents::adapters::memory::{MemoryConsent, MemoryProposals};
use agents::ports::{AgentEvents, ApplyCommand, BoundProvider, Clock, MailExcerpt, ReadMail, ResolveProvider, SearchMail};
use agents::{AgentEvent, AgentPorts, AgentRuntime, AskOutcome, AskRequest, AskTurn};
use ai_providers::{CompleteText, TextRequest};
use contracts::{AgentConfig, AgentPermission, Command, CommandOutcome, CoreError, KeychainRef, SearchHit};
use serde_json::Value;

/// The answer ceiling the core passes in from the ledger; any value works for a fake provider.
pub const MAX_TOKENS: u32 = 700;
/// Received time of the first fixture row; each later row is one minute older (the fixture gives display words only).
pub const FIRST_RECEIVED_MS: i64 = 1_762_500_000_000;

pub fn fixture_rows() -> Vec<MailExcerpt> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../docs/design/fixtures/mac-main-sample.json");
    let doc: Value = serde_json::from_str(&std::fs::read_to_string(&path).expect("mac-main-sample.json")).expect("fixture is JSON");
    let accounts: HashMap<String, String> = doc["accounts"]
        .as_array()
        .expect("accounts")
        .iter()
        .map(|a| (a["id"].as_str().unwrap_or_default().to_string(), a["name"].as_str().unwrap_or_default().to_string()))
        .collect();
    doc["messages"]
        .as_array()
        .expect("messages")
        .iter()
        .enumerate()
        .map(|(i, m)| {
            let s = |k: &str| m[k].as_str().unwrap_or_default().to_string();
            let account_id = s("account");
            MailExcerpt {
                message_id: s("id"),
                thread_id: s("id"),
                account_name: accounts.get(&account_id).cloned().unwrap_or_default(),
                account_id,
                sender_name: s("sender"),
                subject: s("subject"),
                received_at_ms: FIRST_RECEIVED_MS - 60_000 * i64::try_from(i).expect("row index"),
                text: s("preview"),
            }
        })
        .collect()
}

pub struct FixtureMail {
    pub rows: Vec<MailExcerpt>,
    pub queries: Mutex<Vec<(String, usize)>>,
    pub read: Mutex<Vec<String>>,
}

impl FixtureMail {
    pub fn ids(&self) -> HashSet<String> {
        self.rows.iter().map(|r| r.message_id.clone()).collect()
    }
}

impl SearchMail for FixtureMail {
    fn execute(&self, query: &str, limit: usize) -> Result<Vec<SearchHit>, CoreError> {
        self.queries.lock().unwrap().push((query.to_string(), limit));
        let words: Vec<String> = query.to_lowercase().split(|c: char| !c.is_alphanumeric()).filter(|w| w.len() > 3).map(str::to_string).collect();
        Ok(self
            .rows
            .iter()
            .filter(|r| {
                let hay = format!("{} {} {}", r.sender_name, r.subject, r.text).to_lowercase();
                words.iter().any(|w| hay.contains(w.as_str()))
            })
            .take(limit)
            .map(|r| SearchHit { message_id: r.message_id.clone(), score: 1.0, snippet: r.subject.clone(), matched_in: contracts::MatchedIn::MeaningOnly, attachment_name: None, attachment_note: None })
            .collect())
    }
}

impl ReadMail for FixtureMail {
    fn execute(&self, message_ids: &[String]) -> Result<Vec<MailExcerpt>, CoreError> {
        self.read.lock().unwrap().extend(message_ids.iter().cloned());
        Ok(message_ids.iter().filter_map(|id| self.rows.iter().find(|r| &r.message_id == id).cloned()).collect())
    }
}

#[derive(Debug, Clone)]
pub struct Seen {
    pub system: String,
    pub prompt: String,
    pub model: String,
    pub max_output_tokens: u32,
}

/// Answers each call with the next scripted reply; the last one repeats. Opens no socket.
pub struct ScriptedProvider {
    replies: Mutex<VecDeque<String>>,
    seen: Mutex<Vec<Seen>>,
}

impl ScriptedProvider {
    pub fn new(replies: &[&str]) -> ScriptedProvider {
        ScriptedProvider { replies: Mutex::new(replies.iter().map(|r| r.to_string()).collect()), seen: Mutex::new(Vec::new()) }
    }
    pub fn script(&self, replies: &[&str]) {
        *self.replies.lock().unwrap() = replies.iter().map(|r| r.to_string()).collect();
    }
    pub fn seen(&self) -> Vec<Seen> {
        self.seen.lock().unwrap().clone()
    }
    pub fn calls(&self) -> usize {
        self.seen.lock().unwrap().len()
    }
}

impl CompleteText for ScriptedProvider {
    fn execute(&self, _key: Option<&KeychainRef>, request: &TextRequest) -> Result<String, CoreError> {
        self.seen.lock().unwrap().push(Seen {
            system: request.system.clone().unwrap_or_default(),
            prompt: request.prompt.clone(),
            model: request.model.clone(),
            max_output_tokens: request.max_output_tokens,
        });
        let mut replies = self.replies.lock().unwrap();
        let next = if replies.len() > 1 { replies.pop_front() } else { replies.front().cloned() };
        next.ok_or_else(|| CoreError::Unexpected { reason_words: "the scripted provider has no reply".into() })
    }
}

/// provider id → (name, on device, client)
pub struct Providers(pub HashMap<String, (String, bool, Arc<ScriptedProvider>)>);

impl ResolveProvider for Providers {
    fn execute(&self, provider_id: &str) -> Result<BoundProvider, CoreError> {
        let (name, on_device, client) = self.0.get(provider_id).ok_or_else(|| CoreError::NotFound { kind: "provider".into(), id: provider_id.into() })?;
        Ok(BoundProvider { name: name.clone(), on_device: *on_device, key: None, client: client.clone() })
    }
}

/// Stands in for actions::Actions::run: label assignments as (label, message id) pairs.
#[derive(Default)]
pub struct LabelBook {
    pub assignments: Mutex<HashSet<(String, String)>>,
    pub applied: Mutex<Vec<Command>>,
    pub refuse_next: Mutex<bool>,
}

impl LabelBook {
    pub fn assignments(&self) -> HashSet<(String, String)> {
        self.assignments.lock().unwrap().clone()
    }
    pub fn applied(&self) -> usize {
        self.applied.lock().unwrap().len()
    }
}

impl ApplyCommand for LabelBook {
    fn execute(&self, command: Command) -> Result<CommandOutcome, CoreError> {
        if std::mem::take(&mut *self.refuse_next.lock().unwrap()) {
            return Err(CoreError::Action {
                failure: contracts::ActionFailed { account_name: "Studio".into(), reason_words: "the server refused the label".into(), retryable: true, code: None },
            });
        }
        self.applied.lock().unwrap().push(command.clone());
        if let Command::Label { message_ids, label_name, .. } = command {
            let mut a = self.assignments.lock().unwrap();
            for id in message_ids {
                a.insert((label_name.clone(), id));
            }
        }
        Ok(CommandOutcome { undo_entry_id: None, created_id: None })
    }
}

#[derive(Default)]
pub struct Events(pub Mutex<Vec<AgentEvent>>);

impl Events {
    pub fn all(&self) -> Vec<AgentEvent> {
        self.0.lock().unwrap().clone()
    }
    pub fn count(&self, pick: impl Fn(&AgentEvent) -> bool) -> usize {
        self.all().iter().filter(|e| pick(e)).count()
    }
}

impl AgentEvents for Events {
    fn execute(&self, event: AgentEvent) {
        self.0.lock().unwrap().push(event);
    }
}

pub struct TickClock(AtomicI64);

impl Clock for TickClock {
    fn execute(&self) -> i64 {
        self.0.fetch_add(1, Ordering::SeqCst)
    }
}

pub struct Harness {
    pub mail: Arc<FixtureMail>,
    pub provider: Arc<ScriptedProvider>,
    pub local: Arc<ScriptedProvider>,
    pub labels: Arc<LabelBook>,
    pub consent: Arc<MemoryConsent>,
    pub proposals: Arc<MemoryProposals>,
    pub events: Arc<Events>,
    pub runtime: AgentRuntime,
}

impl Harness {
    /// "cloud" and "cloud-b" are off-device providers sharing one script; "local" runs on the device.
    pub fn new(script: &[&str]) -> Harness {
        let mail = Arc::new(FixtureMail { rows: fixture_rows(), queries: Mutex::new(Vec::new()), read: Mutex::new(Vec::new()) });
        let provider = Arc::new(ScriptedProvider::new(script));
        let local = Arc::new(ScriptedProvider::new(script));
        let providers = Providers(HashMap::from([
            ("cloud".to_string(), ("OpenAI".to_string(), false, provider.clone())),
            ("cloud-b".to_string(), ("Gemini".to_string(), false, provider.clone())),
            ("local".to_string(), ("Ollama".to_string(), true, local.clone())),
        ]));
        let labels = Arc::new(LabelBook::default());
        let consent = Arc::new(MemoryConsent::default());
        let proposals = Arc::new(MemoryProposals::default());
        let events = Arc::new(Events::default());
        let ports = AgentPorts {
            providers: Arc::new(providers),
            search: mail.clone(),
            read: mail.clone(),
            apply: labels.clone(),
            consent_given: consent.clone(),
            record_consent: consent.clone(),
            save_proposal: proposals.clone(),
            decide_proposal: proposals.clone(),
            events: events.clone(),
            clock: Arc::new(TickClock(AtomicI64::new(1_000))),
        };
        Harness { mail, provider, local, labels, consent, proposals, events, runtime: AgentRuntime::new(ports, MAX_TOKENS) }
    }

    pub fn agent(&self, allowed: &[AgentPermission]) -> AgentConfig {
        AgentConfig {
            id: "supplier-desk".into(),
            name: "Supplier desk".into(),
            purpose: "Answers supplier questions".into(),
            provider_id: "cloud".into(),
            model: "fake-model".into(),
            instructions: String::new(),
            allowed: allowed.to_vec(),
            built_in: false,
        }
    }

    pub fn ask(&self, agent: AgentConfig, question: &str) -> AskOutcome {
        self.runtime.ask(AskRequest { conversation_id: "conv-1".into(), agent, question: question.into() }).expect("ask runs")
    }

    pub fn finished(&self, agent: AgentConfig, question: &str) -> AskTurn {
        match self.ask(agent, question) {
            AskOutcome::Finished(turn) => turn,
            other => panic!("expected an answer, got {other:?}"),
        }
    }
}
