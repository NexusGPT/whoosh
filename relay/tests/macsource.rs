//! relay-macsource: a Mac-signed event for a paired device becomes one APNs alert; an unsigned or badly signed one becomes none.
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
#[cfg(feature = "adapter-jmap")]
#[path = "../src/adapters/jmap/mod.rs"]
mod adapter_jmap;
#[path = "../src/adapters/macsource/mod.rs"]
mod adapter_macsource;
#[path = "relay/common.rs"]
mod common;
#[path = "../src/adapters/testkit.rs"]
mod testkit;

use common::FIXTURE_TOKEN;
use jsonwebtoken::{encode, Algorithm, EncodingKey, Header};
use p256::ecdsa::SigningKey;
use p256::pkcs8::{EncodePrivateKey, EncodePublicKey, LineEnding};
use registry::adapter::ProviderAdapter;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::sync::Arc;
use testkit::{rig, FakeClock, Rig, OTHER_TOKEN};

struct Clk(Arc<FakeClock>);

impl adapter_macsource::ledger::Clock for Clk {
    fn now(&self) -> i64 {
        self.0.now()
    }
}

struct MacKey {
    private: EncodingKey,
    public_pem: String,
}

fn mac_key() -> MacKey {
    let k = SigningKey::random(&mut rand::rngs::OsRng);
    MacKey {
        private: EncodingKey::from_ec_pem(k.to_pkcs8_pem(LineEnding::LF).unwrap().as_bytes()).unwrap(),
        public_pem: k.verifying_key().to_public_key_pem(LineEnding::LF).unwrap(),
    }
}

fn now() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs() as i64
}

fn signed(key: &MacKey, claims: Value) -> String {
    format!("Bearer {}", encode(&Header::new(Algorithm::ES256), &claims, &key.private).unwrap())
}

fn event_claims(routing_id: &str, jti: &str) -> Value {
    json!({ "sub": routing_id, "jti": jti, "ref": "imap-uid-4411", "iat": now(), "exp": now() + 300 })
}

fn event_body(routing_id: &str) -> Vec<u8> {
    json!({ "routing_id": routing_id }).to_string().into_bytes()
}

async fn paired(routing_id: &str, key: &MacKey) -> Rig {
    let adapter = adapter_macsource::MacSourceAdapter::new(Arc::new(adapter_macsource::ledger::Ledger::open("macsource", ":memory:")), Arc::new(Clk(FakeClock::at(now()))));
    let rig = rig(vec![Arc::new(adapter) as Arc<dyn ProviderAdapter>]).await;
    rig.register(FIXTURE_TOKEN, routing_id);
    rig.register(OTHER_TOKEN, routing_id);
    let pair = json!({ "pair": { "routing_id": routing_id, "public_key_pem": key.public_pem } });
    assert_eq!(rig.post("macsource", &[], pair.to_string().into_bytes()).await.0, 202);
    rig
}

#[tokio::test]
async fn a_mac_signed_event_for_a_paired_device_becomes_one_alert_per_device() {
    let key = mac_key();
    let rig = paired("acct-s1", &key).await;
    let (s, v) = rig.post("macsource", &[("authorization", signed(&key, event_claims("acct-s1", "evt-1")))], event_body("acct-s1")).await;
    assert_eq!((s, v["queued"].clone()), (202, json!(2)), "{v}");
    let alerts = rig.wait_alerts("acct-s1", 2).await;
    assert_eq!(rig.settle("acct-s1").await, 2);
    assert_eq!(alerts[0]["body"]["whoosh"]["ref"], "imap-uid-4411");
    // The same signed event replayed inside its lifetime queues nothing.
    let (_, v) = rig.post("macsource", &[("authorization", signed(&key, event_claims("acct-s1", "evt-1")))], event_body("acct-s1")).await;
    assert_eq!(v["queued"], 0);
    assert_eq!(rig.settle("acct-s1").await, 2);
}

#[tokio::test]
async fn unsigned_badly_signed_expired_and_mismatched_events_produce_no_alert() {
    let key = mac_key();
    let stranger = mac_key();
    let rig = paired("acct-s2", &key).await;
    let stale = json!({ "sub": "acct-s2", "jti": "evt-old", "iat": now() - 7200, "exp": now() - 3600 });
    let cases: Vec<(&str, Vec<(&str, String)>, Vec<u8>)> = vec![
        ("unsigned", vec![], event_body("acct-s2")),
        ("signed by another key", vec![("authorization", signed(&stranger, event_claims("acct-s2", "evt-2")))], event_body("acct-s2")),
        ("expired", vec![("authorization", signed(&key, stale))], event_body("acct-s2")),
        ("signed for another routing id", vec![("authorization", signed(&key, event_claims("acct-other", "evt-3")))], event_body("acct-s2")),
        ("not a token", vec![("authorization", "Bearer abc.def.ghi".into())], event_body("acct-s2")),
        ("unpaired routing id", vec![("authorization", signed(&key, event_claims("acct-nobody", "evt-4")))], event_body("acct-nobody")),
    ];
    for (name, headers, body) in cases {
        assert_eq!(rig.post("macsource", &headers, body).await.0, 401, "{name}");
    }
    assert_eq!(rig.post("macsource", &[("authorization", signed(&key, event_claims("acct-s2", "")))], event_body("acct-s2")).await.0, 400, "a signed event with no id is malformed");
    assert_eq!(rig.settle("acct-s2").await, 0, "no alert for any refused event");
}

#[tokio::test]
async fn re_pairing_a_routing_id_needs_the_old_key_to_sign_the_new_one() {
    let old = mac_key();
    let new = mac_key();
    let rig = paired("acct-s3", &old).await;
    let take_over = json!({ "pair": { "routing_id": "acct-s3", "public_key_pem": new.public_pem } }).to_string().into_bytes();
    assert_eq!(rig.post("macsource", &[], take_over.clone()).await.0, 401, "a stranger cannot replace the key");
    let proof = json!({ "sub": "acct-s3", "pair": hex::encode(Sha256::digest(new.public_pem.as_bytes())), "exp": now() + 300 });
    assert_eq!(rig.post("macsource", &[("authorization", signed(&old, proof))], take_over).await.0, 202, "the old key vouches for the new one");
    assert_eq!(rig.post("macsource", &[("authorization", signed(&old, event_claims("acct-s3", "evt-5")))], event_body("acct-s3")).await.0, 401, "the old key no longer works");
    assert_eq!(rig.post("macsource", &[("authorization", signed(&new, event_claims("acct-s3", "evt-6")))], event_body("acct-s3")).await.1["queued"], 2);
    let junk = json!({ "pair": { "routing_id": "acct-s3", "public_key_pem": "not a key" } }).to_string().into_bytes();
    assert_eq!(rig.post("macsource", &[], junk).await.0, 400);
}
