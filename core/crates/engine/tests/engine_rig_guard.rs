//! engine-act-r6 round 5: a rig core (`RustCore::open_production(.., rig: true)`, the hidden test copy) closes its process to every host but
//! this Mac's ("nothing leaves this Mac except the relay"). Adding an account at a real-looking address with no redirection is refused in
//! words: no connection (a counting listener), no name lookup (the guard's resolver stub counts), the refusal counted on
//! `rig/refused-outbound`. The sign-in authority and the one-click unsubscribe refuse the same way. Every target is local: a guard removed
//! by its mutant is counted on 127.0.0.1 ("0.0.0.0" reaches the listener), never on the network. One process, one rig core.
#[path = "../../netguard/tests/stubs.rs"]
mod stubs;

use std::sync::atomic::Ordering;

use contracts::*;
use engine::sign_in::authority::AuthorityHttp;
use engine::RustCore;
use store::NewMessage;

fn refused(core: &RustCore) -> (u64, Option<String>) {
    let raw = core.state(netguard::REFUSED_STATE_PATH.into()).expect("the rig core serves rig/refused-outbound");
    let v: serde_json::Value = serde_json::from_str(&raw).unwrap();
    (v["count"].as_u64().unwrap(), v["last_host"].as_str().map(str::to_string))
}

#[test]
fn a_rig_core_reaches_this_mac_only_and_counts_what_it_refused() {
    for k in ["WHOOSH_CORE_CHECK_SERVERS", "WHOOSH_CORE_PLAIN_LOOPBACK", "WHOOSH_CORE_CHECK_AUTHORITY", "WHOOSH_CORE_CHECK_PROVIDERS", "WHOOSH_CORE_MUTANT"] {
        assert!(std::env::var(k).is_err(), "{k} is set: this test runs with no redirection");
    }
    let imap = stubs::listener();
    let asked = stubs::resolver_to(&imap);
    assert!(!netguard::is_closed(), "open before the rig core");
    let dir = tempfile::tempdir().unwrap();
    let core = RustCore::open_production(dir.path().to_str().unwrap(), true).expect("rig core");
    assert!(netguard::is_closed(), "the rig core closed its process at open");
    assert_eq!(refused(&core), (0, None));

    // Add account at example.org, typed settings, no redirection: refused in words, nothing dialled, nothing resolved
    let config = AccountConfig {
        imap_host: "imap.example.org".into(), imap_port: 993, imap_security: ConnectionSecurity::Ssl, smtp_host: String::new(), smtp_port: 465,
        smtp_security: ConnectionSecurity::Ssl, username: "owner@example.org".into(), smtp_username: "owner@example.org".into(), auth_method: AuthMethod::Password, source: DetectionSource::Manual,
    };
    let r = core.run(Command::AddAccountConfigured { address: "owner@example.org".into(), display_name: None, config, password: SecretText { value: "pw".into() } });
    assert!(matches!(&r, Err(CoreError::Action { failure }) if failure.reason_words.contains("not this Mac")), "refused in words: {r:?}");
    assert_eq!(asked.load(Ordering::SeqCst), 0, "no name lookup");
    assert_eq!(imap.count(), 0, "no connection");
    assert!(core.accounts().is_empty(), "nothing added");
    let (n, host) = refused(&core);
    assert!(n >= 1 && host.as_deref() == Some("imap.example.org"), "{n} {host:?}");

    // the detector, typed a real address: ISPDB, the domain's files, Autodiscover, DNS and the probe are each refused, none resolved
    let before = refused(&core).0;
    let found = core.detect_provider("owner@example.org".into());
    assert!(refused(&core).0 > before, "the detector's calls were refused: {found:?}");
    assert_eq!(asked.load(Ordering::SeqCst), 0, "the probe resolved nothing");
    assert_eq!(imap.count(), 0);

    // the sign-in authority (OpenID configuration, the authorize redirect, the token)
    let web = stubs::listener();
    let r = AuthorityHttp::new().get_status(&format!("https://0.0.0.0:{}/common/v2.0/.well-known/openid-configuration", web.port));
    assert!(matches!(&r, Err(CoreError::Action { failure }) if failure.code.as_deref() == Some(netguard::OUTBOUND_REFUSED_CODE)), "{r:?}");
    assert_eq!(web.count(), 0, "the authority opened nothing");
    assert_eq!(refused(&core).1.as_deref(), Some("0.0.0.0"));

    // the one-click unsubscribe (RFC 8058) of a stored message
    let store = core.store();
    let here = AccountConfig { imap_host: "127.0.0.1".into(), imap_port: 1, imap_security: ConnectionSecurity::Ssl, smtp_host: String::new(), smtp_port: 1,
        smtp_security: ConnectionSecurity::Ssl, username: "owner".into(), smtp_username: "owner".into(), auth_method: AuthMethod::Password, source: DetectionSource::Manual };
    let account = store.add_account("owner@this-mac.test", "Owner", MailProvider::Imap, "svc\u{1f}acct", false, here, 0).unwrap();
    let list = format!("https://0.0.0.0:{}/unsubscribe/1", web.port);
    store.insert_messages(vec![NewMessage {
        message: Message { id: "m-list".into(), thread_id: "t-list".into(), account_id: account, sender_name: "News".into(), from_address: "news@lists.example.org".into(), to: vec![],
            subject: "Weekly".into(), preview: String::new(), received_at_ms: 1, read: true, archived: false, folder: "INBOX".into(), attachment_names: vec![], login_code: None,
            trackers_blocked: 0, list_unsubscribe: vec![list], list_unsubscribe_one_click: true, reply_to_name: None, cc: vec![], bcc: vec![], is_outgoing: false },
        remote_id: None, body_text: None, body_html: None, has_list_header: true, size_bytes: None, tab_id: None,
    }]).unwrap();
    let r = core.run(Command::Unsubscribe { message_id: "m-list".into() });
    assert!(r.is_err(), "refused: {r:?}");
    assert_eq!(web.count(), 0, "the unsubscribe opened nothing");
}
// mutant "rig-core-never-closes": rig.rs `apply`, `netguard::close_to_this_mac();` deleted (red: the account add dials, the resolver is asked)
// mutant "authority-unguarded": sign_in/authority.rs `client`, the `netguard::outbound_allowed_url(url)?;` line deleted (red: the listener counts one)
// mutant "unsubscribe-unguarded": unsubscribe.rs `post_one_click`, the `netguard::outbound_allowed_url(url)?;` line deleted (red: the listener counts one)
// mutant "refusals-not-served": rust_core.rs `state`, the `rig/refused-outbound` link deleted (red: no answer)
