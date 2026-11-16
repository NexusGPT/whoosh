//! Delta 8 invariants that live in the types.
use contracts::*;

fn step(kind: ValidationStepKind, ok: bool, words: &str) -> ValidationStep { ValidationStep { kind, ok, words: words.into() } }

fn config() -> AccountConfig {
    AccountConfig {
        imap_host: "imap.mail.me.com".into(), imap_port: 993, imap_security: ConnectionSecurity::Ssl,
        smtp_host: "smtp.mail.me.com".into(), smtp_port: 587, smtp_security: ConnectionSecurity::Starttls,
        username: "assem.fixture".into(), smtp_username: "assem.fixture@icloud.com".into(),
        auth_method: AuthMethod::Password, source: DetectionSource::Ispdb,
    }
}

// ---- group 1: add account

#[test]
fn a_detection_result_derives_the_step_it_reached() {
    let progress = vec![
        DetectionProgress { step: DetectionStep::Ispdb, found: false, words: DetectionStep::Ispdb.words().into() },
        DetectionProgress { step: DetectionStep::AutoconfigUrls, found: false, words: DetectionStep::AutoconfigUrls.words().into() },
        DetectionProgress { step: DetectionStep::Srv, found: true, words: DetectionStep::Srv.words().into() },
    ];
    let r = DetectionResult::new(DetectionOutcome::Found { config: config() }, progress, false, None);
    assert_eq!(r.step_reached, Some(DetectionStep::Srv));
    let refused = DetectionResult::new(DetectionOutcome::Refused { words: "This address cannot be used: no @.".into() }, vec![], false, None);
    assert_eq!(refused.step_reached, None, "an address refused before any step reached no step");
}

#[test]
fn a_validation_is_ok_only_when_every_step_passed_and_the_folders_were_listed() {
    let all = vec![
        step(ValidationStepKind::ImapConnect, true, "connected"), step(ValidationStepKind::ImapLogin, true, "signed in"),
        step(ValidationStepKind::ImapList, true, "5 folders"), step(ValidationStepKind::SmtpEhlo, true, "ehlo"), step(ValidationStepKind::SmtpAuth, true, "auth"),
    ];
    assert!(ValidationReport::from_steps(all.clone(), vec![], 5, Some("imap.mail.me.com".into())).ok);
    assert_eq!(ValidationReport::from_steps(all, vec![], 5, Some("imap.mail.me.com".into())).refused_by, None);
    // signed in but never listed: not an account yet
    let no_list = vec![step(ValidationStepKind::ImapConnect, true, "connected"), step(ValidationStepKind::ImapLogin, true, "signed in")];
    assert!(!ValidationReport::from_steps(no_list, vec![], 0, None).ok);
    // refused at login: the words are the failing step's own, the host is named
    let refused = vec![step(ValidationStepKind::ImapConnect, true, "connected"), step(ValidationStepKind::ImapLogin, false, "imap.mail.me.com refused the sign-in: wrong password.")];
    let r = ValidationReport::from_steps(refused, vec![], 0, Some("imap.mail.me.com".into()));
    assert!(!r.ok);
    assert_eq!(r.refused_by.as_deref(), Some("imap.mail.me.com"));
    assert!(r.summary_words.contains("wrong password"));
}

#[test]
fn the_password_of_an_add_account_command_never_reaches_a_log_line() {
    let pw = "app-specific-0001-xxxx";
    let command = Command::AddAccountConfigured { address: "assem.fixture@icloud.com".into(), display_name: None, config: config(), password: SecretText { value: pw.into() } };
    let shown = format!("{command:?}");
    assert!(!shown.contains(pw), "the password is printed: {shown}");
    assert!(shown.contains("imap.mail.me.com"));
}
