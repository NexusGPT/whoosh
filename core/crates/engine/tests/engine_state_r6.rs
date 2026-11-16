//! engine-act-r6 round 6: the state paths the screens read that only the fixture core served (docs/STATE-PATHS.json `gap` rows), now served by
//! the real core from a store the IMAP sync filled (sync-imap's fake server on 127.0.0.1): each test changes the fact through the core's own
//! commands and reads the path again. Nothing leaves this Mac. The single-edit mutant each test was seen red on is named under it.
#![allow(dead_code, unused_imports)]
#[path = "../../autoconfig/tests/support/mod.rs"]
mod detect_fakes;
#[allow(dead_code, unused_imports)]
#[path = "../../sync-imap/tests/support/mod.rs"]
mod imap_fakes;

use std::sync::Arc;
use std::time::{Duration, Instant};

use contracts::*;
use detect_fakes::{FakeDns, FakeHttp, FakeProbe};
use engine::{RustCore, SyncPhase, TestTransport};
use imap_fakes::FakeImap;
use secrets::{MemoryKeychain, Namespace};
use store::Store;

const USER: &str = "owner@example.test";
const WAIT: Duration = Duration::from_secs(25);

fn tab(t: &str) -> InboxQuery { InboxQuery { tab_id: t.into(), account_id: None, label_id: None, mailbox: None, account_ids: vec![], filters: vec![], smart_folder_id: None } }
fn wait_until(what: &str, mut ok: impl FnMut() -> bool) {
    let t0 = Instant::now();
    while t0.elapsed() < WAIT { if ok() { return; } std::thread::sleep(Duration::from_millis(40)); }
    panic!("never reached within {WAIT:?}: {what}");
}

/// The fake IMAP server (with `archived` messages already in its Archive folder) and the account added through the core; listening when it returns.
fn synced(inbox: u32, archived: u32) -> (RustCore, FakeImap, tokio::runtime::Runtime, tempfile::TempDir) { synced_flagged(inbox, archived, &[]) }
/// As `synced`, with the INBOX messages of `flagged` (UIDs) already starred ON THE SERVER (`\Flagged`) before the first sync.
fn synced_flagged(inbox: u32, archived: u32, flagged: &[u32]) -> (RustCore, FakeImap, tokio::runtime::Runtime, tempfile::TempDir) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let imap = rt.block_on(FakeImap::start());
    imap.seed("INBOX", inbox);
    imap.seed("Archive", archived);
    imap.with(|s| for m in s.boxes.iter_mut().find(|b| b.name == "INBOX").unwrap().msgs.iter_mut().filter(|m| flagged.contains(&m.uid)) { m.flags.insert("\\Flagged".into()); });
    let dir = tempfile::tempdir().unwrap();
    let core = RustCore::with_parts(Store::open(&dir.path().join("whoosh.sqlite")).unwrap(), Arc::new(MemoryKeychain::new(Namespace::rig())),
        Box::new(autoconfig::Detector::new(FakeHttp::default(), FakeDns::default(), FakeProbe::default())), TestTransport::PlainLoopback);
    let config = AccountConfig {
        imap_host: "127.0.0.1".into(), imap_port: imap.port, imap_security: ConnectionSecurity::Ssl, smtp_host: String::new(), smtp_port: 25,
        smtp_security: ConnectionSecurity::Starttls, username: USER.into(), smtp_username: USER.into(), auth_method: AuthMethod::Password, source: DetectionSource::Manual,
    };
    let id = core.run(Command::AddAccountConfigured { address: USER.into(), display_name: Some("Owner".into()), config, password: SecretText { value: "app-password".into() } })
        .expect("added").created_id.unwrap();
    wait_until("listening", || core.sync_phase(&id) == Some(SyncPhase::Listening));
    (core, imap, rt, dir)
}
fn by_subject(core: &RustCore, s: &str) -> InboxRow { core.inbox(tab("primary")).into_iter().find(|r| r.subject == s).unwrap_or_else(|| panic!("no row {s}")) }
fn state(core: &RustCore, path: String) -> Option<String> { core.state(path) }

