//! Shared test rig: spawns the real relay binary and the local APNs stub, never an in-process fake.
use p256::ecdsa::SigningKey;
use p256::pkcs8::{EncodePrivateKey, EncodePublicKey, LineEnding};
use serde_json::Value;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc;
use std::time::{Duration, Instant};

pub const FIXTURE_TOKEN: &str = "00fc13adff785122b4ad28809a3420982341241421348097878e577c991de8f0";
pub const GONE_TOKEN: &str = "ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff";
pub const SIGNAL_TOKEN: &str = "test-signal-token";
const WAIT: Duration = Duration::from_secs(20);

pub struct Proc(Child);
impl Drop for Proc {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn spawn(args: &[&str], envs: &[(&str, String)]) -> (Proc, String) {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_relay"));
    cmd.args(args).stdout(Stdio::piped()).stderr(Stdio::inherit());
    for (k, v) in envs {
        cmd.env(k, v);
    }
    let mut child = cmd.spawn().expect("spawn relay binary");
    let out = child.stdout.take().expect("piped stdout");
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        for line in BufReader::new(out).lines().map_while(Result::ok) {
            if let Some(url) = line.strip_prefix("listening on ") {
                let _ = tx.send(url.to_string());
            }
        }
    });
    let url = rx.recv_timeout(WAIT).expect("the process printed 'listening on ...'");
    (Proc(child), url)
}

pub fn work_dir(name: &str) -> PathBuf {
    let d = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(format!("{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).expect("create work dir");
    d
}

pub struct Stub {
    pub _proc: Proc,
    pub url: String,
    pub log: PathBuf,
    pub key_path: PathBuf,
}

pub fn start_stub(dir: &Path) -> Stub {
    let key = SigningKey::random(&mut rand::rngs::OsRng);
    let key_path = dir.join("AuthKey_TEST.p8");
    std::fs::write(&key_path, key.to_pkcs8_pem(LineEnding::LF).expect("pem").as_bytes()).expect("write key");
    let pub_path = dir.join("pub.pem");
    std::fs::write(&pub_path, key.verifying_key().to_public_key_pem(LineEnding::LF).expect("pub pem")).expect("write pub");
    let log = dir.join("stub.log");
    let (proc_, url) = spawn(
        &["apns-stub", "--bind", "127.0.0.1:0", "--pub-key", pub_path.to_str().unwrap(), "--ok-token", FIXTURE_TOKEN, "--gone-token", GONE_TOKEN, "--log", log.to_str().unwrap()],
        &[],
    );
    Stub { _proc: proc_, url, log, key_path }
}

impl Stub {
    pub fn lines(&self) -> Vec<Value> {
        std::fs::read_to_string(&self.log).unwrap_or_default().lines().filter_map(|l| serde_json::from_str(l).ok()).collect()
    }
}

pub struct Relay {
    pub _proc: Proc,
    pub base: String,
    pub db: PathBuf,
    pub http: reqwest::Client,
}

pub fn start_relay(dir: &Path, stub: Option<&Stub>) -> Relay {
    let db = dir.join("relay.sqlite3");
    let mut envs = vec![("RELAY_DB", db.to_str().unwrap().to_string()), ("RELAY_SIGNAL_TOKEN", SIGNAL_TOKEN.to_string())];
    if let Some(s) = stub {
        envs.push(("RELAY_APNS_KEY_PATH", s.key_path.to_str().unwrap().to_string()));
        envs.push(("RELAY_APNS_KEY_ID", "TESTKEYID01".into()));
        envs.push(("RELAY_APNS_TEAM_ID", "TESTTEAM01".into()));
        envs.push(("RELAY_APNS_TOPIC", "app.whoosh.test".into()));
        envs.push(("RELAY_APNS_SANDBOX_URL", s.url.clone()));
        envs.push(("RELAY_APNS_PRODUCTION_URL", s.url.clone()));
    }
    let (proc_, base) = spawn(&[], &envs);
    let http = reqwest::Client::builder().redirect(reqwest::redirect::Policy::none()).build().expect("http client");
    Relay { _proc: proc_, base, db, http }
}

impl Relay {
    pub async fn get(&self, path: &str) -> (u16, Value) {
        let r = self.http.get(format!("{}{path}", self.base)).send().await.expect("GET");
        (r.status().as_u16(), r.json().await.unwrap_or(Value::Null))
    }

    pub async fn post(&self, path: &str, body: Value) -> (u16, Value) {
        let r = self.http.post(format!("{}{path}", self.base)).json(&body).send().await.expect("POST");
        (r.status().as_u16(), r.json().await.unwrap_or(Value::Null))
    }

    pub async fn register(&self, token: &str, routing_id: &str) -> (u16, Value) {
        self.post("/v1/registrations", serde_json::json!({ "device_token": token, "routing_id": routing_id, "environment": "sandbox" })).await
    }

    pub async fn signal(&self, signal_id: &str, routing_id: &str) -> (u16, Value) {
        let r = self
            .http
            .post(format!("{}/v1/inbound/loopback", self.base))
            .bearer_auth(SIGNAL_TOKEN)
            .json(&serde_json::json!({ "signal_id": signal_id, "routing_id": routing_id, "opaque_ref": "history-42" }))
            .send()
            .await
            .expect("signal");
        (r.status().as_u16(), r.json().await.unwrap_or(Value::Null))
    }

    pub async fn wait_job_state(&self, state: &str, at_least: i64) -> Value {
        let start = Instant::now();
        loop {
            let (_, h) = self.get("/v1/health").await;
            if h["jobs"][state].as_i64().unwrap_or(0) >= at_least {
                return h;
            }
            assert!(start.elapsed() < WAIT, "jobs.{state} never reached {at_least}; health: {h}");
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    }
}
