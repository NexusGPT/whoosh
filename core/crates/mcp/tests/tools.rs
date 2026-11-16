//! The two tools: search returns results only after the owner approved; the draft tool saves and never sends.
mod support;

use contracts::constants::SEARCH_SNIPPET_CHARS;
use mcp::{codes, ApprovalAction, Decision, SearchHit};
use serde_json::json;
use support::*;

fn granted(decision: Decision) -> Rig {
    let rig = Rig::new(decision);
    rig.server.allow("Claude Desktop").unwrap();
    rig
}

#[test]
fn a_granted_search_returns_results_only_after_the_owner_approved() {
    let rig = granted(Decision::Approved);
    let mut app = rig.connect("Claude Desktop");
    let reply = call(&mut app, "search_mail", json!({ "query": "contract" }));
    let found = result_json(&reply);
    assert_eq!(found["results"].as_array().unwrap().len(), 2);
    assert_eq!(found["results"][0]["subject"], "Contract renewal 1");
    assert_eq!(rig.events(), vec!["approve", "search"], "the prompt came first");
    let asked = rig.approver.asked.lock().unwrap();
    assert_eq!(asked[0].client, "Claude Desktop");
    assert_eq!(asked[0].action, ApprovalAction::Search { query: "contract".into() });
}

#[test]
fn a_refused_search_returns_nothing_and_never_touches_the_mail() {
    let rig = granted(Decision::Refused);
    let mut app = rig.connect("Claude Desktop");
    let reply = call(&mut app, "search_mail", json!({ "query": "contract" }));
    assert_eq!(error_code(&reply), codes::REFUSED);
    assert!(!reply.to_string().contains("Contract renewal"));
    assert_eq!(rig.events(), vec!["approve"], "the search never ran");
}

#[test]
fn every_search_asks_again() {
    let rig = granted(Decision::Approved);
    let mut app = rig.connect("Claude Desktop");
    call(&mut app, "search_mail", json!({ "query": "one" }));
    call(&mut app, "search_mail", json!({ "query": "two" }));
    assert_eq!(rig.approver.asked.lock().unwrap().len(), 2);
}

#[test]
fn results_are_cut_to_the_limit_and_the_snippet_length_before_they_leave() {
    let rig = granted(Decision::Approved);
    let long = SearchHit { snippet: "é".repeat(SEARCH_SNIPPET_CHARS as usize * 3), ..hit(9) };
    *rig.search.hits.lock().unwrap() = vec![long, hit(2), hit(3)];
    let mut app = rig.connect("Claude Desktop");
    let found = result_json(&call(&mut app, "search_mail", json!({ "query": "x", "limit": 2 })));
    assert_eq!(found["results"].as_array().unwrap().len(), 2);
    assert_eq!(found["results"][0]["snippet"].as_str().unwrap().chars().count(), SEARCH_SNIPPET_CHARS as usize);
}

#[test]
fn the_limit_never_passes_the_default_and_the_query_reaches_the_search_unchanged() {
    let rig = granted(Decision::Approved);
    let mut app = rig.connect("Claude Desktop");
    call(&mut app, "search_mail", json!({ "query": "  from:maya  ", "limit": 100000 }));
    let q = rig.search.queries.lock().unwrap()[0].clone();
    assert_eq!(q.limit as u64, contracts::constants::SEARCH_DEFAULT_HITS);
    assert_eq!(q.text, "  from:maya  ");
}

#[test]
fn bad_search_arguments_are_refused_before_the_owner_is_asked() {
    let rig = granted(Decision::Approved);
    let mut app = rig.connect("Claude Desktop");
    for args in [json!({}), json!({ "query": "" }), json!({ "query": "   " }), json!({ "query": 5 }), json!({ "query": "x", "limit": 0 }), json!({ "query": "x", "limit": "ten" }), json!([1])] {
        let reply = call(&mut app, "search_mail", args.clone());
        assert_eq!(error_code(&reply), codes::INVALID_PARAMS, "{args}");
    }
    assert!(rig.events().is_empty());
}

