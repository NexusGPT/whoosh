//! Step: the domain's mail record (MX). The mail exchangers are read in preference order; the first that belongs to a
//! provider in the table below fills that provider's servers (SPEC A76: the domain's mail record, after ISPDB and the
//! autoconfig host, before the probed guesses). A domain whose exchangers match no row misses, and the guess runs next.
//! The table also names the provider of a configuration found by any step, by its IMAP host.
use super::StepResult;
use crate::consts::{IMAPS_PORT, M365_IMAP_HOST, M365_SMTP_HOST, PROTON_BRIDGE_IMAP_PORT, PROTON_BRIDGE_SMTP_PORT, SMTPS_PORT, SUBMISSION_PORT};
use crate::domain::{address::EmailAddress, config::{AuthMethod, MailConfig, Outcome, Provider, Security, Source}};
use crate::ports::{DnsAnswer, DnsPort, DnsQuery};

pub(crate) struct ProviderRow {
    /// None: a known server set with no `Provider` name yet (Yahoo: the name needs a variant in the engine's map, which this crate does not own).
    pub provider: Option<Provider>,
    /// An exchanger belongs to the row when its host equals or ends with ".<suffix>".
    pub mx_suffixes: &'static [&'static str],
    pub imap_host: &'static str,
    pub imap_port: u16,
    pub imap_security: Security,
    pub smtp_host: &'static str,
    pub smtp_port: u16,
    pub smtp_security: Security,
    pub auth_method: AuthMethod,
}

pub(crate) const PROVIDERS: [ProviderRow; 6] = [
    // Microsoft 365: a tenant's MX is <tenant>.mail.protection.outlook.com
    // (https://learn.microsoft.com/en-us/microsoft-365/enterprise/external-domain-name-system-records); IMAP and SMTP hosts
    // (https://support.microsoft.com/en-us/office/pop-imap-and-smtp-settings-for-outlook-com-d088b986-291d-42b8-9564-9c414e2aa040).
    // Exchange Online takes no password login for IMAP or SMTP: sign-in only.
    ProviderRow { provider: Some(Provider::Microsoft365), mx_suffixes: &["mail.protection.outlook.com"], imap_host: M365_IMAP_HOST, imap_port: IMAPS_PORT, imap_security: Security::Ssl, smtp_host: M365_SMTP_HOST, smtp_port: SUBMISSION_PORT, smtp_security: Security::Starttls, auth_method: AuthMethod::Oauth2 },
    // Google Workspace and Gmail: MX aspmx.l.google.com / smtp.google.com / *.googlemail.com (https://support.google.com/a/answer/174125);
    // IMAP and SMTP settings (https://support.google.com/mail/answer/7126229). An app password works.
    ProviderRow { provider: Some(Provider::Google), mx_suffixes: &["google.com", "googlemail.com"], imap_host: "imap.gmail.com", imap_port: IMAPS_PORT, imap_security: Security::Ssl, smtp_host: "smtp.gmail.com", smtp_port: SUBMISSION_PORT, smtp_security: Security::Starttls, auth_method: AuthMethod::Password },
    // iCloud Mail, including custom domains (MX mx01.mail.icloud.com): server settings (https://support.apple.com/en-us/102525).
    // App-specific password.
    ProviderRow { provider: Some(Provider::Icloud), mx_suffixes: &["icloud.com"], imap_host: "imap.mail.me.com", imap_port: IMAPS_PORT, imap_security: Security::Ssl, smtp_host: "smtp.mail.me.com", smtp_port: SUBMISSION_PORT, smtp_security: Security::Starttls, auth_method: AuthMethod::Password },
    // Fastmail: MX in1-smtp.messagingengine.com; server names and ports
    // (https://www.fastmail.help/hc/en-us/articles/1500000278342-Server-names-and-ports). App password.
    ProviderRow { provider: Some(Provider::Fastmail), mx_suffixes: &["messagingengine.com"], imap_host: "imap.fastmail.com", imap_port: IMAPS_PORT, imap_security: Security::Ssl, smtp_host: "smtp.fastmail.com", smtp_port: SUBMISSION_PORT, smtp_security: Security::Starttls, auth_method: AuthMethod::Password },
    // Proton Mail: MX mail.protonmail.ch / mailsec.protonmail.ch; mail is read through Proton Mail Bridge on this Mac
    // (https://proton.me/support/imap-smtp-and-pop3-setup): 127.0.0.1, IMAP 1143 and SMTP 1025, STARTTLS, the Bridge password.
    ProviderRow { provider: Some(Provider::ProtonBridge), mx_suffixes: &["protonmail.ch", "protonmail.com"], imap_host: "127.0.0.1", imap_port: PROTON_BRIDGE_IMAP_PORT, imap_security: Security::Starttls, smtp_host: "127.0.0.1", smtp_port: PROTON_BRIDGE_SMTP_PORT, smtp_security: Security::Starttls, auth_method: AuthMethod::Password },
    // Yahoo Mail, including a custom domain whose MX is <host>.am0.yahoodns.net. Only am0.yahoodns.net is matched: the shorter
    // yahoodns.net is "also used for AOL" (fixtures/autoconfig/ispdb-yahoo.com.xml:24-25), so the wider suffix would also match AOL domains.
    // source: the recorded Thunderbird ISPDB answer on this disk, fixtures/autoconfig/ispdb-yahoo.com.xml:31-33 (IMAP imap.mail.yahoo.com:993 SSL)
    // and :55-57 (SMTP smtp.mail.yahoo.com:465 SSL); Yahoo's own help page it names (documentation url, same file :63) is not on this
    // disk, so these values are unverified against Yahoo itself, see docs/lanes/provider-limits.md questions_for_owner. A password
    // login (the fixture lists password-cleartext next to OAuth2; Whoosh holds no Yahoo OAuth client that this file knows of).
    ProviderRow { provider: None, mx_suffixes: &["am0.yahoodns.net"], imap_host: "imap.mail.yahoo.com", imap_port: IMAPS_PORT, imap_security: Security::Ssl, smtp_host: "smtp.mail.yahoo.com", smtp_port: SMTPS_PORT, smtp_security: Security::Ssl, auth_method: AuthMethod::Password },
];