/// The ids the ⇧S filter lists (the inbox with the Starred filter) and the Starred view lists.
fn starred_lists(core: &RustCore) -> (Vec<String>, Vec<String>) {
    let ids = |q: InboxQuery| core.inbox(q).into_iter().map(|r| r.message_id).collect::<Vec<_>>();
    (ids(InboxQuery { filters: vec![InboxFilter::Starred], ..tab("primary") }), ids(InboxQuery { mailbox: Some(Mailbox::Starred), ..tab("primary") }))
}

#[test]
fn starred_follows_star_and_unstar() {
    let (core, _imap, _rt, _d) = synced(2, 0);
    let m = by_subject(&core, "Message 1").message_id;
    assert_eq!(state(&core, format!("message/{m}/starred")).as_deref(), Some("false"));
    core.run(Command::Star { message_ids: vec![m.clone()], starred: true }).expect("star");
    // at once, and the same truth in all three: the mark, the ⇧S filter, the Starred view
    assert_eq!(state(&core, format!("message/{m}/starred")).as_deref(), Some("true"));
    assert_eq!(starred_lists(&core), (vec![m.clone()], vec![m.clone()]), "S shows in the ⇧S filter and the Starred view at once");
    core.run(Command::Star { message_ids: vec![m.clone()], starred: false }).expect("unstar");
    assert_eq!(state(&core, format!("message/{m}/starred")).as_deref(), Some("false"));
    assert_eq!(starred_lists(&core), (vec![], vec![]), "and leaves both at once");
    assert_eq!(state(&core, "message/no-such-message/starred".into()), None, "an id the store does not hold answers nothing (as message/<id>/muted)");
}

fn settled(core: &RustCore) -> bool { core.store().pending_actions().unwrap().is_empty() }
/// The server's own flags of an INBOX message, and how many STOREs that set (+) or cleared (-) `\Flagged` it received.
fn server_flagged(imap: &FakeImap, uid: u32) -> bool { imap.with(|s| s.boxes.iter().find(|b| b.name == "INBOX").unwrap().msgs.iter().any(|m| m.uid == uid && m.flags.contains("\\Flagged"))) }
fn flag_stores(imap: &FakeImap, sign: &str) -> usize { imap.log().iter().filter(|l| l.contains("STORE") && l.contains("\\Flagged") && l.contains(&format!("{sign}FLAGS"))).count() }

#[test]
fn star_tells_the_server_once_and_a_server_starred_message_is_kept_then_unstarred_on_the_server() {
    // Message 3 is starred on the server before the first sync (its \Flagged): the sync stores it starred
    let (core, imap, _rt, _d) = synced_flagged(3, 0, &[3]);
    let (m1, m3) = (by_subject(&core, "Message 1").message_id, by_subject(&core, "Message 3").message_id);
    assert_eq!(starred_lists(&core), (vec![m3.clone()], vec![m3.clone()]), "the server's star is in the ⇧S filter and the Starred view");
    // S on Message 1: one \Flagged STORE, and S again on it sends nothing more
    core.run(Command::Star { message_ids: vec![m1.clone()], starred: true }).expect("star");
    wait_until("the star reached the server", || settled(&core) && server_flagged(&imap, 1));
    assert_eq!(flag_stores(&imap, "+"), 1, "\\Flagged was sent once: {:?}", imap.log().iter().filter(|l| l.contains("STORE")).collect::<Vec<_>>());
    core.run(Command::Star { message_ids: vec![m1.clone()], starred: true }).expect("star again");
    wait_until("settled", || settled(&core));
    assert_eq!(flag_stores(&imap, "+"), 1, "a message already starred sends nothing again");
    // unstarring a DIFFERENT message keeps the server's star on Message 3
    core.run(Command::Star { message_ids: vec![m1.clone()], starred: false }).expect("unstar m1");
    assert_eq!(starred_lists(&core), (vec![m3.clone()], vec![m3.clone()]), "Message 3 stays starred");
    assert_eq!(state(&core, format!("message/{m3}/starred")).as_deref(), Some("true"));
    // unstarring Message 3 itself (starred by the server, never by S, so it carries no label): gone here at once, and gone on the server
    core.run(Command::Star { message_ids: vec![m3.clone()], starred: false }).expect("unstar m3");
    assert_eq!(starred_lists(&core), (vec![], vec![]));
    wait_until("the server's \\Flagged on Message 3 is cleared", || settled(&core) && !server_flagged(&imap, 3));
    assert!(!server_flagged(&imap, 1), "Message 1 is unstarred on the server too");
}
// mutant "star-group-from-the-label": star.rs, the `applied.groups = groups_by_account(&changed, ..)` line deleted (red: the server's star on Message 3 is never cleared)
// mutant "starred-inverted": message_state.rs, `m.starred.to_string()` -> `(!m.starred).to_string()` (red: "true" before any star)
// mutant "star-column-not-set": actions star.rs, the `set_flag(ctx, "starred", &changed_ids, starred)?;` line deleted (red: S is missing from ⇧S, the Starred view and the mark)

