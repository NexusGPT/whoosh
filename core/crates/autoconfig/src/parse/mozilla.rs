//! The Mozilla Autoconfig format, version 1.1 (clientConfig > emailProvider > incomingServer / outgoingServer),
//! as served by the provider database (step a) and by a domain's own autoconfig files (step b).
use crate::domain::address::EmailAddress;
use crate::domain::config::{AuthMethod, MailConfig, Security, Source};
use crate::xml::{self, Element};

/// Replaces the format's placeholders. Returns None when an unknown placeholder is left, because a
/// login or host with a literal `%…%` in it can only fail.
pub fn resolve_placeholders(value: &str, email: &EmailAddress) -> Option<String> {
    let out = value
        .replace("%EMAILADDRESS%", email.full())
        .replace("%EMAILLOCALPART%", email.local_part())
        .replace("%EMAILDOMAIN%", email.domain());
    (!out.contains('%')).then_some(out)
}

pub(crate) fn security(socket_type: &str) -> Option<Security> {
    match socket_type.trim().to_ascii_uppercase().as_str() {
        "SSL" | "TLS" => Some(Security::Ssl),
        "STARTTLS" => Some(Security::Starttls),
        _ => None, // "plain": a password is never sent in clear
    }
}

struct Server { host: String, port: u16, security: Security, username: String, oauth2: bool }

fn server(el: &Element, email: &EmailAddress) -> Option<Server> {
    let host = resolve_placeholders(&el.child_text("hostname")?, email)?;
    let port = el.child_text("port")?.parse::<u16>().ok()?;
    let security = security(&el.child_text("socketType")?)?;
    let username = resolve_placeholders(&el.child_text("username").unwrap_or_else(|| "%EMAILADDRESS%".into()), email)?;
    let oauth2 = el.children_named("authentication").any(|a| a.text().eq_ignore_ascii_case("OAuth2"));
    (!host.is_empty() && port != 0).then_some(Server { host, port, security, username, oauth2 })
}

fn first_server(provider: &Element, tag: &str, kind: &str, email: &EmailAddress) -> Option<Server> {
    provider
        .children_named(tag)
        .filter(|e| e.attr("type").is_some_and(|t| t.eq_ignore_ascii_case(kind)))
        .find_map(|e| server(e, email))
}

/// The first IMAP server and the first SMTP server offered over TLS, or None.
pub fn parse(doc: &str, email: &EmailAddress, source: Source) -> Option<MailConfig> {
    let root = xml::parse(doc).ok()?;
    if !root.name.eq_ignore_ascii_case("clientConfig") { return None; }
    let provider = root.child("emailProvider")?;
    let imap = first_server(provider, "incomingServer", "imap", email)?;
    let smtp = first_server(provider, "outgoingServer", "smtp", email)?;
    Some(MailConfig {
        imap_host: imap.host,
        imap_port: imap.port,
        imap_security: imap.security,
        smtp_host: smtp.host,
        smtp_port: smtp.port,
        smtp_security: smtp.security,
        username: imap.username,
        smtp_username: smtp.username,
        auth_method: if imap.oauth2 { AuthMethod::Oauth2 } else { AuthMethod::Password },
        source,
    })
}
