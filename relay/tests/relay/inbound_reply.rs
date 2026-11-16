//! Round 2: the adapter port sees the request URI and may answer the request; a renew signal is a
//! background push. Everything runs against the real binary and the local APNs stub.
use crate::common::*;
use serde_json::json;

/// A number from docs/CONSTANTS.md, read at test time so it lives once.
fn ledger_value(name: &str) -> usize {
    let text = std::fs::read_to_string(format!("{}/../docs/CONSTANTS.md", env!("CARGO_MANIFEST_DIR"))).expect("docs/CONSTANTS.md");
    let row = text.lines().find(|l| l.starts_with(&format!("| {name} |"))).unwrap_or_else(|| panic!("ledger row {name}"));
    row.split('|').nth(2).expect("value cell").trim().parse().expect("a plain number")
}

#[cfg(any(feature = "adapter-graph", feature = "adapter-jmap"))]
fn post_raw(r: &Relay, path_and_query: &str, body: Vec<u8>) -> reqwest::RequestBuilder {
    r.http.post(format!("{}{path_and_query}", r.base)).body(body)
}

#[cfg(feature = "adapter-graph")]
#[tokio::test]
async fn graph_validation_handshake_is_echoed_as_plain_text_and_queues_nothing() {
    let dir = work_dir("graph-validation");
    let r = start_relay(&dir, None);
    let res = post_raw(&r, "/v1/inbound/graph?validationToken=Validation%3a+Token%20ok", vec![]).send().await.unwrap();
    assert_eq!(res.status().as_u16(), 200);
    assert!(res.headers()["content-type"].to_str().unwrap().starts_with("text/plain"), "Graph requires text/plain");
    assert_eq!(res.headers()["x-content-type-options"], "nosniff");
    assert_eq!(res.text().await.unwrap(), "Validation: Token ok", "the token comes back decoded, byte for byte");
    assert_eq!(r.get("/v1/health").await.1["queue_depth"], 0);
    // Without the token the same route is the ordinary JSON delivery path again.
    let plain = post_raw(&r, "/v1/inbound/graph", b"{}".to_vec()).send().await.unwrap();
    assert_eq!(plain.status().as_u16(), 400, "no validationToken: the adapter decodes (and refuses a non-delivery)");
}

#[cfg(feature = "adapter-jmap")]
#[tokio::test]
async fn jmap_push_url_carries_a_per_subscription_secret() {
    let dir = work_dir("jmap-secret");
    let r = start_relay(&dir, None);
    r.register(FIXTURE_TOKEN, "acct-1").await;
    let secret = "s3cret-s3cret-s3cret";
    let bind = json!({ "bind": { "routing_id": "acct-1", "account_id": "u1234", "secret": secret } });
    assert_eq!(post_raw(&r, "/v1/inbound/jmap", bind.to_string().into_bytes()).send().await.unwrap().status().as_u16(), 202);
    let change = |state: &str| json!({ "@type": "StateChange", "changed": { "u1234": { "EmailDelivery": state } } }).to_string().into_bytes();
    let denied = post_raw(&r, "/v1/inbound/jmap", change("a1")).send().await.unwrap();
    assert_eq!(denied.status().as_u16(), 401, "no secret in the URL");
    let wrong = post_raw(&r, "/v1/inbound/jmap?s=not-the-secret-not-the-secret", change("a1")).send().await.unwrap();
    assert_eq!(wrong.status().as_u16(), 401, "wrong secret");
    assert_eq!(r.get("/v1/health").await.1["queue_depth"], 0, "a refused push queues nothing");
    let ok = post_raw(&r, &format!("/v1/inbound/jmap?s={secret}"), change("a1")).send().await.unwrap();
    assert_eq!(ok.status().as_u16(), 202);
    assert_eq!(ok.json::<serde_json::Value>().await.unwrap()["queued"], 1);
    // A malformed secret is refused at bind time.
    let weak = json!({ "bind": { "routing_id": "acct-1", "account_id": "u9", "secret": "short" } });
    assert_eq!(post_raw(&r, "/v1/inbound/jmap", weak.to_string().into_bytes()).send().await.unwrap().status().as_u16(), 400);
}

#[cfg(feature = "adapter-jmap")]
#[tokio::test]
async fn a_jmap_bind_without_a_secret_keeps_the_old_behaviour() {
    let dir = work_dir("jmap-nosecret");
    let r = start_relay(&dir, None);
    r.register(FIXTURE_TOKEN, "acct-1").await;
    let bind = json!({ "bind": { "routing_id": "acct-1", "account_id": "u77" } });
    assert_eq!(post_raw(&r, "/v1/inbound/jmap", bind.to_string().into_bytes()).send().await.unwrap().status().as_u16(), 202);
    let change = json!({ "@type": "StateChange", "changed": { "u77": { "EmailDelivery": "b1" } } }).to_string().into_bytes();
    let ok = post_raw(&r, "/v1/inbound/jmap", change).send().await.unwrap();
    assert_eq!(ok.json::<serde_json::Value>().await.unwrap()["queued"], 1);
}

async fn sent_lines(dir_name: &str, refs: &[&str]) -> Vec<serde_json::Value> {
    let dir = work_dir(dir_name);
    let stub = start_stub(&dir);
    let r = start_relay(&dir, Some(&stub));
    r.register(FIXTURE_TOKEN, "acct-1").await;
    for (i, rf) in refs.iter().enumerate() {
        let res = r
            .http
            .post(format!("{}/v1/inbound/loopback", r.base))
            .bearer_auth(SIGNAL_TOKEN)
            .json(&json!({ "signal_id": format!("sig-{i}"), "routing_id": "acct-1", "opaque_ref": rf }))
            .send()
            .await
            .unwrap();
        assert_eq!(res.status().as_u16(), 202);
        r.wait_job_state("sent", i as i64 + 1).await;
    }
    stub.lines()
}

#[tokio::test]
async fn a_renew_signal_is_a_background_push_and_mail_is_still_an_alert() {
    let lines = sent_lines("renew-background", &["watch-renew:graph", "history-42"]).await;
    assert_eq!(lines.len(), 2);
    let (renew, mail) = (&lines[0], &lines[1]);
    assert_eq!(renew["push_type"], "background");
    assert_eq!(renew["priority"], "5", "Apple refuses a background push at priority 10");
    assert_eq!(renew["body"]["aps"], json!({ "content-available": 1 }), "no alert, no sound, no badge");
    assert_eq!(renew["body"]["whoosh"]["ref"], "watch-renew:graph", "the device reads the ref to know what to renew");
    assert_eq!(mail["push_type"], "alert");
    assert_eq!(mail["priority"], "10");
    assert_eq!(mail["body"]["aps"]["alert"]["title"], "New mail");
}

#[tokio::test]
async fn a_ref_over_the_cap_is_left_out_and_the_push_still_goes() {
    let cap = ledger_value("RELAY_APNS_REF_MAX_B");
    let at_cap = "a".repeat(cap);
    let over = "b".repeat(cap + 1);
    let lines = sent_lines("ref-cap", &[&at_cap, &over]).await;
    assert_eq!(lines[0]["body"]["whoosh"]["ref"], at_cap.as_str(), "a ref at the cap travels whole");
    assert!(lines[1]["body"]["whoosh"]["ref"].is_null(), "a ref over the cap is dropped, not truncated");
    assert_eq!(lines[1]["push_type"], "alert");
}
