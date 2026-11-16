//! gap-engine-tests (item f2dd23de): the two rule previews of the real core, `rule_match_count` (a smart-folder rule in words) and
//! `split_tab_rule_match_count` (a split-tab rule), answer the real count from the store. Both answered a constant zero before.
//! Each test runs over the rig's production core. Red-first broken copy of each test: put `0` back in the function
//! (rust_core.rs `rule_match_count` returning `0`, `split_tab_rule_match_count` returning `0`): every assertion of a non-zero count fails.
//! The speed test prints one line, `rule counts, <name>: N matches from M mails in X ms`, and fails over 50 ms (INTERACTION_COMMIT_MS).
//! Run it with `-- --nocapture` and in the release profile.
use std::time::{Duration, Instant};

use contracts::*;
use engine::RustCore;
use store::NewMessage;

const USER: &str = "owner@example.test";
const WAIT: Duration = Duration::from_secs(120);
/// docs/CONSTANTS.md INTERACTION_COMMIT_MS
const BUDGET_MS: u128 = 50;

fn config() -> AccountConfig {
    AccountConfig {
        imap_host: "127.0.0.1".into(), imap_port: 1, imap_security: ConnectionSecurity::Ssl, smtp_host: "127.0.0.1".into(), smtp_port: 1,
        smtp_security: ConnectionSecurity::Starttls, username: USER.into(), smtp_username: USER.into(), auth_method: AuthMethod::Password, source: DetectionSource::Manual,
    }
}

#[allow(clippy::too_many_arguments)]
fn mail(account: &str, id: &str, folder: &str, sender: (&str, &str), to: &[&str], subject: &str, body: &str, at: i64) -> NewMessage {
    NewMessage {
        message: Message {
            id: id.into(), thread_id: format!("{id}:thread"), account_id: account.into(), sender_name: sender.0.into(), from_address: sender.1.into(),
            to: to.iter().map(|s| s.to_string()).collect(), subject: subject.into(), preview: body.into(), received_at_ms: at, read: false, archived: false,
            folder: folder.into(), attachment_names: vec![], login_code: None, trackers_blocked: 0, list_unsubscribe: vec![], list_unsubscribe_one_click: false,
            reply_to_name: None, cc: vec![], bcc: vec![], is_outgoing: false,
        },
        remote_id: None, body_text: Some(body.into()), body_html: None, has_list_header: false, size_bytes: None, tab_id: None,
    }
}

fn index(core: &RustCore, ids: &[String]) {
    let bg = core.background();
    bg.reindex(ids);
    assert!(bg.settle_index(WAIT), "the indexer finished");
}

/// m1, m2 from Zoe at acme.test, m3 from Bob at other.test (all INBOX); m4 the owner's Sent copy that holds the word too; m5 archived.
fn core_with_mail(dir: &std::path::Path) -> (RustCore, String) {
    let core = RustCore::open_production(dir.to_str().unwrap(), true).expect("the rig core opens");
    let store = core.store();
    let a = store.add_account(USER, "Owner", MailProvider::Imap, "svc\u{1f}acct", false, config(), 1).unwrap();
    let mut archived = mail(&a, "m5", "INBOX", ("Cleo Fox", "cleo@acme.test"), &[USER], "Old invoice", "an invoice from long ago", 5_000);
    archived.message.archived = true;
    store.insert_messages(vec![
        mail(&a, "m1", "INBOX", ("Zoe Martin", "zoe@acme.test"), &[USER], "Invoice for March", "please pay the invoice by Friday", 1_000),
        mail(&a, "m2", "INBOX", ("Zoe Martin", "zoe@acme.test"), &[USER], "Lunch", "salmon at noon", 2_000),
        mail(&a, "m3", "INBOX", ("Bob Stone", "bob@other.test"), &[USER], "Reminder", "the invoice is overdue", 3_000),
        mail(&a, "m4", "Sent", ("Owner", USER), &["bob@other.test"], "Re: Reminder", "the invoice is paid", 4_000),
        archived,
    ]).unwrap();
    index(&core, &["m1", "m2", "m3", "m4", "m5"].map(String::from));
    (core, a)
}

/// Broken copy: `rule_match_count` returning `0` -> every non-zero assertion fails.
#[test]
fn a_words_rule_counts_the_stored_inbox_mail_it_matches() {
    let dir = tempfile::tempdir().unwrap();
    let (core, _) = core_with_mail(dir.path());
    assert_eq!(core.rule_match_count("invoice".into()), 2, "m1 and m3: the Sent copy and the archived mail are not in the inbox");
    assert_eq!(core.rule_match_count("salmon".into()), 1, "one mail holds the word");
    assert_eq!(core.rule_match_count("from:zoe".into()), 2, "a filter alone counts too");
    assert_eq!(core.rule_match_count("from:zoe invoice".into()), 1, "the filter and the word together");
    assert_eq!(core.rule_match_count("zzzunheardof".into()), 0, "a rule that matches no stored mail says 0");
    assert_eq!(core.rule_match_count("   ".into()), 0, "a blank rule matches nothing");
}

