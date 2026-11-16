//! Ask: one question to one agent (C-J5.4, C-J5.6; the Writer's draft, C-J3.3). The loop calls the model at most
//! AGENT_MAX_STEPS times; each reply is a tool call the agent is allowed, or the answer.
use ai_providers::TextRequest;
use contracts::constants::{AGENT_CONTEXT_CHUNKS, AGENT_MAX_STEPS, SEARCH_SNIPPET_CHARS};
use contracts::{AgentConfig, AgentProposal, Command, CoreError, ProposalDecision};
use serde_json::Value;

use contracts::AnswerKind;
use crate::domain::answer::{AgentAnswer, CitedMessage, NOTHING_FOUND};
use crate::domain::event::AgentEvent;
use crate::domain::permission::{offered, Tool};
use crate::domain::prompt;
use crate::domain::proposal::StoredProposal;
use crate::domain::reply::{self, ModelReply};
use crate::ports::MailExcerpt;
use crate::runtime::AgentPorts;

pub struct AskRequest {
    /// The conversation the answer belongs to; `AgentDelta` events carry it.
    pub conversation_id: String,
    pub agent: AgentConfig,
    pub question: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AskTurn {
    pub answer: AgentAnswer,
    /// Proposals made in this run, all still pending.
    pub proposals: Vec<AgentProposal>,
    /// Model calls made.
    pub steps: u32,
    /// True when the run hit AGENT_MAX_STEPS.
    pub stopped: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub enum AskOutcome {
    /// Nothing was sent. Ask the owner; on yes, `GiveConsent` then ask again.
    NeedsConsent { provider_id: String, provider_name: String },
    Finished(AskTurn),
}

pub struct Ask {
    ports: AgentPorts,
    max_output_tokens: u32,
}

/// What one run has gathered: the mail shown to the model (citations resolve against it only), the transcript, proposals.
struct Run {
    shown: Vec<MailExcerpt>,
    so_far: Vec<String>,
    proposals: Vec<AgentProposal>,
}

fn max_steps() -> u32 {
    u32::try_from(AGENT_MAX_STEPS).unwrap_or(u32::MAX)
}

fn context_room(run: &Run) -> usize {
    usize::try_from(AGENT_CONTEXT_CHUNKS).unwrap_or(usize::MAX).saturating_sub(run.shown.len())
}

impl Ask {
    pub fn new(ports: AgentPorts, max_output_tokens: u32) -> Ask {
        Ask { ports, max_output_tokens }
    }

    pub fn execute(&self, request: AskRequest) -> Result<AskOutcome, CoreError> {
        let AskRequest { conversation_id, agent, question } = request;
        let provider = self.ports.providers.execute(&agent.provider_id)?;
        if !provider.on_device && !self.ports.consent_given.execute(&agent.provider_id) {
            self.emit(AgentEvent::ConsentNeeded { provider_id: agent.provider_id.clone(), provider_name: provider.name.clone() });
            return Ok(AskOutcome::NeedsConsent { provider_id: agent.provider_id, provider_name: provider.name });
        }
        let tools = offered(&agent.allowed);
        let system = prompt::system(&agent, &tools);
        let mut run = Run { shown: Vec::new(), so_far: Vec::new(), proposals: Vec::new() };
        for step in 1..=max_steps() {
            let call = TextRequest {
                model: agent.model.clone(),
                system: Some(system.clone()),
                prompt: prompt::turn(&question, &run.so_far),
                max_output_tokens: self.max_output_tokens,
            };
            let raw = provider.client.execute(provider.key.as_ref(), &call).inspect_err(|e| {
                if let CoreError::Action { failure } = e {
                    self.emit(AgentEvent::Failed { failure: failure.clone() });
                }
            })?;
            match reply::parse(&raw) {
                ModelReply::Answer { text, cite } => {
                    self.emit(AgentEvent::Step { number: step, words: "Wrote the answer".into() });
                    return Ok(AskOutcome::Finished(self.finish(run, AnswerKind::Answer, text, &cite, step)));
                }
                ModelReply::Call { tool, args } => match Tool::from_name(&tool).filter(|t| tools.contains(t)) {
                    None => {
                        run.so_far.push(format!("{tool}: this agent may not do that. Use only the tools listed."));
                        self.emit(AgentEvent::Step { number: step, words: format!("Asked to {tool}, which {} may not do", agent.name) });
                    }
                    Some(Tool::Draft) => {
                        self.emit(AgentEvent::Step { number: step, words: "Wrote a draft".into() });
                        let body = reply::text(&args["body"]);
                        let cite = reply::ids(&args["cite"]);
                        return Ok(AskOutcome::Finished(self.finish(run, AnswerKind::Draft, body, &cite, step)));
                    }
                    Some(tool) => {
                        let words = self.call(tool, &args, &mut run, &conversation_id, &agent)?;
                        self.emit(AgentEvent::Step { number: step, words });
                    }
                },
            }
        }
        let steps = max_steps();
        let words = format!("Stopped after {steps} steps without an answer. Ask a narrower question.");
        self.emit(AgentEvent::Stopped { steps, words: words.clone() });
        let answer = AgentAnswer { kind: AnswerKind::Answer, text: words, citations: Vec::new() };
        Ok(AskOutcome::Finished(AskTurn { answer, proposals: run.proposals, steps, stopped: true }))
    }

    fn emit(&self, event: AgentEvent) {
        self.ports.events.execute(event);
    }

    /// Runs one allowed tool; returns what the agent did, in words.
    fn call(&self, tool: Tool, args: &Value, run: &mut Run, conversation_id: &str, agent: &AgentConfig) -> Result<String, CoreError> {
        match tool {
            Tool::Search => self.search(&reply::text(&args["query"]), run),
            Tool::ReadMail => self.read(reply::ids(&args["message_ids"]), run),
            Tool::ProposeLabel => self.propose_label(args, run, conversation_id, agent),
            Tool::ProposeSend => self.propose_send(&reply::text(&args["draft_id"]), run, conversation_id, agent),
            Tool::Draft => Ok("Wrote a draft".into()),
        }
    }

    fn search(&self, query: &str, run: &mut Run) -> Result<String, CoreError> {
        if query.is_empty() {
            run.so_far.push("search: give the words to search for.".into());
            return Ok("Search needed words".into());
        }
        let room = context_room(run);
        if room == 0 {
            run.so_far.push("search: you were shown as many messages as may leave the device. Answer now.".into());
            return Ok("Reached the excerpt limit".into());
        }
        let hits = self.ports.search.execute(query, room)?;
        let fresh: Vec<String> = hits.iter().map(|h| h.message_id.clone()).filter(|id| !run.shown.iter().any(|m| &m.message_id == id)).take(room).collect();
        let found = self.ports.read.execute(&fresh)?;
        run.so_far.push(format!("search \"{query}\": {} messages", found.len()));
        for m in &found {
            let snippet = hits.iter().find(|h| h.message_id == m.message_id).map(|h| h.snippet.as_str()).unwrap_or_default();
            run.so_far.push(format!("[{}] {} · {} · {}: {}", m.message_id, m.sender_name, m.account_name, m.subject, snippet));
        }
        let n = found.len();
        run.shown.extend(found);
        Ok(format!("Searched your mail for \u{201c}{query}\u{201d}: {n} messages"))
    }

    fn read(&self, ids: Vec<String>, run: &mut Run) -> Result<String, CoreError> {
        let room = context_room(run);
        let fresh: Vec<String> = ids.into_iter().filter(|id| !run.shown.iter().any(|m| &m.message_id == id)).take(room).collect();
        let found = self.ports.read.execute(&fresh)?;
        run.so_far.push(format!("read_mail: {} messages", found.len()));
        for m in &found {
            run.so_far.push(format!("[{}] {} · {} · {}\n{}", m.message_id, m.sender_name, m.account_name, m.subject, m.text));
        }
        let n = found.len();
        run.shown.extend(found);
        Ok(format!("Read {n} messages"))
    }

    fn proposal_id(&self, conversation_id: &str, run: &Run) -> String {
        format!("{conversation_id}-{}-{}", self.ports.clock.execute(), run.proposals.len() + 1)
    }

    fn store(&self, proposal: AgentProposal, command: Command, run: &mut Run) -> Result<String, CoreError> {
        self.ports.save_proposal.execute(StoredProposal { proposal: proposal.clone(), command })?;
        run.so_far.push(format!("proposal {} waits for the owner's approval. Nothing has changed.", proposal.id));
        let words = format!("{}, waiting for your approval", proposal.action_words);
        self.emit(AgentEvent::ProposalReady { proposal: proposal.clone() });
        run.proposals.push(proposal);
        Ok(words)
    }

    fn propose_label(&self, args: &Value, run: &mut Run, conversation_id: &str, agent: &AgentConfig) -> Result<String, CoreError> {
        let label = reply::text(&args["label"]);
        let mut ids: Vec<String> = Vec::new();
        for id in reply::ids(&args["message_ids"]) {
            if run.shown.iter().any(|m| m.message_id == id) && !ids.contains(&id) {
                ids.push(id);
            }
        }
        if label.is_empty() || ids.is_empty() {
            run.so_far.push("propose_label: name a label and messages you were shown.".into());
            return Ok("Could not propose a label".into());
        }
        let action_words = match ids.len() {
            1 => format!("Label this message as \u{201c}{label}\u{201d}"),
            n => format!("Label these {n} messages as \u{201c}{label}\u{201d}"),
        };
        let proposal = AgentProposal {
            id: self.proposal_id(conversation_id, run),
            agent_id: agent.id.clone(),
            action_words,
            message_ids: ids.clone(),
            label: Some(label.clone()),
            decision: ProposalDecision::Pending,
            draft_id: None,
        };
        self.store(proposal, Command::Label { message_ids: ids, label_name: label, create_if_missing: true }, run)
    }

    fn propose_send(&self, draft_id: &str, run: &mut Run, conversation_id: &str, agent: &AgentConfig) -> Result<String, CoreError> {
        if draft_id.is_empty() {
            run.so_far.push("propose_send: name the draft to send.".into());
            return Ok("Could not propose a send".into());
        }
        let proposal = AgentProposal {
            id: self.proposal_id(conversation_id, run),
            agent_id: agent.id.clone(),
            action_words: "Send this draft".into(),
            message_ids: Vec::new(),
            label: None,
            decision: ProposalDecision::Pending,
            draft_id: Some(draft_id.to_string()),
        };
        self.store(proposal, Command::Send { draft_id: draft_id.to_string() }, run)
    }

    /// The answer the owner reads. Citations are rebuilt from mail the agent was shown; an id it was not shown is dropped.
    /// An answer that cites nothing it was shown (and proposed nothing) says nothing was found.
    fn finish(&self, run: Run, kind: AnswerKind, text: String, cite: &[String], steps: u32) -> AskTurn {
        let mut wanted: Vec<String> = cite.to_vec();
        if wanted.iter().all(|id| !run.shown.iter().any(|m| &m.message_id == id)) {
            wanted.extend(run.proposals.iter().flat_map(|p| p.message_ids.clone()));
        }
        let mut citations: Vec<CitedMessage> = Vec::new();
        for id in wanted {
            if citations.iter().any(|c| c.message_id == id) {
                continue;
            }
            if let Some(m) = run.shown.iter().find(|m| m.message_id == id) {
                citations.push(cited(m));
            }
        }
        // A draft, an answer resting on mail it was shown, or an answer that comes with a proposal (the proposal lists its messages).
        let grounded = kind == AnswerKind::Draft || !citations.is_empty() || !run.proposals.is_empty();
        let (kind, text) = if grounded && !text.trim().is_empty() { (kind, text) } else { (AnswerKind::Answer, NOTHING_FOUND.to_string()) };
        for chunk in text.split_inclusive('\n') {
            self.emit(AgentEvent::Chunk { text: chunk.to_string() });
        }
        let answer = AgentAnswer { kind, text, citations };
        self.emit(AgentEvent::Answered { answer: answer.clone() });
        AskTurn { answer, proposals: run.proposals, steps, stopped: false }
    }
}

fn cited(m: &MailExcerpt) -> CitedMessage {
    let first = m.text.split_inclusive(['.', '!', '?', '\n']).next().unwrap_or_default().trim();
    let limit = usize::try_from(SEARCH_SNIPPET_CHARS).unwrap_or(usize::MAX);
    CitedMessage {
        message_id: m.message_id.clone(),
        thread_id: m.thread_id.clone(),
        account_id: m.account_id.clone(),
        account_name: m.account_name.clone(),
        sender_name: m.sender_name.clone(),
        received_at_ms: m.received_at_ms,
        subject: m.subject.clone(),
        quote: first.chars().take(limit).collect(),
    }
}
