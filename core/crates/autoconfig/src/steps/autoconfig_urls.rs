//! Step b: the domain's own autoconfig files, the subdomain URL first, then the well-known URL.
use super::{get_ok, StepResult};
use crate::domain::{address::EmailAddress, config::{Outcome, Source}};
use crate::parse::mozilla;
use crate::ports::HttpPort;

pub(crate) fn urls(email: &EmailAddress) -> [String; 2] {
    let d = email.domain();
    [
        format!("https://autoconfig.{d}/mail/config-v1.1.xml?emailaddress={}", email.full().replace('+', "%2B")),
        format!("https://{d}/.well-known/autoconfig/mail/config-v1.1.xml"),
    ]
}

pub(crate) fn run(http: &dyn HttpPort, email: &EmailAddress, user_agent: &str) -> StepResult {
    for url in urls(email) {
        if let Ok(Some(body)) = get_ok(http, url, user_agent) {
            if let Some(c) = mozilla::parse(&body, email, Source::Autoconfig) {
                return StepResult::Answered(Outcome::Found(c), false);
            }
        }
    }
    StepResult::Missed
}
