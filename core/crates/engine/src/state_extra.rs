//! gap-engine-state: the two state paths the fixture core serves and the real core did not (docs/STATE-PATHS.json rows `mcp_grant/<app>`,
//! `split_tab/<tab>/name`), answered the way the fixture answers them for what the real core holds.
//!  - `mcp_grant/<app>`: whether the app may use the MCP server. The real app runs no MCP server and `RustCore::mcp_grants()` is empty (SetMcpGrant is not
//!    built), so no app holds a grant: `false`, as `mcp_server/on` says `false` (rust_core.rs). The fixture answers nothing for an app it does not list;
//!    the real core lists none, so it says `false` for the asked app (the reader, McpSession.swift McpGrantDoor.read, takes a bool or nil).
//!  - `split_tab/<tab>/name`: the name of a tab of the strip, a JSON string, from the same `inbox_tabs` list `inbox/tabs` answers (views and pinned
//!    labels). An id that is not a tab of the strip answers nothing, as the fixture's `tabNames` does.
//!  - `smart_folder/<folder>/count` is not here: the real core saves smart folders (smart_folder_cmds.rs `state` answers a saved folder's count), and the
//!    path of a folder it does not hold answers nothing, as the fixture's does.
use contracts::inbox_tabs;
use store::Store;

use crate::reads::json_text;

pub(crate) fn state(store: &Store, path: &str) -> Option<String> {
    let p: Vec<&str> = path.split('/').collect();
    match p.as_slice() {
        ["mcp_grant", app] if !app.is_empty() => Some("false".to_string()),
        ["split_tab", tab, "name"] => inbox_tabs(store.labels_full().ok()?).into_iter().find(|t| t.id == *tab).map(|t| json_text(&t.name)),
        _ => None,
    }
}
