//! search-forms part 3: the people of all stored mail. `contacts()` lists every address that sent the owner mail or that the owner
//! sent mail to; the state paths `people/prefix/<letters>` and `people/address/<address>` give each person's name, address,
//! from_count and to_count. Each test runs over the rig's production core. On c83fce8c each fails: `contacts()` answered an empty
//! list and no `people/` path was served.
use std::time::Duration;

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

fn mail(account: &str, id: &str, folder: &str, sender: (&str, &str), to: &[&str], at: i64) -> NewMessage {
    NewMessage {
        message: Message {
            id: id.into(), thread_id: format!("{id}:thread"), account_id: account.into(), sender_name: sender.0.into(), from_address: sender.1.into(),
            to: to.iter().map(|s| s.to_string()).collect(), subject: format!("Subject {id}"), preview: String::new(), received_at_ms: at, read: false, archived: false,
            folder: folder.into(), attachment_names: vec![], login_code: None, trackers_blocked: 0, list_unsubscribe: vec![], list_unsubscribe_one_click: false,
            reply_to_name: None, cc: vec![], bcc: vec![], is_outgoing: false,
        },
        remote_id: None, body_text: None, body_html: None, has_list_header: false, size_bytes: None, tab_id: None,
    }
}

/// Two mails from Zoë Martin, one from Bob, and the owner's own mail in Sent to Jean Dupont (who never wrote).
fn core_with_mail(dir: &std::path::Path) -> (RustCore, String) {
    let core = RustCore::open_production(dir.to_str().unwrap(), true).expect("the rig core opens");
    let store = core.store();
    let a = store.add_account(USER, "Owner", MailProvider::Imap, "svc\u{1f}acct", false, config(), 1).unwrap();
    store.insert_messages(vec![
        mail(&a, "m1", "INBOX", ("Zoë Martin", "zoe@acme.test"), &[USER], 1_000),
        mail(&a, "m2", "INBOX", ("Zoë Martin", "Zoe@Acme.test"), &[USER], 2_000),
        mail(&a, "m3", "INBOX", ("Bob Stone", "bob@other.test"), &[USER], 3_000),
        mail(&a, "m4", "Sent", ("Owner", USER), &["Jean Dupont <jean.dupont@client.test>"], 4_000),
    ]).unwrap();
    // the list is built beside the open: these tests read the whole list, so they wait for `people/ready`
    let t0 = std::time::Instant::now();
    while core.state("people/ready".into()).as_deref() != Some("true") && t0.elapsed() < Duration::from_secs(60) { std::thread::sleep(Duration::from_millis(20)); }
    assert_eq!(core.state("people/ready".into()).as_deref(), Some("true"), "people/ready is served and the first build ends");
    (core, a)
}

fn person(core: &RustCore, address: &str) -> serde_json::Value {
    let raw = core.state(format!("people/address/{address}"));
    assert!(raw.is_some(), "people/address/{address} is served");
    serde_json::from_str(&raw.unwrap()).unwrap()
}

fn prefix(core: &RustCore, letters: &str) -> Vec<String> {
    let raw = core.state(format!("people/prefix/{letters}"));
    assert!(raw.is_some(), "people/prefix/{letters} is served");
    let v: Vec<serde_json::Value> = serde_json::from_str(&raw.unwrap()).unwrap();
    v.iter().map(|p| p["address"].as_str().unwrap().to_string()).collect()
}

#[test]
fn a_contacts_is_not_empty_once_mail_is_stored() {
    let dir = tempfile::tempdir().unwrap();
    let (core, account) = core_with_mail(dir.path());
    let all = core.contacts();
    assert!(!all.is_empty(), "contacts() lists the people of the stored mail");
    let zoe = all.iter().find(|c| c.addresses == vec!["zoe@acme.test".to_string()]).expect("Zoë, once, her address in lower case");
    assert_eq!(zoe.name, "Zoë Martin");
    assert_eq!(zoe.mail_count_per_account.get(&account), Some(&2));
    assert_eq!(all.first().map(|c| c.addresses.clone()), Some(vec!["zoe@acme.test".to_string()]), "the most mail first");
    assert!(all.iter().all(|c| c.addresses != vec![USER.to_string()]), "the owner's own address is not a person");
}

