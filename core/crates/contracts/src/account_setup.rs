//! Add account by IMAP (delta 8, group 1): the shapes the Add account screen reads while it detects, checks and saves an
//! account (docs/JOURNEYS.md J4.1 to J4.5, docs/DATA-MAP.md "Add account by IMAP").
//!
//! The password is a `SecretText` everywhere it crosses: it goes to the Keychain through the secrets crate's port and is
//! never in the account table, a state path or a log line. `AccountConfig` holds only what the account table keeps.

/// The wire security of one server: implicit TLS, or STARTTLS upgrade.
#[derive(uniffi::Enum, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ConnectionSecurity { Ssl, Starttls }

/// How the account signs in. `Oauth2` (delta 14) is the person's own app registration: `SetOAuthClient`, then
/// `StartOAuthSignIn` (the owner: no vendor client ID ships, ever).
#[derive(uniffi::Enum, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AuthMethod { Password, Oauth2 }

/// The provider detection named (delta 14, core-autoconfig r2's table). Not `MailProvider`, the account's own field:
/// detection also knows Proton Mail Bridge, a local bridge with no account kind of its own.
#[derive(uniffi::Enum, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DetectedProvider { Microsoft365, Google, Icloud, Fastmail, ProtonBridge }

/// Where a configuration came from. `Manual` = typed by the owner on the Manual setup form.
#[derive(uniffi::Enum, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DetectionSource { Ispdb, Autoconfig, Srv, Autodiscover, MailRecord, Guess, Manual }

/// A complete server configuration: the account table's server columns (migration 0003).
#[derive(uniffi::Record, Debug, Clone, PartialEq, Eq)]
pub struct AccountConfig {
    pub imap_host: String,
    pub imap_port: u16,
    pub imap_security: ConnectionSecurity,
    pub smtp_host: String,
    pub smtp_port: u16,
    pub smtp_security: ConnectionSecurity,
    /// the IMAP login, placeholders resolved
    pub username: String,
    /// the SMTP login; equal to `username` except where the source names another (iCloud: local part for IMAP, full address for SMTP)
    pub smtp_username: String,
    pub auth_method: AuthMethod,
    pub source: DetectionSource,
}

/// The detection steps in the owner's order (J4.1); the mail record comes before the guess (delta 14, core-autoconfig r2).
#[derive(uniffi::Enum, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DetectionStep { Ispdb, AutoconfigUrls, Srv, Autodiscover, MailRecord, Guess }

impl DetectionStep {
    /// The progress line naming the step (J4.1).
    pub fn words(self) -> &'static str {
        match self {
            Self::Ispdb => "Looking in the provider database…",
            Self::AutoconfigUrls => "Asking the domain for its mail settings…",
            Self::Srv => "Reading the domain's DNS mail records…",
            Self::Autodiscover => "Asking Microsoft Autodiscover…",
            // delta 14 (core-autoconfig r2): the domain's MX record names the provider (Microsoft 365, Google, ...)
            Self::MailRecord => "Reading the domain's mail record…",
            Self::Guess => "Trying the usual server names…",
        }
    }
}

/// One step of a detection: what ran, whether it answered, the line the screen shows.
#[derive(uniffi::Record, Debug, Clone, PartialEq, Eq)]
pub struct DetectionProgress { pub step: DetectionStep, pub found: bool, pub words: String }

/// What detection concluded. A configuration or a refusal in words, never both and never neither.
#[derive(uniffi::Enum, Debug, Clone, PartialEq, Eq)]
pub enum DetectionOutcome {
    Found { config: AccountConfig },
    /// The guess found an IMAP server and no sending server: Manual setup opens with the IMAP half filled.
    ImapOnly { imap_host: String, imap_port: u16, imap_security: ConnectionSecurity, username: String, source: DetectionSource },
    /// Nothing usable: the line the screen shows above Manual setup.
    Refused { words: String },
}

