//! real-settings: the settings commands the real core answered "The local engine does not do this yet." to now run on the store
//! (core/crates/engine/src/settings_cmds.rs, core/crates/store/src/app_settings.rs, and the two arms of ai_settings.rs): the notch, the notifications,
//! the key preset, the split tabs, the provider key removal and the mail consent. Every case runs `RustCore` over a real migrated store and a fake
//! secret store (`MemoryKeychain`, rig namespace: never the owner's Keychain). Nothing here opens a socket.
//!
//! Per command: the write, its read back through the trait read and the state path, the refusal cases (the test engine's words), the restart (the store
//! is closed and opened again), and the time on a store of 2,000 mails (one line `settings, <name>: <ms> ms on 2000 mails`, a failure over 50 ms).
//! Run it with `-- --nocapture` and in the release profile. Each test ends with its BROKEN COPY in a `// broken copy:` comment: one edit of the file named
//! that turns the test red (the exact `find` of each is in docs/lanes/real-settings.md).
#[allow(dead_code, unused_imports)]
#[path = "../../autoconfig/tests/support/mod.rs"]
mod detect_fakes;

use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, Instant};

use contracts::*;
use detect_fakes::{FakeDns, FakeHttp, FakeProbe};
use engine::{RustCore, TestTransport};
use secrets::{DeleteSecret, MemoryKeychain, Namespace, ReadSecret, Secret, SecretKind, StoreSecret};
use store::{NewMessage, Store};

const USER: &str = "owner@example.test";
const SECOND: &str = "second@example.test";
/// docs/CONSTANTS.md INTERACTION_COMMIT_MS
const BUDGET_MS: f64 = 50.0;
const MAILS: usize = 2_000;
const KEY: &str = "FIXTURE-sk-ant-real-settings-61c9e0aa";

fn config(user: &str) -> AccountConfig {
    AccountConfig {
        imap_host: "127.0.0.1".into(), imap_port: 1, imap_security: ConnectionSecurity::Ssl, smtp_host: "127.0.0.1".into(), smtp_port: 1,
        smtp_security: ConnectionSecurity::Starttls, username: user.into(), smtp_username: user.into(), auth_method: AuthMethod::Password, source: DetectionSource::Manual,
    }
}

fn open_core(path: &Path, keys: Arc<MemoryKeychain>) -> RustCore {
    let detector: Box<dyn engine::Detect> = Box::new(autoconfig::Detector::new(FakeHttp::default(), FakeDns::default(), FakeProbe::default()));
    RustCore::with_parts(Store::open(path).unwrap(), keys, detector, TestTransport::Real)
}

struct Rig { core: RustCore, keys: Arc<MemoryKeychain>, dir: tempfile::TempDir, accounts: Vec<String> }

impl Rig {
    fn new() -> Rig {
        let dir = tempfile::tempdir().unwrap();
        let keys = Arc::new(MemoryKeychain::new(Namespace::rig()));
        let core = open_core(&dir.path().join("whoosh.sqlite"), keys.clone());
        let store = core.store();
        let a = store.add_account(USER, "Owner", MailProvider::Imap, "svc\u{1f}acct", false, config(USER), 1).unwrap();
        let b = store.add_account(SECOND, "Second", MailProvider::Imap, "svc2\u{1f}acct2", false, config(SECOND), 2).unwrap();
        Rig { core, keys, dir, accounts: vec![a, b] }
    }
    fn path(&self) -> std::path::PathBuf { self.dir.path().join("whoosh.sqlite") }
    fn state(&self, path: &str) -> Option<String> { self.core.state(path.to_string()) }
    fn run(&self, c: Command) -> CommandOutcome { self.core.run(c).unwrap_or_else(|e| panic!("refused: {e:?}")) }
    /// The app is closed and opened again over the same store and the same secret store.
    fn restart(self) -> Rig {
        let Rig { core, keys, dir, accounts } = self;
        drop(core);
        let core = open_core(&dir.path().join("whoosh.sqlite"), keys.clone());
        Rig { core, keys, dir, accounts }
    }
}

/// The words of an Invalid refusal: (field, words). Anything else, or an acceptance, fails the test.
fn invalid(r: Result<CommandOutcome, CoreError>) -> (String, String) {
    match r {
        Err(CoreError::Invalid { field, reason_words }) => (field, reason_words),
        Err(other) => panic!("expected an Invalid refusal, got {other:?}"),
        Ok(_) => panic!("expected an Invalid refusal, the command was accepted"),
    }
}

fn not_found(r: Result<CommandOutcome, CoreError>) -> (String, String) {
    match r {
        Err(CoreError::NotFound { kind, id }) => (kind, id),
        Err(other) => panic!("expected NotFound, got {other:?}"),
        Ok(_) => panic!("expected NotFound, the command was accepted"),
    }
}

fn s(v: &str) -> Option<String> { Some(v.to_string()) }

