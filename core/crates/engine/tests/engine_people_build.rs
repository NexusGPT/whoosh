//! search-forms part 3: the people list is built beside the open, never on its path. A read while the first build is still running
//! answers at once from what is built so far (`people/ready` false, no wait, no panic), and the list is whole once it says true.
//! Its own test binary: the build pause it sets is process-wide. On c83fce8c it fails: no `people/` path was served.
use std::time::{Duration, Instant};

use contracts::*;
use engine::RustCore;
use store::NewMessage;

const USER: &str = "owner@example.test";
/// The test's pause before each build step: long enough that the reads below come before the first step.
const PAUSE_MS: u64 = 3_000;
/// A read must not wait for the build: far under the pause.
const AT_ONCE: Duration = Duration::from_millis(1_000);

fn mail(account: &str, id: &str, sender: (&str, &str), at: i64) -> NewMessage {
    NewMessage {
        message: Message {
            id: id.into(), thread_id: format!("{id}:thread"), account_id: account.into(), sender_name: sender.0.into(), from_address: sender.1.into(),
            to: vec![USER.into()], subject: format!("Subject {id}"), preview: String::new(), received_at_ms: at, read: false, archived: false,
            folder: "INBOX".into(), attachment_names: vec![], login_code: None, trackers_blocked: 0, list_unsubscribe: vec![], list_unsubscribe_one_click: false,
            reply_to_name: None, cc: vec![], bcc: vec![], is_outgoing: false,
        },
        remote_id: None, body_text: None, body_html: None, has_list_header: false, size_bytes: None, tab_id: None,
    }
}

#[test]
fn a_read_while_the_list_is_being_built_answers_at_once() {
    let dir = tempfile::tempdir().unwrap();
    {
        let core = RustCore::open_production(dir.path().to_str().unwrap(), true).expect("the rig core opens");
        let store = core.store();
        let config = AccountConfig {
            imap_host: "127.0.0.1".into(), imap_port: 1, imap_security: ConnectionSecurity::Ssl, smtp_host: "127.0.0.1".into(), smtp_port: 1,
            smtp_security: ConnectionSecurity::Starttls, username: USER.into(), smtp_username: USER.into(), auth_method: AuthMethod::Password, source: DetectionSource::Manual,
        };
        let a = store.add_account(USER, "Owner", MailProvider::Imap, "svc\u{1f}acct", false, config, 1).unwrap();
        store.insert_messages((0..50).map(|i| mail(&a, &format!("m{i}"), ("Zoë Martin", "zoe@acme.test"), 1_000 + i)).collect()).unwrap();
    }
    std::thread::sleep(Duration::from_millis(200));
    RustCore::set_people_build_pause(PAUSE_MS);
    let core = RustCore::open_production(dir.path().to_str().unwrap(), true).expect("the rig core reopens");
    let t0 = Instant::now();
    let ready = core.state("people/ready".into());
    let zoe = core.state("people/prefix/zoe".into());
    let count = core.state("people/count".into());
    let contacts = core.contacts();
    let took = t0.elapsed();
    RustCore::set_people_build_pause(0);
    eprintln!("four reads during the build took {took:?}");
    assert_eq!(ready.as_deref(), Some("false"), "the build is still running");
    assert_eq!(zoe.as_deref(), Some("[]"), "nothing built yet: an empty answer, not a wait");
    assert_eq!(count.as_deref(), Some("0"), "the count is the people built so far");
    assert!(contacts.is_empty());
    assert!(took < AT_ONCE, "the reads did not wait for the build: {took:?}");
    let t1 = Instant::now();
    while core.state("people/ready".into()).as_deref() != Some("true") && t1.elapsed() < Duration::from_secs(60) { std::thread::sleep(Duration::from_millis(50)); }
    assert_eq!(core.state("people/ready".into()).as_deref(), Some("true"), "the build ends");
    let v: Vec<serde_json::Value> = serde_json::from_str(&core.state("people/prefix/zoe".into()).unwrap()).unwrap();
    assert_eq!(v.first().and_then(|p| p["from_count"].as_u64()), Some(50), "the whole list once built: {v:?}");
}
