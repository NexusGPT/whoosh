//! The crates' own shapes into the contract's: detection (autoconfig) and validation (sync-imap).
use contracts::{AccountConfig, AuthMethod, ConnectionSecurity, DetectedProvider, DetectionOutcome, DetectionProgress, DetectionResult, DetectionSource, DetectionStep,
    FolderRoleMapping, ValidationReport, ValidationStep, ValidationStepKind};

pub fn step(s: autoconfig::Step) -> DetectionStep {
    match s {
        autoconfig::Step::Ispdb => DetectionStep::Ispdb,
        autoconfig::Step::AutoconfigUrls => DetectionStep::AutoconfigUrls,
        autoconfig::Step::Srv => DetectionStep::Srv,
        autoconfig::Step::Autodiscover => DetectionStep::Autodiscover,
        autoconfig::Step::MailRecord => DetectionStep::MailRecord,
        autoconfig::Step::Guess => DetectionStep::Guess,
    }
}
fn source(s: autoconfig::Source) -> DetectionSource {
    match s {
        autoconfig::Source::Ispdb => DetectionSource::Ispdb,
        autoconfig::Source::Autoconfig => DetectionSource::Autoconfig,
        autoconfig::Source::Srv => DetectionSource::Srv,
        autoconfig::Source::Autodiscover => DetectionSource::Autodiscover,
        autoconfig::Source::MailRecord => DetectionSource::MailRecord,
        autoconfig::Source::Guess => DetectionSource::Guess,
    }
}
fn security(s: autoconfig::Security) -> ConnectionSecurity {
    match s { autoconfig::Security::Ssl => ConnectionSecurity::Ssl, autoconfig::Security::Starttls => ConnectionSecurity::Starttls }
}
fn provider(p: autoconfig::Provider) -> DetectedProvider {
    match p {
        autoconfig::Provider::Microsoft365 => DetectedProvider::Microsoft365,
        autoconfig::Provider::Google => DetectedProvider::Google,
        autoconfig::Provider::Icloud => DetectedProvider::Icloud,
        autoconfig::Provider::Fastmail => DetectedProvider::Fastmail,
        autoconfig::Provider::ProtonBridge => DetectedProvider::ProtonBridge,
    }
}

/// The detector's result as Add account reads it: the outcome, one progress line per step that ran (the last one is the
/// one that answered, when one did), the Microsoft 365 tenant, the provider and whether a sign-in is needed.
pub fn detection(r: autoconfig::DetectionResult) -> DetectionResult {
    let answered = !matches!(r.outcome, autoconfig::Outcome::Refused(_));
    let n = r.steps_tried.len();
    let progress = r.steps_tried.iter().enumerate().map(|(i, s)| {
        let s = step(*s);
        DetectionProgress { step: s, found: answered && i + 1 == n, words: s.words().to_string() }
    }).collect();
    let outcome = match r.outcome {
        autoconfig::Outcome::Found(c) => DetectionOutcome::Found { config: AccountConfig {
            imap_host: c.imap_host, imap_port: c.imap_port, imap_security: security(c.imap_security), smtp_host: c.smtp_host, smtp_port: c.smtp_port,
            smtp_security: security(c.smtp_security), username: c.username, smtp_username: c.smtp_username,
            auth_method: match c.auth_method { autoconfig::AuthMethod::Password => AuthMethod::Password, autoconfig::AuthMethod::Oauth2 => AuthMethod::Oauth2 },
            source: source(c.source),
        } },
        autoconfig::Outcome::ImapOnly(c) => DetectionOutcome::ImapOnly { imap_host: c.imap_host, imap_port: c.imap_port, imap_security: security(c.imap_security), username: c.username, source: source(c.source) },
        autoconfig::Outcome::Refused(why) => DetectionOutcome::Refused { words: why.words() },
    };
    DetectionResult::new(outcome, progress, r.microsoft_365_tenant, None).with_sign_in(r.provider.map(provider), r.sign_in_needed)
}

fn kind(s: sync_imap::Step) -> ValidationStepKind {
    match s {
        sync_imap::Step::ImapConnect => ValidationStepKind::ImapConnect,
        sync_imap::Step::ImapTls => ValidationStepKind::ImapTls,
        sync_imap::Step::ImapLogin => ValidationStepKind::ImapLogin,
        sync_imap::Step::ImapList => ValidationStepKind::ImapList,
        sync_imap::Step::SmtpConnect => ValidationStepKind::SmtpEhlo,
        sync_imap::Step::SmtpLogin => ValidationStepKind::SmtpAuth,
    }
}
fn role_word(r: sync_imap::FolderRole) -> &'static str {
    use sync_imap::FolderRole::*;
    match r { Inbox => "inbox", Sent => "sent", Drafts => "drafts", Trash => "trash", Junk => "junk", Archive => "archive", All => "all", Flagged => "flagged", Other => "other" }
}

/// The sign-in check as the J4.2 screen reads it; the failing host is the server of the first failed step.
pub fn validation(r: sync_imap::ValidationReport, config: &AccountConfig) -> ValidationReport {
    let failing = r.first_failure().map(|f| match f.step {
        sync_imap::Step::SmtpConnect | sync_imap::Step::SmtpLogin => config.smtp_host.clone(),
        _ => config.imap_host.clone(),
    });
    let steps = r.steps.iter().map(|s| ValidationStep { kind: kind(s.step), ok: s.ok, words: s.words.clone() }).collect();
    let roles = r.roles.iter().map(|m| FolderRoleMapping { role: role_word(m.role).to_string(), folder: m.folder.clone(), by_special_use: m.by == sync_imap::RoleBy::SpecialUse }).collect();
    ValidationReport::from_steps(steps, roles, r.folders.len() as u32, failing)
}
