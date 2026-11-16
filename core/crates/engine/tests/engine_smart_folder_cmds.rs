//! real-folders (SPEC A143, A153): SaveSmartFolder, RenameSmartFolder, UpdateSmartFolder and DeleteSmartFolder on the REAL core
//! (`smart_folder_cmds.rs`), with the reads the app uses for them: `smart_folders()`, the list of a folder's mails (`inbox` with
//! `smart_folder_id`), and the state paths `smart_folder/count`, `smart_folder/<id>/{count,unread,threads,rule,parent}`,
//! `smart_folder/<id or name>/{rule,exists}`. They answered "The local engine does not do this yet." before.
//! Each test runs over the rig's production core with no model. Every test names its one-line broken copy (red first).
//! The speed test PRINTS one line per operation (`smart folders, <what>: N ms on 2000 mails`) and fails over 50 ms; run it with
//! `-- --nocapture` and in the release profile. Only synthetic addresses (`.example`, `.test`) appear.
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

fn mail(account: &str, id: &str, folder: &str, from: &str, subject: &str, body: &str, at: i64) -> NewMessage {
    NewMessage {
        message: Message {
            id: id.into(), thread_id: format!("{id}:thread"), account_id: account.into(), sender_name: "Sender".into(), from_address: from.into(),
            to: vec![USER.into()], subject: subject.into(), preview: body.into(), received_at_ms: at, read: false, archived: false,
            folder: folder.into(), attachment_names: vec![], login_code: None, trackers_blocked: 0, list_unsubscribe: vec![], list_unsubscribe_one_click: false,
            reply_to_name: None, cc: vec![], bcc: vec![], is_outgoing: false,
        },
        remote_id: None, body_text: Some(body.into()), body_html: None, has_list_header: false, size_bytes: None, tab_id: None,
    }
}

fn open(dir: &std::path::Path) -> (RustCore, String) {
    let core = RustCore::open_production(dir.to_str().unwrap(), true).expect("the rig core opens");
    let a = core.store().add_account(USER, "Owner", MailProvider::Imap, "svc\u{1f}acct", false, config(), 1).unwrap();
    (core, a)
}

fn index(core: &RustCore, ids: &[String]) {
    let bg = core.background();
    bg.reindex(ids);
    assert!(bg.settle_index(WAIT), "the indexer finished");
}

// The sender-domain fixture of engine_sender_domain.rs: Lorvan A (Jan), B (Feb, upper-case address), S (Mar, a sub-domain), the look-alike L (Apr),
// Z (May) at acme.test, and T, the owner's own mail in Sent to a Lorvan address (Jun).
const A: &str = "a-lorvan-invoice";
const B: &str = "b-lorvan-stay";
const S: &str = "s-lorvan-sub";
const L: &str = "l-lookalike";
const Z: &str = "z-acme";
const T: &str = "t-sent-to-lorvan";
const DAY: i64 = 86_400_000;
const JAN: i64 = 1_767_225_600_000;

fn lorvan_core(dir: &std::path::Path) -> (RustCore, String) {
    let (core, a) = open(dir);
    core.store().insert_messages(vec![
        mail(&a, A, "INBOX", "reservations@lorvan-hotels.example", "Booking invoice", "Your invoice.", JAN + 14 * DAY),
        mail(&a, B, "INBOX", "RESERVATIONS@LORVAN-HOTELS.EXAMPLE", "Your stay", "The booking is confirmed.", JAN + 45 * DAY),
        mail(&a, S, "INBOX", "news@mail.lorvan-hotels.example", "Booking offer", "An offer.", JAN + 73 * DAY),
        mail(&a, L, "INBOX", "x@notlorvan-hotels.example", "Booking invoice", "Not the hotel.", JAN + 104 * DAY),
        mail(&a, Z, "INBOX", "zoe@acme.test", "Booking invoice", "The booking numbers.", JAN + 134 * DAY),
        mail(&a, T, "Sent", USER, "Re: booking", "Thanks.", JAN + 165 * DAY),
    ]).unwrap();
    index(&core, &[A, B, S, L, Z, T].map(String::from));
    (core, a)
}

fn save(core: &RustCore, name: &str, rule: &str) -> String {
    core.run(Command::SaveSmartFolder { name: name.into(), rule_words: rule.into() }).expect("the folder is saved").created_id.expect("a created id")
}

fn refusal(r: Result<CommandOutcome, CoreError>) -> CoreError { r.expect_err("the command is refused") }

fn query(folder: &str) -> InboxQuery {
    InboxQuery { tab_id: "primary".into(), account_id: None, label_id: None, mailbox: None, account_ids: vec![], filters: vec![], smart_folder_id: Some(folder.into()) }
}

