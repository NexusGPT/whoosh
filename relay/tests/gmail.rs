//! relay-gmail: Pub/Sub push in, one APNs alert per registered device out; the daily renew job
//! under a fake clock. The relay sources compile into this crate (see src/adapters/testkit.rs);
//! the APNs side is relay-core's stub process, the clock is the only fake.
#![allow(dead_code, unused_imports)]
#[path = "../src/registry/mod.rs"]
mod registry;
#[path = "../src/routes/mod.rs"]
mod routes;
#[path = "../src/apns/mod.rs"]
mod apns;
#[path = "../src/tracking/mod.rs"]
mod tracking;
#[path = "../src/adapters/gmail/mod.rs"]
mod adapter_gmail;
#[cfg(feature = "adapter-graph")]
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

use adapter_gmail::{GmailAdapter, GmailConfig, GMAIL_WATCH_RENEW_H};
use common::FIXTURE_TOKEN;
use jsonwebtoken::{encode, EncodingKey, Header};
use registry::adapter::ProviderAdapter;
use serde_json::{json, Value};
use std::sync::Arc;
use std::time::Duration;
use testkit::{rig, FakeClock, Rig, OTHER_TOKEN};

const FIX: &str = "../_captures/relay-gmail/fixtures";
const AUDIENCE: &str = "https://relay.whoosh.test/v1/inbound/gmail";
const PUSH_ACCOUNT: &str = "pubsub-push@whoosh-test.iam.gserviceaccount.com";
const T0: i64 = 1_800_000_000;
const WEEK: i64 = 7 * 24 * 3600;

fn fixture(name: &str) -> String {
    std::fs::read_to_string(format!("{}/{FIX}/{name}", env!("CARGO_MANIFEST_DIR"))).unwrap_or_else(|e| panic!("fixture {name}: {e}"))
}

fn now_real() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs() as i64
}

/// The OIDC bearer token Pub/Sub attaches, signed with the throwaway test key.
fn bearer(audience: &str, email: &str, verified: bool, kid: &str) -> String {
    let mut header = Header::new(jsonwebtoken::Algorithm::RS256);
    header.kid = Some(kid.into());
    let now = now_real();
    let claims = json!({ "iss": "https://accounts.google.com", "aud": audience, "email": email, "email_verified": verified, "iat": now, "exp": now + 3600 });
    let key = EncodingKey::from_rsa_pem(fixture("push-signer-test-private.pem").as_bytes()).unwrap();
    format!("Bearer {}", encode(&header, &claims, &key).unwrap())
}

fn good() -> String {
    bearer(AUDIENCE, PUSH_ACCOUNT, true, "test-key-1")
}

struct Bed {
    rig: Rig,
    adapter: Arc<GmailAdapter>,
    clock: Arc<FakeClock>,
}

fn jwks_path() -> String {
    format!("{}/{FIX}/jwks.json", env!("CARGO_MANIFEST_DIR"))
}

async fn bed(tick_ms: u64) -> Bed {
    let clock = FakeClock::at(T0);
    let config = GmailConfig { jwks_path: Some(jwks_path()), audience: Some(AUDIENCE.into()), push_account: Some(PUSH_ACCOUNT.into()), tick: Duration::from_millis(tick_ms) };
    let ledger = Arc::new(adapter_gmail::ledger::Ledger::open("gmail", ":memory:"));
    let adapter = Arc::new(GmailAdapter::new(config, ledger, Arc::new(Clk(clock.clone())), b"test-binding-key".to_vec()));
    let rig = rig(vec![adapter.clone() as Arc<dyn ProviderAdapter>]).await;
    Bed { rig, adapter, clock }
}

struct Clk(Arc<FakeClock>);

impl adapter_gmail::ledger::Clock for Clk {
    fn now(&self) -> i64 {
        self.0.now()
    }
}

fn bind_body(routing_id: &str, expires_in: i64) -> Vec<u8> {
    json!({ "bind": { "routing_id": routing_id, "email_address": "Owner@Example.test", "expires_at": T0 + expires_in } }).to_string().into_bytes()
}

fn push_body() -> Vec<u8> {
    fixture("pubsub-push.json").into_bytes()
}

async fn bound(b: &Bed, routing_id: &str) {
    b.rig.register(FIXTURE_TOKEN, routing_id);
    b.rig.register(OTHER_TOKEN, routing_id);
    let (s, v) = b.rig.post("gmail", &[], bind_body(routing_id, WEEK)).await;
    assert_eq!((s, v["signals"].clone()), (202, json!(0)), "a bind is acknowledged and signals nothing: {v}");
}

