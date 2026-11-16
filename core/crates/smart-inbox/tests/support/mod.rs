#![allow(dead_code)]
//! Shared by the smart-inbox tests: the labelled fixture, a store seeded through the REAL arrival path
//! (`PlaceNewMessages`), the fixture's model, and ledger reads. Numbers come from docs/CONSTANTS.md.
use contracts::{DecisionAnswer, DecisionPort, DecisionProviderKind, DecisionQuestion, DecisionResult, DecisionState, CoreError, Message};
use serde_json::Value;
use smart_inbox::{header_facts, AskModel, Incoming, ModelAnswer, ModelAsk, ModelVerdict, PlaceNewMessages, DecisionPage};
use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use store::{NewMessage, Store};

pub struct TempDb(pub PathBuf);
impl TempDb {
    pub fn new(tag: &str) -> TempDb {
        // The clock ticks in microseconds on this Mac, so two tests with one tag in one process can read the same time:
        // a process-wide counter keeps their folders apart (the flake of the first counts run was this collision).
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let n = NEXT.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        let dir = std::env::temp_dir().join(format!("whoosh-smartinbox-{tag}-{}-{n}-{}", std::process::id(), std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
        std::fs::create_dir_all(&dir).unwrap();
        TempDb(dir.join("whoosh.db"))
    }
}
impl Drop for TempDb {
    fn drop(&mut self) {
        if let Some(d) = self.0.parent() {
            let _ = std::fs::remove_dir_all(d);
        }
    }
}

const LEDGER: &str = include_str!("../../../../../docs/CONSTANTS.md");

/// The first number (integer or decimal) in the Value cell of a ledger row.
pub fn constant_f64(name: &str) -> f64 {
    let line = LEDGER.lines().find(|l| l.trim_start().starts_with(&format!("| {name} |"))).unwrap_or_else(|| panic!("docs/CONSTANTS.md has no {name} row"));
    let cell = line.split('|').nth(2).unwrap().trim();
    let num: String = cell.chars().skip_while(|c| !c.is_ascii_digit()).take_while(|c| c.is_ascii_digit() || *c == '.').collect();
    num.parse().unwrap_or_else(|_| panic!("{name} cell {cell:?} holds no number"))
}

pub fn fixture() -> Value {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../fixtures/smart-inbox.json");
    serde_json::from_str(&std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()))).unwrap()
}

pub fn s(v: &Value, k: &str) -> String {
    v[k].as_str().unwrap_or("").to_string()
}
pub fn strs(v: &Value, k: &str) -> Vec<String> {
    v[k].as_array().map(|a| a.iter().filter_map(|x| x.as_str().map(String::from)).collect()).unwrap_or_default()
}
pub fn tab_id_of(label: &str) -> String {
    store::tab_id(label)
}

/// What the fixture says the model answers, per message id. A message with no entry gets no answer.
pub struct FixtureModel {
    answers: HashMap<String, Value>,
    pub asked: Mutex<Vec<(String, DecisionPage)>>,
    pub as_kind: DecisionProviderKind,
}
impl FixtureModel {
    pub fn new(fx: &Value, as_kind: DecisionProviderKind) -> FixtureModel {
        let answers = fx["messages"].as_array().unwrap().iter().filter(|m| !m["model"].is_null()).map(|m| (s(m, "id"), m["model"].clone())).collect();
        FixtureModel { answers, asked: Mutex::new(vec![]), as_kind }
    }
}
impl AskModel for FixtureModel {
    fn execute(&self, ask: &ModelAsk<'_>) -> Option<ModelVerdict> {
        self.asked.lock().unwrap().push((ask.facts.message_id.clone(), ask.page));
        let a = self.answers.get(&ask.facts.message_id)?;
        match ask.page {
            DecisionPage::WhichTab => {
                let t = a.get("tab")?;
                Some(ModelVerdict { answer: ModelAnswer::Choice(s(t, "choice")), probability: t["p"].as_f64()?, by: self.as_kind })
            }
            DecisionPage::NeedsMyReply => {
                let t = a.get("needs_reply")?;
                Some(ModelVerdict { answer: ModelAnswer::Yes(t["yes"].as_bool()?), probability: t["p"].as_f64()?, by: self.as_kind })
            }
        }
    }
}

