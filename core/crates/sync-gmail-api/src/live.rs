//! The real HTTPS transport (reqwest, blocking, rustls). Behind the `live` feature; the core does not enable it
//! until Google's restricted-scope verification has passed and the owner switches `gmail_api_verified` on.
use crate::transport::{Method, Request, Response, Transport};
use std::time::Duration;

const BASE: &str = "https://gmail.googleapis.com/gmail/v1/users/me/";

pub struct ReqwestTransport {
    client: reqwest::blocking::Client,
    /// The address every request path is joined to: `BASE`, except through `with_base`.
    base: String,
}

impl ReqwestTransport {
    pub fn new(timeout: Duration) -> Result<Self, String> {
        Self::with_base(timeout, BASE.to_string())
    }

    /// A test seam (fix-core, item c02dad3b): the same transport against another base address, so a test can point it at a server on 127.0.0.1 that
    /// answers a redirect. `base` ends with `/`. The core always builds its transport with `new`.
    pub fn with_base(timeout: Duration, base: String) -> Result<Self, String> {
        let mut builder = reqwest::blocking::Client::builder().timeout(timeout).redirect(guarded_redirects());
        if netguard::test_resolver().is_some() { builder = builder.dns_resolver(std::sync::Arc::new(TestLookup)); }
        let client = builder.build().map_err(|e| e.to_string())?;
        Ok(ReqwestTransport { client, base })
    }
}

impl Transport for ReqwestTransport {
    fn send(&self, request: &Request) -> Result<Response, String> {
        let url = format!("{}{}{}", self.base, request.path, query_string(&request.query));
        netguard::outbound_allowed_url(&url).map_err(|e| e.to_string())?;   // rig-net-rust: a rig copy reaches this Mac only
        let builder = match request.method {
            Method::Get => self.client.get(&url),
            Method::Post => self.client.post(&url),
        };
        let mut builder = builder.bearer_auth(&request.bearer);
        if let Some(body) = &request.json_body {
            builder = builder.header("Content-Type", "application/json").body(body.clone());
        }
        let response = builder.send().map_err(|e| e.to_string())?;
        let status = response.status().as_u16();
        let retry_after_ms = response
            .headers()
            .get("retry-after")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.trim().parse::<i64>().ok())
            .map(|s| s * 1000);
        let body = response.text().map_err(|e| e.to_string())?;
        Ok(Response { status, body, retry_after_ms })
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

/// `?k=v&k=v` with RFC 3986 unreserved characters left alone and everything else percent-encoded.
fn query_string(pairs: &[(String, String)]) -> String {
    fn enc(s: &str) -> String {
        s.bytes()
            .map(|b| match b {
                b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => (b as char).to_string(),
                _ => format!("%{b:02X}"),
            })
            .collect()
    }
    if pairs.is_empty() {
        return String::new();
    }
    let joined: Vec<String> = pairs.iter().map(|(k, v)| format!("{}={}", enc(k), enc(v))).collect();
    format!("?{}", joined.join("&"))
}

#[cfg(test)]
mod tests {
    use super::query_string;

    #[test]
    fn query_values_are_encoded_and_repeated_keys_kept() {
        let q = query_string(&[("historyTypes".into(), "messageAdded".into()), ("historyTypes".into(), "label Added".into()), ("pageToken".into(), "a/b+c=".into())]);
        assert_eq!(q, "?historyTypes=messageAdded&historyTypes=label%20Added&pageToken=a%2Fb%2Bc%3D");
        assert_eq!(query_string(&[]), "");
    }
}