/// Runs `f`, prints `settings, <name>: <ms> ms on 2000 mails` and fails over the owner's 50 ms bound.
fn timed<R>(name: &str, f: impl FnOnce() -> R) -> R {
    let t = Instant::now();
    let out = f();
    let ms = t.elapsed().as_secs_f64() * 1000.0;
    println!("settings, {name}: {ms:.1} ms on {MAILS} mails");
    assert!(ms < BUDGET_MS, "{name} took {ms:.1} ms, over {BUDGET_MS} ms");
    out
}

fn fill_inbox(core: &RustCore, account: &str) {
    let batch: Vec<NewMessage> = (0..MAILS).map(|i| NewMessage {
        message: Message {
            id: format!("m{i}"), thread_id: format!("m{i}:thread"), account_id: account.into(), sender_name: format!("Sender {}", i % 40), from_address: format!("s{}@shop{}.test", i % 40, i % 7),
            to: vec![USER.into()], subject: format!("Order {i}"), preview: format!("body of order {i}"), received_at_ms: 1_000 + i as i64, read: i % 3 == 0, archived: false,
            folder: "INBOX".into(), attachment_names: vec![], login_code: None, trackers_blocked: 0, list_unsubscribe: vec![], list_unsubscribe_one_click: false,
            reply_to_name: None, cc: vec![], bcc: vec![], is_outgoing: false,
        },
        remote_id: None, body_text: Some(format!("body of order {i}")), body_html: None, has_list_header: false, size_bytes: None, tab_id: None,
    }).collect();
    core.store().insert_messages(batch).unwrap();
}

// ---------------------------------------------------------------- the notch

#[test]
fn the_notch_settings_are_kept_and_read_back_and_survive_a_restart() {
    let rig = Rig::new();
    // before the first save: the documented defaults, as the test engine's
    assert_eq!(rig.core.notch_setting(), NotchSetting { show: true, buttons: vec!["mark-read".into(), "archive".into(), "reply".into()], hidden_in_screen_share: true, follow_focus: true, list_shows: NotchList::Latest });
    assert_eq!((rig.state("notch_setting/show"), rig.state("notch_setting/enabled"), rig.state("notch_setting/buttons")), (s("\"latest\""), s("true"), s("[\"mark-read\",\"archive\",\"reply\"]")));
    assert_eq!((rig.state("notch_setting/hidden_in_screen_share"), rig.state("notch_setting/follow_focus"), rig.state("notch_setting/nowhere")), (s("true"), s("true"), None));
    let wanted = NotchSetting { show: false, buttons: vec!["reply".into(), "delete".into(), "reply".into()], hidden_in_screen_share: false, follow_focus: true, list_shows: NotchList::Unread };
    rig.run(Command::SetNotchSetting { setting: wanted });
    let kept = NotchSetting { show: false, buttons: vec!["reply".into(), "delete".into()], hidden_in_screen_share: false, follow_focus: true, list_shows: NotchList::Unread };
    assert_eq!(rig.core.notch_setting(), kept, "a button named twice is kept once, in the order given");
    assert_eq!((rig.state("notch_setting/show"), rig.state("notch_setting/enabled"), rig.state("notch_setting/buttons")), (s("\"unread\""), s("false"), s("[\"reply\",\"delete\"]")));
    assert_eq!((rig.state("notch_setting/hidden_in_screen_share"), rig.state("notch_setting/follow_focus")), (s("false"), s("true")));
    rig.run(Command::SetNotchSetting { setting: NotchSetting { buttons: vec![], ..kept.clone() } });
    assert_eq!(rig.state("notch_setting/buttons"), s("\"none\""), "no button reads none");
    rig.run(Command::SetNotchSetting { setting: kept.clone() });
    let rig = rig.restart();
    assert_eq!(rig.core.notch_setting(), kept, "the notch settings are on disk");
    assert_eq!(rig.state("notch_setting/follow_focus"), s("true"));
}
// broken copy: store/src/app_settings.rs `notch_setting_put`: `list_word(s.list_shows)` becomes `"latest"` (the list choice is lost: unread reads latest).

#[test]
fn a_notch_button_outside_the_four_is_refused_in_words_and_nothing_is_written() {
    let rig = Rig::new();
    let before = rig.core.notch_setting();
    let bad = NotchSetting { show: false, buttons: vec!["archive".into(), "snooze".into()], hidden_in_screen_share: false, follow_focus: true, list_shows: NotchList::Unread };
    let (field, words) = invalid(rig.core.run(Command::SetNotchSetting { setting: bad }));
    assert_eq!(field, "buttons");
    assert_eq!(words, "“snooze” is not a notch button; the buttons are mark-read, archive, reply and delete");
    assert_eq!(rig.core.notch_setting(), before, "a refused save changes nothing");
}
// broken copy: store/src/app_settings.rs `notch_setting_put`: the `NOTCH_BUTTONS.contains` test becomes `true` (every word is a button).