/// A DecisionPort that records what it was sent and answers like the fixture model.
pub struct RecordingPort {
    pub kind: DecisionProviderKind,
    pub calls: Mutex<Vec<(DecisionState, Vec<DecisionQuestion>)>>,
    pub answer_choice: Option<(String, f64)>,
}
impl DecisionPort for RecordingPort {
    fn provider(&self) -> DecisionProviderKind {
        self.kind
    }
    fn decide(&self, state: DecisionState, questions: Vec<DecisionQuestion>) -> Result<DecisionResult, CoreError> {
        self.calls.lock().unwrap().push((state, questions.clone()));
        let answers = questions
            .iter()
            .map(|q| match q {
                DecisionQuestion::Choice { id, .. } => {
                    let (c, p) = self.answer_choice.clone().unwrap_or(("Primary".into(), 0.5));
                    DecisionAnswer::Choice { id: id.clone(), choice: c.clone(), probabilities: HashMap::from([(c, p)]) }
                }
                DecisionQuestion::Score { id, .. } => DecisionAnswer::Score { id: id.clone(), value: 0.0 },
                DecisionQuestion::Noul { id, .. } => DecisionAnswer::Noul { id: id.clone(), yes: false, probability: 0.1 },
            })
            .collect();
        Ok(DecisionResult { answers, answered_by: self.kind, checkpoint: None })
    }
}

pub fn message_of(m: &Value) -> Message {
    Message { list_unsubscribe: vec![], list_unsubscribe_one_click: false,
        id: s(m, "id"), thread_id: s(m, "thread"), account_id: s(m, "account"), sender_name: s(m, "from"), from_address: s(m, "from"),
        to: strs(m, "to"), subject: s(m, "subject"), preview: s(m, "body"), received_at_ms: m["at_ms"].as_i64().unwrap(),
        read: m["read"].as_bool().unwrap(), archived: m["archived"].as_bool().unwrap(), folder: s(m, "folder"),
        attachment_names: vec![], login_code: None, trackers_blocked: 0,
        reply_to_name: None,
        cc: vec![], bcc: vec![], is_outgoing: false,
    }
}

/// The raw header block the sync lane would have in hand for this message.
pub fn raw_headers(m: &Value) -> Vec<u8> {
    let mut h = format!("From: {}\r\nSubject: {}\r\n", s(m, "from"), s(m, "subject"));
    if let Some(l) = m["list_id"].as_str() {
        h.push_str(&format!("List-Id: {l}\r\n"));
    }
    if m["list_unsubscribe"].as_bool().unwrap_or(false) {
        h.push_str("List-Unsubscribe: <mailto:unsubscribe@list-example.com>,\r\n <https://list-example.com/u/1>\r\n");
    }
    h.push_str("\r\nbody\r\n");
    h.into_bytes()
}

pub fn incoming(m: &Value) -> Incoming {
    let h = header_facts(&raw_headers(m));
    Incoming {
        new: NewMessage {
            message: Message { cc: strs(m, "cc"), ..message_of(m) }, remote_id: None, body_text: Some(s(m, "body")), body_html: None,
            has_list_header: false, size_bytes: None, tab_id: None,
        },
        headers: h,
        gmail_labels: strs(m, "gm_labels"),
    }
}

pub fn seed_accounts_and_vips(store: &Store, fx: &Value) {
    let accounts = fx["accounts"].as_array().unwrap().clone();
    let vips = fx["vips"].clone();
    store
        .write(move |ctx| {
            for (i, a) in accounts.iter().enumerate() {
                ctx.tx.execute(
                    "INSERT INTO account (id, address, provider, display_name, colour, sort_order, credential_kind, keychain_ref, updated_at_ms) \
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, 'manual', 'svc' || char(31) || ?1, 0)",
                    rusqlite::params![s(a, "id"), s(a, "address"), s(a, "provider"), s(a, "display_name"), s(a, "colour"), i as i64],
                )?;
            }
            for v in vips["addresses"].as_array().unwrap() {
                ctx.tx.execute("INSERT INTO vip (kind, value, updated_at_ms) VALUES ('address', ?1, 0)", [v.as_str().unwrap()])?;
            }
            for v in vips["domains"].as_array().unwrap() {
                ctx.tx.execute("INSERT INTO vip (kind, value, updated_at_ms) VALUES ('domain', ?1, 0)", [v.as_str().unwrap()])?;
            }
            Ok(())
        })
        .unwrap();
}

