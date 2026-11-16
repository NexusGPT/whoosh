//! The grant rules: an ungranted app gets nothing; a grant is per app and revocable; the gate fails closed.
mod support;

use std::sync::Arc;

use contracts::McpGrant;
use mcp::{codes, Decision, McpServer, Ports};
use serde_json::json;
use support::*;

#[test]
fn an_ungranted_client_receives_an_authorisation_error_and_no_mail() {
    let rig = Rig::new(Decision::Approved);
    let mut app = rig.connect("Claude Desktop");
    let reply = call(&mut app, "search_mail", json!({ "query": "contract" }));
    assert_eq!(error_code(&reply), codes::NOT_AUTHORISED);
    assert!(reply["error"]["message"].as_str().unwrap().contains("Claude Desktop"));
    assert!(!reply.to_string().contains("Contract renewal"), "no mail in the answer");
    assert!(rig.events().is_empty(), "the owner is not asked and nothing is searched: {:?}", rig.events());
}

#[test]
fn an_ungranted_client_cannot_draft_either() {
    let rig = Rig::new(Decision::Approved);
    let mut app = rig.connect("Claude Desktop");
    let reply = call(&mut app, "create_draft", json!({ "to": ["a@example.com"], "subject": "Hi", "body": "Hello" }));
    assert_eq!(error_code(&reply), codes::NOT_AUTHORISED);
    assert!(rig.drafts.saved.lock().unwrap().is_empty());
}

#[test]
fn a_grant_is_per_app() {
    let rig = Rig::new(Decision::Approved);
    rig.server.allow("Claude Desktop").unwrap();
    let mut cursor = rig.connect("Cursor");
    let mut claude = rig.connect("Claude Desktop");
    assert_eq!(error_code(&call(&mut cursor, "search_mail", json!({ "query": "x" }))), codes::NOT_AUTHORISED);
    assert!(call(&mut claude, "search_mail", json!({ "query": "x" })).get("result").is_some());
    assert_eq!(rig.approver.asked.lock().unwrap().len(), 1, "only Claude Desktop reached the prompt");
}

#[test]
fn revoking_ends_access_at_once() {
    let rig = Rig::new(Decision::Approved);
    rig.server.allow("Claude Desktop").unwrap();
    let mut app = rig.connect("Claude Desktop");
    assert!(call(&mut app, "search_mail", json!({ "query": "x" })).get("result").is_some());
    rig.server.revoke("Claude Desktop").unwrap();
    assert_eq!(error_code(&call(&mut app, "search_mail", json!({ "query": "x" }))), codes::NOT_AUTHORISED);
    assert_eq!(rig.search.queries.lock().unwrap().len(), 1, "the second search never ran");
}

#[test]
fn a_revoke_made_while_the_prompt_is_open_still_wins() {
    let rig = Rig::new(Decision::Approved);
    rig.server.allow("Claude Desktop").unwrap();
    let server = rig.server.clone();
    *rig.approver.before_answer.lock().unwrap() = Some(Box::new(move || {
        server.revoke("Claude Desktop").unwrap();
    }));
    let mut app = rig.connect("Claude Desktop");
    let reply = call(&mut app, "search_mail", json!({ "query": "x" }));
    assert_eq!(error_code(&reply), codes::NOT_AUTHORISED);
    assert!(rig.search.queries.lock().unwrap().is_empty(), "approved, then revoked: nothing is searched");
}

#[test]
fn the_gate_stays_closed_when_the_grant_store_fails() {
    let log: Log = Arc::default();
    let grants = Arc::new(FlakyGrants::default());
    let approver = Arc::new(ScriptedApprover { decision: std::sync::Mutex::new(Decision::Approved), asked: Default::default(), log: log.clone(), before_answer: Default::default() });
    let search = Arc::new(FakeSearch { hits: std::sync::Mutex::new(vec![hit(1)]), queries: Default::default(), log: log.clone() });
    let drafts = Arc::new(FakeDrafts { saved: Default::default(), log: log.clone() });
    let server = McpServer::new(Ports { grants: grants.clone(), approver, search, drafts, clock: Arc::new(FixedClock(1)) });
    server.start();
    server.allow("Claude Desktop").unwrap();
    let mut c = server.connect(None);
    assert!(send(&mut c, 1, "initialize", json!({ "clientInfo": { "name": "Claude Desktop" } })).get("result").is_some());
    assert!(call(&mut c, "search_mail", json!({ "query": "x" })).get("result").is_some(), "healthy store: the grant opens the gate");
    log.lock().unwrap().clear();
    grants.break_it();
    let reply = call(&mut c, "search_mail", json!({ "query": "x" }));
    assert_eq!(error_code(&reply), codes::NOT_AUTHORISED);
    assert!(log.lock().unwrap().is_empty(), "no prompt, no search: {:?}", log.lock().unwrap());
}

#[test]
fn a_connection_that_never_named_itself_gets_nothing() {
    let rig = Rig::new(Decision::Approved);
    rig.server.allow("Claude Desktop").unwrap();
    let mut c = rig.server.connect(None);
    assert_eq!(error_code(&call(&mut c, "search_mail", json!({ "query": "x" }))), codes::NOT_AUTHORISED);
}

#[test]
fn a_peer_the_host_proved_outranks_the_name_the_app_declares() {
    let rig = Rig::new(Decision::Approved);
    rig.server.allow("Claude Desktop").unwrap();
    let mut c = rig.server.connect(Some("Cursor".into()));
    let hello = send(&mut c, 1, "initialize", json!({ "clientInfo": { "name": "Claude Desktop" } }));
    assert!(hello.get("result").is_some());
    assert_eq!(c.client(), Some("Cursor"));
    assert_eq!(error_code(&call(&mut c, "search_mail", json!({ "query": "x" }))), codes::NOT_AUTHORISED);
}

#[test]
fn an_app_seen_at_initialize_is_listed_ungranted_and_allow_stamps_the_time() {
    let rig = Rig::new(Decision::Approved);
    let _ = rig.connect("Claude Desktop");
    let _ = rig.connect("Cursor");
    let _ = rig.connect("Claude Desktop");
    let listed = rig.server.grants().unwrap();
    assert_eq!(listed, vec![
        McpGrant { app_name: "Claude Desktop".into(), allowed: false, granted_at_ms: None },
        McpGrant { app_name: "Cursor".into(), allowed: false, granted_at_ms: None },
    ]);
    rig.server.allow("Cursor").unwrap();
    assert_eq!(rig.server.grants().unwrap()[1], McpGrant { app_name: "Cursor".into(), allowed: true, granted_at_ms: Some(42) });
}

#[test]
fn saying_hello_again_never_changes_a_grant() {
    let rig = Rig::new(Decision::Approved);
    rig.server.allow("Claude Desktop").unwrap();
    let _ = rig.connect("Claude Desktop");
    assert!(rig.server.grants().unwrap()[0].allowed);
}
