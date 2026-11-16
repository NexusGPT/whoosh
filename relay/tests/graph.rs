//! relay-graph: change notifications in, one APNs alert per registered device out; rich content
//! forwarded still encrypted; subscription renew signals before expiry under a fake clock.
#![allow(dead_code, unused_imports)]
#[path = "../src/registry/mod.rs"]
mod registry;
#[path = "../src/routes/mod.rs"]
mod routes;
#[path = "../src/apns/mod.rs"]
mod apns;
#[path = "../src/tracking/mod.rs"]
mod tracking;
#[cfg(feature = "adapter-gmail")]
#[path = "../src/adapters/gmail/mod.rs"]
mod adapter_gmail;
#[path = "../src/adapters/graph/mod.rs"]
mod adapter_graph;
#[cfg(feature = "adapter-jmap")]
#[path = "../src/adapters/jmap/mod.rs"]
mod adapter_jmap;
#[cfg(feature = "adapter-macsource")]
#[path = "../src/adapters/macsource/mod.rs"]
mod adapter_macsource;
#[path = "relay/common.rs"]
mod common;
#[path = "../src/adapters/testkit.rs"]
mod testkit;

use adapter_graph::{GraphAdapter, GraphConfig, GRAPH_SUB_MAX_MIN, GRAPH_SUB_RICH_MAX_MIN, REF_MAX_BYTES, RENEW_MARGIN_MIN};
use base64::Engine;
use common::FIXTURE_TOKEN;
use registry::adapter::ProviderAdapter;
use serde_json::{json, Value};
use std::sync::Arc;
use std::time::Duration;
use testkit::{rig, FakeClock, Rig, OTHER_TOKEN};

const T0: i64 = 1_800_000_000;
const STATE: &str = "secret-client-state";

fn fixture(name: &str) -> Value {
    let path = format!("{}/../_captures/relay-graph/fixtures/{name}", env!("CARGO_MANIFEST_DIR"));
    serde_json::from_str(&std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path}: {e}"))).unwrap()
}

struct Clk(Arc<FakeClock>);

impl adapter_graph::ledger::Clock for Clk {
    fn now(&self) -> i64 {
        self.0.now()
    }
}

struct Bed {
    rig: Rig,
    adapter: Arc<GraphAdapter>,
    clock: Arc<FakeClock>,
}

async fn bed(tick_ms: u64) -> Bed {
    let clock = FakeClock::at(T0);
    let config = GraphConfig { renew_margin_min: RENEW_MARGIN_MIN, tick: Duration::from_millis(tick_ms) };
    let ledger = Arc::new(adapter_graph::ledger::Ledger::open("graph", ":memory:"));
    let adapter = Arc::new(GraphAdapter::new(config, ledger, Arc::new(Clk(clock.clone())), b"test-binding-key".to_vec()));
    let rig = rig(vec![adapter.clone() as Arc<dyn ProviderAdapter>]).await;
    Bed { rig, adapter, clock }
}

fn bind(sub: &str, routing_id: &str, lifetime_min: i64, rich: bool) -> Vec<u8> {
    json!({ "bind": { "routing_id": routing_id, "subscription_id": sub, "client_state": STATE, "expires_at": T0 + lifetime_min * 60, "rich": rich } }).to_string().into_bytes()
}

async fn bound(b: &Bed, sub: &str, routing_id: &str, lifetime_min: i64, rich: bool) {
    b.rig.register(FIXTURE_TOKEN, routing_id);
    b.rig.register(OTHER_TOKEN, routing_id);
    let (s, v) = b.rig.post("graph", &[], bind(sub, routing_id, lifetime_min, rich)).await;
    assert_eq!((s, v["signals"].clone()), (202, json!(0)), "bind: {v}");
}

fn bytes(v: &Value) -> Vec<u8> {
    base64::engine::general_purpose::STANDARD.decode(v.as_str().unwrap()).unwrap()
}

