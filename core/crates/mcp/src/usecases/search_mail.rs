//! `search_mail`: grant, then the owner's approval, then the local search, then the cut results.
//! Nothing is searched before the owner approved, so a refusal leaves no trace of the mail in the answer.
use std::sync::Arc;

use contracts::constants::SEARCH_SNIPPET_CHARS;
use serde_json::{json, Value};

use crate::args;
use crate::domain::{ApprovalAction, ApprovalRequest, McpError, SearchHit};
use crate::ports::MailSearch;
use crate::usecases::gate::Gate;

pub struct SearchMail {
    gate: Arc<Gate>,
    search: Arc<dyn MailSearch>,
}

impl SearchMail {
    pub fn new(gate: Arc<Gate>, search: Arc<dyn MailSearch>) -> SearchMail {
        SearchMail { gate, search }
    }

    /// The tool result as the client reads it: `{"results": [...]}`.
    pub fn execute(&self, client: Option<&str>, arguments: &Value) -> Result<Value, McpError> {
        let name = self.gate.authorise(client)?;
        let query = args::search(arguments)?;
        self.gate.approve(&name, ApprovalRequest { client: name.clone(), action: ApprovalAction::Search { query: query.text.clone() } })?;
        let hits = self.search.execute(&query)?;
        let results: Vec<Value> = hits.into_iter().take(query.limit).map(hit_json).collect();
        Ok(json!({ "results": results }))
    }
}

fn hit_json(hit: SearchHit) -> Value {
    let snippet: String = hit.snippet.chars().take(usize::try_from(SEARCH_SNIPPET_CHARS).unwrap_or(usize::MAX)).collect();
    json!({
        "message_id": hit.message_id,
        "thread_id": hit.thread_id,
        "account": hit.account_name,
        "sender": hit.sender_name,
        "from": hit.from_address,
        "subject": hit.subject,
        "snippet": snippet,
        "received_at_ms": hit.received_at_ms,
    })
}