fn ids_of(rows: &[InboxRow]) -> Vec<String> { rows.iter().map(|r| r.message_id.clone()).collect() }

fn state(core: &RustCore, path: &str) -> Option<String> { core.state(path.into()) }

/// Broken copy: the dispatch line `c if smart_folder_cmds::is_command(&c) => ...` in rust_core.rs `run` is deleted (the answer is "does not do this yet", code
/// engine.not_built_yet), or `smart_folders()` returns `vec![]` again.
#[test]
fn the_four_commands_are_served_and_the_folders_are_read_back_in_order() {
    let dir = tempfile::tempdir().unwrap();
    let (core, _) = lorvan_core(dir.path());
    assert!(core.smart_folders().is_empty(), "a new store holds no folder");
    assert_eq!(state(&core, "smart_folder/count").as_deref(), Some("0"));
    let hotels = core.run(Command::SaveSmartFolder { name: "Hotels".into(), rule_words: "from:@lorvan-hotels.example".into() }).unwrap();
    assert_eq!(hotels.undo_entry_id, None, "the test engine records no undo entry for a smart folder");
    let hotels = hotels.created_id.expect("the id of the new folder");
    let taxes = save(&core, "Taxes", "tax");
    let invoices = save(&core, "Invoices", "invoice");
    let listed = core.smart_folders();
    assert_eq!(listed.iter().map(|f| f.name.as_str()).collect::<Vec<_>>(), ["Hotels", "Taxes", "Invoices"]);
    assert_eq!(listed[0], SmartFolder { id: hotels.clone(), name: "Hotels".into(), rule_words: "from:@lorvan-hotels.example".into(), parent_id: None });
    // update: new rule and a parent
    let upd = core.run(Command::UpdateSmartFolder { folder_id: invoices.clone(), rule_words: "invoice from:@lorvan-hotels.example".into(), parent_id: Some(hotels.clone()) }).unwrap();
    assert_eq!(upd, CommandOutcome { undo_entry_id: None, created_id: None });
    assert_eq!(core.smart_folders().iter().map(|f| f.name.as_str()).collect::<Vec<_>>(), ["Hotels", "Invoices", "Taxes"], "parents before children");
    // rename keeps the rule
    core.run(Command::RenameSmartFolder { folder_id: taxes.clone(), name: "Tax papers".into() }).unwrap();
    let renamed = core.smart_folders().into_iter().find(|f| f.id == taxes).unwrap();
    assert_eq!((renamed.name.as_str(), renamed.rule_words.as_str()), ("Tax papers", "tax"));
    // the state paths the test engine serves
    assert_eq!(state(&core, "smart_folder/count").as_deref(), Some("3"));
    assert_eq!(state(&core, &format!("smart_folder/{hotels}/rule")).as_deref(), Some("\"from:@lorvan-hotels.example\""));
    assert_eq!(state(&core, "smart_folder/hotels/rule").as_deref(), Some("\"from:@lorvan-hotels.example\""), "by name, any case");
    assert_eq!(state(&core, "smart_folder/Tax papers/rule").as_deref(), Some("\"tax\""), "a name with a space is read whole");
    assert_eq!(state(&core, &format!("smart_folder/{invoices}/parent")).as_deref(), Some(format!("\"{hotels}\"").as_str()));
    assert_eq!(state(&core, &format!("smart_folder/{hotels}/parent")), None, "a top-level folder has no parent");
    assert_eq!(state(&core, &format!("smart_folder/{taxes}/exists")).as_deref(), Some("true"));
    assert_eq!(state(&core, "smart_folder/Tax papers/exists").as_deref(), Some("true"));
    assert_eq!(state(&core, "smart_folder/Tax/exists").as_deref(), Some("false"));
    // delete: the folder inside it moves up
    core.run(Command::DeleteSmartFolder { folder_id: hotels.clone() }).unwrap();
    let after = core.smart_folders();
    assert_eq!(after.iter().map(|f| f.name.as_str()).collect::<Vec<_>>(), ["Tax papers", "Invoices"]);
    assert_eq!(after.iter().find(|f| f.id == invoices).unwrap().parent_id, None, "the child went up to the top level");
    assert_eq!(state(&core, "smart_folder/count").as_deref(), Some("2"));
    assert_eq!(state(&core, &format!("smart_folder/{hotels}/exists")).as_deref(), Some("false"));
}