// ---------------------------------------------------------------- the notifications

#[test]
fn each_accounts_notification_setting_is_kept_and_read_back_and_survives_a_restart() {
    let rig = Rig::new();
    let (a, b) = (rig.accounts[0].clone(), rig.accounts[1].clone());
    let both = |id: &str| NotificationSetting { account_id: id.into(), enabled: true, delivery: Some(NotificationDelivery::Both) };
    assert_eq!(rig.core.notification_settings(), vec![both(&a), both(&b)], "every account is on and delivers to both before the owner chooses");
    assert_eq!((rig.state("notification_setting/off"), rig.state("notification_setting/delivery")), (s("[]"), s("\"both\"")));
    rig.run(Command::SetNotificationSetting { setting: NotificationSetting { account_id: a.clone(), enabled: true, delivery: Some(NotificationDelivery::Notch) } });
    assert_eq!(rig.state(&format!("notification_setting/{a}/delivery")), s("\"notch\""));
    assert_eq!(rig.state(&format!("notification_setting/{b}/delivery")), s("\"both\""));
    assert_eq!(rig.state("notification_setting/delivery"), s("\"both\""), "the accounts differ: both");
    rig.run(Command::SetNotificationSetting { setting: NotificationSetting { account_id: b.clone(), enabled: true, delivery: Some(NotificationDelivery::Notch) } });
    assert_eq!(rig.state("notification_setting/delivery"), s("\"notch\""), "every account agrees: that one");
    // SetNotification mutes and unmutes one account and leaves where it announces as it was
    rig.run(Command::SetNotification { account_id: a.clone(), enabled: false });
    assert_eq!((rig.state(&format!("notification_setting/{a}/enabled")), rig.state(&format!("notification_setting/{a}/delivery"))), (s("false"), s("\"notch\"")));
    assert_eq!(rig.state("notification_setting/off"), Some(format!("[\"{a}\"]")));
    assert_eq!(rig.core.notification_settings()[0], NotificationSetting { account_id: a.clone(), enabled: false, delivery: Some(NotificationDelivery::Notch) });
    // a setting with no delivery chosen is both (the contract's default)
    rig.run(Command::SetNotificationSetting { setting: NotificationSetting { account_id: b.clone(), enabled: true, delivery: None } });
    assert_eq!(rig.state(&format!("notification_setting/{b}/delivery")), s("\"both\""));
    rig.run(Command::SetNotificationSetting { setting: NotificationSetting { account_id: b.clone(), enabled: false, delivery: Some(NotificationDelivery::Notifications) } });
    let mut want_off = vec![a.clone(), b.clone()];
    want_off.sort();
    assert_eq!(rig.state("notification_setting/off"), Some(format!("[\"{}\",\"{}\"]", want_off[0], want_off[1])), "the muted accounts, sorted");
    let rig = rig.restart();
    assert_eq!(rig.state(&format!("notification_setting/{b}/delivery")), s("\"notifications\""), "the setting is on disk");
    assert_eq!(rig.state(&format!("notification_setting/{a}/enabled")), s("false"));
}
// broken copy: store/src/app_settings.rs `notification_write`: `(None, true) => kept.unwrap_or_else(...)` becomes `(None, true) => "both".into()` (a mute forgets where the account announces).

#[test]
fn a_notification_setting_for_an_unknown_account_is_not_found_and_writes_nothing() {
    let rig = Rig::new();
    let before = rig.core.notification_settings();
    assert_eq!(not_found(rig.core.run(Command::SetNotificationSetting { setting: NotificationSetting { account_id: "nobody".into(), enabled: false, delivery: None } })), ("account".into(), "nobody".into()));
    assert_eq!(not_found(rig.core.run(Command::SetNotification { account_id: "nobody".into(), enabled: false })), ("account".into(), "nobody".into()));
    assert_eq!(rig.core.notification_settings(), before);
    assert_eq!(rig.state("notification_setting/nobody/enabled"), None, "an account that is not one has no path");
}
// broken copy: store/src/app_settings.rs `notification_write`: the `exists(... FROM account ...)` test is removed (the row is written for any id, or the foreign key fails as an Unexpected error).

// ---------------------------------------------------------------- the key preset

#[test]
fn the_key_preset_is_kept_read_back_refused_when_unknown_and_survives_a_restart() {
    let rig = Rig::new();
    assert_eq!((rig.core.shortcut_preset().preset, rig.state("shortcut_preset/name")), ("Superhuman".to_string(), s("\"Superhuman\"")));
    rig.run(Command::SetShortcutPreset { preset: "GotEmail".into() });
    assert_eq!((rig.core.shortcut_preset().preset, rig.state("shortcut_preset/name")), ("GotEmail".to_string(), s("\"GotEmail\"")));
    let (field, words) = invalid(rig.core.run(Command::SetShortcutPreset { preset: "Vim".into() }));
    assert_eq!((field.as_str(), words.as_str()), ("preset", "the presets are Superhuman and GotEmail"));
    assert_eq!(rig.state("shortcut_preset/name"), s("\"GotEmail\""), "a refused preset changes nothing");
    let rig = rig.restart();
    assert_eq!(rig.core.shortcut_preset().preset, "GotEmail", "the preset is on disk");
    assert_eq!(rig.state("shortcut_preset/name"), s("\"GotEmail\""));
}
// broken copy: store/src/app_settings.rs `shortcut_preset_put`: the `SHORTCUT_PRESETS.contains` test is removed (an unknown preset is kept).

