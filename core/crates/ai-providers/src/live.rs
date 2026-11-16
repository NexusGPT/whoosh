//! The real HTTPS transport (reqwest, blocking, rustls). Behind the `live` feature; the core does not enable it
//! until the owner allows a first call to a provider. The caller passes the timeout from the ledger.
use std::time::Duration;

use crate::transport::{Body, Method, Request, Response, Transport};

pub struct ReqwestTransport {
    client: reqwest::blocking::Client,
}

impl ReqwestTransport {
    pub fn new(timeout: Duration) -> Result<Self, String> {
        let mut builder = reqwest::blocking::Client::builder().timeout(timeout).redirect(guarded_redirects());
        if netguard::test_resolver().is_some() { builder = builder.dns_resolver(std::sync::Arc::new(TestLookup)); }
        let client = builder.build().map_err(|e| e.to_string())?;
        Ok(ReqwestTransport { client })
    }
}

impl Transport for ReqwestTransport {
    fn send(&self, request: &Request) -> Result<Response, String> {
        netguard::outbound_allowed_url(&request.url).map_err(|e| e.to_string())?;   // rig-net-rust: a rig copy reaches this Mac only
        let builder = match request.method {
            Method::Get => self.client.get(&request.url),
            Method::Post => self.client.post(&request.url),
        };
        let mut builder = request.headers.iter().fold(builder, |b, (k, v)| b.header(k.as_str(), v.as_str()));
        builder = match &request.body {
            Body::None => builder,
            Body::Json(text) => builder.body(text.clone()),
            Body::Bytes { content_type, data } => builder.header("Content-Type", content_type.as_str()).body(data.clone()),
        };
        let response = builder.send().map_err(|e| e.to_string())?;
        let status = response.status().as_u16();
        let body = response.text().map_err(|e| e.to_string())?;
        Ok(Response { status, body })
    }
}

/// rig-net-rust (audit r77, gap 6): a redirect is a new host, asked of the rig guard before it is followed; past that, reqwest's own
/// default limit of redirects is kept.
fn guarded_redirects() -> reqwest::redirect::Policy {
    reqwest::redirect::Policy::custom(|a| match netguard::outbound_allowed_url(a.url().as_str()) {
        Err(e) => a.error(e.to_string()),
        Ok(()) if a.previous().len() >= REDIRECTS_KEPT => a.error("too many redirects"),
        Ok(()) => a.follow(),
    })
}

/// reqwest's own default limit of redirects (`reqwest::redirect::Policy::default`), kept unchanged.
const REDIRECTS_KEPT: usize = 10;

/// Tests only (a resolver set by `netguard::set_resolver`): every name this client looks up goes through netguard's lookup, so a guard
/// removed by a mutant reaches a listener on 127.0.0.1, never the network. Without a test resolver the client keeps reqwest's own.
struct TestLookup;

impl reqwest::dns::Resolve for TestLookup {
    fn resolve(&self, name: reqwest::dns::Name) -> reqwest::dns::Resolving {
        let host = name.as_str().to_string();
        Box::pin(async move {
            let addrs = netguard::lookup(&host, 0).map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)?;
            Ok(Box::new(addrs.into_iter()) as reqwest::dns::Addrs)
        })
    }
}