#[tokio::test]
async fn a_recorded_pubsub_push_gives_one_alert_per_registered_device() {
    let b = bed(3_600_000).await;
    bound(&b, "acct-g1").await;
    let (s, v) = b.rig.post("gmail", &[("authorization", good())], push_body()).await;
    assert_eq!((s, v["queued"].clone()), (202, json!(2)), "two devices, two jobs: {v}");
    let alerts = b.rig.wait_alerts("acct-g1", 2).await;
    assert_eq!(b.rig.settle("acct-g1").await, 2, "one alert per device, no more");
    let mut tokens: Vec<_> = alerts.iter().map(|a| a["token"].as_str().unwrap().to_string()).collect();
    tokens.sort();
    let mut expected = vec![FIXTURE_TOKEN.to_string(), OTHER_TOKEN.to_string()];
    expected.sort();
    assert_eq!(tokens, expected);
    assert_eq!(alerts[0]["body"]["whoosh"]["ref"], "9876543210", "the phone gets the opaque history id");
    assert!(!alerts[0]["body"].to_string().contains("example.test"), "the address never reaches the phone payload");
}

#[tokio::test]
async fn a_redelivered_push_sends_nothing_more() {
    let b = bed(3_600_000).await;
    bound(&b, "acct-g2").await;
    b.rig.post("gmail", &[("authorization", good())], push_body()).await;
    b.rig.wait_alerts("acct-g2", 2).await;
    let (_, v) = b.rig.post("gmail", &[("authorization", good())], push_body()).await;
    assert_eq!(v["queued"], 0, "Pub/Sub delivers at least once; the history id is the dedup key");
    assert_eq!(b.rig.settle("acct-g2").await, 2);
}

#[tokio::test]
async fn pushes_that_fail_the_proof_are_refused_and_signal_nothing() {
    let b = bed(3_600_000).await;
    bound(&b, "acct-g3").await;
    let cases: Vec<(&str, Vec<(&str, String)>)> = vec![
        ("no token", vec![]),
        ("wrong audience", vec![("authorization", bearer("https://evil.test", PUSH_ACCOUNT, true, "test-key-1"))]),
        ("wrong service account", vec![("authorization", bearer(AUDIENCE, "other@whoosh-test.iam.gserviceaccount.com", true, "test-key-1"))]),
        ("unverified email", vec![("authorization", bearer(AUDIENCE, PUSH_ACCOUNT, false, "test-key-1"))]),
        ("unknown key id", vec![("authorization", bearer(AUDIENCE, PUSH_ACCOUNT, true, "other-key"))]),
        ("garbage token", vec![("authorization", "Bearer not.a.jwt".to_string())]),
    ];
    for (name, headers) in cases {
        let (s, _) = b.rig.post("gmail", &headers, push_body()).await;
        assert_eq!(s, 401, "{name}");
    }
    assert_eq!(b.rig.settle("acct-g3").await, 0, "nothing was sent for any refused push");
}