// ---------------------------------------------------------------- the split tabs

fn names(core: &RustCore) -> Vec<String> { core.split_tabs().into_iter().map(|t| t.name).collect() }
fn ids(core: &RustCore) -> Vec<String> { core.split_tabs().into_iter().map(|t| t.id).collect() }

#[test]
fn the_default_tabs_are_the_six_shipped_and_the_tab_counts_read_the_same_rows() {
    let rig = Rig::new();
    let shipped: Vec<String> = store::default_tab_names();
    assert_eq!(shipped.len(), 6, "the ledger's six default tabs");
    assert_eq!(names(&rig.core), shipped);
    assert_eq!(ids(&rig.core), shipped.iter().map(|n| store::tab_id(n)).collect::<Vec<_>>());
    assert!(rig.core.split_tabs().iter().all(|t| t.enabled && matches!(t.rule, SplitTabRule::Builtin { .. })));
    assert_eq!(rig.core.split_tabs().iter().map(|t| t.order).collect::<Vec<_>>(), (0..6).collect::<Vec<u32>>());
    assert_eq!(rig.state("split_tab/count"), s("6"));
    assert_eq!(rig.state("split_tab/needs-reply/rule"), s("\"builtin:needs-reply\""), "a default tab's rule names its id, as the test engine's");
    assert_eq!(rig.state("split_tab/updates/name"), s("\"Updates\""), "a default tab keeps the strip's own name");
}

#[test]
fn a_split_tab_is_added_renamed_reordered_hidden_and_removed_and_each_survives_a_restart() {
    let rig = Rig::new();
    let first = rig.run(Command::AddSplitTab { name: "Orders & Receipts".into(), rule: SplitTabRule::Sender { address: "orders@shop.test".into() } }).created_id.expect("the new tab's id");
    assert_eq!(first, "orders-receipts");
    let second = rig.run(Command::AddSplitTab { name: "Shop".into(), rule: SplitTabRule::Domain { domain: "shop.test".into() } }).created_id.unwrap();
    let third = rig.run(Command::AddSplitTab { name: "Mine".into(), rule: SplitTabRule::Account { account_id: rig.accounts[0].clone() } }).created_id.unwrap();
    assert_eq!(rig.core.split_tabs().len(), 9);
    assert_eq!(ids(&rig.core)[6..], [first.clone(), second.clone(), third.clone()], "a tab goes after the last one");
    assert_eq!((rig.state(&format!("split_tab/{first}/order")), rig.state(&format!("split_tab/{first}/rule")), rig.state(&format!("split_tab/{first}/enabled"))), (s("6"), s("\"sender:orders@shop.test\""), s("true")));
    assert_eq!(rig.state(&format!("split_tab/{first}/name")), s("\"Orders & Receipts\""));
    assert_eq!(rig.state(&format!("split_tab/{second}/rule")), s("\"domain:shop.test\""));
    assert_eq!(rig.state("split_tab/count"), s("9"));
    // rename: only the name changes, a default tab too
    rig.run(Command::RenameSplitTab { tab_id: first.clone(), name: "  Receipts ".into() });
    rig.run(Command::RenameSplitTab { tab_id: "updates".into(), name: "News".into() });
    assert_eq!((rig.state(&format!("split_tab/{first}/name")), rig.state("split_tab/updates/name")), (s("\"Receipts\""), s("\"News\"")));
    assert_eq!(rig.core.split_tabs()[2].name, "News");
    // reorder: the new first, the others after
    let mut order = ids(&rig.core);
    order[1..].rotate_right(1);   // Primary is a view and stays first (delta 16): the last tab goes to the second place
    rig.run(Command::ReorderSplitTabs { ids: order.clone() });
    assert_eq!(ids(&rig.core), order);
    assert_eq!(rig.state(&format!("split_tab/{third}/order")), s("1"));
    // hide one: it leaves the counts' strip and stays in the store; show it again
    let counted = |core: &RustCore| core.counts().tabs.into_iter().map(|t| t.tab_id).collect::<Vec<_>>();
    assert!(counted(&rig.core).contains(&"promotions".to_string()));
    rig.run(Command::SetSplitTabEnabled { id: "promotions".into(), on: false });
    assert_eq!(rig.state("split_tab/promotions/enabled"), s("false"));
    assert!(!counted(&rig.core).contains(&"promotions".to_string()), "a hidden tab leaves the counts");
    assert!(ids(&rig.core).contains(&"promotions".to_string()), "a hidden tab stays in the store");
    let rig = rig.restart();
    assert_eq!(ids(&rig.core), order, "the order is on disk");
    assert_eq!(rig.state("split_tab/promotions/enabled"), s("false"));
    assert_eq!((rig.state(&format!("split_tab/{first}/name")), rig.state("split_tab/updates/name")), (s("\"Receipts\""), s("\"News\"")));
    rig.run(Command::SetSplitTabEnabled { id: "promotions".into(), on: true });
    assert!(counted(&rig.core).contains(&"promotions".to_string()));
    // remove an added tab: the others close up
    rig.run(Command::RemoveSplitTab { id: second.clone() });
    assert_eq!(rig.core.split_tabs().len(), 8);
    assert!(!ids(&rig.core).contains(&second));
    assert_eq!(rig.core.split_tabs().iter().map(|t| t.order).collect::<Vec<_>>(), (0..8).collect::<Vec<u32>>());
    assert_eq!(rig.state(&format!("split_tab/{second}/rule")), None, "a removed tab has no path");
    let rig = rig.restart();
    assert!(!ids(&rig.core).contains(&second), "the removal is on disk");
}
// broken copy: store/src/app_settings.rs `split_tab_add`: `rows.len() as i64` (the new tab's place) becomes `0` (an added tab goes first).
// broken copy: store/src/app_settings.rs `split_tab_set_enabled`: `ctx.tx.execute("UPDATE split_tab SET enabled` is made a no-op (`SET enabled = enabled`): the tab stays in the counts.
// broken copy: store/src/app_settings.rs `split_tab_reorder`: `params![id, i as i64, now_ms]` becomes `params![id, 0_i64, now_ms]` (the order is not kept).

