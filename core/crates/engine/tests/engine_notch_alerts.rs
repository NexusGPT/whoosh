//! The notch's alerts (owner's picks of 14 Nov 2026, docs/surfaces/mac-notch.md "1B 2A 3A"), through the rig's production core
//! (`open_production(_, rig = true)`) on a fresh data folder; the account points at 127.0.0.1 (loopback only). The settings are saved whole
//! (`Command::SetNotchAlerts`), read whole (`WhooshCore::notch_alerts`), served field by field at `notch_alerts/<field>`, and they survive a reopen
//! (migration 0033). The kind of a stored mail (`WhooshCore::notch_kind`, `notch_kind/<message>`) follows `contracts::notch_kind_of` over the
//! mail's own facts. Failure messages name message ids only.
//!
//! BROKEN COPIES, each seen red on the test named:
//!   N1 settings_cmds.rs `Command::SetNotchAlerts { alerts } => store.notch_alerts_put(contracts::notch_alerts_default())?`  -> the_alert_settings_are_saved_whole_and_survive_a_reopen
//!   N2 app_settings.rs `notch_alerts_put` binds `a.updates` at the place of `a.promotions`                                 -> the_alert_settings_are_saved_whole_and_survive_a_reopen
//!   N3 label_meta.rs `notch_facts` answers `has_sign_in_code: false`                                                       -> each_stored_mail_has_its_kind
//!   N4 label_meta.rs `notch_facts` answers `tab_id: None`                                                                  -> each_stored_mail_has_its_kind
//!   N5 settings_cmds.rs `notch_kind` ends in `.unwrap_or(NotchKind::Updates)`                                              -> each_stored_mail_has_its_kind
use contracts::*;
use engine::RustCore;
use store::NewMessage;

const USER: &str = "owner@example.test";

fn config() -> AccountConfig {
    AccountConfig {
        imap_host: "127.0.0.1".into(), imap_port: 1, imap_security: ConnectionSecurity::Ssl, smtp_host: "127.0.0.1".into(), smtp_port: 1,
        smtp_security: ConnectionSecurity::Starttls, username: USER.into(), smtp_username: USER.into(), auth_method: AuthMethod::Password, source: DetectionSource::Manual,
    }
}

fn mail(account: &str, id: &str, from: &str, tab: Option<&str>, list: bool, code: Option<&str>, at: i64) -> NewMessage {
    NewMessage {
        message: Message {
            id: id.into(), thread_id: format!("t-{id}"), account_id: account.into(), sender_name: "Sender".into(), from_address: from.into(),
            to: vec![USER.into()], subject: format!("subject {id}"), preview: String::new(), received_at_ms: at, read: false, archived: false,
            folder: "INBOX".into(), attachment_names: vec![], login_code: code.map(String::from), trackers_blocked: 0, list_unsubscribe: vec![],
            list_unsubscribe_one_click: false, reply_to_name: None, cc: vec![], bcc: vec![], is_outgoing: false,
        },
        remote_id: None, body_text: None, body_html: None, has_list_header: list, size_bytes: None, tab_id: tab.map(String::from),
    }
}

fn open(dir: &tempfile::TempDir) -> RustCore { RustCore::open_production(dir.path().to_str().unwrap(), true).expect("the rig core opens") }

const FIELDS: [&str; 9] = ["vip", "needs_reply", "primary", "updates", "promotions", "sign_in_codes", "stay", "sound", "full_mail_loads_pictures"];
fn served(core: &RustCore) -> Vec<String> { FIELDS.iter().map(|f| core.state(format!("notch_alerts/{f}")).unwrap_or_else(|| format!("{f}: no answer"))).collect() }

#[test]
fn before_any_save_the_start_values_are_the_picked_set() {
    let dir = tempfile::tempdir().unwrap();
    let core = open(&dir);
    assert_eq!(core.notch_alerts(), notch_alerts_default());
    assert_eq!(served(&core), ["\"card\"", "\"card\"", "\"line\"", "\"light\"", "\"nothing\"", "\"card\"", "\"10s\"", "false", "true"]);
    assert_eq!(core.state("notch_alerts/colour".into()), None, "a field that does not exist has no answer from the alert settings");
}

