mod support;
use std::{
    collections::HashMap,
    sync::{atomic::AtomicUsize, Arc, Mutex},
};

use contracts::CoreError;
use support::*;
use sync_graph::{folders_to_sync, limits, parse_inbound, ChangeType, DeltaChange, LifecycleEvent};

const STATE: &str = "client-state-example";

fn folder_map() -> HashMap<String, String> {
    HashMap::from([("sub-inbox-example".to_string(), "inbox-id".to_string())])
}

#[test]
fn a_created_notification_names_the_message_and_drops_a_forged_one() {
    let inbound = parse_inbound(None, &fixture("notification_created.json"), STATE).expect("parse");
    assert_eq!(inbound.notices.len(), 1);
    assert_eq!(inbound.notices[0].message_id, "AAMkNewExample01=");
    assert_eq!(inbound.notices[0].change_type, ChangeType::Created);
    assert_eq!(inbound.rejected, 1, "the item with another clientState is dropped");
    assert!(inbound.validation.is_none());
}

#[test]
fn the_handshake_hands_back_the_validation_token_and_an_empty_body_is_fine() {
    let inbound = parse_inbound(Some("Validation: token-example"), "", STATE).expect("parse");
    assert_eq!(inbound.validation.as_deref(), Some("Validation: token-example"));
    assert!(inbound.notices.is_empty() && inbound.lifecycle.is_empty());
}

#[test]
fn lifecycle_events_are_read_and_a_missed_one_asks_for_a_delta_round() {
    let inbound = parse_inbound(None, &fixture("notification_lifecycle.json"), STATE).expect("parse");
    assert_eq!(inbound.lifecycle.len(), 2);
    assert_eq!(inbound.lifecycle[0].event, LifecycleEvent::Missed);
    assert_eq!(inbound.lifecycle[1].event, LifecycleEvent::ReauthorizationRequired);
    let folders = folders_to_sync(&inbound, &folder_map());
    assert_eq!(folders.into_iter().collect::<Vec<_>>(), vec!["inbox-id".to_string()], "the unknown subscription is ignored");
}

#[test]
fn a_shape_this_app_does_not_know_is_refused_not_guessed() {
    let bad = r#"{"value":[{"subscriptionId":"s","clientState":"client-state-example","changeType":"teleported","resourceData":{"id":"x"}}]}"#;
    assert!(matches!(parse_inbound(None, bad, STATE), Err(CoreError::Invalid { .. })));
    assert!(matches!(parse_inbound(None, "not json", STATE), Err(CoreError::Invalid { .. })));
}

#[tokio::test]
async fn subscriptions_are_created_renewed_and_deleted_within_the_lifetime_ceiling() {
    let stub = start(0, |req| match req.method.as_str() {
        "DELETE" => Resp::status(204, String::new()),
        _ => Resp::ok(fixture("subscription_created.json")),
    })
    .await;
    let r = rig(&stub.base, 50, 3);
    let now_ms = 1_793_000_000_000;
    let sub = r.mailbox.create_subscription("inbox-id", "https://relay.example.net/graph/notify", Some("https://relay.example.net/graph/lifecycle"), STATE, now_ms).await.expect("create");
    assert_eq!((sub.id.as_str(), sub.resource.as_str()), ("sub-inbox-example", "me/mailFolders('inbox-id')/messages"));
    r.mailbox.renew_subscription(&sub.id, now_ms + 1).await.expect("renew");
    r.mailbox.delete_subscription(&sub.id).await.expect("delete");

    let reqs = stub.requests();
    assert_eq!(reqs.iter().map(|q| (q.method.as_str(), q.target.as_str())).collect::<Vec<_>>(), vec![("POST", "/v1.0/subscriptions"), ("PATCH", "/v1.0/subscriptions/sub-inbox-example"), ("DELETE", "/v1.0/subscriptions/sub-inbox-example")]);
    let body: serde_json::Value = serde_json::from_str(&reqs[0].body).expect("json body");
    assert_eq!(body["changeType"], "created,updated,deleted");
    assert_eq!(body["resource"], "me/mailFolders('inbox-id')/messages");
    assert_eq!(body["notificationUrl"], "https://relay.example.net/graph/notify");
    assert_eq!(body["lifecycleNotificationUrl"], "https://relay.example.net/graph/lifecycle");
    assert_eq!(body["clientState"], STATE);
    let asked = chrono::DateTime::parse_from_rfc3339(body["expirationDateTime"].as_str().unwrap()).unwrap().timestamp_millis();
    assert_eq!(asked, now_ms + (limits::GRAPH_SUB_CEILING_MIN - 1) * 60_000, "one minute under the ceiling");
    assert!(asked - now_ms < limits::GRAPH_SUB_CEILING_MIN * 60_000);
    assert!(sub.needs_renewal(sub.expires_at_ms - 1_000, 60_000));
    assert!(!sub.needs_renewal(sub.expires_at_ms - 3_600_000, 60_000));
}

#[tokio::test]
async fn a_notification_leads_to_one_delta_round_that_fetches_the_new_message() {
    let model = Arc::new(Mutex::new(Model::new(5)));
    let served = Arc::new(AtomicUsize::new(0));
    let stub = start(0, model_handler(model.clone(), served)).await;
    let r = rig(&stub.base, 50, 3);
    let first = r.mailbox.delta_round("inbox-id", "Inbox", None).await.expect("first");
    assert_eq!(first.changes.len(), 5);

    model.lock().unwrap().add("AAMkNewExample01=");
    let inbound = parse_inbound(None, &fixture("notification_created.json"), STATE).expect("parse");
    let folders = folders_to_sync(&inbound, &folder_map());
    assert_eq!(folders.len(), 1);
    let folder = folders.iter().next().unwrap();
    let round = r.mailbox.delta_round(folder, "Inbox", Some(&first.cursor)).await.expect("round");
    assert_eq!(round.changes.len(), 1);
    let DeltaChange::Upsert(m) = &round.changes[0] else { panic!("not an upsert") };
    assert_eq!(m.id, inbound.notices[0].message_id, "the notified message is the one the round brought");
}
