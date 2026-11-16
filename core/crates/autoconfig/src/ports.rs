//! The three ports. One method each, named `execute`. Fakes stand in for them in tests; the real
//! implementations live in `adapters`.

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NetError {
    /// No route out of this Mac.
    Offline,
    /// The name does not resolve.
    HostNotFound,
    Timeout,
    Refused,
    Tls(String),
    /// Anything else, with the real message.
    Other(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Method { Get, Post }

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HttpRequest {
    pub method: Method,
    pub url: String,
    pub user_agent: String,
    /// A POST body (Autodiscover's XML request), sent as text/xml.
    pub body: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HttpResponse {
    pub status: u16,
    pub body: String,
}

pub trait HttpPort {
    fn execute(&self, request: &HttpRequest) -> Result<HttpResponse, NetError>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DnsQuery {
    /// An SRV lookup of a full name, e.g. `_imaps._tcp.example.org`.
    Srv(String),
    /// Does the domain exist at all (any record)?
    Exists(String),
    /// The domain's mail record (MX).
    Mx(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SrvRecord {
    pub priority: u16,
    pub weight: u16,
    pub port: u16,
    pub target: String,
}

/// One mail exchanger: a lower preference is tried first.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MxRecord {
    pub preference: u16,
    pub exchange: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DnsAnswer {
    Srv(Vec<SrvRecord>),
    Exists(bool),
    Mx(Vec<MxRecord>),
}

pub trait DnsPort {
    fn execute(&self, query: &DnsQuery) -> Result<DnsAnswer, NetError>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Protocol { Imap, Smtp }

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProbeRequest {
    pub host: String,
    pub port: u16,
    pub protocol: Protocol,
    pub security: crate::domain::config::Security,
}

/// The server greeted over TLS (directly, or after STARTTLS).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProbeOk {
    pub greeting: String,
}

pub trait ProbePort {
    fn execute(&self, request: &ProbeRequest) -> Result<ProbeOk, NetError>;
}

impl<T: HttpPort + ?Sized> HttpPort for &T {
    fn execute(&self, request: &HttpRequest) -> Result<HttpResponse, NetError> { (**self).execute(request) }
}
impl<T: DnsPort + ?Sized> DnsPort for &T {
    fn execute(&self, query: &DnsQuery) -> Result<DnsAnswer, NetError> { (**self).execute(query) }
}
impl<T: ProbePort + ?Sized> ProbePort for &T {
    fn execute(&self, request: &ProbeRequest) -> Result<ProbeOk, NetError> { (**self).execute(request) }
}
