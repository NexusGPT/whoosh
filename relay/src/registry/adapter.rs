//! The seam a provider-family adapter plugs into. The four adapter lanes (gmail, graph, jmap,
//! macsource) each implement `ProviderAdapter` in relay/src/adapters/<family>/mod.rs, expose
//! `pub fn adapter() -> Arc<dyn ProviderAdapter>`, and switch on their Cargo feature.
//!
//! An adapter turns one inbound provider request (a Pub/Sub push, a Graph notification, a JMAP
//! WebPush, a Mac-signed event) into zero or more `MailSignal`s. It never sees APNs and never
//! stores a mailbox credential: the relay holds device tokens and routing ids only.
use super::Dispatcher;
use serde::Deserialize;
use std::sync::Arc;

/// "Something new for this account". `signal_id` is the dedup key (a history id, a notification
/// id); `opaque_ref` is carried to the phone untouched.
#[derive(Debug, Clone, Deserialize)]
pub struct MailSignal {
    pub signal_id: String,
    pub routing_id: String,
    #[serde(default)]
    pub opaque_ref: Option<String>,
}

pub struct InboundRequest {
    /// Path and query exactly as received, for example `/v1/inbound/graph?validationToken=abc`.
    /// A provider that proves a URL with a query token, or an adapter that keeps a per-
    /// subscription secret in the push URL, reads it here.
    #[allow(dead_code)] // read by adapters (a build with no adapter feature has none)
    pub uri: String,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

impl InboundRequest {
    /// The query string without the leading `?`; empty when the URI has none.
    #[allow(dead_code)]
    pub fn query(&self) -> &str {
        self.uri.split_once('?').map(|(_, q)| q).unwrap_or("")
    }
}

/// An answer the route sends back instead of decoding (for example Graph's validation handshake).
/// It queues nothing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Reply {
    pub status: u16,
    pub content_type: &'static str,
    pub body: String,
}

impl Reply {
    #[allow(dead_code)]
    pub fn text(body: String) -> Reply {
        Reply { status: 200, content_type: "text/plain; charset=utf-8", body }
    }
}

#[derive(Debug)]
pub enum AdapterError {
    /// The provider's own proof (signature, JWT, shared secret) failed. Maps to 401.
    Unauthorized(String),
    /// The payload is not what this provider sends. Maps to 400.
    Malformed(String),
}

pub trait ProviderAdapter: Send + Sync {
    /// URL-safe family name; the route is POST /v1/inbound/<family>.
    fn family(&self) -> &'static str;
    /// Asked first, before `decode`. `Some` is sent back as the HTTP answer and nothing is
    /// decoded or queued. Must not need a binding or an upstream: it runs before any proof.
    fn reply(&self, _request: &InboundRequest) -> Option<Reply> {
        None
    }
    fn decode(&self, request: &InboundRequest) -> Result<Vec<MailSignal>, AdapterError>;
    /// Called once at boot (watch renewal, subscription renewal). Must return quickly and must
    /// not require an upstream answer: the relay boots without any provider reachable. The
    /// dispatcher lets a renewal or polling job emit signals on its own schedule.
    fn start(&self, _dispatcher: Arc<Dispatcher>) {}
}

pub struct AdapterSet {
    adapters: Vec<Arc<dyn ProviderAdapter>>,
}

impl AdapterSet {
    pub fn new(adapters: Vec<Arc<dyn ProviderAdapter>>) -> AdapterSet {
        AdapterSet { adapters }
    }

    /// The adapters compiled in. Each lane switches its own feature on for its own tests.
    /// `loopback` is always built in when a signal token is configured.
    pub fn builtin(signal_token: Option<String>) -> AdapterSet {
        let loopback: Option<Arc<dyn ProviderAdapter>> = signal_token.map(|t| Arc::new(super::loopback::Loopback::new(t)) as Arc<dyn ProviderAdapter>);
        AdapterSet::new(loopback.into_iter().chain([
            #[cfg(feature = "adapter-gmail")]
            crate::adapter_gmail::adapter(),
            #[cfg(feature = "adapter-graph")]
            crate::adapter_graph::adapter(),
            #[cfg(feature = "adapter-jmap")]
            crate::adapter_jmap::adapter(),
            #[cfg(feature = "adapter-macsource")]
            crate::adapter_macsource::adapter(),
        ]).collect())
    }

    pub fn get(&self, family: &str) -> Option<&Arc<dyn ProviderAdapter>> {
        self.adapters.iter().find(|a| a.family() == family)
    }

    pub fn families(&self) -> Vec<&'static str> {
        self.adapters.iter().map(|a| a.family()).collect()
    }

    pub fn start_all(&self, dispatcher: &Arc<Dispatcher>) {
        for adapter in &self.adapters {
            adapter.start(dispatcher.clone());
        }
    }
}