#[test]
fn archived_is_true_for_e_and_for_a_message_the_server_holds_in_archive() {
    let (core, _imap, _rt, _d) = synced(2, 1);
    let m = by_subject(&core, "Message 1").message_id;
    assert_eq!(state(&core, format!("message/{m}/archived")).as_deref(), Some("false"));
    core.run(Command::Archive { message_ids: vec![m.clone()] }).expect("archive");
    assert_eq!(state(&core, format!("message/{m}/archived")).as_deref(), Some("true"), "E: the row keeps INBOX and the archived mark");
    core.run(Command::Unarchive { message_ids: vec![m.clone()] }).expect("unarchive");
    assert_eq!(state(&core, format!("message/{m}/archived")).as_deref(), Some("false"));
    // the message the sync stored from the server's Archive folder: ⇧E outside the Archive view must find it archived too
    let in_archive = core.inbox(InboxQuery { mailbox: Some(Mailbox::Archive), ..tab("primary") });
    assert_eq!(in_archive.len(), 1, "the synced Archive message is listed");
    assert_eq!(state(&core, format!("message/{}/archived", in_archive[0].message_id)).as_deref(), Some("true"));
}
// mutant "archived-inverted": message_state.rs `archived` arm, `m.archived.to_string()` -> `(!m.archived).to_string()` (red)
// (round 6's first arm also looked for an Archive-role folder; its mutant survived because the sync stores those messages with the mark, so that part was redundant and went)

#[test]
fn snoozed_and_its_return_follow_a_snooze() {
    let (core, _imap, _rt, _d) = synced(2, 0);
    let (a, b) = (by_subject(&core, "Message 1").message_id, by_subject(&core, "Message 2").message_id);
    let at = engine::SystemClock.now_ms() + 3 * 3_600_000;
    core.run(Command::Snooze { message_ids: vec![a.clone()], when: SnoozeWhen::At { at_ms: at } }).expect("snooze");
    assert_eq!(state(&core, format!("message/{a}/snoozed")).as_deref(), Some("true"));
    assert_eq!(state(&core, format!("snooze/{a}/return_ms")), Some(at.to_string()));
    let local = engine::message_state_local_time(at, engine::SystemClock.utc_offset_s());
    assert_eq!(state(&core, format!("snooze/{a}/return_local_time")), Some(format!("\"{local}\"")), "\"HH:mm\" in this Mac's time");
    // the same fact as the core's own snooze read
    assert!(core.snoozed_messages().iter().any(|s| s.message_id == a && s.return_at_ms == at));
    // a message that is not snoozed
    assert_eq!(state(&core, format!("message/{b}/snoozed")).as_deref(), Some("false"));
    assert_eq!(state(&core, format!("snooze/{b}/return_ms")), None, "nothing while not snoozed (the fixture's rule)");
    assert_eq!(state(&core, format!("snooze/{b}/return_local_time")), None);
}
// mutant "snoozed-inverted": message_state.rs, `m.snoozed_until_ms.is_some()` -> `m.snoozed_until_ms.is_none()` (red)
// mutant "return-in-seconds": message_state.rs `return_ms` arm, `ms.to_string()` -> `(ms / 1000).to_string()` (red)
// mutant "local-time-shifted": message_state.rs `return_local_time` arm, `local_time(ms, offset_s)` -> `local_time(ms + 3_600_000, offset_s)` (red)

