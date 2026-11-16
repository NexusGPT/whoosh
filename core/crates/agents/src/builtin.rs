//! The built-in agents. The Writer drafts replies in the owner's voice (C-J3.3): it reads and searches mail and drafts;
//! it cannot label and cannot send, not even with confirmation.
use contracts::{AgentConfig, AgentPermission};

pub const WRITER_ID: &str = "writer";

pub const WRITER_INSTRUCTIONS: &str = "Write the reply the owner would write: their tone, their length, their sign-off. \
Use the thread and the owner's earlier replies you are shown. Never invent a fact, a date or a promise the mail does not hold.";

/// The Writer on the provider and model the owner picked in Settings: AI providers.
pub fn writer(provider_id: &str, model: &str) -> AgentConfig {
    AgentConfig {
        id: WRITER_ID.to_string(),
        name: "Writer".to_string(),
        purpose: "Drafts replies in your voice".to_string(),
        provider_id: provider_id.to_string(),
        model: model.to_string(),
        instructions: WRITER_INSTRUCTIONS.to_string(),
        allowed: vec![AgentPermission::ReadMail, AgentPermission::Search, AgentPermission::Draft],
        built_in: true,
    }
}