#[tokio::test]
async fn malformed_and_unbound_pushes_are_handled_without_an_alert() {
    let b = bed(3_600_000).await;
    bound(&b, "acct-g4").await;
    let (s, _) = b.rig.post("gmail", &[("authorization", good())], b"{\"message\":{\"data\":\"!!\"}}".to_vec()).await;
    assert_eq!(s, 400);
    let (s, _) = b.rig.post("gmail", &[], b"nope".to_vec()).await;
    assert_eq!(s, 400);
    // An address nobody bound is acknowledged (so Pub/Sub stops retrying) and signals nothing.
    let data = base64_std(r#"{"emailAddress":"stranger@example.test","historyId":"5"}"#);
    let body = json!({ "message": { "data": data } }).to_string().into_bytes();
    let (s, v) = b.rig.post("gmail", &[("authorization", good())], body).await;
    assert_eq!((s, v["signals"].clone()), (202, json!(0)));
    assert_eq!(b.rig.settle("acct-g4").await, 0);
}

fn base64_std(s: &str) -> String {
    use base64::Engine;
    base64::engine::general_purpose::STANDARD.encode(s)
}

#[tokio::test]
async fn the_renew_job_runs_inside_24_hours_under_the_fake_clock() {
    let b = bed(20).await;
    bound(&b, "acct-g5").await;
    // Just before the 24 h mark nothing is due.
    b.clock.advance(GMAIL_WATCH_RENEW_H * 3600 - 1);
    tokio::time::sleep(Duration::from_millis(200)).await;
    assert!(b.rig.alerts("acct-g5").is_empty(), "no renew signal before 24 h");
    // At 24 h the running job (the loop `start` launched, ticking every 20 ms) sends one signal per device.
    b.clock.advance(1);
    let alerts = b.rig.wait_alerts("acct-g5", 2).await;
    assert!(alerts.iter().all(|a| a["body"]["whoosh"]["ref"] == "watch-renew:gmail"));
    // The job does not repeat inside the same period.
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert_eq!(b.rig.alerts("acct-g5").len(), 2, "one renew signal per device per period");
}

#[tokio::test]
async fn a_rebind_after_renewal_moves_the_next_renewal_out() {
    let b = bed(3_600_000).await;
    bound(&b, "acct-g6").await;
    b.clock.advance(GMAIL_WATCH_RENEW_H * 3600 - 60);
    let (s, _) = b.rig.post("gmail", &[], json!({ "bind": { "routing_id": "acct-g6", "email_address": "owner@example.test", "expires_at": T0 + 2 * WEEK } }).to_string().into_bytes()).await;
    assert_eq!(s, 202);
    b.clock.advance(120);
    assert_eq!(b.adapter.renew_now(&b.rig.dispatcher), 0, "the device renewed, so nothing is due at the old time");
    b.clock.advance(GMAIL_WATCH_RENEW_H * 3600);
    assert_eq!(b.adapter.renew_now(&b.rig.dispatcher), 1, "due again 24 h after the new bind");
}

#[tokio::test]
async fn a_watch_that_lapsed_is_no_longer_nudged_and_bad_binds_are_refused() {
    let b = bed(3_600_000).await;
    b.rig.register(FIXTURE_TOKEN, "acct-g7");
    let (s, _) = b.rig.post("gmail", &[], bind_body("acct-g7", 3_600)).await;
    assert_eq!(s, 202);
    b.clock.advance(7_200);
    assert_eq!(b.adapter.renew_now(&b.rig.dispatcher), 0, "past expiry the watch is lapsed; the device finds out on open");
    for bad in [
        json!({ "bind": { "routing_id": "bad id!", "email_address": "a@b.test", "expires_at": T0 + 100 } }),
        json!({ "bind": { "routing_id": "acct-g7", "email_address": "nobody", "expires_at": T0 + 100 } }),
        json!({ "bind": { "routing_id": "acct-g7", "email_address": "a@b.test", "expires_at": T0 - 5 } }),
    ] {
        assert_eq!(b.rig.post("gmail", &[], bad.to_string().into_bytes()).await.0, 400);
    }
}

#[test]
fn the_renew_period_is_the_ledger_value_and_under_gmails_seven_days() {
    let text = std::fs::read_to_string(format!("{}/../docs/CONSTANTS.md", env!("CARGO_MANIFEST_DIR"))).unwrap();
    let row = text.lines().find(|l| l.starts_with("| GMAIL_WATCH_RENEW_H |")).expect("ledger row");
    let value: i64 = row.split('|').nth(2).unwrap().trim().split_whitespace().next().unwrap().parse().unwrap();
    assert_eq!(GMAIL_WATCH_RENEW_H, value, "the adapter constant mirrors docs/CONSTANTS.md");
    assert!(GMAIL_WATCH_RENEW_H * 3600 < WEEK);
}

#[tokio::test]
async fn a_bound_address_cannot_be_taken_over_but_its_owner_can_release_it() {
    let b = bed(3_600_000).await;
    bound(&b, "acct-g8").await;
    b.rig.register(FIXTURE_TOKEN, "acct-attacker");
    let (s, _) = b.rig.post("gmail", &[], bind_body("acct-attacker", WEEK)).await;
    assert_eq!(s, 401, "a stranger who knows the address cannot rebind it");
    let (_, v) = b.rig.post("gmail", &[("authorization", good())], push_body()).await;
    assert_eq!(v["queued"], 2, "alerts still go to the owner's devices");
    assert!(b.rig.alerts("acct-attacker").is_empty());
    let release = |routing_id: &str| json!({ "unbind": { "routing_id": routing_id, "email_address": "owner@example.test" } }).to_string().into_bytes();
    assert_eq!(b.rig.post("gmail", &[], release("acct-attacker")).await.0, 202, "release is idempotent and silent for a non-owner");
    assert_eq!(b.rig.post("gmail", &[("authorization", good())], serde_json::to_vec(&json!({ "message": { "data": base64_std(r#"{"emailAddress":"owner@example.test","historyId":"11"}"#) } })).unwrap()).await.1["queued"], 2, "the non-owner's release changed nothing");
    assert_eq!(b.rig.post("gmail", &[], release("acct-g8")).await.0, 202);
    let after = json!({ "message": { "data": base64_std(r#"{"emailAddress":"owner@example.test","historyId":"12"}"#) } });
    assert_eq!(b.rig.post("gmail", &[("authorization", good())], serde_json::to_vec(&after).unwrap()).await.1["signals"], 0, "released: acknowledged, no signal");
    assert_eq!(b.rig.post("gmail", &[], bind_body("acct-attacker", WEEK)).await.0, 202, "after a release the address can be bound again");
}
