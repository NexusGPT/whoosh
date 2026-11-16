//! The detected configuration (the eight fields of docs/PRD.md plus its source) and the result of a detection.

/// Plain text is never a result: a server offered only in plain text is skipped.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Security { Ssl, Starttls }

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthMethod { Password, Oauth2 }

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Source { Ispdb, Autoconfig, Srv, Autodiscover, MailRecord, Guess }

/// The mail provider behind a configuration, when it is one the sheet has words for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Provider { Microsoft365, Google, Icloud, Fastmail, ProtonBridge }

impl Provider {
    pub fn as_str(self) -> &'static str {
        match self { Self::Microsoft365 => "microsoft365", Self::Google => "google", Self::Icloud => "icloud", Self::Fastmail => "fastmail", Self::ProtonBridge => "proton-bridge" }
    }
    /// The provider's name as the sheet says it.
    pub fn name(self) -> &'static str {
        match self { Self::Microsoft365 => "Microsoft 365", Self::Google => "Google", Self::Icloud => "iCloud", Self::Fastmail => "Fastmail", Self::ProtonBridge => "Proton Mail Bridge" }
    }
}

impl Security {
    pub fn as_str(self) -> &'static str { match self { Self::Ssl => "ssl", Self::Starttls => "starttls" } }
}
impl AuthMethod {
    pub fn as_str(self) -> &'static str { match self { Self::Password => "password", Self::Oauth2 => "oauth2" } }
}
impl Source {
    pub fn as_str(self) -> &'static str {
        match self { Self::Ispdb => "ispdb", Self::Autoconfig => "autoconfig", Self::Srv => "srv", Self::Autodiscover => "autodiscover", Self::MailRecord => "mx", Self::Guess => "guess" }
    }
}

/// A complete configuration, pre-filled into the Add account summary.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MailConfig {
    pub imap_host: String,
    pub imap_port: u16,
    pub imap_security: Security,
    pub smtp_host: String,
    pub smtp_port: u16,
    pub smtp_security: Security,
    /// The IMAP login, placeholders resolved.
    pub username: String,
    /// The SMTP login, placeholders resolved. Equal to `username` except where the source names another
    /// (the iCloud entry logs IMAP in with the local part and SMTP with the full address).
    pub smtp_username: String,
    pub auth_method: AuthMethod,
    pub source: Source,
}

/// The guess found an IMAP server and no sending server: Manual setup opens with the IMAP half filled.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImapOnlyConfig {
    pub imap_host: String,
    pub imap_port: u16,
    pub imap_security: Security,
    pub username: String,
    pub source: Source,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Refusal {
    InvalidAddress(String),
    NoNetwork,
    DomainNotFound(String),
    NothingAnswered(String),
}

impl Refusal {
    /// The line the Add account screen shows above Manual setup.
    pub fn words(&self) -> String {
        match self {
            Self::InvalidAddress(reason) => format!("This address cannot be used: {reason}."),
            Self::NoNetwork => "This Mac is not connected to the internet, so the mail settings cannot be looked up. Connect and try again, or use Manual setup.".into(),
            Self::DomainNotFound(d) => format!("The domain {d} does not exist. Check the address for a typo."),
            Self::NothingAnswered(d) => format!(
                "No mail settings were found for {d}: no provider database entry, no autoconfig file, no DNS SRV record, no Autodiscover answer, no mail record of a known provider, and no server answered on the usual ports. Use Manual setup."
            ),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    Found(MailConfig),
    ImapOnly(ImapOnlyConfig),
    Refused(Refusal),
}

/// The five steps, in the owner's order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Step { Ispdb, AutoconfigUrls, Srv, Autodiscover, MailRecord, Guess }

impl Step {
    /// The progress line in words naming the step (J4.1).
    pub fn words(self) -> &'static str {
        match self {
            Self::Ispdb => "Looking in the provider database…",
            Self::AutoconfigUrls => "Asking the domain for its mail settings…",
            Self::Srv => "Reading the domain's DNS mail records…",
            Self::Autodiscover => "Asking Microsoft Autodiscover…",
            Self::MailRecord => "Reading the domain's mail record…",
            Self::Guess => "Trying the usual server names…",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DetectionResult {
    pub outcome: Outcome,
    /// Every step that ran, in the order it ran; the last one answered or refused.
    pub steps_tried: Vec<Step>,
    /// Autodiscover named the domain a Microsoft 365 tenant (J4.3: "Sign in with Microsoft: coming soon").
    pub microsoft_365_tenant: bool,
    /// The provider behind the configuration, read from its IMAP host; None for a refusal or an unknown host.
    pub provider: Option<Provider>,
    /// No password can work: the account needs a sign-in (Microsoft 365, SPEC A77).
    pub sign_in_needed: bool,
}