#[test]
fn the_split_tab_refusals_are_in_the_test_engines_words_and_write_nothing() {
    let rig = Rig::new();
    let before = rig.core.split_tabs();
    let add = |name: &str, rule: SplitTabRule| rig.core.run(Command::AddSplitTab { name: name.into(), rule });
    assert_eq!(invalid(add("   ", SplitTabRule::Sender { address: "a@b.test".into() })), ("name".into(), "a tab needs a name".into()));
    assert_eq!(invalid(add("updates", SplitTabRule::Sender { address: "a@b.test".into() })), ("name".into(), "a tab called “updates” exists already".into()), "a name taken, in any case");
    assert_eq!(invalid(add("X", SplitTabRule::Builtin { name: "Updates".into() })).1, "pick a rule: sender, domain, account, label or decision category");
    assert_eq!(invalid(add("X", SplitTabRule::Sender { address: "nobody".into() })).1, "a sender is a full address, like orders@example.com");
    assert_eq!(invalid(add("X", SplitTabRule::Sender { address: "@shop.test".into() })).1, "a sender is a full address, like orders@example.com");
    assert_eq!(invalid(add("X", SplitTabRule::Domain { domain: "shop".into() })).1, "a domain looks like example.com");
    assert_eq!(invalid(add("X", SplitTabRule::Domain { domain: "a@shop.test".into() })).1, "a domain looks like example.com");
    assert_eq!(invalid(add("X", SplitTabRule::DecisionCategory { category: " ".into() })).1, "name the decision category");
    assert_eq!(not_found(add("X", SplitTabRule::Account { account_id: "nobody".into() })), ("account".into(), "nobody".into()));
    assert_eq!(not_found(add("X", SplitTabRule::Label { label_id: "nolabel".into() })), ("label".into(), "nolabel".into()));
    assert_eq!(not_found(rig.core.run(Command::RenameSplitTab { tab_id: "nope".into(), name: "A".into() })), ("split tab".into(), "nope".into()));
    assert_eq!(invalid(rig.core.run(Command::RenameSplitTab { tab_id: "updates".into(), name: " ".into() })), ("name".into(), "a tab needs a name".into()));
    let mut short = ids(&rig.core);
    short.pop();
    assert_eq!(invalid(rig.core.run(Command::ReorderSplitTabs { ids: short })), ("ids".into(), "the order must list every tab once".into()));
    let mut twice = ids(&rig.core);
    twice[2] = twice[1].clone();   // a tab twice and another missing (Primary stays first: the contract's door answers first for a view out of its place)
    assert_eq!(invalid(rig.core.run(Command::ReorderSplitTabs { ids: twice })).1, "the order must list every tab once");
    let mut moved = ids(&rig.core);
    moved.rotate_left(1);   // Primary is moved from the first place: the views stay first
    assert_eq!(invalid(rig.core.run(Command::ReorderSplitTabs { ids: moved })), ("view_not_a_label".into(), "Primary is a view of the inbox, not a label, so it cannot be reordered: the views stay first.".into()));
    assert_eq!(not_found(rig.core.run(Command::SetSplitTabEnabled { id: "nope".into(), on: false })), ("split tab".into(), "nope".into()));
    assert_eq!(not_found(rig.core.run(Command::RemoveSplitTab { id: "nope".into() })), ("split tab".into(), "nope".into()));
    assert_eq!(invalid(rig.core.run(Command::RemoveSplitTab { id: "updates".into() })), ("id".into(), "the default tabs cannot be removed: hide a tab instead".into()));
    assert_eq!(rig.core.split_tabs(), before, "no refusal changed a tab");
    // the last shown tab stays
    for id in ids(&rig.core).into_iter().skip(1) { rig.run(Command::SetSplitTabEnabled { id, on: false }); }
    // Primary is a view: the command cannot hide it (delta 16, the contract's door), so the store's own guard is asked straight
    assert_eq!(invalid(rig.core.run(Command::SetSplitTabEnabled { id: "primary".into(), on: false })), ("view_not_a_label".into(), "Primary is a view of the inbox, not a label, so it cannot be hidden.".into()));
    match rig.core.store().split_tab_set_enabled("primary".into(), false, 1) {
        Err(store::app_settings::Refused::Invalid { field, words }) => assert_eq!((field, words.as_str()), ("enabled", "at least one tab has to stay")),
        other => panic!("expected the store's last-tab refusal, got {other:?}"),
    }
    assert_eq!(rig.state("split_tab/primary/enabled"), s("true"));
}
// broken copy: store/src/app_settings.rs `split_tab_remove`: the `Builtin` test is removed (a default tab can be removed).
// broken copy: store/src/app_settings.rs `split_tab_set_enabled`: `== 1` in the last-shown test becomes `== 0` (the last shown tab can be hidden).
// broken copy: store/src/app_settings.rs `split_tab_add`: the duplicate-name test is removed (a taken name is accepted).