#[test]
fn b_a_person_only_seen_as_a_recipient_of_sent_mail_is_listed_with_to_count() {
    let dir = tempfile::tempdir().unwrap();
    let (core, _) = core_with_mail(dir.path());
    let jean = person(&core, "jean.dupont@client.test");
    assert_eq!(jean["name"], "Jean Dupont");
    assert_eq!((jean["from_count"].as_u64(), jean["to_count"].as_u64()), (Some(0), Some(1)));
    assert!(core.contact("jean.dupont@client.test".into()).is_some(), "contact(address) finds them too");
}

/// (b2) A mail in the Sent folder counts its recipients even when it names an address that is not the account's own (an alias):
/// the folder, not the From address alone, says the owner sent it. Mutant: only a From equal to an account's address counts.
#[test]
fn b2_a_mail_in_the_sent_folder_from_an_alias_counts_its_recipients() {
    let dir = tempfile::tempdir().unwrap();
    let (core, account) = core_with_mail(dir.path());
    core.store().insert_messages(vec![mail(&account, "m7", "Sent", ("Owner", "alias@elsewhere.test"), &["Lea Roy <lea.roy@client.test>"], 7_000)]).unwrap();
    let t0 = std::time::Instant::now();
    let to_count = || core.state("people/address/lea.roy@client.test".into()).and_then(|raw| serde_json::from_str::<serde_json::Value>(&raw).ok()).and_then(|p| p["to_count"].as_u64());
    while to_count() != Some(1) && t0.elapsed() < Duration::from_secs(10) { std::thread::sleep(Duration::from_millis(20)); }
    assert_eq!(to_count(), Some(1), "a mail in Sent from an alias counts its recipient");
}

#[test]
fn c_the_prefix_read_matches_a_first_name_a_last_name_and_an_address_accents_folded() {
    let dir = tempfile::tempdir().unwrap();
    let (core, _) = core_with_mail(dir.path());
    assert_eq!(prefix(&core, "jea"), vec!["jean.dupont@client.test"], "a first name");
    assert_eq!(prefix(&core, "dup"), vec!["jean.dupont@client.test"], "a last name");
    assert_eq!(prefix(&core, "jean.d"), vec!["jean.dupont@client.test"], "an address");
    assert_eq!(prefix(&core, "bob@"), vec!["bob@other.test"], "an address with its @");
    assert_eq!(prefix(&core, "zoe"), vec!["zoe@acme.test"], "Zoë typed without the accent");
    assert_eq!(prefix(&core, "ZOË"), vec!["zoe@acme.test"], "and with it, in capitals");
    assert!(prefix(&core, "mart").contains(&"zoe@acme.test".to_string()));
    assert!(prefix(&core, "xyz").is_empty());
}

#[test]
fn d_the_counts_move_when_a_mail_arrives() {
    let dir = tempfile::tempdir().unwrap();
    let (core, account) = core_with_mail(dir.path());
    assert_eq!(person(&core, "zoe@acme.test")["from_count"].as_u64(), Some(2));
    core.store().insert_messages(vec![mail(&account, "m5", "INBOX", ("Zoë Martin", "zoe@acme.test"), &[USER], 5_000)]).unwrap();
    std::thread::sleep(Duration::from_millis(50));
    assert_eq!(person(&core, "zoe@acme.test")["from_count"].as_u64(), Some(3), "the arrival is counted");
    core.store().insert_messages(vec![mail(&account, "m6", "Sent", ("Owner", USER), &["zoe@acme.test"], 6_000)]).unwrap();
    assert_eq!(person(&core, "zoe@acme.test")["to_count"].as_u64(), Some(1), "a mail the owner sent her is counted");
}
