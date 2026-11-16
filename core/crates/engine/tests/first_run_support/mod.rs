//! Shared by `engine_first_run.rs` (T-26) and `state_parity.rs` (T-03, T-04): a real core on a new installation after a first sync against fake IMAP servers
//! (docs/lanes/realcore-firstrun.testplan.md section 5). Nothing is seeded: the store is empty, the accounts are added through `AddAccountConfigured`, the
//! sync lists the folders. Two accounts: the first server sends SPECIAL-USE (INBOX, "Éléments envoyés", "Projects"), the second does not ("Sent Items", "Receipts").
#![allow(dead_code)]
#[allow(dead_code, unused_imports)]
#[path = "../../../autoconfig/tests/support/mod.rs"]
pub mod detect_fakes;
#[allow(dead_code, unused_imports)]
#[path = "../../../sync-imap/tests/support/mod.rs"]
pub mod imap_fakes;

use std::sync::Arc;
use std::time::{Duration, Instant};

use contracts::*;
use detect_fakes::{FakeDns, FakeHttp, FakeProbe};
use engine::{RustCore, TestTransport};
use imap_fakes::{smtp::FakeSmtp, FakeImap, Mbox};
use secrets::{MemoryKeychain, Namespace};
use serde_json::{json, Value};
use store::Store;

pub const PW: &str = "app-password";
pub const ONE: &str = "one@example.test";
pub const TWO: &str = "two@example.test";

pub fn detector() -> Box<dyn engine::Detect> { Box::new(autoconfig::Detector::new(FakeHttp::default(), FakeDns::default(), FakeProbe::default())) }

fn mbox(name: &str, special: Option<&'static str>) -> Mbox {
    Mbox { name: name.into(), special, uid_validity: 1000, next_uid: 1, highest_modseq: 1, msgs: vec![], expunged: vec![], version: 0 }
}

pub struct Rig {
    pub core: RustCore,
    pub accounts: Vec<String>,
    pub imaps: Vec<FakeImap>,
    _smtps: Vec<FakeSmtp>,
    _rt: tokio::runtime::Runtime,
    _dir: tempfile::TempDir,
}

pub fn wait_until(what: &str, mut ok: impl FnMut() -> bool) {
    let t0 = Instant::now();
    while t0.elapsed() < Duration::from_secs(30) {
        if ok() { return; }
        std::thread::sleep(Duration::from_millis(50));
    }
    panic!("never reached within 30 s: {what}");
}

impl Rig {
    /// A fresh core, two accounts added, their first sync finished (every folder stored, both accounts listening). `messages` = how many messages the
    /// first account's INBOX holds (the golden uses 0: counts and progress then do not depend on timing).
    pub fn first_sync(messages: u32) -> Rig {
        let rt = tokio::runtime::Runtime::new().unwrap();
        let one = rt.block_on(FakeImap::start_with(true));
        one.with(|s| s.boxes = vec![mbox("INBOX", None), mbox("Éléments envoyés", Some("\\Sent")), mbox("Projects", None)]);
        let two = rt.block_on(FakeImap::start_with(false));
        two.with(|s| s.boxes = vec![mbox("INBOX", None), mbox("Sent Items", None), mbox("Receipts", None)]);
        if messages > 0 { one.seed("INBOX", messages); }
        let smtps = vec![rt.block_on(FakeSmtp::start(ONE, PW)), rt.block_on(FakeSmtp::start(TWO, PW))];
        let dir = tempfile::tempdir().unwrap();
        let store = Store::open(&dir.path().join("whoosh.sqlite")).unwrap();
        let core = RustCore::with_parts(store, Arc::new(MemoryKeychain::new(Namespace::rig())), detector(), TestTransport::PlainLoopback);
        let mut accounts = vec![];
        for (user, imap, smtp) in [(ONE, &one, &smtps[0]), (TWO, &two, &smtps[1])] {
            let config = AccountConfig {
                imap_host: "127.0.0.1".into(), imap_port: imap.port, imap_security: ConnectionSecurity::Ssl, smtp_host: "127.0.0.1".into(), smtp_port: smtp.port,
                smtp_security: ConnectionSecurity::Starttls, username: user.into(), smtp_username: user.into(), auth_method: AuthMethod::Password, source: DetectionSource::Manual,
            };
            let id = core.run(Command::AddAccountConfigured { address: user.into(), display_name: Some(user.into()), config, password: SecretText { value: PW.into() } }).expect("added").created_id.unwrap();
            accounts.push(id);
        }
        let rig = Rig { core, accounts, imaps: vec![one, two], _smtps: smtps, _rt: rt, _dir: dir };
        wait_until("both accounts finished their first sync", || {
            rig.core.sync_states().iter().filter(|s| s.history_fetched).count() == 2
                && rig.accounts.iter().all(|a| rig.core.sync_phase(a) == Some(engine::SyncPhase::Listening))
        });
        wait_until("six folders are sources", || rig.sources_folder_count() == 6);
        // the pane derives a folder's tile from the stored messages and the sync's own record, so it is final once both accounts finished; waiting for the
        // enabled folders to leave "not started" is a guard that fails loudly if a tile were ever to stay there
        wait_until("every enabled folder reads a sync state", || rig.enabled_folders_reported());
        rig
    }