#[derive(uniffi::Record, Debug, Clone, PartialEq, Eq)]
pub struct DetectionResult {
    pub outcome: DetectionOutcome,
    /// The last step that ran; None when the address was refused before any step.
    pub step_reached: Option<DetectionStep>,
    /// Every step that ran, in order.
    pub progress: Vec<DetectionProgress>,
    /// Autodiscover named the domain a Microsoft 365 tenant (J4.3).
    pub microsoft_365_tenant: bool,
    /// "Sign in with Google: coming soon" when the provider is OAuth2-only; None otherwise.
    pub sign_in_coming_soon: Option<String>,
    /// The provider the address belongs to, when detection knows it (delta 14; core-autoconfig r2 fills it from the
    /// configuration's host by its provider table). None = a plain IMAP host. The default keeps Swift callers compiling.
    #[uniffi(default = None)]
    pub provider: Option<DetectedProvider>,
    /// The provider signs in by OAuth2 only for this address, so Add account asks for the person's own client ID
    /// (`SetOAuthClient`) and opens the sign-in (`StartOAuthSignIn`) instead of a password (delta 14).
    #[uniffi(default = false)]
    pub sign_in_needed: bool,
}

impl DetectionResult {
    /// `step_reached` is derived from `progress`, never stated twice. (UniFFI exports no record constructor; Swift builds the same
    /// value with `DetectionResult.make` in Sources/CoreBridge/FixtureAddAccount.swift, proven equal by RootDelta8Tests.)
    pub fn new(outcome: DetectionOutcome, progress: Vec<DetectionProgress>, microsoft_365_tenant: bool, sign_in_coming_soon: Option<String>) -> Self {
        let step_reached = progress.last().map(|p| p.step);
        DetectionResult { outcome, step_reached, progress, microsoft_365_tenant, sign_in_coming_soon, provider: None, sign_in_needed: false }
    }

    /// The same result with the provider and whether it needs an OAuth2 sign-in (delta 14). `sign_in_needed` is kept only for
    /// a provider that signs in by OAuth2 (`oauth_provider_for`), so a plain IMAP host never asks for a client ID.
    pub fn with_sign_in(mut self, provider: Option<DetectedProvider>, sign_in_needed: bool) -> Self {
        self.sign_in_needed = sign_in_needed && provider.and_then(crate::oauth_provider_for_detected).is_some();
        self.provider = provider;
        self
    }
}

/// The steps of a validation (J4.2). SMTP "ehlo" and "auth" are the sending half.
#[derive(uniffi::Enum, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ValidationStepKind { ImapConnect, ImapTls, ImapLogin, ImapList, SmtpEhlo, SmtpAuth }

#[derive(uniffi::Record, Debug, Clone, PartialEq, Eq)]
pub struct ValidationStep { pub kind: ValidationStepKind, pub ok: bool, pub words: String }

/// One folder the server's list mapped to a role ("sent" -> "Sent Mail"), by SPECIAL-USE or by name.
#[derive(uniffi::Record, Debug, Clone, PartialEq, Eq)]
pub struct FolderRoleMapping { pub role: String, pub folder: String, pub by_special_use: bool }

/// The outcome of checking a configuration with a real login (J4.2). `ok` and `refused_by` are derived from `steps` by
/// `from_steps`; build the report there and the two cannot disagree with the steps.
#[derive(uniffi::Record, Debug, Clone, PartialEq, Eq)]
pub struct ValidationReport {
    pub ok: bool,
    pub steps: Vec<ValidationStep>,
    pub roles: Vec<FolderRoleMapping>,
    pub folder_count: u32,
    /// The host that refused, in the failing step's own words; None when every step passed.
    pub refused_by: Option<String>,
    /// One line for the screen: what passed, or which server refused and why. A refusal keeps the server's RAW text here; the
    /// sheet shows `cause`'s plain sentence first and this text behind "Details" (SPEC A87).
    pub summary_words: String,
    /// Why the check failed, as one known cause (SPEC A87, 2026-11-09); None when every step passed. Additive: a writer
    /// built before A87 leaves it None and the screen falls back to the raw words.
    #[uniffi(default = None)]
    pub cause: Option<RefusalCause>,
}

