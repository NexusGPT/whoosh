//! Step e: guess imap./mail./the bare domain (IMAP) and smtp./mail. (SMTP) on the usual ports with a TCP+TLS probe. Per host,
//! implicit TLS first; STARTTLS on 143/587 only when the implicit-TLS port refuses. The bare domain comes last, after the
//! named hosts (the comparison with rusted-mail, provider_autodiscovery.rs:517-555: it probes the bare domain for IMAP).
//! Fills what answers.
use super::StepResult;
use crate::consts::{IMAPS_PORT, IMAP_PORT, SMTPS_PORT, SUBMISSION_PORT};
use crate::domain::{address::EmailAddress, config::{AuthMethod, ImapOnlyConfig, MailConfig, Outcome, Security, Source}};
use crate::ports::{ProbePort, ProbeRequest, Protocol};

pub(crate) fn candidates(email: &EmailAddress, protocol: Protocol) -> Vec<(String, u16, Security)> {
    let d = email.domain();
    let (hosts, tls_port, starttls_port) = match protocol {
        Protocol::Imap => (vec![format!("imap.{d}"), format!("mail.{d}"), d.to_string()], IMAPS_PORT, IMAP_PORT),
        Protocol::Smtp => (vec![format!("smtp.{d}"), format!("mail.{d}")], SMTPS_PORT, SUBMISSION_PORT),
    };
    hosts.into_iter().flat_map(|h| [(h.clone(), tls_port, Security::Ssl), (h, starttls_port, Security::Starttls)]).collect()
}

fn first_answer(probe: &dyn ProbePort, email: &EmailAddress, protocol: Protocol) -> Option<(String, u16, Security)> {
    candidates(email, protocol).into_iter().find(|(host, port, security)| {
        probe.execute(&ProbeRequest { host: host.clone(), port: *port, protocol, security: *security }).is_ok()
    })
}

pub(crate) fn run(probe: &dyn ProbePort, email: &EmailAddress) -> StepResult {
    let Some((imap_host, imap_port, imap_security)) = first_answer(probe, email, Protocol::Imap) else { return StepResult::Missed };
    let username = email.full().to_string();
    let outcome = match first_answer(probe, email, Protocol::Smtp) {
        Some((smtp_host, smtp_port, smtp_security)) => Outcome::Found(MailConfig {
            imap_host, imap_port, imap_security, smtp_host, smtp_port, smtp_security,
            smtp_username: username.clone(), username, auth_method: AuthMethod::Password, source: Source::Guess,
        }),
        None => Outcome::ImapOnly(ImapOnlyConfig { imap_host, imap_port, imap_security, username, source: Source::Guess }),
    };
    StepResult::Answered(outcome, false)
}
