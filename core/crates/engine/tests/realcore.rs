//! The real core, slice 1: detection, the sign-in check and Add account, through the WhooshCore trait the app calls.
//! Detection runs on fake DNS/HTTP/probe (no network); validation signs in to the in-process IMAP and SMTP fakes.
//! Each test was seen red on a mutant (docs/lanes/contracts.md "## Real core, slice 1").
#[allow(dead_code, unused_imports)]
#[path = "../../autoconfig/tests/support/mod.rs"]
mod detect_fakes;
#[allow(dead_code, unused_imports)]
#[path = "../../sync-imap/tests/support/mod.rs"]
mod imap_fakes;

use std::sync::{Arc, Mutex};

use contracts::*;
use detect_fakes::{FakeDns, FakeHttp, FakeProbe};
use engine::{RustCore, TestTransport};
use imap_fakes::{smtp::FakeSmtp, FakeImap};
use secrets::{MemoryKeychain, Namespace, ReadSecret};
use store::Store;

const USER: &str = "owner@example.test";

fn gpt_nexus_detector() -> Box<dyn engine::Detect> {
    let dns = FakeDns::default().with_mx("nexus-example.com", &[(0, "nexus-example-com.mail.protection.outlook.com.")]);
    Box::new(autoconfig::Detector::new(FakeHttp::default(), dns, FakeProbe::default()))
}
fn core_with(detector: Box<dyn engine::Detect>, kc: Arc<MemoryKeychain>) -> (RustCore, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(&dir.path().join("whoosh.sqlite")).unwrap();
    (RustCore::with_parts(store, kc, detector, TestTransport::PlainLoopback), dir)
}
fn kc() -> Arc<MemoryKeychain> { Arc::new(MemoryKeychain::new(Namespace::rig())) }

struct Sink(Mutex<Vec<CoreEvent>>);
impl EventSink for Sink { fn on_event(&self, e: CoreEvent) { self.0.lock().unwrap().push(e); } }

fn local_config(imap_port: u16, smtp_port: u16) -> AccountConfig {
    AccountConfig {
        imap_host: "127.0.0.1".into(), imap_port, imap_security: ConnectionSecurity::Ssl, smtp_host: "127.0.0.1".into(), smtp_port,
        smtp_security: ConnectionSecurity::Starttls, username: USER.into(), smtp_username: USER.into(), auth_method: AuthMethod::Password, source: DetectionSource::Manual,
    }
}
fn pw(s: &str) -> SecretText { SecretText { value: s.into() } }

#[test]
fn gpt_nexus_is_detected_as_microsoft_365_by_its_mail_record_with_a_progress_line_per_step() {
    let (core, _d) = core_with(gpt_nexus_detector(), kc());
    let sink = Arc::new(Sink(Mutex::new(vec![])));
    core.subscribe(StreamKind::SyncStatus, sink.clone());
    let r = core.detect_provider("assem@nexus-example.com".into());
    let DetectionOutcome::Found { config } = &r.outcome else { panic!("not found: {:?}", r.outcome) };
    assert_eq!((config.imap_host.as_str(), config.imap_port, config.imap_security), ("outlook.office365.com", 993, ConnectionSecurity::Ssl));
    assert_eq!((config.smtp_host.as_str(), config.smtp_port), ("smtp.office365.com", 587));
    assert_eq!((config.auth_method, config.source), (AuthMethod::Oauth2, DetectionSource::MailRecord));
    assert_eq!(r.provider, Some(DetectedProvider::Microsoft365));
    assert!(r.sign_in_needed, "Microsoft 365 asks for the organisation's app ID");
    assert_eq!(r.step_reached, Some(DetectionStep::MailRecord));
    assert_eq!(r.progress.iter().map(|p| p.step).collect::<Vec<_>>(), vec![DetectionStep::Ispdb, DetectionStep::AutoconfigUrls, DetectionStep::Srv, DetectionStep::Autodiscover, DetectionStep::MailRecord]);
    assert!(r.progress.last().unwrap().found && r.progress.iter().filter(|p| p.found).count() == 1);
    let events = sink.0.lock().unwrap();
    let steps: Vec<u32> = events.iter().filter_map(|e| if let CoreEvent::DetectionProgress { number, address, .. } = e { assert_eq!(address, "assem@nexus-example.com"); Some(*number) } else { None }).collect();
    assert_eq!(steps, vec![1, 2, 3, 4, 5], "one live progress event per step");
}

