//! Fake ports that record every call, and readers for fixtures/autoconfig.
#![allow(dead_code)]
use autoconfig::*;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Mutex;

pub fn fixture(name: &str) -> String {
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../fixtures/autoconfig").join(name);
    std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("read {}: {e}", p.display()))
}

#[derive(Default)]
pub struct FakeHttp {
    pub answers: HashMap<String, Result<HttpResponse, NetError>>,
    pub calls: Mutex<Vec<HttpRequest>>,
}

impl FakeHttp {
    pub fn ok(mut self, url: impl Into<String>, body: impl Into<String>) -> Self {
        self.answers.insert(url.into(), Ok(HttpResponse { status: 200, body: body.into() }));
        self
    }
    pub fn status(mut self, url: impl Into<String>, status: u16, body: impl Into<String>) -> Self {
        self.answers.insert(url.into(), Ok(HttpResponse { status, body: body.into() }));
        self
    }
    pub fn fail(mut self, url: impl Into<String>, e: NetError) -> Self {
        self.answers.insert(url.into(), Err(e));
        self
    }
    pub fn urls(&self) -> Vec<String> { self.calls.lock().expect("lock").iter().map(|r| r.url.clone()).collect() }
    pub fn called(&self, prefix: &str) -> bool { self.urls().iter().any(|u| u.starts_with(prefix)) }
}

impl HttpPort for FakeHttp {
    fn execute(&self, req: &HttpRequest) -> Result<HttpResponse, NetError> {
        self.calls.lock().expect("lock").push(req.clone());
        // unknown URLs: the provider database says 404, any other host does not resolve
        self.answers.get(&req.url).cloned().unwrap_or_else(|| {
            if req.url.starts_with(consts::ISPDB_BASE) { Ok(HttpResponse { status: 404, body: String::new() }) } else { Err(NetError::HostNotFound) }
        })
    }
}

#[derive(Default)]
pub struct FakeDns {
    pub srv: HashMap<String, Vec<SrvRecord>>,
    pub mx: HashMap<String, Vec<MxRecord>>,
    pub missing_domains: Vec<String>,
    pub calls: Mutex<Vec<DnsQuery>>,
}

impl FakeDns {
    /// Reads a fixture of "srv <name> <priority> <weight> <port> <target>" lines.
    pub fn from_fixture(text: &str) -> Self {
        let mut dns = Self::default();
        for line in text.lines().filter(|l| l.starts_with("srv ")) {
            let f: Vec<&str> = line.split_whitespace().collect();
            let rec = SrvRecord { priority: f[2].parse().expect("prio"), weight: f[3].parse().expect("weight"), port: f[4].parse().expect("port"), target: f[5].into() };
            dns.srv.entry(f[1].to_string()).or_default().push(rec);
        }
        dns
    }
    pub fn count(&self) -> usize { self.calls.lock().expect("lock").len() }
    pub fn with_mx(mut self, domain: &str, records: &[(u16, &str)]) -> Self {
        self.mx.insert(domain.into(), records.iter().map(|(p, h)| MxRecord { preference: *p, exchange: (*h).into() }).collect());
        self
    }
}

impl DnsPort for FakeDns {
    fn execute(&self, q: &DnsQuery) -> Result<DnsAnswer, NetError> {
        self.calls.lock().expect("lock").push(q.clone());
        Ok(match q {
            DnsQuery::Exists(d) => DnsAnswer::Exists(!self.missing_domains.contains(d)),
            DnsQuery::Srv(n) => DnsAnswer::Srv(self.srv.get(n).cloned().unwrap_or_default()),
            DnsQuery::Mx(d) => DnsAnswer::Mx(self.mx.get(d).cloned().unwrap_or_default()),
        })
    }
}

#[derive(Default)]
pub struct FakeProbe {
    pub answering: Vec<(String, u16, Security)>,
    pub calls: Mutex<Vec<ProbeRequest>>,
}

impl FakeProbe {
    /// Reads a fixture of "probe <host> <port> <ssl|starttls>" lines.
    pub fn from_fixture(text: &str) -> Self {
        let answering = text.lines().filter(|l| l.starts_with("probe ")).map(|l| {
            let f: Vec<&str> = l.split_whitespace().collect();
            let sec = if f[3] == "ssl" { Security::Ssl } else { Security::Starttls };
            (f[1].to_string(), f[2].parse().expect("port"), sec)
        }).collect();
        Self { answering, ..Self::default() }
    }
    pub fn tried(&self) -> Vec<(String, u16, Security)> {
        self.calls.lock().expect("lock").iter().map(|r| (r.host.clone(), r.port, r.security)).collect()
    }
    pub fn count(&self) -> usize { self.calls.lock().expect("lock").len() }
}

impl ProbePort for FakeProbe {
    fn execute(&self, r: &ProbeRequest) -> Result<ProbeOk, NetError> {
        self.calls.lock().expect("lock").push(r.clone());
        if self.answering.contains(&(r.host.clone(), r.port, r.security)) { Ok(ProbeOk { greeting: "* OK".into() }) } else { Err(NetError::Refused) }
    }
}

pub fn found(r: &DetectionResult) -> &MailConfig {
    match &r.outcome { Outcome::Found(c) => c, other => panic!("expected a found config, got {other:?}") }
}