#[test]
fn recent_searches_are_newest_first_one_per_words_at_most_the_cap() {
    let (core, _imap, _rt, _d) = synced(1, 0);
    let recent = |core: &RustCore| -> Vec<String> { serde_json::from_str(&state(core, "search/recent".into()).expect("served")).expect("a JSON array") };
    assert!(recent(&core).is_empty(), "a new installation has none");
    core.run(Command::RecordSearch { query: "Linea quote".into() }).expect("recorded");
    core.run(Command::RecordSearch { query: "  invoice  ".into() }).expect("recorded");
    assert_eq!(recent(&core), vec!["invoice".to_string(), "Linea quote".into()], "newest first, trimmed");
    core.run(Command::RecordSearch { query: "LINEA QUOTE".into() }).expect("the same words again");
    assert_eq!(recent(&core), vec!["LINEA QUOTE".to_string(), "invoice".into()], "one row per distinct words, in any case");
    for i in 0..10 { core.run(Command::RecordSearch { query: format!("words {i}") }).expect("recorded"); }
    let r = recent(&core);
    assert_eq!(r.len(), contracts::constants::SEARCH_RECENT_MAX as usize, "at most SEARCH_RECENT_MAX");
    assert_eq!(r[0], "words 9");
    assert!(matches!(core.run(Command::RecordSearch { query: "   ".into() }), Err(CoreError::Invalid { .. })), "empty words are refused");
}
// mutant "same-words-twice": realcore.rs `record_search`, the `DELETE FROM search_history WHERE lower(query) = lower(?1)` line deleted (red: the second "Linea quote" fails the primary key, or stays twice)
// mutant "record-search-not-built": rust_core.rs, the `Command::RecordSearch` arm deleted (red: not built yet)

#[test]
fn a_saved_drafts_whole_text_follows_save_and_update() {
    let (core, _imap, _rt, _d) = synced(1, 0);
    let body = "Hello Karen,\n\nthe plan holds.\n\n-- \nOwner";
    let id = core.run(Command::SaveDraft { from_account_id: None, to: vec!["karen@example.test".into()], cc: vec![], subject: "Plan".into(), body: body.into(), reply_to_thread_id: None })
        .expect("saved").created_id.unwrap();
    let read = |core: &RustCore| -> Option<String> { state(core, format!("draft/{id}/body")).map(|j| serde_json::from_str::<String>(&j).expect("a JSON string")) };
    assert_eq!(read(&core).as_deref(), Some(body), "the whole text, line breaks kept");
    core.run(Command::UpdateDraft { draft_id: id.clone(), from_account_id: None, to: vec!["karen@example.test".into()], cc: vec![], bcc: vec![], subject: "Plan".into(), body: "Edited.".into() }).expect("updated");
    assert_eq!(read(&core).as_deref(), Some("Edited."));
    assert_eq!(state(&core, "draft/no-such-draft/body".into()), None);
}
// mutant "draft-body-not-served": draft_files.rs, the `["draft", id, "body"]` arm deleted (red: no answer)

#[test]
fn the_real_apps_mcp_server_reads_off() {
    let (core, _imap, _rt, _d) = synced(1, 0);
    assert_eq!(state(&core, "mcp_server/on".into()).as_deref(), Some("false"));
}
// mutant "mcp-server-not-served": rust_core.rs, the `mcp_server/on` link deleted (red: no answer)