#[test]
fn the_alert_settings_are_saved_whole_and_survive_a_reopen() {
    let dir = tempfile::tempdir().unwrap();
    // every field differs from its start value, and no two styles of neighbouring fields are equal
    let first = NotchAlerts {
        vip: NotchAlertStyle::Opened, needs_reply: NotchAlertStyle::Line, primary: NotchAlertStyle::Nothing, updates: NotchAlertStyle::Card,
        promotions: NotchAlertStyle::Light, sign_in_codes: NotchAlertStyle::Line, stay: NotchStay::UntilActed, sound: true, full_mail_loads_pictures: false,
    };
    {
        let core = open(&dir);
        let out = core.run(Command::SetNotchAlerts { alerts: first.clone() }).expect("the save is answered");
        assert_eq!((out.undo_entry_id, out.created_id), (None, None), "a setting has no undo entry and creates nothing");
        assert_eq!(core.notch_alerts(), first);
        assert_eq!(served(&core), ["\"opened\"", "\"line\"", "\"nothing\"", "\"card\"", "\"light\"", "\"line\"", "\"until-acted\"", "true", "false"]);
    }
    let core = open(&dir);
    assert_eq!(core.notch_alerts(), first, "the saved settings after a reopen");
    // a second save replaces the first whole: one kind changed through the contract's own door, the rest as saved
    let second = notch_alerts_with(first.clone(), NotchKind::Promotions, NotchAlertStyle::Opened);
    core.run(Command::SetNotchAlerts { alerts: NotchAlerts { stay: NotchStay::FiveSeconds, ..second.clone() } }).expect("the second save is answered");
    let now = core.notch_alerts();
    assert_eq!((now.promotions, now.stay, now.updates, now.vip), (NotchAlertStyle::Opened, NotchStay::FiveSeconds, NotchAlertStyle::Card, NotchAlertStyle::Opened));
    assert_eq!(core.state("notch_alerts/stay".into()).as_deref(), Some("\"5s\""));
}

#[test]
fn each_stored_mail_has_its_kind() {
    let dir = tempfile::tempdir().unwrap();
    let core = open(&dir);
    let store = core.store();
    let a = store.add_account(USER, "Owner", MailProvider::Imap, "svc\u{1f}acct", false, config(), 1).unwrap();
    let t = 1_700_000_000_000i64;
    // (id, from, category tab, list header, sign-in code) -> the kind
    let cases: Vec<(&str, &str, Option<&str>, bool, Option<&str>, NotchKind)> = vec![
        ("k-code", "vip@partner.example.test", Some("promotions"), true, Some("482913"), NotchKind::SignInCode),
        ("k-vip-address", "vip@partner.example.test", Some("promotions"), true, None, NotchKind::Vip),
        ("k-vip-domain", "anyone@mail.board.example.test", Some("updates"), false, None, NotchKind::Vip),
        ("k-needs-reply", "person@example.test", Some("needs-reply"), false, None, NotchKind::NeedsReply),
        ("k-updates", "system@example.test", Some("updates"), true, None, NotchKind::Updates),
        ("k-promotions", "shop@example.test", Some("promotions"), true, None, NotchKind::Promotions),
        ("k-social", "network@example.test", Some("social"), true, None, NotchKind::Promotions),
        ("k-primary-with-list", "club@example.test", Some("primary"), true, None, NotchKind::Primary),
        ("k-empty-code", "person2@example.test", Some("primary"), false, Some("  "), NotchKind::Primary),
    ];
    store.insert_messages(cases.iter().enumerate().map(|(i, c)| mail(&a, c.0, c.1, c.2, c.3, c.4, t + i as i64)).collect()).unwrap();
    {
        // the VIP list has no command in the engine yet: the two rows are written the way the store reads them (kind, lower-case value)
        let db = rusqlite::Connection::open(dir.path().join("whoosh.sqlite")).unwrap();
        db.busy_timeout(std::time::Duration::from_secs(10)).unwrap();
        db.execute("INSERT INTO vip (kind, value, updated_at_ms) VALUES ('address', 'vip@partner.example.test', 1), ('domain', 'board.example.test', 1)", []).unwrap();
    }
    for (id, _, tab, _, _, want) in &cases {
        // the category is read at the same moment: the core's background filing may re-file a mail, and the kind must follow what is stored now
        let facts = store.notch_facts(id).unwrap().unwrap_or_else(|| panic!("{id} has facts"));
        assert_eq!(facts.tab_id.as_deref(), *tab, "{id}: premise, the mail is still in the category it was stored with");
        assert_eq!(core.notch_kind(id.to_string()), *want, "{id}");
        assert_eq!(core.state(format!("notch_kind/{id}")), Some(format!("\"{}\"", notch_kind_word(*want))), "{id}: the state path");
    }
    // a mail the store does not hold is Primary, through the read and the state path
    assert_eq!(core.notch_kind("no-such-mail".into()), NotchKind::Primary);
    assert_eq!(core.state("notch_kind/no-such-mail".into()).as_deref(), Some("\"primary\""));
    // the style each kind takes with the start values (the picked set): the code and the VIPs a card, a promotion nothing
    let alerts = core.notch_alerts();
    let styles: Vec<String> = cases.iter().map(|c| notch_alert_style_word(notch_alert_style(alerts.clone(), core.notch_kind(c.0.to_string())))).collect();
    assert_eq!(styles, ["card", "card", "card", "card", "light", "nothing", "nothing", "line", "line"]);
}