#[test]
fn restoring_the_default_tabs_gives_back_exactly_what_the_first_launch_wrote() {
    let rig = Rig::new();
    let shipped = rig.core.store().split_tab_rows().unwrap();
    assert!(shipped.iter().all(|r| r.updated_at_ms == 0), "the seed writes time 0");
    rig.run(Command::AddSplitTab { name: "Shop".into(), rule: SplitTabRule::Domain { domain: "shop.test".into() } });
    rig.run(Command::RenameSplitTab { tab_id: "updates".into(), name: "Inbox".into() });   // Primary is a view and cannot be renamed (delta 16)
    rig.run(Command::SetSplitTabEnabled { id: "social".into(), on: false });
    let mut order = ids(&rig.core);
    order[1..].reverse();   // Primary stays first
    rig.run(Command::ReorderSplitTabs { ids: order });
    assert_ne!(rig.core.store().split_tab_rows().unwrap(), shipped);
    rig.run(Command::RestoreDefaultSplitTabs);
    assert_eq!(rig.core.store().split_tab_rows().unwrap(), shipped, "the six rows, their names, order, rules, shown, and time 0: byte for byte what the first launch wrote");
    assert_eq!(rig.state("split_tab/count"), s("6"));
    assert_eq!(rig.state("split_tab/primary/name"), s("\"Primary\""));
    let rig = rig.restart();
    assert_eq!(rig.core.store().split_tab_rows().unwrap(), shipped, "and it is on disk");
}
// broken copy: store/src/app_settings.rs `split_tab_restore_defaults`: `ctx.tx.execute("DELETE FROM split_tab", [])?;` is removed (the owner's tabs stay).

// ---------------------------------------------------------------- the key removal and the mail consent

fn keep_key(rig: &Rig, provider: &str, name: &str) -> KeychainRef {
    let r = StoreSecret::execute(&*rig.keys, SecretKind::ProviderCredential, provider, &Secret::new(KEY.as_bytes().to_vec())).unwrap();
    rig.core.store().ai_provider_connect(provider.into(), name.into(), vec!["a-model".into()], format!("{}\u{1f}{}", r.service, r.account), 5).unwrap();
    r
}

