mod support;
use contracts::SyncCursor;
use serde_json::json;
use support::*;
use sync_gmail_api::limits::*;
use sync_gmail_api::{Change, GmailError, ResyncReason, SyncMode};

fn cursor(id: &str) -> SyncCursor {
    SyncCursor::GmailHistory { history_id: id.to_string() }
}

#[test]
fn history_from_a_start_id_transfers_only_the_changes() {
    let clock = Clock::new(T0);
    let replay = Replay::from_files(&clock, &["history_incremental.json"]);
    let api = open(&replay, &clock);
    let mut sink = VecSink::default();
    let summary = api.sync(Some(&cursor("1000")), &mut sink).expect("sync");

    // Only the history call and the one surviving added message were requested: no listing, no profile, no other body.
    assert_eq!(replay.paths(), vec!["history", "messages/m-new-1"]);
    assert_eq!(summary.mode, SyncMode::Incremental);
    assert_eq!(summary.reason, None);
    assert_eq!(summary.cursor, cursor("1006"));
    assert_eq!(summary.messages, 1);

    // Bodies: one message, decoded from base64url.
    let bodies: Vec<_> = sink.batches.iter().flat_map(|b| b.messages.iter()).collect();
    assert_eq!(bodies.len(), 1);
    assert_eq!(bodies[0].id, "m-new-1");
    assert_eq!(bodies[0].thread_id, "t-1");
    assert_eq!(bodies[0].internal_date_ms, 1_793_696_400_000);
    let text = String::from_utf8(bodies[0].rfc822.clone()).unwrap();
    assert!(text.contains("From: Alice Example <alice@example.test>") && text.contains("Subject: Lunch on Thursday?"), "{text}");

    // Changes: the added-then-deleted message never appears as added; the delete stays (harmless for the store).
    let all: Vec<&Change> = sink.batches.iter().flat_map(|b| b.changes.iter()).collect();
    assert_eq!(all.len(), 5, "{all:?}");
    assert!(all.contains(&&Change::Added { message_id: "m-new-1".into(), thread_id: "t-1".into(), label_ids: vec!["INBOX".into(), "UNREAD".into()] }));
    assert!(all.contains(&&Change::LabelsChanged { message_id: "m-old-7".into(), added: vec!["STARRED".into()], removed: vec![] }));
    assert!(all.contains(&&Change::LabelsChanged { message_id: "m-old-8".into(), added: vec![], removed: vec!["UNREAD".into()] }));
    assert!(all.contains(&&Change::Deleted { message_id: "m-gone-3".into() }));
    assert!(all.contains(&&Change::Deleted { message_id: "m-new-2".into() }));
    assert!(!all.iter().any(|c| matches!(c, Change::Added { message_id, .. } if message_id == "m-new-2")));
    assert_eq!(summary.changes, 5);
}

#[test]
fn the_history_request_asks_for_every_change_kind_from_the_start_id() {
    let clock = Clock::new(T0);
    let replay = Replay::from_files(&clock, &["history_no_change.json"]);
    let api = open(&replay, &clock);
    let summary = api.sync(Some(&cursor("1006")), &mut VecSink::default()).expect("sync");
    assert_eq!(summary.cursor, cursor("1006"));
    assert_eq!(summary.changes, 0);
    let log = replay.requests();
    assert_eq!(log.len(), 1);
    let q = &log[0].request.query;
    assert!(q.contains(&("startHistoryId".into(), "1006".into())));
    assert!(q.contains(&("maxResults".into(), GMAIL_PAGE_SIZE.to_string())));
    for kind in ["messageAdded", "messageDeleted", "labelAdded", "labelRemoved"] {
        assert!(q.contains(&("historyTypes".into(), kind.into())), "missing historyTypes={kind}");
    }
}

#[test]
fn history_pages_are_followed_to_the_end() {
    let clock = Clock::new(T0);
    let replay = Replay::from_files(&clock, &["history_paged.json"]);
    let api = open(&replay, &clock);
    let mut sink = VecSink::default();
    let summary = api.sync(Some(&cursor("1006")), &mut sink).expect("sync");
    assert_eq!(replay.paths(), vec!["history", "history", "messages/m-p-1", "messages/m-p-2"]);
    assert_eq!(summary.cursor, cursor("1008"));
    assert_eq!(summary.messages, 2);
}