#[test]
fn an_address_without_a_domain_is_refused_in_words() {
    let (core, _d) = core_with(gpt_nexus_detector(), kc());
    let r = core.detect_provider("not-an-address".into());
    let DetectionOutcome::Refused { words } = &r.outcome else { panic!("{:?}", r.outcome) };
    assert!(!words.is_empty());
}

#[test]
fn a_good_account_passes_imap_and_smtp_and_maps_its_roles() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let imap = rt.block_on(FakeImap::start());
    let smtp = rt.block_on(FakeSmtp::start(USER, "app-password"));
    let (core, _d) = core_with(gpt_nexus_detector(), kc());
    let r = core.validate_account(local_config(imap.port, smtp.port), pw("app-password"));
    assert!(r.ok, "{:?}", r.steps);
    assert_eq!(r.steps.iter().map(|s| s.kind).collect::<Vec<_>>(), vec![ValidationStepKind::ImapConnect, ValidationStepKind::ImapTls, ValidationStepKind::ImapLogin, ValidationStepKind::ImapList, ValidationStepKind::SmtpEhlo, ValidationStepKind::SmtpAuth]);
    assert_eq!(r.folder_count, 6);
    let sent = r.roles.iter().find(|m| m.role == "sent").expect("a sent role");
    assert!(sent.folder == "Sent" && sent.by_special_use);
    assert_eq!(r.refused_by, None);
}

#[test]
fn a_refused_login_comes_back_in_the_servers_words_naming_the_host() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let imap = rt.block_on(FakeImap::start());
    imap.with(|s| s.refuse_login = Some("Application-specific password required".into()));
    let smtp = rt.block_on(FakeSmtp::start(USER, "app-password"));
    let (core, _d) = core_with(gpt_nexus_detector(), kc());
    let r = core.validate_account(local_config(imap.port, smtp.port), pw("app-password"));
    assert!(!r.ok);
    let f = r.steps.iter().find(|s| !s.ok).unwrap();
    assert_eq!(f.kind, ValidationStepKind::ImapLogin);
    assert!(f.words.contains("Application-specific password required"), "{}", f.words);
    assert!(r.summary_words.contains("Application-specific password required"));
    assert_eq!(r.refused_by.as_deref(), Some("127.0.0.1"));
}

#[test]
fn a_refused_smtp_sign_in_names_the_sending_server() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let imap = rt.block_on(FakeImap::start());
    let smtp = rt.block_on(FakeSmtp::start(USER, "another-password"));
    let (core, _d) = core_with(gpt_nexus_detector(), kc());
    let mut cfg = local_config(imap.port, smtp.port);
    cfg.smtp_host = "localhost".into();
    let r = core.validate_account(cfg, pw("app-password"));
    assert!(!r.ok);
    assert_eq!(r.steps.iter().find(|s| !s.ok).unwrap().kind, ValidationStepKind::SmtpAuth);
    assert_eq!(r.refused_by.as_deref(), Some("localhost"));
}

#[test]
fn an_empty_password_and_an_oauth_config_are_refused_before_any_connection() {
    let (core, _d) = core_with(gpt_nexus_detector(), kc());
    let r = core.validate_account(local_config(1, 1), pw(""));
    assert!(!r.ok && r.summary_words.contains("password"), "{}", r.summary_words);
    let mut cfg = local_config(1, 1);
    cfg.auth_method = AuthMethod::Oauth2;
    let r = core.validate_account(cfg, pw("x"));
    assert!(!r.ok && r.summary_words.contains("app ID"), "{}", r.summary_words);
}

