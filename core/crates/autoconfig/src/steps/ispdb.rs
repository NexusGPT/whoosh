//! Step a: the provider database, https://autoconfig.thunderbird.net/v1.1/{domain}. Sends the domain only.
use super::{get_ok, StepResult};
use crate::consts::ISPDB_BASE;
use crate::domain::{address::EmailAddress, config::{Outcome, Refusal, Source}};
use crate::parse::mozilla;
use crate::ports::{HttpPort, NetError};

pub(crate) fn run(http: &dyn HttpPort, email: &EmailAddress, user_agent: &str) -> StepResult {
    match get_ok(http, format!("{ISPDB_BASE}{}", email.domain()), user_agent) {
        Ok(Some(body)) => match mozilla::parse(&body, email, Source::Ispdb) {
            Some(c) => StepResult::Answered(Outcome::Found(c), false),
            None => StepResult::Missed,
        },
        Ok(None) => StepResult::Missed,
        // the database host is always up: failing to reach it means this Mac is offline
        Err(NetError::Offline | NetError::HostNotFound) => StepResult::Stop(Refusal::NoNetwork),
        Err(_) => StepResult::Missed,
    }
}
