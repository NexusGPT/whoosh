//! The server the host holds: on or off, the grant list, and the connections. The Swift pane reads and changes
//! grants here; an outside app reaches the same grants through a `Connection`.
use std::io::{self, BufRead, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use contracts::{Clock, McpGrant};
use serde_json::Value;

use crate::connection::Connection;
use crate::domain::McpError;
use crate::ports::{Approver, DraftWriter, GrantList, GrantSave, GrantStore, MailSearch};
use crate::usecases::create_draft::CreateDraft;
use crate::usecases::gate::Gate;
use crate::usecases::introduce::Introduce;
use crate::usecases::search_mail::SearchMail;

/// Everything the server needs from the rest of the core.
pub struct Ports {
    pub grants: Arc<dyn GrantStore>,
    pub approver: Arc<dyn Approver>,
    pub search: Arc<dyn MailSearch>,
    pub drafts: Arc<dyn DraftWriter>,
    pub clock: Arc<dyn Clock>,
}

pub struct McpServer {
    running: AtomicBool,
    grants: Arc<dyn GrantStore>,
    clock: Arc<dyn Clock>,
    introduce: Introduce,
    search_mail: SearchMail,
    create_draft: CreateDraft,
}

impl McpServer {
    /// A new server, off. The host calls `start` when the owner has the server on.
    pub fn new(ports: Ports) -> Arc<McpServer> {
        let gate = Arc::new(Gate::new(ports.grants.clone(), ports.approver));
        Arc::new(McpServer {
            running: AtomicBool::new(false),
            introduce: Introduce::new(ports.grants.clone(), ports.grants.clone()),
            search_mail: SearchMail::new(gate.clone(), ports.search),
            create_draft: CreateDraft::new(gate, ports.drafts),
            grants: ports.grants,
            clock: ports.clock,
        })
    }

    pub fn start(&self) {
        self.running.store(true, Ordering::SeqCst);
    }

    /// Off at once: every connection's next message is answered "the server is off", and `serve` returns.
    pub fn stop(&self) {
        self.running.store(false, Ordering::SeqCst);
    }

    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::SeqCst)
    }

    /// Every app Whoosh has seen, in first-seen order, with whether it is allowed.
    pub fn grants(&self) -> Result<Vec<McpGrant>, McpError> {
        GrantList::execute(&*self.grants)
    }

    /// The owner allows an app. Takes effect on that app's next call.
    pub fn allow(&self, app_name: &str) -> Result<McpGrant, McpError> {
        let grant = McpGrant { app_name: app_name.to_string(), allowed: true, granted_at_ms: Some(self.clock.now_ms()) };
        GrantSave::execute(&*self.grants, &grant)?;
        Ok(grant)
    }

    /// The owner revokes an app. Its next call, and a call waiting on an approval prompt, is refused.
    pub fn revoke(&self, app_name: &str) -> Result<McpGrant, McpError> {
        let grant = McpGrant { app_name: app_name.to_string(), allowed: false, granted_at_ms: None };
        GrantSave::execute(&*self.grants, &grant)?;
        Ok(grant)
    }

    /// A new connection. `peer` is the app's name when the host can prove it (for example from the code
    /// signature of the process on the other end); None lets the app name itself in `initialize`.
    pub fn connect(self: &Arc<Self>, peer: Option<String>) -> Connection {
        Connection::new(self.clone(), peer)
    }

    /// Serves one stdio connection until the reader ends or the server is stopped. One JSON message per line.
    pub fn serve<R: BufRead, W: Write>(self: &Arc<Self>, peer: Option<String>, mut reader: R, mut writer: W) -> io::Result<()> {
        let mut connection = self.connect(peer);
        let mut raw = Vec::new();
        loop {
            raw.clear();
            if reader.read_until(b'\n', &mut raw)? == 0 {
                return Ok(());
            }
            let line = String::from_utf8_lossy(&raw);
            if line.trim().is_empty() {
                continue;
            }
            // Bytes that are not UTF-8 become U+FFFD and fail to parse: the client gets a parse error line.
            let reply = connection.handle_line(&line);
            if let Some(reply) = reply {
                writeln!(writer, "{reply}")?;
                writer.flush()?;
            }
            if !self.is_running() {
                return Ok(());
            }
        }
    }

    pub(crate) fn introduce(&self, app_name: &str) -> Result<(), McpError> {
        self.introduce.execute(app_name)
    }

    pub(crate) fn search_mail(&self, client: Option<&str>, arguments: &Value) -> Result<Value, McpError> {
        self.search_mail.execute(client, arguments)
    }

    pub(crate) fn create_draft(&self, client: Option<&str>, arguments: &Value) -> Result<Value, McpError> {
        self.create_draft.execute(client, arguments)
    }
}
