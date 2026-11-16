//! One mailbox's HTTP engine: bearer token, the per-mailbox throttle around every call, 429 and
//! 5xx backoff, and the failure words the owner reads. The real body of a Graph error (its code and
//! message) is kept in the words, never only the status.
use std::sync::Arc;

use contracts::{ActionFailed, CoreError};
use reqwest::{Method, Url};
use serde_json::Value;

use crate::{
    model::invalid,
    throttle::Throttle,
    time::BoxFut,
};

/// Where the access token comes from (the secrets lane). The client never stores a token.
pub trait TokenSource: Send + Sync {
    fn access_token(&self) -> BoxFut<'_, Result<String, CoreError>>;
}

/// Told when Microsoft slows this mailbox down, so the owner can read it before the retry lands.
pub type SlowedSink = Arc<dyn Fn(ActionFailed) + Send + Sync>;

/// Everything the caller decides. No `Default`: the retry and timeout numbers are not in
/// docs/CONSTANTS.md yet (lane notes "## Constants to add"), so the sync engine passes them.
#[derive(Debug, Clone)]
pub struct GraphConfig {
    /// "https://graph.microsoft.com/v1.0"; a test points it at 127.0.0.1.
    pub base_url: String,
    /// "me" or "users/<id>".
    pub mailbox_path: String,
    pub account_id: String,
    /// The name the owner reads in every failure.
    pub account_name: String,
    pub max_attempts: u32,
    pub backoff_base_ms: u64,
    /// Also the longest Retry-After this client honours.
    pub backoff_max_ms: u64,
    pub timeout_ms: u64,
    pub page_size: u32,
}

pub(crate) struct Reply {
    pub status: u16,
    pub body: String,
}

enum Attempt {
    Done(Reply),
    Retry { failure: ActionFailed, wait_ms: u64, slowed: bool },
}

pub struct GraphMailbox {
    pub(crate) cfg: GraphConfig,
    http: reqwest::Client,
    tokens: Arc<dyn TokenSource>,
    throttle: Arc<Throttle>,
    slowed: Option<SlowedSink>,
}