#[test]
fn a_draft_is_saved_after_approval_and_is_reported_unsent() {
    let rig = granted(Decision::Approved);
    let mut app = rig.connect("Claude Desktop");
    let reply = call(&mut app, "create_draft", json!({ "to": ["maya@example.com"], "cc": ["lee@example.com"], "subject": "Renewal", "body": "Hello Maya", "reply_to_thread_id": "thr-1" }));
    let out = result_json(&reply);
    assert_eq!(out["draft_id"], "draft-1");
    assert_eq!(out["sent"], false);
    assert_eq!(rig.events(), vec!["approve", "draft"]);
    let saved = rig.drafts.saved.lock().unwrap();
    assert_eq!(saved[0].to, vec!["maya@example.com"]);
    assert_eq!(saved[0].cc, vec!["lee@example.com"]);
    assert_eq!(saved[0].reply_to_thread_id.as_deref(), Some("thr-1"));
    assert_eq!(saved[0].from_account_id, None);
    assert_eq!(rig.approver.asked.lock().unwrap()[0].action, ApprovalAction::Draft { to: vec!["maya@example.com".into()], subject: "Renewal".into() });
}

#[test]
fn a_refused_draft_saves_nothing() {
    let rig = granted(Decision::Refused);
    let mut app = rig.connect("Claude Desktop");
    let reply = call(&mut app, "create_draft", json!({ "to": ["a@example.com"], "subject": "Hi", "body": "Hello" }));
    assert_eq!(error_code(&reply), codes::REFUSED);
    assert!(rig.drafts.saved.lock().unwrap().is_empty());
}

#[test]
fn there_is_no_tool_that_sends() {
    let rig = granted(Decision::Approved);
    let mut app = rig.connect("Claude Desktop");
    let listed = send(&mut app, 3, "tools/list", json!({}));
    let names: Vec<&str> = listed["result"]["tools"].as_array().unwrap().iter().map(|t| t["name"].as_str().unwrap()).collect();
    assert_eq!(names, vec!["search_mail", "create_draft"]);
    for name in ["send_mail", "send_draft", "send", "send_email", "send_message"] {
        let reply = call(&mut app, name, json!({ "to": ["a@example.com"], "subject": "Hi", "body": "Hello", "draft_id": "draft-1" }));
        assert_eq!(error_code(&reply), codes::INVALID_PARAMS, "{name}");
    }
    assert!(rig.events().is_empty(), "no tool, no prompt, no draft: {:?}", rig.events());
}

#[test]
fn bad_draft_arguments_are_refused_before_the_owner_is_asked() {
    let rig = granted(Decision::Approved);
    let mut app = rig.connect("Claude Desktop");
    let base = json!({ "to": ["a@example.com"], "subject": "Hi", "body": "Hello" });
    let cases = [
        json!({ "subject": "Hi", "body": "Hello" }),
        json!({ "to": [], "subject": "Hi", "body": "Hello" }),
        json!({ "to": "a@example.com", "subject": "Hi", "body": "Hello" }),
        json!({ "to": ["not-an-address"], "subject": "Hi", "body": "Hello" }),
        json!({ "to": ["a@example.com, b@example.com"], "subject": "Hi", "body": "Hello" }),
        json!({ "to": ["a@example.com\r\nBcc: x@example.com"], "subject": "Hi", "body": "Hello" }),
        json!({ "to": ["a@example.com"], "cc": ["nope"], "subject": "Hi", "body": "Hello" }),
        json!({ "to": ["a@example.com"], "subject": "Hi\r\nBcc: x@example.com", "body": "Hello" }),
        json!({ "to": ["a@example.com"], "subject": "Hi" }),
        json!({ "to": ["a@example.com"], "subject": "Hi", "body": 7 }),
    ];
    for args in cases {
        assert_eq!(error_code(&call(&mut app, "create_draft", args.clone())), codes::INVALID_PARAMS, "{args}");
    }
    assert!(rig.events().is_empty());
    assert!(call(&mut app, "create_draft", base).get("result").is_some(), "the good shape still works");
}