/// Broken copy: delete the `NameTaken` line of `smart_folder_save` in store delta12.rs (the duplicate is saved and the first assertion fails), or make
/// `run` answer `Ok` for an unknown id (the NotFound assertions fail).
#[test]
fn every_refusal_is_a_sentence_and_none_is_the_not_built_yet_answer() {
    let dir = tempfile::tempdir().unwrap();
    let (core, _) = lorvan_core(dir.path());
    let a = save(&core, "Hotels", "booking");
    let b = save(&core, "Taxes", "tax");
    let run = |c: Command| refusal(core.run(c));
    let invalid = |field: &str, words: &str| CoreError::Invalid { field: field.into(), reason_words: words.into() };
    let missing = |id: &str| CoreError::NotFound { kind: "smart folder".into(), id: id.into() };
    assert_eq!(run(Command::SaveSmartFolder { name: "hotels".into(), rule_words: "x".into() }), invalid("name", "the name exists already"));
    assert_eq!(run(Command::SaveSmartFolder { name: "  ".into(), rule_words: "x".into() }), invalid("name", "a smart folder needs a name"));
    assert_eq!(run(Command::SaveSmartFolder { name: "New".into(), rule_words: "  ".into() }), invalid("rule", "a smart folder needs a rule"));
    assert_eq!(run(Command::RenameSmartFolder { folder_id: "sf-none".into(), name: "X".into() }), missing("sf-none"));
    assert_eq!(run(Command::RenameSmartFolder { folder_id: a.clone(), name: " ".into() }), invalid("name", "a smart folder needs a name"));
    assert_eq!(run(Command::RenameSmartFolder { folder_id: a.clone(), name: "TAXES".into() }), invalid("name", "there is already a smart folder named \"TAXES\""));
    assert_eq!(run(Command::UpdateSmartFolder { folder_id: "sf-none".into(), rule_words: "x".into(), parent_id: None }), missing("sf-none"));
    assert_eq!(run(Command::UpdateSmartFolder { folder_id: a.clone(), rule_words: " ".into(), parent_id: None }), invalid("rule", "a smart folder needs a rule"));
    assert_eq!(run(Command::UpdateSmartFolder { folder_id: a.clone(), rule_words: "x".into(), parent_id: Some(a.clone()) }), invalid("parent", "a folder cannot sit inside itself"));
    assert_eq!(run(Command::UpdateSmartFolder { folder_id: a.clone(), rule_words: "x".into(), parent_id: Some("sf-none".into()) }), missing("sf-none"));
    core.run(Command::UpdateSmartFolder { folder_id: b.clone(), rule_words: "tax".into(), parent_id: Some(a.clone()) }).unwrap();
    assert_eq!(run(Command::UpdateSmartFolder { folder_id: a.clone(), rule_words: "x".into(), parent_id: Some(b.clone()) }), invalid("parent", "a folder cannot sit inside one of its own folders"));
    assert_eq!(run(Command::DeleteSmartFolder { folder_id: "sf-none".into() }), missing("sf-none"));
    // none of the refusals changed a folder
    let now = core.smart_folders();
    assert_eq!(now.iter().map(|f| (f.name.as_str(), f.rule_words.as_str())).collect::<Vec<_>>(), [("Hotels", "booking"), ("Taxes", "tax")]);
}

