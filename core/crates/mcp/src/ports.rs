//! The ports: one method each, named `execute`. The host (the FFI lane) binds them to the store, the search
//! crate, the actions crate and the approval sheet. Tests bind them to fakes.
use contracts::McpGrant;

use crate::domain::{ApprovalRequest, Decision, McpError, NewDraft, SearchHit, SearchQuery};

/// The grant for one app, or None when Whoosh has never seen the app.
pub trait GrantLookup: Send + Sync {
    fn execute(&self, app_name: &str) -> Result<Option<McpGrant>, McpError>;
}

/// Writes one grant row (insert or replace by app name).
pub trait GrantSave: Send + Sync {
    fn execute(&self, grant: &McpGrant) -> Result<(), McpError>;
}

/// Every app Whoosh has seen, in first-seen order.
pub trait GrantList: Send + Sync {
    fn execute(&self) -> Result<Vec<McpGrant>, McpError>;
}

/// The grant store as the FFI lane binds it: the three ports above on one object.
pub trait GrantStore: GrantLookup + GrantSave + GrantList {}
impl<T: GrantLookup + GrantSave + GrantList> GrantStore for T {}

/// The approval prompt in Whoosh. `execute` returns when the owner has decided; it blocks the one connection
/// that asked, never the others. No answer means no results: a host that cannot show the prompt answers Refused.
pub trait Approver: Send + Sync {
    fn execute(&self, request: &ApprovalRequest) -> Decision;
}

/// The local search (the search crate binds it). Called only after the owner approved.
pub trait MailSearch: Send + Sync {
    fn execute(&self, query: &SearchQuery) -> Result<Vec<SearchHit>, McpError>;
}

/// Saves a draft in the owner's mailbox and returns its id. The actions crate binds it. There is no send port.
pub trait DraftWriter: Send + Sync {
    fn execute(&self, draft: &NewDraft) -> Result<String, McpError>;
}