#[tokio::test]
async fn a_rich_notification_is_forwarded_still_encrypted_bytes_equal() {
    let b = bed(3_600_000).await;
    bound(&b, "sub-rich-0001", "acct-m1", GRAPH_SUB_RICH_MAX_MIN - 1, true).await;
    let recorded = fixture("notification-rich.json");
    let (s, v) = b.rig.post("graph", &[], recorded.to_string().into_bytes()).await;
    assert_eq!((s, v["queued"].clone()), (202, json!(2)), "one job per device: {v}");
    let alerts = b.rig.wait_alerts("acct-m1", 2).await;
    assert_eq!(b.rig.settle("acct-m1").await, 2, "one alert per device");
    let sent: Value = serde_json::from_str(alerts[0]["body"]["whoosh"]["ref"].as_str().unwrap()).unwrap();
    let original = &recorded["value"][0]["encryptedContent"];
    for field in ["data", "dataKey", "dataSignature"] {
        assert_eq!(bytes(&sent["encryptedContent"][field]), bytes(&original[field]), "{field}: the ciphertext bytes are untouched");
        assert_eq!(sent["encryptedContent"][field], original[field], "{field}: the string is untouched");
    }
    assert_eq!(sent["encryptedContent"]["encryptionCertificateId"], original["encryptionCertificateId"]);
    assert!(alerts[0]["body"].to_string().len() < 4096, "the whole APNs body fits the 4,096 byte limit");
}

#[tokio::test]
async fn an_oversized_encrypted_payload_falls_back_to_the_message_id() {
    let b = bed(3_600_000).await;
    bound(&b, "sub-rich-0001", "acct-m2", GRAPH_SUB_RICH_MAX_MIN - 1, true).await;
    let mut big = fixture("notification-rich.json");
    big["value"][0]["encryptedContent"]["data"] = json!(base64::engine::general_purpose::STANDARD.encode(vec![7u8; REF_MAX_BYTES]));
    b.rig.post("graph", &[], big.to_string().into_bytes()).await;
    let alerts = b.rig.wait_alerts("acct-m2", 2).await;
    assert_eq!(alerts[0]["body"]["whoosh"]["ref"], "AAMkADk1rich", "the phone fetches the mail itself");
    assert!(alerts[0]["body"].to_string().len() < 4096);
}

#[tokio::test]
async fn a_plain_notification_gives_one_alert_and_a_repeat_gives_none() {
    let b = bed(3_600_000).await;
    bound(&b, "sub-plain-0001", "acct-m3", GRAPH_SUB_MAX_MIN - 1, false).await;
    let plain = fixture("notification-plain.json").to_string().into_bytes();
    assert_eq!(b.rig.post("graph", &[], plain.clone()).await.1["queued"], 2);
    b.rig.wait_alerts("acct-m3", 2).await;
    assert_eq!(b.rig.post("graph", &[], plain).await.1["queued"], 0, "Graph redelivers; the message id is the dedup key");
    assert_eq!(b.rig.settle("acct-m3").await, 2);
}

#[tokio::test]
async fn a_wrong_client_state_is_refused_and_unknown_or_non_new_mail_signals_nothing() {
    let b = bed(3_600_000).await;
    bound(&b, "sub-plain-0001", "acct-m4", GRAPH_SUB_MAX_MIN - 1, false).await;
    let mut forged = fixture("notification-plain.json");
    forged["value"][0]["clientState"] = json!("guessed");
    assert_eq!(b.rig.post("graph", &[], forged.to_string().into_bytes()).await.0, 401);
    let mut updated = fixture("notification-plain.json");
    updated["value"][0]["changeType"] = json!("updated");
    assert_eq!(b.rig.post("graph", &[], updated.to_string().into_bytes()).await.1["signals"], 0, "only created mail alerts");
    assert_eq!(b.rig.post("graph", &[], fixture("notification-rich.json").to_string().into_bytes()).await.1["signals"], 0, "an unbound subscription is acknowledged, not routed");
    assert_eq!(b.rig.post("graph", &[], b"{}".to_vec()).await.0, 400);
    assert_eq!(b.rig.settle("acct-m4").await, 0);
}

#[tokio::test]
async fn a_subscription_renews_before_10080_minutes_under_the_fake_clock() {
    let b = bed(20).await;
    let lifetime = GRAPH_SUB_MAX_MIN - 1;
    bound(&b, "sub-plain-0001", "acct-m5", lifetime, false).await;
    let due_min = lifetime - RENEW_MARGIN_MIN;
    b.clock.advance((due_min - 1) * 60);
    tokio::time::sleep(Duration::from_millis(200)).await;
    assert!(b.rig.alerts("acct-m5").is_empty(), "nothing a minute before the margin");
    b.clock.advance(60);
    let alerts = b.rig.wait_alerts("acct-m5", 2).await;
    assert!(alerts.iter().all(|a| a["body"]["whoosh"]["ref"] == "watch-renew:graph"));
    assert!((b.clock.now() - T0) / 60 < GRAPH_SUB_MAX_MIN, "the signal went out before the 10,080 minute ceiling");
}