/// A saved folder keeps the sender-domain filter (SPEC A153): `from:@lorvan-hotels.example` lists A, B and the sub-domain S newest first, not the look-alike
/// L, not Z, not the owner's Sent copy T; its count, unread figure and `threads` are read from the same rows as the list.
/// Broken copy: `smart_folder_cmds::inbox` returns `Vec::new()` (the list is empty), or `members` reads `hits` for the count and the store for the list (the
/// owner's archived mail shows in one and not the other), or `Store::smart_folder_members` drops its `ORDER BY` (the order assertion fails).
#[test]
fn a_folder_with_a_domain_filter_lists_counts_and_agrees_with_itself() {
    let dir = tempfile::tempdir().unwrap();
    let (core, account) = lorvan_core(dir.path());
    let domain = save(&core, "Hotels", "from:@lorvan-hotels.example");
    let words = save(&core, "Hotel invoices", "from:@lorvan-hotels.example invoice");
    let rows = core.inbox(query(&domain));
    assert_eq!(ids_of(&rows), [S, B, A], "newest first; the sub-domain is kept, the look-alike, Z and the Sent copy are not");
    assert_eq!(state(&core, &format!("smart_folder/{domain}/count")).as_deref(), Some("3"));
    assert_eq!(state(&core, &format!("smart_folder/{domain}/unread")).as_deref(), Some("3"));
    assert_eq!(state(&core, &format!("smart_folder/{domain}/threads")).as_deref(), Some(format!("[\"{S}\",\"{B}\",\"{A}\"]").as_str()));
    assert_eq!(ids_of(&core.inbox(query(&words))), [A], "the domain and a word together");
    assert_eq!(state(&core, &format!("smart_folder/{words}/count")).as_deref(), Some("1"));
    assert_eq!(core.rule_match_count("from:@lorvan-hotels.example".into()), 3, "the editor's count is the same reading");
    // the list ignores the query's tab, as the contract says; the account set and the filters still apply
    assert_eq!(ids_of(&core.inbox(InboxQuery { tab_id: "updates".into(), ..query(&domain) })), [S, B, A]);
    assert!(core.inbox(InboxQuery { account_ids: vec!["acc-nobody".into()], ..query(&domain) }).is_empty());
    assert_eq!(core.inbox(InboxQuery { account_ids: vec![account.clone()], ..query(&domain) }).len(), 3);
    // reading one mail lowers unread, not count; the Unread filter agrees
    core.store().write(move |c| { c.tx.execute("UPDATE message SET read = 1 WHERE id = ?1", [B])?; Ok(()) }).unwrap();
    assert_eq!(state(&core, &format!("smart_folder/{domain}/count")).as_deref(), Some("3"));
    assert_eq!(state(&core, &format!("smart_folder/{domain}/unread")).as_deref(), Some("2"));
    assert_eq!(ids_of(&core.inbox(InboxQuery { filters: vec![InboxFilter::Unread], ..query(&domain) })), [S, A]);
    // an archive takes the mail out of the list and of every figure
    core.store().write(move |c| { c.tx.execute("UPDATE message SET archived = 1 WHERE id = ?1", [S])?; Ok(()) }).unwrap();
    assert_eq!(ids_of(&core.inbox(query(&domain))), [B, A]);
    assert_eq!(state(&core, &format!("smart_folder/{domain}/count")).as_deref(), Some("2"));
    assert_eq!(state(&core, &format!("smart_folder/{domain}/threads")).as_deref(), Some(format!("[\"{B}\",\"{A}\"]").as_str()));
    // unknown folder: an empty list, no figure
    assert!(core.inbox(query("sf-none")).is_empty());
    assert_eq!(state(&core, "smart_folder/sf-none/threads"), None);
    assert_eq!(state(&core, "smart_folder/sf-none/unread"), None);
}

/// An update changes what the folder lists at once; a delete empties its list; a rename keeps the list.
/// Broken copy: `smart_folder_update` writes the parent but not `rule_words` (the list after the update still shows A, B, S).
#[test]
fn updating_the_rule_changes_the_list_and_deleting_the_folder_ends_it() {
    let dir = tempfile::tempdir().unwrap();
    let (core, _) = lorvan_core(dir.path());
    let f = save(&core, "Mail", "from:@lorvan-hotels.example");
    assert_eq!(ids_of(&core.inbox(query(&f))), [S, B, A]);
    core.run(Command::UpdateSmartFolder { folder_id: f.clone(), rule_words: "from:@acme.test".into(), parent_id: None }).unwrap();
    assert_eq!(ids_of(&core.inbox(query(&f))), [Z], "the new rule lists the new mail");
    core.run(Command::RenameSmartFolder { folder_id: f.clone(), name: "Acme".into() }).unwrap();
    assert_eq!(ids_of(&core.inbox(query(&f))), [Z], "a rename keeps the list");
    assert_eq!(state(&core, &format!("smart_folder/{f}/count")).as_deref(), Some("1"));
    core.run(Command::DeleteSmartFolder { folder_id: f.clone() }).unwrap();
    assert!(core.inbox(query(&f)).is_empty());
    assert!(core.smart_folders().is_empty());
    assert_eq!(state(&core, &format!("smart_folder/{f}/threads")), None);
    // no mail was touched
    assert_eq!(core.rows_for_messages(vec![A.into(), Z.into()]).len(), 2);
}

