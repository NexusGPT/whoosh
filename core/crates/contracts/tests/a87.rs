//! SPEC A87 (2026-11-09): a failed sign-in check names one known cause; the server's raw text stays in summary_words.
//! Each case below was seen red with its classifier line broken (docs/lanes/contracts.md "## A87").
use contracts::*;

fn step(kind: ValidationStepKind, ok: bool, words: &str) -> ValidationStep { ValidationStep { kind, ok, words: words.into() } }
fn ok(kind: ValidationStepKind) -> ValidationStep { step(kind, true, "fine") }
fn failed_at(kind: ValidationStepKind, words: &str) -> ValidationReport {
    use ValidationStepKind::*;
    let order = [ImapConnect, ImapTls, ImapLogin, ImapList, SmtpEhlo, SmtpAuth];
    let mut steps: Vec<ValidationStep> = order.iter().take_while(|k| **k != kind).map(|k| ok(*k)).collect();
    steps.push(step(kind, false, words));
    ValidationReport::from_steps(steps, vec![], 0, Some("host.example".into()))
}

#[test]
fn the_owners_microsoft_refusal_is_basic_auth_disabled_and_keeps_the_raw_text() {
    let raw = "The server refused the sign-in: code: None, info: Some(\"Basic authentication is disabled.\")";
    let r = failed_at(ValidationStepKind::ImapLogin, raw);
    assert_eq!(r.cause, Some(RefusalCause::BasicAuthDisabled));
    assert_eq!(r.summary_words, raw, "the raw server text is kept for Details");
    let smtp = failed_at(ValidationStepKind::SmtpAuth, "535 5.7.139 Authentication unsuccessful, basic authentication is disabled.");
    assert_eq!(smtp.cause, Some(RefusalCause::BasicAuthDisabled), "the SMTP form of the same refusal");
}

#[test]
fn each_failing_step_maps_to_its_cause() {
    use ValidationStepKind::*;
    assert_eq!(failed_at(ImapLogin, "Invalid credentials (Failure)").cause, Some(RefusalCause::BadPassword));
    assert_eq!(failed_at(ImapLogin, "Application-specific password required").cause, Some(RefusalCause::BadPassword));
    assert_eq!(failed_at(ImapConnect, "Cannot reach imap.example on port 993").cause, Some(RefusalCause::HostUnreachable));
    assert_eq!(failed_at(SmtpEhlo, "smtp.example did not answer").cause, Some(RefusalCause::HostUnreachable));
    assert_eq!(failed_at(ImapTls, "The secure connection failed").cause, Some(RefusalCause::TlsFailed));
    assert_eq!(failed_at(SmtpAuth, "535 5.7.8 Authentication credentials invalid").cause, Some(RefusalCause::SmtpRefused));
    assert_eq!(failed_at(ImapList, "LIST failed").cause, Some(RefusalCause::Unknown));
}

#[test]
fn a_passed_check_has_no_cause_and_a_report_that_never_listed_is_unknown() {
    use ValidationStepKind::*;
    let all: Vec<ValidationStep> = [ImapConnect, ImapTls, ImapLogin, ImapList, SmtpEhlo, SmtpAuth].iter().map(|k| ok(*k)).collect();
    assert_eq!(ValidationReport::from_steps(all, vec![], 3, None).cause, None);
    let unlisted = vec![ok(ImapConnect), ok(ImapLogin)];
    assert_eq!(ValidationReport::from_steps(unlisted, vec![], 0, None).cause, Some(RefusalCause::Unknown));
}

#[test]
fn every_cause_has_its_id() {
    let ids: Vec<String> = [RefusalCause::BasicAuthDisabled, RefusalCause::BadPassword, RefusalCause::HostUnreachable, RefusalCause::TlsFailed, RefusalCause::SmtpRefused, RefusalCause::Unknown]
        .into_iter().map(refusal_cause_id).collect();
    assert_eq!(ids, ["basic_auth_disabled", "bad_password", "host_unreachable", "tls_failed", "smtp_refused", "unknown"]);
}
