//! Fakes for the five ports and a rig that drives the server the way an outside app does: JSON lines.
#![allow(dead_code)]
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use contracts::{Clock, McpGrant};
use mcp::{Approver, ApprovalRequest, Connection, Decision, DraftWriter, GrantList, GrantLookup, GrantSave, MailSearch, McpError, McpServer, MemoryGrants, NewDraft, Ports, SearchHit, SearchQuery};
use serde_json::{json, Value};

pub type Log = Arc<Mutex<Vec<String>>>;

pub struct FixedClock(pub i64);
impl Clock for FixedClock {
    fn now_ms(&self) -> i64 {
        self.0
    }
    fn utc_offset_s(&self) -> i32 {
        0
    }
}

/// Answers every prompt with `decision`, and keeps what it was asked. `before_answer` runs inside the prompt.
pub struct ScriptedApprover {
    pub decision: Mutex<Decision>,
    pub asked: Mutex<Vec<ApprovalRequest>>,
    pub log: Log,
    pub before_answer: Mutex<Option<Box<dyn Fn() + Send>>>,
}
impl Approver for ScriptedApprover {
    fn execute(&self, request: &ApprovalRequest) -> Decision {
        self.log.lock().unwrap().push("approve".into());
        self.asked.lock().unwrap().push(request.clone());
        if let Some(hook) = self.before_answer.lock().unwrap().as_ref() {
            hook();
        }
        *self.decision.lock().unwrap()
    }
}

pub struct FakeSearch {
    pub hits: Mutex<Vec<SearchHit>>,
    pub queries: Mutex<Vec<SearchQuery>>,
    pub log: Log,
}
impl MailSearch for FakeSearch {
    fn execute(&self, query: &SearchQuery) -> Result<Vec<SearchHit>, McpError> {
        self.log.lock().unwrap().push("search".into());
        self.queries.lock().unwrap().push(query.clone());
        Ok(self.hits.lock().unwrap().clone())
    }
}

pub struct FakeDrafts {
    pub saved: Mutex<Vec<NewDraft>>,
    pub log: Log,
}
impl DraftWriter for FakeDrafts {
    fn execute(&self, draft: &NewDraft) -> Result<String, McpError> {
        self.log.lock().unwrap().push("draft".into());
        self.saved.lock().unwrap().push(draft.clone());
        Ok("draft-1".to_string())
    }
}

/// A grant store that is healthy until `break_it` is called, then fails every read and write: the gate must stay closed.
#[derive(Default)]
pub struct FlakyGrants {
    pub inner: MemoryGrants,
    pub broken: AtomicBool,
}
impl FlakyGrants {
    pub fn break_it(&self) {
        self.broken.store(true, Ordering::SeqCst);
    }
    fn check(&self) -> Result<(), McpError> {
        if self.broken.load(Ordering::SeqCst) {
            return Err(McpError::Backend { reason_words: "the grant table cannot be read".into() });
        }
        Ok(())
    }
}
impl GrantLookup for FlakyGrants {
    fn execute(&self, app_name: &str) -> Result<Option<McpGrant>, McpError> {
        self.check()?;
        GrantLookup::execute(&self.inner, app_name)
    }
}
impl GrantSave for FlakyGrants {
    fn execute(&self, grant: &McpGrant) -> Result<(), McpError> {
        self.check()?;
        GrantSave::execute(&self.inner, grant)
    }
}
impl GrantList for FlakyGrants {
    fn execute(&self) -> Result<Vec<McpGrant>, McpError> {
        self.check()?;
        GrantList::execute(&self.inner)
    }
}

pub struct Rig {
    pub server: Arc<McpServer>,
    pub grants: Arc<MemoryGrants>,
    pub approver: Arc<ScriptedApprover>,
    pub search: Arc<FakeSearch>,
    pub drafts: Arc<FakeDrafts>,
    pub log: Log,
}

pub fn hit(n: usize) -> SearchHit {
    SearchHit {
        message_id: format!("msg-{n}"),
        thread_id: format!("thr-{n}"),
        account_name: "Provanta RFP".into(),
        sender_name: "Maya Peeters".into(),
        from_address: "maya@example.com".into(),
        subject: format!("Contract renewal {n}"),
        snippet: "Please find the renewed contract attached.".into(),
        received_at_ms: 1_700_000_000_000 + n as i64,
    }
}

impl Rig {
    /// A running server whose owner answers `decision` to every prompt.
    pub fn new(decision: Decision) -> Rig {
        let log: Log = Arc::default();
        let grants = Arc::new(MemoryGrants::new());
        let approver = Arc::new(ScriptedApprover { decision: Mutex::new(decision), asked: Mutex::default(), log: log.clone(), before_answer: Mutex::new(None) });
        let search = Arc::new(FakeSearch { hits: Mutex::new(vec![hit(1), hit(2)]), queries: Mutex::default(), log: log.clone() });
        let drafts = Arc::new(FakeDrafts { saved: Mutex::default(), log: log.clone() });
        let server = McpServer::new(Ports { grants: grants.clone(), approver: approver.clone(), search: search.clone(), drafts: drafts.clone(), clock: Arc::new(FixedClock(42)) });
        server.start();
        Rig { server, grants, approver, search, drafts, log }
    }

    /// A connection that has said hello as `app` (declared name, no proven peer).
    pub fn connect(&self, app: &str) -> Connection {
        let mut c = self.server.connect(None);
        let reply = send(&mut c, 1, "initialize", json!({ "protocolVersion": "2025-06-18", "clientInfo": { "name": app, "version": "1" }, "capabilities": {} }));
        assert!(reply.get("result").is_some(), "initialize failed: {reply}");
        c
    }

    pub fn events(&self) -> Vec<String> {
        self.log.lock().unwrap().clone()
    }
}

pub fn send(c: &mut Connection, id: i64, method: &str, params: Value) -> Value {
    let line = json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params }).to_string();
    let reply = c.handle_line(&line).expect("a request is answered");
    serde_json::from_str(&reply).expect("the reply is JSON")
}

pub fn call(c: &mut Connection, tool: &str, arguments: Value) -> Value {
    send(c, 2, "tools/call", json!({ "name": tool, "arguments": arguments }))
}

pub fn error_code(reply: &Value) -> i64 {
    reply["error"]["code"].as_i64().unwrap_or_else(|| panic!("expected an error, got {reply}"))
}

/// The tool result text, parsed.
pub fn result_json(reply: &Value) -> Value {
    let text = reply["result"]["content"][0]["text"].as_str().unwrap_or_else(|| panic!("expected a result, got {reply}"));
    serde_json::from_str(text).unwrap()
}
