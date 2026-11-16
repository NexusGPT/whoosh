//! Whoosh's local MCP server (Model Context Protocol, JSON-RPC 2.0 over stdio) for outside AI apps on this Mac.
//!
//! An outside app (Claude Desktop, Cursor) names itself at `initialize`. It gets nothing from the mail until the
//! owner allows it by name; after that every `search_mail` and every `create_draft` waits for the owner's approval
//! in Whoosh, and a refusal returns nothing. Grants are per app and revocable at any time. The server has no tool
//! that sends mail.
//!
//! The host binds five ports (`ports`): the grant store, the approval prompt, the search, the draft writer, and
//! the contract's clock. Public surface for the FFI lane: `McpServer` (`new`, `start`, `stop`, `is_running`,
//! `grants`, `allow`, `revoke`, `connect`, `serve`), `Ports`, `Connection::handle_line`, and the port traits.
//!
//! Third-party code: serde_json only (MIT OR Apache-2.0). The JSON-RPC 2.0 framing and the MCP methods
//! (initialize, ping, tools/list, tools/call) are written here from the public specification; no MCP crate is
//! vendored offline, and no code is copied from imap-mcp or mail-mcp.

mod args;
pub mod codes;
mod connection;
pub mod domain;
pub mod memory;
pub mod ports;
mod server;
pub mod tools;
pub mod usecases;

pub use connection::Connection;
pub use domain::{ApprovalAction, ApprovalRequest, Decision, McpError, NewDraft, SearchHit, SearchQuery};
pub use memory::MemoryGrants;
pub use ports::{Approver, DraftWriter, GrantList, GrantLookup, GrantSave, GrantStore, MailSearch};
pub use server::{McpServer, Ports};
