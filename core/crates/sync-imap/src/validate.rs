//! The login test of the add-account screen: connect with the config's security, sign in, LIST,
//! map the folder roles, and (when a sending server is given) check SMTP EHLO and sign-in. Every
//! step reports its outcome in words the owner can act on; the server's own text is kept.
use std::sync::Arc;

use async_trait::async_trait;
use send::{Security as SmtpSecurity, SmtpCredentials, SmtpSettings};

use crate::{
    adapter::{list_on, open_session, step_fail, Auth, Dial, OpenFail, TokenSource},
    port::Folder,
    roles::{role_and_source, FolderRole, RoleBy},
    transport::Security,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Step { ImapConnect, ImapTls, ImapLogin, ImapList, SmtpConnect, SmtpLogin }

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StepResult { pub step: Step, pub ok: bool, pub words: String }

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RoleMapping { pub role: FolderRole, pub folder: String, pub by: RoleBy }

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Credential {
    /// The mail password or an app password.
    Password(String),
    /// An OAuth2 access token (XOAUTH2 on IMAP and SMTP).
    Token(String),
}

#[derive(Debug, Clone)]
pub struct ImapEndpoint { pub host: String, pub port: u16, pub security: Security }

#[derive(Debug, Clone)]
pub struct SmtpEndpoint { pub host: String, pub port: u16, pub security: SmtpSecurity }

#[derive(Debug, Clone)]
pub struct ValidationConfig {
    pub username: String,
    pub imap: ImapEndpoint,
    pub smtp: Option<SmtpEndpoint>,
    /// Extra trusted CA certificates (DER); empty in normal use.
    pub extra_roots_der: Vec<Vec<u8>>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ValidationReport {
    /// The steps that ran, in order. A step after a failed one does not run.
    pub steps: Vec<StepResult>,
    pub folders: Vec<Folder>,
    pub roles: Vec<RoleMapping>,
    /// Capabilities the server advertised after sign-in (IDLE, CONDSTORE, QRESYNC, SPECIAL-USE, ...).
    pub capabilities: Vec<String>,
}

impl ValidationReport {
    /// True only when every step that ran passed and the mail server answered the folder list.
    pub fn ok(&self) -> bool {
        self.steps.iter().all(|s| s.ok) && self.steps.iter().any(|s| s.step == Step::ImapList)
    }
    pub fn first_failure(&self) -> Option<&StepResult> { self.steps.iter().find(|s| !s.ok) }
    fn pass(&mut self, step: Step, words: impl Into<String>) { self.steps.push(StepResult { step, ok: true, words: words.into() }); }
    fn fail(&mut self, step: Step, words: impl Into<String>) { self.steps.push(StepResult { step, ok: false, words: words.into() }); }
}

struct FixedToken(String);
#[async_trait]
impl TokenSource for FixedToken {
    async fn access_token(&self) -> Result<String, String> { Ok(self.0.clone()) }
}

fn secure_words(security: Security, host: &str) -> String {
    match security {
        Security::Tls => format!("The connection to {host} is encrypted (TLS from the first byte)."),
        Security::StartTls => format!("The connection to {host} was switched to TLS (STARTTLS)."),
        Security::PlainLoopbackOnly => format!("{host} is this Mac: no encryption needed."),
    }
}

fn record_open_fail(report: &mut ValidationReport, config: &ValidationConfig, f: OpenFail) {
    // The steps before the failing one passed.
    if f.step != Step::ImapConnect {
        report.pass(Step::ImapConnect, format!("Reached {} on port {}.", config.imap.host, config.imap.port));
    }
    if f.step == Step::ImapLogin {
        report.pass(Step::ImapTls, secure_words(config.imap.security, &config.imap.host));
    }
    report.fail(f.step, f.words);
}

/// Checks an IMAP account (and its SMTP server when given) with a real sign-in. Nothing is saved.
pub async fn validate(config: &ValidationConfig, credential: &Credential) -> ValidationReport {
    let mut report = ValidationReport::default();
    let auth = match credential {
        Credential::Password(p) => Auth::Password(p.clone()),
        Credential::Token(t) => Auth::XOAuth2(Arc::new(FixedToken(t.clone()))),
    };
    let dial = Dial { host: &config.imap.host, port: config.imap.port, security: config.imap.security, username: &config.username, auth: &auth, extra_roots_der: &config.extra_roots_der };
    match open_session(&dial, &None).await {
        Err(f) => record_open_fail(&mut report, config, f),
        Ok(mut sess) => {
            report.pass(Step::ImapConnect, format!("Reached {} on port {}.", config.imap.host, config.imap.port));
            report.pass(Step::ImapTls, secure_words(config.imap.security, &config.imap.host));
            report.pass(Step::ImapLogin, format!("Signed in as {}.", config.username));
            report.capabilities = sess.caps.names();
            match list_on(&mut sess).await {
                Err(e) => { let f = step_fail(Step::ImapList, e); report.fail(f.step, f.words); }
                Ok(listed) if listed.is_empty() => report.fail(Step::ImapList, "Signed in, but the server lists no folders."),
                Ok(listed) => {
                    report.pass(Step::ImapList, format!("The server lists {} folders.", listed.len()));
                    for l in &listed {
                        let (role, by) = role_and_source(&l.folder.name, &l.attrs);
                        if role != FolderRole::Other { report.roles.push(RoleMapping { role, folder: l.folder.name.clone(), by }); }
                    }
                    report.folders = listed.into_iter().map(|l| l.folder).collect();
                }
            }
        }
    }
    if let Some(smtp) = &config.smtp {
        check_smtp(&mut report, config, smtp, credential).await;
    }
    report
}

async fn check_smtp(report: &mut ValidationReport, config: &ValidationConfig, smtp: &SmtpEndpoint, credential: &Credential) {
    let (secret, token) = match credential { Credential::Password(p) => (p.clone(), false), Credential::Token(t) => (t.clone(), true) };
    let settings = SmtpSettings {
        host: smtp.host.clone(),
        port: smtp.port,
        security: smtp.security,
        credentials: Some(SmtpCredentials { username: config.username.clone(), secret, bearer: token }),
    };
    let host = smtp.host.clone();
    let outcome = tokio::task::spawn_blocking(move || smtp_check(&settings, token)).await;
    match outcome {
        Ok(Ok(())) => {
            report.pass(Step::SmtpConnect, format!("The sending server {host} answered EHLO."));
            report.pass(Step::SmtpLogin, format!("Signed in to {host} for sending."));
        }
        Ok(Err((step, words))) => {
            if step == Step::SmtpLogin { report.pass(Step::SmtpConnect, format!("The sending server {host} answered EHLO.")); }
            report.fail(step, words);
        }
        Err(e) => report.fail(Step::SmtpConnect, format!("The sending server check stopped: {e}")),
    }
}

/// EHLO, the TLS upgrade the settings ask for, and AUTH, then NOOP. Written to be replaced by a
/// `verify` function in the send crate's smtp module (reported in docs/lanes/core-sync-imap.md);
/// until that exists this builds the same lettre transport the send crate builds.
fn smtp_check(settings: &SmtpSettings, token: bool) -> Result<(), (Step, String)> {
    use lettre::transport::smtp::{authentication::{Credentials, Mechanism}, SmtpTransport};
    let host = settings.host.as_str();
    netguard::outbound_allowed(host).map_err(|e| (Step::SmtpConnect, match e { contracts::CoreError::Action { failure } => failure.reason_words, other => other.to_string() }))?;   // engine-act-r6 round 5
    let loopback = host == "localhost" || host.parse::<std::net::IpAddr>().map(|ip| ip.is_loopback()).unwrap_or(false);
    let builder = match settings.security {
        SmtpSecurity::Plaintext => {
            if !loopback { return Err((Step::SmtpConnect, format!("Plain SMTP is refused for {host}: only this Mac may skip encryption."))); }
            SmtpTransport::builder_dangerous(host)
        }
        SmtpSecurity::Implicit => SmtpTransport::relay(host).map_err(|e| (Step::SmtpConnect, e.to_string()))?,
        SmtpSecurity::StartTls => SmtpTransport::starttls_relay(host).map_err(|e| (Step::SmtpConnect, e.to_string()))?,
    };
    let mut builder = builder.port(settings.port);
    if let Some(c) = &settings.credentials {
        builder = builder.credentials(Credentials::new(c.username.clone(), c.secret.clone()));
        if token { builder = builder.authentication(vec![Mechanism::Xoauth2]); }
    }
    match builder.build().test_connection() {
        Ok(_) => Ok(()),
        Err(e) => {
            // A server that answered with a status refused the sign-in; no status means no connection.
            let step = if e.status().is_some() { Step::SmtpLogin } else { Step::SmtpConnect };
            let words = if step == Step::SmtpLogin { format!("The sending server refused the sign-in: {e}") } else { format!("Cannot reach the sending server {host} on port {}: {e}", settings.port) };
            Err((step, words))
        }
    }
}