    fn enabled_folders_reported(&self) -> bool {
        self.state("reply/sources").and_then(|s| serde_json::from_str::<Value>(&s).ok()).is_some_and(|v| {
            v["accounts"].as_array().is_some_and(|a| a.iter().flat_map(|c| c["folders"].as_array().cloned().unwrap_or_default())
                .filter(|f| f["enabled"] == true).all(|f| f["progress"]["state"] != "not_started"))
        })
    }

    fn sources_folder_count(&self) -> usize {
        self.state("reply/sources").and_then(|s| serde_json::from_str::<Value>(&s).ok())
            .map(|v| v["accounts"].as_array().map(|a| a.iter().map(|c| c["folders"].as_array().map(|f| f.len()).unwrap_or(0)).sum()).unwrap_or(0)).unwrap_or(0)
    }

    pub fn state(&self, path: &str) -> Option<String> { self.core.state(path.to_string()) }

    /// The text with each account's real id replaced by a placeholder, so a golden does not hold a generated id.
    pub fn mask(&self, text: &str) -> String {
        let mut t = text.to_string();
        for (i, a) in self.accounts.iter().enumerate() { t = t.replace(a.as_str(), &format!("$ACCOUNT_{}", i + 1)); }
        t
    }

    /// One state path as the golden holds it: its JSON (account ids masked), or null when the core answers nothing.
    pub fn path_value(&self, path: &str) -> Value {
        match self.state(path) {
            None => Value::Null,
            Some(text) => serde_json::from_str(&self.mask(&text)).unwrap_or_else(|e| panic!("{path} is not JSON: {text:?}: {e}")),
        }
    }

    /// Replaces the placeholders a path pattern may hold: `$ACCOUNT_n` by the real id, `*` is not expanded here.
    pub fn unmask(&self, path: &str) -> String {
        let mut p = path.to_string();
        for (i, a) in self.accounts.iter().enumerate() { p = p.replace(&format!("$ACCOUNT_{}", i + 1), a); }
        p
    }
}

// ---------------------------------------------------------------- records as JSON (the contract has no serde)

pub fn label_json(l: &MailLabel) -> Value {
    json!({
        "id": l.id, "name": l.name, "parent_id": l.parent_id, "colour": l.colour, "path": l.path, "depth": l.depth, "description": l.description, "pinned": l.pinned,
        "scope": match &l.scope { LabelScope::All => json!("all"), LabelScope::Account { account_id } => json!({"account": account_id}) },
        "kind": match l.kind { LabelKind::BuiltIn => "built_in", LabelKind::Owner => "owner" }, "order": l.order,
        "entry_enabled": l.entry_enabled, "ai_enabled": l.ai_enabled, "exit_enabled": l.exit_enabled,
        "ai_reads": {"subject": l.ai_reads.subject, "body": l.ai_reads.body, "rest_of_thread": l.ai_reads.rest_of_thread, "attachments": l.ai_reads.attachments},
        "exit_fallback": l.exit_fallback,
    })
}

pub fn provider_json(p: &AiProvider) -> Value {
    json!({"id": p.id, "name": p.name, "models": p.models, "connected": p.connected, "keychain": p.keychain.is_some(), "mail_consent_at_ms": p.mail_consent_at_ms})
}

pub fn permission_word(p: &AgentPermission) -> &'static str {
    match p { AgentPermission::ReadMail => "read_mail", AgentPermission::Search => "search", AgentPermission::Draft => "draft", AgentPermission::Label => "label", AgentPermission::SendWithConfirmation => "send_with_confirmation" }
}

pub fn agent_json(a: &AgentConfig) -> Value {
    json!({"id": a.id, "name": a.name, "purpose": a.purpose, "provider_id": a.provider_id, "model": a.model, "instructions": a.instructions,
        "allowed": a.allowed.iter().map(permission_word).collect::<Vec<_>>(), "built_in": a.built_in})
}

pub fn speech_json(s: &SpeechEngine) -> Value {
    json!({"kind": match &s.kind { SpeechEngineKind::Local => json!("local"), SpeechEngineKind::Cloud { provider_id } => json!({"cloud": provider_id}) }, "ready": s.ready})
}