#[test]
fn removing_a_provider_key_deletes_the_keychain_item_and_the_row_and_prints_no_key() {
    let rig = Rig::new();
    let r = keep_key(&rig, "anthropic", "Anthropic");
    assert_eq!((rig.state("provider_credential/anthropic/stored"), rig.state("ai_provider/anthropic/state")), (s("true"), s("\"connected\"")));
    timed("RemoveProviderKey", || rig.run(Command::RemoveProviderKey { provider_id: "anthropic".into() }));
    assert!(ReadSecret::execute(&*rig.keys, &r).is_err(), "the Keychain item is gone");
    assert!(rig.core.store().ai_provider_row("anthropic").unwrap().is_none(), "the provider row is gone");
    assert_eq!((rig.state("provider_credential/anthropic/stored"), rig.state("ai_provider/anthropic/state")), (s("false"), s("\"not connected\"")));
    assert!(!rig.core.ai_providers().iter().find(|p| p.id == "anthropic").unwrap().connected);
    rig.run(Command::RemoveProviderKey { provider_id: "anthropic".into() }); // a provider with no key is not an error
    assert_eq!(not_found(rig.core.run(Command::RemoveProviderKey { provider_id: "nowhere".into() })), ("provider".into(), "nowhere".into()));
    let log = std::fs::read_to_string(rig.dir.path().join("core.log")).unwrap_or_default();
    assert!(!log.contains(KEY), "a key is never logged");
    let rig = rig.restart();
    assert_eq!(rig.state("ai_provider/anthropic/state"), s("\"not connected\""), "the removal is on disk");
}
// broken copy: engine/src/ai_settings.rs `is_command`: `| Command::RemoveProviderKey { .. }` is removed (the command is refused as not built yet).
// broken copy: engine/src/ai_settings.rs `remove_provider_key`: `DeleteSecret::execute(&**keychain, &r)?;` is removed (the Keychain item stays).

/// A secret store whose Keychain refuses to delete (a locked Keychain).
struct Stubborn(MemoryKeychain);
impl StoreSecret for Stubborn {
    fn execute(&self, kind: SecretKind, name: &str, secret: &Secret) -> Result<KeychainRef, CoreError> { StoreSecret::execute(&self.0, kind, name, secret) }
}
impl ReadSecret for Stubborn {
    fn execute(&self, r: &KeychainRef) -> Result<Secret, CoreError> { ReadSecret::execute(&self.0, r) }
}
impl DeleteSecret for Stubborn {
    fn execute(&self, _r: &KeychainRef) -> Result<(), CoreError> { Err(CoreError::Unexpected { reason_words: "The Keychain is locked.".into() }) }
}

#[test]
fn a_keychain_that_refuses_the_delete_leaves_the_row_so_the_key_is_not_orphaned() {
    let dir = tempfile::tempdir().unwrap();
    let keys = Arc::new(Stubborn(MemoryKeychain::new(Namespace::rig())));
    let detector: Box<dyn engine::Detect> = Box::new(autoconfig::Detector::new(FakeHttp::default(), FakeDns::default(), FakeProbe::default()));
    let core = RustCore::with_parts(Store::open(&dir.path().join("whoosh.sqlite")).unwrap(), keys.clone(), detector, TestTransport::Real);
    let r = StoreSecret::execute(&*keys, SecretKind::ProviderCredential, "openai", &Secret::new(KEY.as_bytes().to_vec())).unwrap();
    core.store().ai_provider_connect("openai".into(), "OpenAI".into(), vec![], format!("{}\u{1f}{}", r.service, r.account), 5).unwrap();
    let err = core.run(Command::RemoveProviderKey { provider_id: "openai".into() }).err().expect("the Keychain refused, so the command is refused");
    assert!(matches!(&err, CoreError::Unexpected { reason_words } if reason_words == "The Keychain is locked."), "{err:?}");
    assert!(!format!("{err:?}").contains(KEY), "the refusal does not carry the key");
    assert!(core.store().ai_provider_row("openai").unwrap().is_some(), "the row stays: the owner can try again");
    assert!(ReadSecret::execute(&*keys, &r).is_ok(), "the key is still in the Keychain, and a row points to it");
}
// broken copy: engine/src/ai_settings.rs `remove_provider_key`: the `ai_provider_forget` is moved above the `DeleteSecret` (the row goes first, the key is orphaned when the Keychain refuses).

