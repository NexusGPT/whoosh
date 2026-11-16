//! The tool list the server offers. Two tools: search and draft. There is no send tool, by design.
use serde_json::{json, Value};

pub const SEARCH_MAIL: &str = "search_mail";
pub const CREATE_DRAFT: &str = "create_draft";

pub fn list() -> Value {
    json!({ "tools": [
        {
            "name": SEARCH_MAIL,
            "description": "Search the owner's mail on this Mac. The owner is asked to approve each search in Whoosh before any result is returned.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "query": { "type": "string", "description": "What to look for, in the owner's words." },
                    "limit": { "type": "integer", "minimum": 1, "description": "Most results wanted. Whoosh never returns more than its own default." }
                },
                "required": ["query"]
            }
        },
        {
            "name": CREATE_DRAFT,
            "description": "Save a draft in Whoosh for the owner to review. The draft is never sent by this tool; the owner approves the request in Whoosh and sends it themselves.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "to": { "type": "array", "items": { "type": "string" }, "description": "Recipient addresses, at least one." },
                    "cc": { "type": "array", "items": { "type": "string" } },
                    "subject": { "type": "string", "description": "One line." },
                    "body": { "type": "string" },
                    "account_id": { "type": "string", "description": "The account to write from; the owner's default when left out." },
                    "reply_to_thread_id": { "type": "string", "description": "A thread id from search_mail, to write a reply." }
                },
                "required": ["to", "subject", "body"]
            }
        }
    ] })
}