#[tokio::test]
async fn a_rich_subscription_renews_before_1440_minutes_under_the_fake_clock() {
    let b = bed(3_600_000).await;
    let lifetime = GRAPH_SUB_RICH_MAX_MIN - 1;
    bound(&b, "sub-rich-0001", "acct-m6", lifetime, true).await;
    b.clock.advance((lifetime - RENEW_MARGIN_MIN) * 60 - 1);
    assert_eq!(b.adapter.renew_now(&b.rig.dispatcher), 0);
    b.clock.advance(1);
    assert_eq!(b.adapter.renew_now(&b.rig.dispatcher), 1);
    assert!((b.clock.now() - T0) / 60 < GRAPH_SUB_RICH_MAX_MIN, "the signal went out before the 1,440 minute ceiling");
    b.rig.wait_alerts("acct-m6", 2).await;
    // Not renewed by the device: asked again halfway to the end, never after it.
    b.clock.advance(30 * 60);
    assert_eq!(b.adapter.renew_now(&b.rig.dispatcher), 1, "second ask");
    b.clock.advance(60 * 60);
    assert_eq!(b.adapter.renew_now(&b.rig.dispatcher), 0, "past expiry the relay stops asking");
}

#[tokio::test]
async fn the_lifecycle_notice_asks_the_device_to_renew_and_lifetimes_beyond_graphs_ceiling_are_refused() {
    let b = bed(3_600_000).await;
    bound(&b, "sub-rich-0001", "acct-m7", GRAPH_SUB_RICH_MAX_MIN - 1, true).await;
    let (s, v) = b.rig.post("graph", &[], fixture("lifecycle-reauthorization.json").to_string().into_bytes()).await;
    assert_eq!((s, v["queued"].clone()), (202, json!(2)));
    assert!(b.rig.wait_alerts("acct-m7", 2).await.iter().all(|a| a["body"]["whoosh"]["ref"] == "watch-renew:graph"));
    assert_eq!(b.rig.post("graph", &[], bind("s", "acct-m7", GRAPH_SUB_RICH_MAX_MIN + 1, true)).await.0, 400, "a rich subscription cannot last past 1,440 minutes");
    assert_eq!(b.rig.post("graph", &[], bind("s", "acct-m7", GRAPH_SUB_MAX_MIN + 1, false)).await.0, 400);
}

#[test]
fn the_validation_token_is_echoed_decoded_and_the_ceilings_match_the_ledger() {
    assert_eq!(adapter_graph::validation_reply("validationToken=Validation%3a+Token%20ok").as_deref(), Some("Validation: Token ok"));
    assert_eq!(adapter_graph::validation_reply("?x=1&validationToken=abc").as_deref(), Some("abc"));
    assert_eq!(adapter_graph::validation_reply("x=1"), None);
    let text = std::fs::read_to_string(format!("{}/../docs/CONSTANTS.md", env!("CARGO_MANIFEST_DIR"))).unwrap();
    let row = text.lines().find(|l| l.starts_with("| GRAPH_SUB_RENEW_MIN |")).expect("ledger row");
    assert!(row.contains(&format!("{}", "10,080")) && row.contains("1,440"), "{row}");
    assert_eq!((GRAPH_SUB_MAX_MIN, GRAPH_SUB_RICH_MAX_MIN), (10_080, 1_440));
}

#[tokio::test]
async fn a_bound_subscription_cannot_be_taken_over_but_its_owner_can_release_it() {
    let b = bed(3_600_000).await;
    bound(&b, "sub-plain-0001", "acct-m8", GRAPH_SUB_MAX_MIN - 1, false).await;
    assert_eq!(b.rig.post("graph", &[], bind("sub-plain-0001", "acct-attacker", GRAPH_SUB_MAX_MIN - 1, false)).await.0, 401);
    let release = |routing_id: &str| json!({ "unbind": { "routing_id": routing_id, "subscription_id": "sub-plain-0001" } }).to_string().into_bytes();
    assert_eq!(b.rig.post("graph", &[], release("acct-attacker")).await.0, 202);
    assert_eq!(b.rig.post("graph", &[], fixture("notification-plain.json").to_string().into_bytes()).await.1["queued"], 2, "a non-owner's release changed nothing");
    assert_eq!(b.rig.post("graph", &[], release("acct-m8")).await.0, 202);
    assert_eq!(b.rig.post("graph", &[], fixture("notification-plain.json").to_string().into_bytes()).await.1["signals"], 0);
}
