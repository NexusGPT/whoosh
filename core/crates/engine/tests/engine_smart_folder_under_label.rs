//! fix-nesting (docs/JOURNEYS.md J2.6, the owner "nests 'Supplier quotes' under a 'Work' label"): a smart folder's parent is another smart folder OR
//! a label, on the REAL core. The app's editor sends the label's NAME as `UpdateSmartFolder.parent_id` (SmartFolderEditor.swift:71 chips carry the
//! name; LabelsContactsModel.swift:77-81 passes a name no smart folder carries as it is); the store resolves it to the label's id (by id first,
//! else by name as the label commands find one: `store::find_label`, its path, the case ignored) and keeps it in `smart_folder.parent_label_id`
//! (migration 0029); every read (`smart_folders()`, `smart_folder/<id>/parent`) answers the label's id. A deleted label sends its smart folders
//! to the top level (the lead's default; SPEC.md is silent).
//! Each test runs over the rig's production core with no model and names its one-line broken copy (red first). The speed test PRINTS one line
//! per operation (`smart folder under a label, <what>: N ms on 2000 mails`), each measured alone, and fails over 50 ms; run it with
//! `-- --nocapture` in the release profile. Only synthetic addresses (`.example`, `.test`) appear.
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

fn mail(account: &str, id: &str, from: &str, subject: &str, body: &str, at: i64) -> NewMessage {
    NewMessage {
        message: Message {
            id: id.into(), thread_id: format!("{id}:thread"), account_id: account.into(), sender_name: "Sender".into(), from_address: from.into(),
            to: vec![USER.into()], subject: subject.into(), preview: body.into(), received_at_ms: at, read: false, archived: false,
            folder: "INBOX".into(), attachment_names: vec![], login_code: None, trackers_blocked: 0, list_unsubscribe: vec![], list_unsubscribe_one_click: false,
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

fn save(core: &RustCore, name: &str, rule: &str) -> String {
    core.run(Command::SaveSmartFolder { name: name.into(), rule_words: rule.into() }).expect("the folder is saved").created_id.expect("a created id")
}

fn label(core: &RustCore, name: &str, parent: Option<&str>) -> String {
    core.run(Command::CreateLabel { name: name.into(), parent_id: parent.map(String::from) }).expect("the label is made").created_id.expect("the label's id")
}

/// What the app's editor sends on Save (LabelsContactsModel.swift:95, :98): the rule again and the parent.
fn nest(core: &RustCore, folder: &str, rule: &str, parent: Option<&str>) -> Result<CommandOutcome, CoreError> {
    core.run(Command::UpdateSmartFolder { folder_id: folder.into(), rule_words: rule.into(), parent_id: parent.map(String::from) })
}

fn state(core: &RustCore, path: &str) -> Option<String> { core.state(path.into()) }

fn quoted(s: &str) -> String { format!("\"{s}\"") }

/// The parent the sidebar's list gives a folder.
fn listed_parent(core: &RustCore, folder: &str) -> Option<String> {
    core.smart_folders().into_iter().find(|f| f.id == folder).expect("the folder is listed").parent_id
}

/// J2.6 as the app does it: the editor's chip sends "Work" (the label's name); both reads the app uses answer the label's ID. The label's id
/// and the name in another case are taken too.
/// Broken copy: in store delta12.rs `smart_folder_update`, replace `to.1 = Some(label.id);` with `return Ok(Err(refuse::not_found(p)));` (every
/// label parent is refused "smart folder ... not found" as before: the first `expect` fails).
#[test]
fn a_folder_nested_under_a_label_by_its_name_reads_back_the_labels_id() {
    let dir = tempfile::tempdir().unwrap();
    let (core, _) = open(dir.path());
    let work = label(&core, "Work", None);
    let quotes = save(&core, "Supplier quotes", "quote");
    let out = nest(&core, &quotes, "quote", Some("Work")).expect("a label's name is a parent");
    assert_eq!(out, CommandOutcome { undo_entry_id: None, created_id: None });
    assert_eq!(state(&core, &format!("smart_folder/{quotes}/parent")), Some(quoted(&work)), "the state path answers the label's id");
    assert_eq!(listed_parent(&core, &quotes), Some(work.clone()), "the sidebar's list answers the label's id");
    assert_eq!(core.smart_folders().len(), 1, "a folder under a label is still listed once");
    // the name in another case, and the label's id itself
    nest(&core, &quotes, "quote", None).unwrap();
    nest(&core, &quotes, "quote", Some("WORK")).expect("the case of the name is ignored, as the label commands ignore it");
    assert_eq!(listed_parent(&core, &quotes), Some(work.clone()));
    nest(&core, &quotes, "quote", None).unwrap();
    nest(&core, &quotes, "quote", Some(work.as_str())).expect("a label's id is a parent");
    assert_eq!(state(&core, &format!("smart_folder/{quotes}/parent")), Some(quoted(&work)));
    // the rule sent with the parent is kept (J2.6 "edits the rule")
    nest(&core, &quotes, "quote and has attachment", Some("Work")).unwrap();
    assert_eq!(state(&core, &format!("smart_folder/{quotes}/rule")), Some(quoted("quote and has attachment")));
    assert_eq!(listed_parent(&core, &quotes), Some(work));
}

/// A move between the two kinds leaves exactly one parent: label to smart folder, smart folder to label, either to the top level.
/// Broken copy: in `smart_folder_update`'s UPDATE, write `parent_label_id = COALESCE(?4, parent_label_id)` for `parent_label_id = ?4` (the old label
/// stays behind the new NULL: the "back to the top level" assertion reads the label again).
#[test]
fn a_move_between_a_label_a_folder_and_the_top_level_keeps_one_parent() {
    let dir = tempfile::tempdir().unwrap();
    let (core, _) = open(dir.path());
    let work = label(&core, "Work", None);
    let clients = save(&core, "Clients", "client");
    let quotes = save(&core, "Supplier quotes", "quote");
    nest(&core, &quotes, "quote", Some("Work")).unwrap();
    nest(&core, &quotes, "quote", Some(clients.as_str())).unwrap();
    assert_eq!(listed_parent(&core, &quotes), Some(clients.clone()), "a smart folder parent replaces the label");
    assert_eq!(core.smart_folders().iter().map(|f| f.name.as_str()).collect::<Vec<_>>(), ["Clients", "Supplier quotes"], "parents before children");
    nest(&core, &quotes, "quote", Some("Work")).unwrap();
    assert_eq!(listed_parent(&core, &quotes), Some(work), "a label replaces the smart folder parent");
    nest(&core, &quotes, "quote", None).unwrap();
    assert_eq!(listed_parent(&core, &quotes), None, "back to the top level: no parent of either kind is left");
    assert_eq!(state(&core, &format!("smart_folder/{quotes}/parent")), None);
}

/// The app turns a name into a smart folder's id when a folder carries it (LabelsContactsModel.swift:80); the core keeps the same order: a smart
/// folder of that name wins over a label of that name, its own name is left out (then the label is taken), and a smart folder parent keeps its
/// loop check.
/// Broken copy: in `smart_folder_update`, delete `.or_else(|| folders.iter().find(|f| f.id != id && f.name.to_lowercase() == lower))` (the name
/// "Work" then finds the label: the first assertion reads the label's id).
#[test]
fn a_smart_folder_of_the_same_name_wins_and_keeps_its_loop_check() {
    let dir = tempfile::tempdir().unwrap();
    let (core, _) = open(dir.path());
    let work_label = label(&core, "Work", None);
    let a = save(&core, "Work", "work");
    let b = save(&core, "Inner", "inner");
    nest(&core, &b, "inner", Some("work")).unwrap();
    assert_eq!(listed_parent(&core, &b), Some(a.clone()), "the smart folder named Work wins over the label named Work");
    let err = nest(&core, &a, "work", Some(b.as_str())).expect_err("a loop through smart folders is refused");
    assert_eq!(err, CoreError::Invalid { field: "parent".into(), reason_words: "a folder cannot sit inside one of its own folders".into() });
    nest(&core, &b, "inner", None).unwrap();
    nest(&core, &a, "work", Some("Work")).expect("its own name is left out, so the label of that name is taken");
    assert_eq!(listed_parent(&core, &a), Some(work_label));
}

/// Every refusal the smart folder update had is kept, with a label tree in the store, and none changes a folder.
/// Broken copy: in `smart_folder_update`, replace `crate::labels::get_label(c.tx, p)?.or(crate::labels::find_label(c.tx, p)?)` with
/// `crate::labels::find_label(c.tx, "Work")?` (any unknown parent lands under Work: the `missing("Nowhere")` assertion fails).
#[test]
fn every_refusal_is_kept_with_labels_in_the_store() {
    let dir = tempfile::tempdir().unwrap();
    let (core, _) = open(dir.path());
    let work = label(&core, "Work", None);
    let a = save(&core, "Supplier quotes", "quote");
    let invalid = |field: &str, words: &str| CoreError::Invalid { field: field.into(), reason_words: words.into() };
    let missing = |id: &str| CoreError::NotFound { kind: "smart folder".into(), id: id.into() };
    assert_eq!(nest(&core, &a, "quote", Some("Nowhere")).unwrap_err(), missing("Nowhere"), "neither a smart folder nor a label");
    assert_eq!(nest(&core, &a, "quote", Some("lbl-none")).unwrap_err(), missing("lbl-none"));
    assert_eq!(nest(&core, &a, "quote", Some(a.as_str())).unwrap_err(), invalid("parent", "a folder cannot sit inside itself"));
    assert_eq!(nest(&core, &a, "  ", Some("Work")).unwrap_err(), invalid("rule", "a smart folder needs a rule"));
    assert_eq!(nest(&core, "sf-none", "quote", Some("Work")).unwrap_err(), missing("sf-none"));
    assert_eq!(core.smart_folders(), vec![SmartFolder { id: a.clone(), name: "Supplier quotes".into(), rule_words: "quote".into(), parent_id: None }], "no refusal changed the folder");
    nest(&core, &a, "quote", Some(work.as_str())).unwrap();
    assert_eq!(nest(&core, &a, "quote", Some("Nowhere")).unwrap_err(), missing("Nowhere"));
    assert_eq!(listed_parent(&core, &a), Some(work), "a refused move keeps the label parent");
}

/// Kept on this Mac: a core opened again on the same files lists the folder under the label.
/// Broken copy: `all_folders` in store delta12.rs selects `parent_id` again instead of `COALESCE(parent_id, parent_label_id)` (the list after the
/// restart answers no parent).
#[test]
fn the_label_parent_survives_a_restart() {
    let dir = tempfile::tempdir().unwrap();
    let (work, quotes) = {
        let (core, _) = open(dir.path());
        let work = label(&core, "Work", None);
        let quotes = save(&core, "Supplier quotes", "quote");
        nest(&core, &quotes, "quote", Some("Work")).unwrap();
        (work, quotes)
    };
    let core = RustCore::open_production(dir.path().to_str().unwrap(), true).expect("the core opens again");
    assert_eq!(listed_parent(&core, &quotes), Some(work.clone()));
    assert_eq!(state(&core, &format!("smart_folder/{quotes}/parent")), Some(quoted(&work)));
}

/// A deleted label (with the label under it) sends its smart folders to the top level; the folders, their rules and every other parent stay.
/// Broken copy: in core/crates/store/migrations/sql/0029_smart_folder_label_parent.sql drop ` ON DELETE SET NULL` (the label's delete is refused by
/// the foreign key: the `DeleteLabel` expect fails).
#[test]
fn a_deleted_label_sends_its_smart_folders_to_the_top_level() {
    let dir = tempfile::tempdir().unwrap();
    let (core, _) = open(dir.path());
    let work = label(&core, "Work", None);
    let suppliers = label(&core, "Suppliers", Some(work.as_str()));
    let quotes = save(&core, "Supplier quotes", "quote");
    let deliveries = save(&core, "Deliveries", "delivery");
    let urgent = save(&core, "Urgent deliveries", "delivery urgent");
    nest(&core, &quotes, "quote", Some("Work")).unwrap();
    nest(&core, &deliveries, "delivery", Some(suppliers.as_str())).expect("a nested label's id is a parent");
    nest(&core, &urgent, "delivery urgent", Some(deliveries.as_str())).unwrap();
    core.run(Command::DeleteLabel { label_id: work.clone() }).expect("the label is deleted");
    let after = core.smart_folders();
    assert_eq!(after.len(), 3, "no smart folder is deleted with the label");
    assert_eq!(listed_parent(&core, &quotes), None, "under the deleted label: back to the top level");
    assert_eq!(listed_parent(&core, &deliveries), None, "under the label inside it: back to the top level");
    assert_eq!(listed_parent(&core, &urgent), Some(deliveries.clone()), "a smart folder parent is untouched");
    assert_eq!(state(&core, &format!("smart_folder/{quotes}/parent")), None);
    assert_eq!(state(&core, &format!("smart_folder/{quotes}/rule")), Some(quoted("quote")), "the rule stays");
}

/// A smart folder under a label that is deleted hands its own folders to that label (one level up), as a folder under a folder does.
/// Broken copy: in `smart_folder_delete`, write `parent_label_id = NULL` instead of `parent_label_id = ?3` in the UPDATE of the children (the child
/// goes to the top level instead of to the label).
#[test]
fn deleting_a_folder_under_a_label_moves_its_folders_to_the_label() {
    let dir = tempfile::tempdir().unwrap();
    let (core, _) = open(dir.path());
    let work = label(&core, "Work", None);
    let quotes = save(&core, "Supplier quotes", "quote");
    let urgent = save(&core, "Urgent quotes", "quote urgent");
    nest(&core, &quotes, "quote", Some("Work")).unwrap();
    nest(&core, &urgent, "quote urgent", Some(quotes.as_str())).unwrap();
    core.run(Command::DeleteSmartFolder { folder_id: quotes.clone() }).unwrap();
    assert_eq!(core.smart_folders().len(), 1);
    assert_eq!(listed_parent(&core, &urgent), Some(work.clone()), "the child moved up to the label");
    assert_eq!(state(&core, &format!("smart_folder/{urgent}/parent")), Some(quoted(&work)));
}

/// 2,000 stored mails: each command and read of a folder under a label answers under 50 ms, each measured alone (a line is printed per operation).
/// Broken copy: a `std::thread::sleep(std::time::Duration::from_millis(60));` at the head of `smart_folder_update` in store delta12.rs.
#[test]
fn each_command_and_read_under_a_label_answers_under_50_ms_on_2000_mails() {
    const MAILS: usize = 2_000;
    let dir = tempfile::tempdir().unwrap();
    let (core, a) = open(dir.path());
    let batch: Vec<NewMessage> = (0..MAILS).map(|i| {
        let body = if i % 10 == 0 { format!("note {i}: the supplier quote is attached") } else { format!("note {i}: nothing to quote here") };
        mail(&a, &format!("s{i}"), &format!("sender{}@firm{}.test", i % 20, i % 4), &format!("Subject {i}"), &body, 10_000 + i as i64)
    }).collect();
    let ids: Vec<String> = batch.iter().map(|m| m.message.id.clone()).collect();
    core.store().insert_messages(batch).unwrap();
    let bg = core.background();
    bg.reindex(&ids);
    assert!(bg.settle_index(WAIT), "the indexer finished");
    for i in 0..20 { label(&core, &format!("Label {i}"), None); }
    let work = label(&core, "Work", None);
    for i in 0..18 { save(&core, &format!("Spare {i}"), "from:@firm3.test"); }
    let quotes = save(&core, "Supplier quotes", "supplier");
    let urgent = save(&core, "Urgent quotes", "supplier quote");
    fn timed<R>(what: &str, run: impl FnOnce() -> R) -> R {
        let t = Instant::now();
        let out = run();
        let ms = t.elapsed().as_millis();
        println!("smart folder under a label, {what}: {ms} ms on {MAILS} mails");
        assert!(ms < BUDGET_MS, "{what} took {ms} ms, over {BUDGET_MS} ms");
        out
    }
    timed("update with the label's name", || nest(&core, &quotes, "supplier", Some("Work")).unwrap());
    timed("update with the label's id", || nest(&core, &quotes, "supplier", Some(work.as_str())).unwrap());
    timed("update with a smart folder parent", || nest(&core, &urgent, "supplier quote", Some(quotes.as_str())).unwrap());
    assert_eq!(timed("list 20 folders", || core.smart_folders()).len(), 20);
    assert_eq!(timed("the parent state path", || state(&core, &format!("smart_folder/{quotes}/parent"))), Some(quoted(&work)));
    assert_eq!(timed("the count of the folder under the label", || state(&core, &format!("smart_folder/{quotes}/count"))).as_deref(),
        Some((MAILS / 10).to_string().as_str()));
    timed("refused: an unknown parent", || nest(&core, &quotes, "supplier", Some("Nowhere")).unwrap_err());
    timed("delete the folder under the label", || core.run(Command::DeleteSmartFolder { folder_id: quotes.clone() }).unwrap());
    assert_eq!(listed_parent(&core, &urgent), Some(work.clone()));
    timed("delete the label", || core.run(Command::DeleteLabel { label_id: work.clone() }).unwrap());
    assert_eq!(listed_parent(&core, &urgent), None);
}