/// The known reasons a sign-in check fails (SPEC A87). Each has one plain sentence in the catalogue
/// (`AddAccountWords.causeSentence`); the server's own text stays in `ValidationReport.summary_words`.
#[derive(uniffi::Enum, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RefusalCause {
    /// The provider turned off password sign-in (Microsoft 365: "Basic authentication is disabled"): use the app ID.
    BasicAuthDisabled,
    /// The incoming server refused the password.
    BadPassword,
    /// A server did not answer (connect, or the sending server's greeting).
    HostUnreachable,
    /// The secure connection could not be made.
    TlsFailed,
    /// The sending server refused the sign-in.
    SmtpRefused,
    /// Any other refusal: the raw text is all there is.
    Unknown,
}

/// The cause id the screen and the checks read ("basic_auth_disabled", ...). Total over the enum: a new cause is a build error.
#[uniffi::export]
pub fn refusal_cause_id(cause: RefusalCause) -> String {
    match cause {
        RefusalCause::BasicAuthDisabled => "basic_auth_disabled",
        RefusalCause::BadPassword => "bad_password",
        RefusalCause::HostUnreachable => "host_unreachable",
        RefusalCause::TlsFailed => "tls_failed",
        RefusalCause::SmtpRefused => "smtp_refused",
        RefusalCause::Unknown => "unknown",
    }.to_string()
}

/// The phrases providers use when password sign-in is switched off (lower case). Microsoft: "Basic authentication is
/// disabled" (IMAP, the owner's 14:38 screen) and SMTP's 5.7.139 "basic authentication is disabled"; Gmail: "Web login
/// required" / "Application-specific password required" mean an app password, which is a refused password, not this.
const BASIC_AUTH_OFF: [&str; 3] = ["basic authentication is disabled", "basicauthblocked", "smtpclientauthentication is disabled"];

/// The cause of the FIRST failed step (steps after a failure never run): by the step, then by the server's words.
/// None when no step failed.
#[uniffi::export]
pub fn refusal_cause(steps: Vec<ValidationStep>) -> Option<RefusalCause> {
    let f = steps.iter().find(|s| !s.ok)?;
    let w = f.words.to_lowercase();
    if BASIC_AUTH_OFF.iter().any(|p| w.contains(p)) { return Some(RefusalCause::BasicAuthDisabled); }
    Some(match f.kind {
        ValidationStepKind::ImapConnect | ValidationStepKind::SmtpEhlo => RefusalCause::HostUnreachable,
        ValidationStepKind::ImapTls => RefusalCause::TlsFailed,
        ValidationStepKind::ImapLogin => RefusalCause::BadPassword,
        ValidationStepKind::SmtpAuth => RefusalCause::SmtpRefused,
        ValidationStepKind::ImapList => RefusalCause::Unknown,
    })
}

impl ValidationReport {
    /// (Swift twin: `ValidationReport.fromSteps` in Sources/CoreBridge/FixtureAddAccount.swift.)
    /// ok = every step that ran passed AND the folder list was read (a login alone is not an account).
    /// A step after a failed one does not run, so the report holds steps up to the first failure.
    pub fn from_steps(steps: Vec<ValidationStep>, roles: Vec<FolderRoleMapping>, folder_count: u32, failing_host: Option<String>) -> Self {
        let ok = steps.iter().all(|s| s.ok) && steps.iter().any(|s| s.kind == ValidationStepKind::ImapList);
        let failure = steps.iter().find(|s| !s.ok);
        let refused_by = if ok { None } else { failing_host };
        let summary_words = match (ok, failure) {
            (true, _) => format!("The mail server and the sending server both answered. {folder_count} folders found."),
            (false, Some(f)) => f.words.clone(),
            (false, None) => "The folder list was not read, so the account cannot be added yet.".into(),
        };
        let cause = if ok { None } else { refusal_cause(steps.clone()).or(Some(RefusalCause::Unknown)) };
        ValidationReport { ok, steps, roles, folder_count, refused_by, summary_words, cause }
    }
}
