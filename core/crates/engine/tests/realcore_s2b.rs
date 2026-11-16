//! The real core, slice 2b: the browser sign-in. A local fake authority on 127.0.0.1 (the tenant's OpenID configuration,
//! the sign-in page that redirects to the loopback with a code, the token endpoint) and the engine's own test browser
//! (`Follower`) play Microsoft and the person; sync-imap's IMAP and SMTP fakes accept XOAUTH2 with the issued token. No
//! outside network, no window. Each test was seen red on its mutant (docs/lanes/engine-oauth.md): a runtime one is chosen
//! with WHOOSH_TEST_OAUTH_MUTANT=<name>, read by this file only.
#[allow(dead_code, unused_imports)]
#[path = "../../autoconfig/tests/support/mod.rs"]
mod detect_fakes;
#[allow(dead_code, unused_imports)]
#[path = "../../sync-imap/tests/support/mod.rs"]
mod imap_fakes;

use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use contracts::*;
use detect_fakes::{FakeDns, FakeHttp, FakeProbe};
use engine::sign_in::{authority::AuthorityHttp, browser::{Follower, NoBrowser}};
use engine::{OAuthMutant, RustCore, SignInParts, TestTransport};
use imap_fakes::{smtp::FakeSmtp, FakeImap};
use secrets::{MemoryKeychain, Namespace, ReadSecret, Secret, SecretKind, StoreSecret};
use store::Store;

const USER: &str = "owner@example.test";
const DOMAIN: &str = "example.test";
/// The access token the fake authority issues: the IMAP fake's own XOAUTH2 token, so a sign-in it accepts is this one.
const ACCESS: &str = "token-1";
const WAIT: Duration = Duration::from_secs(20);

/// A client ID of the Microsoft shape, built from parts (the repository holds no real one).
fn client_id() -> String { [8, 4, 4, 4, 12].iter().map(|n| "c".repeat(*n)).collect::<Vec<_>>().join("-") }
fn mutant() -> OAuthMutant { std::env::var("WHOOSH_TEST_OAUTH_MUTANT").map(|v| OAuthMutant::from_name(&v)).unwrap_or(OAuthMutant::None) }
fn kc() -> Arc<MemoryKeychain> { Arc::new(MemoryKeychain::new(Namespace::rig())) }
fn item(kc: &MemoryKeychain, name: &str) -> Option<String> {
    let r = Namespace::rig().reference(SecretKind::AccountCredential, name).ok()?;
    ReadSecret::execute(kc, &r).ok().map(|s| String::from_utf8_lossy(s.expose()).into_owned())
}
fn tab() -> InboxQuery { InboxQuery { tab_id: "primary".into(), account_id: None, label_id: None, mailbox: None, account_ids: vec![], filters: vec![], smart_folder_id: None } }
fn detector() -> Box<dyn engine::Detect> { Box::new(autoconfig::Detector::new(FakeHttp::default(), FakeDns::default(), FakeProbe::default())) }

fn wait_until(what: &str, mut ok: impl FnMut() -> bool) {
    let t0 = Instant::now();
    while t0.elapsed() < WAIT { if ok() { return; } std::thread::sleep(Duration::from_millis(30)); }
    panic!("never reached within {WAIT:?}: {what}");
}

