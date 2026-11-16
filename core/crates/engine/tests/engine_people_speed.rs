//! gap-engine-tests (item c92234f0): "the read is measured under 50 ms". Over the rig's production core, 5,000 received mails from 1,500
//! distinct senders and 500 Sent mails of 2 recipients each are stored; the test waits for `people/ready`, then times the three reads the
//! search screen makes, `people/prefix/<3 letters>`, `contact(<address>)` and `contacts()`, each cold (the first read after the build) and
//! warm (the next), prints one line per read, and fails when a read takes 50 ms or more (docs/CONSTANTS.md INTERACTION_COMMIT_MS).
//!
//! Line printed per read (`-- --nocapture`; run it in the release profile): `people read, <name>: N people from M mails in X ms`.
//! Red-first broken copy: people.rs `prefix`, a `std::thread::sleep(std::time::Duration::from_millis(60));` before
//! `Self::ranked(b.people.values().filter(|p| p.starts_with(&folded)))` -> the two prefix reads fail on the time.
//! If `contacts()` over the 1,500 people is over the budget that is a finding for the engine (cap or page it), never a reason to move the budget.
use std::time::{Duration, Instant};

use contracts::*;
use engine::RustCore;
use store::NewMessage;

const USER: &str = "owner@example.test";
const RECEIVED: usize = 5_000;
const SENDERS: usize = 1_500;
const SENT: usize = 500;
/// docs/CONSTANTS.md INTERACTION_COMMIT_MS
const BUDGET_MS: u128 = 50;

fn config() -> AccountConfig {
    AccountConfig {
        imap_host: "127.0.0.1".into(), imap_port: 1, imap_security: ConnectionSecurity::Ssl, smtp_host: "127.0.0.1".into(), smtp_port: 1,
        smtp_security: ConnectionSecurity::Starttls, username: USER.into(), smtp_username: USER.into(), auth_method: AuthMethod::Password, source: DetectionSource::Manual,
    }
}

fn mail(account: &str, id: String, folder: &str, sender: (String, String), to: Vec<String>, at: i64) -> NewMessage {
    NewMessage {
        message: Message {
            id: id.clone(), thread_id: format!("{id}:thread"), account_id: account.into(), sender_name: sender.0, from_address: sender.1,
            to, subject: format!("Subject {id}"), preview: String::new(), received_at_ms: at, read: false, archived: false,
            folder: folder.into(), attachment_names: vec![], login_code: None, trackers_blocked: 0, list_unsubscribe: vec![], list_unsubscribe_one_click: false,
            reply_to_name: None, cc: vec![], bcc: vec![], is_outgoing: false,
        },
        remote_id: None, body_text: None, body_html: None, has_list_header: false, size_bytes: None, tab_id: None,
    }
}

#[test]
fn the_people_reads_answer_under_50_ms_on_2000_or_more_mails() {
    let dir = tempfile::tempdir().unwrap();
    let core = RustCore::open_production(dir.path().to_str().unwrap(), true).expect("the rig core opens");
    let store = core.store();
    let a = store.add_account(USER, "Owner", MailProvider::Imap, "svc\u{1f}acct", false, config(), 1).unwrap();
    let mut batch = Vec::with_capacity(RECEIVED + SENT);
    for i in 0..RECEIVED {
        let k = i % SENDERS;
        batch.push(mail(&a, format!("r{i}"), "INBOX", (format!("Person{k} Family{k}"), format!("person{k}@firm{}.test", k % 50)), vec![USER.to_string()], 1_000 + i as i64));
    }
    for j in 0..SENT {
        let to = vec![format!("Friend{j} Name{j} <friend{j}@client{}.test>", j % 25), format!("person{}@firm{}.test", (j * 3) % SENDERS, ((j * 3) % SENDERS) % 50)];
        batch.push(mail(&a, format!("s{j}"), "Sent", ("Owner".to_string(), USER.to_string()), to, 100_000 + j as i64));
    }
    let mails = batch.len();
    store.insert_messages(batch).unwrap();

    let t0 = Instant::now();
    while core.state("people/ready".into()).as_deref() != Some("true") && t0.elapsed() < Duration::from_secs(120) { std::thread::sleep(Duration::from_millis(20)); }
    assert_eq!(core.state("people/ready".into()).as_deref(), Some("true"), "the first build ended");
    let people: usize = core.state("people/count".into()).unwrap().parse().unwrap();
    assert!(people >= SENDERS, "the list holds the {SENDERS} senders and the Sent recipients: {people}");

    let mut worst = 0u128;
    for pass in ["cold", "warm"] {
        let t = Instant::now();
        let raw = core.state("people/prefix/per".into()).expect("people/prefix is served");
        let ms = t.elapsed().as_millis();
        let rows: Vec<serde_json::Value> = serde_json::from_str(&raw).unwrap();
        assert!(!rows.is_empty(), "the prefix read found people");
        println!("people read, prefix {pass}: {people} people from {mails} mails in {ms} ms");
        assert!(ms < BUDGET_MS, "the prefix read ({pass}) took {ms} ms, over {BUDGET_MS} ms");
        worst = worst.max(ms);

        let t = Instant::now();
        let one = core.contact("person7@firm7.test".into());
        let ms = t.elapsed().as_millis();
        assert!(one.is_some(), "contact(address) found the person");
        println!("people read, contact {pass}: {people} people from {mails} mails in {ms} ms");
        assert!(ms < BUDGET_MS, "contact ({pass}) took {ms} ms, over {BUDGET_MS} ms");
        worst = worst.max(ms);

        let t = Instant::now();
        let all = core.contacts();
        let ms = t.elapsed().as_millis();
        assert_eq!(all.len(), people, "contacts() lists every person");
        println!("people read, contacts {pass}: {people} people from {mails} mails in {ms} ms");
        assert!(ms < BUDGET_MS, "contacts() ({pass}) took {ms} ms, over {BUDGET_MS} ms");
        worst = worst.max(ms);
    }
    println!("people reads, slowest of the six: {worst} ms (budget {BUDGET_MS} ms)");
}
