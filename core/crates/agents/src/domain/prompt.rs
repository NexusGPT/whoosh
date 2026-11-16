//! What the model is told. The owner's instructions come first, editable per agent; when they are empty the default below
//! stands in (Zero's stored-prompt-with-default pattern, research/ai-providers.md:47). Only offered tools are described.
use contracts::AgentConfig;

use super::permission::{allowed_words, Tool};

/// Stands in for empty instructions on an agent the owner made.
pub const DEFAULT_INSTRUCTIONS: &str =
    "Answer from the owner's mail only. Be brief and plain. When the mail does not answer the question, say so.";

pub fn instructions(agent: &AgentConfig) -> &str {
    let own = agent.instructions.trim();
    if own.is_empty() {
        DEFAULT_INSTRUCTIONS
    } else {
        own
    }
}

pub fn system(agent: &AgentConfig, tools: &[Tool]) -> String {
    let mut lines = vec![
        instructions(agent).to_string(),
        String::new(),
        format!("You are \"{}\": {}.", agent.name, agent.purpose.trim_end_matches('.')),
        format!("You may do only these things: {}.", allowed_words(&agent.allowed).join(", ")),
        "Reply with ONE JSON object and nothing else.".to_string(),
    ];
    if !tools.is_empty() {
        lines.push("Tools you may call, one per reply:".to_string());
        lines.extend(tools.iter().map(|t| format!("- {}", t.usage())));
    }
    lines.push(r#"To answer: {"answer":"<your words>","cite":["<id>"]}. Cite every message your answer rests on, by the id you were shown. Cite only ids you were shown."#.to_string());
    lines.push("Nothing you propose changes the owner's mail until the owner approves it.".to_string());
    lines.join("\n")
}

/// One step's prompt: the question, then what happened so far.
pub fn turn(question: &str, so_far: &[String]) -> String {
    let mut out = format!("Question: {question}");
    if !so_far.is_empty() {
        out.push_str("\n\nSo far:\n");
        out.push_str(&so_far.join("\n"));
    }
    out.push_str("\n\nReply with one JSON object.");
    out
}