pub fn decision_json(d: &Option<DecisionModel>) -> Value {
    match d { None => Value::Null, Some(d) => json!({"provider": format!("{:?}", d.provider), "model": d.model, "keychain": d.keychain.is_some(), "decisions_enabled": d.decisions_enabled, "last_run_ms": d.last_run_ms}) }
}

/// The reads of the panes that are not state paths, as JSON. Accounts are counted by their address (their ids are generated).
pub fn reads(rig: &Rig) -> Value {
    let mut labels = rig.core.labels();
    labels.sort_by(|a, b| a.order.cmp(&b.order).then(a.id.cmp(&b.id)));
    let accounts: Vec<Value> = rig.core.accounts().iter().map(|a| json!({"address": a.address, "name": a.name, "provider": format!("{:?}", a.provider), "unread": a.unread, "healthy": a.health.is_none()})).collect();
    json!({
        "labels": labels.iter().map(label_json).collect::<Vec<_>>(),
        "ai_providers": rig.core.ai_providers().iter().map(provider_json).collect::<Vec<_>>(),
        "agent_configs": rig.core.agent_configs().iter().map(agent_json).collect::<Vec<_>>(),
        "decision_model": decision_json(&rig.core.decision_model()),
        "speech_engine": speech_json(&rig.core.speech_engine()),
        "accounts": accounts,
    })
}

// ---------------------------------------------------------------- the golden

/// The state paths a new installation answers, as the golden lists them: the fixed ones, then one group per label and per provider and agent.
pub fn first_run_paths(rig: &Rig) -> Vec<String> {
    let mut paths: Vec<String> = [
        "reply/use/auto", "reply/use/on_demand", "reply/filter", "reply/on_demand", "reply/counts", "reply/sources", "reply/retrieval", "reply/voice",
        "reply/prompt/current", "reply/prompt/versions", "reply/prompt/variables", "reply/notes", "reply/calendars/enabled", "reply/calendars/events", "reply/progress",
        "speech_engine/chosen/state", "inbox/tabs",
    ].iter().map(|s| s.to_string()).collect();
    for l in rig.core.labels() {
        for leaf in ["flow/entry_enabled", "flow/ai_enabled", "flow/exit_enabled", "flow/ai_reads", "flow/exit_fallback", "flow/counts", "flow/ai_refusal", "chains/entry", "chains/exit"] {
            paths.push(format!("label/{}/{leaf}", l.id));
        }
    }
    for p in rig.core.ai_providers() {
        for leaf in ["state", "connected", "models", "refusal", "key"] { paths.push(format!("ai_provider/{}/{leaf}", p.id)); }
        paths.push(format!("provider_credential/{}/stored", p.id));
    }
    for a in rig.core.agent_configs() { paths.push(format!("agent_config/{}/allowed", a.id)); }
    paths
}

/// The whole first-run snapshot of a real core: `{"reads": {...}, "state": {path: value or null}}`.
pub fn snapshot(rig: &Rig) -> Value {
    let mut state = serde_json::Map::new();
    for p in first_run_paths(rig) { state.insert(p.clone(), rig.path_value(&p)); }
    json!({"reads": reads(rig), "state": Value::Object(state)})
}

pub fn golden_file() -> std::path::PathBuf { std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../docs/FIRST-RUN.json") }

/// Every leaf of a JSON value as (dotted path, leaf text); arrays by index.
pub fn leaves(v: &Value, at: &str, out: &mut Vec<(String, String)>) {
    match v {
        Value::Object(m) if !m.is_empty() => for (k, x) in m { leaves(x, &if at.is_empty() { k.clone() } else { format!("{at}.{k}") }, out); },
        Value::Array(a) if !a.is_empty() => for (i, x) in a.iter().enumerate() { leaves(x, &format!("{at}[{i}]"), out); },
        other => out.push((at.to_string(), other.to_string())),
    }
}

/// The differences between a golden and what the core answers, one line per leaf, path by path. Empty = equal.
pub fn diff(golden: &Value, got: &Value) -> Vec<String> {
    let (mut g, mut c) = (vec![], vec![]);
    leaves(golden, "", &mut g);
    leaves(got, "", &mut c);
    let mut out = vec![];
    for (path, want) in &g {
        match c.iter().find(|(p, _)| p == path) {
            None => out.push(format!("MISSING  {path}: the golden says {want}, the core has nothing there")),
            Some((_, have)) if have != want => out.push(format!("DIFFERS  {path}: the golden says {want}, the core answers {have}")),
            _ => {}
        }
    }
    for (path, have) in &c { if !g.iter().any(|(p, _)| p == path) { out.push(format!("EXTRA    {path}: the core answers {have}, the golden has nothing there")); } }
    out
}
