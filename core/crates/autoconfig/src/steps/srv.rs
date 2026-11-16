//! Step c: DNS SRV records, RFC 6186. `_imaps._tcp` (implicit TLS), else `_imap._tcp` (STARTTLS); for sending
//! `_submission._tcp`, else `_submissions._tcp` (implicit TLS, RFC 8314). A target of "." means not offered.
//! The step first asks whether the domain exists at all: a domain that does not exist stops detection.
use super::StepResult;
use crate::consts::SMTPS_PORT;
use crate::domain::{address::EmailAddress, config::{AuthMethod, MailConfig, Outcome, Refusal, Security, Source}};
use crate::ports::{DnsAnswer, DnsPort, DnsQuery, SrvRecord};

fn best(dns: &dyn DnsPort, name: String) -> Option<SrvRecord> {
    match dns.execute(&DnsQuery::Srv(name)) {
        Ok(DnsAnswer::Srv(mut records)) => {
            records.retain(|r| r.target != "." && !r.target.is_empty() && r.port != 0);
            // lowest priority first; among equals, the heaviest weight
            records.sort_by(|a, b| a.priority.cmp(&b.priority).then(b.weight.cmp(&a.weight)));
            records.into_iter().next().map(|mut r| { r.target = r.target.trim_end_matches('.').to_string(); r })
        }
        _ => None,
    }
}

pub(crate) fn run(dns: &dyn DnsPort, email: &EmailAddress) -> StepResult {
    let d = email.domain();
    if let Ok(DnsAnswer::Exists(false)) = dns.execute(&DnsQuery::Exists(d.to_string())) {
        return StepResult::Stop(Refusal::DomainNotFound(d.to_string()));
    }
    let imap = best(dns, format!("_imaps._tcp.{d}")).map(|r| (r, Security::Ssl))
        .or_else(|| best(dns, format!("_imap._tcp.{d}")).map(|r| (r, Security::Starttls)));
    let Some((imap, imap_security)) = imap else { return StepResult::Missed };
    let smtp = best(dns, format!("_submission._tcp.{d}"))
        .map(|r| { let s = if r.port == SMTPS_PORT { Security::Ssl } else { Security::Starttls }; (r, s) })
        .or_else(|| best(dns, format!("_submissions._tcp.{d}")).map(|r| (r, Security::Ssl)));
    let Some((smtp, smtp_security)) = smtp else { return StepResult::Missed };
    StepResult::Answered(
        Outcome::Found(MailConfig {
            imap_host: imap.target,
            imap_port: imap.port,
            imap_security,
            smtp_host: smtp.target,
            smtp_port: smtp.port,
            smtp_security,
            username: email.full().into(),
            smtp_username: email.full().into(),
            auth_method: AuthMethod::Password,
            source: Source::Srv,
        }),
        false,
    )
}
