//! Microsoft Autodiscover: the POX (plain old XML) response of a domain's own Autodiscover server, and the
//! JSON answer of the shared Microsoft 365 front that says whether a domain is a Microsoft 365 tenant.
use crate::consts::{IMAPS_PORT, M365_IMAP_HOST, M365_SMTP_HOST, SMTPS_PORT, SUBMISSION_PORT};
use crate::domain::address::EmailAddress;
use crate::domain::config::{AuthMethod, MailConfig, Security, Source};
use crate::xml;

/// The request body of an Autodiscover POX query (the Outlook response schema, 2006a).
pub fn pox_request(email: &EmailAddress) -> String {
    format!(
        "<?xml version=\"1.0\" encoding=\"utf-8\"?>\n<Autodiscover xmlns=\"http://schemas.microsoft.com/exchange/autodiscover/outlook/requestschema/2006\"><Request><EMailAddress>{}</EMailAddress><AcceptableResponseSchema>http://schemas.microsoft.com/exchange/autodiscover/outlook/responseschema/2006a</AcceptableResponseSchema></Request></Autodiscover>",
        email.full()
    )
}

struct Proto { host: String, port: u16, security: Security, login: Option<String> }

fn proto(el: &xml::Element) -> Option<Proto> {
    let host = el.child_text("Server")?;
    let port = el.child_text("Port")?.parse::<u16>().ok()?;
    let security = match el.child_text("Encryption").map(|s| s.to_ascii_uppercase()) {
        Some(e) if e == "SSL" => Security::Ssl,
        Some(e) if e == "TLS" => Security::Starttls,
        Some(_) => return None,
        // no Encryption element: SSL=on means TLS, implicit on the implicit-TLS ports, STARTTLS elsewhere
        None => match el.child_text("SSL").map(|s| s.to_ascii_lowercase()).as_deref() {
            Some("off") => return None,
            _ if port == IMAPS_PORT || port == SMTPS_PORT => Security::Ssl,
            _ => Security::Starttls,
        },
    };
    let login = el.child_text("LoginName").filter(|s| !s.is_empty());
    (!host.is_empty()).then_some(Proto { host, port, security, login })
}

/// IMAP and SMTP settings from a POX response, or None.
pub fn parse_pox(doc: &str, email: &EmailAddress) -> Option<MailConfig> {
    let root = xml::parse(doc).ok()?;
    let account = root.child("Response")?.child("Account")?;
    let pick = |kind: &str| account.children_named("Protocol").filter(|p| p.child_text("Type").is_some_and(|t| t.eq_ignore_ascii_case(kind))).find_map(proto);
    let imap = pick("IMAP")?;
    let smtp = pick("SMTP")?;
    let username = imap.login.clone().unwrap_or_else(|| email.full().to_string());
    Some(MailConfig {
        imap_host: imap.host,
        imap_port: imap.port,
        imap_security: imap.security,
        smtp_host: smtp.host,
        smtp_port: smtp.port,
        smtp_security: smtp.security,
        smtp_username: smtp.login.unwrap_or_else(|| username.clone()),
        username,
        auth_method: AuthMethod::Password,
        source: Source::Autodiscover,
    })
}

/// True when the shared front points the address at Exchange Online.
pub fn is_microsoft_365(json: &str) -> bool {
    serde_json::from_str::<serde_json::Value>(json)
        .ok()
        .and_then(|v| v.get("Url").and_then(|u| u.as_str()).map(|u| u.to_ascii_lowercase()))
        .is_some_and(|u| u.starts_with(&format!("https://{M365_IMAP_HOST}/")))
}

/// Exchange Online's IMAP and SMTP settings; sign-in is OAuth2 (coming soon), Manual setup stays open.
pub fn microsoft_365_config(email: &EmailAddress) -> MailConfig {
    MailConfig {
        imap_host: M365_IMAP_HOST.into(),
        imap_port: IMAPS_PORT,
        imap_security: Security::Ssl,
        smtp_host: M365_SMTP_HOST.into(),
        smtp_port: SUBMISSION_PORT,
        smtp_security: Security::Starttls,
        username: email.full().into(),
        smtp_username: email.full().into(),
        auth_method: AuthMethod::Oauth2,
        source: Source::Autodiscover,
    }
}