/// Broken copy: `split_tab_rule_match_count` returning `0`.
#[test]
fn a_split_tab_rule_counts_the_inbox_mail_it_would_hold() {
    let dir = tempfile::tempdir().unwrap();
    let (core, account) = core_with_mail(dir.path());
    let store = core.store();
    let work = store.label_create(store::NewLabel { id: None, name: "Work".into(), parent_id: None, colour: None, account_ids: vec![], now_ms: 1 }).unwrap();
    store.label_assign(work.id.clone(), vec!["m1".to_string(), "m3".to_string(), "m4".to_string(), "m5".to_string()], 1).unwrap();
    let count = |rule: SplitTabRule| core.split_tab_rule_match_count(rule);
    assert_eq!(count(SplitTabRule::Sender { address: "zoe@acme.test".into() }), 2, "a sender");
    assert_eq!(count(SplitTabRule::Domain { domain: "other.test".into() }), 1, "a domain");
    assert_eq!(count(SplitTabRule::Account { account_id: account }), 4, "an account: the inbound INBOX mail, as the labels screen counts it");
    assert_eq!(count(SplitTabRule::Builtin { name: "primary".into() }), 3, "a built-in tab: its own list's rows (m1 m2 m3; not Sent, not archived)");
    assert_eq!(count(SplitTabRule::Label { label_id: work.id }), 2, "a label: the inbox mail filed under it (m1, m3)");
    assert_eq!(count(SplitTabRule::Sender { address: "nobody@nowhere.test".into() }), 0, "a rule that holds nothing says 0");
    assert_eq!(count(SplitTabRule::DecisionCategory { category: "no-such-tab".into() }), 0);
}

/// The reads stay under 50 ms on 2,000 stored mails (about 20 senders, one mail in ten holds the word). Prints one line per read.
/// Broken copy: a `std::thread::sleep(std::time::Duration::from_millis(60));` at the head of `rule_counts::words_count`.
#[test]
fn the_rule_counts_answer_under_50_ms_on_2000_mails() {
    const MAILS: usize = 2_000;
    let dir = tempfile::tempdir().unwrap();
    let core = RustCore::open_production(dir.path().to_str().unwrap(), true).expect("the rig core opens");
    let store = core.store();
    let a = store.add_account(USER, "Owner", MailProvider::Imap, "svc\u{1f}acct", false, config(), 1).unwrap();
    let batch: Vec<NewMessage> = (0..MAILS).map(|i| {
        let from = format!("sender{}@firm{}.test", i % 20, i % 4);
        let body = if i % 10 == 0 { format!("note {i}: the quarterly invoice is attached") } else { format!("note {i}: nothing to pay here") };
        mail(&a, &format!("s{i}"), "INBOX", (&format!("Sender {}", i % 20), &from), &[USER], &format!("Subject {i}"), &body, 10_000 + i as i64)
    }).collect();
    let ids: Vec<String> = batch.iter().map(|m| m.message.id.clone()).collect();
    store.insert_messages(batch).unwrap();
    index(&core, &ids);
    let timed = |name: &str, run: &dyn Fn() -> u32, want: u32| {
        let t = Instant::now();
        let n = run();
        let ms = t.elapsed().as_millis();
        println!("rule counts, {name}: {n} matches from {MAILS} mails in {ms} ms");
        assert_eq!(n, want, "{name}");
        assert!(ms < BUDGET_MS, "{name} took {ms} ms, over {BUDGET_MS} ms");
    };
    for pass in ["cold", "warm"] {
        timed(&format!("words rule {pass}"), &|| core.rule_match_count("invoice".into()), (MAILS / 10) as u32);
        timed(&format!("sender rule {pass}"), &|| core.split_tab_rule_match_count(SplitTabRule::Sender { address: "sender3@firm3.test".into() }), (MAILS / 20) as u32);
        timed(&format!("domain rule {pass}"), &|| core.split_tab_rule_match_count(SplitTabRule::Domain { domain: "firm1.test".into() }), (MAILS / 4) as u32);
        timed(&format!("tab rule {pass}"), &|| core.split_tab_rule_match_count(SplitTabRule::Builtin { name: "primary".into() }), MAILS as u32);
    }
}
