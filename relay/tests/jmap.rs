//! relay-jmap: a JMAP WebPush StateChange in, one APNs alert per registered device out.
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
#[cfg(feature = "adapter-graph")]
#[path = "../src/adapters/graph/mod.rs"]
mod adapter_graph;
#[path = "../src/adapters/jmap/mod.rs"]
mod adapter_jmap;
#[cfg(feature = "adapter-macsource")]
#[path = "../src/adapters/macsource/mod.rs"]
mod adapter_macsource;
#[path = "relay/common.rs"]
mod common;
#[path = "../src/adapters/testkit.rs"]
mod testkit;

use common::FIXTURE_TOKEN;
use registry::adapter::ProviderAdapter;
use serde_json::{json, Value};
use std::sync::Arc;
use testkit::{rig, FakeClock, Rig, OTHER_TOKEN};

struct Clk(Arc<FakeClock>);

impl adapter_jmap::ledger::Clock for Clk {
    fn now(&self) -> i64 {
        self.0.now()
    }
}

fn fixture(name: &str) -> Vec<u8> {
    std::fs::read(format!("{}/../_captures/relay-jmap/fixtures/{name}", env!("CARGO_MANIFEST_DIR"))).unwrap_or_else(|e| panic!("{name}: {e}"))
}

async fn bed(routing_id: &str) -> Rig {
    let adapter = adapter_jmap::JmapAdapter::new(Arc::new(adapter_jmap::ledger::Ledger::open("jmap", ":memory:")), Arc::new(Clk(FakeClock::at(1_800_000_000))), b"test-binding-key".to_vec());
    let rig = rig(vec![Arc::new(adapter) as Arc<dyn ProviderAdapter>]).await;
    rig.register(FIXTURE_TOKEN, routing_id);
    rig.register(OTHER_TOKEN, routing_id);
    let body = json!({ "bind": { "routing_id": routing_id, "account_id": "u1f2a3b4c" } }).to_string().into_bytes();
    assert_eq!(rig.post("jmap", &[], body).await.0, 202);
    rig
}

#[tokio::test]
async fn a_webpush_fixture_gives_one_alert_per_registered_device() {
    let rig = bed("acct-j1").await;
    let (s, v) = rig.post("jmap", &[("ttl", "86400".into()), ("urgency", "high".into())], fixture("webpush-statechange.json")).await;
    assert_eq!((s, v["queued"].clone()), (202, json!(2)), "{v}");
    let alerts = rig.wait_alerts("acct-j1", 2).await;
    assert_eq!(rig.settle("acct-j1").await, 2, "one alert per device");
    assert_eq!(alerts[0]["body"]["whoosh"]["ref"], "d-state-12", "the phone gets the opaque EmailDelivery state");
}

#[tokio::test]
async fn a_redelivered_push_and_a_flag_change_send_nothing_more() {
    let rig = bed("acct-j2").await;
    rig.post("jmap", &[], fixture("webpush-statechange.json")).await;
    rig.wait_alerts("acct-j2", 2).await;
    assert_eq!(rig.post("jmap", &[], fixture("webpush-statechange.json")).await.1["queued"], 0, "WebPush may redeliver; the delivery state is the dedup key");
    let (s, v) = rig.post("jmap", &[], fixture("webpush-flag-change.json")).await;
    assert_eq!((s, v["signals"].clone()), (202, json!(0)), "Email state alone (flags, folders) is not new mail");
    assert_eq!(rig.settle("acct-j2").await, 2);
}

#[tokio::test]
async fn unbound_accounts_verification_and_junk_signal_nothing() {
    let rig = bed("acct-j3").await;
    let stranger = json!({ "@type": "StateChange", "changed": { "u-someone-else": { "EmailDelivery": "d-1" } } }).to_string().into_bytes();
    assert_eq!(rig.post("jmap", &[], stranger).await.1["signals"], 0);
    let (s, v) = rig.post("jmap", &[], fixture("webpush-verification.json")).await;
    assert_eq!((s, v["signals"].clone()), (202, json!(0)), "acknowledged so the server does not mark the subscription failed");
    assert_eq!(rig.post("jmap", &[], b"{\"@type\":\"Other\"}".to_vec()).await.0, 400);
    assert_eq!(rig.post("jmap", &[], b"nope".to_vec()).await.0, 400);
    assert_eq!(rig.post("jmap", &[], json!({ "bind": { "routing_id": "x y", "account_id": "a" } }).to_string().into_bytes()).await.0, 400);
    assert_eq!(rig.settle("acct-j3").await, 0);
}

#[tokio::test]
async fn a_bound_account_cannot_be_taken_over_but_its_owner_can_release_it() {
    let rig = bed("acct-j4").await;
    let take = json!({ "bind": { "routing_id": "acct-attacker", "account_id": "u1f2a3b4c" } }).to_string().into_bytes();
    assert_eq!(rig.post("jmap", &[], take).await.0, 401);
    assert_eq!(rig.post("jmap", &[], json!({ "unbind": { "routing_id": "acct-attacker", "account_id": "u1f2a3b4c" } }).to_string().into_bytes()).await.0, 202);
    assert_eq!(rig.post("jmap", &[], fixture("webpush-statechange.json")).await.1["queued"], 2, "alerts still go to the owner");
    assert_eq!(rig.post("jmap", &[], json!({ "unbind": { "routing_id": "acct-j4", "account_id": "u1f2a3b4c" } }).to_string().into_bytes()).await.0, 202);
    let again = json!({ "@type": "StateChange", "changed": { "u1f2a3b4c": { "EmailDelivery": "d-state-99" } } }).to_string().into_bytes();
    assert_eq!(rig.post("jmap", &[], again).await.1["signals"], 0);
}