fn decode(s: &str) -> String {
    let b = s.as_bytes();
    let (mut out, mut i) = (Vec::new(), 0);
    while i < b.len() {
        match b[i] {
            b'+' => { out.push(b' '); i += 1 }
            b'%' if i + 2 < b.len() => { out.push(u8::from_str_radix(&s[i + 1..i + 3], 16).unwrap_or(b'?')); i += 3 }
            c => { out.push(c); i += 1 }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}
fn fields(q: &str) -> Vec<(String, String)> {
    q.split('&').filter(|p| !p.is_empty()).map(|p| { let (k, v) = p.split_once('=').unwrap_or((p, "")); (decode(k), decode(v)) }).collect()
}
fn get(f: &[(String, String)], k: &str) -> String { f.iter().find(|(n, _)| n == k).map(|(_, v)| v.clone()).unwrap_or_default() }

/// The fake authority: tenants whose OpenID configuration answers 200, a held or refusing sign-in page (`single_tenant`: the
/// `common` authority refuses the client ID with AADSTS50194, as Microsoft does for a single-tenant registration), the token endpoint
/// (PKCE checked against the challenge the page saw), and a log of every request (paths and grants, no query values).
struct Authority {
    port: u16,
    tenants: Vec<String>,
    hold: AtomicBool,
    single_tenant: AtomicBool,
    refuse: Mutex<Option<String>>,
    refuse_refresh: Mutex<Option<String>>,
    challenge: Mutex<String>,
    log: Mutex<Vec<String>>,
}

impl Authority {
    fn start() -> Arc<Authority> {
        let l = TcpListener::bind("127.0.0.1:0").unwrap();
        let a = Arc::new(Authority { port: l.local_addr().unwrap().port(), tenants: vec![DOMAIN.into(), MICROSOFT_ANY_ORGANISATION.into(), "common".into()], hold: AtomicBool::new(false), single_tenant: AtomicBool::new(false),
            refuse: Mutex::new(None), refuse_refresh: Mutex::new(None), challenge: Mutex::new(String::new()), log: Mutex::new(vec![]) });
        let me = a.clone();
        std::thread::spawn(move || for s in l.incoming().flatten() { let me = me.clone(); std::thread::spawn(move || me.serve(s)); });
        a
    }
    fn base(&self) -> String { format!("http://127.0.0.1:{}", self.port) }
    fn log(&self) -> Vec<String> { self.log.lock().unwrap().clone() }

    fn serve(&self, mut s: TcpStream) {
        let mut r = BufReader::new(s.try_clone().unwrap());
        let mut line = String::new();
        if r.read_line(&mut line).is_err() { return; }
        let mut len = 0usize;
        loop {
            let mut h = String::new();
            if r.read_line(&mut h).is_err() || h.trim().is_empty() { break; }
            if let Some(v) = h.to_ascii_lowercase().strip_prefix("content-length:") { len = v.trim().parse().unwrap_or(0); }
        }
        let mut body = vec![0u8; len];
        let _ = r.read_exact(&mut body);
        let mut parts = line.split_whitespace();
        let (method, target) = (parts.next().unwrap_or(""), parts.next().unwrap_or(""));
        let (path, query) = target.split_once('?').unwrap_or((target, ""));
        let tenant = path.trim_start_matches('/').split('/').next().unwrap_or("").to_string();
        let answer = |status: &str, headers: &str, body: &str| format!("HTTP/1.1 {status}\r\n{headers}Content-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len());
        let out = if path.ends_with("/v2.0/.well-known/openid-configuration") {
            self.log.lock().unwrap().push(format!("{method} {path}"));
            if self.tenants.contains(&tenant) { answer("200 OK", "Content-Type: application/json\r\n", r#"{"issuer":"fake"}"#) }
            else { answer("400 Bad Request", "Content-Type: application/json\r\n", r#"{"error":"invalid_tenant","error_description":"AADSTS90002: Tenant not found."}"#) }
        } else if path.ends_with("/oauth2/v2.0/authorize") {
            self.log.lock().unwrap().push(format!("{method} {path}"));
            let t0 = Instant::now();
            while self.hold.load(Ordering::SeqCst) && t0.elapsed() < WAIT { std::thread::sleep(Duration::from_millis(20)); }
            let q = fields(query);
            *self.challenge.lock().unwrap() = get(&q, "code_challenge");
            let single = tenant == "common" && self.single_tenant.load(Ordering::SeqCst);
            let words = if single { Some("AADSTS50194: Application is not configured as a multi-tenant application.".to_string()) } else { self.refuse.lock().unwrap().clone() };
            let back = match words {
                Some(words) => ::oauth::redirect_with(&get(&q, "redirect_uri"), &[("error", "access_denied"), ("error_description", &words), ("state", &get(&q, "state"))]),
                None => ::oauth::redirect_with(&get(&q, "redirect_uri"), &[("code", "the-code"), ("state", &get(&q, "state"))]),
            };
            if self.tenants.contains(&tenant) { answer("302 Found", &format!("Location: {back}\r\n"), "") } else { answer("400 Bad Request", "", "AADSTS90002") }
        } else if path.ends_with("/oauth2/v2.0/token") && method == "POST" {
            let f = fields(&String::from_utf8_lossy(&body));
            let grant = get(&f, "grant_type");
            self.log.lock().unwrap().push(format!("{method} {path} {grant}"));
            let tokens = |rt: &str| answer("200 OK", "Content-Type: application/json\r\n", &format!(r#"{{"access_token":"{ACCESS}","refresh_token":"{rt}","expires_in":3600}}"#));
            let refuse = |words: &str| answer("400 Bad Request", "Content-Type: application/json\r\n", &format!(r#"{{"error":"invalid_grant","error_description":"{words}"}}"#));
            match grant.as_str() {
                "authorization_code" if get(&f, "code") == "the-code" && ::oauth::Pkce::from_verifier(get(&f, "code_verifier")).challenge == *self.challenge.lock().unwrap() => tokens("rt-1"),
                "authorization_code" => refuse("the code or its PKCE verifier is wrong"),
                "refresh_token" => match self.refuse_refresh.lock().unwrap().clone() {
                    Some(words) => refuse(&words),
                    None if ["rt-1", "rt-2"].contains(&get(&f, "refresh_token").as_str()) => tokens("rt-2"),
                    None => refuse("unknown refresh token"),
                },
                _ => refuse("unsupported grant"),
            }
        } else { answer("404 Not Found", "", "") };
        let _ = s.write_all(out.as_bytes());
    }
}

fn parts(a: &Authority, browser: Arc<dyn ::oauth::OpenBrowser>) -> SignInParts { SignInParts { login_base: a.base(), browser, mutant: mutant() } }
fn follower() -> Arc<dyn ::oauth::OpenBrowser> { Arc::new(Follower { http: Arc::new(AuthorityHttp::new()) }) }

fn core_at(a: &Authority, keychain: Arc<MemoryKeychain>) -> (RustCore, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(&dir.path().join("whoosh.sqlite")).unwrap();
    (RustCore::with_sign_in(store, keychain, detector(), TestTransport::PlainLoopback, parts(a, follower())), dir)
}

fn oauth_config(imap_port: u16, smtp_port: u16) -> AccountConfig {
    AccountConfig {
        imap_host: "127.0.0.1".into(), imap_port, imap_security: ConnectionSecurity::Ssl, smtp_host: "127.0.0.1".into(), smtp_port,
        smtp_security: ConnectionSecurity::Starttls, username: USER.into(), smtp_username: USER.into(), auth_method: AuthMethod::Oauth2, source: DetectionSource::Autodiscover,
    }
}

fn code_of(r: Result<CommandOutcome, CoreError>) -> (Option<String>, String) {
    match r { Err(CoreError::Action { failure }) => (failure.code, failure.reason_words), other => panic!("not a coded refusal: {other:?}") }
}

fn sign_in(core: &RustCore) -> OAuthSignInState { sign_in_as(core, USER) }
fn sign_in_as(core: &RustCore, address: &str) -> OAuthSignInState {
    core.run(Command::SetOAuthClient { account_draft_id: address.into(), provider: OAuthProvider::Microsoft, client_id: client_id() }).expect("client set");
    core.run(Command::StartOAuthSignIn { account_draft_id: address.into() }).expect("sign-in started");
    let mut last = core.oauth_sign_in_state(address.into());
    let t0 = Instant::now();
    while t0.elapsed() < WAIT && matches!(last, OAuthSignInState::Waiting { .. } | OAuthSignInState::Ready) { std::thread::sleep(Duration::from_millis(30)); last = core.oauth_sign_in_state(address.into()); }
    last
}

#[test]
fn set_client_stores_the_id_in_the_keychain_and_the_state_reads_ready() {
    let a = Authority::start();
    let keychain = kc();
    let (core, _d) = core_at(&a, keychain.clone());
    assert_eq!(core.oauth_sign_in_state(USER.into()), OAuthSignInState::NeedsClientId);
    core.run(Command::SetOAuthClient { account_draft_id: "Owner@Example.TEST ".into(), provider: OAuthProvider::Microsoft, client_id: client_id() }).expect("set");
    assert_eq!(core.oauth_sign_in_state(USER.into()), OAuthSignInState::Ready);
    assert_eq!(item(&keychain, &format!("{USER}:oauth-client")), Some(client_id()), "the client ID is read back from the Keychain");
    let (code, words) = code_of(core.run(Command::SetOAuthClient { account_draft_id: USER.into(), provider: OAuthProvider::Microsoft, client_id: "not an id".into() }));
    assert_eq!((code.as_deref(), words.contains("no spaces")), (Some("oauth.no_client_id"), true), "{words}");
}

#[test]
fn start_opens_the_common_authority_and_reaches_waiting() {
    let a = Authority::start();
    a.hold.store(true, Ordering::SeqCst);
    let (core, _d) = core_at(&a, kc());
    core.run(Command::SetOAuthClient { account_draft_id: USER.into(), provider: OAuthProvider::Microsoft, client_id: client_id() }).unwrap();
    core.run(Command::StartOAuthSignIn { account_draft_id: USER.into() }).expect("started");
    let OAuthSignInState::Waiting { authorize_url } = core.oauth_sign_in_state(USER.into()) else { panic!("not waiting: {:?}", core.oauth_sign_in_state(USER.into())) };
    assert!(authorize_url.starts_with(&format!("{}/common/oauth2/v2.0/authorize?", a.base())), "the multi-tenant default signs in at common: {authorize_url}");
    assert!(authorize_url.contains("code_challenge_method=S256") && authorize_url.contains(&client_id()) && authorize_url.contains("redirect_uri=http%3A%2F%2Flocalhost%3A"));
    assert!(authorize_url.contains("IMAP.AccessAsUser.All") && authorize_url.contains("offline_access"), "the documented scopes: {authorize_url}");
    assert!(!a.log().iter().any(|l| l.contains("openid-configuration")), "no tenant lookup on the default path: {:?}", a.log());
    a.hold.store(false, Ordering::SeqCst);
}

#[test]
fn a_single_tenant_id_refused_at_common_signs_in_at_the_addresss_tenant() {
    let a = Authority::start();
    a.single_tenant.store(true, Ordering::SeqCst);
    let keychain = kc();
    let (core, _d) = core_at(&a, keychain.clone());
    assert_eq!(sign_in(&core), OAuthSignInState::Done);
    let want = [format!("GET /common/oauth2/v2.0/authorize"), format!("GET /{DOMAIN}/v2.0/.well-known/openid-configuration"),
                format!("GET /{DOMAIN}/oauth2/v2.0/authorize"), format!("POST /{DOMAIN}/oauth2/v2.0/token authorization_code")];
    assert_eq!(a.log(), want.to_vec(), "common refused with AADSTS50194, then the tenant of the address");
    assert_eq!(item(&keychain, &format!("{USER}:oauth-authority")), Some(format!("{}/{DOMAIN}/oauth2/v2.0/token", a.base())), "the account refreshes at its tenant");
}

#[test]
fn a_single_tenant_id_whose_domain_is_not_a_tenant_signs_in_at_organizations() {
    let a = Authority::start();
    a.single_tenant.store(true, Ordering::SeqCst);
    let (core, _d) = core_at(&a, kc());
    assert_eq!(sign_in_as(&core, "someone@no-tenant.test"), OAuthSignInState::Done);
    assert!(a.log().contains(&"GET /organizations/oauth2/v2.0/authorize".to_string()) && a.log().contains(&"POST /organizations/oauth2/v2.0/token authorization_code".to_string()), "{:?}", a.log());
}

#[test]
fn the_loopback_code_is_exchanged_and_the_refresh_token_lands_in_the_keychain() {
    let a = Authority::start();
    let keychain = kc();
    let (core, _d) = core_at(&a, keychain.clone());
    assert_eq!(sign_in(&core), OAuthSignInState::Done);
    assert!(a.log().contains(&"POST /common/oauth2/v2.0/token authorization_code".to_string()), "{:?}", a.log());
    assert_eq!(item(&keychain, &format!("{USER}:oauth-refresh")).as_deref(), Some("rt-1"), "the refresh token is in the Keychain");
    assert_eq!(item(&keychain, &format!("{USER}:oauth-authority")), Some(format!("{}/common/oauth2/v2.0/token", a.base())), "the authority is kept beside it");
}

#[test]
fn a_refused_answer_lands_as_refused_with_the_servers_words() {
    let a = Authority::start();
    *a.refuse.lock().unwrap() = Some("AADSTS65004: User declined to consent".into());
    let (core, _d) = core_at(&a, kc());
    match sign_in(&core) { OAuthSignInState::Refused { words } => assert!(words.contains("AADSTS65004: User declined to consent"), "{words}"), other => panic!("{other:?}") }
    let (code, words) = code_of(core.run(Command::AddAccountConfigured { address: USER.into(), display_name: None, config: oauth_config(1, 1), password: SecretText { value: String::new() } }));
    assert_eq!(code.as_deref(), Some("oauth.refused"));
    assert!(words.contains("AADSTS65004"), "{words}");
}

#[test]
fn add_account_with_oauth2_validates_by_xoauth2_and_starts_the_sync() {
    let a = Authority::start();
    let rt = tokio::runtime::Runtime::new().unwrap();
    let imap = rt.block_on(FakeImap::start());
    imap.seed("INBOX", 3);
    let smtp = rt.block_on(FakeSmtp::start(USER, ACCESS));
    let (core, _d) = core_at(&a, kc());
    assert_eq!(sign_in(&core), OAuthSignInState::Done);
    let out = core.run(Command::AddAccountConfigured { address: USER.into(), display_name: None, config: oauth_config(imap.port, smtp.port), password: SecretText { value: String::new() } })
        .expect("the oauth2 account is added");
    let id = out.created_id.unwrap();
    wait_until("three inbox rows synced by XOAUTH2", || core.inbox(tab()).len() == 3);
    let log = imap.log();
    assert!(log.iter().any(|l| l.to_ascii_uppercase().contains("AUTHENTICATE XOAUTH2")) && !log.iter().any(|l| l.to_ascii_uppercase().contains(" LOGIN ")), "{log:?}");
    assert!(smtp.log.lock().unwrap().iter().any(|l| l.to_ascii_uppercase().contains("XOAUTH2")), "the send check signed in by XOAUTH2");
    let acc = core.accounts().into_iter().find(|x| x.id == id).unwrap();
    assert_eq!(acc.provider, MailProvider::Microsoft);
    assert!(a.log().iter().any(|l| l.ends_with("token refresh_token")), "a fresh access token was asked with the refresh token: {:?}", a.log());
}

/// The Keychain and the store as a previous run left them: the account, its refresh token, client ID and authority.
fn stored(a: &Authority, keychain: &MemoryKeychain, imap_port: u16) -> (Store, String, tempfile::TempDir) {
    let put = |name: String, v: String| StoreSecret::execute(keychain, SecretKind::AccountCredential, &name, &Secret::new(v.into_bytes())).unwrap();
    let r = put(format!("{USER}:oauth-refresh"), "rt-1".into());
    put(format!("{USER}:oauth-client"), client_id());
    put(format!("{USER}:oauth-authority"), format!("{}/{DOMAIN}/oauth2/v2.0/token", a.base()));
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(&dir.path().join("whoosh.sqlite")).unwrap();
    let id = store.add_account(USER, "Owner", MailProvider::Microsoft, &format!("{}\u{1f}{}", r.service, r.account), true, oauth_config(imap_port, 1), 1).unwrap();
    (store, id, dir)
}

#[test]
fn a_stored_oauth2_account_is_rebuilt_at_launch_and_a_refused_refresh_lands_on_its_sync_state() {
    let a = Authority::start();
    let rt = tokio::runtime::Runtime::new().unwrap();
    let imap = rt.block_on(FakeImap::start());
    imap.seed("INBOX", 3);
    let keychain = kc();
    let (store, _id, _d) = stored(&a, &keychain, imap.port);
    let core = RustCore::with_sign_in(store, keychain, detector(), TestTransport::PlainLoopback, parts(&a, Arc::new(NoBrowser)));
    core.start_stored_accounts();
    wait_until("three inbox rows after the launch", || core.inbox(tab()).len() == 3);
    assert!(imap.log().iter().any(|l| l.to_ascii_uppercase().contains("AUTHENTICATE XOAUTH2")));

    let b = Authority::start();
    *b.refuse_refresh.lock().unwrap() = Some("AADSTS700082: The refresh token has expired".into());
    let keychain = kc();
    let (store, id, _d2) = stored(&b, &keychain, imap.port);
    let core = RustCore::with_sign_in(store, keychain, detector(), TestTransport::PlainLoopback, parts(&b, Arc::new(NoBrowser)));
    core.start_stored_accounts();
    wait_until("the refused refresh on the sync state", || core.sync_states().iter().any(|s| s.account_id == id && s.last_error.is_some()));
    let f = core.sync_states().into_iter().find(|s| s.account_id == id).unwrap().last_error.unwrap();
    assert!(f.reason_words.contains("AADSTS700082: The refresh token has expired"), "{}", f.reason_words);
}

#[test]
fn a_consent_required_refusal_carries_its_own_code_in_the_state_and_in_the_add_refusal() {
    // A97: the tenant lets no plain user approve the app. Microsoft answers the loopback with AADSTS65001; the state read and the
    // add-account refusal both say `oauth.consent_required`, the provider's words stay whole behind the code.
    let a = Authority::start();
    *a.refuse.lock().unwrap() = Some("AADSTS65001: The user or administrator has not consented to use the application.".into());
    let (core, _d) = core_at(&a, kc());
    match sign_in(&core) {
        OAuthSignInState::Refused { words } => assert!(words.starts_with("oauth.consent_required: ") && words.contains("AADSTS65001"), "{words}"),
        other => panic!("{other:?}"),
    }
    let (code, words) = code_of(core.run(Command::AddAccountConfigured { address: USER.into(), display_name: None, config: oauth_config(1, 1), password: SecretText { value: String::new() } }));
    assert_eq!(code.as_deref(), Some("oauth.consent_required"));
    assert!(words.contains("AADSTS65001") && !words.starts_with("oauth."), "{words}");
    // the words that mean it, at the loopback and at the token exchange; every other refusal stays `oauth.refused`
    use engine::sign_in::codes::refused_code;
    for w in ["the provider refused the sign-in: consent_required", r#"{"error":"invalid_grant","error_description":"AADSTS65001: The user has not consented"}"#] {
        assert_eq!(refused_code(w), "oauth.consent_required", "{w}");
    }
    assert_eq!(refused_code("the provider refused the sign-in: AADSTS65004: User declined to consent"), "oauth.refused");
}

#[test]
fn every_refusal_carries_its_code() {
    let a = Authority::start();
    let (core, _d) = core_at(&a, kc());
    // fix-blockforget: BlockSender is served now (block_cmds.rs), so it carries no refusal code (it was refused "does not do this yet", engine.not_built_yet)
    let out = core.run(Command::BlockSender { address: "someone@example.test".into() }).expect("BlockSender is served");
    assert_eq!((out.undo_entry_id, core.blocks().len()), (None, 1));
    let add = |c: &RustCore| c.run(Command::AddAccountConfigured { address: USER.into(), display_name: None, config: oauth_config(1, 1), password: SecretText { value: String::new() } });
    assert_eq!(code_of(core.run(Command::StartOAuthSignIn { account_draft_id: USER.into() })).0.as_deref(), Some("oauth.no_client_id"));
    assert_eq!(code_of(add(&core)).0.as_deref(), Some("oauth.no_client_id"));
    core.run(Command::SetOAuthClient { account_draft_id: USER.into(), provider: OAuthProvider::Microsoft, client_id: client_id() }).unwrap();
    assert_eq!(code_of(add(&core)).0.as_deref(), Some("oauth.sign_in_not_done"));
    let dir = tempfile::tempdir().unwrap();
    let quiet = RustCore::with_sign_in(Store::open(&dir.path().join("w.sqlite")).unwrap(), kc(), detector(), TestTransport::PlainLoopback, parts(&a, Arc::new(NoBrowser)));
    quiet.run(Command::SetOAuthClient { account_draft_id: USER.into(), provider: OAuthProvider::Microsoft, client_id: client_id() }).unwrap();
    let (code, words) = code_of(quiet.run(Command::StartOAuthSignIn { account_draft_id: USER.into() }));
    assert_eq!((code.as_deref(), words.contains("opens no browser")), (Some("oauth.browser"), true), "{words}");
}
