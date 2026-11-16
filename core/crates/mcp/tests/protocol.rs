//! The wire: JSON-RPC 2.0 lines over a stream, the MCP handshake, and the server's on and off.
mod support;

use std::io::Cursor;

use mcp::{codes, Decision};
use serde_json::{json, Value};
use support::*;

#[test]
fn initialize_names_the_server_and_echoes_a_version_it_speaks() {
    let rig = Rig::new(Decision::Approved);
    let mut c = rig.server.connect(None);
    let reply = send(&mut c, 7, "initialize", json!({ "protocolVersion": "2024-11-05", "clientInfo": { "name": "Cursor" } }));
    assert_eq!(reply["id"], 7);
    assert_eq!(reply["jsonrpc"], "2.0");
    assert_eq!(reply["result"]["protocolVersion"], "2024-11-05");
    assert_eq!(reply["result"]["serverInfo"]["name"], "whoosh");
    assert!(reply["result"]["capabilities"]["tools"].is_object());
}

#[test]
fn an_unknown_protocol_version_gets_the_newest_one_the_server_speaks() {
    let rig = Rig::new(Decision::Approved);
    let mut c = rig.server.connect(None);
    let reply = send(&mut c, 1, "initialize", json!({ "protocolVersion": "1999-01-01", "clientInfo": { "name": "Cursor" } }));
    assert_eq!(reply["result"]["protocolVersion"], "2025-06-18");
}

#[test]
fn initialize_without_a_name_is_refused_and_a_second_initialize_cannot_change_the_name() {
    let rig = Rig::new(Decision::Approved);
    let mut c = rig.server.connect(None);
    assert_eq!(error_code(&send(&mut c, 1, "initialize", json!({}))), codes::INVALID_PARAMS);
    assert_eq!(error_code(&send(&mut c, 2, "initialize", json!({ "clientInfo": { "name": "   " } }))), codes::INVALID_PARAMS);
    assert!(send(&mut c, 3, "initialize", json!({ "clientInfo": { "name": "Cursor" } })).get("result").is_some());
    assert_eq!(error_code(&send(&mut c, 4, "initialize", json!({ "clientInfo": { "name": "Claude Desktop" } }))), codes::INVALID_REQUEST);
    assert_eq!(c.client(), Some("Cursor"));
}

