//! Contract delta 14: the bring-your-own OAuth2 flow end to end against a fake authority (an in-process token endpoint)
//! and the real loopback listener; the test plays the browser. No vendor client ID appears here (built from parts).
use std::io::{Read, Write};
use std::net::TcpStream;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use contracts::{Clock, CoreError, OAuthAuthority, OAuthProvider, OAuthSignInState};
use oauth::*;
use secrets::{MemoryKeychain, Namespace, ReadSecret, StoreSecret};

fn client_id() -> String { [8, 4, 4, 4, 12].iter().map(|n| "c".repeat(*n)).collect::<Vec<_>>().join("-") }

struct FixedClock(Mutex<i64>);
impl Clock for FixedClock {
    fn now_ms(&self) -> i64 { *self.0.lock().unwrap() }
    fn utc_offset_s(&self) -> i32 { 0 }
}

/// The fake authority's token endpoint: checks the PKCE verifier against the challenge the browser saw.
#[derive(Default)]
struct FakeAuthority { challenge: Mutex<Option<String>>, forms: Mutex<Vec<Vec<(String, String)>>>, refuse: bool }
impl TokenEndpoint for FakeAuthority {
    fn post_form(&self, _url: &str, form: &[(&str, &str)]) -> Result<String, CoreError> {
        self.forms.lock().unwrap().push(form.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect());
        let get = |k: &str| form.iter().find(|(n, _)| *n == k).map(|(_, v)| v.to_string()).unwrap_or_default();
        if self.refuse { return Ok(r#"{"error":"invalid_client","error_description":"AADSTS700016: application not found"}"#.into()); }
        match get("grant_type").as_str() {
            "authorization_code" => {
                let want = self.challenge.lock().unwrap().clone().unwrap_or_default();
                if Pkce::from_verifier(get("code_verifier")).challenge != want { return Ok(r#"{"error":"invalid_grant","error_description":"PKCE verification failed"}"#.into()); }
                Ok(r#"{"access_token":"at-1","refresh_token":"rt-1","expires_in":3600}"#.into())
            }
            "refresh_token" if get("refresh_token") == "rt-1" => Ok(r#"{"access_token":"at-2","refresh_token":"rt-2","expires_in":3600}"#.into()),
            _ => Ok(r#"{"error":"invalid_grant"}"#.into()),
        }
    }
}

/// The browser: records the URL, then follows the redirect the way the provider would after the person signed in.
struct Browser { url: Mutex<Option<String>>, authority: Arc<FakeAuthority>, tamper_state: bool }
impl OpenBrowser for Browser {
    fn open(&self, url: &str) -> Result<(), CoreError> {
        *self.url.lock().unwrap() = Some(url.to_string());
        let q = |k: &str| url.split(&format!("{k}=")).nth(1).map(|v| v.split('&').next().unwrap().to_string()).unwrap_or_default();
        *self.authority.challenge.lock().unwrap() = Some(q("code_challenge"));
        let redirect = q("redirect_uri").replace("%3A", ":").replace("%2F", "/");
        let state = if self.tamper_state { "someone-else".to_string() } else { q("state") };
        std::thread::spawn(move || {
            let target = redirect_with("/", &[("code", "the-code"), ("state", &state)]);
            let host = redirect.trim_start_matches("http://").trim_end_matches('/').to_string();
            let mut s = TcpStream::connect(host).unwrap();
            write!(s, "GET {target} HTTP/1.1\r\nHost: x\r\n\r\n").unwrap();
            let mut b = String::new();
            let _ = s.read_to_string(&mut b);
        });
        Ok(())
    }
}

fn endpoints(p: OAuthProvider) -> OAuthAuthority {
    let mut e = contracts::oauth_endpoints(p);
    e.authorize_url = "http://127.0.0.1:1/authorize".into();   // the fake authority: never the real one in a test
    e.token_url = "http://127.0.0.1:1/token".into();
    e
}

fn rig(authority: Arc<FakeAuthority>, tamper: bool) -> (SignIns, Arc<Browser>, Arc<MemoryKeychain>) {
    let kc = Arc::new(MemoryKeychain::new(Namespace::rig()));
    let browser = Arc::new(Browser { url: Mutex::new(None), authority: authority.clone(), tamper_state: tamper });
    let mut s = SignIns::new(kc.clone(), kc.clone(), authority, browser.clone(), Arc::new(FixedClock(Mutex::new(1_000))), Duration::from_secs(5));
    s.endpoints = endpoints;
    (s, browser, kc)
}

fn wait_done(s: &SignIns, draft: &str) -> OAuthSignInState {
    for _ in 0..250 {
        let st = s.state(draft);
        if !matches!(st, OAuthSignInState::Waiting { .. }) { return st; }
        std::thread::sleep(Duration::from_millis(20));
    }
    s.state(draft)
}

#[test]
fn the_rfc_7636_example_verifier_gives_its_challenge() {
    assert_eq!(Pkce::from_verifier("dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk".into()).challenge, "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM");
    let p = Pkce::new().unwrap();
    assert!(p.verifier.len() >= 43 && p.verifier.len() <= 128);
}

#[test]
fn a_sign_in_runs_pkce_over_the_loopback_and_keeps_the_refresh_token() {
    let authority = Arc::new(FakeAuthority::default());
    let (s, browser, kc) = rig(authority.clone(), false);
    let draft = "Assem@Northlight-Example.DE";
    assert_eq!(s.state(draft), OAuthSignInState::NeedsClientId);
    assert!(s.start(draft).is_err(), "no client ID, no sign-in");
    s.set_client(draft, OAuthProvider::Microsoft, &client_id()).unwrap();
    assert_eq!(s.state(draft), OAuthSignInState::Ready);
    s.start(draft).unwrap();
    assert_eq!(wait_done(&s, draft), OAuthSignInState::Done);
    let url = browser.url.lock().unwrap().clone().unwrap();
    assert!(url.contains("code_challenge_method=S256") && url.contains("redirect_uri=http%3A%2F%2Flocalhost%3A") && url.contains(&client_id()));
    assert!(url.contains("scope=offline_access%20https%3A%2F%2Foutlook.office.com%2FIMAP.AccessAsUser.All"));
    let form = authority.forms.lock().unwrap()[0].clone();
    assert!(form.iter().any(|(k, _)| k == "code_verifier") && !form.iter().any(|(k, _)| k == "client_secret"), "a public client: no secret");
    let r = s.reference_of(&refresh_secret_name("assem@northlight-example.de")).unwrap();
    assert_eq!(ReadSecret::execute(&*kc, &r).unwrap().expose(), b"rt-1");
    assert_ne!(r, s.reference_of(&client_secret_name("assem@northlight-example.de")).unwrap(), "the refresh token has its own item");
    let c = s.reference_of(&client_secret_name("assem@northlight-example.de")).unwrap();
    assert_eq!(ReadSecret::execute(&*kc, &c).unwrap().expose(), client_id().as_bytes(), "the client ID is still the one pasted");
}

#[test]
fn a_wrong_state_or_a_refusing_authority_is_refused_in_words() {
    let (s, _, _) = rig(Arc::new(FakeAuthority::default()), true);
    s.set_client("a@x.example", OAuthProvider::Google, &format!("{}-{}{}", "1".repeat(6), "y".repeat(8), [".apps", ".googleusercontent", ".com"].concat())).unwrap();
    s.start("a@x.example").unwrap();
    match wait_done(&s, "a@x.example") { OAuthSignInState::Refused { words } => assert!(words.contains("state"), "{words}"), other => panic!("{other:?}") }
    let (s2, _, _) = rig(Arc::new(FakeAuthority { refuse: true, ..Default::default() }), false);
    s2.set_client("b@x.example", OAuthProvider::Microsoft, &client_id()).unwrap();
    s2.start("b@x.example").unwrap();
    match wait_done(&s2, "b@x.example") { OAuthSignInState::Refused { words } => assert!(words.contains("application not found"), "{words}"), other => panic!("{other:?}") }
    assert!(s2.set_client("b@x.example", OAuthProvider::Microsoft, "nope").is_err(), "a pasted ID that is not one is refused");
}

#[test]
fn the_access_token_is_refreshed_before_it_expires_and_a_rotated_refresh_token_is_kept() {
    let authority = Arc::new(FakeAuthority::default());
    let kc = Arc::new(MemoryKeychain::new(Namespace::rig()));
    let r = StoreSecret::execute(&*kc, secrets::SecretKind::AccountCredential, "a@x.example:oauth-refresh", &secrets::Secret::new(b"rt-1".to_vec())).unwrap();
    let clock = Arc::new(FixedClock(Mutex::new(0)));
    let t = RefreshingToken {
        endpoint: authority.clone(), token_url: "http://127.0.0.1:1/token".into(), client_id: client_id(), refresh_ref: r.clone(),
        refresh_name: "a@x.example:oauth-refresh".into(), read: kc.clone(), store: kc.clone(), clock: clock.clone(), margin_ms: 300_000,
        cached: Mutex::new(Some(TokenSet { access_token: "at-0".into(), refresh_token: None, expires_at_ms: 1_000_000 })),
    };
    assert_eq!(t.access_token().unwrap(), "at-0", "well before expiry: the cached token");
    *clock.0.lock().unwrap() = 1_000_000 - 299_000;   // inside the margin
    assert_eq!(t.access_token().unwrap(), "at-2", "refreshed before it expires");
    assert_eq!(ReadSecret::execute(&*kc, &r).unwrap().expose(), b"rt-2", "the rotated refresh token replaces the old one");
}

#[test]
fn the_ledger_sets_the_wait_and_the_margin() {
    assert_eq!(ledger_timeout(), Duration::from_secs(contracts::constants::OAUTH_LOOPBACK_TIMEOUT_S));
    assert_eq!(ledger_margin_ms(), contracts::constants::OAUTH_REFRESH_MARGIN_S as i64 * 1000);
}