/// The folders are kept on this Mac: a core opened again on the same files lists them, with their rule, parent and count.
/// Broken copy: `smart_folder_save` in store delta12.rs writes inside a transaction it rolls back (`Err` after the INSERT), or the rows live in memory only.
#[test]
fn the_folders_survive_a_restart() {
    let dir = tempfile::tempdir().unwrap();
    let (parent, child) = {
        let (core, _) = lorvan_core(dir.path());
        let parent = save(&core, "Hotels", "from:@lorvan-hotels.example");
        let child = save(&core, "Offers", "from:@mail.lorvan-hotels.example");
        core.run(Command::UpdateSmartFolder { folder_id: child.clone(), rule_words: "from:@mail.lorvan-hotels.example".into(), parent_id: Some(parent.clone()) }).unwrap();
        core.run(Command::RenameSmartFolder { folder_id: parent.clone(), name: "Lorvan hotels".into() }).unwrap();
        (parent, child)
    };
    let core = RustCore::open_production(dir.path().to_str().unwrap(), true).expect("the core opens again");
    let listed = core.smart_folders();
    assert_eq!(listed.iter().map(|f| (f.id.as_str(), f.name.as_str(), f.parent_id.as_deref())).collect::<Vec<_>>(),
        [(parent.as_str(), "Lorvan hotels", None), (child.as_str(), "Offers", Some(parent.as_str()))]);
    assert_eq!(listed[0].rule_words, "from:@lorvan-hotels.example");
    assert_eq!(state(&core, &format!("smart_folder/{parent}/count")).as_deref(), Some("3"));
    assert_eq!(ids_of(&core.inbox(query(&child))), [S]);
}

/// 2,000 stored mails: each command and each read answers under 50 ms (a test prints the time). One mail in ten holds the word.
/// Broken copy: a `std::thread::sleep(std::time::Duration::from_millis(60));` at the head of `smart_folder_cmds::run`, or of `smart_folder_cmds::members`.
#[test]
fn the_commands_and_reads_answer_under_50_ms_on_2000_mails() {
    const MAILS: usize = 2_000;
    let dir = tempfile::tempdir().unwrap();
    let (core, a) = open(dir.path());
    let batch: Vec<NewMessage> = (0..MAILS).map(|i| {
        let from = format!("sender{}@firm{}.test", i % 20, i % 4);
        let body = if i % 10 == 0 { format!("note {i}: the quarterly invoice is attached") } else { format!("note {i}: nothing to pay here") };
        mail(&a, &format!("s{i}"), "INBOX", &from, &format!("Subject {i}"), &body, 10_000 + i as i64)
    }).collect();
    let ids: Vec<String> = batch.iter().map(|m| m.message.id.clone()).collect();
    core.store().insert_messages(batch).unwrap();
    index(&core, &ids);
    fn timed<R>(what: &str, run: impl FnOnce() -> R) -> R {
        let t = Instant::now();
        let out = run();
        let ms = t.elapsed().as_millis();
        println!("smart folders, {what}: {ms} ms on {MAILS} mails");
        assert!(ms < BUDGET_MS, "{what} took {ms} ms, over {BUDGET_MS} ms");
        out
    }
    let domain = timed("save a domain folder", || save(&core, "Firm 1", "from:@firm1.test"));
    let words = timed("save a words folder", || save(&core, "Invoices", "invoice"));
    let both = timed("save a domain and words folder", || save(&core, "Firm 2 invoices", "from:@firm2.test invoice"));
    for i in 0..17 { save(&core, &format!("Spare {i}"), "from:@firm3.test"); }
    timed("rename", || core.run(Command::RenameSmartFolder { folder_id: domain.clone(), name: "Firm one".into() }).unwrap());
    timed("update", || core.run(Command::UpdateSmartFolder { folder_id: both.clone(), rule_words: "from:@firm2.test invoice".into(), parent_id: Some(words.clone()) }).unwrap());
    assert_eq!(timed("list 20 folders", || core.smart_folders()).len(), 20);
    for pass in ["cold", "warm"] {
        let n = timed(&format!("count of a domain folder, {pass}"), || state(&core, &format!("smart_folder/{domain}/count")));
        assert_eq!(n.as_deref(), Some((MAILS / 4).to_string().as_str()));
        let n = timed(&format!("count of a words folder, {pass}"), || state(&core, &format!("smart_folder/{words}/count")));
        assert_eq!(n.as_deref(), Some((MAILS / 10).to_string().as_str()));
        let n = timed(&format!("unread of a words folder, {pass}"), || state(&core, &format!("smart_folder/{words}/unread")));
        assert_eq!(n.as_deref(), Some((MAILS / 10).to_string().as_str()));
        let n = timed(&format!("list of a domain folder, {pass}"), || core.inbox(query(&domain)).len());
        assert_eq!(n, MAILS / 4);
        let n = timed(&format!("list of a domain and words folder, {pass}"), || core.inbox(query(&both)).len());
        assert!(n > 0 && n < MAILS / 4, "firm2 holds {n} invoices");
    }
    timed("delete", || core.run(Command::DeleteSmartFolder { folder_id: domain.clone() }).unwrap());
}
