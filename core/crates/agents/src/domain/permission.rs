//! The fixed set of allowed actions, in the owner's words, and the tools each one binds (C-J5.3).
use contracts::AgentPermission;

/// What the owner reads for one allowed action (docs/PRD.md:40).
pub fn words(permission: &AgentPermission) -> &'static str {
    match permission {
        AgentPermission::ReadMail => "read mail",
        AgentPermission::Search => "search",
        AgentPermission::Draft => "draft",
        AgentPermission::Label => "label",
        AgentPermission::SendWithConfirmation => "send with your confirmation",
    }
}

/// An agent's allowed actions as the agent card lists them: fixed order, each once.
pub fn allowed_words(allowed: &[AgentPermission]) -> Vec<&'static str> {
    PERMISSIONS.iter().filter(|p| allowed.contains(p)).map(words).collect()
}

const PERMISSIONS: [AgentPermission; 5] =
    [AgentPermission::ReadMail, AgentPermission::Search, AgentPermission::Draft, AgentPermission::Label, AgentPermission::SendWithConfirmation];

/// A tool the model can call. Each one is bound to exactly one allowed action.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Tool {
    Search,
    ReadMail,
    Draft,
    ProposeLabel,
    ProposeSend,
}

impl Tool {
    pub const ALL: [Tool; 5] = [Tool::Search, Tool::ReadMail, Tool::Draft, Tool::ProposeLabel, Tool::ProposeSend];

    /// The name the model writes in `{"tool": "<name>"}`.
    pub fn name(self) -> &'static str {
        match self {
            Tool::Search => "search",
            Tool::ReadMail => "read_mail",
            Tool::Draft => "draft",
            Tool::ProposeLabel => "propose_label",
            Tool::ProposeSend => "propose_send",
        }
    }

    pub fn from_name(name: &str) -> Option<Tool> {
        Tool::ALL.into_iter().find(|t| t.name() == name)
    }

    /// The allowed action this tool needs.
    pub fn needs(self) -> AgentPermission {
        match self {
            Tool::Search => AgentPermission::Search,
            Tool::ReadMail => AgentPermission::ReadMail,
            Tool::Draft => AgentPermission::Draft,
            Tool::ProposeLabel => AgentPermission::Label,
            Tool::ProposeSend => AgentPermission::SendWithConfirmation,
        }
    }

    /// The call shape and what it does, as the model is told.
    pub fn usage(self) -> &'static str {
        match self {
            Tool::Search => r#"{"tool":"search","query":"<words>"} finds messages in the owner's mail and shows a short excerpt of each."#,
            Tool::ReadMail => r#"{"tool":"read_mail","message_ids":["<id>"]} shows the text of messages you were shown."#,
            Tool::Draft => r#"{"tool":"draft","body":"<the reply>","cite":["<id>"]} ends your turn with a draft the owner edits and sends; nothing is sent."#,
            Tool::ProposeLabel => {
                r#"{"tool":"propose_label","message_ids":["<id>"],"label":"<name or Parent > Child>"} proposes a label; nothing changes until the owner approves."#
            }
            Tool::ProposeSend => r#"{"tool":"propose_send","draft_id":"<id>"} proposes sending a saved draft; nothing is sent until the owner approves."#,
        }
    }
}

/// The tools an agent is offered: those its allowed actions name, and no other.
pub fn offered(allowed: &[AgentPermission]) -> Vec<Tool> {
    Tool::ALL.into_iter().filter(|t| allowed.contains(&t.needs())).collect()
}
