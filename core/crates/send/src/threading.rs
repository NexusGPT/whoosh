//! Threading headers (RFC 5322 section 3.6.4): what a reply or a forward says about the message it answers.
//!
//! A reply carries `In-Reply-To` (the parent's Message-ID) and `References` (the parent's References, then the
//! parent's Message-ID, oldest first). A forward carries `References` only: it continues the conversation's
//! chain for the owner's mail client but does not claim to answer the parent.
use contracts::CoreError;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThreadKind {
    Reply,
    Forward,
}

/// The parent message, resolved to its headers. `message_id` is the parent's own Message-ID with its angle
/// brackets; `references` is the parent's chain, oldest first (its References header, or, when it has none,
/// its In-Reply-To).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReplyContext {
    pub kind: ThreadKind,
    pub parent_message_id: String,
    pub parent_references: Vec<String>,
}

impl ReplyContext {
    /// The `References` value as a list: the parent's chain, then the parent, each id once, oldest first.
    pub fn references(&self) -> Vec<String> {
        let mut out: Vec<String> = Vec::new();
        for id in self.parent_references.iter().chain(std::iter::once(&self.parent_message_id)) {
            if !out.contains(id) {
                out.push(id.clone());
            }
        }
        out
    }

    /// The `In-Reply-To` value: the parent's Message-ID, for a reply only.
    pub fn in_reply_to(&self) -> Option<&str> {
        (self.kind == ThreadKind::Reply).then_some(self.parent_message_id.as_str())
    }
}

/// Where the parent message lives on its server. `remote_id` is the IMAP UID, written as its digits.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParentRef {
    pub account_id: String,
    pub folder: String,
    pub remote_id: String,
}

/// The parent's headers as its server holds them (the store keeps no Message-ID or References).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParentHeaders {
    pub message_id: String,
    pub references: Vec<String>,
}

/// Read the headers of one message from its mail server. Implemented by the IMAP adapter (core/crates/sync-imap).
pub trait ParentHeadersPort: Send + Sync {
    fn execute(&self, parent: &ParentRef) -> Result<ParentHeaders, CoreError>;
}

/// The message ids in a header value, each with its angle brackets, in the order written.
/// `"<a@x> <b@x>"` gives two ids; text outside angle brackets is dropped.
pub fn parse_message_ids(value: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut rest = value;
    while let Some(start) = rest.find('<') {
        let Some(len) = rest[start..].find('>') else { break };
        let id = &rest[start..start + len + 1];
        if id.len() > 2 && !id.contains(char::is_whitespace) {
            out.push(id.to_string());
        }
        rest = &rest[start + len + 1..];
    }
    out
}
