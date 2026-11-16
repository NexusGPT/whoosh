use crate::common::*;
use serde_json::json;

#[tokio::test]
async fn register_and_unregister_round_trip() {
    let r = start_relay(&work_dir("roundtrip"), None);
    let (s, b) = r.register(FIXTURE_TOKEN, "acct-1").await;
    assert_eq!((s, b["created"].clone()), (201, json!(true)));
    let (s, b) = r.register(FIXTURE_TOKEN, "acct-1").await;
    assert_eq!((s, b["created"].clone()), (200, json!(false)), "registering twice is idempotent");
    let (_, l) = r.get("/v1/registrations/acct-1").await;
    assert_eq!(l["count"], 1);
    assert_eq!(l["devices"][0]["environment"], "sandbox");
    assert!(!l.to_string().contains(FIXTURE_TOKEN), "lookup never hands a device token back");
    let del = r.http.delete(format!("{}/v1/registrations/acct-1/{FIXTURE_TOKEN}", r.base)).send().await.unwrap();
    assert_eq!(del.status().as_u16(), 204);
    let (_, l) = r.get("/v1/registrations/acct-1").await;
    assert_eq!(l["count"], 0);
    let again = r.http.delete(format!("{}/v1/registrations/acct-1/{FIXTURE_TOKEN}", r.base)).send().await.unwrap();
    assert_eq!(again.status().as_u16(), 404);
    let (_, h) = r.get("/v1/health").await;
    assert_eq!(h["registrations"], 0);
}

#[tokio::test]
async fn unregister_removes_only_its_own_pair() {
    let r = start_relay(&work_dir("scope-unreg"), None);
    r.register(FIXTURE_TOKEN, "acct-1").await;
    r.register(GONE_TOKEN, "acct-1").await;
    r.register(FIXTURE_TOKEN, "acct-2").await;
    let del = r.http.delete(format!("{}/v1/registrations/acct-1/{FIXTURE_TOKEN}", r.base)).send().await.unwrap();
    assert_eq!(del.status().as_u16(), 204);
    assert_eq!(r.get("/v1/registrations/acct-1").await.1["count"], 1, "the other device on acct-1 stays");
    assert_eq!(r.get("/v1/registrations/acct-2").await.1["count"], 1, "the same device on acct-2 stays");
}

#[tokio::test]
async fn routing_id_lookup_is_scoped_to_its_id() {
    let r = start_relay(&work_dir("lookup"), None);
    r.register(FIXTURE_TOKEN, "acct-1").await;
    r.register(GONE_TOKEN, "acct-2").await;
    r.register("abcdef", "acct-2").await;
    assert_eq!(r.get("/v1/registrations/acct-1").await.1["count"], 1);
    assert_eq!(r.get("/v1/registrations/acct-2").await.1["count"], 2);
    assert_eq!(r.get("/v1/registrations/acct-unknown").await.1["count"], 0);
}

#[tokio::test]
async fn bad_input_uses_one_error_vocabulary() {
    let r = start_relay(&work_dir("errors"), None);
    let (s, b) = r.post("/v1/registrations", json!({ "device_token": "not hex!", "routing_id": "a", "environment": "sandbox" })).await;
    assert_eq!(s, 400);
    assert_eq!(b["error"]["code"], "bad_request");
    let (s, b) = r.post("/v1/registrations", json!({ "device_token": "ab", "routing_id": "a", "environment": "moon" })).await;
    assert_eq!((s, b["error"]["code"].clone()), (400, json!("bad_request")));
    let raw = r.http.post(format!("{}/v1/registrations", r.base)).body("{nope").header("content-type", "application/json").send().await.unwrap();
    assert_eq!(raw.status().as_u16(), 400);
    let b: serde_json::Value = raw.json().await.unwrap();
    assert_eq!(b["error"]["code"], "bad_request", "a malformed body answers in the same vocabulary");
}

#[tokio::test]
async fn root_path_answers_200_with_the_health_body() {
    let r = start_relay(&work_dir("root-probe"), None);
    let (s, root) = r.get("/").await;
    assert_eq!(s, 200, "hosting platforms probe GET /");
    let (_, health) = r.get("/v1/health").await;
    assert_eq!(root, health, "GET / and GET /v1/health return the same body");
    assert_eq!(root["version"], env!("CARGO_PKG_VERSION"));
}

#[tokio::test]
async fn health_reports_version_and_queue_depth() {
    let r = start_relay(&work_dir("health"), None);
    let (s, h) = r.get("/v1/health").await;
    assert_eq!(s, 200);
    assert_eq!(h["version"], env!("CARGO_PKG_VERSION"));
    assert_eq!(h["queue_depth"], 0);
    assert!(h["routes"].as_array().unwrap().iter().any(|x| x == "GET /v1/health"), "the route list describes the service");
    r.register(FIXTURE_TOKEN, "acct-1").await;
    r.register(GONE_TOKEN, "acct-1").await;
    // No APNs configured: jobs wait, the relay stays up and says so.
    assert_eq!(r.signal("sig-1", "acct-1").await.1["queued"], 2);
    let (_, h) = r.get("/v1/health").await;
    assert_eq!(h["queue_depth"], 2);
    assert_eq!(h["status"], "degraded");
    assert_eq!(h["apns"]["state"], "unconfigured");
    assert_eq!(r.signal("sig-1", "acct-1").await.1["queued"], 0, "a repeated signal id queues nothing");
    assert_eq!(r.signal("sig-2", "acct-1").await.1["queued"], 2);
    assert_eq!(r.get("/v1/health").await.1["queue_depth"], 4);
}

#[tokio::test]
async fn inbound_route_refuses_unknown_family_and_wrong_token() {
    let r = start_relay(&work_dir("inbound"), None);
    let (s, b) = r.post("/v1/inbound/nosuchfamily", json!({})).await;
    assert_eq!((s, b["error"]["code"].clone()), (404, json!("not_found")));
    let bad = r.http.post(format!("{}/v1/inbound/loopback", r.base)).bearer_auth("wrong").json(&json!({ "signal_id": "s", "routing_id": "a" })).send().await.unwrap();
    assert_eq!(bad.status().as_u16(), 401);
    let none = r.http.post(format!("{}/v1/inbound/loopback", r.base)).json(&json!({ "signal_id": "s", "routing_id": "a" })).send().await.unwrap();
    assert_eq!(none.status().as_u16(), 401);
    let (s, b) = r.signal("", "a").await;
    assert_eq!((s, b["error"]["code"].clone()), (400, json!("bad_request")));
}
