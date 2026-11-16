//! The words of the server: what a client may ask, what the owner is asked, what leaves, and why a call fails.
use std::fmt;

/// A search the outside app asked for, already parsed and bounded.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchQuery {
    pub text: String,
    pub limit: usize,
}

/// One result that may leave this Mac. The snippet is cut to SEARCH_SNIPPET_CHARS before it is built.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchHit {
    pub message_id: String,
    pub thread_id: String,
    pub account_name: String,
    pub sender_name: String,
    pub from_address: String,
    pub subject: String,
    pub snippet: String,
    pub received_at_ms: i64,
}

/// A draft the outside app asked Whoosh to write. It is saved as a draft; nothing here can send it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewDraft {
    /// None: the core picks the owner's default account.
    pub from_account_id: Option<String>,
    pub to: Vec<String>,
    pub cc: Vec<String>,
    pub subject: String,
    pub body: String,
    pub reply_to_thread_id: Option<String>,
}

/// What the owner is asked to approve, in the words the prompt shows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ApprovalAction {
    Search { query: String },
    Draft { to: Vec<String>, subject: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApprovalRequest {
    /// The app's name, as the grant list shows it ("Claude Desktop").
    pub client: String,
    pub action: ApprovalAction,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Decision {
    Approved,
    Refused,
}

/// Why a call did not produce its result. The server turns each into one JSON-RPC error with words.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum McpError {
    ServerOff,
    NotAuthorised { client: String },
    Refused { client: String },
    InvalidParams { field: String, reason_words: String },
    UnknownTool { name: String },
    /// A port failed (store, search, drafts): the reason in words.
    Backend { reason_words: String },
}

impl fmt::Display for McpError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            McpError::ServerOff => write!(f, "Whoosh's local server is off. The owner can start it in Outside AI apps."),
            McpError::NotAuthorised { client } => write!(f, "{client} is not allowed to use this mailbox. The owner can allow it in Whoosh, under Outside AI apps."),
            McpError::Refused { client } => write!(f, "The owner refused this request from {client}. Nothing was shared."),
            McpError::InvalidParams { field, reason_words } => write!(f, "{field}: {reason_words}"),
            McpError::UnknownTool { name } => write!(f, "no tool named {name}"),
            McpError::Backend { reason_words } => write!(f, "{reason_words}"),
        }
    }
}

impl std::error::Error for McpError {}
