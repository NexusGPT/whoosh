//! indexer-race: a core reopened on the same data folder, many times in a row, with work queued each time, always settles its
//! indexer, and its word index holds every mail. Each reopening stores a batch of new mails, opens the word index with nothing
//! queued (as a search does on a launch with nothing new: the reopened index starts its exact comparison with the table at once,
//! WAKE_REPAIR), then queues some of the batch while that comparison runs (as the crawl and the reader do).
//! On 61f5631d the comparison's `take_now` swallowed the WAKE_IDS of a `reindex` that landed during it: the ids stayed in the word
//! index's `pending` with no flag left, `pending()` never reached 0, and `settle_index` waited out its whole ceiling (the hang of
//! engine_search_forms `e_a_restart_gives_the_same_list`, line 125).
//! Red-first broken copy (one line): in core/crates/search-text/src/lib.rs `step`, delete
//! `if more & WAKE_IDS != 0 { reindex_pending(engine, shared)?; }`; this test then fails at "the indexer settled" on its first
//! reopening whose `reindex` lands during the comparison.
//! No mail text is printed: the test prints counts and times only. The rig core's account names 127.0.0.1 and has no password in
//! the rig keyring, so no sync starts and no socket opens.
use std::time::{Duration, Instant};

use contracts::*;
use engine::RustCore;
use store::NewMessage;

const USER: &str = "owner@example.test";
/// Reopenings of the same folder after the first open.
const REOPENINGS: usize = 30;
/// New mails stored at each opening (the comparison of a reopened index has them to catch up, so it lasts while work is queued).
const NEW_PER_OPENING: usize = 400;
/// Of those, the ones queued to the indexer while the comparison runs.
const QUEUED_PER_OPENING: usize = 16;
/// A ceiling on the indexer's settling, never a budget (the indexer runs at the OS background priority).
const SETTLE: Duration = Duration::from_secs(30);
/// A ceiling on opening the word index (the previous core's writer can outlive its core for a moment: its last owner may be a
/// thread the core's drop does not join, see docs/lanes/indexer-race.md).
const OPEN: Duration = Duration::from_secs(10);
const OPEN_RETRY: Duration = Duration::from_millis(10);

fn config() -> AccountConfig {
    AccountConfig {
        imap_host: "127.0.0.1".into(), imap_port: 1, imap_security: ConnectionSecurity::Ssl, smtp_host: "127.0.0.1".into(), smtp_port: 1,
        smtp_security: ConnectionSecurity::Starttls, username: USER.into(), smtp_username: USER.into(), auth_method: AuthMethod::Password, source: DetectionSource::Manual,
    }
}

/// The word only the mails of opening `n` hold ("tidemarkb" for 1, "tidemarkba" for 10): letters only, so the word index keeps it whole.
fn opening_word(n: usize) -> String {
    let letters: String = n.to_string().chars().map(|c| (b'a' + c.to_digit(10).unwrap_or(0) as u8) as char).collect();
    format!("tidemark{letters}")
}

fn mail(account: &str, id: &str, word: &str, at: i64) -> NewMessage {
    NewMessage {
        message: Message {
            id: id.into(), thread_id: format!("{id}:thread"), account_id: account.into(), sender_name: "Sender".into(), from_address: "sender@example.test".into(),
            to: vec![USER.into()], subject: "Weekly notes".into(), preview: String::new(), received_at_ms: at, read: false, archived: false,
            folder: "INBOX".into(), attachment_names: vec![], login_code: None, trackers_blocked: 0, list_unsubscribe: vec![], list_unsubscribe_one_click: false,
            reply_to_name: None, cc: vec![], bcc: vec![], is_outgoing: false,
        },
        remote_id: None, body_text: Some(format!("The {word} figures for this week are attached.")), body_html: None, has_list_header: false, size_bytes: None, tab_id: None,
    }
}

#[test]
fn a_core_reopened_on_one_folder_settles_its_indexer_every_time() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().to_str().unwrap().to_string();
    let t0 = Instant::now();
    let mut waited_for_writer = 0usize;
    for opening in 0..=REOPENINGS {
        let core = RustCore::open_production(&path, true).expect("the rig core opens");
        let store = core.store();
        let account = match store.account_summaries().unwrap().first() {
            Some(a) => a.id.clone(),
            None => store.add_account(USER, "Owner", MailProvider::Imap, "svc\u{1f}acct", false, config(), 1).unwrap(),
        };
        let word = opening_word(opening);
        let ids: Vec<String> = (0..NEW_PER_OPENING).map(|i| format!("o{opening}-m{i}")).collect();
        let base = 1_700_000_000_000 + (opening * NEW_PER_OPENING) as i64 * 1_000;
        store.insert_messages(ids.iter().enumerate().map(|(i, id)| mail(&account, id, &word, base + i as i64 * 1_000)).collect()).unwrap();
        let bg = core.background();
        // the word index opens with nothing queued; on a reopening it starts its exact comparison at once
        let asked = Instant::now();
        while !bg.open_word_index() {
            waited_for_writer += 1;
            assert!(asked.elapsed() < OPEN, "opening {opening}: the word index did not open within {OPEN:?}");
            std::thread::sleep(OPEN_RETRY);
        }
        // work queued while the comparison runs
        bg.reindex(&ids[..QUEUED_PER_OPENING]);
        let settled_at = Instant::now();
        assert!(bg.settle_index(SETTLE), "opening {opening}: the indexer settled (waited {:?})", settled_at.elapsed());
        let found = bg.search_words(&word, NEW_PER_OPENING * 2).len();
        assert_eq!(found, NEW_PER_OPENING, "opening {opening}: the word index holds every mail of this opening");
        drop(core);
    }
    eprintln!("indexer_reopen: {} openings settled in {:?}; the word index waited for the previous writer {waited_for_writer} time(s)", REOPENINGS + 1, t0.elapsed());
}
