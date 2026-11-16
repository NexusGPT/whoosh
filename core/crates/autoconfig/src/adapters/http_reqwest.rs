//! The real HttpPort over reqwest (blocking, rustls). HTTPS only, redirects followed only to HTTPS,
//! AUTOCONFIG_HTTP_TIMEOUT_MS per request. Behind the `live` feature.
use std::time::Duration;

use crate::adapters::body_cap;
use crate::consts::{AUTOCONFIG_BODY_MAX_BYTES, AUTOCONFIG_HTTP_TIMEOUT_MS};
use crate::ports::{HttpPort, HttpRequest, HttpResponse, Method, NetError};

const MAX_REDIRECTS: usize = 5;

pub struct ReqwestHttp {
    client: reqwest::blocking::Client,
    /// Set only by `new_plain_loopback`: plain http, and only to 127.0.0.1 (a test's local fake server).
    loopback_only: bool,
}

impl ReqwestHttp {
    pub fn new() -> Result<Self, String> {
        // ring is the TLS provider (Cargo.toml: rustls-no-provider); an Err means one is installed already, which is fine.
        let _ = rustls::crypto::ring::default_provider().install_default();
        let policy = reqwest::redirect::Policy::custom(|a| {
            // engine-act-r6 round 5: a redirect is a new host; a rig copy follows none past this Mac
            let away = netguard::outbound_allowed(a.url().host_str().unwrap_or("")).is_err();
            if away || a.url().scheme() != "https" || a.previous().len() >= MAX_REDIRECTS { a.stop() } else { a.follow() }
        });
        let client = reqwest::blocking::Client::builder()
            .https_only(true)
            .redirect(policy)
            .timeout(Duration::from_millis(AUTOCONFIG_HTTP_TIMEOUT_MS))
            .build()
            .map_err(|e| format!("{e:?}"))?;
        Ok(Self { client, loopback_only: false })
    }

    /// For a test against a local fake server: plain http, and every URL but one on 127.0.0.1 is refused before anything is sent.
    /// The body limit, the redirect policy and the timeout are the real client's.
    #[doc(hidden)]
    pub fn new_plain_loopback() -> Result<Self, String> {
        let _ = rustls::crypto::ring::default_provider().install_default();   // the same provider as `new` (the client builder needs one even for plain http)
        let client = reqwest::blocking::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .timeout(Duration::from_millis(AUTOCONFIG_HTTP_TIMEOUT_MS))
            .build()
            .map_err(|e| format!("{e:?}"))?;
        Ok(Self { client, loopback_only: true })
    }
}

fn net_error(e: reqwest::Error) -> NetError {
    let chain = format!("{e:?}");
    if e.is_timeout() {
        NetError::Timeout
    } else if e.is_connect() && (chain.contains("dns error") || chain.contains("failed to lookup")) {
        NetError::HostNotFound
    } else if e.is_connect() {
        NetError::Refused
    } else {
        NetError::Other(chain)
    }
}

impl HttpPort for ReqwestHttp {
    fn execute(&self, req: &HttpRequest) -> Result<HttpResponse, NetError> {
        if self.loopback_only && !req.url.starts_with("http://127.0.0.1:") { return Err(NetError::Refused); }
        netguard::outbound_allowed_url(&req.url).map_err(|_| NetError::Refused)?;   // engine-act-r6 round 5: before the name is resolved
        let mut rb = match req.method {
            Method::Get => self.client.get(&req.url),
            Method::Post => self.client.post(&req.url),
        };
        rb = rb.header(reqwest::header::USER_AGENT, req.user_agent.as_str());
        if let Some(body) = &req.body {
            rb = rb.header(reqwest::header::CONTENT_TYPE, "text/xml; charset=utf-8").body(body.clone());
        }
        let resp = rb.send().map_err(net_error)?;
        let status = resp.status().as_u16();
        // The body is read up to AUTOCONFIG_BODY_MAX_BYTES: a length the server names above it is refused unread, and a body that
        // names none (chunked) is cut off one byte past the limit.
        if resp.content_length().is_some_and(|n| n > AUTOCONFIG_BODY_MAX_BYTES) {
            return Err(NetError::Other(body_cap::too_large_words(AUTOCONFIG_BODY_MAX_BYTES)));
        }
        let body = body_cap::read_body(resp)?;
        Ok(HttpResponse { status, body })
    }
}
