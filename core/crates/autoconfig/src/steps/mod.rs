//! One file per step. Each step answers, misses (the next step runs), or stops detection with a refusal.
pub(crate) mod autoconfig_urls;
pub(crate) mod autodiscover;
pub(crate) mod guess;
pub(crate) mod ispdb;
pub(crate) mod mail_record;
pub(crate) mod srv;

use crate::domain::config::{Outcome, Refusal};
use crate::ports::{HttpPort, HttpRequest, Method, NetError};

pub(crate) enum StepResult {
    /// `Outcome::Found` or `Outcome::ImapOnly`, and whether the domain is a Microsoft 365 tenant.
    Answered(Outcome, bool),
    Missed,
    Stop(Refusal),
}

/// A GET that answered 200 with a body; anything else is None.
pub(crate) fn get_ok(http: &dyn HttpPort, url: String, user_agent: &str) -> Result<Option<String>, NetError> {
    let req = HttpRequest { method: Method::Get, url, user_agent: user_agent.to_string(), body: None };
    match http.execute(&req) {
        Ok(r) if r.status == 200 && !r.body.trim().is_empty() => Ok(Some(r.body)),
        Ok(_) => Ok(None),
        Err(e) => Err(e),
    }
}
