//! Step d: Microsoft Autodiscover. The domain's own server (POX over HTTPS POST) for IMAP/SMTP settings,
//! then the shared Microsoft 365 front, which marks the domain a tenant (sign-in by OAuth2, coming soon).
use super::{get_ok, StepResult};
use crate::consts::MS_AUTODISCOVER_V2;
use crate::domain::{address::EmailAddress, config::Outcome};
use crate::parse::autodiscover;
use crate::ports::{HttpPort, HttpRequest, Method};

pub(crate) fn run(http: &dyn HttpPort, email: &EmailAddress, user_agent: &str) -> StepResult {
    let req = HttpRequest {
        method: Method::Post,
        url: format!("https://autodiscover.{}/autodiscover/autodiscover.xml", email.domain()),
        user_agent: user_agent.to_string(),
        body: Some(autodiscover::pox_request(email)),
    };
    if let Ok(r) = http.execute(&req) {
        if r.status == 200 {
            if let Some(c) = autodiscover::parse_pox(&r.body, email) {
                return StepResult::Answered(Outcome::Found(c), false);
            }
        }
    }
    let url = format!("{MS_AUTODISCOVER_V2}?Email={}&Protocol=AutodiscoverV1", email.full().replace('+', "%2B"));
    if let Ok(Some(body)) = get_ok(http, url, user_agent) {
        if autodiscover::is_microsoft_365(&body) {
            return StepResult::Answered(Outcome::Found(autodiscover::microsoft_365_config(email)), true);
        }
    }
    StepResult::Missed
}
