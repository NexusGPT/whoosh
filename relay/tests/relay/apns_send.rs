use crate::common::*;
use serde_json::json;

#[tokio::test]
async fn sandbox_send_reaches_the_local_stub_with_200_and_a_signal_only_payload() {
    let dir = work_dir("apns-ok");
    let stub = start_stub(&dir);
    let r = start_relay(&dir, Some(&stub));
    r.register(FIXTURE_TOKEN, "acct-1").await;
    assert_eq!(r.signal("history-42", "acct-1").await.1["queued"], 1);
    let h = r.wait_job_state("sent", 1).await;
    assert_eq!(h["queue_depth"], 0);
    assert_eq!(h["status"], "ok");
    let lines = stub.lines();
    assert_eq!(lines.len(), 1, "exactly one request reached the stub");
    let l = &lines[0];
    assert_eq!(l["token"], FIXTURE_TOKEN);
    assert_eq!(l["jwt_ok"], true, "the ES256 provider token verifies against the key pair");
    assert_eq!(l["topic"], "app.whoosh.test");
    assert_eq!(l["push_type"], "alert");
    assert_eq!(l["collapse_id"].as_str().unwrap().len(), 64);
    let body = l["body"].as_object().unwrap();
    let mut keys: Vec<_> = body.keys().cloned().collect();
    keys.sort();
    assert_eq!(keys, vec!["aps", "whoosh"], "the payload is a signal and nothing else");
    assert_eq!(l["body"]["whoosh"]["routing_id"], "acct-1");
    assert_eq!(l["body"]["whoosh"]["ref"], "history-42");
}

#[tokio::test]
async fn a_repeated_signal_sends_once() {
    let dir = work_dir("apns-dedup");
    let stub = start_stub(&dir);
    let r = start_relay(&dir, Some(&stub));
    r.register(FIXTURE_TOKEN, "acct-1").await;
    r.signal("history-42", "acct-1").await;
    r.signal("history-42", "acct-1").await;
    r.wait_job_state("sent", 1).await;
    r.signal("history-43", "acct-1").await;
    r.wait_job_state("sent", 2).await;
    assert_eq!(stub.lines().len(), 2, "two distinct signals, two sends");
}

#[tokio::test]
async fn apns_gone_removes_the_registration() {
    let dir = work_dir("apns-gone");
    let stub = start_stub(&dir);
    let r = start_relay(&dir, Some(&stub));
    r.register(GONE_TOKEN, "acct-1").await;
    r.register("abcdef0123", "acct-1").await;
    r.signal("history-1", "acct-1").await;
    let h = r.wait_job_state("rejected", 2).await;
    assert_eq!(h["jobs"]["sent"], 0);
    assert_eq!(r.get("/v1/registrations/acct-1").await.1["count"], 0, "410 and BadDeviceToken both drop the registration");
}

#[tokio::test]
async fn a_signal_reaches_only_its_own_routing_id() {
    let dir = work_dir("apns-scope");
    let stub = start_stub(&dir);
    let r = start_relay(&dir, Some(&stub));
    r.register(FIXTURE_TOKEN, "acct-2").await;
    assert_eq!(r.signal("history-1", "acct-1").await.1["queued"], 0);
    assert_eq!(r.get("/v1/health").await.1["jobs"], json!({ "queued": 0, "sent": 0, "rejected": 0, "failed": 0 }));
    assert!(stub.lines().is_empty());
}

#[tokio::test]
async fn the_relay_boots_without_a_reachable_apns() {
    // Boot resilience: APNs configured but nothing listening. The relay serves and the job fails visibly.
    let dir = work_dir("apns-down");
    let stub = start_stub(&dir);
    let down = Stub { url: "http://127.0.0.1:1".into(), ..stub };
    let r = start_relay(&dir, Some(&down));
    assert_eq!(r.get("/v1/health").await.0, 200);
    r.register(FIXTURE_TOKEN, "acct-1").await;
    r.signal("history-1", "acct-1").await;
    let h = r.wait_job_state("failed", 1).await;
    assert_eq!(h["queue_depth"], 0);
}