#[test]
fn the_owners_allow_is_kept_per_provider_read_back_and_survives_a_restart_and_a_key_removal() {
    let rig = Rig::new();
    assert_eq!((rig.state("agent_consent/anthropic"), rig.state("agent_consent/jev"), rig.state("agent_consent/nowhere")), (s("false"), s("false"), None));
    timed("GiveAgentConsent", || rig.run(Command::GiveAgentConsent { provider_id: "anthropic".into() }));
    assert_eq!((rig.state("agent_consent/anthropic"), rig.state("agent_consent/openai")), (s("true"), s("false")));
    let at = |core: &RustCore, id: &str| core.ai_providers().into_iter().find(|p| p.id == id).unwrap().mail_consent_at_ms;
    let first = at(&rig.core, "anthropic").expect("the provider row carries the time of the Allow");
    assert_eq!(at(&rig.core, "openai"), None);
    assert!(!rig.core.ai_providers().iter().find(|p| p.id == "anthropic").unwrap().connected, "consent is not a key: the provider is still not connected");
    std::thread::sleep(Duration::from_millis(5));
    rig.run(Command::GiveAgentConsent { provider_id: "anthropic".into() });
    assert_eq!(at(&rig.core, "anthropic"), Some(first), "a second Allow keeps the first time");
    assert_eq!(not_found(rig.core.run(Command::GiveAgentConsent { provider_id: "nowhere".into() })), ("provider".into(), "nowhere".into()));
    rig.run(Command::GiveAgentConsent { provider_id: "jev".into() });
    assert_eq!(rig.state("agent_consent/jev"), s("true"));
    assert!(rig.core.ai_providers().iter().all(|p| p.id != "jev"), "a consent given before its key does not make a Jev row appear");
    // the key arrives and goes: the Allow stays (the test engine's forgetKeyLocked keeps it)
    keep_key(&rig, "anthropic", "Anthropic");
    assert_eq!(at(&rig.core, "anthropic"), Some(first), "saving a key keeps the Allow");
    rig.run(Command::RemoveProviderKey { provider_id: "anthropic".into() });
    assert_eq!(rig.state("agent_consent/anthropic"), s("true"), "removing the key keeps the Allow");
    assert_eq!(rig.state("ai_provider/anthropic/state"), s("\"not connected\""));
    assert!(rig.core.store().ai_provider_row("anthropic").unwrap().is_some_and(|r| r.keychain_ref.is_none() && !r.connected));
    let rig = rig.restart();
    assert_eq!(at(&rig.core, "anthropic"), Some(first), "the Allow is on disk");
    assert_eq!(rig.state("agent_consent/jev"), s("true"));
}
// broken copy: engine/src/ai_settings.rs `is_command`: `| Command::GiveAgentConsent { .. }` is removed (the command is refused as not built yet).
// broken copy: store/src/app_settings.rs `agent_consent_give`: `coalesce(mail_consent_at_ms, ?3)` becomes `?3` (a second Allow moves the time).

// ---------------------------------------------------------------- the time, and the network

#[test]
fn every_settings_command_answers_under_50_ms_on_a_store_of_2000_mails() {
    let rig = Rig::new();
    fill_inbox(&rig.core, &rig.accounts[0]);
    let a = rig.accounts[0].clone();
    timed("SetNotchSetting", || rig.run(Command::SetNotchSetting { setting: NotchSetting { show: false, buttons: vec!["delete".into()], hidden_in_screen_share: true, follow_focus: true, list_shows: NotchList::Unread } }));
    timed("SetNotificationSetting", || rig.run(Command::SetNotificationSetting { setting: NotificationSetting { account_id: a.clone(), enabled: false, delivery: Some(NotificationDelivery::Notifications) } }));
    timed("SetNotification", || rig.run(Command::SetNotification { account_id: a.clone(), enabled: true }));
    timed("SetShortcutPreset", || rig.run(Command::SetShortcutPreset { preset: "GotEmail".into() }));
    let id = timed("AddSplitTab", || rig.run(Command::AddSplitTab { name: "Shop".into(), rule: SplitTabRule::Domain { domain: "shop3.test".into() } })).created_id.unwrap();
    timed("RenameSplitTab", || rig.run(Command::RenameSplitTab { tab_id: id.clone(), name: "Shops".into() }));
    let mut order = ids(&rig.core);
    order[1..].reverse();   // Primary stays first
    timed("ReorderSplitTabs", || rig.run(Command::ReorderSplitTabs { ids: order }));
    timed("SetSplitTabEnabled", || rig.run(Command::SetSplitTabEnabled { id: "updates".into(), on: false }));
    timed("RemoveSplitTab", || rig.run(Command::RemoveSplitTab { id }));
    timed("RestoreDefaultSplitTabs", || rig.run(Command::RestoreDefaultSplitTabs));
    // the reads the app makes after a write are as fast
    timed("the six reads", || {
        rig.core.notch_setting(); rig.core.notification_settings(); rig.core.shortcut_preset(); rig.core.split_tabs();
        rig.state("split_tab/updates/rule"); rig.state("notification_setting/off");
    });
}
// broken copy: engine/src/settings_cmds.rs `run`: a `std::thread::sleep(std::time::Duration::from_millis(60));` before the match (every write is over 50 ms).

#[test]
fn no_settings_write_opens_a_socket_or_waits_for_the_network() {
    // A settings write is a store write: the files that serve it name no network crate, no socket and no address.
    for (file, text) in [("engine/src/settings_cmds.rs", include_str!("../src/settings_cmds.rs")), ("store/src/app_settings.rs", include_str!("../../store/src/app_settings.rs"))] {
        for word in ["TcpStream", "UdpSocket", "reqwest", "hyper", "tokio::", "http://", "https://", "std::net", "ureq"] {
            let hits: Vec<(usize, &str)> = text.lines().enumerate().filter(|(_, l)| !l.trim_start().starts_with("//") && l.contains(word)).collect();
            assert!(hits.is_empty(), "{file} names `{word}`: {hits:?}");
        }
    }
}
// broken copy: engine/src/settings_cmds.rs `run`: a line `let _ = std::net::TcpStream::connect("127.0.0.1:9");` before the match.
