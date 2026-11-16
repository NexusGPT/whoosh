//! Every typed round-2 op commits inside COMMIT_MS (docs/CONSTANTS.md) on a small store: the number is read from the
//! ledger, never typed here. The 1M-message cost of the reads is tests/label_budget.rs and tests/read_budget.rs.
mod common;
use common::*;
use std::time::{Duration, Instant};
use store::{FolderProgress, NewLabel};

fn timed<R>(budget: Duration, what: &str, f: impl FnOnce() -> R) -> R {
    let t = Instant::now();
    let out = f();
    let took = t.elapsed();
    assert!(took <= budget, "{what} took {took:?}, COMMIT_MS is {budget:?}");
    out
}

#[test]
fn each_typed_op_commits_inside_commit_ms() {
    let budget = Duration::from_millis(constant("COMMIT_MS").expect("the ledger has COMMIT_MS"));
    let db = TempDb::new("commit-budget");
    let store = small_store(&db, 30);
    store.counts().unwrap();
    let new = |id: &str, name: &str, parent: Option<&str>| NewLabel { id: Some(id.into()), name: name.into(), parent_id: parent.map(String::from), now_ms: 1, ..Default::default() };
    timed(budget, "label create", || store.label_create(new("c", "Clients", None)).unwrap());
    timed(budget, "label create under a parent", || store.label_create(new("p", "Provanta", Some("c"))).unwrap());
    timed(budget, "label ensure path", || store.label_ensure_path("Work > Q4".into(), vec![], 2).unwrap());
    timed(budget, "label assign", || store.label_assign("p".into(), vec!["m000".into(), "m001".into(), "m002".into()], 3).unwrap());
    timed(budget, "label rename", || store.label_rename("c".into(), "Customers".into(), 4).unwrap());
    timed(budget, "label move", || store.label_move("p".into(), None, 5).unwrap());
    timed(budget, "label unassign", || store.label_unassign("p".into(), vec!["m000".into()]).unwrap());
    timed(budget, "folder move", || store.set_folder(vec!["m003".into(), "m004".into()], "Trash".into()).unwrap());
    timed(budget, "account move", || store.move_to_account(vec!["m006".into()], "acc-b".into()).unwrap());
    timed(budget, "message delete", || store.delete_messages(vec!["m003".into()]).unwrap());
    timed(budget, "voice profile write", || store.set_voice_profile("owner0@example.test".into(), "Short.".into(), 1, 1).unwrap());
    let page: Vec<_> = (0..50).map(|i| new_msg(msg(&format!("q{i:03}"), &format!("qt{i:03}"), "acc-a", "x@ext.test", 1_700_000_100_000 + i), Some("primary"))).collect();
    timed(budget, "a page of 50 messages with its cursor", || {
        store.write_page(page, FolderProgress { account_id: "acc-a".into(), folder: "INBOX".into(), oldest: Some(1), done: false, cursor: None, now_ms: 9 }).unwrap()
    });
    timed(budget, "unread ids of a tab", || store.unread_ids(&contracts::InboxQuery { tab_id: "primary".into(), account_id: None, label_id: None, mailbox: None, account_ids: vec![], filters: vec![], smart_folder_id: None }).unwrap());
    assert_eq!(store.counts().unwrap(), store.recount().unwrap());
}