/// A store with the whole fixture arrived oldest first, one message per batch (so each placement sees the history before it),
/// through the real arrival path. Snoozed messages are snoozed afterwards.
pub fn seeded(db: &TempDb, fx: &Value, model: &dyn AskModel) -> Store {
    seeded_after(db, fx, model, |_| {})
}

/// Like `seeded`, with `before_mail` run on the empty store (accounts and VIPs in, no mail yet).
pub fn seeded_after(db: &TempDb, fx: &Value, model: &dyn AskModel, before_mail: impl FnOnce(&Store)) -> Store {
    let store = Store::open(&db.0).unwrap();
    seed_accounts_and_vips(&store, fx);
    before_mail(&store);
    let mut msgs: Vec<&Value> = fx["messages"].as_array().unwrap().iter().collect();
    msgs.sort_by_key(|m| (m["at_ms"].as_i64().unwrap(), s(m, "id")));
    let uc = PlaceNewMessages { store: &store, model };
    for m in &msgs {
        uc.execute(vec![incoming(m)], m["at_ms"].as_i64().unwrap()).unwrap();
    }
    let labels = fx["labels"].as_array().cloned().unwrap_or_default();
    let assigned: Vec<(String, String)> = msgs.iter().flat_map(|m| strs(m, "labels").into_iter().map(move |l| (s(m, "id"), l))).collect();
    store
        .write(move |ctx| {
            for l in &labels {
                ctx.tx.execute("INSERT INTO label (id, name, parent_id, updated_at_ms) VALUES (?1, ?2, ?3, 0)", rusqlite::params![s(l, "id"), s(l, "name"), l["parent"].as_str()])?;
            }
            for (m, l) in &assigned {
                ctx.tx.execute("INSERT INTO label_assignment (label_id, message_id, updated_at_ms) VALUES (?1, ?2, 0)", [l, m])?;
            }
            Ok(())
        })
        .unwrap();
    let snoozed: Vec<String> = msgs.iter().filter(|m| m["snoozed"].as_bool().unwrap()).map(|m| s(m, "id")).collect();
    store.snooze(snoozed, fx["now_ms"].as_i64().unwrap() + 86_400_000, fx["now_ms"].as_i64().unwrap()).unwrap();
    store
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Row {
    pub tab: String,
    pub decided_by: String,
    pub rule_id: Option<String>,
    pub probability: Option<String>,
}

/// message_category as it stands: message id -> row.
pub fn categories(store: &Store) -> BTreeMap<String, Row> {
    let conn = smart_inbox::load::open_reader(store.path()).unwrap();
    let mut st = conn.prepare("SELECT message_id, tab_id, decided_by, rule_id, probability FROM message_category ORDER BY message_id").unwrap();
    st.query_map([], |r| Ok((r.get::<_, String>(0)?, Row { tab: r.get(1)?, decided_by: r.get(2)?, rule_id: r.get(3)?, probability: r.get::<_, Option<f64>>(4)?.map(|p| format!("{p:.3}")) })))
        .unwrap()
        .map(|r| r.unwrap())
        .collect()
}

/// (labelled inbox message id, expected tab id) for the messages the fixture labels.
pub fn labelled(fx: &Value) -> Vec<(String, String)> {
    fx["messages"].as_array().unwrap().iter().filter(|m| s(m, "folder") == "INBOX" && s(m, "expected_tab") != "-").filter(|m| !s(m, "thread").starts_with("tc-")).map(|m| (s(m, "id"), s(m, "expected_tab"))).collect()
}
