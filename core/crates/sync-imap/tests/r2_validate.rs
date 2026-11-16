//! Round 2, validation: a real sign-in and LIST (and SMTP EHLO + auth) before an account is saved.
//! The servers are the in-process fakes; each failure must come back as words the owner can act on.
mod support;

use support::smtp::FakeSmtp;
use support::*;
use sync_imap::*;

const USER: &str = "owner@example.test";

fn vcfg(port: u16, security: Security, roots: Vec<Vec<u8>>) -> ValidationConfig {
    ValidationConfig { username: USER.into(), imap: ImapEndpoint { host: "127.0.0.1".into(), port, security }, smtp: None, extra_roots_der: roots }
}
fn pw() -> Credential { Credential::Password("app-password".into()) }
fn step(r: &ValidationReport, s: Step) -> Option<&StepResult> { r.steps.iter().find(|x| x.step == s) }

#[tokio::test(flavor = "multi_thread")]
async fn a_good_account_passes_every_step_and_lists_its_roles() {
    let srv = FakeImap::start().await;
    let r = validate(&vcfg(srv.port, Security::PlainLoopbackOnly, vec![]), &pw()).await;
    assert!(r.ok(), "{:?}", r.steps);
    assert_eq!(r.steps.iter().map(|s| s.step).collect::<Vec<_>>(), vec![Step::ImapConnect, Step::ImapTls, Step::ImapLogin, Step::ImapList]);
    assert!(step(&r, Step::ImapList).unwrap().words.contains("6 folders"));
    for cap in ["IDLE", "CONDSTORE", "QRESYNC", "SPECIAL-USE"] { assert!(r.capabilities.iter().any(|c| c == cap), "{cap} missing in {:?}", r.capabilities); }
    let sent = r.roles.iter().find(|m| m.role == FolderRole::Sent).expect("a Sent role");
    assert_eq!((sent.folder.as_str(), sent.by), ("Sent", RoleBy::SpecialUse));
    assert_eq!(r.folders.len(), 6);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_refused_login_comes_back_in_the_servers_words_and_nothing_after_it_runs() {
    let srv = FakeImap::start().await;
    srv.with(|s| s.refuse_login = Some("Application-specific password required".into()));
    let r = validate(&vcfg(srv.port, Security::PlainLoopbackOnly, vec![]), &pw()).await;
    assert!(!r.ok());
    let f = r.first_failure().expect("a failed step");
    assert_eq!(f.step, Step::ImapLogin);
    assert!(f.words.contains("Application-specific password required"), "{}", f.words);
    assert!(step(&r, Step::ImapList).is_none(), "LIST must not run after a refused login");
}

#[tokio::test(flavor = "multi_thread")]
async fn an_unreachable_host_is_named_with_its_port() {
    let port = { let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap(); l.local_addr().unwrap().port() };
    let r = validate(&vcfg(port, Security::PlainLoopbackOnly, vec![]), &pw()).await;
    let f = r.first_failure().expect("a failed step");
    assert_eq!(f.step, Step::ImapConnect);
    assert!(f.words.contains("Cannot reach 127.0.0.1") && f.words.contains(&port.to_string()), "{}", f.words);
}

#[tokio::test(flavor = "multi_thread")]
async fn tls_asked_of_a_plain_port_fails_at_the_tls_step() {
    // The STARTTLS fake greets in clear text, so a client that expects TLS from the first byte must fail.
    let srv = FakeImap::start_secure(TlsMode::StartTls).await;
    let r = validate(&vcfg(srv.port, Security::Tls, vec![test_ca_der()]), &pw()).await;
    let f = r.first_failure().expect("a failed step");
    assert_eq!(f.step, Step::ImapTls);
    assert!(f.words.contains("secure connection"), "{}", f.words);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_certificate_the_system_does_not_trust_is_refused_in_words() {
    let srv = FakeImap::start_secure(TlsMode::Implicit).await;
    let r = validate(&vcfg(srv.port, Security::Tls, vec![]), &pw()).await;
    let f = r.first_failure().expect("a failed step");
    assert_eq!(f.step, Step::ImapTls);
    assert!(f.words.to_ascii_lowercase().contains("certificate"), "{}", f.words);
}

#[tokio::test(flavor = "multi_thread")]
async fn implicit_tls_with_a_trusted_certificate_signs_in() {
    let srv = FakeImap::start_secure(TlsMode::Implicit).await;
    let r = validate(&vcfg(srv.port, Security::Tls, vec![test_ca_der()]), &pw()).await;
    assert!(r.ok(), "{:?}", r.steps);
    assert!(step(&r, Step::ImapTls).unwrap().words.contains("TLS from the first byte"));
}

#[tokio::test(flavor = "multi_thread")]
async fn starttls_is_asked_for_before_the_password_is_sent() {
    let srv = FakeImap::start_secure(TlsMode::StartTls).await;
    let r = validate(&vcfg(srv.port, Security::StartTls, vec![test_ca_der()]), &pw()).await;
    assert!(r.ok(), "{:?}", r.steps);
    assert!(step(&r, Step::ImapTls).unwrap().words.contains("STARTTLS"));
    let log = srv.log();
    let at = |needle: &str| log.iter().position(|l| l.contains(needle));
    let (starttls, login) = (at("STARTTLS").expect("STARTTLS sent"), at("LOGIN").expect("LOGIN sent"));
    assert!(starttls < login, "the password went before STARTTLS: {log:?}");
}

#[tokio::test(flavor = "multi_thread")]
async fn a_server_with_login_disabled_is_signed_in_with_authenticate_plain() {
    let srv = FakeImap::start().await;
    srv.with(|s| s.login_disabled = true);
    let r = validate(&vcfg(srv.port, Security::PlainLoopbackOnly, vec![]), &pw()).await;
    assert!(r.ok(), "{:?}", r.steps);
    let log = srv.log();
    assert!(log.iter().any(|l| l.contains("AUTHENTICATE PLAIN")), "{log:?}");
    assert!(!log.iter().any(|l| l.contains("LOGIN ")), "{log:?}");
}

#[tokio::test(flavor = "multi_thread")]
async fn smtp_ehlo_and_sign_in_are_in_the_same_report() {
    let imap = FakeImap::start().await;
    let smtp = FakeSmtp::start(USER, "app-password").await;
    let mut cfg = vcfg(imap.port, Security::PlainLoopbackOnly, vec![]);
    cfg.smtp = Some(SmtpEndpoint { host: "127.0.0.1".into(), port: smtp.port, security: send::Security::Plaintext });
    let r = validate(&cfg, &pw()).await;
    assert!(r.ok(), "{:?}", r.steps);
    assert_eq!(r.steps.iter().map(|s| s.step).collect::<Vec<_>>(), vec![Step::ImapConnect, Step::ImapTls, Step::ImapLogin, Step::ImapList, Step::SmtpConnect, Step::SmtpLogin]);
    let log = smtp.log.lock().unwrap().clone();
    assert!(log.iter().any(|l| l.starts_with("EHLO")), "{log:?}");
    assert!(log.iter().any(|l| l.starts_with("AUTH")), "{log:?}");
}

#[tokio::test(flavor = "multi_thread")]
async fn a_refused_smtp_sign_in_is_reported_beside_a_good_imap_sign_in() {
    let imap = FakeImap::start().await;
    let smtp = FakeSmtp::start(USER, "another-password").await;
    let mut cfg = vcfg(imap.port, Security::PlainLoopbackOnly, vec![]);
    cfg.smtp = Some(SmtpEndpoint { host: "127.0.0.1".into(), port: smtp.port, security: send::Security::Plaintext });
    let r = validate(&cfg, &pw()).await;
    assert!(!r.ok());
    assert!(step(&r, Step::ImapList).unwrap().ok);
    let f = r.first_failure().unwrap();
    assert_eq!(f.step, Step::SmtpLogin);
    assert!(f.words.contains("not accepted") || f.words.contains("535"), "{}", f.words);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_token_signs_in_with_xoauth2_on_imap_and_smtp() {
    let imap = FakeImap::start().await;
    let smtp = FakeSmtp::start(USER, "token-1").await;
    let mut cfg = vcfg(imap.port, Security::PlainLoopbackOnly, vec![]);
    cfg.smtp = Some(SmtpEndpoint { host: "127.0.0.1".into(), port: smtp.port, security: send::Security::Plaintext });
    let r = validate(&cfg, &Credential::Token("token-1".into())).await;
    assert!(r.ok(), "{:?}", r.steps);
    assert!(imap.log().iter().any(|l| l.contains("AUTHENTICATE XOAUTH2")));
    let bad = validate(&cfg, &Credential::Token("stale".into())).await;
    assert_eq!(bad.first_failure().unwrap().step, Step::ImapLogin);
    assert!(bad.first_failure().unwrap().words.contains("token refused"));
}

#[tokio::test(flavor = "multi_thread")]
async fn special_use_attributes_win_over_names_and_names_fill_in_without_them() {
    // Names no heuristic knows: only the RFC 6154 attributes can find the roles.
    let srv = FakeImap::start().await;
    srv.with(|s| for (i, n) in ["INBOX", "Gesendet", "Entwuerfe", "Papierkorb", "Werbung", "Zeitungen"].iter().enumerate() { s.boxes[i].name = (*n).into(); });
    let r = validate(&vcfg(srv.port, Security::PlainLoopbackOnly, vec![]), &pw()).await;
    let by = |role| r.roles.iter().find(|m| m.role == role).map(|m| (m.folder.clone(), m.by));
    assert_eq!(by(FolderRole::Sent), Some(("Gesendet".into(), RoleBy::SpecialUse)));
    assert_eq!(by(FolderRole::Drafts), Some(("Entwuerfe".into(), RoleBy::SpecialUse)));
    assert_eq!(by(FolderRole::Trash), Some(("Papierkorb".into(), RoleBy::SpecialUse)));
    assert_eq!(by(FolderRole::Junk), Some(("Werbung".into(), RoleBy::SpecialUse)));

    // No SPECIAL-USE at all: the common names decide.
    let plain = FakeImap::start_with(false).await;
    plain.with(|s| s.cap_special_use = false);
    let r = validate(&vcfg(plain.port, Security::PlainLoopbackOnly, vec![]), &pw()).await;
    let by = |role| r.roles.iter().find(|m| m.role == role).map(|m| (m.folder.clone(), m.by));
    assert_eq!(by(FolderRole::Sent), Some(("Sent Messages".into(), RoleBy::Name)));
    assert_eq!(by(FolderRole::Trash), Some(("Deleted Messages".into(), RoleBy::Name)));
    assert!(!r.capabilities.iter().any(|c| c == "SPECIAL-USE"));

    // A LIST with no Sent folder by attribute or name: the report simply has no Sent role.
    let bare = FakeImap::start_with(false).await;
    bare.with(|s| s.boxes[1].name = "Zzz".into());
    let r = validate(&vcfg(bare.port, Security::PlainLoopbackOnly, vec![]), &pw()).await;
    assert!(r.ok());
    assert!(r.roles.iter().all(|m| m.role != FolderRole::Sent));
}
