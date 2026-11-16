//! gap-engine-state: the state paths the fixture core serves and the real core now serves too (engine/src/state_extra.rs): `mcp_grant/<app>`,
//! `split_tab/<tab>/name`. The third test checks that a path of an unknown smart folder answers nothing (state_extra.rs has no arm for it: the
//! saved folders are served by smart_folder_cmds.rs). A real core after a first sync, asked through `WhooshCore::state`.
//! Each case prints one `PASS <case>` line (run with `-- --nocapture`).
//!
//! Broken copies (single edits in state_extra.rs; each is red in the named test):
//! - X1 `["mcp_grant", app] ...  => Some("false")` becomes `Some("true")`: `mcp_grant_says_no_app_holds_a_grant_while_no_mcp_server_runs`
//! - X2 the `split_tab` arm answers `t.id` instead of `t.name`: `a_split_tab_name_is_the_name_inbox_tabs_lists`
//! - X3 state_extra.rs gets back an arm before `_ => None`: `["smart_folder", folder, "count"] if !folder.is_empty() => Some("0".to_string()),`:
//!   `a_smart_folder_that_is_not_saved_has_no_count_row`
mod first_run_support;

use contracts::{Command, WhooshCore};
use first_run_support::*;
use serde_json::Value;

#[test]
fn mcp_grant_says_no_app_holds_a_grant_while_no_mcp_server_runs() {
    let rig = Rig::first_sync(1);
    assert_eq!(rig.state("mcp_grant/claude-desktop").as_deref(), Some("false"), "mcp_grant/claude-desktop");
    assert_eq!(rig.state("mcp_grant/another-app").as_deref(), Some("false"), "mcp_grant/another-app");
    assert_eq!(rig.state("mcp_server/on").as_deref(), Some("false"), "the server is off, as every grant says");
    assert_eq!(rig.state("mcp_grant/"), None, "an empty app name answers nothing");
    println!("PASS mcp_grant-false-for-every-app");
}

#[test]
fn a_split_tab_name_is_the_name_inbox_tabs_lists() {
    let rig = Rig::first_sync(3);
    let tabs: Value = serde_json::from_str(&rig.state("inbox/tabs").expect("inbox/tabs answers")).unwrap();
    let tabs = tabs.as_array().expect("inbox/tabs is a list");
    assert!(!tabs.is_empty(), "the strip lists no tab");
    for t in tabs {
        let id = t["id"].as_str().unwrap();
        let got = rig.state(&format!("split_tab/{id}/name")).unwrap_or_else(|| panic!("split_tab/{id}/name answers nothing"));
        let name: Value = serde_json::from_str(&got).unwrap_or_else(|e| panic!("split_tab/{id}/name is not JSON: {got}: {e}"));
        assert_eq!(name, t["name"], "split_tab/{id}/name is the name inbox/tabs lists");
        assert!(name.as_str().is_some_and(|n| !n.is_empty()), "split_tab/{id}/name is a non-empty string: {got}");
    }
    assert_eq!(rig.state("split_tab/no-such-tab/name"), None, "an id outside the strip answers nothing, as the fixture does");
    assert!(rig.state("split_tab/primary/name").is_some(), "the primary view has a name");
    println!("PASS split_tab-name-of-every-tab-of-the-strip ({} tabs)", tabs.len());
}

#[test]
fn a_smart_folder_that_is_not_saved_has_no_count_row() {
    let rig = Rig::first_sync(3);
    assert_eq!(rig.state("smart_folder/no-such-folder/count"), None, "an unknown folder answers nothing, as the test engine does");
    assert_eq!(rig.state("smart_folder//count"), None, "an empty folder id answers nothing");
    // a saved folder still answers its own count (smart_folder_cmds.rs), so removing the arm took nothing from a real folder
    let id = rig.core.run(Command::SaveSmartFolder { name: "Hotels".into(), rule_words: "from:@lorvan-hotels.example".into() }).expect("the folder is saved").created_id.expect("a created id");
    let count = rig.state(&format!("smart_folder/{id}/count")).expect("a saved folder answers its count");
    assert!(count.parse::<u32>().is_ok(), "the count is a number: {count}");
    assert_eq!(rig.state(&format!("smart_folder/{id}x/count")), None, "an id that is not the saved folder's answers nothing");
    println!("PASS smart_folder-unknown-folder-answers-nothing");
}