#[test]
fn a_404_history_triggers_a_full_resync() {
    let clock = Clock::new(T0);
    let replay = Replay::from_files(&clock, &["history_404_full_resync.json"]);
    let api = open(&replay, &clock);
    let mut sink = VecSink::default();
    let summary = api.sync(Some(&cursor("50")), &mut sink).expect("sync");

    assert_eq!(summary.mode, SyncMode::FullResync);
    assert_eq!(summary.reason, Some(ResyncReason::HistoryExpired));
    // The position is read before the listing; each list page is fetched and stored before the next page is asked for.
    assert_eq!(replay.paths(), vec!["history", "profile", "messages", "messages/m-a", "messages/m-b", "messages", "messages/m-c"]);
    assert_eq!(summary.cursor, cursor("2000"));
    // m-c was deleted between the listing and the fetch: counted as gone, not an error.
    assert_eq!(summary.messages, 2);
    assert_eq!(summary.gone, 1);
    let ids: Vec<&str> = sink.batches.iter().flat_map(|b| b.messages.iter().map(|m| m.id.as_str())).collect();
    assert_eq!(ids, vec!["m-a", "m-b"]);
}

#[test]
fn no_cursor_is_a_full_sync_and_never_calls_history() {
    let clock = Clock::new(T0);
    let replay = Replay::from_files(&clock, &["history_404_full_resync.json"]);
    let api = open(&replay, &clock);
    let summary = api.sync(None, &mut VecSink::default()).expect("sync");
    assert_eq!(summary.reason, Some(ResyncReason::NoCursor));
    assert!(!replay.paths().contains(&"history".to_string()));
    assert_eq!(summary.cursor, cursor("2000"));
}

#[test]
fn a_cursor_of_another_provider_is_refused_without_a_request() {
    let clock = Clock::new(T0);
    let replay = Replay::new(&clock);
    let api = open(&replay, &clock);
    let err = api.sync(Some(&SyncCursor::GraphDelta { token: "x".into() }), &mut VecSink::default()).unwrap_err();
    assert!(matches!(err, GmailError::Invalid { .. }), "{err:?}");
    assert_eq!(replay.count(), 0);
}

#[test]
fn a_sink_that_refuses_stops_the_sync_and_no_cursor_is_returned() {
    let clock = Clock::new(T0);
    let replay = Replay::from_files(&clock, &["history_incremental.json"]);
    let api = open(&replay, &clock);
    let err = api.sync(Some(&cursor("1000")), &mut FailingSink).unwrap_err();
    assert_eq!(err, GmailError::Sink { message: "disk full".into() });
}

#[test]
fn an_expired_token_is_named_with_googles_own_words() {
    let clock = Clock::new(T0);
    let replay = Replay::new(&clock);
    replay.push(json!({"path": "history", "status": 401, "body": {"error": {"code": 401, "message": "Invalid Credentials", "errors": [{"reason": "authError"}]}}}));
    let api = open(&replay, &clock);
    let err = api.sync(Some(&cursor("1")), &mut VecSink::default()).unwrap_err();
    assert_eq!(err, GmailError::AuthExpired { message: "Invalid Credentials".into() });
    assert!(!err.to_string().contains(TOKEN));
    assert!(!err.retryable());
}

#[test]
fn a_server_error_carries_the_real_body_not_the_status_alone() {
    let clock = Clock::new(T0);
    let replay = Replay::new(&clock);
    replay.push(json!({"path": "history", "status": 503, "body": {"error": {"code": 503, "message": "Backend Error", "errors": [{"reason": "backendError"}]}}}));
    let api = open(&replay, &clock);
    let err = api.sync(Some(&cursor("1")), &mut VecSink::default()).unwrap_err();
    assert_eq!(err, GmailError::Http { status: 503, message: "Backend Error".into() });
    assert!(err.retryable());
}

#[test]
fn the_bearer_token_never_shows_in_a_debug_line() {
    let clock = Clock::new(T0);
    let replay = Replay::from_files(&clock, &["history_no_change.json"]);
    open(&replay, &clock).sync(Some(&cursor("1006")), &mut VecSink::default()).unwrap();
    let log = replay.requests();
    assert!(!format!("{:?}", log[0].request).contains(TOKEN));
}