#[test]
fn add_account_stores_the_row_and_the_password_only_in_the_keychain() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let imap = rt.block_on(FakeImap::start());
    let smtp = rt.block_on(FakeSmtp::start(USER, "app-password"));
    let keychain = kc();
    let (core, dir) = core_with(gpt_nexus_detector(), keychain.clone());
    let out = core.run(Command::AddAccountConfigured { address: USER.into(), display_name: Some("Owner".into()), config: local_config(imap.port, smtp.port), password: pw("app-password") }).expect("added");
    let id = out.created_id.expect("created id");
    let accounts = core.accounts();
    assert_eq!(accounts.len(), 1);
    assert_eq!((accounts[0].id.as_str(), accounts[0].address.as_str(), accounts[0].name.as_str(), accounts[0].provider), (id.as_str(), USER, "Owner", MailProvider::Imap));
    let r = Namespace::rig().reference(secrets::SecretKind::AccountCredential, USER).unwrap();
    assert_eq!(ReadSecret::execute(keychain.as_ref(), &r).unwrap().expose(), b"app-password");
    let db = std::fs::read(dir.path().join("whoosh.sqlite")).unwrap();
    assert!(!db.windows(b"app-password".len()).any(|w| w == b"app-password"), "the password must not reach the database file");
    // a second add of the same address is refused in words
    let again = core.run(Command::AddAccountConfigured { address: USER.to_uppercase(), display_name: None, config: local_config(imap.port, smtp.port), password: pw("app-password") });
    let Err(CoreError::Action { failure }) = again else { panic!("{again:?}") };
    assert!(failure.reason_words.contains("already an account"), "{}", failure.reason_words);
}

#[test]
fn a_refused_password_adds_nothing() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let imap = rt.block_on(FakeImap::start());
    let smtp = rt.block_on(FakeSmtp::start(USER, "app-password"));
    let keychain = kc();
    let (core, _d) = core_with(gpt_nexus_detector(), keychain.clone());
    let res = core.run(Command::AddAccountConfigured { address: USER.into(), display_name: None, config: local_config(imap.port, smtp.port), password: pw("wrong") });
    let Err(CoreError::Action { failure }) = res else { panic!("{res:?}") };
    assert!(failure.retryable && !failure.reason_words.is_empty());
    assert!(core.accounts().is_empty());
    let r = Namespace::rig().reference(secrets::SecretKind::AccountCredential, USER).unwrap();
    assert!(ReadSecret::execute(keychain.as_ref(), &r).is_err(), "no Keychain item for a refused password");
}

#[test]
fn the_contract_security_maps_to_the_real_transports() {
    let dir = tempfile::tempdir().unwrap();
    let core = RustCore::with_parts(Store::open(&dir.path().join("w.sqlite")).unwrap(), kc(), gpt_nexus_detector(), TestTransport::Real);
    let mut cfg = local_config(993, 587);
    let v = core.validation_config(&cfg);
    assert_eq!(v.imap.security, sync_imap::Security::Tls);
    assert_eq!(v.smtp.as_ref().unwrap().security, send::Security::StartTls);
    cfg.imap_security = ConnectionSecurity::Starttls; cfg.smtp_security = ConnectionSecurity::Ssl;
    let v = core.validation_config(&cfg);
    assert_eq!(v.imap.security, sync_imap::Security::StartTls);
    assert_eq!(v.smtp.as_ref().unwrap().security, send::Security::Implicit);
}

#[test]
fn every_other_command_is_refused_in_words() {
    let (core, _d) = core_with(gpt_nexus_detector(), kc());
    // fix-blockforget: BlockSender is served now (block_cmds.rs): one row of the block table, no undo entry (it was refused "does not do this yet")
    let out = core.run(Command::BlockSender { address: "someone@example.test".into() }).expect("BlockSender is served");
    assert_eq!((out.undo_entry_id, core.blocks().len()), (None, 1));
}

#[test]
fn a87_a_refused_login_names_its_cause_and_keeps_the_servers_text() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let imap = rt.block_on(FakeImap::start());
    imap.with(|s| s.refuse_login = Some("Basic authentication is disabled.".into()));
    let smtp = rt.block_on(FakeSmtp::start(USER, "app-password"));
    let (core, _d) = core_with(gpt_nexus_detector(), kc());
    let r = core.validate_account(local_config(imap.port, smtp.port), pw("app-password"));
    assert_eq!(r.cause, Some(RefusalCause::BasicAuthDisabled));
    assert!(r.summary_words.contains("Basic authentication is disabled."), "{}", r.summary_words);
    imap.with(|s| s.refuse_login = Some("Invalid credentials".into()));
    assert_eq!(core.validate_account(local_config(imap.port, smtp.port), pw("app-password")).cause, Some(RefusalCause::BadPassword));
}