pub fn slowed_words(wait_ms: u64) -> String {
    format!("slowed by Microsoft, trying again in {} s", wait_ms.div_ceil(1000))
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

fn graph_error(body: &str) -> (String, String) {
    let v: Value = serde_json::from_str(body).unwrap_or(Value::Null);
    let pick = |k: &str| v["error"][k].as_str().unwrap_or_default().chars().take(200).collect::<String>();
    (pick("code"), pick("message"))
}

impl GraphMailbox {
    pub fn new(cfg: GraphConfig, tokens: Arc<dyn TokenSource>, throttle: Arc<Throttle>, slowed: Option<SlowedSink>) -> Result<Self, CoreError> {
        if cfg.max_attempts == 0 {
            return Err(invalid("max_attempts", "must be at least 1"));
        }
        netguard::outbound_allowed_url(&cfg.base_url)?;   // rig-net-rust: a rig copy builds no mailbox for a host other than this Mac
        let mut builder = reqwest::Client::builder().timeout(std::time::Duration::from_millis(cfg.timeout_ms)).redirect(guarded_redirects());
        if netguard::test_resolver().is_some() {
            builder = builder.dns_resolver(Arc::new(TestLookup));
        }
        let http = builder
            .build()
            .map_err(|e| CoreError::Unexpected { reason_words: format!("cannot build the HTTP client: {e}") })?;
        Ok(GraphMailbox { cfg, http, tokens, throttle, slowed })
    }

    pub fn throttle(&self) -> &Arc<Throttle> {
        &self.throttle
    }

    fn failure(&self, words: String, retryable: bool) -> ActionFailed {
        ActionFailed { account_name: self.cfg.account_name.clone(), reason_words: words, retryable, code: None }
    }

    fn backoff(&self, attempt: u32) -> u64 {
        self.cfg.backoff_base_ms.saturating_mul(1u64 << (attempt - 1).min(20)).min(self.cfg.backoff_max_ms)
    }

    /// `base_url/<mailbox or segments...>`, every segment percent-encoded.
    pub(crate) fn url(&self, segments: &[&str]) -> Result<Url, CoreError> {
        let mut u = Url::parse(&self.cfg.base_url).map_err(|e| invalid("base_url", e.to_string()))?;
        u.path_segments_mut().map_err(|_| invalid("base_url", "cannot carry a path"))?.pop_if_empty().extend(segments);
        Ok(u)
    }

    pub(crate) fn mailbox_url(&self, rest: &[&str]) -> Result<Url, CoreError> {
        let mut segs: Vec<&str> = self.cfg.mailbox_path.split('/').collect();
        segs.extend_from_slice(rest);
        self.url(&segs)
    }

    /// A link Microsoft hands back is followed only on the origin this mailbox was configured with,
    /// so a poisoned page cannot send the bearer token elsewhere.
    pub(crate) fn check_origin(&self, link: &str) -> Result<(), CoreError> {
        let base = Url::parse(&self.cfg.base_url).map_err(|e| invalid("base_url", e.to_string()))?;
        let target = Url::parse(link).map_err(|e| invalid("link", format!("{e}")))?;
        let same = base.scheme() == target.scheme() && base.host_str() == target.host_str() && base.port_or_known_default() == target.port_or_known_default();
        if same { Ok(()) } else { Err(invalid("link", "a link points outside the Microsoft address this mailbox uses; not followed")) }
    }

    async fn attempt_once(&self, attempt: u32, method: &Method, url: &str, extra: &[(&str, String)], body: Option<&Value>, allow_gone: bool) -> Result<Attempt, CoreError> {
        let _slot = self.throttle.acquire().await;
        netguard::outbound_allowed_url(url)?;   // rig-net-rust: the dial itself, before the token is read and before any lookup
        let token = self.tokens.access_token().await?;
        let mut rb = self.http.request(method.clone(), url).bearer_auth(&token).header("Accept", "application/json");
        for (k, v) in extra {
            rb = rb.header(*k, v);
        }
        if let Some(b) = body {
            rb = rb.json(b);
        }
        let unreachable = || Attempt::Retry { failure: self.failure("could not reach Microsoft, check the connection".into(), true), wait_ms: self.backoff(attempt), slowed: false };
        let resp = match rb.send().await {
            Ok(r) => r,
            Err(_) => return Ok(unreachable()),
        };
        let status = resp.status().as_u16();
        let retry_after_ms = resp.headers().get("retry-after").and_then(|v| v.to_str().ok()).and_then(|s| s.trim().parse::<u64>().ok()).map(|s| (s * 1000).min(self.cfg.backoff_max_ms));
        let text = match resp.text().await {
            Ok(t) => t,
            Err(_) => return Ok(unreachable()),
        };
        let (code, message) = graph_error(&text);
        match status {
            200..=299 => Ok(Attempt::Done(Reply { status, body: text })),
            410 if allow_gone => Ok(Attempt::Done(Reply { status, body: text })),
            429 | 503 if status == 429 || retry_after_ms.is_some() => {
                let wait_ms = retry_after_ms.unwrap_or_else(|| self.backoff(attempt));
                Ok(Attempt::Retry { failure: self.failure(slowed_words(wait_ms), true), wait_ms, slowed: true })
            }
            500..=599 => Ok(Attempt::Retry { failure: self.failure(format!("Microsoft is not answering (HTTP {status} {code})"), true), wait_ms: self.backoff(attempt), slowed: false }),
            401 => Err(CoreError::Action { failure: self.failure(format!("Microsoft sign-in expired, sign in to this account again (HTTP 401 {code})"), false) }),
            _ => Err(CoreError::Action { failure: self.failure(format!("Microsoft refused the request (HTTP {status} {code}: {message})"), false) }),
        }
    }

    /// One call with the throttle around it and bounded retries. A retried call pauses the whole mailbox.
    pub(crate) async fn send(&self, method: Method, url: &str, extra: &[(&str, String)], body: Option<&Value>, allow_gone: bool) -> Result<Reply, CoreError> {
        let mut last = None;
        for attempt in 1..=self.cfg.max_attempts {
            match self.attempt_once(attempt, &method, url, extra, body, allow_gone).await? {
                Attempt::Done(reply) => return Ok(reply),
                Attempt::Retry { failure, wait_ms, slowed } => {
                    if attempt < self.cfg.max_attempts {
                        if slowed {
                            if let Some(sink) = &self.slowed {
                                sink(failure.clone());
                            }
                        }
                        self.throttle.pause_for(wait_ms);
                    }
                    last = Some(failure);
                }
            }
        }
        Err(CoreError::Action { failure: last.expect("max_attempts is at least 1") })
    }

    pub(crate) fn page_header(&self) -> Vec<(&'static str, String)> {
        vec![("Prefer", format!("odata.maxpagesize={}, IdType=\"ImmutableId\"", self.cfg.page_size))]
    }
}
