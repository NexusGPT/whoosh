//! What an agent hands back: words, and the messages they rest on.
use contracts::{AnswerKind, AskAnswer, Citation};

/// One message an answer rests on, with what the citation chip shows (C-J5.4: account, sender, date) and the id the
/// reader opens (C-J5.5). Built only from mail the agent was shown in this run, never from the model's own words.
#[derive(Debug, Clone, PartialEq)]
pub struct CitedMessage {
    pub message_id: String,
    pub thread_id: String,
    pub account_id: String,
    pub account_name: String,
    pub sender_name: String,
    pub received_at_ms: i64,
    pub subject: String,
    pub quote: String,
}

impl CitedMessage {
    /// The contract's narrower citation (message id and quote), as `AgentTurn.citations` stores it.
    pub fn citation(&self) -> Citation {
        Citation {
            message_id: self.message_id.clone(),
            quote: self.quote.clone(),
            account_id: self.account_id.clone(),
            sender_name: self.sender_name.clone(),
            received_at_ms: self.received_at_ms,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct AgentAnswer {
    pub kind: AnswerKind,
    pub text: String,
    pub citations: Vec<CitedMessage>,
}

impl AgentAnswer {
    /// The contract's `AskAnswer` (delta 7): the same kind and words, with the narrower citations.
    pub fn ask_answer(&self) -> AskAnswer {
        AskAnswer { kind: self.kind, text: self.text.clone(), citations: self.citations.iter().map(CitedMessage::citation).collect() }
    }
}

/// What the owner reads when no message the agent saw supports an answer (Sources/Features/AskMac/AskWords.swift).
pub const NOTHING_FOUND: &str = "I found nothing in your mail about that.";