fn belongs(host: &str, suffix: &str) -> bool { host == suffix || host.ends_with(&format!(".{suffix}")) }

/// The provider row an exchanger host belongs to.
pub(crate) fn row_for_mx(host: &str) -> Option<&'static ProviderRow> {
    let host = host.trim_end_matches('.').to_ascii_lowercase();
    PROVIDERS.iter().find(|r| r.mx_suffixes.iter().any(|s| belongs(&host, s)))
}

/// The provider whose IMAP host this is (a configuration found by any step).
pub fn provider_for_imap_host(host: &str) -> Option<Provider> {
    let host = host.trim_end_matches('.').to_ascii_lowercase();
    PROVIDERS.iter().find(|r| r.imap_host == host).and_then(|r| r.provider)
}

pub(crate) fn run(dns: &dyn DnsPort, email: &EmailAddress) -> StepResult {
    let Ok(DnsAnswer::Mx(mut records)) = dns.execute(&DnsQuery::Mx(email.domain().to_string())) else { return StepResult::Missed };
    records.sort_by_key(|r| r.preference);
    let Some(row) = records.iter().find_map(|r| row_for_mx(&r.exchange)) else { return StepResult::Missed };
    let login = email.full().to_string();
    StepResult::Answered(
        Outcome::Found(MailConfig {
            imap_host: row.imap_host.into(),
            imap_port: row.imap_port,
            imap_security: row.imap_security,
            smtp_host: row.smtp_host.into(),
            smtp_port: row.smtp_port,
            smtp_security: row.smtp_security,
            username: login.clone(),
            smtp_username: login,
            auth_method: row.auth_method,
            source: Source::MailRecord,
        }),
        row.provider == Some(Provider::Microsoft365),
    )
}