#[test]
fn a_notification_gets_no_reply_and_ping_gets_an_empty_result() {
    let rig = Rig::new(Decision::Approved);
    let mut c = rig.connect("Cursor");
    assert_eq!(c.handle_line(r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#), None);
    assert_eq!(c.handle_line(r#"{"jsonrpc":"2.0","method":"nobody/knows"}"#), None);
    assert_eq!(send(&mut c, 5, "ping", json!({}))["result"], json!({}));
}

#[test]
fn bad_messages_get_the_standard_errors() {
    let rig = Rig::new(Decision::Approved);
    let mut c = rig.connect("Cursor");
    let parse: Value = serde_json::from_str(&c.handle_line("{not json").unwrap()).unwrap();
    assert_eq!(error_code(&parse), codes::PARSE_ERROR);
    assert_eq!(parse["id"], Value::Null);
    let batch: Value = serde_json::from_str(&c.handle_line("[{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"ping\"}]").unwrap()).unwrap();
    assert_eq!(error_code(&batch), codes::INVALID_REQUEST);
    assert_eq!(error_code(&send(&mut c, 9, "resources/list", json!({}))), codes::METHOD_NOT_FOUND);
    let no_method: Value = serde_json::from_str(&c.handle_line(r#"{"jsonrpc":"2.0","id":3}"#).unwrap()).unwrap();
    assert_eq!(error_code(&no_method), codes::INVALID_REQUEST);
    assert_eq!(no_method["id"], 3);
}

#[test]
fn a_string_id_is_answered_with_the_same_string() {
    let rig = Rig::new(Decision::Approved);
    let mut c = rig.connect("Cursor");
    let reply: Value = serde_json::from_str(&c.handle_line(r#"{"jsonrpc":"2.0","id":"abc","method":"ping"}"#).unwrap()).unwrap();
    assert_eq!(reply["id"], "abc");
}

#[test]
fn a_whole_stdio_session_reads_lines_and_writes_one_line_per_reply() {
    let rig = Rig::new(Decision::Approved);
    rig.server.allow("Claude Desktop").unwrap();
    let input = [
        json!({ "jsonrpc": "2.0", "id": 1, "method": "initialize", "params": { "protocolVersion": "2025-06-18", "clientInfo": { "name": "Claude Desktop" } } }).to_string(),
        json!({ "jsonrpc": "2.0", "method": "notifications/initialized" }).to_string(),
        String::new(),
        json!({ "jsonrpc": "2.0", "id": 2, "method": "tools/call", "params": { "name": "search_mail", "arguments": { "query": "line\nbreak" } } }).to_string(),
    ]
    .join("\n");
    let mut out = Vec::new();
    rig.server.serve(None, Cursor::new(input), &mut out).unwrap();
    let text = String::from_utf8(out).unwrap();
    let lines: Vec<&str> = text.lines().collect();
    assert_eq!(lines.len(), 2, "one reply per request, none for the notification: {text}");
    let search: Value = serde_json::from_str(lines[1]).unwrap();
    assert_eq!(search["id"], 2);
    assert_eq!(result_json(&search)["results"].as_array().unwrap().len(), 2);
}

#[test]
fn bytes_that_are_not_text_get_a_parse_error_line_not_a_crash() {
    let rig = Rig::new(Decision::Approved);
    let mut out = Vec::new();
    rig.server.serve(None, Cursor::new(vec![0xff, 0xfe, b'\n']), &mut out).unwrap();
    let reply: Value = serde_json::from_slice(&out).unwrap();
    assert_eq!(error_code(&reply), codes::PARSE_ERROR);
}

#[test]
fn a_server_that_is_off_answers_every_request_with_one_error_and_start_brings_it_back() {
    let rig = Rig::new(Decision::Approved);
    rig.server.allow("Claude Desktop").unwrap();
    let mut app = rig.connect("Claude Desktop");
    rig.server.stop();
    assert!(!rig.server.is_running());
    assert_eq!(error_code(&call(&mut app, "search_mail", json!({ "query": "x" }))), codes::SERVER_OFF);
    assert_eq!(error_code(&send(&mut app, 3, "ping", json!({}))), codes::SERVER_OFF);
    assert!(rig.events().is_empty());
    rig.server.start();
    assert!(call(&mut app, "search_mail", json!({ "query": "x" })).get("result").is_some());
}

#[test]
fn stopping_ends_a_running_stdio_session_after_its_next_message() {
    let rig = Rig::new(Decision::Approved);
    rig.server.stop();
    let input = format!("{}\n{}\n", json!({ "jsonrpc": "2.0", "id": 1, "method": "ping" }), json!({ "jsonrpc": "2.0", "id": 2, "method": "ping" }));
    let mut out = Vec::new();
    rig.server.serve(None, Cursor::new(input), &mut out).unwrap();
    let text = String::from_utf8(out).unwrap();
    assert_eq!(text.lines().count(), 1, "the session closed after the first answer: {text}");
    assert_eq!(error_code(&serde_json::from_str(text.lines().next().unwrap()).unwrap()), codes::SERVER_OFF);
}

#[test]
fn a_new_server_is_off_until_started() {
    use std::sync::Arc;
    let rig = Rig::new(Decision::Approved);
    let fresh = mcp::McpServer::new(mcp::Ports { grants: rig.grants.clone(), approver: rig.approver.clone(), search: rig.search.clone(), drafts: rig.drafts.clone(), clock: Arc::new(FixedClock(1)) });
    assert!(!fresh.is_running());
}
