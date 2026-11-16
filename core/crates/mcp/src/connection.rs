//! One client connection: a line of JSON-RPC 2.0 in, at most one line out. This is the thin controller; the
//! rules live in the use cases. A connection starts unnamed and becomes a named client once, at `initialize`.
use std::sync::Arc;

use serde_json::{json, Map, Value};

use crate::codes;
use crate::domain::McpError;
use crate::server::McpServer;
use crate::tools;

/// Protocol revisions this server speaks, oldest first. The client's own is echoed when listed here.
const SUPPORTED_VERSIONS: [&str; 3] = ["2024-11-05", "2025-03-26", "2025-06-18"];

struct Failure {
    code: i64,
    message: String,
}

impl Failure {
    fn new(code: i64, message: impl Into<String>) -> Failure {
        Failure { code, message: message.into() }
    }
}

impl From<McpError> for Failure {
    fn from(e: McpError) -> Failure {
        let code = match &e {
            McpError::ServerOff => codes::SERVER_OFF,
            McpError::NotAuthorised { .. } => codes::NOT_AUTHORISED,
            McpError::Refused { .. } => codes::REFUSED,
            McpError::InvalidParams { .. } | McpError::UnknownTool { .. } => codes::INVALID_PARAMS,
            McpError::Backend { .. } => codes::INTERNAL_ERROR,
        };
        Failure::new(code, e.to_string())
    }
}

pub struct Connection {
    server: Arc<McpServer>,
    /// The name the grant list knows this app by. A peer name the host proved (its code signature) wins over
    /// the name the app declares in `initialize`.
    peer: Option<String>,
    client: Option<String>,
}

impl Connection {
    pub(crate) fn new(server: Arc<McpServer>, peer: Option<String>) -> Connection {
        Connection { server, peer, client: None }
    }

    /// The app this connection acts as, once it has introduced itself.
    pub fn client(&self) -> Option<&str> {
        self.client.as_deref()
    }

    /// Handles one line. None for a notification (no reply is ever sent to one).
    pub fn handle_line(&mut self, line: &str) -> Option<String> {
        let parsed: Value = match serde_json::from_str(line.trim()) {
            Ok(v) => v,
            Err(_) => return Some(error_line(&Value::Null, &Failure::new(codes::PARSE_ERROR, "the message is not valid JSON"))),
        };
        let Some(object) = parsed.as_object() else {
            let why = if parsed.is_array() { "batches are not supported; send one message per line" } else { "the message must be a JSON object" };
            return Some(error_line(&Value::Null, &Failure::new(codes::INVALID_REQUEST, why)));
        };
        let id = object.get("id").cloned();
        let Some(method) = object.get("method").and_then(Value::as_str) else {
            // A response or a malformed message: only a message with an id is answered.
            return id.map(|id| error_line(&id, &Failure::new(codes::INVALID_REQUEST, "the message has no method")));
        };
        if !self.server.is_running() {
            return id.map(|id| error_line(&id, &McpError::ServerOff.into()));
        }
        let params = object.get("params").cloned().unwrap_or(Value::Null);
        let outcome = self.dispatch(method, &params);
        match (id, outcome) {
            (None, _) => None,
            (Some(id), Ok(result)) => Some(json!({ "jsonrpc": "2.0", "id": id, "result": result }).to_string()),
            (Some(id), Err(failure)) => Some(error_line(&id, &failure)),
        }
    }

    fn dispatch(&mut self, method: &str, params: &Value) -> Result<Value, Failure> {
        match method {
            "initialize" => self.initialize(params),
            "ping" => Ok(json!({})),
            "tools/list" => Ok(tools::list()),
            "tools/call" => self.call_tool(params),
            m if m.starts_with("notifications/") => Ok(Value::Null),
            other => Err(Failure::new(codes::METHOD_NOT_FOUND, format!("no method named {other}"))),
        }
    }

    fn initialize(&mut self, params: &Value) -> Result<Value, Failure> {
        if self.client.is_some() {
            return Err(Failure::new(codes::INVALID_REQUEST, "this connection is already initialised"));
        }
        let declared = params.pointer("/clientInfo/name").and_then(Value::as_str).map(str::trim).filter(|n| !n.is_empty());
        let Some(name) = self.peer.clone().or_else(|| declared.map(str::to_string)) else {
            return Err(Failure::new(codes::INVALID_PARAMS, "clientInfo.name is required: the owner allows apps by name"));
        };
        self.server.introduce(&name)?;
        self.client = Some(name);
        let asked = params.get("protocolVersion").and_then(Value::as_str);
        let version = asked.filter(|v| SUPPORTED_VERSIONS.contains(v)).unwrap_or(SUPPORTED_VERSIONS[SUPPORTED_VERSIONS.len() - 1]);
        Ok(json!({
            "protocolVersion": version,
            "capabilities": { "tools": { "listChanged": false } },
            "serverInfo": { "name": "whoosh", "version": env!("CARGO_PKG_VERSION") },
            "instructions": "Whoosh searches and drafts mail for the owner. Every request is approved by the owner in Whoosh. Drafts are never sent."
        }))
    }

    fn call_tool(&mut self, params: &Value) -> Result<Value, Failure> {
        let Some(name) = params.get("name").and_then(Value::as_str) else {
            return Err(Failure::new(codes::INVALID_PARAMS, "name: the tool to call is required"));
        };
        let empty = Value::Object(Map::new());
        let arguments = match params.get("arguments") {
            None | Some(Value::Null) => &empty,
            Some(v) => v,
        };
        let client = self.client.as_deref();
        let result = match name {
            tools::SEARCH_MAIL => self.server.search_mail(client, arguments)?,
            tools::CREATE_DRAFT => self.server.create_draft(client, arguments)?,
            other => return Err(McpError::UnknownTool { name: other.to_string() }.into()),
        };
        Ok(json!({ "content": [{ "type": "text", "text": result.to_string() }], "isError": false }))
    }
}

fn error_line(id: &Value, failure: &Failure) -> String {
    json!({ "jsonrpc": "2.0", "id": id, "error": { "code": failure.code, "message": failure.message } }).to_string()
}
