//! fix-core (docs/lanes/fix-core.md): the rig-only hooks the hidden real-core slices need (`WHOOSH_CORE_CHECK_CONDITIONS` and the mutants
//! `crawl-never-runs` and `rule-counts-zero`), read by `RustCore::open_production(.., rig = true)` and by nothing else (a core opened with rig false is not
//! opened here: it would open the owner's Keychain; the guard is the `rig.then(..)` and `if !rig { Mutant::None }` lines). One test, because the
//! variables are the process's own and the cores are opened one after the other. Each part names its broken copy (one edit, red first):
//!   conditions   crawl.rs `conditions_from_check`: `c.battery_percent = Some(n);` -> `c.battery_percent = None;` (no pause reads Battery)
//!   crawl-never-runs: rust_core.rs `if mutant == Mutant::CrawlNeverRuns { core.deps.bodies.hold_crawl(true); }` -> `if false {` (the crawl is not held)
//!   rule-counts-zero: rust_core.rs `rule_set_count`, the line `if rule_counts::zeroed() {` -> `if false {` (the count is real)
use contracts::*;
use engine::RustCore;

const CONDITIONS: &str = "WHOOSH_CORE_CHECK_CONDITIONS";
const MUTANT: &str = "WHOOSH_CORE_MUTANT";

fn open(rig: bool) -> (RustCore, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    let core = RustCore::open_production(dir.path().to_str().unwrap(), rig).expect("the core opens");
    (core, dir)
}
fn pause(core: &RustCore) -> String { format!("{:?}", core.background().paused()) }

#[test]
fn the_rig_hooks_set_the_conditions_and_the_two_mutants_and_the_owners_copy_reads_none_of_them() {
    std::env::remove_var(MUTANT);
    std::env::remove_var(CONDITIONS);
    let (plain, _d0) = open(true);
    assert_eq!(pause(&plain), "None", "no variable: the crawl is not paused");

    std::env::set_var(CONDITIONS, "battery:15");
    let (battery, _d1) = open(true);
    assert_eq!(pause(&battery), "Some(Battery)", "on battery at 15%, under ATTACHMENT_INDEX_BATTERY_MIN: paused for the battery");
    std::env::set_var(CONDITIONS, "battery:15,heat");
    let (heat, _d2) = open(true);
    assert_eq!(pause(&heat), "Some(Heat)", "heat is named before the battery");
    std::env::set_var(CONDITIONS, "battery:90");
    let (full, _d3) = open(true);
    assert_eq!(pause(&full), "None", "on battery at 90%: not paused");
    for refused in ["battery:abc", "battery:101", "cold", "", "battery:15,cold"] {
        std::env::set_var(CONDITIONS, refused);
        let (core, _d) = open(true);
        assert_eq!(pause(&core), "None", "{refused:?} is no condition: the copy keeps its real ones");
    }
    std::env::remove_var(CONDITIONS);

    // crawl-never-runs: the crawl is held for good
    std::env::set_var(MUTANT, "crawl-never-runs");
    let (never, _d5) = open(true);
    assert_eq!(pause(&never), "Some(Held)", "the mutant holds the crawl");

    // rule-counts-zero: the label rule count answers 0 for a rule that holds mail, and the real count is back without the mutant
    let seeded = |core: &RustCore| {
        let config = AccountConfig {
            imap_host: "127.0.0.1".into(), imap_port: 1, imap_security: ConnectionSecurity::Ssl, smtp_host: "127.0.0.1".into(), smtp_port: 1,
            smtp_security: ConnectionSecurity::Starttls, username: "owner@example.test".into(), smtp_username: "owner@example.test".into(), auth_method: AuthMethod::Password, source: DetectionSource::Manual,
        };
        let id = core.store().add_account("owner@example.test", "Owner", MailProvider::Imap, "svc\u{1f}acct", false, config, 1).unwrap();
        core.store().insert_messages(vec![store::NewMessage {
            message: Message {
                id: "m1".into(), thread_id: "t1".into(), account_id: id.clone(), sender_name: "Them".into(), from_address: "them@ext.test".into(), to: vec!["owner@example.test".into()],
                subject: "Hello".into(), preview: "Hello".into(), received_at_ms: 1_000, read: false, archived: false, folder: "INBOX".into(),
                attachment_names: vec![], login_code: None, trackers_blocked: 0, list_unsubscribe: vec![], list_unsubscribe_one_click: false,
                reply_to_name: None, cc: vec![], bcc: vec![], is_outgoing: false,
            },
            remote_id: None, body_text: None, body_html: None, has_list_header: false, size_bytes: Some(10), tab_id: None,
        }]).unwrap();
        RuleSet { conditions: vec![RuleCondition::Account { account_id: id }], match_mode: MatchMode::Any }
    };
    std::env::set_var(MUTANT, "rule-counts-zero");
    let (zero, _d7) = open(true);
    let zero_rule = seeded(&zero);
    assert_eq!(zero.rule_set_count(zero_rule).count, 0, "the mutant: the label rule count answers 0");
    std::env::remove_var(MUTANT);
    let (real, _d8) = open(true);
    let real_rule = seeded(&real);
    assert_eq!(real.rule_set_count(real_rule).count, 1, "without the mutant: the real count");
}
